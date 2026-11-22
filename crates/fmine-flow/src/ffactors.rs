//! Determinism: BitwisePortable
//! F-factors as posterior quantities and site-profile definitions (`BD-148`, plan §20.3–§20.5,
//! `G-TOUCH-3`, `FACT R7 §4.3`).
//!
//! All public quantities use exact `fmine_units::ExactQty` (zero bare `f64` parameters; L-6).
//! Enforces two core Touchstone laws:
//! 1. Stockpile decoupling (`G-TOUCH-3`): any F-factor whose definition requires stockpile
//!    adjustment refuses computation unless the caller explicitly provides an inventory-adjusted
//!    denominator (never naive `mined_this_month ÷ milled_this_month`).
//! 2. Mutual-compensation detection (`FACT R7 §4.3`): `F3 = F1 × F2` is always accompanied by
//!    separate posterior quantiles for `F1` and `F2`, and flags `mutual_compensation_detected = true`
//!    whenever both `F1` and `F2` depart from `1.0` beyond `mutual_comp_tol_bps` while `F3` masks
//!    the error by sitting within `mutual_comp_tol_bps` of `1.0`.

use std::collections::BTreeSet;

use fmine_units::ExactQty;

use crate::FlowError;
use crate::graph::FlowGraph;

/// F-factor family identifier (plan §20.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FFactorFamily {
    F1,
    F2,
    F3,
    Mcf,
    Rcf,
}

impl FFactorFamily {
    fn parse(s: &str) -> Result<Self, FlowError> {
        match s {
            "F1" => Ok(Self::F1),
            "F2" => Ok(Self::F2),
            "F3" => Ok(Self::F3),
            "MCF" => Ok(Self::Mcf),
            "RCF" => Ok(Self::Rcf),
            other => Err(FlowError::InvalidGraph {
                detail: format!("unknown F-factor family {other:?}"),
            }),
        }
    }
}

/// Site-profile definition of one reconciliation F-factor parsed from `registries/ffactors.toml`
/// (`BD-148`, `BD-278`, plan §20.4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FFactorDefinition {
    pub id: String,
    pub version: u32,
    pub family: FFactorFamily,
    pub site_profile: String,
    pub numerator_node: String,
    pub denominator_node: String,
    pub quantity_basis: String,
    pub support: String,
    pub requires_stockpile_adjustment: bool,
    pub mutual_comp_tol_bps: u32,
    pub source_e_ids: Vec<String>,
    pub source_locator: String,
    pub description: String,
}

impl FFactorDefinition {
    /// Canonical versioned registry key required by `G-TOUCH-4` (e.g. `F1.barrick_kibali.conversion@v1`).
    #[must_use]
    pub fn versioned_key(&self) -> String {
        format!("{}@v{}", self.id, self.version)
    }
}

/// Exact rational posterior quantile triplet (`p10 <= p50 <= p90`) for a quantity or F-factor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExactPosteriorTriplet {
    pub p10: ExactQty,
    pub p50: ExactQty,
    pub p90: ExactQty,
}

impl ExactPosteriorTriplet {
    /// Construct a validated monotone posterior quantile triplet (`p10 <= p50 <= p90`).
    pub fn new(p10: ExactQty, p50: ExactQty, p90: ExactQty) -> Result<Self, FlowError> {
        if p10.checked_cmp(p50)? == core::cmp::Ordering::Greater
            || p50.checked_cmp(p90)? == core::cmp::Ordering::Greater
        {
            return Err(FlowError::InvalidQuantity {
                detail: "posterior quantiles must satisfy p10 <= p50 <= p90".to_string(),
            });
        }
        Ok(Self { p10, p50, p90 })
    }

