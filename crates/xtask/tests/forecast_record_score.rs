//! BD-076 independent end-to-end known answers. Every source and TSA identity here is synthetic.
//! The local authority proves cryptographic plumbing only, never independent historical existence.

#![cfg(test)]

use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use fmine_calib::record::parse_utc;
use fmine_core::hash::sha256;
use fmine_io::xlsx::{MinimalCell, MinimalSheet, build_minimal_xlsx};

const CUT: &str = "2026-09-30T00:00:00Z";
const AS_OF: &str = "2027-03-01T00:00:00Z";
const CSV: &str = "site,target,point,p10,p90,uncalibrated\nSite A,production,45,40,50,true\nSite A,unit_cost,105,100,110,true\nSite B,production,45,40,50,true\nSite B,unit_cost,105,100,110,true\n";

fn sheet(name: &str, quarter: &str, values: [f64; 2]) -> MinimalSheet {
    let text = |value: &str| MinimalCell::Text(value.into());
    let (year, ending) = match quarter {
        "2025Q1" => (2025.0, "March 31"),
        "2025Q2" => (2025.0, "June 30"),
        "2026Q3" => (2026.0, "September 30"),
        "2026Q4" => (2026.0, "December 31"),
        _ => panic!("fixture quarter"),
    };
    let months = match (name, quarter) {
        ("QTD", _) => "three",
        ("YTD", "2026Q3") => "nine",
        _ => "twelve",
    };
    let mut cells = vec![
        ("A1".into(), text("Summary of Operations")),
        (
            "A3".into(),
            text(&format!("For the {months} months ended {ending},")),
        ),
        ("B3".into(), MinimalCell::Number(year)),
        ("B4".into(), text("Gold produced (000s oz 100%)")),
        ("C4".into(), text("All-in sustaining costs ($/oz)")),
    ];
    for (row, site) in [(5, "Site A"), (6, "Site B"), (7, "Site C")] {
        cells.push((format!("A{row}"), text(&format!("{site} (100%)"))));
        for (col, value) in ["B", "C"].into_iter().zip(values) {
            let cell = if site == "Site C" || (site == "Site B" && quarter == "2026Q4") {
                text("—")
            } else {
                MinimalCell::Number(value)
            };
            cells.push((format!("{col}{row}"), cell));
        }
    }
    MinimalSheet {
        name: name.into(),
        hidden: false,
        cells,
    }
}

fn rules() -> String {
    let mut rules = String::from(
        r#"rules_id = "SYNTHETIC-2026-rules"
record_id = "SYNTHETIC-2026-record"
issuer = "synthetic"
alpha = "0.2"
quantile_estimator = "inverse_ecdf"
rho_low = "0"
rho_high = "0.5"
coverage_level = "0.95"
calibration_claim_min_n = 35
nowcast_from_utc = "2026-10-02T00:00:00Z"
proof_by_utc = "2026-12-31T23:59:59Z"
interval_score = "Width plus ten times the distance outside the central 80 percent interval"
report = "Retain live missing outcomes and compare methods on their shared available sites"
calibration_claim = "Synthetic protocol demonstration, no skill or calibration claim"
outcome_vintage = "Original Q4 release only"
excluded_rows = "Site C is explicitly excluded before this record is stamped"
"#,
    );
    for (target, field, outcome) in [
        ("production", "au_prod_100_koz", "fy_minus_9m"),
        ("unit_cost", "aisc_usd_per_oz", "q4_qtd"),
    ] {
        writeln!(rules, "\n[[target]]\nid = \"{target}\"\nquantity = \"Synthetic {target}\"\nscored_quantity = \"{outcome}\"\nfield = \"{field}\"\noutcome = \"{outcome}\"").unwrap();
        writeln!(rules, "\n[[baseline]]\nid = \"issuer_guidance\"\ntarget = \"{target}\"\nrule = \"Latest agreed pre-cut annual guidance, production residual or unchanged cost\"\nsource = \"Guidance.pdf page 1\"").unwrap();
        writeln!(rules, "\n[[baseline]]\nid = \"run_rate_persistence\"\ntarget = \"{target}\"\nrule = \"Q3 point times pooled consecutive-quarter ratio quantiles\"\nsource = \"Original Q1 Q2 and Q3 benchmark workbooks fixed by these rules\"\npool_from = \"2025Q1\"\npool_through = \"2025Q2\"\npoint_quarter = \"2026Q3\"").unwrap();
    }
    for site in ["Site A", "Site B", "Site C"] {
        let status = if site == "Site C" {
            "not_published"
        } else {
            "live"
        };
        writeln!(rules, "\n[[site]]\nname = \"{site}\"\nstatus = \"{status}\"\nlocator = \"Q2.xlsx:QTD site row\"").unwrap();
        if site == "Site C" {
            rules.push_str("reason = \"Synthetic site excluded in the frozen universe\"\n");
        }
    }
    rules
}

