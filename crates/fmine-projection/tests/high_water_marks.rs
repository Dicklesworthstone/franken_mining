//! High-water-mark tests (`BD-120`'s done-when): a projection follows the ledger batch by batch, skips replays,
//! refuses gaps and forks without moving its mark, reports behind and diverged, resumes from its store's marks,
//! and reads the same from a durable segment as from L0.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use fmine_core::anchor::{FMT_ANCHOR_V1, LedgerAnchor};
use fmine_core::hash::Digest32;
use fmine_evidence::RefusalCode;
use fmine_ledger::batch::{DeltaBatch, DeltaKind, DeltaOp, FMT_LEDGER_V2, GENESIS_PREV_HASH};
use fmine_ledger::cell::CellLease;
use fmine_ledger::l0::L0Ledger;
use fmine_ledger::segment::{SegmentReader, SemanticSegmentWriter};
use fmine_projection::mark::{HighWaterMark, Status};
use fmine_projection::projection::{Fed, Projection, ProjectionError, Projector};

const A: &str = "CELL-SITE(a)";
const B: &str = "CELL-SITE(b)";

/// A projection of every cell's live keys, with the marks kept in the same store. `apply` builds the next state
/// and swaps it in, so it is both-or-neither. Fault switches stand in for a store that fails or misbehaves.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct KeyIndex {
    live: BTreeMap<(String, String), Digest32>,
    marks: BTreeMap<String, HighWaterMark>,
    fail_at: Option<(String, u64)>,
    forget_marks: bool,
    hide_marks: std::cell::Cell<bool>,
}

impl KeyIndex {
    /// The state a rebuild must reproduce: live keys and marks, not the fault switches.
    fn state(
        &self,
    ) -> (
        BTreeMap<(String, String), Digest32>,
        BTreeMap<String, HighWaterMark>,
    ) {
        (self.live.clone(), self.marks.clone())
    }
}

impl Projector for KeyIndex {
    type Error = String;

    fn apply(&mut self, batch: &DeltaBatch, mark: &HighWaterMark) -> Result<(), String> {
        if self.fail_at.as_ref() == Some(&(batch.cell_id.clone(), batch.seq)) {
            return Err(format!("disk full at {} {}", batch.cell_id, batch.seq));
        }
        let mut live = self.live.clone();
        for op in &batch.deltas {
            if matches!(op.kind, DeltaKind::Put | DeltaKind::Supersede) {
                live.insert((batch.cell_id.clone(), op.key.clone()), op.payload_digest);
            }
        }
        self.live = live;
        if !self.forget_marks {
            self.marks.insert(batch.cell_id.clone(), *mark);
        }
        Ok(())
    }

    fn stored_marks(&self) -> Result<BTreeMap<String, HighWaterMark>, String> {
        Ok(if self.hide_marks.get() {
            BTreeMap::new()
        } else {
            self.marks.clone()
        })
    }
}

struct Cells {
    ledger: L0Ledger,
    leases: BTreeMap<String, CellLease>,
}

impl Cells {
    fn new() -> Self {
        let mut ledger = L0Ledger::new();
        let mut leases = BTreeMap::new();
        for cell in [A, B] {
            leases.insert(
                cell.to_string(),
                ledger.create_cell(cell, "node-1").unwrap(),
            );
        }
        Self { ledger, leases }
    }

    /// Commit one batch to `cell` that puts `key` with `payload`.
    fn commit(&mut self, cell: &str, key: &str, payload: &str) -> Digest32 {
        self.commit_bytes(cell, key, payload.as_bytes().to_vec())
    }

    fn commit_bytes(&mut self, cell: &str, key: &str, payload: Vec<u8>) -> Digest32 {
        let lease = &self.leases[cell];
        let seq = self.ledger.committed_count(cell).unwrap();
        let prev_hash = self.ledger.head_hash(cell).unwrap();
        let batch = DeltaBatch {
            format_version: FMT_LEDGER_V2,
            cell_id: cell.to_string(),
            epoch: lease.epoch,
            seq,
            prev_hash,
            anchor: LedgerAnchor {
                format_version: FMT_ANCHOR_V1,
                cell_id: cell.to_string(),
                ledger_epoch: lease.epoch,
                commit_sequence: seq,
                adapter_registry_epoch: 1,
                schema_epoch: 1,
                policy_epoch: 1,
                privacy_epoch: 1,
                residency_epoch: 1,
                state_root: prev_hash,
            },
            deltas: vec![DeltaOp::new(key, DeltaKind::Put, payload)],
            witnesses: Vec::new(),
            cross_cell_refs: Vec::new(),
        };
        self.ledger.commit(lease, batch).unwrap()
    }

