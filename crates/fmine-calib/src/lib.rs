//! Frozen forecast ledger (`INV-008`), proper scoring rules (`CRPS`, log, Winkler),
//! `CalibrationFlag` auto-downgrade (`NL-8`), `REFCLASS-` $\varepsilon$-contamination (`GB-32`),
//! and Vecchia/Rosenblatt spatial PIT e-process monitoring (`GB-08`, `BD-311`).

use std::collections::BTreeMap;

use fmine_claim::Claim;
use fmine_core::id::{ObjectId, PrefixKind};
use fmine_evidence::{EvidenceClass, Refusal};
use fmine_numerics::cholesky_solve;
use fmine_numerics::eproc::BettingEProcess;
use fmine_numerics::norm_cdf;

/// Immutable forecast record registered before outcome realization (`BD-047`, `INV-008`).
#[derive(Debug, Clone, PartialEq)]
pub struct FrozenForecast {
    pub forecast_id: ObjectId,
    pub model_key: String,
    pub registered_ns: i64,
    pub freeze_time_ns: i64,
    pub mean: f64,
    pub std_dev: f64,
    pub p10: f64,
    pub p90: f64,
    pub realized_outcome: Option<f64>,
}

/// Append-only ledger of frozen forecasts (`BD-047`, `INV-008`).
#[derive(Debug, Default, Clone)]
pub struct ForecastLedger {
    records: BTreeMap<ObjectId, FrozenForecast>,
}

impl ForecastLedger {
    /// Creates an empty `ForecastLedger`.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a new forecast prior to `freeze_time_ns` (`FMINE-REF-403` if `registered_ns > freeze_time_ns`).
    pub fn register(
        &mut self,
        model_key: impl Into<String>,
        registered_ns: i64,
        freeze_time_ns: i64,
        mean: f64,
        std_dev: f64,
    ) -> Result<ObjectId, Refusal> {
        if registered_ns > freeze_time_ns {
            return Err(Refusal::from_code(
                "FMINE-REF-403",
                format!("forecast registered at {registered_ns} after freeze_time {freeze_time_ns}"),
            ));
        }
        // A forecast that cannot be scored is refused at entry, so `evaluate_model` never meets one. The code is
        // the ledger's only registered refusal until BD-328 adds an invalid-forecast code.
        if !mean.is_finite() || !(std_dev.is_finite() && std_dev > 0.0) {
            return Err(Refusal::from_code(
                "FMINE-REF-403",
                format!("forecast needs a finite mean and a positive finite std_dev, got mean {mean}, std_dev {std_dev}"),
            ));
        }
        let model_key = model_key.into();
        let z80 = 1.281_551_565_544_600_4;
        let p10 = mean - z80 * std_dev;
        let p90 = mean + z80 * std_dev;
        let payload = format!("{model_key}:{registered_ns}:{freeze_time_ns}:{mean:.8}:{std_dev:.8}");
        let forecast_id = ObjectId::from_canonical_bytes(PrefixKind::Forecast, payload.as_bytes());
        if self.records.contains_key(&forecast_id) {
            return Err(Refusal::from_code(
                "FMINE-REF-403",
                format!("duplicate or mutated frozen forecast {forecast_id}"),
            ));
        }
        self.records.insert(
            forecast_id,
            FrozenForecast {
                forecast_id,
                model_key,
                registered_ns,
                freeze_time_ns,
                mean,
                std_dev,
                p10,
                p90,
                realized_outcome: None,
            },
        );
        Ok(forecast_id)
    }

