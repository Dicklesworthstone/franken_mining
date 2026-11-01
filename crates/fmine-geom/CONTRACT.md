# CONTRACT.md — `fmine-geom`

## 1. Purpose
Exact geometric predicates, the first slice of the Strata geometry crate (`BD-193`). `orient2d`, `orient3d`, `incircle` and `insphere` return the exact sign of their determinant, and `orient2d_sos` and `orient3d_sos` break exact ties by Simulation of Simplicity. The code is lifted from `frankensim` fs-ivl and fs-math (`registries/lifts.toml`). Nothing in the workspace calls it yet. VdbGrid (`BD-191`), winding numbers, meshes, block grids and sub-blocks are later slices.

- **Crate:** `fmine-geom`
- **Layer:** K0
- **Subsystem:** Strata
- **LOC budget:** 30,000 planned / 35,000 ceiling
- **Allowed Module Families:** `["exact"]`

## 2. Inputs
Points as `[f64; 2]` or `[f64; 3]` coordinate arrays in one caller-chosen Cartesian frame and unit; the predicates are unit-free, since only signs come out. The SoS forms also take one `u64` index per point, which must be pairwise distinct.

## 3. Outputs
- `Sign` (`Negative`, `Zero`, `Positive`): the exact sign of the determinant. `Zero` means exactly degenerate (collinear, coplanar, cocircular or cospherical).
- `Stage` (`Filtered`, `Adaptive`, `Exact`), from the `*_with_stage` forms: which rung of the ladder decided the sign.

None is a Claim.

## 4. Invariants
- Each sign equals the sign of the real-arithmetic determinant of the given floats. Tests: `lattice_battery_matches_i128_oracle` and `lattice_battery_3d_matches_i128_oracle` (6,000 random small-lattice cases against an `i128` oracle), `kettner_class_breakdown_naive_float_disagrees_somewhere`, and the families in `tests/predicate_corpus.rs`.
- Swapping two arguments flips the sign: the Kettner test and every corpus family check it.
- The SoS forms never return `Zero` for distinct indices, are antisymmetric under all 24 permutations, and agree with `orient3d` on points moved a small distance along the same moment curve: `sos_is_total_antisymmetric_and_consistent`, `orient3d_sos_full_em_ladder_antisymmetric_total_consistent`.
- The expansion arithmetic is error-free: `(a + b) − a − b`, scale-versus-product and distributivity residuals are the empty expansion (`sum_scale_product_satisfy_exact_residual_laws`).
- General-position inputs resolve at stage A more than 99% of the time (`filter_rates_general_position_resolves_fast`).

## 5. Refusals
Every public predicate returns `Result<_, Refusal>`; none panics on bad input.
- `FMINE-REF-109` (proposed): a non-finite coordinate, or a repeated SoS index. Tests: `non_finite_coordinates_are_refused_109_not_signed`, `repeated_sos_indices_are_refused_109`.
- `FMINE-REF-207` (proposed): an intermediate difference, product or expansion component overflowed, so the sign cannot be certified. Tests: `intermediate_overflow_is_refused_207_not_signed`, and in `exact::expansion` the four tests that replaced upstream's `should_panic` tests.

`tests/predicate_corpus.rs::refusal_rows_are_refused_with_their_codes` repeats each case through the public API. No input is clamped or rescaled to make it fit.

## 6. Determinism class
`BitwisePortable`, declared on every file and checked by `xtask check-det`. The arithmetic is `+`, `−`, `×` and `mul_add`, each correctly rounded under IEEE 754, and no step depends on the platform's libm. No cross-ISA golden digest has been run for this crate yet.

## 7. Evidence class of outputs
Not applicable: no Claims are emitted, so no `EvidenceClass` or `KnowledgeState` is assigned.

## 8. Validity domain
- Signs are exact when no intermediate result overflows or underflows: coordinate differences and their monomials up to degree 2 (`orient2d`), 3 (`orient3d`), 4 (`incircle`) and 5 (`insphere`) must stay in the normal `f64` range. Overflow is detected and refused (`FMINE-REF-207`).
- Subnormal underflow is not detected. Coordinates whose differences or products fall into the subnormal range are outside the certificate, and the sign is a no-claim.
- `incircle` assumes `a, b, c` counterclockwise and `insphere` assumes `a, b, c, d` positively oriented; the opposite orientation flips the sign, as in Shewchuk's convention.

## 9. Oracles
- An all-integer `i128` determinant oracle over lattice inputs, in the unit tests and in `tests/predicate_corpus.rs`. The corpus scales dyadic inputs to integers, so its oracle sees exactly the numbers the predicates see.
- Hand-derived signs for the corpus literal rows, ten of them frankensim's exact-corpus predicate rows (`fsim-r033` to `fsim-r042`), each also checked against the oracle.
- No external reference implementation (no Shewchuk `predicates.c`, no CGAL) is run.

## 10. Known gaps
- `BD-193` (bd-193): no caller in the workspace yet; the first will be a mesh or block-model slice.
- `orient3d`, `incircle` and `insphere` go from stage A straight to the exact stage, as upstream does; only `orient2d` has stages B and C. The signs are the same; near-degenerate throughput is lower.
- Not ported from upstream's `tests/exact_corpus.rs`: its fixture file, decoder and drift guard, and its interval, Newton and Taylor-model rows, which need fs-ivl's `Interval`, not lifted here. The expansion arithmetic is crate-private, so its `proddiff` rows are covered by the unit tests instead.
- No benchmark: stage-A filter rate is measured, wall time is not.

## 11. Public API
- `exact::Sign` (`flip`), `exact::Stage`
- `exact::orient2d`, `exact::orient2d_with_stage`, `exact::orient2d_sos`
- `exact::orient3d`, `exact::orient3d_with_stage`, `exact::orient3d_sos`
- `exact::incircle`, `exact::incircle_with_stage`
- `exact::insphere`, `exact::insphere_with_stage`

The same items are public under `exact::predicates`.

## 12. Proof obligations
No PO-* is discharged here. The stage-A, B and C error bounds are Shewchuk's (1997) proven constants, carried over unchanged and not re-derived; the tests check signs against an exact oracle and do not prove the bounds.
