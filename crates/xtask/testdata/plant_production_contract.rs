//! SYNTHETIC independent arithmetic and strict admission through the actual binary.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

use fmine_core::hash::sha256;
use fmine_io::omf::json::{Json, parse};

const GRINDING: &str = include_str!("../../../tests/plant/grinding_joint.toml");
const RECOVERY: &str = include_str!("../../../tests/plant/production_recovery.toml");
const PRODUCTION: &str = include_str!("../../../tests/plant/production_joint.toml");
static NEXT: AtomicUsize = AtomicUsize::new(0);

fn scratch() -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "fmine-joint-production-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&path).expect("test directory");
    for (name, text) in [
        ("grinding.toml", GRINDING),
        ("recovery.toml", RECOVERY),
        ("production.toml", PRODUCTION),
    ] {
        std::fs::write(path.join(name), text).expect("source worksheet");
    }
    path
}

fn arguments() -> Vec<String> {
    [
        "plant",
        "production",
        "--input",
        "production.toml",
        "--grinding",
        "grinding.toml",
        "--recovery",
        "recovery.toml",
    ]
    .map(str::to_owned)
    .to_vec()
}

fn run(root: &Path, extra: &[&str]) -> (i32, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_fmine"))
        .current_dir(root)
        .args(arguments())
        .args(extra)
        .output()
        .expect("run fmine");
    (
        output.status.code().expect("exit code"),
        String::from_utf8(output.stdout).expect("UTF-8"),
    )
}

fn field<'a>(value: &'a Json, path: &[&str]) -> &'a Json {
    path.iter()
        .fold(value, |value, key| value.get(key).expect(key))
}

fn close(value: &Json, expected: f64) {
    let actual = value.f64().expect("number");
    assert!((actual - expected).abs() < 1e-12, "{actual} != {expected}");
}

#[test]
fn executable_keeps_dependence_source_bytes_units_and_claim_ancestry() {
    let root = scratch();
    let (code, output) = run(&root, &[]);
    assert_eq!(code, 0, "{output}");
    assert_eq!(run(&root, &[]), (code, output.clone()));
    let json = parse(output.as_bytes()).expect("JSON envelope");
    let p = field(&json, &["payload", "production"]);
    for (quantity, expected) in [
        ("dry_tonnes", 200.0),
        ("contained_metal_kg", 0.32),
        ("recovered_metal_kg", 0.24),
    ] {
        close(field(p, &["summary", quantity, "mean"]), expected);
    }
    close(field(p, &["summary", "recovered_metal_kg", "p10"]), 0.192);
    close(field(p, &["summary", "recovered_metal_kg", "p50"]), 0.192);
    close(field(p, &["summary", "recovered_metal_kg", "p90"]), 0.288);
    assert_ne!(
        field(p, &["summary", "recovered_metal_kg", "mean"]).f64(),
        Some(0.375)
    );
    assert_eq!(
        field(p, &["submitted_worksheet"]).as_str(),
        Some(PRODUCTION)
    );
    assert_eq!(
        field(p, &["source", "sha256"]).as_str(),
        Some(sha256(PRODUCTION.as_bytes()).to_hex().as_str())
    );
    for (name, text) in [("grinding", GRINDING), ("recovery", RECOVERY)] {
        assert_eq!(
            field(&json, &["payload", "sources", name, "submitted_worksheet"]).as_str(),
            Some(text)
        );
        assert_eq!(
            field(&json, &["payload", "sources", name, "source", "sha256"]).as_str(),
            Some(sha256(text.as_bytes()).to_hex().as_str())
        );
    }
    let claim = field(p, &["admitted_claim"]);
    assert_eq!(field(claim, &["evidence"]).as_str(), Some("Assumed"));
    assert_eq!(
        field(claim, &["confidentiality"]).as_str(),
        Some("Internal")
    );
    assert_eq!(
        field(claim, &["calibration"]).as_str(),
        Some("uncalibrated")
    );
    assert_eq!(
        field(claim, &["parents"])
            .as_array()
            .expect("parents")
            .len(),
        3
    );
    assert_eq!(
        field(p, &["representation_quantity"]).as_str(),
        Some("recovered_metal_kg")
    );
    let (code, markdown) = run(&root, &["--format", "markdown"]);
    assert_eq!(code, 0);
    for text in [
        "Declared joint plant production",
        "0.24",
        "Source evidence",
        "uncalibrated",
    ] {
        assert!(markdown.contains(text), "{text}: {markdown}");
    }
}

