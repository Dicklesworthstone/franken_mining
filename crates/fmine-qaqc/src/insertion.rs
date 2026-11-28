//! QC insertion rate accounting, blank evaluation (including versioned site "and" vs "either/or"
//! carryover rules), control-absent period detection, per-lab disaggregation, and batch failure
//! cascade (`BD-167`, `BD-282`; plan §12.1, §12.4, E165–E168, E226, E373, C2 §4.5b, PO-CORE-3/4).
//! Determinism: BitwisePortable (ordered `BTreeMap`/`BTreeSet` and IEEE-754 arithmetic only).

use std::collections::{BTreeMap, BTreeSet};

use fmine_drill::records::{
    AssayMethod, AssayResult, AssayValue, BlankStage, DuplicateStage, QcInsertion, QcKind, Sample,
    SampleType,
};

use crate::crm::{
    CrmBatchEvaluation, CrmBatchInput, CrmCertificateSpec, CrmObservation, SiteCrmRule,
    evaluate_crm_batch,
};
use crate::error::QaqcError;

/// Explicit denominator for QC insertion percentages (E165, E226).
///
/// Kibali Table 11-2 mixed denominators (INFERENCE, E226); every rate here names its denominator
/// explicitly alongside its raw numerator and denominator counts.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum InsertionDenominator {
    /// QC insertions divided by routine (drilled, non-QC) samples.
    RoutineSamples,
    /// QC insertions divided by all submitted samples (routine + QC).
    AllSubmittedSamples,
}

/// Provenance of insertion targets (plan §12.1, E373, FX-5).
///
/// Textbook defaults are `PolicyDefault`, never compliance.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TargetBasis {
    /// Textbook recommendation (Abzalov AMG PDF 169 / E373 / FX-5); never compliance.
    PolicyDefault {
        /// Citation string.
        source: String,
    },
    /// Program target explicitly set by a Qualified Person policy.
    QpPolicy {
        /// Policy identifier.
        policy_id: String,
        /// Policy version.
        version: String,
    },
}

/// Target range `[min_pct, max_pct]` for one QC control type.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RateBandPct {
    /// Minimum target insertion percentage (e.g. `3.0` for 3%).
    pub min_pct: f64,
    /// Optional upper target percentage (e.g. `5.0` for 3–5%; `None` for lower-bounded like `>= 5%`).
    pub max_pct: Option<f64>,
}

/// Target insertion-rate policy across QC control types (plan §12.1, E373, FX-5).
#[derive(Clone, Debug, PartialEq)]
pub struct InsertionTargetSpec {
    /// Whether targets are textbook `PolicyDefault` or an explicit `QpPolicy`.
    pub basis: TargetBasis,
    /// Denominator convention used when comparing achieved vs target.
    pub denominator: InsertionDenominator,
    /// CRM insertion target (default `3–5%`).
    pub crm_pct: RateBandPct,
    /// Blank insertion target (default `3–5%`).
    pub blank_pct: RateBandPct,
    /// Field duplicate insertion target (default `5–10%`).
    pub field_duplicate_pct: RateBandPct,
    /// Pulp duplicate insertion target (default `3–5%`).
    pub pulp_duplicate_pct: RateBandPct,
    /// External/umpire share of duplicates (default `>= 5%` of duplicates, E373).
    pub external_share_of_duplicates_min_pct: f64,
}

impl InsertionTargetSpec {
    /// Textbook defaults from Abzalov AMG PDF 169 (`E373`, `FX-5`), tagged `PolicyDefault`.
    #[must_use]
    pub fn textbook_policy_default() -> Self {
        Self {
            basis: TargetBasis::PolicyDefault {
                source: "Abzalov AMG PDF 169 (E373, FX-5); PolicyDefault, never compliance"
                    .to_string(),
            },
            denominator: InsertionDenominator::RoutineSamples,
            crm_pct: RateBandPct {
                min_pct: 3.0,
                max_pct: Some(5.0),
            },
            blank_pct: RateBandPct {
                min_pct: 3.0,
                max_pct: Some(5.0),
            },
            field_duplicate_pct: RateBandPct {
                min_pct: 5.0,
                max_pct: Some(10.0),
            },
            pulp_duplicate_pct: RateBandPct {
                min_pct: 3.0,
                max_pct: Some(5.0),
            },
            external_share_of_duplicates_min_pct: 5.0,
        }
    }

    /// Validates all target percentage bands.
    pub fn validate(&self) -> Result<(), QaqcError> {
        for (name, band) in [
            ("crm_pct", self.crm_pct),
            ("blank_pct", self.blank_pct),
            ("field_duplicate_pct", self.field_duplicate_pct),
            ("pulp_duplicate_pct", self.pulp_duplicate_pct),
        ] {
            if !(band.min_pct.is_finite() && (0.0..=100.0).contains(&band.min_pct)) {
                return Err(QaqcError::invalid(
                    name,
                    format!("min_pct {} outside [0, 100]", band.min_pct),
                ));
            }
            if let Some(max) = band.max_pct
                && !(max.is_finite() && max >= band.min_pct && max <= 100.0)
            {
                return Err(QaqcError::invalid(
                    name,
                    format!("max_pct {max} outside [{}, 100]", band.min_pct),
                ));
            }
        }
        if !(self.external_share_of_duplicates_min_pct.is_finite()
            && (0.0..=100.0).contains(&self.external_share_of_duplicates_min_pct))
        {
            return Err(QaqcError::invalid(
                "external_share_of_duplicates_min_pct",
                format!(
                    "{} outside [0, 100]",
                    self.external_share_of_duplicates_min_pct
                ),
            ));
        }
        Ok(())
    }
}

/// One achieved-vs-target insertion rate with explicit numerator and denominator.
#[derive(Clone, Debug, PartialEq)]
pub struct AchievedRate {
    /// Count of inserted controls of this type.
    pub numerator: usize,
    /// Denominator count under [`InsertionDenominator`].
    pub denominator: usize,
    /// Achieved percentage `100 * numerator / denominator`; `None` when `denominator == 0`.
    pub achieved_pct: Option<f64>,
    /// Target band.
    pub target: RateBandPct,
    /// `true` iff `achieved_pct` is present and at or above `target.min_pct`.
    pub meets_min_target: bool,
    /// `true` when `numerator == 0` and `denominator > 0` (`ControlAbsent`).
    pub control_absent: bool,
}

