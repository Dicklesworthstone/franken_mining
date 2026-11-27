//! Duplicate precision statistics (`CV_AVR%`, `HARD`, `AMPD`, Thompson–Howarth, `E374` mass
//! match, `FieldDuplicateAbsent`), Gy's fundamental sampling error (general and calibrated form
//! with `K`/`α` provenance and grade-level validity), and nugget split → protocol-vs-infill
//! decision options (`BD-167`, `BD-284`; plan §12.3, E359–E363, E369–E374, E390, FX-1, FX-4).
//! Determinism: BitwisePortable (IEEE-754 arithmetic and `fmine_core::det` primitives only).

use std::collections::BTreeMap;

use fmine_core::det;
use fmine_drill::records::{AssayValue, DuplicateStage, SampleType};

use crate::error::QaqcError;
use crate::stats::{pow_pos, sorted_quantile};

/// Mandatory Thompson–Howarth caveat (plan §12.3, E369–E370).
pub const THOMPSON_HOWARTH_CAVEAT: &str = "assumes normal errors; optimistic";

/// Minimum duplicate pairs required by the Thompson–Howarth estimator (E369).
pub const THOMPSON_HOWARTH_MIN_PAIRS: usize = 50;

/// Default François-Bongarçon (1993) uncalibrated Gy constants (`E363`).
pub const GY_DEFAULT_K: f64 = 470.0;
/// Default François-Bongarçon (1993) uncalibrated Gy exponent (`E363`).
pub const GY_DEFAULT_ALPHA: f64 = 1.5;
/// Gy's default safety line: relative variance `0.01` = 10% relative error (`E360`).
pub const GY_SAFETY_LINE_REL_VAR: f64 = 0.01;
/// Minimum increments/cuts before a sampling protocol is flagged (AMG PDF 144, plan §12.3).
pub const GY_MIN_INCREMENTS: usize = 30;

/// One duplicate assay pair (original and split) with optional mass/support metadata (`E374`).
#[derive(Clone, Debug, PartialEq)]
pub struct DuplicatePair {
    /// Original sample id.
    pub original_sample_id: String,
    /// Duplicate sample id.
    pub duplicate_sample_id: String,
    /// Stage at which the duplicate was split.
    pub stage: DuplicateStage,
    /// Original assay value.
    pub original: AssayValue,
    /// Duplicate assay value.
    pub duplicate: AssayValue,
    /// Original sample type (for E374 quarter-core-of-half-core guard).
    pub original_sample_type: Option<SampleType>,
    /// Duplicate sample type.
    pub duplicate_sample_type: Option<SampleType>,
    /// Original sample mass in grams, if weighed.
    pub original_mass_g: Option<f64>,
    /// Duplicate sample mass in grams, if weighed.
    pub duplicate_mass_g: Option<f64>,
    /// Optional grade band label for grouped reporting.
    pub grade_band: Option<String>,
}

/// Policy governing duplicate pair evaluation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DuplicatePolicy {
    /// Maximum tolerated relative mass discrepancy `|m_dup - m_orig| / m_orig` (default `0.25`).
    pub max_relative_mass_diff: f64,
    /// If `true`, any mass/support mismatch (`E374`) fails the call with
    /// [`QaqcError::DuplicateSupportMismatch`]; if `false`, unfit pairs are excluded and listed.
    pub refuse_on_mass_mismatch: bool,
}

impl Default for DuplicatePolicy {
    fn default() -> Self {
        Self {
            max_relative_mass_diff: 0.25,
            refuse_on_mass_mismatch: false,
        }
    }
}

/// Thompson–Howarth precision estimate (`E369`, `E370`).
#[derive(Clone, Debug, PartialEq)]
pub struct ThompsonHowarthEstimate {
    /// Number of valid pairs used (always `>= 50`).
    pub n_pairs: usize,
    /// Number of 11-pair Thompson–Howarth bins formed.
    pub n_bins: usize,
    /// Fitted zero-concentration intercept `s_0` (absolute SD at grade 0).
    pub intercept_s0: f64,
    /// Fitted proportional slope `k` (`SD(c) = s_0 + k * c`).
    pub slope_k: f64,
    /// Relative precision error (%) at the median pair mean (`100 * SD(c_med) / c_med`).
    pub relative_sd_pct_at_median: f64,
    /// Mandatory caveat: `"assumes normal errors; optimistic"` (`E370`).
    pub caveat: String,
}

/// Canonical duplicate precision report for one group (plan §12.3, E369–E374).
#[derive(Clone, Debug, PartialEq)]
pub struct DuplicatePrecisionReport {
    /// Number of input pairs supplied.
    pub input_pairs: usize,
    /// Valid uncensored positive-sum mass-matched pairs included in `CV_AVR%`.
    pub valid_pairs: usize,
    /// Pairs excluded because either side was censored (`G-CENS`, never substituted).
    pub excluded_censored: usize,
    /// Pairs excluded because both sides were `0.0` (`0/0` undefined).
    pub excluded_zero_pairs: usize,
    /// Duplicate sample ids rejected for `E374` mass or support mismatch.
    pub unfit_mass_mismatch_ids: Vec<String>,
    /// Canonical relative precision: `CV_AVR% = 100 * sqrt((2/N) * sum((a-b)^2/(a+b)^2))` (`E371`).
    pub cv_avr_pct: f64,
    /// Relative variance `(CV_AVR% / 100)^2` feeding the stage/nugget decomposition.
    pub relative_variance: f64,
    /// Derived HARD view: `CV_AVR% / sqrt(2)` (`E371`).
    pub hard_from_cv_pct: f64,
    /// Derived AMPD view: `sqrt(2) * CV_AVR%` (`E371`).
    pub ampd_from_cv_pct: f64,
    /// Empirical median per-pair HARD (`100 * |a-b| / (a+b)`).
    pub median_pair_hard_pct: f64,
    /// Empirical 90th-percentile per-pair HARD (`100 * |a-b| / (a+b)`).
    pub p90_pair_hard_pct: f64,
    /// Empirical mean per-pair AMPD (`200 * |a-b| / (a+b)`).
    pub mean_pair_ampd_pct: f64,
    /// Thompson–Howarth view when `valid_pairs >= 50` (always carries the optimistic caveat);
    /// `None` when `valid_pairs < 50` (`E369`).
    pub thompson_howarth: Option<ThompsonHowarthEstimate>,
}

