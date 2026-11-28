//! Integration tests for `BD-174` (plan §24.5): Technical-report Data-Verification Evidence
//! Index skeleton wired directly to live `fmine-qaqc` CRM, insertion, blank, duplicate, and
//! sequential monitor outputs.

use fmine_drill::records::{AssayMethod, AssayValue, BlankStage, DuplicateStage, SampleType};
use fmine_evidence::RefusalCode;
use fmine_qaqc::crm::{
    CrmBatchInput, CrmCertificateSpec, CrmObservation, SdBasis, SiteCrmRule, evaluate_crm_batch,
};
use fmine_qaqc::duplicates::{DuplicatePair, DuplicatePolicy, evaluate_duplicates};
use fmine_qaqc::insertion::{
    BlankCarryoverThresholds, BlankObservation, InsertionTargetSpec, PrecedingSampleGrade,
    SiteBlankRule, compute_insertion_rates, evaluate_blanks,
};
use fmine_qaqc::monitors::{
    CrmMonitorConfig, CrmReferenceParams, CrmSequentialMonitor, CrmStreamKey, CrmStreamValue,
};
use fmine_qaqc::verification_index::{
    AuditFinding, BlankEvidenceSummary, CasOriginalRef, CheckSamplingRecord,
    ClassificationLabelAttempt, ControlAbsentRecord, CrmEvidenceSummary,
    DataVerificationIndexRequest, DensityAndSurveyVerification, DisclosureDepth,
    DrillholeDatabaseAudit, DuplicateEvidenceSummary, FindingCategory, FindingSeverity,
    LabFailureSummary, MonitorEvidenceSummary, OriginalArtifactKind, QaqcCampaignEvidence,
    QaqcRegime, SiteVisitRecord, TechnicalReportStandard, UmpireBatchSummary,
    build_data_verification_index, refuse_classification_label, render_verification_index_markdown,
};