fn workspace() -> PathBuf {
    workspace_with_copper_candidate(false)
}

fn workspace_with_copper_candidate(copper: bool) -> PathBuf {
    workspace_with_signed_cost_forecast(copper, false)
}

fn workspace_with_signed_cost_forecast(copper: bool, signed_cost: bool) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "fmine-record-score-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    for dir in [
        "corpus",
        "record",
        "tsa",
        "registries/sources",
        "registries/transcriptions",
    ] {
        fs::create_dir_all(root.join(dir)).unwrap();
    }
    let books = [
        ("Q1.xlsx", "2025Q1", "2025-04-01", [100.0, 100.0], None),
        ("Q2.xlsx", "2025Q2", "2025-07-01", [110.0, 110.0], None),
        (
            "Q3.xlsx",
            "2026Q3",
            "2026-10-01",
            [40.0, 100.0],
            Some([100.0, 500.0]),
        ),
        (
            "Q4.xlsx",
            "2026Q4",
            "2027-01-15",
            [53.0, 120.0],
            Some([152.0, 999.0]),
        ),
    ];
    let mut registry = String::from(
        "schema = \"fmine.sources.v1\"\nissuer = \"synthetic\"\ncorpus_dir = \"corpus\"\nretrieved_utc = \"2027-02-01T00:00:00Z\"\nurl_prefix = \"https://example.invalid/\"\n",
    );
    let mut inputs = String::new();
    for (file, quarter, published, mut qtd, ytd) in books {
        if signed_cost && quarter == "2026Q4" {
            qtd[1] = -120.0;
        }
        let mut sheets = vec![sheet("QTD", quarter, qtd)];
        if let Some(values) = ytd {
            sheets.push(sheet("YTD", quarter, values));
        }
        if copper {
            for sheet in &mut sheets {
                for (cell, value) in &mut sheet.cells {
                    match cell.as_str() {
                        "B4" => *value = MinimalCell::Text("Copper produced (kt 100%)".into()),
                        "C4" => *value = MinimalCell::Text("C1 cash costs ($/lb)".into()),
                        _ => {}
                    }
                }
            }
        }
        let bytes = build_minimal_xlsx(&sheets);
        fs::write(root.join("corpus").join(file), &bytes).unwrap();
        let (len, hash) = (bytes.len(), sha256(&bytes).to_hex());
        writeln!(registry, "\n[[source]]\nfile = \"{file}\"\nkind = \"mine_stats\"\nquarter = \"{quarter}\"\npath = \"{file}\"\nurl_basis = \"W1 table\"\npublished_at = \"{published}\"\npublished_at_basis = \"FACT: invented release date\"\nbytes = {len}\nsha256 = \"{hash}\"").unwrap();
        if copper {
            writeln!(registry, "retrieved_utc = \"{published}T12:00:00Z\"").unwrap();
        }
    }
    let doc: &[u8] = if copper {
        b"Explicitly synthetic guidance: Site A production 140 - 160 kt; C1 105 - 115 USD per sold copper pound."
    } else {
        b"Explicitly synthetic guidance: Site A production 140 - 160 koz; AISC 105 - 115 USD per sold ounce."
    };
    fs::write(root.join("corpus/Guidance.pdf"), doc).unwrap();
    let (len, hash) = (doc.len(), sha256(doc).to_hex());
    add_input(&mut inputs, "Guidance.pdf", "2026-01-15", len, &hash);
    writeln!(registry, "\n[[document]]\nfile = \"Guidance.pdf\"\ndir = \"corpus\"\nkind = \"mda\"\nurl = \"https://example.invalid/Guidance.pdf\"\nurl_basis = \"W1 table\"\npublished_at = \"2026-01-15\"\npublished_at_basis = \"FACT: invented fixture\"\npages = 1\nbytes = {len}\nsha256 = \"{hash}\"").unwrap();
    for site in ["Site A", "Site B", "Site C"] {
        writeln!(registry, "\n[[subject]]\nname = \"{site}\"").unwrap();
    }
    fs::write(root.join("registries/sources/synthetic.toml"), registry).unwrap();
    let mut transcriptions =
        String::from("schema = \"fmine.transcriptions.v1\"\nissuer = \"synthetic\"\n");
    for (metric, token, unit, basis) in [
        ("production_koz", "140 - 160", "koz", "100%"),
        (
            "aisc_usd_per_oz",
            "105 - 115",
            "usd_per_oz",
            "100%;aisc=issuer-reported-per-sold-oz",
        ),
    ] {
        let (metric, unit, basis) = if copper {
            if metric == "production_koz" {
                ("production_kt", "kt", "100%")
            } else {
                (
                    "c1_usd_per_lb",
                    "usd_per_lb",
                    "100%;c1=issuer-reported-per-sold-lb",
                )
            }
        } else {
            (metric, unit, basis)
        };
        for reader in ["reader-a", "reader-b"] {
            writeln!(transcriptions, "\n[[entry]]\ndoc = \"Guidance.pdf\"\ndoc_sha256 = \"{hash}\"\npage = 1\ntable = \"Synthetic guidance\"\nrow = \"Site A\"\ncol = \"{metric}\"\ntoken = \"{token}\"\nquantity = \"guidance_site_a_{metric}\"\nunit = \"{unit}\"\nbasis = \"{basis}\"\nperiod = \"2026FY\"\nreader = \"{reader}\"").unwrap();
        }
    }
    fs::write(
        root.join("registries/transcriptions/synthetic.toml"),
        transcriptions,
    )
    .unwrap();
    let rules = if copper {
        rules()
            .replace("au_prod_100_koz", "cu_prod_100_kt")
            .replace("aisc_usd_per_oz", "c1_usd_per_lb")
    } else {
        rules()
    };
    let cut = if copper { "2026-10-03T00:00:00Z" } else { CUT };
    fs::write(root.join("record/rules.toml"), &rules).unwrap();
    let csv = if signed_cost {
        CSV.replace(
            "Site A,unit_cost,105,100,110",
            "Site A,unit_cost,-120,-125,-115",
        )
        .replace("Site B,unit_cost,105,100,110", "Site B,unit_cost,0,-5,5")
    } else if copper {
        let (json, exit) = fmine_cli::run(
            &[
                "forecast",
                "record",
                "build",
                "--rules",
                "record/rules.toml",
                "--model",
                "pooled-quarterly-ratio-empirical-v1",
                "--as-of",
                cut,
                "--decimal-places",
                "2",
            ]
            .map(str::to_string),
            &root,
        );
        assert_eq!(exit, 0, "{json}");
        let json = fmine_io::omf::json::parse(json.as_bytes()).unwrap();
        let payload = json.get("payload").unwrap();
        for source in payload.get("input_manifest").unwrap().as_array().unwrap() {
            let s = |key| source.get(key).unwrap().as_str().unwrap();
            let len = source.get("bytes").unwrap().i64().unwrap();
            writeln!(inputs, "\n[[input]]\ndoc_id = \"{}\"\nurl = \"{}\"\npublication_date = \"{}\"\nretrieved_utc = \"{}\"\nbytes = {len}\nsha256 = \"{}\"", s("file"), s("url"), s("published_at"), s("retrieved_utc"), s("sha256")).unwrap();
        }
        payload
            .get("forecasts_csv")
            .unwrap()
            .as_str()
            .unwrap()
            .to_string()
    } else {
        CSV.to_string()
    };
    // Independent construction of the published FMRUL1 and FMREC1 byte framing.
    let rules_hash = sha256(format!("FMRUL1\n{}\n{rules}", rules.len()).as_bytes()).to_hex();
    let manifest = format!(
        "record_id = \"SYNTHETIC-2026-record\"\nauthor_role = \"synthetic-test-author\"\ninformation_cutoff_utc = \"{cut}\"\nlabel = \"nowcast\"\ncontact = \"pre-contact\"\nattestation = \"No non-public issuer information was used to make these forecasts.\"\nrules_sha256 = \"{rules_hash}\"\n{inputs}"
    );
    let record_hash =
        sha256(format!("FMREC1\n{}\n{manifest}{}\n{csv}", manifest.len(), csv.len()).as_bytes())
            .to_hex();
    for (file, bytes) in [
        ("rules.toml", rules.as_str()),
        ("manifest.toml", manifest.as_str()),
        ("forecasts.csv", csv.as_str()),
    ] {
        fs::write(root.join("record").join(file), bytes).unwrap();
    }
    fs::write(root.join("record/rules.sha256"), format!("{rules_hash}\n")).unwrap();
    fs::write(
        root.join("record/record.sha256"),
        format!("{record_hash}\n"),
    )
    .unwrap();
    timestamp_record(&root, &rules_hash, &record_hash);
    root
}

