//! Certified volumes and block proportions for closed triangle meshes (plan §13.9 `fmine-geom::volcert`, §13.11
//! `VolumeEnclosure`, PO-STRATA-8; bd-wq34).
//! Determinism: BitwisePortable (exact expansion arithmetic, exact `orient2d` and `orient3d` signs, IEEE comparisons
//! and `next_up`/`next_down`; the inside/outside label is `winding_exact`, through `det::atan2`, rounded to an
//! integer. The outputs are integers and outward-rounded `f64`.)
//!
//! - [`ClosedMesh::certify`] welds vertices whose coordinates have identical bits, then refuses a mesh that is not
//!   closed and consistently oriented: every directed edge needs exactly one reverse twin, and no triangle may
//!   repeat a vertex.
//! - [`ClosedMesh::volume`] is the divergence theorem, the sum over triangles of `a·(b×c)/6`, computed exactly in
//!   expansion arithmetic and rounded outward once. The mesh's volume (the integral of its winding number) lies in
//!   `[lo, hi]`, a few units in the last place wide (PO-STRATA-8). The plan says interval arithmetic; exact
//!   arithmetic is tighter and needs no local origin, so far-off coordinates (UTM northings) lose nothing.
//! - [`block_proportions`] splits each block of a [`BlockGrid`] into `8^depth` equal sub-cells. A sub-cell joins the
//!   band when an exact separating-axis test cannot separate its closed box from some triangle. Every other
//!   sub-cell lies wholly inside or wholly outside: it takes the winding number of its face-connected component of
//!   surface-free sub-cells and blocks, evaluated once per component, or 0 when the component reaches past the
//!   mesh's bounding box. A block's proportion inside the mesh then lies in `[inside, inside + band] / 8^depth`.
//!
//! What the enclosure rests on. The band test, the cell boundaries and the volume are exact. The inside/outside
//! label is `winding_exact` rounded to an integer, accepted only within 1e-6 of 0 or 1; that check catches a broken
//! evaluation, it does not prove the rounding right. A proportion counts nominal sub-cells: the `f64` boundaries
//! differ from the nominal ones by about one unit in the last place of the coordinate.

use std::collections::BTreeMap;

use fmine_evidence::{Refusal, RefusalCode};

use crate::exact::expansion::{fast_expansion_sum_zeroelim, prod_diff, scale_expansion_zeroelim};
use crate::exact::{Sign, orient2d, orient3d};
use crate::vdb::VdbGrid;
use crate::winding::{Soup, winding_exact};

/// The deepest refinement: `8^6` = 262,144 sub-cells per block.
pub const MAX_DEPTH: u8 = 6;
/// Blocks near the mesh plus the sub-cells of crossed blocks; a request needing more is refused.
const MAX_NODES: u64 = 1 << 25;
/// How far a winding number may sit from an integer before its label is refused.
const INTEGRAL_TOLERANCE: f64 = 1e-6;
/// A volume sum with more expansion components than this is refused.
const MAX_COMPONENTS: usize = 4096;
/// The smallest nonzero coordinate magnitude certified, 2^-200: products of three coordinates and their error
/// terms stay clear of the subnormal range, so every error-free transformation is exact.
const MIN_MAGNITUDE: f64 = f64::from_bits((1023 - 200) << 52);
/// The largest coordinate magnitude certified, 2^200: products of three stay finite.
const MAX_MAGNITUDE: f64 = f64::from_bits((1023 + 200) << 52);
/// The union-find node standing for everything beyond the mesh's bounding box, where the winding number is 0.
const OUT: u32 = 0;

fn outside_domain(detail: String) -> Refusal {
    Refusal::new(RefusalCode::OutsidePhysicalDomain109, detail)
}

fn uncertified(detail: String) -> Refusal {
    Refusal::new(RefusalCode::NumericalCertificateFailed207, detail)
}

/// An enclosure `[lo, hi]` of a closed mesh's volume (plan §13.11 `VolumeEnclosure`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VolumeEnclosure {
    lo: f64,
    hi: f64,
}

impl VolumeEnclosure {
    /// The lower end.
    #[must_use]
    pub fn lo(&self) -> f64 {
        self.lo
    }

    /// The upper end.
    #[must_use]
    pub fn hi(&self) -> f64 {
        self.hi
    }
}

/// A triangle mesh certified closed and consistently oriented, with its volume enclosure.
#[derive(Debug, Clone, PartialEq)]
pub struct ClosedMesh {
    soup: Soup,
    corners: Vec<[[f64; 3]; 3]>,
    boxes: Vec<[[f64; 3]; 2]>,
    bounds: [[f64; 3]; 2],
    volume: VolumeEnclosure,
}

impl ClosedMesh {
    /// Weld vertices whose coordinates have identical bits (`-0.0` counting as `0.0`), then certify the mesh.
    ///
    /// # Errors
    /// `FMINE-REF-109` for a corner index past the positions, a non-finite coordinate, or a nonzero coordinate whose
    /// magnitude is outside `[2^-200, 2^200]`. `FMINE-REF-207` for no triangles, a triangle repeating a vertex, a
    /// directed edge run by two triangles (inconsistent orientation, or more than two triangles on one edge), an
    /// edge with no reverse twin (an open mesh), or a volume that is not positive (inward-facing or flat).
    pub fn certify(positions: Vec<[f64; 3]>, triangles: Vec<[u32; 3]>) -> Result<Self, Refusal> {
        if triangles.is_empty() {
            return Err(uncertified(
                "closed mesh: there are no triangles".to_string(),
            ));
        }
        let (welded, remap) = weld(&positions)?;
        let mut tris = Vec::with_capacity(triangles.len());
        for (t, tri) in triangles.iter().enumerate() {
            let mut out = [0u32; 3];
            for (slot, &v) in out.iter_mut().zip(tri) {
                *slot = *usize::try_from(v)
                    .ok()
                    .and_then(|i| remap.get(i))
                    .ok_or_else(|| {
                        outside_domain(format!(
                            "closed mesh: triangle {t} names vertex {v}, past the {} positions",
                            positions.len()
                        ))
                    })?;
            }
            if out[0] == out[1] || out[1] == out[2] || out[2] == out[0] {
                return Err(uncertified(format!(
                    "closed mesh: triangle {t} repeats a vertex once identical positions are welded"
                )));
            }
            tris.push(out);
        }
        check_closed(&tris)?;
        let soup = Soup::new(welded, tris)?;
        for i in 0..soup.position_count() {
            for x in soup.position(i).unwrap_or_default() {
                if x != 0.0 && !(MIN_MAGNITUDE..=MAX_MAGNITUDE).contains(&x.abs()) {
                    return Err(outside_domain(format!(
                        "closed mesh: coordinate {x:e} of vertex {i} is outside the magnitudes 2^-200 to 2^200 \
                         where the volume is exact"
                    )));
                }
            }
        }
        let mut corners = Vec::with_capacity(soup.triangles().len());
        for t in 0..soup.triangles().len() {
            corners.push(
                soup.corners(t).ok_or_else(|| {
                    uncertified(format!("closed mesh: triangle {t} has no corners"))
                })?,
            );
        }
        let boxes: Vec<[[f64; 3]; 2]> = corners.iter().map(triangle_box).collect();
        let first = *boxes
            .first()
            .ok_or_else(|| uncertified("closed mesh: there are no triangles".to_string()))?;
        let bounds = boxes.iter().fold(first, |acc, b| {
            [
                std::array::from_fn(|a| acc[0][a].min(b[0][a])),
                std::array::from_fn(|a| acc[1][a].max(b[1][a])),
            ]
        });
        let volume = exact_volume(&corners)?;
        if volume.hi <= 0.0 {
            return Err(uncertified(format!(
                "closed mesh: the volume lies in [{:e}, {:e}], not above zero: the triangles face inward (reverse \
                 each one's vertex order) or enclose nothing",
                volume.lo, volume.hi
            )));
        }
        Ok(Self {
            soup,
            corners,
            boxes,
            bounds,
            volume,
        })
    }

