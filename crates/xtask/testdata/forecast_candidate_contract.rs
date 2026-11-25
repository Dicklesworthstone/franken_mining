// FACT: independent invented quarterly ratios and source-mutation controls through the actual CLI.
use std::fmt::Write as _;
use std::path::PathBuf;
use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};

use fmine_calib::record::{
    ATTESTATION, Rat, parse_date, parse_forecasts, parse_manifest, parse_rules, parse_utc,
};
use fmine_core::hash::sha256;
use fmine_io::omf::json::{self, Json};
use fmine_io::xlsx::{MinimalCell, MinimalSheet, MinimalStyles, build_minimal_xlsx_styled};

const MODEL: &str = "pooled-quarterly-ratio-empirical-v1";
const SEASONAL_MODEL: &str = "seasonal-quarterly-ratio-empirical-v1";
const CHANGE_MODEL: &str = "pooled-quarterly-change-empirical-v1";
const SEASONAL_CHANGE_MODEL: &str = "seasonal-quarterly-change-empirical-v1";
const CUT: &str = "2030-10-03T00:00:00Z";
static SERIAL: AtomicUsize = AtomicUsize::new(0);

const RULES: &str = r#"
rules_id = "invented-cli-rules"
record_id = "invented-cli-record"
issuer = "synthetic"
alpha = "0.2"
interval_score = "invented declared interval score"
quantile_estimator = "inverse_ecdf"
rho_low = "0"
rho_high = "0.5"
coverage_level = "0.95"
report = "retain every declared member"
calibration_claim_min_n = 35
calibration_claim = "uncalibrated"
nowcast_from_utc = "2030-10-01T00:00:00Z"
proof_by_utc = "2030-12-31T23:59:59Z"
outcome_vintage = "original only"
excluded_rows = "aggregate rows are not members"

[[target]]
id = "production"
quantity = "gold produced, 100% basis"
scored_quantity = "Q4 implied production"
field = "au_prod_100_koz"
outcome = "fy_minus_9m"
[[target]]
id = "unit_cost"
quantity = "AISC per attributable ounce sold"
scored_quantity = "printed Q4 AISC"
field = "aisc_usd_per_oz"
outcome = "q4_qtd"

[[baseline]]
id = "run_rate_persistence"
target = "production"
rule = "positive consecutive-quarter ratios"
source = "invented original workbooks"
pool_from = "2030Q1"
pool_through = "2030Q2"
point_quarter = "2030Q3"
[[baseline]]
id = "run_rate_persistence"
target = "unit_cost"
rule = "positive consecutive-quarter ratios"
source = "invented original workbooks"
pool_from = "2030Q1"
pool_through = "2030Q2"
point_quarter = "2030Q3"

[[site]]
name = "Site A"
status = "live"
locator = "synthetic:QTD!A3"
[[site]]
name = "Site B"
status = "live"
locator = "synthetic:QTD!A4"
[[site]]
name = "Retired Site"
status = "not_published"
locator = "synthetic:QTD!A5"
reason = "invented retired site stays in the declared universe"
"#;

struct Fixture {
    root: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        Self::with_metrics(
            RULES,
            "Gold produced (000s oz, 100%)",
            "All-in sustaining costs ($/oz)",
        )
    }

    fn with_metrics(rules: &str, production_header: &str, cost_header: &str) -> Self {
        Self::with_quarters(
            rules,
            production_header,
            cost_header,
            &[(2030, 1, 1), (2030, 2, 2), (2030, 3, 3)],
        )
    }

    fn with_quarters(
        rules: &str,
        production_header: &str,
        cost_header: &str,
        quarters: &[(i32, u8, u8)],
    ) -> Self {
        let root = std::env::temp_dir().join(format!(
            "fmine-forecast-candidate-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("invented")).unwrap();
        std::fs::create_dir_all(root.join("registries/sources")).unwrap();
        std::fs::write(root.join("rules.toml"), rules).unwrap();
        let mut registry = String::from(
            "schema = \"fmine.sources.v1\"\nissuer = \"synthetic\"\ncorpus_dir = \"invented\"\n\
             retrieved_utc = \"2030-10-02T10:00:00Z/10:01:00Z\"\nurl_prefix = \"https://example.invalid/\"\n",
        );
        for (index, &(year, quarter, values)) in quarters.iter().enumerate() {
            let bytes = workbook(year, quarter, values, production_header, cost_header);
            let file = format!("Invented_Q{}.xlsx", index + 1);
            std::fs::write(root.join("invented").join(&file), &bytes).unwrap();
            writeln!(registry,
                "\n[[source]]\nfile = \"{file}\"\nkind = \"mine_stats\"\nquarter = \"{year}Q{quarter}\"\n\
                 path = \"{file}\"\nurl_basis = \"fetched and verified 2030-10-02\"\nbytes = {}\nsha256 = \"{}\"",
                bytes.len(), sha256(&bytes).to_hex()
            ).unwrap();
        }
        registry.push_str("\n[[subject]]\nname = \"Site A\"\n[[subject]]\nname = \"Site B\"\n");
        std::fs::write(root.join("registries/sources/synthetic.toml"), registry).unwrap();
        Self { root }
    }

    fn args(&self) -> Vec<String> {
        [
            "forecast",
            "record",
            "build",
            "--rules",
            "rules.toml",
            "--model",
            MODEL,
            "--as-of",
            CUT,
            "--decimal-places",
            "2",
            "--workspace",
        ]
        .into_iter()
        .map(str::to_string)
        .chain([self.root.to_str().unwrap().to_string()])
        .collect()
    }
}

fn workbook(
    year: i32,
    quarter: u8,
    values: u8,
    production_header: &str,
    cost_header: &str,
) -> Vec<u8> {
    let text = |value: &str| MinimalCell::Text(value.into());
    let styled = |value: &str| MinimalCell::Styled {
        raw: value.into(),
        xf: 1,
    };
    // All four columns retain a 0.4 stored fraction that is absent from their integer display.
    // Production ratios are 125/100 and 100/200; cost ratios are 1100/1000 and 2400/2000.
    let values = match values {
        1 => ["100.4", "1000.4", "200.4", "2000.4"],
        2 => ["125.4", "1100.4", "100.4", "2400.4"],
        4 => ["100.4", "-10.4", "200.4", "0.4"],
        5 => ["125.4", "0.4", "210.4", "-20.4"],
        6 => ["300.4", "0.4", "80.4", "-40.4"],
        7 => ["0.4", "1500.4", "0.4", "3000.4"],
        _ => ["300.4", "1500.4", "80.4", "3000.4"],
    };
    build_minimal_xlsx_styled(
        &[MinimalSheet {
            name: "QTD".into(),
            hidden: false,
            cells: vec![
                ("A1".into(), text("For the three months ended")),
                ("B1".into(), MinimalCell::Number(f64::from(year))),
                // Different releases remain distinct sources even when their numerical cells repeat.
                (
                    "D1".into(),
                    text(&format!("Synthetic release {year}Q{quarter}")),
                ),
                ("B2".into(), text(production_header)),
                ("C2".into(), text(cost_header)),
                ("A3".into(), text("Site A (50%)")),
                ("B3".into(), styled(values[0])),
                ("C3".into(), styled(values[1])),
                ("A4".into(), text("Site B (75%)")),
                ("B4".into(), styled(values[2])),
                ("C4".into(), styled(values[3])),
            ],
        }],
        Some(&MinimalStyles {
            num_fmts: vec![],
            xfs: vec![0, 1],
        }),
    )
}

fn output(args: &[String]) -> Output {
    let output = Command::new(env!("CARGO_BIN_EXE_fmine"))
        .args(args)
        .output()
        .expect("run actual binary");
    assert!(output.stderr.is_empty(), "{:?}", output.stderr);
    output
}

fn invoke(args: &[String]) -> (i32, Json, String) {
    let output = output(args);
    let text = String::from_utf8(output.stdout).unwrap();
    assert_eq!(text.lines().count(), 1, "one NDJSON envelope: {text}");
    let value = json::parse(text.as_bytes()).expect("valid finite JSON");
    let exit = output.status.code().expect("normal exit");
    assert_eq!(at(&value, &["exit_code"]).i64(), Some(i64::from(exit)));
    for (_, flag) in at(&value, &["flags"]).as_object().unwrap() {
        assert_eq!(flag.as_bool(), Some(false));
    }
    assert_eq!(at(&value, &["certificate"]), &Json::Null);
    (exit, value, text)
}

fn at<'a>(value: &'a Json, path: &[&str]) -> &'a Json {
    path.iter().fold(value, |value, key| {
        value
            .get(key)
            .unwrap_or_else(|| panic!("missing {key}: {value:?}"))
    })
}

