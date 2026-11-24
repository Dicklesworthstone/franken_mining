//! Actual-binary capital-return controls on declared data and invented authenticated financial filings.
//! Determinism: Exact
#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

use fmine_cli::{OutputFormat, invoke};
use fmine_core::hash::sha256;
use fmine_io::omf::json::{Json, parse};

const WORKSHEET: &str = include_str!("../../../tests/metrics/capital_return.toml");
static NEXT: AtomicU64 = AtomicU64::new(0);

fn fixture() -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "fmine-capital-return-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("return.toml"), WORKSHEET).unwrap();
    root
}

fn args(values: &[&str]) -> Vec<String> {
    values.iter().map(|v| (*v).into()).collect()
}
fn binary(root: &Path, args: &[String]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_fmine"))
        .args(args)
        .arg("--workspace")
        .arg(root)
        .output()
        .unwrap()
}
fn field<'a>(value: &'a Json, keys: &[&str]) -> &'a Json {
    keys.iter().fold(value, |v, key| v.get(key).expect(key))
}

#[test]
fn worksheet_binary_returns_exact_known_answers_and_complete_claims_in_both_formats() {
    let root = fixture();
    let words = args(&["metrics", "roce", "--input", "return.toml"]);
    let (library, format) = invoke(&words, &root);
    assert_eq!(format, OutputFormat::Json);
    assert_eq!(library.exit_code, 0, "{}", library.to_json());
    let output = binary(&root, &words);
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        format!("{}\n", library.to_json())
    );
    assert_eq!(output.stdout, binary(&root, &words).stdout);
    let json = parse(&output.stdout).unwrap();
    let value = field(&json, &["payload", "receipt", "result", "value"]);
    assert_eq!(
        field(value, &["period_return_percent", "point"]).as_str(),
        Some("12")
    );
    assert_eq!(
        field(value, &["annualized_return_percent", "point"]).as_str(),
        Some("24")
    );
    assert_eq!(
        field(&json, &["payload", "receipt", "inputs"])
            .as_array()
            .unwrap()
            .len(),
        4
    );
    assert_eq!(
        field(&json, &["payload", "receipt", "result", "evidence"]).as_str(),
        Some("Assumed")
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("\"execution_authorized\":false"));
    let mut markdown = words.clone();
    markdown.extend(args(&["--format", "markdown"]));
    assert_eq!(
        String::from_utf8_lossy(&binary(&root, &markdown).stdout),
        format!("{}\n", library.rendered_markdown.unwrap())
    );
    fs::write(
        root.join("return.toml"),
        WORKSHEET.replace("amount = \"12\"", "amount = \"-7\""),
    )
    .unwrap();
    let changed = binary(&root, &words);
    assert_eq!(changed.status.code(), Some(0));
    let changed_json = parse(&changed.stdout).unwrap();
    assert_eq!(
        field(
            &changed_json,
            &[
                "payload",
                "receipt",
                "result",
                "value",
                "period_return_percent",
                "point"
            ]
        )
        .as_str(),
        Some("-7")
    );
    assert_ne!(
        field(&json, &["payload", "receipt", "digest"]),
        field(&changed_json, &["payload", "receipt", "digest"])
    );
}

fn write_filing(root: &Path, earnings: &str, readers: &[&str], excluded: bool) {
    for path in [
        "documents",
        "registries/sources",
        "registries/transcriptions",
    ] {
        fs::create_dir_all(root.join(path)).unwrap();
    }
    let pdf = format!(
        "%PDF-1.7 Invented financial filing: H1 earnings {earnings}; opening 80.0; closing 120.0. Not a real filing."
    );
    let hash = sha256(pdf.as_bytes()).to_hex();
    fs::write(root.join("documents/invented.pdf"), &pdf).unwrap();
    fs::write(root.join("registries/sources/roce_test.toml"),format!(
        "schema = \"fmine.sources.v1\"\nissuer = \"roce_test\"\ncorpus_dir = \"documents\"\nretrieved_utc = \"2026-08-02T00:00:00Z\"\nurl_prefix = \"https://example.invalid/\"\n\
        [[subject]]\nname = \"Invented company\"\n\
        [[document]]\nfile = \"invented.pdf\"\nkind = \"mda\"\nurl = \"https://example.invalid/invented.pdf\"\nurl_basis = \"W1 table\"\n\
        published_at = \"2026-08-01\"\npublished_at_basis = \"FACT: invented release\"\npages = 2\nbytes = {}\nsha256 = \"{hash}\"\n",pdf.len())).unwrap();
    let status = if excluded {
        "excluded;exclusion_reason=development project not producing"
    } else {
        "applicable"
    };
    let common = format!(
        "subject=invented_company;ownership=full;perimeter=continuing;accounting_basis=historical_cost;currency=USD;monetary_scale=1000000;definition=issuer_return;version=1;earnings_basis=adjusted operating earnings;tax_basis=after_tax;capital_employed_basis=net operating assets;capital_basis=opening_closing_mean;annualization=explicit;annualization_factor=2;annualization_basis=half_year_times_two;applicability={status};definition_page=2;definition_table=Definitions;definition_row=Capital return;definition_col=Basis"
    );
    let mut entries =
        String::from("schema = \"fmine.transcriptions.v1\"\nissuer = \"roce_test\"\n");
    for (role, token, date) in [
        ("earnings", earnings, None),
        ("opening", "80.0", Some("2025-12-31")),
        ("closing", "120.0", Some("2026-06-30")),
    ] {
        let basis = date.map_or_else(
            || common.clone(),
            |date| format!("{common};snapshot={date}"),
        );
        for reader in readers {
            let _ = write!(
                entries,
                "\n[[entry]]\ndoc = \"invented.pdf\"\ndoc_sha256 = \"{hash}\"\npage = 1\ntable = \"Capital return\"\nrow = \"{role}\"\ncol = \"H1 2026\"\ntoken = \"{token}\"\nquantity = \"capital_return_{role}\"\nunit = \"USD_m\"\nbasis = \"{basis}\"\nperiod = \"2026H1\"\nreader = \"{reader}\"\n"
            );
        }
    }
    fs::write(
        root.join("registries/transcriptions/roce_test.toml"),
        entries,
    )
    .unwrap();
}

