//! Strict source-bound entity funding rows, assembled within the original capital source document.
//! Determinism: Exact (no row is synthesized, stripped from the source, or assigned another file hash).

use fmine_registry::toml_subset::TomlRow;

use super::*;
use crate::entity::{
    Entity, EntityCandidateCashFlow, EntityDistribution, EntityDistributionGate, EntityFloor,
    EntityFunding, EntityOpeningCash, EntityPeriod, EntityStateCashFlow, EntityTransfer,
    MAX_ENTITIES, MAX_TRANSFERS,
};

mod debt;

struct Header {
    cash_basis: String,
    opening_date: String,
    objective_entity: String,
}

#[derive(Default)]
pub(super) struct EntityRows {
    header: Option<Header>,
    entities: BTreeMap<String, Entity>,
    periods: BTreeMap<String, EntityPeriod>,
    floors: BTreeMap<(String, String), EntityFloor>,
    opening: BTreeMap<(String, String), EntityOpeningCash>,
    baseline: BTreeMap<(String, String, String), EntityStateCashFlow>,
    candidates: BTreeMap<(String, String, String, String), EntityCandidateCashFlow>,
    transfers: BTreeMap<String, EntityTransfer>,
    transfer_amounts: BTreeMap<(String, String), Money>,
    distributions: BTreeMap<String, EntityDistribution>,
    gates: BTreeMap<(String, String, String), EntityDistributionGate>,
    debt: debt::DebtRows,
    anchors: Vec<SourceAnchor>,
}

pub(super) struct EntityContext<'a> {
    pub policy: &'a ProgramPolicy,
    pub states: &'a BTreeMap<String, Rat>,
    pub candidates: &'a BTreeMap<String, CapitalCandidate>,
    pub unknowns: &'a [UuTerm],
    pub uu_anchors: &'a [SourceAnchor],
    pub top: &'a [TomlField],
    pub source: &'a SourceAnchor,
}

pub(super) fn handles_table(table: &str) -> bool {
    matches!(
        table,
        "entity_funding"
            | "entity"
            | "entity_period"
            | "entity_floor"
            | "entity_opening_cash"
            | "entity_baseline_cash_flow"
            | "entity_candidate_cash_flow"
            | "entity_transfer"
            | "entity_transfer_amount"
            | "entity_distribution"
            | "entity_distribution_gate"
    ) || debt::handles_table(table)
}

fn entity_fields<'a>(row: &'a TomlRow, required: &[&str], optional: &[&str]) -> Result<Fields<'a>> {
    let result = fields(&row.fields, required, optional, row.line)?;
    for name in [
        "objective_entity",
        "entity",
        "from_entity",
        "to_entity",
        "parent_entity",
        "transfer",
    ] {
        if let Some((value, line)) = result.get(name)
            && !key(value)
        {
            return Err(input(
                *line,
                "entity and transfer identifiers require 1..=128 ASCII letters, digits, dots, underscores or hyphens",
            ));
        }
    }
    Ok(result)
}

impl EntityRows {
    pub(super) fn is_declared(&self) -> bool {
        self.header.is_some()
    }