    /// The welded soup (for [`crate::winding::WindingOctree`] or other winding queries).
    #[must_use]
    pub fn soup(&self) -> &Soup {
        &self.soup
    }

    /// The axis-aligned bounding box `[min, max]` of the triangles.
    #[must_use]
    pub fn bounds(&self) -> [[f64; 3]; 2] {
        self.bounds
    }

    /// The enclosure of the volume, the integral of the winding number over space (PO-STRATA-8).
    #[must_use]
    pub fn volume(&self) -> VolumeEnclosure {
        self.volume
    }
}

/// Weld positions with identical bits; `remap[i]` is position `i`'s welded index.
fn weld(positions: &[[f64; 3]]) -> Result<(Vec<[f64; 3]>, Vec<u32>), Refusal> {
    let mut seen: BTreeMap<[u64; 3], u32> = BTreeMap::new();
    let mut welded = Vec::new();
    let mut remap = Vec::with_capacity(positions.len());
    for p in positions {
        // Adding +0.0 turns -0.0 into +0.0 and leaves every other value alone.
        let q = p.map(|x| x + 0.0);
        let key = q.map(f64::to_bits);
        let id = if let Some(&id) = seen.get(&key) {
            id
        } else {
            let id = u32::try_from(welded.len()).map_err(|_| {
                outside_domain("closed mesh: more than u32::MAX distinct positions".to_string())
            })?;
            seen.insert(key, id);
            welded.push(q);
            id
        };
        remap.push(id);
    }
    Ok((welded, remap))
}

/// Every directed edge appears once, and its reverse once.
fn check_closed(tris: &[[u32; 3]]) -> Result<(), Refusal> {
    let mut edges: BTreeMap<(u32, u32), usize> = BTreeMap::new();
    for (t, &[a, b, c]) in tris.iter().enumerate() {
        for e in [(a, b), (b, c), (c, a)] {
            if let Some(first) = edges.insert(e, t) {
                return Err(uncertified(format!(
                    "closed mesh: triangles {first} and {t} both run edge {} -> {}: the mesh is not consistently \
                     oriented, or more than two triangles share the edge",
                    e.0, e.1
                )));
            }
        }
    }
    if let Some((&(a, b), &t)) = edges.iter().find(|(e, _)| !edges.contains_key(&(e.1, e.0))) {
        return Err(uncertified(format!(
            "closed mesh: edge {a} -> {b} of triangle {t} has no triangle running {b} -> {a}: the mesh is not closed"
        )));
    }
    Ok(())
}

fn triangle_box(c: &[[f64; 3]; 3]) -> [[f64; 3]; 2] {
    [
        std::array::from_fn(|a| c[0][a].min(c[1][a]).min(c[2][a])),
        std::array::from_fn(|a| c[0][a].max(c[1][a]).max(c[2][a])),
    ]
}

/// `a·(b×c)` as an exact expansion.
fn triple_product(a: [f64; 3], b: [f64; 3], c: [f64; 3]) -> Result<Vec<f64>, Refusal> {
    let x = scale_expansion_zeroelim(&prod_diff(b[1], c[2], b[2], c[1])?, a[0])?;
    let y = scale_expansion_zeroelim(&prod_diff(b[2], c[0], b[0], c[2])?, a[1])?;
    let z = scale_expansion_zeroelim(&prod_diff(b[0], c[1], b[1], c[0])?, a[2])?;
    fast_expansion_sum_zeroelim(&fast_expansion_sum_zeroelim(&x, &y)?, &z)
}

/// The divergence theorem in exact arithmetic, rounded outward once.
fn exact_volume(corners: &[[[f64; 3]; 3]]) -> Result<VolumeEnclosure, Refusal> {
    let mut total: Vec<f64> = Vec::new();
    for &[a, b, c] in corners {
        total = fast_expansion_sum_zeroelim(&total, &triple_product(a, b, c)?)?;
        if total.len() > MAX_COMPONENTS {
            return Err(uncertified(format!(
                "closed mesh volume: the exact sum grew past {MAX_COMPONENTS} components"
            )));
        }
    }
    // Each rounded step moves by at most half a unit in the last place, so stepping one unit outward after each
    // keeps the exact value inside; the components are summed smallest first.
    let (mut lo, mut hi) = (0.0_f64, 0.0_f64);
    for &x in &total {
        lo = (lo + x).next_down();
        hi = (hi + x).next_up();
    }
    let (lo, hi) = ((lo / 6.0).next_down(), (hi / 6.0).next_up());
    if !(lo.is_finite() && hi.is_finite()) {
        return Err(uncertified(
            "closed mesh volume: the enclosure overflowed".to_string(),
        ));
    }
    Ok(VolumeEnclosure { lo, hi })
}

/// A regular grid of axis-aligned blocks: block `(i, j, k)` spans `origin + size·(i, j, k)` to
/// `origin + size·(i + 1, j + 1, k + 1)`, in the mesh's frame and unit.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BlockGrid {
    origin: [f64; 3],
    size: [f64; 3],
    counts: [u32; 3],
}

impl BlockGrid {
    /// # Errors
    /// `FMINE-REF-109` for a non-finite origin, a block size that is not finite and positive, a block count of zero
    /// or above `i32::MAX`, or a far corner that overflows.
    pub fn new(origin: [f64; 3], size: [f64; 3], counts: [u32; 3]) -> Result<Self, Refusal> {
        for (axis, ((&o, &s), &n)) in origin.iter().zip(&size).zip(&counts).enumerate() {
            if !o.is_finite() || !s.is_finite() || s <= 0.0 {
                return Err(outside_domain(format!(
                    "block grid: axis {axis} has origin {o} and block size {s}; both must be finite and the size \
                     positive"
                )));
            }
            if n == 0 || i32::try_from(n).is_err() {
                return Err(outside_domain(format!(
                    "block grid: axis {axis} has {n} blocks; it needs 1 to {}",
                    i32::MAX
                )));
            }
            if !(o + s * f64::from(n)).is_finite() {
                return Err(outside_domain(format!(
                    "block grid: the far corner of axis {axis} overflows"
                )));
            }
        }
        Ok(Self {
            origin,
            size,
            counts,
        })
    }

    /// The origin, the lowest corner of block `(0, 0, 0)`.
    #[must_use]
    pub fn origin(&self) -> [f64; 3] {
        self.origin
    }

    /// The block size along each axis.
    #[must_use]
    pub fn size(&self) -> [f64; 3] {
        self.size
    }

    /// The number of blocks along each axis.
    #[must_use]
    pub fn counts(&self) -> [u32; 3] {
        self.counts
    }
}

/// The sub-cell lattice of a grid at one depth. Every boundary is computed by [`Lattice::at`] from its integer
/// index, so neighbouring cells, and a cell and its children, share boundaries bit for bit.
#[derive(Debug, Clone, Copy)]
struct Lattice {
    origin: [f64; 3],
    size: [f64; 3],
    counts: [u32; 3],
    /// `2^-depth`.
    scale: f64,
    /// `2^depth`, sub-cells along a block's edge.
    side: u64,
}

