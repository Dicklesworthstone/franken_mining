# GATE-SYNC-01: 1,000-Seed Partition and Crash Matrix (`BD-109`)

Verdict: PASS

- **Bead:** `BD-109` (`bd-109`, WP-05 Sync and lab)
- **Claim type:** Known answers (deterministic 1,000-seed partition, link-drop, duplicate-replay, lease-rotation, and crash-at-boundary matrix with planted-fault negative controls)
- **Plan specification:** §26.12 (`GATE-SYNC-01`), §26.7 (`PO-BED-1`, `PO-BED-2`, `PO-BED-3`, `PO-BED-4`, `PO-BED-5`, `PO-BED-10`), §26.3 (`INV-012`)
- **Test harness:** `crates/fmine-verify/tests/gate_sync_01.rs`
  - `test_gate_sync_01_1000_seed_partition_and_crash_matrix`
  - `test_gate_sync_01_planted_faults_fail_the_matrix`

## Measured Gate Run (FACT)

- **Command:** `cargo test -j 1 --locked -p fmine-verify --test gate_sync_01 -- --nocapture`
- **Host / Target:** RCH remote worker `vmi1153651` (`x86_64-unknown-linux-gnu`), `dev` profile, `rustc 1.100` (`nightly-2026-08-31`) (cross-host verified bit-identical to `ovh-a`)
- **Duration:** `117.91s` on `vmi1153651` (`8.37s` on `ovh-a`), `2 passed; 0 failed`
- **Aggregate matrix digest (BLAKE3 over 1,000 per-seed digests):** `5a1441e591472ecd73e4b3c276a404af772b79387d522d435aa92e0e4bb71569`
- **Repeatability:** Re-executing seeds `0..10` produced bit-identical per-seed digests (`10 / 10`), and exact wire `SyncCert` retransmissions (`from_seq -> to_seq`) returned `SyncApplyOutcome::AlreadyApplied` (`3,273 / 3,273`).

| Metric | Plan §26.12 Threshold | Observed | Verdict |
|---|---|---|---|
| Seeded runs | `1,000` (`0..1000`) | `1,000` | PASS |
| Prefix / history divergences (`PO-BED-2`) | `0` | `0` | PASS |
| Duplicate applies under replay | `0` | `0` (`3,273 / 3,273` idempotent `AlreadyApplied`) | PASS |
| `SyncCert` RFC 6962 proofs verified | `100%` | `3,273 / 3,273` (`100%`) | PASS |
| Obligation leaks (`OB-SYNC-ACK`, `OB-SEAL`) | `0` | `0` | PASS |
| Bedrock FM oracle violations (`PO-BED-1/2/3/4/10`) | `0` | `0` | PASS |
| Split-brain lower-epoch batches quarantined (`PO-BED-1`) | all injected | `2,000 / 2,000` | PASS |
| `F1` stale partitions detected (`VersionVector::detect_lag`) | exercised | `512` | PASS |
| `INV-012` cross-cell gluing blocks then passes | `1,000 / 1,000` | `1,000` blocked (`FMINE-REF-502`), `1,000` allowed | PASS |
| `PO-BED-3` CAS `OB-SEAL` crash-at-boundary resumes | all boundaries `0..=3` | `[313, 291, 289, 107]` (`1,000` total) | PASS |

## Planted-Fault Negative Controls

`test_gate_sync_01_planted_faults_fail_the_matrix` verifies five independent injected failures are caught fail-closed:
1. Forged `SyncCert` (`mmr_root_after` bit flip) refuses apply, marks replica `stale = true`, and leaves `committed_seq` unchanged.
2. Unquarantined lower-epoch write triggers `check_po_bed_1_single_writer`.
3. Premature CAS root publication before chunk verification triggers `check_po_bed_3_root_last_cas`.
4. Unresolved `OB-SYNC-ACK` obligation remains visible in `L0Ledger::open_obligations`.
5. Forked replica digest triggers `check_po_bed_2_prefix_consistency`.

## Validity Boundary

Exercises the in-memory `L0Ledger`, RFC 6962 `Mmr` / `SyncCert`, `VersionVector`, `GluingGateDecision`, filesystem `CasStore` + `ObSealReceipt` hard-kill resume, and all five `fmine-lab` FM oracles deterministically over `PhiloxStream` seeds `0..1000`. Live QUIC/ATP wire transport throughput and 7-day multi-site catch-up timing belong to `INT-ATP-001` (`BD-110`) and `GATE-SYNC-02` (`BD-267`).
