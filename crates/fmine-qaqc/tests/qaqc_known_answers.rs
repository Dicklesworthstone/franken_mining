//! Known-answer and oracle verification tests for `fmine-qaqc` (`BD-167`, `BD-168`, `BD-282`,
//! `BD-284`; plan §12.1–§12.4, E165–E168, E221–E222, E226, E359–E374, E390, FX-1, FX-2, FX-4, FX-5).

use fmine_core::det;
use fmine_core::rand::{PhiloxStream, RngKey};
use fmine_drill::records::{
    AssayMethod, AssayResult, AssayValue, BlankStage, CertificateRef, DuplicateStage, QaqcRegime,
    QcInsertion, QcKind, Sample, SampleType,
};
use fmine_evidence::RefusalCode;
use fmine_qaqc::QaqcError;
use fmine_qaqc::censored::{
    CensoredDataset, SubstitutionAttempt, fit_censored_lognormal_mle, fit_censored_ros,
    refuse_dl_substitution,
};
use fmine_qaqc::crm::{
    CrmBatchInput, CrmCertificateSpec, CrmObservation, CrmPointStatus, MultiBasisConfig, SdBasis,
    SiteCrmRule, evaluate_crm_all_bases, evaluate_crm_batch,
};
use fmine_qaqc::duplicates::{
    DuplicatePair, DuplicatePolicy, GY_SAFETY_LINE_REL_VAR, GyCalibratedConstants,
    GyCalibrationMethod, GyGeneralInput, GyGradeWindow, GyNomogramRequest, GySamplingStage,
    InformationOption, NomogramEvidence, NomogramValidity, NuggetSplitInput,
    StageDecompositionStatus, THOMPSON_HOWARTH_CAVEAT, ThirtyPiecesDiameter, ThirtyPiecesInput,
    calibrate_thirty_pieces, decompose_duplicate_stages, evaluate_duplicates, evaluate_gy_nomogram,
    evaluate_nugget_split, gy_general_fse_variance, reference_precision_policy_defaults,
};
use fmine_qaqc::insertion::{
    BlankCarryoverThresholds, BlankObservation, BlankStatus, CampaignQaqcInput,
    InsertionDenominator, InsertionTargetSpec, SiteBlankRule, TargetBasis, compute_insertion_rates,
    evaluate_blanks, evaluate_campaign_qaqc,
};

fn next_std_normal(rng: &mut PhiloxStream) -> f64 {
    let u1 = rng.next_f64_open01();
    let u2 = rng.next_f64_open01();
    det::sqrt(-2.0 * det::ln(u1)) * det::cos(2.0 * std::f64::consts::PI * u2)
}

fn routine_sample(id: &str, interval: &str, sample_type: SampleType, mass_g: f64) -> Sample {
    Sample {
        sample_id: id.to_string(),
        interval_ids: vec![interval.to_string()],
        mass_g: Some(mass_g),
        sample_type,
        splitter: "riffle".to_string(),
        custody: Vec::new(),
    }
}

fn qc_sample(id: &str) -> Sample {
    Sample {
        sample_id: id.to_string(),
        interval_ids: Vec::new(),
        mass_g: Some(100.0),
        sample_type: SampleType::QcMaterial,
        splitter: "lab".to_string(),
        custody: Vec::new(),
    }
}

fn result(
    id: &str,
    sample_id: &str,
    lab: &str,
    batch: &str,
    method: AssayMethod,
    value: AssayValue,
    received_at_ns: i64,
) -> AssayResult {
    AssayResult {
        result_id: id.to_string(),
        sample_id: sample_id.to_string(),
        dispatch_id: "D-01".to_string(),
        analyte: "Au".to_string(),
        method,
        value,
        lab: lab.to_string(),
        lab_batch: batch.to_string(),
        certificate: CertificateRef::LimsDirectNoCertificate,
        qaqc_regime: QaqcRegime::Current,
        received_at_ns,
    }
}