fn rate_row(numerator: usize, denominator: usize, target: RateBandPct) -> AchievedRate {
    let achieved_pct = if denominator > 0 {
        Some(100.0 * (numerator as f64) / (denominator as f64))
    } else {
        None
    };
    let meets_min_target = achieved_pct.is_some_and(|p| p >= target.min_pct);
    AchievedRate {
        numerator,
        denominator,
        achieved_pct,
        target,
        meets_min_target,
        control_absent: numerator == 0 && denominator > 0,
    }
}

/// Summary of achieved QC insertion rates with explicit denominator and target basis.
#[derive(Clone, Debug, PartialEq)]
pub struct InsertionRateSummary {
    /// Basis of the target values (`PolicyDefault` vs `QpPolicy`).
    pub target_basis: TargetBasis,
    /// Denominator convention applied to CRM/blank/duplicate rates.
    pub denominator_kind: InsertionDenominator,
    /// Count of routine (drilled, non-QC-material, non-duplicate, non-umpire) samples.
    pub routine_sample_count: usize,
    /// Count of all submitted samples (`routine + crm + blank + duplicates + umpires`).
    pub all_submitted_sample_count: usize,
    /// CRM insertion rate.
    pub crm: AchievedRate,
    /// Blank insertion rate (coarse + pulp).
    pub blank: AchievedRate,
    /// Coarse blank count.
    pub coarse_blank_count: usize,
    /// Pulp blank count.
    pub pulp_blank_count: usize,
    /// Field duplicate insertion rate.
    pub field_duplicate: AchievedRate,
    /// Coarse-reject duplicate count.
    pub coarse_reject_duplicate_count: usize,
    /// Pulp duplicate insertion rate.
    pub pulp_duplicate: AchievedRate,
    /// External / umpire assays as a percentage of total duplicates (E373).
    pub external_share_of_duplicates: AchievedRate,
    /// Controls completely absent despite routine samples existing (`E168`).
    pub absent_controls: Vec<String>,
}

/// Failure logic of a versioned blank contamination rule (C2 §4.5b, plan §12.4).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BlankFailureMode {
    /// Fails if blank measured value strictly exceeds `dl_multiple * detection_limit`
    /// (requires `detection_limit > 0`).
    AboveMultipleOfDl {
        /// Detection limit (mass fraction).
        detection_limit: f64,
        /// Failure multiplier (e.g. `3.0` or `5.0`).
        dl_multiple: f64,
    },
    /// Carlin 2025 wording (`C p.115`, C2 §4.5b): fails when blank exceeds `abs_threshold`
    /// **AND** exceeds `carryover_fraction * preceding_grade`. A missing preceding grade is
    /// unknown in `[0, 1]`; it never silently removes the carryover condition.
    CarryoverAnd {
        /// Absolute contamination threshold (mass fraction).
        abs_threshold: f64,
        /// Fraction of the preceding routine sample grade (e.g. `0.01` = 1% carryover).
        carryover_fraction: f64,
    },
    /// Turquoise Ridge 2024 wording (`T p.128`, C2 §4.5b): fails when blank exceeds
    /// `abs_threshold` **OR** exceeds `carryover_fraction * preceding_grade`.
    CarryoverOr {
        /// Absolute contamination threshold (mass fraction).
        abs_threshold: f64,
        /// Fraction of the preceding routine sample grade (e.g. `0.01` = 1% carryover).
        carryover_fraction: f64,
    },
}

/// Versioned blank contamination rule registered as policy data (C2 §4.5b).
#[derive(Clone, Debug, PartialEq)]
pub struct SiteBlankRule {
    /// Registry identifier of the blank rule.
    pub rule_id: String,
    /// Rule version string.
    pub version: String,
    /// Source document citation.
    pub source_doc: String,
    /// Failure predicate.
    pub mode: BlankFailureMode,
}

/// Thresholds for carryover blank rules (`C2 §4.5b`), preventing conjunctive/disjunctive mismatch.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BlankCarryoverThresholds {
    /// Absolute contamination threshold (mass fraction).
    pub abs_threshold: f64,
    /// Fraction of the preceding routine sample grade (e.g. `0.01` = 1% carryover).
    pub carryover_fraction: f64,
}

impl SiteBlankRule {
    /// Builds a versioned blank rule from an explicit [`BlankFailureMode`] (no bare `f64` params, L-6).
    #[must_use]
    pub fn new(
        rule_id: impl Into<String>,
        version: impl Into<String>,
        source_doc: impl Into<String>,
        mode: BlankFailureMode,
    ) -> Self {
        Self {
            rule_id: rule_id.into(),
            version: version.into(),
            source_doc: source_doc.into(),
            mode,
        }
    }

    /// Standard `> k * DL` blank failure rule (default `5 * DL` at a 5 ppb = `5e-9` detection limit).
    #[must_use]
    pub fn multiple_of_dl_default() -> Self {
        Self::new(
            "corebox.blank.dl_multiple",
            "1",
            "Corebox policy default (> 5 * DL)",
            BlankFailureMode::AboveMultipleOfDl {
                detection_limit: 5.0e-9,
                dl_multiple: 5.0,
            },
        )
    }

    /// Carlin 2025 blank rule (`C p.115`, C2 §4.5b): conjunctive `"and"` wording.
    #[must_use]
    pub fn carlin_and_v1(thresholds: BlankCarryoverThresholds) -> Self {
        Self::new(
            "site.ngm.carlin.blank",
            "2025-tr-and",
            "Carlin 2025 NI 43-101 p.115 (C2 §4.5b conjunctive 'and')",
            BlankFailureMode::CarryoverAnd {
                abs_threshold: thresholds.abs_threshold,
                carryover_fraction: thresholds.carryover_fraction,
            },
        )
    }

