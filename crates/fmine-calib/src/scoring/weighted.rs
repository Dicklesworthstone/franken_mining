//! DESIGN (BD-047/048): finite predictive scenarios with exact declared probability mass.
//! Determinism: DeterministicPerIsa (exact CDF arithmetic, floating-point loss integration).

use std::cmp::Ordering;

use fmine_units::ExactQty;

use super::{MAX_EMPIRICAL_SAMPLES, PositiveSum, arithmetic, weighted_gap};
use crate::ScoreError;

const ZERO: ExactQty = ExactQty::from_integer(0);
const ONE: ExactQty = ExactQty::from_integer(1);
const TWO: ExactQty = ExactQty::from_integer(2);

/// CRPS of a finite predictive distribution with exact, strictly positive probabilities summing to one.
///
/// DESIGN: sorting a scratch copy and integrating positive squared-CDF gaps costs O(n log n) time
/// and O(n) space. Cumulative and remaining masses are exact rationals; only loss integration uses
/// floating point. Duplicate values retain their combined mass. No normalization, resampling,
/// smoothing or omission repairs an invalid population. Unrepresentable arithmetic refuses.
pub fn weighted_empirical_crps(
    samples: &[f64],
    weights: &[ExactQty],
    outcome: f64,
) -> Result<f64, ScoreError> {
    SortedWeightedEmpirical::new(samples, weights)?.crps(outcome)
}

/// Central interval of an exact-weight finite predictive distribution, without interpolation.
///
/// DESIGN: this new distribution interprets `alpha` through its canonical shortest decimal spelling,
/// so `0.2` selects exact rational levels `1/10` and `9/10`. It does not reinterpret a legacy uniform
/// empirical forecast. Quantiles select the first value whose exact cumulative mass reaches the
/// requested exact level, including equality. Rational capacity failures refuse; discrete inclusive
/// intervals need not contain exactly `1-alpha` probability and confer no calibration claim.
pub fn weighted_empirical_interval(
    samples: &[f64],
    weights: &[ExactQty],
    alpha: f64,
) -> Result<(f64, f64), ScoreError> {
    SortedWeightedEmpirical::new(samples, weights)?.interval(alpha)
}

pub(crate) fn validate_weighted(samples: &[f64], weights: &[ExactQty]) -> Result<(), ScoreError> {
    if samples.is_empty() || samples.len() > MAX_EMPIRICAL_SAMPLES {
        return Err(ScoreError::InvalidParameter {
            name: "weighted_empirical_sample_count",
            value: samples.len() as f64,
        });
    }
    if samples.len() != weights.len() {
        return Err(ScoreError::InvalidParameter {
            name: "weighted_empirical_weight_count",
            value: weights.len() as f64,
        });
    }
    if samples.iter().any(|value| !value.is_finite()) {
        return Err(ScoreError::NonFinite);
    }
    let mut total = ZERO;
    for &weight in weights {
        if weight.signum() <= 0 || weight.num() > weight.den() {
            return Err(ScoreError::InvalidParameter {
                name: "weighted_empirical_probability",
                value: weight.to_f64(),
            });
        }
        total = total
            .checked_add(weight)
            .map_err(|_| arithmetic("exact weighted empirical probability sum"))?;
    }
    if total != ONE {
        return Err(ScoreError::InvalidParameter {
            name: "weighted_empirical_probability_sum",
            value: total.to_f64(),
        });
    }
    Ok(())
}

/// One sorted population supplies an assessment's intervals, loss and completion bounds.
pub(crate) struct SortedWeightedEmpirical {
    /// Values are sorted, with complete exact cumulative masses including every repeated sample.
    points: Vec<(f64, ExactQty)>,
}

impl SortedWeightedEmpirical {
    pub(crate) fn new(samples: &[f64], weights: &[ExactQty]) -> Result<Self, ScoreError> {
        validate_weighted(samples, weights)?;
        let mut points: Vec<_> = samples
            .iter()
            .copied()
            .zip(weights.iter().copied())
            .collect();
        points.sort_by(|(left, lw), (right, rw)| {
            left.total_cmp(right)
                .then_with(|| lw.num().cmp(&rw.num()))
                .then_with(|| lw.den().cmp(&rw.den()))
        });
        let mut cumulative = ZERO;
        for (_, weight) in &mut points {
            cumulative = cumulative
                .checked_add(*weight)
                .map_err(|_| arithmetic("exact weighted empirical cumulative mass"))?;
            *weight = cumulative;
        }
        Ok(Self { points })
    }

    fn quantile(&self, level: ExactQty) -> Result<f64, ScoreError> {
        let mut lo = 0;
        let mut hi = self.points.len();
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            if self.points[mid]
                .1
                .checked_cmp(level)
                .map_err(|_| arithmetic("exact weighted empirical quantile comparison"))?
                == Ordering::Less
            {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        self.points
            .get(lo)
            .map(|(value, _)| *value)
            .ok_or_else(|| arithmetic("weighted empirical quantile support"))
    }

    pub(crate) fn interval(&self, alpha: f64) -> Result<(f64, f64), ScoreError> {
        if !alpha.is_finite() {
            return Err(ScoreError::NonFinite);
        }
        if !(alpha > 0.0 && alpha < 1.0) {
            return Err(ScoreError::AlphaOutOfDomain(alpha));
        }
        let tail = ExactQty::from_decimal_str(&alpha.to_string())
            .and_then(|level| level.checked_div(TWO))
            .map_err(|_| arithmetic("exact weighted empirical interval level"))?;
        let upper = ONE
            .checked_sub(tail)
            .map_err(|_| arithmetic("exact weighted empirical interval level"))?;
        Ok((self.quantile(tail)?, self.quantile(upper)?))
    }

    pub(crate) fn crps(&self, outcome: f64) -> Result<f64, ScoreError> {
        if !outcome.is_finite() {
            return Err(ScoreError::NonFinite);
        }
        let mut score = PositiveSum::default();
        let first = self.points[0].0;
        let last = self.points[self.points.len() - 1].0;
        if outcome < first {
            score.add(weighted_gap(outcome, first, 1.0)?)?;
        } else if outcome > last {
            score.add(weighted_gap(last, outcome, 1.0)?)?;
        }
        for pair in self.points.windows(2) {
            let ((left, cumulative), (right, _)) = (pair[0], pair[1]);
            if left == right {
                continue;
            }
            let below = cumulative.to_f64();
            // Subtraction before projection preserves tiny declared tail mass when F rounds to one.
            let above = ONE
                .checked_sub(cumulative)
                .map_err(|_| arithmetic("exact weighted empirical tail mass"))?
                .to_f64();
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
        let median = self.quantile(
            ONE.checked_div(TWO)
                .map_err(|_| arithmetic("weighted empirical median level"))?,
        )?;
        if let Some((lo, hi)) = domain {
            if !lo.is_finite() || !hi.is_finite() {
                return Err(ScoreError::NonFinite);
            }
            if lo > hi {
                return Err(ScoreError::InvertedInterval { lo, hi });
            }
            // The loss is convex in the outcome. This selects its domain extremizer, never repairs
            // an observation: actual outcomes must independently satisfy the frozen domain.
            let minimum = if median < lo {
                lo
            } else if median > hi {
                hi
            } else {
                median
            };
            Ok((
                self.crps(minimum)?,
                Some(self.crps(lo)?.max(self.crps(hi)?)),
            ))
        } else {
            Ok((self.crps(median)?, None))
        }
    }
}

#[cfg(test)]
mod tests;
