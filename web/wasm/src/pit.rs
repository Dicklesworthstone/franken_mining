//! `fmine-optimize` for the figures: the certified ultimate pit, nested shells by revenue factor, and TopoSort's
//! greedy schedule, all on a regular block grid with one slope everywhere.
//!
//! **Grid.** As in `pitflow::slope`: block `(i, j, k)` is `(k·ny + j)·nx + i`, `i` east, `j` north, `k = 0` the
//! TOP layer. Block sizes arrive in metres and become whole millimetres, the integer unit the crate's exact cone
//! test needs; a uniform unit change leaves every cone unchanged.
//!
//! **Values.** The crate is integer-only (`i64`). Dollars are multiplied by a power of two and rounded: by default
//! the largest such scale that keeps every intermediate value of the crate far below its 2⁶² guard (the pit:
//! `n · max|v| · scale ≤ 2⁶⁰`, and at most 2⁴⁰ per block). The certificate is exact for the rounded integers;
//! the rounding moves no block by more than half a unit in `1 / scale` dollars. Values come back divided by the
//! scale, which every result reports.

#![forbid(unsafe_code)]

use crate::Output;
use fmine_optimize::pitflow::shells::{RevenueFactor, ShellModel, nested_shells};
use fmine_optimize::pitflow::slope::{BlockGrid, SlopeTable, SlopeTangent, slope_arcs};
use fmine_optimize::pitflow::{ClosureProblem, solve, verify};
use fmine_optimize::toposort::{ScheduleProblem, check, greedy};

/// Revenue factors are exact fractions over this denominator.
pub const FACTOR_DEN: i64 = 10_000;
/// Tonnages and capacities are exact in kilograms.
pub const TONNE_SCALE: f64 = 1_000.0;

/// A block grid with one slope everywhere.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Geometry {
    /// Blocks east, north and down.
    pub counts: [u32; 3],
    /// Block sizes in metres.
    pub sizes_m: [f64; 3],
    /// The slope as the exact tangent `rise / run`.
    pub rise: u32,
    pub run: u32,
    /// Levels of each block's cone required directly; more is closer to the true cone, and slower.
    pub levels: u32,
}

impl Geometry {
    pub fn block_count(&self) -> usize {
        self.counts.iter().map(|&c| c as usize).product()
    }

    /// The crate's precedence arcs in CSR form, and the counts the figures show.
    pub fn arcs(&self) -> Result<Arcs, String> {
        let mut sizes = [0_u32; 3];
        for (slot, &m) in sizes.iter_mut().zip(&self.sizes_m) {
            let mm = (m * 1000.0).round();
            if !(1.0..f64::from(u32::MAX)).contains(&mm) {
                return Err(format!("block size {m} m must be at least 1 mm and finite"));
            }
            *slot = mm as u32;
        }
        let grid = BlockGrid::new(self.counts, sizes).map_err(|r| r.to_string())?;
        let tangent = SlopeTangent::new(self.rise, self.run).map_err(|r| r.to_string())?;
        let table = SlopeTable::uniform(tangent);
        let n = grid.block_count();
        let arcs =
            slope_arcs(&grid, &table, &vec![0; n], self.levels).map_err(|r| r.to_string())?;
        let full_cone_arcs = arcs.full_cone_arcs();
        let boundary_blocks = arcs.boundary_blocks();
        let (offsets, targets) = arcs.into_csr();
        Ok(Arcs {
            offsets,
            targets,
            full_cone_arcs,
            boundary_blocks,
        })
    }
}

/// CSR arcs and their counts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Arcs {
    pub offsets: Vec<usize>,
    pub targets: Vec<u32>,
    pub full_cone_arcs: u64,
    pub boundary_blocks: usize,
}

/// The power-of-two scale with `max_abs · scale ≤ budget`, or `requested` when it is positive.
pub fn pick_scale(max_abs: f64, budget: f64, requested: f64) -> Result<f64, String> {
    if requested.is_finite() && requested > 0.0 {
        return Ok(requested);
    }
    if !(max_abs.is_finite() && budget.is_finite() && budget > 0.0) {
        return Err(format!("cannot scale values of size {max_abs}"));
    }
    if max_abs == 0.0 {
        return Ok(1.0);
    }
    let exp = (budget / max_abs).log2().floor().clamp(-1000.0, 1000.0) as i32;
    Ok(2.0_f64.powi(exp))
}

/// `round(v · scale)` for each value, refusing a non-finite one.
pub fn to_i64(values: &[f64], scale: f64, what: &str) -> Result<Vec<i64>, String> {
    values
        .iter()
        .enumerate()
        .map(|(b, &v)| {
            let s = (v * scale).round();
            if s.is_finite() && s.abs() < 9.0e18 {
                Ok(s as i64)
            } else {
                Err(format!("{what} {b} is {v}, not a finite number in range"))
            }
        })
        .collect()
}

