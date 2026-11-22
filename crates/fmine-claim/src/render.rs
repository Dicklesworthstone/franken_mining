//! Claim rendering: `level1`, `markdown`, and FrankenMining rows (`BD-052`, plan §30.5, §7.2 G-EXEC).
//! Determinism: Exact
//!
//! Enforces plan §30.5 rendering rules:
//! - Ranges/representations and evidence class sit next to every number.
//! - G-EXEC suppression (§7.2): renderers display the knowledge state instead of a number unless
//!   `claim.state` is `Known` or `Estimated`.
//! - Every rendered Claim includes clickable `[display](fmine:clm_...)` linkage, evidence class
//!   badge, KK/KU/UU epistemic classification, colour, flip margin, calibration flag, and UU terms.
//! - Executive plain sentences precede the coded analyst FM-row layout, use `"evidence state"`
//!   (never `"safe"`), and every Markdown report carries its anchor digest, certificate digest, and
//!   the mandatory `no_claim` boundary banner (`INV-002`).

use std::fmt::Write as _;

use fmine_evidence::{
    AoaStatus, AssertionSource, Color, EpistemicStatus, EvidenceClass, KnowledgeState, KuKind,
    Refusal,
};

use crate::{Claim, Confidentiality};

/// Mandatory `INV-002` no-claim banner carried on every rendered report (plan §30.5).
pub const NO_CLAIM_BANNER: &str = "This report makes no reserve disclosure claim, no execution authorization, no geotechnical sign-off and no regulatory compliance claim.";

/// Flip-margin summary attached to a decision-relevant Claim (plan §30.5, §9.6).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlipMarginSummary {
    /// Dominant driver that can reverse the conclusion (e.g. `"gold_price"`).
    pub driver: String,
    /// Threshold at which the conclusion flips (e.g. `"$2,900/oz"`).
    pub threshold_text: String,
    /// Current distance from the flip boundary (e.g. `"+$350/oz"`).
    pub margin_text: String,
    /// Plain-sentence executive explanation (e.g. `"the conclusion reverses if gold falls below $2,900/oz"`).
    pub plain_sentence: String,
}

/// Caller-supplied rendering inputs wrapping a canonical [`Claim<T>`].
#[derive(Debug, Clone)]
pub struct ClaimRenderInput<'a, T> {
    /// Canonical Claim to render.
    pub claim: &'a Claim<T>,
    /// Pre-formatted range/interval value text supplied by the typed quantity owner (e.g. `"1,420–1,510"`).
    pub value_range_text: &'a str,
    /// Explicit unit text (e.g. `"$/oz sold (attributable)"`).
    pub unit: &'a str,
    /// Optional plain-sentence calibration track record (e.g. `"inside its 80% range in 15 of 20 quarters"`).
    pub calibration_summary: Option<&'a str>,
    /// Optional flip-margin summary.
    pub flip_margin: Option<&'a FlipMarginSummary>,
}

/// Structured FrankenMining Claim row (`§30.5`) shared by `level1`, `markdown`, and table renderers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FmClaimRow {
    pub claim_id: String,
    pub metric_key: String,
    /// Displayed range/value or G-EXEC state token when numeric display is suppressed.
    pub display_value: String,
    pub unit: String,
    pub suppressed: bool,
    pub evidence_badge: String,
    pub knowledge_state: &'static str,
    pub epistemic_axis: &'static str,
    pub color_label: String,
    pub calibration_flag: &'static str,
    pub calibration_detail: String,
    pub flip_margin_coded: String,
    pub flip_margin_plain: String,
    pub uu_summary: String,
    pub stale: bool,
    pub clickable_link: String,
    pub plain_sentence: String,
    pub anchor_locators: Vec<String>,
}

