//! FACT: source-derived portfolio known answers and whole-command source admission controls.
//! Determinism: DeterministicPerIsa (inherited production); expected financial answers are exact.

#![allow(clippy::expect_used)]

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use fmine_cli::{Envelope, OutputFormat, invoke};
use fmine_core::hash::sha256;
use fmine_io::omf::json::{Json, parse};
use fmine_numerics::exact_feas::Rat;

const SOURCE: &str = include_str!("../../../tests/decisions/production_investment.toml");
const PORTFOLIO: &str = include_str!("../../../tests/capital/production_investments.toml");
static NEXT: AtomicU64 = AtomicU64::new(0);

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("workspace root")
}

fn binary() -> PathBuf {
    if let Some(path) = std::env::var_os("FMINE_TEST_BINARY") {
        return PathBuf::from(path);
    }
    let executable = std::env::current_exe().expect("test executable");
    executable
        .ancestors()
        .find(|path| {
            path.file_name()
                .is_some_and(|name| name == "deps" || name == "build")
        })
        .and_then(Path::parent)
        .expect("Cargo profile directory")
        .join(format!("fmine{}", std::env::consts::EXE_SUFFIX))
}

fn temporary() -> PathBuf {
    let p = std::env::temp_dir().join(format!(
        "fmine-capital-investments-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&p).expect("temporary source directory");
    p
}

fn write(dir: &Path, name: &str, text: &str) -> PathBuf {
    let path = dir.join(name);
    std::fs::write(&path, text).expect("write synthetic fixture");
    path
}

fn args(path: &Path) -> Vec<String> {
    vec![
        "decide".into(),
        "program".into(),
        path.to_string_lossy().into_owned(),
    ]
}

fn run(path: &Path) -> Envelope {
    let (envelope, format) = invoke(&args(path), &root());
    assert_eq!(format, OutputFormat::Json);
    envelope
}

fn field<'a>(value: &'a Json, keys: &[&str]) -> &'a Json {
    keys.iter().fold(value, |v, k| v.get(k).expect(k))
}

fn json(envelope: &Envelope) -> Json {
    assert_eq!(envelope.exit_code, 0, "{}", envelope.to_json());
    parse(envelope.payload_json.as_bytes()).expect("portfolio JSON")
}

fn selected(row: &Json) -> Vec<&str> {
    field(row, &["selected"])
        .as_array()
        .expect("selected candidates")
        .iter()
        .map(|v| v.as_str().expect("candidate id"))
        .collect()
}

fn choice<'a>(payload: &'a Json, members: &[&str]) -> &'a Json {
    field(payload, &["program", "summary", "choices"])
        .as_array()
        .expect("all subsets")
        .iter()
        .find(|c| selected(c) == members)
        .expect("subset retained")
}

fn winners(payload: &Json, name: &str) -> Vec<Vec<String>> {
    let summary = field(payload, &["program", "summary"]);
    let ids = field(summary, &[name]).as_array().expect("winners");
    field(summary, &["choices"])
        .as_array()
        .expect("choices")
        .iter()
        .filter(|c| ids.iter().any(|id| id == field(c, &["bundle_id"])))
        .map(|c| selected(c).into_iter().map(str::to_owned).collect())
        .collect()
}

fn extra_candidate(source: &Path) -> String {
    format!(
        "\n[[candidate]]\nid = \"aux\"\nlabel = \"Separate synthetic operation\"\ninvestment = \"{}\"\nalternative = \"process\"\n",
        source.to_string_lossy().replace('\\', "\\\\")
    )
}

/// A second explicitly distinct operation with the same conditional scenario law.
/// Its physical and commercial worksheets are all independently source-bound.
fn auxiliary(dir: &Path) -> PathBuf {
    let paths = [
        "tests/plant/production_joint.toml",
        "tests/plant/grinding_joint.toml",
        "tests/plant/grinding_higher_power.toml",
        "tests/plant/production_recovery.toml",
        "tests/decisions/production_valuation.toml",
        "tests/decisions/production_valuation_higher_power.toml",
        "tests/plant/investment_second_production.toml",
        "tests/plant/investment_second_grinding.toml",
        "tests/plant/investment_second_grinding_high.toml",
        "tests/plant/investment_second_recovery.toml",
        "tests/decisions/investment_second_valuation.toml",
        "tests/decisions/investment_second_valuation_high.toml",
    ];
    let support = |s: &str| {
        s.replace("synthetic-plant/v1", "synthetic-auxiliary-plant/v1")
            .replace("synthetic-ball-mill", "synthetic-auxiliary-mill")
    };
    let mut manifest = support(SOURCE).replace(
        "initial_capex_usd = \"1000\"",
        "initial_capex_usd = \"1500\"",
    );
    for (index, source) in paths.iter().enumerate() {
        let original =
            std::fs::read_to_string(root().join(source)).expect("original source fixture");
        let path = write(dir, &format!("aux-{index}.toml"), &support(&original));
        manifest = manifest.replace(source, &path.to_string_lossy().replace('\\', "\\\\"));
    }
    write(dir, "aux-investment.toml", &manifest)
}

fn portfolio(dir: &Path) -> PathBuf {
    let aux = auxiliary(dir);
    let text = PORTFOLIO.replace("budget = \"8000\"", "budget = \"2500\"") + &extra_candidate(&aux);
    write(dir, "portfolio.toml", &text)
}

