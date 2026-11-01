//! BD-341: the operator path retains the evidence and uncertainty of the descriptive report.

use std::path::{Path, PathBuf};

use fmine_cli::{CliCommand, OutputFormat, invoke, parse_args, run};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn args(values: &[&str]) -> Vec<String> {
    values.iter().map(ToString::to_string).collect()
}

#[test]
fn dated_track_record_requires_a_valid_date_and_unambiguous_source_selection() {
    let (command, format, _) = parse_args(
        &args(&[
            "track-record",
            "--issuer",
            "barrick",
            "--cohort-as-of",
            "2026-10-31",
            "--site",
            "north-mara",
            "--through",
            "2026Q2",
            "--json",
        ]),
        &root(),
    )
    .unwrap();
    assert_eq!(format, OutputFormat::Json);
    assert_eq!(
        command,
        CliCommand::TrackRecordCohortAsOf {
            issuer: "barrick".into(),
            site: Some("north-mara".into()),
            through: Some("2026Q2".into()),
            as_of: "2026-10-31".into(),
        }
    );

    for command in [
        vec!["track-record"],
        vec!["track-record", "extra", "--synthetic"],
        vec!["track-record", "--issuer", "barrick", "--cohort-as-of"],
        vec![
            "track-record",
            "--issuer",
            "unknown",
            "--cohort-as-of",
            "2026-10-31",
        ],
        vec![
            "track-record",
            "--issuer",
            "barrick",
            "--cohort-as-of",
            "2026-02-30",
        ],
        vec![
            "track-record",
            "--issuer",
            "barrick",
            "--cohort-as-of",
            "2026-10-31T00:00:00Z",
        ],
        vec!["track-record", "--synthetic", "--issuer", "barrick"],
        vec!["track-record", "--synthetic", "--site", "north-mara"],
        vec!["track-record", "--synthetic", "--through", "2026Q2"],
        vec![
            "track-record",
            "--synthetic",
            "--cohort-as-of",
            "2026-02-30",
        ],
        vec!["track-record", "--synthetic", "--selector", "latest-as-of"],
        vec![
            "track-record",
            "--issuer",
            "barrick",
            "--cohort-as-of",
            "2026-10-31",
            "--site",
            " ",
        ],
        vec![
            "track-record",
            "--issuer",
            "barrick",
            "--as-of",
            "2026-10-31",
            "--cohort-as-of",
            "2026-10-31",
        ],
        vec![
            "track-record",
            "--issuer",
            "barrick",
            "--cohort-as-of",
            "2026-10-31",
            "--price-subject",
            "barrick",
        ],
        vec![
            "track-record",
            "--synthetic",
            "--cohort-as-of",
            "2032-12-31",
            "--price-subject",
            "Company",
        ],
    ] {
        let (envelope, _) = invoke(&args(&command), &root());
        assert_eq!(envelope.verb, "track-record", "{command:?}");
        assert_eq!(envelope.exit_code, 2, "{command:?}");
        assert_eq!(envelope.status, "usage_error", "{command:?}");
    }
}

#[test]
fn track_record_synthetic_is_deterministic_cited_and_keeps_authority_flags_false() {
    let command = args(&[
        "track-record",
        "--synthetic",
        "--cohort-as-of",
        "2032-12-31",
    ]);
    let (envelope, _) = invoke(&command, &root());
    assert_eq!(envelope.exit_code, 0, "{}", envelope.to_json());
    assert_eq!(envelope.verb, "track-record");
    assert!(envelope.anchor_id.is_none());
    assert!(envelope.certificate.is_none());
    assert!(envelope.no_claim.is_some());
    let (json, status) = run(&command, &root());
    assert_eq!(status, 0);
    assert_eq!(run(&command, &root()), (json.clone(), 0));
    for flag in [
        "reserve_disclosure_claim",
        "execution_authorized",
        "geotechnical_signoff",
        "regulatory_compliance_claim",
    ] {
        assert_eq!(json.matches(&format!("\"{flag}\":false")).count(), 1);
        assert!(!json.contains(&format!("\"{flag}\":true")));
    }
    assert!(json.contains("synthetic"));
    assert!(json.contains("sha256"));
    assert!(json.contains("claim_id"));
    assert!(json.contains("digest"));
    for action in envelope.next_actions {
        let words = action
            .command
            .split_whitespace()
            .skip(1)
            .collect::<Vec<_>>();
        assert!(parse_args(&args(&words), &root()).is_ok(), "{action:?}");
    }
}

#[test]
fn track_record_markdown_exposes_comparisons_missing_members_and_citations() {
    let (markdown, status) = run(
        &args(&[
            "track-record",
            "--synthetic",
            "--cohort-as-of",
            "2032-12-31",
            "--format",
            "markdown",
        ]),
        &root(),
    );
    assert_eq!(status, 0, "{markdown}");
    assert!(markdown.starts_with("# Guidance track record"));
    assert!(markdown.contains("Synthetic"));
    assert!(markdown.contains("Source citations"));
    assert!(markdown.contains("SHA-256"));
    assert!(markdown.contains("Missing"));
    assert!(markdown.contains("Restatements"));
    let later = markdown
        .split("## Restatements and comparative appearances")
        .nth(1)
        .unwrap()
        .split("## Descriptive cohorts")
        .next()
        .unwrap();
    assert!(later.contains("production\\_koz"));
    assert!(later.contains("attributable:4/5"));
    assert!(!markdown.contains("```json"));
}

#[test]
fn robot_docs_advertise_track_record_and_its_evidence_boundary() {
    let (json, status) = run(&args(&["robot-docs"]), &root());
    assert_eq!(status, 0);
    assert!(json.contains("track-record --issuer <issuer> --cohort-as-of YYYY-MM-DD"));
    assert!(json.contains("track-record --issuer <issuer> [--site <site>] --as-of YYYY-MM-DD"));
    assert!(json.contains("track-record --synthetic"));
    assert!(json.contains("Every admitted guidance member remains in its denominator"));
}
