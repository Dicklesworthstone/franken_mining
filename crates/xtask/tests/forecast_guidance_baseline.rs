//! Independent exact controls through actual registered XLSX bytes and two-reader transcription loading.
//! Every workbook and PDF byte below is invented evidence, never a claim about an issuer.
#![cfg(test)]

use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use fmine_core::hash::sha256;
use fmine_io::xlsx::{MinimalCell, MinimalSheet, MinimalStyles, build_minimal_xlsx_styled};
use fmine_numerics::exact_feas::Rat;
use fmine_public::synthetic::SYNTHETIC_ISSUER;
use xtask::forecast::{
    baseline_lines_as_of,
    guidance::{self, BaselineReport, BaselineRow},
};

static NEXT: AtomicUsize = AtomicUsize::new(0);
const RULES: &str = include_str!("../../../forecasts/fy2026/rules.toml");

fn doc_bytes(name: &str) -> Vec<u8> {
    format!("%PDF-1.7 invented {name} baseline test bytes").into_bytes()
}
fn registry(root: &Path) -> PathBuf {
    root.join(format!("registries/sources/{SYNTHETIC_ISSUER}.toml"))
}
fn transcripts(root: &Path) -> PathBuf {
    root.join(format!("registries/transcriptions/{SYNTHETIC_ISSUER}.toml"))
}

fn sheet(name: &str, year: i32, months: &str, end: &str, production: &str) -> MinimalSheet {
    let text = |s: &str| MinimalCell::Text(s.into());
    let number = |s: &str| MinimalCell::Styled {
        raw: s.into(),
        xf: 0,
    };
    MinimalSheet {
        name: name.into(),
        hidden: false,
        cells: vec![
            (
                "A3".into(),
                text(&format!("For the {months} months ended {end},")),
            ),
            ("B3".into(), number(&year.to_string())),
            ("A5".into(), text("North Mara (60%)")),
            ("B4".into(), text("Gold produced (000s oz 100%)")),
            ("B5".into(), number(production)),
            ("C4".into(), text("All-in sustaining costs ($/oz)")),
            ("C5".into(), number("1000")),
        ],
    }
}

fn source(root: &Path, release: &str, sheets: &[MinimalSheet]) {
    let bytes = build_minimal_xlsx_styled(
        sheets,
        Some(&MinimalStyles {
            num_fmts: vec![],
            xfs: vec![1],
        }),
    );
    let file = format!("{release}.xlsx");
    std::fs::write(root.join("corpus").join(&file), &bytes).unwrap();
    let mut reg = std::fs::read_to_string(registry(root)).unwrap();
    write!(reg, "\n[[source]]\nfile = \"{file}\"\nkind = \"mine_stats\"\nquarter = \"{release}\"\npath = \"{file}\"\nurl_basis = \"W1 table\"\nbytes = {}\nsha256 = \"{}\"\n", bytes.len(), sha256(&bytes).to_hex()).unwrap();
    std::fs::write(registry(root), reg).unwrap();
}

fn document(root: &Path, name: &str, date: &str) {
    let bytes = doc_bytes(name);
    std::fs::write(root.join("corpus").join(name), &bytes).unwrap();
    let mut reg = std::fs::read_to_string(registry(root)).unwrap();
    write!(reg, "\n[[document]]\nfile = \"{name}\"\nkind = \"mda\"\nurl = \"https://example.invalid/{name}\"\nurl_basis = \"W1 table\"\npublished_at = \"{date}\"\npublished_at_basis = \"FACT: invented test publication\"\npages = 1\nbytes = {}\nsha256 = \"{}\"\n", bytes.len(), sha256(&bytes).to_hex()).unwrap();
    std::fs::write(registry(root), reg).unwrap();
}

fn reading(name: &str, reader: &str, metric: &str, token: &str, unit: &str, basis: &str) -> String {
    format!(
        "\n[[entry]]\ndoc = \"{name}\"\ndoc_sha256 = \"{}\"\npage = 1\ntable = \"Invented guidance\"\nrow = \"North Mara\"\ncol = \"{metric}\"\ntoken = \"{token}\"\nquantity = \"guidance_north_mara_{metric}\"\nunit = \"{unit}\"\nbasis = \"{basis}\"\nperiod = \"2026FY\"\nreader = \"{reader}\"\n",
        sha256(&doc_bytes(name)).to_hex()
    )
}

fn add_guidance(root: &Path, name: &str, date: &str, token: &str, readers: &[&str]) {
    document(root, name, date);
    let mut text = std::fs::read_to_string(transcripts(root)).unwrap();
    for reader in readers {
        text.push_str(&reading(
            name,
            reader,
            "production_koz",
            token,
            "koz",
            "attributable:3/5",
        ));
        text.push_str(&reading(
            name,
            reader,
            "aisc_usd_per_oz",
            "900 - 1100",
            "usd_per_oz",
            "attributable:3/5;aisc=issuer-reported-per-sold-oz",
        ));
    }
    std::fs::write(transcripts(root), text).unwrap();
}