#[test]
fn test_fx2_crm_multi_basis_canmet_and_bias_interval() {
    // Abzalov AMG Ex. 10.1.1.a (FX-2, E364-E366), expressed as mass fraction (% Fe / 100):
    // mu = 60.73%, sigma_C = 0.09% (within-lab), sigma_L = 0.20% (between-lab).
    let raw_pct = [
        60.94, 60.99, 61.04, 61.06, 61.06, 61.09, 61.10, 61.14, 61.21, 61.24,
    ];
    let spec = CrmCertificateSpec {
        crm_id: "FX2-Fe-CRM".to_string(),
        analyte: "Fe".to_string(),
        method: AssayMethod::FusionXrf,
        certified_mean: 0.6073,
        sigma_c: 0.0009,
        sigma_l: 0.0020,
        certificate_source: "Abzalov AMG Ex. 10.1.1.a (FX-2)".to_string(),
    };
    let observations: Vec<CrmObservation> = raw_pct
        .iter()
        .enumerate()
        .map(|(i, &pct)| CrmObservation {
            result_id: format!("R-{i:02}"),
            sample_id: format!("S-CRM-{i:02}"),
            lab: "LAB-A".to_string(),
            lab_batch: "B-FX2".to_string(),
            value: AssayValue::Measured {
                mass_fraction: pct / 100.0,
            },
            received_at_ns: (i as i64 + 1) * 1_000,
        })
        .collect();

    let input = CrmBatchInput { spec, observations };
    let cmp = evaluate_crm_all_bases(&input, &MultiBasisConfig::default()).expect("FX-2 evaluates");

    // E366: on sigma_C -> 10/10 warnings (> 2 sigma_C), 8/10 failures (> 3 sigma_C).
    let sig_c = cmp.on_sigma_c.as_ref().expect("sigma_c view");
    assert_eq!(sig_c.sd_basis, SdBasis::CertifiedWithinLabSigmaC);
    assert_eq!(sig_c.warning_count, 10);
    assert_eq!(sig_c.single_failure_count, 8);
    assert!(!sig_c.passed);

    // E366: on sigma_L -> 3/10 warnings (> 2 sigma_L), 0/10 failures (> 3 sigma_L).
    let sig_l = cmp.on_sigma_l.as_ref().expect("sigma_l view");
    assert_eq!(sig_l.sd_basis, SdBasis::CertifiedBetweenLabSigmaL);
    assert_eq!(sig_l.warning_count, 3);
    assert_eq!(sig_l.single_failure_count, 0);
    assert!(sig_l.passed);

    // E365: CANMET batch mean passes: |m - mu| = 0.00357 <= 2 * sqrt(0.0020^2 + S_W^2 / 10) = 0.004042.
    let canmet = cmp.canmet.as_ref().expect("canmet view");
    assert!(canmet.passed);
    let canmet_detail = canmet.canmet.expect("canmet detail");
    assert!(canmet_detail.passed);
    assert!((canmet_detail.abs_bias - 0.00357).abs() < 1e-6);
    assert!((canmet_detail.tolerance_limit - 0.004_042_0).abs() < 1e-5);

    // E366: Mandatory bias interval beside pass/fail detects ~12.3 SE certain positive bias!
    let bias = &cmp.bias;
    assert_eq!(bias.n, 10);
    assert!((bias.bias - 0.00357).abs() < 1e-6);
    let t = bias.t_statistic.expect("t statistic");
    assert!((t - 12.28).abs() < 0.15, "expected ~12.28 SE bias, got {t}");
    assert!(bias.excludes_zero, "95% Student-t CI must exclude 0");
    assert!(bias.ci_lower.expect("ci_lower") > 0.0028);
    assert!(bias.ci_upper.expect("ci_upper") < 0.0044);

    // G-CENS: a censored CRM observation is refused, never substituted with DL/2.
    let mut bad_input = input;
    bad_input.observations[0].value = AssayValue::BelowDetection { limit: 0.0001 };
    let err = evaluate_crm_batch(
        &bad_input,
        &SiteCrmRule::single_value_default(SdBasis::CertifiedWithinLabSigmaC),
    )
    .expect_err("censored CRM refused");
    assert_eq!(
        err.to_refusal().code,
        RefusalCode::SourceCellUnusable208.as_str()
    );
}

#[test]
fn test_site_crm_rules_kibali_loulo_ngm_and_grade_reassay_windows() {
    // High-grade Au CRM (2.0 ppm = 2e-6, sigma_c = 0.1e-6).
    // Two consecutive results at +2.3 SD (neither exceeds 3 SD).
    let make_input = |mu: f64, sd: f64, zs: &[f64]| CrmBatchInput {
        spec: CrmCertificateSpec {
            crm_id: "OREAS-264".to_string(),
            analyte: "Au".to_string(),
            method: AssayMethod::FireAssayAas,
            certified_mean: mu,
            sigma_c: sd,
            sigma_l: sd * 2.0,
            certificate_source: "OREAS cert".to_string(),
        },
        observations: zs
            .iter()
            .enumerate()
            .map(|(i, &z)| CrmObservation {
                result_id: format!("R{i}"),
                sample_id: format!("S{i}"),
                lab: "ALS".to_string(),
                lab_batch: "B1".to_string(),
                value: AssayValue::Measured {
                    mass_fraction: mu + z * sd,
                },
                received_at_ns: i as i64 * 100,
            })
            .collect(),
    };

    let two_consec_high = make_input(2.0e-6, 0.1e-6, &[0.2, 2.3, 2.4, 0.1]);
    let kibali = evaluate_crm_batch(
        &two_consec_high,
        &SiteCrmRule::kibali_v1(SdBasis::CertifiedWithinLabSigmaC),
    )
    .expect("kibali");
    let loulo = evaluate_crm_batch(
        &two_consec_high,
        &SiteCrmRule::loulo_v1(SdBasis::CertifiedWithinLabSigmaC),
    )
    .expect("loulo");
    let ngm = evaluate_crm_batch(
        &two_consec_high,
        &SiteCrmRule::ngm_v1(SdBasis::CertifiedWithinLabSigmaC),
    )
    .expect("ngm");

    // Kibali (single > 3 SD) and Loulo (3 consec > 2 SD) pass; NGM (2 consec > 2 SD same side) fails!
    assert!(kibali.passed);
    assert!(loulo.passed);
    assert!(!ngm.passed);
    assert_eq!(ngm.consecutive_failure_count, 1);
    assert_eq!(ngm.points[2].status, CrmPointStatus::FailConsecutive);
    assert_eq!(ngm.reassay_bracket_each_side, Some(10)); // High-grade (>= 1 ppm) tier -> ±10

    // Low-grade CRM (0.4 ppm = 0.4e-6) triggers the ±5 bracket on NGM failure (E166).
    let two_consec_low = make_input(0.4e-6, 0.02e-6, &[2.2, 2.5]);
    let ngm_low = evaluate_crm_batch(
        &two_consec_low,
        &SiteCrmRule::ngm_v1(SdBasis::CertifiedWithinLabSigmaC),
    )
    .expect("ngm low");
    assert!(!ngm_low.passed);
    assert_eq!(ngm_low.reassay_bracket_each_side, Some(5));

    // Opposite-side consecutive warnings (+2.3, -2.3) do NOT trigger same-side NGM bias failure.
    let opposite_sides = make_input(2.0e-6, 0.1e-6, &[2.3, -2.3, 2.2]);
    let ngm_opp = evaluate_crm_batch(
        &opposite_sides,
        &SiteCrmRule::ngm_v1(SdBasis::CertifiedWithinLabSigmaC),
    )
    .expect("ngm opposite");
    assert!(ngm_opp.passed);

    // Three consecutive > +2 SD triggers Loulo failure (E222) while Kibali still passes.
    let three_consec = make_input(2.0e-6, 0.1e-6, &[2.1, 2.2, 2.3]);
    let loulo3 = evaluate_crm_batch(
        &three_consec,
        &SiteCrmRule::loulo_v1(SdBasis::CertifiedWithinLabSigmaC),
    )
    .expect("loulo 3");
    let kibali3 = evaluate_crm_batch(
        &three_consec,
        &SiteCrmRule::kibali_v1(SdBasis::CertifiedWithinLabSigmaC),
    )
    .expect("kibali 3");
    assert!(!loulo3.passed);
    assert_eq!(loulo3.consecutive_failure_count, 1);
    assert!(kibali3.passed);
}

