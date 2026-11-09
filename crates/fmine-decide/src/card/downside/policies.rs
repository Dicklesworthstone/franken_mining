//! Exact bounded optimization of one-test, signal-contingent downside policies.
//! Determinism: Exact
//! Enumerates deterministic mappings, never actions selected after learning the true state.

use super::*;
use crate::card::information::{InformationStats, PolicyRisk};

mod render;
#[cfg(test)]
mod tests;

pub const METHOD: &str = "exact_deterministic";
pub const MAX_POLICIES: u64 = 65_536;
pub const MAX_WORK: u64 = 4_000_000;
pub const TIE_RULE: &str = "Among exactly optimal feasible deterministic mappings, retain the first lexicographic tuple of action_ids in ascending reachable signal_id order and report the exact number of co-optimal mappings. Unreachable signals have no action and do not multiply the policy count. A representative does not assert uniqueness or tail-risk optimality.";
pub const SCOPE: &str = "Opt-in exhaustive optimization of one test followed by one action per observed signal. The objective is nominal expected net payoff. The requested nominal or Wasserstein probability screen applies the declared strict monetary-floor event and inclusive probability cap to realized state/signal outcomes. Test and delay costs are charged once in every outcome, including do nothing. Only state weights vary in the Wasserstein screen; the original likelihoods remain fixed. All options and act-now alternatives are compared on the same admitted model. The search refuses before enumeration if its complete population exceeds the declared policy or work budget; no truncated optimum is emitted. This does not optimize randomized or sequential policies, constrain CVaR or loss severity, infer calibrated probabilities or authorize action.";
pub const WORK_UNIT: &str = "state/signal cells, state-weight cells and requested transport-objective state pairs; includes preparation and one representative risk law per option, excludes the pre-existing fixed-policy evaluation";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolicyScreen {
    Nominal,
    Wasserstein,
}

impl PolicyScreen {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Nominal => "nominal",
            Self::Wasserstein => "wasserstein",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicyOptimization {
    pub screen: PolicyScreen,
    /// Complete deterministic mappings across all declared information options.
    pub max_policies: u64,
    /// Conservative state/signal and transport-objective cells for the complete search.
    pub max_work: u64,
}

impl PolicyOptimization {
    pub(in crate::card) fn validate(&self) -> Result<()> {
        if !(1..=MAX_POLICIES).contains(&self.max_policies)
            || !(1..=MAX_WORK).contains(&self.max_work)
        {
            return Err(refusal(
                RefusalCode::OutsideAoa203,
                format!(
                    "policy optimization requires max_policies in 1..={MAX_POLICIES} and max_work in 1..={MAX_WORK}"
                ),
            ));
        }
        Ok(())
    }