fn fixture(q3: bool, actual: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "fmine-baseline-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    for dir in [
        "corpus",
        "registries/sources",
        "registries/transcriptions",
        "forecasts/fy2026",
    ] {
        std::fs::create_dir_all(root.join(dir)).unwrap();
    }
    std::fs::write(
        root.join("forecasts/fy2026/rules.toml"),
        RULES.replace(
            "issuer = \"barrick\"",
            &format!("issuer = \"{SYNTHETIC_ISSUER}\""),
        ),
    )
    .unwrap();
    std::fs::write(registry(&root), format!("schema = \"fmine.sources.v1\"\nissuer = \"{SYNTHETIC_ISSUER}\"\ncorpus_dir = \"corpus\"\nretrieved_utc = \"2026-10-01T00:00:00Z\"\nurl_prefix = \"https://example.invalid/\"\n[[subject]]\nname = \"North Mara\"\n")).unwrap();
    std::fs::write(
        transcripts(&root),
        format!("schema = \"fmine.transcriptions.v1\"\nissuer = \"{SYNTHETIC_ISSUER}\"\n"),
    )
    .unwrap();
    for year in 2024..=2026 {
        for q in 1..=if year == 2026 {
            if q3 { 3 } else { 2 }
        } else {
            4
        } {
            let end = ["March 31", "June 30", "September 30", "December 31"][q - 1];
            let mut sheets = vec![sheet("QTD", year, "three", end, "100")];
            if year == 2026 && q == 3 {
                sheets.push(sheet("YTD", year, "nine", end, actual));
            }
            source(&root, &format!("{year}Q{q}"), &sheets);
        }
    }
    add_guidance(
        &root,
        "Initial.pdf",
        "2026-01-15",
        "180 - 240",
        &["reader-a", "reader-b"],
    );
    add_guidance(
        &root,
        "Revision.pdf",
        "2026-10-02",
        "240 - 300",
        &["reader-a", "reader-b"],
    );
    root
}

fn run(root: &Path) -> BaselineReport {
    guidance::run(root, "2026-11-10").unwrap()
}
fn row<'a>(report: &'a BaselineReport, target: &str) -> &'a BaselineRow {
    report
        .rows
        .iter()
        .find(|r| r.site == "North Mara" && r.target == target)
        .unwrap()
}
fn interval(a: i128, b: i128) -> Option<(Rat, Rat)> {
    Some((Rat::from_integer(a), Rat::from_integer(b)))
}

#[test]
fn registered_guidance_and_original_nine_months_produce_exact_source_bound_baselines() {
    let root = fixture(true, "300");
    let r = run(&root);
    assert_eq!(r.rows.len(), 26);
    assert_eq!(
        r.rows
            .iter()
            .filter(|r| r.status == "not_published")
            .count(),
        4
    );
    assert_eq!(r.rows.iter().filter(|r| r.interval.is_some()).count(), 2);
    let production = row(&r, "production");
    assert_eq!(production.interval, interval(100, 200));
    assert_eq!(production.basis, "100%");
    assert_eq!(production.guidance[0].document.file, "Revision.pdf");
    assert_eq!(production.actuals.len(), 1);
    assert!(production.actuals[0].locator.contains("2026Q3.xlsx:YTD!B5"));
    assert_eq!(row(&r, "unit_cost").interval, interval(900, 1100));
    assert!(row(&r, "unit_cost").actuals.is_empty());
    let lines = baseline_lines_as_of(&root, "2026-11-10").unwrap();
    assert!(
        lines
            .iter()
            .any(|l| l.contains("\"baseline\":\"run_rate_persistence\""))
    );
    assert!(
        lines
            .iter()
            .any(|l| l.contains("\"baseline\":\"issuer_guidance\""))
    );
    assert!(lines[0].contains("\"synthetic\":true"));
    assert!(
        lines
            .join("\n")
            .contains(&production.actuals[0].claim.claim_id.to_string())
    );
    assert_eq!(lines, baseline_lines_as_of(&root, "2026-11-10").unwrap());
}

#[test]
fn cut_selects_last_guidance_and_does_not_read_future_document_bytes() {
    let root = fixture(true, "300");
    add_guidance(
        &root,
        "Future.pdf",
        "2026-12-01",
        "300 - 360",
        &["reader-a", "reader-b"],
    );
    let early = guidance::run(&root, "2026-10-01").unwrap();
    assert_eq!(row(&early, "production").interval, interval(0, 100));
    assert_eq!(row(&run(&root), "production").interval, interval(100, 200));
    let late = guidance::run(&root, "2026-12-01").unwrap();
    assert_eq!(row(&late, "production").interval, interval(200, 300));
    std::fs::write(root.join("corpus/Future.pdf"), b"altered future bytes").unwrap();
    assert!(guidance::run(&root, "2026-11-10").is_ok());
    assert!(
        guidance::run(&root, "2026-12-01")
            .unwrap_err()
            .join(" ")
            .contains("FMINE-REF-505")
    );
}

