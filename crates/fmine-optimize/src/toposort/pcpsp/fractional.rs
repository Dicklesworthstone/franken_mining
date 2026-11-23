//! Exact fractional destinations and bounded period improvement (plan §15.6).
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
//! [`reschedule`] then searches mining periods while retaining every destination
//! fraction exactly, including those supplied by an outside planner.

use fmine_evidence::Refusal;
use fmine_numerics::exact_feas::Rat;

use super::{PcpspModel, candidate, price};
use crate::toposort::refuse;

pub mod joint;

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

/// At most this many block/precedence/resource and target-period work units are
/// examined by one exact period search, across all requested rounds.
pub const PERIOD_WORK: usize = 262_144;

/// At most this many following mined blocks, in ID order, are considered as
/// partners when single-block period moves stall.
pub const PERIOD_PAIR_SCAN: usize = 64;

/// Actual work and exact objective numerators for a fixed-fraction period search.
/// A stopped search retains its feasible incumbent; it supplies no bound.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PeriodSearch {
    pub before: i128,
    pub after: i128,
    pub rounds: usize,
    /// Accepted single-block or atomic two-block moves.
    pub moves: usize,
    /// Atomic two-block moves, included in `moves`.
    pub pair_moves: usize,
    pub trials: usize,
    pub work: usize,
    pub work_exhausted: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rescheduled {
    pub schedule: candidate::SplitSchedule,
    pub checked: candidate::SplitCheck,
    pub search: PeriodSearch,
}

struct PeriodBlock {
    id: usize,
    value: i128,
    uses: Vec<i128>,
}

struct PeriodPairs<'a> {
    model: &'a PcpspModel,
    blocks: &'a [PeriodBlock],
    factors: &'a [i64],
    dependants: &'a [Vec<usize>],
    scale: i128,
}

impl PeriodPairs<'_> {
    /// Accept the first pair with a profitable feasible common period, taking
    /// its best period and the earliest of ties. Resume single moves afterwards.
    fn advance(
        &self,
        periods: &mut [Option<usize>],
        used: &mut [Vec<i128>],
        search: &mut PeriodSearch,
        budget: usize,
    ) -> Result<bool, Refusal> {
        let bad = |detail| refuse(format!("fractional periods: {detail}"));
        let overflow = || bad("exact pair objective or resource arithmetic overflows i128");
        for (index, first_block) in self.blocks.iter().enumerate() {
            for second_block in self.blocks.iter().skip(index + 1).take(PERIOD_PAIR_SCAN) {
                let pair = [first_block, second_block];
                let ids = [first_block.id, second_block.id];
                let scan = ids.iter().fold(1_usize, |n, &b| {
                    n.saturating_add(self.model.requires[b].len())
                        .saturating_add(self.dependants[b].len())
                });
                if scan > budget - search.work {
                    search.work_exhausted = true;
                    return Ok(false);
                }
                search.work += scan;
                let old = [
                    periods[ids[0]].ok_or_else(|| bad("mined pair member lost its period"))?,
                    periods[ids[1]].ok_or_else(|| bad("mined pair member lost its period"))?,
                ];
                let (mut first, mut last) = (0, self.model.periods() - 1);
                for b in ids {
                    for &required in &self.model.requires[b] {
                        if !ids.contains(&(required as usize)) {
                            first = first.max(
                                periods[required as usize]
                                    .ok_or_else(|| bad("mined predecessor lost its period"))?,
                            );
                        }
                    }
                    for &dependent in &self.dependants[b] {
                        if !ids.contains(&dependent) {
                            last = last.min(
                                periods[dependent]
                                    .ok_or_else(|| bad("mined dependant lost its period"))?,
                            );
                        }
                    }
                }
                let mut best = None;
                for (to, &factor) in self.factors.iter().enumerate().take(last + 1).skip(first) {
                    if old == [to, to] {
                        continue;
                    }
                    // Charge every possible affected period, even when two
                    // coincide or an early row rejects the proposal.
                    let trial = self.model.resources().saturating_mul(3).saturating_add(1);
                    if trial > budget - search.work {
                        search.work_exhausted = true;
                        break;
                    }
                    search.work += trial;
                    search.trials += 1;
                    let gain = pair.iter().zip(old).try_fold(0_i128, |sum, (b, from)| {
                        b.value
                            .checked_mul(i128::from(factor) - i128::from(self.factors[from]))
                            .and_then(|value| sum.checked_add(value))
                            .ok_or_else(overflow)
                    })?;
                    if gain <= best.map_or(0, |(_, gain)| gain) {
                        continue;
                    }
                    let affected = [old[0], old[1], to];
                    let mut feasible = true;
                    'periods: for (i, &t) in affected.iter().enumerate() {
                        if affected[..i].contains(&t) {
                            continue;
                        }
                        for (r, row) in used.iter().enumerate() {
                            let mut total = row[t];
                            for (block, from) in pair.iter().zip(old) {
                                if from == t {
                                    total =
                                        total.checked_sub(block.uses[r]).ok_or_else(overflow)?;
                                }
                                if to == t {
                                    total =
                                        total.checked_add(block.uses[r]).ok_or_else(overflow)?;
                                }
                            }
                            if total > i128::from(self.model.limits[r][t]) * self.scale {
                                feasible = false;
                                break 'periods;
                            }
                        }
                    }
                    if feasible {
                        best = Some((to, gain));
                    }
                }
                if let Some((to, gain)) = best {
                    for (r, row) in used.iter_mut().enumerate() {
                        for (block, from) in pair.iter().zip(old) {
                            row[from] =
                                row[from].checked_sub(block.uses[r]).ok_or_else(overflow)?;
                            row[to] = row[to].checked_add(block.uses[r]).ok_or_else(overflow)?;
                        }
                    }
                    periods[ids[0]] = Some(to);
                    periods[ids[1]] = Some(to);
                    search.after = search.after.checked_add(gain).ok_or_else(overflow)?;
                    search.moves += 1;
                    search.pair_moves += 1;
                    return Ok(true);
                }
                if search.work_exhausted {
                    return Ok(false);
                }
            }
        }
        Ok(false)
    }
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

