//! Restarted primal–dual hybrid gradient for large sparse linear programs (PDLP; `BD-251`, plan §15.4), with a
//! certified dual bound (PO-PUSH-4).
//! Determinism: DeterministicPerIsa (fixed-order sums at any thread count; `sqrt` and `powf`; no cross-host digest
//! test).
//!
//! The problem is `minimise c·x subject to row_lo ≤ A x ≤ row_hi and col_lo ≤ x ≤ col_hi`, with `A` a
//! [`fmine_numerics::sparse::Csr`]. A bound may be infinite; an equality row has `row_lo = row_hi`.
//!
//! The method follows Applegate et al. (2021), "Practical large-scale linear programming using primal-dual hybrid
//! gradient", as cuPDLP (Lu and Yang, 2023) runs it:
//! - ten rounds of Ruiz equilibration, then Pock–Chambolle scaling (α = 1);
//! - PDHG steps `x' = proj(x − τ(c − Kᵀy))`, `y' = prox(y − σK(2x' − x))` with `τ = η/ω`, `σ = ηω`, and PDLP's
//!   adaptive step rule for `η`;
//! - every 64 steps, the KKT error of the current point and of the step-weighted average. The better one becomes
//!   the restart candidate. A restart happens when its error is at most 0.2 of the error at the last restart, or at
//!   most 0.8 and rising, or when the steps since the last restart pass 0.36 of all steps. The primal weight `ω`
//!   moves to the geometric mean of itself and `‖Δy‖/‖Δx‖` across the restart;
//! - it stops when, in the unscaled problem, `‖primal residual‖ ≤ ε(1 + ‖b‖)`, `‖dual residual‖ ≤ ε(1 + ‖c‖)`
//!   and `|c·x − dual objective| ≤ ε(1 + |c·x| + |dual objective|)`, where `b` holds each row's largest finite
//!   bound.
//!
//! Every returned solution carries a [`certified_bound`]: the highest of its final multipliers', of the
//! multipliers with the best dual objective seen at any evaluation, and of each of those moved for open columns
//! (bd-tcp4), with the multipliers it came from. So the optimum is bounded even when the iteration limit or the
//! observer stops the run.
//!
//! At each evaluation, three candidate Farkas rays are tried: the last step's change in `y`, its change since the
//! last restart, and `y` itself (bd-gj80). One that [`certified_infeasibility`] certifies proves that no `x` meets
//! the rows and column bounds, and stops the run with [`PdlpStatus::Infeasible`]. A candidate is certified only
//! when a plain-arithmetic value of it is positive, and after a failed attempt the next waits 1, 2, 4, … up to 64
//! evaluations. There is no presolve, feasibility polishing or unboundedness detection: an unbounded problem, and
//! an infeasible one that no candidate proves, run to the iteration limit.

mod bound;
mod engine;
mod mat;

pub use bound::{DualBound, certified_bound, certified_infeasibility};

use fmine_evidence::{Refusal, RefusalCode};
use fmine_numerics::sparse::Csr;

use mat::Mat;

fn refuse(detail: impl Into<String>) -> Refusal {
    Refusal::new(RefusalCode::NumericalCertificateFailed207, detail)
}

/// `minimise c·x subject to row_lo ≤ A x ≤ row_hi, col_lo ≤ x ≤ col_hi`, checked on construction.
#[derive(Debug, Clone)]
pub struct LpProblem {
    pub(crate) c: Vec<f64>,
    a: Mat,
    pub(crate) at: Mat,
    pub(crate) row_lo: Vec<f64>,
    pub(crate) row_hi: Vec<f64>,
    pub(crate) col_lo: Vec<f64>,
    pub(crate) col_hi: Vec<f64>,
}

fn check_bounds(what: &str, lo: &[f64], hi: &[f64], n: usize) -> Result<(), Refusal> {
    if lo.len() != n || hi.len() != n {
        return Err(refuse(format!(
            "{what} bounds: {} lower and {} upper for {n}",
            lo.len(),
            hi.len()
        )));
    }
    for (k, (&l, &h)) in lo.iter().zip(hi).enumerate() {
        if l.is_nan() || h.is_nan() || l > h || l == f64::INFINITY || h == f64::NEG_INFINITY {
            return Err(refuse(format!("{what} {k} has bounds [{l}, {h}]")));
        }
    }
    Ok(())
}

