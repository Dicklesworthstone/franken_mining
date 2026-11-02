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
            if calls.is_multiple_of(2) { 0.5 } else { 0.9 }
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
