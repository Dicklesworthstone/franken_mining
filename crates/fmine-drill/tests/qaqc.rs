//! Known answers for fixed CRM rules (`BD-167`, `BD-282`); synthetic values, not a lab validation.

#![allow(clippy::unwrap_used, clippy::panic, clippy::float_cmp)]

use fmine_drill::qaqc::{
    CrmBatch, CrmPolicy, CrmState, MAX_CRM_OBSERVATIONS, SdBasis, score_crm_batch,
};
use fmine_drill::records::DrillError;

fn policy() -> CrmPolicy {
    CrmPolicy {
        warning_k: 2.0,
        failure_k: 3.0,
        confidence: 0.95,
    }
}

/// Deliberately different interpretations: ten C warnings/eight failures, three L warnings/no failures,
/// four running-SW warnings/no failures, and a CANMET pass despite a precisely positive observed bias.
fn batch() -> CrmBatch {
    CrmBatch {
        certified_mean: 100.0,
        sigma_c: 1.0,
        sigma_l: 2.5,
        lab_running_sw: 2.0,
        values: vec![
            102.5, 102.75, 103.25, 103.5, 103.75, 104.0, 104.25, 105.25, 105.5, 105.75,
        ],
    }
}

fn near(got: f64, want: f64, tolerance: f64) {
    assert!((got - want).abs() <= tolerance, "{got:.17e} != {want:.17e}");
}

fn field(error: DrillError) -> String {
    match error {
        DrillError::Invalid { field, .. } => field,
        other => panic!("{other:?}"),
    }
}

#[test]
fn named_sd_bases_and_canmet_retain_their_different_known_answers() {
    let input = batch();
    let got = score_crm_batch(&input, &policy()).unwrap();
    assert_eq!(got.n, 10);
    near(got.mean, 104.05, 1e-12);
    near(got.sample_sd, (58.0_f64 / 45.0).sqrt(), 1e-12);
    near(got.standard_error, (58.0_f64 / 450.0).sqrt(), 1e-12);
    near(got.bias.signed_bias, 4.05, 1e-12);
    near(got.bias.magnitude, 4.05, 1e-12);
    assert_eq!(got.bias.degrees_of_freedom, 9);
    let t9 = 2.262_157_162_8;
    near(got.bias.t_critical, t9, 1e-9);
    let margin = t9 * (58.0_f64 / 450.0).sqrt();
    near(got.bias.lower, 4.05 - margin, 1e-9);
    near(got.bias.upper, 4.05 + margin, 1e-9);
    assert!(got.bias.lower > 0.0);
    assert!(got.bias.magnitude / got.standard_error > 11.0);

    for (assessment, (basis, sd, warnings, failures)) in got.single_value.iter().zip([
        (SdBasis::CertifiedWithinLab, 1.0, 10, 8),
        (SdBasis::CertifiedBetweenLab, 2.5, 3, 0),
        (SdBasis::LabRunning, 2.0, 4, 0),
    ]) {
        assert_eq!((assessment.basis, assessment.sd), (basis, sd));
        assert_eq!(
            (assessment.warnings, assessment.failures),
            (warnings, failures)
        );
        assert_eq!(
            (assessment.warning_limit, assessment.failure_limit),
            (2.0 * sd, 3.0 * sd)
        );
        assert_eq!(assessment.results.len(), input.values.len());
        for (i, result) in assessment.results.iter().enumerate() {
            assert_eq!(result.index, i);
            assert_eq!(result.deviation, input.values[i] - 100.0);
        }
    }
    near(got.canmet.limit, 2.0 * 6.65_f64.sqrt(), 1e-12);
    near(got.canmet.absolute_bias, 4.05, 1e-12);
    assert!(
        got.canmet.passed,
        "a CANMET pass does not erase single-result failures or observed bias"
    );
    assert_ne!(
        got.single_value[2].sd, got.sample_sd,
        "the declared running SW is not estimated from this batch"
    );
}

