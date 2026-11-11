//! SYNTHETIC command checks with a separately derived one-tank inverse.
//! Determinism: DeterministicPerIsa

#![cfg(test)]
#![allow(clippy::expect_used)]

use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use fmine_cli::{Envelope, OutputFormat, invoke};
use fmine_core::hash::sha256;
use fmine_io::omf::json::{Json, parse};

const TEXT: &str = include_str!("../../../tests/plant/rtd_inverse.toml");
static NEXT: AtomicUsize = AtomicUsize::new(0);

fn scratch() -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "fmine-rtd-inverse-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed),
    ));
    std::fs::create_dir_all(&path).expect("test directory");
    path
}
fn evaluate(text: &str) -> (Envelope, Json) {
    let root = scratch();
    std::fs::write(root.join("inverse.toml"), text).expect("worksheet");
    let args = ["plant", "rtd-infer", "--input", "inverse.toml", "--json"].map(str::to_owned);
    let (envelope, format) = invoke(&args, &root);
    assert_eq!(format, OutputFormat::Json);
    assert_eq!(
        invoke(&args, &root).0.to_json(),
        envelope.to_json(),
        "deterministic rerun"
    );
    let value = parse(envelope.payload_json.as_bytes()).expect("valid result JSON");
    (envelope, value)
}
fn at<'a>(value: &'a Json, path: &[&str]) -> &'a Json {
    path.iter().fold(value, |v, key| v.get(key).expect(key))
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
        .position(|row| at(row, &["key"]).as_str() == Some(key))
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
fn command_recovers_missing_feeds_with_analytic_joint_covariance_and_no_redundancy() {
    let (envelope, value) = evaluate(TEXT);
    assert_eq!(envelope.exit_code, 0, "{}", envelope.to_json());
    assert_eq!(
        at(&value, &["schema"]).as_str(),
        Some("fmine.plant-rtd-inverse-result.v1")
    );
    assert_eq!(
        at(&value, &["source", "sha256"]).as_str(),
        Some(sha256(TEXT.as_bytes()).to_hex().as_str())
    );
    assert_eq!(at(&value, &["submitted_worksheet"]).as_str(), Some(TEXT));
    assert_eq!(at(&value, &["measured_coordinate_count"]).i64(), Some(2));
    assert_eq!(array(at(&value, &["coordinate_order"])).len(), 4);
    close(estimate(&value, "feed/0/1000000000"), 2.0);
    close(estimate(&value, "feed/1000000000/2000000000"), 4.0);
    let h = (-1.0_f64).exp();
    let k = (1.0 - h).powi(2);
    let response = array(at(&value, &["result", "response_matrix"]));
    for (actual, expected) in response.iter().zip([h, 0.0, k, h]) {
        close(number(actual), expected);
    }
    // H^-1 diag(.01,.04) H^-T: an independently reduced triangular solve.
    close(
        covariance(&value, "feed/0/1000000000", "feed/0/1000000000"),
        0.01 / h.powi(2),
    );
    close(
        covariance(&value, "feed/0/1000000000", "feed/1000000000/2000000000"),
        -0.01 * k / h.powi(3),
    );
    close(
        covariance(
            &value,
            "feed/1000000000/2000000000",
            "feed/1000000000/2000000000",
        ),
        0.01 * k.powi(2) / h.powi(4) + 0.04 / h.powi(2),
    );
    let intervals = array(at(&value, &["result", "intervals"]));
    close(number(at(&intervals[0], &["opening_outlet_kg"])), 1.0 - h);
    close(
        number(at(&intervals[1], &["opening_outlet_kg"])),
        h * (1.0 - h),
    );
    close(
        number(at(&intervals[1], &["closing_inventory_kg"])),
        4.0 - 2.0 * h - h * h,
    );
    for row in intervals {
        close(
            number(at(row, &["fitted_outlet_kg"])),
            number(at(row, &["predicted_outlet_kg"])),
        );
        close(number(at(row, &["conservation_residual_kg"])), 0.0);
    }
    assert_eq!(
        at(&value, &["result", "gross_error", "status"]).as_str(),
        Some("no_redundancy")
    );
    assert_eq!(
        at(&value, &["result", "gross_error", "chi_square"]),
        &Json::Null
    );
    assert_eq!(
        at(&value, &["admitted_claim", "evidence"]).as_str(),
        Some("Assumed")
    );
    assert_eq!(
        at(&value, &["admitted_claim", "confidentiality"]).as_str(),
        Some("Internal")
    );
    for c in array(at(&value, &["source_inputs"])) {
        assert_eq!(at(c, &["feed_kg_exact"]), &Json::Null);
        assert!(!array(at(c, &["claim", "sources"])).is_empty());
    }
    let full = parse(envelope.to_json().as_bytes()).expect("envelope");
    for (_, flag) in at(&full, &["flags"]).as_object().expect("flags") {
        assert_eq!(flag.as_bool(), Some(false));
    }
    let markdown = envelope.rendered_markdown.expect("markdown");
    assert!(markdown.contains("Unmeasured, inferred"));
    assert!(markdown.contains("this is not a passed test"));
}

