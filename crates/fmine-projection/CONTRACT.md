# CONTRACT.md — `fmine-projection`

## 1. Purpose
The projector framework (`BD-120`, plan §26.1) and its first production projector, the fsqlite drillhole projection (`BD-121`, plan §27.4). fsqlite, fgdb and frankensearch views are derived from the ledger: rebuildable, high-water-marked and never consulted as truth. A projector's store records each cell's high-water mark with the batch that moved it. A projection applies each cell's committed batches once, in order, and tells where it stands against the log. `drill::DrillStore` holds each drill record's current version, each logging interval's 3-D box in an R-tree and its logging text in FTS5.

- **Crate:** `fmine-projection`
- **Layer:** V
- **Subsystem:** Bedrock
- **LOC budget:** 20,000 planned / 23,000 ceiling
- **Allowed Module Families:** `["drill", "mark", "projection"]`

## 2. Inputs
- Committed `fmine_ledger::batch::DeltaBatch`es, from `L0Ledger::batches` or from a segment (`SegmentReader::open(..).batches`).
- A `projection::Projector`: the store being projected into.
- For `drill`: the `drill/<kind>/<id>` records `fmine-drill::store` writes (`Put` and `Supersede` ops; other keys are skipped), and a store path or an in-memory store.
- Dependencies: `fmine-core`, `fmine-drill`, `fmine-evidence` and `fmine-ledger` (`V -> K1`, `V -> S0`, `V -> S1`). One external crate on native targets only: `fsqlite =0.5.1` (FD-FSQL-00), default features off, features `native`, `fts5` and `rtree`, through `fsqlite::blocking::Connection` alone (INT-FSQL-001 criterion 10). It brings asupersync `=0.6.2` (FD-AS-00), patched to the revision `franken-stack.lock` pins. wasm32 builds have no `drill` module.

## 3. Outputs
- `mark::HighWaterMark` per cell (batches applied and the last one's digest) and `mark::Anchor`, the vector of them (FMT-ANCHOR-1's `(cell, seq, digest)` marks, plan §6.3).
- `mark::Stamped<T>`: an answer with the anchor it was read at.
- `mark::Status` of a cell against the ledger or a segment's digests: `Current`, `Behind { by }` or `Diverged`.
- `drill::DrillStore` answers: `bbox(crs, box)` (the intervals whose box meets a query box), `text(fts5_query)`, `record(cell, key)` (current version, version count and the batch that wrote it), `intervals()`, `unlocated_holes()` (collared holes with no trace, and why), `index_garbage()`.
- `drill::boxes::interval_box`: the axis-aligned box covering a desurveyed trace between two depths.

## 4. Invariants
- **Mark and state move together:** `Projector::apply` stores the batch's effect and the new mark both or neither; `Projection::open` starts from the marks the store holds (`high_water_marks::test_a_restarted_projection_resumes_from_its_stores_marks`).
- **In order, once:** batch `n` of a cell is applied only when `n` batches are applied and its `prev_hash` is the mark's head. Earlier batches are skipped, and the last applied one must match the mark's digest. Gaps and forks are refused, and the mark does not move (`high_water_marks::test_replays_are_skipped_and_gaps_and_forks_move_no_mark`).
- **The mark is the ledger's chain:** after batch `n` the mark's head is that batch's digest, the `prev_hash` of batch `n + 1` (`high_water_marks::test_marks_follow_the_ledger_batch_by_batch_across_cells`).
- **Checked marks:** after an apply the store must hold the new mark. After a failed apply the in-memory marks are reread from the store (`high_water_marks::test_a_failed_or_unrecorded_apply_moves_no_mark`).
- **Status:** a mark past the log, or with a head that is not the log's digest at that point, is `Diverged`; a prefix is `Behind`; the log's head is `Current` (`high_water_marks::test_status_reports_behind_current_and_diverged`).
- **Either rung:** a projection fed from a segment on disk reaches the same state and marks as one fed from L0 (`high_water_marks::test_a_segment_on_disk_projects_like_the_l0_ledger`).
- **Drill store equals its reference:** after every batch of seeded campaigns (collars, surveys and intervals, with collars moved, surveys and intervals moved between holes, and holes made undesurveyable and desurveyable again), the store holds the records, interval rows, bit-identical boxes and unlocated holes that a from-scratch recomputation gives (`drill::tests::test_the_store_matches_the_reference_after_every_batch`).
- **Exact bbox answers:** `bbox` returns exactly the intervals whose `f64` box meets the query box in the query's CRS: no phantom from the `f32` R-tree, a superseded box or an unlocated hole, and none missing; faces that touch count (same test, against brute force).
- **Boxes cover the trace:** every point of the minimum-curvature trace between two depths lies in `interval_box` (`drill::boxes::tests::test_every_point_of_a_curved_trace_lies_in_its_box`).
- **Batch or nothing:** a process killed at any write of an apply leaves the store at the batch before, or at the batch once `COMMIT` returned, with the mark saying which; one killed in `rebuild_indexes` leaves the indexes current and right, or stale and refusing (`drill::tests::test_a_crash_at_any_write_leaves_a_batch_wholly_applied_or_absent`, `drill::tests::test_a_crash_in_an_index_rebuild_leaves_them_current_or_refusing`).
- **Rebuild equals incremental:** a store fed with deferred indexes and rebuilt holds what a store fed batch by batch holds, and keeps up after (`drill::tests::test_a_deferred_rebuild_equals_the_incremental_store`).

