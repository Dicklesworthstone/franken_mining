//! Certified Reference Material (CRM) evaluation with explicit SD basis, CANMET batch-mean
//! test, bias confidence intervals, and versioned site rule texts (`BD-167`, `BD-282`;
//! plan §12.4, E166, E221, E222, E364–E366, FX-2).
//! Determinism: BitwisePortable (IEEE-754 arithmetic and `fmine_core::det` primitives only).

use fmine_core::det;
use fmine_drill::records::{AssayMethod, AssayValue};

use crate::error::QaqcError;
use crate::stats::{mean_and_sample_sd, t_critical_two_sided, t_two_sided_p};

/// Named standard-deviation basis for CRM rules (plan §12.4, E364–E366).
///
/// An unnamed `±k SD` rule is ambiguous: the same ten-result batch (FX-2) produces 10 warnings
/// and 8 failures on `σ_C`, 3 warnings and 0 failures on `σ_L`, and a CANMET pass.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SdBasis {
    /// Certified within-lab standard deviation (`σ_C`, E364).
    CertifiedWithinLabSigmaC,
    /// Certified between-lab standard deviation (`σ_L`, E364–E366).
    CertifiedBetweenLabSigmaL,
    /// Laboratory running within-lab standard deviation (`S_W`) computed from the batch.
    RunningWithinLabSw,
}

impl SdBasis {
    /// Stable lower-case code for reports and policy tables.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::CertifiedWithinLabSigmaC => "sigma_c",
            Self::CertifiedBetweenLabSigmaL => "sigma_l",
            Self::RunningWithinLabSw => "running_sw",
        }
    }
}

/// Statistical test applied by a CRM rule (plan §12.4, E364–E365).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CrmTestKind {
    /// Per-observation threshold `|X - μ| <= k * SD` on the rule's named [`SdBasis`].
    SingleValue {
        /// Warning threshold multiple (default `2.0`).
        warn_k: f64,
        /// Failure threshold multiple (default `3.0`).
        fail_k: f64,
    },
    /// Batch-mean CANMET tolerance test: `|m - μ| <= 2 * sqrt(σ_L^2 + S_W^2 / n)` (plan §12.4).
    CanmetBatchMean,
}

/// Multi-point consecutive bias rule inside a site policy (e.g. NGM E166, Loulo E222).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ConsecutiveBiasRule {
    /// Number of consecutive CRM results triggering failure (e.g. 2 for NGM, 3 for Loulo).
    pub count: usize,
    /// SD multiple exceeded by each consecutive point (e.g. `2.0`).
    pub sd_multiple: f64,
    /// Whether all points in the run must lie on the same side of the certified mean.
    pub same_side: bool,
}

/// Grade-tiered bracketing re-assay window triggered on CRM failure (NGM E166).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GradeReassayWindow {
    /// Lower inclusive bound of the certified mass-fraction tier.
    pub min_mass_fraction: f64,
    /// Number of bracketing routine samples before and after the failed CRM to re-assay (`±N`).
    pub bracket_samples_each_side: usize,
}

/// Versioned site CRM rule text registered as policy data (plan §12.4, BD-282).
#[derive(Clone, Debug, PartialEq)]
pub struct SiteCrmRule {
    /// Registry identifier of the rule (e.g. `ngm.crm.v1`).
    pub rule_id: String,
    /// Rule version string.
    pub version: String,
    /// Source document / filing reference (e.g. `Carlin 2025 NI 43-101 p.113`).
    pub source_doc: String,
    /// Named SD basis used to evaluate single-value and consecutive rules.
    pub sd_basis: SdBasis,
    /// Primary test kind (single-value thresholds or CANMET batch mean).
    pub test: CrmTestKind,
    /// Optional consecutive same-side bias failure rule (e.g. 2× > ±2 SD or 3× > ±2 SD).
    pub consecutive_rule: Option<ConsecutiveBiasRule>,
    /// Optional grade-tiered re-assay windows sorted ascending by `min_mass_fraction`.
    pub reassay_windows: Vec<GradeReassayWindow>,
    /// Confidence level for the mandatory [`BiasInterval`] (default `0.95`).
    pub confidence_level: f64,
}

