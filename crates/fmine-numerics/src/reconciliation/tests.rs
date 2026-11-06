//! Determinism: DeterministicPerIsa

use super::*;

fn close(a: f64, b: f64) {
    assert!((a - b).abs() <= 2e-10 * b.abs().max(1.0), "{a} != {b}");
}
fn diagonal(values: &[f64]) -> Vec<f64> {
    let mut result = vec![0.0; values.len() * values.len()];
    for (i, &v) in values.iter().enumerate() {
        result[i * values.len() + i] = v;
    }
    result
}

#[test]
fn one_balance_known_answer_and_posterior_covariance() {
    let result = reconcile_linear(
        &[10.0, 7.0, 4.0],
        &diagonal(&[1.0, 4.0, 9.0]),
        &[1.0, -1.0, -1.0],
        &[0.0],
    )
    .unwrap();
    for (actual, expected) in
        result
            .estimates
            .iter()
            .zip([10.0 + 1.0 / 14.0, 7.0 - 4.0 / 14.0, 4.0 - 9.0 / 14.0])
    {
        close(*actual, expected);
    }
    close(result.chi_square, 1.0 / 14.0);
    let v = [1.0, -4.0, -9.0];
    for i in 0..3 {
        for j in 0..3 {
            close(result.adjustment_covariance[i * 3 + j], v[i] * v[j] / 14.0);
            close(
                result.posterior_covariance[i * 3 + j],
                if i == j { [1.0, 4.0, 9.0][i] } else { 0.0 } - v[i] * v[j] / 14.0,
            );
        }
    }
    for z in result.normalized_adjustments {
        close(z.unwrap().abs(), 1.0 / 14.0_f64.sqrt());
    }
    assert_eq!(result.degrees_of_freedom, 1);
}

#[test]
fn correlated_measurements_adjust_an_indirectly_observed_variable() {
    // x1=x2. x3 is absent from the constraint but correlated with x1.
    let covariance = [4.0, 1.0, 1.0, 1.0, 9.0, 0.0, 1.0, 0.0, 4.0];
    let result =
        reconcile_linear(&[10.0, 12.0, 5.0], &covariance, &[1.0, -1.0, 0.0], &[0.0]).unwrap();
    close(result.estimates[0], 10.0 + 6.0 / 11.0);
    close(result.estimates[1], 12.0 - 16.0 / 11.0);
    close(result.estimates[2], 5.0 + 2.0 / 11.0);
    close(result.chi_square, 4.0 / 11.0);
    assert!(result.normalized_adjustments[2].is_some());
}

#[test]
fn primary_paper_example_and_independent_nullspace_oracle() {
    // FACT: Tamhane & Mah (1985), §2.3 Example 1 and Table 1, p411.
    // Units remain the source's 1000 lb-mol/hour; no claim these are plant kg.
    // Independent algebra: null(A) is spanned by (1,29,7,23), so fit one
    // scalar t = sum(v_i*y_i/var_i) / sum(v_i^2/var_i).
    let y = [0.1858, 4.7935, 1.2295, 3.8800];
    let variances = [0.000289, 0.0025, 0.000576, 0.04];
    let a = [
        0.1, 0.6, -0.2, -0.7, 0.8, 0.1, -0.2, -0.1, 0.1, 0.3, -0.6, -0.2,
    ];
    let r = reconcile_linear(&y, &diagonal(&variances), &a, &[0.0; 3]).unwrap();
    let v = [1.0, 29.0, 7.0, 23.0];
    let precision = (0..4).map(|i| v[i] * v[i] / variances[i]).sum::<f64>();
    let t = (0..4).map(|i| v[i] * y[i] / variances[i]).sum::<f64>() / precision;
    for i in 0..4 {
        close(r.estimates[i], v[i] * t);
        for j in 0..4 {
            close(r.posterior_covariance[i * 4 + j], v[i] * v[j] / precision);
        }
    }
    let objective = (0..4)
        .map(|i| (v[i] * t - y[i]).powi(2) / variances[i])
        .sum::<f64>();
    close(r.chi_square, objective);
    // The published global-test decision uses 7.815 for three degrees of freedom.
    assert!(r.chi_square > 7.815);
}

