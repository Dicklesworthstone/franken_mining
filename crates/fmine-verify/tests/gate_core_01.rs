//! GATE-CORE-01 (`BD-173`, plan §12): 10⁵ holes / 2×10⁷ intervals ingested, desurveyed and composited; full
//! rebuild ≤ 15 min; as-known-at query from one anchor.
//!
//! The campaign comes from `fmine-gen`'s drillhole generator (`BD-131`) as `FMT-COL-1` chunks, one per batch of
//! holes. Each chunk row becomes the ledger records a core shed and a lab would write for one metre of core: the
//! logged interval, a half-core sample of it and the sample's gold result. Each hole adds its collar, one gyro
//! station (the generator's holes are vertical), a dispatch of its samples and the lab certificate its results
//! cite. After the campaign one more batch re-assays every thousandth hole from hole 7: each of its measured gold
//! results is superseded at 1.25 times the value.
//!
//! The run has two parts.
//! - **The ledger**, not timed: the batches are chained and written to sealed `FMT-SEG-1` segments of at most
//!   192 MiB. This is the log the rebuild starts from.
//! - **The rebuild**, timed from opening the first segment to the last composite. A reader thread opens each
//!   segment, which verifies its batch digests, chain and footer, and hands its batches on. They are fed to a
//!   sharded drill projection (`ShardedDrillStore`) with its indexes deferred; each hole is desurveyed from its
//!   collar and survey as its batch is applied. When the projection stands at the last campaign batch, before the
//!   re-assay, the as-known-at answer is read there: the checked holes' records and composites, stamped with that
//!   anchor, against the generator. Then come the re-assay batch, the index load on every shard, and a parallel
//!   pass that reads every hole's records from the store and composites them to 2 m within lithology runs
//!   (length-weighted; `composite` refuses a hole whose composites do not conserve its inputs).
//!
//! After the rebuild the head is checked: the checked holes' records and composites against the generator with
//! the re-assay applied, their results' version counts, and desurvey. A column query around each checked collar
//! returns exactly that hole's intervals, and a point query where the generator places an interval's midpoint
//! returns exactly that interval.
//!
//! The scale run is ignored by default; run it in release:
//!
//! ```text
//! cargo test --release -p fmine-verify --test gate_core_01 -- --ignored --nocapture
//! ```
//!
//! `FMINE_GATE_CORE_HOLES` sets the hole count (default 100 000), `FMINE_GATE_CORE_INTERVALS` the intervals per
//! hole (default 200), `FMINE_GATE_CORE_SHARDS` the shard stores (default 8), `FMINE_GATE_CORE_THREADS` the
//! compositing threads (default 16), `FMINE_GATE_CORE_DIR` the directory for the segments and the store (default
//! the temp dir) and `FMINE_GATE_CORE_OUT` a file for the measured numbers as JSON lines.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]

use std::collections::BTreeMap;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use fmine_array::col::{QualifierBits, unpack_measurand_qualifier};
use fmine_claim::{AdditivityClass, SpatialSupport};
use fmine_core::anchor::{FMT_ANCHOR_V1, LedgerAnchor};
use fmine_core::hash::{Digest32, sha256};
use fmine_drill::composite::{
    CompositeSet, Domain, Residual, Rule, Weighting, composite, sample_inputs,
};
use fmine_drill::join::Span;
use fmine_drill::records::{
    AssayMethod, AssayResult, AssayValue, CertificateRef, Collar, CustodyEvent, Dispatch,
    DrillRecord, HoleType, Interval, LabCertificate, Logger, Orientation, Position, PositionMethod,
    QaqcRegime, RecordKind, RunDirection, Sample, SampleType, Survey, SurveyInstrument,
};
use fmine_drill::store::{Change, delta_ops, record_key};
use fmine_gen::drillholes::{DrillholeCampaignConfig, generate_collar, generate_interval_chunk};
use fmine_ledger::batch::{DeltaBatch, FMT_LEDGER_V2, GENESIS_PREV_HASH};
use fmine_ledger::mmr::Mmr;
use fmine_ledger::segment::{SegmentReader, SegmentState, SegmentWriter};
use fmine_projection::drill::boxes::Box3;
use fmine_projection::drill::sharded::ShardedDrillStore;
use fmine_projection::drill::{HoleRecords, IndexState, IntervalRef};
use fmine_projection::mark::HighWaterMark;
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
/// The re-assay: every `REASSAY_EVERY`-th hole from `REASSAY_FIRST`, measured results times `REASSAY_FACTOR`.
const REASSAY_FIRST: u64 = 7;
const REASSAY_EVERY: u64 = 1_000;
const REASSAY_FACTOR: f64 = 1.25;
const REASSAY_REASON: &str = "re-assayed: the batch's check assays read high";
/// The compositing rule: 2 m, length-weighted, a short last piece kept, gold as an intensive quantity on core.
const RULE: Rule = Rule {
    length_mm: 2_000,
    weighting: Weighting::Length,
    residual: Residual::Keep,
    additivity: AdditivityClass::IntensiveWeighted,
    input_support: SpatialSupport::PointCore,
};
/// Largest segment the ledger writes; `FMT-SEG-1` allows 256 MiB.
const SEGMENT_BYTES: u64 = 192 * 1024 * 1024;
/// The gate's rebuild budget.
const BUDGET: Duration = Duration::from_secs(15 * 60);

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

