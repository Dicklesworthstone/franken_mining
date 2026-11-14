//! Determinism: DeterministicPerIsa (exact probabilities with floating-point score comparisons).

use super::*;

fn q(num: i128, den: i128) -> ExactQty {
    ExactQty::new(num, den).unwrap()
}

// Independent O(n²) definition, unlike the candidate's sorted positive-gap integral.
fn quadratic(samples: &[f64], weights: &[ExactQty], outcome: f64) -> f64 {
    let error: f64 = samples
        .iter()
        .zip(weights)
        .map(|(x, p)| p.to_f64() * (x - outcome).abs())
        .sum();
    let mut pairwise = 0.0;
    for (x, p) in samples.iter().zip(weights) {
        for (z, w) in samples.iter().zip(weights) {
            pairwise += p.to_f64() * w.to_f64() * (x - z).abs();
        }
    }
    error - pairwise / 2.0
}

#[test]
fn weighted_crps_matches_independent_definition_and_hand_answers() {
    for (outcome, expected) in [(-1.0, 3.25), (0.0, 2.25), (1.0, 1.75), (4.0, 0.25)] {
        assert_eq!(
            weighted_empirical_crps(&[0.0, 4.0], &[q(1, 4), q(3, 4)], outcome).unwrap(),
            expected
        );
    }
    for a in [-3.0, 0.0, 4.0] {
        for b in [-3.0, 0.0, 4.0] {
            for c in [-3.0, 0.0, 4.0] {
                let samples = [a, b, c];
                for numerators in [[1, 2, 7], [2, 3, 5], [8, 1, 1]] {
                    let weights = numerators.map(|n| q(n, 10));
                    for outcome in [-4.0, -3.0, -1.0, 0.0, 1.0, 4.0, 5.0] {
                        let actual = weighted_empirical_crps(&samples, &weights, outcome).unwrap();
                        let expected = quadratic(&samples, &weights, outcome);
                        assert!((actual - expected).abs() < 2e-14);
                    }
                }
            }
        }
    }
}

#[test]
fn weighted_quantiles_use_exact_mass_at_jumps_and_adjacent_decimal_levels() {
    let samples = [0.0, 1.0, 2.0];
    let weights = [q(1, 10), q(4, 5), q(1, 10)];
    for (alpha, expected) in [
        (0.2, (0.0, 1.0)),
        (f64::from_bits(0.2_f64.to_bits() + 1), (1.0, 1.0)),
        (f64::from_bits(0.2_f64.to_bits() - 1), (0.0, 2.0)),
    ] {
        assert_eq!(
            weighted_empirical_interval(&samples, &weights, alpha).unwrap(),
            expected
        );
    }
    // The first mass rounds to 0.1 as f64 but is strictly below the exact 1/10 jump.
    let denominator = 100_000_000_000_000_000_000_i128;
    let below = q(denominator / 10 - 1, denominator);
    assert_eq!(below.to_f64(), 0.1);
    assert_eq!(
        weighted_empirical_interval(&[2.0, 7.0], &[below, ONE.checked_sub(below).unwrap()], 0.2)
            .unwrap(),
        (7.0, 7.0)
    );
}

#[test]
fn weighted_scores_retain_tiny_tail_mass_and_avoid_cancellation() {
    let tail = q(1, 100_000_000_000_000_000_000);
    let large = ONE.checked_sub(tail).unwrap();
    assert_eq!(large.to_f64(), 1.0);
    let score = weighted_empirical_crps(&[0.0, 1e40], &[large, tail], 0.0).unwrap();
    assert!((score - 1.0).abs() < 4e-16);
    let offset = 1e16;
    assert_eq!(
        weighted_empirical_crps(&[offset, offset + 4.0], &[q(1, 4), q(3, 4)], offset).unwrap(),
        2.25
    );
    // A full signed gap overflows, while this probability-weighted positive contribution fits.
    let score =
        weighted_empirical_crps(&[-f64::MAX, f64::MAX], &[q(1, 2), q(1, 2)], -f64::MAX).unwrap();
    assert_eq!(score, f64::MAX / 2.0);
}

