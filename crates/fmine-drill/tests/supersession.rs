//! Supersession tests (`BD-163`'s done-when), run on both ledger rungs: the L0 writer path under a lease and the
//! durable segment writer, which is reopened from disk where a test says so.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use fmine_evidence::RefusalCode;
use fmine_ledger::batch::{DeltaKind, DeltaOp};
use fmine_ledger::cell::CellLease;
use fmine_ledger::l0::L0Ledger;
use fmine_ledger::segment::SemanticSegmentWriter;

use fmine_core::hash::Digest32;
use fmine_drill::records;
use fmine_drill::records::{
    Collar, DrillRecord, Position, PositionMethod, RecordKind, RecordVersion, encode_version,
};
use fmine_drill::store::{
    Change, StoreError, StoredVersion, as_of, batch_for, commit, current, delta_ops, drill_law,
    history, parse_key, record_key,
};

#[allow(dead_code)]
#[path = "common/fixtures.rs"]
mod fixtures;

use fixtures::{
    collar, crm, dispatch, duplicate, interval, linked_set, loss, qc_material, result, sample,
    survey,
};
use fmine_drill::records::{AssayResult, BlankStage, QcInsertion, QcKind, SampleLoss};

const CELL: &str = "CELL-SITE(drill-test)";
const EPOCH: u64 = 1;

