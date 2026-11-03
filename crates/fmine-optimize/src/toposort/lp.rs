//! Determinism: BitwisePortable (IEEE `f64` `+ − × ÷`, `round` and comparisons; the discount power is taken by
//! repeated squaring, the product order `powi` uses, without a libm call). The schedules are checked in `i128`.
//!
//! Schedules from an LP relaxation (plan §15.4 row `fmine-optimize::toposort`, feasible incumbents from LP solutions;
//! `bd-ubzs`). This crate formulates the relaxation and reads its solution but does not solve it, since it does not
//! depend on `fmine-lp`: [`Relaxation`]'s parts are what `fmine_lp::pdlp::LpProblem::new` takes. PCPSP's version is
//! [`super::pcpsp::lp`].
//!
//! 1. [`relaxation`]: the LP in "by" variables `w[b][t]`, the share of block `b` mined by period `t`, at column
//!    `t·n + b`, each in `[0, 1]`. Its rows are `w[b][t] ≤ w[q][t]` for each block `b` and block `q` it requires,
//!    `w[b][t − 1] ≤ w[b][t]`, and, for each resource and period, `Σ_b use[b] · (w[b][t] − w[b][t − 1]) ≤ limit[t]`.
//!    It maximises `Σ_{b,t} (value[b] · f_t − value[b] · f_{t+1}) · w[b][t]`, with `f_t = 1 / (1 + rate)^t` counted
//!    from period 0 and `f_T = 0`; the cost is the negative, a minimisation. Values, uses and limits are back in
//!    units: the problem's integers divided by `10^decimals`, the places they were scaled at.
//! 2. [`Shares`] holds a solution's `w`. It refuses a share that is not a number in `[0, 1]` rather than clamp it.
//! 3. [`starts`]: TopoSort's six schedules. A block is ranked by its LP expected period, `Σ_t (1 − w[b][t])` at 10⁹
//!    to the period, and left out when the LP has mined less than a threshold of it by the last period: 10⁻⁶, 0.5 or
//!    0.9 ([`THRESHOLDS`]). Each threshold gives [`super::greedy`]'s schedule, and [`super::greedy_from`]'s with each
//!    block released at its LP median period (`bd-wife`): the first period by which the LP has mined half of it, or
//!    the last. The LP mines a required block at least as fast as the block itself, so no block's median comes before
//!    that of a block it requires.
//! 4. [`incumbent`] runs [`super::search::improve`] on each start and takes the first with the highest discounted
//!    value.

use fmine_evidence::Refusal;
use fmine_numerics::sparse::{Coo, Csr};

use super::search::{Improvement, improve};
use super::{MAX_PERIODS, ScheduleProblem, greedy, greedy_from, refuse};
use crate::pitflow::minelib::MAX_DECIMALS;

/// The final shares below which [`starts`] leaves a block out, one pair of schedules each.
pub const THRESHOLDS: [f64; 3] = [1e-6, 0.5, 0.9];

/// The share by which a block's LP median period is counted.
const MEDIAN: f64 = 0.5;

/// Ranks are the LP expected period at this many to the period.
const RANK_SCALE: f64 = 1e9;

/// An LP as a minimisation: `cost · x` subject to `row_lo ≤ matrix · x ≤ row_hi` and `col_lo ≤ x ≤ col_hi`.
#[derive(Debug, Clone, PartialEq)]
pub struct Relaxation {
    /// One cost per column.
    pub cost: Vec<f64>,
    /// The constraint matrix.
    pub matrix: Csr,
    /// Each row's lower bound; `-∞` when it has none.
    pub row_lo: Vec<f64>,
    /// Each row's upper bound.
    pub row_hi: Vec<f64>,
    /// Each column's lower bound.
    pub col_lo: Vec<f64>,
    /// Each column's upper bound.
    pub col_hi: Vec<f64>,
}

/// `base^exp` by repeated squaring, in the product order of `powi` (compiler-builtins' `__powidf2`), so the costs
/// are those `powi` would give.
fn power(mut base: f64, mut exp: usize) -> f64 {
    let mut product = 1.0;
    loop {
        if exp & 1 == 1 {
            product *= base;
        }
        exp >>= 1;
        if exp == 0 {
            return product;
        }
        base *= base;
    }
}

