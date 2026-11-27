//! DESIGN (`BD-167`, `BD-282`, plan §12.4): CRM single-value rules on three named SD bases,
//! the CANMET batch-mean rule, and the observed bias with its Student-t confidence interval.
//! Determinism: BitwisePortable (fixed-order IEEE-754 reductions and `fmine_core::det` math).
//!
//! All concentrations and SDs use one caller-declared unit, analyte, method, CRM and lab. The caller
//! supplies certificate and policy provenance; this numerical kernel authenticates neither. It never
//! substitutes the observed sample SD for the declared lab running SD. The conditional bias interval
//! assumes independent, identically distributed normal results and holds the certified mean fixed;
//! certificate uncertainty, serial dependence and systematic error are not included in that interval.
//! These are fixed-batch checks, not an anytime-valid monitor, a compliance finding or a release decision.

use fmine_core::det;

use crate::records::DrillError;
use crate::stats::t_two_sided_p;

/// Bound on one fixed CRM batch; at least two observations are needed for the sample SD and interval.
pub const MAX_CRM_OBSERVATIONS: usize = 4096;

/// All numeric inputs in one common concentration unit. Values retain caller order.
#[derive(Clone, Debug, PartialEq)]
pub struct CrmBatch {
    pub certified_mean: f64,
    /// Certified within-lab SD, σ_C, as named by plan §12.4.
    pub sigma_c: f64,
    /// Certified between-lab SD, σ_L, as named by plan §12.4.
    pub sigma_l: f64,
    /// Independently supplied running within-lab SD, S_W; not estimated from this batch.
    pub lab_running_sw: f64,
    /// Uncensored observations; every value participates in the assessment.
    pub values: Vec<f64>,
}

/// Explicit policy: a strict exceedance warns or fails; equality remains inside the limit.
/// Comparisons use the computed binary64 values; no decimal rounding tolerance is assumed.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CrmPolicy {
    pub warning_k: f64,
    pub failure_k: f64,
    /// Central two-sided confidence level for the signed bias interval, strictly between zero and one.
    pub confidence: f64,
}

/// The three distinct SD interpretations; none is inferred from another.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SdBasis {
    CertifiedWithinLab,
    CertifiedBetweenLab,
    LabRunning,
}

impl SdBasis {
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::CertifiedWithinLab => "sigma_c",
            Self::CertifiedBetweenLab => "sigma_l",
            Self::LabRunning => "lab_running_sw",
        }
    }
}

/// Exclusive single-observation outcome on one SD basis.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CrmState {
    Pass,
    Warning,
    Failure,
}

impl CrmState {
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::Pass => "pass",
            Self::Warning => "warning",
            Self::Failure => "failure",
        }
    }
}

/// One result, indexed back to the original input order and its source identity.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ObservationCheck {
    pub index: usize,
    /// Signed observation minus certified mean, in the supplied concentration unit.
    pub deviation: f64,
    pub state: CrmState,
}

#[derive(Clone, Debug, PartialEq)]
pub struct BasisAssessment {
    pub basis: SdBasis,
    pub sd: f64,
    /// Absolute deviation at the warning boundary, `warning_k × sd`.
    pub warning_limit: f64,
    /// Absolute deviation at the failure boundary, `failure_k × sd`.
    pub failure_limit: f64,
    /// All results exceeding the warning limit, INCLUDING the failures.
    pub warnings: usize,
    /// Results exceeding the failure limit; a subset of `warnings`.
    pub failures: usize,
    pub results: Vec<ObservationCheck>,
}

