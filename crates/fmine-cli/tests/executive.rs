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
fn delayed_copper_receipt_creates_an_intraday_shortfall_despite_identical_closing_cash() {
    let c = card("hedge");
    let summary = c.get("liquidity").expect("liquidity summary");
    let get = |o: &Json, key: &str| o.get(key).and_then(Json::as_str).expect("field").to_owned();
    assert_eq!(get(summary, "latest_collection_date"), "2030-03-05");
    let advance = summary
        .get("advance_against_delayed_invoice")
        .expect("invoice split");
    assert_eq!(get(advance, "amount_usd"), "4000000");
    assert_eq!(get(advance, "by"), "2030-03-05");
    assert_eq!(get(advance, "remaining_on_delayed_date_usd"), "7000000");
    let planned = summary.get("planned").expect("original date");
    let delayed = summary.get("delayed").expect("delayed date");
    assert_eq!(get(planned, "additional_line_usd"), "0");
    assert_eq!(planned.get("first_shortfall"), Some(&Json::Null));
    for case in [planned, delayed] {
        assert_eq!(get(case, "closing_cash_before_financing_usd"), "15000000");
    }
    assert_eq!(get(delayed, "peak_draw_needed_usd"), "7000000");
    assert_eq!(get(delayed, "additional_line_usd"), "4000000");
    for (key, date, gap) in [
        ("first_shortfall", "2030-03-06", "2000000"),
        ("peak_shortfall", "2030-03-07", "4000000"),
    ] {
        let point = delayed.get(key).expect("funding checkpoint");
        assert_eq!(
            (
                get(point, "date"),
                get(point, "phase"),
                get(point, "additional_usd")
            ),
            (date.to_owned(), "calls".to_owned(), gap.to_owned())
        );
    }
    let rows = c.get("rows").and_then(Json::as_array).expect("two ledgers");
    assert_eq!(rows.len(), 26);
    let checkpoint = rows
        .iter()
        .filter_map(Json::as_array)
        .find(|r| {
            r[0].as_str() == Some("delayed")
                && r[1].as_str() == Some("2030-03-07")
                && r[2].as_str() == Some("calls")
        })
        .expect("binding call");
    let amounts: Vec<_> = checkpoint[3..]
        .iter()
        .map(|x| x.as_str().expect("whole USD"))
        .collect();
    assert_eq!(
        amounts,
        [
            "-3000000", "0", "0", "1000000", "3000000", "7000000", "4000000"
        ]
    );
    let paper = run("hedge", true);
    assert!(paper.status.success());
    let text = String::from_utf8(paper.stdout).expect("Markdown");
    assert!(
        text.contains("collect by 2030-03-05")
            && text.contains("NB-47")
            && text.contains("synthetic")
    );
}

#[test]
fn nickel_price_supports_continuing_but_leaves_a_mine_on_care_below_restart_price() {
    let c = card("care");
    let result = c.get("operating_review").expect("review");
    assert_eq!(
        result.get("starting_state").and_then(Json::as_str),
        Some("operating")
    );
    let states = result.get("states").expect("both starting states");
    for (state, winner, threshold, advantage) in [
        ("operating", "continue", "10000", "1000000"),
        ("care", "remain_on_care", "13750", "1300000"),
    ] {
        let analysis = states.get(state).expect("state");
        let funded = analysis
            .get("funded_best")
            .and_then(Json::as_array)
            .expect("funded choices");
        assert_eq!(funded.len(), 1);
        assert_eq!(funded[0].as_str(), Some(winner));
        assert_eq!(
            analysis
                .get("advantage_over_next_funded_usd")
                .and_then(Json::as_str),
            Some(advantage)
        );
        for field in ["tie_usd_per_tonne", "first_whole_usd_per_tonne"] {
            assert_eq!(analysis.get(field).and_then(Json::as_str), Some(threshold));
        }
    }
    let rows = c.get("rows").and_then(Json::as_array).expect("choices");
    assert_eq!(rows.len(), 6);
    for (label, cash, value) in [
        ("continue", "-1000000", "-4000000"),
        ("suspend", "-2000000", "-5000000"),
        ("close from operating", "-4700000", "-5900000"),
        ("restart", "-3000000", "-6000000"),
        ("remain on care", "-800000", "-3800000"),
        ("close from care", "-3900000", "-5100000"),
    ] {
        let row = rows
            .iter()
            .filter_map(Json::as_array)
            .find(|r| r[1].as_str() == Some(label))
            .expect("route");
        assert_eq!(
            (row[4].as_str(), row[6].as_str()),
            (Some(cash), Some(value))
        );
    }
}

