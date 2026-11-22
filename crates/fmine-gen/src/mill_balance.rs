//! Determinism: BitwisePortable
//! Mill sampler and metallurgical balance generator (`BD-133`, plan §33.3).
//!
//! Couples the sealed Cu–Au flotation world (`BD-129`) with seeded sampler/moisture biases
//! (`BD-130`) to emit shift/period feed, concentrate, and tailings sampler records separately from
//! their exact mass-conserving sealed metallurgical balance truth (`GATE-GEN-01`), including exact
//! two-product formula recovery `R = c(f - t) / (f(c - t))` and mass pull `(f - t) / (c - t)`.

use fmine_core::codec::Encoder;
use fmine_core::det::norm_ppf;
use fmine_core::rand::{PhiloxStream, RngKey};

use crate::GenError;
use crate::biases::{BiasRegistry, SeededBiasProfile};
use crate::cu_au_world::{CuAuFlotationSpec, generate_cu_au_flotation_truth};
use crate::seal::{SealKey, SealedTruth};

/// Configuration for the mill sampler and metallurgical balance generator (`BD-133`).
#[derive(Debug, Clone, PartialEq)]
pub struct MillBalanceConfig {
    pub plant_id: String,
    pub periods: usize,
    pub blocks_per_period: usize,
    pub feed_weightometer_sigma: f64,
    pub assay_rel_sigma: f64,
    pub biases: SeededBiasProfile,
    pub cu_au_spec: CuAuFlotationSpec,
}

impl MillBalanceConfig {
    /// Standard 24-period Cu–Au concentrator balance benchmark (100 blocks/period).
    #[must_use]
    pub fn standard() -> Self {
        Self {
            plant_id: "PLANT-FLOT-01".to_string(),
            periods: 24,
            blocks_per_period: 100,
            feed_weightometer_sigma: 0.015,
            assay_rel_sigma: 0.02,
            biases: SeededBiasProfile {
                sampler: 0.96,
                moisture: 1.08,
                ..SeededBiasProfile::unbiased()
            },
            cu_au_spec: CuAuFlotationSpec::standard(),
        }
    }
}

/// Noisy, potentially biased plant sampler observations handed to engines under test.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MillPeriodObservation {
    pub period_index: usize,
    pub obs_feed_dry_tonnes: f64,
    pub obs_feed_cu_pct: f64,
    pub obs_feed_au_g_per_t: f64,
    pub obs_conc_dry_tonnes: f64,
    pub obs_conc_cu_pct: f64,
    pub obs_conc_au_g_per_t: f64,
    pub obs_tail_cu_pct: f64,
    pub obs_tail_au_g_per_t: f64,
    /// Two-product formula mass pull implied by observed Cu assays `(f - t) / (c - t)`.
    pub obs_two_product_mass_pull: f64,
    /// Two-product formula Cu recovery implied by observed Cu assays.
    pub obs_two_product_cu_recovery: f64,
}

/// Public observation bundle for one mill balance run.
#[derive(Debug, Clone, PartialEq)]
pub struct MillBalanceObservations {
    pub plant_id: String,
    pub seed_root: u64,
    pub periods: Vec<MillPeriodObservation>,
}

/// Exact mass-conserving ground truth for one period inside [`SealedTruth<MillBalanceTruth>`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MillPeriodTruth {
    pub period_index: usize,
    pub true_feed_dry_tonnes: f64,
    pub true_conc_dry_tonnes: f64,
    pub true_tail_dry_tonnes: f64,
    pub true_feed_cu_pct: f64,
    pub true_conc_cu_pct: f64,
    pub true_tail_cu_pct: f64,
    pub true_feed_au_g_per_t: f64,
    pub true_conc_au_g_per_t: f64,
    pub true_tail_au_g_per_t: f64,
    pub true_cu_recovery: f64,
    pub true_au_recovery: f64,
    pub two_product_cu_recovery: f64,
}

/// Sealed ground truth for a mill metallurgical balance run (`BD-133`, `GATE-GEN-01`).
#[derive(Debug, Clone, PartialEq)]
pub struct MillBalanceTruth {
    pub plant_id: String,
    pub seed_root: u64,
    pub biases: SeededBiasProfile,
    pub periods: Vec<MillPeriodTruth>,
}

