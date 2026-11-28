//! Source-to-verdict blank contamination assessment; censor intervals never become substituted values.
//! Determinism: BitwisePortable

use std::collections::BTreeSet;

use fmine_drill::records::BlankStage;
use fmine_qaqc::insertion::{
    BlankFailureMode, BlankObservation, BlankStatus, PrecedingSampleGrade, SiteBlankRule,
    evaluate_blanks,
};
use fmine_registry::toml_subset::parse_subset;

use super::*;

pub const ROBOT_DOCS_JSON: &str = r#"{"schema":"fmine.blank-batch.v1","command":"drill blanks --input <blanks.toml>","input":"Quoted-string TOML; at most 1 MiB and 1..4096 ordered observations","top_fields":["schema","analyte","method","unit","basis","evidence_lane","source_description","omitted_uncertainty","policy_id","policy_version","policy_source","rule"],"rule":"dl_multiple requires detection_limit and dl_multiple; carryover_and or carryover_or requires abs_threshold and carryover_fraction","observation_fields":["result_id","sample_id","lab","batch","stage","value"],"optional_observation_fields":["preceding_sample_id","preceding_value"],"predecessor":"Supply both predecessor fields together for carryover rules; omitted means unknown, never zero","values":"Measured, <DL or >UL mass fractions; all have one declared analyte, method and dry or wet basis; stage coarse or pulp","output":"Every source row, censor bounds, threshold bounds, definite pass/failure or indeterminate, confirmed and possible failure rates, per-lab counts, source hash and diagnostic Claim","example":"tests/drill/blank_batch.toml","side_effect":"Read-only; certificate, policy approval and custody are not verified"}"#;

fn rule(top: &BTreeMap<&str, &str>) -> Result<(SiteBlankRule, String), Refusal> {
    let parameter = |key| {
        top.get(key)
            .ok_or_else(|| invalid(format!("missing {key}")))
            .and_then(|token| number(token))
    };
    let (mode, json) = match top["rule"] {
        "dl_multiple" => {
            if top.contains_key("abs_threshold") || top.contains_key("carryover_fraction") {
                return Err(invalid(
                    "dl_multiple cannot also declare carryover thresholds",
                ));
            }
            let detection_limit = parameter("detection_limit")?;
            let dl_multiple = parameter("dl_multiple")?;
            (
                BlankFailureMode::AboveMultipleOfDl {
                    detection_limit,
                    dl_multiple,
                },
                format!(
                    "{{\"mode\":\"dl_multiple\",\"detection_limit\":{detection_limit},\"dl_multiple\":{dl_multiple}}}"
                ),
            )
        }
        mode @ ("carryover_and" | "carryover_or") => {
            if top.contains_key("detection_limit") || top.contains_key("dl_multiple") {
                return Err(invalid(
                    "carryover rules cannot also declare DL-multiple thresholds",
                ));
            }
            let abs_threshold = parameter("abs_threshold")?;
            let carryover_fraction = parameter("carryover_fraction")?;
            (
                if mode == "carryover_and" {
                    BlankFailureMode::CarryoverAnd {
                        abs_threshold,
                        carryover_fraction,
                    }
                } else {
                    BlankFailureMode::CarryoverOr {
                        abs_threshold,
                        carryover_fraction,
                    }
                },
                format!(
                    "{{\"mode\":{},\"abs_threshold\":{abs_threshold},\"carryover_fraction\":{carryover_fraction}}}",
                    js(mode)
                ),
            )
        }
        _ => {
            return Err(invalid(
                "rule must be dl_multiple, carryover_and or carryover_or",
            ));
        }
    };
    Ok((
        SiteBlankRule::new(
            top["policy_id"],
            top["policy_version"],
            top["policy_source"],
            mode,
        ),
        json,
    ))
}

fn status(status: BlankStatus) -> &'static str {
    match status {
        BlankStatus::PassBelowDetection => "pass_below_detection",
        BlankStatus::PassMeasured => "pass_measured",
        BlankStatus::PassCensored => "pass_censored",
        BlankStatus::FailContamination => "fail_contamination",
        BlankStatus::Indeterminate => "indeterminate",
    }
}

