//! Original FrankenMining code (not lifted). Upstream analogue: frankensim crates/fs-voi/src/lib.rs @ 20463525d (not copied).
//! Expected Value of Perfect and Sample Information (`BD-022`, `BD-225`, `GATE-HELM-02`).
//! Determinism: DeterministicPerIsa (no cross-host digest test; plan §29.2, bd-hp2j.1).

/// How far a probability vector's sum may be from 1 before it is refused.
pub const PROBABILITY_SUM_TOLERANCE: f64 = 1e-9;

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

/// Why a VOI computation refused (bd-a482). There is no zero default and no clamped result.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum VoiError {
    /// No states, no actions, or no signals.
    Empty { what: &'static str },
    /// A matrix does not have the shape the states, actions or signals give it.
    Shape {
        what: &'static str,
        row: usize,
        expected: usize,
        found: usize,
    },
    /// A payoff, probability or likelihood is NaN or infinite.
    NonFinite { what: &'static str, row: usize },
    /// The sample cost is NaN, infinite or negative.
    CostOutOfDomain(f64),
    /// A probability is negative or above 1.
    ProbabilityOutOfDomain {
        what: &'static str,
        row: usize,
        value: f64,
    },
    /// A probability vector does not sum to 1 within [`PROBABILITY_SUM_TOLERANCE`].
    NotNormalised {
        what: &'static str,
        row: usize,
        sum: f64,
    },
}

impl std::fmt::Display for VoiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Empty { what } => write!(f, "{what} is empty"),
            Self::Shape {
                what,
                row,
                expected,
                found,
            } => write!(
                f,
                "{what} row {row} has {found} entries, expected {expected}"
            ),
            Self::NonFinite { what, row } => write!(f, "{what} row {row} holds a non-finite value"),
            Self::CostOutOfDomain(c) => write!(f, "sample cost {c} is not finite and non-negative"),
            Self::ProbabilityOutOfDomain { what, row, value } => {
                write!(
                    f,
                    "{what} row {row} holds probability {value}, outside [0, 1]"
                )
            }
            Self::NotNormalised { what, row, sum } => {
                write!(f, "{what} row {row} sums to {sum}, not 1")
            }
        }
    }
}

impl std::error::Error for VoiError {}

/// A probability vector: every entry finite in `[0, 1]`, summing to 1 within [`PROBABILITY_SUM_TOLERANCE`].
fn check_probabilities(what: &'static str, row: usize, p: &[f64]) -> Result<(), VoiError> {
    if p.iter().any(|v| !v.is_finite()) {
        return Err(VoiError::NonFinite { what, row });
    }
    if let Some(&value) = p.iter().find(|v| !(0.0..=1.0).contains(*v)) {
        return Err(VoiError::ProbabilityOutOfDomain { what, row, value });
    }
    let sum: f64 = p.iter().sum();
    if (sum - 1.0).abs() > PROBABILITY_SUM_TOLERANCE {
        return Err(VoiError::NotNormalised { what, row, sum });
    }
    Ok(())
}

impl VoiProblem {
    /// The number of actions, after checking the problem's shape, payoffs and prior.
    fn checked_actions(&self) -> Result<usize, VoiError> {
        let n_states = self.state_probs.len();
        if n_states == 0 {
            return Err(VoiError::Empty {
                what: "state_probs",
            });
        }
        if self.payoffs.len() != n_states {
            return Err(VoiError::Shape {
                what: "payoffs",
                row: 0,
                expected: n_states,
                found: self.payoffs.len(),
            });
        }
        let n_actions = self.payoffs[0].len();
        if n_actions == 0 {
            return Err(VoiError::Empty { what: "payoffs" });
        }
        for (row, payoff) in self.payoffs.iter().enumerate() {
            if payoff.len() != n_actions {
                return Err(VoiError::Shape {
                    what: "payoffs",
                    row,
                    expected: n_actions,
                    found: payoff.len(),
                });
            }
            if payoff.iter().any(|v| !v.is_finite()) {
                return Err(VoiError::NonFinite {
                    what: "payoffs",
                    row,
                });
            }
        }
        check_probabilities("state_probs", 0, &self.state_probs)?;
        Ok(n_actions)
    }

    /// Computes the prior optimal action and Expected Value of Perfect Information.
    ///
    /// EVPI is not clamped. It is never negative: each state's best payoff is at least the prior action's, and
    /// the two sums are rounded term by term in the same order.
    ///
    /// # Errors
    /// [`VoiError`] for an empty or ragged problem, a non-finite payoff, or a prior that is not a probability
    /// vector.
    pub fn compute_evpi(&self) -> Result<(usize, f64, f64), VoiError> {
        let n_actions = self.checked_actions()?;
        let mut best_action = 0;
        let mut best_prior_ev = f64::NEG_INFINITY;
        for a in 0..n_actions {
            let mut ev = 0.0_f64;
            for (p, payoff) in self.state_probs.iter().zip(&self.payoffs) {
                ev += p * payoff[a];
            }
            if ev > best_prior_ev {
                best_prior_ev = ev;
                best_action = a;
            }
        }
        let mut ws_ev = 0.0_f64;
        for (p, payoff) in self.state_probs.iter().zip(&self.payoffs) {
            let max_s = payoff.iter().copied().fold(f64::NEG_INFINITY, f64::max);
            ws_ev += p * max_s;
        }
        Ok((best_action, best_prior_ev, ws_ev - best_prior_ev))
    }

