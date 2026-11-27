//! Sequential QA/QC monitors: anytime-valid e-process drift detection (`BD-169`, wired to
//! `fmine_numerics::eproc::BettingEProcess`) and pattern + dispersion-collapse (tampering)
//! monitors (`BD-283`, plan §12.4, E367–E368, `GATE-CORE-02`).
//! Determinism: BitwisePortable (IEEE-754 arithmetic and `fmine_core::det` primitives only).

use fmine_drill::records::AssayMethod;
use fmine_numerics::eproc::BettingEProcess;

use crate::crm::SdBasis;
use crate::error::QaqcError;
use crate::stats::{
    binomial_two_sided_half_p, chi_square_lower_cdf, mean_and_sample_sd, normal_cdf,
};

/// Scope of the `InvestigationCase` opened by a QA/QC monitor alarm (plan §12.4, E368).
///
/// Dispersion collapse is an **integrity** investigation (possible data tampering, Sketchley
/// 1998), never a routine recalibration.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AlarmScope {
    /// Mean drift, shift, or systematic analytical bias.
    CalibrationOrBias,
    /// Rapid dispersion collapse indicating potential data tampering (`E368`).
    Integrity,
}

/// Which sequential monitor fired.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MonitorKind {
    /// Anytime-valid PIT betting e-process mean-drift monitor (`BD-169`).
    EProcessDrift,
    /// Two successive points beyond `±2 SD` on the same side (`E367`, Leaver et al. 1997).
    TwoConsecutiveBeyond2Sd,
    /// Four successive points beyond `±1 SD` on the same side (`E367`, Leaver et al. 1997).
    FourConsecutiveBeyond1Sd,
    /// Sign imbalance relative to the certified mean (`E367`).
    SignImbalance,
    /// Rapid fall in running CRM dispersion relative to certified SD (`E368`, Sketchley 1998).
    DispersionCollapse,
}

/// A concrete alarm raised by a sequential QA/QC monitor.
#[derive(Clone, Debug, PartialEq)]
pub struct MonitorAlarm {
    /// Monitor that fired.
    pub monitor: MonitorKind,
    /// Investigation scope (`Integrity` for dispersion collapse, `CalibrationOrBias` otherwise).
    pub scope: AlarmScope,
    /// 1-based observation step at which the alarm fired.
    pub step: usize,
    /// Diagnostic metric value (wealth, z-score, binomial p, or SD ratio).
    pub statistic: f64,
    /// Human-readable reason.
    pub detail: String,
}

/// Stream identity `(crm_id, lab, method, sd_basis)` for per-lab sequential QA/QC monitoring.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CrmStreamKey {
    /// CRM id.
    pub crm_id: String,
    /// Laboratory name.
    pub lab: String,
    /// Assay method (measurand).
    pub method: AssayMethod,
    /// Certified SD basis used for standardization.
    pub sd_basis: SdBasis,
}

/// Configuration for the combined CRM drift e-process and pattern/dispersion monitors.
#[derive(Clone, Debug, PartialEq)]
pub struct CrmMonitorConfig {
    /// Stream identity.
    pub key: CrmStreamKey,
    /// Certified mean `μ` (mass fraction).
    pub certified_mean: f64,
    /// Certified reference standard deviation `σ_ref` (`σ_C` or `σ_L` per `key.sd_basis`).
    pub reference_sd: f64,
    /// Significance level `α` for Ville's anytime-valid e-process threshold `1/α` (default `0.05`).
    pub eproc_alpha: f64,
    /// Minimum non-zero deviations before testing sign imbalance (default `12`).
    pub sign_min_samples: usize,
    /// Two-sided binomial p-value threshold for sign imbalance (default `0.01`).
    pub sign_alpha: f64,
    /// Rolling window size for the dispersion-collapse monitor (default `15`, min `6`).
    pub dispersion_window: usize,
    /// Maximum `running_sd / reference_sd` ratio considered a dispersion collapse (default `0.45`).
    pub collapse_ratio_threshold: f64,
    /// Chi-square lower-tail p-value threshold for dispersion collapse (default `0.001`).
    pub dispersion_alpha: f64,
}

/// Reference certified parameters for constructing a [`CrmMonitorConfig`] without bare `f64` params.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CrmReferenceParams {
    /// Certified mean `μ` (mass fraction).
    pub certified_mean: f64,
    /// Certified reference standard deviation `σ_ref` (`σ_C` or `σ_L`).
    pub reference_sd: f64,
}

