//! Provenance: frankensim crates/fs-rep-sdf/src/vdb.rs @ 20463525d9775a18c1b237d2676f364f367175ab; licence LicenseRef-MIT-OpenAI-Anthropic-Rider; Changes: unsplit's expect calls become masked casts; dilate refuses (FMINE-REF-109) instead of overflowing when an active voxel sits on the i32 index boundary, and erode treats an out-of-range neighbour as inactive; a test module is added (upstream's conformance oracle check rsdf-004, a golden iter_active digest, boundary tests); Left out: nothing in this file (fs-rep-sdf's SDF, narrow-band and level-set code that sits on the grid is not lifted)
//! Determinism: BitwisePortable (integer index arithmetic; the one float operation is the `bytes_per_active`
//! division in `memory_stats`).
//!
//! FrankenVDB: the sparse hierarchical tile tree — root map → 32³ internal nodes → 8³ bitmasked leaves. In
//! FrankenMining it is the topology and index layer under block models, stockpile voxels and belief tiles (plan
//! §27, `BD-191`); columnar attribute chunks, rotated grid frames and sub-blocking are later slices.
//!
//! Determinism discipline: the root and internal children are BTreeMaps
//! (sorted iteration — HashMap iteration order is banned from results),
//! and in-leaf order is linear index order, so `iter_active` is a total
//! deterministic order.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use fmine_evidence::{Refusal, RefusalCode};

const LEAF_LOG2: i32 = 3;
const LEAF_DIM: i32 = 1 << LEAF_LOG2; // 8
const LEAF_VOLUME: usize = 512;
const INTERNAL_LOG2: i32 = 5;
const INTERNAL_DIM: i32 = 1 << INTERNAL_LOG2; // 32

/// The six face directions, in the fixed order `dilate` and `erode` use.
const DIRS: [[i32; 3]; 6] = [
    [-1, 0, 0],
    [1, 0, 0],
    [0, -1, 0],
    [0, 1, 0],
    [0, 0, -1],
    [0, 0, 1],
];

/// An 8³ bitmasked leaf: full value array + active mask.
struct Leaf<T> {
    mask: [u64; LEAF_VOLUME / 64],
    values: [T; LEAF_VOLUME],
}

impl<T: Copy> Leaf<T> {
    fn new(background: T) -> Self {
        Leaf {
            mask: [0; LEAF_VOLUME / 64],
            values: [background; LEAF_VOLUME],
        }
    }

    fn bit(idx: usize) -> (usize, u64) {
        (idx / 64, 1u64 << (idx % 64))
    }

    fn is_active(&self, idx: usize) -> bool {
        let (w, b) = Self::bit(idx);
        self.mask[w] & b != 0
    }

    fn set(&mut self, idx: usize, v: T) {
        let (w, b) = Self::bit(idx);
        self.mask[w] |= b;
        self.values[idx] = v;
    }

    fn deactivate(&mut self, idx: usize) {
        let (w, b) = Self::bit(idx);
        self.mask[w] &= !b;
    }

    fn active_count(&self) -> u64 {
        self.mask.iter().map(|w| u64::from(w.count_ones())).sum()
    }
}

/// One 32³ internal node: sparse children keyed by packed in-node index
/// (BTreeMap keeps memory proportional to occupancy AND iteration
/// deterministic).
struct InternalNode<T> {
    children: BTreeMap<u32, Box<Leaf<T>>>,
}

impl<T> InternalNode<T> {
    fn new() -> Self {
        InternalNode {
            children: BTreeMap::new(),
        }
    }
}

/// The sparse grid over `i32` voxel indices. `T` is the voxel payload.
pub struct VdbGrid<T: Copy> {
    background: T,
    root: BTreeMap<[i32; 3], InternalNode<T>>,
}

/// Memory/topology statistics.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VdbStats {
    /// Active voxels.
    pub active: u64,
    /// Allocated leaves.
    pub leaves: u64,
    /// Allocated internal nodes.
    pub internals: u64,
    /// Resident payload bytes (leaves + masks + node maps, estimated from
    /// layout — not an allocator measurement).
    pub resident_bytes: u64,
    /// Resident bytes per active voxel.
    pub bytes_per_active: f64,
}