#[test]
fn test_blanks_carlin_and_vs_turquoise_ridge_or_and_g_cens() {
    // C2 §4.5b: Carlin blank rule uses "and"; Turquoise Ridge uses "either/or".
    // Blank = 0.025 ppm (2.5e-8), preceding sample = 5.0 ppm (5e-6).
    // abs_threshold = 0.020 ppm (2.0e-8), carryover_fraction = 0.01 (1% of 5 ppm = 0.050 ppm = 5.0e-8).
    // Blank exceeds abs_threshold (2.5e-8 > 2.0e-8) but does NOT exceed 1% carryover (2.5e-8 <= 5.0e-8).
    let blanks = vec![
        BlankObservation {
            result_id: "RB-1".to_string(),
            sample_id: "SB-1".to_string(),
            stage: BlankStage::Coarse,
            lab: "ALS".to_string(),
            lab_batch: "B-1".to_string(),
            value: AssayValue::BelowDetection { limit: 5.0e-9 },
            preceding_sample_mass_fraction: Some(5.0e-6),
        },
        BlankObservation {
            result_id: "RB-2".to_string(),
            sample_id: "SB-2".to_string(),
            stage: BlankStage::Coarse,
            lab: "ALS".to_string(),
            lab_batch: "B-1".to_string(),
            value: AssayValue::Measured {
                mass_fraction: 2.5e-8,
            },
            preceding_sample_mass_fraction: Some(5.0e-6),
        },
        BlankObservation {
            result_id: "RB-3".to_string(),
            sample_id: "SB-3".to_string(),
            stage: BlankStage::Pulp,
            lab: "ALS".to_string(),
            lab_batch: "B-2".to_string(),
            value: AssayValue::AboveUpper { limit: 1.0e-4 },
            preceding_sample_mass_fraction: Some(1.0e-6),
        },
    ];

    let thresholds = BlankCarryoverThresholds {
        abs_threshold: 2.0e-8,
        carryover_fraction: 0.01,
    };
    let carlin_rule = SiteBlankRule::carlin_and_v1(thresholds);
    let tr_rule = SiteBlankRule::turquoise_ridge_or_v1(thresholds);

    let carlin_eval = evaluate_blanks(&blanks, &carlin_rule).expect("carlin blank eval");
    let tr_eval = evaluate_blanks(&blanks, &tr_rule).expect("tr blank eval");

    // RB-1 (<DL) passes cleanly without DL/2 substitution in both rules.
    assert_eq!(
        carlin_eval.verdicts[0].status,
        BlankStatus::PassBelowDetection
    );
    assert_eq!(carlin_eval.verdicts[0].measured_mass_fraction, None);
    assert_eq!(carlin_eval.below_detection_count, 1);

    // RB-2 passes Carlin ("and") and fails Turquoise Ridge ("either/or")!
    assert_eq!(carlin_eval.verdicts[1].status, BlankStatus::PassMeasured);
    assert_eq!(tr_eval.verdicts[1].status, BlankStatus::FailContamination);

    // RB-3 (AboveUpper) fails contamination in both.
    assert_eq!(
        carlin_eval.verdicts[2].status,
        BlankStatus::FailContamination
    );
    assert_eq!(carlin_eval.failure_count, 1);
    assert_eq!(tr_eval.failure_count, 2);
}

