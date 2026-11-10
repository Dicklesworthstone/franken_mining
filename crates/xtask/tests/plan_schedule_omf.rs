//! Synthetic OMF schedules through the actual CLI, checked by independent exact enumeration.
#![cfg(test)]

use std::path::{Path, PathBuf};
use std::process::Command;

fn oracle(case: &str) {
    let binary = std::env::var_os("FMINE_TEST_BINARY")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            std::env::current_exe()
                .unwrap()
                .ancestors()
                .find(|p| p.file_name().is_some_and(|n| n == "deps" || n == "build"))
                .and_then(Path::parent)
                .unwrap()
                .join(format!("fmine{}", std::env::consts::EXE_SUFFIX))
        });
    let output = Command::new("python3")
        .arg("-I")
        .arg(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("testdata/plan_schedule_omf_oracle.py"))
        .arg(binary)
        .arg(case)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{case}:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn omf_schedules_and_certificates_match_independent_exhaustive_enumeration() {
    oracle("schedules");
}

#[test]
fn omf_candidates_are_checked_in_original_order_and_never_trusted_when_infeasible() {
    oracle("candidates");
}

#[test]
fn omf_schedule_specs_and_attributes_refuse_unsupported_or_ambiguous_declarations() {
    oracle("refusals");
}

#[test]
fn omf_schedule_binary_preserves_sources_formats_and_strict_command_modes() {
    oracle("command");
}
