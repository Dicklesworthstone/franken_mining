//! Original FrankenMining code (not lifted). Upstream analogue: frankensim crates/fs-voi/src/lib.rs @ 20463525d (not copied).
//! Expected Value of Perfect and Sample Information (`BD-022`, `BD-225`, `GATE-HELM-02`).
//! Determinism: DeterministicPerIsa (no cross-host digest test; plan §29.2, bd-hp2j.1).

/// Decision problem payoff matrix across discrete/ensemble states (`S` states x `A` actions).
#[derive(Debug, Clone, PartialEq)]
pub struct VoiProblem {
    /// Prior state weights / probabilities summing to `1.0` (`len = n_states`).
    pub state_probs: Vec<f64>,
    /// Payoff matrix `payoffs[state][action]` in USD millions (`$M`).
    pub payoffs: Vec<Vec<f64>>,
}

/// Summary of EVPI / EVSI evaluation (`GATE-HELM-02`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VoiSummary {
    pub prior_best_action: usize,
    pub prior_expected_value: f64,
    pub perfect_info_expected_value: f64,
    pub evpi: f64,
    pub evsi: f64,
    pub net_voi: f64,
}

impl VoiProblem {
    /// Computes the prior optimal action and Expected Value of Perfect Information (`EVPI >= 0`).
    #[must_use]
    pub fn compute_evpi(&self) -> (usize, f64, f64) {
        let n_states = self.state_probs.len();
        if n_states == 0 || self.payoffs.is_empty() {
            return (0, 0.0, 0.0);
        }
        let n_actions = self.payoffs[0].len();
        let mut best_action = 0;
        let mut best_prior_ev = f64::NEG_INFINITY;

        for a in 0..n_actions {
            let mut ev = 0.0_f64;
            for s in 0..n_states {
                ev += self.state_probs[s] * self.payoffs[s][a];
            }
            if ev > best_prior_ev {
                best_prior_ev = ev;
                best_action = a;
            }
        }

        let mut ws_ev = 0.0_f64;
        for s in 0..n_states {
            let max_s = self.payoffs[s]
                .iter()
                .copied()
                .fold(f64::NEG_INFINITY, f64::max);
            ws_ev += self.state_probs[s] * max_s;
        }
        let evpi = (ws_ev - best_prior_ev).max(0.0);
        (best_action, best_prior_ev, evpi)
    }

    /// Computes Expected Value of Sample Information (`0 <= EVSI <= EVPI`) for a signal likelihood
    /// matrix `likelihood[state][signal]` (`P(signal | state)`) and sample cost `cost`.
    #[must_use]
    pub fn compute_evsi(&self, likelihood: &[Vec<f64>], cost: f64) -> VoiSummary {
        let (prior_best_action, prior_ev, evpi) = self.compute_evpi();
        let n_states = self.state_probs.len();
        if n_states == 0 || likelihood.len() != n_states || likelihood[0].is_empty() {
            return VoiSummary {
                prior_best_action,
                prior_expected_value: prior_ev,
                perfect_info_expected_value: prior_ev + evpi,
                evpi,
                evsi: 0.0,
                net_voi: -cost,
            };
        }
        let n_signals = likelihood[0].len();
        let n_actions = self.payoffs[0].len();

        let mut preposterior_ev = 0.0_f64;
        // `z` indexes a column of the likelihood matrix across every row `s`, which no single iterator gives.
        #[allow(clippy::needless_range_loop)]
        for z in 0..n_signals {
            // For signal z, choose action a maximizing sum_s P(s) * P(z | s) * u(s, a).
            let mut best_joint_for_z = f64::NEG_INFINITY;
            for a in 0..n_actions {
                let mut joint_ev = 0.0_f64;
                for s in 0..n_states {
                    joint_ev += self.state_probs[s] * likelihood[s][z] * self.payoffs[s][a];
                }
                if joint_ev > best_joint_for_z {
                    best_joint_for_z = joint_ev;
                }
            }
            preposterior_ev += best_joint_for_z;
        }

        let evsi = (preposterior_ev - prior_ev).clamp(0.0, evpi);
        VoiSummary {
            prior_best_action,
            prior_expected_value: prior_ev,
            perfect_info_expected_value: prior_ev + evpi,
            evpi,
            evsi,
            net_voi: evsi - cost,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_evpi_non_negative_and_evsi_monotone() {
        // Two states (High-grade deep feeder vs barren fault), two actions (Sink shaft vs Walk away).
        let problem = VoiProblem {
            state_probs: vec![0.4, 0.6],
            payoffs: vec![
                vec![250.0, 0.0],  // High grade: shaft yields +$250M, walk yields $0M
                vec![-120.0, 0.0], // Barren: shaft loses -$120M, walk yields $0M
            ],
        };

        // Uninformative signal: P(z|s) = [0.5, 0.5] -> EVSI == 0.
        let uninformative = vec![vec![0.5, 0.5], vec![0.5, 0.5]];
        let s0 = problem.compute_evsi(&uninformative, 2.0);
        assert!(s0.evpi >= 0.0);
        assert!(s0.evsi.abs() < 1e-12);

        // Partially informative twin drillhole: P(hit|high)=0.8, P(miss|barren)=0.85.
        let partial = vec![vec![0.80, 0.20], vec![0.15, 0.85]];
        let s1 = problem.compute_evsi(&partial, 2.0);

        // Higher-precision 3-hole fence: P(hit|high)=0.95, P(miss|barren)=0.95.
        let high_prec = vec![vec![0.95, 0.05], vec![0.05, 0.95]];
        let s2 = problem.compute_evsi(&high_prec, 5.0);

        // Perfect assay signal: P(z=s|s)=1.0 -> EVSI == EVPI.
        let perfect = vec![vec![1.0, 0.0], vec![0.0, 1.0]];
        let s_perf = problem.compute_evsi(&perfect, 0.0);

        assert!(s0.evsi <= s1.evsi + 1e-12);
        assert!(s1.evsi <= s2.evsi + 1e-12);
        assert!(s2.evsi <= s_perf.evsi + 1e-12);
        assert!((s_perf.evsi - s_perf.evpi).abs() < 1e-12);
    }
}