#[test]
fn test_insertion_rates_denominators_control_absent_and_campaign_cascade() {
    // Build 20 routine samples + 1 CRM + 1 blank + 1 unfit quarter-core duplicate of half-core original.
    // Note: field_duplicate = 1/20 = 5% of routine, 1/23 = 4.35% of all submitted (E226 denominator clarity!).
    // Umpires = 0 -> listed in absent_controls (E168).
    let mut samples = Vec::new();
    for i in 1..=20 {
        samples.push(routine_sample(
            &format!("S-{i:02}"),
            &format!("INT-{i:02}"),
            SampleType::HalfCore,
            2000.0,
        ));
    }
    samples.push(qc_sample("QC-CRM-1"));
    samples.push(qc_sample("QC-BLK-1"));
    samples.push(routine_sample(
        "QC-DUP-1",
        "INT-01",
        SampleType::QuarterCore,
        1000.0,
    ));

    let insertions = vec![
        QcInsertion {
            insertion_id: "INS-1".to_string(),
            sample_id: "QC-CRM-1".to_string(),
            kind: QcKind::Crm {
                crm_id: "CRM-A".to_string(),
                certified_mass_fraction: 1.0e-6,
                sigma_c: 0.05e-6,
                sigma_l: 0.10e-6,
                certificate_source: "cert-A".to_string(),
            },
        },
        QcInsertion {
            insertion_id: "INS-2".to_string(),
            sample_id: "QC-BLK-1".to_string(),
            kind: QcKind::Blank {
                stage: BlankStage::Coarse,
            },
        },
        QcInsertion {
            insertion_id: "INS-3".to_string(),
            sample_id: "QC-DUP-1".to_string(),
            kind: QcKind::Duplicate {
                stage: DuplicateStage::Field,
                original_sample_id: "S-01".to_string(),
            },
        },
    ];

    let mut targets_routine = InsertionTargetSpec::textbook_policy_default();
    assert!(matches!(
        targets_routine.basis,
        TargetBasis::PolicyDefault { .. }
    ));
    let rate_routine =
        compute_insertion_rates(&samples, &insertions, &targets_routine).expect("routine denom");
    assert_eq!(rate_routine.routine_sample_count, 20);
    assert_eq!(rate_routine.all_submitted_sample_count, 23);
    assert_eq!(rate_routine.crm.denominator, 20);
    assert!((rate_routine.crm.achieved_pct.expect("crm pct") - 5.0).abs() < 1e-12);
    assert!(rate_routine.absent_controls.contains(&"umpire".to_string()));
    assert!(
        rate_routine
            .absent_controls
            .contains(&"pulp_duplicate".to_string())
    );

    targets_routine.denominator = InsertionDenominator::AllSubmittedSamples;
    let rate_all =
        compute_insertion_rates(&samples, &insertions, &targets_routine).expect("all denom");
    assert_eq!(rate_all.crm.denominator, 23);
    assert!((rate_all.crm.achieved_pct.expect("crm pct") - (100.0 / 23.0)).abs() < 1e-12);

    // Campaign evaluation: CRM fails (> 3 sigma_c) in batch B-FAIL; routine samples S-01 and S-02
    // in B-FAIL cascade to FailedBatchCascade (PO-CORE-4), while S-03 in B-OK is untouched.
    let results = vec![
        result(
            "RES-1",
            "S-01",
            "ALS",
            "B-FAIL",
            AssayMethod::FireAssayAas,
            AssayValue::Measured {
                mass_fraction: 1.5e-6,
            },
            100,
        ),
        result(
            "RES-2",
            "S-02",
            "ALS",
            "B-FAIL",
            AssayMethod::FireAssayAas,
            AssayValue::Measured {
                mass_fraction: 2.0e-6,
            },
            110,
        ),
        result(
            "RES-CRM",
            "QC-CRM-1",
            "ALS",
            "B-FAIL",
            AssayMethod::FireAssayAas,
            AssayValue::Measured {
                mass_fraction: 1.25e-6, // +5 sigma_c failure!
            },
            120,
        ),
        result(
            "RES-BLK",
            "QC-BLK-1",
            "ALS",
            "B-OK",
            AssayMethod::FireAssayAas,
            AssayValue::BelowDetection { limit: 5.0e-9 },
            130,
        ),
        result(
            "RES-3",
            "S-03",
            "ALS",
            "B-OK",
            AssayMethod::FireAssayAas,
            AssayValue::Measured {
                mass_fraction: 0.8e-6,
            },
            140,
        ),
    ];

    let report = evaluate_campaign_qaqc(&CampaignQaqcInput {
        analyte: "Au".to_string(),
        method: AssayMethod::FireAssayAas,
        samples: &samples,
        insertions: &insertions,
        results: &results,
        insertion_targets: InsertionTargetSpec::textbook_policy_default(),
        crm_rule: SiteCrmRule::kibali_v1(SdBasis::CertifiedWithinLabSigmaC),
        blank_rule: SiteBlankRule::multiple_of_dl_default(),
    })
    .expect("campaign report");

    assert_eq!(report.unfit_mass_duplicates, vec!["QC-DUP-1".to_string()]);
    assert_eq!(report.failed_batches.len(), 1);
    let fb = &report.failed_batches[0];
    assert_eq!(fb.lab, "ALS");
    assert_eq!(fb.lab_batch, "B-FAIL");
    assert_eq!(
        fb.affected_routine_result_ids,
        vec!["RES-1".to_string(), "RES-2".to_string()]
    );
    assert_eq!(
        fb.affected_routine_sample_ids,
        vec!["S-01".to_string(), "S-02".to_string()]
    );
}

