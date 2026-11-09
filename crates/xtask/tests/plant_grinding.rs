//! SYNTHETIC known answers for the source-bound grinding command and its refusals.
//! Determinism: DeterministicPerIsa

#![cfg(test)]
#![allow(clippy::expect_used)]

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use fmine_cli::{Envelope, invoke, run};
use fmine_core::hash::sha256;
use fmine_io::omf::json::{Json, parse};

const ONE: &str = include_str!("../../../tests/plant/grinding_bond.toml");
const JOINT: &str = include_str!("../../../tests/plant/grinding_joint.toml");
static NEXT: AtomicUsize = AtomicUsize::new(0);

fn scratch() -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "fmine-grinding-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&path).expect("test directory");
    path
}
fn args() -> Vec<String> {
    ["plant", "grinding", "--input", "grinding.toml"]
        .map(str::to_owned)
        .to_vec()
}
fn evaluate(root: &Path, text: &str) -> (Envelope, Json) {
    std::fs::write(root.join("grinding.toml"), text).expect("worksheet bytes");
    let envelope = invoke(&args(), root).0;
    assert_eq!(envelope.to_json(), invoke(&args(), root).0.to_json());
    let json = parse(envelope.payload_json.as_bytes()).expect("payload JSON");
    (envelope, json)
}
fn at<'a>(value: &'a Json, path: &[&str]) -> &'a Json {
    path.iter().fold(value, |v, key| v.get(key).expect(key))
}
fn close(value: &Json, expected: f64) {
    let actual = value.f64().expect("finite number");
    assert!((actual - expected).abs() < 1e-10, "{actual} != {expected}");
}
fn admitted(envelope: &Envelope) {
    assert_eq!(envelope.exit_code, 0, "{}", envelope.to_json());
    assert!(envelope.certificate.is_none());
    assert!(envelope.no_claim.expect("scope").contains("uncalibrated"));
}

#[test]
fn command_reproduces_energy_and_throughput_with_explicit_units_and_source_claims() {
    let root = scratch();
    let (envelope, json) = evaluate(&root, ONE);
    admitted(&envelope);
    close(
        at(
            &json,
            &[
                "grinding",
                "summary",
                "specific_energy_kwh_per_dry_t",
                "mean",
            ],
        ),
        5.0,
    );
    close(
        at(
            &json,
            &["grinding", "summary", "throughput_dry_t_per_hour", "mean"],
        ),
        160.0,
    );
    assert_eq!(
        at(&json, &["grinding", "units", "throughput"]).as_str(),
        Some("dry_metric_tonne/hour")
    );
    assert_eq!(
        at(&json, &["grinding", "submitted_worksheet"]).as_str(),
        Some(ONE)
    );
    let digest = sha256(ONE.as_bytes()).to_hex();
    assert_eq!(
        at(&json, &["source", "sha256"]).as_str(),
        Some(digest.as_str())
    );
    for path in [
        &["grinding", "model", "claim"][..],
        &["grinding", "admitted_claim"][..],
    ] {
        let claim = at(&json, path);
        assert_eq!(at(claim, &["evidence"]).as_str(), Some("Assumed"));
        assert_eq!(at(claim, &["confidentiality"]).as_str(), Some("Internal"));
        assert_eq!(at(claim, &["calibration"]).as_str(), Some("uncalibrated"));
        let sources = at(claim, &["sources"]).as_array().expect("sources");
        assert!(!sources.is_empty());
        for source in sources {
            assert_eq!(at(source, &["sha256"]).as_str(), Some(digest.as_str()));
            assert!(
                !at(source, &["locator"])
                    .as_str()
                    .expect("locator")
                    .is_empty()
            );
        }
    }
    let mut markdown_args = args();
    markdown_args.extend(["--format".into(), "markdown".into()]);
    let (markdown, code) = run(&markdown_args, &root);
    assert_eq!(code, 0);
    assert_eq!(
        Some(markdown.as_str()),
        envelope.rendered_markdown.as_deref()
    );
    assert!(markdown.contains("160"));
    assert!(markdown.contains("Declared closed validity ranges"));
}

