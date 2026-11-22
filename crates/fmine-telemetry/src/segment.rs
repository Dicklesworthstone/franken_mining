//! Determinism: BitwisePortable
//! Columnar telemetry segment batching, 250 ms timer / capacity sealing, SHA-256 segment roots,
//! and `(segment_root, offset_range)` trace references (`BD-242`, plan §26.8).

use std::collections::BTreeMap;

use fmine_core::Digest32;
use fmine_core::hash::sha256;

use crate::TelemetryError;

/// Default telemetry segment seal timer: 250 ms in nanoseconds (plan §26.8).
pub const DEFAULT_SEAL_INTERVAL_NS: u64 = 250_000_000;

/// Default maximum segment payload size: 64 MiB (plan §26.7 budget table).
pub const DEFAULT_MAX_SEGMENT_BYTES: usize = 64 * 1024 * 1024;

/// Maximum permitted absolute base-10 exponent for fixed-point telemetry values.
pub const MAX_ABS_SCALE_EXP10: i8 = 12;

/// Canonical domain prefix for telemetry segment root hashing.
const SEGMENT_DOMAIN_PREFIX: &[u8] = b"fmine:telemetry-segment:v1\0";

/// Estimated encoded bytes per sample inside a columnar segment (8B ts + 8B val + 2B quality).
pub const ESTIMATED_BYTES_PER_SAMPLE: usize = 18;

/// Why a telemetry segment was sealed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum SealReason {
    /// Timer elapsed (`timestamp_ns - first_ts_ns >= seal_interval_ns`, default 250 ms).
    Timer250Ms = 1,
    /// Segment sample count or byte budget reached.
    CapacityReached = 2,
    /// Early seal triggered by ingest overload backpressure (plan §26.8).
    EarlyBackpressureSeal = 3,
    /// Explicit caller flush (e.g. clean shutdown or end of shift window).
    ExplicitFlush = 4,
}

impl SealReason {
    #[must_use]
    pub const fn tag_byte(self) -> u8 {
        self as u8
    }
}

/// One raw telemetry measurement in bit-portable fixed-point representation:
/// physical value = `value_scaled * 10^scale_exp10`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TelemetrySample {
    pub signal_id: u32,
    pub timestamp_ns: u64,
    pub value_scaled: i64,
    pub scale_exp10: i8,
    pub quality_flags: u16,
}

impl TelemetrySample {
    /// Validate fixed-point scale bounds.
    pub fn validate(&self) -> Result<(), TelemetryError> {
        if self.scale_exp10.abs() > MAX_ABS_SCALE_EXP10 {
            return Err(TelemetryError::InvalidConfig {
                field: "scale_exp10",
                detail: format!(
                    "scale_exp10 {} outside [-{}, {}]",
                    self.scale_exp10, MAX_ABS_SCALE_EXP10, MAX_ABS_SCALE_EXP10
                ),
            });
        }
        Ok(())
    }
}

/// Columnar storage for a single `signal_id` within a sealed segment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignalColumn {
    pub signal_id: u32,
    pub scale_exp10: i8,
    pub timestamps_ns: Vec<u64>,
    pub values_scaled: Vec<i64>,
    pub quality_flags: Vec<u16>,
}

impl SignalColumn {
    /// Number of samples in this column.
    #[must_use]
    pub fn len(&self) -> usize {
        self.timestamps_ns.len()
    }

    /// Returns `true` if the column has zero samples.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.timestamps_ns.is_empty()
    }

    /// Reconstruct sample at index `idx`.
    #[must_use]
    pub fn sample_at(&self, idx: usize) -> Option<TelemetrySample> {
        Some(TelemetrySample {
            signal_id: self.signal_id,
            timestamp_ns: *self.timestamps_ns.get(idx)?,
            value_scaled: *self.values_scaled.get(idx)?,
            scale_exp10: self.scale_exp10,
            quality_flags: *self.quality_flags.get(idx)?,
        })
    }
}

