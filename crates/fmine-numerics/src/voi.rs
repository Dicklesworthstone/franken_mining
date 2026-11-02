//! Original FrankenMining code (not lifted). Upstream analogue: frankensim crates/fs-voi/src/lib.rs @ 20463525d (not copied).
//! Expected Value of Perfect and Sample Information (`BD-022`, `BD-225`, `GATE-HELM-02`).
//! Determinism: DeterministicPerIsa (no cross-host digest test; plan §29.2, bd-hp2j.1).

use std::fmt;

use fmine_core::rand::{PhiloxStream, RngKey};

/// Floating-point admission tolerance for a declared probability distribution. Accepted inputs are
/// never renormalized. Arithmetic that cannot retain the information bounds still refuses.
pub const PROBABILITY_MASS_TOLERANCE: f64 = 1e-12;

#[derive(Debug, Clone, PartialEq)]
pub enum VoiError {
    EmptyStates,
    EmptyActions,
    Shape {
        matrix: &'static str,
        row: Option<usize>,
        expected: usize,
        actual: usize,
    },
    Probability {
        distribution: &'static str,
        row: Option<usize>,
        index: usize,
        value: f64,
    },
    ProbabilityMass {
        distribution: &'static str,
        row: Option<usize>,
        sum: f64,
    },
    Payoff {
        state: usize,
        action: usize,
        value: f64,
    },
    InvalidCost {
        value: f64,
    },
    Arithmetic {
        quantity: &'static str,
    },
    InformationBound {
        evsi: f64,
        evpi: f64,
    },
    /// A Monte Carlo estimate asked for fewer than two draws, which give no standard error.
    TooFewSamples {
        draws: usize,
    },
}

impl VoiError {
    #[must_use]
    pub const fn is_numerical_failure(&self) -> bool {
        matches!(
            self,
            Self::Arithmetic { .. } | Self::InformationBound { .. }
        )
    }
}

impl fmt::Display for VoiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyStates => f.write_str("a decision needs at least one state"),
            Self::EmptyActions => f.write_str("a decision needs at least one action"),
            Self::Shape {
                matrix,
                row,
                expected,
                actual,
            } => write!(
                f,
                "{matrix} row {row:?}: expected {expected} entries, found {actual}"
            ),
            Self::Probability {
                distribution,
                row,
                index,
                value,
            } => write!(
                f,
                "{distribution} row {row:?}, entry {index}: invalid probability {value}"
            ),
            Self::ProbabilityMass {
                distribution,
                row,
                sum,
            } => write!(
                f,
                "{distribution} row {row:?}: probability mass {sum} does not sum to one"
            ),
            Self::Payoff {
                state,
                action,
                value,
            } => write!(
                f,
                "state {state}, action {action}: non-finite payoff {value}"
            ),
            Self::InvalidCost { value } => {
                write!(f, "experiment cost must be finite and nonnegative: {value}")
            }
            Self::Arithmetic { quantity } => write!(f, "cannot represent {quantity} reliably"),
            Self::InformationBound { evsi, evpi } => write!(
                f,
                "sample information {evsi} exceeds perfect information {evpi}; no score was clamped"
            ),
            Self::TooFewSamples { draws } => write!(
                f,
                "{draws} Monte Carlo draws give no standard error; at least 2 are needed"
            ),
        }
    }
}

impl std::error::Error for VoiError {}

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

/// An explicitly supplied experiment, with its cost in the payoff matrix's unit. Each row is
/// P(signal | state), including rows for states whose prior probability is zero.
#[derive(Debug, Clone, Copy)]
pub struct VoiExperiment<'a> {
    pub likelihood: &'a [Vec<f64>],
    pub cost: f64,
}

