//! Original FrankenMining code (not lifted). Upstream analogue: frankensim crates/fs-eproc/src/lib.rs @ 20463525d (not copied).
//! Anytime-valid betting martingale e-processes (`BD-022`, `GB-08`).
//! Determinism: DeterministicPerIsa (no cross-host digest test; plan §29.2, bd-hp2j.1).

/// Why a [`BettingEProcess`] refused (bd-a482). Nothing is clamped into range.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EProcError {
    /// `null_mean` is not a finite value in `(0, 1)`.
    NullMeanOutOfDomain(f64),
    /// `alpha` is not a finite value in `(0, 1)`.
    AlphaOutOfDomain(f64),
    /// An observation is not a finite value in `[0, 1]`.
    ObservationOutOfDomain(f64),
}

impl std::fmt::Display for EProcError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NullMeanOutOfDomain(m) => write!(f, "null mean must lie in (0, 1), got {m}"),
            Self::AlphaOutOfDomain(a) => write!(f, "alpha must lie in (0, 1), got {a}"),
            Self::ObservationOutOfDomain(u) => {
                write!(f, "an observation must lie in [0, 1], got {u}")
            }
        }
    }
}

impl std::error::Error for EProcError {}

/// Anytime-valid betting martingale e-process for testing $H_0: \mathbb{E}[U_t \mid U_{<t}] = \mu_0$
/// on bounded or PIT-transformed sequential observations $u_t \in [0, 1]$ (`GB-08`).
///
/// The bet fraction $\lambda_t$ is predictable (it reads only $u_{<t}$) and is held to $[-0.75, 0.75]$, so each
/// factor $1 + \lambda_t (u_t - \mu_0) \ge 0.25$ and $\mathbb{E}_0[E_t] = 1$. Its sign follows the running mean,
/// so the null is two-sided.
#[derive(Debug, Clone, PartialEq)]
pub struct BettingEProcess {
    pub null_mean: f64,
    pub alpha: f64,
    pub wealth: f64,
    pub max_wealth: f64,
    pub n_steps: u64,
    running_diff_sum: f64,
    running_var_sum: f64,
}

impl BettingEProcess {
    /// A `BettingEProcess` testing `E[U_t] = null_mean` at significance level `alpha`.
    ///
    /// # Errors
    /// [`EProcError::NullMeanOutOfDomain`] or [`EProcError::AlphaOutOfDomain`] for a value outside `(0, 1)` or
    /// not finite.
    pub fn new(null_mean: f64, alpha: f64) -> Result<Self, EProcError> {
        if !(null_mean.is_finite() && null_mean > 0.0 && null_mean < 1.0) {
            return Err(EProcError::NullMeanOutOfDomain(null_mean));
        }
        if !(alpha.is_finite() && alpha > 0.0 && alpha < 1.0) {
            return Err(EProcError::AlphaOutOfDomain(alpha));
        }
        Ok(Self {
            null_mean,
            alpha,
            wealth: 1.0,
            max_wealth: 1.0,
            n_steps: 0,
            running_diff_sum: 0.0,
            running_var_sum: 0.08333333333333333, // Uniform(0,1) prior variance 1/12
        })
    }

    /// Rejection threshold $1 / \alpha$ guaranteed by Ville's inequality.
    #[must_use]
    pub fn threshold(&self) -> f64 {
        1.0 / self.alpha
    }

    /// Updates the e-process with an observation `u` and returns the updated wealth $E_t$.
    ///
    /// # Errors
    /// [`EProcError::ObservationOutOfDomain`] for `u` outside `[0, 1]` or not finite. The state is unchanged.
    pub fn observe(&mut self, u: f64) -> Result<f64, EProcError> {
        if !(0.0..=1.0).contains(&u) {
            return Err(EProcError::ObservationOutOfDomain(u));
        }
        let diff = u - self.null_mean;

        // Predictable bet fraction, from the history before `u`. The bound is part of the betting rule, not a
        // repair of an input: it keeps every factor at or above 0.25, since |diff| < 1.
        let lambda = (self.running_diff_sum / (self.running_var_sum + 1.0)).clamp(-0.75, 0.75);
        let factor = 1.0 + lambda * diff;

        self.wealth *= factor;
        if self.wealth > self.max_wealth {
            self.max_wealth = self.wealth;
        }
        self.n_steps += 1;
        self.running_diff_sum += diff;
        self.running_var_sum += diff * diff;
        Ok(self.wealth)
    }

