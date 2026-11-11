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
    fixture_with_metric(q3, actual, None)
}

fn fixture_with_metric(q3: bool, actual: &str, copper_cost: Option<&str>) -> PathBuf {
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
    let mut rules = RULES.replace(
        "issuer = \"barrick\"",
        &format!("issuer = \"{SYNTHETIC_ISSUER}\""),
    );
    if let Some(cost) = copper_cost {
        rules = rules
            .replace("au_prod_100_koz", "cu_prod_100_kt")
            .replace("aisc_usd_per_oz", cost);
    }
    std::fs::write(root.join("forecasts/fy2026/rules.toml"), rules).unwrap();
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
            if let Some(cost) = copper_cost {
                for sheet in &mut sheets {
                    for (at, cell) in &mut sheet.cells {
                        if at == "B4" {
                            *cell = MinimalCell::Text("Copper produced (kt 100%)".into());
                        } else if at == "C4" {
                            *cell = MinimalCell::Text(
                                if cost == "c1_usd_per_lb" {
                                    "C1 cash costs ($/lb)"
                                } else {
                                    "All-in sustaining costs ($/lb)"
                                }
                                .into(),
                            );
                        }
                    }
                }
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
    if let Some(cost) = copper_cost {
        let definition = if cost == "c1_usd_per_lb" {
            "c1"
        } else {
            "aisc"
        };
        let readings = std::fs::read_to_string(transcripts(&root))
            .unwrap()
            .replace("production_koz", "production_kt")
            .replace("unit = \"koz\"", "unit = \"kt\"")
            .replace("aisc_usd_per_oz", cost)
            .replace("unit = \"usd_per_oz\"", "unit = \"usd_per_lb\"")
            .replace("900 - 1100", "2.40 - 3.00")
            .replace(
                ";aisc=issuer-reported-per-sold-oz",
                &format!(";{definition}=issuer-reported-per-sold-lb"),
            );
        std::fs::write(transcripts(&root), readings).unwrap();
    }
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
fn copper_guidance_uses_exact_kilotonnes_and_distinct_sold_pound_cost_definitions() {
    for cost in ["c1_usd_per_lb", "aisc_usd_per_lb"] {
        let root = fixture_with_metric(true, "300", Some(cost));
        let report = run(&root);
        let production = row(&report, "production");
        // Independent arithmetic: attributable [240, 300] / (3/5) - 300 kt.
        assert_eq!(production.interval, interval(100, 200));
        assert_eq!(production.unit, "kt");
        assert_eq!(production.basis, "100%");
        assert_eq!(
            production.actuals[0].key.line.label,
            "summary:cu_prod_100_kt"
        );
        assert_eq!(production.actuals[0].key.unit, "kt");
        let rate = row(&report, "unit_cost");
        assert_eq!(
            rate.interval,
            Some((Rat::new(12, 5).unwrap(), Rat::from_integer(3)))
        );
        assert_eq!(rate.unit, "usd_per_lb");
        assert!(
            rate.actuals.is_empty(),
            "rates must not subtract nine-month values"
        );
        assert!(rate.guidance[0].reading.quantity.ends_with(cost));
        let lines = baseline_lines_as_of(&root, "2026-11-10").unwrap();
        assert!(lines[0].contains("\"synthetic\":true"));
        assert!(lines.join("\n").contains("summary:cu_prod_100_kt"));
        assert_eq!(lines, baseline_lines_as_of(&root, "2026-11-10").unwrap());
    }
}

#[test]
fn copper_baseline_refuses_unit_definition_and_additivity_substitutions() {
    for (from, to, target) in [
        ("unit = \"kt\"", "unit = \"koz\"", "production"),
        (
            "unit = \"usd_per_lb\"",
            "unit = \"usd_per_oz\"",
            "unit_cost",
        ),
        (
            ";c1=issuer-reported-per-sold-lb",
            ";aisc=issuer-reported-per-sold-lb",
            "unit_cost",
        ),
        (
            ";c1=issuer-reported-per-sold-lb",
            ";c1=issuer-reported-per-produced-lb",
            "unit_cost",
        ),
    ] {
        let root = fixture_with_metric(true, "300", Some("c1_usd_per_lb"));
        let readings = std::fs::read_to_string(transcripts(&root))
            .unwrap()
            .replace(from, to);
        std::fs::write(transcripts(&root), readings).unwrap();
        assert!(row(&run(&root), target).interval.is_none());
    }
    let root = fixture_with_metric(true, "300", Some("c1_usd_per_lb"));
    let path = root.join("forecasts/fy2026/rules.toml");
    let rules = std::fs::read_to_string(&path)
        .unwrap()
        .replace("outcome = \"q4_qtd\"", "outcome = \"fy_minus_9m\"");
    std::fs::write(path, rules).unwrap();
    assert!(guidance::run(&root, "2026-11-10").is_err());
    assert!(xtask::forecast::persistence_baselines(&root).is_err());
}

#[test]
fn copper_baseline_keeps_missing_originals_and_signed_remainders_visible() {
    let root = fixture_with_metric(false, "300", Some("c1_usd_per_lb"));
    let report = run(&root);
    assert_eq!(row(&report, "production").interval, None);
    assert!(
        row(&report, "production")
            .reason
            .contains("original Q3-release")
    );
    assert!(row(&report, "unit_cost").interval.is_some());
    let signed = fixture_with_metric(true, "550", Some("aisc_usd_per_lb"));
    assert_eq!(
        row(&run(&signed), "production").interval,
        interval(-150, -50)
    );
}

#[test]
fn one_sided_latest_guidance_never_becomes_a_finite_baseline_or_selects_an_older_range() {
    for token in ["at least 240", "≤300"] {
        let root = fixture(true, "300");
        add_guidance(
            &root,
            "Bound.pdf",
            "2026-10-03",
            token,
            &["reader-a", "reader-b"],
        );
        let report = run(&root);
        let production = row(&report, "production");
        assert!(production.interval.is_none());
        assert!(production.reason.contains("one-sided guidance"));
        assert_eq!(production.guidance[0].document.file, "Bound.pdf");
        assert_eq!(report.rows.len(), 26);
        assert_eq!(row(&report, "unit_cost").interval, interval(900, 1100));
    }
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

#[test]
fn each_workbook_uses_its_own_retrieval_upper_bound_before_opening_source_bytes() {
    let root = fixture(true, "300");
    let text = std::fs::read_to_string(registry(&root))
        .unwrap()
        .replace("2026-10-01T00:00:00Z", "2026-07-02T00:00:00Z")
        .replace(
            "file = \"2026Q3.xlsx\"",
            "file = \"2026Q3.xlsx\"\nretrieved_utc = \"2026-10-02T23:59:59Z/2026-10-03T00:00:01Z\"",
        );
    std::fs::write(registry(&root), text).unwrap();
    let early = guidance::run(&root, "2026-10-02").unwrap_err().join(" ");
    assert!(early.contains("verified retrieval 2026-10-03"), "{early}");
    let ready = guidance::run(&root, "2026-10-03").unwrap();
    assert_eq!(ready.workbooks_available_by, "2026-10-03");
    assert_eq!(row(&ready, "production").interval, interval(100, 200));
    assert_eq!(row(&ready, "production").actuals.len(), 1);

    std::fs::write(root.join("corpus/2026Q3.xlsx"), b"altered future bytes").unwrap();
    let early = guidance::run(&root, "2026-10-02").unwrap_err().join(" ");
    assert!(early.contains("verified retrieval 2026-10-03"), "{early}");
    assert!(
        guidance::run(&root, "2026-10-03")
            .unwrap_err()
            .join(" ")
            .contains("FMINE-REF-505")
    );
}

#[test]
fn later_batch_cannot_hide_an_impossible_source_date_or_a_reversed_interval() {
    let root = fixture(true, "300");
    let original = std::fs::read_to_string(registry(&root)).unwrap();
    std::fs::write(
        registry(&root),
        original.replace(
            "file = \"2026Q3.xlsx\"",
            "file = \"2026Q3.xlsx\"\nretrieved_utc = \"2026-09-29T23:59:59Z\"",
        ),
    )
    .unwrap();
    let error = guidance::run(&root, "2026-11-10").unwrap_err().join(" ");
    assert!(
        error.contains("2026Q3.xlsx: quarter end 2026-09-30"),
        "{error}"
    );

    std::fs::write(
        registry(&root),
        original.replace(
            "2026-10-01T00:00:00Z",
            "2026-10-01T00:00:00Z/2026-09-30T23:59:59Z",
        ),
    )
    .unwrap();
    let error = guidance::run(&root, "2026-11-10").unwrap_err().join(" ");
    assert!(error.contains("retrieval interval is reversed"), "{error}");
}
