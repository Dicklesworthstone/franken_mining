//! Determinism: BitwisePortable (IEEE `f64` `+ − × ÷` and comparisons, as in [`crate::toposort::lp`]; the pit in
//! `i64`). The schedules are checked in `i128`.
//!
//! PCPSP schedules from an LP relaxation (`bd-ubzs`), as [`crate::toposort::lp`] gives them for a
//! [`crate::toposort::ScheduleProblem`]. This crate formulates and reads; the caller solves.
//!
//! 1. [`scope`]: the blocks the LP needs. They are the ultimate pit `P` under each block's most valuable destination,
//!    `w_b = max_d value[b][d]`, from [`crate::pitflow::solve`] with its certificate checked by
//!    [`crate::pitflow::verify`]. That leaves the LP's optimum, and the schedule problem's, unchanged. For a closed set
//!    `T`, `T ∪ P` is closed, so `w(T \ P) ≤ 0`. Summed by parts, a schedule is worth `Σ_t (f_t − f_{t+1}) u(T_t)`,
//!    where `T_t` is what is mined by period `t`, `u_b ≤ w_b` is the value at the destinations used and `f` falls
//!    (`f_T = 0`). Cutting every `T_t` down to `T_t ∩ P` therefore never lowers the value, and with uses non-negative
//!    and limits upper it stays feasible. The LP's by-period shares are mixtures of closed sets (their level sets),
//!    so the same holds there, and a bound on the smaller LP bounds the whole instance. With a negative use (w23's
//!    blending rows), a block outside `P` can make the room that lets ore be milled, so cutting `T_t` down can break a
//!    limit; the scope is then every block.
//! 2. [`relaxation`]: over the scope's blocks, column `i` being block `blocks[i]`. The "by" shares `w[i][t]` are at
//!    `t·n + i`, as in [`crate::toposort::lp`]. After them, `y[i][d][t]`, the share of the block sent to destination
//!    `d` in period `t`, is at `n·T + (t·n + i)·D + d`, with `Σ_d y[i][d][t] = w[i][t] − w[i][t − 1]`. Limits apply
//!    to `Σ_{i,d} use[r][b][d] · y[i][d][t]`. The value is `Σ value[b][d] · y[i][d][t] · f_t`, `f_t = 1 / (1 + rate)^t`
//!    from period 0, at the model's rate, and the cost is its negative. Values, uses and limits are back in units:
//!    the model's integers divided by `10^decimals`.
//! 3. [`Solution`] reads a solution back over every block; a block outside the scope is never mined. Each block goes
//!    whole to the destination the LP sends most of it to, the lowest of equals. A block the LP sends less than
//!    10⁻⁹ of in all goes to its most valuable destination, the lowest of equals.
//! 4. [`starts`] and [`incumbent`] are [`crate::toposort::lp`]'s, with [`super::greedy`], [`super::greedy_from`] and
//!    [`super::improve`].

use std::cmp::Reverse;

use fmine_evidence::Refusal;
use fmine_numerics::sparse::Coo;

use super::{PcpspImprovement, PcpspModel, greedy, greedy_from, improve};
use crate::pitflow::{ClosureProblem, solve, verify};
use crate::toposort::cpit::RATE_SCALE;
use crate::toposort::lp::{Relaxation, Shares, THRESHOLDS, count, factors, first_best, put};
use crate::toposort::refuse;

/// A block the LP sends less than this of in all goes to its most valuable destination.
const SENT: f64 = 1e-9;

/// The blocks [`relaxation`] needs (see the module notes).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Scope {
    /// The blocks in the LP, in id order. Column `i` of [`relaxation`] is block `blocks[i]`.
    pub blocks: Vec<usize>,
    /// How many blocks the ultimate pit has.
    pub pit_blocks: usize,
    /// The ultimate pit's value at each block's most valuable destination, checked by its certificate.
    pub pit_value: i64,
    /// Whether the LP has every block because a use is negative.
    pub whole: bool,
}

