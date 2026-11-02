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
