//! Original FrankenMining code (not lifted). Upstream analogue: frankensim crates/fs-voi/src/lib.rs @ 20463525d (not copied).
//! Expected Value of Perfect and Sample Information (`BD-022`, `BD-225`, `GATE-HELM-02`).
//! Determinism: DeterministicPerIsa (no cross-host digest test; plan §29.2, bd-hp2j.1).

use std::fmt;

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
}
