//! Public assurance metric engine (`WP-03` & `WP-06`, layer `E`):
//! - `AiscConsolidationBridge` (`L_corp`, `L_JV`, `G-BULL-3`, `BD-060`–`BD-062`, `BD-272`)
//! - Versioned `PriceDeck` and `compute_geo` (`GB-27`, `BD-063`, `BD-273`)
//! - `ReserveReplacementBridge` (`190%` organic vs `504%` total, `E603–E604`, `BD-064`)
//! - Exact LMDI 5-factor waterfall (`lmdi`, `GB-26`, `BD-065`, `BD-274`)
//! - Fixed-point cut-off grade solver (`solve_cutoff_grade`, `E182–E184`, `BD-275`).

pub mod lmdi;

use fmine_core::anchor::SourceAnchor;
use fmine_evidence::Refusal;

pub use lmdi::{LmdiState, LmdiWaterfall, decompose_aisc_lmdi, logarithmic_mean};

// --- AISC Consolidation Bridge (L_corp & L_JV, G-BULL-3, BD-062, BD-272) ---

/// Operating site row in a consolidated AISC disclosure table (`BD-062`).
#[derive(Debug, Clone, PartialEq)]
pub struct SiteAiscRow {
    pub site_name: String,
    pub aisc_numerator_usd_m: f64,
    pub sales_volume_koz_or_mlb: f64,
    pub reported_aisc_per_unit: f64,
    pub numerator_anchor: SourceAnchor,
}

/// Explicit corporate (`L_corp`) and JV equity (`L_JV`) consolidation bridge (`G-BULL-3`, `BD-272`).
#[derive(Debug, Clone, PartialEq)]
pub struct AiscConsolidationBridge {
    pub metal: String,
    pub period_label: String,
    pub site_rows: Vec<SiteAiscRow>,
    pub site_numerator_sum_usd_m: f64,
    /// `L_corp`: Corporate sustaining capex + exploration + consolidation eliminations (`$M`).
    pub l_corp_numerator_adj_usd_m: f64,
    pub consolidated_numerator_usd_m: f64,
    pub site_sales_sum: f64,
    /// `L_JV`: Equity/third-party sales volume reconciliation between site rows and consolidated denominator.
    pub l_jv_sales_adj: f64,
    pub consolidated_sales: f64,
    pub computed_consolidated_aisc: f64,
    pub reported_consolidated_aisc: f64,
    /// Sub-million rounding discrepancy (`reported_consolidated_aisc - computed_consolidated_aisc`).
    pub rounding_bridge_per_unit: f64,
    pub consolidated_anchor: SourceAnchor,
}

impl AiscConsolidationBridge {
    /// Verifies that `site_numerator_sum + l_corp_adj == consolidated_numerator` (`G-BULL-3`).
    /// If `include_l_corp` is `false` and `l_corp_numerator_adj_usd_m != 0`, refuses with `FMINE-REF-203`.
    pub fn verify_rollup(&self, include_l_corp: bool) -> Result<(), Refusal> {
        let gap = if include_l_corp {
            (self.site_numerator_sum_usd_m + self.l_corp_numerator_adj_usd_m)
                - self.consolidated_numerator_usd_m
        } else {
            self.site_numerator_sum_usd_m - self.consolidated_numerator_usd_m
        };
        if gap.abs() > 0.5 {
            return Err(Refusal::from_code(
                "FMINE-REF-203",
                format!(
                    "{} {} site-sum (${:.1}M) != consolidated (${:.1}M) without L_corp=${:.1}M",
                    self.metal,
                    self.period_label,
                    self.site_numerator_sum_usd_m,
                    self.consolidated_numerator_usd_m,
                    self.l_corp_numerator_adj_usd_m
                ),
            ));
        }
        Ok(())
    }
}

// --- Versioned PriceDeck & Gold-Equivalent Ounce (GEO) Engine (GB-27, BD-063, BD-273) ---

/// Versioned commodity price deck for Gold-Equivalent Ounce (`GEO`) conversion (`GB-27`, `BD-063`).
#[derive(Debug, Clone, PartialEq)]
pub struct PriceDeck {
    pub deck_id: String,
    pub gold_usd_per_oz: f64,
    pub copper_usd_per_lb: f64,
    pub silver_usd_per_oz: f64,
}

impl PriceDeck {
    /// Barrick 2024 Reserve Price Deck (`$1,300/oz Au`, `$3.00/lb Cu`, `$18.00/oz Ag`).
    #[must_use]
    pub fn barrick_reserve_2024() -> Self {
        Self {
            deck_id: "BARRICK-RESERVE-2024".to_string(),
            gold_usd_per_oz: 1_300.0,
            copper_usd_per_lb: 3.00,
            silver_usd_per_oz: 18.00,
        }
    }

    /// Barrick 2025 Reserve Price Deck (`$1,400/oz Au`, `$4.00/lb Cu`, `$20.00/oz Ag`).
    #[must_use]
    pub fn barrick_reserve_2025() -> Self {
        Self {
            deck_id: "BARRICK-RESERVE-2025".to_string(),
            gold_usd_per_oz: 1_400.0,
            copper_usd_per_lb: 4.00,
            silver_usd_per_oz: 20.00,
        }
    }
}

/// Gold-Equivalent Ounce (`GEO`) breakdown tagged with its `PriceDeck` (`BD-063`, `BD-273`).
#[derive(Debug, Clone, PartialEq)]
pub struct GeoBreakdown {
    pub deck_id: String,
    pub gold_koz: f64,
    pub copper_mlb: f64,
    pub copper_geo_koz: f64,
    pub total_geo_koz: f64,
}