    /// Returns `true` if the running maximum wealth has crossed Ville's threshold $1 / \alpha$.
    #[must_use]
    pub fn rejected_null(&self) -> bool {
        self.max_wealth >= self.threshold()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fmine_core::rand::{PhiloxStream, RngKey};

    #[test]
    fn test_ville_inequality_type1_error_bound() {
        let alpha = 0.05;
        let n_trials = 500;
        let horizon = 200;
        let mut false_positives = 0;

        for trial in 0..n_trials {
            let mut rng = PhiloxStream::new(RngKey {
                seed_root: 1001,
                partition: 1,
                domain: 1,
                var_group: 1,
                realization: trial as u32,
                brick: 0,
            });
            let mut ep = BettingEProcess::new(0.5, alpha).expect("valid parameters");
            for _ in 0..horizon {
                let u = rng.next_f64_open01();
                ep.observe(u).expect("u in (0, 1)");
            }
            if ep.rejected_null() {
                false_positives += 1;
            }
        }
        let empirical_fpr = (false_positives as f64) / (n_trials as f64);
        assert!(
            empirical_fpr <= alpha + 0.02,
            "Ville Type-I error bound violated: empirical FPR {empirical_fpr} > {alpha}"
        );

        // Under alternative E[U] ~ 0.75, e-process must reject decisively.
        let mut ep_alt = BettingEProcess::new(0.5, 0.01).expect("valid parameters");
        let mut rng_alt = PhiloxStream::new(RngKey {
            seed_root: 42,
            partition: 2,
            domain: 1,
            var_group: 1,
            realization: 0,
            brick: 0,
        });
        for _ in 0..150 {
            let u = 0.5 + 0.5 * rng_alt.next_f64_open01();
            ep_alt.observe(u).expect("u in (0.5, 1)");
        }
        assert!(
            ep_alt.rejected_null(),
            "must reject under shifted mean 0.75"
        );
    }

    #[test]
    fn test_eproc_refuses_out_of_domain_parameters_and_observations() {
        for m in [0.0, 1.0, -0.1, 1.5, f64::NAN, f64::INFINITY] {
            assert!(
                matches!(
                    BettingEProcess::new(m, 0.05),
                    Err(EProcError::NullMeanOutOfDomain(_))
                ),
                "null mean {m}"
            );
        }
        for a in [0.0, 1.0, -0.01, 2.0, f64::NAN, f64::NEG_INFINITY] {
            assert!(
                matches!(
                    BettingEProcess::new(0.5, a),
                    Err(EProcError::AlphaOutOfDomain(_))
                ),
                "alpha {a}"
            );
        }
        // Before bd-a482, alpha 0.9 became 0.5 and a null mean of 0.995 became 0.99; both now stand as given.
        let ep = BettingEProcess::new(0.995, 0.9).expect("valid parameters");
        assert_eq!((ep.null_mean, ep.alpha), (0.995, 0.9));

        let mut ep = BettingEProcess::new(0.5, 0.05).expect("valid parameters");
        ep.observe(0.9).expect("in range");
        let before = ep.clone();
        for u in [1.3, -0.1, f64::NAN, f64::INFINITY] {
            match ep.observe(u) {
                Err(EProcError::ObservationOutOfDomain(v)) => {
                    assert!(v == u || (v.is_nan() && u.is_nan()), "{v} vs {u}");
                }
                other => panic!("observation {u} must refuse, got {other:?}"),
            }
            assert_eq!(ep, before, "a refused observation must change nothing");
        }
        // The endpoints are observations, not errors.
        ep.observe(0.0).expect("0 is in [0, 1]");
        ep.observe(1.0).expect("1 is in [0, 1]");
    }
}
