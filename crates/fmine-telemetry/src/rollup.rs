//! Determinism: BitwisePortable
//! Deterministic 1-minute telemetry rollups and compact 16-byte replication payload (`BD-242`,
//! plan §26.8: "5,000 signals at 16 B per minute is ≈ 0.12 GB/day, about 1% of a 10 Mbit/s link").

use std::collections::BTreeMap;

use fmine_units::ExactQty;

use crate::TelemetryError;
use crate::segment::TelemetrySegment;

/// Default rollup window: 1 minute in nanoseconds (plan §26.8).
pub const DEFAULT_ROLLUP_WINDOW_NS: u64 = 60_000_000_000;

/// Rollup aggregation configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RollupConfig {
    pub window_ns: u64,
}

impl Default for RollupConfig {
    fn default() -> Self {
        Self {
            window_ns: DEFAULT_ROLLUP_WINDOW_NS,
        }
    }
}

/// Compact 16-byte wire representation per signal per rollup window (plan §26.8 link budget:
/// `min_i32 + max_i32 + mean_i32 + count_u32 = 16 B`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CompactRollup16 {
    pub min_scaled_i32: i32,
    pub max_scaled_i32: i32,
    pub mean_scaled_i32: i32,
    pub sample_count_u32: u32,
}

impl CompactRollup16 {
    /// Encode to exact 16 little-endian bytes.
    #[must_use]
    pub fn to_bytes(self) -> [u8; 16] {
        let mut out = [0u8; 16];
        out[0..4].copy_from_slice(&self.min_scaled_i32.to_le_bytes());
        out[4..8].copy_from_slice(&self.max_scaled_i32.to_le_bytes());
        out[8..12].copy_from_slice(&self.mean_scaled_i32.to_le_bytes());
        out[12..16].copy_from_slice(&self.sample_count_u32.to_le_bytes());
        out
    }

    /// Decode from exact 16 little-endian bytes.
    #[must_use]
    pub fn from_bytes(bytes: [u8; 16]) -> Self {
        Self {
            min_scaled_i32: i32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]),
            max_scaled_i32: i32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]),
            mean_scaled_i32: i32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]),
            sample_count_u32: u32::from_le_bytes([bytes[12], bytes[13], bytes[14], bytes[15]]),
        }
    }
}

/// Exact per-signal rollup over one `[bucket_start_ns, bucket_end_ns)` window.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignalRollup {
    pub signal_id: u32,
    pub bucket_start_ns: u64,
    pub bucket_end_ns: u64,
    pub scale_exp10: i8,
    pub sample_count: u64,
    pub min_scaled: i64,
    pub max_scaled: i64,
    pub sum_scaled: i128,
}

fn pow10_i128(exp: u32) -> Result<i128, TelemetryError> {
    let mut acc = 1_i128;
    for _ in 0..exp {
        acc = acc.checked_mul(10).ok_or(TelemetryError::Overflow)?;
    }
    Ok(acc)
}

impl SignalRollup {
    /// Exact rational mean `(sum_scaled / sample_count) * 10^scale_exp10` with zero float drift.
    pub fn exact_mean(&self) -> Result<ExactQty, TelemetryError> {
        if self.sample_count == 0 {
            return Err(TelemetryError::InvalidConfig {
                field: "sample_count",
                detail: "cannot derive mean for empty rollup bucket".to_string(),
            });
        }
        let base = ExactQty::new(self.sum_scaled, i128::from(self.sample_count))?;
        if self.scale_exp10 == 0 {
            Ok(base)
        } else if self.scale_exp10 > 0 {
            let factor = ExactQty::from_integer(pow10_i128(self.scale_exp10 as u32)?);
            Ok(base.checked_mul(factor)?)
        } else {
            let factor = ExactQty::new(1, pow10_i128(self.scale_exp10.unsigned_abs() as u32)?)?;
            Ok(base.checked_mul(factor)?)
        }
    }