#[test]
fn cli_exports_manifest_inputs_with_actual_retrieval_bounds() {
    let fixture = Fixture::new();
    let args = fixture.args();
    let (exit, undated, _) = invoke(&args);
    assert_eq!(exit, 0);
    assert_eq!(undated.get("payload").unwrap().get("manifest_inputs_toml"), Some(&Json::Null));

    let path = fixture.root.join("registries/sources/synthetic.toml");
    let mut registry = std::fs::read_to_string(&path).unwrap();
    let dates = ["2030-05-01", "2030-08-01", "2030-10-02"];
    for (index, date) in dates.iter().enumerate() {
        let source_path = format!("path = \"Invented_Q{}.xlsx\"\n", index + 1);
        registry = registry.replacen(
            &source_path,
            &format!(
                "{source_path}published_at = \"{date}\"\npublished_at_basis = \"FACT: invented release day\"\n"
            ),
            1,
        );
        if index == 1 {
            std::fs::write(&path, &registry).unwrap();
            let (exit, partial, _) = invoke(&args);
            assert_eq!(exit, 0);
            assert_eq!(at(&partial, &["payload", "manifest_inputs_toml"]), &Json::Null);
        }
    }
    std::fs::write(path, registry).unwrap();
    let (exit, dated, text) = invoke(&args);
    assert_eq!(exit, 0, "{text}");
    let report = at(&dated, &["payload"]);
    let rows = at(report, &["manifest_inputs_toml"]).as_str().unwrap();
    let rules = at(report, &["rules_digest"]).as_str().unwrap();
    let manifest = parse_manifest(&format!(
        "record_id = \"invented-cli-record\"\nauthor_role = \"synthetic-test-author\"\n\
         information_cutoff_utc = \"{CUT}\"\nlabel = \"nowcast\"\ncontact = \"pre-contact\"\n\
         attestation = \"{ATTESTATION}\"\nrules_sha256 = \"{rules}\"\n{rows}"
    ))
    .expect("the exported inputs fit the frozen-record parser");
    let sources = at(report, &["input_manifest"]).as_array().unwrap();
    assert_eq!(manifest.inputs.len(), 3);
    assert_eq!(manifest.inputs.len(), sources.len());
    for ((input, source), date) in manifest.inputs.iter().zip(sources).zip(dates) {
        assert_eq!(Some(input.doc_id.as_str()), at(source, &["file"]).as_str());
        assert_eq!(Some(input.url.as_str()), at(source, &["url"]).as_str());
        assert_eq!(i64::try_from(input.bytes).ok(), at(source, &["bytes"]).i64());
        let workbook = std::fs::read(fixture.root.join("invented").join(&input.doc_id)).unwrap();
        assert_eq!(sha256(&workbook).0, input.sha256);
        assert_eq!(Some(input.publication_day), parse_date(date));
        assert_eq!(Some(input.retrieved_utc), parse_utc("2030-10-02T10:01:00Z"));
    }
    assert_eq!(
        at(&sources[2], &["retrieved_utc"]).as_str(),
        Some("2030-10-02T10:00:00Z/10:01:00Z")
    );
    assert_eq!(
        at(&sources[2], &["available_by_utc"]).as_str(),
        Some("2030-10-02T23:59:59Z")
    );
    let markdown = output(&[args, vec!["--format".into(), "markdown".into()]].concat());
    assert!(markdown.status.success());
    assert!(String::from_utf8(markdown.stdout).unwrap().contains(rows));
}

fn prepared_record_fixture() -> Fixture {
    let mut rules = RULES.to_string();
    for target in ["production", "unit_cost"] {
        writeln!(rules, "\n[[baseline]]\nid = \"issuer_guidance\"\ntarget = \"{target}\"\nrule = \"declared matching issuer range\"\nsource = \"synthetic guidance\"").unwrap();
    }
    let fixture = Fixture::with_metrics(&rules,
        "Gold produced (000s oz, 100%)", "All-in sustaining costs ($/oz)");
    let registry_path = fixture.root.join("registries/sources/synthetic.toml");
    let mut registry = std::fs::read_to_string(&registry_path).unwrap();
    for (index, day) in ["2030-05-01", "2030-08-01", "2030-10-02"].iter().enumerate() {
        let path = format!("path = \"Invented_Q{}.xlsx\"\n", index + 1);
        registry = registry.replacen(&path, &format!(
            "{path}published_at = \"{day}\"\npublished_at_basis = \"FACT: synthetic release day\"\n"
        ), 1);
    }
    std::fs::write(registry_path, registry).unwrap();
    let (exit, built, text) = invoke(&fixture.args());
    assert_eq!(exit, 0, "{text}");
    let candidate = at(&built, &["payload"]);
    let rows = at(candidate, &["manifest_inputs_toml"]).as_str().unwrap();
    let digest = at(candidate, &["rules_digest"]).as_str().unwrap();
    let dir = fixture.root.join("record");
    std::fs::create_dir(&dir).unwrap();
    std::fs::write(dir.join("rules.toml"), rules).unwrap();
    std::fs::write(dir.join("forecasts.csv"), at(candidate, &["forecasts_csv"]).as_str().unwrap()).unwrap();
    std::fs::write(dir.join("manifest.toml"), format!(
        "record_id = \"invented-cli-record\"\nauthor_role = \"synthetic-test-author\"\n\
         information_cutoff_utc = \"{CUT}\"\nlabel = \"nowcast\"\ncontact = \"pre-contact\"\n\
         attestation = \"{ATTESTATION}\"\nrules_sha256 = \"{digest}\"\n{rows}"
    )).unwrap();
    fixture
}

fn record_check_args(fixture: &Fixture) -> Vec<String> {
    ["forecast", "record", "check", "--record", "record", "--workspace"]
        .into_iter().map(str::to_string).chain([fixture.root.to_str().unwrap().to_string()]).collect()
}

