//! INT-FSQL-001's scale clause (`BD-121`): bbox query p99 ≤ 50 ms over 10⁷ intervals.
//!
//! Ignored by default; run in release:
//!
//! ```text
//! cargo test --release -p fmine-projection --test drill_scale -- --ignored --nocapture
//! ```
//!
//! `FMINE_DRILL_SCALE_HOLES` sets the hole count (default 100 000, at 100 one-metre intervals each: 10⁷
//! intervals), `FMINE_DRILL_SCALE_DIR` the directory for the store (default the temp dir) and
//! `FMINE_DRILL_SCALE_OUT` a file for the measured numbers as JSON lines.
//!
//! The holes sit on a 50 m grid, each with three gyro stations that steepen it from −60° to −80°. Batches of 100
//! holes (10 400 records) are chained as a ledger would chain them and fed through `Projection::feed` with the
//! indexes deferred; `rebuild_indexes` then loads each in one statement. Then 2 000 queries of 25 m and 200 m
//! cubes at random places in the field, and two batches that relog 100 intervals with the indexes kept current.
//!
//! The text index is reported, not required: past `TEXT_INDEX_MAX_ROWS` (10⁵) intervals the store withholds it,
//! because fsqlite 0.5.1's FTS5 does not scale to a campaign (the store's module documentation). Where it is not
//! current, the relogs apply without it and the text query is checked to refuse.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::cast_precision_loss
)]

use std::io::Write as _;
use std::time::{Duration, Instant};

use fmine_core::anchor::{FMT_ANCHOR_V1, LedgerAnchor};
use fmine_drill::records::{
    Collar, DrillRecord, HoleType, Interval, Logger, Orientation, Position, PositionMethod,
    RunDirection, Survey, SurveyInstrument,
};
use fmine_drill::store::{Change, delta_ops};
use fmine_ledger::batch::{DeltaBatch, FMT_LEDGER_V2, GENESIS_PREV_HASH};
use fmine_projection::drill::boxes::Box3;
use fmine_projection::drill::{DrillStore, DrillStoreError, Index, IndexState};
use fmine_projection::projection::Projection;

const CELL: &str = "CELL-SITE(drill-scale)";
const CRS: &str = "EPSG:32735";
const INTERVALS_PER_HOLE: usize = 100;
const HOLES_PER_BATCH: usize = 100;
const SPACING_M: f64 = 50.0;

fn env_or<T: std::str::FromStr>(name: &str, default: T) -> T {
    std::env::var(name)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

fn grid_side(holes: usize) -> usize {
    (1..).find(|s| s * s >= holes).unwrap()
}

/// Hole `h`'s records, or with `relog` = `Some(shift_mm)` its intervals relogged `shift_mm` deeper.
fn hole_changes(h: usize, side: usize, relog: Option<u64>) -> Vec<Change> {
    let hole_id = format!("H{h:06}");
    let (col, row) = (h % side, h / side);
    let mut out = Vec::with_capacity(4 + INTERVALS_PER_HOLE);
    let shift = relog.unwrap_or(0);
    if relog.is_none() {
        out.push(Change::Put(DrillRecord::Collar(Collar {
            hole_id: hole_id.clone(),
            hole_type: HoleType::Diamond,
            program: "scale".to_string(),
            planned: None,
            actual: Some(Position {
                crs: CRS.to_string(),
                easting_m: 400_000.0 + SPACING_M * col as f64,
                northing_m: 8_640_000.0 + SPACING_M * row as f64,
                elevation_m: 1_300.0,
                method: PositionMethod::DifferentialGnss,
                sigma_horizontal_m: 0.05,
                sigma_vertical_m: 0.1,
            }),
        })));
        for (k, dip) in [-60.0, -70.0, -80.0].into_iter().enumerate() {
            out.push(Change::Put(DrillRecord::Survey(Survey {
                survey_id: format!("{hole_id}-S{k}"),
                hole_id: hole_id.clone(),
                depth_mm: 40_000 * k as u64,
                instrument: SurveyInstrument::Gyro,
                orientation: Some(Orientation {
                    azimuth_deg: 45.0 + 5.0 * k as f64,
                    dip_deg: dip,
                    sigma_azimuth_deg: 0.5,
                    sigma_dip_deg: 0.25,
                }),
                magnetic_interference: false,
                run: RunDirection::Out,
            })));
        }
    }
    for i in 0..INTERVALS_PER_HOLE {
        let interval = DrillRecord::Interval(Interval {
            interval_id: format!("{hole_id}-I{i:03}"),
            hole_id: hole_id.clone(),
            from_mm: shift + 1_000 * i as u64,
            to_mm: shift + 1_000 * (i as u64 + 1),
            recovery: None,
            lithology: if i % 7 == 0 { "GRANITE" } else { "SCHIST" }.to_string(),
            alteration: None,
            mineralization: if relog.is_some() {
                "pyrite veins relogged"
            } else {
                "pyrite veins"
            }
            .to_string(),
            weathering: "fresh".to_string(),
            logger: Logger::Human {
                logger_id: "geo-07".to_string(),
            },
        });
        out.push(if relog.is_some() {
            Change::Supersede {
                record: interval,
                reason: "relogged".to_string(),
            }
        } else {
            Change::Put(interval)
        });
    }
    out
}

fn chained(seq: u64, prev_hash: fmine_core::hash::Digest32, changes: &[Change]) -> DeltaBatch {
    DeltaBatch {
        format_version: FMT_LEDGER_V2,
        cell_id: CELL.to_string(),
        epoch: 1,
        seq,
        prev_hash,
        anchor: LedgerAnchor {
            format_version: FMT_ANCHOR_V1,
            cell_id: CELL.to_string(),
            ledger_epoch: 1,
            commit_sequence: seq,
            adapter_registry_epoch: 1,
            schema_epoch: 1,
            policy_epoch: 1,
            privacy_epoch: 1,
            residency_epoch: 1,
            state_root: prev_hash,
        },
        deltas: delta_ops(changes).unwrap(),
        witnesses: Vec::new(),
        cross_cell_refs: Vec::new(),
    }
}

/// The q-quantile of sorted `xs` (nearest rank).
fn quantile(xs: &[Duration], q: f64) -> Duration {
    let rank = ((q * xs.len() as f64).ceil() as usize).clamp(1, xs.len());
    xs[rank - 1]
}

fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1e3
}

