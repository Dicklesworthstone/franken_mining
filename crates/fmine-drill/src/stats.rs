//! Statistics shared by the drill checks: Student's t p-values, ranks, Spearman's rank correlation, the
//! Mann–Whitney rank-sum test and sample quantiles.
//! Determinism: BitwisePortable (IEEE-754 arithmetic with `fmine_core::det` logarithms, exponentials, square roots
//! and the normal CDF).

use fmine_core::det;

/// `P(|T| ≥ |t|)` for Student's t with `df` degrees of freedom: `I_{df/(df+t²)}(df/2, 1/2)`.
pub(crate) fn t_two_sided_p(t: f64, df: u32) -> f64 {
    let v = f64::from(df);
    let x = v / (v + t * t);
    regularized_beta(v / 2.0, 0.5, x)
}

/// `I_x(a, b)` by the continued fraction (modified Lentz), on the side where it converges fast.
fn regularized_beta(a: f64, b: f64, x: f64) -> f64 {
    if x <= 0.0 {
        return 0.0;
    }
    if x >= 1.0 {
        return 1.0;
    }
    let front = det::exp(
        ln_gamma(a + b) - ln_gamma(a) - ln_gamma(b) + a * det::ln(x) + b * det::ln(1.0 - x),
    );
    if x < (a + 1.0) / (a + b + 2.0) {
        front * beta_fraction(a, b, x) / a
    } else {
        1.0 - front * beta_fraction(b, a, 1.0 - x) / b
    }
}

fn beta_fraction(a: f64, b: f64, x: f64) -> f64 {
    const TINY: f64 = 1e-300;
    const EPS: f64 = 1e-16;
    let clamp = |v: f64| if v.abs() < TINY { TINY } else { v };
    let (qab, qap, qam) = (a + b, a + 1.0, a - 1.0);
    let mut c = 1.0;
    let mut d = 1.0 / clamp(1.0 - qab * x / qap);
    let mut h = d;
    for m in 1..=1_000_u32 {
        let m = f64::from(m);
        let m2 = 2.0 * m;
        let even = m * (b - m) * x / ((qam + m2) * (a + m2));
        d = 1.0 / clamp(1.0 + even * d);
        c = clamp(1.0 + even / c);
        h *= d * c;
        let odd = -(a + m) * (qab + m) * x / ((a + m2) * (qap + m2));
        d = 1.0 / clamp(1.0 + odd * d);
        c = clamp(1.0 + odd / c);
        let step = d * c;
        h *= step;
        if (step - 1.0).abs() < EPS {
            break;
        }
    }
    h
}

/// `ln Γ(x)` for `x ≥ 0.5` (Lanczos, g = 7, nine terms).
pub(crate) fn ln_gamma(x: f64) -> f64 {
    const G: f64 = 7.0;
    const C: [f64; 9] = [
        0.999_999_999_999_809_9,
        676.520_368_121_885_1,
        -1_259.139_216_722_402_8,
        771.323_428_777_653_1,
        -176.615_029_162_140_6,
        12.507_343_278_686_905,
        -0.138_571_095_265_720_12,
        9.984_369_578_019_572e-6,
        1.505_632_735_149_311_6e-7,
    ];
    const HALF_LN_TWO_PI: f64 = 0.918_938_533_204_672_7;
    let x = x - 1.0;
    let mut sum = C[0];
    for (i, c) in (1_u32..).zip(&C[1..]) {
        sum += c / (x + f64::from(i));
    }
    let t = x + G + 0.5;
    HALF_LN_TWO_PI + (x + 0.5) * det::ln(t) - t + det::ln(sum)
}

/// `Φ(z)`, the standard normal CDF, accurate in the lower tail: below `z = −3` by Laplace's continued fraction
/// `φ(x)/(x + 1/(x + 2/(x + …)))` with `x = −z`, where `1 + erf` would cancel to zero; otherwise `det::norm_cdf`.
pub(crate) fn normal_cdf(z: f64) -> f64 {
    if z >= -3.0 {
        return det::norm_cdf(z);
    }
    let x = -z;
    let mut f = x;
    for k in (1..=200_u32).rev() {
        f = x + f64::from(k) / f;
    }
    // φ(x) = exp(−x²/2)/√(2π).
    const INV_SQRT_TWO_PI: f64 = 0.398_942_280_401_432_7;
    INV_SQRT_TWO_PI * det::exp(-0.5 * x * x) / f
}