/// Evaluates duplicate precision for a slice of pairs under `policy`.
///
/// Computes canonical `CV_AVR%`, derived `HARD` and `AMPD`, enforces `E374` mass matching,
/// preserves censored pairs without DL/2 substitution, and emits Thompson–Howarth only when
/// `N >= 50` with its mandatory `"assumes normal errors; optimistic"` label.
pub fn evaluate_duplicates(
    pairs: &[DuplicatePair],
    policy: &DuplicatePolicy,
) -> Result<DuplicatePrecisionReport, QaqcError> {
    if !(policy.max_relative_mass_diff.is_finite() && policy.max_relative_mass_diff >= 0.0) {
        return Err(QaqcError::invalid(
            "duplicate_policy.max_relative_mass_diff",
            format!(
                "must be finite and >= 0, got {}",
                policy.max_relative_mass_diff
            ),
        ));
    }

    let mut excluded_censored = 0usize;
    let mut excluded_zero_pairs = 0usize;
    let mut unfit_mass_mismatch_ids = Vec::new();
    let mut valid_ab: Vec<(f64, f64)> = Vec::new();

    for p in pairs {
        if let Some(reason) = check_mass_support_mismatch(p, policy.max_relative_mass_diff) {
            if policy.refuse_on_mass_mismatch {
                return Err(QaqcError::DuplicateSupportMismatch {
                    original_sample_id: p.original_sample_id.clone(),
                    duplicate_sample_id: p.duplicate_sample_id.clone(),
                    detail: reason,
                });
            }
            unfit_mass_mismatch_ids.push(p.duplicate_sample_id.clone());
            continue;
        }
        match (p.original, p.duplicate) {
            (
                AssayValue::Measured { mass_fraction: a },
                AssayValue::Measured { mass_fraction: b },
            ) => {
                if !(a.is_finite()
                    && b.is_finite()
                    && (0.0..=1.0).contains(&a)
                    && (0.0..=1.0).contains(&b))
                {
                    return Err(QaqcError::invalid(
                        "duplicate.value",
                        format!(
                            "pair ('{}', '{}'): values ({a}, {b}) outside [0, 1]",
                            p.original_sample_id, p.duplicate_sample_id
                        ),
                    ));
                }
                if a + b == 0.0 {
                    excluded_zero_pairs += 1;
                } else {
                    valid_ab.push((a, b));
                }
            }
            _ => {
                excluded_censored += 1;
            }
        }
    }

    if valid_ab.is_empty() {
        return Err(QaqcError::InsufficientData {
            context: "duplicates.evaluate".to_string(),
            required: 1,
            actual: 0,
        });
    }

    let n = valid_ab.len();
    let mut sum_sq_rel = 0.0;
    let mut pair_hard = Vec::with_capacity(n);
    let mut sum_ampd = 0.0;

    for &(a, b) in &valid_ab {
        let diff = (a - b).abs();
        let sum = a + b;
        let ratio = diff / sum;
        sum_sq_rel += ratio * ratio;
        pair_hard.push(100.0 * ratio);
        sum_ampd += 200.0 * ratio;
    }

    let relative_variance = (2.0 / (n as f64)) * sum_sq_rel;
    let cv_avr_pct = 100.0 * det::sqrt(relative_variance);
    let sqrt2 = det::sqrt(2.0);
    let hard_from_cv_pct = cv_avr_pct / sqrt2;
    let ampd_from_cv_pct = sqrt2 * cv_avr_pct;

    pair_hard.sort_by(f64::total_cmp);
    let median_pair_hard_pct = sorted_quantile(&pair_hard, 0.50);
    let p90_pair_hard_pct = sorted_quantile(&pair_hard, 0.90);
    let mean_pair_ampd_pct = sum_ampd / (n as f64);

    let thompson_howarth = if n >= THOMPSON_HOWARTH_MIN_PAIRS {
        compute_thompson_howarth(&valid_ab)
    } else {
        None
    };

    unfit_mass_mismatch_ids.sort();

    Ok(DuplicatePrecisionReport {
        input_pairs: pairs.len(),
        valid_pairs: n,
        excluded_censored,
        excluded_zero_pairs,
        unfit_mass_mismatch_ids,
        cv_avr_pct,
        relative_variance,
        hard_from_cv_pct,
        ampd_from_cv_pct,
        median_pair_hard_pct,
        p90_pair_hard_pct,
        mean_pair_ampd_pct,
        thompson_howarth,
    })
}

fn check_mass_support_mismatch(p: &DuplicatePair, max_rel_diff: f64) -> Option<String> {
    if let (Some(orig_t), Some(dup_t)) = (p.original_sample_type, p.duplicate_sample_type)
        && orig_t != dup_t
    {
        return Some(format!(
            "duplicate sample type '{}' does not match original '{}' (unequal support mass; E374)",
            dup_t.code(),
            orig_t.code()
        ));
    }
    if let (Some(orig_m), Some(dup_m)) = (p.original_mass_g, p.duplicate_mass_g) {
        if !(orig_m.is_finite() && dup_m.is_finite() && orig_m > 0.0 && dup_m > 0.0) {
            return Some(format!(
                "invalid sample masses orig={orig_m} g, dup={dup_m} g"
            ));
        }
        let rel = (dup_m - orig_m).abs() / orig_m;
        if rel > max_rel_diff {
            return Some(format!(
                "mass mismatch {:.1}% exceeds tolerance {:.1}% (orig={orig_m} g, dup={dup_m} g; E374)",
                100.0 * rel,
                100.0 * max_rel_diff
            ));
        }
    }
    None
}

