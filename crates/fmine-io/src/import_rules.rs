//! Determinism: DeterministicPerIsa
//!
//! Corpus-driven import integrity rules (`BD-291`, plan §25.2/§25.5/§32.1):
//! - `G-UNIT-5` (`E346`, `E347`): document-quantity replay catches dimension mismatches (e.g. an
//!   area printed in `m³`) and conflicting same-document hours/fuel/GHG derivations as structured
//!   discrepancies rather than trusting printed numbers.
//! - `G-CRS-3` (`E314`): coordinates with no stated CRS recovered by fit are returned as a
//!   `Claim<CrsFitResult>` with residual (`ModelledUncalibrated`, never silently assigned as `Measured`).
//! - Asserted column basis (`E240`): checks `"Contained"` vs `"Recovered"` metal identity against
//!   `dry_tonnes * grade * recovery` (e.g. Kibali `"Contained Gold"` holding recovered ounces).
//! - `DocApproval` (`RISK-29`, `E304`): every [`DocLocator`] carries an approval state; cells from
//!   `Draft` revisions cap at [`KnowledgeState::Conflicted`] and `Superseded` at [`KnowledgeState::Stale`].

use fmine_claim::{AdditivityClass, Claim, Confidentiality, Representation, SpatialSupport};
use fmine_core::anchor::SourceAnchor;
use fmine_core::hash::sha256;
use fmine_evidence::{
    EpistemicStatus, EvidenceClass, KnowledgeState, KuKind, Refusal, RefusalCode,
};

use crate::dialects::ColumnBasis;

/// Troy ounce mass in grams (exact international troy ounce: 31.1034768 g/oz).
pub const GRAMS_PER_TROY_OZ: f64 = 31.103_476_8;

/// Approval state of a source document revision (`RISK-29`, `E304`, plan §32.1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DocApproval {
    /// Unapproved draft revision — caps downstream [`KnowledgeState`] at `Conflicted`.
    Draft { rev: String },
    /// Formally approved revision with sign-off stamp.
    Approved { stamp: String },
    /// Superseded by a newer revision — caps downstream [`KnowledgeState`] at `Stale`.
    Superseded { by: String },
}

/// Source document locator carrying mandatory [`DocApproval`] (`BD-291`, `RISK-29`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocLocator {
    pub file: String,
    pub sha256: String,
    pub locator: String,
    pub approval: DocApproval,
}

impl DocLocator {
    /// Caps a cell's [`KnowledgeState`] according to its document approval state (`RISK-29`):
    /// - `Draft` never exceeds `Conflicted` (never `Known` or `Estimated`);
    /// - `Superseded` never exceeds `Stale`;
    /// - `Approved` preserves the requested state unchanged.
    #[must_use]
    pub const fn cap_knowledge_state(&self, requested: KnowledgeState) -> KnowledgeState {
        match self.approval {
            DocApproval::Approved { .. } => requested,
            DocApproval::Draft { .. } => match requested {
                KnowledgeState::Known | KnowledgeState::Estimated => KnowledgeState::Conflicted,
                other => other,
            },
            DocApproval::Superseded { .. } => match requested {
                KnowledgeState::Known | KnowledgeState::Estimated => KnowledgeState::Stale,
                other => other,
            },
        }
    }
}

/// Structured discrepancy filed on import when a document fails replay, basis, or CRS rules (`BD-291`).
#[derive(Debug, Clone, PartialEq)]
pub struct ImportDiscrepancy {
    pub rule_id: &'static str,
    pub evidence_ref: &'static str,
    pub locator: DocLocator,
    pub refusal: Refusal,
    pub expected_summary: String,
    pub reported_summary: String,
}

/// Physical dimension tag checked during `G-UNIT-5` document-quantity replay (`E347`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuantityDimension {
    AreaL2,
    VolumeL3,
    Mass,
    Duration,
    Energy,
}

