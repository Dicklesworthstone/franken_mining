//! A lower bound on the problem's minimum from any row multipliers, by weak duality, with every rounding directed
//! outward (PO-PUSH-4).
//! Determinism: BitwisePortable (IEEE `+ − × ÷`, `mul_add`, `next_up` and `next_down`, in a fixed order; no
//! transcendental call).
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
//! - Implied bounds (bd-tcp4): where a column's infinite bound meets a reduced cost of the wrong sign, its term
//!   uses instead a finite bound that the rows imply for every feasible `x`. Row `i` gives
//!   `aᵢⱼxⱼ ≤ hiᵢ − Σₖ≠ⱼ min aᵢₖxₖ` and `aᵢⱼxⱼ ≥ loᵢ − Σₖ≠ⱼ max aᵢₖxₖ`, the sums taken over the column bounds.
//!   A bound once found feeds later rows, for up to [`IMPLIED_ROUNDS`] passes. Each sum, product and quotient is
//!   rounded outward as above; a quotient's exact remainder `mul_add(q, b, −a)` decides its step. The bound holds
//!   at every feasible `x`, so the term stays a valid minimum. Only a column open on the side its reduced cost
//!   points to is touched, so a boxed problem's bound does not change by a bit.
//! - Where the rows imply no finite bound either, the term is `−∞` and no bound is returned. A free column with no
//!   implied bound therefore certifies only when its reduced cost is exactly zero.
//! - [`shift_for_open_columns`] moves multipliers so that such a column's reduced cost gets the right sign, by a
//!   margin. It is a heuristic for the solver to try: whatever it returns is certified like any other `y`.
//!
//! The bound is for the problem as stored in `f64`. How the caller rounded its own data into `f64` is not bounded
//! here.

use fmine_evidence::Refusal;

use super::{LpProblem, refuse};

/// Below this magnitude a product's `mul_add` residual may itself have underflowed.
const EFT_PRODUCT_MIN: f64 = 1.0e-289;

/// Passes over the rows when deriving implied column bounds. A pass that finds no new finite bound ends it sooner.
const IMPLIED_ROUNDS: usize = 16;

/// The reduced cost [`shift_for_open_columns`] aims for, relative to `1 + |cⱼ| + Σᵢ |aᵢⱼ yᵢ|`: far above the
/// rounding of that sum, far below any reduced cost that matters.
const SHIFT_MARGIN: f64 = 1e-11;

