# CONTRACT.md — `fmine-ledger`

## 1. Purpose
The ledger is Bedrock's L1 authority store (plan §26). It provides:
- `mmr` (BD-083): RFC 6962 Merkle Mountain Range over batch digests with inclusion and consistency proofs (`verify_inclusion`, `verify_consistency`).
- `batch` (BD-081): `FMT-LEDGER-1` canonical delta batch codec (`FMNE` + `LDGR` + CRC32C, strict canonical ascending order, SHA-256 chain digests, N/N-1 versioning).
- `segment` (BD-082): `FMT-SEG-1` append-only segment files (`SEGH` header, length-prefixed `LDGR` batch frames, `SEGF` MMR peak footer, advisory OS locking, exhaustive torn-tail inspection).
- `cell` (BD-084): single-writer `CellLease` (`FMNE` + `LEAS`), monotonic fencing epochs (`PO-BED-1`), split-brain lower-epoch quarantine (`F6`), and cross-cell gluing verification (`F12`).
- `verify` (BD-088): read-only hash-chain/MMR verification and digest-bound `F7` torn-tail repair plan/apply (`PO-ADIT-3`) that archives complete pre-repair bytes before truncating to the last verified record.

- **Crate:** `fmine-ledger`
- **Layer:** S1
- **Subsystem:** Bedrock
- **LOC budget:** 24,000 planned / 28,000 ceiling
- **Allowed Module Families:** `["mmr", "batch", "segment", "cell", "verify", "l0", "as_of_replay"]`

## 2. Inputs
- **MMR:** batch digests via `Mmr::push(&[u8])`, plus sizes, indices, roots, and proofs (`Digest32`).
- **Batches (`FMT-LEDGER-1`):** `DeltaBatch` structs or raw bytes (`FMNE` + `LDGR` + CRC32C) with embedded `LedgerAnchor`, `DeltaOp`, `Witness`, and `CrossCellRef` sequences.
- **Segments (`FMT-SEG-1`):** filesystem paths (`SegmentWriter`, `SegmentReader`) or raw byte slices (`inspect_segment_bytes`).
- **Cell authority & gluing:** `CellLease`, `DeltaBatch`, and enterprise `BTreeMap<String, Vec<Digest32>>` anchor views.
- **Verification & repair:** segment path plus expected canonical SHA-256 `plan_digest` (`apply_segment_repair`).
- **Dependencies:** `fmine-core` and `fmine-evidence` (strictly `S1 → S0`).

## 3. Outputs
- `mmr::{Mmr, MmrError, leaf_hash, empty_root, root_from_peaks, verify_inclusion, verify_consistency}`.
- `batch::{DeltaBatch, DeltaOp, DeltaKind, Witness, WitnessKind, CrossCellRef, LedgerError, GENESIS_PREV_HASH, FMT_LEDGER_V1}`.
- `segment::{SegmentHeader, SegmentFooter, SegmentWriter, SegmentReader, SegmentInspection, SegmentState, inspect_segment_bytes}`.
- `cell::{CellLease, CellAuthority, ConflictRecord, AdmitOutcome, GluingIssue, GluingReport, check_gluing}`.
- `verify::{SegmentVerification, RepairAction, SegmentRepairPlan, SegmentRepairReceipt, verify_segment, plan_segment_repair, apply_segment_repair, retain_repair_evidence}`
- `l0::{L0Ledger, L0Error, KeyVersion, CellSnapshot}` (`BD-080`: L0 in-memory reference ledger and semantic oracle for L1 parity)
- `as_of_replay::{AsOfReplayError, ReplayRun, run_as_of, assert_bit_identical_replay}` (`BD-053`: bit-identical as-of replay harness bound by `FMT-CERT-1`).

