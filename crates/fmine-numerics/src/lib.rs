//! Deterministic numerical kernels (`WP-02`, `BD-022`):
//! Determinism: DeterministicPerIsa (`norm_cdf` and `norm_inv` call std `exp`, `ln` and `sqrt`; `brent_root` and
//! `cholesky_solve` use only `+ − × ÷` and `sqrt`).
//! - Dense SPD Cholesky solver (`cholesky_solve`, `FMINE-REF-301`)
//! - Brent's bracketed root-finder (`brent_root`), which refuses on a missing sign change, a non-finite value or
//!   non-convergence and never returns an unconverged iterate (bd-t1dk)
//! - Standard normal CDF (`norm_cdf`) and Acklam inverse quantile (`norm_inv`)
//! - Lifted `frankensim` modules: `eproc`, `voi`, `risk`, `tropical`.

pub mod eproc;
pub mod risk;
pub mod tropical;
pub mod voi;

use fmine_evidence::Refusal;

/// Solves $A x = b$ for an $n \times n$ row-major symmetric positive-definite matrix `a`
/// via Cholesky factorization $A = L L^\top$.
///
/// Refuses with `FMINE-REF-301` if `a` is not positive-definite (`BD-022`).
pub fn cholesky_solve(a: &[f64], b: &[f64], n: usize) -> Result<Vec<f64>, Refusal> {
    if n == 0 || a.len() != n * n || b.len() != n {
        return Err(Refusal::from_code(
            "FMINE-REF-301",
            format!("invalid matrix dimensions n={n}, a.len()={}, b.len()={}", a.len(), b.len()),
        ));
    }
    let mut l = vec![0.0_f64; n * n];
    for i in 0..n {
        for j in 0..=i {
            let mut sum = a[i * n + j];
            for k in 0..j {
                sum -= l[i * n + k] * l[j * n + k];
            }
            if i == j {
                if !sum.is_finite() || sum <= 1e-14 {
                    return Err(Refusal::from_code(
                        "FMINE-REF-301",
                        format!("matrix is not positive-definite at pivot {i} (diag={sum})"),
                    ));
                }
                l[i * n + j] = sum.sqrt();
            } else {
                l[i * n + j] = sum / l[j * n + j];
            }
        }
    }

    // Forward substitution L y = b
    let mut y = vec![0.0_f64; n];
    for i in 0..n {
        let mut sum = b[i];
        for k in 0..i {
            sum -= l[i * n + k] * y[k];
        }
        y[i] = sum / l[i * n + i];
    }

    // Back substitution L^T x = y
    let mut x = vec![0.0_f64; n];
    for i in (0..n).rev() {
        let mut sum = y[i];
        for k in (i + 1)..n {
            sum -= l[k * n + i] * x[k];
        }
        x[i] = sum / l[i * n + i];
    }
    Ok(x)
}

/// A converged root from [`brent_root`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Root {
    /// The root estimate.
    pub x: f64,
    /// `f(x)`.
    pub f_x: f64,
    /// The final bracket `(lo, hi)` around `x` (ordered).
    pub bracket: (f64, f64),
    /// Iterations used.
    pub iterations: usize,
}

/// Why [`brent_root`] refused. There is no best-effort result.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RootError {
    /// `f(a)` and `f(b)` have the same strict sign, so `[a, b]` does not bracket a root.
    NoSignChange { fa: f64, fb: f64 },
    /// `f` returned NaN or an infinity at `at` (an endpoint or an iterate).
    NonFinite { at: f64 },
    /// `max_iter` iterations ran without meeting `tol`.
    NotConverged { iterations: usize, bracket_width: f64, f_b: f64 },
}

impl std::fmt::Display for RootError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoSignChange { fa, fb } => write!(f, "no sign change on the bracket: f(a) {fa}, f(b) {fb}"),
            Self::NonFinite { at } => write!(f, "f is not finite at {at}"),
            Self::NotConverged { iterations, bracket_width, f_b } => write!(
                f,
                "no convergence after {iterations} iterations (bracket width {bracket_width}, f(b) {f_b})"
            ),
        }
    }
}

impl std::error::Error for RootError {}

