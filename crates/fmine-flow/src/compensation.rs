//! Determinism: BitwisePortable
//! Within-factor compensation detector and `KA-TOUCH-KIBALI-2016-2025` known-answer audit
//! (`BD-280`, plan §20.4–§20.5, `FACT E154–E156`, `FACT E210–E212`).
//!
//! All public physical/ratio quantities use exact `fmine_units::ExactQty` (zero bare `f64`
//! parameters; L-6).
//!
//! Enforces two Touchstone laws:
//! 1. Within-factor compensation detection (§20.4): even inside a single reconciliation factor
//!    (such as NGM Goldstrike Roaster `MCF_out = 109% t / 94% g / 102% oz`, `FACT E154`),
//!    opposite-sign tonnes and grade errors can partially or wholly cancel in the metal ratio.
//!    When both `|tonnes_ratio - 1|` and `|grade_ratio - 1|` exceed the declared tolerance
//!    `tol_bps` in opposite directions, the period is flagged for an `InvestigationCase` rather
//!    than allowing the masked metal ratio to be read alone.
//! 2. Signed multi-year bias prior extraction (§20.5): computes an exact rational signed bias
//!    prior (mean, min, max, exact sample variance, sign persistence) over multi-year series such
//!    as `KA-TOUCH-KIBALI-2016-2025` (100% project basis; Barrick 45% attributable; `E210–E212`),
//!    plus check-out vs check-in MCF divergence (`E210`).

use std::collections::BTreeSet;

use fmine_units::ExactQty;

use crate::FlowError;

/// Classification of within-factor tonnes-vs-grade compensation for a single period.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CompensationClass {
    /// No opposite-sign tonnes/grade deviation exceeding tolerance.
    None,
    /// Tonnes and grade deviate beyond tolerance in opposite directions while metal sits within
    /// tolerance (`|metal_ratio - 1| <= tol`), fully masking the operational error.
    MaskedWithinTolerance,
    /// Tonnes and grade deviate beyond tolerance in opposite directions and partially offset each
    /// other (`|metal_ratio - 1| > tol` yet strictly smaller than `max(|t_dev|, |g_dev|)`).
    PartiallyMaskedOpposing,
}

impl CompensationClass {
    /// Returns `true` whenever opposite-sign compensation requires opening an `InvestigationCase`.
    #[must_use]
    pub const fn requires_investigation(self) -> bool {
        matches!(
            self,
            Self::MaskedWithinTolerance | Self::PartiallyMaskedOpposing
        )
    }
}

/// Input to the within-factor tonnes-vs-grade compensation detector for one site and period.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WithinFactorInput {
    pub site_id: String,
    pub period: String,
    /// Versioned F-factor / MCF registry key (`G-TOUCH-4`), e.g. `MCF_out.ngm@v1`.
    pub factor_key: String,
    /// Explicit ownership basis recorded with every figure (e.g. `"100% project; Barrick 45%"`).
    pub ownership_basis: String,
    /// Tonnes ratio (`actual / call`, e.g. `109/100` for `+9%`). Must be strictly positive.
    pub tonnes_ratio: ExactQty,
    /// Grade ratio (`actual / call`, e.g. `94/100` for `-6%`). Must be strictly positive.
    pub grade_ratio: ExactQty,
    /// Reported metal ratio (`actual / call`, e.g. `102/100` for `+2%`). Must be strictly positive.
    pub metal_ratio: ExactQty,
    /// Declared site/policy tolerance band in basis points (`1..=9999`, e.g. `500` = `±5%`).
    pub tol_bps: u32,
    /// Evidence register identifiers (`E...`) backing this observation.
    pub source_e_ids: Vec<String>,
}

/// Structured trigger emitted whenever within-factor compensation demands an `InvestigationCase`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvestigationTrigger {
    pub case_key: String,
    pub site_id: String,
    pub period: String,
    pub factor_key: String,
    pub ownership_basis: String,
    pub class: CompensationClass,
    pub summary: String,
    pub source_e_ids: Vec<String>,
}

/// Exact verdict from `detect_within_factor_compensation`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WithinFactorVerdict {
    pub site_id: String,
    pub period: String,
    pub factor_key: String,
    pub ownership_basis: String,
    pub tonnes_deviation: ExactQty,
    pub grade_deviation: ExactQty,
    pub metal_deviation: ExactQty,
    /// Exact discrepancy `(tonnes_ratio * grade_ratio) - metal_ratio` (captures source rounding
    /// or reporting anomalies without silently hiding them).
    pub product_minus_metal: ExactQty,
    pub tolerance: ExactQty,
    pub class: CompensationClass,
    pub compensation_detected: bool,
    pub fully_masked_metal: bool,
    pub investigation_trigger: Option<InvestigationTrigger>,
}

