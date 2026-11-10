//! SYNTHETIC end-to-end inventory trajectories with independently reduced closure answers.
//! Determinism: DeterministicPerIsa

#![cfg(test)]
#![allow(clippy::expect_used)]

use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use fmine_cli::{Envelope, OutputFormat, invoke};
use fmine_core::hash::sha256;
use fmine_io::omf::json::{Json, parse};

const TEXT: &str = include_str!("../../../tests/plant/trajectory_survey.toml");
static NEXT: AtomicUsize = AtomicUsize::new(0);

fn scratch() -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "fmine-trajectory-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&path).expect("test directory");
    path
}
fn evaluate(text: &str) -> (Envelope, Json) {
    let root = scratch();
    std::fs::write(root.join("trajectory.toml"), text).expect("worksheet");
    let args = [
        "plant",
        "trajectory",
        "--input",
        "trajectory.toml",
        "--json",
    ]
    .map(str::to_owned);
    let (envelope, format) = invoke(&args, &root);
    assert_eq!(format, OutputFormat::Json);
    assert_eq!(
        invoke(&args, &root).0.to_json(),
        envelope.to_json(),
        "deterministic rerun"
    );
    let payload = parse(envelope.payload_json.as_bytes()).expect("valid payload JSON");
    (envelope, payload)
}
fn at<'a>(value: &'a Json, path: &[&str]) -> &'a Json {
    path.iter()
        .fold(value, |value, key| value.get(key).expect(key))
}
fn array(value: &Json) -> &[Json] {
    value.as_array().expect("array")
}
fn number(value: &Json) -> f64 {
    value.f64().expect("finite number")
}
fn close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() <= 2e-9 * expected.abs().max(1.0),
        "{actual} != {expected}"
    );
}
fn index(value: &Json, key: &str) -> usize {
    array(at(value, &["coordinate_order"]))
        .iter()
        .position(|v| at(v, &["key"]).as_str() == Some(key))
        .expect("coordinate")
}
fn estimate(value: &Json, key: &str) -> f64 {
    number(&array(at(value, &["result", "estimates_kg"]))[index(value, key)])
}
fn covariance(value: &Json, left: &str, right: &str) -> f64 {
    let n = array(at(value, &["coordinate_order"])).len();
    number(
        &array(at(value, &["result", "posterior_covariance_kg2"]))
            [index(value, left) * n + index(value, right)],
    )
}
fn changed(text: &str, old: &str, new: &str) -> String {
    assert!(text.contains(old), "mutation must target real bytes");
    text.replacen(old, new, 1)
}
fn refused(text: &str, code: &str) -> (Envelope, Json) {
    let (envelope, value) = evaluate(text);
    assert_ne!(envelope.exit_code, 0, "{}", envelope.to_json());
    assert_eq!(
        envelope.refusal.as_ref().expect("refusal").code,
        code,
        "{}",
        envelope.to_json()
    );
    assert_eq!(at(&value, &["submitted_worksheet"]).as_str(), Some(text));
    assert_eq!(at(&value, &["admitted_claim"]), &Json::Null);
    (envelope, value)
}