    /// Attaches the realized outcome once observed after `freeze_time_ns`.
    /// Refuses with `FMINE-REF-403` if an outcome was already recorded or `outcome_ns <= freeze_time_ns`.
    pub fn record_outcome(
        &mut self,
        forecast_id: ObjectId,
        outcome_ns: i64,
        realized: f64,
    ) -> Result<(), Refusal> {
        let rec = self.records.get_mut(&forecast_id).ok_or_else(|| {
            Refusal::from_code("FMINE-REF-403", format!("unknown forecast {forecast_id}"))
        })?;
        if rec.realized_outcome.is_some() || outcome_ns <= rec.freeze_time_ns {
            return Err(Refusal::from_code(
                "FMINE-REF-403",
                format!(
                    "cannot overwrite outcome or score at {outcome_ns} <= freeze_time {}",
                    rec.freeze_time_ns
                ),
            ));
        }
        if !realized.is_finite() {
            return Err(Refusal::from_code(
                "FMINE-REF-403",
                format!("outcome for {forecast_id} must be finite, got {realized}"),
            ));
        }
        rec.realized_outcome = Some(realized);
        Ok(())
    }

    /// Evaluates calibration across all realized forecasts for `model_key` (`BD-048`, `BD-049`).
    ///
    /// A realized record whose score cannot be computed refuses the whole evaluation; it is never skipped, so the
    /// denominator cannot shrink silently. `register` and `record_outcome` refuse such records at entry.
    pub fn evaluate_model(&self, model_key: &str) -> Result<CalibrationReport, ScoreError> {
        let mut n = 0_usize;
        let mut crps_sum = 0.0_f64;
        let mut winkler_sum = 0.0_f64;
        let mut covered_80 = 0_usize;
        let mut z_sum = 0.0_f64;

        for rec in self.records.values() {
            if rec.model_key != model_key {
                continue;
            }
            if let Some(y) = rec.realized_outcome {
                n += 1;
                crps_sum += gaussian_crps(rec.mean, rec.std_dev, y)?;
                winkler_sum += winkler_interval_score(rec.p10, rec.p90, 0.20, y)?;
                if y >= rec.p10 && y <= rec.p90 {
                    covered_80 += 1;
                }
                z_sum += (y - rec.mean) / rec.std_dev;
            }
        }
        if n == 0 {
            return Ok(CalibrationReport {
                n_scored: 0,
                mean_crps: 0.0,
                mean_winkler_80: 0.0,
                empirical_coverage_80: 0.0,
                mean_standardized_bias: 0.0,
                flag: CalibrationFlag::Unscored,
            });
        }
        let cov80 = (covered_80 as f64) / (n as f64);
        let bias = z_sum / (n as f64);
        let flag = if bias.abs() > 0.5 {
            CalibrationFlag::Biased
        } else if cov80 < 0.65 {
            CalibrationFlag::Overconfident
        } else {
            CalibrationFlag::Calibrated
        };
        Ok(CalibrationReport {
            n_scored: n,
            mean_crps: crps_sum / (n as f64),
            mean_winkler_80: winkler_sum / (n as f64),
            empirical_coverage_80: cov80,
            mean_standardized_bias: bias,
            flag,
        })
    }
}

/// Calibration health classification (`BD-049`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CalibrationFlag {
    Calibrated,
    Overconfident,
    Biased,
    Unscored,
}

/// Summary report produced by `ForecastLedger::evaluate_model` (`BD-048`, `BD-049`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CalibrationReport {
    pub n_scored: usize,
    pub mean_crps: f64,
    pub mean_winkler_80: f64,
    pub empirical_coverage_80: f64,
    pub mean_standardized_bias: f64,
    pub flag: CalibrationFlag,
}

/// Applies `CalibrationFlag` to a `Claim<T>`, enforcing `NL-8`: if `flag` is `Overconfident`
/// or `Biased`, marks `calibration_failed = true` and downgrades `Calibrated -> Assumed` (`BD-049`).
pub fn apply_calibration_gate<T>(claim: &mut Claim<T>, flag: CalibrationFlag) {
    match flag {
        CalibrationFlag::Calibrated => {}
        CalibrationFlag::Unscored => {
            if claim.evidence == EvidenceClass::Calibrated {
                claim.evidence = EvidenceClass::Assumed;
            }
        }
        CalibrationFlag::Overconfident | CalibrationFlag::Biased => {
            claim.calibration_failed = true;
            if claim.evidence == EvidenceClass::Calibrated {
                claim.evidence = EvidenceClass::Assumed;
            }
        }
    }
}