fn evidence_badge(class: EvidenceClass) -> String {
    match class {
        EvidenceClass::Measured => "MEASURED".to_string(),
        EvidenceClass::Derived => "DERIVED".to_string(),
        EvidenceClass::ModelledCalibrated => "MODELLED_CALIBRATED".to_string(),
        EvidenceClass::ModelledUncalibrated => "MODELLED_UNCALIBRATED".to_string(),
        EvidenceClass::Elicited => "ELICITED".to_string(),
        EvidenceClass::Transferred { aoa } => {
            let tag = match aoa {
                AoaStatus::Inside => "INSIDE_AOA",
                AoaStatus::NotAssessed => "AOA_NOT_ASSESSED",
                AoaStatus::Outside => "OUTSIDE_AOA",
            };
            format!("TRANSFERRED({tag})")
        }
        EvidenceClass::Assumed => "ASSUMED".to_string(),
        EvidenceClass::Assertion { source } => {
            let src = match source {
                AssertionSource::Issuer => "ISSUER",
                AssertionSource::Extraction => "EXTRACTION",
                AssertionSource::Secondary => "SECONDARY",
                AssertionSource::Agent => "AGENT",
            };
            format!("ASSERTION({src})")
        }
    }
}

const fn knowledge_state_label(state: KnowledgeState) -> &'static str {
    match state {
        KnowledgeState::Known => "known",
        KnowledgeState::Estimated => "estimated",
        KnowledgeState::Unknown => "unknown",
        KnowledgeState::Conflicted => "conflicted",
        KnowledgeState::Stale => "stale",
        KnowledgeState::NotObservable => "not_observable",
        KnowledgeState::Redacted => "redacted",
        KnowledgeState::Indeterminate => "indeterminate",
        KnowledgeState::NotApplicable => "not_applicable",
    }
}

const fn epistemic_axis_label(status: EpistemicStatus) -> &'static str {
    match status {
        EpistemicStatus::KnownKnown => "KK",
        EpistemicStatus::KnownUnknown(KuKind::Aleatory) => "KU(aleatory)",
        EpistemicStatus::KnownUnknown(KuKind::Parametric) => "KU(parametric)",
        EpistemicStatus::KnownUnknown(KuKind::Structural { .. }) => "KU(structural)",
        EpistemicStatus::UnmodelledBudget => "UU(budget)",
    }
}

fn color_label(res: &Result<Color, Refusal>) -> String {
    match res {
        Ok(Color::Validated { dataset, .. }) => format!("Validated({dataset})"),
        Ok(Color::Verified { lo, hi }) => format!("Verified([{lo}, {hi}])"),
        Ok(Color::Estimated { estimator, .. }) => format!("Estimated({estimator})"),
        Err(refusal) => format!("Refused({})", refusal.code),
    }
}

const fn calibration_flag<T>(claim: &Claim<T>) -> &'static str {
    if claim.calibration_failed {
        "calibration_failed"
    } else if matches!(claim.evidence, EvidenceClass::ModelledCalibrated) {
        "calibrated"
    } else if matches!(
        claim.evidence,
        EvidenceClass::Measured | EvidenceClass::Derived | EvidenceClass::Assertion { .. }
    ) {
        "not_applicable"
    } else {
        "uncalibrated"
    }
}

fn sanitize_inline(s: &str) -> String {
    s.replace(['\r', '\n'], " ")
}

fn escape_md_cell(s: &str) -> String {
    sanitize_inline(s).replace('|', "\\|")
}

