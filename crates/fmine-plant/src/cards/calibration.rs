//! Determinism: BitwisePortable
//! Crucible K2 recovery calibration card over aligned lab-vs-plant monthly lot pairs
//! (`BD-289`, plan §17.3, `FACT E177`: Turquoise Ridge 71 BTAC-vs-plant monthly lots).
//!
//! Pure exact rational (`ExactQty`) and integer-square-root arithmetic with zero bare `f64`
//! parameters (L-6) and zero floating-point variance across ISAs. Expanding-window online
//! coverage is strictly causal (`0..t -> t`, `INV-008` / `INV-016` zero lookahead). Missing lab
//! lots are accounted for explicitly and never silently filled with zero.

use core::cmp::Ordering;
use std::collections::BTreeSet;

use fmine_core::codec::Encoder;
use fmine_core::hash::{Digest32, sha256};
use fmine_evidence::{Refusal, RefusalCode};
use fmine_units::ExactQty;

/// Mandatory disclosure banner on every recovery calibration card (`INV-002`, `NL-8`).
pub const RECOVERY_CALIBRATION_NO_CLAIM: &str = "Historical lab-vs-plant recovery calibration card over explicitly supplied lot pairs. Strictly causal expanding-window coverage and bias describe the supplied lot sequence only; missing lab lots are counted and never imputed. This card does not classify a resource or reserve, authorize plant control, or guarantee transferability to untested ore domains.";

/// Status of one historical lot row in a lab-vs-plant comparison table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LotPairStatus {
    /// Both bench/lab and commercial plant recoveries are recorded (in percentage points `[0, 100]`).
    Paired {
        lab_recovery_pct: ExactQty,
        plant_recovery_pct: ExactQty,
        /// Optional printed `plant - lab` column from the source table, checked against rounding tolerance.
        printed_diff_pct_pts: Option<ExactQty>,
    },
    /// Lab test result is unavailable/missing (e.g., `"No Results Available"` / `"N/A"`).
    MissingLabResult {
        plant_recovery_pct: Option<ExactQty>,
        reason: String,
    },
}

/// One chronological lot observation (e.g., one monthly stockpile lot).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LabVsPlantLot {
    pub period_label: String,
    pub lot_id: String,
    pub status: LotPairStatus,
}

/// Policy parameters governing bias, outlier, rounding-audit, and causal coverage classification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CalibrationCardPolicy {
    /// Number of initial paired lots used to warm up the causal expanding-window estimator (`>= 2`).
    pub warmup_lots: usize,
    /// Half-width in percentage points for evaluating raw uncalibrated lab predictions (`> 0`).
    pub naive_half_width_pct_pts: ExactQty,
    /// Predictive interval half-width multiplier `z > 0` applied to `sqrt(s_t^2 * (1 + 1/t))`
    /// (e.g., `128155 / 100000` for nominal 80% normal coverage).
    pub calibrated_z_multiplier: ExactQty,
    /// Nominal target coverage fraction in `(0, 1]` (e.g., `80 / 100`).
    pub nominal_coverage_target: ExactQty,
    /// Acceptable shortfall below `nominal_coverage_target` before flagging `Overconfident` (`[0, 1]`).
    pub coverage_lower_tolerance: ExactQty,
    /// Maximum acceptable `|mean_bias| / sample_sd` before flagging `Biased` (`>= 0`).
    pub max_abs_standardized_bias: ExactQty,
    /// Symmetric outlier magnitude threshold in percentage points (e.g., `5` for `> +5` / `< -5`).
    pub outlier_threshold_pct_pts: ExactQty,
    /// Maximum allowed discrepancy between a table's printed `Difference` column and `plant - lab`
    /// (`>= 0`; e.g., `1 / 10` = `0.1` pct pt for single-decimal independently rounded columns).
    pub max_printed_diff_rounding_tolerance: ExactQty,
}

impl CalibrationCardPolicy {
    /// Standard 80%-coverage recovery calibration policy matching `E177` thresholds (`±5` pts,
    /// `0.1` pt printed-diff rounding tolerance, `z = 1.28155`, warmup = 10 lots).
    pub fn standard_80pct() -> Result<Self, Refusal> {
        Ok(Self {
            warmup_lots: 10,
            naive_half_width_pct_pts: ExactQty::from_integer(5),
            calibrated_z_multiplier: ExactQty::new(128_155, 100_000).map_err(units_err)?,
            nominal_coverage_target: ExactQty::new(80, 100).map_err(units_err)?,
            coverage_lower_tolerance: ExactQty::new(10, 100).map_err(units_err)?,
            max_abs_standardized_bias: ExactQty::new(25, 100).map_err(units_err)?,
            outlier_threshold_pct_pts: ExactQty::from_integer(5),
            max_printed_diff_rounding_tolerance: ExactQty::new(1, 10).map_err(units_err)?,
        })
    }

    fn validate(&self) -> Result<(), Refusal> {
        let zero = ExactQty::from_integer(0);
        let one = ExactQty::from_integer(1);
        if self.warmup_lots < 2 {
            return Err(Refusal::new(
                RefusalCode::OutsidePhysicalDomain109,
                "calibration warmup_lots must be >= 2 to define sample variance",
            ));
        }
        if self.naive_half_width_pct_pts.signum() <= 0
            || self.calibrated_z_multiplier.signum() <= 0
            || self.outlier_threshold_pct_pts.signum() <= 0
        {
            return Err(Refusal::new(
                RefusalCode::OutsidePhysicalDomain109,
                "half-width, z-multiplier, and outlier threshold must be strictly positive",
            ));
        }
        if self.nominal_coverage_target.signum() <= 0
            || cmp_qty(self.nominal_coverage_target, one)? == Ordering::Greater
        {
            return Err(Refusal::new(
                RefusalCode::OutsidePhysicalDomain109,
                "nominal_coverage_target must lie in (0, 1]",
            ));
        }
        if self.coverage_lower_tolerance.signum() < 0
            || cmp_qty(self.coverage_lower_tolerance, one)? == Ordering::Greater
            || self.max_abs_standardized_bias.signum() < 0
            || cmp_qty(self.max_printed_diff_rounding_tolerance, zero)? == Ordering::Less
        {
            return Err(Refusal::new(
                RefusalCode::OutsidePhysicalDomain109,
                "tolerances and standardized-bias limit must be non-negative",
            ));
        }
        Ok(())
    }
}