/// Immutable sealed columnar telemetry segment with SHA-256 content root.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TelemetrySegment {
    pub site_id: String,
    pub segment_seq: u64,
    pub seal_reason: SealReason,
    pub first_ts_ns: u64,
    pub last_ts_ns: u64,
    pub sample_count: u64,
    pub estimated_bytes: usize,
    pub columns: BTreeMap<u32, SignalColumn>,
    pub segment_root: Digest32,
}

impl TelemetrySegment {
    /// Compute canonical bytes and SHA-256 root digest over header metadata and columnar payload.
    pub fn compute_root(&self) -> Result<Digest32, TelemetryError> {
        let mut buf = Vec::new();
        buf.extend_from_slice(SEGMENT_DOMAIN_PREFIX);
        let site_len = u32::try_from(self.site_id.len()).map_err(|_| TelemetryError::Overflow)?;
        buf.extend_from_slice(&site_len.to_le_bytes());
        buf.extend_from_slice(self.site_id.as_bytes());
        buf.extend_from_slice(&self.segment_seq.to_le_bytes());
        buf.push(self.seal_reason.tag_byte());
        buf.extend_from_slice(&self.first_ts_ns.to_le_bytes());
        buf.extend_from_slice(&self.last_ts_ns.to_le_bytes());
        buf.extend_from_slice(&self.sample_count.to_le_bytes());

        let col_count = u32::try_from(self.columns.len()).map_err(|_| TelemetryError::Overflow)?;
        buf.extend_from_slice(&col_count.to_le_bytes());
        for (&sig_id, col) in &self.columns {
            if col.signal_id != sig_id
                || col.timestamps_ns.len() != col.values_scaled.len()
                || col.timestamps_ns.len() != col.quality_flags.len()
            {
                return Err(TelemetryError::DigestMismatch {
                    kind: "telemetry_segment",
                    detail: format!(
                        "column {sig_id} internal vector lengths or signal_id mismatch"
                    ),
                });
            }
            buf.extend_from_slice(&sig_id.to_le_bytes());
            buf.push(col.scale_exp10 as u8);
            let n = u32::try_from(col.timestamps_ns.len()).map_err(|_| TelemetryError::Overflow)?;
            buf.extend_from_slice(&n.to_le_bytes());
            // Delta-encode monotonic timestamps and values into the canonical digest stream.
            let mut prev_ts = 0_u64;
            let mut prev_val = 0_i64;
            for i in 0..col.timestamps_ns.len() {
                let ts = col.timestamps_ns[i];
                let d_ts = if i == 0 {
                    ts
                } else {
                    ts.checked_sub(prev_ts)
                        .ok_or(TelemetryError::OutOfOrderTimestamp {
                            signal_id: sig_id,
                            prev_ts_ns: prev_ts,
                            got_ts_ns: ts,
                        })?
                };
                prev_ts = ts;
                let val = col.values_scaled[i];
                let d_val = if i == 0 {
                    i128::from(val)
                } else {
                    i128::from(val) - i128::from(prev_val)
                };
                prev_val = val;
                buf.extend_from_slice(&d_ts.to_le_bytes());
                buf.extend_from_slice(&d_val.to_le_bytes());
                buf.extend_from_slice(&col.quality_flags[i].to_le_bytes());
            }
        }
        Ok(sha256(&buf))
    }