impl SiteCrmRule {
    /// Default single-value rule on a named SD basis (`±2 SD` warning, `±3 SD` failure, 95% bias CI).
    #[must_use]
    pub fn single_value_default(sd_basis: SdBasis) -> Self {
        Self {
            rule_id: format!("corebox.crm.single_{}", sd_basis.code()),
            version: "1".to_string(),
            source_doc: "plan §12.4 default (E364-E366)".to_string(),
            sd_basis,
            test: CrmTestKind::SingleValue {
                warn_k: 2.0,
                fail_k: 3.0,
            },
            consecutive_rule: None,
            reassay_windows: Vec::new(),
            confidence_level: 0.95,
        }
    }

    /// CANMET batch-mean tolerance rule (`|m - μ| <= 2 * sqrt(σ_L^2 + S_W^2 / n)`, E365).
    #[must_use]
    pub fn canmet_v1() -> Self {
        Self {
            rule_id: "corebox.crm.canmet".to_string(),
            version: "1".to_string(),
            source_doc: "Abzalov AMG PDF 150 / plan §12.4 (E365)".to_string(),
            sd_basis: SdBasis::CertifiedBetweenLabSigmaL,
            test: CrmTestKind::CanmetBatchMean,
            consecutive_rule: None,
            reassay_windows: Vec::new(),
            confidence_level: 0.95,
        }
    }

    /// Kibali site rule (`E221`): any single CRM result outside `±3 SD` fails.
    #[must_use]
    pub fn kibali_v1(sd_basis: SdBasis) -> Self {
        Self {
            rule_id: "site.kibali.crm".to_string(),
            version: "2022-tr".to_string(),
            source_doc: "Kibali Technical Report p.102 (E221)".to_string(),
            sd_basis,
            test: CrmTestKind::SingleValue {
                warn_k: 2.0,
                fail_k: 3.0,
            },
            consecutive_rule: None,
            reassay_windows: Vec::new(),
            confidence_level: 0.95,
        }
    }

    /// Loulo-Gounkoto site rule (`E222`): single `> ±3 SD` or three consecutive `> ±2 SD` on one side.
    #[must_use]
    pub fn loulo_v1(sd_basis: SdBasis) -> Self {
        Self {
            rule_id: "site.loulo.crm".to_string(),
            version: "2023-tr".to_string(),
            source_doc: "Loulo-Gounkoto Technical Report (E222)".to_string(),
            sd_basis,
            test: CrmTestKind::SingleValue {
                warn_k: 2.0,
                fail_k: 3.0,
            },
            consecutive_rule: Some(ConsecutiveBiasRule {
                count: 3,
                sd_multiple: 2.0,
                same_side: true,
            }),
            reassay_windows: Vec::new(),
            confidence_level: 0.95,
        }
    }

    /// Nevada Gold Mines (NGM) site rule (`E166`): single `> ±3 SD` or two consecutive `> ±2 SD`
    /// on the same side, with grade-tiered `±5` / `±10` sample re-assay windows.
    #[must_use]
    pub fn ngm_v1(sd_basis: SdBasis) -> Self {
        Self {
            rule_id: "site.ngm.crm".to_string(),
            version: "2025-tr".to_string(),
            source_doc: "NGM Carlin 2025 p.113 / Turquoise Ridge 2024 p.128 (E166)".to_string(),
            sd_basis,
            test: CrmTestKind::SingleValue {
                warn_k: 2.0,
                fail_k: 3.0,
            },
            consecutive_rule: Some(ConsecutiveBiasRule {
                count: 2,
                sd_multiple: 2.0,
                same_side: true,
            }),
            reassay_windows: vec![
                GradeReassayWindow {
                    min_mass_fraction: 0.0,
                    bracket_samples_each_side: 5,
                },
                // High-grade tier (>= 1 ppm = 1e-6 mass fraction) expands re-assay bracket to ±10.
                GradeReassayWindow {
                    min_mass_fraction: 1.0e-6,
                    bracket_samples_each_side: 10,
                },
            ],
            confidence_level: 0.95,
        }
    }

