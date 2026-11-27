# CONTRACT.md — `fmine-projection`

## 1. Purpose
The projector framework (`BD-120`, plan §26.1). fsqlite, fgdb and frankensearch views are derived from the ledger: rebuildable, high-water-marked and never consulted as truth. A projector's store records each cell's high-water mark with the batch that moved it. A projection applies each cell's committed batches once, in order, and tells where it stands against the log.

- **Crate:** `fmine-projection`
- **Layer:** V
- **Subsystem:** Bedrock
- **LOC budget:** 20,000 planned / 23,000 ceiling
- **Allowed Module Families:** `["mark", "projection"]`

## 2. Inputs
- Committed `fmine_ledger::batch::DeltaBatch`es, from `L0Ledger::batches` or from a segment (`SegmentReader::open(..).batches`).
- A `projection::Projector`: the store being projected into.
- Dependencies: `fmine-core`, `fmine-evidence` and `fmine-ledger` (`V -> S0`, `V -> S1`). Zero external crates.

## 3. Outputs
- `mark::HighWaterMark` per cell (batches applied and the last one's digest) and `mark::Anchor`, the vector of them (FMT-ANCHOR-1's `(cell, seq, digest)` marks, plan §6.3).
- `mark::Stamped<T>`: an answer with the anchor it was read at.
- `mark::Status` of a cell against the ledger or a segment's digests: `Current`, `Behind { by }` or `Diverged`.

## 4. Invariants
- **Mark and state move together:** `Projector::apply` stores the batch's effect and the new mark both or neither; `Projection::open` starts from the marks the store holds (`high_water_marks::test_a_restarted_projection_resumes_from_its_stores_marks`).
- **In order, once:** batch `n` of a cell is applied only when `n` batches are applied and its `prev_hash` is the mark's head. Earlier batches are skipped, and the last applied one must match the mark's digest. Gaps and forks are refused, and the mark does not move (`high_water_marks::test_replays_are_skipped_and_gaps_and_forks_move_no_mark`).
- **The mark is the ledger's chain:** after batch `n` the mark's head is that batch's digest, the `prev_hash` of batch `n + 1` (`high_water_marks::test_marks_follow_the_ledger_batch_by_batch_across_cells`).
- **Checked marks:** after an apply the store must hold the new mark. After a failed apply the in-memory marks are reread from the store (`high_water_marks::test_a_failed_or_unrecorded_apply_moves_no_mark`).
- **Status:** a mark past the log, or with a head that is not the log's digest at that point, is `Diverged`; a prefix is `Behind`; the log's head is `Current` (`high_water_marks::test_status_reports_behind_current_and_diverged`).
- **Either rung:** a projection fed from a segment on disk reaches the same state and marks as one fed from L0 (`high_water_marks::test_a_segment_on_disk_projects_like_the_l0_ledger`).

## 5. Refusals
- `ProjectionError::Gap` → `FMINE-REF-502` (a batch past the next one).
- `ProjectionError::ChainBreak` and `ProjectionError::MarkNotRecorded` → `FMINE-REF-505` (a fork or a diverged projection; a store that did not keep its mark).
- `ProjectionError::Store` → `FMINE-REF-506` (the projector's store failed).
- `ProjectionError::Ledger` and `ProjectionError::Source` keep the ledger's codes.

## 6. Determinism class
`BitwisePortable` (plan §29.2): sequence numbers, digests and ordered maps only. A projector's own store states its own class.

## 7. Evidence class of outputs
Derived views; never evidence or authority (plan §26.1). An answer's anchor names the ledger state it reflects.

## 8. Validity domain
- One authority log per cell, as `fmine-ledger` commits it. Quarantined lower-epoch batches are not projected.
- Batches before the last applied one are skipped without a digest check. `status` compares the whole chain through the mark's head digest.

## 9. Oracles
- The ledger's own head hashes and `prev_hash_at`, and a segment's verified batch digests.
- An in-memory key-index projector in the tests, with switches for a failing store and for one that drops marks.

## 10. Known gaps
- No production projector yet: fsqlite tables, R-tree and FTS5 are `BD-121`; fgdb is the `INT-FGDB-001` spike (`BD-126`); frankensearch is `BD-262`.
- Discard-and-rebuild on `Diverged` and freshness-required refusals for queries are `BD-122` (F5). This crate reports the status they act on.
- Multi-segment logs: a projection catches up from one segment's batches at a time.

## 11. Public API
- `mark::{HighWaterMark, Anchor, Stamped, Status}`
- `projection::{Projector, Fed, ProjectionError, Projection}`

## 12. Proof obligations
- `BD-120` (HWM tests, `tests/high_water_marks.rs`): `high_water_marks::test_marks_follow_the_ledger_batch_by_batch_across_cells`, `high_water_marks::test_replays_are_skipped_and_gaps_and_forks_move_no_mark`, `high_water_marks::test_status_reports_behind_current_and_diverged`, `high_water_marks::test_a_restarted_projection_resumes_from_its_stores_marks`, `high_water_marks::test_a_failed_or_unrecorded_apply_moves_no_mark`, `high_water_marks::test_a_segment_on_disk_projects_like_the_l0_ledger`.
