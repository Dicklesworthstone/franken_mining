//! DESIGN (BD-048): univariate uniform empirical forecasts, without a Gaussian approximation.
//! Sorted scratch values determine scores; callers retain the original draw order in frozen records.
//! This module supplies no shared realization-space, weighting, or calibration claim.
//! Determinism: DeterministicPerIsa (no cross-host empirical-score digest has been established).

use crate::ScoreError;

/// Independent of the journal adapter's encoded-operation byte limit.
pub const MAX_EMPIRICAL_SAMPLES: usize = 4096;

/// CRPS of the uniform empirical distribution, including repeated or degenerate samples.
///
/// DESIGN: integrate the squared ECDF error over positive sorted gaps in O(n log n) time and O(n)
/// scratch space. This avoids subtracting two large absolute-deviation expectations. Invalid sample
/// counts, nonfinite inputs and unrepresentable score arithmetic refuse; no score is clamped.
pub fn empirical_crps(samples: &[f64], outcome: f64) -> Result<f64, ScoreError> {
    SortedEmpirical::new(samples)?.crps(outcome)
}

/// The central `(1 - alpha)` interval of the uniform empirical distribution.
///
/// DESIGN: endpoints use the inverse ECDF at `alpha / 2` and `1 - alpha / 2`, without interpolation.
/// A quantile is the first sorted sample whose computed f64 CDF fraction `k / n` is at least the
/// requested level. Equality selects that sample, including exact CDF jumps; levels rounded to zero
/// or one select the minimum or maximum. Comparing CDF fractions avoids a rounded `ceil(n * p)`.
pub fn empirical_interval(samples: &[f64], alpha: f64) -> Result<(f64, f64), ScoreError> {
    SortedEmpirical::new(samples)?.interval(alpha)
}

/// One sort can serve an assessment's interval, score and unresolved-outcome bounds.
pub(crate) struct SortedEmpirical {
    values: Vec<f64>,
}

impl SortedEmpirical {
    pub(crate) fn new(samples: &[f64]) -> Result<Self, ScoreError> {
        if samples.is_empty() || samples.len() > MAX_EMPIRICAL_SAMPLES {
            return Err(ScoreError::InvalidParameter {
                name: "empirical_sample_count",
                value: samples.len() as f64,
            });
        }
        if samples.iter().any(|value| !value.is_finite()) {
            return Err(ScoreError::NonFinite);
        }
        let mut values = samples.to_vec();
        values.sort_by(f64::total_cmp);
        Ok(Self { values })
    }

