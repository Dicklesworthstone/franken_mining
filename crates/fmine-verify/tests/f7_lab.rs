//! `F7` lab test (`BD-088`; plan §26.7 and the §26.11 failure table): a crash at every write boundary of an
//! `FMT-SEG-1` segment, then detection at open, read-only degraded mode, the digest-bound repair, and resumed
//! writes.
//!
//! Crash model. `SegmentWriter` syncs after every frame, so a crash can damage only the write in flight: an
//! append's length prefix and batch frame, or the seal's footer. Every acknowledged record survives. Of the
//! write in flight, any length from the old end of the file to the new one may survive, and within it any
//! 512-byte sector may be lost, since a disk need not persist the sectors of one write in order. A lost
//! sector keeps its bytes before the old end, which were synced, and reads back zeros or stale bytes after it.
//!
//! Two campaigns:
//! - exhaustive: for an append into an empty segment, an append that spans sectors, and a seal, every
//!   surviving length with no sector lost;
//! - seeded: `LabRuntime` seeds `0..SEEDS` choose how many batches were acknowledged, which write is in flight,
//!   the surviving length, the lost sectors and their fill, and whether the repair itself is cut short once
//!   (a partial evidence copy, or the evidence kept and the target untouched). Every 50th seed runs twice,
//!   in a fresh directory, and must give the same trace digest and the same bytes.
//!
//! Oracles, for every crash state:
//! 1. Durability: every acknowledged record verifies, with its head hash. The record in flight counts only if
//!    it survived whole, and then the file is clean.
//! 2. Detection and read-only degraded mode: any other damage reads as `torn_tail` at the old end, and
//!    `SegmentWriter::open_append` refuses until the segment is repaired.
//! 3. The repair plan is deterministic; applying it keeps the complete pre-repair bytes as evidence and
//!    leaves the file byte-identical to its state before the write in flight (history is never rewritten).
//! 4. Writes resume: the batch in flight is appended again if it was lost, the segment is sealed, and the
//!    result is byte-identical to the same segment written with no crash.
#![allow(clippy::expect_used, clippy::panic)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use fmine_core::anchor::{FMT_ANCHOR_V1, LedgerAnchor};
use fmine_core::hash::{Digest32, sha256};
use fmine_core::rand::{PhiloxStream, RngKey};
use fmine_lab::runtime::{DiagnosticLevel, LabEvent, LabRuntime, SchedulerPolicy};
use fmine_ledger::batch::{
    DeltaBatch, DeltaKind, DeltaOp, FMT_LEDGER_V1, GENESIS_PREV_HASH, Witness, WitnessKind,
};
use fmine_ledger::segment::{SegmentState, SegmentWriter, inspect_segment_bytes};
use fmine_ledger::verify::{
    RepairAction, apply_segment_repair, plan_segment_repair, retain_repair_evidence, verify_segment,
};

const CELL: &str = "CELL-SITE(lab)";
const SECTOR: usize = 512;
/// Seeds per CI run (plan §26.7: 0..999 in CI).
const SEEDS: u64 = 1000;
/// Most batches acknowledged before the crash in the seeded campaign.
const MAX_ACKED: u64 = 5;

/// Batch `seq` of the lab cell. Payload sizes cycle through 24, 300 and 900 bytes, so frames range from
/// well inside one sector to over two sectors long, and the fencing epoch rises every fourth batch.
fn batch(seq: u64, prev_hash: Digest32) -> DeltaBatch {
    let epoch = 1 + seq / 4;
    let payload_len = [24_usize, 300, 900][(seq % 3) as usize];
    let payload = (0..payload_len)
        .map(|i| (seq as u8).wrapping_mul(31).wrapping_add(i as u8))
        .collect();
    DeltaBatch {
        format_version: FMT_LEDGER_V1,
        cell_id: CELL.to_string(),
        epoch,
        seq,
        prev_hash,
        anchor: LedgerAnchor {
            format_version: FMT_ANCHOR_V1,
            cell_id: CELL.to_string(),
            ledger_epoch: epoch,
            commit_sequence: seq,
            adapter_registry_epoch: 1,
            schema_epoch: 1,
            policy_epoch: 1,
            privacy_epoch: 1,
            residency_epoch: 1,
            state_root: sha256(format!("{CELL}:{seq}").as_bytes()),
        },
        deltas: vec![DeltaOp::new(
            format!("sample/{seq:04}"),
            DeltaKind::Put,
            payload,
        )],
        witnesses: vec![Witness::new(
            WitnessKind::Continuity,
            format!("receipt/{seq:04}"),
            sha256(&seq.to_le_bytes()),
        )],
        cross_cell_refs: Vec::new(),
    }
}

