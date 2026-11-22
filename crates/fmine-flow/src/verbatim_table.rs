//! Determinism: BitwisePortable
//! Verbatim reconciliation-table importer, identity recomputation, `G-TOUCH-6` residual-row
//! `NOT_DEFINED` grade enforcement, and `DiscrepancyRecord` filing (`BD-279`, plan §20.4,
//! `FACT E206–E209`).
//!
//! All public quantities use exact `fmine_units::ExactQty` (zero bare `f64` parameters; L-6).
//! Enforces three Touchstone integrity laws:
//! 1. Verbatim ingestion: published tonnes, grades, ounces, and ratios are retained as printed
//!    without silently editing or plugging source figures.
//! 2. Exact identity recomputation: `recomputed_oz = tonnes * grade_g_per_t / 31.1034768` and
//!    exact numerator/denominator ratios are computed and compared against printed values; every
//!    mismatch beyond tolerance files a typed [`DiscrepancyRecord`].
//! 3. `G-TOUCH-6`: any residual (difference) row (e.g. Kibali `"GC Adjustment"`, Loulo
//!    `"Stockpile Change"`) stores `(Δt, Δoz)` with `grade_g_per_t = DerivedGrade::NotDefined`
//!    and files a `DiscrepancyRecord` if the source table printed a grade on that row.

use fmine_units::ExactQty;

use crate::FlowError;
use crate::ffactors::format_exact_pct_4dp;

/// Exact SI definition of one troy ounce in grams (`31.1034768 g = 311_034_768 / 10_000_000 g`).
pub fn grams_per_troy_oz() -> ExactQty {
    // 311_034_768 / 10_000_000 is exact and non-zero; reduce cleanly via `ExactQty::new`.
    ExactQty::new(311_034_768, 10_000_000).unwrap_or_else(|_| ExactQty::from_integer(31))
}

/// Role of a verbatim reconciliation-table row (`G-TOUCH-6`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RowRole {
    /// Physical ore / feed / check-out stream (`tonnes >= 0`, `metal_oz >= 0`, grade defined when `tonnes > 0`).
    PhysicalStream,
    /// Residual or difference row (`Δt`, `Δoz` may have arbitrary signs; grade is `NOT_DEFINED` per `G-TOUCH-6`).
    ResidualDifference,
}

/// Grade representation after `G-TOUCH-6` enforcement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DerivedGrade {
    /// Validated physical grade in `g/t` on a non-residual stream.
    Defined(ExactQty),
    /// Grade is mathematically undefined (`G-TOUCH-6` residual/difference row or zero-tonnage row).
    NotDefined { reason: String },
}

/// Classification of a filed reconciliation discrepancy (`BD-279`, `E206–E209`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DiscrepancyKind {
    /// `tonnes * grade_g_per_t / 31.1034768` disagrees with published `metal_oz` beyond tolerance (e.g. E209).
    MetalMassIdentityMismatch,
    /// Recomputed `numerator / denominator` disagrees with published ratio beyond tolerance (e.g. E206).
    RatioIdentityMismatch,
    /// `G-TOUCH-6`: a residual/difference row printed a grade (e.g. Kibali `-31.15 g/t` E207, Loulo `-0.54 g/t` E208).
    ResidualRowPrintedGrade,
}

/// Immutable discrepancy record filed whenever published reconciliation tables break exact identities.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscrepancyRecord {
    pub table_id: String,
    pub row_label: String,
    pub source_locator: String,
    pub source_e_id: String,
    pub kind: DiscrepancyKind,
    pub printed_value: String,
    pub recomputed_value: String,
    pub detail: String,
}

/// Raw verbatim row extracted from a published reconciliation table (`BD-279`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerbatimTableRowInput {
    pub table_id: String,
    pub row_label: String,
    pub source_locator: String,
    pub source_e_id: String,
    pub role: RowRole,
    pub printed_tonnes: ExactQty,
    pub printed_grade_g_per_t: Option<ExactQty>,
    pub printed_metal_oz: ExactQty,
    /// Relative metal-mass identity tolerance in basis points (e.g. `100` = `1.0%` for rounding in printed tables).
    pub metal_identity_tol_bps: u32,
}

