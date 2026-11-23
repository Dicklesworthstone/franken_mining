//! Exact joint mining-period and two-route moves for a fixed mined set.
//! Determinism: BitwisePortable
//!
//! For one block, a destination pair and a proposed period, every signed
//! resource row bounds one integer route fraction. Their intersection gives
//! the best feasible point on the declared decimal grid without an LP solve.
//! Other blocks retain their periods and routes. This bounded neighborhood
//! can cross a period move that is infeasible at the original route fractions;
//! it does not certify the full joint scheduling optimum.

use fmine_evidence::Refusal;

use super::super::{PcpspModel, candidate};
use crate::toposort::refuse;

/// Shared bound on precedence scans, period scans and destination-pair rows.
pub const WORK: usize = 262_144;

/// Exact objective numerators and actual work for the joint neighborhood.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JointSearch {
    pub before: i128,
    pub after: i128,
    pub rounds: usize,
    pub moves: usize,
    /// Accepted moves that change the mining period.
    pub period_moves: usize,
    /// Accepted moves that change one or more destination fractions.
    pub route_moves: usize,
    pub trials: usize,
    pub work: usize,
    pub work_exhausted: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Improvement {
    pub schedule: candidate::SplitSchedule,
    pub checked: candidate::SplitCheck,
    pub search: JointSearch,
}

struct Block {
    id: usize,
    routes: Vec<(u32, i64)>,
    value: i128,
    uses: Vec<i128>,
}

struct Move {
    period: usize,
    routes: Vec<(u32, i64)>,
    value: i128,
    uses: Vec<i128>,
    gain: i128,
}

fn bad(detail: &str) -> Refusal {
    refuse(format!("joint period/routes: {detail}"))
}

fn overflow() -> Refusal {
    bad("exact objective or resource arithmetic overflows i128")
}

fn charge(search: &mut JointSearch, budget: usize, work: usize) -> bool {
    if work > budget - search.work {
        search.work_exhausted = true;
        return false;
    }
    search.work += work;
    true
}

/// Best feasible integer share of `right` when the remaining share goes to
/// `left`. Negative row slopes impose lower bounds; positive slopes impose
/// upper bounds. Endpoints remain inclusive and are never rounded outside.
fn fraction(
    model: &PcpspModel,
    block: usize,
    left: usize,
    right: usize,
    room: &[i128],
    scale: i128,
) -> Result<Option<i128>, Refusal> {
    let (mut lo, mut hi) = (0_i128, scale);
    let start = block * model.destinations();
    for (row, &room) in model.uses.iter().zip(room) {
        let base = i128::from(row[start + left]);
        let slope = i128::from(row[start + right]) - base;
        let rhs = room
            .checked_sub(base.checked_mul(scale).ok_or_else(overflow)?)
            .ok_or_else(overflow)?;
        if slope == 0 {
            if rhs < 0 {
                return Ok(None);
            }
        } else {
            // Euclidean division is floor for a positive divisor and ceil
            // for a negative divisor, exactly the two required bounds.
            let end = rhs.checked_div_euclid(slope).ok_or_else(overflow)?;
            if slope > 0 {
                hi = hi.min(end);
            } else {
                lo = lo.max(end);
            }
        }
        if lo > hi {
            return Ok(None);
        }
    }
    Ok(Some(
        if model.values[start + right] > model.values[start + left] {
            hi
        } else {
            lo
        },
    ))
}