fn compute_thompson_howarth(pairs: &[(f64, f64)]) -> Option<ThompsonHowarthEstimate> {
    if pairs.len() < THOMPSON_HOWARTH_MIN_PAIRS {
        return None;
    }
    // Sort pairs ascending by pair mean (a + b)/2 and partition into groups of 11 (Thompson & Howarth 1978).
    let mut ordered: Vec<(f64, f64)> = pairs
        .iter()
        .map(|&(a, b)| (0.5 * (a + b), (a - b).abs()))
        .collect();
    ordered.sort_by(|x, y| x.0.total_cmp(&y.0).then_with(|| x.1.total_cmp(&y.1)));

    const GROUP_SIZE: usize = 11;
    // For |a - b| where a, b ~ N(c, s^2), median(|a - b|) = sqrt(2) * Phi^-1(0.75) * s = 0.95387255 * s.
    const MEDIAN_DIFF_TO_SD: f64 = 0.953_872_552_408_939_7;

    let mut xs = Vec::new();
    let mut ys = Vec::new();
    for chunk in ordered.as_chunks::<GROUP_SIZE>().0 {
        let mean_c = chunk.iter().map(|&(m, _)| m).sum::<f64>() / (GROUP_SIZE as f64);
        let mut diffs: Vec<f64> = chunk.iter().map(|&(_, d)| d).collect();
        diffs.sort_by(f64::total_cmp);
        let med_diff = diffs[GROUP_SIZE / 2];
        xs.push(mean_c);
        ys.push(med_diff / MEDIAN_DIFF_TO_SD);
    }
    if xs.len() < 2 {
        return None;
    }

    let n_bins = xs.len();
    let x_bar = xs.iter().sum::<f64>() / (n_bins as f64);
    let y_bar = ys.iter().sum::<f64>() / (n_bins as f64);
    let sxx: f64 = xs.iter().map(|&x| (x - x_bar) * (x - x_bar)).sum();
    let sxy: f64 = xs
        .iter()
        .zip(&ys)
        .map(|(&x, &y)| (x - x_bar) * (y - y_bar))
        .sum();
    let (slope_k, intercept_s0) = if sxx > 0.0 {
        let k = sxy / sxx;
        (k, y_bar - k * x_bar)
    } else {
        (0.0, y_bar)
    };

    let all_means: Vec<f64> = ordered.iter().map(|&(m, _)| m).collect();
    let c_med = sorted_quantile(&all_means, 0.50);
    let relative_sd_pct_at_median = if c_med > 0.0 {
        100.0 * (intercept_s0 + slope_k * c_med) / c_med
    } else {
        0.0
    };

    Some(ThompsonHowarthEstimate {
        n_pairs: pairs.len(),
        n_bins,
        intercept_s0,
        slope_k,
        relative_sd_pct_at_median,
        caveat: THOMPSON_HOWARTH_CAVEAT.to_string(),
    })
}

/// Identifiability status of duplicate stage variance decomposition (plan §12.3, C4 P12).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StageDecompositionStatus {
    /// All present stages decomposed cleanly with non-negative incremental variances.
    Identified,
    /// No valid field duplicates were provided (`C4 P12`, Porgera): stage decomposition is not identifiable.
    FieldDuplicateAbsent,
    /// An upstream stage's relative variance was smaller than a downstream stage's (non-negative
    /// incremental variance violated; reported verbatim without clamping).
    NonMonotoneStageVariance,
}

/// Decomposition of duplicate relative variance across field, coarse-reject, and pulp stages.
#[derive(Clone, Debug, PartialEq)]
pub struct StageVarianceDecomposition {
    /// Identifiability status.
    pub status: StageDecompositionStatus,
    /// Per-stage precision reports present.
    pub by_stage: BTreeMap<DuplicateStage, DuplicatePrecisionReport>,
    /// Pulp (analytical) relative variance (`σ²_pulp`), if pulp duplicates exist.
    pub analytical_rel_var: Option<f64>,
    /// Coarse-reject incremental variance (`σ²_coarse - σ²_pulp`), if both exist.
    pub prep_incremental_rel_var: Option<f64>,
    /// Field incremental variance (`σ²_field - σ²_coarse`, or `σ²_field - σ²_pulp`), if identifiable.
    pub field_incremental_rel_var: Option<f64>,
}

