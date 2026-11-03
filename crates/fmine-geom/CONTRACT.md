# CONTRACT.md — `fmine-geom`

## 1. Purpose
The Strata geometry crate. Three slices so far, all lifted from `frankensim` (`registries/lifts.toml`):
- `exact` (`BD-193`): `orient2d`, `orient3d`, `incircle` and `insphere` return the exact sign of their determinant, and `orient2d_sos` and `orient3d_sos` break exact ties by Simulation of Simplicity (from fs-ivl and fs-math);
- `vdb` (`BD-191`): `VdbGrid<T>`, a sparse root map → 32³ internal node → 8³ bitmasked leaf tree over `i32` voxel indices, with a deterministic `iter_active` order (from fs-rep-sdf);
- `winding` (`BD-192`): generalized winding numbers over a triangle `Soup`, exact (`winding_exact`, a sum of solid angles) or through the dipole `WindingOctree`, with fmine-core's `det::atan2` in place of std's (from fs-rep-mesh). A point is inside when the number exceeds ½, which stays right on soups with holes, flipped patches and duplicates.

A fourth slice is built here on all three, not lifted:
- `volcert` (bd-wq34; plan §13.9, §13.11 `VolumeEnclosure`, PO-STRATA-8): `ClosedMesh::certify` refuses a mesh that is not closed and consistently oriented; `volume` is the divergence-theorem sum computed exactly and rounded outward; `block_proportions` gives each block of an axis-aligned `BlockGrid` the number of its `8^depth` sub-cells wholly inside the mesh and the number whose interior the surface meets, which bound the proportion inside from both sides. A surface lying on sub-cell faces, as a pit shell built from blocks does, cuts no sub-cell, so its proportions are exact. This is how a block model takes its domain coding from a wireframe.

Outside the crate only `fmine-io`'s `omf::proportions_in_surface` (bd-aqfl) calls in, through `volcert`. DXF ingest, rotated grid frames, sub-blocks and columnar attribute chunks are later slices.

- **Crate:** `fmine-geom`
- **Layer:** K0
- **Subsystem:** Strata
- **LOC budget:** 30,000 planned / 35,000 ceiling
- **Allowed Module Families:** `["exact", "vdb", "winding", "volcert"]`

## 2. Inputs
- `exact`: points as `[f64; 2]` or `[f64; 3]` coordinate arrays in one caller-chosen Cartesian frame and unit; the predicates are unit-free, since only signs come out. The SoS forms also take one `u64` index per point, which must be pairwise distinct.
- `vdb`: `[i32; 3]` voxel indices and a `Copy` payload `T`, with a background value returned for inactive voxels. The grid has no frame, cell size or unit; mapping world coordinates to indices is the caller's.
- `winding`: a `Soup` of `[f64; 3]` positions and `[u32; 3]` corner-index triangles, with no connectivity, orientation or closedness assumed; query points as `[f64; 3]` in the same frame; and the octree's `beta` (at least 1, 2 by the paper). The number is unit-free.
- `volcert`: a mesh as `[f64; 3]` positions and `[u32; 3]` triangles, which must form closed, outward-facing shells once positions with identical bits are welded; a `BlockGrid` of origin, positive block size and block counts per axis, axis-aligned, in the mesh's frame and unit; and a depth from 0 to `MAX_DEPTH` (6).

## 3. Outputs
- `Sign` (`Negative`, `Zero`, `Positive`): the exact sign of the determinant. `Zero` means exactly degenerate (collinear, coplanar, cocircular or cospherical).
- `Stage` (`Filtered`, `Adaptive`, `Exact`), from the `*_with_stage` forms: which rung of the ladder decided the sign.
- `VdbGrid` reads (`get`, `is_active`, `active_count`, `iter_active`) and `VdbStats`, a layout-derived memory estimate (`memory_stats`, `to_json`).
- `winding`: an `f64` generalized winding number (`triangle_winding` per triangle in `[−½, ½]`, `winding_exact`, `WindingOctree::winding`) and the `inside` classification (`w > ½`).
- `volcert`: a certified `ClosedMesh` (welded `Soup`, bounding box); a `VolumeEnclosure` `[lo, hi]` in the cube of the coordinate unit; and `Proportions`, a `VdbGrid<BlockProportion>` over block indices where each stored block carries `inside` and `band` sub-cell counts (an absent block is wholly outside), with `bounds` giving `[inside, inside + band] / 8^depth` and `ProportionStats` counting near blocks, crossed blocks and winding queries.