#[test]
fn test_bd174_data_verification_index_wires_live_qaqc_and_renders_all_standards() {
    // 1. Live CRM evaluation (Abzalov FX-2 style batch).
    let cert = CrmCertificateSpec {
        crm_id: "OREAS-264".to_string(),
        analyte: "Au".to_string(),
        method: AssayMethod::FireAssayAas,
        certified_mean: 2.0e-6,
        sigma_c: 0.08e-6,
        sigma_l: 0.12e-6,
        certificate_source: "OREAS-264 cert v1".to_string(),
    };
    let crm_obs: Vec<CrmObservation> = [2.01e-6, 1.98e-6, 2.05e-6, 2.02e-6, 1.97e-6, 2.04e-6]
        .iter()
        .enumerate()
        .map(|(i, &v)| CrmObservation {
            result_id: format!("CRM-R{i}"),
            sample_id: format!("CRM-S{i}"),
            lab: "ALS-Primary".to_string(),
            lab_batch: "B001".to_string(),
            value: AssayValue::Measured { mass_fraction: v },
            received_at_ns: (i as i64) * 1_000_000_000,
        })
        .collect();
    let crm_eval = evaluate_crm_batch(
        &CrmBatchInput {
            spec: cert,
            observations: crm_obs,
        },
        &SiteCrmRule::ngm_v1(SdBasis::CertifiedWithinLabSigmaC),
    )
    .expect("live CRM eval");

    // 2. Live Blank evaluation (Carlin AND rule).
    let blank_rule = SiteBlankRule::carlin_and_v1(BlankCarryoverThresholds {
        abs_threshold: 2.5e-8,
        carryover_fraction: 0.01,
    });
    let blank_eval = evaluate_blanks(
        &[
            BlankObservation {
                result_id: "BLK-1".to_string(),
                sample_id: "BLK-S1".to_string(),
                stage: BlankStage::Coarse,
                lab: "ALS-Primary".to_string(),
                lab_batch: "B001".to_string(),
                value: AssayValue::BelowDetection { limit: 5.0e-9 },
                preceding_sample_grade: Some(PrecedingSampleGrade::Measured(1.0e-6)),
            },
            BlankObservation {
                result_id: "BLK-2".to_string(),
                sample_id: "BLK-S2".to_string(),
                stage: BlankStage::Pulp,
                lab: "ALS-Primary".to_string(),
                lab_batch: "B001".to_string(),
                value: AssayValue::Measured {
                    mass_fraction: 4.0e-8,
                },
                preceding_sample_grade: Some(PrecedingSampleGrade::Measured(1.0e-6)),
            },
        ],
        &blank_rule,
    )
    .expect("live blank eval");

    // 3. Live Duplicate evaluation.
    let dup_pairs: Vec<DuplicatePair> = (0..6)
        .map(|i| DuplicatePair {
            original_sample_id: format!("O{i}"),
            duplicate_sample_id: format!("D{i}"),
            stage: DuplicateStage::CoarseReject,
            grade_band: Some("1-5ppm".to_string()),
            original: AssayValue::Measured {
                mass_fraction: 2.0e-6 + (i as f64) * 1.0e-7,
            },
            duplicate: AssayValue::Measured {
                mass_fraction: 2.05e-6 + (i as f64) * 1.0e-7,
            },
            original_sample_type: Some(SampleType::HalfCore),
            duplicate_sample_type: Some(SampleType::HalfCore),
            original_mass_g: Some(2500.0),
            duplicate_mass_g: Some(2480.0),
        })
        .collect();
    let dup_rep = evaluate_duplicates(&dup_pairs, &DuplicatePolicy::default()).expect("dup eval");

    // 4. Live Insertion rates (with missing field duplicate to verify E168 gap synthesis!).
    let ins_rates = compute_insertion_rates(
        &[
            fmine_drill::records::Sample {
                sample_id: "R1".to_string(),
                interval_ids: vec!["INT-1".to_string()],
                mass_g: Some(2500.0),
                sample_type: SampleType::HalfCore,
                splitter: "diamond_saw".to_string(),
                custody: Vec::new(),
            },
            fmine_drill::records::Sample {
                sample_id: "C1".to_string(),
                interval_ids: Vec::new(),
                mass_g: Some(100.0),
                sample_type: SampleType::QcMaterial,
                splitter: "packet".to_string(),
                custody: Vec::new(),
            },
        ],
        &[fmine_drill::records::QcInsertion {
            insertion_id: "QC1".to_string(),
            sample_id: "C1".to_string(),
            kind: fmine_drill::records::QcKind::Crm {
                crm_id: "OREAS-264".to_string(),
                certified_mass_fraction: 2.0e-6,
                sigma_c: 0.08e-6,
                sigma_l: 0.12e-6,
                certificate_source: "OREAS-264 cert v1".to_string(),
            },
        }],
        &InsertionTargetSpec::textbook_policy_default(),
    )
    .expect("insertion rates");

    // 5. Live sequential CRM monitor with dispersion-collapse integrity alarm.
    let mut mon = CrmSequentialMonitor::new(CrmMonitorConfig::with_reference(
        CrmStreamKey {
            crm_id: "OREAS-264".to_string(),
            lab: "ALS-Primary".to_string(),
            method: AssayMethod::FireAssayAas,
            sd_basis: SdBasis::CertifiedWithinLabSigmaC,
        },
        CrmReferenceParams {
            certified_mean: 2.0e-6,
            reference_sd: 0.08e-6,
        },
    ))
    .expect("monitor");
    for step in 0..16 {
        let tiny = if step % 2 == 0 { 0.004e-6 } else { -0.004e-6 };
        let _ = mon
            .observe(CrmStreamValue {
                measured: 2.0e-6 + tiny,
            })
            .expect("observe");
    }
    let mon_snap = mon.snapshot();
    assert!(mon_snap.dispersion_collapse_step.is_some());

    let req = DataVerificationIndexRequest {
        property_name: "Carlin-Style Synthetic Gold".to_string(),
        effective_date: "2026-06-30".to_string(),
        standard: TechnicalReportStandard::Ni43101Proposed,
        evidence_cut_id: "cut-2026Q2".to_string(),
        signed_originals: vec![
            CasOriginalRef {
                ref_id: "CERT-01".to_string(),
                kind: OriginalArtifactKind::LabCertificate,
                locator: "cas/cert_01.pdf".to_string(),
                sha256_hex: "1".repeat(64),
            },
            CasOriginalRef {
                ref_id: "SURV-01".to_string(),
                kind: OriginalArtifactKind::SurveyReport,
                locator: "cas/surv_01.csv".to_string(),
                sha256_hex: "2".repeat(64),
            },
            CasOriginalRef {
                ref_id: "UMP-01".to_string(),
                kind: OriginalArtifactKind::UmpireCertificate,
                locator: "cas/ump_01.pdf".to_string(),
                sha256_hex: "3".repeat(64),
            },
            CasOriginalRef {
                ref_id: "VISIT-01".to_string(),
                kind: OriginalArtifactKind::SiteVisitMemo,
                locator: "cas/visit_01.md".to_string(),
                sha256_hex: "4".repeat(64),
            },
        ],
        database_audits: vec![DrillholeDatabaseAudit {
            audit_id: "AUD-1".to_string(),
            holes_audited: 50,
            collars_verified: 50,
            surveys_verified: 450,
            assays_verified: 8200,
            rejected_rows: 0,
            composite_support_mismatches: 0,
            original_refs: vec!["CERT-01".to_string(), "SURV-01".to_string()],
            notes: "Full Corebox lineage verification".to_string(),
        }],
        qaqc_campaigns: vec![
            QaqcCampaignEvidence {
                campaign_id: "2025-2026-DD".to_string(),
                qaqc_regime: QaqcRegime::ModernFull,
                disclosure_depth: DisclosureDepth::CountsAndFailureRates,
                insertion_rates: Some(ins_rates),
                crm_summaries: vec![CrmEvidenceSummary::from_evaluation(
                    "OREAS-264",
                    "ALS-Primary",
                    &crm_eval,
                )],
                blank_summaries: vec![BlankEvidenceSummary::from_evaluation(&blank_eval)],
                duplicate_summaries: vec![DuplicateEvidenceSummary::from_report(
                    "coarse_reject",
                    "1-5ppm",
                    &dup_rep,
                )],
                monitor_summaries: vec![MonitorEvidenceSummary::from_snapshot(&mon_snap, None)],
                per_lab_failures: vec![LabFailureSummary {
                    lab: "ALS-Primary".to_string(),
                    crm_total: 6,
                    crm_failures: 0,
                    blank_total: 2,
                    blank_failures: 1,
                    failed_batches_cascaded: 1,
                }],
                control_absent_periods: vec![ControlAbsentRecord {
                    period_label: "2012Q1".to_string(),
                    control_kind: "coarse_blank".to_string(),
                    routine_samples_affected: 320,
                }],
                total_batches: 20,
                batches_without_signed_certificate: 1,
                original_refs: vec!["CERT-01".to_string()],
            },
            QaqcCampaignEvidence {
                campaign_id: "LEGACY-RULES-ONLY".to_string(),
                qaqc_regime: QaqcRegime::LegacyPartial,
                disclosure_depth: DisclosureDepth::RulesOnly,
                insertion_rates: None,
                crm_summaries: Vec::new(),
                blank_summaries: Vec::new(),
                duplicate_summaries: Vec::new(),
                monitor_summaries: Vec::new(),
                per_lab_failures: Vec::new(),
                control_absent_periods: Vec::new(),
                total_batches: 10,
                batches_without_signed_certificate: 3,
                original_refs: Vec::new(),
            },
        ],
        umpire_batches: vec![UmpireBatchSummary {
            batch_id: "UMP-Q2".to_string(),
            primary_lab: "ALS-Primary".to_string(),
            umpire_lab: "SGS-Umpire".to_string(),
            analyte: "Au".to_string(),
            method: "FA-AAS".to_string(),
            pair_count: 100,
            mean_bias_pct: -0.4,
            bias_ci_lower_pct: -1.2,
            bias_ci_upper_pct: 0.4,
            cv_avr_pct: 5.8,
            original_refs: vec!["UMP-01".to_string()],
        }],
        density_and_survey: Some(DensityAndSurveyVerification {
            density_samples_total: 200,
            density_lab_checks: 6, // 3.0% < 5% E382 threshold -> triggers gap!
            grade_samples_total: 8200,
            nominal_density_spacing_m: 90.0,
            optimal_grid_spacing_m: 50.0, // E383 sparsity -> triggers gap!
            survey_holes_total: 50,
            survey_holes_checked: 50,
            survey_instrument: "Gyro".to_string(),
            original_refs: vec!["SURV-01".to_string()],
        }),
        site_visits: vec![SiteVisitRecord {
            visit_date: "2026-05-10".to_string(),
            qp_role: "Independent QP (Geology)".to_string(),
            scope_observed: "Core shed & lab audit".to_string(),
            check_samples_collected: 12,
            original_refs: vec!["VISIT-01".to_string()],
        }],
        check_sampling: vec![CheckSamplingRecord {
            campaign_id: "2025-2026-DD".to_string(),
            sample_count: 12,
            analyte: "Au".to_string(),
            method: "FA-AAS".to_string(),
            mean_relative_diff_pct: 1.1,
            original_refs: vec!["VISIT-01".to_string()],
        }],
        audit_findings: vec![AuditFinding {
            finding_id: "E164-DQO".to_string(),
            category: FindingCategory::MissingDataQualityObjectives,
            severity: FindingSeverity::Moderate,
            summary: "Legacy campaign lacked documented Data Quality Objectives (E164)".to_string(),
            resolved: false,
            resolution_note: None,
            source_ref: Some("VISIT-01".to_string()),
        }],
    };

    let idx_a = build_data_verification_index(&req).expect("build index A");
    let idx_b = build_data_verification_index(&req).expect("build index B");
    assert_eq!(idx_a.digest_hex, idx_b.digest_hex);
    assert!(!idx_a.reserve_disclosure_claim());
    assert!(!idx_a.execution_authorized());
    assert!(!idx_a.geotechnical_signoff());
    assert!(!idx_a.regulatory_compliance_claim());
    assert!(
        idx_a
            .draft_standard_banner
            .expect("G-HALL-2 draft banner")
            .contains("DRAFT STANDARD")
    );
    assert_eq!(idx_a.item_sections.len(), 4);
    // Verify synthesized §24.5 gaps: rules-only, no-cert share, absent controls, integrity alarm,
    // E382 density check < 5%, E383 density grid sparsity.
    let gap_ids: Vec<&str> = idx_a
        .synthesized_gaps
        .iter()
        .map(|g| g.finding_id.as_str())
        .collect();
    assert!(gap_ids.contains(&"GAP-RULES-ONLY-LEGACY-RULES-ONLY"));
    assert!(gap_ids.contains(&"GAP-DENSITY-LAB-CHECK-E382"));
    assert!(gap_ids.contains(&"GAP-DENSITY-GRID-E383"));
    assert!(gap_ids.iter().any(|id| id.starts_with("GAP-INTEGRITY-")));
    assert!(gap_ids.iter().any(|id| id.starts_with("GAP-NO-CERT-")));

    let md = render_verification_index_markdown(&idx_a);
    assert!(md.contains("DRAFT STANDARD — NOT IN FORCE"));
    assert!(md.contains("## Item 10"));
    assert!(md.contains("## Item 11"));
    assert!(md.contains("## Item 12"));
    assert!(md.contains("## Item 14"));
    assert!(md.contains("OREAS-264"));
    assert!(md.contains("E164-DQO"));
    assert!(md.contains(&"1".repeat(64)));

    // Verify all 4 code standards render cleanly with proper draft/non-draft behavior.
    for std_kind in [
        TechnicalReportStandard::Ni43101Current,
        TechnicalReportStandard::Sk1300,
        TechnicalReportStandard::Jorc2012Table1,
    ] {
        let mut r2 = req.clone();
        r2.standard = std_kind;
        let idx2 = build_data_verification_index(&r2).expect("build standard");
        assert!(idx2.draft_standard_banner.is_none());
        let md2 = render_verification_index_markdown(&idx2);
        assert!(!md2.contains("DRAFT STANDARD"));
        assert!(!md2.is_empty());
    }

    // Guard G-HALL-1 refuses software classification labels.
    let g1 = refuse_classification_label(&ClassificationLabelAttempt {
        proposed_label: "Measured".to_string(),
    });
    assert_eq!(
        g1.to_refusal().code,
        RefusalCode::OutsidePhysicalDomain109.as_str()
    );

    // Dangling CAS original ref refuses with FMINE-REF-208.
    let mut bad_ref = req.clone();
    bad_ref.database_audits[0]
        .original_refs
        .push("MISSING-REF".to_string());
    let err = build_data_verification_index(&bad_ref).expect_err("dangling CAS ref must refuse");
    assert_eq!(
        err.to_refusal().code,
        RefusalCode::SourceCellUnusable208.as_str()
    );

    // Duplicate campaign_id, audit_id, or finding_id must refuse.
    let mut dup_camp = req.clone();
    dup_camp.qaqc_campaigns[1].campaign_id = dup_camp.qaqc_campaigns[0].campaign_id.clone();
    assert!(build_data_verification_index(&dup_camp).is_err());

    let mut dup_find = req.clone();
    let f0 = dup_find.audit_findings[0].clone();
    dup_find.audit_findings.push(f0);
    assert!(build_data_verification_index(&dup_find).is_err());

    // Non-finite embedded QA/QC summary value (NaN in CRM bias or duplicate CV) must refuse.
    let mut nan_crm = req.clone();
    nan_crm.qaqc_campaigns[0].crm_summaries[0].bias.bias = f64::NAN;
    assert!(build_data_verification_index(&nan_crm).is_err());

    let mut nan_dup = req.clone();
    nan_dup.qaqc_campaigns[0].duplicate_summaries[0].cv_avr_pct = f64::NAN;
    assert!(build_data_verification_index(&nan_dup).is_err());

    // Markdown table cells escape pipe/backtick characters and unlinked originals affect digest.
    let mut md_escape = req;
    md_escape.signed_originals[0].locator = "cas/cert|a`b.pdf".to_string();
    let escaped_idx = build_data_verification_index(&md_escape).expect("escaped locator valid");
    assert_ne!(escaped_idx.digest_hex, idx_a.digest_hex);
    let escaped_md = render_verification_index_markdown(&escaped_idx);
    assert!(escaped_md.contains("cas/cert\\|a'b.pdf"));
}
