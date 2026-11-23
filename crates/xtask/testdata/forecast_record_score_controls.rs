#[cfg(test)]
mod tests {
    use super::*;

    fn args(words: &[&str]) -> Vec<String> {
        words.iter().map(|s| (*s).into()).collect()
    }

    #[test]
    fn record_scoring_requires_explicit_record_cut_and_trust_roots() {
        let valid = args(&[
            "forecast",
            "record",
            "score",
            "--record",
            "record",
            "--as-of",
            "2030-02-01T00:00:00Z",
            "--tsa-roots",
            "trusted.pem",
        ]);
        let (command, format, _) = parse(&valid, Path::new(".")).expect("explicit inputs");
        assert_eq!(command.record, Path::new("record"));
        assert_eq!(format, OutputFormat::Json);
        for flag in ["--record", "--as-of", "--tsa-roots"] {
            let mut missing = valid.clone();
            let at = missing.iter().position(|s| s == flag).expect("flag");
            missing.drain(at..at + 2);
            assert!(parse(&missing, Path::new(".")).is_err());
        }
        for extra in [
            args(&["--trust-host-clock"]),
            args(&["--selector", "latest-as-of"]),
            args(&["--as-of", "2030-02-01T00:00:00Z"]),
            args(&["--json", "--format", "markdown"]),
        ] {
            let mut invalid = valid.clone();
            invalid.extend(extra);
            assert!(parse(&invalid, Path::new(".")).is_err());
        }
        let mut short = valid;
        short[6] = "2030-02-01".into();
        assert!(parse(&short, Path::new(".")).is_err());
    }

    #[test]
    fn quarterly_proof_upper_bound_must_precede_the_entire_scored_quarter_end() {
        let mut rules =
            record::parse_rules(include_str!("../../../forecasts/fy2026/rules.toml")).unwrap();
        for target in &mut rules.targets {
            target.outcome = record::Outcome::QuarterQtd;
        }
        for baseline in &mut rules.baselines {
            if let record::BaselineKind::Persistence { ref mut point, .. } = baseline.kind {
                *point = record::Quarter { year: 2026, q: 4 };
            }
        }
        let end = parse_utc("2027-04-01T00:00:00Z").unwrap();
        assert_eq!(
            require_pre_outcome_proof(&rules, end - 1)
                .unwrap()
                .to_string(),
            "2027Q1"
        );
        for upper in [end, end + 1, parse_utc("2027-12-31T00:00:00Z").unwrap()] {
            let error = require_pre_outcome_proof(&rules, upper).unwrap_err();
            assert_eq!(error.code, "FMINE-REF-206");
            assert!(error.what.contains("scored quarter"));
        }
        let annual =
            record::parse_rules(include_str!("../../../forecasts/fy2026/rules.toml")).unwrap();
        assert!(
            require_pre_outcome_proof(&annual, parse_utc("2026-12-31T23:59:59Z").unwrap()).is_ok()
        );
        assert!(
            require_pre_outcome_proof(&annual, parse_utc("2027-01-01T00:00:00Z").unwrap()).is_err()
        );
    }

    #[test]
    fn post_quarter_proofs_require_explicit_frozen_mode_and_earliest_release_deadline() {
        let annual = record::parse_rules(include_str!("../../../forecasts/fy2026/rules.toml")).unwrap();
        let mut nowcast = annual.clone();
        for target in &mut nowcast.targets {
            target.outcome = record::Outcome::QuarterQtdNowcast;
        }
        nowcast.dates = record::RecordDates {
            nowcast_from: parse_utc("2027-01-01T00:00:00Z").unwrap(),
            proof_by: parse_utc("2027-01-14T23:59:59Z").unwrap(),
        };
        nowcast.nowcast_availability = Some(record::NowcastAvailability {
            earliest_release_utc: parse_utc("2027-01-15T00:00:00Z").unwrap(),
            basis: "Synthetic first preliminary announcement".into(),
        });
        let proof = parse_utc("2027-01-05T00:00:00Z").unwrap();
        assert_eq!(require_pre_outcome_proof(&nowcast, proof).unwrap().to_string(), "2026Q4");
        assert_eq!(require_pre_outcome_proof(&annual, proof).unwrap_err().code, "FMINE-REF-206");
        for upper in [
            parse_utc("2026-12-31T23:59:59Z").unwrap(),
            parse_utc("2027-01-15T00:00:00Z").unwrap(),
        ] {
            assert_eq!(
                require_pre_outcome_proof(&nowcast, upper).unwrap_err().code,
                "FMINE-REF-206"
            );
        }
        nowcast.dates.proof_by = nowcast.nowcast_availability.as_ref().unwrap().earliest_release_utc;
        assert!(require_pre_outcome_proof(&nowcast, proof).is_err());
    }

