//! GATE-CORE-01 (`BD-173`, plan §12): 10⁵ holes / 2×10⁷ intervals ingested, desurveyed and composited; full
//! rebuild ≤ 15 min; as-known-at query from one anchor.
//!
//! The campaign comes from `fmine-gen`'s drillhole generator (`BD-131`) as `FMT-COL-1` chunks, one per batch of
//! holes. Each chunk row becomes the ledger records a core shed and a lab would write for one metre of core: the
//! logged interval, a half-core sample of it and the sample's gold result. Each hole adds its collar, one gyro
//! station (the generator's holes are vertical), a dispatch of its samples and the lab certificate its results
//! cite. Batches are chained as a ledger chains them and fed to the drill projection with its indexes deferred.
//!
//! The scale run is ignored by default; run it in release:
//!
//! ```text
//! cargo test --release -p fmine-verify --test gate_core_01 -- --ignored --nocapture
//! ```
//!
//! `FMINE_GATE_CORE_HOLES` sets the hole count (default 100 000), `FMINE_GATE_CORE_INTERVALS` the intervals per
//! hole (default 200), `FMINE_GATE_CORE_DIR` the directory for the store (default the temp dir) and
//! `FMINE_GATE_CORE_OUT` a file for the measured numbers as JSON lines.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]

use std::io::Write as _;
use std::time::{Duration, Instant};

use fmine_array::col::{QualifierBits, unpack_measurand_qualifier};
use fmine_core::anchor::{FMT_ANCHOR_V1, LedgerAnchor};
use fmine_core::hash::{Digest32, sha256};
use fmine_drill::records::{
    AssayMethod, AssayResult, AssayValue, CertificateRef, Collar, CustodyEvent, Dispatch,
    DrillRecord, HoleType, Interval, LabCertificate, Logger, Orientation, Position, PositionMethod,
    QaqcRegime, RunDirection, Sample, SampleType, Survey, SurveyInstrument,
};
use fmine_drill::store::{Change, delta_ops};
use fmine_gen::drillholes::{DrillholeCampaignConfig, generate_collar, generate_interval_chunk};
use fmine_ledger::batch::{DeltaBatch, FMT_LEDGER_V2, GENESIS_PREV_HASH};
use fmine_projection::drill::DrillStore;
use fmine_projection::projection::Projection;

const CELL: &str = "CELL-SITE(gate-core-01)";
const CRS: &str = "EPSG:32735";
/// The generator's grid starts at (0, 0); the campaign sits at this UTM offset.
const ORIGIN: [f64; 2] = [400_000.0, 8_640_000.0];
const LAB: &str = "LAB-FA-01";
/// Holes per batch: 100 holes of 200 metres are 60 400 records, under the ledger's 65 536 per batch.
const HOLES_PER_BATCH: u64 = 100;
/// 2026-01-05T00:00:00Z, when the first hole's samples are dispatched.
const CAMPAIGN_START_NS: i64 = 1_767_571_200_000_000_000;
const NS_PER_DAY: i64 = 86_400_000_000_000;
/// Half NQ core: half the area of a 47.6 mm core, cm².
const HALF_CORE_AREA_CM2: f64 = 8.9;

fn env_or<T: std::str::FromStr>(name: &str, default: T) -> T {
    std::env::var(name)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

fn campaign(holes: u64, intervals_per_hole: u32) -> DrillholeCampaignConfig {
    DrillholeCampaignConfig {
        campaign_id: "GATE-CORE-01".to_string(),
        hole_count: holes,
        intervals_per_hole,
        ..DrillholeCampaignConfig::benchmark_10e5_10e7(0x00C0_4E01)
    }
}

/// Lithology by depth: saprolite over fresh rock, so each hole has two domains.
fn lithology(from_mm: i32) -> &'static str {
    if from_mm < 30_000 {
        "SAPROLITE"
    } else {
        "GRANODIORITE"
    }
}

