//! Public assurance metric engine (`WP-03` & `WP-06`, layer `E`):
//! - `AiscConsolidationBridge` (`L_corp`, `L_JV`, `G-BULL-3`, `BD-060`–`BD-062`, `BD-272`)
//! - Versioned `PriceDeck` and `compute_geo` (`GB-27`, `BD-063`, `BD-273`)
//! - `ReserveReplacementBridge` (one stated basis per bridge; `E603–E604`, `BD-064`)
//! - Exact LMDI 5-factor waterfall (`lmdi`, `GB-26`, `BD-065`, `BD-274`)
//! - Fixed-point cut-off grade solver (`solve_cutoff_grade`, `BD-275`). It is tested on a synthetic recovery
//!   curve only; reproducing the published cut-offs (E182–E184, BD-274) needs the reports' cost and price inputs.

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
    /// `None` when the cited source states no silver price.
    pub silver_usd_per_oz: Option<f64>,
}

impl PriceDeck {
    /// Barrick's long-term mineral reserve prices for one reserve year, as printed in the 2026 Management
    /// Information Circular, p.86 GEO note (FACT, E606): 2025 $1,500/oz Au and $3.25/lb Cu; 2024 $1,400 and
    /// $3.00; 2023 $1,300 and $3.00. The note states no silver price. `None` for any other year.
    #[must_use]
    pub fn barrick_reserve(year: u16) -> Option<Self> {
        let (gold, copper) = match year {
            2023 => (1_300.0, 3.00),
            2024 => (1_400.0, 3.00),
            2025 => (1_500.0, 3.25),
            _ => return None,
        };
        Some(Self {
            deck_id: format!("BARRICK-RESERVE-{year}"),
            gold_usd_per_oz: gold,
            copper_usd_per_lb: copper,
            silver_usd_per_oz: None,
        })
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

// --- Reserve Replacement Bridge (BD-064) ---

/// Reserve replacement on one stated basis, split into organic additions and project conversions (`BD-064`).
///
/// Every input must be on the same metal basis and window. Barrick's 2026 Circular prints 2023–25 replacement
/// as ~190% of gold ounces and 504% of gold-equivalent ounces: the two figures differ by metal basis, not by
/// organic versus project additions (FACT, E603, E604), so each is computed on its own basis.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ReserveReplacementBridge {
    pub depletion_moz: f64,
    pub organic_drilling_and_model_additions_moz: f64,
    pub greenfield_project_conversions_moz: f64,
    pub organic_replacement_pct: f64,
    pub headline_replacement_pct: f64,
}

impl ReserveReplacementBridge {
    /// `None` when depletion is not a positive finite number: the ratio is then undefined, and it is never
    /// clamped to a small denominator.
    #[must_use]
    pub fn compute(
        depletion_moz: f64,
        organic_additions_moz: f64,
        greenfield_conversions_moz: f64,
    ) -> Option<Self> {
        if !(depletion_moz.is_finite() && depletion_moz > 0.0) {
            return None;
        }
        let organic_pct = 100.0 * organic_additions_moz / depletion_moz;
        let headline_pct =
            100.0 * (organic_additions_moz + greenfield_conversions_moz) / depletion_moz;
        Some(Self {
            depletion_moz,
            organic_drilling_and_model_additions_moz: organic_additions_moz,
            greenfield_project_conversions_moz: greenfield_conversions_moz,
            organic_replacement_pct: organic_pct,
            headline_replacement_pct: headline_pct,
        })
    }
}

// --- Fixed-Point Cut-Off Grade Solver (E182-E184, BD-275) ---

/// Iteration cap of [`solve_cutoff_grade`]; reaching it without convergence is a refusal, never a result.
pub const CUTOFF_MAX_ITER: usize = 100;
/// Convergence tolerance of [`solve_cutoff_grade`] on the step `|g_{n+1} − g_n|` (grade units).
pub const CUTOFF_TOL: f64 = 1e-10;

/// A converged break-even cut-off grade.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CutoffSolution {
    /// The cut-off grade `g` with `g = cost / (price · recovery(g))`.
    pub grade: f64,
    /// Iterations used (at most [`CUTOFF_MAX_ITER`]).
    pub iterations: usize,
    /// `|g − cost / (price · recovery(g))|` evaluated at the returned grade.
    pub residual: f64,
}