/// Decomposes duplicate pairs across stages (`Field`, `CoarseReject`, `Pulp`).
///
/// Sets [`StageDecompositionStatus::FieldDuplicateAbsent`] when no field duplicates exist (C4 P12)
/// and reports negative incremental residuals verbatim without clamping.
pub fn decompose_duplicate_stages(
    pairs: &[DuplicatePair],
    policy: &DuplicatePolicy,
) -> Result<StageVarianceDecomposition, QaqcError> {
    let mut grouped: BTreeMap<DuplicateStage, Vec<DuplicatePair>> = BTreeMap::new();
    for p in pairs {
        grouped.entry(p.stage).or_default().push(p.clone());
    }

    let mut by_stage = BTreeMap::new();
    for (stage, stage_pairs) in grouped {
        match evaluate_duplicates(&stage_pairs, policy) {
            Ok(rep) => {
                by_stage.insert(stage, rep);
            }
            Err(QaqcError::InsufficientData { .. }) => {}
            Err(err) => return Err(err),
        }
    }

    let pulp_v = by_stage
        .get(&DuplicateStage::Pulp)
        .map(|r| r.relative_variance);
    let coarse_v = by_stage
        .get(&DuplicateStage::CoarseReject)
        .map(|r| r.relative_variance);
    let field_v = by_stage
        .get(&DuplicateStage::Field)
        .map(|r| r.relative_variance);

    let prep_inc = match (coarse_v, pulp_v) {
        (Some(c), Some(p)) => Some(c - p),
        _ => None,
    };
    let field_inc = match (field_v, coarse_v, pulp_v) {
        (Some(f), Some(c), _) => Some(f - c),
        (Some(f), None, Some(p)) => Some(f - p),
        (Some(f), None, None) => Some(f),
        (None, _, _) => None,
    };

    let status = if field_v.is_none() {
        StageDecompositionStatus::FieldDuplicateAbsent
    } else if prep_inc.is_some_and(|v| v < 0.0) || field_inc.is_some_and(|v| v < 0.0) {
        StageDecompositionStatus::NonMonotoneStageVariance
    } else {
        StageDecompositionStatus::Identified
    };

    Ok(StageVarianceDecomposition {
        status,
        by_stage,
        analytical_rel_var: pulp_v,
        prep_incremental_rel_var: prep_inc,
        field_incremental_rel_var: field_inc,
    })
}

/// Reference duplicate precision levels (Abzalov 2008 / AMG Table 10.3, `E372`, `FX-4`).
/// Registered strictly as `PolicyDefault`, never as compliance.
#[derive(Clone, Debug, PartialEq)]
pub struct PrecisionReferenceLevel {
    /// Deposit and analyte description.
    pub deposit_and_analyte: &'static str,
    /// Stage (`CoarseReject` or `Pulp`).
    pub stage: DuplicateStage,
    /// Best-practice CV_AVR% level.
    pub best_cv_avr_pct: f64,
    /// Acceptable CV_AVR% level.
    pub acceptable_cv_avr_pct: f64,
    /// Provenance notice confirming `PolicyDefault` status.
    pub basis: &'static str,
}

/// Registered `PolicyDefault` duplicate precision reference table (`E372`, `FX-4`, AMG Table 10.3).
#[must_use]
pub fn reference_precision_policy_defaults() -> Vec<PrecisionReferenceLevel> {
    const BASIS: &str =
        "PolicyDefault (Abzalov 2008 / AMG Table 10.3, E372, FX-4; never compliance)";
    vec![
        PrecisionReferenceLevel {
            deposit_and_analyte: "Gold (coarse to medium)",
            stage: DuplicateStage::CoarseReject,
            best_cv_avr_pct: 20.0,
            acceptable_cv_avr_pct: 30.0,
            basis: BASIS,
        },
        PrecisionReferenceLevel {
            deposit_and_analyte: "Gold (coarse to medium)",
            stage: DuplicateStage::Pulp,
            best_cv_avr_pct: 10.0,
            acceptable_cv_avr_pct: 20.0,
            basis: BASIS,
        },
        PrecisionReferenceLevel {
            deposit_and_analyte: "Cu-Mo-Au porphyry (Cu)",
            stage: DuplicateStage::CoarseReject,
            best_cv_avr_pct: 5.0,
            acceptable_cv_avr_pct: 10.0,
            basis: BASIS,
        },
        PrecisionReferenceLevel {
            deposit_and_analyte: "Cu-Mo-Au porphyry (Cu)",
            stage: DuplicateStage::Pulp,
            best_cv_avr_pct: 3.0,
            acceptable_cv_avr_pct: 10.0,
            basis: BASIS,
        },
        PrecisionReferenceLevel {
            deposit_and_analyte: "Cu-Mo-Au porphyry (Au)",
            stage: DuplicateStage::CoarseReject,
            best_cv_avr_pct: 10.0,
            acceptable_cv_avr_pct: 15.0,
            basis: BASIS,
        },
        PrecisionReferenceLevel {
            deposit_and_analyte: "Cu-Mo-Au porphyry (Au)",
            stage: DuplicateStage::Pulp,
            best_cv_avr_pct: 5.0,
            acceptable_cv_avr_pct: 10.0,
            basis: BASIS,
        },
        PrecisionReferenceLevel {
            deposit_and_analyte: "Cu-Au-Fe skarn / IOCG (Cu)",
            stage: DuplicateStage::CoarseReject,
            best_cv_avr_pct: 7.5,
            acceptable_cv_avr_pct: 15.0,
            basis: BASIS,
        },
        PrecisionReferenceLevel {
            deposit_and_analyte: "Cu-Au-Fe skarn / IOCG (Au)",
            stage: DuplicateStage::CoarseReject,
            best_cv_avr_pct: 15.0,
            acceptable_cv_avr_pct: 25.0,
            basis: BASIS,
        },
    ]
}

// ============================================================================
// Gy's Fundamental Sampling Error (FSE) & Calibrated Nomogram (BD-284, §12.3)
// ============================================================================

/// Parameters for Gy's general FSE form: `σ²_FSE = (1/M_S - 1/M_L) * c * f * g * l * d^3`
/// (`d` in cm, `M_S < M_L` in g, `c` in g/cm³; plan §12.3, E359–E363).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GyGeneralInput {
    /// Sample mass `M_S` (g).
    pub sample_mass_g: f64,
    /// Lot mass `M_L` (g); strictly greater than `M_S`.
    pub lot_mass_g: f64,
    /// Mineralogical factor `c` (g/cm³).
    pub mineralogical_c_g_per_cm3: f64,
    /// Shape factor `f` (dimensionless, `(0, 1]`, typically ~0.5).
    pub shape_f: f64,
    /// Granulometric factor `g` (dimensionless, `(0, 1]`, typically ~0.25).
    pub granulometric_g: f64,
    /// Liberation factor `l` (dimensionless, `(0, 1]`).
    pub liberation_l: f64,
    /// Nominal top particle size `d` (cm).
    pub nominal_diameter_cm: f64,
}

