//! The drill projection store against a single-threaded reference (INT-FSQL-001 criterion 1): seeded campaigns of
//! collars, surveys and intervals, committed to an L0 ledger under `drill_law`, with supersessions that move
//! collars, move surveys and intervals between holes and make holes undesurveyable and desurveyable again.
//!
//! The reference recomputes every box from the current records after each batch; the store updates only what a
//! batch touches. They must agree on every record, interval row, box (bit for bit), unlocated hole, R-tree row,
//! bbox answer and FTS5 answer. Crash injection aborts a child process at every write of an apply.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation
)]

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

use fmine_drill::records::{
    Alteration, AssayMethod, AssayValue, CertificateRef, Collar, CustodyEvent, Dispatch, HoleType,
    LabCertificate, Logger, Orientation, Position, PositionMethod, QaqcRegime, Recovery,
    RecoveryMethod, RunDirection, SampleType, Survey, SurveyInstrument,
};
use fmine_drill::store::{Change, commit};
use fmine_ledger::batch::DeltaOp;
use fmine_ledger::cell::CellLease;
use fmine_ledger::l0::L0Ledger;

use super::sharded::ShardedDrillStore;
use super::*;
use crate::projection::{Projection, Projector};

const CELL: &str = "CELL-SITE(drill-projection)";
const CRS: &str = "EPSG:32735";
const OTHER_CRS: &str = "EPSG:32736";
const HOLES: usize = 6;
const INTERVALS: usize = 36;
const STATIONS: usize = 4;

/// SplitMix64: a seeded stream for the campaign generator.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }

    fn unit(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1_u64 << 53) as f64
    }

    fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[self.below(items.len())]
    }
}

const LITHOLOGY: [&str; 4] = ["SCHIST", "GRANITE", "DIORITE", "SHALE"];
const MINERALIZATION: [&str; 4] = ["pyrite veins", "chalcopyrite 1%", "barren", "bornite blebs"];
const WEATHERING: [&str; 3] = ["fresh", "oxidised", "moderately weathered"];
const ALTERATION: [&str; 3] = ["sericite", "chlorite", "potassic"];
const INTENSITY: [&str; 3] = ["weak", "moderate", "strong"];

fn hole_id(h: usize) -> String {
    format!("H{h}")
}

fn position(h: usize, rng: &mut Rng, method: PositionMethod) -> Position {
    Position {
        crs: if h == HOLES - 1 { OTHER_CRS } else { CRS }.to_string(),
        easting_m: 421_000.0 + 40.0 * h as f64 + 5.0 * rng.unit(),
        northing_m: 8_652_100.25 + 25.0 * (h % 3) as f64 + 5.0 * rng.unit(),
        elevation_m: 1_300.0 + 3.0 * rng.unit(),
        method,
        sigma_horizontal_m: 0.05,
        sigma_vertical_m: 0.1,
    }
}

fn collar(h: usize, rng: &mut Rng) -> Collar {
    // Every third hole has only its planned position.
    let planned_only = h % 3 == 2;
    Collar {
        hole_id: hole_id(h),
        hole_type: HoleType::Diamond,
        program: "2026 infill".to_string(),
        planned: planned_only.then(|| position(h, rng, PositionMethod::Design)),
        actual: (!planned_only).then(|| position(h, rng, PositionMethod::DifferentialGnss)),
    }
}

fn survey(h: usize, k: usize, rng: &mut Rng) -> Survey {
    // Stations sit 25 m apart; one time in eight a station takes another's depth, which desurvey refuses.
    let slot = if rng.below(8) == 0 {
        rng.below(STATIONS)
    } else {
        k
    };
    Survey {
        survey_id: format!("{}-S{k}", hole_id(h)),
        hole_id: hole_id(h),
        depth_mm: 25_000 * slot as u64,
        instrument: SurveyInstrument::Gyro,
        orientation: Some(Orientation {
            azimuth_deg: (60.0 + 80.0 * rng.unit()).min(359.0),
            dip_deg: -85.0 + 40.0 * rng.unit(),
            sigma_azimuth_deg: 0.5,
            sigma_dip_deg: 0.25,
        }),
        magnetic_interference: false,
        run: RunDirection::Out,
    }
}

fn interval(i: usize, h: usize, rng: &mut Rng) -> Interval {
    let from_mm = 500 * rng.below(240) as u64;
    Interval {
        interval_id: format!("I{i:03}"),
        hole_id: hole_id(h),
        from_mm,
        to_mm: from_mm + 500 + 500 * rng.below(6) as u64,
        recovery: Some(Recovery {
            percent: 90.0 + 10.0 * rng.unit(),
            method: RecoveryMethod::Length,
        }),
        lithology: rng.pick(&LITHOLOGY).to_string(),
        alteration: (rng.below(3) != 0).then(|| Alteration {
            kind: rng.pick(&ALTERATION).to_string(),
            intensity: rng.pick(&INTENSITY).to_string(),
        }),
        mineralization: rng.pick(&MINERALIZATION).to_string(),
        weathering: rng.pick(&WEATHERING).to_string(),
        logger: Logger::Human {
            logger_id: "geo-07".to_string(),
        },
    }
}

/// A record as the reference holds it.
#[derive(Clone)]
struct Held {
    version: RecordVersion,
    versions: u64,
    seq: u64,
}

/// A seeded campaign in an L0 ledger, with the reference state after each batch.
struct Campaign {
    ledger: L0Ledger,
    states: Vec<BTreeMap<String, Held>>,
}

impl Campaign {
    fn new(seed: u64, batches: usize) -> Self {
        let mut rng = Rng(seed);
        let mut ledger = L0Ledger::new();
        let lease = ledger.create_cell(CELL, "node-1").unwrap();
        let mut held: BTreeMap<String, Held> = BTreeMap::new();
        let mut states = vec![held.clone()];
        let mut notes = 0;
        for seq in 0..batches as u64 {
            let changes = Self::changes(&mut rng, &held, seq);
            let mut staged: Vec<(String, Change)> = Vec::new();
            for change in changes {
                let record = match &change {
                    Change::Put(r) | Change::Supersede { record: r, .. } => r,
                };
                let key = record_key(record.kind(), record.id());
                if staged.iter().all(|(k, _)| *k != key) {
                    staged.push((key, change));
                }
            }
            let changes: Vec<Change> = staged.iter().map(|(_, c)| c.clone()).collect();
            commit_with_note(
                &mut ledger,
                &lease,
                &changes,
                (seq % 4 == 1).then(|| {
                    notes += 1;
                    format!("note/{notes}")
                }),
            );
            for (key, change) in staged {
                let (record, correction) = match change {
                    Change::Put(r) => (r, None),
                    Change::Supersede { record, reason } => (record, Some(reason)),
                };
                let versions = held.get(&key).map_or(0, |h| h.versions) + 1;
                held.insert(
                    key,
                    Held {
                        version: RecordVersion { record, correction },
                        versions,
                        seq,
                    },
                );
            }
            states.push(held.clone());
        }
        Self { ledger, states }
    }

    /// One batch's changes: collars first, then surveys and intervals on holes already collared, then
    /// supersessions of what is recorded.
    fn changes(rng: &mut Rng, held: &BTreeMap<String, Held>, seq: u64) -> Vec<Change> {
        let collared: Vec<usize> = (0..HOLES)
            .filter(|h| held.contains_key(&record_key(RecordKind::Collar, &hole_id(*h))))
            .collect();
        let mut out = Vec::new();
        if seq == 0 || (collared.len() < HOLES && rng.below(2) == 0) {
            for h in (0..HOLES)
                .filter(|h| !collared.contains(h))
                .take(1 + rng.below(3))
            {
                out.push(Change::Put(DrillRecord::Collar(collar(h, rng))));
            }
        }
        if collared.is_empty() {
            return out;
        }
        for _ in 0..1 + rng.below(6) {
            let h = *rng.pick(&collared);
            let fresh = match rng.below(10) {
                0..=2 => DrillRecord::Survey(survey(h, rng.below(STATIONS), rng)),
                3..=7 => DrillRecord::Interval(interval(rng.below(INTERVALS), h, rng)),
                8 => DrillRecord::Collar(collar(h, rng)),
                // Move a recorded survey or interval to hole `h`.
                _ => match held.values().map(|v| &v.version.record).find(|r| {
                    matches!(r, DrillRecord::Survey(_) | DrillRecord::Interval(_))
                        && rng.below(4) == 0
                }) {
                    Some(DrillRecord::Survey(s)) => {
                        let k = s.survey_id.rsplit('S').next().unwrap().parse().unwrap();
                        DrillRecord::Survey(Survey {
                            survey_id: s.survey_id.clone(),
                            ..survey(h, k, rng)
                        })
                    }
                    Some(DrillRecord::Interval(iv)) => DrillRecord::Interval(Interval {
                        hole_id: hole_id(h),
                        ..iv.clone()
                    }),
                    _ => continue,
                },
            };
            let key = record_key(fresh.kind(), fresh.id());
            out.push(if held.contains_key(&key) {
                Change::Supersede {
                    record: fresh,
                    reason: format!("relogged in batch {seq}"),
                }
            } else {
                Change::Put(fresh)
            });
        }
        out
    }

    fn batches(&self) -> &[DeltaBatch] {
        self.ledger.batches(CELL).unwrap()
    }
}

/// Commit `changes` under `drill_law`, plus a put of `note` (a key no drill projection reads) when given.
fn commit_with_note(
    ledger: &mut L0Ledger,
    lease: &CellLease,
    changes: &[Change],
    note: Option<String>,
) {
    let Some(note) = note else {
        commit(ledger, lease, changes).unwrap();
        return;
    };
    let mut deltas = fmine_drill::store::delta_ops(changes).unwrap();
    deltas.push(DeltaOp::new(
        note,
        DeltaKind::Put,
        b"not a drill record".to_vec(),
    ));
    deltas.sort();
    let batch = fmine_drill::store::batch_for(ledger, CELL, lease.epoch, deltas).unwrap();
    ledger
        .commit_with_conservation(lease, batch, fmine_drill::store::drill_law)
        .unwrap();
}

/// What the store must hold for `held`, computed from scratch.
struct Expected {
    records: BTreeMap<(String, String), StoredRecord>,
    intervals: Vec<IntervalRow>,
    unlocated: Vec<UnlocatedHole>,
    /// The collared holes.
    collared: Vec<(String, String)>,
    /// The records of every hole a collar, survey or interval names.
    holes: BTreeMap<String, HoleRecords>,
}