impl CrmMonitorConfig {
    /// Builds a validated default monitor configuration for one `(crm_id, lab, method, sd_basis)`.
    #[must_use]
    pub fn with_reference(key: CrmStreamKey, reference: CrmReferenceParams) -> Self {
        Self {
            key,
            certified_mean: reference.certified_mean,
            reference_sd: reference.reference_sd,
            eproc_alpha: 0.05,
            sign_min_samples: 12,
            sign_alpha: 0.01,
            dispersion_window: 15,
            collapse_ratio_threshold: 0.45,
            dispersion_alpha: 0.001,
        }
    }

    /// Validates all configuration parameters.
    pub fn validate(&self) -> Result<(), QaqcError> {
        if self.key.crm_id.trim().is_empty() || self.key.lab.trim().is_empty() {
            return Err(QaqcError::invalid(
                "monitor.key",
                "crm_id and lab must be non-empty",
            ));
        }
        if !self.certified_mean.is_finite() {
            return Err(QaqcError::invalid(
                "monitor.certified_mean",
                "must be finite",
            ));
        }
        if !(self.reference_sd.is_finite() && self.reference_sd > 0.0) {
            return Err(QaqcError::invalid(
                "monitor.reference_sd",
                format!("must be finite and > 0, got {}", self.reference_sd),
            ));
        }
        if !(self.eproc_alpha.is_finite() && self.eproc_alpha > 0.0 && self.eproc_alpha < 1.0) {
            return Err(QaqcError::invalid(
                "monitor.eproc_alpha",
                format!("must lie in (0, 1), got {}", self.eproc_alpha),
            ));
        }
        if self.sign_min_samples < 6 {
            return Err(QaqcError::invalid(
                "monitor.sign_min_samples",
                format!("must be >= 6, got {}", self.sign_min_samples),
            ));
        }
        if !(self.sign_alpha.is_finite() && self.sign_alpha > 0.0 && self.sign_alpha < 1.0) {
            return Err(QaqcError::invalid(
                "monitor.sign_alpha",
                format!("must lie in (0, 1), got {}", self.sign_alpha),
            ));
        }
        if self.dispersion_window < 6 {
            return Err(QaqcError::invalid(
                "monitor.dispersion_window",
                format!("must be >= 6, got {}", self.dispersion_window),
            ));
        }
        if !(self.collapse_ratio_threshold.is_finite()
            && self.collapse_ratio_threshold > 0.0
            && self.collapse_ratio_threshold < 1.0)
        {
            return Err(QaqcError::invalid(
                "monitor.collapse_ratio_threshold",
                format!("must lie in (0, 1), got {}", self.collapse_ratio_threshold),
            ));
        }
        if !(self.dispersion_alpha.is_finite()
            && self.dispersion_alpha > 0.0
            && self.dispersion_alpha < 1.0)
        {
            return Err(QaqcError::invalid(
                "monitor.dispersion_alpha",
                format!("must lie in (0, 1), got {}", self.dispersion_alpha),
            ));
        }
        Ok(())
    }
}

/// Snapshot of one sequential CRM monitor suite after ingesting `n_steps` observations.
#[derive(Clone, Debug, PartialEq)]
pub struct CrmMonitorSnapshot {
    /// Stream identity.
    pub key: CrmStreamKey,
    /// Observations processed.
    pub n_steps: usize,
    /// Current e-process wealth `E_t`.
    pub eproc_wealth: f64,
    /// Running maximum e-process wealth `max_{s<=t} E_s`.
    pub eproc_max_wealth: f64,
    /// Ville's rejection threshold `1 / alpha`.
    pub eproc_threshold: f64,
    /// First step at which the e-process alarmed (`None` if not alarmed).
    pub eproc_alarm_step: Option<usize>,
    /// First step at which the dispersion-collapse (integrity) monitor alarmed.
    pub dispersion_collapse_step: Option<usize>,
    /// Most recent rolling window `running_sd / reference_sd` ratio (once window is full).
    pub latest_dispersion_ratio: Option<f64>,
    /// All alarms emitted in step order.
    pub alarms: Vec<MonitorAlarm>,
}

