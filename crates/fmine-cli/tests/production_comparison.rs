//! SYNTHETIC source-bound operating alternatives through the actual command.
//! Determinism: DeterministicPerIsa (conditional physical calculations).

#![allow(clippy::expect_used, clippy::panic)]

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use fmine_cli::{OutputFormat, invoke, parse_args, run};
use fmine_core::hash::sha256;
use fmine_io::omf::json::{Json, parse};

const MANIFEST: &str = include_str!("../../../tests/decisions/production_comparison.toml");
const TESTWORK: &str = include_str!("../../../tests/decisions/production_testwork.toml");
const HIGH_TERMS: &str =
    include_str!("../../../tests/decisions/production_valuation_higher_power.toml");
const HIGH_GRINDING: &str = include_str!("../../../tests/plant/grinding_higher_power.toml");
static NEXT: AtomicU64 = AtomicU64::new(0);

fn fixture() -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "fmine-production-comparison-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(root.join("tests/plant")).expect("plant fixtures");
    std::fs::create_dir_all(root.join("tests/decisions")).expect("decision fixtures");
    for (path, text) in [
        ("tests/decisions/production_comparison.toml", MANIFEST),
        (
            "tests/decisions/production_valuation_higher_power.toml",
            HIGH_TERMS,
        ),
        (
            "tests/decisions/production_valuation.toml",
            include_str!("../../../tests/decisions/production_valuation.toml"),
        ),
        (
            "tests/plant/grinding_joint.toml",
            include_str!("../../../tests/plant/grinding_joint.toml"),
        ),
        ("tests/plant/grinding_higher_power.toml", HIGH_GRINDING),
        (
            "tests/plant/production_joint.toml",
            include_str!("../../../tests/plant/production_joint.toml"),
        ),
        (
            "tests/plant/production_recovery.toml",
            include_str!("../../../tests/plant/production_recovery.toml"),
        ),
    ] {
        std::fs::write(root.join(path), text).expect("synthetic source");
    }
    std::fs::canonicalize(&root).expect("canonical temp root")
}

fn arguments() -> Vec<String> {
    [
        "decide",
        "production",
        "--manifest",
        "tests/decisions/production_comparison.toml",
    ]
    .map(str::to_owned)
    .into()
}
fn field<'a>(value: &'a Json, keys: &[&str]) -> &'a Json {
    keys.iter()
        .fold(value, |value, key| value.get(key).expect(key))
}
fn binary(root: &Path, args: &[String]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_fmine"))
        .current_dir(root)
        .args(args)
        .output()
        .expect("actual fmine binary")
}