#[test]
fn affine_constraints_and_unchanged_unconstrained_measurement() {
    let result = reconcile_linear(
        &[2.0, 4.0, 9.0],
        &diagonal(&[1.0, 1.0, 4.0]),
        &[1.0, 1.0, 0.0],
        &[8.0],
    )
    .unwrap();
    assert_eq!(result.estimates, vec![3.0, 5.0, 9.0]);
    assert_eq!(result.normalized_adjustments[2], None);
    assert_eq!(result.posterior_covariance[8], 4.0);
    close(result.chi_square, 2.0);
}

#[test]
fn scaling_and_permutation_do_not_change_the_statistical_problem() {
    let y = [10.0, 7.0, 4.0];
    let variances = [1.0, 4.0, 9.0];
    let a = [1.0, -1.0, -1.0];
    let baseline = reconcile_linear(&y, &diagonal(&variances), &a, &[0.0]).unwrap();
    for scale in [1e-12, 1e12] {
        let ys = y.map(|v| v * scale);
        let vs = variances.map(|v| v * scale * scale);
        let result = reconcile_linear(&ys, &diagonal(&vs), &a, &[0.0]).unwrap();
        close(result.chi_square, baseline.chi_square);
        for i in 0..3 {
            close(result.estimates[i] / scale, baseline.estimates[i]);
        }
    }
    let permuted = reconcile_linear(
        &[4.0, 10.0, 7.0],
        &diagonal(&[9.0, 1.0, 4.0]),
        &[-1.0, 1.0, -1.0],
        &[0.0],
    )
    .unwrap();
    for (i, j) in [2, 0, 1].into_iter().enumerate() {
        close(permuted.estimates[i], baseline.estimates[j]);
    }
    let rescaled_rows =
        reconcile_linear(&y, &diagonal(&variances), &a.map(|v| v * 1e-12), &[0.0]).unwrap();
    close(rescaled_rows.chi_square, baseline.chi_square);
}

#[test]
fn covariance_propagation_matches_independent_finite_error_population() {
    // Six equally weighted deterministic errors have exact mean zero and
    // covariance diag(1,4,9). Push them through the reconciler and compare
    // the full resulting covariance, including off-diagonal entries.
    let truth = [12.0, 8.0, 4.0];
    let v = [1.0, 4.0, 9.0];
    let a = [1.0, -1.0, -1.0];
    let reference = reconcile_linear(&truth, &diagonal(&v), &a, &[0.0]).unwrap();
    let mut outcomes = Vec::new();
    for i in 0..3 {
        for sign in [-1.0, 1.0] {
            let mut y = truth;
            y[i] += sign * (3.0_f64 * v[i]).sqrt();
            outcomes.push(
                reconcile_linear(&y, &diagonal(&v), &a, &[0.0])
                    .unwrap()
                    .estimates,
            );
        }
    }
    for i in 0..3 {
        close(outcomes.iter().map(|x| x[i]).sum::<f64>() / 6.0, truth[i]);
        for j in 0..3 {
            close(
                outcomes
                    .iter()
                    .map(|x| (x[i] - truth[i]) * (x[j] - truth[j]))
                    .sum::<f64>()
                    / 6.0,
                reference.posterior_covariance[i * 3 + j],
            );
        }
    }
}

#[test]
fn malformed_singular_or_indefinite_declarations_refuse() {
    for v in [
        vec![1.0, 2.0, 2.0, 1.0],
        vec![1.0, 1.0, 1.0, 1.0],
        vec![1.0, 0.1, 0.2, 1.0],
        vec![1.0, 0.0, 0.0, 0.0],
        vec![1.0, 0.0, 0.0, f64::NAN],
    ] {
        assert!(reconcile_linear(&[1.0, 2.0], &v, &[1.0, -1.0], &[0.0]).is_err());
    }
    assert!(
        reconcile_linear(
            &[1.0, 2.0],
            &diagonal(&[1.0, 1.0]),
            &[1.0, -1.0, 2.0, -2.0],
            &[0.0, 0.0]
        )
        .is_err()
    );
    assert!(
        reconcile_linear(
            &[f64::INFINITY, 2.0],
            &diagonal(&[1.0, 1.0]),
            &[1.0, -1.0],
            &[0.0]
        )
        .is_err()
    );
    assert!(reconcile_linear(&vec![1.0; MAX_OBSERVATIONS + 1], &[], &[], &[0.0]).is_err());
}