#[test]
fn test_po_core_3_refuses_mixed_methods_and_disaggregates_by_lab() {
    // E167: internal mine lab ("GQ-INTERNAL") has much higher failure rate than accredited ("ALS").
    let samples = vec![
        qc_sample("C1"),
        qc_sample("C2"),
        qc_sample("C3"),
        qc_sample("C4"),
        qc_sample("C5"),
    ];
    let insertions: Vec<QcInsertion> = samples
        .iter()
        .enumerate()
        .map(|(i, s)| QcInsertion {
            insertion_id: format!("I{i}"),
            sample_id: s.sample_id.clone(),
            kind: QcKind::Crm {
                crm_id: "OREAS-264".to_string(),
                certified_mass_fraction: 1.0e-6,
                sigma_c: 0.05e-6,
                sigma_l: 0.10e-6,
                certificate_source: "OREAS".to_string(),
            },
        })
        .collect();

    // PO-CORE-3: mixing FireAssayAas and PhotonAssay in one evaluation must refuse (FMINE-REF-105).
    let mixed = vec![
        result(
            "R1",
            "C1",
            "ALS",
            "B1",
            AssayMethod::FireAssayAas,
            AssayValue::Measured {
                mass_fraction: 1.0e-6,
            },
            10,
        ),
        result(
            "R2",
            "C2",
            "ALS",
            "B1",
            AssayMethod::PhotonAssay,
            AssayValue::Measured {
                mass_fraction: 1.027e-6,
            },
            20,
        ),
    ];
    let err = evaluate_campaign_qaqc(&CampaignQaqcInput {
        analyte: "Au".to_string(),
        method: AssayMethod::FireAssayAas,
        samples: &samples,
        insertions: &insertions,
        results: &mixed,
        insertion_targets: InsertionTargetSpec::textbook_policy_default(),
        crm_rule: SiteCrmRule::kibali_v1(SdBasis::CertifiedWithinLabSigmaC),
        blank_rule: SiteBlankRule::multiple_of_dl_default(),
    })
    .expect_err("PO-CORE-3 must refuse mixed methods");
    assert_eq!(
        err.to_refusal().code,
        RefusalCode::BasisMismatch105.as_str()
    );

    // Per-lab disaggregation: GQ fails 2/2 (100%), ALS fails 0/3 (0%).
    let disagg = vec![
        result(
            "R1",
            "C1",
            "GQ-INTERNAL",
            "B1",
            AssayMethod::FireAssayAas,
            AssayValue::Measured {
                mass_fraction: 1.30e-6,
            },
            10,
        ),
        result(
            "R2",
            "C2",
            "GQ-INTERNAL",
            "B2",
            AssayMethod::FireAssayAas,
            AssayValue::Measured {
                mass_fraction: 0.70e-6,
            },
            20,
        ),
        result(
            "R3",
            "C3",
            "ALS",
            "B3",
            AssayMethod::FireAssayAas,
            AssayValue::Measured {
                mass_fraction: 1.01e-6,
            },
            30,
        ),
        result(
            "R4",
            "C4",
            "ALS",
            "B3",
            AssayMethod::FireAssayAas,
            AssayValue::Measured {
                mass_fraction: 0.99e-6,
            },
            40,
        ),
        result(
            "R5",
            "C5",
            "ALS",
            "B4",
            AssayMethod::FireAssayAas,
            AssayValue::Measured {
                mass_fraction: 1.02e-6,
            },
            50,
        ),
    ];
    let rep = evaluate_campaign_qaqc(&CampaignQaqcInput {
        analyte: "Au".to_string(),
        method: AssayMethod::FireAssayAas,
        samples: &samples,
        insertions: &insertions,
        results: &disagg,
        insertion_targets: InsertionTargetSpec::textbook_policy_default(),
        crm_rule: SiteCrmRule::kibali_v1(SdBasis::CertifiedWithinLabSigmaC),
        blank_rule: SiteBlankRule::multiple_of_dl_default(),
    })
    .expect("disaggregated report");

    assert_eq!(rep.per_lab["GQ-INTERNAL"].crm_failures, 2);
    assert!(
        (rep.per_lab["GQ-INTERNAL"]
            .crm_failure_rate_pct
            .expect("gq rate")
            - 100.0)
            .abs()
            < 1e-12
    );
    assert_eq!(rep.per_lab["ALS"].crm_failures, 0);
    assert!((rep.per_lab["ALS"].crm_failure_rate_pct.expect("als rate") - 0.0).abs() < 1e-12);

    // Regression: simultaneous single (> 3 SD) + consecutive (> 2 SD) trigger under NGM rule must
    // NOT double-count failed CRM observations (> 100% failure rate).
    let two_extreme = vec![
        result(
            "R1",
            "C1",
            "GQ-INTERNAL",
            "B1",
            AssayMethod::FireAssayAas,
            AssayValue::Measured {
                mass_fraction: 1.30e-6,
            },
            10,
        ),
        result(
            "R2",
            "C2",
            "GQ-INTERNAL",
            "B1",
            AssayMethod::FireAssayAas,
            AssayValue::Measured {
                mass_fraction: 1.35e-6,
            },
            20,
        ),
    ];
    let ngm_rep = evaluate_campaign_qaqc(&CampaignQaqcInput {
        analyte: "Au".to_string(),
        method: AssayMethod::FireAssayAas,
        samples: &samples,
        insertions: &insertions,
        results: &two_extreme,
        insertion_targets: InsertionTargetSpec::textbook_policy_default(),
        crm_rule: SiteCrmRule::ngm_v1(SdBasis::CertifiedWithinLabSigmaC),
        blank_rule: SiteBlankRule::multiple_of_dl_default(),
    })
    .expect("ngm extreme report");
    assert_eq!(ngm_rep.per_lab["GQ-INTERNAL"].crm_total, 2);
    assert_eq!(ngm_rep.per_lab["GQ-INTERNAL"].crm_failures, 2);
    assert!(
        (ngm_rep.per_lab["GQ-INTERNAL"]
            .crm_failure_rate_pct
            .expect("rate")
            - 100.0)
            .abs()
            < 1e-12
    );

    // Regression: out-of-order input results + a <DL routine sample immediately before a blank
    // must NOT leak an earlier high-grade routine sample's carryover into Carlin "and" blank rule.
    let samples_co = vec![
        routine_sample("S-HI", "I-1", SampleType::HalfCore, 2000.0),
        routine_sample("S-BDL", "I-2", SampleType::HalfCore, 2000.0),
        qc_sample("QC-B"),
    ];
    let ins_co = vec![QcInsertion {
        insertion_id: "INS-B".to_string(),
        sample_id: "QC-B".to_string(),
        kind: QcKind::Blank {
            stage: BlankStage::Coarse,
        },
    }];
    // Deliberately reverse input order vs timestamps: t=10 high grade (10 ppm), t=20 <DL, t=30 blank (0.025 ppm).
    // Because t=20 cleared preceding grade, t=30 has no preceding grade and exceeds abs_threshold (0.020 ppm) -> FAIL.
    let res_co = vec![
        result(
            "R-BLK",
            "QC-B",
            "ALS",
            "B-CO",
            AssayMethod::FireAssayAas,
            AssayValue::Measured {
                mass_fraction: 2.5e-8,
            },
            30,
        ),
        result(
            "R-BDL",
            "S-BDL",
            "ALS",
            "B-CO",
            AssayMethod::FireAssayAas,
            AssayValue::BelowDetection { limit: 5.0e-9 },
            20,
        ),
        result(
            "R-HI",
            "S-HI",
            "ALS",
            "B-CO",
            AssayMethod::FireAssayAas,
            AssayValue::Measured {
                mass_fraction: 10.0e-6,
            },
            10,
        ),
    ];
    let co_rep = evaluate_campaign_qaqc(&CampaignQaqcInput {
        analyte: "Au".to_string(),
        method: AssayMethod::FireAssayAas,
        samples: &samples_co,
        insertions: &ins_co,
        results: &res_co,
        insertion_targets: InsertionTargetSpec::textbook_policy_default(),
        crm_rule: SiteCrmRule::kibali_v1(SdBasis::CertifiedWithinLabSigmaC),
        blank_rule: SiteBlankRule::carlin_and_v1(BlankCarryoverThresholds {
            abs_threshold: 2.0e-8,
            carryover_fraction: 0.01,
        }),
    })
    .expect("carryover order-invariance report");
    assert_eq!(co_rep.per_lab["ALS"].blank_failures, 1);
}

