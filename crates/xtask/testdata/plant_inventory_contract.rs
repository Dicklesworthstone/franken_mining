// FACT: constructed arithmetic answers test the actual command and its JSON boundary (BD-177).
use fmine_io::omf::json::{self, Json};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

const CLOSED: &str = include_str!("../../../tests/plant/closed_inventory.toml");
const MEASURED: &str = include_str!("../../../tests/plant/measured_inventory.toml");
static SERIAL: AtomicUsize = AtomicUsize::new(0);

fn scratch(text: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "fmine-plant-{}-{}.toml",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::write(&path, text).expect("write constructed worksheet");
    path
}
fn run(mode: &str, path: &Path) -> (i32, Json, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_fmine"))
        .args(["plant", mode, "--input"])
        .arg(path)
        .arg("--json")
        .output()
        .expect("run actual binary");
    assert!(
        output.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let text = String::from_utf8(output.stdout).expect("UTF-8");
    assert_eq!(text.lines().count(), 1, "one NDJSON envelope");
    let value = json::parse(text.as_bytes()).expect("valid finite JSON");
    let status = output.status.code().expect("normal exit");
    assert_eq!(
        value.get("exit_code").and_then(Json::i64),
        Some(i64::from(status))
    );
    for (_, flag) in value.get("flags").unwrap().as_object().unwrap() {
        assert_eq!(flag.as_bool(), Some(false));
    }
    (status, value, text)
}
fn at<'a>(value: &'a Json, path: &[&str]) -> &'a Json {
    path.iter().fold(value, |v, key| {
        v.get(key).unwrap_or_else(|| panic!("missing {key}: {v:?}"))
    })
}
fn numbers(value: &Json) -> Vec<f64> {
    value
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.f64().unwrap())
        .collect()
}
fn exact(value: &Json) -> (i128, i128) {
    (
        value
            .get("numerator")
            .unwrap()
            .as_str()
            .unwrap()
            .parse()
            .unwrap(),
        value
            .get("denominator")
            .unwrap()
            .as_str()
            .unwrap()
            .parse()
            .unwrap(),
    )
}

#[test]
fn exact_balance_preserves_sources_and_reruns_byte_identically() {
    let path = scratch(CLOSED);
    let (code, value, first) = run("balance", &path);
    assert_eq!(code, 0, "{value:?}");
    assert_eq!(
        exact(at(&value, &["payload", "result", "residual_kg"])),
        (0, 1)
    );
    assert_eq!(
        exact(at(&value, &["payload", "result", "inventory_change_kg"])),
        (20, 1)
    );
    assert_eq!(
        exact(at(&value, &["payload", "result", "product_to_feed_ratio"])),
        (18, 25)
    );
    assert_eq!(
        at(&value, &["payload", "result", "ratio_is_recovery"]).as_bool(),
        Some(false)
    );
    assert_eq!(
        at(&value, &["payload", "observations"])
            .as_array()
            .unwrap()
            .len(),
        5
    );
    assert_eq!(
        at(&value, &["payload", "admitted_claim", "evidence"]).as_str(),
        Some("Assumed")
    );
    assert_eq!(
        at(&value, &["payload", "admitted_claim", "confidentiality"]).as_str(),
        Some("Internal")
    );
    assert_eq!(
        at(&value, &["payload", "source", "sha256"]).as_str(),
        Some(
            fmine_core::hash::sha256(CLOSED.as_bytes())
                .to_hex()
                .as_str()
        )
    );
    assert_eq!(run("balance", &path).2, first);
    std::fs::write(&path, format!("{CLOSED}\n# different submitted bytes\n")).unwrap();
    let (_, changed, _) = run("balance", &path);
    assert_ne!(
        at(&changed, &["payload", "receipt_digest"]),
        at(&value, &["payload", "receipt_digest"])
    );
    assert_eq!(
        at(&changed, &["payload", "result"]),
        at(&value, &["payload", "result"])
    );
}

#[test]
fn full_covariance_reconciliation_matches_hand_solution() {
    let (code, value, _) = run("reconcile", &scratch(MEASURED));
    assert_eq!(code, 0, "{value:?}");
    let result = at(&value, &["payload", "result"]);
    assert_eq!(
        numbers(at(result, &["observed_kg"])),
        [10.0, 100.0, 72.0, 8.0, 31.0]
    );
    let expected = [10.0625, 100.25, 71.9375, 7.9375, 30.4375];
    for (a, b) in numbers(at(result, &["estimates_kg"]))
        .into_iter()
        .zip(expected)
    {
        assert!((a - b).abs() < 1e-12);
    }
    assert!((at(result, &["chi_square"]).f64().unwrap() - 0.0625).abs() < 1e-12);
    assert_eq!(exact(at(result, &["raw_residual_exact_kg"])), (-1, 1));
    assert!(at(result, &["reconciled_residual_kg"]).f64().unwrap().abs() < 1e-9);
    let covariance = numbers(at(result, &["posterior_covariance_kg2"]));
    assert_eq!(covariance.len(), 25);
    assert!((covariance[0] - 15.0 / 16.0).abs() < 1e-12);
    assert!((covariance[1] + 1.0 / 4.0).abs() < 1e-12);
    assert!((covariance[24] - 63.0 / 16.0).abs() < 1e-12);
    assert_eq!(
        at(&value, &["payload", "admitted_claim", "state"]).as_str(),
        Some("Estimated")
    );
    assert_eq!(
        at(&value, &["payload", "admitted_claim", "parents"])
            .as_array()
            .unwrap()
            .len(),
        7
    );
}