/// `f_t = 1 / (1 + rate)^t` for each period, the rate being `rate / rate_scale`, and `10^decimals`.
pub(super) fn factors(
    rate: i64,
    rate_scale: i64,
    decimals: u32,
    periods: usize,
) -> Result<(Vec<f64>, f64), Refusal> {
    if rate < 0 || rate_scale <= 0 {
        return Err(refuse(format!(
            "toposort lp: discount rate {rate} / {rate_scale}; a rate of at least 0 over a scale above 0"
        )));
    }
    if decimals > MAX_DECIMALS {
        return Err(refuse(format!(
            "toposort lp: {decimals} decimal places; at most {MAX_DECIMALS}"
        )));
    }
    let rate = rate as f64 / rate_scale as f64;
    // Exact: 10⁹ is far below 2⁵³.
    let scale = (0..decimals).fold(1.0, |s, _| s * 10.0);
    Ok((
        (0..periods).map(|t| 1.0 / power(1.0 + rate, t)).collect(),
        scale,
    ))
}

/// `Σ a · b` over the pairs, or `None` on overflow.
pub(super) fn count(terms: &[(usize, usize)]) -> Option<usize> {
    terms
        .iter()
        .try_fold(0_usize, |sum, &(a, b)| sum.checked_add(a.checked_mul(b)?))
}

/// One staged entry, refused if it falls outside the matrix.
pub(super) fn put(coo: &mut Coo, row: usize, col: usize, val: f64) -> Result<(), Refusal> {
    coo.push(row, col, val)
        .map_err(|e| refuse(format!("toposort lp: {e}")))
}

/// The relaxation of `problem` (see the module notes). The discount rate per period is `rate / rate_scale`, as for
/// [`super::search::discount_factors`], and the problem's values are at `10^decimals` per unit, as
/// [`super::cpit::parse`] scales them. Uses and limits are divided by the same: that scales each resource row alike,
/// so the LP's solutions and value are the same whatever places they were read at.
///
/// # Errors
/// `FMINE-REF-207` (proposed) when `rate` is negative, `rate_scale` is not above 0 or `decimals` is above
/// [`MAX_DECIMALS`], or when the LP's size does not fit `usize`.
pub fn relaxation(
    problem: &ScheduleProblem,
    rate: i64,
    rate_scale: i64,
    decimals: u32,
) -> Result<Relaxation, Refusal> {
    let blocks = problem.blocks();
    let (n, periods) = (blocks.node_count(), problem.periods());
    let (factor, scale) = factors(rate, rate_scale, decimals, periods)?;
    let col = |b: usize, t: usize| t * n + b;
    let arcs: usize = (0..n)
        .map(|b| blocks.requires(b).map_or(0, <[u32]>::len))
        .sum();
    let too_big = || refuse(format!("toposort lp: {n} blocks over {periods} periods"));
    let columns = count(&[(n, periods)]).ok_or_else(too_big)?;
    let rows = count(&[
        (arcs, periods),
        (n, periods - 1),
        (problem.resources(), periods),
    ])
    .ok_or_else(too_big)?;
    let mut coo = Coo::new(rows, columns).map_err(|e| refuse(format!("toposort lp: {e}")))?;
    let (mut lo, mut hi) = (Vec::with_capacity(rows), Vec::with_capacity(rows));
    let mut r = 0;
    for t in 0..periods {
        for b in 0..n {
            for &q in blocks.requires(b).unwrap_or(&[]) {
                put(&mut coo, r, col(b, t), 1.0)?;
                put(&mut coo, r, col(q as usize, t), -1.0)?;
                lo.push(f64::NEG_INFINITY);
                hi.push(0.0);
                r += 1;
            }
        }
    }
    for t in 1..periods {
        for b in 0..n {
            put(&mut coo, r, col(b, t - 1), 1.0)?;
            put(&mut coo, r, col(b, t), -1.0)?;
            lo.push(f64::NEG_INFINITY);
            hi.push(0.0);
            r += 1;
        }
    }
    for res in 0..problem.resources() {
        let (uses, caps) = (
            problem.usage(res).unwrap_or(&[]),
            problem.limits(res).unwrap_or(&[]),
        );
        for (t, &cap) in caps.iter().enumerate() {
            for (b, &q) in uses.iter().enumerate().filter(|(_, q)| **q != 0) {
                let q = q as f64 / scale;
                put(&mut coo, r, col(b, t), q)?;
                if t > 0 {
                    put(&mut coo, r, col(b, t - 1), -q)?;
                }
            }
            lo.push(f64::NEG_INFINITY);
            hi.push(cap as f64 / scale);
            r += 1;
        }
    }
    let mut cost = vec![0.0; columns];
    for (b, &w) in blocks.weights().iter().enumerate() {
        let profit = w as f64 / scale;
        for t in 0..periods {
            let next = factor.get(t + 1).map_or(0.0, |f| profit * f);
            cost[col(b, t)] = -(profit * factor[t] - next);
        }
    }
    Ok(Relaxation {
        cost,
        matrix: coo.assemble(),
        row_lo: lo,
        row_hi: hi,
        col_lo: vec![0.0; columns],
        col_hi: vec![1.0; columns],
    })
}