fn replace_record_rules(fixture: &Fixture, rules: &str) {
    let dir = fixture.root.join("record");
    let path = dir.join("rules.toml");
    let old = fmine_calib::record::rules_digest(&std::fs::read(&path).unwrap()).to_hex();
    let new = fmine_calib::record::rules_digest(rules.as_bytes()).to_hex();
    let manifest_path = dir.join("manifest.toml");
    let manifest = std::fs::read_to_string(&manifest_path).unwrap().replace(&old, &new);
    std::fs::write(path, rules).unwrap();
    std::fs::write(manifest_path, manifest).unwrap();
}

#[test]
fn cli_checks_exported_record_inputs_without_proofs_outcomes_or_writes() {
    let fixture = prepared_record_fixture();
    let args = record_check_args(&fixture);
    let dir = fixture.root.join("record");
    let paths = ["rules.toml", "manifest.toml", "forecasts.csv"];
    let before = paths.map(|path| std::fs::read(dir.join(path)).unwrap());
    let (exit, checked, text) = invoke(&args);
    assert_eq!(exit, 0, "{text}");
    assert_eq!(invoke(&args).2, text, "preflight is deterministic");
    let report = at(&checked, &["payload"]);
    for (key, expected) in [("schema", "fmine.forecast-record-check.v1"), ("status", "inputs_valid"),
        ("lane", "synthetic"), ("outcome_quarter", "2030Q4"), ("timestamp_verification", "not_performed"),
        ("author_attestation", ATTESTATION)] {
        assert_eq!(at(report, &[key]).as_str(), Some(expected));
    }
    assert_eq!(at(report, &["n_forecasts"]).i64(), Some(4));
    assert_eq!(at(report, &["n_live_sites"]).i64(), Some(2));
    assert_eq!(at(report, &["n_targets"]).i64(), Some(2));
    assert_eq!(at(report, &["not_published_sites"]).as_array().unwrap()[0].as_str(), Some("Retired Site"));
    assert_eq!(at(report, &["inputs"]).as_array().unwrap().len(), 3);
    for source in at(report, &["inputs"]).as_array().unwrap() {
        assert_eq!(at(source, &["bytes_verified"]).as_bool(), Some(true));
    }
    for field in ["model_reproduced", "baseline_values_checked", "outcomes_read", "issued",
        "stored_rules_digest_checked", "stored_record_digest_checked", "gate_eligible"] {
        assert_eq!(at(report, &[field]).as_bool(), Some(false));
    }
    let rules_digest = fmine_calib::record::rules_digest(&before[0]).to_hex();
    let record_digest = fmine_calib::record::record_digest(&before[1], &before[2]).to_hex();
    assert_eq!(at(report, &["rules_sha256"]).as_str(), Some(rules_digest.as_str()));
    assert_eq!(at(report, &["record_sha256"]).as_str(), Some(record_digest.as_str()));
    let markdown = output(&[args.clone(), vec!["--format".into(), "markdown".into()]].concat());
    assert!(markdown.status.success());
    let markdown = String::from_utf8(markdown.stdout).unwrap();
    assert!(markdown.contains(&record_digest) && markdown.contains("No future outcome files"));
    assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 3, "check creates no hashes or receipts");
    assert_eq!(paths.map(|path| std::fs::read(dir.join(path)).unwrap()), before);
    std::fs::write(dir.join("rules.sha256"), format!("{rules_digest}\n")).unwrap();
    std::fs::write(dir.join("record.sha256"), format!("{record_digest}\n")).unwrap();
    let (exit, checked, text) = invoke(&args);
    assert_eq!(exit, 0, "{text}");
    assert_eq!(at(&checked, &["payload", "stored_record_digest_checked"]).as_bool(), Some(true));
    std::fs::write(dir.join("record.sha256"), format!("{}\n", "0".repeat(64))).unwrap();
    let (exit, refused, _) = invoke(&args);
    assert_eq!(exit, 8);
    assert_eq!(at(&refused, &["refusal", "code"]).as_str(), Some("FMINE-REF-505"));
    assert_eq!(std::fs::read_to_string(dir.join("record.sha256")).unwrap(), format!("{}\n", "0".repeat(64)));
    let (exit, docs, _) = invoke(&["robot-docs".into()]);
    assert_eq!(exit, 0);
    assert_eq!(at(&docs, &["payload", "forecast_record_check", "schema"]).as_str(), Some("fmine.forecast-record-check.v1"));
}

#[test]
fn cli_record_check_refuses_changed_unknown_or_unreadable_declared_sources() {
    for change in ["bytes", "missing", "unknown", "url", "publication", "retrieval"] {
        let fixture = prepared_record_fixture();
        let source = fixture.root.join("invented/Invented_Q1.xlsx");
        let manifest_path = fixture.root.join("record/manifest.toml");
        let manifest = std::fs::read_to_string(&manifest_path).unwrap();
        match change {
            "bytes" => {
                let mut bytes = std::fs::read(&source).unwrap();
                bytes[0] ^= 1;
                std::fs::write(&source, bytes).unwrap();
            }
            "missing" => std::fs::rename(&source, source.with_extension("unavailable")).unwrap(),
            "unknown" => std::fs::write(&manifest_path, manifest.replace("Invented_Q1.xlsx", "Unregistered.xlsx")).unwrap(),
            "url" => std::fs::write(&manifest_path, manifest.replace("https://example.invalid/", "https://changed.invalid/")).unwrap(),
            "publication" => std::fs::write(&manifest_path, manifest.replace("2030-05-01", "2030-05-02")).unwrap(),
            "retrieval" => std::fs::write(&manifest_path, manifest.replacen("2030-10-02T10:01:00Z", "2030-04-30T23:59:59Z", 1)).unwrap(),
            _ => unreachable!(),
        }
        let (exit, refused, text) = invoke(&record_check_args(&fixture));
        assert_ne!(exit, 0, "{change}: {text}");
        assert_eq!(at(&refused, &["payload"]), &Json::Null);
        assert!(at(&refused, &["refusal", "what"]).as_str().unwrap().contains(
            if change == "unknown" { "Unregistered.xlsx" } else { "Invented_Q1.xlsx" }
        ), "{change}: {text}");
    }
}

#[test]
fn cli_record_check_preserves_declared_same_day_retrieval_without_inventing_time_evidence() {
    let fixture = prepared_record_fixture();
    let path = fixture.root.join("record/manifest.toml");
    let manifest = std::fs::read_to_string(&path).unwrap();
    // Scoring uses the frozen author's retrieval declaration, not the candidate builder's
    // conservative publication-day upper bound. The preflight retains that distinction.
    std::fs::write(&path, manifest.replace(CUT, "2030-10-02T12:00:00Z")).unwrap();
    let (exit, checked, text) = invoke(&record_check_args(&fixture));
    assert_eq!(exit, 0, "{text}");
    let input = &at(&checked, &["payload", "inputs"]).as_array().unwrap()[2];
    assert_eq!(at(input, &["registered_publication_date"]).as_str(), Some("2030-10-02"));
    assert_eq!(at(input, &["declared_retrieved_utc"]).as_str(), Some("2030-10-02T10:01:00Z"));
    assert_eq!(at(input, &["availability_basis"]).as_str(), Some("frozen_manifest_retrieval_declaration"));
    assert_eq!(at(&checked, &["payload", "timestamp_verification"]).as_str(), Some("not_performed"));
    std::fs::write(&path, manifest.replace(CUT, "2030-10-02T09:00:00Z")).unwrap();
    let (exit, refused, text) = invoke(&record_check_args(&fixture));
    assert_ne!(exit, 0, "{text}");
    assert_eq!(at(&refused, &["payload"]), &Json::Null);
}

