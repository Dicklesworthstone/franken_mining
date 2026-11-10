//! Synthetic OMF revenue-factor shells checked against independently enumerated closed sets.
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
        .arg(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("testdata/plan_shells_omf_oracle.py"))
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
fn omf_shell_optima_nesting_and_first_entry_match_independent_closed_set_enumeration() {
    oracle("shells");
}

#[test]
fn omf_shell_sources_mutations_formats_and_robot_docs_remain_bound_to_real_bytes() {
    oracle("sources");
}

#[test]
fn omf_shells_refuse_ambiguous_units_factors_and_unsupported_source_declarations() {
    oracle("refusals");
}

#[test]
fn omf_shell_command_modes_and_bounded_inputs_are_strict() {
    oracle("command");
}
