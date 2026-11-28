//! Technical-report Data-Verification Evidence Index skeleton (`BD-174`, plan §24.5, `G-HALL-1`,
//! `G-HALL-2`, `INV-002`).
//! Determinism: BitwisePortable (canonical `BTreeMap`/`BTreeSet` ordering, explicit denominators,
//! CAS SHA-256 original links, and BLAKE3 digest).

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use fmine_core::codec::Encoder;
use fmine_core::hash::blake3_hash;

use crate::crm::{BiasInterval, CrmBatchEvaluation, SdBasis};
use crate::duplicates::DuplicatePrecisionReport;
use crate::error::QaqcError;
use crate::insertion::{BlankBatchEvaluation, InsertionRateSummary};
use crate::monitors::{AlarmScope, CrmMonitorSnapshot};

/// Default laboratory density check fraction band (`E382`, plan §24.5: 5–10%).
pub const DENSITY_LAB_CHECK_MIN_PCT: f64 = 5.0;
pub const DENSITY_LAB_CHECK_MAX_PCT: f64 = 10.0;

/// Target disclosure code / technical-report standard for the data-verification index skeleton.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum TechnicalReportStandard {
    /// Current Canadian NI 43-101 Form 43-101F1.
    Ni43101Current,
    /// Proposed NI 43-101 replacement requiring per-item QP data verification (`B5 W7`, `G-HALL-2` draft).
    Ni43101Proposed,
    /// SEC Regulation S-K 1300 Technical Report Summary.
    Sk1300,
    /// JORC 2012 Table 1 (Sections 1–3).
    Jorc2012Table1,
}

impl TechnicalReportStandard {
    /// Stable wire/display code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::Ni43101Current => "ni43101_current",
            Self::Ni43101Proposed => "ni43101_proposed",
            Self::Sk1300 => "sk1300",
            Self::Jorc2012Table1 => "jorc2012_table1",
        }
    }

    /// Human title.
    #[must_use]
    pub const fn title(self) -> &'static str {
        match self {
            Self::Ni43101Current => "NI 43-101 (Current Form 43-101F1)",
            Self::Ni43101Proposed => "NI 43-101 (Proposed Replacement — Draft Standard)",
            Self::Sk1300 => "SEC S-K 1300 Technical Report Summary",
            Self::Jorc2012Table1 => "JORC 2012 Table 1",
        }
    }

    /// Guard `G-HALL-2`: standards not yet in force must render `"DRAFT STANDARD"` on every output.
    #[must_use]
    pub const fn is_draft_standard(self) -> bool {
        matches!(self, Self::Ni43101Proposed)
    }

    /// Technical-report items covered by the data-verification skeleton.
    #[must_use]
    pub const fn item_slots(self) -> &'static [(&'static str, &'static str)] {
        match self {
            Self::Ni43101Current | Self::Ni43101Proposed => &[
                (
                    "Item 10",
                    "Drilling — Collar, Survey, and Database Lineage Verification",
                ),
                (
                    "Item 11",
                    "Sample Preparation, Analyses and Security — QA/QC & Chain of Custody",
                ),
                (
                    "Item 12",
                    "Data Verification — Audits, Umpires, Density, Site Visits, Check Sampling & Limitations",
                ),
                (
                    "Item 14",
                    "Mineral Resource Estimates — Input Data Adequacy & Unresolved Verification Gaps",
                ),
            ],
            Self::Sk1300 => &[
                ("Item 7", "Exploration — Drilling & Survey Verification"),
                (
                    "Item 8",
                    "Sample Preparation, Analyses, and Security — QA/QC Performance",
                ),
                (
                    "Item 9",
                    "Data Verification — Database Audit, Umpires, Density, Site Visits & Limitations",
                ),
                (
                    "Item 11",
                    "Mineral Resource Estimates — Verification Support & Unresolved Gaps",
                ),
            ],
            Self::Jorc2012Table1 => &[
                (
                    "Section 1",
                    "Sampling Techniques and Data — Drilling, QA/QC, Density & Database Audit",
                ),
                (
                    "Section 2",
                    "Reporting of Exploration Results — Verification, Site Visits & Check Sampling",
                ),
                (
                    "Section 3",
                    "Estimation and Reporting of Mineral Resources — Data Verification Trail & Gaps",
                ),
            ],
        }
    }
}

/// Kind of signed original artifact stored in the content-addressed store (CAS).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum OriginalArtifactKind {
    LabCertificate,
    SurveyReport,
    DensityReport,
    SiteVisitMemo,
    UmpireCertificate,
    DatabaseExport,
    QaqcPolicy,
}

impl OriginalArtifactKind {
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::LabCertificate => "lab_certificate",
            Self::SurveyReport => "survey_report",
            Self::DensityReport => "density_report",
            Self::SiteVisitMemo => "site_visit_memo",
            Self::UmpireCertificate => "umpire_certificate",
            Self::DatabaseExport => "database_export",
            Self::QaqcPolicy => "qaqc_policy",
        }
    }
}

/// Reference to a signed original artifact pinned by SHA-256 in the CAS (§24.5).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CasOriginalRef {
    /// Local reference identifier cited by evidence sections.
    pub ref_id: String,
    /// Artifact kind.
    pub kind: OriginalArtifactKind,
    /// Logical CAS path or archive locator.
    pub locator: String,
    /// Lowercase 64-hex SHA-256 digest of the original bytes.
    pub sha256_hex: String,
}

/// Corebox drillhole database audit summary (§24.5 bullet 1).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DrillholeDatabaseAudit {
    pub audit_id: String,
    pub holes_audited: usize,
    pub collars_verified: usize,
    pub surveys_verified: usize,
    pub assays_verified: usize,
    pub rejected_rows: usize,
    pub composite_support_mismatches: usize,
    pub original_refs: Vec<String>,
    pub notes: String,
}

/// Historical vs modern QA/QC control regime per campaign (§12, §24.5).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum QaqcRegime {
    ModernFull,
    LegacyPartial,
    HistoricUncontrolled,
    Unverified,
}

impl QaqcRegime {
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::ModernFull => "modern_full",
            Self::LegacyPartial => "legacy_partial",
            Self::HistoricUncontrolled => "historic_uncontrolled",
            Self::Unverified => "unverified",
        }
    }
}

/// Disclosure depth observed in source documentation (`C2 §4.5f`: Carlin counts vs TR/Cortez rules-only).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum DisclosureDepth {
    /// Quantitative counts, denominators, and failure rates disclosed (e.g. Carlin 2025).
    CountsAndFailureRates,
    /// Only policy rules described without quantitative counts/failure rates (e.g. TR/Cortez gap).
    RulesOnly,
}

