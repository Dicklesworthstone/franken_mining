//! BD-350.S4 native CLI regression, with synthetic controls and the E876 public transcription.

#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::fs::OpenOptions;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use fmine_core::hash::sha256;
use fmine_io::omf::json::{Json, parse};

const MODEL: &str = include_str!("../../../tests/exploration/developer_npv.toml");
const TAGUAS: &str = include_str!("../../../tests/exploration/taguas_2021_pre_tax.toml");
static NEXT: AtomicU64 = AtomicU64::new(0);

fn run(input: &str, format: &str) -> Output {
    let executable = std::env::var_os("FMINE_TEST_BINARY").map_or_else(
        || {
            let current = std::env::current_exe().expect("test executable");
            let profile = current
                .ancestors()
                .find(|p| {
                    p.file_name()
                        .is_some_and(|name| name == "build" || name == "deps")
                })
                .and_then(Path::parent)
                .expect("Cargo profile directory");
            profile.join(format!("fmine{}", std::env::consts::EXE_SUFFIX))
        },
        PathBuf::from,
    );
    let started = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
    let path = std::env::temp_dir().join(format!(
        "fmine-developer-npv-{}-{}-{}.toml",
        std::process::id(),
        started.as_nanos(),
        NEXT.fetch_add(1, Ordering::Relaxed),
    ));
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .unwrap()
        .write_all(input.as_bytes())
        .unwrap();
    Command::new(executable)
        .args(["check", "developer-npv"])
        .arg(path)
        .args(["--format", format])
        .output()
        .unwrap()
}

fn field<'a>(value: &'a Json, key: &str) -> &'a Json {
    value.get(key).unwrap()
}

fn payload(input: &str) -> Json {
    let out = run(input, "json");
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(out.status.success(), "{text}");
    for flag in [
        "execution_authorized",
        "reserve_disclosure_claim",
        "geotechnical_signoff",
        "regulatory_compliance_claim",
    ] {
        assert!(text.contains(&format!("\"{flag}\":false")));
    }
    field(&parse(&out.stdout).unwrap(), "payload").clone()
}

#[test]
fn native_replay_matches_the_unique_convention_and_retains_citations() {
    let value = payload(MODEL);
    assert_eq!(
        field(&value, "compatible_convention").as_str(),
        Some("MidYearAll")
    );
    assert_eq!(
        field(field(&value, "worksheet"), "sha256").as_str(),
        Some(sha256(MODEL.as_bytes()).to_hex().as_str())
    );
    assert_eq!(
        field(&value, "npv_locator").as_str(),
        Some("p. 10, headline NPV")
    );
    let rows = field(&value, "comparisons").as_array().unwrap();
    assert_eq!(rows.len(), 3);
    assert_eq!(
        field(&rows[0], "npv_bounds_usd_m").as_array().unwrap()[0].as_str(),
        Some("4764500/194481")
    );
    assert_eq!(field(&rows[0], "overlaps_printed").as_bool(), Some(false));
    assert_eq!(field(&rows[1], "overlaps_printed").as_bool(), Some(true));
    assert_eq!(field(&rows[2], "overlaps_printed").as_bool(), Some(false));
    let flow = &field(&value, "flows").as_array().unwrap()[4];
    assert_eq!(field(flow, "locator").as_str(), Some("p. 11, year 4"));
    assert_eq!(field(flow, "cash_flow_usd_m").as_str(), Some("20/1"));
    let irr = field(&value, "irr");
    assert_eq!(field(irr, "status").as_str(), Some("converged"));
    let (n, d) = field(irr, "rate")
        .as_str()
        .unwrap()
        .split_once('/')
        .unwrap();
    let n: i128 = n.parse().unwrap();
    let d: i128 = d.parse().unwrap();
    assert!(20 * n > 3 * d && 25 * n < 4 * d);
    let md = run(MODEL, "markdown");
    assert!(md.status.success());
    assert!(
        String::from_utf8(md.stdout)
            .unwrap()
            .contains("Compatible convention **MidYearAll**")
    );
    let rounded = MODEL
        .replace("flow_step = \"0\"", "flow_step = \"0.001\"")
        .replace("irr_lo = \"0.05\"\nirr_hi = \"0.25\"\n", "");
    let rounded = payload(&rounded);
    assert_eq!(
        field(&rounded, "compatible_convention").as_str(),
        Some("MidYearAll")
    );
    assert_eq!(
        field(field(&rounded, "irr"), "status").as_str(),
        Some("not_requested")
    );
    assert_ne!(field(&rounded, "comparisons"), field(&value, "comparisons"));
}