/// An LP solution's by-period shares: how much of each block the LP has mined by each period.
#[derive(Debug, Clone, PartialEq)]
pub struct Shares {
    blocks: usize,
    periods: u32,
    by: Vec<f64>,
}

impl Shares {
    /// `by[t · blocks + b]` is the share of block `b` mined by period `t`, the layout of [`relaxation`]'s columns.
    ///
    /// # Errors
    /// `FMINE-REF-207` (proposed) when there are no periods or more than [`MAX_PERIODS`], when `by` does not have one
    /// share per block and period, or when a share is not a number in `[0, 1]`.
    pub fn new(blocks: usize, periods: usize, by: Vec<f64>) -> Result<Self, Refusal> {
        if periods == 0 || periods > MAX_PERIODS {
            return Err(refuse(format!(
                "toposort lp: {periods} periods; at least 1 and at most {MAX_PERIODS}"
            )));
        }
        if blocks.checked_mul(periods) != Some(by.len()) {
            return Err(refuse(format!(
                "toposort lp: {} shares for {blocks} blocks over {periods} periods",
                by.len()
            )));
        }
        if let Some(i) = by.iter().position(|s| !(0.0..=1.0).contains(s)) {
            return Err(refuse(format!(
                "toposort lp: block {}'s share by period {} is {}, not in [0, 1]",
                i % blocks,
                i / blocks,
                by[i]
            )));
        }
        let periods = u32::try_from(periods)
            .map_err(|_| refuse(format!("toposort lp: {periods} periods do not fit u32")))?;
        Ok(Self {
            blocks,
            periods,
            by,
        })
    }

    /// The blocks.
    #[must_use]
    pub fn blocks(&self) -> usize {
        self.blocks
    }

    /// The periods.
    #[must_use]
    pub fn periods(&self) -> usize {
        self.periods as usize
    }

    /// The share of block `b` mined by period `t`, if both exist.
    #[must_use]
    pub fn share(&self, b: usize, t: usize) -> Option<f64> {
        if b >= self.blocks {
            return None;
        }
        self.by
            .get(t.checked_mul(self.blocks)?.checked_add(b)?)
            .copied()
    }

    fn at(&self, b: usize, t: u32) -> f64 {
        self.by[t as usize * self.blocks + b]
    }

    /// Block `b`'s LP expected number of periods before it is mined, `Σ_t (1 − w[b][t])`.
    #[must_use]
    pub fn expected(&self, b: usize) -> Option<f64> {
        (b < self.blocks).then(|| (0..self.periods).map(|t| 1.0 - self.at(b, t)).sum())
    }