#[test]
fn simultaneous_or_pending_latest_guidance_cannot_silently_select_an_older_value() {
    let root = fixture(true, "300");
    add_guidance(
        &root,
        "SameDate.pdf",
        "2026-10-02",
        "300 - 360",
        &["reader-a", "reader-b"],
    );
    let r = run(&root);
    assert!(row(&r, "production").interval.is_none());
    assert_eq!(row(&r, "production").guidance.len(), 2);
    assert!(
        row(&r, "production")
            .reason
            .contains("latest publication date")
    );
    let pending = fixture(true, "300");
    add_guidance(
        &pending,
        "Pending.pdf",
        "2026-10-03",
        "300 - 360",
        &["reader-a"],
    );
    let r = run(&pending);
    assert!(row(&r, "production").interval.is_none());
    assert!(row(&r, "production").pending_readings[0].contains("Pending.pdf"));
}

#[test]
fn guidance_units_sold_denominator_and_ownership_are_required() {
    for (from, to, target) in [
        ("unit = \"koz\"", "unit = \"kt\"", "production"),
        (
            "basis = \"attributable:3/5\"",
            "basis = \"attributable:0/5\"",
            "production",
        ),
        (
            ";aisc=issuer-reported-per-sold-oz",
            ";aisc=issuer-reported-per-produced-oz",
            "unit_cost",
        ),
    ] {
        let root = fixture(true, "300");
        let text = std::fs::read_to_string(transcripts(&root))
            .unwrap()
            .replace(from, to);
        std::fs::write(transcripts(&root), text).unwrap();
        assert!(row(&run(&root), target).interval.is_none());
    }
}

#[test]
fn missing_original_nine_months_stays_missing_and_no_comparative_fills_it() {
    let assert_missing_original = |report: &BaselineReport| {
        assert_eq!(report.rows.len(), 26);
        let production = row(report, "production");
        assert_eq!(production.status, "not_available");
        assert_eq!(
            production.reason,
            "original Q3-release nine-month production is not available"
        );
        assert!(production.interval.is_none());
        assert!(production.actuals.is_empty());
        assert_eq!(production.guidance.len(), 1);
        assert_eq!(production.guidance[0].document.file, "Revision.pdf");
        let cost = row(report, "unit_cost");
        assert_eq!(cost.status, "available");
        assert_eq!(cost.interval, interval(900, 1100));
        assert!(cost.actuals.is_empty());
        assert_eq!(cost.guidance.len(), 1);
        assert_eq!(cost.guidance[0].document.file, "Revision.pdf");
    };
    let root = fixture(false, "300");
    let r = run(&root);
    assert_missing_original(&r);
    source(
        &root,
        "2026Q4",
        &[sheet("YTD", 2026, "nine", "September 30", "301")],
    );
    let text = std::fs::read_to_string(registry(&root))
        .unwrap()
        .replace("2026-10-01T00:00:00Z", "2027-01-15T00:00:00Z");
    std::fs::write(registry(&root), text).unwrap();
    let later = guidance::run(&root, "2027-02-01").unwrap();
    assert_missing_original(&later);
}

#[test]
fn source_changes_move_values_and_digest_and_unregistered_changes_refuse() {
    let root = fixture(true, "300");
    let r = run(&root);
    let changed = fixture(true, "301");
    let other = run(&changed);
    assert_eq!(row(&other, "production").interval, interval(99, 199));
    assert_ne!(r.json_lines()[0], other.json_lines()[0]);
    assert_ne!(
        row(&r, "production").actuals[0].claim.claim_id,
        row(&other, "production").actuals[0].claim.claim_id
    );
    std::fs::write(root.join("corpus/2026Q3.xlsx"), b"altered bytes").unwrap();
    assert!(
        guidance::run(&root, "2026-11-10")
            .unwrap_err()
            .join(" ")
            .contains("FMINE-REF-505")
    );
    let missing = fixture(true, "300");
    std::fs::rename(
        missing.join("corpus/2026Q3.xlsx"),
        missing.join("corpus/2026Q3.xlsx.saved"),
    )
    .unwrap();
    assert!(
        guidance::run(&missing, "2026-11-10")
            .unwrap_err()
            .join(" ")
            .contains("FMINE-REF-506")
    );
}

#[test]
fn nine_month_baseline_uses_printed_value_and_keeps_underlying_value_in_evidence() {
    let root = fixture(true, "300.4");
    let r = run(&root);
    let production = row(&r, "production");
    assert_eq!(production.interval, interval(100, 200));
    let evidence = production.actuals[0].to_json();
    assert!(evidence.contains("\"stored\":\"300.4\""));
    assert!(evidence.contains("\"displayed\":\"300\""));
}

#[test]
fn negative_remainder_is_visible_and_availability_never_uses_the_host_clock() {
    let root = fixture(true, "600");
    assert_eq!(
        row(&run(&root), "production").interval,
        interval(-200, -100)
    );
    assert!(
        guidance::run(&root, "2026-09-30")
            .unwrap_err()
            .join(" ")
            .contains("verified retrieval")
    );
    assert!(guidance::run(&root, "2026-13-01").is_err());
}