    /// Turquoise Ridge 2024 blank rule (`T p.128`, C2 §4.5b): disjunctive `"either/or"` wording.
    #[must_use]
    pub fn turquoise_ridge_or_v1(thresholds: BlankCarryoverThresholds) -> Self {
        Self::new(
            "site.ngm.turquoise_ridge.blank",
            "2024-tr-or",
            "Turquoise Ridge 2024 NI 43-101 p.128 (C2 §4.5b disjunctive 'either/or')",
            BlankFailureMode::CarryoverOr {
                abs_threshold: thresholds.abs_threshold,
                carryover_fraction: thresholds.carryover_fraction,
            },
        )
    }

    /// Validates rule parameters.
    pub fn validate(&self) -> Result<(), QaqcError> {
        if self.rule_id.trim().is_empty()
            || self.version.trim().is_empty()
            || self.source_doc.trim().is_empty()
        {
            return Err(QaqcError::invalid(
                "blank_rule",
                "rule_id, version, and source_doc must be non-empty",
            ));
        }
        match self.mode {
            BlankFailureMode::AboveMultipleOfDl {
                detection_limit,
                dl_multiple,
            } => {
                if !(detection_limit.is_finite()
                    && detection_limit > 0.0
                    && detection_limit <= 1.0
                    && dl_multiple.is_finite()
                    && dl_multiple > 0.0)
                {
                    return Err(QaqcError::invalid(
                        "blank_rule.dl",
                        format!(
                            "require detection_limit in (0, 1] and dl_multiple > 0, got dl={detection_limit} k={dl_multiple}"
                        ),
                    ));
                }
                let threshold = detection_limit * dl_multiple;
                if !threshold.is_finite() || threshold == 0.0 {
                    return Err(QaqcError::invalid(
                        "blank_rule.dl",
                        "the positive detection-limit threshold must remain finite and nonzero",
                    ));
                }
            }
            BlankFailureMode::CarryoverAnd {
                abs_threshold,
                carryover_fraction,
            }
            | BlankFailureMode::CarryoverOr {
                abs_threshold,
                carryover_fraction,
            } => {
                if !(abs_threshold.is_finite()
                    && (0.0..=1.0).contains(&abs_threshold)
                    && carryover_fraction.is_finite()
                    && carryover_fraction > 0.0
                    && carryover_fraction <= 1.0)
                {
                    return Err(QaqcError::invalid(
                        "blank_rule.carryover",
                        format!(
                            "invalid abs_threshold={abs_threshold} carryover_fraction={carryover_fraction}"
                        ),
                    ));
                }
            }
        }
        Ok(())
    }
}

/// Grade of the routine sample immediately preceding a blank in laboratory batch order.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PrecedingSampleGrade {
    /// Measured routine sample mass fraction in `[0, 1]`.
    Measured(f64),
    /// Over-limit (`>UL`) routine sample whose true grade strictly exceeds `limit`.
    AboveUpper {
        /// Upper analytical reporting limit (mass fraction in `(0, 1]`).
        limit: f64,
    },
    /// Below-detection routine sample: its mass fraction is in `[0, limit)`.
    BelowDetection {
        /// Detection limit (mass fraction in `(0, 1]`).
        limit: f64,
    },
}

/// One blank observation with optional preceding routine sample grade (for carryover checks).
#[derive(Clone, Debug, PartialEq)]
pub struct BlankObservation {
    /// Result id.
    pub result_id: String,
    /// Sample id.
    pub sample_id: String,
    /// Blank stage (coarse vs pulp).
    pub stage: BlankStage,
    /// Lab name.
    pub lab: String,
    /// Lab batch id.
    pub lab_batch: String,
    /// Observed blank assay value (below-detection is kept as non-detect, never DL/2).
    pub value: AssayValue,
    /// Preceding routine sample grade, retaining censoring; `None` is unknown in `[0, 1]`.
    pub preceding_sample_grade: Option<PrecedingSampleGrade>,
}

/// Verdict for a single blank observation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlankStatus {
    /// Every concentration admitted by the below-detection bound passes the rule.
    PassBelowDetection,
    /// Measured within blank tolerance.
    PassMeasured,
    /// Every concentration admitted by an above-upper bound passes the rule.
    PassCensored,
    /// Every admissible blank/predecessor concentration pair fails the contamination rule.
    FailContamination,
    /// Some admissible concentrations pass and others fail; no point substitute is used.
    Indeterminate,
}

/// Detailed blank verdict.
#[derive(Clone, Debug, PartialEq)]
pub struct BlankVerdict {
    /// Result id.
    pub result_id: String,
    /// Sample id.
    pub sample_id: String,
    /// Blank stage.
    pub stage: BlankStage,
    /// Lab name.
    pub lab: String,
    /// Lab batch id.
    pub lab_batch: String,
    /// Status under the evaluated blank rule.
    pub status: BlankStatus,
    /// Measured mass fraction, or `None` when censored.
    pub measured_mass_fraction: Option<f64>,
    /// Original assay, including its exact censoring kind and bound.
    pub value: AssayValue,
    /// Original preceding grade declaration, including censoring or absence.
    pub preceding_sample_grade: Option<PrecedingSampleGrade>,
    /// Smallest possible effective failure threshold under the declared rule.
    pub threshold_lower: f64,
    /// Largest possible effective failure threshold under the declared rule.
    pub threshold_upper: f64,
    /// Whether `threshold_lower` is excluded by the preceding grade's censoring.
    pub threshold_lower_exclusive: bool,
    /// Whether `threshold_upper` is excluded by the preceding grade's censoring.
    pub threshold_upper_exclusive: bool,
}

/// Summary of blank evaluations under one versioned [`SiteBlankRule`].
#[derive(Clone, Debug, PartialEq)]
pub struct BlankBatchEvaluation {
    /// Rule identifier.
    pub rule_id: String,
    /// Rule version.
    pub rule_version: String,
    /// Source document.
    pub rule_source_doc: String,
    /// Numeric predicate evaluated; provenance text does not replace its actual parameters.
    pub rule_mode: BlankFailureMode,
    /// Total blanks evaluated.
    pub total_count: usize,
    /// Count below detection limit.
    pub below_detection_count: usize,
    /// Blanks whose complete admissible range passes the rule.
    pub pass_count: usize,
    /// Blanks whose bounds do not determine whether the rule passes or fails.
    pub indeterminate_count: usize,
    /// Count failing contamination check.
    pub failure_count: usize,
    /// Confirmed failure percentage over all blanks; a lower bound when any are indeterminate.
    /// `None` if `total_count == 0`.
    pub failure_rate_pct: Option<f64>,
    /// Upper bound on failure percentage, counting indeterminate blanks as possibly failed.
    pub possible_failure_rate_pct: Option<f64>,
    /// Per-blank verdicts in input order.
    pub verdicts: Vec<BlankVerdict>,
}