#[test]
fn test_duplicates_cv_avr_hard_ampd_mass_guard_thompson_howarth_and_field_absent() {
    // Hand-worked exact pairs: (a, b) = (0.00010, 0.00012) and (0.00020, 0.00016):
    // Pair 1: |a-b|/(a+b) = 0.02/0.22 = 1/11 -> sq = 1/121
    // Pair 2: |a-b|/(a+b) = 0.04/0.36 = 1/9  -> sq = 1/81
    // CV_AVR% = 100 * sqrt((2/2) * (1/121 + 1/81)) = 100 * sqrt(202 / 9801) = 14.35622...%
    let exact_pairs = vec![
        DuplicatePair {
            original_sample_id: "S1".to_string(),
            duplicate_sample_id: "D1".to_string(),
            stage: DuplicateStage::Pulp,
            original: AssayValue::Measured {
                mass_fraction: 0.00010,
            },
            duplicate: AssayValue::Measured {
                mass_fraction: 0.00012,
            },
            original_sample_type: Some(SampleType::HalfCore),
            duplicate_sample_type: Some(SampleType::HalfCore),
            original_mass_g: Some(500.0),
            duplicate_mass_g: Some(505.0),
            grade_band: None,
        },
        DuplicatePair {
            original_sample_id: "S2".to_string(),
            duplicate_sample_id: "D2".to_string(),
            stage: DuplicateStage::Pulp,
            original: AssayValue::Measured {
                mass_fraction: 0.00020,
            },
            duplicate: AssayValue::Measured {
                mass_fraction: 0.00016,
            },
            original_sample_type: Some(SampleType::HalfCore),
            duplicate_sample_type: Some(SampleType::HalfCore),
            original_mass_g: Some(500.0),
            duplicate_mass_g: Some(495.0),
            grade_band: None,
        },
    ];
    let rep = evaluate_duplicates(&exact_pairs, &DuplicatePolicy::default()).expect("exact dup");
    let expected_cv = 100.0 * det::sqrt(1.0 / 121.0 + 1.0 / 81.0);
    assert!((rep.cv_avr_pct - expected_cv).abs() < 1e-12);
    assert!((rep.hard_from_cv_pct - expected_cv / det::sqrt(2.0)).abs() < 1e-12);
    assert!((rep.ampd_from_cv_pct - expected_cv * det::sqrt(2.0)).abs() < 1e-12);
    assert!(rep.thompson_howarth.is_none()); // N = 2 < 50 (E369)

    // E374: quarter-core duplicate of half-core original refused in strict mode (FMINE-REF-102).
    let unfit_pair = DuplicatePair {
        original_sample_id: "S-HALF".to_string(),
        duplicate_sample_id: "D-QUARTER".to_string(),
        stage: DuplicateStage::Field,
        original: AssayValue::Measured {
            mass_fraction: 0.00010,
        },
        duplicate: AssayValue::Measured {
            mass_fraction: 0.00011,
        },
        original_sample_type: Some(SampleType::HalfCore),
        duplicate_sample_type: Some(SampleType::QuarterCore),
        original_mass_g: Some(2000.0),
        duplicate_mass_g: Some(1000.0),
        grade_band: None,
    };
    let strict_err = evaluate_duplicates(
        &[unfit_pair],
        &DuplicatePolicy {
            max_relative_mass_diff: 0.25,
            refuse_on_mass_mismatch: true,
        },
    )
    .expect_err("strict E374 refuses quarter-core of half-core");
    assert_eq!(
        strict_err.to_refusal().code,
        RefusalCode::SupportMismatch102.as_str()
    );

    // C4 P12: Porgera-style dataset with pulp/coarse duplicates but ZERO field duplicates sets FieldDuplicateAbsent.
    let decomp = decompose_duplicate_stages(&exact_pairs, &DuplicatePolicy::default())
        .expect("stage decomp");
    assert_eq!(
        decomp.status,
        StageDecompositionStatus::FieldDuplicateAbsent
    );
    assert!(decomp.analytical_rel_var.is_some());
    assert!(decomp.field_incremental_rel_var.is_none());

    // E369 & E370: Thompson-Howarth absent at N=49, present at N=66 with optimistic caveat.
    let mut rng = PhiloxStream::new(RngKey {
        seed_root: 167,
        partition: 1,
        domain: 1,
        var_group: 1,
        realization: 0,
        brick: 0,
    });
    let mut synth_pairs = Vec::new();
    for i in 0..66 {
        let base = 1.0e-6 + (i as f64) * 1.0e-7;
        let sd = 0.08 * base; // 8% proportional precision error
        let a = (base + sd * next_std_normal(&mut rng)).max(1.0e-9);
        let b = (base + sd * next_std_normal(&mut rng)).max(1.0e-9);
        synth_pairs.push(DuplicatePair {
            original_sample_id: format!("O{i}"),
            duplicate_sample_id: format!("P{i}"),
            stage: DuplicateStage::Pulp,
            original: AssayValue::Measured { mass_fraction: a },
            duplicate: AssayValue::Measured { mass_fraction: b },
            original_sample_type: None,
            duplicate_sample_type: None,
            original_mass_g: None,
            duplicate_mass_g: None,
            grade_band: None,
        });
    }
    let rep49 = evaluate_duplicates(&synth_pairs[..49], &DuplicatePolicy::default()).expect("n=49");
    assert!(rep49.thompson_howarth.is_none());

    let rep66 = evaluate_duplicates(&synth_pairs, &DuplicatePolicy::default()).expect("n=66");
    let th = rep66
        .thompson_howarth
        .expect("n=66 produces Thompson-Howarth");
    assert_eq!(th.n_pairs, 66);
    assert_eq!(th.n_bins, 6);
    assert_eq!(th.caveat, THOMPSON_HOWARTH_CAVEAT);
    assert!(
        (th.relative_sd_pct_at_median - 8.0).abs() < 4.5,
        "expected ~8% relative SD, got {}",
        th.relative_sd_pct_at_median
    );

    // FX-4 reference levels registered strictly as PolicyDefault.
    let defaults = reference_precision_policy_defaults();
    assert!(!defaults.is_empty());
    assert!(defaults.iter().all(|r| r.basis.contains("PolicyDefault")));
}