/// Result of importing one verbatim reconciliation-table row under `G-TOUCH-6` and exact identity checks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportedReconciliationRow {
    pub table_id: String,
    pub row_label: String,
    pub source_locator: String,
    pub source_e_id: String,
    pub role: RowRole,
    pub tonnes: ExactQty,
    pub metal_oz: ExactQty,
    pub grade_g_per_t: DerivedGrade,
    pub recomputed_metal_oz: Option<ExactQty>,
    pub discrepancies: Vec<DiscrepancyRecord>,
}

/// Pair of verbatim reconciliation rows with published `(tonnes_ratio, grade_ratio, metal_ratio)`
/// for exact ratio identity auditing (e.g. Kibali 2024 MCF row, `FACT E206`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerbatimRatioRowInput {
    pub table_id: String,
    pub row_label: String,
    pub source_locator: String,
    pub source_e_id: String,
    pub denominator_tonnes: ExactQty,
    pub denominator_grade_g_per_t: ExactQty,
    pub denominator_metal_oz: ExactQty,
    pub numerator_tonnes: ExactQty,
    pub numerator_grade_g_per_t: ExactQty,
    pub numerator_metal_oz: ExactQty,
    pub printed_tonnes_ratio: ExactQty,
    pub printed_grade_ratio: ExactQty,
    pub printed_metal_ratio: ExactQty,
    /// Absolute ratio tolerance in basis points (e.g. `100` = `±1.0%` for integer-% printed columns).
    pub ratio_tol_bps: u32,
}

/// Recomputed exact ratios and any filed [`DiscrepancyRecord`]s for a two-stream ratio row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerbatimRatioAudit {
    pub recomputed_tonnes_ratio: ExactQty,
    pub recomputed_grade_ratio: ExactQty,
    pub recomputed_metal_ratio: ExactQty,
    pub discrepancies: Vec<DiscrepancyRecord>,
}

fn abs_exact(q: ExactQty) -> Result<ExactQty, FlowError> {
    if q.signum() < 0 {
        Ok(q.checked_neg()?)
    } else {
        Ok(q)
    }
}

fn format_exact_2dp(q: ExactQty) -> String {
    let sign = if q.num() < 0 { "-" } else { "" };
    let num_abs = q.num().unsigned_abs();
    let den_abs = q.den().unsigned_abs();
    let scaled = num_abs.saturating_mul(100);
    let rounded = scaled.saturating_add(den_abs / 2) / den_abs;
    let whole = rounded / 100;
    let frac = rounded % 100;
    format!("{sign}{whole}.{frac:02}")
}

