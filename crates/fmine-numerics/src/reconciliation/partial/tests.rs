//! Independent constructed linear-Gaussian and finite-population known answers.

use super::*;

fn close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() <= 5e-11 * expected.abs().max(1e-100),
        "{actual:.17e} != {expected:.17e}"
    );
}

fn series(y: &[f64]) -> Result<PartialReconciliation> {
    // Feed -> unmeasured intermediate -> measured product and measured tail.
    // Eliminating the intermediate independently gives feed-product-tail=0.
    reconcile_with_unmeasured(
        y,
        &[4.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 9.0],
        &[1.0, 0.0, 0.0, 0.0, -1.0, -1.0],
        &[-1.0, 1.0],
        &[0.0, 0.0],
        1,
    )
}

#[test]
fn missing_internal_flow_matches_independently_eliminated_constraint_and_full_covariance() {
    let r = series(&[100.0, 72.0, 31.0]).unwrap();
    let expected = [706.0 / 7.0, 1005.0 / 14.0, 407.0 / 14.0];
    for (&actual, expected) in r.measured_estimates.iter().zip(expected) {
        close(actual, expected);
    }
    close(r.latent_estimates[0], 706.0 / 7.0);
    close(r.chi_square, 9.0 / 14.0);
    assert_eq!(r.degrees_of_freedom, 1);
    assert_eq!(r.raw_observed_residuals, vec![100.0, -103.0]);
    let expected_covariance = [
        20.0 / 7.0,
        2.0 / 7.0,
        18.0 / 7.0,
        20.0 / 7.0,
        2.0 / 7.0,
        13.0 / 14.0,
        -9.0 / 14.0,
        2.0 / 7.0,
        18.0 / 7.0,
        -9.0 / 14.0,
        45.0 / 14.0,
        18.0 / 7.0,
        20.0 / 7.0,
        2.0 / 7.0,
        18.0 / 7.0,
        20.0 / 7.0,
    ];
    for (&actual, expected) in r.posterior_covariance.iter().zip(expected_covariance) {
        close(actual, expected);
    }
    let expected_map = [
        5.0 / 7.0,
        2.0 / 7.0,
        2.0 / 7.0,
        1.0 / 14.0,
        13.0 / 14.0,
        -1.0 / 14.0,
        9.0 / 14.0,
        -9.0 / 14.0,
        5.0 / 14.0,
        5.0 / 7.0,
        2.0 / 7.0,
        2.0 / 7.0,
    ];
    for (&actual, expected) in r.estimator_map.iter().zip(expected_map) {
        close(actual, expected);
    }
    for value in &r.reconciled_residuals {
        assert!(value.abs() < 1e-12);
    }
}

#[test]
fn no_redundancy_preserves_measurements_and_infers_stock_difference_with_correlation() {
    let r = reconcile_with_unmeasured(
        &[10.0, 30.0],
        &[4.0, 1.0, 1.0, 9.0],
        &[1.0, -1.0],
        &[1.0],
        &[0.0],
        1,
    )
    .unwrap();
    assert_eq!(r.measured_estimates, vec![10.0, 30.0]);
    assert_eq!(r.measured_adjustments, vec![0.0; 2]);
    assert_eq!(r.adjustment_covariance, vec![0.0; 4]);
    assert_eq!(r.normalized_adjustments, vec![None; 2]);
    assert_eq!(r.chi_square, 0.0);
    assert_eq!(r.degrees_of_freedom, 0);
    close(r.latent_estimates[0], 20.0);
    for (&actual, expected) in r
        .posterior_covariance
        .iter()
        .zip([4.0, 1.0, -3.0, 1.0, 9.0, 8.0, -3.0, 8.0, 11.0])
    {
        close(actual, expected);
    }
    for (&actual, expected) in r.estimator_map.iter().zip([1.0, 0.0, 0.0, 1.0, -1.0, 1.0]) {
        close(actual, expected);
    }
}

#[test]
fn redundant_partial_balance_retains_cross_measurement_covariance() {
    let v = [4.0, 1.0, 0.5, 1.0, 2.0, 0.0, 0.5, 0.0, 9.0];
    let r = reconcile_with_unmeasured(
        &[100.0, 72.0, 31.0],
        &v,
        &[1.0, 0.0, 0.0, 0.0, -1.0, -1.0],
        &[-1.0, 1.0],
        &[0.0, 0.0],
        1,
    )
    .unwrap();
    // Independently eliminate the missing stream: a=[1,-1,-1],
    // V*a=[2.5,-1,-8.5], a'V*a=12 and observed imbalance=-3.
    let va = [2.5, -1.0, -8.5];
    for (&actual, expected) in r.measured_estimates.iter().zip([100.625, 71.75, 28.875]) {
        close(actual, expected);
    }
    close(r.latent_estimates[0], 100.625);
    close(r.chi_square, 0.75);
    for i in 0..4 {
        for j in 0..4 {
            let a = if i == 3 { 0 } else { i };
            let b = if j == 3 { 0 } else { j };
            close(
                r.posterior_covariance[i * 4 + j],
                v[a * 3 + b] - va[a] * va[b] / 12.0,
            );
        }
    }
}

