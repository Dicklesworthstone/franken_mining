//! DESIGN (`BD-174`, plan §24.5): read-only technical-report Data-Verification Evidence Index
//! skeleton renderer (`fmine drill verify-index --input <worksheet.toml>`).
//! Determinism: BitwisePortable (canonical `BTreeMap`/`BTreeSet` ordering, CAS SHA-256 original
//! links, `G-HALL-1`/`G-HALL-2`/`INV-002` guards, and BLAKE3 index digest).

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use fmine_core::{Digest32, anchor::SourceAnchor};
use fmine_evidence::{Refusal, RefusalCode};
use fmine_qaqc::verification_index::{
    AuditFinding, CasOriginalRef, CheckSamplingRecord, ControlAbsentRecord,
    DataVerificationIndexRequest, DensityAndSurveyVerification, DisclosureDepth,
    DrillholeDatabaseAudit, FindingCategory, FindingSeverity, LabFailureSummary,
    OriginalArtifactKind, QaqcCampaignEvidence, QaqcRegime, SiteVisitRecord,
    TechnicalReportStandard, UmpireBatchSummary, build_data_verification_index,
    render_verification_index_markdown,
};
use fmine_registry::toml_subset::{TomlField, TomlValue, parse_subset};

use crate::{Envelope, journal_arguments, json_string as quote, worksheet_input};

pub const VERB: &str = "drill verify-index";
pub const USAGE: &str = "drill verify-index --input <verification-index.toml>";
pub const NO_CLAIM: &str = "Draft QP data-verification evidence index skeleton only (plan §24.5, BD-174). Assigns no resource or reserve classification (G-HALL-1), establishes no regulatory compliance, and sets reserve_disclosure_claim=false, execution_authorized=false, geotechnical_signoff=false, regulatory_compliance_claim=false (INV-002).";
pub const ROBOT_DOCS_JSON: &str = concat!(
    r#"{"schema":"fmine.data-verification-index.v1","#,
    r#""command":"drill verify-index --input <verification-index.toml>","#,
    r#""standards":["ni43101_current","ni43101_proposed","sk1300","jorc2012_table1"],"#,
    r#""guards":["G-HALL-1 no classification labels","G-HALL-2 draft standard banner on ni43101_proposed","INV-002 all four authority flags false"],"#,
    r#""synthesized_gaps":["rules_only disclosure gap (C2 §4.5f)","no_certificate_share > 0","absent controls (E168)","density lab check < 5% (E382)","sparse density grid (E383)","missing database audit / umpire / site visit"],"#,
    r#""example":"tests/drill/data_verification_index.toml","#,
    r#""side_effect":"Read-only; renders per-item Markdown skeleton and canonical JSON with BLAKE3 digest"}"#,
);

const SCHEMA: &str = "fmine.data-verification-index.v1";
const MAX_BYTES: u64 = 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Command {
    pub input: PathBuf,
}

pub fn parse(args: &[String]) -> Result<Command, String> {
    let (mut options, words) = journal_arguments(args, &[])?;
    let input = options.remove("--input").filter(|s| !s.trim().is_empty());
    if words != ["drill", "verify-index"] || !options.is_empty() {
        return Err(USAGE.to_string());
    }
    Ok(Command {
        input: input.ok_or(USAGE)?.into(),
    })
}

fn invalid(detail: impl Into<String>) -> Refusal {
    Refusal::new(
        RefusalCode::SourceCellUnusable208,
        format!("drill verify-index declaration: {}", detail.into()),
    )
}

fn check_keys(fields: &[TomlField], required: &[&str], optional: &[&str]) -> Result<(), Refusal> {
    let mut seen = BTreeSet::new();
    for f in fields {
        if !required.contains(&f.key.as_str()) && !optional.contains(&f.key.as_str()) {
            return Err(invalid(format!(
                "unknown field '{}' at line {}",
                f.key, f.line
            )));
        }
        if !seen.insert(f.key.as_str()) {
            return Err(invalid(format!(
                "duplicate field '{}' at line {}",
                f.key, f.line
            )));
        }
    }
    for req in required {
        if !seen.contains(*req) {
            return Err(invalid(format!("missing required field '{req}'")));
        }
    }
    Ok(())
}

