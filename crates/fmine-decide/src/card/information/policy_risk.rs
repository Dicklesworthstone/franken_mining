//! Realized risk of one fixed signal-contingent policy, not risk of posterior mean forecasts.
//! Determinism: Exact (rational joint masses; original integer money orders the distribution).
//!
//! The supplied likelihood matrix suffices for this policy's marginal law. It does not supply the
//! counterfactual joint signals needed for cross-test regret or sequential/batched information.

use std::collections::{BTreeMap, BTreeSet};

use super::{InformationOption, SignalPolicy};
use crate::card::{DecisionSummary, ExactMoney, ONE, Result, ZERO, arithmetic, quantile, refusal};
use fmine_evidence::{Refusal, RefusalCode};
use fmine_numerics::exact_feas::Rat;
use fmine_units::{Currency, Money};

pub const POLICY_RULE: &str = "For each reachable signal, follow its displayed selected_action: the first action_id among exact expected-payoff co-winners. This is a fixed expected-value-optimal policy, not a CVaR-optimal policy. Other posterior-tied actions may have different tail risks; reachable ties are listed explicitly.";
pub const RISK_SCOPE: &str = "Exact marginal law of realized net payoff over state and signal, with joint mass P(state) times P(signal | state). Test and delay costs are subtracted once in every outcome, including when the policy chooses do nothing. Quantiles are weighted inverse-CDF net payoffs; CVaR95 is the largest 5% of signed loss = -net payoff, with fractional boundary mass and negative all-gain loss retained. Loss probability means net payoff below zero. No calibrated probabilities, cross-option regret, joint test-signal dependence, safety assessment or authorization are inferred.";

/// A positive mass in the aggregated, ascending-payoff law. Uniform cost preserves gross ordering.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicyPayoffMass {
    pub probability: Rat,
    pub gross_payoff: ExactMoney,
    pub net_payoff: ExactMoney,
}

/// Derived once during admission, retained in the sealed card and bound by its output Claim and digest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicyRisk {
    pub mean: ExactMoney,
    pub p10_p50_p90: [ExactMoney; 3],
    pub cvar95_loss: ExactMoney,
    pub loss_probability: Rat,
    pub distribution: Vec<PolicyPayoffMass>,
    pub reachable_tie_signals: Vec<String>,
}

fn failed(detail: &str) -> Refusal {
    refusal(RefusalCode::NumericalCertificateFailed207, detail)
}

