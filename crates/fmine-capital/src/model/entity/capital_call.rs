//! Strict declared subscription rounds within the original entity funding source.
//! Determinism: Exact (explicit sequences and complete, source-located state rows).

use super::*;
use crate::entity::capital_call::{
    CASH_FLOW_CONVENTION, CapitalCall, CapitalCallState, EntityCapitalCallFunding,
    MAX_CAPITAL_CALLS, METHOD,
};

#[derive(Default)]
pub(super) struct CapitalCallRows {
    header: Option<(String, String)>,
    calls: BTreeMap<String, CapitalCall>,
    states: BTreeMap<(String, String), CapitalCallState>,
}

pub(super) fn handles_table(table: &str) -> bool {
    matches!(
        table,
        "entity_capital_calls" | "capital_call" | "capital_call_state"
    )
}

impl CapitalCallRows {
    pub(super) fn push(&mut self, row: &TomlRow) -> Result<String> {
        let section = match row.table.as_str() {
            "entity_capital_calls" => {
                let fields = entity_fields(row, &["method", "cash_flow_convention"], &[])?;
                if self.header.is_some() {
                    return Err(input(
                        row.line,
                        "only one entity_capital_calls declaration is allowed",
                    ));
                }
                let method = get(&fields, "method")?;
                let convention = get(&fields, "cash_flow_convention")?;
                if method != METHOD || convention != CASH_FLOW_CONVENTION {
                    return Err(input(
                        row.line,
                        "capital calls require method = pre_money_subscription and cash_flow_convention = native_excludes_modeled_capital_calls",
                    ));
                }
                self.header = Some((method.into(), convention.into()));
                "entity capital-call method and cash-flow convention".into()
            }
            "capital_call" => {
                let fields = entity_fields(
                    row,
                    &["id", "entity", "period", "sequence", "basis"],
                    &["candidate"],
                )?;
                let sequence = get(&fields, "sequence")?;
                let sequence = sequence
                    .bytes()
                    .all(|byte| byte.is_ascii_digit())
                    .then(|| sequence.parse::<u16>().ok())
                    .flatten()
                    .filter(|value| (1..=MAX_CAPITAL_CALLS).contains(&usize::from(*value)))
                    .ok_or_else(|| {
                        input(
                            row.line,
                            "capital-call sequence must be a quoted integer in 1..=64",
                        )
                    })?;
                let id = get(&fields, "id")?;
                let call = CapitalCall {
                    id: id.into(),
                    entity_id: get(&fields, "entity")?.into(),
                    period_id: get(&fields, "period")?.into(),
                    sequence,
                    candidate_id: fields.get("candidate").map(|(value, _)| (*value).into()),
                    basis: get(&fields, "basis")?.into(),
                    states: Vec::new(),
                };
                if self.calls.insert(id.into(), call).is_some() {
                    return Err(input(row.line, "duplicate capital call"));
                }
                format!("capital call {id}")
            }
            "capital_call_state" => {
                let fields = entity_fields(
                    row,
                    &[
                        "call",
                        "state",
                        "pre_money_value",
                        "required_funding",
                        "parent_contribution",
                        "external_contribution",
                    ],
                    &[],
                )?;
                let call = get(&fields, "call")?;
                let state = get(&fields, "state")?;
                if !key(call) {
                    return Err(input(
                        row.line,
                        "capital-call identifiers require 1..=128 ASCII letters, digits, dots, underscores or hyphens",
                    ));
                }
                let value = CapitalCallState {
                    state_id: state.into(),
                    pre_money_value: money(get(&fields, "pre_money_value")?, row.line)?,
                    required_funding: money(get(&fields, "required_funding")?, row.line)?,
                    parent_contribution: money(get(&fields, "parent_contribution")?, row.line)?,
                    external_contribution: money(get(&fields, "external_contribution")?, row.line)?,
                };
                if self
                    .states
                    .insert((call.into(), state.into()), value)
                    .is_some()
                {
                    return Err(input(row.line, "duplicate capital-call/state amounts"));
                }
                format!("capital call state {call}/{state}")
            }
            other => {
                return Err(input(
                    row.line,
                    format!("unknown capital-call table {other}"),
                ));
            }
        };
        if self.calls.len() > MAX_CAPITAL_CALLS
            || self.states.len() > MAX_CAPITAL_CALLS * MAX_STATES
        {
            return Err(input(
                row.line,
                "capital-call rows exceed the bounded 64-call and 256-state domain",
            ));
        }
        Ok(section)
    }

    pub(super) fn finish(self) -> Result<Option<EntityCapitalCallFunding>> {
        let Some((method, cash_flow_convention)) = self.header else {
            if !self.calls.is_empty() || !self.states.is_empty() {
                return Err(input(
                    1,
                    "capital-call rows require an explicit entity_capital_calls declaration",
                ));
            }
            return Ok(None);
        };
        let mut calls = self.calls;
        for ((call, _), state) in self.states {
            calls
                .get_mut(&call)
                .ok_or_else(|| input(1, format!("amounts name unknown capital call {call}")))?
                .states
                .push(state);
        }
        Ok(Some(EntityCapitalCallFunding {
            method,
            cash_flow_convention,
            calls: calls.into_values().collect(),
        }))
    }
}