None is a Claim.

## 4. Invariants
- Each sign equals the sign of the real-arithmetic determinant of the given floats. Tests: `lattice_battery_matches_i128_oracle` and `lattice_battery_3d_matches_i128_oracle` (6,000 random small-lattice cases against an `i128` oracle), `kettner_class_breakdown_naive_float_disagrees_somewhere`, and the families in `tests/predicate_corpus.rs`.
- Swapping two arguments flips the sign: the Kettner test and every corpus family check it.
- The SoS forms never return `Zero` for distinct indices, are antisymmetric under all 24 permutations, and agree with `orient3d` on points moved a small distance along the same moment curve: `sos_is_total_antisymmetric_and_consistent`, `orient3d_sos_full_em_ladder_antisymmetric_total_consistent`.
- The expansion arithmetic is error-free: `(a + b) − a − b`, scale-versus-product and distributivity residuals are the empty expansion (`sum_scale_product_satisfy_exact_residual_laws`).
- General-position inputs resolve at stage A more than 99% of the time (`filter_rates_general_position_resolves_fast`).
- `VdbGrid` agrees exactly with a `BTreeMap` oracle on actives, values, the iterated set and misses (`rsdf_004_vdb_matches_the_oracle`, upstream's rsdf-004). Index splitting round-trips for every `i32` coordinate tested, extremes included (`split_unsplit_round_trips_across_signs_and_extremes`).
- `iter_active` is strictly increasing in (root key, internal index, leaf index) and does not depend on insertion order. Tests: `golden_iter_active_digest` (pinned digest) and `tests/gate_bm_01.rs`, which builds 10⁸ cells in two opposite orders and compares digests.
- The lifted winding answers equal upstream's own to within 1e-13 at every one of 1,264 corpus queries, exactly and at beta 1, 2 and 4; the measured worst is 4.4e-16, 3,137 of the 5,056 values have identical bits, and `inside` agrees wherever upstream is not within 1e-13 of ½ (`differential_corpus_matches_upstream`). Upstream run on an arm64 Mac differs from its x86_64 answers in 107 values; the lift is within the same bound of both (`upstream_answers_differ_by_host_and_the_lift_matches_both`).
- A closed outward soup winds 1 inside and 0 outside, a flipped one −1, two overlapping cubes 2, and translating and scaling soup and point together leaves the number unchanged (`closed_soups_wind_by_their_orientation_and_overlap`). On upstream's nightmare soup the beta-2 octree classifies more than 99% of 2,000 points off the surface correctly (`rmesh_003_winding_classifies_nightmare_soup`, measured 2,000), and off the surface shell of a 1,280-triangle icosphere it stays within 0.05 of the exact number (`rmesh_004_dipole_approximation_tracks_exact_within_declared_error`, measured 0.024).
- `volcert`, all in `src/volcert.rs`:
  - Box wireframes get exactly the sub-cell counts an independent coordinate-comparison oracle predicts, block by block, on a 4 × 5 × 3 grid of unequal block sides at depths 0 to 3: faces off the lattice, a hollow (an inward box inside an outward one) and a box running past the grid on three sides; and, with faces on sub-cell faces, four cases that come out exact, with no band and `inside` times the sub-cell volume equal to the volume: one box, one flush with both of the grid's x faces, two shells touching across a face, and an aligned hollow (`box_wireframes_match_the_analytic_sub_cell_counts`). An octahedron whose slanted faces pass through lattice corners and edges matches an L1-distance oracle block by block at depths 0 to 3, with more than 100 sub-cells touching a face only at a corner, edge or face of their own, none of them band (`an_octahedron_through_lattice_corners_matches_its_l1_oracle`). A grid wholly inside a mesh is all `inside`, one away from it stores nothing (`grids_wholly_inside_or_outside_the_mesh`).
  - The volume enclosure contains the exact volume of a box (6.125), a tetrahedron (1) and an octahedron (8), at most 16 ε wide relative to the volume; it stays as tight with the box moved to UTM-sized coordinates, where a plain `f64` divergence sum is off by 2.3e-10 (`volumes_enclose_exact_analytic_values_even_far_from_the_origin`).
  - Over a grid covering a 512-triangle sphere and a 1,024-triangle torus, `Σ inside` and `Σ (inside + band)` times the sub-cell volume bracket the volume enclosure at every depth, and the band shrinks by at least a quarter per depth step, asserted at every step on the sphere and at the last on the torus (measured sphere band volume 4.45, 2.24, 1.12, 0.56 at depths 0 to 3); the block on the torus's axis is empty (`sums_bracket_the_exact_volume_and_the_band_thins_with_depth`).
  - A grid clipped to cut through the sphere on four more faces than the full grid gives each of its 288 blocks the proportion the full grid gives it (`a_clipped_grid_labels_its_blocks_as_the_full_grid_does`).
  - Welding joins positions with identical bits, `−0.0` with `0.0` (`welding_joins_identical_positions_including_signed_zero`).
  - Planted bugs, each caught and reverted: a separating line or plane not allowed to touch the cell (the octahedron), dropping the far-vertex condition from the separation test (six tests), treating every shared face as free (the aligned boxes), joining the wrong cell across a block face (four tests), dropping one triangle from the volume sum (the volume oracle and the digest).

## 5. Refusals
Every public predicate returns `Result<_, Refusal>`; none panics on bad input.
- `FMINE-REF-109` (proposed): a non-finite coordinate, or a repeated SoS index. Tests: `non_finite_coordinates_are_refused_109_not_signed`, `repeated_sos_indices_are_refused_109`.
- `FMINE-REF-109` (proposed): `VdbGrid::dilate` when an active voxel lies on the `i32` index boundary; the grid is left unchanged (`dilate_refuses_at_the_index_boundary_and_leaves_the_grid_unchanged`).
- `FMINE-REF-207` (proposed): an intermediate difference, product or expansion component overflowed, so the sign cannot be certified. Tests: `intermediate_overflow_is_refused_207_not_signed`, and in `exact::expansion` the four tests that replaced upstream's `should_panic` tests.

- `FMINE-REF-109` (proposed): `Soup::new` with a non-finite coordinate, a corner index past the positions, or more than `u32::MAX` triangles; a non-finite query point; a `beta` below 1 or not finite, which upstream raised to 1 (`bad_soups_queries_and_betas_are_refused`).
- `FMINE-REF-207` (proposed): a triangle's determinant or denominator overflows (corner distances near `1e105`, where std's `atan2` of two infinities gave upstream an answer), or an octree node's centroid, radius or moment does (`overflowing_arithmetic_is_refused_207_not_answered`).