fn validate_non_empty(field: &str, value: &str) -> Result<(), FlowError> {
    if value.trim().is_empty() {
        return Err(FlowError::InvalidGraph {
            detail: format!("compensation field '{field}' must be non-empty"),
        });
    }
    Ok(())
}

fn fmt_qty(q: ExactQty) -> String {
    format!("{}/{}", q.num(), q.den())
}

fn validate_positive_ratio(field: &str, ratio: ExactQty) -> Result<(), FlowError> {
    if ratio.signum() <= 0 {
        return Err(FlowError::InvalidQuantity {
            detail: format!(
                "ratio '{field}' must be strictly positive, got {}",
                fmt_qty(ratio)
            ),
        });
    }
    Ok(())
}

fn validate_tol_bps(tol_bps: u32) -> Result<ExactQty, FlowError> {
    if tol_bps == 0 || tol_bps >= 10_000 {
        return Err(FlowError::InvalidQuantity {
            detail: format!("tol_bps must be in 1..=9999, got {tol_bps}"),
        });
    }
    Ok(ExactQty::new(i128::from(tol_bps), 10_000)?)
}

fn abs_qty(q: ExactQty) -> Result<ExactQty, FlowError> {
    if q.signum() < 0 {
        Ok(ExactQty::from_integer(0).checked_sub(q)?)
    } else {
        Ok(q)
    }
}

/// Detect within-factor tonnes-vs-grade compensating errors (`BD-280`, plan §20.4, `FACT E154`).
///
/// Refuses invalid (non-positive) ratios, empty metadata, or out-of-range `tol_bps` (`FMINE-REF-109`
/// / `FMINE-REF-107`) rather than clamping.
pub fn detect_within_factor_compensation(
    input: &WithinFactorInput,
) -> Result<WithinFactorVerdict, FlowError> {
    validate_non_empty("site_id", &input.site_id)?;
    validate_non_empty("period", &input.period)?;
    validate_non_empty("factor_key", &input.factor_key)?;
    validate_non_empty("ownership_basis", &input.ownership_basis)?;
    validate_positive_ratio("tonnes_ratio", input.tonnes_ratio)?;
    validate_positive_ratio("grade_ratio", input.grade_ratio)?;
    validate_positive_ratio("metal_ratio", input.metal_ratio)?;
    let tolerance = validate_tol_bps(input.tol_bps)?;

    let one = ExactQty::from_integer(1);
    let t_dev = input.tonnes_ratio.checked_sub(one)?;
    let g_dev = input.grade_ratio.checked_sub(one)?;
    let m_dev = input.metal_ratio.checked_sub(one)?;
    let product = input.tonnes_ratio.checked_mul(input.grade_ratio)?;
    let product_minus_metal = product.checked_sub(input.metal_ratio)?;

    let abs_t = abs_qty(t_dev)?;
    let abs_g = abs_qty(g_dev)?;
    let abs_m = abs_qty(m_dev)?;

    let opposite_signs =
        (t_dev.signum() > 0 && g_dev.signum() < 0) || (t_dev.signum() < 0 && g_dev.signum() > 0);
    let t_exceeds = abs_t.checked_cmp(tolerance)? == core::cmp::Ordering::Greater;
    let g_exceeds = abs_g.checked_cmp(tolerance)? == core::cmp::Ordering::Greater;
    let m_within = abs_m.checked_cmp(tolerance)? != core::cmp::Ordering::Greater;

    let max_component_dev = if abs_t.checked_cmp(abs_g)? == core::cmp::Ordering::Greater {
        abs_t
    } else {
        abs_g
    };
    let m_smaller_than_max = abs_m.checked_cmp(max_component_dev)? == core::cmp::Ordering::Less;

    let class = if opposite_signs && t_exceeds && g_exceeds {
        if m_within {
            CompensationClass::MaskedWithinTolerance
        } else if m_smaller_than_max {
            CompensationClass::PartiallyMaskedOpposing
        } else {
            CompensationClass::None
        }
    } else {
        CompensationClass::None
    };

    let compensation_detected = class.requires_investigation();
    let fully_masked_metal = class == CompensationClass::MaskedWithinTolerance;

    let investigation_trigger = if compensation_detected {
        let mode = if fully_masked_metal {
            "masked within metal tolerance"
        } else {
            "partially masked by opposing tonnes/grade deviations"
        };
        Some(InvestigationTrigger {
            case_key: format!(
                "INV-COMP:{}:{}:{}",
                input.site_id, input.factor_key, input.period
            ),
            site_id: input.site_id.clone(),
            period: input.period.clone(),
            factor_key: input.factor_key.clone(),
            ownership_basis: input.ownership_basis.clone(),
            class,
            summary: format!(
                "{} {} ({}) [{}]: within-factor compensation ({mode}); tonnes_dev={}, \
                 grade_dev={}, metal_dev={}, tol={}",
                input.site_id,
                input.period,
                input.factor_key,
                input.ownership_basis,
                fmt_qty(t_dev),
                fmt_qty(g_dev),
                fmt_qty(m_dev),
                fmt_qty(tolerance),
            ),
            source_e_ids: input.source_e_ids.clone(),
        })
    } else {
        None
    };

    Ok(WithinFactorVerdict {
        site_id: input.site_id.clone(),
        period: input.period.clone(),
        factor_key: input.factor_key.clone(),
        ownership_basis: input.ownership_basis.clone(),
        tonnes_deviation: t_dev,
        grade_deviation: g_dev,
        metal_deviation: m_dev,
        product_minus_metal,
        tolerance,
        class,
        compensation_detected,
        fully_masked_metal,
        investigation_trigger,
    })
}