/// Calibration health classification emitted by a [`RecoveryCalibrationCard`] (`BD-049`, `NL-8`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecoveryCalibrationFlag {
    Calibrated,
    Biased,
    Overconfident,
    BiasedAndOverconfident,
    Unscored,
}

impl RecoveryCalibrationFlag {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Calibrated => "calibrated",
            Self::Biased => "biased",
            Self::Overconfident => "overconfident",
            Self::BiasedAndOverconfident => "biased_and_overconfident",
            Self::Unscored => "unscored",
        }
    }

    fn tag(self) -> u8 {
        match self {
            Self::Calibrated => 1,
            Self::Biased => 2,
            Self::Overconfident => 3,
            Self::BiasedAndOverconfident => 4,
            Self::Unscored => 5,
        }
    }
}

/// One strictly causal expanding-window out-of-sample evaluation step (`0..t -> t`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OnlineStepDiagnostic {
    pub lot_id: String,
    pub lab_recovery_pct: ExactQty,
    pub plant_recovery_pct: ExactQty,
    pub residual_pct_pts: ExactQty,
    /// Sample mean of `plant - lab` over prior paired lots `0..t` only.
    pub prior_mean_bias_pct_pts: ExactQty,
    /// `true` iff `|plant - lab| <= naive_half_width_pct_pts`.
    pub raw_lab_covered: bool,
    /// `true` iff `(plant - (lab + prior_mean_bias))^2 <= z^2 * s_prior^2 * (1 + 1/t)`.
    pub bias_corrected_covered: bool,
}

/// Complete deterministic recovery calibration card with bias, dispersion, outliers, and causal
/// coverage (`BD-289`, `E177`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecoveryCalibrationCard {
    pub card_id: String,
    pub site: String,
    pub circuit: String,
    pub source_locator: String,
    pub source_sha256_hex: String,
    pub total_lots: usize,
    pub paired_count: usize,
    pub missing_lab_count: usize,
    /// Exact rational sample mean of `plant_recovery_pct - lab_recovery_pct` (percentage points).
    pub mean_bias_pct_pts: ExactQty,
    /// Exact rational unbiased sample variance `Σ(d_i - d̄)^2 / (n - 1)`.
    pub sample_variance_pct_pts_sq: ExactQty,
    /// Exact `10^-6`-scaled integer square root of `sample_variance_pct_pts_sq`.
    pub sample_sd_pct_pts: ExactQty,
    pub min_residual_pct_pts: ExactQty,
    pub max_residual_pct_pts: ExactQty,
    pub lots_strictly_above_threshold: usize,
    pub lots_strictly_below_neg_threshold: usize,
    /// Count of rows where the source's printed `Difference` column differed from `plant - lab`
    /// by a non-zero amount within `max_printed_diff_rounding_tolerance`.
    pub printed_diff_rounding_discrepancies: usize,
    pub scored_online_count: usize,
    pub raw_lab_covered_count: usize,
    pub raw_lab_coverage: ExactQty,
    pub bias_corrected_covered_count: usize,
    pub bias_corrected_coverage: ExactQty,
    pub raw_lab_flag: RecoveryCalibrationFlag,
    pub bias_corrected_flag: RecoveryCalibrationFlag,
    pub online_steps: Vec<OnlineStepDiagnostic>,
    pub no_claim: &'static str,
    pub card_digest: Digest32,
}

/// Parse a multi-page Markdown pipe table of lab-vs-plant monthly lots (`|Date|Lot|Lab|Plant|Diff|`)
/// beginning after `table_header_marker`.
///
/// Tolerates page-break headers/footers between continuation chunks and marks empty / `"No Results Available"`
/// / `"N/A"` lab entries explicitly as [`LotPairStatus::MissingLabResult`].
pub fn parse_markdown_lab_vs_plant_table(
    markdown: &str,
    table_header_marker: &str,
) -> Result<Vec<LabVsPlantLot>, Refusal> {
    if table_header_marker.trim().is_empty() {
        return Err(Refusal::new(
            RefusalCode::UndeclaredUnitBasisOrTz107,
            "table_header_marker must be non-empty",
        ));
    }

    let mut in_table = false;
    let mut saw_first_lot = false;
    let mut lots = Vec::new();

    for (idx, raw_line) in markdown.lines().enumerate() {
        let line_no = idx + 1;
        let line = raw_line.trim();
        if !in_table {
            if line.contains(table_header_marker) {
                in_table = true;
            }
            continue;
        }
        if line.is_empty()
            || line.starts_with("<!--")
            || line.starts_with("Page ")
            || line.starts_with("March ")
            || line.starts_with("####")
            || line.contains("NI 43-101 Technical Report")
        {
            continue;
        }
        if !line.starts_with('|') {
            if saw_first_lot {
                break;
            }
            continue;
        }

        let trimmed_pipes = line.trim_matches('|');
        let cols: Vec<&str> = trimmed_pipes.split('|').map(str::trim).collect();
        if cols.len() < 4 {
            continue;
        }
        // Skip header and Markdown divider rows.
        if cols[0].contains("**")
            || cols[0].chars().all(|c| c == '-' || c == ':' || c == ' ')
            || cols[1].contains("**")
        {
            continue;
        }

        let period_label = cols[0].to_string();
        let lot_id = cols[1].to_string();
        if period_label.is_empty() || lot_id.is_empty() {
            return Err(Refusal::new(
                RefusalCode::UndeclaredUnitBasisOrTz107,
                format!("line {line_no}: empty period or lot_id in lab-vs-plant table"),
            ));
        }

        let lab_raw = cols[2];
        let plant_raw = cols[3];
        let diff_raw = cols.get(4).copied().unwrap_or("");
        saw_first_lot = true;

        let lab_missing = lab_raw.is_empty()
            || lab_raw.eq_ignore_ascii_case("n/a")
            || lab_raw.to_ascii_lowercase().contains("no results");

        let status = if lab_missing {
            let plant_opt = if plant_raw.is_empty() || plant_raw.eq_ignore_ascii_case("n/a") {
                None
            } else {
                Some(parse_exact_pct(plant_raw, line_no, "plant")?)
            };
            let reason = if lab_raw.is_empty() {
                "blank_lab_cell".to_string()
            } else {
                lab_raw.to_string()
            };
            LotPairStatus::MissingLabResult {
                plant_recovery_pct: plant_opt,
                reason,
            }
        } else {
            let lab_recovery_pct = parse_exact_pct(lab_raw, line_no, "lab")?;
            let plant_recovery_pct = parse_exact_pct(plant_raw, line_no, "plant")?;
            let printed_diff_pct_pts =
                if diff_raw.is_empty() || diff_raw.eq_ignore_ascii_case("n/a") {
                    None
                } else {
                    Some(parse_exact_pct(diff_raw, line_no, "printed_diff")?)
                };
            LotPairStatus::Paired {
                lab_recovery_pct,
                plant_recovery_pct,
                printed_diff_pct_pts,
            }
        };

        lots.push(LabVsPlantLot {
            period_label,
            lot_id,
            status,
        });
    }

    if lots.is_empty() {
        return Err(Refusal::new(
            RefusalCode::NoClaimInDomain201,
            format!("no lot rows found after marker {table_header_marker:?}"),
        ));
    }
    Ok(lots)
}