/// Build a checked [`FmClaimRow`] from `input`, enforcing `reader` confidentiality clearance,
/// canonical Claim ID verification, and §7.2 G-EXEC numeric suppression.
pub fn build_fm_row<T>(
    input: &ClaimRenderInput<'_, T>,
    reader: &Confidentiality,
) -> Result<FmClaimRow, Refusal> {
    let claim = input.claim;
    claim.verify_export_clearance(reader)?;
    claim.check_id()?;

    let claim_id = claim.claim_id.to_string();
    let state_label = knowledge_state_label(claim.state);
    let numeric_permitted = matches!(
        claim.state,
        KnowledgeState::Known | KnowledgeState::Estimated
    );
    let (display_value, suppressed) = if numeric_permitted {
        (sanitize_inline(input.value_range_text), false)
    } else {
        (format!("[evidence state: {state_label}]"), true)
    };

    let badge = evidence_badge(claim.evidence);
    let axis = epistemic_axis_label(claim.epistemic);
    let color = color_label(&claim.color());
    let cal_flag = calibration_flag(claim);
    let cal_detail = input
        .calibration_summary
        .map_or_else(|| cal_flag.to_string(), str::to_string);

    let (flip_coded, flip_plain) = match input.flip_margin {
        Some(fm) => (
            format!(
                "{} @ {} (margin {})",
                fm.driver, fm.threshold_text, fm.margin_text
            ),
            fm.plain_sentence.clone(),
        ),
        None => (
            "none".to_string(),
            "no flip margin computed for this claim".to_string(),
        ),
    };

    let uu_summary = if claim.uu_terms.is_empty() {
        "none".to_string()
    } else {
        let mut parts = Vec::with_capacity(claim.uu_terms.len());
        for uu in &claim.uu_terms {
            parts.push(format!(
                "{} (${:.1}M; trigger: {})",
                uu.scenario_id, uu.downside_exposure_usd_m, uu.trigger_indicator
            ));
        }
        parts.join("; ")
    };

    let clickable_link = if suppressed {
        format!("[{display_value}](fmine:{claim_id})")
    } else {
        format!("[{} {}](fmine:{claim_id})", display_value, input.unit)
    };

    let stale_note = if claim.stale {
        " Warning: upstream dependency superseded (stale)."
    } else {
        ""
    };

    let plain_sentence = if suppressed {
        format!(
            "{} is currently in evidence state '{}' ({}, {}); numeric value is suppressed. Calibration: {}. Flip margin: {}.{}",
            claim.metric_key, state_label, badge, axis, cal_detail, flip_plain, stale_note
        )
    } else {
        format!(
            "{} is {} {} with evidence state '{}' ({}, {}, colour {}). Calibration: {}. Flip margin: {}.{}",
            claim.metric_key,
            display_value,
            input.unit,
            state_label,
            badge,
            axis,
            color,
            cal_detail,
            flip_plain,
            stale_note
        )
    };

    let anchor_locators = claim
        .anchors
        .iter()
        .map(|a| format!("{}:{}", a.artifact_path, a.locator))
        .collect();

    Ok(FmClaimRow {
        claim_id,
        metric_key: claim.metric_key.clone(),
        display_value,
        unit: input.unit.to_string(),
        suppressed,
        evidence_badge: badge,
        knowledge_state: state_label,
        epistemic_axis: axis,
        color_label: color,
        calibration_flag: cal_flag,
        calibration_detail: cal_detail,
        flip_margin_coded: flip_coded,
        flip_margin_plain: flip_plain,
        uu_summary,
        stale: claim.stale,
        clickable_link,
        plain_sentence,
        anchor_locators,
    })
}

/// Level-1 compact single-line Claim rendering (`BD-052`, plan §30.5).
#[must_use]
pub fn render_level1(row: &FmClaimRow) -> String {
    let val_part = if row.suppressed {
        row.display_value.clone()
    } else {
        format!("{} {}", row.display_value, row.unit)
    };
    let stale_tag = if row.stale { " | STALE" } else { "" };
    format!(
        "[{} | {} | {} | {} | {} | cal={} | flip={} | {}]{}",
        row.claim_id,
        row.metric_key,
        val_part,
        row.evidence_badge,
        row.epistemic_axis,
        row.calibration_flag,
        row.flip_margin_coded,
        row.color_label,
        stale_tag
    )
}