/// Finds a root $x \in [a, b]$ of $f(x) = 0$ using deterministic bisection-secant (Dekker/Brent)
/// to tolerance `tol`.
///
/// Returns [`RootError::NotConverged`] when `max_iter` runs out; before bd-t1dk it returned the last iterate,
/// which is not a root.
pub fn brent_root<F>(mut a: f64, mut b: f64, tol: f64, max_iter: usize, mut f: F) -> Result<Root, RootError>
where
    F: FnMut(f64) -> f64,
{
    let mut fa = f(a);
    if !fa.is_finite() {
        return Err(RootError::NonFinite { at: a });
    }
    let mut fb = f(b);
    if !fb.is_finite() {
        return Err(RootError::NonFinite { at: b });
    }
    if (fa > 0.0 && fb > 0.0) || (fa < 0.0 && fb < 0.0) {
        return Err(RootError::NoSignChange { fa, fb });
    }
    if fa.abs() < fb.abs() {
        std::mem::swap(&mut a, &mut b);
        std::mem::swap(&mut fa, &mut fb);
    }
    let mut c = a;
    let mut fc = fa;
    let mut d = b - a;
    let mut e = d;

    let done = |x: f64, f_x: f64, other: f64, iterations: usize| Root {
        x,
        f_x,
        bracket: (x.min(other), x.max(other)),
        iterations,
    };
    for iter in 0..max_iter {
        if fb.abs() <= tol {
            return Ok(done(b, fb, c, iter));
        }
        if (fb > 0.0 && fc > 0.0) || (fb < 0.0 && fc < 0.0) {
            c = a;
            fc = fa;
            d = b - a;
            e = d;
        }
        if fc.abs() < fb.abs() {
            a = b;
            b = c;
            c = a;
            fa = fb;
            fb = fc;
            fc = fa;
        }
        let tol1 = 2.0 * f64::EPSILON * b.abs() + 0.5 * tol;
        let xm = 0.5 * (c - b);
        if xm.abs() <= tol1 {
            return Ok(done(b, fb, c, iter));
        }
        if e.abs() >= tol1 && fa.abs() > fb.abs() {
            let s = fb / fa;
            let (mut p, mut q) = if (a - c).abs() < 1e-15 {
                (2.0 * xm * s, 1.0 - s)
            } else {
                let q0 = fa / fc;
                let r = fb / fc;
                (
                    s * (2.0 * xm * q0 * (q0 - r) - (b - a) * (r - 1.0)),
                    (q0 - 1.0) * (r - 1.0) * (s - 1.0),
                )
            };
            if p > 0.0 {
                q = -q;
            }
            p = p.abs();
            if 2.0 * p < (3.0 * xm * q - (tol1 * q).abs()).min((e * q).abs()) {
                e = d;
                d = p / q;
            } else {
                d = xm;
                e = d;
            }
        } else {
            d = xm;
            e = d;
        }
        a = b;
        fa = fb;
        if d.abs() > tol1 {
            b += d;
        } else if xm >= 0.0 {
            b += tol1;
        } else {
            b -= tol1;
        }
        fb = f(b);
        if !fb.is_finite() {
            return Err(RootError::NonFinite { at: b });
        }
    }
    Err(RootError::NotConverged { iterations: max_iter, bracket_width: (c - b).abs(), f_b: fb })
}

/// Standard normal cumulative distribution function $\Phi(z)$ (Abramowitz & Stegun 7.1.26, error $< 1.5 \times 10^{-7}$).
#[must_use]
pub fn norm_cdf(z: f64) -> f64 {
    let x = z / std::f64::consts::SQRT_2;
    let sign = if x < 0.0 { -1.0 } else { 1.0 };
    let ax = x.abs();
    let t = 1.0 / (1.0 + 0.327_591_1 * ax);
    let y = 1.0
        - (((((1.061_405_429 * t - 1.453_152_027) * t) + 1.421_413_741) * t - 0.284_496_736) * t
            + 0.254_829_592)
            * t
            * (-ax * ax).exp();
    0.5 * (1.0 + sign * y)
}

