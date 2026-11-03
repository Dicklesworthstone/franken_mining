//! Strict independently source-bound entity NAV/share declarations within the original model.
//! Determinism: Exact (complete matrices and original entity-specific row anchors).

use fmine_registry::toml_subset::TomlRow;

use super::*;
use crate::entity::valuation::{
    CandidateNavChange, EntityValuation, MAX_SECONDARY_ENTITIES, MAX_VALUATION_ROWS,
};

struct Header {
    share_class: String,
    valuation_basis: String,
    shares_per_unit: Rat,
    initial_share_units: Rat,
    max_tail_loss_per_share: Option<Rat>,
}

#[derive(Default)]
struct PendingValuation {
    header: Option<Header>,
    baseline: BTreeMap<String, Money>,
    nav_changes: BTreeMap<(String, String), Money>,
    share_changes: BTreeMap<(String, String), Rat>,
    anchors: Vec<SourceAnchor>,
}

#[derive(Default)]
pub(super) struct ValuationRows {
    entities: BTreeMap<String, PendingValuation>,
}

pub(super) fn handles_table(table: &str) -> bool {
    matches!(
        table,
        "entity_valuation"
            | "entity_baseline_nav"
            | "entity_candidate_payoff"
            | "entity_candidate_share_change"
    )
}

impl ValuationRows {
    pub(super) fn push(&mut self, row: &TomlRow, source: &SourceAnchor) -> Result<()> {
        let (required, optional): (&[&str], &[&str]) = match row.table.as_str() {
            "entity_valuation" => (
                &[
                    "entity",
                    "share_class",
                    "valuation_basis",
                    "shares_per_unit",
                    "initial_share_units",
                ],
                &["max_tail_loss_per_share"],
            ),
            "entity_baseline_nav" => (&["entity", "state", "value"], &[]),
            "entity_candidate_payoff" | "entity_candidate_share_change" => {
                (&["entity", "candidate", "state", "value"], &[])
            }
            other => {
                return Err(input(
                    row.line,
                    format!("unknown entity valuation table {other}"),
                ));
            }
        };
        let f = fields(&row.fields, required, optional, row.line)?;
        let entity = get(&f, "entity")?;
        if !key(entity) {
            return Err(input(
                row.line,
                "entity valuation identifiers require 1..=128 ASCII letters, digits, dots, underscores or hyphens",
            ));
        }
        if !self.entities.contains_key(entity) && self.entities.len() >= MAX_SECONDARY_ENTITIES {
            return Err(input(
                row.line,
                "at most three additional entity valuations are supported",
            ));
        }
        let pending = self.entities.entry(entity.into()).or_default();
        let section = match row.table.as_str() {
            "entity_valuation" => {
                if pending.header.is_some() {
                    return Err(input(row.line, "duplicate entity valuation declaration"));
                }
                pending.header = Some(Header {
                    share_class: get(&f, "share_class")?.into(),
                    valuation_basis: get(&f, "valuation_basis")?.into(),
                    shares_per_unit: share_fraction(get(&f, "shares_per_unit")?, row.line)?,
                    initial_share_units: share_fraction(get(&f, "initial_share_units")?, row.line)?,
                    max_tail_loss_per_share: f
                        .get("max_tail_loss_per_share")
                        .map(|(value, line)| share_fraction(value, *line))
                        .transpose()?,
                });
                format!("entity valuation {entity} basis, shares and per-share risk limit")
            }
            "entity_baseline_nav" => {
                let state = get(&f, "state")?;
                if pending
                    .baseline
                    .insert(state.into(), money(get(&f, "value")?, row.line)?)
                    .is_some()
                {
                    return Err(input(row.line, "duplicate entity/state baseline NAV"));
                }
                format!("entity baseline NAV {entity}/{state}")
            }
            "entity_candidate_payoff" => {
                let candidate = get(&f, "candidate")?;
                let state = get(&f, "state")?;
                if pending
                    .nav_changes
                    .insert(
                        (candidate.into(), state.into()),
                        money(get(&f, "value")?, row.line)?,
                    )
                    .is_some()
                {
                    return Err(input(
                        row.line,
                        "duplicate entity/candidate/state NAV change",
                    ));
                }
                format!("entity candidate payoff {entity}/{candidate}/{state}")
            }
            "entity_candidate_share_change" => {
                let candidate = get(&f, "candidate")?;
                let state = get(&f, "state")?;
                if pending
                    .share_changes
                    .insert(
                        (candidate.into(), state.into()),
                        share_fraction(get(&f, "value")?, row.line)?,
                    )
                    .is_some()
                {
                    return Err(input(
                        row.line,
                        "duplicate entity/candidate/state share change",
                    ));
                }
                format!("entity candidate share change {entity}/{candidate}/{state}")
            }
            other => {
                return Err(input(
                    row.line,
                    format!("unknown entity valuation table {other}"),
                ));
            }
        };
        if pending.baseline.len() > MAX_STATES
            || pending.nav_changes.len() > MAX_CANDIDATES * MAX_STATES
            || pending.share_changes.len() > MAX_CANDIDATES * MAX_STATES
        {
            return Err(input(
                row.line,
                "entity valuation rows exceed 256 states and 8 candidates per entity",
            ));
        }
        pending.anchors.push(located(source, row.line, &section));
        Ok(())
    }

