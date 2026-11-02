//! Strict debt rows within the original source-bound entity funding declaration.
//! Determinism: Exact (complete declared matrices, quoted fractions and original row locators).

use super::*;
use crate::entity::debt::{
    CASH_FLOW_CONVENTION, DebtBaselineFlow, DebtCandidateFlow, DebtFacility, DebtOpeningPrincipal,
    EntityDebtFunding,
};

#[derive(Default)]
pub(super) struct DebtRows {
    convention: Option<String>,
    facilities: BTreeMap<String, DebtFacility>,
    opening: BTreeMap<(String, String), DebtOpeningPrincipal>,
    baseline: BTreeMap<(String, String, String), DebtBaselineFlow>,
    candidates: BTreeMap<(String, String, String, String), DebtCandidateFlow>,
}

pub(super) fn handles_table(table: &str) -> bool {
    matches!(
        table,
        "entity_debt"
            | "debt_facility"
            | "debt_opening_principal"
            | "debt_service"
            | "candidate_debt_service"
    )
}

fn debt_fields<'a>(row: &'a TomlRow, required: &[&str]) -> Result<Fields<'a>> {
    let result = entity_fields(row, required, &[])?;
    if let Some((facility, line)) = result.get("facility")
        && !key(facility)
    {
        return Err(input(
            *line,
            "debt facility identifiers require 1..=128 ASCII letters, digits, dots, underscores or hyphens",
        ));
    }
    Ok(result)
}

fn debt_fraction(value: &str, line: usize) -> Result<Rat> {
    share_fraction(value, line).map_err(|_| {
        input(
            line,
            "debt rates, thresholds and sweep fractions must be exact signed n/d fractions with a positive denominator fitting i128",
        )
    })
}

impl DebtRows {
    pub(super) fn push(&mut self, row: &TomlRow) -> Result<String> {
        let section = match row.table.as_str() {
            "entity_debt" => {
                let fields = debt_fields(row, &["cash_flow_convention"])?;
                if self.convention.is_some() {
                    return Err(input(
                        row.line,
                        "only one entity_debt declaration is allowed",
                    ));
                }
                let convention = get(&fields, "cash_flow_convention")?;
                if convention != CASH_FLOW_CONVENTION {
                    return Err(input(
                        row.line,
                        "entity debt requires cash_flow_convention = native_excludes_modeled_debt",
                    ));
                }
                self.convention = Some(convention.into());
                "entity debt cash-flow convention".into()
            }
            "debt_facility" => {
                let fields = debt_fields(
                    row,
                    &[
                        "id",
                        "entity",
                        "basis",
                        "cfads_basis",
                        "minimum_dscr",
                        "sweep_fraction",
                        "allow_distributions_without_service",
                    ],
                )?;
                let id = get(&fields, "id")?;
                let allow_distributions_without_service = match get(
                    &fields,
                    "allow_distributions_without_service",
                )? {
                    "true" => true,
                    "false" => false,
                    _ => {
                        return Err(input(
                            row.line,
                            "allow_distributions_without_service must be the quoted string true or false",
                        ));
                    }
                };
                if self
                    .facilities
                    .insert(
                        id.into(),
                        DebtFacility {
                            id: id.into(),
                            entity_id: get(&fields, "entity")?.into(),
                            basis: get(&fields, "basis")?.into(),
                            cfads_basis: get(&fields, "cfads_basis")?.into(),
                            minimum_dscr: debt_fraction(get(&fields, "minimum_dscr")?, row.line)?,
                            sweep_fraction: debt_fraction(
                                get(&fields, "sweep_fraction")?,
                                row.line,
                            )?,
                            allow_distributions_without_service,
                        },
                    )
                    .is_some()
                {
                    return Err(input(row.line, "duplicate debt facility"));
                }
                format!("debt facility {id}")
            }
            "debt_opening_principal" => {
                let fields = debt_fields(row, &["facility", "state", "value"])?;
                let facility = get(&fields, "facility")?;
                let state = get(&fields, "state")?;
                if self
                    .opening
                    .insert(
                        (facility.into(), state.into()),
                        DebtOpeningPrincipal {
                            facility_id: facility.into(),
                            state_id: state.into(),
                            value: money(get(&fields, "value")?, row.line)?,
                        },
                    )
                    .is_some()
                {
                    return Err(input(
                        row.line,
                        "duplicate debt facility/state opening principal",
                    ));
                }
                format!("debt opening principal {facility}/{state}")
            }
            "debt_service" => {
                let fields = debt_fields(
                    row,
                    &[
                        "facility",
                        "period",
                        "state",
                        "interest_rate",
                        "draw",
                        "principal_payment_cap",
                        "cfads",
                    ],
                )?;
                let facility = get(&fields, "facility")?;
                let period = get(&fields, "period")?;
                let state = get(&fields, "state")?;
                if self
                    .baseline
                    .insert(
                        (facility.into(), period.into(), state.into()),
                        DebtBaselineFlow {
                            facility_id: facility.into(),
                            period_id: period.into(),
                            state_id: state.into(),
                            interest_rate: debt_fraction(get(&fields, "interest_rate")?, row.line)?,
                            draw: money(get(&fields, "draw")?, row.line)?,
                            principal_payment_cap: money(
                                get(&fields, "principal_payment_cap")?,
                                row.line,
                            )?,
                            cfads: money(get(&fields, "cfads")?, row.line)?,
                        },
                    )
                    .is_some()
                {
                    return Err(input(
                        row.line,
                        "duplicate debt facility/period/state service",
                    ));
                }
                format!("debt service {facility}/{period}/{state}")
            }
            "candidate_debt_service" => {
                let fields = debt_fields(
                    row,
                    &[
                        "facility",
                        "period",
                        "candidate",
                        "state",
                        "draw",
                        "principal_payment_cap",
                        "cfads",
                    ],
                )?;
                let facility = get(&fields, "facility")?;
                let period = get(&fields, "period")?;
                let candidate = get(&fields, "candidate")?;
                let state = get(&fields, "state")?;
                if self
                    .candidates
                    .insert(
                        (
                            facility.into(),
                            period.into(),
                            candidate.into(),
                            state.into(),
                        ),
                        DebtCandidateFlow {
                            facility_id: facility.into(),
                            period_id: period.into(),
                            candidate_id: candidate.into(),
                            state_id: state.into(),
                            draw: money(get(&fields, "draw")?, row.line)?,
                            principal_payment_cap: money(
                                get(&fields, "principal_payment_cap")?,
                                row.line,
                            )?,
                            cfads: money(get(&fields, "cfads")?, row.line)?,
                        },
                    )
                    .is_some()
                {
                    return Err(input(
                        row.line,
                        "duplicate debt facility/period/candidate/state increment",
                    ));
                }
                format!("candidate debt service {facility}/{period}/{candidate}/{state}")
            }
            other => {
                return Err(input(
                    row.line,
                    format!("unknown entity debt table {other}"),
                ));
            }
        };
        self.check_counts(row.line)?;
        Ok(section)
    }

