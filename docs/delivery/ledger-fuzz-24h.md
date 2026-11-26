# BD-081: 24-hour ledger batch fuzz run (`fuzz/fuzz_targets/ledger.rs`)

Verdict: PASS (24 h run completed on 2026-11-26 at `349f0eb`; self-verified only, single host `aarch64-macos`). The codec it ran is `FMT-LEDGER-1` schema `v0` and `v1`, with `v2` refused as unsupported. Schema `v2` keyed-state witnesses came later in `3064dd85` (BD-085), and this run does not cover them. See "Changes after the run".

- **Gate / Done-when (plan §39, BD-081):** `24 h fuzz clean` over the `FMT-LEDGER-1` delta batch (`crates/fmine-ledger/src/batch.rs`). The batch is framed with `FMNE` + `LDGR` + version + CRC32C through `fmine_core::codec`, its sequences are in strict canonical order, and it is bound to its anchor.
- **Source run recorded by:** SilentBison (agent), logged on `bd-081` on 2026-11-25 15:11 UTC.
- **Delivery record committed:** 2026-11-26, after the run ended.

## Run parameters and result

Command: `nice -n 10 bash scripts/fuzz.sh 86400 ledger`, built at `349f0eb` with `cargo fuzz build -s none` (debug assertions on, so an arithmetic overflow is a finding), `-rss_limit_mb=2048 -timeout=10`. There is no sanitizer, because the workspace forbids `unsafe_code`. Binary sha256 `5f5049a64ba4526354da54b53db38f1ea42cf8d7a981914d41484e88f4bd4e0b`.

| Field | Value |
|---|---|
| Target | `fuzz/fuzz_targets/ledger.rs` |
| Commit tested | `349f0eb` |
| Started / ended | `2026-11-25T15:10:24Z` / `2026-11-26T15:10:26Z` |
| Duration | `86,401 s` (24 h) |
| Exit status | `0` |
| Total executions | `6,057,340,689` (about 70,107 a second) |
| Edge coverage / features | `514` / `1,078` |
| Corpus size | `307`, grown from 6 seeds |
| Peak RSS | `29 MB` |
| Crash / timeout / OOM artifacts | `0` |

The target reads a mode byte, then wraps the rest as a frame of version 0, 1 or an unsupported 2. Any frame it accepts must re-encode byte for byte to the input. The re-encoding always carries the right CRC32C, so a frame accepted with a wrong one, as after a flipped bit, fails that check. The six seeds (`fuzz/seeds.py`) decode, so mutation starts past the CRC. They are version 0 and 1 bodies, a whole frame, and a frame whose CRC the target repairs, and every delta and witness kind appears in them.

The runner printed `scripts/fuzz.sh: line 66: unexpected EOF while looking for matching '"'` after the result line. `096aa53c` edited `scripts/fuzz.sh` during the run, adding `segment` to the default target list. Bash had already parsed the target loop, so the result line was written as the run started it. After the loop, bash read the edited file at the old offset, and the final summary line and the runner's exit status were lost. The result line above, the libFuzzer log's `Done 6057340689 runs in 86401 second(s)`, and the empty `artifacts/ledger/` directory are the evidence.

## Unit and planted-negative checks

- `test:fmine-ledger::batch::tests::test_fmt_ledger_v1_and_n_minus_1_roundtrip_and_one_bit_flip_refusal`
- `test:fmine-ledger::batch::tests::test_fmt_ledger_refuses_disorder_tampered_payload_and_anchor_mismatch`
- Planted negatives before the 24 h run, each in an rch worker copy only and put back there (cmp equal):
  - Encoder writes `epoch ^ 1`: every seed fails with "accepted a non-canonical FMT-LEDGER-1 frame".
  - Decoder takes an unknown delta kind as a put: the fuzzer found it within 300 s at the round-trip assertion.

## Changes after the run

`git log 349f0eb..HEAD -- crates/fmine-core crates/fmine-evidence crates/fmine-ledger fuzz/src fuzz/fuzz_targets/ledger.rs` is not empty at `88d1281`:

- `3064dd85` (BD-085) changed the code this target runs. It added schema `v2` with a `KeyState` witness kind that admits subjects up to the key bound, refused `KeyState` below `v2`, and refused two `KeyState` witnesses for one key. It also widened the target's version selector to 0 to 3, with 3 unsupported. The `v0` and `v1` encodings stay pinned by `test:fmine-ledger::batch::tests::key_state_schema_is_opt_in_and_legacy_bytes_remain_pinned`. A `v0` or `v1` frame with witness tag 6 is still refused, now as an invalid field after decoding instead of an unknown tag.
- The other commits changed `segment.rs`, `cell.rs`, `l0.rs` and `verify.rs` in `fmine-ledger`, and added `plan_cert.rs` to `fmine-core`. `DeltaBatch::from_bytes` and `to_bytes` do not call into them, and `fmine_core::codec` is unchanged.

A fresh 24 h run of the same target at `88d1281`, which includes `v2`, started on 2026-11-26T15:36:54Z on the same host (binary sha256 `7c95e48571231c4a64bcf64cd39bcc778801a9f56cae02c67feb0283094b343d`). It starts from this run's 307-entry corpus plus ten seeds, four of them `v2`. Its result line will be `runs/long-ledger-88d1281.ndjson` under the fuzz target directory. BD-081 stays open until it ends, because `crates/fmine-ledger/CONTRACT.md` lists the `v2` run as BD-081's open item.

## Limits

- Executed on one ISA (`aarch64-macos`) with debug assertions enabled and no sanitizer.
- Solo verification; not independently replicated on `x86_64-linux`.
- Covers the `FMT-LEDGER-1` batch codec only. Segment files (`FMT-SEG-1`) have their own target, `fuzz/fuzz_targets/segment.rs`, which has had no long run.
