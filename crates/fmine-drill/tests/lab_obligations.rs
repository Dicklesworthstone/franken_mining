//! `OB-LAB` replay tests (`BD-172`'s done-when: obligation replay). A drill cell's batches fold into lab
//! obligations with known transitions. The fold equals the replay of its own journal, the fold of the same
//! batches read back from a segment on disk, the fold built one batch at a time, and a durable obligation
//! registry reopened from disk. Closing the ingest region with a sample outstanding is refused.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use fmine_drill::lab::{LabBook, LabConflict, LabError, OWNER, obligation_id};
use fmine_drill::records::{self, AssayResult, DrillRecord, SampleLoss};
use fmine_drill::store::{Change, batch_for, commit, delta_ops, drill_law};
use fmine_evidence::RefusalCode;
use fmine_ledger::batch::DeltaBatch;
use fmine_ledger::l0::L0Ledger;
use fmine_ledger::segment::{SegmentReader, SemanticSegmentWriter};
use fmine_runtime::obligations::{
    DurableObligationRegistry, ObligationError, ObligationJournalEvent, ObligationKind,
    ObligationRegistry, ObligationState,
};

#[allow(dead_code)]
#[path = "common/fixtures.rs"]
mod fixtures;

use fixtures::{collar, dispatch, interval, loss, result, sample};

const CELL: &str = "CELL-SITE(lab-test)";
const LAB: &str = "LAB-A";
/// The fixtures' dispatch time.
const SENT: i64 = 1_790_000_400_000_000_000;
const DAY: i64 = 86_400_000_000_000;

fn put(record: DrillRecord) -> Change {
    Change::Put(record)
}

fn received(result_id: &str, sample_id: &str, dispatch_id: &str, days: i64) -> Change {
    put(DrillRecord::AssayResult(AssayResult {
        received_at_ns: SENT + days * DAY,
        ..result(result_id, sample_id, dispatch_id, LAB, None)
    }))
}

fn lost(loss_id: &str, dispatch_id: &str, sample_id: &str, days: i64) -> Change {
    put(DrillRecord::SampleLoss(SampleLoss {
        declared_at_ns: SENT + days * DAY,
        ..loss(loss_id, dispatch_id, sample_id)
    }))
}

/// `(dispatch, sample)` pairs.
type Pairs = Vec<(&'static str, &'static str)>;

/// The scenario, one batch per entry, with the samples outstanding after each.
fn scenario() -> Vec<(Vec<Change>, Pairs)> {
    let samples: Vec<Change> = ["S-0001", "S-0002", "S-0003", "S-0004"]
        .into_iter()
        .map(|s| put(DrillRecord::Sample(sample(s, &["H1-I1"]))))
        .collect();
    let mut first = vec![
        put(DrillRecord::Collar(collar("H1"))),
        put(DrillRecord::Interval(interval("H1-I1", "H1", 0, 1_500))),
    ];
    first.extend(samples);
    vec![
        (first, vec![]),
        (
            vec![put(DrillRecord::Dispatch(dispatch(
                "D-01",
                LAB,
                &["S-0001", "S-0002", "S-0003"],
            )))],
            vec![("D-01", "S-0001"), ("D-01", "S-0002"), ("D-01", "S-0003")],
        ),
        (
            vec![received("R-1", "S-0001", "D-01", 9)],
            vec![("D-01", "S-0002"), ("D-01", "S-0003")],
        ),
        (
            vec![put(DrillRecord::AssayResult(AssayResult {
                analyte: "Au".to_string(),
                received_at_ns: SENT + 12 * DAY,
                ..result("R-1-Au", "S-0001", "D-01", LAB, None)
            }))],
            vec![("D-01", "S-0002"), ("D-01", "S-0003")],
        ),
        (
            vec![lost("L-1", "D-01", "S-0002", 20)],
            vec![("D-01", "S-0003")],
        ),
        (
            vec![Change::Supersede {
                record: DrillRecord::Dispatch(dispatch(
                    "D-01",
                    LAB,
                    &["S-0001", "S-0002", "S-0003", "S-0004"],
                )),
                reason: "S-0004 was in the same box".to_string(),
            }],
            vec![("D-01", "S-0003"), ("D-01", "S-0004")],
        ),
        (
            // A re-dispatch and its result in one batch: a second obligation, opened and resolved.
            vec![
                put(DrillRecord::Dispatch(dispatch("D-02", LAB, &["S-0003"]))),
                received("R-3", "S-0003", "D-02", 30),
            ],
            vec![("D-01", "S-0003"), ("D-01", "S-0004")],
        ),
        (
            vec![
                lost("L-2", "D-01", "S-0001", 31),
                received("R-9", "S-0002", "D-01", 32),
                lost("L-3", "D-01", "S-0002", 33),
            ],
            vec![("D-01", "S-0003"), ("D-01", "S-0004")],
        ),
        (
            vec![
                lost("L-4", "D-01", "S-0003", 40),
                received("R-4", "S-0004", "D-01", 41),
            ],
            vec![],
        ),
    ]
}

