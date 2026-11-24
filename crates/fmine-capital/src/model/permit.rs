//! DESIGN (BD-297): strict source rows for main-work continuation requirements.
//! Determinism: Exact

use fmine_registry::toml_subset::TomlRow;

use super::*;
use crate::permit::{DeemedRule, Disposition, Pause, ProgramPermits, Record, Terms};

#[derive(Default)]
pub(super) struct Rows {
    applications: BTreeMap<String, Terms>,
    records: BTreeMap<(String, String), Record>,
    pauses: BTreeMap<(String, String), Vec<Pause>>,
    requirements: BTreeSet<(String, String)>,
    anchors: Vec<SourceAnchor>,
}

impl Rows {
    pub(super) fn push(&mut self, row: &TomlRow, source: &SourceAnchor) -> Result<()> {
        let parse = |names| fields(&row.fields, names, &[], row.line);
        let pct = |value| {
            share_fraction(value, row.line).map_err(|_| {
                input(
                    row.line,
                    "permit scope percentages require exact n/d fractions in percentage points",
                )
            })
        };
        match row.table.as_str() {
            "permit" => {
                let f = parse(&[
                    "application",
                    "authority",
                    "filed_on",
                    "clock_start",
                    "clock_days",
                    "deemed_rule",
                    "reopen_at_pct",
                    "change_basis",
                    "source",
                    "known_on",
                ])?;
                let terms = Terms {
                    authority: get(&f, "authority")?.into(),
                    filed_on: get(&f, "filed_on")?.into(),
                    clock_start: get(&f, "clock_start")?.into(),
                    clock_days: get(&f, "clock_days")?
                        .parse()
                        .map_err(|_| input(row.line, "clock_days requires a positive integer"))?,
                    deemed_rule: DeemedRule::parse(get(&f, "deemed_rule")?)?,
                    reopen_at_pct: pct(get(&f, "reopen_at_pct")?)?,
                    change_basis: get(&f, "change_basis")?.into(),
                    source: get(&f, "source")?.into(),
                    known_on: get(&f, "known_on")?.into(),
                };
                if self
                    .applications
                    .insert(get(&f, "application")?.into(), terms)
                    .is_some()
                {
                    return Err(input(row.line, "duplicate permit application"));
                }
            }
            "permit_state" => {
                let f = parse(&[
                    "application",
                    "state",
                    "status",
                    "effective_on",
                    "scope_change_pct",
                    "change_basis",
                    "source",
                    "known_on",
                ])?;
                let record = Record {
                    disposition: Disposition::parse(get(&f, "status")?)?,
                    effective_on: get(&f, "effective_on")?.into(),
                    scope_change_pct: pct(get(&f, "scope_change_pct")?)?,
                    change_basis: get(&f, "change_basis")?.into(),
                    source: get(&f, "source")?.into(),
                    known_on: get(&f, "known_on")?.into(),
                };
                if self
                    .records
                    .insert(
                        (get(&f, "application")?.into(), get(&f, "state")?.into()),
                        record,
                    )
                    .is_some()
                {
                    return Err(input(
                        row.line,
                        "duplicate permit application and state status",
                    ));
                }
            }
            "permit_pause" => {
                let f = parse(&[
                    "application",
                    "state",
                    "from",
                    "resumed_on",
                    "source",
                    "known_on",
                ])?;
                let resumed = get(&f, "resumed_on")?;
                self.pauses
                    .entry((get(&f, "application")?.into(), get(&f, "state")?.into()))
                    .or_default()
                    .push(Pause {
                        from: get(&f, "from")?.into(),
                        resumed_on: (resumed != "open").then(|| resumed.into()),
                        source: get(&f, "source")?.into(),
                        known_on: get(&f, "known_on")?.into(),
                    });
            }
            "stage_permit" => {
                let f = parse(&["candidate", "application"])?;
                if !self
                    .requirements
                    .insert((get(&f, "candidate")?.into(), get(&f, "application")?.into()))
                {
                    return Err(input(row.line, "duplicate candidate permit requirement"));
                }
            }
            _ => return Err(input(row.line, "unknown permit table")),
        }
        self.anchors.push(located(source, row.line, &row.table));
        Ok(())
    }

    pub(super) fn finish(
        mut self,
        model_key: &str,
        information: Option<&Claim<RecourseBasis>>,
        unknowns: &[UuTerm],
        uu_anchors: &[SourceAnchor],
    ) -> Result<Option<Claim<ProgramPermits>>> {
        if self.anchors.is_empty() {
            return Ok(None);
        }
        let information = information
            .ok_or_else(|| input(1, "permit rows require an explicit recourse declaration"))?;
        let declaration = ProgramPermits {
            model_key: model_key.into(),
            as_of: information.value.observation_date.clone(),
            applications: self.applications,
            records: self.records,
            pauses: self.pauses,
            requirements: self.requirements,
        };
        self.anchors.extend(information.anchors.iter().cloned());
        self.anchors.extend_from_slice(uu_anchors);
        let tag = declaration.canonical_tag();
        Ok(Some(assumed(
            format!("capital.{model_key}.stage-permits"),
            declaration,
            &tag,
            AdditivityClass::NonAdditive,
            unknowns,
            self.anchors,
        )))
    }
}
