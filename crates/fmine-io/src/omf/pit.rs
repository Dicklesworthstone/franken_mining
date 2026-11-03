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

use fmine_evidence::{Refusal, RefusalCode};
use fmine_optimize::pitflow::shells::{NestedShells, RevenueFactor, ShellModel, nested_shells};
use fmine_optimize::pitflow::slope::{BlockGrid, SlopeTable, slope_arcs};
use fmine_optimize::pitflow::{ClosureProblem, PitSolution, solve};

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
                    let w = (x * f64::from(per_unit)).round();
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

    /// Per-block results in `pitflow`'s order, such as `PitSolution::in_pit` or `NestedShells::first_shell`, in OMF's
    /// order: one per parent block, U fastest, then V, then W from the bottom.
    #[must_use]
    pub fn omf_order<T: Clone>(&self, per_block: &[T]) -> Vec<T> {
        reversed_layers(per_block, self.count)
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