/// Why [`solve_cutoff_grade`] refused. No input is floored or clamped into its domain (AGENTS rule 5).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CutoffError {
    /// Processing plus G&A cost is not a positive finite number.
    NonPositiveCost(f64),
    /// Net metal price is not a positive finite number.
    NonPositivePrice(f64),
    /// The initial guess is not a positive finite grade.
    NonPositiveGuess(f64),
    /// The recovery function returned a value outside `(0, 1]` (or a non-finite value) at `at_grade`.
    RecoveryOutOfDomain { at_grade: f64, value: f64 },
    /// The iteration reached [`CUTOFF_MAX_ITER`] without the step falling below [`CUTOFF_TOL`].
    NotConverged { iterations: usize, last_step: f64 },
}

impl std::fmt::Display for CutoffError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NonPositiveCost(c) => write!(
                f,
                "processing and G&A cost must be positive and finite, got {c}"
            ),
            Self::NonPositivePrice(p) => {
                write!(f, "net metal price must be positive and finite, got {p}")
            }
            Self::NonPositiveGuess(g) => write!(
                f,
                "initial grade guess must be positive and finite, got {g}"
            ),
            Self::RecoveryOutOfDomain { at_grade, value } => {
                write!(f, "recovery {value} at grade {at_grade} is outside (0, 1]")
            }
            Self::NotConverged {
                iterations,
                last_step,
            } => {
                write!(
                    f,
                    "no convergence after {iterations} iterations (last step {last_step})"
                )
            }
        }
    }
}

impl std::error::Error for CutoffError {}