#[test]
fn test_gy_fx1_calibration_nomogram_validity_and_nugget_split() {
    // FX-1 (AMG PDF 141, E361): rel_var = 9.41, mass = 2889 g, sphere_d = 12.25 cm -> d_N = 19.45 cm, K = 316.9.
    let cal_sphere = calibrate_thirty_pieces(&ThirtyPiecesInput {
        relative_variance: 9.41,
        mean_piece_mass_g: 2889.0,
        diameter: ThirtyPiecesDiameter::EquivalentSphereCm(12.25),
        alpha: 1.5,
        calibration_grade: 2.0e-6,
        valid_grade_min: 1.0e-6,
        valid_grade_max: 4.0e-6,
    })
    .expect("FX-1 calibration from sphere diameter");
    assert!((cal_sphere.nominal_diameter_cm - 19.4457).abs() < 0.005);
    assert!((cal_sphere.constants.k - 317.0).abs() < 0.2);
    assert_eq!(
        cal_sphere.constants.method,
        GyCalibrationMethod::ThirtyPieces
    );

    let cal_rounded_dn = calibrate_thirty_pieces(&ThirtyPiecesInput {
        relative_variance: 9.41,
        mean_piece_mass_g: 2889.0,
        diameter: ThirtyPiecesDiameter::NominalCm(19.45),
        alpha: 1.5,
        calibration_grade: 2.0e-6,
        valid_grade_min: 1.0e-6,
        valid_grade_max: 4.0e-6,
    })
    .expect("FX-1 calibration from d_N = 19.45");
    assert!((cal_rounded_dn.constants.k - 316.925_917_861_148).abs() < 1e-9);
    assert_eq!((cal_rounded_dn.constants.k * 10.0).round() / 10.0, 316.9);

    // General Gy form exact hand check: (1/500 - 1/5000) * 100 * 0.5 * 0.25 * 0.8 * 0.2^3
    // = 0.0018 * 10 * 0.008 = 0.000144.
    let gen_var = gy_general_fse_variance(&GyGeneralInput {
        sample_mass_g: 500.0,
        lot_mass_g: 5000.0,
        mineralogical_c_g_per_cm3: 100.0,
        shape_f: 0.5,
        granulometric_g: 0.25,
        liberation_l: 0.8,
        nominal_diameter_cm: 0.2,
    })
    .expect("general Gy");
    assert!((gen_var - 0.000_144).abs() < 1e-12);

    // Nomogram evaluated INSIDE calibration grade -> Derived + WithinValidity.
    let stage_ok = GySamplingStage {
        stage_name: "pulp_split".to_string(),
        nominal_diameter_cm: 0.0075, // 75 um
        lot_mass_g: 1000.0,
        sample_mass_g: 50.0,
        increments: 40,
    };
    let in_band = evaluate_gy_nomogram(&GyNomogramRequest {
        constants: cal_rounded_dn.constants.clone(),
        evaluation_grade: 2.5e-6,
        safety_line_rel_var: GY_SAFETY_LINE_REL_VAR,
        stages: vec![stage_ok.clone()],
    })
    .expect("in-band nomogram");
    assert_eq!(in_band.evidence, NomogramEvidence::Derived);
    assert_eq!(in_band.validity, NomogramValidity::WithinValidity);
    assert!(in_band.protocol_within_policy);

    // E359: Nomogram evaluated OFF its calibration grade -> OutsideValidity!
    let off_band = evaluate_gy_nomogram(&GyNomogramRequest {
        constants: cal_rounded_dn.constants,
        evaluation_grade: 10.0e-6, // far outside [1e-6, 4e-6]
        safety_line_rel_var: GY_SAFETY_LINE_REL_VAR,
        stages: vec![stage_ok.clone()],
    })
    .expect("off-band nomogram");
    assert_eq!(off_band.validity, NomogramValidity::OutsideValidity);
    assert!(!off_band.protocol_within_policy);

    // E362-E363: Assumed K=470, alpha=1.5 -> Speculative; < 30 increments flagged.
    let assumed = GyCalibratedConstants::francois_bongarcon_assumed(GyGradeWindow {
        calibration_grade: 2.0e-6,
        valid_grade_min: 1.0e-6,
        valid_grade_max: 4.0e-6,
    });
    let mut stage_few_inc = stage_ok;
    stage_few_inc.increments = 12;
    let spec_rep = evaluate_gy_nomogram(&GyNomogramRequest {
        constants: assumed,
        evaluation_grade: 2.0e-6,
        safety_line_rel_var: GY_SAFETY_LINE_REL_VAR,
        stages: vec![stage_few_inc],
    })
    .expect("assumed nomogram");
    assert_eq!(spec_rep.evidence, NomogramEvidence::Speculative);
    assert!(spec_rep.stages[0].insufficient_increments);
    assert!(!spec_rep.protocol_within_policy);

    // E390: Nugget split -> sampling share > policy proposes ImproveSamplingProtocol + InfillDrilling.
    let split_dom = evaluate_nugget_split(&NuggetSplitInput {
        duplicate_rel_var_at_composite: 0.13,
        variogram_relative_nugget: 0.20,
        policy_share_threshold: 0.50,
    })
    .expect("sampling dominates nugget");
    assert!(split_dom.sampling_dominates);
    assert!((split_dom.sampling_share - 0.65).abs() < 1e-12);
    assert!((split_dom.geological_nugget - 0.07).abs() < 1e-12);
    assert_eq!(
        split_dom.helm_options,
        vec![
            InformationOption::ImproveSamplingProtocol,
            InformationOption::InfillDrilling
        ]
    );

    let split_geo = evaluate_nugget_split(&NuggetSplitInput {
        duplicate_rel_var_at_composite: 0.04,
        variogram_relative_nugget: 0.20,
        policy_share_threshold: 0.50,
    })
    .expect("geology dominates nugget");
    assert!(!split_geo.sampling_dominates);
    assert_eq!(
        split_geo.helm_options,
        vec![InformationOption::InfillDrilling]
    );

    // Refuses duplicate variance > total variogram nugget (FMINE-REF-104, never clamps negative!).
    let split_bad = evaluate_nugget_split(&NuggetSplitInput {
        duplicate_rel_var_at_composite: 0.25,
        variogram_relative_nugget: 0.20,
        policy_share_threshold: 0.50,
    })
    .expect_err("duplicate var > nugget must refuse");
    assert_eq!(
        split_bad.to_refusal().code,
        RefusalCode::NonIdentifiableAttribution104.as_str()
    );
}