impl DisclosureDepth {
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::CountsAndFailureRates => "counts_and_failure_rates",
            Self::RulesOnly => "rules_only",
        }
    }
}

/// Condensed CRM evidence record derived directly from [`CrmBatchEvaluation`].
#[derive(Clone, Debug, PartialEq)]
pub struct CrmEvidenceSummary {
    pub crm_id: String,
    pub lab: String,
    pub rule_id: String,
    pub rule_version: String,
    pub sd_basis: SdBasis,
    pub sample_count: usize,
    pub warning_count: usize,
    pub failure_count: usize,
    pub passed: bool,
    pub bias: BiasInterval,
}

impl CrmEvidenceSummary {
    /// Constructs a CRM verification summary directly from an evaluated [`CrmBatchEvaluation`].
    #[must_use]
    pub fn from_evaluation(
        crm_id: impl Into<String>,
        lab: impl Into<String>,
        eval: &CrmBatchEvaluation,
    ) -> Self {
        Self {
            crm_id: crm_id.into(),
            lab: lab.into(),
            rule_id: eval.rule_id.clone(),
            rule_version: eval.rule_version.clone(),
            sd_basis: eval.sd_basis,
            sample_count: eval.bias.n,
            warning_count: eval.warning_count,
            failure_count: eval.single_failure_count + eval.consecutive_failure_count,
            passed: eval.passed,
            bias: eval.bias.clone(),
        }
    }
}

/// Condensed blank evidence record derived directly from [`BlankBatchEvaluation`].
#[derive(Clone, Debug, PartialEq)]
pub struct BlankEvidenceSummary {
    pub rule_id: String,
    pub rule_version: String,
    pub total_count: usize,
    pub failure_count: usize,
    pub indeterminate_count: usize,
    pub failure_rate_pct: Option<f64>,
    pub possible_failure_rate_pct: Option<f64>,
}

impl BlankEvidenceSummary {
    /// Constructs a blank verification summary directly from an evaluated [`BlankBatchEvaluation`].
    #[must_use]
    pub fn from_evaluation(eval: &BlankBatchEvaluation) -> Self {
        Self {
            rule_id: eval.rule_id.clone(),
            rule_version: eval.rule_version.clone(),
            total_count: eval.total_count,
            failure_count: eval.failure_count,
            indeterminate_count: eval.indeterminate_count,
            failure_rate_pct: eval.failure_rate_pct,
            possible_failure_rate_pct: eval.possible_failure_rate_pct,
        }
    }
}

/// Condensed duplicate precision record derived directly from [`DuplicatePrecisionReport`].
#[derive(Clone, Debug, PartialEq)]
pub struct DuplicateEvidenceSummary {
    pub stage_label: String,
    pub grade_band: String,
    pub valid_pairs: usize,
    pub excluded_censored: usize,
    pub mass_mismatch_count: usize,
    pub unverified_support_pairs: usize,
    pub cv_avr_pct: f64,
    pub hard_from_cv_pct: f64,
}

impl DuplicateEvidenceSummary {
    /// Constructs a duplicate verification summary directly from a [`DuplicatePrecisionReport`].
    #[must_use]
    pub fn from_report(
        stage_label: impl Into<String>,
        grade_band: impl Into<String>,
        report: &DuplicatePrecisionReport,
    ) -> Self {
        Self {
            stage_label: stage_label.into(),
            grade_band: grade_band.into(),
            valid_pairs: report.valid_pairs,
            excluded_censored: report.excluded_censored,
            mass_mismatch_count: report.unfit_mass_mismatch_ids.len(),
            unverified_support_pairs: report.unverified_support_pairs,
            cv_avr_pct: report.cv_avr_pct,
            hard_from_cv_pct: report.hard_from_cv_pct,
        }
    }
}

/// Condensed sequential e-process & integrity monitor record derived from [`CrmMonitorSnapshot`].
#[derive(Clone, Debug, PartialEq)]
pub struct MonitorEvidenceSummary {
    pub crm_id: String,
    pub lab: String,
    pub steps_observed: usize,
    pub eproc_max_log_wealth: f64,
    pub eproc_alarm_step: Option<usize>,
    pub dispersion_collapse_step: Option<usize>,
    pub integrity_alarm_count: usize,
    pub calibration_alarm_count: usize,
    pub disposition_note: Option<String>,
}

impl MonitorEvidenceSummary {
    /// Constructs a monitor summary directly from a [`CrmMonitorSnapshot`].
    #[must_use]
    pub fn from_snapshot(snap: &CrmMonitorSnapshot, disposition_note: Option<String>) -> Self {
        let integrity_alarm_count = snap
            .alarms
            .iter()
            .filter(|a| a.scope == AlarmScope::Integrity)
            .count();
        let calibration_alarm_count = snap
            .alarms
            .iter()
            .filter(|a| a.scope == AlarmScope::CalibrationOrBias)
            .count();
        Self {
            crm_id: snap.key.crm_id.clone(),
            lab: snap.key.lab.clone(),
            steps_observed: snap.n_steps,
            eproc_max_log_wealth: snap.eproc_max_log_wealth,
            eproc_alarm_step: snap.eproc_alarm_step,
            dispersion_collapse_step: snap.dispersion_collapse_step,
            integrity_alarm_count,
            calibration_alarm_count,
            disposition_note,
        }
    }
}

/// Disaggregated per-laboratory failure rate summary (§12.4, §24.5, `E167`).
#[derive(Clone, Debug, PartialEq)]
pub struct LabFailureSummary {
    pub lab: String,
    pub crm_total: usize,
    pub crm_failures: usize,
    pub blank_total: usize,
    pub blank_failures: usize,
    pub failed_batches_cascaded: usize,
}

impl LabFailureSummary {
    #[must_use]
    pub fn crm_failure_rate_pct(&self) -> Option<f64> {
        (self.crm_total > 0).then(|| 100.0 * (self.crm_failures as f64) / (self.crm_total as f64))
    }

    #[must_use]
    pub fn blank_failure_rate_pct(&self) -> Option<f64> {
        (self.blank_total > 0)
            .then(|| 100.0 * (self.blank_failures as f64) / (self.blank_total as f64))
    }
}

/// Period where a QA/QC control was absent (`E168`, §24.5).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ControlAbsentRecord {
    pub period_label: String,
    pub control_kind: String,
    pub routine_samples_affected: usize,
}