impl VdbStats {
    /// Canonical JSON.
    #[must_use]
    pub fn to_json(&self) -> String {
        let mut s = String::with_capacity(96);
        let _ = write!(
            s,
            "{{\"active\":{},\"leaves\":{},\"internals\":{},\"resident_bytes\":{},\
             \"bytes_per_active\":{:.2}}}",
            self.active, self.leaves, self.internals, self.resident_bytes, self.bytes_per_active
        );
        s
    }
}

/// Split a global voxel coordinate into (root key, packed internal index,
/// packed leaf index). Arithmetic shifts give floor semantics for
/// negatives; low-bit masks are non-negative in two's complement.
fn split(coord: [i32; 3]) -> ([i32; 3], u32, usize) {
    let leaf = [
        coord[0] >> LEAF_LOG2,
        coord[1] >> LEAF_LOG2,
        coord[2] >> LEAF_LOG2,
    ];
    let root = [
        leaf[0] >> INTERNAL_LOG2,
        leaf[1] >> INTERNAL_LOG2,
        leaf[2] >> INTERNAL_LOG2,
    ];
    let li = [
        (leaf[0] & (INTERNAL_DIM - 1)) as u32,
        (leaf[1] & (INTERNAL_DIM - 1)) as u32,
        (leaf[2] & (INTERNAL_DIM - 1)) as u32,
    ];
    let internal_idx = (li[2] << (2 * INTERNAL_LOG2)) | (li[1] << INTERNAL_LOG2) | li[0];
    let vi = [
        (coord[0] & (LEAF_DIM - 1)) as usize,
        (coord[1] & (LEAF_DIM - 1)) as usize,
        (coord[2] & (LEAF_DIM - 1)) as usize,
    ];
    let leaf_idx = (vi[2] << (2 * LEAF_LOG2 as usize)) | (vi[1] << (LEAF_LOG2 as usize)) | vi[0];
    (root, internal_idx, leaf_idx)
}

/// Inverse of [`split`]'s leaf-local part: rebuild the global coordinate.
fn unsplit(root: [i32; 3], internal_idx: u32, leaf_idx: usize) -> [i32; 3] {
    // Masked values are < 32 / < 8, so the casts cannot wrap.
    let li = [
        (internal_idx & (INTERNAL_DIM as u32 - 1)) as i32,
        ((internal_idx >> INTERNAL_LOG2) & (INTERNAL_DIM as u32 - 1)) as i32,
        ((internal_idx >> (2 * INTERNAL_LOG2)) & (INTERNAL_DIM as u32 - 1)) as i32,
    ];
    let vi = [
        (leaf_idx & (LEAF_DIM as usize - 1)) as i32,
        ((leaf_idx >> LEAF_LOG2) & (LEAF_DIM as usize - 1)) as i32,
        ((leaf_idx >> (2 * LEAF_LOG2)) & (LEAF_DIM as usize - 1)) as i32,
    ];
    [
        ((root[0] << INTERNAL_LOG2) + li[0]) * LEAF_DIM + vi[0],
        ((root[1] << INTERNAL_LOG2) + li[1]) * LEAF_DIM + vi[1],
        ((root[2] << INTERNAL_LOG2) + li[2]) * LEAF_DIM + vi[2],
    ]
}

/// `c + d`, or `None` when it leaves the `i32` index range.
fn neighbour(c: [i32; 3], d: [i32; 3]) -> Option<[i32; 3]> {
    Some([
        c[0].checked_add(d[0])?,
        c[1].checked_add(d[1])?,
        c[2].checked_add(d[2])?,
    ])
}

impl<T: Copy> VdbGrid<T> {
    /// Empty grid returning `background` everywhere.
    #[must_use]
    pub fn new(background: T) -> Self {
        VdbGrid {
            background,
            root: BTreeMap::new(),
        }
    }

    /// The background value.
    #[must_use]
    pub fn background(&self) -> T {
        self.background
    }

