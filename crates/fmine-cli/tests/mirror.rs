//! BD-346 executable smoke and typed selection guard; the independent source matrix lives in T.

#![cfg(test)]
#![allow(clippy::expect_used)]

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