#[test]
fn actual_command_reports_shared_states_full_covariance_and_original_source_claims() {
    let (envelope, value) = evaluate(TEXT);
    assert_eq!(envelope.exit_code, 0, "{}", envelope.to_json());
    assert_eq!(envelope.status, "ok");
    assert!(envelope.certificate.is_none());
    assert!(
        envelope
            .no_claim
            .expect("conditional scope")
            .contains("uncalibrated")
    );
    assert_eq!(
        at(&value, &["schema"]).as_str(),
        Some("fmine.plant-trajectory-result.v1")
    );
    assert_eq!(at(&value, &["submitted_worksheet"]).as_str(), Some(TEXT));
    assert_eq!(
        at(&value, &["source", "sha256"]).as_str(),
        Some(sha256(TEXT.as_bytes()).to_hex().as_str())
    );
    assert_eq!(at(&value, &["measured_coordinate_count"]).i64(), Some(6));
    assert_eq!(array(at(&value, &["source_inputs"])).len(), 7);
    let stocks = array(at(&value, &["result", "stocks"]));
    assert_eq!(stocks.len(), 3);
    assert_eq!(at(&stocks[1], &["at_ns"]).as_str(), Some("10"));
    assert_eq!(at(&stocks[1], &["observed_kg"]), &Json::Null);
    assert_eq!(at(&stocks[1], &["source_claim_id"]), &Json::Null);
    assert_eq!(at(&stocks[1], &["unmeasured"]).as_bool(), Some(true));
    close(estimate(&value, "stock/0"), 65.0 / 6.0);
    close(estimate(&value, "stock/10"), 29.0 / 2.0);
    close(estimate(&value, "stock/20"), 115.0 / 6.0);
    close(estimate(&value, "inflow/0/10"), 35.0 / 6.0);
    close(estimate(&value, "outflow/10/20"), 19.0 / 6.0);
    close(covariance(&value, "stock/10", "stock/10"), 1.5);
    close(covariance(&value, "stock/10", "stock/20"), 0.5);
    close(covariance(&value, "stock/20", "stock/20"), 5.0 / 6.0);
    close(
        number(at(&value, &["result", "gross_error", "chi_square"])),
        25.0 / 6.0,
    );
    assert_eq!(
        at(&value, &["result", "gross_error", "degrees_of_freedom"]).i64(),
        Some(1)
    );
    assert_eq!(
        at(&value, &["admitted_claim", "evidence"]).as_str(),
        Some("Assumed")
    );
    assert_eq!(
        at(&value, &["admitted_claim", "confidentiality"]).as_str(),
        Some("Internal")
    );
    for row in array(at(&value, &["source_inputs"])) {
        assert_eq!(at(row, &["claim", "evidence"]).as_str(), Some("Assumed"));
        assert!(!array(at(row, &["claim", "sources"])).is_empty());
    }
    let all = parse(envelope.to_json().as_bytes()).expect("envelope");
    for (_, flag) in at(&all, &["flags"]).as_object().expect("flags") {
        assert_eq!(flag.as_bool(), Some(false));
    }
    let markdown = envelope.rendered_markdown.expect("markdown");
    assert!(markdown.contains("Shared boundary stocks"));
    assert!(markdown.contains("Unmeasured, inferred"));
}

#[test]
fn cross_period_error_correlation_changes_joint_posterior_without_disappearing() {
    let text = changed(
        TEXT,
        "left = \"inflow/0/10\"\nright = \"inflow/10/20\"\nkg2 = \"0\"",
        "left = \"inflow/0/10\"\nright = \"inflow/10/20\"\nkg2 = \"0.5\"",
    );
    let (envelope, value) = evaluate(&text);
    assert_eq!(envelope.exit_code, 0, "{}", envelope.to_json());
    close(estimate(&value, "stock/10"), 14.5);
    close(estimate(&value, "inflow/0/10"), 85.0 / 14.0);
    close(covariance(&value, "stock/10", "stock/10"), 5.0 / 4.0);
    close(covariance(&value, "stock/20", "stock/20"), 6.0 / 7.0);
    close(covariance(&value, "stock/10", "inflow/10/20"), -1.0 / 4.0);
    close(
        number(at(&value, &["result", "gross_error", "chi_square"])),
        25.0 / 7.0,
    );
}

fn without_terminal_survey() -> String {
    let mut blocks = TEXT.split("\n[[");
    let mut text = blocks.next().expect("header").to_owned();
    for block in blocks {
        if block.starts_with("stocktake]]") && block.contains("at_ns = \"20\"") {
            text.push_str("\n[[unmeasured_stocktake]]\nat_ns = \"20\"\n");
        } else if !(block.starts_with("covariance]]") && block.contains("\"stock/20\"")) {
            text.push_str("\n[[");
            text.push_str(block);
        }
    }
    text
}

#[test]
fn no_terminal_survey_infers_trajectory_without_fabricating_a_gross_error_pass() {
    let text = without_terminal_survey();
    let (envelope, before) = evaluate(&text);
    assert_eq!(envelope.exit_code, 0, "{}", envelope.to_json());
    close(estimate(&before, "stock/10"), 12.0);
    close(estimate(&before, "stock/20"), 15.0);
    close(covariance(&before, "stock/10", "stock/10"), 3.0);
    close(covariance(&before, "stock/20", "stock/20"), 5.0);
    close(covariance(&before, "stock/10", "stock/20"), 3.0);
    assert_eq!(
        at(&before, &["result", "gross_error", "status"]).as_str(),
        Some("no_redundancy")
    );
    assert_eq!(
        at(&before, &["result", "gross_error", "chi_square"]),
        &Json::Null
    );
    assert_eq!(
        at(&before, &["result", "gross_error", "degrees_of_freedom"]).i64(),
        Some(0)
    );
    let (_, after) = evaluate(TEXT);
    assert_ne!(
        at(&before, &["receipt_digest"]),
        at(&after, &["receipt_digest"])
    );
    assert_eq!(
        at(&before, &["submitted_worksheet"]).as_str(),
        Some(text.as_str())
    );
    assert_eq!(array(at(&before, &["source_inputs"])).len(), 6);
}

