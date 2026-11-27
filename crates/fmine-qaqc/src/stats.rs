//! Deterministic special functions and distribution utilities shared by `fmine-qaqc`.
//! Determinism: BitwisePortable (IEEE-754 arithmetic and `fmine_core::det` primitives only).

use fmine_core::det;

/// `1 / sqrt(2 * pi)`.
pub(crate) const INV_SQRT_TWO_PI: f64 = 0.398_942_280_401_432_7;

/// Deterministic `base^exp` for strictly positive finite `base` using `det::exp` and `det::ln`.
pub(crate) fn pow_pos(base: f64, exp: f64) -> f64 {
    if exp == 0.0 {
        1.0
    } else if exp == 1.0 {
        base
    } else {
        det::exp(exp * det::ln(base))
    }
}

/// Standard normal density `phi(z)`.
pub(crate) fn normal_pdf(z: f64) -> f64 {
    INV_SQRT_TWO_PI * det::exp(-0.5 * z * z)
}

/// Standard normal CDF `Phi(z)`, using Laplace's continued fraction in the lower tail (`z < -3`)
/// where `1 + erf(z / sqrt(2))` cancels, and symmetry in the upper tail (`z > 3`).
pub(crate) fn normal_cdf(z: f64) -> f64 {
    if z < -3.0 {
        lower_normal_tail(-z)
    } else if z > 3.0 {
        1.0 - lower_normal_tail(z)
    } else {
        det::norm_cdf(z)
    }
}

/// `Phi(-x)` for `x >= 3` via Laplace's continued fraction `phi(x) / (x + 1/(x + 2/(x + ...)))`.
fn lower_normal_tail(x: f64) -> f64 {
    let mut f = x;
    for k in (1..=200_u32).rev() {
        f = x + f64::from(k) / f;
    }
    INV_SQRT_TWO_PI * det::exp(-0.5 * x * x) / f
}

/// `ln(Phi(z))` accurate even far in the left tail (for censored log-likelihoods).
pub(crate) fn ln_normal_cdf(z: f64) -> f64 {
    if z >= -3.0 {
        det::ln(normal_cdf(z).max(1e-300))
    } else {
        let x = -z;
        let mut f = x;
        for k in (1..=200_u32).rev() {
            f = x + f64::from(k) / f;
        }
        det::ln(INV_SQRT_TWO_PI) - 0.5 * x * x - det::ln(f)
    }
}

/// Inverse Mills ratio `phi(z) / Phi(z)`, stable far into the left tail (`z < -3`).
pub(crate) fn inverse_mills_lower(z: f64) -> f64 {
    if z >= -3.0 {
        let cdf = normal_cdf(z);
        if cdf <= 1e-300 {
            -z
        } else {
            normal_pdf(z) / cdf
        }
    } else {
        // phi(-x) / Phi(-x) = x + 1/(x + 2/(x + ...))
        let x = -z;
        let mut f = x;
        for k in (1..=200_u32).rev() {
            f = x + f64::from(k) / f;
        }
        f
    }
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

/// Regularized incomplete beta `I_x(a, b)` via modified Lentz continued fraction.
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
    let floor = |v: f64| if v.abs() < TINY { TINY } else { v };
    let (qab, qap, qam) = (a + b, a + 1.0, a - 1.0);
    let mut c = 1.0;
    let mut d = 1.0 / floor(1.0 - qab * x / qap);
    let mut h = d;
    for m in 1..=1_000_u32 {
        let mf = f64::from(m);
        let m2 = 2.0 * mf;
        let even = mf * (b - mf) * x / ((qam + m2) * (a + m2));
        d = 1.0 / floor(1.0 + even * d);
        c = floor(1.0 + even / c);
        h *= d * c;
        let odd = -(a + mf) * (qab + mf) * x / ((a + m2) * (qap + m2));
        d = 1.0 / floor(1.0 + odd * d);
        c = floor(1.0 + odd / c);
        let step = d * c;
        h *= step;
        if (step - 1.0).abs() < EPS {
            break;
        }
    }
    h
}