/// Deterministic Markdown rendering for a single Claim (`BD-052`, plan §30.5):
/// plain-sentence executive view first, coded analyst FM rows second, anchor/cert/no-claim footer.
#[must_use]
pub fn render_markdown(row: &FmClaimRow, anchor_digest: &str, cert_digest: &str) -> String {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "### {} ({})",
        sanitize_inline(&row.metric_key),
        sanitize_inline(&row.clickable_link)
    );
    let _ = writeln!(out);
    let _ = writeln!(
        out,
        "**Executive summary:** {}",
        sanitize_inline(&row.plain_sentence)
    );
    let _ = writeln!(out);
    let _ = writeln!(out, "| Field | Value |");
    let _ = writeln!(out, "|---|---|");
    let _ = writeln!(out, "| Claim | {} |", escape_md_cell(&row.clickable_link));
    let _ = writeln!(
        out,
        "| Evidence class | `{}` |",
        escape_md_cell(&row.evidence_badge)
    );
    let state_cell = if row.stale {
        format!("{} (STALE)", row.knowledge_state)
    } else {
        row.knowledge_state.to_string()
    };
    let _ = writeln!(out, "| Evidence state | `{state_cell}` |");
    let _ = writeln!(out, "| Epistemic (KK/KU/UU) | `{}` |", row.epistemic_axis);
    let _ = writeln!(out, "| Colour | `{}` |", escape_md_cell(&row.color_label));
    let _ = writeln!(
        out,
        "| Calibration | `{}` ({}) |",
        row.calibration_flag,
        escape_md_cell(&row.calibration_detail)
    );
    let _ = writeln!(
        out,
        "| Flip margin | {} |",
        escape_md_cell(&row.flip_margin_coded)
    );
    let _ = writeln!(out, "| UU terms | {} |", escape_md_cell(&row.uu_summary));
    let sources = if row.anchor_locators.is_empty() {
        "none".to_string()
    } else {
        row.anchor_locators.join(", ")
    };
    let _ = writeln!(out, "| Sources | {} |", escape_md_cell(&sources));
    let _ = writeln!(out);
    let _ = writeln!(
        out,
        "Anchor: `{anchor_digest}` | Certificate: `{cert_digest}`"
    );
    let _ = writeln!(out, "> {NO_CLAIM_BANNER}");
    out
}