/// Per-campaign QA/QC performance block (§24.5 bullets 2 & 8).
#[derive(Clone, Debug, PartialEq)]
pub struct QaqcCampaignEvidence {
    pub campaign_id: String,
    pub qaqc_regime: QaqcRegime,
    pub disclosure_depth: DisclosureDepth,
    pub insertion_rates: Option<InsertionRateSummary>,
    pub crm_summaries: Vec<CrmEvidenceSummary>,
    pub blank_summaries: Vec<BlankEvidenceSummary>,
    pub duplicate_summaries: Vec<DuplicateEvidenceSummary>,
    pub monitor_summaries: Vec<MonitorEvidenceSummary>,
    pub per_lab_failures: Vec<LabFailureSummary>,
    pub control_absent_periods: Vec<ControlAbsentRecord>,
    pub total_batches: usize,
    pub batches_without_signed_certificate: usize,
    pub original_refs: Vec<String>,
}

impl QaqcCampaignEvidence {
    /// Percentage of assay batches lacking a linked signed lab certificate (`None` if `total_batches == 0`).
    #[must_use]
    pub fn no_certificate_share_pct(&self) -> Option<f64> {
        (self.total_batches > 0).then(|| {
            100.0 * (self.batches_without_signed_certificate as f64) / (self.total_batches as f64)
        })
    }
}

/// Umpire inter-laboratory check batch summary (§24.5 bullet 3).
#[derive(Clone, Debug, PartialEq)]
pub struct UmpireBatchSummary {
    pub batch_id: String,
    pub primary_lab: String,
    pub umpire_lab: String,
    pub analyte: String,
    pub method: String,
    pub pair_count: usize,
    pub mean_bias_pct: f64,
    pub bias_ci_lower_pct: f64,
    pub bias_ci_upper_pct: f64,
    pub cv_avr_pct: f64,
    pub original_refs: Vec<String>,
}

/// Density and downhole survey verification summary (§24.5 bullet 4, `E382`, `E383`).
#[derive(Clone, Debug, PartialEq)]
pub struct DensityAndSurveyVerification {
    pub density_samples_total: usize,
    pub density_lab_checks: usize,
    pub grade_samples_total: usize,
    pub nominal_density_spacing_m: f64,
    pub optimal_grid_spacing_m: f64,
    pub survey_holes_total: usize,
    pub survey_holes_checked: usize,
    pub survey_instrument: String,
    pub original_refs: Vec<String>,
}

impl DensityAndSurveyVerification {
    #[must_use]
    pub fn lab_check_fraction_pct(&self) -> Option<f64> {
        (self.density_samples_total > 0)
            .then(|| 100.0 * (self.density_lab_checks as f64) / (self.density_samples_total as f64))
    }

    #[must_use]
    pub fn density_to_grade_ratio(&self) -> Option<f64> {
        (self.grade_samples_total > 0)
            .then(|| (self.density_samples_total as f64) / (self.grade_samples_total as f64))
    }
}

/// QP site-visit record (generic role only — zero personal PII, `AGENTS.md`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SiteVisitRecord {
    pub visit_date: String,
    pub qp_role: String,
    pub scope_observed: String,
    pub check_samples_collected: usize,
    pub original_refs: Vec<String>,
}

/// Independent check-sampling summary (§24.5 bullet 6).
#[derive(Clone, Debug, PartialEq)]
pub struct CheckSamplingRecord {
    pub campaign_id: String,
    pub sample_count: usize,
    pub analyte: String,
    pub method: String,
    pub mean_relative_diff_pct: f64,
    pub original_refs: Vec<String>,
}

/// Category of verification audit finding or limitation (§24.5 bullets 7 & 8, `E164`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum FindingCategory {
    RcVsDdBias,
    PorosityAssumption,
    MissingDataQualityObjectives,
    ControlAbsentPeriod,
    RulesOnlyDisclosureGap,
    CertificateChainGap,
    DensitySparsityGap,
    LaboratoryDriftOrIntegrity,
    UnresolvedLimitation,
}

impl FindingCategory {
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::RcVsDdBias => "rc_vs_dd_bias",
            Self::PorosityAssumption => "porosity_assumption",
            Self::MissingDataQualityObjectives => "missing_dqo",
            Self::ControlAbsentPeriod => "control_absent_period",
            Self::RulesOnlyDisclosureGap => "rules_only_disclosure_gap",
            Self::CertificateChainGap => "certificate_chain_gap",
            Self::DensitySparsityGap => "density_sparsity_gap",
            Self::LaboratoryDriftOrIntegrity => "lab_drift_or_integrity",
            Self::UnresolvedLimitation => "unresolved_limitation",
        }
    }
}

/// Severity of an audit finding or synthesized verification gap.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum FindingSeverity {
    Material,
    Moderate,
    Advisory,
}

impl FindingSeverity {
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::Material => "material",
            Self::Moderate => "moderate",
            Self::Advisory => "advisory",
        }
    }
}

/// Explicit audit finding or limitation attached to the verification index.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuditFinding {
    pub finding_id: String,
    pub category: FindingCategory,
    pub severity: FindingSeverity,
    pub summary: String,
    pub resolved: bool,
    pub resolution_note: Option<String>,
    pub source_ref: Option<String>,
}

/// Prohibited attempt to request a software resource/reserve classification label (`G-HALL-1`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClassificationLabelAttempt {
    pub proposed_label: String,
}

/// Guard `G-HALL-1` (`E023`, `E031`, plan §24.9): software never emits resource/reserve class labels.
pub fn refuse_classification_label(attempt: &ClassificationLabelAttempt) -> QaqcError {
    QaqcError::invalid(
        "verification_index.classification",
        format!(
            "G-HALL-1 refusal: software cannot assign or suggest classification label '{}'; the data-verification index supplies evidence only for the CP/QP",
            attempt.proposed_label
        ),
    )
}

/// Complete input request to assemble a per-item technical-report Data-Verification Index (`BD-174`).
#[derive(Clone, Debug, PartialEq)]
pub struct DataVerificationIndexRequest {
    pub property_name: String,
    pub effective_date: String,
    pub standard: TechnicalReportStandard,
    pub evidence_cut_id: String,
    pub signed_originals: Vec<CasOriginalRef>,
    pub database_audits: Vec<DrillholeDatabaseAudit>,
    pub qaqc_campaigns: Vec<QaqcCampaignEvidence>,
    pub umpire_batches: Vec<UmpireBatchSummary>,
    pub density_and_survey: Option<DensityAndSurveyVerification>,
    pub site_visits: Vec<SiteVisitRecord>,
    pub check_sampling: Vec<CheckSamplingRecord>,
    pub audit_findings: Vec<AuditFinding>,
}

