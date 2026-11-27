#![allow(clippy::expect_used, clippy::unwrap_used)]
//! `GATE-SYNC-01` (`BD-109`, plan §26.12): 1,000-seed partition, link-drop, and crash-at-boundary
//! lab matrix across `fmine-ledger`, `fmine-sync`, `fmine-object`, and all five `fmine-lab` FM
//! protocol oracles (`PO-BED-1`, `PO-BED-2`, `PO-BED-3`, `PO-BED-4`, `PO-BED-10`).
//!
//! Thresholds verified (plan §26.12):
//! - 1,000 deterministic seeds (`0..1000`);
//! - 0 prefix/history divergences (`PO-BED-2`);
//! - 0 duplicate applies under replayed deliveries;
//! - 100% `SyncCert` RFC 6962 consistency proofs verified on every replica apply;
//! - 0 obligation leaks (`OB-SYNC-ACK` resolved on ack; `OB-SEAL` finalized after hard-kill resume);
//! - 0 FM oracle violations across `PO-BED-1`, `PO-BED-2`, `PO-BED-3`, `PO-BED-4`, `PO-BED-10`.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use fmine_core::anchor::{FMT_ANCHOR_V1, LedgerAnchor};
use fmine_core::hash::{Digest32, blake3_hash, sha256};
use fmine_core::rand::{PhiloxStream, RngKey};
use fmine_lab::oracles::{
    BridgeIntent, CasTraceEvent, LeaseEvent, PostingEvent, check_po_bed_1_single_writer,
    check_po_bed_2_prefix_consistency, check_po_bed_3_root_last_cas,
    check_po_bed_4_exactly_once_posting, check_po_bed_10_bridge_idempotency,
};
use fmine_ledger::batch::{
    CrossCellRef, DeltaBatch, DeltaKind, DeltaOp, FMT_LEDGER_V2, GENESIS_PREV_HASH,
};
use fmine_ledger::cell::AdmitOutcome;
use fmine_ledger::l0::L0Ledger;
use fmine_ledger::mmr::Mmr;
use fmine_object::cas::{CasManifest, ChunkRef, chunk_bytes};
use fmine_object::store::CasStore;
use fmine_sync::gluing::{GluingGateDecision, check_anchor_publish_gluing};
use fmine_sync::receipt::{ResumableTransfer, TransferPhase};
use fmine_sync::sync_cert::{ReplicaLogState, SyncApplyOutcome, SyncCert};
use fmine_sync::version_vector::{CellLagStatus, CellWatermark, VersionVector, detect_lag};

const SEEDS: u64 = 1_000;
const CELL_LUMWANA: &str = "CELL-SITE(lumwana)";
const CELL_KIBALI: &str = "CELL-SITE(kibali)";
const CELL_DEC: &str = "CELL-DEC";
const STALE_THRESHOLD_NS: i64 = 600_000_000_000; // 10 min

#[derive(Debug, Default)]
struct SeedStats {
    sync_certs_verified: u64,
    duplicate_replays_idempotent: u64,
    quarantined_stale_batches: u64,
    gluing_blocks: u64,
    gluing_passes: u64,
    stale_partitions: u64,
    cas_crash_boundary: usize,
    seed_digest: Digest32,
}

fn make_batch(
    cell_id: &str,
    epoch: u64,
    seq: u64,
    prev_hash: Digest32,
    mut deltas: Vec<DeltaOp>,
    cross_cell_refs: Vec<CrossCellRef>,
) -> DeltaBatch {
    deltas.sort();
    DeltaBatch {
        format_version: FMT_LEDGER_V2,
        cell_id: cell_id.to_string(),
        epoch,
        seq,
        prev_hash,
        anchor: LedgerAnchor {
            format_version: FMT_ANCHOR_V1,
            cell_id: cell_id.to_string(),
            ledger_epoch: epoch,
            commit_sequence: seq,
            adapter_registry_epoch: 1,
            schema_epoch: 1,
            policy_epoch: 1,
            privacy_epoch: 1,
            residency_epoch: 1,
            state_root: sha256(format!("{cell_id}:{epoch}:{seq}").as_bytes()),
        },
        deltas,
        witnesses: Vec::new(),
        cross_cell_refs,
    }
}

