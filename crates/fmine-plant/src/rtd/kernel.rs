//! DESIGN: analytic equal-tank transition and integrated outlet weights.
//! For x = throughput volume / one active tank volume, p_k=e^-x x^k/k!
//! and q_k=P(Poisson(x)>k). Initial stock i reaches outlet with q_(N-1-i).
//! Constant feed contributes q_j/x to tank j and E[(Poisson(x)-N)+]/x
//! to the outlet. All weights use positive sums; outlet is not a balance plug.
//! Long intervals compose steps with x<=32. The only truncation is a bounded
//! positive Poisson tail, reduced below relative binary64 resolution.
//! Determinism: DeterministicPerIsa

use super::{Result, numerical};

pub(super) const MAX_TURNOVER: f64 = 32.0;

pub(super) struct Weights {
    pub retained: Vec<f64>,
    pub exited: Vec<f64>,
    pub fed: Vec<f64>,
    pub fed_exit: f64,
}

/// DESIGN: unit-agnostic compensated reduction, internal to RTD.
/// Each accumulator uses one caller-chosen scalar unit: dimensionless transition
/// weights or admitted material-channel masses. Public RTD inputs remain Claims;
/// this arithmetic helper performs neither source admission nor unit conversion.
#[derive(Default)]
pub(super) struct Sum {
    value: f64,
    correction: f64,
}
impl Sum {
    pub fn add(&mut self, value: f64) -> Result<()> {
        let next = self.value + value;
        self.correction += if self.value.abs() >= value.abs() {
            (self.value - next) + value
        } else {
            (value - next) + self.value
        };
        self.value = next;
        if !self.value.is_finite() || !self.correction.is_finite() {
            return Err(numerical("RTD sum exceeds binary64 range"));
        }
        Ok(())
    }
    pub fn finish(&self) -> Result<f64> {
        let value = self.value + self.correction;
        if !value.is_finite() {
            return Err(numerical("RTD compensated sum is unrepresentable"));
        }
        Ok(value)
    }
}

pub(super) fn sum(values: impl IntoIterator<Item = f64>) -> Result<f64> {
    let mut total = Sum::default();
    for value in values {
        total.add(value)?;
    }
    total.finish()
}

pub(super) fn product(a: f64, b: f64) -> Result<f64> {
    let value = a * b;
    if !value.is_finite() || (a != 0.0 && b != 0.0 && value == 0.0) {
        return Err(numerical("RTD product loses numerical range"));
    }
    Ok(value)
}

pub(super) fn quotient(a: f64, b: f64) -> Result<f64> {
    let value = a / b;
    if !value.is_finite() || (a != 0.0 && value == 0.0) {
        return Err(numerical("RTD quotient loses numerical range"));
    }
    Ok(value)
}

pub(super) fn weights(tanks: usize, x: f64) -> Result<Weights> {
    if !(1..=64).contains(&tanks) || !x.is_finite() || x <= 0.0 || x > MAX_TURNOVER {
        return Err(numerical("invalid bounded RTD transition"));
    }
    let mut probabilities = vec![(-x).exp()];
    for k in 1..=tanks {
        probabilities.push(quotient(product(probabilities[k - 1], x)?, k as f64)?);
    }
    let mut tail = Sum::default();
    let mut overshoot = Sum::default();
    let mut probability = probabilities[tanks];
    let mut resolved = false;
    for k in tanks + 1..=512 {
        probability = quotient(product(probability, x)?, k as f64)?;
        tail.add(probability)?;
        overshoot.add(product((k - tanks) as f64, probability)?)?;
        let ratio = x / (k + 1) as f64;
        if ratio < 1.0 {
            let remainder = probability * ratio / (1.0 - ratio);
            let weighted_remainder = remainder * (k - tanks + 1) as f64
                + probability * ratio * ratio / ((1.0 - ratio) * (1.0 - ratio));
            if remainder <= f64::EPSILON * tail.finish()? * 0.25
                && weighted_remainder <= f64::EPSILON * overshoot.finish()? * 0.25
            {
                resolved = true;
                break;
            }
        }
    }
    if !resolved {
        return Err(numerical("RTD Poisson tail exceeds its fixed work bound"));
    }
    let mut exited = vec![0.0; tanks];
    let mut remainder = tail;
    for k in (0..tanks).rev() {
        remainder.add(probabilities[k + 1])?;
        exited[k] = remainder.finish()?;
    }
    let fed = exited
        .iter()
        .map(|&q| quotient(q, x))
        .collect::<Result<Vec<_>>>()?;
    let fed_exit = quotient(overshoot.finish()?, x)?;
    for k in 0..tanks {
        let identity = sum(probabilities[..=k].iter().copied().chain([exited[k]]))?;
        if (identity - 1.0).abs() > 2e-13 {
            return Err(numerical(
                "RTD initial-stock transition fails normalization",
            ));
        }
    }
    if (sum(fed.iter().copied().chain([fed_exit]))? - 1.0).abs() > 2e-13 {
        return Err(numerical(
            "RTD integrated-feed transition fails normalization",
        ));
    }
    probabilities.truncate(tanks);
    Ok(Weights {
        retained: probabilities,
        exited,
        fed,
        fed_exit,
    })
}

/// One analytic interval; stocks are changed only in this local working state.
/// Columns are dry matter, H2O and canonical elemental channels.
pub(super) fn advance(stock: &mut [Vec<f64>], feed: &[f64], weights: &Weights) -> Result<Vec<f64>> {
    let n = stock.len();
    let mut next = vec![vec![0.0; feed.len()]; n];
    let mut output = vec![0.0; feed.len()];
    for (c, &fresh) in feed.iter().enumerate() {
        let mut discharged = Sum::default();
        discharged.add(product(fresh, weights.fed_exit)?)?;
        for (i, row) in stock.iter().enumerate() {
            discharged.add(product(row[c], weights.exited[n - 1 - i])?)?;
        }
        output[c] = discharged.finish()?;
        for j in 0..n {
            let mut retained = Sum::default();
            retained.add(product(fresh, weights.fed[j])?)?;
            for (i, row) in stock[..=j].iter().enumerate() {
                retained.add(product(row[c], weights.retained[j - i])?)?;
            }
            next[j][c] = retained.finish()?;
        }
    }
    for (destination, row) in stock.iter_mut().zip(next) {
        *destination = row;
    }
    Ok(output)
}
