# CONTRACT.md — `fmine-geom`

## 1. Purpose
The Strata geometry crate. Two slices so far, both lifted from `frankensim` (`registries/lifts.toml`):
- `exact` (`BD-193`): `orient2d`, `orient3d`, `incircle` and `insphere` return the exact sign of their determinant, and `orient2d_sos` and `orient3d_sos` break exact ties by Simulation of Simplicity (from fs-ivl and fs-math);
- `vdb` (`BD-191`): `VdbGrid<T>`, a sparse root map → 32³ internal node → 8³ bitmasked leaf tree over `i32` voxel indices, with a deterministic `iter_active` order (from fs-rep-sdf).

Nothing in the workspace calls either yet. Winding numbers, meshes, block grids, columnar attribute chunks, rotated grid frames and sub-blocks are later slices.

- **Crate:** `fmine-geom`
- **Layer:** K0
- **Subsystem:** Strata
- **LOC budget:** 30,000 planned / 35,000 ceiling
- **Allowed Module Families:** `["exact", "vdb"]`

## 2. Inputs
- `exact`: points as `[f64; 2]` or `[f64; 3]` coordinate arrays in one caller-chosen Cartesian frame and unit; the predicates are unit-free, since only signs come out. The SoS forms also take one `u64` index per point, which must be pairwise distinct.
- `vdb`: `[i32; 3]` voxel indices and a `Copy` payload `T`, with a background value returned for inactive voxels. The grid has no frame, cell size or unit; mapping world coordinates to indices is the caller's.

## 3. Outputs
- `Sign` (`Negative`, `Zero`, `Positive`): the exact sign of the determinant. `Zero` means exactly degenerate (collinear, coplanar, cocircular or cospherical).
- `Stage` (`Filtered`, `Adaptive`, `Exact`), from the `*_with_stage` forms: which rung of the ladder decided the sign.
- `VdbGrid` reads (`get`, `is_active`, `active_count`, `iter_active`) and `VdbStats`, a layout-derived memory estimate (`memory_stats`, `to_json`).

None is a Claim.

## 4. Invariants
- Each sign equals the sign of the real-arithmetic determinant of the given floats. Tests: `lattice_battery_matches_i128_oracle` and `lattice_battery_3d_matches_i128_oracle` (6,000 random small-lattice cases against an `i128` oracle), `kettner_class_breakdown_naive_float_disagrees_somewhere`, and the families in `tests/predicate_corpus.rs`.
- Swapping two arguments flips the sign: the Kettner test and every corpus family check it.
- The SoS forms never return `Zero` for distinct indices, are antisymmetric under all 24 permutations, and agree with `orient3d` on points moved a small distance along the same moment curve: `sos_is_total_antisymmetric_and_consistent`, `orient3d_sos_full_em_ladder_antisymmetric_total_consistent`.
- The expansion arithmetic is error-free: `(a + b) − a − b`, scale-versus-product and distributivity residuals are the empty expansion (`sum_scale_product_satisfy_exact_residual_laws`).
- General-position inputs resolve at stage A more than 99% of the time (`filter_rates_general_position_resolves_fast`).
- `VdbGrid` agrees exactly with a `BTreeMap` oracle on actives, values, the iterated set and misses (`rsdf_004_vdb_matches_the_oracle`, upstream's rsdf-004). Index splitting round-trips for every `i32` coordinate tested, extremes included (`split_unsplit_round_trips_across_signs_and_extremes`).
- `iter_active` is strictly increasing in (root key, internal index, leaf index) and does not depend on insertion order. Tests: `golden_iter_active_digest` (pinned digest) and `tests/gate_bm_01.rs`, which builds 10⁸ cells in two opposite orders and compares digests.

## 5. Refusals
Every public predicate returns `Result<_, Refusal>`; none panics on bad input.
- `FMINE-REF-109` (proposed): a non-finite coordinate, or a repeated SoS index. Tests: `non_finite_coordinates_are_refused_109_not_signed`, `repeated_sos_indices_are_refused_109`.
- `FMINE-REF-109` (proposed): `VdbGrid::dilate` when an active voxel lies on the `i32` index boundary; the grid is left unchanged (`dilate_refuses_at_the_index_boundary_and_leaves_the_grid_unchanged`).
- `FMINE-REF-207` (proposed): an intermediate difference, product or expansion component overflowed, so the sign cannot be certified. Tests: `intermediate_overflow_is_refused_207_not_signed`, and in `exact::expansion` the four tests that replaced upstream's `should_panic` tests.

`tests/predicate_corpus.rs::refusal_rows_are_refused_with_their_codes` repeats each predicate case through the public API. No input is clamped or rescaled to make it fit.

## 6. Determinism class
`BitwisePortable`, declared on every file and checked by `xtask check-det`. The predicates use `+`, `−`, `×` and `mul_add`, each correctly rounded under IEEE 754, and no step depends on the platform's libm. `vdb` is integer index arithmetic over BTreeMaps; its one float operation is `bytes_per_active`. The GATE-BM-01 digest is pinned from one machine (aarch64-macos); no second ISA has reproduced it yet.

## 7. Evidence class of outputs
Not applicable: no Claims are emitted, so no `EvidenceClass` or `KnowledgeState` is assigned.

## 8. Validity domain
- Signs are exact when no intermediate result overflows or underflows: coordinate differences and their monomials up to degree 2 (`orient2d`), 3 (`orient3d`), 4 (`incircle`) and 5 (`insphere`) must stay in the normal `f64` range. Overflow is detected and refused (`FMINE-REF-207`).
- Subnormal underflow is not detected. Coordinates whose differences or products fall into the subnormal range are outside the certificate, and the sign is a no-claim.
- `incircle` assumes `a, b, c` counterclockwise and `insphere` assumes `a, b, c, d` positively oriented; the opposite orientation flips the sign, as in Shewchuk's convention.
- `VdbGrid` memory is proportional to allocated leaves, not active voxels: each leaf holds 512 payload slots whatever its occupancy. A dense box of 10⁸ `u64` cells measured 1.35× the payload in RSS (GATE-BM-01). One active voxel per leaf costs about 4 KiB per voxel for an 8-byte payload. Deactivated voxels do not free their leaf.

## 9. Oracles
- An all-integer `i128` determinant oracle over lattice inputs, in the unit tests and in `tests/predicate_corpus.rs`. The corpus scales dyadic inputs to integers, so its oracle sees exactly the numbers the predicates see.
- Hand-derived signs for the corpus literal rows, ten of them frankensim's exact-corpus predicate rows (`fsim-r033` to `fsim-r042`), each also checked against the oracle.
- A `BTreeMap<[i32; 3], T>` oracle for `VdbGrid` (rsdf-004).
- No external reference implementation (no Shewchuk `predicates.c`, no CGAL, no OpenVDB) is run.

## 10. Known gaps
- `BD-193` (bd-193) and `BD-191` (bd-191): no caller in the workspace yet; the first will be a mesh, block-model or stockpile slice.
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

The `exact` items are also public under `exact::predicates`.

## 12. Proof obligations
No PO-* is discharged here. The stage-A, B and C error bounds are Shewchuk's (1997) proven constants, carried over unchanged and not re-derived; the tests check signs against an exact oracle and do not prove the bounds.