#[test]
fn correlated_carrier_and_trace_element_stock_differences_retain_posterior_scale() {
    let mut v = vec![0.0; 36];
    for (i, variance) in [4.0, 1.0, 0.0004, 4.0, 1.0, 0.0004].into_iter().enumerate() {
        v[i * 6 + i] = variance;
    }
    v[2] = 0.02;
    v[12] = 0.02;
    let r = reconcile_with_unmeasured(
        &[100.0, 40.0, 2.0, 70.0, 25.0, 1.4],
        &v,
        &[
            1.0, 0.0, 0.0, -1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, -1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0,
            -1.0,
        ],
        &[-1.0, 0.0, 0.0, 0.0, -1.0, 0.0, 0.0, 0.0, -1.0],
        &[0.0; 3],
        3,
    )
    .unwrap();
    assert_eq!(r.degrees_of_freedom, 0);
    assert_eq!(r.measured_adjustments, vec![0.0; 6]);
    for (&actual, expected) in r.latent_estimates.iter().zip([30.0, 15.0, 0.6]) {
        close(actual, expected);
    }
    for (i, variance) in [8.0, 2.0, 0.0008].into_iter().enumerate() {
        close(r.posterior_covariance[(6 + i) * 9 + 6 + i], variance);
    }
    close(r.posterior_covariance[6 * 9 + 8], 0.02);
    // Unrelated water/element cross terms stay roundoff near zero, never a
    // variance floor or a reason to delete the physical source correlation.
    assert!(r.posterior_covariance[7 * 9 + 8].abs() < 1e-15);
}

#[test]
fn affine_constraints_allow_fixed_measured_coordinate_and_signed_latent_coordinate() {
    let r = reconcile_with_unmeasured(
        &[2.0, 3.0],
        &[1.0, 0.0, 0.0, 4.0],
        &[1.0, 0.0, 0.0, 1.0],
        &[1.0, 0.0],
        &[0.0, 5.0],
        1,
    )
    .unwrap();
    assert_eq!(r.measured_estimates, vec![2.0, 5.0]);
    assert_eq!(r.latent_estimates, vec![-2.0]);
    assert_eq!(r.chi_square, 1.0);
    assert_eq!(r.degrees_of_freedom, 1);
    assert_eq!(r.normalized_adjustments, vec![None, Some(1.0)]);
    assert_eq!(
        r.posterior_covariance,
        vec![1.0, 0.0, -1.0, 0.0, 0.0, 0.0, -1.0, 0.0, 1.0]
    );
    // Negative inferred mass admission belongs to the domain adapter, not K0.
}

#[test]
fn multiple_observable_missing_coordinates_need_no_fake_measurement_covariance() {
    let r = reconcile_with_unmeasured(
        &[2.0, 3.0],
        &[4.0, 1.0, 1.0, 9.0],
        &[1.0, 0.0, 0.0, 1.0],
        &[1.0, 1.0, 0.0, 1.0],
        &[5.0, 7.0],
        2,
    )
    .unwrap();
    assert_eq!(r.measured_estimates, vec![2.0, 3.0]);
    close(r.latent_estimates[0], -1.0);
    close(r.latent_estimates[1], 4.0);
    assert_eq!(r.degrees_of_freedom, 0);
    assert_eq!(r.chi_square, 0.0);
    let expected = [
        4.0, 1.0, -3.0, -1.0, 1.0, 9.0, 8.0, -9.0, -3.0, 8.0, 11.0, -8.0, -1.0, -9.0, -8.0, 9.0,
    ];
    for (&actual, expected) in r.posterior_covariance.iter().zip(expected) {
        close(actual, expected);
    }
}

#[test]
fn unobservable_split_cycle_and_excess_unknowns_refuse_inference() {
    for au in [
        [1.0, 1.0, 1.0, 1.0],
        [-1.0, 1.0, 1.0, -1.0],
        [1.0, 0.0, 0.0, 0.0],
    ] {
        let e = reconcile_with_unmeasured(
            &[2.0, 3.0],
            &[1.0, 0.0, 0.0, 1.0],
            &[1.0, 0.0, 0.0, 1.0],
            &au,
            &[0.0, 0.0],
            2,
        )
        .unwrap_err();
        assert_eq!(e.code, "FMINE-REF-104");
    }
    assert_eq!(
        reconcile_with_unmeasured(&[2.0], &[1.0], &[1.0], &[1.0, 1.0], &[0.0], 2)
            .unwrap_err()
            .code,
        "FMINE-REF-104"
    );
}

