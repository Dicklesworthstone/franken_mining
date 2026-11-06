// FACT: independent synthetic layer arithmetic exercised through the actual CLI (BD-176/287).
use fmine_io::omf::json::{self, Json};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

const WORKSHEET: &str = include_str!("../../../tests/plant/recovery_card.toml");
const GOLD_INPUT: &str = "[[input]]\nname = \"Au\"\nmeasurand = \"total-Au/dry\"\nunit = \"g_per_dry_t\"\nvalue = \"5.500\"";
const OPERATION: &str = "[[operation]]\nid = \"invented-availability-factor\"\nrevision = \"v1\"\nmultiplier = \"0.950\"";
static SERIAL: AtomicUsize = AtomicUsize::new(0);

fn scratch(text: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "fmine-plant-recovery-{}-{}.toml",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::write(&path, text).expect("write constructed recovery worksheet");
    path
}

fn run(path: &Path) -> (i32, Json, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_fmine"))
        .args(["plant", "recovery", "--input"])
        .arg(path)
        .arg("--json")
        .output()
        .expect("run actual binary");
    assert!(output.stderr.is_empty(), "{:?}", output.stderr);
    let text = String::from_utf8(output.stdout).unwrap();
    assert_eq!(text.lines().count(), 1, "one NDJSON envelope");
    let value = json::parse(text.as_bytes()).expect("valid finite JSON");
    let status = output.status.code().expect("normal exit");
    assert_eq!(at(&value, &["exit_code"]).i64(), Some(i64::from(status)));
    for (_, flag) in at(&value, &["flags"]).as_object().unwrap() {
        assert_eq!(flag.as_bool(), Some(false));
    }
    assert_eq!(at(&value, &["certificate"]), &Json::Null);
    (status, value, text)
}

fn at<'a>(value: &'a Json, path: &[&str]) -> &'a Json {
    path.iter().fold(value, |value, key| {
        value
            .get(key)
            .unwrap_or_else(|| panic!("missing {key}: {value:?}"))
    })
}

fn close(actual: &Json, expected: f64) {
    let actual = actual.f64().expect("finite numeric output");
    assert!((actual - expected).abs() < 2e-14, "{actual} != {expected}");
}

fn exact(value: &Json) -> (i128, i128) {
    (
        at(value, &["numerator"]).as_str().unwrap().parse().unwrap(),
        at(value, &["denominator"])
            .as_str()
            .unwrap()
            .parse()
            .unwrap(),
    )
}

fn changed(old: &str, new: &str) -> String {
    assert!(WORKSHEET.contains(old), "missing test mutation {old}");
    WORKSHEET.replace(old, new)
}

fn refused(text: &str, code: &str) -> Json {
    let (status, value, _) = run(&scratch(text));
    assert_ne!(status, 0, "{value:?}");
    assert_eq!(
        at(&value, &["refusal", "code"]).as_str(),
        Some(code),
        "{value:?}"
    );
    if at(&value, &["payload"]) != &Json::Null {
        for field in ["result", "admitted_claim", "receipt_digest"] {
            assert_eq!(at(&value, &["payload", field]), &Json::Null);
        }
    }
    value
}

