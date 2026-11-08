//! Fractional destinations at fixed mining periods (plan §15.6).
//! Determinism: DeterministicPerIsa (Price proposes; exact integer checks admit).
//!
//! Price's whole-assignment columns guide the search. Each profitable column is
//! joined to the original feasible assignment by a line, and the largest exact
//! decimal-grid step within every signed resource limit is considered. Mining
//! periods and total mined fractions never change. This is a bounded heuristic,
//! not an optimum of the destination LP. No proposal or no representable gain
//! returns the original feasible schedule. The candidate checker independently
//! verifies the result; no floating weight or feasibility tolerance is admitted.

use fmine_evidence::Refusal;

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
    pub proposals: usize,
}

fn overflow() -> Refusal {
    refuse("fractional destinations: exact interpolation overflows i128".into())
}

fn add(total: &mut i128, value: i64) -> Result<(), Refusal> {
    *total = total.checked_add(i128::from(value)).ok_or_else(overflow)?;
    Ok(())
}

/// Propose exact fractional destination changes for a feasible whole schedule.
/// The scale is `10^share_decimals` (0..=9). Each period uses one bounded Price
/// pass: at most 256 columns, 1,024 simplex steps per master solve and at most
/// resource-count plus three directions. The first strictly best direction wins
/// ties. Zero-discount periods are retained without work.
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
    let mut selected: Vec<Option<(i64, Vec<usize>)>> = vec![None; model.periods()];
    let (mut improved_periods, mut proposals) = (0, 0);
    for (t, blocks) in by_period.iter().enumerate() {
        if blocks.is_empty() || factors[t] == 0 {
            continue;
        }
        let caps: Vec<i128> = model.limits.iter().map(|r| i128::from(r[t])).collect();
        let Some(directions) = price::proposals(model, blocks, destinations, &caps) else {
            continue;
        };
        let base = checked.period_values[t];
        let mut best = base;
        for direction in directions {
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
                selected[t] = Some((step, direction));
            }
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
            let (step, to) = selected[t]
                .as_ref()
                .map_or((0, from), |(q, d)| (*q, d[i] as u32));
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
            if from == to {
                emit(from, scale);
            } else {
                emit(from, scale - step);
                emit(to, step);
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
