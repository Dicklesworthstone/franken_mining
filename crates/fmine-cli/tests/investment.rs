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
const TESTWORK: &str = include_str!("../../../tests/decisions/investment_testwork.toml");
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
fn actual_binary_values_testwork_from_retained_investment_cash_and_preserves_signal_policies() {
    let arguments = args(Path::new("tests/decisions/investment_testwork.toml"));
    let (library, _) = invoke(&arguments, &root());
    assert_eq!(library.exit_code, 0, "{}", library.to_json());
    let output = binary(&arguments);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim_end(),
        library.to_json()
    );
    assert_eq!(output.stdout, binary(&arguments).stdout);
    let json = parse(&output.stdout).expect("testwork investment JSON");
    let decision = field(&json, &["payload", "investment", "decision"]);
    let information = field(decision, &["summary", "information"]);
    let test = &field(information, &["options"])
        .as_array()
        .expect("options")[0];
    for (key, value) in [
        ("gross_value", "50394/5"),
        ("net_value", "49769/5"),
        ("evsi", "17432/5"),
        ("net_voi", "16807/5"),
        ("total_cost", "125/1"),
    ] {
        assert_eq!(field(test, &[key]).as_str(), Some(value), "{key}");
    }
    for (id, expected) in [("soft-result", "process"), ("hard-result", "higher-power")] {
        let signal = field(test, &["signals"])
            .as_array()
            .expect("signals")
            .iter()
            .find(|s| field(s, &["signal_id"]).as_str() == Some(id))
            .expect("signal");
        assert_eq!(field(signal, &["selected_action"]).as_str(), Some(expected));
    }
    assert_eq!(
        field(&json, &["payload", "manifest", "submitted_worksheet"]).as_str(),
        Some(TESTWORK)
    );
    assert_eq!(
        field(&json, &["payload", "manifest", "source", "sha256"]).as_str(),
        Some(sha256(TESTWORK.as_bytes()).to_hex().as_str())
    );
    assert_eq!(
        field(decision, &["output_claim", "evidence"]).as_str(),
        Some("assumed")
    );
    assert_eq!(
        field(decision, &["output_claim", "confidentiality"]).as_str(),
        Some("Internal")
    );
    assert_eq!(
        field(decision, &["input_claims", "information_options"])
            .as_array()
            .expect("source-bound option")
            .len(),
        1
    );
    for flag in [
        "reserve_disclosure_claim",
        "execution_authorized",
        "geotechnical_signoff",
        "regulatory_compliance_claim",
    ] {
        assert!(library.to_json().contains(&format!("\"{flag}\":false")));
    }
    let mut markdown = arguments;
    markdown.extend(["--format".into(), "markdown".into()]);
    let output = binary(&markdown);
    let (text, exit) = run(&markdown, &root());
    assert_eq!(exit, 0);
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim_end(),
        text.trim_end()
    );
    assert!(text.contains("Value of information"));
    assert!(text.contains("charged once on every signal branch"));
    assert!(text.contains("soft-result") && text.contains("hard-result"));
}

