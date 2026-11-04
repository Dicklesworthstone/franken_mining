//! DESIGN (BD-229): adapt assessed capital outcomes to the shared decision robustness engine.
//! Determinism: Exact (cash, ownership, debt and information policies are assessed only once).

use super::*;
pub use engine::{METHOD, NO_CLAIM, SelectionStabilityDesign, SelectionStabilitySummary};
use fmine_decide::card::stability::{
    self as engine, SelectionScope, SelectionStabilityProblem, StabilityAlternative,
    StabilityTailLimit, ordered_values as ordered,
};

pub(crate) fn check_work(inputs: &ProgramInputs, population: usize) -> Result<()> {
    if let Some(claim) = &inputs.selection_stability {
        claim.value.check_work(
            inputs.weights.value.states.len(),
            population,
            1 + inputs.entity_valuations.len(),
        )?;
    }
    Ok(())
}

fn missing() -> Refusal {
    bad(
        RefusalCode::NumericalCertificateFailed207,
        "stability requires complete assessed outcomes on the original support",
    )
}

/// `policies` accompanies the already assessed comparison rows before those rows move into the report.
pub(crate) fn assess(
    inputs: &ProgramInputs,
    choices: &[ProgramChoice],
    policies: Option<&[recourse::PolicyChoice]>,
    metadata: &Metadata,
    nominal_winners: &[String],
) -> Result<Option<SelectionStabilitySummary>> {
    let Some(design) = &inputs.selection_stability else {
        return Ok(None);
    };
    let states = &inputs.weights.value.states;
    let candidates: Vec<_> = inputs
        .candidates
        .iter()
        .map(|c| c.value.candidate_id.clone())
        .collect();
    let mask = |selected: &[String]| {
        candidates.iter().enumerate().fold(0_u16, |mask, (i, id)| {
            mask | (u16::from(selected.contains(id)) << i)
        })
    };
    let mut scopes = vec![SelectionScope {
        scope: policies.map_or("static", |_| "initial").into(),
        state_id: None,
    }];
    if policies.is_some() {
        scopes.extend(states.iter().map(|(state, _)| SelectionScope {
            scope: "realized_state".into(),
            state_id: Some(state.clone()),
        }));
    }
    let mut alternatives = Vec::with_capacity(choices.len());
    for (index, choice) in choices.iter().enumerate() {
        let structurally_feasible = choice.structural_rejections.is_empty();
        let mut selections = vec![mask(&choice.selected)];
        if let Some(policy) = policies.and_then(|rows| rows.get(index)) {
            selections = vec![mask(&policy.initial_selected)];
            for (state, _) in states {
                let row = policy
                    .states
                    .iter()
                    .find(|row| &row.state_id == state)
                    .ok_or_else(missing)?;
                selections.push(mask(&row.selected));
            }
        }
        let mut values = Vec::new();
        let mut tail_limits = Vec::new();
        if structurally_feasible {
            values = if inputs.equity.is_some() {
                let stats = choice.per_share_statistics.as_ref().ok_or_else(missing)?;
                ordered(
                    states,
                    stats
                        .states
                        .iter()
                        .map(|row| (row.state_id.as_str(), row.payoff.major)),
                )?
            } else {
                let stats = choice.statistics.as_ref().ok_or_else(missing)?;
                ordered(
                    states,
                    stats
                        .states
                        .iter()
                        .map(|row| (row.state_id.as_str(), row.payoff.major)),
                )?
            };
            for valuation in &choice.entity_valuations {
                if let Some(cap) = valuation.max_tail_loss_per_share {
                    let stats = valuation
                        .per_share_statistics
                        .as_ref()
                        .ok_or_else(missing)?;
                    tail_limits.push(StabilityTailLimit {
                        name: valuation.entity.clone(),
                        limit: cap.major,
                        values: ordered(
                            states,
                            stats
                                .states
                                .iter()
                                .map(|row| (row.state_id.as_str(), row.payoff.major)),
                        )?,
                    });
                }
            }
        }
        alternatives.push(StabilityAlternative {
            alternative_id: choice.bundle_id.clone(),
            structurally_feasible,
            values,
            tail_limits,
            selections,
        });
    }
    let problem = SelectionStabilityProblem {
        design: design.value.clone(),
        states: states.clone(),
        objective_unit: if inputs.equity.is_some() {
            "USD/share"
        } else {
            "USD"
        }
        .into(),
        tail_mass: inputs.policy.value.tail_mass,
        primary_tail_limit: match &inputs.equity {
            Some(equity) => equity.value.max_tail_loss_per_share,
            None => inputs.policy.value.max_tail_loss.map(amount).transpose()?,
        },
        candidate_ids: candidates,
        scopes,
        alternatives,
    };
    let tag = problem.canonical_tag();
    let claim = metadata.claim(
        "capital.selection-stability-problem/1",
        problem,
        &tag,
        AdditivityClass::NonAdditive,
        true,
    );
    let result = engine::evaluate(&claim)?.value;
    if result.nominal_winners != nominal_winners {
        return Err(bad(
            RefusalCode::NumericalCertificateFailed207,
            "stability changed the original model's nominal winners",
        ));
    }
    Ok(Some(result))
}