fn scratch(tag: &str) -> PathBuf {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let dir = std::env::temp_dir().join(format!(
        "fmine-drill-{tag}-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir.join("drill.fmseg")
}

enum Rung {
    L0 {
        ledger: L0Ledger,
        lease: CellLease,
    },
    L1 {
        path: PathBuf,
        writer: Option<SemanticSegmentWriter>,
    },
}

impl Rung {
    fn both(tag: &str) -> [Self; 2] {
        let mut ledger = L0Ledger::new();
        let lease = ledger.create_cell(CELL, "node-1").unwrap();
        let path = scratch(tag);
        let writer = SemanticSegmentWriter::create(&path, CELL).unwrap();
        [
            Self::L0 { ledger, lease },
            Self::L1 {
                path,
                writer: Some(writer),
            },
        ]
    }

    const fn name(&self) -> &'static str {
        match self {
            Self::L0 { .. } => "L0",
            Self::L1 { .. } => "L1",
        }
    }

    fn ledger(&self) -> &L0Ledger {
        match self {
            Self::L0 { ledger, .. } => ledger,
            Self::L1 { writer, .. } => writer.as_ref().unwrap().ledger(),
        }
    }

    fn write(&mut self, changes: &[Change]) -> Result<Digest32, StoreError> {
        match self {
            Self::L0 { ledger, lease } => commit(ledger, lease, changes),
            Self::L1 { writer, .. } => {
                // The durable append is the caller's: this crate builds the batch and supplies the law.
                let w = writer.as_mut().unwrap();
                let batch = batch_for(w.ledger(), CELL, EPOCH, delta_ops(changes)?)?;
                Ok(w.append_with_law(&batch, drill_law)?)
            }
        }
    }

    /// A batch of raw operations, past `delta_ops`' checks, so that only the ledger and the law judge it.
    fn write_ops(&mut self, ops: Vec<DeltaOp>) -> Result<Digest32, StoreError> {
        match self {
            Self::L0 { ledger, lease } => {
                let batch = batch_for(ledger, CELL, lease.epoch, ops)?;
                Ok(ledger.commit_with_conservation(lease, batch, drill_law)?)
            }
            Self::L1 { writer, .. } => {
                let w = writer.as_mut().unwrap();
                let batch = batch_for(w.ledger(), CELL, EPOCH, ops)?;
                Ok(w.append_with_law(&batch, drill_law)?)
            }
        }
    }

    fn reopen(&mut self) {
        if let Self::L1 { path, writer } = self {
            drop(writer.take());
            *writer = Some(SemanticSegmentWriter::open_append(path).unwrap());
        }
    }

    /// What a refusal must leave alone: the head, and the segment's bytes.
    fn fingerprint(&self) -> (u64, Digest32, Option<Vec<u8>>) {
        let l = self.ledger();
        let bytes = match self {
            Self::L0 { .. } => None,
            Self::L1 { path, .. } => Some(std::fs::read(path).unwrap()),
        };
        (
            l.committed_count(CELL).unwrap(),
            l.head_hash(CELL).unwrap(),
            bytes,
        )
    }

    fn count(&self) -> u64 {
        self.ledger().committed_count(CELL).unwrap()
    }

    fn current(&self, kind: RecordKind, id: &str) -> Option<RecordVersion> {
        current(self.ledger(), CELL, kind, id).unwrap()
    }

    fn history(&self, kind: RecordKind, id: &str) -> Vec<StoredVersion> {
        history(self.ledger(), CELL, kind, id).unwrap()
    }
}

fn put(record: DrillRecord) -> Change {
    Change::Put(record)
}

fn supersede(record: DrillRecord, reason: &str) -> Change {
    Change::Supersede {
        record,
        reason: reason.to_string(),
    }
}

/// `result` is refused with `code`, and the rung is exactly as it was.
fn assert_refused(
    rung: &Rung,
    what: &str,
    before: &(u64, Digest32, Option<Vec<u8>>),
    result: Result<Digest32, StoreError>,
    code: RefusalCode,
) {
    let err = match result {
        Ok(_) => panic!("{} {what}: admitted", rung.name()),
        Err(e) => e,
    };
    assert_eq!(
        err.to_refusal().code,
        code.spec().code,
        "{} {what}: {err}",
        rung.name()
    );
    assert_eq!(
        &rung.fingerprint(),
        before,
        "{} {what}: state changed",
        rung.name()
    );
}

/// Negative control for a law refusal: on a copy of the L0 ledger, the same batch with a law that admits
/// everything commits, so the refusal was `drill_law`'s and not the ledger's.
fn admitted_without_the_law(rung: &Rung, what: &str, ops: Vec<DeltaOp>) {
    if let Rung::L0 { ledger, lease } = rung {
        let mut copy = ledger.clone();
        let batch = batch_for(&copy, CELL, lease.epoch, ops).unwrap();
        copy.commit_with_conservation(lease, batch, |_, _| Ok(()))
            .unwrap_or_else(|e| panic!("{what}: the ledger alone refuses it: {e}"));
    }
}

fn refuses(rung: &mut Rung, what: &str, changes: &[Change], code: RefusalCode) {
    let before = rung.fingerprint();
    let result = rung.write(changes);
    assert_refused(rung, what, &before, result, code);
    if code == RefusalCode::Conservation101 {
        admitted_without_the_law(rung, what, delta_ops(changes).unwrap());
    }
}

fn refuses_ops(rung: &mut Rung, what: &str, ops: Vec<DeltaOp>, code: RefusalCode) {
    let before = rung.fingerprint();
    let result = rung.write_ops(ops.clone());
    assert_refused(rung, what, &before, result, code);
    if code == RefusalCode::Conservation101 {
        admitted_without_the_law(rung, what, ops);
    }
}

fn resurveyed(east_shift_m: f64) -> Collar {
    let mut c = collar("H1");
    let actual = c.actual.as_mut().unwrap();
    *actual = Position {
        easting_m: actual.easting_m + east_shift_m,
        method: PositionMethod::TotalStation,
        sigma_horizontal_m: 0.01,
        ..actual.clone()
    };
    c
}

#[test]
fn test_corrections_supersede_and_every_version_stays_readable() {
    let mut heads = Vec::new();
    for mut rung in Rung::both("supersede") {
        for record in linked_set() {
            rung.write(&[put(record)])
                .expect("each record after the ones it names");
        }
        let before_fix = rung.count();
        let original = collar("H1");
        let first_fix = resurveyed(0.3);
        let mut second_fix = resurveyed(0.3);
        second_fix.program = "2026 infill, phase 2".to_string();
        rung.write(&[supersede(
            DrillRecord::Collar(first_fix.clone()),
            "resurveyed by total station on 2026-11-20",
        )])
        .expect("a correction of a recorded collar");
        rung.write(&[supersede(
            DrillRecord::Collar(second_fix.clone()),
            "program renamed by the site geologist",
        )])
        .expect("a second correction");

        for reopened in [false, true] {
            if reopened {
                rung.reopen();
            }
            let tag = format!("{} reopened={reopened}", rung.name());
            assert_eq!(
                rung.current(RecordKind::Collar, "H1"),
                Some(RecordVersion {
                    record: DrillRecord::Collar(second_fix.clone()),
                    correction: Some("program renamed by the site geologist".to_string()),
                }),
                "{tag}"
            );
            let versions = rung.history(RecordKind::Collar, "H1");
            let records: Vec<_> = versions.iter().map(|v| v.version.record.clone()).collect();
            assert_eq!(
                records,
                vec![
                    DrillRecord::Collar(original.clone()),
                    DrillRecord::Collar(first_fix.clone()),
                    DrillRecord::Collar(second_fix.clone()),
                ],
                "{tag}"
            );
            assert_eq!(versions[0].version.correction, None, "{tag}");
            assert!(versions.windows(2).all(|w| w[0].seq < w[1].seq), "{tag}");
            assert!(versions.iter().all(|v| v.epoch == EPOCH), "{tag}");
            // The first version's bytes are the ones written, untouched by the corrections.
            assert_eq!(
                rung.ledger().key_history(CELL, "drill/collar/H1").unwrap()[0].payload,
                encode_version(&DrillRecord::Collar(original.clone()), None).unwrap(),
                "{tag}"
            );
            let at = |count| as_of(rung.ledger(), CELL, count, RecordKind::Collar, "H1").unwrap();
            assert_eq!(at(0), None, "{tag}");
            assert_eq!(
                at(before_fix).map(|v| v.record),
                Some(DrillRecord::Collar(original.clone())),
                "{tag}"
            );
            assert_eq!(
                at(before_fix + 1).map(|v| v.record),
                Some(DrillRecord::Collar(first_fix.clone())),
                "{tag}"
            );
            assert_eq!(
                at(before_fix + 2).map(|v| v.record),
                Some(DrillRecord::Collar(second_fix.clone())),
                "{tag}"
            );
            assert_eq!(rung.current(RecordKind::Collar, "H2"), None, "{tag}");
            assert!(rung.history(RecordKind::Collar, "H2").is_empty(), "{tag}");
        }

        // What named the collar still names it, and new records may.
        assert_eq!(
            rung.current(RecordKind::Survey, "H1-S1").map(|v| v.record),
            Some(DrillRecord::Survey(survey("H1-S1", "H1", 30_000)))
        );
        rung.write(&[put(DrillRecord::Interval(interval(
            "H1-I3", "H1", 3_000, 4_200,
        )))])
        .expect("an interval of the corrected hole");
        heads.push(rung.fingerprint().1);
    }
    assert_eq!(heads[0], heads[1], "both rungs hold the same chain");
}

#[test]
fn test_an_edit_in_place_is_refused_and_changes_nothing() {
    for mut rung in Rung::both("edit") {
        rung.write(&linked_set().into_iter().map(put).collect::<Vec<_>>())
            .expect("the linked set in one batch");
        refuses(
            &mut rung,
            "a Put over a recorded collar",
            &[put(DrillRecord::Collar(resurveyed(0.3)))],
            RefusalCode::StaleAnchor502,
        );
        refuses(
            &mut rung,
            "a correction of an unrecorded collar",
            &[supersede(DrillRecord::Collar(collar("H2")), "typo")],
            RefusalCode::NoClaimInDomain201,
        );
        refuses(
            &mut rung,
            "two changes to one record in one batch",
            &[
                supersede(DrillRecord::Collar(resurveyed(0.1)), "first"),
                supersede(DrillRecord::Collar(resurveyed(0.2)), "second"),
            ],
            RefusalCode::OutsidePhysicalDomain109,
        );
        refuses(
            &mut rung,
            "a correction without a reason",
            &[supersede(DrillRecord::Collar(resurveyed(0.1)), " ")],
            RefusalCode::OutsidePhysicalDomain109,
        );
        // Past delta_ops: a Put and a Supersede of one new key in one batch.
        let key = record_key(RecordKind::Collar, "H2");
        refuses_ops(
            &mut rung,
            "two versions of one key in one batch",
            vec![
                DeltaOp::new(
                    &key,
                    DeltaKind::Put,
                    encode_version(&DrillRecord::Collar(collar("H2")), None).unwrap(),
                ),
                DeltaOp::new(
                    &key,
                    DeltaKind::Supersede,
                    encode_version(&DrillRecord::Collar(collar("H2")), Some("again")).unwrap(),
                ),
            ],
            RefusalCode::Conservation101,
        );
        assert_eq!(
            rung.history(RecordKind::Collar, "H1").len(),
            1,
            "{}",
            rung.name()
        );
    }
}

#[test]
fn test_the_law_refuses_dangling_and_inconsistent_records_and_writes_nothing() {
    use RefusalCode::Conservation101 as LAW;
    let mut heads = Vec::new();
    for mut rung in Rung::both("law") {
        rung.write(&linked_set().into_iter().map(put).collect::<Vec<_>>())
            .expect("the linked set in one batch");
        let r = &mut rung;
        refuses(
            r,
            "a survey of a hole with no collar",
            &[put(DrillRecord::Survey(survey("H9-S1", "H9", 0)))],
            LAW,
        );
        refuses(
            r,
            "an interval of a hole with no collar",
            &[put(DrillRecord::Interval(interval("H9-I1", "H9", 0, 10)))],
            LAW,
        );
        refuses(
            r,
            "a sample of an unrecorded interval",
            &[put(DrillRecord::Sample(sample(
                "S-0009",
                &["H1-I1", "H1-I9"],
            )))],
            LAW,
        );
        refuses(
            r,
            "a dispatch of an unrecorded sample",
            &[put(DrillRecord::Dispatch(dispatch(
                "D-09",
                "LAB-A",
                &["S-0009"],
            )))],
            LAW,
        );
        r.write(&[put(DrillRecord::Sample(sample("S-0003", &["H1-I2"])))])
            .expect("a third sample");
        refuses(
            r,
            "a result whose dispatch does not list its sample",
            &[put(DrillRecord::AssayResult(result(
                "R-2", "S-0003", "D-01", "LAB-A", None,
            )))],
            LAW,
        );
        refuses(
            r,
            "a result from another lab than the dispatch's",
            &[put(DrillRecord::AssayResult(result(
                "R-3", "S-0001", "D-01", "LAB-B", None,
            )))],
            LAW,
        );
        refuses(
            r,
            "a result citing an unrecorded certificate",
            &[put(DrillRecord::AssayResult(result(
                "R-4",
                "S-0001",
                "D-01",
                "LAB-A",
                Some("C-99"),
            )))],
            LAW,
        );
        refuses(
            r,
            "a duplicate of an unrecorded sample",
            &[put(DrillRecord::QcInsertion(duplicate(
                "Q-2", "S-0002", "S-0404",
            )))],
            LAW,
        );
        refuses(
            r,
            "a same-batch dispatch that does not list the result's sample",
            &[
                put(DrillRecord::Dispatch(dispatch(
                    "D-02",
                    "LAB-A",
                    &["S-0002"],
                ))),
                put(DrillRecord::AssayResult(result(
                    "R-5", "S-0003", "D-02", "LAB-A", None,
                ))),
            ],
            LAW,
        );
        // OB-LAB: a result arrives, or a loss is declared, after its dispatch left, for a sample it lists.
        refuses(
            r,
            "a result received before its dispatch left",
            &[put(DrillRecord::AssayResult(AssayResult {
                received_at_ns: 1_790_000_399_999_999_999,
                ..result("R-6", "S-0001", "D-01", "LAB-A", None)
            }))],
            LAW,
        );
        refuses(
            r,
            "a loss of a sample its dispatch does not list",
            &[put(DrillRecord::SampleLoss(loss("L-2", "D-01", "S-0003")))],
            LAW,
        );
        refuses(
            r,
            "a loss declared before its dispatch left",
            &[put(DrillRecord::SampleLoss(SampleLoss {
                declared_at_ns: 1_790_000_000_000_000_000,
                ..loss("L-2", "D-01", "S-0001")
            }))],
            LAW,
        );
        refuses(
            r,
            "a loss naming an unrecorded dispatch",
            &[put(DrillRecord::SampleLoss(loss("L-2", "D-77", "S-0001")))],
            LAW,
        );
        refuses(
            r,
            "a result correction that moves it to another sample",
            &[supersede(
                DrillRecord::AssayResult(result("R-1", "S-0002", "D-01", "LAB-A", Some("C-77"))),
                "wrong bag",
            )],
            LAW,
        );
        refuses(
            r,
            "a loss correction that moves it to another sample",
            &[supersede(
                DrillRecord::SampleLoss(loss("L-1", "D-01", "S-0001")),
                "wrong bag",
            )],
            LAW,
        );
        refuses(
            r,
            "a dispatch correction that drops a sample",
            &[supersede(
                DrillRecord::Dispatch(dispatch("D-01", "LAB-A", &["S-0001"])),
                "S-0002 never left",
            )],
            LAW,
        );
        refuses(
            r,
            "a dispatch correction that moves the lab",
            &[supersede(
                DrillRecord::Dispatch(dispatch("D-01", "LAB-B", &["S-0001", "S-0002"])),
                "wrong lab",
            )],
            LAW,
        );

        // CRMs and blanks are inserted QC material; duplicates and umpires check drilled samples.
        r.write(&[put(DrillRecord::Sample(qc_material("S-CRM1")))])
            .expect("a QC material slot, taken from no interval");
        refuses(
            r,
            "a CRM on a drilled sample",
            &[put(DrillRecord::QcInsertion(crm("Q-2", "S-0001")))],
            LAW,
        );
        refuses(
            r,
            "a blank on a drilled sample",
            &[put(DrillRecord::QcInsertion(QcInsertion {
                kind: QcKind::Blank {
                    stage: BlankStage::Coarse,
                },
                ..crm("Q-2", "S-0002")
            }))],
            LAW,
        );
        refuses(
            r,
            "a duplicate of QC material",
            &[put(DrillRecord::QcInsertion(duplicate(
                "Q-2", "S-0002", "S-CRM1",
            )))],
            LAW,
        );
        refuses(
            r,
            "a duplicate carried by a QC material slot",
            &[put(DrillRecord::QcInsertion(duplicate(
                "Q-2", "S-CRM1", "S-0001",
            )))],
            LAW,
        );
        r.write(&[put(DrillRecord::QcInsertion(crm("Q-2", "S-CRM1")))])
            .expect("a CRM on its QC material slot");
        refuses(
            r,
            "a QC material slot corrected into a drilled sample",
            &[supersede(
                DrillRecord::Sample(sample("S-CRM1", &["H1-I1"])),
                "it was core after all",
            )],
            LAW,
        );
        refuses(
            r,
            "a drilled sample corrected into QC material",
            &[supersede(
                DrillRecord::Sample(qc_material("S-0002")),
                "it was a blank",
            )],
            LAW,
        );

        // The same records in the order the law asks for are admitted.
        r.write(&[supersede(
            DrillRecord::Dispatch(dispatch("D-01", "LAB-A", &["S-0001", "S-0002", "S-0003"])),
            "S-0003 was in the same box",
        )])
        .expect("a dispatch correction that adds a sample");
        r.write(&[put(DrillRecord::AssayResult(result(
            "R-2", "S-0003", "D-01", "LAB-A", None,
        )))])
        .expect("now the dispatch lists it");
        r.write(&[
            put(DrillRecord::Dispatch(dispatch(
                "D-02",
                "LAB-A",
                &["S-0003"],
            ))),
            put(DrillRecord::AssayResult(result(
                "R-5", "S-0003", "D-02", "LAB-A", None,
            ))),
        ])
        .expect("a dispatch and its result in one batch");

        // Raw operations the Change API cannot form.
        let collar_bytes = encode_version(&DrillRecord::Collar(collar("H1")), None).unwrap();
        refuses_ops(
            r,
            "collar bytes under a survey key",
            vec![DeltaOp::new(
                "drill/survey/H1",
                DeltaKind::Put,
                collar_bytes.clone(),
            )],
            LAW,
        );
        refuses_ops(
            r,
            "a record under another id's key",
            vec![DeltaOp::new(
                "drill/collar/H1",
                DeltaKind::Supersede,
                encode_version(&DrillRecord::Collar(collar("H2")), Some("x")).unwrap(),
            )],
            LAW,
        );
        refuses_ops(
            r,
            "a correction without its reason",
            vec![DeltaOp::new(
                "drill/collar/H1",
                DeltaKind::Supersede,
                encode_version(&DrillRecord::Collar(resurveyed(0.5)), None).unwrap(),
            )],
            LAW,
        );
        refuses_ops(
            r,
            "a first version carrying a correction",
            vec![DeltaOp::new(
                "drill/collar/H2",
                DeltaKind::Put,
                encode_version(&DrillRecord::Collar(collar("H2")), Some("x")).unwrap(),
            )],
            LAW,
        );
        refuses_ops(
            r,
            "an obligation on a drill key",
            vec![DeltaOp::new(
                "drill/collar/H3",
                DeltaKind::ReserveObligation,
                Vec::new(),
            )],
            LAW,
        );
        refuses_ops(
            r,
            "an unknown drill kind",
            vec![DeltaOp::new(
                "drill/core/H1",
                DeltaKind::Put,
                collar_bytes.clone(),
            )],
            LAW,
        );
        refuses_ops(
            r,
            "bytes that are not a record",
            vec![DeltaOp::new(
                "drill/collar/H4",
                DeltaKind::Put,
                b"collar H4".to_vec(),
            )],
            LAW,
        );

        // Keys outside drill/ are not the law's.
        r.write_ops(vec![DeltaOp::new(
            "lot/001",
            DeltaKind::Put,
            b"dry_t=100".to_vec(),
        )])
        .expect("another record family in the same cell");
        heads.push(r.fingerprint().1);
    }
    assert_eq!(
        heads[0], heads[1],
        "both rungs admitted and refused the same batches"
    );
}

#[test]
fn test_records_written_one_per_batch_or_together_read_the_same() {
    let [mut together, _] = Rung::both("together");
    let [_, mut apart] = Rung::both("apart");
    together
        .write(&linked_set().into_iter().map(put).collect::<Vec<_>>())
        .expect("one batch");
    for record in linked_set() {
        apart.write(&[put(record)]).expect("one batch each");
    }
    apart.reopen();
    for record in linked_set() {
        let (kind, id) = (record.kind(), record.id().to_string());
        let want = Some(RecordVersion {
            record,
            correction: None,
        });
        assert_eq!(together.current(kind, &id), want);
        assert_eq!(apart.current(kind, &id), want);
    }
}

#[test]
fn keys_name_their_kind_and_id_and_nothing_else() {
    for &kind in RecordKind::ALL {
        let key = record_key(kind, "ID-1");
        assert_eq!(parse_key(&key), Some((kind, "ID-1")));
    }
    assert_eq!(
        record_key(RecordKind::AssayResult, "R-1"),
        "drill/result/R-1"
    );
    assert_eq!(parse_key("drill/core/H1"), None);
    assert_eq!(parse_key("drill/collar"), None);
    assert_eq!(parse_key("lot/001"), None);
}