/// Computes `GeoBreakdown` under an explicit `PriceDeck` (`GB-27`).
#[must_use]
pub fn compute_geo(gold_koz: f64, copper_mlb: f64, deck: &PriceDeck) -> GeoBreakdown {
    let copper_geo_koz = (copper_mlb * 1_000.0 * deck.copper_usd_per_lb) / deck.gold_usd_per_oz;
    GeoBreakdown {
        deck_id: deck.deck_id.clone(),
        gold_koz,
        copper_mlb,
        copper_geo_koz,
        total_geo_koz: gold_koz + copper_geo_koz,
    }
}

// --- Reserve Replacement Bridge (190% Organic vs 504% Headline, E603-E604, BD-064) ---

/// Decomposes headline vs organic reserve replacement (`E603–E604`, `BD-064`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ReserveReplacementBridge {
    pub depletion_moz: f64,
    pub organic_drilling_and_model_additions_moz: f64,
    pub greenfield_project_conversions_moz: f64,
    pub organic_replacement_pct: f64,
    pub headline_replacement_pct: f64,
}

impl ReserveReplacementBridge {
    /// Computes the Barrick FY2024 Gold Reserve Replacement bridge (`E603–E604`):
    /// - Attributable depletion (`depletion_moz`): `~3.91 Moz` (or `4.46 Moz` 100% basis)
    /// - Organic brownfield additions: `~7.43 Moz` -> `190%` organic replacement
    /// - Greenfield feasibility conversions (Reko Diq `+13.0 Moz` + Lumwana Super Pit `+1.7 Moz` = `12.28 Moz` net project addition) -> `504%` headline replacement.
    #[must_use]
    pub fn compute(
        depletion_moz: f64,
        organic_additions_moz: f64,
        greenfield_conversions_moz: f64,
    ) -> Self {
        let dep = depletion_moz.max(1e-9);
        let organic_pct = 100.0 * organic_additions_moz / dep;
        let headline_pct = 100.0 * (organic_additions_moz + greenfield_conversions_moz) / dep;
        Self {
            depletion_moz,
            organic_drilling_and_model_additions_moz: organic_additions_moz,
            greenfield_project_conversions_moz: greenfield_conversions_moz,
            organic_replacement_pct: organic_pct,
            headline_replacement_pct: headline_pct,
        }
    }
}

// --- Fixed-Point Cut-Off Grade Solver (E182-E184, BD-275) ---

/// Solves for the marginal break-even cut-off grade $g_c^*$ via fixed-point iteration (`BD-275`).
#[must_use]
pub fn solve_cutoff_grade<F>(
    processing_and_ga_usd_per_t: f64,
    net_metal_price_usd_per_unit: f64,
    initial_guess: f64,
    mut recovery_fn: F,
) -> (f64, usize)
where
    F: FnMut(f64) -> f64,
{
    let mut g = initial_guess.max(1e-6);
    let denom_price = net_metal_price_usd_per_unit.max(1e-9);
    for iter in 1..=100 {
        let rec = recovery_fn(g).clamp(0.05, 0.999);
        let next_g = processing_and_ga_usd_per_t / (denom_price * rec);
        if (next_g - g).abs() < 1e-10 {
            return (next_g, iter);
        }
        g = 0.5 * g + 0.5 * next_g;
    }
    (g, 100)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_aisc_consolidation_bridge_l_corp_refusal_and_reconciliation() {
        let anchor = SourceAnchor::new(
            "corpus_of_input_docs/barrick-aisc-2025-q4.xlsx",
            [0x11; 32],
            "Gold AISC!N206",
        );
        let fy24 = AiscConsolidationBridge {
            metal: "Gold".to_string(),
            period_label: "FY2024".to_string(),
            site_rows: Vec::new(),
            site_numerator_sum_usd_m: 5931.0,
            l_corp_numerator_adj_usd_m: -49.0,
            consolidated_numerator_usd_m: 5882.0,
            site_sales_sum: 3956.0,
            l_jv_sales_adj: 17.0,
            consolidated_sales: 3973.0,
            computed_consolidated_aisc: 5_882_000.0 / 3973.0,
            reported_consolidated_aisc: 1484.0,
            rounding_bridge_per_unit: 1484.0 - (5_882_000.0 / 3973.0),
            consolidated_anchor: anchor,
        };
        let err = fy24
            .verify_rollup(false)
            .expect_err("naive site sum must refuse without L_corp");
        assert_eq!(err.code, "FMINE-REF-203");
        fy24.verify_rollup(true).expect("reconciled with L_corp");
    }

    #[test]
    fn test_versioned_geo_and_reserve_replacement_bridge() {
        let d24 = PriceDeck::barrick_reserve_2024();
        let d25 = PriceDeck::barrick_reserve_2025();
        let g24 = compute_geo(3910.0, 430.0, &d24);
        let g25 = compute_geo(3910.0, 430.0, &d25);
        assert_ne!(g24.total_geo_koz, g25.total_geo_koz);
        assert!(g25.copper_geo_koz > g24.copper_geo_koz);

        let rr = ReserveReplacementBridge::compute(3.91, 7.429, 12.277);
        assert!((rr.organic_replacement_pct - 190.0).abs() < 0.5);
        assert!((rr.headline_replacement_pct - 504.0).abs() < 0.5);
    }

    #[test]
    fn test_fixed_point_cutoff_grade_solver() {
        let (gc, iters) = solve_cutoff_grade(14.50, 8_500.0, 0.0025, |g| {
            0.90 * (1.0 - (-800.0 * g).exp())
        });
        assert!(iters < 50);
        assert!(gc > 0.0015 && gc < 0.0030);
    }
}