    fn quantile(&self, level: f64) -> f64 {
        let mut lo = 0;
        let mut hi = self.values.len();
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            if (mid + 1) as f64 / (self.values.len() as f64) < level {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        // Callers supply a level in [0, 1], and the last CDF fraction is exactly one.
        self.values[lo]
    }

    pub(crate) fn interval(&self, alpha: f64) -> Result<(f64, f64), ScoreError> {
        if !alpha.is_finite() {
            return Err(ScoreError::NonFinite);
        }
        if !(alpha > 0.0 && alpha < 1.0) {
            return Err(ScoreError::AlphaOutOfDomain(alpha));
        }
        Ok((self.quantile(alpha / 2.0), self.quantile(1.0 - alpha / 2.0)))
    }

    pub(crate) fn crps(&self, outcome: f64) -> Result<f64, ScoreError> {
        if !outcome.is_finite() {
            return Err(ScoreError::NonFinite);
        }
        let mut score = PositiveSum::default();
        let first = self.values[0];
        let last = self.values[self.values.len() - 1];
        if outcome < first {
            score.add(weighted_gap(outcome, first, 1.0)?)?;
        } else if outcome > last {
            score.add(weighted_gap(last, outcome, 1.0)?)?;
        }
        let n = self.values.len();
        for (index, pair) in self.values.windows(2).enumerate() {
            let (left, right) = (pair[0], pair[1]);
            let below = (index + 1) as f64 / n as f64;
            let above = (n - index - 1) as f64 / n as f64;
            if outcome > left {
                score.add(weighted_gap(left, right.min(outcome), below * below)?)?;
            }
            if outcome < right {
                score.add(weighted_gap(left.max(outcome), right, above * above)?)?;
            }
        }
        score.finish()
    }

    pub(crate) fn bounds(
        &self,
        domain: Option<(f64, f64)>,
    ) -> Result<(f64, Option<f64>), ScoreError> {
        // CRPS is convex in the outcome and minimized at an empirical median. Over a closed
        // declared domain, its minimum is the median or the nearer endpoint; its maximum is at
        // an endpoint. This selects an extremizer, never repairs a supplied outcome.
        let median = self.quantile(0.5);
        if let Some((lo, hi)) = domain {
            let minimizer = if median < lo {
                lo
            } else if median > hi {
                hi
            } else {
                median
            };
            Ok((
                self.crps(minimizer)?,
                Some(self.crps(lo)?.max(self.crps(hi)?)),
            ))
        } else {
            Ok((self.crps(median)?, None))
        }
    }
}

fn arithmetic(quantity: &'static str) -> ScoreError {
    ScoreError::Arithmetic { quantity }
}

fn weighted_gap(left: f64, right: f64, weight: f64) -> Result<f64, ScoreError> {
    if left == right || weight == 0.0 {
        return Ok(0.0);
    }
    let gap = right - left;
    let term = if gap.is_finite() {
        gap * weight
    } else {
        // A cross-zero gap can exceed f64::MAX while its weighted contribution is representable.
        // Scaling each endpoint first leaves a sum of nonnegative magnitudes in that case.
        right * weight - left * weight
    };
    if !term.is_finite() || term <= 0.0 {
        Err(arithmetic("positive empirical CRPS gap"))
    } else {
        Ok(term)
    }
}

#[derive(Default)]
struct PositiveSum {
    sum: f64,
    correction: f64,
}

impl PositiveSum {
    fn add(&mut self, value: f64) -> Result<(), ScoreError> {
        let next = self.sum + value;
        if !next.is_finite() {
            return Err(arithmetic("empirical CRPS sum"));
        }
        let residual = if self.sum >= value {
            (self.sum - next) + value
        } else {
            (value - next) + self.sum
        };
        self.correction += residual;
        if !self.correction.is_finite() {
            return Err(arithmetic("empirical CRPS compensation"));
        }
        self.sum = next;
        Ok(())
    }

