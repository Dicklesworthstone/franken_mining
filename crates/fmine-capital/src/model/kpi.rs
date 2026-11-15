//! DESIGN (BD-229): source-bound economic KPI component matrices and explicit constraints.
//! Determinism: Exact (quoted rational quantities and original source anchors).

use super::*;
use crate::robustness::kpi::{Aggregation, Baseline, Constraint, Design, Direction, Increment, MAX_CONSTRAINTS, Metric};
use fmine_registry::toml_subset::TomlRow;

#[derive(Default)]
pub(super) struct Rows {
    constraints: BTreeMap<String, (Constraint, usize)>,
    baseline: BTreeMap<(String, String), (Rat, Rat, usize)>,
    increments: BTreeMap<(String, String, String), (Rat, Rat, usize)>,
}

impl Rows {
    pub(super) fn push(&mut self, row: &TomlRow) -> Result<()> {
        match row.table.as_str() {
            "kpi_constraint" => {
                let f = fields(&row.fields, &["id", "metric", "basis", "reporting_period", "aggregation", "direction", "limit"], &[], row.line)?;
                let id = get(&f, "id")?;
                let metric = match get(&f, "metric")? {
                    "gold_aisc" => Metric::GoldAisc,
                    "copper_aisc" => Metric::CopperAisc,
                    "roce" => Metric::Roce,
                    "geo" => Metric::Geo,
                    _ => return Err(input(row.line, "KPI metric must be gold_aisc, copper_aisc, roce or geo; safety and hard constraints cannot be priced")),
                };
                let aggregation = match get(&f, "aggregation")? {
                    "ratio_of_expected_totals" => Aggregation::RatioOfExpectedTotals,
                    "every_state" => Aggregation::EveryState,
                    _ => return Err(input(row.line, "KPI aggregation must be ratio_of_expected_totals or every_state")),
                };
                let direction = match get(&f, "direction")? {
                    "upper" => Direction::Upper,
                    "lower" => Direction::Lower,
                    _ => return Err(input(row.line, "KPI direction must be upper or lower; limits are inclusive")),
                };
                let constraint = Constraint {
                    id: id.into(), metric, basis: get(&f, "basis")?.into(), period: get(&f, "reporting_period")?.into(),
                    aggregation, direction, limit: quantity(get(&f, "limit")?, row.line)?, baseline: vec![], increments: vec![],
                };
                if self.constraints.len() >= MAX_CONSTRAINTS {
                    return Err(input(row.line, "KPI comparison admits at most eight declared constraints"));
                }
                if self.constraints.insert(id.into(), (constraint, row.line)).is_some() {
                    return Err(input(row.line, "duplicate KPI constraint"));
                }
            }
            "kpi_baseline" => {
                let f = fields(&row.fields, &["constraint", "state", "numerator", "denominator"], &[], row.line)?;
                let id = get(&f, "constraint")?;
                if !key(id) { return Err(input(row.line, "KPI constraint needs a bounded identifier")); }
                let pair = (id.into(), get(&f, "state")?.into());
                let value = (quantity(get(&f, "numerator")?, row.line)?, quantity(get(&f, "denominator")?, row.line)?, row.line);
                if self.baseline.insert(pair, value).is_some() {
                    return Err(input(row.line, "duplicate KPI baseline state"));
                }
            }
            "kpi_increment" => {
                let f = fields(&row.fields, &["constraint", "candidate", "state", "numerator", "denominator"], &[], row.line)?;
                let id = get(&f, "constraint")?;
                if !key(id) { return Err(input(row.line, "KPI constraint needs a bounded identifier")); }
                let pair = (id.into(), get(&f, "candidate")?.into(), get(&f, "state")?.into());
                let value = (quantity(get(&f, "numerator")?, row.line)?, quantity(get(&f, "denominator")?, row.line)?, row.line);
                if self.increments.insert(pair, value).is_some() {
                    return Err(input(row.line, "duplicate KPI candidate/state increment"));
                }
            }
            _ => return Err(input(row.line, "unknown KPI table")),
        }
        Ok(())
    }

    pub(super) fn finish(
        mut self,
        model_key: &str,
        source: &SourceAnchor,
        top: &[TomlField],
        unknowns: &[UuTerm],
        uu_anchors: &[SourceAnchor],
    ) -> Result<Option<Claim<Design>>> {
        if self.constraints.is_empty() {
            if !self.baseline.is_empty() || !self.increments.is_empty() {
                return Err(input(1, "KPI components require an explicit constraint declaration"));
            }
            return Ok(None);
        }
        let mut anchors: Vec<_> = self.constraints.values().map(|(c, line)| located(source, *line, &format!("KPI constraint {}", c.id))).collect();
        for ((id, state), (numerator, denominator, line)) in self.baseline {
            let Some((c, _)) = self.constraints.get_mut(&id) else {
                return Err(input(line, "baseline names an unknown KPI constraint"));
            };
            anchors.push(located(source, line, &format!("KPI baseline {id}/{state}")));
            c.baseline.push(Baseline { state_id: state, numerator, denominator });
        }
        for ((id, candidate, state), (numerator, denominator, line)) in self.increments {
            let Some((c, _)) = self.constraints.get_mut(&id) else {
                return Err(input(line, "increment names an unknown KPI constraint"));
            };
            anchors.push(located(source, line, &format!("KPI increment {id}/{candidate}/{state}")));
            c.increments.push(Increment { candidate_id: candidate, state_id: state, numerator, denominator });
        }
        anchors.extend(top.iter().filter(|field| field.key == "model_key").map(|field| located(source, field.line, &field.key)));
        anchors.extend(uu_anchors.iter().cloned());
        let design = Design { model_key: model_key.into(), constraints: self.constraints.into_values().map(|(c, _)| c).collect() };
        let tag = design.canonical_tag();
        Ok(Some(assumed(format!("capital.{model_key}.kpis"), design, &tag, AdditivityClass::NonAdditive, unknowns, anchors)))
    }
}

fn quantity(value: &str, line: usize) -> Result<Rat> {
    share_fraction(value, line).map_err(|_| input(line, "KPI quantities require exact signed n/d fractions with a positive denominator fitting i128"))
}
