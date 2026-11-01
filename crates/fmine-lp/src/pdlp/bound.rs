//! A lower bound on the problem's minimum from any row multipliers, by weak duality, with every rounding directed
//! outward (PO-PUSH-4).
//! Determinism: BitwisePortable (IEEE `+ − ×`, `mul_add`, `next_up` and `next_down`; no transcendental call).
//!
//! For any `y` and any feasible `x`, with `r = c − Aᵀy` computed exactly:
//!
//! `c·x = Σᵢ yᵢ (Ax)ᵢ + Σⱼ rⱼ xⱼ ≥ Σᵢ min_{s ∈ [loᵢ, hiᵢ]} yᵢ s + Σⱼ min_{t ∈ [lⱼ, uⱼ]} rⱼ t`.
//!
//! - Dual repair: a multiplier whose sign would need an infinite row bound (`yᵢ > 0` with `loᵢ = −∞`, or `yᵢ < 0`
//!   with `hiᵢ = +∞`) is set to zero, which keeps the inequality and makes its row term finite.
//! - Outward rounding: each reduced cost is carried as an interval `[r⁻, r⁺]`, each term of the sum is rounded
//!   down, and so is each partial sum. Every `+` and `×` is done once in IEEE arithmetic and its exact rounding
//!   error found by an error-free transformation (TwoSum; the residual `mul_add(a, b, −a·b)`). When the error is
//!   zero the result is kept, and otherwise it is moved one step toward the side the bound needs. A product too
//!   small for its residual to be exact (below 2⁻⁹⁶⁰ in magnitude) is moved one step either way.
//! - A column whose infinite bound meets a reduced cost of the wrong sign makes the bound `−∞`, and no bound is
//!   returned. A free column therefore certifies only when its reduced cost is exactly zero.
//!
//! The bound is for the problem as stored in `f64`. How the caller rounded its own data into `f64` is not bounded
//! here.

use fmine_evidence::Refusal;

use super::{LpProblem, refuse};

/// Below this magnitude a product's `mul_add` residual may itself have underflowed.
const EFT_PRODUCT_MIN: f64 = 1.0e-289;

/// A certified lower bound on `min c·x`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DualBound {
    value: f64,
    repaired_rows: usize,
}

impl DualBound {
    /// No feasible `x` has `c·x` below this.
    #[must_use]
    pub fn value(&self) -> f64 {
        self.value
    }

    /// Multipliers set to zero because their sign needed an infinite row bound.
    #[must_use]
    pub fn repaired_rows(&self) -> usize {
        self.repaired_rows
    }
}

/// The largest double not above `a·b`.
fn mul_lo(a: f64, b: f64) -> f64 {
    if a == 0.0 || b == 0.0 {
        return 0.0;
    }
    let p = a * b;
    if p == f64::INFINITY {
        return f64::MAX;
    }
    if !p.is_finite() || p.abs() < EFT_PRODUCT_MIN {
        return p.next_down();
    }
    if a.mul_add(b, -p) < 0.0 {
        p.next_down()
    } else {
        p
    }
}

/// The smallest double not below `a·b`.
fn mul_hi(a: f64, b: f64) -> f64 {
    -mul_lo(-a, b)
}

/// The exact error of `s = a + b` (TwoSum), or `None` when an intermediate overflowed.
fn sum_error(a: f64, b: f64, s: f64) -> Option<f64> {
    let bb = s - a;
    let t = (a - (s - bb)) + (b - bb);
    t.is_finite().then_some(t)
}

/// The largest double not above `a + b`.
fn add_lo(a: f64, b: f64) -> f64 {
    let s = a + b;
    if s == f64::INFINITY {
        return f64::MAX;
    }
    if !s.is_finite() {
        return s;
    }
    match sum_error(a, b, s) {
        Some(t) if t >= 0.0 => s,
        _ => s.next_down(),
    }
}