#[test]
fn common_deck_reverses_gold_investment_ranking_and_retains_the_original_terms() {
    let payload = card("deck");
    let deck = payload.get("price_deck").expect("price deck");
    let get = |o: &Json, key: &str| o.get(key).and_then(Json::as_str).expect("field").to_owned();
    let winners = |key: &str| {
        deck.get(key)
            .and_then(Json::as_array)
            .expect("winners")
            .iter()
            .map(|v| v.as_str().expect("label"))
            .collect::<Vec<_>>()
    };
    assert_eq!(winners("submitted_winners"), ["Higher-power upgrade"]);
    assert_eq!(winners("common_winners"), ["Base mill upgrade"]);
    assert_eq!(deck.get("project_order_reversed"), Some(&Json::Bool(true)));
    assert_eq!(get(deck, "submitted_first_minus_second_usd"), "-23326/5");
    assert_eq!(get(deck, "common_first_minus_second_usd"), "4538/5");
    assert_eq!(
        row(&payload, "Higher-power upgrade"),
        [
            "Higher-power upgrade",
            "US$11,257.60",
            "US$5,684.80",
            "-US$5,572.80"
        ]
    );
    let second = &deck
        .get("candidates")
        .and_then(Json::as_array)
        .expect("candidates")[1];
    let prices = second
        .get("prices")
        .and_then(Json::as_array)
        .expect("prices");
    let hard = prices
        .iter()
        .find(|p| get(p, "period") == "first" && get(p, "state") == "hard")
        .expect("first-period hard ore");
    assert_eq!(get(hard, "submitted_usd_per_kg"), "85000");
    assert_eq!(get(hard, "common_usd_per_kg"), "70000");
    let calculations = deck
        .get("source_calculations")
        .and_then(Json::as_array)
        .expect("source calculations");
    assert_eq!(calculations.len(), 1, "shared manifest is loaded once");
    let submitted = calculations[0]
        .get("submitted")
        .expect("original calculation");
    assert!(
        get(
            submitted.get("manifest").expect("original manifest"),
            "submitted_worksheet"
        )
        .contains("sponsor_power_terms.toml")
    );
    assert_ne!(
        get(
            submitted.get("investment").expect("original investment"),
            "digest"
        ),
        get(
            calculations[0]
                .get("common_deck")
                .expect("replayed investment"),
            "digest"
        )
    );
    let full = String::from_utf8(run("deck", true).stdout).expect("full Markdown");
    assert!(full.contains(&get(deck, "digest")) && full.contains("C-4 row 4"));
    assert_eq!(run("deck", false).stdout, run("deck", false).stdout);
}

