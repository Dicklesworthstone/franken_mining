//! Determinism: BitwisePortable
//! Bounded telemetry ingest backpressure, early segment sealing, deterministic downsampling with
//! cryptographically hashed `ContinuityReceipt`s, and raw ring-buffer retention (`BD-242`, plan
//! §26.8, `PO-BED-7`).
//!
//! Core law: **data is never dropped silently**. Under overload, the open segment seals early,
//! deterministic downsampling engages, raw samples are retained in the on-site ring buffer while
//! budget allows, and every dropped sample is accounted for in a signed/hashed `ContinuityReceipt`
//! satisfying exact conservation (`admitted_count + dropped_count == total_offered`).

use std::collections::{BTreeMap, VecDeque};

use fmine_core::Digest32;
use fmine_core::hash::sha256;

use crate::TelemetryError;
use crate::segment::{
    ESTIMATED_BYTES_PER_SAMPLE, SealReason, SegmentLogBuilder, SegmentPolicy, TelemetrySample,
    TelemetrySegment,
};

/// Canonical domain prefix for continuity receipt SHA-256 digests.
const RECEIPT_DOMAIN_PREFIX: &[u8] = b"fmine:continuity-receipt:v1\0";

/// Explicit reason why samples were excluded from the sealed segment stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum DropReason {
    /// Ingest queue exceeded bound: sealed open segment early and kept 1-of-`k` per signal.
    QueueOverloadDownsampled = 1,
    /// Hard queue saturation after downsampling: excess samples rejected with receipt.
    QueueHardLimitRejected = 2,
}

impl DropReason {
    #[must_use]
    pub const fn tag_byte(self) -> u8 {
        self as u8
    }
}

/// Bounded ingest and backpressure policy (`BD-242`, plan §26.7–§26.8).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IngestPolicy {
    pub site_id: String,
    pub seal_interval_ns: u64,
    pub max_segment_samples: usize,
    pub max_segment_bytes: usize,
    /// Maximum incoming samples admitted without triggering overload backpressure in one batch.
    pub max_batch_admit_samples: usize,
    /// Deterministic stride `k >= 2`: under overload keep every `k`-th sample per signal (0, k, 2k, ...).
    pub downsample_stride: usize,
    /// Capacity of the local raw ring buffer retaining original samples while disk/RAM allows.
    pub ring_capacity_samples: usize,
}

impl IngestPolicy {
    /// Validate policy invariants (strict positive bounds; `downsample_stride >= 2`).
    pub fn validate(&self) -> Result<(), TelemetryError> {
        let seg_policy = SegmentPolicy {
            site_id: self.site_id.clone(),
            seal_interval_ns: self.seal_interval_ns,
            max_segment_samples: self.max_segment_samples,
            max_segment_bytes: self.max_segment_bytes,
        };
        seg_policy.validate()?;
        if self.max_batch_admit_samples == 0 {
            return Err(TelemetryError::InvalidConfig {
                field: "max_batch_admit_samples",
                detail: "max_batch_admit_samples must be > 0".to_string(),
            });
        }
        if self.downsample_stride < 2 {
            return Err(TelemetryError::InvalidConfig {
                field: "downsample_stride",
                detail: format!(
                    "downsample_stride must be >= 2 for downsampling, got {}",
                    self.downsample_stride
                ),
            });
        }
        if self.max_segment_bytes < ESTIMATED_BYTES_PER_SAMPLE {
            return Err(TelemetryError::InvalidConfig {
                field: "max_segment_bytes",
                detail: "max_segment_bytes too small".to_string(),
            });
        }
        Ok(())
    }
}

/// Cryptographically hashed continuity receipt stating exactly what was dropped, over which time
/// window, why, and how many raw samples remain retained in the local ring buffer (plan §26.8).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContinuityReceipt {
    pub site_id: String,
    pub receipt_seq: u64,
    /// Root of the segment sealed early upon overload (or all-zero `Digest32([0u8; 32])` if no open segment
    /// existed prior to overload).
    pub early_sealed_segment_root: Digest32,
    pub window_start_ns: u64,
    pub window_end_ns: u64,
    pub total_offered: u64,
    pub admitted_count: u64,
    pub dropped_count: u64,
    pub downsample_stride: u32,
    pub drop_reason: DropReason,
    /// Number of offered samples from this window captured in the local raw ring buffer.
    pub ring_retained_count: u64,
    /// Per-signal dropped counts in deterministic `signal_id` order.
    pub dropped_by_signal: BTreeMap<u32, u64>,
    pub receipt_digest: Digest32,
}