/// Signed mean bias and its conditional, central Student-t interval.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BiasInterval {
    pub signed_bias: f64,
    pub magnitude: f64,
    /// Lower bound on the SIGNED bias, not a nonnegative magnitude bound.
    pub lower: f64,
    /// Upper bound on the SIGNED bias; negative values are retained.
    pub upper: f64,
    pub confidence: f64,
    pub degrees_of_freedom: u32,
    pub t_critical: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CanmetAssessment {
    /// `2 × sqrt(sigma_l² + lab_running_sw² / n)` in the supplied concentration unit.
    pub limit: f64,
    pub absolute_bias: f64,
    /// `absolute_bias <= limit`; this result does not override any single-value warning or failure.
    pub passed: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CrmAssessment {
    pub n: usize,
    pub mean: f64,
    /// Observed sample SD with `n − 1` denominator.
    pub sample_sd: f64,
    /// `sample_sd / sqrt(n)`, used only in the conditional bias interval.
    pub standard_error: f64,
    pub bias: BiasInterval,
    /// Certified within-lab, certified between-lab, then lab running SD, in that order.
    pub single_value: [BasisAssessment; 3],
    pub canmet: CanmetAssessment,
}

fn invalid(field: &str, detail: impl Into<String>) -> DrillError {
    DrillError::invalid(field, detail)
}

/// Neumaier reduction of bounded normalized values, in sorted observation order.
fn sum(values: impl Iterator<Item = f64>) -> f64 {
    let (mut total, mut correction) = (0.0_f64, 0.0);
    for value in values {
        let next = total + value;
        correction += if total.abs() >= value.abs() {
            (total - next) + value
        } else {
            (value - next) + total
        };
        total = next;
    }
    total + correction
}

/// Offset and scale before reduction, avoiding squares of large concentrations or cancellation in raw moments.
fn moments(values: &[f64], n: f64) -> (f64, f64) {
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    let min = sorted[0];
    let range = sorted[sorted.len() - 1] - min;
    if range == 0.0 {
        return (min, 0.0);
    }
    let normalized: Vec<f64> = sorted.iter().map(|x| (x - min) / range).collect();
    let mean = sum(normalized.iter().copied()) / n;
    let variance = sum(normalized.iter().map(|x| (x - mean) * (x - mean))) / (n - 1.0);
    (min + range * mean, range * det::sqrt(variance))
}

/// Invert the existing two-sided t tail; the returned upper bracket has tail at most `1 − confidence`.
fn critical_t(df: u32, confidence: f64) -> Result<f64, DrillError> {
    let tail = 1.0 - confidence;
    let probability = |t| {
        let p = t_two_sided_p(t, df);
        if p.is_finite() && (0.0..=1.0).contains(&p) {
            Ok(p)
        } else {
            Err(invalid(
                "qaqc.confidence",
                "Student-t tail is outside finite probability arithmetic",
            ))
        }
    };
    let (mut lower, mut upper) = (0.0, 1.0);
    for _ in 0..64 {
        if probability(upper)? <= tail {
            break;
        }
        lower = upper;
        upper *= 2.0;
    }
    if probability(upper)? > tail {
        return Err(invalid(
            "qaqc.confidence",
            "Student-t critical value could not be bracketed",
        ));
    }
    for _ in 0..80 {
        let middle = lower + (upper - lower) / 2.0;
        if middle == lower || middle == upper {
            break;
        }
        if probability(middle)? > tail {
            lower = middle;
        } else {
            upper = middle;
        }
    }
    if (probability(upper)? - tail).abs() > 5e-10 {
        return Err(invalid(
            "qaqc.confidence",
            "requested confidence exceeds Student-t numerical resolution",
        ));
    }
    Ok(upper)
}

fn assess_basis(
    batch: &CrmBatch,
    policy: &CrmPolicy,
    basis: SdBasis,
    sd: f64,
) -> Result<BasisAssessment, DrillError> {
    let warning_limit = policy.warning_k * sd;
    let failure_limit = policy.failure_k * sd;
    if !warning_limit.is_finite()
        || !failure_limit.is_finite()
        || (sd > 0.0 && warning_limit == 0.0)
    {
        return Err(invalid(
            "qaqc.limits",
            "single-value limits overflow or underflow finite arithmetic",
        ));
    }
    let mut assessment = BasisAssessment {
        basis,
        sd,
        warning_limit,
        failure_limit,
        warnings: 0,
        failures: 0,
        results: Vec::with_capacity(batch.values.len()),
    };
    for (index, value) in batch.values.iter().enumerate() {
        let deviation = value - batch.certified_mean;
        let warns = deviation.abs() > warning_limit;
        let fails = deviation.abs() > failure_limit;
        assessment.warnings += usize::from(warns);
        assessment.failures += usize::from(fails);
        assessment.results.push(ObservationCheck {
            index,
            deviation,
            state: if fails {
                CrmState::Failure
            } else if warns {
                CrmState::Warning
            } else {
                CrmState::Pass
            },
        });
    }
    Ok(assessment)
}

/// Evaluate every supplied result on all three named SD bases, and report CANMET and bias separately.
///
/// The observed bias interval is `(mean − certified_mean) ± t × sample_sd / sqrt(n)` on `n − 1` df.
/// It is a fixed-batch interval conditional on the module's assumptions, not a calibrated probability
/// or an anytime-valid sequential test. Zero observed SD gives a collapsed conditional interval;
/// it does not establish that the lab or reference has zero uncertainty.
///
/// # Errors
/// [`DrillError::Invalid`] (`FMINE-REF-109`) for a batch outside `2..=MAX_CRM_OBSERVATIONS`, a negative or
/// nonfinite concentration/SD, policy without `0 < warning_k < failure_k`, unrepresentable confidence,
/// or arithmetic that overflows or loses a nonzero uncertainty term. No observations are omitted.
pub fn score_crm_batch(batch: &CrmBatch, policy: &CrmPolicy) -> Result<CrmAssessment, DrillError> {
    let n = batch.values.len();
    if !(2..=MAX_CRM_OBSERVATIONS).contains(&n) {
        return Err(invalid(
            "qaqc.values",
            format!("a CRM batch requires 2..={MAX_CRM_OBSERVATIONS} observations; got {n}"),
        ));
    }
    for (field, value) in [
        ("qaqc.certified_mean", batch.certified_mean),
        ("qaqc.sigma_c", batch.sigma_c),
        ("qaqc.sigma_l", batch.sigma_l),
        ("qaqc.lab_running_sw", batch.lab_running_sw),
    ] {
        if !value.is_finite() || value < 0.0 {
            return Err(invalid(
                field,
                "concentration and SD declarations must be finite and nonnegative",
            ));
        }
    }
    if let Some(index) = batch.values.iter().position(|x| !x.is_finite() || *x < 0.0) {
        return Err(invalid(
            "qaqc.values",
            format!("observation {index} must be finite and nonnegative"),
        ));
    }
    if !policy.warning_k.is_finite()
        || !policy.failure_k.is_finite()
        || policy.warning_k <= 0.0
        || policy.failure_k <= policy.warning_k
    {
        return Err(invalid(
            "qaqc.policy",
            "finite multipliers require 0 < warning_k < failure_k",
        ));
    }
    let tail = 1.0 - policy.confidence;
    if !policy.confidence.is_finite()
        || !(policy.confidence > 0.0 && policy.confidence < 1.0)
        || tail == 0.0
        || tail == 1.0
    {
        return Err(invalid(
            "qaqc.confidence",
            "confidence and its tail must both be representable strictly between zero and one",
        ));
    }
    let count = u32::try_from(n).map_err(|e| invalid("qaqc.values", e.to_string()))?;
    let degrees_of_freedom = count - 1;
    let nf = f64::from(count);
    let (mean, sample_sd) = moments(&batch.values, nf);
    let root_n = det::sqrt(nf);
    let standard_error = sample_sd / root_n;
    if (mean == 0.0 && batch.values.iter().any(|x| *x > 0.0))
        || (sample_sd == 0.0 && batch.values.iter().any(|x| *x != mean))
        || (sample_sd > 0.0 && standard_error == 0.0)
    {
        return Err(invalid(
            "qaqc.arithmetic",
            "nonzero batch moments underflow finite arithmetic",
        ));
    }
    let t_critical = critical_t(degrees_of_freedom, policy.confidence)?;
    let signed_bias = mean - batch.certified_mean;
    let margin = t_critical * standard_error;
    let lower = signed_bias - margin;
    let upper = signed_bias + margin;
    let running_standard_error = batch.lab_running_sw / root_n;
    if (standard_error > 0.0 && t_critical > 0.0 && margin == 0.0)
        || (margin > 0.0 && lower == upper)
        || (batch.lab_running_sw > 0.0 && running_standard_error == 0.0)
    {
        return Err(invalid(
            "qaqc.arithmetic",
            "a nonzero uncertainty term or bias interval is lost to finite precision",
        ));
    }
    let limit = 2.0 * det::hypot(batch.sigma_l, running_standard_error);
    if [
        mean,
        sample_sd,
        standard_error,
        signed_bias,
        margin,
        lower,
        upper,
        limit,
    ]
    .iter()
    .any(|x| !x.is_finite())
    {
        return Err(invalid(
            "qaqc.arithmetic",
            "batch moments, bias interval or CANMET limit exceed finite arithmetic",
        ));
    }
    let magnitude = signed_bias.abs();
    Ok(CrmAssessment {
        n,
        mean,
        sample_sd,
        standard_error,
        bias: BiasInterval {
            signed_bias,
            magnitude,
            lower,
            upper,
            confidence: policy.confidence,
            degrees_of_freedom,
            t_critical,
        },
        single_value: [
            assess_basis(batch, policy, SdBasis::CertifiedWithinLab, batch.sigma_c)?,
            assess_basis(batch, policy, SdBasis::CertifiedBetweenLab, batch.sigma_l)?,
            assess_basis(batch, policy, SdBasis::LabRunning, batch.lab_running_sw)?,
        ],
        canmet: CanmetAssessment {
            limit,
            absolute_bias: magnitude,
            passed: magnitude <= limit,
        },
    })
}
