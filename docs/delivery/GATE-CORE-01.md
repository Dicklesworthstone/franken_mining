# GATE-CORE-01: 10⁵ holes / 2 × 10⁷ intervals ingested, desurveyed and composited; rebuild ≤ 15 min (BD-173)

Verdict: PASS (measured locally at `a2fb2bc0` on 2026-11-28, two runs: 652.8 s and 633.2 s against 900 s;
self-verified only, no CI run). The claim is Performance (plan §33.1): a rebuild of the stated size within the
budget on the host below. The checks that each run must pass first are listed under "What a run checks"; they are
not a separate claim.

- **Bead:** BD-173 (`bd-173`, WP-09) (plan §12, Corebox gates)
- **Threshold (plan §12, TARGET):** 10⁵ holes / 2 × 10⁷ intervals ingested, desurveyed and composited; full
  rebuild ≤ 15 min; as-known-at query from one anchor.
- **Code:** the store is `crates/fmine-projection/src/drill/sharded.rs` (`90f13707`) over `src/drill.rs`; the
  harness is `crates/fmine-verify/tests/gate_core_01.rs` (`a2fb2bc0`), test
  `gate_core_01::test_gate_core_01_full_rebuild` (ignored by default; run in release).
- **Command:** `FMINE_GATE_CORE_SHARDS=12 FMINE_GATE_CORE_THREADS=24 cargo test --release -p fmine-verify --test
  gate_core_01 -- --ignored --nocapture`, run as the built test binary (SHA-256 prefix `bbbf74902c7c5ed8`).

## The run

The campaign is `fmine-gen`'s drillhole generator (BD-131, `DrillholeCampaignConfig::benchmark_10e5_10e7`), seed
root `0x00C04E01`: 10⁵ vertical holes of 200 m on a 25 m grid. Each metre of core becomes a logged interval, a
half-core sample of it and the sample's fire-assay gold result; each hole adds its collar, one gyro station, a
dispatch of its samples and the lab certificate its results cite. That is 1,000 batches of 100 holes and
60.4 × 10⁶ records. A last batch re-assays every thousandth hole from hole 7: its 20,000 measured results are
superseded at 1.25 times the value.

1. **The ledger (not timed).** The 1,001 batches are chained and written to 63 sealed `FMT-SEG-1` segments of at
   most 192 MiB (11,833,519,259 bytes), each continuing the chain and MMR of the one before. This is the log a
   rebuild starts from.