#[test]
fn weighted_permutation_ties_and_affine_changes_preserve_the_distribution() {
    let samples = [-2.0, 1.0, 5.0];
    let weights = [q(1, 5), q(3, 10), q(1, 2)];
    for outcome in [-4.0, 0.0, 1.0, 5.0, 8.0] {
        let expected = weighted_empirical_crps(&samples, &weights, outcome).unwrap();
        for indices in [[0, 2, 1], [1, 0, 2], [1, 2, 0], [2, 0, 1], [2, 1, 0]] {
            let permuted = indices.map(|i| samples[i]);
            let probabilities = indices.map(|i| weights[i]);
            assert_eq!(
                weighted_empirical_crps(&permuted, &probabilities, outcome)
                    .unwrap()
                    .to_bits(),
                expected.to_bits()
            );
        }
        let changed = samples.map(|x| 8.0 * x + 16.0);
        let actual = weighted_empirical_crps(&changed, &weights, 8.0 * outcome + 16.0).unwrap();
        assert!((actual - 8.0 * expected).abs() < 1e-13);
    }
    for outcome in [-1.0, 0.0, 1.0, 3.0] {
        let tied = weighted_empirical_crps(&[0.0, 2.0, 0.0], &[q(1, 4), q(1, 2), q(1, 4)], outcome)
            .unwrap();
        assert_eq!(
            tied,
            weighted_empirical_crps(&[0.0, 2.0], &[q(1, 2); 2], outcome).unwrap()
        );
        assert_eq!(
            tied,
            super::super::empirical_crps(&[0.0, 2.0, 0.0, 2.0], outcome).unwrap()
        );
    }
    assert_eq!(
        weighted_empirical_crps(&[3.0, 3.0], &[q(1, 7), q(6, 7)], -2.0).unwrap(),
        5.0
    );
    assert_eq!(
        weighted_empirical_interval(&[3.0, 3.0], &[q(1, 7), q(6, 7)], 0.2).unwrap(),
        (3.0, 3.0)
    );
}

#[test]
fn weighted_completion_bounds_match_convex_loss_extrema() {
    let distribution = SortedWeightedEmpirical::new(&[0.0, 4.0], &[q(1, 4), q(3, 4)]).unwrap();
    assert_eq!(distribution.bounds(None).unwrap(), (0.25, None));
    assert_eq!(
        distribution.bounds(Some((-2.0, 2.0))).unwrap(),
        (1.25, Some(4.25))
    );
    assert_eq!(
        distribution.bounds(Some((2.0, 6.0))).unwrap(),
        (0.25, Some(2.25))
    );
    assert_eq!(
        distribution.bounds(Some((6.0, 8.0))).unwrap(),
        (2.25, Some(4.25))
    );
    assert_eq!(
        distribution.bounds(Some((3.0, 3.0))).unwrap(),
        (0.75, Some(0.75))
    );
    for domain in [(f64::NAN, 4.0), (0.0, f64::INFINITY), (1.0, 0.0)] {
        assert!(distribution.bounds(Some(domain)).is_err());
    }
}

#[test]
fn weighted_invalid_populations_and_arithmetic_refuse_without_normalization() {
    for (samples, weights) in [
        (vec![], vec![]),
        (vec![1.0, 2.0], vec![ONE]),
        (vec![1.0, 2.0], vec![q(1, 3), q(1, 3)]),
        (vec![1.0, 2.0], vec![q(2, 3), q(2, 3)]),
        (vec![1.0, 2.0], vec![ZERO, ONE]),
        (vec![1.0, 2.0], vec![q(-1, 2), q(3, 2)]),
        (vec![f64::NAN], vec![ONE]),
        (vec![f64::INFINITY], vec![ONE]),
        (
            vec![0.0; MAX_EMPIRICAL_SAMPLES + 1],
            vec![ONE; MAX_EMPIRICAL_SAMPLES + 1],
        ),
    ] {
        assert!(weighted_empirical_crps(&samples, &weights, 1.0).is_err());
        assert!(weighted_empirical_interval(&samples, &weights, 0.2).is_err());
    }
    // Checked exact sum capacity is observable, never a tolerance-based "approximately one".
    let probabilities = [q(1, i128::MAX), q(1, i128::MAX - 1)];
    assert!(matches!(
        weighted_empirical_crps(&[0.0, 1.0], &probabilities, 1.0),
        Err(ScoreError::Arithmetic { .. })
    ));
    for alpha in [0.0, 1.0, -0.2, f64::NAN, f64::INFINITY, f64::MIN_POSITIVE] {
        assert!(weighted_empirical_interval(&[0.0], &[ONE], alpha).is_err());
    }
    for outcome in [f64::NAN, f64::INFINITY] {
        assert!(weighted_empirical_crps(&[0.0], &[ONE], outcome).is_err());
    }
    assert!(weighted_empirical_crps(&[-f64::MAX], &[ONE], f64::MAX).is_err());
    assert!(weighted_empirical_crps(&[0.0, f64::from_bits(1)], &[q(1, 2), q(1, 2)], 0.0,).is_err());
}