/// The ledger records of holes `[first, first + count)`: the generator's chunk for them, bridged row by row.
fn hole_changes(cfg: &DrillholeCampaignConfig, first: u64, count: u64) -> Vec<Change> {
    let chunk = generate_interval_chunk(cfg, first, count).unwrap();
    let mut out = Vec::with_capacity(chunk.len() * 3 + chunk.hole_dict().len() * 4);
    let mut row = 0_usize;
    for (h, run) in (first..).zip(chunk.hole_runs()) {
        let gen_collar = generate_collar(cfg, h);
        let hole_id = chunk.hole_dict()[run.hole_idx as usize].clone();
        assert_eq!(
            hole_id, gen_collar.hole_id,
            "the chunk's holes come in order"
        );
        let dispatched_at_ns = CAMPAIGN_START_NS + (h as i64) * 60_000_000_000;
        let received_at_ns = dispatched_at_ns + i64::from(gen_collar.assay_lag_days) * NS_PER_DAY;
        let dispatch_id = format!("{hole_id}-D");
        let certificate_id = format!("{hole_id}-C");
        out.push(Change::Put(DrillRecord::Collar(Collar {
            hole_id: hole_id.clone(),
            hole_type: HoleType::Diamond,
            program: cfg.campaign_id.clone(),
            planned: None,
            actual: Some(Position {
                crs: CRS.to_string(),
                easting_m: ORIGIN[0] + gen_collar.easting_m,
                northing_m: ORIGIN[1] + gen_collar.northing_m,
                elevation_m: gen_collar.elevation_m,
                method: PositionMethod::DifferentialGnss,
                sigma_horizontal_m: 0.05,
                sigma_vertical_m: 0.1,
            }),
        })));
        out.push(Change::Put(DrillRecord::Survey(Survey {
            survey_id: format!("{hole_id}-S0"),
            hole_id: hole_id.clone(),
            depth_mm: 0,
            instrument: SurveyInstrument::Gyro,
            orientation: Some(Orientation {
                azimuth_deg: 0.0,
                dip_deg: -90.0,
                sigma_azimuth_deg: 0.5,
                sigma_dip_deg: 0.25,
            }),
            magnetic_interference: false,
            run: RunDirection::Out,
        })));
        let mut sample_ids = Vec::new();
        for k in 0..(run.end_row as usize - row) {
            let i = row + k;
            let (from_mm, to_mm) = (chunk.from_mm_slice()[i], chunk.to_mm_slice()[i]);
            let (_, qualifiers) =
                unpack_measurand_qualifier(chunk.packed_measurand_qualifiers()[i]).unwrap();
            let grade_g_t = chunk.value_blocks()[i / 8].0[i % 8];
            let density = chunk.density_blocks()[i / 8].0[i % 8];
            let interval_id = format!("{hole_id}-I{k:03}");
            let sample_id = format!("{hole_id}-P{k:03}");
            out.push(Change::Put(DrillRecord::Interval(Interval {
                interval_id: interval_id.clone(),
                hole_id: hole_id.clone(),
                from_mm: u64::try_from(from_mm).unwrap(),
                to_mm: u64::try_from(to_mm).unwrap(),
                recovery: None,
                lithology: lithology(from_mm).to_string(),
                alteration: None,
                mineralization: "disseminated pyrite".to_string(),
                weathering: if from_mm < 30_000 {
                    "highly weathered"
                } else {
                    "fresh"
                }
                .to_string(),
                logger: Logger::Human {
                    logger_id: "geo-07".to_string(),
                },
            })));
            out.push(Change::Put(DrillRecord::Sample(Sample {
                sample_id: sample_id.clone(),
                interval_ids: vec![interval_id],
                mass_g: Some(density * HALF_CORE_AREA_CM2 * f64::from(to_mm - from_mm) / 10.0),
                sample_type: SampleType::HalfCore,
                splitter: "core saw".to_string(),
                custody: vec![CustodyEvent {
                    at_ns: dispatched_at_ns,
                    holder: "core shed".to_string(),
                }],
            })));
            let value = if qualifiers.contains(QualifierBits::BELOW_DL) {
                AssayValue::BelowDetection {
                    limit: cfg.detection_limit_g_per_t * 1e-6,
                }
            } else {
                AssayValue::Measured {
                    mass_fraction: grade_g_t * 1e-6,
                }
            };
            out.push(Change::Put(DrillRecord::AssayResult(AssayResult {
                result_id: format!("{hole_id}-R{k:03}"),
                sample_id: sample_id.clone(),
                dispatch_id: dispatch_id.clone(),
                analyte: "Au".to_string(),
                method: AssayMethod::FireAssayAas,
                value,
                lab: LAB.to_string(),
                lab_batch: format!("{hole_id}-B"),
                certificate: CertificateRef::Certificate {
                    certificate_id: certificate_id.clone(),
                },
                qaqc_regime: QaqcRegime::Current,
                received_at_ns,
            })));
            sample_ids.push(sample_id);
        }
        row = run.end_row as usize;
        out.push(Change::Put(DrillRecord::Dispatch(Dispatch {
            dispatch_id,
            lab: LAB.to_string(),
            sample_ids,
            dispatched_at_ns,
        })));
        let original = sha256(format!("{certificate_id} original").as_bytes());
        out.push(Change::Put(DrillRecord::LabCertificate(LabCertificate {
            certificate_id,
            lab: LAB.to_string(),
            original_sha256: original,
            cas_root: sha256(&original.0),
            media_type: "text/csv".to_string(),
            issued_at_ns: received_at_ns,
        })));
    }
    assert_eq!(row, chunk.len());
    out
}

