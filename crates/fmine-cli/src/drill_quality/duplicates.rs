//! Source-bound duplicate precision grouped by declared stage and grade band.
//! Determinism: BitwisePortable

use std::collections::BTreeSet;

use fmine_drill::records::{DuplicateStage, SampleType};
use fmine_qaqc::{
    QaqcError,
    duplicates::{
        DuplicatePair, DuplicatePairStatus, DuplicatePolicy, DuplicatePrecisionReport,
        StageDecompositionStatus, decompose_duplicate_stages, evaluate_duplicates,
    },
};
use fmine_registry::toml_subset::parse_subset;

use super::*;

pub const ROBOT_DOCS_JSON: &str = r#"{"schema":"fmine.duplicate-batch.v1","command":"drill duplicates --input <duplicates.toml>","input":"Quoted-string TOML; at most 1 MiB and 1..4096 pairs","top_fields":["schema","analyte","method","unit","basis","lab","evidence_lane","source_description","omitted_uncertainty","policy_id","policy_version","policy_source","max_relative_mass_diff","refuse_on_mass_mismatch","cv_limit_pct"],"pair_fields":["original_sample_id","duplicate_sample_id","stage","grade_band","original","duplicate","original_mass_g","duplicate_mass_g","original_sample_type","duplicate_sample_type"],"values":"Measured, <DL or >UL mass fractions; dry or wet basis, one lab and method; no conversion or censor substitution","groups":"Each stage and grade band is scored separately; duplicate IDs are unique and an original occurs once per group; sample masses and types are mandatory","precision":"CV_AVR%=100*sqrt((2/N)*sum((a-b)^2/(a+b)^2)); strict exceedance of explicit cv_limit_pct fails the group's precision policy; excluded pairs do not enter N","output":"Per-group statistics and eligibility, every raw pair and source line, unavailable groups, actual fitted/bin denominators for Thompson-Howarth and stage-decomposition identifiability by grade band","example":"tests/drill/duplicate_batch.toml","side_effect":"Read-only; stage subtraction is conditional on comparable groups and is not a causal nugget attribution"}"#;

fn stage(token: &str) -> Result<DuplicateStage, Refusal> {
    DuplicateStage::ALL
        .iter()
        .copied()
        .find(|stage| stage.code() == token)
        .ok_or_else(|| invalid("duplicate stage must be field, coarse_reject or pulp"))
}

fn sample_type(token: &str) -> Result<SampleType, Refusal> {
    SampleType::ALL
        .iter()
        .copied()
        .find(|kind| kind.code() == token && *kind != SampleType::QcMaterial)
        .ok_or_else(|| invalid("duplicate sample type must name a drilled sample support"))
}

fn precision_json(report: &DuplicatePrecisionReport) -> String {
    let th = report.thompson_howarth.as_ref().map_or_else(|| "null".into(), |th| format!("{{\"n_pairs\":{},\"fitted_pairs\":{},\"unbinned_pairs\":{},\"n_bins\":{},\"intercept_s0\":{},\"slope_k\":{},\"relative_sd_pct_at_median\":{},\"caveat\":{}}}",
        th.n_pairs, th.fitted_pairs, th.unbinned_pairs, th.n_bins, th.intercept_s0, th.slope_k, th.relative_sd_pct_at_median, js(&th.caveat)));
    format!(
        "{{\"input_pairs\":{},\"valid_pairs\":{},\"excluded_censored\":{},\"excluded_zero_pairs\":{},\"mass_mismatches\":{},\"unverified_support_pairs\":{},\"cv_avr_pct\":{},\"relative_variance\":{},\"hard_from_cv_pct\":{},\"ampd_from_cv_pct\":{},\"median_pair_hard_pct\":{},\"p90_pair_hard_pct\":{},\"mean_pair_ampd_pct\":{},\"thompson_howarth\":{th}}}",
        report.input_pairs,
        report.valid_pairs,
        report.excluded_censored,
        report.excluded_zero_pairs,
        report.unfit_mass_mismatch_ids.len(),
        report.unverified_support_pairs,
        report.cv_avr_pct,
        report.relative_variance,
        report.hard_from_cv_pct,
        report.ampd_from_cv_pct,
        report.median_pair_hard_pct,
        report.p90_pair_hard_pct,
        report.mean_pair_ampd_pct
    )
}

