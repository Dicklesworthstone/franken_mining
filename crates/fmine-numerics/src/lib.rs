//! Deterministic numerical kernels (`WP-02`, `BD-022`):
//! Determinism: DeterministicPerIsa (`norm_cdf` and `norm_inv` delegate to `fmine_core::det`; `brent_root` and
//! `cholesky_solve` use only `+ − × ÷` and `sqrt`. BitwisePortable once one CI run passes its digests, bd-hp2j.3).
//! - Dense SPD Cholesky solver (`cholesky_solve`, `FMINE-REF-207`)
//! - Brent's bracketed root-finder (`brent_root`), which refuses on a missing sign change, a non-finite value or
//!   non-convergence and never returns an unconverged iterate (bd-t1dk)
//! - Standard normal CDF (`norm_cdf`) and Acklam inverse quantile (`norm_inv`)
//! - Lifted `frankensim` modules: `eproc`, `voi`, `risk`, `tropical`.
//! - `exact_feas`: exact rounding-interval feasibility with checked witnesses and Farkas certificates (bd-4wvl.1).

pub mod eproc;
pub mod exact_feas;
pub mod risk;
pub mod tropical;
pub mod voi;

use fmine_core::hash::blake3_hash;
use fmine_core::rand::{PhiloxStream, RngKey};
use fmine_evidence::{Refusal, RefusalCode};

