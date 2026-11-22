//! Determinism: BitwisePortable
//! Sealed-truth scoring harness (`BD-127`, plan §33.3, `GATE-GEN-01`).
//!
//! Only `ScoringHarness` holds the [`SealKey`] needed to open a world's [`SealedTruth`]. Engines
//! under test never receive the key or truth struct; scoring happens strictly post-hoc.

use fmine_core::det::sqrt;
use fmine_core::hash::Digest32;

use crate::GenError;
use crate::biases::BiasRegistry;
use crate::cu_au_world::{CuAuFlotationSpec, CuAuFlotationTruth, generate_cu_au_flotation_truth};
use crate::geology::{OpenPitAuGeologySpec, OpenPitAuGeologyTruth, generate_geology_truth};
use crate::mill_balance::{
    MillBalanceConfig, MillBalanceObservations, MillBalanceTruth, generate_mill_balance_raw,
};
use crate::seal::{SealKey, SealedTruth};
use crate::worlds::{
    OpenPitAuConfig, OpenPitAuObservations, OpenPitAuTruth, WorldCondition,
    generate_open_pit_au_raw,
};

/// Predictive interval submitted by an engine under test for one period's true contained Au.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct IntervalPrediction {
    pub period_index: usize,
    pub point_estimate: f64,
    pub lower_80: f64,
    pub upper_80: f64,
}

impl IntervalPrediction {
    fn validate(&self) -> Result<(), GenError> {
        if !(self.point_estimate.is_finite()
            && self.lower_80.is_finite()
            && self.upper_80.is_finite())
        {
            return Err(GenError::InvalidPrediction {
                detail: format!("non-finite prediction at period {}", self.period_index),
            });
        }
        if self.lower_80 > self.upper_80 {
            return Err(GenError::InvalidPrediction {
                detail: format!(
                    "lower_80 ({}) > upper_80 ({}) at period {}",
                    self.lower_80, self.upper_80, self.period_index
                ),
            });
        }
        Ok(())
    }
}

/// Complete engine submission scored against sealed Open-Pit Au ground truth.
#[derive(Debug, Clone, PartialEq)]
pub struct OpenPitAuPrediction {
    pub estimated_resource_grade_factor: f64,
    pub estimated_sampler_grade_factor: f64,
    pub period_contained_au_grams: Vec<IntervalPrediction>,
}

/// Post-hoc scoring summary emitted by [`ScoringHarness`] after opening the seal.
#[derive(Debug, Clone, PartialEq)]
pub struct ScoreCard {
    pub world_id: String,
    pub seed_root: u64,
    pub truth_commitment: Digest32,
    pub periods_scored: usize,
    pub covered_80: usize,
    pub empirical_coverage_80: f64,
    pub relative_rmse: f64,
    pub resource_grade_bias_abs_error: f64,
    pub sampler_grade_bias_abs_error: f64,
}

/// Exclusive holder of the [`SealKey`] for generating and scoring synthetic-truth worlds (`GATE-GEN-01`).
#[derive(Debug)]
pub struct ScoringHarness {
    key: SealKey,
}

impl ScoringHarness {
    /// Mint a new scoring harness bound to `(seed_root, harness_tag)`.
    #[must_use]
    pub fn new(seed_root: u64, harness_tag: &str) -> Self {
        Self {
            key: SealKey::mint(seed_root, harness_tag),
        }
    }

    /// Generate an Open-Pit Au synthetic world, returning observations separately from its sealed truth.
    pub fn generate_open_pit_au(
        &self,
        seed_root: u64,
        cfg: &OpenPitAuConfig,
    ) -> Result<(OpenPitAuObservations, SealedTruth<OpenPitAuTruth>), GenError> {
        let (obs, truth) = generate_open_pit_au_raw(seed_root, cfg)?;
        let bytes = truth.canonical_bytes();
        let sealed = SealedTruth::seal(&cfg.world_id, seed_root, &bytes, &self.key, truth);
        Ok((obs, sealed))
    }

    /// Generate a sealed 3D Open-Pit Au geological block model (`BD-128`).
    pub fn generate_open_pit_au_geology(
        &self,
        seed_root: u64,
        spec: &OpenPitAuGeologySpec,
    ) -> Result<SealedTruth<OpenPitAuGeologyTruth>, GenError> {
        generate_geology_truth(seed_root, spec, &self.key)
    }