#[test]
fn source_portfolio_derives_all_subsets_without_transcribing_returns_or_double_charging_capex() {
    let dir = temporary();
    let path = portfolio(&dir);
    let envelope = run(&path);
    let payload = json(&envelope);
    assert_eq!(
        winners(&payload, "feasible_winners"),
        vec![vec!["aux", "original"]]
    );
    assert_eq!(
        field(&payload, &["program", "summary", "choices"])
            .as_array()
            .expect("subsets")
            .len(),
        8
    );
    // Independent arithmetic from the two source cash periods:
    // original: [808,9315.2]*(1+1/2)-1000 = [212,12972.8], mean 6592.4;
    // auxiliary: the same cash less 1500 = [-288,12472.8], mean 6092.4.
    // Combined: [-76,25445.6], mean 12684.8, spend 2500, worst-half signed loss 76.
    let both = choice(&payload, &["aux", "original"]);
    assert_eq!(
        field(both, &["statistics", "mean"]).as_str(),
        Some("63424/5")
    );
    assert_eq!(field(both, &["spend"]).as_str(), Some("2500/1"));
    assert_eq!(field(both, &["budget_slack"]).as_str(), Some("0/1"));
    assert_eq!(field(both, &["tail_loss"]).as_str(), Some("76/1"));
    assert!(
        !field(
            choice(&payload, &["higher", "original"]),
            &["structural_rejections"]
        )
        .as_array()
        .expect("exclusion")
        .is_empty()
    );
    assert_eq!(
        field(&payload, &["program", "summary", "objective"]).as_str(),
        Some("expected_discounted_incremental_cash")
    );
    assert_eq!(envelope.to_json(), run(&path).to_json());
    let actual = Command::new(binary())
        .args(args(&path))
        .arg("--workspace")
        .arg(root())
        .output()
        .expect("actual binary");
    assert!(
        actual.status.success(),
        "{}",
        String::from_utf8_lossy(&actual.stdout)
    );
    assert_eq!(
        String::from_utf8_lossy(&actual.stdout).trim_end(),
        envelope.to_json()
    );
    let markdown = Command::new(binary())
        .args(args(&path))
        .args(["--format", "markdown", "--workspace"])
        .arg(root())
        .output()
        .expect("actual Markdown");
    assert!(markdown.status.success());
    assert_eq!(
        String::from_utf8_lossy(&markdown.stdout),
        format!(
            "{}\n",
            envelope.rendered_markdown.as_deref().expect("Markdown")
        )
    );
}

#[test]
fn source_portfolio_retains_original_bytes_digests_and_claim_ancestry() {
    let dir = temporary();
    let path = portfolio(&dir);
    let envelope = run(&path);
    let payload = json(&envelope);
    let source_text = std::fs::read_to_string(&path).expect("portfolio source");
    assert_eq!(
        field(&payload, &["manifest", "submitted_worksheet"]).as_str(),
        Some(source_text.as_str())
    );
    assert_eq!(
        field(&payload, &["manifest", "source", "sha256"]).as_str(),
        Some(sha256(source_text.as_bytes()).to_hex().as_str())
    );
    let sources = field(&payload, &["sources"]).as_array().expect("sources");
    assert_eq!(sources.len(), 2);
    let original = sources
        .iter()
        .find(|s| {
            field(s, &["investment_manifest"]).as_str()
                == Some("tests/decisions/production_investment.toml")
        })
        .expect("original source");
    assert_eq!(
        field(
            original,
            &["calculation", "manifest", "submitted_worksheet"]
        )
        .as_str(),
        Some(SOURCE)
    );
    assert_eq!(
        field(original, &["calculation", "sources"])
            .as_array()
            .expect("all four source bundles")
            .len(),
        4
    );
    let claim = field(&payload, &["program", "claim"]);
    assert_eq!(field(claim, &["evidence"]).as_str(), Some("assumed"));
    assert_eq!(
        field(claim, &["confidentiality"]).as_str(),
        Some("Internal")
    );
    assert!(
        !field(claim, &["parents"])
            .as_array()
            .expect("parents")
            .is_empty()
    );
    let original_sha = sha256(SOURCE.as_bytes()).to_hex();
    assert!(
        field(claim, &["anchors"])
            .as_array()
            .expect("anchors")
            .iter()
            .any(|a| field(a, &["sha256"]).as_str() == Some(original_sha.as_str()))
    );
    let changed = write(
        &dir,
        "commented.toml",
        &(source_text + "\n# Changed source declaration\n"),
    );
    let changed = json(&run(&changed));
    assert_ne!(field(&payload, &["digest"]), field(&changed, &["digest"]));
}

#[test]
fn portfolio_risk_dependencies_and_exclusions_apply_to_complete_source_outcomes() {
    let dir = temporary();
    let path = portfolio(&dir);
    let text = std::fs::read_to_string(path).expect("manifest");
    let limited = text.replace(
        "tail_mass = \"1/2\"",
        "tail_mass = \"1/2\"\nmax_tail_loss = \"0\"",
    );
    let report = json(&run(&write(&dir, "risk.toml", &limited)));
    assert_eq!(
        winners(&report, "unconstrained_winners"),
        vec![vec!["aux", "original"]]
    );
    assert_eq!(winners(&report, "feasible_winners"), vec![vec!["original"]]);
    assert_eq!(
        field(
            &report,
            &["program", "summary", "expected_value_cost_of_risk_limit"]
        )
        .as_str(),
        Some("30462/5")
    );
    assert_eq!(
        field(choice(&report, &["aux", "original"]), &["risk_rejected"]).as_bool(),
        Some(true)
    );
    let equal = limited.replace("max_tail_loss = \"0\"", "max_tail_loss = \"76\"");
    assert_eq!(
        winners(
            &json(&run(&write(&dir, "equal.toml", &equal))),
            "feasible_winners"
        ),
        vec![vec!["aux", "original"]]
    );
    let dependency = text + "\n[[requires]]\ncandidate = \"original\"\nprerequisite = \"higher\"\n";
    let report = json(&run(&write(&dir, "dependency.toml", &dependency)));
    assert_eq!(winners(&report, "feasible_winners"), vec![vec!["aux"]]);
    assert!(
        !field(choice(&report, &["original"]), &["structural_rejections"])
            .as_array()
            .expect("missing prerequisite")
            .is_empty()
    );
}

fn refused(dir: &Path, name: &str, text: &str, why: &str) {
    let envelope = run(&write(dir, name, text));
    assert_ne!(envelope.exit_code, 0, "unexpected successful source bridge");
    assert!(envelope.to_json().contains(why), "{}", envelope.to_json());
    assert!(!envelope.payload_json.contains("\"program\""));
}

