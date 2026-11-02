//! Determinism: BitwisePortable (`i64` and `i128` integer arithmetic only; no floating point, hashing or threads).
//!
//! Multi-period schedules on a precedence graph (plan §15.4 row `fmine-optimize::toposort`; `bd-kcpj`): the
//! feasibility check PO-PUSH-5 asks for, and TopoSort, the greedy of Chicoisne, Espinoza, Goycoolea, Moreno and Rubio
//! (Operations Research 60(3), 2012) that turns a ranking of the blocks into a feasible schedule.
//!
//! **Problem.** Blocks carry values and require other blocks, as in `pitflow`. There are `T` periods and some
//! resources: block `b` uses `usage[r][b] ≥ 0` of resource `r` in the period it is mined, and resource `r` may use at
//! most `limit[r][t] ≥ 0` in period `t`. A schedule mines each block in one period or not at all. It is feasible when
//! every mined block's required blocks are mined in the same period or earlier, and no resource goes over its limit
//! in any period. The limits are upper limits only, so mining nothing is feasible.
//!
//! **Value.** [`check`] returns each period's value sum exactly, undiscounted. Discounting is the caller's.
//!
//! **TopoSort.** The caller ranks each block, lower first, or leaves it out with `None`.
//! - The order is Kahn's: repeatedly take, among the blocks whose required blocks have all been taken, the one with
//!   the lowest rank, ties by id. Precedence comes first, so any ranking gives a topological order. Blocks on a
//!   precedence cycle are never taken and stay unmined.
//! - Each block in that order goes in the earliest period, no earlier than its required blocks, with room on every
//!   resource. A block without a rank, with a required block left unmined, or with no period that has room stays
//!   unmined.
//! - [`greedy`] runs [`check`] on its schedule and refuses rather than return one that fails.
//!
//! [`check`] is written apart from [`greedy`] and shares nothing with it but the problem type. Both live in this
//! crate, so it is not the separate `fmine-verify` checker that plan §15.4 names.

pub mod cpit;

use std::cmp::Reverse;
use std::collections::BinaryHeap;

use fmine_evidence::{Refusal, RefusalCode};

use crate::pitflow::ClosureProblem;

/// How many unmet requirements [`ScheduleCheck`] keeps as witnesses; all of them are counted.
pub const WITNESSES: usize = 10;

/// The most periods a [`ScheduleProblem`] may have: daily periods for over 179 years. [`check`] and [`greedy`] keep a
/// sum per period, so without a bound a problem with no resources would make them allocate for periods it never
/// describes (found by the `cpit` fuzz target).
pub const MAX_PERIODS: usize = 1 << 16;

fn refuse(detail: String) -> Refusal {
    Refusal::new(RefusalCode::NumericalCertificateFailed207, detail)
}

/// A block or period index as `u32`. Both are below `u32::MAX` by the problem's guards.
fn id(x: usize) -> Result<u32, Refusal> {
    u32::try_from(x).map_err(|_| refuse(format!("toposort: index {x} does not fit u32")))
}

/// Blocks with their values and requirements, the periods, and the resources.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScheduleProblem {
    blocks: ClosureProblem,
    periods: usize,
    /// `usage[r][b]`.
    usage: Vec<Vec<i64>>,
    /// `limits[r][t]`.
    limits: Vec<Vec<i64>>,
}

impl ScheduleProblem {
    /// # Errors
    /// `FMINE-REF-207` (proposed) when there are no periods or more than [`MAX_PERIODS`], when the resources do not each
    /// have one use per block and one limit per period, or when a use or a limit is negative.
    pub fn new(
        blocks: ClosureProblem,
        periods: usize,
        usage: Vec<Vec<i64>>,
        limits: Vec<Vec<i64>>,
    ) -> Result<Self, Refusal> {
        if periods == 0 || periods > MAX_PERIODS {
            return Err(refuse(format!(
                "toposort: {periods} periods; at least 1 and at most {MAX_PERIODS}"
            )));
        }
        if usage.len() != limits.len() {
            return Err(refuse(format!(
                "toposort: {} resources have uses and {} have limits",
                usage.len(),
                limits.len()
            )));
        }
        let n = blocks.node_count();
        for (r, (uses, caps)) in usage.iter().zip(&limits).enumerate() {
            if uses.len() != n || caps.len() != periods {
                return Err(refuse(format!(
                    "toposort: resource {r} has {} uses for {n} blocks and {} limits for {periods} periods",
                    uses.len(),
                    caps.len()
                )));
            }
            if let Some(b) = uses.iter().position(|&q| q < 0) {
                return Err(refuse(format!(
                    "toposort: block {b} uses a negative amount of resource {r}"
                )));
            }
            if let Some(t) = caps.iter().position(|&q| q < 0) {
                return Err(refuse(format!(
                    "toposort: resource {r} has a negative limit in period {t}"
                )));
            }
        }
        Ok(Self {
            blocks,
            periods,
            usage,
            limits,
        })
    }