/// `hole`'s records by their definition: its collar, surveys and intervals; the samples whose first interval is one
/// of them; the results of those samples.
fn reference_hole_records<'a>(
    records: impl Iterator<Item = &'a DrillRecord> + Clone,
    hole: &str,
) -> HoleRecords {
    let mut out = HoleRecords::default();
    for record in records.clone() {
        match record {
            DrillRecord::Collar(c) if c.hole_id == hole => out.collar = Some(c.clone()),
            DrillRecord::Survey(s) if s.hole_id == hole => out.surveys.push(s.clone()),
            DrillRecord::Interval(iv) if iv.hole_id == hole => out.intervals.push(iv.clone()),
            _ => {}
        }
    }
    for record in records.clone() {
        if let DrillRecord::Sample(s) = record
            && s.interval_ids
                .first()
                .is_some_and(|first| out.intervals.iter().any(|iv| iv.interval_id == *first))
        {
            out.samples.push(s.clone());
        }
    }
    for record in records {
        if let DrillRecord::AssayResult(r) = record
            && out.samples.iter().any(|s| s.sample_id == r.sample_id)
        {
            out.results.push(r.clone());
        }
    }
    out.surveys.sort_by(|a, b| a.survey_id.cmp(&b.survey_id));
    out.intervals
        .sort_by(|a, b| a.interval_id.cmp(&b.interval_id));
    out.samples.sort_by(|a, b| a.sample_id.cmp(&b.sample_id));
    out.results.sort_by(|a, b| a.result_id.cmp(&b.result_id));
    out
}

fn expected(held: &BTreeMap<String, Held>) -> Expected {
    let records = held
        .iter()
        .map(|(key, h)| {
            (
                (CELL.to_string(), key.clone()),
                StoredRecord {
                    versions: h.versions,
                    seq: h.seq,
                    version: h.version.clone(),
                },
            )
        })
        .collect();
    let mut traces = BTreeMap::new();
    for h in held.values() {
        if let DrillRecord::Collar(c) = &h.version.record {
            let surveys: Vec<Survey> = held
                .values()
                .filter_map(|v| match &v.version.record {
                    DrillRecord::Survey(s) if s.hole_id == c.hole_id => Some(s.clone()),
                    _ => None,
                })
                .collect();
            traces.insert(
                c.hole_id.clone(),
                desurvey(c, &surveys).map_err(|e| e.to_string()),
            );
        }
    }
    let mut intervals = Vec::new();
    for h in held.values() {
        if let DrillRecord::Interval(iv) = &h.version.record {
            let located = match traces.get(&iv.hole_id) {
                Some(Ok(t)) => Some((t.crs().to_string(), interval_box(t, iv.from_mm, iv.to_mm))),
                _ => None,
            };
            intervals.push(IntervalRow {
                at: IntervalRef {
                    cell_id: CELL.to_string(),
                    interval_id: iv.interval_id.clone(),
                    hole_id: iv.hole_id.clone(),
                },
                from_mm: iv.from_mm,
                to_mm: iv.to_mm,
                body: body(iv),
                located,
            });
        }
    }
    intervals.sort_by(|a, b| a.at.cmp(&b.at));
    let unlocated = traces
        .into_iter()
        .filter_map(|(hole, t)| {
            t.err().map(|reason| UnlocatedHole {
                cell_id: CELL.to_string(),
                hole_id: hole,
                reason,
            })
        })
        .collect();
    let current = || held.values().map(|h| &h.version.record);
    let mut collared = Vec::new();
    let mut named = BTreeSet::new();
    for record in current() {
        match record {
            DrillRecord::Collar(c) => {
                collared.push((CELL.to_string(), c.hole_id.clone()));
                named.insert(c.hole_id.clone());
            }
            DrillRecord::Survey(s) => {
                named.insert(s.hole_id.clone());
            }
            DrillRecord::Interval(iv) => {
                named.insert(iv.hole_id.clone());
            }
            _ => {}
        }
    }
    collared.sort();
    let holes = named
        .into_iter()
        .map(|hole| {
            let records = reference_hole_records(current(), &hole);
            (hole, records)
        })
        .collect();
    Expected {
        records,
        intervals,
        unlocated,
        collared,
        holes,
    }
}

/// The R-tree's rows: id and stored box.
fn rtree_rows(store: &DrillStore) -> BTreeMap<i64, [f64; 6]> {
    let mut st = store
        .conn
        .prepare("SELECT id, min_e, max_e, min_n, max_n, min_z, max_z FROM drill_interval_box")
        .unwrap();
    st.query_map(params![], |r| {
        Ok((
            r.get::<_, i64>(0)?,
            [
                r.get(1)?,
                r.get(2)?,
                r.get(3)?,
                r.get(4)?,
                r.get(5)?,
                r.get(6)?,
            ],
        ))
    })
    .unwrap()
    .collect::<Result<BTreeMap<_, _>, _>>()
    .unwrap()
}

/// The interval's current R-tree id (`None` without a box) and text id.
fn ids_of(store: &DrillStore, interval_id: &str) -> (Option<i64>, i64) {
    store
        .conn
        .query_row(
            "SELECT bid, tid FROM drill_interval WHERE cell_id = ?1 AND interval_id = ?2",
            params![CELL, interval_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap()
}

/// The text index's rows: rowid and body.
fn text_rows(store: &DrillStore) -> BTreeMap<i64, String> {
    let mut st = store
        .conn
        .prepare("SELECT rowid, body FROM drill_interval_text")
        .unwrap();
    st.query_map(params![], |r| {
        Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?))
    })
    .unwrap()
    .collect::<Result<BTreeMap<_, _>, _>>()
    .unwrap()
}

/// The store holds exactly `want`. While the R-tree is current it holds a row covering each located box under the
/// interval's current R-tree id; while the text index is current it holds each interval's text under its current
/// text id; and every other row of a current index is counted as garbage.
/// The answers a whole store and a sharded one both give, so one set of checks covers both. Each method is the
/// store's own.
trait Answers {
    fn record_keys(&self) -> Result<Vec<(String, String)>, DrillStoreError>;
    fn record(&self, cell_id: &str, key: &str) -> Result<Option<StoredRecord>, DrillStoreError>;
    fn intervals(&self) -> Result<Vec<IntervalRow>, DrillStoreError>;
    fn unlocated_holes(&self) -> Result<Vec<UnlocatedHole>, DrillStoreError>;
    fn holes(&self) -> Result<Vec<(String, String)>, DrillStoreError>;
    fn hole_records(&self, cell_id: &str, hole_id: &str) -> Result<HoleRecords, DrillStoreError>;
    fn bbox(&self, crs: &str, query: &Box3) -> Result<Vec<IntervalRef>, DrillStoreError>;
    fn text(&self, query: &str) -> Result<Vec<IntervalRef>, DrillStoreError>;
}

macro_rules! answers {
    ($store:ty) => {
        impl Answers for $store {
            fn record_keys(&self) -> Result<Vec<(String, String)>, DrillStoreError> {
                <$store>::record_keys(self)
            }
            fn record(
                &self,
                cell_id: &str,
                key: &str,
            ) -> Result<Option<StoredRecord>, DrillStoreError> {
                <$store>::record(self, cell_id, key)
            }
            fn intervals(&self) -> Result<Vec<IntervalRow>, DrillStoreError> {
                <$store>::intervals(self)
            }
            fn unlocated_holes(&self) -> Result<Vec<UnlocatedHole>, DrillStoreError> {
                <$store>::unlocated_holes(self)
            }
            fn holes(&self) -> Result<Vec<(String, String)>, DrillStoreError> {
                <$store>::holes(self)
            }
            fn hole_records(
                &self,
                cell_id: &str,
                hole_id: &str,
            ) -> Result<HoleRecords, DrillStoreError> {
                <$store>::hole_records(self, cell_id, hole_id)
            }
            fn bbox(&self, crs: &str, query: &Box3) -> Result<Vec<IntervalRef>, DrillStoreError> {
                <$store>::bbox(self, crs, query)
            }
            fn text(&self, query: &str) -> Result<Vec<IntervalRef>, DrillStoreError> {
                <$store>::text(self, query)
            }
        }
    };
}
answers!(DrillStore);
answers!(ShardedDrillStore<'_>);

/// The store's records, interval rows, unlocated holes, hole listing and each hole's records are the reference's.
fn assert_answers(store: &impl Answers, want: &Expected, context: &str) {
    let keys = store.record_keys().unwrap();
    assert_eq!(
        keys,
        want.records.keys().cloned().collect::<Vec<_>>(),
        "{context}: record keys"
    );
    for ((cell, key), record) in &want.records {
        assert_eq!(
            store.record(cell, key).unwrap().as_ref(),
            Some(record),
            "{context}: record {key}"
        );
    }
    assert_eq!(
        store.intervals().unwrap(),
        want.intervals,
        "{context}: intervals"
    );
    assert_eq!(
        store.unlocated_holes().unwrap(),
        want.unlocated,
        "{context}: unlocated holes"
    );
    assert_eq!(store.holes().unwrap(), want.collared, "{context}: holes");
    for (hole, records) in &want.holes {
        assert_eq!(
            &store.hole_records(CELL, hole).unwrap(),
            records,
            "{context}: records of {hole}"
        );
    }
}

/// [`assert_answers`], and the index rows are those of the store's intervals plus the garbage it counts.
fn assert_holds(store: &DrillStore, want: &Expected, context: &str) {
    assert_answers(store, want, context);
    let states = store.index_states().unwrap();
    let garbage = store.index_garbage();
    if states.texts == IndexState::Current {
        let texts = text_rows(store);
        for iv in &want.intervals {
            let (_, tid) = ids_of(store, &iv.at.interval_id);
            assert_eq!(
                texts.get(&tid),
                Some(&iv.body),
                "{context}: text of {}",
                iv.at.interval_id
            );
        }
        assert_eq!(
            texts.len() as u64,
            want.intervals.len() as u64 + garbage.stale_texts,
            "{context}: text rows"
        );
    }
    if states.boxes != IndexState::Current {
        return;
    }
    let boxes = rtree_rows(store);
    let mut located = 0;
    for iv in &want.intervals {
        let (bid, _) = ids_of(store, &iv.at.interval_id);
        let Some((_, b)) = &iv.located else {
            assert_eq!(
                bid, None,
                "{context}: unlocated {} has an R-tree id",
                iv.at.interval_id
            );
            continue;
        };
        located += 1;
        let bid =
            bid.unwrap_or_else(|| panic!("{context}: {} has no R-tree id", iv.at.interval_id));
        let r = boxes
            .get(&bid)
            .unwrap_or_else(|| panic!("{context}: {} not in the R-tree", iv.at.interval_id));
        let (lo, hi) = (b.min(), b.max());
        for k in 0..3 {
            assert!(
                r[2 * k] <= lo[k] && hi[k] <= r[2 * k + 1],
                "{context}: {} R-tree row {r:?} inside {b:?}",
                iv.at.interval_id
            );
            assert!(
                lo[k] - r[2 * k] < 1.0 && r[2 * k + 1] - hi[k] < 1.0,
                "{context}: R-tree row {r:?} far from {b:?}"
            );
        }
    }
    assert_eq!(
        boxes.len() as u64,
        located + garbage.stale_boxes,
        "{context}: R-tree rows"
    );
}

/// The refusal of a query on `index` while it is stale.
const fn stale(index: Index) -> DrillStoreError {
    DrillStoreError::IndexStale {
        index,
        reason: None,
    }
}

/// Query boxes around the campaign: random ones, and for each located interval its own box and its upper corner
/// (touching counts).
fn queries(want: &Expected, rng: &mut Rng) -> Vec<(String, Box3)> {
    let mut out = Vec::new();
    for _ in 0..60 {
        let c = [
            421_000.0 + 260.0 * rng.unit(),
            8_652_090.0 + 80.0 * rng.unit(),
            1_180.0 + 130.0 * rng.unit(),
        ];
        let half = [
            0.2 + 30.0 * rng.unit(),
            0.2 + 30.0 * rng.unit(),
            0.2 + 30.0 * rng.unit(),
        ];
        let b = Box3::new(
            [c[0] - half[0], c[1] - half[1], c[2] - half[2]],
            [c[0] + half[0], c[1] + half[1], c[2] + half[2]],
        )
        .unwrap();
        out.push((
            if rng.below(5) == 0 { OTHER_CRS } else { CRS }.to_string(),
            b,
        ));
    }
    for iv in &want.intervals {
        if let Some((crs, b)) = &iv.located {
            out.push((crs.clone(), *b));
            out.push((crs.clone(), Box3::new(b.max(), b.max()).unwrap()));
        }
    }
    out
}

/// Every interval whose box meets `query` in `crs`: no phantom answers and no missing ones (criterion 4).
fn assert_bbox_answers(
    store: &impl Answers,
    want: &Expected,
    queries: &[(String, Box3)],
    context: &str,
) {
    for (crs, q) in queries {
        let brute: Vec<IntervalRef> = want
            .intervals
            .iter()
            .filter(|iv| {
                iv.located
                    .as_ref()
                    .is_some_and(|(c, b)| c == crs && b.meets(q))
            })
            .map(|iv| iv.at.clone())
            .collect();
        assert_eq!(
            store.bbox(crs, q).unwrap(),
            brute,
            "{context}: bbox {crs} {q:?}"
        );
    }
}

/// FTS5 tokens: runs of letters and digits, folded to lower case (the `unicode61` tokenizer on ASCII text).
fn tokens(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|t| !t.is_empty())
        .map(str::to_ascii_lowercase)
        .collect()
}

