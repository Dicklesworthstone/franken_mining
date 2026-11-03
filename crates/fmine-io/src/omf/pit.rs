//! An OMF block model as an ultimate-pit problem (bd-joiv, plan §15.4): a regular block model, block values from its
//! attributes and precedence from slope templates, solved and certified by `fmine_optimize::pitflow`.
//!
//! **Order.** OMF puts a block model's `Primitives` values on its parent blocks "increasing U first, then V, then W"
//! (omf-rust `block_model.rs`), and W grows upward from the frame's origin. `pitflow` numbers block `(i, j, k)` as
//! `(k·ny + j)·nx + i` with `k = 0` the top layer. So `i = u`, `j = v` and `k = nz − 1 − w`: the same blocks with the
//! layers in reverse, a reordering that is its own inverse.
//!
//! **Frame.** W must point straight up, `(0, 0, 1)`, and V must lie counterclockwise of U seen from above. The model
//! may be rotated about W; slope sectors are then measured from U toward V, not from east toward north.
//!
//! **Sizes.** `pitflow` takes block sizes as integers in one length unit. The cone test is homogeneous in the sizes,
//! so they are scaled by the smallest power of two, at most `2^MAX_SIZE_SCALE`, that makes all three integers below
//! 2^32. Scaling by a power of two is exact; sizes no such power makes integers, such as 0.1, are refused.
//!
//! **Values.** A block's weight is its value times `per_unit`, rounded to the nearest integer (halves away from zero).
//! Each block moves by at most half a unit, `1 / (2 · per_unit)`, so the pit is best for the rounded values and
//! within `blocks / per_unit` of best for the attribute's own.
//!
//! **Schedules** (bd-fr8x). [`PitModel::schedule`] builds `toposort`'s problem from the values, the precedence arcs
//! and capacities whose per-block uses are attributes, rounded up so that a schedule within the scaled limits is
//! within the true ones. It places blocks in the caller's rank order by TopoSort (`toposort::greedy`) and improves
//! that schedule by local search at the discount rate (`toposort::search::improve`). Both run `toposort::check` and
//! refuse a schedule that fails it. The schedule is feasible and locally improved, not proved best.
//!
//! **LP schedules** (bd-djb7). [`PitModel::lp_schedule`] composes the pipeline the MineLib harnesses run. PDLP
//! (`fmine_lp`) solves the schedule's LP relaxation (`toposort::lp::relaxation`, at the values' own `per_unit`).
//! TopoSort then builds six schedules from the LP's shares (`lp::starts`), and local search improves each, keeping
//! the best (`lp::incumbent`). PDLP's certified dual bound, negated, is an upper bound on the discounted value of
//! every schedule (with discount factors `(1 + rate)^−t` in `f64`), so the incumbent's gap to it bounds how far
//! from best the schedule can be.

use fmine_evidence::{Refusal, RefusalCode};
use fmine_lp::pdlp::{LpProblem, PdlpOptions, PdlpSolution, solve as solve_lp};
use fmine_optimize::pitflow::shells::{NestedShells, RevenueFactor, ShellModel, nested_shells};
use fmine_optimize::pitflow::slope::{BlockGrid, SlopeTable, slope_arcs};
use fmine_optimize::pitflow::{ClosureProblem, PitSolution, solve};
use fmine_optimize::toposort::lp::{Incumbent, Shares, incumbent, relaxation, starts};
use fmine_optimize::toposort::search::{Improvement, discount_factors, improve};
use fmine_optimize::toposort::{ScheduleProblem, greedy};

use super::proportions::{element, kind, missing};
use super::{
    Attribute, AttributeData, BlockModel, Element, Geometry, Grid3, Location, Numbers, Project,
};

/// The largest power of two, as an exponent, by which block sizes are scaled to make them integers.
pub const MAX_SIZE_SCALE: u32 = 32;

/// 2^63: weights must lie below it in magnitude.
const TWO_63: f64 = 9_223_372_036_854_775_808.0;

fn outside(detail: String) -> Refusal {
    Refusal::new(RefusalCode::OutsidePhysicalDomain109, detail)
}

/// `size` scaled by the smallest power of two, at most `2^MAX_SIZE_SCALE`, that makes all three integers from 1 to
/// `u32::MAX`, with that power's exponent.
fn integer_sizes(size: [f64; 3]) -> Option<([u32; 3], u32)> {
    let mut scaled = size;
    for m in 0..=MAX_SIZE_SCALE {
        if scaled
            .iter()
            .all(|s| s.fract() == 0.0 && (1.0..=f64::from(u32::MAX)).contains(s))
        {
            return Some((scaled.map(|s| s as u32), m));
        }
        scaled = scaled.map(|s| s * 2.0);
    }
    None
}

