# BD-017: 24-hour codec fuzz run (`fuzz/fuzz_targets/codec.rs`)

Verdict: PASS (24 h run completed on 2026-11-02 at `33a7bdf`; self-verified only, single host `aarch64-macos`; codec and fuzz sources verified unchanged from `33a7bdf` to `58b9729504bf`)

- **Gate / Done-when (plan §39, BD-017):** `24 h fuzz clean` over `FMNE` magic, little-endian framing, bounds enforcement, CRC32C, and strict canonical-order sequence encoding/decoding (`crates/fmine-core/src/codec.rs`).
- **Source run recorded by:** SwiftBasin (agent), logged on `bd-017` on 2026-11-02 22:22 UTC.
- **Delivery record committed:** 2026-11-20 after verifying `git log 33a7bdf..HEAD -- crates/fmine-core/src/codec.rs fuzz/fuzz_targets/codec.rs fuzz/src/lib.rs` is empty.

## Run parameters and result

Command: `bash scripts/fuzz.sh 86400 codec` (built at `33a7bdf` with debug assertions on, `-rss_limit_mb=2048 -timeout=10`, no sanitizer because the workspace forbids `unsafe_code`).

| Field | Value |
|---|---|
| Target | `fuzz/fuzz_targets/codec.rs` |
| Commit tested | `33a7bdf` (unchanged through `58b9729504bf`) |
| Duration | `86,400 s` (24 h) |
| Exit status | `0` |
| Total executions | `2,232,326,022` |
| Edge coverage | `477` |
| Corpus size | `3,801` |
| Crash / timeout / OOM artifacts | `0` |

## Unit & planted-negative checks

- `test:fmine-core::codec::tests::test_each_header_error_is_its_own_variant`
- `test:fmine-core::codec::tests::test_each_body_error_is_its_own_variant`
- `test:fmine-core::codec::tests::test_canonical_seq_round_trips_and_refuses_disorder`
- `test:fmine-core::codec::tests::test_every_error_displays_a_message`
- Planted negatives caught before the 24 h run (each reverted):
  - `get_bytes` bound loosened by 1 byte: caught in 11,138 fuzz runs.
  - Decoder `>=` changed to `>`: caught by `test_canonical_seq_round_trips_and_refuses_disorder`.
  - Encoder `>=` changed to `>`: caught by `codec` fuzz target after ~250 runs.

## Limits

- Executed on one ISA (`aarch64-macos`) with debug assertions enabled and no sanitizer.
- Solo verification; not independently replicated on `x86_64-linux`.
