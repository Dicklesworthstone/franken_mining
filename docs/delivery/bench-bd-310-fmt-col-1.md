# Delivery Record — `BD-310` (`FMT-COL-1` Columnar Interval Chunks & `10^7`-Interval Sweep-Line Benchmark)

- **Bead:** `bd-310` (Plan `BD-310`, WP-01, P0)
- **Claim type:** Performance + Known answers (conservation) — never conflated (§33.1)
- **Date:** 2026-11-21 UTC (2026-11-20 local)
- **Verdict:** PASS (solo-verified on `hz3` via `rch`; no CI run recorded yet)

## Hardware & Environment (FACT)

- **Host:** RCH worker `hz3` (`x86_64` Linux)
- **CPU:** `AMD EPYC 7282 16-Core Processor` (64 logical CPUs; benchmark executes single-threaded on 1 core)
- **Memory:** 251 GiB RAM
- **Toolchain:** `nightly-2026-08-31` (`rustc 1.100`, Rust 2024 edition)
- **Profile:** `--release` (optimized)

## Command

```bash
export CARGO_TARGET_DIR="${RCH_TARGET_BASE:-${TMPDIR:-/tmp}}/rch_target_fmine"
rch exec --worker hz3 -- cargo test --release -p fmine-array --test fmt_col_1_bench -- --ignored --nocapture
```

## Workload Specification (FACT)

- **Test function:** `crates/fmine-array/tests/fmt_col_1_bench.rs::bench_fmt_col_1_10m_intervals_under_3s_single_core`
- **Dataset:** `10,000,000` intervals across `100,000` drillholes (`100` contiguous `1.5 m` intervals per hole, depth `0.0 m..150.0 m`), composited into `7,500,000` equal-length `2.0 m` composites (`75` composites per hole), deterministic closed-form density (`2.60 + 0.05 * (h % 5)`) and grade (`0.4 + 0.01 * ((h * 31 + j) % 200)`), zero randomness.
- **Format exercised:** `FMT-COL-1` (`crates/fmine-array/src/col.rs`): dictionary + run-length `HoleId`, delta-zigzag LEB128 millimetre depths, bit-packed `u16` `(12-bit measurand_id | 3-bit QualifierBits)`, `u64` validity bitmap, 64-byte-aligned (`#[repr(C, align(64))]`) value/density/variance blocks, `ChunkZoneMap`, and `composite_chunk_into` single-pass O(N) sweep-line compositing into pre-allocated `&mut Vec<CompositeRow>`.

## Measured Results — 3 Replicates on `hz3` (FACT)

| Replicate | RCH Job | Intervals | Holes | Composites | Elapsed (ms) | ns / interval | Mass rel. err | Metal rel. err | Budget (`< 3000 ms`) |
|---|---|---|---|---|---|---|---|---|---|
| 1 | `j-30117904582181340` | 10,000,000 | 100,000 | 7,500,000 | `213.16` | `21.32` | `9.312e-13` | `6.860e-12` | PASS (14.1× headroom) |
| 2 | `j-30117904582181363` | 10,000,000 | 100,000 | 7,500,000 | `156.28` | `15.63` | `9.312e-13` | `6.860e-12` | PASS (19.2× headroom) |
| 3 | `j-30117904582181364` | 10,000,000 | 100,000 | 7,500,000 | `160.01` | `16.00` | `9.312e-13` | `6.860e-12` | PASS (18.7× headroom) |

- **Spread (3 replicates):** min `156.28 ms`, median `160.01 ms`, mean `176.48 ms`, max `213.16 ms` (`15.63–21.32 ns/interval`).
- **Conservation:** Mass relative error `9.312e-13 < 1e-10`; metal relative error `6.860e-12 < 1e-10`.

## Zero-Per-Interval Allocation Proof & Honest Scope Limits

- **Workspace constraint:** `unsafe_code = "forbid"` workspace-wide forbids installing a custom `#[global_allocator]` (`GlobalAlloc` is an `unsafe trait`).
- **Verified property:** `composite_chunk_into` takes `&mut Vec<CompositeRow>` pre-allocated by the caller (`Vec::with_capacity(expected_composites)`) and verifies `out.capacity() == cap_before` after compositing all `10,000,000` intervals; its inner sweep-line loop allocates zero `Vec`, `String`, `Box`, or heap collections (verified by inspection of `crates/fmine-array/src/col.rs::composite_chunk_into` and `capacity_before == capacity_after` assertion in both the `100k` always-run test and the `10^7` benchmark).
- **Unit & codec tests passing alongside:**
  - `test:fmine-array::col::tests::test_fmt_col_1_roundtrip_bit_packing_zone_map_and_one_bit_flip_refusal`
  - `test:fmine-array::col::tests::test_fmt_col_1_multi_hole_compositing_conserves_mass_metal_and_skips_invalid`
  - `test:crates/fmine-array/tests/fmt_col_1_bench.rs#test_fmt_col_1_100k_intervals_sweep_line_conservation`
  - `test:crates/fmine-array/tests/fmt_col_1_bench.rs#bench_fmt_col_1_10m_intervals_under_3s_single_core`
- **Verification status:** Solo-verified by agent on `hz3`; not yet independently replicated on CI or macOS ARM64.
