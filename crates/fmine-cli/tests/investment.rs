//! FACT: synthetic two-period cash investment through the actual CLI binary.
//! Determinism: DeterministicPerIsa

#![allow(clippy::expect_used, clippy::panic)]

use fmine_cli::{OutputFormat, invoke, parse_args, run};
use fmine_core::hash::sha256;
use fmine_io::omf::json::{Json, parse};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

const MANIFEST: &str = include_str!("../../../tests/decisions/production_investment.toml");
static NEXT: AtomicU64 = AtomicU64::new(0);

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repository")
}
fn temporary(text: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!(
        "fmine-investment-{}-{}.toml",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::write(&p, text).expect("synthetic input");
    p
}
fn args(path: &Path) -> Vec<String> {
    vec![
        "decide".into(),
        "investment".into(),
        "--manifest".into(),
        path.to_string_lossy().into_owned(),
    ]
}
fn field<'a>(value: &'a Json, keys: &[&str]) -> &'a Json {
    keys.iter().fold(value, |v, k| v.get(k).expect(k))
}
fn binary(arguments: &[String]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_fmine"))
        .current_dir(root())
        .args(arguments)
        .output()
        .expect("fmine binary")
}

#[test]
fn actual_binary_recomputes_two_period_investments_and_retains_all_original_sources() {
    let args = args(Path::new("tests/decisions/production_investment.toml"));
    let (library, format) = invoke(&args, &root());
    assert_eq!(format, OutputFormat::Json);
    assert_eq!(library.exit_code, 0, "{}", library.to_json());
    let output = binary(&args);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim_end(),
        library.to_json()
    );
    assert_eq!(output.stdout, binary(&args).stdout);
    let json = parse(&output.stdout).expect("investment JSON");
    let summary = field(&json, &["payload", "investment", "decision", "summary"]);
    assert_eq!(
        field(summary, &["expected_value_primary"]).as_str(),
        Some("process")
    );
    let actions = field(summary, &["alternatives"])
        .as_array()
        .expect("three choices");
    assert_eq!(actions.len(), 3);
    for (id, mean) in [
        ("process", "32962/5"),
        ("higher-power", "28424/5"),
        ("do-nothing", "0/1"),
    ] {
        let row = actions
            .iter()
            .find(|a| field(a, &["action_id"]).as_str() == Some(id))
            .expect(id);
        assert_eq!(field(row, &["mean"]).as_str(), Some(mean));
    }
    assert_eq!(
        field(&json, &["payload", "manifest", "submitted_worksheet"]).as_str(),
        Some(MANIFEST)
    );
    assert_eq!(
        field(&json, &["payload", "manifest", "source", "sha256"]).as_str(),
        Some(sha256(MANIFEST.as_bytes()).to_hex().as_str())
    );
    let sources = field(&json, &["payload", "sources"])
        .as_array()
        .expect("source bundles");
    assert_eq!(sources.len(), 4);
    let second = sources
        .iter()
        .find(|b| {
            field(b, &["alternative_id"]).as_str() == Some("process")
                && field(b, &["period_id"]).as_str() == Some("second")
        })
        .expect("second original mode");
    assert_eq!(
        field(second, &["terms", "submitted_worksheet"]).as_str(),
        Some(include_str!(
            "../../../tests/decisions/investment_second_valuation.toml"
        ))
    );
    assert_eq!(
        field(
            second,
            &["production", "sources", "grinding", "submitted_worksheet"]
        )
        .as_str(),
        Some(include_str!(
            "../../../tests/plant/investment_second_grinding.toml"
        ))
    );
    assert_eq!(
        field(&json, &["payload", "investment", "cashflows"])
            .as_array()
            .expect("flows")
            .len(),
        8
    );
    let claim = field(
        &json,
        &["payload", "investment", "decision", "output_claim"],
    );
    assert_eq!(field(claim, &["evidence"]).as_str(), Some("assumed"));
    assert_eq!(
        field(claim, &["confidentiality"]).as_str(),
        Some("Internal")
    );
    let mut markdown = args;
    markdown.extend(["--format".into(), "markdown".into()]);
    let output = binary(&markdown);
    let (library, code) = run(&markdown, &root());
    assert_eq!(code, 0);
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim_end(),
        library.trim_end()
    );
    assert!(library.contains("Initial investment"));
    assert!(library.contains("minus discounted incremental cash value"));
}

#[test]
fn declared_timing_changes_the_winner_and_bad_sources_never_return_a_partial_card() {
    let input =
        temporary(&MANIFEST.replace("discount_factor = \"1/2\"", "discount_factor = \"1\""));
    let output = binary(&args(&input));
    assert!(output.status.success());
    let value = parse(&output.stdout).expect("investment");
    assert_eq!(
        field(
            &value,
            &[
                "payload",
                "investment",
                "decision",
                "summary",
                "expected_value_primary"
            ]
        )
        .as_str(),
        Some("higher-power")
    );
    let bad_terms = temporary(
        &include_str!("../../../tests/decisions/investment_second_valuation_high.toml")
            .replace("do_nothing_usd = \"0\"", "do_nothing_usd = \"0.000001\""),
    );
    for text in [
        MANIFEST.replace(
            "start_ns = \"3600000000000\"",
            "start_ns = \"3599999999999\"",
        ),
        MANIFEST.replace("valuation_time_ns = \"0\"", "valuation_time_ns = \"1\""),
        MANIFEST.replace(
            "tests/decisions/investment_second_valuation_high.toml",
            &bad_terms.to_string_lossy(),
        ),
        MANIFEST.replace(
            "tests/plant/investment_second_grinding.toml",
            "tests/plant/missing-investment-input.toml",
        ),
    ] {
        let output = binary(&args(&temporary(&text)));
        assert!(!output.status.success());
        assert!(!String::from_utf8_lossy(&output.stdout).contains("\"expected_value_primary\""));
        let refusal = parse(&output.stdout).expect("refusal envelope");
        assert_eq!(
            field(&refusal, &["verb"]).as_str(),
            Some("decide investment")
        );
    }
}

#[test]
fn investment_usage_and_robot_schema_are_discoverable_with_shared_output_flags() {
    for words in [
        vec!["decide", "investment"],
        vec!["decide", "investment", "--manifest"],
        vec!["decide", "investment", "--input", "model.toml"],
        vec![
            "decide",
            "investment",
            "--manifest",
            "model.toml",
            "--manifest",
            "other.toml",
        ],
    ] {
        let words = words.into_iter().map(str::to_owned).collect::<Vec<_>>();
        assert!(parse_args(&words, &root()).is_err());
        assert_eq!(binary(&words).status.code(), Some(2));
    }
    let (docs, _) = invoke(&["robot-docs".into()], &root());
    assert_eq!(docs.exit_code, 0);
    assert!(docs.to_json().contains("fmine.production-investment.v1"));
    assert!(docs.to_json().contains("initial_capex_usd"));
    let words = [
        "--format",
        "markdown",
        "decide",
        "investment",
        "--manifest",
        "tests/decisions/production_investment.toml",
    ]
    .map(str::to_owned);
    let (result, format) = invoke(&words, &root());
    assert_eq!(format, OutputFormat::Markdown);
    assert_eq!(result.exit_code, 0);
}