    fn batch(&self, cell: &str, seq: usize) -> DeltaBatch {
        self.ledger.batches(cell).unwrap()[seq].clone()
    }

    fn head(&self, cell: &str) -> HighWaterMark {
        HighWaterMark {
            applied: self.ledger.committed_count(cell).unwrap(),
            head: self.ledger.head_hash(cell).unwrap(),
        }
    }
}

/// Five batches in cell A, keys `lot/0` … `lot/4`.
fn five_in_a() -> Cells {
    let mut cells = Cells::new();
    for i in 0..5 {
        cells.commit(A, &format!("lot/{i}"), &format!("dry_t={}", 100 + i));
    }
    cells
}

fn projection() -> Projection<KeyIndex> {
    Projection::open(KeyIndex::default()).unwrap()
}

fn code(e: &ProjectionError) -> &'static str {
    e.to_refusal().code
}

#[test]
fn test_marks_follow_the_ledger_batch_by_batch_across_cells() {
    let mut cells = Cells::new();
    let mut p = projection();
    assert_eq!(p.mark(A), HighWaterMark::GENESIS);
    assert_eq!(p.mark(A).head, GENESIS_PREV_HASH);
    for (i, cell) in [A, B, A, A, B, A].into_iter().enumerate() {
        let digest = cells.commit(cell, &format!("lot/{i}"), &format!("dry_t={i}"));
        let seq = usize::try_from(cells.ledger.committed_count(cell).unwrap() - 1).unwrap();
        assert_eq!(
            p.status(&cells.ledger, cell).unwrap(),
            Status::Behind { by: 1 }
        );
        let fed = p.feed(&cells.batch(cell, seq)).unwrap();
        assert_eq!(fed, Fed::Applied(cells.head(cell)));
        assert_eq!(
            p.mark(cell).head,
            digest,
            "the mark's head is the batch's digest"
        );
        for c in [A, B] {
            assert_eq!(p.mark(c), cells.head(c));
            assert_eq!(p.status(&cells.ledger, c).unwrap(), Status::Current);
        }
        let answer = p.answer(|index| index.live.len());
        assert_eq!(answer.value, i + 1);
        assert_eq!(answer.anchor.mark(A), cells.head(A));
        assert_eq!(answer.anchor.mark(B), cells.head(B));
    }
    assert_eq!(p.mark(A).applied, 4);
    assert_eq!(p.mark(B).applied, 2);
}