/// Evaluates Gy's general FSE variance `σ²_FSE = (1/M_S - 1/M_L) * c * f * g * l * d^3`.
pub fn gy_general_fse_variance(input: &GyGeneralInput) -> Result<f64, QaqcError> {
    if !(input.sample_mass_g.is_finite()
        && input.lot_mass_g.is_finite()
        && input.sample_mass_g > 0.0
        && input.lot_mass_g > input.sample_mass_g)
    {
        return Err(QaqcError::invalid(
            "gy.mass",
            format!(
                "require 0 < sample_mass_g ({}) < lot_mass_g ({})",
                input.sample_mass_g, input.lot_mass_g
            ),
        ));
    }
    if !(input.mineralogical_c_g_per_cm3.is_finite() && input.mineralogical_c_g_per_cm3 > 0.0) {
        return Err(QaqcError::invalid(
            "gy.c",
            format!("must be > 0, got {}", input.mineralogical_c_g_per_cm3),
        ));
    }
    for (name, val) in [
        ("gy.f", input.shape_f),
        ("gy.g", input.granulometric_g),
        ("gy.l", input.liberation_l),
    ] {
        if !(val.is_finite() && val > 0.0 && val <= 1.0) {
            return Err(QaqcError::invalid(
                name,
                format!("must lie in (0, 1], got {val}"),
            ));
        }
    }
    if !(input.nominal_diameter_cm.is_finite() && input.nominal_diameter_cm > 0.0) {
        return Err(QaqcError::invalid(
            "gy.d_cm",
            format!("must be > 0, got {}", input.nominal_diameter_cm),
        ));
    }
    let d3 = input.nominal_diameter_cm * input.nominal_diameter_cm * input.nominal_diameter_cm;
    let var = (1.0 / input.sample_mass_g - 1.0 / input.lot_mass_g)
        * input.mineralogical_c_g_per_cm3
        * input.shape_f
        * input.granulometric_g
        * input.liberation_l
        * d3;
    if !var.is_finite() {
        return Err(QaqcError::NumericalFailure {
            context: "gy.general_fse".to_string(),
            detail: "FSE relative variance overflowed to non-finite value".to_string(),
        });
    }
    Ok(var)
}

/// Calibration provenance of Gy's `(K, α)` constants (plan §12.3, E361–E363).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GyCalibrationMethod {
    /// Minimum Sampling Theory Experiment (MSTE).
    Mste,
    /// Abzalov / Pitard "30-pieces" test (`FX-1`, E361).
    ThirtyPieces,
    /// Heterogeneity test (HT).
    HeterogeneityTest,
    /// Calibrated empirically from drill-core duplicate data.
    DrillData,
    /// Uncalibrated assumption (e.g. François-Bongarçon 1993 default `K = 470, α = 1.5`, E363).
    /// Renders any derived nomogram `Speculative` (`E362`).
    Assumed {
        /// Citation or rationale for the assumed constants.
        source: String,
    },
}

/// Calibrated Gy sampling constants with provenance and grade validity range (`E359`, `E361–E363`).
#[derive(Clone, Debug, PartialEq)]
pub struct GyCalibratedConstants {
    /// Sampling constant `K` (`g · cm^-α`).
    pub k: f64,
    /// Granulometric exponent `α` (typically `1.5`, in `(0, 3]`).
    pub alpha: f64,
    /// How `K` and `α` were obtained.
    pub method: GyCalibrationMethod,
    /// Reference grade at which `K` was calibrated (mass fraction or consistent positive grade unit).
    pub calibration_grade: f64,
    /// Valid lower grade bound for this calibration (`E359`).
    pub valid_grade_min: f64,
    /// Valid upper grade bound for this calibration (`E359`).
    pub valid_grade_max: f64,
}

/// Calibration grade validity window for [`GyCalibratedConstants`] (`E359`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GyGradeWindow {
    /// Reference grade at which `K` was calibrated.
    pub calibration_grade: f64,
    /// Valid lower grade bound.
    pub valid_grade_min: f64,
    /// Valid upper grade bound.
    pub valid_grade_max: f64,
}

impl GyCalibratedConstants {
    /// François-Bongarçon (1993) uncalibrated default (`K = 470, α = 1.5`, `E363`), tagged `Assumed`.
    #[must_use]
    pub fn francois_bongarcon_assumed(window: GyGradeWindow) -> Self {
        Self {
            k: GY_DEFAULT_K,
            alpha: GY_DEFAULT_ALPHA,
            method: GyCalibrationMethod::Assumed {
                source: "Francois-Bongarcon 1993 default (E363; uncalibrated)".to_string(),
            },
            calibration_grade: window.calibration_grade,
            valid_grade_min: window.valid_grade_min,
            valid_grade_max: window.valid_grade_max,
        }
    }