/// The write in flight when the crash hits.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum InFlight {
    /// Appending batch `acked` after `acked` acknowledged batches.
    Append,
    /// Sealing after `acked` acknowledged batches.
    Seal,
}

/// How the repair itself is cut short once before it completes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RepairCrash {
    None,
    /// A crash while the evidence was being written: this many bytes of it at the partial path.
    PartialEvidence(usize),
    /// A crash after the evidence was kept and before the truncation.
    EvidenceKept,
}

/// Segments written with no crash, by `(batches, sealed)`, with the head hash after each batch.
struct Reference {
    dir: PathBuf,
    files: BTreeMap<(u64, bool), Vec<u8>>,
    heads: Vec<Digest32>,
}

impl Reference {
    fn new(dir: PathBuf) -> Self {
        std::fs::create_dir_all(&dir).expect("reference dir");
        Self {
            dir,
            files: BTreeMap::new(),
            heads: Vec::new(),
        }
    }

    fn bytes(&mut self, batches: u64, sealed: bool) -> Vec<u8> {
        if let Some(bytes) = self.files.get(&(batches, sealed)) {
            return bytes.clone();
        }
        let path = self.dir.join(format!("ref-{batches}-{sealed}.fmseg"));
        let mut writer =
            SegmentWriter::create(&path, CELL, 0, GENESIS_PREV_HASH, Vec::new()).expect("create");
        let mut prev = GENESIS_PREV_HASH;
        for seq in 0..batches {
            prev = writer.append(&batch(seq, prev)).expect("append");
            if self.heads.len() as u64 == seq {
                self.heads.push(prev);
            }
        }
        if sealed {
            writer.seal().expect("seal");
        } else {
            drop(writer);
        }
        let bytes = std::fs::read(&path).expect("read reference");
        self.files.insert((batches, sealed), bytes.clone());
        bytes
    }

    /// Head hash after `batches` batches.
    fn head(&mut self, batches: u64) -> Digest32 {
        if batches == 0 {
            return GENESIS_PREV_HASH;
        }
        self.bytes(batches, false);
        self.heads[(batches - 1) as usize]
    }
}

/// One crash state: `acked` acknowledged batches, the write in flight, and the bytes on disk after it.
struct CrashState {
    acked: u64,
    in_flight: InFlight,
    before: Vec<u8>,
    intended: Vec<u8>,
    on_disk: Vec<u8>,
}

impl CrashState {
    fn new(
        reference: &mut Reference,
        acked: u64,
        in_flight: InFlight,
        damage: impl FnOnce(&[u8], &[u8]) -> Vec<u8>,
    ) -> Self {
        let before = reference.bytes(acked, false);
        let intended = match in_flight {
            InFlight::Append => reference.bytes(acked + 1, false),
            InFlight::Seal => reference.bytes(acked, true),
        };
        assert!(intended.starts_with(&before), "segments only grow");
        let on_disk = damage(&before, &intended);
        Self {
            acked,
            in_flight,
            before,
            intended,
            on_disk,
        }
    }

    fn survived_whole(&self) -> bool {
        self.on_disk == self.intended
    }

    fn final_reference(&self, reference: &mut Reference) -> Vec<u8> {
        match self.in_flight {
            InFlight::Append => reference.bytes(self.acked + 1, true),
            InFlight::Seal => reference.bytes(self.acked, true),
        }
    }
}

