//! Determinism: DeterministicPerIsa

use super::*;

fn normal(mean: f64, std_dev: f64) -> GaussianParameters {
    GaussianParameters { mean, std_dev }
}

fn close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() <= 2e-13 * expected.abs().max(1.0),
        "actual={actual:.17e}, expected={expected:.17e}"
    );
}

#[test]
fn gaussian_difference_and_stationary_extrema_match_independent_known_answers() {
    // Independent normal-CDF / density closed forms evaluated with Python math.erf/exp,
    // not generated from the candidate. The third maximum is attained at y=4.
    for (left, right, y, expected, bounds) in [
        (
            normal(0.0, 2.0),
            normal(0.0, 1.0),
            0.0,
            0.233_694_977_255_109_13,
            (-0.564_189_583_547_756_3, 0.233_694_977_255_109_13),
        ),
        (
            normal(0.0, 1.0),
            normal(2.0, 1.0),
            0.0,
            -1.219_096_844_430_794_1,
            (-2.0, 2.0),
        ),
        (
            normal(0.0, 2.0),
            normal(2.0, 1.0),
            4.0,
            1.452_791_821_685_903_3,
            (-2.564_189_583_547_756, 1.452_791_821_685_903_3),
        ),
        (
            normal(0.0, 2.0),
            normal(2.0, 1.0),
            -1.0,
            -1.773_767_662_576_627_9,
            (-2.564_189_583_547_756, 1.452_791_821_685_903_3),
        ),
    ] {
        close(difference(left, right, y).unwrap(), expected);
        let actual = extrema(left, right).unwrap();
        close(actual.0, bounds.0);
        close(actual.1, bounds.1);
        let reverse = extrema(right, left).unwrap();
        close(reverse.0, -bounds.1);
        close(reverse.1, -bounds.0);
        close(difference(right, left, y).unwrap(), -expected);
    }
}

#[test]
fn gaussian_tail_differences_keep_small_signals_and_crossings_need_no_physical_coordinate() {
    // Subtracting independently rounded CRPS values at these outcomes gives zero instead of +/-1.
    let (left, right) = (normal(0.0, 1.0), normal(1.0, 1.0));
    assert_eq!(difference(left, right, 1e100), Some(1.0));
    assert_eq!(difference(left, right, -1e100), Some(-1.0));
    close(
        difference(normal(0.0, 2.0), left, 1e100).unwrap(),
        -0.564_189_583_547_756_3,
    );

    // The crossing z and physical y overflow. Its limiting expression is still representable.
    let beyond = extrema(normal(0.0, 1.0), normal(1e308, 1.0_f64.next_up())).unwrap();
    assert_eq!(beyond, (-1e308, 1e308));
    // Any individual score here overflows, but identical predictive distributions cancel exactly.
    let same = normal(-f64::MAX, 1.0);
    assert_eq!(difference(same, same, f64::MAX), Some(0.0));
    // A nonidentical symmetric pair at the midpoint also has representable zero difference.
    assert_eq!(
        difference(normal(-f64::MAX, 1.0), normal(f64::MAX, 1.0), 0.0),
        Some(0.0)
    );
    assert!(extrema(normal(-f64::MAX, 1.0), normal(f64::MAX, 1.0)).is_none());
}

#[test]
fn gaussian_global_candidates_bound_independent_grid_and_preserve_units_and_translation() {
    for (left, right) in [
        (normal(0.0, 2.0), normal(2.0, 1.0)),
        (normal(-3.0, 0.25), normal(7.0, 4.0)),
        (normal(8.0, 3.0), normal(-2.0, 3.0)),
        (normal(1.0, 2.0), normal(1.0, 2.0)),
    ] {
        let bounds = extrema(left, right).unwrap();
        for i in -160..=160 {
            let y = f64::from(i) * 0.25;
            // Existing scalar owner uses the usual closed form; the candidate evaluates a stable difference.
            let reference = crate::gaussian_crps(left.mean, left.std_dev, y).unwrap()
                - crate::gaussian_crps(right.mean, right.std_dev, y).unwrap();
            close(difference(left, right, y).unwrap(), reference);
            assert!(reference >= bounds.0 - 1e-12 && reference <= bounds.1 + 1e-12);
            close(
                difference(
                    normal(left.mean + 1e12, left.std_dev),
                    normal(right.mean + 1e12, right.std_dev),
                    y + 1e12,
                )
                .unwrap(),
                reference,
            );
            let factor = 1e120;
            close(
                difference(
                    normal(left.mean * factor, left.std_dev * factor),
                    normal(right.mean * factor, right.std_dev * factor),
                    y * factor,
                )
                .unwrap()
                    / factor,
                reference,
            );
        }
    }
}

#[test]
fn gaussian_tail_correction_preserves_scaled_subnormal_density_and_boundary_continuity() {
    // Independent 100-decimal mpmath erfc/density evaluation, including a density that underflows
    // before multiplying by sigma. These constants are not outputs of the candidate.
    for (scale, t, expected) in [
        (1.0, 4.0, 0.000_014_290_516_864_811_334),
        (1.0, 8.0, 1.510_052_482_389_299_8e-16),
        (1e308, 40.0, 1.825_668_944_582_594_5e-43),
    ] {
        let actual = tail_correction(scale, t);
        assert!(
            (actual / expected - 1.0).abs() < 2e-12,
            "{t}: {actual} versus {expected}"
        );
    }
    assert_eq!(tail_correction(f64::MAX, 55.0), 0.0);
    assert_eq!(tail_correction(1.0, f64::INFINITY), 0.0);
    assert!(
        (tail_correction(1.0, 4.0_f64.next_down()) / tail_correction(1.0, 4.0) - 1.0).abs() < 1e-10
    );
}

#[test]
fn gaussian_summary_keeps_missingness_and_avoids_intermediate_sum_overflow() {
    assert_eq!(
        mean([Some(f64::MAX), Some(f64::MAX)].into_iter()),
        Some(f64::MAX)
    );
    assert_eq!(
        mean([Some(f64::MAX), Some(-f64::MAX)].into_iter()),
        Some(0.0)
    );
    assert_eq!(mean([Some(1.0), None].into_iter()), None);
    assert_eq!(mean(std::iter::empty()), None);
}
