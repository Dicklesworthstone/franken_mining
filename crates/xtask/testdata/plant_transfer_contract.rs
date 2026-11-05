// FACT: independently constructed lot totals and Jacobian covariance, through the real CLI.
use fmine_io::omf::json::{self, Json};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

const RAW: &str = include_str!("../../../tests/plant/raw_transfer.toml");
const ORDER: [&str; 6] = [
    "lot-1/wet_mass",
    "lot-1/moisture",
    "lot-1/Cu",
    "lot-2/wet_mass",
    "lot-2/moisture",
    "lot-2/Cu",
];
// Independent hand calculation: J diag(sigma) R diag(sigma) J^T, [dry, water, Cu].
const CORRELATED: [f64; 9] = [
    7.17, -4.62, 0.1217, -4.62, 5.07, -0.0842, 0.1217, -0.0842, 0.035881,
];
const INDEPENDENT: [f64; 9] = [
    6.45, -4.75, 0.1109, -4.75, 5.05, -0.0859, 0.1109, -0.0859, 0.035737,
];
static SERIAL: AtomicUsize = AtomicUsize::new(0);

fn scratch(text: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "fmine-plant-transfer-{}-{}.toml",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::write(&path, text).expect("write constructed raw lots");
    path
}

fn run(path: &Path) -> (i32, Json, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_fmine"))
        .args(["plant", "transfer", "--input"])
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
    let value = json::parse(text.as_bytes()).expect("valid finite JSON");
    let status = output.status.code().expect("normal exit");
    assert_eq!(at(&value, &["exit_code"]).i64(), Some(i64::from(status)));
    for (_, flag) in at(&value, &["flags"]).as_object().unwrap() {
        assert_eq!(flag.as_bool(), Some(false));
    }
    (status, value, text)
}

fn at<'a>(value: &'a Json, path: &[&str]) -> &'a Json {
    path.iter().fold(value, |v, key| {
        v.get(key).unwrap_or_else(|| panic!("missing {key}: {v:?}"))
    })
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

fn observation<'a>(value: &'a Json, key: &str) -> &'a Json {
    at(value, &["payload", "observations"])
        .as_array()
        .unwrap()
        .iter()
        .find(|v| v.get("key").and_then(Json::as_str) == Some(key))
        .unwrap()
}

fn covariance(value: &Json, expected: &[f64; 9]) {
    let entries = at(value, &["payload", "result", "linearized_covariance_kg2"])
        .as_array()
        .unwrap();
    assert_eq!(entries.len(), expected.len());
    for (i, (entry, expected)) in entries.iter().zip(expected).enumerate() {
        let actual = entry.f64().unwrap();
        assert!(
            (actual - expected).abs() < 1e-11 * (1.0 + expected.abs()),
            "covariance[{i}]: {actual} != {expected}"
        );
    }
}

fn replace(old: &str, new: &str) -> String {
    assert!(RAW.contains(old), "fixture lacks {old}");
    RAW.replacen(old, new, 1)
}

fn refused(text: &str, code: &str) -> Json {
    let (status, value, _) = run(&scratch(text));
    assert_ne!(status, 0, "accepted {text}");
    assert_eq!(
        at(&value, &["refusal", "code"]).as_str(),
        Some(code),
        "{value:?}"
    );
    for field in [
        "admitted_claim",
        "nominal_transfer",
        "receipt_digest",
        "result",
    ] {
        assert_eq!(at(&value, &["payload", field]), &Json::Null);
    }
    value
}

#[test]
fn two_lots_match_exact_totals_and_independent_full_covariance() {
    let (status, value, _) = run(&scratch(RAW));
    assert_eq!(status, 0, "{value:?}");
    assert_eq!(at(&value, &["verb"]).as_str(), Some("plant transfer"));
    let result = at(&value, &["payload", "result"]);
    assert_eq!(exact(at(result, &["dry_matter_kg"])), (250, 1));
    assert_eq!(exact(at(result, &["water_kg"])), (50, 1));
    let metal = &at(result, &["elements_kg"]).as_array().unwrap()[0];
    assert_eq!(at(metal, &["element"]).as_str(), Some("Cu"));
    assert_eq!(
        at(metal, &["measurand"]).as_str(),
        Some("total-Cu/all-phases")
    );
    assert_eq!(exact(at(metal, &["kg"])), (41, 10));
    let grade = &at(result, &["dry_grade_mass_fractions"])
        .as_array()
        .unwrap()[0];
    assert_eq!(exact(at(grade, &["mass_fraction"])), (41, 2500));
    assert_ne!(
        exact(at(grade, &["mass_fraction"])),
        (3, 200),
        "not the arithmetic mean grade"
    );
    let order: Vec<_> = at(result, &["covariance_order"])
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert_eq!(order, ["dry", "water", "Cu"]);
    assert_eq!(
        at(result, &["covariance_layout"]).as_str(),
        Some("row_major")
    );
    assert_eq!(
        at(result, &["covariance_approximation"]).as_str(),
        Some("first_order")
    );
    assert_eq!(at(result, &["transformed_distribution"]), &Json::Null);
    covariance(&value, &CORRELATED);
}