#[test]
fn incompatible_or_incomplete_investment_sources_never_yield_a_partial_portfolio() {
    let dir = temporary();
    let path = portfolio(&dir);
    let text = std::fs::read_to_string(path).expect("portfolio");
    let aux = dir.join("aux-investment.toml");
    let original = std::fs::read_to_string(&aux).expect("auxiliary");
    for (from, to, why) in [
        (
            "discount_factor = \"1/2\"",
            "discount_factor = \"1/3\"",
            "identical complete shared",
        ),
        (
            "valuation_time_ns = \"0\"",
            "valuation_time_ns = \"-1\"",
            "identical complete shared",
        ),
        (
            "SYNTHETIC persistent soft or hard ore",
            "DIFFERENT regime labels",
            "identical complete shared",
        ),
        (
            "initial_capex_usd = \"1500\"",
            "initial_capex_usd = \"1500.00001\"",
            "exactly representable",
        ),
        (
            "alternative = \"process\"",
            "alternative = \"missing\"",
            "complete investment-period bundles",
        ),
    ] {
        write(&dir, "aux-investment.toml", &original.replace(from, to));
        refused(&dir, "invalid.toml", &text, why);
    }
    write(&dir, "aux-investment.toml", &original);
    let local_limit = original.clone()
        + "\n[[downside_limit]]\nfloor_usd = \"0\"\nmax_breach_probability = \"1/1\"\n";
    write(&dir, "aux-investment.toml", &local_limit);
    refused(
        &dir,
        "local-limit.toml",
        &text,
        "source investment downside limits",
    );
    write(&dir, "aux-investment.toml", &original);
    // Even an unselected alternative's missing bundle refuses the complete source.
    let missing = original.replace("aux-11.toml", "missing-terms.toml");
    write(&dir, "aux-investment.toml", &missing);
    refused(&dir, "missing.toml", &text, "missing-terms.toml");
}

#[test]
fn foreign_shared_states_and_overlap_across_separate_manifests_refuse() {
    let dir = temporary();
    let path = portfolio(&dir);
    let text = std::fs::read_to_string(path).expect("portfolio");
    // Preserve every source's own complete population while changing one shared
    // state label everywhere in the second operation. Pairing it to the first
    // operation would otherwise fabricate a common scenario.
    for index in 0..12 {
        let path = dir.join(format!("aux-{index}.toml"));
        let source = std::fs::read_to_string(&path).expect("auxiliary source");
        std::fs::write(&path, source.replace("\"soft\"", "\"different-state\""))
            .expect("complete renamed state");
    }
    refused(
        &dir,
        "foreign-state.toml",
        &text,
        "identical complete shared",
    );
    let duplicate = write(&dir, "same-operation.toml", SOURCE);
    refused(
        &dir,
        "cross-source-overlap.toml",
        &(PORTFOLIO.to_owned() + &extra_candidate(&duplicate)),
        "overlapping operating support",
    );
}

#[test]
fn overlapping_sources_duplicate_fields_and_foreign_constraints_refuse() {
    let dir = temporary();
    refused(
        &dir,
        "overlap.toml",
        &PORTFOLIO.replace("exclusion_group = \"same-operating-circuit\"\n", ""),
        "overlapping operating support",
    );
    refused(
        &dir,
        "duplicate.toml",
        &PORTFOLIO.replace("budget = \"8000\"", "budget = \"8000\"\nbudget = \"9000\""),
        "duplicate key",
    );
    refused(
        &dir,
        "unknown.toml",
        &(PORTFOLIO.to_owned()
            + "\n[[requires]]\ncandidate = \"original\"\nprerequisite = \"missing\"\n"),
        "unknown prerequisite",
    );
    refused(
        &dir,
        "payoff.toml",
        &(PORTFOLIO.to_owned()
            + "\n[[payoff]]\ncandidate = \"original\"\nstate = \"soft\"\nvalue = \"99999\"\n"),
        "unsupported table payoff",
    );
    let (docs, _) = invoke(&["robot-docs".into()], &root());
    assert!(docs.payload_json.contains("fmine.capital-investments.v1"));
    assert!(docs.payload_json.contains("production_investments.toml"));
}

fn cash_declaration(opening: &str, first_soft: &str, second_soft: &str) -> String {
    let mut text = String::from(
        r#"
[[funding_schedule]]
cash_basis = "SYNTHETIC nominal USD; baseline excludes investment increments and initial capital"
opening_date = "1970-01-01"
settlement_convention = "source_period_increment_at_cash_period_close"
cash_rounding = "nearest_ties_even_0_0001_usd"

[[cash_period]]
id = "first"
cash_period = "1970-01-01/1970-01-01"
minimum_cash = "0"

[[cash_period]]
id = "second"
cash_period = "1970-01-02/1970-01-02"
minimum_cash = "0"
"#,
    );
    for state in ["soft", "hard"] {
        text.push_str(&format!(
            "\n[[opening_cash]]\nstate = \"{state}\"\nvalue = \"{opening}\"\n"
        ));
        for (period, soft_value) in [("first", first_soft), ("second", second_soft)] {
            let value = if state == "soft" { soft_value } else { "0" };
            text.push_str(&format!(
                "\n[[baseline_cash_flow]]\nperiod = \"{period}\"\nstate = \"{state}\"\nvalue = \"{value}\"\n"
            ));
        }
    }
    text
}

fn checkpoint<'a>(choice: &'a Json, state: &str, period: Option<&str>) -> &'a Json {
    field(choice, &["funding_schedule", "checkpoints"])
        .as_array()
        .expect("complete cash checkpoints")
        .iter()
        .find(|row| {
            field(row, &["state_id"]).as_str() == Some(state)
                && field(row, &["period_id"]).as_str() == period
        })
        .expect("state/period checkpoint")
}

#[test]
fn derived_nominal_cash_pays_initial_capital_once_and_preserves_discounted_risk() {
    let dir = temporary();
    let path = portfolio(&dir);
    let nominal = json(&run(&path));
    let text =
        std::fs::read_to_string(&path).expect("portfolio") + &cash_declaration("2500", "0", "0");
    let path = write(&dir, "funded.toml", &text);
    let envelope = run(&path);
    let payload = json(&envelope);
    let both = choice(&payload, &["aux", "original"]);
    let original = choice(&nominal, &["aux", "original"]);
    assert_eq!(
        field(both, &["statistics"]),
        field(original, &["statistics"])
    );
    assert_eq!(field(both, &["tail_loss"]).as_str(), Some("76/1"));
    assert_eq!(
        winners(&payload, "feasible_winners"),
        vec![vec!["aux", "original"]]
    );
    let opening = checkpoint(both, "soft", None);
    assert_eq!(field(opening, &["opening_cash"]).as_str(), Some("2500/1"));
    assert_eq!(
        field(opening, &["candidate_cash_flow"]).as_str(),
        Some("-2500/1")
    );
    assert_eq!(field(opening, &["closing_cash"]).as_str(), Some("0/1"));
    let first = checkpoint(both, "soft", Some("first"));
    assert_eq!(field(first, &["closing_cash"]).as_str(), Some("1616/1"));
    let second = checkpoint(both, "soft", Some("second"));
    // Two independently derived 808 USD receipts in each period. The 1/2
    // valuation multiplier never discounts nominal funding or charges capex twice.
    assert_eq!(
        field(second, &["candidate_cash_flow"]).as_str(),
        Some("1616/1")
    );
    assert_eq!(field(second, &["closing_cash"]).as_str(), Some("3232/1"));
    assert_eq!(
        field(checkpoint(both, "hard", Some("second")), &["closing_cash"]).as_str(),
        Some("186304/5")
    );
    let funding = field(&payload, &["program", "inputs", "funding_schedule"]);
    let source_sha = sha256(SOURCE.as_bytes()).to_hex();
    assert!(
        field(funding, &["anchors"])
            .as_array()
            .expect("source anchors")
            .iter()
            .any(|a| field(a, &["sha256"]).as_str() == Some(source_sha.as_str()))
    );
    assert_eq!(envelope.to_json(), run(&path).to_json());
    let actual = Command::new(binary())
        .args(args(&path))
        .args(["--workspace"])
        .arg(root())
        .output()
        .expect("funded CLI binary");
    assert!(actual.status.success());
    assert_eq!(
        String::from_utf8_lossy(&actual.stdout).trim_end(),
        envelope.to_json()
    );
    assert!(
        envelope
            .rendered_markdown
            .as_deref()
            .expect("cash Markdown")
            .contains("Funding at declared cash checkpoints")
    );
    assert!(
        run(&portfolio(&dir))
            .no_claim
            .expect("scope")
            .contains("Liquidity is not assessed")
    );
}