    /// Each block's rank for [`super::greedy`], its expected period at 10⁹ to the period, or `None` when the LP has
    /// mined less than `threshold` of it by the last period. Crate-only: [`starts`] takes its thresholds from
    /// [`THRESHOLDS`], and a public `f64` threshold would need an L-6 exception.
    #[must_use]
    pub(crate) fn ranks(&self, threshold: f64) -> Vec<Option<i64>> {
        let last = self.periods - 1;
        (0..self.blocks)
            .map(|b| {
                (self.at(b, last) >= threshold).then(|| {
                    let expected: f64 = (0..self.periods).map(|t| 1.0 - self.at(b, t)).sum();
                    (expected * RANK_SCALE).round() as i64
                })
            })
            .collect()
    }

    /// Each block's LP median period: the first by which the LP has mined half of it, or the last.
    #[must_use]
    pub fn medians(&self) -> Vec<u32> {
        (0..self.blocks)
            .map(|b| {
                (0..self.periods)
                    .find(|&t| self.at(b, t) >= MEDIAN)
                    .unwrap_or(self.periods - 1)
            })
            .collect()
    }
}

/// One of [`starts`]'s schedules.
#[derive(Debug, Clone, PartialEq)]
pub struct Start {
    /// The final share below which a block was left out, one of [`THRESHOLDS`].
    pub threshold: f64,
    /// Whether each block was released at its LP median period ([`super::greedy_from`]).
    pub released: bool,
    /// The schedule. It passes [`super::check`].
    pub schedule: Vec<Option<u32>>,
}

/// TopoSort's six schedules from an LP solution (see the module notes): for each of [`THRESHOLDS`], the earliest
/// placement and then the one released at the LP medians.
///
/// # Errors
/// `FMINE-REF-207` (proposed) when `shares` does not have `problem`'s blocks and periods, or as [`super::greedy`]
/// refuses.
pub fn starts(problem: &ScheduleProblem, shares: &Shares) -> Result<Vec<Start>, Refusal> {
    let n = problem.blocks().node_count();
    if shares.blocks() != n || shares.periods() != problem.periods() {
        return Err(refuse(format!(
            "toposort lp: shares for {} blocks over {} periods, for a problem of {n} over {}",
            shares.blocks(),
            shares.periods(),
            problem.periods()
        )));
    }
    let medians = shares.medians();
    let mut out = Vec::with_capacity(2 * THRESHOLDS.len());
    for threshold in THRESHOLDS {
        let rank = shares.ranks(threshold);
        for released in [false, true] {
            let schedule = if released {
                greedy_from(problem, &rank, &medians)?
            } else {
                greedy(problem, &rank)?
            };
            out.push(Start {
                threshold,
                released,
                schedule,
            });
        }
    }
    Ok(out)
}

/// The index of the first highest of `values`, if there is one.
pub(super) fn first_best(values: impl IntoIterator<Item = i128>) -> Option<usize> {
    let mut best: Option<(usize, i128)> = None;
    for (i, v) in values.into_iter().enumerate() {
        if best.is_none_or(|(_, b)| v > b) {
            best = Some((i, v));
        }
    }
    best.map(|(i, _)| i)
}

/// What [`incumbent`] returns.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Incumbent {
    /// [`super::search::improve`]'s result from each start, in the starts' order.
    pub searched: Vec<Improvement>,
    /// The index in `searched` of the first with the highest discounted value.
    pub best: usize,
}

impl Incumbent {
    /// The best of the searched schedules.
    #[must_use]
    pub fn schedule(&self) -> &[Option<u32>] {
        self.searched
            .get(self.best)
            .map_or(&[], |i| i.schedule.as_slice())
    }
}

