//! DESIGN (BD-048): univariate uniform empirical forecasts and explicitly floored event log scores.
//! Sorted scratch values determine empirical scores; callers retain original draws in frozen records.
//! This module supplies no shared realization-space, weighting, or calibration claim.
//! Determinism: DeterministicPerIsa (no cross-host empirical-score digest has been established).

use crate::ScoreError;
use fmine_core::det;

/// DESIGN (BD-048/BD-343): Gaussian CRPS extrema over a prospectively frozen finite outcome domain.
/// Its derivative in the outcome is `2 * Phi((y - mean) / std_dev) - 1`, so the minimum is at the
/// mean or nearer domain endpoint and the maximum is at an endpoint. Selecting an extremizer does
/// not clamp a supplied observation; outcome admission separately enforces the frozen domain.
pub(crate) fn gaussian_crps_bounds(
    mean: f64,
    std_dev: f64,
    (lo, hi): (f64, f64),
) -> Result<(f64, f64), ScoreError> {
    if !lo.is_finite() || !hi.is_finite() {
        return Err(ScoreError::NonFinite);
    }
    if lo > hi {
        return Err(ScoreError::InvertedInterval { lo, hi });
    }
    let left = crate::gaussian_crps(mean, std_dev, lo)?;
    let right = crate::gaussian_crps(mean, std_dev, hi)?;
    let minimum = if mean < lo {
        left
    } else if mean > hi {
        right
    } else {
        crate::gaussian_crps(mean, std_dev, mean)?
    };
    let maximum = left.max(right);
    if [left, right, minimum]
        .iter()
        .all(|value| value.is_finite() && *value >= 0.0)
        && minimum <= maximum
    {
        Ok((minimum, maximum))
    } else {
        Err(arithmetic("Gaussian CRPS completion bounds"))
    }
}

