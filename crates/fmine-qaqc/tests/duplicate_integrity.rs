//! Synthetic known answers for source-preserving duplicate precision (`BD-167`, `BD-284`).

#![allow(clippy::unwrap_used, clippy::panic, clippy::float_cmp)]

use fmine_drill::records::{AssayValue, DuplicateStage, SampleType};
use fmine_qaqc::QaqcError;
use fmine_qaqc::duplicates::{
    DuplicatePair, DuplicatePairStatus, DuplicatePolicy, StageDecompositionStatus,
    decompose_duplicate_stages, evaluate_duplicates,
};

fn pair(id: usize, a: f64, b: f64) -> DuplicatePair {
    DuplicatePair {
        original_sample_id: format!("S{id}"),
        duplicate_sample_id: format!("D{id}"),
        stage: DuplicateStage::Pulp,
        original: AssayValue::Measured { mass_fraction: a },
        duplicate: AssayValue::Measured { mass_fraction: b },
        original_sample_type: Some(SampleType::HalfCore),
        duplicate_sample_type: Some(SampleType::HalfCore),
        original_mass_g: Some(500.0),
        duplicate_mass_g: Some(500.0),
        grade_band: Some("declared_band".into()),
    }
}

fn near(got: f64, want: f64, tolerance: f64) {
    assert!((got - want).abs() <= tolerance, "{got:e} != {want:e}");
}

#[test]
fn precision_keeps_every_exclusion_and_marks_missing_support_unverified() {
    let mut pairs = vec![
        pair(0, 0.00010, 0.00012),
        pair(1, 0.00020, 0.00016),
        pair(2, 0.2, 0.2),
        pair(3, 0.1, 0.1),
        pair(4, 0.0, 0.0),
        pair(5, 0.1, 0.2),
    ];
    pairs[2].original_mass_g = None;
    pairs[3].original = AssayValue::BelowDetection { limit: 0.1 };
    pairs[5].duplicate_sample_type = Some(SampleType::QuarterCore);
    let got = evaluate_duplicates(&pairs, &DuplicatePolicy::default()).unwrap();
    assert_eq!((got.input_pairs, got.valid_pairs), (6, 3));
    assert_eq!((got.excluded_censored, got.excluded_zero_pairs), (1, 1));
    assert_eq!(got.unfit_mass_mismatch_ids, ["D5"]);
    assert_eq!(got.unverified_support_pairs, 1);
    near(
        got.cv_avr_pct,
        100.0 * ((2.0_f64 / 3.0) * (1.0 / 121.0 + 1.0 / 81.0)).sqrt(),
        1e-12,
    );
    for (index, verdict) in got.pairs.iter().enumerate() {
        assert_eq!(verdict.index, index);
        assert_eq!(verdict.original_sample_id, pairs[index].original_sample_id);
        assert_eq!(
            verdict.duplicate_sample_id,
            pairs[index].duplicate_sample_id
        );
    }
    assert!(!got.pairs[2].mass_support_verified);
    assert_eq!(got.pairs[3].status, DuplicatePairStatus::Censored);
    assert_eq!(got.pairs[4].status, DuplicatePairStatus::BothZero);
    assert_eq!(
        got.pairs[5].status,
        DuplicatePairStatus::MassSupportMismatch
    );
    assert!(
        got.pairs[3..]
            .iter()
            .all(|row| row.pair_hard_pct.is_none() && row.detail.is_some())
    );
    pairs.reverse();
    let reverse = evaluate_duplicates(&pairs, &DuplicatePolicy::default()).unwrap();
    assert_eq!(got.cv_avr_pct.to_bits(), reverse.cv_avr_pct.to_bits());
    assert_eq!(
        got.mean_pair_ampd_pct.to_bits(),
        reverse.mean_pair_ampd_pct.to_bits()
    );
    assert_eq!(reverse.pairs[0].duplicate_sample_id, "D5");
}

