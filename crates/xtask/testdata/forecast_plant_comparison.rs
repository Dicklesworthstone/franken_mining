//! SYNTHETIC: raw and fitted plant models are registered before their common processing interval,
//! resolved from the same supplied observation, then compared using only immutable saved reports.

use super::*;
use fmine_calib::ledger::comparison::PairedScore;
use fmine_core::Digest32;
use fmine_io::omf::json::Json;

fn field<'a>(value: &'a Json, keys: &[&str]) -> &'a Json {
    keys.iter()
        .fold(value, |value, key| value.get(key).expect(key))
}

fn saved(root: &Path, cohort: &str, cut: i64, now: i64) -> Digest32 {
    let envelope = execute_with_clock(
        &Command::Assess {
            journal: "history".into(),
            head: head(root),
            cohort: cohort.into(),
            as_of: format_utc(cut / NS_PER_SECOND),
            selector: "first-published".into(),
            expected_digest: None,
        },
        root,
        &Clock(now),
    );
    assert_eq!(envelope.exit_code, 0, "{envelope:?}");
    let json = parse_json(envelope.payload_json.as_bytes()).unwrap();
    hex(
        field(&json, &["report", "assessment", "digest"])
            .as_str()
            .unwrap(),
    )
    .unwrap()
}

fn compare(root: &Path, left: Digest32, right: Digest32, score: PairedScore) -> crate::Envelope {
    execute_with_clock(
        &Command::CompareSupplied {
            journal: "history".into(),
            head: Some(head(root)),
            left,
            right,
            score,
        },
        root,
        &NoClock,
    )
}

fn fraction(value: &Json, num: &str, den: &str) {
    assert_eq!(field(value, &["num"]).as_str(), Some(num));
    assert_eq!(field(value, &["den"]).as_str(), Some(den));
}

