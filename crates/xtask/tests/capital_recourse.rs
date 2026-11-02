//! Independent two-stage policy arithmetic and source-complete real-binary rendering.

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
        .arg(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("testdata/capital_recourse_oracle.py"))
        .arg(binary());
    command
}

#[test]
fn capital_recourse_matches_independent_fraction_policy_enumeration() {
    let output = oracle().output().expect("execute capital recourse oracle");
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
        .expect("capital recourse Markdown");
    for (format, expected) in [("json", json.as_str()), ("markdown", markdown)] {
        let output = Command::new(binary())
            .args(&args)
            .args(["--format", format, "--workspace"])
            .arg(root)
            .output()
            .expect("execute actual capital recourse command");
        assert_eq!(
            output.status.code(),
            Some(0),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            String::from_utf8(output.stdout).expect("UTF-8 rendering"),
            format!("{expected}\n")
        );
    }
    envelope
}

fn assert_known_equity_policy(envelope: &fmine_cli::Envelope) {
    for field in [
        "\"mode\":\"declared_two_stage_capital_comparison\"",
        "\"schema\":\"fmine.capital-recourse.v1\"",
        "\"net_adaptive_value_per_share\":\"19/120\"",
        "\"net_adaptive_value\":null",
        "\"net_adaptive_value_status\":\"available\"",
        "\"objective_unit\":\"USD/share\"",
        "\"statistics_unit\":\"USD\"",
        "\"static_comparison\":{\"schema\":\"fmine.capital-program.v1\"",
    ] {
        assert!(envelope.payload_json.contains(field), "missing {field}");
    }
    let markdown = envelope
        .rendered_markdown
        .as_deref()
        .expect("capital recourse Markdown");
    for section in [
        "# Two-stage capital policy comparison",
        "## Adaptive and precommitted comparison",
        "## Declared information and stages",
        "## Complete policy outcomes in USD",
        "## Complete policy outcomes in USD/share",
        "## Complete precommitted comparison and program inputs",
    ] {
        assert!(markdown.contains(section), "missing {section}");
    }
    assert_eq!(markdown.matches("\n## Policy `").count(), 20);
    assert_eq!(
        markdown
            .matches("### Continuation by information set")
            .count(),
        20
    );
    assert_eq!(
        markdown
            .matches("### Cash at every declared checkpoint")
            .count(),
        20
    );
    assert_eq!(
        markdown
            .matches("### Equity and actual shares in every state")
            .count(),
        20
    );
    assert_eq!(markdown.matches("### Cash timeline for `").count(), 8);
    assert!(markdown.contains("| Net adaptive value (USD/share) | 19/120 | 19/120 |"));
    // Successful project: transaction proceeds and construction spending share one cash path.
    assert!(markdown.contains(
        "| good | operation | 2030-12-31 | 20/1 | 0/1 | 60/1 | 80/1 | 20/1 | 60/1 | floor met |"
    ));
    assert!(markdown.contains(
        "| good | 1000/1 | 250/1 | 1250/1 | 100/1 | 20/1 | 120/1 | 10/1 | 125/12 | 5/12 |"
    ));
    // Abandonment retains the paid study's NAV loss and its earlier cash expenditure.
    assert!(markdown.contains(
        "| bad | operation | 2030-12-31 | 20/1 | 0/1 | 0/1 | 20/1 | 20/1 | 0/1 | floor met |"
    ));
    assert!(
        markdown.contains(
            "| bad | 1000/1 | -10/1 | 990/1 | 100/1 | 0/1 | 100/1 | 10/1 | 99/10 | -1/10 |"
        )
    );
    assert!(markdown.contains("Not acquired: common continuation"));
}

#[test]
fn capital_recourse_library_and_binary_render_every_policy_identically() {
    let fixture = oracle()
        .arg("--fixture-only")
        .output()
        .expect("create recourse fixture");
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
    assert_known_equity_policy(&render_parity(&root, "capital-program.toml"));
}

#[test]
fn checked_in_recourse_example_runs_and_binds_its_original_source_bytes() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crates directory")
        .parent()
        .expect("workspace directory");
    let model = "tests/capital/recourse.toml";
    let before = std::fs::read(root.join(model)).expect("checked-in recourse example");
    let envelope = render_parity(root, model);
    assert_known_equity_policy(&envelope);
    assert!(envelope.payload_json.contains(&format!(
        "\"sha256\":\"{}\",\"bytes\":{}}}",
        sha256(&before).to_hex(),
        before.len()
    )));
    assert_eq!(
        std::fs::read(root.join(model)).expect("unchanged example"),
        before
    );
}