#[test]
fn executable_refuses_mismatched_grade_grind_scope_and_probability_without_partial_output() {
    let cases = [
        (
            "production.toml",
            PRODUCTION.replace("grade_g_per_dry_t = \"4\"", "grade_g_per_dry_t = \"3\""),
        ),
        (
            "production.toml",
            PRODUCTION.replacen("weight = \"1/2\"", "weight = \"1/3\"", 1),
        ),
        (
            "production.toml",
            PRODUCTION.replace("end_ns = \"3600000000000\"", "end_ns = \"7200000000000\""),
        ),
        (
            "production.toml",
            PRODUCTION.replace("scheduled_hours = \"1\"", "scheduled_hours = \"2\""),
        ),
        (
            "recovery.toml",
            RECOVERY.replace("value = \"100\"", "value = \"101\""),
        ),
        (
            "grinding.toml",
            GRINDING.replace("id = \"hard\"", "id = \"other\""),
        ),
    ];
    for (file, text) in cases {
        let root = scratch();
        std::fs::write(root.join(file), text).expect("changed source");
        let (code, output) = run(&root, &[]);
        assert_ne!(code, 0, "{file}: {output}");
        assert!(output.contains("FMINE-REF-"), "{output}");
        assert!(
            !output.contains("fmine.plant-production-result.v1"),
            "no admitted partial result: {output}"
        );
    }
}

#[test]
fn every_input_source_is_live_and_unavailable_sources_refuse() {
    let root = scratch();
    let (code, before) = run(&root, &[]);
    assert_eq!(code, 0);
    let old = parse(before.as_bytes()).expect("before JSON");
    for (file, original) in [
        ("grinding.toml", GRINDING),
        ("recovery.toml", RECOVERY),
        ("production.toml", PRODUCTION),
    ] {
        std::fs::write(
            root.join(file),
            format!("{original}\n# distinct source bytes\n"),
        )
        .expect("changed bytes");
        let (code, changed) = run(&root, &[]);
        assert_eq!(code, 0, "{changed}");
        let json = parse(changed.as_bytes()).expect("changed JSON");
        close(
            field(
                &json,
                &[
                    "payload",
                    "production",
                    "summary",
                    "recovered_metal_kg",
                    "mean",
                ],
            ),
            0.24,
        );
        assert_ne!(
            field(&json, &["payload", "production", "receipt_digest"]).as_str(),
            field(&old, &["payload", "production", "receipt_digest"]).as_str()
        );
        std::fs::write(root.join(file), "").expect("blank source");
        assert_ne!(run(&root, &[]).0, 0, "{file}");
        std::fs::write(root.join(file), original).expect("restore synthetic source");
    }
    let mut args = arguments();
    args[5] = "absent-grinding.toml".into();
    let (output, code) = fmine_cli::run(&args, &root);
    assert_ne!(code, 0, "{output}");
    assert!(output.contains("FMINE-REF-506"), "{output}");
}

#[test]
fn parser_requires_all_three_unique_inputs_and_robot_docs_explain_the_model() {
    let root = scratch();
    let mut duplicate = arguments();
    duplicate[4] = "--input".into();
    assert_eq!(fmine_cli::run(&duplicate, &root).1, 2);
    assert_eq!(fmine_cli::run(&arguments()[..6], &root).1, 2);
    let (text, code) = fmine_cli::run(&["robot-docs".into()], &root);
    assert_eq!(code, 0);
    let json = parse(text.as_bytes()).expect("robot documentation");
    assert_eq!(
        field(&json, &["payload", "plant_production", "schema"]).as_str(),
        Some("fmine.plant-production.v1")
    );
    assert!(text.contains("forecast journal register-plant"));
}