    /// Open a sealed 3D Open-Pit Au geology truth with this harness's private key (`GATE-GEN-01`).
    pub fn open_geology_truth<'a>(
        &self,
        sealed: &'a SealedTruth<OpenPitAuGeologyTruth>,
    ) -> Result<&'a OpenPitAuGeologyTruth, GenError> {
        Ok(sealed.open(&self.key)?)
    }

    /// Generate a sealed Open-Pit Cu–Au porphyry flotation world (`BD-129`).
    pub fn generate_cu_au_flotation(
        &self,
        seed_root: u64,
        spec: &CuAuFlotationSpec,
    ) -> Result<SealedTruth<CuAuFlotationTruth>, GenError> {
        generate_cu_au_flotation_truth(seed_root, spec, &self.key)
    }

    /// Open a sealed Open-Pit Cu–Au flotation world truth (`GATE-GEN-01`).
    pub fn open_cu_au_truth<'a>(
        &self,
        sealed: &'a SealedTruth<CuAuFlotationTruth>,
    ) -> Result<&'a CuAuFlotationTruth, GenError> {
        Ok(sealed.open(&self.key)?)
    }

    /// Generate separated mill sampler observations and sealed metallurgical balance truth (`BD-133`).
    pub fn generate_mill_balance(
        &self,
        seed_root: u64,
        cfg: &MillBalanceConfig,
        registry: Option<&BiasRegistry>,
    ) -> Result<(MillBalanceObservations, SealedTruth<MillBalanceTruth>), GenError> {
        generate_mill_balance_raw(seed_root, cfg, registry, &self.key)
    }

    /// Open a sealed mill balance truth with this harness's private key (`GATE-GEN-01`).
    pub fn open_mill_balance_truth<'a>(
        &self,
        sealed: &'a SealedTruth<MillBalanceTruth>,
    ) -> Result<&'a MillBalanceTruth, GenError> {
        Ok(sealed.open(&self.key)?)
    }

    /// Open `sealed_truth` with this harness's private key and score `pred` post-hoc.
    ///
    /// For [`WorldCondition::IdentifiabilityProbe`], refuses any unflagged attempt to claim separated
    /// individual bias factors with [`GenError::NonIdentifiableProbe`] (`FMINE-REF-104`).
    pub fn score_open_pit_au(
        &self,
        sealed_truth: &SealedTruth<OpenPitAuTruth>,
        pred: &OpenPitAuPrediction,
    ) -> Result<ScoreCard, GenError> {
        let truth = sealed_truth.open(&self.key)?;
        if truth.condition == WorldCondition::IdentifiabilityProbe {
            return Err(GenError::NonIdentifiableProbe {
                detail: format!(
                    "world '{}' is a confounded identifiability probe; individual bias separation must refuse",
                    truth.world_id
                ),
            });
        }
        if pred.period_contained_au_grams.len() != truth.periods.len() {
            return Err(GenError::InvalidPrediction {
                detail: format!(
                    "prediction has {} periods, expected {}",
                    pred.period_contained_au_grams.len(),
                    truth.periods.len()
                ),
            });
        }
        if !(pred.estimated_resource_grade_factor.is_finite()
            && pred.estimated_sampler_grade_factor.is_finite())
        {
            return Err(GenError::InvalidPrediction {
                detail: "estimated bias factors must be finite".to_string(),
            });
        }

        let mut covered = 0usize;
        let mut sq_rel_err_sum = 0.0_f64;
        for (p, t) in pred
            .period_contained_au_grams
            .iter()
            .zip(truth.periods.iter())
        {
            p.validate()?;
            if p.period_index != t.period_index {
                return Err(GenError::InvalidPrediction {
                    detail: format!(
                        "period index mismatch: prediction {} != truth {}",
                        p.period_index, t.period_index
                    ),
                });
            }
            if t.true_contained_au_grams >= p.lower_80 && t.true_contained_au_grams <= p.upper_80 {
                covered += 1;
            }
            let rel_err =
                (p.point_estimate - t.true_contained_au_grams) / t.true_contained_au_grams;
            sq_rel_err_sum += rel_err * rel_err;
        }
        let n = truth.periods.len();
        let empirical_coverage_80 = (covered as f64) / (n as f64);
        let relative_rmse = sqrt(sq_rel_err_sum / (n as f64));

        Ok(ScoreCard {
            world_id: truth.world_id.clone(),
            seed_root: truth.seed_root,
            truth_commitment: sealed_truth.truth_commitment(),
            periods_scored: n,
            covered_80: covered,
            empirical_coverage_80,
            relative_rmse,
            resource_grade_bias_abs_error: (pred.estimated_resource_grade_factor
                - truth.biases.resource_grade_factor)
                .abs(),
            sampler_grade_bias_abs_error: (pred.estimated_sampler_grade_factor
                - truth.biases.sampler_grade_factor)
                .abs(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::seal::SealError;
    use crate::worlds::{BiasVector, MisspecKind};

    fn sample_config(condition: WorldCondition) -> OpenPitAuConfig {
        OpenPitAuConfig {
            world_id: "au-openpit-001".to_string(),
            periods: 40,
            base_dry_tonnes_per_period: 100_000.0,
            base_true_grade_g_per_t: 2.0,
            measurement_rel_sigma: 0.02,
            biases: BiasVector {
                resource_grade_factor: 1.12,
                dilution_tonnes_factor: 1.05,
                moisture_factor: 1.00,
                sampler_grade_factor: 0.94,
            },
            condition,
        }
    }

    #[test]
    fn test_sealed_truth_isolation_and_deterministic_replay() {
        let cfg = sample_config(WorldCondition::CorrectlySpecified);
        let h1 = ScoringHarness::new(2026, "harness-A");
        let h2 = ScoringHarness::new(2026, "harness-A");
        let other = ScoringHarness::new(2026, "harness-B");

        let (obs1, seal1) = h1.generate_open_pit_au(77, &cfg).expect("gen 1");
        let (obs2, seal2) = h2.generate_open_pit_au(77, &cfg).expect("gen 2");

        // Same seed + config yields bit-identical observations and truth commitment.
        assert_eq!(obs1, obs2);
        assert_eq!(seal1.truth_commitment(), seal2.truth_commitment());
        assert!(format!("{seal1:?}").contains("<sealed>"));

        // Different seed produces distinct truth commitment.
        let (_, seal_diff) = h1.generate_open_pit_au(78, &cfg).expect("gen diff");
        assert_ne!(seal1.truth_commitment(), seal_diff.truth_commitment());

        // Another harness cannot open seal1 (GATE-GEN-01).
        let dummy_pred = OpenPitAuPrediction {
            estimated_resource_grade_factor: 1.0,
            estimated_sampler_grade_factor: 1.0,
            period_contained_au_grams: Vec::new(),
        };
        let err = other.score_open_pit_au(&seal1, &dummy_pred).unwrap_err();
        assert!(matches!(err, GenError::Seal(SealError::InvalidKey { .. })));
        assert_eq!(err.to_refusal().code, "FMINE-REF-301");

        // Identifiability probe refuses bias separation with FMINE-REF-104.
        let probe_cfg = sample_config(WorldCondition::IdentifiabilityProbe);
        let (_, probe_seal) = h1.generate_open_pit_au(99, &probe_cfg).expect("gen probe");
        let probe_err = h1.score_open_pit_au(&probe_seal, &dummy_pred).unwrap_err();
        assert_eq!(probe_err.to_refusal().code, "FMINE-REF-104");
    }

    #[test]
    fn test_scoring_harness_recovers_seeded_biases_and_coverage() {
        let cfg = sample_config(WorldCondition::CorrectlySpecified);
        let harness = ScoringHarness::new(2026, "scoring-suite");
        let (obs, sealed) = harness.generate_open_pit_au(4242, &cfg).expect("gen world");

        // Engine sees ONLY `obs` — estimates contained Au from refinery outturn / nominal_recovery,
        // and recovers resource_grade_factor & sampler_grade_factor from ratio of observed grades
        // against implied true grade (contained / truck_scale_tonnes).
        let mut rm_ratios = Vec::with_capacity(obs.periods.len());
        let mut samp_ratios = Vec::with_capacity(obs.periods.len());
        let mut preds = Vec::with_capacity(obs.periods.len());

        // 80% normal z-quantile ~ 1.28155; refinery sigma is 0.25 * 0.02 = 0.005 (use 0.015 half-width
        // reflecting honest measurement uncertainty).
        for p in &obs.periods {
            let est_contained = p.refinery_recovered_au_grams / obs.nominal_recovery;
            let implied_grade = est_contained / p.truck_scale_dry_tonnes;
            rm_ratios.push(p.resource_model_grade_g_per_t / implied_grade);
            samp_ratios.push(p.plant_sampler_grade_g_per_t / implied_grade);
            preds.push(IntervalPrediction {
                period_index: p.period_index,
                point_estimate: est_contained,
                lower_80: est_contained * (1.0 - 0.008),
                upper_80: est_contained * (1.0 + 0.008),
            });
        }
        let est_rm = rm_ratios.iter().sum::<f64>() / (rm_ratios.len() as f64);
        let est_samp = samp_ratios.iter().sum::<f64>() / (samp_ratios.len() as f64);

        let card = harness
            .score_open_pit_au(
                &sealed,
                &OpenPitAuPrediction {
                    estimated_resource_grade_factor: est_rm,
                    estimated_sampler_grade_factor: est_samp,
                    period_contained_au_grams: preds,
                },
            )
            .expect("score card");

        assert_eq!(card.periods_scored, 40);
        assert!(
            card.resource_grade_bias_abs_error < 0.03,
            "resource bias error too large: {}",
            card.resource_grade_bias_abs_error
        );
        assert!(
            card.sampler_grade_bias_abs_error < 0.03,
            "sampler bias error too large: {}",
            card.sampler_grade_bias_abs_error
        );
        assert!(
            card.empirical_coverage_80 >= 0.70,
            "empirical 80% coverage too low: {}",
            card.empirical_coverage_80
        );
        assert!(
            card.relative_rmse < 0.02,
            "RMSE too high: {}",
            card.relative_rmse
        );

        // Verify misspecified variant generates cleanly with distinct shifted observations.
        let mis_cfg = sample_config(WorldCondition::Misspecified(MisspecKind::OutOfModelFault));
        let (obs_mis, _) = harness
            .generate_open_pit_au(4242, &mis_cfg)
            .expect("gen mis");
        assert!(
            obs_mis.periods[35].refinery_recovered_au_grams
                < obs.periods[35].refinery_recovered_au_grams
        );
    }
}