/// Checks `G-UNIT-5` dimension replay (`E347`): a quantity labelled as `expected_dim` (e.g. disturbed
/// area `AreaL2`) whose printed unit carries a conflicting dimension (e.g. `m3` / `VolumeL3`) files
/// a structured [`ImportDiscrepancy`] instead of being admitted silently.
#[must_use]
pub fn check_dimension_replay_g_unit_5(
    locator: &DocLocator,
    quantity_label: &str,
    expected_dim: QuantityDimension,
    printed_unit: &str,
    printed_dim: QuantityDimension,
) -> Option<ImportDiscrepancy> {
    if expected_dim == printed_dim {
        return None;
    }
    Some(ImportDiscrepancy {
        rule_id: "G-UNIT-5",
        evidence_ref: "E347",
        locator: locator.clone(),
        refusal: Refusal::new(
            RefusalCode::BasisMismatch105,
            format!(
                "G-UNIT-5 dimension mismatch for '{quantity_label}': expected {expected_dim:?}, document prints unit '{printed_unit}' ({printed_dim:?})"
            ),
        ),
        expected_summary: format!("{expected_dim:?}"),
        reported_summary: format!("{printed_unit} ({printed_dim:?})"),
    })
}

/// Checks `G-UNIT-5` same-document derivation replay (`E346`): given a document's stated operating
/// hours `stated_hours`, fuel burn rate `fuel_rate_per_hr`, and reported fuel consumption
/// `reported_fuel`, files a discrepancy when implied hours conflict beyond `rel_tol`.
#[must_use]
pub fn check_hours_fuel_replay_g_unit_5(
    locator: &DocLocator,
    stated_hours: f64,
    fuel_rate_per_hr: f64,
    reported_fuel: f64,
    rel_tol: f64,
) -> Option<ImportDiscrepancy> {
    if !stated_hours.is_finite()
        || !fuel_rate_per_hr.is_finite()
        || !reported_fuel.is_finite()
        || stated_hours <= 0.0
        || fuel_rate_per_hr <= 0.0
    {
        return Some(ImportDiscrepancy {
            rule_id: "G-UNIT-5",
            evidence_ref: "E346",
            locator: locator.clone(),
            refusal: Refusal::new(
                RefusalCode::OutsidePhysicalDomain109,
                "G-UNIT-5 non-positive or non-finite hours/fuel input".to_string(),
            ),
            expected_summary: "positive finite hours and fuel_rate_per_hr".to_string(),
            reported_summary: format!(
                "hours={stated_hours}, rate={fuel_rate_per_hr}, fuel={reported_fuel}"
            ),
        });
    }
    let expected_fuel = stated_hours * fuel_rate_per_hr;
    let implied_hours = reported_fuel / fuel_rate_per_hr;
    let rel_err = (reported_fuel - expected_fuel).abs() / expected_fuel.abs();
    if rel_err <= rel_tol {
        return None;
    }
    Some(ImportDiscrepancy {
        rule_id: "G-UNIT-5",
        evidence_ref: "E346",
        locator: locator.clone(),
        refusal: Refusal::new(
            RefusalCode::Conservation101,
            format!(
                "G-UNIT-5 hours/fuel conflict: stated {stated_hours:.1} h implies {expected_fuel:.1}, reported {reported_fuel:.1} implies {implied_hours:.1} h (rel_err={rel_err:.4})"
            ),
        ),
        expected_summary: format!("{expected_fuel:.2} (from {stated_hours:.1} h)"),
        reported_summary: format!("{reported_fuel:.2} (implies {implied_hours:.1} h)"),
    })
}