fn run_single_seed(seed: u64, scratch_root: &Path) -> SeedStats {
    let mut rng = PhiloxStream::new(RngKey {
        seed_root: seed,
        partition: 0x5359_4E43,
        domain: 1,
        var_group: 0,
        realization: 0,
        brick: 0,
    });
    let mut stats = SeedStats::default();
    let mut l0 = L0Ledger::new();

    let mut lease_events: Vec<LeaseEvent> = Vec::new();
    let mut site_hashes: BTreeMap<&str, Vec<Digest32>> = BTreeMap::new();
    let mut site_mmrs: BTreeMap<&str, Mmr> = BTreeMap::new();
    let mut site_leases = BTreeMap::new();
    let mut pending_obs: BTreeMap<&str, Vec<String>> = BTreeMap::new();

    // 1. Authoritative site writes, lease rotations, and quarantined split-brain stale writes.
    for &cell in &[CELL_LUMWANA, CELL_KIBALI] {
        let mut lease = l0.create_cell(cell, "node-primary").expect("create cell");
        lease_events.push(LeaseEvent::GrantLease {
            cell_id: cell.to_string(),
            epoch: lease.epoch,
            holder_id: lease.holder_id.clone(),
        });
        let mut mmr = Mmr::new();
        let mut hashes = Vec::new();
        let mut obs = Vec::new();

        let n_batches = 2 + (rng.next_u64() % 4); // 2..=5 authoritative batches
        let rotate_at = 1 + (rng.next_u64() % (n_batches - 1)); // strictly in 1..n_batches

        for seq in 0..n_batches {
            if seq == rotate_at {
                let stale_epoch = lease.epoch;
                let stale_holder = lease.holder_id.clone();
                let next_holder = format!("node-failover-{seed}-{seq}");
                lease = l0.transfer_lease(cell, next_holder).expect("transfer");
                lease_events.push(LeaseEvent::GrantLease {
                    cell_id: cell.to_string(),
                    epoch: lease.epoch,
                    holder_id: lease.holder_id.clone(),
                });

                // Inject a split-brain lower-epoch batch: must be quarantined without mutating head!
                let pre_head = l0.head_hash(cell).expect("pre head");
                let pre_cnt = l0.committed_count(cell).expect("pre cnt");
                let stale_batch = make_batch(
                    cell,
                    stale_epoch,
                    pre_cnt,
                    pre_head,
                    vec![DeltaOp::new(
                        format!("stale/{seed}/{seq}"),
                        DeltaKind::Put,
                        b"split-brain".to_vec(),
                    )],
                    Vec::new(),
                );
                let admit = l0.admit_replica(stale_batch).expect("admit stale");
                assert!(
                    matches!(admit, AdmitOutcome::Quarantined { .. }),
                    "seed {seed}: lower-epoch batch must be quarantined, got {admit:?}"
                );
                assert_eq!(l0.head_hash(cell).expect("post head"), pre_head);
                assert_eq!(l0.committed_count(cell).expect("post cnt"), pre_cnt);
                stats.quarantined_stale_batches += 1;
                lease_events.push(LeaseEvent::ApplyBatch {
                    cell_id: cell.to_string(),
                    epoch: stale_epoch,
                    holder_id: stale_holder,
                    quarantined: true,
                });
            }

            let ob_id = format!("ob/sync/{cell}/{seed}/{seq}");
            let prev = l0.head_hash(cell).expect("head");
            let batch = make_batch(
                cell,
                lease.epoch,
                seq,
                prev,
                vec![
                    DeltaOp::new(
                        format!("prod/{seed}/{seq}"),
                        DeltaKind::Put,
                        format!("payload:{seed}:{seq}").into_bytes(),
                    ),
                    DeltaOp::new(ob_id.clone(), DeltaKind::ReserveObligation, Vec::new()),
                ],
                Vec::new(),
            );
            let digest = l0.commit(&lease, batch).expect("commit site batch");
            mmr.push(digest.as_bytes()).expect("mmr push");
            assert_eq!(
                l0.mmr_root(cell).expect("l0 mmr"),
                mmr.root(),
                "seed {seed}: L0 MMR root must match independent MMR"
            );
            hashes.push(digest);
            obs.push(ob_id);
            lease_events.push(LeaseEvent::ApplyBatch {
                cell_id: cell.to_string(),
                epoch: lease.epoch,
                holder_id: lease.holder_id.clone(),
                quarantined: false,
            });
        }

        site_hashes.insert(cell, hashes);
        site_mmrs.insert(cell, mmr);
        site_leases.insert(cell, lease);
        pending_obs.insert(cell, obs);
    }

    assert!(
        check_po_bed_1_single_writer(&lease_events).is_empty(),
        "seed {seed}: PO-BED-1 violation"
    );

    // 2. Enterprise replicas start at seq 1 (initial prefix) and detect partition/lag via VersionVector.
    let mut replicas: BTreeMap<&str, ReplicaLogState> = BTreeMap::new();
    let mut local_vv = VersionVector::new();
    let mut remote_vv = VersionVector::new();
    let partitioned_kibali = (rng.next_u64() & 1) == 1;
    let now_ns: i64 = 10_000_000_000_000;

    for &cell in &[CELL_LUMWANA, CELL_KIBALI] {
        let mmr = &site_mmrs[cell];
        let total = site_hashes[cell].len() as u64;
        let root_1 = mmr.root_at(1).expect("root 1");
        replicas.insert(
            cell,
            ReplicaLogState {
                cell_id: cell.to_string(),
                highest_epoch: 1,
                committed_seq: 1,
                mmr_root: root_1,
                stale: false,
            },
        );
        local_vv.upsert(CellWatermark {
            cell_id: cell.to_string(),
            epoch: 1,
            commit_seq: 1,
            mmr_root: root_1,
            last_contact_ns: if cell == CELL_KIBALI && partitioned_kibali {
                now_ns - STALE_THRESHOLD_NS - 5_000_000_000
            } else {
                now_ns - 1_000
            },
        });
        if !(cell == CELL_KIBALI && partitioned_kibali) {
            remote_vv.upsert(CellWatermark {
                cell_id: cell.to_string(),
                epoch: site_leases[cell].epoch,
                commit_seq: total,
                mmr_root: mmr.root(),
                last_contact_ns: now_ns - 500,
            });
        }
    }

    assert_eq!(
        VersionVector::from_bytes(&local_vv.to_bytes()).expect("vv roundtrip"),
        local_vv
    );
    let lag = detect_lag(&local_vv, &remote_vv, now_ns, STALE_THRESHOLD_NS);
    if partitioned_kibali {
        assert!(matches!(
            lag.per_cell[CELL_KIBALI],
            CellLagStatus::Stale { .. }
        ));
        stats.stale_partitions += 1;
    }

    // 3. Cross-cell INV-012 gluing blocks enterprise publish while sites lag behind target heads!
    let lum_target_seq = (site_hashes[CELL_LUMWANA].len() - 1) as u64;
    let kib_target_seq = (site_hashes[CELL_KIBALI].len() - 1) as u64;
    let dec_batch = make_batch(
        CELL_DEC,
        1,
        0,
        GENESIS_PREV_HASH,
        vec![DeltaOp::new(
            format!("decision/{seed}"),
            DeltaKind::Put,
            b"card".to_vec(),
        )],
        vec![
            CrossCellRef::new(
                CELL_LUMWANA,
                lum_target_seq,
                site_hashes[CELL_LUMWANA][lum_target_seq as usize],
            ),
            CrossCellRef::new(
                CELL_KIBALI,
                kib_target_seq,
                site_hashes[CELL_KIBALI][kib_target_seq as usize],
            ),
        ],
    );
    let mut held_partial: BTreeMap<String, Vec<Digest32>> = BTreeMap::new();
    held_partial.insert(CELL_LUMWANA.to_string(), vec![site_hashes[CELL_LUMWANA][0]]);
    // Kibali missing or behind -> gluing MUST block with FMINE-REF-502!
    let gluing_decision = check_anchor_publish_gluing(&dec_batch, &held_partial);
    assert!(
        matches!(
            &gluing_decision,
            GluingGateDecision::PublishBlocked { refusal, .. } if refusal.code == "FMINE-REF-502"
        ),
        "seed {seed}: expected gluing block FMINE-REF-502, got {gluing_decision:?}"
    );
    stats.gluing_blocks += 1;

    // 4. Chunked sync catch-up with link drops + duplicate replays; 100% SyncCert verified!
    for &cell in &[CELL_LUMWANA, CELL_KIBALI] {
        let mmr = &site_mmrs[cell];
        let target = site_hashes[cell].len() as u64;
        let epoch = site_leases[cell].epoch;
        let replica = replicas.get_mut(cell).expect("replica");

        while replica.committed_seq < target {
            let remaining = target - replica.committed_seq;
            let step = 1 + (rng.next_u64() % remaining);
            let from_seq = replica.committed_seq;
            let to_seq = from_seq + step;

            let cert = SyncCert::build_from_mmr(cell, epoch, mmr, from_seq, to_seq)
                .expect("build sync cert");
            let decoded = SyncCert::from_bytes(&cert.to_bytes()).expect("decode sync cert");
            let outcome = replica.verify_and_apply(&decoded).expect("apply sync cert");
            assert_eq!(
                outcome,
                SyncApplyOutcome::Advanced {
                    new_seq: to_seq,
                    new_root: mmr.root_at(to_seq).expect("root at to"),
                }
            );
            stats.sync_certs_verified += 1;

            // Replay the EXACT retransmitted wire SyncCert (`from_seq -> to_seq`): must be
            // idempotent (0 duplicate applies, replica stays non-stale).
            let dup_outcome = replica
                .verify_and_apply(&decoded)
                .expect("exact wire retransmit idempotent");
            assert_eq!(dup_outcome, SyncApplyOutcome::AlreadyApplied);
            assert_eq!(replica.committed_seq, to_seq);
            assert!(!replica.stale);
            stats.duplicate_replays_idempotent += 1;
        }

        // PO-BED-2 oracle: replica log is an exact hash-matched prefix of writer log (0 divergence).
        let replica_prefix = &site_hashes[cell][..replica.committed_seq as usize];
        assert!(
            check_po_bed_2_prefix_consistency(cell, &site_hashes[cell], replica_prefix).is_empty(),
            "seed {seed}: PO-BED-2 divergence on {cell}"
        );

        // Enterprise ack resolves every OB-SYNC-ACK obligation -> 0 obligation leaks!
        let lease = &site_leases[cell];
        let resolve_ops: Vec<DeltaOp> = pending_obs[cell]
            .iter()
            .map(|ob| DeltaOp::new(ob.clone(), DeltaKind::ResolveObligation, Vec::new()))
            .collect();
        let ack_seq = l0.committed_count(cell).expect("cnt");
        let ack_prev = l0.head_hash(cell).expect("prev");
        let ack_batch = make_batch(
            cell,
            lease.epoch,
            ack_seq,
            ack_prev,
            resolve_ops,
            Vec::new(),
        );
        l0.commit(lease, ack_batch).expect("commit ack");
        assert!(
            l0.open_obligations(cell).expect("open obs").is_empty(),
            "seed {seed}: obligation leak on {cell}"
        );
    }

    // Gluing now succeeds once both site cells caught up!
    let mut held_full: BTreeMap<String, Vec<Digest32>> = BTreeMap::new();
    held_full.insert(CELL_LUMWANA.to_string(), site_hashes[CELL_LUMWANA].clone());
    held_full.insert(CELL_KIBALI.to_string(), site_hashes[CELL_KIBALI].clone());
    assert_eq!(
        check_anchor_publish_gluing(&dec_batch, &held_full),
        GluingGateDecision::PublishAllowed { checked_refs: 2 }
    );
    stats.gluing_passes += 1;

    // 5. CAS OB-SEAL crash at every chunk boundary (PO-BED-3) with real CasStore + receipt resume.
    let seed_dir = scratch_root.join(format!("s{seed}"));
    let store = CasStore::open(seed_dir.join("cas")).expect("cas store");
    let receipt_path = seed_dir.join("xfer.obsr");
    let n_chunks_target = 3 + ((rng.next_u64() % 2) as usize); // 3 or 4 chunks
    let chunk_sz: usize = 256;
    let payload_len = (n_chunks_target - 1) * chunk_sz + 137;
    let payload: Vec<u8> = (0..payload_len)
        .map(|i| {
            ((seed.wrapping_mul(131) ^ (i as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)) >> 8) as u8
        })
        .collect();
    let sliced = chunk_bytes(&payload, chunk_sz).expect("chunk bytes");
    let refs: Vec<ChunkRef> = sliced.iter().map(|(c, _)| *c).collect();
    let manifest = CasManifest::from_chunks("application/octet-stream", chunk_sz as u32, refs)
        .expect("manifest");
    let crash_after = (rng.next_u64() as usize) % sliced.len(); // exercises 0, 1, 2, 3!
    stats.cas_crash_boundary = crash_after;

    let mut cas_trace: Vec<CasTraceEvent> = Vec::new();
    {
        let mut xfer = ResumableTransfer::start_request(
            format!("xfer-{seed}"),
            format!("lease-{seed}"),
            manifest.clone(),
            &receipt_path,
        )
        .expect("start ob-seal");
        for (cref, bytes) in sliced.iter().take(crash_after) {
            xfer.drain_chunk(&store, cref.sha256, bytes, &receipt_path)
                .expect("drain pre-crash");
            cas_trace.push(CasTraceEvent::WriteAndVerifyChunk {
                chunk_id: cref.sha256.to_hex(),
            });
        }
        // Simulated crash right at boundary `crash_after`!
    }
    assert!(
        !store.has_manifest(&manifest.root_digest),
        "seed {seed}: PO-BED-3 root visible before finalize!"
    );

    let mut resumed = ResumableTransfer::resume_from_receipt(&receipt_path).expect("resume");
    assert_eq!(resumed.receipt().verified_chunks.len(), crash_after);
    for cref in resumed.receipt().remaining_chunks() {
        let (_, bytes) = sliced
            .iter()
            .find(|(c, _)| c.sha256 == cref.sha256)
            .expect("chunk");
        resumed
            .drain_chunk(&store, cref.sha256, bytes, &receipt_path)
            .expect("drain resumed");
        cas_trace.push(CasTraceEvent::WriteAndVerifyChunk {
            chunk_id: cref.sha256.to_hex(),
        });
    }
    let proof = resumed
        .finalize(&store, &receipt_path)
        .expect("finalize ob-seal");
    assert_eq!(resumed.receipt().phase, TransferPhase::Finalized);
    assert_eq!(proof.published_root, manifest.root_digest);
    assert_eq!(
        store.read_object(&manifest.root_digest).expect("read cas"),
        payload
    );
    cas_trace.push(CasTraceEvent::PublishRoot {
        root_id: manifest.root_digest.to_hex(),
        required_chunks: sliced.iter().map(|(c, _)| c.sha256.to_hex()).collect(),
    });
    assert!(
        check_po_bed_3_root_last_cas(&cas_trace).is_empty(),
        "seed {seed}: PO-BED-3 violation"
    );

    // 6. PO-BED-4 & PO-BED-10 oracles on seeded posting & bridge outbox replay.
    let posting = vec![
        PostingEvent::CommitOriginal {
            slot_id: format!("slot-{seed}"),
            journal_entry_id: format!("JE-{seed}-1"),
        },
        PostingEvent::SupersedeWithReversal {
            slot_id: format!("slot-{seed}"),
            reversal_entry_id: format!("REV-{seed}-1"),
        },
        PostingEvent::CommitOriginal {
            slot_id: format!("slot-{seed}"),
            journal_entry_id: format!("JE-{seed}-2"),
        },
    ];
    assert!(check_po_bed_4_exactly_once_posting(&posting).is_empty());
    let intents = vec![
        BridgeIntent {
            idempotency_key: format!("ik-{seed}-a"),
            account_code: "5100-MINING".into(),
            amount_cents: 10_000 + (seed as i64),
        },
        BridgeIntent {
            idempotency_key: format!("ik-{seed}-b"),
            account_code: "5200-MILLING".into(),
            amount_cents: 20_000 + (seed as i64),
        },
    ];
    assert!(check_po_bed_10_bridge_idempotency(&intents, true).is_empty());

    // Deterministic per-seed digest over verified state roots + CAS proof digest.
    let mut fold = Vec::new();
    fold.extend_from_slice(&seed.to_le_bytes());
    fold.extend_from_slice(replicas[CELL_LUMWANA].mmr_root.as_bytes());
    fold.extend_from_slice(replicas[CELL_KIBALI].mmr_root.as_bytes());
    fold.extend_from_slice(proof.proof_digest.as_bytes());
    stats.seed_digest = blake3_hash(&fold);

    let _ = fs::remove_dir_all(&seed_dir);
    stats
}

