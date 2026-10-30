//! Provenance: frankensim crates/fs-tropical/src/lib.rs @ 20463525d9775a18c1b237d2676f364f367175ab;
//! license: MIT/Apache-2.0. Tropical `(max, +)` / `(min, +)` critical-path schedule algebra (`BD-022`).
//! Determinism: DeterministicPerIsa (no cross-host digest test; plan §29.2, bd-hp2j.1).

/// Precedence arc `(u -> v)` with minimum lag `lag` (`start[v] >= start[u] + lag`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PrecedenceArc {
    pub from: usize,
    pub to: usize,
    pub min_lag: f64,
}

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

    /// Adds a precedence constraint `start[to] >= start[from] + min_lag`.
    pub fn add_arc(&mut self, from: usize, to: usize, min_lag: f64) {
        self.arcs.push(PrecedenceArc { from, to, min_lag });
    }

    /// Computes earliest start times (`(max, +)` tropical longest path from roots) and returns
    /// `None` if a positive precedence cycle exists.
    #[must_use]
    pub fn earliest_starts(&self) -> Option<Vec<f64>> {
        let mut es = vec![0.0_f64; self.n_nodes];
        for _ in 0..self.n_nodes {
            let mut changed = false;
            for arc in &self.arcs {
                let cand = es[arc.from] + arc.min_lag;
                if cand > es[arc.to] + 1e-12 {
                    es[arc.to] = cand;
                    changed = true;
                }
            }
            if !changed {
                return Some(es);
            }
        }
        // Extra pass: if anything still increases, a positive cycle is present.
        for arc in &self.arcs {
            if es[arc.from] + arc.min_lag > es[arc.to] + 1e-12 {
                return None;
            }
        }
        Some(es)
    }

    /// Computes latest start times (`(min, +)` tropical backward pass) given `project_deadline`.
    #[must_use]
    pub fn latest_starts(&self, project_deadline: f64) -> Option<Vec<f64>> {
        let _ = self.earliest_starts()?;
        let mut ls = vec![project_deadline; self.n_nodes];
        for _ in 0..self.n_nodes {
            let mut changed = false;
            for arc in &self.arcs {
                let cand = ls[arc.to] - arc.min_lag;
                if cand < ls[arc.from] - 1e-12 {
                    ls[arc.from] = cand;
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }
        Some(ls)
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
}