fn max_abs(values: &[f64]) -> f64 {
    values.iter().fold(0.0_f64, |m, v| m.max(v.abs()))
}

/// Budget for the pit: `n · max|v| · scale ≤ 2⁶⁰`, at most 2⁴⁰ per block.
fn pit_budget(n: usize) -> f64 {
    (2.0_f64.powi(60) / n.max(1) as f64).min(2.0_f64.powi(40))
}

/// The certified ultimate pit of `values` (one per block, in dollars).
///
/// Output: `u8s` the pit flags; `f64s` = `[value $, verified, arcs, full-cone arcs, boundary blocks, blocks in the
/// pit, mergers, splits, relabels, max label, scale]`. `verified` is 1 when the crate's independent `verify`, run
/// again here on the returned pit and its pseudoflow certificate, gives the solver's value.
pub fn pit(geo: &Geometry, values: &[f64], scale: f64) -> Result<Output, String> {
    let n = geo.block_count();
    if values.len() != n {
        return Err(format!("{} values for {n} blocks", values.len()));
    }
    let scale = pick_scale(max_abs(values), pit_budget(n), scale)?;
    let weights = to_i64(values, scale, "block value")?;
    let arcs = geo.arcs()?;
    let arc_count = arcs.targets.len();
    let problem =
        ClosureProblem::new(weights, arcs.offsets, arcs.targets).map_err(|r| r.to_string())?;
    let sol = solve(&problem).map_err(|r| r.to_string())?;
    let verified = verify(&problem, sol.in_pit(), sol.arc_flow()) == Ok(sol.value());
    let stats = sol.stats();
    Ok(Output {
        f64s: vec![
            sol.value() as f64 / scale,
            f64::from(u8::from(verified)),
            arc_count as f64,
            arcs.full_cone_arcs as f64,
            arcs.boundary_blocks as f64,
            sol.pit_size() as f64,
            stats.mergers as f64,
            stats.splits as f64,
            stats.relabels as f64,
            f64::from(stats.max_label),
            scale,
        ],
        i32s: Vec::new(),
        u8s: sol.in_pit().iter().map(|&b| u8::from(b)).collect(),
    })
}

/// Nested shells. Input: `n` revenues, `n` costs (dollars), then `factor_count` strictly increasing revenue factors.
///
/// Output: `i32s` the first shell holding each block, −1 for none; `f64s` = `[scale, arcs]`, then per shell
/// `[factor, value $ at that factor, blocks, blocks in the pit solved there]`.
pub fn shells(
    geo: &Geometry,
    input: &[f64],
    factor_count: usize,
    scale: f64,
) -> Result<Output, String> {
    let n = geo.block_count();
    if input.len() != 2 * n + factor_count {
        return Err(format!(
            "{} inputs for {n} revenues, {n} costs and {factor_count} factors",
            input.len()
        ));
    }
    let (revenue, rest) = input.split_at(n);
    let (cost, factors) = rest.split_at(n);
    let fmax = factors.iter().fold(0.0_f64, |m, &f| m.max(f));
    if factor_count == 0 || !fmax.is_finite() {
        return Err("give at least one finite revenue factor".to_string());
    }
    // values_at(λ) = num·revenue − den·cost, with num ≤ fmax·den.
    let per_unit = (fmax + 1.0) * FACTOR_DEN as f64;
    let scale = pick_scale(
        max_abs(revenue).max(max_abs(cost)),
        pit_budget(n) / per_unit,
        scale,
    )?;
    let factor_list = factors
        .iter()
        .map(|&f| {
            let num = (f * FACTOR_DEN as f64).round();
            if !(1.0..9.0e18).contains(&num) {
                return Err(format!(
                    "revenue factor {f} must be at least 1/{FACTOR_DEN}"
                ));
            }
            RevenueFactor::new(num as i64, FACTOR_DEN).map_err(|r| r.to_string())
        })
        .collect::<Result<Vec<_>, _>>()?;
    let arcs = geo.arcs()?;
    let arc_count = arcs.targets.len();
    let model = ShellModel::new(
        to_i64(revenue, scale, "revenue")?,
        to_i64(cost, scale, "cost")?,
        arcs.offsets,
        arcs.targets,
    )
    .map_err(|r| r.to_string())?;
    let nested = nested_shells(&model, &factor_list).map_err(|r| r.to_string())?;
    let mut f64s = vec![scale, arc_count as f64];
    for s in nested.shells() {
        let den = s.factor.den() as f64;
        f64s.extend([
            s.factor.num() as f64 / den,
            s.value as f64 / den / scale,
            s.blocks as f64,
            s.solved_pit_blocks as f64,
        ]);
    }
    Ok(Output {
        f64s,
        i32s: nested
            .first_shell()
            .iter()
            .map(|&k| i32::try_from(k).unwrap_or(-1))
            .collect(),
        u8s: Vec::new(),
    })
}