#[test]
fn gross_errors_retain_observations_and_diagnostics_without_an_admitted_claim() {
    let input = MEASURED.replace("kg = \"31\"", "kg = \"50\"");
    let (code, value, _) = run("reconcile", &scratch(&input));
    assert_ne!(code, 0);
    assert_eq!(at(&value, &["status"]).as_str(), Some("refused"));
    assert_eq!(
        at(&value, &["refusal", "code"]).as_str(),
        Some("FMINE-REF-101")
    );
    assert_eq!(at(&value, &["payload", "admitted_claim"]), &Json::Null);
    assert_eq!(
        at(&value, &["payload", "observations"])
            .as_array()
            .unwrap()
            .len(),
        5
    );
    assert!(
        (at(&value, &["payload", "result", "chi_square"])
            .f64()
            .unwrap()
            - 25.0)
            .abs()
            < 1e-10
    );
    assert_eq!(
        exact(at(&value, &["payload", "result", "raw_residual_exact_kg"])),
        (-20, 1)
    );
    assert!(
        at(&value, &["payload", "receipt_digest"])
            .as_str()
            .is_some()
    );
}

#[test]
fn exact_mode_never_plugs_a_residual_or_ignores_an_error_model() {
    let (code, value, _) = run(
        "balance",
        &scratch(&CLOSED.replace("kg = \"30\"", "kg = \"31\"")),
    );
    assert_ne!(code, 0);
    assert_eq!(
        at(&value, &["refusal", "code"]).as_str(),
        Some("FMINE-REF-101")
    );
    assert_eq!(at(&value, &["payload", "admitted_claim"]), &Json::Null);
    assert_eq!(
        at(&value, &["payload", "observations"])
            .as_array()
            .unwrap()
            .len(),
        5
    );
    assert_ne!(run("balance", &scratch(MEASURED)).0, 0);
}

#[test]
fn strict_worksheet_guards_reject_ambiguous_or_incomplete_measurements() {
    for text in [
        CLOSED.replace("complete = \"true\"", "complete = \"false\""),
        CLOSED.replace(
            "kg-total-element/all-phases/full-physical-boundary",
            "tonnes",
        ),
        CLOSED.replace("role = \"closing\"", "role = \"feed\""),
        CLOSED.replace("role = \"feed\"", "role = \"unrecorded-loss\""),
        CLOSED.replace(
            "role = \"opening\"\nkg = \"10\"\nstart_ns = \"0\"\nend_ns = \"0\"",
            "role = \"opening\"\nkg = \"10\"\nstart_ns = \"0\"\nend_ns = \"10\"",
        ),
        CLOSED.replace("kg = \"100\"", "kg = \"100\"\nvariance = \"1\""),
        CLOSED.replace("kg = \"100\"", "kg = \"1e-2147483648\""),
        CLOSED.replace("kg = \"100\"", "kg = \"1/0\""),
        CLOSED.replace("kg = \"100\"", "kg = \"-100\""),
    ] {
        assert_ne!(run("balance", &scratch(&text)).0, 0, "{text}");
    }
    for text in [
        MEASURED.replace(
            "[[covariance]]\nleft = \"opening\"\nright = \"feed\"\nkg2 = \"0\"",
            "",
        ),
        MEASURED.replace(
            "right = \"feed\"\nkg2 = \"0\"",
            "right = \"opening\"\nkg2 = \"0\"",
        ),
        MEASURED.replace("kg2 = \"9\"", "kg2 = \"NaN\""),
        MEASURED.replace("kg2 = \"9\"", "kg2 = \"0\""),
        MEASURED.replace(
            "unbiased_gaussian_errors = \"true\"",
            "unbiased_gaussian_errors = \"false\"",
        ),
        MEASURED.replace(
            "global_chi_square_limit = \"9\"",
            "global_chi_square_limit = \"0\"",
        ),
    ] {
        assert_ne!(run("reconcile", &scratch(&text)).0, 0, "{text}");
    }
}

#[test]
fn command_grammar_missing_files_and_robot_docs_are_real_binary_contracts() {
    for args in [
        vec!["plant"],
        vec!["plant", "reconcile"],
        vec!["plant", "unknown"],
        vec!["plant", "balance", "--input", "x", "--input", "y"],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_fmine"))
            .args(args)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(2));
        json::parse(&output.stdout).expect("usage JSON");
    }
    assert_ne!(
        run("balance", Path::new("/missing/fmine-plant-input.toml")).0,
        0
    );
    let output = Command::new(env!("CARGO_BIN_EXE_fmine"))
        .arg("robot-docs")
        .output()
        .unwrap();
    assert!(output.status.success());
    let value = json::parse(&output.stdout).unwrap();
    assert_eq!(
        at(&value, &["payload", "plant_inventory", "schema"]).as_str(),
        Some("fmine.plant-inventory.v1")
    );
    let path = scratch(CLOSED);
    let output = Command::new(env!("CARGO_BIN_EXE_fmine"))
        .args(["--format", "markdown", "plant", "balance", "--input"])
        .arg(path)
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(
        String::from_utf8(output.stdout)
            .unwrap()
            .contains("plant balance")
    );
}
