//! SYNTHETIC end-to-end physical-to-cash decision controls (BD-176/181/225).
//! Expected cash margins below are independent hand calculations, not regenerated fixtures.

#![allow(clippy::expect_used, clippy::panic)]

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use fmine_cli::{Envelope, OutputFormat, invoke, parse_args};
use fmine_core::hash::sha256;
use fmine_io::omf::json::{Json, parse};

const GRINDING: &str = include_str!("../../../tests/plant/grinding_joint.toml");
const RECOVERY: &str = include_str!("../../../tests/plant/production_recovery.toml");
const PRODUCTION: &str = include_str!("../../../tests/plant/production_joint.toml");
const TERMS: &str = include_str!("../../../tests/decisions/production_valuation.toml");
static NEXT: AtomicU64 = AtomicU64::new(0);

fn arguments() -> Vec<String> {
    [
        "decide",
        "production",
        "--input",
        "production.toml",
        "--grinding",
        "grinding.toml",
        "--recovery",
        "recovery.toml",
        "--terms",
        "terms.toml",
    ]
    .map(str::to_owned)
    .into()
}

fn fixture() -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "fmine-production-valuation-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed),
    ));
    std::fs::create_dir_all(&root).expect("test directory");
    for (name, content) in [
        ("grinding.toml", GRINDING),
        ("recovery.toml", RECOVERY),
        ("production.toml", PRODUCTION),
        ("terms.toml", TERMS),
    ] {
        std::fs::write(root.join(name), content).expect("source worksheet");
    }
    root
}

fn field<'a>(value: &'a Json, keys: &[&str]) -> &'a Json {
    keys.iter()
        .fold(value, |value, key| value.get(key).expect(key))
}

fn okay(root: &std::path::Path) -> (Envelope, Json) {
    let (envelope, format) = invoke(&arguments(), root);
    assert_eq!(format, OutputFormat::Json);
    assert_eq!(envelope.exit_code, 0, "{}", envelope.to_json());
    let parsed = parse(envelope.to_json().as_bytes()).expect("complete JSON envelope");
    (envelope, parsed)
}

#[test]
fn full_physical_commercial_path_matches_independent_margins_and_preserves_sources() {
    let root = fixture();
    let (envelope, json) = okay(&root);
    assert_eq!(envelope.to_json(), okay(&root).0.to_json());
    let summary = field(&json, &["payload", "valuation", "decision", "summary"]);
    assert_eq!(
        field(summary, &["valuation_basis"]).as_str(),
        Some("immediate_one_period_cash_margin")
    );
    assert_eq!(
        field(summary, &["expected_value_primary"]).as_str(),
        Some("process")
    );
    let alternatives = field(summary, &["alternatives"])
        .as_array()
        .expect("alternatives");
    let process = alternatives
        .iter()
        .find(|a| field(a, &["action_id"]).as_str() == Some("process"))
        .expect("process action");
    // (808 + 9315.2) / 2; royalties and output are paired in their original states.
    assert_eq!(field(process, &["mean"]).as_str(), Some("25308/5"));
    assert_eq!(field(process, &["cvar95_loss"]).as_str(), Some("-808/1"));
    assert_eq!(field(process, &["max_regret"]).as_str(), Some("0/1"));
    for (id, expected) in [("soft", "808/1"), ("hard", "46576/5")] {
        let row = field(process, &["states"])
            .as_array()
            .expect("states")
            .iter()
            .find(|s| field(s, &["state_id"]).as_str() == Some(id))
            .expect("state");
        assert_eq!(field(row, &["payoff"]).as_str(), Some(expected));
    }
    let terms = field(&json, &["payload", "terms"]);
    assert_eq!(field(terms, &["submitted_worksheet"]).as_str(), Some(TERMS));
    assert_eq!(
        field(terms, &["source", "sha256"]).as_str(),
        Some(sha256(TERMS.as_bytes()).to_hex().as_str())
    );
    for (name, expected) in [("grinding", GRINDING), ("recovery", RECOVERY)] {
        assert_eq!(
            field(
                &json,
                &[
                    "payload",
                    "production",
                    "sources",
                    name,
                    "submitted_worksheet"
                ]
            )
            .as_str(),
            Some(expected)
        );
    }
    assert_eq!(
        field(
            &json,
            &["payload", "production", "production", "submitted_worksheet"]
        )
        .as_str(),
        Some(PRODUCTION)
    );
    let claim = field(&json, &["payload", "valuation", "decision", "output_claim"]);
    assert_eq!(field(claim, &["evidence"]).as_str(), Some("assumed"));
    assert_eq!(
        field(claim, &["confidentiality"]).as_str(),
        Some("Internal")
    );
    for flag in [
        "reserve_disclosure_claim",
        "execution_authorized",
        "geotechnical_signoff",
        "regulatory_compliance_claim",
    ] {
        assert!(envelope.to_json().contains(&format!("\"{flag}\":false")));
    }
    let mut args = arguments();
    args.extend(["--format".into(), "markdown".into()]);
    let (markdown, format) = invoke(&args, &root);
    assert_eq!(format, OutputFormat::Markdown);
    assert_eq!(markdown.exit_code, 0);
    assert_eq!(markdown.rendered_markdown, envelope.rendered_markdown);
    let body = markdown.rendered_markdown.expect("rendered decision");
    assert!(body.contains("cash"), "{body}");
    assert!(body.contains("round"), "{body}");
}