fn get_str<'a>(fields: &'a [TomlField], key: &str) -> Result<Option<&'a str>, Refusal> {
    let Some(f) = fields.iter().find(|f| f.key == key) else {
        return Ok(None);
    };
    let TomlValue::Str(s) = &f.value else {
        return Err(invalid(format!("field '{key}' must be a quoted string")));
    };
    if s.is_empty() || s.trim() != s || s.len() > 4096 || s.chars().any(char::is_control) {
        return Err(invalid(format!(
            "field '{key}' must be bounded non-empty text without edge whitespace or control chars"
        )));
    }
    Ok(Some(s.as_str()))
}

fn req_str<'a>(fields: &'a [TomlField], key: &str) -> Result<&'a str, Refusal> {
    get_str(fields, key)?.ok_or_else(|| invalid(format!("missing required field '{key}'")))
}

fn req_usize(fields: &[TomlField], key: &str) -> Result<usize, Refusal> {
    match fields.iter().find(|f| f.key == key).map(|f| &f.value) {
        Some(TomlValue::Int(n)) => usize::try_from(*n)
            .map_err(|_| invalid(format!("field '{key}' must be a non-negative integer"))),
        Some(TomlValue::Str(s)) => s
            .parse::<usize>()
            .map_err(|_| invalid(format!("field '{key}' must be a non-negative integer"))),
        _ => Err(invalid(format!(
            "field '{key}' must be a non-negative integer"
        ))),
    }
}

fn req_f64(fields: &[TomlField], key: &str) -> Result<f64, Refusal> {
    let val = match fields.iter().find(|f| f.key == key).map(|f| &f.value) {
        Some(TomlValue::Int(n)) => *n as f64,
        Some(TomlValue::Str(s)) => {
            let trimmed = s.trim();
            let v = trimmed
                .parse::<f64>()
                .map_err(|_| invalid(format!("field '{key}' must be a finite number")))?;
            let nonzero_mantissa = trimmed
                .split(['e', 'E'])
                .next()
                .unwrap_or("")
                .bytes()
                .any(|b| matches!(b, b'1'..=b'9'));
            if !v.is_finite() || (v == 0.0 && nonzero_mantissa) {
                return Err(invalid(format!(
                    "field '{key}' must be finite and must not underflow to zero"
                )));
            }
            v
        }
        None => return Err(invalid(format!("missing required numeric field '{key}'"))),
    };
    if !val.is_finite() {
        return Err(invalid(format!("field '{key}' must be finite")));
    }
    Ok(val)
}

fn req_bool(fields: &[TomlField], key: &str) -> Result<bool, Refusal> {
    match req_str(fields, key)? {
        "true" => Ok(true),
        "false" => Ok(false),
        other => Err(invalid(format!(
            "field '{key}' must be \"true\" or \"false\", got '{other}'"
        ))),
    }
}

fn parse_ref_list(fields: &[TomlField], key: &str) -> Result<Vec<String>, Refusal> {
    let Some(raw) = get_str(fields, key)? else {
        return Ok(Vec::new());
    };
    let mut out = Vec::new();
    let mut seen = BTreeSet::new();
    for piece in raw.split(',') {
        let trimmed = piece.trim();
        if trimmed.is_empty() {
            return Err(invalid(format!("field '{key}' contains empty reference")));
        }
        if !seen.insert(trimmed.to_string()) {
            return Err(invalid(format!(
                "field '{key}' contains duplicate reference '{trimmed}'"
            )));
        }
        out.push(trimmed.to_string());
    }
    Ok(out)
}

fn parse_standard(raw: &str) -> Result<TechnicalReportStandard, Refusal> {
    match raw {
        "ni43101_current" => Ok(TechnicalReportStandard::Ni43101Current),
        "ni43101_proposed" => Ok(TechnicalReportStandard::Ni43101Proposed),
        "sk1300" => Ok(TechnicalReportStandard::Sk1300),
        "jorc2012_table1" => Ok(TechnicalReportStandard::Jorc2012Table1),
        other => Err(invalid(format!("unknown standard '{other}'"))),
    }
}

fn parse_orig_kind(raw: &str) -> Result<OriginalArtifactKind, Refusal> {
    match raw {
        "lab_certificate" => Ok(OriginalArtifactKind::LabCertificate),
        "survey_report" => Ok(OriginalArtifactKind::SurveyReport),
        "density_report" => Ok(OriginalArtifactKind::DensityReport),
        "site_visit_memo" => Ok(OriginalArtifactKind::SiteVisitMemo),
        "umpire_certificate" => Ok(OriginalArtifactKind::UmpireCertificate),
        "database_export" => Ok(OriginalArtifactKind::DatabaseExport),
        "qaqc_policy" => Ok(OriginalArtifactKind::QaqcPolicy),
        other => Err(invalid(format!("unknown original artifact kind '{other}'"))),
    }
}

