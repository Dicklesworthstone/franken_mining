//! Determinism: BitwisePortable (`i64`, `i128` and `u128` integer arithmetic only; no floating point, hashing or
//! threads).
//!
//! Precedence arcs from slope templates (plan §15.4: "implicit precedence arcs from slope templates (azimuth sector
//! × geotechnical domain from §14.2)"; `bd-t75z`).
//!
//! **Grid.** Block `(i, j, k)` is `(k·ny + j)·nx + i`, as everywhere in `pitflow`.
//! - `i` runs east and `j` north.
//! - `k = 0` is the top layer.
//! - Block sizes are positive integers in one caller-chosen length unit.
//!
//! **Cone.** A block requires every block whose centre lies inside its upward cone, up to `levels` layers above.
//! - Take an upper block at horizontal offset `(Δx, Δy)` and `Δk` layers up, so `Δz = Δk·dz`.
//! - It is inside when `√(Δx² + Δy²) · tan θ ≤ Δz`, tested exactly as `(Δx² + Δy²)·rise² ≤ Δz²·run²`.
//! - `tan θ = rise / run` is the slope of the lower block's domain in the azimuth sector holding `(Δx, Δy)`: the
//!   direction from the lower block to the upper one, so the bearing of the wall from the pit floor.
//! - The block straight above is always inside.
//!
//! **Sectors.** A domain's sectors are listed counterclockwise (east toward north).
//! - Each sector starts at a boundary direction `(east, north)` and runs up to the next boundary, wrapping round.
//! - A direction on a boundary belongs to the sector that starts there.
//! - Every comparison is an exact cross or dot product.
//!
//! **Fewer arcs, same closure.** An offset `o` is dropped when the block's cone holds some `o₁` with fewer layers
//! (`Δk₁ < Δk`) that reaches a block whose own cone holds `o − o₁`. By induction on `Δk`, the arcs kept still imply
//! every block of every K-level cone: `o₁` and `o − o₁` each span fewer layers than `o`, so each is kept or implied
//! in turn.
//! - Interior blocks share one reduced pattern per domain. A block is interior when its cone's horizontal reach lies
//!   inside the grid and every block of that reach box above it is in its own domain. The test then comes out the
//!   same for each of them; near the top of the grid too, since `o₁` lies in a layer between a block and `o`.
//! - Other blocks are near a side of the grid or a domain contact; `boundary_blocks` counts them. Each of their
//!   offsets is tested against up to [`WITNESSES`] candidates for `o₁`, chosen from the interior pattern with those
//!   straight up first, since those always exist. That may keep a few arcs a full search would drop; it never drops
//!   one it should keep.
//! - The interior check counts, by a 3D prefix sum, the blocks in the box whose next block east, north or below lies
//!   in another domain; any such block, even one on the box's face, sends the block to the boundary path.
//!
//! **Accuracy.** Requiring each block's K-level cone is the usual approximation.
//! - Within `levels` layers, the pit's closure holds every block of the true cone.
//! - Further up it holds only what chains of K-level cones reach, so a pit wall may be steeper there than its slope
//!   allows. More levels shrink the gap.
//! - With one slope everywhere the closure never leaves the true cone, since `|o₁ + o₂| ≤ |o₁| + |o₂|`; no wall is
//!   flatter than it must be. With slopes that vary by sector or domain this need not hold.

use super::{NONE, refuse};
use fmine_evidence::Refusal;

/// The most offsets one domain's cone may hold; more is refused rather than built.
pub const MAX_PATTERN: usize = 1 << 16;

/// The most candidate offsets scanned across all domains' reach boxes; more is refused rather than scanned.
pub const MAX_SCAN: u64 = 1 << 26;

/// The most candidates for `o₁` kept for each implied offset, for blocks near a side or a domain contact.
pub const WITNESSES: usize = 8;

/// A regular block grid: counts east, north and down, and block sizes in one integer length unit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BlockGrid {
    n: [u32; 3],
    size: [u32; 3],
}

impl BlockGrid {
    /// `counts` and `sizes` are `[east, north, down]`.
    ///
    /// # Errors
    /// `FMINE-REF-207` (proposed) when a count or size is zero, or there are `u32::MAX` or more blocks.
    pub fn new(counts: [u32; 3], sizes: [u32; 3]) -> Result<Self, Refusal> {
        if counts.contains(&0) || sizes.contains(&0) {
            return Err(refuse(format!(
                "slope: block counts {counts:?} and sizes {sizes:?} must all be positive"
            )));
        }
        let blocks = counts.iter().map(|&c| u128::from(c)).product::<u128>();
        if blocks >= u128::from(NONE) {
            return Err(refuse(format!(
                "slope: {blocks} blocks; there must be fewer than {NONE}"
            )));
        }
        Ok(Self {
            n: counts,
            size: sizes,
        })
    }

