//! Independent entity funding arithmetic and complete library/binary reporting parity.
//! Determinism: Exact (declared input bytes, Fraction reference and rational output).

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
        .arg(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("testdata/capital_entity_oracle.py"))
        .arg(binary());
    command
}

#[test]
fn capital_entity_funding_matches_independent_fraction_ledgers_and_populations() {
    let output = oracle().output().expect("execute entity funding oracle");
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
        .expect("entity funding Markdown");
    for (format, expected) in [("json", json.as_str()), ("markdown", markdown)] {
        let output = Command::new(binary())
            .args(&args)
            .args(["--format", format, "--workspace"])
            .arg(root)
            .output()
            .expect("execute actual entity funding command");
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
        "\"net_adaptive_value_per_share\":\"9/20\"",
        "\"net_adaptive_value\":null",
        "\"net_adaptive_value_status\":\"available\"",
        "\"objective_unit\":\"USD/share\"",
        "\"statistics_unit\":\"USD\"",
        "\"entity_funding\":{",
        "\"objective_entity\":\"parent\"",
        "\"phase\":\"pre_distribution\"",
        "\"kind\":\"entity_cash_shortfall\"",
        "\"static_comparison\":{\"schema\":\"fmine.capital-program.v1\"",
    ] {
        assert!(envelope.payload_json.contains(field), "missing {field}");
    }
    let markdown = envelope
        .rendered_markdown
        .as_deref()
        .expect("entity funding Markdown");
    for section in [
        "## Entity cash, capital calls and distributions",
        "### Declared entities",
        "### Declared entity floors",
        "### Declared native candidate cash flows",
        "### Declared ownership distributions",
        "### Declared distribution gates and caps",
        "**Cash conservation across the modeled graph**",
    ] {
        assert!(markdown.contains(section), "missing {section}");
    }
    assert_eq!(markdown.matches("\n## Policy `").count(), 6);
    assert_eq!(
        markdown.matches("### Entity cash ledger: Policy ").count(),
        6
    );
    assert_eq!(
        markdown.matches("### Entity cash ledger: Bundle ").count(),
        4
    );
    assert!(markdown.contains("| Net adaptive value (USD/share) | 9/20 | 9/20 |"));
    // The JV pays 60 to Parent and 40 outside; the whole 100 is never credited to Parent.
    assert!(
        markdown.contains("| jv | second | 120/1 | 20/1 | 100/1 | 100/1 | 60/1 | 40/1 | 20/1 |")
    );
    assert!(markdown.contains(
        "| second | 2030-08-31 | 40/1 | 100/1 | 0/1 | 0/1 | 60/1 | 40/1 | 100/1 | 0/1 |"
    ));
    // Good can spend next period. Bad abandons and retains its study's NAV loss.
    assert!(markdown.contains(
        "| parent | third | 2030-12-31 | Closing | 80/1 | 30/1 | 20/1 | 10/1 | floor met |"
    ));
    assert!(markdown.contains(
        "| parent | third | 2030-12-31 | Closing | 20/1 | 20/1 | 20/1 | 0/1 | floor met |"
    ));
    assert!(markdown.contains(
        "| good | 1000/1 | 95/1 | 1095/1 | 100/1 | 0/1 | 100/1 | 10/1 | 219/20 | 19/20 |"
    ));
    assert!(
        markdown.contains(
            "| bad | 1000/1 | -5/1 | 995/1 | 100/1 | 0/1 | 100/1 | 10/1 | 199/20 | -1/20 |"
        )
    );
    // A fixed build after Bad retains its own rejected cash trajectory in the full comparison.
    assert!(markdown.contains(
        "| parent | third | 2030-12-31 | Before distributions | 20/1 | -30/1 | 20/1 | -50/1 | floor breach |"
    ));
}

#[test]
fn entity_funding_library_and_binary_render_all_static_and_adaptive_ledgers_identically() {
    let fixture = oracle()
        .arg("--fixture-only")
        .output()
        .expect("create entity funding fixture");
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
fn checked_in_entity_funding_example_runs_and_preserves_its_source_bytes() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crates directory")
        .parent()
        .expect("workspace directory");
    let model = "tests/capital/entity_funding.toml";
    let before = std::fs::read(root.join(model)).expect("checked-in entity funding example");
    let envelope = render_parity(root, model);
    assert_complete_entity_policy(&envelope);
    assert!(envelope.payload_json.contains(&format!(
        "\"sha256\":\"{}\",\"bytes\":{}}}",
        sha256(&before).to_hex(),
        before.len()
    )));
    assert_eq!(
        std::fs::read(root.join(model)).expect("unchanged entity funding example"),
        before
    );
}