    /// Validates rule parameters without clamping.
    pub fn validate(&self) -> Result<(), QaqcError> {
        if self.rule_id.trim().is_empty() {
            return Err(QaqcError::invalid("crm_rule.rule_id", "must not be empty"));
        }
        if self.version.trim().is_empty() {
            return Err(QaqcError::invalid("crm_rule.version", "must not be empty"));
        }
        if self.source_doc.trim().is_empty() {
            return Err(QaqcError::invalid(
                "crm_rule.source_doc",
                "must not be empty",
            ));
        }
        if !(self.confidence_level.is_finite()
            && self.confidence_level > 0.0
            && self.confidence_level < 1.0)
        {
            return Err(QaqcError::invalid(
                "crm_rule.confidence_level",
                format!("must lie in (0, 1), got {}", self.confidence_level),
            ));
        }
        if let CrmTestKind::SingleValue { warn_k, fail_k } = self.test
            && !(warn_k.is_finite() && fail_k.is_finite() && warn_k > 0.0 && fail_k >= warn_k)
        {
            return Err(QaqcError::invalid(
                "crm_rule.test",
                format!("require 0 < warn_k ({warn_k}) <= fail_k ({fail_k})"),
            ));
        }
        if let Some(c) = self.consecutive_rule
            && (c.count < 2 || !(c.sd_multiple.is_finite() && c.sd_multiple > 0.0))
        {
            return Err(QaqcError::invalid(
                "crm_rule.consecutive_rule",
                format!(
                    "require count >= 2 and sd_multiple > 0, got count={} sd_multiple={}",
                    c.count, c.sd_multiple
                ),
            ));
        }
        for w in &self.reassay_windows {
            if !(w.min_mass_fraction.is_finite() && w.min_mass_fraction >= 0.0) {
                return Err(QaqcError::invalid(
                    "crm_rule.reassay_windows",
                    format!("invalid min_mass_fraction {}", w.min_mass_fraction),
                ));
            }
        }
        Ok(())
    }

    fn reassay_bracket_for(&self, certified_mass_fraction: f64) -> Option<usize> {
        self.reassay_windows
            .iter()
            .filter(|w| certified_mass_fraction >= w.min_mass_fraction)
            .max_by(|a, b| a.min_mass_fraction.total_cmp(&b.min_mass_fraction))
            .map(|w| w.bracket_samples_each_side)
    }
}

/// Certified reference material parameters in consistent mass-fraction (or explicit unit) scale.
#[derive(Clone, Debug, PartialEq)]
pub struct CrmCertificateSpec {
    /// CRM identifier (e.g. `OREAS 264`, `FX-2-Fe`).
    pub crm_id: String,
    /// Analyte name (e.g. `Au`, `Fe`).
    pub analyte: String,
    /// Assay method defining the measurand (`PO-CORE-3`).
    pub method: AssayMethod,
    /// Certified mean `μ` (in mass fraction `[0, 1]`).
    pub certified_mean: f64,
    /// Certified within-lab standard deviation `σ_C` (same unit as `certified_mean`).
    pub sigma_c: f64,
    /// Certified between-lab standard deviation `σ_L` (same unit as `certified_mean`).
    pub sigma_l: f64,
    /// Certificate source reference.
    pub certificate_source: String,
}

impl CrmCertificateSpec {
    /// Validates certificate parameters.
    pub fn validate(&self) -> Result<(), QaqcError> {
        if self.crm_id.trim().is_empty() {
            return Err(QaqcError::invalid("crm.crm_id", "must not be empty"));
        }
        if self.analyte.trim().is_empty() {
            return Err(QaqcError::invalid("crm.analyte", "must not be empty"));
        }
        if !(self.certified_mean.is_finite()
            && self.certified_mean >= 0.0
            && self.certified_mean <= 1.0)
        {
            return Err(QaqcError::invalid(
                "crm.certified_mean",
                format!("mass fraction {} outside [0, 1]", self.certified_mean),
            ));
        }
        if !(self.sigma_c.is_finite() && self.sigma_c >= 0.0 && self.sigma_c <= 1.0) {
            return Err(QaqcError::invalid(
                "crm.sigma_c",
                format!("sigma_c {} outside [0, 1]", self.sigma_c),
            ));
        }
        if !(self.sigma_l.is_finite() && self.sigma_l >= 0.0 && self.sigma_l <= 1.0) {
            return Err(QaqcError::invalid(
                "crm.sigma_l",
                format!("sigma_l {} outside [0, 1]", self.sigma_l),
            ));
        }
        if self.certificate_source.trim().is_empty() {
            return Err(QaqcError::invalid(
                "crm.certificate_source",
                "must not be empty",
            ));
        }
        Ok(())
    }
}