/// Per-block values from OMF's order to `pitflow`'s, or back: the layers in reverse.
fn reversed_layers<T: Clone>(per_block: &[T], count: [u32; 3]) -> Vec<T> {
    let layer = (count[0] as usize * count[1] as usize).max(1);
    per_block.chunks(layer).rev().flatten().cloned().collect()
}

/// The one attribute of `e` called `name`, on its parent blocks.
fn on_blocks<'a>(e: &'a Element, name: &str) -> Result<&'a Attribute, Refusal> {
    let found: Vec<&Attribute> = e.attributes.iter().filter(|a| a.name == name).collect();
    let [a] = found.as_slice() else {
        return Err(missing(format!(
            "OMF: block model \"{}\" has {} attributes called \"{name}\"; a pit needs exactly one",
            e.name,
            found.len()
        )));
    };
    if a.location != Location::Primitives {
        return Err(missing(format!(
            "OMF: attribute \"{name}\" of block model \"{}\" is on {:?}, not on the parent blocks (Primitives)",
            e.name, a.location
        )));
    }
    Ok(a)
}

/// A regular OMF block model with its precedence arcs, ready for ultimate pits and nested shells. Blocks are in
/// `pitflow`'s order; [`PitModel::omf_order`] puts per-block results back in OMF's.
#[derive(Debug, Clone)]
pub struct PitModel<'p> {
    element: &'p Element,
    count: [u32; 3],
    grid: BlockGrid,
    size_scale: u32,
    offsets: Vec<usize>,
    targets: Vec<u32>,
}

