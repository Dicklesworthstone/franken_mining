//! DESIGN (BD-314): declared pre-money subscriptions change funded JV ownership and later dividends.
//! Determinism: Exact (explicit call sequence, shared-state contributions and rational ownership carry).

use super::*;

pub(super) mod render;
#[cfg(test)]
mod tests;

pub const METHOD: &str = "pre_money_subscription";
pub const CASH_FLOW_CONVENTION: &str = "native_excludes_modeled_capital_calls";
pub const MAX_CAPITAL_CALLS: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapitalCallState {
    pub state_id: String,
    pub pre_money_value: Money,
    pub required_funding: Money,
    pub parent_contribution: Money,
    pub external_contribution: Money,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapitalCall {
    pub id: String,
    pub entity_id: String,
    pub period_id: String,
    /// Positive explicit order within an entity/period, independent of identifiers and input order.
    pub sequence: u16,
    pub candidate_id: Option<String>,
    pub basis: String,
    pub states: Vec<CapitalCallState>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntityCapitalCallFunding {
    pub method: String,
    pub cash_flow_convention: String,
    pub calls: Vec<CapitalCall>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CapitalCallCashFlows {
    pub parent_in: ExactMoney,
    pub parent_out: ExactMoney,
    pub external_in: ExactMoney,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapitalCallResult {
    pub call_id: String,
    pub entity_id: String,
    pub parent_entity_id: String,
    pub period_id: String,
    pub checkpoint_date: String,
    pub sequence: u16,
    pub candidate_id: Option<String>,
    pub basis: String,
    pub active: bool,
    pub pre_money_value: ExactMoney,
    pub required_funding: ExactMoney,
    pub declared_parent_contribution: ExactMoney,
    pub declared_external_contribution: ExactMoney,
    pub parent_contribution: ExactMoney,
    pub external_contribution: ExactMoney,
    /// Active calls report contributions minus required funding; inactive calls report zero.
    pub funding_surplus: ExactMoney,
    pub parent_share_before: Rat,
    pub parent_share_after: Rat,
    pub external_share_after: Rat,
    pub ownership_residual: Rat,
}

pub(super) fn validate_bounds(calls: &EntityCapitalCallFunding, states: usize) -> Result<()> {
    if !(1..=MAX_CAPITAL_CALLS).contains(&calls.calls.len())
        || calls.calls.iter().any(|call| call.states.len() != states)
    {
        return Err(bounded(
            "capital calls require 1..=64 calls and every shared state exactly once, including zero contributions",
        ));
    }
    Ok(())
}

pub(super) fn validate(
    funding: &EntityFunding,
    states: &BTreeSet<&str>,
    candidates: &BTreeMap<&str, usize>,
) -> Result<()> {
    let Some(calls) = &funding.capital_calls else {
        return Ok(());
    };
    validate_bounds(calls, states.len())?;
    if calls.method != METHOD || calls.cash_flow_convention != CASH_FLOW_CONVENTION {
        return Err(invalid(
            "capital calls require pre_money_subscription and native_excludes_modeled_capital_calls",
        ));
    }
    let periods: BTreeSet<_> = funding.periods.iter().map(|p| p.id.as_str()).collect();
    let children: BTreeSet<_> = funding
        .distributions
        .iter()
        .map(|d| d.entity_id.as_str())
        .collect();
    let mut ids = BTreeSet::new();
    let mut sequences = BTreeSet::new();
    for call in &calls.calls {
        if !key(&call.id)
            || !ids.insert(call.id.as_str())
            || !children.contains(call.entity_id.as_str())
            || !periods.contains(call.period_id.as_str())
            || !(1..=MAX_CAPITAL_CALLS).contains(&usize::from(call.sequence))
            || !sequences.insert((
                call.entity_id.as_str(),
                call.period_id.as_str(),
                call.sequence,
            ))
            || call
                .candidate_id
                .as_deref()
                .is_some_and(|id| !candidates.contains_key(id))
            || !named(&call.basis)
        {
            return Err(invalid(
                "capital calls need unique ids, a distributing entity, known period and optional candidate, a unique entity/period sequence in 1..=64 and an explicit pre-money subscription basis",
            ));
        }
        let mut present = BTreeSet::new();
        for row in &call.states {
            if !states.contains(row.state_id.as_str())
                || !present.insert(row.state_id.as_str())
                || row.pre_money_value.minor <= 0
                || [
                    row.pre_money_value,
                    row.required_funding,
                    row.parent_contribution,
                    row.external_contribution,
                ]
                .iter()
                .any(|value| value.ccy != funding.currency || value.minor < 0)
            {
                return Err(invalid(
                    "each capital call needs every shared state once, positive pre-money equity value, and nonnegative required funding and contributions in the program currency",
                ));
            }
        }
    }
    Ok(())
}

pub(super) fn canonicalize(calls: &mut EntityCapitalCallFunding, periods: &BTreeMap<&str, usize>) {
    calls.calls.sort_by(|a, b| {
        (
            periods.get(a.period_id.as_str()),
            &a.entity_id,
            a.sequence,
            &a.id,
        )
            .cmp(&(
                periods.get(b.period_id.as_str()),
                &b.entity_id,
                b.sequence,
                &b.id,
            ))
    });
    for call in &mut calls.calls {
        call.states.sort_by(|a, b| a.state_id.cmp(&b.state_id));
    }
}

#[derive(Clone, Copy)]
struct Amounts {
    pre_money: Rat,
    required: Rat,
    parent: Rat,
    external: Rat,
}

struct IndexedCall<'a> {
    declaration: &'a CapitalCall,
    entity: usize,
    parent: usize,
    parent_id: &'a str,
    period: usize,
    candidate_bit: Option<usize>,
    states: Vec<Amounts>,
}

pub(super) struct Index<'a> {
    currency: Currency,
    entity_count: usize,
    calls: Vec<IndexedCall<'a>>,
}

pub(super) struct Settlement {
    pub rows: Vec<CapitalCallResult>,
    pub flows: Vec<CapitalCallCashFlows>,
    pub totals: CapitalCallCashFlows,
}

impl<'a> Index<'a> {
    pub(super) fn new(
        funding: &'a EntityFunding,
        entities: &BTreeMap<&str, usize>,
        periods: &BTreeMap<&str, usize>,
        states: &BTreeMap<&str, usize>,
        candidates: &BTreeMap<&str, usize>,
    ) -> Result<Self> {
        let declaration = funding
            .capital_calls
            .as_ref()
            .ok_or_else(|| internal("admitted entity funding lost its capital calls"))?;
        let parents: BTreeMap<_, _> = funding
            .distributions
            .iter()
            .map(|d| (d.entity_id.as_str(), d.parent_entity_id.as_str()))
            .collect();
        let mut calls = Vec::with_capacity(declaration.calls.len());
        for call in &declaration.calls {
            let parent_id = *parents
                .get(call.entity_id.as_str())
                .ok_or_else(|| internal("admitted capital call lost its parent"))?;
            let mut amounts = vec![
                Amounts {
                    pre_money: ZERO,
                    required: ZERO,
                    parent: ZERO,
                    external: ZERO
                };
                states.len()
            ];
            for row in &call.states {
                amounts[index_of(states, &row.state_id)?] = Amounts {
                    pre_money: amount(row.pre_money_value)?,
                    required: amount(row.required_funding)?,
                    parent: amount(row.parent_contribution)?,
                    external: amount(row.external_contribution)?,
                };
            }
            calls.push(IndexedCall {
                declaration: call,
                entity: index_of(entities, &call.entity_id)?,
                parent: index_of(entities, parent_id)?,
                parent_id,
                period: index_of(periods, &call.period_id)?,
                candidate_bit: call
                    .candidate_id
                    .as_deref()
                    .map(|id| index_of(candidates, id).map(|index| 1_usize << index))
                    .transpose()?,
                states: amounts,
            });
        }
        calls.sort_by(|a, b| {
            (
                a.period,
                a.entity,
                a.declaration.sequence,
                &a.declaration.id,
            )
                .cmp(&(
                    b.period,
                    b.entity,
                    b.declaration.sequence,
                    &b.declaration.id,
                ))
        });
        Ok(Self {
            currency: funding.currency,
            entity_count: entities.len(),
            calls,
        })
    }

    fn money(&self, major: Rat) -> ExactMoney {
        ExactMoney {
            currency: self.currency,
            major,
        }
    }

    pub(super) fn settle(
        &self,
        period: usize,
        state: usize,
        selected_mask: usize,
        closing_date: &str,
        shares: &mut [Rat],
    ) -> Result<Settlement> {
        let zero = CapitalCallCashFlows {
            parent_in: self.money(ZERO),
            parent_out: self.money(ZERO),
            external_in: self.money(ZERO),
        };
        let mut flows = vec![zero; self.entity_count];
        let mut totals = zero;
        let mut rows = Vec::new();
        for indexed in self.calls.iter().filter(|call| call.period == period) {
            let call = indexed.declaration;
            let declared = indexed.states[state];
            let active = indexed
                .candidate_bit
                .is_none_or(|bit| selected_mask & bit != 0);
            let parent = if active { declared.parent } else { ZERO };
            let external = if active { declared.external } else { ZERO };
            let raised = add(parent, external)?;
            let before = shares[indexed.entity];
            let after = if active {
                arithmetic(
                    add(arithmetic(before.checked_mul(declared.pre_money))?, parent)?
                        .checked_div(add(declared.pre_money, raised)?),
                )?
            } else {
                before
            };
            let outside = subtract(ONE, after)?;
            let residual = subtract(add(after, outside)?, ONE)?;
            if residual != ZERO || after.signum() < 0 || outside.signum() < 0 {
                return Err(internal("capital-call ownership did not conserve exactly"));
            }
            shares[indexed.entity] = after;
            flows[indexed.entity].parent_in.major =
                add(flows[indexed.entity].parent_in.major, parent)?;
            flows[indexed.parent].parent_out.major =
                add(flows[indexed.parent].parent_out.major, parent)?;
            flows[indexed.entity].external_in.major =
                add(flows[indexed.entity].external_in.major, external)?;
            totals.parent_in.major = add(totals.parent_in.major, parent)?;
            totals.parent_out.major = add(totals.parent_out.major, parent)?;
            totals.external_in.major = add(totals.external_in.major, external)?;
            rows.push(CapitalCallResult {
                call_id: call.id.clone(),
                entity_id: call.entity_id.clone(),
                parent_entity_id: indexed.parent_id.into(),
                period_id: call.period_id.clone(),
                checkpoint_date: closing_date.into(),
                sequence: call.sequence,
                candidate_id: call.candidate_id.clone(),
                basis: call.basis.clone(),
                active,
                pre_money_value: self.money(declared.pre_money),
                required_funding: self.money(declared.required),
                declared_parent_contribution: self.money(declared.parent),
                declared_external_contribution: self.money(declared.external),
                parent_contribution: self.money(parent),
                external_contribution: self.money(external),
                funding_surplus: self.money(if active {
                    subtract(raised, declared.required)?
                } else {
                    ZERO
                }),
                parent_share_before: before,
                parent_share_after: after,
                external_share_after: outside,
                ownership_residual: residual,
            });
        }
        Ok(Settlement {
            rows,
            flows,
            totals,
        })
    }
}