/// Two-sided Student-t p-value `P(|T_df| >= |t|)`.
pub(crate) fn t_two_sided_p(t: f64, df: u32) -> f64 {
    let v = f64::from(df);
    let x = v / (v + t * t);
    regularized_beta(v / 2.0, 0.5, x)
}

/// Two-sided Student-t critical value `t_{1 - alpha/2, df}` for `confidence = 1 - alpha in (0, 1)`.
pub(crate) fn t_critical_two_sided(confidence: f64, df: u32) -> f64 {
    let alpha = 1.0 - confidence;
    let mut lo = 0.0_f64;
    let mut hi = 1.0_f64;
    while t_two_sided_p(hi, df) > alpha && hi < 1e9 {
        hi *= 2.0;
    }
    for _ in 0..80 {
        let mid = 0.5 * (lo + hi);
        if t_two_sided_p(mid, df) > alpha {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    0.5 * (lo + hi)
}

/// Lower tail CDF of chi-square distribution `P(ChiSq_df <= x) = P(df/2, x/2)` using regularized
/// incomplete gamma series / Lentz continued fraction.
pub(crate) fn chi_square_lower_cdf(x: f64, df: u32) -> f64 {
    if x <= 0.0 {
        return 0.0;
    }
    let a = 0.5 * f64::from(df);
    let z = 0.5 * x;
    if z < a + 1.0 {
        gamma_p_series(a, z)
    } else {
        1.0 - gamma_q_cf(a, z)
    }
}

fn gamma_p_series(a: f64, x: f64) -> f64 {
    let mut ap = a;
    let mut term = 1.0 / a;
    let mut sum = term;
    for _ in 0..500 {
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
    for i in 1..=500_u32 {
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

/// Exact two-sided binomial p-value for `k` positive signs out of `n` non-zero trials under `p = 0.5`.
pub(crate) fn binomial_two_sided_half_p(k: usize, n: usize) -> f64 {
    if n == 0 {
        return 1.0;
    }
    let tail_k = k.min(n - k);
    let ln_n_fact = ln_gamma((n + 1) as f64);
    let ln_half = -det::ln(2.0);
    let mut tail = 0.0;
    for i in 0..=tail_k {
        let log_pmf = ln_n_fact - ln_gamma((i + 1) as f64) - ln_gamma((n - i + 1) as f64)
            + (n as f64) * ln_half;
        tail += det::exp(log_pmf);
    }
    (2.0 * tail).min(1.0)
}

/// Sample mean and unbiased sample standard deviation (`n - 1` denominator) via Welford's algorithm.
pub(crate) fn mean_and_sample_sd(values: &[f64]) -> Option<(f64, f64)> {
    let n = values.len();
    if n < 2 {
        return None;
    }
    let mut mean = 0.0;
    let mut m2 = 0.0;
    for (idx, &v) in values.iter().enumerate() {
        let count = (idx + 1) as f64;
        let delta = v - mean;
        mean += delta / count;
        let delta2 = v - mean;
        m2 += delta * delta2;
    }
    let variance = (m2 / ((n - 1) as f64)).max(0.0);
    Some((mean, det::sqrt(variance)))
}

/// Linearly interpolated quantile at `p in [0, 1]` on a non-empty pre-sorted ascending slice.
pub(crate) fn sorted_quantile(sorted: &[f64], p: f64) -> f64 {
    let n = sorted.len();
    if n == 1 {
        return sorted[0];
    }
    let pos = p.clamp(0.0, 1.0) * ((n - 1) as f64);
    let lo = pos.floor() as usize;
    let hi = (lo + 1).min(n - 1);
    let frac = pos - (lo as f64);
    sorted[lo] + frac * (sorted[hi] - sorted[lo])
}