fn rss_kib() -> u64 {
    let out = std::process::Command::new("ps")
        .args(["-o", "rss=", "-p", &std::process::id().to_string()])
        .output()
        .unwrap();
    String::from_utf8_lossy(&out.stdout)
        .trim()
        .parse()
        .unwrap_or(0)
}

#[test]
#[ignore = "scale run: 10^7 intervals, minutes in release; see the module documentation"]
fn test_bbox_p99_over_ten_million_intervals() {
    let holes: usize = env_or("FMINE_DRILL_SCALE_HOLES", 100_000);
    let dir = std::env::var("FMINE_DRILL_SCALE_DIR")
        .map_or_else(|_| std::env::temp_dir(), std::path::PathBuf::from)
        .join(format!("fmine-drill-scale-{holes}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut out: Box<dyn std::io::Write> = match std::env::var("FMINE_DRILL_SCALE_OUT") {
        Ok(p) => Box::new(
            std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(p)
                .unwrap(),
        ),
        Err(_) => Box::new(std::io::sink()),
    };
    let mut report = |line: String| {
        eprintln!("{line}");
        writeln!(out, "{line}").unwrap();
    };
    let side = grid_side(holes);

    let mut store = DrillStore::open(dir.join("drill.db")).unwrap();
    store.defer_indexes(true);
    let mut projection = Projection::open(store).unwrap();
    let (mut seq, mut prev) = (0_u64, GENESIS_PREV_HASH);
    let started = Instant::now();
    let mut slowest = Duration::ZERO;
    for first in (0..holes).step_by(HOLES_PER_BATCH) {
        let changes: Vec<Change> = (first..(first + HOLES_PER_BATCH).min(holes))
            .flat_map(|h| hole_changes(h, side, None))
            .collect();
        let batch = chained(seq, prev, &changes);
        let t = Instant::now();
        projection.feed(&batch).unwrap();
        slowest = slowest.max(t.elapsed());
        prev = batch.batch_digest().unwrap();
        seq += 1;
        if seq % 100 == 0 {
            eprintln!(
                "{} holes in {:.0} s, rss {} MiB",
                seq as usize * HOLES_PER_BATCH,
                started.elapsed().as_secs_f64(),
                rss_kib() / 1024
            );
        }
    }
    let ingest = started.elapsed();
    let intervals = holes * INTERVALS_PER_HOLE;
    report(format!(
        "{{\"phase\":\"ingest\",\"holes\":{holes},\"intervals\":{intervals},\"batches\":{seq},\"seconds\":{:.1},\
         \"intervals_per_second\":{:.0},\"slowest_batch_ms\":{:.1},\"rss_mib\":{}}}",
        ingest.as_secs_f64(),
        intervals as f64 / ingest.as_secs_f64(),
        ms(slowest),
        rss_kib() / 1024
    ));

    let mut store = projection.into_projector();
    let t = Instant::now();
    let states = store.rebuild_indexes().unwrap();
    store.defer_indexes(false);
    assert_eq!(states.boxes, IndexState::Current);
    let texts = match &states.texts {
        IndexState::Current => "\"current\"".to_string(),
        other => format!("\"{}\"", format!("{other:?}").replace('"', "'")),
    };
    report(format!(
        "{{\"phase\":\"index_load\",\"rows\":{intervals},\"seconds\":{:.1},\"rss_mib\":{},\"texts\":{texts}}}",
        t.elapsed().as_secs_f64(),
        rss_kib() / 1024
    ));

    // Queries: SplitMix64 positions inside the field.
    let mut state = 0x5EED_u64;
    let mut unit = || {
        state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        ((z ^ (z >> 31)) >> 11) as f64 / (1_u64 << 53) as f64
    };
    let extent = SPACING_M * side as f64;
    for size in [25.0, 200.0] {
        let mut times = Vec::new();
        let mut hits = 0;
        for _ in 0..1_000 {
            let lo = [
                400_000.0 + extent * unit(),
                8_640_000.0 + extent * unit(),
                1_200.0 + 100.0 * unit(),
            ];
            let q = Box3::new(lo, [lo[0] + size, lo[1] + size, lo[2] + size]).unwrap();
            let t = Instant::now();
            hits += store.bbox(CRS, &q).unwrap().len();
            times.push(t.elapsed());
        }
        times.sort();
        report(format!(
            "{{\"phase\":\"bbox\",\"cube_m\":{size},\"queries\":1000,\"hits\":{hits},\"p50_ms\":{:.3},\
             \"p99_ms\":{:.3},\"max_ms\":{:.3}}}",
            ms(quantile(&times, 0.5)),
            ms(quantile(&times, 0.99)),
            ms(times[times.len() - 1])
        ));
        if size == 25.0 {
            assert!(hits > 0, "the 25 m queries found nothing");
            assert!(
                quantile(&times, 0.99) <= Duration::from_millis(50),
                "INT-FSQL-001: bbox p99 {:.3} ms > 50 ms",
                ms(quantile(&times, 0.99))
            );
        }
    }

    // Two relogs of one hole's intervals with the indexes current: new text on the same depths (text rows only),
    // then the same text 0.5 m deeper (R-tree rows only).
    let mut projection = Projection::open(store).unwrap();
    for (name, shift_mm) in [("relog_text", 0), ("relog_depths", 500)] {
        let relog = chained(seq, prev, &hole_changes(holes / 2, side, Some(shift_mm)));
        let t = Instant::now();
        projection.feed(&relog).unwrap();
        let garbage = projection.projector().index_garbage();
        report(format!(
            "{{\"phase\":\"{name}\",\"intervals\":{INTERVALS_PER_HOLE},\"ms\":{:.1},\"stale_boxes\":{},\
             \"stale_texts\":{}}}",
            ms(t.elapsed()),
            garbage.stale_boxes,
            garbage.stale_texts
        ));
        prev = relog.batch_digest().unwrap();
        seq += 1;
    }
    if states.texts == IndexState::Current {
        let mut times = Vec::new();
        for _ in 0..20 {
            let t = Instant::now();
            assert_eq!(
                projection.projector().text("relogged").unwrap().len(),
                INTERVALS_PER_HOLE
            );
            times.push(t.elapsed());
        }
        times.sort();
        report(format!(
            "{{\"phase\":\"text\",\"query\":\"relogged\",\"hits\":{INTERVALS_PER_HOLE},\"queries\":20,\
             \"p50_ms\":{:.3},\"max_ms\":{:.3}}}",
            ms(quantile(&times, 0.5)),
            ms(times[times.len() - 1])
        ));
    } else {
        let refused = projection.projector().text("relogged").unwrap_err();
        assert!(
            matches!(
                refused,
                DrillStoreError::IndexStale {
                    index: Index::Texts,
                    reason: Some(_)
                }
            ),
            "{refused:?}"
        );
        report(format!(
            "{{\"phase\":\"text\",\"query\":\"relogged\",\"refused\":\"{}\"}}",
            refused.to_refusal().code
        ));
    }
    let mid = holes / 2;
    let corner = [
        400_000.0 + SPACING_M * (mid % side) as f64,
        8_640_000.0 + SPACING_M * (mid / side) as f64,
        1_299.0,
    ];
    let q = Box3::new(
        [corner[0] - 1.0, corner[1] - 1.0, corner[2]],
        [corner[0] + 1.0, corner[1] + 1.0, corner[2] + 1.0],
    )
    .unwrap();
    let near = projection.projector().bbox(CRS, &q).unwrap();
    assert!(
        near.iter().any(|r| r.hole_id == format!("H{mid:06}")),
        "{near:?}"
    );
}