#[test]
fn nominal_recovery_preserves_versions_source_claims_and_independent_layer_answers() {
    let path = scratch(WORKSHEET);
    let (status, value, first) = run(&path);
    assert_eq!(status, 0, "{value:?}");
    assert_eq!(
        run(&path).2,
        first,
        "identical source gives identical output"
    );
    assert_eq!(at(&value, &["verb"]).as_str(), Some("plant recovery"));
    assert_eq!(
        at(&value, &["payload", "schema"]).as_str(),
        Some("fmine.plant-recovery-result.v1")
    );
    let result = at(&value, &["payload", "result"]);
    close(at(result, &["test_work_fraction"]), 4.0 / 5.0);
    let intermediate = at(result, &["operational_fractions"]).as_array().unwrap();
    assert_eq!(intermediate.len(), 2);
    close(&intermediate[0], 19.0 / 25.0);
    close(&intermediate[1], 969.0 / 1250.0);
    close(at(result, &["corrected_before_cap"]), 497.0 / 625.0);
    close(at(result, &["recovery_fraction"]), 79.0 / 100.0);
    assert_eq!(at(result, &["cap_applied"]).as_bool(), Some(true));
    assert_eq!(
        at(result, &["route"]).as_str(),
        Some("synthetic-roast-and-leach")
    );
    let model = at(&value, &["payload", "model"]);
    let test_work = at(model, &["test_work", "claim"]);
    let operations = at(model, &["operations"]).as_array().unwrap();
    assert_eq!(
        at(&operations[0], &["raw_multiplier"]).as_str(),
        Some("0.950")
    );
    assert_eq!(exact(at(&operations[0], &["multiplier"])), (19, 20));
    assert_eq!(
        at(&operations[1], &["version", "revision"]).as_str(),
        Some("v2")
    );
    for layer in operations
        .iter()
        .chain([at(model, &["correction"]), at(model, &["route"])])
    {
        assert_eq!(at(layer, &["test_work_claim_id"]), at(test_work, &["id"]));
    }
    let inputs = at(&value, &["payload", "inputs", "values"])
        .as_array()
        .unwrap();
    let gold = inputs
        .iter()
        .find(|row| at(row, &["name"]).as_str() == Some("Au"))
        .unwrap();
    assert_eq!(at(gold, &["raw_value"]).as_str(), Some("5.500"));
    assert_eq!(exact(at(gold, &["value"])), (11, 2));
    assert_eq!(at(gold, &["unit"]).as_str(), Some("g_per_dry_t"));
    let digest = fmine_core::hash::sha256(WORKSHEET.as_bytes()).to_hex();
    assert_eq!(
        at(&value, &["payload", "source", "sha256"]).as_str(),
        Some(digest.as_str())
    );
    for claim in operations.iter().map(|row| at(row, &["claim"])).chain([
        test_work,
        at(model, &["correction", "claim"]),
        at(model, &["route", "claim"]),
        at(&value, &["payload", "inputs", "claim"]),
        at(&value, &["payload", "admitted_claim"]),
    ]) {
        assert_eq!(at(claim, &["evidence"]).as_str(), Some("Assumed"));
        assert_eq!(at(claim, &["confidentiality"]).as_str(), Some("Internal"));
        assert_eq!(at(claim, &["calibration"]).as_str(), Some("uncalibrated"));
        for source in at(claim, &["sources"]).as_array().unwrap() {
            assert_eq!(at(source, &["sha256"]).as_str(), Some(digest.as_str()));
        }
    }
    let admitted = at(&value, &["payload", "admitted_claim"]);
    assert_eq!(at(admitted, &["state"]).as_str(), Some("Estimated"));
    assert_eq!(at(admitted, &["parents"]).as_array().unwrap().len(), 6);
    assert!(
        at(&value, &["no_claim"])
            .as_str()
            .unwrap()
            .contains("Nominal recovery")
    );
}

#[test]
fn output_scale_cap_and_operation_order_follow_explicit_declarations() {
    let (_, original, _) = run(&scratch(WORKSHEET));
    let fractional = changed("70 + 2 * Au - S", "(70 + 2 * Au - S) / 100")
        .replace("output_scale = \"percent\"", "output_scale = \"fraction\"");
    let (status, fraction, _) = run(&scratch(&fractional));
    assert_eq!(status, 0, "{fraction:?}");
    assert_eq!(
        at(&fraction, &["payload", "result"]),
        at(&original, &["payload", "result"])
    );
    let uncapped = changed("upper_cap_fraction = \"0.79\"", "");
    let (status, value, _) = run(&scratch(&uncapped));
    assert_eq!(status, 0, "{value:?}");
    close(
        at(&value, &["payload", "result", "recovery_fraction"]),
        497.0 / 625.0,
    );
    assert_eq!(
        at(&value, &["payload", "result", "cap_applied"]).as_bool(),
        Some(false)
    );
    assert_eq!(
        at(
            &value,
            &["payload", "model", "correction", "upper_cap_fraction"]
        ),
        &Json::Null
    );
    let reordered = format!("{}\n{OPERATION}\n", changed(OPERATION, ""));
    let (status, value, _) = run(&scratch(&reordered));
    assert_eq!(status, 0, "{value:?}");
    let fractions = at(&value, &["payload", "result", "operational_fractions"])
        .as_array()
        .unwrap();
    close(&fractions[0], 102.0 / 125.0);
    close(&fractions[1], 969.0 / 1250.0);
}

#[test]
fn source_bytes_and_version_revisions_change_receipts_without_altering_numbers() {
    let path = scratch(WORKSHEET);
    let (_, original, _) = run(&path);
    for text in [
        format!("{WORKSHEET}\n# source-byte-only change\n"),
        changed("revision = \"v1\"", "revision = \"v3\""),
    ] {
        std::fs::write(&path, text).unwrap();
        let (status, updated, _) = run(&path);
        assert_eq!(status, 0, "{updated:?}");
        assert_ne!(
            at(&original, &["payload", "source", "sha256"]),
            at(&updated, &["payload", "source", "sha256"])
        );
        assert_ne!(
            at(&original, &["payload", "receipt_digest"]),
            at(&updated, &["payload", "receipt_digest"])
        );
        assert_ne!(
            at(&original, &["payload", "model", "test_work", "claim", "id"]),
            at(&updated, &["payload", "model", "test_work", "claim", "id"])
        );
        assert_eq!(
            at(&original, &["payload", "result"]),
            at(&updated, &["payload", "result"])
        );
    }
}

