//! DESIGN. Matched-material screening of declared period totals (BD-162, §20.4).
//! Determinism: BitwisePortable for the exact ratios and classifications.

use std::collections::BTreeSet;
use std::path::Path;

use fmine_core::{Digest32, anchor::SourceAnchor};
use fmine_evidence::{Refusal, RefusalCode};
use fmine_flow::compensation::{
    CompensationClass, WithinFactorInput, detect_within_factor_compensation,
};
use fmine_registry::toml_subset::{TomlField, TomlValue, parse_subset};
use fmine_units::ExactQty;

use crate::{Envelope, json_string as quote, worksheet_input};

pub(super) const USAGE: &str = "recon run --input <matched-cohorts.toml>";
const VERB: &str = "recon run";
const NO_CLAIM: &str = "Conditional arithmetic on declared dry tonnes and contained metal at the same material boundary. Inventory transit, measurement uncertainty and cause are unresolved. No period-close attestation.";

fn input_error(detail: impl Into<String>) -> Refusal {
    Refusal::new(RefusalCode::InputMissingOrUnreadable506, detail)
}

fn physical(error: impl ToString) -> Refusal {
    Refusal::new(RefusalCode::OutsidePhysicalDomain109, error.to_string())
}

fn keys(fields: &[TomlField], required: &[&str]) -> Result<(), Refusal> {
    for field in fields {
        if !required.contains(&field.key.as_str()) {
            return Err(input_error(format!(
                "unknown field {} at line {}",
                field.key, field.line
            )));
        }
    }
    for key in required {
        if !fields.iter().any(|field| field.key == *key) {
            return Err(input_error(format!("missing required field {key}")));
        }
    }
    Ok(())
}

fn field<'a>(fields: &'a [TomlField], key: &str) -> Result<&'a TomlValue, Refusal> {
    fields
        .iter()
        .find(|f| f.key == key)
        .map(|f| &f.value)
        .ok_or_else(|| input_error(format!("missing required field {key}")))
}

fn label<'a>(fields: &'a [TomlField], key: &str) -> Result<&'a str, Refusal> {
    match field(fields, key)? {
        TomlValue::Str(s)
            if !s.trim().is_empty() && s.trim() == s && !s.chars().any(char::is_control) =>
        {
            Ok(s)
        }
        _ => Err(input_error(format!(
            "{key} needs a nonempty text label without edge whitespace or control characters"
        ))),
    }
}

fn quantity(fields: &[TomlField], key: &str) -> Result<ExactQty, Refusal> {
    match field(fields, key)? {
        TomlValue::Int(n) => Ok(ExactQty::from_integer(i128::from(*n))),
        TomlValue::Str(s) => {
            ExactQty::from_decimal_str(s).map_err(|e| physical(format!("{key}: {e}")))
        }
    }
}

fn fraction(q: ExactQty) -> String {
    format!("{}/{}", q.num(), q.den())
}

fn percent(q: ExactQty) -> String {
    // Only the human display is rounded. All decisions and all JSON ratios use exact rationals.
    format!("{:.2}%", 100.0 * (q.num() as f64 / q.den() as f64))
}

fn markdown_cell(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('|', "\\|")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

struct Amounts {
    tonnes: ExactQty,
    metal: ExactQty,
}

impl Amounts {
    fn grade(&self) -> Result<Option<ExactQty>, Refusal> {
        if self.tonnes.signum() < 0 || self.metal.signum() < 0 {
            return Err(physical(
                "dry tonnes and contained metal must be nonnegative",
            ));
        }
        if self.tonnes.signum() == 0 {
            return if self.metal.signum() == 0 {
                Ok(None)
            } else {
                Err(physical("positive metal at zero dry mass"))
            };
        }
        let grade = self.metal.checked_div(self.tonnes).map_err(physical)?;
        if grade
            .checked_cmp(ExactQty::from_integer(1000))
            .map_err(physical)?
            .is_gt()
        {
            return Err(physical("contained metal exceeds the dry mass (1000 kg/t)"));
        }
        Ok(Some(grade))
    }

    fn json(&self) -> String {
        format!(
            "{{\"dry_tonnes\":{},\"contained_metal_kg\":{}}}",
            quote(&fraction(self.tonnes)),
            quote(&fraction(self.metal))
        )
    }
}

fn classify(input: &WithinFactorInput) -> Result<&'static str, Refusal> {
    let verdict = detect_within_factor_compensation(input).map_err(|e| e.to_refusal())?;
    match verdict.class {
        CompensationClass::MaskedWithinTolerance => Ok("masked_within_metal_tolerance"),
        CompensationClass::PartiallyMaskedOpposing => Ok("partially_masked"),
        CompensationClass::None => {
            let lower = verdict.tolerance.checked_neg().map_err(physical)?;
            for d in [
                verdict.tonnes_deviation,
                verdict.grade_deviation,
                verdict.metal_deviation,
            ] {
                if d.checked_cmp(lower).map_err(physical)?.is_lt()
                    || d.checked_cmp(verdict.tolerance).map_err(physical)?.is_gt()
                {
                    return Ok("outside_tolerance");
                }
            }
            Ok("within_tolerance")
        }
    }
}