    /// Number of blocks.
    #[must_use]
    pub fn block_count(&self) -> usize {
        self.n.iter().map(|&c| c as usize).product()
    }

    /// The index of block `(i, j, k)`, or `None` outside the grid.
    #[must_use]
    pub fn index(&self, i: u32, j: u32, k: u32) -> Option<usize> {
        let [nx, ny, nz] = self.n;
        (i < nx && j < ny && k < nz)
            .then(|| (k as usize * ny as usize + j as usize) * nx as usize + i as usize)
    }

    /// The index of an in-grid block given as `i64`s.
    fn at(&self, [i, j, k]: [i64; 3]) -> usize {
        let [nx, ny, _] = self.n.map(|c| c as usize);
        (k as usize * ny + j as usize) * nx + i as usize
    }

    /// Block `(i, j, k)` moved by `o`, `o[2]` layers up, if that block exists.
    fn shifted(&self, [i, j, k]: [i64; 3], o: Offset) -> Option<usize> {
        let to = |v: i64| u32::try_from(v).ok();
        self.index(to(i + o[0])?, to(j + o[1])?, to(k - o[2])?)
    }
}

/// A slope as the exact tangent `rise / run` of its angle from horizontal, both positive.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SlopeTangent {
    rise: u32,
    run: u32,
}

impl SlopeTangent {
    /// # Errors
    /// `FMINE-REF-207` (proposed) unless both parts are positive.
    pub fn new(rise: u32, run: u32) -> Result<Self, Refusal> {
        if rise == 0 || run == 0 {
            return Err(refuse(format!(
                "slope: tangent {rise}/{run} must have a positive rise and run"
            )));
        }
        Ok(Self { rise, run })
    }
}

/// One azimuth sector: it starts at the boundary direction `(east, north)` and holds one slope.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sector {
    from: [i32; 2],
    tangent: SlopeTangent,
}

impl Sector {
    /// # Errors
    /// `FMINE-REF-207` (proposed) when the boundary direction is `(0, 0)`.
    pub fn new(from_east: i32, from_north: i32, tangent: SlopeTangent) -> Result<Self, Refusal> {
        if from_east == 0 && from_north == 0 {
            return Err(refuse(
                "slope: a sector boundary needs a direction, not (0, 0)".to_string(),
            ));
        }
        Ok(Self {
            from: [from_east, from_north],
            tangent,
        })
    }
}

fn cross(a: [i64; 2], b: [i64; 2]) -> i128 {
    i128::from(a[0]) * i128::from(b[1]) - i128::from(a[1]) * i128::from(b[0])
}

fn dot(a: [i64; 2], b: [i64; 2]) -> i128 {
    i128::from(a[0]) * i128::from(b[0]) + i128::from(a[1]) * i128::from(b[1])
}

/// Whether the counterclockwise angle from `b0` to `a` is less than that to `c`; both angles are in `[0, 2π)`.
fn angle_less(b0: [i64; 2], a: [i64; 2], c: [i64; 2]) -> bool {
    let half = |v: [i64; 2]| {
        let (cr, dt) = (cross(b0, v), dot(b0, v));
        u8::from(!(cr > 0 || (cr == 0 && dt > 0)))
    };
    let (ha, hc) = (half(a), half(c));
    if ha != hc {
        return ha < hc;
    }
    cross(a, c) > 0
}

fn wide(v: [i32; 2]) -> [i64; 2] {
    [i64::from(v[0]), i64::from(v[1])]
}

/// Slopes per geotechnical domain, each domain a counterclockwise list of sectors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlopeTable {
    domains: Vec<Vec<Sector>>,
}

