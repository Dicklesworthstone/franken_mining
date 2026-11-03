//! DESIGN (BD-225, §22.5): exact finite-support Wasserstein checks on declared act-now alternatives.
//! Determinism: Exact (rational transport witnesses, common support and canonical action ordering).

use super::*;
use fmine_numerics::risk::exact::compare;
use fmine_numerics::risk::transport::{TransportCertificate, TransportPlan};

mod render;
#[cfg(test)]
mod tests;

pub const METHOD: &str = "wasserstein_1";
pub const MAX_TRANSPORT_STATES: usize = 64;
pub const MAX_TRANSPORT_WORK: usize = 2_097_152;
pub const NO_CLAIM: &str = "Conditional finite-support Wasserstein-1 robustness of the original act-now actions. The declared dimensionless metric and radius define which scenario weights may change; neither is inferred or calibrated. Every action keeps its original payoff in every named state. Worst-case mean and worst-case expected same-state regret use separate adversarial distributions and exact primal/dual transport certificates. This does not optimize information policies, certify robust CVaR or hard constraints, add omitted regimes, supply empirical confidence coverage, or authorize execution. Nominal information values and nominal tail statistics remain separate.";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateDistance {
    pub from_state: String,
    pub to_state: String,
    pub distance: Rat,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RobustnessDesign {
    pub model_key: String,
    pub method: String,
    pub radius: Rat,
    /// Explains the explicitly dimensionless metric; there is no implicit normalization.
    pub metric_basis: String,
    /// Explains the ambiguity budget, without asserting a confidence level or fitting it to outcomes.
    pub radius_basis: String,
    /// Every ordered pair is explicit, including zero diagonal entries.
    pub distances: Vec<StateDistance>,
}

impl RobustnessDesign {
    #[must_use]
    pub fn canonical_tag(&self) -> String {
        let mut enc = Encoder::new_frame(*b"WSDG", 1);
        for text in [
            &self.model_key,
            &self.method,
            &self.metric_basis,
            &self.radius_basis,
        ] {
            enc.put_str(text);
        }
        put_rat(&mut enc, self.radius);
        let mut rows: Vec<_> = self.distances.iter().collect();
        rows.sort_by(|a, b| {
            (&a.from_state, &a.to_state)
                .cmp(&(&b.from_state, &b.to_state))
                .then_with(|| compare(a.distance, b.distance))
        });
        enc.put_u64(rows.len() as u64);
        for row in rows {
            enc.put_str(&row.from_state);
            enc.put_str(&row.to_state);
            put_rat(&mut enc, row.distance);
        }
        format!("decision-wasserstein-design/v1:{}", sha256(&enc.finish()))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RobustAlternative {
    pub action_id: String,
    pub nominal_mean: Rat,
    pub worst_case_mean: Rat,
    pub nominal_mean_regret: Rat,
    pub worst_case_expected_regret: Rat,
    /// Maximizes signed loss = -payoff, so its value is the negative of worst_case_mean.
    pub loss_certificate: TransportCertificate,
    pub regret_certificate: TransportCertificate,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RobustnessSummary {
    pub design: RobustnessDesign,
    pub currency: Currency,
    pub state_ids: Vec<String>,
    pub nominal_weights: Vec<Rat>,
    pub nominal_expected_value_winners: Vec<String>,
    pub maximin_mean_order: Vec<String>,
    pub maximin_mean_winners: Vec<String>,
    pub minimax_expected_regret_order: Vec<String>,
    pub minimax_expected_regret_winners: Vec<String>,
    pub nominal_winner_survives_maximin: bool,
    pub nominal_winner_survives_minimax_regret: bool,
    pub alternatives: Vec<RobustAlternative>,
}

fn bad(why: &str) -> Refusal {
    refusal(RefusalCode::NoClaimInDomain201, why)
}

fn text(value: &str) -> bool {
    named(value) && value.len() <= 4096
}

fn identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'.'))
}

/// Bounds all new work before cloning the complete card, hashing configuration or allocating geometry.
pub(super) fn check_work(inputs: &DecisionInputs) -> Result<()> {
    let Some(claim) = &inputs.robustness else {
        return Ok(());
    };
    let design = &claim.value;
    let states = inputs.weights.value.states.len();
    let cells = states.checked_mul(states);
    let work = cells
        .and_then(|n| n.checked_mul(inputs.actions.len()))
        .and_then(|n| n.checked_mul(2));
    if !(1..=MAX_TRANSPORT_STATES).contains(&states)
        || cells != Some(design.distances.len())
        || work.is_none_or(|n| n > MAX_TRANSPORT_WORK)
    {
        return Err(bad(
            "Wasserstein robustness requires 1..=64 states, every ordered distance pair and at most 2097152 action/state-pair evaluations",
        ));
    }
    if design.method != METHOD
        || design.radius.signum() < 0
        || !text(&design.model_key)
        || !text(&design.metric_basis)
        || !text(&design.radius_basis)
        || inputs
            .weights
            .value
            .states
            .iter()
            .any(|(id, _)| !identifier(id))
        || inputs
            .actions
            .iter()
            .any(|a| !identifier(&a.value.action_id))
        || design
            .distances
            .iter()
            .any(|r| !identifier(&r.from_state) || !identifier(&r.to_state))
    {
        return Err(bad(
            "Wasserstein robustness needs method wasserstein_1, a nonnegative radius, explicit metric/radius bases and identifiers of at most 128 ASCII identifier bytes",
        ));
    }
    Ok(())
}

fn rankings(rows: &[RobustAlternative], regret: bool) -> (Vec<String>, Vec<String>) {
    let value = |i: usize| {
        if regret {
            rows[i].worst_case_expected_regret
        } else {
            rows[i].worst_case_mean
        }
    };
    let mut order: Vec<_> = (0..rows.len()).collect();
    order.sort_by(|a, b| {
        let cmp = compare(value(*a), value(*b));
        (if regret { cmp } else { cmp.reverse() }).then(rows[*a].action_id.cmp(&rows[*b].action_id))
    });
    let best = order.first().map(|i| value(*i));
    let winners = order
        .iter()
        .filter(|i| Some(value(**i)) == best)
        .map(|i| rows[*i].action_id.clone())
        .collect();
    (
        order
            .into_iter()
            .map(|i| rows[i].action_id.clone())
            .collect(),
        winners,
    )
}

/// Called only after the complete original action/state population has passed normal card admission.
pub(super) fn assess(
    configuration: &mut Option<Claim<RobustnessDesign>>,
    summary: &DecisionSummary,
) -> Result<(Option<RobustnessSummary>, ColorRank)> {
    let Some(claim) = configuration else {
        return Ok((None, ColorRank::Verified));
    };
    let color = admit(
        claim,
        &claim.value.canonical_tag(),
        AdditivityClass::NonAdditive,
    )?;
    if claim.value.model_key != summary.model_key {
        return Err(refusal(
            RefusalCode::BasisMismatch105,
            "Wasserstein configuration belongs to another model",
        ));
    }
    claim
        .value
        .distances
        .sort_by(|a, b| (&a.from_state, &a.to_state).cmp(&(&b.from_state, &b.to_state)));
    let first = summary
        .alternatives
        .first()
        .ok_or_else(|| bad("Wasserstein comparison has no original action"))?;
    let state_ids: Vec<_> = first.states.iter().map(|s| s.state_id.clone()).collect();
    let nominal_weights: Vec<_> = first.states.iter().map(|s| s.weight).collect();
    let index: BTreeMap<_, _> = state_ids
        .iter()
        .enumerate()
        .map(|(i, id)| (id.as_str(), i))
        .collect();
    let mut matrix = vec![vec![None; state_ids.len()]; state_ids.len()];
    for row in &claim.value.distances {
        let (Some(&from), Some(&to)) = (
            index.get(row.from_state.as_str()),
            index.get(row.to_state.as_str()),
        ) else {
            return Err(refusal(
                RefusalCode::BasisMismatch105,
                "Wasserstein distance references an unknown original state",
            ));
        };
        if matrix[from][to].replace(row.distance).is_some() {
            return Err(refusal(
                RefusalCode::BasisMismatch105,
                "Wasserstein distance repeats an original state pair",
            ));
        }
    }
    let costs = matrix
        .into_iter()
        .map(|row| {
            row.into_iter()
                .map(|v| v.ok_or_else(|| bad("Wasserstein metric omits an original state pair")))
                .collect::<Result<Vec<_>>>()
        })
        .collect::<Result<Vec<_>>>()?;
    let plan = TransportPlan::new(&nominal_weights, &costs)?;
    let mut alternatives = Vec::with_capacity(summary.alternatives.len());
    for action in &summary.alternatives {
        if action.states.len() != state_ids.len()
            || action
                .states
                .iter()
                .zip(&state_ids)
                .any(|(s, id)| &s.state_id != id)
        {
            return Err(bad(
                "Wasserstein action lost the common original state support",
            ));
        }
        let losses = action
            .states
            .iter()
            .map(|s| arithmetic(ZERO.checked_sub(s.payoff.major)))
            .collect::<Result<Vec<_>>>()?;
        let regrets: Vec<_> = action.states.iter().map(|s| s.regret.major).collect();
        let loss_certificate = plan.maximize(&losses, claim.value.radius)?;
        let regret_certificate = plan.maximize(&regrets, claim.value.radius)?;
        alternatives.push(RobustAlternative {
            action_id: action.action_id.clone(),
            nominal_mean: action.mean.major,
            worst_case_mean: arithmetic(ZERO.checked_sub(loss_certificate.worst_case_value))?,
            nominal_mean_regret: action.mean_regret.major,
            worst_case_expected_regret: regret_certificate.worst_case_value,
            loss_certificate,
            regret_certificate,
        });
    }
    let (maximin_mean_order, maximin_mean_winners) = rankings(&alternatives, false);
    let (minimax_expected_regret_order, minimax_expected_regret_winners) =
        rankings(&alternatives, true);
    let survives = |winners: &[String]| {
        winners
            .iter()
            .any(|id| summary.expected_value_winners.contains(id))
    };
    Ok((
        Some(RobustnessSummary {
            design: claim.value.clone(),
            currency: summary.currency,
            state_ids,
            nominal_weights,
            nominal_expected_value_winners: summary.expected_value_winners.clone(),
            nominal_winner_survives_maximin: survives(&maximin_mean_winners),
            nominal_winner_survives_minimax_regret: survives(&minimax_expected_regret_winners),
            maximin_mean_order,
            maximin_mean_winners,
            minimax_expected_regret_order,
            minimax_expected_regret_winners,
            alternatives,
        }),
        color,
    ))
}