#[test]
fn actual_binary_compares_both_modes_with_exact_regret_and_complete_source_snapshots() {
    let root = fixture();
    let args = arguments();
    let (library, format) = invoke(&args, &root);
    assert_eq!(format, OutputFormat::Json);
    assert_eq!(library.exit_code, 0, "{}", library.to_json());
    let output = binary(&root, &args);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim_end(),
        library.to_json()
    );
    assert_eq!(output.stdout, binary(&root, &args).stdout);
    let json = parse(&output.stdout).expect("JSON output");
    let summary = field(&json, &["payload", "comparison", "decision", "summary"]);
    assert_eq!(
        field(summary, &["expected_value_primary"]).as_str(),
        Some("higher-power")
    );
    let choices = field(summary, &["alternatives"])
        .as_array()
        .expect("choices");
    assert_eq!(choices.len(), 3);
    for (id, mean, tail, regret) in [
        ("process", "25308/5", "-808/1", "23288/5"),
        ("higher-power", "45616/5", "384/1", "596/1"),
    ] {
        let row = choices
            .iter()
            .find(|row| field(row, &["action_id"]).as_str() == Some(id))
            .expect("mode");
        for (key, expected) in [
            ("mean", mean),
            ("cvar95_loss", tail),
            ("mean_regret", regret),
        ] {
            assert_eq!(field(row, &[key]).as_str(), Some(expected));
        }
    }
    assert_eq!(
        field(&json, &["payload", "manifest", "submitted_worksheet"]).as_str(),
        Some(MANIFEST)
    );
    let sources = field(&json, &["payload", "alternatives"])
        .as_array()
        .expect("source bundles");
    assert_eq!(sources.len(), 2);
    let high = sources
        .iter()
        .find(|row| field(row, &["action_id"]).as_str() == Some("higher-power"))
        .expect("higher-power sources");
    assert_eq!(
        field(high, &["terms", "submitted_worksheet"]).as_str(),
        Some(HIGH_TERMS)
    );
    assert_eq!(
        field(high, &["terms", "source", "sha256"]).as_str(),
        Some(sha256(HIGH_TERMS.as_bytes()).to_hex().as_str())
    );
    assert_eq!(
        field(
            high,
            &["production", "sources", "grinding", "submitted_worksheet"]
        )
        .as_str(),
        Some(HIGH_GRINDING)
    );
    let claim = field(
        &json,
        &["payload", "comparison", "decision", "output_claim"],
    );
    assert_eq!(field(claim, &["evidence"]).as_str(), Some("assumed"));
    assert_eq!(
        field(claim, &["confidentiality"]).as_str(),
        Some("Internal")
    );
    let mut markdown_args = args;
    markdown_args.extend(["--format".into(), "markdown".into()]);
    let markdown = binary(&root, &markdown_args);
    assert!(markdown.status.success());
    let (library_markdown, status) = run(&markdown_args, &root);
    assert_eq!(status, 0);
    assert_eq!(
        String::from_utf8_lossy(&markdown.stdout).trim_end(),
        library_markdown.trim_end()
    );
}

#[test]
fn a_common_ore_supply_limit_reverses_the_preferred_grind_through_actual_valuation() {
    let root = fixture();
    let grinding = include_str!("../../../tests/plant/grinding_joint.toml")
        .replace("p80_microns = \"100\"", "p80_microns = \"144\"");
    let recovery = include_str!("../../../tests/plant/production_recovery.toml")
        .replace("value = \"100\"", "value = \"144\"");
    let terms = include_str!("../../../tests/decisions/production_valuation.toml")
        .replace("action_id = \"process\"", "action_id = \"coarse\"")
        .replace(
            "action_label = \"Process the declared shift\"",
            "action_label = \"Coarser grind\"",
        );
    let manifest = MANIFEST
        .replace("higher-power", "coarse")
        .replace("grinding_higher_power.toml", "grinding_coarse.toml")
        .replace("production_valuation_higher_power.toml", "production_valuation_coarse.toml")
        .replace(
            "grinding = \"tests/plant/grinding_coarse.toml\"\nrecovery = \"tests/plant/production_recovery.toml\"",
            "grinding = \"tests/plant/grinding_coarse.toml\"\nrecovery = \"tests/plant/production_recovery_coarse.toml\"",
        )
        .replace(
            "only the declared power and operating charges change by action",
            "only P80 and its declared recovery response change by action; available ore and commercial terms are common",
        );
    for (path, text) in [
        ("tests/plant/grinding_coarse.toml", grinding),
        ("tests/plant/production_recovery_coarse.toml", recovery),
        ("tests/decisions/production_valuation_coarse.toml", terms),
        ("tests/decisions/production_comparison.toml", manifest),
    ] {
        std::fs::write(root.join(path), text).expect("authored coarse-grind alternative");
    }
    let summary = |output: &std::process::Output| {
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stdout)
        );
        parse(&output.stdout).expect("decision JSON")
    };
    let first = summary(&binary(&root, &arguments()));
    let first = field(&first, &["payload", "comparison", "decision", "summary"]);
    assert_eq!(
        field(first, &["expected_value_primary"]).as_str(),
        Some("coarse")
    );
    let mean = |value: &Json, id: &str| {
        let row = field(value, &["alternatives"])
            .as_array()
            .expect("alternatives")
            .iter()
            .find(|row| field(row, &["action_id"]).as_str() == Some(id))
            .expect("action");
        field(row, &["mean"])
            .as_str()
            .expect("exact mean")
            .to_owned()
    };
    // Independent Bond/response arithmetic: P80 144 gives Q 480/120 t/h and
    // recovery .556/.856. The same commercial terms give $809.12/$13,908.352.
    assert_eq!(mean(first, "process"), "25308/5");
    assert_eq!(mean(first, "coarse"), "919842/125");

    let supply = include_str!("../../../tests/plant/production_supply_limited.toml")
        .replace(
            "available_dry_tonnes = \"100\"",
            "available_dry_tonnes = \"50\"",
        )
        .replace("100 dry metric tonnes", "50 dry metric tonnes");
    std::fs::write(root.join("tests/plant/production_joint.toml"), &supply)
        .expect("same 50 t supply for both alternatives");
    let second = summary(&binary(&root, &arguments()));
    let decision = field(&second, &["payload", "comparison", "decision", "summary"]);
    assert_eq!(
        field(decision, &["expected_value_primary"]).as_str(),
        Some("process")
    );
    // Both states and modes now process 50 t. Fine margins are -717.50/5072;
    // coarse margins are -811.55/4628.48. Do-nothing remains in the comparison.
    assert_eq!(mean(decision, "process"), "8709/4");
    assert_eq!(mean(decision, "coarse"), "381693/200");
    for alternative in field(&second, &["payload", "alternatives"])
        .as_array()
        .expect("sources")
    {
        let production = field(alternative, &["production", "production"]);
        assert_eq!(
            field(production, &["submitted_worksheet"]).as_str(),
            Some(supply.as_str())
        );
        assert_eq!(
            field(production, &["schema"]).as_str(),
            Some("fmine.plant-production-result.v2")
        );
        for state in field(production, &["scenarios"])
            .as_array()
            .expect("states")
        {
            assert_eq!(field(state, &["dry_tonnes"]).f64(), Some(50.0));
            assert_eq!(
                field(state, &["supply_use", "binding_constraint"]).as_str(),
                Some("ore_supply")
            );
        }
    }
}

