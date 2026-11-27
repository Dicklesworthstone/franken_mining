# BD-082: 24-hour segment scanner fuzz run (`fuzz/fuzz_targets/segment.rs`)

Verdict: PASS (24 h run completed on 2026-11-27 at `95724a2`; self-verified only, single host `aarch64-macos`).

- **Scope:** the `FMT-SEG-1` scanner, `fmine_ledger::segment::inspect_segment_bytes` (BD-082, BD-088, `F7`). BD-082's done-when is its torn-write tests, and it closed on them in `07dfee39`; this run is extra evidence for the scanner, recorded here and on THREAT-MODEL-M0 open item 3.
- **Source run recorded by:** SilentBison (agent), logged on `bd-082` on 2026-11-26 16:42 UTC.
- **Delivery record committed:** 2026-11-27, after the run ended.

## Run parameters and result

Command: `nice -n 19 bash scripts/fuzz.sh 86400 segment`, built at `95724a2` with `cargo fuzz build -s none` (debug assertions on), `-rss_limit_mb=2048 -timeout=10`, no sanitizer because the workspace forbids `unsafe_code`. The binary's sha256 was not recorded at the start, and the file has been relinked since, so it cannot be given. Nor was it recorded whether the working tree had uncommitted edits in the target's closure when the run was built.

| Field | Value |
|---|---|
| Target | `fuzz/fuzz_targets/segment.rs` |
| Commit tested | `95724a2` |
| Started / ended | `2026-11-26T16:42:15Z` / `2026-11-27T16:42:16Z` |
| Duration | `86,401 s` (24 h) |
| Exit status | `0` |
| Total executions | `2,531,745,813` (about 29,302 a second, at nice 19 with load near 165 on 36 cores at the start) |
| Edge coverage / features | `468` / `1,408`, from `301` / `586` at start |
| Corpus size | `229`, grown from 5 seeds |
| Peak RSS | `29 MB` |
| Crash / timeout / OOM artifacts | `0` |

The target writes one clean segment of six batches and a seal, and each input damages a copy of it (the header of `fuzz/fuzz_targets/segment.rs` describes the three modes). A crash state must read as a torn tail at the old end with the committed batches and head hash kept. Damage inside one record with a whole record after it must never read as a torn tail, which repair would truncate. A torn tail's offset must be the verified length, and that prefix must inspect as an open segment with the same batches and head hash.

The result line is `runs/long-segment-95724a2.ndjson` under the fuzz target directory, and `artifacts/segment/` is empty. The libFuzzer log ends `Done 2531745813 runs in 86401 second(s)`. The runner then printed `scripts/fuzz.sh: line 63: one: command not found`: `fc430f9d` added a target name to `scripts/fuzz.sh` during the run, and bash read the edited file at the old offset after the target loop. The four-byte shift made it run the tail of `done` as a command, then the summary `{"targets":1,"failures":0}` and the exit test as written.

`git log 95724a2..HEAD -- crates/fmine-ledger/src/segment.rs crates/fmine-ledger/src/batch.rs crates/fmine-core/src/codec.rs crates/fmine-core/src/hash.rs fuzz/src fuzz/fuzz_targets/segment.rs` was empty when the run ended.

## Unit and planted-negative checks

- `test:fmine-ledger::segment::tests::test_every_post_header_truncation_is_clean_or_recoverable_torn_tail_and_interior_flip_refuses`
- `test:fmine-ledger::segment::tests::test_stray_bytes_before_a_whole_record_refuse_instead_of_truncating`
- `test:fmine-ledger::segment::tests::test_whole_last_record_this_build_cannot_decode_refuses_instead_of_truncating`
- Before the long run, with the scanner's old search start planted in an rch worker copy (put back cmp-equal), this target found the regression in 1.84 million runs (`bd-082`, 2026-11-25).

## Limits

- One ISA (`aarch64-macos`), debug assertions on, no sanitizer, not replicated on `x86_64-linux`.
- Inputs stay within libFuzzer's default 4 KiB, so a scan that slows with file size cannot show here; `segment::tests::test_crafted_overlapping_candidates_refuse_within_the_check_budget` covers that.
- The binary's hash and the tree's state at build time are unrecorded (above).
