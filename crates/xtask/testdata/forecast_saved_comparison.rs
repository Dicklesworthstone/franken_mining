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