/// One ordered CRM assay observation within a lab stream or batch.
#[derive(Clone, Debug, PartialEq)]
pub struct CrmObservation {
    /// Assay result id.
    pub result_id: String,
    /// Sample id.
    pub sample_id: String,
    /// Laboratory name.
    pub lab: String,
    /// Laboratory batch identifier.
    pub lab_batch: String,
    /// Assay value (censored CRM values are refused rather than substituted under `G-CENS`).
    pub value: AssayValue,
    /// Sequence timestamp (ns since Unix epoch, UTC).
    pub received_at_ns: i64,
}

/// Input bundle for evaluating a batch or sequence of CRM results for one `(CRM, lab, method)`.
#[derive(Clone, Debug, PartialEq)]
pub struct CrmBatchInput {
    /// Certified reference material specification.
    pub spec: CrmCertificateSpec,
    /// Ordered observations for this CRM, analyte, and method.
    pub observations: Vec<CrmObservation>,
}

/// Point-level outcome of one CRM observation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CrmPointStatus {
    /// Within warning and failure bounds.
    Pass,
    /// Beyond `warn_k * SD` but within `fail_k * SD` (and no consecutive failure triggered).
    Warning,
    /// Beyond `fail_k * SD` on the named basis.
    FailSingle,
    /// Triggered the rule's consecutive-point bias failure (e.g. 2× > ±2 SD same side).
    FailConsecutive,
}

/// Detailed verdict for one CRM observation.
#[derive(Clone, Debug, PartialEq)]
pub struct CrmPointVerdict {
    /// Result id.
    pub result_id: String,
    /// Sample id.
    pub sample_id: String,
    /// Lab batch id.
    pub lab_batch: String,
    /// Measured value (mass fraction).
    pub value: f64,
    /// Signed deviation `value - certified_mean`.
    pub deviation: f64,
    /// Standardized residual `(value - certified_mean) / basis_sd`.
    pub z_score: f64,
    /// Verdict under the evaluated rule.
    pub status: CrmPointStatus,
}

/// Mandatory bias magnitude and Student-t confidence interval reported beside every CRM pass/fail
/// verdict (plan §12.4, E365–E366).
#[derive(Clone, Debug, PartialEq)]
pub struct BiasInterval {
    /// Number of observations in the batch.
    pub n: usize,
    /// Sample mean `m`.
    pub sample_mean: f64,
    /// Certified mean `μ`.
    pub certified_mean: f64,
    /// Signed mean bias `m - μ`.
    pub bias: f64,
    /// Relative bias `100 * (m - μ) / μ`; `None` when `μ == 0`.
    pub relative_bias_pct: Option<f64>,
    /// Running within-lab sample SD `S_W` (`None` when `n < 2`).
    pub running_sd: Option<f64>,
    /// Standard error `S_W / sqrt(n)` (`None` when `n < 2`).
    pub standard_error: Option<f64>,
    /// Student-t statistic `bias / SE` (`None` when `n < 2` or `SE == 0`).
    pub t_statistic: Option<f64>,
    /// Two-sided Student-t p-value for `H_0: bias == 0` (`None` when `n < 2` or `SE == 0`).
    pub two_sided_p: Option<f64>,
    /// Confidence level (e.g. `0.95`).
    pub confidence_level: f64,
    /// Lower confidence bound on `bias` (`None` when `n < 2`).
    pub ci_lower: Option<f64>,
    /// Upper confidence bound on `bias` (`None` when `n < 2`).
    pub ci_upper: Option<f64>,
    /// `true` when `n >= 2` and the confidence interval strictly excludes `0.0`.
    pub excludes_zero: bool,
}