impl SlopeTable {
    /// # Errors
    /// `FMINE-REF-207` (proposed):
    /// - when there are no domains, or more than 65,536;
    /// - when a domain has no sectors;
    /// - when a domain's boundaries do not go strictly counterclockwise, within one turn, from its first.
    pub fn new(domains: Vec<Vec<Sector>>) -> Result<Self, Refusal> {
        if domains.is_empty() || domains.len() > usize::from(u16::MAX) + 1 {
            return Err(refuse(format!(
                "slope: {} domains; there must be 1 to 65,536",
                domains.len()
            )));
        }
        for (d, sectors) in domains.iter().enumerate() {
            let Some(first) = sectors.first() else {
                return Err(refuse(format!("slope: domain {d} has no sectors")));
            };
            let b0 = wide(first.from);
            if let Some(s) = (1..sectors.len())
                .find(|&s| !angle_less(b0, wide(sectors[s - 1].from), wide(sectors[s].from)))
            {
                return Err(refuse(format!(
                    "slope: domain {d}'s sector {s} starts at {:?}, which is not counterclockwise of {:?} within \
                     one turn of {:?}",
                    sectors[s].from,
                    sectors[s - 1].from,
                    first.from
                )));
            }
        }
        Ok(Self { domains })
    }

    /// One domain with one slope in every direction.
    #[must_use]
    pub fn uniform(tangent: SlopeTangent) -> Self {
        Self {
            domains: vec![vec![Sector {
                from: [1, 0],
                tangent,
            }]],
        }
    }

    /// The slope in `domain` for the direction `v`, which is not `(0, 0)`.
    fn tangent(&self, domain: usize, v: [i64; 2]) -> SlopeTangent {
        let sectors = &self.domains[domain];
        let b0 = wide(sectors[0].from);
        let mut at = 0;
        for (s, sector) in sectors.iter().enumerate().skip(1) {
            if angle_less(b0, v, wide(sector.from)) {
                break;
            }
            at = s;
        }
        sectors[at].tangent
    }
}

/// Precedence arcs in CSR form, for [`super::ClosureProblem::new`], with counts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlopeArcs {
    offsets: Vec<usize>,
    targets: Vec<u32>,
    full_cone_arcs: u64,
    boundary_blocks: usize,
}

impl SlopeArcs {
    /// CSR row offsets, one more than there are blocks.
    #[must_use]
    pub fn offsets(&self) -> &[usize] {
        &self.offsets
    }

    /// The blocks each block requires, in CSR order.
    #[must_use]
    pub fn targets(&self) -> &[u32] {
        &self.targets
    }

    /// Arcs kept.
    #[must_use]
    pub fn arc_count(&self) -> usize {
        self.targets.len()
    }

    /// Arcs there would be with every block of every K-level cone required directly.
    #[must_use]
    pub fn full_cone_arcs(&self) -> u64 {
        self.full_cone_arcs
    }

    /// Blocks whose arcs were reduced one by one, near a side of the grid or a domain contact.
    #[must_use]
    pub fn boundary_blocks(&self) -> usize {
        self.boundary_blocks
    }

    /// `(offsets, targets)`.
    #[must_use]
    pub fn into_csr(self) -> (Vec<usize>, Vec<u32>) {
        (self.offsets, self.targets)
    }
}

/// An offset `(Δi, Δj, Δk)`, `Δk ≥ 1` layers up.
type Offset = [i64; 3];

