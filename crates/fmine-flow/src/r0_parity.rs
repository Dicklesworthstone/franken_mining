//! Determinism: BitwisePortable
//! R0 parity mode adopting a site's registered F-factor / MCF conventions (`BD-160`, plan §20.4,
//! `G-TOUCH-3`, `G-TOUCH-4`).
//!
//! All public signatures use exact `fmine_units::ExactQty` (zero bare `f64` parameters; L-6).
//! R0 parity mode reproduces a site's published reconciliation pack strictly under that site's
//! registered definitions from `registries/ffactors.toml`, refusing cross-profile definitions,
//! unadjusted stockpile ratios (`G-TOUCH-3`), and unversioned renders (`G-TOUCH-4`).

use std::collections::BTreeMap;

use fmine_core::codec::Encoder;
use fmine_core::hash::{Digest32, sha256};
use fmine_units::ExactQty;

use crate::FlowError;
use crate::ffactors::{
    ExactPosteriorTriplet, FFactorDefinition, PosteriorFFactor, compute_posterior_ffactor,
    render_ffactor_g_touch_4,
};
use crate::graph::FlowGraph;

/// One row of a site's reconciliation pack evaluated under R0 parity mode (`BD-160`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SitePackRow {
    /// Period label (e.g. `"2024"`, `"2025Q2"`).
    pub period: String,
    /// Exact registry key in `registries/ffactors.toml` belonging to the adopted `site_profile`.
    pub definition_id: String,
    /// Numerator posterior triplet (`p10 <= p50 <= p90`) in the definition's declared basis.
    pub numerator: ExactPosteriorTriplet,
    /// Denominator posterior triplet (`0 < p10 <= p50 <= p90`) in the definition's declared basis.
    pub denominator: ExactPosteriorTriplet,
    /// Explicit attestation whether intervening stockpile inventory changes have been adjusted (`G-TOUCH-3`).
    pub stockpile_adjusted: bool,
    /// Site's published/printed ratio (`p50`) to check parity against (e.g. `101 / 100` for `101%`).
    pub published_ratio_p50: ExactQty,
    /// Parity tolerance in basis points (`0..=10_000`, e.g. `50` = `±0.50%` rounding tolerance).
    pub tolerance_bps: u32,
}

/// R0 parity verdict for one `(period, definition_id)` row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct R0ParityRowVerdict {
    pub period: String,
    pub registry_key: String,
    pub rendered_g_touch_4: String,
    pub computed: PosteriorFFactor,
    pub published_ratio_p50: ExactQty,
    pub abs_diff: ExactQty,
    pub within_tolerance: bool,
}

/// Complete deterministic R0 site-convention parity report (`BD-160`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct R0ParityReport {
    pub site_profile: String,
    pub rows: Vec<R0ParityRowVerdict>,
    pub all_reproduced: bool,
    pub report_digest: Digest32,
}

fn abs_exact(q: ExactQty) -> Result<ExactQty, FlowError> {
    if q.signum() < 0 {
        Ok(q.checked_neg()?)
    } else {
        Ok(q)
    }
}

