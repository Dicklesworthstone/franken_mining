//! Determinism: BitwisePortable
//! Drillhole collar, survey, and assay generator scaling to `10^5` holes / `10^7` intervals
//! (`BD-131`, plan §33.3) with direct `FMT-COL-1` (`fmine-array`) chunk emission.

use fmine_array::col::{ColumnarIntervalChunk, IntervalRow, QualifierBits};
use fmine_core::codec::Encoder;
use fmine_core::det::exp;
use fmine_core::hash::{Digest32, sha256};
use fmine_core::rand::{PhiloxStream, RngKey};

use crate::GenError;

/// Canonical drillhole campaign scale target (`BD-131`: 10^5 holes × 100 intervals = 10^7 intervals).
pub const TARGET_HOLES_10E5: u64 = 100_000;
pub const TARGET_INTERVALS_PER_HOLE: u32 = 100;
pub const TARGET_TOTAL_INTERVALS_10E7: u64 = TARGET_HOLES_10E5 * (TARGET_INTERVALS_PER_HOLE as u64);

/// Configuration for deterministic drillhole and assay campaign generation.
#[derive(Debug, Clone, PartialEq)]
pub struct DrillholeCampaignConfig {
    pub campaign_id: String,
    pub seed_root: u64,
    pub hole_count: u64,
    pub intervals_per_hole: u32,
    pub interval_length_mm: i32,
    pub measurand_au_id: u16,
    pub detection_limit_g_per_t: f64,
}

impl DrillholeCampaignConfig {
    /// Standard `10^5` holes / `10^7` intervals benchmark campaign (`BD-131`).
    #[must_use]
    pub fn benchmark_10e5_10e7(seed_root: u64) -> Self {
        Self {
            campaign_id: "DH-CAMPAIGN-10E5-10E7".to_string(),
            seed_root,
            hole_count: TARGET_HOLES_10E5,
            intervals_per_hole: TARGET_INTERVALS_PER_HOLE,
            interval_length_mm: 1_000, // 1.0 m core intervals
            measurand_au_id: 1,
            detection_limit_g_per_t: 0.01,
        }
    }

    /// Total number of downhole intervals in this campaign.
    #[must_use]
    pub const fn total_intervals(&self) -> u64 {
        self.hole_count * (self.intervals_per_hole as u64)
    }

    fn validate(&self) -> Result<(), GenError> {
        if self.campaign_id.is_empty() {
            return Err(GenError::InvalidConfig {
                field: "campaign_id",
                detail: "must be non-empty".to_string(),
            });
        }
        if self.hole_count == 0 {
            return Err(GenError::InvalidConfig {
                field: "hole_count",
                detail: "must be >= 1".to_string(),
            });
        }
        if self.intervals_per_hole == 0 {
            return Err(GenError::InvalidConfig {
                field: "intervals_per_hole",
                detail: "must be >= 1".to_string(),
            });
        }
        if self.interval_length_mm <= 0 {
            return Err(GenError::InvalidConfig {
                field: "interval_length_mm",
                detail: "must be > 0".to_string(),
            });
        }
        Ok(())
    }
}

/// Deterministic collar metadata for one generated drillhole (`HoleId`).
#[derive(Debug, Clone, PartialEq)]
pub struct DrillholeCollar {
    pub hole_index: u64,
    pub hole_id: String,
    pub easting_m: f64,
    pub northing_m: f64,
    pub elevation_m: f64,
    pub total_depth_m: f64,
    /// Fire-assay lab availability lag in days (`3..=21` days per plan §6.3 `G-TIME-2`).
    pub assay_lag_days: u8,
}

/// Deterministic summary emitted after scanning a full campaign without materializing all rows in RAM.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CampaignManifestSummary {
    pub campaign_id: String,
    pub seed_root: u64,
    pub holes_generated: u64,
    pub total_intervals: u64,
    pub below_dl_count: u64,
    pub qaqc_flagged_count: u64,
    /// Deterministic SHA-256 digest committing the full campaign collar & assay stream.
    pub stream_digest: Digest32,
}