#[test]
fn a_bad_alternative_refuses_the_whole_command_without_partial_rankings() {
    let root = fixture();
    for terms in [
        HIGH_TERMS.replace("action_id = \"higher-power\"", "action_id = \"foreign\""),
        HIGH_TERMS.replace("do_nothing_usd = \"0\"", "do_nothing_usd = \"0.000001\""),
        HIGH_TERMS.replace(
            "with no opening stock",
            "with different timing and no opening stock",
        ),
        String::new(),
    ] {
        std::fs::write(
            root.join("tests/decisions/production_valuation_higher_power.toml"),
            terms,
        )
        .expect("invalid synthetic terms");
        let output = binary(&root, &arguments());
        assert!(!output.status.success());
        let text = String::from_utf8_lossy(&output.stdout);
        assert!(!text.contains("\"expected_value_primary\""));
        let json = parse(&output.stdout).expect("refusal envelope");
        assert_eq!(field(&json, &["verb"]).as_str(), Some("decide production"));
    }
}

#[test]
fn manifest_options_are_exclusive_and_the_comparison_schema_is_discoverable() {
    let root = fixture();
    for extra in [
        vec!["--input", "other.toml"],
        vec!["--manifest", "other.toml"],
        vec!["--terms"],
    ] {
        let mut args = arguments();
        args.extend(extra.into_iter().map(str::to_owned));
        assert!(parse_args(&args, &root).is_err());
        assert_eq!(binary(&root, &args).status.code(), Some(2));
    }
    let (docs, _) = invoke(&["robot-docs".into()], &root);
    assert_eq!(docs.exit_code, 0);
    let json = parse(docs.to_json().as_bytes()).expect("robot documentation");
    assert_eq!(
        field(
            &json,
            &["payload", "production_decision", "comparison", "schema"]
        )
        .as_str(),
        Some("fmine.production-comparison.v1")
    );
}

