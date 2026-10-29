# CONTRACT.md — `fmine-array`

## 1. Charter & Layer
- **Crate:** `fmine-array`
- **Layer:** `S0` (Substrate)
- **Subsystem:** `Bedrock`
- **LOC Budget:** `12k planned / 14k ceiling` (`registries/loc_budget.toml`)
- **Allowed Module Families:** `["grid", "col", "interval"]`

## 2. Mathematical & Domain Formulation
Implements dense 2D/3D block-model arrays (`Grid2D<T>`, `Grid3D<T>`), 64-byte cache-line-aligned `AlignedF64Block` (`FMT-COL-1` binary codec with `FMNE` magic and CRC32C), and `ColumnarIntervalTable` with zero-allocation sweep-line downhole compositing (`composite_into`, `BD-020`).

## 3. Authority, Epistemics & Representation
Preserves exact interval mass-grade conservation ($\sum L_i \rho_i g_i$) during downhole compositing.

## 4. Public API Surface
- `Grid2D<T>`, `Grid3D<T>`
- `AlignedF64Block`, `FMT_COL_V1`
- `ColumnarIntervalTable`, `CompositeInterval`

## 5. Guards & Typed Refusals
- `CodecError` on corrupted or non-finite `FMT-COL-1` frames.

## 6. Proof Obligations (`PO-*`)
- `PO-ARR-1`: `AlignedF64Block::as_ptr() as usize % 64 == 0` and bit-exact `FMT-COL-1` round-trip.
- `PO-ARR-2`: `ColumnarIntervalTable::composite_into` conserves total dry mass and contained metal to `< 1e-12` with zero heap allocations after warmup.

## 7. Failure Modes & Deterministic Fallbacks
Out-of-bounds grid indices return `None`; corrupted `FMT-COL-1` buffers fail closed with `CodecError`.

## 8. Gates & Kill Criteria (`GATE-*` / `KILL-*`)
- `GATE-BED-01`

## 9. Lifted Code & Foundation Dependencies
- Depends on `fmine-core`, `fmine-units`.

## 10. Non-Goals & Scope Boundary
Does not implement kriging or SGS (`fmine-geostat`).