impl LpProblem {
    /// Checks and stores the problem.
    ///
    /// # Errors
    /// `FMINE-REF-207` (proposed) when a length does not match `a`, a cost or matrix entry is not finite, a bound
    /// is NaN, a lower bound is `+∞` or exceeds its upper bound, an upper bound is `−∞`, or `a` has `u32::MAX` or
    /// more rows or columns.
    pub fn new(
        c: Vec<f64>,
        a: &Csr,
        row_lo: Vec<f64>,
        row_hi: Vec<f64>,
        col_lo: Vec<f64>,
        col_hi: Vec<f64>,
    ) -> Result<Self, Refusal> {
        let a = Mat::from_csr(a)?;
        let (m, n) = (a.nrows(), a.ncols);
        if c.len() != n {
            return Err(refuse(format!("{} costs for {n} columns", c.len())));
        }
        if let Some(j) = c.iter().position(|v| !v.is_finite()) {
            return Err(refuse(format!("cost {j} is {}", c[j])));
        }
        check_bounds("row", &row_lo, &row_hi, m)?;
        check_bounds("column", &col_lo, &col_hi, n)?;
        let at = a.transpose();
        Ok(LpProblem {
            c,
            a,
            at,
            row_lo,
            row_hi,
            col_lo,
            col_hi,
        })
    }

    /// Rows of `A`.
    #[must_use]
    pub fn rows(&self) -> usize {
        self.a.nrows()
    }

    /// Columns of `A`, one per variable.
    #[must_use]
    pub fn cols(&self) -> usize {
        self.a.ncols
    }

    /// Stored entries of `A`.
    #[must_use]
    pub fn nnz(&self) -> usize {
        self.a.nnz()
    }

    /// `c·x`, summed in column order.
    ///
    /// # Errors
    /// `FMINE-REF-207` when `x` is not one value per column.
    pub fn objective(&self, x: &[f64]) -> Result<f64, Refusal> {
        if x.len() != self.cols() {
            return Err(refuse(format!(
                "{} values for {} columns",
                x.len(),
                self.cols()
            )));
        }
        Ok(self.c.iter().zip(x).map(|(c, v)| c * v).sum())
    }

    /// The largest violation of a row or column bound by `x`, in the problem's own units.
    ///
    /// # Errors
    /// `FMINE-REF-207` when `x` is not one value per column.
    pub fn max_violation(&self, x: &[f64]) -> Result<f64, Refusal> {
        if x.len() != self.cols() {
            return Err(refuse(format!(
                "{} values for {} columns",
                x.len(),
                self.cols()
            )));
        }
        let mut worst = 0.0f64;
        for ((&v, &l), &u) in x.iter().zip(&self.col_lo).zip(&self.col_hi) {
            worst = worst.max(l - v).max(v - u);
        }
        for r in 0..self.rows() {
            let (cols, vals) = self.a.row(r);
            let ax: f64 = cols
                .iter()
                .zip(vals)
                .map(|(&j, &a)| a * x[j as usize])
                .sum();
            worst = worst.max(self.row_lo[r] - ax).max(ax - self.row_hi[r]);
        }
        Ok(worst)
    }
}

/// Solver settings. Any value outside its stated range is refused, never adjusted.
#[derive(Debug, Clone, PartialEq)]
pub struct PdlpOptions {
    /// Relative tolerance ε of the stopping rule, in `(0, 1)`. Default `1e-4`, cuPDLP's. ε does not bound the
    /// certified bound's distance from the optimum: on MineLib's CPIT LPs at 1e-4 that distance reached 1.05%,
    /// and at 1e-6 it stayed within 1e-4 (CONTRACT §8).
    pub tolerance: f64,
    /// Accepted PDHG steps before giving up, at least 1. Default 1,000,000.
    pub max_iterations: u64,
    /// Worker threads; 0 picks the machine's parallelism, at most 16. Small problems use one regardless. The
    /// result is the same at any count.
    pub threads: usize,
}

