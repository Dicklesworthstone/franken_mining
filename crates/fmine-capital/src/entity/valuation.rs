//! DESIGN (BD-314, §23.2): separate declared entity NAV/share outcomes under one primary objective.
//! Determinism: Exact (canonical shared-state matrices and rational share normalization).

use crate::*;

pub(crate) mod render;
#[cfg(test)]
mod tests;

pub const MAX_SECONDARY_ENTITIES: usize = 3;
pub const MAX_VALUATION_ROWS: usize = 65_536;

/// Already-net total equity-NAV change for one secondary entity in one shared state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CandidateNavChange {
    pub candidate_id: String,
    pub state_id: String,
    pub value: Money,
}

/// A separate valuation perimeter; its NAV is never inferred from the cash or ownership graph.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntityValuation {
    pub equity: EquityBasis,
    pub nav_changes: Vec<CandidateNavChange>,
}

impl EntityValuation {
    #[must_use]
    pub fn canonical_tag(&self) -> String {
        crate::render::value_tag("capital-entity-valuation/v1", &render::input_json(self))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntityValuationAssessment {
    pub entity: String,
    pub share_class: String,
    pub equity: EquityAssessment,
    /// Statistics use the population satisfying all entities' structural constraints, before risk.
    pub per_share_statistics: Option<PerShareStats>,
    pub per_share_tail_loss: Option<ExactPerShare>,
    pub max_tail_loss_per_share: Option<ExactPerShare>,
    /// A defined whole-policy tail loss strictly exceeds this entity's declared limit.
    pub risk_rejected: bool,
}

/// Counts retained equity-state rows for the complete static and, when supplied, policy populations.
pub(crate) fn check_work_bound(inputs: &ProgramInputs, population: usize) -> Result<()> {
    if inputs.entity_valuations.is_empty() {
        return Ok(());
    }
    let count = inputs.entity_valuations.len();
    let work = count
        .checked_add(1)
        .and_then(|entities| entities.checked_mul(inputs.weights.value.states.len()))
        .and_then(|rows| rows.checked_mul(population));
    if count > MAX_SECONDARY_ENTITIES || work.is_none_or(|rows| rows > MAX_VALUATION_ROWS) {
        return Err(bad(
            RefusalCode::NoClaimInDomain201,
            "secondary valuations admit at most three entities and 65,536 primary-plus-secondary state rows across the full static and policy populations",
        ));
    }
    Ok(())
}

pub(crate) fn validate_bounds(
    valuation: &EntityValuation,
    state_count: usize,
    candidate_count: usize,
) -> Result<()> {
    if !(1..=MAX_STATES).contains(&state_count) || !(1..=MAX_CANDIDATES).contains(&candidate_count)
    {
        return Err(bad(
            RefusalCode::NoClaimInDomain201,
            "secondary valuations require 1..=256 states and 1..=8 candidates",
        ));
    }
    validate_equity_bounds(&valuation.equity, state_count, candidate_count)?;
    if valuation.nav_changes.len() != state_count * candidate_count {
        return Err(bad(
            RefusalCode::NoClaimInDomain201,
            "secondary equity NAV requires every candidate/shared-state change, including explicit zeroes",
        ));
    }
    Ok(())
}

pub(crate) fn validate(
    claim: &Claim<EntityValuation>,
    policy: &ProgramPolicy,
    states: &BTreeSet<&str>,
    candidates: &BTreeMap<&str, usize>,
) -> Result<()> {
    let valuation = &claim.value;
    validate_equity_values(&valuation.equity, policy, states, candidates, false)?;
    let mut present = BTreeSet::new();
    for row in &valuation.nav_changes {
        if !candidates.contains_key(row.candidate_id.as_str())
            || !states.contains(row.state_id.as_str())
            || !present.insert((row.candidate_id.as_str(), row.state_id.as_str()))
            || row.value.ccy != policy.currency
        {
            return Err(bad(
                RefusalCode::BasisMismatch105,
                "secondary equity NAV changes require each known candidate/shared-state pair once in the policy currency",
            ));
        }
    }
    admit(
        claim,
        &valuation.canonical_tag(),
        AdditivityClass::NonAdditive,
    )
}

pub(crate) fn canonicalize(valuation: &mut EntityValuation) {
    valuation.equity.baseline_nav.sort_by(|a, b| a.0.cmp(&b.0));
    valuation
        .equity
        .share_changes
        .sort_by(|a, b| (&a.candidate_id, &a.state_id).cmp(&(&b.candidate_id, &b.state_id)));
    valuation
        .nav_changes
        .sort_by(|a, b| (&a.candidate_id, &a.state_id).cmp(&(&b.candidate_id, &b.state_id)));
}

pub(crate) struct Index<'a> {
    declarations: &'a [Claim<EntityValuation>],
    valuations: Vec<EquityIndex<'a>>,
}

impl<'a> Index<'a> {
    pub(crate) fn new(declarations: &'a [Claim<EntityValuation>]) -> Result<Self> {
        let valuations = declarations
            .iter()
            .map(|claim| {
                EquityIndex::with_nav_changes(
                    &claim.value.equity,
                    claim
                        .value
                        .nav_changes
                        .iter()
                        .map(|row| row.value)
                        .collect(),
                )
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(Self {
            declarations,
            valuations,
        })
    }

    pub(crate) fn assess(&self, selected_mask: usize) -> Result<Vec<EntityValuationAssessment>> {
        self.declarations
            .iter()
            .zip(&self.valuations)
            .map(|(claim, index)| {
                let basis = &claim.value.equity;
                Ok(EntityValuationAssessment {
                    entity: basis.entity.clone(),
                    share_class: basis.share_class.clone(),
                    equity: index.assess(selected_mask)?,
                    per_share_statistics: None,
                    per_share_tail_loss: None,
                    max_tail_loss_per_share: basis.max_tail_loss_per_share.map(|major| {
                        ExactPerShare {
                            currency: basis.currency,
                            major,
                        }
                    }),
                    risk_rejected: false,
                })
            })
            .collect()
    }
}

pub(crate) fn populate_statistics(
    choices: &mut [ProgramChoice],
    inputs: &ProgramInputs,
) -> Result<()> {
    for (entity_index, claim) in inputs.entity_valuations.iter().enumerate() {
        let missing = || {
            bad(
                RefusalCode::NumericalCertificateFailed207,
                "structurally feasible program lost a declared secondary entity valuation",
            )
        };
        let population = choices
            .iter()
            .enumerate()
            .filter(|(_, choice)| choice.structural_rejections.is_empty())
            .map(|(index, choice)| {
                let assessment = choice
                    .entity_valuations
                    .get(entity_index)
                    .ok_or_else(missing)?;
                if assessment.entity != claim.value.equity.entity
                    || assessment.share_class != claim.value.equity.share_class
                {
                    return Err(missing());
                }
                Ok((
                    index,
                    EquityStatisticsInput::new(choice, &assessment.equity),
                ))
            })
            .collect::<Result<Vec<_>>>()?;
        let outcomes = equity_statistics(
            &population
                .iter()
                .map(|(_, input)| input)
                .collect::<Vec<_>>(),
            &inputs.weights.value,
            claim.value.equity.currency,
            inputs.policy.value.tail_mass,
        )?;
        let indices: Vec<_> = population.iter().map(|(index, _)| *index).collect();
        drop(population);
        for (index, (statistics, tail)) in indices.into_iter().zip(outcomes) {
            let choice = choices.get_mut(index).ok_or_else(missing)?;
            let assessment = choice
                .entity_valuations
                .get_mut(entity_index)
                .ok_or_else(missing)?;
            assessment.per_share_statistics = Some(statistics);
            assessment.per_share_tail_loss = Some(tail);
            assessment.risk_rejected = claim
                .value
                .equity
                .max_tail_loss_per_share
                .is_some_and(|limit| exact_compare(tail.major, limit).is_gt());
            choice.risk_rejected |= assessment.risk_rejected;
        }
    }
    Ok(())
}