    pub(super) fn push(&mut self, row: &TomlRow, source: &SourceAnchor) -> Result<()> {
        let section = match row.table.as_str() {
            "entity_funding" => {
                let f = entity_fields(
                    row,
                    &["cash_basis", "opening_date", "objective_entity"],
                    &[],
                )?;
                if self.header.is_some() {
                    return Err(input(
                        row.line,
                        "only one entity_funding declaration is allowed",
                    ));
                }
                self.header = Some(Header {
                    cash_basis: get(&f, "cash_basis")?.into(),
                    opening_date: get(&f, "opening_date")?.into(),
                    objective_entity: get(&f, "objective_entity")?.into(),
                });
                "entity funding objective, opening date and cash basis".into()
            }
            "entity" => {
                let f = entity_fields(row, &["id", "label"], &[])?;
                let id = get(&f, "id")?;
                if self
                    .entities
                    .insert(
                        id.into(),
                        Entity {
                            id: id.into(),
                            label: get(&f, "label")?.into(),
                        },
                    )
                    .is_some()
                {
                    return Err(input(row.line, "duplicate entity"));
                }
                format!("entity {id}")
            }
            "entity_period" => {
                let f = entity_fields(row, &["id", "cash_period"], &[])?;
                let id = get(&f, "id")?;
                if self
                    .periods
                    .insert(
                        id.into(),
                        EntityPeriod {
                            id: id.into(),
                            cash_period: get(&f, "cash_period")?.into(),
                        },
                    )
                    .is_some()
                {
                    return Err(input(row.line, "duplicate entity cash period"));
                }
                format!("entity period {id}")
            }
            "entity_floor" => {
                let f = entity_fields(row, &["entity", "period", "value"], &[])?;
                let entity = get(&f, "entity")?;
                let period = get(&f, "period")?;
                if self
                    .floors
                    .insert(
                        (entity.into(), period.into()),
                        EntityFloor {
                            entity_id: entity.into(),
                            period_id: period.into(),
                            minimum_cash: money(get(&f, "value")?, row.line)?,
                        },
                    )
                    .is_some()
                {
                    return Err(input(row.line, "duplicate entity/period cash floor"));
                }
                format!("entity floor {entity}/{period}")
            }
            "entity_opening_cash" => {
                let f = entity_fields(row, &["entity", "state", "value"], &[])?;
                let entity = get(&f, "entity")?;
                let state = get(&f, "state")?;
                if self
                    .opening
                    .insert(
                        (entity.into(), state.into()),
                        EntityOpeningCash {
                            entity_id: entity.into(),
                            state_id: state.into(),
                            value: money(get(&f, "value")?, row.line)?,
                        },
                    )
                    .is_some()
                {
                    return Err(input(row.line, "duplicate entity/state opening cash"));
                }
                format!("entity opening cash {entity}/{state}")
            }
            "entity_baseline_cash_flow" => {
                let f = entity_fields(row, &["entity", "period", "state", "value"], &[])?;
                let entity = get(&f, "entity")?;
                let period = get(&f, "period")?;
                let state = get(&f, "state")?;
                if self
                    .baseline
                    .insert(
                        (entity.into(), period.into(), state.into()),
                        EntityStateCashFlow {
                            entity_id: entity.into(),
                            period_id: period.into(),
                            state_id: state.into(),
                            value: money(get(&f, "value")?, row.line)?,
                        },
                    )
                    .is_some()
                {
                    return Err(input(
                        row.line,
                        "duplicate entity/period/state baseline cash flow",
                    ));
                }
                format!("entity baseline cash flow {entity}/{period}/{state}")
            }
            "entity_candidate_cash_flow" => {
                let f = entity_fields(
                    row,
                    &["entity", "period", "candidate", "state", "value"],
                    &[],
                )?;
                let entity = get(&f, "entity")?;
                let period = get(&f, "period")?;
                let candidate = get(&f, "candidate")?;
                let state = get(&f, "state")?;
                if self
                    .candidates
                    .insert(
                        (entity.into(), period.into(), candidate.into(), state.into()),
                        EntityCandidateCashFlow {
                            entity_id: entity.into(),
                            period_id: period.into(),
                            candidate_id: candidate.into(),
                            state_id: state.into(),
                            value: money(get(&f, "value")?, row.line)?,
                        },
                    )
                    .is_some()
                {
                    return Err(input(
                        row.line,
                        "duplicate entity/period/candidate/state cash flow",
                    ));
                }
                format!("entity candidate cash flow {entity}/{period}/{candidate}/{state}")
            }
            "entity_transfer" => {
                let f = entity_fields(
                    row,
                    &["id", "from_entity", "to_entity", "period", "basis"],
                    &["candidate"],
                )?;
                let id = get(&f, "id")?;
                if self
                    .transfers
                    .insert(
                        id.into(),
                        EntityTransfer {
                            id: id.into(),
                            from_entity: get(&f, "from_entity")?.into(),
                            to_entity: get(&f, "to_entity")?.into(),
                            period_id: get(&f, "period")?.into(),
                            candidate_id: f.get("candidate").map(|(value, _)| (*value).into()),
                            amounts: Vec::new(),
                            basis: get(&f, "basis")?.into(),
                        },
                    )
                    .is_some()
                {
                    return Err(input(row.line, "duplicate entity transfer"));
                }
                format!("entity transfer {id}")
            }
            "entity_transfer_amount" => {
                let f = entity_fields(row, &["transfer", "state", "value"], &[])?;
                let transfer = get(&f, "transfer")?;
                let state = get(&f, "state")?;
                if self
                    .transfer_amounts
                    .insert(
                        (transfer.into(), state.into()),
                        money(get(&f, "value")?, row.line)?,
                    )
                    .is_some()
                {
                    return Err(input(row.line, "duplicate transfer/state amount"));
                }
                format!("entity transfer amount {transfer}/{state}")
            }
            "entity_distribution" => {
                let f = entity_fields(
                    row,
                    &["entity", "parent_entity", "parent_share", "basis"],
                    &[],
                )?;
                let entity = get(&f, "entity")?;
                let parent = get(&f, "parent_entity")?;
                if self
                    .distributions
                    .insert(
                        entity.into(),
                        EntityDistribution {
                            entity_id: entity.into(),
                            parent_entity_id: parent.into(),
                            parent_share: share_fraction(get(&f, "parent_share")?, row.line)?,
                            basis: get(&f, "basis")?.into(),
                        },
                    )
                    .is_some()
                {
                    return Err(input(row.line, "duplicate distributing entity"));
                }
                format!("entity distribution {entity}/{parent}")
            }
            "entity_distribution_gate" => {
                let f = entity_fields(
                    row,
                    &[
                        "entity",
                        "period",
                        "state",
                        "allowed",
                        "maximum_distribution",
                    ],
                    &[],
                )?;
                let entity = get(&f, "entity")?;
                let period = get(&f, "period")?;
                let state = get(&f, "state")?;
                let allowed = match get(&f, "allowed")? {
                    "true" => true,
                    "false" => false,
                    _ => {
                        return Err(input(
                            row.line,
                            "distribution allowed must be the quoted string true or false",
                        ));
                    }
                };
                if self
                    .gates
                    .insert(
                        (entity.into(), period.into(), state.into()),
                        EntityDistributionGate {
                            entity_id: entity.into(),
                            period_id: period.into(),
                            state_id: state.into(),
                            allowed,
                            maximum_distribution: money(
                                get(&f, "maximum_distribution")?,
                                row.line,
                            )?,
                        },
                    )
                    .is_some()
                {
                    return Err(input(row.line, "duplicate distributor/period/state gate"));
                }
                format!("entity distribution gate {entity}/{period}/{state}")
            }
            other if debt::handles_table(other) => self.debt.push(row)?,
            other => {
                return Err(input(
                    row.line,
                    format!("unknown entity funding table {other}"),
                ));
            }
        };
        self.check_counts(row.line)?;
        self.anchors.push(located(source, row.line, &section));
        Ok(())
    }