/// Checks asserted column metal basis (`E240`, plan §32.1): verifies whether a column asserting
/// [`ColumnBasis::Contained`] (e.g. Kibali `"Contained Gold"`) actually equals contained ounces
/// (`dry_tonnes * grade_g_per_t / 31.1034768`) or recovered ounces (`contained * recovery_frac`).
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn check_asserted_metal_basis_e240(
    locator: &DocLocator,
    column_header: &str,
    asserted_basis: ColumnBasis,
    dry_tonnes: f64,
    grade_g_per_t: f64,
    recovery_frac: f64,
    reported_oz: f64,
    rel_tol: f64,
) -> Option<ImportDiscrepancy> {
    let contained_oz = dry_tonnes * grade_g_per_t / GRAMS_PER_TROY_OZ;
    let recovered_oz = contained_oz * recovery_frac;
    let expected_oz = match asserted_basis {
        ColumnBasis::Contained => contained_oz,
        ColumnBasis::Recovered => recovered_oz,
        _ => return None,
    };
    let rel_err = (reported_oz - expected_oz).abs() / expected_oz.abs().max(1e-12);
    if rel_err <= rel_tol {
        return None;
    }
    let alt_basis = if asserted_basis == ColumnBasis::Contained {
        let rec_err = (reported_oz - recovered_oz).abs() / recovered_oz.abs().max(1e-12);
        if rec_err <= rel_tol {
            "matches Recovered basis (tonnes * grade * recovery)"
        } else {
            "matches neither Contained nor Recovered basis"
        }
    } else {
        "does not match Recovered basis"
    };
    Some(ImportDiscrepancy {
        rule_id: "ASSERTED-BASIS",
        evidence_ref: "E240",
        locator: locator.clone(),
        refusal: Refusal::new(
            RefusalCode::BasisMismatch105,
            format!(
                "column '{column_header}' asserts {asserted_basis:?} ({expected_oz:.1} oz), but reported {reported_oz:.1} oz {alt_basis}"
            ),
        ),
        expected_summary: format!("{expected_oz:.1} oz ({asserted_basis:?})"),
        reported_summary: format!("{reported_oz:.1} oz ({alt_basis})"),
    })
}

/// Fitted 2-D translation result returned inside a `Claim` by [`fit_crs_claim_g_crs_3`] (`E314`).
#[derive(Debug, Clone, PartialEq)]
pub struct CrsFitResult {
    pub candidate_crs: String,
    pub dx_m: f64,
    pub dy_m: f64,
    pub rms_residual_m: f64,
    pub max_residual_m: f64,
    pub control_points: usize,
}

/// Local-grid `(x, y)` paired with target-CRS `(easting, northing)` control monument (`G-CRS-3`).
pub type ControlPair = ((f64, f64), (f64, f64));

/// Recovers an unstated CRS by least-squares translation fit against control monuments (`G-CRS-3`, `E314`).
///
/// The result is ALWAYS wrapped in a [`Claim`] with [`EvidenceClass::ModelledUncalibrated`] (never
/// silently assigned as `Measured`), and if `max_residual_m > max_allowed_residual_m` (or fewer than
/// 2 control pairs exist) also returns an [`ImportDiscrepancy`].
#[must_use]
pub fn fit_crs_claim_g_crs_3(
    locator: &DocLocator,
    candidate_crs: &str,
    control_pairs: &[ControlPair],
    max_allowed_residual_m: f64,
) -> (Claim<CrsFitResult>, Option<ImportDiscrepancy>) {
    let n = control_pairs.len();
    let (dx_m, dy_m, rms_residual_m, max_residual_m) = if n == 0 {
        (0.0, 0.0, f64::INFINITY, f64::INFINITY)
    } else {
        let mut sum_dx = 0.0;
        let mut sum_dy = 0.0;
        for &((lx, ly), (ex, ey)) in control_pairs {
            sum_dx += ex - lx;
            sum_dy += ey - ly;
        }
        let dx = sum_dx / (n as f64);
        let dy = sum_dy / (n as f64);
        let mut sum_sq = 0.0;
        let mut max_r = 0.0_f64;
        for &((lx, ly), (ex, ey)) in control_pairs {
            let rx = (lx + dx) - ex;
            let ry = (ly + dy) - ey;
            let r = rx.hypot(ry);
            sum_sq += r * r;
            max_r = max_r.max(r);
        }
        (dx, dy, (sum_sq / (n as f64)).sqrt(), max_r)
    };

    let within_tol =
        n >= 2 && max_residual_m.is_finite() && max_residual_m <= max_allowed_residual_m;
    let raw_state = if within_tol {
        KnowledgeState::Estimated
    } else {
        KnowledgeState::Conflicted
    };
    let state = locator.cap_knowledge_state(raw_state);

    let fit = CrsFitResult {
        candidate_crs: candidate_crs.to_string(),
        dx_m,
        dy_m,
        rms_residual_m,
        max_residual_m,
        control_points: n,
    };
    let tag =
        format!("crs_fit:{candidate_crs}:n={n}:rms={rms_residual_m:.4}:max={max_residual_m:.4}");
    let artifact_sha256 =
        parse_hex32(&locator.sha256).unwrap_or_else(|| sha256(locator.sha256.as_bytes()).0);
    let anchor = SourceAnchor::new(
        locator.file.clone(),
        artifact_sha256,
        format!("{}#crs_fit:{candidate_crs}", locator.locator),
    );
    let claim = Claim::new(
        "import.crs_fit",
        fit,
        &tag,
        Representation::Deterministic,
        SpatialSupport::MiningPolygon,
        AdditivityClass::NonAdditive,
        EvidenceClass::ModelledUncalibrated,
        EpistemicStatus::KnownUnknown(KuKind::Parametric),
        state,
        Confidentiality::internal(),
        Vec::new(),
        Vec::new(),
        vec![anchor],
    );

    let discrepancy = if within_tol {
        None
    } else {
        Some(ImportDiscrepancy {
            rule_id: "G-CRS-3",
            evidence_ref: "E314",
            locator: locator.clone(),
            refusal: Refusal::new(
                RefusalCode::CrsUnregisteredOrIncompatible106,
                format!(
                    "G-CRS-3 CRS-by-fit to {candidate_crs} failed: n={n}, max_residual={max_residual_m:.3} m > tol {max_allowed_residual_m:.3} m"
                ),
            ),
            expected_summary: format!("n >= 2 and max_residual <= {max_allowed_residual_m:.3} m"),
            reported_summary: format!(
                "n={n}, rms={rms_residual_m:.3} m, max={max_residual_m:.3} m"
            ),
        })
    };

    (claim, discrepancy)
}