/// The LP's blocks: the ultimate pit under each block's most valuable destination, or every block when a use is
/// negative (see the module notes).
///
/// # Errors
/// `FMINE-REF-207` (proposed) when a block has no destination, as [`crate::pitflow::solve`] and
/// [`crate::pitflow::verify`] refuse, or, as a defect, when the certificate's value is not the pit's.
pub fn scope(model: &PcpspModel) -> Result<Scope, Refusal> {
    let (n, dn) = (model.blocks(), model.destinations());
    let best = (0..n)
        .map(|b| {
            (0..dn)
                .filter_map(|d| model.value(b, d))
                .max()
                .ok_or_else(|| refuse(format!("pcpsp lp: block {b} has no destination")))
        })
        .collect::<Result<Vec<i64>, Refusal>>()?;
    let requires: Vec<Vec<u32>> = (0..n)
        .map(|b| model.requires(b).unwrap_or(&[]).to_vec())
        .collect();
    let closure = ClosureProblem::from_requirements(best, &requires)?;
    let pit = solve(&closure)?;
    let certified = verify(&closure, pit.in_pit(), pit.arc_flow())?;
    if certified != pit.value() {
        return Err(refuse(format!(
            "pcpsp lp: defect, the pit's certificate is worth {certified}, not its {}",
            pit.value()
        )));
    }
    let in_pit: Vec<usize> = (0..n).filter(|&b| pit.in_pit()[b]).collect();
    let negative = (0..model.resources())
        .any(|r| (0..n).any(|b| (0..dn).any(|d| model.uses(r, b, d).is_some_and(|q| q < 0))));
    Ok(Scope {
        pit_blocks: in_pit.len(),
        pit_value: pit.value(),
        whole: negative,
        blocks: if negative { (0..n).collect() } else { in_pit },
    })
}

/// Each block's column among `blocks`, which must be in increasing id order and within the model.
fn columns(model: &PcpspModel, blocks: &[usize]) -> Result<Vec<Option<usize>>, Refusal> {
    let n = model.blocks();
    if blocks.windows(2).any(|w| w[0] >= w[1]) || blocks.last().is_some_and(|&b| b >= n) {
        return Err(refuse(format!(
            "pcpsp lp: the LP's blocks are not in increasing order among the model's {n}"
        )));
    }
    let mut index = vec![None; n];
    for (i, &b) in blocks.iter().enumerate() {
        index[b] = Some(i);
    }
    Ok(index)
}