impl Lattice {
    fn new(grid: &BlockGrid, depth: u8) -> Result<Self, Refusal> {
        let side = 1u64 << depth;
        let lattice = Self {
            origin: grid.origin,
            size: grid.size,
            counts: grid.counts,
            scale: 1.0 / f64::from(1u32 << depth),
            side,
        };
        for (axis, (&o, &s)) in grid.origin.iter().zip(&grid.size).enumerate() {
            let n = grid.counts.get(axis).copied().unwrap_or_default();
            let far = lattice.at(axis, u64::from(n) * side);
            let magnitude = o.abs().max(far.abs()).max(s * f64::from(n));
            let ulp = magnitude.next_up() - magnitude;
            let step = s * lattice.scale;
            if step <= 4.0 * ulp {
                return Err(outside_domain(format!(
                    "block grid: sub-cells {step:e} wide on axis {axis} at depth {depth} are within four units in \
                     the last place of coordinates near {magnitude:e}; use a shallower depth or a local origin"
                )));
            }
        }
        Ok(lattice)
    }

    /// Boundary `j` of `axis`, counted in sub-cells from the origin.
    fn at(&self, axis: usize, j: u64) -> f64 {
        let (o, s) = (
            self.origin.get(axis).copied().unwrap_or_default(),
            self.size.get(axis).copied().unwrap_or_default(),
        );
        o + s * (j as f64 * self.scale)
    }

    /// The closed box of the cell whose lowest sub-cell is `base` and whose edge is `span` sub-cells.
    fn cell(&self, base: [u64; 3], span: u64) -> [[f64; 3]; 2] {
        [
            std::array::from_fn(|a| self.at(a, base[a])),
            std::array::from_fn(|a| self.at(a, base[a] + span)),
        ]
    }

    /// The closed box of block `ijk`.
    fn block(&self, ijk: [u64; 3]) -> [[f64; 3]; 2] {
        self.cell(ijk.map(|i| i * self.side), self.side)
    }
}

/// The blocks whose closed boxes meet the mesh's bounding box; every other block lies outside the mesh.
#[derive(Debug, Clone, Copy)]
struct SubGrid {
    /// The lowest block index on each axis.
    lo: [u64; 3],
    /// Blocks along each axis.
    dims: [u64; 3],
    /// Per axis, `[low, high]`: whether the space just past the sub-grid's face is beyond the bounding box, so a
    /// surface-free cell on that face winds 0.
    beyond_out: [[bool; 2]; 3],
}

impl SubGrid {
    fn new(lattice: &Lattice, bounds: &[[f64; 3]; 2]) -> Option<Self> {
        let mut lo = [0u64; 3];
        let mut dims = [0u64; 3];
        let mut beyond_out = [[false; 2]; 3];
        for axis in 0..3 {
            let n = lattice.counts.get(axis).copied().unwrap_or_default();
            let edge = |i: u32| lattice.at(axis, u64::from(i) * lattice.side);
            let (min, max) = (bounds[0][axis], bounds[1][axis]);
            let first = partition(n, |i| edge(i + 1) < min);
            let end = partition(n, |i| edge(i) <= max);
            if first >= end {
                return None;
            }
            lo[axis] = u64::from(first);
            dims[axis] = u64::from(end - first);
            beyond_out[axis] = [edge(first) <= min, edge(end) >= max];
        }
        Some(Self {
            lo,
            dims,
            beyond_out,
        })
    }

    fn count(&self) -> u64 {
        self.dims[0] * self.dims[1] * self.dims[2]
    }

    /// Sub-grid coordinates of linear index `b` (x fastest).
    fn coords(&self, b: u64) -> [u64; 3] {
        [
            b % self.dims[0],
            (b / self.dims[0]) % self.dims[1],
            b / (self.dims[0] * self.dims[1]),
        ]
    }

    fn stride(&self) -> [u64; 3] {
        [1, self.dims[0], self.dims[0] * self.dims[1]]
    }
}

/// The number of leading indices in `0..n` for which `pred` holds, `pred` being true then false.
fn partition(n: u32, pred: impl Fn(u32) -> bool) -> u32 {
    let (mut lo, mut hi) = (0u32, n);
    while lo < hi {
        let mid = lo + (hi - lo) / 2;
        if pred(mid) {
            lo = mid + 1;
        } else {
            hi = mid;
        }
    }
    lo
}

/// The eight corners of a box.
fn box_corners(b: &[[f64; 3]; 2]) -> [[f64; 3]; 8] {
    std::array::from_fn(|i| [b[i & 1][0], b[(i >> 1) & 1][1], b[(i >> 2) & 1][2]])
}

/// Whether the closed triangle meets the closed box: the 13-axis separating-axis test in exact predicates, so
/// `false` proves them disjoint. Box normals compare coordinates; the triangle normal is `orient3d` of the eight
/// corners; each edge-by-axis direction is `orient2d` in the plane across that axis.
fn meets(tri: &[[f64; 3]; 3], tbox: &[[f64; 3]; 2], cell: &[[f64; 3]; 2]) -> Result<bool, Refusal> {
    if (0..3).any(|a| tbox[1][a] < cell[0][a] || tbox[0][a] > cell[1][a]) {
        return Ok(false);
    }
    let mut side = Sign::Zero;
    let mut separated = true;
    for corner in box_corners(cell) {
        let s = orient3d(tri[0], tri[1], tri[2], corner)?;
        if s == Sign::Zero || (side != Sign::Zero && s != side) {
            separated = false;
            break;
        }
        side = s;
    }
    if separated {
        return Ok(false);
    }
    for axis in 0..3 {
        let (u, v) = ((axis + 1) % 3, (axis + 2) % 3);
        let flat = |p: [f64; 3]| [p[u], p[v]];
        let rect = [
            [cell[0][u], cell[0][v]],
            [cell[1][u], cell[0][v]],
            [cell[0][u], cell[1][v]],
            [cell[1][u], cell[1][v]],
        ];
        for e in 0..3 {
            let (p, q, r) = (flat(tri[e]), flat(tri[(e + 1) % 3]), flat(tri[(e + 2) % 3]));
            // The line through edge p-q separates when the whole rectangle lies strictly on one side and the third
            // corner does not lie on that side.
            let first = orient2d(p, q, rect[0])?;
            if first == Sign::Zero || first == orient2d(p, q, r)? {
                continue;
            }
            let mut all = true;
            for &c in &rect[1..] {
                if orient2d(p, q, c)? != first {
                    all = false;
                    break;
                }
            }
            if all {
                return Ok(false);
            }
        }
    }
    Ok(true)
}

/// Marks the sub-cells of one block that the surface meets.
struct Marker<'a> {
    mesh: &'a ClosedMesh,
    lattice: &'a Lattice,
    block_base: [u64; 3],
    crossed: Vec<bool>,
    any: bool,
}

impl Marker<'_> {
    fn mark(&mut self, base: [u64; 3], span: u64, candidates: &[u32]) -> Result<(), Refusal> {
        let cell = self.lattice.cell(base, span);
        let mut keep = Vec::new();
        for &t in candidates {
            let i = usize::try_from(t).unwrap_or(usize::MAX);
            let (Some(corners), Some(tbox)) = (self.mesh.corners.get(i), self.mesh.boxes.get(i))
            else {
                return Err(uncertified(format!(
                    "block proportions: triangle {t} is not in the mesh"
                )));
            };
            if meets(corners, tbox, &cell)? {
                keep.push(t);
            }
        }
        if keep.is_empty() {
            return Ok(());
        }
        if span == 1 {
            let side = self.lattice.side;
            let l: [u64; 3] = std::array::from_fn(|a| base[a] - self.block_base[a]);
            let index = usize::try_from(l[0] + side * (l[1] + side * l[2])).unwrap_or(usize::MAX);
            let slot = self.crossed.get_mut(index).ok_or_else(|| {
                uncertified(format!(
                    "block proportions: sub-cell {l:?} is outside its block"
                ))
            })?;
            *slot = true;
            self.any = true;
            return Ok(());
        }
        let half = span / 2;
        for octant in 0..8u64 {
            let child = [
                base[0] + (octant & 1) * half,
                base[1] + ((octant >> 1) & 1) * half,
                base[2] + (octant >> 2) * half,
            ];
            self.mark(child, half, &keep)?;
        }
        Ok(())
    }
}