#[test]
fn test_replays_are_skipped_and_gaps_and_forks_move_no_mark() {
    let cells = five_in_a();
    let mut p = projection();
    for seq in 0..3 {
        p.feed(&cells.batch(A, seq)).unwrap();
    }
    let at_three = p.mark(A);
    let state = p.projector().state();

    for seq in 0..3 {
        assert_eq!(
            p.feed(&cells.batch(A, seq)).unwrap(),
            Fed::AlreadyApplied,
            "replay of {seq}"
        );
    }
    let gap = p.feed(&cells.batch(A, 4)).unwrap_err();
    assert!(
        matches!(
            gap,
            ProjectionError::Gap {
                expected_seq: 3,
                seq: 4,
                ..
            }
        ),
        "{gap}"
    );
    assert_eq!(code(&gap), RefusalCode::StaleAnchor502.spec().code);

    // A second history of cell A: the same first three batches, then a different fourth.
    let mut fork = Cells::new();
    for i in 0..3 {
        fork.commit(A, &format!("lot/{i}"), &format!("dry_t={}", 100 + i));
    }
    fork.commit(A, "lot/3", "dry_t=999");
    fork.commit(A, "lot/4", "dry_t=104");
    assert_eq!(
        fork.ledger.prev_hash_at(A, 3).unwrap(),
        at_three.head,
        "same prefix"
    );
    assert_ne!(fork.batch(A, 3), cells.batch(A, 3));

    p.feed(&cells.batch(A, 3)).unwrap();
    let at_four = p.mark(A);
    let state_at_four = p.projector().state();
    for (what, batch) in [
        (
            "the fork's batch 3 where batch 3 is applied",
            fork.batch(A, 3),
        ),
        (
            "the fork's batch 4, which chains to its own batch 3",
            fork.batch(A, 4),
        ),
    ] {
        let err = p.feed(&batch).unwrap_err();
        assert!(
            matches!(err, ProjectionError::ChainBreak { .. }),
            "{what}: {err}"
        );
        assert_eq!(
            code(&err),
            RefusalCode::DigestMismatch505.spec().code,
            "{what}"
        );
        assert_eq!(p.mark(A), at_four, "{what}");
        assert_eq!(p.projector().state(), state_at_four, "{what}");
    }
    assert_ne!(state, state_at_four);
    assert_eq!(
        p.status(&cells.ledger, A).unwrap(),
        Status::Behind { by: 1 }
    );
}

#[test]
fn test_status_reports_behind_current_and_diverged() {
    let cells = five_in_a();
    let mut p = projection();
    p.catch_up(cells.ledger.batches(A).unwrap().get(..3).unwrap())
        .unwrap();
    assert_eq!(
        p.status(&cells.ledger, A).unwrap(),
        Status::Behind { by: 2 }
    );
    assert_eq!(
        p.status(&cells.ledger, B).unwrap(),
        Status::Current,
        "an empty cell"
    );
    assert_eq!(p.catch_up_with(&cells.ledger, A).unwrap(), 2);
    assert_eq!(p.status(&cells.ledger, A).unwrap(), Status::Current);
    assert_eq!(p.catch_up_with(&cells.ledger, A).unwrap(), 0);

    // A projection of another history of A is not on this ledger.
    let mut fork = Cells::new();
    fork.commit(A, "lot/0", "dry_t=100");
    fork.commit(A, "lot/1", "dry_t=555");
    let mut q = projection();
    q.catch_up_with(&fork.ledger, A).unwrap();
    assert_eq!(q.status(&cells.ledger, A).unwrap(), Status::Diverged);

    // Nor is one that has applied more batches than the ledger holds.
    let mut short = Cells::new();
    short.commit(A, "lot/0", "dry_t=100");
    assert_eq!(p.status(&short.ledger, A).unwrap(), Status::Diverged);

    // The same answers from a list of digests.
    let digests: Vec<Digest32> = cells
        .ledger
        .batches(A)
        .unwrap()
        .iter()
        .map(|b| b.batch_digest().unwrap())
        .collect();
    assert_eq!(p.status_in(A, &digests), Status::Current);
    assert_eq!(p.status_in(A, &digests[..4]), Status::Diverged);
    assert_eq!(q.status_in(A, &digests), Status::Diverged);
    let mut r = projection();
    r.feed(&cells.batch(A, 0)).unwrap();
    assert_eq!(r.status_in(A, &digests), Status::Behind { by: 4 });
    assert_eq!(projection().status_in(A, &[]), Status::Current);
    assert_eq!(
        projection().status_in(A, &digests),
        Status::Behind { by: 5 }
    );
}

#[test]
fn test_a_restarted_projection_resumes_from_its_stores_marks() {
    let cells = five_in_a();
    let mut full = projection();
    full.catch_up_with(&cells.ledger, A).unwrap();

    let mut p = projection();
    p.catch_up(cells.ledger.batches(A).unwrap().get(..2).unwrap())
        .unwrap();
    let persisted = p.into_projector();
    let mut reopened = Projection::open(persisted).unwrap();
    assert_eq!(reopened.mark(A).applied, 2);
    assert_eq!(
        reopened.status(&cells.ledger, A).unwrap(),
        Status::Behind { by: 3 }
    );
    assert_eq!(reopened.catch_up_with(&cells.ledger, A).unwrap(), 3);
    assert_eq!(reopened.projector().state(), full.projector().state());
    assert_eq!(reopened.anchor(), full.anchor());
}