#[test]
fn invalid_values_cannot_hide_behind_censoring_or_support_exclusions() {
    for bad in [f64::NAN, f64::INFINITY, -0.1, 1.1] {
        let mut p = pair(1, bad, 0.1);
        p.duplicate = AssayValue::BelowDetection { limit: 0.1 };
        assert!(matches!(
            evaluate_duplicates(&[pair(0, 0.1, 0.2), p], &DuplicatePolicy::default()),
            Err(QaqcError::InvalidParameter { .. })
        ));
    }
    for limit in [f64::NAN, f64::INFINITY, 0.0, -0.1, 1.1] {
        for value in [
            AssayValue::BelowDetection { limit },
            AssayValue::AboveUpper { limit },
        ] {
            let mut p = pair(1, 0.1, 0.2);
            p.duplicate = value;
            p.duplicate_sample_type = Some(SampleType::QuarterCore);
            assert!(matches!(
                evaluate_duplicates(&[pair(0, 0.1, 0.2), p], &DuplicatePolicy::default()),
                Err(QaqcError::InvalidParameter { .. })
            ));
        }
    }
    for mass in [f64::NAN, f64::INFINITY, 0.0, -1.0] {
        let mut p = pair(0, 0.1, 0.2);
        p.original_mass_g = Some(mass);
        p.duplicate_mass_g = None;
        assert!(matches!(
            evaluate_duplicates(&[p], &DuplicatePolicy::default()),
            Err(QaqcError::InvalidParameter { .. })
        ));
    }
    let mut impossible = pair(1, 0.1, 0.2);
    impossible.duplicate = AssayValue::AboveUpper { limit: 1.0 };
    assert!(
        evaluate_duplicates(
            &[pair(0, 0.1, 0.2), impossible],
            &DuplicatePolicy::default()
        )
        .is_err()
    );
}

#[test]
fn groups_refuse_mixed_stages_bands_and_duplicate_identity() {
    let first = pair(0, 0.1, 0.2);
    let mut other = pair(1, 0.1, 0.2);
    other.stage = DuplicateStage::Field;
    assert!(
        evaluate_duplicates(&[first.clone(), other.clone()], &DuplicatePolicy::default()).is_err()
    );
    other.stage = first.stage;
    other.grade_band = Some("another_band".into());
    assert!(
        evaluate_duplicates(&[first.clone(), other.clone()], &DuplicatePolicy::default()).is_err()
    );
    assert!(
        decompose_duplicate_stages(&[first.clone(), other], &DuplicatePolicy::default()).is_err()
    );
    assert!(
        evaluate_duplicates(&[first.clone(), first.clone()], &DuplicatePolicy::default()).is_err()
    );
    let mut self_pair = first;
    self_pair.duplicate_sample_id = self_pair.original_sample_id.clone();
    assert!(evaluate_duplicates(&[self_pair], &DuplicatePolicy::default()).is_err());
    assert!(matches!(
        evaluate_duplicates(&[pair(0, 0.0, 0.0)], &DuplicatePolicy::default()),
        Err(QaqcError::InsufficientData { .. })
    ));
}

#[test]
fn stage_attribution_requires_its_actual_components_and_retains_unavailable_stages() {
    let mut field = pair(0, 0.4, 0.6);
    field.stage = DuplicateStage::Field;
    let mut coarse = pair(1, 0.45, 0.55);
    coarse.stage = DuplicateStage::CoarseReject;
    let pulp = pair(2, 0.49, 0.51);
    let report = decompose_duplicate_stages(
        &[field.clone(), coarse.clone(), pulp.clone()],
        &DuplicatePolicy::default(),
    )
    .unwrap();
    assert_eq!(report.status, StageDecompositionStatus::Identified);
    near(report.analytical_rel_var.unwrap(), 0.0008, 1e-16);
    near(report.prep_incremental_rel_var.unwrap(), 0.0192, 1e-16);
    near(report.field_incremental_rel_var.unwrap(), 0.06, 1e-16);
    let only_field =
        decompose_duplicate_stages(&[field.clone()], &DuplicatePolicy::default()).unwrap();
    assert_eq!(
        only_field.status,
        StageDecompositionStatus::IncompleteStages
    );
    assert_eq!(only_field.field_incremental_rel_var, None);
    let missing_coarse =
        decompose_duplicate_stages(&[field.clone(), pulp.clone()], &DuplicatePolicy::default())
            .unwrap();
    assert_eq!(
        missing_coarse.status,
        StageDecompositionStatus::IncompleteStages
    );
    assert_eq!(missing_coarse.field_incremental_rel_var, None);
    field.original = AssayValue::BelowDetection { limit: 0.5 };
    let unavailable =
        decompose_duplicate_stages(&[field, pulp], &DuplicatePolicy::default()).unwrap();
    assert_eq!(
        unavailable.status,
        StageDecompositionStatus::FieldDuplicateAbsent
    );
    assert_eq!(unavailable.unavailable_stages, [DuplicateStage::Field]);
    let mut smaller_field = pair(3, 0.49, 0.51);
    smaller_field.stage = DuplicateStage::Field;
    let inverted =
        decompose_duplicate_stages(&[smaller_field, coarse], &DuplicatePolicy::default()).unwrap();
    assert_eq!(
        inverted.status,
        StageDecompositionStatus::NonMonotoneStageVariance
    );
    near(inverted.field_incremental_rel_var.unwrap(), -0.0192, 1e-16);
}