fn measured_first_feed(value: &str) -> String {
    let mut text = changed(
        TEXT,
        "feed_kg = \"unmeasured\"",
        &format!("feed_kg = \"{value}\""),
    );
    text.push_str(concat!(
        "\n[[covariance]]\nleft = \"feed/0/1000000000\"\nright = \"feed/0/1000000000\"\nkg2 = \"0.04\"\n",
        "\n[[covariance]]\nleft = \"feed/0/1000000000\"\nright = \"outlet/0/1000000000\"\nkg2 = \"0\"\n",
        "\n[[covariance]]\nleft = \"feed/0/1000000000\"\nright = \"outlet/1000000000/2000000000\"\nkg2 = \"0\"\n",
    ));
    text
}

#[test]
fn genuine_feed_observation_creates_a_redundant_error_test_and_changes_the_posterior() {
    let (envelope, value) = evaluate(&measured_first_feed("2.2"));
    assert_eq!(envelope.exit_code, 0, "{}", envelope.to_json());
    let h = (-1.0_f64).exp();
    let k = (1.0 - h).powi(2);
    let precision = 25.0 + 100.0 * h * h;
    let f1 = (55.0 + 200.0 * h * h) / precision;
    let f2 = (2.0 + h + h * h - h * (1.0 - h) - k * f1) / h;
    close(estimate(&value, "feed/0/1000000000"), f1);
    close(estimate(&value, "feed/1000000000/2000000000"), f2);
    close(
        covariance(&value, "feed/0/1000000000", "feed/0/1000000000"),
        1.0 / precision,
    );
    close(
        covariance(&value, "feed/0/1000000000", "feed/1000000000/2000000000"),
        -k / h / precision,
    );
    let expected_chi = (f1 - 2.2).powi(2) / 0.04 + (h * (f1 - 2.0)).powi(2) / 0.01;
    close(
        number(at(&value, &["result", "gross_error", "chi_square"])),
        expected_chi,
    );
    assert_eq!(
        at(&value, &["result", "gross_error", "degrees_of_freedom"]).i64(),
        Some(1)
    );
    assert_ne!(
        at(&value, &["result", "measured_adjustments_kg"]),
        &Json::Null
    );
}

#[test]
fn cross_period_measurement_covariance_is_preserved_in_inferred_feed_uncertainty() {
    let text = changed(
        TEXT,
        "right = \"outlet/1000000000/2000000000\"\nkg2 = \"0\"",
        "right = \"outlet/1000000000/2000000000\"\nkg2 = \"0.005\"",
    );
    let (envelope, value) = evaluate(&text);
    assert_eq!(envelope.exit_code, 0, "{}", envelope.to_json());
    close(estimate(&value, "feed/0/1000000000"), 2.0);
    close(estimate(&value, "feed/1000000000/2000000000"), 4.0);
    let h = (-1.0_f64).exp();
    let k = (1.0 - h).powi(2);
    close(
        covariance(&value, "feed/0/1000000000", "feed/1000000000/2000000000"),
        0.005 / h.powi(2) - 0.01 * k / h.powi(3),
    );
    close(
        covariance(
            &value,
            "feed/1000000000/2000000000",
            "feed/1000000000/2000000000",
        ),
        0.01 * k.powi(2) / h.powi(4) + 0.04 / h.powi(2) - 2.0 * k * 0.005 / h.powi(3),
    );
}