#[test]
fn invalid_error_models_or_dependent_measured_rows_are_not_repaired() {
    for covariance in [
        [1.0, 2.0, 2.0, 1.0],
        [1.0, 0.1, 0.2, 1.0],
        [0.0, 0.0, 0.0, 1.0],
        [f64::NAN, 0.0, 0.0, 1.0],
    ] {
        assert_eq!(
            reconcile_with_unmeasured(
                &[2.0, 3.0],
                &covariance,
                &[1.0, 0.0, 0.0, 1.0],
                &[1.0, 0.0],
                &[0.0, 0.0],
                1
            )
            .unwrap_err()
            .code,
            "FMINE-REF-207"
        );
    }
    assert_eq!(
        reconcile_with_unmeasured(
            &[2.0, 3.0],
            &[1.0, 0.0, 0.0, 1.0],
            &[1.0, 0.0, 1.0, 0.0],
            &[0.0, 1.0],
            &[0.0, 0.0],
            1
        )
        .unwrap_err()
        .code,
        "FMINE-REF-207"
    );
    assert!(
        reconcile_with_unmeasured(
            &[2.0, 3.0],
            &[1.0, 0.0, 0.0, 1.0],
            &[1.0, 0.0, 0.0, 1.0],
            &[1.0],
            &[0.0, 0.0],
            1
        )
        .is_err()
    );
    assert!(reconcile_with_unmeasured(&[2.0], &[1.0], &[1.0], &[], &[0.0], 0).is_err());
}

#[test]
fn row_scaling_and_latent_unit_scaling_preserve_the_physical_solution() {
    let original = series(&[100.0, 72.0, 31.0]).unwrap();
    // Multiply the two conservation rows by 1e-6 and 1e6 respectively, and
    // express the unmeasured coordinate in tonnes (1000 kg per coordinate).
    let scaled = reconcile_with_unmeasured(
        &[100.0, 72.0, 31.0],
        &[4.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 9.0],
        &[1e-6, 0.0, 0.0, 0.0, -1e6, -1e6],
        &[-1e-3, 1e9],
        &[0.0, 0.0],
        1,
    )
    .unwrap();
    for (&actual, &expected) in scaled
        .measured_estimates
        .iter()
        .zip(&original.measured_estimates)
    {
        close(actual, expected);
    }
    close(
        scaled.latent_estimates[0] * 1000.0,
        original.latent_estimates[0],
    );
    close(scaled.chi_square, original.chi_square);
    for i in 0..4 {
        for j in 0..4 {
            let factor = (if i == 3 { 1000.0 } else { 1.0 }) * (if j == 3 { 1000.0 } else { 1.0 });
            close(
                scaled.posterior_covariance[i * 4 + j] * factor,
                original.posterior_covariance[i * 4 + j],
            );
        }
    }
}

#[test]
fn finite_error_population_reproduces_joint_covariance_and_objective_degrees_of_freedom() {
    // Eight equiprobable independent ±1 errors give V=diag(4,1,9) exactly.
    // The noiseless truth satisfies the constraints: [100,70,30], latent100.
    let truth = [100.0, 70.0, 30.0, 100.0];
    let mut mean = [0.0; 4];
    let mut moments = [0.0; 16];
    let mut objective = 0.0;
    for a in [-1.0, 1.0] {
        for b in [-1.0, 1.0] {
            for c in [-1.0, 1.0] {
                let r = series(&[100.0 + 2.0 * a, 70.0 + b, 30.0 + 3.0 * c]).unwrap();
                let fitted = [
                    r.measured_estimates[0],
                    r.measured_estimates[1],
                    r.measured_estimates[2],
                    r.latent_estimates[0],
                ];
                for i in 0..4 {
                    mean[i] += (fitted[i] - truth[i]) / 8.0;
                    for j in 0..4 {
                        moments[i * 4 + j] += (fitted[i] - truth[i]) * (fitted[j] - truth[j]) / 8.0;
                    }
                }
                objective += r.chi_square / 8.0;
            }
        }
    }
    for m in mean {
        assert!(m.abs() < 1e-13);
    }
    let r = series(&truth[..3]).unwrap();
    for (&actual, &expected) in moments.iter().zip(&r.posterior_covariance) {
        close(actual, expected);
    }
    close(objective, 1.0);
    // This moment oracle is not an assertion that these discrete errors have
    // a Gaussian chi-square distribution or empirical plant coverage.
}
