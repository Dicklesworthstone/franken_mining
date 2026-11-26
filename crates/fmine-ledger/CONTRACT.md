# CONTRACT.md — `fmine-ledger`

## 1. Purpose
The ledger is Bedrock's L1 authority store (plan §26). It provides:
- `mmr` (BD-083): RFC 6962 Merkle Mountain Range over batch digests with inclusion and consistency proofs (`verify_inclusion`, `verify_consistency`).
- `batch` (BD-081/BD-086): `FMT-LEDGER-1` canonical delta batch codec (`FMNE` + `LDGR` + CRC32C, strict canonical ascending order, SHA-256 chain digests), with opt-in version 2 key-state read witnesses and preserved version 0/1 bytes.
- `segment` (BD-082): `FMT-SEG-1` append-only segment files (`SEGH` header, length-prefixed `LDGR` batch frames, `SEGF` MMR peak footer, advisory OS locking, exhaustive torn-tail inspection).
- `cell` (BD-084): single-writer `CellLease` (`FMNE` + `LEAS`), monotonic fencing epochs (`PO-BED-1`), split-brain lower-epoch quarantine (`F6`), and cross-cell gluing verification (`F12`).
- `verify` (BD-088): read-only hash-chain/MMR verification and digest-bound `F7` torn-tail repair plan/apply (`PO-ADIT-3`) that archives complete pre-repair bytes before truncating to the last verified record.
- **FACT (BD-085/BD-086):** `l0` checks declared key-state witnesses against the pre-batch index; `segment::SemanticSegmentWriter` combines those checks, append-only key/obligation semantics and an optional application-law callback with synced persistence and index reconstruction on reopen.

- **Crate:** `fmine-ledger`
- **Layer:** S1
- **Subsystem:** Bedrock
- **LOC budget:** 24,000 planned / 28,000 ceiling
- **Allowed Module Families:** `["mmr", "batch", "segment", "cell", "verify", "l0", "as_of_replay"]`

## 2. Inputs
- **MMR:** batch digests via `Mmr::push(&[u8])`, plus sizes, indices, roots, and proofs (`Digest32`).
- **Batches (`FMT-LEDGER-1`):** `DeltaBatch` structs or raw bytes (`FMNE` + `LDGR` + CRC32C) with embedded `LedgerAnchor`, `DeltaOp`, `Witness`, and `CrossCellRef` sequences.
- **Segments (`FMT-SEG-1`):** filesystem paths (`SegmentWriter`, `SemanticSegmentWriter`, `SegmentReader`) or raw byte slices (`inspect_segment_bytes`).
- **Cell authority & gluing:** `CellLease`, `DeltaBatch`, and enterprise `BTreeMap<String, Vec<Digest32>>` anchor views.
- **Verification & repair:** segment path plus expected canonical SHA-256 `plan_digest` (`apply_segment_repair`).
- **Dependencies:** `fmine-core` and `fmine-evidence` (strictly `S1 → S0`).

## 3. Outputs
- `mmr::{Mmr, MmrError, leaf_hash, empty_root, root_from_peaks, verify_inclusion, verify_consistency}`.
- `batch::{DeltaBatch, DeltaOp, DeltaKind, Witness, WitnessKind, CrossCellRef, LedgerError, GENESIS_PREV_HASH, FMT_LEDGER_V1, FMT_LEDGER_V2}`.
- `segment::{SegmentHeader, SegmentFooter, SegmentWriter, SemanticSegmentWriter, SegmentReader, SegmentInspection, SegmentState, inspect_segment_bytes}`.
- `cell::{CellLease, CellAuthority, ConflictRecord, AdmitOutcome, GluingIssue, GluingReport, check_gluing}`.
- `verify::{SegmentVerification, RepairAction, SegmentRepairPlan, SegmentRepairReceipt, verify_segment, plan_segment_repair, apply_segment_repair, retain_repair_evidence}`
- `l0::{L0Ledger, L0Error, KeyVersion, CellSnapshot}` (`BD-080`: L0 in-memory reference ledger and semantic oracle for L1 parity)
- `as_of_replay::{AsOfReplayError, ReplayRun, run_as_of, assert_bit_identical_replay}` (`BD-053`: bit-identical as-of replay harness bound by `FMT-CERT-1`).

