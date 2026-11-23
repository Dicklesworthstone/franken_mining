// FACT: independent raw-measurement answer through the actual CLI (BD-151/155/177).
use fmine_io::omf::json::{self, Json};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

const WORKSHEET: &str = include_str!("../../../tests/plant/raw_balance.toml");
static SERIAL: AtomicUsize = AtomicUsize::new(0);

fn scratch(text: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "fmine-raw-balance-{}-{}.toml",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::write(&path, text).expect("write synthetic worksheet");
    path
}

fn at<'a>(value: &'a Json, path: &[&str]) -> &'a Json {
    path.iter().fold(value, |value, key| {
        value
            .get(key)
            .unwrap_or_else(|| panic!("missing {key}: {value:?}"))
    })
}

fn run(path: &Path) -> (i32, Json, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_fmine"))
        .args(["plant", "reconcile-raw", "--input"])
        .arg(path)
        .arg("--json")
        .output()
        .expect("run actual binary");
    assert!(
        output.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let text = String::from_utf8(output.stdout).unwrap();
    assert_eq!(text.lines().count(), 1, "one NDJSON envelope");
    let value = json::parse(text.as_bytes()).expect("finite JSON");
    let code = output.status.code().unwrap();
    assert_eq!(at(&value, &["exit_code"]).i64(), Some(i64::from(code)));
    for (_, flag) in at(&value, &["flags"]).as_object().unwrap() {
        assert_eq!(flag.as_bool(), Some(false));
    }
    (code, value, text)
}

#[test]
fn raw_likelihood_command_conserves_extensive_quantities_and_retains_source_bytes() {
    let path = scratch(WORKSHEET);
    let (code, value, first) = run(&path);
    assert_eq!(code, 0, "{value:?}");
    assert_eq!(at(&value, &["verb"]).as_str(), Some("plant reconcile-raw"));
    let payload = at(&value, &["payload"]);
    assert_eq!(
        at(payload, &["schema"]).as_str(),
        Some("fmine.plant-raw-balance-result.v1")
    );
    assert_eq!(at(payload, &["source_bytes"]).as_str(), Some(WORKSHEET));
    for (key, expected) in [
        ("evidence", "Assumed"),
        ("state", "Estimated"),
        ("confidentiality", "Internal"),
        ("calibration", "uncalibrated"),
    ] {
        assert_eq!(at(payload, &["admitted_claim", key]).as_str(), Some(expected));
    }
    let sources = at(payload, &["sources"]).as_array().unwrap();
    assert_eq!(sources.len(), 1);
    let hash = fmine_core::hash::sha256(WORKSHEET.as_bytes()).to_hex();
    assert_eq!(at(&sources[0], &["sha256"]).as_str(), Some(hash.as_str()));
    let diagnostics = at(payload, &["diagnostics"]);
    let streams = at(diagnostics, &["streams"]).as_array().unwrap();
    assert_eq!(streams.len(), 2);
    for (stream, id, quantities) in [
        (&streams[0], "feed", [80.0, 20.0, 8.0]),
        (&streams[1], "product", [40.0, 10.0, 2.0]),
    ] {
        assert_eq!(at(stream, &["id"]).as_str(), Some(id));
        for (key, expected) in ["dry_kg", "water_kg", "elemental_kg"]
            .into_iter()
            .zip(quantities)
        {
            assert!((at(stream, &[key]).f64().unwrap() - expected).abs() < 1e-7);
        }
    }
    assert!(
        (at(diagnostics, &["standardized_misfit"]).f64().unwrap() - 0.818625).abs() < 1e-9
    );
    for residual in at(diagnostics, &["residuals_kg"]).as_array().unwrap() {
        assert!(residual.f64().unwrap().abs() <= 1e-8);
    }
    assert_eq!(
        at(diagnostics, &["local_extensive_covariance_kg2"])
            .as_array()
            .unwrap()
            .len(),
        36
    );
    assert_eq!(run(&path).2, first);
    std::fs::write(&path, format!("{WORKSHEET}\n# same observations, different bytes\n")).unwrap();
    let (code, changed, _) = run(&path);
    assert_eq!(code, 0);
    assert_ne!(
        at(&changed, &["payload", "receipt_digest"]),
        at(payload, &["receipt_digest"])
    );
    assert_eq!(
        at(&changed, &["payload", "diagnostics"]),
        diagnostics
    );
}

#[test]
fn rejected_raw_fit_keeps_diagnostics_and_never_admits_a_claim() {
    let worksheet = WORKSHEET.replace(
        "max_standardized_misfit = \"10\"",
        "max_standardized_misfit = \"0.1\"",
    );
    let (code, value, _) = run(&scratch(&worksheet));
    assert_ne!(code, 0);
    assert_eq!(at(&value, &["status"]).as_str(), Some("refused"));
    assert_eq!(
        at(&value, &["refusal", "code"]).as_str(),
        Some("FMINE-REF-101")
    );
    assert_eq!(at(&value, &["payload", "admitted_claim"]), &Json::Null);
    assert_eq!(
        at(&value, &["payload", "source_bytes"]).as_str(),
        Some(worksheet.as_str())
    );
    assert!(
        (at(&value, &["payload", "diagnostics", "standardized_misfit"])
            .f64()
            .unwrap()
            - 0.818625)
            .abs()
            < 1e-9
    );
    let rejected = WORKSHEET.replace(
        "unbiased_gaussian_raw_errors = \"true\"",
        "unbiased_gaussian_raw_errors = \"false\"",
    );
    let (code, value, _) = run(&scratch(&rejected));
    assert_ne!(code, 0);
    assert_eq!(
        at(&value, &["refusal", "code"]).as_str(),
        Some("FMINE-REF-201")
    );
}

#[test]
fn raw_reconciliation_has_real_command_docs_and_markdown() {
    let output = Command::new(env!("CARGO_BIN_EXE_fmine"))
        .arg("robot-docs")
        .output()
        .unwrap();
    assert!(output.status.success());
    let value = json::parse(&output.stdout).unwrap();
    assert_eq!(
        at(&value, &["payload", "plant_raw_balance", "schema"]).as_str(),
        Some("fmine.plant-raw-balance.v1")
    );
    let output = Command::new(env!("CARGO_BIN_EXE_fmine"))
        .args(["--format", "markdown", "plant", "reconcile-raw", "--input"])
        .arg(scratch(WORKSHEET))
        .output()
        .unwrap();
    assert!(output.status.success());
    let markdown = String::from_utf8(output.stdout).unwrap();
    assert!(markdown.contains("# Raw-measurement plant reconciliation"));
    assert!(markdown.contains("feed") && markdown.contains("product"));
    assert!(markdown.contains("Receipt:"));
}