/// The re-assayed holes of a campaign of `holes`.
fn reassayed(holes: u64) -> Vec<u64> {
    (REASSAY_FIRST..holes)
        .step_by(REASSAY_EVERY as usize)
        .collect()
}

/// Hole `h`'s re-assay: each measured gold result superseded at `REASSAY_FACTOR` times its value, received 30
/// days after the first; a result below detection stands.
fn reassay(cfg: &DrillholeCampaignConfig, h: u64) -> Vec<Change> {
    hole_changes(cfg, h, 1)
        .into_iter()
        .filter_map(|change| match change {
            Change::Put(DrillRecord::AssayResult(
                r @ AssayResult {
                    value: AssayValue::Measured { mass_fraction },
                    ..
                },
            )) => Some(Change::Supersede {
                record: DrillRecord::AssayResult(AssayResult {
                    value: AssayValue::Measured {
                        mass_fraction: mass_fraction * REASSAY_FACTOR,
                    },
                    received_at_ns: r.received_at_ns + 30 * NS_PER_DAY,
                    ..r
                }),
                reason: REASSAY_REASON.to_string(),
            }),
            _ => None,
        })
        .collect()
}

/// Hole `h`'s records as the generator writes them, after its re-assay when `after_reassay`, each list sorted by
/// id as the store gives them.
fn generated(cfg: &DrillholeCampaignConfig, h: u64, after_reassay: bool) -> HoleRecords {
    let mut changes = hole_changes(cfg, h, 1);
    if after_reassay {
        changes.extend(reassay(cfg, h));
    }
    let mut out = HoleRecords::default();
    let mut results = BTreeMap::new();
    for change in changes {
        let (Change::Put(record) | Change::Supersede { record, .. }) = change;
        match record {
            DrillRecord::Collar(c) => out.collar = Some(c),
            DrillRecord::Survey(s) => out.surveys.push(s),
            DrillRecord::Interval(iv) => out.intervals.push(iv),
            DrillRecord::Sample(s) => out.samples.push(s),
            DrillRecord::AssayResult(r) => {
                results.insert(r.result_id.clone(), r);
            }
            _ => {}
        }
    }
    out.results = results.into_values().collect();
    out.surveys.sort_by(|a, b| a.survey_id.cmp(&b.survey_id));
    out.intervals
        .sort_by(|a, b| a.interval_id.cmp(&b.interval_id));
    out.samples.sort_by(|a, b| a.sample_id.cmp(&b.sample_id));
    out
}

/// A hole's lithology runs: consecutive intervals of one lithology make one domain.
fn lithology_runs(intervals: &[Interval]) -> Vec<Domain> {
    let mut by_depth: Vec<&Interval> = intervals.iter().collect();
    by_depth.sort_by_key(|iv| iv.from_mm);
    let mut out: Vec<Domain> = Vec::new();
    for iv in by_depth {
        match out.last_mut() {
            Some(d) if d.code == iv.lithology && d.span.to_mm == iv.from_mm => {
                d.span.to_mm = iv.to_mm
            }
            _ => out.push(Domain {
                code: iv.lithology.clone(),
                span: Span {
                    from_mm: iv.from_mm,
                    to_mm: iv.to_mm,
                },
            }),
        }
    }
    out
}

