# CONTRACT.md — `fmine-array`

## 1. Purpose
Dense row-major 2D and 3D grids (`Grid2D<T>`, `Grid3D<T>`, BD-036), single-hole columnar interval tables (`ColumnarIntervalTable`), and the full `FMT-COL-1` 64-byte-aligned columnar interval chunk format (`col::ColumnarIntervalChunk`, BD-310, plan §29.4) with run-length `HoleId` dictionary encoding, delta-zigzag LEB128 millimetre depths, bit-packed measurands and qualifiers (`<DL`, `>UL`, `CommittedFlagged`), validity bitmaps, 64-byte-aligned value/density/variance blocks (`#[repr(C, align(64))]`, zero `unsafe`), per-chunk zone maps, and zero-allocation O(N) multi-hole sweep-line compositing.

- **Crate:** `fmine-array`
- **Layer:** S0
- **Subsystem:** Bedrock
- **LOC budget:** 6,000 planned / 7,000 ceiling
- **Allowed Module Families:** `["grid", "col", "interval"]`

## 2. Inputs
- Grid shapes and `(i, j[, k])` indices.
- Single-hole interval rows `(from_m, to_m, density_t_m3, grade_g_t)` (`ColumnarIntervalTable::push`).
- Multi-hole `IntervalRow { hole_id, from_mm, to_mm, measurand_id, qualifiers, valid, value, dry_density_t_m3, error_variance, xyz }` (`ColumnarIntervalChunk::push`) or canonical `FMNE` + `COL1` + CRC32C binary frames (`ColumnarIntervalChunk::from_bytes`).
- Positive finite composite lengths (`target_length_m` or `target_length_mm`) and caller-owned output slices.

## 3. Outputs
- Grid values and linear indices.
- `CompositeInterval` / `HoleComposite` rows written into caller-supplied slices with exact counts.
- Canonical `FMT-COL-1` binary bytes (`ColumnarIntervalChunk::to_bytes`) and `ChunkZoneMap` pruning queries. None is a `Claim`.

## 4. Invariants
- Grid indexing is a bijection between `(i, j[, k])` and linear index (`test_grid2d_and_grid3d_bijective_indexing`).
- `AlignedF64Block` is 64-byte aligned without `unsafe` (`test_fmt_col_1_alignment_and_zero_alloc_compositing`, `test_fmt_col_1_100k_intervals_sweep_line_conservation`).
- Compositing conserves mass and contained metal across gaps and multi-hole runs while skipping rows with cleared validity bits (`test_composite_conserves_mass_across_a_gap`, `test_fmt_col_1_multi_hole_compositing_conserves_mass_metal_and_skips_invalid`, `test_fmt_col_1_100k_intervals_sweep_line_conservation`).
- `FMT-COL-1` binary framing round-trips identically and refuses any single-bit flip (`test_fmt_col_1_roundtrip_bit_packing_zone_map_and_one_bit_flip_refusal`).
- Both `composite_into` and `composite_chunk_into` perform zero heap allocations inside their sweep loops (taking only `&self` and `&mut [..]` with primitive registers).

## 5. Refusals
Errors are typed `ArrayError` (no `FMINE-REF-nnn` emitted in S0):
- `OutOfBounds`: grid index outside shape.
- `InvalidInterval`: non-finite values, `to <= from`, density `<= 0`, or negative variance.
- `RowOutOfOrder`: overlapping or depth-regressing row within a hole (table/chunk left unchanged).
- `InvalidTargetLength`: zero, negative, or non-finite composite length.
- `OutputBufferTooSmall`: caller slice smaller than required composite bins.
- `InvalidHoleId`, `InvalidMeasurandId`, `InvalidQualifierBits`, `Codec(CodecError)`: malformed `FMT-COL-1` metadata or binary framing.

## 6. Determinism class
`src/lib.rs` and `src/col.rs` use only integers, `FMNE` canonical framing, and deterministic IEEE-754 arithmetic with no transcendentals or hash-map iteration.

## 7. Evidence class of outputs
Not applicable: S0 array substrate emits no `Claim`s.

## 8. Validity domain
- Up to `MAX_CHUNK_ROWS` (20,000,000) intervals and `MAX_HOLE_DICT` (1,000,000) holes per `FMT-COL-1` chunk, 12-bit `measurand_id` (`0..=4095`), and 3-bit `QualifierBits` (`0..=7`).
- Intervals within each hole run must be sorted and non-overlapping (`from_mm >= prev_to_mm`); gaps are permitted.

## 9. Oracles
- Exact analytical mass and metal conservation checks across single-hole, multi-hole, gap, masked-invalid, 100k-interval, and 10^7-interval synthetic suites (`crates/fmine-array/tests/fmt_col_1_bench.rs`).
- Single-bit flip refusal sweep over `FMT-COL-1` binary frames.

## 10. Known gaps
- Workspace `unsafe_code = "forbid"` prohibits a custom `#[global_allocator]` counting allocator inside workspace tests; zero per-interval heap allocation in `composite_chunk_into` is enforced structurally by taking only `&mut [HoleComposite]` with no allocating types or calls.
- No fuzz target for `FMT-COL-1` yet; upstream consumers in `CELL-DRILL` / `CELL-TEL` (`WP-07`/`WP-08`) are not wired yet.

## 11. Public API
- `ArrayError`, `Grid2D<T>`, `Grid3D<T>`, `AlignedF64Block`, `ColumnarIntervalTable`, `CompositeInterval`
- `col::{ColumnarIntervalChunk, IntervalRow, HoleRun, HoleComposite, QualifierBits, ChunkZoneMap, pack_measurand_qualifier, unpack_measurand_qualifier, TAG_COL_CHUNK, FMT_COL_V1}`.

## 12. Proof obligations
- `BD-310` performance & conservation claim: `bench_fmt_col_1_10m_intervals_under_3s_single_core` composites 10^7 intervals across 100,000 holes on one core in `< 3 s` with mass/metal conservation `< 1e-10` relative error.