impl ContinuityReceipt {
    /// Compute canonical SHA-256 digest over all receipt fields.
    pub fn compute_digest(&self) -> Result<Digest32, TelemetryError> {
        let mut buf = Vec::new();
        buf.extend_from_slice(RECEIPT_DOMAIN_PREFIX);
        let site_len = u32::try_from(self.site_id.len()).map_err(|_| TelemetryError::Overflow)?;
        buf.extend_from_slice(&site_len.to_le_bytes());
        buf.extend_from_slice(self.site_id.as_bytes());
        buf.extend_from_slice(&self.receipt_seq.to_le_bytes());
        buf.extend_from_slice(self.early_sealed_segment_root.as_bytes());
        buf.extend_from_slice(&self.window_start_ns.to_le_bytes());
        buf.extend_from_slice(&self.window_end_ns.to_le_bytes());
        buf.extend_from_slice(&self.total_offered.to_le_bytes());
        buf.extend_from_slice(&self.admitted_count.to_le_bytes());
        buf.extend_from_slice(&self.dropped_count.to_le_bytes());
        buf.extend_from_slice(&self.downsample_stride.to_le_bytes());
        buf.push(self.drop_reason.tag_byte());
        buf.extend_from_slice(&self.ring_retained_count.to_le_bytes());
        let sig_count =
            u32::try_from(self.dropped_by_signal.len()).map_err(|_| TelemetryError::Overflow)?;
        buf.extend_from_slice(&sig_count.to_le_bytes());
        for (&sig_id, &cnt) in &self.dropped_by_signal {
            buf.extend_from_slice(&sig_id.to_le_bytes());
            buf.extend_from_slice(&cnt.to_le_bytes());
        }
        Ok(sha256(&buf))
    }

    /// Verify exact internal conservation (`admitted + dropped == total_offered`, `Σ dropped_by_signal == dropped`)
    /// AND SHA-256 receipt digest (`FMINE-REF-505` on mismatch).
    pub fn verify(&self) -> Result<(), TelemetryError> {
        let accounted = self
            .admitted_count
            .checked_add(self.dropped_count)
            .ok_or(TelemetryError::Overflow)?;
        if accounted != self.total_offered {
            return Err(TelemetryError::DigestMismatch {
                kind: "continuity_receipt",
                detail: format!(
                    "receipt {} conservation broken: admitted({}) + dropped({}) != offered({})",
                    self.receipt_seq, self.admitted_count, self.dropped_count, self.total_offered
                ),
            });
        }
        let mut sig_sum = 0_u64;
        for &c in self.dropped_by_signal.values() {
            sig_sum = sig_sum.checked_add(c).ok_or(TelemetryError::Overflow)?;
        }
        if sig_sum != self.dropped_count {
            return Err(TelemetryError::DigestMismatch {
                kind: "continuity_receipt",
                detail: format!(
                    "receipt {} per-signal dropped sum ({sig_sum}) != dropped_count ({})",
                    self.receipt_seq, self.dropped_count
                ),
            });
        }
        let computed = self.compute_digest()?;
        if computed == self.receipt_digest {
            Ok(())
        } else {
            Err(TelemetryError::DigestMismatch {
                kind: "continuity_receipt",
                detail: format!(
                    "receipt {} digest expected {}, computed {}",
                    self.receipt_seq,
                    self.receipt_digest.to_hex(),
                    computed.to_hex()
                ),
            })
        }
    }
}

/// Bounded local raw ring buffer retaining original high-rate samples while site budget allows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawRingBuffer {
    capacity: usize,
    buf: VecDeque<TelemetrySample>,
    evicted_count: u64,
}

impl RawRingBuffer {
    /// Construct a ring buffer with maximum `capacity` retained raw samples (`0` disables local ring).
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity,
            buf: VecDeque::with_capacity(capacity),
            evicted_count: 0,
        }
    }

    /// Push one raw sample into the ring buffer, evicting the oldest if at capacity.
    /// Returns `true` if the sample is retained (`capacity > 0`).
    pub fn push(&mut self, sample: TelemetrySample) -> bool {
        if self.capacity == 0 {
            return false;
        }
        if self.buf.len() == self.capacity {
            self.buf.pop_front();
            self.evicted_count = self.evicted_count.saturating_add(1);
        }
        self.buf.push_back(sample);
        true
    }

    /// Currently retained raw sample count.
    #[must_use]
    pub fn len(&self) -> usize {
        self.buf.len()
    }

    /// Returns `true` if no raw samples are retained.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.buf.is_empty()
    }

    /// Total number of older samples overwritten by ring wrap-around.
    #[must_use]
    pub const fn evicted_count(&self) -> u64 {
        self.evicted_count
    }

    /// Snapshot of currently retained raw samples in FIFO order.
    #[must_use]
    pub fn snapshot(&self) -> Vec<TelemetrySample> {
        self.buf.iter().copied().collect()
    }
}