## 5. Refusals
- `ProjectionError::Gap` → `FMINE-REF-502` (a batch past the next one).
- `ProjectionError::ChainBreak` and `ProjectionError::MarkNotRecorded` → `FMINE-REF-505` (a fork or a diverged projection; a store that did not keep its mark).
- `ProjectionError::Store` → `FMINE-REF-506` (the projector's store failed).
- `ProjectionError::Ledger` and `ProjectionError::Source` keep the ledger's codes.
- `drill::DrillStoreError::IndexStale` → `FMINE-REF-502` (indexes deferred or a rebuild stopped: `bbox` and `text` refuse). `Sql`, `Record` (a drill key whose payload is another record, or no drill kind), `Schema` (another schema's store) and `Range` → `FMINE-REF-506`. Through `Projection::feed` a drill error is `ProjectionError::Store`.

## 6. Determinism class
`BitwisePortable` (plan §29.2): sequence numbers, digests and ordered maps only. A projector's own store states its own class. `drill`: `BitwisePortable` answers. Boxes use IEEE-754 arithmetic over `fmine_drill::desurvey` points; row, box and text ids follow the batches. The `f32` R-tree only proposes candidates. The store file's bytes are fsqlite's and not claimed.

## 7. Evidence class of outputs
Derived views; never evidence or authority (plan §26.1). An answer's anchor names the ledger state it reflects.

## 8. Validity domain
- One authority log per cell, as `fmine-ledger` commits it. Quarantined lower-epoch batches are not projected.
- Batches before the last applied one are skipped without a digest check. `status` compares the whole chain through the mark's head digest.

## 9. Oracles
- The ledger's own head hashes and `prev_hash_at`, and a segment's verified batch digests.
- An in-memory key-index projector in the tests, with switches for a failing store and for one that drops marks.
- `drill`: a from-scratch reference built from the campaign's current records with `fmine_drill::desurvey` and `interval_box`; brute-force box intersection; a token scan for FTS5 words, phrases and prefixes; trace points sampled every 5 mm for the box bound. Seven planted mutants are caught (a moved survey's old hole not recomputed, no stale-row removal, no exact refinement, no transaction, index hits looked up by row id).

## 10. Known gaps
- fgdb is the `INT-FGDB-001` spike (`BD-126`); frankensearch is `BD-262`.
- `drill` text search does not scale in fsqlite 0.5.1: an FTS5 `MATCH` or insert costs time in proportion to the index (100 hits: 59 ms at 10⁵ rows, 0.67 s at 10⁶; 10⁴ hits: 25 s at 10⁶). Text search over a full campaign is frankensearch's (`INT-FSEARCH-001`); `text` serves small stores.
- `drill` index rows are append-only. A changed box or text leaves its old row as garbage (`index_garbage`) until `rebuild_indexes`, because deleting rows from fsqlite 0.5.1's R-tree or FTS5 costs a pass over the whole index. Each statement that writes the R-tree copies it, so a batch writes it in few statements.
- `drill` projects collars, surveys and intervals; samples, assays and QA/QC records are kept as current versions in `drill_record` but have no tables of their own yet.
- Discard-and-rebuild on `Diverged` and freshness-required refusals for queries are `BD-122` (F5). This crate reports the status they act on.
- Multi-segment logs: a projection catches up from one segment's batches at a time.

## 11. Public API
- `mark::{HighWaterMark, Anchor, Stamped, Status}`
- `projection::{Projector, Fed, ProjectionError, Projection}`
- `drill::{DrillStore, DrillStoreError, IntervalRef, IntervalRow, StoredRecord, UnlocatedHole, IndexGarbage, SCHEMA, RTREE_ROWS_PER_STATEMENT}` (native only)
- `drill::boxes::{Box3, interval_box, ROUNDING_M}`

## 12. Proof obligations
- `BD-120` (HWM tests, `tests/high_water_marks.rs`): `high_water_marks::test_marks_follow_the_ledger_batch_by_batch_across_cells`, `high_water_marks::test_replays_are_skipped_and_gaps_and_forks_move_no_mark`, `high_water_marks::test_status_reports_behind_current_and_diverged`, `high_water_marks::test_a_restarted_projection_resumes_from_its_stores_marks`, `high_water_marks::test_a_failed_or_unrecorded_apply_moves_no_mark`, `high_water_marks::test_a_segment_on_disk_projects_like_the_l0_ledger`.
- `BD-121` (fsqlite drill projection, INT-FSQL-001, `src/drill/tests.rs`, `src/drill/boxes.rs`, `tests/drill_scale.rs`): `drill::tests::test_the_store_matches_the_reference_after_every_batch`, `drill::tests::test_a_deferred_rebuild_equals_the_incremental_store`, `drill::tests::test_rebuilding_the_indexes_drops_their_garbage_and_keeps_every_answer`, `drill::tests::test_replaying_a_prefix_gives_the_projection_as_of_it`, `drill::tests::test_a_store_on_disk_resumes_after_reopen`, `drill::tests::test_a_crash_at_any_write_leaves_a_batch_wholly_applied_or_absent`, `drill::tests::test_a_crash_in_an_index_rebuild_leaves_them_current_or_refusing`, `drill::tests::test_a_malformed_drill_record_is_refused_and_changes_nothing`, `drill::tests::test_a_store_of_another_schema_is_refused`, `drill::boxes::tests::test_every_point_of_a_curved_trace_lies_in_its_box`, `drill_scale::test_bbox_p99_over_ten_million_intervals` (ignored; release; `docs/delivery/INT-FSQL-001.md`).