#[derive(Clone, Copy)]
struct BlankBounds {
    lower: f64,
    upper: f64,
    lower_exclusive: bool,
    upper_exclusive: bool,
}

impl BlankBounds {
    fn point(value: f64) -> Self {
        Self {
            lower: value,
            upper: value,
            lower_exclusive: false,
            upper_exclusive: false,
        }
    }

    fn assay(value: AssayValue, field: &str) -> Result<Self, QaqcError> {
        match value {
            AssayValue::Measured { mass_fraction }
                if mass_fraction.is_finite() && (0.0..=1.0).contains(&mass_fraction) =>
            {
                Ok(Self::point(mass_fraction))
            }
            AssayValue::BelowDetection { limit }
                if limit.is_finite() && limit > 0.0 && limit <= 1.0 =>
            {
                Ok(Self {
                    lower: 0.0,
                    upper: limit,
                    lower_exclusive: false,
                    upper_exclusive: true,
                })
            }
            AssayValue::AboveUpper { limit } if limit.is_finite() && limit > 0.0 && limit < 1.0 => {
                Ok(Self {
                    lower: limit,
                    upper: 1.0,
                    lower_exclusive: true,
                    upper_exclusive: false,
                })
            }
            _ => Err(QaqcError::invalid(
                field,
                "assay must define a nonempty finite mass-fraction range in [0, 1]",
            )),
        }
    }

    fn scale(self, fraction: f64) -> Result<Self, QaqcError> {
        let lower = self.lower * fraction;
        let upper = self.upper * fraction;
        if (self.lower > 0.0 && lower == 0.0)
            || (self.upper > 0.0 && upper == 0.0)
            || (lower == upper && (self.lower_exclusive || self.upper_exclusive))
        {
            return Err(QaqcError::NumericalFailure {
                context: "blank carryover threshold".into(),
                detail: "a nonzero bound or nonempty censored range was lost to finite precision"
                    .into(),
            });
        }
        Ok(Self {
            lower,
            upper,
            ..self
        })
    }

    fn maximum(self, threshold: f64) -> Self {
        Self {
            lower: self.lower.max(threshold),
            upper: self.upper.max(threshold),
            lower_exclusive: self.lower >= threshold && self.lower_exclusive,
            upper_exclusive: self.upper > threshold && self.upper_exclusive,
        }
    }

    fn minimum(self, threshold: f64) -> Self {
        Self {
            lower: self.lower.min(threshold),
            upper: self.upper.min(threshold),
            lower_exclusive: self.lower < threshold && self.lower_exclusive,
            upper_exclusive: self.upper <= threshold && self.upper_exclusive,
        }
    }
}

/// Evaluates blank observations under a versioned [`SiteBlankRule`].
///
/// Strict exceedance is evaluated over the entire admissible concentration range. A censored
/// value is not replaced by its bound or DL/2 (`G-CENS`), and a missing preceding grade is unknown
/// in `[0, 1]`. A result is [`BlankStatus::Indeterminate`] when both pass and failure are possible.
/// AND uses the maximum of the two thresholds; OR uses their minimum. All comparisons are on the
/// computed binary64 values, with equality passing and no undeclared rounding tolerance.
pub fn evaluate_blanks(
    observations: &[BlankObservation],
    rule: &SiteBlankRule,
) -> Result<BlankBatchEvaluation, QaqcError> {
    rule.validate()?;
    let mut verdicts = Vec::with_capacity(observations.len());
    let mut below_detection_count = 0usize;
    let mut failure_count = 0usize;
    let mut pass_count = 0usize;
    let mut indeterminate_count = 0usize;

    for obs in observations {
        let value = BlankBounds::assay(obs.value, "blank.value")?;
        let preceding = match obs.preceding_sample_grade {
            Some(PrecedingSampleGrade::Measured(mass_fraction)) => BlankBounds::assay(
                AssayValue::Measured { mass_fraction },
                "blank.preceding_sample_grade",
            )?,
            Some(PrecedingSampleGrade::AboveUpper { limit }) => BlankBounds::assay(
                AssayValue::AboveUpper { limit },
                "blank.preceding_sample_grade",
            )?,
            Some(PrecedingSampleGrade::BelowDetection { limit }) => BlankBounds::assay(
                AssayValue::BelowDetection { limit },
                "blank.preceding_sample_grade",
            )?,
            None => BlankBounds {
                lower: 0.0,
                upper: 1.0,
                lower_exclusive: false,
                upper_exclusive: false,
            },
        };
        let threshold = match rule.mode {
            BlankFailureMode::AboveMultipleOfDl {
                detection_limit,
                dl_multiple,
            } => BlankBounds::point(detection_limit * dl_multiple),
            BlankFailureMode::CarryoverAnd {
                abs_threshold,
                carryover_fraction,
            } => preceding.scale(carryover_fraction)?.maximum(abs_threshold),
            BlankFailureMode::CarryoverOr {
                abs_threshold,
                carryover_fraction,
            } => preceding.scale(carryover_fraction)?.minimum(abs_threshold),
        };
        let measured_mass_fraction = match obs.value {
            AssayValue::Measured { mass_fraction } => Some(mass_fraction),
            AssayValue::BelowDetection { .. } | AssayValue::AboveUpper { .. } => None,
        };
        below_detection_count +=
            usize::from(matches!(obs.value, AssayValue::BelowDetection { .. }));
        let status = if value.upper <= threshold.lower {
            pass_count += 1;
            match obs.value {
                AssayValue::Measured { .. } => BlankStatus::PassMeasured,
                AssayValue::BelowDetection { .. } => BlankStatus::PassBelowDetection,
                AssayValue::AboveUpper { .. } => BlankStatus::PassCensored,
            }
        } else if value.lower > threshold.upper
            || (value.lower == threshold.upper
                && (value.lower_exclusive || threshold.upper_exclusive))
        {
            failure_count += 1;
            BlankStatus::FailContamination
        } else {
            indeterminate_count += 1;
            BlankStatus::Indeterminate
        };

        verdicts.push(BlankVerdict {
            result_id: obs.result_id.clone(),
            sample_id: obs.sample_id.clone(),
            stage: obs.stage,
            lab: obs.lab.clone(),
            lab_batch: obs.lab_batch.clone(),
            status,
            measured_mass_fraction,
            value: obs.value,
            preceding_sample_grade: obs.preceding_sample_grade,
            threshold_lower: threshold.lower,
            threshold_upper: threshold.upper,
            threshold_lower_exclusive: threshold.lower_exclusive,
            threshold_upper_exclusive: threshold.upper_exclusive,
        });
    }

    let total_count = observations.len();
    let failure_rate_pct = if total_count > 0 {
        Some(100.0 * (failure_count as f64) / (total_count as f64))
    } else {
        None
    };
    let possible_failure_rate_pct = if total_count > 0 {
        Some(100.0 * ((failure_count + indeterminate_count) as f64) / (total_count as f64))
    } else {
        None
    };

    Ok(BlankBatchEvaluation {
        rule_id: rule.rule_id.clone(),
        rule_version: rule.version.clone(),
        rule_source_doc: rule.source_doc.clone(),
        rule_mode: rule.mode,
        total_count,
        below_detection_count,
        pass_count,
        indeterminate_count,
        failure_count,
        failure_rate_pct,
        possible_failure_rate_pct,
        verdicts,
    })
}