    /// Construct a validated strictly positive posterior triplet (`0 < p10 <= p50 <= p90`).
    pub fn new_positive(p10: ExactQty, p50: ExactQty, p90: ExactQty) -> Result<Self, FlowError> {
        if p10.signum() <= 0 {
            return Err(FlowError::InvalidQuantity {
                detail: "posterior denominator quantile p10 must be strictly positive".to_string(),
            });
        }
        Self::new(p10, p50, p90)
    }
}

/// Posterior F-factor estimate retaining full uncertainty bounds and definition version (never a bare scalar point).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PosteriorFFactor {
    pub definition_id: String,
    pub definition_version: u32,
    pub family: FFactorFamily,
    pub ratio: ExactPosteriorTriplet,
    pub stockpile_adjusted: bool,
}

/// Composite `F3 = F1 × F2` reconciliation diagnostic exposing mutual compensation (`R7 §4.3`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct F3Decomposition {
    pub f1: PosteriorFFactor,
    pub f2: PosteriorFFactor,
    pub f3_product: ExactPosteriorTriplet,
    /// True when `|F1.p50 - 1| > tol` and `|F2.p50 - 1| > tol` while `|F3.p50 - 1| <= tol`.
    pub mutual_compensation_detected: bool,
}

/// Parse `registries/ffactors.toml` (schema version 1), refusing malformed or duplicate rows.
pub fn parse_ffactor_registry(text: &str) -> Result<Vec<FFactorDefinition>, FlowError> {
    let mut saw_version = false;
    let mut out = Vec::new();
    let mut seen_ids = BTreeSet::new();
    let mut cur: Option<Vec<(String, String)>> = None;

    let flush = |fields: Vec<(String, String)>,
                 seen: &mut BTreeSet<String>,
                 out: &mut Vec<FFactorDefinition>|
     -> Result<(), FlowError> {
        let get = |k: &str| -> Result<String, FlowError> {
            fields
                .iter()
                .find(|(fk, _)| fk == k)
                .map(|(_, v)| v.clone())
                .ok_or_else(|| FlowError::InvalidGraph {
                    detail: format!("[[ffactor]] missing field '{k}'"),
                })
        };
        let get_non_empty = |k: &str| -> Result<String, FlowError> {
            let v = get(k)?;
            if v.trim().is_empty() {
                return Err(FlowError::InvalidGraph {
                    detail: format!("[[ffactor]] field '{k}' must be non-empty"),
                });
            }
            Ok(v)
        };
        let id = get("id")?;
        if id.is_empty() || !seen.insert(id.clone()) {
            return Err(FlowError::DuplicateId {
                kind: "ffactor",
                id,
            });
        }
        let ver_raw = get("version")?;
        let version: u32 = ver_raw.parse().map_err(|_| FlowError::InvalidGraph {
            detail: format!("ffactor '{id}' invalid version {ver_raw:?}"),
        })?;
        if version == 0 {
            return Err(FlowError::InvalidGraph {
                detail: format!("ffactor '{id}' version must be >= 1"),
            });
        }
        let family = FFactorFamily::parse(&get("family")?)?;
        let req_raw = get("requires_stockpile_adjustment")?;
        let requires_stockpile_adjustment = match req_raw.as_str() {
            "true" => true,
            "false" => false,
            other => {
                return Err(FlowError::InvalidGraph {
                    detail: format!(
                        "ffactor '{id}' requires_stockpile_adjustment must be bool, got {other:?}"
                    ),
                });
            }
        };
        let tol_raw = get("mutual_comp_tol_bps")?;
        let mutual_comp_tol_bps: u32 = tol_raw.parse().map_err(|_| FlowError::InvalidGraph {
            detail: format!("ffactor '{id}' invalid mutual_comp_tol_bps {tol_raw:?}"),
        })?;
        if !(1..=9_999).contains(&mutual_comp_tol_bps) {
            return Err(FlowError::InvalidGraph {
                detail: format!(
                    "ffactor '{id}' mutual_comp_tol_bps {mutual_comp_tol_bps} outside 1..=9999"
                ),
            });
        }
        let source_e_ids: Vec<String> = get("source_e_ids")?
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(ToString::to_string)
            .collect();
        if source_e_ids.is_empty() {
            return Err(FlowError::InvalidGraph {
                detail: format!("ffactor '{id}' must cite at least one source_e_id"),
            });
        }
        out.push(FFactorDefinition {
            id,
            version,
            family,
            site_profile: get_non_empty("site_profile")?,
            numerator_node: get_non_empty("numerator_node")?,
            denominator_node: get_non_empty("denominator_node")?,
            quantity_basis: get_non_empty("quantity_basis")?,
            support: get_non_empty("support")?,
            requires_stockpile_adjustment,
            mutual_comp_tol_bps,
            source_e_ids,
            source_locator: get_non_empty("source_locator")?,
            description: get_non_empty("description")?,
        });
        Ok(())
    };

    for (idx, raw) in text.lines().enumerate() {
        let line = idx + 1;
        let s = raw.trim();
        if s.is_empty() || s.starts_with('#') {
            continue;
        }
        if s == "[[ffactor]]" {
            if !saw_version {
                return Err(FlowError::InvalidGraph {
                    detail: format!("line {line}: schema_version = 1 must precede [[ffactor]]"),
                });
            }
            if let Some(prev) = cur.take() {
                flush(prev, &mut seen_ids, &mut out)?;
            }
            cur = Some(Vec::new());
            continue;
        }
        let Some((k, v)) = s.split_once('=') else {
            return Err(FlowError::InvalidGraph {
                detail: format!("line {line}: expected key = value"),
            });
        };
        let key = k.trim();
        let val = v.trim();
        match cur.as_mut() {
            None => {
                if !saw_version && key == "schema_version" && val == "1" {
                    saw_version = true;
                } else {
                    return Err(FlowError::InvalidGraph {
                        detail: format!("line {line}: expected single schema_version = 1"),
                    });
                }
            }
            Some(fields) => {
                let parsed =
                    if let Some(inner) = val.strip_prefix('"').and_then(|x| x.strip_suffix('"')) {
                        inner.to_string()
                    } else {
                        val.to_string()
                    };
                if fields.iter().any(|(fk, _)| fk == key) {
                    return Err(FlowError::DuplicateId {
                        kind: "ffactor_field",
                        id: key.to_string(),
                    });
                }
                fields.push((key.to_string(), parsed));
            }
        }
    }
    if let Some(last) = cur.take() {
        flush(last, &mut seen_ids, &mut out)?;
    }
    if !saw_version || out.is_empty() {
        return Err(FlowError::InvalidGraph {
            detail:
                "ffactors registry requires schema_version = 1 and at least one [[ffactor]] row"
                    .to_string(),
        });
    }
    Ok(out)
}

