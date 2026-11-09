// FACT: independent invented quarterly ratios and source-mutation controls through the actual CLI.
use std::fmt::Write as _;
use std::path::PathBuf;
use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};

use fmine_calib::record::{parse_forecasts, parse_rules};
use fmine_core::hash::sha256;
use fmine_io::omf::json::{self, Json};
use fmine_io::xlsx::{MinimalCell, MinimalSheet, MinimalStyles, build_minimal_xlsx_styled};

const MODEL: &str = "pooled-quarterly-ratio-empirical-v1";
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
        let root = std::env::temp_dir().join(format!(
            "fmine-forecast-candidate-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("invented")).unwrap();
        std::fs::create_dir_all(root.join("registries/sources")).unwrap();
        std::fs::write(root.join("rules.toml"), RULES).unwrap();
        let mut registry = String::from(
            "schema = \"fmine.sources.v1\"\nissuer = \"synthetic\"\ncorpus_dir = \"invented\"\n\
             retrieved_utc = \"2030-10-02T10:00:00Z/10:01:00Z\"\nurl_prefix = \"https://example.invalid/\"\n",
        );
        for quarter in 1..=3 {
            let bytes = workbook(quarter);
            let file = format!("Invented_Q{quarter}.xlsx");
            std::fs::write(root.join("invented").join(&file), &bytes).unwrap();
            writeln!(registry,
                "\n[[source]]\nfile = \"{file}\"\nkind = \"mine_stats\"\nquarter = \"2030Q{quarter}\"\n\
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

fn workbook(quarter: u8) -> Vec<u8> {
    let text = |value: &str| MinimalCell::Text(value.into());
    let styled = |value: &str| MinimalCell::Styled {
        raw: value.into(),
        xf: 1,
    };
    // All four columns retain a 0.4 stored fraction that is absent from their integer display.
    // Production ratios are 125/100 and 100/200; cost ratios are 1100/1000 and 2400/2000.
    let values = match quarter {
        1 => ["100.4", "1000.4", "200.4", "2000.4"],
        2 => ["125.4", "1100.4", "100.4", "2400.4"],
        _ => ["300.4", "1500.4", "80.4", "3000.4"],
    };
    build_minimal_xlsx_styled(
        &[MinimalSheet {
            name: "QTD".into(),
            hidden: false,
            cells: vec![
                ("A1".into(), text("For the three months ended")),
                ("B1".into(), MinimalCell::Number(2030.0)),
                ("B2".into(), text("Gold produced (000s oz, 100%)")),
                ("C2".into(), text("All-in sustaining costs ($/oz)")),
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
        ("--model", "unknown", format!("--model requires {MODEL}")),
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
}