/// The relaxation over `blocks` (see the module notes), at the model's own discount rate. The model's values are at
/// `10^decimals` per unit, the places [`super::parse`] scaled them at. Uses and limits are divided by the same: that
/// scales each resource row alike, so the LP's solutions and value are the same whatever places they were read at.
/// `blocks` is normally [`scope`]'s.
///
/// # Errors
/// `FMINE-REF-207` (proposed) when `blocks` is not in increasing order within the model, when a block in it requires
/// one that is not, when the model's rate is negative, when `decimals` is above
/// [`crate::pitflow::minelib::MAX_DECIMALS`], or when the LP's size does not fit `usize`.
pub fn relaxation(
    model: &PcpspModel,
    blocks: &[usize],
    decimals: u32,
) -> Result<Relaxation, Refusal> {
    let index = columns(model, blocks)?;
    let (n, periods, dn) = (blocks.len(), model.periods(), model.destinations());
    let (factor, scale) = factors(model.discount_rate, RATE_SCALE, decimals, periods)?;
    let too_big = || refuse(format!("pcpsp lp: {n} blocks over {periods} periods"));
    let by = count(&[(n, periods)]).ok_or_else(too_big)?;
    let all = count(&[(by, 1 + dn)]).ok_or_else(too_big)?;
    let w = |i: usize, t: usize| t * n + i;
    let y = |i: usize, d: usize, t: usize| by + (t * n + i) * dn + d;
    let arcs: usize = blocks
        .iter()
        .map(|&b| model.requires(b).map_or(0, <[u32]>::len))
        .sum();
    let rows = count(&[
        (arcs, periods),
        (n, periods - 1),
        (n, periods),
        (model.resources(), periods),
    ])
    .ok_or_else(too_big)?;
    let mut coo = Coo::new(rows, all).map_err(|e| refuse(format!("pcpsp lp: {e}")))?;
    let (mut lo, mut hi) = (Vec::with_capacity(rows), Vec::with_capacity(rows));
    let mut r = 0;
    for t in 0..periods {
        for (i, &b) in blocks.iter().enumerate() {
            for &q in model.requires(b).unwrap_or(&[]) {
                let qi = index[q as usize].ok_or_else(|| {
                    refuse(format!(
                        "pcpsp lp: block {b} requires block {q}, which is not among the LP's blocks"
                    ))
                })?;
                put(&mut coo, r, w(i, t), 1.0)?;
                put(&mut coo, r, w(qi, t), -1.0)?;
                lo.push(f64::NEG_INFINITY);
                hi.push(0.0);
                r += 1;
            }
        }
    }
    for t in 1..periods {
        for i in 0..n {
            put(&mut coo, r, w(i, t - 1), 1.0)?;
            put(&mut coo, r, w(i, t), -1.0)?;
            lo.push(f64::NEG_INFINITY);
            hi.push(0.0);
            r += 1;
        }
    }
    for t in 0..periods {
        for i in 0..n {
            for d in 0..dn {
                put(&mut coo, r, y(i, d, t), 1.0)?;
            }
            put(&mut coo, r, w(i, t), -1.0)?;
            if t > 0 {
                put(&mut coo, r, w(i, t - 1), 1.0)?;
            }
            lo.push(0.0);
            hi.push(0.0);
            r += 1;
        }
    }
    for res in 0..model.resources() {
        for (t, &cap) in model.limits(res).unwrap_or(&[]).iter().enumerate() {
            for (i, &b) in blocks.iter().enumerate() {
                for d in 0..dn {
                    if let Some(q) = model.uses(res, b, d).filter(|&q| q != 0) {
                        put(&mut coo, r, y(i, d, t), q as f64 / scale)?;
                    }
                }
            }
            lo.push(f64::NEG_INFINITY);
            hi.push(cap as f64 / scale);
            r += 1;
        }
    }
    let mut cost = vec![0.0; all];
    for (t, &f) in factor.iter().enumerate() {
        for (i, &b) in blocks.iter().enumerate() {
            for d in 0..dn {
                if let Some(v) = model.value(b, d) {
                    cost[y(i, d, t)] = -(v as f64 / scale) * f;
                }
            }
        }
    }
    Ok(Relaxation {
        cost,
        matrix: coo.assemble(),
        row_lo: lo,
        row_hi: hi,
        col_lo: vec![0.0; all],
        col_hi: vec![1.0; all],
    })
}

/// An LP solution read back over every block: its by-period shares and each block's destination.
#[derive(Debug, Clone, PartialEq)]
pub struct Solution {
    shares: Shares,
    destination: Vec<u32>,
}

impl Solution {
    /// Reads `x`, [`relaxation`]'s columns over `blocks` (see the module notes).
    ///
    /// # Errors
    /// `FMINE-REF-207` (proposed) when `blocks` is not in increasing order within the model, when `x` does not have
    /// one value per column, or when a value is not a number in `[0, 1]`.
    pub fn new(model: &PcpspModel, blocks: &[usize], x: &[f64]) -> Result<Self, Refusal> {
        let index = columns(model, blocks)?;
        let (n, periods, dn) = (model.blocks(), model.periods(), model.destinations());
        let lp = blocks.len();
        let too_big = || refuse(format!("pcpsp lp: {lp} blocks over {periods} periods"));
        let by = count(&[(lp, periods)]).ok_or_else(too_big)?;
        let all = count(&[(by, 1 + dn)]).ok_or_else(too_big)?;
        if x.len() != all {
            return Err(refuse(format!(
                "pcpsp lp: {} values for the {all} columns of {lp} blocks",
                x.len()
            )));
        }
        if let Some(c) = x.iter().position(|v| !(0.0..=1.0).contains(v)) {
            return Err(refuse(format!(
                "pcpsp lp: column {c} is {}, not in [0, 1]",
                x[c]
            )));
        }
        let mut shares = vec![0.0; count(&[(n, periods)]).ok_or_else(too_big)?];
        for t in 0..periods {
            for (i, &b) in blocks.iter().enumerate() {
                shares[t * n + b] = x[t * lp + i];
            }
        }
        let shares = Shares::new(n, periods, shares)?;
        let destination = (0..n)
            .map(|b| {
                let mass = |d: usize| -> f64 {
                    index[b].map_or(0.0, |i| {
                        (0..periods).map(|t| x[by + (t * lp + i) * dn + d]).sum()
                    })
                };
                let d = if (0..dn).map(mass).sum::<f64>() > SENT {
                    (0..dn).max_by(|&a, &c| mass(a).total_cmp(&mass(c)).then(c.cmp(&a)))
                } else {
                    (0..dn).max_by_key(|&d| (model.value(b, d), Reverse(d)))
                };
                d.and_then(|d| u32::try_from(d).ok())
                    .ok_or_else(|| refuse(format!("pcpsp lp: block {b} has no destination")))
            })
            .collect::<Result<Vec<u32>, Refusal>>()?;
        Ok(Self {
            shares,
            destination,
        })
    }