/// Move already mined blocks between periods without changing their exact
/// destination fractions. Blocks are visited in ID order; each takes its best
/// strictly profitable feasible period, with the earliest period breaking ties.
/// Both the vacated and receiving periods must retain every signed resource
/// limit, and all mined predecessors and dependants must retain their order.
/// After single moves stall, pairs from the next [`PERIOD_PAIR_SCAN`] mined
/// blocks may enter one common period atomically. Internal precedence moves
/// with the pair; every external predecessor and dependant still bounds it.
/// The first improving pair takes its best period, then single moves resume.
/// No block is added or removed, no fraction is rounded, and no temporary
/// infeasibility or loss is accepted. Larger groups, distant partners and pairs
/// moving to different new periods are outside this bounded neighborhood.
///
/// The caller supplies 0..=1,000 rounds. [`PERIOD_WORK`] additionally bounds the
/// search across those rounds; input aggregation and the independent final
/// candidate check are linear in the supplied model and assignments. Zero
/// rounds, an empty schedule or equal factors retain the original schedule.
///
/// # Errors
/// Structural candidate errors propagate. Infeasible schedules, missing or
/// negative factors, more than 1,000 rounds, exact arithmetic overflow, or a
/// failed final feasibility/value check refuse with `FMINE-REF-207`.
pub fn reschedule(
    model: &PcpspModel,
    original: &candidate::SplitSchedule,
    factors: &[i64],
    rounds: usize,
) -> Result<Rescheduled, Refusal> {
    reschedule_bounded(model, original, factors, rounds, PERIOD_WORK)
}