#[test]
fn threshold_equalities_warning_counts_and_zero_sds_are_explicit() {
    let input = CrmBatch {
        certified_mean: 100.0,
        sigma_c: 1.0,
        sigma_l: 1.0,
        lab_running_sw: 1.0,
        values: vec![97.0, 98.0, 100.0, 102.0, 103.0, 104.0],
    };
    let got = score_crm_batch(&input, &policy()).unwrap();
    for basis in got.single_value {
        let states: Vec<_> = basis.results.iter().map(|r| r.state).collect();
        assert_eq!(
            states,
            vec![
                CrmState::Warning,
                CrmState::Pass,
                CrmState::Pass,
                CrmState::Pass,
                CrmState::Warning,
                CrmState::Failure,
            ]
        );
        assert_eq!((basis.warnings, basis.failures), (3, 1));
    }

    let mut exact = CrmBatch {
        certified_mean: 10.0,
        sigma_c: 0.0,
        sigma_l: 0.0,
        lab_running_sw: 0.0,
        values: vec![10.0, 10.0],
    };
    let got = score_crm_batch(&exact, &policy()).unwrap();
    assert_eq!(
        (
            got.sample_sd,
            got.standard_error,
            got.bias.lower,
            got.bias.upper
        ),
        (0.0, 0.0, 0.0, 0.0)
    );
    assert!(got.canmet.passed);
    assert_eq!(got.canmet.limit, 0.0);
    for basis in got.single_value {
        assert_eq!((basis.warnings, basis.failures), (0, 0));
    }
    exact.values[1] = 11.0;
    let got = score_crm_batch(&exact, &policy()).unwrap();
    assert!(!got.canmet.passed);
    assert!(
        got.bias.lower < 0.0,
        "signed interval is not clipped at zero"
    );
    for basis in got.single_value {
        assert_eq!((basis.warnings, basis.failures), (1, 1));
        assert_eq!(basis.results[0].state, CrmState::Pass);
        assert_eq!(basis.results[1].state, CrmState::Failure);
    }
    // CANMET equality passes too: n=2, no running variation, sigma_L=1 -> limit=2.
    exact.sigma_l = 1.0;
    exact.values = vec![12.0, 12.0];
    let got = score_crm_batch(&exact, &policy()).unwrap();
    assert_eq!(
        (
            got.canmet.absolute_bias,
            got.canmet.limit,
            got.canmet.passed
        ),
        (2.0, 2.0, true)
    );
}

#[test]
fn student_t_intervals_match_independent_cauchy_and_two_df_closed_forms() {
    for confidence in [0.5, 0.8, 0.95, 0.99] {
        for (values, want_t) in [
            (
                vec![0.0, 2.0],
                (std::f64::consts::PI * confidence / 2.0).tan(),
            ),
            (
                vec![0.0, 1.0, 2.0],
                confidence * (2.0 / (1.0 - confidence * confidence)).sqrt(),
            ),
        ] {
            let input = CrmBatch {
                values,
                certified_mean: 0.0,
                ..batch()
            };
            let got = score_crm_batch(
                &input,
                &CrmPolicy {
                    confidence,
                    ..policy()
                },
            )
            .unwrap();
            near(got.bias.t_critical, want_t, want_t * 1e-10);
            near(
                got.bias.lower,
                1.0 - want_t * got.standard_error,
                want_t * 1e-10,
            );
            near(
                got.bias.upper,
                1.0 + want_t * got.standard_error,
                want_t * 1e-10,
            );
            assert_eq!(got.bias.confidence, confidence);
        }
    }
}

#[test]
fn stable_moments_and_input_permutation_preserve_statistics_and_row_identity() {
    let input = CrmBatch {
        certified_mean: 1e12,
        values: vec![1e12 + 1.0, 1e12 + 2.0, 1e12 + 3.0, 1e12 + 4.0, 1e12 + 5.0],
        ..batch()
    };
    let got = score_crm_batch(&input, &policy()).unwrap();
    assert_eq!(got.mean, 1e12 + 3.0);
    near(got.sample_sd, 2.5_f64.sqrt(), 1e-14);
    near(got.standard_error, 0.5_f64.sqrt(), 1e-14);
    let mut permuted = input.clone();
    permuted.values = vec![
        input.values[3],
        input.values[0],
        input.values[4],
        input.values[1],
        input.values[2],
    ];
    let other = score_crm_batch(&permuted, &policy()).unwrap();
    assert_eq!(
        (got.mean, got.sample_sd, got.bias),
        (other.mean, other.sample_sd, other.bias)
    );
    for basis in other.single_value {
        for (index, check) in basis.results.iter().enumerate() {
            assert_eq!(check.index, index);
            assert_eq!(
                check.deviation,
                permuted.values[index] - permuted.certified_mean
            );
        }
    }
    let input = batch();
    let mut scaled = input.clone();
    for value in &mut scaled.values {
        *value *= 1000.0;
    }
    scaled.certified_mean *= 1000.0;
    scaled.sigma_c *= 1000.0;
    scaled.sigma_l *= 1000.0;
    scaled.lab_running_sw *= 1000.0;
    let before = score_crm_batch(&input, &policy()).unwrap();
    let after = score_crm_batch(&scaled, &policy()).unwrap();
    near(after.bias.lower, before.bias.lower * 1000.0, 1e-8);
    near(after.bias.upper, before.bias.upper * 1000.0, 1e-8);
    near(after.canmet.limit, before.canmet.limit * 1000.0, 1e-8);
    for (a, b) in before.single_value.iter().zip(after.single_value) {
        assert_eq!((a.warnings, a.failures), (b.warnings, b.failures));
    }
}

