# CONTRACT.md — `fmine-array`

## 1. Purpose
Dense row-major 2D and 3D grids (`Grid2D<T>`, `Grid3D<T>`, BD-036) and a columnar downhole interval table that composites intervals to a target length into a caller-supplied slice (`ColumnarIntervalTable`, BD-310). Nothing in the workspace calls it yet. It does no kriging or simulation, and has no binary format: the `FMT-COL-1` name covers only the 64-byte alignment of `AlignedF64Block`.

- **Crate:** `fmine-array`
- **Layer:** S0
- **Subsystem:** Bedrock
- **LOC budget:** 6,000 planned / 7,000 ceiling
- **Allowed Module Families:** `["grid", "col", "interval"]`

## 2. Inputs
Grid shapes and `(i, j[, k])` indices; interval rows `(from_m, to_m, density_t_m3, grade_g_t)` as bare `f64` in metres, t/m³ and g/t (`push`, `src/lib.rs:210`); a composite length `target_length_m`.

## 3. Outputs
Grid values and linear indices; `CompositeInterval { from_m, to_m, length_m, mean_density_t_m3, mass_weighted_grade_g_t }` rows written into the caller's slice, with their count. None is a Claim.

## 4. Invariants
- Grid indexing is a bijection between `(i, j[, k])` and the linear index: `test_grid2d_and_grid3d_bijective_indexing`.
- `AlignedF64Block` is 64-byte aligned, and compositing conserves mass and contained metal to 1e-12 for depth-ordered rows: `test_fmt_col_1_alignment_and_zero_alloc_compositing` (three rows). `composite_into` allocates nothing, by inspection; allocations are not counted.

## 5. Refusals
No `FMINE-REF-nnn` code is emitted here; errors are typed `ArrayError` (`src/lib.rs:8`):
- `OutOfBounds`: a grid index outside the shape;
- `InvalidInterval`: a non-finite value, `to_m <= from_m` or density `<= 0` in `push`;
- `OutputBufferTooSmall`: the caller's slice holds fewer composites than needed.

## 6. Determinism class
Undeclared: `src/lib.rs` has no `//! Determinism:` header, which `xtask check-det` requires only in the K and E layers. The code calls no transcendental; `composite_into` divides and floors `f64` values.

## 7. Evidence class of outputs
Not applicable: no Claims are emitted, so no `EvidenceClass` or `KnowledgeState` is assigned.

## 8. Validity domain
Composites are right only when rows are stored in depth order without overlaps. `push` does not check this. A row shallower than the first row is dropped without an error, and overlapping rows are counted twice. A `target_length_m` of 0 or less returns `Ok(0)`, and a NaN target gives one bin; neither is refused. All of this is bd-b6r7.

## 9. Oracles
None (gap): the compositing test's expected sums are computed in the test itself from the same three rows.

## 10. Known gaps
- bd-b6r7: rows out of depth order or overlapping, and bad target lengths, are not refused (see Validity domain).
- BD-310 (bd-310): no 10^7-interval benchmark, timing or allocation counter; no hole-id dictionary or bit-packing.
- BD-036 (bd-036): stride and view tests; it stays open behind bd-016 (the GATE-LAYERS-00 chain).

## 11. Public API
- `ArrayError`
- `Grid2D<T>` and `Grid3D<T>`: `filled`, `linear_index`, `coords_of`, `get`, `set`; `Grid3D::as_slice`
- `AlignedF64Block`
- `ColumnarIntervalTable`: `new`, `push`, `len`, `is_empty`, `composite_into`; `CompositeInterval`

## 12. Proof obligations
No PO-* is discharged here yet. The conservation check above (`test_fmt_col_1_alignment_and_zero_alloc_compositing`) covers depth-ordered input only.