impl<'p> PitModel<'p> {
    /// The BlockModel element `block_model`, each block requiring its `levels`-layer cone under `slopes`. A block's
    /// slope domain is its category in the Category attribute `domain`, or domain 0 for every block when `domain`
    /// is `None`.
    ///
    /// # Errors
    /// `FMINE-REF-506` when no element, or more than one, is called `block_model`, or it is not a BlockModel; and
    /// when `domain` names no attribute, or more than one, or one that is not a Category on the parent blocks, or has
    /// a null. `FMINE-REF-109` for a tensor grid, a W axis that is not straight up, a mirrored frame, sizes no power
    /// of two up to `2^MAX_SIZE_SCALE` makes integers below 2^32, or a category above 65,535. Then the refusals of
    /// `pitflow::slope::BlockGrid::new` and `slope_arcs` (`FMINE-REF-207`), among them a domain the table lacks.
    pub fn new(
        project: &'p Project,
        block_model: &str,
        slopes: &SlopeTable,
        domain: Option<&str>,
        levels: u32,
    ) -> Result<Self, Refusal> {
        let e = element(project, block_model)?;
        let Geometry::BlockModel(BlockModel { orient, grid, .. }) = &e.geometry else {
            return Err(missing(format!(
                "OMF: element \"{block_model}\" is {}, not a BlockModel",
                kind(&e.geometry)
            )));
        };
        let Grid3::Regular { size, count } = grid else {
            return Err(outside(format!(
                "OMF: block model \"{block_model}\" has a tensor grid; a pit needs a regular one"
            )));
        };
        let (u, v) = (orient.u, orient.v);
        if orient.w != [0.0, 0.0, 1.0] || u[2] != 0.0 || v[2] != 0.0 {
            return Err(outside(format!(
                "OMF: block model \"{block_model}\" has W {:?}, U {u:?} and V {v:?}; a pit needs W straight up and U \
                 and V level",
                orient.w
            )));
        }
        if u[0] * v[1] - u[1] * v[0] <= 0.0 {
            return Err(outside(format!(
                "OMF: block model \"{block_model}\" is mirrored (U {u:?}, V {v:?}); a pit needs V counterclockwise \
                 of U seen from above"
            )));
        }
        let (sizes, size_scale) = integer_sizes(*size).ok_or_else(|| {
            outside(format!(
                "OMF: block model \"{block_model}\"'s block sizes {size:?} are not integers below 2^32 when scaled \
                 by any power of two up to 2^{MAX_SIZE_SCALE}"
            ))
        })?;
        let grid = BlockGrid::new(*count, sizes)?;
        let domain_of = match domain {
            None => vec![0; grid.block_count()],
            Some(name) => {
                let a = on_blocks(e, name)?;
                let AttributeData::Category { values, .. } = &a.data else {
                    return Err(missing(format!(
                        "OMF: attribute \"{name}\" of block model \"{block_model}\" is not a Category; slope domains \
                         are categories"
                    )));
                };
                let omf = values
                    .iter()
                    .enumerate()
                    .map(|(b, d)| {
                        let d = d.ok_or_else(|| {
                            missing(format!(
                                "OMF: attribute \"{name}\" of block model \"{block_model}\" has no category for \
                                 block {:?}",
                                uvw(b, *count)
                            ))
                        })?;
                        u16::try_from(d).map_err(|_| {
                            outside(format!(
                                "OMF: block {:?} of block model \"{block_model}\" is in category {d}; slope domains \
                                 stop at 65,535",
                                uvw(b, *count)
                            ))
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                reversed_layers(&omf, *count)
            }
        };
        let (offsets, targets) = slope_arcs(&grid, slopes, &domain_of, levels)?.into_csr();
        Ok(Self {
            element: e,
            count: *count,
            grid,
            size_scale,
            offsets,
            targets,
        })
    }

    /// The `pitflow` grid. Its block sizes are the OMF sizes times `2^size_scale()`.
    #[must_use]
    pub fn grid(&self) -> &BlockGrid {
        &self.grid
    }

    /// The exponent of the power of two the block sizes were scaled by.
    #[must_use]
    pub fn size_scale(&self) -> u32 {
        self.size_scale
    }

    /// Precedence arcs kept.
    #[must_use]
    pub fn arc_count(&self) -> usize {
        self.targets.len()
    }

    /// The Number attribute `attribute` on the parent blocks, times `per_unit` and rounded to the nearest integer, in
    /// `pitflow`'s order.
    ///
    /// # Errors
    /// `FMINE-REF-506` when no attribute, or more than one, is called `attribute`, or it is not a Number on the parent
    /// blocks, or holds dates, or has a null. `FMINE-REF-109` for `per_unit` 0, or a value that is not finite or whose
    /// weight is outside `i64`.
    pub fn weights(&self, attribute: &str, per_unit: u32) -> Result<Vec<i64>, Refusal> {
        self.scaled(attribute, per_unit, f64::round)
    }

    /// As [`PitModel::weights`], with floats rounded up rather than to the nearest integer: a block's use of a
    /// capacity, so that a schedule within the scaled limits is within the true ones.
    ///
    /// # Errors
    /// As [`PitModel::weights`].
    pub fn uses(&self, attribute: &str, per_unit: u32) -> Result<Vec<i64>, Refusal> {
        self.scaled(attribute, per_unit, f64::ceil)
    }

    fn scaled(
        &self,
        attribute: &str,
        per_unit: u32,
        round: fn(f64) -> f64,
    ) -> Result<Vec<i64>, Refusal> {
        let name = &self.element.name;
        if per_unit == 0 {
            return Err(outside(format!(
                "OMF: weights for block model \"{name}\" need a per_unit of at least 1"
            )));
        }
        let a = on_blocks(self.element, attribute)?;
        let null = |b: usize| {
            missing(format!(
                "OMF: attribute \"{attribute}\" of block model \"{name}\" has no value for block {:?}",
                uvw(b, self.count)
            ))
        };
        let range = |b: usize, x: String| {
            outside(format!(
                "OMF: block {:?} of block model \"{name}\" has {attribute} {x}, whose weight at {per_unit} per unit \
                 is not a finite i64",
                uvw(b, self.count)
            ))
        };
        let omf = match &a.data {
            AttributeData::Number {
                values: Numbers::Integer(values),
                ..
            } => values
                .iter()
                .enumerate()
                .map(|(b, x)| {
                    let x = x.ok_or_else(|| null(b))?;
                    x.checked_mul(i64::from(per_unit))
                        .ok_or_else(|| range(b, x.to_string()))
                })
                .collect::<Result<Vec<_>, _>>()?,
            AttributeData::Number {
                values: Numbers::Float(values),
                ..
            } => values
                .iter()
                .enumerate()
                .map(|(b, x)| {
                    let x = x.ok_or_else(|| null(b))?;
                    let w = round(x * f64::from(per_unit));
                    if w.is_finite() && w.abs() < TWO_63 {
                        Ok(w as i64)
                    } else {
                        Err(range(b, x.to_string()))
                    }
                })
                .collect::<Result<Vec<_>, _>>()?,
            _ => {
                return Err(missing(format!(
                    "OMF: attribute \"{attribute}\" of block model \"{name}\" is not a Number of integers or floats"
                )));
            }
        };
        Ok(reversed_layers(&omf, self.count))
    }

    /// The certified ultimate pit for the values in `attribute`, at `per_unit` ([`PitModel::weights`]). Its value is
    /// in units of `1 / per_unit`, and its blocks are in `pitflow`'s order.
    ///
    /// # Errors
    /// Those of [`PitModel::weights`], `ClosureProblem::new` and `pitflow::solve`.
    pub fn ultimate_pit(&self, attribute: &str, per_unit: u32) -> Result<PitSolution, Refusal> {
        let problem = ClosureProblem::new(
            self.weights(attribute, per_unit)?,
            self.offsets.clone(),
            self.targets.clone(),
        )?;
        solve(&problem)
    }

    /// Certified nested shells, one per revenue factor, for the revenue and cost in the attributes `revenue` and
    /// `cost`, each at `per_unit` ([`PitModel::weights`]); see `pitflow::shells`.
    ///
    /// # Errors
    /// Those of [`PitModel::weights`], `ShellModel::new` (among them a negative revenue) and `nested_shells`.
    pub fn shells(
        &self,
        revenue: &str,
        cost: &str,
        per_unit: u32,
        factors: &[RevenueFactor],
    ) -> Result<NestedShells, Refusal> {
        let model = ShellModel::new(
            self.weights(revenue, per_unit)?,
            self.weights(cost, per_unit)?,
            self.offsets.clone(),
            self.targets.clone(),
        )?;
        nested_shells(&model, factors)
    }

    /// A feasible, locally improved schedule (see the module notes). `rank` holds one entry per parent block in OMF's
    /// order: lower ranks are taken first, and `None` leaves a block out.
    ///
    /// # Errors
    /// Those of [`PitModel::weights`] and [`PitModel::uses`] for the value and each capacity, of
    /// `ClosureProblem::new` and `ScheduleProblem::new` (among them limits that are not one per period, or negative),
    /// of `discount_factors`, and of `greedy` (among them a rank that is not one per block) and `improve`.
    pub fn schedule(
        &self,
        spec: &ScheduleSpec<'_>,
        rank: &[Option<i64>],
    ) -> Result<OmfSchedule, Refusal> {
        let (problem, factors) = self.schedule_problem(spec)?;
        let start = greedy(&problem, &reversed_layers(rank, self.count))?;
        let improvement = improve(&problem, &start, &factors, spec.max_rounds)?;
        Ok(OmfSchedule {
            problem,
            factors,
            improvement,
        })
    }

    /// A schedule guided by the LP relaxation, as the MineLib harnesses build one (see the module notes): PDLP solves
    /// `toposort::lp::relaxation` under `pdlp`; `lp::starts` turns its solution into TopoSort schedules; and
    /// `lp::incumbent` improves each by local search and keeps the best.
    ///
    /// # Errors
    /// Those of [`PitModel::schedule`] but the rank's, and of `relaxation`, `LpProblem::new`, PDLP's `solve` (among
    /// them options out of range), `Shares::new`, `starts` and `incumbent`.
    pub fn lp_schedule(
        &self,
        spec: &ScheduleSpec<'_>,
        pdlp: &PdlpOptions,
    ) -> Result<LpSchedule, Refusal> {
        let (problem, factors) = self.schedule_problem(spec)?;
        let r = relaxation(&problem, spec.discount[0], spec.discount[1], 0)?;
        let lp = solve_lp(
            &LpProblem::new(r.cost, &r.matrix, r.row_lo, r.row_hi, r.col_lo, r.col_hi)?,
            pdlp,
        )?;
        let shares = Shares::new(problem.blocks().node_count(), spec.periods, lp.x().to_vec())?;
        let starts = starts(&problem, &shares)?;
        let incumbent = incumbent(&problem, &starts, &factors, spec.max_rounds)?;
        Ok(LpSchedule {
            problem,
            factors,
            lp,
            incumbent,
        })
    }

    fn schedule_problem(
        &self,
        spec: &ScheduleSpec<'_>,
    ) -> Result<(ScheduleProblem, Vec<i64>), Refusal> {
        let blocks = ClosureProblem::new(
            self.weights(spec.value, spec.per_unit)?,
            self.offsets.clone(),
            self.targets.clone(),
        )?;
        let usage = spec
            .capacities
            .iter()
            .map(|c| self.uses(c.attribute, c.per_unit))
            .collect::<Result<Vec<_>, _>>()?;
        let limits = spec.capacities.iter().map(|c| c.limits.to_vec()).collect();
        let problem = ScheduleProblem::new(blocks, spec.periods, usage, limits)?;
        let factors = discount_factors(spec.discount[0], spec.discount[1], spec.periods)?;
        Ok((problem, factors))
    }

    /// Per-block results in `pitflow`'s order, such as `PitSolution::in_pit` or `NestedShells::first_shell`, in OMF's
    /// order: one per parent block, U fastest, then V, then W from the bottom.
    #[must_use]
    pub fn omf_order<T: Clone>(&self, per_block: &[T]) -> Vec<T> {
        reversed_layers(per_block, self.count)
    }
}

/// A per-period limit on a quantity each block uses, such as tonnes mined or milled: the Number attribute
/// `attribute` at `per_unit`, rounded up ([`PitModel::uses`]), and one limit per period in the same units.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Capacity<'a> {
    pub attribute: &'a str,
    pub per_unit: u32,
    pub limits: &'a [i64],
}

/// What [`PitModel::schedule`] schedules: the values in the attribute `value` at `per_unit`, over `periods` periods
/// under `capacities`, discounted at `discount[0] / discount[1]` a period, with at most `max_rounds` rounds of local
/// search.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScheduleSpec<'a> {
    pub value: &'a str,
    pub per_unit: u32,
    pub capacities: &'a [Capacity<'a>],
    pub periods: usize,
    pub discount: [i64; 2],
    pub max_rounds: usize,
}

/// A schedule from [`PitModel::schedule`], in `pitflow`'s block order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OmfSchedule {
    problem: ScheduleProblem,
    factors: Vec<i64>,
    improvement: Improvement,
}