/// Compute a conservative exact posterior F-factor enclosure `[num.p10/den.p90, num.p50/den.p50, num.p90/den.p10]`.
///
/// Enforces `G-TOUCH-3`: if `def.requires_stockpile_adjustment` is true OR `graph` has an
/// intervening stockpile between `denominator_node` and `numerator_node`, `stockpile_adjusted`
/// MUST be `true`, otherwise computation refuses with `FMINE-REF-102`.
pub fn compute_posterior_ffactor(
    def: &FFactorDefinition,
    graph: Option<&FlowGraph>,
    numerator: ExactPosteriorTriplet,
    denominator: ExactPosteriorTriplet,
    stockpile_adjusted: bool,
) -> Result<PosteriorFFactor, FlowError> {
    // Re-validate monotonic quantile order in case caller constructed struct fields directly.
    let _ = ExactPosteriorTriplet::new(numerator.p10, numerator.p50, numerator.p90)?;
    let _ = ExactPosteriorTriplet::new_positive(denominator.p10, denominator.p50, denominator.p90)?;
    if numerator.p10.signum() < 0 {
        return Err(FlowError::InvalidQuantity {
            detail: "F-factor requires non-negative numerator and strictly positive denominator"
                .to_string(),
        });
    }
    if !stockpile_adjusted {
        if let Some(g) = graph {
            g.check_direct_ratio_admissible(&def.denominator_node, &def.numerator_node)?;
        }
        if def.requires_stockpile_adjustment {
            return Err(FlowError::StockpileDecouplingViolation {
                source_node: def.denominator_node.clone(),
                target_node: def.numerator_node.clone(),
                intervening_stockpiles: vec!["declared_stockpile_adjustment_required".to_string()],
            });
        }
    }
    // Conservative exact ratio bounds: [num.p10 / den.p90, num.p50 / den.p50, num.p90 / den.p10].
    let p10 = numerator.p10.checked_div(denominator.p90)?;
    let p50 = numerator.p50.checked_div(denominator.p50)?;
    let p90 = numerator.p90.checked_div(denominator.p10)?;
    let ratio = ExactPosteriorTriplet::new(p10, p50, p90)?;
    Ok(PosteriorFFactor {
        definition_id: def.id.clone(),
        definition_version: def.version,
        family: def.family,
        ratio,
        stockpile_adjusted,
    })
}

