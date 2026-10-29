//! Provenance: frankensim crates/fs-eproc/src/lib.rs @ 20463525d9775a18c1b237d2676f364f367175ab;
//! license: MIT/Apache-2.0. Anytime-valid betting martingale e-processes (`BD-022`, `GB-08`).

/// Anytime-valid betting martingale e-process for testing $H_0: \mathbb{E}[Z_t] \le \mu_0$
/// on bounded or PIT-transformed sequential observations $u_t \in [0, 1]$ (`GB-08`).
///
/// Uses predictable Kelly / GRAPA-style betting fraction $\lambda_t \in (-0.95, 0.95)$
/// so that each multiplicative factor $1 + \lambda_t (u_t - \mu_0) > 0$ and $\mathbb{E}_0[E_t] = 1$.
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
    /// Creates a new `BettingEProcess` testing `E[U_t] == null_mean` at significance level `alpha`.
    #[must_use]
    pub fn new(null_mean: f64, alpha: f64) -> Self {
        Self {
            null_mean: null_mean.clamp(0.01, 0.99),
            alpha: alpha.clamp(1e-6, 0.5),
            wealth: 1.0,
            max_wealth: 1.0,
            n_steps: 0,
            running_diff_sum: 0.0,
            running_var_sum: 0.08333333333333333, // Uniform(0,1) prior variance 1/12
        }
    }

    /// Rejection threshold $1 / \alpha$ guaranteed by Ville's inequality.
    #[must_use]
    pub fn threshold(&self) -> f64 {
        1.0 / self.alpha
    }

    /// Updates the e-process with a new observation `u` in `[0.0, 1.0]` and returns the updated wealth $E_t$.
    pub fn observe(&mut self, u: f64) -> f64 {
        let u_clamped = u.clamp(0.0, 1.0);
        let diff = u_clamped - self.null_mean;

        // Predictable bet fraction determined strictly from t-1 history.
        let lambda = (self.running_diff_sum / (self.running_var_sum + 1.0)).clamp(-0.75, 0.75);
        let factor = (1.0 + lambda * diff).max(1e-6);

        self.wealth *= factor;
        if self.wealth > self.max_wealth {
            self.max_wealth = self.wealth;
        }
        self.n_steps += 1;
        self.running_diff_sum += diff;
        self.running_var_sum += diff * diff;
        self.wealth
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
            let mut ep = BettingEProcess::new(0.5, alpha);
            for _ in 0..horizon {
                let u = rng.next_f64_open01();
                ep.observe(u);
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
        let mut ep_alt = BettingEProcess::new(0.5, 0.01);
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
            ep_alt.observe(u);
        }
        assert!(ep_alt.rejected_null(), "must reject under shifted mean 0.75");
    }
}