/// Combined sequential QA/QC monitor per `(CRM, lab, method)`:
/// 1. Anytime-valid PIT betting e-process (`fmine_numerics::eproc::BettingEProcess`, `BD-169`).
/// 2. Multi-rule pattern monitors (`2 > ±2 SD`, `4 > ±1 SD` same side, sign imbalance, `BD-283`).
/// 3. Rolling chi-square dispersion-collapse tampering monitor (`AlarmScope::Integrity`, `BD-283`).
#[derive(Clone, Debug)]
pub struct CrmSequentialMonitor {
    config: CrmMonitorConfig,
    eproc: BettingEProcess,
    n_steps: usize,
    eproc_alarm_step: Option<usize>,
    dispersion_collapse_step: Option<usize>,
    latest_dispersion_ratio: Option<f64>,
    // Consecutive-run counters for pattern monitors.
    run_gt2_pos: usize,
    run_gt2_neg: usize,
    run_gt1_pos: usize,
    run_gt1_neg: usize,
    // Sign imbalance counters over the stream.
    pos_signs: usize,
    neg_signs: usize,
    sign_alarmed: bool,
    // Rolling window of recent z-scores for dispersion-collapse check.
    recent_z: Vec<f64>,
    alarms: Vec<MonitorAlarm>,
}

/// One typed observation passed into [`CrmSequentialMonitor::observe`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CrmStreamValue {
    /// Measured CRM assay value (same unit as `certified_mean`).
    pub measured: f64,
}

impl CrmSequentialMonitor {
    /// Creates a new sequential monitor for one `(CRM, lab, method)` stream.
    pub fn new(config: CrmMonitorConfig) -> Result<Self, QaqcError> {
        config.validate()?;
        let eproc = BettingEProcess::new(0.5, config.eproc_alpha)?;
        Ok(Self {
            config,
            eproc,
            n_steps: 0,
            eproc_alarm_step: None,
            dispersion_collapse_step: None,
            latest_dispersion_ratio: None,
            run_gt2_pos: 0,
            run_gt2_neg: 0,
            run_gt1_pos: 0,
            run_gt1_neg: 0,
            pos_signs: 0,
            neg_signs: 0,
            sign_alarmed: false,
            recent_z: Vec::new(),
            alarms: Vec::new(),
        })
    }

