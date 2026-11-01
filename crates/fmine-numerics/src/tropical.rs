//! Original FrankenMining code (not lifted). Upstream analogue: frankensim crates/fs-tropical/src/lib.rs @ 20463525d (not copied).
//! Tropical `(max, +)` / `(min, +)` critical-path schedule algebra (`BD-022`).
//! Determinism: DeterministicPerIsa (no cross-host digest test; plan §29.2, bd-hp2j.1).

/// How much a start time must grow, in the lags' own unit, for a relaxation to count as a change.
///
/// It is absolute, not scaled to the lags. A start can be left up to this much short of its longest path for each
/// arc on that path; a positive cycle whose total lag is below it is not detected; and once starts exceed about
/// 1e4 an ulp is larger than it, so a zero-total cycle whose lags do not cancel exactly in floating point can be
/// refused as positive.
pub const RELAXATION_TOLERANCE: f64 = 1e-12;

/// Precedence arc `(u -> v)` with minimum lag `lag` (`start[v] >= start[u] + lag`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PrecedenceArc {
    pub from: usize,
    pub to: usize,
    pub min_lag: f64,
}

/// Why a schedule pass refused (bd-a482). Before it, a bad arc panicked on an index or poisoned every start.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TropicalError {
    /// An arc names a node at or past `n_nodes`.
    ArcOutOfRange {
        from: usize,
        to: usize,
        n_nodes: usize,
    },
    /// An arc's lag is NaN or infinite.
    NonFiniteLag { from: usize, to: usize, lag: f64 },
    /// The arcs contain a cycle of positive total lag, so no start times satisfy them.
    PositiveCycle,
    /// The project deadline is NaN or infinite.
    NonFiniteDeadline(f64),
}

impl std::fmt::Display for TropicalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ArcOutOfRange { from, to, n_nodes } => {
                write!(f, "arc {from} -> {to} names a node outside 0..{n_nodes}")
            }
            Self::NonFiniteLag { from, to, lag } => write!(f, "arc {from} -> {to} has lag {lag}"),
            Self::PositiveCycle => write!(f, "the precedence arcs contain a positive cycle"),
            Self::NonFiniteDeadline(d) => write!(f, "project deadline {d} is not finite"),
        }
    }
}

impl std::error::Error for TropicalError {}

/// Tropical precedence DAG for bench/stope and project critical-path analysis (`BD-022`).
#[derive(Debug, Clone, Default)]
pub struct TropicalDag {
    pub n_nodes: usize,
    pub arcs: Vec<PrecedenceArc>,
}

impl TropicalDag {
    /// Creates a precedence graph on `n_nodes` activities (`0..n_nodes`).
    #[must_use]
    pub const fn new(n_nodes: usize) -> Self {
        Self {
            n_nodes,
            arcs: Vec::new(),
        }
    }

    /// Adds a precedence constraint `start[to] >= start[from] + min_lag`. It is checked when a pass runs, since
    /// `arcs` is public and may be filled directly.
    pub fn add_arc(&mut self, from: usize, to: usize, min_lag: f64) {
        self.arcs.push(PrecedenceArc { from, to, min_lag });
    }

    /// Every arc inside `0..n_nodes` with a finite lag.
    fn check_arcs(&self) -> Result<(), TropicalError> {
        for arc in &self.arcs {
            if arc.from >= self.n_nodes || arc.to >= self.n_nodes {
                return Err(TropicalError::ArcOutOfRange {
                    from: arc.from,
                    to: arc.to,
                    n_nodes: self.n_nodes,
                });
            }
            if !arc.min_lag.is_finite() {
                return Err(TropicalError::NonFiniteLag {
                    from: arc.from,
                    to: arc.to,
                    lag: arc.min_lag,
                });
            }
        }
        Ok(())
    }

    /// Computes earliest start times (`(max, +)` tropical longest path from roots).
    ///
    /// # Errors
    /// [`TropicalError::ArcOutOfRange`] or [`TropicalError::NonFiniteLag`] for a bad arc, and
    /// [`TropicalError::PositiveCycle`] when no start times satisfy the arcs.
    pub fn earliest_starts(&self) -> Result<Vec<f64>, TropicalError> {
        self.check_arcs()?;
        let mut es = vec![0.0_f64; self.n_nodes];
        for _ in 0..self.n_nodes {
            let mut changed = false;
            for arc in &self.arcs {
                let cand = es[arc.from] + arc.min_lag;
                if cand > es[arc.to] + RELAXATION_TOLERANCE {
                    es[arc.to] = cand;
                    changed = true;
                }
            }
            if !changed {
                return Ok(es);
            }
        }
        // Extra pass: if anything still increases, a positive cycle is present.
        for arc in &self.arcs {
            if es[arc.from] + arc.min_lag > es[arc.to] + RELAXATION_TOLERANCE {
                return Err(TropicalError::PositiveCycle);
            }
        }
        Ok(es)
    }