pub(super) fn evaluate(text: &str, source: SourceAnchor) -> Result<Envelope, Refusal> {
    let doc = parse_subset(text).map_err(|e| invalid(e.to_string()))?;
    let mut required = COMMON.to_vec();
    required.push("rule");
    let top = fields(
        &doc.top,
        &required,
        &[
            "detection_limit",
            "dl_multiple",
            "abs_threshold",
            "carryover_fraction",
        ],
    )?;
    let metadata = common(&top, "fmine.blank-batch.v1")?;
    let (rule, policy) = rule(&top)?;
    if !(1..=MAX_ROWS).contains(&doc.rows.len()) {
        return Err(invalid("provide 1..4096 blank observations"));
    }
    let (mut result_ids, mut sample_ids) = (BTreeSet::new(), BTreeSet::new());
    let mut observations = Vec::new();
    let mut locators = Vec::new();
    for row in &doc.rows {
        if row.table != "observation" {
            return Err(invalid(format!("unknown table {}", row.table)));
        }
        let fields = fields(
            &row.fields,
            &["result_id", "sample_id", "lab", "batch", "stage", "value"],
            &["preceding_sample_id", "preceding_value"],
        )?;
        if fields["result_id"].len() > 256
            || fields["sample_id"].len() > 256
            || !result_ids.insert(fields["result_id"])
            || !sample_ids.insert(fields["sample_id"])
        {
            return Err(invalid(
                "result_id and sample_id must each be unique and at most 256 bytes",
            ));
        }
        let stage = match fields["stage"] {
            "coarse" => BlankStage::Coarse,
            "pulp" => BlankStage::Pulp,
            _ => return Err(invalid("blank stage must be coarse or pulp")),
        };
        let predecessor = match (
            fields.get("preceding_sample_id"),
            fields.get("preceding_value"),
        ) {
            (None, None) => None,
            (Some(id), Some(value)) if id.len() <= 256 && *id != fields["sample_id"] => {
                if matches!(rule.mode, BlankFailureMode::AboveMultipleOfDl { .. }) {
                    return Err(invalid("preceding sample applies only to carryover rules"));
                }
                Some(match assay(value)? {
                    AssayValue::Measured { mass_fraction } => {
                        PrecedingSampleGrade::Measured(mass_fraction)
                    }
                    AssayValue::BelowDetection { limit } => {
                        PrecedingSampleGrade::BelowDetection { limit }
                    }
                    AssayValue::AboveUpper { limit } => PrecedingSampleGrade::AboveUpper { limit },
                })
            }
            _ => {
                return Err(invalid(
                    "supply both preceding_sample_id and preceding_value; predecessor must be distinct and bounded",
                ));
            }
        };
        locators.push((
            row.line,
            fields["value"],
            fields.get("preceding_sample_id").copied(),
            fields.get("preceding_value").copied(),
        ));
        observations.push(BlankObservation {
            result_id: fields["result_id"].into(),
            sample_id: fields["sample_id"].into(),
            stage,
            lab: fields["lab"].into(),
            lab_batch: fields["batch"].into(),
            value: assay(fields["value"])?,
            preceding_sample_grade: predecessor,
        });
    }
    let result = evaluate_blanks(&observations, &rule).map_err(|e| e.to_refusal())?;
    let mut markdown = heading("Blank contamination assessment", &top);
    let _ = write!(
        markdown,
        "Rule: `{}`. {} supplied; {} definite passes, {} confirmed failures, {} indeterminate. Confirmed failure rate {}%; possible failure rate {}%. Denominator includes every supplied blank.\n\n| Sample | Lab | Batch | Stage | Supplied result | Status | Threshold range | Source line |\n|---|---|---|---|---|---|---|---:|\n",
        top["rule"],
        result.total_count,
        result.pass_count,
        result.failure_count,
        result.indeterminate_count,
        optional_number(result.failure_rate_pct),
        optional_number(result.possible_failure_rate_pct)
    );
    let mut rows = Vec::new();
    let mut labs: BTreeMap<&str, [usize; 4]> = BTreeMap::new();
    for (index, verdict) in result.verdicts.iter().enumerate() {
        let (line, token, predecessor_id, predecessor_value) = locators[index];
        let state = status(verdict.status);
        let counts = labs.entry(&verdict.lab).or_default();
        counts[0] += 1;
        counts[match verdict.status {
            BlankStatus::FailContamination => 2,
            BlankStatus::Indeterminate => 3,
            _ => 1,
        }] += 1;
        let predecessor = match (predecessor_id, predecessor_value) {
            (Some(id), Some(value)) => format!(
                "{{\"sample_id\":{},\"value\":{}}}",
                js(id),
                assay_json(assay(value)?)
            ),
            _ => "null".into(),
        };
        rows.push(format!("{{\"index\":{index},\"source_line\":{line},\"result_id\":{},\"sample_id\":{},\"lab\":{},\"batch\":{},\"stage\":{},\"value\":{},\"preceding_sample\":{predecessor},\"status\":{},\"threshold\":{{\"lower\":{},\"upper\":{},\"lower_exclusive\":{},\"upper_exclusive\":{}}}}}",
            js(&verdict.result_id), js(&verdict.sample_id), js(&verdict.lab), js(&verdict.lab_batch), js(verdict.stage.code()), assay_json(verdict.value), js(state),
            verdict.threshold_lower, verdict.threshold_upper, verdict.threshold_lower_exclusive, verdict.threshold_upper_exclusive));
        let cell = worksheet_input::markdown_cell;
        let _ = writeln!(
            markdown,
            "| {} | {} | {} | {} | {} | {} | {}{}, {}{} | {line} |",
            cell(&verdict.sample_id),
            cell(&verdict.lab),
            cell(&verdict.lab_batch),
            verdict.stage.code(),
            cell(token),
            state,
            if verdict.threshold_lower_exclusive {
                "("
            } else {
                "["
            },
            verdict.threshold_lower,
            verdict.threshold_upper,
            if verdict.threshold_upper_exclusive {
                ")"
            } else {
                "]"
            }
        );
    }
    let per_lab = labs
        .into_iter()
        .map(|(lab, counts)| {
            format!(
                "{{\"lab\":{},\"n\":{},\"passes\":{},\"failures\":{},\"indeterminate\":{}}}",
                js(lab),
                counts[0],
                counts[1],
                counts[2],
                counts[3]
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let report = format!(
        "{{\"schema\":\"fmine.blank-batch-report.v1\",{metadata},\"source\":{},\"policy\":{policy},\"threshold_comparison\":\"strict_binary64_exceedance\",\"n\":{},\"passes\":{},\"failures\":{},\"indeterminate\":{},\"below_detection\":{},\"confirmed_failure_rate_pct\":{},\"possible_failure_rate_pct\":{},\"failure_rate_denominator\":\"all_supplied_blanks\",\"observations\":[{}],\"per_lab\":[{per_lab}]}}",
        worksheet_input::source_json(&source, text.len()),
        result.total_count,
        result.pass_count,
        result.failure_count,
        result.indeterminate_count,
        result.below_detection_count,
        optional_number(result.failure_rate_pct),
        optional_number(result.possible_failure_rate_pct),
        rows.join(",")
    );
    finish(Kind::Blanks, result, report, markdown, source)
}
