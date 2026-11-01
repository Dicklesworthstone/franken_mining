//! Original FrankenMining code (not lifted). Upstream analogue: frankensim crates/fs-voi/src/lib.rs @ 20463525d (not copied).
//! Expected Value of Perfect and Sample Information (`BD-022`, `BD-225`, `GATE-HELM-02`).
//! Determinism: DeterministicPerIsa (no cross-host digest test; plan §29.2, bd-hp2j.1).

use fmine_core::rand::{PhiloxStream, RngKey};

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

/// The sign of a net VOI at two Monte Carlo standard errors (plan §22.2 `VoiStatus`, §22.4).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum VoiStatus {
    /// `net_voi − 2·se > 0`.
    Positive,
    /// `net_voi + 2·se < 0`.
    Negative,
    /// Neither: the sign is unresolved at 2·SE. A net VOI of exactly 0 with no sampling error is here too.
    Indeterminate { se: f64 },
}

impl VoiStatus {
    /// The status of a net VOI `net` whose standard error is `se`.
    #[must_use]
    pub fn at_two_se(net: f64, se: f64) -> Self {
        if net - 2.0 * se > 0.0 {
            Self::Positive
        } else if net + 2.0 * se < 0.0 {
            Self::Negative
        } else {
            Self::Indeterminate { se }
        }
    }
}