/// Ranks `1..=n` of `values` in input order, ties sharing their average rank.
pub(crate) fn average_ranks(values: &[f64]) -> Vec<f64> {
    let mut order: Vec<usize> = (0..values.len()).collect();
    order.sort_by(|&a, &b| values[a].total_cmp(&values[b]));
    let mut ranks = vec![0.0; values.len()];
    let mut start = 0;
    while start < order.len() {
        let mut end = start + 1;
        while end < order.len() && values[order[end]] == values[order[start]] {
            end += 1;
        }
        // Ranks start + 1 ..= end share their mean.
        #[allow(clippy::cast_precision_loss)]
        let rank = (start + end + 1) as f64 / 2.0;
        for &i in &order[start..end] {
            ranks[i] = rank;
        }
        start = end;
    }
    ranks
}

/// `Σ (t³ − t)` over the tie runs of `values`.
fn tie_sum(values: &[f64]) -> f64 {
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    sorted
        .chunk_by(|a, b| a == b)
        .map(|run| {
            #[allow(clippy::cast_precision_loss)]
            let t = run.len() as f64;
            t * t * t - t
        })
        .sum()
}

/// Spearman's rank correlation with its two-sided p.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct RankCorrelation {
    /// Pearson's correlation of the average ranks.
    pub rho: f64,
    /// Two-sided p from `t = ρ·√((n−2)/(1−ρ²))` on `n − 2` degrees of freedom.
    pub p_two_sided: f64,
}

/// Spearman's ρ of `x` and `y` (equal lengths); `None` with fewer than three pairs or a side whose values are all
/// equal.
pub(crate) fn spearman(x: &[f64], y: &[f64]) -> Option<RankCorrelation> {
    let n = x.len();
    if n < 3 || y.len() != n {
        return None;
    }
    let (rx, ry) = (average_ranks(x), average_ranks(y));
    #[allow(clippy::cast_precision_loss)]
    let mean = (n + 1) as f64 / 2.0;
    let (mut sxx, mut syy, mut sxy) = (0.0, 0.0, 0.0);
    for (a, b) in rx.iter().zip(&ry) {
        let (da, db) = (a - mean, b - mean);
        sxx += da * da;
        syy += db * db;
        sxy += da * db;
    }
    if sxx == 0.0 || syy == 0.0 {
        return None;
    }
    let rho = (sxy / det::sqrt(sxx * syy)).clamp(-1.0, 1.0);
    let df = u32::try_from(n - 2).ok()?;
    let p = if rho.abs() >= 1.0 {
        0.0
    } else {
        t_two_sided_p(rho * det::sqrt(f64::from(df) / (1.0 - rho * rho)), df)
    };
    Some(RankCorrelation {
        rho,
        p_two_sided: p,
    })
}

/// The Mann–Whitney rank-sum test of `a` against `b`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct RankSum {
    /// `U` of `a`: pairs where `a` is above `b`, plus half the ties.
    pub u: f64,
    /// Normal score of `U` with the tie and continuity corrections.
    pub z: f64,
    /// Two-sided p, `2·Φ(−|z|)`.
    pub p_two_sided: f64,
}

/// Mann–Whitney of `a` against `b`; `None` when either is empty or every value is equal.
pub(crate) fn mann_whitney(a: &[f64], b: &[f64]) -> Option<RankSum> {
    if a.is_empty() || b.is_empty() {
        return None;
    }
    let all: Vec<f64> = a.iter().chain(b).copied().collect();
    let ranks = average_ranks(&all);
    #[allow(clippy::cast_precision_loss)]
    let (na, nb, n) = (a.len() as f64, b.len() as f64, all.len() as f64);
    let rank_sum_a: f64 = ranks[..a.len()].iter().sum();
    let u = rank_sum_a - na * (na + 1.0) / 2.0;
    let mean = na * nb / 2.0;
    let variance = na * nb / 12.0 * ((n + 1.0) - tie_sum(&all) / (n * (n - 1.0)));
    if variance <= 0.0 {
        return None;
    }
    let gap = u - mean;
    let corrected = if gap.abs() <= 0.5 {
        0.0
    } else {
        gap - 0.5_f64.copysign(gap)
    };
    let z = corrected / det::sqrt(variance);
    Some(RankSum {
        u,
        z,
        p_two_sided: (2.0 * normal_cdf(-z.abs())).min(1.0),
    })
}