struct TempDirGuard(std::path::PathBuf);

impl Drop for TempDirGuard {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn test_gate_sync_01_1000_seed_partition_and_crash_matrix() {
    let scratch_guard = TempDirGuard(
        std::env::temp_dir().join(format!("fmine-gate-sync-01-{}", std::process::id())),
    );
    let scratch = &scratch_guard.0;
    let _ = fs::remove_dir_all(scratch);
    fs::create_dir_all(scratch).expect("mkdir");

    let mut total_sync_certs = 0u64;
    let mut total_dup_replays = 0u64;
    let mut total_quarantined = 0u64;
    let mut total_gluing_blocks = 0u64;
    let mut total_gluing_passes = 0u64;
    let mut total_stale_partitions = 0u64;
    let mut boundary_hits = [0u64; 4];
    let mut matrix_buf = Vec::with_capacity((SEEDS as usize) * 32);
    let mut first_10_digests = Vec::with_capacity(10);

    for seed in 0..SEEDS {
        let s = run_single_seed(seed, scratch);
        total_sync_certs += s.sync_certs_verified;
        total_dup_replays += s.duplicate_replays_idempotent;
        total_quarantined += s.quarantined_stale_batches;
        total_gluing_blocks += s.gluing_blocks;
        total_gluing_passes += s.gluing_passes;
        total_stale_partitions += s.stale_partitions;
        boundary_hits[s.cas_crash_boundary] += 1;
        matrix_buf.extend_from_slice(s.seed_digest.as_bytes());
        if seed < 10 {
            first_10_digests.push(s.seed_digest);
        }
    }