/// One structured item section in the rendered technical-report data-verification index.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TechnicalReportItemSection {
    pub item_code: String,
    pub item_title: String,
    pub evidence_lines: Vec<String>,
    pub gaps: Vec<String>,
    pub linked_original_refs: Vec<String>,
}

/// Built, deterministic, non-classifying Data-Verification Evidence Index (`BD-174`, plan §24.5).
#[derive(Clone, Debug, PartialEq)]
pub struct DataVerificationIndex {
    pub property_name: String,
    pub effective_date: String,
    pub standard: TechnicalReportStandard,
    pub evidence_cut_id: String,
    pub draft_standard_banner: Option<&'static str>,
    pub item_sections: Vec<TechnicalReportItemSection>,
    pub synthesized_gaps: Vec<AuditFinding>,
    pub declared_findings: Vec<AuditFinding>,
    pub signed_originals: BTreeMap<String, CasOriginalRef>,
    pub overall_no_certificate_share_pct: Option<f64>,
    pub density_lab_check_fraction_pct: Option<f64>,
    pub unresolved_gap_count: usize,
    pub digest_hex: String,
}

impl DataVerificationIndex {
    /// `INV-002` / `G-HALL-1`: always `false`.
    #[must_use]
    pub const fn reserve_disclosure_claim(&self) -> bool {
        false
    }

    /// `INV-002`: always `false`.
    #[must_use]
    pub const fn execution_authorized(&self) -> bool {
        false
    }

    /// `INV-002`: always `false`.
    #[must_use]
    pub const fn geotechnical_signoff(&self) -> bool {
        false
    }

    /// `INV-002`: always `false`.
    #[must_use]
    pub const fn regulatory_compliance_claim(&self) -> bool {
        false
    }
}