    /// Verify structural invariants (`sample_count`, `first_ts_ns <= last_ts_ns`, scale bounds)
    /// and that `self.segment_root` matches the canonical content digest (`FMINE-REF-505` on mismatch).
    pub fn verify_root(&self) -> Result<(), TelemetryError> {
        let mut actual_count = 0_u64;
        for col in self.columns.values() {
            if col.scale_exp10.abs() > MAX_ABS_SCALE_EXP10 {
                return Err(TelemetryError::InvalidConfig {
                    field: "scale_exp10",
                    detail: format!(
                        "column {} scale_exp10 {} outside [-{}, {}]",
                        col.signal_id, col.scale_exp10, MAX_ABS_SCALE_EXP10, MAX_ABS_SCALE_EXP10
                    ),
                });
            }
            let n = u64::try_from(col.len()).map_err(|_| TelemetryError::Overflow)?;
            actual_count = actual_count
                .checked_add(n)
                .ok_or(TelemetryError::Overflow)?;
        }
        if actual_count != self.sample_count {
            return Err(TelemetryError::DigestMismatch {
                kind: "telemetry_segment",
                detail: format!(
                    "segment {} declared sample_count {} != actual column samples {}",
                    self.segment_seq, self.sample_count, actual_count
                ),
            });
        }
        if self.sample_count > 0 && self.first_ts_ns > self.last_ts_ns {
            return Err(TelemetryError::DigestMismatch {
                kind: "telemetry_segment",
                detail: format!(
                    "segment {} inverted bounds {} > {}",
                    self.segment_seq, self.first_ts_ns, self.last_ts_ns
                ),
            });
        }
        let computed = self.compute_root()?;
        if computed == self.segment_root {
            Ok(())
        } else {
            Err(TelemetryError::DigestMismatch {
                kind: "telemetry_segment",
                detail: format!(
                    "segment {} root expected {}, computed {}",
                    self.segment_seq,
                    self.segment_root.to_hex(),
                    computed.to_hex()
                ),
            })
        }
    }
}

/// Reference stored inside a ledger transition fact pointing back to its raw high-rate trace
/// (`(segment_root, signal_id, start_offset..end_offset)`, plan §26.8).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SegmentTraceRef {
    pub segment_root: Digest32,
    pub signal_id: u32,
    pub start_offset: u32,
    pub end_offset: u32,
}

/// Resolve a [`SegmentTraceRef`] against a sealed [`TelemetrySegment`], verifying segment integrity
/// and offset bounds before returning the exact raw trace slice.
pub fn slice_trace(
    segment: &TelemetrySegment,
    trace_ref: &SegmentTraceRef,
) -> Result<Vec<TelemetrySample>, TelemetryError> {
    segment.verify_root()?;
    if segment.segment_root != trace_ref.segment_root {
        return Err(TelemetryError::InvalidTraceRef {
            detail: format!(
                "segment_root mismatch: ref {}, segment {}",
                trace_ref.segment_root.to_hex(),
                segment.segment_root.to_hex()
            ),
        });
    }
    if trace_ref.start_offset >= trace_ref.end_offset {
        return Err(TelemetryError::InvalidTraceRef {
            detail: format!(
                "empty or inverted trace range {}..{}",
                trace_ref.start_offset, trace_ref.end_offset
            ),
        });
    }
    let col = segment.columns.get(&trace_ref.signal_id).ok_or_else(|| {
        TelemetryError::InvalidTraceRef {
            detail: format!("signal {} absent from segment", trace_ref.signal_id),
        }
    })?;
    let start = trace_ref.start_offset as usize;
    let end = trace_ref.end_offset as usize;
    if end > col.len() {
        return Err(TelemetryError::InvalidTraceRef {
            detail: format!(
                "trace range {}..{} out of bounds for signal {} len {}",
                start,
                end,
                trace_ref.signal_id,
                col.len()
            ),
        });
    }
    let mut out = Vec::with_capacity(end - start);
    for idx in start..end {
        if let Some(s) = col.sample_at(idx) {
            out.push(s);
        }
    }
    Ok(out)
}

/// Configuration for sealing columnar telemetry segments.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SegmentPolicy {
    pub site_id: String,
    pub seal_interval_ns: u64,
    pub max_segment_samples: usize,
    pub max_segment_bytes: usize,
}