## 4. Invariants
- **RFC 6962 MMR parity:** all 196 `transparency-dev/merkle` inclusion/consistency probes, 64-leaf exhaustive verification, and bit-flip tampering refusals pass.
- **Canonical batch framing (`BD-081`):** `DeltaBatch::from_bytes` accepts only strictly ascending sequences, valid `SHA-256(payload)` digests, matching `(cell_id, epoch, seq)` on the embedded `LedgerAnchor`, valid CRC32C, and zero trailing bytes. Re-encoding any accepted frame is byte-identical.
- **Append-only chain & MMR footer (`BD-082`):** every batch in a segment chains `seq`, `prev_hash`, and monotonic `epoch`. Sealed `SEGF` footers must match the replayed `record_count`, `head_hash`, `mmr_root`, and `mmr_peaks`.
- **FACT — declared key-state reads (`BD-086`):** `L0Ledger::key_witness` binds the cell, key, presence and latest version's sequence, epoch, operation and payload digest in a domain-separated canonical digest. Every `KeyState` witness is checked before any batch delta is applied. An unrelated key write preserves the witness; an absent-to-present transition or a same-payload supersession invalidates it. Duplicate key-state subjects and use in version 0/1 frames are refused. Historical frames remain readable without implying that they declared a read set.
- **FACT — durable semantic admission (`BD-085`):** `SemanticSegmentWriter` checks structural continuity, key-state witnesses, key/obligation operations and the caller's law before appending. The index advances only after the frame is synced. A semantic or law refusal changes neither bytes nor index. An uncertain persistence error disables further appends and sealing on that handle; reopening inspects and replays the durable history under the same exclusive file lock. A complete unacknowledged append replays once, and its original sequence cannot be appended twice.
- **FACT — bounded semantic replay:** `SegmentInspection::replay_semantics` checks the already inspected prefix and returns its L0 snapshot, including key versions and open obligations. Replay requires a complete genesis segment, checks the resulting head and MMR, and refuses a continuation without its preceding state. A successful prefix replay does not make a torn segment clean.
- **Single-writer fencing & split-brain quarantine (`PO-BED-1`, `F6`):** `CellAuthority::transfer_lease` strictly increments `epoch + 1`; direct commits with a stale lease refuse with `EpochRegression`; lower-epoch batches received by a replica are quarantined into `ConflictRecord` without mutating authority state.
- **Cross-cell gluing (`F12`):** `check_gluing` flags every missing cell, out-of-range sequence, or digest mismatch.
- **Torn tail or interior (`F7`, `INV-003`):** bytes that do not form a verified record are a torn tail unless a whole record starts at or after them (a length prefix, in range, then a batch or footer frame that passes its CRC); then they are interior corruption. The CRC alone decides, so a last record that this build cannot decode, such as a newer format version, is refused rather than truncated. A lost length prefix with the frame persisted, or a partly written batch with its embedded anchor frame, is still a torn tail; a damaged interior length that points past the end of the file is never truncated. The search for a whole record checks CRCs over at most twice the bytes from the damage to the end of the file, so a crafted tail of overlapping candidate frames cannot make inspection quadratic; a tail over that budget is refused too.
- **Non-destructive F7 repair (`BD-088`, `PO-ADIT-3`, `INV-003`):** interior corruption, chain breaks, and footer mismatches are refused at plan and apply time. `apply_segment_repair` refuses any mismatched `plan_digest` before touching disk, keeps the full original bytes at `<path>.torn.<sha12>.evidence` before truncating (`retain_repair_evidence`: written to `.partial`, synced and renamed into place; a retry after a crash keeps an archive that already holds those bytes, syncing it and its directory again before truncating, and refuses any other file there, or a symbolic link or, on Unix, a hard link that could lead to the target and be truncated with it), and re-verifies the truncated prefix from byte 0.