/// Outcome of submitting one batch to [`BoundedIngestEdge`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IngestBatchOutcome {
    pub sealed_segments: Vec<TelemetrySegment>,
    pub continuity_receipt: Option<ContinuityReceipt>,
    pub admitted_count: u64,
    pub dropped_count: u64,
}

/// Bounded site telemetry ingest edge enforcing early sealing, deterministic downsampling, raw
/// ring-buffer retention, and mandatory [`ContinuityReceipt`] emission (plan §26.8).
#[derive(Debug, Clone)]
pub struct BoundedIngestEdge {
    policy: IngestPolicy,
    builder: SegmentLogBuilder,
    ring: RawRingBuffer,
    next_receipt_seq: u64,
    total_offered: u64,
    total_admitted: u64,
    total_dropped: u64,
}

impl BoundedIngestEdge {
    /// Construct a validated bounded ingest edge.
    pub fn new(policy: IngestPolicy) -> Result<Self, TelemetryError> {
        policy.validate()?;
        let seg_policy = SegmentPolicy {
            site_id: policy.site_id.clone(),
            seal_interval_ns: policy.seal_interval_ns,
            max_segment_samples: policy.max_segment_samples,
            max_segment_bytes: policy.max_segment_bytes,
        };
        let builder = SegmentLogBuilder::new(seg_policy)?;
        let ring = RawRingBuffer::new(policy.ring_capacity_samples);
        Ok(Self {
            policy,
            builder,
            ring,
            next_receipt_seq: 0,
            total_offered: 0,
            total_admitted: 0,
            total_dropped: 0,
        })
    }

    /// Read-only access to the local raw ring buffer.
    #[must_use]
    pub const fn raw_ring(&self) -> &RawRingBuffer {
        &self.ring
    }

    /// Cumulative samples offered to this ingest edge.
    #[must_use]
    pub const fn total_offered(&self) -> u64 {
        self.total_offered
    }

    /// Cumulative samples admitted into segments.
    #[must_use]
    pub const fn total_admitted(&self) -> u64 {
        self.total_admitted
    }

    /// Cumulative samples dropped with continuity receipts.
    #[must_use]
    pub const fn total_dropped(&self) -> u64 {
        self.total_dropped
    }