#[test]
fn extreme_variance_ratio_preserves_the_small_retained_covariance() {
    // For x1+x2=0 with independent errors v1 and v2, the posterior is
    // v1*v2/(v1+v2) * [[1,-1],[-1,1]]. Its nonzero scale is 1e-20 here.
    // Subtractive V-KAV previously returned [[0,-1e-20],[-1e-20,1e-20]],
    // an indefinite matrix that the old original-V-scaled audit accepted.
    for variances in [[1e20, 1e-20], [1e-20, 1e20]] {
        let result = reconcile_linear(&[0.0; 2], &diagonal(&variances), &[1.0; 2], &[0.0]).unwrap();
        for (&actual, expected) in result
            .posterior_covariance
            .iter()
            .zip([1e-20, -1e-20, -1e-20, 1e-20])
        {
            assert!(
                (actual - expected).abs() <= 2e-12 * expected.abs(),
                "retained covariance {actual:e} != {expected:e}"
            );
        }
        assert!(result.posterior_covariance[0] > 0.0);
        assert!(result.posterior_covariance[3] > 0.0);
        for j in 0..2 {
            assert!(
                (result.posterior_covariance[j] + result.posterior_covariance[2 + j]).abs()
                    <= 1e-32
            );
        }
        assert_eq!(result.estimates, vec![0.0; 2]);
        assert_eq!(result.chi_square, 0.0);
    }
}

#[test]
fn exactly_fixed_coordinates_have_zero_posterior_covariance() {
    let full = reconcile_linear(
        &[3.0, 4.0],
        &diagonal(&[4.0, 9.0]),
        &[1.0, 0.0, 0.0, 1.0],
        &[1.0, 2.0],
    )
    .unwrap();
    assert_eq!(full.estimates, vec![1.0, 2.0]);
    assert_eq!(full.posterior_covariance, vec![0.0; 4]);
    assert_eq!(full.adjustment_covariance, diagonal(&[4.0, 9.0]));

    // Conditioning the first coordinate also reduces the correlated second
    // variance, but the fixed coordinate has neither variance nor cross terms.
    let partial =
        reconcile_linear(&[3.0, 4.0], &[4.0, 1.0, 1.0, 9.0], &[1.0, 0.0], &[1.0]).unwrap();
    assert_eq!(partial.estimates, vec![1.0, 3.5]);
    assert_eq!(partial.posterior_covariance, vec![0.0, 0.0, 0.0, 8.75]);
}

#[test]
fn covariance_consistency_refuses_zero_variance_cross_terms_and_excess_correlation() {
    assert!(audit_covariance(&[0.0, -1e-20, -1e-20, 1e-20], 2, "test covariance").is_err());
    assert!(audit_covariance(&[1e-20, 1.01e-20, 1.01e-20, 1e-20], 2, "test covariance").is_err());
    assert!(audit_covariance(&[1e-20, -1e-20, -1e-20, 1e-20], 2, "test covariance").is_ok());
}

#[test]
fn linear_covariance_projection_matches_correlated_analytic_answers_and_zero_rows() {
    let covariance = [4.0, 1.0, 1.0, 9.0];
    // Y=(2X1-X2, 3X2, 0). Independent expansion gives Var(Y1)=21,
    // Cov(Y1,Y2)=-21 and Var(Y2)=81; the last coordinate is identically zero.
    let projected =
        propagate_linear_covariance(&covariance, &[2.0, -1.0, 0.0, 3.0, 0.0, 0.0], 2, 3).unwrap();
    assert_eq!(
        projected,
        vec![21.0, -21.0, 0.0, -21.0, 81.0, 0.0, 0.0, 0.0, 0.0]
    );
    // A singular result need not contain any zero rows: Y2=2Y1 here.
    assert_eq!(
        propagate_linear_covariance(&covariance, &[1.0, 1.0, 2.0, 2.0], 2, 2).unwrap(),
        vec![15.0, 30.0, 30.0, 60.0]
    );
    assert_eq!(
        propagate_linear_covariance(&covariance, &[0.0; 4], 2, 2).unwrap(),
        vec![0.0; 4]
    );
}

