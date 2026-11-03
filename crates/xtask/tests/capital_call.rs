//! Independent subscription arithmetic and source-bound capital-call CLI parity.
//! Determinism: Exact (Fraction reference; complete static and adaptive cash ledgers).

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
        .arg(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("testdata/capital_call_oracle.py"))
        .arg(binary());
    command
}

#[test]
fn capital_call_ownership_cash_and_policies_match_independent_fractions() {
    let output = oracle().output().expect("execute capital-call oracle");
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
        .expect("capital-call Markdown");
    for (format, expected) in [("json", json.as_str()), ("markdown", markdown)] {
        let output = Command::new(binary())
            .args(&args)
            .args(["--format", format, "--workspace"])
            .arg(root)
            .output()
            .expect("execute actual capital-call command");
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
    envelope
}

fn assert_complete_subscription_policy(envelope: &fmine_cli::Envelope) {
    for field in [
        "\"mode\":\"declared_two_stage_capital_comparison\"",
        "\"schema\":\"fmine.capital-recourse.v1\"",
        "\"method\":\"pre_money_subscription\"",
        "\"cash_flow_convention\":\"native_excludes_modeled_capital_calls\"",
        "\"net_adaptive_value_per_share\":\"9/20\"",
        "\"per_share_tail_loss\":\"1/20\"",
        "\"call_id\":\"study_obligation\"",
        "\"call_id\":\"build_subscription\"",
        "\"parent_share_after\":\"7/11\"",
        "\"parent_share_after\":\"13/21\"",
        "\"parent_share_after\":\"17/21\"",
        "\"parent_receipt\":\"130/1\"",
        "\"external_distribution\":\"80/1\"",
        "\"ownership_residual\":\"0/1\"",
        "\"capital_call\":{\"parent_in\":\"0/1\",\"parent_out\":\"70/1\",\"external_in\":\"0/1\"}",
        "\"capital_call\":{\"parent_in\":\"70/1\",\"parent_out\":\"0/1\",\"external_in\":\"40/1\"}",
        "\"kind\":\"entity_cash_shortfall\"",
        "\"static_comparison\":{\"schema\":\"fmine.capital-program.v1\"",
    ] {
        assert!(envelope.payload_json.contains(field), "missing {field}");
    }
    let markdown = envelope
        .rendered_markdown
        .as_deref()
        .expect("capital-call Markdown");
    assert_eq!(markdown.matches("\n## Policy `").count(), 6);
    assert_eq!(
        markdown.matches("### Entity cash ledger: Policy ").count(),
        6
    );
    assert_eq!(
        markdown.matches("### Entity cash ledger: Bundle ").count(),
        4
    );
    for heading in [
        "### Declared JV subscriptions",
        "**JV subscription activity**",
        "**Subscription funding (USD)**",
        "**Ownership after each call**",
        "**Subscription cash by checkpoint (USD)**",
        "**Subscription cash conservation components (USD)**",
    ] {
        assert!(markdown.contains(heading), "missing {heading}");
    }
    for row in [
        "| study_obligation | 100/1 | 10/1 | 10/1 | 0/1 | 10/1 | 0/1 | 0/1 |",
        "| build_subscription | 110/1 | 100/1 | 100/1 | 0/1 | 0/1 | 0/1 | 0/1 |",
        "| study_obligation | 3/5 | 7/11 | 4/11 | 0/1 |",
        "| build_subscription | 7/11 | 13/21 | 8/21 | 0/1 |",
        "| build_subscription | 7/11 | 7/11 | 4/11 | 0/1 |",
        "| good | 1000/1 | 95/1 | 1095/1 | 100/1 | 0/1 | 100/1 | 10/1 | 219/20 | 19/20 |",
        "| bad | 1000/1 | -5/1 | 995/1 | 100/1 | 0/1 | 100/1 | 10/1 | 199/20 | -1/20 |",
        "| Net adaptive value (USD/share) | 9/20 | 9/20 |",
    ] {
        assert!(markdown.contains(row), "missing hand-calculated row {row}");
    }
}

#[test]
fn capital_call_library_and_binary_preserve_every_static_and_adaptive_path() {
    let fixture = oracle()
        .arg("--fixture-only")
        .output()
        .expect("create independent capital-call fixture");
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
    assert_complete_subscription_policy(&render_parity(&root, "capital-program.toml"));
}

#[test]
fn checked_in_capital_call_example_runs_without_changing_source_bytes() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crates directory")
        .parent()
        .expect("workspace directory");
    let model = "tests/capital/capital_calls.toml";
    let before = std::fs::read(root.join(model)).expect("checked-in capital-call example");
    let envelope = render_parity(root, model);
    assert_complete_subscription_policy(&envelope);
    assert!(envelope.payload_json.contains(&format!(
        "\"sha256\":\"{}\",\"bytes\":{}}}",
        sha256(&before).to_hex(),
        before.len()
    )));
    assert_eq!(
        std::fs::read(root.join(model)).expect("unchanged capital-call example"),
        before
    );
}