/// Improve one mined block's period and routing together, keeping the mined
/// set unchanged. Every destination pair (including one whole destination) is
/// considered in ID order. The first pair/period of equal value is retained;
/// an equal-value proposal never replaces the input. The two fractions sum to
/// exactly one at the original scale. Existing allocations with more than two
/// destinations are retained unless a tested pair strictly improves them.
///
/// Both affected periods retain every signed resource limit, and all incoming
/// and outgoing mining precedences remain satisfied. After a successful round,
/// the next round can exploit the capacity it freed. At most [`WORK`] work
/// units and 1,000 rounds run; exhaustion returns the checked incumbent. No
/// block insertion, pair of moving blocks, temporary loss or infeasibility,
/// or three-or-more-route optimization is attempted.
///
/// # Errors
/// Invalid or infeasible starts, invalid factors, more than 1,000 rounds,
/// arithmetic overflow or failed independent final verification refuse.
pub fn improve(
    model: &PcpspModel,
    original: &candidate::SplitSchedule,
    factors: &[i64],
    rounds: usize,
) -> Result<Improvement, Refusal> {
    improve_bounded(model, original, factors, rounds, WORK)
}

fn improve_bounded(
    model: &PcpspModel,
    original: &candidate::SplitSchedule,
    factors: &[i64],
    rounds: usize,
    budget: usize,
) -> Result<Improvement, Refusal> {
    if rounds > 1_000 {
        return Err(bad("rounds must be 0..=1000"));
    }
    let checked = candidate::check(model, original)?;
    if !checked.feasible() {
        return Err(bad("starting schedule is infeasible"));
    }
    let before = checked.discounted_value(factors)?;
    let mut search = JointSearch {
        before,
        after: before,
        rounds: 0,
        moves: 0,
        period_moves: 0,
        route_moves: 0,
        trials: 0,
        work: 0,
        work_exhausted: false,
    };
    if rounds == 0 || original.assignments().is_empty() {
        return Ok(Improvement {
            schedule: original.clone(),
            checked,
            search,
        });
    }
    let scale = i128::from(original.scale());
    let mut periods = vec![None; model.blocks()];
    let mut blocks: Vec<Block> = Vec::new();
    for a in original.assignments() {
        let b = a.block as usize;
        periods[b] = Some(a.period as usize);
        if blocks.last().is_none_or(|block| block.id != b) {
            blocks.push(Block {
                id: b,
                routes: Vec::new(),
                value: 0,
                uses: vec![0; model.resources()],
            });
        }
        let at = b * model.destinations() + a.destination as usize;
        let last = blocks.len() - 1;
        let block = &mut blocks[last];
        block.routes.push((a.destination, a.fraction));
        block.value = block
            .value
            .checked_add(i128::from(model.values[at]) * i128::from(a.fraction))
            .ok_or_else(overflow)?;
        for (used, row) in block.uses.iter_mut().zip(&model.uses) {
            *used = used
                .checked_add(i128::from(row[at]) * i128::from(a.fraction))
                .ok_or_else(overflow)?;
        }
    }
    let mut dependants = vec![Vec::new(); model.blocks()];
    for block in &blocks {
        for &required in &model.requires[block.id] {
            dependants[required as usize].push(block.id);
        }
    }
    let mut used = checked.used.clone();
    'rounds: for _ in 0..rounds {
        search.rounds += 1;
        let mut changed = false;
        for block in &mut blocks {
            let b = block.id;
            let scan = 1_usize
                .saturating_add(model.requires[b].len())
                .saturating_add(dependants[b].len())
                .saturating_add(model.resources());
            if !charge(&mut search, budget, scan) {
                break 'rounds;
            }
            let from = periods[b].ok_or_else(|| bad("mined block lost its period"))?;
            let old_value = block
                .value
                .checked_mul(i128::from(factors[from]))
                .ok_or_else(overflow)?;
            let (mut first, mut last) = (0, model.periods() - 1);
            for &required in &model.requires[b] {
                if required as usize != b {
                    first = first.max(
                        periods[required as usize]
                            .ok_or_else(|| bad("mined predecessor lost its period"))?,
                    );
                }
            }
            for &dependent in &dependants[b] {
                if dependent != b {
                    last = last.min(
                        periods[dependent].ok_or_else(|| bad("mined dependant lost its period"))?,
                    );
                }
            }
            let mut vacated_ok = true;
            for (r, &amount) in block.uses.iter().enumerate() {
                if used[r][from].checked_sub(amount).ok_or_else(overflow)?
                    > i128::from(model.limits[r][from]) * scale
                {
                    vacated_ok = false;
                }
            }
            let mut best: Option<Move> = None;
            'periods: for (to, &factor) in factors.iter().enumerate().take(last + 1).skip(first) {
                if !charge(&mut search, budget, model.resources().saturating_add(1)) {
                    break;
                }
                if to != from && !vacated_ok {
                    continue;
                }
                let room = model
                    .limits
                    .iter()
                    .enumerate()
                    .map(|(r, limits)| {
                        let other = if to == from {
                            used[r][to]
                                .checked_sub(block.uses[r])
                                .ok_or_else(overflow)?
                        } else {
                            used[r][to]
                        };
                        (i128::from(limits[to]) * scale)
                            .checked_sub(other)
                            .ok_or_else(overflow)
                    })
                    .collect::<Result<Vec<_>, Refusal>>()?;
                let at = b * model.destinations();
                for left in 0..model.destinations() {
                    for right in left..model.destinations() {
                        if !charge(&mut search, budget, model.resources().saturating_add(1)) {
                            break 'periods;
                        }
                        search.trials += 1;
                        let Some(x) = fraction(model, b, left, right, &room, scale)? else {
                            continue;
                        };
                        let value = (i128::from(model.values[at + left]) * (scale - x))
                            .checked_add(i128::from(model.values[at + right]) * x)
                            .ok_or_else(overflow)?;
                        let gain = value
                            .checked_mul(i128::from(factor))
                            .and_then(|v| v.checked_sub(old_value))
                            .ok_or_else(overflow)?;
                        if gain <= best.as_ref().map_or(0, |m| m.gain) {
                            continue;
                        }
                        let mut routes = Vec::with_capacity(2);
                        for (d, q) in [(left, scale - x), (right, x)] {
                            if q > 0 {
                                routes.push((d as u32, i64::try_from(q).map_err(|_| overflow())?));
                            }
                        }
                        let uses = model
                            .uses
                            .iter()
                            .map(|row| {
                                (i128::from(row[at + left]) * (scale - x))
                                    .checked_add(i128::from(row[at + right]) * x)
                                    .ok_or_else(overflow)
                            })
                            .collect::<Result<Vec<_>, Refusal>>()?;
                        best = Some(Move {
                            period: to,
                            routes,
                            value,
                            uses,
                            gain,
                        });
                    }
                }
            }
            if let Some(next) = best {
                for (r, row) in used.iter_mut().enumerate() {
                    row[from] = row[from].checked_sub(block.uses[r]).ok_or_else(overflow)?;
                    row[next.period] = row[next.period]
                        .checked_add(next.uses[r])
                        .ok_or_else(overflow)?;
                }
                search.after = search.after.checked_add(next.gain).ok_or_else(overflow)?;
                search.moves += 1;
                search.period_moves += usize::from(next.period != from);
                search.route_moves += usize::from(next.routes != block.routes);
                periods[b] = Some(next.period);
                block.routes = next.routes;
                block.value = next.value;
                block.uses = next.uses;
                changed = true;
            }
            if search.work_exhausted {
                break 'rounds;
            }
        }
        if !changed {
            break;
        }
    }
    let mut assignments = Vec::new();
    for block in &blocks {
        let period = periods[block.id].ok_or_else(|| bad("output block lost its period"))?;
        for &(destination, fraction) in &block.routes {
            assignments.push(candidate::DestinationShare {
                block: block.id as u32,
                destination,
                period: period as u32,
                fraction,
            });
        }
    }
    let schedule = candidate::SplitSchedule::new(model, assignments, original.share_decimals())?;
    let checked = candidate::check(model, &schedule)?;
    if !checked.feasible()
        || checked.used != used
        || checked.discounted_value(factors)? != search.after
        || search.after < search.before
    {
        return Err(bad("independent feasibility or value recomputation failed"));
    }
    Ok(Improvement {
        schedule,
        checked,
        search,
    })
}

#[cfg(test)]
mod tests;