/// Acklam rational approximation of the standard normal inverse CDF $\Phi^{-1}(p)$ for $p \in (0, 1)$.
#[must_use]
pub fn norm_inv(p: f64) -> f64 {
    let u = p.clamp(1e-15, 1.0 - 1e-15);
    const A: [f64; 6] = [
        -3.969_683_028_665_376e1,
        2.209_460_984_245_205e2,
        -2.759_285_104_469_687e2,
        1.383_577_518_672_69e2,
        -3.066_479_806_614_716e1,
        2.506_628_277_459_239,
    ];
    const B: [f64; 5] = [
        -5.447_609_879_822_406e1,
        1.615_858_368_580_409e2,
        -1.556_989_798_598_866e2,
        6.680_131_188_771_972e1,
        -1.328_068_155_288_572e1,
    ];
    const C: [f64; 6] = [
        -7.784_894_002_430_293e-3,
        -3.223_964_580_411_365e-1,
        -2.400_758_277_161_838,
        -2.549_732_539_343_734,
        4.374_664_141_464_968,
        2.938_163_982_698_783,
    ];
    const D: [f64; 4] = [
        7.784_695_709_041_462e-3,
        3.224_671_290_700_398e-1,
        2.445_134_137_142_996,
        3.754_408_661_907_416,
    ];
    let p_low = 0.02425;
    let p_high = 1.0 - p_low;
    if u < p_low {
        let q = (-2.0 * u.ln()).sqrt();
        (((((C[0] * q + C[1]) * q + C[2]) * q + C[3]) * q + C[4]) * q + C[5])
            / ((((D[0] * q + D[1]) * q + D[2]) * q + D[3]) * q + 1.0)
    } else if u <= p_high {
        let q = u - 0.5;
        let r = q * q;
        (((((A[0] * r + A[1]) * r + A[2]) * r + A[3]) * r + A[4]) * r + A[5]) * q
            / (((((B[0] * r + B[1]) * r + B[2]) * r + B[3]) * r + B[4]) * r + 1.0)
    } else {
        let q = (-2.0 * (1.0 - u).ln()).sqrt();
        -(((((C[0] * q + C[1]) * q + C[2]) * q + C[3]) * q + C[4]) * q + C[5])
            / ((((D[0] * q + D[1]) * q + D[2]) * q + D[3]) * q + 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cholesky_solve_and_singular_refusal() {
        let a = [4.0, 2.0, 2.0, 3.0];
        let b = [10.0, 7.0];
        let x = cholesky_solve(&a, &b, 2).expect("SPD system");
        assert!((x[0] - 2.0).abs() < 1e-12);
        assert!((x[1] - 1.0).abs() < 1e-12);

        let singular = [1.0, 2.0, 2.0, 4.0];
        let err = cholesky_solve(&singular, &b, 2).expect_err("singular must refuse");
        assert_eq!(err.code, "FMINE-REF-301");
    }

    #[test]
    fn test_brent_root_and_normal_roundtrip() {
        let root = brent_root(1.0, 2.0, 1e-12, 100, |x| x * x - 2.0).expect("sqrt(2)");
        assert!((root.x - std::f64::consts::SQRT_2).abs() < 1e-11);
        assert!(root.f_x.abs() <= 1e-12, "f(x) = {}", root.f_x);
        assert!(root.bracket.0 <= root.x && root.x <= root.bracket.1);

        for &p in &[0.01, 0.10, 0.25, 0.50, 0.75, 0.90, 0.99] {
            let z = norm_inv(p);
            let p_back = norm_cdf(z);
            assert!((p_back - p).abs() < 1e-5, "failed at p={p}: got {p_back}");
        }
    }

    /// Regression for the 2026-10-29 defect (bd-t1dk): with `max_iter` too small, `brent_root` returned the last
    /// iterate as `Some(x)` although `x` is not a root.
    #[test]
    fn test_brent_refuses_non_convergence_and_bad_inputs() {
        let err = brent_root(0.0, 2.0, 1e-15, 3, |x| x * x * x - 2.0).expect_err("3 iterations are too few");
        assert!(matches!(err, RootError::NotConverged { iterations: 3, .. }), "{err:?}");

        assert_eq!(
            brent_root(1.0, 2.0, 1e-12, 100, |x| x * x + 1.0),
            Err(RootError::NoSignChange { fa: 2.0, fb: 5.0 })
        );
        assert_eq!(
            brent_root(1.0, 2.0, 1e-12, 100, |_| f64::NAN),
            Err(RootError::NonFinite { at: 1.0 })
        );
        // Finite at both ends, NaN everywhere inside: the first interior iterate refuses.
        let err = brent_root(1.0, 2.0, 1e-12, 100, |x| if x > 1.0 && x < 2.0 { f64::NAN } else { x - 1.5 })
            .expect_err("interior NaN");
        assert!(matches!(err, RootError::NonFinite { at } if at > 1.0 && at < 2.0), "{err:?}");
    }

    /// Seeded property over monotone cubics `(x − r)^3 + k (x − r)` with a known root `r`: converged results lie
    /// within tolerance of `r`, and an `Ok` is never returned without convergence.
    #[test]
    fn test_brent_property_monotone_cubics() {
        use fmine_core::rand::{PhiloxStream, RngKey};
        let mut rng = PhiloxStream::new(RngKey {
            seed_root: 0xB2E7,
            partition: 0,
            domain: 0,
            var_group: 0,
            realization: 0,
            brick: 0,
        });
        for case in 0..500 {
            let r = -5.0 + 10.0 * rng.next_f64_open01();
            let k = 0.1 + 5.0 * rng.next_f64_open01();
            let lo = r - (0.1 + 10.0 * rng.next_f64_open01());
            let hi = r + (0.1 + 10.0 * rng.next_f64_open01());
            let f = |x: f64| (x - r).powi(3) + k * (x - r);
            let root = brent_root(lo, hi, 1e-12, 200, f).unwrap_or_else(|e| panic!("case {case}: {e}"));
            assert!((root.x - r).abs() <= 1e-9, "case {case}: x {} vs r {r}", root.x);
            // With two iterations the result is either a genuine convergence or a refusal, never a bare iterate.
            match brent_root(lo, hi, 1e-12, 2, f) {
                Ok(early) => assert!(early.f_x.abs() <= 1e-12 || early.bracket.1 - early.bracket.0 <= 1e-9),
                Err(RootError::NotConverged { iterations, .. }) => assert_eq!(iterations, 2),
                Err(other) => panic!("case {case}: {other:?}"),
            }
        }
    }
}
