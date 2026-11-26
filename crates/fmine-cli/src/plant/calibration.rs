//! DESIGN (BD-289): read-only admission and presentation of a historical calibration card.
//! Determinism: Exact

use std::fmt::Write as _;
use std::path::Path;

use fmine_core::Digest32;
use fmine_evidence::{Refusal, RefusalCode};
use fmine_plant::cards::calibration::{
    CalibrationCardPolicy, LabVsPlantLot, LotPairStatus, RECOVERY_CALIBRATION_NO_CLAIM,
    RecoveryCalibrationCard, build_recovery_calibration_card, parse_markdown_lab_vs_plant_table,
};
use fmine_registry::toml_subset::{TomlValue, parse_subset};

use crate::{Envelope, json_string as quoted, worksheet_input};

const VERB: &str = "plant recovery-audit";
const SCHEMA: &str = "fmine.recovery-audit.v1";
const MAX_LOTS: usize = 512;
const NO_CLAIM: &str = RECOVERY_CALIBRATION_NO_CLAIM;
pub(super) const ROBOT_DOCS_JSON: &str = r#"{"schema":"fmine.recovery-audit.v1","required":["schema","card_id","site","circuit","source_path","source_sha256","table_marker","source_locator","lot_order","expected_lots"],"source":"Workspace-relative or absolute UTF-8 Markdown path. Pin its SHA-256. table_marker must equal one trimmed line. The following columns are period, lot, lab recovery percent, plant recovery percent and optional printed plant-minus-lab percentage-point difference. Bold the header labels when present. Declare lot_order = source_rows_chronological. The caller supplies chronology, pairing and completeness. expected_lots is the entire table count, 2 through 512, including missing lab results.","policy":"standard_80pct. Warm up on ten prior paired lots. Raw half-width is five percentage points. Corrected intervals use only earlier paired residuals. Counts and exact coverage fractions are descriptive for the supplied table.","scope":"The SHA-256 verifies the submitted extraction. The card does not verify it against a PDF, qualify a forecast or authorize a process change."}"#;

fn invalid(what: impl Into<String>) -> Refusal {
    Refusal::new(RefusalCode::UndeclaredUnitBasisOrTz107, what)
}

pub(super) fn execute(input: &Path, root: &Path) -> Envelope {
    let mut envelope = evaluate(input, root)
        .unwrap_or_else(|refusal| Envelope::refused(VERB, refusal, super::actions()));
    envelope.no_claim = Some(NO_CLAIM);
    envelope
}