/// CANMET batch-mean tolerance test detail (`|m - μ| <= 2 * sqrt(σ_L^2 + S_W^2 / n)`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CanmetBatchDetail {
    /// Absolute bias `|m - μ|`.
    pub abs_bias: f64,
    /// CANMET tolerance limit `2 * sqrt(σ_L^2 + S_W^2 / n)`.
    pub tolerance_limit: f64,
    /// `true` when `abs_bias <= tolerance_limit`.
    pub passed: bool,
}

/// Complete evaluation of a CRM batch under one named [`SiteCrmRule`].
#[derive(Clone, Debug, PartialEq)]
pub struct CrmBatchEvaluation {
    /// Evaluated rule id.
    pub rule_id: String,
    /// Evaluated rule version.
    pub rule_version: String,
    /// Source document for the rule text.
    pub rule_source_doc: String,
    /// Named SD basis used.
    pub sd_basis: SdBasis,
    /// Numeric SD value actually applied for z-scores (`None` only for pure CANMET if unused).
    pub effective_sd: f64,
    /// Overall batch pass (`true` iff zero point/consecutive failures and CANMET passed if applied).
    pub passed: bool,
    /// Count of observations beyond `warn_k * SD` (includes single failures).
    pub warning_count: usize,
    /// Count of single-value failures (`|X - μ| > fail_k * SD`).
    pub single_failure_count: usize,
    /// Count of consecutive-bias rule triggers.
    pub consecutive_failure_count: usize,
    /// Count of distinct failing CRM observations (`FailSingle` or `FailConsecutive`; never exceeds `points.len()`).
    pub failed_point_count: usize,
    /// Per-observation verdicts in input order.
    pub points: Vec<CrmPointVerdict>,
    /// Mandatory bias magnitude and Student-t confidence interval (always populated).
    pub bias: BiasInterval,
    /// CANMET detail when `rule.test == CrmTestKind::CanmetBatchMean`.
    pub canmet: Option<CanmetBatchDetail>,
    /// Recommended bracketing re-assay window (samples each side) if the batch failed and the
    /// site rule declares grade-tiered windows.
    pub reassay_bracket_each_side: Option<usize>,
}

