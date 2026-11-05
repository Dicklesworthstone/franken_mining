// FACT: constructed exact conversion answers exercised through the actual CLI (BD-177/336).
use fmine_io::omf::json::{self, Json};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

const MATERIAL: &str = include_str!("../../../tests/plant/material_projection.toml");
const DRY_MASS: &str = "[[mass]]\nrole = \"dry\"\nvalue = \"2.5\"\nunit = \"t\"";
const WATER_MASS: &str = "[[mass]]\nrole = \"water\"\nvalue = \"125\"\nunit = \"kg\"";
static SERIAL: AtomicUsize = AtomicUsize::new(0);

fn scratch(text: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "fmine-plant-projection-{}-{}.toml",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::write(&path, text).expect("write constructed material");
    path
}

fn run(path: &Path) -> (i32, Json, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_fmine"))
        .args(["plant", "project", "--input"])
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

fn metal<'a>(value: &'a Json, element: &str) -> &'a Json {
    at(value, &["payload", "result", "elements_kg"])
        .as_array()
        .unwrap()
        .iter()
        .find(|v| v.get("element").and_then(Json::as_str) == Some(element))
        .unwrap()
}

fn wet_material() -> String {
    MATERIAL
        .replace(
            DRY_MASS,
            "[[mass]]\nrole = \"wet\"\nvalue = \"2.625\"\nunit = \"t\"",
        )
        .replace(WATER_MASS, "[[moisture]]\nwater_over_wet_mass = \"1/21\"")
}

#[test]
fn exact_dry_material_projection_keeps_original_units_tokens_and_source_claims() {
    let path = scratch(MATERIAL);
    let (status, value, first) = run(&path);
    assert_eq!(status, 0, "{value:?}");
    assert_eq!(at(&value, &["verb"]).as_str(), Some("plant project"));
    assert_eq!(
        at(&value, &["payload", "schema"]).as_str(),
        Some("fmine.plant-material-result.v1")
    );
    assert_eq!(
        exact(at(&value, &["payload", "result", "dry_matter_kg"])),
        (2500, 1)
    );
    assert_eq!(
        exact(at(&value, &["payload", "result", "water_kg"])),
        (125, 1)
    );
    assert_eq!(exact(at(metal(&value, "Au"), &["kg"])), (1, 100));
    assert_eq!(exact(at(metal(&value, "Cu"), &["kg"])), (50, 1));
    let rows = at(&value, &["payload", "observations"]).as_array().unwrap();
    assert_eq!(rows.len(), 4);
    let gold = rows
        .iter()
        .find(|v| v.get("element").and_then(Json::as_str) == Some("Au"))
        .unwrap();
    assert_eq!(at(gold, &["raw_value"]).as_str(), Some("4.000"));
    assert_eq!(at(gold, &["unit"]).as_str(), Some("g_per_t"));
    assert_eq!(exact(at(gold, &["value"])), (4, 1));
    assert_eq!(
        at(&value, &["payload", "admitted_claim", "evidence"]).as_str(),
        Some("Assumed")
    );
    assert_eq!(
        at(&value, &["payload", "admitted_claim", "confidentiality"]).as_str(),
        Some("Internal")
    );
    assert_eq!(
        at(&value, &["payload", "admitted_claim", "calibration"]).as_str(),
        Some("uncalibrated")
    );
    assert_eq!(
        at(&value, &["payload", "admitted_claim", "parents"])
            .as_array()
            .unwrap()
            .len(),
        rows.len()
    );
    let source_digest = fmine_core::hash::sha256(MATERIAL.as_bytes()).to_hex();
    assert_eq!(
        at(&value, &["payload", "source", "sha256"]).as_str(),
        Some(source_digest.as_str())
    );
    for row in rows {
        let sources = at(row, &["claim", "sources"]).as_array().unwrap();
        assert_eq!(sources.len(), 1);
        assert_eq!(
            at(&sources[0], &["sha256"]).as_str(),
            Some(source_digest.as_str())
        );
        assert!(
            at(&sources[0], &["locator"])
                .as_str()
                .unwrap()
                .contains("line")
        );
    }
    assert_eq!(run(&path).2, first);
}

#[test]
fn wet_route_and_explicit_unit_equivalents_match_the_same_dry_material() {
    let (_, dry, _) = run(&scratch(MATERIAL));
    let (status, wet, _) = run(&scratch(&wet_material()));
    assert_eq!(status, 0, "{wet:?}");
    assert_eq!(
        at(&dry, &["payload", "result"]),
        at(&wet, &["payload", "result"])
    );
    assert_ne!(
        at(&dry, &["payload", "receipt_digest"]),
        at(&wet, &["payload", "receipt_digest"])
    );
    let alternate = MATERIAL
        .replace(
            DRY_MASS,
            "[[mass]]\nrole = \"dry\"\nvalue = \"2500\"\nunit = \"kg\"",
        )
        .replace(
            "value = \"4.000\"\nunit = \"g_per_t\"",
            "value = \"0.000004\"\nunit = \"mass_fraction\"",
        )
        .replace(
            "value = \"2\"\nunit = \"mass_percent\"",
            "value = \"20000\"\nunit = \"g_per_t\"",
        );
    let (status, alternate, _) = run(&scratch(&alternate));
    assert_eq!(status, 0, "{alternate:?}");
    assert_eq!(
        at(&dry, &["payload", "result"]),
        at(&alternate, &["payload", "result"])
    );
}