fn reschedule_bounded(
    model: &PcpspModel,
    original: &candidate::SplitSchedule,
    factors: &[i64],
    rounds: usize,
    budget: usize,
) -> Result<Rescheduled, Refusal> {
    let bad = |detail| refuse(format!("fractional periods: {detail}"));
    if rounds > 1_000 {
        return Err(bad("rounds must be 0..=1000"));
    }
    let checked = candidate::check(model, original)?;
    if !checked.feasible() {
        return Err(bad("starting schedule is infeasible"));
    }
    let before = checked.discounted_value(factors)?;
    let mut search = PeriodSearch {
        before,
        after: before,
        rounds: 0,
        moves: 0,
        pair_moves: 0,
        trials: 0,
        work: 0,
        work_exhausted: false,
    };
    if rounds == 0 || original.assignments().is_empty() || factors.windows(2).all(|w| w[0] == w[1])
    {
        return Ok(Rescheduled {
            schedule: original.clone(),
            checked,
            search,
        });
    }
    let overflow = || bad("exact objective or resource arithmetic overflows i128");
    let scale = i128::from(original.scale());
    let mut periods = vec![None; model.blocks()];
    let mut blocks: Vec<PeriodBlock> = Vec::new();
    for a in original.assignments() {
        let b = a.block as usize;
        periods[b] = Some(a.period as usize);
        if blocks.last().is_none_or(|block| block.id != b) {
            blocks.push(PeriodBlock {
                id: b,
                value: 0,
                uses: vec![0; model.resources()],
            });
        }
        let at = b * model.destinations() + a.destination as usize;
        let last = blocks.len() - 1;
        let block = &mut blocks[last];
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
    let pairs = PeriodPairs {
        model,
        blocks: &blocks,
        factors,
        dependants: &dependants,
        scale,
    };
    'rounds: for _ in 0..rounds {
        search.rounds += 1;
        let mut changed = false;
        for block in &blocks {
            let b = block.id;
            let scan = 1_usize
                .saturating_add(model.requires[b].len())
                .saturating_add(dependants[b].len());
            if scan > budget - search.work {
                search.work_exhausted = true;
                break 'rounds;
            }
            search.work += scan;
            if block.value == 0 {
                continue;
            }
            let from = periods[b].ok_or_else(|| bad("mined block lost its period"))?;
            let mut first = 0;
            let mut last = model.periods() - 1;
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
            let mut best = None;
            for (to, &factor) in factors.iter().enumerate().take(last + 1).skip(first) {
                if to == from {
                    continue;
                }
                let trial = 1 + model.resources();
                if trial > budget - search.work {
                    search.work_exhausted = true;
                    break;
                }
                search.work += trial;
                search.trials += 1;
                let gain = block
                    .value
                    .checked_mul(i128::from(factor) - i128::from(factors[from]))
                    .ok_or_else(overflow)?;
                if gain <= best.map_or(0, |(_, gain)| gain) {
                    continue;
                }
                let mut feasible = true;
                for (r, &amount) in block.uses.iter().enumerate() {
                    let vacated = used[r][from].checked_sub(amount).ok_or_else(overflow)?;
                    let received = used[r][to].checked_add(amount).ok_or_else(overflow)?;
                    // Negative blend credits are material: removing one can
                    // violate the old period even when the new one has room.
                    if vacated > i128::from(model.limits[r][from]) * scale
                        || received > i128::from(model.limits[r][to]) * scale
                    {
                        feasible = false;
                        break;
                    }
                }
                if feasible {
                    best = Some((to, gain));
                }
            }
            if let Some((to, gain)) = best {
                for (r, &amount) in block.uses.iter().enumerate() {
                    used[r][from] = used[r][from].checked_sub(amount).ok_or_else(overflow)?;
                    used[r][to] = used[r][to].checked_add(amount).ok_or_else(overflow)?;
                }
                periods[b] = Some(to);
                search.after = search.after.checked_add(gain).ok_or_else(overflow)?;
                search.moves += 1;
                changed = true;
            }
            if search.work_exhausted {
                break 'rounds;
            }
        }
        if !changed && !pairs.advance(&mut periods, &mut used, &mut search, budget)? {
            break;
        }
        if search.work_exhausted {
            break;
        }
    }
    let assignments = original
        .assignments()
        .iter()
        .map(|a| {
            let period =
                periods[a.block as usize].ok_or_else(|| bad("output block lost its period"))?;
            Ok(candidate::DestinationShare {
                period: period as u32,
                ..*a
            })
        })
        .collect::<Result<Vec<_>, Refusal>>()?;
    let schedule = candidate::SplitSchedule::new(model, assignments, original.share_decimals())?;
    let checked = candidate::check(model, &schedule)?;
    if !checked.feasible()
        || checked.used != used
        || checked.discounted_value(factors)? != search.after
        || search.after < search.before
    {
        return Err(bad("independent feasibility or value recomputation failed"));
    }
    Ok(Rescheduled {
        schedule,
        checked,
        search,
    })
}

#[cfg(test)]
mod tests;
