//! Original standardized two-sided Page CUSUM (BD-051, plan §10.4); no upstream code copied.
//! Determinism: DeterministicPerIsa (fixed-order finite floating-point recurrences).
//!
//! DESIGN: `upper = max(0, upper + z - k)` and `lower = max(0, lower - z - k)`,
//! both starting at zero; equality to the declared threshold alarms. The reset to zero is part
//! of the recurrence, never a repair of an invalid input. This is a change diagnostic, not an
//! e-process, probability, calibrated false-alarm rate or automatic authorization to retrain.
//! Reference: NIST Dataplot, CUSUM Control Chart, the two standardized recurrences:
//! <https://itl.nist.gov/div898/software/dataplot/refman1/auxillar/cusum.htm>.

/// DESIGN: bounds replay work and keeps the count exactly representable in f64 if rendered there.
pub const MAX_OBSERVATIONS: u64 = 1_000_000;

/// A rejected construction or update; rejection never changes the process state.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CusumError {
    ReferenceValueOutOfDomain(f64),
    ThresholdOutOfDomain(f64),
    ObservationOutOfDomain(f64),
    ObservationLimit,
    NumericalFailure,
}

impl std::fmt::Display for CusumError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ReferenceValueOutOfDomain(k) => {
                write!(
                    f,
                    "CUSUM reference value must be finite and positive, got {k}"
                )
            }
            Self::ThresholdOutOfDomain(h) => {
                write!(f, "CUSUM threshold must be finite and positive, got {h}")
            }
            Self::ObservationOutOfDomain(z) => {
                write!(f, "CUSUM standardized residual must be finite, got {z}")
            }
            Self::ObservationLimit => {
                write!(f, "CUSUM permits at most {MAX_OBSERVATIONS} observations")
            }
            Self::NumericalFailure => write!(f, "CUSUM recurrence is not representable"),
        }
    }
}

impl std::error::Error for CusumError {}

/// Current evidence, directional running maxima and sticky first crossings.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CusumSnapshot {
    pub n: u64,
    pub upper: f64,
    pub lower: f64,
    pub max_upper: f64,
    pub max_lower: f64,
    pub threshold: f64,
    pub first_upper_crossing: Option<u64>,
    pub first_lower_crossing: Option<u64>,
}

impl CusumSnapshot {
    #[must_use]
    pub fn alarmed(&self) -> bool {
        self.first_crossing().is_some()
    }

    /// Earliest one-based crossing in either direction. A later return to zero never withdraws it.
    #[must_use]
    pub fn first_crossing(&self) -> Option<u64> {
        match (self.first_upper_crossing, self.first_lower_crossing) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (Some(a), None) | (None, Some(a)) => Some(a),
            (None, None) => None,
        }
    }
}

/// O(1) state and work per standardized residual. The caller freezes `k`, `h`, support and
/// order prospectively and supplies the reference/tuning assumptions. No head start or rearming
/// is inferred, and no average-run-length guarantee follows from these parameters alone.
#[derive(Debug, Clone, PartialEq)]
pub struct TwoSidedCusum {
    reference_value: f64,
    state: CusumSnapshot,
}

impl TwoSidedCusum {
    pub fn new(reference_value: f64, threshold: f64) -> Result<Self, CusumError> {
        if !reference_value.is_finite() || reference_value <= 0.0 {
            return Err(CusumError::ReferenceValueOutOfDomain(reference_value));
        }
        if !threshold.is_finite() || threshold <= 0.0 {
            return Err(CusumError::ThresholdOutOfDomain(threshold));
        }
        Ok(Self {
            reference_value,
            state: CusumSnapshot {
                n: 0,
                upper: 0.0,
                lower: 0.0,
                max_upper: 0.0,
                max_lower: 0.0,
                threshold,
                first_upper_crossing: None,
                first_lower_crossing: None,
            },
        })
    }

    #[must_use]
    pub fn snapshot(&self) -> CusumSnapshot {
        self.state
    }