impl MillBalanceTruth {
    pub(crate) fn canonical_bytes(&self) -> Vec<u8> {
        let mut enc = Encoder::new_frame(*b"MILL", 1);
        enc.put_str(&self.plant_id);
        enc.put_u64(self.seed_root);
        enc.put_u32(self.periods.len() as u32);
        for p in &self.periods {
            enc.put_u64(p.true_feed_dry_tonnes.to_bits());
            enc.put_u64(p.true_conc_dry_tonnes.to_bits());
            enc.put_u64(p.true_tail_dry_tonnes.to_bits());
            enc.put_u64(p.true_cu_recovery.to_bits());
            enc.put_u64(p.true_au_recovery.to_bits());
        }
        enc.finish()
    }
}

/// Exact two-product metallurgical mass pull `C/F = (f - t) / (c - t)` and recovery
/// `R = c(f - t) / (f(c - t))` (plan §33.2 / §17.5).
pub fn two_product_balance(
    feed_grade: f64,
    conc_grade: f64,
    tail_grade: f64,
) -> Result<(f64, f64), GenError> {
    if !(feed_grade.is_finite()
        && conc_grade.is_finite()
        && tail_grade.is_finite()
        && feed_grade > 0.0
        && conc_grade > feed_grade
        && feed_grade > tail_grade
        && tail_grade >= 0.0)
    {
        return Err(GenError::InvalidConfig {
            field: "two_product_grades",
            detail: format!(
                "require 0 <= tail ({tail_grade}) < feed ({feed_grade}) < conc ({conc_grade})"
            ),
        });
    }
    let mass_pull = (feed_grade - tail_grade) / (conc_grade - tail_grade);
    let recovery = (conc_grade * mass_pull) / feed_grade;
    Ok((mass_pull, recovery))
}

fn draw_normal(stream: &mut PhiloxStream) -> f64 {
    let u = ((stream.next_u64() >> 11) as f64) * (1.0 / ((1u64 << 53) as f64));
    norm_ppf(1e-12 + u * (1.0 - 2e-12))
}