#[test]
fn unusable_observations_declarations_policy_and_nonfinite_results_are_refused() {
    let largest = CrmBatch {
        values: vec![100.0; MAX_CRM_OBSERVATIONS],
        ..batch()
    };
    let got = score_crm_batch(&largest, &policy()).unwrap();
    assert_eq!(got.n, MAX_CRM_OBSERVATIONS);
    assert!(got.canmet.passed);
    assert_eq!(got.bias.signed_bias, 0.0);
    assert!((1.96..1.97).contains(&got.bias.t_critical));
    for values in [
        vec![],
        vec![1.0],
        vec![1.0; MAX_CRM_OBSERVATIONS + 1],
        vec![1.0, -0.1],
        vec![1.0, f64::NAN],
        vec![1.0, f64::INFINITY],
    ] {
        assert_eq!(
            field(score_crm_batch(&CrmBatch { values, ..batch() }, &policy()).unwrap_err()),
            "qaqc.values"
        );
    }
    for invalid in [-1.0, f64::NAN, f64::INFINITY] {
        for (input, wanted) in [
            (
                CrmBatch {
                    certified_mean: invalid,
                    ..batch()
                },
                "qaqc.certified_mean",
            ),
            (
                CrmBatch {
                    sigma_c: invalid,
                    ..batch()
                },
                "qaqc.sigma_c",
            ),
            (
                CrmBatch {
                    sigma_l: invalid,
                    ..batch()
                },
                "qaqc.sigma_l",
            ),
            (
                CrmBatch {
                    lab_running_sw: invalid,
                    ..batch()
                },
                "qaqc.lab_running_sw",
            ),
        ] {
            assert_eq!(
                field(score_crm_batch(&input, &policy()).unwrap_err()),
                wanted
            );
        }
    }
    for invalid in [
        CrmPolicy {
            warning_k: 0.0,
            ..policy()
        },
        CrmPolicy {
            warning_k: 3.0,
            ..policy()
        },
        CrmPolicy {
            failure_k: f64::NAN,
            ..policy()
        },
        CrmPolicy {
            failure_k: f64::INFINITY,
            ..policy()
        },
    ] {
        assert_eq!(
            field(score_crm_batch(&batch(), &invalid).unwrap_err()),
            "qaqc.policy"
        );
    }
    for confidence in [0.0, 1.0, -0.5, f64::NAN, f64::MIN_POSITIVE] {
        assert_eq!(
            field(
                score_crm_batch(
                    &batch(),
                    &CrmPolicy {
                        confidence,
                        ..policy()
                    }
                )
                .unwrap_err()
            ),
            "qaqc.confidence"
        );
    }
    let overflowing_limits = CrmPolicy {
        warning_k: 1e308,
        failure_k: f64::MAX,
        ..policy()
    };
    assert_eq!(
        field(score_crm_batch(&batch(), &overflowing_limits).unwrap_err()),
        "qaqc.limits"
    );
    let overflowing_bias = CrmBatch {
        certified_mean: 0.0,
        values: vec![0.0, f64::MAX],
        ..batch()
    };
    assert_eq!(
        field(score_crm_batch(&overflowing_bias, &policy()).unwrap_err()),
        "qaqc.arithmetic"
    );
    let overflowing_canmet = CrmBatch {
        sigma_l: f64::MAX,
        ..batch()
    };
    assert_eq!(
        field(score_crm_batch(&overflowing_canmet, &policy()).unwrap_err()),
        "qaqc.arithmetic"
    );
    let underflowing = CrmBatch {
        values: vec![0.0, f64::from_bits(1)],
        ..batch()
    };
    assert_eq!(
        field(score_crm_batch(&underflowing, &policy()).unwrap_err()),
        "qaqc.arithmetic"
    );
}

#[test]
fn finite_precision_cannot_turn_nonzero_uncertainty_into_an_exact_answer() {
    let smallest = f64::from_bits(1);
    // Positive observed SE, but the requested t critical value is below one half: its product underflows.
    let margin_underflow = CrmBatch {
        certified_mean: 0.0,
        sigma_c: 0.0,
        sigma_l: 0.0,
        lab_running_sw: 0.0,
        values: vec![smallest, f64::from_bits(3)],
    };
    // S_W / sqrt(4) underflows although S_W is supplied as nonzero.
    let canmet_underflow = CrmBatch {
        sigma_c: 0.0,
        sigma_l: 0.0,
        lab_running_sw: smallest,
        values: vec![100.0; 4],
        ..batch()
    };
    // A finite, positive margin is swallowed by the much larger signed bias on both sides.
    let collapsed_interval = CrmBatch {
        certified_mean: 1e100,
        values: vec![1.0, 2.0],
        ..batch()
    };
    for (input, confidence) in [
        (margin_underflow, 0.25),
        (canmet_underflow, 0.95),
        (collapsed_interval, 0.95),
    ] {
        let policy = CrmPolicy {
            confidence,
            ..policy()
        };
        assert_eq!(
            field(score_crm_batch(&input, &policy).unwrap_err()),
            "qaqc.arithmetic"
        );
    }
}
