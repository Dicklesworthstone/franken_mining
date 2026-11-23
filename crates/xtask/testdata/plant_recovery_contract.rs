// FACT: independent synthetic layer arithmetic exercised through the actual CLI (BD-176/287).
use fmine_io::omf::json::{self, Json};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

const WORKSHEET: &str = include_str!("../../../tests/plant/recovery_card.toml");
const SCENARIOS: &str = include_str!("../../../tests/plant/recovery_scenarios.toml");
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
fn joint_scenarios_propagate_nonlinearity_with_exact_weights_and_complete_receipts() {
    let path = scratch(SCENARIOS);
    let (status, value, first) = run(&path);
    assert_eq!(status, 0, "{value:?}");
    assert_eq!(run(&path).2, first);
    close(at(&value, &["payload", "result", "recovery_fraction"]), 0.25);
    let ensemble = at(&value, &["payload", "ensemble"]);
    assert_eq!(at(ensemble, &["calibration"]).as_str(), Some("uncalibrated"));
    let summary = at(ensemble, &["summary"]);
    for (field, expected) in [("mean", 0.5), ("variance", 0.25), ("minimum", 0.0),
        ("p10", 0.0), ("p50", 0.0), ("p90", 1.0), ("maximum", 1.0)] {
        close(at(summary, &[field]), expected);
    }
    assert_eq!(exact(at(summary, &["probability_capped"])), (0, 1));
    let members = at(ensemble, &["members"]).as_array().unwrap();
    assert_eq!(members.len(), 2);
    let hash = fmine_core::hash::sha256(SCENARIOS.as_bytes()).to_hex();
    for (member, id, recovery) in [(&members[0], "high", 1.0), (&members[1], "low", 0.0)] {
        assert_eq!(at(member, &["id"]).as_str(), Some(id));
        assert_eq!(exact(at(member, &["weight"])), (1, 2));
        close(at(member, &["result", "recovery_fraction"]), recovery);
        for claim in [at(member, &["admitted_claim"]), at(member, &["inputs", "claim"])] {
            assert_eq!(at(claim, &["evidence"]).as_str(), Some("Assumed"));
            assert_eq!(at(claim, &["confidentiality"]).as_str(), Some("Internal"));
            assert_eq!(at(claim, &["calibration"]).as_str(), Some("uncalibrated"));
            for source in at(claim, &["sources"]).as_array().unwrap() {
                assert_eq!(at(source, &["sha256"]).as_str(), Some(hash.as_str()));
            }
        }
    }
    let declarations = at(&value, &["payload", "population", "scenarios"]).as_array().unwrap();
    assert_eq!(at(&declarations[1], &["raw_weight"]).as_str(), Some("0.500"));
    assert_eq!(at(&declarations[0], &["inputs", "values"]).as_array().unwrap()[0]
        .get("raw_value").unwrap().as_str(), Some("2.000"));
    let (_, nominal, _) = run(&scratch(WORKSHEET));
    assert!(at(&nominal, &["payload"]).get("ensemble").is_none());
    assert!(at(&nominal, &["payload"]).get("population").is_none());
    std::fs::write(&path, format!("{SCENARIOS}\n# new source bytes\n")).unwrap();
    let (_, revised, _) = run(&path);
    assert_eq!(at(&revised, &["payload", "ensemble", "summary"]), summary);
    assert_ne!(at(&revised, &["payload", "ensemble", "receipt_digest"]), at(ensemble, &["receipt_digest"]));
}