fn parse_regime(raw: &str) -> Result<QaqcRegime, Refusal> {
    match raw {
        "modern_full" => Ok(QaqcRegime::ModernFull),
        "legacy_partial" => Ok(QaqcRegime::LegacyPartial),
        "historic_uncontrolled" => Ok(QaqcRegime::HistoricUncontrolled),
        "unverified" => Ok(QaqcRegime::Unverified),
        other => Err(invalid(format!("unknown qaqc_regime '{other}'"))),
    }
}

fn parse_depth(raw: &str) -> Result<DisclosureDepth, Refusal> {
    match raw {
        "counts_and_failure_rates" => Ok(DisclosureDepth::CountsAndFailureRates),
        "rules_only" => Ok(DisclosureDepth::RulesOnly),
        other => Err(invalid(format!("unknown disclosure_depth '{other}'"))),
    }
}

fn parse_finding_category(raw: &str) -> Result<FindingCategory, Refusal> {
    match raw {
        "rc_vs_dd_bias" => Ok(FindingCategory::RcVsDdBias),
        "porosity_assumption" => Ok(FindingCategory::PorosityAssumption),
        "missing_dqo" => Ok(FindingCategory::MissingDataQualityObjectives),
        "control_absent_period" => Ok(FindingCategory::ControlAbsentPeriod),
        "rules_only_disclosure_gap" => Ok(FindingCategory::RulesOnlyDisclosureGap),
        "certificate_chain_gap" => Ok(FindingCategory::CertificateChainGap),
        "density_sparsity_gap" => Ok(FindingCategory::DensitySparsityGap),
        "lab_drift_or_integrity" => Ok(FindingCategory::LaboratoryDriftOrIntegrity),
        "unresolved_limitation" => Ok(FindingCategory::UnresolvedLimitation),
        other => Err(invalid(format!("unknown audit finding category '{other}'"))),
    }
}

fn parse_finding_severity(raw: &str) -> Result<FindingSeverity, Refusal> {
    match raw {
        "material" => Ok(FindingSeverity::Material),
        "moderate" => Ok(FindingSeverity::Moderate),
        "advisory" => Ok(FindingSeverity::Advisory),
        other => Err(invalid(format!("unknown audit finding severity '{other}'"))),
    }
}