    /// Activate a voxel with a value (O(log root) + O(log node)).
    pub fn set(&mut self, coord: [i32; 3], value: T) {
        let (root, ii, li) = split(coord);
        self.root
            .entry(root)
            .or_insert_with(InternalNode::new)
            .children
            .entry(ii)
            .or_insert_with(|| Box::new(Leaf::new(self.background)))
            .set(li, value);
    }

    /// Read a voxel (background when inactive).
    #[must_use]
    pub fn get(&self, coord: [i32; 3]) -> T {
        let (root, ii, li) = split(coord);
        self.root
            .get(&root)
            .and_then(|n| n.children.get(&ii))
            .and_then(|leaf| leaf.is_active(li).then(|| leaf.values[li]))
            .unwrap_or(self.background)
    }

    /// True when the voxel is active.
    #[must_use]
    pub fn is_active(&self, coord: [i32; 3]) -> bool {
        let (root, ii, li) = split(coord);
        self.root
            .get(&root)
            .and_then(|n| n.children.get(&ii))
            .is_some_and(|leaf| leaf.is_active(li))
    }

    /// Deactivate a voxel (value reverts to background on read).
    pub fn deactivate(&mut self, coord: [i32; 3]) {
        let (root, ii, li) = split(coord);
        if let Some(node) = self.root.get_mut(&root)
            && let Some(leaf) = node.children.get_mut(&ii)
        {
            leaf.deactivate(li);
        }
    }

    /// Active voxel count.
    #[must_use]
    pub fn active_count(&self) -> u64 {
        self.root
            .values()
            .flat_map(|n| n.children.values())
            .map(|l| l.active_count())
            .sum()
    }