#[test]
fn interruption_card_preserves_the_loss_bridge_and_both_exclusion_outcomes() {
    let c = card("interruption");
    assert_eq!(row(&c, "Economic loss")[1], "US$6,000,000");
    assert_eq!(row(&c, "Policy-eligible loss")[1], "US$4,000,000");
    let bridge = c.get("indemnity").expect("indemnity");
    let exact = |obj: &Json| {
        let field = |k| {
            obj.get(k)
                .and_then(Json::as_str)
                .expect("half of a bound")
                .to_owned()
        };
        (field("lower"), field("upper"))
    };
    let branches = bridge
        .get("conditional_recovery")
        .and_then(Json::as_array)
        .expect("branches");
    assert_eq!(branches.len(), 2);
    for (branch, condition, amount) in [
        (&branches[0], "no_exclusion_applies", "3000000"),
        (&branches[1], "exclusion_applies", "0"),
    ] {
        assert_eq!(
            branch.get("condition").and_then(Json::as_str),
            Some(condition)
        );
        assert_eq!(
            exact(branch.get("usd").expect("branch amounts")),
            (amount.into(), amount.into())
        );
    }
    let steps = bridge
        .get("monetary_steps")
        .and_then(Json::as_array)
        .expect("two terms");
    assert_eq!(steps.len(), 2);
    assert_eq!(
        steps[0].get("clause").and_then(Json::as_str),
        Some("Schedule B, item 2")
    );
    assert_eq!(
        exact(steps[0].get("remaining_usd").expect("after deductible")),
        ("3500000".into(), "3500000".into())
    );
    assert_eq!(
        exact(steps[1].get("remaining_usd").expect("after limit")),
        ("3000000".into(), "3000000".into())
    );
    assert!(bridge.get("coverage_verdict").is_none());
    let full = String::from_utf8(run("interruption", true).stdout).expect("Markdown");
    assert!(full.contains("§6.4") && full.contains("whether any unresolved exclusion applies"));
    let binary = run("interruption", false);
    let envelope = parse(&binary.stdout).expect("full envelope");
    let flags = envelope.get("flags").expect("flags");
    for flag in [
        "reserve_disclosure_claim",
        "execution_authorized",
        "geotechnical_signoff",
        "regulatory_compliance_claim",
    ] {
        assert_eq!(flags.get(flag), Some(&Json::Bool(false)));
    }
    assert_eq!(binary.stdout, run("interruption", false).stdout);
}

#[test]
fn early_works_values_both_paths_when_elapsed_clock_still_requires_confirmation() {
    let payload = card("permit");
    let permit = payload.get("permit_gate").expect("permit");
    let works = permit.get("early_works").expect("early works");
    assert_eq!(permit.get("main_terms_admitted"), Some(&Json::Bool(false)));
    assert_eq!(works.get("comparison_eligible"), Some(&Json::Bool(true)));
    for (field, value) in [
        ("spend_usd", "6000000"),
        ("maximum_spend_preserving_floor_usd", "7000000"),
        ("cash_slack_usd", "1000000"),
        ("schedule_value_usd", "9000000"),
        ("gain_if_proceed_usd", "8000000"),
        ("loss_if_no_proceed_usd", "5000000"),
        ("break_even_proceed_probability", "5/13"),
    ] {
        assert_eq!(
            works.get(field).and_then(Json::as_str),
            Some(value),
            "{field}"
        );
    }
    assert!(finding(&payload, "2030-04-01"));
    assert!(finding(&payload, "main works remain on hold"));
    assert!(finding(&payload, "5/13 (38.46%)"));
    let binary = run("permit", false);
    let envelope = parse(&binary.stdout).expect("valid JSON");
    let flags = envelope.get("flags").expect("flags");
    for flag in [
        "reserve_disclosure_claim",
        "execution_authorized",
        "geotechnical_signoff",
        "regulatory_compliance_claim",
    ] {
        assert_eq!(flags.get(flag), Some(&Json::Bool(false)));
    }
    assert_eq!(binary.stdout, run("permit", false).stdout);
    let markdown = run("permit", true);
    assert!(markdown.status.success());
    let text = String::from_utf8(markdown.stdout).expect("Markdown");
    assert!(text.contains("Synthetic clock pause and restart notice NR-13"));
    assert!(text.contains("-US$5,000,000"));
}