fn screen(text: &str, anchor: &SourceAnchor) -> Result<Envelope, Refusal> {
    let doc = parse_subset(text).map_err(|e| input_error(e.to_string()))?;
    keys(
        &doc.top,
        &[
            "schema",
            "site",
            "period",
            "element",
            "ownership_basis",
            "tolerance_bps",
        ],
    )?;
    if label(&doc.top, "schema")? != "fmine.matched-cohorts.v1" || doc.rows.is_empty() {
        return Err(input_error(
            "expected fmine.matched-cohorts.v1 and at least one [[cohort]]",
        ));
    }
    let site = label(&doc.top, "site")?;
    let period = label(&doc.top, "period")?;
    let element = label(&doc.top, "element")?;
    let basis = label(&doc.top, "ownership_basis")?;
    let tol = match field(&doc.top, "tolerance_bps")? {
        TomlValue::Int(n @ 1..=9999) => *n as u32,
        _ => return Err(physical("tolerance_bps must be an integer in 1..=9999")),
    };
    let mut seen = BTreeSet::new();
    let mut rows = Vec::new();
    let mut flagged = 0usize;
    let mut markdown = format!(
        "# Matched-cohort reconciliation\n\n{} · {} · {} · {}\n\nActual as a percentage of plan, with a tolerance of {tol} basis points.\n\n| Material support | Source line | Dry tonnes | Grade | Contained metal | Finding |\n|---|---:|---:|---:|---:|---|\n",
        markdown_cell(site),
        markdown_cell(period),
        markdown_cell(element),
        markdown_cell(basis)
    );
    for row in &doc.rows {
        if row.table != "cohort" {
            return Err(input_error(format!(
                "unexpected table {} at line {}",
                row.table, row.line
            )));
        }
        let f = &row.fields;
        keys(
            f,
            &[
                "call_support",
                "actual_support",
                "call_dry_tonnes",
                "actual_dry_tonnes",
                "call_contained_metal_kg",
                "actual_contained_metal_kg",
            ],
        )?;
        let support = label(f, "call_support")?;
        if support != label(f, "actual_support")? {
            return Err(Refusal::new(
                RefusalCode::SupportMismatch102,
                format!(
                    "line {}: call and actual name different material supports",
                    row.line
                ),
            ));
        }
        if !seen.insert(support) {
            return Err(input_error(format!(
                "line {}: repeated cohort {support}",
                row.line
            )));
        }
        let call = Amounts {
            tonnes: quantity(f, "call_dry_tonnes")?,
            metal: quantity(f, "call_contained_metal_kg")?,
        };
        let actual = Amounts {
            tonnes: quantity(f, "actual_dry_tonnes")?,
            metal: quantity(f, "actual_contained_metal_kg")?,
        };
        let call_grade = call.grade()?.filter(|g| g.signum() > 0).ok_or_else(|| {
            physical(format!(
                "{support}: positive planned dry mass and metal required for actual/plan factors"
            ))
        })?;
        let tonnes_ratio = actual.tonnes.checked_div(call.tonnes).map_err(physical)?;
        let metal_ratio = actual.metal.checked_div(call.metal).map_err(physical)?;
        let grade_ratio = actual
            .grade()?
            .map(|g| g.checked_div(call_grade))
            .transpose()
            .map_err(physical)?;
        let finding = match grade_ratio {
            None => "no_actual_tonnage",
            Some(g) if g.signum() == 0 => "no_actual_metal",
            Some(g) => classify(&WithinFactorInput {
                site_id: site.into(),
                period: period.into(),
                factor_key: "matched_cohort.actual_over_call@v1".into(),
                ownership_basis: basis.into(),
                tonnes_ratio,
                grade_ratio: g,
                metal_ratio,
                tol_bps: tol,
                source_e_ids: vec![],
            })?,
        };
        let investigate = finding != "within_tolerance";
        flagged += usize::from(investigate);
        rows.push(format!("{{\"support\":{},\"worksheet_line\":{},\"call\":{},\"actual\":{},\"tonnes_ratio\":{},\"grade_ratio\":{},\"metal_ratio\":{},\"finding\":{},\"review\":{investigate}}}",
            quote(support),row.line,call.json(),actual.json(),quote(&fraction(tonnes_ratio)),
            grade_ratio.map_or_else(||"null".into(),|g| quote(&fraction(g))),quote(&fraction(metal_ratio)),quote(finding)));
        markdown.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} |\n",
            markdown_cell(support),
            row.line,
            percent(tonnes_ratio),
            grade_ratio.map_or_else(|| "undefined".into(), percent),
            percent(metal_ratio),
            finding.replace('_', " ")
        ));
    }
    markdown.push_str(&format!("\n{flagged} of {} cohorts need review.\n\nSource {} (SHA-256 `{}`). Inputs are assumed and uncalibrated.\n\n{NO_CLAIM}\n",
        rows.len(),markdown_cell(&anchor.artifact_path),Digest32(anchor.artifact_sha256)));
    let source = worksheet_input::source_json(anchor, text.len());
    let payload = format!(
        "{{\"schema\":\"fmine.matched-cohort-result.v1\",\"source\":{source},\"site\":{},\"period\":{},\"element\":{},\"ownership_basis\":{},\"factor\":\"matched_cohort.actual_over_call@v1\",\"evidence\":\"assumed\",\"calibration\":\"uncalibrated\",\"confidentiality\":\"internal\",\"tolerance_bps\":{tol},\"review_count\":{flagged},\"cohorts\":[{}]}}",
        quote(site),
        quote(period),
        quote(element),
        quote(basis),
        rows.join(",")
    );
    let mut env = Envelope::ok(VERB, payload, vec![]);
    env.rendered_markdown = Some(markdown);
    Ok(env)
}

