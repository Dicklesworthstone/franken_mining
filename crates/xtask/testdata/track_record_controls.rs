use super::*;
use crate::{OutputFormat, invoke, run};
use std::path::PathBuf;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("workspace root")
        .to_path_buf()
}

fn args(words: &[&str]) -> Vec<String> {
    words.iter().map(|s| (*s).to_string()).collect()
}

#[test]
fn synthetic_command_reports_cited_scores_missing_members_and_false_authority() {
    let query = args(&["track-record", "--synthetic", "--json"]);
    let (envelope, format) = invoke(&query, &root());
    assert_eq!(format, OutputFormat::Json);
    assert_eq!(
        (envelope.status, envelope.exit_code),
        ("ok", 0),
        "{}",
        envelope.to_json()
    );
    assert_eq!(envelope.verb, "track-record");
    assert_eq!(envelope.no_claim, Some(NO_CLAIM));
    assert!(envelope.anchor_id.is_none() && envelope.certificate.is_none());
    let payload = &envelope.payload_json;
    assert!(payload.contains("\"schema\":\"fmine.guidance.track-record.v1\""));
    assert!(payload.contains("\"lane\":\"synthetic\""));
    assert!(payload.contains("\"z\":{\"num\":\"3\",\"den\":\"2\"}"));
    assert!(payload.contains("\"n_unresolved\":1"));
    assert!(payload.contains("\"state\":\"unresolved\""));
    assert!(payload.contains("\"sha256\":") && payload.contains("\"in\":["));
    assert!(payload.contains("\"calibration_verdict\":null"));
    assert!(payload.contains("no affiliation implied"));
    assert!(envelope.to_json().contains(crate::FLAGS_JSON));
    assert_eq!(run(&query, &root()), run(&query, &root()));
    assert_eq!(run(&query, &root()).0.lines().count(), 1);
    let digest = data::synthetic().expect("synthetic report").digest.to_hex();
    assert!(payload.contains(&digest));
}

#[test]
fn markdown_keeps_citations_uncertainty_and_literal_source_text() {
    let mut report = data::synthetic().expect("synthetic report");
    report.issuer = "<script>|[link](bad)\n# forged".into();
    let rendered = render(&report);
    assert!(rendered.contains("Guidance and annual outcomes"));
    assert!(rendered.contains("Whole-cohort hit-share completion bounds"));
    assert!(rendered.contains("Unresolved"));
    assert!(rendered.contains("1.5"));
    assert!(rendered.contains("Source citations"));
    assert!(rendered.contains("Canonical Claim links"));
    assert!(rendered.contains(&report.rows[0].member.guidance.claim_id.to_string()));
    assert!(rendered.contains(&report.rows[0].guidance.doc_sha256));
    assert!(!rendered.contains("<script>"));
    assert!(!rendered.contains("\n# forged"));
    assert!(rendered.contains("&lt;script&gt;\\|\\[link\\](bad) # forged"));
    assert_eq!(rendered, render(&report));
    let (printed, code) = run(
        &args(&["track-record", "--synthetic", "--format", "markdown"]),
        &root(),
    );
    assert_eq!(code, 0);
    assert!(printed.starts_with("# Guidance track record:"));
}