/// One annual row in a multi-year reconciliation series (e.g. `KA-TOUCH-KIBALI-2016-2025`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnnualFactorRow {
    pub year: u16,
    pub tonnes_ratio: ExactQty,
    pub grade_ratio: ExactQty,
    pub metal_ratio: ExactQty,
}

impl AnnualFactorRow {
    /// Construct an annual row from integer percentage variances (e.g. `+31, -27, -4` → `131/100, 73/100, 96/100`).
    pub fn from_variance_pct(
        year: u16,
        tonnes_var_pct: i64,
        grade_var_pct: i64,
        metal_var_pct: i64,
    ) -> Result<Self, FlowError> {
        let tonnes_ratio = ExactQty::new(100_i128 + i128::from(tonnes_var_pct), 100)?;
        let grade_ratio = ExactQty::new(100_i128 + i128::from(grade_var_pct), 100)?;
        let metal_ratio = ExactQty::new(100_i128 + i128::from(metal_var_pct), 100)?;
        validate_positive_ratio("tonnes_ratio", tonnes_ratio)?;
        validate_positive_ratio("grade_ratio", grade_ratio)?;
        validate_positive_ratio("metal_ratio", metal_ratio)?;
        Ok(Self {
            year,
            tonnes_ratio,
            grade_ratio,
            metal_ratio,
        })
    }
}

/// Exact rational signed bias prior fitted across a multi-year reconciliation series (§20.4–§20.5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignedBiasPrior {
    pub n_periods: u32,
    /// `true` iff every period's metal bias `(metal_ratio - 1)` is strictly on the same side of 0.
    pub all_same_sign: bool,
    /// `-1` if all negative, `+1` if all positive, `0` if mixed or zero.
    pub persistent_sign: i8,
    /// Exact arithmetic mean of `(metal_ratio - 1)` across the series.
    pub mean_metal_bias: ExactQty,
    /// Exact arithmetic mean of `(tonnes_ratio - 1)` across the series.
    pub mean_tonnes_bias: ExactQty,
    /// Exact arithmetic mean of `(grade_ratio - 1)` across the series.
    pub mean_grade_bias: ExactQty,
    pub min_metal_bias: ExactQty,
    pub max_metal_bias: ExactQty,
    /// Exact unbiased sample variance `Σ(b_i - mean)^2 / (n - 1)` when `n >= 2`, else `None`.
    pub sample_variance_metal: Option<ExactQty>,
}

/// Exact check-out vs check-in MCF divergence diagnostic (e.g. Kibali 2025 Table 15-26, `E210`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McfCheckOutDivergence {
    pub gc_call_oz: ExactQty,
    pub check_in_oz: ExactQty,
    pub check_out_oz: ExactQty,
    /// Exact `check_in_oz / gc_call_oz`.
    pub mcf_in_ratio: ExactQty,
    /// Exact `check_out_oz / gc_call_oz`.
    pub mcf_out_ratio: ExactQty,
    /// Exact `(check_out_oz - check_in_oz) / check_in_oz` ("unaccounted gains" when positive).
    pub checkout_over_checkin_delta: ExactQty,
}