#[test]
fn profitable_source_portfolio_cannot_spend_future_receipts_at_opening() {
    let dir = temporary();
    let path = portfolio(&dir);
    let text = std::fs::read_to_string(path).expect("portfolio")
        + &cash_declaration("2499.9999", "0", "0");
    let payload = json(&run(&write(&dir, "opening-shortfall.toml", &text)));
    assert_eq!(
        winners(&payload, "feasible_winners"),
        vec![vec!["original"]]
    );
    let both = choice(&payload, &["aux", "original"]);
    assert_eq!(
        field(checkpoint(both, "soft", None), &["slack"]).as_str(),
        Some("-1/10000")
    );
    assert_eq!(
        field(checkpoint(both, "soft", Some("second")), &["closing_cash"]).as_str(),
        Some("32319999/10000")
    );
    assert!(
        field(both, &["structural_rejections"])
            .as_array()
            .expect("initial funding rejection")
            .iter()
            .any(|r| field(r, &["kind"]).as_str() == Some("cash_schedule_shortfall"))
    );
}

#[test]
fn source_cash_floor_breaches_survive_later_recovery_and_exact_equality_passes() {
    let dir = temporary();
    let path = portfolio(&dir);
    let source = std::fs::read_to_string(path).expect("portfolio");
    let text = source.clone() + &cash_declaration("2500", "-1616.0001", "5000");
    let payload = json(&run(&write(&dir, "period-shortfall.toml", &text)));
    assert_eq!(
        winners(&payload, "feasible_winners"),
        vec![vec!["original"]]
    );
    let both = choice(&payload, &["aux", "original"]);
    assert_eq!(
        field(checkpoint(both, "soft", Some("first")), &["slack"]).as_str(),
        Some("-1/10000")
    );
    assert_eq!(
        field(checkpoint(both, "soft", Some("second")), &["closing_cash"]).as_str(),
        Some("66159999/10000")
    );
    let equal = source + &cash_declaration("2500", "-1616", "5000");
    let payload = json(&run(&write(&dir, "floor-equality.toml", &equal)));
    assert_eq!(
        winners(&payload, "feasible_winners"),
        vec![vec!["aux", "original"]]
    );
    assert_eq!(
        field(
            checkpoint(
                choice(&payload, &["aux", "original"]),
                "soft",
                Some("first")
            ),
            &["slack"]
        )
        .as_str(),
        Some("0/1")
    );
}

#[test]
fn source_funding_requires_complete_exact_declared_cash_and_valid_settlement_timing() {
    let dir = temporary();
    let path = portfolio(&dir);
    let source = std::fs::read_to_string(path).expect("portfolio");
    let cash = cash_declaration("2500", "0", "0");
    for (from, to, why) in [
        (
            "opening_date = \"1970-01-01\"",
            "opening_date = \"1969-12-31\"",
            "UTC valuation date",
        ),
        (
            "1970-01-01/1970-01-01",
            "1969-12-31/1969-12-31",
            "cannot precede",
        ),
        (
            "1970-01-02/1970-01-02",
            "1970-01-03/1970-01-03",
            "contiguous",
        ),
        ("id = \"second\"", "id = \"foreign\"", "cash period ids"),
        (
            "cash_rounding = \"nearest_ties_even_0_0001_usd\"",
            "cash_rounding = \"implicit\"",
            "exact_0_0001_usd",
        ),
        (
            "[[baseline_cash_flow]]\nperiod = \"second\"\nstate = \"hard\"\nvalue = \"0\"\n",
            "",
            "every opening state",
        ),
        (
            "value = \"2500\"",
            "value = \"2500.00001\"",
            "exactly representable",
        ),
    ] {
        refused(
            &dir,
            "bad-cash.toml",
            &(source.clone() + &cash.replace(from, to)),
            why,
        );
    }
    refused(
        &dir,
        "orphan-cash.toml",
        &(source.clone() + "\n[[opening_cash]]\nstate = \"soft\"\nvalue = \"2500\"\n"),
        "explicit funding_schedule",
    );
    refused(
        &dir,
        "transcribed-increment.toml",
        &(source.clone()
            + &cash
            + "\n[[candidate_cash_flow]]\nperiod = \"first\"\ncandidate = \"original\"\nstate = \"soft\"\nvalue = \"99999\"\n"),
        "unsupported table candidate_cash_flow",
    );
    // Exact settlement remains an explicit alternative to declared rounding,
    // and must reject an unrepresentable nominal source increment.
    let terms = dir.join("aux-4.toml");
    let original = std::fs::read_to_string(&terms).expect("auxiliary commercial source");
    write(
        &dir,
        "aux-4.toml",
        &original.replace("fixed_usd = \"1000\"", "fixed_usd = \"1000.00001\""),
    );
    refused(
        &dir,
        "unrounded-source-cash.toml",
        &(source + &cash.replace("nearest_ties_even_0_0001_usd", "exact_0_0001_usd")),
        "exactly representable",
    );
}