/// First-class record of a time/period gap where a QC control was absent (`E168`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ControlAbsentPeriod {
    /// Period label (for example `"2015-2021"` or `"campaign-A"`).
    pub period_label: String,
    /// Control type that was absent (`"crm"`, `"blank"`, `"field_duplicate"`, `"pulp_duplicate"`, `"umpire"`).
    pub control_kind: String,
    /// Number of routine samples assayed during the control-absent period.
    pub routine_samples_affected: usize,
}

/// Per-lab disaggregated QA/QC performance summary (`E167`: internal mine labs vs accredited lab).
#[derive(Clone, Debug, PartialEq)]
pub struct LabQaqcSummary {
    /// Laboratory name.
    pub lab: String,
    /// Total CRM results evaluated at this lab.
    pub crm_total: usize,
    /// Total CRM failures at this lab.
    pub crm_failures: usize,
    /// CRM failure rate percent (`None` if `crm_total == 0`).
    pub crm_failure_rate_pct: Option<f64>,
    /// Total blank results evaluated at this lab.
    pub blank_total: usize,
    /// Total blank failures at this lab.
    pub blank_failures: usize,
    /// Blanks whose censoring or missing preceding grade leaves the rule undecided.
    pub blank_indeterminate: usize,
    /// Confirmed blank failure percentage over all blanks (`None` if `blank_total == 0`).
    /// This is a lower bound when `blank_indeterminate > 0`.
    pub blank_failure_rate_pct: Option<f64>,
    /// Per-CRM detailed evaluations at this lab, ordered by `crm_id`.
    pub crm_evaluations: BTreeMap<String, CrmBatchEvaluation>,
    /// Blank evaluation at this lab.
    pub blank_evaluation: BlankBatchEvaluation,
}

/// Cascade record linking a failed `(lab, lab_batch)` to every routine result in that batch
/// whose status must flip to stale/conflicted (`PO-CORE-4`, plan §12.4).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FailedBatchCascade {
    /// Laboratory name.
    pub lab: String,
    /// Laboratory batch identifier.
    pub lab_batch: String,
    /// Reasons the batch failed (CRM failure, blank contamination, etc.).
    pub failure_reasons: Vec<String>,
    /// Routine assay result ids in this batch flipped to `Stale`/`Conflicted`.
    pub affected_routine_result_ids: Vec<String>,
    /// Routine sample ids in this batch affected.
    pub affected_routine_sample_ids: Vec<String>,
}

/// Input bundle directly consuming canonical `fmine-drill` ledger records for one analyte + method.
#[derive(Clone, Debug, PartialEq)]
pub struct CampaignQaqcInput<'a> {
    /// Target analyte (e.g. `"Au"`). Every result must match or `PO-CORE-3` refuses.
    pub analyte: String,
    /// Target assay method defining the measurand. Every result must match or `PO-CORE-3` refuses.
    pub method: AssayMethod,
    /// Canonical `Sample` records from `fmine-drill`.
    pub samples: &'a [Sample],
    /// Canonical `QcInsertion` records from `fmine-drill`.
    pub insertions: &'a [QcInsertion],
    /// Canonical `AssayResult` records from `fmine-drill` for this analyte/method.
    pub results: &'a [AssayResult],
    /// Insertion target policy.
    pub insertion_targets: InsertionTargetSpec,
    /// Versioned CRM evaluation rule.
    pub crm_rule: SiteCrmRule,
    /// Versioned blank evaluation rule.
    pub blank_rule: SiteBlankRule,
}

/// Integrated campaign QA/QC report over canonical `fmine-drill` records (`BD-167`).
#[derive(Clone, Debug, PartialEq)]
pub struct CampaignQaqcReport {
    /// Analyte evaluated.
    pub analyte: String,
    /// Assay method (measurand) evaluated.
    pub method: AssayMethod,
    /// Insertion-rate accounting with explicit denominator and target basis.
    pub insertion_rates: InsertionRateSummary,
    /// Per-lab disaggregated CRM and blank performance (`E167`).
    pub per_lab: BTreeMap<String, LabQaqcSummary>,
    /// Failed batches and the routine samples/results cascaded to `Stale`/`Conflicted` (`PO-CORE-4`).
    pub failed_batches: Vec<FailedBatchCascade>,
    /// Duplicate samples whose `SampleType` or `mass_g` violates `E374` (for example quarter-core
    /// duplicate of a half-core original).
    pub unfit_mass_duplicates: Vec<String>,
}

