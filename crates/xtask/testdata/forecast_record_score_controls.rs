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
}