/// A certified lower bound on `min c·x`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DualBound {
    value: f64,
    repaired_rows: usize,
    implied_columns: usize,
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

    /// Columns whose infinite bound met a reduced cost of the wrong sign, and whose term used a bound the rows
    /// imply instead.
    #[must_use]
    pub fn implied_columns(&self) -> usize {
        self.implied_columns
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

/// The smallest double not below `a / b`, for `b > 0`; `±∞` or NaN when the quotient is not finite.
fn div_hi(a: f64, b: f64) -> f64 {
    let q = a / b;
    if !q.is_finite() || a == 0.0 {
        return q;
    }
    if q.abs() < EFT_PRODUCT_MIN || a.abs() < EFT_PRODUCT_MIN {
        return q.next_up();
    }
    // The remainder `q·b − a` of a correctly rounded quotient is itself a double, so `mul_add` gives it exactly.
    if q.mul_add(b, -a) < 0.0 {
        q.next_up()
    } else {
        q
    }
}

/// The largest double not above `a / b`, for `b > 0`.
fn div_lo(a: f64, b: f64) -> f64 {
    -div_hi(-a, b)
}

/// Which terms of a row's activity sum were left out because their column bound is infinite.
#[derive(Clone, Copy)]
enum Open {
    None,
    One(usize),
    Many,
}

impl Open {
    fn with(self, j: usize) -> Open {
        match self {
            Open::None => Open::One(j),
            _ => Open::Many,
        }
    }

    /// Of a sum `total` (rounded toward `down`'s side) holding `own`'s term unless `own` is the left-out one, the
    /// sum of the other terms, rounded the same way; `None` when another term is left out.
    fn others(self, total: f64, j: usize, own: f64, down: bool) -> Option<f64> {
        match self {
            Open::None if down => Some(add_lo(total, -own)),
            Open::None => Some(add_hi(total, -own)),
            Open::One(k) if k == j => Some(total),
            _ => None,
        }
    }
}

/// The column bounds, with each infinite one replaced where the rows allow by a finite bound that every feasible
/// `x` meets (module notes). Finite bounds are kept as they are; one found here may be tightened by a later row.
fn implied_bounds(problem: &LpProblem) -> (Vec<f64>, Vec<f64>) {
    let mut lo = problem.col_lo.clone();
    let mut hi = problem.col_hi.clone();
    let open: Vec<(bool, bool)> = lo
        .iter()
        .zip(&hi)
        .map(|(l, u)| (l.is_infinite(), u.is_infinite()))
        .collect();
    if !open.iter().any(|&(l, u)| l || u) {
        return (lo, hi);
    }
    for _ in 0..IMPLIED_ROUNDS {
        let mut found = false;
        for r in 0..problem.rows() {
            let (row_lo, row_hi) = (problem.row_lo[r], problem.row_hi[r]);
            let (cols, vals) = problem.a.row(r);
            // The row's least and greatest activity over the column bounds, rounded outward, its infinite terms
            // left out and noted.
            let (mut least, mut most) = (0.0f64, 0.0f64);
            let (mut least_open, mut most_open) = (Open::None, Open::None);
            let ends = |j: usize, a: f64, lo: &[f64], hi: &[f64]| {
                if a > 0.0 {
                    (lo[j], hi[j])
                } else {
                    (hi[j], lo[j])
                }
            };
            for (&j, &a) in cols.iter().zip(vals) {
                let j = j as usize;
                if a == 0.0 {
                    continue;
                }
                let (at_least, at_most) = ends(j, a, &lo, &hi);
                if at_least.is_finite() {
                    least = add_lo(least, mul_lo(a, at_least));
                } else {
                    least_open = least_open.with(j);
                }
                if at_most.is_finite() {
                    most = add_hi(most, mul_hi(a, at_most));
                } else {
                    most_open = most_open.with(j);
                }
            }
            for (&j, &a) in cols.iter().zip(vals) {
                let j = j as usize;
                let (open_lo, open_hi) = open[j];
                if a == 0.0 || !(open_lo || open_hi) {
                    continue;
                }
                let (at_least, at_most) = ends(j, a, &lo, &hi);
                // The other columns' least activity, rounded down, and greatest, rounded up. This column's own term
                // is finite whenever it is in the sum; it is taken out rounded the safe way.
                let others_least = least_open.others(least, j, mul_hi(a, at_least), true);
                let others_most = most_open.others(most, j, mul_lo(a, at_most), false);
                let (up, down) = if a > 0.0 {
                    (
                        others_least
                            .filter(|_| row_hi.is_finite())
                            .map(|s| div_hi(add_hi(row_hi, -s), a)),
                        others_most
                            .filter(|_| row_lo.is_finite())
                            .map(|s| div_lo(add_lo(row_lo, -s), a)),
                    )
                } else {
                    (
                        others_most
                            .filter(|_| row_lo.is_finite())
                            .map(|s| div_hi(add_hi(s, -row_lo), -a)),
                        others_least
                            .filter(|_| row_hi.is_finite())
                            .map(|s| div_lo(add_lo(s, -row_hi), -a)),
                    )
                };
                if let Some(u) = up.filter(|u| open_hi && u.is_finite() && *u < hi[j]) {
                    found |= hi[j].is_infinite();
                    hi[j] = u;
                }
                if let Some(l) = down.filter(|l| open_lo && l.is_finite() && *l > lo[j]) {
                    found |= lo[j].is_infinite();
                    lo[j] = l;
                }
            }
        }
        if !found {
            break;
        }
    }
    (lo, hi)
}

/// `y` moved so that each column open on one side, with no bound there that the rows imply, has a reduced cost
/// pointing away from that side by [`SHIFT_MARGIN`]; `None` when no column needs it or none could be helped.
///
/// For each such column `j` in turn, one row `i` of it takes the change `δ` in `yᵢ` that sets `rⱼ` to the margin.
/// The rows are tried by `|aᵢⱼ|`, largest first. A row is taken only when `yᵢ + δ` keeps a sign its finite sides
/// allow, and no other open column of the row with no implied bound is pushed to the wrong side of its margin. A
/// column open on both sides is left alone, since it needs a reduced cost of exactly zero. The arithmetic is plain:
/// [`certified_bound`] checks the result.
pub(crate) fn shift_for_open_columns(problem: &LpProblem, y: &[f64]) -> Option<Vec<f64>> {
    if y.len() != problem.rows() || y.iter().any(|v| !v.is_finite()) {
        return None;
    }
    let (il, iu) = implied_bounds(problem);
    // Which side of each column is open with no implied bound: −1 below only, +1 above only, 2 both, 0 neither.
    let side: Vec<i8> = il
        .iter()
        .zip(&iu)
        .map(|(l, u)| match (l.is_infinite(), u.is_infinite()) {
            (false, false) => 0,
            (true, false) => -1,
            (false, true) => 1,
            (true, true) => 2,
        })
        .collect();
    if side.iter().all(|&s| s == 0) {
        return None;
    }
    let n = problem.cols();
    let (mut r, mut margin) = (vec![0.0f64; n], vec![0.0f64; n]);
    for j in 0..n {
        let (rows, vals) = problem.at.row(j);
        let (mut rj, mut size) = (problem.c[j], 1.0 + problem.c[j].abs());
        for (&i, &a) in rows.iter().zip(vals) {
            let t = a * y[i as usize];
            rj -= t;
            size += t.abs();
        }
        r[j] = rj;
        margin[j] = SHIFT_MARGIN * size;
    }
    // Whether column k's reduced cost may drop by `change` from `from`.
    let allowed = |k: usize, from: f64, change: f64, margin: f64| match side[k] {
        1 => from - change >= from.min(margin),
        -1 => from - change <= from.max(-margin),
        2 => change == 0.0,
        _ => true,
    };
    let mut y = y.to_vec();
    let mut moved = false;
    for j in 0..n {
        let target = match side[j] {
            1 if r[j] < margin[j] => margin[j],
            -1 if r[j] > -margin[j] => -margin[j],
            _ => continue,
        };
        let (rows, vals) = problem.at.row(j);
        let mut order: Vec<(u32, f64)> = rows.iter().copied().zip(vals.iter().copied()).collect();
        order.sort_by(|p, q| q.1.abs().total_cmp(&p.1.abs()).then(p.0.cmp(&q.0)));
        for (i, aij) in order {
            let i = i as usize;
            if aij == 0.0 {
                continue;
            }
            let delta = (r[j] - target) / aij;
            let yi = y[i] + delta;
            if !delta.is_finite()
                || (yi > 0.0 && problem.row_lo[i] == f64::NEG_INFINITY)
                || (yi < 0.0 && problem.row_hi[i] == f64::INFINITY)
            {
                continue;
            }
            let (cols, avals) = problem.a.row(i);
            let fits = cols.iter().zip(avals).all(|(&k, &a)| {
                let k = k as usize;
                k == j || allowed(k, r[k], a * delta, margin[k])
            });
            if !fits {
                continue;
            }
            y[i] = yi;
            for (&k, &a) in cols.iter().zip(avals) {
                r[k as usize] -= a * delta;
            }
            moved = true;
            break;
        }
    }
    moved.then_some(y)
}

/// The bound from row multipliers `y`, or `None` when a column open on the side its reduced cost points to has no
/// bound there that the rows imply either.
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
    let mut implied = None;
    let mut implied_columns = 0;
    for j in 0..problem.cols() {
        let (rows, vals) = problem.at.row(j);
        let (mut rlo, mut rhi) = (problem.c[j], problem.c[j]);
        for (&i, &a) in rows.iter().zip(vals) {
            let yi = ys[i as usize];
            rlo = add_lo(rlo, -mul_hi(a, yi));
            rhi = add_hi(rhi, -mul_lo(a, yi));
        }
        if !(rlo.is_finite() && rhi.is_finite()) {
            return Ok(None);
        }
        let (mut l, mut u) = (problem.col_lo[j], problem.col_hi[j]);
        let (open_lo, open_hi) = (
            l == f64::NEG_INFINITY && rhi > 0.0,
            u == f64::INFINITY && rlo < 0.0,
        );
        if open_lo || open_hi {
            let (il, iu) = implied.get_or_insert_with(|| implied_bounds(problem));
            if open_lo {
                l = il[j];
            }
            if open_hi {
                u = iu[j];
            }
            if (l == f64::NEG_INFINITY && rhi > 0.0) || (u == f64::INFINITY && rlo < 0.0) {
                return Ok(None);
            }
            implied_columns += 1;
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
        implied_columns,
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

    #[test]
    fn directed_quotients_enclose_the_exact_result() {
        assert_eq!(div_hi(6.0, 3.0), 2.0);
        assert_eq!(div_lo(6.0, 3.0), 2.0);
        assert_eq!(div_hi(0.0, 3.0), 0.0);
        // 1/3 rounds down to the nearest double, so that is its lower quotient and the next one up its upper.
        assert_eq!(div_lo(1.0, 3.0), 1.0 / 3.0);
        assert_eq!(div_hi(1.0, 3.0), (1.0f64 / 3.0).next_up());
        assert_eq!(div_hi(-1.0, 3.0), -(1.0f64 / 3.0));
        // 1/10 rounds up: the other way about.
        assert_eq!(div_hi(1.0, 10.0), 0.1);
        assert_eq!(div_lo(1.0, 10.0), 0.1f64.next_down());
        // Near underflow the remainder may not be exact, so the quotient steps either way.
        assert_eq!(div_hi(1e-300, 1.0), 1e-300f64.next_up());
        assert_eq!(div_lo(1e-300, 1.0), 1e-300f64.next_down());
        assert_eq!(div_hi(f64::MAX, 0.5), f64::INFINITY);
    }

    /// A row: its entries as (column, value), then its lower and upper bounds.
    type Row<'a> = (&'a [(usize, f64)], f64, f64);

    fn problem(rows: &[Row], l: &[f64], u: &[f64]) -> LpProblem {
        let mut coo = fmine_numerics::sparse::Coo::new(rows.len(), l.len()).unwrap();
        for (i, (entries, _, _)) in rows.iter().enumerate() {
            for &(j, v) in *entries {
                coo.push(i, j, v).unwrap();
            }
        }
        LpProblem::new(
            vec![0.0; l.len()],
            &coo.assemble(),
            rows.iter().map(|r| r.1).collect(),
            rows.iter().map(|r| r.2).collect(),
            l.to_vec(),
            u.to_vec(),
        )
        .unwrap()
    }

    #[test]
    fn rows_imply_bounds_for_open_columns_only() {
        let inf = f64::INFINITY;
        // x0 + 2x1 <= 4 and 3x0 + x1 <= 6 over x >= 0: tops 4 then 2 for x0, and 2 for x1.
        let p = problem(
            &[
                (&[(0, 1.0), (1, 2.0)], -inf, 4.0),
                (&[(0, 3.0), (1, 1.0)], -inf, 6.0),
            ],
            &[0.0, 0.0],
            &[inf, inf],
        );
        assert_eq!(implied_bounds(&p), (vec![0.0, 0.0], vec![2.0, 2.0]));
        // x2 − x0 <= 1 needs x0's top, which x0 + x1 = 2 with 0 <= x1 <= 5 gives only after the first row was
        // passed: a second round. The first row also lifts x0's bottom from −3 to −1.
        let p = problem(
            &[
                (&[(2, 1.0), (0, -1.0)], -inf, 1.0),
                (&[(0, 1.0), (1, 1.0)], 2.0, 2.0),
            ],
            &[-inf, 0.0, 0.0],
            &[inf, 5.0, inf],
        );
        assert_eq!(
            implied_bounds(&p),
            (vec![-1.0, 0.0, 0.0], vec![2.0, 5.0, 3.0])
        );
        // A finite bound is never touched, even where a row implies a tighter one.
        let p = problem(&[(&[(0, 1.0)], -inf, 1.0)], &[0.0], &[10.0]);
        assert_eq!(implied_bounds(&p), (vec![0.0], vec![10.0]));
        // x0 + x1 >= 1 over x >= 0 implies no top, and −x0 >= −7 implies x0 <= 7 from a negative entry.
        let p = problem(
            &[(&[(0, 1.0), (1, 1.0)], 1.0, inf), (&[(0, -1.0)], -7.0, inf)],
            &[0.0, 0.0],
            &[inf, inf],
        );
        assert_eq!(implied_bounds(&p), (vec![0.0, 0.0], vec![7.0, inf]));
    }
}
