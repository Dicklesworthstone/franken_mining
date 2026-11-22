# CONTRACT.md — `fmine-telemetry`

## 1. Purpose
Headframe S1 site telemetry segment log, bounded ingest backpressure with mandatory continuity receipts, raw ring-buffer retention, and 1-minute deterministic rollups (`BD-242`, plan §26.7–§26.8, `PO-BED-7`):
- `segment`: columnar fixed-point telemetry segment batching (`TelemetrySample`, `SignalColumn`, `TelemetrySegment`), deterministic 250 ms timer / capacity sealing, canonical SHA-256 `segment_root` integrity verification (`FMINE-REF-505`), and `(segment_root, signal_id, start_offset, end_offset)` trace references linking raw high-rate traces to ledger transition facts;
- `backpressure`: bounded ingest edge (`BoundedIngestEdge`, `IngestPolicy`), early segment sealing under overload, deterministic per-signal downsampling emitting cryptographically hashed `ContinuityReceipt`s (`admitted_count + dropped_count == total_offered`, never dropping data silently), and a bounded `RawRingBuffer` retaining original raw samples on site while local budget allows;
- `rollup`: fixed-window (default 1-minute) deterministic per-signal rollups (`SignalRollup`) with exact `ExactQty` mean derivation, sample-count conservation across segments, compact 16-byte wire representation for low-bandwidth replication, and refusal on mixed fixed-point scales (`FMINE-REF-105`).

- **Crate:** `fmine-telemetry`
- **Layer:** S1
- **Subsystem:** Headframe
- **LOC budget:** 16,000 planned / 18,000 ceiling
- **Allowed Module Families:** `["segment", "backpressure", "rollup"]`

## 2. Inputs
- Fixed-point scaled `TelemetrySample` batches (`signal_id`, monotonic `timestamp_ns`, `value_scaled`, `scale_exp10`, `quality_flags`) and validated `IngestPolicy` / `SegmentPolicy` / `RollupConfig` configurations.
- Dependencies: `fmine-core`, `fmine-units`, `fmine-time`, `fmine-evidence` (`S1 → S0`, zero external crates). Performs zero direct network/filesystem I/O, zero clocks, and zero threads.

## 3. Outputs
- Sealed columnar `TelemetrySegment`s with SHA-256 `segment_root`s, verified `SegmentTraceRef` trace slices, cryptographically hashed `ContinuityReceipt`s accounting for every offered sample, `RawRingBuffer` snapshots, and 1-minute `SignalRollup` records with exact `ExactQty` means.
- Emits no `Claim`s directly and holds no write path to ISA-95 Levels 0–2 (`INV-011`).

## 4. Invariants
- **No silent drops (plan §26.8, `PO-BED-7`):** every sample offered to `BoundedIngestEdge` is accounted for in either a sealed `TelemetrySegment` (`admitted_count`) or an explicit `ContinuityReceipt` (`dropped_count`), verified by `audit_continuity` (`admitted_count + dropped_count == total_offered`).
- **Cryptographic tamper detection:** `TelemetrySegment::verify_root` and `ContinuityReceipt::verify_digest` recompute canonical SHA-256 digests and refuse any byte/metadata tampering with `FMINE-REF-505`.
- **Trace reference integrity:** `slice_trace` checks both `segment_root` equality and `start_offset < end_offset <= column.len()` before returning raw evidence samples for a ledger transition fact.
- **Scale & conservation integrity in rollups:** mixing different `scale_exp10` values for the same `signal_id` refuses with `FMINE-REF-105`; total rollup `sample_count` equals total sealed segment sample count exactly.
- **Monotonic per-signal timestamps:** backward timestamps within a signal column are refused (`FMINE-REF-107`).

## 5. Refusals
- `TelemetryError::InvalidConfig` / `TelemetryError::InvalidSample` → `FMINE-REF-109` (exit 3): zero capacities/intervals, invalid `downsample_stride < 2`, or arithmetic overflow.
- `TelemetryError::OutOfOrderTimestamp` → `FMINE-REF-107` (exit 3): non-monotonic timestamp within a signal stream or empty `site_id`.
- `TelemetryError::ScaleMismatch` → `FMINE-REF-105` (exit 3): conflicting `scale_exp10` within the same `signal_id`.
- `TelemetryError::InvalidTraceRef` → `FMINE-REF-201` (exit 3): trace reference points to a mismatched segment root, missing signal, or out-of-range offset.
- `TelemetryError::DigestMismatch` → `FMINE-REF-505` (exit 8): tampered segment or continuity receipt digest.

## 6. Determinism class
All files (`lib.rs`, `segment.rs`, `backpressure.rs`, `rollup.rs`) declare `//! Determinism: BitwisePortable`. Arithmetic uses integer fixed-point (`i64`/`i128`) and exact `ExactQty` rationals over `BTreeMap` collections; hashes use deterministic `fmine_core::hash::sha256`.

## 7. Evidence class of outputs
Not applicable: S1 site telemetry storage/rollup infrastructure; upstream Headframe engines wrap operational summaries in `Claim`s with `EvidenceClass` and provenance.

## 8. Validity domain
- High-rate site telemetry streams (e.g. 10 Hz truck payload weighing, plant tags) encoded in signed 64-bit fixed-point with `scale_exp10 ∈ -12..=12`, up to `u64::MAX` nanosecond timestamps and `i128` rollup sums.

## 9. Oracles
- Deterministic segment seal/root/tamper and trace-slicing tests in `segment.rs`; overload early-seal, downsampling, raw ring retention, and conservation-verified `ContinuityReceipt` tests in `backpressure.rs`; 1-minute rollup exact-mean, compact 16-byte wire codec, and scale-mismatch refusal tests in `rollup.rs`.

## 10. Known gaps
- High-throughput endurance harness (`10^5 msg/s` for 1 h, `BD-243`) and disk-backed WAL/compaction I/O adapter sit in subsequent WP-17 beads.

## 11. Public API
- `TelemetryError`
- `segment::{DEFAULT_SEAL_INTERVAL_NS, DEFAULT_MAX_SEGMENT_BYTES, SealReason, TelemetrySample, SignalColumn, TelemetrySegment, SegmentTraceRef, SegmentPolicy, SegmentLogBuilder, slice_trace}`
- `backpressure::{DropReason, IngestPolicy, ContinuityReceipt, RawRingBuffer, IngestBatchOutcome, BoundedIngestEdge, ContinuityAuditReport, audit_continuity}`
- `rollup::{DEFAULT_ROLLUP_WINDOW_NS, RollupConfig, SignalRollup, CompactRollup16, compute_rollups}`

## 12. Proof obligations
- `BD-242` / `PO-BED-7` (continuity receipts tested, zero silent drops under overload): `backpressure::tests::test_overload_early_seal_downsampling_ring_retention_and_continuity_receipts`.
- Segment root tamper resistance & transition-to-trace linkage: `segment::tests::test_segment_sealing_root_verification_and_trace_refs`.
- 1-minute rollup conservation & scale safety: `rollup::tests::test_one_minute_rollups_exact_means_compact_16b_and_scale_refusal`.