/// A hole's gold composites under [`RULE`], from its records.
fn composites(hole: &str, records: &HoleRecords) -> CompositeSet {
    let inputs = sample_inputs(
        hole,
        &records.intervals,
        &records.samples,
        &records.results,
        "Au",
    )
    .unwrap();
    composite(&lithology_runs(&records.intervals), &inputs, &RULE).unwrap()
}

/// Composites per hole of `metres`: 2 m pieces of the saprolite above 30 m and of the fresh rock below it, a
/// short last piece kept.
fn composites_per_hole(metres: u64) -> u64 {
    let saprolite = metres.min(30);
    saprolite.div_ceil(2) + (metres - saprolite).div_ceil(2)
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

/// `text` escaped for a JSON string.
fn json_text(text: &str) -> String {
    text.replace('\\', "\\\\").replace('"', "\\\"")
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

/// One run's size and where it writes.
struct Params {
    holes: u64,
    per_hole: u32,
    batch_holes: u64,
    shards: usize,
    threads: usize,
    segment_bytes: u64,
    /// Every `check_every`-th hole is checked, besides the re-assayed ones.
    check_every: u64,
    dir: PathBuf,
}

/// The ledger on disk: its segments in order and every batch's digest.
struct Ledger {
    segments: Vec<PathBuf>,
    digests: Vec<Digest32>,
}

/// Chain the campaign's batches and its re-assay batch into sealed segments in `dir`, a new segment before a
/// batch that could take the current one past `segment_bytes`.
fn write_ledger(
    cfg: &DrillholeCampaignConfig,
    p: &Params,
    dir: &Path,
    report: &mut dyn FnMut(String),
) -> Ledger {
    std::fs::create_dir_all(dir).unwrap();
    let started = Instant::now();
    let firsts: Vec<u64> = (0..p.holes).step_by(p.batch_holes as usize).collect();
    let reassay_changes: Vec<Change> = reassayed(p.holes)
        .into_iter()
        .flat_map(|h| reassay(cfg, h))
        .collect();
    let mut segments = Vec::new();
    let mut digests: Vec<Digest32> = Vec::new();
    let mut mmr = Mmr::new();
    let mut open: Option<(SegmentWriter, PathBuf, u64)> = None;
    let mut last_len = 0;
    let mut records = 0;
    for seq in 0..=firsts.len() as u64 {
        let changes = match firsts.get(seq as usize) {
            Some(&first) => hole_changes(cfg, first, p.batch_holes.min(p.holes - first)),
            None => reassay_changes.clone(),
        };
        records += changes.len();
        let prev = digests.last().copied().unwrap_or(GENESIS_PREV_HASH);
        let batch = chained(seq, prev, &changes);
        let full = open.as_ref().is_some_and(|(_, path, held)| {
            *held > 0 && std::fs::metadata(path).unwrap().len() + 2 * last_len > p.segment_bytes
        });
        if open.is_none() || full {
            if let Some((writer, _, _)) = open.take() {
                writer.seal().unwrap();
            }
            let path = dir.join(format!("seg-{:05}.fseg", segments.len()));
            let peaks = if seq == 0 {
                Vec::new()
            } else {
                mmr.peaks_at(seq).unwrap()
            };
            let writer = SegmentWriter::create(&path, CELL, seq, prev, peaks).unwrap();
            segments.push(path.clone());
            open = Some((writer, path, 0));
        }
        let (writer, path, held) = open.as_mut().unwrap();
        let before = std::fs::metadata(&*path).unwrap().len();
        let digest = writer.append(&batch).unwrap();
        last_len = std::fs::metadata(&*path).unwrap().len() - before;
        *held += 1;
        mmr.push(digest.as_bytes()).unwrap();
        digests.push(digest);
    }
    open.take().unwrap().0.seal().unwrap();
    let bytes: u64 = segments
        .iter()
        .map(|s| std::fs::metadata(s).unwrap().len())
        .sum();
    report(format!(
        "{{\"phase\":\"ledger\",\"batches\":{},\"records\":{records},\"reassayed_results\":{},\"segments\":{},\
         \"bytes\":{bytes},\"seconds\":{:.1}}}",
        digests.len(),
        reassay_changes.len(),
        segments.len(),
        started.elapsed().as_secs_f64()
    ));
    Ledger { segments, digests }
}

/// Open each segment in order, check that it is sealed and continues the chain, and send its batches.
fn read_segments(segments: &[PathBuf], to: &mpsc::SyncSender<DeltaBatch>) {
    let (mut seq, mut head) = (0, GENESIS_PREV_HASH);
    for path in segments {
        let segment = SegmentReader::open(path).unwrap();
        assert!(
            matches!(segment.state, SegmentState::Sealed(_)),
            "{}: not sealed",
            path.display()
        );
        assert_eq!(segment.header.first_seq, seq, "{}", path.display());
        assert_eq!(segment.header.prev_segment_head, head, "{}", path.display());
        seq += segment.batches.len() as u64;
        head = segment.head_hash;
        for batch in segment.batches {
            if to.send(batch).is_err() {
                return;
            }
        }
    }
}

/// What the compositing pass read and made.
#[derive(Default)]
struct Totals {
    holes: u64,
    intervals: u64,
    samples: u64,
    results: u64,
    composites: u64,
    sampled_mm: u64,
    worst_conservation: f64,
}

impl Totals {
    fn merge(mut self, other: Self) -> Self {
        self.holes += other.holes;
        self.intervals += other.intervals;
        self.samples += other.samples;
        self.results += other.results;
        self.composites += other.composites;
        self.sampled_mm += other.sampled_mm;
        self.worst_conservation = self.worst_conservation.max(other.worst_conservation);
        self
    }
}

/// Read every hole's records from the store and composite them, on `threads` threads.
fn composite_every_hole(store: &ShardedDrillStore<'_>, threads: usize) -> Totals {
    let holes = store.holes().unwrap();
    let next = AtomicUsize::new(0);
    std::thread::scope(|scope| {
        let workers: Vec<_> = (0..threads)
            .map(|_| {
                scope.spawn(|| {
                    let mut t = Totals::default();
                    while let Some((cell, hole)) = holes.get(next.fetch_add(1, Ordering::Relaxed)) {
                        let records = store.hole_records(cell, hole).unwrap();
                        let set = composites(hole, &records);
                        t.holes += 1;
                        t.intervals += records.intervals.len() as u64;
                        t.samples += records.samples.len() as u64;
                        t.results += records.results.len() as u64;
                        t.composites += set.composites.len() as u64;
                        t.sampled_mm += set.composites.iter().map(|c| c.sampled_mm).sum::<u64>();
                        t.worst_conservation =
                            t.worst_conservation.max(set.conservation.relative_error);
                    }
                    t
                })
            })
            .collect();
        workers
            .into_iter()
            .map(|w| w.join().unwrap())
            .fold(Totals::default(), Totals::merge)
    })
}

/// The checked holes' records and composites as the store answers them, against the generator's.
fn check_holes(
    store: &ShardedDrillStore<'_>,
    cfg: &DrillholeCampaignConfig,
    checked: &[u64],
    after_reassay: bool,
    context: &str,
) {
    let again = reassayed(cfg.hole_count);
    for &h in checked {
        let hole = generate_collar(cfg, h).hole_id;
        let records = store.hole_records(CELL, &hole).unwrap();
        let want = generated(cfg, h, after_reassay && again.contains(&h));
        assert_eq!(records, want, "{context}: the records of {hole}");
        assert_eq!(
            composites(&hole, &records),
            composites(&hole, &want),
            "{context}: the composites of {hole}"
        );
    }
}

/// The head after the rebuild: version counts of the checked holes' results, and desurvey.
fn check_head(
    store: &ShardedDrillStore<'_>,
    cfg: &DrillholeCampaignConfig,
    checked: &[u64],
    reassay_seq: u64,
) {
    assert_eq!(store.unlocated_holes().unwrap(), Vec::new());
    let again = reassayed(cfg.hole_count);
    let depth_m = f64::from(cfg.intervals_per_hole) * f64::from(cfg.interval_length_mm) / 1000.0;
    for &h in checked {
        let collar = generate_collar(cfg, h);
        let hole = collar.hole_id.clone();
        let records = generated(cfg, h, false);
        if again.contains(&h) {
            assert_ne!(
                generated(cfg, h, true),
                records,
                "{hole}: the re-assay changes a result"
            );
        }
        for r in &records.results {
            let stored = store
                .record(CELL, &record_key(RecordKind::AssayResult, &r.result_id))
                .unwrap()
                .unwrap();
            let superseded = again.contains(&h) && matches!(r.value, AssayValue::Measured { .. });
            if superseded {
                assert_eq!(stored.versions, 2, "{}", r.result_id);
                assert_eq!(stored.seq, reassay_seq, "{}", r.result_id);
                assert_eq!(
                    stored.version.correction.as_deref(),
                    Some(REASSAY_REASON),
                    "{}",
                    r.result_id
                );
            } else {
                assert_eq!(stored.versions, 1, "{}", r.result_id);
                assert_eq!(stored.version.correction, None, "{}", r.result_id);
            }
        }
        // A column a metre either side of the collar, down past the bottom of the hole, holds its intervals and
        // no other hole's (collars are 25 m apart, give or take 2 m).
        let (x, y, top) = (
            ORIGIN[0] + collar.easting_m,
            ORIGIN[1] + collar.northing_m,
            collar.elevation_m,
        );
        let column = Box3::new(
            [x - 1.0, y - 1.0, top - depth_m - 1.0],
            [x + 1.0, y + 1.0, top + 1.0],
        )
        .unwrap();
        let mut want: Vec<IntervalRef> = records
            .intervals
            .iter()
            .map(|iv| IntervalRef {
                cell_id: CELL.to_string(),
                interval_id: iv.interval_id.clone(),
                hole_id: hole.clone(),
            })
            .collect();
        want.sort();
        assert_eq!(store.bbox(CRS, &column).unwrap(), want, "{hole}: column");
        // The generator places interval k's midpoint on the collar's vertical, (from + to) / 2 below it
        // (`fmine_gen::drillholes::generate_interval_chunk`); that point lies in interval k's box alone.
        let n = records.intervals.len();
        for k in [0, n / 2, n - 1] {
            let iv = &records.intervals[k];
            let mid = top - (iv.from_mm + iv.to_mm) as f64 / 2000.0;
            let at = Box3::new([x, y, mid], [x, y, mid]).unwrap();
            assert_eq!(
                store.bbox(CRS, &at).unwrap(),
                vec![want[k].clone()],
                "{hole}: midpoint of {}",
                iv.interval_id
            );
        }
    }
}

/// Write the ledger, rebuild the projection from it, and check the answers; returns the rebuild's duration.
fn run(p: &Params, report: &mut dyn FnMut(String)) -> Duration {
    let cfg = campaign(p.holes, p.per_hole);
    let ledger = write_ledger(&cfg, p, &p.dir.join("segments"), report);
    let campaign_batches = p.holes.div_ceil(p.batch_holes);
    assert_eq!(ledger.digests.len() as u64, campaign_batches + 1);
    let anchor = HighWaterMark {
        applied: campaign_batches,
        head: ledger.digests[campaign_batches as usize - 1],
    };
    let mut checked: Vec<u64> = (0..p.holes).step_by(p.check_every as usize).collect();
    checked.extend(reassayed(p.holes));
    checked.sort_unstable();
    checked.dedup();
    let intervals = p.holes * u64::from(p.per_hole);

    let started = Instant::now();
    ShardedDrillStore::with_shards_at(p.dir.join("shards"), p.shards, |mut store| {
        store.defer_indexes(true).unwrap();
        let mut projection = Projection::open(store).unwrap();
        assert!(projection.anchor().marks.is_empty(), "a new store");
        let mut known_at = None;
        std::thread::scope(|scope| {
            let (to, batches) = mpsc::sync_channel(4);
            let segments = &ledger.segments;
            scope.spawn(move || read_segments(segments, &to));
            let mut window = (Instant::now(), 0_u64);
            for batch in batches {
                projection.feed(&batch).unwrap();
                let applied = projection.mark(CELL).applied;
                if applied == anchor.applied {
                    // As known at the anchor: before the re-assay batch, whatever the head becomes.
                    let t = Instant::now();
                    let answer = projection.answer(|store| {
                        check_holes(store, &cfg, &checked, false, "as known at the anchor");
                    });
                    assert_eq!(answer.anchor.mark(CELL), anchor);
                    known_at = Some(t.elapsed());
                }
                if applied % 100 == 0 && applied <= campaign_batches {
                    let done = (applied * p.batch_holes).min(p.holes) * u64::from(p.per_hole);
                    let rate = (done - window.1) as f64 / window.0.elapsed().as_secs_f64();
                    report(format!(
                        "{{\"phase\":\"progress\",\"intervals\":{done},\"seconds\":{:.1},\
                         \"window_intervals_per_second\":{rate:.0},\"rss_mib\":{}}}",
                        started.elapsed().as_secs_f64(),
                        rss_mib()
                    ));
                    window = (Instant::now(), done);
                }
            }
        });
        let fed = started.elapsed();
        assert_eq!(
            projection.mark(CELL),
            HighWaterMark {
                applied: campaign_batches + 1,
                head: *ledger.digests.last().unwrap(),
            }
        );
        let known_at = known_at.expect("the projection stood at the anchor");
        report(format!(
            "{{\"phase\":\"feed\",\"shards\":{},\"holes\":{},\"intervals\":{intervals},\"batches\":{},\
             \"seconds\":{:.1},\"intervals_per_second\":{:.0},\"as_known_at_holes\":{},\
             \"as_known_at_seconds\":{:.2},\"rss_mib\":{}}}",
            p.shards,
            p.holes,
            campaign_batches + 1,
            fed.as_secs_f64(),
            intervals as f64 / fed.as_secs_f64(),
            checked.len(),
            known_at.as_secs_f64(),
            rss_mib()
        ));

        let mut store = projection.into_projector();
        let t = Instant::now();
        let states = store.rebuild_indexes().unwrap();
        assert_eq!(states.boxes, IndexState::Current);
        report(format!(
            "{{\"phase\":\"indexes\",\"seconds\":{:.1},\"boxes\":\"{}\",\"texts\":\"{}\",\"rss_mib\":{}}}",
            t.elapsed().as_secs_f64(),
            json_text(&format!("{:?}", states.boxes)),
            json_text(&format!("{:?}", states.texts)),
            rss_mib()
        ));

        let projection = Projection::open(store).unwrap();
        let t = Instant::now();
        let totals = composite_every_hole(projection.projector(), p.threads);
        let rebuild = started.elapsed();
        report(format!(
            "{{\"phase\":\"composite\",\"threads\":{},\"holes\":{},\"composites\":{},\"sampled_mm\":{},\
             \"worst_relative_conservation\":{:e},\"seconds\":{:.1},\"rss_mib\":{}}}",
            p.threads,
            totals.holes,
            totals.composites,
            totals.sampled_mm,
            totals.worst_conservation,
            t.elapsed().as_secs_f64(),
            rss_mib()
        ));
        assert_eq!(totals.holes, p.holes);
        assert_eq!(totals.intervals, intervals);
        assert_eq!(totals.samples, intervals);
        assert_eq!(totals.results, intervals);
        assert_eq!(
            totals.composites,
            p.holes * composites_per_hole(u64::from(p.per_hole))
        );
        // Every metre of core has a sample and a result, so every millimetre is in a sampled composite.
        assert_eq!(totals.sampled_mm, intervals * 1_000);
        report(format!(
            "{{\"phase\":\"rebuild\",\"seconds\":{:.1},\"budget_seconds\":{}}}",
            rebuild.as_secs_f64(),
            BUDGET.as_secs()
        ));

        let t = Instant::now();
        let store = projection.projector();
        check_holes(store, &cfg, &checked, true, "at the head");
        check_head(store, &cfg, &checked, campaign_batches);
        report(format!(
            "{{\"phase\":\"head\",\"checked_holes\":{},\"seconds\":{:.1}}}",
            checked.len(),
            t.elapsed().as_secs_f64()
        ));
        rebuild
    })
    .unwrap()
}

/// The bridged records are ones a ledger takes: three holes of ten metres, and the re-assay of one of them, commit
/// through `drill_law`, which checks every record and that each one's references exist.
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
    let again = reassay(&cfg, 1);
    assert_eq!(again.len(), 10 - changes_below(&cfg, 1));
    fmine_drill::store::commit(&mut ledger, &lease, &again).unwrap();
}

/// Results of hole `h` below detection.
fn changes_below(cfg: &DrillholeCampaignConfig, h: u64) -> usize {
    generated(cfg, h, false)
        .results
        .iter()
        .filter(|r| matches!(r.value, AssayValue::BelowDetection { .. }))
        .count()
}

/// One hole generated alone is the same as within its batch, so a hole's records can be regenerated to check it.
#[test]
fn test_a_hole_generated_alone_is_the_hole_of_its_batch() {
    let cfg = campaign(30, 12);
    let batch = hole_changes(&cfg, 10, 10);
    let alone: Vec<Change> = (10..20).flat_map(|h| hole_changes(&cfg, h, 1)).collect();
    assert_eq!(batch, alone);
}

/// The whole pipeline at a small size: 24 holes of 40 m in batches of 5 holes and segments of about two batches,
/// rebuilt into 3 shards. Every hole is checked as known at the anchor and at the head (hole 7 is re-assayed),
/// desurvey included, and every hole is composited.
#[test]
fn test_a_small_campaign_rebuilds_from_segments_and_answers_as_known_at() {
    static RUN: AtomicUsize = AtomicUsize::new(0);
    let dir = std::env::temp_dir().join(format!(
        "fmine-gate-core-01-small-{}-{}",
        std::process::id(),
        RUN.fetch_add(1, Ordering::Relaxed)
    ));
    let p = Params {
        holes: 24,
        per_hole: 40,
        batch_holes: 5,
        shards: 3,
        threads: 4,
        segment_bytes: 160 * 1024,
        check_every: 1,
        dir,
    };
    let mut lines = Vec::new();
    run(&p, &mut |line| lines.push(line));
    let segments = lines
        .iter()
        .find(|l| l.contains("\"phase\":\"ledger\""))
        .unwrap();
    assert!(
        segments.contains("\"batches\":6,"),
        "five campaign batches and the re-assay: {segments}"
    );
    assert!(
        !segments.contains("\"segments\":1,"),
        "the ledger spans several segments: {segments}"
    );
    for phase in ["feed", "indexes", "composite", "rebuild", "head"] {
        assert!(
            lines
                .iter()
                .any(|l| l.contains(&format!("\"phase\":\"{phase}\""))),
            "{phase}: {lines:?}"
        );
    }
}

#[test]
#[ignore = "scale run: 2x10^7 intervals in release; see the module documentation"]
fn test_gate_core_01_full_rebuild() {
    let holes: u64 = env_or("FMINE_GATE_CORE_HOLES", 100_000);
    let p = Params {
        holes,
        per_hole: env_or("FMINE_GATE_CORE_INTERVALS", 200),
        batch_holes: HOLES_PER_BATCH,
        shards: env_or("FMINE_GATE_CORE_SHARDS", 8),
        threads: env_or("FMINE_GATE_CORE_THREADS", 16),
        segment_bytes: SEGMENT_BYTES,
        check_every: (holes / 50).max(1),
        dir: std::env::var("FMINE_GATE_CORE_DIR")
            .map_or_else(|_| std::env::temp_dir(), PathBuf::from)
            .join(format!("fmine-gate-core-01-{holes}-{}", std::process::id())),
    };
    let mut out: Box<dyn std::io::Write> = match std::env::var("FMINE_GATE_CORE_OUT") {
        Ok(path) => Box::new(
            std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)
                .unwrap(),
        ),
        Err(_) => Box::new(std::io::sink()),
    };
    let rebuild = run(&p, &mut |line| {
        eprintln!("{line}");
        writeln!(out, "{line}").unwrap();
    });
    assert!(
        rebuild <= BUDGET,
        "the rebuild took {:.1} s, past the {} s budget",
        rebuild.as_secs_f64(),
        BUDGET.as_secs()
    );
}