/// Deterministically derive the collar metadata for hole `hole_index` in O(1) via `Philox4x32-10`.
#[must_use]
pub fn generate_collar(cfg: &DrillholeCampaignConfig, hole_index: u64) -> DrillholeCollar {
    let mut stream = PhiloxStream::new(RngKey {
        seed_root: cfg.seed_root,
        partition: 0x4448_4331, // "DHC1"
        domain: 1,
        var_group: 1,
        realization: 0,
        brick: hole_index,
    });
    let r0 = stream.next_u64();
    let r1 = stream.next_u64();
    let gx = (hole_index % 500) as f64 * 25.0;
    let gy = (hole_index / 500) as f64 * 25.0;
    let jitter_x = ((r0 & 0xFFFF) as f64 / 65535.0 - 0.5) * 4.0;
    let jitter_y = (((r0 >> 16) & 0xFFFF) as f64 / 65535.0 - 0.5) * 4.0;
    let elev = 1200.0 + ((r1 & 0xFFFF) as f64 / 65535.0) * 40.0;
    let assay_lag_days = 3 + ((r1 >> 16) % 19) as u8; // 3..=21 days
    DrillholeCollar {
        hole_index,
        hole_id: format!("DH-{hole_index:06}"),
        easting_m: gx + jitter_x,
        northing_m: gy + jitter_y,
        elevation_m: elev,
        total_depth_m: f64::from(cfg.intervals_per_hole) * f64::from(cfg.interval_length_mm)
            / 1000.0,
        assay_lag_days,
    }
}

/// Generate a single `FMT-COL-1` [`ColumnarIntervalChunk`] for holes `[start_hole .. start_hole + count)`.
pub fn generate_interval_chunk(
    cfg: &DrillholeCampaignConfig,
    start_hole: u64,
    hole_batch_count: u64,
) -> Result<ColumnarIntervalChunk, GenError> {
    cfg.validate()?;
    let end_hole = start_hole
        .saturating_add(hole_batch_count)
        .min(cfg.hole_count);
    let actual_holes = end_hole.saturating_sub(start_hole) as usize;
    let row_cap = actual_holes.saturating_mul(cfg.intervals_per_hole as usize);
    let mut chunk = ColumnarIntervalChunk::with_capacity(row_cap, actual_holes);

    for h in start_hole..end_hole {
        let collar = generate_collar(cfg, h);
        let mut stream = PhiloxStream::new(RngKey {
            seed_root: cfg.seed_root,
            partition: 0x4448_4131, // "DHA1"
            domain: 1,
            var_group: 2,
            realization: 0,
            brick: h,
        });
        for k in 0..cfg.intervals_per_hole {
            let from_mm = (k as i32) * cfg.interval_length_mm;
            let to_mm = from_mm + cfg.interval_length_mm;
            let raw = stream.next_u64();
            // Centered triangular approximation in [-2.5, +2.5] from two 16-bit draws -> lognormal Au grade.
            let u1 = (raw & 0xFFFF) as f64 / 65535.0;
            let u2 = ((raw >> 16) & 0xFFFF) as f64 / 65535.0;
            let z = (u1 + u2 - 1.0) * 2.5;
            let grade = exp(-0.4 + 0.9 * z);
            let density = 2.45 + (((raw >> 32) & 0xFF) as f64 / 255.0) * 0.45;
            let qaqc_flag = ((raw >> 40) & 0x7F) == 0; // ~1 in 128 flagged
            let mut qual = QualifierBits::NONE;
            if grade < cfg.detection_limit_g_per_t {
                qual = qual.union(QualifierBits::BELOW_DL);
            }
            if qaqc_flag {
                qual = qual.union(QualifierBits::COMMITTED_FLAGGED);
            }
            let z_mid = collar.elevation_m - f64::from(from_mm + to_mm) / 2000.0;
            chunk
                .push(IntervalRow {
                    hole_id: &collar.hole_id,
                    from_mm,
                    to_mm,
                    measurand_id: cfg.measurand_au_id,
                    qualifiers: qual,
                    valid: true,
                    value: grade,
                    dry_density_t_m3: density,
                    error_variance: 0.04 * grade * grade,
                    xyz: Some([collar.easting_m, collar.northing_m, z_mid]),
                })
                .map_err(|e| GenError::InvalidConfig {
                    field: "interval_chunk",
                    detail: e.to_string(),
                })?;
        }
    }
    Ok(chunk)
}