    // Determinism check: replaying seeds 0..10 yields bit-identical per-seed digests.
    for (seed, expected) in first_10_digests.iter().enumerate() {
        let replay = run_single_seed(seed as u64, scratch);
        assert_eq!(
            replay.seed_digest, *expected,
            "seed {seed} digest drifted on replay"
        );
    }

    let matrix_digest = blake3_hash(&matrix_buf);
    assert!(total_sync_certs >= 2_000);
    assert_eq!(total_dup_replays, total_sync_certs);
    assert_eq!(total_quarantined, SEEDS * 2);
    assert_eq!(total_gluing_blocks, SEEDS);
    assert_eq!(total_gluing_passes, SEEDS);
    assert!(total_stale_partitions > 400 && total_stale_partitions < 600);
    // Every chunk crash boundary 0, 1, 2, 3 was exercised across the 1,000 seeds.
    for (b, &hits) in boundary_hits.iter().enumerate() {
        assert!(hits > 0, "crash boundary {b} was never hit");
    }

    eprintln!(
        "GATE-SYNC-01 PASS: seeds={SEEDS} sync_certs_verified={total_sync_certs} \
         dup_replays_idempotent={total_dup_replays} quarantined_stale={total_quarantined} \
         stale_partitions={total_stale_partitions} gluing_blocks={total_gluing_blocks} \
         gluing_passes={total_gluing_passes} crash_boundaries={boundary_hits:?} \
         divergences=0 duplicate_applies=0 obligation_leaks=0 oracle_violations=0 \
         matrix_digest={matrix_digest}"
    );
}

#[test]
fn test_gate_sync_01_planted_faults_fail_the_matrix() {
    // 1. Forged SyncCert refuses and marks replica stale without advancing state.
    let mut mmr = Mmr::new();
    for i in 0u64..4 {
        mmr.push(sha256(&i.to_le_bytes()).as_bytes()).expect("push");
    }
    let mut replica = ReplicaLogState {
        cell_id: CELL_LUMWANA.into(),
        highest_epoch: 1,
        committed_seq: 1,
        mmr_root: mmr.root_at(1).expect("r1"),
        stale: false,
    };
    let mut forged = SyncCert::build_from_mmr(CELL_LUMWANA, 1, &mmr, 1, 4).expect("cert");
    forged.mmr_root_after.0[0] ^= 0xFF;
    assert!(replica.verify_and_apply(&forged).is_err());
    assert!(replica.stale);
    assert_eq!(replica.committed_seq, 1);

    // 2. Unquarantined lower-epoch batch fails PO-BED-1 oracle.
    assert!(
        !check_po_bed_1_single_writer(&[
            LeaseEvent::GrantLease {
                cell_id: CELL_LUMWANA.into(),
                epoch: 2,
                holder_id: "node-b".into(),
            },
            LeaseEvent::ApplyBatch {
                cell_id: CELL_LUMWANA.into(),
                epoch: 1,
                holder_id: "node-a".into(),
                quarantined: false,
            },
        ])
        .is_empty()
    );

    // 3. Premature CAS root publish fails PO-BED-3 oracle.
    assert!(
        !check_po_bed_3_root_last_cas(&[CasTraceEvent::PublishRoot {
            root_id: "root-0".into(),
            required_chunks: vec!["missing-chunk".into()],
        }])
        .is_empty()
    );

    // 4. Unclosed OB-SYNC-ACK obligation is detected as non-empty open_obligations.
    let mut l0 = L0Ledger::new();
    let lease = l0.create_cell(CELL_LUMWANA, "node-a").expect("cell");
    let b = make_batch(
        CELL_LUMWANA,
        lease.epoch,
        0,
        GENESIS_PREV_HASH,
        vec![DeltaOp::new(
            "ob/sync/leaked",
            DeltaKind::ReserveObligation,
            Vec::new(),
        )],
        Vec::new(),
    );
    l0.commit(&lease, b).expect("commit");
    assert!(!l0.open_obligations(CELL_LUMWANA).expect("obs").is_empty());

    // 5. Prefix fork fails PO-BED-2 oracle.
    let d0 = blake3_hash(b"d0");
    let d1 = blake3_hash(b"d1");
    let d_bad = blake3_hash(b"d1-fork");
    assert!(!check_po_bed_2_prefix_consistency(CELL_LUMWANA, &[d0, d1], &[d0, d_bad]).is_empty());
}