#[test]
fn linear_covariance_projection_preserves_units_and_small_variances() {
    let covariance = [4.0, 1.0, 1.0, 9.0];
    let jacobian = [2.0, -1.0, 0.0, 3.0];
    let baseline = propagate_linear_covariance(&covariance, &jacobian, 2, 2).unwrap();
    // X'=diag(s)X must be paired with J'=J diag(s)^-1, preserving Y.
    for scales in [[1e12, 1e-12], [1e-12, 1e12]] {
        let scaled_v: Vec<_> = (0..4)
            .map(|k| covariance[k] * scales[k / 2] * scales[k % 2])
            .collect();
        let scaled_j: Vec<_> = (0..4).map(|k| jacobian[k] / scales[k % 2]).collect();
        let actual = propagate_linear_covariance(&scaled_v, &scaled_j, 2, 2).unwrap();
        for (a, b) in actual.into_iter().zip(&baseline) {
            close(a, *b);
        }
    }
    let tiny =
        propagate_linear_covariance(&diagonal(&[1e20, 1e-20]), &[1e-20, 1.0], 2, 1).unwrap()[0];
    assert!((tiny - 2e-20).abs() <= 2e-12 * 2e-20, "{tiny:e}");
}

#[test]
fn linear_covariance_projection_retains_cross_lot_measurement_correlation() {
    // Authored two-lot first-order oracle, not measured plant calibration.
    // Nominal (wet kg, wet-basis moisture, dry element fraction):
    // (100,.1,.01), (200,.2,.02); stddevs (1,.01,.001) for both lots.
    // Raw mass errors have correlation .5; every other off-diagonal is
    // explicitly zero. These derivative rows already include the stddevs.
    let mut correlation = diagonal(&[1.0; 6]);
    correlation[3] = 0.5;
    correlation[18] = 0.5;
    let jacobian = [
        0.9, -1.0, 0.0, 0.8, -2.0, 0.0, 0.1, 1.0, 0.0, 0.2, 2.0, 0.0, 0.009, -0.01, 0.09, 0.016,
        -0.04, 0.16,
    ];
    // Independent rational expansion, in dry matter/water/element order, kg².
    let expected = [
        7.17, -4.62, 0.1217, -4.62, 5.07, -0.0842, 0.1217, -0.0842, 0.035881,
    ];
    let actual = propagate_linear_covariance(&correlation, &jacobian, 6, 3).unwrap();
    for (&a, b) in actual.iter().zip(expected) {
        close(a, b);
    }
    // Var(dry+water)=Var(wet1+wet2)=1+1+2*.5, not 2.
    close(actual[0] + actual[4] + 2.0 * actual[1], 3.0);
}

#[test]
fn linear_covariance_projection_refuses_invalid_sources_shapes_and_lost_range() {
    for covariance in [
        [1.0, 1.0, 1.0, 1.0],
        [1.0, 2.0, 2.0, 1.0],
        [1.0, 0.1, 0.2, 1.0],
        [1.0, 0.0, 0.0, 0.0],
        [1.0, 0.0, 0.0, f64::NAN],
    ] {
        assert!(propagate_linear_covariance(&covariance, &[1.0, 0.0], 2, 1).is_err());
    }
    assert!(propagate_linear_covariance(&[], &[], 0, 1).is_err());
    assert!(propagate_linear_covariance(&[1.0], &[], 1, 0).is_err());
    assert!(propagate_linear_covariance(&[], &[], 129, 1).is_err());
    assert!(propagate_linear_covariance(&[], &[], 1, 129).is_err());
    assert!(propagate_linear_covariance(&[1.0], &[1.0], 2, 1).is_err());
    assert!(propagate_linear_covariance(&[1.0], &[], 1, 1).is_err());
    assert!(propagate_linear_covariance(&[1.0], &[f64::INFINITY], 1, 1).is_err());
    assert!(propagate_linear_covariance(&[1e300], &[1e100], 1, 1).is_err());
    assert!(propagate_linear_covariance(&[1e-200], &[1e-200], 1, 1).is_err());
    // Even a constant output does not authorize an invalid source covariance.
    assert!(propagate_linear_covariance(&[0.0], &[0.0], 1, 1).is_err());
}