    /// Observes the next CRM value in stream order, updating the e-process and all pattern/tampering
    /// monitors. Returns any newly emitted alarms at this step.
    pub fn observe(&mut self, value: CrmStreamValue) -> Result<Vec<MonitorAlarm>, QaqcError> {
        if !value.measured.is_finite() {
            return Err(QaqcError::invalid(
                "monitor.measured",
                format!("observation {} is not finite", value.measured),
            ));
        }

        self.n_steps += 1;
        let step = self.n_steps;
        let z = (value.measured - self.config.certified_mean) / self.config.reference_sd;
        let mut step_alarms = Vec::new();

        // 1. Anytime-valid PIT betting e-process (BD-169).
        // Under H_0: X_t ~ N(mu, sigma^2), U_t = Phi(z_t) ~ Uniform(0, 1) with E[U_t] = 0.5.
        let u = normal_cdf(z);
        let wealth = self.eproc.observe(u)?;
        if self.eproc.rejected_null() && self.eproc_alarm_step.is_none() {
            self.eproc_alarm_step = Some(step);
            step_alarms.push(MonitorAlarm {
                monitor: MonitorKind::EProcessDrift,
                scope: AlarmScope::CalibrationOrBias,
                step,
                statistic: wealth,
                detail: format!(
                    "e-process wealth {wealth:.2} crossed Ville threshold {:.2} (alpha={})",
                    self.eproc.threshold(),
                    self.config.eproc_alpha
                ),
            });
        }

        // 2. Two consecutive beyond ±2 SD on the same side (E367).
        if z > 2.0 {
            self.run_gt2_pos += 1;
            self.run_gt2_neg = 0;
        } else if z < -2.0 {
            self.run_gt2_neg += 1;
            self.run_gt2_pos = 0;
        } else {
            self.run_gt2_pos = 0;
            self.run_gt2_neg = 0;
        }
        if self.run_gt2_pos == 2 || self.run_gt2_neg == 2 {
            step_alarms.push(MonitorAlarm {
                monitor: MonitorKind::TwoConsecutiveBeyond2Sd,
                scope: AlarmScope::CalibrationOrBias,
                step,
                statistic: z,
                detail: format!(
                    "two successive CRM results beyond 2 SD on the same side (z={z:.2})"
                ),
            });
        }

        // 3. Four consecutive beyond ±1 SD on the same side (E367).
        if z > 1.0 {
            self.run_gt1_pos += 1;
            self.run_gt1_neg = 0;
        } else if z < -1.0 {
            self.run_gt1_neg += 1;
            self.run_gt1_pos = 0;
        } else {
            self.run_gt1_pos = 0;
            self.run_gt1_neg = 0;
        }
        if self.run_gt1_pos == 4 || self.run_gt1_neg == 4 {
            step_alarms.push(MonitorAlarm {
                monitor: MonitorKind::FourConsecutiveBeyond1Sd,
                scope: AlarmScope::CalibrationOrBias,
                step,
                statistic: z,
                detail: format!(
                    "four successive CRM results beyond 1 SD on the same side (z={z:.2})"
                ),
            });
        }

        // 4. Sign imbalance relative to certified mean (E367).
        if z > 0.0 {
            self.pos_signs += 1;
        } else if z < 0.0 {
            self.neg_signs += 1;
        }
        let non_zero = self.pos_signs + self.neg_signs;
        if !self.sign_alarmed && non_zero >= self.config.sign_min_samples {
            let p = binomial_two_sided_half_p(self.pos_signs, non_zero);
            if p <= self.config.sign_alpha {
                self.sign_alarmed = true;
                step_alarms.push(MonitorAlarm {
                    monitor: MonitorKind::SignImbalance,
                    scope: AlarmScope::CalibrationOrBias,
                    step,
                    statistic: p,
                    detail: format!(
                        "sign imbalance: {}+ / {}- out of {} (two-sided binomial p={p:.4})",
                        self.pos_signs, self.neg_signs, non_zero
                    ),
                });
            }
        }

        // 5. Rolling dispersion-collapse (tampering) monitor (E368, GATE-CORE-02).
        // Opens an Integrity InvestigationCase, never a recalibration.
        self.recent_z.push(z);
        if self.recent_z.len() > self.config.dispersion_window {
            self.recent_z.remove(0);
        }
        if self.recent_z.len() == self.config.dispersion_window
            && let Some((_mean, ratio_sd)) = mean_and_sample_sd(&self.recent_z)
        {
            self.latest_dispersion_ratio = Some(ratio_sd);
            let df = u32::try_from(self.config.dispersion_window - 1).unwrap_or(u32::MAX);
            let chi_sq = f64::from(df) * ratio_sd * ratio_sd;
            let lower_p = chi_square_lower_cdf(chi_sq, df);
            if ratio_sd <= self.config.collapse_ratio_threshold
                && lower_p <= self.config.dispersion_alpha
                && self.dispersion_collapse_step.is_none()
            {
                self.dispersion_collapse_step = Some(step);
                step_alarms.push(MonitorAlarm {
                    monitor: MonitorKind::DispersionCollapse,
                    scope: AlarmScope::Integrity,
                    step,
                    statistic: ratio_sd,
                    detail: format!(
                        "running CRM SD collapsed to {:.3}x reference SD over {} samples (chi-sq lower p={lower_p:.6}); open Integrity InvestigationCase",
                        ratio_sd, self.config.dispersion_window
                    ),
                });
            }
        }

        self.alarms.extend(step_alarms.iter().cloned());
        Ok(step_alarms)
    }

    /// Feeds an entire slice of CRM observations in stream order and returns the final snapshot.
    pub fn observe_all(
        &mut self,
        values: &[CrmStreamValue],
    ) -> Result<CrmMonitorSnapshot, QaqcError> {
        for &v in values {
            self.observe(v)?;
        }
        Ok(self.snapshot())
    }

    /// Current snapshot of monitor state and alarms.
    #[must_use]
    pub fn snapshot(&self) -> CrmMonitorSnapshot {
        CrmMonitorSnapshot {
            key: self.config.key.clone(),
            n_steps: self.n_steps,
            eproc_wealth: self.eproc.wealth,
            eproc_max_wealth: self.eproc.max_wealth,
            eproc_threshold: self.eproc.threshold(),
            eproc_alarm_step: self.eproc_alarm_step,
            dispersion_collapse_step: self.dispersion_collapse_step,
            latest_dispersion_ratio: self.latest_dispersion_ratio,
            alarms: self.alarms.clone(),
        }
    }
}