    fn check_counts(&self, line: usize) -> Result<()> {
        let entity_periods = MAX_ENTITIES * MAX_CASH_PERIODS;
        if self.entities.len() > MAX_ENTITIES
            || self.periods.len() > MAX_CASH_PERIODS
            || self.transfers.len() > MAX_TRANSFERS
            || self.distributions.len() >= MAX_ENTITIES
            || self.floors.len() > entity_periods
            || self.opening.len() > MAX_ENTITIES * MAX_STATES
            || self.baseline.len() > entity_periods * MAX_STATES
            || self.candidates.len() > entity_periods * MAX_STATES * MAX_CANDIDATES
            || self.transfer_amounts.len() > MAX_TRANSFERS * MAX_STATES
            || self.gates.len() > (MAX_ENTITIES - 1) * MAX_CASH_PERIODS * MAX_STATES
        {
            return Err(input(
                line,
                "entity funding rows exceed the bounded 4-entity, 16-period, 64-transfer, 256-state and 8-candidate domain",
            ));
        }
        Ok(())
    }

    pub(super) fn finish(self, context: EntityContext<'_>) -> Result<Option<Claim<EntityFunding>>> {
        let Some(header) = self.header else {
            if !self.anchors.is_empty() {
                return Err(input(
                    1,
                    "entity cash, transfer and distribution rows require an explicit entity_funding declaration",
                ));
            }
            return Ok(None);
        };
        let mut transfers = self.transfers;
        for ((transfer, state), value) in self.transfer_amounts {
            transfers
                .get_mut(&transfer)
                .ok_or_else(|| {
                    input(
                        1,
                        format!("amount names unknown entity transfer {transfer}"),
                    )
                })?
                .amounts
                .push((state, value));
        }
        let mut funding = EntityFunding {
            model_key: context.policy.model_key.clone(),
            currency: context.policy.currency,
            cash_basis: header.cash_basis,
            opening_date: header.opening_date,
            objective_entity: header.objective_entity,
            entities: self.entities.into_values().collect(),
            periods: self.periods.into_values().collect(),
            floors: self.floors.into_values().collect(),
            opening_cash: self.opening.into_values().collect(),
            baseline_flows: self.baseline.into_values().collect(),
            candidate_flows: self.candidates.into_values().collect(),
            transfers: transfers.into_values().collect(),
            distributions: self.distributions.into_values().collect(),
            gates: self.gates.into_values().collect(),
            debt: self.debt.finish()?,
            capital_calls: None,
        };
        crate::entity::validate_bounds(&funding, context.states.len(), context.candidates.len())?;
        crate::entity::canonicalize(&mut funding);
        let tag = funding.canonical_tag();
        let mut anchors = self.anchors;
        anchors.extend(context.uu_anchors.iter().cloned());
        anchors.extend(
            context
                .top
                .iter()
                .filter(|field| matches!(field.key.as_str(), "model_key" | "currency"))
                .map(|field| located(context.source, field.line, &field.key)),
        );
        let claim = assumed(
            format!("capital.{}.entity_funding", context.policy.model_key),
            funding,
            &tag,
            AdditivityClass::NonAdditive,
            context.unknowns,
            anchors,
        );
        let states = context.states.keys().map(String::as_str).collect();
        let candidates = context
            .candidates
            .keys()
            .enumerate()
            .map(|(index, id)| (id.as_str(), index))
            .collect();
        crate::entity::validate(&claim, context.policy, &states, &candidates)?;
        Ok(Some(claim))
    }
}
