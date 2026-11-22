//! Determinism: BitwisePortable
//! Queue-stability and synthetic high-rate ingest endurance auditor (`BD-243`, plan §26.7–§26.8,
//! `PO-BED-7`, `GATE-INGEST-01`: `ρ <= 0.7`, `0` unreceipted loss, `p99 <= 2 s`).

use fmine_units::ExactQty;

use crate::TelemetryError;
use crate::backpressure::{BoundedIngestEdge, ContinuityReceipt, IngestPolicy, audit_continuity};
use crate::segment::{TelemetrySample, TelemetrySegment};

/// `PO-BED-7` queue utilization ceiling (`7/10 = 0.7`).
pub const PO_BED_7_MAX_UTILIZATION_NUM: i128 = 7;
pub const PO_BED_7_MAX_UTILIZATION_DEN: i128 = 10;

/// `GATE-INGEST-01` p99 ingest-to-durable latency ceiling (`2 s` in nanoseconds).
pub const GATE_INGEST_01_MAX_P99_LATENCY_NS: u64 = 2_000_000_000;

/// Queue stability & Little's law (`L = λ W`) audit summary (`PO-BED-7`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueueStabilityAudit {
    pub arrival_rate_msg_per_s: ExactQty,
    pub service_rate_msg_per_s: ExactQty,
    /// Exact utilization `ρ = λ / μ`.
    pub utilization_rho: ExactQty,
    /// Mean residence time `W` in seconds.
    pub mean_wait_s: ExactQty,
    /// Exact Little's law mean queue depth `L = λ · W`.
    pub littles_law_mean_queue: ExactQty,
    pub p99_ingest_to_seal_ns: u64,
    pub total_offered: u64,
    pub total_sealed: u64,
    pub total_receipt_dropped: u64,
    pub unreceipted_loss: u64,
    /// `true` iff `ρ <= 0.7`, `unreceipted_loss == 0`, and `p99_ingest_to_seal_ns <= 2 s`.
    pub meets_po_bed_7: bool,
}

/// Evaluate `PO-BED-7` queueing stability (`ρ <= 0.7`, Little's law `L = λ W`, zero unreceipted
/// loss, `p99 <= 2 s`) over exact observed rates and sealed telemetry artifacts.
pub fn audit_queue_stability(
    arrival_rate_msg_per_s: ExactQty,
    service_rate_msg_per_s: ExactQty,
    mean_wait_s: ExactQty,
    p99_ingest_to_seal_ns: u64,
    segments: &[TelemetrySegment],
    receipts: &[ContinuityReceipt],
    expected_total_offered: u64,
) -> Result<QueueStabilityAudit, TelemetryError> {
    if arrival_rate_msg_per_s.signum() < 0
        || service_rate_msg_per_s.signum() <= 0
        || mean_wait_s.signum() < 0
    {
        return Err(TelemetryError::InvalidConfig {
            field: "queue_rates",
            detail: "arrival_rate >= 0, service_rate > 0, mean_wait_s >= 0 required".to_string(),
        });
    }

    let continuity = audit_continuity(segments, receipts, expected_total_offered)?;
    let utilization_rho = arrival_rate_msg_per_s.checked_div(service_rate_msg_per_s)?;
    let littles_law_mean_queue = arrival_rate_msg_per_s.checked_mul(mean_wait_s)?;
    let rho_cap = ExactQty::new(PO_BED_7_MAX_UTILIZATION_NUM, PO_BED_7_MAX_UTILIZATION_DEN)?;

    let unreceipted_loss = expected_total_offered.saturating_sub(continuity.total_accounted);
    let meets_po_bed_7 = utilization_rho.checked_cmp(rho_cap)?.is_le()
        && unreceipted_loss == 0
        && p99_ingest_to_seal_ns <= GATE_INGEST_01_MAX_P99_LATENCY_NS;

    Ok(QueueStabilityAudit {
        arrival_rate_msg_per_s,
        service_rate_msg_per_s,
        utilization_rho,
        mean_wait_s,
        littles_law_mean_queue,
        p99_ingest_to_seal_ns,
        total_offered: expected_total_offered,
        total_sealed: continuity.total_sealed_samples,
        total_receipt_dropped: continuity.total_receipt_dropped,
        unreceipted_loss,
        meets_po_bed_7,
    })
}

