//! Determinism: BitwisePortable (`i64` and `i128` integer arithmetic only; no floating point, hashing or threads).
//!
//! Local search after TopoSort (`bd-5lxu`; plan §15.4, "TopoSort ... plus local search and repair"). [`improve`] takes
//! a feasible schedule and moves one block, or trades two, at a time. A move is made only when every requirement and
//! limit still holds and the schedule's discounted value strictly rises. The value uses the caller's integer discount
//! factors, from [`discount_factors`] or elsewhere.
//!
//! Each round tries, in this order:
//! - **Earlier.** A block of positive value goes to the earliest period with room, no earlier than its latest required
//!   block.
//! - **Swap.** For each pair of periods `t` and `t + 1`, a block in `t + 1` whose required blocks are all mined by `t`
//!   trades places with a block of lower value in `t` that no block in `t` requires, when both periods keep every
//!   limit. Higher-valued blocks choose first, among the `SWAP_SCAN` lowest-valued candidates.
//! - **Add.** An unmined block of positive value whose required blocks are all mined goes to the earliest period with
//!   room.
//! - **Later.** A block of negative value goes to the latest period with room, no later than the earliest period of a
//!   mined block that requires it.
//! - **Drop.** A mined block of negative value that no mined block requires is unmined.
//!
//! **Later** comes after the moves that bring value forward, because a block it moves can no longer be mined before
//! the blocks that require it: given waste in period 0 and the ore under it in period 2, **Earlier** brings the ore to
//! period 0 when there is room, where **Later** first would send the waste to period 2 and keep the ore there.
//!
//! A use may be negative (a blending row), so taking a block out of a period can put that period over a limit too.
//! **Earlier**, **Later** and **Drop** therefore check the period the block leaves as well as the one it enters;
//! **Swap** checks both periods, and **Add** leaves none.
//!
//! Rounds stop after one without a move, or after the caller's limit. Blocks are visited in Kahn order (ties by id),
//! and in reverse for **Later** and **Drop**. Blocks on a precedence cycle are never visited by those four, and a swap
//! never takes one. [`improve`] ends with [`check`], and refuses rather than return a schedule that fails it or is
//! worth less than the one it was given.
//!
//! It is a heuristic: nothing bounds its distance from the optimum.

use fmine_evidence::Refusal;

use super::{MAX_PERIODS, ScheduleProblem, check, dependents, refuse};

/// The factor of period 0 from [`discount_factors`]: 2⁴⁰.
pub const DISCOUNT_SCALE: i64 = 1 << 40;

/// How many lower-valued blocks each swap candidate looks at, at most.
pub const SWAP_SCAN: usize = 64;

/// `factor[0] = DISCOUNT_SCALE` and `factor[t + 1] = ⌊factor[t] · rate_scale / (rate_scale + rate)⌋`: the discount
/// `(1 + rate / rate_scale)^−t`, times `DISCOUNT_SCALE`, rounded down one period at a time. The factors never rise.
///
/// # Errors
/// `FMINE-REF-207` (proposed) for a negative rate, a scale that is not positive, or periods outside `1..=MAX_PERIODS`.
pub fn discount_factors(rate: i64, rate_scale: i64, periods: usize) -> Result<Vec<i64>, Refusal> {
    if rate < 0 || rate_scale <= 0 {
        return Err(refuse(format!(
            "discount: rate {rate} over {rate_scale}; the rate may not be negative and the scale must be positive"
        )));
    }
    if periods == 0 || periods > MAX_PERIODS {
        return Err(refuse(format!(
            "discount: {periods} periods; at least 1 and at most {MAX_PERIODS}"
        )));
    }
    let (num, den) = (
        i128::from(rate_scale),
        i128::from(rate_scale) + i128::from(rate),
    );
    let mut factors = Vec::with_capacity(periods);
    let mut f = i128::from(DISCOUNT_SCALE);
    for _ in 0..periods {
        // `f` only falls from `DISCOUNT_SCALE`, so it fits `i64`.
        factors.push(i64::try_from(f).map_err(|_| refuse(format!("discount: factor {f}")))?);
        f = f * num / den;
    }
    Ok(factors)
}