pub(super) fn evaluate(text: &str, source: SourceAnchor) -> Result<Envelope, Refusal> {
    let doc = parse_subset(text).map_err(|e| invalid(e.to_string()))?;
    let mut required = COMMON.to_vec();
    required.extend([
        "lab",
        "max_relative_mass_diff",
        "refuse_on_mass_mismatch",
        "cv_limit_pct",
    ]);
    let top = fields(&doc.top, &required, &[])?;
    let metadata = common(&top, "fmine.duplicate-batch.v1")?;
    if !(1..=MAX_ROWS).contains(&doc.rows.len()) {
        return Err(invalid("provide 1..4096 duplicate pairs"));
    }
    let policy = DuplicatePolicy {
        max_relative_mass_diff: number(top["max_relative_mass_diff"])?,
        refuse_on_mass_mismatch: match top["refuse_on_mass_mismatch"] {
            "true" => true,
            "false" => false,
            _ => {
                return Err(invalid(
                    "refuse_on_mass_mismatch must be quoted true or false",
                ));
            }
        },
    };
    let cv_limit = number(top["cv_limit_pct"])?;
    if cv_limit < 0.0 {
        return Err(invalid("cv_limit_pct must be nonnegative"));
    }
    let mut pairs = Vec::new();
    let mut rows = Vec::new();
    let mut groups: BTreeMap<(&str, DuplicateStage), Vec<usize>> = BTreeMap::new();
    let (mut duplicate_ids, mut originals) = (BTreeSet::new(), BTreeSet::new());
    for (index, row) in doc.rows.iter().enumerate() {
        if row.table != "pair" {
            return Err(invalid(format!("unknown table {}", row.table)));
        }
        let fields = fields(
            &row.fields,
            &[
                "original_sample_id",
                "duplicate_sample_id",
                "stage",
                "grade_band",
                "original",
                "duplicate",
                "original_mass_g",
                "duplicate_mass_g",
                "original_sample_type",
                "duplicate_sample_type",
            ],
            &[],
        )?;
        let (original_id, duplicate_id) =
            (fields["original_sample_id"], fields["duplicate_sample_id"]);
        let stage = stage(fields["stage"])?;
        let band = fields["grade_band"];
        if original_id == duplicate_id
            || original_id.len() > 128
            || duplicate_id.len() > 128
            || !duplicate_ids.insert(duplicate_id)
            || !originals.insert((band, stage, original_id))
        {
            return Err(invalid(
                "IDs must be bounded and distinct; duplicate IDs are unique and an original occurs once per stage/band",
            ));
        }
        let original_mass = number(fields["original_mass_g"])?;
        let duplicate_mass = number(fields["duplicate_mass_g"])?;
        if original_mass <= 0.0 || duplicate_mass <= 0.0 {
            return Err(invalid("both sample masses must be positive grams"));
        }
        let pair = DuplicatePair {
            original_sample_id: original_id.into(),
            duplicate_sample_id: duplicate_id.into(),
            stage,
            original: assay(fields["original"])?,
            duplicate: assay(fields["duplicate"])?,
            original_sample_type: Some(sample_type(fields["original_sample_type"])?),
            duplicate_sample_type: Some(sample_type(fields["duplicate_sample_type"])?),
            original_mass_g: Some(original_mass),
            duplicate_mass_g: Some(duplicate_mass),
            grade_band: Some(band.into()),
        };
        rows.push(format!("{{\"index\":{index},\"source_line\":{},\"original_sample_id\":{},\"duplicate_sample_id\":{},\"stage\":{},\"grade_band\":{},\"original\":{},\"duplicate\":{},\"original_mass_g\":{original_mass},\"duplicate_mass_g\":{duplicate_mass},\"original_sample_type\":{},\"duplicate_sample_type\":{}}}",
            row.line, js(original_id), js(duplicate_id), js(stage.code()), js(band), assay_json(pair.original), assay_json(pair.duplicate), js(fields["original_sample_type"]), js(fields["duplicate_sample_type"])));
        groups.entry((band, stage)).or_default().push(index);
        pairs.push(pair);
    }
    if pairs
        .iter()
        .any(|pair| duplicate_ids.contains(pair.original_sample_id.as_str()))
    {
        return Err(invalid(
            "an original sample ID cannot also be a duplicate sample ID in this batch",
        ));
    }
    let mut markdown = heading("Duplicate precision assessment", &top);
    let _ = write!(
        markdown,
        "Lab **{}**. {} input pairs; CV_AVR policy limit {}%.\n\n| Grade band | Stage | Supplied | Used | CV_AVR % | Precision policy | Excluded censored | Excluded zero | Mass/support mismatches |\n|---|---|---:|---:|---:|---|---:|---:|---:|\n",
        worksheet_input::markdown_cell(top["lab"]),
        pairs.len(),
        cv_limit
    );
    let mut results = Vec::new();
    let mut group_json = Vec::new();
    let mut all_valid = 0;
    for ((band, stage), indices) in &groups {
        let selected = indices
            .iter()
            .map(|index| pairs[*index].clone())
            .collect::<Vec<_>>();
        let indices_json = indices
            .iter()
            .map(usize::to_string)
            .collect::<Vec<_>>()
            .join(",");
        let result = evaluate_duplicates(&selected, &policy);
        let prefix = format!(
            "\"grade_band\":{},\"stage\":{},\"source_indices\":[{indices_json}]",
            js(band),
            js(stage.code())
        );
        match &result {
            Ok(report) => {
                all_valid += report.valid_pairs;
                let passed = report.cv_avr_pct <= cv_limit;
                let dispositions = report.pairs.iter().map(|pair| {
                    let state = match pair.status { DuplicatePairStatus::Included => "included", DuplicatePairStatus::Censored => "excluded_censored", DuplicatePairStatus::BothZero => "excluded_both_zero", DuplicatePairStatus::MassSupportMismatch => "excluded_mass_support_mismatch" };
                    format!("{{\"source_index\":{},\"status\":{},\"mass_support_verified\":{},\"pair_hard_pct\":{},\"detail\":{}}}", indices[pair.index], js(state), pair.mass_support_verified, optional_number(pair.pair_hard_pct), pair.detail.as_deref().map_or_else(|| "null".into(), js))
                }).collect::<Vec<_>>().join(",");
                group_json.push(format!("{{{prefix},\"status\":\"scored\",\"precision_policy_passed\":{passed},\"statistics\":{},\"pairs\":[{dispositions}]}}", precision_json(report)));
                let _ = writeln!(
                    markdown,
                    "| {} | {} | {} | {} | {} | {} | {} | {} | {} |",
                    worksheet_input::markdown_cell(band),
                    stage.code(),
                    report.input_pairs,
                    report.valid_pairs,
                    report.cv_avr_pct,
                    if passed { "pass" } else { "failure" },
                    report.excluded_censored,
                    report.excluded_zero_pairs,
                    report.unfit_mass_mismatch_ids.len()
                );
            }
            Err(error @ QaqcError::InsufficientData { .. }) => {
                group_json.push(format!("{{{prefix},\"status\":\"unavailable\",\"precision_policy_passed\":null,\"statistics\":null,\"reason\":{}}}", js(&error.to_string())));
                let _ = writeln!(
                    markdown,
                    "| {} | {} | {} | 0 | unavailable | not evaluated | — | — | — |",
                    worksheet_input::markdown_cell(band),
                    stage.code(),
                    selected.len()
                );
            }
            Err(error) => return Err(error.to_refusal()),
        }
        results.push(result);
    }
    let mut by_band: BTreeMap<&str, Vec<DuplicatePair>> = BTreeMap::new();
    for ((band, _), indices) in &groups {
        by_band
            .entry(band)
            .or_default()
            .extend(indices.iter().map(|i| pairs[*i].clone()));
    }
    let mut decomposition = Vec::new();
    for (band, selected) in by_band {
        let stage = decompose_duplicate_stages(&selected, &policy).map_err(|e| e.to_refusal())?;
        let status = match stage.status {
            StageDecompositionStatus::Identified => "identified",
            StageDecompositionStatus::FieldDuplicateAbsent => "field_duplicate_absent",
            StageDecompositionStatus::IncompleteStages => "incomplete_stages",
            StageDecompositionStatus::NonMonotoneStageVariance => "nonmonotone_stage_variance",
        };
        let unavailable = stage
            .unavailable_stages
            .iter()
            .map(|stage| js(stage.code()))
            .collect::<Vec<_>>()
            .join(",");
        decomposition.push(format!("{{\"grade_band\":{},\"status\":{},\"unavailable_supplied_stages\":[{unavailable}],\"analytical_relative_variance\":{},\"preparation_incremental_relative_variance\":{},\"field_incremental_relative_variance\":{}}}", js(band), js(status), optional_number(stage.analytical_rel_var), optional_number(stage.prep_incremental_rel_var), optional_number(stage.field_incremental_rel_var)));
        let _ = writeln!(
            markdown,
            "\nStage decomposition for **{}**: {status}. Analytical {}, preparation incremental {}, field incremental {}. Missing stages remain unavailable.",
            worksheet_input::markdown_cell(band),
            optional_number(stage.analytical_rel_var),
            optional_number(stage.prep_incremental_rel_var),
            optional_number(stage.field_incremental_rel_var)
        );
    }
    markdown.push_str("\nCV_AVR uses only eligible uncensored positive-sum pairs. HARD and AMPD derived from CV are distinct from empirical pair summaries. Thompson–Howarth requires at least 50 usable pairs and an identifiable finite fit; its reported fitted count excludes incomplete bins. It assumes normal errors and is optimistic. Stage differences require comparable groups within each grade band; they do not establish a causal geological/sampling nugget split.\n");
    let report = format!(
        "{{\"schema\":\"fmine.duplicate-batch-report.v1\",{metadata},\"lab\":{},\"source\":{},\"policy\":{{\"max_relative_mass_diff\":{},\"refuse_on_mass_mismatch\":{},\"cv_limit_pct\":{cv_limit}}},\"threshold_comparison\":\"strict_binary64_exceedance\",\"input_pairs\":{},\"valid_pairs\":{all_valid},\"pairs\":[{}],\"groups\":[{}],\"stage_decomposition\":[{}]}}",
        js(top["lab"]),
        worksheet_input::source_json(&source, text.len()),
        policy.max_relative_mass_diff,
        policy.refuse_on_mass_mismatch,
        pairs.len(),
        rows.join(","),
        group_json.join(","),
        decomposition.join(",")
    );
    finish(Kind::Duplicates, results, report, markdown, source)
}