    /// Ingest a batch of raw telemetry samples.
    ///
    /// - Validates every sample before mutating state so malformed batches leave the edge consistent.
    /// - If `batch.len() <= policy.max_batch_admit_samples`, admits every sample into the segment
    ///   builder (and retains in the raw ring buffer) with `continuity_receipt = None`.
    /// - If `batch.len() > policy.max_batch_admit_samples`, triggers backpressure:
    ///   1. Seals any open segment early (`SealReason::EarlyBackpressureSeal`);
    ///   2. Retains raw samples in `RawRingBuffer` while capacity allows;
    ///   3. Deterministically downsamples per signal (`idx % downsample_stride == 0`), capping at
    ///      `max_batch_admit_samples` if even the downsampled stream exceeds the hard bound;
    ///   4. Emits a hashed [`ContinuityReceipt`] accounting for every dropped sample.
    pub fn ingest_batch(
        &mut self,
        batch: &[TelemetrySample],
    ) -> Result<IngestBatchOutcome, TelemetryError> {
        if batch.is_empty() {
            return Ok(IngestBatchOutcome {
                sealed_segments: Vec::new(),
                continuity_receipt: None,
                admitted_count: 0,
                dropped_count: 0,
            });
        }
        self.builder.validate_batch(batch)?;

        let offered_u64 = u64::try_from(batch.len()).map_err(|_| TelemetryError::Overflow)?;
        let mut sealed_segments = Vec::new();

        if batch.len() <= self.policy.max_batch_admit_samples {
            // Normal path: zero drops, full admission.
            for &sample in batch {
                sealed_segments.extend(self.builder.push(sample)?);
                self.ring.push(sample);
            }
            self.total_offered = self
                .total_offered
                .checked_add(offered_u64)
                .ok_or(TelemetryError::Overflow)?;
            self.total_admitted = self
                .total_admitted
                .checked_add(offered_u64)
                .ok_or(TelemetryError::Overflow)?;
            return Ok(IngestBatchOutcome {
                sealed_segments,
                continuity_receipt: None,
                admitted_count: offered_u64,
                dropped_count: 0,
            });
        }

        // Overload path: 1) early seal open segment.
        let early_root =
            if let Some(early_seg) = self.builder.seal_open(SealReason::EarlyBackpressureSeal)? {
                let root = early_seg.segment_root;
                sealed_segments.push(early_seg);
                root
            } else {
                Digest32([0u8; 32])
            };

        // 2) Deterministic per-signal downsampling + raw ring retention.
        let mut window_start_ns = u64::MAX;
        let mut window_end_ns = 0_u64;
        let mut per_signal_index: BTreeMap<u32, usize> = BTreeMap::new();
        let mut dropped_by_signal: BTreeMap<u32, u64> = BTreeMap::new();
        let mut admitted_count = 0_u64;
        let mut dropped_count = 0_u64;
        let mut ring_retained_count = 0_u64;
        let mut hit_hard_limit = false;
        let max_admit_u64 = u64::try_from(self.policy.max_batch_admit_samples)
            .map_err(|_| TelemetryError::Overflow)?;

        for &sample in batch {
            window_start_ns = window_start_ns.min(sample.timestamp_ns);
            window_end_ns = window_end_ns.max(sample.timestamp_ns);

            if self.ring.push(sample) {
                ring_retained_count = ring_retained_count
                    .checked_add(1)
                    .ok_or(TelemetryError::Overflow)?;
            }

            let sig_idx = per_signal_index.entry(sample.signal_id).or_insert(0);
            let keep_by_stride = (*sig_idx).is_multiple_of(self.policy.downsample_stride);
            *sig_idx += 1;

            if keep_by_stride && admitted_count < max_admit_u64 {
                sealed_segments.extend(self.builder.push(sample)?);
                admitted_count = admitted_count
                    .checked_add(1)
                    .ok_or(TelemetryError::Overflow)?;
            } else {
                if keep_by_stride && admitted_count >= max_admit_u64 {
                    hit_hard_limit = true;
                }
                dropped_count = dropped_count
                    .checked_add(1)
                    .ok_or(TelemetryError::Overflow)?;
                let entry = dropped_by_signal.entry(sample.signal_id).or_insert(0);
                *entry = entry.checked_add(1).ok_or(TelemetryError::Overflow)?;
            }
        }

        let receipt_seq = self.next_receipt_seq;
        self.next_receipt_seq = self
            .next_receipt_seq
            .checked_add(1)
            .ok_or(TelemetryError::Overflow)?;
        let stride_u32 =
            u32::try_from(self.policy.downsample_stride).map_err(|_| TelemetryError::Overflow)?;

        let drop_reason = if hit_hard_limit {
            DropReason::QueueHardLimitRejected
        } else {
            DropReason::QueueOverloadDownsampled
        };

        let mut receipt = ContinuityReceipt {
            site_id: self.policy.site_id.clone(),
            receipt_seq,
            early_sealed_segment_root: early_root,
            window_start_ns,
            window_end_ns,
            total_offered: offered_u64,
            admitted_count,
            dropped_count,
            downsample_stride: stride_u32,
            drop_reason,
            ring_retained_count,
            dropped_by_signal,
            receipt_digest: Digest32([0u8; 32]),
        };
        receipt.receipt_digest = receipt.compute_digest()?;

        self.total_offered = self
            .total_offered
            .checked_add(offered_u64)
            .ok_or(TelemetryError::Overflow)?;
        self.total_admitted = self
            .total_admitted
            .checked_add(admitted_count)
            .ok_or(TelemetryError::Overflow)?;
        self.total_dropped = self
            .total_dropped
            .checked_add(dropped_count)
            .ok_or(TelemetryError::Overflow)?;

        Ok(IngestBatchOutcome {
            sealed_segments,
            continuity_receipt: Some(receipt),
            admitted_count,
            dropped_count,
        })
    }

    /// Flush any currently buffered samples into a sealed segment.
    pub fn flush(&mut self) -> Result<Option<TelemetrySegment>, TelemetryError> {
        self.builder.seal_open(SealReason::ExplicitFlush)
    }
}