/// Deterministic synthetic high-rate ingest simulation driving `BoundedIngestEdge` across
/// `n_batches` batches of `batch_size` samples spanning `signal_count` distinct signals.
pub fn run_synthetic_ingest_endurance(
    policy: IngestPolicy,
    n_batches: u64,
    batch_size: usize,
    signal_count: u32,
    sample_spacing_ns: u64,
    service_rate_msg_per_s: ExactQty,
) -> Result<QueueStabilityAudit, TelemetryError> {
    if n_batches == 0 || batch_size == 0 || signal_count == 0 || sample_spacing_ns == 0 {
        return Err(TelemetryError::InvalidConfig {
            field: "endurance_params",
            detail: "n_batches, batch_size, signal_count, sample_spacing_ns must be > 0"
                .to_string(),
        });
    }
    let mut edge = BoundedIngestEdge::new(policy)?;
    let mut all_segments = Vec::new();
    let mut all_receipts = Vec::new();
    let mut seal_latencies_ns = Vec::new();
    let mut global_idx = 0_u64;

    for _ in 0..n_batches {
        let mut batch = Vec::with_capacity(batch_size);
        for _ in 0..batch_size {
            let sig = (global_idx % u64::from(signal_count)) as u32 + 1;
            let ts = global_idx
                .checked_mul(sample_spacing_ns)
                .ok_or(TelemetryError::Overflow)?;
            batch.push(TelemetrySample {
                signal_id: sig,
                timestamp_ns: ts,
                value_scaled: (global_idx % 10_000) as i64,
                scale_exp10: -2,
                quality_flags: 0,
            });
            global_idx += 1;
        }
        let outcome = edge.ingest_batch(&batch)?;
        for seg in outcome.sealed_segments {
            seal_latencies_ns.push(seg.last_ts_ns.saturating_sub(seg.first_ts_ns));
            all_segments.push(seg);
        }
        if let Some(r) = outcome.continuity_receipt {
            all_receipts.push(r);
        }
    }
    if let Some(tail) = edge.flush()? {
        seal_latencies_ns.push(tail.last_ts_ns.saturating_sub(tail.first_ts_ns));
        all_segments.push(tail);
    }

    seal_latencies_ns.sort_unstable();
    let p99_ingest_to_seal_ns = if seal_latencies_ns.is_empty() {
        0
    } else {
        let idx = ((seal_latencies_ns.len() - 1) * 99) / 100;
        seal_latencies_ns[idx]
    };

    // Arrival rate = 1e9 / sample_spacing_ns messages per second.
    let arrival_rate = ExactQty::new(1_000_000_000, i128::from(sample_spacing_ns))?;
    let mean_wait_s = ExactQty::new(i128::from(p99_ingest_to_seal_ns), 1_000_000_000)?;

    audit_queue_stability(
        arrival_rate,
        service_rate_msg_per_s,
        mean_wait_s,
        p99_ingest_to_seal_ns,
        &all_segments,
        &all_receipts,
        edge.total_offered(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::segment::{DEFAULT_MAX_SEGMENT_BYTES, DEFAULT_SEAL_INTERVAL_NS};

    #[test]
    fn test_po_bed_7_synthetic_100k_msg_per_s_queue_stability_and_zero_unreceipted_loss() {
        // 10^5 msg/s => 10_000 ns spacing between consecutive messages across 50 signals;
        // planned service rate = 150,000 msg/s => rho = 2/3 <= 0.7!
        let policy = IngestPolicy {
            site_id: "barrick_kibali".to_string(),
            seal_interval_ns: DEFAULT_SEAL_INTERVAL_NS, // 250 ms
            max_segment_samples: 25_000,
            max_segment_bytes: DEFAULT_MAX_SEGMENT_BYTES,
            max_batch_admit_samples: 2_000,
            downsample_stride: 2,
            ring_capacity_samples: 5_000,
        };
        let audit = run_synthetic_ingest_endurance(
            policy,
            30,    // 30 batches
            1_000, // 30,000 messages total spanning 0.30 s at 100,000 msg/s
            50,
            10_000, // 10 us spacing = 100,000 msg/s
            ExactQty::from_integer(150_000),
        )
        .unwrap();

        assert_eq!(audit.total_offered, 30_000);
        assert_eq!(audit.total_sealed, 30_000);
        assert_eq!(audit.total_receipt_dropped, 0);
        assert_eq!(audit.unreceipted_loss, 0);
        assert_eq!(audit.utilization_rho, ExactQty::new(2, 3).unwrap());
        assert!(audit.p99_ingest_to_seal_ns <= GATE_INGEST_01_MAX_P99_LATENCY_NS);
        assert!(audit.meets_po_bed_7);

        // Overloaded arrival rate (rho = 100k / 120k = 5/6 > 0.7) flags meets_po_bed_7 = false!
        let overloaded = run_synthetic_ingest_endurance(
            IngestPolicy {
                site_id: "barrick_kibali".to_string(),
                seal_interval_ns: DEFAULT_SEAL_INTERVAL_NS,
                max_segment_samples: 25_000,
                max_segment_bytes: DEFAULT_MAX_SEGMENT_BYTES,
                max_batch_admit_samples: 500, // forces backpressure downsampling + ContinuityReceipts!
                downsample_stride: 2,
                ring_capacity_samples: 2_000,
            },
            10,
            1_000,
            20,
            10_000,
            ExactQty::from_integer(120_000),
        )
        .unwrap();
        assert_eq!(overloaded.unreceipted_loss, 0);
        assert_eq!(
            overloaded.total_sealed + overloaded.total_receipt_dropped,
            10_000
        );
        assert!(overloaded.total_receipt_dropped > 0);
        assert!(!overloaded.meets_po_bed_7);
    }
}
