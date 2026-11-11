//! Fractional destinations at fixed mining periods (plan §15.6).
//! Determinism: DeterministicPerIsa (Price proposes; exact integer checks admit).
//!
//! Price's whole-assignment columns guide the search. Each profitable column is
//! joined to the original feasible assignment by a line, and the largest exact
//! decimal-grid step within every signed resource limit is considered. The last
//! successful master basis also supplies an exact rational mixture when it can
//! be reconstructed. Its per-block destination totals must be representable on
//! the requested decimal grid; no mixture weight is rounded. Mining periods and
//! total mined fractions never change. This is a bounded heuristic, not a proof
//! of the destination LP optimum. No proposal or no representable gain
//! returns the original feasible schedule. The candidate checker independently
//! verifies the result; no floating weight or feasibility tolerance is admitted.

use fmine_evidence::Refusal;
use fmine_numerics::exact_feas::Rat;

use super::{PcpspModel, candidate, price};
use crate::toposort::refuse;

/// A feasible exact schedule, including the original when no direction improves
/// it. Objective numerators use `checked.scale` times the caller's factor scale.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Improvement {
    pub schedule: candidate::SplitSchedule,
    pub checked: candidate::SplitCheck,
    pub before: i128,
    pub after: i128,
    pub improved_periods: usize,
    /// Whole-assignment proposals used for line searches; the optional exact
    /// master mixture uses the same pricing pass and is not another direction.
    pub proposals: usize,
}

#[derive(Clone)]
enum Selection {
    Line(i64, Vec<usize>),
    Mixture(Vec<Vec<i64>>),
}

fn overflow() -> Refusal {
    refuse("fractional destinations: exact interpolation overflows i128".into())
}

fn add(total: &mut i128, value: i64) -> Result<(), Refusal> {
    *total = total.checked_add(i128::from(value)).ok_or_else(overflow)?;
    Ok(())
}

/// Convert only exactly representable per-block totals, then independently
/// check their integer resource sums before considering this advisory mixture.
/// Failed reconstruction or arithmetic preserves the line-search fallback.
fn decimal_mixture(
    model: &PcpspModel,
    blocks: &[usize],
    caps: &[i128],
    shares: &[Vec<Rat>],
    scale: i64,
) -> Option<(i128, Vec<Vec<i64>>)> {
    if shares.len() != blocks.len() || caps.len() != model.resources() {
        return None;
    }
    let mut fractions = Vec::with_capacity(blocks.len());
    let mut value = 0_i128;
    let mut used = vec![0_i128; model.resources()];
    for (&b, row) in blocks.iter().zip(shares) {
        if row.len() != model.destinations() {
            return None;
        }
        let mut total = 0_i64;
        let mut exact = Vec::with_capacity(row.len());
        for (d, &share) in row.iter().enumerate() {
            let share = share
                .checked_mul(Rat::from_integer(i128::from(scale)))
                .ok()?;
            if share.den() != 1 || share.signum() < 0 {
                return None;
            }
            let fraction = i64::try_from(share.num()).ok()?;
            total = total.checked_add(fraction)?;
            value = value
                .checked_add(i128::from(model.value(b, d)?).checked_mul(i128::from(fraction))?)?;
            for (r, amount) in used.iter_mut().enumerate() {
                *amount = amount.checked_add(
                    i128::from(model.uses(r, b, d)?).checked_mul(i128::from(fraction))?,
                )?;
            }
            exact.push(fraction);
        }
        if total != scale {
            return None;
        }
        fractions.push(exact);
    }
    for (&amount, &cap) in used.iter().zip(caps) {
        if amount > cap.checked_mul(i128::from(scale))? {
            return None;
        }
    }
    Some((value, fractions))
}