/// Called only after complete matrix/Claim admission and the exact expected-value policy evaluation.
/// Reconstructs masses from the original likelihoods rather than treating posterior means as outcomes.
pub(super) fn evaluate(
    option: &InformationOption,
    base: &DecisionSummary,
    policies: &[SignalPolicy],
    likelihoods: &BTreeMap<(&str, &str), Rat>,
    total_cost: Rat,
    expected_gross: Rat,
) -> Result<PolicyRisk> {
    if policies.len() != option.signals.len() {
        return Err(failed("risk policy does not cover every declared signal"));
    }
    let states = &base
        .alternatives
        .first()
        .ok_or_else(|| failed("risk needs the admitted state population"))?
        .states;
    let actions: BTreeMap<_, _> = base
        .alternatives
        .iter()
        .map(|a| (a.action_id.as_str(), a))
        .collect();
    let mut seen = BTreeSet::new();
    let mut masses: BTreeMap<i128, Rat> = BTreeMap::new();
    let mut tied = Vec::new();
    for policy in policies {
        if !seen.insert(&policy.signal_id) || !option.signals.contains(&policy.signal_id) {
            return Err(failed("risk policy has a duplicate or undeclared signal"));
        }
        let chosen = policy
            .selected_action
            .as_deref()
            .map(|id| {
                actions
                    .get(id)
                    .copied()
                    .ok_or_else(|| failed("risk policy names an absent action"))
            })
            .transpose()?;
        if policy.probability == ZERO {
            if chosen.is_some() || !policy.best_actions.is_empty() {
                return Err(failed("an unreachable signal cannot select a risk policy"));
            }
        } else {
            if chosen.is_none() || policy.selected_action.as_ref() != policy.best_actions.first() {
                return Err(failed(
                    "risk must follow the displayed expected-value tie policy",
                ));
            }
            if policy.best_actions.len() > 1 {
                tied.push(policy.signal_id.clone());
            }
        }
        let mut signal_mass = ZERO;
        for (index, state) in states.iter().enumerate() {
            let likelihood = likelihoods
                .get(&(state.state_id.as_str(), policy.signal_id.as_str()))
                .ok_or_else(|| failed("risk likelihood matrix is incomplete"))?;
            let mass = arithmetic(state.weight.checked_mul(*likelihood))?;
            if mass.signum() < 0 {
                return Err(failed("negative joint risk mass"));
            }
            signal_mass = arithmetic(signal_mass.checked_add(mass))?;
            if mass == ZERO {
                continue;
            }
            let payoff = chosen
                .and_then(|a| a.states.get(index))
                .filter(|s| s.state_id == state.state_id && s.payoff.currency == base.currency)
                .ok_or_else(|| failed("risk lacks the chosen payoff on the same state support"))?
                .payoff;
            // Every admitted action payoff originated in exact Money. Recovering that integer avoids
            // overflow-prone rational comparison during sorting. Never round a non-monetary value.
            let minor = arithmetic(payoff.major.checked_mul(Rat::from_integer(Money::SCALE)))?;
            if minor.den() != 1 {
                return Err(failed("risk payoff is not on the admitted monetary grid"));
            }
            let slot = masses.entry(minor.num()).or_insert(ZERO);
            *slot = arithmetic(slot.checked_add(mass))?;
        }
        if signal_mass != policy.probability {
            return Err(failed(
                "risk joint masses disagree with the signal probability",
            ));
        }
    }
    tied.sort();
    summarize(masses, base.currency, total_cost, expected_gross, tied)
}