/// `Σ value[b] · factor[t]` over the mined blocks, `t` being the block's period.
///
/// # Errors
/// `FMINE-REF-207` (proposed) when `schedule` does not have one entry per block, when `factors` does not have one per
/// period, or when a block's period is past the last.
pub fn discounted_value(
    problem: &ScheduleProblem,
    schedule: &[Option<u32>],
    factors: &[i64],
) -> Result<i128, Refusal> {
    let weights = problem.blocks().weights();
    if schedule.len() != weights.len() || factors.len() != problem.periods() {
        return Err(refuse(format!(
            "discounted value: {} entries for {} blocks and {} factors for {} periods",
            schedule.len(),
            weights.len(),
            factors.len(),
            problem.periods()
        )));
    }
    let mut total = 0_i128;
    for (b, (slot, &v)) in schedule.iter().zip(weights).enumerate() {
        if let Some(t) = *slot {
            let f = factors.get(t as usize).ok_or_else(|| {
                refuse(format!(
                    "discounted value: block {b} is in period {t}, past the last"
                ))
            })?;
            total += i128::from(v) * i128::from(*f);
        }
    }
    Ok(total)
}

/// How many moves of each kind [`improve`] made.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Moves {
    /// Blocks of negative value moved later.
    pub later: usize,
    /// Blocks of positive value moved earlier.
    pub earlier: usize,
    /// Pairs traded between adjacent periods.
    pub swaps: usize,
    /// Blocks added.
    pub added: usize,
    /// Blocks dropped.
    pub dropped: usize,
}

/// What [`improve`] returns.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Improvement {
    /// The improved schedule. It passes [`check`].
    pub schedule: Vec<Option<u32>>,
    /// [`discounted_value`] of the schedule given.
    pub before: i128,
    /// [`discounted_value`] of the improved schedule; never below `before`.
    pub after: i128,
    /// Rounds run, the last of them the one without a move unless the limit stopped it.
    pub rounds: usize,
    /// Moves made.
    pub moves: Moves,
}

/// Local search from a feasible schedule (see the module notes).
///
/// # Errors
/// `FMINE-REF-207` (proposed) when the schedule given fails [`check`] or does not fit the problem, when `factors`
/// does not have one non-negative factor per period or rises, or, as a defect, when the result fails [`check`] or is
/// worth less than the schedule given.
pub fn improve(
    problem: &ScheduleProblem,
    schedule: &[Option<u32>],
    factors: &[i64],
    max_rounds: usize,
) -> Result<Improvement, Refusal> {
    let first = check(problem, schedule)?;
    if !first.feasible() {
        return Err(refuse(format!(
            "local search: the schedule given fails its check ({} unmet requirements, limits exceeded at {:?})",
            first.unmet, first.over_limit
        )));
    }
    let periods = problem.periods();
    if factors.len() != periods
        || factors.iter().any(|&f| f < 0)
        || factors.windows(2).any(|w| w[1] > w[0])
    {
        return Err(refuse(format!(
            "local search: {} factors for {periods} periods; they must be non-negative and never rise",
            factors.len()
        )));
    }
    let before = discounted_value(problem, schedule, factors)?;
    let mut search = Search::new(problem, schedule.to_vec(), first.used, factors)?;
    let mut rounds = 0;
    while rounds < max_rounds {
        rounds += 1;
        let made = search.round();
        if made == 0 {
            break;
        }
    }
    let Search {
        schedule, moves, ..
    } = search;
    let last = check(problem, &schedule)?;
    if !last.feasible() {
        return Err(refuse(format!(
            "local search: the improved schedule fails its check ({} unmet requirements, limits exceeded at {:?})",
            last.unmet, last.over_limit
        )));
    }
    let after = discounted_value(problem, &schedule, factors)?;
    if after < before {
        return Err(refuse(format!(
            "local search: the improved schedule is worth {after}, less than the {before} it was given"
        )));
    }
    Ok(Improvement {
        schedule,
        before,
        after,
        rounds,
        moves,
    })
}