    /// Append one finite standardized residual. Both directional updates succeed before state changes.
    pub fn observe(&mut self, z: f64) -> Result<CusumSnapshot, CusumError> {
        if !z.is_finite() {
            return Err(CusumError::ObservationOutOfDomain(z));
        }
        if self.state.n >= MAX_OBSERVATIONS {
            return Err(CusumError::ObservationLimit);
        }
        let upper = self.state.upper + z - self.reference_value;
        let lower = self.state.lower - z - self.reference_value;
        if !upper.is_finite() || !lower.is_finite() {
            return Err(CusumError::NumericalFailure);
        }
        let upper = upper.max(0.0);
        let lower = lower.max(0.0);
        let n = self.state.n + 1;
        self.state = CusumSnapshot {
            n,
            upper,
            lower,
            max_upper: self.state.max_upper.max(upper),
            max_lower: self.state.max_lower.max(lower),
            threshold: self.state.threshold,
            first_upper_crossing: self
                .state
                .first_upper_crossing
                .or_else(|| (upper >= self.state.threshold).then_some(n)),
            first_lower_crossing: self
                .state
                .first_lower_crossing
                .or_else(|| (lower >= self.state.threshold).then_some(n)),
        };
        Ok(self.state)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn independent_all_suffix_sums_recover_every_prefix_and_direction() {
        // INFERENCE: the Page recurrence equals max(0, every suffix sum of z-k), independently
        // evaluated with integer half-units. This catches accidental resets and sign reversals.
        let twice_z: [i64; 12] = [0, 1, 3, 4, 1, -3, -5, -4, 0, 2, 5, -1];
        let mut process = TwoSidedCusum::new(0.5, 2.0).unwrap();
        let mut max_upper = 0;
        let mut max_lower = 0;
        let mut first_upper = None;
        let mut first_lower = None;
        for n in 1..=twice_z.len() {
            let suffix = |direction: i64| -> i64 {
                (0..n)
                    .map(|start| {
                        twice_z[start..n]
                            .iter()
                            .map(|z| direction * z - 1)
                            .sum::<i64>()
                    })
                    .max()
                    .unwrap_or(0)
                    .max(0)
            };
            let upper: i64 = suffix(1);
            let lower: i64 = suffix(-1);
            max_upper = max_upper.max(upper);
            max_lower = max_lower.max(lower);
            first_upper = first_upper.or_else(|| (upper >= 4).then_some(n as u64));
            first_lower = first_lower.or_else(|| (lower >= 4).then_some(n as u64));
            let actual = process.observe(twice_z[n - 1] as f64 / 2.0).unwrap();
            assert_eq!(actual.n, n as u64);
            assert_eq!(
                (actual.upper, actual.lower),
                (upper as f64 / 2.0, lower as f64 / 2.0)
            );
            assert_eq!(
                (actual.max_upper, actual.max_lower),
                (max_upper as f64 / 2.0, max_lower as f64 / 2.0)
            );
            assert_eq!(
                (actual.first_upper_crossing, actual.first_lower_crossing),
                (first_upper, first_lower)
            );
        }
    }

    #[test]
    fn threshold_equality_and_recovery_preserve_first_crossings() {
        let mut process = TwoSidedCusum::new(0.5, 2.0).unwrap();
        assert!(!process.snapshot().alarmed());
        assert!(!process.observe(1.5).unwrap().alarmed());
        let high = process.observe(1.5).unwrap();
        assert_eq!((high.upper, high.first_upper_crossing), (2.0, Some(2)));
        for _ in 0..4 {
            process.observe(0.0).unwrap();
        }
        let recovered = process.snapshot();
        assert_eq!(
            (recovered.upper, recovered.lower, recovered.max_upper),
            (0.0, 0.0, 2.0)
        );
        assert_eq!(recovered.first_crossing(), Some(2));
        process.observe(-2.5).unwrap();
        let low = process.snapshot();
        assert_eq!(
            (low.first_upper_crossing, low.first_lower_crossing),
            (Some(2), Some(7))
        );
        assert_eq!(low.first_crossing(), Some(2));
    }

    #[test]
    fn invalid_inputs_overflow_and_work_limit_leave_state_unchanged() {
        for invalid in [0.0, -1.0, f64::INFINITY, f64::NEG_INFINITY, f64::NAN] {
            assert!(TwoSidedCusum::new(invalid, 2.0).is_err());
            assert!(TwoSidedCusum::new(0.5, invalid).is_err());
        }
        let mut process = TwoSidedCusum::new(0.5, 2.0).unwrap();
        for invalid in [f64::INFINITY, f64::NEG_INFINITY, f64::NAN] {
            let before = process.clone();
            assert!(process.observe(invalid).is_err());
            assert_eq!(process, before);
        }
        process.observe(f64::MAX).unwrap();
        let before = process.clone();
        assert_eq!(process.observe(f64::MAX), Err(CusumError::NumericalFailure));
        assert_eq!(process, before);
        process.state.n = MAX_OBSERVATIONS;
        let before = process.clone();
        assert_eq!(process.observe(0.0), Err(CusumError::ObservationLimit));
        assert_eq!(process, before);
    }
}