/// Format an exact rational ratio as a deterministic 4-decimal percentage string (e.g. `101.20%`)
/// using integer rounding halves away from zero (no bare `f64`).
#[must_use]
pub fn format_exact_pct_4dp(q: ExactQty) -> String {
    // Scale by 100 * 10_000 = 1_000_000 to get basis points * 100 (4 decimal places of %).
    let sign = if q.num() < 0 { "-" } else { "" };
    let num_abs = q.num().unsigned_abs();
    let den_abs = q.den().unsigned_abs();
    let scaled_num = num_abs.saturating_mul(1_000_000);
    let half_den = den_abs / 2;
    let rounded = scaled_num.saturating_add(half_den) / den_abs;
    let whole = rounded / 10_000;
    let frac = rounded % 10_000;
    format!("{sign}{whole}.{frac:04}%")
}

/// Enforce `G-TOUCH-4` (`BD-278`, plan §20.4): render a [`PosteriorFFactor`] strictly with its
/// versioned registry key (`id@vN`), support, source locator, and `[p10, p90]` posterior interval.
///
/// Refuses (`FMINE-REF-107`) any attempt to render a bare family label (`"F1"`, `"F2"`, `"F3"`,
/// `"MCF"`, `"RCF"`), an unversioned (`version == 0`) or mismatched definition, an empty period,
/// or an unvalidated quantile triplet.
pub fn render_ffactor_g_touch_4(
    factor: &PosteriorFFactor,
    def: &FFactorDefinition,
    period_label: &str,
) -> Result<String, FlowError> {
    if def.id.is_empty()
        || def.version == 0
        || def.support.trim().is_empty()
        || def.source_locator.trim().is_empty()
        || period_label.trim().is_empty()
    {
        return Err(FlowError::InvalidGraph {
            detail: "G-TOUCH-4: F-factor render requires non-empty registry id, version >= 1, support, source_locator, and period"
                .to_string(),
        });
    }
    if matches!(def.id.as_str(), "F1" | "F2" | "F3" | "MCF" | "RCF") {
        return Err(FlowError::InvalidGraph {
            detail: format!(
                "G-TOUCH-4: bare label '{}' is forbidden; render the full site registry key",
                def.id
            ),
        });
    }
    if factor.definition_id != def.id || factor.definition_version != def.version {
        return Err(FlowError::InvalidGraph {
            detail: format!(
                "G-TOUCH-4: factor ({}@v{}) does not match registry definition ({}@v{})",
                factor.definition_id, factor.definition_version, def.id, def.version
            ),
        });
    }
    let _ = ExactPosteriorTriplet::new(factor.ratio.p10, factor.ratio.p50, factor.ratio.p90)?;
    if factor.ratio.p10.signum() < 0 {
        return Err(FlowError::InvalidQuantity {
            detail: "G-TOUCH-4: cannot render negative F-factor ratio".to_string(),
        });
    }
    Ok(format!(
        "{} [{}] ({}; {}/{}; support={}; src={}): p50={} [p10={}, p90={}]",
        def.versioned_key(),
        period_label.trim(),
        def.site_profile,
        def.numerator_node,
        def.denominator_node,
        def.support,
        def.source_locator,
        format_exact_pct_4dp(factor.ratio.p50),
        format_exact_pct_4dp(factor.ratio.p10),
        format_exact_pct_4dp(factor.ratio.p90),
    ))
}