/// Deterministic multi-row Markdown table with all FrankenMining rows (`BD-052`, plan §30.5).
#[must_use]
pub fn render_fm_table(rows: &[FmClaimRow], anchor_digest: &str, cert_digest: &str) -> String {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "| Metric | Value | Evidence | State | KK/KU/UU | Colour | Calibration | Flip Margin | UU Terms |"
    );
    let _ = writeln!(out, "|---|---|---|---|---|---|---|---|---|");
    for row in rows {
        let state_cell = if row.stale {
            format!("{} (STALE)", row.knowledge_state)
        } else {
            row.knowledge_state.to_string()
        };
        let _ = writeln!(
            out,
            "| {} | {} | `{}` | `{}` | `{}` | `{}` | `{}` | {} | {} |",
            escape_md_cell(&row.metric_key),
            escape_md_cell(&row.clickable_link),
            escape_md_cell(&row.evidence_badge),
            state_cell,
            row.epistemic_axis,
            escape_md_cell(&row.color_label),
            row.calibration_flag,
            escape_md_cell(&row.flip_margin_coded),
            escape_md_cell(&row.uu_summary),
        );
    }
    let _ = writeln!(out);
    let _ = writeln!(
        out,
        "Anchor: `{anchor_digest}` | Certificate: `{cert_digest}`"
    );
    let _ = writeln!(out, "> {NO_CLAIM_BANNER}");
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use fmine_core::anchor::SourceAnchor;
    use fmine_core::hash::sha256;
    use fmine_evidence::color::ValidityDomain;

    use crate::{AdditivityClass, LabelRegistry, Representation, SpatialSupport, UuTerm};

    #[test]
    fn test_claim_rendering_level1_markdown_and_fm_rows_golden() {
        // 1. Measured + Validated KK Claim.
        let anchor = SourceAnchor::new(
            "corpus/Barrick_Q2_2026.xlsx",
            sha256(b"wb-q2-2026").0,
            "Gold AISC!L14",
        );
        let measured = Claim::new(
            "barrick.ngm.aisc_2026q2",
            1450.0_f64,
            "1450",
            Representation::Interval {
                lo: 1445.0,
                hi: 1455.0,
            },
            SpatialSupport::PeriodAggregate,
            AdditivityClass::IntensiveWeighted,
            EvidenceClass::Measured,
            EpistemicStatus::KnownKnown,
            KnowledgeState::Known,
            Confidentiality::public(),
            Vec::new(),
            Vec::new(),
            vec![anchor.clone()],
        )
        .with_regime(ValidityDomain::unconstrained(), "qaqc-2026q2");

        let row1 = build_fm_row(
            &ClaimRenderInput {
                claim: &measured,
                value_range_text: "1,445–1,455",
                unit: "$/oz sold (61.5% attributable)",
                calibration_summary: None,
                flip_margin: None,
            },
            &Confidentiality::public(),
        )
        .expect("render measured row");

        let expected_l1 = format!(
            "[{} | barrick.ngm.aisc_2026q2 | 1,445–1,455 $/oz sold (61.5% attributable) | MEASURED | KK | cal=not_applicable | flip=none | Validated(qaqc-2026q2)]",
            measured.claim_id
        );
        assert_eq!(render_level1(&row1), expected_l1);

        // 2. ModelledUncalibrated KU+UU Claim with flip margin & calibration track record.
        let modeled = Claim::new(
            "lumwana.superpit.npv_delta",
            420.0_f64,
            "420",
            Representation::Quantiles {
                p10: 180.0,
                p50: 420.0,
                p90: 690.0,
            },
            SpatialSupport::PeriodAggregate,
            AdditivityClass::ExtensiveAdditive,
            EvidenceClass::ModelledUncalibrated,
            EpistemicStatus::KnownUnknown(KuKind::Parametric),
            KnowledgeState::Estimated,
            Confidentiality::public(),
            vec![UuTerm {
                scenario_id: "UU-ZAMBIA-POWER".to_string(),
                description: "Hydro drought grid curtailment".to_string(),
                downside_exposure_usd_m: 85.0,
                trigger_indicator: "Kariba dam level < 15%".to_string(),
            }],
            Vec::new(),
            vec![anchor.clone()],
        );
        let flip = FlipMarginSummary {
            driver: "copper_price".to_string(),
            threshold_text: "$3.40/lb".to_string(),
            margin_text: "+$0.75/lb".to_string(),
            plain_sentence: "the conclusion reverses if copper falls below $3.40/lb".to_string(),
        };
        let row2 = build_fm_row(
            &ClaimRenderInput {
                claim: &modeled,
                value_range_text: "P10=$180M / P50=$420M / P90=$690M",
                unit: "USD_M (100% project)",
                calibration_summary: Some("inside its 80% range in 15 of 20 quarters"),
                flip_margin: Some(&flip),
            },
            &Confidentiality::public(),
        )
        .expect("render modeled row");

        let md = render_markdown(&row2, "anc_deadbeef", "cert_cafebabe");
        let expected_md = format!(
            "### lumwana.superpit.npv_delta ([P10=$180M / P50=$420M / P90=$690M USD_M (100% project)](fmine:{id}))\n\
            \n\
            **Executive summary:** lumwana.superpit.npv_delta is P10=$180M / P50=$420M / P90=$690M USD_M (100% project) with evidence state 'estimated' (MODELLED_UNCALIBRATED, KU(parametric), colour Estimated(class:unanchored)). Calibration: inside its 80% range in 15 of 20 quarters. Flip margin: the conclusion reverses if copper falls below $3.40/lb.\n\
            \n\
            | Field | Value |\n\
            |---|---|\n\
            | Claim | [P10=$180M / P50=$420M / P90=$690M USD_M (100% project)](fmine:{id}) |\n\
            | Evidence class | `MODELLED_UNCALIBRATED` |\n\
            | Evidence state | `estimated` |\n\
            | Epistemic (KK/KU/UU) | `KU(parametric)` |\n\
            | Colour | `Estimated(class:unanchored)` |\n\
            | Calibration | `uncalibrated` (inside its 80% range in 15 of 20 quarters) |\n\
            | Flip margin | copper_price @ $3.40/lb (margin +$0.75/lb) |\n\
            | UU terms | UU-ZAMBIA-POWER ($85.0M; trigger: Kariba dam level < 15%) |\n\
            | Sources | corpus/Barrick_Q2_2026.xlsx:Gold AISC!L14 |\n\
            \n\
            Anchor: `anc_deadbeef` | Certificate: `cert_cafebabe`\n\
            > {NO_CLAIM_BANNER}\n",
            id = modeled.claim_id
        );
        assert_eq!(md, expected_md);
        assert!(!md.to_ascii_lowercase().contains("safe"));

        // 3. Multi-row FM table contains both rows + anchor/cert/no-claim footer.
        let table = render_fm_table(&[row1, row2], "anc_deadbeef", "cert_cafebabe");
        assert!(table.contains("| Metric | Value | Evidence | State | KK/KU/UU | Colour | Calibration | Flip Margin | UU Terms |"));
        assert!(table.contains("barrick.ngm.aisc_2026q2"));
        assert!(table.contains("lumwana.superpit.npv_delta"));
        assert!(table.contains(NO_CLAIM_BANNER));

        // 4. G-EXEC numeric suppression (§7.2): Unknown state suppresses the numeric value.
        let unknown = Claim::new(
            "tsf.piezometer.margin",
            9999.0_f64,
            "9999",
            Representation::Deterministic,
            SpatialSupport::PointCore,
            AdditivityClass::NonAdditive,
            EvidenceClass::Assumed,
            EpistemicStatus::UnmodelledBudget,
            KnowledgeState::Unknown,
            Confidentiality::public(),
            Vec::new(),
            Vec::new(),
            vec![anchor.clone()],
        );
        let row_unk = build_fm_row(
            &ClaimRenderInput {
                claim: &unknown,
                value_range_text: "9999.0",
                unit: "kPa",
                calibration_summary: None,
                flip_margin: None,
            },
            &Confidentiality::public(),
        )
        .expect("render unknown row");
        assert!(row_unk.suppressed);
        assert_eq!(row_unk.display_value, "[evidence state: unknown]");
        assert!(!render_level1(&row_unk).contains("9999.0"));
        assert!(!render_markdown(&row_unk, "anc_1", "cert_1").contains("9999.0"));

        // 5. Confidentiality wall guard: walled claim refused to a public reader.
        let reg = LabelRegistry::new([], ["W-mnpi-q3"]).expect("register wall");
        let wall = reg.wall("W-mnpi-q3").expect("lookup wall");
        let walled = Claim::new(
            "secret.assay",
            12.5_f64,
            "12.5",
            Representation::Deterministic,
            SpatialSupport::PointCore,
            AdditivityClass::IntensiveWeighted,
            EvidenceClass::Measured,
            EpistemicStatus::KnownKnown,
            KnowledgeState::Known,
            Confidentiality::mnpi(wall),
            Vec::new(),
            Vec::new(),
            vec![anchor],
        );
        let err = build_fm_row(
            &ClaimRenderInput {
                claim: &walled,
                value_range_text: "12.5",
                unit: "g/t",
                calibration_summary: None,
                flip_margin: None,
            },
            &Confidentiality::public(),
        )
        .unwrap_err();
        assert_eq!(err.code, "FMINE-REF-301");

        // 6. Stale claim displays "(STALE)" in both single-claim Markdown and multi-row FM table,
        //    and CRLF in text cells is escaped without splitting Markdown rows.
        let mut stale_claim = measured.clone();
        stale_claim.stale = true;
        let stale_row = build_fm_row(
            &ClaimRenderInput {
                claim: &stale_claim,
                value_range_text: "1450\r\n1460",
                unit: "$/oz",
                calibration_summary: None,
                flip_margin: None,
            },
            &Confidentiality::public(),
        )
        .expect("render stale row");
        let stale_md = render_markdown(&stale_row, "anc_s", "cert_s");
        assert!(stale_md.contains("`known (STALE)`"), "{stale_md}");
        assert!(!stale_md.contains('\r'));
        let stale_tbl = render_fm_table(&[stale_row], "anc_s", "cert_s");
        assert!(stale_tbl.contains("`known (STALE)`"), "{stale_tbl}");
        assert!(!stale_tbl.contains('\r'));
    }
}