/// Union-find over node indices; the smaller root wins, so a component's root is its smallest node and [`OUT`]
/// roots its own component.
struct Dsu(Vec<u32>);

impl Dsu {
    fn find(&mut self, mut x: u32) -> u32 {
        loop {
            let p = self.0[x as usize];
            if p == x {
                return x;
            }
            let grandparent = self.0[p as usize];
            self.0[x as usize] = grandparent;
            x = grandparent;
        }
    }

    fn union(&mut self, a: u32, b: u32) {
        let (ra, rb) = (self.find(a), self.find(b));
        if ra != rb {
            let (keep, merge) = if ra < rb { (ra, rb) } else { (rb, ra) };
            self.0[merge as usize] = keep;
        }
    }
}

/// One block's proportion inside the mesh, in sub-cells of `8^depth` per block: `inside` sub-cells lie wholly
/// inside, and the surface meets the `band` ones.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BlockProportion {
    inside: u32,
    band: u32,
}

impl BlockProportion {
    /// Sub-cells wholly inside the mesh.
    #[must_use]
    pub fn inside(&self) -> u32 {
        self.inside
    }

    /// Sub-cells the surface meets.
    #[must_use]
    pub fn band(&self) -> u32 {
        self.band
    }
}

/// Work counts from one [`block_proportions`] call.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ProportionStats {
    /// Blocks whose closed boxes meet the mesh's bounding box.
    pub near_blocks: u64,
    /// Blocks the surface meets.
    pub crossed_blocks: u64,
    /// Winding-number evaluations, one per surface-free component not joined to the outside.
    pub winding_queries: u64,
}

/// The proportion inside a closed mesh of every block of a grid; blocks not stored are wholly outside.
pub struct Proportions {
    grid: BlockGrid,
    depth: u8,
    blocks: VdbGrid<BlockProportion>,
    stats: ProportionStats,
}

impl std::fmt::Debug for Proportions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Proportions")
            .field("grid", &self.grid)
            .field("depth", &self.depth)
            .field("stats", &self.stats)
            .finish_non_exhaustive()
    }
}

impl Proportions {
    /// The grid.
    #[must_use]
    pub fn grid(&self) -> &BlockGrid {
        &self.grid
    }

    /// The refinement depth.
    #[must_use]
    pub fn depth(&self) -> u8 {
        self.depth
    }

    /// Sub-cells per block, `8^depth`.
    #[must_use]
    pub fn cells_per_block(&self) -> u32 {
        1 << (3 * u32::from(self.depth))
    }

    /// Block `ijk`'s proportion; zero for a block wholly outside or not in the grid.
    #[must_use]
    pub fn get(&self, ijk: [u32; 3]) -> BlockProportion {
        let (Ok(i), Ok(j), Ok(k)) = (
            i32::try_from(ijk[0]),
            i32::try_from(ijk[1]),
            i32::try_from(ijk[2]),
        ) else {
            return BlockProportion::default();
        };
        self.blocks.get([i, j, k])
    }

    /// The enclosure `[inside, inside + band] / 8^depth` of block `ijk`'s proportion inside the mesh, exact.
    #[must_use]
    pub fn bounds(&self, ijk: [u32; 3]) -> [f64; 2] {
        let p = self.get(ijk);
        let cells = f64::from(self.cells_per_block());
        [
            f64::from(p.inside) / cells,
            f64::from(p.inside + p.band) / cells,
        ]
    }

    /// The blocks with any part inside or on the surface, in `VdbGrid::iter_active` order.
    pub fn iter(&self) -> impl Iterator<Item = ([u32; 3], BlockProportion)> + '_ {
        self.blocks.iter_active().filter_map(|(c, p)| {
            Some((
                [
                    u32::try_from(c[0]).ok()?,
                    u32::try_from(c[1]).ok()?,
                    u32::try_from(c[2]).ok()?,
                ],
                p,
            ))
        })
    }

    /// The underlying sparse grid of stored blocks.
    #[must_use]
    pub fn blocks(&self) -> &VdbGrid<BlockProportion> {
        &self.blocks
    }

    /// Work counts.
    #[must_use]
    pub fn stats(&self) -> ProportionStats {
        self.stats
    }
}

/// A crossed block: its sub-grid index and coordinates, and which of its sub-cells the surface meets.
struct Crossed {
    index: u64,
    coords: [u64; 3],
    cells: Vec<bool>,
}

