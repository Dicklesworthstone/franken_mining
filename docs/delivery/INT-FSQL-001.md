# INT-FSQL-001: frankensqlite (`fsqlite =0.5.1`) as the drillhole projection store

Verdict: PASS (measured locally on 2026-11-28 on the tree of the commit that adds this record; self-verified only,
no CI run). Text search over a full campaign is not part of the pass (below).

- **Bead:** BD-121 (`bd-121`, WP-06) (plan §27.4, §34.2)
- **Date:** 2026-11-28
- **Foundation row:** `FD-FSQL-00` (`franken-stack.lock`): `pin = "=0.5.1"`, posture `consume`, confined to
  `fmine-projection` and `fmine-telemetry`. The plan names `v0.4.12`; 0.5.1 is the release on crates.io, and it
  brings asupersync `=0.6.2` into the graph (`docs/delivery/INT-AS-001.md`, pin move of 2026-11-27).
- **Role:** a projection. One writer, rebuilt from `fmine-ledger`, never consulted as truth. The store issues
  plain `BEGIN`, never `BEGIN CONCURRENT`, so criteria 2, 6 and 9 do not apply (plan §34.2). fsqlite 0.5.1 runs
  even a plain `BEGIN` through its concurrent-writer machinery (a profile of ingest shows
  `begin_concurrent::concurrent_savepoint` and the SSI commit path), but one connection does all of the store's
  reads and writes, so no second schedule exists to explore.
- **Code:** `crates/fmine-projection/src/drill.rs`, `src/drill/boxes.rs`; tests in `src/drill/tests.rs`,
  `tests/high_water_marks.rs` and `tests/drill_scale.rs`.
- **Status:** criteria 1, 3, 4, 5, 7, 8 and 10 PASS on the tests named below. Scale clause (bbox p99 ≤ 50 ms
  over 10⁷ intervals): PASS, p99 2.245 ms on 25 m cubes. Text search over a full campaign FAILS in fsqlite 0.5.1
  (below). The store keeps a text index up to 10⁵ intervals (`TEXT_INDEX_MAX_ROWS`) and withholds it past that;
  then, or when the engine fails a text write, `text` refuses (`FMINE-REF-502`) and the rest of the store goes
  on. Campaign-scale text search is INT-FSEARCH-001's.

## Criteria

| # | Criterion | Result | Evidence |
|---|---|---|---|
| 1 | A single-threaded reference implementation of each projection | PASS | `drill::tests::test_the_store_matches_the_reference_after_every_batch`: 4 seeded campaigns of 28 batches (collars moved, surveys and intervals moved between holes, holes made undesurveyable and desurveyable again). After every batch the store equals a from-scratch recomputation on every record, interval row, box (bit for bit), unlocated hole, R-tree row, bbox answer and FTS5 answer. |
| 2 | Schedule exploration over concurrent reads and projection writes | N/A | No `BEGIN CONCURRENT`; one connection reads and writes. |
| 3 | Crash injection at every reserve/materialize/publish boundary | PASS | `drill::tests::test_a_crash_at_any_write_leaves_a_batch_wholly_applied_or_absent`: a child process is killed at every write of an apply, from `BEGIN` to just after `COMMIT`, in each of 8 batches (more than 60 crash points). The store holds the batch before or, once `COMMIT` has returned, the batch itself, and its mark says which; feeding resumes to the reference. `drill::tests::test_a_crash_in_an_index_rebuild_leaves_them_current_or_refusing` does the same at every write of `rebuild_indexes`. |
| 4 | Positive and negative-read phantom tests | PASS | In test 1, every bbox query's answer equals brute-force box intersection over the reference: no interval missing and none extra, including intervals whose box moved away from the query or whose hole lost its trace. Superseded index rows never answer. |
| 5 | Deterministic semantic-rebase equivalence | PASS | `drill::tests::test_a_deferred_rebuild_equals_the_incremental_store` (indexes deferred and loaded once equal batch-by-batch feeding, and the store keeps up after); `drill::tests::test_rebuilding_the_indexes_drops_their_garbage_and_keeps_every_answer`. |
| 6 | Commit-combiner sequence and active-registration proofs | N/A | No `BEGIN CONCURRENT`; one connection. |
| 7 | Stale-anchor refusal and historical `AS OF` replay | PASS | `high_water_marks::test_replays_are_skipped_and_gaps_and_forks_move_no_mark` (a replayed batch is skipped; a gap or a fork moves no mark and is refused); `drill::tests::test_replaying_a_prefix_gives_the_projection_as_of_it`. |
| 8 | Recovery that distinguishes published, staged, aborted and indeterminate states | PASS | One transaction holds a batch's effect and its mark, so after a crash the mark names the last published batch and nothing staged survives (test 3). Each index records `current`, `stale` (deferred or a rebuild stopped), `failed: <engine message>` or (text) `withheld: <reason>`, and an index not current refuses its queries (`drill::tests::test_a_text_index_the_engine_cannot_write_fails_alone`, `drill::tests::test_a_text_index_past_its_limit_is_withheld_and_the_rest_goes_on`, `drill::tests::test_a_store_on_disk_resumes_after_reopen`). |
| 9 | Witness-refinement exhaustion creates false conflicts only | N/A | No `BEGIN CONCURRENT`; one connection. |
| 10 | No direct use of lower-level APIs that bypass the connection and witness pipeline | PASS | `fmine-projection` imports `fsqlite::blocking::{Connection, Error, Params, Row, ToSql, params}` and nothing else from fsqlite (default features off; `native`, `fts5`, `rtree`). |