fn run_worksheet(text: &str, anchor: &SourceAnchor) -> Result<Envelope, Refusal> {
    let doc = parse_subset(text).map_err(|e| invalid(e.to_string()))?;
    check_keys(
        &doc.top,
        &[
            "schema",
            "property_name",
            "effective_date",
            "standard",
            "evidence_cut_id",
        ],
        &[],
    )?;
    if req_str(&doc.top, "schema")? != SCHEMA {
        return Err(invalid(format!("schema must be '{SCHEMA}'")));
    }

    let mut signed_originals = Vec::new();
    let mut database_audits = Vec::new();
    let mut campaigns: BTreeMap<String, QaqcCampaignEvidence> = BTreeMap::new();
    let mut campaign_order: Vec<String> = Vec::new();
    let mut umpire_batches = Vec::new();
    let mut density_and_survey = None;
    let mut site_visits = Vec::new();
    let mut check_sampling = Vec::new();
    let mut audit_findings = Vec::new();

    for row in &doc.rows {
        let f = &row.fields;
        match row.table.as_str() {
            "original" => {
                check_keys(f, &["ref_id", "kind", "locator", "sha256_hex"], &[])?;
                signed_originals.push(CasOriginalRef {
                    ref_id: req_str(f, "ref_id")?.to_string(),
                    kind: parse_orig_kind(req_str(f, "kind")?)?,
                    locator: req_str(f, "locator")?.to_string(),
                    sha256_hex: req_str(f, "sha256_hex")?.to_string(),
                });
            }
            "database_audit" => {
                check_keys(
                    f,
                    &[
                        "audit_id",
                        "holes_audited",
                        "collars_verified",
                        "surveys_verified",
                        "assays_verified",
                        "rejected_rows",
                        "composite_support_mismatches",
                        "notes",
                    ],
                    &["original_refs"],
                )?;
                database_audits.push(DrillholeDatabaseAudit {
                    audit_id: req_str(f, "audit_id")?.to_string(),
                    holes_audited: req_usize(f, "holes_audited")?,
                    collars_verified: req_usize(f, "collars_verified")?,
                    surveys_verified: req_usize(f, "surveys_verified")?,
                    assays_verified: req_usize(f, "assays_verified")?,
                    rejected_rows: req_usize(f, "rejected_rows")?,
                    composite_support_mismatches: req_usize(f, "composite_support_mismatches")?,
                    original_refs: parse_ref_list(f, "original_refs")?,
                    notes: req_str(f, "notes")?.to_string(),
                });
            }
            "qaqc_campaign" => {
                check_keys(
                    f,
                    &[
                        "campaign_id",
                        "qaqc_regime",
                        "disclosure_depth",
                        "total_batches",
                        "batches_without_signed_certificate",
                    ],
                    &["original_refs"],
                )?;
                let cid = req_str(f, "campaign_id")?.to_string();
                if campaigns.contains_key(&cid) {
                    return Err(invalid(format!("duplicate qaqc_campaign '{cid}'")));
                }
                campaign_order.push(cid.clone());
                campaigns.insert(
                    cid.clone(),
                    QaqcCampaignEvidence {
                        campaign_id: cid,
                        qaqc_regime: parse_regime(req_str(f, "qaqc_regime")?)?,
                        disclosure_depth: parse_depth(req_str(f, "disclosure_depth")?)?,
                        insertion_rates: None,
                        crm_summaries: Vec::new(),
                        blank_summaries: Vec::new(),
                        duplicate_summaries: Vec::new(),
                        monitor_summaries: Vec::new(),
                        per_lab_failures: Vec::new(),
                        control_absent_periods: Vec::new(),
                        total_batches: req_usize(f, "total_batches")?,
                        batches_without_signed_certificate: req_usize(
                            f,
                            "batches_without_signed_certificate",
                        )?,
                        original_refs: parse_ref_list(f, "original_refs")?,
                    },
                );
            }
            "lab_failure" => {
                check_keys(
                    f,
                    &[
                        "campaign_id",
                        "lab",
                        "crm_total",
                        "crm_failures",
                        "blank_total",
                        "blank_failures",
                        "failed_batches_cascaded",
                    ],
                    &[],
                )?;
                let cid = req_str(f, "campaign_id")?;
                let camp = campaigns.get_mut(cid).ok_or_else(|| {
                    invalid(format!(
                        "line {}: lab_failure references undeclared qaqc_campaign '{cid}'",
                        row.line
                    ))
                })?;
                camp.per_lab_failures.push(LabFailureSummary {
                    lab: req_str(f, "lab")?.to_string(),
                    crm_total: req_usize(f, "crm_total")?,
                    crm_failures: req_usize(f, "crm_failures")?,
                    blank_total: req_usize(f, "blank_total")?,
                    blank_failures: req_usize(f, "blank_failures")?,
                    failed_batches_cascaded: req_usize(f, "failed_batches_cascaded")?,
                });
            }
            "control_absent" => {
                check_keys(
                    f,
                    &[
                        "campaign_id",
                        "period_label",
                        "control_kind",
                        "routine_samples_affected",
                    ],
                    &[],
                )?;
                let cid = req_str(f, "campaign_id")?;
                let camp = campaigns.get_mut(cid).ok_or_else(|| {
                    invalid(format!(
                        "line {}: control_absent references undeclared qaqc_campaign '{cid}'",
                        row.line
                    ))
                })?;
                camp.control_absent_periods.push(ControlAbsentRecord {
                    period_label: req_str(f, "period_label")?.to_string(),
                    control_kind: req_str(f, "control_kind")?.to_string(),
                    routine_samples_affected: req_usize(f, "routine_samples_affected")?,
                });
            }
            "umpire_batch" => {
                check_keys(
                    f,
                    &[
                        "batch_id",
                        "primary_lab",
                        "umpire_lab",
                        "analyte",
                        "method",
                        "pair_count",
                        "mean_bias_pct",
                        "bias_ci_lower_pct",
                        "bias_ci_upper_pct",
                        "cv_avr_pct",
                    ],
                    &["original_refs"],
                )?;
                umpire_batches.push(UmpireBatchSummary {
                    batch_id: req_str(f, "batch_id")?.to_string(),
                    primary_lab: req_str(f, "primary_lab")?.to_string(),
                    umpire_lab: req_str(f, "umpire_lab")?.to_string(),
                    analyte: req_str(f, "analyte")?.to_string(),
                    method: req_str(f, "method")?.to_string(),
                    pair_count: req_usize(f, "pair_count")?,
                    mean_bias_pct: req_f64(f, "mean_bias_pct")?,
                    bias_ci_lower_pct: req_f64(f, "bias_ci_lower_pct")?,
                    bias_ci_upper_pct: req_f64(f, "bias_ci_upper_pct")?,
                    cv_avr_pct: req_f64(f, "cv_avr_pct")?,
                    original_refs: parse_ref_list(f, "original_refs")?,
                });
            }
            "density_survey" => {
                if density_and_survey.is_some() {
                    return Err(invalid("duplicate [[density_survey]] section"));
                }
                check_keys(
                    f,
                    &[
                        "density_samples_total",
                        "density_lab_checks",
                        "grade_samples_total",
                        "nominal_density_spacing_m",
                        "optimal_grid_spacing_m",
                        "survey_holes_total",
                        "survey_holes_checked",
                        "survey_instrument",
                    ],
                    &["original_refs"],
                )?;
                density_and_survey = Some(DensityAndSurveyVerification {
                    density_samples_total: req_usize(f, "density_samples_total")?,
                    density_lab_checks: req_usize(f, "density_lab_checks")?,
                    grade_samples_total: req_usize(f, "grade_samples_total")?,
                    nominal_density_spacing_m: req_f64(f, "nominal_density_spacing_m")?,
                    optimal_grid_spacing_m: req_f64(f, "optimal_grid_spacing_m")?,
                    survey_holes_total: req_usize(f, "survey_holes_total")?,
                    survey_holes_checked: req_usize(f, "survey_holes_checked")?,
                    survey_instrument: req_str(f, "survey_instrument")?.to_string(),
                    original_refs: parse_ref_list(f, "original_refs")?,
                });
            }
            "site_visit" => {
                check_keys(
                    f,
                    &[
                        "visit_date",
                        "qp_role",
                        "scope_observed",
                        "check_samples_collected",
                    ],
                    &["original_refs"],
                )?;
                site_visits.push(SiteVisitRecord {
                    visit_date: req_str(f, "visit_date")?.to_string(),
                    qp_role: req_str(f, "qp_role")?.to_string(),
                    scope_observed: req_str(f, "scope_observed")?.to_string(),
                    check_samples_collected: req_usize(f, "check_samples_collected")?,
                    original_refs: parse_ref_list(f, "original_refs")?,
                });
            }
            "check_sampling" => {
                check_keys(
                    f,
                    &[
                        "campaign_id",
                        "sample_count",
                        "analyte",
                        "method",
                        "mean_relative_diff_pct",
                    ],
                    &["original_refs"],
                )?;
                check_sampling.push(CheckSamplingRecord {
                    campaign_id: req_str(f, "campaign_id")?.to_string(),
                    sample_count: req_usize(f, "sample_count")?,
                    analyte: req_str(f, "analyte")?.to_string(),
                    method: req_str(f, "method")?.to_string(),
                    mean_relative_diff_pct: req_f64(f, "mean_relative_diff_pct")?,
                    original_refs: parse_ref_list(f, "original_refs")?,
                });
            }
            "audit_finding" => {
                check_keys(
                    f,
                    &["finding_id", "category", "severity", "summary", "resolved"],
                    &["resolution_note", "source_ref"],
                )?;
                audit_findings.push(AuditFinding {
                    finding_id: req_str(f, "finding_id")?.to_string(),
                    category: parse_finding_category(req_str(f, "category")?)?,
                    severity: parse_finding_severity(req_str(f, "severity")?)?,
                    summary: req_str(f, "summary")?.to_string(),
                    resolved: req_bool(f, "resolved")?,
                    resolution_note: get_str(f, "resolution_note")?.map(ToString::to_string),
                    source_ref: get_str(f, "source_ref")?.map(ToString::to_string),
                });
            }
            other => {
                return Err(invalid(format!(
                    "line {}: unexpected table '[[{other}]]'",
                    row.line
                )));
            }
        }
    }

    let mut qaqc_campaigns = Vec::with_capacity(campaign_order.len());
    for cid in campaign_order {
        if let Some(camp) = campaigns.remove(&cid) {
            qaqc_campaigns.push(camp);
        }
    }

    let req = DataVerificationIndexRequest {
        property_name: req_str(&doc.top, "property_name")?.to_string(),
        effective_date: req_str(&doc.top, "effective_date")?.to_string(),
        standard: parse_standard(req_str(&doc.top, "standard")?)?,
        evidence_cut_id: req_str(&doc.top, "evidence_cut_id")?.to_string(),
        signed_originals,
        database_audits,
        qaqc_campaigns,
        umpire_batches,
        density_and_survey,
        site_visits,
        check_sampling,
        audit_findings,
    };

    let index = build_data_verification_index(&req).map_err(|e| e.to_refusal())?;
    let markdown = render_verification_index_markdown(&index);

    let items_json: Vec<String> = index
        .item_sections
        .iter()
        .map(|s| {
            format!(
                "{{\"item_code\":{},\"item_title\":{},\"evidence_count\":{},\"gap_count\":{},\"linked_original_refs\":[{}]}}",
                quote(&s.item_code),
                quote(&s.item_title),
                s.evidence_lines.len(),
                s.gaps.len(),
                s.linked_original_refs
                    .iter()
                    .map(|r| quote(r))
                    .collect::<Vec<_>>()
                    .join(",")
            )
        })
        .collect();
    let gaps_json: Vec<String> = index
        .synthesized_gaps
        .iter()
        .map(|g| {
            format!(
                "{{\"finding_id\":{},\"category\":{},\"severity\":{},\"summary\":{}}}",
                quote(&g.finding_id),
                quote(g.category.code()),
                quote(g.severity.code()),
                quote(&g.summary),
            )
        })
        .collect();
    let declared_json: Vec<String> = index
        .declared_findings
        .iter()
        .map(|f| {
            format!(
                "{{\"finding_id\":{},\"category\":{},\"severity\":{},\"resolved\":{},\"summary\":{}}}",
                quote(&f.finding_id),
                quote(f.category.code()),
                quote(f.severity.code()),
                f.resolved,
                quote(&f.summary),
            )
        })
        .collect();
    let source_json = worksheet_input::source_json(anchor, text.len());
    let no_cert_json = index
        .overall_no_certificate_share_pct
        .map_or_else(|| "null".to_string(), |v| format!("{v}"));
    let dens_check_json = index
        .density_lab_check_fraction_pct
        .map_or_else(|| "null".to_string(), |v| format!("{v}"));
    let draft_json = index
        .draft_standard_banner
        .map_or_else(|| "null".to_string(), quote);
    let payload = format!(
        "{{\"schema\":\"fmine.data-verification-index-result.v1\",\"source\":{source_json},\"property_name\":{},\"effective_date\":{},\"standard\":{},\"evidence_cut_id\":{},\"draft_standard_banner\":{draft_json},\"index_digest_blake3\":{},\"unresolved_gap_count\":{},\"overall_no_certificate_share_pct\":{no_cert_json},\"density_lab_check_fraction_pct\":{dens_check_json},\"signed_original_count\":{},\"items\":[{}],\"declared_findings\":[{}],\"synthesized_gaps\":[{}],\"authority_flags\":{{\"reserve_disclosure_claim\":false,\"execution_authorized\":false,\"geotechnical_signoff\":false,\"regulatory_compliance_claim\":false}},\"source_sha256\":{}}}",
        quote(&index.property_name),
        quote(&index.effective_date),
        quote(index.standard.code()),
        quote(&index.evidence_cut_id),
        quote(&index.digest_hex),
        index.unresolved_gap_count,
        index.signed_originals.len(),
        items_json.join(","),
        declared_json.join(","),
        gaps_json.join(","),
        quote(&Digest32(anchor.artifact_sha256).to_string()),
    );
    let mut env = Envelope::ok(VERB, payload, vec![]);
    env.rendered_markdown = Some(markdown);
    Ok(env)
}

pub fn execute(cmd: &Command, root: &Path) -> Envelope {
    let result = worksheet_input::read(
        &root.join(&cmd.input),
        "technical-report data-verification index",
        MAX_BYTES,
    )
    .and_then(|(text, anchor)| run_worksheet(&text, &anchor));
    let mut env = result.unwrap_or_else(|r| Envelope::refused(VERB, r, vec![]));
    env.no_claim = Some(NO_CLAIM);
    env
}