2. **The rebuild (timed).** From opening the first segment to the last composite:
   - a reader thread opens each segment (`SegmentReader::open` checks each batch frame, the hash chain and the
     footer's MMR peaks), checks that it is sealed and continues the one before, and hands its batches on;
   - they are fed to a `ShardedDrillStore` of 12 fsqlite stores with deferred indexes. Each hole is desurveyed
     (minimum curvature, `fmine_drill::desurvey`) and its intervals boxed as its batch is applied;
   - when 1,000 batches are applied, before the re-assay, the as-known-at answer is read there (below);
   - the re-assay batch, then the R-tree load on every shard;
   - a pass on 24 threads that reads every hole's records from the store (`hole_records`) and composites its gold
     to 2 m within lithology runs (`fmine_drill::composite`, length-weighted, residual kept, `IntensiveWeighted` on
     `PointCore`; a hole whose composites do not conserve Σ length × grade to 10⁻¹² is refused, PO-CORE-2).
3. **The head checks (not timed).**

## Measured

Host: Apple M5 Ultra, 36 cores (12 performance, 24 efficiency), 256 GiB, macOS 27.0.1, rustc 1.100.0-nightly
(2026-08-30), release build; store and ledger on an external SANDISK SSD (APFS, PCI-Express). rch ran the build
locally. Other work on the machine held the one-minute load average between 140 and 342 during both runs, so the
times include contention for the 36 cores.

| | Run 1 | Run 2 |
|---|---|---|
| Ledger written (not timed) | 251.7 s | 249.1 s |
| Feed: 2 × 10⁷ intervals, 1,001 batches | 431.7 s (46,333 intervals/s) | 415.2 s (48,175 intervals/s) |
| As-known-at answer, 150 holes | 0.64 s | 1.20 s |
| R-tree load, 12 shards | 45.2 s | 33.8 s |
| Composite pass, 10⁵ holes, 10⁷ composites | 175.8 s | 184.2 s |
| **Rebuild** | **652.8 s** | **633.2 s** |
| Resident memory after the R-tree load | 25.3 GiB | 21.9 GiB |
| Head checks, 150 holes (not timed) | 14.1 s | 8.6 s |
| Load average before / after | 224.5 / 173.8 | 163.2 / 342.5 |

The rebuild is 247 s to 267 s inside the budget. After run 1 the shard stores held 29 GiB on disk. At 10⁴ holes
(2 × 10⁶ intervals, 8 shards, 16 threads, load average near 118) the rebuild took 60.3 s: feed 44.4 s, R-tree
2.4 s, composite 13.5 s.

The JSON lines of both runs (progress lines left out, long lines wrapped):

```
run 1
{"phase":"ledger","batches":1001,"records":60420000,"reassayed_results":20000,"segments":63,
 "bytes":11833519259,"seconds":251.7}
{"phase":"feed","shards":12,"holes":100000,"intervals":20000000,"batches":1001,"seconds":431.7,
 "intervals_per_second":46333,"as_known_at_holes":150,"as_known_at_seconds":0.64,"rss_mib":3184}
{"phase":"indexes","seconds":45.2,"boxes":"Current","texts":"Withheld(\"the store holds 1667082 intervals, past
 the 100000 its text index is kept for in fsqlite 0.5.1; text search over a full campaign is INT-FSEARCH-001's\")",
 "rss_mib":25937}
{"phase":"composite","threads":24,"holes":100000,"composites":10000000,"sampled_mm":20000000000,
 "worst_relative_conservation":2.1787569594701626e-16,"seconds":175.8,"rss_mib":20759}
{"phase":"rebuild","seconds":652.8,"budget_seconds":900}
{"phase":"head","checked_holes":150,"seconds":14.1}

run 2
{"phase":"ledger","batches":1001,"records":60420000,"reassayed_results":20000,"segments":63,
 "bytes":11833519259,"seconds":249.1}
{"phase":"feed","shards":12,"holes":100000,"intervals":20000000,"batches":1001,"seconds":415.2,
 "intervals_per_second":48175,"as_known_at_holes":150,"as_known_at_seconds":1.20,"rss_mib":3222}
{"phase":"indexes","seconds":33.8,"boxes":"Current","texts":"Withheld(\"the store holds 1667082 intervals, past
 the 100000 its text index is kept for in fsqlite 0.5.1; text search over a full campaign is INT-FSEARCH-001's\")",
 "rss_mib":22417}
{"phase":"composite","threads":24,"holes":100000,"composites":10000000,"sampled_mm":20000000000,
 "worst_relative_conservation":2.1787569594701626e-16,"seconds":184.2,"rss_mib":18994}
{"phase":"rebuild","seconds":633.2,"budget_seconds":900}
{"phase":"head","checked_holes":150,"seconds":8.6}
```

Both runs ended `test result: ok. 1 passed; 0 failed`, in 918.70 s and 891.00 s including the ledger and the head
checks.

## What a run checks

A run fails, and records no rebuild time, unless all of these hold.

- **As known at one anchor.** With 1,000 batches applied the answer is read through `Projection::answer`, which
  stamps it with the projection's marks. The stamp must be the ledger's anchor at that point (1,000 batches
  applied, the digest of the last campaign batch as head). For 150 holes (every 2,000th and the 100 re-assayed)
  the records the store returns, and the composites made from them, must equal the generator's before the
  re-assay. At the head the same holes must equal the generator's after it. In the small run of the same pipeline,
  checking the anchor against the re-assayed state fails at the first re-assayed hole.
- **Every hole composited, exactly.** The pass reads 10⁵ holes and 2 × 10⁷ intervals, samples and results, and
  makes 10⁷ composites (15 in the 30 m of saprolite and 85 in the fresh rock of each hole). Every millimetre of
  core is in a sampled composite (2 × 10¹⁰ mm). The worst relative conservation error over all holes is
  2.2 × 10⁻¹⁶.
- **Corrections versioned.** Each measured result of a checked re-assayed hole has two versions, the second written
  by the re-assay batch with its reason; every other checked result has one.
- **Desurveyed where the generator put it.** No collared hole is unlocated. For each checked hole a column 1 m
  either side of its collar returns exactly its 200 intervals, and a point query where the generator places the
  midpoint of its first, middle and last interval returns exactly that interval.
- **Indexes current.** After the load the R-tree on every shard is `Current`.

`gate_core_01::test_a_small_campaign_rebuilds_from_segments_and_answers_as_known_at` runs the same pipeline at 24
holes of 40 m over several segments and 3 shards, checking every hole, on every test run.
`gate_core_01::test_a_hole_generated_alone_is_the_hole_of_its_batch` shows that one hole regenerated alone, the
known answer the checks use, is the hole the ledger holds.

## Why sharded

One fsqlite 0.5.1 store is written through one connection, which cannot leave the thread that opened it. An
earlier version of this harness fed the same 2 × 10⁷ intervals into one store in 1,227.8 s, past the budget before
an index or a composite. Separate stores in one process scale with their threads: in a probe since removed from
the harness, 4.8 × 10⁵ intervals went in at 14,048 intervals/s into one store, 24,593 into two, 46,288 into four
and 77,972 into eight.

`ShardedDrillStore` routes collars and surveys to every shard and every other record to one by a hash of its key.
It commits each batch on all shards or none, refuses answers while a stop between commits leaves shards apart, and
merges answers (`crates/fmine-projection/CONTRACT.md`, BD-173 tests). Each shard is a `DrillStore` with one
connection that issues plain `BEGIN`, the configuration INT-FSQL-001 admits; no two connections share a file. The
shard threads are scoped and joined before `with_shards_at` returns (INT-AS-001 criterion 9).

## Limits

- **Text search is not part of the pass.** Each shard holds about 1.67 × 10⁶ intervals, past the 10⁵ its FTS5
  index is kept for in fsqlite 0.5.1, so the text index is withheld and `text` refuses (`FMINE-REF-502`,
  INT-FSQL-001). Text search at campaign scale is INT-FSEARCH-001's.
- **Composites are computed, not stored.** The pass composites every hole and checks conservation and totals; no
  composite table is written.
- **The as-known-at answer is read while the rebuild stands at the anchor.** The projection holds one state, its
  head. An answer at an earlier anchor after the head has moved is a replay to that anchor
  (`drill::tests::test_replaying_a_prefix_gives_the_projection_as_of_it`), which this run does not time.
- **Records are checked in full for 150 holes,** and for all 10⁵ by count, composite count, sampled length and
  conservation.
- **The generator's holes are vertical** with one survey station, so desurvey here is a straight line. Curved
  traces are covered by PO-CORE-1's tests (`crates/fmine-drill/tests/desurvey.rs`) and
  `drill::boxes::tests::test_every_point_of_a_curved_trace_lies_in_its_box`.
- **One host, a loaded one.** Two runs on a machine shared with other work; no CI run and no run on an idle
  machine.