#[test]
fn cli_record_check_keeps_population_attestation_and_both_baselines_mandatory() {
    for change in ["population", "attestation", "baseline"] {
        let fixture = prepared_record_fixture();
        let dir = fixture.root.join("record");
        match change {
            "population" => {
                let path = dir.join("forecasts.csv");
                let csv = std::fs::read_to_string(&path).unwrap().lines()
                    .filter(|line| !line.starts_with("Site A,production,"))
                    .map(|line| format!("{line}\n")).collect::<String>();
                std::fs::write(path, csv).unwrap();
            }
            "attestation" => {
                let path = dir.join("manifest.toml");
                let manifest = std::fs::read_to_string(&path).unwrap().replace(ATTESTATION, "not supplied");
                std::fs::write(path, manifest).unwrap();
            }
            "baseline" => {
                let text = std::fs::read_to_string(dir.join("rules.toml")).unwrap();
                let text = text.split("\n[[baseline]]\nid = \"issuer_guidance\"").next().unwrap();
                replace_record_rules(&fixture, text);
            }
            _ => unreachable!(),
        }
        let (exit, refused, text) = invoke(&record_check_args(&fixture));
        assert_ne!(exit, 0, "{change}: {text}");
        assert_eq!(at(&refused, &["payload"]), &Json::Null);
    }
}

#[test]
fn cli_record_check_refuses_impossible_labels_and_integer_second_proof_windows() {
    let fixture = prepared_record_fixture();
    let dir = fixture.root.join("record");
    let rules = std::fs::read_to_string(dir.join("rules.toml")).unwrap();
    let path = dir.join("manifest.toml");
    let manifest = std::fs::read_to_string(&path).unwrap();
    for cutoff in ["2030-12-31T23:59:59Z", "2031-01-01T00:00:00Z"] {
        std::fs::write(&path, manifest.replace(CUT, cutoff)).unwrap();
        let (exit, refused, text) = invoke(&record_check_args(&fixture));
        assert_ne!(exit, 0, "{text}");
        assert_eq!(at(&refused, &["refusal", "code"]).as_str(), Some("FMINE-REF-206"));
    }
    std::fs::write(&path, manifest.replace("label = \"nowcast\"", "label = \"forecast\"")).unwrap();
    replace_record_rules(&fixture, &rules.replace("2030-10-01T00:00:00Z", "2030-10-03T00:00:01Z"));
    let (exit, refused, text) = invoke(&record_check_args(&fixture));
    assert_ne!(exit, 0, "{text}");
    assert_eq!(at(&refused, &["refusal", "code"]).as_str(), Some("FMINE-REF-206"));
    replace_record_rules(&fixture, &rules.replace("2030-10-01T00:00:00Z", "2030-10-03T00:00:02Z"));
    let (exit, _, text) = invoke(&record_check_args(&fixture));
    assert_eq!(exit, 0, "one possible second before the nowcast boundary: {text}");
}

#[test]
fn cli_record_check_handles_post_quarter_drafts_without_opening_future_outcomes() {
    let fixture = prepared_record_fixture();
    let dir = fixture.root.join("record");
    let rules = std::fs::read_to_string(dir.join("rules.toml")).unwrap()
        .replace("fy_minus_9m", "quarter_qtd_nowcast")
        .replace("q4_qtd", "quarter_qtd_nowcast")
        .replace("2030-10-01T00:00:00Z", "2031-01-01T00:00:00Z")
        .replace("2030-12-31T23:59:59Z", "2031-01-14T23:59:59Z")
        .replacen("[[target]]",
            "earliest_outcome_release_utc = \"2031-01-15T00:00:00Z\"\noutcome_release_basis = \"Synthetic preliminary release\"\n\n[[target]]", 1);
    replace_record_rules(&fixture, &rules);
    let manifest_path = dir.join("manifest.toml");
    let manifest = std::fs::read_to_string(&manifest_path).unwrap().replace(CUT, "2031-01-02T00:00:00Z");
    std::fs::write(&manifest_path, &manifest).unwrap();
    let registry_path = fixture.root.join("registries/sources/synthetic.toml");
    let mut registry = std::fs::read_to_string(&registry_path).unwrap();
    // Metadata for an outcome deliberately absent on disk; it supplies a publication boundary only.
    writeln!(registry,
        "\n[[source]]\nfile = \"Future_Q4.xlsx\"\nkind = \"mine_stats\"\nquarter = \"2030Q4\"\n\
         path = \"Future_Q4.xlsx\"\nurl_basis = \"fetched and verified 2031-01-15\"\n\
         bytes = 1\nsha256 = \"{}\"\npublished_at = \"2031-01-15\"\n\
         published_at_basis = \"FACT: invented outcome publication day\"\n",
        sha256(b"x").to_hex()
    ).unwrap();
    std::fs::write(&registry_path, registry).unwrap();
    let (exit, report, text) = invoke(&record_check_args(&fixture));
    assert_eq!(exit, 0, "{text}");
    assert_eq!(at(&report, &["payload", "outcomes_read"]).as_bool(), Some(false));
    assert_eq!(at(&report, &["payload", "inputs"]).as_array().unwrap().len(), 3);
    std::fs::write(&manifest_path, manifest.replace("label = \"nowcast\"", "label = \"forecast\"")).unwrap();
    let (exit, refused, text) = invoke(&record_check_args(&fixture));
    assert_ne!(exit, 0, "{text}");
    assert_eq!(at(&refused, &["refusal", "code"]).as_str(), Some("FMINE-REF-206"));
    std::fs::write(&manifest_path, manifest.replace("2031-01-02T00:00:00Z", "2031-01-15T00:00:00Z")).unwrap();
    let (exit, refused, text) = invoke(&record_check_args(&fixture));
    assert_ne!(exit, 0, "{text}");
    assert_eq!(at(&refused, &["refusal", "code"]).as_str(), Some("FMINE-REF-206"));
}