fn add_input(inputs: &mut String, file: &str, published: &str, len: usize, hash: &str) {
    writeln!(inputs, "\n[[input]]\ndoc_id = \"{file}\"\nurl = \"https://example.invalid/{file}\"\npublication_date = \"{published}\"\nretrieved_utc = \"2026-09-29T12:00:00Z\"\nbytes = {len}\nsha256 = \"{hash}\"").unwrap();
}

fn openssl(dir: &Path, args: &str) {
    let output = Command::new("openssl")
        .current_dir(dir)
        .args(args.split_ascii_whitespace())
        .env("LC_ALL", "C")
        .output()
        .expect("OpenSSL 3.x is an explicit prerequisite");
    assert!(
        output.status.success(),
        "openssl {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn timestamp_record(root: &Path, rules_hash: &str, record_hash: &str) {
    let tsa = root.join("tsa");
    fs::write(
        tsa.join("tsa.cnf"),
        r#"[ tsa_config ]
serial = ./serial
crypto_device = builtin
signer_digest = sha256
default_policy = 1.2.3.4.1
digests = sha256
accuracy = secs:0
ordering = no
tsa_name = no
ess_cert_id_chain = no
ess_cert_id_alg = sha256
[ req ]
distinguished_name = dn
prompt = no
[ dn ]
CN = synthetic_test_tsa
[ tsa_ext ]
basicConstraints = CA:FALSE
keyUsage = critical, digitalSignature
extendedKeyUsage = critical, timeStamping
[ ca_ext ]
basicConstraints = critical, CA:TRUE
keyUsage = critical, keyCertSign, cRLSign
"#,
    )
    .unwrap();
    fs::write(tsa.join("serial"), "01\n").unwrap();
    // Fixture arguments are space-free tokens passed directly to OpenSSL, with no shell.
    openssl(
        &tsa,
        "req -x509 -newkey rsa:2048 -nodes -keyout ca.key -out ca.pem -days 2 -subj /CN=synthetic_test_root -config tsa.cnf -extensions ca_ext",
    );
    openssl(
        &tsa,
        "req -newkey rsa:2048 -nodes -keyout tsa.key -out tsa.csr -subj /CN=synthetic_test_tsa -config tsa.cnf",
    );
    openssl(
        &tsa,
        "x509 -req -in tsa.csr -CA ca.pem -CAkey ca.key -CAcreateserial -out tsa.pem -days 2 -extfile tsa.cnf -extensions tsa_ext",
    );
    openssl(
        &tsa,
        "req -x509 -newkey rsa:2048 -nodes -keyout wrong.key -out wrong.pem -days 2 -subj /CN=unrelated_synthetic_root -config tsa.cnf -extensions ca_ext",
    );
    for (name, digest) in [("rules", rules_hash), ("record", record_hash)] {
        if name == "record" {
            std::thread::sleep(Duration::from_secs(2));
        }
        let query = format!("../record/{name}.tsq");
        let receipt = format!("../record/{name}.tsr");
        openssl(
            &tsa,
            &format!("ts -query -digest {digest} -sha256 -cert -out {query}"),
        );
        openssl(
            &tsa,
            &format!(
                "ts -reply -config tsa.cnf -section tsa_config -queryfile {query} -inkey tsa.key -signer tsa.pem -out {receipt}"
            ),
        );
    }
}

fn run(root: &Path, as_of: &str, roots: &str, format: &str) -> (String, u8) {
    fmine_cli::run(
        &[
            "forecast",
            "record",
            "score",
            "--record",
            "record",
            "--as-of",
            as_of,
            "--tsa-roots",
            roots,
            "--format",
            format,
        ]
        .map(str::to_string),
        root,
    )
}

fn member<'a>(json: &'a str, site: &str, target: &str) -> &'a str {
    json.split(&format!(
        "{{\"site\":\"{site}\",\"target\":\"{target}\",\"status\":"
    ))
    .nth(1)
    .expect("named frozen member")
    .split("\"paired\":")
    .next()
    .unwrap()
}

