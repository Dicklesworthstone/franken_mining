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

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

use fmine_drill::records::{
    Alteration, Collar, HoleType, Logger, Orientation, Position, PositionMethod, Recovery,
    RecoveryMethod, RunDirection, Survey, SurveyInstrument,
};
use fmine_drill::store::{Change, commit};
use fmine_ledger::batch::DeltaOp;
use fmine_ledger::cell::CellLease;
use fmine_ledger::l0::L0Ledger;

use super::*;
use crate::projection::Projection;

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
    Expected {
        records,
        intervals,
        unlocated,
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

/// The store holds exactly `want`. While its indexes are current, the R-tree holds a row covering each located
/// box under the interval's current R-tree id, the text index holds each interval's text under its current text
/// id, and every other index row is counted as garbage.
fn assert_holds(store: &DrillStore, want: &Expected, context: &str) {
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
    if !store.indexes_are_current().unwrap() {
        return;
    }
    let (boxes, texts) = (rtree_rows(store), text_rows(store));
    let garbage = store.index_garbage();
    let mut located = 0;
    for iv in &want.intervals {
        let (bid, tid) = ids_of(store, &iv.at.interval_id);
        assert_eq!(
            texts.get(&tid),
            Some(&iv.body),
            "{context}: text of {}",
            iv.at.interval_id
        );
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
    assert_eq!(
        texts.len() as u64,
        want.intervals.len() as u64 + garbage.stale_texts,
        "{context}: text rows"
    );
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
    store: &DrillStore,
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

fn assert_text_answers(store: &DrillStore, want: &Expected, context: &str) {
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
    assert_eq!(refused, DrillStoreError::IndexStale);
    assert_eq!(refused.to_refusal().code, "FMINE-REF-502");
    assert_eq!(
        rebuilt.projector().text("schist").unwrap_err(),
        DrillStoreError::IndexStale
    );
    assert!(!rebuilt.projector().indexes_are_current().unwrap());
    let mut store = rebuilt.into_projector();
    store.rebuild_indexes().unwrap();
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

/// A process killed at any write of `rebuild_indexes` leaves the indexes either as they were, or stale (bbox and
/// text refuse until a rebuild), or rebuilt; never current and wrong. A rebuild after the crash repairs them.
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
        if store.indexes_are_current().unwrap() {
            assert_bbox_answers(&store, &want, &qs, &context);
            assert_text_answers(&store, &want, &context);
        } else {
            stale_seen += 1;
            assert_eq!(
                store.bbox(CRS, &qs[0].1).unwrap_err(),
                DrillStoreError::IndexStale
            );
            assert_eq!(
                store.text("schist").unwrap_err(),
                DrillStoreError::IndexStale
            );
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
        "no crash point fell between the two transactions"
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