#[test]
fn seasonal_cli_builds_a_distinct_source_bound_forecast_with_complete_off_season_audit() {
    let rules = RULES
        .replace("pool_from = \"2030Q1\"", "pool_from = \"2029Q1\"")
        .replace("pool_through = \"2030Q2\"", "pool_through = \"2029Q4\"");
    let fixture = Fixture::with_quarters(
        &rules,
        "Gold produced (000s oz, 100%)",
        "All-in sustaining costs ($/oz)",
        &[
            (2029, 1, 1),
            (2029, 2, 2),
            (2029, 3, 1),
            (2029, 4, 2),
            (2030, 3, 3),
        ],
    );
    let mut args = fixture.args();
    let model = args.iter().position(|s| s == "--model").unwrap() + 1;
    args[model] = SEASONAL_MODEL.into();
    let (exit, value, bytes) = invoke(&args);
    assert_eq!(exit, 0, "{bytes}");
    assert_eq!(invoke(&args).2, bytes);
    let report = at(&value, &["payload"]);
    assert_eq!(at(report, &["model"]).as_str(), Some(SEASONAL_MODEL));
    assert_eq!(at(report, &["uncalibrated"]).as_bool(), Some(true));
    assert_eq!(at(report, &["issued"]).as_bool(), Some(false));
    assert_eq!(at(report, &["attestation"]), &Json::Null);
    assert_eq!(at(report, &["timestamp_proof"]), &Json::Null);
    let csv = at(report, &["forecasts_csv"]).as_str().unwrap();
    assert_eq!(
        csv,
        concat!(
            "site,target,point,p10,p90,uncalibrated\n",
            "Site A,production,150.00,150.00,375.00,true\n",
            "Site A,unit_cost,1650.00,1650.00,1800.00,true\n",
            "Site B,production,40.00,40.00,100.00,true\n",
            "Site B,unit_cost,3300.00,3300.00,3600.00,true\n",
        )
    );
    assert_eq!(
        parse_forecasts(csv.as_bytes(), &parse_rules(&rules).unwrap())
            .unwrap()
            .len(),
        4
    );
    assert_eq!(at(report, &["input_manifest"]).as_array().unwrap().len(), 5);
    assert_eq!(at(report, &["observations"]).as_array().unwrap().len(), 20);
    for pool in at(report, &["pools"]).as_array().unwrap() {
        assert_eq!(at(pool, &["seasonal_destination_quarter"]).i64(), Some(4));
        assert_eq!(at(pool, &["ratio_count"]).i64(), Some(2));
        assert!(at(pool, &["excluded_pairs"]).as_array().unwrap().is_empty());
        let excluded = at(pool, &["off_season_pairs"]).as_array().unwrap();
        assert_eq!(excluded.len(), 4);
        assert_eq!(at(&excluded[0], &["from"]).as_str(), Some("2029Q1"));
        assert_eq!(at(&excluded[0], &["to"]).as_str(), Some("2029Q2"));
        assert_ne!(at(&excluded[0], &["before"]), &Json::Null);
        assert_ne!(at(&excluded[0], &["after"]), &Json::Null);
    }
    let no_claim = at(&value, &["no_claim"]).as_str().unwrap();
    assert!(no_claim.contains("Synthetic, uncalibrated seasonal draft"));
    assert!(no_claim.contains("all-season persistence comparator is unchanged"));
    assert!(!no_claim.contains("bounds equal"));
    let (exit, default, _) = invoke(&fixture.args());
    assert_eq!(exit, 0);
    let default = at(&default, &["payload"]);
    assert_ne!(
        at(default, &["forecasts_csv"]),
        at(report, &["forecasts_csv"])
    );
    assert_ne!(
        at(default, &["receipt_digest"]),
        at(report, &["receipt_digest"])
    );
    assert_eq!(
        at(default, &["input_manifest"]),
        at(report, &["input_manifest"])
    );
    assert_eq!(
        at(default, &["observations"]),
        at(report, &["observations"])
    );
    let mut markdown = args.clone();
    markdown.extend(["--format".into(), "markdown".into()]);
    let rendered = output(&markdown);
    assert!(rendered.status.success());
    let rendered = String::from_utf8(rendered.stdout).unwrap();
    assert!(rendered.contains("Selected transitions end in calendar Q4"));
    assert!(rendered.contains("4 other-season pairs remain in the JSON audit"));
    assert!(rendered.contains(SEASONAL_MODEL));
    // Even a source used only in excluded seasons must pass the normal byte authentication.
    std::fs::write(
        fixture.root.join("invented/Invented_Q1.xlsx"),
        b"altered off-season source",
    )
    .unwrap();
    let (exit, refusal, _) = invoke(&args);
    assert_ne!(exit, 0);
    assert_eq!(at(&refusal, &["payload"]), &Json::Null);
    assert!(
        at(&refusal, &["refusal", "what"])
            .as_str()
            .unwrap()
            .contains("Invented_Q1.xlsx")
    );
}

#[test]
fn seasonal_cli_refuses_an_empty_selected_season_without_using_all_season_pairs() {
    let fixture = Fixture::new();
    let mut args = fixture.args();
    let model = args.iter().position(|s| s == "--model").unwrap() + 1;
    args[model] = SEASONAL_MODEL.into();
    let (exit, refusal, _) = invoke(&args);
    assert_ne!(exit, 0);
    assert_eq!(at(&refusal, &["payload"]), &Json::Null);
    assert_eq!(
        at(&refusal, &["refusal", "code"]).as_str(),
        Some("FMINE-REF-208")
    );
    assert!(
        at(&refusal, &["refusal", "what"])
            .as_str()
            .unwrap()
            .contains("no positive adjacent-quarter ratio into Q4")
    );
    let (exit, default, _) = invoke(&fixture.args());
    assert_eq!(
        exit, 0,
        "the same source corpus supports the explicitly different model: {default:?}"
    );
}

#[test]
fn additive_cli_forecasts_restart_from_displayed_zero_production_with_source_evidence() {
    for (model, seasonal) in [(CHANGE_MODEL, false), (SEASONAL_CHANGE_MODEL, true)] {
        let rules = if seasonal {
            RULES
                .replace("pool_from = \"2030Q1\"", "pool_from = \"2029Q1\"")
                .replace("pool_through = \"2030Q2\"", "pool_through = \"2029Q4\"")
        } else {
            RULES.into()
        };
        let quarters = if seasonal {
            vec![
                (2029, 1, 1),
                (2029, 2, 2),
                (2029, 3, 4),
                (2029, 4, 5),
                (2030, 3, 7),
            ]
        } else {
            vec![(2030, 1, 4), (2030, 2, 5), (2030, 3, 7)]
        };
        let fixture = Fixture::with_quarters(
            &rules,
            "Gold produced (000s oz, 100%)",
            "All-in sustaining costs ($/oz)",
            &quarters,
        );
        let mut args = fixture.args();
        let model_arg = args.iter().position(|s| s == "--model").unwrap() + 1;
        args[model_arg] = model.into();
        let (exit, value, bytes) = invoke(&args);
        assert_eq!(exit, 0, "{bytes}");
        assert_eq!(invoke(&args).2, bytes);
        let report = at(&value, &["payload"]);
        let csv = at(report, &["forecasts_csv"]).as_str().unwrap();
        let rows = parse_forecasts(csv.as_bytes(), &parse_rules(&rules).unwrap()).unwrap();
        for row in rows.iter().filter(|r| r.target == "production") {
            // Printed changes are 125-100 and 210-200; the displayed latest amounts are both zero.
            assert_eq!(
                (row.lo, row.point, row.hi),
                (
                    Rat::from_integer(10),
                    Rat::from_integer(10),
                    Rat::from_integer(25)
                )
            );
        }
        assert_eq!(at(report, &["issued"]).as_bool(), Some(false));
        assert_eq!(at(report, &["uncalibrated"]).as_bool(), Some(true));
        for row in at(report, &["observations"])
            .as_array()
            .unwrap()
            .iter()
            .filter(|r| {
                at(r, &["target"]).as_str() == Some("production")
                    && at(r, &["role"]).as_str() == Some("point")
            })
        {
            assert_eq!(at(row, &["value"]).as_str(), Some("0"));
            assert_eq!(at(row, &["reason"]), &Json::Null);
            assert_eq!(
                at(row, &["claim", "value", "stored"]).as_str(),
                Some("0.4")
            );
            assert_eq!(
                at(row, &["claim", "value", "displayed"]).as_str(),
                Some("0")
            );
            assert_eq!(at(row, &["sha256"]), at(row, &["claim", "sha256"]));
        }
        args.extend(["--format".into(), "markdown".into()]);
        let markdown = output(&args);
        assert!(markdown.status.success());
        assert!(
            String::from_utf8(markdown.stdout)
                .unwrap()
                .contains("Site A,production,10.00,10.00,25.00,true")
        );
        // Retaining the original ratio model must not silently select this restart assumption.
        let (exit, refused, _) = invoke(&fixture.args());
        assert_ne!(exit, 0);
        assert_eq!(at(&refused, &["payload"]), &Json::Null);
    }
}