fn fraction(value: &Json) -> Rat {
    let (n, d) = value
        .as_str()
        .expect("rational string")
        .split_once('/')
        .expect("rational components");
    Rat::new(
        n.parse().expect("numerator"),
        d.parse().expect("denominator"),
    )
    .expect("exact rational")
}

#[test]
fn declared_cash_rounding_keeps_every_original_amount_and_signed_rounding_delta() {
    let dir = temporary();
    let path = portfolio(&dir);
    let source = std::fs::read_to_string(path).expect("portfolio");
    let cash = cash_declaration("2500", "0", "0");
    let payload = json(&run(&write(
        &dir,
        "rounded-cash.toml",
        &(source.clone() + &cash),
    )));
    assert_eq!(
        field(&payload, &["source_cash", "cash_rounding"]).as_str(),
        Some("nearest_ties_even_0_0001_usd")
    );
    let rows = field(&payload, &["source_cash", "rows"])
        .as_array()
        .expect("every candidate/period/state source amount");
    assert_eq!(rows.len(), 12);
    let mut nonzero = 0;
    for row in rows {
        let original = fraction(field(row, &["unrounded_increment_usd"]));
        let rounded = fraction(field(row, &["cash_increment_usd"]));
        let delta = fraction(field(row, &["rounding_delta_usd"]));
        assert_eq!(rounded.checked_sub(original).expect("difference"), delta);
        assert!(
            delta
                .checked_cmp(Rat::new(1, 20_000).expect("half minor unit"))
                .expect("upper bound")
                .is_le()
        );
        assert!(
            delta
                .checked_cmp(Rat::new(-1, 20_000).expect("negative half minor unit"))
                .expect("lower bound")
                .is_ge()
        );
        nonzero += usize::from(delta != Rat::from_integer(0));
    }
    // Exact binary64 physical amounts generally are not exact minor USD units.
    assert!(nonzero > 0);
    refused(
        &dir,
        "exact-residue.toml",
        &(source + &cash.replace("nearest_ties_even_0_0001_usd", "exact_0_0001_usd")),
        "exactly representable",
    );
}

fn robustness_declaration(radius: &str) -> String {
    let mut text = format!(
        r#"
[[robustness]]
method = "wasserstein_1"
radius = "{radius}"
metric_basis = "SYNTHETIC unit distance between distinct complete shared scenarios"
radius_basis = "Declared scenario-weight sensitivity; no confidence coverage inferred"
support = "declared_shared_scenarios"
distance_unit = "dimensionless"
"#,
    );
    for from in ["hard", "soft"] {
        for to in ["hard", "soft"] {
            let value = if from == to { "0/1" } else { "1/1" };
            text.push_str(&format!(
                "\n[[transport_distance]]\nfrom = \"{from}\"\nto = \"{to}\"\nvalue = \"{value}\"\n"
            ));
        }
    }
    text
}

fn robust_choice<'a>(payload: &'a Json, members: &[&str]) -> &'a Json {
    let id = field(choice(payload, members), &["bundle_id"]);
    field(payload, &["program", "summary", "robustness", "bundles"])
        .as_array()
        .expect("nominally feasible stressed bundles")
        .iter()
        .find(|row| field(row, &["bundle_id"]) == id)
        .expect("stressed bundle retained")
}

fn robust_winners(payload: &Json, name: &str) -> Vec<Vec<String>> {
    let summary = field(payload, &["program", "summary"]);
    let ids = field(summary, &["robustness", name])
        .as_array()
        .expect("stress winners");
    field(summary, &["choices"])
        .as_array()
        .expect("choices")
        .iter()
        .filter(|c| ids.iter().any(|id| id == field(c, &["bundle_id"])))
        .map(|c| selected(c).into_iter().map(str::to_owned).collect())
        .collect()
}

#[test]
fn source_weight_stress_has_distinct_exact_mean_and_regret_winners_and_real_cli_receipts() {
    let dir = temporary();
    let path = portfolio(&dir);
    let nominal = json(&run(&path));
    let source = std::fs::read_to_string(path).expect("portfolio source");
    let text = source + &robustness_declaration("1/2");
    let path = write(&dir, "robust.toml", &text);
    let envelope = run(&path);
    let payload = json(&envelope);
    assert_eq!(
        field(&payload, &["program", "summary", "choices"]),
        field(&nominal, &["program", "summary", "choices"])
    );
    assert_eq!(field(&payload, &["sources"]), field(&nominal, &["sources"]));
    assert_eq!(
        winners(&payload, "feasible_winners"),
        vec![vec!["aux", "original"]]
    );
    assert_eq!(
        robust_winners(&payload, "maximin_mean_winners"),
        vec![vec!["original"]]
    );
    assert_eq!(
        robust_winners(&payload, "minimax_expected_regret_winners"),
        vec![vec!["aux", "original"]]
    );
    let stress = field(&payload, &["program", "summary", "robustness"]);
    assert_eq!(field(stress, &["objective_unit"]).as_str(), Some("USD"));
    assert_eq!(
        field(stress, &["state_ids"]),
        &Json::Array(vec![
            Json::String("hard".into()),
            Json::String("soft".into())
        ])
    );
    // Radius 1/2 and unit distance allow the original 1/2,1/2 law to put all
    // mass on either complete state. Net source cash remains [-76,25445.6]
    // for both projects and [212,12972.8] for the original project.
    let both = robust_choice(&payload, &["aux", "original"]);
    assert_eq!(field(both, &["nominal_mean"]).as_str(), Some("63424/5"));
    assert_eq!(field(both, &["worst_case_mean"]).as_str(), Some("-76/1"));
    assert_eq!(
        field(both, &["worst_case_expected_regret"]).as_str(),
        Some("288/1")
    );
    let original = robust_choice(&payload, &["original"]);
    assert_eq!(
        field(original, &["worst_case_mean"]).as_str(),
        Some("212/1")
    );
    assert_eq!(
        field(original, &["worst_case_expected_regret"]).as_str(),
        Some("62364/5")
    );
    for certificate in ["loss_certificate", "regret_certificate"] {
        assert_eq!(
            field(both, &[certificate, "duality_gap"]).as_str(),
            Some("0/1")
        );
        assert_eq!(
            field(both, &[certificate, "transport_cost"]).as_str(),
            Some("1/2")
        );
    }
    let claim = field(&payload, &["program", "inputs", "robustness"]);
    assert_eq!(field(claim, &["evidence"]).as_str(), Some("assumed"));
    let anchors = field(claim, &["anchors"])
        .as_array()
        .expect("robustness source anchors");
    for sha in [sha256(text.as_bytes()), sha256(SOURCE.as_bytes())] {
        let sha = sha.to_hex();
        assert!(
            anchors
                .iter()
                .any(|anchor| field(anchor, &["sha256"]).as_str() == Some(sha.as_str()))
        );
    }
    assert!(anchors.iter().any(|anchor| {
        field(anchor, &["locator"])
            .as_str()
            .is_some_and(|locator| locator.ends_with("robustness.distance_unit"))
    }));
    assert_eq!(envelope.to_json(), run(&path).to_json());
    let actual = Command::new(binary())
        .args(args(&path))
        .args(["--workspace"])
        .arg(root())
        .output()
        .expect("robust capital CLI binary");
    assert!(actual.status.success());
    assert_eq!(
        String::from_utf8_lossy(&actual.stdout).trim_end(),
        envelope.to_json()
    );
    let markdown = Command::new(binary())
        .args(args(&path))
        .args(["--format", "markdown", "--workspace"])
        .arg(root())
        .output()
        .expect("robust capital Markdown");
    assert!(markdown.status.success());
    let rendered = envelope.rendered_markdown.as_deref().expect("Markdown");
    assert_eq!(
        String::from_utf8_lossy(&markdown.stdout),
        format!("{rendered}\n")
    );
    assert!(rendered.contains("Capital scenario-weight robustness"));
    assert!(rendered.contains("Source dual potential"));
}