/// Evaluates a CRM batch under a versioned [`SiteCrmRule`] with an explicit [`SdBasis`].
///
/// Always computes and attaches [`BiasInterval`] alongside pass/fail so a CANMET tolerance
/// pass can never mask a statistically certain bias (E365–E366).
///
/// # Errors
/// - [`QaqcError::InvalidParameter`] for invalid certificate or rule parameters or a zero basis SD.
/// - [`QaqcError::InsufficientData`] for empty observations or `< 2` values under `RunningWithinLabSw` / `CanmetBatchMean`.
/// - [`QaqcError::UnusableInput`] if any CRM observation is censored (`G-CENS`: never substitutes DL/2).
pub fn evaluate_crm_batch(
    input: &CrmBatchInput,
    rule: &SiteCrmRule,
) -> Result<CrmBatchEvaluation, QaqcError> {
    input.spec.validate()?;
    rule.validate()?;

    if input.observations.is_empty() {
        return Err(QaqcError::InsufficientData {
            context: "crm.evaluate_batch".to_string(),
            required: 1,
            actual: 0,
        });
    }

    let mut values = Vec::with_capacity(input.observations.len());
    for obs in &input.observations {
        match obs.value {
            AssayValue::Measured { mass_fraction } => {
                if !(mass_fraction.is_finite() && (0.0..=1.0).contains(&mass_fraction)) {
                    return Err(QaqcError::invalid(
                        "crm.observation.value",
                        format!(
                            "result '{}': mass fraction {mass_fraction} outside [0, 1]",
                            obs.result_id
                        ),
                    ));
                }
                values.push(mass_fraction);
            }
            AssayValue::BelowDetection { limit } => {
                return Err(QaqcError::UnusableInput {
                    context: format!("crm.result '{}'", obs.result_id),
                    detail: format!(
                        "CRM result is below detection (<{limit}); G-CENS forbids substituting DL/2"
                    ),
                });
            }
            AssayValue::AboveUpper { limit } => {
                return Err(QaqcError::UnusableInput {
                    context: format!("crm.result '{}'", obs.result_id),
                    detail: format!("CRM result is above upper limit (>{limit})"),
                });
            }
        }
    }

    let bias = compute_bias_interval(&values, input.spec.certified_mean, rule.confidence_level);

    let effective_sd = match rule.test {
        CrmTestKind::CanmetBatchMean => {
            if values.len() < 2 {
                return Err(QaqcError::InsufficientData {
                    context: "crm.canmet_batch_mean".to_string(),
                    required: 2,
                    actual: values.len(),
                });
            }
            if input.spec.sigma_l <= 0.0 {
                return Err(QaqcError::invalid(
                    "crm.sigma_l",
                    "CANMET test requires certified between-lab sigma_l > 0",
                ));
            }
            input.spec.sigma_l
        }
        CrmTestKind::SingleValue { .. } => match rule.sd_basis {
            SdBasis::CertifiedWithinLabSigmaC => {
                if input.spec.sigma_c <= 0.0 {
                    return Err(QaqcError::invalid(
                        "crm.sigma_c",
                        "sigma_c basis requires certified within-lab sigma_c > 0",
                    ));
                }
                input.spec.sigma_c
            }
            SdBasis::CertifiedBetweenLabSigmaL => {
                if input.spec.sigma_l <= 0.0 {
                    return Err(QaqcError::invalid(
                        "crm.sigma_l",
                        "sigma_l basis requires certified between-lab sigma_l > 0",
                    ));
                }
                input.spec.sigma_l
            }
            SdBasis::RunningWithinLabSw => {
                let sw = bias.running_sd.ok_or_else(|| QaqcError::InsufficientData {
                    context: "crm.running_sw_basis".to_string(),
                    required: 2,
                    actual: values.len(),
                })?;
                if sw <= 0.0 {
                    return Err(QaqcError::invalid(
                        "crm.running_sw",
                        "running_sw basis requires non-zero batch standard deviation",
                    ));
                }
                sw
            }
        },
    };

    let mut points = Vec::with_capacity(values.len());
    let mut warning_count = 0usize;
    let mut single_failure_count = 0usize;
    let mut consecutive_failure_count = 0usize;
    let mut consec_pos = 0usize;
    let mut consec_neg = 0usize;
    let mut consec_any = 0usize;

    for (obs, &val) in input.observations.iter().zip(&values) {
        let deviation = val - input.spec.certified_mean;
        let z_score = deviation / effective_sd;
        let abs_z = z_score.abs();

        let mut status = match rule.test {
            CrmTestKind::SingleValue { warn_k, fail_k } => {
                if abs_z > warn_k {
                    warning_count += 1;
                }
                if abs_z > fail_k {
                    single_failure_count += 1;
                    CrmPointStatus::FailSingle
                } else if abs_z > warn_k {
                    CrmPointStatus::Warning
                } else {
                    CrmPointStatus::Pass
                }
            }
            CrmTestKind::CanmetBatchMean => CrmPointStatus::Pass,
        };

        if let Some(consec) = rule.consecutive_rule {
            if abs_z > consec.sd_multiple {
                consec_any += 1;
                if deviation > 0.0 {
                    consec_pos += 1;
                    consec_neg = 0;
                } else if deviation < 0.0 {
                    consec_neg += 1;
                    consec_pos = 0;
                } else {
                    consec_pos = 0;
                    consec_neg = 0;
                }
            } else {
                consec_any = 0;
                consec_pos = 0;
                consec_neg = 0;
            }
            let triggered = if consec.same_side {
                consec_pos >= consec.count || consec_neg >= consec.count
            } else {
                consec_any >= consec.count
            };
            if triggered {
                consecutive_failure_count += 1;
                if status != CrmPointStatus::FailSingle {
                    status = CrmPointStatus::FailConsecutive;
                }
            }
        }

        points.push(CrmPointVerdict {
            result_id: obs.result_id.clone(),
            sample_id: obs.sample_id.clone(),
            lab_batch: obs.lab_batch.clone(),
            value: val,
            deviation,
            z_score,
            status,
        });
    }

    let canmet = if matches!(rule.test, CrmTestKind::CanmetBatchMean) {
        let sw = bias.running_sd.unwrap_or(0.0);
        let n_f = values.len() as f64;
        let tolerance_limit =
            2.0 * det::sqrt(input.spec.sigma_l * input.spec.sigma_l + (sw * sw) / n_f);
        let abs_bias = bias.bias.abs();
        Some(CanmetBatchDetail {
            abs_bias,
            tolerance_limit,
            passed: abs_bias <= tolerance_limit,
        })
    } else {
        None
    };

    let failed_point_count = points
        .iter()
        .filter(|p| {
            matches!(
                p.status,
                CrmPointStatus::FailSingle | CrmPointStatus::FailConsecutive
            )
        })
        .count();

    let passed = single_failure_count == 0
        && consecutive_failure_count == 0
        && canmet.is_none_or(|c| c.passed);

    let reassay_bracket_each_side = if passed {
        None
    } else {
        rule.reassay_bracket_for(input.spec.certified_mean)
    };

    Ok(CrmBatchEvaluation {
        rule_id: rule.rule_id.clone(),
        rule_version: rule.version.clone(),
        rule_source_doc: rule.source_doc.clone(),
        sd_basis: rule.sd_basis,
        effective_sd,
        passed,
        warning_count,
        single_failure_count,
        consecutive_failure_count,
        failed_point_count,
        points,
        bias,
        canmet,
        reassay_bracket_each_side,
    })
}