fn assert_text_answers(store: &impl Answers, want: &Expected, context: &str) {
    let mut vocabulary: Vec<String> = LITHOLOGY
        .iter()
        .chain(&MINERALIZATION)
        .chain(&WEATHERING)
        .chain(&ALTERATION)
        .chain(&INTENSITY)
        .flat_map(|t| tokens(t))
        .collect();
    vocabulary.sort();
    vocabulary.dedup();
    let matching = |hit: &dyn Fn(&[String]) -> bool| -> Vec<IntervalRef> {
        want.intervals
            .iter()
            .filter(|iv| hit(&tokens(&iv.body)))
            .map(|iv| iv.at.clone())
            .collect()
    };
    for word in &vocabulary {
        let want_hits = matching(&|t| t.contains(word));
        assert_eq!(
            store.text(word).unwrap(),
            want_hits,
            "{context}: text '{word}'"
        );
    }
    let phrase = matching(&|t| t.windows(2).any(|w| w[0] == "pyrite" && w[1] == "veins"));
    assert_eq!(
        store.text("\"pyrite veins\"").unwrap(),
        phrase,
        "{context}: phrase"
    );
    let prefix = matching(&|t| t.iter().any(|w| w.starts_with("gran")));
    assert_eq!(store.text("gran*").unwrap(), prefix, "{context}: prefix");
}

