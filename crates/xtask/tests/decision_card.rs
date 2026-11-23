#![cfg(test)]

//! BD-225/227: real-binary finite-decision known answers and library/rendering parity.
//! Fixtures declare invented assumptions; none represents a real investment recommendation.

#![allow(clippy::expect_used)]

use fmine_cli::OutputFormat;
use std::path::PathBuf;
use std::process::Command;

fn binary() -> PathBuf {
    if let Some(path) = std::env::var_os("FMINE_TEST_BINARY") {
        return PathBuf::from(path);
    }
    let executable = std::env::current_exe().expect("current T executable");
    let profile = executable
        .ancestors()
        .find(|path| {
            path.file_name()
                .is_some_and(|name| name == "deps" || name == "build")
        })
        .and_then(std::path::Path::parent)
        .expect("Cargo profile directory; set FMINE_TEST_BINARY for another layout");
    profile.join(format!("fmine{}", std::env::consts::EXE_SUFFIX))
}

fn oracle() -> Command {
    let mut command = Command::new("python3");
    command
        .arg("-I")
        .arg(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("testdata/decision_card_oracle.py"))
        .arg(binary());
    command
}

#[test]
fn finite_decision_known_answers_pass_the_independent_python_oracle() {
    let output = oracle()
        .output()
        .expect("execute finite decision scenarios");
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn executive_one_pager_matches_the_expansion_golden_and_preserves_policy_constraints() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let brief = |model: &str| {
        let result = Command::new(binary())
            .current_dir(&root)
            .args(["decide", "card", model, "--workspace", ".", "--brief"])
            .output()
            .expect("run executive view");
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stdout)
        );
        String::from_utf8(result.stdout).expect("Markdown")
    };
    // 0.6 × 300m + 0.4 × (−200m) = 100m; 100m − 55m = 45m.
    // The expected Claim id predates the brief and comes from the unchanged analyst card.
    assert_eq!(
        brief("tests/decisions/expansion.toml"),
        concat!(
            include_str!("../../../tests/decisions/expansion_brief.md"),
            "\n"
        )
    );
    let constrained = brief("tests/decisions/constrained_testwork.toml");
    assert!(
        constrained.contains("## Choices within the downside limit")
            && constrained.contains("optimized test policy")
            && constrained.contains("28.50")
    );
    let robust = brief("tests/decisions/wasserstein_information.toml");
    assert!(
        robust.contains("Including fixed test responses")
            && robust.contains("**Stressed state weights.**")
    );
    for args in [
        &["decide", "card", "unused", "--brief", "--json"][..],
        &["decide", "card", "unused", "--format", "json", "--brief"],
        &["decide", "program", "unused", "--brief"],
        &["decide", "card", "unused", "--brief", "--brief"],
    ] {
        let owned: Vec<_> = args.iter().map(|s| (*s).to_owned()).collect();
        assert!(fmine_cli::parse_args(&owned, &root).is_err());
    }
}

fn rendering_parity(mut oracle: Command) {
    let fixture = oracle
        .arg("--fixture-only")
        .output()
        .expect("create declared decision fixture");
    assert!(
        fixture.status.success(),
        "{}",
        String::from_utf8_lossy(&fixture.stderr)
    );
    let root = PathBuf::from(
        String::from_utf8(fixture.stdout)
            .expect("fixture path is UTF-8")
            .trim(),
    );
    let args = ["decide", "card", "decision-model.toml"].map(str::to_owned);
    let (expected, format) = fmine_cli::invoke(&args, &root);
    assert_eq!(format, OutputFormat::Json);
    assert_eq!(expected.exit_code, 0, "{}", expected.to_json());
    assert!(expected.refusal.is_none());
    assert_eq!(
        expected.to_json(),
        fmine_cli::invoke(&args, &root).0.to_json()
    );
    for (format, text) in [
        ("json", expected.to_json()),
        (
            "markdown",
            expected
                .rendered_markdown
                .clone()
                .expect("decision Markdown"),
        ),
    ] {
        let output = Command::new(binary())
            .args(&args)
            .args(["--format", format, "--workspace"])
            .arg(&root)
            .output()
            .expect("execute actual decision command");
        assert_eq!(output.status.code(), Some(0));
        assert_eq!(
            String::from_utf8(output.stdout).expect("rendering is UTF-8"),
            format!("{text}\n")
        );
    }
}

#[test]
fn real_binary_and_library_keep_identical_json_and_markdown() {
    rendering_parity(oracle());
}

#[test]
fn downside_brief_matches_the_hand_calculated_fixed_and_optimized_choices() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let markdown = |model: &str| {
        let output = Command::new(binary())
            .current_dir(&root)
            .args(["decide", "card", model, "--format", "markdown"])
            .output()
            .expect("execute the decision command");
        assert!(output.status.success(), "{:?}", output.stderr);
        String::from_utf8(output.stdout).expect("Markdown is UTF-8")
    };
    let fixed = markdown("tests/decisions/downside_limit.toml");
    let (_, body) = fixed.split_once("\n\n").expect("decision title");
    let (brief, _) = body
        .split_once("\nDraft comparison")
        .expect("scope below brief");
    assert_eq!(
        brief,
        include_str!("../../../tests/decisions/downside_brief.md")
    );
    let optimized = markdown("tests/decisions/constrained_testwork.toml");
    let (brief, _) = optimized
        .split_once("\nDraft comparison")
        .expect("scope below brief");
    assert!(brief.contains("| Nominal | hold (act now) | 0 | 34 |"));
    assert!(brief.contains(
        "| Nominal with optimized test responses | testwork (optimized test policy) | 28.50 | 5.50 |"
    ));
}

#[test]
fn information_policy_keeps_identical_library_and_binary_rendering() {
    let mut oracle = oracle();
    oracle.arg("--with-information");
    rendering_parity(oracle);
}

#[test]
fn flip_margins_pass_independent_exact_witness_and_boundary_checks() {
    let script =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("testdata/decision_sensitivity_oracle.py");
    let output = Command::new("python3")
        .arg("-I")
        .arg(script)
        .arg(binary())
        .output()
        .expect("execute exact decision sensitivity scenarios");
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn information_policy_tail_risk_frontier_passes_independent_fraction_checks() {
    let script =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("testdata/decision_policy_risk_oracle.py");
    let output = Command::new("python3")
        .arg("-I")
        .arg(script)
        .arg(binary())
        .output()
        .expect("execute exact information-policy risk scenarios");
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