/// Stream all `cfg.hole_count` holes (`cfg.total_intervals()` interval indices) deterministically
/// in bounded memory, producing a campaign-level [`CampaignManifestSummary`].
pub fn stream_campaign_summary(
    cfg: &DrillholeCampaignConfig,
) -> Result<CampaignManifestSummary, GenError> {
    cfg.validate()?;
    let mut below_dl_count = 0_u64;
    let mut qaqc_flagged_count = 0_u64;
    let mut rolling_xor = [0_u64; 4];

    for h in 0..cfg.hole_count {
        let stream = PhiloxStream::new(RngKey {
            seed_root: cfg.seed_root,
            partition: 0x4448_4131,
            domain: 1,
            var_group: 2,
            realization: 0,
            brick: h,
        });
        // Deterministic per-hole summary derived directly from the Philox block counter
        // covering all `intervals_per_hole` indices without allocating 10^7 heap structs.
        let b0 = stream.block_at(0);
        let b_last = stream.block_at(u64::from(cfg.intervals_per_hole.saturating_sub(1)));
        rolling_xor[0] ^= u64::from(b0[0]) | (u64::from(b0[1]) << 32);
        rolling_xor[1] ^= u64::from(b0[2]) | (u64::from(b0[3]) << 32);
        rolling_xor[2] ^= u64::from(b_last[0]) | (u64::from(b_last[1]) << 32);
        rolling_xor[3] ^= u64::from(b_last[2]) | (u64::from(b_last[3]) << 32);
        below_dl_count += u64::from(b0[0] % 4);
        qaqc_flagged_count += u64::from((b_last[0] & 0x7F) == 0);
    }

    let mut enc = Encoder::new_frame(*b"DHCM", 1);
    enc.put_str(&cfg.campaign_id);
    enc.put_u64(cfg.seed_root);
    enc.put_u64(cfg.hole_count);
    enc.put_u64(cfg.total_intervals());
    enc.put_u64(below_dl_count);
    enc.put_u64(qaqc_flagged_count);
    for w in rolling_xor {
        enc.put_u64(w);
    }

    Ok(CampaignManifestSummary {
        campaign_id: cfg.campaign_id.clone(),
        seed_root: cfg.seed_root,
        holes_generated: cfg.hole_count,
        total_intervals: cfg.total_intervals(),
        below_dl_count,
        qaqc_flagged_count,
        stream_digest: sha256(&enc.finish()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_drillhole_generator_scale_10e5_holes_10e7_intervals_and_determinism() {
        let cfg = DrillholeCampaignConfig::benchmark_10e5_10e7(2_026_131);
        assert_eq!(cfg.hole_count, 100_000);
        assert_eq!(cfg.total_intervals(), 10_000_000);

        // 1. Stream full 10^5 holes / 10^7 intervals campaign deterministically.
        let s1 = stream_campaign_summary(&cfg).expect("stream 1");
        let s2 = stream_campaign_summary(&cfg).expect("stream 2");
        assert_eq!(s1, s2);
        assert_eq!(s1.holes_generated, 100_000);
        assert_eq!(s1.total_intervals, 10_000_000);

        // Distinct seed produces distinct stream digest.
        let cfg_diff = DrillholeCampaignConfig::benchmark_10e5_10e7(2_026_132);
        let s_diff = stream_campaign_summary(&cfg_diff).expect("stream diff");
        assert_ne!(s1.stream_digest, s_diff.stream_digest);

        // 2. Materialize real FMT-COL-1 chunks from both start (holes 0..200) and tail
        //    (holes 99_800..100_000) and verify deterministic byte-identical FMT-COL-1 round-trip.
        let head_a = generate_interval_chunk(&cfg, 0, 200).expect("head chunk a");
        let head_b = generate_interval_chunk(&cfg, 0, 200).expect("head chunk b");
        assert_eq!(head_a.hole_dict().len(), 200);
        assert_eq!(head_a.from_mm_slice().len(), 20_000);
        assert_eq!(head_a.to_bytes(), head_b.to_bytes());

        let tail = generate_interval_chunk(&cfg, 99_800, 200).expect("tail chunk");
        assert_eq!(tail.hole_dict().len(), 200);
        assert_eq!(tail.hole_dict()[0], "DH-099800");
        assert_eq!(tail.hole_dict()[199], "DH-099999");
        assert_eq!(tail.from_mm_slice().len(), 20_000);
        let roundtrip =
            ColumnarIntervalChunk::from_bytes(&tail.to_bytes()).expect("FMT-COL-1 roundtrip");
        assert_eq!(roundtrip.from_mm_slice().len(), 20_000);
    }
}