/// Generate separated mill observations and sealed metallurgical balance truth (`BD-133`).
pub(crate) fn generate_mill_balance_raw(
    seed_root: u64,
    cfg: &MillBalanceConfig,
    registry: Option<&BiasRegistry>,
    key: &SealKey,
) -> Result<(MillBalanceObservations, SealedTruth<MillBalanceTruth>), GenError> {
    if cfg.plant_id.is_empty() || cfg.periods == 0 || cfg.blocks_per_period == 0 {
        return Err(GenError::InvalidConfig {
            field: "mill_balance_config",
            detail: "plant_id, periods, and blocks_per_period must be non-zero".to_string(),
        });
    }
    if !(cfg.feed_weightometer_sigma.is_finite()
        && (0.0..=0.25).contains(&cfg.feed_weightometer_sigma))
        || !(cfg.assay_rel_sigma.is_finite() && (0.0..=0.25).contains(&cfg.assay_rel_sigma))
    {
        return Err(GenError::InvalidConfig {
            field: "mill_noise_sigma",
            detail: "feed_weightometer_sigma and assay_rel_sigma must be finite and in [0, 0.25]"
                .to_string(),
        });
    }
    let bias_vals = [
        cfg.biases.resource_model,
        cfg.biases.dilution,
        cfg.biases.survey,
        cfg.biases.sampler,
        cfg.biases.density,
        cfg.biases.moisture,
        cfg.biases.truck_payload,
    ];
    if !bias_vals.iter().all(|v| v.is_finite() && *v > 0.0)
        || !(0.0..12.0).contains(&cfg.biases.moisture)
    {
        return Err(GenError::InvalidConfig {
            field: "biases",
            detail: "all bias factors must be finite and > 0 (and moisture < 12.0)".to_string(),
        });
    }
    if let Some(reg) = registry {
        cfg.biases.validate_against(reg)?;
    }

    let sealed_world =
        generate_cu_au_flotation_truth(seed_root ^ 0x4D49_4C4C, &cfg.cu_au_spec, key)?;
    let world = sealed_world.open(key)?;
    let needed_blocks = cfg
        .periods
        .checked_mul(cfg.blocks_per_period)
        .ok_or_else(|| GenError::InvalidConfig {
            field: "periods",
            detail: "periods * blocks_per_period overflowed usize".to_string(),
        })?;
    if needed_blocks > world.blocks.len() {
        return Err(GenError::InvalidConfig {
            field: "periods",
            detail: format!(
                "requested {needed_blocks} blocks exceed generated Cu-Au world size {}",
                world.blocks.len()
            ),
        });
    }

    let mut stream = PhiloxStream::new(RngKey {
        seed_root,
        partition: 0x4D49_4C4C, // "MILL"
        domain: 1,
        var_group: 1,
        realization: 0,
        brick: 0,
    });

    let mut obs_periods = Vec::with_capacity(cfg.periods);
    let mut truth_periods = Vec::with_capacity(cfg.periods);

    for p in 0..cfg.periods {
        let start = p * cfg.blocks_per_period;
        let slice = &world.blocks[start..start + cfg.blocks_per_period];

        let mut feed_t = 0.0_f64;
        let mut conc_t = 0.0_f64;
        let mut tail_t = 0.0_f64;
        let mut feed_cu = 0.0_f64;
        let mut conc_cu = 0.0_f64;
        let mut tail_cu = 0.0_f64;
        let mut feed_au = 0.0_f64;
        let mut conc_au = 0.0_f64;
        let mut tail_au = 0.0_f64;

        for b in slice {
            feed_t += b.dry_tonnes;
            conc_t += b.conc_dry_tonnes;
            tail_t += b.tail_dry_tonnes;
            feed_cu += b.dry_tonnes * (b.cu_total_pct / 100.0);
            conc_cu += b.conc_dry_tonnes * (b.conc_cu_pct / 100.0);
            tail_cu += b.tail_dry_tonnes * (b.tail_cu_pct / 100.0);
            feed_au += b.dry_tonnes * b.au_grade_g_per_t;
            conc_au += b.conc_dry_tonnes * b.conc_au_g_per_t;
            tail_au += b.tail_dry_tonnes * b.tail_au_g_per_t;
        }

        let true_feed_cu_pct = (feed_cu / feed_t) * 100.0;
        let true_conc_cu_pct = (conc_cu / conc_t) * 100.0;
        let true_tail_cu_pct = (tail_cu / tail_t) * 100.0;
        let true_feed_au_gt = feed_au / feed_t;
        let true_conc_au_gt = conc_au / conc_t;
        let true_tail_au_gt = tail_au / tail_t;

        let true_cu_rec = conc_cu / feed_cu;
        let true_au_rec = conc_au / feed_au;
        let (_tp_pull, tp_cu_rec) =
            two_product_balance(true_feed_cu_pct, true_conc_cu_pct, true_tail_cu_pct)?;

        // Biased & noisy observations at feed weightometer, head sampler, conc weighbridge, tails sampler.
        // Multiplicative lognormal noise guarantees strictly positive readings in all Gaussian tails.
        let dry_moist_bias = (1.0 - 0.08 * cfg.biases.moisture) / (1.0 - 0.08);
        let obs_feed_t = feed_t
            * dry_moist_bias
            * fmine_core::det::exp(cfg.feed_weightometer_sigma * draw_normal(&mut stream));
        let obs_feed_cu = true_feed_cu_pct
            * cfg.biases.sampler
            * fmine_core::det::exp(cfg.assay_rel_sigma * draw_normal(&mut stream));
        let obs_feed_au = true_feed_au_gt
            * cfg.biases.sampler
            * fmine_core::det::exp(cfg.assay_rel_sigma * draw_normal(&mut stream));
        // Concentrate custody is high-precision (quarter noise, unbiased weighbridge).
        let obs_conc_t = conc_t
            * fmine_core::det::exp(0.25 * cfg.feed_weightometer_sigma * draw_normal(&mut stream));
        let obs_conc_cu = true_conc_cu_pct
            * fmine_core::det::exp(0.25 * cfg.assay_rel_sigma * draw_normal(&mut stream));
        let obs_conc_au = true_conc_au_gt
            * fmine_core::det::exp(0.25 * cfg.assay_rel_sigma * draw_normal(&mut stream));
        let obs_tail_cu = (true_tail_cu_pct
            * fmine_core::det::exp(cfg.assay_rel_sigma * draw_normal(&mut stream)))
        .min(obs_feed_cu * 0.95)
        .max(1e-6);
        let obs_tail_au = (true_tail_au_gt
            * fmine_core::det::exp(cfg.assay_rel_sigma * draw_normal(&mut stream)))
        .max(1e-6);

        let (obs_tp_pull, obs_tp_rec) = two_product_balance(obs_feed_cu, obs_conc_cu, obs_tail_cu)?;

        obs_periods.push(MillPeriodObservation {
            period_index: p,
            obs_feed_dry_tonnes: obs_feed_t,
            obs_feed_cu_pct: obs_feed_cu,
            obs_feed_au_g_per_t: obs_feed_au,
            obs_conc_dry_tonnes: obs_conc_t,
            obs_conc_cu_pct: obs_conc_cu,
            obs_conc_au_g_per_t: obs_conc_au,
            obs_tail_cu_pct: obs_tail_cu,
            obs_tail_au_g_per_t: obs_tail_au,
            obs_two_product_mass_pull: obs_tp_pull,
            obs_two_product_cu_recovery: obs_tp_rec,
        });
        truth_periods.push(MillPeriodTruth {
            period_index: p,
            true_feed_dry_tonnes: feed_t,
            true_conc_dry_tonnes: conc_t,
            true_tail_dry_tonnes: tail_t,
            true_feed_cu_pct,
            true_conc_cu_pct,
            true_tail_cu_pct,
            true_feed_au_g_per_t: true_feed_au_gt,
            true_conc_au_g_per_t: true_conc_au_gt,
            true_tail_au_g_per_t: true_tail_au_gt,
            true_cu_recovery: true_cu_rec,
            true_au_recovery: true_au_rec,
            two_product_cu_recovery: tp_cu_rec,
        });
    }

    let obs = MillBalanceObservations {
        plant_id: cfg.plant_id.clone(),
        seed_root,
        periods: obs_periods,
    };
    let truth = MillBalanceTruth {
        plant_id: cfg.plant_id.clone(),
        seed_root,
        biases: cfg.biases,
        periods: truth_periods,
    };
    let bytes = truth.canonical_bytes();
    let sealed = SealedTruth::seal(&cfg.plant_id, seed_root, &bytes, key, truth);
    Ok((obs, sealed))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    use crate::biases::parse_bias_registry;
    use crate::harness::ScoringHarness;

    #[test]
    fn test_mill_sampler_and_metallurgical_balance_generator_determinism() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|p| p.parent())
            .expect("workspace root")
            .to_path_buf();
        let reg_text =
            std::fs::read_to_string(root.join("registries/biases.toml")).expect("read biases.toml");
        let reg = parse_bias_registry(&reg_text).expect("parse bias registry");

        let cfg = MillBalanceConfig::standard();
        let h1 = ScoringHarness::new(2_026, "mill-A");
        let h2 = ScoringHarness::new(2_026, "mill-A");
        let other = ScoringHarness::new(2_026, "mill-B");

        let (obs1, seal1) = h1
            .generate_mill_balance(2_026_133, &cfg, Some(&reg))
            .expect("gen 1");
        let (obs2, seal2) = h2
            .generate_mill_balance(2_026_133, &cfg, Some(&reg))
            .expect("gen 2");
        assert_eq!(obs1, obs2);
        assert_eq!(seal1.truth_commitment(), seal2.truth_commitment());
        assert_eq!(obs1.periods.len(), 24);

        // Wrong harness cannot open sealed mill balance truth (GATE-GEN-01).
        let err = other.open_mill_balance_truth(&seal1).unwrap_err();
        assert_eq!(err.to_refusal().code, "FMINE-REF-301");

        // Verify exact mass conservation and exact two-product formula identity on sealed truth.
        let truth = h1.open_mill_balance_truth(&seal1).expect("open truth");
        for p in &truth.periods {
            let mass_err =
                (p.true_feed_dry_tonnes - (p.true_conc_dry_tonnes + p.true_tail_dry_tonnes)).abs()
                    / p.true_feed_dry_tonnes;
            assert!(
                mass_err < 1e-12,
                "period {} mass err {mass_err}",
                p.period_index
            );
            assert!(
                (p.true_cu_recovery - p.two_product_cu_recovery).abs() < 1e-12,
                "period {} two-product Cu recovery mismatch: {} vs {}",
                p.period_index,
                p.true_cu_recovery,
                p.two_product_cu_recovery
            );
        }

        // Seeded 0.96 head-sampler bias systematically shifts observed feed Cu below true feed Cu.
        let mean_ratio: f64 = obs1
            .periods
            .iter()
            .zip(truth.periods.iter())
            .map(|(o, t)| o.obs_feed_cu_pct / t.true_feed_cu_pct)
            .sum::<f64>()
            / (obs1.periods.len() as f64);
        assert!(
            (mean_ratio - 0.96).abs() < 0.015,
            "expected mean observed/true head Cu ~ 0.96, got {mean_ratio}"
        );

        // Regression: out-of-range noise sigma and non-positive bias factor refuse without panicking.
        let mut bad_sigma = cfg.clone();
        bad_sigma.assay_rel_sigma = 0.50;
        assert!(
            h1.generate_mill_balance(2_026_133, &bad_sigma, None)
                .is_err()
        );
        let mut bad_bias = cfg;
        bad_bias.biases.sampler = 0.0;
        assert!(
            h1.generate_mill_balance(2_026_133, &bad_bias, None)
                .is_err()
        );
    }
}