#[test]
fn joint_pairing_and_per_member_caps_change_recovery_without_inventing_independence() {
    let requirement = "\n[[requirement]]\nname = \"S\"\nmeasurand = \"total-S/dry\"\nunit = \"dry_mass_fraction\"\nlower = \"0\"\nupper = \"1\"\nlower_inclusive = \"true\"\nupper_inclusive = \"true\"\n";
    let input = |scenario: &str, value: &str| format!(
        "\n[[input]]\n{scenario}name = \"S\"\nmeasurand = \"total-S/dry\"\nunit = \"dry_mass_fraction\"\nvalue = \"{value}\"\n");
    let shared = SCENARIOS.replace("Au^2 / 4", "Au * S / 2")
        + requirement + &input("", "1/2");
    for (low, high, expected) in [("0", "1", 0.5), ("1", "0", 0.0)] {
        let paired = shared.clone() + &input("scenario = \"low\"\n", low)
            + &input("scenario = \"high\"\n", high);
        let (status, value, _) = run(&scratch(&paired));
        assert_eq!(status, 0, "{value:?}");
        close(at(&value, &["payload", "ensemble", "summary", "mean"]), expected);
        close(at(&value, &["payload", "result", "recovery_fraction"]), 0.25);
    }
    let capped = SCENARIOS.replace("Au^2 / 4", "0.6 + 0.3 * Au")
        .replace("additive_fraction = \"0\"", "additive_fraction = \"0\"\nupper_cap_fraction = \"0.8\"");
    let (status, value, _) = run(&scratch(&capped));
    assert_eq!(status, 0, "{value:?}");
    close(at(&value, &["payload", "ensemble", "summary", "mean"]), 0.7);
    close(at(&value, &["payload", "ensemble", "summary", "variance"]), 0.01);
    assert_eq!(exact(at(&value, &["payload", "ensemble", "summary", "probability_capped"])), (1, 2));
    close(at(&value, &["payload", "result", "recovery_fraction"]), 0.8);
}