fn summarize(
    masses: BTreeMap<i128, Rat>,
    currency: Currency,
    cost: Rat,
    expected_gross: Rat,
    reachable_tie_signals: Vec<String>,
) -> Result<PolicyRisk> {
    if masses.is_empty() || cost.signum() < 0 {
        return Err(failed(
            "risk needs a nonempty law and nonnegative uniform costs",
        ));
    }
    let money = |major| ExactMoney { currency, major };
    let mut distribution = Vec::with_capacity(masses.len());
    let (mut total, mut gross_mean, mut net_mean, mut loss_probability) = (ZERO, ZERO, ZERO, ZERO);
    for (minor, probability) in masses {
        if probability.signum() <= 0 {
            return Err(failed("risk law bins must carry strictly positive mass"));
        }
        let gross = arithmetic(Rat::new(minor, Money::SCALE))?;
        let net = arithmetic(gross.checked_sub(cost))?;
        total = arithmetic(total.checked_add(probability))?;
        gross_mean =
            arithmetic(gross_mean.checked_add(arithmetic(gross.checked_mul(probability))?))?;
        net_mean = arithmetic(net_mean.checked_add(arithmetic(net.checked_mul(probability))?))?;
        if net.signum() < 0 {
            loss_probability = arithmetic(loss_probability.checked_add(probability))?;
        }
        distribution.push(PolicyPayoffMass {
            probability,
            gross_payoff: money(gross),
            net_payoff: money(net),
        });
    }
    if total != ONE
        || gross_mean != expected_gross
        || net_mean != arithmetic(expected_gross.checked_sub(cost))?
    {
        return Err(failed(
            "risk law must have unit mass and reproduce the policy's exact net value",
        ));
    }
    let sorted: Vec<_> = distribution
        .iter()
        .map(|bin| (bin.net_payoff.major, bin.probability))
        .collect();
    let mut quantiles = [money(ZERO); 3];
    for (index, (num, den)) in [(1, 10), (1, 2), (9, 10)].into_iter().enumerate() {
        quantiles[index] = money(quantile(&sorted, arithmetic(Rat::new(num, den))?)?);
    }
    let tail = arithmetic(Rat::new(1, 20))?;
    let (mut remaining, mut integral) = (tail, ZERO);
    for (net, mass) in sorted {
        if remaining == ZERO {
            break;
        }
        let take = if arithmetic(mass.checked_cmp(remaining))? == std::cmp::Ordering::Greater {
            remaining
        } else {
            mass
        };
        integral = arithmetic(integral.checked_add(arithmetic(net.checked_mul(take))?))?;
        remaining = arithmetic(remaining.checked_sub(take))?;
    }
    if remaining != ZERO {
        return Err(failed("risk law does not fill the required tail mass"));
    }
    Ok(PolicyRisk {
        mean: money(net_mean),
        p10_p50_p90: quantiles,
        cvar95_loss: money(arithmetic(
            arithmetic(integral.checked_div(tail))?.checked_neg(),
        )?),
        loss_probability,
        distribution,
        reachable_tie_signals,
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::card::{evaluate, model};
    use fmine_core::anchor::SourceAnchor;
    use fmine_core::hash::sha256;

    fn r(n: i128, d: i128) -> Rat {
        Rat::new(n, d).unwrap()
    }

    fn law(rows: &[(i128, Rat)], cost: Rat) -> PolicyRisk {
        let mean = rows.iter().fold(ZERO, |sum, (value, p)| {
            sum.checked_add(Rat::from_integer(*value).checked_mul(*p).unwrap())
                .unwrap()
        });
        summarize(
            rows.iter().map(|(v, p)| (v * Money::SCALE, *p)).collect(),
            Currency::USD,
            cost,
            mean,
            Vec::new(),
        )
        .unwrap()
    }

    #[test]
    fn actual_gb29_policy_has_more_tail_loss_despite_positive_net_information_value() {
        // Joint outcomes: high/good=51%, high/bad=9%, low/good=6%, low/bad=34%.
        // The bad signal chooses do nothing; it still pays both test and delay cost.
        let risk = law(
            &[
                (300_000_000, r(51, 100)),
                (0, r(43, 100)),
                (-200_000_000, r(3, 50)),
            ],
            r(9_800_000, 1),
        );
        assert_eq!(risk.mean.major, r(131_200_000, 1));
        assert_eq!(risk.cvar95_loss.major, r(209_800_000, 1));
        assert_eq!(risk.loss_probability, r(49, 100));
        assert_eq!(
            risk.p10_p50_p90.map(|m| m.major),
            [r(-9_800_000, 1), r(290_200_000, 1), r(290_200_000, 1)]
        );
        assert_eq!(risk.distribution.len(), 3);
        assert_eq!(risk.distribution[1].gross_payoff.major, ZERO);
        assert_eq!(risk.distribution[1].net_payoff.major, r(-9_800_000, 1));
    }

    #[test]
    fn fractional_tail_boundary_and_inverse_cdf_endpoints_are_exact() {
        let risk = law(
            &[(-100, r(1, 50)), (-50, r(2, 25)), (10, r(9, 10))],
            r(3, 1),
        );
        assert_eq!(risk.mean.major, ZERO);
        assert_eq!(risk.cvar95_loss.major, r(73, 1));
        assert_eq!(risk.loss_probability, r(1, 10));
        assert_eq!(
            risk.p10_p50_p90.map(|m| m.major),
            [r(-53, 1), r(7, 1), r(7, 1)]
        );
    }

    #[test]
    fn all_gain_signed_losses_and_uniform_cost_translation_are_preserved() {
        let raw = law(&[(10, r(1, 2)), (30, r(1, 2))], ZERO);
        let cost = law(&[(10, r(1, 2)), (30, r(1, 2))], r(2, 1));
        assert_eq!(raw.cvar95_loss.major, r(-10, 1));
        assert_eq!(cost.cvar95_loss.major, r(-8, 1));
        assert_eq!(
            cost.mean.major,
            raw.mean.major.checked_sub(r(2, 1)).unwrap()
        );
        assert_eq!(cost.loss_probability, ZERO);
        let zero = law(&[(0, ONE)], ZERO);
        assert_eq!(zero.loss_probability, ZERO);
        assert_eq!(zero.cvar95_loss.major, ZERO);
    }

    #[test]
    fn incomplete_mass_wrong_mean_and_unrepresentable_net_outcomes_refuse() {
        for (mass, mean) in [(r(1, 2), r(5, 1)), (ONE, r(11, 1)), (ZERO, ZERO)] {
            let err = summarize(
                BTreeMap::from([(10 * Money::SCALE, mass)]),
                Currency::USD,
                ZERO,
                mean,
                Vec::new(),
            )
            .unwrap_err();
            assert_eq!(err.code, "FMINE-REF-207");
        }
        assert!(
            summarize(
                BTreeMap::from([(-Money::SCALE, ONE)]),
                Currency::USD,
                Rat::from_integer(i128::MAX),
                r(-1, 1),
                Vec::new(),
            )
            .is_err()
        );
    }

    #[test]
    fn full_model_retains_unreachable_signals_and_exposes_risk_relevant_posterior_ties() {
        let mut text = String::from(
            "schema = \"fmine.decision-model.v1\"\nmodel_key = \"invented-risk\"\nquestion = \"Compare invented policies\"\ncurrency = \"USD\"\nvaluation_basis = \"invented NPV\"\n",
        );
        for state in ["high", "low"] {
            text.push_str(&format!(
                "\n[[state]]\nid = \"{state}\"\nweight = \"1/2\"\n"
            ));
        }
        for (action, role, values) in [
            ("a_risky", "action", [10, -10]),
            ("z_nothing", "do_nothing", [0, 0]),
        ] {
            text.push_str(&format!(
                "\n[[action]]\nid = \"{action}\"\nlabel = \"{action}\"\nrole = \"{role}\"\n"
            ));
            for (state, value) in ["high", "low"].into_iter().zip(values) {
                text.push_str(&format!("\n[[payoff]]\naction = \"{action}\"\nstate = \"{state}\"\nvalue = \"{value}\"\n"));
            }
        }
        text.push_str("\n[[uu]]\nid = \"omission\"\ndescription = \"Invented omitted event\"\ntrigger = \"new event\"\ndownside_usd_m = \"1\"\n\n[[information_option]]\nid = \"test\"\nlabel = \"Invented test\"\ntest_cost = \"1\"\ndelay_cost = \"2\"\ndelay_days = 7\n");
        for (signal, probability) in [("seen", "1/1"), ("impossible", "0/1")] {
            text.push_str(&format!(
                "\n[[signal]]\noption = \"test\"\nid = \"{signal}\"\n"
            ));
            for state in ["high", "low"] {
                text.push_str(&format!("\n[[likelihood]]\noption = \"test\"\nstate = \"{state}\"\nsignal = \"{signal}\"\nprobability = \"{probability}\"\n"));
            }
        }
        let inputs = model::parse(
            &text,
            SourceAnchor::new("invented.toml", sha256(text.as_bytes()).0, "model"),
        )
        .unwrap();
        let original = inputs.clone();
        let card = evaluate(&inputs).unwrap();
        let option = &card.summary().information.as_ref().unwrap().options[0];
        assert_eq!(option.policy_risk.reachable_tie_signals, ["seen"]);
        assert_eq!(option.signals[0].signal_id, "impossible");
        assert!(option.signals[0].selected_action.is_none());
        assert_eq!(option.policy_risk.distribution.len(), 2);
        assert_eq!(option.policy_risk.mean, option.net_value);
        assert_eq!(option.policy_risk.cvar95_loss.major, r(13, 1));
        assert_eq!(option.policy_risk.loss_probability, r(1, 2));
        let json = card.to_json();
        assert!(json.contains("\"policy_risk\":"));
        assert!(json.contains("\"reachable_tie_signals\":[\"seen\"]"));
        let mut reordered = inputs.clone();
        reordered.information_options[0].value.signals.reverse();
        reordered.information_options[0].value.likelihoods.reverse();
        reordered.actions.reverse();
        assert_eq!(evaluate(&reordered).unwrap().to_json(), json);
        assert_eq!(inputs, original, "evaluation must not mutate the model");
    }
}
