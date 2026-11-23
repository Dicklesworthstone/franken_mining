//! FACT: known answers for invented procurement, land, closure, group and capital decisions.
//! Determinism: Exact
#![allow(clippy::expect_used, clippy::panic)]

use fmine_core::hash::sha256;
use fmine_io::omf::json::{Json, parse};
use std::path::Path;
use std::process::Command;

fn run(file: &str, markdown: bool) -> std::process::Output {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut command = Command::new(env!("CARGO_BIN_EXE_fmine"));
    command.current_dir(&root).args([
        "decide",
        "program",
        &format!("tests/capital/executive_{file}.toml"),
    ]);
    if markdown {
        command.args(["--format", "markdown"]);
    } else {
        command.arg("--json");
    }
    command.output().expect("run CLI")
}

fn card(file: &str) -> Json {
    let result = run(file, false);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stdout)
    );
    let envelope = parse(&result.stdout).expect("valid JSON");
    let payload = envelope.get("payload").expect("payload");
    let worksheet = payload
        .get("worksheet")
        .and_then(Json::as_str)
        .expect("source");
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(format!("../../tests/capital/executive_{file}.toml"));
    assert_eq!(std::fs::read(path).expect("fixture"), worksheet.as_bytes());
    assert_eq!(
        payload
            .get("source")
            .and_then(|s| s.get("sha256"))
            .and_then(Json::as_str),
        Some(sha256(worksheet.as_bytes()).to_string().as_str())
    );
    assert_eq!(
        payload.get("evidence").and_then(Json::as_str),
        Some("synthetic")
    );
    payload.clone()
}
fn row<'a>(card: &'a Json, label: &str) -> Vec<&'a str> {
    card.get("rows")
        .and_then(Json::as_array)
        .expect("rows")
        .iter()
        .map(|r| r.as_array().expect("columns"))
        .find(|r| r[0].as_str() == Some(label))
        .expect("named result")
        .iter()
        .map(|v| v.as_str().expect("display value"))
        .collect()
}
fn finding(c: &Json, needle: &str) -> bool {
    c.get("findings")
        .and_then(Json::as_array)
        .expect("findings")
        .iter()
        .any(|s| s.as_str().is_some_and(|s| s.contains(needle)))
}

#[test]
fn procurement_preserves_the_competing_project_and_prices_only_critical_delay() {
    let c = card("purchase");
    assert!(finding(&c, "Hold (250000 USD)"));
    assert!(finding(&c, "probability 1/20"));
    let limits = c.get("procurement_limits").expect("negotiation terms");
    for (key, expected) in [
        ("max_hold_fee_usd", "500000"),
        ("fixed_exercise_credit_usd", "0"),
        ("max_replacement_price_usd", "20500000"),
        ("latest_on_time_delivery", "2030-11-27"),
    ] {
        assert_eq!(limits.get(key).and_then(Json::as_str), Some(expected));
    }
    assert!(finding(
        &c,
        "reduce the price by at least 1000000 USD and bring delivery forward by at least 18 days"
    ));
    assert_eq!(
        row(&c, "Order / cancel"),
        [
            "Order / cancel",
            "3000000",
            "2000000",
            "5000000",
            "not applicable",
            "not applicable",
            "-1500000",
            "22000000",
            "15500000"
        ]
    );
    assert_eq!(
        row(&c, "Hold / proceed"),
        [
            "Hold / proceed",
            "250000",
            "20000000",
            "20250000",
            "0",
            "0",
            "1250000",
            "6750000",
            "250000"
        ]
    );
    assert_eq!(
        row(&c, "Lapse / proceed"),
        [
            "Lapse / proceed",
            "0",
            "21500000",
            "21500000",
            "18",
            "1440000",
            "1500000",
            "5500000",
            "-1000000"
        ]
    );
    assert!(finding(&c, "displaces North Ridge substation"));
    let printed = run("purchase", true);
    assert!(printed.status.success());
    let md = String::from_utf8(printed.stdout).expect("UTF8");
    assert!(md.contains("Hold (250000 USD)") && md.contains("VD-17") && md.contains("synthetic"));
    assert_eq!(run("purchase", false).stdout, run("purchase", false).stdout);
}
#[test]
fn second_asset_credits_the_option_and_a_later_delivery_consumes_only_float() {
    let c = card("purchase_potash");
    assert!(finding(&c, "Lapse (0 USD)"));
    assert!(finding(&c, "probability 1/34"));
    let limits = c.get("procurement_limits").expect("negotiation terms");
    for (key, expected) in [
        ("max_hold_fee_usd", "1100000"),
        ("fixed_exercise_credit_usd", "100000"),
        ("max_replacement_price_usd", "9000000"),
        ("latest_on_time_delivery", "2030-09-16"),
    ] {
        assert_eq!(limits.get(key).and_then(Json::as_str), Some(expected));
    }
    for outcome in ["Order / cancel", "Hold / cancel", "Lapse / cancel"] {
        assert_eq!(&row(&c, outcome)[4..6], ["not applicable"; 2]);
    }
    assert_eq!(
        &row(&c, "Hold / proceed")[1..4],
        ["150000", "7900000", "8050000"]
    );
    assert_eq!(&row(&c, "Lapse / proceed")[4..6], ["0", "0"]);
}
#[test]
fn rehabilitation_separates_cash_credit_surety_and_rebonding() {
    let c = card("rehabilitation");
    assert_eq!(
        &row(&c, "North waste-rock cover")[4..],
        ["1510000", "0", "0"]
    );
    assert_eq!(
        &row(&c, "East stockpile rehabilitation")[4..],
        ["-575000", "4200000", "1000000"]
    );
}
#[test]
fn tenure_retains_overlapping_economic_ranges_and_blind_survey() {
    let c = card("tenure");
    assert!(finding(&c, "insufficiently sensitive") && finding(&c, "ranges overlap"));
    assert_eq!(&row(&c, "farm_out")[2..4], ["210000", "740000"]);
    assert_eq!(&row(&c, "surrender")[2..4], ["-630000", "-280000"]);
}
#[test]
fn group_recovery_converts_project_to_attributable_once() {
    let c = card("group");
    assert!(finding(
        &c,
        "Mesa must supply at least 700 above its baseline"
    ));
    assert_eq!(&row(&c, "Valley")[1..5], ["6000", "6000", "4800", "5100"]);
    assert!(
        finding(&c, "current plan 12000")
            && finding(&c, "increase needed 1000")
            && finding(&c, "before deadlines 1300")
    );
}
#[test]
fn approval_cohort_scores_holdout_without_later_closeout_or_silent_drops() {
    let c = card("estimates");
    assert!(finding(&c, "5 approved projects; 1 comparable"));
    assert!(finding(&c, "original 2500000 USD; adjusted 500000 USD"));
    assert_eq!(row(&c, "East circuit")[6], "unavailable before holdout");
    assert_eq!(row(&c, "Northern transformer")[5], "cancelled");
    assert_eq!(row(&c, "Main transformer")[5], "scope_changed");
}