#[test]
fn cross_lot_correlation_changes_all_covariance_channels_without_changing_nominal_totals() {
    let (_, correlated, _) = run(&scratch(RAW));
    let text = replace("value = \"0.5000\"", "value = \"0\"");
    let (status, independent, _) = run(&scratch(&text));
    assert_eq!(status, 0, "{independent:?}");
    covariance(&independent, &INDEPENDENT);
    for field in [
        "dry_matter_kg",
        "water_kg",
        "elements_kg",
        "dry_grade_mass_fractions",
    ] {
        assert_eq!(
            at(&correlated, &["payload", "result", field]),
            at(&independent, &["payload", "result", field])
        );
    }
    assert_ne!(
        at(&correlated, &["payload", "receipt_digest"]),
        at(&independent, &["payload", "receipt_digest"])
    );
}

#[test]
fn original_tokens_source_identity_and_parent_claims_bind_the_complete_projection() {
    let (status, value, _) = run(&scratch(RAW));
    assert_eq!(status, 0, "{value:?}");
    let digest = fmine_core::hash::sha256(RAW.as_bytes()).to_hex();
    assert_eq!(
        at(&value, &["payload", "source", "sha256"]).as_str(),
        Some(digest.as_str())
    );
    assert_eq!(
        at(&value, &["payload", "source", "bytes"]).i64(),
        Some(RAW.len() as i64)
    );
    let first = observation(&value, ORDER[0]);
    assert_eq!(at(first, &["raw_value"]).as_str(), Some("100.000"));
    assert_eq!(
        at(first, &["raw_standard_uncertainty"]).as_str(),
        Some("1.00")
    );
    assert_eq!(at(first, &["raw_start_ns"]).as_str(), Some("0"));
    assert_eq!(at(first, &["raw_end_ns"]).as_str(), Some("10"));
    assert_eq!(at(first, &["support"]).as_str(), Some("PlantLot"));
    let model = at(&value, &["payload", "error_model"]);
    assert_eq!(at(model, &["canonical_binding"]).as_bool(), Some(true));
    assert_eq!(
        at(model, &["first_order_approximation_accepted"]).as_bool(),
        Some(true)
    );
    let coordinates = at(model, &["coordinate_order"]).as_array().unwrap();
    let correlation = at(model, &["correlation"]).as_array().unwrap();
    assert_eq!(correlation.len(), 36);
    assert_eq!(correlation[3].f64(), Some(0.5));
    assert_eq!(correlation[18].f64(), Some(0.5));
    let declarations = at(model, &["declared_correlations"]).as_array().unwrap();
    assert_eq!(declarations.len(), 21);
    assert_eq!(
        at(&declarations[3], &["raw_value"]).as_str(),
        Some("0.5000")
    );
    let mut parents = Vec::new();
    for (i, key) in ORDER.iter().enumerate() {
        let row = observation(&value, key);
        assert_eq!(at(&coordinates[i], &["key"]).as_str(), Some(*key));
        assert_eq!(
            at(&coordinates[i], &["source_claim_id"]),
            at(row, &["claim", "id"])
        );
        parents.push(at(row, &["claim", "id"]));
    }
    parents.push(at(&value, &["payload", "declaration", "claim", "id"]));
    parents.push(at(model, &["claim", "id"]));
    let output = at(&value, &["payload", "admitted_claim"]);
    let output_parents = at(output, &["parents"]).as_array().unwrap();
    assert_eq!(output_parents.len(), parents.len());
    assert!(parents.iter().all(|p| output_parents.contains(p)));
    assert_eq!(at(output, &["state"]).as_str(), Some("Estimated"));
    for claim in std::iter::once(at(&value, &["payload", "declaration", "claim"]))
        .chain(
            ORDER
                .iter()
                .map(|key| at(observation(&value, key), &["claim"])),
        )
        .chain(std::iter::once(at(model, &["claim"])))
    {
        assert_eq!(at(claim, &["evidence"]).as_str(), Some("Assumed"));
        assert_eq!(at(claim, &["confidentiality"]).as_str(), Some("Internal"));
        assert_eq!(at(claim, &["calibration"]).as_str(), Some("uncalibrated"));
        let anchors = at(claim, &["sources"]).as_array().unwrap();
        assert_eq!(anchors.len(), 1);
        assert_eq!(at(&anchors[0], &["sha256"]).as_str(), Some(digest.as_str()));
        assert!(
            at(&anchors[0], &["locator"])
                .as_str()
                .unwrap()
                .contains("line")
        );
    }
    let transfer = at(&value, &["payload", "nominal_transfer"]);
    assert_eq!(
        at(transfer, &["error_model_adoption"]).as_str(),
        Some("requires_separate_declaration")
    );
    assert_eq!(
        at(transfer, &["claim", "parents"]).as_array().unwrap(),
        &[at(output, &["id"]).clone()]
    );
    assert_eq!(
        at(transfer, &["claim", "evidence"]).as_str(),
        Some("Assumed")
    );
    assert_eq!(
        at(transfer, &["claim", "confidentiality"]).as_str(),
        Some("Internal")
    );
}

