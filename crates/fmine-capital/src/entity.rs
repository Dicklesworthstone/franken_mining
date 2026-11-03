//! DESIGN (BD-314, §23.2): exact entity cash, fixed funding calls and gated ownership distributions.
//! Determinism: Exact (declared shared states, canonical period order and rational ownership splits).

use super::*;
use debt::{DebtCashFlows, DebtDistributionGate, DebtServiceResult, EntityDebtFunding};

pub mod debt;
pub(super) mod render;
#[cfg(test)]
mod tests;
pub mod valuation;

pub const MAX_ENTITIES: usize = 4;
pub const MAX_TRANSFERS: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entity {
    pub id: String,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntityPeriod {
    pub id: String,
    pub cash_period: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntityFloor {
    pub entity_id: String,
    pub period_id: String,
    pub minimum_cash: Money,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntityOpeningCash {
    pub entity_id: String,
    pub state_id: String,
    pub value: Money,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntityStateCashFlow {
    pub entity_id: String,
    pub period_id: String,
    pub state_id: String,
    pub value: Money,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntityCandidateCashFlow {
    pub entity_id: String,
    pub period_id: String,
    pub candidate_id: String,
    pub state_id: String,
    pub value: Money,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntityTransfer {
    pub id: String,
    pub from_entity: String,
    pub to_entity: String,
    pub period_id: String,
    /// None is an unconditional transfer; otherwise the named candidate activates it.
    pub candidate_id: Option<String>,
    pub amounts: Vec<(String, Money)>,
    pub basis: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntityDistribution {
    pub entity_id: String,
    pub parent_entity_id: String,
    /// The remainder is paid outside the modeled entity graph, never credited to the parent.
    pub parent_share: Rat,
    pub basis: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntityDistributionGate {
    pub entity_id: String,
    pub period_id: String,
    pub state_id: String,
    pub allowed: bool,
    pub maximum_distribution: Money,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntityFunding {
    pub model_key: String,
    pub currency: Currency,
    pub cash_basis: String,
    pub opening_date: String,
    pub objective_entity: String,
    pub entities: Vec<Entity>,
    pub periods: Vec<EntityPeriod>,
    pub floors: Vec<EntityFloor>,
    pub opening_cash: Vec<EntityOpeningCash>,
    pub baseline_flows: Vec<EntityStateCashFlow>,
    pub candidate_flows: Vec<EntityCandidateCashFlow>,
    pub transfers: Vec<EntityTransfer>,
    pub distributions: Vec<EntityDistribution>,
    pub gates: Vec<EntityDistributionGate>,
    pub debt: Option<EntityDebtFunding>,
}

impl EntityFunding {
    #[must_use]
    pub fn canonical_tag(&self) -> String {
        super::render::value_tag("capital-entity-funding/v1", &render::funding_json(self))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntityCashPhase {
    Opening,
    PreDistribution,
    Closing,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntityCashCheckpoint {
    pub entity_id: String,
    pub period_id: Option<String>,
    pub checkpoint_date: String,
    pub phase: EntityCashPhase,
    pub opening_cash: ExactMoney,
    pub baseline_cash_flow: ExactMoney,
    pub candidate_cash_flow: ExactMoney,
    pub transfer_in: ExactMoney,
    pub transfer_out: ExactMoney,
    pub distribution_in: ExactMoney,
    pub distribution_out: ExactMoney,
    pub closing_cash: ExactMoney,
    pub minimum_cash: ExactMoney,
    pub slack: ExactMoney,
    pub debt: Option<DebtCashFlows>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntityTransferResult {
    pub transfer_id: String,
    pub from_entity: String,
    pub to_entity: String,
    pub period_id: String,
    pub checkpoint_date: String,
    pub candidate_id: Option<String>,
    pub basis: String,
    pub active: bool,
    pub declared_amount: ExactMoney,
    pub actual_amount: ExactMoney,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntityDistributionResult {
    pub entity_id: String,
    pub parent_entity_id: String,
    pub period_id: String,
    pub checkpoint_date: String,
    pub basis: String,
    pub parent_share: Rat,
    pub external_share: Rat,
    pub allowed: bool,
    pub maximum_distribution: ExactMoney,
    pub cash_before_distribution: ExactMoney,
    pub minimum_cash: ExactMoney,
    pub available_surplus: ExactMoney,
    pub gross_distribution: ExactMoney,
    pub parent_receipt: ExactMoney,
    pub external_distribution: ExactMoney,
    pub cash_after_distribution: ExactMoney,
    pub debt_gate: Option<DebtDistributionGate>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntityConservation {
    pub period_id: String,
    pub checkpoint_date: String,
    pub opening_cash: ExactMoney,
    pub baseline_cash_flow: ExactMoney,
    pub candidate_cash_flow: ExactMoney,
    pub internal_transfers: ExactMoney,
    pub internal_distributions: ExactMoney,
    pub external_distributions: ExactMoney,
    pub closing_cash: ExactMoney,
    /// Closing plus outside distributions minus opening and native flows; exactly zero.
    pub residual: ExactMoney,
    pub debt: Option<DebtCashFlows>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntityFundingState {
    pub state_id: String,
    pub checkpoints: Vec<EntityCashCheckpoint>,
    pub transfers: Vec<EntityTransferResult>,
    pub distributions: Vec<EntityDistributionResult>,
    pub conservation: Vec<EntityConservation>,
    pub minimum_slack: ExactMoney,
    pub debt: Vec<DebtServiceResult>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntityFundingAssessment {
    pub states: Vec<EntityFundingState>,
    pub minimum_slack: ExactMoney,
}

fn invalid(why: impl Into<String>) -> Refusal {
    bad(RefusalCode::BasisMismatch105, why)
}

fn internal(why: impl Into<String>) -> Refusal {
    bad(RefusalCode::NumericalCertificateFailed207, why)
}

fn bounded(why: impl Into<String>) -> Refusal {
    bad(RefusalCode::NoClaimInDomain201, why)
}

pub(super) fn work_per_state(funding: &EntityFunding) -> Result<usize> {
    let periods = funding.periods.len();
    periods
        .checked_mul(2)
        .and_then(|n| n.checked_add(1))
        .and_then(|n| n.checked_mul(funding.entities.len()))
        .and_then(|n| n.checked_add(funding.transfers.len()))
        .and_then(|n| {
            funding
                .distributions
                .len()
                .checked_mul(periods)
                .and_then(|rows| n.checked_add(rows))
        })
        .and_then(|n| n.checked_add(periods))
        .and_then(|n| {
            funding
                .debt
                .as_ref()
                .map_or(Some(0), |debt| debt.facilities.len().checked_mul(periods))
                .and_then(|rows| n.checked_add(rows))
        })
        .ok_or_else(|| bounded("entity cash result work exceeds the bounded domain"))
}

/// Complete typed matrix sizes and total output work are checked before canonical tagging allocates.
pub(super) fn validate_bounds(
    funding: &EntityFunding,
    state_count: usize,
    candidate_count: usize,
) -> Result<()> {
    let entities = funding.entities.len();
    let periods = funding.periods.len();
    if !(1..=MAX_ENTITIES).contains(&entities)
        || !(1..=MAX_CASH_PERIODS).contains(&periods)
        || !(1..=MAX_STATES).contains(&state_count)
        || !(1..=MAX_CANDIDATES).contains(&candidate_count)
        || funding.transfers.len() > MAX_TRANSFERS
        || funding.distributions.len() >= entities
    {
        return Err(bounded(
            "entity funding requires 1..=4 entities, 1..=16 periods, at most 64 fixed transfers and an acyclic one-parent ownership graph",
        ));
    }
    if let Some(debt) = &funding.debt {
        debt::validate_bounds(debt, entities, periods, state_count, candidate_count)?;
    }
    let work = (1_usize << candidate_count)
        .checked_mul(state_count)
        .and_then(|n| {
            work_per_state(funding)
                .ok()
                .and_then(|rows| n.checked_mul(rows))
        });
    if work.is_none_or(|n| n > MAX_CASH_CHECKPOINTS) {
        return Err(bounded(
            "entity funding may produce at most 65536 checkpoint, transfer, distribution, debt-service and conservation rows across all declared subsets",
        ));
    }
    let entity_periods = entities * periods;
    if funding.floors.len() != entity_periods
        || funding.opening_cash.len() != entities * state_count
        || funding.baseline_flows.len() != entity_periods * state_count
        || funding.candidate_flows.len() != entity_periods * candidate_count * state_count
        || funding.gates.len() != funding.distributions.len() * periods * state_count
        || funding
            .transfers
            .iter()
            .any(|transfer| transfer.amounts.len() != state_count)
    {
        return Err(bounded(
            "entity funding requires complete entity/period floors, entity/state openings, entity/period/state baseline and entity/period/candidate/state cash flows, transfer/state amounts and distributor/period/state gates, including zeroes",
        ));
    }
    Ok(())
}

pub(super) fn validate(
    claim: &Claim<EntityFunding>,
    policy: &ProgramPolicy,
    states: &BTreeSet<&str>,
    candidates: &BTreeMap<&str, usize>,
) -> Result<()> {
    let funding = &claim.value;
    validate_bounds(funding, states.len(), candidates.len())?;
    if funding.model_key != policy.model_key
        || funding.currency != policy.currency
        || !named(&funding.cash_basis)
        || !key(&funding.objective_entity)
    {
        return Err(invalid(
            "entity funding requires the policy model/currency, a named objective entity and an explicit cash basis",
        ));
    }
    let mut entities = BTreeSet::new();
    for entity in &funding.entities {
        if !key(&entity.id) || !named(&entity.label) || !entities.insert(entity.id.as_str()) {
            return Err(invalid(
                "entity funding requires unique bounded entity identifiers and explicit labels",
            ));
        }
    }
    if !entities.contains(funding.objective_entity.as_str()) {
        return Err(invalid(
            "the objective entity must be a declared cash entity",
        ));
    }
    let periods = validate_periods(funding)?;
    let nonnegative = |value: Money| value.ccy == policy.currency && value.minor >= 0;
    let mut floors = BTreeSet::new();
    for row in &funding.floors {
        if !entities.contains(row.entity_id.as_str())
            || !periods.contains(row.period_id.as_str())
            || !floors.insert((row.entity_id.as_str(), row.period_id.as_str()))
            || !nonnegative(row.minimum_cash)
        {
            return Err(invalid(
                "each known entity/period requires exactly one nonnegative cash floor in the policy currency",
            ));
        }
    }
    let mut opening = BTreeSet::new();
    for row in &funding.opening_cash {
        if !entities.contains(row.entity_id.as_str())
            || !states.contains(row.state_id.as_str())
            || !opening.insert((row.entity_id.as_str(), row.state_id.as_str()))
            || !nonnegative(row.value)
        {
            return Err(invalid(
                "each known entity/state requires exactly one nonnegative opening balance in the policy currency",
            ));
        }
    }
    let mut baseline = BTreeSet::new();
    for row in &funding.baseline_flows {
        if !entities.contains(row.entity_id.as_str())
            || !periods.contains(row.period_id.as_str())
            || !states.contains(row.state_id.as_str())
            || !baseline.insert((
                row.entity_id.as_str(),
                row.period_id.as_str(),
                row.state_id.as_str(),
            ))
            || row.value.ccy != policy.currency
        {
            return Err(invalid(
                "baseline native flows require each known entity/period/state exactly once in the policy currency",
            ));
        }
    }
    let mut flows = BTreeSet::new();
    for row in &funding.candidate_flows {
        if !entities.contains(row.entity_id.as_str())
            || !periods.contains(row.period_id.as_str())
            || !candidates.contains_key(row.candidate_id.as_str())
            || !states.contains(row.state_id.as_str())
            || !flows.insert((
                row.entity_id.as_str(),
                row.period_id.as_str(),
                row.candidate_id.as_str(),
                row.state_id.as_str(),
            ))
            || row.value.ccy != policy.currency
        {
            return Err(invalid(
                "candidate native flows require each known entity/period/candidate/state exactly once in the policy currency",
            ));
        }
    }
    let mut transfers = BTreeSet::new();
    for transfer in &funding.transfers {
        if !key(&transfer.id)
            || !transfers.insert(transfer.id.as_str())
            || !entities.contains(transfer.from_entity.as_str())
            || !entities.contains(transfer.to_entity.as_str())
            || transfer.from_entity == transfer.to_entity
            || !periods.contains(transfer.period_id.as_str())
            || transfer
                .candidate_id
                .as_deref()
                .is_some_and(|candidate| !candidates.contains_key(candidate))
            || !named(&transfer.basis)
        {
            return Err(invalid(
                "fixed transfers require unique ids, distinct known endpoints, a known period, any known activating candidate and an explicit basis",
            ));
        }
        let mut present = BTreeSet::new();
        for (state, value) in &transfer.amounts {
            if !states.contains(state.as_str())
                || !present.insert(state.as_str())
                || !nonnegative(*value)
            {
                return Err(invalid(
                    "every fixed transfer requires each shared state exactly once with a nonnegative amount in the policy currency",
                ));
            }
        }
    }
    let mut parents = BTreeMap::new();
    for distribution in &funding.distributions {
        if !entities.contains(distribution.entity_id.as_str())
            || !entities.contains(distribution.parent_entity_id.as_str())
            || distribution.entity_id == distribution.parent_entity_id
            || parents
                .insert(
                    distribution.entity_id.as_str(),
                    distribution.parent_entity_id.as_str(),
                )
                .is_some()
            || distribution.parent_share.signum() < 0
            || exact_compare(distribution.parent_share, ONE).is_gt()
            || !named(&distribution.basis)
        {
            return Err(invalid(
                "each distributing entity requires one distinct known parent, a parent share in [0,1] and an explicit distribution basis",
            ));
        }
    }
    for entity in &entities {
        let mut seen = BTreeSet::new();
        let mut current = *entity;
        while let Some(parent) = parents.get(current) {
            if !seen.insert(current) {
                return Err(invalid("the ownership distribution graph must be acyclic"));
            }
            current = parent;
        }
    }
    let mut gates = BTreeSet::new();
    for gate in &funding.gates {
        if !parents.contains_key(gate.entity_id.as_str())
            || !periods.contains(gate.period_id.as_str())
            || !states.contains(gate.state_id.as_str())
            || !gates.insert((
                gate.entity_id.as_str(),
                gate.period_id.as_str(),
                gate.state_id.as_str(),
            ))
            || !nonnegative(gate.maximum_distribution)
        {
            return Err(invalid(
                "every distributing entity/period/state requires exactly one explicit permission and nonnegative distribution cap",
            ));
        }
    }
    if let Some(debt) = &funding.debt {
        debt::validate(
            debt,
            policy.currency,
            &entities,
            &periods,
            states,
            candidates,
        )?;
    }
    admit(
        claim,
        &funding.canonical_tag(),
        AdditivityClass::NonAdditive,
    )
}

fn validate_periods(funding: &EntityFunding) -> Result<BTreeSet<&str>> {
    let date_error = |why| bad(RefusalCode::UndeclaredUnitBasisOrTz107, why);
    if !fmine_registry::sources::is_date(&funding.opening_date) {
        return Err(date_error(
            "entity funding opening_date must be a valid YYYY-MM-DD date",
        ));
    }
    let mut ids = BTreeSet::new();
    let mut ordered = Vec::with_capacity(funding.periods.len());
    for period in &funding.periods {
        if !key(&period.id) || !ids.insert(period.id.as_str()) {
            return Err(invalid(
                "entity periods require distinct bounded identifiers",
            ));
        }
        let dates = period_dates(&period.cash_period).ok_or_else(|| {
            date_error("entity periods require ordered inclusive YYYY-MM-DD/YYYY-MM-DD dates")
        })?;
        ordered.push(dates);
    }
    ordered.sort_unstable();
    let mut expected_start = funding.opening_date.clone();
    for (index, (start, end)) in ordered.iter().enumerate() {
        if *start != expected_start {
            return Err(date_error(
                "entity periods must be contiguous from opening_date without gaps or overlaps",
            ));
        }
        if index + 1 < ordered.len() {
            expected_start = next_date(end).ok_or_else(|| {
                date_error("entity period end has no representable next calendar date")
            })?;
        }
    }
    Ok(ids)
}

pub(super) fn canonicalize(funding: &mut EntityFunding) {
    funding.entities.sort_by(|a, b| a.id.cmp(&b.id));
    funding
        .periods
        .sort_by(|a, b| (&a.cash_period, &a.id).cmp(&(&b.cash_period, &b.id)));
    let periods: BTreeMap<_, _> = funding
        .periods
        .iter()
        .enumerate()
        .map(|(index, period)| (period.id.as_str(), index))
        .collect();
    funding.floors.sort_by(|a, b| {
        (periods.get(a.period_id.as_str()), &a.entity_id)
            .cmp(&(periods.get(b.period_id.as_str()), &b.entity_id))
    });
    funding
        .opening_cash
        .sort_by(|a, b| (&a.entity_id, &a.state_id).cmp(&(&b.entity_id, &b.state_id)));
    funding.baseline_flows.sort_by(|a, b| {
        (periods.get(a.period_id.as_str()), &a.entity_id, &a.state_id).cmp(&(
            periods.get(b.period_id.as_str()),
            &b.entity_id,
            &b.state_id,
        ))
    });
    funding.candidate_flows.sort_by(|a, b| {
        (
            periods.get(a.period_id.as_str()),
            &a.entity_id,
            &a.candidate_id,
            &a.state_id,
        )
            .cmp(&(
                periods.get(b.period_id.as_str()),
                &b.entity_id,
                &b.candidate_id,
                &b.state_id,
            ))
    });
    funding.transfers.sort_by(|a, b| {
        (periods.get(a.period_id.as_str()), &a.id).cmp(&(periods.get(b.period_id.as_str()), &b.id))
    });
    for transfer in &mut funding.transfers {
        transfer.amounts.sort_by(|a, b| a.0.cmp(&b.0));
    }
    funding.distributions.sort_by(|a, b| {
        (&a.entity_id, &a.parent_entity_id).cmp(&(&b.entity_id, &b.parent_entity_id))
    });
    funding.gates.sort_by(|a, b| {
        (periods.get(a.period_id.as_str()), &a.entity_id, &a.state_id).cmp(&(
            periods.get(b.period_id.as_str()),
            &b.entity_id,
            &b.state_id,
        ))
    });
    if let Some(debt) = &mut funding.debt {
        debt::canonicalize(debt, &periods);
    }
}

struct IndexedTransfer<'a> {
    declaration: &'a EntityTransfer,
    from: usize,
    to: usize,
    period: usize,
    candidate_bit: Option<usize>,
    amounts: Vec<Rat>,
}

#[derive(Clone, Copy)]
struct IndexedGate {
    allowed: bool,
    maximum: Rat,
}

struct IndexedDistribution<'a> {
    declaration: &'a EntityDistribution,
    entity: usize,
    parent: usize,
    external_share: Rat,
    gates: Vec<IndexedGate>,
}

pub(super) struct Index<'a> {
    declaration: &'a EntityFunding,
    entities: Vec<&'a Entity>,
    periods: Vec<&'a EntityPeriod>,
    closing_dates: Vec<&'a str>,
    states: Vec<String>,
    candidate_count: usize,
    opening: Vec<Rat>,
    floors: Vec<Rat>,
    baseline: Vec<Rat>,
    candidate: Vec<Rat>,
    transfers: Vec<IndexedTransfer<'a>>,
    /// Topological child-first order, with entity identifier breaking independent ties.
    distributions: Vec<IndexedDistribution<'a>>,
    /// Includes roots and non-distributors so no debtor bypasses its sweep.
    waterfall_order: Vec<usize>,
    debt: Option<debt::Index<'a>>,
}

fn index_of(index: &BTreeMap<&str, usize>, key: &str) -> Result<usize> {
    index
        .get(key)
        .copied()
        .ok_or_else(|| internal("admitted entity funding lost a known matrix identifier"))
}

fn add(a: Rat, b: Rat) -> Result<Rat> {
    arithmetic(a.checked_add(b))
}

fn subtract(a: Rat, b: Rat) -> Result<Rat> {
    arithmetic(a.checked_sub(b))
}

fn total(values: &[Rat]) -> Result<Rat> {
    values.iter().try_fold(ZERO, |sum, value| add(sum, *value))
}

fn minimum(current: &mut Option<Rat>, value: Rat) {
    if current.is_none_or(|old| exact_compare(value, old).is_lt()) {
        *current = Some(value);
    }
}

struct Components {
    opening: Rat,
    baseline: Rat,
    candidate: Rat,
    transfer_in: Rat,
    transfer_out: Rat,
    distribution_in: Rat,
    distribution_out: Rat,
    closing: Rat,
    debt: Option<DebtCashFlows>,
}

impl<'a> Index<'a> {
    pub(super) fn new(
        funding: &'a EntityFunding,
        candidates: &[Claim<CapitalCandidate>],
        weights: &WeightSet,
    ) -> Result<Self> {
        validate_bounds(funding, weights.states.len(), candidates.len())?;
        let mut entities: Vec<_> = funding.entities.iter().collect();
        entities.sort_by(|a, b| a.id.cmp(&b.id));
        let mut periods: Vec<_> = funding.periods.iter().collect();
        periods.sort_by(|a, b| (&a.cash_period, &a.id).cmp(&(&b.cash_period, &b.id)));
        let closing_dates: Vec<_> = periods
            .iter()
            .map(|period| {
                period_dates(&period.cash_period)
                    .map(|(_, end)| end)
                    .ok_or_else(|| internal("admitted entity period lost its closing date"))
            })
            .collect::<Result<_>>()?;
        let mut states: Vec<_> = weights.states.iter().map(|(id, _)| id.clone()).collect();
        states.sort();
        let entity_index: BTreeMap<_, _> = entities
            .iter()
            .enumerate()
            .map(|(index, entity)| (entity.id.as_str(), index))
            .collect();
        let period_index: BTreeMap<_, _> = periods
            .iter()
            .enumerate()
            .map(|(index, period)| (period.id.as_str(), index))
            .collect();
        let state_index: BTreeMap<_, _> = states
            .iter()
            .enumerate()
            .map(|(index, state)| (state.as_str(), index))
            .collect();
        let candidate_index: BTreeMap<_, _> = candidates
            .iter()
            .enumerate()
            .map(|(index, candidate)| (candidate.value.candidate_id.as_str(), index))
            .collect();
        let entity_count = entities.len();
        let state_count = states.len();
        let candidate_count = candidates.len();
        let mut opening = vec![ZERO; entity_count * state_count];
        let mut floors = vec![ZERO; periods.len() * entity_count];
        let mut baseline = vec![ZERO; periods.len() * entity_count * state_count];
        let mut candidate =
            vec![ZERO; periods.len() * entity_count * candidate_count * state_count];
        for row in &funding.opening_cash {
            let entity = index_of(&entity_index, &row.entity_id)?;
            let state = index_of(&state_index, &row.state_id)?;
            opening[entity * state_count + state] = amount(row.value)?;
        }
        for row in &funding.floors {
            let entity = index_of(&entity_index, &row.entity_id)?;
            let period = index_of(&period_index, &row.period_id)?;
            floors[period * entity_count + entity] = amount(row.minimum_cash)?;
        }
        for row in &funding.baseline_flows {
            let entity = index_of(&entity_index, &row.entity_id)?;
            let period = index_of(&period_index, &row.period_id)?;
            let state = index_of(&state_index, &row.state_id)?;
            baseline[(period * entity_count + entity) * state_count + state] = amount(row.value)?;
        }
        for row in &funding.candidate_flows {
            let entity = index_of(&entity_index, &row.entity_id)?;
            let period = index_of(&period_index, &row.period_id)?;
            let selected = index_of(&candidate_index, &row.candidate_id)?;
            let state = index_of(&state_index, &row.state_id)?;
            candidate[((period * entity_count + entity) * candidate_count + selected)
                * state_count
                + state] = amount(row.value)?;
        }
        let mut transfer_rows: Vec<_> = funding.transfers.iter().collect();
        transfer_rows.sort_by(|a, b| {
            (period_index.get(a.period_id.as_str()), &a.id)
                .cmp(&(period_index.get(b.period_id.as_str()), &b.id))
        });
        let mut transfers = Vec::with_capacity(transfer_rows.len());
        for transfer in transfer_rows {
            let mut amounts = vec![ZERO; state_count];
            for (state, value) in &transfer.amounts {
                amounts[index_of(&state_index, state)?] = amount(*value)?;
            }
            transfers.push(IndexedTransfer {
                declaration: transfer,
                from: index_of(&entity_index, &transfer.from_entity)?,
                to: index_of(&entity_index, &transfer.to_entity)?,
                period: index_of(&period_index, &transfer.period_id)?,
                candidate_bit: transfer
                    .candidate_id
                    .as_deref()
                    .map(|id| index_of(&candidate_index, id).map(|index| 1_usize << index))
                    .transpose()?,
                amounts,
            });
        }
        let distributions =
            Self::index_distributions(funding, &entity_index, &period_index, &state_index)?;
        let waterfall_order = Self::waterfall_order(funding, &entity_index)?;
        let debt = funding
            .debt
            .as_ref()
            .map(|debt| {
                debt::Index::new(
                    debt,
                    funding.currency,
                    &entity_index,
                    &period_index,
                    &state_index,
                    &candidate_index,
                )
            })
            .transpose()?;
        Ok(Self {
            declaration: funding,
            entities,
            periods,
            closing_dates,
            states,
            candidate_count,
            opening,
            floors,
            baseline,
            candidate,
            transfers,
            distributions,
            waterfall_order,
            debt,
        })
    }

    fn waterfall_order(
        funding: &EntityFunding,
        entities: &BTreeMap<&str, usize>,
    ) -> Result<Vec<usize>> {
        let parents: BTreeMap<_, _> = funding
            .distributions
            .iter()
            .map(|row| (row.entity_id.as_str(), row.parent_entity_id.as_str()))
            .collect();
        let mut ordered = Vec::with_capacity(entities.len());
        for (entity, index) in entities {
            let mut depth = 0;
            let mut current = *entity;
            while let Some(parent) = parents.get(current) {
                depth += 1;
                if depth >= entities.len() {
                    return Err(internal("admitted ownership graph contains a cycle"));
                }
                current = parent;
            }
            ordered.push((*index, depth, *entity));
        }
        ordered.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.2.cmp(b.2)));
        Ok(ordered.into_iter().map(|(index, _, _)| index).collect())
    }

    fn index_distributions(
        funding: &'a EntityFunding,
        entities: &BTreeMap<&str, usize>,
        periods: &BTreeMap<&str, usize>,
        states: &BTreeMap<&str, usize>,
    ) -> Result<Vec<IndexedDistribution<'a>>> {
        let parents: BTreeMap<_, _> = funding
            .distributions
            .iter()
            .map(|row| (row.entity_id.as_str(), row.parent_entity_id.as_str()))
            .collect();
        let mut ordered = Vec::with_capacity(funding.distributions.len());
        for rule in &funding.distributions {
            let mut depth = 0;
            let mut current = rule.entity_id.as_str();
            while let Some(parent) = parents.get(current) {
                depth += 1;
                if depth >= entities.len() {
                    return Err(internal("admitted ownership graph contains a cycle"));
                }
                current = parent;
            }
            ordered.push((rule, depth));
        }
        ordered.sort_by(|(a, a_depth), (b, b_depth)| {
            b_depth
                .cmp(a_depth)
                .then_with(|| a.entity_id.cmp(&b.entity_id))
        });
        let mut distributions = Vec::with_capacity(ordered.len());
        let gates: BTreeMap<_, _> = funding
            .gates
            .iter()
            .map(|row| {
                (
                    (
                        row.entity_id.as_str(),
                        row.period_id.as_str(),
                        row.state_id.as_str(),
                    ),
                    row,
                )
            })
            .collect();
        let mut ordered_periods: Vec<_> = periods.iter().collect();
        ordered_periods.sort_by_key(|(_, index)| **index);
        let mut ordered_states: Vec<_> = states.iter().collect();
        ordered_states.sort_by_key(|(_, index)| **index);
        for (rule, _) in ordered {
            let mut indexed_gates = Vec::with_capacity(periods.len() * states.len());
            for (period, _) in &ordered_periods {
                for (state, _) in &ordered_states {
                    let gate = gates
                        .get(&(rule.entity_id.as_str(), **period, **state))
                        .ok_or_else(|| {
                            internal("admitted ownership graph lost a distribution gate")
                        })?;
                    indexed_gates.push(IndexedGate {
                        allowed: gate.allowed,
                        maximum: amount(gate.maximum_distribution)?,
                    });
                }
            }
            distributions.push(IndexedDistribution {
                declaration: rule,
                entity: index_of(entities, &rule.entity_id)?,
                parent: index_of(entities, &rule.parent_entity_id)?,
                external_share: subtract(ONE, rule.parent_share)?,
                gates: indexed_gates,
            });
        }
        Ok(distributions)
    }

    fn money(&self, major: Rat) -> ExactMoney {
        ExactMoney {
            currency: self.declaration.currency,
            major,
        }
    }

    fn checkpoint(
        &self,
        entity: usize,
        period: Option<usize>,
        phase: EntityCashPhase,
        values: Components,
    ) -> Result<EntityCashCheckpoint> {
        let floor = self.floors[period.unwrap_or(0) * self.entities.len() + entity];
        Ok(EntityCashCheckpoint {
            entity_id: self.entities[entity].id.clone(),
            period_id: period.map(|index| self.periods[index].id.clone()),
            checkpoint_date: period
                .map(|index| self.closing_dates[index])
                .unwrap_or(self.declaration.opening_date.as_str())
                .into(),
            phase,
            opening_cash: self.money(values.opening),
            baseline_cash_flow: self.money(values.baseline),
            candidate_cash_flow: self.money(values.candidate),
            transfer_in: self.money(values.transfer_in),
            transfer_out: self.money(values.transfer_out),
            distribution_in: self.money(values.distribution_in),
            distribution_out: self.money(values.distribution_out),
            closing_cash: self.money(values.closing),
            minimum_cash: self.money(floor),
            slack: self.money(subtract(values.closing, floor)?),
            debt: values.debt,
        })
    }

    pub(super) fn assess(&self, selected_mask: usize) -> Result<EntityFundingAssessment> {
        if selected_mask >= (1_usize << self.candidate_count) {
            return Err(internal(
                "entity assessment received an unknown candidate mask",
            ));
        }
        let mut states = Vec::with_capacity(self.states.len());
        let mut minimum_slack = None;
        for state in 0..self.states.len() {
            let assessment = self.assess_state(selected_mask, state)?;
            minimum(&mut minimum_slack, assessment.minimum_slack.major);
            states.push(assessment);
        }
        Ok(EntityFundingAssessment {
            states,
            minimum_slack: self.money(
                minimum_slack.ok_or_else(|| internal("entity assessment lacks shared states"))?,
            ),
        })
    }

    fn assess_state(&self, selected_mask: usize, state: usize) -> Result<EntityFundingState> {
        let entity_count = self.entities.len();
        let state_count = self.states.len();
        let mut balances: Vec<_> = (0..entity_count)
            .map(|entity| self.opening[entity * state_count + state])
            .collect();
        let mut checkpoints = Vec::with_capacity(entity_count * (2 * self.periods.len() + 1));
        let mut transfers = Vec::with_capacity(self.transfers.len());
        let mut distributions = Vec::with_capacity(self.distributions.len() * self.periods.len());
        let mut conservation = Vec::with_capacity(self.periods.len());
        let mut principal = self
            .debt
            .as_ref()
            .map(|debt| debt.opening(state))
            .transpose()?
            .unwrap_or_default();
        let mut debt_results = Vec::with_capacity(
            self.declaration
                .debt
                .as_ref()
                .map_or(0, |debt| debt.facilities.len() * self.periods.len()),
        );
        let mut minimum_slack = None;
        for (entity, opening) in balances.iter().copied().enumerate() {
            let row = self.checkpoint(
                entity,
                None,
                EntityCashPhase::Opening,
                Components {
                    opening,
                    baseline: ZERO,
                    candidate: ZERO,
                    transfer_in: ZERO,
                    transfer_out: ZERO,
                    distribution_in: ZERO,
                    distribution_out: ZERO,
                    closing: opening,
                    debt: None,
                },
            )?;
            minimum(&mut minimum_slack, row.slack.major);
            checkpoints.push(row);
        }
        for period in 0..self.periods.len() {
            let opening = balances.clone();
            let mut baseline = vec![ZERO; entity_count];
            let mut candidate = vec![ZERO; entity_count];
            let mut transfer_in = vec![ZERO; entity_count];
            let mut transfer_out = vec![ZERO; entity_count];
            for entity in 0..entity_count {
                baseline[entity] =
                    self.baseline[(period * entity_count + entity) * state_count + state];
                for selected in 0..self.candidate_count {
                    if selected_mask & (1 << selected) != 0 {
                        let value = self.candidate[((period * entity_count + entity)
                            * self.candidate_count
                            + selected)
                            * state_count
                            + state];
                        candidate[entity] = add(candidate[entity], value)?;
                    }
                }
            }
            let mut total_transfers = ZERO;
            for transfer in self.transfers.iter().filter(|row| row.period == period) {
                let active = transfer
                    .candidate_bit
                    .is_none_or(|bit| selected_mask & bit != 0);
                let declared = transfer.amounts[state];
                let actual = if active { declared } else { ZERO };
                transfer_in[transfer.to] = add(transfer_in[transfer.to], actual)?;
                transfer_out[transfer.from] = add(transfer_out[transfer.from], actual)?;
                total_transfers = add(total_transfers, actual)?;
                let row = transfer.declaration;
                transfers.push(EntityTransferResult {
                    transfer_id: row.id.clone(),
                    from_entity: row.from_entity.clone(),
                    to_entity: row.to_entity.clone(),
                    period_id: row.period_id.clone(),
                    checkpoint_date: self.closing_dates[period].into(),
                    candidate_id: row.candidate_id.clone(),
                    basis: row.basis.clone(),
                    active,
                    declared_amount: self.money(declared),
                    actual_amount: self.money(actual),
                });
            }
            // Native flows and fixed transfers settle before external debt draws and mandatory service.
            for entity in 0..entity_count {
                balances[entity] = subtract(
                    add(
                        add(add(opening[entity], baseline[entity])?, candidate[entity])?,
                        transfer_in[entity],
                    )?,
                    transfer_out[entity],
                )?;
            }
            let mut services = if let Some(debt) = &self.debt {
                debt.service(
                    &debt::Period {
                        index: period,
                        state,
                        selected_mask,
                        id: &self.periods[period].id,
                        closing_date: self.closing_dates[period],
                    },
                    &mut balances,
                    &mut principal,
                    &self.floors[period * entity_count..(period + 1) * entity_count],
                )?
            } else {
                Vec::new()
            };
            // Every mandatory-service checkpoint remains binding even if later child receipts repair it.
            for entity in 0..entity_count {
                let row = self.checkpoint(
                    entity,
                    Some(period),
                    EntityCashPhase::PreDistribution,
                    Components {
                        opening: opening[entity],
                        baseline: baseline[entity],
                        candidate: candidate[entity],
                        transfer_in: transfer_in[entity],
                        transfer_out: transfer_out[entity],
                        distribution_in: ZERO,
                        distribution_out: ZERO,
                        closing: balances[entity],
                        debt: self
                            .debt
                            .as_ref()
                            .map(|debt| debt.cash_flows_for(entity, &services))
                            .transpose()?
                            .flatten(),
                    },
                )?;
                minimum(&mut minimum_slack, row.slack.major);
                checkpoints.push(row);
            }
            let mut distribution_in = vec![ZERO; entity_count];
            let mut distribution_out = vec![ZERO; entity_count];
            let mut external_distributions = ZERO;
            let mut internal_distributions = ZERO;
            for child in self.waterfall_order.iter().copied() {
                if let Some(debt) = &self.debt {
                    debt.sweep_entity(child, &mut balances[child], &mut principal, &mut services)?;
                }
                let Some(distribution) = self.distributions.iter().find(|row| row.entity == child)
                else {
                    continue;
                };
                let parent = distribution.parent;
                let rule = distribution.declaration;
                let gate = distribution.gates[period * state_count + state];
                let before = balances[child];
                let floor = self.floors[period * entity_count + child];
                let slack = subtract(before, floor)?;
                let surplus = if slack.signum() > 0 { slack } else { ZERO };
                let debt_gate = self
                    .debt
                    .as_ref()
                    .map(|debt| debt.gate_for(child, &services, gate.allowed))
                    .transpose()?
                    .flatten();
                let gross = if gate.allowed
                    && debt_gate
                        .as_ref()
                        .is_none_or(|gate| gate.distribution_allowed)
                {
                    if exact_compare(surplus, gate.maximum).is_lt() {
                        surplus
                    } else {
                        gate.maximum
                    }
                } else {
                    ZERO
                };
                let receipt = arithmetic(gross.checked_mul(rule.parent_share))?;
                let outside = subtract(gross, receipt)?;
                balances[child] = subtract(before, gross)?;
                balances[parent] = add(balances[parent], receipt)?;
                distribution_out[child] = add(distribution_out[child], gross)?;
                distribution_in[parent] = add(distribution_in[parent], receipt)?;
                external_distributions = add(external_distributions, outside)?;
                internal_distributions = add(internal_distributions, receipt)?;
                distributions.push(EntityDistributionResult {
                    entity_id: rule.entity_id.clone(),
                    parent_entity_id: rule.parent_entity_id.clone(),
                    period_id: self.periods[period].id.clone(),
                    checkpoint_date: self.closing_dates[period].into(),
                    basis: rule.basis.clone(),
                    parent_share: rule.parent_share,
                    external_share: distribution.external_share,
                    allowed: gate.allowed,
                    maximum_distribution: self.money(gate.maximum),
                    cash_before_distribution: self.money(before),
                    minimum_cash: self.money(floor),
                    available_surplus: self.money(surplus),
                    gross_distribution: self.money(gross),
                    parent_receipt: self.money(receipt),
                    external_distribution: self.money(outside),
                    cash_after_distribution: self.money(balances[child]),
                    debt_gate,
                });
            }
            for entity in 0..entity_count {
                let row = self.checkpoint(
                    entity,
                    Some(period),
                    EntityCashPhase::Closing,
                    Components {
                        opening: opening[entity],
                        baseline: baseline[entity],
                        candidate: candidate[entity],
                        transfer_in: transfer_in[entity],
                        transfer_out: transfer_out[entity],
                        distribution_in: distribution_in[entity],
                        distribution_out: distribution_out[entity],
                        closing: balances[entity],
                        debt: self
                            .debt
                            .as_ref()
                            .map(|debt| debt.cash_flows_for(entity, &services))
                            .transpose()?
                            .flatten(),
                    },
                )?;
                minimum(&mut minimum_slack, row.slack.major);
                checkpoints.push(row);
            }
            let opening_total = total(&opening)?;
            let baseline_total = total(&baseline)?;
            let candidate_total = total(&candidate)?;
            let closing_total = total(&balances)?;
            let debt_flows = self
                .debt
                .as_ref()
                .map(|debt| debt.totals(&services))
                .transpose()?;
            let mut residual = subtract(
                subtract(
                    subtract(add(closing_total, external_distributions)?, opening_total)?,
                    baseline_total,
                )?,
                candidate_total,
            )?;
            if let Some(debt) = &debt_flows {
                residual = subtract(
                    add(
                        add(
                            add(residual, debt.interest.major)?,
                            debt.scheduled_principal.major,
                        )?,
                        debt.sweep.major,
                    )?,
                    debt.draw.major,
                )?;
            }
            if residual != ZERO {
                return Err(internal(
                    "entity cash did not conserve exactly across transfers and distributions",
                ));
            }
            conservation.push(EntityConservation {
                period_id: self.periods[period].id.clone(),
                checkpoint_date: self.closing_dates[period].into(),
                opening_cash: self.money(opening_total),
                baseline_cash_flow: self.money(baseline_total),
                candidate_cash_flow: self.money(candidate_total),
                internal_transfers: self.money(total_transfers),
                internal_distributions: self.money(internal_distributions),
                external_distributions: self.money(external_distributions),
                closing_cash: self.money(closing_total),
                residual: self.money(residual),
                debt: debt_flows,
            });
            debt_results.extend(services);
        }
        Ok(EntityFundingState {
            state_id: self.states[state].clone(),
            checkpoints,
            transfers,
            distributions,
            conservation,
            minimum_slack: self.money(
                minimum_slack.ok_or_else(|| internal("entity state lacks cash checkpoints"))?,
            ),
            debt: debt_results,
        })
    }
}
