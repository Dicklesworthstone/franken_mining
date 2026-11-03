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
//! Every returned solution carries a [`certified_bound`]: the higher of its final multipliers' and of the
//! multipliers with the best dual objective seen at any evaluation, with the multipliers it came from. So the
//! optimum is bounded even when the iteration limit or the observer stops the run. There is no presolve,
//! feasibility polishing or infeasibility detection: an infeasible or unbounded problem runs to the iteration
//! limit.

mod bound;
mod engine;
mod mat;

pub use bound::{DualBound, certified_bound};

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

    /// No feasible point is cheaper than this. It is the higher of two [`certified_bound`]s: that of `y`, and that
    /// of the multipliers with the highest dual objective seen at any evaluation. The dual objective is not
    /// monotone along the run, so the second can be the tighter.
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
    let (status, x, y, best_y, last) = engine::run_pdlp(problem, options, observer)?;
    let at_end = certified_bound(problem, &y)?;
    let at_best = certified_bound(problem, &best_y)?;
    let best_is_higher = match (at_best, at_end) {
        (Some(b), Some(e)) => b.value() > e.value(),
        (Some(_), None) => true,
        (None, _) => false,
    };
    let (bound, bound_y) = if best_is_higher {
        (at_best, best_y)
    } else {
        (at_end, y.clone())
    };
    Ok(PdlpSolution {
        status,
        x,
        y,
        last,
        bound,
        bound_y,
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

    /// With the columns unbounded above, a reduced cost a hair below zero leaves no bound; one is either absent or
    /// valid.
    #[test]
    fn unbounded_columns_certify_only_with_sign_feasible_reduced_costs() {
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
        // y slightly too small in magnitude: column 0's reduced cost is negative, so no bound.
        assert_eq!(certified_bound(&p, &[-0.4, -0.2 + 1e-9]).unwrap(), None);
        // y slightly too large: both reduced costs positive, a valid (weaker) bound.
        let b = certified_bound(&p, &[-0.4 - 1e-9, -0.2 - 1e-9])
            .unwrap()
            .unwrap()
            .value();
        assert!(b <= -2.8 && b > -2.8 - 1e-6, "{b}");
        let s = solve(&p, &opts()).unwrap();
        assert_eq!(s.status(), PdlpStatus::Optimal);
        if let Some(b) = s.bound() {
            assert!(b.value() <= -2.8);
        }
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
        // Any other multiplier leaves the free column a non-zero reduced cost: no bound.
        assert_eq!(certified_bound(&p, &[1.0 + 1e-12]).unwrap(), None);
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