    /// Validates Gy constants and grade validity bounds.
    pub fn validate(&self) -> Result<(), QaqcError> {
        if !(self.k.is_finite() && self.k > 0.0) {
            return Err(QaqcError::invalid(
                "gy.k",
                format!("must be > 0, got {}", self.k),
            ));
        }
        if !(self.alpha.is_finite() && self.alpha > 0.0 && self.alpha <= 3.0) {
            return Err(QaqcError::invalid(
                "gy.alpha",
                format!("must lie in (0, 3], got {}", self.alpha),
            ));
        }
        if !(self.calibration_grade.is_finite()
            && self.calibration_grade > 0.0
            && self.valid_grade_min.is_finite()
            && self.valid_grade_max.is_finite()
            && self.valid_grade_min > 0.0
            && self.valid_grade_min <= self.calibration_grade
            && self.calibration_grade <= self.valid_grade_max)
        {
            return Err(QaqcError::invalid(
                "gy.calibration_grade",
                format!(
                    "require 0 < valid_grade_min ({}) <= calibration_grade ({}) <= valid_grade_max ({})",
                    self.valid_grade_min, self.calibration_grade, self.valid_grade_max
                ),
            ));
        }
        Ok(())
    }
}

/// Input to calibrate `K` from a "30-pieces" test (`FX-1`, E361, AMG PDF 141).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ThirtyPiecesInput {
    /// Observed relative variance of the 30 pieces (e.g. `9.41` in FX-1).
    pub relative_variance: f64,
    /// Mean piece mass in grams (e.g. `2889.0` g in FX-1).
    pub mean_piece_mass_g: f64,
    /// Either equivalent sphere diameter (cm) to convert via `d_N = d_sphere / 0.25^(1/3)`,
    /// or direct nominal diameter `d_N` (cm).
    pub diameter: ThirtyPiecesDiameter,
    /// Exponent `α` (typically `1.5`).
    pub alpha: f64,
    /// Grade at which the 30-pieces test was calibrated.
    pub calibration_grade: f64,
    /// Lower valid grade bound (`E359`).
    pub valid_grade_min: f64,
    /// Upper valid grade bound (`E359`).
    pub valid_grade_max: f64,
}

/// Diameter input convention for [`calibrate_thirty_pieces`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ThirtyPiecesDiameter {
    /// Equivalent sphere diameter `d_s` (cm), converted via `d_N = d_s / 0.25^(1/3)` (FX-1).
    EquivalentSphereCm(f64),
    /// Direct nominal diameter `d_N` (cm).
    NominalCm(f64),
}

/// Result of calibrating Gy's `K` from a 30-pieces experiment (`FX-1`).
#[derive(Clone, Debug, PartialEq)]
pub struct ThirtyPiecesCalibration {
    /// Nominal particle diameter `d_N` (cm).
    pub nominal_diameter_cm: f64,
    /// Calibrated constants with `GyCalibrationMethod::ThirtyPieces`.
    pub constants: GyCalibratedConstants,
}

/// Calibrates Gy's `K = relative_variance * mass_g / d_N^α` from a 30-pieces test (`FX-1`, E361).
pub fn calibrate_thirty_pieces(
    input: &ThirtyPiecesInput,
) -> Result<ThirtyPiecesCalibration, QaqcError> {
    if !(input.relative_variance.is_finite() && input.relative_variance > 0.0) {
        return Err(QaqcError::invalid(
            "thirty_pieces.relative_variance",
            format!("must be > 0, got {}", input.relative_variance),
        ));
    }
    if !(input.mean_piece_mass_g.is_finite() && input.mean_piece_mass_g > 0.0) {
        return Err(QaqcError::invalid(
            "thirty_pieces.mean_piece_mass_g",
            format!("must be > 0, got {}", input.mean_piece_mass_g),
        ));
    }
    if !(input.alpha.is_finite() && input.alpha > 0.0 && input.alpha <= 3.0) {
        return Err(QaqcError::invalid(
            "thirty_pieces.alpha",
            format!("must lie in (0, 3], got {}", input.alpha),
        ));
    }
    let nominal_diameter_cm = match input.diameter {
        ThirtyPiecesDiameter::EquivalentSphereCm(d_s) => {
            if !(d_s.is_finite() && d_s > 0.0) {
                return Err(QaqcError::invalid(
                    "thirty_pieces.sphere_diameter_cm",
                    format!("must be > 0, got {d_s}"),
                ));
            }
            d_s / pow_pos(0.25, 1.0 / 3.0)
        }
        ThirtyPiecesDiameter::NominalCm(d_n) => {
            if !(d_n.is_finite() && d_n > 0.0) {
                return Err(QaqcError::invalid(
                    "thirty_pieces.nominal_diameter_cm",
                    format!("must be > 0, got {d_n}"),
                ));
            }
            d_n
        }
    };

    let k = input.relative_variance * input.mean_piece_mass_g
        / pow_pos(nominal_diameter_cm, input.alpha);
    let constants = GyCalibratedConstants {
        k,
        alpha: input.alpha,
        method: GyCalibrationMethod::ThirtyPieces,
        calibration_grade: input.calibration_grade,
        valid_grade_min: input.valid_grade_min,
        valid_grade_max: input.valid_grade_max,
    };
    constants.validate()?;

    Ok(ThirtyPiecesCalibration {
        nominal_diameter_cm,
        constants,
    })
}

/// One mass-reduction stage of a sampling protocol (e.g. crush → riffle split → pulverize → fire assay).
#[derive(Clone, Debug, PartialEq)]
pub struct GySamplingStage {
    /// Human-readable stage name (e.g. `"crush_2mm_split"`).
    pub stage_name: String,
    /// Nominal top particle size `d_N` (cm) entering the split.
    pub nominal_diameter_cm: f64,
    /// Input lot mass `M_L` (g).
    pub lot_mass_g: f64,
    /// Output sample mass `M_S` (g), strictly less than `lot_mass_g`.
    pub sample_mass_g: f64,
    /// Number of increments or cutter passes forming the split (flagged when `< 30`).
    pub increments: usize,
}

