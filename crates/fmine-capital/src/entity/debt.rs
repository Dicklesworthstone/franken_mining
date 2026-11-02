//! DESIGN (BD-314): explicitly declared senior debt service, coverage locks and principal sweeps.
//! Determinism: Exact (full-period rational rates, shared-state cash and exact principal carry).

use super::*;

pub(super) mod render;
#[cfg(test)]
mod tests;

pub const CASH_FLOW_CONVENTION: &str = "native_excludes_modeled_debt";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DebtFacility {
    pub id: String,
    pub entity_id: String,
    pub basis: String,
    pub cfads_basis: String,
    pub minimum_dscr: Rat,
    pub sweep_fraction: Rat,
    pub allow_distributions_without_service: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DebtOpeningPrincipal {
    pub facility_id: String,
    pub state_id: String,
    pub value: Money,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DebtBaselineFlow {
    pub facility_id: String,
    pub period_id: String,
    pub state_id: String,
    /// A full declared-period rate applied to opening principal plus the current draw.
    pub interest_rate: Rat,
    pub draw: Money,
    pub principal_payment_cap: Money,
    pub cfads: Money,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DebtCandidateFlow {
    pub facility_id: String,
    pub period_id: String,
    pub candidate_id: String,
    pub state_id: String,
    pub draw: Money,
    pub principal_payment_cap: Money,
    pub cfads: Money,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntityDebtFunding {
    pub cash_flow_convention: String,
    pub facilities: Vec<DebtFacility>,
    pub opening_principal: Vec<DebtOpeningPrincipal>,
    pub baseline_flows: Vec<DebtBaselineFlow>,
    pub candidate_flows: Vec<DebtCandidateFlow>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DebtCoverageStatus {
    Covered,
    BelowMinimum,
    NoServiceDue,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DebtCashFlows {
    pub draw: ExactMoney,
    pub interest: ExactMoney,
    pub scheduled_principal: ExactMoney,
    pub sweep: ExactMoney,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DebtDistributionGate {
    pub facility_id: String,
    pub dscr: Option<Rat>,
    pub minimum_dscr: Rat,
    pub coverage_status: DebtCoverageStatus,
    pub distribution_allowed: bool,
    pub effective_allowed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DebtServiceResult {
    pub facility_id: String,
    pub entity_id: String,
    pub period_id: String,
    pub checkpoint_date: String,
    pub opening_principal: ExactMoney,
    pub baseline_draw: ExactMoney,
    pub candidate_draw: ExactMoney,
    pub draw: ExactMoney,
    pub principal_before_service: ExactMoney,
    pub interest_rate: Rat,
    pub interest: ExactMoney,
    pub baseline_principal_payment_cap: ExactMoney,
    pub candidate_principal_payment_cap: ExactMoney,
    pub principal_payment_cap: ExactMoney,
    pub scheduled_principal: ExactMoney,
    pub scheduled_service: ExactMoney,
    pub baseline_cfads: ExactMoney,
    pub candidate_cfads: ExactMoney,
    pub cfads: ExactMoney,
    pub dscr: Option<Rat>,
    pub minimum_dscr: Rat,
    pub coverage_status: DebtCoverageStatus,
    pub distribution_allowed: bool,
    pub allow_distributions_without_service: bool,
    pub cash_before_service: ExactMoney,
    pub cash_after_service: ExactMoney,
    pub cash_before_sweep: ExactMoney,
    pub minimum_cash: ExactMoney,
    pub sweep_fraction: Rat,
    pub available_sweep_cash: ExactMoney,
    pub sweep: ExactMoney,
    pub cash_after_sweep: ExactMoney,
    pub closing_principal: ExactMoney,
    pub principal_residual: ExactMoney,
}

pub(super) fn validate_bounds(
    debt: &EntityDebtFunding,
    entity_count: usize,
    period_count: usize,
    state_count: usize,
    candidate_count: usize,
) -> Result<()> {
    let facilities = debt.facilities.len();
    if !(1..=entity_count).contains(&facilities)
        || entity_count > MAX_ENTITIES
        || !(1..=MAX_CASH_PERIODS).contains(&period_count)
        || !(1..=MAX_STATES).contains(&state_count)
        || !(1..=MAX_CANDIDATES).contains(&candidate_count)
        || debt.opening_principal.len() != facilities * state_count
        || debt.baseline_flows.len() != facilities * period_count * state_count
        || debt.candidate_flows.len() != facilities * period_count * candidate_count * state_count
    {
        return Err(bounded(
            "entity debt requires one facility per debtor entity and complete facility/state openings, facility/period/state baseline and facility/period/candidate/state increments, including zeroes",
        ));
    }
    Ok(())
}

pub(super) fn validate(
    debt: &EntityDebtFunding,
    currency: Currency,
    entities: &BTreeSet<&str>,
    periods: &BTreeSet<&str>,
    states: &BTreeSet<&str>,
    candidates: &BTreeMap<&str, usize>,
) -> Result<()> {
    validate_bounds(
        debt,
        entities.len(),
        periods.len(),
        states.len(),
        candidates.len(),
    )?;
    if debt.cash_flow_convention != CASH_FLOW_CONVENTION {
        return Err(invalid(
            "entity debt requires native_excludes_modeled_debt: native flows exclude modeled draws, service and sweeps; CFADS is a separate covenant metric",
        ));
    }
    let mut facilities = BTreeSet::new();
    let mut debtors = BTreeSet::new();
    for facility in &debt.facilities {
        if !key(&facility.id)
            || !facilities.insert(facility.id.as_str())
            || !entities.contains(facility.entity_id.as_str())
            || !debtors.insert(facility.entity_id.as_str())
            || !named(&facility.basis)
            || !named(&facility.cfads_basis)
            || facility.minimum_dscr.signum() <= 0
            || facility.sweep_fraction.signum() < 0
            || exact_compare(facility.sweep_fraction, ONE).is_gt()
        {
            return Err(invalid(
                "debt facilities require unique ids and debtor entities, explicit contract/CFADS bases, a positive coverage threshold and a sweep fraction in [0,1]",
            ));
        }
    }
    let nonnegative = |value: Money| value.ccy == currency && value.minor >= 0;
    let mut openings = BTreeSet::new();
    for row in &debt.opening_principal {
        if !facilities.contains(row.facility_id.as_str())
            || !states.contains(row.state_id.as_str())
            || !openings.insert((row.facility_id.as_str(), row.state_id.as_str()))
            || !nonnegative(row.value)
        {
            return Err(invalid(
                "every debt facility/state requires one nonnegative opening principal in the policy currency",
            ));
        }
    }
    let mut baseline = BTreeSet::new();
    for row in &debt.baseline_flows {
        if !facilities.contains(row.facility_id.as_str())
            || !periods.contains(row.period_id.as_str())
            || !states.contains(row.state_id.as_str())
            || !baseline.insert((
                row.facility_id.as_str(),
                row.period_id.as_str(),
                row.state_id.as_str(),
            ))
            || row.interest_rate.signum() < 0
            || !nonnegative(row.draw)
            || !nonnegative(row.principal_payment_cap)
            || row.cfads.ccy != currency
        {
            return Err(invalid(
                "baseline debt requires each known facility/period/state once, a nonnegative full-period rate/draw/principal cap and explicit signed CFADS in the policy currency",
            ));
        }
    }
    let mut increments = BTreeSet::new();
    for row in &debt.candidate_flows {
        if !facilities.contains(row.facility_id.as_str())
            || !periods.contains(row.period_id.as_str())
            || !candidates.contains_key(row.candidate_id.as_str())
            || !states.contains(row.state_id.as_str())
            || !increments.insert((
                row.facility_id.as_str(),
                row.period_id.as_str(),
                row.candidate_id.as_str(),
                row.state_id.as_str(),
            ))
            || !nonnegative(row.draw)
            || !nonnegative(row.principal_payment_cap)
            || row.cfads.ccy != currency
        {
            return Err(invalid(
                "candidate debt requires each known facility/period/candidate/state once, nonnegative draw/principal-cap increments and signed CFADS in the policy currency",
            ));
        }
    }
    Ok(())
}

pub(super) fn canonicalize(debt: &mut EntityDebtFunding, periods: &BTreeMap<&str, usize>) {
    debt.facilities.sort_by(|a, b| a.id.cmp(&b.id));
    debt.opening_principal
        .sort_by(|a, b| (&a.facility_id, &a.state_id).cmp(&(&b.facility_id, &b.state_id)));
    debt.baseline_flows.sort_by(|a, b| {
        (
            periods.get(a.period_id.as_str()),
            &a.facility_id,
            &a.state_id,
        )
            .cmp(&(
                periods.get(b.period_id.as_str()),
                &b.facility_id,
                &b.state_id,
            ))
    });
    debt.candidate_flows.sort_by(|a, b| {
        (
            periods.get(a.period_id.as_str()),
            &a.facility_id,
            &a.candidate_id,
            &a.state_id,
        )
            .cmp(&(
                periods.get(b.period_id.as_str()),
                &b.facility_id,
                &b.candidate_id,
                &b.state_id,
            ))
    });
}

#[derive(Clone, Copy)]
struct Flow {
    interest_rate: Rat,
    draw: Rat,
    principal_cap: Rat,
    cfads: Rat,
}

const EMPTY_FLOW: Flow = Flow {
    interest_rate: ZERO,
    draw: ZERO,
    principal_cap: ZERO,
    cfads: ZERO,
};

struct IndexedFacility<'a> {
    declaration: &'a DebtFacility,
    entity: usize,
}

pub(super) struct Index<'a> {
    currency: Currency,
    facilities: Vec<IndexedFacility<'a>>,
    by_entity: Vec<Option<usize>>,
    period_count: usize,
    state_count: usize,
    candidate_count: usize,
    opening: Vec<Rat>,
    baseline: Vec<Flow>,
    candidate: Vec<Flow>,
}

pub(super) struct Period<'a> {
    pub index: usize,
    pub state: usize,
    pub selected_mask: usize,
    pub id: &'a str,
    pub closing_date: &'a str,
}

impl<'a> Index<'a> {
    pub(super) fn new(
        debt: &'a EntityDebtFunding,
        currency: Currency,
        entities: &BTreeMap<&str, usize>,
        periods: &BTreeMap<&str, usize>,
        states: &BTreeMap<&str, usize>,
        candidates: &BTreeMap<&str, usize>,
    ) -> Result<Self> {
        validate_bounds(
            debt,
            entities.len(),
            periods.len(),
            states.len(),
            candidates.len(),
        )?;
        let mut ordered: Vec<_> = debt.facilities.iter().collect();
        ordered.sort_by(|a, b| a.id.cmp(&b.id));
        let facility_index: BTreeMap<_, _> = ordered
            .iter()
            .enumerate()
            .map(|(index, facility)| (facility.id.as_str(), index))
            .collect();
        let facility_count = ordered.len();
        let state_count = states.len();
        let period_count = periods.len();
        let candidate_count = candidates.len();
        let mut by_entity = vec![None; entities.len()];
        let mut facilities = Vec::with_capacity(facility_count);
        for (index, declaration) in ordered.into_iter().enumerate() {
            let entity = index_of(entities, &declaration.entity_id)?;
            if by_entity[entity].replace(index).is_some() {
                return Err(internal(
                    "admitted debt has multiple facilities for one entity",
                ));
            }
            facilities.push(IndexedFacility {
                declaration,
                entity,
            });
        }
        let mut opening = vec![ZERO; facility_count * state_count];
        for row in &debt.opening_principal {
            let facility = index_of(&facility_index, &row.facility_id)?;
            let state = index_of(states, &row.state_id)?;
            opening[facility * state_count + state] = amount(row.value)?;
        }
        let mut baseline = vec![EMPTY_FLOW; period_count * facility_count * state_count];
        for row in &debt.baseline_flows {
            let facility = index_of(&facility_index, &row.facility_id)?;
            let period = index_of(periods, &row.period_id)?;
            let state = index_of(states, &row.state_id)?;
            baseline[(period * facility_count + facility) * state_count + state] = Flow {
                interest_rate: row.interest_rate,
                draw: amount(row.draw)?,
                principal_cap: amount(row.principal_payment_cap)?,
                cfads: amount(row.cfads)?,
            };
        }
        let mut candidate =
            vec![EMPTY_FLOW; period_count * facility_count * candidate_count * state_count];
        for row in &debt.candidate_flows {
            let facility = index_of(&facility_index, &row.facility_id)?;
            let period = index_of(periods, &row.period_id)?;
            let selected = index_of(candidates, &row.candidate_id)?;
            let state = index_of(states, &row.state_id)?;
            candidate[((period * facility_count + facility) * candidate_count + selected)
                * state_count
                + state] = Flow {
                interest_rate: ZERO,
                draw: amount(row.draw)?,
                principal_cap: amount(row.principal_payment_cap)?,
                cfads: amount(row.cfads)?,
            };
        }
        Ok(Self {
            currency,
            facilities,
            by_entity,
            period_count,
            state_count,
            candidate_count,
            opening,
            baseline,
            candidate,
        })
    }

    fn money(&self, major: Rat) -> ExactMoney {
        ExactMoney {
            currency: self.currency,
            major,
        }
    }

    pub(super) fn opening(&self, state: usize) -> Result<Vec<Rat>> {
        if state >= self.state_count {
            return Err(internal("debt opening requested an unknown state"));
        }
        Ok((0..self.facilities.len())
            .map(|facility| self.opening[facility * self.state_count + state])
            .collect())
    }

    pub(super) fn service(
        &self,
        period: &Period<'_>,
        cash: &mut [Rat],
        principal: &mut [Rat],
        floors: &[Rat],
    ) -> Result<Vec<DebtServiceResult>> {
        if period.index >= self.period_count
            || period.state >= self.state_count
            || period.selected_mask >= (1_usize << self.candidate_count)
            || cash.len() != self.by_entity.len()
            || floors.len() != cash.len()
            || principal.len() != self.facilities.len()
        {
            return Err(internal(
                "debt service lost its admitted period/entity/state dimensions",
            ));
        }
        let mut result = Vec::with_capacity(self.facilities.len());
        for (index, facility) in self.facilities.iter().enumerate() {
            let terms = facility.declaration;
            let baseline = self.baseline
                [(period.index * self.facilities.len() + index) * self.state_count + period.state];
            let mut increment = EMPTY_FLOW;
            for selected in 0..self.candidate_count {
                if period.selected_mask & (1 << selected) != 0 {
                    let row = self.candidate[((period.index * self.facilities.len() + index)
                        * self.candidate_count
                        + selected)
                        * self.state_count
                        + period.state];
                    increment.draw = add(increment.draw, row.draw)?;
                    increment.principal_cap = add(increment.principal_cap, row.principal_cap)?;
                    increment.cfads = add(increment.cfads, row.cfads)?;
                }
            }
            let opening = principal[index];
            let draw = add(baseline.draw, increment.draw)?;
            let before = add(opening, draw)?;
            let interest = arithmetic(before.checked_mul(baseline.interest_rate))?;
            let cap = add(baseline.principal_cap, increment.principal_cap)?;
            let scheduled = if exact_compare(cap, before).is_lt() {
                cap
            } else {
                before
            };
            let service = add(interest, scheduled)?;
            let cfads = add(baseline.cfads, increment.cfads)?;
            let (dscr, coverage_status, distribution_allowed) = if service == ZERO {
                (
                    None,
                    DebtCoverageStatus::NoServiceDue,
                    terms.allow_distributions_without_service,
                )
            } else {
                let ratio = arithmetic(cfads.checked_div(service))?;
                let covered = !exact_compare(ratio, terms.minimum_dscr).is_lt();
                (
                    Some(ratio),
                    if covered {
                        DebtCoverageStatus::Covered
                    } else {
                        DebtCoverageStatus::BelowMinimum
                    },
                    covered,
                )
            };
            let cash_before = cash[facility.entity];
            let cash_after = subtract(add(cash_before, draw)?, service)?;
            cash[facility.entity] = cash_after;
            principal[index] = subtract(before, scheduled)?;
            result.push(DebtServiceResult {
                facility_id: terms.id.clone(),
                entity_id: terms.entity_id.clone(),
                period_id: period.id.into(),
                checkpoint_date: period.closing_date.into(),
                opening_principal: self.money(opening),
                baseline_draw: self.money(baseline.draw),
                candidate_draw: self.money(increment.draw),
                draw: self.money(draw),
                principal_before_service: self.money(before),
                interest_rate: baseline.interest_rate,
                interest: self.money(interest),
                baseline_principal_payment_cap: self.money(baseline.principal_cap),
                candidate_principal_payment_cap: self.money(increment.principal_cap),
                principal_payment_cap: self.money(cap),
                scheduled_principal: self.money(scheduled),
                scheduled_service: self.money(service),
                baseline_cfads: self.money(baseline.cfads),
                candidate_cfads: self.money(increment.cfads),
                cfads: self.money(cfads),
                dscr,
                minimum_dscr: terms.minimum_dscr,
                coverage_status,
                distribution_allowed,
                allow_distributions_without_service: terms.allow_distributions_without_service,
                cash_before_service: self.money(cash_before),
                cash_after_service: self.money(cash_after),
                cash_before_sweep: self.money(cash_after),
                minimum_cash: self.money(floors[facility.entity]),
                sweep_fraction: terms.sweep_fraction,
                available_sweep_cash: self.money(ZERO),
                sweep: self.money(ZERO),
                cash_after_sweep: self.money(cash_after),
                closing_principal: self.money(principal[index]),
                principal_residual: self.money(ZERO),
            });
        }
        Ok(result)
    }

    fn facility_for(&self, entity: usize) -> Result<Option<usize>> {
        self.by_entity
            .get(entity)
            .copied()
            .ok_or_else(|| internal("debt lookup requested an unknown entity"))
    }

    pub(super) fn sweep_entity(
        &self,
        entity: usize,
        cash: &mut Rat,
        principal: &mut [Rat],
        results: &mut [DebtServiceResult],
    ) -> Result<()> {
        let Some(index) = self.facility_for(entity)? else {
            return Ok(());
        };
        let remaining = principal
            .get_mut(index)
            .ok_or_else(|| internal("debt sweep lost its principal balance"))?;
        let row = results
            .get_mut(index)
            .ok_or_else(|| internal("debt sweep lost its mandatory-service result"))?;
        let excess = subtract(*cash, row.minimum_cash.major)?;
        let available = if excess.signum() > 0 { excess } else { ZERO };
        let requested = arithmetic(available.checked_mul(row.sweep_fraction))?;
        let sweep = if exact_compare(requested, *remaining).is_lt() {
            requested
        } else {
            *remaining
        };
        let before = *cash;
        *cash = subtract(before, sweep)?;
        *remaining = subtract(*remaining, sweep)?;
        let residual = subtract(
            subtract(
                add(add(*remaining, row.scheduled_principal.major)?, sweep)?,
                row.opening_principal.major,
            )?,
            row.draw.major,
        )?;
        if residual != ZERO {
            return Err(internal(
                "debt principal did not conserve exactly through scheduled repayment and sweep",
            ));
        }
        row.cash_before_sweep = self.money(before);
        row.available_sweep_cash = self.money(available);
        row.sweep = self.money(sweep);
        row.cash_after_sweep = self.money(*cash);
        row.closing_principal = self.money(*remaining);
        row.principal_residual = self.money(residual);
        Ok(())
    }

    pub(super) fn cash_flows_for(
        &self,
        entity: usize,
        results: &[DebtServiceResult],
    ) -> Result<Option<DebtCashFlows>> {
        let Some(index) = self.facility_for(entity)? else {
            return Ok(None);
        };
        let row = results
            .get(index)
            .ok_or_else(|| internal("debt cash components lost a service result"))?;
        Ok(Some(DebtCashFlows {
            draw: row.draw,
            interest: row.interest,
            scheduled_principal: row.scheduled_principal,
            sweep: row.sweep,
        }))
    }

    pub(super) fn gate_for(
        &self,
        entity: usize,
        results: &[DebtServiceResult],
        declared_allowed: bool,
    ) -> Result<Option<DebtDistributionGate>> {
        let Some(index) = self.facility_for(entity)? else {
            return Ok(None);
        };
        let row = results
            .get(index)
            .ok_or_else(|| internal("debt distribution gate lost a service result"))?;
        Ok(Some(DebtDistributionGate {
            facility_id: row.facility_id.clone(),
            dscr: row.dscr,
            minimum_dscr: row.minimum_dscr,
            coverage_status: row.coverage_status,
            distribution_allowed: row.distribution_allowed,
            effective_allowed: declared_allowed && row.distribution_allowed,
        }))
    }

    pub(super) fn totals(&self, results: &[DebtServiceResult]) -> Result<DebtCashFlows> {
        let mut draw = ZERO;
        let mut interest = ZERO;
        let mut principal = ZERO;
        let mut sweep = ZERO;
        for row in results {
            draw = add(draw, row.draw.major)?;
            interest = add(interest, row.interest.major)?;
            principal = add(principal, row.scheduled_principal.major)?;
            sweep = add(sweep, row.sweep.major)?;
        }
        Ok(DebtCashFlows {
            draw: self.money(draw),
            interest: self.money(interest),
            scheduled_principal: self.money(principal),
            sweep: self.money(sweep),
        })
    }
}