/// Each start searched by [`super::search::improve`], with `factors` and `max_rounds` as it takes them, and the best.
///
/// # Errors
/// `FMINE-REF-207` (proposed) when there are no starts, or as [`super::search::improve`] refuses.
pub fn incumbent(
    problem: &ScheduleProblem,
    starts: &[Start],
    factors: &[i64],
    max_rounds: usize,
) -> Result<Incumbent, Refusal> {
    let searched = starts
        .iter()
        .map(|s| improve(problem, &s.schedule, factors, max_rounds))
        .collect::<Result<Vec<_>, _>>()?;
    let best = first_best(searched.iter().map(|i| i.after))
        .ok_or_else(|| refuse("toposort lp: no starts to search".to_string()))?;
    Ok(Incumbent { searched, best })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pitflow::ClosureProblem;
    use crate::toposort::check;
    use crate::toposort::search::discount_factors;

    /// Three blocks over two periods: block 2 (value 5) requires block 0 (value −1); block 1 (value 3) stands alone.
    /// One resource, a use of 1 per block, at most 2 per period.
    fn small() -> ScheduleProblem {
        let blocks =
            ClosureProblem::from_requirements(vec![-1, 3, 5], &[vec![], vec![], vec![0]]).unwrap();
        ScheduleProblem::new(blocks, 2, vec![vec![1, 1, 1]], vec![vec![2, 2]]).unwrap()
    }

    #[test]
    fn power_matches_powi_bit_for_bit() {
        for rate in [0.0, 0.08, 0.1, 0.15, 1.0 / 3.0, 0.123_456_789] {
            for t in 0..200_usize {
                let expected = (1.0_f64 + rate).powi(t as i32);
                assert_eq!(
                    power(1.0 + rate, t).to_bits(),
                    expected.to_bits(),
                    "rate {rate}, t {t}"
                );
            }
        }
    }

    #[test]
    fn the_relaxation_of_a_small_case_has_each_row_and_cost() {
        // A rate of 1/4 per period, the integers in whole units.
        let r = relaxation(&small(), 1, 4, 0).unwrap();
        // Columns w[b][t] at t·3 + b; rows: one arc per period (2), monotone (3), the resource per period (2).
        assert_eq!((r.matrix.nrows(), r.matrix.ncols()), (7, 6));
        let row = |i: usize| {
            let (cols, vals) = r.matrix.row(i).unwrap();
            cols.iter()
                .copied()
                .zip(vals.iter().copied())
                .collect::<Vec<_>>()
        };
        assert_eq!(row(0), [(0, -1.0), (2, 1.0)]);
        assert_eq!(row(1), [(3, -1.0), (5, 1.0)]);
        assert_eq!(row(2), [(0, 1.0), (3, -1.0)]);
        assert_eq!(row(4), [(2, 1.0), (5, -1.0)]);
        assert_eq!(row(5), [(0, 1.0), (1, 1.0), (2, 1.0)]);
        assert_eq!(
            row(6),
            [
                (0, -1.0),
                (1, -1.0),
                (2, -1.0),
                (3, 1.0),
                (4, 1.0),
                (5, 1.0)
            ]
        );
        assert_eq!(r.row_lo, [f64::NEG_INFINITY; 7]);
        assert_eq!(r.row_hi, [0.0, 0.0, 0.0, 0.0, 0.0, 2.0, 2.0]);
        // f = [1, 0.8]: period 0 earns v·(1 − 0.8) for being mined by then, period 1 earns v·0.8.
        let expected = [0.2, -0.6, -1.0, 0.8, -2.4, -4.0];
        for (c, e) in r.cost.iter().zip(expected) {
            assert!((c - e).abs() < 1e-15, "{:?}", r.cost);
        }
        assert_eq!((r.col_lo, r.col_hi), (vec![0.0; 6], vec![1.0; 6]));
        // At one decimal place the integers are tenths: limits and costs a tenth as large, the matrix unchanged
        // except for the uses.
        let tenths = relaxation(&small(), 1, 4, 1).unwrap();
        assert_eq!((tenths.row_hi[5], tenths.row_hi[6]), (0.2, 0.2));
        assert_eq!(tenths.matrix.get(5, 0), Some(0.1));
        assert!((tenths.cost[5] - -0.4).abs() < 1e-15, "{:?}", tenths.cost);
    }

    #[test]
    fn the_relaxation_refuses_a_bad_rate_or_places() {
        for (rate, rate_scale, decimals, why) in [
            (-1, 4, 0, "discount rate -1 / 4"),
            (1, 0, 0, "discount rate 1 / 0"),
            (1, -4, 0, "discount rate 1 / -4"),
            (1, 4, MAX_DECIMALS + 1, "10 decimal places"),
        ] {
            let e = relaxation(&small(), rate, rate_scale, decimals).unwrap_err();
            assert!(e.to_string().contains(why), "{e}");
        }
    }

    #[test]
    fn shares_refuse_what_is_not_a_share_in_range() {
        assert!(Shares::new(2, 1, vec![0.0, 1.0]).is_ok());
        for (blocks, periods, by, why) in [
            (2, 1, vec![0.0], "1 shares for 2 blocks"),
            (2, 0, vec![], "0 periods"),
            (
                1,
                2,
                vec![0.5, f64::NAN],
                "block 0's share by period 1 is NaN",
            ),
            (2, 1, vec![0.0, 1.0 + 1e-12], "block 1's share by period 0"),
            (2, 1, vec![-1e-300, 0.0], "block 0's share by period 0"),
        ] {
            let e = Shares::new(blocks, periods, by).unwrap_err();
            assert!(e.to_string().contains(why), "{e}");
        }
    }

    #[test]
    fn ranks_and_medians_follow_the_lp() {
        // Block 0 is mined by period 0, block 1 half by period 1, block 2 a tenth by period 2, block 3 never.
        let s = Shares::new(
            4,
            3,
            vec![1.0, 0.0, 0.0, 0.0, 1.0, 0.5, 0.0, 0.0, 1.0, 0.75, 0.1, 0.0],
        )
        .unwrap();
        assert_eq!(s.expected(1), Some(1.75));
        assert_eq!(s.expected(4), None);
        assert_eq!(s.share(2, 2), Some(0.1));
        assert_eq!(s.share(0, 3), None);
        assert_eq!(s.medians(), [0, 1, 2, 2]);
        assert_eq!(
            s.ranks(1e-6),
            [Some(0), Some(1_750_000_000), Some(2_900_000_000), None]
        );
        assert_eq!(s.ranks(0.5), [Some(0), Some(1_750_000_000), None, None]);
        assert_eq!(s.ranks(0.9), [Some(0), None, None, None]);
    }

    #[test]
    fn starts_and_the_incumbent_on_a_small_case() {
        let p = small();
        // The LP mines block 0 and half of blocks 1 and 2 in period 0, the rest in period 1. Blocks 1 and 2 tie on
        // rank, and every median is period 0.
        let s = Shares::new(3, 2, vec![1.0, 0.5, 0.5, 1.0, 1.0, 1.0]).unwrap();
        assert_eq!(s.medians(), [0, 0, 0]);
        let starts = starts(&p, &s).unwrap();
        assert_eq!(starts.len(), 6);
        let labels: Vec<(f64, bool)> = starts.iter().map(|s| (s.threshold, s.released)).collect();
        assert_eq!(
            labels,
            [
                (1e-6, false),
                (1e-6, true),
                (0.5, false),
                (0.5, true),
                (0.9, false),
                (0.9, true)
            ]
        );
        for s in &starts {
            assert!(check(&p, &s.schedule).unwrap().feasible());
            // Block 0 goes first, then block 1 on the tie by id; they fill period 0, and block 2 waits for period 1.
            assert_eq!(s.schedule, [Some(0), Some(0), Some(1)]);
        }
        let factors = discount_factors(1, 4, 2).unwrap();
        let i = incumbent(&p, &starts, &factors, 10).unwrap();
        assert_eq!(i.searched.len(), 6);
        assert_eq!(i.best, 0);
        // Block 2 before block 1 is worth more: 5 + (−1) now and 3 later beats 3 + (−1) now and 5 later.
        assert_eq!(i.schedule(), [Some(0), Some(1), Some(0)]);
        assert!(i.searched.iter().all(|x| x.after >= x.before));
    }

    #[test]
    fn the_incumbent_is_the_first_of_the_best() {
        assert_eq!(first_best([3, 7, 7, 1]), Some(1));
        assert_eq!(first_best([-5]), Some(0));
        assert_eq!(first_best(Vec::<i128>::new()), None);
        let p = small();
        let e = incumbent(&p, &[], &discount_factors(1, 4, 2).unwrap(), 10).unwrap_err();
        assert!(e.to_string().contains("no starts"), "{e}");
        let wrong = Shares::new(2, 2, vec![0.0; 4]).unwrap();
        let e = starts(&p, &wrong).unwrap_err();
        assert!(e.to_string().contains("shares for 2 blocks"), "{e}");
    }
}