/// Request to evaluate a multi-stage Gy sampling protocol nomogram.
#[derive(Clone, Debug, PartialEq)]
pub struct GyNomogramRequest {
    /// Calibrated (or assumed) Gy constants with calibration grade range.
    pub constants: GyCalibratedConstants,
    /// Grade level at which the protocol is being evaluated (checked against calibration range, `E359`).
    pub evaluation_grade: f64,
    /// Policy acceptance safety line on relative variance (default `0.01`, `E360`).
    pub safety_line_rel_var: f64,
    /// Ordered sampling stages.
    pub stages: Vec<GySamplingStage>,
}

/// Evidence class of a Gy nomogram (plan §12.3, E362–E363).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NomogramEvidence {
    /// Calibrated from experimental/drill data (`Mste`, `ThirtyPieces`, `HeterogeneityTest`, `DrillData`).
    Derived,
    /// Built from uncalibrated `Assumed` constants (`K = 470, α = 1.5`).
    Speculative,
}

/// Validity status of a Gy nomogram at the queried grade (`E359`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NomogramValidity {
    /// Evaluation grade falls within `[valid_grade_min, valid_grade_max]`.
    WithinValidity,
    /// Evaluation grade falls outside the calibration grade band (`OutsideValidity`, `E359`).
    OutsideValidity,
}

/// Per-stage FSE evaluation inside a [`GyNomogramReport`].
#[derive(Clone, Debug, PartialEq)]
pub struct GyStageOutcome {
    /// Stage name.
    pub stage_name: String,
    /// Stage FSE relative variance `K * d_N^α * (1/M_S - 1/M_L)`.
    pub fse_rel_var: f64,
    /// Stage relative standard error `100 * sqrt(fse_rel_var)` (%).
    pub fse_rel_sd_pct: f64,
    /// `true` when `fse_rel_var > safety_line_rel_var`.
    pub exceeds_safety_line: bool,
    /// `true` when `increments < 30` (AMG PDF 144).
    pub insufficient_increments: bool,
}

/// Complete Gy sampling nomogram report (`BD-284`, plan §12.3).
#[derive(Clone, Debug, PartialEq)]
pub struct GyNomogramReport {
    /// `Derived` if calibrated, `Speculative` if `Assumed` (`E362–E363`).
    pub evidence: NomogramEvidence,
    /// `WithinValidity` or `OutsideValidity` off the calibration grade (`E359`).
    pub validity: NomogramValidity,
    /// Calibration method provenance.
    pub calibration_method: GyCalibrationMethod,
    /// Policy safety line used (default `0.01`).
    pub safety_line_rel_var: f64,
    /// Per-stage FSE breakdown.
    pub stages: Vec<GyStageOutcome>,
    /// Total cumulative FSE relative variance across stages (`Σ σ²_FSE,i`).
    pub total_fse_rel_var: f64,
    /// Total cumulative FSE relative standard error `100 * sqrt(Σ σ²_FSE,i)` (%).
    pub total_fse_rel_sd_pct: f64,
    /// `true` iff total FSE variance is within the safety line AND no stage has `< 30` increments
    /// AND `validity == WithinValidity`.
    pub protocol_within_policy: bool,
}

/// Evaluates a Gy calibrated sampling protocol across one or more splitting stages (`BD-284`).
pub fn evaluate_gy_nomogram(request: &GyNomogramRequest) -> Result<GyNomogramReport, QaqcError> {
    request.constants.validate()?;
    if !(request.evaluation_grade.is_finite() && request.evaluation_grade > 0.0) {
        return Err(QaqcError::invalid(
            "gy.evaluation_grade",
            format!("must be > 0, got {}", request.evaluation_grade),
        ));
    }
    if !(request.safety_line_rel_var.is_finite() && request.safety_line_rel_var > 0.0) {
        return Err(QaqcError::invalid(
            "gy.safety_line_rel_var",
            format!("must be > 0, got {}", request.safety_line_rel_var),
        ));
    }
    if request.stages.is_empty() {
        return Err(QaqcError::InsufficientData {
            context: "gy.nomogram.stages".to_string(),
            required: 1,
            actual: 0,
        });
    }

    let evidence = match request.constants.method {
        GyCalibrationMethod::Assumed { .. } => NomogramEvidence::Speculative,
        _ => NomogramEvidence::Derived,
    };
    let validity = if request.evaluation_grade >= request.constants.valid_grade_min
        && request.evaluation_grade <= request.constants.valid_grade_max
    {
        NomogramValidity::WithinValidity
    } else {
        NomogramValidity::OutsideValidity
    };

    let mut stage_outcomes = Vec::with_capacity(request.stages.len());
    let mut total_fse_rel_var = 0.0;
    let mut any_increment_flag = false;

    for st in &request.stages {
        if st.stage_name.trim().is_empty() {
            return Err(QaqcError::invalid("gy.stage.name", "must not be empty"));
        }
        if !(st.nominal_diameter_cm.is_finite() && st.nominal_diameter_cm > 0.0) {
            return Err(QaqcError::invalid(
                "gy.stage.nominal_diameter_cm",
                format!("stage '{}': diameter must be > 0", st.stage_name),
            ));
        }
        if !(st.sample_mass_g.is_finite()
            && st.lot_mass_g.is_finite()
            && st.sample_mass_g > 0.0
            && st.lot_mass_g > st.sample_mass_g)
        {
            return Err(QaqcError::invalid(
                "gy.stage.mass",
                format!(
                    "stage '{}': require 0 < sample_mass_g ({}) < lot_mass_g ({})",
                    st.stage_name, st.sample_mass_g, st.lot_mass_g
                ),
            ));
        }
        let var = request.constants.k
            * pow_pos(st.nominal_diameter_cm, request.constants.alpha)
            * (1.0 / st.sample_mass_g - 1.0 / st.lot_mass_g);
        if !var.is_finite() {
            return Err(QaqcError::NumericalFailure {
                context: format!("gy.stage '{}'", st.stage_name),
                detail: "stage FSE relative variance overflowed to non-finite value".to_string(),
            });
        }
        let insufficient_increments = st.increments < GY_MIN_INCREMENTS;
        if insufficient_increments {
            any_increment_flag = true;
        }
        total_fse_rel_var += var;
        if !total_fse_rel_var.is_finite() {
            return Err(QaqcError::NumericalFailure {
                context: "gy.nomogram".to_string(),
                detail: "cumulative FSE relative variance overflowed to non-finite value"
                    .to_string(),
            });
        }
        stage_outcomes.push(GyStageOutcome {
            stage_name: st.stage_name.clone(),
            fse_rel_var: var,
            fse_rel_sd_pct: 100.0 * det::sqrt(var),
            exceeds_safety_line: var > request.safety_line_rel_var,
            insufficient_increments,
        });
    }

    let protocol_within_policy = validity == NomogramValidity::WithinValidity
        && total_fse_rel_var <= request.safety_line_rel_var
        && !any_increment_flag;

    Ok(GyNomogramReport {
        evidence,
        validity,
        calibration_method: request.constants.method.clone(),
        safety_line_rel_var: request.safety_line_rel_var,
        stages: stage_outcomes,
        total_fse_rel_var,
        total_fse_rel_sd_pct: 100.0 * det::sqrt(total_fse_rel_var),
        protocol_within_policy,
    })
}

