//! FACT: source-derived portfolio known answers and whole-command source admission controls.
//! Determinism: DeterministicPerIsa (inherited production); expected financial answers are exact.

#![allow(clippy::expect_used)]

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use fmine_cli::{Envelope, OutputFormat, invoke};
use fmine_core::hash::sha256;
use fmine_io::omf::json::{Json, parse};

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