impl SegmentPolicy {
    /// Validate policy bounds (zero interval/capacity or empty site_id refused).
    pub fn validate(&self) -> Result<(), TelemetryError> {
        if self.site_id.trim().is_empty() {
            return Err(TelemetryError::InvalidConfig {
                field: "site_id",
                detail: "site_id must be non-empty".to_string(),
            });
        }
        if self.seal_interval_ns == 0 {
            return Err(TelemetryError::InvalidConfig {
                field: "seal_interval_ns",
                detail: "seal_interval_ns must be > 0".to_string(),
            });
        }
        if self.max_segment_samples == 0 {
            return Err(TelemetryError::InvalidConfig {
                field: "max_segment_samples",
                detail: "max_segment_samples must be > 0".to_string(),
            });
        }
        if self.max_segment_bytes < ESTIMATED_BYTES_PER_SAMPLE {
            return Err(TelemetryError::InvalidConfig {
                field: "max_segment_bytes",
                detail: format!(
                    "max_segment_bytes must be >= {ESTIMATED_BYTES_PER_SAMPLE}, got {}",
                    self.max_segment_bytes
                ),
            });
        }
        Ok(())
    }
}

/// Deterministic columnar telemetry segment builder implementing 250 ms timer and capacity sealing.
#[derive(Debug, Clone)]
pub struct SegmentLogBuilder {
    policy: SegmentPolicy,
    next_seq: u64,
    columns: BTreeMap<u32, SignalColumn>,
    /// Tracks last timestamp per signal across segment boundaries so monotonicity holds globally.
    last_ts_by_signal: BTreeMap<u32, u64>,
    /// Tracks declared fixed-point scale per signal across segment boundaries.
    scale_by_signal: BTreeMap<u32, i8>,
    first_ts_ns: Option<u64>,
    last_ts_ns: u64,
    open_samples: usize,
    open_bytes: usize,
}

impl SegmentLogBuilder {
    /// Construct a validated builder starting at `segment_seq = 0`.
    pub fn new(policy: SegmentPolicy) -> Result<Self, TelemetryError> {
        policy.validate()?;
        Ok(Self {
            policy,
            next_seq: 0,
            columns: BTreeMap::new(),
            last_ts_by_signal: BTreeMap::new(),
            scale_by_signal: BTreeMap::new(),
            first_ts_ns: None,
            last_ts_ns: 0,
            open_samples: 0,
            open_bytes: 0,
        })
    }

    /// Number of unsealed samples currently buffered.
    #[must_use]
    pub const fn open_sample_count(&self) -> usize {
        self.open_samples
    }

    /// Estimated unsealed payload size in bytes.
    #[must_use]
    pub const fn open_estimated_bytes(&self) -> usize {
        self.open_bytes
    }

    /// Pre-validate an entire incoming batch for fixed-point scale consistency and per-signal
    /// timestamp monotonicity without mutating builder state.
    pub fn validate_batch(&self, batch: &[TelemetrySample]) -> Result<(), TelemetryError> {
        let mut pending_scales: BTreeMap<u32, i8> = BTreeMap::new();
        let mut pending_ts: BTreeMap<u32, u64> = BTreeMap::new();
        for sample in batch {
            sample.validate()?;
            let expected_scale = pending_scales
                .get(&sample.signal_id)
                .copied()
                .or_else(|| self.scale_by_signal.get(&sample.signal_id).copied());
            if let Some(exp10) = expected_scale
                && exp10 != sample.scale_exp10
            {
                return Err(TelemetryError::ScaleMismatch {
                    signal_id: sample.signal_id,
                    expected_exp10: exp10,
                    got_exp10: sample.scale_exp10,
                });
            }
            pending_scales.insert(sample.signal_id, sample.scale_exp10);

            let prev_ts = pending_ts
                .get(&sample.signal_id)
                .copied()
                .or_else(|| self.last_ts_by_signal.get(&sample.signal_id).copied());
            if let Some(prev) = prev_ts
                && sample.timestamp_ns < prev
            {
                return Err(TelemetryError::OutOfOrderTimestamp {
                    signal_id: sample.signal_id,
                    prev_ts_ns: prev,
                    got_ts_ns: sample.timestamp_ns,
                });
            }
            pending_ts.insert(sample.signal_id, sample.timestamp_ns);
        }
        Ok(())
    }