#[test]
fn coordinate_units_measurands_and_exact_validity_are_never_inferred() {
    for input in [
        GOLD_INPUT.replace("g_per_dry_t", "oz_per_dry_st"),
        GOLD_INPUT.replace("total-Au/dry", "partial-Au/dry"),
    ] {
        let value = refused(&changed(GOLD_INPUT, &input), "FMINE-REF-107");
        assert!(
            at(&value, &["payload", "model", "test_work", "claim"])
                .as_object()
                .is_some()
        );
    }
    refused(
        &changed(
            "processing_support = \"true\"",
            "processing_support = \"false\"",
        ),
        "FMINE-REF-102",
    );
    refused(
        &changed("end_ns = \"20\"", "end_ns = \"101\""),
        "FMINE-REF-203",
    );
    refused(
        &changed("end_ns = \"20\"", "end_ns = \"10\""),
        "FMINE-REF-109",
    );
    let at_upper = changed("upper = \"10.0\"", "upper = \"11/2\"");
    assert_eq!(
        run(&scratch(&at_upper)).0,
        0,
        "closed exact boundary is admitted"
    );
    refused(
        &at_upper.replace("upper_inclusive = \"true\"", "upper_inclusive = \"false\""),
        "FMINE-REF-203",
    );
    let beyond_exact = changed("upper = \"10.0\"", "upper = \"5.50000000000000001\"")
        .replace("value = \"5.500\"", "value = \"5.50000000000000002\"");
    refused(&beyond_exact, "FMINE-REF-203");
    refused(&changed(GOLD_INPUT, ""), "FMINE-REF-201");
    refused(
        &changed("name = \"Au\"\nmeasurand", "name = \"Unknown\"\nmeasurand"),
        "FMINE-REF-201",
    );
}

#[test]
fn malformed_or_impossible_models_refuse_while_preserving_parsed_declarations() {
    for (old, new, code) in [
        ("70 + 2 * Au - S", "(70 + 2 * Au - S", "FMINE-REF-201"),
        ("70 + 2 * Au - S", "1 / (Au - 5.5)", "FMINE-REF-109"),
        ("70 + 2 * Au - S", "exp(1000)", "FMINE-REF-207"),
        (
            "multiplier = \"0.950\"",
            "multiplier = \"-1\"",
            "FMINE-REF-109",
        ),
        (
            "upper_cap_fraction = \"0.79\"",
            "upper_cap_fraction = \"1.1\"",
            "FMINE-REF-109",
        ),
        (
            "additive_fraction = \"0.02\"",
            "additive_fraction = \"-2\"",
            "FMINE-REF-109",
        ),
    ] {
        let value = refused(&changed(old, new), code);
        assert!(at(&value, &["payload", "model"]).as_object().is_some());
        assert_eq!(
            at(&value, &["payload", "inputs", "values"])
                .as_array()
                .unwrap()
                .len(),
            2
        );
    }
    refused(&format!("{WORKSHEET}\n{OPERATION}\n"), "FMINE-REF-109");
    let beyond_one = changed("70 + 2 * Au - S", "200").replace("upper_cap_fraction = \"0.79\"", "");
    refused(&beyond_one, "FMINE-REF-109");
}

#[test]
fn strict_schema_and_robot_docs_make_the_workflow_discoverable() {
    for text in [
        changed("schema = \"fmine.plant-recovery.v1\"", "schema = \"wrong\""),
        changed(
            "processing_support = \"true\"",
            "processing_support = \"yes\"",
        ),
        changed("formula =", "unexpected_formula ="),
        changed("value = \"5.500\"", "value = \"NaN\""),
        changed("value = \"5.500\"", "value = \"1/0\""),
        format!("{WORKSHEET}\n{GOLD_INPUT}\n"),
        format!("{WORKSHEET}\n[[test_work]]\nid = \"duplicate\"\n"),
        format!("{WORKSHEET}\n[[forecast]]\nid = \"unsupported\"\n"),
        "#".repeat(1024 * 1024 + 1),
    ] {
        refused(&text, "FMINE-REF-506");
    }
    let missing = scratch(WORKSHEET).with_extension("missing");
    let (status, value, _) = run(&missing);
    assert_ne!(status, 0);
    assert_eq!(
        at(&value, &["refusal", "code"]).as_str(),
        Some("FMINE-REF-506")
    );
    let output = Command::new(env!("CARGO_BIN_EXE_fmine"))
        .arg("robot-docs")
        .output()
        .unwrap();
    assert!(output.status.success());
    let docs = json::parse(&output.stdout).unwrap();
    assert_eq!(
        at(&docs, &["payload", "plant_recovery", "schema"]).as_str(),
        Some("fmine.plant-recovery.v1")
    );
    assert!(
        at(&docs, &["payload", "plant_recovery", "coordinates"])
            .as_str()
            .unwrap()
            .contains("no implicit conversions")
    );
}