    /// The block values and requirements.
    #[must_use]
    pub fn blocks(&self) -> &ClosureProblem {
        &self.blocks
    }

    /// Number of periods.
    #[must_use]
    pub fn periods(&self) -> usize {
        self.periods
    }

    /// Number of resources.
    #[must_use]
    pub fn resources(&self) -> usize {
        self.usage.len()
    }

    /// Each block's use of resource `r`; `None` when there is no resource `r`.
    #[must_use]
    pub fn usage(&self, r: usize) -> Option<&[i64]> {
        self.usage.get(r).map(Vec::as_slice)
    }

    /// Resource `r`'s limit in each period; `None` when there is no resource `r`.
    #[must_use]
    pub fn limits(&self, r: usize) -> Option<&[i64]> {
        self.limits.get(r).map(Vec::as_slice)
    }
}

/// What a schedule is, against the problem it was checked on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScheduleCheck {
    /// Blocks mined.
    pub mined: usize,
    /// The value mined in each period, undiscounted.
    pub period_values: Vec<i128>,
    /// `used[r][t]`: how much of resource `r` period `t` uses.
    pub used: Vec<Vec<i128>>,
    /// Requirements `(block, required)` with the block mined and the required block unmined or mined later, counted
    /// per arc.
    pub unmet: usize,
    /// The first [`WITNESSES`] of them, by block then required block.
    pub first_unmet: Vec<(u32, u32)>,
    /// Every `(resource, period)` that uses more than its limit.
    pub over_limit: Vec<(usize, usize)>,
}

impl ScheduleCheck {
    /// Whether every requirement is met and every limit kept.
    #[must_use]
    pub fn feasible(&self) -> bool {
        self.unmet == 0 && self.over_limit.is_empty()
    }

    /// The undiscounted value of all mined blocks.
    #[must_use]
    pub fn value(&self) -> i128 {
        self.period_values.iter().sum()
    }
}

/// PO-PUSH-5: precedence across periods and every limit, with the exact value mined in each period.
///
/// # Errors
/// `FMINE-REF-207` (proposed) when `schedule` does not have one entry per block, or names a period past the last.
pub fn check(
    problem: &ScheduleProblem,
    schedule: &[Option<u32>],
) -> Result<ScheduleCheck, Refusal> {
    let blocks = &problem.blocks;
    let n = blocks.node_count();
    let periods = problem.periods;
    if schedule.len() != n {
        return Err(refuse(format!(
            "schedule check: {} entries for {n} blocks",
            schedule.len()
        )));
    }
    let mut period_values = vec![0_i128; periods];
    let mut used = vec![vec![0_i128; periods]; problem.usage.len()];
    let (mut mined, mut unmet, mut first_unmet) = (0, 0, Vec::new());
    for (b, (&slot, &value)) in schedule.iter().zip(blocks.weights()).enumerate() {
        let Some(t) = slot else {
            continue;
        };
        let period = t as usize;
        if period >= periods {
            return Err(refuse(format!(
                "schedule check: block {b} is in period {t}, but there are {periods}"
            )));
        }
        mined += 1;
        period_values[period] += i128::from(value);
        for (row, uses) in used.iter_mut().zip(&problem.usage) {
            row[period] += i128::from(uses[b]);
        }
        for &q in blocks.requires(b).unwrap_or(&[]) {
            let met = schedule
                .get(q as usize)
                .copied()
                .flatten()
                .is_some_and(|tq| tq <= t);
            if !met {
                unmet += 1;
                if first_unmet.len() < WITNESSES {
                    first_unmet.push((id(b)?, q));
                }
            }
        }
    }
    let mut over_limit = Vec::new();
    for (r, (row, caps)) in used.iter().zip(&problem.limits).enumerate() {
        for (t, (&u, &cap)) in row.iter().zip(caps).enumerate() {
            if u > i128::from(cap) {
                over_limit.push((r, t));
            }
        }
    }
    Ok(ScheduleCheck {
        mined,
        period_values,
        used,
        unmet,
        first_unmet,
        over_limit,
    })
}