/// Propose exact fractional destination changes for a feasible whole schedule.
/// The scale is `10^share_decimals` (0..=9). Each period uses one bounded Price
/// pass: at most 256 columns, 1,024 simplex steps per master solve and at most
/// resource-count plus three directions, plus one exact reconstruction for at
/// most 32 resources. The first strictly best direction wins ties; an equal
/// mixture does not displace it. Zero-discount periods are retained without work.
///
/// # Errors
/// Invalid schedules, dimensions, scales or negative factors refuse. Numerical
/// overflow or a failed independent final check also refuses; an unsuccessful
/// pricing pass simply preserves that period's original assignment.
pub fn polish(
    model: &PcpspModel,
    periods: &[Option<u32>],
    destinations: &[u32],
    factors: &[i64],
    share_decimals: u32,
) -> Result<Improvement, Refusal> {
    let original = candidate::from_whole(model, periods, destinations, share_decimals)?;
    let checked = candidate::check(model, &original)?;
    if !checked.feasible() {
        return Err(refuse(
            "fractional destinations: starting schedule is infeasible".into(),
        ));
    }
    let before = checked.discounted_value(factors)?;
    let scale = original.scale();
    let mut by_period = vec![Vec::new(); model.periods()];
    for (b, period) in periods.iter().enumerate() {
        if let Some(t) = period {
            by_period[*t as usize].push(b);
        }
    }
    let mut selected: Vec<Option<Selection>> = vec![None; model.periods()];
    let (mut improved_periods, mut proposals) = (0, 0);
    for (t, blocks) in by_period.iter().enumerate() {
        if blocks.is_empty() || factors[t] == 0 {
            continue;
        }
        let caps: Vec<i128> = model.limits.iter().map(|r| i128::from(r[t])).collect();
        let Some(priced) = price::proposals(model, blocks, destinations, &caps, true) else {
            continue;
        };
        let base = checked.period_values[t];
        let mut best = base;
        for direction in priced.directions {
            proposals += 1;
            let mut value = 0_i128;
            let mut uses = vec![0_i128; model.resources()];
            for (&b, &d) in blocks.iter().zip(&direction) {
                let at = b * model.destinations() + d;
                add(&mut value, model.values[at])?;
                for (used, row) in uses.iter_mut().zip(&model.uses) {
                    add(used, row[at])?;
                }
            }
            let target = value.checked_mul(i128::from(scale)).ok_or_else(overflow)?;
            if target <= base {
                continue;
            }
            let mut step = i128::from(scale);
            for ((&used, &cap), initial) in uses.iter().zip(&caps).zip(&checked.used) {
                // The starting point is whole, so division by scale is exact.
                let initial_use = initial[t] / i128::from(scale);
                let delta = used.checked_sub(initial_use).ok_or_else(overflow)?;
                if delta > 0 {
                    let room = cap
                        .checked_sub(initial_use)
                        .ok_or_else(overflow)?
                        .checked_mul(i128::from(scale))
                        .ok_or_else(overflow)?;
                    step = step.min(room / delta);
                }
            }
            if step == 0 {
                continue;
            }
            let original_value = base / i128::from(scale);
            let gain = value
                .checked_sub(original_value)
                .ok_or_else(overflow)?
                .checked_mul(step)
                .ok_or_else(overflow)?;
            let proposed = base.checked_add(gain).ok_or_else(overflow)?;
            if proposed > best {
                let step = i64::try_from(step).map_err(|_| overflow())?;
                best = proposed;
                selected[t] = Some(Selection::Line(step, direction));
            }
        }
        if let Some((value, shares)) = priced
            .shares
            .as_ref()
            .and_then(|shares| decimal_mixture(model, blocks, &caps, shares, scale))
            && value > best
        {
            selected[t] = Some(Selection::Mixture(shares));
        }
        improved_periods += usize::from(selected[t].is_some());
    }
    if improved_periods == 0 {
        return Ok(Improvement {
            schedule: original,
            checked,
            before,
            after: before,
            improved_periods,
            proposals,
        });
    }
    let mut assignments = Vec::new();
    for (t, blocks) in by_period.iter().enumerate() {
        for (i, &b) in blocks.iter().enumerate() {
            let from = destinations[b];
            let mut emit = |destination, fraction| {
                if fraction > 0 {
                    assignments.push(candidate::DestinationShare {
                        block: b as u32,
                        destination,
                        period: t as u32,
                        fraction,
                    });
                }
            };
            match &selected[t] {
                Some(Selection::Line(step, direction)) => {
                    let to = direction[i] as u32;
                    if from == to {
                        emit(from, scale);
                    } else {
                        emit(from, scale - step);
                        emit(to, *step);
                    }
                }
                Some(Selection::Mixture(shares)) => {
                    for (d, &fraction) in shares[i].iter().enumerate() {
                        emit(d as u32, fraction);
                    }
                }
                None => emit(from, scale),
            }
        }
    }
    let schedule = candidate::SplitSchedule::new(model, assignments, share_decimals)?;
    let checked = candidate::check(model, &schedule)?;
    let after = checked.discounted_value(factors)?;
    if !checked.feasible() || after <= before {
        return Err(refuse(
            "fractional destinations: independent feasibility or strict value improvement failed"
                .into(),
        ));
    }
    Ok(Improvement {
        schedule,
        checked,
        before,
        after,
        improved_periods,
        proposals,
    })
}

#[cfg(test)]
mod tests;