#[test]
fn additive_cli_builds_nonzero_width_at_zero_cost_and_keeps_source_bound_change_units() {
    for (model, seasonal) in [(CHANGE_MODEL, false), (SEASONAL_CHANGE_MODEL, true)] {
        let rules = if seasonal {
            RULES
                .replace("pool_from = \"2030Q1\"", "pool_from = \"2029Q1\"")
                .replace("pool_through = \"2030Q2\"", "pool_through = \"2029Q4\"")
        } else {
            RULES.into()
        };
        let quarters = if seasonal {
            vec![
                (2029, 1, 1),
                (2029, 2, 2),
                (2029, 3, 4),
                (2029, 4, 5),
                (2030, 3, 6),
            ]
        } else {
            vec![(2030, 1, 4), (2030, 2, 5), (2030, 3, 6)]
        };
        let fixture = Fixture::with_quarters(
            &rules,
            "Gold produced (000s oz, 100%)",
            "All-in sustaining costs ($/oz)",
            &quarters,
        );
        let mut args = fixture.args();
        let selected = args.iter().position(|s| s == "--model").unwrap() + 1;
        args[selected] = model.into();
        let (exit, value, bytes) = invoke(&args);
        assert_eq!(exit, 0, "{bytes}");
        assert_eq!(invoke(&args).2, bytes, "deterministic complete output");
        let report = at(&value, &["payload"]);
        assert_eq!(
            at(report, &["schema"]).as_str(),
            Some("fmine.forecast-record-candidate.v2")
        );
        assert_eq!(at(report, &["model"]).as_str(), Some(model));
        assert_eq!(at(report, &["issued"]).as_bool(), Some(false));
        assert_eq!(at(report, &["uncalibrated"]).as_bool(), Some(true));
        assert_eq!(at(report, &["attestation"]), &Json::Null);
        assert_eq!(at(report, &["timestamp_proof"]), &Json::Null);
        let csv = at(report, &["forecasts_csv"]).as_str().unwrap();
        assert_eq!(
            csv,
            concat!(
                "site,target,point,p10,p90,uncalibrated\n",
                "Site A,production,310.00,310.00,325.00,true\n",
                "Site A,unit_cost,-20.00,-20.00,10.00,true\n",
                "Site B,production,90.00,90.00,105.00,true\n",
                "Site B,unit_cost,-60.00,-60.00,-30.00,true\n",
            )
        );
        assert_eq!(
            parse_forecasts(csv.as_bytes(), &parse_rules(&rules).unwrap())
                .unwrap()
                .len(),
            4
        );
        assert_eq!(
            at(report, &["observations"]).as_array().unwrap().len(),
            4 * quarters.len()
        );
        for pool in at(report, &["pools"]).as_array().unwrap() {
            assert_eq!(at(pool, &["change_kind"]).as_str(), Some("additive"));
            assert_eq!(at(pool, &["change_count"]).i64(), Some(2));
            assert!(pool.get("ratios").is_none());
            assert!(at(pool, &["excluded_pairs"]).as_array().unwrap().is_empty());
            let unit = if at(pool, &["target"]).as_str() == Some("production") {
                "koz"
            } else {
                "usd_per_oz"
            };
            assert_eq!(at(pool, &["change_unit"]).as_str(), Some(unit));
            if seasonal {
                assert_eq!(at(pool, &["seasonal_destination_quarter"]).i64(), Some(4));
                assert_eq!(at(pool, &["off_season_pairs"]).as_array().unwrap().len(), 4);
            }
        }
        for row in at(report, &["observations"]).as_array().unwrap() {
            assert_eq!(at(row, &["reason"]), &Json::Null);
            assert_ne!(at(row, &["claim"]), &Json::Null);
        }
        let no_claim = at(&value, &["no_claim"]).as_str().unwrap();
        assert!(no_claim.contains("Synthetic, uncalibrated additive-change draft"));
        assert!(no_claim.contains("ratio persistence comparator is unchanged"));
        let mut markdown = args.clone();
        markdown.extend(["--format".into(), "markdown".into()]);
        let rendered = output(&markdown);
        assert!(rendered.status.success());
        let rendered = String::from_utf8(rendered.stdout).unwrap();
        assert!(rendered.contains("additive changes in the target's unit"));
        if seasonal {
            assert!(rendered.contains("Selected transitions end in calendar Q4"));
        }
        // An additive model cannot bypass original-byte admission, even off season.
        std::fs::write(
            fixture.root.join("invented/Invented_Q1.xlsx"),
            b"altered history",
        )
        .unwrap();
        let (exit, refused, _) = invoke(&args);
        assert_ne!(exit, 0);
        assert_eq!(at(&refused, &["payload"]), &Json::Null);
    }
}

#[test]
fn copper_draft_cli_keeps_source_units_and_emits_record_parseable_csv() {
    for (field, header) in [
        ("c1_usd_per_lb", "C1 cash costs ($/lb)"),
        ("aisc_usd_per_lb", "All-in sustaining costs ($/lb)"),
    ] {
        let rules = RULES
            .replace("au_prod_100_koz", "cu_prod_100_kt")
            .replace("gold produced", "copper produced")
            .replace("aisc_usd_per_oz", field)
            .replace("ounce sold", "copper pound sold");
        let fixture = Fixture::with_metrics(&rules, "Copper produced (kt 100%)", header);
        let (exit, value, bytes) = invoke(&fixture.args());
        assert_eq!(exit, 0, "{bytes}");
        let report = at(&value, &["payload"]);
        let csv = at(report, &["forecasts_csv"]).as_str().unwrap();
        let parsed = parse_forecasts(csv.as_bytes(), &parse_rules(&rules).unwrap()).unwrap();
        assert_eq!(parsed.len(), 4);
        let production = parsed
            .iter()
            .find(|r| r.site == "Site A" && r.target == "production")
            .unwrap();
        assert_eq!(production.point, Rat::from_integer(150));
        assert_eq!(production.hi, Rat::from_integer(375));
        assert!(bytes.contains("\"unit\":\"kt\""));
        assert!(bytes.contains("\"unit\":\"usd_per_lb\""));
        assert!(bytes.contains(&format!("summary:{field}")));
        assert!(!bytes.contains("summary:au_prod_100_koz"));
        for target in at(report, &["targets"]).as_array().unwrap() {
            let production = at(target, &["id"]).as_str() == Some("production");
            assert_eq!(
                at(target, &["unit"]).as_str(),
                Some(if production { "kt" } else { "usd_per_lb" })
            );
            assert_eq!(
                at(target, &["basis"]).as_str(),
                Some(if production {
                    "100%"
                } else {
                    "reported_rate_per_copper_pound_sold"
                })
            );
        }
        assert_eq!(invoke(&fixture.args()).2, bytes);
    }
}