#[test]
fn gross_error_rejection_keeps_diagnostics_and_original_observations_without_admitted_claim() {
    let text = changed(
        TEXT,
        "global_chi_square_limit = \"10\"",
        "global_chi_square_limit = \"1\"",
    );
    let (envelope, value) = refused(&text, "FMINE-REF-101");
    assert_eq!(at(&value, &["admitted_claim"]), &Json::Null);
    assert_eq!(
        at(&value, &["refusal", "code"]).as_str(),
        Some("FMINE-REF-101")
    );
    assert!(at(&value, &["receipt_digest"]).as_str().is_some());
    assert_eq!(array(at(&value, &["source_inputs"])).len(), 7);
    assert_eq!(
        at(&value, &["submitted_worksheet"]).as_str(),
        Some(text.as_str())
    );
    close(estimate(&value, "stock/10"), 14.5);
    close(
        number(&array(at(&value, &["result", "observed_kg"]))[index(&value, "stock/20")]),
        20.0,
    );
    assert!(
        envelope
            .rendered_markdown
            .expect("investigation report")
            .contains("Rejected:")
    );
}

#[test]
fn closed_grammar_population_and_declared_covariance_refuse_ambiguous_inputs() {
    refused(
        &changed(TEXT, "end_ns = \"10\"", "end_ns = \"9\""),
        "FMINE-REF-102",
    );
    refused(
        &changed(TEXT, "at_ns = \"10\"", "at_ns = \"11\""),
        "FMINE-REF-102",
    );
    refused(
        &changed(TEXT, "inflow_kg = \"5\"", "inflow_kg = \"-5\""),
        "FMINE-REF-109",
    );
    refused(
        &changed(TEXT, "complete = \"true\"", "complete = \"false\""),
        "FMINE-REF-201",
    );
    refused(
        &changed(
            TEXT,
            "kg-total-element/all-phases/full-physical-boundary",
            "attributable-kg",
        ),
        "FMINE-REF-107",
    );
    refused(
        &format!("{TEXT}\n[[unmeasured_stocktake]]\nat_ns = \"0\"\n"),
        "FMINE-REF-105",
    );
    refused(
        &format!("{TEXT}\n[[stocktake]]\nat_ns = \"0\"\nkg = \"10\"\n"),
        "FMINE-REF-105",
    );
    refused(
        &changed(TEXT, "left = \"stock/0\"", "left = \"stock/10\""),
        "FMINE-REF-506",
    );
    refused(
        &format!("{TEXT}\n[[covariance]]\nleft = \"stock/0\"\nright = \"stock/0\"\nkg2 = \"1\"\n"),
        "FMINE-REF-506",
    );
    refused(
        &changed(TEXT, "kg2 = \"1\"", "kg2 = \"NaN\""),
        "FMINE-REF-506",
    );
    refused(
        &changed(TEXT, "kg2 = \"1\"", "kg2 = \"0\""),
        "FMINE-REF-109",
    );
    refused(
        &changed(TEXT, "kg2 = \"1\"", "kg2 = \"1e-9999\""),
        "FMINE-REF-207",
    );
    let missing_pair = changed(
        TEXT,
        "[[covariance]]\nleft = \"stock/0\"\nright = \"stock/20\"\nkg2 = \"0\"",
        "# removed declared cross covariance",
    );
    refused(&missing_pair, "FMINE-REF-506");
    refused(
        &changed(
            TEXT,
            "inflow_kg = \"5\"",
            "inflow_kg = \"5\"\nunknown_field = \"ignored?\"",
        ),
        "FMINE-REF-506",
    );
}

#[test]
fn unanchored_offset_refuses_and_command_docs_name_actual_schema() {
    let text = without_terminal_survey();
    let text = changed(
        &text,
        "[[stocktake]]\nat_ns = \"0\"\nkg = \"10\"",
        "[[unmeasured_stocktake]]\nat_ns = \"0\"",
    );
    refused(&text, "FMINE-REF-104");
    let root = scratch();
    let docs = invoke(&["robot-docs".into()], &root).0.to_json();
    assert!(docs.contains("fmine.plant-trajectory.v1"));
    assert!(docs.contains("plant trajectory --input"));
    let (bad, _) = invoke(
        &[
            "plant".into(),
            "trajectory".into(),
            "--input".into(),
            "missing.toml".into(),
        ],
        &root,
    );
    assert_ne!(bad.exit_code, 0);
    assert_eq!(bad.refusal.expect("missing source").code, "FMINE-REF-506");
}