#[test]
fn staged_permit_binary_preserves_blocked_paths_and_the_source_bound_comparison() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let file = "tests/capital/staged_permit.toml";
    let run = |flags: &[&str]| {
        let result = Command::new(env!("CARGO_BIN_EXE_fmine"))
            .current_dir(&root)
            .args(["decide", "program", file])
            .args(flags)
            .output()
            .expect("staged capital CLI");
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stdout)
        );
        result.stdout
    };
    let bytes = run(&["--json"]);
    assert_eq!(bytes, run(&["--json"]));
    let envelope = parse(&bytes).expect("JSON envelope");
    let payload = envelope.get("payload").expect("payload");
    let hash = sha256(&std::fs::read(root.join(file)).expect("source bytes")).to_string();
    assert_eq!(
        payload
            .get("source")
            .and_then(|s| s.get("sha256"))
            .and_then(Json::as_str),
        Some(hash.as_str())
    );
    let program = payload.get("program").expect("program");
    let summary = program.get("summary").expect("policy summary");
    assert_eq!(
        summary.get("net_adaptive_value").and_then(Json::as_str),
        Some("1500000/1")
    );
    let static_summary = program
        .get("static_comparison")
        .and_then(|v| v.get("summary"))
        .expect("static summary");
    let review = static_summary
        .get("permit_reviews")
        .and_then(Json::as_array)
        .expect("all gate outcomes");
    assert_eq!(review.len(), 2);
    assert_eq!(
        review[1].get("state_id").and_then(Json::as_str),
        Some("pending")
    );
    assert_eq!(
        review[1]
            .get("review")
            .and_then(|r| r.get("status"))
            .and_then(Json::as_str),
        Some("clock_elapsed")
    );
    for flag in [
        "reserve_disclosure_claim",
        "execution_authorized",
        "geotechnical_signoff",
        "regulatory_compliance_claim",
    ] {
        assert_eq!(program.get(flag), Some(&Json::Bool(false)));
    }
    let paper = String::from_utf8(run(&["--format", "markdown"])).expect("Markdown");
    assert!(paper.contains("| NRC-42 | cleared | permission_documented | 2030-04-01 |"));
    assert!(paper.contains("| NRC-42 | pending | clock_elapsed | 2030-04-01 |"));
    let brief = String::from_utf8(run(&["--brief"])).expect("committee brief");
    for text in [
        "**Best precommitted choice** Hold cash. **Net adaptive value** US$1,500,000.",
        "**Initial commitment** early. **Initial planned cost** US$6,000,000.",
        "**Review on 2030-04-02**",
        "| clearance-documented | 1/2 | main | US$4,000,000 |",
        "| no-clearance-documented | 1/2 | No additional commitment | US$0 |",
        "| US$1,500,000 | US$5,000,000 | US$6,000,000 | US$2,000,000 | Current co-winner |",
        "| NRC-42 | pending | clock elapsed | 2030-04-01 |",
        "main requires documented permission for NRC-42",
        "Comparison digest",
        &hash,
    ] {
        assert!(brief.contains(text), "missing {text}\n{brief}");
    }
    assert!(brief.len() * 2 < paper.len());
    assert_eq!(bytes, run(&["--json"]));
}

#[test]
fn staged_equity_brief_keeps_shared_outcome_actions_and_exact_per_share_value() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let result = Command::new(env!("CARGO_BIN_EXE_fmine"))
        .current_dir(root)
        .args([
            "decide",
            "program",
            "tests/capital/recourse.toml",
            "--brief",
        ])
        .output()
        .expect("staged equity brief");
    assert!(result.status.success());
    let brief = String::from_utf8(result.stdout).expect("Markdown");
    for text in [
        "**Net adaptive value** 19/120 USD/share.",
        "Primary limit 1/10 USD/share.",
        "**Initial commitment** study. **Initial planned cost** US$10.",
        "| bad | 1/2 | No additional commitment | US$0 |",
        "| good | 1/2 | issue + project | US$100 |",
        "| 19/120 USD/share | 1/10 USD/share | US$10 | US$0 | Current co-winner |",
    ] {
        assert!(brief.contains(text), "missing {text}\n{brief}");
    }
    assert!(!brief.contains("**Net adaptive value** US$120"));
}

