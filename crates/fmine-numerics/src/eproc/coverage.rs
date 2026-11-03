//! Original Bernoulli coverage mixture (BD-051, plan §10.4, GB-08); no upstream code copied.
//! Determinism: DeterministicPerIsa (fixed-order logarithms through `fmine_core::det`).
//!
//! DESIGN: a uniform Beta(1,1) mixture of Bernoulli likelihood ratios tests the point null
//! `P(hit_t | past) = p`. It is two-sided: both excess misses and excess hits can reject. It does
//! not test the composite null "coverage is at least p". The caller freezes the null, significance,
//! support and observation order before outcomes and establishes the conditional-null assumption.
//! Marginal coverage, an effective sample size or a correlation sensitivity range alone is insufficient.

use fmine_core::det;

/// DESIGN: bounds the admitted sequence and keeps all count-to-f64 conversions exact.
pub const MAX_OBSERVATIONS: u64 = 1_000_000;

/// A rejected construction or update. A rejected update never changes the process.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CoverageError {
    NullProbabilityOutOfDomain(f64),
    AlarmAlphaOutOfDomain(f64),
    ObservationLimit,
    NumericalFailure,
}

impl std::fmt::Display for CoverageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NullProbabilityOutOfDomain(p) => {
                write!(
                    f,
                    "coverage null probability must be finite and in (0, 1), got {p}"
                )
            }
            Self::AlarmAlphaOutOfDomain(a) => {
                write!(
                    f,
                    "coverage alarm alpha must be finite and in (0, 1), got {a}"
                )
            }
            Self::ObservationLimit => {
                write!(
                    f,
                    "coverage process permits at most {MAX_OBSERVATIONS} observations"
                )
            }
            Self::NumericalFailure => write!(f, "coverage log evidence is not representable"),
        }
    }
}

impl std::error::Error for CoverageError {}

/// Evidence at one prefix, including the initial prefix of length zero.
///
/// Logarithms are authoritative and finite for every returned snapshot. Exponentials are display
/// conveniences: an unrepresentable positive value is `None`, never infinity or an underflowed zero.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CoverageSnapshot {
    pub n: u64,
    pub hits: u64,
    pub log_e_value: f64,
    /// Includes initial evidence 1, so its logarithm is never negative.
    pub max_log_e_value: f64,
    pub log_threshold: f64,
    /// The first represented log-threshold crossing's one-based observation index, never withdrawn.
    /// Rounding can affect timing at a mathematical equality; this is not an exact certificate.
    pub first_crossing: Option<u64>,
}

impl CoverageSnapshot {
    #[must_use]
    pub fn alarmed(&self) -> bool {
        self.first_crossing.is_some()
    }

    #[must_use]
    pub fn e_value(&self) -> Option<f64> {
        finite_positive_exp(self.log_e_value)
    }

    #[must_use]
    pub fn max_e_value(&self) -> Option<f64> {
        finite_positive_exp(self.max_log_e_value)
    }

    #[must_use]
    pub fn threshold(&self) -> Option<f64> {
        finite_positive_exp(self.log_threshold)
    }
}

fn finite_positive_exp(log_value: f64) -> Option<f64> {
    let value = det::exp(log_value);
    (value.is_finite() && value > 0.0).then_some(value)
}

/// A constant-space mixture likelihood-ratio process for binary interval hits.
///
/// INFERENCE from the beta integral: after `n` observations and `k` hits,
/// `E_n = B(k+1,n-k+1) / (p^k (1-p)^(n-k))`. Its next factor is the Beta(1,1)
/// posterior predictive probability of the observation divided by its probability under the null.
/// Under the declared conditional point null the factor has conditional expectation one, so Ville's
/// inequality bounds the probability of ever crossing `1/alarm_alpha` by `alarm_alpha`.
/// See plan §10.4 GB-08 and Howard et al. (2021), DOI:10.1214/20-AOS1991, mixture martingales.
///
/// DESIGN: O(1) time and memory per observation; compensated log accumulation avoids wealth overflow
/// and reduces summation error. This is floating-point evidence, not an exact-arithmetic certificate.
#[derive(Debug, Clone, PartialEq)]
pub struct BernoulliCoverageProcess {
    log_hit: f64,
    log_miss: f64,
    compensation: f64,
    state: CoverageSnapshot,
}

impl BernoulliCoverageProcess {
    pub fn new(null_probability: f64, alarm_alpha: f64) -> Result<Self, CoverageError> {
        if !(null_probability.is_finite() && null_probability > 0.0 && null_probability < 1.0) {
            return Err(CoverageError::NullProbabilityOutOfDomain(null_probability));
        }
        if !(alarm_alpha.is_finite() && alarm_alpha > 0.0 && alarm_alpha < 1.0) {
            return Err(CoverageError::AlarmAlphaOutOfDomain(alarm_alpha));
        }
        // Correct the rounded subtraction in ln(1-p), including p too small to change 1.0.
        let complement = 1.0 - null_probability;
        let log_miss = if complement == 1.0 {
            -null_probability
        } else {
            det::ln(complement) * (-null_probability / (complement - 1.0))
        };
        let log_hit = det::ln(null_probability);
        let log_threshold = -det::ln(alarm_alpha);
        if !(log_hit.is_finite() && log_miss.is_finite() && log_threshold.is_finite()) {
            return Err(CoverageError::NumericalFailure);
        }
        Ok(Self {
            log_hit,
            log_miss,
            compensation: 0.0,
            state: CoverageSnapshot {
                n: 0,
                hits: 0,
                log_e_value: 0.0,
                max_log_e_value: 0.0,
                log_threshold,
                first_crossing: None,
            },
        })
    }

    #[must_use]
    pub fn snapshot(&self) -> CoverageSnapshot {
        self.state
    }

    /// Append one hit or miss. No missing observation can be silently passed as a miss.
    pub fn observe(&mut self, hit: bool) -> Result<CoverageSnapshot, CoverageError> {
        if self.state.n >= MAX_OBSERVATIONS {
            return Err(CoverageError::ObservationLimit);
        }
        let previous_count = if hit {
            self.state.hits
        } else {
            self.state.n - self.state.hits
        };
        let predictive = (previous_count + 1) as f64 / (self.state.n + 2) as f64;
        let log_null = if hit { self.log_hit } else { self.log_miss };
        let corrected = (det::ln(predictive) - log_null) - self.compensation;
        let log_e_value = self.state.log_e_value + corrected;
        let compensation = (log_e_value - self.state.log_e_value) - corrected;
        if !(log_e_value.is_finite() && compensation.is_finite()) {
            return Err(CoverageError::NumericalFailure);
        }
        let n = self.state.n + 1;
        let first_crossing = self
            .state
            .first_crossing
            .or_else(|| (log_e_value >= self.state.log_threshold).then_some(n));
        self.state = CoverageSnapshot {
            n,
            hits: self.state.hits + u64::from(hit),
            log_e_value,
            max_log_e_value: self.state.max_log_e_value.max(log_e_value),
            log_threshold: self.state.log_threshold,
            first_crossing,
        };
        self.compensation = compensation;
        Ok(self.state)
    }
}

#[cfg(test)]
mod tests;