#[test]
fn testwork_command_refuses_incomplete_likelihoods_and_noisy_policy_keeps_realized_downside() {
    for input in [
        TESTWORK.replace(
            "information_basis = \"before_action_same_payoffs_explicit_delay_cost\"\n",
            "",
        ),
        TESTWORK
            .rsplit_once("[[likelihood]]")
            .expect("matrix")
            .0
            .into(),
        TESTWORK.replace("test_cost = \"100\"", "test_cost = \"0.00001\""),
    ] {
        let path = temporary(&input);
        let output = binary(&args(&path));
        assert!(!output.status.success());
        let body = String::from_utf8(output.stdout).expect("UTF-8 refusal");
        assert!(body.contains("FMINE-REF-"), "{body}");
        assert!(!body.contains("\"expected_value_primary\""), "{body}");
    }
    let noisy = TESTWORK
        .replace("probability = \"1/1\"", "probability = \"3/4\"")
        .replace("probability = \"0/1\"", "probability = \"1/4\"")
        + "\n[[downside_limit]]\nfloor_usd = \"0\"\nmax_breach_probability = \"0\"\n";
    let output = binary(&args(&temporary(&noisy)));
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    let json = parse(&output.stdout).expect("noisy investment JSON");
    let summary = field(&json, &["payload", "investment", "decision", "summary"]);
    let option = &field(summary, &["information", "options"])
        .as_array()
        .expect("test")[0];
    assert_eq!(
        field(option, &["policy_risk", "loss_probability"]).as_str(),
        Some("1/8")
    );
    let choices = field(summary, &["downside", "choices"])
        .as_array()
        .expect("choices");
    let test = choices
        .iter()
        .find(|c| field(c, &["id"]).as_str() == Some("ore-test"))
        .expect("test screen");
    assert_eq!(
        field(test, &["nominal_breach_probability"]).as_str(),
        Some("1/8")
    );
    let (docs, _) = invoke(&["robot-docs".into()], &root());
    assert!(
        docs.to_json()
            .contains("before_action_same_payoffs_explicit_delay_cost")
    );
    assert!(docs.to_json().contains("investment_testwork.toml"));
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
    assert!(docs.to_json().contains("downside_limit_fields"));
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

#[test]
fn actual_binary_screens_investment_losses_and_preserves_sources_and_nominal_ranking() {
    let text = format!(
        "{}\n[[downside_limit]]\nfloor_usd = \"0\"\nmax_breach_probability = \"0\"\n",
        MANIFEST.replace("discount_factor = \"1/2\"", "discount_factor = \"1\"")
    );
    let path = temporary(&text);
    let arguments = args(&path);
    let output = binary(&arguments);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert_eq!(output.stdout, binary(&arguments).stdout);
    let json = parse(&output.stdout).expect("investment JSON");
    let summary = field(&json, &["payload", "investment", "decision", "summary"]);
    assert_eq!(
        field(summary, &["expected_value_primary"]).as_str(),
        Some("higher-power")
    );
    let screen = field(summary, &["downside"]);
    let winners = field(screen, &["nominal", "winners"])
        .as_array()
        .expect("eligible winners");
    assert_eq!(winners.len(), 1);
    assert_eq!(field(&winners[0], &["id"]).as_str(), Some("process"));
    assert_eq!(
        field(screen, &["choices"])
            .as_array()
            .expect("all choices")
            .len(),
        3
    );
    assert_eq!(
        field(&json, &["payload", "manifest", "source", "sha256"]).as_str(),
        Some(sha256(text.as_bytes()).to_hex().as_str())
    );
    assert_eq!(
        field(&json, &["payload", "manifest", "submitted_worksheet"]).as_str(),
        Some(text.as_str())
    );
    let mut markdown = arguments;
    markdown.extend(["--format".into(), "markdown".into()]);
    let output = binary(&markdown);
    assert!(output.status.success());
    let rendered = String::from_utf8(output.stdout).expect("markdown");
    assert!(rendered.contains("final rounded discounted incremental value after initial capex"));
    assert!(rendered.contains("Expected-value co-winners:** higher-power"));
    assert!(rendered.contains("Downside-eligible expected-value co-winners:** process"));
}

#[test]
fn binary_reports_no_eligible_investment_and_refuses_malformed_limits_atomically() {
    let text = format!(
        "{MANIFEST}\n[[downside_limit]]\nfloor_usd = \"212.0001\"\nmax_breach_probability = \"0\"\n"
    );
    let output = binary(&args(&temporary(&text)));
    assert!(output.status.success());
    let json = parse(&output.stdout).expect("investment JSON");
    let screen = field(
        &json,
        &[
            "payload",
            "investment",
            "decision",
            "summary",
            "downside",
            "nominal",
        ],
    );
    assert_eq!(
        field(screen, &["status"]).as_str(),
        Some("no_eligible_choice")
    );
    assert!(
        field(screen, &["winners"])
            .as_array()
            .expect("winners")
            .is_empty()
    );
    for text in [
        text.replace("floor_usd = \"212.0001\"", "floor_usd = \"0.00001\""),
        text.replace(
            "max_breach_probability = \"0\"",
            "max_breach_probability = \"1.0001\"",
        ),
        format!("{text}\n[[downside_limit]]\nfloor_usd = \"0\"\nmax_breach_probability = \"0\"\n"),
    ] {
        let output = binary(&args(&temporary(&text)));
        assert!(!output.status.success());
        assert!(!String::from_utf8_lossy(&output.stdout).contains("\"expected_value_primary\""));
        assert_eq!(
            field(&parse(&output.stdout).expect("refusal"), &["verb"]).as_str(),
            Some("decide investment")
        );
    }
}