#[test]
fn executive_briefs_keep_decisive_terms_alternatives_and_the_full_worksheet_digest() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let brief = |model: &str, flags: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_fmine"))
            .current_dir(&root)
            .args([
                "decide",
                "program",
                &format!("tests/capital/{model}.toml"),
                "--brief",
            ])
            .args(flags)
            .output()
            .expect("run brief")
    };
    for name in [
        "hedge",
        "care",
        "deck",
        "interruption",
        "permit",
        "purchase",
        "purchase_potash",
        "rehabilitation",
        "tenure",
        "group",
        "estimates",
    ] {
        let c = card(name);
        let result = brief(&format!("executive_{name}"), &[]);
        assert!(result.status.success(), "{name}");
        let text = String::from_utf8(result.stdout).expect("Markdown");
        let full = String::from_utf8(run(name, true).stdout).expect("full Markdown");
        let digest = c
            .get("source")
            .and_then(|s| s.get("sha256"))
            .and_then(Json::as_str)
            .expect("digest");
        assert!(
            text.contains(digest) && text.contains("Uncertainty:") && text.contains("Revisit if:")
        );
        assert!(full.contains("(known ") && !text.contains("(known "));
        for finding in full
            .lines()
            .filter(|line| line.starts_with("- ") && !line.contains("(known "))
        {
            assert!(
                text.lines().any(|line| line == finding),
                "{name}: {finding}"
            );
        }
        if name == "hedge" {
            assert!(text.contains("US$4,000,000 of the invoice by 2030-03-05"));
            assert!(text.contains("remaining US$7,000,000 on 2030-03-08"));
            assert!(text.contains("First shortfall of US$2,000,000 on 2030-03-06"));
            assert!(text.contains("Closing cash before financing is US$15,000,000"));
            assert!(full.contains("| delayed |") && !text.contains("| delayed |"));
        } else {
            let alternatives: Vec<_> = full.lines().filter(|line| line.starts_with('|')).collect();
            assert!(!alternatives.is_empty() && alternatives.iter().all(|&row| text.contains(row)));
        }
        assert!(text.len() < full.len(), "{name}");
    }
    for flags in [&["--json"][..], &["--format", "json"], &["--brief"]] {
        assert_eq!(brief("executive_hedge", flags).status.code(), Some(2));
    }
    assert_eq!(brief("kpi_constraints", &[]).status.code(), Some(2));
    assert_eq!(brief("equity", &[]).status.code(), Some(3));
}

#[test]
fn actual_capital_brief_names_the_current_choice_and_next_exact_limit() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let run = |name: &str, flag: &str| {
        let result = Command::new(env!("CARGO_BIN_EXE_fmine"))
            .current_dir(&root)
            .args([
                "decide",
                "program",
                &format!("tests/capital/{name}.toml"),
                flag,
            ])
            .output()
            .expect("capital CLI");
        assert!(result.status.success(), "{name}");
        String::from_utf8(result.stdout).expect("UTF-8")
    };
    let gb30 = run("gb30", "--brief");
    for text in [
        "**Current co-winners:** A + D.",
        "**Before the tail-loss limit:** A + E.",
        "US$130,820,000",
        "**First higher-value limit:** US$210,000,000 (US$60,000,000 more).",
        "| A + D | US$241,180,000 | US$130,600,000 |",
        "| Hold cash |",
        "Cash slack is unavailable",
    ] {
        assert!(gb30.contains(text), "missing {text}");
    }
    for name in ["gb30", "production_investments"] {
        let report = parse(run(name, "--json").as_bytes()).expect("comparison JSON");
        let payload = report.get("payload").expect("payload");
        let digest = payload
            .get("program")
            .and_then(|p| p.get("digest"))
            .and_then(Json::as_str)
            .expect("program digest");
        let brief = run(name, "--brief");
        assert!(brief.contains(digest), "{name}");
        if name == "production_investments" {
            assert!(
                brief.contains("Mean discounted incremental cash")
                    && brief.contains("Source receipt digest")
            );
            let source = payload
                .get("digest")
                .and_then(Json::as_str)
                .expect("source digest");
            assert!(brief.contains(source));
        }
    }
    let stress = run("wasserstein", "--brief");
    assert!(stress.contains("scenario-weight stress") && stress.contains("every stressed law"));
    assert!(run("selection_stability", "--brief").contains("Conditional resampling"));
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