fn exceeds_unit_tol(q: ExactQty, tol_bps: u32) -> Result<bool, FlowError> {
    let one = ExactQty::from_integer(1);
    let diff = q.checked_sub(one)?;
    let abs_diff = if diff.signum() < 0 {
        diff.checked_neg()?
    } else {
        diff
    };
    let tol = ExactQty::new(i128::from(tol_bps), 10_000)?;
    Ok(abs_diff.checked_cmp(tol)? == core::cmp::Ordering::Greater)
}

/// Compose `F3 = F1 × F2` and detect mutual compensation where opposing `F1` and `F2` biases mask
/// each other in `F3` (plan §20.3, `FACT R7 §4.3`).
pub fn decompose_f3(
    f1: PosteriorFFactor,
    f2: PosteriorFFactor,
    mutual_comp_tol_bps: u32,
) -> Result<F3Decomposition, FlowError> {
    if f1.family != FFactorFamily::F1 || f2.family != FFactorFamily::F2 {
        return Err(FlowError::InvalidQuantity {
            detail: format!(
                "decompose_f3 requires (F1, F2) families, got ({:?}, {:?})",
                f1.family, f2.family
            ),
        });
    }
    if !(1..=9_999).contains(&mutual_comp_tol_bps) {
        return Err(FlowError::InvalidQuantity {
            detail: format!("mutual_comp_tol_bps ({mutual_comp_tol_bps}) must lie in 1..=9999"),
        });
    }
    let _ = ExactPosteriorTriplet::new(f1.ratio.p10, f1.ratio.p50, f1.ratio.p90)?;
    let _ = ExactPosteriorTriplet::new(f2.ratio.p10, f2.ratio.p50, f2.ratio.p90)?;
    if f1.ratio.p10.signum() < 0 || f2.ratio.p10.signum() < 0 {
        return Err(FlowError::InvalidQuantity {
            detail: "F1 and F2 ratios must be non-negative".to_string(),
        });
    }
    let p10 = f1.ratio.p10.checked_mul(f2.ratio.p10)?;
    let p50 = f1.ratio.p50.checked_mul(f2.ratio.p50)?;
    let p90 = f1.ratio.p90.checked_mul(f2.ratio.p90)?;
    let f3_product = ExactPosteriorTriplet::new(p10, p50, p90)?;

    let f1_off = exceeds_unit_tol(f1.ratio.p50, mutual_comp_tol_bps)?;
    let f2_off = exceeds_unit_tol(f2.ratio.p50, mutual_comp_tol_bps)?;
    let f3_masked = !exceeds_unit_tol(f3_product.p50, mutual_comp_tol_bps)?;

    Ok(F3Decomposition {
        f1,
        f2,
        f3_product,
        mutual_compensation_detected: f1_off && f2_off && f3_masked,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    use crate::graph::{FlowNode, FlowStream, NodeClass, StockpileMixingModel};

    #[test]
    fn test_ffactor_posterior_definitions_registry_and_mutual_compensation_detection() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|p| p.parent())
            .expect("workspace root")
            .to_path_buf();
        let text =
            std::fs::read_to_string(root.join("registries/ffactors.toml")).expect("read registry");
        let defs = parse_ffactor_registry(&text).expect("parse ffactors.toml");
        assert!(defs.len() >= 6);

        let f1_def = defs
            .iter()
            .find(|d| d.id == "F1_metal_au")
            .expect("F1_metal_au");
        let f2_def = defs
            .iter()
            .find(|d| d.id == "F2_metal_au")
            .expect("F2_metal_au");

        // Build graph with ROM stockpile between grade_control and mill_feed.
        let g = FlowGraph::build(
            vec![
                FlowNode::new(
                    "resource_model",
                    "Resource",
                    NodeClass::InSituVolume,
                    false,
                    None,
                )
                .unwrap(),
                FlowNode::new("grade_control", "GC", NodeClass::BlastedAndDug, false, None)
                    .unwrap(),
                FlowNode::new(
                    "rom",
                    "ROM",
                    NodeClass::StockpileOrPad,
                    true,
                    Some(StockpileMixingModel::Fifo),
                )
                .unwrap(),
                FlowNode::new("mill_feed", "Mill", NodeClass::PlantUnit, false, None).unwrap(),
            ],
            vec![
                FlowStream::new("r_gc", "resource_model", "grade_control", false, ["Au"]).unwrap(),
                FlowStream::new("gc_rom", "grade_control", "rom", false, ["Au"]).unwrap(),
                FlowStream::new("rom_m", "rom", "mill_feed", false, ["Au"]).unwrap(),
            ],
        )
        .expect("graph");

        // Unadjusted F2 across stockpile must refuse with FMINE-REF-102 (G-TOUCH-3).
        let res = ExactPosteriorTriplet::new_positive(
            ExactQty::from_integer(950),
            ExactQty::from_integer(1000),
            ExactQty::from_integer(1050),
        )
        .unwrap();
        let gc = ExactPosteriorTriplet::new_positive(
            ExactQty::from_integer(1140),
            ExactQty::from_integer(1200),
            ExactQty::from_integer(1260),
        )
        .unwrap();
        let mill = ExactPosteriorTriplet::new_positive(
            ExactQty::from_integer(960),
            ExactQty::from_integer(1008),
            ExactQty::from_integer(1050),
        )
        .unwrap();

        let err = compute_posterior_ffactor(f2_def, Some(&g), mill, gc, false).unwrap_err();
        assert_eq!(err.to_refusal().code, "FMINE-REF-102");

        // F1 = 1200 / 1000 = 1.20 (+20% overestimate); F2 (adjusted) = 1008 / 1200 = 0.84 (-16% loss).
        // F3 = 1.20 * 0.84 = 1.008 (within 5% of 1.00 -> textbook mutual compensation!).
        let f1 = compute_posterior_ffactor(f1_def, Some(&g), gc, res, false).expect("f1");
        let f2 = compute_posterior_ffactor(f2_def, Some(&g), mill, gc, true).expect("f2");
        assert_eq!(f1.ratio.p50, ExactQty::new(6, 5).unwrap());
        assert_eq!(f2.ratio.p50, ExactQty::new(21, 25).unwrap());

        let decomp = decompose_f3(f1.clone(), f2.clone(), f2_def.mutual_comp_tol_bps).expect("f3");
        assert_eq!(decomp.f3_product.p50, ExactQty::new(126, 125).unwrap()); // 1.008
        assert!(
            decomp.mutual_compensation_detected,
            "opposing +20% F1 and -16% F2 hiding inside 1.008 F3 must flag mutual compensation"
        );

        // Regression: swapped families (F2, F1), out-of-range tol_bps, and inverted triplet refuse.
        assert!(decompose_f3(f2.clone(), f1.clone(), 500).is_err());
        assert!(decompose_f3(f1.clone(), f2, 0).is_err());
        assert!(
            decompose_f3(
                f1,
                PosteriorFFactor {
                    definition_id: "F2_bad".to_string(),
                    definition_version: 1,
                    family: FFactorFamily::F2,
                    ratio: ExactPosteriorTriplet {
                        p10: ExactQty::from_integer(2),
                        p50: ExactQty::from_integer(1),
                        p90: ExactQty::from_integer(3),
                    },
                    stockpile_adjusted: true,
                },
                500
            )
            .is_err()
        );
    }

    #[test]
    fn test_site_specific_ffactor_registry_and_g_touch_4_rendering() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|p| p.parent())
            .expect("workspace root")
            .to_path_buf();
        let text =
            std::fs::read_to_string(root.join("registries/ffactors.toml")).expect("read registry");
        let defs = parse_ffactor_registry(&text).expect("parse ffactors.toml");

        // All 7 §20.4 site-specific definitions (Kibali, Loulo, NGM) present with E-ids & locators.
        let required_keys = [
            "F1.barrick_kibali.conversion",
            "MCF_out.barrick_africa",
            "MCF_in.barrick_africa",
            "MCF_t.barrick_loulo",
            "MCF_g.barrick_loulo",
            "RCF.barrick_loulo",
            "MCF_out.ngm",
        ];
        for key in required_keys {
            let def = defs
                .iter()
                .find(|d| d.id == key)
                .unwrap_or_else(|| panic!("missing §20.4 registry key {key}"));
            assert!(def.version >= 1);
            assert!(!def.source_e_ids.is_empty());
            assert!(!def.source_locator.is_empty());
        }
        // NGM F1 is NOT public (DATA-GATE, plan §20.4): must NOT exist in the public registry.
        assert!(
            defs.iter()
                .all(|d| !(d.site_profile == "ngm" && d.family == FFactorFamily::F1))
        );

        // G-TOUCH-4 renders the full versioned registry key + interval, never bare "F1".
        let kibali_f1 = defs
            .iter()
            .find(|d| d.id == "F1.barrick_kibali.conversion")
            .expect("kibali F1");
        let num = ExactPosteriorTriplet::new_positive(
            ExactQty::from_integer(980),
            ExactQty::from_integer(1000),
            ExactQty::from_integer(1020),
        )
        .unwrap();
        let den = ExactPosteriorTriplet::new_positive(
            ExactQty::from_integer(990),
            ExactQty::from_integer(1000),
            ExactQty::from_integer(1010),
        )
        .unwrap();
        let post = compute_posterior_ffactor(kibali_f1, None, num, den, false).expect("kibali f1");
        let rendered =
            render_ffactor_g_touch_4(&post, kibali_f1, "2025").expect("G-TOUCH-4 render");
        assert!(rendered.starts_with("F1.barrick_kibali.conversion@v1 [2025]"));
        assert!(rendered.contains("p50=100.0000%"));
        assert!(rendered.contains("p10="));
        assert!(rendered.contains("p90="));
        assert!(rendered.contains("Kibali_TR_2025:p183"));

        // G-TOUCH-4 refuses bare label, version == 0, empty period, or definition mismatch.
        let mut bare = kibali_f1.clone();
        bare.id = "F1".to_string();
        let mut bare_post = post.clone();
        bare_post.definition_id = "F1".to_string();
        assert_eq!(
            render_ffactor_g_touch_4(&bare_post, &bare, "2025")
                .unwrap_err()
                .to_refusal()
                .code,
            "FMINE-REF-107"
        );
        assert_eq!(
            render_ffactor_g_touch_4(&post, kibali_f1, "   ")
                .unwrap_err()
                .to_refusal()
                .code,
            "FMINE-REF-107"
        );
        let mut v0 = kibali_f1.clone();
        v0.version = 0;
        assert_eq!(
            render_ffactor_g_touch_4(&post, &v0, "2025")
                .unwrap_err()
                .to_refusal()
                .code,
            "FMINE-REF-107"
        );
    }
}