/// The optimal response to one possible signal, conditional on the declared decision model.
/// Impossible signals have no posterior or action policy; they are retained with zero contribution.
#[derive(Debug, Clone, PartialEq)]
pub struct VoiSignalPolicy {
    pub signal_index: usize,
    pub probability: f64,
    pub posterior_state_probs: Option<Vec<f64>>,
    /// All ties in compensated expected-payoff differences, in ascending input action order.
    /// The first is the deterministic choice; rounded absolute payoffs can conceal a real difference.
    pub best_actions: Vec<usize>,
    pub action_expected_payoffs: Option<Vec<f64>>,
    /// P(signal) times the gain from adapting instead of keeping the prior optimal action.
    /// The independently evaluated summary can differ by floating-point arithmetic rounding.
    pub evsi_contribution: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct VoiAnalysis {
    pub summary: VoiSummary,
    pub prior_action_values: Vec<f64>,
    pub prior_best_actions: Vec<usize>,
    pub signals: Vec<VoiSignalPolicy>,
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

/// Validates the shared decision once and caches its action values, policy and statewise maxima.
/// An immutable borrow prevents a campaign from using a different prior or payoff matrix afterward.
#[derive(Debug)]
pub struct ValidatedVoiProblem<'a> {
    problem: &'a VoiProblem,
    prior_action_values: Vec<f64>,
    prior_best_actions: Vec<usize>,
    state_maxima: Vec<f64>,
    perfect_info_expected_value: f64,
    evpi: f64,
}

impl VoiProblem {
    pub fn validate(&self) -> Result<ValidatedVoiProblem<'_>, VoiError> {
        let n_states = self.state_probs.len();
        if n_states == 0 {
            return Err(VoiError::EmptyStates);
        }
        shape("payoffs", None, n_states, self.payoffs.len())?;
        let n_actions = self.payoffs.first().map_or(0, Vec::len);
        if n_actions == 0 {
            return Err(VoiError::EmptyActions);
        }
        distribution(&self.state_probs, "prior", None)?;
        for (state, row) in self.payoffs.iter().enumerate() {
            shape("payoffs", Some(state), n_actions, row.len())?;
            for (action, value) in row.iter().enumerate() {
                if !value.is_finite() {
                    return Err(VoiError::Payoff {
                        state,
                        action,
                        value: *value,
                    });
                }
            }
        }
        let prior_action_values = (0..n_actions)
            .map(|action| {
                weighted_sum(
                    self.state_probs
                        .iter()
                        .zip(&self.payoffs)
                        .map(|(p, row)| (*p, row[action])),
                    "prior expected payoff",
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        let prior_best_actions = optimal_actions(&self.state_probs, &self.payoffs, n_actions)?;
        let prior_best_action = *prior_best_actions.first().ok_or(VoiError::EmptyActions)?;
        let state_maxima = self
            .payoffs
            .iter()
            .map(|row| {
                row.iter()
                    .copied()
                    .reduce(f64::max)
                    .ok_or(VoiError::EmptyActions)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let perfect_info_expected_value = weighted_sum(
            self.state_probs
                .iter()
                .zip(&state_maxima)
                .map(|(p, value)| (*p, *value)),
            "perfect-information expected payoff",
        )?;
        let evpi = weighted_difference(
            self.state_probs
                .iter()
                .zip(&state_maxima)
                .zip(&self.payoffs)
                .map(|((p, maximum), row)| (*p, *maximum, row[prior_best_action])),
            "perfect-information value",
        )?;
        if evpi < 0.0 {
            return Err(VoiError::Arithmetic {
                quantity: "nonnegative perfect-information value",
            });
        }
        Ok(ValidatedVoiProblem {
            problem: self,
            prior_action_values,
            prior_best_actions,
            state_maxima,
            perfect_info_expected_value,
            evpi,
        })
    }

    /// Computes the prior optimal action, its expected payoff and EVPI. Invalid models refuse.
    pub fn compute_evpi(&self) -> Result<(usize, f64, f64), VoiError> {
        let checked = self.validate()?;
        let action = *checked
            .prior_best_actions
            .first()
            .ok_or(VoiError::EmptyActions)?;
        Ok((action, checked.prior_action_values[action], checked.evpi))
    }

    /// Compatibility summary over the same validated, fully reported experiment calculation.
    pub fn compute_evsi(&self, likelihood: &[Vec<f64>], cost: f64) -> Result<VoiSummary, VoiError> {
        Ok(self.evaluate(VoiExperiment { likelihood, cost })?.summary)
    }

    pub fn evaluate(&self, experiment: VoiExperiment<'_>) -> Result<VoiAnalysis, VoiError> {
        self.validate()?.evaluate(experiment)
    }

    /// [`ValidatedVoiProblem::estimate_evsi_mc`] on this problem, after validating it.
    pub fn estimate_evsi_mc(
        &self,
        likelihood: &[Vec<f64>],
        cost: f64,
        n_samples: usize,
        key: RngKey,
    ) -> Result<McEvsi, VoiError> {
        self.validate()?
            .estimate_evsi_mc(VoiExperiment { likelihood, cost }, n_samples, key)
    }
}

impl ValidatedVoiProblem<'_> {
    pub fn evaluate(&self, experiment: VoiExperiment<'_>) -> Result<VoiAnalysis, VoiError> {
        let VoiExperiment { likelihood, cost } = experiment;
        if !cost.is_finite() || cost < 0.0 {
            return Err(VoiError::InvalidCost { value: cost });
        }
        shape(
            "likelihood",
            None,
            self.problem.state_probs.len(),
            likelihood.len(),
        )?;
        let n_signals = likelihood.first().map_or(0, Vec::len);
        if n_signals == 0 {
            return Err(VoiError::Shape {
                matrix: "likelihood",
                row: Some(0),
                expected: 1,
                actual: 0,
            });
        }
        for (state, row) in likelihood.iter().enumerate() {
            shape("likelihood", Some(state), n_signals, row.len())?;
            distribution(row, "likelihood", Some(state))?;
        }
        let mut signals = Vec::with_capacity(n_signals);
        for signal_index in 0..n_signals {
            let joint = self
                .problem
                .state_probs
                .iter()
                .zip(likelihood)
                .map(|(p, row)| {
                    let joint = p * row[signal_index];
                    if *p > 0.0 && row[signal_index] > 0.0 && joint == 0.0 {
                        return Err(VoiError::Arithmetic {
                            quantity: "joint signal probability (underflow)",
                        });
                    }
                    finite(joint, "joint signal probability")
                })
                .collect::<Result<Vec<_>, _>>()?;
            let probability = sum(joint.iter().copied(), "signal probability")?;
            if probability == 0.0 {
                signals.push(VoiSignalPolicy {
                    signal_index,
                    probability,
                    posterior_state_probs: None,
                    best_actions: Vec::new(),
                    action_expected_payoffs: None,
                    evsi_contribution: 0.0,
                });
                continue;
            }
            if probability > 1.0 {
                return Err(VoiError::Arithmetic {
                    quantity: "signal probability above one",
                });
            }
            let posterior = joint
                .iter()
                .map(|mass| mass / probability)
                .collect::<Vec<_>>();
            let n_actions = self.prior_action_values.len();
            let prior_best = *self
                .prior_best_actions
                .first()
                .ok_or(VoiError::EmptyActions)?;
            let first_likelihood = self
                .problem
                .state_probs
                .iter()
                .zip(likelihood)
                .find(|(p, _)| **p > 0.0)
                .map(|(_, row)| row[signal_index]);
            let uninformative = self
                .problem
                .state_probs
                .iter()
                .zip(likelihood)
                .all(|(p, row)| *p == 0.0 || Some(row[signal_index]) == first_likelihood);
            let best_actions = if uninformative {
                self.prior_best_actions.clone()
            } else {
                optimal_actions(&joint, &self.problem.payoffs, n_actions)?
            };
            let chosen = *best_actions.first().ok_or(VoiError::EmptyActions)?;
            let evsi_contribution = weighted_difference(
                joint
                    .iter()
                    .zip(&self.problem.payoffs)
                    .map(|(mass, row)| (*mass, row[chosen], row[prior_best])),
                "conditional information gain",
            )?;
            if evsi_contribution < 0.0 {
                return Err(VoiError::Arithmetic {
                    quantity: "nonnegative conditional information gain",
                });
            }
            let action_expected_payoffs = (0..n_actions)
                .map(|action| {
                    weighted_sum(
                        posterior
                            .iter()
                            .zip(&self.problem.payoffs)
                            .map(|(p, row)| (*p, row[action])),
                        "posterior expected payoff",
                    )
                })
                .collect::<Result<Vec<_>, _>>()?;
            signals.push(VoiSignalPolicy {
                signal_index,
                probability,
                posterior_state_probs: Some(posterior),
                best_actions,
                action_expected_payoffs: Some(action_expected_payoffs),
                evsi_contribution,
            });
        }
        let direct_gain = sum(
            signals.iter().map(|s| s.evsi_contribution),
            "sample-information value",
        )?;
        let remaining = self.remaining_regret(likelihood, &signals)?;
        // EVSI + remaining perfect-information value = EVPI. Evaluate both forms: the direct gain
        // retains small information values; the regret complement retains fully revealing experiments.
        // This is an independent calculation, never a clamp of an out-of-bound score.
        let scale = self.evpi.max(direct_gain).max(remaining);
        let operations =
            self.problem.state_probs.len() + n_signals + self.prior_action_values.len();
        let tolerance = finite(
            scale * (PROBABILITY_MASS_TOLERANCE + 32.0 * f64::EPSILON * operations as f64),
            "information identity tolerance",
        )?;
        let residual = finite(
            (direct_gain - self.evpi) + remaining,
            "information identity residual",
        )?;
        if residual.abs() > tolerance {
            return Err(VoiError::Arithmetic {
                quantity: "gain plus residual regret must equal EVPI",
            });
        }
        let evsi = if direct_gain <= self.evpi / 2.0 {
            direct_gain
        } else {
            self.evpi - remaining
        };
        if !evsi.is_finite() || evsi < 0.0 || evsi > self.evpi {
            return Err(VoiError::InformationBound {
                evsi,
                evpi: self.evpi,
            });
        }
        let prior_best_action = *self
            .prior_best_actions
            .first()
            .ok_or(VoiError::EmptyActions)?;
        Ok(VoiAnalysis {
            summary: VoiSummary {
                prior_best_action,
                prior_expected_value: self.prior_action_values[prior_best_action],
                perfect_info_expected_value: self.perfect_info_expected_value,
                evpi: self.evpi,
                evsi,
                net_voi: finite(evsi - cost, "net information value")?,
            },
            prior_action_values: self.prior_action_values.clone(),
            prior_best_actions: self.prior_best_actions.clone(),
            signals,
        })
    }

    /// Estimates EVSI by Monte Carlo over the preposterior (plan §22.4), with its standard error.
    ///
    /// Each of `n_samples` draws takes a state `s` from the prior and a signal `z` from `likelihood[s]`, and
    /// scores `u(s, a*(z)) − u(s, a₀)`. Here `a₀` is the prior action and `a*(z)` the action after the signal:
    /// `a₀` whenever it is among [`ValidatedVoiProblem::evaluate`]'s optimal actions for `z`, else the first of
    /// them. Keeping `a₀` on a tie leaves EVSI unchanged and keeps ties out of the Monte Carlo variance. Both
    /// terms use the same draw (common random numbers). The prior term's mean is known, so it is a control
    /// variate with coefficient 1, which keeps the estimate unbiased. The mean of the differences is EVSI in
    /// expectation, and `se` is their sample standard deviation over `√n`.
    ///
    /// The estimate is unbiased only because `a*` is the exact Bayes action, which a finite problem supplies. Any
    /// other decision rule scores at most the Bayes rule's value, so a rule that approximates it gives an estimate
    /// biased low.
    ///
    /// When no signal makes another action strictly better than `a₀`, EVSI is exactly 0. That is checked first, and
    /// nothing is sampled (GB-29). A draw lands on a category by inverse CDF over its positive entries in order; a
    /// row that sums to less than 1, within [`PROBABILITY_MASS_TOLERANCE`], gives the shortfall to its last
    /// positive entry.
    ///
    /// Each `key` names one stream, so a call is a pure function of the problem, the experiment, `n_samples` and
    /// `key`.
    ///
    /// # Errors
    /// Whatever [`ValidatedVoiProblem::evaluate`] refuses, and [`VoiError::TooFewSamples`] for fewer than two
    /// draws.
    pub fn estimate_evsi_mc(
        &self,
        experiment: VoiExperiment<'_>,
        n_samples: usize,
        key: RngKey,
    ) -> Result<McEvsi, VoiError> {
        let analysis = self.evaluate(experiment)?;
        if n_samples < 2 {
            return Err(VoiError::TooFewSamples { draws: n_samples });
        }
        let prior = analysis.summary.prior_best_action;
        // An impossible signal has no optimal actions and is never drawn; it keeps the prior action.
        let rule: Vec<usize> = analysis
            .signals
            .iter()
            .map(|s| {
                s.best_actions.first().map_or(prior, |&first| {
                    if s.best_actions.contains(&prior) {
                        prior
                    } else {
                        first
                    }
                })
            })
            .collect();
        let cost = experiment.cost;
        if rule.iter().all(|&a| a == prior) {
            return Ok(McEvsi {
                prior_best_action: prior,
                evsi: 0.0,
                se: 0.0,
                n_samples: 0,
                net_voi: -cost,
                status: VoiStatus::at_two_se(-cost, 0.0),
                changes_choice: false,
            });
        }
        let (state_probs, payoffs) = (&self.problem.state_probs, &self.problem.payoffs);
        let mut rng = PhiloxStream::new(key);
        // Welford's running mean and sum of squared deviations.
        let mut mean = 0.0_f64;
        let mut m2 = 0.0_f64;
        for i in 0..n_samples {
            let s = draw_index(state_probs, rng.next_f64_open01());
            let z = draw_index(&experiment.likelihood[s], rng.next_f64_open01());
            let d = payoffs[s][rule[z]] - payoffs[s][prior];
            let delta = d - mean;
            mean += delta / (i + 1) as f64;
            m2 += delta * (d - mean);
        }
        let n = n_samples as f64;
        let se = fmine_core::det::sqrt(m2 / (n - 1.0) / n);
        let net_voi = mean - cost;
        Ok(McEvsi {
            prior_best_action: prior,
            evsi: mean,
            se,
            n_samples,
            net_voi,
            status: VoiStatus::at_two_se(net_voi, se),
            changes_choice: true,
        })
    }

    fn remaining_regret(
        &self,
        likelihood: &[Vec<f64>],
        signals: &[VoiSignalPolicy],
    ) -> Result<f64, VoiError> {
        let mut total = Accumulator::default();
        let mut masses = vec![Accumulator::default(); self.prior_action_values.len()];
        for (state, ((p, row), maximum)) in self
            .problem
            .state_probs
            .iter()
            .zip(&self.problem.payoffs)
            .zip(&self.state_maxima)
            .enumerate()
        {
            masses.fill(Accumulator::default());
            for (signal, probability) in signals.iter().zip(&likelihood[state]) {
                if let Some(action) = signal.best_actions.first() {
                    masses[*action].add(*probability, "policy likelihood mass")?;
                }
            }
            // Grouping signals with the same chosen action makes a split of a fully revealing signal
            // retain zero residual regret, even when its separate probability products round differently.
            for (action, mass) in masses.iter().enumerate() {
                let weight = p * mass.value("policy likelihood mass")?;
                total.product(weight, *maximum, "remaining perfect-information value")?;
                total.product(-weight, row[action], "remaining perfect-information value")?;
            }
        }
        let remaining = total.value("remaining perfect-information value")?;
        if remaining < 0.0 {
            return Err(VoiError::Arithmetic {
                quantity: "nonnegative residual regret",
            });
        }
        Ok(remaining)
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

fn finite(value: f64, quantity: &'static str) -> Result<f64, VoiError> {
    if value.is_finite() {
        Ok(value)
    } else {
        Err(VoiError::Arithmetic { quantity })
    }
}

fn sum(values: impl IntoIterator<Item = f64>, quantity: &'static str) -> Result<f64, VoiError> {
    let mut accumulator = Accumulator::default();
    for value in values {
        accumulator.add(value, quantity)?;
    }
    accumulator.value(quantity)
}

#[derive(Debug, Clone, Copy, Default)]
struct Accumulator {
    total: f64,
    correction: f64,
}

impl Accumulator {
    fn add(&mut self, value: f64, quantity: &'static str) -> Result<(), VoiError> {
        finite(value, quantity)?;
        let next = finite(self.total + value, quantity)?;
        let residual = if self.total.abs() >= value.abs() {
            (self.total - next) + value
        } else {
            (value - next) + self.total
        };
        self.correction = finite(self.correction + residual, quantity)?;
        self.total = next;
        Ok(())
    }

    fn product(&mut self, weight: f64, value: f64, quantity: &'static str) -> Result<(), VoiError> {
        let product = finite(weight * value, quantity)?;
        self.add(product, quantity)?;
        self.add(weight.mul_add(value, -product), quantity)
    }

    fn value(&self, quantity: &'static str) -> Result<f64, VoiError> {
        finite(self.total + self.correction, quantity)
    }
}

fn weighted_sum(
    values: impl IntoIterator<Item = (f64, f64)>,
    quantity: &'static str,
) -> Result<f64, VoiError> {
    let mut total = Accumulator::default();
    for (weight, value) in values {
        total.product(weight, value, quantity)?;
    }
    total.value(quantity)
}

fn weighted_difference(
    values: impl IntoIterator<Item = (f64, f64, f64)>,
    quantity: &'static str,
) -> Result<f64, VoiError> {
    let mut total = Accumulator::default();
    for (weight, left, right) in values {
        total.product(weight, left, quantity)?;
        total.product(-weight, right, quantity)?;
    }
    total.value(quantity)
}

fn optimal_actions(
    weights: &[f64],
    payoffs: &[Vec<f64>],
    n_actions: usize,
) -> Result<Vec<usize>, VoiError> {
    let difference = |left: usize, right: usize| {
        weighted_difference(
            weights
                .iter()
                .zip(payoffs)
                .map(|(p, row)| (*p, row[left], row[right])),
            "expected action difference",
        )
    };
    let mut best = 0;
    for action in 1..n_actions {
        if difference(action, best)? > 0.0 {
            best = action;
        }
    }
    let mut ties = Vec::new();
    for action in 0..n_actions {
        let delta = difference(action, best)?;
        if delta > 0.0 {
            return Err(VoiError::Arithmetic {
                quantity: "consistent optimal action ordering",
            });
        }
        if delta == 0.0 {
            ties.push(action);
        }
    }
    Ok(ties)
}

fn shape(
    matrix: &'static str,
    row: Option<usize>,
    expected: usize,
    actual: usize,
) -> Result<(), VoiError> {
    if expected == actual {
        Ok(())
    } else {
        Err(VoiError::Shape {
            matrix,
            row,
            expected,
            actual,
        })
    }
}

fn distribution(values: &[f64], name: &'static str, row: Option<usize>) -> Result<(), VoiError> {
    for (index, value) in values.iter().copied().enumerate() {
        if !value.is_finite() || !(0.0..=1.0).contains(&value) {
            return Err(VoiError::Probability {
                distribution: name,
                row,
                index,
                value,
            });
        }
    }
    let mass = sum(values.iter().copied(), "probability mass")?;
    if (mass - 1.0).abs() > PROBABILITY_MASS_TOLERANCE {
        return Err(VoiError::ProbabilityMass {
            distribution: name,
            row,
            sum: mass,
        });
    }
    Ok(())
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
        let s0 = problem
            .compute_evsi(&uninformative, 2.0)
            .expect("uninformative experiment");
        assert!(s0.evpi >= 0.0);
        assert!(s0.evsi.abs() < 1e-12);

        // Partially informative twin drillhole: P(hit|high)=0.8, P(miss|barren)=0.85.
        let partial = vec![vec![0.80, 0.20], vec![0.15, 0.85]];
        let s1 = problem
            .compute_evsi(&partial, 2.0)
            .expect("partial information");

        // Higher-precision 3-hole fence: P(hit|high)=0.95, P(miss|barren)=0.95.
        let high_prec = vec![vec![0.95, 0.05], vec![0.05, 0.95]];
        let s2 = problem
            .compute_evsi(&high_prec, 5.0)
            .expect("higher precision");

        // Perfect assay signal: P(z=s|s)=1.0 -> EVSI == EVPI.
        let perfect = vec![vec![1.0, 0.0], vec![0.0, 1.0]];
        let s_perf = problem
            .compute_evsi(&perfect, 0.0)
            .expect("perfect information");

        assert!(s0.evsi <= s1.evsi + 1e-12);
        assert!(s1.evsi <= s2.evsi + 1e-12);
        assert!(s2.evsi <= s_perf.evsi + 1e-12);
        assert!((s_perf.evsi - s_perf.evpi).abs() < 1e-12);
    }

    #[test]
    fn malformed_models_and_experiments_refuse_without_a_zero_score() {
        let valid = VoiProblem {
            state_probs: vec![0.5, 0.5],
            payoffs: vec![vec![10.0, 0.0], vec![-10.0, 0.0]],
        };
        for probabilities in [
            vec![],
            vec![0.5],
            vec![0.6, 0.6],
            vec![-0.5, 1.5],
            vec![f64::NAN, 0.5],
        ] {
            assert!(
                VoiProblem {
                    state_probs: probabilities,
                    payoffs: valid.payoffs.clone()
                }
                .validate()
                .is_err()
            );
        }
        for payoffs in [
            vec![],
            vec![vec![]; 2],
            vec![vec![0.0]],
            vec![vec![0.0, 1.0], vec![2.0]],
            vec![vec![f64::INFINITY], vec![0.0]],
        ] {
            assert!(
                VoiProblem {
                    state_probs: valid.state_probs.clone(),
                    payoffs
                }
                .validate()
                .is_err()
            );
        }
        for likelihood in [
            vec![],
            vec![vec![]; 2],
            vec![vec![1.0]],
            vec![vec![1.0], vec![0.5, 0.5]],
            vec![vec![0.8, 0.8]; 2],
            vec![vec![1.1, -0.1]; 2],
            vec![vec![f64::NAN, 0.5]; 2],
        ] {
            assert!(valid.compute_evsi(&likelihood, 0.0).is_err());
        }
        for cost in [-1.0, f64::NAN, f64::INFINITY] {
            assert!(matches!(
                valid.compute_evsi(&[vec![1.0], vec![1.0]], cost),
                Err(VoiError::InvalidCost { .. })
            ));
        }
        let zero_prior = VoiProblem {
            state_probs: vec![1.0, 0.0],
            payoffs: valid.payoffs,
        };
        assert!(
            zero_prior
                .compute_evsi(&[vec![1.0], vec![0.0]], 0.0)
                .is_err(),
            "an impossible state does not license a malformed likelihood row"
        );
    }

    #[test]
    fn no_signal_cannot_create_value_from_large_common_offsets_or_cancellation() {
        for (payoffs, best) in [
            (vec![vec![1e16, 1e16 + 2.0], vec![1e16, 1e16]], 1),
            (
                vec![vec![1e16, -1e16, -1e16 + 2.0], vec![-1e16, 1e16, 1e16]],
                2,
            ),
        ] {
            let problem = VoiProblem {
                state_probs: vec![0.5, 0.5],
                payoffs,
            };
            for likelihood in [vec![vec![1.0]; 2], vec![vec![0.1, 0.9]; 2]] {
                let result = problem
                    .evaluate(VoiExperiment {
                        likelihood: &likelihood,
                        cost: 3.0,
                    })
                    .expect("finite decision");
                assert_eq!(result.prior_best_actions, [best]);
                assert!(result.signals.iter().all(|s| s.best_actions == [best]));
                assert_eq!(result.summary.evsi, 0.0);
                assert_eq!(result.summary.net_voi, -3.0);
            }
        }
    }

    #[test]
    fn splitting_a_fully_revealing_signal_preserves_perfect_information() {
        let problem = VoiProblem {
            state_probs: vec![1.0 / 16.0, 15.0 / 16.0],
            payoffs: vec![vec![0.1, 0.0], vec![-1.0, 0.0]],
        };
        let unsplit = problem
            .compute_evsi(&[vec![1.0, 0.0], vec![0.0, 1.0]], 0.0)
            .expect("perfect signal");
        let split = problem
            .evaluate(VoiExperiment {
                likelihood: &[
                    vec![1.0 / 16.0, 3.0 / 16.0, 0.75, 0.0],
                    vec![0.0, 0.0, 0.0, 1.0],
                ],
                cost: 0.0,
            })
            .expect("same information, split signals");
        assert_eq!(split.summary, unsplit);
        assert_eq!(split.summary.evsi, split.summary.evpi);
        assert_eq!(split.summary.evpi, 0.00625);
        assert_eq!(split.signals[0].best_actions, [0]);
        assert_eq!(split.signals[3].best_actions, [1]);
    }

    #[test]
    fn every_small_discrete_policy_matches_an_independent_exhaustive_oracle() {
        // Three actions, three observations, two states: enumerate all 27 policies for each of
        // 729 integer payoff matrices. Dyadic probabilities make the independent arithmetic exact.
        let likelihood = vec![vec![0.5, 0.25, 0.25], vec![0.25, 0.5, 0.25]];
        for encoding in 0..729 {
            let mut digits = encoding;
            let mut payoffs = vec![vec![0.0; 3]; 2];
            for row in &mut payoffs {
                for payoff in row {
                    *payoff = [-4.0, 0.0, 4.0][digits % 3];
                    digits /= 3;
                }
            }
            let prior = (0..3)
                .map(|a| 0.25 * payoffs[0][a] + 0.75 * payoffs[1][a])
                .fold(f64::NEG_INFINITY, f64::max);
            let mut oracle = f64::NEG_INFINITY;
            for first in 0..3 {
                for second in 0..3 {
                    for third in 0..3 {
                        let mut value = 0.0;
                        for (state, p) in [0.25, 0.75].into_iter().enumerate() {
                            for (signal, action) in [first, second, third].into_iter().enumerate() {
                                value += p * likelihood[state][signal] * payoffs[state][action];
                            }
                        }
                        oracle = oracle.max(value);
                    }
                }
            }
            let problem = VoiProblem {
                state_probs: vec![0.25, 0.75],
                payoffs,
            };
            let report = problem
                .evaluate(VoiExperiment {
                    likelihood: &likelihood,
                    cost: 0.5,
                })
                .expect("finite enumerated case");
            assert_eq!(report.summary.evsi, oracle - prior, "matrix {encoding}");
            assert_eq!(report.summary.net_voi, oracle - prior - 0.5);
            assert!(report.summary.evsi >= 0.0 && report.summary.evsi <= report.summary.evpi);
            let mut policy_value = 0.0;
            for (state, p) in problem.state_probs.iter().enumerate() {
                for signal in &report.signals {
                    policy_value += p
                        * likelihood[state][signal.signal_index]
                        * problem.payoffs[state][signal.best_actions[0]];
                }
            }
            assert_eq!(
                policy_value, oracle,
                "reported policy for matrix {encoding}"
            );
        }
    }

    #[test]
    fn common_payoff_offsets_preserve_decision_value() {
        let likelihood = vec![vec![0.75, 0.25], vec![0.25, 0.75]];
        let base = VoiProblem {
            state_probs: vec![0.5, 0.5],
            payoffs: vec![vec![8.0, 0.0], vec![-8.0, 0.0]],
        };
        let shifted = VoiProblem {
            state_probs: base.state_probs.clone(),
            payoffs: base
                .payoffs
                .iter()
                .map(|row| row.iter().map(|v| v + 1e15).collect())
                .collect(),
        };
        let first = base
            .evaluate(VoiExperiment {
                likelihood: &likelihood,
                cost: 0.0,
            })
            .expect("base");
        let second = shifted
            .evaluate(VoiExperiment {
                likelihood: &likelihood,
                cost: 0.0,
            })
            .expect("offset");
        assert_eq!(first.summary.evpi, 4.0);
        assert_eq!(first.summary.evsi, 2.0);
        assert_eq!(second.summary.evpi, first.summary.evpi);
        assert_eq!(second.summary.evsi, first.summary.evsi);
        assert_eq!(second.prior_best_actions, first.prior_best_actions);
        for (a, b) in first.signals.iter().zip(second.signals) {
            assert_eq!(a.best_actions, b.best_actions);
        }
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
        // A tie keeps the prior action. The prior picks action 1 (0.5 against 0); signal 0 ties actions 0 and 1
        // exactly (dyadic), and signal 1 keeps action 1. Taking the first tied action would sample a "change"
        // worth nothing.
        let tied = VoiProblem {
            state_probs: vec![0.5, 0.5],
            payoffs: vec![vec![2.0, 1.0], vec![-2.0, 0.0]],
        };
        let tied_lik = [vec![0.5, 0.5], vec![0.25, 0.75]];
        let analysis = tied
            .evaluate(VoiExperiment {
                likelihood: &tied_lik,
                cost: 0.0,
            })
            .expect("valid");
        assert_eq!(analysis.summary.prior_best_action, 1);
        assert_eq!(analysis.signals[0].best_actions, [0, 1]);
        assert_eq!(analysis.summary.evsi, 0.0);
        let m = tied
            .estimate_evsi_mc(&tied_lik, 0.0, 1_000, key(0, 0, 0))
            .expect("valid");
        assert!(!m.changes_choice);
        assert_eq!((m.evsi, m.n_samples), (0.0, 0));
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
            Err(VoiError::TooFewSamples { draws: 1 })
        );
        assert_eq!(
            p.estimate_evsi_mc(&lik, 1.0, 0, k),
            Err(VoiError::TooFewSamples { draws: 0 })
        );
        assert_eq!(
            p.estimate_evsi_mc(&lik, -1.0, 100, k),
            Err(VoiError::InvalidCost { value: -1.0 })
        );
        assert!(matches!(
            p.estimate_evsi_mc(&[vec![0.85, 0.25], vec![0.15, 0.85]], 1.0, 100, k),
            Err(VoiError::ProbabilityMass {
                distribution: "likelihood",
                row: Some(0),
                ..
            })
        ));
        assert!(matches!(
            p.estimate_evsi_mc(&lik[..1], 1.0, 100, k),
            Err(VoiError::Shape {
                matrix: "likelihood",
                ..
            })
        ));
        let bad_prior = VoiProblem {
            state_probs: vec![0.6, 0.5],
            payoffs: p.payoffs.clone(),
        };
        assert!(matches!(
            bad_prior.estimate_evsi_mc(&lik, 1.0, 100, k),
            Err(VoiError::ProbabilityMass {
                distribution: "prior",
                ..
            })
        ));
        assert!(!VoiError::TooFewSamples { draws: 1 }.is_numerical_failure());
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
        // A row 1e-13 short of 1 gives the shortfall to its last positive entry.
        assert_eq!(draw_index(&[0.5, 0.5 - 1e-13, 0.0], 1.0 - 1e-16), 1);
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