fn is_valid_sha256_hex(s: &str) -> bool {
    s.len() == 64 && s.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

fn check_refs(
    refs: &[String],
    originals: &BTreeMap<String, CasOriginalRef>,
    context: &str,
) -> Result<(), QaqcError> {
    for r in refs {
        if !originals.contains_key(r) {
            return Err(QaqcError::UnusableInput {
                context: context.to_string(),
                detail: format!("unknown CAS original reference '{r}'"),
            });
        }
    }
    Ok(())
}

/// Validates all Corebox/QAQC/CAS inputs, synthesizes explicit §24.5 verification gaps, and
/// assembles the deterministic per-item [`DataVerificationIndex`] (`BD-174`).
pub fn build_data_verification_index(
    req: &DataVerificationIndexRequest,
) -> Result<DataVerificationIndex, QaqcError> {
    if req.property_name.trim().is_empty()
        || req.effective_date.trim().is_empty()
        || req.evidence_cut_id.trim().is_empty()
    {
        return Err(QaqcError::invalid(
            "verification_index.metadata",
            "property_name, effective_date, and evidence_cut_id must be non-empty",
        ));
    }

    let mut originals = BTreeMap::new();
    for orig in &req.signed_originals {
        if orig.ref_id.trim().is_empty() || orig.locator.trim().is_empty() {
            return Err(QaqcError::invalid(
                "verification_index.signed_originals",
                "ref_id and locator must be non-empty",
            ));
        }
        if !is_valid_sha256_hex(&orig.sha256_hex) {
            return Err(QaqcError::invalid(
                "verification_index.signed_originals",
                format!(
                    "original '{}' sha256_hex must be 64 lowercase hex characters",
                    orig.ref_id
                ),
            ));
        }
        if originals
            .insert(orig.ref_id.clone(), orig.clone())
            .is_some()
        {
            return Err(QaqcError::invalid(
                "verification_index.signed_originals",
                format!("duplicate original ref_id '{}'", orig.ref_id),
            ));
        }
    }

    for audit in &req.database_audits {
        if audit.audit_id.trim().is_empty() {
            return Err(QaqcError::invalid(
                "verification_index.database_audits",
                "audit_id must not be empty",
            ));
        }
        check_refs(&audit.original_refs, &originals, "database_audits")?;
    }
    for camp in &req.qaqc_campaigns {
        if camp.campaign_id.trim().is_empty() {
            return Err(QaqcError::invalid(
                "verification_index.qaqc_campaigns",
                "campaign_id must not be empty",
            ));
        }
        if camp.batches_without_signed_certificate > camp.total_batches {
            return Err(QaqcError::invalid(
                "verification_index.qaqc_campaigns",
                format!(
                    "campaign '{}' batches_without_signed_certificate ({}) exceeds total_batches ({})",
                    camp.campaign_id, camp.batches_without_signed_certificate, camp.total_batches
                ),
            ));
        }
        for lab in &camp.per_lab_failures {
            if lab.crm_failures > lab.crm_total || lab.blank_failures > lab.blank_total {
                return Err(QaqcError::invalid(
                    "verification_index.per_lab_failures",
                    format!(
                        "lab '{}' failure count exceeds total evaluated count",
                        lab.lab
                    ),
                ));
            }
        }
        check_refs(&camp.original_refs, &originals, "qaqc_campaigns")?;
    }
    for u in &req.umpire_batches {
        if u.batch_id.trim().is_empty() || u.pair_count == 0 {
            return Err(QaqcError::invalid(
                "verification_index.umpire_batches",
                "umpire batch requires non-empty batch_id and pair_count >= 1",
            ));
        }
        if !(u.mean_bias_pct.is_finite()
            && u.bias_ci_lower_pct.is_finite()
            && u.bias_ci_upper_pct.is_finite()
            && u.bias_ci_lower_pct <= u.bias_ci_upper_pct
            && u.cv_avr_pct.is_finite()
            && u.cv_avr_pct >= 0.0)
        {
            return Err(QaqcError::invalid(
                "verification_index.umpire_batches",
                format!(
                    "umpire batch '{}' has invalid bias interval or CV",
                    u.batch_id
                ),
            ));
        }
        check_refs(&u.original_refs, &originals, "umpire_batches")?;
    }
    if let Some(ds) = &req.density_and_survey {
        if ds.density_lab_checks > ds.density_samples_total
            || ds.survey_holes_checked > ds.survey_holes_total
            || !(ds.nominal_density_spacing_m.is_finite()
                && ds.nominal_density_spacing_m > 0.0
                && ds.optimal_grid_spacing_m.is_finite()
                && ds.optimal_grid_spacing_m > 0.0)
        {
            return Err(QaqcError::invalid(
                "verification_index.density_and_survey",
                "density/survey counts exceed totals or spacings are non-positive/non-finite",
            ));
        }
        check_refs(&ds.original_refs, &originals, "density_and_survey")?;
    }
    for sv in &req.site_visits {
        if sv.visit_date.trim().is_empty() || sv.qp_role.trim().is_empty() {
            return Err(QaqcError::invalid(
                "verification_index.site_visits",
                "site visit requires non-empty visit_date and qp_role",
            ));
        }
        check_refs(&sv.original_refs, &originals, "site_visits")?;
    }
    for cs in &req.check_sampling {
        if cs.campaign_id.trim().is_empty()
            || cs.sample_count == 0
            || !cs.mean_relative_diff_pct.is_finite()
        {
            return Err(QaqcError::invalid(
                "verification_index.check_sampling",
                "check sampling requires non-empty campaign_id, sample_count >= 1, and finite diff",
            ));
        }
        check_refs(&cs.original_refs, &originals, "check_sampling")?;
    }
    for f in &req.audit_findings {
        if f.finding_id.trim().is_empty() || f.summary.trim().is_empty() {
            return Err(QaqcError::invalid(
                "verification_index.audit_findings",
                "finding requires non-empty finding_id and summary",
            ));
        }
        if let Some(sr) = &f.source_ref
            && !originals.contains_key(sr)
        {
            return Err(QaqcError::UnusableInput {
                context: "audit_findings".to_string(),
                detail: format!("unknown CAS original reference '{sr}'"),
            });
        }
    }

    // Synthesize explicit §24.5 verification gaps (never hide missing evidence!).
    let mut synthesized_gaps: Vec<AuditFinding> = Vec::new();
    let mut total_batches_all = 0usize;
    let mut uncertified_batches_all = 0usize;

    for camp in &req.qaqc_campaigns {
        total_batches_all += camp.total_batches;
        uncertified_batches_all += camp.batches_without_signed_certificate;

        if camp.disclosure_depth == DisclosureDepth::RulesOnly {
            synthesized_gaps.push(AuditFinding {
                finding_id: format!("GAP-RULES-ONLY-{}", camp.campaign_id),
                category: FindingCategory::RulesOnlyDisclosureGap,
                severity: FindingSeverity::Material,
                summary: format!(
                    "Campaign '{}' discloses QA/QC rules only without quantitative counts or failure rates (C2 §4.5f gap)",
                    camp.campaign_id
                ),
                resolved: false,
                resolution_note: None,
                source_ref: None,
            });
        }
        if let Some(pct) = camp.no_certificate_share_pct()
            && pct > 0.0
        {
            synthesized_gaps.push(AuditFinding {
                finding_id: format!("GAP-NO-CERT-{}", camp.campaign_id),
                category: FindingCategory::CertificateChainGap,
                severity: if pct >= 5.0 {
                    FindingSeverity::Material
                } else {
                    FindingSeverity::Moderate
                },
                summary: format!(
                    "Campaign '{}': {}/{} assay batches ({pct:.2}%) lack a linked signed laboratory certificate",
                    camp.campaign_id, camp.batches_without_signed_certificate, camp.total_batches
                ),
                resolved: false,
                resolution_note: None,
                source_ref: None,
            });
        }
        if let Some(ins) = &camp.insertion_rates {
            for ctrl in &ins.absent_controls {
                synthesized_gaps.push(AuditFinding {
                    finding_id: format!("GAP-ABSENT-{}-{ctrl}", camp.campaign_id),
                    category: FindingCategory::ControlAbsentPeriod,
                    severity: FindingSeverity::Material,
                    summary: format!(
                        "Campaign '{}': control '{}' is completely absent across {} routine samples (E168)",
                        camp.campaign_id, ctrl, ins.routine_sample_count
                    ),
                    resolved: false,
                    resolution_note: None,
                    source_ref: None,
                });
            }
        }
        for cap in &camp.control_absent_periods {
            synthesized_gaps.push(AuditFinding {
                finding_id: format!(
                    "GAP-ABSENT-PERIOD-{}-{}",
                    camp.campaign_id, cap.period_label
                ),
                category: FindingCategory::ControlAbsentPeriod,
                severity: FindingSeverity::Material,
                summary: format!(
                    "Campaign '{}' period '{}': '{}' absent across {} routine samples",
                    camp.campaign_id,
                    cap.period_label,
                    cap.control_kind,
                    cap.routine_samples_affected
                ),
                resolved: false,
                resolution_note: None,
                source_ref: None,
            });
        }
        for mon in &camp.monitor_summaries {
            if mon.integrity_alarm_count > 0 && mon.disposition_note.is_none() {
                synthesized_gaps.push(AuditFinding {
                    finding_id: format!("GAP-INTEGRITY-{}-{}", mon.crm_id, mon.lab),
                    category: FindingCategory::LaboratoryDriftOrIntegrity,
                    severity: FindingSeverity::Material,
                    summary: format!(
                        "Campaign '{}' CRM '{}' at '{}': {} unresolved dispersion-collapse integrity alarm(s) (step {:?})",
                        camp.campaign_id,
                        mon.crm_id,
                        mon.lab,
                        mon.integrity_alarm_count,
                        mon.dispersion_collapse_step
                    ),
                    resolved: false,
                    resolution_note: None,
                    source_ref: None,
                });
            }
            if mon.eproc_alarm_step.is_some() && mon.disposition_note.is_none() {
                synthesized_gaps.push(AuditFinding {
                    finding_id: format!("GAP-EPROC-{}-{}", mon.crm_id, mon.lab),
                    category: FindingCategory::LaboratoryDriftOrIntegrity,
                    severity: FindingSeverity::Moderate,
                    summary: format!(
                        "Campaign '{}' CRM '{}' at '{}': unresolved sequential e-process drift alarm at step {:?}",
                        camp.campaign_id, mon.crm_id, mon.lab, mon.eproc_alarm_step
                    ),
                    resolved: false,
                    resolution_note: None,
                    source_ref: None,
                });
            }
        }
    }

    let overall_no_certificate_share_pct = (total_batches_all > 0)
        .then(|| 100.0 * (uncertified_batches_all as f64) / (total_batches_all as f64));

    let density_lab_check_fraction_pct = req
        .density_and_survey
        .as_ref()
        .and_then(DensityAndSurveyVerification::lab_check_fraction_pct);

    if let Some(ds) = &req.density_and_survey {
        if let Some(pct) = ds.lab_check_fraction_pct()
            && pct < DENSITY_LAB_CHECK_MIN_PCT
        {
            synthesized_gaps.push(AuditFinding {
                finding_id: "GAP-DENSITY-LAB-CHECK-E382".to_string(),
                category: FindingCategory::DensitySparsityGap,
                severity: FindingSeverity::Moderate,
                summary: format!(
                    "Laboratory density check fraction is {pct:.2}% ({}/{}), below default 5-10% benchmark (E382)",
                    ds.density_lab_checks, ds.density_samples_total
                ),
                resolved: false,
                resolution_note: None,
                source_ref: None,
            });
        }
        if ds.nominal_density_spacing_m > ds.optimal_grid_spacing_m {
            let ratio = ds.nominal_density_spacing_m / ds.optimal_grid_spacing_m;
            synthesized_gaps.push(AuditFinding {
                finding_id: "GAP-DENSITY-GRID-E383".to_string(),
                category: FindingCategory::DensitySparsityGap,
                severity: FindingSeverity::Moderate,
                summary: format!(
                    "Nominal density spacing ({:.1} m) is {:.2}x sparser than optimal confidence-class grid ({:.1} m; E383)",
                    ds.nominal_density_spacing_m, ratio, ds.optimal_grid_spacing_m
                ),
                resolved: false,
                resolution_note: None,
                source_ref: None,
            });
        }
    } else {
        synthesized_gaps.push(AuditFinding {
            finding_id: "GAP-DENSITY-SURVEY-ABSENT".to_string(),
            category: FindingCategory::DensitySparsityGap,
            severity: FindingSeverity::Material,
            summary: "No density or downhole survey verification record supplied".to_string(),
            resolved: false,
            resolution_note: None,
            source_ref: None,
        });
    }

    if req.database_audits.is_empty() {
        synthesized_gaps.push(AuditFinding {
            finding_id: "GAP-DB-AUDIT-ABSENT".to_string(),
            category: FindingCategory::UnresolvedLimitation,
            severity: FindingSeverity::Material,
            summary: "No drillhole database audit supplied against original Corebox records"
                .to_string(),
            resolved: false,
            resolution_note: None,
            source_ref: None,
        });
    }
    if req.umpire_batches.is_empty() {
        synthesized_gaps.push(AuditFinding {
            finding_id: "GAP-UMPIRE-ABSENT".to_string(),
            category: FindingCategory::UnresolvedLimitation,
            severity: FindingSeverity::Moderate,
            summary: "No umpire inter-laboratory check assay results supplied".to_string(),
            resolved: false,
            resolution_note: None,
            source_ref: None,
        });
    }
    if req.site_visits.is_empty() {
        synthesized_gaps.push(AuditFinding {
            finding_id: "GAP-SITE-VISIT-ABSENT".to_string(),
            category: FindingCategory::UnresolvedLimitation,
            severity: FindingSeverity::Material,
            summary: "No QP site-visit verification record supplied".to_string(),
            resolved: false,
            resolution_note: None,
            source_ref: None,
        });
    }

    // Populate item sections deterministically according to the selected code standard.
    let slots = req.standard.item_slots();
    let mut item_sections: Vec<TechnicalReportItemSection> = slots
        .iter()
        .map(|(code, title)| TechnicalReportItemSection {
            item_code: (*code).to_string(),
            item_title: (*title).to_string(),
            evidence_lines: Vec::new(),
            gaps: Vec::new(),
            linked_original_refs: Vec::new(),
        })
        .collect();

    // Index mapping:
    // NI 43-101 / S-K 1300: 0 = Drilling, 1 = Sample Prep/QAQC, 2 = Data Verification, 3 = MRE Basis
    // JORC Table 1:         0 = Section 1 (Sampling & Data), 1 = Section 2 (Exploration/Site), 2 = Section 3 (MRE Basis)
    let (idx_drill, idx_qaqc, idx_verify, idx_mre) = match req.standard {
        TechnicalReportStandard::Jorc2012Table1 => (0usize, 0usize, 1usize, 2usize),
        TechnicalReportStandard::Ni43101Current
        | TechnicalReportStandard::Ni43101Proposed
        | TechnicalReportStandard::Sk1300 => (0usize, 1usize, 2usize, 3usize),
    };

    for audit in &req.database_audits {
        item_sections[idx_drill].evidence_lines.push(format!(
            "Database audit '{}': holes={}, collars={}, surveys={}, assays={}, rejected_rows={}, composite_support_mismatches={} ({})",
            audit.audit_id,
            audit.holes_audited,
            audit.collars_verified,
            audit.surveys_verified,
            audit.assays_verified,
            audit.rejected_rows,
            audit.composite_support_mismatches,
            audit.notes
        ));
        item_sections[idx_drill]
            .linked_original_refs
            .extend(audit.original_refs.iter().cloned());
    }

    for camp in &req.qaqc_campaigns {
        let cert_str = camp.no_certificate_share_pct().map_or_else(
            || "n/a (0 batches)".to_string(),
            |p| {
                format!(
                    "{p:.2}% ({}/{})",
                    camp.batches_without_signed_certificate, camp.total_batches
                )
            },
        );
        item_sections[idx_qaqc].evidence_lines.push(format!(
            "Campaign '{}': regime={}, disclosure_depth={}, no_certificate_share={cert_str}",
            camp.campaign_id,
            camp.qaqc_regime.code(),
            camp.disclosure_depth.code()
        ));
        if let Some(ins) = &camp.insertion_rates {
            let fmt_pct =
                |p: Option<f64>| p.map_or_else(|| "n/a".to_string(), |v| format!("{v:.2}%"));
            item_sections[idx_qaqc].evidence_lines.push(format!(
                "  Insertion rates ({:?}, basis {:?}, routine={}, all_submitted={}): CRM {} ({}/{}), Blank {} ({}/{}), FieldDup {} ({}/{}), PulpDup {} ({}/{})",
                ins.denominator_kind,
                ins.target_basis,
                ins.routine_sample_count,
                ins.all_submitted_sample_count,
                fmt_pct(ins.crm.achieved_pct),
                ins.crm.numerator,
                ins.crm.denominator,
                fmt_pct(ins.blank.achieved_pct),
                ins.blank.numerator,
                ins.blank.denominator,
                fmt_pct(ins.field_duplicate.achieved_pct),
                ins.field_duplicate.numerator,
                ins.field_duplicate.denominator,
                fmt_pct(ins.pulp_duplicate.achieved_pct),
                ins.pulp_duplicate.numerator,
                ins.pulp_duplicate.denominator,
            ));
        }
        for crm in &camp.crm_summaries {
            let ci_str = match (crm.bias.ci_lower, crm.bias.ci_upper) {
                (Some(lo), Some(hi)) => format!("[{lo:+.6}, {hi:+.6}]"),
                _ => "n/a (n<2)".to_string(),
            };
            item_sections[idx_qaqc].evidence_lines.push(format!(
                "  CRM '{}' at '{}' (rule {} v{}, basis {}): n={}, warn={}, fail={}, passed={}, bias={:+.6} CI={ci_str} excludes_zero={}",
                crm.crm_id,
                crm.lab,
                crm.rule_id,
                crm.rule_version,
                crm.sd_basis.code(),
                crm.sample_count,
                crm.warning_count,
                crm.failure_count,
                crm.passed,
                crm.bias.bias,
                crm.bias.excludes_zero
            ));
        }
        for blk in &camp.blank_summaries {
            let rate = blk
                .failure_rate_pct
                .map_or_else(|| "n/a".to_string(), |r| format!("{r:.2}%"));
            let poss = blk
                .possible_failure_rate_pct
                .map_or_else(|| "n/a".to_string(), |r| format!("{r:.2}%"));
            item_sections[idx_qaqc].evidence_lines.push(format!(
                "  Blanks (rule {} v{}): total={}, fail={}, indeterminate={}, confirmed_fail_rate={rate}, possible_fail_rate={poss}",
                blk.rule_id, blk.rule_version, blk.total_count, blk.failure_count, blk.indeterminate_count
            ));
        }
        for dup in &camp.duplicate_summaries {
            item_sections[idx_qaqc].evidence_lines.push(format!(
                "  Duplicates [{}/{}]: valid_pairs={}, censored_excl={}, mass_mismatch={}, unverified_support={}, CV_AVR={:.2}%, HARD={:.2}%",
                dup.stage_label,
                dup.grade_band,
                dup.valid_pairs,
                dup.excluded_censored,
                dup.mass_mismatch_count,
                dup.unverified_support_pairs,
                dup.cv_avr_pct,
                dup.hard_from_cv_pct
            ));
        }
        for mon in &camp.monitor_summaries {
            item_sections[idx_qaqc].evidence_lines.push(format!(
                "  Sequential monitor '{}' at '{}': steps={}, max_ln_wealth={:.3}, eproc_alarm_step={:?}, dispersion_collapse_step={:?}, integrity_alarms={}, disposition={}",
                mon.crm_id,
                mon.lab,
                mon.steps_observed,
                mon.eproc_max_log_wealth,
                mon.eproc_alarm_step,
                mon.dispersion_collapse_step,
                mon.integrity_alarm_count,
                mon.disposition_note.as_deref().unwrap_or("UNRESOLVED")
            ));
        }
        for lab in &camp.per_lab_failures {
            let crm_r = lab
                .crm_failure_rate_pct()
                .map_or_else(|| "n/a".to_string(), |r| format!("{r:.2}%"));
            let blk_r = lab
                .blank_failure_rate_pct()
                .map_or_else(|| "n/a".to_string(), |r| format!("{r:.2}%"));
            item_sections[idx_qaqc].evidence_lines.push(format!(
                "  Lab '{}': CRM fail {}/{} ({crm_r}), Blank fail {}/{} ({blk_r}), cascaded_batches={}",
                lab.lab,
                lab.crm_failures,
                lab.crm_total,
                lab.blank_failures,
                lab.blank_total,
                lab.failed_batches_cascaded
            ));
        }
        item_sections[idx_qaqc]
            .linked_original_refs
            .extend(camp.original_refs.iter().cloned());
    }

    for u in &req.umpire_batches {
        item_sections[idx_verify].evidence_lines.push(format!(
            "Umpire batch '{}' ({} -> {}, {} / {}): n={}, mean_bias={:+.2}% CI=[{:+.2}%, {:+.2}%], CV_AVR={:.2}%",
            u.batch_id,
            u.primary_lab,
            u.umpire_lab,
            u.analyte,
            u.method,
            u.pair_count,
            u.mean_bias_pct,
            u.bias_ci_lower_pct,
            u.bias_ci_upper_pct,
            u.cv_avr_pct
        ));
        item_sections[idx_verify]
            .linked_original_refs
            .extend(u.original_refs.iter().cloned());
    }

    if let Some(ds) = &req.density_and_survey {
        let check_pct = ds
            .lab_check_fraction_pct()
            .map_or_else(|| "n/a".to_string(), |p| format!("{p:.2}%"));
        let d_g = ds
            .density_to_grade_ratio()
            .map_or_else(|| "n/a".to_string(), |r| format!("{r:.4}"));
        item_sections[idx_verify].evidence_lines.push(format!(
            "Density verification: total={}, lab_checks={} ({check_pct}; E382 benchmark {}-{}%), density/grade_ratio={d_g}, spacing={:.1}m vs optimal {:.1}m (E383)",
            ds.density_samples_total,
            ds.density_lab_checks,
            DENSITY_LAB_CHECK_MIN_PCT,
            DENSITY_LAB_CHECK_MAX_PCT,
            ds.nominal_density_spacing_m,
            ds.optimal_grid_spacing_m
        ));
        item_sections[idx_drill].evidence_lines.push(format!(
            "Downhole survey verification ({}): checked {}/{} holes",
            ds.survey_instrument, ds.survey_holes_checked, ds.survey_holes_total
        ));
        item_sections[idx_verify]
            .linked_original_refs
            .extend(ds.original_refs.iter().cloned());
    }

    for sv in &req.site_visits {
        item_sections[idx_verify].evidence_lines.push(format!(
            "Site visit ({}, role '{}'): scope='{}', check_samples_collected={}",
            sv.visit_date, sv.qp_role, sv.scope_observed, sv.check_samples_collected
        ));
        item_sections[idx_verify]
            .linked_original_refs
            .extend(sv.original_refs.iter().cloned());
    }

    for cs in &req.check_sampling {
        item_sections[idx_verify].evidence_lines.push(format!(
            "Check sampling (campaign '{}', {} / {}): n={}, mean_rel_diff={:+.2}%",
            cs.campaign_id, cs.analyte, cs.method, cs.sample_count, cs.mean_relative_diff_pct
        ));
        item_sections[idx_verify]
            .linked_original_refs
            .extend(cs.original_refs.iter().cloned());
    }

    // Route all findings and synthesized gaps to both their primary item and the MRE input-adequacy section.
    let all_findings: Vec<&AuditFinding> = req
        .audit_findings
        .iter()
        .chain(synthesized_gaps.iter())
        .collect();
    for f in &all_findings {
        let status = if f.resolved { "RESOLVED" } else { "UNRESOLVED" };
        let line = format!(
            "[{} | {} | {}] {}: {}{}",
            f.severity.code(),
            status,
            f.category.code(),
            f.finding_id,
            f.summary,
            f.resolution_note
                .as_ref()
                .map_or_else(String::new, |n| format!(" (resolution: {n})"))
        );
        let target_idx = match f.category {
            FindingCategory::RcVsDdBias => idx_drill,
            FindingCategory::ControlAbsentPeriod
            | FindingCategory::RulesOnlyDisclosureGap
            | FindingCategory::CertificateChainGap
            | FindingCategory::LaboratoryDriftOrIntegrity => idx_qaqc,
            FindingCategory::PorosityAssumption
            | FindingCategory::MissingDataQualityObjectives
            | FindingCategory::DensitySparsityGap
            | FindingCategory::UnresolvedLimitation => idx_verify,
        };
        if !f.resolved {
            item_sections[target_idx].gaps.push(line.clone());
            item_sections[idx_mre].gaps.push(line);
        } else {
            item_sections[target_idx].evidence_lines.push(line);
        }
        if let Some(sr) = &f.source_ref {
            item_sections[target_idx]
                .linked_original_refs
                .push(sr.clone());
        }
    }

    // Canonical deduplication & sorting of original refs per section.
    for sec in &mut item_sections {
        let unique: BTreeSet<String> = sec.linked_original_refs.drain(..).collect();
        sec.linked_original_refs = unique.into_iter().collect();
    }

    let unresolved_gap_count = req
        .audit_findings
        .iter()
        .chain(synthesized_gaps.iter())
        .filter(|f| !f.resolved)
        .count();

    let draft_standard_banner = req.standard.is_draft_standard().then_some(
        "DRAFT STANDARD — NOT IN FORCE (G-HALL-2): rendered against proposed NI 43-101 replacement requirements",
    );

    // Deterministic BLAKE3 digest over the rendered canonical index skeleton.
    let mut enc = Encoder::new_frame(*b"DVI1", 1);
    enc.put_str(&req.property_name);
    enc.put_str(&req.effective_date);
    enc.put_str(req.standard.code());
    enc.put_str(&req.evidence_cut_id);
    enc.put_u32(item_sections.len() as u32);
    for sec in &item_sections {
        enc.put_str(&sec.item_code);
        enc.put_u32(sec.evidence_lines.len() as u32);
        for l in &sec.evidence_lines {
            enc.put_str(l);
        }
        enc.put_u32(sec.gaps.len() as u32);
        for g in &sec.gaps {
            enc.put_str(g);
        }
        enc.put_u32(sec.linked_original_refs.len() as u32);
        for r in &sec.linked_original_refs {
            enc.put_str(r);
            if let Some(orig) = originals.get(r) {
                enc.put_str(&orig.sha256_hex);
            }
        }
    }
    let digest_hex = blake3_hash(&enc.finish()).to_hex();

    Ok(DataVerificationIndex {
        property_name: req.property_name.clone(),
        effective_date: req.effective_date.clone(),
        standard: req.standard,
        evidence_cut_id: req.evidence_cut_id.clone(),
        draft_standard_banner,
        item_sections,
        synthesized_gaps,
        declared_findings: req.audit_findings.clone(),
        signed_originals: originals,
        overall_no_certificate_share_pct,
        density_lab_check_fraction_pct,
        unresolved_gap_count,
        digest_hex,
    })
}

/// Renders a built [`DataVerificationIndex`] into deterministic GitHub-flavored Markdown per
/// technical-report item (`BD-174`, plan §24.5, `G-HALL-1`, `G-HALL-2`, `INV-002`).
#[must_use]
pub fn render_verification_index_markdown(index: &DataVerificationIndex) -> String {
    let mut md = String::new();
    if let Some(banner) = index.draft_standard_banner {
        let _ = writeln!(md, "> **WARNING:** {banner}\n");
    }
    let _ = writeln!(
        md,
        "# Data-Verification Evidence Index — {}\n",
        index.property_name
    );
    let _ = writeln!(
        md,
        "- **Standard:** {}\n- **Effective Date:** {}\n- **Evidence Cut:** `{}`\n- **Unresolved Gaps:** {}\n- **Index Digest (BLAKE3):** `{}`\n",
        index.standard.title(),
        index.effective_date,
        index.evidence_cut_id,
        index.unresolved_gap_count,
        index.digest_hex
    );

    for sec in &index.item_sections {
        let _ = writeln!(md, "## {} — {}\n", sec.item_code, sec.item_title);
        if sec.evidence_lines.is_empty() {
            md.push_str("_No verification evidence linked for this item._\n");
        } else {
            for line in &sec.evidence_lines {
                let _ = writeln!(md, "- {line}");
            }
        }
        if !sec.gaps.is_empty() {
            md.push_str("\n### Open Verification Gaps & Limitations\n\n");
            for gap in &sec.gaps {
                let _ = writeln!(md, "- {gap}");
            }
        }
        if !sec.linked_original_refs.is_empty() {
            md.push_str("\n### Linked Signed Originals (CAS)\n\n| Ref ID | Kind | Locator | SHA-256 |\n|---|---|---|---|\n");
            for r in &sec.linked_original_refs {
                if let Some(orig) = index.signed_originals.get(r) {
                    let _ = writeln!(
                        md,
                        "| `{}` | `{}` | `{}` | `{}` |",
                        orig.ref_id,
                        orig.kind.code(),
                        orig.locator,
                        orig.sha256_hex
                    );
                }
            }
        }
        md.push('\n');
    }

    md.push_str(
        "---\n\n_Non-claim notice (`INV-002`, `G-HALL-1`): This data-verification index is a draft evidence trail for the Competent Person / Qualified Person. It assigns no resource or reserve classification, makes no regulatory compliance claim, and carries `reserve_disclosure_claim=false`, `execution_authorized=false`, `geotechnical_signoff=false`, `regulatory_compliance_claim=false`._\n",
    );
    md
}