    /// Push one telemetry sample. Automatically seals the prior open segment on timer rollover
    /// (default 250 ms) OR seals the current segment immediately if capacity is reached.
    ///
    /// Returns up to two sealed segments (e.g. prior timer-expired segment plus immediate
    /// capacity seal when `max_segment_samples == 1`).
    pub fn push(
        &mut self,
        sample: TelemetrySample,
    ) -> Result<Vec<TelemetrySegment>, TelemetryError> {
        sample.validate()?;
        if let Some(&expected_exp10) = self.scale_by_signal.get(&sample.signal_id)
            && expected_exp10 != sample.scale_exp10
        {
            return Err(TelemetryError::ScaleMismatch {
                signal_id: sample.signal_id,
                expected_exp10,
                got_exp10: sample.scale_exp10,
            });
        }
        if let Some(&prev_ts) = self.last_ts_by_signal.get(&sample.signal_id)
            && sample.timestamp_ns < prev_ts
        {
            return Err(TelemetryError::OutOfOrderTimestamp {
                signal_id: sample.signal_id,
                prev_ts_ns: prev_ts,
                got_ts_ns: sample.timestamp_ns,
            });
        }

        let mut sealed = Vec::new();
        if let Some(first_ts) = self.first_ts_ns
            && sample.timestamp_ns.saturating_sub(first_ts) >= self.policy.seal_interval_ns
            && let Some(seg) = self.seal_open(SealReason::Timer250Ms)?
        {
            sealed.push(seg);
        }

        self.scale_by_signal
            .insert(sample.signal_id, sample.scale_exp10);
        self.last_ts_by_signal
            .insert(sample.signal_id, sample.timestamp_ns);

        let col = self
            .columns
            .entry(sample.signal_id)
            .or_insert_with(|| SignalColumn {
                signal_id: sample.signal_id,
                scale_exp10: sample.scale_exp10,
                timestamps_ns: Vec::new(),
                values_scaled: Vec::new(),
                quality_flags: Vec::new(),
            });
        col.timestamps_ns.push(sample.timestamp_ns);
        col.values_scaled.push(sample.value_scaled);
        col.quality_flags.push(sample.quality_flags);

        self.first_ts_ns = Some(match self.first_ts_ns {
            Some(cur) => cur.min(sample.timestamp_ns),
            None => sample.timestamp_ns,
        });
        self.last_ts_ns = self.last_ts_ns.max(sample.timestamp_ns);
        self.open_samples = self
            .open_samples
            .checked_add(1)
            .ok_or(TelemetryError::Overflow)?;
        self.open_bytes = self
            .open_bytes
            .checked_add(ESTIMATED_BYTES_PER_SAMPLE)
            .ok_or(TelemetryError::Overflow)?;

        if (self.open_samples >= self.policy.max_segment_samples
            || self.open_bytes >= self.policy.max_segment_bytes)
            && let Some(seg) = self.seal_open(SealReason::CapacityReached)?
        {
            sealed.push(seg);
        }

        Ok(sealed)
    }

