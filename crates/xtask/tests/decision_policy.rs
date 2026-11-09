//! Actual CLI policies checked against an independent exact finite-law oracle.
//! Determinism: Exact

#![cfg(test)]

use std::path::PathBuf;
use std::process::Command;

#[test]
fn constrained_testwork_matches_independent_fraction_enumeration() {
    let binary = std::env::var_os("FMINE_TEST_BINARY")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            let executable = std::env::current_exe().unwrap();
            executable
                .ancestors()
                .find(|p| p.file_name().is_some_and(|n| n == "deps" || n == "build"))
                .and_then(std::path::Path::parent)
                .unwrap()
                .join(format!("fmine{}", std::env::consts::EXE_SUFFIX))
        });
    let output = Command::new("python3")
        .arg("-I")
        .arg(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("testdata/decision_policy_oracle.py"))
        .arg(binary)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