/// Computes achieved QC insertion rates from canonical `Sample` and `QcInsertion` records.
///
/// Validates uniqueness of sample/insertion ids and ensures every QC insertion references a
/// supplied `Sample` record.
pub fn compute_insertion_rates(
    samples: &[Sample],
    insertions: &[QcInsertion],
    targets: &InsertionTargetSpec,
) -> Result<InsertionRateSummary, QaqcError> {
    targets.validate()?;

    let mut sample_map: BTreeMap<&str, &Sample> = BTreeMap::new();
    for s in samples {
        if sample_map.insert(s.sample_id.as_str(), s).is_some() {
            return Err(QaqcError::invalid(
                "samples",
                format!("duplicate sample_id '{}'", s.sample_id),
            ));
        }
    }

    let mut qc_sample_ids = BTreeSet::new();
    let mut crm_count = 0usize;
    let mut coarse_blank_count = 0usize;
    let mut pulp_blank_count = 0usize;
    let mut field_dup_count = 0usize;
    let mut coarse_dup_count = 0usize;
    let mut pulp_dup_count = 0usize;
    let mut umpire_count = 0usize;

    for ins in insertions {
        let Some(sample) = sample_map.get(ins.sample_id.as_str()) else {
            return Err(QaqcError::UnusableInput {
                context: format!("qc.insertion '{}'", ins.insertion_id),
                detail: format!("references unknown sample '{}'", ins.sample_id),
            });
        };
        if !qc_sample_ids.insert(ins.sample_id.as_str()) {
            return Err(QaqcError::invalid(
                "insertions",
                format!(
                    "sample '{}' has multiple QcInsertion records",
                    ins.sample_id
                ),
            ));
        }
        match &ins.kind {
            QcKind::Crm { .. } => {
                if sample.sample_type != SampleType::QcMaterial {
                    return Err(QaqcError::invalid(
                        "insertions",
                        format!(
                            "CRM insertion '{}' references non-QcMaterial sample '{}' ({})",
                            ins.insertion_id,
                            ins.sample_id,
                            sample.sample_type.code()
                        ),
                    ));
                }
                crm_count += 1;
            }
            QcKind::Blank { stage } => {
                if sample.sample_type != SampleType::QcMaterial {
                    return Err(QaqcError::invalid(
                        "insertions",
                        format!(
                            "blank insertion '{}' references non-QcMaterial sample '{}' ({})",
                            ins.insertion_id,
                            ins.sample_id,
                            sample.sample_type.code()
                        ),
                    ));
                }
                match stage {
                    BlankStage::Coarse => coarse_blank_count += 1,
                    BlankStage::Pulp => pulp_blank_count += 1,
                }
            }
            QcKind::Duplicate {
                stage,
                original_sample_id,
            } => {
                if sample.sample_type == SampleType::QcMaterial {
                    return Err(QaqcError::invalid(
                        "insertions",
                        format!(
                            "duplicate insertion '{}' must reference a drilled sample, not QcMaterial '{}'",
                            ins.insertion_id, ins.sample_id
                        ),
                    ));
                }
                if original_sample_id == &ins.sample_id {
                    return Err(QaqcError::invalid(
                        "insertions",
                        format!(
                            "duplicate insertion '{}' cannot reference itself ('{}')",
                            ins.insertion_id, ins.sample_id
                        ),
                    ));
                }
                if !sample_map.contains_key(original_sample_id.as_str()) {
                    return Err(QaqcError::UnusableInput {
                        context: format!("qc.insertion '{}'", ins.insertion_id),
                        detail: format!(
                            "duplicate references unknown original sample '{original_sample_id}'"
                        ),
                    });
                }
                match stage {
                    DuplicateStage::Field => field_dup_count += 1,
                    DuplicateStage::CoarseReject => coarse_dup_count += 1,
                    DuplicateStage::Pulp => pulp_dup_count += 1,
                }
            }
            QcKind::Umpire {
                original_sample_id, ..
            } => {
                if sample.sample_type == SampleType::QcMaterial {
                    return Err(QaqcError::invalid(
                        "insertions",
                        format!(
                            "umpire insertion '{}' must reference a drilled sample, not QcMaterial '{}'",
                            ins.insertion_id, ins.sample_id
                        ),
                    ));
                }
                if original_sample_id == &ins.sample_id {
                    return Err(QaqcError::invalid(
                        "insertions",
                        format!(
                            "umpire insertion '{}' cannot reference itself ('{}')",
                            ins.insertion_id, ins.sample_id
                        ),
                    ));
                }
                if !sample_map.contains_key(original_sample_id.as_str()) {
                    return Err(QaqcError::UnusableInput {
                        context: format!("qc.insertion '{}'", ins.insertion_id),
                        detail: format!(
                            "umpire references unknown original sample '{original_sample_id}'"
                        ),
                    });
                }
                umpire_count += 1;
            }
        }
    }

    let routine_sample_count = samples
        .iter()
        .filter(|s| {
            s.sample_type != SampleType::QcMaterial && !qc_sample_ids.contains(s.sample_id.as_str())
        })
        .count();
    let all_submitted_sample_count = samples.len();

    let denom = match targets.denominator {
        InsertionDenominator::RoutineSamples => routine_sample_count,
        InsertionDenominator::AllSubmittedSamples => all_submitted_sample_count,
    };

    let blank_total = coarse_blank_count + pulp_blank_count;
    let total_duplicates = field_dup_count + coarse_dup_count + pulp_dup_count;

    let crm = rate_row(crm_count, denom, targets.crm_pct);
    let blank = rate_row(blank_total, denom, targets.blank_pct);
    let field_duplicate = rate_row(field_dup_count, denom, targets.field_duplicate_pct);
    let pulp_duplicate = rate_row(pulp_dup_count, denom, targets.pulp_duplicate_pct);
    let external_share_of_duplicates = rate_row(
        umpire_count,
        total_duplicates,
        RateBandPct {
            min_pct: targets.external_share_of_duplicates_min_pct,
            max_pct: None,
        },
    );

    let mut absent_controls = Vec::new();
    if routine_sample_count > 0 {
        if crm_count == 0 {
            absent_controls.push("crm".to_string());
        }
        if blank_total == 0 {
            absent_controls.push("blank".to_string());
        }
        if field_dup_count == 0 {
            absent_controls.push("field_duplicate".to_string());
        }
        if pulp_dup_count == 0 {
            absent_controls.push("pulp_duplicate".to_string());
        }
        if umpire_count == 0 {
            absent_controls.push("umpire".to_string());
        }
    }

    Ok(InsertionRateSummary {
        target_basis: targets.basis.clone(),
        denominator_kind: targets.denominator,
        routine_sample_count,
        all_submitted_sample_count,
        crm,
        blank,
        coarse_blank_count,
        pulp_blank_count,
        field_duplicate,
        coarse_reject_duplicate_count: coarse_dup_count,
        pulp_duplicate,
        external_share_of_duplicates,
        absent_controls,
    })
}