/// The search state: the schedule, each resource's use per period, and the graph in both directions.
struct Search<'a> {
    problem: &'a ScheduleProblem,
    factors: &'a [i64],
    schedule: Vec<Option<u32>>,
    used: Vec<Vec<i128>>,
    start: Vec<usize>,
    dependents: Vec<u32>,
    /// Kahn order, ties by id; blocks on a precedence cycle are missing.
    order: Vec<usize>,
    moves: Moves,
}

impl<'a> Search<'a> {
    fn new(
        problem: &'a ScheduleProblem,
        schedule: Vec<Option<u32>>,
        used: Vec<Vec<i128>>,
        factors: &'a [i64],
    ) -> Result<Self, Refusal> {
        let blocks = problem.blocks();
        let n = blocks.node_count();
        let (start, dependents) = dependents(blocks)?;
        let mut waiting: Vec<usize> = (0..n)
            .map(|b| blocks.requires(b).map_or(0, <[u32]>::len))
            .collect();
        let mut order: Vec<usize> = (0..n).filter(|&b| waiting[b] == 0).collect();
        let mut next = 0;
        while next < order.len() {
            let b = order[next];
            next += 1;
            for &d in &dependents[start[b]..start[b + 1]] {
                let d = d as usize;
                waiting[d] -= 1;
                if waiting[d] == 0 {
                    order.push(d);
                }
            }
        }
        Ok(Self {
            problem,
            factors,
            schedule,
            used,
            start,
            dependents,
            order,
            moves: Moves::default(),
        })
    }

    fn period(&self, b: usize) -> Option<usize> {
        self.schedule[b].map(|t| t as usize)
    }

    fn value(&self, b: usize) -> i64 {
        self.problem.blocks().weights()[b]
    }

