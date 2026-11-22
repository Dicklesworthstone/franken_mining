//! Synthetic source-bound saved comparisons exercise the durable read-only command.

use super::*;
use fmine_io::omf::json::parse;

#[test]
fn saved_comparison_preserves_publication_evidence_without_clock_source_io_or_append() {
    let mut fixture = Fixture::annual("100%");
    let register = fixture.request(
        "wider.toml",
        "[[forecast]]\nid = \"two\"\nmodel_key = \"invented-wider-target\"\noutcome_due = \"2030-12-31T23:59:59Z\"\nkind = \"interval\"\nlo = \"300\"\nhi = \"500\"\nnominal = \"0.8\"\n",
    );
    fixture.ok(
        Command::Register {
            journal: JOURNAL.into(),
            head: fixture.head,
            request: register,
        },
        ns("2030-01-04T00:00:00Z"),
    );
    let freeze = fixture.request(
        "wider-cohort.toml",
        &format!(
            "[[cohort]]\nname = \"wider\"\nmembers = \"two\"\nresolution_rule = \"registered-public-cell\"\neligible_sources = \"{ISSUER}\"\npublic_issuer = \"{ISSUER}\"\npublic_metric = \"production_koz\"\npublic_period = \"2030FY\"\npublic_ownership = \"100%\"\npublic_members = \"two=Site A\"\n"
        ),
    );
    fixture.ok(
        Command::Freeze {
            journal: JOURNAL.into(),
            head: fixture.head,
            request: freeze,
        },
        ns("2030-01-05T00:00:00Z"),
    );
    fixture.ok(fixture.resolve(), ns(AVAILABLE));
    fixture.ok(
        Command::Resolve {
            journal: JOURNAL.into(),
            head: fixture.head,
            cohort: "wider".into(),
            forecast: "two".into(),
        },
        ns(AVAILABLE) + 1,
    );
    let mut saved = Vec::new();
    for (index, cohort) in [COHORT, "wider"].into_iter().enumerate() {
        let report = fixture.ok(
            Command::Assess {
                journal: JOURNAL.into(),
                head: fixture.head,
                cohort: cohort.into(),
                as_of: CUT.into(),
                selector: "first-published".into(),
                expected_digest: None,
            },
            ns(CUT) + index as i64,
        );
        saved.push((
            digest_field(assessment(&report.payload_json), "digest"),
            assessment(&report.payload_json).to_string(),
        ));
    }
    let command = Command::Compare {
        journal: JOURNAL.into(),
        head: Some(fixture.head),
        left: saved[0].0,
        right: saved[1].0,
    };
    let before = fixture.bytes();
    let compared = execute_with_clock(&command, &fixture.root, &NeverClock);
    assert_eq!(compared.exit_code, 0, "{compared:?}");
    assert_eq!(fixture.bytes(), before);
    let json = parse(compared.payload_json.as_bytes()).unwrap();
    let comparison = json.get("comparison").unwrap();
    assert_eq!(comparison.get("n_members").unwrap().f64(), Some(1.0));
    assert_eq!(comparison.get("n_paired").unwrap().f64(), Some(1.0));
    assert_eq!(comparison.get("n_unmatched").unwrap().f64(), Some(0.0));
    let mean = comparison.get("observed_mean_difference").unwrap();
    // Both intervals cover printed 400. Their losses are their widths: 100 - 200 = -100.
    assert_eq!(mean.get("num").unwrap().as_str(), Some("-100"));
    assert_eq!(mean.get("den").unwrap().as_str(), Some("1"));
    let population = comparison.get("whole_population").unwrap();
    assert_eq!(population.get("lower").unwrap(), mean);
    assert_eq!(population.get("upper").unwrap(), mean);
    assert!(compared.payload_json.contains("2030Q4.xlsx:YTD!B5"));
    assert!(
        compared
            .payload_json
            .contains(&fixture.source_digest.to_hex())
    );
    let markdown = compared.rendered_markdown.as_ref().unwrap();
    assert!(markdown.contains("Observed mean difference: -100"));
    assert!(markdown.contains("Left Site A: forecast"));
    assert!(markdown.contains("Right Site A: forecast"));

    // A saved comparison is a projection of admitted journal evidence, with no workbook reread.
    fs::write(
        fixture.root.join("corpus/2030Q4.xlsx"),
        b"unavailable source",
    )
    .unwrap();
    assert_eq!(
        execute_with_clock(&command, &fixture.root, &NeverClock).to_json(),
        compared.to_json()
    );
    for (digest, expected) in &saved {
        let report = execute_with_clock(
            &Command::Report {
                journal: JOURNAL.into(),
                head: Some(fixture.head),
                assessment: *digest,
                provenance: None,
                timestamp: None,
            },
            &fixture.root,
            &NeverClock,
        );
        assert_eq!(report.exit_code, 0, "{report:?}");
        assert_eq!(assessment(&report.payload_json), expected);
    }
    assert_eq!(fixture.bytes(), before);

    let args: Vec<_> = [
        "forecast",
        "journal",
        "compare",
        "--journal",
        JOURNAL,
        "--left",
        &saved[0].0.to_hex(),
        "--right",
        &saved[1].0.to_hex(),
        "--head",
        &fixture.head.to_hex(),
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();
    let (parsed, _, root) = parse_args(&args, &fixture.root).unwrap();
    assert_eq!(root, fixture.root);
    assert_eq!(parsed, CliCommand::ForecastJournal(command));
    for extra in [
        vec!["--trust-host-clock"],
        vec!["--selector", "latest-as-of"],
        vec!["--request", "new.toml"],
    ] {
        let mut invalid = args.clone();
        invalid.extend(extra.into_iter().map(str::to_owned));
        assert!(parse_args(&invalid, &fixture.root).is_err());
    }
    let unknown = execute_with_clock(
        &Command::Compare {
            journal: JOURNAL.into(),
            head: Some(fixture.head),
            left: saved[0].0,
            right: Digest32::default(),
        },
        &fixture.root,
        &NeverClock,
    );
    assert_ne!(unknown.exit_code, 0);
    assert_eq!(fixture.bytes(), before);
}

#[test]
fn saved_crps_comparison_preserves_exact_weights_sources_and_read_only_cli_semantics() {
    let mut fixture = Fixture::annual("100%");
    for (index, (id, kind, weights)) in [
        (
            "finite-left",
            "weighted_empirical",
            "weights = \"1/4,3/4\"\n",
        ),
        ("finite-right", "empirical", ""),
    ]
    .into_iter()
    .enumerate()
    {
        let request = fixture.request(
            &format!("{id}.toml"),
            &format!("[[forecast]]\nid = \"{id}\"\nmodel_key = \"{id}\"\noutcome_due = \"2030-12-31T23:59:59Z\"\nkind = \"{kind}\"\nsamples = \"300,500\"\n{weights}"),
        );
        fixture.ok(
            Command::Register {
                journal: JOURNAL.into(),
                head: fixture.head,
                request,
            },
            ns("2030-01-04T00:00:00Z") + index as i64,
        );
    }
    for (index, id) in ["finite-left", "finite-right"].into_iter().enumerate() {
        let request = fixture.request(
            &format!("{id}-cohort.toml"),
            &format!("[[cohort]]\nname = \"{id}\"\nmembers = \"{id}\"\nresolution_rule = \"registered-public-cell\"\neligible_sources = \"{ISSUER}\"\npublic_issuer = \"{ISSUER}\"\npublic_metric = \"production_koz\"\npublic_period = \"2030FY\"\npublic_ownership = \"100%\"\npublic_members = \"{id}=Site A\"\n"),
        );
        fixture.ok(
            Command::Freeze {
                journal: JOURNAL.into(),
                head: fixture.head,
                request,
            },
            ns("2030-01-05T00:00:00Z") + index as i64,
        );
    }
    let mut pending = Vec::new();
    for (index, cohort) in ["finite-left", "finite-right"].into_iter().enumerate() {
        let report = fixture.ok(
            Command::Assess {
                journal: JOURNAL.into(),
                head: fixture.head,
                cohort: cohort.into(),
                as_of: "2031-01-01T00:00:00Z".into(),
                selector: "first-published".into(),
                expected_digest: None,
            },
            ns("2031-01-01T00:00:00Z") + index as i64,
        );
        pending.push(digest_field(assessment(&report.payload_json), "digest"));
    }
    let pending_report = execute_with_clock(
        &Command::CompareCrps {
            journal: JOURNAL.into(),
            head: Some(fixture.head),
            left: pending[0],
            right: pending[1],
        },
        &fixture.root,
        &NeverClock,
    );
    assert_eq!(pending_report.exit_code, 0, "{pending_report:?}");
    let parsed_pending = parse(pending_report.payload_json.as_bytes()).unwrap();
    let pending_comparison = parsed_pending.get("comparison").unwrap();
    assert_eq!(
        pending_comparison.get("n_unresolved").unwrap().f64(),
        Some(1.0)
    );
    let bounds = pending_comparison.get("whole_population").unwrap();
    assert_eq!(
        bounds.get("lower").unwrap().get("num").unwrap().as_str(),
        Some("-75")
    );
    assert_eq!(
        bounds.get("lower").unwrap().get("den").unwrap().as_str(),
        Some("2")
    );
    assert_eq!(
        bounds.get("upper").unwrap().get("num").unwrap().as_str(),
        Some("125")
    );
    assert_eq!(
        bounds.get("upper").unwrap().get("den").unwrap().as_str(),
        Some("2")
    );
    for (index, id) in ["finite-left", "finite-right"].into_iter().enumerate() {
        fixture.ok(
            Command::Resolve {
                journal: JOURNAL.into(),
                head: fixture.head,
                cohort: id.into(),
                forecast: id.into(),
            },
            ns(AVAILABLE) + index as i64,
        );
    }
    let mut saved = Vec::new();
    for (index, cohort) in ["finite-left", "finite-right", COHORT]
        .into_iter()
        .enumerate()
    {
        let report = fixture.ok(
            Command::Assess {
                journal: JOURNAL.into(),
                head: fixture.head,
                cohort: cohort.into(),
                as_of: CUT.into(),
                selector: "first-published".into(),
                expected_digest: None,
            },
            ns(CUT) + index as i64,
        );
        saved.push(digest_field(assessment(&report.payload_json), "digest"));
    }
    let before = fixture.bytes();
    let command = Command::CompareCrps {
        journal: JOURNAL.into(),
        head: Some(fixture.head),
        left: saved[0],
        right: saved[1],
    };
    let compared = execute_with_clock(&command, &fixture.root, &NeverClock);
    assert_eq!(compared.exit_code, 0, "{compared:?}");
    let json = parse(compared.payload_json.as_bytes()).unwrap();
    let comparison = json.get("comparison").unwrap();
    assert_eq!(
        comparison.get("schema").unwrap().as_str(),
        Some("fmine.forecast-crps-comparison.v1")
    );
    assert_eq!(comparison.get("n_paired").unwrap().f64(), Some(1.0));
    let mean = comparison.get("observed_mean_difference").unwrap();
    // At source-printed 400: weighted CRPS=62.5, uniform CRPS=50, difference=12.5.
    assert_eq!(mean.get("num").unwrap().as_str(), Some("25"));
    assert_eq!(mean.get("den").unwrap().as_str(), Some("2"));
    assert_eq!(
        comparison
            .get("whole_population")
            .unwrap()
            .get("lower")
            .unwrap(),
        mean
    );
    assert_eq!(
        comparison
            .get("whole_population")
            .unwrap()
            .get("upper")
            .unwrap(),
        mean
    );
    assert!(
        compared
            .payload_json
            .contains("\"weights\":[\"1/4\",\"3/4\"]")
    );
    assert!(compared.payload_json.contains("2030Q4.xlsx:YTD!B5"));
    assert!(
        compared
            .payload_json
            .contains(&fixture.source_digest.to_hex())
    );
    let md = compared.rendered_markdown.as_ref().unwrap();
    assert!(md.contains("Paired full-distribution CRPS assessment"));
    assert!(md.contains("Observed mean difference: 25/2"));
    assert!(md.contains("| 300 | 1/4 |"));
    assert!(md.contains("| 500 | 3/4 |"));
    assert!(md.contains("Left Site A: forecast") && md.contains("Right Site A: forecast"));

    let old = execute_with_clock(
        &Command::CompareCrps {
            journal: JOURNAL.into(),
            head: Some(fixture.head),
            left: pending[0],
            right: pending[1],
        },
        &fixture.root,
        &NeverClock,
    );
    assert_eq!(
        parse(old.payload_json.as_bytes())
            .unwrap()
            .get("comparison")
            .unwrap(),
        pending_comparison
    );
    let interval = execute_with_clock(
        &Command::Compare {
            journal: JOURNAL.into(),
            head: Some(fixture.head),
            left: saved[0],
            right: saved[1],
        },
        &fixture.root,
        &NeverClock,
    );
    assert_eq!(interval.exit_code, 0);
    assert!(
        interval
            .payload_json
            .contains("\"schema\":\"fmine.forecast-comparison.v1\"")
    );
    assert!(
        interval
            .payload_json
            .contains("\"observed_mean_difference\":{\"num\":\"0\",\"den\":\"1\"}")
    );

    let args: Vec<_> = [
        "forecast",
        "journal",
        "compare",
        "--journal",
        JOURNAL,
        "--left",
        &saved[0].to_hex(),
        "--right",
        &saved[1].to_hex(),
        "--head",
        &fixture.head.to_hex(),
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();
    let mut crps_args = args.clone();
    crps_args.extend(["--score".into(), "crps".into()]);
    assert_eq!(
        parse_args(&crps_args, &fixture.root).unwrap().0,
        CliCommand::ForecastJournal(command.clone())
    );
    let mut explicit_default = args.clone();
    explicit_default.extend(["--score".into(), "interval".into()]);
    assert_eq!(
        parse_args(&args, &fixture.root).unwrap().0,
        parse_args(&explicit_default, &fixture.root).unwrap().0
    );
    for extra in [
        vec!["--score", "log"],
        vec!["--trust-host-clock"],
        vec!["--as-of", CUT],
        vec!["--score", "crps", "--score", "interval"],
    ] {
        let mut invalid = args.clone();
        invalid.extend(extra.into_iter().map(str::to_owned));
        assert!(parse_args(&invalid, &fixture.root).is_err());
    }
    let unsupported = execute_with_clock(
        &Command::CompareCrps {
            journal: JOURNAL.into(),
            head: Some(fixture.head),
            left: saved[0],
            right: saved[2],
        },
        &fixture.root,
        &NeverClock,
    );
    assert_ne!(unsupported.exit_code, 0);
    assert!(unsupported.refusal.unwrap().what.contains("supports only"));
    fs::write(
        fixture.root.join("corpus/2030Q4.xlsx"),
        b"unavailable source",
    )
    .unwrap();
    assert_eq!(
        execute_with_clock(&command, &fixture.root, &NeverClock).to_json(),
        compared.to_json()
    );
    assert_eq!(fixture.bytes(), before);
}

#[test]
fn saved_gaussian_crps_comparison_keeps_pending_bounds_and_source_bound_read_only_replay() {
    let mut fixture = Fixture::with_payload(
        "production_koz",
        "100%",
        "2030FY",
        "kind = \"gaussian\"\nmean = \"400\"\nstd_dev = \"20\"",
        None,
    );
    let request = fixture.request(
        "gaussian-right.toml",
        "[[forecast]]\nid = \"two\"\nmodel_key = \"invented-narrower-gaussian\"\noutcome_due = \"2030-12-31T23:59:59Z\"\nkind = \"gaussian\"\nmean = \"400\"\nstd_dev = \"10\"\n",
    );
    fixture.ok(
        Command::Register {
            journal: JOURNAL.into(),
            head: fixture.head,
            request,
        },
        ns("2030-01-04T00:00:00Z"),
    );
    let request = fixture.request(
        "gaussian-right-cohort.toml",
        &format!("[[cohort]]\nname = \"gaussian-right\"\nmembers = \"two\"\nresolution_rule = \"registered-public-cell\"\neligible_sources = \"{ISSUER}\"\npublic_issuer = \"{ISSUER}\"\npublic_metric = \"production_koz\"\npublic_period = \"2030FY\"\npublic_ownership = \"100%\"\npublic_members = \"two=Site A\"\n"),
    );
    fixture.ok(
        Command::Freeze {
            journal: JOURNAL.into(),
            head: fixture.head,
            request,
        },
        ns("2030-01-05T00:00:00Z"),
    );
    let mut pending = Vec::new();
    for (index, cohort) in [COHORT, "gaussian-right"].into_iter().enumerate() {
        let report = fixture.ok(
            Command::Assess {
                journal: JOURNAL.into(),
                head: fixture.head,
                cohort: cohort.into(),
                as_of: "2031-01-01T00:00:00Z".into(),
                selector: "first-published".into(),
                expected_digest: None,
            },
            ns("2031-01-01T00:00:00Z") + index as i64,
        );
        pending.push(digest_field(assessment(&report.payload_json), "digest"));
    }
    let compare = |head, left, right| Command::CompareCrps {
        journal: JOURNAL.into(),
        head: Some(head),
        left,
        right,
    };
    let pending_bytes = fixture.bytes();
    let pending_report = execute_with_clock(
        &compare(fixture.head, pending[0], pending[1]),
        &fixture.root,
        &NeverClock,
    );
    assert_eq!(pending_report.exit_code, 0, "{pending_report:?}");
    assert_eq!(fixture.bytes(), pending_bytes);
    let pending_json = parse(pending_report.payload_json.as_bytes()).unwrap();
    let pending_comparison = pending_json.get("comparison").unwrap();
    assert_eq!(pending_comparison.get("n_paired").unwrap().f64(), Some(0.0));
    assert_eq!(
        pending_comparison.get("n_unresolved").unwrap().f64(),
        Some(1.0)
    );
    assert_eq!(
        pending_comparison
            .get("observed_mean_difference")
            .unwrap()
            .f64(),
        None
    );
    let completion = pending_comparison.get("whole_population").unwrap();
    // DESIGN: equal means, sigma difference 10. The global extrema are -10/sqrt(pi)
    // in the tails and 10*(sqrt(2)-1)/sqrt(pi) at the common mean; no candidate oracle.
    let lower = -10.0 / std::f64::consts::PI.sqrt();
    let upper = 10.0 * (2.0_f64.sqrt() - 1.0) / std::f64::consts::PI.sqrt();
    assert!((completion.get("lower").unwrap().f64().unwrap() - lower).abs() < 1e-12);
    assert!((completion.get("upper").unwrap().f64().unwrap() - upper).abs() < 1e-12);

    fixture.ok(fixture.resolve(), ns(AVAILABLE));
    fixture.ok(
        Command::Resolve {
            journal: JOURNAL.into(),
            head: fixture.head,
            cohort: "gaussian-right".into(),
            forecast: "two".into(),
        },
        ns(AVAILABLE) + 1,
    );
    let mut saved = Vec::new();
    for (index, cohort) in [COHORT, "gaussian-right"].into_iter().enumerate() {
        let report = fixture.ok(
            Command::Assess {
                journal: JOURNAL.into(),
                head: fixture.head,
                cohort: cohort.into(),
                as_of: CUT.into(),
                selector: "first-published".into(),
                expected_digest: None,
            },
            ns(CUT) + index as i64,
        );
        saved.push(digest_field(assessment(&report.payload_json), "digest"));
    }
    let before = fixture.bytes();
    let command = compare(fixture.head, saved[0], saved[1]);
    let compared = execute_with_clock(&command, &fixture.root, &NeverClock);
    assert_eq!(compared.exit_code, 0, "{compared:?}");
    let json = parse(compared.payload_json.as_bytes()).unwrap();
    let comparison = json.get("comparison").unwrap();
    assert_eq!(
        comparison.get("schema").unwrap().as_str(),
        Some("fmine.forecast-gaussian-crps-comparison.v1")
    );
    assert_eq!(
        comparison.get("arithmetic").unwrap().as_str(),
        Some("approximate")
    );
    assert_eq!(comparison.get("n_paired").unwrap().f64(), Some(1.0));
    assert_eq!(comparison.get("n_unresolved").unwrap().f64(), Some(0.0));
    let mean = comparison
        .get("observed_mean_difference")
        .unwrap()
        .f64()
        .unwrap();
    assert!((mean - upper).abs() < 1e-12);
    let population = comparison.get("whole_population").unwrap();
    assert_eq!(population.get("lower").unwrap().f64(), Some(mean));
    assert_eq!(population.get("upper").unwrap().f64(), Some(mean));
    assert!(compared.payload_json.contains("2030Q4.xlsx:YTD!B5"));
    assert!(
        compared
            .payload_json
            .contains(&fixture.source_digest.to_hex())
    );
    assert!(
        compared
            .rendered_markdown
            .as_ref()
            .unwrap()
            .contains("approximate")
    );

    // The workbook can become unusable without changing a saved comparison or sampling its clock.
    fs::write(
        fixture.root.join("corpus/2030Q4.xlsx"),
        b"unavailable source",
    )
    .unwrap();
    assert_eq!(
        execute_with_clock(&command, &fixture.root, &NeverClock).to_json(),
        compared.to_json()
    );
    let pending_again = execute_with_clock(
        &compare(fixture.head, pending[0], pending[1]),
        &fixture.root,
        &NeverClock,
    );
    assert_eq!(
        parse(pending_again.payload_json.as_bytes())
            .unwrap()
            .get("comparison")
            .unwrap(),
        pending_comparison
    );
    let reverse = execute_with_clock(
        &compare(fixture.head, saved[1], saved[0]),
        &fixture.root,
        &NeverClock,
    );
    assert_eq!(reverse.exit_code, 0, "{reverse:?}");
    let reverse_json = parse(reverse.payload_json.as_bytes()).unwrap();
    let reverse_mean = reverse_json
        .get("comparison")
        .unwrap()
        .get("observed_mean_difference")
        .unwrap()
        .f64()
        .unwrap();
    assert!((reverse_mean + mean).abs() < 1e-12);

    let args: Vec<_> = [
        "forecast",
        "journal",
        "compare",
        "--journal",
        JOURNAL,
        "--left",
        &saved[0].to_hex(),
        "--right",
        &saved[1].to_hex(),
        "--head",
        &fixture.head.to_hex(),
        "--score",
        "crps",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();
    let (output, exit) = fmine_cli::run(&args, &fixture.root);
    assert_eq!(exit, 0, "{output}");
    assert_eq!(
        parse(output.as_bytes())
            .unwrap()
            .get("payload")
            .unwrap()
            .get("comparison")
            .unwrap(),
        comparison
    );
    assert_eq!(
        fixture.bytes(),
        before,
        "comparison must preserve the exact journal and retained head"
    );
    DurableJournal::open_read(&fixture.root.join(JOURNAL), Some(fixture.head)).unwrap();
}