impl Default for PdlpOptions {
    fn default() -> Self {
        PdlpOptions {
            tolerance: 1e-4,
            max_iterations: 1_000_000,
            threads: 0,
        }
    }
}

/// Why the solver stopped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PdlpStatus {
    /// The stopping rule held.
    Optimal,
    /// `max_iterations` steps ran first.
    IterationLimit,
    /// The observer asked to stop.
    Stopped,
    /// Row multipliers proved that no `x` meets the rows and column bounds: [`PdlpSolution::infeasibility_ray`].
    Infeasible,
}

/// Where the run stands at an evaluation, every 64 steps; the measures are those of the better of the current and
/// average points, unscaled.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Progress {
    pub iterations: u64,
    pub restarts: u64,
    pub primal_objective: f64,
    pub dual_objective: f64,
    /// `‖primal residual‖ / (1 + ‖b‖)`.
    pub relative_primal_residual: f64,
    /// `‖dual residual‖ / (1 + ‖c‖)`.
    pub relative_dual_residual: f64,
    /// `|c·x − dual objective| / (1 + |c·x| + |dual objective|)`.
    pub relative_gap: f64,
    pub step_size: f64,
    pub primal_weight: f64,
}

/// The solver's result, in the problem's own units.
#[derive(Debug, Clone, PartialEq)]
pub struct PdlpSolution {
    status: PdlpStatus,
    x: Vec<f64>,
    y: Vec<f64>,
    last: Progress,
    bound: Option<DualBound>,
    bound_y: Vec<f64>,
    ray: Option<Vec<f64>>,
}

impl PdlpSolution {
    #[must_use]
    pub fn status(&self) -> PdlpStatus {
        self.status
    }

    /// One value per column, within the column bounds.
    #[must_use]
    pub fn x(&self) -> &[f64] {
        &self.x
    }

    /// One multiplier per row.
    #[must_use]
    pub fn y(&self) -> &[f64] {
        &self.y
    }

    /// The measures at the returned point.
    #[must_use]
    pub fn progress(&self) -> &Progress {
        &self.last
    }

    /// No feasible point is cheaper than this. It is the highest [`certified_bound`] of up to four multipliers:
    /// `y`; those with the highest dual objective seen at any evaluation; and each of the two moved so that a
    /// column open on the side its reduced cost points to, with no bound there that the rows imply, gets a
    /// reduced cost of the right sign (bd-tcp4). The dual objective is not monotone along the run, so the
    /// best-seen can be the tighter. On a boxed problem nothing is moved. A tie goes to the earlier. With
    /// [`PdlpStatus::Infeasible`] there is no feasible point, so the bound holds but says nothing.
    #[must_use]
    pub fn bound(&self) -> Option<DualBound> {
        self.bound
    }

    /// The multipliers [`PdlpSolution::bound`] came from, one per row: `certified_bound(problem,
    /// bound_multipliers())` gives it again.
    #[must_use]
    pub fn bound_multipliers(&self) -> &[f64] {
        &self.bound_y
    }

    /// With [`PdlpStatus::Infeasible`], the row multipliers that prove no `x` meets the rows and column bounds:
    /// `certified_infeasibility(problem, ray)` certifies them again. `None` with any other status.
    #[must_use]
    pub fn infeasibility_ray(&self) -> Option<&[f64]> {
        self.ray.as_deref()
    }
}

/// Solves `problem`.
///
/// # Errors
/// `FMINE-REF-207` (proposed) for options outside their ranges or a worker thread that panicked.
pub fn solve(problem: &LpProblem, options: &PdlpOptions) -> Result<PdlpSolution, Refusal> {
    solve_observed(problem, options, &mut |_| true)
}