/// The proportion of every block of `grid` inside `mesh`, as sure/band enclosures at `depth` (plan §13.9).
///
/// # Errors
/// `FMINE-REF-109` for a depth above [`MAX_DEPTH`], a grid too fine for `f64` at its coordinates, or more than
/// 2^25 blocks and sub-cells to label (split the grid or lower the depth). `FMINE-REF-207` when a surface-free
/// component's winding number is not within 1e-6 of an integer, is negative (an inward shell), or is 2 or more
/// (overlapping shells), or when an exact predicate overflows.
pub fn block_proportions(
    mesh: &ClosedMesh,
    grid: &BlockGrid,
    depth: u8,
) -> Result<Proportions, Refusal> {
    if depth > MAX_DEPTH {
        return Err(outside_domain(format!(
            "block proportions: depth {depth} is above the maximum {MAX_DEPTH}"
        )));
    }
    let lattice = Lattice::new(grid, depth)?;
    let mut out = Proportions {
        grid: *grid,
        depth,
        blocks: VdbGrid::new(BlockProportion::default()),
        stats: ProportionStats::default(),
    };
    let Some(sub) = SubGrid::new(&lattice, &mesh.bounds) else {
        return Ok(out);
    };
    let near = sub.count();
    let side = lattice.side;
    let cells = side * side * side;
    let cells_u32 = u32::try_from(cells).unwrap_or(u32::MAX);
    let too_many = || {
        outside_domain(format!(
            "block proportions: more than {MAX_NODES} blocks and sub-cells to label; split the grid or lower the \
             depth"
        ))
    };
    if near + 1 > MAX_NODES {
        return Err(too_many());
    }

    // Bin each triangle into the near blocks its bounding box may meet; the exact test runs later, so the range
    // only has to be a superset. Near the grid the computed quotient is within a fraction of a block of the exact
    // one (a sub-cell step is above 4 ulp of the grid's coordinates), so one block either side covers a box face
    // lying on a block face; far from the grid the range is clamped anyway.
    let mut bins: BTreeMap<u64, Vec<u32>> = BTreeMap::new();
    let stride = sub.stride();
    for (t, tbox) in mesh.boxes.iter().enumerate() {
        let t = u32::try_from(t).unwrap_or(u32::MAX);
        let mut range = [[0u64; 2]; 3];
        let mut inside_range = true;
        for (axis, r) in range.iter_mut().enumerate() {
            let (o, s) = (lattice.origin[axis], lattice.size[axis]);
            let first = ((tbox[0][axis] - o) / s).floor() - 1.0;
            let last = ((tbox[1][axis] - o) / s).floor() + 1.0;
            let (lo, hi) = (
                sub.lo[axis] as f64,
                (sub.lo[axis] + sub.dims[axis] - 1) as f64,
            );
            if last < lo || first > hi {
                inside_range = false;
                break;
            }
            *r = [
                first.max(lo) as u64 - sub.lo[axis],
                last.min(hi) as u64 - sub.lo[axis],
            ];
        }
        if !inside_range {
            continue;
        }
        for z in range[2][0]..=range[2][1] {
            for y in range[1][0]..=range[1][1] {
                for x in range[0][0]..=range[0][1] {
                    bins.entry(x + stride[1] * y + stride[2] * z)
                        .or_default()
                        .push(t);
                }
            }
        }
    }

    // Mark the sub-cells the surface meets, block by block.
    let mut crossed: Vec<Crossed> = Vec::new();
    let mut slot_of: BTreeMap<u64, usize> = BTreeMap::new();
    for (&index, candidates) in &bins {
        let coords = sub.coords(index);
        let block_base: [u64; 3] = std::array::from_fn(|a| (sub.lo[a] + coords[a]) * side);
        let mut marker = Marker {
            mesh,
            lattice: &lattice,
            block_base,
            crossed: vec![false; usize::try_from(cells).unwrap_or(usize::MAX)],
            any: false,
        };
        marker.mark(block_base, side, candidates)?;
        if marker.any {
            if 1 + near + (crossed.len() as u64 + 1) * cells > MAX_NODES {
                return Err(too_many());
            }
            slot_of.insert(index, crossed.len());
            crossed.push(Crossed {
                index,
                coords,
                cells: marker.crossed,
            });
        }
    }

    // Join face-adjacent surface-free blocks and sub-cells. Node 0 is OUT, node 1 + b is near block b, and node
    // 1 + near + s·cells + c is sub-cell c of crossed block s.
    let total = 1 + near + crossed.len() as u64 * cells;
    let mut dsu = Dsu((0..u32::try_from(total).map_err(|_| too_many())?).collect());
    let block_node = |b: u64| (1 + b) as u32;
    let cell_node = |s: usize, c: u64| (1 + near + s as u64 * cells + c) as u32;
    let is_crossed = |b: u64| slot_of.contains_key(&b);
    for b in 0..near {
        if is_crossed(b) {
            continue;
        }
        let c = sub.coords(b);
        for axis in 0..3 {
            if c[axis] + 1 < sub.dims[axis] {
                let next = b + stride[axis];
                if !is_crossed(next) {
                    dsu.union(block_node(b), block_node(next));
                }
            } else if sub.beyond_out[axis][1] {
                dsu.union(block_node(b), OUT);
            }
            if c[axis] == 0 && sub.beyond_out[axis][0] {
                dsu.union(block_node(b), OUT);
            }
        }
    }
    let cell_stride = [1, side, side * side];
    for (s, block) in crossed.iter().enumerate() {
        for (c, &hit) in (0..cells).zip(&block.cells) {
            if hit {
                continue;
            }
            let me = cell_node(s, c);
            let l = [c % side, (c / side) % side, c / (side * side)];
            for axis in 0..3 {
                if l[axis] + 1 < side {
                    let next = c + cell_stride[axis];
                    if !block.cells.get(next as usize).copied().unwrap_or(true) {
                        dsu.union(me, cell_node(s, next));
                    }
                } else if block.coords[axis] + 1 == sub.dims[axis] {
                    if sub.beyond_out[axis][1] {
                        dsu.union(me, OUT);
                    }
                } else {
                    let nb = block.index + stride[axis];
                    if let Some(&s2) = slot_of.get(&nb) {
                        // The neighbour's matching sub-cell sits on its low face.
                        let mirror = c - (side - 1) * cell_stride[axis];
                        let free = crossed
                            .get(s2)
                            .and_then(|n| n.cells.get(mirror as usize))
                            .is_some_and(|&h| !h);
                        if free {
                            dsu.union(me, cell_node(s2, mirror));
                        }
                    } else {
                        dsu.union(me, block_node(nb));
                    }
                }
                if l[axis] == 0 {
                    if block.coords[axis] == 0 {
                        if sub.beyond_out[axis][0] {
                            dsu.union(me, OUT);
                        }
                    } else {
                        let nb = block.index - stride[axis];
                        if !is_crossed(nb) {
                            dsu.union(me, block_node(nb));
                        }
                    }
                }
            }
        }
    }

    // Label each component once, at its root (its smallest node), which comes first in index order.
    let mut inside_root: BTreeMap<u32, bool> = BTreeMap::new();
    let mut queries = 0u64;
    let mut label = |dsu: &mut Dsu, node: u32, point: [f64; 3]| -> Result<bool, Refusal> {
        let root = dsu.find(node);
        if root == OUT {
            return Ok(false);
        }
        if let Some(&known) = inside_root.get(&root) {
            return Ok(known);
        }
        queries += 1;
        let w = winding_exact(&mesh.soup, point)?;
        let k = w.round();
        if (w - k).abs() > INTEGRAL_TOLERANCE {
            return Err(uncertified(format!(
                "block proportions: the winding number at {point:?}, off the surface, is {w}, not within \
                 {INTEGRAL_TOLERANCE:e} of an integer"
            )));
        }
        if k < 0.0 {
            return Err(uncertified(format!(
                "block proportions: the winding number at {point:?} is {k}: a shell faces inward (reverse its \
                 triangles' vertex order)"
            )));
        }
        if k > 1.0 {
            return Err(uncertified(format!(
                "block proportions: the winding number at {point:?} is {k}: shells overlap, so the proportion \
                 inside is ambiguous"
            )));
        }
        inside_root.insert(root, k == 1.0);
        Ok(k == 1.0)
    };
    let centre =
        |b: &[[f64; 3]; 2]| -> [f64; 3] { std::array::from_fn(|a| (b[0][a] + b[1][a]) / 2.0) };
    let ijk = |coords: [u64; 3]| -> Result<[i32; 3], Refusal> {
        let mut out = [0i32; 3];
        for (o, (&c, &l)) in out.iter_mut().zip(coords.iter().zip(&sub.lo)) {
            *o = i32::try_from(c + l).map_err(|_| {
                outside_domain("block proportions: a block index exceeds i32".to_string())
            })?;
        }
        Ok(out)
    };
    for b in 0..near {
        if is_crossed(b) {
            continue;
        }
        let coords = sub.coords(b);
        let global: [u64; 3] = std::array::from_fn(|a| sub.lo[a] + coords[a]);
        if label(&mut dsu, block_node(b), centre(&lattice.block(global)))? {
            out.blocks.set(
                ijk(coords)?,
                BlockProportion {
                    inside: cells_u32,
                    band: 0,
                },
            );
        }
    }
    for (s, block) in crossed.iter().enumerate() {
        let base: [u64; 3] = std::array::from_fn(|a| (sub.lo[a] + block.coords[a]) * side);
        let (mut inside, mut band) = (0u32, 0u32);
        for (c, &hit) in (0..cells).zip(&block.cells) {
            if hit {
                band += 1;
                continue;
            }
            let l = [c % side, (c / side) % side, c / (side * side)];
            let at: [u64; 3] = std::array::from_fn(|a| base[a] + l[a]);
            if label(&mut dsu, cell_node(s, c), centre(&lattice.cell(at, 1)))? {
                inside += 1;
            }
        }
        out.blocks
            .set(ijk(block.coords)?, BlockProportion { inside, band });
    }
    out.stats = ProportionStats {
        near_blocks: near,
        crossed_blocks: crossed.len() as u64,
        winding_queries: queries,
    };
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use fmine_core::det;
    use fmine_core::hash::sha256;

    type Mesh = (Vec<[f64; 3]>, Vec<[u32; 3]>);

    /// The 12 outward triangles of the box `[lo, hi]`; vertex `i` takes `hi` on axis `a` when bit `a` of `i` is set.
    fn box_mesh(lo: [f64; 3], hi: [f64; 3]) -> Mesh {
        let positions = (0..8usize)
            .map(|i| std::array::from_fn(|a| if (i >> a) & 1 == 1 { hi[a] } else { lo[a] }))
            .collect();
        let triangles = vec![
            [0, 2, 1],
            [1, 2, 3],
            [4, 5, 6],
            [5, 7, 6],
            [0, 1, 4],
            [1, 5, 4],
            [2, 6, 3],
            [3, 6, 7],
            [0, 4, 2],
            [2, 4, 6],
            [1, 3, 5],
            [3, 7, 5],
        ];
        (positions, triangles)
    }

    fn reversed((p, t): Mesh) -> Mesh {
        (p, t.into_iter().map(|[a, b, c]| [a, c, b]).collect())
    }

    fn join((mut p, mut t): Mesh, (q, u): Mesh) -> Mesh {
        let base = u32::try_from(p.len()).unwrap();
        p.extend(q);
        t.extend(u.into_iter().map(|tri| tri.map(|v| v + base)));
        (p, t)
    }

    fn certify((p, t): Mesh) -> ClosedMesh {
        ClosedMesh::certify(p, t).unwrap()
    }

    /// The tetrahedron (0,0,0), (2,0,0), (0,3,0), (0,0,1): volume exactly 1.
    fn tetrahedron() -> Mesh {
        (
            vec![
                [0.0, 0.0, 0.0],
                [2.0, 0.0, 0.0],
                [0.0, 3.0, 0.0],
                [0.0, 0.0, 1.0],
            ],
            vec![[1, 2, 3], [0, 3, 2], [0, 1, 3], [0, 2, 1]],
        )
    }

    /// The octahedron with half-axes `r` about `c`: volume `4/3 · r0 · r1 · r2`.
    fn octahedron(c: [f64; 3], r: [f64; 3]) -> Mesh {
        let positions = vec![
            [c[0] + r[0], c[1], c[2]],
            [c[0] - r[0], c[1], c[2]],
            [c[0], c[1] + r[1], c[2]],
            [c[0], c[1] - r[1], c[2]],
            [c[0], c[1], c[2] + r[2]],
            [c[0], c[1], c[2] - r[2]],
        ];
        let mut triangles = Vec::new();
        for s in 0..8u32 {
            let (x, y, z) = (s & 1, 2 + ((s >> 1) & 1), 4 + ((s >> 2) & 1));
            // An odd number of negative half-axes flips the orientation.
            triangles.push(if s.count_ones() % 2 == 0 {
                [x, y, z]
            } else {
                [x, z, y]
            });
        }
        (positions, triangles)
    }

    /// A sphere: the octahedron split `level` times, each corner pushed out to `radius` (`sqrt` is correctly
    /// rounded). Corners are emitted per triangle; certification welds the identical ones.
    fn sphere(center: [f64; 3], radius: f64, level: u32) -> Mesh {
        let (_, faces) = octahedron([0.0; 3], [1.0; 3]);
        let unit = octahedron([0.0; 3], [1.0; 3]).0;
        let mut tris: Vec<[[f64; 3]; 3]> =
            faces.iter().map(|f| f.map(|v| unit[v as usize])).collect();
        let mid =
            |a: [f64; 3], b: [f64; 3]| -> [f64; 3] { std::array::from_fn(|i| (a[i] + b[i]) / 2.0) };
        for _ in 0..level {
            tris = tris
                .into_iter()
                .flat_map(|[a, b, c]| {
                    let (ab, bc, ca) = (mid(a, b), mid(b, c), mid(c, a));
                    [[a, ab, ca], [ab, b, bc], [ca, bc, c], [ab, bc, ca]]
                })
                .collect();
        }
        let mut positions = Vec::new();
        let mut triangles = Vec::new();
        for tri in tris {
            let base = u32::try_from(positions.len()).unwrap();
            for p in tri {
                let n = (p[0] * p[0] + p[1] * p[1] + p[2] * p[2]).sqrt();
                positions.push(std::array::from_fn(|a| center[a] + radius * p[a] / n));
            }
            triangles.push([base, base + 1, base + 2]);
        }
        (positions, triangles)
    }

    /// A torus about the z axis through `center`: `nu` × `nv` quads of two triangles, angles through `det`.
    fn torus(center: [f64; 3], major: f64, minor: f64, nu: u32, nv: u32) -> Mesh {
        let mut positions = Vec::new();
        for i in 0..nu {
            let u = 2.0 * std::f64::consts::PI * f64::from(i) / f64::from(nu);
            for j in 0..nv {
                let v = 2.0 * std::f64::consts::PI * f64::from(j) / f64::from(nv);
                let ring = major + minor * det::cos(v);
                positions.push([
                    center[0] + ring * det::cos(u),
                    center[1] + ring * det::sin(u),
                    center[2] + minor * det::sin(v),
                ]);
            }
        }
        let at = |i: u32, j: u32| (i % nu) * nv + (j % nv);
        let mut triangles = Vec::new();
        for i in 0..nu {
            for j in 0..nv {
                triangles.push([at(i, j), at(i + 1, j), at(i + 1, j + 1)]);
                triangles.push([at(i, j), at(i + 1, j + 1), at(i, j + 1)]);
            }
        }
        (positions, triangles)
    }

    /// The winding number of the boxes `(lo, hi, ±1)` over a closed sub-cell, or `None` when the sub-cell meets a
    /// box's surface. Plain coordinate comparisons, independent of the module.
    fn analytic(boxes: &[([f64; 3], [f64; 3], i32)], cell: [[f64; 3]; 2]) -> Option<i32> {
        let mut k = 0;
        for &(lo, hi, s) in boxes {
            let meets = (0..3).all(|a| cell[0][a] <= hi[a] && cell[1][a] >= lo[a]);
            let within = (0..3).all(|a| cell[0][a] > lo[a] && cell[1][a] < hi[a]);
            if meets && !within {
                return None;
            }
            if within {
                k += s;
            }
        }
        Some(k)
    }

    /// Checks every block of a dyadic grid against the analytic inside and band counts; returns the totals.
    fn check_boxes(
        boxes: &[([f64; 3], [f64; 3], i32)],
        origin: [f64; 3],
        size: [f64; 3],
        counts: [u32; 3],
        depth: u8,
    ) -> (u64, u64) {
        let mut mesh: Mesh = (Vec::new(), Vec::new());
        for &(lo, hi, s) in boxes {
            let b = box_mesh(lo, hi);
            mesh = join(mesh, if s > 0 { b } else { reversed(b) });
        }
        let mesh = certify(mesh);
        let grid = BlockGrid::new(origin, size, counts).unwrap();
        let got = block_proportions(&mesh, &grid, depth).unwrap();
        let side = 1u32 << depth;
        let (mut inside_total, mut band_total) = (0u64, 0u64);
        for k in 0..counts[2] {
            for j in 0..counts[1] {
                for i in 0..counts[0] {
                    let ijk = [i, j, k];
                    let (mut inside, mut band) = (0u32, 0u32);
                    for z in 0..side {
                        for y in 0..side {
                            for x in 0..side {
                                let l = [x, y, z];
                                let at = |a: usize, d: u32| {
                                    origin[a]
                                        + size[a]
                                            * (f64::from(ijk[a] * side + l[a] + d)
                                                / f64::from(side))
                                };
                                let cell = [
                                    std::array::from_fn(|a| at(a, 0)),
                                    std::array::from_fn(|a| at(a, 1)),
                                ];
                                match analytic(boxes, cell) {
                                    None => band += 1,
                                    Some(1) => inside += 1,
                                    Some(0) => {}
                                    Some(k) => panic!("analytic winding {k} at {cell:?}"),
                                }
                            }
                        }
                    }
                    assert_eq!(
                        got.get(ijk),
                        BlockProportion { inside, band },
                        "block {ijk:?} at depth {depth}"
                    );
                    inside_total += u64::from(inside);
                    band_total += u64::from(band);
                }
            }
        }
        let stats = got.stats();
        println!(
            "{{\"test\":\"volcert_boxes\",\"depth\":{depth},\"inside\":{inside_total},\"band\":{band_total},\
             \"near_blocks\":{},\"crossed_blocks\":{},\"winding_queries\":{}}}",
            stats.near_blocks, stats.crossed_blocks, stats.winding_queries
        );
        (inside_total, band_total)
    }

    #[test]
    fn box_wireframes_match_the_analytic_sub_cell_counts() {
        let (origin, size, counts) = ([0.0; 3], [0.5, 0.25, 1.0], [4, 5, 3]);
        // Faces off the lattice: every sub-cell is inside, outside or cut.
        let one = [([0.1, 0.2, 0.3], [1.37, 0.93, 2.6], 1)];
        for depth in 0..=3 {
            let (inside, band) = check_boxes(&one, origin, size, counts, depth);
            assert!(inside > 0 && band > 0);
        }
        // Faces on sub-cell boundaries: the closed sub-cells on both sides touch the surface, so both are band.
        let aligned = [([0.5, 0.25, 1.0], [1.25, 0.75, 2.0], 1)];
        check_boxes(&aligned, origin, size, counts, 2);
        // A hollow: an inward box inside an outward one winds 0 in the cavity.
        let hollow = [
            ([0.1, 0.2, 0.3], [1.9, 1.2, 2.8], 1),
            ([0.6, 0.45, 0.9], [1.3, 0.95, 2.1], -1),
        ];
        check_boxes(&hollow, origin, size, counts, 3);
        // A box running past the grid on three sides: the clipped faces join nothing outside, so their components
        // are labelled by winding queries.
        let past = [([-0.3, 0.2, 0.3], [1.37, 1.6, 3.4], 1)];
        check_boxes(&past, origin, size, counts, 2);
    }

    #[test]
    fn grids_wholly_inside_or_outside_the_mesh() {
        let mesh = certify(box_mesh([0.0; 3], [2.0; 3]));
        let inside = BlockGrid::new([0.5; 3], [0.25; 3], [2; 3]).unwrap();
        let got = block_proportions(&mesh, &inside, 3).unwrap();
        assert_eq!(got.iter().count(), 8);
        assert!(got.iter().all(|(_, p)| p
            == BlockProportion {
                inside: 512,
                band: 0
            }));
        assert_eq!(got.bounds([1, 1, 1]), [1.0, 1.0]);
        assert_eq!(
            got.stats().winding_queries,
            1,
            "one component, no face beyond the mesh"
        );
        let away = BlockGrid::new([5.0; 3], [0.25; 3], [2; 3]).unwrap();
        let got = block_proportions(&mesh, &away, 3).unwrap();
        assert_eq!(got.iter().count(), 0);
        assert_eq!(got.stats(), ProportionStats::default());
    }

    #[test]
    fn volumes_enclose_exact_analytic_values_even_far_from_the_origin() {
        let tight = |v: VolumeEnclosure, exact: f64| {
            assert!(v.lo() <= exact && exact <= v.hi(), "{v:?} misses {exact}");
            assert!(
                v.hi() - v.lo() <= 16.0 * f64::EPSILON * exact,
                "{v:?} is wide"
            );
        };
        let lo = [0.5, -1.0, 0.125];
        let hi = [2.25, 3.0, 1.0];
        tight(certify(box_mesh(lo, hi)).volume(), 6.125);
        tight(certify(tetrahedron()).volume(), 1.0);
        tight(
            certify(octahedron([10.5, -4.25, 7.0], [3.0, 1.0, 2.0])).volume(),
            8.0,
        );
        // A UTM-sized offset: the exact sum cancels it, where a plain f64 divergence sum does not.
        let shift = [500_000.0, 7_000_000.0, 1_000.0];
        let far = box_mesh(
            std::array::from_fn(|a| lo[a] + shift[a]),
            std::array::from_fn(|a| hi[a] + shift[a]),
        );
        let naive: f64 = far
            .1
            .iter()
            .map(|t| {
                let [a, b, c] = t.map(|v| far.0[v as usize]);
                (a[0] * (b[1] * c[2] - b[2] * c[1])
                    + a[1] * (b[2] * c[0] - b[0] * c[2])
                    + a[2] * (b[0] * c[1] - b[1] * c[0]))
                    / 6.0
            })
            .sum();
        let v = certify(far).volume();
        tight(v, 6.125);
        println!(
            "{{\"test\":\"volcert_far_box\",\"lo\":{:e},\"hi\":{:e},\"naive_f64\":{naive:e}}}",
            v.lo(),
            v.hi()
        );
        // A refined sphere sits inside its circumscribed ball.
        let s = certify(sphere([0.0; 3], 1.0, 4)).volume();
        assert!(s.hi() < 4.0 / 3.0 * std::f64::consts::PI && s.lo() > 4.0);
    }

    /// Over a grid covering the mesh, `sum(inside)` and `sum(inside + band)` bracket the exact volume, and the band
    /// thins as the depth grows. Returns the band volume per depth.
    fn bracket(
        mesh: &ClosedMesh,
        grid: &BlockGrid,
        depths: std::ops::RangeInclusive<u8>,
        name: &str,
    ) -> Vec<f64> {
        let v = mesh.volume();
        let block = grid.size().iter().product::<f64>();
        let mut bands = Vec::new();
        for depth in depths {
            let got = block_proportions(mesh, grid, depth).unwrap();
            let cell = block / f64::from(got.cells_per_block());
            let (inside, band) = got.iter().fold((0u64, 0u64), |(i, b), (_, p)| {
                (i + u64::from(p.inside()), b + u64::from(p.band()))
            });
            let (lo, hi) = (inside as f64 * cell, (inside + band) as f64 * cell);
            assert!(
                lo <= v.hi() && v.lo() <= hi,
                "{name} depth {depth}: [{lo}, {hi}] misses {v:?}"
            );
            let stats = got.stats();
            println!(
                "{{\"test\":\"volcert_bracket\",\"mesh\":\"{name}\",\"depth\":{depth},\"inside_volume\":{lo},\
                 \"outer_volume\":{hi},\"exact_lo\":{},\"near_blocks\":{},\"crossed_blocks\":{},\
                 \"winding_queries\":{}}}",
                v.lo(),
                stats.near_blocks,
                stats.crossed_blocks,
                stats.winding_queries
            );
            bands.push(hi - lo);
        }
        bands
    }

    #[test]
    fn sums_bracket_the_exact_volume_and_the_band_thins_with_depth() {
        let grid = BlockGrid::new([0.0; 3], [0.25; 3], [12; 3]).unwrap();
        let ball = certify(sphere([1.3, 1.1, 0.9], 1.0, 3));
        let bands = bracket(&ball, &grid, 0..=3, "sphere");
        for w in bands.windows(2) {
            assert!(w[1] <= 0.75 * w[0], "band {bands:?} does not thin");
        }
        let ring_grid = BlockGrid::new([0.0; 3], [0.25; 3], [16, 16, 8]).unwrap();
        let ring = certify(torus([2.0, 2.0, 1.0], 1.2, 0.4, 32, 16));
        let bands = bracket(&ring, &ring_grid, 0..=2, "torus");
        assert!(bands[2] <= 0.75 * bands[1]);
        // The hole: the block on the axis is wholly outside.
        let got = block_proportions(&ring, &ring_grid, 2).unwrap();
        assert_eq!(got.get([7, 7, 3]), BlockProportion::default());
        assert!(got.iter().any(|(_, p)| p.inside() > 0));
    }

    #[test]
    fn a_clipped_grid_labels_its_blocks_as_the_full_grid_does() {
        let mesh = certify(sphere([1.3, 1.1, 0.9], 1.0, 3));
        let full = BlockGrid::new([0.0; 3], [0.25; 3], [12; 3]).unwrap();
        // Offset (3, 2, 0) blocks: the clipped grid cuts through the sphere on four more faces than the full grid.
        let clipped = BlockGrid::new([0.75, 0.5, 0.0], [0.25; 3], [4, 6, 12]).unwrap();
        let a = block_proportions(&mesh, &full, 3).unwrap();
        let b = block_proportions(&mesh, &clipped, 3).unwrap();
        let mut compared = 0;
        for k in 0..12 {
            for j in 0..6 {
                for i in 0..4 {
                    assert_eq!(
                        b.get([i, j, k]),
                        a.get([i + 3, j + 2, k]),
                        "block {i} {j} {k}"
                    );
                    compared += 1;
                }
            }
        }
        assert_eq!(compared, 288);
        assert!(b.stats().winding_queries >= 1);
    }

    #[test]
    fn welding_joins_identical_positions_including_signed_zero() {
        let (p, t) = box_mesh([0.0, -1.0, 0.0], [1.0, 1.0, 2.0]);
        // Every triangle with its own three corners, some zeros written as -0.0.
        let mut positions = Vec::new();
        let mut triangles = Vec::new();
        for (n, tri) in t.iter().enumerate() {
            let base = u32::try_from(positions.len()).unwrap();
            for &v in tri {
                let q = p[v as usize];
                positions.push(if n % 2 == 0 {
                    q.map(|x| if x == 0.0 { -0.0 } else { x })
                } else {
                    q
                });
            }
            triangles.push([base, base + 1, base + 2]);
        }
        let welded = ClosedMesh::certify(positions, triangles).unwrap();
        assert_eq!(welded.soup().position_count(), 8);
        assert_eq!(welded.volume(), certify((p, t)).volume());
    }

    fn refusal(mesh: Mesh) -> Refusal {
        ClosedMesh::certify(mesh.0, mesh.1).unwrap_err()
    }

    #[test]
    fn meshes_that_are_not_closed_and_outward_are_refused() {
        let cube = || box_mesh([0.0; 3], [1.0; 3]);
        let r = refusal((Vec::new(), Vec::new()));
        assert_eq!(r.code, "FMINE-REF-207");
        let (p, mut t) = cube();
        t.pop();
        let r = refusal((p, t));
        assert!(
            r.code == "FMINE-REF-207" && r.what.contains("not closed"),
            "{}",
            r.what
        );
        let (p, mut t) = cube();
        t[0] = [t[0][0], t[0][2], t[0][1]];
        let r = refusal((p, t));
        assert!(r.what.contains("not consistently"), "{}", r.what);
        let (p, mut t) = cube();
        t.push([0, 0, 1]);
        assert!(refusal((p, t)).what.contains("repeats a vertex"));
        let (p, mut t) = cube();
        t.push([0, 1, 99]);
        assert_eq!(refusal((p, t)).code, "FMINE-REF-109");
        let (mut p, t) = cube();
        p[7][2] = f64::NAN;
        assert_eq!(refusal((p, t)).code, "FMINE-REF-109");
        for bad in [1e-70, 1e70] {
            let (mut p, t) = cube();
            p[7][2] = bad;
            let r = refusal((p, t));
            assert!(
                r.code == "FMINE-REF-109" && r.what.contains("2^-200"),
                "{}",
                r.what
            );
        }
        let r = refusal(reversed(cube()));
        assert!(
            r.code == "FMINE-REF-207" && r.what.contains("face inward"),
            "{}",
            r.what
        );
    }

    #[test]
    fn overlapping_and_stray_inward_shells_are_refused_not_counted() {
        let grid = BlockGrid::new([0.0; 3], [0.25; 3], [16; 3]).unwrap();
        let overlap = certify(join(
            box_mesh([0.1; 3], [1.9; 3]),
            box_mesh([1.1; 3], [2.9; 3]),
        ));
        let r = block_proportions(&overlap, &grid, 1).unwrap_err();
        assert!(
            r.code == "FMINE-REF-207" && r.what.contains("shells overlap"),
            "{}",
            r.what
        );
        // An inward box beside, not inside, a bigger outward one: the volume is positive, the label is -1.
        let stray = certify(join(
            box_mesh([0.1; 3], [2.1; 3]),
            reversed(box_mesh([2.6, 0.1, 0.1], [3.4, 0.9, 0.9])),
        ));
        let r = block_proportions(&stray, &grid, 1).unwrap_err();
        assert!(
            r.code == "FMINE-REF-207" && r.what.contains("faces inward"),
            "{}",
            r.what
        );
    }

    #[test]
    fn bad_grids_and_depths_are_refused() {
        let o = [0.0; 3];
        for (origin, size, counts) in [
            (o, [0.0, 1.0, 1.0], [1, 1, 1]),
            (o, [-1.0, 1.0, 1.0], [1, 1, 1]),
            ([f64::NAN, 0.0, 0.0], [1.0; 3], [1, 1, 1]),
            (o, [1.0; 3], [0, 1, 1]),
            (o, [1.0; 3], [1 << 31, 1, 1]),
            ([1e308, 0.0, 0.0], [1e308, 1.0, 1.0], [2, 1, 1]),
        ] {
            assert_eq!(
                BlockGrid::new(origin, size, counts).unwrap_err().code,
                "FMINE-REF-109"
            );
        }
        let mesh = certify(box_mesh([0.0; 3], [1.0; 3]));
        let grid = BlockGrid::new(o, [0.5; 3], [2; 3]).unwrap();
        let r = block_proportions(&mesh, &grid, MAX_DEPTH + 1).unwrap_err();
        assert_eq!(r.code, "FMINE-REF-109");
        let fine = BlockGrid::new([1e15, 0.0, 0.0], [1e-3, 1.0, 1.0], [4, 1, 1]).unwrap();
        let r = block_proportions(&mesh, &fine, 6).unwrap_err();
        assert!(
            r.code == "FMINE-REF-109" && r.what.contains("units in the last place"),
            "{}",
            r.what
        );
    }

    /// The digest of the three fixture families' volumes and block proportions (sphere, torus, hollow box).
    fn fixture_digest() -> (String, usize) {
        let mut bytes = Vec::new();
        let mut blocks = 0;
        let hollow = join(
            box_mesh([0.1, 0.2, 0.3], [1.9, 1.2, 2.8]),
            reversed(box_mesh([0.6, 0.45, 0.9], [1.3, 0.95, 2.1])),
        );
        let cases = [
            (
                sphere([1.3, 1.1, 0.9], 1.0, 3),
                BlockGrid::new([0.0; 3], [0.25; 3], [12; 3]).unwrap(),
                3,
            ),
            (
                torus([2.0, 2.0, 1.0], 1.2, 0.4, 32, 16),
                BlockGrid::new([0.0; 3], [0.25; 3], [16, 16, 8]).unwrap(),
                2,
            ),
            (
                hollow,
                BlockGrid::new([0.0; 3], [0.5, 0.25, 1.0], [4, 5, 3]).unwrap(),
                3,
            ),
        ];
        for (mesh, grid, depth) in cases {
            let mesh = certify(mesh);
            let v = mesh.volume();
            bytes.extend(v.lo().to_bits().to_le_bytes());
            bytes.extend(v.hi().to_bits().to_le_bytes());
            let got = block_proportions(&mesh, &grid, depth).unwrap();
            for (ijk, p) in got.iter() {
                for x in ijk.into_iter().chain([p.inside(), p.band()]) {
                    bytes.extend(x.to_le_bytes());
                }
                blocks += 1;
            }
        }
        (sha256(&bytes).to_hex(), blocks)
    }

    /// Pinned on x86_64-linux, dev and release, and reproduced on aarch64-darwin (GATE-DET-01).
    const FIXTURE_DIGEST: &str = "8eed3487ef39434b47cfa490724b83cb83f1a7f7d1cabeadf631178369dda9fb";

    #[test]
    fn fixture_digest_is_pinned() {
        let (digest, blocks) = fixture_digest();
        println!(
            "{{\"test\":\"volcert_digest\",\"arch\":\"{}\",\"digest\":\"{digest}\",\"blocks\":{blocks}}}",
            std::env::consts::ARCH
        );
        assert_eq!(digest, FIXTURE_DIGEST);
    }
}