    /// Computes latest start times (`(min, +)` tropical backward pass) given `project_deadline`.
    ///
    /// # Errors
    /// [`TropicalError::NonFiniteDeadline`] for a NaN or infinite deadline, and anything
    /// [`Self::earliest_starts`] refuses.
    pub fn latest_starts(&self, project_deadline: f64) -> Result<Vec<f64>, TropicalError> {
        if !project_deadline.is_finite() {
            return Err(TropicalError::NonFiniteDeadline(project_deadline));
        }
        let _ = self.earliest_starts()?;
        let mut ls = vec![project_deadline; self.n_nodes];
        for _ in 0..self.n_nodes {
            let mut changed = false;
            for arc in &self.arcs {
                let cand = ls[arc.to] - arc.min_lag;
                if cand < ls[arc.from] - RELAXATION_TOLERANCE {
                    ls[arc.from] = cand;
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }
        Ok(ls)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_min_plus_longest_path_duality() {
        // 4-node mining sequence: Overburden strip (0) -> Bench A (1) & Bench B (2) -> Crusher feed (3).
        let mut dag = TropicalDag::new(4);
        dag.add_arc(0, 1, 10.0);
        dag.add_arc(0, 2, 14.0);
        dag.add_arc(1, 3, 8.0);
        dag.add_arc(2, 3, 12.0);

        let es = dag.earliest_starts().expect("acyclic DAG");
        assert_eq!(es, vec![0.0, 10.0, 14.0, 26.0]);

        // Duality: at deadline = makespan (26.0), critical path nodes (0 -> 2 -> 3) have zero total float (LS == ES),
        // whereas non-critical node 1 has positive total float (LS - ES = 18 - 10 = 8).
        let ls = dag.latest_starts(26.0).expect("valid backward pass");
        assert!((ls[0] - es[0]).abs() < 1e-12);
        assert!((ls[2] - es[2]).abs() < 1e-12);
        assert!((ls[3] - es[3]).abs() < 1e-12);
        assert!(((ls[1] - es[1]) - 8.0).abs() < 1e-12);
    }

    #[test]
    fn test_tropical_refuses_bad_arcs_cycles_and_deadlines() {
        // Before bd-a482 an out-of-range arc panicked on an index and a NaN lag gave NaN starts.
        let mut out_of_range = TropicalDag::new(2);
        out_of_range.add_arc(0, 2, 1.0);
        assert_eq!(
            out_of_range.earliest_starts(),
            Err(TropicalError::ArcOutOfRange {
                from: 0,
                to: 2,
                n_nodes: 2
            })
        );
        assert!(out_of_range.latest_starts(5.0).is_err());

        let mut nan_lag = TropicalDag::new(2);
        nan_lag.add_arc(0, 1, f64::NAN);
        assert!(matches!(
            nan_lag.earliest_starts(),
            Err(TropicalError::NonFiniteLag { from: 0, to: 1, .. })
        ));

        let mut cycle = TropicalDag::new(3);
        cycle.add_arc(0, 1, 2.0);
        cycle.add_arc(1, 2, 2.0);
        cycle.add_arc(2, 0, -3.0);
        assert_eq!(cycle.earliest_starts(), Err(TropicalError::PositiveCycle));
        assert_eq!(cycle.latest_starts(10.0), Err(TropicalError::PositiveCycle));

        // A cycle of zero total lag is feasible: the activities start together.
        let mut tied = TropicalDag::new(2);
        tied.add_arc(0, 1, 0.0);
        tied.add_arc(1, 0, 0.0);
        assert_eq!(tied.earliest_starts(), Ok(vec![0.0, 0.0]));

        let mut dag = TropicalDag::new(2);
        dag.add_arc(0, 1, 3.0);
        assert!(matches!(
            dag.latest_starts(f64::INFINITY),
            Err(TropicalError::NonFiniteDeadline(_))
        ));
        assert_eq!(dag.latest_starts(10.0), Ok(vec![7.0, 10.0]));
    }
}