#[test]
fn tax_schedule_preserves_effective_flows_and_irr_refusal_preserves_npv() {
    let after = MODEL
        .replace("pre_tax", "after_tax")
        .replace(
            "amount = \"-100\"",
            "amount = \"-100\"\ntax_outflow = \"0\"\ntax_locator = \"p. 12, year 0\"",
        )
        .replace(
            "amount = \"30\"",
            "amount = \"35\"\ntax_outflow = \"5\"\ntax_locator = \"p. 12, year 1\"",
        )
        .replace(
            "amount = \"40\"",
            "amount = \"45\"\ntax_outflow = \"5\"\ntax_locator = \"p. 12, year 2\"",
        )
        .replace(
            "amount = \"50\"",
            "amount = \"55\"\ntax_outflow = \"5\"\ntax_locator = \"p. 12, year 3\"",
        )
        .replace(
            "amount = \"20\"",
            "amount = \"25\"\ntax_outflow = \"5\"\ntax_locator = \"p. 12, year 4\"",
        )
        .replace("irr_hi = \"0.25\"", "irr_hi = \"0.06\"");
    let value = payload(&after);
    assert_eq!(field(&value, "tax_basis").as_str(), Some("after_tax"));
    assert_eq!(
        field(&value, "comparisons"),
        field(&payload(MODEL), "comparisons")
    );
    let flows = field(&value, "flows").as_array().unwrap();
    assert_eq!(
        field(&flows[1], "tax_locator").as_str(),
        Some("p. 12, year 1")
    );
    let irr = field(&value, "irr");
    assert_eq!(field(irr, "status").as_str(), Some("refused"));
    assert_eq!(field(irr, "code").as_str(), Some("FMINE-REF-207"));
    assert!(irr.get("rate").is_none());
}

#[test]
fn ambiguous_timing_missing_tax_and_invalid_schedules_are_refused() {
    let cases = [
        (
            MODEL
                .replace("discount_rate = \"0.05\"", "discount_rate = \"0\"")
                .replace("\"25.1\"", "\"40\""),
            "FMINE-REF-104",
        ),
        (MODEL.replace("\"25.1\"", "\"80\""), "FMINE-REF-104"),
        (MODEL.replace("pre_tax", "after_tax"), "FMINE-REF-107"),
        (
            MODEL.replace("year = \"3\"", "year = \"4\""),
            "FMINE-REF-105",
        ),
        (
            MODEL.replace("flow_step = \"0\"", "flow_step = \"0.00001\""),
            "FMINE-REF-105",
        ),
    ];
    for (input, code) in cases {
        let out = run(&input, "json");
        assert!(!out.status.success());
        let body = parse(&out.stdout).unwrap();
        assert_eq!(field(field(&body, "refusal"), "code").as_str(), Some(code));
    }
}

#[test]
fn taguas_report_reproduces_only_with_the_first_period_timing_shift() {
    let value = payload(TAGUAS);
    assert_eq!(
        field(&value, "compatible_convention").as_str(),
        Some("EndYear")
    );
    assert_eq!(
        field(&value, "year_zero_timing").as_str(),
        Some("end_of_first_period")
    );
    assert_eq!(field(&value, "flows").as_array().unwrap().len(), 13);
    assert_eq!(
        field(field(&value, "source_document"), "declared_sha256").as_str(),
        Some("74ef05e198a0b08ac6aecf8369546d69808b72d06fc5560a2ce829f88822ae7d")
    );
    let rows = field(&value, "comparisons").as_array().unwrap();
    let bounds = field(&rows[0], "npv_bounds_usd_m").as_array().unwrap();
    // Independently sum the printed values and half-steps as Fractions with exponents 1–13.
    assert_eq!(
        bounds[0].as_str(),
        Some("2241714205505561680583/21613627482767873424")
    );
    assert_eq!(
        bounds[1].as_str(),
        Some("33628275521617059419387/324204412241518101360")
    );
    assert_eq!(field(&rows[1], "overlaps_printed").as_bool(), Some(false));
    assert_eq!(field(&rows[2], "overlaps_printed").as_bool(), Some(false));
    let readable = run(TAGUAS, "markdown");
    assert!(readable.status.success());
    let markdown = String::from_utf8(readable.stdout).unwrap();
    assert!(markdown.contains("| EndYear | 103.7176 | 103.7255 | true |"));
    assert!(markdown.contains("Use JSON for exact values."));
    let at_valuation = run(
        &TAGUAS.replace("end_of_first_period", "valuation_date"),
        "json",
    );
    assert!(!at_valuation.status.success());
    let body = parse(&at_valuation.stdout).unwrap();
    assert_eq!(
        field(field(&body, "refusal"), "code").as_str(),
        Some("FMINE-REF-104")
    );
    let outside_range = run(&TAGUAS.replace("\"104\"", "\"140\""), "json");
    assert!(!outside_range.status.success());
    let body = parse(&outside_range.stdout).unwrap();
    let refusal = field(&body, "refusal");
    assert_eq!(field(refusal, "code").as_str(), Some("FMINE-REF-104"));
    assert!(
        field(refusal, "what")
            .as_str()
            .unwrap()
            .contains("year-zero reference")
    );
}
