//! Determinism: BitwisePortable
//! Synthetic mine world generators (`BD-127`, plan §33.3):
//! - Control (`CorrectlySpecified`), misspecified (`Misspecified(MisspecKind)`), and
//!   confounded-bias (`IdentifiabilityProbe`) world conditions;
//! - Deterministic `Philox4x32-10` Open-Pit Au world generating observable resource-model,
//!   truck-scale, moisture, and plant measurements while sealing true tonnes, true grades, and
//!   injected multiplicative biases inside [`crate::seal::SealedTruth`].

use fmine_core::codec::Encoder;
use fmine_core::det::{exp, norm_ppf};
use fmine_core::rand::{PhiloxStream, RngKey};

use crate::GenError;

/// Misspecification families from plan §33.3.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MisspecKind {
    WrongVariogram,
    WrongDomainBoundary,
    OutOfModelFault,
    NonStationaryGrade,
    BiasedCapexProcess,
}

/// Experimental condition controlling control vs. misspecification vs. identifiability testing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WorldCondition {
    /// Model family matches data-generating process (tests false-alarm rate <= alpha).
    CorrectlySpecified,
    /// Structural departure from the assumed model (tests alarm + coverage restoration).
    Misspecified(MisspecKind),
    /// Two structurally confounded biases whose individual split is non-identifiable (`FMINE-REF-104`).
    IdentifiabilityProbe,
}

/// Injected multiplicative systematic bias vector for an open-pit Au mine (plan §33.3).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BiasVector {
    /// Multiplicative bias of resource model grade vs. true in-situ grade (e.g. `1.08` = +8% overestimate).
    pub resource_grade_factor: f64,
    /// Multiplicative dilution factor on mined tonnes (e.g. `1.05` = +5% waste dilution).
    pub dilution_tonnes_factor: f64,
    /// Multiplicative bias of wet-to-dry moisture correction (e.g. `0.98`).
    pub moisture_factor: f64,
    /// Multiplicative bias of plant head sampler vs. true mill feed grade (e.g. `0.97`).
    pub sampler_grade_factor: f64,
}

impl BiasVector {
    /// Unbiased control vector (all factors = `1.0`).
    #[must_use]
    pub const fn unbiased() -> Self {
        Self {
            resource_grade_factor: 1.0,
            dilution_tonnes_factor: 1.0,
            moisture_factor: 1.0,
            sampler_grade_factor: 1.0,
        }
    }

    fn validate(&self) -> Result<(), GenError> {
        for (field, v) in [
            ("resource_grade_factor", self.resource_grade_factor),
            ("dilution_tonnes_factor", self.dilution_tonnes_factor),
            ("moisture_factor", self.moisture_factor),
            ("sampler_grade_factor", self.sampler_grade_factor),
        ] {
            if !(v.is_finite() && v > 0.2 && v < 5.0) {
                return Err(GenError::InvalidConfig {
                    field,
                    detail: format!("bias factor {v} outside (0.2, 5.0)"),
                });
            }
        }
        Ok(())
    }
}

/// Configuration for generating a synthetic Open-Pit Au mine world.
#[derive(Debug, Clone, PartialEq)]
pub struct OpenPitAuConfig {
    pub world_id: String,
    pub periods: usize,
    pub base_dry_tonnes_per_period: f64,
    pub base_true_grade_g_per_t: f64,
    pub measurement_rel_sigma: f64,
    pub biases: BiasVector,
    pub condition: WorldCondition,
}

impl OpenPitAuConfig {
    fn validate(&self) -> Result<(), GenError> {
        if self.world_id.is_empty() {
            return Err(GenError::InvalidConfig {
                field: "world_id",
                detail: "must be non-empty".to_string(),
            });
        }
        if !(1..=240).contains(&self.periods) {
            return Err(GenError::InvalidConfig {
                field: "periods",
                detail: format!("{} outside 1..=240", self.periods),
            });
        }
        if !(self.base_dry_tonnes_per_period.is_finite() && self.base_dry_tonnes_per_period > 0.0) {
            return Err(GenError::InvalidConfig {
                field: "base_dry_tonnes_per_period",
                detail: "must be finite and > 0".to_string(),
            });
        }
        if !(self.base_true_grade_g_per_t.is_finite() && self.base_true_grade_g_per_t > 0.0) {
            return Err(GenError::InvalidConfig {
                field: "base_true_grade_g_per_t",
                detail: "must be finite and > 0".to_string(),
            });
        }
        if !(self.measurement_rel_sigma.is_finite()
            && self.measurement_rel_sigma > 0.0
            && self.measurement_rel_sigma < 0.5)
        {
            return Err(GenError::InvalidConfig {
                field: "measurement_rel_sigma",
                detail: "must lie in (0, 0.5)".to_string(),
            });
        }
        self.biases.validate()
    }
}

/// Observable per-period site records exposed to an engine under test (NO ground truth).
#[derive(Debug, Clone, PartialEq)]
pub struct PeriodObservation {
    pub period_index: usize,
    pub resource_model_tonnes: f64,
    pub resource_model_grade_g_per_t: f64,
    pub truck_scale_dry_tonnes: f64,
    pub plant_sampler_grade_g_per_t: f64,
    pub refinery_recovered_au_grams: f64,
}

/// Complete observable package handed to an engine under test.
#[derive(Debug, Clone, PartialEq)]
pub struct OpenPitAuObservations {
    pub world_id: String,
    pub seed_root: u64,
    pub condition: WorldCondition,
    pub nominal_recovery: f64,
    pub periods: Vec<PeriodObservation>,
}

