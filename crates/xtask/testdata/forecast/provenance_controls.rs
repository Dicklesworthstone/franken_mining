#[cfg(test)]
mod tests {
    use super::*;
    use fmine_calib::OutcomeSelector;

    const BUNDLE: &str = include_str!("../../../../tests/forecasts/provenance_synthetic.toml");
    const LEDGER: &str = include_str!("../../../../tests/forecasts/cohort_synthetic.toml");

    fn check(bundle: &str, ledger: &str) -> Result<String, AssessmentFailure> {
        let mut replay = Replay::parse(ledger).expect("synthetic ledger");
        let assessment = replay
            .assessment(
                "synthetic-demo",
                super::super::time("2026-10-31T00:00:00Z").expect("time"),
                OutcomeSelector::FirstPublished,
            )
            .expect("assessment");
        Bundle::parse(bundle)?.verify(&replay, &assessment, &sha256(bundle.as_bytes()).to_hex())
    }

    #[test]
    fn all_frozen_members_are_checked_at_registration_including_missing_outcomes() {
        let result = check(BUNDLE, LEDGER).expect("clean ancestry");
        assert_eq!(result, check(BUNDLE, LEDGER).expect("same inputs"));
        assert_eq!(result.matches("\"cone_digest\"").count(), 3);
        assert_eq!(
            result
                .matches("\"decision_cut\":\"2026-07-01T00:00:00Z\"")
                .count(),
            3
        );
        assert!(result.contains("south-unit-cost"));
        assert!(result.contains("\"time_evidence\":\"supplied_not_verified\""));
        assert!(result.contains("\"generator_replay\":\"not_performed\""));
    }

    #[test]
    fn fitted_input_exclusion_and_deck_leaks_refuse_with_frontier_paths() {
        for (before, node) in [
            (
                "2026-06-15T00:00:00Z",
                "evd_11111111111111111111111111111111",
            ),
            (
                "2026-06-20T00:00:00Z",
                "evd_33333333333333333333333333333333",
            ),
            (
                "2026-06-25T00:00:00Z",
                "evd_44444444444444444444444444444444",
            ),
        ] {
            // The evidence was visible by October's assessment, but unavailable at July's registration.
            let changed = BUNDLE.replace(before, "2026-08-01T00:00:00Z");
            let error = check(&changed, LEDGER).expect_err("future evidence");
            assert_eq!(error.refusal.code, "FMINE-REF-110");
            let diagnostics = error.diagnostics.expect("typed cone diagnostic");
            assert!(
                diagnostics.contains(&format!("\"node\":\"{node}\"")),
                "{diagnostics}"
            );
            assert!(diagnostics.contains("\"path\":[\"for_"));
            assert!(diagnostics.contains("\"after_cut\":true"));
            assert!(diagnostics.contains("2026-07-01T00:00:00Z"));
        }
    }

    #[test]
    fn anchor_visibility_is_required_even_when_wall_clock_is_early() {
        let changed = BUNDLE.replace(
            "\ncell = \"observations\"\nseq = 10",
            "\ncell = \"observations\"\nseq = 9",
        );
        let error = check(&changed, LEDGER).expect_err("outside anchor");
        assert_eq!(error.refusal.code, "FMINE-REF-110");
        assert!(
            error
                .diagnostics
                .expect("diagnostic")
                .contains("\"after_anchor\":true")
        );
    }

    #[test]
    fn missing_time_missing_member_and_wrong_forecast_binding_never_pass() {
        let missing_time = BUNDLE.replace("available_at = \"2026-06-15T00:00:00Z\"\n", "");
        assert_eq!(
            check(&missing_time, LEDGER)
                .expect_err("missing time")
                .refusal
                .code,
            "FMINE-REF-110"
        );
        let tail = BUNDLE.rfind("[[binding]]").expect("final binding");
        let error = check(&BUNDLE[..tail], LEDGER).expect_err("missing unresolved member");
        assert!(error.refusal.what.contains("south-unit-cost"));
        let changed = LEDGER.replace("lo = \"90\"", "lo = \"91\"");
        let error = check(BUNDLE, &changed).expect_err("different forecast payload");
        assert!(error.refusal.what.contains("frozen forecast ID"));
        let changed = BUNDLE.replace(
            "performed_at = \"2026-07-01T00:00:00Z\"",
            "performed_at = \"2026-06-30T00:00:00Z\"",
        );
        assert!(
            check(&changed, LEDGER)
                .expect_err("backdated forecast root")
                .refusal
                .what
                .contains("registered_at")
        );
    }

    #[test]
    fn strict_schema_and_lane_checks_preserve_the_evidence_boundary() {
        for changed in [
            BUNDLE.replace("lane = \"synthetic\"", "lane = \"public\""),
            BUNDLE.replace("kind = \"observation\"", "kind = \"invented\""),
            BUNDLE.replace("origin = \"source\"", "origin = \"guess\""),
            BUNDLE.replace("seq = 10", "seq = -1"),
            format!(
                "{BUNDLE}\n[[anchor]]\nid = \"registration\"\ncell = \"observations\"\nseq = 10\n"
            ),
            format!(
                "{BUNDLE}\n[[binding]]\nforecast = \"south-unit-cost\"\nforecast_id = \"for_76ff13dac8c451928240cc7c75548508\"\nanchor = \"registration\"\n"
            ),
        ] {
            assert!(
                Bundle::parse(&changed).is_err(),
                "malformed provenance accepted"
            );
        }
        assert!(
            check(
                &BUNDLE.replace("lane = \"synthetic\"", "lane = \"supplied\""),
                LEDGER
            )
            .is_err()
        );
    }

    #[test]
    fn unrelated_future_objects_do_not_rewrite_any_cone_digest() {
        let extra = format!(
            "{BUNDLE}\n[[node]]\nid = \"evd_55555555555555555555555555555555\"\nkind = \"registry_row\"\norigin = \"source\"\navailable_at = \"2027-01-01T00:00:00Z\"\n"
        );
        let original = Bundle::parse(BUNDLE).expect("bundle");
        let changed = Bundle::parse(&extra).expect("bundle with an unrelated incomplete node");
        for binding in original.bindings.values() {
            let cut = ConeCut {
                decision_ns: super::super::time("2026-07-01T00:00:00Z").expect("time"),
                anchor: original.anchors[&binding.anchor].clone(),
            };
            assert_eq!(
                original
                    .graph
                    .check(binding.forecast_id, &cut)
                    .expect("clean")
                    .digest(),
                changed
                    .graph
                    .check(binding.forecast_id, &cut)
                    .expect("unrelated")
                    .digest()
            );
        }
    }
}
