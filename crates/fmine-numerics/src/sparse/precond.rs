//! Provenance: frankensim crates/fs-sparse/src/precond.rs @ 20463525d9775a18c1b237d2676f364f367175ab; licence LicenseRef-MIT-OpenAI-Anthropic-Rider; Changes: Precond::apply returns Result and refuses vectors of the wrong length; ilu0 refuses a non-square matrix and a non-finite entry with IluError instead of asserting, and treats a non-finite pivot as a breakdown; the factored matrix reuses A's pattern instead of going through from_parts; pcg starts from a borrowed x0 and returns Result<PcgSolution, PcgError>, refusing wrong lengths, non-finite input, an invalid tolerance, a pᵀAp or rᵀz breakdown and non-convergence instead of handing back an unconverged iterate; tests come from tests/precond_battery.rs with a test_ prefix, plus residual known answers, an LDLt differential and refusal tests; Left out: lambda_max_estimate, Chebyshev, SaAmg with aggregate, smoothed_prolongator, scale_rows_by_dinv and add_scaled (they need ops::spgemm and ops::transpose), the cross-ISA golden hash
//! Determinism: DeterministicPerIsa (no cross-host digest test; plan §29.2, bd-hp2j.1).
//!
//! Preconditioned conjugate gradients and the zero-fill incomplete LU it is usually paired with.
//!
//! Everything exposes the OPERATOR interface ([`Precond::apply`]); nothing here requires assembling what can be
//! applied. Deterministic by construction: fixed iteration orders, sequential dot products, and no platform libm
//! anywhere in solver state.
//!
//! v1 scope note: ILU(0) is sequential (level scheduling is a recorded perf refinement; the upstream bead labels it
//! the bandwidth-hostile fallback, not the default).

use super::{Csr, SparseError};

/// The operator interface: z ← M⁻¹·r (approximately).
pub trait Precond {
    /// Apply the preconditioner to `r`, writing `z`. Refuses lengths that do not match the operator.
    fn apply(&self, r: &[f64], z: &mut [f64]) -> Result<(), SparseError>;
}

/// Identity (the "no preconditioner" baseline).
#[derive(Debug, Clone, Copy)]
pub struct IdentityPrecond;

impl Precond for IdentityPrecond {
    fn apply(&self, r: &[f64], z: &mut [f64]) -> Result<(), SparseError> {
        if z.len() != r.len() {
            return Err(SparseError::LengthMismatch {
                what: "identity z (r)",
                expected: r.len(),
                got: z.len(),
            });
        }
        z.copy_from_slice(r);
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// ILU(0)
// ---------------------------------------------------------------------------

/// Why an incomplete factorization refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IluError {
    /// ILU(0) needs a square matrix.
    NotSquare { nrows: usize, ncols: usize },
    /// A stored entry is NaN or infinite.
    NonFiniteEntry { row: usize, col: usize },
    /// The pivot of `row` is missing from the pattern, zero, or non-finite after elimination. Standard remedy:
    /// retry on A + σ·diag(A) with growing σ (caller policy, never applied here).
    Breakdown { row: usize },
}

impl core::fmt::Display for IluError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            IluError::NotSquare { nrows, ncols } => {
                write!(f, "ILU(0) needs a square matrix, not {nrows}x{ncols}")
            }
            IluError::NonFiniteEntry { row, col } => {
                write!(f, "entry ({row},{col}) is NaN or infinite")
            }
            IluError::Breakdown { row } => {
                write!(
                    f,
                    "ILU(0) breakdown: pivot of row {row} is missing, zero or non-finite"
                )
            }
        }
    }
}

impl std::error::Error for IluError {}

/// ILU(0): incomplete LU with zero fill on the CSR pattern. Sequential v1
/// (the bandwidth-hostile fallback per plan; level scheduling recorded).
#[derive(Debug, Clone)]
pub struct Ilu0 {
    n: usize,
    /// Factored values on A's pattern (L unit-lower strictly below diag,
    /// U on/above), plus A's structure.
    factored: Csr,
}