/// Build and verify a deterministic [`RecoveryCalibrationCard`] from chronological lab-vs-plant lots.
#[allow(clippy::too_many_arguments)]
pub fn build_recovery_calibration_card(
    card_id: impl Into<String>,
    site: impl Into<String>,
    circuit: impl Into<String>,
    source_locator: impl Into<String>,
    source_sha256_hex: impl Into<String>,
    lots: &[LabVsPlantLot],
    policy: &CalibrationCardPolicy,
) -> Result<RecoveryCalibrationCard, Refusal> {
    let card_id = card_id.into();
    let site = site.into();
    let circuit = circuit.into();
    let source_locator = source_locator.into();
    let source_sha256_hex = source_sha256_hex.into();

    if card_id.trim().is_empty()
        || site.trim().is_empty()
        || circuit.trim().is_empty()
        || source_locator.trim().is_empty()
    {
        return Err(Refusal::new(
            RefusalCode::UndeclaredUnitBasisOrTz107,
            "card_id, site, circuit, and source_locator must be non-empty",
        ));
    }
    if !is_lower_hex64(&source_sha256_hex) {
        return Err(Refusal::new(
            RefusalCode::UndeclaredUnitBasisOrTz107,
            format!("source_sha256_hex must be 64 lowercase hex chars, got {source_sha256_hex:?}"),
        ));
    }
    policy.validate()?;

    let zero = ExactQty::from_integer(0);
    let hundred = ExactQty::from_integer(100);
    let mut seen_lots = BTreeSet::new();
    let mut paired: Vec<(&str, ExactQty, ExactQty, ExactQty)> = Vec::new();
    let mut missing_lab_count = 0usize;
    let mut printed_diff_rounding_discrepancies = 0usize;

    for lot in lots {
        if lot.lot_id.trim().is_empty() || lot.period_label.trim().is_empty() {
            return Err(Refusal::new(
                RefusalCode::UndeclaredUnitBasisOrTz107,
                "every lot must have non-empty period_label and lot_id",
            ));
        }
        if !seen_lots.insert(lot.lot_id.as_str()) {
            return Err(Refusal::new(
                RefusalCode::FrozenRecordMutation507,
                format!("duplicate lot_id {:?}", lot.lot_id),
            ));
        }
        match &lot.status {
            LotPairStatus::MissingLabResult {
                plant_recovery_pct, ..
            } => {
                if let Some(p) = *plant_recovery_pct {
                    validate_pct_range("plant_recovery_pct", p, zero, hundred)?;
                }
                missing_lab_count += 1;
            }
            LotPairStatus::Paired {
                lab_recovery_pct,
                plant_recovery_pct,
                printed_diff_pct_pts,
            } => {
                validate_pct_range("lab_recovery_pct", *lab_recovery_pct, zero, hundred)?;
                validate_pct_range("plant_recovery_pct", *plant_recovery_pct, zero, hundred)?;
                let diff = plant_recovery_pct
                    .checked_sub(*lab_recovery_pct)
                    .map_err(units_err)?;
                if let Some(printed) = *printed_diff_pct_pts {
                    let gap = abs_qty(printed.checked_sub(diff).map_err(units_err)?)?;
                    if cmp_qty(gap, policy.max_printed_diff_rounding_tolerance)?
                        == Ordering::Greater
                    {
                        return Err(Refusal::new(
                            RefusalCode::FrozenRecordMutation507,
                            format!(
                                "lot {:?}: printed diff disagrees with plant - lab by {}/{} > tolerance {}/{}",
                                lot.lot_id,
                                gap.num(),
                                gap.den(),
                                policy.max_printed_diff_rounding_tolerance.num(),
                                policy.max_printed_diff_rounding_tolerance.den()
                            ),
                        ));
                    }
                    if gap.signum() > 0 {
                        printed_diff_rounding_discrepancies += 1;
                    }
                }
                paired.push((
                    lot.lot_id.as_str(),
                    *lab_recovery_pct,
                    *plant_recovery_pct,
                    diff,
                ));
            }
        }
    }

    let paired_count = paired.len();
    if paired_count < 2 {
        return Err(Refusal::new(
            RefusalCode::NoClaimInDomain201,
            format!(
                "need >= 2 paired lots to compute bias and sample variance, got {paired_count}"
            ),
        ));
    }

    // Exact sample mean, unbiased variance, min/max, and outlier counts.
    let mut sum_diff = zero;
    let mut min_res = paired[0].3;
    let mut max_res = paired[0].3;
    let neg_thresh = policy
        .outlier_threshold_pct_pts
        .checked_neg()
        .map_err(units_err)?;
    let mut above_cnt = 0usize;
    let mut below_cnt = 0usize;

    for &(_, _, _, d) in &paired {
        sum_diff = sum_diff.checked_add(d).map_err(units_err)?;
        if cmp_qty(d, min_res)? == Ordering::Less {
            min_res = d;
        }
        if cmp_qty(d, max_res)? == Ordering::Greater {
            max_res = d;
        }
        if cmp_qty(d, policy.outlier_threshold_pct_pts)? == Ordering::Greater {
            above_cnt += 1;
        }
        if cmp_qty(d, neg_thresh)? == Ordering::Less {
            below_cnt += 1;
        }
    }

    let n_qty = ExactQty::from_integer(paired_count as i128);
    let mean_bias_pct_pts = sum_diff.checked_div(n_qty).map_err(units_err)?;

    let mut sum_sq = zero;
    for &(_, _, _, d) in &paired {
        let centered = d.checked_sub(mean_bias_pct_pts).map_err(units_err)?;
        let sq = centered.checked_mul(centered).map_err(units_err)?;
        sum_sq = sum_sq.checked_add(sq).map_err(units_err)?;
    }
    let sample_variance_pct_pts_sq = sum_sq
        .checked_div(ExactQty::from_integer((paired_count - 1) as i128))
        .map_err(units_err)?;
    let sample_sd_pct_pts = exact_sqrt_micro(sample_variance_pct_pts_sq)?;

    // Strictly causal expanding-window online coverage (0..t predicts lot t).
    let z_sq = policy
        .calibrated_z_multiplier
        .checked_mul(policy.calibrated_z_multiplier)
        .map_err(units_err)?;
    let mut online_steps = Vec::new();
    let mut raw_covered_count = 0usize;
    let mut corr_covered_count = 0usize;
    let mut online_corr_errors: Vec<ExactQty> = Vec::new();

    if paired_count > policy.warmup_lots {
        for t in policy.warmup_lots..paired_count {
            let (lot_id, lab, plant, diff) = paired[t];
            let mut prior_sum = zero;
            for &(_, _, _, pd) in &paired[..t] {
                prior_sum = prior_sum.checked_add(pd).map_err(units_err)?;
            }
            let t_qty = ExactQty::from_integer(t as i128);
            let prior_mean = prior_sum.checked_div(t_qty).map_err(units_err)?;

            let mut prior_sq = zero;
            for &(_, _, _, pd) in &paired[..t] {
                let c = pd.checked_sub(prior_mean).map_err(units_err)?;
                prior_sq = prior_sq
                    .checked_add(c.checked_mul(c).map_err(units_err)?)
                    .map_err(units_err)?;
            }
            let prior_var = prior_sq
                .checked_div(ExactQty::from_integer((t - 1) as i128))
                .map_err(units_err)?;

            // Raw uncalibrated check: |plant - lab| <= naive_half_width.
            let raw_ok =
                cmp_qty(abs_qty(diff)?, policy.naive_half_width_pct_pts)? != Ordering::Greater;
            if raw_ok {
                raw_covered_count += 1;
            }

            // Bias-corrected causal prediction: lab + prior_mean, predictive variance = prior_var * (t + 1) / t.
            let corr_err = diff.checked_sub(prior_mean).map_err(units_err)?;
            // Quantize to 10^-6 pct-pt grid so summing across varying denominators 1..t never
            // triggers LCM growth overflow in i128 rational arithmetic.
            online_corr_errors.push(quantize_micro(corr_err)?);
            let corr_err_sq = corr_err.checked_mul(corr_err).map_err(units_err)?;
            let pred_factor = ExactQty::new((t + 1) as i128, t as i128).map_err(units_err)?;
            let pred_half_width_sq = z_sq
                .checked_mul(prior_var)
                .map_err(units_err)?
                .checked_mul(pred_factor)
                .map_err(units_err)?;
            let corr_ok = cmp_qty(corr_err_sq, pred_half_width_sq)? != Ordering::Greater;
            if corr_ok {
                corr_covered_count += 1;
            }

            online_steps.push(OnlineStepDiagnostic {
                lot_id: lot_id.to_string(),
                lab_recovery_pct: lab,
                plant_recovery_pct: plant,
                residual_pct_pts: diff,
                prior_mean_bias_pct_pts: prior_mean,
                raw_lab_covered: raw_ok,
                bias_corrected_covered: corr_ok,
            });
        }
    }

    let scored_online_count = online_steps.len();
    let (raw_lab_coverage, bias_corrected_coverage, raw_lab_flag, bias_corrected_flag) =
        if scored_online_count == 0 {
            (
                zero,
                zero,
                RecoveryCalibrationFlag::Unscored,
                RecoveryCalibrationFlag::Unscored,
            )
        } else {
            let raw_cov = ExactQty::new(raw_covered_count as i128, scored_online_count as i128)
                .map_err(units_err)?;
            let corr_cov = ExactQty::new(corr_covered_count as i128, scored_online_count as i128)
                .map_err(units_err)?;
            let min_cov = policy
                .nominal_coverage_target
                .checked_sub(policy.coverage_lower_tolerance)
                .map_err(units_err)?;

            let raw_biased = is_biased(
                mean_bias_pct_pts,
                sample_variance_pct_pts_sq,
                policy.max_abs_standardized_bias,
            )?;
            let raw_overconf = cmp_qty(raw_cov, min_cov)? == Ordering::Less;
            let raw_flag = classify_flag(raw_biased, raw_overconf);

            let (corr_mean, corr_var) = exact_mean_and_var(&online_corr_errors)?;
            let corr_biased = if online_corr_errors.len() >= 2 {
                is_biased(corr_mean, corr_var, policy.max_abs_standardized_bias)?
            } else {
                false
            };
            let corr_overconf = cmp_qty(corr_cov, min_cov)? == Ordering::Less;
            let corr_flag = classify_flag(corr_biased, corr_overconf);

            (raw_cov, corr_cov, raw_flag, corr_flag)
        };

    // Canonical SHA-256 digest over card metadata, policy, summary statistics, and online verdicts.
    let mut enc = Encoder::new_frame(*b"RCC1", 1);
    enc.put_str(&card_id);
    enc.put_str(&site);
    enc.put_str(&circuit);
    enc.put_str(&source_locator);
    enc.put_str(&source_sha256_hex);
    enc.put_u64(policy.warmup_lots as u64);
    put_qty(&mut enc, policy.naive_half_width_pct_pts);
    put_qty(&mut enc, policy.calibrated_z_multiplier);
    put_qty(&mut enc, policy.nominal_coverage_target);
    put_qty(&mut enc, policy.coverage_lower_tolerance);
    put_qty(&mut enc, policy.max_abs_standardized_bias);
    put_qty(&mut enc, policy.outlier_threshold_pct_pts);
    put_qty(&mut enc, policy.max_printed_diff_rounding_tolerance);
    enc.put_u64(lots.len() as u64);
    enc.put_u64(paired_count as u64);
    enc.put_u64(missing_lab_count as u64);
    put_qty(&mut enc, mean_bias_pct_pts);
    put_qty(&mut enc, sample_variance_pct_pts_sq);
    put_qty(&mut enc, sample_sd_pct_pts);
    put_qty(&mut enc, min_res);
    put_qty(&mut enc, max_res);
    enc.put_u64(above_cnt as u64);
    enc.put_u64(below_cnt as u64);
    enc.put_u64(printed_diff_rounding_discrepancies as u64);
    enc.put_u64(scored_online_count as u64);
    enc.put_u64(raw_covered_count as u64);
    enc.put_u64(corr_covered_count as u64);
    enc.put_u8(raw_lab_flag.tag());
    enc.put_u8(bias_corrected_flag.tag());
    let card_digest = sha256(&enc.finish());

    Ok(RecoveryCalibrationCard {
        card_id,
        site,
        circuit,
        source_locator,
        source_sha256_hex,
        total_lots: lots.len(),
        paired_count,
        missing_lab_count,
        mean_bias_pct_pts,
        sample_variance_pct_pts_sq,
        sample_sd_pct_pts,
        min_residual_pct_pts: min_res,
        max_residual_pct_pts: max_res,
        lots_strictly_above_threshold: above_cnt,
        lots_strictly_below_neg_threshold: below_cnt,
        printed_diff_rounding_discrepancies,
        scored_online_count,
        raw_lab_covered_count: raw_covered_count,
        raw_lab_coverage,
        bias_corrected_covered_count: corr_covered_count,
        bias_corrected_coverage,
        raw_lab_flag,
        bias_corrected_flag,
        online_steps,
        no_claim: RECOVERY_CALIBRATION_NO_CLAIM,
        card_digest,
    })
}

