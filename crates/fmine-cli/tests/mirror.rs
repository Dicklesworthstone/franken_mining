//! BD-346 executable smoke and typed selection guard; the independent source matrix lives in T.

#![cfg(test)]
#![allow(clippy::expect_used)]

use std::path::Path;
use std::process::Command;

#[test]
fn source_bound_matrix_passes_independent_python_oracle() {
    let output = Command::new("python3")
        .args([
            "-I",
            concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../xtask/testdata/mirror_oracle.py"
            ),
            "--binary",
            env!("CARGO_BIN_EXE_fmine"),
        ])
        .output()
        .expect("run T-layer mirror source matrix against the real executable");
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn binary(arguments: &[&str]) -> String {
    let output = Command::new(env!("CARGO_BIN_EXE_fmine"))
        .args(arguments)
        .output()
        .expect("run mirror executable");
    assert!(output.status.success(), "{:?}", output);
    String::from_utf8(output.stdout).expect("mirror emits UTF-8")
}

#[test]
fn synthetic_binary_emits_readable_markdown() {
    // The T-layer matrix independently parses the synthetic JSON and checks every authority/evidence flag.
    let markdown = binary(&["mirror", "--synthetic", "--format", "markdown"]);
    for expected in ["Alpha.pdf", "Beta.pdf", "615", "385", "invented-mine"] {
        assert!(
            markdown.contains(expected),
            "missing {expected}: {markdown}"
        );
    }
}

#[test]
fn typed_execute_rejects_reserved_synthetic_issuer() {
    use fmine_cli::mirror::{Command as MirrorCommand, execute, parse};
    let args: Vec<String> = [
        "mirror",
        "--asset",
        "example-jv",
        "--period",
        "2025",
        "--as-of",
        "2026-02-01",
        "--left-issuer",
        "alpha",
        "--left-document",
        "Left.pdf",
        "--right-issuer",
        "beta",
        "--right-document",
        "Right.pdf",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();
    let MirrorCommand::Public(mut selection) = parse(&args).expect("valid explicit selection")
    else {
        panic!("explicit reporters must select the public path");
    };
    selection.reporters[0].issuer = "synthetic".into();
    let report = execute(
        &MirrorCommand::Public(selection),
        Path::new("unused-workspace"),
    );
    assert_eq!(
        report.refusal.as_ref().expect("typed refusal").code,
        "FMINE-REF-107"
    );
    assert_ne!(report.exit_code, 0);
    assert!(!report.payload_json.contains("\"status\":\"consistent\""));
}