## Scale (release build, Apple M-series, 256 GiB, SANDISK external SSD)

Holes on a 50 m grid, 100 one-metre intervals each, three gyro stations per hole; batches of 100 holes (10,400
records) chained as a ledger chains them and fed through `Projection::feed` with the indexes deferred, then
`rebuild_indexes`; 1,000 queries each of 25 m and 200 m cubes at random places in the field; two relogs of one
hole with the indexes current (`tests/drill_scale.rs`). The test holds the 25 m cubes (six hits each on average)
to p99 ≤ 50 ms and reports the 200 m cubes (735 hits each on average at 10⁵, 853 at 10⁷) without a bound. The machine is shared: the load
average is given for each run, on 36 cores.

| | 10⁵ intervals (1,000 holes) | 10⁷ intervals (100,000 holes) |
|---|---|---|
| Load average at start | 215 | 253 (328 at the end) |
| Ingest | 9.3 s, 10,731 intervals/s; slowest batch 1.0 s | 1,417 s, 7,056 intervals/s; slowest batch 7.7 s |
| `rebuild_indexes` | 1.3 s; text index current | 88.2 s, R-tree only: text index withheld; 17,868 MiB resident after |
| bbox, 25 m cubes | p50 0.110 ms, p99 2.201 ms, max 6.668 ms | p50 0.119 ms, p99 2.245 ms, max 7.737 ms |
| bbox, 200 m cubes | p50 19.6 ms, p99 126.0 ms, max 275.8 ms | p50 34.6 ms, p99 171.8 ms, max 1,393.8 ms |
| Relog of 100 intervals' text | 136.4 ms | 5,054.3 ms, the first write after the index load; why it is slow is not investigated |
| Relog of 100 intervals' depths | 19.8 ms | 233.6 ms |
| `text("relogged")`, 100 hits | p50 15.9 ms, max 18.3 ms | refused, `FMINE-REF-502` (withheld) |

## fsqlite 0.5.1 defects met (for upstream)

- **FTS5 fails some segment writes** with "fts5: corrupt %_data record: pending hash produced no encodable segment
  leaves" when the text holds few distinct words, each repeated many times. Measured on fresh stores: 400 rows of
  one 4,095-byte field repeating "pyrite" fail where 100 load. 5 × 10⁵ rows of "SCHIST pyrite veins fresh"
  (GRANITE every seventh row) in one `INSERT … SELECT` fail after 243 s, with rowid steps of 1 and 7 alike.
  50,000-row chunks of that text, each in its own transaction, commit four times and the fifth (2 to 2.5 × 10⁵
  rows) fails in its merge after 22.8 s; 10⁵ rows load. 5 × 10⁵ rows of "schist pyrite w<i mod 97>" load in
  2.4 s. The cause is fsqlite's; the mechanism is not established here, and neither is where a real campaign's
  logging vocabulary meets it. The store marks the text index failed and goes on
  (`drill::tests::test_a_text_index_the_engine_cannot_write_fails_alone` meets the limit in the engine itself).
- **Undoing a failed FTS5 statement can take longer than the load.** Two 10⁷ runs loaded the text index in one
  `INSERT … SELECT` of these intervals' text. In the first, the load returned the segment error above less than
  155 s after ingest ended, the R-tree load included. In the second, which gave the text load a transaction of its
  own, sampling 19 minutes into the index load found the engine in `Fts5Table::rollback_to` → `undo` →
  `InvertedIndex::remove_document`. Resident memory had fallen from 10.1 GiB at 28 minutes into the run to 2.0 GiB
  at 39, and the run was stopped there. Its error was never returned, and why the second undo ran so much longer
  is not known. The store now loads the text index in statements of 10,000 rows, and loads none past 10⁵
  intervals.
- **FTS5 cost grows with the index:** a `MATCH` with 100 hits takes 59 ms at 10⁵ rows and 0.67 s at 10⁶; with 10⁴
  hits, 25 s at 10⁶. A delete by rowid costs about 0.6 s per row at 10⁵ rows.
- **Each statement that writes the R-tree copies it,** and a delete from it is a pass over the whole index. The
  store writes the R-tree append-only, in statements of up to 4,096 rows, and drops superseded rows only in
  `rebuild_indexes`.
- **A bare `?` placeholder is an internal error;** numbered placeholders (`?1`) work.
- **A virtual table cannot be recreated in the transaction that dropped it;** `rebuild_indexes` drops in one
  transaction and recreates in the next.
- **chrono** reaches the graph through `fsqlite-func`; RUSTSEC-2026-0543 (unmaintained, informational) is ignored
  in `deny.toml` with that reason.

## Not covered

- Text search at campaign scale: INT-FSEARCH-001 (frankensearch). `text` serves stores of up to 10⁵ intervals and
  refuses past that or once the text index has failed.
- The 15-minute rebuild of 10⁵ holes and 2 × 10⁷ intervals, desurveyed and composited, is BD-173's; the ingest
  rate measured here is its starting point.