#[test]
fn saved_plant_comparison_scores_original_and_fitted_distributions_after_source_loss() {
    let start = time("2030-01-02T00:00:00Z").unwrap();
    let root = fixture(start);
    let first = start - 8 * HOUR;
    let first_end = first + HOUR;
    let second = first + 2 * HOUR;
    let second_end = first + 3 * HOUR;
    let evidence_cut = first + 4 * HOUR;
    let raw = std::fs::read_to_string(root.join("recovery.toml"))
        .unwrap()
        .replace(
            "60 + 10 * Au - P80 / 10",
            "50 + 12.5 * Au - P80 / 8",
        )
        .replace(
            &format!("valid_start_ns = \"{start}\""),
            &format!("valid_start_ns = \"{first}\""),
        );
    std::fs::write(root.join("recovery.toml"), &raw).unwrap();
    init(&root, start - 2 * HOUR);
    let registered_raw = execute_with_clock(
        &command(&root, Target::Recovery),
        &root,
        &Clock(start - HOUR),
    );
    assert_eq!(registered_raw.exit_code, 0, "{registered_raw:?}");
    assert!(
        registered_raw
            .payload_json
            .contains("\"samples\":[0.875,0.5]")
    );
    let fitted = format!(
        r#"{raw}
[[correction_fit]]
id = "invented-physical-pair-fit"
revision = "v1"
method = "unweighted_additive_residual_mean"
baseline = "post_operational_pre_correction"
alignment = "same_processing_support"
evidence_cut_ns = "{evidence_cut}"

[[correction_pair]]
id = "earlier-support"
start_ns = "{first}"
end_ns = "{first_end}"
available_ns = "{first_end}"
baseline_fraction = "0.5"
observed_fraction = "0.5625"
uncensored = "true"

[[correction_pair]]
id = "later-support"
start_ns = "{second}"
end_ns = "{second_end}"
available_ns = "{second_end}"
baseline_fraction = "0.75"
observed_fraction = "0.9375"
uncensored = "true"
"#,
    );
    std::fs::write(root.join("fitted-recovery.toml"), fitted).unwrap();
    let registered_fit = execute_with_clock(
        &Command::RegisterPlant {
            journal: "history".into(),
            head: head(&root),
            input: "production.toml".into(),
            grinding: "grinding.toml".into(),
            recovery: "fitted-recovery.toml".into(),
            id: "fitted".into(),
            target: Target::Recovery,
            author_role: "synthetic-fitted-model-role".into(),
            time_basis: "utc-unix-ns".into(),
        },
        &root,
        &Clock(start - HOUR + 100),
    );
    assert_eq!(registered_fit.exit_code, 0, "{registered_fit:?}");
    // The two exact residuals, 1/16 and 3/16, replace the nominal correction with 1/8.
    assert!(registered_fit.payload_json.contains("\"samples\":[1,0.625]"));
    assert!(registered_fit.payload_json.contains("earlier-support"));
    assert!(registered_fit.payload_json.contains("later-support"));
    for (index, (name, member)) in [("raw", "production-1"), ("fitted", "fitted")]
        .into_iter()
        .enumerate()
    {
        std::fs::write(
            root.join("cohort.toml"),
            format!("[[cohort]]\nname = \"{name}\"\nmembers = \"{member}\"\nresolution_rule = \"Supplied recovery fraction over the complete frozen processing interval\"\neligible_sources = \"synthetic:mill-report\"\nalpha = \"0.5\"\noutcome_lo = \"0\"\noutcome_hi = \"1\"\n"),
        )
        .unwrap();
        let frozen = execute_with_clock(
            &Command::Freeze {
                journal: "history".into(),
                head: head(&root),
                request: "cohort.toml".into(),
            },
            &root,
            &Clock(start - HOUR + 1_000 + index as i64),
        );
        assert_eq!(frozen.exit_code, 0, "{frozen:?}");
    }

    let end = start + HOUR;
    let pending = [
        saved(&root, "raw", end, end + 100),
        saved(&root, "fitted", end, end + 200),
    ];
    let pending_result = compare(&root, pending[0], pending[1], PairedScore::Crps);
    assert_eq!(pending_result.exit_code, 0, "{pending_result:?}");
    let json = parse_json(pending_result.payload_json.as_bytes()).unwrap();
    let report = field(&json, &["comparison"]);
    assert_eq!(field(report, &["n_unresolved"]).u64(), Some(1));
    fraction(field(report, &["whole_population", "lower"]), "-1", "8");
    fraction(field(report, &["whole_population", "upper"]), "1", "8");

    for (index, member) in ["production-1", "fitted"].into_iter().enumerate() {
        std::fs::write(
            root.join("outcome.toml"),
            format!("[[outcome]]\nforecast = \"{member}\"\navailable_at = \"2030-01-02T01:00:01Z\"\nkind = \"published\"\nvalue = \"1\"\nsource = \"synthetic:mill-report\"\nvintage = \"invented-complete-processing-hour\"\nlocator = \"Synthetic!B7\"\nsha256 = \"{}\"\n", "a".repeat(64)),
        )
        .unwrap();
        let outcome = execute_with_clock(
            &Command::Outcome {
                journal: "history".into(),
                head: head(&root),
                request: "outcome.toml".into(),
            },
            &root,
            &Clock(end + 2 * NS_PER_SECOND + index as i64),
        );
        assert_eq!(outcome.exit_code, 0, "{outcome:?}");
    }
    let cut = end + 3 * NS_PER_SECOND;
    let selected = [
        saved(&root, "raw", cut, end + 4 * NS_PER_SECOND),
        saved(&root, "fitted", cut, end + 5 * NS_PER_SECOND),
    ];
    let before = std::fs::read(root.join("history")).unwrap();
    let pinned = head(&root);
    let old_report = compare(&root, pending[0], pending[1], PairedScore::Crps);
    let old = parse_json(old_report.payload_json.as_bytes()).unwrap();
    assert_eq!(field(&old, &["comparison"]), report);

    for path in [
        "production.toml",
        "grinding.toml",
        "recovery.toml",
        "fitted-recovery.toml",
    ] {
        std::fs::rename(root.join(path), root.join(format!("retired-{path}"))).unwrap();
    }
    for forecast in ["production-1", "fitted"] {
        let verified = execute_with_clock(
            &Command::VerifyPlant {
                journal: "history".into(),
                head: Some(pinned),
                forecast: forecast.into(),
            },
            &root,
            &NoClock,
        );
        assert_eq!(verified.exit_code, 0, "{verified:?}");
    }
    let compared = compare(&root, selected[0], selected[1], PairedScore::Crps);
    assert_eq!(compared.exit_code, 0, "{compared:?}");
    let json = parse_json(compared.payload_json.as_bytes()).unwrap();
    let report = field(&json, &["comparison"]);
    assert_eq!(
        field(report, &["schema"]).as_str(),
        Some("fmine.forecast-supplied-crps-comparison.v1")
    );
    assert_eq!(
        field(report, &["source_citations"]).as_str(),
        Some("supplied_not_verified")
    );
    assert_eq!(field(report, &["n_paired"]).u64(), Some(1));
    // At supplied y=1, raw CRPS=7/32 and fitted CRPS=3/32. Their exact difference is 1/8.
    fraction(field(report, &["observed_mean_difference"]), "1", "8");
    fraction(field(report, &["whole_population", "lower"]), "1", "8");
    fraction(field(report, &["whole_population", "upper"]), "1", "8");
    let pair = &field(report, &["members"]).as_array().unwrap()[0];
    assert_eq!(
        field(pair, &["target", "quantity"]).as_str(),
        Some("model_recovery_fraction:Au")
    );
    assert_eq!(field(pair, &["target", "unit"]).as_str(), Some("fraction"));
    assert_eq!(
        field(pair, &["left", "outcome", "citation"]),
        field(pair, &["right", "outcome", "citation"])
    );
    assert_ne!(
        field(pair, &["left", "selected_event", "digest"]),
        field(pair, &["right", "selected_event", "digest"])
    );
    assert!(compared.payload_json.contains("Synthetic!B7"));
    assert!(compared.payload_json.contains(&"a".repeat(64)));
    assert!(
        compared
            .rendered_markdown
            .as_ref()
            .unwrap()
            .contains("supplied, not verified")
    );
    let interval = compare(&root, selected[0], selected[1], PairedScore::Interval);
    assert_eq!(interval.exit_code, 0, "{interval:?}");
    let json = parse_json(interval.payload_json.as_bytes()).unwrap();
    fraction(
        field(&json, &["comparison", "observed_mean_difference"]),
        "1",
        "2",
    );

    let public = execute_with_clock(
        &Command::CompareCrps {
            journal: "history".into(),
            head: Some(pinned),
            left: selected[0],
            right: selected[1],
        },
        &root,
        &NoClock,
    );
    assert!(
        public.refusal.is_some(),
        "supplied comparison must be explicitly selected"
    );
    let again = compare(&root, selected[0], selected[1], PairedScore::Crps);
    assert_eq!(again.payload_json, compared.payload_json);
    assert_eq!(head(&root), pinned);
    assert_eq!(std::fs::read(root.join("history")).unwrap(), before);

    let args: Vec<_> = [
        "forecast",
        "journal",
        "compare",
        "--journal",
        "history",
        "--left",
        &selected[0].to_hex(),
        "--right",
        &selected[1].to_hex(),
        "--head",
        &pinned.to_hex(),
        "--targets",
        "supplied",
        "--score",
        "crps",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();
    let parsed = crate::parse_args(&args, &root).unwrap().0;
    assert_eq!(
        parsed,
        crate::CliCommand::ForecastJournal(Command::CompareSupplied {
            journal: "history".into(),
            head: Some(pinned),
            left: selected[0],
            right: selected[1],
            score: PairedScore::Crps,
        })
    );
    for extra in [vec!["--targets", "public"], vec!["--targets", "supplied"]] {
        let mut duplicate = args.clone();
        duplicate.extend(extra.into_iter().map(str::to_owned));
        assert!(crate::parse_args(&duplicate, &root).is_err());
    }
    let mut invalid = args.clone();
    let value = invalid.iter().position(|value| value == "supplied").unwrap();
    invalid[value] = "inferred".into();
    assert!(crate::parse_args(&invalid, &root).is_err());
}