#[test]
fn thompson_howarth_is_scale_invariant_and_reports_the_fitted_denominator() {
    let pairs: Vec<_> = (0..66)
        .map(|i| {
            let mean = (1 + i / 11) as f64 * 0.01;
            pair(i, mean * 0.92, mean * 1.08)
        })
        .collect();
    let reference = evaluate_duplicates(&pairs, &DuplicatePolicy::default()).unwrap();
    let fit = reference.thompson_howarth.unwrap();
    near(fit.slope_k, 0.16 / 0.953_872_552_408_939_7, 1e-14);
    near(
        fit.relative_sd_pct_at_median,
        100.0 * 0.16 / 0.953_872_552_408_939_7,
        1e-12,
    );
    let tiny: Vec<_> = pairs
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let AssayValue::Measured { mass_fraction: a } = p.original else {
                panic!()
            };
            let AssayValue::Measured { mass_fraction: b } = p.duplicate else {
                panic!()
            };
            pair(i, a * 1e-250, b * 1e-250)
        })
        .collect();
    let tiny_result = evaluate_duplicates(&tiny, &DuplicatePolicy::default()).unwrap();
    let tiny_fit = tiny_result.thompson_howarth.unwrap();
    near(tiny_result.cv_avr_pct, reference.cv_avr_pct, 1e-13);
    near(tiny_fit.slope_k, fit.slope_k, 1e-14);
    near(
        tiny_fit.relative_sd_pct_at_median,
        fit.relative_sd_pct_at_median,
        1e-12,
    );
    let fifty = evaluate_duplicates(&pairs[..50], &DuplicatePolicy::default())
        .unwrap()
        .thompson_howarth
        .unwrap();
    assert_eq!(
        (
            fifty.n_pairs,
            fifty.fitted_pairs,
            fifty.unbinned_pairs,
            fifty.n_bins
        ),
        (50, 44, 6, 4)
    );
    let one_grade: Vec<_> = (0..66).map(|i| pair(i, 0.09, 0.11)).collect();
    assert!(
        evaluate_duplicates(&one_grade, &DuplicatePolicy::default())
            .unwrap()
            .thompson_howarth
            .is_none()
    );
}

#[test]
fn strict_support_policy_and_zero_data_never_return_a_fabricated_precision() {
    let mut bad = pair(0, 0.1, 0.2);
    bad.duplicate_mass_g = Some(200.0);
    let strict = DuplicatePolicy {
        max_relative_mass_diff: 0.25,
        refuse_on_mass_mismatch: true,
    };
    assert!(matches!(
        evaluate_duplicates(&[bad.clone()], &strict),
        Err(QaqcError::DuplicateSupportMismatch { .. })
    ));
    assert!(matches!(
        evaluate_duplicates(&[bad], &DuplicatePolicy::default()),
        Err(QaqcError::InsufficientData { .. })
    ));
    assert!(matches!(
        evaluate_duplicates(&[], &DuplicatePolicy::default()),
        Err(QaqcError::InsufficientData { .. })
    ));
    let invalid = DuplicatePolicy {
        max_relative_mass_diff: f64::NAN,
        refuse_on_mass_mismatch: false,
    };
    assert!(decompose_duplicate_stages(&[], &invalid).is_err());
}