/// Factor A ≈ L·U on A's own pattern.
pub fn ilu0(a: &Csr) -> Result<Ilu0, IluError> {
    let n = a.nrows();
    if n != a.ncols() {
        return Err(IluError::NotSquare {
            nrows: n,
            ncols: a.ncols(),
        });
    }
    // Work on a mutable copy of the CSR values with the same pattern.
    let mut vals: Vec<Vec<f64>> = Vec::with_capacity(n);
    let mut cols: Vec<Vec<usize>> = Vec::with_capacity(n);
    for i in 0..n {
        let (c, v) = a.row_at(i);
        if let Some(k) = v.iter().position(|x| !x.is_finite()) {
            return Err(IluError::NonFiniteEntry { row: i, col: c[k] });
        }
        cols.push(c.to_vec());
        vals.push(v.to_vec());
    }
    for i in 0..n {
        // For each k < i in row i's pattern (ascending): eliminate.
        for kk in 0..cols[i].len() {
            let k = cols[i][kk];
            if k >= i {
                break;
            }
            // pivot U[k][k], checked when row k finished below
            let (ck, vk) = (&cols[k], &vals[k]);
            let Some(dk_pos) = ck.iter().position(|&c| c == k) else {
                return Err(IluError::Breakdown { row: k });
            };
            let piv = vk[dk_pos];
            let lik = vals[i][kk] / piv;
            vals[i][kk] = lik;
            // Row update restricted to row i's pattern: a_ij -= lik * u_kj.
            // Merge over row k's entries j > k that also exist in row i.
            let (ck_c, vk_c): (Vec<usize>, Vec<f64>) = (cols[k].clone(), vals[k].clone());
            for (pos_k, &j) in ck_c.iter().enumerate() {
                if j <= k {
                    continue;
                }
                if let Ok(pos_i) = cols[i].binary_search(&j) {
                    vals[i][pos_i] = (-lik).mul_add(vk_c[pos_k], vals[i][pos_i]);
                }
            }
        }
        // Diagonal must exist, be nonzero and be finite for the NEXT eliminations.
        match cols[i].binary_search(&i) {
            Ok(p) if vals[i][p] != 0.0 && vals[i][p].is_finite() => {}
            _ => return Err(IluError::Breakdown { row: i }),
        }
    }
    // Same pattern, factored values.
    let mut factored = a.clone();
    factored.vals = vals.concat();
    Ok(Ilu0 { n, factored })
}

