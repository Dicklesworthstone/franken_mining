//! Independent entity-denominator arithmetic and complete source-bound library/binary parity.
//! Determinism: Exact (Fraction reference; all static and adaptive entity ledgers remain visible).

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
        .arg(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("testdata/capital_valuation_oracle.py"))
        .arg(binary());
    command
}

#[test]
fn capital_entity_valuations_match_independent_fraction_and_complete_policy_populations() {
    let output = oracle().output().expect("execute entity valuation oracle");
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
        .expect("entity valuation Markdown");
    for (format, expected) in [("json", json.as_str()), ("markdown", markdown)] {
        let output = Command::new(binary())
            .args(&args)
            .args(["--format", format, "--workspace"])
            .arg(root)
            .output()
            .expect("execute actual entity valuation command");
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

fn assert_complete_entity_policy(envelope: &fmine_cli::Envelope) {
    for field in [
        "\"mode\":\"declared_two_stage_capital_comparison\"",
        "\"schema\":\"fmine.capital-recourse.v1\"",
        "\"net_adaptive_value_per_share\":\"2/5\"",
        "\"unconstrained_net_adaptive_value_per_share\":\"7/10\"",
        "\"net_adaptive_value\":null",
        "\"net_adaptive_value_status\":\"available\"",
        "\"expected_per_share_cost_of_risk_limit\":\"3/10\"",
        "\"objective_unit\":\"USD/share\"",
        "\"statistics_unit\":\"USD\"",
        "\"entity_valuations\":[{",
        "\"entity\":\"carve-out\"",
        "\"per_share_tail_loss\":\"31/15\"",
        "\"mean\":\"89/220\"",
        "\"delta_nav_per_share\":\"10/11\"",
        "\"ending_shares\":\"66/1\"",
        "\"kind\":\"entity_cash_shortfall\"",
        "\"static_comparison\":{\"schema\":\"fmine.capital-program.v1\"",
    ] {
        assert!(envelope.payload_json.contains(field), "missing {field}");
    }
    let markdown = envelope
        .rendered_markdown
        .as_deref()
        .expect("entity valuation Markdown");
    for text in [
        "## Additional entity valuations and binding risk limits",
        "### Valuation declaration for carve-out / Invented ordinary shares",
        "The primary equity entity remains the sole selection objective.",
        "entity tail-loss cap exceeded",
        "entity tail-loss cap met",
        "unavailable: structurally rejected",
    ] {
        assert!(markdown.contains(text), "missing {text}");
    }
    assert_eq!(markdown.matches("\n## Policy `").count(), 20);
    assert_eq!(
        markdown.matches("### Entity cash ledger: Policy ").count(),
        20
    );
    assert_eq!(
        markdown.matches("### Entity cash ledger: Bundle ").count(),
        8
    );
    assert_eq!(
        markdown
            .matches(": carve-out / Invented ordinary shares valuation")
            .count(),
        28
    );
    // Independent known answers for both denominators, including the unchosen harmful deal.
    for row in [
        "| good | 600/1 | 120/1 | 720/1 | 60/1 | 6/1 | 66/1 | 10/1 | 120/11 | 10/11 |",
        "| bad | 600/1 | -6/1 | 594/1 | 60/1 | 0/1 | 60/1 | 10/1 | 99/10 | -1/10 |",
        "| good | 600/1 | 114/1 | 714/1 | 60/1 | 30/1 | 90/1 | 10/1 | 119/15 | -31/15 |",
        "| good | 1000/1 | 90/1 | 1090/1 | 100/1 | 0/1 | 100/1 | 10/1 | 109/10 | 9/10 |",
        "| bad | 1000/1 | -10/1 | 990/1 | 100/1 | 0/1 | 100/1 | 10/1 | 99/10 | -1/10 |",
        "| parent | third | 2030-12-31 | Closing | 50/1 | 30/1 | 20/1 | 10/1 | floor met |",
        "| parent | third | 2030-12-31 | Closing | 20/1 | 20/1 | 20/1 | 0/1 | floor met |",
        "| Net adaptive value (USD/share) | 7/10 | 2/5 |",
    ] {
        assert!(markdown.contains(row), "missing hand-calculated row {row}");
    }
}

#[test]
fn entity_valuation_library_and_binary_keep_every_static_and_adaptive_denominator() {
    let fixture = oracle()
        .arg("--fixture-only")
        .output()
        .expect("create independent entity valuation fixture");
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
    assert_complete_entity_policy(&render_parity(&root, "capital-program.toml"));
}

#[test]
fn checked_in_entity_valuation_example_runs_and_preserves_all_source_bytes() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crates directory")
        .parent()
        .expect("workspace directory");
    let model = "tests/capital/entity_valuation.toml";
    let before = std::fs::read(root.join(model)).expect("checked-in entity valuation example");
    let envelope = render_parity(root, model);
    assert_complete_entity_policy(&envelope);
    assert!(envelope.payload_json.contains(&format!(
        "\"sha256\":\"{}\",\"bytes\":{}}}",
        sha256(&before).to_hex(),
        before.len()
    )));
    assert_eq!(
        std::fs::read(root.join(model)).expect("unchanged entity valuation example"),
        before
    );
}