#[test]
fn commercial_source_changes_values_and_scope_or_population_errors_refuse() {
    let root = fixture();
    let original = okay(&root).0.to_json();
    std::fs::write(
        root.join("terms.toml"),
        TERMS.replace("usd_per_kg = \"50000\"", "usd_per_kg = \"40000\""),
    )
    .expect("changed price");
    let changed = okay(&root).0.to_json();
    assert_ne!(original, changed);
    for terms in [
        TERMS.replace("element = \"Au\"", "element = \"Cu\""),
        TERMS.replacen("weight = \"1/2\"", "weight = \"1/3\"", 1),
        TERMS.replace("id = \"hard\"", "id = \"foreign\""),
        TERMS.replace("payable_fraction = \"0.9\"", "payable_fraction = \"1.1\""),
        TERMS.replace("nearest_ties_even_0_0001_usd", "implicit"),
        TERMS.replace("end_ns = \"3600000000000\"", "end_ns = \"7200000000000\""),
        TERMS.replace("immediate_one_period_cash_margin", "project_npv"),
    ] {
        std::fs::write(root.join("terms.toml"), terms).expect("invalid terms");
        let (result, _) = invoke(&arguments(), &root);
        assert_ne!(result.exit_code, 0, "{}", result.to_json());
        assert!(result.refusal.is_some());
        assert!(!result.to_json().contains("\"expected_value_primary\""));
    }
    std::fs::write(root.join("terms.toml"), TERMS).expect("restore original fixture");
    for (name, content) in [
        ("terms.toml", TERMS),
        ("production.toml", PRODUCTION),
        ("grinding.toml", GRINDING),
        ("recovery.toml", RECOVERY),
    ] {
        std::fs::write(root.join(name), "").expect("blank source");
        assert_ne!(invoke(&arguments(), &root).0.exit_code, 0, "{name}");
        std::fs::write(root.join(name), content).expect("restore fixture source");
    }
}

#[test]
fn production_decision_has_strict_options_and_robot_documentation() {
    let root = fixture();
    for extra in [
        vec!["--terms", "other.toml"],
        vec!["--price", "100"],
        vec!["--terms"],
    ] {
        let mut args = arguments();
        args.extend(extra.into_iter().map(str::to_owned));
        assert!(parse_args(&args, &root).is_err());
        let (result, _) = invoke(&args, &root);
        assert_eq!(result.verb, "decide production");
        assert_eq!(result.exit_code, 2);
    }
    let (docs, _) = invoke(&["robot-docs".into()], &root);
    assert_eq!(docs.exit_code, 0);
    let json = parse(docs.to_json().as_bytes()).expect("robot docs JSON");
    assert_eq!(
        field(&json, &["payload", "production_decision", "schema"]).as_str(),
        Some("fmine.production-valuation.v1")
    );
}