fn filing_args() -> Vec<String> {
    args(&[
        "metrics",
        "roce",
        "--issuer",
        "roce_test",
        "--document",
        "invented.pdf",
        "--series",
        "capital_return",
        "--period",
        "2026H1",
        "--as-of",
        "2026-08-02",
    ])
}

#[test]
fn filing_binary_authenticates_sources_retains_rounding_and_moves_when_source_changes() {
    let root = fixture();
    write_filing(&root, "12.0", &["reader_a", "reader_b"], false);
    let words = filing_args();
    let (library, _) = invoke(&words, &root);
    assert_eq!(library.exit_code, 0, "{}", library.to_json());
    let output = binary(&root, &words);
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        format!("{}\n", library.to_json())
    );
    let json = parse(&output.stdout).unwrap();
    let rate = field(
        &json,
        &[
            "payload",
            "receipt",
            "result",
            "value",
            "period_return_percent",
        ],
    );
    assert_eq!(field(rate, &["point"]).as_str(), Some("12"));
    assert_eq!(field(rate, &["lo"]).as_str(), Some("23900/2001"));
    assert_eq!(field(rate, &["hi"]).as_str(), Some("24100/1999"));
    assert!(String::from_utf8_lossy(&output.stdout).contains("definition_table=Definitions"));
    let mut markdown = words.clone();
    markdown.extend(args(&["--format", "markdown"]));
    assert_eq!(
        String::from_utf8_lossy(&binary(&root, &markdown).stdout),
        format!("{}\n", library.rendered_markdown.unwrap())
    );
    fs::write(
        root.join("documents/invented.pdf"),
        b"mutated unregistered filing bytes",
    )
    .unwrap();
    let refused = binary(&root, &words);
    assert_ne!(refused.status.code(), Some(0));
    assert!(String::from_utf8_lossy(&refused.stdout).contains("FMINE-REF-505"));
    write_filing(&root, "13.0", &["reader_a", "reader_b"], false);
    let changed = binary(&root, &words);
    assert_eq!(changed.status.code(), Some(0));
    let changed_json = parse(&changed.stdout).unwrap();
    assert_eq!(
        field(
            &changed_json,
            &[
                "payload",
                "receipt",
                "result",
                "value",
                "period_return_percent",
                "point"
            ]
        )
        .as_str(),
        Some("13")
    );
    assert_ne!(
        field(&json, &["payload", "receipt", "digest"]),
        field(&changed_json, &["payload", "receipt", "digest"])
    );
}

#[test]
fn filing_refusals_preserve_exclusions_reader_completeness_and_the_evidence_cut() {
    let root = fixture();
    let words = filing_args();
    for (readers, excluded, code) in [
        (&["reader_a"][..], false, "FMINE-REF-202"),
        (&["reader_a", "reader_b"][..], true, "FMINE-REF-208"),
    ] {
        write_filing(&root, "12.0", readers, excluded);
        let output = binary(&root, &words);
        assert_ne!(output.status.code(), Some(0));
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains(code), "{stdout}");
        if excluded {
            assert!(stdout.contains("development project not producing"));
        }
    }
    write_filing(&root, "12.0", &["reader_a", "reader_b"], false);
    let mut early = words;
    *early.last_mut().unwrap() = "2026-07-31".into();
    let output = binary(&root, &early);
    assert_ne!(output.status.code(), Some(0));
    assert!(String::from_utf8_lossy(&output.stdout).contains("FMINE-REF-108"));
}

