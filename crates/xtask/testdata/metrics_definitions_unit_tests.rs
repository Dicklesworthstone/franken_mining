#[cfg(test)]
mod tests {
    use super::*;
    use crate::{PriceDeck, compute_geo};

    const AISC: &str = "aisc@wgc-2018";

    fn usd(text: &str) -> ExactQty {
        ExactQty::from_decimal_str(text).expect("decimal")
    }

    fn line(item: &str, amount: &str) -> CostLine {
        CostLine {
            item: item.to_string(),
            usd: usd(amount),
        }
    }

    fn registry() -> MetricRegistry {
        bundled_registry().expect("the committed registry parses")
    }

    fn cost(id: &str, lines: &[CostLine]) -> Result<AllInCost, MetricFailure> {
        all_in_cost_per_oz(&registry(), id, lines, usd("100"), None)
    }

    /// Synthetic amounts, so the arithmetic can be read off: (800 + 300 + 100 − 200) / 100 oz.
    #[test]
    fn test_aisc_counts_its_lines_and_names_what_it_left_out() {
        let got = cost(
            AISC,
            &[
                line("on_site_mining_costs", "800"),
                line("corporate_general_and_administrative", "300"),
                line("lease_payments", "100"),
                line("by_product_credits", "200"),
                line("income_tax", "999"),
            ],
        )
        .expect("computes");
        assert_eq!(got.usd_per_oz, usd("10"));
        assert_eq!(got.left_out, vec!["income_tax"]);
        assert_eq!(got.counted.len(), 4);
        assert_eq!(got.definition, AISC);
        assert_eq!(got.superseded_by, None);
    }

    /// AIC is AISC plus the non-sustaining lines. The same line is refused under AISC, because AISC neither counts
    /// nor excludes it: growth capital cannot slip through either metric unseen.
    #[test]
    fn test_non_sustaining_capital_is_in_aic_and_refused_in_aisc() {
        let lines = [
            line("on_site_mining_costs", "800"),
            line("capital_expenditure_non_sustaining", "400"),
        ];
        let aic = cost("aic@wgc-2018", &lines).expect("computes");
        assert_eq!(aic.usd_per_oz, usd("12"));
        assert_eq!(
            cost(AISC, &lines),
            Err(MetricFailure::UnregisteredItem {
                definition: AISC.to_string(),
                item: "capital_expenditure_non_sustaining".to_string(),
            })
        );
    }

    /// One item, two versions: the same inputs differ by the lease line, and the older result says it is superseded.
    #[test]
    fn test_a_definition_change_moves_the_result_and_is_named() {
        let lines = [
            line("on_site_mining_costs", "800"),
            line("lease_payments", "100"),
        ];
        let older = cost("aisc@wgc-2013", &lines).expect("computes");
        let newer = cost(AISC, &lines).expect("computes");
        assert_eq!(older.usd_per_oz, usd("8"));
        assert_eq!(older.left_out, vec!["lease_payments"]);
        assert_eq!(older.superseded_by.as_deref(), Some(AISC));
        assert_eq!(newer.usd_per_oz, usd("9"));
    }

    #[test]
    fn test_co_product_needs_a_share_and_drops_credits() {
        let id = "aisc.co_product@wgc-2018";
        let lines = [
            line("on_site_mining_costs", "1000"),
            line("by_product_credits", "300"),
        ];
        let reg = registry();
        let got =
            all_in_cost_per_oz(&reg, id, &lines, usd("100"), Some(usd("0.75"))).expect("computes");
        assert_eq!(got.usd_per_oz, usd("7.5"));
        assert_eq!(got.left_out, vec!["by_product_credits"]);
        for share in [None, Some(usd("0")), Some(usd("1.01"))] {
            assert!(matches!(
                all_in_cost_per_oz(&reg, id, &lines, usd("100"), share),
                Err(MetricFailure::AllocationRequired { .. })
            ));
        }
        assert!(matches!(
            all_in_cost_per_oz(&reg, AISC, &lines, usd("100"), Some(usd("0.75"))),
            Err(MetricFailure::AllocationNotApplicable { .. })
        ));
    }

    #[test]
    fn test_refusals() {
        let lines = [line("on_site_mining_costs", "1")];
        assert_eq!(
            cost("aisc@wgc-1999", &lines),
            Err(MetricFailure::UnknownDefinition(
                "aisc@wgc-1999".to_string()
            ))
        );
        let rr = "reserve_replacement.geo@barrick-2026-circular";
        assert_eq!(
            cost(rr, &lines),
            Err(MetricFailure::NotACostDefinition(rr.to_string()))
        );
        assert_eq!(
            all_in_cost_per_oz(&registry(), AISC, &lines, usd("0"), None),
            Err(MetricFailure::NonPositiveOunces)
        );
    }

    fn major(uplift: &str, months: Option<i64>) -> bool {
        let reg = registry();
        is_non_sustaining(
            &reg.project_tests[0],
            Project::AtExistingOperation {
                uplift_percent: usd(uplift),
                months_to_net_cash: months,
            },
        )
        .expect("classifies")
    }

    fn extension(months: i64, years: &str) -> bool {
        let reg = registry();
        is_non_sustaining(
            &reg.project_tests[0],
            Project::Extension {
                duration_months: months,
                ore_phase_years: usd(years),
            },
        )
        .expect("classifies")
    }

    /// FAQ 7: "at least a 10% increase" and no net cash "within 12 months". Both conditions, and both edges.
    #[test]
    fn test_major_project_needs_both_conditions() {
        assert!(major("10", None));
        assert!(major("10", Some(13)));
        assert!(!major("10", Some(12)));
        assert!(!major("9.99", None));
        assert!(!major("50", Some(6)));
    }

    /// FAQ 10: sustaining unless at least 12 months and more than five years of ore.
    #[test]
    fn test_extension_needs_both_conditions() {
        assert!(extension(12, "5.1"));
        assert!(!extension(12, "5"));
        assert!(!extension(11, "10"));
        assert!(extension(36, "12"));
    }

    /// The Circular's decks come from the registry (E606), and a year it does not print stays unregistered.
    #[test]
    fn test_price_decks_read_from_the_registry() {
        let deck = PriceDeck::barrick_reserve(2025).expect("registered");
        assert_eq!(deck.deck_id, "BARRICK-RESERVE-2025");
        assert_eq!(
            (deck.gold_usd_per_oz, deck.copper_usd_per_lb),
            (1_500.0, 3.25)
        );
        assert!(PriceDeck::barrick_reserve(2022).is_none());
        let geo = compute_geo(1_000.0, 1.0, &deck);
        assert!((geo.copper_geo_koz - 2.166_666_666_666_667).abs() < 1e-12);
    }
}