/// Solves $A x = b$ for an $n \times n$ row-major symmetric positive-definite matrix `a`
/// via Cholesky factorization $A = L L^\top$.
///
/// Refuses with `FMINE-REF-207` (numerical certificate failed; §29.6 routes failed linear solves to 2xx) if
/// the dimensions do not describe an $n \times n$ system or `a` is not positive-definite (`BD-022`).
pub fn cholesky_solve(a: &[f64], b: &[f64], n: usize) -> Result<Vec<f64>, Refusal> {
    if n == 0 || a.len() != n * n || b.len() != n {
        return Err(Refusal::new(
            RefusalCode::NumericalCertificateFailed207,
            format!(
                "invalid matrix dimensions n={n}, a.len()={}, b.len()={}",
                a.len(),
                b.len()
            ),
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
                    return Err(Refusal::new(
                        RefusalCode::NumericalCertificateFailed207,
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
    NotConverged {
        iterations: usize,
        bracket_width: f64,
        f_b: f64,
    },
}

impl std::fmt::Display for RootError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoSignChange { fa, fb } => {
                write!(f, "no sign change on the bracket: f(a) {fa}, f(b) {fb}")
            }
            Self::NonFinite { at } => write!(f, "f is not finite at {at}"),
            Self::NotConverged {
                iterations,
                bracket_width,
                f_b,
            } => write!(
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
pub fn brent_root<F>(
    mut a: f64,
    mut b: f64,
    tol: f64,
    max_iter: usize,
    mut f: F,
) -> Result<Root, RootError>
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
    Err(RootError::NotConverged {
        iterations: max_iter,
        bracket_width: (c - b).abs(),
        f_b: fb,
    })
}

/// Standard normal cumulative distribution function $\Phi(z)$: [`fmine_core::det::norm_cdf`] (Abramowitz &
/// Stegun 7.1.26 on `erf`, absolute error below $1.5 \times 10^{-7}$ in `erf`), so no platform libm is called.
#[must_use]
pub fn norm_cdf(z: f64) -> f64 {
    fmine_core::det::norm_cdf(z)
}

/// Standard normal inverse CDF $\Phi^{-1}(p)$: [`fmine_core::det::norm_ppf`] (Acklam's rational approximation).
/// Nothing is clamped: `p` outside $[0, 1]$ or NaN gives NaN, and $p = 0$ and $p = 1$ give $-\infty$ and
/// $+\infty$. Callers must refuse those values rather than use them.
#[must_use]
pub fn norm_inv(p: f64) -> f64 {
    fmine_core::det::norm_ppf(p)
}

// --- Golden digests (bd-hp2j.3): the same bits on every host, or the kernel is not BitwisePortable ---

/// The Philox key of this file's golden digests.
pub const NUMERICS_DIGEST_KEY: RngKey = RngKey {
    seed_root: 0x6e75_6d65,
    partition: 1,
    domain: 0,
    var_group: 0,
    realization: 0,
    brick: 0,
};

/// BLAKE3 over each value's little-endian bits.
fn digest_bits(bits: impl IntoIterator<Item = u64>) -> String {
    let bytes: Vec<u8> = bits.into_iter().flat_map(u64::to_le_bytes).collect();
    blake3_hash(&bytes).to_hex()
}

/// The 10,000 `(z, p)` inputs of the normal digest: `z` uniform on [−12, 12] (|z| > 8 is a third of them); `p`
/// alternately `e^{−700u}` (from about 1e-304 up to 1) and `1 − e^{−37u}` (1 − p from about 1e-16 up to 1).
#[must_use]
pub fn normal_digest_inputs(key: RngKey) -> Vec<(f64, f64)> {
    let mut rng = PhiloxStream::new(key);
    (0..10_000)
        .map(|i| {
            let z = 24.0 * rng.next_f64_open01() - 12.0;
            let u = rng.next_f64_open01();
            let p = if i % 2 == 0 {
                fmine_core::det::exp(-700.0 * u)
            } else {
                1.0 - fmine_core::det::exp(-37.0 * u)
            };
            (z, p)
        })
        .collect()
}

/// The digest of `norm_cdf(z)` and `norm_inv(p)` over [`normal_digest_inputs`].
#[must_use]
pub fn digest_norm_cdf_inv(key: RngKey) -> String {
    digest_bits(
        normal_digest_inputs(key)
            .into_iter()
            .flat_map(|(z, p)| [norm_cdf(z), norm_inv(p)].map(f64::to_bits)),
    )
}

/// [`digest_norm_cdf_inv`] under [`NUMERICS_DIGEST_KEY`].
#[must_use]
pub fn golden_digest_norm_cdf_inv_10k() -> String {
    digest_norm_cdf_inv(NUMERICS_DIGEST_KEY)
}

/// The digest of 10,000 seeded `cholesky_solve` systems (3 × 3, `A = M Mᵀ + I/2` with `M` and `b` uniform on
/// [−1, 1]; the solution's three values) and 10,000 `brent_root` problems (`x³ = c`, `c` uniform on [0.1, 10],
/// bracket [0, 3], tolerance 1e-12; the root). A refusal hashes as `u64::MAX`.
#[must_use]
pub fn digest_cholesky_brent(key: RngKey) -> String {
    let mut rng = PhiloxStream::new(key);
    let mut bits = Vec::with_capacity(40_000);
    let mut u = || 2.0 * rng.next_f64_open01() - 1.0;
    for _ in 0..10_000 {
        let m: Vec<f64> = (0..9).map(|_| u()).collect();
        let b: Vec<f64> = (0..3).map(|_| u()).collect();
        let mut a = vec![0.0; 9];
        for i in 0..3 {
            for j in 0..3 {
                let dot: f64 = (0..3).map(|k| m[3 * i + k] * m[3 * j + k]).sum();
                a[3 * i + j] = dot + if i == j { 0.5 } else { 0.0 };
            }
        }
        match cholesky_solve(&a, &b, 3) {
            Ok(x) => bits.extend(x.iter().map(|v| v.to_bits())),
            Err(_) => bits.push(u64::MAX),
        }
    }
    for _ in 0..10_000 {
        let c = 0.1 + 9.9 * (0.5 * u() + 0.5);
        let root = brent_root(0.0, 3.0, 1e-12, 200, |x| x * x * x - c);
        bits.push(root.map_or(u64::MAX, |r| r.x.to_bits()));
    }
    digest_bits(bits)
}

/// [`digest_cholesky_brent`] under [`NUMERICS_DIGEST_KEY`] (partition 2).
#[must_use]
pub fn golden_digest_cholesky_brent_10k() -> String {
    digest_cholesky_brent(RngKey {
        partition: 2,
        ..NUMERICS_DIGEST_KEY
    })
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
        assert_eq!((err.code, err.exit_code), ("FMINE-REF-207", 3));
    }

    #[test]
    fn test_cholesky_non_spd_refuses_207() {
        // Not positive definite (eigenvalues 3 and -1), and a dimension mismatch: both 207, exit 3, never 301
        // (the MNPI wall).
        let err = cholesky_solve(&[1.0, 2.0, 2.0, 1.0], &[1.0, 1.0], 2).expect_err("indefinite");
        assert_eq!((err.code, err.exit_code), ("FMINE-REF-207", 3));
        assert!(
            err.what.contains("not positive-definite at pivot 1"),
            "{}",
            err.what
        );
        let err = cholesky_solve(&[1.0, 0.0, 0.0], &[1.0, 1.0], 2).expect_err("dimensions");
        assert_eq!((err.code, err.exit_code), ("FMINE-REF-207", 3));
        assert!(
            err.what
                .contains("invalid matrix dimensions n=2, a.len()=3"),
            "{}",
            err.what
        );
    }

    /// The bd-hp2j.3 log line: kernel, input count, the folded Philox key and counter prefix, digest, host.
    fn digest_line(kernel: &str, key: RngKey, digest: &str) -> String {
        let (k, c) = key.to_philox_key_and_hi_ctr();
        format!(
            "{{\"kernel\":\"{kernel}\",\"n\":10000,\"rng_key\":\"{:08x}{:08x}{:08x}{:08x}\",\"digest\":\"{digest}\",\
             \"target\":\"{}-{}\"}}",
            k[0],
            k[1],
            c[0],
            c[1],
            std::env::consts::ARCH,
            std::env::consts::OS
        )
    }

    // TARGETs, first observed on aarch64-macos (bd-hp2j.3). Never regenerated on another host: a difference
    // there is the failure this test exists to catch.
    const NORM_CDF_INV_10K: &str =
        "c597baa370caa8d34160bf165d9dc13aa6c6fee3c50d44541d6a01c2ab6a993a";
    const CHOLESKY_BRENT_10K: &str =
        "b7e9a3f30c520bfcb6cf55e7c8306348252ee2753ac53a5440cddf34c96c63f0";

    #[test]
    fn test_golden_digest_norm_cdf_inv_10k() {
        let d = golden_digest_norm_cdf_inv_10k();
        println!("{}", digest_line("norm_cdf_inv", NUMERICS_DIGEST_KEY, &d));
        assert_eq!(d, NORM_CDF_INV_10K);
    }

    #[test]
    fn test_golden_digest_cholesky_brent_10k() {
        let d = golden_digest_cholesky_brent_10k();
        let key = RngKey {
            partition: 2,
            ..NUMERICS_DIGEST_KEY
        };
        println!("{}", digest_line("cholesky_brent", key, &d));
        assert_eq!(d, CHOLESKY_BRENT_10K);
    }

    #[test]
    fn test_normal_digest_inputs_cover_tails_and_change_with_key() {
        let inputs = normal_digest_inputs(NUMERICS_DIGEST_KEY);
        assert!(inputs.iter().any(|&(z, _)| z > 8.0) && inputs.iter().any(|&(z, _)| z < -8.0));
        assert!(inputs.iter().any(|&(_, p)| p < 1e-200), "no p near 0");
        assert!(inputs.iter().any(|&(_, p)| 1.0 - p < 1e-12), "no p near 1");
        assert!(inputs.iter().all(|&(_, p)| p > 0.0 && p < 1.0));
        let other = RngKey {
            seed_root: NUMERICS_DIGEST_KEY.seed_root + 1,
            ..NUMERICS_DIGEST_KEY
        };
        assert_ne!(digest_norm_cdf_inv(other), golden_digest_norm_cdf_inv_10k());
        assert_ne!(
            digest_cholesky_brent(RngKey {
                partition: 2,
                ..other
            }),
            golden_digest_cholesky_brent_10k()
        );
    }

    /// bd-hp2j.2: the old body clamped `p` into [1e-15, 1 − 1e-15] and returned a finite quantile for any input.
    #[test]
    fn test_norm_inv_out_of_domain_is_nan_not_clamped() {
        assert!(norm_inv(-0.1).is_nan());
        assert!(norm_inv(1.1).is_nan());
        assert!(norm_inv(f64::NAN).is_nan());
        assert_eq!(norm_inv(0.0), f64::NEG_INFINITY);
        assert_eq!(norm_inv(1.0), f64::INFINITY);
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
        let err = brent_root(0.0, 2.0, 1e-15, 3, |x| x * x * x - 2.0)
            .expect_err("3 iterations are too few");
        assert!(
            matches!(err, RootError::NotConverged { iterations: 3, .. }),
            "{err:?}"
        );

        assert_eq!(
            brent_root(1.0, 2.0, 1e-12, 100, |x| x * x + 1.0),
            Err(RootError::NoSignChange { fa: 2.0, fb: 5.0 })
        );
        assert_eq!(
            brent_root(1.0, 2.0, 1e-12, 100, |_| f64::NAN),
            Err(RootError::NonFinite { at: 1.0 })
        );
        // Finite at both ends, NaN everywhere inside: the first interior iterate refuses.
        let err = brent_root(1.0, 2.0, 1e-12, 100, |x| {
            if x > 1.0 && x < 2.0 {
                f64::NAN
            } else {
                x - 1.5
            }
        })
        .expect_err("interior NaN");
        assert!(
            matches!(err, RootError::NonFinite { at } if at > 1.0 && at < 2.0),
            "{err:?}"
        );
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
            let root =
                brent_root(lo, hi, 1e-12, 200, f).unwrap_or_else(|e| panic!("case {case}: {e}"));
            assert!(
                (root.x - r).abs() <= 1e-9,
                "case {case}: x {} vs r {r}",
                root.x
            );
            // With two iterations the result is either a genuine convergence or a refusal, never a bare iterate.
            match brent_root(lo, hi, 1e-12, 2, f) {
                Ok(early) => {
                    assert!(early.f_x.abs() <= 1e-12 || early.bracket.1 - early.bracket.0 <= 1e-9)
                }
                Err(RootError::NotConverged { iterations, .. }) => assert_eq!(iterations, 2),
                Err(other) => panic!("case {case}: {other:?}"),
            }
        }
    }
}