/// Multi-basis comparison across `σ_C`, `σ_L`, `S_W`, and CANMET on the same CRM batch (E366).
#[derive(Clone, Debug, PartialEq)]
pub struct MultiBasisComparison {
    /// Single-value evaluation on certified within-lab `σ_C` (if `σ_C > 0`).
    pub on_sigma_c: Option<CrmBatchEvaluation>,
    /// Single-value evaluation on certified between-lab `σ_L` (if `σ_L > 0`).
    pub on_sigma_l: Option<CrmBatchEvaluation>,
    /// Single-value evaluation on running within-lab `S_W` (if `n >= 2` and `S_W > 0`).
    pub on_running_sw: Option<CrmBatchEvaluation>,
    /// CANMET batch-mean evaluation (if `n >= 2` and `σ_L > 0`).
    pub canmet: Option<CrmBatchEvaluation>,
    /// Shared bias magnitude and Student-t CI across all views.
    pub bias: BiasInterval,
}

/// Configuration for [`evaluate_crm_all_bases`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MultiBasisConfig {
    /// Warning SD multiple for the single-value views (default `2.0`).
    pub warn_k: f64,
    /// Failure SD multiple for the single-value views (default `3.0`).
    pub fail_k: f64,
    /// Confidence level for the Student-t bias interval (default `0.95`).
    pub confidence_level: f64,
}

impl Default for MultiBasisConfig {
    fn default() -> Self {
        Self {
            warn_k: 2.0,
            fail_k: 3.0,
            confidence_level: 0.95,
        }
    }
}

