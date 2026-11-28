//! Deterministic distribution special functions for chi-square, non-central chi-square, and
//! standard normal p-values, critical values, and power calculations (`BD-150`).
//! Determinism: BitwisePortable (IEEE-754 arithmetic and `fmine_core::det` primitives only).

use fmine_core::det;

/// `1 / sqrt(2 * pi)`.
const INV_SQRT_TWO_PI: f64 = 0.398_942_280_401_432_7;

/// Standard normal CDF `Phi(z)`, using Laplace's continued fraction far in the tails (`|z| > 3`).
pub(crate) fn normal_cdf(z: f64) -> f64 {
    if z < -3.0 {
        lower_normal_tail(-z)
    } else if z > 3.0 {
        1.0 - lower_normal_tail(z)
    } else {
        det::norm_cdf(z)
    }
}

fn lower_normal_tail(x: f64) -> f64 {
    let mut f = x;
    for k in (1..=200_u32).rev() {
        f = x + f64::from(k) / f;
    }
    INV_SQRT_TWO_PI * det::exp(-0.5 * x * x) / f
}

/// Two-sided standard normal p-value `P(|Z| >= |z|)`.
pub(crate) fn normal_two_sided_p(z: f64) -> f64 {
    (2.0 * normal_cdf(-z.abs())).clamp(0.0, 1.0)
}

/// Exact two-sided rejection probability `P(|Z| >= z_crit)` for `Z ~ N(mu, 1)`.
pub(crate) fn normal_two_sided_power(mu: f64, z_crit: f64) -> f64 {
    (normal_cdf(mu - z_crit) + normal_cdf(-mu - z_crit)).clamp(0.0, 1.0)
}