/// Runs detection, repair and resumption on `path` and checks oracles 1 to 4. Returns the trace payloads:
/// the verified records after the crash, the torn bytes, and the final file's first eight digest bytes.
fn recover(
    path: &Path,
    state: &CrashState,
    repair_crash: RepairCrash,
    reference: &mut Reference,
    label: &str,
) -> [u64; 3] {
    std::fs::write(path, &state.on_disk).expect("write crash state");
    let acked = state.acked;

    // 1 and 2: durability, detection, read-only degraded mode.
    let verified = verify_segment(path).unwrap_or_else(|e| panic!("{label}: verify refused: {e}"));
    let torn = !state.survived_whole() && state.on_disk != state.before;
    if torn {
        assert_eq!(verified.status, "torn_tail", "{label}");
        assert_eq!(verified.record_count, acked, "{label}");
        assert_eq!(verified.head_hash, reference.head(acked), "{label}");
        assert_eq!(
            verified.verified_bytes,
            state.before.len() as u64,
            "{label}"
        );
        assert!(
            SegmentWriter::open_append(path).is_err(),
            "{label}: a torn segment must refuse writes"
        );
    } else {
        assert_eq!(verified.torn_bytes, 0, "{label}");
        let whole_append = state.survived_whole() && state.in_flight == InFlight::Append;
        let records = if whole_append { acked + 1 } else { acked };
        assert_eq!(verified.record_count, records, "{label}");
        assert_eq!(verified.head_hash, reference.head(records), "{label}");
    }

    // 3: a deterministic plan; evidence kept; the file restored byte for byte.
    let plan = plan_segment_repair(path).unwrap_or_else(|e| panic!("{label}: plan refused: {e}"));
    assert_eq!(
        plan_segment_repair(path).expect("plan again"),
        plan,
        "{label}: plan must be deterministic"
    );
    if torn {
        assert_eq!(plan.action, RepairAction::TruncateTornTail, "{label}");
        assert_eq!(plan.verified_bytes, state.before.len() as u64, "{label}");
        let evidence = plan.evidence_path.clone().expect("evidence path");
        match repair_crash {
            RepairCrash::None => {}
            RepairCrash::PartialEvidence(len) => {
                let mut partial = evidence.as_os_str().to_os_string();
                partial.push(".partial");
                std::fs::write(PathBuf::from(partial), &state.on_disk[..len])
                    .expect("partial evidence");
            }
            RepairCrash::EvidenceKept => {
                retain_repair_evidence(&evidence, &state.on_disk).expect("keep evidence");
                assert_eq!(
                    plan_segment_repair(path).expect("plan after crash"),
                    plan,
                    "{label}: the retry must plan the same repair"
                );
            }
        }
        let receipt = apply_segment_repair(path, plan.plan_digest)
            .unwrap_or_else(|e| panic!("{label}: repair refused: {e}"));
        assert_eq!(receipt.retained_records, acked, "{label}");
        assert_eq!(
            std::fs::read(&evidence).expect("evidence"),
            state.on_disk,
            "{label}: evidence must hold the complete pre-repair bytes"
        );
        assert_eq!(
            std::fs::read(path).expect("repaired"),
            state.before,
            "{label}: repair must restore the bytes before the write in flight"
        );
    } else {
        assert_eq!(plan.action, RepairAction::NoOp, "{label}");
    }

    // 4: writes resume, and the segment ends byte-identical to one written with no crash.
    let resumed = inspect_segment_bytes(&std::fs::read(path).expect("read")).expect("inspect");
    if !matches!(resumed.state, SegmentState::Sealed(_)) {
        let records = resumed.batches.len() as u64;
        let mut writer =
            SegmentWriter::open_append(path).unwrap_or_else(|e| panic!("{label}: resume: {e}"));
        if state.in_flight == InFlight::Append && records == acked {
            writer
                .append(&batch(acked, reference.head(acked)))
                .expect("append the batch in flight again");
        }
        writer.seal().expect("seal");
    }
    let final_bytes = std::fs::read(path).expect("final");
    assert_eq!(
        final_bytes,
        state.final_reference(reference),
        "{label}: resumed segment differs from the crash-free one"
    );
    let digest = sha256(&final_bytes);
    let mut prefix = [0_u8; 8];
    prefix.copy_from_slice(&digest.as_bytes()[..8]);
    [
        verified.record_count,
        verified.torn_bytes,
        u64::from_le_bytes(prefix),
    ]
}