/// Summary of an end-to-end continuity audit across sealed segments and continuity receipts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContinuityAuditReport {
    pub segment_count: usize,
    pub receipt_count: usize,
    pub total_sealed_samples: u64,
    pub total_receipt_dropped: u64,
    pub total_accounted: u64,
}

/// Verify cryptographic integrity of every segment and every continuity receipt, and prove exact
/// sample conservation (`total_sealed_samples + total_receipt_dropped == expected_total_offered`).
pub fn audit_continuity(
    segments: &[TelemetrySegment],
    receipts: &[ContinuityReceipt],
    expected_total_offered: u64,
) -> Result<ContinuityAuditReport, TelemetryError> {
    let mut total_sealed_samples = 0_u64;
    for seg in segments {
        seg.verify_root()?;
        total_sealed_samples = total_sealed_samples
            .checked_add(seg.sample_count)
            .ok_or(TelemetryError::Overflow)?;
    }

    let mut total_receipt_dropped = 0_u64;
    for receipt in receipts {
        receipt.verify()?;
        total_receipt_dropped = total_receipt_dropped
            .checked_add(receipt.dropped_count)
            .ok_or(TelemetryError::Overflow)?;
    }

    let total_accounted = total_sealed_samples
        .checked_add(total_receipt_dropped)
        .ok_or(TelemetryError::Overflow)?;
    if total_accounted != expected_total_offered {
        return Err(TelemetryError::DigestMismatch {
            kind: "continuity_audit",
            detail: format!(
                "unaccounted telemetry samples: sealed({total_sealed_samples}) + \
                 dropped({total_receipt_dropped}) = {total_accounted} != offered({expected_total_offered})"
            ),
        });
    }

    Ok(ContinuityAuditReport {
        segment_count: segments.len(),
        receipt_count: receipts.len(),
        total_sealed_samples,
        total_receipt_dropped,
        total_accounted,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::segment::{DEFAULT_MAX_SEGMENT_BYTES, DEFAULT_SEAL_INTERVAL_NS};
    use fmine_evidence::RefusalCode;

    fn mk_sample(sig: u32, ts_ms: u64, val: i64) -> TelemetrySample {
        TelemetrySample {
            signal_id: sig,
            timestamp_ns: ts_ms * 1_000_000,
            value_scaled: val,
            scale_exp10: -1,
            quality_flags: 0,
        }
    }

    #[test]
    fn test_overload_early_seal_downsampling_ring_retention_and_continuity_receipts() {
        let mut edge = BoundedIngestEdge::new(IngestPolicy {
            site_id: "ngm_cortez".to_string(),
            seal_interval_ns: DEFAULT_SEAL_INTERVAL_NS,
            max_segment_samples: 50,
            max_segment_bytes: DEFAULT_MAX_SEGMENT_BYTES,
            max_batch_admit_samples: 10,
            downsample_stride: 3, // keep 1 of every 3 per signal under overload
            ring_capacity_samples: 25,
        })
        .unwrap();

        let mut all_segments = Vec::new();
        let mut all_receipts = Vec::new();

        // Batch 1 (normal load: 4 samples <= 10) -> buffered, no receipt.
        let normal_batch: Vec<TelemetrySample> = (0..4)
            .map(|i| mk_sample(1, i * 20, (i as i64) * 100))
            .collect();
        let out1 = edge.ingest_batch(&normal_batch).unwrap();
        assert!(out1.continuity_receipt.is_none());
        assert_eq!(out1.admitted_count, 4);
        assert_eq!(out1.dropped_count, 0);
        all_segments.extend(out1.sealed_segments);

        // Batch 2 (overload: 18 samples > 10 across 2 signals, 9 per signal)
        // -> forces EarlyBackpressureSeal of the 4 buffered samples, downsamples 1-of-3 (keeps 3 per signal = 6, drops 12),
        // retains raw samples in ring buffer, and emits a verified ContinuityReceipt!
        let mut burst = Vec::new();
        for i in 0..9_u64 {
            let ts = 100 + i * 10;
            burst.push(mk_sample(1, ts, (i as i64) * 10));
            burst.push(mk_sample(2, ts, (i as i64) * 20));
        }
        let out2 = edge.ingest_batch(&burst).unwrap();
        assert_eq!(out2.admitted_count, 6);
        assert_eq!(out2.dropped_count, 12);
        assert!(
            out2.sealed_segments
                .iter()
                .any(|s| s.seal_reason == SealReason::EarlyBackpressureSeal),
            "overload must seal open segment early"
        );
        let early_seg = &out2.sealed_segments[0];
        assert_eq!(early_seg.sample_count, 4);

        let receipt = out2
            .continuity_receipt
            .expect("overload MUST emit a ContinuityReceipt");
        receipt.verify().unwrap();
        assert_eq!(receipt.early_sealed_segment_root, early_seg.segment_root);
        assert_eq!(receipt.total_offered, 18);
        assert_eq!(receipt.admitted_count, 6);
        assert_eq!(receipt.dropped_count, 12);
        assert_eq!(receipt.drop_reason, DropReason::QueueOverloadDownsampled);
        assert_eq!(receipt.ring_retained_count, 18);
        assert_eq!(receipt.dropped_by_signal.get(&1), Some(&6));
        assert_eq!(receipt.dropped_by_signal.get(&2), Some(&6));
        // Local raw ring retained all 4 + 18 = 22 samples (< ring_capacity 25) with 0 evictions.
        assert_eq!(edge.raw_ring().len(), 22);
        assert_eq!(edge.raw_ring().evicted_count(), 0);

        all_segments.extend(out2.sealed_segments);
        all_receipts.push(receipt.clone());

        // Flush remaining 6 downsampled samples.
        if let Some(tail) = edge.flush().unwrap() {
            all_segments.push(tail);
        }

        // End-to-end conservation audit: 4 + 6 sealed + 12 dropped = 22 offered (zero silent drops!).
        let report = audit_continuity(&all_segments, &all_receipts, edge.total_offered()).unwrap();
        assert_eq!(report.total_sealed_samples, 10);
        assert_eq!(report.total_receipt_dropped, 12);
        assert_eq!(report.total_accounted, 22);

        // Tampering with receipt dropped_count fails verify & audit_continuity with FMINE-REF-505.
        let mut tampered_receipt = receipt;
        tampered_receipt.dropped_count -= 1;
        assert_eq!(
            tampered_receipt.verify().unwrap_err().to_refusal().code,
            RefusalCode::DigestMismatch505.spec().code
        );
        // Silent drop (claiming 23 offered when only 22 accounted) fails audit_continuity.
        assert_eq!(
            audit_continuity(&all_segments, &all_receipts, 23)
                .unwrap_err()
                .to_refusal()
                .code,
            RefusalCode::DigestMismatch505.spec().code
        );
    }

    #[test]
    fn test_hard_limit_rejection_and_policy_refusals() {
        // Invalid stride (< 2) refused with FMINE-REF-109.
        let bad = BoundedIngestEdge::new(IngestPolicy {
            site_id: "s".to_string(),
            seal_interval_ns: DEFAULT_SEAL_INTERVAL_NS,
            max_segment_samples: 10,
            max_segment_bytes: DEFAULT_MAX_SEGMENT_BYTES,
            max_batch_admit_samples: 5,
            downsample_stride: 1,
            ring_capacity_samples: 10,
        });
        assert_eq!(
            bad.unwrap_err().to_refusal().code,
            RefusalCode::OutsidePhysicalDomain109.spec().code
        );

        // Hard limit: batch=12, stride=2 would keep 6 > max_batch_admit=4 -> caps at 4, marks QueueHardLimitRejected.
        let mut edge = BoundedIngestEdge::new(IngestPolicy {
            site_id: "s".to_string(),
            seal_interval_ns: DEFAULT_SEAL_INTERVAL_NS,
            max_segment_samples: 100,
            max_segment_bytes: DEFAULT_MAX_SEGMENT_BYTES,
            max_batch_admit_samples: 4,
            downsample_stride: 2,
            ring_capacity_samples: 5,
        })
        .unwrap();
        let batch: Vec<TelemetrySample> = (0..12).map(|i| mk_sample(7, i * 5, i as i64)).collect();
        let out = edge.ingest_batch(&batch).unwrap();
        let r = out.continuity_receipt.unwrap();
        r.verify().unwrap();
        assert_eq!(r.admitted_count, 4);
        assert_eq!(r.dropped_count, 8);
        assert_eq!(r.drop_reason, DropReason::QueueHardLimitRejected);
        // Ring buffer capped at 5 samples, evicting 7 oldest.
        assert_eq!(edge.raw_ring().len(), 5);
        assert_eq!(edge.raw_ring().evicted_count(), 7);
    }
}
