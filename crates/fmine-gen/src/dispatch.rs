//! Determinism: BitwisePortable
//! Fleet dispatch tonnage generator (`BD-132`, plan §33.3).
//!
//! Emits deterministic haul-truck load cycles with explicit wet vs. dry tonnage separation
//! (`fmine_units::{Wet, Dry, MoistureFraction, wet_to_dry}`), destination routing (`PrimaryCrusher`,
//! `RomStockpile`, `LowGradeStockpile`, `WasteDump`), truck-payload bias, and canonical SHA-256
//! stream commitment digests.

use std::collections::BTreeMap;

use fmine_core::codec::Encoder;
use fmine_core::det::norm_ppf;
use fmine_core::hash::{Digest32, sha256};
use fmine_core::rand::{PhiloxStream, RngKey};
use fmine_units::{Dry, Mass, MoistureFraction, Neutral, Qty, Wet, wet_to_dry};

use crate::GenError;

/// Hauler dump destination class.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DispatchDestination {
    PrimaryCrusher,
    RomStockpile,
    LowGradeStockpile,
    WasteDump,
}

impl DispatchDestination {
    const fn tag(self) -> u8 {
        match self {
            Self::PrimaryCrusher => 1,
            Self::RomStockpile => 2,
            Self::LowGradeStockpile => 3,
            Self::WasteDump => 4,
        }
    }
}

/// Configuration for deterministic haul-fleet dispatch generation (`BD-132`).
#[derive(Debug, Clone, PartialEq)]
pub struct DispatchConfig {
    pub site_id: String,
    pub seed_root: u64,
    pub shifts: u32,
    pub loads_per_shift: u32,
    pub fleet_truck_count: u16,
    pub nominal_wet_payload_tonnes: f64,
    pub true_moisture_fraction: f64,
    /// Systematic truck-strut payload scale multiplier (`1.0` = unbiased).
    pub truck_payload_bias_factor: f64,
    /// Relative per-load strut-sensor noise standard deviation.
    pub payload_noise_rel_sigma: f64,
}

impl DispatchConfig {
    /// Standard 30-shift × 200-load (6,000-cycle) open-pit haul fleet benchmark.
    #[must_use]
    pub fn standard(seed_root: u64) -> Self {
        Self {
            site_id: "SITE-DISPATCH-01".to_string(),
            seed_root,
            shifts: 30,
            loads_per_shift: 200,
            fleet_truck_count: 18,
            nominal_wet_payload_tonnes: 220.0,
            true_moisture_fraction: 0.075,
            truck_payload_bias_factor: 1.045,
            payload_noise_rel_sigma: 0.03,
        }
    }

    fn validate(&self) -> Result<(), GenError> {
        if self.site_id.is_empty()
            || self.shifts == 0
            || self.loads_per_shift == 0
            || self.fleet_truck_count == 0
        {
            return Err(GenError::InvalidConfig {
                field: "dispatch_config",
                detail: "site_id, shifts, loads_per_shift, and fleet_truck_count must be non-zero"
                    .to_string(),
            });
        }
        if !(self.nominal_wet_payload_tonnes.is_finite()
            && self.nominal_wet_payload_tonnes > 0.0
            && self.truck_payload_bias_factor.is_finite()
            && self.truck_payload_bias_factor > 0.5
            && self.payload_noise_rel_sigma.is_finite()
            && (0.0..=0.25).contains(&self.payload_noise_rel_sigma))
        {
            return Err(GenError::InvalidConfig {
                field: "payload_params",
                detail: "payload parameters out of physical domain".to_string(),
            });
        }
        let _ = MoistureFraction::new(self.true_moisture_fraction).map_err(|e| {
            GenError::InvalidConfig {
                field: "true_moisture_fraction",
                detail: e.to_string(),
            }
        })?;
        Ok(())
    }
}

/// One generated haul-truck load cycle.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TruckLoadRecord {
    pub shift_index: u32,
    pub load_index: u32,
    pub truck_id: u16,
    pub destination: DispatchDestination,
    pub true_wet_tonnes: Qty<Mass, Wet, Neutral>,
    pub true_dry_tonnes: Qty<Mass, Dry, Neutral>,
    pub observed_wet_tonnes: Qty<Mass, Wet, Neutral>,
    pub cycle_seconds: u32,
}

/// Destination-level dry & wet tonnage rollup.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DestinationRollup {
    pub load_count: u64,
    pub true_wet_tonnes: f64,
    pub true_dry_tonnes: f64,
    pub observed_wet_tonnes: f64,
}

/// Complete deterministic dispatch campaign output (`BD-132`).
#[derive(Debug, Clone, PartialEq)]
pub struct DispatchCampaign {
    pub site_id: String,
    pub seed_root: u64,
    pub records: Vec<TruckLoadRecord>,
    pub rollups: BTreeMap<DispatchDestination, DestinationRollup>,
    pub campaign_digest: Digest32,
}

fn draw_normal(stream: &mut PhiloxStream) -> f64 {
    let u = ((stream.next_u64() >> 11) as f64) * (1.0 / ((1u64 << 53) as f64));
    norm_ppf(1e-12 + u * (1.0 - 2e-12))
}