// ============================================================================
// Nugget Split -> Protocol-vs-Infill Information Option (BD-284, E390, §12.3)
// ============================================================================

/// Information-gathering option proposed to Helm for EVSI pricing (`E390`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum InformationOption {
    /// Additional infill drilling to reduce geological spacing uncertainty.
    InfillDrilling,
    /// Improve the sampling/preparation protocol (larger split mass, finer crush) because
    /// sampling variance dominates the variogram nugget (`E390`).
    ImproveSamplingProtocol,
}

/// Input to split the pairwise-relative-variogram nugget into sampling vs geological components.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NuggetSplitInput {
    /// Duplicate relative variance at composite support (`σ²_samp >= 0`).
    pub duplicate_rel_var_at_composite: f64,
    /// Pairwise-relative-variogram total nugget (`C_0 > 0`).
    pub variogram_relative_nugget: f64,
    /// Policy threshold share above which `ImproveSamplingProtocol` enters Helm's options (e.g. `0.50`).
    pub policy_share_threshold: f64,
}

/// Output of the nugget split feeding Helm's EVSI options (`E390`, plan §12.3).
#[derive(Clone, Debug, PartialEq)]
pub struct NuggetSplitDecision {
    /// Sampling share of the total nugget (`σ²_samp / C_0`, in `[0, 1]`).
    pub sampling_share: f64,
    /// Geological share of the total nugget (`1 - sampling_share`).
    pub geological_share: f64,
    /// Implied geological nugget (`C_0 - σ²_samp >= 0`).
    pub geological_nugget: f64,
    /// Whether `sampling_share > policy_share_threshold`.
    pub sampling_dominates: bool,
    /// Candidate information options forwarded to Helm for equal-basis EVSI pricing.
    pub helm_options: Vec<InformationOption>,
}

/// Splits the pairwise-relative-variogram nugget into geological vs sampling variance (`BD-284`, `E390`).
///
/// Refuses (`FMINE-REF-104` `NonIdentifiable`) when duplicate variance exceeds the total variogram
/// nugget rather than clamping the geological nugget to zero.
pub fn evaluate_nugget_split(input: &NuggetSplitInput) -> Result<NuggetSplitDecision, QaqcError> {
    if !(input.duplicate_rel_var_at_composite.is_finite()
        && input.duplicate_rel_var_at_composite >= 0.0)
    {
        return Err(QaqcError::invalid(
            "nugget_split.duplicate_rel_var_at_composite",
            format!("must be >= 0, got {}", input.duplicate_rel_var_at_composite),
        ));
    }
    if !(input.variogram_relative_nugget.is_finite() && input.variogram_relative_nugget > 0.0) {
        return Err(QaqcError::invalid(
            "nugget_split.variogram_relative_nugget",
            format!("must be > 0, got {}", input.variogram_relative_nugget),
        ));
    }
    if !(input.policy_share_threshold.is_finite()
        && input.policy_share_threshold > 0.0
        && input.policy_share_threshold < 1.0)
    {
        return Err(QaqcError::invalid(
            "nugget_split.policy_share_threshold",
            format!("must lie in (0, 1), got {}", input.policy_share_threshold),
        ));
    }
    if input.duplicate_rel_var_at_composite > input.variogram_relative_nugget {
        return Err(QaqcError::NonIdentifiable {
            detail: format!(
                "composite duplicate relative variance ({}) exceeds pairwise-relative variogram nugget ({}); geological nugget cannot be negative",
                input.duplicate_rel_var_at_composite, input.variogram_relative_nugget
            ),
        });
    }

    let sampling_share = input.duplicate_rel_var_at_composite / input.variogram_relative_nugget;
    let geological_nugget = input.variogram_relative_nugget - input.duplicate_rel_var_at_composite;
    let geological_share = 1.0 - sampling_share;
    let sampling_dominates = sampling_share > input.policy_share_threshold;

    let helm_options = if sampling_dominates {
        vec![
            InformationOption::ImproveSamplingProtocol,
            InformationOption::InfillDrilling,
        ]
    } else {
        vec![InformationOption::InfillDrilling]
    };

    Ok(NuggetSplitDecision {
        sampling_share,
        geological_share,
        geological_nugget,
        sampling_dominates,
        helm_options,
    })
}