/// A Monte Carlo estimate of EVSI with its standard error (plan §22.4; `GATE-HELM-02`, `G-HELM-6`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct McEvsi {
    pub prior_best_action: usize,
    /// The mean of the paired differences: an unbiased estimate of EVSI.
    pub evsi: f64,
    /// The Monte Carlo standard error of `evsi`, `s / √n`. 0 when no signal changes the choice.
    pub se: f64,
    /// Draws taken: the requested number, or 0 when no signal changes the choice.
    pub n_samples: usize,
    /// `evsi − cost`.
    pub net_voi: f64,
    /// The sign of `net_voi` at 2·`se`.
    pub status: VoiStatus,
    /// Whether some signal makes another action strictly better than the prior one in expected value. When none
    /// does, EVSI is exactly 0 and nothing is sampled: "no test result changes the choice" (plan §22.4, GB-29).
    pub changes_choice: bool,
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
    /// A Monte Carlo estimate asked for fewer than two draws, which give no standard error.
    TooFewSamples(usize),
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
            Self::TooFewSamples(n) => {
                write!(
                    f,
                    "{n} Monte Carlo draws give no standard error; at least 2 are needed"
                )
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

    /// The number of signals, after checking that `likelihood` has one probability row per state over the same
    /// signals and that `cost` is finite and non-negative.
    fn checked_signals(&self, likelihood: &[Vec<f64>], cost: f64) -> Result<usize, VoiError> {
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
        Ok(n_signals)
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
        let n_signals = self.checked_signals(likelihood, cost)?;
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

    /// The action taken after each signal: the prior action `prior`, unless another action has a strictly
    /// higher joint expected value `Σ_s P(s) P(z | s) u(s, a)`. Keeping the prior action on a tie leaves EVSI
    /// unchanged and keeps ties out of the Monte Carlo variance.
    fn bayes_rule(&self, likelihood: &[Vec<f64>], prior: usize, n_signals: usize) -> Vec<usize> {
        let n_actions = self.payoffs[0].len();
        let joint = |z: usize, a: usize| -> f64 {
            let mut ev = 0.0_f64;
            for ((p, l), payoff) in self.state_probs.iter().zip(likelihood).zip(&self.payoffs) {
                ev += p * l[z] * payoff[a];
            }
            ev
        };
        (0..n_signals)
            .map(|z| {
                let mut best = prior;
                let mut best_ev = joint(z, prior);
                for a in 0..n_actions {
                    let ev = joint(z, a);
                    if ev > best_ev {
                        best = a;
                        best_ev = ev;
                    }
                }
                best
            })
            .collect()
    }

    /// Estimates EVSI by Monte Carlo over the preposterior (plan §22.4), with its standard error.
    ///
    /// Each of `n_samples` draws takes a state `s` from the prior and a signal `z` from `likelihood[s]`, and
    /// scores `u(s, a*(z)) − u(s, a₀)`. Here `a*` is the Bayes action after the signal and `a₀` the prior action.
    /// Both terms use the same draw (common random numbers). The prior term's mean is known, so it is a control
    /// variate with coefficient 1, which keeps the estimate unbiased. The mean of the differences is EVSI in
    /// expectation, and `se` is their sample standard deviation over `√n`.
    ///
    /// The estimate is unbiased only because `a*` is the exact Bayes action, which a finite problem supplies. Any
    /// other decision rule scores at most the Bayes rule's value, so a rule that approximates it gives an estimate
    /// biased low.
    ///
    /// When no signal makes another action strictly better than `a₀`, EVSI is exactly 0. That is checked first, and
    /// nothing is sampled (GB-29). A draw lands on a category by inverse CDF over its positive entries in order; a
    /// row that sums to less than 1, within [`PROBABILITY_SUM_TOLERANCE`], gives the shortfall to its last positive
    /// entry.
    ///
    /// Each `key` names one stream, so a call is a pure function of the problem, `likelihood`, `cost`, `n_samples`
    /// and `key`.
    ///
    /// # Errors
    /// [`VoiError`] for anything [`Self::compute_evsi`] refuses, and [`VoiError::TooFewSamples`] for fewer than two
    /// draws.
    pub fn estimate_evsi_mc(
        &self,
        likelihood: &[Vec<f64>],
        cost: f64,
        n_samples: usize,
        key: RngKey,
    ) -> Result<McEvsi, VoiError> {
        let (prior_best_action, _, _) = self.compute_evpi()?;
        let n_signals = self.checked_signals(likelihood, cost)?;
        if n_samples < 2 {
            return Err(VoiError::TooFewSamples(n_samples));
        }
        let rule = self.bayes_rule(likelihood, prior_best_action, n_signals);
        if rule.iter().all(|&a| a == prior_best_action) {
            return Ok(McEvsi {
                prior_best_action,
                evsi: 0.0,
                se: 0.0,
                n_samples: 0,
                net_voi: -cost,
                status: VoiStatus::at_two_se(-cost, 0.0),
                changes_choice: false,
            });
        }
        let mut rng = PhiloxStream::new(key);
        // Welford's running mean and sum of squared deviations.
        let mut mean = 0.0_f64;
        let mut m2 = 0.0_f64;
        for i in 0..n_samples {
            let s = draw_index(&self.state_probs, rng.next_f64_open01());
            let z = draw_index(&likelihood[s], rng.next_f64_open01());
            let d = self.payoffs[s][rule[z]] - self.payoffs[s][prior_best_action];
            let delta = d - mean;
            mean += delta / (i + 1) as f64;
            m2 += delta * (d - mean);
        }
        let n = n_samples as f64;
        let se = fmine_core::det::sqrt(m2 / (n - 1.0) / n);
        let net_voi = mean - cost;
        Ok(McEvsi {
            prior_best_action,
            evsi: mean,
            se,
            n_samples,
            net_voi,
            status: VoiStatus::at_two_se(net_voi, se),
            changes_choice: true,
        })
    }
}

/// The category a uniform `u` in (0, 1) falls in, by inverse CDF over the positive entries of `p` in order. A
/// zero entry is never chosen. If rounding leaves `u` above the running sum, the last positive entry is chosen.
fn draw_index(p: &[f64], u: f64) -> usize {
    let mut cum = 0.0_f64;
    let mut last_positive = 0;
    for (i, &pi) in p.iter().enumerate() {
        if pi > 0.0 {
            last_positive = i;
            cum += pi;
            if u < cum {
                return i;
            }
        }
    }
    last_positive
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

    /// GB-29 (plan §22.8): recovery high (P = `p_high`, NPV +$300M) or low (NPV −$200M); build or not; a test
    /// that is right with probability `accuracy` in both states.
    fn gb29(p_high: f64, accuracy: f64) -> (VoiProblem, Vec<Vec<f64>>) {
        (
            VoiProblem {
                state_probs: vec![p_high, 1.0 - p_high],
                payoffs: vec![vec![300.0, 0.0], vec![-200.0, 0.0]],
            },
            vec![
                vec![accuracy, 1.0 - accuracy],
                vec![1.0 - accuracy, accuracy],
            ],
        )
    }

    fn key(partition: u32, domain: u32, realization: u32) -> RngKey {
        RngKey {
            seed_root: GATE_HELM_02_SEED,
            partition,
            domain,
            var_group: 0,
            realization,
            brick: 0,
        }
    }

    /// Fixed before the gate first ran, and not to be changed to make it pass.
    const GATE_HELM_02_SEED: u64 = 0x4845_4c4d_3032;

    #[test]
    fn test_gb29_closed_form_known_answers() {
        // Plan §22.8's hand computation: EV(build now) $100M, EVPI $80M, EVSI $41.0M against $9.8M of cost
        // and delay. The two flips: accuracy 73% gives EVSI $9.8M; a prior of 0.78 still flips on a "low"
        // result, with EVSI about $2.3M (B7 EV-P2-4).
        let (p, lik) = gb29(0.6, 0.85);
        let s = p.compute_evsi(&lik, 9.8).expect("GB-29");
        assert_eq!(s.prior_best_action, 0, "build");
        assert!((s.prior_expected_value - 100.0).abs() < 1e-12);
        assert!((s.evpi - 80.0).abs() < 1e-12);
        assert!((s.evsi - 41.0).abs() < 1e-12, "EVSI {}", s.evsi);
        assert!((s.net_voi - 31.2).abs() < 1e-12);
        let (p, lik) = gb29(0.6, 0.73);
        let evsi = p.compute_evsi(&lik, 0.0).expect("accuracy 0.73").evsi;
        assert!((evsi - 9.8).abs() < 1e-12, "EVSI {evsi}");
        let (p, lik) = gb29(0.78, 0.85);
        let evsi = p.compute_evsi(&lik, 0.0).expect("prior 0.78").evsi;
        assert!((evsi - 2.3).abs() < 1e-12, "EVSI {evsi}");
        // Past the no-flip prior 34/43 a "low" result leaves building best, so EVSI is 0 up to rounding.
        let (p, lik) = gb29(0.8, 0.85);
        let evsi = p.compute_evsi(&lik, 0.0).expect("prior 0.8").evsi;
        assert!(evsi.abs() < 1e-12, "EVSI {evsi}");
    }

    #[test]
    fn test_mc_evsi_without_a_flip_is_exactly_zero_and_unsampled() {
        let (p, lik) = gb29(0.8, 0.85);
        let m = p
            .estimate_evsi_mc(&lik, 9.8, 1_000, key(0, 0, 0))
            .expect("valid");
        assert!(!m.changes_choice);
        assert_eq!((m.evsi, m.se, m.n_samples), (0.0, 0.0, 0));
        assert_eq!(m.net_voi, -9.8);
        assert_eq!(m.status, VoiStatus::Negative);
        let free = p
            .estimate_evsi_mc(&lik, 0.0, 1_000, key(0, 0, 0))
            .expect("valid");
        assert_eq!(free.status, VoiStatus::Indeterminate { se: 0.0 });
        // An uninformative test never flips anything either.
        let (p, _) = gb29(0.6, 0.85);
        let flat = vec![vec![0.5, 0.5], vec![0.5, 0.5]];
        let m = p
            .estimate_evsi_mc(&flat, 1.0, 1_000, key(0, 0, 0))
            .expect("valid");
        assert!(!m.changes_choice);
        assert_eq!(m.evsi, 0.0);
        // GB-29 itself flips on a "low" result and is sampled.
        let (p, lik) = gb29(0.6, 0.85);
        let m = p
            .estimate_evsi_mc(&lik, 9.8, 1_000, key(0, 0, 0))
            .expect("valid");
        assert!(m.changes_choice);
        assert_eq!(m.n_samples, 1_000);
        assert!(m.se > 0.0);
    }

    #[test]
    fn test_mc_evsi_refuses_what_the_closed_form_refuses() {
        let (p, lik) = gb29(0.6, 0.85);
        let k = key(0, 0, 0);
        assert_eq!(
            p.estimate_evsi_mc(&lik, 1.0, 1, k),
            Err(VoiError::TooFewSamples(1))
        );
        assert_eq!(
            p.estimate_evsi_mc(&lik, 1.0, 0, k),
            Err(VoiError::TooFewSamples(0))
        );
        assert_eq!(
            p.estimate_evsi_mc(&lik, -1.0, 100, k),
            Err(VoiError::CostOutOfDomain(-1.0))
        );
        assert!(matches!(
            p.estimate_evsi_mc(&[vec![0.85, 0.25], vec![0.15, 0.85]], 1.0, 100, k),
            Err(VoiError::NotNormalised {
                what: "likelihood",
                row: 0,
                ..
            })
        ));
        assert!(matches!(
            p.estimate_evsi_mc(&lik[..1], 1.0, 100, k),
            Err(VoiError::Shape {
                what: "likelihood",
                ..
            })
        ));
        let bad_prior = VoiProblem {
            state_probs: vec![0.6, 0.5],
            payoffs: p.payoffs.clone(),
        };
        assert!(matches!(
            bad_prior.estimate_evsi_mc(&lik, 1.0, 100, k),
            Err(VoiError::NotNormalised {
                what: "state_probs",
                ..
            })
        ));
    }

    #[test]
    fn test_mc_evsi_is_a_pure_function_of_its_key() {
        let (p, lik) = gb29(0.6, 0.85);
        let a = p
            .estimate_evsi_mc(&lik, 9.8, 500, key(0, 0, 7))
            .expect("valid");
        let b = p
            .estimate_evsi_mc(&lik, 9.8, 500, key(0, 0, 7))
            .expect("valid");
        assert_eq!(a.evsi.to_bits(), b.evsi.to_bits());
        assert_eq!(a.se.to_bits(), b.se.to_bits());
        let c = p
            .estimate_evsi_mc(&lik, 9.8, 500, key(0, 0, 8))
            .expect("valid");
        assert_ne!(a.evsi.to_bits(), c.evsi.to_bits());
    }

    #[test]
    fn test_draw_index_never_lands_on_a_zero_entry() {
        let p = [0.0, 0.5, 0.0, 0.5, 0.0];
        for u in [1e-300, 0.25, 0.5 - 1e-16, 0.5, 0.75, 1.0 - 1e-16] {
            let i = draw_index(&p, u);
            assert!(p[i] > 0.0, "u {u} gave {i}");
        }
        assert_eq!(draw_index(&p, 0.25), 1);
        assert_eq!(draw_index(&p, 0.75), 3);
        // A row 1e-10 short of 1 gives the shortfall to its last positive entry.
        assert_eq!(draw_index(&[0.5, 0.5 - 1e-10, 0.0], 1.0 - 1e-16), 1);
    }

    #[test]
    fn test_voi_status_at_two_se() {
        assert_eq!(VoiStatus::at_two_se(2.1, 1.0), VoiStatus::Positive);
        assert_eq!(
            VoiStatus::at_two_se(2.0, 1.0),
            VoiStatus::Indeterminate { se: 1.0 }
        );
        assert_eq!(
            VoiStatus::at_two_se(-1.9, 1.0),
            VoiStatus::Indeterminate { se: 1.0 }
        );
        assert_eq!(VoiStatus::at_two_se(-2.1, 1.0), VoiStatus::Negative);
        assert_eq!(VoiStatus::at_two_se(1e-9, 0.0), VoiStatus::Positive);
    }

    /// `P(X ≥ t)` for `X ~ χ²(k)` from the closed forms, with `x = t/2`: `e^{−x} Σ_{i<k/2} xⁱ/i!` for even `k`, and
    /// `erfc(√x) + e^{−x} Σ_{i=1}^{(k−1)/2} x^{i−1/2}/Γ(i+1/2)` for odd `k`. Every term is below 1, so nothing
    /// overflows. For `k ≤ 200`, an `e^{−x}` that underflows to 0 leaves out less than 1e-190. For odd `k`,
    /// `erfc` is `1 − erf`, which costs about 1e-16 absolute, so far-tail values lose relative accuracy; the gate
    /// reads only tails between 0.005 and 0.995. Test-only: GATE-HELM-02 is its one consumer.
    fn chi_square_sf(t: f64, k: u32) -> f64 {
        assert!((1..=200).contains(&k) && t.is_finite() && t >= 0.0);
        let x = 0.5 * t;
        if k.is_multiple_of(2) {
            let mut term = fmine_core::det::exp(-x);
            let mut sum = term;
            for i in 1..k / 2 {
                term *= x / f64::from(i);
                sum += term;
            }
            sum
        } else {
            let rx = fmine_core::det::sqrt(x);
            // i = 1: e^{−x} x^{1/2} / Γ(3/2), and Γ(3/2) = √π / 2.
            let mut term = fmine_core::det::exp(-x) * std::f64::consts::FRAC_2_SQRT_PI * rx;
            let mut sum = 1.0 - fmine_core::det::erf(rx);
            for i in 1..=(k - 1) / 2 {
                sum += term;
                term *= x / (f64::from(i) + 0.5);
            }
            sum
        }
    }

    #[test]
    fn test_chi_square_sf_known_answers() {
        // scipy.stats.chi2.sf (scipy 1.18.1), as printed by `python3 -I scripts/oracle/chi_square_known_answers.py
        // --scipy`. That script also integrates the density by Simpson's rule on CPython's libm, which agrees
        // with these to 5e-13 relative.
        let cases: [(f64, u32, f64); 12] = [
            (3.841458820694124, 1, 0.04999999999999994),
            (0.5, 1, 0.47950012218695337),
            (5.991464547107979, 2, 0.05000000000000008),
            (7.814727903251178, 3, 0.05000000000000002),
            (0.5, 3, 0.9188914116546758),
            (2.0, 4, 0.7357588823428847),
            (40.0, 7, 1.258790387371308e-06),
            (23.209251158954356, 10, 0.010000000000000004),
            (3.0738, 12, 0.995000181621716),
            (12.0, 12, 0.4456796413646112),
            (28.2995, 12, 0.005000031844890244),
            (60.0, 25, 0.00010455486131526447),
        ];
        for (t, k, want) in cases {
            let got = chi_square_sf(t, k);
            assert!(
                (got - want).abs() <= 1e-12 * want + 1e-16,
                "chi2 sf({t}, {k}) = {got}, want {want}"
            );
        }
        assert_eq!(chi_square_sf(0.0, 12), 1.0);
    }

    /// GATE-HELM-02's problems, each with its likelihood. Four come from GB-29 (plan §22.8): the card, its two
    /// flips (accuracy 73%, prior 0.78) and a three-action variant with an asymmetric test. Eight are seeded:
    /// 2 to 4 states, 2 or 3 actions, 2 or 3 signals, payoffs uniform on [−300, 300] $M, and prior and
    /// likelihood rows normalised from weights uniform on [0.05, 1.05]. A seeded problem in which no signal
    /// changes the choice has no Monte Carlo error to test, so the next seed replaces it.
    fn gate_problems() -> Vec<(VoiProblem, Vec<Vec<f64>>)> {
        let mut problems = vec![gb29(0.6, 0.85), gb29(0.6, 0.73), gb29(0.78, 0.85)];
        problems.push((
            VoiProblem {
                state_probs: vec![0.5, 0.5],
                payoffs: vec![vec![500.0, 200.0, 0.0], vec![-400.0, -90.0, 0.0]],
            },
            vec![vec![0.9, 0.1], vec![0.3, 0.7]],
        ));
        let weights = |rng: &mut PhiloxStream, n: u32| -> Vec<f64> {
            let w: Vec<f64> = (0..n).map(|_| 0.05 + rng.next_f64_open01()).collect();
            let total: f64 = w.iter().sum();
            w.into_iter().map(|v| v / total).collect()
        };
        let mut draw = 0;
        for g in 0..8_u32 {
            let (n_states, n_actions, n_signals) = (2 + g % 3, 2 + g % 2, 2 + (g / 2) % 2);
            loop {
                let mut rng = PhiloxStream::new(key(1, 0, draw));
                draw += 1;
                let state_probs = weights(&mut rng, n_states);
                let likelihood: Vec<Vec<f64>> = (0..n_states)
                    .map(|_| weights(&mut rng, n_signals))
                    .collect();
                let payoffs = (0..n_states)
                    .map(|_| {
                        (0..n_actions)
                            .map(|_| 600.0 * rng.next_f64_open01() - 300.0)
                            .collect()
                    })
                    .collect();
                let p = VoiProblem {
                    state_probs,
                    payoffs,
                };
                let flips = p
                    .estimate_evsi_mc(&likelihood, 0.0, 2, key(1, 1, 0))
                    .expect("valid")
                    .changes_choice;
                if flips {
                    problems.push((p, likelihood));
                    break;
                }
                assert!(draw < 1_000, "no seeded problem with a flip");
            }
        }
        problems
    }

    /// The χ² statistic of `K` problems, each given as its closed-form EVSI and its seeds' `(estimate, se)`: the
    /// sum of the squared standardised deviations of the seed means, each standardised by the seeds' own
    /// reported standard errors. Returns the statistic and `P(χ²(K) ≥ statistic)`.
    fn chi_square_gate(rows: &[(f64, Vec<(f64, f64)>)]) -> (f64, f64) {
        let statistic: f64 = rows
            .iter()
            .map(|(closed, seeds)| {
                let n = seeds.len() as f64;
                let mean = seeds.iter().map(|(e, _)| e).sum::<f64>() / n;
                let se = fmine_core::det::sqrt(seeds.iter().map(|(_, s)| s * s).sum::<f64>()) / n;
                let z = (mean - closed) / se;
                z * z
            })
            .sum();
        let k = u32::try_from(rows.len()).expect("few problems");
        (statistic, chi_square_sf(statistic, k))
    }

    /// Two-sided at α = 0.01. The upper tail catches a biased estimate or an understated SE; the lower tail
    /// catches an overstated SE, which would otherwise pass any estimate.
    fn gate_passes(sf: f64) -> bool {
        (0.005..=0.995).contains(&sf)
    }

    #[test]
    fn test_mc_evsi_recovers_closed_form_chi2() {
        // GATE-HELM-02 (plan §22.7): the Monte Carlo estimator recovers the closed-form EVSI of K GB-29-type
        // discrete problems, by a χ² test of the K standardised deviations of the 100-seed means at α = 0.01.
        // There is no per-seed 2·SE rule: it fails an unbiased estimator almost surely over 100 seeds (R7-C121).
        const SEEDS: u32 = 100;
        const DRAWS: usize = 2_000;
        let problems = gate_problems();
        assert_eq!(problems.len(), 12);
        let rows: Vec<(f64, Vec<(f64, f64)>)> = problems
            .iter()
            .zip(0_u32..)
            .map(|((p, lik), k)| {
                let closed = p.compute_evsi(lik, 0.0).expect("valid").evsi;
                let seeds = (0..SEEDS)
                    .map(|j| {
                        let m = p
                            .estimate_evsi_mc(lik, 0.0, DRAWS, key(2, k, j))
                            .expect("valid");
                        assert!(m.changes_choice && m.se > 0.0);
                        (m.evsi, m.se)
                    })
                    .collect();
                (closed, seeds)
            })
            .collect();
        let (statistic, sf) = chi_square_gate(&rows);
        eprintln!(
            "GATE-HELM-02: K = {}, chi-square = {statistic:.4}, P(chi2 >= statistic) = {sf:.4}",
            rows.len()
        );
        assert!(
            gate_passes(sf),
            "estimator fails: chi-square {statistic}, upper tail {sf}"
        );

        // Planted negatives, each a transform of the same seeds. A bias of half a reported SE per seed is small
        // next to each seed's own 2·SE band, yet moves each 100-seed mean by about 5 of its SEs. Reporting a
        // third of the SE scales the statistic by 9, and three times the SE divides it by 9; at K = 12 an
        // unbiased estimator's statistic is caught by both with probability above 0.99.
        type SeedTransform = fn(f64, f64) -> (f64, f64);
        let negatives: [(&str, SeedTransform); 3] = [
            ("biased by se/2", |e, s| (e + 0.5 * s, s)),
            ("se understated 3x", |e, s| (e, s / 3.0)),
            ("se overstated 3x", |e, s| (e, 3.0 * s)),
        ];
        for (name, f) in negatives {
            let planted: Vec<(f64, Vec<(f64, f64)>)> = rows
                .iter()
                .map(|(closed, seeds)| (*closed, seeds.iter().map(|&(e, s)| f(e, s)).collect()))
                .collect();
            let (statistic, sf) = chi_square_gate(&planted);
            assert!(
                !gate_passes(sf),
                "{name} passes: chi-square {statistic}, upper tail {sf}"
            );
        }
    }
}
