//! DESIGN. Matched-material screening and Touchstone posterior reconciliation / attested close
//! (`BD-162`, plan §20.4 & §20.9).
//! Determinism: BitwisePortable for matched-cohort screening; DeterministicPerIsa for posterior
//! reconciliation and certificate sealing.

pub mod posterior;

use std::collections::BTreeSet;
use std::path::Path;

use fmine_core::{Digest32, anchor::SourceAnchor};
use fmine_evidence::{Refusal, RefusalCode};
use fmine_flow::compensation::{
    CompensationClass, WithinFactorInput, detect_within_factor_compensation,
};
use fmine_registry::toml_subset::{TomlField, TomlValue, parse_subset};
use fmine_units::{ExactQty, UnitsError};

use crate::worksheet_input::markdown_cell;
use crate::{Envelope, json_string as quote, worksheet_input};

pub(super) const USAGE: &str = "recon run --input <worksheet.toml>";
pub(super) use posterior::{
    CLOSE_USAGE, CLOSE_VERB, CloseCommand, ROBOT_DOCS_JSON, execute_close, parse_close,
};
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

fn ratio(actual: ExactQty, planned: ExactQty) -> Result<Option<ExactQty>, Refusal> {
    if planned.signum() == 0 {
        Ok(None)
    } else {
        actual.checked_div(planned).map(Some).map_err(physical)
    }
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

/// DESIGN. Average the two orders in which tonnes and grade can change.
/// The tonnes and grade terms sum to the change in contained kg for the same support.
fn metal_split(
    tonnes_delta: ExactQty,
    metal_delta: ExactQty,
    call_grade: ExactQty,
    actual_grade: ExactQty,
) -> Result<(ExactQty, ExactQty), UnitsError> {
    let zero = ExactQty::from_integer(0);
    if tonnes_delta == zero {
        return Ok((zero, metal_delta));
    }
    if call_grade == actual_grade {
        return Ok((metal_delta, zero));
    }
    let mean_grade = call_grade
        .checked_add(actual_grade)?
        .checked_div(ExactQty::from_integer(2))?;
    let tonnes_effect = tonnes_delta.checked_mul(mean_grade)?;
    Ok((tonnes_effect, metal_delta.checked_sub(tonnes_effect)?))
}

fn exceeds_tolerance(deviation: ExactQty, tolerance: ExactQty) -> Result<bool, Refusal> {
    let magnitude = if deviation.signum() < 0 {
        deviation.checked_neg().map_err(physical)?
    } else {
        deviation
    };
    Ok(magnitude.checked_cmp(tolerance).map_err(physical)?.is_gt())
}

fn classify(input: &WithinFactorInput) -> Result<&'static str, Refusal> {
    let verdict = detect_within_factor_compensation(input).map_err(|e| e.to_refusal())?;
    match verdict.class {
        CompensationClass::MaskedWithinTolerance => Ok("masked_within_metal_tolerance"),
        CompensationClass::PartiallyMaskedOpposing => Ok("partially_masked"),
        CompensationClass::None => {
            for d in [
                verdict.tonnes_deviation,
                verdict.grade_deviation,
                verdict.metal_deviation,
            ] {
                if exceeds_tolerance(d, verdict.tolerance)? {
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
        "# Matched-cohort reconciliation\n\n{} · {} · {} · {}\n\nPercentages show actual divided by plan, rounded for display. Findings use the exact {tol} basis point tolerance. Differences are actual minus plan. The metal split averages the tonnes-first and grade-first changes. Zero dry tonnes leave the corresponding grade undefined.\n\n| Material support | Source line | Dry tonnes | Grade | Contained metal | Finding | Difference (dry t) | Difference (kg metal) | From tonnes (kg) | From grade (kg) |\n|---|---:|---:|---:|---:|---|---:|---:|---:|---:|\n",
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
        let call_grade = call.grade()?;
        let actual_grade = actual.grade()?;
        let tonnes_ratio = ratio(actual.tonnes, call.tonnes)?;
        let metal_ratio = ratio(actual.metal, call.metal)?;
        let grade_ratio = match (actual_grade, call_grade) {
            (Some(a), Some(c)) => ratio(a, c)?,
            _ => None,
        };
        let tonnes_delta = actual.tonnes.checked_sub(call.tonnes).map_err(physical)?;
        let metal_delta = actual.metal.checked_sub(call.metal).map_err(physical)?;
        let finding = match (tonnes_ratio, metal_ratio, grade_ratio) {
            (None, _, _) if actual.tonnes.signum() == 0 => "no_movement",
            (None, _, _) => "unplanned_movement",
            (Some(_), _, _) if actual.tonnes.signum() == 0 => "no_actual_tonnage",
            (Some(_), None, _) if actual.metal.signum() > 0 => "unplanned_metal",
            (Some(t), None, _) => {
                let tolerance = ExactQty::new(i128::from(tol), 10_000).map_err(physical)?;
                let difference = t.checked_sub(ExactQty::from_integer(1)).map_err(physical)?;
                if exceeds_tolerance(difference, tolerance)? {
                    "outside_tolerance"
                } else {
                    "within_tolerance"
                }
            }
            (Some(_), Some(_), _) if actual.metal.signum() == 0 => "no_actual_metal",
            (Some(t), Some(m), Some(g)) => classify(&WithinFactorInput {
                site_id: site.into(),
                period: period.into(),
                factor_key: "matched_cohort.actual_over_call@v1".into(),
                ownership_basis: basis.into(),
                tonnes_ratio: t,
                grade_ratio: g,
                metal_ratio: m,
                tol_bps: tol,
                source_e_ids: vec![],
            })?,
            _ => return Err(physical("inconsistent grade and metal denominators")),
        };
        let investigate = !matches!(finding, "within_tolerance" | "no_movement");
        flagged += usize::from(investigate);
        let (split_status, split) = match (call_grade, actual_grade) {
            (Some(c), Some(a)) => match metal_split(tonnes_delta, metal_delta, c, a) {
                Ok(pair) => ("computed", Some(pair)),
                Err(_) => ("outside_exact_range", None),
            },
            _ => ("undefined_grade", None),
        };
        let json_ratio =
            |q: Option<ExactQty>| q.map_or_else(|| "null".into(), |q| quote(&fraction(q)));
        let (from_tonnes, from_grade) = split.unzip();
        rows.push(format!("{{\"support\":{},\"worksheet_line\":{},\"call\":{},\"actual\":{},\"dry_tonnes_delta\":{},\"contained_metal_kg_delta\":{},\"tonnes_ratio\":{},\"grade_ratio\":{},\"metal_ratio\":{},\"finding\":{},\"review\":{investigate},\"metal_split\":{{\"status\":{},\"from_dry_tonnes_kg\":{},\"from_grade_kg\":{}}}}}",
            quote(support),row.line,call.json(),actual.json(),quote(&fraction(tonnes_delta)),quote(&fraction(metal_delta)),
            json_ratio(tonnes_ratio),json_ratio(grade_ratio),json_ratio(metal_ratio),quote(finding),
            quote(split_status),json_ratio(from_tonnes),json_ratio(from_grade)));
        let display = |q: Option<ExactQty>| q.map_or_else(|| "undefined".into(), percent);
        let display_kg =
            |q: Option<ExactQty>| q.map_or_else(|| split_status.replace('_', " "), fraction);
        markdown.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} | {} | {} | {} | {} |\n",
            markdown_cell(support),
            row.line,
            display(tonnes_ratio),
            display(grade_ratio),
            display(metal_ratio),
            finding.replace('_', " "),
            fraction(tonnes_delta),
            fraction(metal_delta),
            display_kg(from_tonnes),
            display_kg(from_grade)
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
    let result = worksheet_input::read(&root.join(input), "reconciliation worksheet", 1024 * 1024)
        .and_then(|(text, source)| {
            let is_posterior = parse_subset(&text)
                .ok()
                .and_then(|doc| {
                    doc.top
                        .iter()
                        .find(|f| f.key == "schema")
                        .and_then(|f| match &f.value {
                            TomlValue::Str(s) => Some(s == posterior::POSTERIOR_SCHEMA),
                            TomlValue::Int(_) => None,
                        })
                })
                .unwrap_or(false);
            if is_posterior {
                posterior::run_posterior(&text, &source, input)
            } else {
                let mut env = screen(&text, &source)?;
                env.no_claim = Some(NO_CLAIM);
                Ok(env)
            }
        });
    result.unwrap_or_else(|e| {
        let mut env = Envelope::refused(VERB, e, vec![]);
        env.no_claim = Some(NO_CLAIM);
        env
    })
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
    fn metal_split_matches_both_change_orders_and_reverses_on_swapping_periods() {
        let q = ExactQty::from_integer;
        for t0 in [1, 5, 12] {
            for t1 in [1, 7, 12] {
                for g0 in [0, 2, 9] {
                    for g1 in [0, 3, 9] {
                        let dt = t1 - t0;
                        let dm = t1 * g1 - t0 * g0;
                        let (tonnes, grade) = metal_split(q(dt), q(dm), q(g0), q(g1)).unwrap();
                        let mean = |n| ExactQty::new(n, 2).unwrap();
                        assert_eq!(tonnes, mean(dt * g0 + dt * g1));
                        assert_eq!(grade, mean((t0 + t1) * (g1 - g0)));
                        assert_eq!(tonnes.checked_add(grade).unwrap(), q(dm));
                        let reversed = metal_split(q(-dt), q(-dm), q(g1), q(g0)).unwrap();
                        assert_eq!(
                            reversed,
                            (tonnes.checked_neg().unwrap(), grade.checked_neg().unwrap())
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn unrepresentable_split_preserves_the_cohort_and_total_metal_delta() {
        let text = EXAMPLE
            .replacen(
                "call_dry_tonnes = 1000",
                "call_dry_tonnes = \"100000000000000000001\"",
                1,
            )
            .replacen(
                "actual_dry_tonnes = 1200",
                "actual_dry_tonnes = \"100000000000000000002\"",
                1,
            )
            .replacen(
                "call_contained_metal_kg = 10000",
                "call_contained_metal_kg = 1",
                1,
            )
            .replacen(
                "actual_contained_metal_kg = 10000",
                "actual_contained_metal_kg = 1",
                1,
            );
        let result = run(&text).unwrap();
        assert!(
            result
                .payload_json
                .contains(r#""contained_metal_kg_delta":"0/1""#)
        );
        assert!(result.payload_json.contains(r#""metal_split":{"status":"outside_exact_range","from_dry_tonnes_kg":null,"from_grade_kg":null}"#));
        assert!(
            result
                .rendered_markdown
                .unwrap()
                .contains("0/1 | outside exact range | outside exact range")
        );
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
    }

    #[test]
    fn zero_calls_preserve_valid_cohorts_and_only_undefined_ratios_are_null() {
        for (planned, moved, metal, ratios, finding, review) in [
            (0, 0, 0, ("null", "null"), "no_movement", false),
            (0, 10, 0, ("null", "null"), "unplanned_movement", true),
            (0, 10, 12, ("null", "null"), "unplanned_movement", true),
            (100, 95, 0, ("\"19/20\"", "null"), "within_tolerance", false),
            (100, 94, 0, ("\"47/50\"", "null"), "outside_tolerance", true),
            (
                100,
                105,
                0,
                ("\"21/20\"", "null"),
                "within_tolerance",
                false,
            ),
            (
                100,
                106,
                0,
                ("\"53/50\"", "null"),
                "outside_tolerance",
                true,
            ),
            (100, 100, 12, ("\"1/1\"", "null"), "unplanned_metal", true),
            (100, 0, 0, ("\"0/1\"", "null"), "no_actual_tonnage", true),
        ] {
            let text = EXAMPLE
                .replace(
                    "call_dry_tonnes = 0",
                    &format!("call_dry_tonnes = {planned}"),
                )
                .replace(
                    "actual_dry_tonnes = 200",
                    &format!("actual_dry_tonnes = {moved}"),
                )
                .replace(
                    "actual_contained_metal_kg = 2500",
                    &format!("actual_contained_metal_kg = {metal}"),
                );
            let json = run(&text).unwrap().payload_json;
            assert!(json.contains("masked_within_metal_tolerance"));
            let (tonnes_ratio, metal_ratio) = ratios;
            assert!(json.contains(&format!("\"dry_tonnes_delta\":\"{}/1\",\"contained_metal_kg_delta\":\"{metal}/1\",\"tonnes_ratio\":{tonnes_ratio},\"grade_ratio\":null,\"metal_ratio\":{metal_ratio},\"finding\":\"{finding}\",\"review\":{review}", moved-planned)), "{json}");
        }
        assert_eq!(
            run(&EXAMPLE.replace("call_contained_metal_kg = 0", "call_contained_metal_kg = 1"))
                .unwrap_err()
                .code,
            "FMINE-REF-109"
        );
    }

    #[test]
    fn worksheet_labels_render_as_literal_text() {
        let label = "[bench](west) | A_1 & <ore>";
        let env = run(&EXAMPLE.replace("East pit / unplanned cut", label)).unwrap();
        assert!(env.payload_json.contains(label));
        assert!(
            env.rendered_markdown
                .unwrap()
                .contains(r"\[bench\](west) \| A\_1 &amp; &lt;ore&gt;")
        );
    }
}