// --- Proper Scoring Rules (BD-048) ---

/// Closed-form Gneiting & Raftery (2007) Continuous Ranked Probability Score (`CRPS`)
/// for $\mathcal{N}(\mu, \sigma^2)$ evaluated at realization $y$:
/// $\text{CRPS}(\mathcal{N}(\mu, \sigma^2), y) = \sigma \left[ z (2 \Phi(z) - 1) + 2 \phi(z) - \frac{1}{\sqrt{\pi}} \right]$.
pub fn gaussian_crps(mean: f64, std_dev: f64, y: f64) -> Result<f64, ScoreError> {
    check_gaussian(mean, std_dev, y)?;
    let z = (y - mean) / std_dev;
    let phi_cdf = norm_cdf(z);
    let phi_pdf = (-0.5 * z * z).exp() / (2.0 * std::f64::consts::PI).sqrt();
    let inv_sqrt_pi = 1.0 / std::f64::consts::PI.sqrt();
    Ok(std_dev * (z * (2.0 * phi_cdf - 1.0) + 2.0 * phi_pdf - inv_sqrt_pi))
}

/// Negative log predictive density (`NLL`) for $\mathcal{N}(\mu, \sigma^2)$ at $y$.
pub fn gaussian_log_score(mean: f64, std_dev: f64, y: f64) -> Result<f64, ScoreError> {
    check_gaussian(mean, std_dev, y)?;
    let z = (y - mean) / std_dev;
    Ok(0.5 * (2.0 * std::f64::consts::PI).ln() + std_dev.ln() + 0.5 * z * z)
}

/// Winkler interval score for the central $(1 - \alpha)$ interval $[l, u]$ at realization $y$.
///
/// Refuses `alpha` outside `(0, 1)`, an inverted interval and non-finite inputs; nothing is clamped.
pub fn winkler_interval_score(lo: f64, hi: f64, alpha: f64, y: f64) -> Result<f64, ScoreError> {
    if !(lo.is_finite() && hi.is_finite() && alpha.is_finite() && y.is_finite()) {
        return Err(ScoreError::NonFinite);
    }
    if !(alpha > 0.0 && alpha < 1.0) {
        return Err(ScoreError::AlphaOutOfDomain(alpha));
    }
    if lo > hi {
        return Err(ScoreError::InvertedInterval { lo, hi });
    }
    let width = hi - lo;
    Ok(if y < lo {
        width + (2.0 / alpha) * (lo - y)
    } else if y > hi {
        width + (2.0 / alpha) * (y - hi)
    } else {
        width
    })
}

/// Why a proper score refused (`bd-8wrk`): out-of-domain inputs are never floored or clamped (AGENTS rule 5).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ScoreError {
    /// A mean, bound, level or realization is NaN or infinite.
    NonFinite,
    /// The predictive standard deviation is not positive.
    NonPositiveStdDev(f64),
    /// The interval level `alpha` is outside `(0, 1)`.
    AlphaOutOfDomain(f64),
    /// The interval's lower bound exceeds its upper bound.
    InvertedInterval { lo: f64, hi: f64 },
}

impl std::fmt::Display for ScoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NonFinite => write!(f, "score inputs must be finite"),
            Self::NonPositiveStdDev(s) => write!(f, "predictive std_dev must be positive, got {s}"),
            Self::AlphaOutOfDomain(a) => write!(f, "interval level alpha must lie in (0, 1), got {a}"),
            Self::InvertedInterval { lo, hi } => write!(f, "interval is inverted: lo {lo} > hi {hi}"),
        }
    }
}

impl std::error::Error for ScoreError {}

fn check_gaussian(mean: f64, std_dev: f64, y: f64) -> Result<(), ScoreError> {
    if !(mean.is_finite() && std_dev.is_finite() && y.is_finite()) {
        return Err(ScoreError::NonFinite);
    }
    if std_dev <= 0.0 {
        return Err(ScoreError::NonPositiveStdDev(std_dev));
    }
    Ok(())
}