/// TopoSort's greedy schedule with one resource (tonnes per period).
///
/// Input: `n` values ($), `n` tonnages, `periods` capacities (tonnes), `n` ranks (lower first; NaN leaves a block
/// unmined). When every rank is NaN the bridge ranks for the caller: blocks of the certified ultimate pit,
/// best value first, everything else unmined.
///
/// Output: `i32s` the period of each block, −1 for unmined; `f64s` = `[blocks mined, value $, scale, feasible]`,
/// then the undiscounted value mined in each period ($), then the tonnes mined in each period. `feasible` is the
/// crate's own `check` of the schedule, run again here.
pub fn schedule(
    geo: &Geometry,
    input: &[f64],
    periods: usize,
    scale: f64,
) -> Result<Output, String> {
    let n = geo.block_count();
    if input.len() != 3 * n + periods {
        return Err(format!(
            "{} inputs for {n} values, {n} tonnages, {periods} capacities and {n} ranks",
            input.len()
        ));
    }
    let (values, rest) = input.split_at(n);
    let (tonnes, rest) = rest.split_at(n);
    let (capacity, ranks) = rest.split_at(periods);
    let scale = pick_scale(max_abs(values), pit_budget(n), scale)?;
    let weights = to_i64(values, scale, "block value")?;
    let arcs = geo.arcs()?;
    let blocks = ClosureProblem::new(weights.clone(), arcs.offsets, arcs.targets)
        .map_err(|r| r.to_string())?;
    let rank: Vec<Option<i64>> = if ranks.iter().all(|r| r.is_nan()) {
        let sol = solve(&blocks).map_err(|r| r.to_string())?;
        sol.in_pit()
            .iter()
            .zip(&weights)
            .map(|(&inside, &w)| inside.then_some(-w))
            .collect()
    } else {
        ranks
            .iter()
            .map(|&r| (r.is_finite()).then(|| r.round() as i64))
            .collect()
    };
    let usage = vec![to_i64(tonnes, TONNE_SCALE, "tonnage")?];
    let limits = vec![to_i64(capacity, TONNE_SCALE, "capacity")?];
    let problem =
        ScheduleProblem::new(blocks, periods, usage, limits).map_err(|r| r.to_string())?;
    let plan = greedy(&problem, &rank).map_err(|r| r.to_string())?;
    let checked = check(&problem, &plan).map_err(|r| r.to_string())?;
    let mut f64s = vec![
        checked.mined as f64,
        checked.value() as f64 / scale,
        scale,
        f64::from(u8::from(checked.feasible())),
    ];
    f64s.extend(checked.period_values.iter().map(|&v| v as f64 / scale));
    if let Some(row) = checked.used.first() {
        f64s.extend(row.iter().map(|&u| u as f64 / TONNE_SCALE));
    }
    Ok(Output {
        f64s,
        i32s: plan
            .iter()
            .map(|t| t.and_then(|t| i32::try_from(t).ok()).unwrap_or(-1))
            .collect(),
        u8s: Vec::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every closed set of a tiny grid, by enumeration: the crate's pit must be worth the best.
    #[test]
    fn pit_matches_enumerated_closures() {
        let geo = Geometry {
            counts: [3, 1, 3],
            sizes_m: [10.0, 10.0, 10.0],
            rise: 1,
            run: 1,
            levels: 3,
        };
        let values = [-1.0, -2.0, -1.0, -1.0, 3.0, -4.0, 2.0, 6.0, -1.0];
        let out = pit(&geo, &values, 1.0).unwrap();
        let arcs = geo.arcs().unwrap();
        let n = values.len();
        let mut best = 0.0_f64;
        for mask in 0_u32..(1 << n) {
            let inside = |b: usize| mask >> b & 1 == 1;
            let closed = (0..n).all(|u| {
                !inside(u)
                    || arcs.targets[arcs.offsets[u]..arcs.offsets[u + 1]]
                        .iter()
                        .all(|&v| inside(v as usize))
            });
            if closed {
                let v: f64 = (0..n).filter(|&b| inside(b)).map(|b| values[b]).sum();
                best = best.max(v);
            }
        }
        assert_eq!(out.f64s[0], best);
        assert_eq!(out.f64s[1], 1.0);
    }
}