impl OmfSchedule {
    /// The problem scheduled: the scaled values, precedence, uses and limits.
    #[must_use]
    pub fn problem(&self) -> &ScheduleProblem {
        &self.problem
    }

    /// The discount factors, `DISCOUNT_SCALE` in period 0 (`toposort::search::discount_factors`).
    #[must_use]
    pub fn factors(&self) -> &[i64] {
        &self.factors
    }

    /// The search's result: the schedule, one period or `None` per block, and its discounted value before and after,
    /// in units of `1 / (per_unit · DISCOUNT_SCALE)`.
    #[must_use]
    pub fn improvement(&self) -> &Improvement {
        &self.improvement
    }
}

/// A schedule from [`PitModel::lp_schedule`], in `pitflow`'s block order.
#[derive(Debug, Clone, PartialEq)]
pub struct LpSchedule {
    problem: ScheduleProblem,
    factors: Vec<i64>,
    lp: PdlpSolution,
    incumbent: Incumbent,
}

impl LpSchedule {
    /// The problem scheduled.
    #[must_use]
    pub fn problem(&self) -> &ScheduleProblem {
        &self.problem
    }

    /// The integer discount factors the search used (`toposort::search::discount_factors`).
    #[must_use]
    pub fn factors(&self) -> &[i64] {
        &self.factors
    }

    /// PDLP's solution of the relaxation, a minimisation of minus the discounted value in units of `1 / per_unit`.
    #[must_use]
    pub fn lp(&self) -> &PdlpSolution {
        &self.lp
    }

    /// Minus PDLP's certified bound: no schedule's discounted value, in units of `1 / per_unit` with factors
    /// `(1 + rate)^−t`, exceeds it. `None` when PDLP certified no bound.
    #[must_use]
    pub fn upper_bound(&self) -> Option<f64> {
        self.lp.bound().map(|b| -b.value())
    }

    /// The searched starts and the best of them.
    #[must_use]
    pub fn incumbent(&self) -> &Incumbent {
        &self.incumbent
    }
}

/// OMF block `b`'s `(u, v, w)` indices.
fn uvw(b: usize, [nu, nv, _]: [u32; 3]) -> [usize; 3] {
    let (nu, nv) = (nu as usize, nv as usize);
    [
        b % nu.max(1),
        (b / nu.max(1)) % nv.max(1),
        b / (nu * nv).max(1),
    ]
}