- `FMINE-REF-109` (proposed), `volcert`: a non-finite coordinate, a corner index past the positions, or a nonzero coordinate magnitude outside 2^-200 to 2^200 (`meshes_that_are_not_closed_and_outward_are_refused`); a grid with a non-finite origin, a block size not finite and positive, a count of 0 or above `i32::MAX`, or a far corner that overflows; a depth above 6; sub-cells within four units in the last place of the grid's coordinates; more than 2^25 blocks and sub-cells to label (`bad_grids_and_depths_are_refused`).
- `FMINE-REF-207` (proposed), `volcert`: no triangles, a triangle repeating a vertex, a directed edge without its reverse twin (not closed), a directed edge used twice (not consistently oriented, or more than two triangles on an edge), a volume enclosure not above 0 (the triangles face inward) (`meshes_that_are_not_closed_and_outward_are_refused`); a surface-free component whose winding number is not within 1e-6 of an integer, is negative (a shell faces inward though the total volume is positive) or is above 1 (shells overlap, so the proportion is ambiguous) (`overlapping_and_stray_inward_shells_are_refused_not_counted`); an exact volume sum past 4,096 expansion components (no test).

`tests/predicate_corpus.rs::refusal_rows_are_refused_with_their_codes` repeats each predicate case through the public API. No input is clamped or rescaled to make it fit.