    /// Deterministic iteration over active voxels (root order → internal
    /// order → leaf linear order).
    pub fn iter_active(&self) -> impl Iterator<Item = ([i32; 3], T)> + '_ {
        self.root.iter().flat_map(|(rk, node)| {
            node.children.iter().flat_map(move |(ii, leaf)| {
                (0..LEAF_VOLUME)
                    .filter(|&li| leaf.is_active(li))
                    .map(move |li| (unsplit(*rk, *ii, li), leaf.values[li]))
            })
        })
    }

    /// Dilate the active set by one voxel across the 6 face directions.
    /// New voxels take the activating neighbor's value; already-active
    /// voxels keep theirs. Deterministic (iteration order + fixed
    /// direction order).
    ///
    /// Refuses `FMINE-REF-109`, leaving the grid unchanged, when an active voxel lies on the `i32` index boundary,
    /// where a face neighbour has no index.
    pub fn dilate(&mut self) -> Result<(), Refusal> {
        let snapshot: Vec<([i32; 3], T)> = self.iter_active().collect();
        if let Some((c, _)) = snapshot
            .iter()
            .find(|(c, _)| DIRS.iter().any(|&d| neighbour(*c, d).is_none()))
        {
            return Err(Refusal::new(
                RefusalCode::OutsidePhysicalDomain109,
                format!(
                    "dilate: active voxel {c:?} has a face neighbour outside the i32 index range"
                ),
            ));
        }
        for (c, v) in snapshot {
            for d in DIRS {
                if let Some(n) = neighbour(c, d)
                    && !self.is_active(n)
                {
                    self.set(n, v);
                }
            }
        }
        Ok(())
    }

    /// Erode the active set by one voxel: deactivate actives with any
    /// inactive face neighbor. A neighbour outside the `i32` index range
    /// cannot be active, so it counts as inactive. Deterministic.
    pub fn erode(&mut self) {
        let doomed: Vec<[i32; 3]> = self
            .iter_active()
            .filter(|&(c, _)| {
                DIRS.iter()
                    .any(|&d| neighbour(c, d).is_none_or(|n| !self.is_active(n)))
            })
            .map(|(c, _)| c)
            .collect();
        for c in doomed {
            self.deactivate(c);
        }
    }

    /// Layout-derived memory statistics.
    #[must_use]
    pub fn memory_stats(&self) -> VdbStats {
        let leaves: u64 = self.root.values().map(|n| n.children.len() as u64).sum();
        let internals = self.root.len() as u64;
        let leaf_bytes = (size_of::<Leaf<T>>() + size_of::<u32>() + size_of::<usize>()) as u64;
        let resident = leaves * leaf_bytes + internals * 64;
        let active = self.active_count();
        VdbStats {
            active,
            leaves,
            internals,
            resident_bytes: resident,
            bytes_per_active: resident as f64 / active.max(1) as f64,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fmine_core::hash::Blake3;

    /// The LCG of upstream's conformance suite (`below`, `unit`).
    struct Lcg(u64);

    impl Lcg {
        fn next(&mut self) -> u64 {
            self.0 = self
                .0
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            self.0 >> 11
        }
        fn below(&mut self, n: u64) -> u64 {
            self.next() % n
        }
        fn unit(&mut self) -> f64 {
            self.next() as f64 / (1u64 << 53) as f64
        }
    }

    /// BLAKE3 over `iter_active` as (x, y, z, value bits), little-endian, checking the order is strictly
    /// increasing in (root key, internal index, leaf index) on the way.
    fn iter_digest(grid: &VdbGrid<f64>) -> (u64, String) {
        let mut h = Blake3::new();
        let mut n = 0u64;
        let mut prev: Option<([i32; 3], u32, usize)> = None;
        for (c, v) in grid.iter_active() {
            let key = split(c);
            if let Some(p) = prev {
                assert!(
                    p < key,
                    "iter_active order is not strictly increasing at {c:?}"
                );
            }
            prev = Some(key);
            for x in c {
                h.update(&x.to_le_bytes());
            }
            h.update(&v.to_bits().to_le_bytes());
            n += 1;
        }
        (n, h.finalize().to_hex())
    }

    /// Upstream's rsdf-004 (fs-rep-sdf tests/conformance.rs): clustered and scattered actives against a BTreeMap
    /// oracle, then dilate and erode.
    #[test]
    fn rsdf_004_vdb_matches_the_oracle() {
        const SEED: u64 = 0x5DF4_2026_0706_0BDB;
        let mut rng = Lcg(SEED);
        let mut vdb: VdbGrid<f32> = VdbGrid::new(-1.0);
        let mut oracle: BTreeMap<[i32; 3], f32> = BTreeMap::new();
        // Clustered actives (three blobs) + scattered strays, interleaved
        // reads — the sparse-use shape.
        for blob in 0..3i32 {
            let base = [blob * 40 - 30, blob * 25 - 20, blob * 15 - 10];
            for _ in 0..4_000 {
                let c = [
                    base[0] + (rng.below(16) as i32) - 8,
                    base[1] + (rng.below(16) as i32) - 8,
                    base[2] + (rng.below(16) as i32) - 8,
                ];
                let v = (rng.unit() * 4.0 - 2.0) as f32;
                vdb.set(c, v);
                oracle.insert(c, v);
            }
        }
        for _ in 0..500 {
            let c = [
                (rng.below(4_000) as i32) - 2_000,
                (rng.below(4_000) as i32) - 2_000,
                (rng.below(4_000) as i32) - 2_000,
            ];
            let v = rng.unit() as f32;
            vdb.set(c, v);
            oracle.insert(c, v);
        }
        // Exact agreement: actives, values, iteration set, misses.
        assert_eq!(vdb.active_count(), oracle.len() as u64);
        for (&c, &v) in &oracle {
            assert!(
                vdb.is_active(c) && vdb.get(c).to_bits() == v.to_bits(),
                "{c:?}"
            );
        }
        let iterated: BTreeMap<[i32; 3], f32> = vdb.iter_active().collect();
        assert!(
            iterated == oracle,
            "iter_active set differs from the oracle"
        );
        for _ in 0..2_000 {
            let c = [
                (rng.below(8_000) as i32) - 4_000,
                (rng.below(8_000) as i32) - 4_000,
                (rng.below(8_000) as i32) - 4_000,
            ];
            if !oracle.contains_key(&c) {
                assert!(
                    !vdb.is_active(c) && vdb.get(c).to_bits() == (-1.0f32).to_bits(),
                    "{c:?}"
                );
            }
        }
        // Dilate grows, erode shrinks back below the dilated count.
        let before = vdb.active_count();
        vdb.dilate().expect("in-range dilate");
        let dilated = vdb.active_count();
        vdb.erode();
        let eroded = vdb.active_count();
        assert!(
            dilated > before && eroded < dilated,
            "{before} {dilated} {eroded}"
        );
        let stats = vdb.memory_stats();
        assert_eq!(stats.active, eroded);
        println!(
            "{{\"suite\":\"fmine-geom/vdb\",\"case\":\"rsdf-004\",\"stats\":{}}}",
            stats.to_json()
        );
    }

    /// The split/unsplit round trip, negative and boundary coordinates included.
    #[test]
    fn split_unsplit_round_trips_across_signs_and_extremes() {
        let mut rng = Lcg(0x5917_0000_0000_0001);
        let mut coords = vec![
            [0, 0, 0],
            [-1, -1, -1],
            [7, 8, -9],
            [255, 256, -257],
            [i32::MAX, i32::MIN, 0],
            [i32::MIN, i32::MAX, -1],
        ];
        for _ in 0..10_000 {
            coords.push([
                rng.next() as u32 as i32,
                rng.next() as u32 as i32,
                rng.next() as u32 as i32,
            ]);
        }
        for c in coords {
            let (root, ii, li) = split(c);
            assert_eq!(unsplit(root, ii, li), c);
        }
    }

    /// Golden `iter_active` order digest: a fixed grid of 3,037 voxels in [−300, 300)³, so root keys −2 to 1 on
    /// every axis, negative coordinates included. A change to iteration order, coordinate packing or value storage
    /// changes it. Pinned from the first run (2026-11-01); it guards against change and is not a correctness oracle,
    /// which `rsdf_004_vdb_matches_the_oracle` is.
    #[test]
    fn golden_iter_active_digest() {
        let mut grid: VdbGrid<f64> = VdbGrid::new(f64::NAN);
        let mut rng = Lcg(0x60_1DE7_D1E5_7000);
        for _ in 0..3_000 {
            let c = [
                (rng.below(600) as i32) - 300,
                (rng.below(600) as i32) - 300,
                (rng.below(600) as i32) - 300,
            ];
            grid.set(c, rng.unit());
        }
        for z in 0..37 {
            grid.set([0, 0, z], f64::from(z));
        }
        let (n, digest) = iter_digest(&grid);
        assert_eq!(n, grid.active_count());
        assert_eq!(
            (n, digest.as_str()),
            (
                3037,
                "8bec205063e31148924a926588be6d1923faddfaa06b6ee127fedfea01e70ae4"
            ),
            "golden iter_active digest"
        );
    }

    #[test]
    fn dilate_refuses_at_the_index_boundary_and_leaves_the_grid_unchanged() {
        let mut grid: VdbGrid<u8> = VdbGrid::new(0);
        grid.set([i32::MAX, 0, 0], 1);
        grid.set([0, 0, 0], 2);
        let refusal = grid.dilate().expect_err("boundary voxel");
        assert_eq!(refusal.code, "FMINE-REF-109", "{refusal:?}");
        assert_eq!(grid.active_count(), 2, "a refused dilate changes nothing");
        // Erode treats the missing neighbour as inactive.
        grid.erode();
        assert_eq!(grid.active_count(), 0);
    }

    #[test]
    fn get_set_deactivate_and_background() {
        let mut grid: VdbGrid<i64> = VdbGrid::new(-7);
        assert_eq!(grid.get([5, 5, 5]), -7);
        grid.set([5, 5, 5], 42);
        grid.set([-5, 5, -5], 43);
        assert!(grid.is_active([5, 5, 5]));
        assert_eq!(grid.get([5, 5, 5]), 42);
        grid.deactivate([5, 5, 5]);
        assert!(!grid.is_active([5, 5, 5]));
        assert_eq!(
            grid.get([5, 5, 5]),
            -7,
            "deactivated voxels read the background"
        );
        assert_eq!(grid.active_count(), 1);
        assert_eq!(grid.background(), -7);
    }
}