    pub(super) fn finish(
        self,
        context: entity::EntityContext<'_>,
        primary: Option<&EquityBasis>,
    ) -> Result<Vec<Claim<EntityValuation>>> {
        if self.entities.is_empty() {
            return Ok(Vec::new());
        }
        let primary = primary.ok_or_else(|| {
            bad(
                RefusalCode::BasisMismatch105,
                "entity valuations require an explicit primary equity_basis",
            )
        })?;
        let work = (self.entities.len() + 1)
            .checked_mul(context.states.len())
            .and_then(|rows| rows.checked_mul(1_usize << context.candidates.len()));
        if work.is_none_or(|rows| rows > MAX_VALUATION_ROWS) {
            return Err(bad(
                RefusalCode::NoClaimInDomain201,
                "entity valuation source exceeds 65536 complete primary-plus-secondary static state rows",
            ));
        }
        let mut claims = Vec::with_capacity(self.entities.len());
        for (entity, pending) in self.entities {
            let header = pending.header.ok_or_else(|| {
                input(
                    1,
                    format!("entity {entity} valuation rows require an explicit entity_valuation declaration"),
                )
            })?;
            if entity == primary.entity {
                return Err(bad(
                    RefusalCode::BasisMismatch105,
                    "additional entity valuation cannot duplicate the primary equity entity",
                ));
            }
            if pending
                .baseline
                .keys()
                .any(|state| !context.states.contains_key(state))
                || pending
                    .nav_changes
                    .keys()
                    .chain(pending.share_changes.keys())
                    .any(|(candidate, state)| {
                        !context.candidates.contains_key(candidate)
                            || !context.states.contains_key(state)
                    })
            {
                return Err(input(
                    1,
                    format!("entity {entity} valuation names an unknown candidate or state"),
                ));
            }
            let valuation = EntityValuation {
                equity: EquityBasis {
                    model_key: context.policy.model_key.clone(),
                    currency: context.policy.currency,
                    valuation_basis: header.valuation_basis,
                    entity: entity.clone(),
                    share_class: header.share_class,
                    shares_per_unit: header.shares_per_unit,
                    initial_share_units: header.initial_share_units,
                    baseline_nav: pending.baseline.into_iter().collect(),
                    share_changes: pending
                        .share_changes
                        .into_iter()
                        .map(|((candidate_id, state_id), units)| CandidateShareChange {
                            candidate_id,
                            state_id,
                            units,
                        })
                        .collect(),
                    max_tail_loss_per_share: header.max_tail_loss_per_share,
                },
                nav_changes: pending
                    .nav_changes
                    .into_iter()
                    .map(|((candidate_id, state_id), value)| CandidateNavChange {
                        candidate_id,
                        state_id,
                        value,
                    })
                    .collect(),
            };
            crate::entity::valuation::validate_bounds(
                &valuation,
                context.states.len(),
                context.candidates.len(),
            )?;
            let tag = valuation.canonical_tag();
            let mut anchors = pending.anchors;
            anchors.extend(context.uu_anchors.iter().cloned());
            anchors.extend(
                context
                    .top
                    .iter()
                    .filter(|field| {
                        matches!(
                            field.key.as_str(),
                            "model_key" | "currency" | "payoff_convention"
                        )
                    })
                    .map(|field| located(context.source, field.line, &field.key)),
            );
            claims.push(assumed(
                format!(
                    "capital.{}.entity_valuation.{entity}",
                    context.policy.model_key
                ),
                valuation,
                &tag,
                AdditivityClass::NonAdditive,
                context.unknowns,
                anchors,
            ));
        }
        Ok(claims)
    }
}
