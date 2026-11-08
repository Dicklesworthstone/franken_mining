//! Exact state-weight stress of the already selected information policies (BD-225/227, §22.5).
//! Determinism: Exact
//! Signal likelihoods, selected actions and costs never change under stress.

use super::*;
use crate::card::information::{ChoiceKind, InformationOption, InformationStats};

#[cfg(test)]
mod tests;

pub const FIXED_POLICY_SCOPE: &str = "Conditional finite-support Wasserstein-1 comparison of act-now actions and the displayed fixed one-test policies. For each original state, a policy's net value is its likelihood-weighted selected-action payoff minus test and delay costs exactly once. Only original state weights vary; signal likelihoods and the nominal selected actions, including the displayed resolution of posterior ties, remain fixed. Each policy has its own adversarial state law and exact transport certificate. These conditional state means are not realized signal outcomes or tail-risk estimates. This does not reoptimize signal policies, stress test accuracy, infer cross-test regret or joint signals, certify robust CVaR, or supply empirical confidence coverage.";

/// Kind qualifies the identifier: an action and an information option may share a name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FixedPolicyChoice {
    pub kind: ChoiceKind,
    pub id: String,
    pub nominal_mean: Rat,
    pub worst_case_mean: Rat,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RobustInformationPolicy {
    pub option_id: String,
    /// Original signal order; an impossible signal has no selected action.
    pub signal_actions: Vec<(String, Option<String>)>,
    pub posterior_tie_signals: Vec<String>,
    /// Conditional expected net payoff in the enclosing summary's canonical state order.
    pub state_net_payoffs: Vec<Rat>,
    pub nominal_mean: Rat,
    pub worst_case_mean: Rat,
    /// Maximizes negative conditional payoff, not loss CVaR or hindsight regret.
    pub loss_certificate: TransportCertificate,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FixedPolicyComparison {
    pub policies: Vec<RobustInformationPolicy>,
    pub nominal_expected_value_winners: Vec<FixedPolicyChoice>,
    pub maximin_mean_order: Vec<FixedPolicyChoice>,
    pub maximin_mean_winners: Vec<FixedPolicyChoice>,
    pub nominal_winner_survives_maximin: bool,
}

fn failed(what: &str) -> Refusal {
    refusal(RefusalCode::NumericalCertificateFailed207, what)
}

/// Rebuild conditional means from admitted likelihoods and original action payoffs. Posterior means
/// and the aggregated marginal risk law have lost the state dependence needed for this calculation.
fn conditional_payoffs(
    option: &InformationOption,
    policy: &InformationStats,
    summary: &DecisionSummary,
) -> Result<Vec<Rat>> {
    let states = &summary
        .alternatives
        .first()
        .ok_or_else(|| failed("fixed-policy robustness needs the original state support"))?
        .states;
    let cost = arithmetic(
        arithmetic(Rat::new(option.test_cost.minor, Money::SCALE))?
            .checked_add(arithmetic(Rat::new(option.delay_cost.minor, Money::SCALE))?),
    )?;
    if policy.option_id != option.option_id
        || policy.signals.len() != option.signals.len()
        || cost != policy.total_cost.major
        || option.currency != summary.currency
        || option.test_cost.ccy != summary.currency
        || option.delay_cost.ccy != summary.currency
    {
        return Err(failed(
            "fixed-policy robustness lost its admitted option or costs",
        ));
    }
    let actions: BTreeMap<_, _> = summary
        .alternatives
        .iter()
        .map(|a| (a.action_id.as_str(), a))
        .collect();
    let likelihoods: BTreeMap<_, _> = option
        .likelihoods
        .iter()
        .map(|l| ((l.state_id.as_str(), l.signal_id.as_str()), l.probability))
        .collect();
    let mut values = vec![arithmetic(cost.checked_neg())?; states.len()];
    let mut totals = vec![ZERO; states.len()];
    let mut signals = BTreeSet::new();
    for signal in &policy.signals {
        if !signals.insert(&signal.signal_id) || !option.signals.contains(&signal.signal_id) {
            return Err(failed(
                "fixed-policy robustness lost its original signal population",
            ));
        }
        if (signal.probability == ZERO && signal.selected_action.is_some())
            || (signal.probability != ZERO
                && (signal.selected_action.is_none()
                    || signal.selected_action.as_ref() != signal.best_actions.first()))
        {
            return Err(failed(
                "fixed-policy robustness must keep the displayed nominal policy",
            ));
        }
        let chosen = signal
            .selected_action
            .as_deref()
            .map(|id| {
                actions
                    .get(id)
                    .copied()
                    .ok_or_else(|| failed("fixed-policy robustness selected an absent action"))
            })
            .transpose()?;
        let mut probability = ZERO;
        for (index, state) in states.iter().enumerate() {
            let likelihood = *likelihoods
                .get(&(state.state_id.as_str(), signal.signal_id.as_str()))
                .ok_or_else(|| failed("fixed-policy robustness lacks an original likelihood"))?;
            if likelihood.signum() < 0 || compare(likelihood, ONE).is_gt() {
                return Err(failed("fixed-policy robustness has an invalid likelihood"));
            }
            totals[index] = arithmetic(totals[index].checked_add(likelihood))?;
            probability = arithmetic(
                probability.checked_add(arithmetic(state.weight.checked_mul(likelihood))?),
            )?;
            if likelihood == ZERO {
                continue;
            }
            let payoff = chosen
                .and_then(|a| a.states.get(index))
                .filter(|s| s.state_id == state.state_id && s.payoff.currency == summary.currency)
                .ok_or_else(|| failed("positive likelihood needs a selected same-state payoff"))?
                .payoff
                .major;
            values[index] =
                arithmetic(values[index].checked_add(arithmetic(likelihood.checked_mul(payoff))?))?;
        }
        if probability != signal.probability {
            return Err(failed(
                "fixed-policy robustness changed the nominal signal law",
            ));
        }
    }
    let mut mean = ZERO;
    for ((state, value), total) in states.iter().zip(&values).zip(totals) {
        if total != ONE {
            return Err(failed(
                "fixed-policy robustness does not cover every conditional law",
            ));
        }
        mean = arithmetic(mean.checked_add(arithmetic(state.weight.checked_mul(*value))?))?;
    }
    if mean != policy.net_value.major || mean != policy.policy_risk.mean.major {
        return Err(failed(
            "fixed-policy state means disagree with the admitted nominal value",
        ));
    }
    Ok(values)
}

pub(super) fn assess(
    options: &[Claim<InformationOption>],
    summary: &DecisionSummary,
    actions: &[RobustAlternative],
    plan: &TransportPlan,
    radius: Rat,
) -> Result<Option<FixedPolicyComparison>> {
    let Some(information) = &summary.information else {
        return if options.is_empty() {
            Ok(None)
        } else {
            Err(failed(
                "fixed-policy robustness lacks the nominal information comparison",
            ))
        };
    };
    if options.len() != information.options.len() || options.is_empty() {
        return Err(failed(
            "fixed-policy robustness lost an admitted information option",
        ));
    }
    let original: BTreeMap<_, _> = options
        .iter()
        .map(|c| (c.value.option_id.as_str(), &c.value))
        .collect();
    let mut policies = Vec::with_capacity(options.len());
    let mut choices: Vec<_> = actions
        .iter()
        .map(|a| FixedPolicyChoice {
            kind: ChoiceKind::ActNow,
            id: a.action_id.clone(),
            nominal_mean: a.nominal_mean,
            worst_case_mean: a.worst_case_mean,
        })
        .collect();
    for policy in &information.options {
        let option = original
            .get(policy.option_id.as_str())
            .ok_or_else(|| failed("fixed-policy robustness cannot find its original option"))?;
        let state_net_payoffs = conditional_payoffs(option, policy, summary)?;
        let losses = state_net_payoffs
            .iter()
            .map(|value| arithmetic(value.checked_neg()))
            .collect::<Result<Vec<_>>>()?;
        let loss_certificate = plan.maximize(&losses, radius)?;
        let worst_case_mean = arithmetic(loss_certificate.worst_case_value.checked_neg())?;
        let nominal_mean = policy.net_value.major;
        choices.push(FixedPolicyChoice {
            kind: ChoiceKind::InformationOption,
            id: policy.option_id.clone(),
            nominal_mean,
            worst_case_mean,
        });
        policies.push(RobustInformationPolicy {
            option_id: policy.option_id.clone(),
            signal_actions: policy
                .signals
                .iter()
                .map(|s| (s.signal_id.clone(), s.selected_action.clone()))
                .collect(),
            posterior_tie_signals: policy.policy_risk.reachable_tie_signals.clone(),
            state_net_payoffs,
            nominal_mean,
            worst_case_mean,
            loss_certificate,
        });
    }
    choices.sort_by(|a, b| {
        compare(b.nominal_mean, a.nominal_mean).then((a.kind, &a.id).cmp(&(b.kind, &b.id)))
    });
    let nominal_best = choices.first().map(|c| c.nominal_mean);
    let nominal_expected_value_winners: Vec<_> = choices
        .iter()
        .take_while(|c| Some(c.nominal_mean) == nominal_best)
        .cloned()
        .collect();
    choices.sort_by(|a, b| {
        compare(b.worst_case_mean, a.worst_case_mean).then((a.kind, &a.id).cmp(&(b.kind, &b.id)))
    });
    let robust_best = choices.first().map(|c| c.worst_case_mean);
    let maximin_mean_winners: Vec<_> = choices
        .iter()
        .take_while(|c| Some(c.worst_case_mean) == robust_best)
        .cloned()
        .collect();
    let nominal_winner_survives_maximin = maximin_mean_winners.iter().any(|winner| {
        nominal_expected_value_winners
            .iter()
            .any(|nominal| (winner.kind, &winner.id) == (nominal.kind, &nominal.id))
    });
    Ok(Some(FixedPolicyComparison {
        policies,
        nominal_expected_value_winners,
        maximin_mean_order: choices,
        maximin_mean_winners,
        nominal_winner_survives_maximin,
    }))
}