/// Compute exact check-in / check-out MCF ratios and their divergence (never clamping non-positive inputs).
pub fn audit_mcf_checkout_vs_checkin(
    gc_call_oz: ExactQty,
    check_in_oz: ExactQty,
    check_out_oz: ExactQty,
) -> Result<McfCheckOutDivergence, FlowError> {
    validate_positive_ratio("gc_call_oz", gc_call_oz)?;
    validate_positive_ratio("check_in_oz", check_in_oz)?;
    validate_positive_ratio("check_out_oz", check_out_oz)?;
    let mcf_in_ratio = check_in_oz.checked_div(gc_call_oz)?;
    let mcf_out_ratio = check_out_oz.checked_div(gc_call_oz)?;
    let diff = check_out_oz.checked_sub(check_in_oz)?;
    let checkout_over_checkin_delta = diff.checked_div(check_in_oz)?;
    Ok(McfCheckOutDivergence {
        gc_call_oz,
        check_in_oz,
        check_out_oz,
        mcf_in_ratio,
        mcf_out_ratio,
        checkout_over_checkin_delta,
    })
}

/// Multi-year series audit combining per-year compensation verdicts and the fitted signed bias prior.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MultiYearSeriesAudit {
    pub site_id: String,
    pub factor_key: String,
    pub ownership_basis: String,
    pub annual_verdicts: Vec<WithinFactorVerdict>,
    pub masked_years: Vec<u16>,
    pub partially_masked_years: Vec<u16>,
    pub flagged_years: Vec<u16>,
    pub bias_prior: SignedBiasPrior,
}