#[test]
fn test_censored_mle_and_ros_recover_seeded_lognormal_and_refuse_dl_half() {
    // G-CENS: DL/2 substitution is unconditionally refused (FMINE-REF-109).
    let sub_err = refuse_dl_substitution(&SubstitutionAttempt { dl_fraction: 0.5 })
        .expect_err("G-CENS must refuse DL/2");
    assert!(matches!(sub_err, QaqcError::SubstitutionRefused { .. }));
    assert_eq!(
        sub_err.to_refusal().code,
        RefusalCode::OutsidePhysicalDomain109.as_str()
    );

    // Seeded 400-sample lognormal (mu_ln = -12.0, sigma_ln = 0.85; median ~ 6.14e-6 = 6.14 ppm)
    // with TWO distinct detection limits (DL1 = 3.0e-6 on even rows, DL2 = 6.0e-6 on odd rows),
    // yielding ~35% left censoring.
    let true_mu = -12.0_f64;
    let true_sigma = 0.85_f64;
    let true_mean = det::exp(true_mu + 0.5 * true_sigma * true_sigma);
    let mut rng = PhiloxStream::new(RngKey {
        seed_root: 168_2026,
        partition: 1,
        domain: 1,
        var_group: 1,
        realization: 0,
        brick: 0,
    });

    let mut values = Vec::with_capacity(400);
    let mut naive_dl_half = Vec::with_capacity(400);
    for i in 0..400 {
        let x = det::exp(true_mu + true_sigma * next_std_normal(&mut rng));
        let dl = if i % 2 == 0 { 3.0e-6 } else { 6.0e-6 };
        if x < dl {
            values.push(AssayValue::BelowDetection { limit: dl });
            naive_dl_half.push(0.5 * dl);
        } else {
            values.push(AssayValue::Measured { mass_fraction: x });
            naive_dl_half.push(x);
        }
    }
    let dataset = CensoredDataset { values };

    let mle = fit_censored_lognormal_mle(&dataset).expect("MLE converges");
    assert_eq!(mle.n_total, 400);
    assert!(mle.n_censored > 100 && mle.n_censored < 200);
    assert!(!mle.high_censoring_warning);
    assert!(
        (mle.mu_log - true_mu).abs() < 0.12,
        "MLE mu_log={} vs true={true_mu}",
        mle.mu_log
    );
    assert!(
        (mle.sigma_log - true_sigma).abs() < 0.12,
        "MLE sigma_log={} vs true={true_sigma}",
        mle.sigma_log
    );
    assert!(
        ((mle.arithmetic_mean - true_mean) / true_mean).abs() < 0.15,
        "MLE mean={} vs true={true_mean}",
        mle.arithmetic_mean
    );

    let ros = fit_censored_ros(&dataset).expect("ROS succeeds");
    assert_eq!(ros.n_total, 400);
    assert!(ros.r_squared > 0.97, "ROS R^2 = {}", ros.r_squared);
    assert!(
        (ros.mu_log - true_mu).abs() < 0.15,
        "ROS mu_log={} vs true={true_mu}",
        ros.mu_log
    );
    assert!(
        (ros.sigma_log - true_sigma).abs() < 0.15,
        "ROS sigma_log={} vs true={true_sigma}",
        ros.sigma_log
    );
    assert!(
        ((ros.mean - true_mean) / true_mean).abs() < 0.15,
        "ROS mean={} vs true={true_mean}",
        ros.mean
    );

    // Control check: naive DL/2 substitution produces badly distorted log-SD (artificially low variance).
    let naive_log_mean =
        naive_dl_half.iter().map(|&v| det::ln(v)).sum::<f64>() / (naive_dl_half.len() as f64);
    let naive_log_sd = det::sqrt(
        naive_dl_half
            .iter()
            .map(|&v| {
                let d = det::ln(v) - naive_log_mean;
                d * d
            })
            .sum::<f64>()
            / ((naive_dl_half.len() - 1) as f64),
    );
    assert!(
        (naive_log_sd - true_sigma).abs() > (mle.sigma_log - true_sigma).abs(),
        "MLE ({}) must beat naive DL/2 ({}) on log-SD recovery (true={true_sigma})",
        mle.sigma_log,
        naive_log_sd
    );

    // Refuses AboveUpper without silently dropping.
    let right_cens = CensoredDataset {
        values: vec![
            AssayValue::Measured {
                mass_fraction: 1.0e-5,
            },
            AssayValue::Measured {
                mass_fraction: 2.0e-5,
            },
            AssayValue::AboveUpper { limit: 1.0e-4 },
        ],
    };
    let rc_err = fit_censored_lognormal_mle(&right_cens).expect_err("AboveUpper refused");
    assert_eq!(
        rc_err.to_refusal().code,
        RefusalCode::SourceCellUnusable208.as_str()
    );
}