impl Precond for Ilu0 {
    /// z = U⁻¹·L⁻¹·r (forward then backward substitution on the pattern).
    fn apply(&self, r: &[f64], z: &mut [f64]) -> Result<(), SparseError> {
        let n = self.n;
        for (what, got) in [("ilu0 r (n)", r.len()), ("ilu0 z (n)", z.len())] {
            if got != n {
                return Err(SparseError::LengthMismatch {
                    what,
                    expected: n,
                    got,
                });
            }
        }
        z.copy_from_slice(r);
        // Forward: L is unit-lower on the strict-lower pattern.
        for i in 0..n {
            let (cols, vals) = self.factored.row_at(i);
            let mut v = z[i];
            for (&c, &lv) in cols.iter().zip(vals) {
                if c >= i {
                    break;
                }
                v = (-lv).mul_add(z[c], v);
            }
            z[i] = v;
        }
        // Backward with U (diag included; ilu0 proved it stored and nonzero).
        for i in (0..n).rev() {
            let (cols, vals) = self.factored.row_at(i);
            let mut v = z[i];
            let mut diag = 1.0;
            for (&c, &uv) in cols.iter().zip(vals) {
                if c < i {
                    continue;
                }
                if c == i {
                    diag = uv;
                } else {
                    v = (-uv).mul_add(z[c], v);
                }
            }
            z[i] = v / diag;
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// PCG reference driver
// ---------------------------------------------------------------------------

/// A converged PCG solve: the evidence object.
#[derive(Debug, Clone, PartialEq)]
pub struct PcgSolution {
    /// The iterate that met the tolerance.
    pub x: Vec<f64>,
    /// Iterations performed.
    pub iters: usize,
    /// Final recursive relative residual ‖r‖₂/‖b‖₂ (≤ the tolerance).
    pub rel_residual: f64,
}

/// Why PCG refused. No variant carries an iterate: an unconverged or broken-down solve hands nothing back.
#[derive(Debug, Clone, PartialEq)]
pub enum PcgError {
    /// A length mismatch from the operator, the preconditioner, or the call itself.
    Sparse(SparseError),
    /// CG needs a square operator.
    NotSquare { nrows: usize, ncols: usize },
    /// `what` ("A", "b" or "x0") holds a NaN or infinity at `index` (a stored-entry index for "A").
    NonFinite { what: &'static str, index: usize },
    /// The tolerance is NaN, infinite or negative.
    InvalidTolerance { tol: f64 },
    /// A curvature or preconditioner inner product that must be positive was not, at iteration `iter`: pᵀAp
    /// means A is not SPD on the Krylov space, rᵀz means M is not SPD.
    Breakdown {
        iter: usize,
        quantity: &'static str,
        value: f64,
    },
    /// The tolerance was not met within `iters` iterations.
    NotConverged { iters: usize, rel_residual: f64 },
}

impl From<SparseError> for PcgError {
    fn from(e: SparseError) -> PcgError {
        PcgError::Sparse(e)
    }
}

impl core::fmt::Display for PcgError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            PcgError::Sparse(e) => write!(f, "pcg: {e}"),
            PcgError::NotSquare { nrows, ncols } => {
                write!(f, "pcg needs a square operator, not {nrows}x{ncols}")
            }
            PcgError::NonFinite { what, index } => {
                write!(f, "pcg: {what}[{index}] is NaN or infinite")
            }
            PcgError::InvalidTolerance { tol } => {
                write!(f, "pcg tolerance {tol} must be finite and non-negative")
            }
            PcgError::Breakdown {
                iter,
                quantity,
                value,
            } => write!(
                f,
                "pcg breakdown at iteration {iter}: {quantity} = {value:e} is not positive"
            ),
            PcgError::NotConverged {
                iters,
                rel_residual,
            } => write!(
                f,
                "pcg did not converge in {iters} iterations (relative residual {rel_residual:e})"
            ),
        }
    }
}

impl std::error::Error for PcgError {}

fn dot(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

/// Preconditioned conjugate gradients from `x0` (reference driver — kept minimal and deterministic). With
/// `b = 0` the relative residual is measured against `f64::MIN_POSITIVE`, so only an exact zero residual
/// converges.
pub fn pcg<P: Precond>(
    a: &Csr,
    b: &[f64],
    x0: &[f64],
    m: &P,
    tol: f64,
    max_iters: usize,
) -> Result<PcgSolution, PcgError> {
    let n = a.nrows();
    if a.ncols() != n {
        return Err(PcgError::NotSquare {
            nrows: n,
            ncols: a.ncols(),
        });
    }
    for (what, got) in [("pcg b (nrows)", b.len()), ("pcg x0 (ncols)", x0.len())] {
        if got != n {
            return Err(SparseError::LengthMismatch {
                what,
                expected: n,
                got,
            }
            .into());
        }
    }
    for (what, v) in [("A", a.vals.as_slice()), ("b", b), ("x0", x0)] {
        if let Some(index) = v.iter().position(|x| !x.is_finite()) {
            return Err(PcgError::NonFinite { what, index });
        }
    }
    if !tol.is_finite() || tol < 0.0 {
        return Err(PcgError::InvalidTolerance { tol });
    }
    let bnorm = dot(b, b).sqrt().max(f64::MIN_POSITIVE);
    let mut x = x0.to_vec();
    let mut r = vec![0.0f64; n];
    a.spmv(&x, &mut r)?;
    for i in 0..n {
        r[i] = b[i] - r[i];
    }
    let mut z = vec![0.0f64; n];
    m.apply(&r, &mut z)?;
    let mut p = z.clone();
    let mut rz = dot(&r, &z);
    let mut ap = vec![0.0f64; n];
    for it in 0..max_iters {
        let rel = dot(&r, &r).sqrt() / bnorm;
        if rel <= tol {
            return Ok(PcgSolution {
                x,
                iters: it,
                rel_residual: rel,
            });
        }
        if !(rz > 0.0 && rz.is_finite()) {
            return Err(PcgError::Breakdown {
                iter: it,
                quantity: "rᵀz",
                value: rz,
            });
        }
        a.spmv(&p, &mut ap)?;
        let pap = dot(&p, &ap);
        if !(pap > 0.0 && pap.is_finite()) {
            return Err(PcgError::Breakdown {
                iter: it,
                quantity: "pᵀAp",
                value: pap,
            });
        }
        let alpha = rz / pap;
        for i in 0..n {
            x[i] = alpha.mul_add(p[i], x[i]);
            r[i] = (-alpha).mul_add(ap[i], r[i]);
        }
        m.apply(&r, &mut z)?;
        let rz_new = dot(&r, &z);
        let beta = rz_new / rz;
        rz = rz_new;
        for i in 0..n {
            p[i] = beta.mul_add(p[i], z[i]);
        }
    }
    let rel = dot(&r, &r).sqrt() / bnorm;
    if rel <= tol {
        return Ok(PcgSolution {
            x,
            iters: max_iters,
            rel_residual: rel,
        });
    }
    Err(PcgError::NotConverged {
        iters: max_iters,
        rel_residual: rel,
    })
}

#[cfg(test)]
mod tests {
    use super::super::direct::{DirectOrdering, LdltOptions, SymbolicLdlt};
    use super::super::tests::{csr, laplacian_2d, lcg};
    use super::*;

    fn tridiag(n: usize) -> Csr {
        let mut e = Vec::new();
        for i in 0..n {
            e.push((i, i, 2.0));
            if i > 0 {
                e.push((i, i - 1, -1.0));
            }
            if i + 1 < n {
                e.push((i, i + 1, -1.0));
            }
        }
        csr(n, n, &e)
    }

    fn rhs(n: usize) -> Vec<f64> {
        // Deterministic, libm-free right-hand side.
        (0..n)
            .map(|i| 1.0 + (((i * 40_503) % 101) as f64) / 101.0 - 0.5)
            .collect()
    }

    fn norm2(v: &[f64]) -> f64 {
        dot(v, v).sqrt()
    }

    /// ‖b − A·x‖₂/‖b‖₂, recomputed from scratch (the recursive residual can drift from it).
    fn true_rel_residual(a: &Csr, x: &[f64], b: &[f64]) -> f64 {
        let mut ax = vec![0.0; b.len()];
        a.spmv(x, &mut ax).expect("lengths match");
        let r: Vec<f64> = b.iter().zip(&ax).map(|(p, q)| p - q).collect();
        norm2(&r) / norm2(b)
    }

    fn err2(x: &[f64], want: &[f64]) -> f64 {
        let d: Vec<f64> = x.iter().zip(want).map(|(p, q)| p - q).collect();
        norm2(&d)
    }

    /// Negates its input: an indefinite "preconditioner" that must trip the rᵀz check.
    struct NegatePrecond;

    impl Precond for NegatePrecond {
        fn apply(&self, r: &[f64], z: &mut [f64]) -> Result<(), SparseError> {
            for (zi, ri) in z.iter_mut().zip(r) {
                *zi = -ri;
            }
            Ok(())
        }
    }

    #[test]
    fn test_pcg_residual_known_answers() {
        // Closed forms, not outputs of this code: tridiag(-1,2,-1)·x = 1 has x_i = i(n+1−i)/2. Error bounds are a
        // priori: ‖x − x*‖₂ ≤ κ·(‖b − Ax‖₂/‖b‖₂)·‖x*‖₂, with κ bounded from λ ∈ [4sin²(π/(2(n+1))), 4] and
        // sin t ≥ 2t/π, so κ(T_50) ≤ 51² = 2601 and κ(L_16) ≤ 17² = 289.
        let n = 50;
        let t = tridiag(n);
        let ones = vec![1.0; n];
        let parabola: Vec<f64> = (1..=n).map(|i| (i * (n + 1 - i)) as f64 / 2.0).collect();
        let tol = 1e-12;
        let plain = pcg(&t, &ones, &vec![0.0; n], &IdentityPrecond, tol, 500).expect("plain CG");
        let rel = true_rel_residual(&t, &plain.x, &ones);
        assert!(
            rel <= 10.0 * tol,
            "true residual {rel} drifted from the recursive one"
        );
        assert!(err2(&plain.x, &parabola) <= 2601.0 * rel.max(tol) * norm2(&parabola));
        // ILU(0) of a tridiagonal matrix has no dropped fill, so it is the exact LU and PCG needs one step.
        let ilu = ilu0(&t).expect("tridiagonal factors");
        let one = pcg(&t, &ones, &vec![0.0; n], &ilu, 1e-10, 5).expect("ILU-PCG");
        assert_eq!(
            one.iters, 1,
            "exact preconditioner must converge in one iteration"
        );
        assert!(err2(&one.x, &parabola) <= 2601.0 * 1e-10 * norm2(&parabola));
        // Manufactured solution on the 16×16 grid: integer x, so b = A·x is exact.
        let a = laplacian_2d(16);
        let x_true: Vec<f64> = (0..a.nrows()).map(|u| (u % 7) as f64 - 3.0).collect();
        let mut b = vec![0.0; a.nrows()];
        a.spmv(&x_true, &mut b).expect("lengths match");
        let x0 = vec![0.0; a.nrows()];
        let cg = pcg(&a, &b, &x0, &IdentityPrecond, 1e-10, 1000).expect("CG");
        let ilu = ilu0(&a).expect("grid Laplacian factors");
        let pre = pcg(&a, &b, &x0, &ilu, 1e-10, 1000).expect("ILU-PCG");
        for s in [&cg, &pre] {
            let rel = true_rel_residual(&a, &s.x, &b);
            assert!(err2(&s.x, &x_true) <= 289.0 * rel.max(1e-10) * norm2(&x_true));
        }
        assert!(
            pre.iters < cg.iters,
            "ILU(0) {} vs plain {}",
            pre.iters,
            cg.iters
        );
        println!(
            "{{\"suite\":\"fmine-numerics-sparse-precond\",\"case\":\"residual-known-answers\",\"cg_iters_t50\":{},\"cg_iters_grid\":{},\"ilu_iters_grid\":{},\"verdict\":\"pass\"}}",
            plain.iters, cg.iters, pre.iters
        );
    }

    #[test]
    fn test_ilu_accelerates_cg() {
        let g = 32;
        let a = laplacian_2d(g);
        let n = g * g;
        let b = rhs(n);
        let tol = 1e-10;
        let x0 = vec![0.0; n];
        let plain = pcg(&a, &b, &x0, &IdentityPrecond, tol, 2000).expect("plain CG must converge");
        let ilu = ilu0(&a).expect("Laplacian factors without breakdown");
        let i = pcg(&a, &b, &x0, &ilu, tol, 2000).expect("ILU-PCG must converge");
        assert!(
            i.iters * 2 < plain.iters,
            "ILU(0) must at least halve iterations: {} vs {}",
            i.iters,
            plain.iters
        );
        for k in 0..n {
            assert!(
                (i.x[k] - plain.x[k]).abs() < 1e-7,
                "ilu solution drift at {k}"
            );
        }
        println!(
            "{{\"suite\":\"fmine-numerics-sparse-precond\",\"case\":\"precond-accel\",\"verdict\":\"pass\",\"detail\":\"plain {} vs ilu {} iters\"}}",
            plain.iters, i.iters
        );
    }

    #[test]
    fn test_pcg_agrees_with_ldlt() {
        // Differential against the direct solver, which shares no code with this module beyond Csr.
        let n = 60;
        let mut s = 0xC0FFEEu64;
        let mut e = Vec::new();
        for i in 0..n {
            e.push((i, i, 8.0 + lcg(&mut s)));
            for _ in 0..3 {
                let j = ((lcg(&mut s) + 0.5) * n as f64) as usize % n;
                if j != i {
                    let v = lcg(&mut s);
                    e.push((i, j, v));
                    e.push((j, i, v));
                }
            }
        }
        let a = csr(n, n, &e);
        let b: Vec<f64> = (0..n).map(|_| lcg(&mut s)).collect();
        let sym = SymbolicLdlt::analyze(&a, DirectOrdering::Amd).expect("analyze");
        let want = sym
            .factor(&a, &LdltOptions::default())
            .expect("factor")
            .solve(&b)
            .expect("solve");
        let ilu = ilu0(&a).expect("diagonally dominant factors");
        let got = pcg(&a, &b, &vec![0.0; n], &ilu, 1e-13, 200).expect("ILU-PCG");
        assert!(err2(&got.x, &want) <= 1e-10 * norm2(&want), "pcg vs ldlt");
    }

    #[test]
    fn test_ilu_breakdown_is_typed() {
        // Missing diagonal in row 1 → structured breakdown at that row.
        let a = csr(2, 2, &[(0, 0, 1.0), (0, 1, 1.0), (1, 0, 1.0)]);
        assert_eq!(ilu0(&a).map(|_| ()), Err(IluError::Breakdown { row: 1 }));
        // Stored zero pivot in row 0.
        let z = csr(2, 2, &[(0, 0, 0.0), (0, 1, 1.0), (1, 0, 1.0), (1, 1, 1.0)]);
        assert_eq!(ilu0(&z).map(|_| ()), Err(IluError::Breakdown { row: 0 }));
        // Elimination cancels row 1's pivot exactly: 1 − 1·1 = 0.
        let c = csr(2, 2, &[(0, 0, 1.0), (0, 1, 1.0), (1, 0, 1.0), (1, 1, 1.0)]);
        assert_eq!(ilu0(&c).map(|_| ()), Err(IluError::Breakdown { row: 1 }));
        let rect = csr(2, 3, &[(0, 0, 1.0)]);
        assert_eq!(
            ilu0(&rect).map(|_| ()),
            Err(IluError::NotSquare { nrows: 2, ncols: 3 })
        );
        let nan = csr(2, 2, &[(0, 0, 1.0), (1, 0, f64::NAN), (1, 1, 1.0)]);
        assert_eq!(
            ilu0(&nan).map(|_| ()),
            Err(IluError::NonFiniteEntry { row: 1, col: 0 })
        );
    }

    #[test]
    fn test_pcg_refusals_carry_no_iterate() {
        // diag(1, −1) with b = (0, 1): r = p = e₂, pᵀAp = −1 at the first step.
        let indef = csr(2, 2, &[(0, 0, 1.0), (1, 1, -1.0)]);
        assert_eq!(
            pcg(
                &indef,
                &[0.0, 1.0],
                &[0.0, 0.0],
                &IdentityPrecond,
                1e-10,
                10
            ),
            Err(PcgError::Breakdown {
                iter: 0,
                quantity: "pᵀAp",
                value: -1.0
            })
        );
        let t = tridiag(50);
        let ones = vec![1.0; 50];
        let zeros = vec![0.0; 50];
        assert_eq!(
            pcg(&t, &ones, &zeros, &NegatePrecond, 1e-10, 10),
            Err(PcgError::Breakdown {
                iter: 0,
                quantity: "rᵀz",
                value: -50.0
            })
        );
        assert!(matches!(
            pcg(&t, &ones, &zeros, &IdentityPrecond, 1e-10, 2),
            Err(PcgError::NotConverged { iters: 2, .. })
        ));
        assert_eq!(
            pcg(&t, &ones[..49], &zeros, &IdentityPrecond, 1e-10, 10),
            Err(PcgError::Sparse(SparseError::LengthMismatch {
                what: "pcg b (nrows)",
                expected: 50,
                got: 49
            }))
        );
        assert!(matches!(
            pcg(&t, &ones, &zeros[..3], &IdentityPrecond, 1e-10, 10),
            Err(PcgError::Sparse(SparseError::LengthMismatch {
                what: "pcg x0 (ncols)",
                ..
            }))
        ));
        let mut bad = ones.clone();
        bad[7] = f64::NAN;
        assert_eq!(
            pcg(&t, &bad, &zeros, &IdentityPrecond, 1e-10, 10),
            Err(PcgError::NonFinite {
                what: "b",
                index: 7
            })
        );
        assert_eq!(
            pcg(&t, &ones, &bad, &IdentityPrecond, 1e-10, 10),
            Err(PcgError::NonFinite {
                what: "x0",
                index: 7
            })
        );
        let inf_a = csr(2, 2, &[(0, 0, 1.0), (1, 1, f64::INFINITY)]);
        assert_eq!(
            pcg(
                &inf_a,
                &[1.0, 1.0],
                &[0.0, 0.0],
                &IdentityPrecond,
                1e-10,
                10
            ),
            Err(PcgError::NonFinite {
                what: "A",
                index: 1
            })
        );
        for tol in [f64::NAN, -1e-3, f64::INFINITY] {
            assert!(matches!(
                pcg(&t, &ones, &zeros, &IdentityPrecond, tol, 10),
                Err(PcgError::InvalidTolerance { .. })
            ));
        }
        let rect = csr(2, 3, &[(0, 0, 1.0)]);
        assert_eq!(
            pcg(
                &rect,
                &[1.0, 1.0],
                &[0.0, 0.0, 0.0],
                &IdentityPrecond,
                1e-10,
                10
            ),
            Err(PcgError::NotSquare { nrows: 2, ncols: 3 })
        );
    }

    #[test]
    fn test_zero_rhs_converges_at_once_and_exact_start_needs_no_step() {
        let t = tridiag(10);
        let sol = pcg(&t, &[0.0; 10], &[0.0; 10], &IdentityPrecond, 0.0, 10).expect("zero system");
        assert_eq!((sol.iters, sol.x), (0, vec![0.0; 10]));
        // Starting at the exact solution: integer data, exact residual zero.
        let x: Vec<f64> = (0..10).map(|i| i as f64).collect();
        let mut b = vec![0.0; 10];
        t.spmv(&x, &mut b).expect("lengths match");
        let sol = pcg(&t, &b, &x, &IdentityPrecond, 0.0, 10).expect("already solved");
        assert_eq!((sol.iters, sol.rel_residual), (0, 0.0));
    }

    #[test]
    fn test_precond_apply_refuses_wrong_lengths() {
        let mut z = vec![9.0; 3];
        assert!(IdentityPrecond.apply(&[1.0, 2.0], &mut z).is_err());
        assert_eq!(z, vec![9.0; 3], "a refused apply writes nothing");
        let ilu = ilu0(&tridiag(4)).expect("factors");
        assert_eq!(
            ilu.apply(&[1.0; 3], &mut z),
            Err(SparseError::LengthMismatch {
                what: "ilu0 r (n)",
                expected: 4,
                got: 3
            })
        );
        let mut z4 = vec![9.0; 4];
        assert!(ilu.apply(&[1.0; 4], &mut z4[..2]).is_err());
        assert!(ilu.apply(&[1.0; 4], &mut z4).is_ok());
    }

    #[test]
    fn test_setup_and_solve_are_deterministic() {
        let g = 24;
        let a = laplacian_2d(g);
        let n = g * g;
        let run = || {
            let ilu = ilu0(&a).expect("factors");
            pcg(&a, &rhs(n), &vec![0.0; n], &ilu, 1e-10, 500).expect("converges")
        };
        let (s1, s2) = (run(), run());
        assert_eq!(
            s1.iters, s2.iters,
            "iteration count must be rerun-identical"
        );
        assert!(
            s1.x.iter()
                .zip(&s2.x)
                .all(|(p, q)| p.to_bits() == q.to_bits()),
            "solutions must be BITWISE identical"
        );
    }
}
