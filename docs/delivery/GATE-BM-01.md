# GATE-BM-01: lifted `VdbGrid` at 10⁸ active cells (BD-191)

Verdict: PASS (measured locally at `ef49fcf` on 2026-11-01; self-verified only; no CI run, see below)

- **Gate (plan §26, Bedrock):** the lifted `VdbGrid` with 10⁸ active cells × 8 B payload stays resident within
  2.5× the payload, and `iter_active` yields a deterministic digest.
- **Recorded by:** SwiftBasin (agent). The lift landed in `c88bba7`. This record is from a fresh run at `ef49fcf`,
  whose `fmine-geom` sources match `c88bba7`'s vdb module.

## Measured at `ef49fcf`

`cargo test --release -p fmine-geom -- --include-ignored --nocapture` on an Apple M5 Ultra (256 GiB, macOS,
rustc 1.100.0-nightly 2026-08-30). rch fell back to local. 34 tests passed, 0 failed:
- 26 unit tests, among them `vdb::tests::golden_iter_active_digest` (strict order) and
  `vdb::tests::rsdf_004_vdb_matches_the_oracle`;
- `gate_bm_01_1e8_cells_within_2_5x_payload_with_deterministic_digest`;
- 7 predicate-corpus tests.

```
{"gate":"GATE-BM-01","active":100544625,"payload_bytes":804357000,"rss_delta_bytes":1087127552,"ratio":1.352,
 "layout_estimate":{"active":100544625,"leaves":205379,"internals":18,"resident_bytes":856842340,"bytes_per_active":8.52},
 "build_s":2.0,"digest_s":2.2,"digest":"495a719a42a27ad18415b24a590e8a9291d777ad052d84c81d0dda6346a64de5"}
```

The test builds the grid in two insertion orders. It asserts that both give the pinned digest and that the
resident-set growth, read from `ps`, is at most 2.5× the payload. The measured ratio is 1.352.

## Limits

- Resident memory is read with `ps` around the build, so allocator retention counts against the grid. The test
  fails if `ps` cannot be read.
- The test is `#[ignore]` (about 1 GB, release only), so CI does not run it. No other agent or machine has
  reproduced it.