fn scratch(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("fmine-f7-lab-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("scratch dir");
    dir
}

#[test]
fn f7_lab_crash_at_every_byte_of_the_write_in_flight() {
    let dir = scratch("exhaustive");
    let mut reference = Reference::new(dir.join("reference"));
    let mut cases = 0_usize;
    // An append into an empty segment, an append of a 900-byte payload across sector boundaries, a seal.
    for (acked, in_flight) in [
        (0, InFlight::Append),
        (2, InFlight::Append),
        (3, InFlight::Seal),
    ] {
        let before_len = reference.bytes(acked, false).len();
        let intended_len = match in_flight {
            InFlight::Append => reference.bytes(acked + 1, false).len(),
            InFlight::Seal => reference.bytes(acked, true).len(),
        };
        for cut in before_len..=intended_len {
            let state = CrashState::new(&mut reference, acked, in_flight, |_, intended| {
                intended[..cut].to_vec()
            });
            let path = dir.join(format!("cut-{acked}-{in_flight:?}-{cut}.fmseg"));
            let label = format!("acked {acked}, {in_flight:?} in flight, cut at byte {cut}");
            recover(&path, &state, RepairCrash::None, &mut reference, &label);
            cases += 1;
        }
    }
    assert!(cases > 1000, "only {cases} crash states");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Uniform in `0..n` (`n > 0`), by rejection.
fn below(rng: &mut PhiloxStream, n: u64) -> u64 {
    let reject_below = 0_u64.wrapping_sub(n) % n;
    loop {
        let r = rng.next_u64();
        if r >= reject_below {
            return r % n;
        }
    }
}

/// The crash state and repair crash that `seed` chooses.
fn seeded_crash(seed: u64, reference: &mut Reference) -> (CrashState, RepairCrash) {
    let mut rng = PhiloxStream::new(RngKey {
        seed_root: seed,
        partition: 0x0088,
        domain: 7,
        var_group: 0,
        realization: 0,
        brick: 0,
    });
    let acked = below(&mut rng, MAX_ACKED + 1);
    let in_flight = if below(&mut rng, 4) == 0 {
        InFlight::Seal
    } else {
        InFlight::Append
    };
    let lose_bps = below(&mut rng, 5_000);
    let stale_fill = below(&mut rng, 2) == 1;
    let cut_draw = rng.next_u64();
    let sector_draws: Vec<u64> = (0..8).map(|_| rng.next_u64()).collect();
    let repair_draw = below(&mut rng, 3);
    let partial_draw = rng.next_u64();

    let state = CrashState::new(reference, acked, in_flight, |before, intended| {
        let span = (intended.len() - before.len()) as u64 + 1;
        let cut = before.len() + (cut_draw % span) as usize;
        let mut on_disk = intended[..cut].to_vec();
        let mut filler = PhiloxStream::new(RngKey {
            seed_root: seed,
            partition: 0x0088,
            domain: 8,
            var_group: 0,
            realization: 0,
            brick: 0,
        });
        let first_sector = before.len() / SECTOR;
        for sector in first_sector..cut.div_ceil(SECTOR) {
            let draw = sector_draws[sector - first_sector];
            if draw % 10_000 >= lose_bps {
                continue;
            }
            let start = (sector * SECTOR).max(before.len());
            let end = ((sector + 1) * SECTOR).min(cut);
            for byte in &mut on_disk[start..end] {
                *byte = if stale_fill {
                    filler.next_u64() as u8
                } else {
                    0
                };
            }
        }
        on_disk
    });
    let repair_crash = match repair_draw {
        0 => RepairCrash::None,
        1 => RepairCrash::PartialEvidence((partial_draw % state.on_disk.len() as u64) as usize),
        _ => RepairCrash::EvidenceKept,
    };
    (state, repair_crash)
}

/// One seed as a `LabRuntime` protocol: crash, doctor, repair, resume, audit. Returns the trace digest and
/// the final bytes' digest.
fn run_seed(seed: u64, dir: &Path, reference: &mut Reference) -> (Digest32, Digest32) {
    let (state, repair_crash) = seeded_crash(seed, reference);
    std::fs::create_dir_all(dir).expect("run dir");
    let path = dir.join(format!("seed-{seed}.fmseg"));
    let label = format!(
        "seed {seed}: acked {}, {:?} in flight, {} of {} bytes on disk, repair crash {repair_crash:?}",
        state.acked,
        state.in_flight,
        state.on_disk.len(),
        state.intended.len()
    );
    let mut rt = LabRuntime::new(seed, 0, SchedulerPolicy::Fifo, DiagnosticLevel::Quiet)
        .expect("lab runtime");
    rt.schedule(1, 1, "chaos", "crash", state.on_disk.len() as u64)
        .expect("schedule crash");
    let mut outcome = [0_u64; 3];
    let next = |now: i64, actor: &str, tag: &str, payload: u64| LabEvent {
        event_id: 0,
        deliver_at_ns: now + 1,
        priority: 1,
        actor_id: actor.to_string(),
        action_tag: tag.to_string(),
        payload_u64: payload,
    };
    let report = rt
        .run(16, |now, event, spawned| {
            match event.action_tag.as_str() {
                "crash" => {
                    outcome = recover(&path, &state, repair_crash, reference, &label);
                    spawned.push(next(now, "doctor", "verified_records", outcome[0]));
                }
                "verified_records" => spawned.push(next(now, "doctor", "torn_bytes", outcome[1])),
                "torn_bytes" => spawned.push(next(now, "auditor", "final_digest", outcome[2])),
                _ => {}
            }
            Ok(())
        })
        .expect("lab run");
    assert_eq!(report.steps_executed, 4, "{label}");
    (
        report.trace_digest,
        sha256(&std::fs::read(&path).expect("final")),
    )
}

#[test]
fn f7_lab_seeded_crash_campaign_is_repaired_and_replays_identically() {
    let dir = scratch("seeded");
    let mut reference = Reference::new(dir.join("reference"));
    let mut torn_states = 0_usize;
    for seed in 0..SEEDS {
        let first = run_seed(seed, &dir.join("first"), &mut reference);
        if seed % 50 == 0 {
            assert_eq!(
                run_seed(seed, &dir.join("replay"), &mut reference),
                first,
                "seed {seed}: a rerun must give the same trace digest and bytes"
            );
        }
        let (state, _) = seeded_crash(seed, &mut reference);
        if !state.survived_whole() && state.on_disk != state.before {
            torn_states += 1;
        }
    }
    assert!(
        torn_states > SEEDS as usize / 2,
        "only {torn_states} torn states in {SEEDS} seeds"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
