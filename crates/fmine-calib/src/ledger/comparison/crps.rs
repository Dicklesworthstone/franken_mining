//! DESIGN: exact paired CRPS and global missing-outcome bounds for finite distributions.
//! Determinism: Exact
//! With unit mass on each side, d/dy(CRPS(F,y)-CRPS(G,y)) = 2(F(y)-G(y)), constant between atoms
//! and zero outside their combined support, so every global extremum occurs at an atom; sorting
//! plus two linear sweeps avoids O(n²) pair distances or rescoring at candidate outcomes.

use fmine_evidence::Refusal;
use fmine_units::{ExactQty, UnitsError};

use super::super::ForecastPayload;
use super::{Rat, domain, exact};

const ZERO: Rat = Rat::from_integer(0);
const ONE: Rat = Rat::from_integer(1);
const TWO: Rat = Rat::from_integer(2);

pub(super) fn admit(payload: &ForecastPayload) -> Result<(), Refusal> {
    match payload {
        ForecastPayload::Empirical { .. } | ForecastPayload::WeightedEmpirical { .. } => Ok(()),
        _ => Err(domain(
            "paired CRPS supports only complete empirical or weighted_empirical forecasts; no distribution is inferred from intervals, quantiles or Gaussian parameters",
        )),
    }
}

fn points(payload: &ForecastPayload) -> Result<Vec<(f64, ExactQty)>, UnitsError> {
    let mut points = match payload {
        ForecastPayload::Empirical { samples } => {
            let weight = ONE.checked_div(Rat::from_integer(samples.len() as i128))?;
            samples
                .iter()
                .map(|&value| (value, weight))
                .collect::<Vec<_>>()
        }
        ForecastPayload::WeightedEmpirical { samples, weights } => samples
            .iter()
            .copied()
            .zip(weights.iter().copied())
            .collect(),
        _ => return Err(UnitsError::Overflow), // Admission refuses unsupported payloads separately.
    };
    points.sort_by(|(a, aw), (b, bw)| {
        a.total_cmp(b)
            .then_with(|| aw.num().cmp(&bw.num()))
            .then_with(|| aw.den().cmp(&bw.den()))
    });
    Ok(points)
}

struct Segment {
    left: f64,
    right: f64,
    start: Rat,
    width: Rat,
    slope: Rat,
}

fn calculate(
    left: &ForecastPayload,
    right: &ForecastPayload,
    outcome: Option<f64>,
) -> Result<(Rat, Rat), UnitsError> {
    let (left, right) = (points(left)?, points(right)?);
    let mut knots: Vec<_> = left.iter().chain(&right).map(|(x, _)| *x).collect();
    knots.sort_by(f64::total_cmp);
    // Signed zeros denote the same support point; their exact mass is still fully accumulated below.
    knots.dedup_by(|a, b| *a == *b);
    let first = *knots.first().ok_or(UnitsError::Overflow)?;
    let mut li = 0;
    let mut ri = 0;
    let mut lf = ZERO;
    let mut rf = ZERO;
    let mut at_first = ZERO;
    let mut segments = Vec::with_capacity(knots.len().saturating_sub(1));
    for pair in knots.windows(2) {
        let (x, next) = (pair[0], pair[1]);
        while li < left.len() && left[li].0 <= x {
            lf = lf.checked_add(left[li].1)?;
            li += 1;
        }
        while ri < right.len() && right[ri].0 <= x {
            rf = rf.checked_add(right[ri].1)?;
            ri += 1;
        }
        let start = exact(x)?;
        let width = exact(next)?.checked_sub(start)?;
        let mass_difference = lf.checked_sub(rf)?;
        // At the leftmost knot both CRPS integrals use upper tails. Factoring the difference of
        // their squares avoids separately accumulating two large nonnegative losses.
        let tail_difference = rf
            .checked_sub(lf)?
            .checked_mul(TWO.checked_sub(lf)?.checked_sub(rf)?)?;
        at_first = at_first.checked_add(width.checked_mul(tail_difference)?)?;
        segments.push(Segment {
            left: x,
            right: next,
            start,
            width,
            slope: TWO.checked_mul(mass_difference)?,
        });
    }
    let mut value = at_first;
    let mut minimum = value;
    let mut maximum = value;
    let mut observed = outcome.filter(|&y| y <= first).map(|_| value);
    for segment in segments {
        if let Some(y) = outcome
            && y > segment.left
            && y < segment.right
        {
            observed = Some(
                value.checked_add(
                    segment
                        .slope
                        .checked_mul(exact(y)?.checked_sub(segment.start)?)?,
                )?,
            );
        }
        value = value.checked_add(segment.slope.checked_mul(segment.width)?)?;
        if value.checked_sub(minimum)?.signum() < 0 {
            minimum = value;
        }
        if value.checked_sub(maximum)?.signum() > 0 {
            maximum = value;
        }
        if outcome == Some(segment.right) {
            observed = Some(value);
        }
    }
    if let Some(y) = outcome {
        if y >= *knots.last().ok_or(UnitsError::Overflow)? {
            observed = Some(value);
        }
        let score = observed.ok_or(UnitsError::Overflow)?;
        Ok((score, score))
    } else {
        Ok((minimum, maximum))
    }
}

pub(super) fn scores(
    left: &ForecastPayload,
    right: &ForecastPayload,
    outcome: Option<f64>,
) -> Option<(Rat, Rat)> {
    calculate(left, right, outcome).ok()
}