    fn finish(self) -> Result<f64, ScoreError> {
        let score = self.sum + self.correction;
        if score.is_finite() && score >= 0.0 {
            Ok(score)
        } else {
            Err(arithmetic("empirical CRPS"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn quadratic_oracle(samples: &[f64], y: f64) -> f64 {
        let n = samples.len() as f64;
        let absolute = samples.iter().map(|x| (x - y).abs()).sum::<f64>() / n;
        let pairs = samples
            .iter()
            .flat_map(|x| samples.iter().map(move |z| (x - z).abs()))
            .sum::<f64>();
        absolute - pairs / (2.0 * n * n)
    }

    #[test]
    fn empirical_crps_matches_independent_pairwise_known_answers() {
        assert_eq!(empirical_crps(&[2.0], 5.0).unwrap(), 3.0);
        assert_eq!(empirical_crps(&[2.0, 2.0, 2.0], 2.0).unwrap(), 0.0);
        assert_eq!(empirical_crps(&[0.0, 2.0], 1.0).unwrap(), 0.5);
        assert_eq!(empirical_crps(&[0.0, 2.0], 3.0).unwrap(), 1.5);
        let values = [-1.0, 0.0, 2.0];
        for code in 0..81 {
            let mut digits = code;
            let samples: Vec<_> = (0..4)
                .map(|_| {
                    let value = values[digits % 3];
                    digits /= 3;
                    value
                })
                .collect();
            for outcome in [-2.0, -0.5, 0.0, 1.0, 3.0] {
                let got = empirical_crps(&samples, outcome).unwrap();
                assert_eq!(got, quadratic_oracle(&samples, outcome));
            }
        }
    }

    #[test]
    fn empirical_scores_and_intervals_preserve_permutations_and_uniform_replication() {
        let original = [4.0, -2.0, 1.0, 1.0, 10.0];
        let reordered = [1.0, 10.0, 1.0, -2.0, 4.0];
        let replicated = original.repeat(2);
        for values in [&reordered[..], &replicated] {
            assert_eq!(
                empirical_crps(values, 3.0).unwrap().to_bits(),
                empirical_crps(&original, 3.0).unwrap().to_bits()
            );
            for alpha in [0.1, 0.2, 0.5, 0.9] {
                assert_eq!(
                    empirical_interval(values, alpha).unwrap(),
                    empirical_interval(&original, alpha).unwrap()
                );
            }
        }
    }

    #[test]
    fn inverse_ecdf_selects_closed_jumps_without_rounded_rank_multiplication() {
        let samples: Vec<_> = (0..100).map(f64::from).collect();
        assert_eq!(empirical_interval(&samples, 0.14).unwrap(), (6.0, 92.0));
        assert_eq!(empirical_interval(&[7.0], 0.2).unwrap(), (7.0, 7.0));
        assert_eq!(
            empirical_interval(&[0.0, 1.0, 2.0, 3.0], 0.5).unwrap(),
            (0.0, 2.0)
        );
        assert_eq!(
            empirical_interval(&[0.0, 1.0], f64::from_bits(1)).unwrap(),
            (0.0, 1.0)
        );
    }

    #[test]
    fn positive_gap_scores_retain_large_offsets_and_representable_wide_gaps() {
        let offset = 1e16;
        assert_eq!(
            empirical_crps(&[offset, offset + 4.0], offset + 2.0).unwrap(),
            1.0
        );
        assert_eq!(
            empirical_crps(&[-f64::MAX, f64::MAX], f64::MAX).unwrap(),
            f64::MAX / 2.0
        );
        assert!(matches!(
            empirical_crps(&[-f64::MAX], f64::MAX),
            Err(ScoreError::Arithmetic { .. })
        ));
        assert!(matches!(
            empirical_crps(&[0.0, f64::from_bits(1)], 0.0),
            Err(ScoreError::Arithmetic { .. })
        ));
    }

    #[test]
    fn malformed_empirical_distributions_refuse() {
        for samples in [vec![], vec![f64::NAN], vec![f64::INFINITY]] {
            assert!(empirical_crps(&samples, 0.0).is_err());
            assert!(empirical_interval(&samples, 0.2).is_err());
        }
        assert!(empirical_crps(&vec![0.0; MAX_EMPIRICAL_SAMPLES + 1], 0.0).is_err());
        assert_eq!(
            empirical_crps(&vec![0.0; MAX_EMPIRICAL_SAMPLES], 0.0).unwrap(),
            0.0
        );
        assert!(empirical_crps(&[0.0], f64::NAN).is_err());
        for alpha in [0.0, 1.0, -0.1, f64::NAN, f64::INFINITY] {
            assert!(empirical_interval(&[0.0], alpha).is_err());
        }
    }

    #[test]
    fn empirical_completion_extrema_cover_the_declared_domain() {
        let samples = SortedEmpirical::new(&[0.0, 2.0]).unwrap();
        assert_eq!(samples.bounds(None).unwrap(), (0.5, None));
        assert_eq!(samples.bounds(Some((-2.0, 3.0))).unwrap(), (0.5, Some(2.5)));
        assert_eq!(samples.bounds(Some((4.0, 5.0))).unwrap(), (2.5, Some(3.5)));
        for step in 0..=20 {
            let score = samples.crps(-2.0 + f64::from(step) / 4.0).unwrap();
            assert!((0.5..=2.5).contains(&score));
        }
    }
}
