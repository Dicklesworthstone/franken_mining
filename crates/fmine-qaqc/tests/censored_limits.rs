//! Numeric range controls for positive censored models. No candidate-generated fit oracle.

use fmine_drill::records::AssayValue;
use fmine_qaqc::QaqcError;
use fmine_qaqc::censored::{CensoredDataset, fit_censored_lognormal_mle, fit_censored_ros};

#[test]
fn ros_refuses_a_positive_modeled_order_statistic_that_underflows_to_zero() {
    // Hirsch-Stedinger positions are 5/9 and 7/9 for the measured values, 1/6 for the
    // censored value. Independent normal-quantile regression gives log modeled value
    // approximately -1914.4, below binary64. Positive aggregate summaries cannot conceal it.
    let dataset = CensoredDataset {
        values: vec![
            AssayValue::Measured {
                mass_fraction: 1e-300,
            },
            AssayValue::Measured { mass_fraction: 1.0 },
            AssayValue::BelowDetection { limit: 1e-310 },
        ],
    };
    let before = dataset.clone();
    let error = fit_censored_ros(&dataset).expect_err("lost positive imputation must refuse");
    assert!(
        matches!(&error, QaqcError::NumericalFailure { context, detail }
        if context == "censored.ros" && detail.contains("order statistic") && detail.contains("underflowed"))
    );
    assert_eq!(
        dataset, before,
        "fitting never replaces the raw censored assay"
    );
}

#[test]
fn ros_keeps_representable_small_sample_dispersion_without_squaring_raw_units() {
    let dataset = CensoredDataset {
        values: [1e-200, 2e-200, 4e-200]
            .map(|mass_fraction| AssayValue::Measured { mass_fraction })
            .into(),
    };
    let fit = fit_censored_ros(&dataset).expect("positive representable moments");
    // Actual uncensored measurements are preserved in ROS; normalized mean=7/3,
    // sample variance=7/3 and median=2, all in units of 1e-200.
    assert!((fit.mean / 1e-200 - 7.0 / 3.0).abs() < 1e-14);
    assert!((fit.sd / 1e-200 - (7.0_f64 / 3.0).sqrt()).abs() < 1e-14);
    assert_eq!(fit.median, 2e-200);
    let mle = fit_censored_lognormal_mle(&dataset).expect("MLE moments are representable too");
    assert!(mle.median > 0.0 && mle.arithmetic_mean > 0.0 && mle.arithmetic_sd > 0.0);
}

#[test]
fn ros_normal_score_regression_recovers_independent_normal_decile_values() {
    // Independent SciPy 1.17.0 reference values exp(-4 + 0.5 * norm.ppf(p)) for p=0.1..0.9.
    // With nine uncensored observations, ROS uses exactly those plotting positions. Therefore
    // the known regression intercept is -4, slope is 0.5 and R² is 1. No candidate fit creates
    // these reference observations; the unrefined Acklam scores miss the slope tolerance.
    let values = [
        0.009_650_208_257_535_976,
        0.012024481086797977,
        0.014091263883432213,
        0.016136466768987924,
        0.018_315_638_888_734_18,
        0.020789100408722994,
        0.023806425788174435,
        0.027898303925216854,
        0.03476221641543794,
    ]
    .map(|mass_fraction| AssayValue::Measured { mass_fraction })
    .into();
    let fit = fit_censored_ros(&CensoredDataset { values }).expect("normal-score regression");
    assert!((fit.mu_log + 4.0).abs() < 2e-13);
    assert!((fit.sigma_log - 0.5).abs() < 2e-13);
    assert!((fit.r_squared - 1.0).abs() < 2e-13);
}