#[test]
fn invalid_joint_members_refuse_atomically_without_dropping_or_renormalizing() {
    for text in [
        SCENARIOS.replace("weight = \"0.500\"", "weight = \"0\""),
        SCENARIOS.replace("weight = \"0.500\"", "weight = \"1/3\""),
        SCENARIOS.replace("value = \"2.000\"", "value = \"2.001\""),
        SCENARIOS.replace("scenario = \"high\"", "scenario = \"unlisted\""),
        SCENARIOS.replace("id = \"high\"", "id = \"low\""),
        SCENARIOS.replace("scenario = \"high\"\nname = \"Au\"", "scenario = \"high\"\nname = \"wrong\""),
    ] {
        let (status, value, _) = run(&scratch(&text));
        assert_ne!(status, 0, "{value:?}");
        let payload = at(&value, &["payload"]);
        if payload != &Json::Null {
            for field in ["result", "admitted_claim", "receipt_digest", "ensemble"] {
                assert_eq!(at(payload, &[field]), &Json::Null);
            }
            assert_eq!(at(payload, &["population", "scenarios"]).as_array().unwrap().len(), 2);
        }
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

const FIT: &str = r#"
[[correction_fit]]
id = "synthetic-paired-adjustment"
revision = "fit-v1"
method = "unweighted_additive_residual_mean"
baseline = "post_operational_pre_correction"
alignment = "same_processing_support"
evidence_cut_ns = "9"

[[correction_pair]]
id = "first"
start_ns = "0"
end_ns = "3"
available_ns = "3"
baseline_fraction = "0.600"
observed_fraction = "0.620"
uncensored = "true"

[[correction_pair]]
id = "second"
start_ns = "3"
end_ns = "6"
available_ns = "6"
baseline_fraction = "0.700"
observed_fraction = "0.760"
uncensored = "true"
"#;

#[test]
fn aligned_observations_fit_an_exact_replacement_and_retain_the_original_model() {
    let text = format!("{WORKSHEET}{FIT}");
    let path = scratch(&text);
    let (status, value, bytes) = run(&path);
    assert_eq!(status, 0, "{value:?}");
    assert_eq!(run(&path).2, bytes);
    let payload = at(&value, &["payload"]);
    // (.62-.60 + .76-.70)/2 = .04. The previous +.02 and .79 cap
    // are retained for audit, but are both replaced, not applied again.
    let fitted = at(payload, &["correction_fit_result"]);
    assert_eq!(at(fitted, &["pair_count"]).i64(), Some(2));
    assert_eq!(exact(at(fitted, &["mean_residual_fraction"])), (1, 25));
    assert_eq!(exact(at(fitted, &["sample_residual_variance"])), (1, 1250));
    let residuals = at(fitted, &["residuals"]).as_array().unwrap();
    assert_eq!(exact(at(&residuals[0], &["residual_fraction"])), (1, 50));
    assert_eq!(exact(at(&residuals[1], &["residual_fraction"])), (3, 50));
    close(at(payload, &["result", "recovery_fraction"]), 1019.0 / 1250.0);
    assert_eq!(at(payload, &["result", "cap_applied"]).as_bool(), Some(false));
    assert_eq!(
        exact(at(payload, &["model", "correction", "additive_fraction"])),
        (1, 50)
    );
    assert_eq!(
        exact(at(payload, &["model", "correction", "upper_cap_fraction"])),
        (79, 100)
    );
    let correction = at(fitted, &["correction"]);
    assert_eq!(exact(at(correction, &["additive_fraction"])), (1, 25));
    assert_eq!(at(correction, &["upper_cap_fraction"]), &Json::Null);
    let validity = at(correction, &["fit_validity"]);
    for (key, expected) in [
        ("evidence_cut_ns", "9"),
        ("target_start_ns", "10"),
        ("target_end_ns", "20"),
    ] {
        assert_eq!(at(validity, &[key]).as_str(), Some(expected));
    }
    assert_eq!(at(validity, &["fit_digest"]), at(fitted, &["receipt_digest"]));
    let declaration = at(payload, &["correction_fit"]);
    assert_eq!(
        at(declaration, &["operation_claim_ids"]),
        at(validity, &["operation_claim_ids"])
    );
    assert_eq!(
        at(declaration, &["route_claim_id"]),
        at(validity, &["route_claim_id"])
    );
    let pairs = at(declaration, &["pairs"]).as_array().unwrap();
    assert_eq!(at(&pairs[0], &["raw_baseline_fraction"]).as_str(), Some("0.600"));
    assert_eq!(at(&pairs[1], &["raw_observed_fraction"]).as_str(), Some("0.760"));
    let hash = fmine_core::hash::sha256(text.as_bytes()).to_hex();
    for claim in [
        at(fitted, &["claim"]),
        at(correction, &["claim"]),
        at(declaration, &["claim"]),
        at(&pairs[0], &["claim"]),
        at(&pairs[1], &["claim"]),
        at(payload, &["admitted_claim"]),
    ] {
        assert_eq!(at(claim, &["evidence"]).as_str(), Some("Assumed"));
        assert_eq!(at(claim, &["confidentiality"]).as_str(), Some("Internal"));
        assert_eq!(at(claim, &["calibration"]).as_str(), Some("uncalibrated"));
        for anchor in at(claim, &["sources"]).as_array().unwrap() {
            assert_eq!(at(anchor, &["sha256"]).as_str(), Some(hash.as_str()));
        }
    }
    assert!(at(&value, &["no_claim"]).as_str().unwrap().contains("Sample residual variance"));

    let revised = text.replace("0.620", "0.630").replace("0.760", "0.750");
    std::fs::write(&path, &revised).unwrap();
    let (status, changed, _) = run(&path);
    assert_eq!(status, 0, "{changed:?}");
    let changed = at(&changed, &["payload"]);
    assert_eq!(at(changed, &["result"]), at(payload, &["result"]));
    assert_eq!(
        exact(at(changed, &["correction_fit_result", "sample_residual_variance"])),
        (1, 5000)
    );
    assert_ne!(at(changed, &["receipt_digest"]), at(payload, &["receipt_digest"]));
    assert_ne!(
        at(changed, &["correction_fit_result", "receipt_digest"]),
        at(fitted, &["receipt_digest"])
    );

    let capped = format!("{WORKSHEET}{}", FIT.replace(
        "evidence_cut_ns = \"9\"",
        "evidence_cut_ns = \"9\"\nupper_cap_fraction = \"0.8\""
    ));
    let (status, capped, _) = run(&scratch(&capped));
    assert_eq!(status, 0, "{capped:?}");
    close(at(&capped, &["payload", "result", "recovery_fraction"]), 0.8);
    assert_eq!(at(&capped, &["payload", "result", "cap_applied"]).as_bool(), Some(true));
}

#[test]
fn correction_fit_timing_alignment_censoring_and_incomplete_samples_refuse_atomically() {
    for (from, to, code) in [
        ("evidence_cut_ns = \"9\"", "evidence_cut_ns = \"10\"", "FMINE-REF-203"),
        ("available_ns = \"6\"", "available_ns = \"10\"", "FMINE-REF-203"),
        ("available_ns = \"6\"", "available_ns = \"5\"", "FMINE-REF-203"),
        ("start_ns = \"3\"", "start_ns = \"2\"", "FMINE-REF-102"),
        ("uncensored = \"true\"", "uncensored = \"false\"", "FMINE-REF-202"),
        ("observed_fraction = \"0.760\"", "observed_fraction = \"1.001\"", "FMINE-REF-109"),
        ("id = \"second\"", "id = \"first\"", "FMINE-REF-201"),
    ] {
        let text = format!("{WORKSHEET}{}", FIT.replace(from, to));
        let value = refused(&text, code);
        let payload = at(&value, &["payload"]);
        assert_eq!(at(payload, &["correction_fit_result"]), &Json::Null);
        assert_eq!(at(payload, &["correction_fit", "pairs"]).as_array().unwrap().len(), 2);
        assert!(at(payload, &["model", "correction", "claim"]).as_object().is_some());
    }
    for (from, to) in [
        ("unweighted_additive_residual_mean", "implicit_regression"),
        ("post_operational_pre_correction", "raw_test_work"),
        ("same_processing_support", "equal_timestamps"),
    ] {
        refused(&format!("{WORKSHEET}{}", FIT.replace(from, to)), "FMINE-REF-506");
    }
    let (declaration, pairs) = FIT.split_once("[[correction_pair]]").unwrap();
    refused(&format!("{WORKSHEET}[[correction_pair]]{pairs}"), "FMINE-REF-506");
    refused(&format!("{WORKSHEET}{FIT}{declaration}"), "FMINE-REF-506");
    let one_pair = FIT.split("[[correction_pair]]").take(2).collect::<Vec<_>>().join("[[correction_pair]]");
    let value = refused(&format!("{WORKSHEET}{one_pair}"), "FMINE-REF-201");
    assert_eq!(at(&value, &["payload", "correction_fit_result"]), &Json::Null);
    let rtd = FIT.replace("same_processing_support", "residence_time_aligned");
    assert_eq!(run(&scratch(&format!("{WORKSHEET}{rtd}"))).0, 0);
}

const HISTORICAL_INPUTS: &str = r#"
[[correction_input]]
pair = "first"
name = "Au"
measurand = "total-Au/dry"
unit = "g_per_dry_t"
value = "1.000"

[[correction_input]]
pair = "first"
name = "S"
measurand = "total-S/dry"
unit = "dry_mass_percent"
value = "1.0"

[[correction_input]]
pair = "second"
name = "Au"
measurand = "total-Au/dry"
unit = "g_per_dry_t"
value = "2.000"

[[correction_input]]
pair = "second"
name = "S"
measurand = "total-S/dry"
unit = "dry_mass_percent"
value = "1.0"
"#;

fn evaluated_fit() -> String {
    let model = WORKSHEET
        .replace("70 + 2 * Au - S", "0.125 * Au^2")
        .replace("output_scale = \"percent\"", "output_scale = \"fraction\"")
        .replace("multiplier = \"0.950\"", "multiplier = \"0.5\"")
        .replace("multiplier = \"1.02\"", "multiplier = \"1\"")
        .replace("value = \"5.500\"", "value = \"2.5\"");
    let fit = FIT
        .replace("post_operational_pre_correction", "evaluated_post_operational_pre_correction")
        .replace("baseline_fraction = \"0.600\"\n", "")
        .replace("baseline_fraction = \"0.700\"\n", "")
        .replace("observed_fraction = \"0.620\"", "observed_fraction = \"0.09375\"")
        .replace("observed_fraction = \"0.760\"", "observed_fraction = \"0.34375\"");
    format!("{model}{fit}{HISTORICAL_INPUTS}")
}

#[test]
fn original_process_inputs_produce_nonlinear_baselines_and_exact_fitted_adjustment() {
    let text = evaluated_fit();
    let path = scratch(&text);
    let (status, value, bytes) = run(&path);
    assert_eq!(status, 0, "{value:?}");
    assert_eq!(run(&path).2, bytes);
    let payload = at(&value, &["payload"]);
    let fitted = at(payload, &["correction_fit_result"]);
    // Independently: Au^2 / 8 followed by 1/2 and 1 gives 1/16 and 1/4.
    // Observations 3/32 and 11/32 give residuals 1/32 and 3/32:
    // mean 1/16, sample variance 1/512. At Au=5/2 the final is 25/64+1/16=29/64.
    assert_eq!(exact(at(fitted, &["mean_residual_fraction"])), (1, 16));
    assert_eq!(exact(at(fitted, &["sample_residual_variance"])), (1, 512));
    close(at(payload, &["result", "recovery_fraction"]), 29.0 / 64.0);
    assert_eq!(at(payload, &["result", "cap_applied"]).as_bool(), Some(false));
    assert_eq!(
        at(fitted, &["baseline_evaluation"]).as_str(),
        Some("evaluated_post_operational_pre_correction")
    );
    let pairs = at(fitted, &["derived_pairs"]).as_array().unwrap();
    assert_eq!(pairs.len(), 2);
    let hash = fmine_core::hash::sha256(text.as_bytes()).to_hex();
    for (pair, raw, baseline) in [(&pairs[0], 1.0 / 8.0, (1, 16)), (&pairs[1], 1.0 / 2.0, (1, 4))] {
        close(at(pair, &["test_work_fraction"]), raw);
        assert_eq!(exact(at(pair, &["baseline_fraction"])), baseline);
        let steps = at(pair, &["operational_fractions"]).as_array().unwrap();
        assert_eq!(steps.len(), 2);
        for step in steps {
            close(step, raw / 2.0);
        }
        assert!(at(pair, &["pair_claim", "parents"])
            .as_array().unwrap()
            .contains(at(pair, &["baseline_claim", "id"])));
        for name in ["baseline_claim", "pair_claim", "input_claim", "observation_claim"] {
            let claim = at(pair, &[name]);
            assert_eq!(at(claim, &["evidence"]).as_str(), Some("Assumed"));
            assert_eq!(at(claim, &["confidentiality"]).as_str(), Some("Internal"));
            assert_eq!(at(claim, &["calibration"]).as_str(), Some("uncalibrated"));
            for anchor in at(claim, &["sources"]).as_array().unwrap() {
                assert_eq!(at(anchor, &["sha256"]).as_str(), Some(hash.as_str()));
            }
        }
    }
    assert!(at(fitted, &["claim", "parents"])
        .as_array().unwrap()
        .contains(at(fitted, &["input_claim", "id"])));
    assert_eq!(at(payload, &["correction_fit", "claim"]), &Json::Null);
    let declarations = at(payload, &["correction_fit", "pairs"]).as_array().unwrap();
    let inputs = at(&declarations[0], &["inputs", "values"]).as_array().unwrap();
    assert_eq!(at(&inputs[0], &["raw_value"]).as_str(), Some("1.000"));
    assert!(declarations[0].get("baseline_fraction").is_none());
    assert!(at(&value, &["no_claim"]).as_str().unwrap().contains("binary64"));

    // The audit-only old layer cannot clamp or double-correct historical predictions.
    let replaced = text
        .replace("additive_fraction = \"0.02\"", "additive_fraction = \"0.5\"")
        .replace("upper_cap_fraction = \"0.79\"", "upper_cap_fraction = \"0.01\"");
    let (status, changed, _) = run(&scratch(&replaced));
    assert_eq!(status, 0, "{changed:?}");
    assert_eq!(at(&changed, &["payload", "result"]), at(payload, &["result"]));
    assert_eq!(
        at(&changed, &["payload", "correction_fit_result", "mean_residual_fraction"]),
        at(fitted, &["mean_residual_fraction"])
    );
    assert_ne!(at(&changed, &["payload", "receipt_digest"]), at(payload, &["receipt_digest"]));
}

#[test]
fn historical_input_edits_recalculate_the_fit_and_source_only_edits_change_receipts() {
    let text = evaluated_fit();
    let (status, original, _) = run(&scratch(&text));
    assert_eq!(status, 0, "{original:?}");
    let changed = text.replace("value = \"2.000\"", "value = \"1.000\"");
    let (status, changed, _) = run(&scratch(&changed));
    assert_eq!(status, 0, "{changed:?}");
    // Both baselines are now 1/16, so residuals 1/32 and 9/32 average to 5/32.
    assert_eq!(
        exact(at(&changed, &["payload", "correction_fit_result", "mean_residual_fraction"])),
        (5, 32)
    );
    close(at(&changed, &["payload", "result", "recovery_fraction"]), 35.0 / 64.0);
    let (status, retokenized, _) = run(&scratch(&text.replace("value = \"2.000\"", "value = \"2/1\"")));
    assert_eq!(status, 0, "{retokenized:?}");
    assert_eq!(at(&retokenized, &["payload", "result"]), at(&original, &["payload", "result"]));
    assert_ne!(
        at(&retokenized, &["payload", "correction_fit_result", "receipt_digest"]),
        at(&original, &["payload", "correction_fit_result", "receipt_digest"])
    );
}

#[test]
fn computed_baselines_refuse_missing_mismatched_or_retrospective_history_atomically() {
    let text = evaluated_fit();
    for (before, after, code) in [
        ("value = \"2.000\"", "value = \"10.001\"", "FMINE-REF-203"),
        ("unit = \"g_per_dry_t\"\nvalue = \"2.000\"", "unit = \"ppm_dry_mass\"\nvalue = \"2.000\"", "FMINE-REF-107"),
        ("measurand = \"total-Au/dry\"\nunit = \"g_per_dry_t\"\nvalue = \"2.000\"", "measurand = \"soluble-Au/dry\"\nunit = \"g_per_dry_t\"\nvalue = \"2.000\"", "FMINE-REF-107"),
        ("evidence_cut_ns = \"9\"", "evidence_cut_ns = \"10\"", "FMINE-REF-203"),
        ("available_ns = \"6\"", "available_ns = \"10\"", "FMINE-REF-203"),
        ("uncensored = \"true\"", "uncensored = \"false\"", "FMINE-REF-202"),
        ("observed_fraction = \"0.34375\"", "observed_fraction = \"1.1\"", "FMINE-REF-109"),
    ] {
        assert!(text.contains(before));
        let value = refused(&text.replace(before, after), code);
        let payload = at(&value, &["payload"]);
        assert_eq!(at(payload, &["correction_fit_result"]), &Json::Null);
        assert_eq!(at(payload, &["correction_fit", "pairs"]).as_array().unwrap().len(), 2);
        assert!(at(payload, &["correction_fit", "pairs"])
            .as_array().unwrap().iter()
            .all(|pair| at(pair, &["inputs", "claim"]).as_object().is_some()));
    }
    let missing = text.rsplit_once("[[correction_input]]").unwrap().0;
    let value = refused(missing, "FMINE-REF-201");
    assert_eq!(at(&value, &["payload", "correction_fit_result"]), &Json::Null);
    for malformed in [
        text.replace("pair = \"second\"", "pair = \"unknown\""),
        text.replace("name = \"Au\"\nmeasurand = \"total-Au/dry\"\nunit = \"g_per_dry_t\"\nvalue = \"2.000\"", "name = \"S\"\nmeasurand = \"total-Au/dry\"\nunit = \"g_per_dry_t\"\nvalue = \"2.000\""),
        text.replace("id = \"first\"", "id = \"first\"\nbaseline_fraction = \"0.1\""),
        format!("{WORKSHEET}{FIT}{HISTORICAL_INPUTS}"),
        format!("{WORKSHEET}{HISTORICAL_INPUTS}"),
    ] {
        refused(&malformed, "FMINE-REF-506");
    }
}
