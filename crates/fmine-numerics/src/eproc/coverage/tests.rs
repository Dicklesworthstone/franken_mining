//! Independent beta-integral and exact finite-path checks for the coverage process.
//! Determinism: DeterministicPerIsa (integer oracle and deterministic floating-point comparisons).

use super::*;

fn close(actual: f64, expected: f64, tolerance: f64) {
    assert!(
        (actual - expected).abs() <= tolerance * expected.abs().max(1.0),
        "{actual} differs from {expected} by more than {tolerance} relative/absolute"
    );
}

fn choose(n: u64, k: u64) -> u128 {
    (1..=k.min(n - k)).fold(1, |c, j| c * u128::from(n + 1 - j) / u128::from(j))
}

/// Exact beta integral through factorial cancellation, independent of the sequential recurrence.
/// The tests stay within u128, with null p = 4/5.
fn exact_e(n: u64, k: u64) -> (u128, u128) {
    (
        5_u128.pow(n as u32),
        u128::from(n + 1) * choose(n, k) * 4_u128.pow(k as u32),
    )
}

#[test]
fn coverage_gb08_beta_integral_known_answers() {
    // FACT: exact fractions from 1/((n+1)*binomial(n,k)) / ((4/5)^k (1/5)^(n-k)),
    // computed independently with Python fractions.Fraction, not from the production recurrence.
    for (n, k, expected) in [
        (12, 7, 1.447_276_728_229_181_9),
        (24, 14, 4.528_620_535_514_615),
        (60, 36, 83.517_350_287_034_69),
    ] {
        // Interleave hits, avoiding an artificial run of misses at the start.
        let mut process = BernoulliCoverageProcess::new(0.8, 0.05).expect("valid design");
        for i in 0..n {
            process
                .observe((i + 1) * k / n > i * k / n)
                .expect("bounded");
        }
        let snapshot = process.snapshot();
        assert_eq!((snapshot.n, snapshot.hits), (n, k));
        close(snapshot.e_value().expect("finite"), expected, 3e-13);
        close(snapshot.threshold().expect("finite"), 20.0, 1e-14);
        assert_eq!(snapshot.alarmed(), n == 60);
    }
}

#[test]
fn coverage_all_short_paths_match_exact_evidence_and_first_crossing() {
    const HORIZON: u64 = 12;
    let mut crossing_probability_numerator = 0_u128;
    let mut mean_final_evidence = 0.0;
    for path in 0_u64..1 << HORIZON {
        let mut process = BernoulliCoverageProcess::new(0.8, 0.05).expect("valid design");
        let mut hits = 0;
        let mut first_crossing = None;
        let mut expected_peak = 1.0_f64;
        for n in 1..=HORIZON {
            let hit = path & (1 << (n - 1)) != 0;
            hits += u64::from(hit);
            let (num, den) = exact_e(n, hits);
            let expected_e = num as f64 / den as f64;
            if num >= 20 * den && first_crossing.is_none() {
                first_crossing = Some(n);
            }
            expected_peak = expected_peak.max(expected_e);
            let snapshot = process.observe(hit).expect("bounded");
            close(snapshot.e_value().expect("finite"), expected_e, 3e-13);
            close(
                snapshot.max_e_value().expect("finite"),
                expected_peak,
                3e-13,
            );
            assert_eq!(snapshot.first_crossing, first_crossing);
        }
        let probability_numerator = 4_u128.pow(hits as u32);
        if first_crossing.is_some() {
            crossing_probability_numerator += probability_numerator;
        }
        mean_final_evidence += probability_numerator as f64 / 5_u128.pow(HORIZON as u32) as f64
            * process.snapshot().e_value().expect("finite");
    }
    close(mean_final_evidence, 1.0, 1e-12);
    assert!(crossing_probability_numerator > 0);
    // Exact first-passage null probability, over every complete ordered path, satisfies Ville.
    assert!(20 * crossing_probability_numerator <= 5_u128.pow(HORIZON as u32));
}

#[test]
fn coverage_alarm_is_two_sided_and_sticky() {
    let mut process = BernoulliCoverageProcess::new(0.8, 0.05).expect("valid design");
    for _ in 0..3 {
        process.observe(false).expect("bounded");
    }
    let peak = process.snapshot();
    assert_eq!(peak.first_crossing, Some(3));
    for _ in 0..12 {
        process.observe(true).expect("bounded");
    }
    let after = process.snapshot();
    assert!(after.log_e_value < after.log_threshold);
    assert_eq!(after.first_crossing, Some(3));
    assert_eq!(after.max_log_e_value, peak.max_log_e_value);
    assert!(after.alarmed());

    let mut all_hits = BernoulliCoverageProcess::new(0.8, 0.05).expect("valid design");
    for _ in 0..60 {
        all_hits.observe(true).expect("bounded");
    }
    assert!(
        all_hits.snapshot().alarmed(),
        "excess coverage also rejects the point null"
    );
}