#[test]
fn gross_error_and_negative_mass_retain_rejected_diagnostics_without_admitted_claim() {
    let (envelope, gross) = refused(&measured_first_feed("5"), "FMINE-REF-101");
    assert!(number(at(&gross, &["result", "gross_error", "chi_square"])) > 10.0);
    assert!(at(&gross, &["receipt_digest"]).as_str().is_some());
    assert_eq!(array(at(&gross, &["source_inputs"])).len(), 2);
    assert!(
        envelope
            .rendered_markdown
            .expect("diagnostic markdown")
            .contains("Rejected")
    );
    let text = changed(
        TEXT,
        "outlet_kg = \"1.36787944117144232159552377\"",
        "outlet_kg = \"0.1\"",
    );
    let (_, negative) = refused(&text, "FMINE-REF-109");
    assert!(
        estimate(&negative, "feed/0/1000000000") < 0.0,
        "negative fit must remain visible, never clamped"
    );
    assert!(at(&negative, &["receipt_digest"]).as_str().is_some());
}

#[test]
fn strict_schema_requires_observations_full_covariance_and_explicit_conditional_assumptions() {
    for text in [
        changed(TEXT, "feed_kg = \"unmeasured\"\n", ""),
        changed(TEXT, "kg2 = \"0\"", "variance = \"0\""),
        changed(
            TEXT,
            "right = \"outlet/0/1000000000\"",
            "right = \"feed/0/1000000000\"",
        ),
        changed(
            TEXT,
            "\n[[covariance]]\nleft = \"outlet/0/1000000000\"\nright = \"outlet/1000000000/2000000000\"\nkg2 = \"0\"\n",
            "\n",
        ),
    ] {
        let (_, value) = refused(&text, "FMINE-REF-506");
        assert_eq!(at(&value, &["result"]), &Json::Null);
        assert_eq!(at(&value, &["receipt_digest"]), &Json::Null);
    }
    refused(
        &changed(
            TEXT,
            "mass_basis = \"kg-total/all-phases/full-physical-boundary\"",
            "mass_basis = \"wet-tonnes\"",
        ),
        "FMINE-REF-107",
    );
    let (envelope, value) = evaluate(&changed(
        TEXT,
        "fixed_transport_and_opening = \"true\"",
        "fixed_transport_and_opening = \"false\"",
    ));
    assert_ne!(envelope.exit_code, 0);
    assert_eq!(at(&value, &["admitted_claim"]), &Json::Null);
    let (envelope, value) = evaluate(&changed(TEXT, "kg2 = \"0\"", "kg2 = \"0.1\""));
    assert_ne!(envelope.exit_code, 0, "indefinite covariance must refuse");
    assert_eq!(at(&value, &["admitted_claim"]), &Json::Null);
}

#[test]
fn throughput_time_and_source_identity_are_preserved_by_the_command() {
    let (_, base) = evaluate(TEXT);
    let stretched = TEXT
        .replace("2000000000", "4000000000")
        .replace("1000000000", "2000000000");
    let (envelope, value) = evaluate(&stretched);
    assert_eq!(envelope.exit_code, 0, "{}", envelope.to_json());
    close(estimate(&value, "feed/0/2000000000"), 2.0);
    close(estimate(&value, "feed/2000000000/4000000000"), 4.0);
    assert_eq!(
        at(&base, &["result", "posterior_covariance_kg2"]),
        at(&value, &["result", "posterior_covariance_kg2"])
    );
    assert_ne!(
        at(&base, &["receipt_digest"]),
        at(&value, &["receipt_digest"])
    );
    let root = scratch();
    let (docs, _) = invoke(&["robot-docs".into(), "--json".into()], &root);
    assert_eq!(docs.exit_code, 0);
    assert!(docs.payload_json.contains("plant rtd-infer"));
    assert!(docs.payload_json.contains("fmine.plant-rtd-inverse.v1"));
}