/// The type-7 (linear) sample quantile of ascending `sorted` at probability `p` in [0, 1]; `None` when empty.
pub(crate) fn quantile(sorted: &[f64], p: f64) -> Option<f64> {
    let last = sorted.len().checked_sub(1)?;
    #[allow(clippy::cast_precision_loss)]
    let h = p * last as f64;
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let lo = (h.floor() as usize).min(last);
    let hi = (lo + 1).min(last);
    #[allow(clippy::cast_precision_loss)]
    let frac = h - lo as f64;
    Some(sorted[lo] + frac * (sorted[hi] - sorted[lo]))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Two-sided p from the closed-form t CDFs for 1 to 5 degrees of freedom (`det` arctangent and roots, which the
    /// incomplete-beta path does not use).
    fn closed_form_p(t: f64, df: u32) -> f64 {
        let pi = std::f64::consts::PI;
        let cdf = match df {
            1 => 0.5 + det::atan(t) / pi,
            2 => 0.5 + t / (2.0 * det::sqrt(2.0 + t * t)),
            3 => {
                let r = t / det::sqrt(3.0);
                0.5 + (r / (1.0 + t * t / 3.0) + det::atan(r)) / pi
            }
            4 => {
                let u = 1.0 + t * t / 4.0;
                0.5 + 0.375 * (t / det::sqrt(u)) * (1.0 - t * t / (12.0 * u))
            }
            5 => {
                let r = t / det::sqrt(5.0);
                let u = 1.0 + t * t / 5.0;
                0.5 + ((r / u) * (1.0 + 2.0 / (3.0 * u)) + det::atan(r)) / pi
            }
            _ => unreachable!(),
        };
        2.0 * (1.0 - cdf)
    }

    #[test]
    fn test_t_p_values_match_closed_forms_tables_and_the_normal_limit() {
        let mut state = 0x0285_7EA5_u64;
        let mut worst = 0.0_f64;
        for _ in 0..5_000 {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            #[allow(clippy::cast_precision_loss)]
            let t = (state >> 11) as f64 / (1_u64 << 53) as f64 * 40.0;
            for df in 1..=5 {
                let got = t_two_sided_p(t, df);
                let want = closed_form_p(t, df);
                worst = worst.max((got - want).abs());
                assert!((got - want).abs() < 1e-13, "t {t} df {df}: {got} vs {want}");
            }
        }
        // Published two-sided critical values.
        for (t, df, alpha) in [
            (2.262_157_162_740_991, 9, 0.05),
            (2.042_272_456_301_238, 30, 0.05),
            (1.979_930_405_052_777, 120, 0.05),
            (3.169_272_672_616_951, 10, 0.01),
        ] {
            let p = t_two_sided_p(t, df);
            assert!((p - alpha).abs() < 1e-9, "t {t} df {df}: {p}");
        }
        // Far out in degrees of freedom the t is the normal.
        let p = t_two_sided_p(1.959_963_984_540_054, 1_000_000);
        assert!((p - 0.05).abs() < 1e-5, "{p}");
        assert_eq!(t_two_sided_p(0.0, 7), 1.0);
        println!("t p-values: worst |error| against the closed forms {worst:.2e}");
    }

    #[test]
    fn test_ln_gamma_matches_factorials_and_the_half_integer() {
        let mut fact = 1.0_f64;
        for k in 1..=30_u32 {
            let got = ln_gamma(f64::from(k));
            let want = det::ln(fact);
            assert!(
                (got - want).abs() <= 1e-12 * want.abs().max(1.0),
                "Γ({k}): {got} vs {want}"
            );
            fact *= f64::from(k);
        }
        // Γ(1/2) = √π.
        let half = ln_gamma(0.5);
        let want = 0.5 * det::ln(std::f64::consts::PI);
        assert!((half - want).abs() < 1e-13, "{half} vs {want}");
    }

    #[test]
    fn test_the_normal_tail_keeps_its_digits_where_one_plus_erf_cancels() {
        // Φ(−x) to the digits published for the standard normal.
        for (z, want) in [
            (-3.0, 1.349_898_031_630_094_6e-3),
            (-5.0, 2.866_515_718_791_939e-7),
            (-10.0, 7.619_853_024_160_527e-24),
            (-20.0, 2.753_624_118_606_233_6e-89),
        ] {
            let got = normal_cdf(z);
            assert!(
                (got - want).abs() <= 1e-12 * want,
                "Φ({z}) = {got:e}, want {want:e}"
            );
        }
        assert_eq!(normal_cdf(-2.0), det::norm_cdf(-2.0));
        // Continuous where the two paths meet.
        let (below, above) = (normal_cdf(-3.0 - 1e-12), det::norm_cdf(-3.0 + 1e-12));
        assert!((below - above).abs() < 1e-14, "{below} {above}");
    }

    #[test]
    fn test_ranks_spearman_rank_sum_and_quantiles_have_their_known_answers() {
        assert_eq!(
            average_ranks(&[3.0, 1.0, 2.0, 2.0]),
            vec![4.0, 1.0, 2.5, 2.5]
        );
        // Spearman on the ten-subject IQ and television example: ρ = −29/165, p = 0.627188 on 8 df.
        let iq = [
            106.0, 100.0, 86.0, 101.0, 99.0, 103.0, 97.0, 113.0, 112.0, 110.0,
        ];
        let tv = [7.0, 27.0, 2.0, 50.0, 28.0, 29.0, 20.0, 12.0, 6.0, 17.0];
        let r = spearman(&iq, &tv).unwrap();
        assert!((r.rho + 29.0 / 165.0).abs() < 1e-15, "{r:?}");
        assert!((r.p_two_sided - 0.627_188).abs() < 1e-6, "{r:?}");
        let up: Vec<f64> = (1..=10).map(f64::from).collect();
        let squares: Vec<f64> = up.iter().map(|x| x * x).collect();
        assert_eq!(
            spearman(&up, &squares).map(|r| (r.rho, r.p_two_sided)),
            Some((1.0, 0.0))
        );
        assert_eq!(spearman(&up, &[1.0; 10]), None);
        assert_eq!(spearman(&up[..2], &up[..2]), None);

        // U equals the pair count; separated samples give the hand-worked z.
        let mut state = 0x0286_u64;
        let mut draw = |n: usize| -> Vec<f64> {
            (0..n)
                .map(|_| {
                    state ^= state << 13;
                    state ^= state >> 7;
                    state ^= state << 17;
                    f64::from(u32::try_from(state % 9).unwrap())
                })
                .collect()
        };
        for (na, nb) in [(1, 1), (3, 5), (12, 7), (40, 40)] {
            let (a, b) = (draw(na), draw(nb));
            let pairs: f64 = a
                .iter()
                .flat_map(|x| {
                    b.iter().map(move |y| match x.total_cmp(y) {
                        std::cmp::Ordering::Greater => 1.0,
                        std::cmp::Ordering::Equal => 0.5,
                        std::cmp::Ordering::Less => 0.0,
                    })
                })
                .sum();
            if let Some(r) = mann_whitney(&a, &b) {
                assert_eq!(r.u, pairs, "{na}x{nb}");
            }
        }
        let r = mann_whitney(&[1.0, 2.0, 3.0], &[4.0, 5.0, 6.0]).unwrap();
        assert_eq!(r.u, 0.0);
        assert!((r.z + 4.0 / 5.25_f64.sqrt()).abs() < 1e-15, "{r:?}");
        assert_eq!(mann_whitney(&[2.0, 2.0], &[2.0]), None);
        assert_eq!(mann_whitney(&[], &[2.0]), None);

        let q = [1.0, 2.0, 3.0, 4.0];
        assert_eq!(quantile(&q, 0.5), Some(2.5));
        assert!((quantile(&q, 0.1).unwrap() - 1.3).abs() < 1e-15);
        assert_eq!(
            (quantile(&q, 0.0), quantile(&q, 1.0), quantile(&[], 0.5)),
            (Some(1.0), Some(4.0), None)
        );
    }
}