/// The smallest double not below `a + b`.
fn add_hi(a: f64, b: f64) -> f64 {
    -add_lo(-a, -b)
}

/// The bound from row multipliers `y`, or `None` when an unbounded column meets a reduced cost of the wrong sign.
///
/// # Errors
/// `FMINE-REF-207` when `y` is not one finite value per row.
pub fn certified_bound(problem: &LpProblem, y: &[f64]) -> Result<Option<DualBound>, Refusal> {
    if y.len() != problem.rows() {
        return Err(refuse(format!(
            "{} multipliers for {} rows",
            y.len(),
            problem.rows()
        )));
    }
    if let Some(i) = y.iter().position(|v| !v.is_finite()) {
        return Err(refuse(format!("multiplier {i} is {}", y[i])));
    }
    let mut repaired_rows = 0;
    let mut ys = Vec::with_capacity(y.len());
    let mut total = 0.0f64;
    for ((&yi, &lo), &hi) in y.iter().zip(&problem.row_lo).zip(&problem.row_hi) {
        let side = if yi > 0.0 {
            lo
        } else if yi < 0.0 {
            hi
        } else {
            0.0
        };
        if side.is_finite() {
            ys.push(yi);
            total = add_lo(total, mul_lo(yi, side));
        } else {
            repaired_rows += 1;
            ys.push(0.0);
        }
    }
    for j in 0..problem.cols() {
        let (rows, vals) = problem.at.row(j);
        let (mut rlo, mut rhi) = (problem.c[j], problem.c[j]);
        for (&i, &a) in rows.iter().zip(vals) {
            let yi = ys[i as usize];
            rlo = add_lo(rlo, -mul_hi(a, yi));
            rhi = add_hi(rhi, -mul_lo(a, yi));
        }
        let (l, u) = (problem.col_lo[j], problem.col_hi[j]);
        if !(rlo.is_finite() && rhi.is_finite())
            || (l == f64::NEG_INFINITY && rhi > 0.0)
            || (u == f64::INFINITY && rlo < 0.0)
        {
            return Ok(None);
        }
        let mut term = f64::INFINITY;
        for end in [l, u] {
            if end.is_finite() {
                term = term.min(mul_lo(rlo, end)).min(mul_lo(rhi, end));
            }
        }
        if term == f64::INFINITY {
            // Free column: rlo ≥ 0 ≥ rhi above, so the reduced cost is exactly zero.
            term = 0.0;
        }
        total = add_lo(total, term);
    }
    if total.is_nan() || total == f64::NEG_INFINITY {
        return Ok(None);
    }
    Ok(Some(DualBound {
        value: total,
        repaired_rows,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Exact cases stay exact; inexact ones step outward and enclose the exact value.
    #[test]
    fn directed_operations_enclose_the_exact_result() {
        assert_eq!(mul_lo(3.0, 0.5), 1.5);
        assert_eq!(mul_hi(3.0, 0.5), 1.5);
        assert_eq!(add_lo(1.0, 2.0), 3.0);
        assert_eq!(add_hi(1.0, -1.0), 0.0);
        // 0.1 · 3 is inexact: the two directions are adjacent doubles around it.
        let (lo, hi) = (mul_lo(0.1, 3.0), mul_hi(0.1, 3.0));
        assert_eq!(lo.next_up(), hi);
        // 1 + 2⁻⁶⁰ rounds to 1; the exact sum lies above it.
        let tiny = 2f64.powi(-60);
        assert_eq!(add_lo(1.0, tiny), 1.0);
        assert_eq!(add_hi(1.0, tiny), 1.0f64.next_up());
        assert_eq!(add_lo(1.0, -tiny), 1.0f64.next_down());
        assert_eq!(add_lo(f64::MAX, f64::MAX), f64::MAX);
        assert_eq!(mul_lo(1e-200, 1e-200), (0.0f64).next_down());
        assert_eq!(mul_hi(1e-200, 1e-200), (0.0f64).next_up());
    }
}
