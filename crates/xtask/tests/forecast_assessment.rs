//! CLI transcript and provenance acceptance through the public interface.

#![allow(clippy::expect_used)]

use std::path::{Path, PathBuf};

use fmine_cli::{CliCommand, invoke, parse_args};

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("workspace root")
        .to_path_buf()
}

fn args(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_string()).collect()
}

#[test]
fn forecast_assessment_selectors_and_time_resolution_are_explicit() {
    let base = [
        "forecast",
        "assess",
        "--ledger",
        "tests/forecasts/cohort_synthetic.toml",
        "--cohort",
        "synthetic-demo",
        "--as-of",
        "2026-10-31T00:00:00Z",
    ];
    let (command, _, _) = parse_args(&args(&base), &workspace_root()).expect("assessment args");
    assert!(
        matches!(command, CliCommand::ForecastAssess { selector, .. } if selector == "first-published")
    );
    let mut latest = args(&base);
    latest.extend(args(&["--selector", "latest-as-of"]));
    let (command, _, _) = parse_args(&latest, &workspace_root()).expect("restated selection");
    assert!(
        matches!(command, CliCommand::ForecastAssess { selector, .. } if selector == "latest-as-of")
    );
    let mut guarded = latest;
    guarded.extend(args(&[
        "--provenance",
        "tests/forecasts/provenance_synthetic.toml",
    ]));
    let (command, _, _) = parse_args(&guarded, &workspace_root()).expect("provenance selector");
    assert!(
        matches!(command, CliCommand::ForecastAssess { provenance: Some(path), .. }
        if path == Path::new("tests/forecasts/provenance_synthetic.toml"))
    );
    for bad in [
        vec!["forecast"],
        vec!["forecast", "freeze"],
        vec!["forecast", "assess"],
        vec![
            "forecast",
            "assess",
            "--ledger",
            "cohort.toml",
            "--cohort",
            "demo",
            "--as-of",
            "2026-10-31",
        ],
        vec![
            "forecast",
            "assess",
            "--ledger",
            "cohort.toml",
            "--cohort",
            "demo",
            "--as-of",
            "2026-10-31T00:00:00Z",
            "--selector",
            "latest",
        ],
        vec!["forecast", "assess", "--synthetic"],
        vec!["forecast", "assess", "--issuer", "barrick"],
    ] {
        let (envelope, _) = invoke(&args(&bad), &workspace_root());
        assert_eq!(
            (envelope.verb.as_str(), envelope.exit_code),
            ("forecast assess", 2),
            "{bad:?}"
        );
    }
    let (envelope, _) = invoke(
        &args(&[
            "guidance",
            "completion",
            "--issuer",
            "barrick",
            "--site",
            "north-mara",
            "--through",
            "2026Q2",
            "--as-of",
            "2026-10-31T00:00:00Z",
        ]),
        &workspace_root(),
    );
    assert_eq!(
        envelope.exit_code, 2,
        "guidance keeps its calendar-date contract"
    );
}

#[test]
fn forecast_assessment_checks_all_supplied_provenance_without_issuing_a_certificate() {
    let selected = args(&[
        "forecast",
        "assess",
        "--ledger",
        "tests/forecasts/cohort_synthetic.toml",
        "--cohort",
        "synthetic-demo",
        "--as-of",
        "2026-10-31T00:00:00Z",
        "--provenance",
        "tests/forecasts/provenance_synthetic.toml",
    ]);
    let (env, _) = invoke(&selected, &workspace_root());
    assert_eq!(env.status, "ok", "{}", env.to_json());
    assert_eq!(env.exit_code, 0);
    assert!(env.anchor_id.is_none() && env.certificate.is_none());
    assert!(
        env.payload_json
            .contains("\"temporal_validation\":{\"status\":\"checked\"")
    );
    assert_eq!(env.payload_json.matches("\"cone_digest\"").count(), 3);
    assert!(
        env.payload_json
            .contains("\"n_members\":3,\"n_resolved\":2,\"n_unresolved\":1")
    );
    assert!(
        env.payload_json
            .contains("\"generator_replay\":\"not_performed\"")
    );
    assert_eq!(
        env.to_json(),
        invoke(&selected, &workspace_root()).0.to_json()
    );
    let (docs, _) = invoke(&args(&["robot-docs"]), &workspace_root());
    assert!(docs.payload_json.contains("[--provenance <path>]"));
    assert!(docs.payload_json.contains("fmine.forecast-provenance.v1"));
    for bad in [
        args(&["doctor", "--provenance", "unused.toml"]),
        args(&["forecast", "assess", "--provenance"]),
    ] {
        assert_eq!(invoke(&bad, &workspace_root()).0.exit_code, 2);
    }
}
