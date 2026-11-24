//! Native command checks using invented worksheet and filing data.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

use fmine_core::hash::sha256;
use fmine_io::omf::json::{Json, parse};

static NEXT: AtomicU64 = AtomicU64::new(0);
const WORKSHEET: &str = "tests/metrics/capital_return.toml";
const SERIES: &str = "roce_half_year";
const DOCUMENT: &str = "Invented.pdf";
const PUBLICATION: &str = "2026-07-15";
const DOC: &[u8] = b"Invented filing test bytes for capital returns. No issuer measurements.";

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn native(root: &Path, verb: &[&str], args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_fmine"))
        .current_dir(root)
        .args(verb)
        .args(args)
        .output()
        .unwrap()
}

fn field<'a>(json: &'a Json, keys: &[&str]) -> &'a Json {
    keys.iter()
        .fold(json, |value, key| value.get(key).expect(key))
}

fn parsed(output: &Output) -> Json {
    parse(&output.stdout).unwrap_or_else(|_| panic!("{}", String::from_utf8_lossy(&output.stdout)))
}

fn refuse(output: Output, code: &str) {
    let json = parsed(&output);
    assert!(
        !output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert_eq!(field(&json, &["refusal", "code"]).as_str(), Some(code));
    assert!(matches!(json.get("payload"), None | Some(Json::Null)));
}

#[test]
fn native_worksheet_reports_exact_capital_returns_and_the_declared_annualization() {
    let root = root();
    let route = ["metrics", "capital-return"];
    let args = ["--input", WORKSHEET, "--json"];
    let output = native(&root, &route, &args);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    let json = parsed(&output);
    let payload = field(&json, &["payload"]);
    assert_eq!(
        field(payload, &["mode"]).as_str(),
        Some("declared_worksheet")
    );
    let summary = field(payload, &["receipt", "result", "value"]);
    for (key, amount) in [
        ("earnings", "12"),
        ("capital_employed", "100"),
        ("period_return_percent", "12"),
        ("annualized_return_percent", "24"),
    ] {
        assert_eq!(
            field(summary, &[key, "point"]).as_str(),
            Some(amount),
            "{key}"
        );
    }
    let digest = sha256(&fs::read(root.join(WORKSHEET)).unwrap()).to_hex();
    assert_eq!(
        field(payload, &["source", "sha256"]).as_str(),
        Some(digest.as_str())
    );
    assert_eq!(native(&root, &route, &args).stdout, output.stdout);
    let md = native(
        &root,
        &route,
        &["--input", WORKSHEET, "--format", "markdown"],
    );
    assert!(md.status.success());
    let shown = String::from_utf8(md.stdout).unwrap();
    assert!(shown.contains("| Period return (%) | 12 [12; 12] |"));
    assert!(shown.contains("| Annualized return (%) | 24 [24; 24] |"));
    assert!(shown.contains("opening_closing_mean"));
    let docs = parsed(&native(&root, &["robot-docs"], &["--json"]));
    assert!(
        field(&docs, &["payload", "capital_return"])
            .get("filing")
            .is_some()
    );
}

fn filing_workspace() -> (PathBuf, String, String) {
    let root = std::env::temp_dir().join(format!(
        "fmine-capital-return-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed),
    ));
    fs::create_dir(&root).unwrap();
    for path in ["corpus", "registries/sources", "registries/transcriptions"] {
        fs::create_dir_all(root.join(path)).unwrap();
    }
    let digest = sha256(DOC).to_hex();
    fs::write(root.join("corpus").join(DOCUMENT), DOC).unwrap();
    fs::write(
        root.join("registries/sources/synthetic.toml"),
        format!(
            "schema = \"fmine.sources.v1\"\nissuer = \"synthetic\"\ncorpus_dir = \"corpus\"\nretrieved_utc = \"2026-07-16T00:00:00Z\"\nurl_prefix = \"https://example.invalid/\"\n[[subject]]\nname = \"Invented mine\"\n\n[[document]]\nfile = \"{DOCUMENT}\"\ndir = \"corpus\"\nkind = \"mda\"\nurl = \"https://example.invalid/{DOCUMENT}\"\nurl_basis = \"W1 table\"\npublished_at = \"{PUBLICATION}\"\npublished_at_basis = \"FACT: invented fixture\"\npages = 2\nbytes = {}\nsha256 = \"{digest}\"\n",
            DOC.len(),
        ),
    ).unwrap();
    let common = "subject=Invented mine;ownership=full;perimeter=continuing;accounting_basis=historical cost;currency=USD;monetary_scale=1000000;definition=issuer_return;version=1;earnings_basis=adjusted operating earnings;tax_basis=after_tax;capital_employed_basis=net operating assets;capital_basis=opening_closing_mean;annualization=none;applicability=applicable;definition_page=2;definition_table=Definitions;definition_row=Capital return;definition_col=Basis";
    let mut transcript =
        String::from("schema = \"fmine.transcriptions.v1\"\nissuer = \"synthetic\"\n");
    for (role, token, snapshot) in [
        ("earnings", "12.0", ""),
        ("opening", "80.0", ";snapshot=2025-12-31"),
        ("closing", "120.0", ";snapshot=2026-06-30"),
    ] {
        for reader in ["reader_a", "reader_b"] {
            transcript.push_str(&format!(
                "\n[[entry]]\ndoc = \"{DOCUMENT}\"\ndoc_sha256 = \"{digest}\"\npage = 1\ntable = \"Return\"\nrow = \"{role}\"\ncol = \"H1\"\ntoken = \"{token}\"\nquantity = \"{SERIES}_{role}\"\nunit = \"USD_m\"\nbasis = \"{common}{snapshot}\"\nperiod = \"2026H1\"\nreader = \"{reader}\"\n"
            ));
        }
    }
    fs::write(
        root.join("registries/transcriptions/synthetic.toml"),
        &transcript,
    )
    .unwrap();
    (root, digest, transcript)
}

#[test]
fn native_filing_binds_readers_and_rounding_and_refuses_changed_evidence() {
    let (root, digest, transcript) = filing_workspace();
    let call = |cut, period| {
        native(
            &root,
            &["metrics", "capital-return"],
            &[
                "--issuer",
                "synthetic",
                "--document",
                DOCUMENT,
                "--series",
                SERIES,
                "--period",
                period,
                "--as-of",
                cut,
                "--json",
            ],
        )
    };
    let output = call(PUBLICATION, "2026H1");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    let json = parsed(&output);
    let payload = field(&json, &["payload"]);
    assert_eq!(
        field(payload, &["document", "sha256"]).as_str(),
        Some(digest.as_str())
    );
    let summary = field(payload, &["receipt", "result", "value"]);
    let ratio = field(summary, &["period_return_percent"]);
    // Printed earnings span [11.95, 12.05] and the endpoint mean spans [99.95, 100.05].
    for (key, expected) in [("point", "12"), ("lo", "23900/2001"), ("hi", "24100/1999")] {
        assert_eq!(field(ratio, &[key]).as_str(), Some(expected));
    }
    assert!(matches!(
        summary.get("annualized_return_percent"),
        Some(Json::Null)
    ));
    let raw = String::from_utf8(output.stdout).unwrap();
    for cited in [
        "reader_a",
        "reader_b",
        "Definitions",
        "adjusted operating earnings",
        "Synthetic filing example",
    ] {
        assert!(raw.contains(cited), "{cited}");
    }
    refuse(call("2026-07-14", "2026H1"), "FMINE-REF-108");
    refuse(call(PUBLICATION, "2026H2"), "FMINE-REF-102");
    let transcripts = root.join("registries/transcriptions/synthetic.toml");
    fs::write(
        &transcripts,
        transcript.replacen("token = \"12.0\"", "token = \"13.0\"", 1),
    )
    .unwrap();
    refuse(call(PUBLICATION, "2026H1"), "FMINE-REF-202");
    fs::write(&transcripts, transcript).unwrap();
    let mut altered = DOC.to_vec();
    altered[0] ^= 1;
    fs::write(root.join("corpus").join(DOCUMENT), altered).unwrap();
    refuse(call(PUBLICATION, "2026H1"), "FMINE-REF-505");
}

#[test]
fn native_rejects_mixed_and_duplicate_capital_return_selectors() {
    let root = root();
    for args in [
        ["--input", WORKSHEET, "--issuer", "synthetic"],
        ["--input", WORKSHEET, "--input", WORKSHEET],
    ] {
        let output = native(&root, &["metrics", "capital-return"], &args);
        assert_eq!(output.status.code(), Some(2));
        let json = parsed(&output);
        assert_eq!(
            field(&json, &["verb"]).as_str(),
            Some("metrics capital-return")
        );
    }
}