#[test]
fn bad_moisture_is_refused_with_original_inputs_and_no_projection() {
    for moisture in ["-0.1", "1", "8"] {
        let input = wet_material().replace(
            "water_over_wet_mass = \"1/21\"",
            &format!("water_over_wet_mass = \"{moisture}\""),
        );
        let (status, value, _) = run(&scratch(&input));
        assert_ne!(status, 0);
        assert_eq!(
            at(&value, &["refusal", "code"]).as_str(),
            Some("FMINE-REF-109")
        );
        assert_eq!(at(&value, &["payload", "admitted_claim"]), &Json::Null);
        assert_eq!(at(&value, &["payload", "receipt_digest"]), &Json::Null);
        assert_eq!(at(&value, &["payload", "result"]), &Json::Null);
        assert_eq!(
            at(&value, &["payload", "observations"])
                .as_array()
                .unwrap()
                .len(),
            4
        );
    }
}

#[test]
fn undeclared_bases_units_and_inferred_precision_are_never_admitted() {
    for (text, code) in [
        (
            MATERIAL.replace(
                "total_element/all_non_water_dry_matter",
                "acid_soluble/dry_solids",
            ),
            "FMINE-REF-105",
        ),
        (
            MATERIAL.replace(
                "whole_material_support = \"true\"",
                "whole_material_support = \"false\"",
            ),
            "FMINE-REF-201",
        ),
        (
            MATERIAL.replace("unit = \"t\"", "unit = \"short_tons\""),
            "FMINE-REF-107",
        ),
        (
            MATERIAL.replace("unit = \"g_per_t\"", "unit = \"mg_per_litre\""),
            "FMINE-REF-107",
        ),
        (
            MATERIAL.replace(
                "value = \"4.000\"",
                "value = \"4.000\"\nvariance = \"0.25\"",
            ),
            "FMINE-REF-506",
        ),
        (
            MATERIAL.replace("value = \"4.000\"", "value = 4.000"),
            "FMINE-REF-506",
        ),
        (
            MATERIAL.replace("value = \"4.000\"", "value = \"1e-2147483648\""),
            "FMINE-REF-506",
        ),
        (
            MATERIAL.replace("value = \"4.000\"", "value = \"1/0\""),
            "FMINE-REF-506",
        ),
    ] {
        let (status, value, _) = run(&scratch(&text));
        assert_ne!(status, 0, "{text}");
        assert_eq!(
            at(&value, &["refusal", "code"]).as_str(),
            Some(code),
            "{value:?}"
        );
    }
    for text in [
        MATERIAL.replace(WATER_MASS, ""),
        MATERIAL.replace("element = \"Cu\"", "element = \"Au\""),
        MATERIAL.replace("element = \"Au\"", "element = \"O\""),
        MATERIAL.replace("value = \"2\"", "value = \"101\""),
    ] {
        let (status, value, _) = run(&scratch(&text));
        assert_ne!(status, 0, "{text}");
        assert_eq!(at(&value, &["payload", "admitted_claim"]), &Json::Null);
    }
}

#[test]
fn source_and_numeric_mutations_change_the_receipt_without_losing_the_raw_reading() {
    let path = scratch(MATERIAL);
    let (_, initial, _) = run(&path);
    std::fs::write(
        &path,
        format!("{MATERIAL}\n# extra submitted source bytes\n"),
    )
    .unwrap();
    let (status, annotated, _) = run(&path);
    assert_eq!(status, 0);
    assert_eq!(
        at(&initial, &["payload", "result"]),
        at(&annotated, &["payload", "result"])
    );
    assert_ne!(
        at(&initial, &["payload", "source", "sha256"]),
        at(&annotated, &["payload", "source", "sha256"])
    );
    assert_ne!(
        at(&initial, &["payload", "receipt_digest"]),
        at(&annotated, &["payload", "receipt_digest"])
    );
    std::fs::write(
        &path,
        MATERIAL.replace("value = \"4.000\"", "value = \"8.000\""),
    )
    .unwrap();
    let (status, changed, _) = run(&path);
    assert_eq!(status, 0);
    assert_eq!(exact(at(metal(&changed, "Au"), &["kg"])), (1, 50));
    assert_eq!(exact(at(metal(&changed, "Cu"), &["kg"])), (50, 1));
    assert_ne!(
        at(&initial, &["payload", "receipt_digest"]),
        at(&changed, &["payload", "receipt_digest"])
    );
}

#[test]
fn material_projection_is_available_in_robot_docs_and_refuses_bad_command_shapes() {
    let output = Command::new(env!("CARGO_BIN_EXE_fmine"))
        .arg("robot-docs")
        .output()
        .unwrap();
    assert!(output.status.success());
    let value = json::parse(&output.stdout).unwrap();
    assert_eq!(
        at(&value, &["payload", "plant_material", "schema"]).as_str(),
        Some("fmine.plant-material.v1")
    );
    for args in [
        vec!["plant", "project"],
        vec!["plant", "project", "--input", "a", "--input", "b"],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_fmine"))
            .args(args)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(2));
        json::parse(&output.stdout).unwrap();
    }
    assert_ne!(
        run(Path::new("/missing/fmine-material-worksheet.toml")).0,
        0
    );
}