fn parse_hex32(hex: &str) -> Option<[u8; 32]> {
    let bytes = hex.as_bytes();
    if bytes.len() != 64 {
        return None;
    }
    let (pairs, rem) = bytes.as_chunks::<2>();
    if !rem.is_empty() {
        return None;
    }
    let mut out = [0u8; 32];
    for (slot, pair) in out.iter_mut().zip(pairs) {
        let hi = hex_nibble(pair[0])?;
        let lo = hex_nibble(pair[1])?;
        *slot = (hi << 4) | lo;
    }
    Some(out)
}

const fn hex_nibble(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc(file: &str, approval: DocApproval) -> DocLocator {
        DocLocator {
            file: file.to_string(),
            sha256: "a".repeat(64),
            locator: "Table 4-2".to_string(),
            approval,
        }
    }

    /// BD-291 done-when: environmental-report (E346/E347), CRS-by-fit (E314), Kibali asserted-basis
    /// (E240), and Draft `DocApproval` (RISK-29) fixtures are filed as structured discrepancies.
    #[test]
    fn test_bd_291_g_unit_5_g_crs_3_kibali_basis_and_doc_approval_discrepancies() {
        // 1. E347: environmental report prints disturbed area in m3 (volume dimension) -> G-UNIT-5 discrepancy.
        let esia_doc = doc(
            "synthetic_env_report.pdf",
            DocApproval::Approved {
                stamp: "QP-2026-01".into(),
            },
        );
        let d_dim = check_dimension_replay_g_unit_5(
            &esia_doc,
            "disturbed_footprint_area",
            QuantityDimension::AreaL2,
            "m3",
            QuantityDimension::VolumeL3,
        )
        .expect("E347 dimension mismatch filed");
        assert_eq!(d_dim.rule_id, "G-UNIT-5");
        assert_eq!(d_dim.evidence_ref, "E347");
        assert_eq!(d_dim.refusal.code, "FMINE-REF-105");

        // 2. E346: environmental report fuel/GHG hours conflict with stated equipment hours -> G-UNIT-5 discrepancy.
        let d_hrs = check_hours_fuel_replay_g_unit_5(
            &esia_doc, 4_000.0,   // stated operating hours
            120.0,     // L/hr
            840_000.0, // implies 7,000 h != 4,000 h!
            0.02,
        )
        .expect("E346 hours conflict filed");
        assert_eq!(d_hrs.rule_id, "G-UNIT-5");
        assert_eq!(d_hrs.evidence_ref, "E346");
        assert_eq!(d_hrs.refusal.code, "FMINE-REF-101");

        // 3. E240: Kibali-style "Contained Gold" column holds recovered ounces (89% recovery) -> discrepancy!
        let kibali_doc = doc(
            "synthetic_kibali_reserves.xlsx",
            DocApproval::Approved {
                stamp: "QP-2026-02".into(),
            },
        );
        let tonnes = 1_000_000.0;
        let grade_gpt = 3.50;
        let recovery = 0.89;
        let recovered_oz = tonnes * grade_gpt * recovery / GRAMS_PER_TROY_OZ;
        let d_basis = check_asserted_metal_basis_e240(
            &kibali_doc,
            "Contained Gold (oz)",
            ColumnBasis::Contained,
            tonnes,
            grade_gpt,
            recovery,
            recovered_oz,
            0.01,
        )
        .expect("E240 basis mismatch filed");
        assert_eq!(d_basis.evidence_ref, "E240");
        assert_eq!(d_basis.refusal.code, "FMINE-REF-105");
        assert!(d_basis.reported_summary.contains("Recovered"));
        // Same figure asserted as Recovered passes cleanly.
        assert!(
            check_asserted_metal_basis_e240(
                &kibali_doc,
                "Recovered Gold (oz)",
                ColumnBasis::Recovered,
                tonnes,
                grade_gpt,
                recovery,
                recovered_oz,
                0.01,
            )
            .is_none()
        );

        // 4. G-CRS-3 (E314) + DocApproval::Draft (RISK-29):
        // Good fit from Approved doc is ModelledUncalibrated + Estimated (never Measured).
        let control_good = [
            ((100.0, 200.0), (500_100.0, 4_000_200.0)),
            ((250.0, 400.0), (500_250.0, 4_000_400.0)),
            ((400.0, 150.0), (500_400.1, 4_000_149.9)),
        ];
        let (claim_ok, disc_ok) =
            fit_crs_claim_g_crs_3(&esia_doc, "EPSG:32641", &control_good, 0.5);
        assert!(disc_ok.is_none());
        assert_eq!(claim_ok.evidence, EvidenceClass::ModelledUncalibrated);
        assert_eq!(claim_ok.state, KnowledgeState::Estimated);

        // Same good fit from a Draft document caps KnowledgeState at Conflicted (RISK-29)!
        let draft_doc = doc(
            "synthetic_draft_survey.pdf",
            DocApproval::Draft {
                rev: "Rev 0.2 DRAFT".into(),
            },
        );
        let (claim_draft, _) = fit_crs_claim_g_crs_3(&draft_doc, "EPSG:32641", &control_good, 0.5);
        assert_eq!(claim_draft.state, KnowledgeState::Conflicted);
        assert_eq!(
            draft_doc.cap_knowledge_state(KnowledgeState::Known),
            KnowledgeState::Conflicted
        );

        // Superseded document caps KnowledgeState at Stale.
        let sup_doc = doc(
            "old_report.pdf",
            DocApproval::Superseded {
                by: "rev-2026".into(),
            },
        );
        assert_eq!(
            sup_doc.cap_knowledge_state(KnowledgeState::Known),
            KnowledgeState::Stale
        );

        // Bad fit (15 m outlier) files a G-CRS-3 discrepancy.
        let control_bad = [
            ((100.0, 200.0), (500_100.0, 4_000_200.0)),
            ((250.0, 400.0), (500_265.0, 4_000_400.0)),
        ];
        let (claim_bad, disc_bad) =
            fit_crs_claim_g_crs_3(&esia_doc, "EPSG:32641", &control_bad, 0.5);
        assert_eq!(claim_bad.state, KnowledgeState::Conflicted);
        let d_crs = disc_bad.expect("G-CRS-3 discrepancy filed");
        assert_eq!(d_crs.rule_id, "G-CRS-3");
        assert_eq!(d_crs.evidence_ref, "E314");
        assert_eq!(d_crs.refusal.code, "FMINE-REF-106");
    }
}