#[test]
fn source_cash_tail_limits_are_certified_under_stress_separately_from_nominal_selection() {
    let dir = temporary();
    let path = portfolio(&dir);
    let source = std::fs::read_to_string(path).expect("portfolio source");
    let text = source.replace(
        "tail_mass = \"1/2\"",
        "tail_mass = \"3/4\"\nmax_tail_loss = \"0\"",
    ) + &robustness_declaration("1/2");
    let payload = json(&run(&write(&dir, "robust-tail.toml", &text)));
    assert_eq!(
        winners(&payload, "feasible_winners"),
        vec![vec!["aux", "original"]]
    );
    assert_eq!(
        robust_winners(&payload, "minimax_expected_regret_winners"),
        vec![vec!["aux", "original"]]
    );
    assert_eq!(
        robust_winners(&payload, "robust_tail_minimax_expected_regret_winners"),
        vec![vec!["original"]]
    );
    let both = robust_choice(&payload, &["aux", "original"]);
    assert_eq!(
        field(both, &["robust_tail_feasible"]).as_bool(),
        Some(false)
    );
    let tail = &field(both, &["tail_limits"])
        .as_array()
        .expect("declared tail limit")[0];
    // The nominal worst 3/4 has loss (0.5*76 - 0.25*25445.6)/0.75.
    // Moving the hard state's entire half mass to soft makes that loss 76.
    assert_eq!(
        field(tail, &["nominal_tail_loss"]).as_str(),
        Some("-42156/5")
    );
    assert_eq!(
        field(tail, &["worst_case_tail_loss"]).as_str(),
        Some("76/1")
    );
    assert_eq!(
        field(tail, &["certificate", "duality_gap"]).as_str(),
        Some("0/1")
    );
    assert_eq!(
        field(
            robust_choice(&payload, &["original"]),
            &["robust_tail_feasible"]
        )
        .as_bool(),
        Some(true)
    );
    let equal = json(&run(&write(
        &dir,
        "robust-tail-equality.toml",
        &text.replace("max_tail_loss = \"0\"", "max_tail_loss = \"76\""),
    )));
    assert_eq!(
        robust_winners(&equal, "robust_tail_minimax_expected_regret_winners"),
        vec![vec!["aux", "original"]]
    );
}

#[test]
fn zero_stressed_weight_cannot_erase_an_original_source_cash_shortfall() {
    let dir = temporary();
    let path = portfolio(&dir);
    let source = std::fs::read_to_string(path).expect("portfolio source")
        + &cash_declaration("2500", "-1616.0001", "5000");
    let nominal = json(&run(&write(&dir, "nominal-cash.toml", &source)));
    let payload = json(&run(&write(
        &dir,
        "robust-cash.toml",
        &(source + &robustness_declaration("1/2")),
    )));
    assert_eq!(
        field(&payload, &["source_cash"]),
        field(&nominal, &["source_cash"])
    );
    assert_eq!(
        field(&payload, &["program", "summary", "choices"]),
        field(&nominal, &["program", "summary", "choices"])
    );
    let both = choice(&payload, &["aux", "original"]);
    assert_eq!(
        field(checkpoint(both, "soft", Some("first")), &["slack"]).as_str(),
        Some("-1/10000")
    );
    let stress = field(&payload, &["program", "summary", "robustness"]);
    assert!(
        field(stress, &["structural_rejections"])
            .as_array()
            .expect("original cash exclusions")
            .contains(field(both, &["bundle_id"]))
    );
    assert!(
        field(stress, &["bundles"])
            .as_array()
            .expect("stressed bundles")
            .iter()
            .all(|row| field(row, &["bundle_id"]) != field(both, &["bundle_id"]))
    );
    assert_eq!(
        field(
            robust_choice(&payload, &[]),
            &["regret_certificate", "destination_weights"]
        ),
        &Json::Array(vec![Json::String("1/1".into()), Json::String("0/1".into())])
    );
    assert_eq!(
        robust_winners(&payload, "minimax_expected_regret_winners"),
        vec![vec!["original"]]
    );
}

#[test]
fn source_weight_stress_requires_explicit_support_units_and_complete_valid_geometry() {
    let dir = temporary();
    let stress = robustness_declaration("1/2");
    for (from, to, why) in [
        (
            "support = \"declared_shared_scenarios\"\n",
            "",
            "missing support",
        ),
        (
            "distance_unit = \"dimensionless\"",
            "distance_unit = \"USD\"",
            "dimensionless distances required",
        ),
        (
            "radius = \"1/2\"",
            "radius = \"-1/2\"",
            "radius must be nonnegative",
        ),
        (
            "[[transport_distance]]\nfrom = \"hard\"\nto = \"soft\"\nvalue = \"1/1\"\n",
            "",
            "every ordered distance pair",
        ),
        (
            "from = \"soft\"\nto = \"hard\"\nvalue = \"1/1\"",
            "from = \"soft\"\nto = \"hard\"\nvalue = \"2/1\"",
            "metric must be symmetric",
        ),
        (
            "from = \"soft\"\nto = \"soft\"",
            "from = \"outside\"\nto = \"soft\"",
            "unknown shared state",
        ),
    ] {
        refused(
            &dir,
            "invalid-robustness.toml",
            &(PORTFOLIO.to_owned() + &stress.replace(from, to)),
            why,
        );
    }
    let distance = "\n[[transport_distance]]\nfrom = \"hard\"\nto = \"soft\"\nvalue = \"1/1\"\n";
    refused(
        &dir,
        "duplicate-distance.toml",
        &(PORTFOLIO.to_owned() + &stress + distance),
        "unique transport distances",
    );
    refused(
        &dir,
        "orphan-distance.toml",
        &(PORTFOLIO.to_owned() + distance),
        "explicit robustness declaration",
    );
}