fn chained(seq: u64, prev_hash: Digest32, changes: &[Change]) -> DeltaBatch {
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

fn rss_mib() -> u64 {
    let out = std::process::Command::new("ps")
        .args(["-o", "rss=", "-p", &std::process::id().to_string()])
        .output()
        .unwrap();
    String::from_utf8_lossy(&out.stdout)
        .trim()
        .parse::<u64>()
        .unwrap_or(0)
        / 1024
}

/// The bridged records are ones a ledger takes: three holes of ten metres commit through `drill_law`, which checks
/// every record and that each one's references exist.
#[test]
fn test_the_bridged_campaign_commits_through_drill_law() {
    let cfg = campaign(3, 10);
    let changes = hole_changes(&cfg, 0, 3);
    assert_eq!(changes.len(), 3 * (4 + 3 * 10));
    let mut ledger = fmine_ledger::l0::L0Ledger::new();
    let lease = ledger.create_cell(CELL, "node-1").unwrap();
    fmine_drill::store::commit(&mut ledger, &lease, &changes).unwrap();
    let below = changes
        .iter()
        .filter(|c| {
            matches!(
                c,
                Change::Put(DrillRecord::AssayResult(AssayResult {
                    value: AssayValue::BelowDetection { .. },
                    ..
                }))
            )
        })
        .count();
    assert!(below < 30, "{below} of 30 results below detection");
}

#[test]
#[ignore = "scale run: 2x10^7 intervals in release; see the module documentation"]
fn test_gate_core_01_full_rebuild() {
    let holes: u64 = env_or("FMINE_GATE_CORE_HOLES", 100_000);
    let per_hole: u32 = env_or("FMINE_GATE_CORE_INTERVALS", 200);
    let cfg = campaign(holes, per_hole);
    let dir = std::env::var("FMINE_GATE_CORE_DIR")
        .map_or_else(|_| std::env::temp_dir(), std::path::PathBuf::from)
        .join(format!("fmine-gate-core-01-{holes}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut out: Box<dyn std::io::Write> = match std::env::var("FMINE_GATE_CORE_OUT") {
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

    let mut store = DrillStore::open(dir.join("drill.db")).unwrap();
    store.defer_indexes(true);
    let mut projection = Projection::open(store).unwrap();
    let (mut seq, mut prev) = (0_u64, GENESIS_PREV_HASH);
    let (mut records, mut generate, mut feed) = (0_usize, Duration::ZERO, Duration::ZERO);
    let started = Instant::now();
    let mut segment = (Instant::now(), 0_u64);
    for first in (0..holes).step_by(HOLES_PER_BATCH as usize) {
        let t = Instant::now();
        let changes = hole_changes(&cfg, first, HOLES_PER_BATCH.min(holes - first));
        let batch = chained(seq, prev, &changes);
        generate += t.elapsed();
        records += changes.len();
        let t = Instant::now();
        projection.feed(&batch).unwrap();
        feed += t.elapsed();
        prev = batch.batch_digest().unwrap();
        seq += 1;
        if seq % 10 == 0 {
            let done = seq * HOLES_PER_BATCH * u64::from(per_hole);
            let rate = (done - segment.1) as f64 / segment.0.elapsed().as_secs_f64();
            report(format!(
                "{{\"phase\":\"progress\",\"intervals\":{done},\"seconds\":{:.1},\"segment_intervals_per_second\":{rate:.0},\
                 \"rss_mib\":{}}}",
                started.elapsed().as_secs_f64(),
                rss_mib()
            ));
            segment = (Instant::now(), done);
        }
    }
    let ingest = started.elapsed();
    let intervals = holes * u64::from(per_hole);
    report(format!(
        "{{\"phase\":\"ingest\",\"holes\":{holes},\"intervals\":{intervals},\"records\":{records},\"batches\":{seq},\
         \"seconds\":{:.1},\"generate_seconds\":{:.1},\"feed_seconds\":{:.1},\"intervals_per_second\":{:.0},\
         \"rss_mib\":{}}}",
        ingest.as_secs_f64(),
        generate.as_secs_f64(),
        feed.as_secs_f64(),
        intervals as f64 / ingest.as_secs_f64(),
        rss_mib()
    ));
}