## 4. Invariants
- **RFC 6962 MMR parity:** all 196 `transparency-dev/merkle` inclusion/consistency probes, 64-leaf exhaustive verification, and bit-flip tampering refusals pass.
- **Canonical batch framing (`BD-081`):** `DeltaBatch::from_bytes` accepts only strictly ascending sequences, valid `SHA-256(payload)` digests, matching `(cell_id, epoch, seq)` on the embedded `LedgerAnchor`, valid CRC32C, and zero trailing bytes. Re-encoding any accepted frame is byte-identical.
- **Append-only chain & MMR footer (`BD-082`):** every batch in a segment chains `seq`, `prev_hash`, and monotonic `epoch`. Sealed `SEGF` footers must match the replayed `record_count`, `head_hash`, `mmr_root`, and `mmr_peaks`.
- **Single-writer fencing & split-brain quarantine (`PO-BED-1`, `F6`):** `CellAuthority::transfer_lease` strictly increments `epoch + 1`; direct commits with a stale lease refuse with `EpochRegression`; lower-epoch batches received by a replica are quarantined into `ConflictRecord` without mutating authority state.
- **Cross-cell gluing (`F12`):** `check_gluing` flags every missing cell, out-of-range sequence, or digest mismatch.
- **Torn tail or interior (`F7`, `INV-003`):** bytes that do not form a verified record are a torn tail unless a whole record follows them (a length prefix in range, then a batch or footer frame that passes its CRC); then they are interior corruption. A lost length prefix with the frame persisted, or a partly written batch with its embedded anchor frame, is still a torn tail; a damaged interior length that points past the end of the file is never truncated. The search for a whole record decodes at most twice the bytes from the damage to the end of the file, so a crafted tail of overlapping candidate frames cannot make inspection quadratic; a tail over that budget is refused too.
- **Non-destructive F7 repair (`BD-088`, `PO-ADIT-3`, `INV-003`):** interior corruption, chain breaks, and footer mismatches are refused at plan and apply time. `apply_segment_repair` refuses any mismatched `plan_digest` before touching disk, keeps the full original bytes at `<path>.torn.<sha12>.evidence` before truncating (`retain_repair_evidence`: written to `.partial`, synced and renamed into place; a retry after a crash keeps an archive that already holds those bytes, syncing it and its directory again before truncating, and refuses any other file there), and re-verifies the truncated prefix from byte 0.

## 5. Refusals
- `MmrError` for pure MMR proof/range checks.
- `LedgerError::{Codec, InvalidEnumTag, InvalidField, PayloadDigestMismatch, AnchorMismatch, ChainContinuity, EpochRegression, LeaseViolation, FooterMismatch, InteriorCorruption, RepairDigestMismatch, Io}`.
- `LedgerError::to_refusal()` maps `EpochRegression` and `LeaseViolation` to `FMINE-REF-502`, codec/integrity/chain/footer/repair-digest failures to `FMINE-REF-505`, and filesystem/lock errors to `FMINE-REF-506`.

## 6. Determinism class
- `src/mmr.rs`, `src/batch.rs`, and `src/cell.rs` are `BitwisePortable` (integers, canonical framing, and SHA-256 only).
- `src/segment.rs` and `src/verify.rs` perform local filesystem I/O and advisory locking (`LockedFile`) around the pure `inspect_segment_bytes` scanner; plan digests over identical path/bytes are byte-identical.

## 7. Evidence class of outputs
Not applicable: Bedrock emits authority storage, proofs, and verification receipts rather than physical-quantity `Claim`s.

## 8. Validity domain
- Up to `u64::MAX` MMR leaves (bounded by host memory), `MAX_FRAME_BYTES` (64 MiB) per frame, `MAX_SEGMENT_BYTES` (256 MiB) per single-segment bounded read, and `MAX_MMR_PEAKS` (64) peaks per footer.
- `FMT-LEDGER-1` and `FMT-SEG-1` support versions `0..=1` (`N` and `N-1`).
- Single-segment files only today; multi-segment rotation/compaction and `SyncCert` network transport live in subsequent WP-04/WP-05 beads.