    /// Seal the currently buffered segment with the given [`SealReason`]. Returns `Ok(None)` if
    /// no samples are buffered.
    pub fn seal_open(
        &mut self,
        reason: SealReason,
    ) -> Result<Option<TelemetrySegment>, TelemetryError> {
        if self.open_samples == 0 {
            return Ok(None);
        }
        let seq = self.next_seq;
        self.next_seq = self
            .next_seq
            .checked_add(1)
            .ok_or(TelemetryError::Overflow)?;

        let columns = core::mem::take(&mut self.columns);
        let first_ts_ns = self.first_ts_ns.take().unwrap_or(0);
        let last_ts_ns = self.last_ts_ns;
        let sample_count =
            u64::try_from(self.open_samples).map_err(|_| TelemetryError::Overflow)?;
        let estimated_bytes = self.open_bytes;
        self.open_samples = 0;
        self.open_bytes = 0;
        self.last_ts_ns = 0;

        let mut seg = TelemetrySegment {
            site_id: self.policy.site_id.clone(),
            segment_seq: seq,
            seal_reason: reason,
            first_ts_ns,
            last_ts_ns,
            sample_count,
            estimated_bytes,
            columns,
            segment_root: Digest32([0u8; 32]),
        };
        seg.segment_root = seg.compute_root()?;
        Ok(Some(seg))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fmine_evidence::RefusalCode;

    #[test]
    fn test_segment_sealing_root_verification_and_trace_refs() {
        let mut builder = SegmentLogBuilder::new(SegmentPolicy {
            site_id: "barrick_kibali".to_string(),
            seal_interval_ns: DEFAULT_SEAL_INTERVAL_NS, // 250 ms
            max_segment_samples: 100,
            max_segment_bytes: DEFAULT_MAX_SEGMENT_BYTES,
        })
        .unwrap();

        // 10 Hz payload trace (100 ms spacing): t = 0, 100ms, 200ms in seg 0; t = 300ms triggers 250ms seal!
        let mut sealed = Vec::new();
        for (i, payload_kg) in [0_i64, 85_000, 178_500, 215_200].into_iter().enumerate() {
            let s = TelemetrySample {
                signal_id: 42,
                timestamp_ns: (i as u64) * 100_000_000,
                value_scaled: payload_kg,
                scale_exp10: 0,
                quality_flags: 0,
            };
            sealed.extend(builder.push(s).unwrap());
        }
        assert_eq!(sealed.len(), 1);
        assert_eq!(sealed[0].seal_reason, SealReason::Timer250Ms);
        assert_eq!(sealed[0].sample_count, 3);
        sealed[0].verify_root().unwrap();

        // Link a ledger load transition fact to samples 1..3 of segment 0.
        let tref = SegmentTraceRef {
            segment_root: sealed[0].segment_root,
            signal_id: 42,
            start_offset: 1,
            end_offset: 3,
        };
        let trace = slice_trace(&sealed[0], &tref).unwrap();
        assert_eq!(trace.len(), 2);
        assert_eq!(trace[0].value_scaled, 85_000);
        assert_eq!(trace[1].value_scaled, 178_500);

        // Tampering with a single sample fails verify_root and slice_trace with FMINE-REF-505.
        let mut tampered = sealed[0].clone();
        tampered.columns.get_mut(&42).unwrap().values_scaled[1] += 1;
        assert_eq!(
            tampered.verify_root().unwrap_err().to_refusal().code,
            RefusalCode::DigestMismatch505.spec().code
        );
        assert_eq!(
            slice_trace(&tampered, &tref).unwrap_err().to_refusal().code,
            RefusalCode::DigestMismatch505.spec().code
        );

        // Flush remaining sample.
        let tail = builder
            .seal_open(SealReason::ExplicitFlush)
            .unwrap()
            .unwrap();
        assert_eq!(tail.segment_seq, 1);
        assert_eq!(tail.sample_count, 1);
        tail.verify_root().unwrap();

        // Out-of-order timestamp refused with FMINE-REF-107; scale change refused with FMINE-REF-105.
        let ooo = TelemetrySample {
            signal_id: 42,
            timestamp_ns: 250_000_000, // < 300ms
            value_scaled: 1,
            scale_exp10: 0,
            quality_flags: 0,
        };
        assert_eq!(
            builder.push(ooo).unwrap_err().to_refusal().code,
            RefusalCode::UndeclaredUnitBasisOrTz107.spec().code
        );
        let bad_scale = TelemetrySample {
            signal_id: 42,
            timestamp_ns: 400_000_000,
            value_scaled: 1,
            scale_exp10: -2,
            quality_flags: 0,
        };
        assert_eq!(
            builder.push(bad_scale).unwrap_err().to_refusal().code,
            RefusalCode::BasisMismatch105.spec().code
        );
    }
}