    pub(super) fn encode(&self, enc: &mut Encoder) {
        enc.put_str("policy-optimization/v1");
        enc.put_str(METHOD);
        enc.put_str(self.screen.as_str());
        enc.put_u64(self.max_policies);
        enc.put_u64(self.max_work);
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OptimizedPolicy {
    /// Every declared signal in canonical order. Unreachable signals select no action.
    pub signal_actions: Vec<(String, Option<String>)>,
    pub nominal_mean: Rat,
    pub nominal_breach_probability: Rat,
    pub screened_breach_probability: Rat,
    pub conditional_breach_probabilities: Vec<Rat>,
    pub breach_certificate: Option<TransportCertificate>,
    /// Realized nominal law. Posterior tie signals retain their original EV-policy meaning.
    pub risk: PolicyRisk,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicyOptionSearch {
    pub option_id: String,
    pub reachable_signals: usize,
    pub policies_evaluated: u64,
    pub feasible_policies: u64,
    pub optimal_policy_count: u64,
    pub optimum: Option<OptimizedPolicy>,
    pub fixed_policy_mean: Rat,
    pub fixed_policy_screened_breach_probability: Rat,
    pub fixed_policy_eligible: bool,
    pub expected_value_cost_of_limit: Option<Rat>,
    pub improvement_over_best_eligible_act_now: Option<Rat>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicyOptimizationSummary {
    pub configuration: PolicyOptimization,
    pub policies_evaluated: u64,
    pub work_units: u64,
    pub options: Vec<PolicyOptionSearch>,
    pub best_eligible_act_now: DownsideRanking,
    /// One optimized representative per feasible option, plus every original act-now action.
    pub overall: DownsideRanking,
}

struct Work {
    counts: Vec<u64>,
    policies: u64,
    cells: u64,
}

fn exhausted(configuration: &PolicyOptimization) -> Refusal {
    refusal(
        RefusalCode::OutsideAoa203,
        format!(
            "complete policy optimization exceeds max_policies={} or max_work={}; no policy optimum was produced",
            configuration.max_policies, configuration.max_work
        ),
    )
}

/// Count by bounded multiplication; neither enumerate nor allocate the exponential population.
fn work(
    configuration: &PolicyOptimization,
    summary: &DecisionSummary,
    options: &[InformationStats],
) -> Result<Work> {
    let states = summary.alternatives[0].states.len() as u64;
    let actions = summary.alternatives.len() as u64;
    let mut work = Work {
        counts: Vec::new(),
        policies: 0,
        cells: 0,
    };
    for option in options {
        let reachable = option
            .signals
            .iter()
            .filter(|s| s.probability != ZERO)
            .count() as u64;
        if reachable == 0 {
            return Err(failed(
                "an admitted information option has no reachable signal",
            ));
        }
        let mut count = 1u64;
        for _ in 0..reachable {
            count = count
                .checked_mul(actions)
                .ok_or_else(|| exhausted(configuration))?;
            if count > configuration.max_policies {
                return Err(exhausted(configuration));
            }
        }
        work.policies = work
            .policies
            .checked_add(count)
            .ok_or_else(|| exhausted(configuration))?;
        let cells = (|| {
            let per_policy = reachable.checked_mul(states)?.checked_add(states)?;
            let per_policy = if configuration.screen == PolicyScreen::Wasserstein {
                per_policy.checked_add(states.checked_mul(states)?)?
            } else {
                per_policy
            };
            let preparation_and_risk = (option.signals.len() as u64)
                .checked_add(reachable)?
                .checked_mul(states)?;
            count
                .checked_mul(per_policy)?
                .checked_add(preparation_and_risk)
        })()
        .ok_or_else(|| exhausted(configuration))?;
        work.cells = work
            .cells
            .checked_add(cells)
            .ok_or_else(|| exhausted(configuration))?;
        if work.policies > configuration.max_policies || work.cells > configuration.max_work {
            return Err(exhausted(configuration));
        }
        work.counts.push(count);
    }
    Ok(work)
}

fn screened_probability(choice: &DownsideChoice, screen: PolicyScreen) -> Result<Rat> {
    match screen {
        PolicyScreen::Nominal => Ok(choice.nominal_breach_probability),
        PolicyScreen::Wasserstein => choice.worst_case_breach_probability.ok_or_else(|| {
            failed("Wasserstein policy optimization lacks a probability certificate")
        }),
    }
}

fn eligible(choice: &DownsideChoice, screen: PolicyScreen) -> bool {
    match screen {
        PolicyScreen::Nominal => choice.nominal_eligible,
        PolicyScreen::Wasserstein => choice.robust_eligible == Some(true),
    }
}

struct Search<'a> {
    summary: &'a DecisionSummary,
    limit: &'a DownsideLimit,
    screen: PolicyScreen,
    plan: Option<&'a TransportPlan>,
}

impl Search<'_> {
    fn choice(
        &self,
        option: &InformationStats,
        likelihoods: &[Vec<Rat>],
        mapping: &[usize],
    ) -> Result<DownsideChoice> {
        let states = &self.summary.alternatives[0].states;
        let cost = option.total_cost.major;
        let floor = arithmetic(Rat::new(self.limit.floor.minor, Money::SCALE))?;
        let mut gross = ZERO;
        let mut breaches = vec![ZERO; states.len()];
        for (&action, likelihoods) in mapping.iter().zip(likelihoods) {
            for (index, (state, likelihood)) in states.iter().zip(likelihoods).enumerate() {
                let payoff = self.summary.alternatives[action].states[index].payoff.major;
                let mass = arithmetic(state.weight.checked_mul(*likelihood))?;
                gross = arithmetic(gross.checked_add(arithmetic(payoff.checked_mul(mass))?))?;
                if compare(arithmetic(payoff.checked_sub(cost))?, floor).is_lt() {
                    breaches[index] = arithmetic(breaches[index].checked_add(*likelihood))?;
                }
            }
        }
        evaluate_choice(
            DownsideChoiceId {
                kind: ChoiceKind::InformationOption,
                id: option.option_id.clone(),
            },
            arithmetic(gross.checked_sub(cost))?,
            breaches,
            states,
            self.limit,
            self.plan,
            self.summary.robustness.as_ref().map(|r| r.design.radius),
        )
    }

    fn risk(
        &self,
        option: &InformationStats,
        likelihoods: &[Vec<Rat>],
        mapping: &[usize],
        mean: Rat,
    ) -> Result<PolicyRisk> {
        let states = &self.summary.alternatives[0].states;
        let mut masses = BTreeMap::new();
        for (&action, likelihoods) in mapping.iter().zip(likelihoods) {
            for (index, (state, likelihood)) in states.iter().zip(likelihoods).enumerate() {
                let mass = arithmetic(state.weight.checked_mul(*likelihood))?;
                if mass == ZERO {
                    continue;
                }
                let payoff = self.summary.alternatives[action].states[index].payoff.major;
                let minor = arithmetic(payoff.checked_mul(Rat::from_integer(Money::SCALE)))?;
                if minor.den() != 1 {
                    return Err(failed("optimized payoff left the monetary grid"));
                }
                let bin = masses.entry(minor.num()).or_insert(ZERO);
                *bin = arithmetic(bin.checked_add(mass))?;
            }
        }
        information::summarize_policy_risk(
            masses,
            self.summary.currency,
            option.total_cost.major,
            arithmetic(mean.checked_add(option.total_cost.major))?,
            option.policy_risk.reachable_tie_signals.clone(),
        )
    }

    fn option(
        &self,
        original: &InformationOption,
        option: &InformationStats,
        fixed: &DownsideChoice,
        count: u64,
        act_now: Option<Rat>,
    ) -> Result<PolicyOptionSearch> {
        let states = &self.summary.alternatives[0].states;
        let matrix: BTreeMap<_, _> = original
            .likelihoods
            .iter()
            .map(|l| ((l.state_id.as_str(), l.signal_id.as_str()), l.probability))
            .collect();
        let reachable: Vec<_> = option
            .signals
            .iter()
            .filter(|s| s.probability != ZERO)
            .collect();
        let likelihoods = reachable
            .iter()
            .map(|signal| {
                states
                    .iter()
                    .map(|state| {
                        matrix
                            .get(&(state.state_id.as_str(), signal.signal_id.as_str()))
                            .copied()
                            .ok_or_else(|| failed("policy search lost a source likelihood"))
                    })
                    .collect::<Result<Vec<_>>>()
            })
            .collect::<Result<Vec<_>>>()?;
        let mut mapping = vec![0usize; reachable.len()];
        let mut best: Option<(Vec<usize>, DownsideChoice)> = None;
        let (mut feasible, mut ties) = (0u64, 0u64);
        for _ in 0..count {
            let choice = self.choice(option, &likelihoods, &mapping)?;
            if eligible(&choice, self.screen) {
                feasible += 1;
                match best
                    .as_ref()
                    .map(|(_, b)| compare(choice.nominal_mean, b.nominal_mean))
                {
                    None | Some(Ordering::Greater) => {
                        best = Some((mapping.clone(), choice));
                        ties = 1;
                    }
                    Some(Ordering::Equal) => ties += 1,
                    Some(Ordering::Less) => {}
                }
            }
            for action in mapping.iter_mut().rev() {
                *action += 1;
                if *action < self.summary.alternatives.len() {
                    break;
                }
                *action = 0;
            }
        }
        let optimum = best
            .map(|(mapping, choice)| -> Result<OptimizedPolicy> {
                let selected: BTreeMap<_, _> = reachable
                    .iter()
                    .zip(&mapping)
                    .map(|(signal, &action)| {
                        (
                            signal.signal_id.as_str(),
                            self.summary.alternatives[action].action_id.clone(),
                        )
                    })
                    .collect();
                Ok(OptimizedPolicy {
                    signal_actions: option
                        .signals
                        .iter()
                        .map(|s| {
                            (
                                s.signal_id.clone(),
                                selected.get(s.signal_id.as_str()).cloned(),
                            )
                        })
                        .collect(),
                    nominal_mean: choice.nominal_mean,
                    nominal_breach_probability: choice.nominal_breach_probability,
                    screened_breach_probability: screened_probability(&choice, self.screen)?,
                    risk: self.risk(option, &likelihoods, &mapping, choice.nominal_mean)?,
                    conditional_breach_probabilities: choice.conditional_breach_probabilities,
                    breach_certificate: choice.breach_certificate,
                })
            })
            .transpose()?;
        let cost = optimum
            .as_ref()
            .map(|o| arithmetic(fixed.nominal_mean.checked_sub(o.nominal_mean)))
            .transpose()?;
        if cost.is_some_and(|c| c.signum() < 0) {
            return Err(failed(
                "constrained policy exceeds the unrestricted expected-value optimum",
            ));
        }
        let gain = optimum
            .as_ref()
            .zip(act_now)
            .map(|(o, baseline)| arithmetic(o.nominal_mean.checked_sub(baseline)))
            .transpose()?;
        Ok(PolicyOptionSearch {
            option_id: option.option_id.clone(),
            reachable_signals: reachable.len(),
            policies_evaluated: count,
            feasible_policies: feasible,
            optimal_policy_count: ties,
            optimum,
            fixed_policy_mean: fixed.nominal_mean,
            fixed_policy_screened_breach_probability: screened_probability(fixed, self.screen)?,
            fixed_policy_eligible: eligible(fixed, self.screen),
            expected_value_cost_of_limit: cost,
            improvement_over_best_eligible_act_now: gain,
        })
    }
}

pub(super) fn optimize(
    configuration: &PolicyOptimization,
    original: &[Claim<InformationOption>],
    summary: &DecisionSummary,
    limit: &DownsideLimit,
    fixed: &[DownsideChoice],
    plan: Option<&TransportPlan>,
) -> Result<PolicyOptimizationSummary> {
    let information = summary
        .information
        .as_ref()
        .filter(|i| !i.options.is_empty())
        .ok_or_else(|| {
            refusal(
                RefusalCode::NoClaimInDomain201,
                "policy optimization requires at least one information option",
            )
        })?;
    let robust = configuration.screen == PolicyScreen::Wasserstein;
    if robust && plan.is_none() {
        return Err(refusal(
            RefusalCode::NoClaimInDomain201,
            "Wasserstein policy optimization requires a declared robustness geometry",
        ));
    }
    // This checks every option before evaluating any mapping. An exhausted budget has no partial result.
    let work = work(configuration, summary, &information.options)?;
    let originals: BTreeMap<_, _> = original
        .iter()
        .map(|o| (o.value.option_id.as_str(), &o.value))
        .collect();
    let mut choices: Vec<_> = fixed
        .iter()
        .filter(|c| c.choice.kind == ChoiceKind::ActNow)
        .cloned()
        .collect();
    let best_eligible_act_now = ranking(&choices, robust)?;
    let search = Search {
        summary,
        limit,
        screen: configuration.screen,
        plan: if robust { plan } else { None },
    };
    let mut options = Vec::new();
    for (option, count) in information.options.iter().zip(work.counts) {
        let input = originals
            .get(option.option_id.as_str())
            .ok_or_else(|| failed("policy search lost an original option"))?;
        let fixed_choice = fixed
            .iter()
            .find(|c| {
                c.choice.kind == ChoiceKind::InformationOption && c.choice.id == option.option_id
            })
            .ok_or_else(|| failed("policy search lost the original fixed-policy comparison"))?;
        let result = search.option(
            input,
            option,
            fixed_choice,
            count,
            best_eligible_act_now.best_expected_value,
        )?;
        let mut choice = fixed_choice.clone();
        choice.nominal_eligible = false;
        choice.robust_eligible = Some(false);
        if let Some(optimum) = &result.optimum {
            choice.nominal_mean = optimum.nominal_mean;
            choice.nominal_breach_probability = optimum.nominal_breach_probability;
            choice.conditional_breach_probabilities =
                optimum.conditional_breach_probabilities.clone();
            choice.breach_certificate = optimum.breach_certificate.clone();
            choice.worst_case_breach_probability =
                robust.then_some(optimum.screened_breach_probability);
            choice.nominal_eligible = true;
            choice.robust_eligible = robust.then_some(true);
        }
        choices.push(choice);
        options.push(result);
    }
    let mut overall = ranking(&choices, robust)?;
    let unrestricted = fixed
        .iter()
        .map(|c| c.nominal_mean)
        .max_by(|a, b| compare(*a, *b))
        .ok_or_else(|| failed("policy search lost the unrestricted comparison"))?;
    overall.expected_value_cost_of_limit = overall
        .best_expected_value
        .map(|value| arithmetic(unrestricted.checked_sub(value)))
        .transpose()?;
    Ok(PolicyOptimizationSummary {
        configuration: configuration.clone(),
        policies_evaluated: work.policies,
        work_units: work.cells,
        options,
        best_eligible_act_now,
        overall,
    })
}