/// Solves for the marginal break-even cut-off grade $g_c^*$ via damped fixed-point iteration (`BD-275`).
///
/// Refuses, rather than clamps, every out-of-domain input: a non-positive cost, price or guess, a recovery
/// outside `(0, 1]`, and non-convergence within [`CUTOFF_MAX_ITER`] iterations.
pub fn solve_cutoff_grade<F>(
    processing_and_ga_usd_per_t: f64,
    net_metal_price_usd_per_unit: f64,
    initial_guess: f64,
    mut recovery_fn: F,
) -> Result<CutoffSolution, CutoffError>
where
    F: FnMut(f64) -> f64,
{
    let positive = |x: f64| x.is_finite() && x > 0.0;
    if !positive(processing_and_ga_usd_per_t) {
        return Err(CutoffError::NonPositiveCost(processing_and_ga_usd_per_t));
    }
    if !positive(net_metal_price_usd_per_unit) {
        return Err(CutoffError::NonPositivePrice(net_metal_price_usd_per_unit));
    }
    if !positive(initial_guess) {
        return Err(CutoffError::NonPositiveGuess(initial_guess));
    }
    let mut recovery_at = |g: f64| {
        let rec = recovery_fn(g);
        if rec.is_finite() && rec > 0.0 && rec <= 1.0 {
            Ok(rec)
        } else {
            Err(CutoffError::RecoveryOutOfDomain {
                at_grade: g,
                value: rec,
            })
        }
    };
    let image = |rec: f64| processing_and_ga_usd_per_t / (net_metal_price_usd_per_unit * rec);
    let mut g = initial_guess;
    let mut last_step = f64::INFINITY;
    for iter in 1..=CUTOFF_MAX_ITER {
        let next_g = image(recovery_at(g)?);
        last_step = (next_g - g).abs();
        if last_step < CUTOFF_TOL {
            let residual = (next_g - image(recovery_at(next_g)?)).abs();
            return Ok(CutoffSolution {
                grade: next_g,
                iterations: iter,
                residual,
            });
        }
        g = 0.5 * g + 0.5 * next_g;
    }
    Err(CutoffError::NotConverged {
        iterations: CUTOFF_MAX_ITER,
        last_step,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Synthetic numbers (no issuer's figures): exercises the `L_corp` refusal and reconciliation.
    #[test]
    fn test_aisc_consolidation_bridge_l_corp_refusal_and_reconciliation() {
        let anchor = SourceAnchor::new("synthetic/aisc.xlsx", [0x11; 32], "Synthetic AISC!N206");
        let fy24 = AiscConsolidationBridge {
            metal: "Gold".to_string(),
            period_label: "SYNTHETIC-FY".to_string(),
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

    /// The decks carry the Circular's printed prices (E606); the quantities are synthetic.
    #[test]
    fn test_versioned_geo_and_reserve_replacement_bridge() {
        let deck = |y| PriceDeck::barrick_reserve(y).expect("registered deck year");
        let (d23, d24, d25) = (deck(2023), deck(2024), deck(2025));
        assert_eq!(
            (d23.gold_usd_per_oz, d23.copper_usd_per_lb),
            (1_300.0, 3.00)
        );
        assert_eq!(
            (d24.gold_usd_per_oz, d24.copper_usd_per_lb),
            (1_400.0, 3.00)
        );
        assert_eq!(
            (d25.gold_usd_per_oz, d25.copper_usd_per_lb),
            (1_500.0, 3.25)
        );
        assert_eq!(d25.silver_usd_per_oz, None);
        assert_eq!(d25.deck_id, "BARRICK-RESERVE-2025");
        assert!(PriceDeck::barrick_reserve(2026).is_none());

        // 100 Mlb Cu at $3.25/lb over $1,500/oz Au = 216.666… koz GEO; at $3.00 over $1,400 = 214.285… koz.
        let g25 = compute_geo(1_000.0, 100.0, &d25);
        let g24 = compute_geo(1_000.0, 100.0, &d24);
        assert!((g25.copper_geo_koz - 650.0 / 3.0).abs() < 1e-9);
        assert!((g24.copper_geo_koz - 1_500.0 / 7.0).abs() < 1e-9);
        assert!((g25.total_geo_koz - (1_000.0 + 650.0 / 3.0)).abs() < 1e-9);
        assert_eq!(g25.deck_id, "BARRICK-RESERVE-2025");

        // Synthetic: 4 Moz depleted, 8 Moz organic additions, 12 Moz project conversions.
        let rr = ReserveReplacementBridge::compute(4.0, 8.0, 12.0).expect("positive depletion");
        assert_eq!(
            (rr.organic_replacement_pct, rr.headline_replacement_pct),
            (200.0, 500.0)
        );
        assert!(ReserveReplacementBridge::compute(0.0, 8.0, 12.0).is_none());
        assert!(ReserveReplacementBridge::compute(-1.0, 8.0, 12.0).is_none());
        assert!(ReserveReplacementBridge::compute(f64::NAN, 8.0, 12.0).is_none());
    }

    #[test]
    fn test_fixed_point_cutoff_grade_solver() {
        let sol = solve_cutoff_grade(14.50, 8_500.0, 0.0025, |g| {
            0.90 * (1.0 - (-800.0 * g).exp())
        })
        .expect("converges");
        assert!(sol.iterations < 50);
        assert!(sol.grade > 0.0015 && sol.grade < 0.0030);
        assert!(sol.residual < 1e-9, "fixed-point residual {}", sol.residual);
    }

    /// Regression for the 2026-10-29 finding (bd-8wrk): each out-of-domain input used to be floored or clamped
    /// into a plausible number; now each refuses with its own variant.
    #[test]
    fn test_cutoff_refuses_out_of_domain_inputs() {
        let ok = |_: f64| 0.9;
        assert_eq!(
            solve_cutoff_grade(14.5, 0.0, 0.0025, ok),
            Err(CutoffError::NonPositivePrice(0.0))
        );
        assert_eq!(
            solve_cutoff_grade(14.5, -1.0, 0.0025, ok),
            Err(CutoffError::NonPositivePrice(-1.0))
        );
        assert_eq!(
            solve_cutoff_grade(0.0, 8_500.0, 0.0025, ok),
            Err(CutoffError::NonPositiveCost(0.0))
        );
        assert_eq!(
            solve_cutoff_grade(14.5, 8_500.0, 0.0, ok),
            Err(CutoffError::NonPositiveGuess(0.0))
        );
        assert!(matches!(
            solve_cutoff_grade(14.5, f64::NAN, 0.0025, ok),
            Err(CutoffError::NonPositivePrice(_))
        ));
        assert_eq!(
            solve_cutoff_grade(14.5, 8_500.0, 0.0025, |_| 1.2),
            Err(CutoffError::RecoveryOutOfDomain {
                at_grade: 0.0025,
                value: 1.2
            })
        );
        assert_eq!(
            solve_cutoff_grade(14.5, 8_500.0, 0.0025, |_| 0.0),
            Err(CutoffError::RecoveryOutOfDomain {
                at_grade: 0.0025,
                value: 0.0
            })
        );
        assert!(matches!(
            solve_cutoff_grade(14.5, 8_500.0, 0.0025, |_| f64::NAN),
            Err(CutoffError::RecoveryOutOfDomain { .. })
        ));
        // Recovery exactly 1 is in the domain.
        let one = solve_cutoff_grade(14.5, 8_500.0, 0.0025, |_| 1.0).expect("recovery 1 is valid");
        assert!((one.grade - 14.5 / 8_500.0).abs() < 1e-12);
    }

    /// A recovery that alternates between two values never settles; the solver must refuse after the cap
    /// instead of returning the last iterate as if solved.
    #[test]
    fn test_cutoff_refuses_non_convergence() {
        let mut calls = 0_u32;
        let err = solve_cutoff_grade(14.5, 8_500.0, 0.0025, |_| {
            calls += 1;
            if calls % 2 == 0 { 0.5 } else { 0.9 }
        })
        .expect_err("alternating recovery cannot converge");
        match err {
            CutoffError::NotConverged {
                iterations,
                last_step,
            } => {
                assert_eq!(iterations, CUTOFF_MAX_ITER);
                assert!(last_step >= CUTOFF_TOL);
            }
            other => panic!("expected NotConverged, got {other:?}"),
        }
    }

    /// Seeded property test: for valid inputs the returned grade satisfies the fixed-point equation; for an
    /// invalid recovery no `Ok` is ever returned.
    #[test]
    fn test_cutoff_property_seeded() {
        use fmine_core::rand::{PhiloxStream, RngKey};
        let mut rng = PhiloxStream::new(RngKey {
            seed_root: 0x00C0_7FF0,
            partition: 0,
            domain: 0,
            var_group: 0,
            realization: 0,
            brick: 0,
        });
        for case in 0..500 {
            let cost = 1.0 + 49.0 * rng.next_f64_open01();
            let price = 1_000.0 + 9_000.0 * rng.next_f64_open01();
            let cap = 0.6 + 0.39 * rng.next_f64_open01();
            let k = 100.0 + 900.0 * rng.next_f64_open01();
            let recovery = |g: f64| cap * (1.0 - (-k * g).exp());
            match solve_cutoff_grade(cost, price, 0.005, recovery) {
                Ok(sol) => {
                    let implied = cost / (price * recovery(sol.grade));
                    assert!(
                        (sol.grade - implied).abs() < 1e-8,
                        "case {case}: grade {} vs implied {implied}",
                        sol.grade
                    );
                }
                Err(CutoffError::NotConverged { .. } | CutoffError::RecoveryOutOfDomain { .. }) => {
                }
                Err(other) => panic!("case {case}: valid inputs refused with {other:?}"),
            }
            let bad = 1.0 + rng.next_f64_open01();
            assert!(
                solve_cutoff_grade(cost, price, 0.005, |_| bad).is_err(),
                "case {case}"
            );
        }
    }
}