fn sub(a: Offset, b: Offset) -> Offset {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

/// One domain's cone.
struct Pattern {
    reach: [i64; 2],
    levels: i64,
    /// Which offsets of the reach box the cone holds, layer by layer.
    member: Vec<bool>,
    /// The offsets it holds, by layer, then north, then east.
    full: Vec<Offset>,
    /// For each offset of `full`: none when it is kept, else up to [`WITNESSES`] indices into `full` of offsets `o₁`
    /// with fewer layers whose remainder the cone holds, those straight up first.
    witnesses: Vec<Vec<u32>>,
    reduced: Vec<Offset>,
    /// How many offsets of `full` and of `reduced` span at most `dk` layers, for `dk` from 0 to `levels`.
    full_upto: Vec<usize>,
    reduced_upto: Vec<usize>,
}

impl Pattern {
    fn holds(&self, o: Offset) -> bool {
        let [ri, rj] = self.reach;
        if o[2] < 1 || o[2] > self.levels || o[0].abs() > ri || o[1].abs() > rj {
            return false;
        }
        let (w, h) = (2 * ri + 1, 2 * rj + 1);
        self.member[(((o[2] - 1) * h + o[1] + rj) * w + o[0] + ri) as usize]
    }
}

/// The largest `|Δi|` and `|Δj|` a cone of `sectors` reaches within `levels` layers, and the size of that box.
fn reach(grid: &BlockGrid, sectors: &[Sector], levels: i64) -> ([i64; 2], u128) {
    let [dx, dy, dz] = grid.size.map(u128::from);
    let layers = u128::from(levels.unsigned_abs());
    let flattest = sectors.iter().fold(0u128, |best, s| {
        best.max(dz * layers * u128::from(s.tangent.run) / u128::from(s.tangent.rise))
    });
    let cap = |d: u128, n: u32| (flattest / d).min(u128::from(n));
    let (ri, rj) = (cap(dx, grid.n[0]), cap(dy, grid.n[1]));
    ([ri as i64, rj as i64], (2 * ri + 1) * (2 * rj + 1) * layers)
}

/// Whether `o` lies in the cone of `domain` on `grid`.
fn inside(grid: &BlockGrid, table: &SlopeTable, domain: usize, o: Offset) -> Result<bool, Refusal> {
    if o[0] == 0 && o[1] == 0 {
        return Ok(true);
    }
    let [dx, dy, dz] = grid.size.map(i64::from);
    let overflow = || refuse(format!("slope: offset {o:?} overflows the exact cone test"));
    let ex = o[0].checked_mul(dx).ok_or_else(overflow)?;
    let ny = o[1].checked_mul(dy).ok_or_else(overflow)?;
    let ez = o[2].checked_mul(dz).ok_or_else(overflow)?;
    let t = table.tangent(domain, [ex, ny]);
    let sq = |v: i64| u128::from(v.unsigned_abs()).checked_pow(2);
    let lhs = sq(ex)
        .zip(sq(ny))
        .and_then(|(a, b)| a.checked_add(b))
        .and_then(|h| h.checked_mul(u128::from(t.rise).pow(2)));
    let rhs = sq(ez).and_then(|z| z.checked_mul(u128::from(t.run).pow(2)));
    let (lhs, rhs) = lhs.zip(rhs).ok_or_else(overflow)?;
    Ok(lhs <= rhs)
}

/// Every domain's pattern, after checking the scan and pattern limits.
fn patterns(grid: &BlockGrid, table: &SlopeTable, levels: i64) -> Result<Vec<Pattern>, Refusal> {
    let reaches: Vec<([i64; 2], u128)> = table
        .domains
        .iter()
        .map(|sectors| reach(grid, sectors, levels))
        .collect();
    let scan: u128 = reaches.iter().map(|(_, cells)| cells).sum();
    if scan > u128::from(MAX_SCAN) {
        return Err(refuse(format!(
            "slope: the cones' reach boxes hold {scan} candidate offsets over {levels} layers, above {MAX_SCAN}; use \
             fewer layers, coarser blocks or steeper slopes"
        )));
    }
    let mut out = Vec::with_capacity(reaches.len());
    for (d, &([ri, rj], cells)) in reaches.iter().enumerate() {
        let mut member = Vec::with_capacity(cells as usize);
        let mut full = Vec::new();
        for dk in 1..=levels {
            for dj in -rj..=rj {
                for di in -ri..=ri {
                    let o = [di, dj, dk];
                    let hit = inside(grid, table, d, o)?;
                    member.push(hit);
                    if hit {
                        full.push(o);
                        if full.len() > MAX_PATTERN {
                            return Err(refuse(format!(
                                "slope: domain {d}'s cone holds more than {MAX_PATTERN} offsets over {levels} \
                                 layers; use fewer layers, coarser blocks or steeper slopes"
                            )));
                        }
                    }
                }
            }
        }
        let mut p = Pattern {
            reach: [ri, rj],
            levels,
            member,
            full,
            witnesses: Vec::new(),
            reduced: Vec::new(),
            full_upto: Vec::new(),
            reduced_upto: Vec::new(),
        };
        // Both lists keep `full`'s order by layer.
        let (straight_up, others): (Vec<usize>, Vec<usize>) =
            (0..p.full.len()).partition(|&q| p.full[q][0] == 0 && p.full[q][1] == 0);
        p.witnesses = p
            .full
            .iter()
            .map(|&o| {
                let fewer_layers = |q: &&usize| p.full[**q][2] < o[2];
                straight_up
                    .iter()
                    .take_while(fewer_layers)
                    .chain(others.iter().take_while(fewer_layers))
                    .filter(|&&q| p.holds(sub(o, p.full[q])))
                    .take(WITNESSES)
                    .map(|&q| q as u32)
                    .collect()
            })
            .collect();
        p.reduced = p
            .full
            .iter()
            .zip(&p.witnesses)
            .filter(|(_, w)| w.is_empty())
            .map(|(&o, _)| o)
            .collect();
        p.full_upto = (0..=levels)
            .map(|dk| p.full.partition_point(|o| o[2] <= dk))
            .collect();
        p.reduced_upto = (0..=levels)
            .map(|dk| p.reduced.partition_point(|o| o[2] <= dk))
            .collect();
        out.push(p);
    }
    Ok(out)
}

/// Precedence arcs for `grid`: each block requires its K-level cone under the slopes of its own domain,
/// `domain_of[block]`, with implied arcs dropped. Levels past the bottom of the grid are not needed and not used.
///
/// # Errors
/// `FMINE-REF-207` (proposed):
/// - when `levels` is 0, or `domain_of` does not have one entry per block, or names a domain the table lacks;
/// - when the reach boxes exceed [`MAX_SCAN`], a cone holds more than [`MAX_PATTERN`] offsets, or the cone test
///   would overflow;
/// - when there would be `u32::MAX` or more arcs.
pub fn slope_arcs(
    grid: &BlockGrid,
    table: &SlopeTable,
    domain_of: &[u16],
    levels: u32,
) -> Result<SlopeArcs, Refusal> {
    slope_arcs_bounded(grid, table, domain_of, levels, NONE as usize - 1)
}

/// The same arcs as [`slope_arcs`], with a caller's maximum retained arc count. Zero permits an empty graph.
/// The limit is checked before each append, and target-buffer growth never requests capacity above it.
/// The existing pattern, scan and block-count limits still apply independently.
///
/// # Errors
/// The errors of [`slope_arcs`], or `FMINE-REF-207` when the next retained arc would exceed `max_arcs`,
/// or target-buffer allocation fails. A limit above the existing `u32::MAX - 1` ceiling does not raise it.
pub fn slope_arcs_bounded(
    grid: &BlockGrid,
    table: &SlopeTable,
    domain_of: &[u16],
    levels: u32,
    max_arcs: usize,
) -> Result<SlopeArcs, Refusal> {
    let max_arcs = max_arcs.min(NONE as usize - 1);
    let n = grid.block_count();
    if levels == 0 {
        return Err(refuse("slope: levels must be at least 1".to_string()));
    }
    if domain_of.len() != n {
        return Err(refuse(format!(
            "slope: {} domain entries for {n} blocks",
            domain_of.len()
        )));
    }
    if let Some((b, &d)) = domain_of
        .iter()
        .enumerate()
        .find(|&(_, &d)| usize::from(d) >= table.domains.len())
    {
        return Err(refuse(format!(
            "slope: block {b} is in domain {d}, but the table has {} domains",
            table.domains.len()
        )));
    }
    let [nx, ny, nz] = grid.n.map(i64::from);
    let levels = i64::from(levels).min(nz - 1);
    let patterns = patterns(grid, table, levels)?;
    let mixed = (table.domains.len() > 1).then(|| MixedCounts::new(grid, domain_of));
    let layer = nx * ny;
    let mut offsets = Vec::with_capacity(n + 1);
    offsets.push(0);
    let mut targets: Vec<u32> = Vec::new();
    let (mut full_cone_arcs, mut boundary_blocks) = (0u64, 0usize);
    for k in 0..nz {
        for j in 0..ny {
            for i in 0..nx {
                let b = grid.at([i, j, k]);
                let d = domain_of[b];
                let p = &patterns[usize::from(d)];
                let [ri, rj] = p.reach;
                // Layers above that exist and are used.
                let up = k.min(levels);
                let interior = i >= ri
                    && i + ri < nx
                    && j >= rj
                    && j + rj < ny
                    && (up == 0
                        || mixed.as_ref().is_none_or(|m| {
                            domain_of[b - layer as usize] == d
                                && m.count([i - ri, j - rj, k - up], [i + ri, j + rj, k - 1]) == 0
                        }));
                if interior {
                    full_cone_arcs += p.full_upto[up as usize] as u64;
                    let to = |o: &Offset| (b as i64 + o[0] + o[1] * nx - o[2] * layer) as u32;
                    let reduced = &p.reduced[..p.reduced_upto[up as usize]];
                    reserve_targets(&mut targets, reduced.len(), max_arcs)?;
                    targets.extend(reduced.iter().map(to));
                } else {
                    boundary_blocks += 1;
                    for (&o, w) in p.full[..p.full_upto[up as usize]].iter().zip(&p.witnesses) {
                        let Some(t) = grid.shifted([i, j, k], o) else {
                            continue;
                        };
                        full_cone_arcs += 1;
                        let implied = w.iter().any(|&q| {
                            let o1 = p.full[q as usize];
                            grid.shifted([i, j, k], o1).is_some_and(|via| {
                                patterns[usize::from(domain_of[via])].holds(sub(o, o1))
                            })
                        });
                        if !implied {
                            reserve_targets(&mut targets, 1, max_arcs)?;
                            targets.push(t as u32);
                        }
                    }
                }
                offsets.push(targets.len());
            }
        }
    }
    Ok(SlopeArcs {
        offsets,
        targets,
        full_cone_arcs,
        boundary_blocks,
    })
}

fn reserve_targets(
    targets: &mut Vec<u32>,
    additional: usize,
    max_arcs: usize,
) -> Result<(), Refusal> {
    let needed = targets
        .len()
        .checked_add(additional)
        .filter(|&n| n <= max_arcs)
        .ok_or_else(|| refuse(format!("slope: retained arc budget {max_arcs} exceeded")))?;
    if needed > targets.capacity() {
        // Geometric growth keeps boundary appends cheap, but Vec's implicit growth could exceed the cap.
        let capacity = targets
            .capacity()
            .saturating_mul(2)
            .max(4)
            .max(needed)
            .min(max_arcs);
        targets
            .try_reserve_exact(capacity - targets.len())
            .map_err(|_| refuse(format!("slope: cannot allocate {capacity} retained arcs")))?;
    }
    Ok(())
}

/// Box counts of mixed blocks: those whose next block east, north or below is in another domain.
struct MixedCounts {
    stride: [usize; 2],
    /// `sum[(k·(ny + 1) + j)·(nx + 1) + i]` counts the mixed blocks below `(i, j, k)` on every axis, wrapping in
    /// `u32`; every true count is less than `u32::MAX`, so differences come out exact.
    sum: Vec<u32>,
}

impl MixedCounts {
    fn new(grid: &BlockGrid, domain_of: &[u16]) -> Self {
        let [nx, ny, nz] = grid.n.map(|c| c as usize);
        let stride = [nx + 1, (nx + 1) * (ny + 1)];
        let mut sum = vec![0u32; stride[1] * (nz + 1)];
        let at = |i: usize, j: usize, k: usize| k * stride[1] + j * stride[0] + i;
        for k in 0..nz {
            for j in 0..ny {
                for i in 0..nx {
                    let b = (k * ny + j) * nx + i;
                    let d = domain_of[b];
                    let mixed = (i + 1 < nx && domain_of[b + 1] != d)
                        || (j + 1 < ny && domain_of[b + nx] != d)
                        || (k + 1 < nz && domain_of[b + nx * ny] != d);
                    sum[at(i + 1, j + 1, k + 1)] = u32::from(mixed)
                        .wrapping_add(sum[at(i, j + 1, k + 1)])
                        .wrapping_add(sum[at(i + 1, j, k + 1)])
                        .wrapping_add(sum[at(i + 1, j + 1, k)])
                        .wrapping_sub(sum[at(i, j, k + 1)])
                        .wrapping_sub(sum[at(i, j + 1, k)])
                        .wrapping_sub(sum[at(i + 1, j, k)])
                        .wrapping_add(sum[at(i, j, k)]);
                }
            }
        }
        Self { stride, sum }
    }

    /// Mixed blocks in the in-grid box from `lo` to `hi`, both included.
    fn count(&self, lo: [i64; 3], hi: [i64; 3]) -> u32 {
        let s = |i: i64, j: i64, k: i64| {
            self.sum[k as usize * self.stride[1] + j as usize * self.stride[0] + i as usize]
        };
        let ([i0, j0, k0], [i1, j1, k1]) = (lo, hi.map(|v| v + 1));
        s(i1, j1, k1)
            .wrapping_sub(s(i0, j1, k1))
            .wrapping_sub(s(i1, j0, k1))
            .wrapping_sub(s(i1, j1, k0))
            .wrapping_add(s(i0, j0, k1))
            .wrapping_add(s(i0, j1, k0))
            .wrapping_add(s(i1, j0, k0))
            .wrapping_sub(s(i0, j0, k0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tan(rise: u32, run: u32) -> SlopeTangent {
        SlopeTangent::new(rise, run).unwrap()
    }

    /// The arcs of block `(10, 10, levels + 1)` on a 21 × 21 grid, as sorted offsets `(Δi, Δj, Δk)`.
    fn interior_pattern(table: &SlopeTable, size: [u32; 3], levels: u32) -> Vec<Offset> {
        let grid = BlockGrid::new([21, 21, levels + 2], size).unwrap();
        let arcs = slope_arcs(&grid, table, &vec![0; grid.block_count()], levels).unwrap();
        let [i, j, k] = [10i64, 10, i64::from(levels) + 1];
        let b = grid.at([i, j, k]);
        let mut out: Vec<Offset> = arcs.targets()[arcs.offsets()[b]..arcs.offsets()[b + 1]]
            .iter()
            .map(|&t| {
                let t = i64::from(t);
                [t % 21 - i, (t / 21) % 21 - j, k - t / (21 * 21)]
            })
            .collect();
        out.sort_unstable();
        out
    }

    /// 45° on cubic blocks over one layer: the block above and its four edge neighbours, a plus. The corner centres
    /// are √2 block widths away, beyond the slope.
    #[test]
    fn forty_five_degrees_one_layer_is_a_plus() {
        let p = interior_pattern(&SlopeTable::uniform(tan(1, 1)), [10, 10, 10], 1);
        assert_eq!(
            p,
            vec![[-1, 0, 1], [0, -1, 1], [0, 0, 1], [0, 1, 1], [1, 0, 1]]
        );
    }

    /// 60° (tan 1.732) on 10 m cubes over three layers.
    /// - Layer 1 reaches 5.8 m: only the block above.
    /// - Layer 2 reaches 11.5 m: the four edge neighbours, at 10 m.
    /// - Layer 3 reaches 17.3 m: the four corners, at 14.1 m.
    ///
    /// Everything else is implied.
    #[test]
    fn sixty_degrees_three_layers_known_answer() {
        let p = interior_pattern(&SlopeTable::uniform(tan(1732, 1000)), [10, 10, 10], 3);
        let mut want = vec![[0, 0, 1]];
        want.extend([[-1, 0, 2], [0, -1, 2], [0, 1, 2], [1, 0, 2]]);
        want.extend([[-1, -1, 3], [-1, 1, 3], [1, -1, 3], [1, 1, 3]]);
        want.sort_unstable();
        assert_eq!(p, want);
    }

    /// A flat block, 20 × 20 × 10, at 45° over one layer requires only the block above.
    #[test]
    fn block_shape_matters() {
        let p = interior_pattern(&SlopeTable::uniform(tan(1, 1)), [20, 20, 10], 1);
        assert_eq!(p, vec![[0, 0, 1]]);
    }

    /// Two sectors: 45° from east counterclockwise to west, then nearly vertical from west round to east. North and
    /// east are in the first; west is on the second's boundary, so in it, and south is inside it.
    #[test]
    fn sectors_pick_the_slope_by_direction() {
        let table = SlopeTable::new(vec![vec![
            Sector::new(1, 0, tan(1, 1)).unwrap(),
            Sector::new(-1, 0, tan(100, 1)).unwrap(),
        ]])
        .unwrap();
        let p = interior_pattern(&table, [10, 10, 10], 1);
        assert_eq!(p, vec![[0, 0, 1], [0, 1, 1], [1, 0, 1]]);
    }

    /// A grid one layer deep has no arcs, and levels beyond the grid change nothing.
    #[test]
    fn levels_past_the_grid_are_not_used() {
        let one = SlopeTable::uniform(tan(1, 1));
        let flat = BlockGrid::new([4, 4, 1], [10, 10, 10]).unwrap();
        assert_eq!(slope_arcs(&flat, &one, &[0; 16], 5).unwrap().arc_count(), 0);
        let grid = BlockGrid::new([6, 6, 4], [10, 10, 10]).unwrap();
        let domains = vec![0; grid.block_count()];
        assert_eq!(
            slope_arcs(&grid, &one, &domains, 3).unwrap(),
            slope_arcs(&grid, &one, &domains, 300).unwrap()
        );
    }

    #[test]
    fn retained_arc_budget_checks_boundary_and_interior_before_growth() {
        let table = SlopeTable::uniform(tan(1, 1));
        let grid = BlockGrid::new([3, 3, 2], [1, 1, 1]).unwrap();
        // Nine straight-up arcs, twelve east/west and twelve north/south arcs.
        let exact = slope_arcs_bounded(&grid, &table, &[0; 18], 1, 33).unwrap();
        assert_eq!(exact.arc_count(), 33);
        assert_eq!(exact, slope_arcs(&grid, &table, &[0; 18], 1).unwrap());
        for cap in [0, 2, 18, 32] {
            // Cap 2 stops inside the first corner's three arcs. Cap 18 stops before the central
            // interior block's five-arc bulk append, after fourteen boundary arcs.
            let refusal = slope_arcs_bounded(&grid, &table, &[0; 18], 1, cap).unwrap_err();
            assert_eq!(refusal.code, "FMINE-REF-207");
            assert!(
                refusal
                    .what
                    .contains(&format!("retained arc budget {cap} exceeded"))
            );
        }
        let flat = BlockGrid::new([3, 3, 1], [1, 1, 1]).unwrap();
        assert_eq!(
            slope_arcs_bounded(&flat, &table, &[0; 9], 1, 0)
                .unwrap()
                .arc_count(),
            0
        );
        let mut targets = vec![0, 1];
        let capacity = targets.capacity();
        assert!(reserve_targets(&mut targets, 2, 3).is_err());
        assert_eq!(targets, [0, 1]);
        assert_eq!(targets.capacity(), capacity);
        assert!(reserve_targets(&mut targets, usize::MAX, usize::MAX).is_err());
        assert_eq!(targets.capacity(), capacity);
    }

    #[test]
    fn bad_input_is_refused() {
        fn code<T>(r: Result<T, Refusal>) -> Option<&'static str> {
            r.err().map(|e| e.code)
        }
        let r207 = Some("FMINE-REF-207");
        assert_eq!(code(BlockGrid::new([0, 1, 1], [1, 1, 1])), r207);
        assert_eq!(code(BlockGrid::new([1, 1, 1], [1, 0, 1])), r207);
        assert_eq!(code(BlockGrid::new([65_536, 65_536, 1], [1, 1, 1])), r207);
        assert_eq!(code(SlopeTangent::new(0, 1)), r207);
        assert_eq!(code(SlopeTangent::new(1, 0)), r207);
        assert_eq!(code(Sector::new(0, 0, tan(1, 1))), r207);
        assert_eq!(code(SlopeTable::new(vec![])), r207);
        assert_eq!(code(SlopeTable::new(vec![vec![]])), r207);
        let s = |e, n| Sector::new(e, n, tan(1, 1)).unwrap();
        // South then north goes clockwise; a boundary may not repeat; a turn of 270° is fine.
        assert_eq!(
            code(SlopeTable::new(vec![vec![s(1, 0), s(0, -1), s(0, 1)]])),
            r207
        );
        assert_eq!(
            code(SlopeTable::new(vec![vec![s(1, 0), s(0, 1), s(0, 2)]])),
            r207
        );
        assert!(SlopeTable::new(vec![vec![s(1, 0), s(0, -1)]]).is_ok());
        assert!(SlopeTable::new(vec![vec![s(1, 0), s(0, 1), s(-1, 0), s(0, -1)]]).is_ok());
        let grid = BlockGrid::new([3, 3, 3], [10, 10, 10]).unwrap();
        let one = SlopeTable::uniform(tan(1, 1));
        assert_eq!(code(slope_arcs(&grid, &one, &[0; 27], 0)), r207);
        assert_eq!(code(slope_arcs(&grid, &one, &[0; 26], 1)), r207);
        let mut domains = [0u16; 27];
        domains[5] = 1;
        assert_eq!(code(slope_arcs(&grid, &one, &domains, 1)), r207);
        // A gentle slope over many layers: more than MAX_PATTERN offsets, in a box below MAX_SCAN.
        let wide_grid = BlockGrid::new([400, 400, 60], [1, 1, 10]).unwrap();
        let gentle = SlopeTable::uniform(tan(1, 20));
        assert_eq!(code(patterns(&wide_grid, &gentle, 50)), r207);
        // One flat sliver among steep sectors holds few offsets, but its box is too big to scan.
        let sliver = SlopeTable::new(vec![vec![
            Sector::new(1, 0, tan(1, 1000)).unwrap(),
            Sector::new(1000, 1, tan(1000, 1)).unwrap(),
        ]])
        .unwrap();
        let huge = BlockGrid::new([60_000, 60_000, 1], [1, 1, 1000]).unwrap();
        assert_eq!(code(patterns(&huge, &sliver, 1)), r207);
        // Sizes near u32::MAX overflow the exact test.
        let big = BlockGrid::new([3, 3, 3], [u32::MAX, u32::MAX, u32::MAX]).unwrap();
        let steep = SlopeTable::uniform(tan(u32::MAX, 1));
        assert_eq!(code(inside(&big, &steep, 0, [2, 2, 2])), r207);
    }
}