#[test]
fn source_bytes_and_raw_values_change_repeatable_receipts() {
    let path = scratch(RAW);
    let (status, initial, first) = run(&path);
    assert_eq!(status, 0);
    assert_eq!(run(&path).2, first);
    std::fs::write(
        &path,
        format!("{RAW}\n# Additional submitted source bytes.\n"),
    )
    .unwrap();
    let (status, annotated, _) = run(&path);
    assert_eq!(status, 0);
    assert_eq!(
        at(&initial, &["payload", "result"]),
        at(&annotated, &["payload", "result"])
    );
    for field in ["receipt_digest", "admitted_claim"] {
        assert_ne!(
            at(&initial, &["payload", field]),
            at(&annotated, &["payload", field])
        );
    }
    assert_ne!(
        at(&initial, &["payload", "source", "sha256"]),
        at(&annotated, &["payload", "source", "sha256"])
    );
    std::fs::write(&path, replace("value = \"100.000\"", "value = \"110.000\"")).unwrap();
    let (status, changed, _) = run(&path);
    assert_eq!(status, 0);
    assert_eq!(
        exact(at(&changed, &["payload", "result", "dry_matter_kg"])),
        (259, 1)
    );
    assert_eq!(
        exact(at(&changed, &["payload", "result", "water_kg"])),
        (51, 1)
    );
    let metal = &at(&changed, &["payload", "result", "elements_kg"])
        .as_array()
        .unwrap()[0];
    assert_eq!(exact(at(metal, &["kg"])), (419, 100));
    assert_eq!(
        at(observation(&changed, ORDER[0]), &["raw_value"]).as_str(),
        Some("110.000")
    );
    assert_ne!(
        at(&initial, &["payload", "receipt_digest"]),
        at(&changed, &["payload", "receipt_digest"])
    );
}

#[test]
fn row_permutations_keep_kernel_coordinate_order_and_covariance() {
    let mut blocks = RAW.split("\n[[");
    let top = blocks.next().unwrap();
    let rows: Vec<_> = blocks.collect();
    let reversed = format!(
        "{top}{}",
        rows.iter()
            .rev()
            .map(|row| format!("\n[[{row}"))
            .collect::<String>()
    );
    let (_, original, _) = run(&scratch(RAW));
    let (status, permuted, _) = run(&scratch(&reversed));
    assert_eq!(status, 0, "{permuted:?}");
    assert_eq!(
        at(&original, &["payload", "result"]),
        at(&permuted, &["payload", "result"])
    );
    covariance(&permuted, &CORRELATED);
    let order: Vec<_> = at(&permuted, &["payload", "error_model", "coordinate_order"])
        .as_array()
        .unwrap()
        .iter()
        .map(|v| at(v, &["key"]).as_str().unwrap())
        .collect();
    assert_eq!(order, ORDER);
    assert_eq!(
        at(&original, &["payload", "error_model", "correlation"]),
        at(&permuted, &["payload", "error_model", "correlation"])
    );
    assert_ne!(
        at(&original, &["payload", "receipt_digest"]),
        at(&permuted, &["payload", "receipt_digest"])
    );
}