/// As [`solve`], calling `observer` at each evaluation; it returns `false` to stop the run, which then returns
/// the better of the current and average points with [`PdlpStatus::Stopped`].
///
/// # Errors
/// As [`solve`].
pub fn solve_observed(
    problem: &LpProblem,
    options: &PdlpOptions,
    observer: &mut dyn FnMut(&Progress) -> bool,
) -> Result<PdlpSolution, Refusal> {
    if !(options.tolerance > 0.0 && options.tolerance < 1.0) {
        return Err(refuse(format!(
            "tolerance {} is outside (0, 1)",
            options.tolerance
        )));
    }
    if options.max_iterations == 0 {
        return Err(refuse("max_iterations is 0"));
    }
    let (status, x, y, best_y, last, ray) = engine::run_pdlp(problem, options, observer)?;
    // The final and the best-seen multipliers, then each moved for open columns (bd-tcp4): the highest certified
    // bound wins, the earlier on a tie. On a boxed problem nothing is moved.
    let mut candidates = vec![y.clone(), best_y];
    for k in 0..2 {
        if let Some(moved) = bound::shift_for_open_columns(problem, &candidates[k]) {
            candidates.push(moved);
        }
    }
    let mut chosen: Option<(DualBound, usize)> = None;
    for (k, candidate) in candidates.iter().enumerate() {
        if let Some(b) = certified_bound(problem, candidate)?
            && chosen.is_none_or(|(c, _)| b.value() > c.value())
        {
            chosen = Some((b, k));
        }
    }
    let (bound, bound_y) = match chosen {
        Some((b, k)) => (Some(b), candidates.swap_remove(k)),
        None => (None, y.clone()),
    };
    Ok(PdlpSolution {
        status,
        x,
        y,
        last,
        bound,
        bound_y,
        ray,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use fmine_numerics::sparse::Coo;

    fn csr(rows: usize, cols: usize, entries: &[(usize, usize, f64)]) -> Csr {
        let mut coo = Coo::new(rows, cols).unwrap();
        for &(r, c, v) in entries {
            coo.push(r, c, v).unwrap();
        }
        coo.assemble()
    }

    fn opts() -> PdlpOptions {
        PdlpOptions {
            tolerance: 1e-9,
            max_iterations: 200_000,
            threads: 1,
        }
    }

    /// With x, y in [0, 1], x − y = 1 leaves only (1, 0), so x + y ≥ 1.5 cannot hold: the run stops with a ray that
    /// certifies. With x + y ≥ 0.5 instead, (1, 0) is the one feasible point, and the run ends there with no ray.
    #[test]
    fn an_infeasible_lp_is_proved_infeasible() {
        let a = csr(2, 2, &[(0, 0, 1.0), (0, 1, 1.0), (1, 0, 1.0), (1, 1, -1.0)]);
        let lp = |need: f64| {
            LpProblem::new(
                vec![1.0, 2.0],
                &a,
                vec![need, 1.0],
                vec![f64::INFINITY, 1.0],
                vec![0.0; 2],
                vec![1.0; 2],
            )
            .unwrap()
        };
        let p = lp(1.5);
        let s = solve(&p, &opts()).unwrap();
        assert_eq!(s.status(), PdlpStatus::Infeasible, "{:?}", s.progress());
        let ray = s.infeasibility_ray().unwrap();
        let proof = certified_infeasibility(&p, ray).unwrap().unwrap();
        assert!(proof.value() > 0.0, "{proof:?}");
        assert!(s.progress().iterations < 10_000, "{:?}", s.progress());

        let p = lp(0.5);
        let s = solve(&p, &opts()).unwrap();
        assert_eq!(s.status(), PdlpStatus::Optimal);
        assert_eq!(s.infeasibility_ray(), None);
        assert!(
            (s.progress().primal_objective - 1.0).abs() < 1e-6,
            "{:?}",
            s.progress()
        );
    }

    /// max x + y s.t. x + 2y ≤ 4, 3x + y ≤ 6, 0 ≤ x, y ≤ 10: the vertex (8/5, 6/5), worth 14/5. The upper bounds
    /// are inactive; with them infinite the same multipliers may certify nothing (next test).
    #[test]
    fn two_variable_known_answer() {
        let a = csr(2, 2, &[(0, 0, 1.0), (0, 1, 2.0), (1, 0, 3.0), (1, 1, 1.0)]);
        let inf = f64::INFINITY;
        let p = LpProblem::new(
            vec![-1.0, -1.0],
            &a,
            vec![-inf; 2],
            vec![4.0, 6.0],
            vec![0.0; 2],
            vec![10.0; 2],
        )
        .unwrap();
        let s = solve(&p, &opts()).unwrap();
        assert_eq!(s.status(), PdlpStatus::Optimal);
        assert!(
            (s.x()[0] - 1.6).abs() < 1e-6 && (s.x()[1] - 1.2).abs() < 1e-6,
            "{:?}",
            s.x()
        );
        // Duals of the two rows: 2/5 and 1/5, as multipliers of a ≤ row in a minimisation they are negative.
        assert!(
            (s.y()[0] + 0.4).abs() < 1e-6 && (s.y()[1] + 0.2).abs() < 1e-6,
            "{:?}",
            s.y()
        );
        let b = s.bound().unwrap().value();
        assert!(b <= -2.8 && b > -2.8 - 1e-6, "{b}");
        // The exact duals leave zero reduced costs, so the bound is the optimum itself, rounded down once.
        let exact = certified_bound(&p, &[-0.4, -0.2]).unwrap().unwrap().value();
        assert!(
            exact <= -2.8 && exact >= (-2.8f64).next_down().next_down(),
            "{exact}"
        );
    }

    /// With the columns open above, a reduced cost a hair below zero is met by the top the rows imply (bd-tcp4).
    /// Where the rows imply none, it leaves no bound.
    #[test]
    fn open_columns_certify_through_the_tops_their_rows_imply() {
        let a = csr(2, 2, &[(0, 0, 1.0), (0, 1, 2.0), (1, 0, 3.0), (1, 1, 1.0)]);
        let inf = f64::INFINITY;
        let p = LpProblem::new(
            vec![-1.0, -1.0],
            &a,
            vec![-inf; 2],
            vec![4.0, 6.0],
            vec![0.0; 2],
            vec![inf; 2],
        )
        .unwrap();
        // y slightly too small in magnitude: both reduced costs are negative (−3e-9 and −1e-9), and the rows give
        // both columns the top 2, so the bound is about −2.8 − 2e-9.
        let b = certified_bound(&p, &[-0.4, -0.2 + 1e-9]).unwrap().unwrap();
        assert_eq!(b.implied_columns(), 2);
        assert!(
            b.value() <= -2.8 - 1.9e-9 && b.value() > -2.8 - 2.1e-9,
            "{}",
            b.value()
        );
        // y slightly too large: both reduced costs positive, a valid (weaker) bound, with no implied top used.
        let b = certified_bound(&p, &[-0.4 - 1e-9, -0.2 - 1e-9])
            .unwrap()
            .unwrap();
        assert_eq!(b.implied_columns(), 0);
        assert!(
            b.value() <= -2.8 && b.value() > -2.8 - 1e-6,
            "{}",
            b.value()
        );
        let s = solve(&p, &opts()).unwrap();
        assert_eq!(s.status(), PdlpStatus::Optimal);
        let b = s.bound().unwrap().value();
        assert!(b <= -2.8 && b > -2.8 - 1e-6, "{b}");

        // min x0 + x1 s.t. x0 + x1 >= 1 over x >= 0: no row bounds either column above.
        let p = LpProblem::new(
            vec![1.0, 1.0],
            &csr(1, 2, &[(0, 0, 1.0), (0, 1, 1.0)]),
            vec![1.0],
            vec![inf],
            vec![0.0; 2],
            vec![inf; 2],
        )
        .unwrap();
        assert_eq!(certified_bound(&p, &[1.0 + 1e-9]).unwrap(), None);
        let b = certified_bound(&p, &[1.0 - 1e-9]).unwrap().unwrap();
        assert!(
            b.value() <= 1.0 - 1e-9 && b.value() > 1.0 - 1e-8,
            "{}",
            b.value()
        );
        // Moving the multiplier to about 1 − 3e-11 gives both reduced costs the margin, and a bound.
        let moved = bound::shift_for_open_columns(&p, &[1.0 + 1e-9]).unwrap();
        assert!(moved[0] < 1.0 && moved[0] > 1.0 - 1e-10, "{moved:?}");
        let b = certified_bound(&p, &moved).unwrap().unwrap();
        assert!(b.value() <= 1.0 && b.value() > 1.0 - 1e-10, "{}", b.value());
        // Multipliers already of the right sign by the margin are left as they are.
        assert_eq!(bound::shift_for_open_columns(&p, &[0.5]), None);
        let s = solve(&p, &opts()).unwrap();
        let b = s.bound().unwrap().value();
        assert!(b <= 1.0 && b > 1.0 - 1e-6, "{b}");
    }

    /// The shift leaves boxed problems, and free columns with no implied bound, alone.
    #[test]
    fn the_shift_moves_only_what_it_can_certify() {
        let inf = f64::INFINITY;
        let a = csr(1, 2, &[(0, 0, 1.0), (0, 1, 1.0)]);
        let boxed = LpProblem::new(
            vec![1.0, 1.0],
            &a,
            vec![1.0],
            vec![inf],
            vec![0.0; 2],
            vec![5.0; 2],
        )
        .unwrap();
        assert_eq!(bound::shift_for_open_columns(&boxed, &[2.0]), None);
        // min x0 + x1 s.t. x0 + x1 >= 1 with x0 >= 0 and x1 free: no row bounds x1 either way, so its reduced cost
        // may not move, and the one row that could fix x0's would move it.
        let free = LpProblem::new(
            vec![1.0, 1.0],
            &a,
            vec![1.0],
            vec![inf],
            vec![0.0, -inf],
            vec![inf; 2],
        )
        .unwrap();
        assert_eq!(bound::shift_for_open_columns(&free, &[1.0 + 1e-9]), None);
        // min −x0 s.t. x0 >= 1 is unbounded. The fix would need a negative multiplier on a ≥ row, which its sides
        // forbid, so none is made.
        let a = csr(1, 1, &[(0, 0, 1.0)]);
        let ge =
            LpProblem::new(vec![-1.0], &a, vec![1.0], vec![inf], vec![0.0], vec![inf]).unwrap();
        assert_eq!(bound::shift_for_open_columns(&ge, &[0.0]), None);
    }

    /// min x1 + x2 s.t. x1 + x2 = 2 (an equality row), x1 free, 0 ≤ x2 ≤ 5.
    #[test]
    fn equality_row_and_free_column() {
        let a = csr(1, 2, &[(0, 0, 1.0), (0, 1, 1.0)]);
        let inf = f64::INFINITY;
        let p = LpProblem::new(
            vec![1.0, 1.0],
            &a,
            vec![2.0],
            vec![2.0],
            vec![-inf, 0.0],
            vec![inf, 5.0],
        )
        .unwrap();
        let s = solve(&p, &opts()).unwrap();
        assert_eq!(s.status(), PdlpStatus::Optimal);
        assert!((s.progress().primal_objective - 2.0).abs() < 1e-6);
        // The exact multiplier 1 leaves zero reduced costs, so even the free column certifies, at exactly 2.
        let b = certified_bound(&p, &[1.0]).unwrap().unwrap();
        assert_eq!(b.value(), 2.0);
        assert_eq!(b.implied_columns(), 0);
        // 1 + 1e-12 leaves the free column the reduced cost −1e-12, met by the top 2 the row implies (x1 = 2 − x2):
        // about 2 + 2e-12 − 2e-12 − 5e-12.
        let b = certified_bound(&p, &[1.0 + 1e-12]).unwrap().unwrap();
        assert_eq!(b.implied_columns(), 1);
        assert!(
            b.value() <= 2.0 - 4.9e-12 && b.value() > 2.0 - 5.1e-12,
            "{}",
            b.value()
        );
    }

    /// A ≤ row's multiplier must be non-positive in a minimisation; a positive one is repaired to zero.
    #[test]
    fn wrong_signed_multipliers_are_repaired() {
        let a = csr(1, 1, &[(0, 0, 1.0)]);
        let p = LpProblem::new(
            vec![1.0],
            &a,
            vec![f64::NEG_INFINITY],
            vec![3.0],
            vec![0.0],
            vec![10.0],
        )
        .unwrap();
        let b = certified_bound(&p, &[7.0]).unwrap().unwrap();
        assert_eq!(b.repaired_rows(), 1);
        assert!(b.value() <= 0.0 && b.value() > -1e-300, "{}", b.value());
    }

    #[test]
    fn bad_input_is_refused() {
        let a = csr(1, 2, &[(0, 0, 1.0), (0, 1, 1.0)]);
        let ok = |c: Vec<f64>, lo: f64, hi: f64, l: f64, u: f64| {
            LpProblem::new(c, &a, vec![lo], vec![hi], vec![l; 2], vec![u; 2])
        };
        let inf = f64::INFINITY;
        let code = |r: Result<LpProblem, Refusal>| r.unwrap_err().code;
        assert!(ok(vec![1.0, 1.0], 0.0, 1.0, 0.0, 1.0).is_ok());
        for bad in [
            ok(vec![1.0], 0.0, 1.0, 0.0, 1.0),
            ok(vec![f64::NAN, 1.0], 0.0, 1.0, 0.0, 1.0),
            ok(vec![inf, 1.0], 0.0, 1.0, 0.0, 1.0),
            ok(vec![1.0, 1.0], 2.0, 1.0, 0.0, 1.0),
            ok(vec![1.0, 1.0], inf, inf, 0.0, 1.0),
            ok(vec![1.0, 1.0], -inf, -inf, 0.0, 1.0),
            ok(vec![1.0, 1.0], f64::NAN, 1.0, 0.0, 1.0),
            ok(vec![1.0, 1.0], 0.0, 1.0, 1.0, 0.0),
            LpProblem::new(
                vec![1.0, 1.0],
                &a,
                vec![0.0, 0.0],
                vec![1.0],
                vec![0.0; 2],
                vec![1.0; 2],
            ),
            LpProblem::new(
                vec![1.0, 1.0],
                &csr(1, 2, &[(0, 0, inf)]),
                vec![0.0],
                vec![1.0],
                vec![0.0; 2],
                vec![1.0; 2],
            ),
        ] {
            assert_eq!(code(bad), "FMINE-REF-207");
        }
        let p = ok(vec![1.0, 1.0], 0.0, 1.0, 0.0, 1.0).unwrap();
        for o in [
            PdlpOptions {
                tolerance: 0.0,
                ..PdlpOptions::default()
            },
            PdlpOptions {
                tolerance: 1.0,
                ..PdlpOptions::default()
            },
            PdlpOptions {
                tolerance: f64::NAN,
                ..PdlpOptions::default()
            },
            PdlpOptions {
                max_iterations: 0,
                ..PdlpOptions::default()
            },
        ] {
            assert_eq!(solve(&p, &o).unwrap_err().code, "FMINE-REF-207");
        }
        assert_eq!(certified_bound(&p, &[]).unwrap_err().code, "FMINE-REF-207");
        assert_eq!(
            certified_bound(&p, &[f64::NAN]).unwrap_err().code,
            "FMINE-REF-207"
        );
        assert!(p.objective(&[1.0]).is_err() && p.max_violation(&[1.0]).is_err());
    }

    #[test]
    fn observer_can_stop_the_run() {
        let a = csr(2, 2, &[(0, 0, 1.0), (0, 1, 2.0), (1, 0, 3.0), (1, 1, 1.0)]);
        let inf = f64::INFINITY;
        let p = LpProblem::new(
            vec![-1.0, -1.0],
            &a,
            vec![-inf; 2],
            vec![4.0, 6.0],
            vec![0.0; 2],
            vec![inf; 2],
        )
        .unwrap();
        let mut seen = 0;
        let s = solve_observed(&p, &opts(), &mut |pr| {
            seen += 1;
            pr.iterations < 64
        })
        .unwrap();
        assert!(seen >= 1);
        assert!(matches!(
            s.status(),
            PdlpStatus::Stopped | PdlpStatus::Optimal
        ));
    }
}