/// Negative natural log of the probability assigned to the observed binary outcome (plan §10.2).
///
/// DESIGN: the caller must freeze `probability_floor` in `(0, 0.5)` before outcomes. The loss is
/// `-ln(max(P(outcome), probability_floor))`, in nats; the forecast itself is never changed. The
/// floor deliberately caps extreme losses and removes strict propriety in the floored tails.
/// Invalid probabilities or floors refuse. This bounded diagnostic supplies no calibration verdict.
pub fn bernoulli_log_score(
    probability: f64,
    outcome: bool,
    probability_floor: f64,
) -> Result<f64, ScoreError> {
    if !probability.is_finite() || !probability_floor.is_finite() {
        return Err(ScoreError::NonFinite);
    }
    if !(0.0..=1.0).contains(&probability) {
        return Err(ScoreError::InvalidParameter {
            name: "event_probability",
            value: probability,
        });
    }
    if !(probability_floor > 0.0 && probability_floor < 0.5) {
        return Err(ScoreError::InvalidParameter {
            name: "event_log_floor",
            value: probability_floor,
        });
    }
    let assigned = if outcome {
        probability
    } else {
        1.0 - probability
    };
    let loss = if assigned < probability_floor {
        -det::ln(probability_floor)
    } else if !outcome && probability > 0.0 {
        // Preserve the positive loss when 1-p rounds to one, and correct cancellation otherwise.
        if assigned == 1.0 {
            probability
        } else {
            -det::ln(assigned) * (probability / (1.0 - assigned))
        }
    } else if assigned == 1.0 {
        0.0
    } else {
        -det::ln(assigned)
    };
    if loss.is_finite() && loss >= 0.0 {
        Ok(loss)
    } else {
        Err(arithmetic("event log score"))
    }
}

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

    #[test]
    fn gaussian_completion_extrema_match_independent_normal_known_answers() {
        // FACT: these N(0,1) scores at z=0,1,2,3 were evaluated independently with
        // 80-digit decimal arithmetic and erf's power series, not the candidate's CDF.
        let [s0, s1, s2, s3] = [
            0.233_694_977_255_109_07,
            0.602_441_357_627_616_3,
            1.452_791_821_685_903,
            2.436_574_725_086_339,
        ];
        for (domain, expected) in [
            ((-3.0, -1.0), (s1, s3)),
            ((-1.0, 2.0), (s0, s2)),
            ((1.0, 3.0), (s1, s3)),
            ((0.0, 0.0), (s0, s0)),
            ((2.0, 2.0), (s2, s2)),
        ] {
            for (mean, sd) in [(0.0, 1.0), (7.0, 2.0), (-9.0, 0.125)] {
                let shifted = (mean + sd * domain.0, mean + sd * domain.1);
                let actual = gaussian_crps_bounds(mean, sd, shifted).unwrap();
                assert!((actual.0 - sd * expected.0).abs() < 2e-13);
                assert!((actual.1 - sd * expected.1).abs() < 2e-13);
                for step in 0..=1024 {
                    let y = shifted.0 + (shifted.1 - shifted.0) * f64::from(step) / 1024.0;
                    let score = crate::gaussian_crps(mean, sd, y).unwrap();
                    assert!(score >= actual.0 - 2e-13 && score <= actual.1 + 2e-13);
                }
            }
        }
    }

    #[test]
    fn gaussian_completion_bounds_refuse_invalid_domains_and_unrepresentable_scores() {
        for domain in [
            (f64::NAN, 1.0),
            (0.0, f64::INFINITY),
            (f64::NEG_INFINITY, 0.0),
            (2.0, 1.0),
        ] {
            assert!(gaussian_crps_bounds(0.0, 1.0, domain).is_err());
        }
        for (mean, sd) in [
            (f64::NAN, 1.0),
            (0.0, 0.0),
            (0.0, -1.0),
            (0.0, f64::INFINITY),
        ] {
            assert!(gaussian_crps_bounds(mean, sd, (-1.0, 2.0)).is_err());
        }
        assert!(matches!(
            gaussian_crps_bounds(0.0, 1e-308, (2.0, 3.0)),
            Err(ScoreError::Arithmetic { .. })
        ));
    }

    #[test]
    fn event_log_scores_match_independent_logs_and_retain_the_declared_cap() {
        for p in [0.0_f64, 0.01, 0.125, 0.25, 0.5, 0.75, 0.99, 1.0] {
            for occurred in [false, true] {
                let assigned = if occurred { p } else { 1.0 - p };
                let expected = -assigned.max(0.01).ln();
                let got = bernoulli_log_score(p, occurred, 0.01).unwrap();
                assert!((got - expected).abs() < 2e-14, "p={p}, outcome={occurred}");
            }
        }
        assert_eq!(bernoulli_log_score(1.0, true, 0.01).unwrap(), 0.0);
        assert_eq!(bernoulli_log_score(0.0, false, 0.01).unwrap(), 0.0);
        assert_eq!(
            bernoulli_log_score(0.0, true, 0.01).unwrap(),
            bernoulli_log_score(0.005, true, 0.01).unwrap()
        );
        // FACT: -ln(1-p) is positive even when floating-point subtraction rounds 1-p to one.
        for p in [f64::from_bits(1), 1e-300, 1e-20, 1e-12, 1e-8] {
            let got = bernoulli_log_score(p, false, 0.01).unwrap();
            let expected = -(-p).ln_1p();
            assert!(got > 0.0);
            assert!((got / expected - 1.0).abs() < 2e-14);
        }
        let smallest = f64::from_bits(1);
        assert!((bernoulli_log_score(0.0, true, smallest).unwrap() + smallest.ln()).abs() < 1e-12);
    }

    #[test]
    fn event_log_scoring_refuses_invalid_probabilities_and_floors() {
        for p in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, -0.01, 1.01] {
            assert!(bernoulli_log_score(p, false, 0.01).is_err());
        }
        for floor in [
            f64::NAN,
            f64::INFINITY,
            f64::NEG_INFINITY,
            -0.01,
            0.0,
            0.5,
            1.0,
        ] {
            assert!(bernoulli_log_score(0.25, true, floor).is_err());
        }
    }

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