/// Sealed true per-period physical state (hidden from engines; opened only by `ScoringHarness`).
#[derive(Debug, Clone, PartialEq)]
pub struct PeriodTruth {
    pub period_index: usize,
    pub true_dry_tonnes: f64,
    pub true_grade_g_per_t: f64,
    pub true_contained_au_grams: f64,
    pub true_recovery: f64,
}

/// Complete sealed ground truth of an Open-Pit Au synthetic world.
#[derive(Debug, Clone, PartialEq)]
pub struct OpenPitAuTruth {
    pub world_id: String,
    pub seed_root: u64,
    pub condition: WorldCondition,
    pub biases: BiasVector,
    pub periods: Vec<PeriodTruth>,
}

impl OpenPitAuTruth {
    pub(crate) fn canonical_bytes(&self) -> Vec<u8> {
        let mut enc = Encoder::new_frame(*b"TRTH", 1);
        enc.put_str(&self.world_id);
        enc.put_u64(self.seed_root);
        enc.put_u64(self.biases.resource_grade_factor.to_bits());
        enc.put_u64(self.biases.dilution_tonnes_factor.to_bits());
        enc.put_u64(self.biases.moisture_factor.to_bits());
        enc.put_u64(self.biases.sampler_grade_factor.to_bits());
        enc.put_u32(self.periods.len() as u32);
        for p in &self.periods {
            enc.put_u32(p.period_index as u32);
            enc.put_u64(p.true_dry_tonnes.to_bits());
            enc.put_u64(p.true_grade_g_per_t.to_bits());
            enc.put_u64(p.true_contained_au_grams.to_bits());
            enc.put_u64(p.true_recovery.to_bits());
        }
        enc.finish()
    }
}

fn draw_standard_normal(stream: &mut PhiloxStream) -> f64 {
    // Map 53-bit uniform draw into open interval (1e-12, 1 - 1e-12) for portable norm_ppf.
    let u_raw = ((stream.next_u64() >> 11) as f64) * (1.0 / ((1u64 << 53) as f64));
    let u = 1e-12 + u_raw * (1.0 - 2e-12);
    norm_ppf(u)
}

pub(crate) fn generate_open_pit_au_raw(
    seed_root: u64,
    cfg: &OpenPitAuConfig,
) -> Result<(OpenPitAuObservations, OpenPitAuTruth), GenError> {
    cfg.validate()?;
    let mut stream = PhiloxStream::new(RngKey {
        seed_root,
        partition: 0x4745_4E31, // "GEN1"
        domain: 1,
        var_group: 1,
        realization: 0,
        brick: 0,
    });

    let nominal_recovery = 0.90;
    let mut obs_periods = Vec::with_capacity(cfg.periods);
    let mut truth_periods = Vec::with_capacity(cfg.periods);

    for t in 0..cfg.periods {
        let z_tonnes = draw_standard_normal(&mut stream);
        let z_grade = draw_standard_normal(&mut stream);

        // Optional structural shift under misspecification regimes (plan §33.3).
        let drift = match cfg.condition {
            WorldCondition::Misspecified(MisspecKind::NonStationaryGrade) => {
                1.0 - 0.15 * ((t as f64) / (cfg.periods.max(1) as f64))
            }
            WorldCondition::Misspecified(MisspecKind::OutOfModelFault) if t >= cfg.periods / 2 => {
                0.78
            }
            _ => 1.0,
        };

        let true_dry_tonnes = cfg.base_dry_tonnes_per_period * exp(0.06 * z_tonnes);
        let true_grade_g_per_t = cfg.base_true_grade_g_per_t * drift * exp(0.12 * z_grade);
        let true_contained_au_grams = true_dry_tonnes * true_grade_g_per_t;
        let true_recovery = nominal_recovery;

        let n_rm_t = exp(cfg.measurement_rel_sigma * draw_standard_normal(&mut stream));
        let n_rm_g = exp(cfg.measurement_rel_sigma * draw_standard_normal(&mut stream));
        let n_truck = exp(cfg.measurement_rel_sigma * draw_standard_normal(&mut stream));
        let n_samp = exp(cfg.measurement_rel_sigma * draw_standard_normal(&mut stream));
        let n_ref = exp(0.25 * cfg.measurement_rel_sigma * draw_standard_normal(&mut stream));

        // Resource model predicts pre-dilution tonnes and biased grade; truck scale sees diluted + moisture-biased tonnes.
        let resource_model_tonnes = (true_dry_tonnes / cfg.biases.dilution_tonnes_factor) * n_rm_t;
        let resource_model_grade_g_per_t =
            true_grade_g_per_t * cfg.biases.resource_grade_factor * n_rm_g;
        let truck_scale_dry_tonnes = true_dry_tonnes * cfg.biases.moisture_factor * n_truck;
        let plant_sampler_grade_g_per_t =
            true_grade_g_per_t * cfg.biases.sampler_grade_factor * n_samp;
        let refinery_recovered_au_grams = true_contained_au_grams * true_recovery * n_ref;

        obs_periods.push(PeriodObservation {
            period_index: t,
            resource_model_tonnes,
            resource_model_grade_g_per_t,
            truck_scale_dry_tonnes,
            plant_sampler_grade_g_per_t,
            refinery_recovered_au_grams,
        });
        truth_periods.push(PeriodTruth {
            period_index: t,
            true_dry_tonnes,
            true_grade_g_per_t,
            true_contained_au_grams,
            true_recovery,
        });
    }

    let obs = OpenPitAuObservations {
        world_id: cfg.world_id.clone(),
        seed_root,
        condition: cfg.condition,
        nominal_recovery,
        periods: obs_periods,
    };
    let truth = OpenPitAuTruth {
        world_id: cfg.world_id.clone(),
        seed_root,
        condition: cfg.condition,
        biases: cfg.biases,
        periods: truth_periods,
    };
    Ok((obs, truth))
}