    fn check_counts(&self, line: usize) -> Result<()> {
        let facility_periods = MAX_ENTITIES * MAX_CASH_PERIODS;
        if self.facilities.len() > MAX_ENTITIES
            || self.opening.len() > MAX_ENTITIES * MAX_STATES
            || self.baseline.len() > facility_periods * MAX_STATES
            || self.candidates.len() > facility_periods * MAX_CANDIDATES * MAX_STATES
        {
            return Err(input(
                line,
                "debt rows exceed the bounded 4-facility, 16-period, 256-state and 8-candidate domain",
            ));
        }
        Ok(())
    }

    pub(super) fn finish(self) -> Result<Option<EntityDebtFunding>> {
        let Some(convention) = self.convention else {
            if !self.facilities.is_empty()
                || !self.opening.is_empty()
                || !self.baseline.is_empty()
                || !self.candidates.is_empty()
            {
                return Err(input(
                    1,
                    "debt facility, principal and service rows require an explicit entity_debt declaration",
                ));
            }
            return Ok(None);
        };
        for facility in self
            .opening
            .values()
            .map(|row| row.facility_id.as_str())
            .chain(self.baseline.values().map(|row| row.facility_id.as_str()))
            .chain(self.candidates.values().map(|row| row.facility_id.as_str()))
        {
            if !self.facilities.contains_key(facility) {
                return Err(input(
                    1,
                    format!("debt row names unknown facility {facility}"),
                ));
            }
        }
        Ok(Some(EntityDebtFunding {
            cash_flow_convention: convention,
            facilities: self.facilities.into_values().collect(),
            opening_principal: self.opening.into_values().collect(),
            baseline_flows: self.baseline.into_values().collect(),
            candidate_flows: self.candidates.into_values().collect(),
        }))
    }
}