fn assert_score(fragment: &str, score: u32) {
    assert!(
        fragment.contains(&format!(
            "\"interval_score\":{{\"num\":\"{score}\",\"den\":\"1\"}}"
        )),
        "{fragment}"
    );
}

#[test]
#[ignore = "requires OpenSSL 3.x and host clock from 2026-10-03 through 2026-12-31; real TSA time is never backdated"]
fn copper_candidate_is_frozen_and_scores_original_outcomes_with_both_baselines() {
    let now = i64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    )
    .unwrap();
    assert!(
        (parse_utc("2026-10-03T00:00:00Z").unwrap()..parse_utc("2026-12-31T23:59:45Z").unwrap())
            .contains(&now)
    );
    let root = workspace_with_copper_candidate(true);
    let csv = fs::read_to_string(root.join("record/forecasts.csv")).unwrap();
    assert!(csv.contains("Site A,production,44.00,44.00,44.00,true"));
    assert!(csv.contains("Site A,unit_cost,110.00,110.00,110.00,true"));
    let (json, exit) = run(&root, AS_OF, "tsa/ca.pem", "json");
    assert_eq!(exit, 0, "{json}");
    assert_eq!(run(&root, AS_OF, "tsa/ca.pem", "json"), (json.clone(), 0));
    for (target, outcome, model_score, guidance_score) in
        [("production", 52, 80, 20), ("unit_cost", 120, 100, 60)]
    {
        let item = member(&json, "Site A", target);
        assert!(item.contains(&format!("\"outcome\":{{\"status\":\"available\",\"value\":{{\"num\":\"{outcome}\",\"den\":\"1\"}}}}")), "{item}");
        assert_score(item.split("\"baselines\":").next().unwrap(), model_score);
        for (baseline, expected) in [
            ("issuer_guidance", guidance_score),
            ("run_rate_persistence", model_score),
        ] {
            assert_score(
                item.split(&format!("\"baseline\":\"{baseline}\""))
                    .nth(1)
                    .unwrap()
                    .split("{\"baseline\":")
                    .next()
                    .unwrap(),
                expected,
            );
        }
        assert!(member(&json, "Site B", target).contains("\"forecast_score\":null"));
        assert!(member(&json, "Site C", target).starts_with("\"not_published\""));
    }
    for source in [
        "summary:cu_prod_100_kt",
        "summary:c1_usd_per_lb",
        "guidance_site_a_production_kt",
        "guidance_site_a_c1_usd_per_lb",
        "\"unit\":\"kt\"",
        "\"unit\":\"usd_per_lb\"",
    ] {
        assert!(json.contains(source), "{source}: {json}");
    }
    assert!(!json.contains("summary:au_prod_100_koz"));
    let (markdown, exit) = run(&root, AS_OF, "tsa/ca.pem", "markdown");
    assert_eq!(exit, 0, "{markdown}");
    for meaning in [
        "Metric: `cu_prod_100_kt`. Unit: `kt`.",
        "Metric: `c1_usd_per_lb`. Unit: `usd_per_lb`.",
    ] {
        assert!(markdown.contains(meaning), "{meaning}: {markdown}");
    }
    assert!(!markdown.contains("`usd_per_oz`"));
    fs::write(
        root.join("record/forecasts.csv"),
        csv.replace("44.00", "45.00"),
    )
    .unwrap();
    let (changed, exit) = run(&root, AS_OF, "tsa/ca.pem", "json");
    assert_eq!(exit, 8, "{changed}");
    assert!(changed.contains("FMINE-REF-505"));
    eprintln!("copper_forecast_record_score_fixture={}", root.display());
}