fn units_err(err: fmine_units::UnitsError) -> Refusal {
    Refusal::new(
        RefusalCode::OutsidePhysicalDomain109,
        format!("exact rational arithmetic error: {err}"),
    )
}

fn cmp_qty(a: ExactQty, b: ExactQty) -> Result<Ordering, Refusal> {
    a.checked_cmp(b).map_err(units_err)
}

fn abs_qty(q: ExactQty) -> Result<ExactQty, Refusal> {
    if q.signum() < 0 {
        q.checked_neg().map_err(units_err)
    } else {
        Ok(q)
    }
}

fn validate_pct_range(
    field: &str,
    val: ExactQty,
    lo: ExactQty,
    hi: ExactQty,
) -> Result<(), Refusal> {
    if cmp_qty(val, lo)? == Ordering::Less || cmp_qty(val, hi)? == Ordering::Greater {
        return Err(Refusal::new(
            RefusalCode::OutsidePhysicalDomain109,
            format!(
                "{field} must lie in [0, 100]%, got {}/{}",
                val.num(),
                val.den()
            ),
        ));
    }
    Ok(())
}

fn is_lower_hex64(s: &str) -> bool {
    s.len() == 64 && s.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

fn parse_exact_pct(raw: &str, line_no: usize, field: &str) -> Result<ExactQty, Refusal> {
    let cleaned = raw.trim().trim_end_matches('%').trim();
    ExactQty::from_decimal_str(cleaned).map_err(|err| {
        Refusal::new(
            RefusalCode::UndeclaredUnitBasisOrTz107,
            format!("line {line_no}: invalid {field} decimal {raw:?}: {err}"),
        )
    })
}

fn exact_mean_and_var(vals: &[ExactQty]) -> Result<(ExactQty, ExactQty), Refusal> {
    let zero = ExactQty::from_integer(0);
    if vals.is_empty() {
        return Ok((zero, zero));
    }
    let mut sum = zero;
    for &v in vals {
        sum = sum.checked_add(v).map_err(units_err)?;
    }
    let mean = sum
        .checked_div(ExactQty::from_integer(vals.len() as i128))
        .map_err(units_err)?;
    if vals.len() < 2 {
        return Ok((mean, zero));
    }
    let mut sq = zero;
    for &v in vals {
        let c = v.checked_sub(mean).map_err(units_err)?;
        sq = sq
            .checked_add(c.checked_mul(c).map_err(units_err)?)
            .map_err(units_err)?;
    }
    let var = sq
        .checked_div(ExactQty::from_integer((vals.len() - 1) as i128))
        .map_err(units_err)?;
    Ok((mean, var))
}

/// Exact test `mean_bias^2 > limit^2 * variance` (equivalent to `|mean_bias / sd| > limit`
/// with zero floating point; if `variance == 0`, any non-zero bias flags `true`).
fn is_biased(mean: ExactQty, variance: ExactQty, limit: ExactQty) -> Result<bool, Refusal> {
    if variance.signum() == 0 {
        return Ok(mean.signum() != 0);
    }
    let mean_sq = mean.checked_mul(mean).map_err(units_err)?;
    let lim_sq_var = limit
        .checked_mul(limit)
        .map_err(units_err)?
        .checked_mul(variance)
        .map_err(units_err)?;
    Ok(cmp_qty(mean_sq, lim_sq_var)? == Ordering::Greater)
}

fn classify_flag(biased: bool, overconfident: bool) -> RecoveryCalibrationFlag {
    match (biased, overconfident) {
        (false, false) => RecoveryCalibrationFlag::Calibrated,
        (true, false) => RecoveryCalibrationFlag::Biased,
        (false, true) => RecoveryCalibrationFlag::Overconfident,
        (true, true) => RecoveryCalibrationFlag::BiasedAndOverconfident,
    }
}

/// Compute `floor(sqrt(var) * 10^6) / 10^6` using exact `u128` arithmetic (zero floating point).
fn exact_sqrt_micro(var: ExactQty) -> Result<ExactQty, Refusal> {
    if var.signum() < 0 {
        return Err(Refusal::new(
            RefusalCode::OutsidePhysicalDomain109,
            "variance cannot be negative",
        ));
    }
    if var.signum() == 0 {
        return Ok(ExactQty::from_integer(0));
    }
    let scale: u128 = 1_000_000_000_000; // (10^6)^2
    let num = var.num() as u128;
    let den = var.den() as u128;
    let scaled = if let Some(prod) = num.checked_mul(scale) {
        prod / den
    } else {
        // Fallback exact scaling without overflow: (num / den) * scale + ((num % den) * scale) / den.
        let q = num / den;
        let r = num % den;
        let hi = q.checked_mul(scale).ok_or_else(|| {
            Refusal::new(
                RefusalCode::OutsidePhysicalDomain109,
                "variance too large for micro sqrt",
            )
        })?;
        let lo = {
            let mut r_s = r;
            let mut d_s = den;
            while r_s.checked_mul(scale).is_none() && d_s > 1 {
                r_s >>= 1;
                d_s >>= 1;
            }
            (r_s.saturating_mul(scale)) / d_s.max(1)
        };
        hi.checked_add(lo).ok_or_else(|| {
            Refusal::new(
                RefusalCode::OutsidePhysicalDomain109,
                "variance too large for micro sqrt",
            )
        })?
    };
    let root = isqrt_u128(scaled);
    ExactQty::new(root as i128, 1_000_000).map_err(units_err)
}

/// Round an `ExactQty` half-away-from-zero onto the `10^-6` rational grid to bound LCM growth
/// across expanding-window out-of-sample residuals.
fn quantize_micro(q: ExactQty) -> Result<ExactQty, Refusal> {
    const MICRO: i128 = 1_000_000;
    let neg = q.signum() < 0;
    let abs_num = q.num().unsigned_abs();
    let den = q.den().unsigned_abs();
    let q_int = abs_num / den;
    let r = abs_num % den;
    let mut r_s = r;
    let mut d_s = den;
    while r_s.checked_mul(MICRO as u128).is_none() && d_s > 1 {
        r_s >>= 1;
        d_s >>= 1;
    }
    let scaled_rem = r_s.saturating_mul(MICRO as u128);
    let frac = scaled_rem / d_s.max(1);
    let rem2 = scaled_rem % d_s.max(1);
    let round_up = u128::from(rem2.saturating_mul(2) >= d_s.max(1));
    let total_micro = q_int
        .checked_mul(MICRO as u128)
        .and_then(|v| v.checked_add(frac))
        .and_then(|v| v.checked_add(round_up))
        .ok_or_else(|| {
            Refusal::new(
                RefusalCode::OutsidePhysicalDomain109,
                "quantize_micro overflow",
            )
        })?;
    let signed = i128::try_from(total_micro).map_err(|_| {
        Refusal::new(
            RefusalCode::OutsidePhysicalDomain109,
            "quantize_micro signed overflow",
        )
    })?;
    ExactQty::new(if neg { -signed } else { signed }, MICRO).map_err(units_err)
}

fn isqrt_u128(n: u128) -> u128 {
    if n < 2 {
        return n;
    }
    let mut x = 1_u128 << ((128 - n.leading_zeros()).div_ceil(2));
    loop {
        let next = (x + n / x) / 2;
        if next >= x {
            return x;
        }
        x = next;
    }
}

fn put_qty(enc: &mut Encoder, q: ExactQty) {
    enc.put_i128(q.num());
    enc.put_i128(q.den());
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn dec(s: &str) -> ExactQty {
        ExactQty::from_decimal_str(s).expect("valid decimal")
    }

    #[test]
    fn test_recovery_calibration_card_known_answers_and_strict_causal_online_coverage() {
        // Synthetic 16-lot sequence: 14 paired lots with exact +3.0 pt mean bias and alternating
        // ±1.0 pt oscillation (exact sample variance = 14 / 13), plus 2 missing-lab lots.
        let mut lots = Vec::new();
        for i in 0..14_usize {
            let offset = if i % 2 == 0 { "4.0" } else { "2.0" };
            let lab = dec("85.0");
            let plant = lab.checked_add(dec(offset)).unwrap();
            lots.push(LabVsPlantLot {
                period_label: format!("2025-{:02}", (i % 12) + 1),
                lot_id: format!("SYNTH-LOT-{:02}", i + 1),
                status: LotPairStatus::Paired {
                    lab_recovery_pct: lab,
                    plant_recovery_pct: plant,
                    printed_diff_pct_pts: Some(dec(offset)),
                },
            });
            if i == 4 || i == 9 {
                lots.push(LabVsPlantLot {
                    period_label: "2025-NA".to_string(),
                    lot_id: format!("SYNTH-MISSING-{i}"),
                    status: LotPairStatus::MissingLabResult {
                        plant_recovery_pct: Some(dec("88.0")),
                        reason: "No Results Available".to_string(),
                    },
                });
            }
        }

        let mut policy = CalibrationCardPolicy::standard_80pct().unwrap();
        policy.warmup_lots = 6;
        policy.naive_half_width_pct_pts = dec("2.5");

        let sha = "a".repeat(64);
        let card = build_recovery_calibration_card(
            "synth-trug-like-v1",
            "SyntheticAutoclave",
            "BTAC-vs-Plant",
            "synthetic:table-1",
            sha.clone(),
            &lots,
            &policy,
        )
        .expect("build synthetic card");

        assert_eq!(card.total_lots, 16);
        assert_eq!(card.paired_count, 14);
        assert_eq!(card.missing_lab_count, 2);
        // Exact mean bias = +3.0 pts; exact unbiased variance = 14 / 13.
        assert_eq!(card.mean_bias_pct_pts, ExactQty::from_integer(3));
        assert_eq!(
            card.sample_variance_pct_pts_sq,
            ExactQty::new(14, 13).unwrap()
        );
        // sqrt(14/13) = 1.03774904... -> exact micro-root 1.037749.
        assert_eq!(
            card.sample_sd_pct_pts,
            ExactQty::new(1_037_749, 1_000_000).unwrap()
        );
        assert_eq!(card.scored_online_count, 8);
        // Raw lab covers only the +2.0 lots (4/8 = 50% < 70% floor) and is heavily biased (+3/1.038 > 0.25).
        assert_eq!(card.raw_lab_covered_count, 4);
        assert_eq!(card.raw_lab_coverage, ExactQty::new(1, 2).unwrap());
        assert_eq!(
            card.raw_lab_flag,
            RecoveryCalibrationFlag::BiasedAndOverconfident
        );
        // Bias-corrected causal intervals cover all 8/8 out-of-sample lots and eliminate bias.
        assert_eq!(card.bias_corrected_covered_count, 8);
        assert_eq!(card.bias_corrected_coverage, ExactQty::from_integer(1));
        assert_eq!(
            card.bias_corrected_flag,
            RecoveryCalibrationFlag::Calibrated
        );

        // Deterministic card digest on identical rerun.
        let card2 = build_recovery_calibration_card(
            "synth-trug-like-v1",
            "SyntheticAutoclave",
            "BTAC-vs-Plant",
            "synthetic:table-1",
            sha,
            &lots,
            &policy,
        )
        .expect("rerun");
        assert_eq!(card.card_digest, card2.card_digest);
    }

    #[test]
    fn test_recovery_calibration_card_refuses_malformed_bounds_duplicates_and_diff_tamper() {
        let policy = CalibrationCardPolicy::standard_80pct().unwrap();
        let sha = "b".repeat(64);
        let mk = |id: &str, lab: &str, plant: &str, diff: Option<&str>| LabVsPlantLot {
            period_label: "Jan 2025".to_string(),
            lot_id: id.to_string(),
            status: LotPairStatus::Paired {
                lab_recovery_pct: dec(lab),
                plant_recovery_pct: dec(plant),
                printed_diff_pct_pts: diff.map(dec),
            },
        };

        // 1. Out-of-range recovery (> 100%) refuses with FMINE-REF-109.
        let oob = vec![
            mk("L1", "101.0", "92.0", None),
            mk("L2", "90.0", "91.0", None),
        ];
        assert_eq!(
            build_recovery_calibration_card("c", "s", "k", "loc", sha.clone(), &oob, &policy)
                .unwrap_err()
                .code,
            "FMINE-REF-109"
        );

        // 2. Duplicate lot_id refuses with FMINE-REF-507.
        let dup = vec![
            mk("L1", "90.0", "91.0", Some("1.0")),
            mk("L1", "89.0", "90.0", Some("1.0")),
        ];
        assert_eq!(
            build_recovery_calibration_card("c", "s", "k", "loc", sha.clone(), &dup, &policy)
                .unwrap_err()
                .code,
            "FMINE-REF-507"
        );

        // 3. Printed diff contradicting plant - lab beyond 0.1 tolerance refuses with FMINE-REF-507.
        let tamper = vec![
            mk("L1", "90.0", "92.0", Some("2.5")),
            mk("L2", "89.0", "90.0", Some("1.0")),
        ];
        assert_eq!(
            build_recovery_calibration_card("c", "s", "k", "loc", sha.clone(), &tamper, &policy)
                .unwrap_err()
                .code,
            "FMINE-REF-507"
        );

        // 4. Printed diff within exact 0.1 rounding tolerance is accepted and counted.
        let rounded = vec![
            mk("L1", "90.0", "92.0", Some("2.1")),
            mk("L2", "89.0", "90.0", Some("1.0")),
        ];
        let ok_card =
            build_recovery_calibration_card("c", "s", "k", "loc", sha.clone(), &rounded, &policy)
                .expect("0.1 rounding gap accepted");
        assert_eq!(ok_card.printed_diff_rounding_discrepancies, 1);
        assert_eq!(ok_card.raw_lab_flag, RecoveryCalibrationFlag::Unscored);

        // 5. Fewer than 2 paired lots refuses with FMINE-REF-201; bad SHA refuses with FMINE-REF-107.
        let too_few = vec![mk("L1", "90.0", "92.0", Some("2.0"))];
        assert_eq!(
            build_recovery_calibration_card("c", "s", "k", "loc", sha, &too_few, &policy)
                .unwrap_err()
                .code,
            "FMINE-REF-201"
        );
        assert_eq!(
            build_recovery_calibration_card("c", "s", "k", "loc", "bad-sha", &rounded, &policy)
                .unwrap_err()
                .code,
            "FMINE-REF-107"
        );
    }

    #[test]
    fn test_parse_markdown_lab_vs_plant_table_multipage_synthetic() {
        let md = "\
**Table X Synthetic Lab vs Plant**

|**Date**|**Lot Name**|**BTAC**|**Plant**|**Diff**|
|---|---|---|---|---|
|Jan 2024|Lot #1|88.0|90.5|2.5|
|Feb 2024|Lot #2||89.1|N/A|

March 15, 2024
Page 1
<!-- p.2 -->
Synthetic NI 43-101 Technical Report

|Mar 2024|Lot #3|No Results Available|91.0|N/A|
|---|---|---|---|---|
|Apr 2024|Lot #4|86.4|92.0|5.6|

Narrative paragraph terminates the table.
";
        let lots = parse_markdown_lab_vs_plant_table(md, "**Table X Synthetic Lab vs Plant**")
            .expect("parse multipage synthetic table");
        assert_eq!(lots.len(), 4);
        assert!(matches!(lots[0].status, LotPairStatus::Paired { .. }));
        assert!(matches!(
            lots[1].status,
            LotPairStatus::MissingLabResult { .. }
        ));
        assert!(matches!(
            lots[2].status,
            LotPairStatus::MissingLabResult { .. }
        ));
        assert!(matches!(lots[3].status, LotPairStatus::Paired { .. }));
    }

    #[test]
    fn test_71_lot_expanding_window_no_lcm_overflow_and_policy_digest() {
        // Synthetic 83-lot stress test (71 paired + 12 missing) verifying zero i128 LCM overflow
        // across expanding-window steps t = 10..71 and verifying card_digest sensitivity to policy.
        let mut lots = Vec::new();
        for i in 0..71_usize {
            let offset_tenths = match i % 7 {
                0 => 62,  // +6.2 (> +5)
                1 => -54, // -5.4 (< -5)
                2 => 13,
                3 => 37,
                4 => -8,
                5 => 41,
                _ => 29,
            };
            let lab = dec("88.0");
            let diff = ExactQty::new(offset_tenths, 10).unwrap();
            let plant = lab.checked_add(diff).unwrap();
            let printed = if i % 10 == 0 {
                diff.checked_add(ExactQty::new(1, 10).unwrap()).unwrap()
            } else {
                diff
            };
            lots.push(LabVsPlantLot {
                period_label: format!("2020-{:02}", (i % 12) + 1),
                lot_id: format!("STRESS-71-LOT-{:03}", i + 1),
                status: LotPairStatus::Paired {
                    lab_recovery_pct: lab,
                    plant_recovery_pct: plant,
                    printed_diff_pct_pts: Some(printed),
                },
            });
            if i < 12 {
                lots.push(LabVsPlantLot {
                    period_label: format!("2020-NA-{i}"),
                    lot_id: format!("STRESS-71-MISSING-{:02}", i + 1),
                    status: LotPairStatus::MissingLabResult {
                        plant_recovery_pct: Some(dec("90.0")),
                        reason: "No Results Available".to_string(),
                    },
                });
            }
        }

        let policy_a = CalibrationCardPolicy::standard_80pct().unwrap();
        let mut policy_b = policy_a.clone();
        policy_b.warmup_lots = 12;

        let sha = "c".repeat(64);
        let card_a = build_recovery_calibration_card(
            "stress-71-v1",
            "SyntheticStressSite",
            "BTAC-vs-Plant",
            "synthetic:stress-71",
            sha.clone(),
            &lots,
            &policy_a,
        )
        .expect("71-lot expanding window must not overflow");
        let card_b = build_recovery_calibration_card(
            "stress-71-v1",
            "SyntheticStressSite",
            "BTAC-vs-Plant",
            "synthetic:stress-71",
            sha,
            &lots,
            &policy_b,
        )
        .expect("policy_b card");

        assert_eq!(card_a.total_lots, 83);
        assert_eq!(card_a.paired_count, 71);
        assert_eq!(card_a.missing_lab_count, 12);
        assert_eq!(card_a.scored_online_count, 61);
        assert_eq!(card_a.printed_diff_rounding_discrepancies, 8);
        assert_ne!(card_a.card_digest, card_b.card_digest);
    }

    /// Real-file lane test (`BD-289`, `FACT E177`): reproduces Turquoise Ridge's 71 monthly
    /// BTAC-vs-plant pairs from the local gitignored NI 43-101 extraction (Table 13-2, pp. 136-138).
    #[test]
    #[ignore = "requires local gitignored corpus_of_input_docs/markdown_extractions/Turquoise-Ridge-Technical-Report-March2024.md"]
    fn test_e177_turquoise_ridge_71_pairs_real_corpus_card() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|p| p.parent())
            .expect("workspace root")
            .to_path_buf();
        let path = root.join(
            "corpus_of_input_docs/markdown_extractions/Turquoise-Ridge-Technical-Report-March2024.md",
        );
        let bytes = std::fs::read(&path).expect("read local Turquoise Ridge markdown extraction");
        let sha_hex = sha256(&bytes).to_hex();
        assert_eq!(
            sha_hex,
            "9f55b4e0ea8300d26a6893f80db62fe034a95d92764145986009567370a85029"
        );
        let text = String::from_utf8(bytes).expect("utf-8");
        let lots = parse_markdown_lab_vs_plant_table(
            &text,
            "**Table 13-2 TRUG Monthly Composite BTAC vs Plant Results**",
        )
        .expect("parse Table 13-2");

        let policy = CalibrationCardPolicy::standard_80pct().expect("standard policy");
        let card = build_recovery_calibration_card(
            "trug-btac-vs-plant-2008-2014-v1",
            "Turquoise Ridge Underground (NGM 61.5% Barrick / 38.5% Newmont)",
            "Autoclave BTAC vs Commercial Plant",
            "Turquoise-Ridge-Technical-Report-March2024.md:Table-13-2:p136-138",
            sha_hex,
            &lots,
            &policy,
        )
        .expect("build Turquoise Ridge E177 calibration card");

        // Exact reproduction of FACT E177 (71 paired lots + 12 missing BTAC lots = 83 total).
        assert_eq!(card.total_lots, 83);
        assert_eq!(card.paired_count, 71);
        assert_eq!(card.missing_lab_count, 12);
        // Mean plant - BTAC = +1667 / 710 = +2.347887... -> +2.35 percentage points.
        assert_eq!(card.mean_bias_pct_pts, ExactQty::new(1_667, 710).unwrap());
        // Sample SD = 4.487916... -> 4.49 percentage points.
        assert_eq!(
            card.sample_sd_pct_pts,
            ExactQty::new(4_487_916, 1_000_000).unwrap()
        );
        // Outlier counts: 17 lots > +5 pts, 1 lot < -5 pts (Lot #83: -5.4).
        assert_eq!(card.lots_strictly_above_threshold, 17);
        assert_eq!(card.lots_strictly_below_neg_threshold, 1);
        // 10 lots have exact ±0.1 pt pre-rounding differences in the printed Difference column.
        assert_eq!(card.printed_diff_rounding_discrepancies, 10);
        // Strictly causal expanding-window scoring on lots 11..=71 (61 out-of-sample lots):
        // Raw BTAC (±5 pts): 45/61 (73.77%), standardized bias +0.5232 -> Biased.
        // Bias-corrected causal 80% interval: 51/61 (83.61%), standardized bias +0.1223 -> Calibrated.
        assert_eq!(card.scored_online_count, 61);
        assert_eq!(card.raw_lab_covered_count, 45);
        assert_eq!(card.bias_corrected_covered_count, 51);
        assert_eq!(card.raw_lab_flag, RecoveryCalibrationFlag::Biased);
        assert_eq!(
            card.bias_corrected_flag,
            RecoveryCalibrationFlag::Calibrated
        );
    }
}