/// Evaluates an entire campaign's `fmine-drill` QA/QC records end-to-end: insertion rates,
/// per-lab CRM & blank evaluations, `E374` duplicate mass/support checks, and failed-batch
/// cascade to affected routine samples (`BD-167`, `PO-CORE-3`, `PO-CORE-4`).
pub fn evaluate_campaign_qaqc(
    input: &CampaignQaqcInput<'_>,
) -> Result<CampaignQaqcReport, QaqcError> {
    input.insertion_targets.validate()?;
    input.crm_rule.validate()?;
    input.blank_rule.validate()?;

    let insertion_rates =
        compute_insertion_rates(input.samples, input.insertions, &input.insertion_targets)?;

    let sample_map: BTreeMap<&str, &Sample> = input
        .samples
        .iter()
        .map(|s| (s.sample_id.as_str(), s))
        .collect();
    let qc_by_sample: BTreeMap<&str, &QcInsertion> = input
        .insertions
        .iter()
        .map(|i| (i.sample_id.as_str(), i))
        .collect();

    // Check E374: duplicate mass and physical split type must match original.
    let mut unfit_mass_duplicates = Vec::new();
    for ins in input.insertions {
        if let QcKind::Duplicate {
            original_sample_id, ..
        } = &ins.kind
            && let (Some(orig), Some(dup)) = (
                sample_map.get(original_sample_id.as_str()),
                sample_map.get(ins.sample_id.as_str()),
            )
        {
            let support_type_mismatch = orig.sample_type != dup.sample_type;
            let mass_mismatch = match (orig.mass_g, dup.mass_g) {
                (Some(om), Some(dm)) if om > 0.0 => ((dm - om).abs() / om) > 0.25,
                _ => false,
            };
            if support_type_mismatch || mass_mismatch {
                unfit_mass_duplicates.push(ins.sample_id.clone());
            }
        }
    }
    unfit_mass_duplicates.sort();

    // PO-CORE-3: refuse mixed analyte or method in the same evaluation.
    for res in input.results {
        if res.analyte != input.analyte || res.method != input.method {
            return Err(QaqcError::MethodOrAnalyteMismatch {
                expected: format!("{}/{}", input.analyte, input.method.code()),
                actual: format!("{}/{}", res.analyte, res.method.code()),
            });
        }
        if !sample_map.contains_key(res.sample_id.as_str()) {
            return Err(QaqcError::UnusableInput {
                context: format!("result '{}'", res.result_id),
                detail: format!("references unknown sample '{}'", res.sample_id),
            });
        }
    }

    // Sort results deterministically by (received_at_ns, result_id) before tracking per-batch
    // preceding routine grades so carryover rules are order-invariant.
    let mut ordered_results: Vec<&AssayResult> = input.results.iter().collect();
    ordered_results.sort_by(|a, b| {
        a.received_at_ns
            .cmp(&b.received_at_ns)
            .then_with(|| a.result_id.cmp(&b.result_id))
    });

    // Group CRM results per (lab, crm_id) and blank results per lab, plus routine results per (lab, lab_batch).
    type CrmGroupKey = (String, String);
    let mut crm_specs: BTreeMap<CrmGroupKey, CrmCertificateSpec> = BTreeMap::new();
    let mut crm_obs: BTreeMap<CrmGroupKey, Vec<CrmObservation>> = BTreeMap::new();
    let mut blanks_by_lab: BTreeMap<String, Vec<BlankObservation>> = BTreeMap::new();
    let mut all_labs: BTreeSet<String> = BTreeSet::new();
    let mut routine_by_batch: BTreeMap<(String, String), Vec<(&str, &str)>> = BTreeMap::new();
    let mut prev_routine_grade_by_batch: BTreeMap<(String, String), PrecedingSampleGrade> =
        BTreeMap::new();

    for res in ordered_results {
        all_labs.insert(res.lab.clone());
        let batch_key = (res.lab.clone(), res.lab_batch.clone());
        if let Some(ins) = qc_by_sample.get(res.sample_id.as_str()) {
            match &ins.kind {
                QcKind::Crm {
                    crm_id,
                    certified_mass_fraction,
                    sigma_c,
                    sigma_l,
                    certificate_source,
                } => {
                    let key = (res.lab.clone(), crm_id.clone());
                    let spec = CrmCertificateSpec {
                        crm_id: crm_id.clone(),
                        analyte: input.analyte.clone(),
                        method: input.method,
                        certified_mean: *certified_mass_fraction,
                        sigma_c: *sigma_c,
                        sigma_l: *sigma_l,
                        certificate_source: certificate_source.clone(),
                    };
                    if let Some(existing) = crm_specs.get(&key)
                        && existing != &spec
                    {
                        return Err(QaqcError::invalid(
                            "crm.spec",
                            format!(
                                "conflicting CRM certificate specs for '{crm_id}' at lab '{}'",
                                res.lab
                            ),
                        ));
                    }
                    crm_specs.insert(key.clone(), spec);
                    crm_obs.entry(key).or_default().push(CrmObservation {
                        result_id: res.result_id.clone(),
                        sample_id: res.sample_id.clone(),
                        lab: res.lab.clone(),
                        lab_batch: res.lab_batch.clone(),
                        value: res.value,
                        received_at_ns: res.received_at_ns,
                    });
                }
                QcKind::Blank { stage } => {
                    let preceding = prev_routine_grade_by_batch.get(&batch_key).copied();
                    blanks_by_lab
                        .entry(res.lab.clone())
                        .or_default()
                        .push(BlankObservation {
                            result_id: res.result_id.clone(),
                            sample_id: res.sample_id.clone(),
                            stage: *stage,
                            lab: res.lab.clone(),
                            lab_batch: res.lab_batch.clone(),
                            value: res.value,
                            preceding_sample_grade: preceding,
                        });
                }
                QcKind::Duplicate { .. } | QcKind::Umpire { .. } => {}
            }
        } else {
            routine_by_batch
                .entry(batch_key.clone())
                .or_default()
                .push((res.result_id.as_str(), res.sample_id.as_str()));
            match res.value {
                AssayValue::Measured { mass_fraction } => {
                    prev_routine_grade_by_batch
                        .insert(batch_key, PrecedingSampleGrade::Measured(mass_fraction));
                }
                AssayValue::AboveUpper { limit } => {
                    // Over-limit ore is the highest-risk carryover source: preserve its lower bound `limit`.
                    prev_routine_grade_by_batch
                        .insert(batch_key, PrecedingSampleGrade::AboveUpper { limit });
                }
                AssayValue::BelowDetection { limit } => {
                    // Retain the current censored predecessor instead of an older grade or an invented zero.
                    prev_routine_grade_by_batch
                        .insert(batch_key, PrecedingSampleGrade::BelowDetection { limit });
                }
            }
        }
    }

    let mut per_lab = BTreeMap::new();
    let mut failed_batch_reasons: BTreeMap<(String, String), Vec<String>> = BTreeMap::new();

    for lab in all_labs {
        let mut crm_evaluations = BTreeMap::new();
        let mut crm_total = 0usize;
        let mut crm_failures = 0usize;

        for ((k_lab, crm_id), mut obs) in crm_obs
            .iter()
            .filter(|((l, _), _)| l == &lab)
            .map(|(k, v)| (k.clone(), v.clone()))
        {
            obs.sort_by(|a, b| {
                a.received_at_ns
                    .cmp(&b.received_at_ns)
                    .then_with(|| a.result_id.cmp(&b.result_id))
            });
            let Some(spec) = crm_specs.get(&(k_lab.clone(), crm_id.clone())).cloned() else {
                continue;
            };
            let eval = evaluate_crm_batch(
                &CrmBatchInput {
                    spec,
                    observations: obs,
                },
                &input.crm_rule,
            )?;
            crm_total += eval.points.len();
            crm_failures += eval.failed_point_count;
            for pt in &eval.points {
                if matches!(
                    pt.status,
                    crate::crm::CrmPointStatus::FailSingle
                        | crate::crm::CrmPointStatus::FailConsecutive
                ) {
                    failed_batch_reasons
                        .entry((lab.clone(), pt.lab_batch.clone()))
                        .or_default()
                        .push(format!(
                            "CRM '{crm_id}' result '{}' failed ({:?}, z={:.2} on {})",
                            pt.result_id,
                            pt.status,
                            pt.z_score,
                            eval.sd_basis.code()
                        ));
                }
            }
            if let Some(canmet) = eval.canmet
                && !canmet.passed
            {
                let batches: BTreeSet<&str> =
                    eval.points.iter().map(|p| p.lab_batch.as_str()).collect();
                for b in batches {
                    failed_batch_reasons
                        .entry((lab.clone(), b.to_string()))
                        .or_default()
                        .push(format!(
                            "CRM '{crm_id}' failed CANMET batch mean (|bias|={} > limit={})",
                            canmet.abs_bias, canmet.tolerance_limit
                        ));
                }
            }
            crm_evaluations.insert(crm_id, eval);
        }

        let blank_obs = blanks_by_lab.get(&lab).map_or(&[][..], Vec::as_slice);
        let blank_evaluation = evaluate_blanks(blank_obs, &input.blank_rule)?;
        for v in &blank_evaluation.verdicts {
            if v.status == BlankStatus::FailContamination {
                failed_batch_reasons
                    .entry((lab.clone(), v.lab_batch.clone()))
                    .or_default()
                    .push(format!(
                        "blank '{}' ({}) failed contamination rule '{}'",
                        v.result_id,
                        v.stage.code(),
                        input.blank_rule.rule_id
                    ));
            }
        }

        let crm_failure_rate_pct = if crm_total > 0 {
            Some(100.0 * (crm_failures as f64) / (crm_total as f64))
        } else {
            None
        };

        per_lab.insert(
            lab.clone(),
            LabQaqcSummary {
                lab,
                crm_total,
                crm_failures,
                crm_failure_rate_pct,
                blank_total: blank_evaluation.total_count,
                blank_failures: blank_evaluation.failure_count,
                blank_indeterminate: blank_evaluation.indeterminate_count,
                blank_failure_rate_pct: blank_evaluation.failure_rate_pct,
                crm_evaluations,
                blank_evaluation,
            },
        );
    }

    let mut failed_batches = Vec::new();
    for ((lab, lab_batch), failure_reasons) in failed_batch_reasons {
        let mut affected_routine_result_ids = Vec::new();
        let mut affected_samples = BTreeSet::new();
        if let Some(routine) = routine_by_batch.get(&(lab.clone(), lab_batch.clone())) {
            for &(rid, sid) in routine {
                affected_routine_result_ids.push(rid.to_string());
                affected_samples.insert(sid.to_string());
            }
        }
        affected_routine_result_ids.sort();
        failed_batches.push(FailedBatchCascade {
            lab,
            lab_batch,
            failure_reasons,
            affected_routine_result_ids,
            affected_routine_sample_ids: affected_samples.into_iter().collect(),
        });
    }

    Ok(CampaignQaqcReport {
        analyte: input.analyte.clone(),
        method: input.method,
        insertion_rates,
        per_lab,
        failed_batches,
        unfit_mass_duplicates,
    })
}