    #[test]
    fn markdown_preserves_exact_signed_zero_missing_and_not_published_members() {
        use record::scoring::paired::{Availability, BaselineRow, Bounds, OutcomeRow, assess};
        use record::{Rat, RulesSite, SiteStatus};

        let mut rules =
            record::parse_rules(include_str!("../../../forecasts/fy2026/rules.toml")).unwrap();
        rules.targets.retain(|t| t.id == "unit_cost");
        rules.baselines.retain(|b| b.target == "unit_cost");
        rules.sites = ["Credit", "Zero", "Pending", "Excluded"]
            .into_iter()
            .map(|site| RulesSite {
                name: site.into(),
                status: if site == "Excluded" {
                    SiteStatus::NotPublished
                } else {
                    SiteStatus::Live
                },
                locator: format!("synthetic:{site}"),
                reason: (site == "Excluded").then(|| "frozen exclusion".into()),
            })
            .collect();
        let forecasts = [
            ("Credit", -125, -123, -115),
            ("Zero", -1, 0, 1),
            ("Pending", -6, -4, -2),
        ]
        .into_iter()
        .map(|(site, lo, point, hi)| ForecastRow {
            site: site.into(),
            target: "unit_cost".into(),
            lo: Rat::from_integer(lo),
            point: Rat::from_integer(point),
            hi: Rat::from_integer(hi),
        })
        .collect::<Vec<_>>();
        let outcomes = [
            ("Credit", Availability::Available(Rat::from_integer(-120))),
            ("Zero", Availability::Available(Rat::from_integer(0))),
            (
                "Pending",
                Availability::Unavailable("pending\nrelease".into()),
            ),
            (
                "Excluded",
                Availability::Unavailable("frozen exclusion".into()),
            ),
        ]
        .into_iter()
        .map(|(site, value)| OutcomeRow {
            site: site.into(),
            target: "unit_cost".into(),
            value,
        })
        .collect::<Vec<_>>();
        let bounds = |lo, hi| {
            Availability::Available(Bounds {
                lo: Rat::from_integer(lo),
                hi: Rat::from_integer(hi),
            })
        };
        let baselines = [
            ("Credit", "issuer_guidance", bounds(-130, -110)),
            ("Credit", "run_rate_persistence", bounds(-125, -115)),
            (
                "Zero",
                "issuer_guidance",
                Availability::Unavailable("missing | guidance\nrelease".into()),
            ),
            (
                "Zero",
                "run_rate_persistence",
                Availability::Available(Bounds {
                    lo: Rat::new(-1, 2).unwrap(),
                    hi: Rat::new(1, 2).unwrap(),
                }),
            ),
            ("Pending", "issuer_guidance", bounds(-6, -2)),
            ("Pending", "run_rate_persistence", bounds(-5, -3)),
            (
                "Excluded",
                "issuer_guidance",
                Availability::Unavailable("frozen exclusion".into()),
            ),
            (
                "Excluded",
                "run_rate_persistence",
                Availability::Unavailable("frozen exclusion".into()),
            ),
        ]
        .into_iter()
        .map(|(site, baseline, value)| BaselineRow {
            site: site.into(),
            target: "unit_cost".into(),
            baseline: baseline.into(),
            value,
        })
        .collect::<Vec<_>>();
        for (field, unit) in [
            ("aisc_usd_per_oz", "usd_per_oz"),
            ("c1_usd_per_lb", "usd_per_lb"),
            ("aisc_usd_per_lb", "usd_per_lb"),
        ] {
            rules.targets[0].field = field.into();
            let assessment = assess(&rules, &forecasts, &outcomes, &baselines).unwrap();
            // Both resolved outcomes are inside: losses equal the registered widths, even at zero.
            assert_eq!(
                assessment.members[0].forecast_score.unwrap().interval_score,
                Rat::from_integer(10)
            );
            assert_eq!(
                assessment.members[1].forecast_score.unwrap().interval_score,
                Rat::from_integer(2)
            );
            assert_eq!(
                assessment.members[0].baselines[0]
                    .score
                    .unwrap()
                    .interval_score,
                Rat::from_integer(20)
            );
            assert_eq!(
                assessment.members[1].baselines[1]
                    .score
                    .unwrap()
                    .interval_score,
                Rat::from_integer(1)
            );
            assert_eq!(assessment.targets[0].n_paired, 1);
            let markdown = scores_markdown(&assessment, &rules);
            assert!(markdown.contains(&format!("Metric: `{field}`. Unit: `{unit}`.")));
            assert!(markdown.contains("| Site | Frozen status | Outcome | Forecast point | Forecast bounds | Baseline bounds | Paired |"));
            let table = markdown.split_once("Frozen member inputs;").unwrap().1;
            let rows = table
                .lines()
                .filter(|line| line.starts_with("| "))
                .skip(1)
                .collect::<Vec<_>>();
            assert_eq!(
                rows,
                [
                    "| Credit | live | -120 | -123 | [-125, -115] | issuer guidance: [-130, -110]; run rate persistence: [-125, -115] | yes |",
                    "| Zero | live | 0 | 0 | [-1, 1] | issuer guidance: unavailable: missing \\| guidance release; run rate persistence: [-1/2, 1/2] | no |",
                    "| Pending | live | unavailable: pending release | -4 | [-6, -2] | issuer guidance: [-6, -2]; run rate persistence: [-5, -3] | no |",
                    "| Excluded | not published | unavailable: frozen exclusion | not published | not published | issuer guidance: unavailable: frozen exclusion; run rate persistence: unavailable: frozen exclusion | no |",
                ]
            );
        }
    }
}
