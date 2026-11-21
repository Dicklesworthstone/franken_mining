# CONTRACT.md — `fmine-sync`

## 1. Purpose
Bedrock's S1 offline-first site-to-enterprise replication and anti-entropy layer (plan §26.3, §26.7, §26.9, §26.13 GB-36; `BD-102`–`BD-108`):
- `manifest` (`BD-104`): plain chunk want/have manifest anti-entropy (`FMNE` + `SYNM`) and exact `GB-36` wire-size measurement vs Merkle set-diff;
- `transport` (`BD-105`): QUIC/TLS 1.3 transport policy over ATP with unconditional plaintext ban (`S3(d)`) and first-party macaroon authentication;
- `version_vector` (`BD-102`): per-cell watermarks (`FMNE` + `VVER`) and lag/staleness detection (`F1`);
- `sync_cert` (`BD-103`): RFC 6962 MMR prefix-consistency certificates (`FMNE` + `SCER`, `PO-BED-2`);
- `residency_plan` (`BD-107`): pre-transfer residency planner refusing disallowed exports with `FMINE-REF-303` before any bytes move;
- `gluing` (`BD-108`): cross-cell anchor gluing gate (`INV-012`, `F12`) blocking publication on dangling references;
- `receipt` (`BD-106`): durable `OB-SEAL` chunk receipts (`FMNE` + `OBSR`) enforcing request -> drain -> finalize and hard-kill resumability without ATP 0.6.0 journaling (`S3(e)`).

- **Crate:** `fmine-sync`
- **Layer:** S1
- **Subsystem:** Bedrock
- **LOC budget:** 16,000 planned / 18,000 ceiling
- **Allowed Module Families:** `["manifest", "transport", "version_vector", "sync_cert", "residency_plan", "gluing", "receipt"]`

## 2. Inputs
- `CasManifest` and `CasStore` from `fmine-object` (`FMT-CAS-1`).
- `FmKeyRing`, `Macaroon`, and `ResidencyRegistry` from `fmine-authz`.
- `Mmr`, `DeltaBatch`, and cross-cell digest tables from `fmine-ledger`.
- Explicit caller-supplied timestamps (`now_ns: i64`; no ambient clock reads).

## 3. Outputs
- `WantHaveManifest`, `WantRequest`, `Gb36SizeReport` (`manifest.rs`).
- `AuthenticatedSyncSession` bound to mTLS 1.3 + verified first-party macaroon (`transport.rs`).
- `VersionVector`, `CellWatermark`, `LagReport` (`version_vector.rs`).
- `SyncCert` and `SyncApplyOutcome` (`sync_cert.rs`).
- `ResidencySyncPlan` (`residency_plan.rs`).
- `GluingGateDecision` (`gluing.rs`).
- `ObSealReceipt`, `ResumableTransfer`, and `QuiescenceProof` (`receipt.rs`).

## 4. Invariants
- **GB-36 plain manifest superiority (`BD-104`):** `WantHaveManifest` encodes a 2,048-chunk (8 GiB block model) manifest in ~64 KiB against a ~0.94 MB Merkle set-diff at 30% change rate, and 12-cell version-vector anti-entropy in ~9.5 KB (`test_chunk_want_have_anti_entropy_and_gb36_wire_sizes`).
- **Plaintext transport banned (`BD-105`, `S3(d)`):** any non-QUIC/TLS-1.3 transport scheme, non-TLS-1.3 handshake, zero peer certificate, wrong ALPN, or invalid/expired/wrong-site macaroon is refused before session establishment (`test_quic_tls_atp_transport_refuses_plaintext_and_verifies_macaroon`).
- **Per-cell version vectors (`BD-102`, `F1`):** lag detection identifies behind, ahead, epoch-diverged, and stale-partitioned cells without silently merging conflicting epochs (`test_version_vector_lag_detection_and_stale_partition_f1`).
- **Prefix consistency (`BD-103`, `PO-BED-2`):** every `SyncCert` is verified against `fmine_ledger::mmr::verify_consistency`; tampered roots or forged proofs refuse without mutating replica state (`test_sync_cert_po_bed_2_verifies_prefix_and_refuses_forged_history`).
- **Residency before bytes move (`BD-107`):** any cross-border transfer item failing `ResidencyRegistry::evaluate_transfer` aborts the entire transfer plan with `FMINE-REF-303` and schedules zero bytes (`test_residency_planner_refuses_disallowed_transfer_before_bytes_move_fmine_ref_303`).
- **Anchor gluing (`BD-108`, `INV-012`):** dangling cross-cell references block anchor publication with `NotObservable` or `Stale` (`test_gluing_gate_blocks_anchor_publish_on_dangling_cross_cell_ref`).
- **OB-SEAL hard-kill resume (`BD-106`, `PO-BED-3`, `S3(e)`):** mid-drain crash leaves `CasStore` root unpublished and resumes cleanly from the durable `ObSealReceipt`, publishing only after all chunks verify (`test_ob_seal_chunk_receipts_resume_after_hard_kill_request_drain_finalize`).

