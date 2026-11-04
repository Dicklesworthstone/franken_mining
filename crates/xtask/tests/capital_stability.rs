//! DESIGN: independent resampling comparisons and source-bound capital-stability CLI parity.
//! Determinism: Exact (seeded integer counts and independent Fraction arithmetic).

#![cfg(test)]
#![allow(clippy::expect_used)]

use std::path::{Path, PathBuf};
use std::process::Command;

use fmine_core::hash::sha256;

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

fn oracle() -> Command {
    let mut command = Command::new("python3");
    command
        .args(["-B", "-I"])
        .arg(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("testdata/capital_stability_oracle.py"))
        .arg(binary());
    command
}

#[test]
fn bootstrap_selections_and_all_risk_limits_match_independent_fractions() {
    let output = oracle().output().expect("execute capital-stability oracle");
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn render_parity(root: &Path, model: &str) -> fmine_cli::Envelope {
    let args = ["decide", "program", model].map(str::to_owned);
    let (envelope, format) = fmine_cli::invoke(&args, root);
    assert_eq!(format, fmine_cli::OutputFormat::Json);
    assert_eq!(envelope.exit_code, 0, "{}", envelope.to_json());
    let json = envelope.to_json();
    assert_eq!(json, fmine_cli::invoke(&args, root).0.to_json());
    let markdown = envelope
        .rendered_markdown
        .as_deref()
        .expect("capital-stability Markdown");
    for (format, expected) in [("json", json.as_str()), ("markdown", markdown)] {
        let output = Command::new(binary())
            .args(&args)
            .args(["--format", format, "--workspace"])
            .arg(root)
            .output()
            .expect("execute actual capital-stability command");
        assert_eq!(
            output.status.code(),
            Some(0),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            String::from_utf8(output.stdout).expect("UTF-8 report"),
            format!("{expected}\n")
        );
    }
    for field in [
        "\"selection_frequency_status\":\"conditional_weighted_bootstrap\"",
        "\"method\":\"weighted_multinomial\"",
        "\"seed\":\"229\"",
        "\"replicates\":200",
        "\"draws_per_replicate\":5",
        "\"state_counts\":",
        "\"risk_feasible_replicates\":",
        "\"co_winner_replicates\":",
        "\"nominal_runner_up\":",
        "\"definite_flip_frequency\":",
        "\"possible_flip_frequency\":",
        "\"candidate_frequencies\":",
        "\"frequency_lower\":",
        "\"frequency_upper\":",
    ] {
        assert!(envelope.payload_json.contains(field), "missing {field}");
    }
    assert!(markdown.contains("Selection stability"));
    assert!(markdown.contains("Synthetic conditional scenario-law resampling"));
    envelope
}

#[test]
fn adaptive_stability_preserves_library_binary_and_retained_static_reports() {
    let fixture = oracle()
        .arg("--fixture-only")
        .output()
        .expect("create independent adaptive stability fixture");
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
    let envelope = render_parity(&root, "capital-program.toml");
    for field in [
        "\"schema\":\"fmine.capital-recourse.v1\"",
        "\"static_comparison\":{\"schema\":\"fmine.capital-program.v1\"",
        "\"call_id\":\"study_obligation\"",
        "\"parent_share_after\":\"13/21\"",
        "\"parent_share_after\":\"7/11\"",
        "\"net_adaptive_value_per_share\":\"9/20\"",
    ] {
        assert!(envelope.payload_json.contains(field), "missing {field}");
    }
    assert_eq!(
        envelope
            .rendered_markdown
            .as_deref()
            .expect("adaptive Markdown")
            .matches("\n## Policy `")
            .count(),
        6
    );
}

#[test]
fn checked_in_fragile_choice_example_preserves_its_original_source_bytes() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crates directory")
        .parent()
        .expect("workspace directory");
    let model = "tests/capital/selection_stability.toml";
    let before = std::fs::read(root.join(model)).expect("checked-in stability example");
    let envelope = render_parity(root, model);
    for field in [
        "\"nominal_winners\":[\"bundle_01\"]",
        "\"nominal_runner_up\":[\"bundle_02\"]",
        "\"fragile\":true",
        "\"tail_loss\":\"5/1\"",
        "\"kind\":\"budget_exceeded\"",
    ] {
        assert!(envelope.payload_json.contains(field), "missing {field}");
    }
    assert!(envelope.payload_json.contains(&format!(
        "\"sha256\":\"{}\",\"bytes\":{}}}",
        sha256(&before).to_hex(),
        before.len()
    )));
    assert_eq!(
        std::fs::read(root.join(model)).expect("unchanged stability example"),
        before
    );
}