#[test]
fn gold_tcc_draft_cli_emits_its_own_metric_and_record_parseable_intervals() {
    for quarterly in [false, true] {
        let rules = RULES
            .replace("aisc_usd_per_oz", "tcc_usd_per_oz")
            .replace("AISC", "total cash costs");
        let rules = if quarterly {
            rules
                .replace("fy_minus_9m", "quarter_qtd")
                .replace("q4_qtd", "quarter_qtd")
        } else {
            rules
        };
        let fixture = Fixture::with_metrics(
            &rules,
            "Gold produced (000s oz, 100%)",
            "Total cash costs ($/oz)",
        );
        let (exit, value, bytes) = invoke(&fixture.args());
        assert_eq!(exit, 0, "{bytes}");
        let report = at(&value, &["payload"]);
        let parsed = parse_forecasts(
            at(report, &["forecasts_csv"]).as_str().unwrap().as_bytes(),
            &parse_rules(&rules).unwrap(),
        )
        .unwrap();
        let cost = parsed
            .iter()
            .find(|r| r.site == "Site A" && r.target == "unit_cost")
            .unwrap();
        assert_eq!(
            (cost.lo, cost.point, cost.hi),
            (
                Rat::from_integer(1650),
                Rat::from_integer(1650),
                Rat::from_integer(1800)
            )
        );
        assert!(bytes.contains("summary:tcc_usd_per_oz"));
        assert!(!bytes.contains("summary:aisc_usd_per_oz"));
        let metadata = at(report, &["targets"])
            .as_array()
            .unwrap()
            .iter()
            .find(|t| at(t, &["id"]).as_str() == Some("unit_cost"))
            .unwrap();
        assert_eq!(at(metadata, &["unit"]).as_str(), Some("usd_per_oz"));
        assert_eq!(
            at(metadata, &["basis"]).as_str(),
            Some("reported_rate_per_ounce_sold")
        );
        assert_eq!(invoke(&fixture.args()).2, bytes);
    }
}

#[test]
fn complete_draft_uses_displayed_original_sources_and_independent_quantile_answers() {
    let fixture = Fixture::new();
    let args = fixture.args();
    let (exit, value, first) = invoke(&args);
    assert_eq!(exit, 0, "{value:?}");
    assert_eq!(
        invoke(&args).2,
        first,
        "identical source bytes give identical receipts"
    );
    assert_eq!(
        at(&value, &["verb"]).as_str(),
        Some("forecast record build")
    );
    let report = at(&value, &["payload"]);
    assert_eq!(
        at(report, &["schema"]).as_str(),
        Some("fmine.forecast-record-candidate.v1")
    );
    assert_eq!(at(report, &["model"]).as_str(), Some(MODEL));
    assert_eq!(at(report, &["status"]).as_str(), Some("draft"));
    for name in ["synthetic", "uncalibrated"] {
        assert_eq!(at(report, &[name]).as_bool(), Some(true));
    }
    assert_eq!(at(report, &["issued"]).as_bool(), Some(false));
    for name in ["attestation", "timestamp_proof"] {
        assert_eq!(at(report, &[name]), &Json::Null);
    }
    assert_eq!(
        at(report, &["not_published_sites"]).as_array().unwrap()[0].as_str(),
        Some("Retired Site")
    );
    let csv = at(report, &["forecasts_csv"]).as_str().unwrap();
    assert_eq!(
        csv,
        concat!(
            "site,target,point,p10,p90,uncalibrated\n",
            "Site A,production,150.00,150.00,375.00,true\n",
            "Site A,unit_cost,1650.00,1650.00,1800.00,true\n",
            "Site B,production,40.00,40.00,100.00,true\n",
            "Site B,unit_cost,3300.00,3300.00,3600.00,true\n",
        )
    );
    let parsed = parse_forecasts(csv.as_bytes(), &parse_rules(RULES).unwrap()).unwrap();
    assert_eq!(parsed.len(), 4);
    assert_eq!(
        at(report, &["exact_forecasts"]),
        at(report, &["serialized_forecasts"])
    );
    assert_eq!(at(report, &["pools"]).as_array().unwrap().len(), 2);
    for pool in at(report, &["pools"]).as_array().unwrap() {
        assert_eq!(at(pool, &["ratio_count"]).i64(), Some(2));
        assert!(at(pool, &["excluded_pairs"]).as_array().unwrap().is_empty());
    }
    let sources = at(report, &["input_manifest"]).as_array().unwrap();
    assert_eq!(sources.len(), 3);
    for source in sources {
        let file = at(source, &["file"]).as_str().unwrap();
        let bytes = std::fs::read(fixture.root.join("invented").join(file)).unwrap();
        assert_eq!(
            at(source, &["sha256"]).as_str(),
            Some(sha256(&bytes).to_hex().as_str())
        );
        assert_eq!(
            at(source, &["published_at"]),
            &Json::Null,
            "retrieval is not a publication date"
        );
    }
    let observations = at(report, &["observations"]).as_array().unwrap();
    assert_eq!(observations.len(), 12);
    let latest = observations
        .iter()
        .find(|row| {
            at(row, &["site"]).as_str() == Some("Site A")
                && at(row, &["target"]).as_str() == Some("production")
                && at(row, &["quarter"]).as_str() == Some("2030Q3")
        })
        .unwrap();
    assert_eq!(at(latest, &["value"]).as_str(), Some("300"));
    assert_eq!(
        at(latest, &["claim", "value", "stored"]).as_str(),
        Some("300.4")
    );
    assert_eq!(
        at(latest, &["claim", "value", "displayed"]).as_str(),
        Some("300")
    );
    for row in observations {
        assert_eq!(at(row, &["claim", "evidence"]).as_str(), Some("synthetic"));
        assert_eq!(at(row, &["sha256"]), at(row, &["claim", "sha256"]));
        assert!(!at(row, &["claim", "at"]).as_str().unwrap().is_empty());
    }
    let no_claim = at(&value, &["no_claim"]).as_str().unwrap();
    assert!(no_claim.contains("Synthetic, uncalibrated draft from invented source workbooks"));
    assert!(no_claim.contains("equal the persistence baseline on the same synthetic inputs"));
    assert!(!no_claim.contains("From registered public evidence"));
    assert!(
        !fixture.root.join("forecasts.csv").exists(),
        "the read-only command must not issue files"
    );
}

