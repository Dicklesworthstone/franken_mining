//! DESIGN (BD-048): exact paired binary-event losses on the original frozen probabilities.
//! Determinism: Exact
//! The difference of Brier losses is (p-q)(p+q-2y). For an unresolved binary outcome,
//! its extrema are attained at zero or one; no probability floor or outcome prior is introduced.

use fmine_evidence::Refusal;

use super::super::ForecastPayload;
use super::{Rat, domain, exact};

const ZERO: Rat = Rat::from_integer(0);
const TWO: Rat = Rat::from_integer(2);

pub(super) fn admit(payload: &ForecastPayload) -> Result<(), Refusal> {
    match payload {
        ForecastPayload::Bernoulli { probability }
            if probability.is_finite() && (0.0..=1.0).contains(probability) =>
        {
            Ok(())
        }
        _ => Err(domain(
            "paired Brier loss requires Bernoulli forecasts throughout both complete cohorts; no binary event is inferred from a continuous distribution",
        )),
    }
}

pub(super) fn scores(
    left: &ForecastPayload,
    right: &ForecastPayload,
    outcome: Option<f64>,
) -> Option<(Rat, Rat)> {
    let (
        ForecastPayload::Bernoulli { probability: p },
        ForecastPayload::Bernoulli { probability: q },
    ) = (left, right)
    else {
        return None;
    };
    if admit(left).is_err() || admit(right).is_err() {
        return None;
    }
    if outcome.is_some_and(|y| y != 0.0 && y != 1.0) {
        return None;
    }
    // Equal finite probabilities cancel exactly even when their binary rational denominator
    // exceeds the arithmetic capacity. They do not require an invented rounded probability.
    if p == q {
        return Some((ZERO, ZERO));
    }
    let p = exact(*p).ok()?;
    let q = exact(*q).ok()?;
    let gap = p.checked_sub(q).ok()?;
    let sum = p.checked_add(q).ok()?;
    let difference = |y: f64| {
        gap.checked_mul(sum.checked_sub(if y == 0.0 { ZERO } else { TWO }).ok()?)
            .ok()
    };
    if let Some(y) = outcome {
        let value = difference(y)?;
        return Some((value, value));
    }
    let (at_zero, at_one) = (difference(0.0)?, difference(1.0)?);
    // d/dy = -2(p-q), so comparing the probabilities avoids an unnecessary rational subtraction.
    Some(if gap.signum() > 0 {
        (at_one, at_zero)
    } else {
        (at_zero, at_one)
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn event(probability: f64) -> ForecastPayload {
        ForecastPayload::Bernoulli { probability }
    }

    #[test]
    fn exact_brier_differences_and_completion_extrema_match_direct_losses() {
        for p in 0_i32..=16 {
            for q in 0_i32..=16 {
                let left = event(f64::from(p) / 16.0);
                let right = event(f64::from(q) / 16.0);
                // FACT: directly squaring integer distances on this grid gives losses over 256.
                let at_zero = Rat::new(i128::from(p * p - q * q), 256).unwrap();
                let at_one =
                    Rat::new(i128::from((16 - p) * (16 - p) - (16 - q) * (16 - q)), 256).unwrap();
                assert_eq!(scores(&left, &right, Some(0.0)), Some((at_zero, at_zero)));
                assert_eq!(scores(&left, &right, Some(1.0)), Some((at_one, at_one)));
                let extrema = if p >= q {
                    (at_one, at_zero)
                } else {
                    (at_zero, at_one)
                };
                assert_eq!(scores(&left, &right, None), Some(extrema));
            }
        }
    }

    #[test]
    fn deterministic_probabilities_ties_and_arithmetic_capacity_stay_explicit() {
        let one = Rat::from_integer(1);
        let minus_one = Rat::from_integer(-1);
        assert_eq!(
            scores(&event(0.0), &event(1.0), None),
            Some((minus_one, one))
        );
        assert_eq!(
            scores(&event(0.0), &event(1.0), Some(1.0)),
            Some((one, one))
        );
        for p in [0.0, 0.5, 1.0, f64::from_bits(1), 1e-100] {
            assert_eq!(scores(&event(p), &event(p), None), Some((ZERO, ZERO)));
        }
        assert_eq!(scores(&event(f64::from_bits(1)), &event(0.5), None), None);
        assert_eq!(scores(&event(0.25), &event(0.75), Some(0.5)), None);
        assert!(admit(&event(f64::NAN)).is_err());
        assert!(admit(&event(-0.1)).is_err());
        assert!(
            admit(&ForecastPayload::Empirical {
                samples: vec![0.0, 1.0]
            })
            .is_err()
        );
    }
}