    /// The by-period shares over every block.
    #[must_use]
    pub fn shares(&self) -> &Shares {
        &self.shares
    }

    /// Each block's destination.
    #[must_use]
    pub fn destinations(&self) -> &[u32] {
        &self.destination
    }
}

/// One of [`starts`]'s schedules.
#[derive(Debug, Clone, PartialEq)]
pub struct Start {
    /// The final share below which a block was left out, one of [`THRESHOLDS`].
    pub threshold: f64,
    /// Whether each block was released at its LP median period ([`super::greedy_from`]).
    pub released: bool,
    /// The schedule.
    pub schedule: Vec<Option<u32>>,
    /// Each block's destination, the solution's except for blocks that fell back. With it, the schedule passes
    /// [`crate::toposort::check`].
    pub destination: Vec<u32>,
}

/// TopoSort's six schedules from an LP solution, as [`crate::toposort::lp::starts`], each block trying its
/// solution's destination first.
///
/// # Errors
/// `FMINE-REF-207` (proposed) when `solution` does not have `model`'s blocks and periods, or as [`super::greedy`]
/// refuses.
pub fn starts(model: &PcpspModel, solution: &Solution) -> Result<Vec<Start>, Refusal> {
    let shares = solution.shares();
    if shares.blocks() != model.blocks() || shares.periods() != model.periods() {
        return Err(refuse(format!(
            "pcpsp lp: a solution for {} blocks over {} periods, for a model of {} over {}",
            shares.blocks(),
            shares.periods(),
            model.blocks(),
            model.periods()
        )));
    }
    let medians = shares.medians();
    let mut out = Vec::with_capacity(2 * THRESHOLDS.len());
    for threshold in THRESHOLDS {
        let rank = shares.ranks(threshold);
        for released in [false, true] {
            let (schedule, destination) = if released {
                greedy_from(model, &rank, solution.destinations(), &medians)?
            } else {
                greedy(model, &rank, solution.destinations())?
            };
            out.push(Start {
                threshold,
                released,
                schedule,
                destination,
            });
        }
    }
    Ok(out)
}

/// What [`incumbent`] returns.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Incumbent {
    /// [`super::improve`]'s result from each start, in the starts' order.
    pub searched: Vec<PcpspImprovement>,
    /// The index in `searched` of the first with the highest discounted value.
    pub best: usize,
}

impl Incumbent {
    /// The best of the searched schedules.
    #[must_use]
    pub fn chosen(&self) -> Option<&PcpspImprovement> {
        self.searched.get(self.best)
    }
}

