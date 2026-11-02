//! Independent exact capital-program enumeration and real-binary/library rendering parity.

#![cfg(test)]
#![allow(clippy::expect_used)]

use std::path::{Path, PathBuf};
use std::process::Command;

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
        .arg("-I")
        .arg(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("testdata/capital_program_oracle.py"))
        .arg(binary());
    command
}

#[test]
fn capital_program_matches_independent_fraction_enumeration() {
    let output = oracle().output().expect("execute capital program oracle");
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn assert_render_parity(fixture_argument: &str) -> fmine_cli::Envelope {
    let fixture = oracle()
        .arg(fixture_argument)
        .output()
        .expect("create declared capital-program fixture");
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
    let args = ["decide", "program", "capital-program.toml"].map(str::to_owned);
    let (envelope, format) = fmine_cli::invoke(&args, &root);
    assert_eq!(format, fmine_cli::OutputFormat::Json);
    assert_eq!(envelope.exit_code, 0, "{}", envelope.to_json());
    assert_eq!(
        envelope.to_json(),
        fmine_cli::invoke(&args, &root).0.to_json()
    );
    let json = envelope.to_json();
    let markdown = envelope
        .rendered_markdown
        .as_deref()
        .expect("capital program Markdown");
    for (format, expected) in [("json", json.as_str()), ("markdown", markdown)] {
        let output = Command::new(binary())
            .args(&args)
            .args(["--format", format, "--workspace"])
            .arg(&root)
            .output()
            .expect("execute actual capital-program command");
        assert_eq!(output.status.code(), Some(0));
        assert_eq!(
            String::from_utf8(output.stdout).expect("UTF-8 rendering"),
            format!("{expected}\n")
        );
    }
    envelope
}

#[test]
fn capital_program_library_and_binary_render_identically() {
    let _ = assert_render_parity("--fixture-only");
}

#[test]
fn capital_program_cash_schedule_renders_every_bundle_timeline_in_library_and_binary() {
    let envelope = assert_render_parity("--schedule-fixture-only");
    assert!(
        envelope
            .payload_json
            .contains("\"feasible_winners\":[\"bundle_03\"]")
    );
    let markdown = envelope.rendered_markdown.expect("cash schedule Markdown");
    for section in [
        "## Funding at declared cash checkpoints",
        "### Declared cash periods and floors",
        "### Declared opening cash",
        "### Declared baseline cash flows",
        "### Declared candidate cash flows",
    ] {
        assert!(markdown.contains(section), "missing {section}");
    }
    assert_eq!(markdown.matches("### Cash timeline for `").count(), 4);
    assert!(markdown.contains("| operation | bridge | low | -68/1 |"));
    assert!(
        markdown.contains("cash-floor shortfall in low at construction (2030-06-30): 10/1 USD")
    );
    // Holding later recovers, but its construction-end breach remains visible.
    assert!(markdown.contains(
        "| low | construction | 2030-06-30 | 100/1 | -90/1 | 0/1 | 10/1 | 20/1 | -10/1 | floor breach |"
    ));
    assert!(markdown.contains(
        "| low | operation | 2030-12-31 | 10/1 | 80/1 | 0/1 | 90/1 | 20/1 | 70/1 | floor met |"
    ));
    // The financed plant carries its exact floor balance into the second period.
    assert!(markdown.contains(
        "| low | construction | 2030-06-30 | 100/1 | -90/1 | 10/1 | 20/1 | 20/1 | 0/1 | floor met |"
    ));
    assert!(markdown.contains(
        "| low | operation | 2030-12-31 | 20/1 | 80/1 | -48/1 | 52/1 | 20/1 | 32/1 | floor met |"
    ));
}

#[test]
fn capital_program_equity_renders_exact_per_share_objective_and_total_nav_separately() {
    let envelope = assert_render_parity("--equity-fixture-only");
    for field in [
        "\"feasible_winners\":[\"bundle_05\"]",
        "\"objective_unit\":\"USD/share\"",
        "\"statistics_unit\":\"USD\"",
        "\"per_share_statistics_unit\":\"USD/share\"",
        "\"expected_value_cost_of_risk_limit\":null",
    ] {
        assert!(envelope.payload_json.contains(field), "missing {field}");
    }
    let markdown = envelope.rendered_markdown.expect("equity Markdown");
    for section in [
        "## Per-share objective and ownership",
        "### Declared baseline equity NAV",
        "### Declared candidate share changes",
        "| Baseline NAV (USD) | NAV change (USD) | Ending NAV (USD) |",
    ] {
        assert!(markdown.contains(section), "missing {section}");
    }
    assert_eq!(markdown.matches("### Equity and shares for `").count(), 8);
    assert!(markdown.contains("| project | low | 0/1 |"));
    // Buyback: 920 / 90 - 1000 / 100 = 2/9, while total equity NAV falls by 80.
    assert!(
        markdown.contains(
            "| low | 1000/1 | -80/1 | 920/1 | 100/1 | -10/1 | 90/1 | 10/1 | 92/9 | 2/9 |"
        )
    );
    // Add the project before dividing: 1020 / 90 - 10 = 4/3.
    assert!(
        markdown.contains(
            "| low | 1000/1 | 20/1 | 1020/1 | 100/1 | -10/1 | 90/1 | 10/1 | 34/3 | 4/3 |"
        )
    );
    // Issuance raises total NAV by 160 but dilutes each existing share by 1/3.
    assert!(
        markdown.contains(
            "| low | 1000/1 | 160/1 | 1160/1 | 100/1 | 20/1 | 120/1 | 10/1 | 29/3 | -1/3 |"
        )
    );
}