    /// Encode this rollup into the compact 16-byte replication wire record (`FMINE-REF-109` on
    /// overflow rather than silent truncation).
    pub fn to_compact_16(&self) -> Result<CompactRollup16, TelemetryError> {
        if self.sample_count == 0 {
            return Err(TelemetryError::InvalidConfig {
                field: "sample_count",
                detail: "cannot pack empty rollup bucket".to_string(),
            });
        }
        let min_i32 = i32::try_from(self.min_scaled).map_err(|_| TelemetryError::Overflow)?;
        let max_i32 = i32::try_from(self.max_scaled).map_err(|_| TelemetryError::Overflow)?;
        let count_u32 = u32::try_from(self.sample_count).map_err(|_| TelemetryError::Overflow)?;
        // Truncate-free nearest integer mean in scaled units (ties away from zero), overflow-checked.
        let n = i128::from(self.sample_count);
        let half = n / 2;
        let mean_i128 = if self.sum_scaled >= 0 {
            self.sum_scaled
                .checked_add(half)
                .ok_or(TelemetryError::Overflow)?
                / n
        } else {
            self.sum_scaled
                .checked_sub(half)
                .ok_or(TelemetryError::Overflow)?
                / n
        };
        let mean_i32 = i32::try_from(mean_i128).map_err(|_| TelemetryError::Overflow)?;
        Ok(CompactRollup16 {
            min_scaled_i32: min_i32,
            max_scaled_i32: max_i32,
            mean_scaled_i32: mean_i32,
            sample_count_u32: count_u32,
        })
    }
}