## 5. Refusals
- `MmrError` for pure MMR proof/range checks.
- `LedgerError::{Codec, InvalidEnumTag, InvalidField, PayloadDigestMismatch, AnchorMismatch, ChainContinuity, EpochRegression, LeaseViolation, FooterMismatch, InteriorCorruption, RepairDigestMismatch, Io}`.
- `LedgerError::to_refusal()` maps `EpochRegression` and `LeaseViolation` to `FMINE-REF-502`, codec/integrity/chain/footer/repair-digest failures to `FMINE-REF-505`, and filesystem/lock errors to `FMINE-REF-506`.
- **FACT:** `L0Error::KeyWitnessMismatch` maps a stale declared key read to `FMINE-REF-502`; the failed key and expected/actual digests remain available in the typed error. Application-law failures use the existing conservation refusal.

## 6. Determinism class
- `src/mmr.rs`, `src/batch.rs`, and `src/cell.rs` are `BitwisePortable` (integers, canonical framing, and SHA-256 only).
- `src/segment.rs` and `src/verify.rs` perform local filesystem I/O and advisory locking (`LockedFile`) around the pure `inspect_segment_bytes` scanner; plan digests over identical path/bytes are byte-identical.

## 7. Evidence class of outputs
Not applicable: Bedrock emits authority storage, proofs, and verification receipts rather than physical-quantity `Claim`s.

## 8. Validity domain
- Up to `u64::MAX` MMR leaves (bounded by host memory), `MAX_FRAME_BYTES` (64 MiB) per frame, `MAX_SEGMENT_BYTES` (256 MiB) per single-segment bounded read, and `MAX_MMR_PEAKS` (64) peaks per footer.
- **FACT:** the `FMT-LEDGER-1` codec supports versions `0..=2`; only version 2 permits `WitnessKind::KeyState`. The `FMT-SEG-1` codec continues to support versions `0..=1`.
- Single-segment files only today; multi-segment rotation/compaction and `SyncCert` network transport live in subsequent WP-04/WP-05 beads.
- **DESIGN:** key witnesses certify only the keys the caller declares. Range reads, aggregate predicates, cross-cell dependencies, legacy witness digests and signatures are not certified by this mechanism. The durable writer performs receiver-style admission; its embedding authority must authenticate the persisted writer lease and supply application laws. Reopening cannot re-evaluate an application law that was not persisted with a verifier.

## 9. Oracles
- `fixtures/fmine-ledger/rfc6962_probes.txt`: `transparency-dev/merkle` commit `c39fbf350a405f68571bf4c42a6a518eb0a46a4b` (Apache-2.0) inclusion and consistency probes.
- Known-answer unit tests in `batch.rs`, `segment.rs`, `cell.rs`, and `verify.rs`, including exhaustive byte-by-byte post-header truncation recovery and 1-bit flip refusal sweeps.
- **FACT:** `l0::tests::key_witness_known_answers_bind_cell_key_absence_and_full_version` pins independently encoded absent/present witness digests; `batch::tests::key_state_schema_is_opt_in_and_legacy_bytes_remain_pinned` pins legacy version 0/1 bytes. Semantic tests cover unrelated writes, stale absence, same-payload supersession, pre-batch ordering and refusal atomicity.
- **FACT:** `segment::tests::semantic_writer_reopens_with_exact_l0_history_obligations_and_read_witnesses` compares durable reopen with L0; `semantic_open_refuses_hash_valid_stale_history_without_changing_it` distinguishes chain integrity from semantic validity; `unacknowledged_complete_append_replays_once_and_retries_cannot_duplicate_it` and `failed_storage_write_requires_reopen_and_never_advances_the_semantic_index` exercise real synced storage and an OS write failure.
- `crates/fmine-verify/tests/f7_lab.rs` (T layer): the `F7` lab test. A crash at every byte of two appends (into an empty segment, and one over two sectors long) and of a seal, and `LabRuntime` seeds 0..999 that also lose sectors of the write in flight (zeros or stale bytes) and cut the repair short once; each state is detected, refused for writes, repaired with its evidence kept, and resumed to bytes identical to a crash-free segment.
- Fuzz target `fuzz/fuzz_targets/ledger.rs` for `FMT-LEDGER-1`, and `fuzz/fuzz_targets/segment.rs` for the `FMT-SEG-1` scanner: crash states must read as a torn tail at the old end, damage inside one record with a whole record after it must never read as torn, and every torn tail's verified prefix must inspect clean.