fn evaluate(input: &Path, root: &Path) -> Result<Envelope, Refusal> {
    let (worksheet, worksheet_anchor) =
        worksheet_input::read(&root.join(input), "recovery audit worksheet", 32 * 1024)?;
    let doc = parse_subset(&worksheet).map_err(|error| invalid(error.to_string()))?;
    const FIELDS: [&str; 10] = [
        "schema",
        "card_id",
        "site",
        "circuit",
        "source_path",
        "source_sha256",
        "table_marker",
        "source_locator",
        "lot_order",
        "expected_lots",
    ];
    if !doc.rows.is_empty() || doc.top.iter().any(|f| !FIELDS.contains(&f.key.as_str())) {
        return Err(invalid(
            "recovery audit accepts only its registered top-level fields",
        ));
    }
    let string = |key: &str| match doc.top_get(key).map(|field| &field.value) {
        Some(TomlValue::Str(value)) if !value.is_empty() && value.trim() == value => {
            Ok(value.as_str())
        }
        _ => Err(invalid(format!("{key} must be a nonempty trimmed string"))),
    };
    if string("schema")? != SCHEMA || string("lot_order")? != "source_rows_chronological" {
        return Err(invalid(format!(
            "use {SCHEMA} with lot_order = source_rows_chronological"
        )));
    }
    let expected = match doc.top_get("expected_lots").map(|field| &field.value) {
        Some(TomlValue::Int(count)) => usize::try_from(*count)
            .ok()
            .filter(|count| (2..=MAX_LOTS).contains(count)),
        _ => None,
    }
    .ok_or_else(|| {
        invalid(format!(
            "expected_lots must be an integer from 2 through {MAX_LOTS}"
        ))
    })?;
    let declared_digest = string("source_sha256")?;
    if declared_digest.len() != 64
        || !declared_digest
            .bytes()
            .all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
    {
        return Err(invalid(
            "source_sha256 must be 64 lowercase hexadecimal characters",
        ));
    }
    let (source, anchor) = worksheet_input::read(
        &root.join(string("source_path")?),
        "recovery audit source",
        4 * 1024 * 1024,
    )?;
    let digest = Digest32(anchor.artifact_sha256).to_hex();
    if declared_digest != digest {
        return Err(Refusal::new(
            RefusalCode::DigestMismatch505,
            format!(
                "recovery audit source {} has SHA-256 {digest}, expected {declared_digest}",
                anchor.artifact_path,
            ),
        ));
    }
    let marker = string("table_marker")?;
    let mut matches = source
        .split_inclusive('\n')
        .scan(0, |offset, line| {
            let start = *offset;
            *offset += line.len();
            Some((start, line))
        })
        .filter(|(_, line)| line.trim() == marker);
    let Some((start, _)) = matches.next() else {
        return Err(invalid(
            "table_marker must identify one complete source line",
        ));
    };
    if matches.next().is_some() {
        return Err(invalid("table_marker matches more than one source line"));
    }
    let lots = parse_markdown_lab_vs_plant_table(&source[start..], marker)?;
    if lots.len() != expected {
        return Err(invalid(format!(
            "expected {expected} lots but read {} after table_marker",
            lots.len()
        )));
    }
    let policy = CalibrationCardPolicy::standard_80pct()?;
    let card = build_recovery_calibration_card(
        string("card_id")?,
        string("site")?,
        string("circuit")?,
        string("source_locator")?,
        digest,
        &lots,
        &policy,
    )?;
    let mut envelope = Envelope::ok(VERB, String::new(), vec![]);
    envelope.payload_json = format!(
        "{{\"schema\":\"{SCHEMA}\",\"worksheet\":{},\"source\":{},\"table_marker\":{},\"card\":{}}}",
        worksheet_input::source_json(&worksheet_anchor, worksheet.len()),
        worksheet_input::source_json(&anchor, source.len()),
        quoted(marker),
        card_json(&card, &lots),
    );
    envelope.rendered_markdown = Some(markdown(&card, &anchor.artifact_path));
    Ok(envelope)
}