#[test]
fn complete_joint_scenarios_are_evaluated_before_averaging_and_shutdown_stays_zero() {
    let root = scratch();
    let (envelope, json) = evaluate(&root, JOINT);
    admitted(&envelope);
    close(
        at(
            &json,
            &[
                "grinding",
                "summary",
                "specific_energy_kwh_per_dry_t",
                "mean",
            ],
        ),
        6.25,
    );
    let throughput = at(&json, &["grinding", "summary", "throughput_dry_t_per_hour"]);
    for (statistic, expected) in [
        ("mean", 200.0),
        ("p10", 80.0),
        ("p50", 80.0),
        ("p90", 320.0),
    ] {
        close(at(throughput, &[statistic]), expected);
    }
    assert_ne!(at(throughput, &["mean"]).f64(), Some(128.0));
    let (envelope, shutdown) = evaluate(
        &root,
        &ONE.replace("power_kw = \"1000\"", "power_kw = \"0\""),
    );
    admitted(&envelope);
    close(
        at(
            &shutdown,
            &["grinding", "summary", "throughput_dry_t_per_hour", "mean"],
        ),
        0.0,
    );
    close(
        at(
            &shutdown,
            &[
                "grinding",
                "summary",
                "specific_energy_kwh_per_dry_t",
                "mean",
            ],
        ),
        5.0,
    );
    for key in ["numerator", "denominator"] {
        assert_eq!(
            at(
                &shutdown,
                &["grinding", "summary", "shutdown_probability", key]
            )
            .as_str(),
            Some("1")
        );
    }
}

#[test]
fn invalid_units_domains_or_one_bad_joint_member_refuse_the_complete_result() {
    let root = scratch();
    for (text, from, to) in [
        (
            ONE,
            "mass_basis = \"dry_metric_tonne\"",
            "mass_basis = \"wet_short_ton\"",
        ),
        (ONE, "work_index = \"10\"", "work_index = \"21\""),
        (
            ONE,
            "complete_joint_population = \"true\"",
            "complete_joint_population = \"false\"",
        ),
        (JOINT, "work_index = \"20\"", "work_index = \"NaN\""),
        (JOINT, "weight = \"1/2\"", "weight = \"1/3\""),
    ] {
        assert!(text.contains(from), "test mutation must change the input");
        let invalid = text.replace(from, to);
        let (envelope, json) = evaluate(&root, &invalid);
        assert_ne!(envelope.exit_code, 0, "{}", envelope.to_json());
        assert!(envelope.refusal.is_some());
        assert_eq!(
            at(&json, &["grinding", "submitted_worksheet"]).as_str(),
            Some(invalid.as_str())
        );
        for field in ["model", "summary", "admitted_claim", "receipt_digest"] {
            assert_eq!(at(&json, &["grinding", field]), &Json::Null);
        }
        assert!(
            at(&json, &["grinding", "scenarios"])
                .as_array()
                .expect("scenarios")
                .is_empty()
        );
        assert!(envelope.rendered_markdown.is_none());
    }
}

#[test]
fn original_numeric_tokens_bind_receipts_and_robot_docs_expose_the_workflow() {
    let root = scratch();
    let (original, first) = evaluate(&root, ONE);
    let (respelt, second) = evaluate(
        &root,
        &ONE.replace("work_index = \"10\"", "work_index = \"10.000\""),
    );
    admitted(&original);
    admitted(&respelt);
    assert_eq!(
        at(&first, &["grinding", "summary"]),
        at(&second, &["grinding", "summary"])
    );
    assert_ne!(
        at(&first, &["source", "sha256"]),
        at(&second, &["source", "sha256"])
    );
    assert_ne!(
        at(&first, &["grinding", "receipt_digest"]),
        at(&second, &["grinding", "receipt_digest"])
    );
    let docs = invoke(&["robot-docs".into()], &root).0;
    assert_eq!(docs.exit_code, 0);
    let docs_json = parse(docs.payload_json.as_bytes()).expect("robot docs JSON");
    assert!(docs_json.get("plant_grinding").is_some());
    assert!(docs.payload_json.contains("fmine.plant-grinding.v1"));
    let invalid = invoke(&["plant".into(), "grinding".into()], &root).0;
    assert_eq!(invalid.exit_code, 2);
    assert!(invalid.to_json().contains("plant grinding --input"));
}