/// TopoSort: the feasible schedule this ranking gives (see the module notes). `rank[b]` is block `b`'s rank, lower
/// first; `None` leaves the block unmined.
///
/// # Errors
/// `FMINE-REF-207` (proposed) when `rank` does not have one entry per block, or, as a defect, when [`check`] finds
/// the schedule infeasible.
pub fn greedy(
    problem: &ScheduleProblem,
    rank: &[Option<i64>],
) -> Result<Vec<Option<u32>>, Refusal> {
    let blocks = &problem.blocks;
    let n = blocks.node_count();
    let periods = problem.periods;
    if rank.len() != n {
        return Err(refuse(format!(
            "toposort: {} ranks for {n} blocks",
            rank.len()
        )));
    }
    // The blocks that require each block, in CSR form, and how many requirements each block still waits on.
    let mut waiting = vec![0_usize; n];
    let mut start = vec![0_usize; n + 1];
    for (b, w) in waiting.iter_mut().enumerate() {
        let required = blocks.requires(b).unwrap_or(&[]);
        *w = required.len();
        for &q in required {
            start[q as usize + 1] += 1;
        }
    }
    for i in 0..n {
        start[i + 1] += start[i];
    }
    let mut fill = start.clone();
    let mut dependents = vec![0_u32; start[n]];
    for b in 0..n {
        for &q in blocks.requires(b).unwrap_or(&[]) {
            dependents[fill[q as usize]] = id(b)?;
            fill[q as usize] += 1;
        }
    }
    // A block without a rank is never mined, so where it falls in the order does not matter.
    let key = |b: usize| -> Result<Reverse<(i64, u32)>, Refusal> {
        Ok(Reverse((rank[b].unwrap_or(i64::MAX), id(b)?)))
    };
    let mut ready = BinaryHeap::new();
    for (b, &w) in waiting.iter().enumerate() {
        if w == 0 {
            ready.push(key(b)?);
        }
    }
    let mut schedule: Vec<Option<u32>> = vec![None; n];
    let mut used = vec![vec![0_i128; periods]; problem.usage.len()];
    while let Some(Reverse((_, taken))) = ready.pop() {
        let b = taken as usize;
        if rank[b].is_some() {
            // Every required block was taken before `b`, so its period is final.
            let mut earliest = Some(0_usize);
            for &q in blocks.requires(b).unwrap_or(&[]) {
                earliest = match (earliest, schedule[q as usize]) {
                    (Some(e), Some(tq)) => Some(e.max(tq as usize)),
                    _ => None,
                };
            }
            if let Some(from) = earliest {
                let fits = |t: usize| {
                    used.iter().zip(&problem.usage).zip(&problem.limits).all(
                        |((row, uses), caps)| row[t] + i128::from(uses[b]) <= i128::from(caps[t]),
                    )
                };
                if let Some(t) = (from..periods).find(|&t| fits(t)) {
                    for (row, uses) in used.iter_mut().zip(&problem.usage) {
                        row[t] += i128::from(uses[b]);
                    }
                    schedule[b] = Some(id(t)?);
                }
            }
        }
        for &d in &dependents[start[b]..start[b + 1]] {
            let d = d as usize;
            waiting[d] -= 1;
            if waiting[d] == 0 {
                ready.push(key(d)?);
            }
        }
    }
    let checked = check(problem, &schedule)?;
    if !checked.feasible() {
        return Err(refuse(format!(
            "toposort: the greedy schedule fails its check ({} unmet requirements, limits exceeded at {:?})",
            checked.unmet, checked.over_limit
        )));
    }
    Ok(schedule)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pitflow::solve;

    /// Blocks 0 and 1 are free; block 2 requires both; block 3 requires 2. One resource.
    fn problem(limit: i64) -> ScheduleProblem {
        let blocks = ClosureProblem::from_requirements(
            vec![-2, -3, 10, 4],
            &[vec![], vec![], vec![0, 1], vec![2]],
        )
        .unwrap();
        ScheduleProblem::new(blocks, 3, vec![vec![1, 1, 1, 1]], vec![vec![limit; 3]]).unwrap()
    }

    #[test]
    fn check_counts_late_and_missing_requirements_and_overfull_periods() {
        let p = problem(2);
        let good = check(&p, &[Some(0), Some(0), Some(1), Some(1)]).unwrap();
        assert!(good.feasible());
        assert_eq!(good.period_values, vec![-5, 14, 0]);
        assert_eq!((good.mined, good.value()), (4, 9));
        // Block 2 mined before block 1; block 3 mined without block 2.
        let late = check(&p, &[Some(0), Some(1), Some(0), None]).unwrap();
        assert_eq!((late.unmet, late.first_unmet.clone()), (1, vec![(2, 1)]));
        let missing = check(&p, &[Some(0), Some(0), None, Some(2)]).unwrap();
        assert_eq!(missing.first_unmet, vec![(3, 2)]);
        // The same period is fine; three blocks in one period are not, at a limit of 2.
        let full = check(&p, &[Some(0), Some(0), Some(0), None]).unwrap();
        assert_eq!((full.unmet, full.over_limit), (0, vec![(0, 0)]));
        assert_eq!(check(&p, &[None; 3]).unwrap_err().code, "FMINE-REF-207");
        assert_eq!(
            check(&p, &[Some(3), None, None, None]).unwrap_err().code,
            "FMINE-REF-207"
        );
    }

    #[test]
    fn greedy_follows_rank_within_precedence_and_capacity() {
        // Limit 2: blocks 0 and 1 fill period 0, 2 and 3 go in period 1.
        assert_eq!(
            greedy(&problem(2), &[Some(0); 4]).unwrap(),
            vec![Some(0), Some(0), Some(1), Some(1)]
        );
        // Limit 1: one block a period; block 3 does not fit and stays unmined.
        assert_eq!(
            greedy(&problem(1), &[Some(5), Some(0), Some(0), Some(0)]).unwrap(),
            vec![Some(1), Some(0), Some(2), None]
        );
        // An unranked block leaves out every block that requires it.
        assert_eq!(
            greedy(&problem(9), &[Some(0), None, Some(0), Some(0)]).unwrap(),
            vec![Some(0), None, None, None]
        );
        assert_eq!(
            greedy(&problem(9), &[Some(0); 3]).unwrap_err().code,
            "FMINE-REF-207"
        );
    }

    #[test]
    fn a_precedence_cycle_stays_unmined() {
        let blocks =
            ClosureProblem::from_requirements(vec![5, 5, 1], &[vec![1], vec![0], vec![]]).unwrap();
        let p = ScheduleProblem::new(blocks, 1, vec![], vec![]).unwrap();
        assert_eq!(
            greedy(&p, &[Some(0); 3]).unwrap(),
            vec![None, None, Some(0)]
        );
    }

    #[test]
    fn malformed_problems_refuse() {
        let blocks = || ClosureProblem::from_requirements(vec![1, 2], &[vec![], vec![0]]).unwrap();
        for (periods, usage, limits) in [
            (0, vec![], vec![]),
            (MAX_PERIODS + 1, vec![], vec![]),
            (2, vec![vec![1, 1]], vec![]),
            (2, vec![vec![1]], vec![vec![3, 3]]),
            (2, vec![vec![1, 1]], vec![vec![3]]),
            (2, vec![vec![1, -1]], vec![vec![3, 3]]),
            (2, vec![vec![1, 1]], vec![vec![3, -3]]),
        ] {
            assert_eq!(
                ScheduleProblem::new(blocks(), periods, usage, limits)
                    .unwrap_err()
                    .code,
                "FMINE-REF-207"
            );
        }
        // At the bound and with no resources, every block goes in period 0.
        let p = ScheduleProblem::new(blocks(), MAX_PERIODS, vec![], vec![]).unwrap();
        let s = greedy(&p, &[Some(0), Some(0)]).unwrap();
        assert_eq!(s, vec![Some(0), Some(0)]);
        assert_eq!(check(&p, &s).unwrap().period_values.len(), MAX_PERIODS);
    }

    /// A small deterministic generator, so a failing case can be rebuilt alone.
    fn lcg(seed: u64) -> impl FnMut(u64) -> u64 {
        let mut s = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
        move |m| {
            s = s
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            (s >> 33) % m
        }
    }

    /// A random problem: each block requires some lower-numbered blocks, so the graph is acyclic.
    fn random_problem(seed: u64) -> ScheduleProblem {
        let mut r = lcg(seed);
        let n = 2 + r(5) as usize;
        let periods = 1 + r(3) as usize;
        let resources = r(3) as usize;
        let weights = (0..n).map(|_| r(21) as i64 - 10).collect();
        let requires: Vec<Vec<u32>> = (0..n)
            .map(|b| (0..b as u32).filter(|_| r(3) == 0).collect())
            .collect();
        let usage = (0..resources)
            .map(|_| (0..n).map(|_| r(4) as i64).collect())
            .collect();
        let limits = (0..resources)
            .map(|_| (0..periods).map(|_| r(7) as i64).collect())
            .collect();
        let blocks = ClosureProblem::from_requirements(weights, &requires).unwrap();
        ScheduleProblem::new(blocks, periods, usage, limits).unwrap()
    }

    /// Feasibility written out from the definition, for comparison with [`check`].
    fn feasible_by_definition(p: &ScheduleProblem, s: &[Option<u32>]) -> bool {
        let n = p.blocks().node_count();
        let precedence = (0..n).all(|b| match s[b] {
            None => true,
            Some(t) => p
                .blocks()
                .requires(b)
                .unwrap()
                .iter()
                .all(|&q| match s[q as usize] {
                    Some(tq) => tq <= t,
                    None => false,
                }),
        });
        let capacity = (0..p.resources()).all(|r| {
            (0..p.periods()).all(|t| {
                let total: i64 = (0..n)
                    .filter(|&b| s[b] == Some(t as u32))
                    .map(|b| p.usage(r).unwrap()[b])
                    .sum();
                total <= p.limits(r).unwrap()[t]
            })
        });
        precedence && capacity
    }

    /// Every schedule of `n` blocks over `periods`: each block unmined or in one period.
    fn every_schedule(n: usize, periods: usize) -> Vec<Vec<Option<u32>>> {
        let choices = periods + 1;
        (0..choices.pow(n as u32))
            .map(|mut code| {
                (0..n)
                    .map(|_| {
                        let c = code % choices;
                        code /= choices;
                        (c > 0).then(|| (c - 1) as u32)
                    })
                    .collect()
            })
            .collect()
    }

    #[test]
    fn check_agrees_with_the_definition_on_every_schedule() {
        let (mut agreed, mut feasible_seen, mut infeasible_seen) = (0, 0, 0);
        for seed in 0..60 {
            let p = random_problem(seed);
            for s in every_schedule(p.blocks().node_count(), p.periods()) {
                let c = check(&p, &s).unwrap();
                let expected = feasible_by_definition(&p, &s);
                assert_eq!(c.feasible(), expected, "seed {seed}, schedule {s:?}");
                let value: i128 = s
                    .iter()
                    .zip(p.blocks().weights())
                    .filter(|(t, _)| t.is_some())
                    .map(|(_, &w)| i128::from(w))
                    .sum();
                assert_eq!(c.value(), value);
                agreed += 1;
                if expected {
                    feasible_seen += 1;
                } else {
                    infeasible_seen += 1;
                }
            }
        }
        // Not vacuous: both verdicts occur many times.
        assert!(
            feasible_seen > 1_000 && infeasible_seen > 1_000,
            "{agreed} schedules: {feasible_seen} feasible, {infeasible_seen} not"
        );
    }

    #[test]
    fn greedy_is_feasible_under_any_ranking_and_mines_whatever_fits() {
        let mut mined_some = 0;
        for seed in 0..400 {
            let p = random_problem(seed);
            let n = p.blocks().node_count();
            let mut r = lcg(seed ^ 0xA5A5);
            let rank: Vec<Option<i64>> = (0..n).map(|_| (r(5) > 0).then(|| r(10) as i64)).collect();
            let s = greedy(&p, &rank).unwrap();
            assert!(feasible_by_definition(&p, &s), "seed {seed}: {s:?}");
            // An unmined ranked block had no room: its required blocks were all mined, and every period from theirs
            // on was too full for it.
            for b in 0..n {
                if s[b].is_some() || rank[b].is_none() {
                    continue;
                }
                let required = p.blocks().requires(b).unwrap();
                let Some(from) = required
                    .iter()
                    .map(|&q| s[q as usize].map(|t| t as usize))
                    .try_fold(0, |e, t| t.map(|t| e.max(t)))
                else {
                    continue;
                };
                for t in from..p.periods() {
                    let mut trial = s.clone();
                    trial[b] = Some(t as u32);
                    assert!(
                        !feasible_by_definition(&p, &trial),
                        "seed {seed}: block {b} fits period {t}"
                    );
                }
            }
            mined_some += usize::from(s.iter().any(Option::is_some));
        }
        assert!(mined_some > 300, "{mined_some}");
    }

    #[test]
    fn with_room_to_spare_the_ultimate_pit_is_mined_in_period_zero() {
        for seed in 0..100 {
            let p = random_problem(seed);
            let n = p.blocks().node_count();
            let pit = solve(p.blocks()).unwrap();
            let roomy = ScheduleProblem::new(
                p.blocks().clone(),
                p.periods(),
                vec![vec![1; n]],
                vec![vec![n as i64; p.periods()]],
            )
            .unwrap();
            let rank: Vec<Option<i64>> = pit.in_pit().iter().map(|&i| i.then_some(0)).collect();
            let s = greedy(&roomy, &rank).unwrap();
            let c = check(&roomy, &s).unwrap();
            assert_eq!(c.period_values[0], i128::from(pit.value()), "seed {seed}");
            assert_eq!(c.mined, pit.pit_size(), "seed {seed}");
        }
    }
}