pub(super) fn execute(input: &Path, root: &Path) -> Envelope {
    let result = worksheet_input::read(
        &root.join(input),
        "matched-cohort reconciliation",
        1024 * 1024,
    )
    .and_then(|(text, source)| screen(&text, &source));
    let mut env = result.unwrap_or_else(|e| Envelope::refused(VERB, e, vec![]));
    env.no_claim = Some(NO_CLAIM);
    env
}

#[cfg(test)]
mod tests {
    use super::*;
    use fmine_core::hash::sha256;
    const EXAMPLE: &str = include_str!("../../../tests/recon/matched_cohorts.toml");

    fn run(text: &str) -> Result<Envelope, Refusal> {
        screen(
            text,
            &SourceAnchor::new("sample.toml", sha256(text.as_bytes()).0, "worksheet"),
        )
    }

    #[test]
    fn unreconciled_stockpile_boundaries_and_duplicate_cohorts_refuse() {
        let unmatched = EXAMPLE.replacen(
            "actual_support = \"North pit / bench 240 / polygon A\"",
            "actual_support = \"mill feed after stockpile\"",
            1,
        );
        assert_eq!(
            run(&unmatched).unwrap_err().code,
            RefusalCode::SupportMismatch102.as_str()
        );
        assert_eq!(
            run(&EXAMPLE.replace("polygon B", "polygon A"))
                .unwrap_err()
                .code,
            RefusalCode::InputMissingOrUnreadable506.as_str()
        );
        assert!(run(&EXAMPLE.replace("call_dry_tonnes", "call_wet_tonnes")).is_err());
    }

    #[test]
    fn ordinary_shortfalls_partial_offsets_and_zero_actuals_remain_visible() {
        let changed = |tonnes, metal| {
            EXAMPLE
                .replacen(
                    "actual_dry_tonnes = 1200",
                    &format!("actual_dry_tonnes = {tonnes}"),
                    1,
                )
                .replacen(
                    "actual_contained_metal_kg = 10000",
                    &format!("actual_contained_metal_kg = {metal}"),
                    1,
                )
        };
        for (tonnes, metal, finding, grade) in [
            (800, 8000, "outside_tolerance", "\"1/1\""),
            (1200, 11000, "partially_masked", "\"11/12\""),
            (800, 0, "no_actual_metal", "\"0/1\""),
            (0, 0, "no_actual_tonnage", "null"),
        ] {
            let json = run(&changed(tonnes, metal)).unwrap().payload_json;
            assert!(json.contains(&format!("\"finding\":\"{finding}\",\"review\":true")));
            assert!(json.contains(&format!("\"grade_ratio\":{grade},")));
        }
        for (tonnes, metal) in [(0, 1), (-1, 10), (1, 1001)] {
            assert_eq!(
                run(&changed(tonnes, metal)).unwrap_err().code,
                RefusalCode::OutsidePhysicalDomain109.as_str()
            );
        }
        assert!(
            run(&EXAMPLE.replacen(
                "call_contained_metal_kg = 10000",
                "call_contained_metal_kg = 0",
                1
            ))
            .is_err()
        );
    }
}