fn scratch(tag: &str) -> PathBuf {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let dir = std::env::temp_dir().join(format!(
        "fmine-projection-drill-{tag}-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir.join("drill.db")
}

/// After every batch of four seeded campaigns, the incrementally fed store holds what the reference computes from
/// scratch, answers every bbox query as brute force does, and answers text queries as a token scan does.
#[test]
fn test_the_store_matches_the_reference_after_every_batch() {
    let mut located_seen = (0, 0);
    for seed in [1, 2, 3, 4] {
        let campaign = Campaign::new(seed, 28);
        let mut projection = Projection::open(DrillStore::open_in_memory().unwrap()).unwrap();
        let mut rng = Rng(seed ^ 0xF00D);
        for (k, batch) in campaign.batches().iter().enumerate() {
            projection.feed(batch).unwrap();
            let want = expected(&campaign.states[k + 1]);
            let context = format!("seed {seed} batch {k}");
            let store = projection.projector();
            assert_holds(store, &want, &context);
            assert_bbox_answers(store, &want, &queries(&want, &mut rng), &context);
            assert_text_answers(store, &want, &context);
            located_seen.0 += want
                .intervals
                .iter()
                .filter(|iv| iv.located.is_some())
                .count();
            located_seen.1 += want
                .intervals
                .iter()
                .filter(|iv| iv.located.is_none())
                .count();
        }
        assert_eq!(
            projection.mark(CELL).applied,
            campaign.ledger.committed_count(CELL).unwrap()
        );
    }
    // The campaigns exercise both located and unlocated intervals.
    assert!(
        located_seen.0 > 100 && located_seen.1 > 20,
        "{located_seen:?}"
    );
}

/// The index rows that answer: the R-tree rows under current R-tree ids, the text rows under current text ids.
fn live_index_rows(store: &DrillStore) -> (BTreeMap<i64, [f64; 6]>, BTreeMap<i64, String>) {
    let mut st = store
        .conn
        .prepare("SELECT bid, tid FROM drill_interval")
        .unwrap();
    let ids = st
        .query_map(params![], |r| {
            Ok((r.get::<_, Option<i64>>(0)?, r.get::<_, i64>(1)?))
        })
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    let mut boxes = rtree_rows(store);
    let mut texts = text_rows(store);
    boxes.retain(|bid, _| ids.iter().any(|(b, _)| *b == Some(*bid)));
    texts.retain(|tid, _| ids.iter().any(|(_, t)| t == tid));
    (boxes, texts)
}

/// A rebuild that defers the indexes and loads them once ends where batch-by-batch feeding ends (criterion 5),
/// and keeps up batch by batch after that. While deferred, bbox and text refuse with `FMINE-REF-502`.
#[test]
fn test_a_deferred_rebuild_equals_the_incremental_store() {
    let campaign = Campaign::new(7, 30);
    let batches = campaign.batches();
    let mut incremental = Projection::open(DrillStore::open_in_memory().unwrap()).unwrap();
    incremental.catch_up(batches).unwrap();

    let mut store = DrillStore::open_in_memory().unwrap();
    store.defer_indexes(true);
    let mut rebuilt = Projection::open(store).unwrap();
    rebuilt.catch_up(&batches[..20]).unwrap();
    let any = Box3::new([0.0; 3], [1e7; 3]).unwrap();
    let refused = rebuilt.projector().bbox(CRS, &any).unwrap_err();
    assert_eq!(refused, stale(Index::Boxes));
    assert_eq!(refused.to_refusal().code, "FMINE-REF-502");
    assert_eq!(
        rebuilt.projector().text("schist").unwrap_err(),
        stale(Index::Texts)
    );
    assert!(!rebuilt.projector().indexes_are_current().unwrap());
    let mut store = rebuilt.into_projector();
    let states = store.rebuild_indexes().unwrap();
    assert_eq!(
        states,
        IndexStates {
            boxes: IndexState::Current,
            texts: IndexState::Current
        }
    );
    store.defer_indexes(false);
    assert_eq!(store.index_garbage(), IndexGarbage::default());
    let mut rebuilt = Projection::open(store).unwrap();
    assert_holds(
        rebuilt.projector(),
        &expected(&campaign.states[20]),
        "rebuilt at 20",
    );
    rebuilt.catch_up_with(&campaign.ledger, CELL).unwrap();

    let want = expected(&campaign.states[30]);
    let (a, b) = (incremental.projector(), rebuilt.projector());
    assert_holds(a, &want, "incremental");
    assert_holds(b, &want, "rebuilt then fed");
    assert_eq!(live_index_rows(a), live_index_rows(b));
    assert_eq!(a.stored_marks().unwrap(), b.stored_marks().unwrap());
    let qs = queries(&want, &mut Rng(70));
    assert_bbox_answers(b, &want, &qs, "rebuilt then fed");
    assert_text_answers(b, &want, "rebuilt then fed");
}

/// Supersessions leave index garbage that never answers; `rebuild_indexes` drops it and the answers stay the
/// same. A relog that changes only text keeps the R-tree row, and a move that keeps the text keeps the text row.
#[test]
fn test_rebuilding_the_indexes_drops_their_garbage_and_keeps_every_answer() {
    let campaign = Campaign::new(17, 40);
    let mut projection = Projection::open(DrillStore::open_in_memory().unwrap()).unwrap();
    projection.catch_up(campaign.batches()).unwrap();
    let want = expected(&campaign.states[40]);
    let garbage = projection.projector().index_garbage();
    assert!(
        garbage.stale_boxes > 10 && garbage.stale_texts > 10,
        "{garbage:?}"
    );
    let qs = queries(&want, &mut Rng(170));
    let before: Vec<_> = qs
        .iter()
        .map(|(crs, q)| projection.projector().bbox(crs, q).unwrap())
        .collect();
    let live = live_index_rows(projection.projector());
    let mut store = projection.into_projector();
    store.rebuild_indexes().unwrap();
    assert_eq!(store.index_garbage(), IndexGarbage::default());
    assert_holds(&store, &want, "compacted");
    assert_eq!(live_index_rows(&store), live);
    let after: Vec<_> = qs
        .iter()
        .map(|(crs, q)| store.bbox(crs, q).unwrap())
        .collect();
    assert_eq!(after, before);
    assert_text_answers(&store, &want, "compacted");

    // Each kind of relog leaves exactly the garbage it should: a text change one text row, a depth change one
    // box row, a relog that changes neither nothing. The superseded text never answers.
    let mut rng = Rng(3);
    let mut ledger = L0Ledger::new();
    let lease = ledger.create_cell(CELL, "node-1").unwrap();
    let iv = Interval {
        lithology: "SCHIST".to_string(),
        ..interval(0, 0, &mut rng)
    };
    commit(
        &mut ledger,
        &lease,
        &[
            Change::Put(DrillRecord::Collar(collar(0, &mut rng))),
            Change::Put(DrillRecord::Survey(Survey {
                depth_mm: 0,
                ..survey(0, 0, &mut rng)
            })),
            Change::Put(DrillRecord::Interval(iv.clone())),
        ],
    )
    .unwrap();
    let granite = Interval {
        lithology: "GRANITE".to_string(),
        ..iv.clone()
    };
    let deeper = Interval {
        from_mm: iv.from_mm + 10_000,
        to_mm: iv.to_mm + 10_000,
        ..granite.clone()
    };
    for (n, relog) in [granite, deeper.clone(), deeper].into_iter().enumerate() {
        commit(
            &mut ledger,
            &lease,
            &[Change::Supersede {
                record: DrillRecord::Interval(relog),
                reason: format!("relog {n}"),
            }],
        )
        .unwrap();
    }
    let mut projection = Projection::open(DrillStore::open_in_memory().unwrap()).unwrap();
    let mut seen = Vec::new();
    for batch in ledger.batches(CELL).unwrap() {
        projection.feed(batch).unwrap();
        let g = projection.projector().index_garbage();
        seen.push((g.stale_boxes, g.stale_texts));
    }
    assert_eq!(seen, [(0, 0), (0, 1), (1, 1), (1, 1)]);
    let store = projection.projector();
    assert!(store.text("schist").unwrap().is_empty());
    assert_eq!(store.text("granite").unwrap().len(), 1);
    let rows = store.intervals().unwrap();
    let (_, b) = rows[0].located.clone().unwrap();
    assert_eq!(store.bbox(CRS, &b).unwrap().len(), 1);
}

const LAB: &str = "LAB-A";
const SENT_NS: i64 = 1_767_571_200_000_000_000;

fn lab_sample(id: &str, interval_ids: &[&str]) -> Sample {
    Sample {
        sample_id: id.to_string(),
        interval_ids: interval_ids.iter().map(|i| (*i).to_string()).collect(),
        mass_g: Some(1_000.0),
        sample_type: SampleType::HalfCore,
        splitter: "core saw".to_string(),
        custody: vec![CustodyEvent {
            at_ns: SENT_NS,
            holder: "core shed".to_string(),
        }],
    }
}

fn lab_result(id: &str, sample_id: &str, dispatch_id: &str, certificate_id: &str) -> AssayResult {
    AssayResult {
        result_id: id.to_string(),
        sample_id: sample_id.to_string(),
        dispatch_id: dispatch_id.to_string(),
        analyte: "Au".to_string(),
        method: AssayMethod::FireAssayAas,
        value: AssayValue::Measured {
            mass_fraction: 1.0 / 524_288.0,
        },
        lab: LAB.to_string(),
        lab_batch: "B1".to_string(),
        certificate: CertificateRef::Certificate {
            certificate_id: certificate_id.to_string(),
        },
        qaqc_regime: QaqcRegime::Current,
        received_at_ns: SENT_NS + 86_400_000_000_000,
    }
}

fn lab_paperwork(dispatch_id: &str, certificate_id: &str, sample_ids: &[&str]) -> [DrillRecord; 2] {
    let original = fmine_core::hash::sha256(certificate_id.as_bytes());
    [
        DrillRecord::Dispatch(Dispatch {
            dispatch_id: dispatch_id.to_string(),
            lab: LAB.to_string(),
            sample_ids: sample_ids.iter().map(|s| (*s).to_string()).collect(),
            dispatched_at_ns: SENT_NS,
        }),
        DrillRecord::LabCertificate(LabCertificate {
            certificate_id: certificate_id.to_string(),
            lab: LAB.to_string(),
            original_sha256: original,
            cas_root: fmine_core::hash::sha256(&original.0),
            media_type: "text/csv".to_string(),
            issued_at_ns: SENT_NS + 86_400_000_000_000,
        }),
    ]
}

/// A hole's records follow their parents. Moving an interval takes the samples that start on it, and their results,
/// to its new hole; a sample relinked to an interval of another hole takes its results there. After each batch a
/// whole store and a store of one or three shards answer as the definition does for every hole, one never recorded
/// included, and the ids below say what moved.
#[test]
fn test_a_holes_records_follow_their_intervals_and_samples() {
    assert_lab_campaign(DrillStore::open_in_memory().unwrap(), "whole store");
    for shards in [1, 3] {
        let context = format!("{shards} shards");
        ShardedDrillStore::with_shards_in_memory(shards, |store| {
            assert_lab_campaign(store, &context);
        })
        .unwrap();
    }
}

/// The campaign of [`test_a_holes_records_follow_their_intervals_and_samples`], fed to `store`.
fn assert_lab_campaign<S: Projector + Answers>(store: S, context: &str) {
    let mut ledger = L0Ledger::new();
    let lease = ledger.create_cell(CELL, "node-1").unwrap();
    let mut rng = Rng(7);
    let mut iv = |id: &str, h: usize, from_mm: u64, to_mm: u64| Interval {
        interval_id: id.to_string(),
        from_mm,
        to_mm,
        ..interval(0, h, &mut rng)
    };
    let mut first: Vec<DrillRecord> = Vec::new();
    for h in 0..2 {
        let mut r = Rng(h as u64);
        first.push(DrillRecord::Collar(collar(h, &mut r)));
        first.push(DrillRecord::Survey(survey(h, 0, &mut r)));
    }
    first.extend([
        DrillRecord::Interval(iv("I1", 0, 0, 1_000)),
        DrillRecord::Interval(iv("I2", 0, 1_000, 2_500)),
        DrillRecord::Interval(iv("I3", 0, 2_500, 4_000)),
        DrillRecord::Interval(iv("I5", 1, 0, 1_000)),
        DrillRecord::Sample(lab_sample("P1", &["I1"])),
        DrillRecord::Sample(lab_sample("P2", &["I2", "I3"])),
        DrillRecord::Sample(lab_sample("P5", &["I5"])),
        DrillRecord::AssayResult(lab_result("R1", "P1", "D1", "C1")),
        DrillRecord::AssayResult(lab_result("R2", "P2", "D1", "C1")),
        DrillRecord::AssayResult(lab_result("R5", "P5", "D1", "C1")),
    ]);
    first.extend(lab_paperwork("D1", "C1", &["P1", "P2", "P5"]));
    let mut later = vec![DrillRecord::Interval(iv("I6", 0, 4_000, 5_000))];
    later.push(DrillRecord::Sample(lab_sample("P6", &["I6"])));
    later.push(DrillRecord::AssayResult(lab_result("R6", "P6", "D2", "C2")));
    later.extend(lab_paperwork("D2", "C2", &["P6"]));
    let moved = iv("I1", 1, 0, 1_000);
    let batches: Vec<Vec<Change>> = vec![
        first.into_iter().map(Change::Put).collect(),
        // I1 moves to H1, taking P1 and R1.
        vec![Change::Supersede {
            record: DrillRecord::Interval(moved),
            reason: "relogged onto H1".to_string(),
        }],
        // P2 is relinked to I5 on H1, taking R2, and gains a copper result. (A result cannot be corrected onto
        // another sample: `drill_law` refuses it.)
        vec![
            Change::Supersede {
                record: DrillRecord::Sample(lab_sample("P2", &["I5"])),
                reason: "sample sheet corrected".to_string(),
            },
            Change::Put(DrillRecord::AssayResult(AssayResult {
                analyte: "Cu".to_string(),
                ..lab_result("R7", "P2", "D1", "C1")
            })),
        ],
        later.into_iter().map(Change::Put).collect(),
    ];
    // (samples, results) of H0 and H1 after each batch.
    let ids = |r: &HoleRecords| {
        (
            r.samples
                .iter()
                .map(|s| s.sample_id.clone())
                .collect::<Vec<_>>(),
            r.results
                .iter()
                .map(|x| x.result_id.clone())
                .collect::<Vec<_>>(),
        )
    };
    let v = |s: &[&str]| s.iter().map(|x| (*x).to_string()).collect::<Vec<_>>();
    let stated = [
        [
            (v(&["P1", "P2"]), v(&["R1", "R2"])),
            (v(&["P5"]), v(&["R5"])),
        ],
        [
            (v(&["P2"]), v(&["R2"])),
            (v(&["P1", "P5"]), v(&["R1", "R5"])),
        ],
        [
            (v(&[]), v(&[])),
            (v(&["P1", "P2", "P5"]), v(&["R1", "R2", "R5", "R7"])),
        ],
        [
            (v(&["P6"]), v(&["R6"])),
            (v(&["P1", "P2", "P5"]), v(&["R1", "R2", "R5", "R7"])),
        ],
    ];
    let mut projection = Projection::open(store).unwrap();
    let mut held: BTreeMap<String, DrillRecord> = BTreeMap::new();
    for (k, changes) in batches.iter().enumerate() {
        commit(&mut ledger, &lease, changes).unwrap();
        projection.feed(&ledger.batches(CELL).unwrap()[k]).unwrap();
        for change in changes {
            let (Change::Put(r) | Change::Supersede { record: r, .. }) = change;
            held.insert(record_key(r.kind(), r.id()), r.clone());
        }
        let store = projection.projector();
        for hole in ["H0", "H1", "H9"] {
            assert_eq!(
                store.hole_records(CELL, hole).unwrap(),
                reference_hole_records(held.values(), hole),
                "{context} batch {k}: {hole}"
            );
        }
        for (h, want) in stated[k].iter().enumerate() {
            assert_eq!(
                &ids(&store.hole_records(CELL, &hole_id(h)).unwrap()),
                want,
                "{context} batch {k}: H{h}"
            );
        }
        assert_eq!(
            store.hole_records(CELL, "H9").unwrap(),
            HoleRecords::default()
        );
    }
}

/// Replaying the ledger to batch `k` gives the projection as of `k` (criterion 7's historical replay).
#[test]
fn test_replaying_a_prefix_gives_the_projection_as_of_it() {
    let campaign = Campaign::new(9, 16);
    let batches = campaign.batches();
    for k in [1, 5, 11, 16] {
        let mut projection = Projection::open(DrillStore::open_in_memory().unwrap()).unwrap();
        assert_eq!(projection.catch_up(&batches[..k]).unwrap(), k as u64);
        assert_eq!(projection.mark(CELL).applied, k as u64);
        assert_holds(
            projection.projector(),
            &expected(&campaign.states[k]),
            &format!("as of {k}"),
        );
    }
}

/// A store on disk closed half way resumes from its stored mark and ends where an in-memory store fed in one go
/// ends, row ids included.
#[test]
fn test_a_store_on_disk_resumes_after_reopen() {
    let campaign = Campaign::new(13, 24);
    let batches = campaign.batches();
    let path = scratch("reopen");
    let mut projection = Projection::open(DrillStore::open(&path).unwrap()).unwrap();
    projection.catch_up(&batches[..12]).unwrap();
    drop(projection.into_projector());

    let mut projection = Projection::open(DrillStore::open(&path).unwrap()).unwrap();
    assert_eq!(projection.mark(CELL).applied, 12);
    // Commits do not wait for the disk (NORMAL is 1); fsqlite takes an unknown pragma silently, so read it back.
    let synchronous = projection
        .projector()
        .conn
        .query_row("PRAGMA synchronous", params![], |r| r.get::<_, i64>(0))
        .unwrap();
    assert_eq!(synchronous, 1);
    assert_holds(
        projection.projector(),
        &expected(&campaign.states[12]),
        "reopened",
    );
    assert_eq!(
        projection.catch_up_with(&campaign.ledger, CELL).unwrap(),
        12
    );
    let want = expected(&campaign.states[24]);
    assert_holds(projection.projector(), &want, "resumed");
    let mut whole = Projection::open(DrillStore::open_in_memory().unwrap()).unwrap();
    whole.catch_up(batches).unwrap();
    assert_eq!(
        rtree_rows(projection.projector()),
        rtree_rows(whole.projector())
    );
}

const CRASH_ENV: &str = "FMINE_PROJECTION_DRILL_CRASH";
const CRASH_SEED: u64 = 11;
const CRASH_BATCHES: usize = 8;

/// The child side of the crash tests; without `FMINE_PROJECTION_DRILL_CRASH` it does nothing. With
/// `<path>;<k>;<n>` it feeds batches `0..k` to the store at `path`, then aborts the process at write `n` of
/// batch `k`, or, for `k` = [`CRASH_BATCHES`], at write `n` of `rebuild_indexes`.
#[test]
fn test_crash_child_entry_point() {
    let Ok(spec) = std::env::var(CRASH_ENV) else {
        return;
    };
    let parts: Vec<&str> = spec.split(';').collect();
    let [path, k, n] = parts.as_slice() else {
        panic!("bad {CRASH_ENV}: {spec}");
    };
    let (k, n): (usize, u64) = (k.parse().unwrap(), n.parse().unwrap());
    let campaign = Campaign::new(CRASH_SEED, CRASH_BATCHES);
    let batches = campaign.batches();
    let mut projection = Projection::open(DrillStore::open(path).unwrap()).unwrap();
    projection.catch_up(&batches[..k]).unwrap();
    if k == CRASH_BATCHES {
        let mut store = projection.into_projector();
        store.crash_after(Some(n));
        let rebuilt = store.rebuild_indexes();
        eprintln!("no crash at write {n} of the index rebuild: {rebuilt:?}");
    } else {
        projection.projector().crash_after(Some(n));
        let fed = projection.feed(&batches[k]);
        eprintln!("no crash at write {n} of batch {k}: {fed:?}");
    }
    // Reaching here means the crash point was never hit.
    std::process::exit(3);
}

/// Run the crash child for batch `k` (or the index rebuild) and write `n`; returns the store it left. The child
/// must die by a signal.
fn crash_child(k: usize, n: u64) -> PathBuf {
    let path = scratch("crash");
    let out = Command::new(std::env::current_exe().unwrap())
        .args([
            "drill::tests::test_crash_child_entry_point",
            "--exact",
            "--test-threads=1",
            "--nocapture",
        ])
        .env(CRASH_ENV, format!("{};{k};{n}", path.display()))
        .output()
        .unwrap();
    assert!(
        !out.status.success() && out.status.code().is_none(),
        "batch {k} write {n}: {:?}\n{}",
        out.status,
        String::from_utf8_lossy(&out.stderr)
    );
    path
}

/// A process killed at any write of `rebuild_indexes` leaves each index as it was, stale (its queries refuse until
/// a rebuild) or rebuilt; never current and wrong. A rebuild after the crash repairs them.
#[cfg(unix)]
#[test]
fn test_a_crash_in_an_index_rebuild_leaves_them_current_or_refusing() {
    let campaign = Campaign::new(CRASH_SEED, CRASH_BATCHES);
    let want = expected(&campaign.states[CRASH_BATCHES]);
    let mut dry = Projection::open(DrillStore::open_in_memory().unwrap()).unwrap();
    dry.catch_up(campaign.batches()).unwrap();
    let mut dry = dry.into_projector();
    dry.crash_after(None);
    dry.rebuild_indexes().unwrap();
    let writes = dry.writes();
    let qs = queries(&want, &mut Rng(110));
    let mut stale_seen = 0;
    for n in 1..=writes {
        let mut store = DrillStore::open(crash_child(CRASH_BATCHES, n)).unwrap();
        let context = format!("crash at write {n} of {writes} of the index rebuild");
        assert_holds(&store, &want, &context);
        let states = store.index_states().unwrap();
        if states.boxes == IndexState::Current {
            assert_bbox_answers(&store, &want, &qs, &context);
        } else {
            assert_eq!(states.boxes, IndexState::Stale, "{context}");
            assert_eq!(store.bbox(CRS, &qs[0].1).unwrap_err(), stale(Index::Boxes));
        }
        if states.texts == IndexState::Current {
            assert_text_answers(&store, &want, &context);
        } else {
            assert_eq!(states.texts, IndexState::Stale, "{context}");
            assert_eq!(store.text("schist").unwrap_err(), stale(Index::Texts));
        }
        if states.boxes == IndexState::Current && states.texts == IndexState::Stale {
            stale_seen += 1;
        }
        if n == writes {
            assert_eq!(store.index_garbage(), IndexGarbage::default(), "{context}");
        }
        store.rebuild_indexes().unwrap();
        assert_holds(&store, &want, &format!("{context}, rebuilt"));
        assert_bbox_answers(&store, &want, &qs, &format!("{context}, rebuilt"));
    }
    assert!(
        stale_seen > 0,
        "no crash point fell between the R-tree load and the text load"
    );
}

/// A process killed at any write of an apply, from `BEGIN` to just after `COMMIT`, leaves the store at the batch
/// before or, once `COMMIT` has returned, the batch itself: never part of one (criteria 3 and 8). The stored mark
/// says which, and feeding resumes from it.
#[cfg(unix)]
#[test]
fn test_a_crash_at_any_write_leaves_a_batch_wholly_applied_or_absent() {
    let campaign = Campaign::new(CRASH_SEED, CRASH_BATCHES);
    let batches = campaign.batches();
    let mut crashes = 0;
    for k in 0..CRASH_BATCHES {
        let mut dry = Projection::open(DrillStore::open_in_memory().unwrap()).unwrap();
        dry.catch_up(&batches[..k]).unwrap();
        dry.projector().crash_after(None);
        dry.feed(&batches[k]).unwrap();
        let writes = dry.projector().writes();
        assert!(writes >= 4, "batch {k}: {writes} writes");
        for n in 1..=writes {
            let path = crash_child(k, n);
            crashes += 1;
            let store = DrillStore::open(&path).unwrap();
            let applied = store
                .stored_marks()
                .unwrap()
                .get(CELL)
                .map_or(0, |m| m.applied);
            let at = if n == writes { k + 1 } else { k };
            assert_eq!(applied, at as u64, "batch {k} write {n} of {writes}");
            assert_holds(
                &store,
                &expected(&campaign.states[at]),
                &format!("crash in batch {k} at write {n}"),
            );
            let mut resumed = Projection::open(store).unwrap();
            resumed.catch_up_with(&campaign.ledger, CELL).unwrap();
            assert_holds(
                resumed.projector(),
                &expected(&campaign.states[CRASH_BATCHES]),
                "resumed after a crash",
            );
        }
    }
    assert!(crashes > 60, "{crashes} crash points");
}

/// A drill key whose payload is another record, or a key of no drill kind, is refused with `FMINE-REF-506`, and
/// the store and its mark stay at the batch before.
#[test]
fn test_a_malformed_drill_record_is_refused_and_changes_nothing() {
    let mut rng = Rng(5);
    let mut ledger = L0Ledger::new();
    let lease = ledger.create_cell(CELL, "node-1").unwrap();
    let first = collar(0, &mut rng);
    commit(
        &mut ledger,
        &lease,
        &[Change::Put(DrillRecord::Collar(first.clone()))],
    )
    .unwrap();
    let wrong = fmine_drill::records::encode_version(
        &DrillRecord::Interval(interval(1, 0, &mut rng)),
        None,
    )
    .unwrap();
    let bad_batches = [
        DeltaOp::new(
            record_key(RecordKind::Collar, "H9"),
            DeltaKind::Put,
            wrong.clone(),
        ),
        DeltaOp::new("drill/widget/1", DeltaKind::Put, wrong),
    ];
    let mut held = BTreeMap::new();
    held.insert(
        record_key(RecordKind::Collar, "H0"),
        Held {
            version: RecordVersion {
                record: DrillRecord::Collar(first),
                correction: None,
            },
            versions: 1,
            seq: 0,
        },
    );
    let want = expected(&held);
    for op in bad_batches {
        let mut ledger = ledger.clone();
        let batch = fmine_drill::store::batch_for(&ledger, CELL, lease.epoch, vec![op]).unwrap();
        ledger.commit(&lease, batch).unwrap();
        let batches = ledger.batches(CELL).unwrap();
        let mut projection = Projection::open(DrillStore::open_in_memory().unwrap()).unwrap();
        projection.feed(&batches[0]).unwrap();
        let err = projection.feed(&batches[1]).unwrap_err();
        assert_eq!(err.to_refusal().code, "FMINE-REF-506", "{err}");
        assert_eq!(projection.mark(CELL).applied, 1);
        assert_holds(projection.projector(), &want, &err.to_string());
    }
}

/// A store written under another schema is refused, not read.
#[test]
fn test_a_store_of_another_schema_is_refused() {
    let path = scratch("schema");
    let store = DrillStore::open(&path).unwrap();
    store
        .conn
        .execute(
            "UPDATE projection_meta SET value = 'fmine.projection.drill.v0' WHERE key = 'schema'",
            params![],
        )
        .unwrap();
    drop(store);
    let err = DrillStore::open(&path).unwrap_err();
    assert_eq!(
        err,
        DrillStoreError::Schema {
            found: "fmine.projection.drill.v0".to_string()
        }
    );
    assert_eq!(err.to_refusal().code, "FMINE-REF-506");
}

/// fsqlite 0.5.1's FTS5 fails some writes (module documentation). Here the engine itself fails them: intervals
/// whose only logging text is a 4,094-byte mineralization field repeating one word. A batch the engine cannot index
/// still applies, the R-tree keeps serving, and text queries refuse, naming the engine's failure, until a rebuild
/// loads the text index. A rebuild that meets the limit leaves the text index failed and the R-tree current. If
/// fsqlite lifts the limit, the `Failed` assertions fail, and the module documentation and INT-FSQL-001 must say so.
#[test]
fn test_a_text_index_the_engine_cannot_write_fails_alone() {
    let survey = Survey {
        survey_id: "H0-S0".to_string(),
        hole_id: hole_id(0),
        depth_mm: 0,
        instrument: SurveyInstrument::Gyro,
        orientation: Some(Orientation {
            azimuth_deg: 90.0,
            dip_deg: -60.0,
            sigma_azimuth_deg: 0.5,
            sigma_dip_deg: 0.25,
        }),
        magnetic_interference: false,
        run: RunDirection::Out,
    };
    let hole = [
        Change::Put(DrillRecord::Collar(collar(0, &mut Rng(5)))),
        Change::Put(DrillRecord::Survey(survey)),
    ];
    let short = |i: usize, mineralization: &str| {
        DrillRecord::Interval(Interval {
            interval_id: format!("L{i:03}"),
            hole_id: hole_id(0),
            from_mm: 1_000 * i as u64,
            to_mm: 1_000 * (i as u64 + 1),
            mineralization: mineralization.to_string(),
            ..interval(0, 0, &mut Rng(9))
        })
    };
    let long = |i: usize| {
        let DrillRecord::Interval(iv) = short(i, "pyrite ".repeat(585).trim_end()) else {
            unreachable!("short gives an interval")
        };
        Change::Put(DrillRecord::Interval(Interval {
            lithology: String::new(),
            weathering: String::new(),
            alteration: None,
            ..iv
        }))
    };
    let everywhere = Box3::new([0.0, 0.0, -1e4], [1e7, 1e7, 1e4]).unwrap();

    // Incremental: 100 short texts, then 400 long ones, then the long ones relogged short.
    let mut ledger = L0Ledger::new();
    let lease = ledger.create_cell(CELL, "node-1").unwrap();
    let mut first = hole.to_vec();
    first.extend((0..100).map(|i| Change::Put(short(i, "pyrite veins"))));
    commit(&mut ledger, &lease, &first).unwrap();
    commit(
        &mut ledger,
        &lease,
        &(100..500).map(long).collect::<Vec<_>>(),
    )
    .unwrap();
    let relogged: Vec<Change> = (100..500)
        .map(|i| Change::Supersede {
            record: short(i, "pyrite veins relogged"),
            reason: "relogged".to_string(),
        })
        .collect();
    commit(&mut ledger, &lease, &relogged).unwrap();
    let batches = ledger.batches(CELL).unwrap();

    let mut projection = Projection::open(DrillStore::open_in_memory().unwrap()).unwrap();
    projection.catch_up(&batches[..1]).unwrap();
    assert!(projection.projector().indexes_are_current().unwrap());
    assert_eq!(projection.projector().text("pyrite").unwrap().len(), 100);

    projection.catch_up(&batches[..2]).unwrap();
    let store = projection.projector();
    let states = store.index_states().unwrap();
    assert_eq!(states.boxes, IndexState::Current);
    let IndexState::Failed(why) = &states.texts else {
        panic!("the engine wrote the long texts: {states:?}");
    };
    assert!(
        why.starts_with(ADD_TEXT) && why.contains("no encodable segment leaves"),
        "{why}"
    );
    assert_eq!(store.intervals().unwrap().len(), 500);
    assert_eq!(store.bbox(CRS, &everywhere).unwrap().len(), 500);
    let refused = store.text("pyrite").unwrap_err();
    assert_eq!(
        refused,
        DrillStoreError::IndexStale {
            index: Index::Texts,
            reason: Some(format!("the engine failed to write it: {why}"))
        }
    );
    assert_eq!(refused.to_refusal().code, "FMINE-REF-502");
    assert!(refused.to_string().contains(why.as_str()), "{refused}");

    // Later batches apply without the text index, which stays failed until a rebuild loads it.
    projection.catch_up(batches).unwrap();
    let mut store = projection.into_projector();
    assert_eq!(store.index_states().unwrap().texts, states.texts);
    assert_eq!(store.bbox(CRS, &everywhere).unwrap().len(), 500);
    assert_eq!(
        store.rebuild_indexes().unwrap(),
        IndexStates {
            boxes: IndexState::Current,
            texts: IndexState::Current
        }
    );
    assert_eq!(store.text("pyrite").unwrap().len(), 500);
    assert_eq!(store.text("relogged").unwrap().len(), 400);

    // A rebuild from the ledger that meets the limit: the R-tree loads, the text index is left failed.
    let mut ledger = L0Ledger::new();
    let lease = ledger.create_cell(CELL, "node-1").unwrap();
    let mut only_long = hole.to_vec();
    only_long.extend((0..400).map(long));
    commit(&mut ledger, &lease, &only_long).unwrap();
    let mut store = DrillStore::open_in_memory().unwrap();
    store.defer_indexes(true);
    let mut projection = Projection::open(store).unwrap();
    projection.catch_up(ledger.batches(CELL).unwrap()).unwrap();
    let mut store = projection.into_projector();
    let states = store.rebuild_indexes().unwrap();
    assert_eq!(states.boxes, IndexState::Current);
    assert!(
        matches!(&states.texts, IndexState::Failed(w) if w.starts_with("load the text index")),
        "{states:?}"
    );
    assert_eq!(store.bbox(CRS, &everywhere).unwrap().len(), 400);
    assert!(matches!(
        store.text("pyrite"),
        Err(DrillStoreError::IndexStale {
            index: Index::Texts,
            reason: Some(_)
        })
    ));
}

/// A store that holds more intervals than its text index is kept for withholds it: at the batch that passes the
/// limit when fed incrementally, and in a rebuild of a deferred store, which then loads no text. The R-tree and
/// every other answer go on; text queries refuse and name INT-FSEARCH-001. A raised limit and a rebuild load it.
#[test]
fn test_a_text_index_past_its_limit_is_withheld_and_the_rest_goes_on() {
    const LIMIT: u64 = 12;
    let campaign = Campaign::new(5, 20);
    let batches = campaign.batches();
    let mut store = DrillStore::open_in_memory().unwrap();
    store.limit_text_index(LIMIT);
    let mut projection = Projection::open(store).unwrap();
    let mut rng = Rng(0xBEEF);
    let (mut withheld_at, mut frozen_garbage) = (None, None);
    for (k, batch) in batches.iter().enumerate() {
        projection.feed(batch).unwrap();
        let want = expected(&campaign.states[k + 1]);
        let context = format!("batch {k}");
        let store = projection.projector();
        assert_holds(store, &want, &context);
        assert_bbox_answers(store, &want, &queries(&want, &mut rng), &context);
        let held = store.intervals().unwrap().len() as u64;
        match store.index_states().unwrap().texts {
            IndexState::Current => {
                assert!(withheld_at.is_none() && held <= LIMIT, "{context}: {held}");
                assert_text_answers(store, &want, &context);
            }
            IndexState::Withheld(why) => {
                assert!(held > LIMIT, "{context}: {held}");
                withheld_at.get_or_insert(k);
                assert!(why.contains("INT-FSEARCH-001"), "{why}");
                // A withheld index takes no writes, so it gains no superseded rows.
                let garbage = store.index_garbage().stale_texts;
                assert_eq!(*frozen_garbage.get_or_insert(garbage), garbage, "{context}");
                let refused = store.text("pyrite").unwrap_err();
                assert_eq!(
                    refused,
                    DrillStoreError::IndexStale {
                        index: Index::Texts,
                        reason: Some(why.clone())
                    },
                );
                assert_eq!(refused.to_refusal().code, "FMINE-REF-502");
            }
            other => panic!("{context}: {other:?}"),
        }
    }
    let withheld_at = withheld_at.expect("the campaign never passed the limit");
    assert!(withheld_at > 0, "the limit was passed by the first batch");

    // A rebuild within the same limit loads no text; a raised limit and a rebuild load it.
    let mut store = projection.into_projector();
    let want = expected(&campaign.states[batches.len()]);
    let states = store.rebuild_indexes().unwrap();
    assert_eq!(states.boxes, IndexState::Current);
    assert!(
        matches!(states.texts, IndexState::Withheld(_)),
        "{states:?}"
    );
    assert!(text_rows(&store).is_empty());
    assert_eq!(store.index_garbage(), IndexGarbage::default());
    assert_holds(&store, &want, "rebuilt past the limit");
    store.limit_text_index(TEXT_INDEX_MAX_ROWS);
    assert_eq!(
        store.rebuild_indexes().unwrap(),
        IndexStates {
            boxes: IndexState::Current,
            texts: IndexState::Current
        }
    );
    assert_holds(&store, &want, "rebuilt within the limit");
    assert_text_answers(&store, &want, "rebuilt within the limit");

    // Deferred: the rebuild withholds the text index without loading it.
    let mut store = DrillStore::open_in_memory().unwrap();
    store.defer_indexes(true);
    store.limit_text_index(LIMIT);
    let mut projection = Projection::open(store).unwrap();
    projection.catch_up(batches).unwrap();
    let mut store = projection.into_projector();
    let states = store.rebuild_indexes().unwrap();
    assert_eq!(states.boxes, IndexState::Current);
    assert!(
        matches!(states.texts, IndexState::Withheld(_)),
        "{states:?}"
    );
    assert!(text_rows(&store).is_empty());
    assert_holds(&store, &want, "deferred, rebuilt past the limit");
    assert_bbox_answers(
        &store,
        &want,
        &queries(&want, &mut rng),
        "deferred, rebuilt past the limit",
    );
}

/// Two versions of a record in one batch, which a ledger committed without `drill_law` can hold: the store applies
/// them in order. Batch 1 adds an interval on one hole and relogs it onto another with new text, and adds a survey on
/// hole 0 and moves it to hole 1; batch 2 supersedes both, now stored, twice more. After each batch the store holds
/// the last versions with their version counts, boxes and texts, and (indexes kept current) the exact garbage.
#[test]
fn test_two_versions_of_a_record_in_one_batch_apply_in_order() {
    let mut rng = Rng(9);
    let mut ledger = L0Ledger::new();
    let lease = ledger.create_cell(CELL, "node-1").unwrap();
    let mut first = Vec::new();
    for h in 0..2 {
        first.push(DrillRecord::Collar(collar(h, &mut rng)));
        for k in 0..STATIONS {
            first.push(DrillRecord::Survey(Survey {
                depth_mm: 25_000 * k as u64,
                ..survey(h, k, &mut rng)
            }));
        }
    }
    let changes: Vec<Change> = first.iter().cloned().map(Change::Put).collect();
    commit(&mut ledger, &lease, &changes).unwrap();
    let mut held: BTreeMap<String, Held> = first
        .into_iter()
        .map(|record| {
            let key = record_key(record.kind(), record.id());
            let version = RecordVersion {
                record,
                correction: None,
            };
            (
                key,
                Held {
                    version,
                    versions: 1,
                    seq: 0,
                },
            )
        })
        .collect();

    let v1 = Interval {
        mineralization: "firstpass".to_string(),
        ..interval(3, 0, &mut rng)
    };
    let v2 = Interval {
        mineralization: "secondpass".to_string(),
        ..interval(3, 1, &mut rng)
    };
    assert_eq!(v1.interval_id, v2.interval_id);
    let reason = "relogged in the same batch";
    let station = Survey {
        depth_mm: 25_000 * STATIONS as u64,
        ..survey(0, STATIONS, &mut rng)
    };
    let moved = Survey {
        survey_id: station.survey_id.clone(),
        depth_mm: 25_000 * STATIONS as u64,
        ..survey(1, STATIONS, &mut rng)
    };
    let pairs = [
        (DrillRecord::Interval(v1), DrillRecord::Interval(v2.clone())),
        (
            DrillRecord::Survey(station),
            DrillRecord::Survey(moved.clone()),
        ),
    ];
    let mut ops = Vec::new();
    for (first, second) in &pairs {
        let key = record_key(first.kind(), first.id());
        ops.push(DeltaOp::new(
            key.clone(),
            DeltaKind::Supersede,
            fmine_drill::records::encode_version(second, Some(reason)).unwrap(),
        ));
        ops.push(DeltaOp::new(
            key,
            DeltaKind::Put,
            fmine_drill::records::encode_version(first, None).unwrap(),
        ));
    }
    let batch = fmine_drill::store::batch_for(&ledger, CELL, lease.epoch, ops).unwrap();
    for pair in batch.deltas.chunks(2) {
        assert_eq!(pair[0].key, pair[1].key);
        assert_eq!(
            (pair[0].kind, pair[1].kind),
            (DeltaKind::Put, DeltaKind::Supersede),
            "the put applies first"
        );
    }
    ledger.commit(&lease, batch).unwrap();
    for (_, second) in pairs {
        held.insert(
            record_key(second.kind(), second.id()),
            Held {
                version: RecordVersion {
                    record: second,
                    correction: Some(reason.to_string()),
                },
                versions: 2,
                seq: 1,
            },
        );
    }
    assert!(
        expected(&held).intervals[0].located.is_some(),
        "hole 1 has a trace"
    );
    check(
        &ledger,
        &held,
        &["firstpass"],
        "secondpass",
        IndexGarbage {
            stale_boxes: 0,
            stale_texts: 1,
        },
        &mut rng,
    );

    let relogs = [
        DrillRecord::Interval(Interval {
            mineralization: "thirdpass".to_string(),
            from_mm: v2.from_mm + 2_000,
            to_mm: v2.to_mm + 2_000,
            ..v2.clone()
        }),
        DrillRecord::Interval(Interval {
            mineralization: "fourthpass".to_string(),
            hole_id: hole_id(0),
            ..v2
        }),
        DrillRecord::Survey(Survey {
            hole_id: hole_id(0),
            ..moved.clone()
        }),
        DrillRecord::Survey(Survey {
            survey_id: moved.survey_id.clone(),
            depth_mm: moved.depth_mm,
            ..survey(1, STATIONS, &mut rng)
        }),
    ];
    let ops = relogs
        .iter()
        .enumerate()
        .map(|(i, record)| {
            let why = format!("relogged twice in batch 2, take {i}");
            DeltaOp::new(
                record_key(record.kind(), record.id()),
                DeltaKind::Supersede,
                fmine_drill::records::encode_version(record, Some(&why)).unwrap(),
            )
        })
        .collect();
    let batch = fmine_drill::store::batch_for(&ledger, CELL, lease.epoch, ops).unwrap();
    // The batch's order, not the order above, decides which version of each key is last.
    let mut last_word = "";
    for op in &batch.deltas {
        let version = fmine_drill::records::decode_version(&op.payload).unwrap();
        if let DrillRecord::Interval(iv) = &version.record {
            last_word = if iv.mineralization == "thirdpass" {
                "thirdpass"
            } else {
                "fourthpass"
            };
        }
        let h = held.get_mut(&op.key).unwrap();
        (h.version, h.versions, h.seq) = (version, h.versions + 1, 2);
    }
    ledger.commit(&lease, batch).unwrap();
    let other_word = if last_word == "thirdpass" {
        "fourthpass"
    } else {
        "thirdpass"
    };
    // Both relogs move the interval's box, and each supersedes a text.
    check(
        &ledger,
        &held,
        &["firstpass", "secondpass", other_word],
        last_word,
        IndexGarbage {
            stale_boxes: 1,
            stale_texts: 3,
        },
        &mut rng,
    );

    /// Feed `ledger`'s batches to new stores, indexes kept current and deferred, and check each holds `held`, finds
    /// `present` by text and none of `absent`, and carries `garbage` (none once a rebuild has run).
    fn check(
        ledger: &L0Ledger,
        held: &BTreeMap<String, Held>,
        absent: &[&str],
        present: &str,
        garbage: IndexGarbage,
        rng: &mut Rng,
    ) {
        let want = expected(held);
        assert_eq!(want.intervals.len(), 1);
        let batches = ledger.batches(CELL).unwrap();
        for defer in [false, true] {
            let context = format!("{} batches, indexes deferred: {defer}", batches.len());
            let mut store = DrillStore::open_in_memory().unwrap();
            store.defer_indexes(defer);
            let mut projection = Projection::open(store).unwrap();
            projection.catch_up(batches).unwrap();
            let mut store = projection.into_projector();
            if defer {
                store.rebuild_indexes().unwrap();
            }
            assert_holds(&store, &want, &context);
            assert_bbox_answers(&store, &want, &queries(&want, rng), &context);
            assert_text_answers(&store, &want, &context);
            for word in absent {
                assert!(store.text(word).unwrap().is_empty(), "{context}: {word}");
            }
            assert_eq!(store.text(present).unwrap().len(), 1, "{context}");
            let want_garbage = if defer {
                IndexGarbage::default()
            } else {
                garbage
            };
            assert_eq!(store.index_garbage(), want_garbage, "{context}");
        }
    }
}

/// After every batch of the seeded campaigns, a store of one shard and a store of three answer as the reference
/// does: records, interval rows, holes and their records, bbox answers against brute force and text answers against
/// a token scan. Each of the three shards holds intervals, so the answers are merged ones.
#[test]
fn test_a_sharded_store_matches_the_reference_after_every_batch() {
    for shards in [1, 3] {
        for seed in [1, 2, 3, 4] {
            let campaign = Campaign::new(seed, 28);
            ShardedDrillStore::with_shards_in_memory(shards, |store| {
                assert_eq!(store.shards(), shards);
                let mut projection = Projection::open(store).unwrap();
                let mut rng = Rng(seed ^ 0xF00D);
                for (k, batch) in campaign.batches().iter().enumerate() {
                    projection.feed(batch).unwrap();
                    let want = expected(&campaign.states[k + 1]);
                    let context = format!("{shards} shards, seed {seed} batch {k}");
                    let store = projection.projector();
                    assert_answers(store, &want, &context);
                    assert_bbox_answers(store, &want, &queries(&want, &mut rng), &context);
                    assert_text_answers(store, &want, &context);
                    assert!(store.indexes_are_current().unwrap(), "{context}");
                }
                let store = projection.projector();
                assert_eq!(
                    projection.mark(CELL).applied,
                    campaign.ledger.committed_count(CELL).unwrap()
                );
                for i in 0..shards {
                    assert_eq!(
                        store.shard_marks(i).unwrap(),
                        projection.anchor().marks,
                        "seed {seed}: shard {i}'s marks"
                    );
                    assert!(
                        store.shard_intervals(i).unwrap() > 0,
                        "seed {seed}: shard {i} of {shards} holds no interval"
                    );
                }
            })
            .unwrap();
        }
    }
}

/// A sharded store fed with its indexes deferred refuses bbox and text queries, and once every shard rebuilds them
/// it answers as the reference does and keeps up batch by batch after that.
#[test]
fn test_a_sharded_store_rebuilt_after_deferring_matches_the_reference() {
    let campaign = Campaign::new(7, 30);
    let batches = campaign.batches();
    ShardedDrillStore::with_shards_in_memory(3, |mut store| {
        store.defer_indexes(true).unwrap();
        let mut projection = Projection::open(store).unwrap();
        projection.catch_up(&batches[..20]).unwrap();
        let any = Box3::new([0.0; 3], [1e7; 3]).unwrap();
        let refused = projection.projector().bbox(CRS, &any).unwrap_err();
        assert_eq!(refused, stale(Index::Boxes));
        assert_eq!(refused.to_refusal().code, "FMINE-REF-502");
        assert_eq!(
            projection.projector().text("schist").unwrap_err(),
            stale(Index::Texts)
        );
        assert!(!projection.projector().indexes_are_current().unwrap());
        let mut store = projection.into_projector();
        let current = IndexStates {
            boxes: IndexState::Current,
            texts: IndexState::Current,
        };
        assert_eq!(store.rebuild_indexes().unwrap(), current);
        assert_eq!(store.index_garbage().unwrap(), IndexGarbage::default());
        store.defer_indexes(false).unwrap();
        let mut projection = Projection::open(store).unwrap();
        for at in [20, 30] {
            if at == 30 {
                projection.catch_up_with(&campaign.ledger, CELL).unwrap();
            }
            let want = expected(&campaign.states[at]);
            let context = format!("rebuilt at 20, fed to {at}");
            let store = projection.projector();
            assert_answers(store, &want, &context);
            assert_bbox_answers(store, &want, &queries(&want, &mut Rng(at as u64)), &context);
            assert_text_answers(store, &want, &context);
            assert_eq!(store.index_states().unwrap(), current, "{context}");
        }
    })
    .unwrap();
}

/// A batch that one shard refuses is written on none: the other shards write their share, then roll it back. Every
/// shard keeps its mark and the answers stay those before the batch.
#[test]
fn test_a_batch_one_shard_refuses_moves_no_shard() {
    const SHARDS: usize = 3;
    let mut rng = Rng(5);
    let mut ledger = L0Ledger::new();
    let lease = ledger.create_cell(CELL, "node-1").unwrap();
    let first = collar(0, &mut rng);
    commit(
        &mut ledger,
        &lease,
        &[Change::Put(DrillRecord::Collar(first.clone()))],
    )
    .unwrap();
    // A collar (every shard), an interval of a shard other than 0, and a key shard 0 cannot read.
    let iv = (0..INTERVALS)
        .map(|i| interval(i, 0, &mut rng))
        .find(|iv| {
            sharded::shard_of(&record_key(RecordKind::Interval, &iv.interval_id), SHARDS) != 0
        })
        .unwrap();
    let mut deltas = fmine_drill::store::delta_ops(&[
        Change::Put(DrillRecord::Collar(collar(1, &mut rng))),
        Change::Put(DrillRecord::Interval(iv)),
    ])
    .unwrap();
    deltas.push(DeltaOp::new(
        "drill/widget/1",
        DeltaKind::Put,
        b"not a drill record".to_vec(),
    ));
    deltas.sort();
    let batch = fmine_drill::store::batch_for(&ledger, CELL, lease.epoch, deltas).unwrap();
    ledger.commit(&lease, batch).unwrap();
    let batches = ledger.batches(CELL).unwrap();

    let mut held = BTreeMap::new();
    held.insert(
        record_key(RecordKind::Collar, "H0"),
        Held {
            version: RecordVersion {
                record: DrillRecord::Collar(first),
                correction: None,
            },
            versions: 1,
            seq: 0,
        },
    );
    let want = expected(&held);
    ShardedDrillStore::with_shards_in_memory(SHARDS, |store| {
        let mut projection = Projection::open(store).unwrap();
        projection.feed(&batches[0]).unwrap();
        let before: Vec<_> = (0..SHARDS)
            .map(|i| projection.projector().shard_marks(i).unwrap())
            .collect();
        let err = projection.feed(&batches[1]).unwrap_err();
        assert_eq!(err.to_refusal().code, "FMINE-REF-506", "{err}");
        assert!(err.to_string().contains("drill/widget/1"), "{err}");
        assert_eq!(projection.mark(CELL).applied, 1);
        for (i, marks) in before.iter().enumerate() {
            assert_eq!(
                &projection.projector().shard_marks(i).unwrap(),
                marks,
                "shard {i}"
            );
        }
        assert_answers(projection.projector(), &want, "after the refusal");
        assert!(projection.projector().indexes_are_current().unwrap());
    })
    .unwrap();
}

/// A stop between the shards' commits leaves one shard a batch ahead. Until the rest hold that batch, every answer
/// is refused with `FMINE-REF-502`; catch-up then applies it to the rest, the shard that holds it skips it, and the
/// store answers as the reference does. The same holds after the store is closed and reopened at that point.
#[test]
fn test_a_stop_between_shard_commits_is_finished_by_catch_up() {
    const SHARDS: usize = 3;
    const STOP: u64 = 6;
    let campaign = Campaign::new(3, 12);
    let batches = campaign.batches();
    let apart = DrillStoreError::ShardsApart {
        cell_id: CELL.to_string(),
        behind: STOP,
        ahead: STOP + 1,
    };
    assert_eq!(apart.to_refusal().code, "FMINE-REF-502");
    // Catch up from the stop, then check the answers and that every shard holds the store's marks.
    let finish = |mut projection: Projection<ShardedDrillStore<'_>>, context: &str| {
        let fed = projection.catch_up(batches).unwrap();
        assert_eq!(fed, batches.len() as u64 - STOP, "{context}");
        let want = expected(&campaign.states[batches.len()]);
        let store = projection.projector();
        assert_answers(store, &want, context);
        assert_bbox_answers(store, &want, &queries(&want, &mut Rng(12)), context);
        for i in 0..SHARDS {
            assert_eq!(
                store.shard_marks(i).unwrap(),
                projection.anchor().marks,
                "{context}: shard {i}"
            );
        }
    };
    for reopen in [false, true] {
        let dir = scratch("sharded-stop").with_file_name("shards");
        ShardedDrillStore::with_shards_at(&dir, SHARDS, |store| {
            let mut projection = Projection::open(store).unwrap();
            projection.catch_up(&batches[..STOP as usize]).unwrap();
            let mut store = projection.into_projector();
            store.commit_only(Some(1));
            let mut projection = Projection::open(store).unwrap();
            let err = projection.feed(&batches[STOP as usize]).unwrap_err();
            assert!(err.to_string().contains("stopped after 1 commits"), "{err}");
            assert_eq!(projection.mark(CELL).applied, STOP);
            let mut applied: Vec<u64> = (0..SHARDS)
                .map(|i| projection.projector().shard_marks(i).unwrap()[CELL].applied)
                .collect();
            applied.sort_unstable();
            assert_eq!(applied, [STOP, STOP, STOP + 1]);
            let store = projection.projector();
            assert_eq!(store.record_keys().unwrap_err(), apart);
            assert_eq!(store.intervals().unwrap_err(), apart);
            assert_eq!(store.hole_records(CELL, "H0").unwrap_err(), apart);
            let any = Box3::new([0.0; 3], [1e7; 3]).unwrap();
            assert_eq!(store.bbox(CRS, &any).unwrap_err(), apart);
            if !reopen {
                finish(projection, "not reopened");
            }
        })
        .unwrap();
        if reopen {
            ShardedDrillStore::with_shards_at(&dir, SHARDS, |store| {
                let projection = Projection::open(store).unwrap();
                assert_eq!(projection.mark(CELL).applied, STOP);
                assert_eq!(projection.projector().holes().unwrap_err(), apart);
                finish(projection, "reopened");
            })
            .unwrap();
        }
    }
}

/// Shard `i` of `n` opens only as that shard: as a shard of another count, or as a whole store, it is refused, the
/// caller's work does not run and a refused open creates no shard. A whole store does not open as a shard, and a
/// store of no shards is refused. Opened as built, it answers as before.
#[test]
fn test_a_shard_is_opened_only_as_itself() {
    let campaign = Campaign::new(2, 4);
    let dir = scratch("sharded-count").with_file_name("shards");
    ShardedDrillStore::with_shards_at(&dir, 2, |store| {
        let mut projection = Projection::open(store).unwrap();
        projection.catch_up(campaign.batches()).unwrap();
    })
    .unwrap();
    for n in [1, 3] {
        let mut ran = false;
        let err = ShardedDrillStore::with_shards_at(&dir, n, |_| ran = true).unwrap_err();
        assert!(!ran);
        assert_eq!(
            err,
            DrillStoreError::Shard {
                found: Some("0/2".to_string()),
                expected: Some(format!("0/{n}")),
            }
        );
        assert_eq!(err.to_refusal().code, "FMINE-REF-506");
    }
    assert!(!dir.join("shard-2.db").exists());
    assert_eq!(
        DrillStore::open(dir.join("shard-1.db")).unwrap_err(),
        DrillStoreError::Shard {
            found: Some("1/2".to_string()),
            expected: None,
        }
    );
    let whole = scratch("sharded-whole").with_file_name("shards");
    std::fs::create_dir_all(&whole).unwrap();
    drop(DrillStore::open(whole.join("shard-0.db")).unwrap());
    assert_eq!(
        ShardedDrillStore::with_shards_at(&whole, 1, |_| ()).unwrap_err(),
        DrillStoreError::Shard {
            found: None,
            expected: Some("0/1".to_string()),
        }
    );
    assert!(
        ShardedDrillStore::with_shards_in_memory(0, |_| ())
            .unwrap_err()
            .to_string()
            .contains("at least one shard")
    );
    ShardedDrillStore::with_shards_at(&dir, 2, |store| {
        let projection = Projection::open(store).unwrap();
        assert_eq!(projection.mark(CELL).applied, 4);
        assert_answers(
            projection.projector(),
            &expected(&campaign.states[4]),
            "reopened",
        );
    })
    .unwrap();
}

/// `prepare` leaves a batch open that `finish` commits or rolls back whole: rolled back, the store and its marks
/// are as before; committed, they are the batch's. A second prepare, or a finish with nothing prepared, is refused.
#[test]
fn test_a_prepared_batch_commits_or_rolls_back_whole() {
    let campaign = Campaign::new(4, 3);
    let batches = campaign.batches();
    let mut projection = Projection::open(DrillStore::open_in_memory().unwrap()).unwrap();
    projection.feed(&batches[0]).unwrap();
    let mut store = projection.into_projector();
    let mark = HighWaterMark {
        applied: 2,
        head: batches[1].batch_digest().unwrap(),
    };
    let before = store.stored_marks().unwrap();
    store.prepare(&batches[1], &mark).unwrap();
    assert!(
        store
            .prepare(&batches[1], &mark)
            .unwrap_err()
            .to_string()
            .contains("another batch is prepared")
    );
    store.finish(false).unwrap();
    assert_eq!(store.stored_marks().unwrap(), before);
    assert_holds(&store, &expected(&campaign.states[1]), "rolled back");
    assert!(
        store
            .finish(false)
            .unwrap_err()
            .to_string()
            .contains("no batch is prepared")
    );
    store.prepare(&batches[1], &mark).unwrap();
    store.finish(true).unwrap();
    assert_eq!(store.stored_marks().unwrap()[CELL], mark);
    assert_holds(&store, &expected(&campaign.states[2]), "committed");
}