#[test]
fn actual_binary_applies_source_bound_downside_limits_without_replacing_nominal_rankings() {
    let root = fixture();
    let args = arguments();
    let text = format!(
        "{MANIFEST}\n[[downside_limit]]\nfloor_usd = \"0\"\nmax_breach_probability = \"1/4\"\n"
    );
    std::fs::write(
        root.join("tests/decisions/production_comparison.toml"),
        &text,
    )
    .expect("declared synthetic downside limit");
    let output = binary(&root, &args);
    assert!(output.status.success());
    let (library, _) = invoke(&args, &root);
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim_end(),
        library.to_json()
    );
    assert_eq!(output.stdout, binary(&root, &args).stdout);
    let json = parse(&output.stdout).expect("JSON comparison");
    let summary = field(&json, &["payload", "comparison", "decision", "summary"]);
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
        field(screen, &["nominal", "expected_value_cost_of_limit"]).as_str(),
        Some("20308/5")
    );
    let choices = field(screen, &["choices"])
        .as_array()
        .expect("complete choices");
    assert_eq!(choices.len(), 3);
    let higher = choices
        .iter()
        .find(|choice| field(choice, &["id"]).as_str() == Some("higher-power"))
        .expect("rejected mode retained");
    assert_eq!(
        field(higher, &["nominal_breach_probability"]).as_str(),
        Some("1/2")
    );
    assert_eq!(field(higher, &["nominal_eligible"]).as_bool(), Some(false));
    assert_eq!(
        field(&json, &["payload", "manifest", "submitted_worksheet"]).as_str(),
        Some(text.as_str())
    );
    assert_eq!(
        field(&json, &["payload", "manifest", "source", "sha256"]).as_str(),
        Some(sha256(text.as_bytes()).to_hex().as_str())
    );
    let mut markdown_args = args;
    markdown_args.extend(["--format".into(), "markdown".into()]);
    let markdown = binary(&root, &markdown_args);
    assert!(markdown.status.success());
    let shown = String::from_utf8_lossy(&markdown.stdout);
    assert!(shown.contains("**Downside-eligible expected-value co-winners:** process"));
    assert!(shown.contains("| higher-power | 1/2 | rejected |"));
    assert!(shown.contains("**Expected-value co-winners:** higher-power"));
}