fn stability_declaration() -> &'static str {
    r#"
[[selection_stability]]
method = "weighted_multinomial"
seed = "229"
replicates = "200"
draws_per_replicate = "1"
sampling_basis = "SYNTHETIC conditional scenario-law experiment; one draw is not an effective historical sample size"
support = "declared_shared_scenarios"
resampling_unit = "complete_joint_state"
"#
}

fn stability(payload: &Json) -> &Json {
    field(payload, &["program", "summary", "selection_frequency"])
}

fn stability_choice<'a>(payload: &'a Json, members: &[&str]) -> &'a Json {
    let id = field(choice(payload, members), &["bundle_id"]);
    field(stability(payload), &["alternatives"])
        .as_array()
        .expect("selection frequencies")
        .iter()
        .find(|row| field(row, &["alternative_id"]) == id)
        .expect("original subset frequency retained")
}

/// The sampler's counts are inputs to this independent source-cash selection oracle.
/// Each draw retains both projects and both operating periods in its original state.
fn one_draw_winners(payload: &Json, hard: &[&str], soft: &[&str]) -> [u64; 2] {
    let result = stability(payload);
    assert_eq!(
        field(result, &["state_ids"]),
        &Json::Array(vec![
            Json::String("hard".into()),
            Json::String("soft".into())
        ])
    );
    let replicates = field(result, &["replicates"])
        .as_array()
        .expect("every retained resample");
    assert_eq!(replicates.len(), 200);
    let mut counts = [0; 2];
    for replicate in replicates {
        let draws = field(replicate, &["state_counts"])
            .as_array()
            .expect("complete state count vector");
        assert_eq!(draws.len(), 2);
        let draw_counts = (draws[0].u64(), draws[1].u64());
        assert!(
            matches!(draw_counts, (Some(1), Some(0)) | (Some(0), Some(1))),
            "one complete joint-state draw required"
        );
        let index = usize::from(draw_counts.1 == Some(1));
        counts[index] += 1;
        let expected = choice(payload, if index == 0 { hard } else { soft });
        assert_eq!(
            field(replicate, &["feasible_winners"]),
            &Json::Array(vec![field(expected, &["bundle_id"]).clone()])
        );
    }
    assert!(
        counts[0] > 0 && counts[1] > 0,
        "the fixed seed exercises both laws"
    );
    counts
}

#[test]
fn source_selection_stability_retains_nominal_cash_robustness_and_actual_cli_receipts() {
    let dir = temporary();
    let path = portfolio(&dir);
    let source = std::fs::read_to_string(path).expect("source portfolio")
        + &cash_declaration("2500", "0", "0")
        + &robustness_declaration("1/2");
    let nominal = json(&run(&write(&dir, "nominal-stress.toml", &source)));
    let text = source + stability_declaration();
    let path = write(&dir, "selection-stability.toml", &text);
    let envelope = run(&path);
    let payload = json(&envelope);
    for keys in [
        vec!["program", "summary", "choices"],
        vec!["program", "summary", "feasible_winners"],
        vec!["program", "summary", "robustness"],
        vec!["program", "inputs", "weights", "value"],
        vec!["sources"],
        vec!["source_cash"],
    ] {
        assert_eq!(field(&payload, &keys), field(&nominal, &keys));
    }
    assert_eq!(
        field(&payload, &["program", "summary", "selection_frequency_status"]).as_str(),
        Some("conditional_weighted_bootstrap")
    );
    assert_eq!(
        winners(&payload, "feasible_winners"),
        vec![vec!["aux", "original"]]
    );
    assert_eq!(
        robust_winners(&payload, "maximin_mean_winners"),
        vec![vec!["original"]]
    );
    // Independent source arithmetic: soft gives original 212 > hold 0 > both -76 > aux -288;
    // hard gives both 25445.6 > original 12972.8 > aux 12472.8 > hold 0.
    let [hard, soft] = one_draw_winners(&payload, &["aux", "original"], &["original"]);
    let both = stability_choice(&payload, &["aux", "original"]);
    assert_eq!(field(both, &["co_winner_replicates"]).u64(), Some(hard));
    assert_eq!(field(both, &["risk_feasible_replicates"]).u64(), Some(200));
    assert_eq!(
        fraction(field(both, &["selection_frequency"])),
        Rat::new(i128::from(hard), 200).expect("frequency")
    );
    assert!(
        hard < 160,
        "the declared one-draw experiment exposes fragility"
    );
    assert_eq!(field(both, &["fragile"]).as_bool(), Some(true));
    assert_eq!(
        field(stability(&payload), &["definite_flip_frequency"]),
        field(stability(&payload), &["possible_flip_frequency"])
    );
    assert_eq!(
        fraction(field(stability(&payload), &["definite_flip_frequency"])),
        Rat::new(i128::from(soft), 200).expect("flip frequency")
    );
    let claim = field(&payload, &["program", "inputs", "selection_stability"]);
    assert_eq!(field(claim, &["evidence"]).as_str(), Some("assumed"));
    assert!(
        field(&payload, &["program", "claim", "parents"])
            .as_array()
            .expect("source-bound program parents")
            .contains(field(claim, &["id"]))
    );
    let anchors = field(claim, &["anchors"]).as_array().expect("source anchors");
    for hash in [sha256(text.as_bytes()), sha256(SOURCE.as_bytes())] {
        let hash = hash.to_hex();
        assert!(
            anchors
                .iter()
                .any(|anchor| field(anchor, &["sha256"]).as_str() == Some(hash.as_str()))
        );
    }
    assert!(anchors.iter().any(|anchor| {
        field(anchor, &["locator"])
            .as_str()
            .is_some_and(|locator| locator.ends_with("selection_stability.resampling_unit"))
    }));
    assert_eq!(envelope.to_json(), run(&path).to_json());
    let rendered = envelope.rendered_markdown.as_deref().expect("Markdown");
    assert!(rendered.contains("Selection stability"));
    assert!(rendered.contains("fragile co-winner"));
    assert!(rendered.contains("not calibrated probabilities of optimality"));
    let expected_json = envelope.to_json();
    for (format, expected) in [("json", expected_json.as_str()), ("markdown", rendered)] {
        let actual = Command::new(binary())
            .args(args(&path))
            .args(["--format", format, "--workspace"])
            .arg(root())
            .output()
            .expect("source-derived selection-stability CLI");
        assert!(actual.status.success());
        assert_eq!(
            String::from_utf8_lossy(&actual.stdout),
            format!("{expected}\n")
        );
    }
    let changed = json(&run(&write(
        &dir,
        "changed-sampling-basis.toml",
        &text.replace(
            "conditional scenario-law experiment",
            "changed conditional sampling rationale",
        ),
    )));
    assert_eq!(
        field(stability(&payload), &["replicates"]),
        field(stability(&changed), &["replicates"])
    );
    assert_ne!(field(&payload, &["digest"]), field(&changed, &["digest"]));
    assert_ne!(
        field(claim, &["id"]),
        field(&changed, &["program", "inputs", "selection_stability", "id"])
    );
}