#[test]
fn coverage_invalid_parameters_refuse_and_valid_extremes_are_not_clamped() {
    for p in [
        f64::NAN,
        f64::INFINITY,
        f64::NEG_INFINITY,
        -0.1,
        0.0,
        1.0,
        2.0,
    ] {
        assert!(matches!(
            BernoulliCoverageProcess::new(p, 0.05),
            Err(CoverageError::NullProbabilityOutOfDomain(_))
        ));
        assert!(matches!(
            BernoulliCoverageProcess::new(0.8, p),
            Err(CoverageError::AlarmAlphaOutOfDomain(_))
        ));
    }
    let tiny = f64::from_bits(1);
    let almost_one = f64::from_bits(1.0_f64.to_bits() - 1);
    for p in [tiny, 1e-300, almost_one] {
        let mut process = BernoulliCoverageProcess::new(p, tiny).expect("valid extreme");
        let initial = process.snapshot();
        assert_eq!(
            (initial.n, initial.hits, initial.first_crossing),
            (0, 0, None)
        );
        assert_eq!(initial.e_value(), Some(1.0));
        assert!(initial.log_threshold.is_finite());
        assert_eq!(
            initial.threshold(),
            None,
            "1/min_subnormal cannot be represented"
        );
        for hit in [true, false, true, false] {
            let step = process.observe(hit).expect("finite logs");
            assert!(step.log_e_value.is_finite());
        }
    }
}

#[test]
fn coverage_log_domain_survives_huge_evidence_and_cap_is_atomic() {
    let mut process = BernoulliCoverageProcess::new(0.8, 0.05).expect("valid design");
    for _ in 0..MAX_OBSERVATIONS {
        process.observe(false).expect("within cap");
    }
    let state = process.snapshot();
    assert_eq!(
        (state.n, state.hits, state.first_crossing),
        (MAX_OBSERVATIONS, 0, Some(3))
    );
    assert!(state.log_e_value.is_finite());
    assert_eq!(
        state.e_value(),
        None,
        "overflow stays visible through authoritative log evidence"
    );
    assert_eq!(state.max_e_value(), None);
    // Independent closed form for zero hits: E_n = 1/((n+1) * (1-p)^n).
    let expected_log =
        -det::ln((MAX_OBSERVATIONS + 1) as f64) - MAX_OBSERVATIONS as f64 * det::ln(0.2);
    close(state.log_e_value, expected_log, 3e-14);
    let before = process.clone();
    assert_eq!(process.observe(true), Err(CoverageError::ObservationLimit));
    assert_eq!(process, before);
    assert_eq!(process.observe(false), Err(CoverageError::ObservationLimit));
    assert_eq!(process, before);
}

#[test]
fn coverage_sequence_order_retains_final_evidence_but_not_alarm_history() {
    let mut misses_first = BernoulliCoverageProcess::new(0.8, 0.05).expect("valid design");
    let mut interleaved = misses_first.clone();
    for index in 0..15 {
        misses_first.observe(index >= 3).expect("bounded");
        interleaved.observe(index % 5 != 0).expect("bounded");
    }
    close(
        misses_first.snapshot().log_e_value,
        interleaved.snapshot().log_e_value,
        1e-14,
    );
    assert_eq!(misses_first.snapshot().first_crossing, Some(3));
    assert_eq!(interleaved.snapshot().first_crossing, None);
}

#[test]
fn coverage_exact_boundary_retains_represented_log_comparison() {
    // INFERENCE: three hits at p=1/2 give B(4,1)/(1/2)^3 = 2 exactly. The implementation
    // approximates logarithms: at alarm_alpha=1/2 its first-crossing time can differ from the
    // exact-arithmetic answer. No epsilon silently promotes represented subthreshold evidence.
    for alpha in [0.5 - 1e-12, 0.5, 0.5 + 1e-12] {
        let mut process = BernoulliCoverageProcess::new(0.5, alpha).expect("valid design");
        for _ in 0..3 {
            process.observe(true).expect("bounded");
        }
        let snapshot = process.snapshot();
        close(snapshot.e_value().expect("finite"), 2.0, 2e-15);
        assert_eq!(
            snapshot.first_crossing,
            (snapshot.log_e_value >= snapshot.log_threshold).then_some(3),
        );
        if alpha < 0.5 {
            assert!(!snapshot.alarmed(), "mathematically below threshold");
        } else if alpha > 0.5 {
            assert!(snapshot.alarmed(), "mathematically above threshold");
        }
    }
}