## 5. Refusals
`SyncError` maps via `SyncError::to_refusal()` to:
- `FMINE-REF-303` (`Residency303`, exit 7): cross-border transfer disallowed by `registries/residency.toml`.
- `FMINE-REF-301`/`302`/`304`/`505`: relayed from `fmine-authz::AuthzError`.
- `FMINE-REF-502` (`StaleAnchor502`, exit 6): stale partition, epoch divergence, or dangling cross-cell reference (`INV-012`).
- `FMINE-REF-505` (`DigestMismatch505`, exit 8): plaintext transport attempt (`S3(d)`), TLS policy violation, invalid `SyncCert`, codec/CRC corruption, or chunk mismatch.
- `FMINE-REF-506` (`InputMissingOrUnreadable506`, exit 5): CAS missing chunk or storage I/O refusal.

## 6. Determinism class
- `src/lib.rs`, `src/manifest.rs`, `src/transport.rs`, `src/version_vector.rs`, `src/sync_cert.rs`, `src/residency_plan.rs`, `src/gluing.rs`: `BitwisePortable`.
- `src/receipt.rs`: `Exact` (interacts with `CasStore` filesystem persistence).

## 7. Evidence class of outputs
Replication manifests, sync certificates, residency plans, and transfer receipts are Bedrock authority/integrity artifacts, not physical `Claim`s; they never upgrade `EvidenceClass` (`INV-006`).

## 8. Validity domain
- Up to `1,048,576` chunk digests per `WantHaveManifest`, `1..=16 MiB` chunk size.
- QUIC/TLS 1.3 with ALPN `"fmine-atp/1"` and first-party HMAC-SHA256 macaroons (`FmKeyRing`).
- RFC 6962 MMR consistency proofs for non-empty prefix extensions (`1 <= from_seq <= to_seq`).

## 9. Oracles
- `test_chunk_want_have_anti_entropy_and_gb36_wire_sizes` (`src/manifest.rs`).
- `test_quic_tls_atp_transport_refuses_plaintext_and_verifies_macaroon` (`src/transport.rs`).
- `test_version_vector_lag_detection_and_stale_partition_f1` (`src/version_vector.rs`).
- `test_sync_cert_po_bed_2_verifies_prefix_and_refuses_forged_history` (`src/sync_cert.rs`).
- `test_residency_planner_refuses_disallowed_transfer_before_bytes_move_fmine_ref_303` (`src/residency_plan.rs`).
- `test_gluing_gate_blocks_anchor_publish_on_dangling_cross_cell_ref` (`src/gluing.rs`).
- `test_ob_seal_chunk_receipts_resume_after_hard_kill_request_drain_finalize` (`src/receipt.rs`).

## 10. Known gaps
- Live network socket / ATP 0.6.0 wire adapter waits on `INT-ATP-001` (`BD-110`) and `asupersync` 0.5.0 linkage (`BD-099`).
- 1,000-seed LabRuntime partition & crash chaos matrix belongs to `GATE-SYNC-01` (`BD-109`).
- Self-verified only; no independent review or multi-ISA CI run yet.

## 11. Public API
- `SyncError` (`to_refusal`)
- `manifest::{TAG_SYNC_MANIFEST, WantHaveManifest, WantRequest, Gb36SizeReport, diff_manifest, measure_gb36_comparison}`
- `transport::{ATP_ALPN_V1, TransportScheme, TlsPeerIdentity, AuthenticatedSyncSession}`
- `version_vector::{TAG_VERSION_VECTOR, CellWatermark, VersionVector, CellLagStatus, LagReport, detect_lag}`
- `sync_cert::{TAG_SYNC_CERT, SyncCert, ReplicaLogState, SyncApplyOutcome}`
- `residency_plan::{SyncTransferItem, PlannedTransfer, ResidencySyncPlan, plan_residency_filtered_sync}`
- `gluing::{GluingGateDecision, check_anchor_publish_gluing}`
- `receipt::{TAG_OB_SEAL_RECEIPT, TransferPhase, ObSealReceipt, QuiescenceProof, ResumableTransfer}`

## 12. Proof obligations
- `BD-104` (`GB-36` wire sizes): `test_chunk_want_have_anti_entropy_and_gb36_wire_sizes` (`src/manifest.rs`).
- `BD-105` (`S3(d)` plaintext ban): `test_quic_tls_atp_transport_refuses_plaintext_and_verifies_macaroon` (`src/transport.rs`).
- `PO-BED-2` (`BD-103`): `test_sync_cert_po_bed_2_verifies_prefix_and_refuses_forged_history` (`src/sync_cert.rs`).
- `BD-107` (`FMINE-REF-303`): `test_residency_planner_refuses_disallowed_transfer_before_bytes_move_fmine_ref_303` (`src/residency_plan.rs`).
- `INV-012` (`BD-108`): `test_gluing_gate_blocks_anchor_publish_on_dangling_cross_cell_ref` (`src/gluing.rs`).
- `PO-BED-3` / `S3(e)` (`BD-106`): `test_ob_seal_chunk_receipts_resume_after_hard_kill_request_drain_finalize` (`src/receipt.rs`).