    fn required(&self, b: usize) -> &'a [u32] {
        self.problem.blocks().requires(b).unwrap_or(&[])
    }

    /// The latest period of `b`'s required blocks, or `None` when one is unmined.
    fn earliest(&self, b: usize) -> Option<usize> {
        self.required(b)
            .iter()
            .try_fold(0, |e, &q| self.period(q as usize).map(|tq| e.max(tq)))
    }

    /// The earliest period of a mined block that requires `b`, or the last period when none does.
    fn latest(&self, b: usize) -> usize {
        self.dependents[self.start[b]..self.start[b + 1]]
            .iter()
            .filter_map(|&d| self.period(d as usize))
            .min()
            .unwrap_or(self.problem.periods() - 1)
    }

    /// Whether `b` fits in period `t` on top of what is there.
    fn fits(&self, b: usize, t: usize) -> bool {
        (0..self.problem.resources()).all(|r| {
            let (uses, caps) = (self.uses(r), self.caps(r));
            self.used[r][t] + i128::from(uses[b]) <= i128::from(caps[t])
        })
    }

    /// Whether period `t` keeps every limit without `b`; with a negative use, taking `b` out raises what `t` uses.
    fn leaves(&self, b: usize, t: usize) -> bool {
        (0..self.problem.resources()).all(|r| {
            let (uses, caps) = (self.uses(r), self.caps(r));
            self.used[r][t] - i128::from(uses[b]) <= i128::from(caps[t])
        })
    }

    fn uses(&self, r: usize) -> &'a [i64] {
        self.problem.usage(r).unwrap_or(&[])
    }

    fn caps(&self, r: usize) -> &'a [i64] {
        self.problem.limits(r).unwrap_or(&[])
    }

    /// Moves `b` from `from` (`None`: unmined) to `to` (`None`: unmined), keeping the use per period.
    fn relocate(&mut self, b: usize, from: Option<usize>, to: Option<usize>) {
        for r in 0..self.problem.resources() {
            let u = i128::from(self.uses(r)[b]);
            if let Some(t) = from {
                self.used[r][t] -= u;
            }
            if let Some(t) = to {
                self.used[r][t] += u;
            }
        }
        // Periods are at most `MAX_PERIODS`, so they fit `u32`.
        self.schedule[b] = to.map(|t| t as u32);
    }

    /// One round of every move; returns how many were made.
    fn round(&mut self) -> usize {
        let before = self.moves;
        let order = std::mem::take(&mut self.order);
        let f = self.factors;
        for &b in &order {
            if let Some(t) = self
                .period(b)
                .filter(|&t| self.value(b) > 0 && self.leaves(b, t))
                && let Some(to) = self
                    .earliest(b)
                    .and_then(|lo| (lo..t).find(|&to| f[to] > f[t] && self.fits(b, to)))
            {
                self.relocate(b, Some(t), Some(to));
                self.moves.earlier += 1;
            }
        }
        for t in 0..self.problem.periods().saturating_sub(1) {
            if f[t] > f[t + 1] {
                self.swaps(&order, t);
            }
        }
        for &b in &order {
            if self.period(b).is_none()
                && self.value(b) > 0
                && let Some(to) = self.earliest(b).and_then(|lo| {
                    (lo..self.problem.periods()).find(|&to| f[to] > 0 && self.fits(b, to))
                })
            {
                self.relocate(b, None, Some(to));
                self.moves.added += 1;
            }
        }
        for &b in order.iter().rev() {
            if let Some(t) = self
                .period(b)
                .filter(|&t| self.value(b) < 0 && self.leaves(b, t))
                && let Some(to) = (t + 1..=self.latest(b))
                    .rev()
                    .find(|&to| f[to] < f[t] && self.fits(b, to))
            {
                self.relocate(b, Some(t), Some(to));
                self.moves.later += 1;
            }
        }
        for &b in order.iter().rev() {
            if let Some(t) = self.period(b).filter(|_| self.value(b) < 0) {
                let required_by_mined = self.dependents[self.start[b]..self.start[b + 1]]
                    .iter()
                    .any(|&d| self.schedule[d as usize].is_some());
                if f[t] > 0 && !required_by_mined && self.leaves(b, t) {
                    self.relocate(b, Some(t), None);
                    self.moves.dropped += 1;
                }
            }
        }
        self.order = order;
        let m = self.moves;
        (m.later - before.later)
            + (m.earlier - before.earlier)
            + (m.swaps - before.swaps)
            + (m.added - before.added)
            + (m.dropped - before.dropped)
    }

    /// Trades between periods `t` and `t + 1` (see the module notes).
    fn swaps(&mut self, order: &[usize], t: usize) {
        let mut up: Vec<usize> = order
            .iter()
            .copied()
            .filter(|&b| self.period(b) == Some(t + 1) && self.earliest(b).is_some_and(|e| e <= t))
            .collect();
        let mut down: Vec<usize> = order
            .iter()
            .copied()
            .filter(|&c| self.period(c) == Some(t) && self.latest(c) > t)
            .collect();
        up.sort_by_key(|&b| (std::cmp::Reverse(self.value(b)), b));
        down.sort_by_key(|&c| (self.value(c), c));
        let mut head = 0;
        for b in up {
            // Earlier trades may have changed what `b` needs.
            if self.period(b) != Some(t + 1) || !self.earliest(b).is_some_and(|e| e <= t) {
                continue;
            }
            while head < down.len() && self.period(down[head]) != Some(t) {
                head += 1;
            }
            let mut seen = 0;
            for &c in &down[head..] {
                if self.value(c) >= self.value(b) || seen == SWAP_SCAN {
                    break;
                }
                if self.period(c) != Some(t) {
                    continue;
                }
                seen += 1;
                if self.latest(c) <= t
                    || self.required(b).contains(&(c as u32))
                    || !self.trades(b, c, t)
                {
                    continue;
                }
                self.relocate(b, Some(t + 1), Some(t));
                self.relocate(c, Some(t), Some(t + 1));
                self.moves.swaps += 1;
                break;
            }
        }
    }

    /// Whether `b` (in `t + 1`) and `c` (in `t`) can trade periods within every limit.
    fn trades(&self, b: usize, c: usize, t: usize) -> bool {
        (0..self.problem.resources()).all(|r| {
            let (uses, caps) = (self.uses(r), self.caps(r));
            let (ub, uc) = (i128::from(uses[b]), i128::from(uses[c]));
            self.used[r][t] - uc + ub <= i128::from(caps[t])
                && self.used[r][t + 1] - ub + uc <= i128::from(caps[t + 1])
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pitflow::ClosureProblem;
    use crate::toposort::greedy;
    use crate::toposort::tests::{
        every_schedule, feasible_by_definition, lcg, random_problem, random_signed_problem,
    };

    /// 10% a period, at 10⁹.
    const TEN_PERCENT: (i64, i64) = (100_000_000, 1_000_000_000);

    /// One resource with one limit for every period.
    fn one_resource(
        values: Vec<i64>,
        requires: &[Vec<u32>],
        uses: Vec<i64>,
        periods: usize,
        limit: i64,
    ) -> ScheduleProblem {
        let blocks = ClosureProblem::from_requirements(values, requires).unwrap();
        ScheduleProblem::new(blocks, periods, vec![uses], vec![vec![limit; periods]]).unwrap()
    }

    fn factors(periods: usize) -> Vec<i64> {
        discount_factors(TEN_PERCENT.0, TEN_PERCENT.1, periods).unwrap()
    }

    #[test]
    fn discount_factors_fall_by_the_rate_rounding_down() {
        assert_eq!(
            factors(4),
            vec![
                1_099_511_627_776,
                999_556_025_250,
                908_687_295_681,
                826_079_359_710
            ]
        );
        assert_eq!(discount_factors(0, 1, 3).unwrap(), vec![DISCOUNT_SCALE; 3]);
        for (rate, scale, periods) in [
            (-1, 1, 3),
            (1, 0, 3),
            (1, -1, 3),
            (1, 1, 0),
            (1, 1, MAX_PERIODS + 1),
        ] {
            assert_eq!(
                discount_factors(rate, scale, periods).unwrap_err().code,
                "FMINE-REF-207"
            );
        }
        let f = discount_factors(i64::MAX, 1, MAX_PERIODS).unwrap();
        assert!(
            f.windows(2).all(|w| w[1] <= w[0]) && f[1] == 0,
            "{:?}",
            &f[..3]
        );
    }

    #[test]
    fn discounted_value_sums_each_mined_block_at_its_period() {
        let p = one_resource(
            vec![5, -2, 7],
            &[vec![], vec![], vec![]],
            vec![0, 0, 0],
            2,
            0,
        );
        let f = vec![10, 4];
        assert_eq!(
            discounted_value(&p, &[Some(0), Some(1), None], &f).unwrap(),
            50 - 8
        );
        assert_eq!(discounted_value(&p, &[None; 3], &f).unwrap(), 0);
        assert!(discounted_value(&p, &[None; 2], &f).is_err());
        assert!(discounted_value(&p, &[None; 3], &[10]).is_err());
        assert!(discounted_value(&p, &[Some(2), None, None], &f).is_err());
    }

    #[test]
    fn each_move_on_a_known_case() {
        let f = factors(3);
        // Swap: room for one block a period; greedy took the cheap block first.
        let p = one_resource(vec![1, 10], &[vec![], vec![]], vec![1, 1], 2, 1);
        let s = greedy(&p, &[Some(0), Some(1)]).unwrap();
        assert_eq!(s, vec![Some(0), Some(1)]);
        let i = improve(&p, &s, &f[..2], 10).unwrap();
        assert_eq!(i.schedule, vec![Some(1), Some(0)]);
        assert_eq!(
            i.moves,
            Moves {
                swaps: 1,
                ..Moves::default()
            }
        );
        assert_eq!(i.after - i.before, 9 * i128::from(f[0] - f[1]));
        // Earlier before Later: the ore under the waste comes to period 0, and the waste stays.
        let p = one_resource(vec![-2, 10], &[vec![], vec![0]], vec![1, 1], 3, 2);
        let i = improve(&p, &[Some(0), Some(2)], &f, 10).unwrap();
        assert_eq!(i.schedule, vec![Some(0), Some(0)]);
        assert_eq!(
            i.moves,
            Moves {
                earlier: 1,
                ..Moves::default()
            }
        );
        // With period 0 full, the ore goes to period 1, then trades with the cheap block still in period 0.
        let p = one_resource(
            vec![-2, 10, 1],
            &[vec![], vec![0], vec![]],
            vec![1, 1, 1],
            3,
            2,
        );
        let i = improve(&p, &[Some(0), Some(2), Some(0)], &f, 10).unwrap();
        assert_eq!(i.schedule, vec![Some(0), Some(0), Some(1)]);
        assert_eq!(
            i.moves,
            Moves {
                earlier: 1,
                swaps: 1,
                ..Moves::default()
            }
        );
        // Later: a second resource keeps the ore out of period 0, so its waste follows it to period 1.
        let blocks = ClosureProblem::from_requirements(vec![-2, 10], &[vec![], vec![0]]).unwrap();
        let p = ScheduleProblem::new(
            blocks,
            2,
            vec![vec![1, 1], vec![0, 1]],
            vec![vec![2, 2], vec![0, 1]],
        )
        .unwrap();
        let i = improve(&p, &[Some(0), Some(1)], &f[..2], 10).unwrap();
        assert_eq!(i.schedule, vec![Some(1), Some(1)]);
        assert_eq!(
            i.moves,
            Moves {
                later: 1,
                ..Moves::default()
            }
        );
        // Add: the ore under mined waste.
        let p = one_resource(vec![-2, 10], &[vec![], vec![0]], vec![1, 1], 2, 2);
        let i = improve(&p, &[Some(0), None], &f[..2], 10).unwrap();
        assert_eq!(i.schedule, vec![Some(0), Some(0)]);
        assert_eq!(
            i.moves,
            Moves {
                added: 1,
                ..Moves::default()
            }
        );
        // Drop: waste that no mined block needs.
        let p = one_resource(vec![-1], &[vec![]], vec![1], 1, 1);
        let i = improve(&p, &[Some(0)], &f[..1], 10).unwrap();
        assert_eq!(i.schedule, vec![None]);
        assert_eq!(
            i.moves,
            Moves {
                dropped: 1,
                ..Moves::default()
            }
        );
        // Without discounting nothing moves except what adds or drops value.
        let flat = vec![DISCOUNT_SCALE; 2];
        let p = one_resource(vec![1, 10], &[vec![], vec![]], vec![1, 1], 2, 1);
        let i = improve(&p, &[Some(0), Some(1)], &flat, 10).unwrap();
        assert_eq!(
            (i.schedule, i.moves, i.rounds),
            (vec![Some(0), Some(1)], Moves::default(), 1)
        );
    }

    #[test]
    fn bad_input_refuses() {
        let p = one_resource(vec![1, 10], &[vec![], vec![0]], vec![1, 1], 2, 1);
        let f = factors(2);
        for (s, f) in [
            (vec![Some(0), Some(0)], f.clone()), // over the limit
            (vec![None, Some(0)], f.clone()),    // a requirement unmet
            (vec![Some(0)], f.clone()),          // one entry short
            (vec![Some(0), None], vec![f[0]]),   // one factor short
            (vec![Some(0), None], vec![1, 2]),   // a rising factor
            (vec![Some(0), None], vec![1, -1]),  // a negative factor
        ] {
            assert_eq!(
                improve(&p, &s, &f, 10).unwrap_err().code,
                "FMINE-REF-207",
                "{s:?} {f:?}"
            );
        }
    }

    #[test]
    fn a_block_below_the_blend_stays_where_its_period_needs_it() {
        // A blend with limit 0 in both periods. Block 0 is below the target (use −2), block 1 above it (+2), and both
        // are mined together. Each move of block 0 alone would raise the value and put its period over the blend:
        // - worth −1 in period 0: **Later** to period 1, or **Drop**;
        // - worth −1 in period 1: **Drop**;
        // - worth 1 in period 1: **Earlier** to period 0.
        // Block 1 cannot move alone either: wherever it goes, it is over the blend.
        for (value, period) in [(-1, 0), (-1, 1), (1, 1)] {
            let blocks =
                ClosureProblem::from_requirements(vec![value, 5], &[vec![], vec![]]).unwrap();
            let p = ScheduleProblem::new(blocks, 2, vec![vec![-2, 2]], vec![vec![0, 0]]).unwrap();
            let s = vec![Some(period); 2];
            let i = improve(&p, &s, &factors(2), 10).unwrap();
            assert_eq!(
                (i.schedule, i.moves),
                (s, Moves::default()),
                "{value} {period}"
            );
        }
    }

    /// Greedy under a random ranking, then [`improve`]: the result is feasible by the definition, worth at least as
    /// much, never more than the best schedule there is, and a second call changes nothing. Returns the values before
    /// and after, and the best.
    fn improve_case(p: &ScheduleProblem, seed: u64) -> (i128, i128, i128) {
        let n = p.blocks().node_count();
        let f = factors(p.periods());
        let mut r = lcg(seed ^ 0x5A5A);
        let rank: Vec<Option<i64>> = (0..n).map(|_| (r(5) > 0).then(|| r(10) as i64)).collect();
        let s = greedy(p, &rank).unwrap();
        let i = improve(p, &s, &f, 1_000).unwrap();
        assert!(
            feasible_by_definition(p, &i.schedule),
            "seed {seed}: {:?}",
            i.schedule
        );
        assert_eq!(i.before, discounted_value(p, &s, &f).unwrap());
        assert_eq!(i.after, discounted_value(p, &i.schedule, &f).unwrap());
        assert!(i.after >= i.before, "seed {seed}");
        let again = improve(p, &i.schedule, &f, 1_000).unwrap();
        assert_eq!(
            (again.schedule.as_slice(), again.moves),
            (i.schedule.as_slice(), Moves::default())
        );
        let best = every_schedule(n, p.periods())
            .into_iter()
            .filter(|s| feasible_by_definition(p, s))
            .map(|s| discounted_value(p, &s, &f).unwrap())
            .max()
            .unwrap();
        assert!(i.after <= best, "seed {seed}");
        (i.before, i.after, best)
    }

    #[test]
    fn improve_is_feasible_monotone_and_bounded_by_the_exhaustive_optimum() {
        let (mut cases, mut gained, mut optimal_before, mut optimal_after) = (0, 0, 0, 0);
        for seed in 0..400 {
            let (before, after, best) = improve_case(&random_problem(seed), seed);
            cases += 1;
            gained += usize::from(after > before);
            optimal_before += usize::from(before == best);
            optimal_after += usize::from(after == best);
        }
        println!(
            "{{\"bead\":\"bd-5lxu\",\"cases\":{cases},\"improved\":{gained},\"optimal_greedy\":{optimal_before},\"optimal_after\":{optimal_after}}}"
        );
        assert!(
            gained > 0 && optimal_after > optimal_before,
            "{gained} {optimal_before} {optimal_after}"
        );
    }

    /// The same with one more resource whose uses may be negative.
    #[test]
    fn improve_keeps_every_limit_with_signed_uses() {
        let (mut gained, mut optimal_before, mut optimal_after) = (0, 0, 0);
        for seed in 0..400 {
            let (before, after, best) = improve_case(&random_signed_problem(seed), seed);
            gained += usize::from(after > before);
            optimal_before += usize::from(before == best);
            optimal_after += usize::from(after == best);
        }
        println!(
            "{{\"bead\":\"bd-ixfw\",\"cases\":400,\"improved\":{gained},\"optimal_greedy\":{optimal_before},\"optimal_after\":{optimal_after}}}"
        );
        assert!(
            gained > 0 && optimal_after > optimal_before,
            "{gained} {optimal_before} {optimal_after}"
        );
    }
}