#[test]
fn source_resamples_recompute_joint_tail_limits_instead_of_freezing_nominal_risk_flags() {
    let dir = temporary();
    let path = portfolio(&dir);
    let source = std::fs::read_to_string(path).expect("source portfolio");
    let text = source.replace(
        "tail_mass = \"1/2\"",
        "tail_mass = \"1/2\"\nmax_tail_loss = \"0\"",
    ) + stability_declaration();
    let payload = json(&run(&write(&dir, "resampled-risk.toml", &text)));
    assert_eq!(
        winners(&payload, "feasible_winners"),
        vec![vec!["original"]]
    );
    let both = choice(&payload, &["aux", "original"]);
    assert_eq!(field(both, &["risk_rejected"]).as_bool(), Some(true));
    assert_eq!(field(both, &["tail_loss"]).as_str(), Some("76/1"));
    // In an all-hard draw the complete two-project loss is -25445.6, so it
    // passes the declared zero cap and wins despite its original risk rejection.
    let [hard, soft] = one_draw_winners(&payload, &["aux", "original"], &["original"]);
    let both = stability_choice(&payload, &["aux", "original"]);
    assert_eq!(field(both, &["risk_feasible_replicates"]).u64(), Some(hard));
    assert_eq!(field(both, &["co_winner_replicates"]).u64(), Some(hard));
    let original = stability_choice(&payload, &["original"]);
    assert_eq!(
        field(original, &["risk_feasible_replicates"]).u64(),
        Some(200)
    );
    assert_eq!(field(original, &["co_winner_replicates"]).u64(), Some(soft));
    assert_eq!(
        field(stability(&payload), &["no_feasible_replicates"]).u64(),
        Some(0)
    );
}

#[test]
fn unsampled_source_states_keep_their_original_cash_floor_obligations() {
    let dir = temporary();
    let path = portfolio(&dir);
    let source = std::fs::read_to_string(path).expect("source portfolio")
        + &cash_declaration("2500", "-1616.0001", "5000");
    let nominal = json(&run(&write(&dir, "nominal-floor.toml", &source)));
    let payload = json(&run(&write(
        &dir,
        "resampled-floor.toml",
        &(source + stability_declaration()),
    )));
    assert_eq!(
        field(&payload, &["program", "summary", "choices"]),
        field(&nominal, &["program", "summary", "choices"])
    );
    assert_eq!(
        field(&payload, &["source_cash"]),
        field(&nominal, &["source_cash"])
    );
    let both = choice(&payload, &["aux", "original"]);
    assert_eq!(
        field(checkpoint(both, "soft", Some("first")), &["slack"]).as_str(),
        Some("-1/10000")
    );
    // Both projects fail the original soft-state cash floor, even in all-hard
    // resamples. The original project beats aux by 500 USD in every state.
    one_draw_winners(&payload, &["original"], &["original"]);
    let both = stability_choice(&payload, &["aux", "original"]);
    assert_eq!(field(both, &["structurally_feasible"]).as_bool(), Some(false));
    assert_eq!(field(both, &["risk_feasible_replicates"]).u64(), Some(0));
    assert_eq!(field(both, &["co_winner_replicates"]).u64(), Some(0));
    assert_eq!(
        field(
            stability_choice(&payload, &["original"]),
            &["selection_frequency"]
        )
        .as_str(),
        Some("1/1")
    );
}

#[test]
fn source_selection_stability_requires_explicit_joint_sampling_and_bounded_integer_controls() {
    let dir = temporary();
    for (before, after, why) in [
        (
            "support = \"declared_shared_scenarios\"\n",
            "",
            "missing support",
        ),
        (
            "resampling_unit = \"complete_joint_state\"",
            "resampling_unit = \"independent_candidate_period\"",
            "complete_joint_state resampling required",
        ),
        (
            "seed = \"229\"",
            "seed = \"+229\"",
            "unsigned decimal integers",
        ),
        (
            "seed = \"229\"",
            "seed = \"18446744073709551616\"",
            "exceeds its integer range",
        ),
        (
            "replicates = \"200\"",
            "replicates = \"199\"",
            "200..=2000 replicates",
        ),
        (
            "draws_per_replicate = \"1\"",
            "draws_per_replicate = \"0\"",
            "1..=4096 draws",
        ),
        (
            "replicates = \"200\"",
            "replicates = \"2000\"",
            "8000000 total draws",
        ),
    ] {
        let mut declaration = stability_declaration().replace(before, after);
        if why == "8000000 total draws" {
            declaration = declaration.replace(
                "draws_per_replicate = \"1\"",
                "draws_per_replicate = \"4096\"",
            );
        }
        refused(
            &dir,
            "invalid-selection-stability.toml",
            &(PORTFOLIO.to_owned() + &declaration),
            why,
        );
    }
    refused(
        &dir,
        "duplicate-selection-stability.toml",
        &(PORTFOLIO.to_owned() + stability_declaration() + stability_declaration()),
        "one weighted_multinomial selection_stability declaration",
    );
}