/// Import one verbatim reconciliation-table row, enforcing `G-TOUCH-6` on residual rows and
/// recomputing `tonnes × grade ÷ 31.1034768` on physical streams (`BD-279`).
pub fn import_verbatim_table_row(
    input: &VerbatimTableRowInput,
) -> Result<ImportedReconciliationRow, FlowError> {
    if input.table_id.trim().is_empty()
        || input.row_label.trim().is_empty()
        || input.source_locator.trim().is_empty()
        || input.source_e_id.trim().is_empty()
    {
        return Err(FlowError::InvalidGraph {
            detail: "verbatim table row requires non-empty table_id, row_label, source_locator, and source_e_id"
                .to_string(),
        });
    }
    if input.metal_identity_tol_bps > 10_000 {
        return Err(FlowError::InvalidQuantity {
            detail: format!(
                "metal_identity_tol_bps {} exceeds 10000",
                input.metal_identity_tol_bps
            ),
        });
    }

    let mut discrepancies = Vec::new();
    match input.role {
        RowRole::ResidualDifference => {
            // G-TOUCH-6: store (Δt, Δoz) with grade NOT_DEFINED; file a DiscrepancyRecord if source printed a grade.
            if let Some(printed_g) = input.printed_grade_g_per_t {
                discrepancies.push(DiscrepancyRecord {
                    table_id: input.table_id.clone(),
                    row_label: input.row_label.clone(),
                    source_locator: input.source_locator.clone(),
                    source_e_id: input.source_e_id.clone(),
                    kind: DiscrepancyKind::ResidualRowPrintedGrade,
                    printed_value: format!("{} g/t", format_exact_2dp(printed_g)),
                    recomputed_value: "NOT_DEFINED".to_string(),
                    detail: format!(
                        "G-TOUCH-6: residual difference row '{}' printed a grade ({} g/t); stored (Δt, Δoz) with grade NOT_DEFINED",
                        input.row_label,
                        format_exact_2dp(printed_g)
                    ),
                });
            }
            Ok(ImportedReconciliationRow {
                table_id: input.table_id.clone(),
                row_label: input.row_label.clone(),
                source_locator: input.source_locator.clone(),
                source_e_id: input.source_e_id.clone(),
                role: RowRole::ResidualDifference,
                tonnes: input.printed_tonnes,
                metal_oz: input.printed_metal_oz,
                grade_g_per_t: DerivedGrade::NotDefined {
                    reason: "G-TOUCH-6 residual/difference row carries (Δt, Δoz) only".to_string(),
                },
                recomputed_metal_oz: None,
                discrepancies,
            })
        }
        RowRole::PhysicalStream => {
            if input.printed_tonnes.signum() < 0 || input.printed_metal_oz.signum() < 0 {
                return Err(FlowError::InvalidQuantity {
                    detail: format!(
                        "physical stream '{}' cannot have negative tonnes or metal_oz",
                        input.row_label
                    ),
                });
            }
            let (grade_g_per_t, recomputed_metal_oz) = if input.printed_tonnes.signum() == 0 {
                (
                    DerivedGrade::NotDefined {
                        reason: "zero tonnes: intensive grade undefined".to_string(),
                    },
                    Some(ExactQty::from_integer(0)),
                )
            } else if let Some(g) = input.printed_grade_g_per_t {
                if g.signum() < 0 {
                    return Err(FlowError::InvalidQuantity {
                        detail: format!(
                            "physical stream '{}' cannot have negative grade",
                            input.row_label
                        ),
                    });
                }
                let grams = input.printed_tonnes.checked_mul(g)?;
                let rec_oz = grams.checked_div(grams_per_troy_oz())?;
                let diff = abs_exact(rec_oz.checked_sub(input.printed_metal_oz)?)?;
                let tol_frac = ExactQty::new(i128::from(input.metal_identity_tol_bps), 10_000)?;
                let scale = if rec_oz.signum() > 0 {
                    rec_oz
                } else {
                    ExactQty::from_integer(1)
                };
                let max_err = scale.checked_mul(tol_frac)?;
                if diff.checked_cmp(max_err)? == core::cmp::Ordering::Greater {
                    discrepancies.push(DiscrepancyRecord {
                        table_id: input.table_id.clone(),
                        row_label: input.row_label.clone(),
                        source_locator: input.source_locator.clone(),
                        source_e_id: input.source_e_id.clone(),
                        kind: DiscrepancyKind::MetalMassIdentityMismatch,
                        printed_value: format!("{} oz", format_exact_2dp(input.printed_metal_oz)),
                        recomputed_value: format!("{} oz", format_exact_2dp(rec_oz)),
                        detail: format!(
                            "verbatim row '{}': t * g / 31.1034768 = {} oz contradicts printed {} oz",
                            input.row_label,
                            format_exact_2dp(rec_oz),
                            format_exact_2dp(input.printed_metal_oz)
                        ),
                    });
                }
                (DerivedGrade::Defined(g), Some(rec_oz))
            } else {
                // Derive grade from metal_oz * 31.1034768 / tonnes when no grade column was printed.
                let grams = input.printed_metal_oz.checked_mul(grams_per_troy_oz())?;
                let derived_g = grams.checked_div(input.printed_tonnes)?;
                (
                    DerivedGrade::Defined(derived_g),
                    Some(input.printed_metal_oz),
                )
            };

            Ok(ImportedReconciliationRow {
                table_id: input.table_id.clone(),
                row_label: input.row_label.clone(),
                source_locator: input.source_locator.clone(),
                source_e_id: input.source_e_id.clone(),
                role: RowRole::PhysicalStream,
                tonnes: input.printed_tonnes,
                metal_oz: input.printed_metal_oz,
                grade_g_per_t,
                recomputed_metal_oz,
                discrepancies,
            })
        }
    }
}