/// Exact positive non-centrality `mu >= 0` achieving `normal_two_sided_power(mu, z_crit) == power`.
pub(crate) fn normal_two_sided_ncp_for_power(z_crit: f64, target_power: f64) -> f64 {
    let p0 = normal_two_sided_power(0.0, z_crit);
    if target_power <= p0 {
        return 0.0;
    }
    let mut lo = 0.0_f64;
    let mut hi = (z_crit + det::norm_ppf(target_power).abs() + 2.0).max(1.0);
    while normal_two_sided_power(hi, z_crit) < target_power && hi < 1e6 {
        hi *= 2.0;
    }
    for _ in 0..80 {
        let mid = 0.5 * (lo + hi);
        if normal_two_sided_power(mid, z_crit) < target_power {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    0.5 * (lo + hi)
}

/// `ln Gamma(x)` for `x >= 0.5` via 9-term Lanczos (`g = 7`).
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

fn gamma_p_series(a: f64, x: f64) -> f64 {
    if x <= 0.0 {
        return 0.0;
    }
    let mut ap = a;
    let mut term = 1.0 / a;
    let mut sum = term;
    for _ in 0..1_000 {
        ap += 1.0;
        term *= x / ap;
        sum += term;
        if term.abs() <= sum.abs() * 1e-15 {
            break;
        }
    }
    let log_pref = a * det::ln(x) - x - ln_gamma(a);
    (sum * det::exp(log_pref)).clamp(0.0, 1.0)
}

fn gamma_q_cf(a: f64, x: f64) -> f64 {
    const TINY: f64 = 1e-300;
    let mut b = x + 1.0 - a;
    let mut c = 1.0 / TINY;
    let mut d = 1.0 / if b.abs() < TINY { TINY } else { b };
    let mut h = d;
    for i in 1..=1_000_u32 {
        let fi = f64::from(i);
        let an = -fi * (fi - a);
        b += 2.0;
        d = an * d + b;
        if d.abs() < TINY {
            d = TINY;
        }
        c = b + an / c;
        if c.abs() < TINY {
            c = TINY;
        }
        d = 1.0 / d;
        let del = d * c;
        h *= del;
        if (del - 1.0).abs() <= 1e-15 {
            break;
        }
    }
    let log_pref = a * det::ln(x) - x - ln_gamma(a);
    (det::exp(log_pref) * h).clamp(0.0, 1.0)
}

/// Upper-tail survival probability `P(ChiSq_df >= x) = Q(df / 2, x / 2)`.
pub(crate) fn chi_square_upper_tail(x: f64, df: usize) -> f64 {
    if !x.is_finite() {
        return if x.is_sign_positive() { 0.0 } else { 1.0 };
    }
    if x <= 0.0 {
        return 1.0;
    }
    if df == 0 {
        return 0.0;
    }
    let a = 0.5 * (df as f64);
    let z = 0.5 * x;
    if z < a + 1.0 {
        (1.0 - gamma_p_series(a, z)).clamp(0.0, 1.0)
    } else {
        gamma_q_cf(a, z)
    }
}

/// Upper-tail critical value `c` solving `P(ChiSq_df >= c) = alpha` for `alpha in (0, 1)`.
pub(crate) fn chi_square_critical_upper(alpha: f64, df: usize) -> f64 {
    if df == 0 {
        return 0.0;
    }
    let k = df as f64;
    let z = det::norm_ppf(1.0 - alpha);
    // Wilson-Hilferty cube-root approximation gives a tight starting scale.
    let wh_inner = 1.0 - 2.0 / (9.0 * k) + z * det::sqrt(2.0 / (9.0 * k));
    let approx = if wh_inner.is_finite() && wh_inner > 0.0 {
        k * wh_inner * wh_inner * wh_inner
    } else {
        k
    };
    let mut lo = 0.0_f64;
    let mut hi = (approx * 2.0).max(k + 10.0);
    while chi_square_upper_tail(hi, df) > alpha && hi < 1e9 {
        hi *= 2.0;
    }
    for _ in 0..85 {
        let mid = 0.5 * (lo + hi);
        if chi_square_upper_tail(mid, df) > alpha {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    0.5 * (lo + hi)
}

/// Exact non-central chi-square upper-tail probability `P(ChiSq(df, lambda) >= x)`.
///
/// For moderate non-centrality (`lambda <= 1e5`), evaluates the exact Poisson mixture
/// `sum_{k>=0} Pois(k; lambda/2) * Q((df + 2k)/2, x/2)` starting outward from the Poisson mode
/// `k0 = floor(lambda / 2)` (so `10,000` terms cover `> 44 sigma` on each side without overflow).
/// For very large non-centrality (`lambda > 1e5`), uses Sankaran's (1963) three-moment power
/// normal transformation, preventing `usize` overflow and Poisson tail truncation.
pub(crate) fn noncentral_chi_square_upper_tail(x: f64, df: usize, lambda: f64) -> f64 {
    if !x.is_finite() {
        return if x.is_sign_positive() { 0.0 } else { 1.0 };
    }
    if x <= 0.0 {
        return 1.0;
    }
    if !lambda.is_finite() {
        return if lambda.is_sign_positive() { 1.0 } else { 0.0 };
    }
    if lambda <= 0.0 {
        return chi_square_upper_tail(x, df);
    }
    let half_lam = 0.5 * lambda;
    if half_lam > 5.0e4 {
        // Sankaran (1963) 3-moment normal approximation for large non-centrality.
        let k = df as f64;
        let mean = k + lambda;
        let h = 1.0
            - (2.0 / 3.0) * (k + lambda) * (k + 3.0 * lambda)
                / ((k + 2.0 * lambda) * (k + 2.0 * lambda));
        let p = (k + 2.0 * lambda) / (mean * mean);
        let m = (h - 1.0) * (1.0 - 3.0 * h);
        let mu_s = 1.0 + h * p * (h - 1.0 - 0.5 * (2.0 - h) * m * p);
        let sd_s = h * det::sqrt(2.0 * p) * (1.0 + 0.5 * m * p);
        let ratio_h = det::exp(h * det::ln(x / mean));
        let z = (ratio_h - mu_s) / sd_s;
        return (1.0 - normal_cdf(z)).clamp(0.0, 1.0);
    }
    let k0 = half_lam.floor() as usize;
    let k0_f = k0 as f64;
    let log_w0 = -half_lam + k0_f * det::ln(half_lam) - ln_gamma(k0_f + 1.0);
    let w0 = det::exp(log_w0);

    let mut total = w0 * chi_square_upper_tail(x, df + 2 * k0);

    // Sum upward from k0 + 1.
    let mut w = w0;
    let mut k = k0;
    for _ in 0..10_000 {
        k += 1;
        w *= half_lam / (k as f64);
        if w < 1e-16 {
            break;
        }
        total += w * chi_square_upper_tail(x, df + 2 * k);
    }

    // Sum downward from k0 - 1 to 0.
    let mut w_down = w0;
    for j in (0..k0).rev() {
        w_down *= ((j + 1) as f64) / half_lam;
        if w_down < 1e-16 {
            break;
        }
        total += w_down * chi_square_upper_tail(x, df + 2 * j);
    }

    total.clamp(0.0, 1.0)
}

/// Exact non-centrality `lambda >= 0` achieving `P(ChiSq(df, lambda) >= chi_crit) == target_power`.
pub(crate) fn noncentral_chi_square_ncp_for_power(
    chi_crit: f64,
    df: usize,
    target_power: f64,
) -> f64 {
    let p0 = chi_square_upper_tail(chi_crit, df);
    if target_power <= p0 {
        return 0.0;
    }
    let mut lo = 0.0_f64;
    let mut hi = (chi_crit + (df as f64) + 10.0).max(4.0);
    while noncentral_chi_square_upper_tail(chi_crit, df, hi) < target_power && hi < 1e8 {
        hi *= 2.0;
    }
    for _ in 0..85 {
        let mid = 0.5 * (lo + hi);
        if noncentral_chi_square_upper_tail(chi_crit, df, mid) < target_power {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    0.5 * (lo + hi)
}