## 10. Known gaps
- **DESIGN (BD-085/BD-086 remain partial):** the new keyed/durable path does not complete the multi-worker combiner, persistent lease authentication, signature/conservation/gluing witness verification, multi-segment semantic replay, or crash-lab/performance gates. Raw `SegmentWriter` remains a structural primitive and may persist a semantically invalid history; callers needing semantic admission must use `SemanticSegmentWriter`.
- **OPEN (BD-081):** a fresh 24-hour ledger fuzz run is required for the extended version 2 decoder. The pre-version-2 campaign does not validate the new witness kind. The seed generator now includes version 2 frames and 512-byte key subjects, plus version 1/unsupported-version negative controls; no new 24-hour result is claimed.
- No multi-segment directory catalog, `FMT-CAS-1` chunk store (`BD-090`), or `SyncCert` wire protocol (`BD-103`) yet. `LabRuntime` DPOR over lease transfer (`PO-BED-1`, `BD-101`) is not built; the `F7` lab test covers crash recovery only.
- A torn batch whose payload holds a CRC-valid `FMT-LEDGER-1` batch or segment footer frame, with its length prefix, reads as interior corruption and is refused, not repaired: the scanner cannot tell that frame from a record written after the damage.
- OPEN (owner): damage to the last acknowledged records with no whole record after it, such as bit rot rather than a crash, reads as a torn tail, and `repair apply` truncates it with the complete bytes kept as evidence. That includes a damaged length prefix on an otherwise whole last batch or footer: its frame still verifies, but its length cannot be read, as when a crash loses the sector holding an unacknowledged write's length. Nothing in a segment tells the two causes apart.
- Self-verified only; no independent external review or multi-ISA CI run of the new modules yet.

## 11. Public API
- `mmr::{Mmr, MmrError, leaf_hash, empty_root, root_from_peaks, verify_inclusion, verify_consistency}`
- `batch::{DeltaBatch, DeltaOp, DeltaKind, Witness, WitnessKind, CrossCellRef, LedgerError, TAG_LEDGER_BATCH, FMT_LEDGER_V1, FMT_LEDGER_V2, FMT_LEDGER_MIN_VERSION, GENESIS_PREV_HASH}`
- `segment::{SegmentHeader, SegmentFooter, SegmentWriter, SemanticSegmentWriter, SegmentReader, SegmentInspection, SegmentState, inspect_segment_bytes, TAG_SEGMENT_HEADER, TAG_SEGMENT_FOOTER, FMT_SEG_V1}`
- `cell::{CellLease, CellAuthority, ConflictRecord, AdmitOutcome, GluingIssue, GluingReport, check_gluing, TAG_CELL_LEASE, FMT_LEASE_V1}`
- `verify::{SegmentVerification, RepairAction, SegmentRepairPlan, SegmentRepairReceipt, verify_segment, plan_segment_repair, apply_segment_repair, retain_repair_evidence}`
- `l0::{L0Ledger, L0Error, KeyVersion, CellSnapshot}` (`BD-080`: L0 in-memory reference ledger and semantic oracle for L1 parity)
- `as_of_replay::{AsOfReplayError, ReplayRun, run_as_of, assert_bit_identical_replay}` (`BD-053`: bit-identical as-of replay harness bound by `FMT-CERT-1`)

## 12. Proof obligations
- `PO-BED-1` (single-writer fencing + lower-epoch quarantine): unit-tested in `cell::tests::test_po_bed_1_lease_transfer_fencing_and_split_brain_quarantine` and `cell::tests::test_po_bed_1_one_holder_per_epoch_refuses_leases_the_cell_did_not_grant` (LabRuntime DPOR remains open under `BD-101`).
- `F7` / `PO-ADIT-3` (digest-bound torn-tail repair with pre-truncation evidence retention): unit-tested in `verify::tests::test_f7_torn_tail_repair_retains_evidence_enforces_digest_and_resumes_cleanly` and `verify::tests::test_f7_repair_completes_after_a_crash_during_repair`; lab-tested in `crates/fmine-verify/tests/f7_lab.rs`.
