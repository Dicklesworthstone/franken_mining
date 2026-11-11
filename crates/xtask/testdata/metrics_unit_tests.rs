#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn public_cost_rate_bounds_keep_decimal_scales_signs_and_invalid_steps_explicit() {
        use crate::public_derivation::{CostRateUnit, cost_rate_bounds};
        use crate::roll_forward::quarter::Printed;
        use fmine_numerics::exact_feas::Rat;
        let printed = |value, step_den| Printed {
            value: Rat::from_integer(value),
            step: Rat::new(1, step_den).unwrap(),
        };
        for (unit, numerator, n_step, q_step, lo, hi) in [
            (
                CostRateUnit::GoldKoz,
                150,
                100,
                100,
                (29999000, 20001),
                (30001000, 19999),
            ),
            (
                CostRateUnit::GoldKoz,
                -150,
                100,
                100,
                (-30001000, 19999),
                (-29999000, 20001),
            ),
            (CostRateUnit::GoldKoz, 0, 100, 10, (-100, 1999), (100, 1999)),
            (
                CostRateUnit::CopperMillionPounds,
                200,
                100,
                100,
                (39999, 20001),
                (40001, 19999),
            ),
        ] {
            let result =
                cost_rate_bounds(unit, printed(numerator, n_step), printed(100, q_step)).unwrap();
            assert_eq!(result.lo, Rat::new(lo.0, lo.1).unwrap());
            assert_eq!(result.hi, Rat::new(hi.0, hi.1).unwrap());
            assert!(!result.lo_strict && !result.hi_strict);
        }
        for step in [0, -1] {
            let invalid = Printed {
                value: Rat::from_integer(100),
                step: Rat::from_integer(step),
            };
            for (n, q) in [(invalid, printed(100, 1)), (printed(100, 1), invalid)] {
                assert_eq!(
                    cost_rate_bounds(CostRateUnit::GoldKoz, n, q)
                        .unwrap_err()
                        .code,
                    "FMINE-REF-207"
                );
            }
        }
        for value in [
            Rat::new(1, 2).unwrap(),
            Rat::from_integer(0),
            Rat::from_integer(-1),
        ] {
            let q = Printed {
                value,
                step: Rat::from_integer(1),
            };
            assert_eq!(
                cost_rate_bounds(CostRateUnit::GoldKoz, printed(100, 1), q)
                    .unwrap_err()
                    .code,
                "FMINE-REF-104"
            );
        }
        assert_eq!(
            cost_rate_bounds(
                CostRateUnit::GoldKoz,
                printed(i128::MAX, 1),
                printed(100, 1)
            )
            .unwrap_err()
            .code,
            "FMINE-REF-207"
        );
    }

    #[test]
    fn public_cost_rate_claim_preserves_parent_metadata_and_refuses_mixed_families() {
        use crate::public_derivation::{CostRateUnit, derive_cost_rate};
        use crate::roll_forward::quarter::Printed;
        use fmine_claim::{
            AdditivityClass, Claim, Confidentiality, Representation, SpatialSupport, UuTerm,
        };
        use fmine_evidence::{EpistemicStatus, EvidenceClass, KnowledgeState, KuKind};
        use fmine_numerics::exact_feas::Rat;
        let np = Printed {
            value: Rat::from_integer(200),
            step: Rat::new(1, 100).unwrap(),
        };
        let qp = Printed {
            value: Rat::from_integer(100),
            ..np
        };
        let common = UuTerm {
            scenario_id: "invented-accounting-correction".into(),
            description: "Synthetic unknown cost allocation".into(),
            downside_exposure_usd_m: 1.0,
            trigger_indicator: "later issuer correction".into(),
        };
        let mut extra = common.clone();
        extra.scenario_id = "invented-sales-correction".into();
        let parent = |label: &str, value, evidence, state, terms| {
            Claim::new(
                "public_disclosure.cost_operand",
                value,
                label,
                Representation::Deterministic,
                SpatialSupport::PeriodAggregate,
                AdditivityClass::ExtensiveAdditive,
                evidence,
                EpistemicStatus::KnownUnknown(KuKind::Parametric),
                state,
                Confidentiality::public(),
                terms,
                vec![],
                vec![
                    SourceAnchor::new("invented.xlsx", [41; 32], label),
                    SourceAnchor::new("invented.xlsx", [41; 32], "shared-note"),
                ],
            )
        };
        let n = parent(
            "N",
            np,
            EvidenceClass::Measured,
            KnowledgeState::Known,
            vec![common.clone()],
        );
        let q = parent(
            "Q",
            qp,
            EvidenceClass::ISSUER,
            KnowledgeState::Estimated,
            vec![common.clone(), extra.clone()],
        );
        let derive = |unit, family, n_printed, q_printed| {
            derive_cost_rate(unit, family, (&n, n_printed), (&q, q_printed))
        };
        let result = derive(
            CostRateUnit::CopperMillionPounds,
            "c1_per_lb",
            Some(np),
            Some(qp),
        )
        .unwrap();
        assert!(result.check_id().is_ok());
        assert_eq!(result.metric_key, "public-disclosure.copper-cost-rate/1");
        assert!(result.value_tag().starts_with("c1_per_lb:"));
        assert_eq!(result.value.lo, Rat::new(39999, 20001).unwrap());
        assert_eq!(result.value.hi, Rat::new(40001, 19999).unwrap());
        assert_eq!(result.evidence, EvidenceClass::ISSUER);
        assert_eq!(result.state, KnowledgeState::Estimated);
        assert_eq!(result.epistemic, q.epistemic);
        assert_eq!(result.uu_terms, vec![common, extra]);
        assert_eq!(result.additivity, AdditivityClass::IntensiveWeighted);
        assert_eq!(result.support, SpatialSupport::PeriodAggregate);
        assert!(result.confidentiality.is_public());
        let mut parents = vec![n.claim_id, q.claim_id];
        parents.sort();
        assert_eq!(result.parents, parents);
        let mut anchors = n.anchors.clone();
        anchors.extend(q.anchors.clone());
        anchors.sort();
        anchors.dedup();
        assert_eq!(result.anchors, anchors);
        assert_eq!(result.anchors.len(), 3);
        for (n_printed, q_printed) in [(None, Some(qp)), (Some(np), None)] {
            assert_eq!(
                derive(
                    CostRateUnit::CopperMillionPounds,
                    "c1_per_lb",
                    n_printed,
                    q_printed
                )
                .unwrap_err()
                .code,
                "FMINE-REF-208"
            );
        }
        for (unit, family) in [
            (CostRateUnit::GoldKoz, "c1_per_lb"),
            (CostRateUnit::CopperMillionPounds, "aisc_per_oz"),
            (CostRateUnit::CopperMillionPounds, "cost_of_sales_per_lb"),
        ] {
            assert_eq!(
                derive(unit, family, Some(np), Some(qp)).unwrap_err().code,
                "FMINE-REF-105"
            );
        }
    }

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