#[test]
fn all_failed_downside_screen_has_no_fallback_and_invalid_limits_refuse_atomically() {
    let root = fixture();
    let args = arguments();
    let text = format!(
        "{MANIFEST}\n[[downside_limit]]\nfloor_usd = \"20000\"\nmax_breach_probability = \"0\"\n"
    );
    let path = root.join("tests/decisions/production_comparison.toml");
    std::fs::write(&path, &text).expect("synthetic impossible downside limit");
    let output = binary(&root, &args);
    assert!(output.status.success());
    let json = parse(&output.stdout).expect("complete assessment");
    let screen = field(
        &json,
        &[
            "payload",
            "comparison",
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
            .expect("empty winners")
            .is_empty()
    );
    assert_eq!(field(screen, &["best_expected_value"]), &Json::Null);
    for invalid in [
        text.replace("floor_usd = \"20000\"", "floor_usd = \"0.00001\""),
        text.replace(
            "max_breach_probability = \"0\"",
            "max_breach_probability = \"-1/4\"",
        ),
        text.replace(
            "max_breach_probability = \"0\"",
            "max_breach_probability = \"5/4\"",
        ),
    ] {
        std::fs::write(&path, invalid).expect("invalid declared limit");
        let output = binary(&root, &args);
        assert!(!output.status.success());
        assert!(!String::from_utf8_lossy(&output.stdout).contains("\"expected_value_primary\""));
    }
}

#[test]
fn actual_binary_values_source_derived_testwork_and_renders_the_same_complete_policy() {
    let root = fixture();
    std::fs::write(
        root.join("tests/decisions/production_comparison.toml"),
        TESTWORK,
    )
    .expect("synthetic testwork manifest");
    let args = arguments();
    let (library, _) = invoke(&args, &root);
    assert_eq!(library.exit_code, 0, "{}", library.to_json());
    let output = binary(&root, &args);
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim_end(),
        library.to_json()
    );
    assert_eq!(output.stdout, binary(&root, &args).stdout);
    let json = parse(&output.stdout).expect("testwork output");
    let comparison = field(&json, &["payload", "comparison"]);
    assert_eq!(
        field(comparison, &["information_basis"]).as_str(),
        Some("before_action_same_payoffs_explicit_delay_cost")
    );
    let summary = field(comparison, &["decision", "summary"]);
    assert_eq!(
        field(summary, &["expected_value_primary"]).as_str(),
        Some("higher-power")
    );
    let information = field(summary, &["information"]);
    let option = &field(information, &["options"])
        .as_array()
        .expect("information options")[0];
    for (key, expected) in [
        ("evsi", "596/1"),
        ("test_cost", "80/1"),
        ("delay_cost", "20/1"),
        ("net_voi", "496/1"),
        ("net_value", "48096/5"),
    ] {
        assert_eq!(field(option, &[key]).as_str(), Some(expected));
    }
    let winner = &field(information, &["overall_winners"])
        .as_array()
        .expect("overall winners")[0];
    assert_eq!(field(winner, &["id"]).as_str(), Some("hardness-test"));
    assert_eq!(
        field(winner, &["kind"]).as_str(),
        Some("information_option")
    );
    for (signal, action) in [("soft-result", "process"), ("hard-result", "higher-power")] {
        let policy = field(option, &["signals"])
            .as_array()
            .expect("signal policies")
            .iter()
            .find(|row| field(row, &["signal_id"]).as_str() == Some(signal))
            .expect("retained signal");
        assert_eq!(field(policy, &["selected_action"]).as_str(), Some(action));
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
        field(comparison, &["decision", "output_claim", "evidence"]).as_str(),
        Some("assumed")
    );
    assert_eq!(
        field(comparison, &["decision", "output_claim", "confidentiality"]).as_str(),
        Some("Internal")
    );
    assert!(
        field(&json, &["no_claim"])
            .as_str()
            .expect("scope statement")
            .contains("test and delay costs are USD")
    );
    let mut markdown_args = args;
    markdown_args.extend(["--format".into(), "markdown".into()]);
    let markdown = binary(&root, &markdown_args);
    assert!(markdown.status.success());
    let (rendered, code) = run(&markdown_args, &root);
    assert_eq!(code, 0);
    assert_eq!(
        String::from_utf8_lossy(&markdown.stdout).trim_end(),
        rendered.trim_end()
    );
    for text in [
        "496/1",
        "Conditional policy: hardness-test",
        "Complete declared likelihood matrices",
        "same immediate USD cash-margin basis",
    ] {
        assert!(rendered.contains(text));
    }
    let (docs, _) = invoke(&["robot-docs".into()], &root);
    let docs = parse(docs.to_json().as_bytes()).expect("robot docs");
    assert_eq!(
        field(
            &docs,
            &[
                "payload",
                "production_decision",
                "comparison",
                "information_example",
            ],
        )
        .as_str(),
        Some("tests/decisions/production_testwork.toml")
    );
}

#[test]
fn incomplete_or_unbound_testwork_refuses_the_whole_source_comparison() {
    let root = fixture();
    let path = root.join("tests/decisions/production_comparison.toml");
    let missing_state = TESTWORK
        .split("\n[[likelihood]]")
        .filter(|row| !row.contains("state = \"soft\""))
        .collect::<Vec<_>>()
        .join("\n[[likelihood]]");
    for invalid in [
        missing_state,
        TESTWORK.replace(
            "information_basis = \"before_action_same_payoffs_explicit_delay_cost\"\n",
            "",
        ),
        TESTWORK.replace("state = \"soft\"", "state = \"foreign\""),
        TESTWORK.replace("delay_cost = \"20\"", "delay_cost = \"-0.0001\""),
        TESTWORK.replace("test_cost = \"80\"", "test_cost = \"80.00001\""),
        TESTWORK.replacen("probability = \"1/1\"", "probability = \"9/10\"", 1),
    ] {
        std::fs::write(&path, invalid).expect("invalid synthetic testwork");
        let output = binary(&root, &arguments());
        assert!(!output.status.success());
        let text = String::from_utf8_lossy(&output.stdout);
        assert!(!text.contains("\"expected_value_primary\""));
        assert!(!text.contains("\"overall_winners\""));
        assert!(parse(&output.stdout).is_ok());
    }
}
