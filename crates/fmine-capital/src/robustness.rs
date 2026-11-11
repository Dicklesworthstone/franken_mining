//! DESIGN (BD-229, §§22.5, 23.3): declared state-weight stress of nominally feasible capital choices.
//! Determinism: Exact (rational objectives and transport certificates on the original support).

use super::*;
pub use fmine_decide::card::robustness::{METHOD, RobustnessDesign, StateDistance};
use fmine_numerics::risk::transport::{TransportCertificate, TransportPlan};

mod render;
#[cfg(test)]
mod tests;

pub const MAX_TRANSPORT_STATES: usize = 64;
pub const MAX_TRANSPORT_WORK: usize = 2_097_152;
pub const NO_CLAIM: &str = "Conditional finite-support Wasserstein-1 comparison of every nominally feasible static bundle. Only the original shared-scenario weights vary within the declared dimensionless metric and radius; neither is inferred or calibrated. All original structural, cash, contractual and share-count checks remain binding over the complete support. Eligibility retains the original nominal tail-loss screens, including secondary entity limits; those screens are not recomputed or certified under the stressed weights. Mean and regret use the original primary objective, in USD or exact USD/share. Regret compares the same nominally feasible static bundles in each original state. Each bundle and criterion has its own adversarial law and exact primal/dual certificate. Stress rankings do not replace the nominal selection and do not certify robust CVaR feasibility, omitted regimes, adaptive policies, empirical confidence coverage, actual agreement terms or execution authorization.";
pub const POLICY_NO_CLAIM: &str = "Conditional finite-support Wasserstein-1 comparison of every nominally feasible two-stage policy. Only the original unconditional shared-state weights vary within the declared dimensionless metric and radius; neither is inferred or calibrated. Each policy retains its original paid information gate, observation partition, initial commitments and same-state continuation. No adversary selects a new continuation or reveals a hidden state. All original structural, cash, contractual and share-count checks remain binding over the complete support. Eligibility retains the original nominal tail-loss screens, including secondary entity limits; those screens are not recomputed or certified under stressed weights. Mean and regret use each complete policy's primary objective, in USD or exact USD/share. Regret compares the same nominally feasible policies in each original state; the separate static comparison retains its own population. Each policy and criterion has its own adversarial law and exact primal/dual certificate. Stress rankings do not replace nominal selections or net adaptive value and do not certify robust CVaR feasibility, dynamic adversarial decisions, omitted regimes, empirical confidence coverage, actual agreement terms or execution authorization.";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RobustnessScope {
    StaticBundles,
    AdaptivePolicies,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RobustBundle {
    /// A static bundle id or an adaptive policy id, as declared by the summary's scope.
    pub bundle_id: String,
    pub nominal_mean: Rat,
    pub worst_case_mean: Rat,
    pub nominal_mean_regret: Rat,
    pub worst_case_expected_regret: Rat,
    /// Maximizes signed loss; its value is the negative of worst_case_mean.
    pub loss_certificate: TransportCertificate,
    pub regret_certificate: TransportCertificate,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RobustnessSummary {
    pub scope: RobustnessScope,
    pub design: RobustnessDesign,
    pub objective_unit: String,
    pub state_ids: Vec<String>,
    pub nominal_weights: Vec<Rat>,
    pub structural_rejections: Vec<String>,
    pub nominal_risk_rejections: Vec<String>,
    pub nominal_expected_value_winners: Vec<String>,
    pub maximin_mean_winners: Vec<String>,
    pub minimax_expected_regret_winners: Vec<String>,
    pub nominal_winner_survives_maximin: bool,
    pub nominal_winner_survives_minimax_regret: bool,
    pub bundles: Vec<RobustBundle>,
}

fn invalid(why: &str) -> Refusal {
    bad(RefusalCode::NoClaimInDomain201, why)
}

/// Bound the complete population, including rejected bundles, before hashing or enumerating it.
pub(crate) fn check_work(inputs: &ProgramInputs) -> Result<()> {
    if inputs.robustness.is_none() {
        return Ok(());
    }
    let population = 1_usize
        .checked_shl(inputs.candidates.len() as u32)
        .ok_or_else(|| invalid("capital transport population exceeds its bounded domain"))?;
    check_population_work(inputs, population)
}

/// Adaptive evaluation also retains the static population; neither may evade the shared work cap.
pub(crate) fn check_population_work(inputs: &ProgramInputs, population: usize) -> Result<()> {
    let Some(claim) = &inputs.robustness else {
        return Ok(());
    };
    let d = &claim.value;
    let states = inputs.weights.value.states.len();
    let cells = states.checked_mul(states);
    let work = cells.and_then(|n| population.checked_mul(2)?.checked_mul(n));
    if !(1..=MAX_TRANSPORT_STATES).contains(&states)
        || cells != Some(d.distances.len())
        || work.is_none_or(|n| n > MAX_TRANSPORT_WORK)
    {
        return Err(invalid(
            "capital Wasserstein stress requires 1..=64 states, every ordered distance pair and at most 2097152 objective/state-pair evaluations over all declared static subsets and policies",
        ));
    }
    if d.method != METHOD
        || d.radius.signum() < 0
        || !key(&d.model_key)
        || !named(&d.metric_basis)
        || !named(&d.radius_basis)
        || d.distances
            .iter()
            .any(|row| !key(&row.from_state) || !key(&row.to_state))
    {
        return Err(invalid(
            "capital Wasserstein stress needs wasserstein_1, a nonnegative exact radius, explicit metric/radius bases and bounded state identifiers",
        ));
    }
    Ok(())
}

pub(crate) fn prepare(inputs: &ProgramInputs) -> Result<Option<TransportPlan>> {
    let Some(claim) = &inputs.robustness else {
        return Ok(None);
    };
    let mut states = inputs.weights.value.states.clone();
    states.sort_by(|a, b| a.0.cmp(&b.0));
    let indices: BTreeMap<_, _> = states
        .iter()
        .enumerate()
        .map(|(i, (state, _))| (state.as_str(), i))
        .collect();
    let mut matrix = vec![vec![None; states.len()]; states.len()];
    for row in &claim.value.distances {
        let (Some(&i), Some(&j)) = (
            indices.get(row.from_state.as_str()),
            indices.get(row.to_state.as_str()),
        ) else {
            return Err(bad(
                RefusalCode::BasisMismatch105,
                "transport distance names an unknown shared state",
            ));
        };
        if matrix[i][j].replace(row.distance).is_some() {
            return Err(bad(
                RefusalCode::BasisMismatch105,
                "duplicate transport state pair",
            ));
        }
    }
    let costs = matrix
        .into_iter()
        .map(|row| {
            row.into_iter()
                .map(|value| {
                    value.ok_or_else(|| invalid("transport matrix omits an original state pair"))
                })
                .collect::<Result<Vec<_>>>()
        })
        .collect::<Result<Vec<_>>>()?;
    let weights: Vec<_> = states.iter().map(|(_, weight)| *weight).collect();
    TransportPlan::new(&weights, &costs).map(Some)
}

fn winners(rows: &[RobustBundle], regret: bool) -> Vec<String> {
    let value = |row: &RobustBundle| {
        if regret {
            row.worst_case_expected_regret
        } else {
            row.worst_case_mean
        }
    };
    let best = rows.iter().map(value).reduce(|a, b| {
        if exact_compare(a, b).is_lt() == regret {
            a
        } else {
            b
        }
    });
    rows.iter()
        .filter(|row| Some(value(row)) == best)
        .map(|row| row.bundle_id.clone())
        .collect()
}

pub(crate) fn assess(
    inputs: &ProgramInputs,
    choices: &[ProgramChoice],
    nominal_winners: &[String],
    plan: Option<&TransportPlan>,
) -> Result<Option<RobustnessSummary>> {
    assess_population(
        inputs,
        choices,
        nominal_winners,
        plan,
        RobustnessScope::StaticBundles,
    )
}

pub(crate) fn assess_policies(
    inputs: &ProgramInputs,
    choices: &[ProgramChoice],
    nominal_winners: &[String],
    plan: Option<&TransportPlan>,
) -> Result<Option<RobustnessSummary>> {
    assess_population(
        inputs,
        choices,
        nominal_winners,
        plan,
        RobustnessScope::AdaptivePolicies,
    )
}

fn assess_population(
    inputs: &ProgramInputs,
    choices: &[ProgramChoice],
    nominal_winners: &[String],
    plan: Option<&TransportPlan>,
    scope: RobustnessScope,
) -> Result<Option<RobustnessSummary>> {
    let Some(claim) = &inputs.robustness else {
        return Ok(None);
    };
    let missing = || {
        bad(
            RefusalCode::NumericalCertificateFailed207,
            "capital stress lost complete admitted primary-objective outcomes",
        )
    };
    let plan = plan.ok_or_else(missing)?;
    let states = &inputs.weights.value.states;
    let mut eligible = Vec::new();
    let mut structural_rejections = Vec::new();
    let mut nominal_risk_rejections = Vec::new();
    for choice in choices {
        if !choice.structural_rejections.is_empty() {
            structural_rejections.push(choice.bundle_id.clone());
        } else if choice.risk_rejected {
            nominal_risk_rejections.push(choice.bundle_id.clone());
        } else {
            let values = if inputs.equity.is_some() {
                let stats = choice.per_share_statistics.as_ref().ok_or_else(missing)?;
                fmine_decide::card::stability::ordered_values(
                    states,
                    stats
                        .states
                        .iter()
                        .map(|row| (row.state_id.as_str(), row.payoff.major)),
                )?
            } else {
                let stats = choice.statistics.as_ref().ok_or_else(missing)?;
                fmine_decide::card::stability::ordered_values(
                    states,
                    stats
                        .states
                        .iter()
                        .map(|row| (row.state_id.as_str(), row.payoff.major)),
                )?
            };
            eligible.push((choice.bundle_id.clone(), values));
        }
    }
    let best: Vec<_> = (0..states.len())
        .map(|i| {
            eligible
                .iter()
                .map(|(_, values)| values[i])
                .max_by(|a, b| exact_compare(*a, *b))
        })
        .collect();
    let mean = |values: &[Rat]| {
        values
            .iter()
            .zip(states)
            .try_fold(ZERO, |sum, (value, (_, weight))| {
                arithmetic(sum.checked_add(arithmetic(value.checked_mul(*weight))?))
            })
    };
    let mut bundles = Vec::with_capacity(eligible.len());
    for (bundle_id, values) in eligible {
        let losses = values
            .iter()
            .map(|value| arithmetic(ZERO.checked_sub(*value)))
            .collect::<Result<Vec<_>>>()?;
        let regrets = values
            .iter()
            .zip(&best)
            .map(|(value, best)| arithmetic(best.ok_or_else(missing)?.checked_sub(*value)))
            .collect::<Result<Vec<_>>>()?;
        let loss_certificate = plan.maximize(&losses, claim.value.radius)?;
        let regret_certificate = plan.maximize(&regrets, claim.value.radius)?;
        bundles.push(RobustBundle {
            bundle_id,
            nominal_mean: mean(&values)?,
            worst_case_mean: arithmetic(ZERO.checked_sub(loss_certificate.worst_case_value))?,
            nominal_mean_regret: mean(&regrets)?,
            worst_case_expected_regret: regret_certificate.worst_case_value,
            loss_certificate,
            regret_certificate,
        });
    }
    let maximin_mean_winners = winners(&bundles, false);
    let minimax_expected_regret_winners = winners(&bundles, true);
    let survives = |winners: &[String]| winners.iter().any(|id| nominal_winners.contains(id));
    let mut design = claim.value.clone();
    design
        .distances
        .sort_by(|a, b| (&a.from_state, &a.to_state).cmp(&(&b.from_state, &b.to_state)));
    Ok(Some(RobustnessSummary {
        scope,
        design,
        objective_unit: if inputs.equity.is_some() {
            "USD/share"
        } else {
            "USD"
        }
        .into(),
        state_ids: states.iter().map(|(id, _)| id.clone()).collect(),
        nominal_weights: states.iter().map(|(_, weight)| *weight).collect(),
        structural_rejections,
        nominal_risk_rejections,
        nominal_expected_value_winners: nominal_winners.to_vec(),
        nominal_winner_survives_maximin: survives(&maximin_mean_winners),
        nominal_winner_survives_minimax_regret: survives(&minimax_expected_regret_winners),
        maximin_mean_winners,
        minimax_expected_regret_winners,
        bundles,
    }))
}