// --- Reference-Class Epsilon-Contamination Prior (BD-050, GB-32) ---

/// Huber $\varepsilon$-contaminated reference-class prior blending an internal engineering estimate
/// $\mathcal{N}(\mu_{\text{int}}, \sigma_{\text{int}}^2)$ with an industry outside-view reference class
/// $\mathcal{N}(\mu_{\text{ref}}, \sigma_{\text{ref}}^2)$ (`GB-32`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ReferenceClassPrior {
    pub class_id: &'static str,
    pub ref_mean_multiplier: f64,
    pub ref_std_multiplier: f64,
    pub epsilon: f64,
}

impl ReferenceClassPrior {
    /// Outside-view greenfield porphyry Cu-Au project capex overrun prior (`REFCLASS-CAPEX-GREENFIELD`).
    pub const GREENFIELD_PORPHYRY_CAPEX: Self = Self {
        class_id: "REFCLASS-CAPEX-GREENFIELD",
        ref_mean_multiplier: 1.28,
        ref_std_multiplier: 0.35,
        epsilon: 0.25,
    };

    /// Computes the moment-matched blended mean and standard deviation for an internal baseline `base`.
    #[must_use]
    pub fn blend(&self, base_mean: f64, base_std: f64) -> (f64, f64) {
        let eps = self.epsilon.clamp(0.0, 1.0);
        let ref_mean = base_mean * self.ref_mean_multiplier;
        let ref_std = base_mean.abs() * self.ref_std_multiplier;

        let blended_mean = (1.0 - eps) * base_mean + eps * ref_mean;
        let m2_int = base_std * base_std + base_mean * base_mean;
        let m2_ref = ref_std * ref_std + ref_mean * ref_mean;
        let blended_var = ((1.0 - eps) * m2_int + eps * m2_ref - blended_mean * blended_mean).max(0.0);
        (blended_mean, blended_var.sqrt())
    }
}

// --- Vecchia / Rosenblatt Spatial Front PIT Monitor (BD-051, BD-311, GB-08) ---

/// 3D spatial sample observed at an advancing mining face or blast-hole pattern (`BD-311`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpatialSample {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub prior_mean: f64,
    pub observed_value: f64,
}

/// Vecchia-conditioned Rosenblatt PIT sequential monitor (`Monitor::SpatialFrontPit`, `GB-08`, `BD-311`).
///
/// Decorrelates each new spatial observation $y_i$ against up to `max_neighbors` nearest prior
/// observations under an isotropic exponential covariance $C(h) = \sigma_0^2 \mathbb{I}(h=0) + \sigma_1^2 \exp(-h / r)$,
/// maps the standardized conditional residual $z_i^* = (y_i - \mu_{i|\mathcal{N}(i)}) / \sigma_{i|\mathcal{N}(i)}$
/// to $u_i = \Phi(z_i^*) \in (0, 1)$, and updates a pair of anytime-valid `BettingEProcess` monitors
/// (one for mean bias, one for variance/spread error).
#[derive(Debug, Clone)]
pub struct SpatialFrontPitMonitor {
    pub nugget_var: f64,
    pub sill_var: f64,
    pub range_m: f64,
    pub max_neighbors: usize,
    pub history: Vec<SpatialSample>,
    pub mean_eproc: BettingEProcess,
    pub spread_eproc: BettingEProcess,
}

impl SpatialFrontPitMonitor {
    /// Creates a new `SpatialFrontPitMonitor` with exponential variogram parameters and significance `alpha`.
    #[must_use]
    pub fn new(nugget_var: f64, sill_var: f64, range_m: f64, max_neighbors: usize, alpha: f64) -> Self {
        Self {
            nugget_var: nugget_var.max(1e-6),
            sill_var: sill_var.max(1e-6),
            range_m: range_m.max(1.0),
            max_neighbors: max_neighbors.clamp(1, 16),
            history: Vec::new(),
            mean_eproc: BettingEProcess::new(0.5, alpha * 0.5),
            spread_eproc: BettingEProcess::new(0.25, alpha * 0.5),
        }
    }