#[test]
fn test_a_failed_or_unrecorded_apply_moves_no_mark() {
    let cells = five_in_a();
    let mut p = Projection::open(KeyIndex {
        fail_at: Some((A.to_string(), 2)),
        ..KeyIndex::default()
    })
    .unwrap();
    let err = p.catch_up_with(&cells.ledger, A).unwrap_err();
    assert!(matches!(err, ProjectionError::Store { .. }), "{err}");
    assert_eq!(
        code(&err),
        RefusalCode::InputMissingOrUnreadable506.spec().code
    );
    assert_eq!(p.mark(A).applied, 2, "batches 0 and 1 stay applied");
    assert_eq!(p.mark(A).head, cells.ledger.prev_hash_at(A, 2).unwrap());
    assert_eq!(p.projector().live.len(), 2);

    let mut store = p.into_projector();
    store.fail_at = None;
    let mut p = Projection::open(store).unwrap();
    assert_eq!(p.catch_up_with(&cells.ledger, A).unwrap(), 3);
    assert_eq!(p.status(&cells.ledger, A).unwrap(), Status::Current);

    // A store that applies the batch and does not keep the mark.
    let mut q = Projection::open(KeyIndex {
        forget_marks: true,
        ..KeyIndex::default()
    })
    .unwrap();
    let err = q.feed(&cells.batch(A, 0)).unwrap_err();
    assert!(
        matches!(err, ProjectionError::MarkNotRecorded { .. }),
        "{err}"
    );
    assert_eq!(code(&err), RefusalCode::DigestMismatch505.spec().code);
    assert_eq!(
        q.mark(A),
        HighWaterMark::GENESIS,
        "the anchor is what the store holds"
    );
}