## 9. Oracles
- `fixtures/fmine-ledger/rfc6962_probes.txt`: `transparency-dev/merkle` commit `c39fbf350a405f68571bf4c42a6a518eb0a46a4b` (Apache-2.0) inclusion and consistency probes.
- Known-answer unit tests in `batch.rs`, `segment.rs`, `cell.rs`, and `verify.rs`, including exhaustive byte-by-byte post-header truncation recovery and 1-bit flip refusal sweeps.
- `crates/fmine-verify/tests/f7_lab.rs` (T layer): the `F7` lab test. A crash at every byte of an append and of a seal, and `LabRuntime` seeds 0..999 that also lose sectors of the write in flight (zeros or stale bytes) and cut the repair short once; each state is detected, refused for writes, repaired with its evidence kept, and resumed to bytes identical to a crash-free segment.
- Fuzz target `fuzz/fuzz_targets/ledger.rs` for `FMT-LEDGER-1`.

## 10. Known gaps
- `BD-081`'s 24-hour fuzz run for `fuzz/fuzz_targets/ledger.rs` started on 2026-11-25 and has no result yet (`bd-081` remains `in_progress`).
- No multi-segment directory catalog, `FMT-CAS-1` chunk store (`BD-090`), or `SyncCert` wire protocol (`BD-103`) yet. `LabRuntime` DPOR over lease transfer (`PO-BED-1`, `BD-101`) is not built; the `F7` lab test covers crash recovery only.
- Self-verified only; no independent external review or multi-ISA CI run of the new modules yet.

## 11. Public API
- `mmr::{Mmr, MmrError, leaf_hash, empty_root, root_from_peaks, verify_inclusion, verify_consistency}`
- `batch::{DeltaBatch, DeltaOp, DeltaKind, Witness, WitnessKind, CrossCellRef, LedgerError, TAG_LEDGER_BATCH, FMT_LEDGER_V1, FMT_LEDGER_MIN_VERSION, GENESIS_PREV_HASH}`
- `segment::{SegmentHeader, SegmentFooter, SegmentWriter, SegmentReader, SegmentInspection, SegmentState, inspect_segment_bytes, TAG_SEGMENT_HEADER, TAG_SEGMENT_FOOTER, FMT_SEG_V1}`
- `cell::{CellLease, CellAuthority, ConflictRecord, AdmitOutcome, GluingIssue, GluingReport, check_gluing, TAG_CELL_LEASE, FMT_LEASE_V1}`
- `verify::{SegmentVerification, RepairAction, SegmentRepairPlan, SegmentRepairReceipt, verify_segment, plan_segment_repair, apply_segment_repair}`
- `l0::{L0Ledger, L0Error, KeyVersion, CellSnapshot}` (`BD-080`: L0 in-memory reference ledger and semantic oracle for L1 parity)
- `as_of_replay::{AsOfReplayError, ReplayRun, run_as_of, assert_bit_identical_replay}` (`BD-053`: bit-identical as-of replay harness bound by `FMT-CERT-1`)

## 12. Proof obligations
- `PO-BED-1` (single-writer fencing + lower-epoch quarantine): unit-tested in `cell::tests::test_po_bed_1_lease_transfer_fencing_and_split_brain_quarantine` and `cell::tests::test_po_bed_1_one_holder_per_epoch_refuses_leases_the_cell_did_not_grant` (LabRuntime DPOR remains open under `BD-101`).
- `F7` / `PO-ADIT-3` (digest-bound torn-tail repair with pre-truncation evidence retention): unit-tested in `verify::tests::test_f7_torn_tail_repair_retains_evidence_enforces_digest_and_resumes_cleanly` and `verify::tests::test_f7_repair_completes_after_a_crash_during_repair`; lab-tested in `crates/fmine-verify/tests/f7_lab.rs`.