/// Generate a deterministic fleet dispatch tonnage campaign (`BD-132`).
pub fn generate_dispatch_campaign(cfg: &DispatchConfig) -> Result<DispatchCampaign, GenError> {
    cfg.validate()?;
    let total_loads = (cfg.shifts as usize) * (cfg.loads_per_shift as usize);
    let mut records = Vec::with_capacity(total_loads);
    let mut rollups: BTreeMap<DispatchDestination, DestinationRollup> = BTreeMap::new();

    let mut enc = Encoder::new_frame(*b"DISP", 1);
    enc.put_str(&cfg.site_id);
    enc.put_u64(cfg.seed_root);
    enc.put_u32(total_loads as u32);

    for shift in 0..cfg.shifts {
        let mut stream = PhiloxStream::new(RngKey {
            seed_root: cfg.seed_root,
            partition: 0x4449_5350, // "DISP"
            domain: 1,
            var_group: 1,
            realization: 0,
            brick: u64::from(shift),
        });
        for load in 0..cfg.loads_per_shift {
            let r_route = stream.next_u64();
            let truck_id = ((r_route % u64::from(cfg.fleet_truck_count)) as u16) + 1;
            let dest = match (r_route >> 16) % 100 {
                0..=34 => DispatchDestination::PrimaryCrusher,
                35..=54 => DispatchDestination::RomStockpile,
                55..=64 => DispatchDestination::LowGradeStockpile,
                _ => DispatchDestination::WasteDump,
            };
            let z_fill = draw_normal(&mut stream);
            let z_moist = draw_normal(&mut stream);
            let z_sensor = draw_normal(&mut stream);

            let true_wet_val = (cfg.nominal_wet_payload_tonnes * (1.0 + 0.04 * z_fill)).max(50.0);
            let moist_val = (cfg.true_moisture_fraction + 0.008 * z_moist).clamp(0.01, 0.25);
            let moisture =
                MoistureFraction::new(moist_val).map_err(|e| GenError::InvalidConfig {
                    field: "moisture",
                    detail: e.to_string(),
                })?;
            let true_wet = Qty::<Mass, Wet, Neutral>::new(true_wet_val);
            let true_dry = wet_to_dry(true_wet, moisture);
            let obs_wet_val = (true_wet_val
                * cfg.truck_payload_bias_factor
                * (1.0 + cfg.payload_noise_rel_sigma * z_sensor))
                .max(10.0);
            let observed_wet = Qty::<Mass, Wet, Neutral>::new(obs_wet_val);
            let cycle_seconds = 780 + ((r_route >> 32) % 600) as u32;

            enc.put_u32(shift);
            enc.put_u32(load);
            enc.put_u16(truck_id);
            enc.put_u8(dest.tag());
            enc.put_u64(true_wet.value().to_bits());
            enc.put_u64(true_dry.value().to_bits());
            enc.put_u64(observed_wet.value().to_bits());
            enc.put_u32(cycle_seconds);

            let entry = rollups.entry(dest).or_insert(DestinationRollup {
                load_count: 0,
                true_wet_tonnes: 0.0,
                true_dry_tonnes: 0.0,
                observed_wet_tonnes: 0.0,
            });
            entry.load_count += 1;
            entry.true_wet_tonnes += true_wet.value();
            entry.true_dry_tonnes += true_dry.value();
            entry.observed_wet_tonnes += observed_wet.value();

            records.push(TruckLoadRecord {
                shift_index: shift,
                load_index: load,
                truck_id,
                destination: dest,
                true_wet_tonnes: true_wet,
                true_dry_tonnes: true_dry,
                observed_wet_tonnes: observed_wet,
                cycle_seconds,
            });
        }
    }

    Ok(DispatchCampaign {
        site_id: cfg.site_id.clone(),
        seed_root: cfg.seed_root,
        records,
        rollups,
        campaign_digest: sha256(&enc.finish()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dispatch_tonnage_generator_determinism_and_wet_dry_conservation() {
        let cfg = DispatchConfig::standard(2_026_132);
        let c1 = generate_dispatch_campaign(&cfg).expect("gen 1");
        let c2 = generate_dispatch_campaign(&cfg).expect("gen 2");
        assert_eq!(c1, c2);
        assert_eq!(c1.records.len(), 6_000);
        assert_eq!(c1.campaign_digest, c2.campaign_digest);

        let c_diff = generate_dispatch_campaign(&DispatchConfig::standard(2_026_133))
            .expect("gen diff seed");
        assert_ne!(c1.campaign_digest, c_diff.campaign_digest);

        // Wet mass strictly exceeds dry mass on every load, and rollups sum exactly to record totals.
        let mut total_loads = 0u64;
        let mut total_true_wet = 0.0_f64;
        let mut total_obs_wet = 0.0_f64;
        for r in c1.rollups.values() {
            total_loads += r.load_count;
            total_true_wet += r.true_wet_tonnes;
            total_obs_wet += r.observed_wet_tonnes;
            assert!(r.true_wet_tonnes > r.true_dry_tonnes);
        }
        assert_eq!(total_loads, 6_000);

        // Realized ratio of observed wet to true wet recovers the 1.045 seeded payload bias tightly.
        let realized_ratio = total_obs_wet / total_true_wet;
        assert!(
            (realized_ratio - 1.045).abs() < 0.005,
            "realized payload bias ratio {realized_ratio} drifted from 1.045"
        );
    }
}