/// Each start searched by [`super::improve`], with `factors` and `max_rounds` as it takes them, and the best.
///
/// # Errors
/// `FMINE-REF-207` (proposed) when there are no starts, or as [`super::improve`] refuses.
pub fn incumbent(
    model: &PcpspModel,
    starts: &[Start],
    factors: &[i64],
    max_rounds: usize,
) -> Result<Incumbent, Refusal> {
    let searched = starts
        .iter()
        .map(|s| improve(model, &s.schedule, &s.destination, factors, max_rounds))
        .collect::<Result<Vec<_>, _>>()?;
    let best = first_best(searched.iter().map(|i| i.after))
        .ok_or_else(|| refuse("pcpsp lp: no starts to search".to_string()))?;
    Ok(Incumbent { searched, best })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pitflow::minelib::Rounding;
    use crate::toposort::check;
    use crate::toposort::pcpsp::parse;
    use crate::toposort::search::discount_factors;

    // Two waste blocks over one ore block, as in `pcpsp`'s tiny pair, and a lone waste block 3 that nothing requires.
    // Destination 0 is the dump and 1 the mill. Resource 0 is mining, two blocks a period at either destination;
    // resource 1 is the mill, one block a period. Values are at one place: block 2 is worth −2 dumped, 9.5 milled.
    fn pair(mill_use_of_block_3: &str) -> PcpspModel {
        let text = format!(
            "NAME: lp\nTYPE: PCPSP\nNBLOCKS: 4\nNPERIODS: 2\nNDESTINATIONS: 2\nNRESOURCE_SIDE_CONSTRAINTS: 2\nNGENERAL_SIDE_CONSTRAINTS: 0\nDISCOUNT_RATE: 0.25\nRESOURCE_CONSTRAINT_LIMITS:\n0 0 L 2\n0 1 L 2\n1 0 L 1\n1 1 L 1\nOBJECTIVE_FUNCTION:\n0 -1 -3\n1 -1 -3\n2 -2 9.5\n3 -1 -2\nRESOURCE_CONSTRAINT_COEFFICIENTS:\n0 0 0 1\n0 1 0 1\n0 1 1 1\n1 0 0 1\n1 1 0 1\n1 1 1 1\n2 0 0 1\n2 1 0 1\n2 1 1 1\n3 0 0 1\n3 1 0 1\n3 1 1 {mill_use_of_block_3}\nEOF\n"
        );
        parse(&text, "0 0\n1 0\n2 2 0 1\n3 0\n", 1, Rounding::Refuse, 0).unwrap()
    }

    /// Block 0 dumped in period 0; block 1 half dumped in each period; block 2 half milled in each.
    fn x() -> Vec<f64> {
        let mut x = vec![1.0, 0.5, 0.5, 1.0, 1.0, 1.0];
        // y[i][d][t] at 6 + (3t + i)·2 + d.
        x.extend([1.0, 0.0, 0.5, 0.0, 0.0, 0.5]);
        x.extend([0.0, 0.0, 0.5, 0.0, 0.0, 0.5]);
        x
    }

    #[test]
    fn the_scope_is_the_pit_unless_a_use_is_negative() {
        let s = scope(&pair("1")).unwrap();
        // Block 3 is worth −1 at best and nothing requires it; the pit is worth −1 − 1 + 9.5 at one place.
        assert_eq!(
            s,
            Scope {
                blocks: vec![0, 1, 2],
                pit_blocks: 3,
                pit_value: 75,
                whole: false
            }
        );
        let s = scope(&pair("-1")).unwrap();
        assert_eq!(
            (s.blocks, s.pit_blocks, s.whole),
            (vec![0, 1, 2, 3], 3, true)
        );
    }

    #[test]
    fn the_relaxation_over_the_pit_has_each_row_and_cost() {
        let m = pair("1");
        // Values at one place; the uses were read at none, and dividing them by 10 too scales each resource row alike.
        let r = relaxation(&m, &[0, 1, 2], 1).unwrap();
        // 3 blocks × 2 periods × (1 + 2) columns; rows: 2 arcs × 2, 3 monotone, 6 balances, 2 resources × 2.
        assert_eq!((r.matrix.nrows(), r.matrix.ncols()), (17, 18));
        let row = |i: usize| {
            let (cols, vals) = r.matrix.row(i).unwrap();
            cols.iter()
                .copied()
                .zip(vals.iter().copied())
                .collect::<Vec<_>>()
        };
        // Block 2 requires blocks 0 and 1, in period 0 and then 1.
        assert_eq!(row(0), [(0, -1.0), (2, 1.0)]);
        assert_eq!(row(3), [(4, -1.0), (5, 1.0)]);
        // Block 0's balance in period 1: y[0][0][1] + y[0][1][1] = w[0][1] − w[0][0].
        assert_eq!(row(10), [(0, 1.0), (3, -1.0), (12, 1.0), (13, 1.0)]);
        assert_eq!((r.row_lo[10], r.row_hi[10]), (0.0, 0.0));
        // Mining in period 0 and the mill in period 0, at a tenth.
        assert_eq!(row(13), (6..12).map(|c| (c, 0.1)).collect::<Vec<_>>());
        assert_eq!(row(15), [(7, 0.1), (9, 0.1), (11, 0.1)]);
        assert_eq!((r.row_hi[13], r.row_hi[15]), (0.2, 0.1));
        // Block 2 milled: 9.5 in period 0, 9.5 · 0.8 in period 1; the by shares cost nothing.
        assert_eq!((r.cost[11], r.cost[17]), (-9.5, -9.5 * 0.8));
        assert_eq!(&r.cost[..6], &[0.0; 6]);
        assert_eq!(r.col_hi, vec![1.0; 18]);
    }

    #[test]
    fn the_relaxation_refuses_blocks_that_are_not_closed_or_in_order() {
        let m = pair("1");
        for (blocks, why) in [
            (&[2][..], "block 2 requires block 0"),
            (&[1, 0][..], "not in increasing order"),
            (&[0, 0][..], "not in increasing order"),
            (&[0, 4][..], "not in increasing order"),
        ] {
            let e = relaxation(&m, blocks, 1).unwrap_err();
            assert!(e.to_string().contains(why), "{blocks:?}: {e}");
        }
        let e = relaxation(&m, &[0, 1, 2], 10).unwrap_err();
        assert!(e.to_string().contains("10 decimal places"), "{e}");
    }

    #[test]
    fn the_solution_sends_each_block_where_the_lp_sends_most_of_it() {
        let m = pair("1");
        let s = Solution::new(&m, &[0, 1, 2], &x()).unwrap();
        // Block 3 is outside the LP: never mined, and sent to its most valuable destination, the dump.
        assert_eq!(s.destinations(), [0, 0, 1, 0]);
        assert_eq!(s.shares().share(3, 1), Some(0.0));
        assert_eq!(s.shares().share(1, 0), Some(0.5));
        assert_eq!(s.shares().medians(), [0, 0, 0, 1]);
        // Block 1 half to each: the lower destination.
        let mut tied = x();
        tied[9] = 0.5;
        tied[14] = 0.0;
        assert_eq!(
            Solution::new(&m, &[0, 1, 2], &tied).unwrap().destinations(),
            [0, 0, 1, 0]
        );
        // Block 1 sent mostly to the mill.
        tied[9] = 0.6;
        tied[8] = 0.4;
        assert_eq!(
            Solution::new(&m, &[0, 1, 2], &tied).unwrap().destinations(),
            [0, 1, 1, 0]
        );
    }

    #[test]
    fn the_solution_refuses_what_is_not_a_share_in_range() {
        let m = pair("1");
        for (x, why) in [
            (vec![0.0; 17], "17 values for the 18 columns"),
            ([vec![1.5], vec![0.0; 17]].concat(), "column 0 is 1.5"),
            ([vec![0.0; 17], vec![f64::NAN]].concat(), "column 17 is NaN"),
        ] {
            let e = Solution::new(&m, &[0, 1, 2], &x).unwrap_err();
            assert!(e.to_string().contains(why), "{e}");
        }
    }

    #[test]
    fn starts_and_the_incumbent_on_the_small_pair() {
        let m = pair("1");
        let s = Solution::new(&m, &[0, 1, 2], &x()).unwrap();
        let starts = starts(&m, &s).unwrap();
        assert_eq!(starts.len(), 6);
        for st in &starts {
            // Blocks 0 and 1 fill period 0's mining; the ore follows in period 1, to the mill. Block 3 has no rank.
            assert_eq!(st.schedule, [Some(0), Some(0), Some(1), None]);
            assert_eq!(st.destination, [0, 0, 1, 0]);
            let p = m.with_destinations(&st.destination).unwrap();
            assert!(check(&p, &st.schedule).unwrap().feasible());
        }
        let factors = discount_factors(m.discount_rate, RATE_SCALE, 2).unwrap();
        let i = incumbent(&m, &starts, &factors, 10).unwrap();
        assert_eq!(i.best, 0);
        let best = i.chosen().unwrap();
        // **Later**, in reverse Kahn order, sends waste block 1 to period 1 beside the ore, which has room for it.
        // Block 0 then finds period 1 full, and the cone of the ore with block 1 does not fit in period 0.
        assert_eq!(best.schedule, [Some(0), Some(1), Some(1), None]);
        assert_eq!(best.destination, [0, 0, 1, 0]);
        assert!(best.after > best.before);
        let e = incumbent(&m, &[], &factors, 10).unwrap_err();
        assert!(e.to_string().contains("no starts"), "{e}");
    }
}