/// Audit published `(tonnes_ratio, grade_ratio, metal_ratio)` columns against exact ratios
/// recomputed from printed numerator and denominator cells (`BD-279`, `FACT E206`).
pub fn audit_verbatim_ratio_row(
    input: &VerbatimRatioRowInput,
) -> Result<VerbatimRatioAudit, FlowError> {
    if input.table_id.trim().is_empty()
        || input.row_label.trim().is_empty()
        || input.source_locator.trim().is_empty()
        || input.source_e_id.trim().is_empty()
    {
        return Err(FlowError::InvalidGraph {
            detail: "verbatim ratio row requires non-empty metadata fields".to_string(),
        });
    }
    if input.ratio_tol_bps > 10_000 {
        return Err(FlowError::InvalidQuantity {
            detail: format!("ratio_tol_bps {} exceeds 10000", input.ratio_tol_bps),
        });
    }
    if input.denominator_tonnes.signum() <= 0
        || input.denominator_grade_g_per_t.signum() <= 0
        || input.denominator_metal_oz.signum() <= 0
        || input.numerator_tonnes.signum() < 0
        || input.numerator_grade_g_per_t.signum() < 0
        || input.numerator_metal_oz.signum() < 0
    {
        return Err(FlowError::InvalidQuantity {
            detail: "ratio audit requires positive denominators and non-negative numerators"
                .to_string(),
        });
    }

    let rec_t = input
        .numerator_tonnes
        .checked_div(input.denominator_tonnes)?;
    let rec_g = input
        .numerator_grade_g_per_t
        .checked_div(input.denominator_grade_g_per_t)?;
    let rec_oz = input
        .numerator_metal_oz
        .checked_div(input.denominator_metal_oz)?;
    let tol = ExactQty::new(i128::from(input.ratio_tol_bps), 10_000)?;

    let mut discrepancies = Vec::new();
    let checks = [
        ("tonnes_ratio", input.printed_tonnes_ratio, rec_t),
        ("grade_ratio", input.printed_grade_ratio, rec_g),
        ("metal_ratio", input.printed_metal_ratio, rec_oz),
    ];
    for (channel, printed, recomputed) in checks {
        let diff = abs_exact(recomputed.checked_sub(printed)?)?;
        if diff.checked_cmp(tol)? == core::cmp::Ordering::Greater {
            discrepancies.push(DiscrepancyRecord {
                table_id: input.table_id.clone(),
                row_label: format!("{}:{channel}", input.row_label),
                source_locator: input.source_locator.clone(),
                source_e_id: input.source_e_id.clone(),
                kind: DiscrepancyKind::RatioIdentityMismatch,
                printed_value: format_exact_pct_4dp(printed),
                recomputed_value: format_exact_pct_4dp(recomputed),
                detail: format!(
                    "verbatim ratio '{}' ({channel}): recomputed {} != printed {}",
                    input.row_label,
                    format_exact_pct_4dp(recomputed),
                    format_exact_pct_4dp(printed)
                ),
            });
        }
    }

    Ok(VerbatimRatioAudit {
        recomputed_tonnes_ratio: rec_t,
        recomputed_grade_ratio: rec_g,
        recomputed_metal_ratio: rec_oz,
        discrepancies,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_kibali_2024_mcf_loulo_rcf_and_g_touch_6_residual_discrepancies() {
        // 1. FACT E206 (Kibali 2024 MCF row, K p222):
        //    GC Call: 8,541,084 t @ 2.94 g/t = 808,423 oz
        //    Mill Check Out: 9,145,188 t @ 2.78 g/t = 817,769 oz
        //    Printed MCF: 100% / 96% / 95% vs recomputed 107.07% / 94.56% / 101.16%!
        let kibali_2024 = audit_verbatim_ratio_row(&VerbatimRatioRowInput {
            table_id: "Kibali_Table_15_27".to_string(),
            row_label: "2024".to_string(),
            source_locator: "Kibali_TR_2025:p222".to_string(),
            source_e_id: "E206".to_string(),
            denominator_tonnes: ExactQty::from_integer(8_541_084),
            denominator_grade_g_per_t: ExactQty::new(294, 100).unwrap(),
            denominator_metal_oz: ExactQty::from_integer(808_423),
            numerator_tonnes: ExactQty::from_integer(9_145_188),
            numerator_grade_g_per_t: ExactQty::new(278, 100).unwrap(),
            numerator_metal_oz: ExactQty::from_integer(817_769),
            printed_tonnes_ratio: ExactQty::new(100, 100).unwrap(),
            printed_grade_ratio: ExactQty::new(96, 100).unwrap(),
            printed_metal_ratio: ExactQty::new(95, 100).unwrap(),
            ratio_tol_bps: 100, // ±1.0% integer-percentage rounding bound
        })
        .expect("audit Kibali 2024 MCF row");
        assert_eq!(kibali_2024.discrepancies.len(), 3);
        assert!(
            kibali_2024.discrepancies.iter().all(|d| d.kind
                == DiscrepancyKind::RatioIdentityMismatch
                && d.source_e_id == "E206")
        );
        assert_eq!(
            format_exact_pct_4dp(kibali_2024.recomputed_tonnes_ratio),
            "107.0729%"
        );
        assert_eq!(
            format_exact_pct_4dp(kibali_2024.recomputed_grade_ratio),
            "94.5578%"
        );
        assert_eq!(
            format_exact_pct_4dp(kibali_2024.recomputed_metal_ratio),
            "101.1561%"
        );

        // 2. FACT E209 (Loulo RCF Plant Check Out, L p297/p301):
        //    5,087,313 t @ 4.61 g/t -> 754,015.80 oz vs printed 683,184 oz!
        let loulo_rcf = import_verbatim_table_row(&VerbatimTableRowInput {
            table_id: "Loulo_RCF_Table".to_string(),
            row_label: "Plant Check Out".to_string(),
            source_locator: "Loulo_TR_2023:p297,p301".to_string(),
            source_e_id: "E209".to_string(),
            role: RowRole::PhysicalStream,
            printed_tonnes: ExactQty::from_integer(5_087_313),
            printed_grade_g_per_t: Some(ExactQty::new(461, 100).unwrap()),
            printed_metal_oz: ExactQty::from_integer(683_184),
            metal_identity_tol_bps: 100,
        })
        .expect("import Loulo RCF row");
        assert_eq!(loulo_rcf.discrepancies.len(), 1);
        assert_eq!(
            loulo_rcf.discrepancies[0].kind,
            DiscrepancyKind::MetalMassIdentityMismatch
        );
        assert_eq!(loulo_rcf.discrepancies[0].source_e_id, "E209");
        assert_eq!(
            format_exact_2dp(loulo_rcf.recomputed_metal_oz.expect("recomputed")),
            "754015.80"
        );

        // 3. G-TOUCH-6 & FACT E207 (Kibali GC Adjustment residual row: -10,466 t, -31.15 g/t, +10,482 oz):
        //    Stored as (Δt, Δoz) with grade NOT_DEFINED + DiscrepancyRecord filed!
        let kibali_adj = import_verbatim_table_row(&VerbatimTableRowInput {
            table_id: "Kibali_Table_15_27".to_string(),
            row_label: "GC Adjustment".to_string(),
            source_locator: "Kibali_TR_2025:p222".to_string(),
            source_e_id: "E207".to_string(),
            role: RowRole::ResidualDifference,
            printed_tonnes: ExactQty::from_integer(-10_466),
            printed_grade_g_per_t: Some(ExactQty::new(-3_115, 100).unwrap()),
            printed_metal_oz: ExactQty::from_integer(10_482),
            metal_identity_tol_bps: 100,
        })
        .expect("import Kibali residual row");
        assert_eq!(kibali_adj.tonnes, ExactQty::from_integer(-10_466));
        assert_eq!(kibali_adj.metal_oz, ExactQty::from_integer(10_482));
        assert!(matches!(
            kibali_adj.grade_g_per_t,
            DerivedGrade::NotDefined { .. }
        ));
        assert_eq!(kibali_adj.discrepancies.len(), 1);
        assert_eq!(
            kibali_adj.discrepancies[0].kind,
            DiscrepancyKind::ResidualRowPrintedGrade
        );
        assert_eq!(kibali_adj.discrepancies[0].printed_value, "-31.15 g/t");

        // 4. G-TOUCH-6 & FACT E208 (Loulo Stockpile Change residual row: 1,485,487 t, -0.54 g/t, -25,935 oz):
        let loulo_sp = import_verbatim_table_row(&VerbatimTableRowInput {
            table_id: "Loulo_Reconciliation".to_string(),
            row_label: "Stockpile Change".to_string(),
            source_locator: "Loulo_TR_2023:p297".to_string(),
            source_e_id: "E208".to_string(),
            role: RowRole::ResidualDifference,
            printed_tonnes: ExactQty::from_integer(1_485_487),
            printed_grade_g_per_t: Some(ExactQty::new(-54, 100).unwrap()),
            printed_metal_oz: ExactQty::from_integer(-25_935),
            metal_identity_tol_bps: 100,
        })
        .expect("import Loulo stockpile residual");
        assert!(matches!(
            loulo_sp.grade_g_per_t,
            DerivedGrade::NotDefined { .. }
        ));
        assert_eq!(loulo_sp.discrepancies.len(), 1);
        assert_eq!(
            loulo_sp.discrepancies[0].kind,
            DiscrepancyKind::ResidualRowPrintedGrade
        );
    }
}