## 6. Determinism class
`BitwisePortable`, declared on every file and checked by `xtask check-det`. The predicates use `+`, `−`, `×` and `mul_add`, each correctly rounded under IEEE 754, and no step depends on the platform's libm. `vdb` is integer index arithmetic over BTreeMaps; its one float operation is `bytes_per_active`. `winding` adds `÷` and `sqrt` (both correctly rounded) and one transcendental, `det::atan2`, in upstream's operation order. The GATE-BM-01 digest is pinned from one machine (aarch64-macos); no second ISA has reproduced it yet. The winding digest (`lifted_corpus_digest_is_pinned`) was pinned on an x86_64 Linux worker and reproduced on an arm64 Mac, where upstream's own answers differ from the worker's in 107 values. `volcert` adds exact expansion sums, `next_up`/`next_down` and integer counts; its label comes from `winding_exact`, rounded to an integer. Its digest (`fixture_digest_is_pinned`: the volume enclosures and every block's counts for the sphere, torus and hollow box) was pinned on an x86_64 Linux worker, reproduced in release on a second one, and reproduced on an arm64 Mac, both before and after the band test moved from closed to open sub-cells (the three fixtures touch no lattice face, so the value did not change).

## 7. Evidence class of outputs
Not applicable: no Claims are emitted, so no `EvidenceClass` or `KnowledgeState` is assigned.

## 8. Validity domain
- Signs are exact when no intermediate result overflows or underflows: coordinate differences and their monomials up to degree 2 (`orient2d`), 3 (`orient3d`), 4 (`incircle`) and 5 (`insphere`) must stay in the normal `f64` range. Overflow is detected and refused (`FMINE-REF-207`).
- Subnormal underflow is not detected. Coordinates whose differences or products fall into the subnormal range are outside the certificate, and the sign is a no-claim.
- `incircle` assumes `a, b, c` counterclockwise and `insphere` assumes `a, b, c, d` positively oriented; the opposite orientation flips the sign, as in Shewchuk's convention.
- A winding number is exact up to rounding only off the surface. On a triangle's plane the solid angle jumps by ±½, and on a corner or edge the sign of a zero picks the answer; both are upstream's rule, kept as it is. `inside` near ½ is a no-claim.
- The octree's error is measured (0.024 at beta 2 on the rmesh-004 icosphere, off its surface shell), not bounded: Barill et al. give no a-priori bound, and none is claimed here.
- `VdbGrid` memory is proportional to allocated leaves, not active voxels: each leaf holds 512 payload slots whatever its occupancy. A dense box of 10⁸ `u64` cells measured 1.35× the payload in RSS (GATE-BM-01). One active voxel per leaf costs about 4 KiB per voxel for an 8-byte payload. Deactivated voxels do not free their leaf.
- `volcert` certifies closedness and orientation by edge pairing only: a self-intersecting mesh passes `certify`. Its volume enclosure is then the integral of its winding number, and `block_proportions` refuses the regions where that number is not 0 or 1.
- A sub-cell is in the band only when the surface meets its interior; one that the surface touches at a corner, edge or face is labelled. Two labelled cells sharing a face join one component only when no triangle lying in that face's plane meets the face's interior, which is the only way a surface missing both cells' interiors can cross between them; a face that some coplanar triangle covers keeps its two sides apart, each labelled by its own winding query.
- The `inside` count rests on rounding `winding_exact` at one point per surface-free component, at least half a sub-cell from the surface; the 1e-6 check catches a broken evaluation, it does not prove the rounding right. The band test, the cell boundaries used by it and the volume are exact.
- Sub-cell boundaries are the `f64` values `origin + size · (j / 2^depth)`, within about one unit in the last place of the nominal ones; a proportion counts nominal sub-cells, so it is exact for the `f64` cells, not for the nominal block.

## 9. Oracles
- An all-integer `i128` determinant oracle over lattice inputs, in the unit tests and in `tests/predicate_corpus.rs`. The corpus scales dyadic inputs to integers, so its oracle sees exactly the numbers the predicates see.
- Hand-derived signs for the corpus literal rows, ten of them frankensim's exact-corpus predicate rows (`fsim-r033` to `fsim-r042`), each also checked against the oracle.
- A `BTreeMap<[i32; 3], T>` oracle for `VdbGrid` (rsdf-004).
- For `winding`, upstream itself: frankensim's fs-rep-mesh `winding.rs` and `shapes.rs` at the pinned commit, run unmodified over 13 soups and 1,264 query points (`fixtures/fmine-geom/winding_corpus.txt` and its capture record, made by `scripts/oracle/winding_corpus/`), on an x86_64 Linux worker and an arm64 Mac.
- For `volcert`: closed-form volumes (box, tetrahedron, octahedron); the circumscribed ball for the sphere; and expected sub-cell counts written in the tests without calling the module, from plain coordinate comparisons of each sub-cell against each box, and from the L1 distance range of each sub-cell for the octahedron.
- No external reference implementation (no Shewchuk `predicates.c`, no CGAL, no OpenVDB, no libigl winding number) is run.

## 10. Known gaps
- `BD-193` (bd-193), `BD-191` (bd-191) and `BD-192` (bd-192): their only caller is `volcert`, in this crate, which `fmine-io`'s `omf::proportions_in_surface` (bd-aqfl) calls from outside.
- `winding` takes OMF surfaces only through `volcert` (bd-aqfl) and has no DXF ingest (plan §27 row "fs-rep-mesh `winding_exact`"); the octree is single-threaded and rebuilt per soup.
- `volcert` (bd-wq34): its one caller outside the crate is `fmine-io`'s OMF reader (bd-aqfl), on synthetic files only so far; no DXF reader feeds it; grids are axis-aligned and unrotated, with no sub-blocks; no self-intersection check; time and memory at production grid sizes are not measured (the tests run in about four seconds unoptimized). The plan's "interval arithmetic" for the volume is replaced by exact arithmetic, which is tighter. Each component costs one winding query, linear in the triangle count; only faces covered by coplanar triangles split components, so the aligned single box above needs one query and the touching pair two, but the query count on a real pit shell is not measured.
- `orient3d`, `incircle` and `insphere` go from stage A straight to the exact stage, as upstream does; only `orient2d` has stages B and C. The signs are the same; near-degenerate throughput is lower.
- Not ported from upstream's `tests/exact_corpus.rs`: its fixture file, decoder and drift guard, and its interval, Newton and Taylor-model rows, which need fs-ivl's `Interval`, not lifted here. The expansion arithmetic is crate-private, so its `proddiff` rows are covered by the unit tests instead.
- No predicate benchmark: stage-A filter rate is measured, wall time is not.
- `VdbGrid` has no columnar attribute chunks, rotated or offset frames, sub-blocking (OPEN-STRATA-1), per-brick Merkle digest, paging or parallel iteration (plan §27 row "fs-rep-sdf `VdbGrid`").

## 11. Public API
- `exact::Sign` (`flip`), `exact::Stage`
- `exact::orient2d`, `exact::orient2d_with_stage`, `exact::orient2d_sos`
- `exact::orient3d`, `exact::orient3d_with_stage`, `exact::orient3d_sos`
- `exact::incircle`, `exact::incircle_with_stage`
- `exact::insphere`, `exact::insphere_with_stage`
- `vdb::VdbGrid<T>`: `new`, `background`, `set`, `get`, `is_active`, `deactivate`, `active_count`, `iter_active`, `dilate`, `erode`, `memory_stats`
- `vdb::VdbStats` (`to_json`)
- `winding::Soup`: `new`, `position_count`, `position`, `triangles`, `corners`
- `winding::triangle_winding`, `winding::winding_exact`
- `winding::WindingOctree`: `build`, `soup`, `winding`, `inside`
- `volcert::ClosedMesh`: `certify`, `soup`, `bounds`, `volume`
- `volcert::VolumeEnclosure` (`lo`, `hi`)
- `volcert::BlockGrid`: `new`, `origin`, `size`, `counts`
- `volcert::block_proportions`, `volcert::MAX_DEPTH`
- `volcert::Proportions`: `grid`, `depth`, `cells_per_block`, `get`, `bounds`, `iter`, `blocks`, `stats`
- `volcert::BlockProportion` (`inside`, `band`), `volcert::ProportionStats`

The `exact` items are also public under `exact::predicates`.

## 12. Proof obligations
- PO-STRATA-8 (true volume in `[lo, hi]` for a certified closed mesh; verified on analytic shapes; refuse the certificate on failure): the box, tetrahedron and octahedron, including the box at UTM-sized coordinates (`volumes_enclose_exact_analytic_values_even_far_from_the_origin`), and refusals for meshes that are not closed and outward (bd-wq34).

No other PO-* is discharged here. The stage-A, B and C error bounds are Shewchuk's (1997) proven constants, carried over unchanged and not re-derived; the tests check signs against an exact oracle and do not prove the bounds.