#[test]
fn physical_domains_support_and_basis_refuse_without_clamping() {
    for (old, new) in [
        ("value = \"100.000\"", "value = \"-100.000\""),
        ("value = \"100.000\"", "value = \"0\""),
        ("value = \"0.10\"", "value = \"1\""),
        ("value = \"0.10\"", "value = \"-0.10\""),
        ("value = \"0.0100\"", "value = \"1.1\""),
        ("value = \"0.0100\"", "value = \"-0.0100\""),
    ] {
        let value = refused(&replace(old, new), "FMINE-REF-109");
        let rows = at(&value, &["payload", "observations"]).as_array().unwrap();
        assert_eq!(rows.len(), 6);
        let raw = new
            .strip_prefix("value = \"")
            .unwrap()
            .strip_suffix('"')
            .unwrap();
        assert!(
            rows.iter()
                .any(|row| at(row, &["raw_value"]).as_str() == Some(raw))
        );
        assert_eq!(
            at(&value, &["payload", "error_model", "canonical_binding"]).as_bool(),
            Some(false)
        );
    }
    for field in [
        "complete",
        "nonoverlapping_lots",
        "moisture_is_h2o",
        "whole_lot_support",
        "first_order_approximation_accepted",
    ] {
        refused(
            &replace(
                &format!("{field} = \"true\""),
                &format!("{field} = \"false\""),
            ),
            "FMINE-REF-201",
        );
    }
    for (old, new, code) in [
        ("unit = \"kg\"", "unit = \"t\"", "FMINE-REF-107"),
        (
            "uncertainty_unit = \"kg\"",
            "uncertainty_unit = \"mass_fraction\"",
            "FMINE-REF-107",
        ),
        (
            "unit = \"mass_fraction\"",
            "unit = \"percent\"",
            "FMINE-REF-107",
        ),
        (
            "moisture_basis = \"water_over_wet_mass\"",
            "moisture_basis = \"water_over_dry_mass\"",
            "FMINE-REF-105",
        ),
        (
            "assay_basis = \"total_element/all_non_water_dry_matter\"",
            "assay_basis = \"acid_soluble\"",
            "FMINE-REF-105",
        ),
        (
            "unit = \"kg\"\nstart_ns = \"0\"",
            "unit = \"kg\"\nstart_ns = \"1\"",
            "FMINE-REF-102",
        ),
        ("end_ns = \"10\"", "end_ns = \"21\"", "FMINE-REF-102"),
    ] {
        refused(&replace(old, new), code);
    }
    refused(&RAW.replace("Cu", "O"), "FMINE-REF-107");
    // Removing a source AND its correlations must reach complete-population admission.
    let mut blocks = RAW.split("\n[[");
    let mut missing = blocks.next().unwrap().to_owned();
    for row in blocks {
        if (row.starts_with("measurement]]")
            && row.contains("lot = \"lot-1\"")
            && row.contains("channel = \"Cu\""))
            || (row.starts_with("correlation]]") && row.contains("lot-1/Cu"))
        {
            continue;
        }
        missing.push_str("\n[[");
        missing.push_str(row);
    }
    refused(&missing, "FMINE-REF-105");
}

#[test]
fn every_uncertainty_and_correlation_pair_is_explicit_and_numerically_admitted() {
    for (old, new, code) in [
        ("standard_uncertainty = \"1.00\"\n", "", "FMINE-REF-506"),
        (
            "standard_uncertainty = \"1.00\"",
            "standard_uncertainty = \"NaN\"",
            "FMINE-REF-506",
        ),
        (
            "standard_uncertainty = \"1.00\"",
            "standard_uncertainty = \"1e-9999\"",
            "FMINE-REF-207",
        ),
        (
            "standard_uncertainty = \"1.00\"",
            "standard_uncertainty = \"-1\"",
            "FMINE-REF-109",
        ),
        ("value = \"0.5000\"", "value = \"inf\"", "FMINE-REF-506"),
        ("value = \"0.5000\"", "value = \"1e-9999\"", "FMINE-REF-207"),
        ("value = \"0.5000\"", "value = \"2\"", "FMINE-REF-109"),
        ("value = \"0.5000\"", "value = \"1\"", "FMINE-REF-207"),
        ("value = \"1\"", "value = \"0\"", "FMINE-REF-109"),
        (
            "left = \"lot-1/wet_mass\"",
            "left = \"unknown/wet_mass\"",
            "FMINE-REF-506",
        ),
    ] {
        refused(&replace(old, new), code);
    }
    let pair =
        "\n[[correlation]]\nleft = \"lot-1/wet_mass\"\nright = \"lot-2/Cu\"\nvalue = \"0\"\n";
    refused(&replace(pair, ""), "FMINE-REF-506");
    refused(
        &format!(
            "{RAW}\n[[correlation]]\nleft = \"lot-2/wet_mass\"\nright = \"lot-1/wet_mass\"\nvalue = \"0.5\"\n"
        ),
        "FMINE-REF-506",
    );
    let negative = refused(
        &replace(
            "standard_uncertainty = \"1.00\"",
            "standard_uncertainty = \"-1\"",
        ),
        "FMINE-REF-109",
    );
    assert_eq!(
        at(
            observation(&negative, ORDER[0]),
            &["raw_standard_uncertainty"]
        )
        .as_str(),
        Some("-1")
    );
    assert_eq!(
        at(&negative, &["payload", "error_model", "canonical_binding"]).as_bool(),
        Some(true)
    );
}