fn scratch(tag: &str) -> PathBuf {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let dir = std::env::temp_dir().join(format!(
        "fmine-projection-{tag}-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir.join("cell-a.fmseg")
}

#[test]
fn test_a_segment_on_disk_projects_like_the_l0_ledger() {
    let cells = five_in_a();
    let path = scratch("segment");
    let mut writer = SemanticSegmentWriter::create(&path, A).unwrap();
    for batch in cells.ledger.batches(A).unwrap() {
        writer.append(batch).unwrap();
    }
    drop(writer);
    let inspection = SegmentReader::open(&path).unwrap();
    assert_eq!(inspection.batches.len(), 5);

    let mut from_l0 = projection();
    from_l0.catch_up_with(&cells.ledger, A).unwrap();
    let mut from_disk = projection();
    assert_eq!(
        from_disk.status_in(A, &inspection.batch_digests),
        Status::Behind { by: 5 }
    );
    from_disk.catch_up(&inspection.batches).unwrap();
    assert_eq!(
        from_disk.status_in(A, &inspection.batch_digests),
        Status::Current
    );
    assert_eq!(from_disk.status(&cells.ledger, A).unwrap(), Status::Current);
    assert_eq!(from_disk.projector().state(), from_l0.projector().state());
    assert_eq!(from_disk.mark(A).head, inspection.head_hash);
}

#[test]
fn test_current_answer_checks_every_cell_and_the_persisted_marks() {
    let mut cells = Cells::new();
    cells.commit(A, "lot/1", "dry_t=100");
    let mut p = Projection::rebuild(&cells.ledger, KeyIndex::default()).unwrap();
    assert_eq!(
        p.answer_current(&cells.ledger, |idx| idx.live.len())
            .unwrap()
            .value,
        1
    );

    cells.commit(B, "lot/2", "dry_t=200");
    assert_eq!(
        p.answer(|idx| idx.live.len()).value,
        1,
        "historical reads still work"
    );
    let error = p
        .answer_current::<()>(&cells.ledger, |_| panic!("must refuse before reading"))
        .unwrap_err();
    assert_eq!(
        error,
        ProjectionError::Behind {
            cell_id: B.into(),
            by: 1
        }
    );
    assert_eq!(code(&error), "FMINE-REF-502");
    assert_eq!(p.catch_up_with(&cells.ledger, B).unwrap(), 1);
    assert_eq!(
        p.answer_current(&cells.ledger, |idx| idx.live.len())
            .unwrap()
            .value,
        2
    );

    p.projector().hide_marks.set(true);
    let error = p
        .answer_current::<()>(&cells.ledger, |_| panic!("must refuse before reading"))
        .unwrap_err();
    assert_eq!(code(&error), "FMINE-REF-505");
}

#[test]
fn test_ledger_catch_up_refuses_a_nonprefix_even_without_a_suffix() {
    let cells = five_in_a();
    for applied in [5, 6] {
        let mut p = Projection::open(KeyIndex {
            marks: BTreeMap::from([(
                A.into(),
                HighWaterMark {
                    applied,
                    head: Digest32([42; 32]),
                },
            )]),
            ..KeyIndex::default()
        })
        .unwrap();
        assert_eq!(p.status(&cells.ledger, A).unwrap(), Status::Diverged);
        let old = p.mark(A);
        assert_eq!(
            code(&p.catch_up_with(&cells.ledger, A).unwrap_err()),
            "FMINE-REF-505"
        );
        assert_eq!(p.mark(A), old);
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn test_native_drill_projection_refuses_corruption_and_rebuilds_without_overwriting() {
    use fmine_drill::records::{
        Collar, DrillRecord, HoleType, Position, PositionMethod, encode_version,
    };
    use fmine_projection::drill::DrillStore;

    const KEY: &str = "drill/collar/H1";
    let record = |program: &str| {
        DrillRecord::Collar(Collar {
            hole_id: "H1".into(),
            hole_type: HoleType::Diamond,
            program: program.into(),
            planned: Some(Position {
                crs: "EPSG:32735".into(),
                easting_m: 421_000.0,
                northing_m: 8_652_100.0,
                elevation_m: 1_300.0,
                method: PositionMethod::Design,
                sigma_horizontal_m: 1.0,
                sigma_vertical_m: 1.0,
            }),
            actual: None,
        })
    };
    let expected = record("authority infill");
    let other = record("wrong history");
    let mut cells = Cells::new();
    cells.commit_bytes(A, KEY, encode_version(&expected, None).unwrap());
    cells.commit(B, "notes/1", "also required for a current pad");
    let mut fork = Cells::new();
    fork.commit_bytes(A, KEY, encode_version(&other, None).unwrap());

    let old_path = scratch("corrupt").with_extension("db");
    let new_dir = old_path.with_file_name("replacement");
    let mut wrong = Projection::open(DrillStore::open(&old_path).unwrap()).unwrap();
    wrong.catch_up_with(&fork.ledger, A).unwrap();
    drop(wrong);
    let mut wrong = Projection::open(DrillStore::open(&old_path).unwrap()).unwrap();
    assert_eq!(wrong.mark(A).applied, cells.head(A).applied);
    assert_eq!(
        code(&wrong.catch_up_with(&cells.ledger, A).unwrap_err()),
        "FMINE-REF-505"
    );
    let failed = wrong
        .answer_current::<()>(&cells.ledger, |_| panic!("must refuse before reading"))
        .unwrap_err();
    assert_eq!(code(&failed), "FMINE-REF-505");
    assert!(Projection::rebuild_drill_in(&cells.ledger, old_path.parent().unwrap()).is_err());

    let replacement = Projection::rebuild_drill_in(&cells.ledger, &new_dir).unwrap();
    drop(replacement);
    let rebuilt = Projection::open(DrillStore::open(new_dir.join("drill.db")).unwrap()).unwrap();
    let current = rebuilt
        .answer_current(&cells.ledger, |db| db.record(A, KEY))
        .unwrap();
    assert_eq!(current.value.unwrap().unwrap().version.record, expected);
    assert_eq!(current.anchor.mark(A), cells.head(A));
    assert_eq!(current.anchor.mark(B), cells.head(B));
    assert_eq!(
        wrong
            .projector()
            .record(A, KEY)
            .unwrap()
            .unwrap()
            .version
            .record,
        other
    );
    assert_eq!(wrong.mark(A), fork.head(A));
}