/// Compute deterministic fixed-window rollups across a sequence of sealed [`TelemetrySegment`]s.
///
/// Verifies every segment root before aggregating (`FMINE-REF-505`), refuses conflicting
/// `scale_exp10` for the same `signal_id` (`FMINE-REF-105`), and conserves sample counts exactly.
pub fn compute_rollups(
    segments: &[TelemetrySegment],
    config: RollupConfig,
) -> Result<Vec<SignalRollup>, TelemetryError> {
    if config.window_ns == 0 {
        return Err(TelemetryError::InvalidConfig {
            field: "window_ns",
            detail: "rollup window_ns must be > 0".to_string(),
        });
    }

    let mut scale_by_signal: BTreeMap<u32, i8> = BTreeMap::new();
    // Keyed canonically by (bucket_start_ns, signal_id) for deterministic output ordering.
    let mut buckets: BTreeMap<(u64, u32), SignalRollup> = BTreeMap::new();

    for seg in segments {
        seg.verify_root()?;
        for (&sig_id, col) in &seg.columns {
            if let Some(&expected_exp10) = scale_by_signal.get(&sig_id)
                && expected_exp10 != col.scale_exp10
            {
                return Err(TelemetryError::ScaleMismatch {
                    signal_id: sig_id,
                    expected_exp10,
                    got_exp10: col.scale_exp10,
                });
            }
            scale_by_signal.insert(sig_id, col.scale_exp10);

            for i in 0..col.len() {
                let ts = col.timestamps_ns[i];
                let val = col.values_scaled[i];
                let bucket_idx = ts / config.window_ns;
                let bucket_start_ns = bucket_idx
                    .checked_mul(config.window_ns)
                    .ok_or(TelemetryError::Overflow)?;
                let bucket_end_ns = bucket_start_ns
                    .checked_add(config.window_ns)
                    .ok_or(TelemetryError::Overflow)?;

                match buckets.get_mut(&(bucket_start_ns, sig_id)) {
                    Some(r) => {
                        r.sample_count = r
                            .sample_count
                            .checked_add(1)
                            .ok_or(TelemetryError::Overflow)?;
                        r.min_scaled = r.min_scaled.min(val);
                        r.max_scaled = r.max_scaled.max(val);
                        r.sum_scaled = r
                            .sum_scaled
                            .checked_add(i128::from(val))
                            .ok_or(TelemetryError::Overflow)?;
                    }
                    None => {
                        buckets.insert(
                            (bucket_start_ns, sig_id),
                            SignalRollup {
                                signal_id: sig_id,
                                bucket_start_ns,
                                bucket_end_ns,
                                scale_exp10: col.scale_exp10,
                                sample_count: 1,
                                min_scaled: val,
                                max_scaled: val,
                                sum_scaled: i128::from(val),
                            },
                        );
                    }
                }
            }
        }
    }

    Ok(buckets.into_values().collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::segment::{
        DEFAULT_MAX_SEGMENT_BYTES, DEFAULT_SEAL_INTERVAL_NS, SealReason, SegmentLogBuilder,
        SegmentPolicy, TelemetrySample,
    };
    use fmine_evidence::RefusalCode;

    #[test]
    fn test_one_minute_rollups_exact_means_compact_16b_and_scale_refusal() {
        let mut builder = SegmentLogBuilder::new(SegmentPolicy {
            site_id: "barrick_pv".to_string(),
            seal_interval_ns: DEFAULT_SEAL_INTERVAL_NS,
            max_segment_samples: 100,
            max_segment_bytes: DEFAULT_MAX_SEGMENT_BYTES,
        })
        .unwrap();

        let mut segments = Vec::new();
        // Minute 0 (0..60s): signal 10 has values 100, 120, 140 (scale 10^-1 -> 10.0, 12.0, 14.0 -> mean 12.0).
        // Minute 1 (60..120s): signal 10 has values 200, 210 (scale 10^-1 -> 20.0, 21.0 -> mean 20.5).
        let samples = [
            (10_u32, 1_000_000_000_u64, 100_i64),
            (10, 20_000_000_000, 120),
            (10, 59_000_000_000, 140),
            (10, 60_000_000_000, 200),
            (10, 90_000_000_000, 210),
        ];
        for (sig, ts, val) in samples {
            segments.extend(
                builder
                    .push(TelemetrySample {
                        signal_id: sig,
                        timestamp_ns: ts,
                        value_scaled: val,
                        scale_exp10: -1,
                        quality_flags: 0,
                    })
                    .unwrap(),
            );
        }
        if let Some(tail) = builder.seal_open(SealReason::ExplicitFlush).unwrap() {
            segments.push(tail);
        }

        let rollups = compute_rollups(&segments, RollupConfig::default()).unwrap();
        assert_eq!(rollups.len(), 2);
        // Sample count conservation: 3 + 2 == 5.
        let total_samples: u64 = rollups.iter().map(|r| r.sample_count).sum();
        assert_eq!(total_samples, 5);

        assert_eq!(rollups[0].bucket_start_ns, 0);
        assert_eq!(rollups[0].bucket_end_ns, DEFAULT_ROLLUP_WINDOW_NS);
        assert_eq!(rollups[0].min_scaled, 100);
        assert_eq!(rollups[0].max_scaled, 140);
        assert_eq!(rollups[0].exact_mean().unwrap(), ExactQty::from_integer(12));

        assert_eq!(rollups[1].bucket_start_ns, DEFAULT_ROLLUP_WINDOW_NS);
        assert_eq!(
            rollups[1].exact_mean().unwrap(),
            ExactQty::new(41, 2).unwrap()
        ); // 20.5

        // Compact 16-byte wire codec roundtrip.
        let compact = rollups[0].to_compact_16().unwrap();
        let bytes = compact.to_bytes();
        assert_eq!(bytes.len(), 16);
        assert_eq!(CompactRollup16::from_bytes(bytes), compact);
        assert_eq!(compact.mean_scaled_i32, 120);

        // Conflicting scale across segments for the same signal refuses with FMINE-REF-105.
        let mut b2 = SegmentLogBuilder::new(SegmentPolicy {
            site_id: "barrick_pv".to_string(),
            seal_interval_ns: DEFAULT_SEAL_INTERVAL_NS,
            max_segment_samples: 10,
            max_segment_bytes: DEFAULT_MAX_SEGMENT_BYTES,
        })
        .unwrap();
        b2.push(TelemetrySample {
            signal_id: 10,
            timestamp_ns: 120_000_000_000,
            value_scaled: 50,
            scale_exp10: -2,
            quality_flags: 0,
        })
        .unwrap();
        let conflict_seg = b2.seal_open(SealReason::ExplicitFlush).unwrap().unwrap();
        segments.push(conflict_seg);
        assert_eq!(
            compute_rollups(&segments, RollupConfig::default())
                .unwrap_err()
                .to_refusal()
                .code,
            RefusalCode::BasisMismatch105.spec().code
        );
    }
}