/// Audit a multi-year reconciliation series for within-factor compensation and fit an exact
/// signed bias prior (`BD-280`, `KA-TOUCH-KIBALI-2016-2025`, plan §20.4–§20.5).
pub fn audit_multi_year_series(
    site_id: &str,
    factor_key: &str,
    ownership_basis: &str,
    rows: &[AnnualFactorRow],
    tol_bps: u32,
    source_e_ids: &[String],
) -> Result<MultiYearSeriesAudit, FlowError> {
    validate_non_empty("site_id", site_id)?;
    validate_non_empty("factor_key", factor_key)?;
    validate_non_empty("ownership_basis", ownership_basis)?;
    let _ = validate_tol_bps(tol_bps)?;
    if rows.is_empty() {
        return Err(FlowError::InvalidQuantity {
            detail: "multi-year reconciliation series must contain at least one row".to_string(),
        });
    }

    let mut seen_years = BTreeSet::new();
    let mut prev_year: Option<u16> = None;
    let mut annual_verdicts = Vec::with_capacity(rows.len());
    let mut masked_years = Vec::new();
    let mut partially_masked_years = Vec::new();
    let mut flagged_years = Vec::new();

    let mut sum_m = ExactQty::from_integer(0);
    let mut sum_t = ExactQty::from_integer(0);
    let mut sum_g = ExactQty::from_integer(0);
    let mut min_m: Option<ExactQty> = None;
    let mut max_m: Option<ExactQty> = None;
    let mut all_neg = true;
    let mut all_pos = true;

    for row in rows {
        if !seen_years.insert(row.year) {
            return Err(FlowError::DuplicateId {
                kind: "reconciliation_year",
                id: row.year.to_string(),
            });
        }
        if let Some(prev) = prev_year
            && row.year < prev
        {
            return Err(FlowError::InvalidGraph {
                detail: format!(
                    "multi-year series must be ordered chronologically: {} < {}",
                    row.year, prev
                ),
            });
        }
        prev_year = Some(row.year);

        let verdict = detect_within_factor_compensation(&WithinFactorInput {
            site_id: site_id.to_string(),
            period: row.year.to_string(),
            factor_key: factor_key.to_string(),
            ownership_basis: ownership_basis.to_string(),
            tonnes_ratio: row.tonnes_ratio,
            grade_ratio: row.grade_ratio,
            metal_ratio: row.metal_ratio,
            tol_bps,
            source_e_ids: source_e_ids.to_vec(),
        })?;

        match verdict.class {
            CompensationClass::MaskedWithinTolerance => {
                masked_years.push(row.year);
                flagged_years.push(row.year);
            }
            CompensationClass::PartiallyMaskedOpposing => {
                partially_masked_years.push(row.year);
                flagged_years.push(row.year);
            }
            CompensationClass::None => {}
        }

        let m_dev = verdict.metal_deviation;
        if m_dev.signum() >= 0 {
            all_neg = false;
        }
        if m_dev.signum() <= 0 {
            all_pos = false;
        }
        sum_m = sum_m.checked_add(m_dev)?;
        sum_t = sum_t.checked_add(verdict.tonnes_deviation)?;
        sum_g = sum_g.checked_add(verdict.grade_deviation)?;

        min_m = Some(match min_m {
            Some(cur) if cur.checked_cmp(m_dev)? != core::cmp::Ordering::Greater => cur,
            _ => m_dev,
        });
        max_m = Some(match max_m {
            Some(cur) if cur.checked_cmp(m_dev)? != core::cmp::Ordering::Less => cur,
            _ => m_dev,
        });

        annual_verdicts.push(verdict);
    }

    let n_u32 = u32::try_from(rows.len()).map_err(|_| FlowError::Overflow)?;
    let n_qty = ExactQty::from_integer(i128::from(n_u32));
    let mean_metal_bias = sum_m.checked_div(n_qty)?;
    let mean_tonnes_bias = sum_t.checked_div(n_qty)?;
    let mean_grade_bias = sum_g.checked_div(n_qty)?;

    let sample_variance_metal = if n_u32 >= 2 {
        let mut sq_sum = ExactQty::from_integer(0);
        for v in &annual_verdicts {
            let diff = v.metal_deviation.checked_sub(mean_metal_bias)?;
            let sq = diff.checked_mul(diff)?;
            sq_sum = sq_sum.checked_add(sq)?;
        }
        Some(sq_sum.checked_div(ExactQty::from_integer(i128::from(n_u32 - 1)))?)
    } else {
        None
    };

    let (all_same_sign, persistent_sign) = if all_neg {
        (true, -1_i8)
    } else if all_pos {
        (true, 1_i8)
    } else {
        (false, 0_i8)
    };

    Ok(MultiYearSeriesAudit {
        site_id: site_id.to_string(),
        factor_key: factor_key.to_string(),
        ownership_basis: ownership_basis.to_string(),
        annual_verdicts,
        masked_years,
        partially_masked_years,
        flagged_years,
        bias_prior: SignedBiasPrior {
            n_periods: n_u32,
            all_same_sign,
            persistent_sign,
            mean_metal_bias,
            mean_tonnes_bias,
            mean_grade_bias,
            min_metal_bias: min_m.unwrap_or(ExactQty::from_integer(0)),
            max_metal_bias: max_m.unwrap_or(ExactQty::from_integer(0)),
            sample_variance_metal,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use fmine_evidence::RefusalCode;

    /// `BD-280` Known Answer 1 (`FACT E154–E156`): Goldstrike Roaster 2024 `109% t / 94% g / 102% oz`
    /// is flagged as `MaskedWithinTolerance` with an `InvestigationTrigger`; negative controls
    /// Gold Quarry (`E155`: `99/97/96%`) and Goldstrike Autoclave (`E156`: `99/98/97%`) are not flagged.
    #[test]
    fn test_goldstrike_roaster_2024_within_factor_compensation_and_negative_controls() {
        let roaster = detect_within_factor_compensation(&WithinFactorInput {
            site_id: "ngm_goldstrike_roaster".to_string(),
            period: "2024".to_string(),
            factor_key: "MCF_out.ngm@v1".to_string(),
            ownership_basis: "100% project; Barrick 61.5% / Newmont 38.5%".to_string(),
            tonnes_ratio: ExactQty::new(109, 100).unwrap(),
            grade_ratio: ExactQty::new(94, 100).unwrap(),
            metal_ratio: ExactQty::new(102, 100).unwrap(),
            tol_bps: 500, // ±5%
            source_e_ids: vec!["E154".to_string()],
        })
        .unwrap();

        assert_eq!(roaster.class, CompensationClass::MaskedWithinTolerance);
        assert!(roaster.compensation_detected);
        assert!(roaster.fully_masked_metal);
        assert_eq!(roaster.tonnes_deviation, ExactQty::new(9, 100).unwrap());
        assert_eq!(roaster.grade_deviation, ExactQty::new(-6, 100).unwrap());
        assert_eq!(roaster.metal_deviation, ExactQty::new(2, 100).unwrap());
        // 1.09 * 0.94 - 1.02 = 1.0246 - 1.02 = 0.0046 = 23/5000 (published integer rounding).
        assert_eq!(
            roaster.product_minus_metal,
            ExactQty::new(23, 5000).unwrap()
        );
        let trigger = roaster
            .investigation_trigger
            .as_ref()
            .expect("roaster requires investigation");
        assert_eq!(
            trigger.case_key,
            "INV-COMP:ngm_goldstrike_roaster:MCF_out.ngm@v1:2024"
        );
        assert!(trigger.summary.contains("masked within metal tolerance"));

        // Negative control 1: Gold Quarry 2024 (FACT E155: 99% t / 97% g / 96% oz).
        let quarry = detect_within_factor_compensation(&WithinFactorInput {
            site_id: "ngm_gold_quarry".to_string(),
            period: "2024".to_string(),
            factor_key: "MCF_out.ngm@v1".to_string(),
            ownership_basis: "100% project; Barrick 61.5% / Newmont 38.5%".to_string(),
            tonnes_ratio: ExactQty::new(99, 100).unwrap(),
            grade_ratio: ExactQty::new(97, 100).unwrap(),
            metal_ratio: ExactQty::new(96, 100).unwrap(),
            tol_bps: 500,
            source_e_ids: vec!["E155".to_string()],
        })
        .unwrap();
        assert_eq!(quarry.class, CompensationClass::None);
        assert!(!quarry.compensation_detected);
        assert!(quarry.investigation_trigger.is_none());

        // Negative control 2: Goldstrike Autoclave 2024 (FACT E156: 99% t / 98% g / 97% oz).
        let autoclave = detect_within_factor_compensation(&WithinFactorInput {
            site_id: "ngm_goldstrike_autoclave".to_string(),
            period: "2024".to_string(),
            factor_key: "MCF_out.ngm@v1".to_string(),
            ownership_basis: "100% project; Barrick 61.5% / Newmont 38.5%".to_string(),
            tonnes_ratio: ExactQty::new(99, 100).unwrap(),
            grade_ratio: ExactQty::new(98, 100).unwrap(),
            metal_ratio: ExactQty::new(97, 100).unwrap(),
            tol_bps: 500,
            source_e_ids: vec!["E156".to_string()],
        })
        .unwrap();
        assert_eq!(autoclave.class, CompensationClass::None);
        assert!(!autoclave.compensation_detected);
        assert!(autoclave.investigation_trigger.is_none());
    }

    /// `BD-280` Known Answer 2 (`KA-TOUCH-KIBALI-2016-2025`, `FACT E210–E212`, 100% project basis;
    /// Barrick 45% attributable; Kibali 2026 TRS Tables 15-24, 15-25, 15-26):
    /// - 10-year OP GC call vs raw GC block model (Table 15-25): every year negative on metal
    ///   (`-2%` to `-13%`), exact signed bias prior mean `-6.5%` (`-13/200`), fully masked
    ///   compensation in 2016, 2018, 2024 and partially masked opposing compensation in 2023, 2025;
    /// - 2025 dig-block reconciliation (Table 15-24): `+13% t / -13% g / -1% oz` flagged as
    ///   `MaskedWithinTolerance`;
    /// - 2025 MCF detail (Table 15-26): `MCF_in = 727,674 / 742,630 ≈ 98.0%`,
    ///   `MCF_out = 746,649 / 742,630 ≈ 100.5%`, check-out over check-in `+2.61%` (`18,975 / 727,674`).
    #[test]
    fn test_ka_touch_kibali_2016_2025_series_compensation_and_signed_bias_prior() {
        let rows = vec![
            AnnualFactorRow::from_variance_pct(2016, -11, 9, -4).unwrap(),
            AnnualFactorRow::from_variance_pct(2017, -5, 2, -3).unwrap(),
            AnnualFactorRow::from_variance_pct(2018, 8, -9, -2).unwrap(),
            AnnualFactorRow::from_variance_pct(2019, 4, -6, -3).unwrap(),
            AnnualFactorRow::from_variance_pct(2020, 0, -8, -8).unwrap(),
            AnnualFactorRow::from_variance_pct(2021, -6, -7, -13).unwrap(),
            AnnualFactorRow::from_variance_pct(2022, 3, -16, -13).unwrap(),
            AnnualFactorRow::from_variance_pct(2023, 18, -21, -7).unwrap(),
            AnnualFactorRow::from_variance_pct(2024, 31, -27, -4).unwrap(),
            AnnualFactorRow::from_variance_pct(2025, 27, -27, -8).unwrap(),
        ];

        let audit = audit_multi_year_series(
            "barrick_kibali_op",
            "GC_call_vs_GC_model.barrick_kibali@v1",
            "100% project; Barrick 45% attributable",
            &rows,
            500, // ±5%
            &["E210".to_string(), "E211".to_string(), "E212".to_string()],
        )
        .unwrap();

        assert_eq!(audit.bias_prior.n_periods, 10);
        assert!(audit.bias_prior.all_same_sign);
        assert_eq!(audit.bias_prior.persistent_sign, -1);
        // Sum of annual metal biases: (-4 -3 -2 -3 -8 -13 -13 -7 -4 -8) / 100 = -65 / 100 -> mean = -13 / 200 = -6.5%.
        assert_eq!(
            audit.bias_prior.mean_metal_bias,
            ExactQty::new(-13, 200).unwrap()
        );
        assert_eq!(
            audit.bias_prior.min_metal_bias,
            ExactQty::new(-13, 100).unwrap()
        );
        assert_eq!(
            audit.bias_prior.max_metal_bias,
            ExactQty::new(-2, 100).unwrap()
        );
        // Exact sum of squared deviations around -0.065:
        // (2.5^2 + 3.5^2 + 4.5^2 + 3.5^2 + (-1.5)^2 + (-6.5)^2 + (-6.5)^2 + (-0.5)^2 + 2.5^2 + (-1.5)^2) / 10000
        // = (6.25 + 12.25 + 20.25 + 12.25 + 2.25 + 42.25 + 42.25 + 0.25 + 6.25 + 2.25) / 10000 = 146.5 / 10000 = 293 / 20000.
        // Unbiased sample variance (divide by n-1 = 9) = 293 / 180000.
        assert_eq!(
            audit.bias_prior.sample_variance_metal,
            Some(ExactQty::new(293, 180_000).unwrap())
        );

        // Compensation flags: 2016, 2018, 2024 fully masked within ±5% metal; 2023, 2025 partially masked.
        assert_eq!(audit.masked_years, vec![2016, 2018, 2024]);
        assert_eq!(audit.partially_masked_years, vec![2023, 2025]);
        assert_eq!(audit.flagged_years, vec![2016, 2018, 2023, 2024, 2025]);
        // 2021 (-6% t, -7% g, -13% oz) is same-sign compounding loss, NOT compensation!
        let y2021 = &audit.annual_verdicts[5];
        assert_eq!(y2021.period, "2021");
        assert_eq!(y2021.class, CompensationClass::None);

        // 10-year cumulative total (Table 15-25): +7% t / -12% g / -6% oz -> partially masked opposing.
        let total_10y = detect_within_factor_compensation(&WithinFactorInput {
            site_id: "barrick_kibali_op".to_string(),
            period: "2016-2025".to_string(),
            factor_key: "GC_call_vs_GC_model.barrick_kibali@v1".to_string(),
            ownership_basis: "100% project; Barrick 45% attributable".to_string(),
            tonnes_ratio: ExactQty::new(47_054_265, 44_027_009).unwrap(),
            grade_ratio: ExactQty::new(204, 232).unwrap(),
            metal_ratio: ExactQty::new(3_092_735, 3_290_593).unwrap(),
            tol_bps: 500,
            source_e_ids: vec!["E211".to_string()],
        })
        .unwrap();
        assert_eq!(total_10y.class, CompensationClass::PartiallyMaskedOpposing);
        assert!(total_10y.compensation_detected);

        // 2025 dig-block reconciliation (Table 15-24): 6,354,121 t @ 1.51 = 309,191 oz vs 5,632,144 t @ 1.73 = 313,873 oz (+13% / -13% / -1%).
        let dig_2025 = detect_within_factor_compensation(&WithinFactorInput {
            site_id: "barrick_kibali_op_digblock".to_string(),
            period: "2025".to_string(),
            factor_key: "GC_call_vs_GC_model.barrick_kibali@v1".to_string(),
            ownership_basis: "100% project; Barrick 45% attributable".to_string(),
            tonnes_ratio: ExactQty::new(6_354_121, 5_632_144).unwrap(),
            grade_ratio: ExactQty::new(151, 173).unwrap(),
            metal_ratio: ExactQty::new(309_191, 313_873).unwrap(),
            tol_bps: 500,
            source_e_ids: vec!["E211".to_string()],
        })
        .unwrap();
        assert_eq!(dig_2025.class, CompensationClass::MaskedWithinTolerance);
        assert!(dig_2025.fully_masked_metal);

        // 2025 MCF check-out vs check-in (Table 15-26, E210): 742,630 GC call / 727,674 check-in / 746,649 check-out.
        let mcf_2025 = audit_mcf_checkout_vs_checkin(
            ExactQty::from_integer(742_630),
            ExactQty::from_integer(727_674),
            ExactQty::from_integer(746_649),
        )
        .unwrap();
        assert_eq!(
            mcf_2025.mcf_in_ratio,
            ExactQty::new(727_674, 742_630).unwrap()
        );
        assert_eq!(
            mcf_2025.mcf_out_ratio,
            ExactQty::new(746_649, 742_630).unwrap()
        );
        // Exact check-out over check-in = +18,975 / 727,674 (~+2.6076%, matching E210 "+2.6%").
        assert_eq!(
            mcf_2025.checkout_over_checkin_delta,
            ExactQty::new(18_975, 727_674).unwrap()
        );
        // Verify strictly in (2.60%, 2.62%).
        assert_eq!(
            mcf_2025
                .checkout_over_checkin_delta
                .checked_cmp(ExactQty::new(260, 10_000).unwrap())
                .unwrap(),
            core::cmp::Ordering::Greater
        );
        assert_eq!(
            mcf_2025
                .checkout_over_checkin_delta
                .checked_cmp(ExactQty::new(262, 10_000).unwrap())
                .unwrap(),
            core::cmp::Ordering::Less
        );
    }

    #[test]
    fn test_within_factor_compensation_refuses_invalid_inputs() {
        let base = WithinFactorInput {
            site_id: "site".to_string(),
            period: "2024".to_string(),
            factor_key: "MCF_out.ngm@v1".to_string(),
            ownership_basis: "100%".to_string(),
            tonnes_ratio: ExactQty::new(100, 100).unwrap(),
            grade_ratio: ExactQty::new(100, 100).unwrap(),
            metal_ratio: ExactQty::new(100, 100).unwrap(),
            tol_bps: 500,
            source_e_ids: vec![],
        };

        // Zero / negative ratio refused with FMINE-REF-109.
        let mut zero_t = base.clone();
        zero_t.tonnes_ratio = ExactQty::from_integer(0);
        assert_eq!(
            detect_within_factor_compensation(&zero_t)
                .unwrap_err()
                .to_refusal()
                .code,
            RefusalCode::OutsidePhysicalDomain109.spec().code
        );

        // Out-of-bounds tol_bps refused with FMINE-REF-109.
        let mut bad_tol = base.clone();
        bad_tol.tol_bps = 0;
        assert_eq!(
            detect_within_factor_compensation(&bad_tol)
                .unwrap_err()
                .to_refusal()
                .code,
            RefusalCode::OutsidePhysicalDomain109.spec().code
        );
        bad_tol.tol_bps = 10_000;
        assert_eq!(
            detect_within_factor_compensation(&bad_tol)
                .unwrap_err()
                .to_refusal()
                .code,
            RefusalCode::OutsidePhysicalDomain109.spec().code
        );

        // Empty ownership basis refused with FMINE-REF-107.
        let mut empty_basis = base;
        empty_basis.ownership_basis = "  ".to_string();
        assert_eq!(
            detect_within_factor_compensation(&empty_basis)
                .unwrap_err()
                .to_refusal()
                .code,
            RefusalCode::UndeclaredUnitBasisOrTz107.spec().code
        );

        // Duplicate or out-of-order years and empty series refused.
        assert!(
            audit_multi_year_series("s", "k@v1", "100%", &[], 500, &[]).is_err(),
            "empty series must refuse"
        );
        let dup = vec![
            AnnualFactorRow::from_variance_pct(2024, 0, 0, 0).unwrap(),
            AnnualFactorRow::from_variance_pct(2024, 1, -1, 0).unwrap(),
        ];
        assert!(matches!(
            audit_multi_year_series("s", "k@v1", "100%", &dup, 500, &[]),
            Err(FlowError::DuplicateId { .. })
        ));
        let rev = vec![
            AnnualFactorRow::from_variance_pct(2025, 0, 0, 0).unwrap(),
            AnnualFactorRow::from_variance_pct(2024, 0, 0, 0).unwrap(),
        ];
        assert!(matches!(
            audit_multi_year_series("s", "k@v1", "100%", &rev, 500, &[]),
            Err(FlowError::InvalidGraph { .. })
        ));
    }
}