fn card_json(card: &RecoveryCalibrationCard, lots: &[LabVsPlantLot]) -> String {
    let q = super::exact;
    let coverage = |value| {
        if card.scored_online_count == 0 {
            "null".into()
        } else {
            q(value)
        }
    };
    let mut out = format!(
        concat!(
            "{{\"id\":{},\"digest\":\"{}\",\"site\":{},\"circuit\":{},\"source_locator\":{},",
            "\"support\":\"physical_lot\",\"policy\":\"standard_80pct\",\"lot_order\":\"source_rows_chronological\",",
            "\"total_lots\":{},\"paired_lots\":{},\"missing_lab_lots\":{},",
            "\"mean_bias_pct_pts\":{},\"sample_variance_pct_pts_sq\":{},\"sample_sd_pct_pts\":{},",
            "\"min_residual_pct_pts\":{},\"max_residual_pct_pts\":{},\"above_outlier_threshold\":{},",
            "\"below_negative_outlier_threshold\":{},\"printed_diff_rounding_discrepancies\":{},",
            "\"scored_lots\":{},\"raw_lab_covered\":{},\"raw_lab_coverage\":{},\"raw_lab_flag\":\"{}\",",
            "\"bias_corrected_covered\":{},\"bias_corrected_coverage\":{},\"bias_corrected_flag\":\"{}\",\"lots\":["
        ),
        quoted(&card.card_id),
        card.card_digest,
        quoted(&card.site),
        quoted(&card.circuit),
        quoted(&card.source_locator),
        card.total_lots,
        card.paired_count,
        card.missing_lab_count,
        q(card.mean_bias_pct_pts),
        q(card.sample_variance_pct_pts_sq),
        q(card.sample_sd_pct_pts),
        q(card.min_residual_pct_pts),
        q(card.max_residual_pct_pts),
        card.lots_strictly_above_threshold,
        card.lots_strictly_below_neg_threshold,
        card.printed_diff_rounding_discrepancies,
        card.scored_online_count,
        card.raw_lab_covered_count,
        coverage(card.raw_lab_coverage),
        card.raw_lab_flag.as_str(),
        card.bias_corrected_covered_count,
        coverage(card.bias_corrected_coverage),
        card.bias_corrected_flag.as_str(),
    );
    for (index, lot) in lots.iter().enumerate() {
        let evidence = match &lot.status {
            LotPairStatus::Paired {
                lab_recovery_pct,
                plant_recovery_pct,
                printed_diff_pct_pts,
            } => format!(
                "\"lab_recovery_pct\":{},\"plant_recovery_pct\":{},\"printed_diff_pct_pts\":{}",
                q(*lab_recovery_pct),
                q(*plant_recovery_pct),
                printed_diff_pct_pts.map_or_else(|| "null".into(), q),
            ),
            LotPairStatus::MissingLabResult {
                plant_recovery_pct,
                reason,
            } => format!(
                "\"lab_recovery_pct\":null,\"plant_recovery_pct\":{},\"missing_lab_reason\":{}",
                plant_recovery_pct.map_or_else(|| "null".into(), q),
                quoted(reason),
            ),
        };
        let _ = write!(
            out,
            "{}{{\"id\":{},\"period\":{},{evidence}}}",
            if index == 0 { "" } else { "," },
            quoted(&lot.lot_id),
            quoted(&lot.period_label)
        );
    }
    out.push_str("],\"online_steps\":[");
    for (index, step) in card.online_steps.iter().enumerate() {
        let _ = write!(
            out,
            concat!(
                "{}{{\"lot\":{},\"residual_pct_pts\":{},\"prior_mean_bias_pct_pts\":{},",
                "\"raw_lab_covered\":{},\"bias_corrected_covered\":{}}}"
            ),
            if index == 0 { "" } else { "," },
            quoted(&step.lot_id),
            q(step.residual_pct_pts),
            q(step.prior_mean_bias_pct_pts),
            step.raw_lab_covered,
            step.bias_corrected_covered
        );
    }
    out.push_str("]}");
    out
}

fn markdown(card: &RecoveryCalibrationCard, source_path: &str) -> String {
    let safe = worksheet_input::markdown_cell;
    format!(
        concat!(
            "# Historical recovery audit\n\n{} · {}\n\n",
            "Source {} · {} · SHA-256 {}\n\n",
            "{} paired lots and {} missing lab results ({} total). Mean plant-minus-lab bias ",
            "{}/{} percentage points. Sample standard deviation {}/{} percentage points.\n\n",
            "| Historical interval | Covered lots | Scored lots | Flag |\n",
            "|---|---:|---:|---|\n| Raw lab | {} | {} | {} |\n",
            "| Earlier-lot bias correction | {} | {} | {} |\n\n{}\n",
        ),
        safe(&card.site),
        safe(&card.circuit),
        safe(source_path),
        safe(&card.source_locator),
        card.source_sha256_hex,
        card.paired_count,
        card.missing_lab_count,
        card.total_lots,
        card.mean_bias_pct_pts.num(),
        card.mean_bias_pct_pts.den(),
        card.sample_sd_pct_pts.num(),
        card.sample_sd_pct_pts.den(),
        card.raw_lab_covered_count,
        card.scored_online_count,
        card.raw_lab_flag.as_str(),
        card.bias_corrected_covered_count,
        card.scored_online_count,
        card.bias_corrected_flag.as_str(),
        NO_CLAIM,
    )
}