struct Cell {
    ledger: L0Ledger,
    lease: fmine_ledger::cell::CellLease,
}

impl Cell {
    fn new() -> Self {
        let mut ledger = L0Ledger::new();
        let lease = ledger.create_cell(CELL, "node-1").unwrap();
        Self { ledger, lease }
    }

    fn write(&mut self, changes: &[Change]) {
        commit(&mut self.ledger, &self.lease, changes).unwrap_or_else(|e| panic!("{e}"));
    }

    fn batches(&self) -> &[DeltaBatch] {
        self.ledger.batches(CELL).unwrap()
    }
}

fn written() -> Cell {
    let mut cell = Cell::new();
    for (changes, _) in scenario() {
        cell.write(&changes);
    }
    cell
}

fn pairs(v: &[(&str, &str)]) -> Vec<(String, String)> {
    v.iter()
        .map(|(d, s)| ((*d).to_string(), (*s).to_string()))
        .collect()
}

fn scratch(tag: &str) -> PathBuf {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let dir = std::env::temp_dir().join(format!(
        "fmine-drill-lab-{tag}-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn test_each_dispatched_sample_resolves_to_a_result_or_a_declared_loss() {
    let mut cell = Cell::new();
    let mut book = LabBook::new();
    for (n, (changes, outstanding)) in scenario().into_iter().enumerate() {
        cell.write(&changes);
        book.apply(cell.batches().last().unwrap()).unwrap();
        assert_eq!(book.outstanding(), pairs(&outstanding), "after batch {n}");
    }
    let reg = book.registry();
    let get = |d: &str, s: &str| reg.get(&obligation_id(d, s)).unwrap().clone();

    let r1 = get("D-01", "S-0001");
    assert_eq!(
        (r1.kind, r1.owner_region, r1.state),
        (ObligationKind::ObLab, OWNER, ObligationState::Resolved)
    );
    assert_eq!(
        (r1.opened_at_ns, r1.closed_at_ns),
        (SENT, Some(SENT + 9 * DAY))
    );
    assert_eq!(
        r1.resolution_note.as_deref(),
        Some("result 'R-1' received"),
        "the first result closes it"
    );

    let l1 = get("D-01", "S-0002");
    assert_eq!(
        (l1.state, l1.closed_at_ns),
        (ObligationState::Aborted, Some(SENT + 20 * DAY))
    );
    assert_eq!(
        l1.resolution_note.as_deref(),
        Some("declared lost ('L-1'): bag split in transit; lab received no material (QA-114)")
    );
    assert_eq!(
        get("D-01", "S-0004").opened_at_ns,
        SENT,
        "a sample added by correction"
    );
    assert_eq!(
        get("D-02", "S-0003").state,
        ObligationState::Resolved,
        "the re-dispatch"
    );
    assert_eq!(
        get("D-01", "S-0003").state,
        ObligationState::Aborted,
        "the first dispatch of it, declared lost"
    );
    assert_eq!(reg.journal().len(), 5 + 5, "five opened, five closed");

    let conflict = |d: &str, s: &str, id: &str| (d.to_string(), s.to_string(), id.to_string());
    let seen: Vec<(String, String, String, &str)> = book
        .conflicts()
        .iter()
        .map(|c| match c {
            LabConflict::LossAfterResult {
                dispatch_id,
                sample_id,
                loss_id,
            } => (
                dispatch_id.clone(),
                sample_id.clone(),
                loss_id.clone(),
                "loss after result",
            ),
            LabConflict::RepeatedLoss {
                dispatch_id,
                sample_id,
                loss_id,
            } => (
                dispatch_id.clone(),
                sample_id.clone(),
                loss_id.clone(),
                "repeated loss",
            ),
            LabConflict::ResultAfterLoss {
                dispatch_id,
                sample_id,
                result_id,
            } => (
                dispatch_id.clone(),
                sample_id.clone(),
                result_id.clone(),
                "result after loss",
            ),
        })
        .collect();
    let expect = [
        (conflict("D-01", "S-0001", "L-2"), "loss after result"),
        (conflict("D-01", "S-0002", "L-3"), "repeated loss"),
        (conflict("D-01", "S-0002", "R-9"), "result after loss"),
    ];
    let expect: Vec<(String, String, String, &str)> = expect
        .into_iter()
        .map(|((d, s, id), what)| (d, s, id, what))
        .collect();
    assert_eq!(seen, expect);
    assert_eq!(
        get("D-01", "S-0001").state,
        ObligationState::Resolved,
        "a late loss does not reopen it"
    );
    assert_eq!(
        get("D-01", "S-0002").state,
        ObligationState::Aborted,
        "a late result does not undo the loss"
    );
}

#[test]
fn test_the_fold_replays_from_its_journal_from_disk_and_batch_by_batch() {
    let cell = written();
    let book = LabBook::from_batches(cell.batches()).unwrap();
    assert_eq!(book.applied(), cell.batches().len() as u64);

    // The journal replays to the same obligations.
    assert_eq!(
        &ObligationRegistry::replay(book.journal()).unwrap(),
        book.registry()
    );

    // The same batches, written through the law to a segment and read back, fold to the same book.
    let path = scratch("segment").join("drill.fmseg");
    let mut writer = SemanticSegmentWriter::create(&path, CELL).unwrap();
    for (changes, _) in scenario() {
        let batch = batch_for(writer.ledger(), CELL, 1, delta_ops(&changes).unwrap()).unwrap();
        writer.append_with_law(&batch, drill_law).unwrap();
    }
    drop(writer);
    let on_disk = SegmentReader::open(&path).unwrap();
    assert_eq!(LabBook::from_batches(&on_disk.batches).unwrap(), book);

    // One batch at a time, from any prefix, reaches the same book.
    for cut in 0..=cell.batches().len() {
        let mut partial = LabBook::from_batches(&cell.batches()[..cut]).unwrap();
        for batch in &cell.batches()[cut..] {
            partial.apply(batch).unwrap();
        }
        assert_eq!(partial, book, "resumed after {cut} batches");
    }
}

fn durable(
    path: &std::path::Path,
    journal: &[ObligationJournalEvent],
) -> DurableObligationRegistry {
    let mut reg = DurableObligationRegistry::create(path, "CELL-OB(lab-test)").unwrap();
    for event in journal {
        match event {
            ObligationJournalEvent::Opened {
                obligation_id,
                kind,
                owner_region,
                at_ns,
            } => {
                reg.open_obligation(obligation_id.clone(), *kind, *owner_region, *at_ns)
                    .unwrap();
            }
            ObligationJournalEvent::Resolved {
                obligation_id,
                at_ns,
                note,
            } => {
                reg.resolve(obligation_id, *at_ns, note.clone()).unwrap();
            }
            ObligationJournalEvent::Aborted {
                obligation_id,
                at_ns,
                reason,
            } => {
                reg.abort(obligation_id, *at_ns, reason.clone()).unwrap();
            }
        }
    }
    reg
}

#[test]
fn test_a_durable_registry_reopens_to_the_fold_and_guards_the_ingest_region() {
    let cell = written();
    let dir = scratch("durable");
    // Part way: after the correction that adds S-0004, two samples are outstanding.
    let partway = LabBook::from_batches(&cell.batches()[..6]).unwrap();
    assert_eq!(partway.outstanding().len(), 2);
    drop(durable(&dir.join("partway.fmseg"), partway.journal()));
    let reopened = DurableObligationRegistry::open(&dir.join("partway.fmseg")).unwrap();
    assert_eq!(reopened.registry(), partway.registry());
    let err = reopened.check_region_close(OWNER).unwrap_err();
    assert!(matches!(err, ObligationError::RegionLeak { .. }), "{err}");

    let book = LabBook::from_batches(cell.batches()).unwrap();
    drop(durable(&dir.join("full.fmseg"), book.journal()));
    let reopened = DurableObligationRegistry::open(&dir.join("full.fmseg")).unwrap();
    assert_eq!(reopened.registry(), book.registry());
    reopened.check_region_close(OWNER).unwrap();
}

#[test]
fn test_a_batch_out_of_order_of_another_cell_or_past_the_law_is_refused() {
    let cell = written();
    let batches = cell.batches();
    let err = LabBook::new().apply(&batches[2]).unwrap_err();
    assert!(
        matches!(
            err,
            LabError::OutOfOrder {
                expected_seq: 0,
                seq: 2
            }
        ),
        "{err}"
    );
    assert_eq!(
        err.to_refusal().code,
        RefusalCode::StaleAnchor502.spec().code
    );

    let mut book = LabBook::from_batches(&batches[..2]).unwrap();
    let mut other = batches[2].clone();
    other.cell_id = "CELL-SITE(elsewhere)".to_string();
    let err = book.apply(&other).unwrap_err();
    assert!(matches!(err, LabError::WrongCell { .. }), "{err}");
    assert_eq!(
        err.to_refusal().code,
        RefusalCode::OutsidePhysicalDomain109.spec().code
    );

    // A result for a sample no dispatch sent, admitted on a copy of the ledger without the law.
    let mut ledger = cell.ledger.clone();
    let stray = [received("R-77", "S-0004", "D-02", 50)];
    let batch = batch_for(&ledger, CELL, 1, delta_ops(&stray).unwrap()).unwrap();
    ledger
        .commit_with_conservation(&cell.lease, batch, |_, _| Ok(()))
        .unwrap();
    let all = ledger.batches(CELL).unwrap();
    let err = LabBook::from_batches(all).unwrap_err();
    assert!(
        matches!(err, LabError::Obligation(ObligationError::UnknownId { .. })),
        "{err}"
    );
    assert_eq!(
        err.to_refusal().code,
        RefusalCode::DigestMismatch505.spec().code
    );
    // The law refuses the same batch.
    let mut lawful = cell.ledger.clone();
    assert!(commit(&mut lawful, &cell.lease, &stray).is_err());
}