/// Evaluates one CRM batch simultaneously across every applicable named SD basis plus CANMET,
/// demonstrating basis sensitivity without ambiguity (`BD-282`, `GATE-CORE-02`, E366).
pub fn evaluate_crm_all_bases(
    input: &CrmBatchInput,
    config: &MultiBasisConfig,
) -> Result<MultiBasisComparison, QaqcError> {
    input.spec.validate()?;
    if !(config.warn_k.is_finite()
        && config.fail_k.is_finite()
        && config.warn_k > 0.0
        && config.fail_k >= config.warn_k)
    {
        return Err(QaqcError::invalid(
            "multi_basis.k",
            format!(
                "require 0 < warn_k ({}) <= fail_k ({})",
                config.warn_k, config.fail_k
            ),
        ));
    }
    if !(config.confidence_level.is_finite()
        && config.confidence_level > 0.0
        && config.confidence_level < 1.0)
    {
        return Err(QaqcError::invalid(
            "multi_basis.confidence_level",
            format!("must lie in (0, 1), got {}", config.confidence_level),
        ));
    }

    let make_single = |basis: SdBasis| {
        let mut r = SiteCrmRule::single_value_default(basis);
        r.test = CrmTestKind::SingleValue {
            warn_k: config.warn_k,
            fail_k: config.fail_k,
        };
        r.confidence_level = config.confidence_level;
        r
    };

    let on_sigma_c = if input.spec.sigma_c > 0.0 {
        Some(evaluate_crm_batch(
            input,
            &make_single(SdBasis::CertifiedWithinLabSigmaC),
        )?)
    } else {
        None
    };

    let on_sigma_l = if input.spec.sigma_l > 0.0 {
        Some(evaluate_crm_batch(
            input,
            &make_single(SdBasis::CertifiedBetweenLabSigmaL),
        )?)
    } else {
        None
    };

    let on_running_sw = if input.observations.len() >= 2 {
        match evaluate_crm_batch(input, &make_single(SdBasis::RunningWithinLabSw)) {
            Ok(eval) => Some(eval),
            Err(QaqcError::InvalidParameter { field, .. }) if field == "crm.running_sw" => None,
            Err(err) => return Err(err),
        }
    } else {
        None
    };

    let canmet = if input.observations.len() >= 2 && input.spec.sigma_l > 0.0 {
        let mut r = SiteCrmRule::canmet_v1();
        r.confidence_level = config.confidence_level;
        Some(evaluate_crm_batch(input, &r)?)
    } else {
        None
    };

    let bias = on_sigma_c
        .as_ref()
        .or(on_sigma_l.as_ref())
        .or(on_running_sw.as_ref())
        .or(canmet.as_ref())
        .map(|e| e.bias.clone())
        .ok_or_else(|| {
            QaqcError::invalid(
                "crm.spec",
                "at least one positive SD basis (sigma_c, sigma_l, or running S_W with n >= 2) is required",
            )
        })?;

    Ok(MultiBasisComparison {
        on_sigma_c,
        on_sigma_l,
        on_running_sw,
        canmet,
        bias,
    })
}

fn compute_bias_interval(
    values: &[f64],
    certified_mean: f64,
    confidence_level: f64,
) -> BiasInterval {
    let n = values.len();
    let sample_mean = values.iter().sum::<f64>() / (n as f64);
    let bias = sample_mean - certified_mean;
    let relative_bias_pct = if certified_mean > 0.0 {
        Some(100.0 * bias / certified_mean)
    } else {
        None
    };

    if let Some((mean_w, sd)) = mean_and_sample_sd(values) {
        let bias_w = mean_w - certified_mean;
        let se = sd / det::sqrt(n as f64);
        let df = u32::try_from(n - 1).unwrap_or(u32::MAX);
        let (t_stat, p_val, ci_lo, ci_hi) = if se > 0.0 {
            let t = bias_w / se;
            let p = t_two_sided_p(t, df);
            let crit = t_critical_two_sided(confidence_level, df);
            (Some(t), Some(p), bias_w - crit * se, bias_w + crit * se)
        } else {
            (None, None, bias_w, bias_w)
        };
        let excludes_zero = ci_lo > 0.0 || ci_hi < 0.0;
        BiasInterval {
            n,
            sample_mean: mean_w,
            certified_mean,
            bias: bias_w,
            relative_bias_pct: if certified_mean > 0.0 {
                Some(100.0 * bias_w / certified_mean)
            } else {
                None
            },
            running_sd: Some(sd),
            standard_error: Some(se),
            t_statistic: t_stat,
            two_sided_p: p_val,
            confidence_level,
            ci_lower: Some(ci_lo),
            ci_upper: Some(ci_hi),
            excludes_zero,
        }
    } else {
        BiasInterval {
            n,
            sample_mean,
            certified_mean,
            bias,
            relative_bias_pct,
            running_sd: None,
            standard_error: None,
            t_statistic: None,
            two_sided_p: None,
            confidence_level,
            ci_lower: None,
            ci_upper: None,
            excludes_zero: false,
        }
    }
}
