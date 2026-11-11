//! DESIGN (BD-229): public capital model, independent transport witness checks and CLI parity.
//! Determinism: Exact

#![cfg(test)]
#![allow(clippy::expect_used)]

use std::path::{Path, PathBuf};
use std::process::Command;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("workspace root")
}

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
        .and_then(Path::parent)
        .expect("Cargo profile directory; set FMINE_TEST_BINARY for another layout");
    profile.join(format!("fmine{}", std::env::consts::EXE_SUFFIX))
}

#[test]
fn capital_robustness_source_example_preserves_library_and_binary_rendering() {
    let root = root();
    let args = ["decide", "program", "tests/capital/wasserstein.toml"].map(str::to_owned);
    let (envelope, format) = fmine_cli::invoke(&args, &root);
    assert_eq!(format, fmine_cli::OutputFormat::Json);
    assert_eq!(envelope.exit_code, 0, "{}", envelope.to_json());
    let json = envelope.to_json();
    assert_eq!(json, fmine_cli::invoke(&args, &root).0.to_json());
    for expected in [
        "\"scope\":\"nominally_feasible_static_bundles\"",
        "\"objective_unit\":\"USD\"",
        "\"stressed_tail_loss_feasibility_certified\":false",
        "\"maximin_mean_winners\":[\"bundle_02\"]",
        "\"minimax_expected_regret_winners\":[\"bundle_01\"]",
        "\"worst_case_mean\":\"4000000/1\"",
        "\"worst_case_expected_regret\":\"10000000/1\"",
    ] {
        assert!(json.contains(expected), "missing {expected}");
    }
    let markdown = envelope
        .rendered_markdown
        .as_deref()
        .expect("capital robustness Markdown");
    for expected in [
        "## Capital scenario-weight robustness",
        "### Declared dimensionless transport geometry",
        "signed loss certificate",
        "same-state regret certificate",
        "The nominal tail-loss limits have not been certified under the stressed laws.",
    ] {
        assert!(markdown.contains(expected), "missing {expected}");
    }
    for (format, expected) in [("json", json.as_str()), ("markdown", markdown)] {
        let output = Command::new(binary())
            .args(&args)
            .args(["--format", format, "--workspace"])
            .arg(&root)
            .output()
            .expect("execute actual capital program");
        assert_eq!(output.status.code(), Some(0));
        assert_eq!(
            String::from_utf8(output.stdout).expect("UTF-8 rendering"),
            format!("{expected}\n")
        );
    }
}

#[test]
fn capital_robustness_matches_independent_fraction_vertices_and_transport_duals() {
    let output = Command::new("python3")
        .arg("-I")
        .arg(root().join("crates/xtask/testdata/capital_robustness_oracle.py"))
        .arg(binary())
        .output()
        .expect("execute independent exact transport oracle");
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn adaptive_capital_robustness_matches_independent_policy_and_transport_oracles() {
    let output = Command::new("python3")
        .arg("-I")
        .arg(root().join("crates/xtask/testdata/capital_robustness_oracle.py"))
        .arg(binary())
        .arg("--adaptive")
        .output()
        .expect("execute independent adaptive transport oracle");
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn adaptive_capital_robustness_preserves_actual_cli_json_and_markdown() {
    let fixture = Command::new("python3")
        .arg("-I")
        .arg(root().join("crates/xtask/testdata/capital_robustness_oracle.py"))
        .arg(binary())
        .arg("--adaptive-fixture")
        .output()
        .expect("create synthetic adaptive stress input");
    assert!(
        fixture.status.success(),
        "{}",
        String::from_utf8_lossy(&fixture.stderr)
    );
    let root = PathBuf::from(
        String::from_utf8(fixture.stdout)
            .expect("UTF-8 fixture path")
            .trim(),
    );
    let args = ["decide", "program", "capital-program.toml"].map(str::to_owned);
    let (envelope, format) = fmine_cli::invoke(&args, &root);
    assert_eq!(format, fmine_cli::OutputFormat::Json);
    assert_eq!(envelope.exit_code, 0, "{}", envelope.to_json());
    let json = envelope.to_json();
    let markdown = envelope
        .rendered_markdown
        .as_deref()
        .expect("adaptive stress Markdown");
    for field in [
        "\"scope\":\"nominally_feasible_adaptive_policies\"",
        "\"scope\":\"nominally_feasible_static_bundles\"",
        "\"worst_case_mean\":\"15/1\"",
        "\"worst_case_expected_regret\":\"15/2\"",
    ] {
        assert!(json.contains(field), "missing {field}");
    }
    assert!(markdown.contains("## Capital policy scenario-weight robustness"));
    for (format, expected) in [("json", json.as_str()), ("markdown", markdown)] {
        let output = Command::new(binary())
            .args(&args)
            .args(["--format", format, "--workspace"])
            .arg(&root)
            .output()
            .expect("execute actual adaptive stress command");
        assert_eq!(output.status.code(), Some(0));
        assert_eq!(
            String::from_utf8(output.stdout).expect("UTF-8 report"),
            format!("{expected}\n")
        );
    }
}