#[test]
fn altered_source_and_unavailable_evidence_refuse_without_any_candidate_rows() {
    let fixture = Fixture::new();
    let mut early = fixture.args();
    let cut = early.iter().position(|s| s == "--as-of").unwrap() + 1;
    early[cut] = "2030-10-02T10:00:30Z".into();
    let (exit, refusal, _) = invoke(&early);
    assert_ne!(exit, 0);
    assert_eq!(
        at(&refusal, &["refusal", "code"]).as_str(),
        Some("FMINE-REF-108")
    );
    assert_eq!(at(&refusal, &["payload"]), &Json::Null);
    std::fs::write(
        fixture.root.join("invented/Invented_Q3.xlsx"),
        b"changed source",
    )
    .unwrap();
    let (exit, refusal, _) = invoke(&fixture.args());
    assert_ne!(exit, 0);
    assert_eq!(at(&refusal, &["payload"]), &Json::Null);
    assert!(
        at(&refusal, &["refusal", "what"])
            .as_str()
            .unwrap()
            .contains("Invented_Q3.xlsx")
    );
}

#[test]
fn usage_requires_an_explicit_model_cut_and_unique_options() {
    let fixture = Fixture::new();
    let original = fixture.args();
    let mut variants = Vec::new();
    for key in ["--model", "--rules", "--as-of"] {
        let mut args = original.clone();
        let position = args.iter().position(|s| s == key).unwrap();
        args.drain(position..position + 2);
        variants.push((args, format!("forecast record build requires {key}")));
    }
    for (key, value, message) in [
        (
            "--model",
            "unknown",
            format!("--model requires {MODEL}|{SEASONAL_MODEL}|{CHANGE_MODEL}|{SEASONAL_CHANGE_MODEL}"),
        ),
        (
            "--decimal-places",
            "13",
            "--decimal-places requires an integer from 0 through 12".into(),
        ),
        (
            "--as-of",
            "2030-10-03",
            "--as-of requires YYYY-MM-DDTHH:MM:SSZ".into(),
        ),
    ] {
        let mut args = original.clone();
        let position = args.iter().position(|s| s == key).unwrap();
        args[position + 1] = value.into();
        variants.push((args, message));
    }
    let mut duplicate = original.clone();
    duplicate.extend(["--model".into(), MODEL.into()]);
    variants.push((duplicate, "duplicate option --model".into()));
    for (args, message) in variants {
        let (exit, value, _) = invoke(&args);
        assert_eq!(exit, 2, "{args:?}: {value:?}");
        assert_eq!(at(&value, &["status"]).as_str(), Some("usage_error"));
        assert_eq!(at(&value, &["refusal"]), &Json::Null);
        // The established Envelope::usage_error contract carries only its diagnostic in the payload.
        assert_eq!(at(&value, &["payload"]).as_object().unwrap().len(), 1);
        assert_eq!(
            at(&value, &["payload", "error"]).as_str(),
            Some(message.as_str())
        );
    }
}

#[test]
fn markdown_and_robot_docs_expose_candidate_status_and_export() {
    let fixture = Fixture::new();
    let mut args = fixture.args();
    args.extend(["--format".into(), "markdown".into()]);
    let result = output(&args);
    assert!(result.status.success());
    let text = String::from_utf8(result.stdout).unwrap();
    for expected in [
        "# Draft forecast candidate",
        "**uncalibrated**",
        "Evidence lane: **synthetic**",
        "Synthetic, uncalibrated draft from invented source workbooks",
        "```csv",
        "Site A,production,150.00",
        "Candidate receipt:",
    ] {
        assert!(text.contains(expected), "missing {expected}: {text}");
    }
    assert!(!text.contains("From registered public evidence"));
    let (exit, docs, _) = invoke(&["robot-docs".into()]);
    assert_eq!(exit, 0);
    assert_eq!(
        at(&docs, &["payload", "forecast_candidate", "model"]).as_str(),
        Some(MODEL)
    );
    assert_eq!(
        at(&docs, &["payload", "forecast_candidate", "models"])
            .as_array()
            .unwrap()
            .iter()
            .map(Json::as_str)
            .collect::<Vec<_>>(),
        vec![
            Some(MODEL),
            Some(SEASONAL_MODEL),
            Some(CHANGE_MODEL),
            Some(SEASONAL_CHANGE_MODEL)
        ]
    );
}

#[test]
fn post_quarter_nowcast_cli_exposes_separate_schema_and_frozen_release_policy() {
    let rules = RULES
        .replace("fy_minus_9m", "quarter_qtd_nowcast")
        .replace("q4_qtd", "quarter_qtd_nowcast")
        .replace("2030-10-01T00:00:00Z", "2031-01-01T00:00:00Z")
        .replace("2030-12-31T23:59:59Z", "2031-01-14T23:59:59Z")
        .replacen(
            "[[target]]",
            "earliest_outcome_release_utc = \"2031-01-15T00:00:00Z\"\noutcome_release_basis = \"Synthetic preliminary production announcement\"\n\n[[target]]",
            1,
        );
    let fixture = Fixture::with_metrics(
        &rules,
        "Gold produced (000s oz, 100%)",
        "All-in sustaining costs ($/oz)",
    );
    let mut args = fixture.args();
    let cut_index = args.iter().position(|arg| arg == "--as-of").unwrap() + 1;
    args[cut_index] = "2031-01-05T00:00:00Z".into();
    let (exit, value, _) = invoke(&args);
    assert_eq!(exit, 0, "{value:?}");
    let payload = at(&value, &["payload"]);
    assert_eq!(
        at(payload, &["schema"]).as_str(),
        Some("fmine.forecast-record-nowcast-candidate.v1")
    );
    assert_eq!(at(payload, &["label"]).as_str(), Some("nowcast"));
    assert_eq!(at(payload, &["outcome_quarter"]).as_str(), Some("2030Q4"));
    assert_eq!(
        at(
            payload,
            &["declared_outcome_release", "earliest_release_utc"]
        )
        .as_str(),
        Some("2031-01-15T00:00:00Z")
    );
    assert_eq!(at(payload, &["issued"]).as_bool(), Some(false));
    assert_eq!(at(payload, &["timestamp_proof"]), &Json::Null);
    assert!(
        at(payload, &["targets"])
            .as_array()
            .unwrap()
            .iter()
            .all(|target| { at(target, &["outcome"]).as_str() == Some("quarter_qtd_nowcast") })
    );
    let csv = at(payload, &["forecasts_csv"]).as_str().unwrap();
    assert_eq!(
        parse_forecasts(csv.as_bytes(), &parse_rules(&rules).unwrap())
            .unwrap()
            .len(),
        4
    );
    let mut markdown_args = args.clone();
    markdown_args.extend(["--format".into(), "markdown".into()]);
    let output = output(&markdown_args);
    assert!(output.status.success());
    let markdown = String::from_utf8(output.stdout).unwrap();
    assert!(markdown.contains("Draft post-quarter nowcast candidate"));
    assert!(markdown.contains("scored separately from advance forecasts"));
    assert!(markdown.contains("preliminary production announcement"));
    assert!(!markdown.contains("nowcasts are unsupported"));
    std::fs::write(
        fixture.root.join("rules.toml"),
        RULES
            .replace("fy_minus_9m", "quarter_qtd")
            .replace("q4_qtd", "quarter_qtd"),
    )
    .unwrap();
    let (exit, ordinary, _) = invoke(&args);
    assert_ne!(exit, 0, "{ordinary:?}");
    assert!(format!("{ordinary:?}").contains("FMINE-REF-206"));
}