#[test]
fn explicit_zero_uncertainty_remains_zero_without_variance_floors_or_distribution_claims() {
    let text = RAW
        .replace(
            "standard_uncertainty = \"1.00\"",
            "standard_uncertainty = \"0e-9999\"",
        )
        .replace(
            "standard_uncertainty = \"0.010\"",
            "standard_uncertainty = \"0\"",
        )
        .replace(
            "standard_uncertainty = \"0.0010\"",
            "standard_uncertainty = \"0\"",
        );
    let (status, value, _) = run(&scratch(&text));
    assert_eq!(status, 0, "{value:?}");
    covariance(&value, &[0.0; 9]);
    assert_eq!(
        at(observation(&value, ORDER[0]), &["raw_standard_uncertainty"]).as_str(),
        Some("0e-9999")
    );
    assert_eq!(
        at(&value, &["payload", "result", "transformed_distribution"]),
        &Json::Null
    );
    assert_eq!(
        at(&value, &["payload", "admitted_claim", "evidence"]).as_str(),
        Some("Assumed")
    );
    assert_eq!(
        at(&value, &["payload", "admitted_claim", "state"]).as_str(),
        Some("Estimated")
    );
}

#[test]
fn strict_bounded_input_and_command_surface_are_documented_and_refuse_ambiguity() {
    let output = Command::new(env!("CARGO_BIN_EXE_fmine"))
        .arg("robot-docs")
        .output()
        .unwrap();
    assert!(output.status.success());
    let value = json::parse(&output.stdout).unwrap();
    assert_eq!(
        at(&value, &["payload", "plant_transfer", "schema"]).as_str(),
        Some("fmine.plant-transfer.v1")
    );
    for args in [
        vec!["plant", "transfer"],
        vec!["plant", "transfer", "--input", "a", "--input", "b"],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_fmine"))
            .args(args)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(2));
        json::parse(&output.stdout).unwrap();
    }
    for (old, new) in [
        (
            "schema = \"fmine.plant-transfer.v1\"",
            "schema = \"fmine.plant-transfer.v2\"",
        ),
        ("complete = \"true\"", "complete = true"),
        (
            "complete = \"true\"",
            "complete = \"true\"\noptional_guess = \"yes\"",
        ),
        ("[[lot]]", "[[unknown]]"),
        (
            "standard_uncertainty = \"1.00\"",
            "standard_uncertainty = \"1.00\"\nextra = \"ignored\"",
        ),
        ("value = \"100.000\"", "value = \"1/0\""),
        ("value = \"100.000\"", "value = \"1e1000000\""),
        (
            "right = \"lot-1/wet_mass\"",
            "right = \"lot-1/wet_mass\"\nextra = \"ignored\"",
        ),
    ] {
        refused(&replace(old, new), "FMINE-REF-506");
    }
    let mut too_many = RAW.to_owned();
    for n in 3..=43 {
        too_many.push_str(&format!("\n[[lot]]\nid = \"lot-{n}\"\n"));
    }
    refused(&too_many, "FMINE-REF-506");
    refused(&" ".repeat(1024 * 1024 + 1), "FMINE-REF-506");
    let (status, absent, _) = run(Path::new("/missing/fmine-raw-transfer-worksheet.toml"));
    assert_ne!(status, 0);
    assert_eq!(
        at(&absent, &["refusal", "code"]).as_str(),
        Some("FMINE-REF-506")
    );
    assert_eq!(at(&absent, &["payload", "result"]), &Json::Null);
}