#[test]
#[ignore = "requires OpenSSL 3.x and host clock from 2026-10-03 through 2026-12-31; real TSA time is never backdated"]
fn frozen_record_cli_scores_real_local_timestamps_and_keeps_every_named_site() {
    let now = i64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    )
    .unwrap();
    assert!(
        (parse_utc("2026-10-03T00:00:00Z").unwrap()..parse_utc("2026-12-31T23:59:45Z").unwrap())
            .contains(&now),
        "calendar-dependent cryptographic fixture is outside its declared registration window"
    );
    let root = workspace();
    // The information cut precedes Q3. Registration itself uses the real later TSA time and is a nowcast.
    // Benchmark observations are fixed by the rules, not claimed as inputs available to this forecast.
    let manifest = fs::read_to_string(root.join("record/manifest.toml")).unwrap();
    assert_eq!(manifest.matches("[[input]]").count(), 1);
    assert!(manifest.contains("Guidance.pdf") && !manifest.contains(".xlsx"));
    let (json, exit) = run(&root, AS_OF, "tsa/ca.pem", "json");
    assert_eq!(exit, 0, "{json}");
    assert_eq!(run(&root, AS_OF, "tsa/ca.pem", "json"), (json.clone(), 0));
    for expected in [
        "\"lane\":\"synthetic\"",
        "\"gate_eligible\":false",
        "\"calibration_flag\":\"descriptive\"",
        "\"coverage_denominator\":\"frozen_live_members\"",
        "\"same_information_set_claim\":false",
        "\"in_forecast_manifest\":false",
        "Q3.xlsx:YTD!B5",
        "Q4.xlsx:YTD!B5",
        "Q4.xlsx:QTD!C5",
        "Guidance.pdf",
    ] {
        assert!(json.contains(expected), "missing {expected}: {json}");
    }
    assert_eq!(json.matches("\"n_members\":3,\"n_live\":2,\"n_not_published\":1,\"n_resolved\":1,\"n_unresolved\":1,\"n_paired\":1").count(), 2, "{json}");
    for (target, outcome, model_score, guidance_score, persistence_score) in [
        ("production", 52, 30, 20, 80),
        ("unit_cost", 120, 110, 60, 100),
    ] {
        let a = member(&json, "Site A", target);
        assert!(a.contains(&format!("\"outcome\":{{\"status\":\"available\",\"value\":{{\"num\":\"{outcome}\",\"den\":\"1\"}}}}")), "{a}");
        assert_score(a.split("\"baselines\":").next().unwrap(), model_score);
        for (baseline, expected) in [
            ("issuer_guidance", guidance_score),
            ("run_rate_persistence", persistence_score),
        ] {
            let score = a
                .split(&format!("\"baseline\":\"{baseline}\""))
                .nth(1)
                .unwrap()
                .split("{\"baseline\":")
                .next()
                .unwrap();
            assert_score(score, expected);
        }
        assert!(member(&json, "Site B", target).contains("\"forecast_score\":null"));
        assert!(member(&json, "Site C", target).starts_with("\"not_published\""));
    }
    for score in [30, 20, 80, 110, 60, 100] {
        assert!(
            json.contains(&format!(
                "\"paired_mean_interval_score\":{{\"num\":\"{score}\",\"den\":\"1\"}}"
            )),
            "{json}"
        );
    }
    let (markdown, exit) = run(&root, AS_OF, "tsa/ca.pem", "markdown");
    assert_eq!(exit, 0, "{markdown}");
    assert!(markdown.contains("Frozen forecast record assessment"));
    assert!(markdown.contains("Site B") && markdown.contains("Site C"));
    for meaning in [
        "Metric: `au_prod_100_koz`. Unit: `koz`.",
        "Metric: `aisc_usd_per_oz`. Unit: `usd_per_oz`.",
    ] {
        assert!(markdown.contains(meaning), "{meaning}: {markdown}");
    }
    let (early, exit) = run(&root, "2027-01-14T00:00:00Z", "tsa/ca.pem", "json");
    assert_eq!(exit, 0, "{early}");
    assert_eq!(early.matches("\"n_members\":3,\"n_live\":2,\"n_not_published\":1,\"n_resolved\":0,\"n_unresolved\":2,\"n_paired\":0").count(), 2, "{early}");
    assert!(early.contains("after the assessment cut"));
    let (wrong_ca, exit) = run(&root, AS_OF, "tsa/wrong.pem", "json");
    assert_eq!(exit, 8, "{wrong_ca}");
    assert!(wrong_ca.contains("FMINE-REF-505"));
    fs::write(
        root.join("record/forecasts.csv"),
        CSV.replace("45,40,50", "46,40,50"),
    )
    .unwrap();
    let (altered, exit) = run(&root, AS_OF, "tsa/ca.pem", "json");
    assert_eq!(exit, 8, "{altered}");
    assert!(altered.contains("FMINE-REF-505"));
    fs::write(root.join("record/forecasts.csv"), CSV).unwrap();
    let receipt_path = root.join("record/record.tsr");
    let receipt = fs::read(&receipt_path).unwrap();
    let mut corrupt = receipt.clone();
    corrupt[0] ^= 1;
    fs::write(&receipt_path, corrupt).unwrap();
    let (tampered, exit) = run(&root, AS_OF, "tsa/ca.pem", "json");
    assert_eq!(exit, 8, "{tampered}");
    assert!(tampered.contains("FMINE-REF-505"));
    fs::write(&receipt_path, receipt).unwrap();
    assert_eq!(run(&root, AS_OF, "tsa/ca.pem", "json"), (json.clone(), 0));
    fs::write(root.join("assessment.json"), json).unwrap();
    fs::write(root.join("assessment.md"), markdown).unwrap();
    eprintln!("forecast_record_score_fixture={}", root.display());
    // Scratch source files and throwaway private keys remain local; nothing is committed or deleted.
}