#[test]
fn synthetic_filing_is_labelled_in_both_native_report_formats() {
    let root = fixture();
    write_filing(&root, "12.0", &["reader_a", "reader_b"], false);
    for directory in ["registries/sources", "registries/transcriptions"] {
        let folder = root.join(directory);
        let text = fs::read_to_string(folder.join("roce_test.toml")).unwrap();
        fs::write(
            folder.join("synthetic.toml"),
            text.replace("issuer = \"roce_test\"", "issuer = \"synthetic\""),
        )
        .unwrap();
    }
    let mut words = filing_args();
    let issuer = words.iter().position(|word| word == "--issuer").unwrap() + 1;
    words[issuer] = "synthetic".into();
    for format in ["json", "markdown"] {
        let mut selected = words.clone();
        selected.extend(args(&["--format", format]));
        let report = binary(&root, &selected);
        let text = String::from_utf8(report.stdout).unwrap();
        assert_eq!(report.status.code(), Some(0), "{text}");
        assert!(text.contains("Synthetic filing example with invented inputs."));
    }
}

#[test]
fn command_rejects_mixed_modes_duplicate_options_and_invalid_capital_domain() {
    let root = fixture();
    for extra in [
        &["--issuer", "roce_test"][..],
        &["--input", "return.toml"][..],
        &["--unknown", "value"][..],
    ] {
        let mut words = args(&["metrics", "roce", "--input", "return.toml"]);
        words.extend(args(extra));
        assert_ne!(binary(&root, &words).status.code(), Some(0));
    }
    let invalid = WORKSHEET.replace("amount = \"80\"", "amount = \"-120\"");
    fs::write(root.join("return.toml"), invalid).unwrap();
    let output = binary(&root, &args(&["metrics", "roce", "--input", "return.toml"]));
    assert_ne!(output.status.code(), Some(0));
    assert!(String::from_utf8_lossy(&output.stdout).contains("FMINE-REF-104"));
}

#[test]
fn filing_selects_requested_period_before_agreement_without_dropping_conflicting_readers() {
    let root = fixture();
    write_filing(&root, "12.0", &["reader_a", "reader_b"], false);
    let path = root.join("registries/transcriptions/roce_test.toml");
    let current = fs::read_to_string(&path).unwrap();
    let entry_start = current.find("\n[[entry]]").unwrap();
    let comparative = current[entry_start..]
        .replace("2026H1", "2025H1")
        .replace("H1 2026", "H1 2025")
        .replace("snapshot=2025-12-31", "snapshot=2024-12-31")
        .replace("snapshot=2026-06-30", "snapshot=2025-06-30")
        .replace("token = \"12.0\"", "token = \"6.0\"");
    let complete = format!("{current}{comparative}");
    fs::write(&path, &complete).unwrap();
    for (period, expected) in [("2026H1", "12"), ("2025H1", "6")] {
        let mut words = filing_args();
        let selected = words.iter().position(|v| v == "--period").unwrap() + 1;
        words[selected] = period.into();
        let output = binary(&root, &words);
        assert_eq!(
            output.status.code(),
            Some(0),
            "{}",
            String::from_utf8_lossy(&output.stdout)
        );
        let json = parse(&output.stdout).unwrap();
        assert_eq!(
            field(
                &json,
                &[
                    "payload",
                    "receipt",
                    "result",
                    "value",
                    "period_return_percent",
                    "point"
                ]
            )
            .as_str(),
            Some(expected)
        );
        assert_eq!(
            field(&json, &["payload", "receipt", "inputs"])
                .as_array()
                .unwrap()
                .len(),
            4
        );
    }
    // One reader changes the period at the current earnings locator. Selecting the other reader's
    // current-period entry must retain this conflicting reader at that same locator.
    let conflicted = complete.replacen(
        "period = \"2026H1\"\nreader = \"reader_b\"",
        "period = \"2025H1\"\nreader = \"reader_b\"",
        1,
    );
    assert_ne!(conflicted, complete);
    fs::write(&path, conflicted).unwrap();
    let output = binary(&root, &filing_args());
    assert_ne!(output.status.code(), Some(0));
    assert!(String::from_utf8_lossy(&output.stdout).contains("FMINE-REF-202"));
    // Missing current earnings cannot be filled from the complete comparative period.
    let mut missing = current[..entry_start].to_string();
    for entry in current[entry_start..].split("\n[[entry]]").skip(1) {
        if !entry.contains("quantity = \"capital_return_earnings\"") {
            missing.push_str("\n[[entry]]");
            missing.push_str(entry);
        }
    }
    missing.push_str(&comparative);
    fs::write(&path, missing).unwrap();
    assert_ne!(binary(&root, &filing_args()).status.code(), Some(0));
}