    fn covariance(&self, a: &SpatialSample, b: &SpatialSample) -> f64 {
        let dx = a.x - b.x;
        let dy = a.y - b.y;
        let dz = a.z - b.z;
        let h = (dx * dx + dy * dy + dz * dz).sqrt();
        if h < 1e-9 {
            self.nugget_var + self.sill_var
        } else {
            self.sill_var * (-h / self.range_m).exp()
        }
    }

    /// Conditions `sample` on its nearest Vecchia predecessors, computes the whitened Rosenblatt PIT $u_i$,
    /// updates the e-processes, and appends `sample` to the conditioning history (`BD-311`).
    pub fn observe_sample(&mut self, sample: SpatialSample) -> Result<(f64, bool), Refusal> {
        let total_var = self.nugget_var + self.sill_var;
        let (cond_mean, cond_std) = if self.history.is_empty() {
            (sample.prior_mean, total_var.sqrt())
        } else {
            let mut indexed_dists: Vec<(f64, usize)> = self
                .history
                .iter()
                .enumerate()
                .map(|(idx, prev)| {
                    let dx = sample.x - prev.x;
                    let dy = sample.y - prev.y;
                    let dz = sample.z - prev.z;
                    (dx * dx + dy * dy + dz * dz, idx)
                })
                .collect();
            indexed_dists.sort_by(|a, b| a.0.total_cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
            let k = indexed_dists.len().min(self.max_neighbors);
            let neighbors: Vec<SpatialSample> = indexed_dists[..k]
                .iter()
                .map(|&(_, idx)| self.history[idx])
                .collect();

            let mut k_mat = vec![0.0_f64; k * k];
            let mut k_vec = vec![0.0_f64; k];
            for i in 0..k {
                k_vec[i] = self.covariance(&sample, &neighbors[i]);
                for j in 0..k {
                    k_mat[i * k + j] = if i == j {
                        total_var
                    } else {
                        self.covariance(&neighbors[i], &neighbors[j])
                    };
                }
            }

            let weights = cholesky_solve(&k_mat, &k_vec, k)?;
            let mut mu = sample.prior_mean;
            let mut var_red = 0.0_f64;
            for i in 0..k {
                mu += weights[i] * (neighbors[i].observed_value - neighbors[i].prior_mean);
                var_red += weights[i] * k_vec[i];
            }
            let c_var = (total_var - var_red).max(self.nugget_var);
            (mu, c_var.sqrt())
        };

        let z_star = (sample.observed_value - cond_mean) / cond_std;
        let u_pit = norm_cdf(z_star);
        self.mean_eproc.observe(u_pit);
        //Under Uniform(0,1), E[(u - 0.5)^2] = 1/12, and E[|u - 0.5|] = 0.25.
        self.spread_eproc.observe((u_pit - 0.5).abs());
        self.history.push(sample);

        let alarm = self.mean_eproc.rejected_null() || self.spread_eproc.rejected_null();
        Ok((u_pit, alarm))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fmine_claim::{AdditivityClass, ConfidentialityTier, Representation, SpatialSupport};
    use fmine_core::anchor::SourceAnchor;
    use fmine_core::rand::{PhiloxStream, RngKey};
    use fmine_evidence::KnowledgeState;

    #[test]
    fn test_frozen_forecast_ledger_immutability() {
        let mut ledger = ForecastLedger::new();
        let fid = ledger
            .register("grade_control_carlin", 1_000, 2_000, 3.5, 0.4)
            .expect("valid pre-freeze registration");

        // Registering after freeze_time must fail with FMINE-REF-403.
        let err_late = ledger
            .register("grade_control_carlin", 2_500, 2_000, 3.5, 0.4)
            .expect_err("must reject post-freeze registration");
        assert_eq!(err_late.code, "FMINE-REF-403");

        // Recording outcome after freeze_time succeeds once, then refuses mutation.
        ledger.record_outcome(fid, 3_000, 3.6).expect("first outcome");
        let err_mut = ledger
            .record_outcome(fid, 3_500, 4.0)
            .expect_err("must reject outcome mutation");
        assert_eq!(err_mut.code, "FMINE-REF-403");
    }

    #[test]
    fn test_proper_scoring_rules_crps_and_winkler() {
        // Perfect mean has lower CRPS than biased mean.
        let c_good = gaussian_crps(2.0, 0.5, 2.0).expect("valid");
        let c_bad = gaussian_crps(4.0, 0.5, 2.0).expect("valid");
        assert!(c_good < c_bad);

        // Winkler score penalizes observations outside [lo, hi]: width 2 plus (2 / 0.2) * 1 = 12.
        let w_in = winkler_interval_score(1.0, 3.0, 0.20, 2.0).expect("valid");
        let w_out = winkler_interval_score(1.0, 3.0, 0.20, 4.0).expect("valid");
        assert!((w_in - 2.0).abs() < 1e-12);
        assert!((w_out - 12.0).abs() < 1e-12);
    }

    /// Regression for the 2026-10-29 finding (bd-8wrk): alpha used to be clamped to [1e-6, 1 − 1e-6], an inverted
    /// interval became width 0, and a non-positive std_dev was floored to 1e-12. Each now refuses.
    #[test]
    fn test_scores_refuse_out_of_domain_inputs() {
        assert_eq!(winkler_interval_score(1.0, 3.0, 0.0, 2.0), Err(ScoreError::AlphaOutOfDomain(0.0)));
        assert_eq!(winkler_interval_score(1.0, 3.0, 1.0, 2.0), Err(ScoreError::AlphaOutOfDomain(1.0)));
        assert_eq!(
            winkler_interval_score(3.0, 1.0, 0.2, 2.0),
            Err(ScoreError::InvertedInterval { lo: 3.0, hi: 1.0 })
        );
        assert_eq!(winkler_interval_score(1.0, 3.0, 0.2, f64::NAN), Err(ScoreError::NonFinite));
        assert_eq!(gaussian_crps(2.0, 0.0, 2.0), Err(ScoreError::NonPositiveStdDev(0.0)));
        assert_eq!(gaussian_log_score(2.0, -1.0, 2.0), Err(ScoreError::NonPositiveStdDev(-1.0)));
        assert_eq!(gaussian_log_score(f64::INFINITY, 1.0, 2.0), Err(ScoreError::NonFinite));
        // A degenerate interval (lo == hi) is valid and scores its miss penalty only.
        assert_eq!(winkler_interval_score(2.0, 2.0, 0.5, 3.0), Ok(4.0));
    }

    /// `evaluate_model` had no test. Known answer over three resolved forecasts, plus the entry refusals that keep
    /// unscorable records out of the ledger (so no record is ever skipped during evaluation).
    #[test]
    fn test_evaluate_model_known_answer_and_entry_refusals() {
        let mut ledger = ForecastLedger::new();
        let mut ids = Vec::new();
        for (i, mean) in [10.0, 20.0, 30.0].into_iter().enumerate() {
            let t = 1_000 * (i as i64 + 1);
            ids.push(ledger.register("m", t, t + 100, mean, 1.0).expect("register"));
        }
        // Outcomes: exactly on the mean, inside the 80% band, far outside it.
        for (fid, y) in ids.iter().zip([10.0, 20.5, 35.0]) {
            ledger.record_outcome(*fid, 10_000, y).expect("outcome");
        }
        let report = ledger.evaluate_model("m").expect("scorable");
        assert_eq!(report.n_scored, 3);
        assert!((report.empirical_coverage_80 - 2.0 / 3.0).abs() < 1e-12);
        assert!((report.mean_standardized_bias - 5.5 / 3.0).abs() < 1e-12);
        assert_eq!(report.flag, CalibrationFlag::Biased);
        let z80 = 1.281_551_565_544_600_4_f64;
        let width = 2.0 * z80;
        let expected_winkler = (3.0 * width + (2.0 / 0.2) * (35.0 - (30.0 + z80))) / 3.0;
        assert!((report.mean_winkler_80 - expected_winkler).abs() < 1e-9);
        assert_eq!(ledger.evaluate_model("absent").expect("empty").flag, CalibrationFlag::Unscored);

        assert!(ledger.register("m", 1, 2, 1.0, 0.0).is_err(), "zero std_dev refused");
        assert!(ledger.register("m", 1, 2, 1.0, f64::NAN).is_err(), "NaN std_dev refused");
        assert!(ledger.register("m", 1, 2, f64::NAN, 1.0).is_err(), "NaN mean refused");
        let fid = ledger.register("n", 1, 2, 1.0, 1.0).expect("register");
        assert!(ledger.record_outcome(fid, 3, f64::NAN).is_err(), "NaN outcome refused");
        assert!(ledger.record_outcome(fid, 3, 1.0).is_ok(), "the refused write left the record open");
    }

    #[test]
    fn test_reference_class_epsilon_contamination_gb32() {
        // Reko Diq Phase 1 internal estimate $5.6B blended with greenfield reference class.
        let (mean, std) = ReferenceClassPrior::GREENFIELD_PORPHYRY_CAPEX.blend(5_600.0, 400.0);
        assert!(mean > 5_600.0, "reference class must lift expected capex");
        assert!(std > 400.0, "reference class must widen capex dispersion");

        // Verify NL-8 auto-downgrade on biased calibration flag.
        let mut claim = Claim::new(
            "reko_diq_capex",
            mean,
            "5992.0",
            Representation::Gaussian { mean, std_dev: std },
            SpatialSupport::PeriodAggregate,
            AdditivityClass::ExtensiveAdditive,
            EvidenceClass::Calibrated,
            KnowledgeState::KnownUnknown,
            ConfidentialityTier::Public,
            vec![],
            vec![],
            vec![SourceAnchor::new("esia.pdf", [0; 32], "p.1")],
        );
        apply_calibration_gate(&mut claim, CalibrationFlag::Biased);
        assert!(claim.calibration_failed);
        assert_eq!(claim.evidence, EvidenceClass::Assumed);
    }

    #[test]
    fn test_vecchia_spatial_front_pit_eprocess_gb08() {
        // 1. Under well-specified spatial innovations, monitor does not trip false alarm.
        let mut monitor_ok = SpatialFrontPitMonitor::new(0.25, 0.75, 30.0, 6, 0.02);
        let mut rng = PhiloxStream::new(RngKey {
            seed_root: 777,
            partition: 1,
            domain: 1,
            var_group: 1,
            realization: 0,
            brick: 0,
        });
        for i in 0..80 {
            let x = (i as f64) * 50.0; // Wide spacing so marginal innovation ~ N(0, 1)
            let y = rng.next_normal();
            let (_, alarm) = monitor_ok
                .observe_sample(SpatialSample {
                    x,
                    y: 0.0,
                    z: 0.0,
                    prior_mean: 0.0,
                    observed_value: y,
                })
                .unwrap();
            assert!(!alarm, "unexpected false alarm under null at step {i}");
        }

        // 2. Under systematic +1.5g/t unmodeled high-grade shear zone, monitor trips alarm.
        let mut monitor_shift = SpatialFrontPitMonitor::new(0.25, 0.75, 30.0, 6, 0.05);
        let mut tripped = false;
        for i in 0..60 {
            let x = (i as f64) * 10.0;
            let (_, alarm) = monitor_shift
                .observe_sample(SpatialSample {
                    x,
                    y: 0.0,
                    z: 0.0,
                    prior_mean: 1.0,
                    observed_value: 2.8,
                })
                .unwrap();
            if alarm {
                tripped = true;
                break;
            }
        }
        assert!(tripped, "Vecchia spatial PIT e-process must detect shifted bench grade");
    }
}