#[test]
#[ignore = "requires OpenSSL 3.x and host clock from 2026-10-03 through 2026-12-31; real TSA time is never backdated"]
fn signed_cost_records_preserve_negative_forecasts_outcomes_and_paired_scores() {
    let now = i64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    )
    .unwrap();
    assert!(
        (parse_utc("2026-10-03T00:00:00Z").unwrap()..parse_utc("2026-12-31T23:59:45Z").unwrap())
            .contains(&now)
    );
    for copper in [false, true] {
        let root = workspace_with_signed_cost_forecast(copper, true);
        let csv = fs::read_to_string(root.join("record/forecasts.csv")).unwrap();
        let (json, exit) = run(&root, AS_OF, "tsa/ca.pem", "json");
        assert_eq!(exit, 0, "{json}");
        assert_eq!(run(&root, AS_OF, "tsa/ca.pem", "json"), (json.clone(), 0));
        let a = member(&json, "Site A", "unit_cost");
        assert!(
            a.contains(
                "\"outcome\":{\"status\":\"available\",\"value\":{\"num\":\"-120\",\"den\":\"1\"}}"
            ),
            "{a}"
        );
        assert!(
            a.contains("\"num\":\"-125\"") && a.contains("\"num\":\"-115\""),
            "{a}"
        );
        assert_score(a.split("\"baselines\":").next().unwrap(), 10);
        for (baseline, expected) in [("issuer_guidance", 2260), ("run_rate_persistence", 2300)] {
            assert_score(
                a.split(&format!("\"baseline\":\"{baseline}\""))
                    .nth(1)
                    .unwrap()
                    .split("{\"baseline\":")
                    .next()
                    .unwrap(),
                expected,
            );
        }
        assert!(member(&json, "Site B", "unit_cost").contains("\"forecast_score\":null"));
        assert!(member(&json, "Site C", "unit_cost").starts_with("\"not_published\""));
        assert_eq!(json.matches("\"n_members\":3,\"n_live\":2,\"n_not_published\":1,\"n_resolved\":1,\"n_unresolved\":1,\"n_paired\":1").count(), 2, "{json}");
        assert!(json.contains("Q4.xlsx:QTD!C5"));
        assert!(json.contains(if copper {
            "summary:c1_usd_per_lb"
        } else {
            "summary:aisc_usd_per_oz"
        }));
        assert!(json.contains("\"calibration_flag\":\"descriptive\""));
        let (markdown, exit) = run(&root, AS_OF, "tsa/ca.pem", "markdown");
        assert_eq!(exit, 0, "{markdown}");
        assert!(
            markdown.contains("-120") && markdown.contains("-125") && markdown.contains("-115"),
            "{markdown}"
        );
        // The signed forecast bytes are part of the authenticated record, not a post-proof annotation.
        fs::write(
            root.join("record/forecasts.csv"),
            csv.replace("-125,-115", "-126,-115"),
        )
        .unwrap();
        let (altered, exit) = run(&root, AS_OF, "tsa/ca.pem", "json");
        assert_eq!(exit, 8, "{altered}");
        assert!(altered.contains("FMINE-REF-505"));
        eprintln!("signed_cost_record_fixture={}", root.display());
    }
}