/// Run R0 parity mode for `site_profile` over `pack` using `defs` from `registries/ffactors.toml`.
///
/// Refuses:
/// - empty `site_profile` or empty `pack` (`FMINE-REF-107`);
/// - unknown `definition_id` or a definition registered to a different `site_profile` (`FMINE-REF-107`);
/// - unadjusted stockpile-spanning ratios (`G-TOUCH-3`, `FMINE-REF-102`);
/// - invalid quantile ordering, negative ratios, or `tolerance_bps > 10_000` (`FMINE-REF-109`).
///
/// A published ratio outside `tolerance_bps` is reported honestly as `within_tolerance = false`
/// (and `all_reproduced = false`), never tuned or suppressed.
pub fn run_r0_site_parity(
    site_profile: &str,
    defs: &[FFactorDefinition],
    graph: Option<&FlowGraph>,
    pack: &[SitePackRow],
) -> Result<R0ParityReport, FlowError> {
    if site_profile.trim().is_empty() {
        return Err(FlowError::InvalidGraph {
            detail: "R0 parity requires a non-empty site_profile".to_string(),
        });
    }
    if pack.is_empty() {
        return Err(FlowError::InvalidGraph {
            detail: "R0 parity requires at least one SitePackRow".to_string(),
        });
    }

    let mut profile_defs: BTreeMap<&str, &FFactorDefinition> = BTreeMap::new();
    for def in defs {
        if def.site_profile == site_profile && profile_defs.insert(def.id.as_str(), def).is_some() {
            return Err(FlowError::DuplicateId {
                kind: "ffactor_profile_def",
                id: def.id.clone(),
            });
        }
    }
    if profile_defs.is_empty() {
        return Err(FlowError::InvalidGraph {
            detail: format!("no F-factor definitions registered for site_profile '{site_profile}'"),
        });
    }

    let mut verdicts = Vec::with_capacity(pack.len());
    let mut all_reproduced = true;
    let mut enc = Encoder::new_frame(*b"R0PR", 1);
    enc.put_str(site_profile);
    enc.put_u32(pack.len() as u32);

    for row in pack {
        if row.tolerance_bps > 10_000 {
            return Err(FlowError::InvalidQuantity {
                detail: format!(
                    "row '{}' tolerance_bps {} exceeds 10000",
                    row.period, row.tolerance_bps
                ),
            });
        }
        if row.published_ratio_p50.signum() < 0 {
            return Err(FlowError::InvalidQuantity {
                detail: format!("row '{}' published_ratio_p50 must be >= 0", row.period),
            });
        }
        let Some(&def) = profile_defs.get(row.definition_id.as_str()) else {
            return Err(FlowError::InvalidGraph {
                detail: format!(
                    "definition '{}' is not registered under site_profile '{site_profile}'",
                    row.definition_id
                ),
            });
        };

        let computed = compute_posterior_ffactor(
            def,
            graph,
            row.numerator,
            row.denominator,
            row.stockpile_adjusted,
        )?;
        let rendered = render_ffactor_g_touch_4(&computed, def, &row.period)?;
        let diff = computed.ratio.p50.checked_sub(row.published_ratio_p50)?;
        let abs_diff = abs_exact(diff)?;
        let tol = ExactQty::new(i128::from(row.tolerance_bps), 10_000)?;
        let within_tolerance = abs_diff.checked_cmp(tol)? != core::cmp::Ordering::Greater;
        if !within_tolerance {
            all_reproduced = false;
        }

        let registry_key = def.versioned_key();
        enc.put_str(&row.period);
        enc.put_str(&registry_key);
        enc.put_i128(computed.ratio.p50.num());
        enc.put_i128(computed.ratio.p50.den());
        enc.put_i128(row.published_ratio_p50.num());
        enc.put_i128(row.published_ratio_p50.den());
        enc.put_u8(u8::from(within_tolerance));

        verdicts.push(R0ParityRowVerdict {
            period: row.period.clone(),
            registry_key,
            rendered_g_touch_4: rendered,
            computed,
            published_ratio_p50: row.published_ratio_p50,
            abs_diff,
            within_tolerance,
        });
    }

    Ok(R0ParityReport {
        site_profile: site_profile.to_string(),
        rows: verdicts,
        all_reproduced,
        report_digest: sha256(&enc.finish()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    use crate::ffactors::parse_ffactor_registry;

    fn load_defs() -> Vec<FFactorDefinition> {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|p| p.parent())
            .expect("workspace root")
            .to_path_buf();
        let text =
            std::fs::read_to_string(root.join("registries/ffactors.toml")).expect("read registry");
        parse_ffactor_registry(&text).expect("parse ffactors.toml")
    }

    fn triplet(lo: i128, mid: i128, hi: i128) -> ExactPosteriorTriplet {
        ExactPosteriorTriplet::new_positive(
            ExactQty::from_integer(lo),
            ExactQty::from_integer(mid),
            ExactQty::from_integer(hi),
        )
        .expect("triplet")
    }

    #[test]
    fn test_r0_parity_reproduces_synthetic_site_packs_and_enforces_profile_isolation() {
        let defs = load_defs();

        // 1. Synthetic Kibali site pack adopting barrick_kibali conventions (F1 conversion + MCF_out + MCF_in).
        let kibali_pack = vec![
            SitePackRow {
                period: "2025".to_string(),
                definition_id: "F1.barrick_kibali.conversion".to_string(),
                numerator: triplet(950_000, 970_000, 990_000),
                denominator: triplet(980_000, 1_000_000, 1_020_000),
                stockpile_adjusted: false,
                published_ratio_p50: ExactQty::new(97, 100).unwrap(),
                tolerance_bps: 50,
            },
            SitePackRow {
                period: "2025".to_string(),
                definition_id: "MCF_out.barrick_africa".to_string(),
                numerator: triplet(740_000, 746_649, 753_000),
                denominator: triplet(735_000, 742_630, 750_000),
                stockpile_adjusted: true,
                published_ratio_p50: ExactQty::new(101, 100).unwrap(), // 746,649 / 742,630 = 100.54% rounds to 101%
                tolerance_bps: 50,
            },
            SitePackRow {
                period: "2025".to_string(),
                definition_id: "MCF_in.barrick_africa".to_string(),
                numerator: triplet(720_000, 727_674, 735_000),
                denominator: triplet(735_000, 742_630, 750_000),
                stockpile_adjusted: true,
                published_ratio_p50: ExactQty::new(98, 100).unwrap(), // 727,674 / 742,630 = 97.99% -> 98%
                tolerance_bps: 50,
            },
        ];

        let rep1 = run_r0_site_parity("barrick_kibali", &defs, None, &kibali_pack)
            .expect("kibali R0 parity");
        let rep2 = run_r0_site_parity("barrick_kibali", &defs, None, &kibali_pack)
            .expect("kibali R0 parity rerun");
        assert!(rep1.all_reproduced, "all kibali rows within 0.5% rounding");
        assert_eq!(rep1.report_digest, rep2.report_digest);
        assert_eq!(rep1.rows.len(), 3);
        assert!(
            rep1.rows[0]
                .rendered_g_touch_4
                .starts_with("F1.barrick_kibali.conversion@v1 [2025]")
        );

        // 2. Synthetic NGM site pack adopting ngm conventions (Goldstrike Roaster 102%, Gold Quarry 96%, Autoclave 97%).
        let ngm_pack = vec![
            SitePackRow {
                period: "2024-Goldstrike-Roaster".to_string(),
                definition_id: "MCF_out.ngm".to_string(),
                numerator: triplet(1_020_000, 1_031_000, 1_042_000),
                denominator: triplet(996_000, 1_006_000, 1_016_000),
                stockpile_adjusted: true,
                published_ratio_p50: ExactQty::new(102, 100).unwrap(), // 1031/1006 = 102.49%
                tolerance_bps: 50,
            },
            SitePackRow {
                period: "2024-Gold-Quarry-Roaster".to_string(),
                definition_id: "MCF_out.ngm".to_string(),
                numerator: triplet(590_000, 598_000, 606_000),
                denominator: triplet(613_000, 621_000, 629_000),
                stockpile_adjusted: true,
                published_ratio_p50: ExactQty::new(96, 100).unwrap(), // 598/621 = 96.30%
                tolerance_bps: 50,
            },
        ];
        let ngm_rep = run_r0_site_parity("ngm", &defs, None, &ngm_pack).expect("ngm R0 parity");
        assert!(ngm_rep.all_reproduced);

        // 3. Cross-profile leakage is refused (cannot use Loulo RCF under barrick_kibali).
        let bad_profile_row = vec![SitePackRow {
            period: "2025".to_string(),
            definition_id: "RCF.barrick_loulo".to_string(),
            numerator: triplet(700, 750, 800),
            denominator: triplet(700, 750, 800),
            stockpile_adjusted: true,
            published_ratio_p50: ExactQty::from_integer(1),
            tolerance_bps: 50,
        }];
        assert_eq!(
            run_r0_site_parity("barrick_kibali", &defs, None, &bad_profile_row)
                .unwrap_err()
                .to_refusal()
                .code,
            "FMINE-REF-107"
        );

        // 4. Unadjusted stockpile ratio in R0 parity still refuses with FMINE-REF-102 (G-TOUCH-3).
        let mut unadj = kibali_pack.clone();
        unadj[1].stockpile_adjusted = false;
        assert_eq!(
            run_r0_site_parity("barrick_kibali", &defs, None, &unadj)
                .unwrap_err()
                .to_refusal()
                .code,
            "FMINE-REF-102"
        );

        // 5. Discrepant published figure is flagged honestly (all_reproduced == false), never plugged.
        let mut discrepant = kibali_pack;
        discrepant[1].published_ratio_p50 = ExactQty::new(95, 100).unwrap();
        let bad_rep = run_r0_site_parity("barrick_kibali", &defs, None, &discrepant)
            .expect("discrepancy reported without panicking");
        assert!(!bad_rep.all_reproduced);
        assert!(!bad_rep.rows[1].within_tolerance);
    }
}
