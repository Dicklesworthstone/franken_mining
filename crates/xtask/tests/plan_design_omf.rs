//! Synthetic OMF design quantities checked by analytic box intersections and exact closed-set enumeration.
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
        .arg(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("testdata/plan_design_omf_oracle.py"))
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
fn omf_design_enclosures_match_analytic_boxes_with_signed_values_and_negative_loss() {
    oracle("geometry");
}

#[test]
fn omf_design_source_changes_formats_and_robot_docs_remain_bound_to_real_bytes() {
    oracle("sources");
}

#[test]
fn omf_design_refuses_unsupported_declarations_units_and_invalid_surfaces() {
    oracle("refusals");
}

#[test]
fn omf_design_refuses_geometry_work_before_subdivision_and_mesh_certification() {
    oracle("work");
}