    /// Computes Expected Value of Sample Information for a signal likelihood matrix `likelihood[state][signal]`
    /// (`P(signal | state)`) and sample cost `cost`.
    ///
    /// EVSI is not clamped into `[0, EVPI]`. With a valid likelihood it lies there up to rounding and the
    /// row-sum tolerance (each row off by at most [`PROBABILITY_SUM_TOLERANCE`] moves it by at most that times the
    /// largest payoff). Before bd-a482 a likelihood whose rows did not sum to 1 could give any value, which the
    /// clamp then hid.
    ///
    /// # Errors
    /// [`VoiError`] for anything [`Self::compute_evpi`] refuses, a likelihood that is not one probability row per
    /// state over the same signals, or a `cost` that is not finite and non-negative.
    pub fn compute_evsi(&self, likelihood: &[Vec<f64>], cost: f64) -> Result<VoiSummary, VoiError> {
        let (prior_best_action, prior_ev, evpi) = self.compute_evpi()?;
        let n_states = self.state_probs.len();
        if likelihood.len() != n_states {
            return Err(VoiError::Shape {
                what: "likelihood",
                row: 0,
                expected: n_states,
                found: likelihood.len(),
            });
        }
        let n_signals = likelihood[0].len();
        if n_signals == 0 {
            return Err(VoiError::Empty { what: "likelihood" });
        }
        for (row, l) in likelihood.iter().enumerate() {
            if l.len() != n_signals {
                return Err(VoiError::Shape {
                    what: "likelihood",
                    row,
                    expected: n_signals,
                    found: l.len(),
                });
            }
            check_probabilities("likelihood", row, l)?;
        }
        if !(cost.is_finite() && cost >= 0.0) {
            return Err(VoiError::CostOutOfDomain(cost));
        }
        let n_actions = self.payoffs[0].len();
        let mut preposterior_ev = 0.0_f64;
        for z in 0..n_signals {
            // For signal z, choose action a maximizing sum_s P(s) * P(z | s) * u(s, a).
            let mut best_joint_for_z = f64::NEG_INFINITY;
            for a in 0..n_actions {
                let mut joint_ev = 0.0_f64;
                for ((p, l), payoff) in self.state_probs.iter().zip(likelihood).zip(&self.payoffs) {
                    joint_ev += p * l[z] * payoff[a];
                }
                if joint_ev > best_joint_for_z {
                    best_joint_for_z = joint_ev;
                }
            }
            preposterior_ev += best_joint_for_z;
        }
        let evsi = preposterior_ev - prior_ev;
        Ok(VoiSummary {
            prior_best_action,
            prior_expected_value: prior_ev,
            perfect_info_expected_value: prior_ev + evpi,
            evpi,
            evsi,
            net_voi: evsi - cost,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn problem() -> VoiProblem {
        // Two states (High-grade deep feeder vs barren fault), two actions (Sink shaft vs Walk away).
        VoiProblem {
            state_probs: vec![0.4, 0.6],
            payoffs: vec![
                vec![250.0, 0.0],  // High grade: shaft yields +$250M, walk yields $0M
                vec![-120.0, 0.0], // Barren: shaft loses -$120M, walk yields $0M
            ],
        }
    }

    #[test]
    fn test_evpi_non_negative_and_evsi_monotone() {
        let problem = problem();
        // Uninformative signal: P(z|s) = [0.5, 0.5] -> EVSI == 0.
        let uninformative = vec![vec![0.5, 0.5], vec![0.5, 0.5]];
        let s0 = problem.compute_evsi(&uninformative, 2.0).expect("valid");
        assert!(s0.evpi >= 0.0);
        assert!(s0.evsi.abs() < 1e-12);
        // Partially informative twin drillhole: P(hit|high)=0.8, P(miss|barren)=0.85.
        let partial = vec![vec![0.80, 0.20], vec![0.15, 0.85]];
        let s1 = problem.compute_evsi(&partial, 2.0).expect("valid");
        // Higher-precision 3-hole fence: P(hit|high)=0.95, P(miss|barren)=0.95.
        let high_prec = vec![vec![0.95, 0.05], vec![0.05, 0.95]];
        let s2 = problem.compute_evsi(&high_prec, 5.0).expect("valid");
        // Perfect assay signal: P(z=s|s)=1.0 -> EVSI == EVPI.
        let perfect = vec![vec![1.0, 0.0], vec![0.0, 1.0]];
        let s_perf = problem.compute_evsi(&perfect, 0.0).expect("valid");
        assert!(s0.evsi <= s1.evsi + 1e-12);
        assert!(s1.evsi <= s2.evsi + 1e-12);
        assert!(s2.evsi <= s_perf.evsi + 1e-12);
        assert!((s_perf.evsi - s_perf.evpi).abs() < 1e-12);
    }

    #[test]
    fn test_voi_refuses_bad_shapes_probabilities_and_costs() {
        let ok = problem();
        let lik = vec![vec![0.8, 0.2], vec![0.15, 0.85]];
        let refuses = |p: &VoiProblem, l: &[Vec<f64>], cost: f64| p.compute_evsi(l, cost).is_err();

        // Before bd-a482 these returned zeros, panicked on an index, or clamped EVSI.
        let empty = VoiProblem {
            state_probs: vec![],
            payoffs: vec![],
        };
        assert_eq!(
            empty.compute_evpi(),
            Err(VoiError::Empty {
                what: "state_probs"
            })
        );
        let short = VoiProblem {
            state_probs: vec![0.4, 0.6],
            payoffs: vec![vec![250.0, 0.0]],
        };
        assert!(matches!(
            short.compute_evpi(),
            Err(VoiError::Shape {
                what: "payoffs",
                ..
            })
        ));
        let ragged = VoiProblem {
            state_probs: vec![0.4, 0.6],
            payoffs: vec![vec![250.0, 0.0], vec![-120.0]],
        };
        assert!(matches!(
            ragged.compute_evpi(),
            Err(VoiError::Shape {
                what: "payoffs",
                row: 1,
                expected: 2,
                found: 1
            })
        ));
        let no_actions = VoiProblem {
            state_probs: vec![1.0],
            payoffs: vec![vec![]],
        };
        assert_eq!(
            no_actions.compute_evpi(),
            Err(VoiError::Empty { what: "payoffs" })
        );
        let prior = |state_probs: Vec<f64>| {
            VoiProblem {
                state_probs,
                payoffs: ok.payoffs.clone(),
            }
            .compute_evpi()
        };
        assert!(matches!(
            prior(vec![0.4, 0.5]),
            Err(VoiError::NotNormalised {
                what: "state_probs",
                ..
            })
        ));
        assert!(matches!(
            prior(vec![-0.1, 1.1]),
            Err(VoiError::ProbabilityOutOfDomain { what: "state_probs", row: 0, value }) if value == -0.1
        ));
        assert_eq!(
            prior(vec![f64::NAN, 1.0]),
            Err(VoiError::NonFinite {
                what: "state_probs",
                row: 0
            })
        );
        assert!(matches!(
            ok.compute_evsi(&[vec![1.2, -0.2], vec![0.15, 0.85]], 1.0),
            Err(VoiError::ProbabilityOutOfDomain {
                what: "likelihood",
                row: 0,
                ..
            })
        ));
        let nan_payoff = VoiProblem {
            state_probs: vec![0.4, 0.6],
            payoffs: vec![vec![f64::NAN, 0.0], vec![-120.0, 0.0]],
        };
        assert_eq!(
            nan_payoff.compute_evpi(),
            Err(VoiError::NonFinite {
                what: "payoffs",
                row: 0
            })
        );

        assert!(
            refuses(&ok, &lik[..1], 1.0),
            "one likelihood row for two states"
        );
        assert!(refuses(&ok, &[vec![], vec![]], 1.0), "no signals");
        assert!(
            refuses(&ok, &[vec![0.8, 0.2], vec![1.0]], 1.0),
            "ragged likelihood"
        );
        assert!(
            matches!(
                ok.compute_evsi(&[vec![0.8, 0.3], vec![0.15, 0.85]], 1.0),
                Err(VoiError::NotNormalised {
                    what: "likelihood",
                    row: 0,
                    ..
                })
            ),
            "a likelihood row summing to 1.1"
        );
        assert!(refuses(&ok, &lik, f64::NAN), "NaN cost");
        assert_eq!(
            ok.compute_evsi(&lik, -1.0),
            Err(VoiError::CostOutOfDomain(-1.0))
        );
        assert!(ok.compute_evsi(&lik, 1.0).is_ok());

        // Rows 4e-10 short of 1 are within the tolerance and accepted. Their EVSI is 4e-10 x 28 below 0 and is
        // reported as computed; the old clamp into [0, EVPI] would have shown 0.
        let short = vec![vec![0.5, 0.5 - 4e-10], vec![0.5, 0.5 - 4e-10]];
        let s = ok.compute_evsi(&short, 0.0).expect("within tolerance");
        assert!((s.evsi + 4e-10 * 28.0).abs() < 1e-15, "EVSI {}", s.evsi);
    }
}
