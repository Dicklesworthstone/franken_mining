//! SYNTHETIC source-to-CRM scoring controls (BD-167). No lab or certificate authentication is asserted.
//! Determinism: BitwisePortable

#![allow(clippy::expect_used, clippy::panic)]

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use fmine_cli::{Envelope, OutputFormat, invoke};
use fmine_core::hash::sha256;
use fmine_io::omf::json::{Json, parse};

const BATCH: &str = include_str!("../../../tests/drill/crm_batch.toml");
const INPUT: &str = "crm_batch.toml";
static NEXT: AtomicU64 = AtomicU64::new(0);

fn fixture(text: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "fmine-crm-cli-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&root).expect("fixture directory");
    std::fs::write(root.join(INPUT), text).expect("synthetic batch");
    std::fs::canonicalize(root).expect("canonical fixture root")
}

fn args() -> Vec<String> {
    ["drill", "qaqc", "--input", INPUT]
        .map(str::to_owned)
        .into()
}

fn execute(root: &Path) -> Envelope {
    let (envelope, format) = invoke(&args(), root);
    assert_eq!(format, OutputFormat::Json);
    envelope
}

fn successful(root: &Path) -> Json {
    let envelope = execute(root);
    assert_eq!(envelope.exit_code, 0, "{}", envelope.to_json());
    parse(envelope.to_json().as_bytes()).expect("valid JSON")
}

fn field<'a>(value: &'a Json, path: &[&str]) -> &'a Json {
    path.iter()
        .fold(value, |value, key| value.get(key).expect(key))
}

fn report(value: &Json) -> &Json {
    field(value, &["payload", "report"])
}
fn number(value: &Json, key: &str) -> f64 {
    field(value, &[key]).f64().expect("finite number")
}
fn near(actual: f64, expected: f64) {
    assert!((actual - expected).abs() < 1e-9, "{actual} != {expected}");
}

fn flags(value: &Json) {
    let flags = field(value, &["flags"]).as_object().expect("flags");
    assert_eq!(flags.len(), 4);
    for (_, flag) in flags {
        assert_eq!(flag.as_bool(), Some(false));
    }
    assert_eq!(field(value, &["anchor_id"]), &Json::Null);
    assert_eq!(field(value, &["certificate"]), &Json::Null);
}

#[test]
fn native_command_keeps_all_observations_sd_bases_and_bias_beside_canmet() {
    let root = fixture(BATCH);
    let envelope = execute(&root);
    assert_eq!(envelope.exit_code, 0, "{}", envelope.to_json());
    let native = || {
        Command::new(env!("CARGO_BIN_EXE_fmine"))
            .current_dir(&root)
            .args(args())
            .output()
            .expect("native fmine")
    };
    let first = native();
    assert!(
        first.status.success(),
        "{}",
        String::from_utf8_lossy(&first.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&first.stdout).trim_end(),
        envelope.to_json()
    );
    assert_eq!(
        first.stdout,
        native().stdout,
        "same source has deterministic output"
    );
    let result = successful(&root);
    flags(&result);
    assert_eq!(
        field(&result, &["payload", "claim", "evidence"]).as_str(),
        Some("Assumed")
    );
    assert_eq!(
        field(&result, &["payload", "claim", "confidentiality"]).as_str(),
        Some("Internal")
    );
    assert_eq!(
        field(&result, &["payload", "claim", "calibration"]).as_str(),
        Some("uncalibrated")
    );
    let r = report(&result);
    assert_eq!(field(r, &["analyte"]).as_str(), Some("Au"));
    assert_eq!(field(r, &["n"]).u64(), Some(10));
    near(number(r, "mean"), 104.05);
    near(number(r, "sample_sd").powi(2), 58.0 / 45.0);
    near(number(r, "standard_error").powi(2), 58.0 / 450.0);
    let bias = field(r, &["bias"]);
    near(number(bias, "signed_bias"), 4.05);
    near(number(bias, "magnitude"), 4.05);
    near(number(bias, "t_critical"), 2.262157162741);
    near(
        number(bias, "lower"),
        4.05 - 2.262157162741 * (58.0_f64 / 450.0).sqrt(),
    );
    near(
        number(bias, "upper"),
        4.05 + 2.262157162741 * (58.0_f64 / 450.0).sqrt(),
    );
    assert_eq!(field(bias, &["degrees_of_freedom"]).u64(), Some(9));
    let bases = field(r, &["single_value"]).as_array().expect("SD bases");
    assert_eq!(bases.len(), 3);
    let source_lines: Vec<usize> = BATCH
        .lines()
        .enumerate()
        .filter_map(|(i, line)| (line == "[[observation]]").then_some(i + 1))
        .collect();
    for (basis, (name, warnings, failures)) in bases.iter().zip([
        ("sigma_c", 10, 8),
        ("sigma_l", 3, 0),
        ("lab_running_sw", 4, 0),
    ]) {
        assert_eq!(field(basis, &["basis"]).as_str(), Some(name));
        assert_eq!(field(basis, &["warnings"]).u64(), Some(warnings));
        assert_eq!(field(basis, &["failures"]).u64(), Some(failures));
        assert_eq!(field(basis, &["n"]).u64(), Some(10));
        let observations = field(basis, &["observations"])
            .as_array()
            .expect("all observations");
        assert_eq!(observations.len(), 10);
        for (index, observation) in observations.iter().enumerate() {
            assert_eq!(
                field(observation, &["sample_id"]).as_str(),
                Some(format!("CRM-{:02}", index + 1).as_str())
            );
            assert_eq!(
                field(observation, &["source_line"]).u64(),
                Some(source_lines[index] as u64)
            );
        }
    }
    assert_eq!(
        field(r, &["warning_counts_include_failures"]).as_bool(),
        Some(true)
    );
    let canmet = field(r, &["canmet"]);
    near(number(canmet, "limit"), 2.0 * 6.65_f64.sqrt());
    assert_eq!(field(canmet, &["passed"]).as_bool(), Some(true));
    assert_eq!(
        field(r, &["certificate", "verification"]).as_str(),
        Some("not_performed")
    );
    assert_eq!(
        field(r, &["source", "sha256"]).as_str(),
        Some(sha256(BATCH.as_bytes()).to_string().as_str())
    );
    assert_eq!(
        field(r, &["source", "bytes"]).u64(),
        Some(BATCH.len() as u64)
    );
    assert_eq!(
        std::fs::read(root.join(INPUT)).expect("unchanged source"),
        BATCH.as_bytes()
    );
    assert_eq!(
        std::fs::read_dir(&root).expect("read-only output").count(),
        1
    );
}

#[test]
fn markdown_explains_basis_disagreement_and_source_changes_change_the_claim() {
    let root = fixture(BATCH);
    let first = successful(&root);
    let mut markdown_args = args();
    markdown_args.extend(["--format".to_owned(), "markdown".to_owned()]);
    let (envelope, format) = invoke(&markdown_args, &root);
    assert_eq!(format, OutputFormat::Markdown);
    assert_eq!(envelope.exit_code, 0, "{}", envelope.to_json());
    let markdown = envelope.rendered_markdown.expect("human report");
    for text in [
        "Warnings, including failures",
        "sigma_c",
        "sigma_l",
        "lab_running_sw",
        "**CANMET:** pass",
        "**Signed bias:**",
        "fixed certified mean",
        "CRM-01",
        "CRM-10",
        "Verification: **not performed**",
        "Omitted uncertainty:",
        "SHA-256:",
    ] {
        assert!(markdown.contains(text), "missing {text}");
    }
    let changed = format!("{BATCH}\n# Additional source declaration.\n");
    std::fs::write(root.join(INPUT), &changed).expect("source edit");
    let second = successful(&root);
    assert_ne!(
        field(&first, &["payload", "claim", "id"]),
        field(&second, &["payload", "claim", "id"])
    );
    assert_ne!(
        field(&first, &["payload", "receipt_sha256"]),
        field(&second, &["payload", "receipt_sha256"])
    );
    assert_eq!(
        field(report(&first), &["single_value"]),
        field(report(&second), &["single_value"])
    );
}

#[test]
fn missing_censored_duplicate_or_invalid_policy_inputs_refuse_without_dropping_rows() {
    let root = fixture(BATCH);
    let mut cases = vec![
        BATCH.replace("analyte = \"Au\"\n", ""),
        BATCH.replace("unit = \"ppm\"\n", ""),
        BATCH.replace("method = \"fire-assay-AAS\"", "method = \"\""),
        BATCH.replace(
            "evidence_lane = \"synthetic\"",
            "evidence_lane = \"public\"",
        ),
        BATCH.replace("certified_mean = \"100\"", "certified_mean = 100"),
        BATCH.replace("sigma_c = \"1\"", "sigma_c = \"-1\""),
        BATCH.replace("sigma_l = \"2.5\"", "sigma_l = \"NaN\""),
        BATCH.replace("lab_running_sw = \"2\"\n", ""),
        BATCH.replace("warning_k = \"2\"", "warning_k = \"0\""),
        BATCH.replace("failure_k = \"3\"", "failure_k = \"2\""),
        BATCH.replace("confidence = \"0.95\"", "confidence = \"1\""),
        BATCH.replace("sample_id = \"CRM-02\"", "sample_id = \"CRM-01\""),
        BATCH.replace("value = \"102.5\"", "value = \"<102.5\""),
        BATCH.replace("value = \"102.5\"", "value = \"-1\""),
        BATCH.replace("value = \"102.5\"", "value = \"1e-400\""),
        BATCH.replace("sigma_c = \"1\"", "sigma_c = \"1e-400\""),
        BATCH.replace("[[observation]]", "[[unrecognized]]"),
    ];
    let header = BATCH.split("[[observation]]").next().expect("header");
    cases.push(format!(
        "{header}[[observation]]\nsample_id = \"ONLY\"\nvalue = \"100\"\n"
    ));
    for input in cases {
        std::fs::write(root.join(INPUT), &input).expect("invalid source");
        let result = execute(&root);
        assert_eq!(result.status, "refused", "{}", result.to_json());
        assert!(matches!(
            result.refusal.as_ref().expect("typed refusal").code,
            "FMINE-REF-208" | "FMINE-REF-109"
        ));
        assert_eq!(result.payload_json, "null", "no partial batch result");
        flags(&parse(result.to_json().as_bytes()).expect("refusal JSON"));
    }
}

#[test]
fn zero_sd_and_threshold_equalities_remain_finite_and_bounds_are_enforced() {
    let header = BATCH.split("[[observation]]").next().expect("header");
    let header = header
        .replace("sigma_c = \"1\"", "sigma_c = \"0\"")
        .replace("sigma_l = \"2.5\"", "sigma_l = \"0\"")
        .replace("lab_running_sw = \"2\"", "lab_running_sw = \"0\"");
    let input = format!(
        "{header}[[observation]]\nsample_id = \"A\"\nvalue = \"100\"\n[[observation]]\nsample_id = \"B\"\nvalue = \"100\"\n"
    );
    let root = fixture(&input);
    let result = successful(&root);
    for basis in field(report(&result), &["single_value"])
        .as_array()
        .expect("bases")
    {
        assert_eq!(field(basis, &["warnings"]).u64(), Some(0));
        assert_eq!(field(basis, &["failures"]).u64(), Some(0));
    }
    assert_eq!(
        field(report(&result), &["canmet", "passed"]).as_bool(),
        Some(true)
    );
    near(number(field(report(&result), &["bias"]), "lower"), 0.0);
    near(number(field(report(&result), &["bias"]), "upper"), 0.0);
    let changed = input.replacen("value = \"100\"", "value = \"100.5\"", 1);
    std::fs::write(root.join(INPUT), changed).expect("nonzero departure");
    let result = successful(&root);
    for basis in field(report(&result), &["single_value"])
        .as_array()
        .expect("bases")
    {
        assert_eq!(field(basis, &["failures"]).u64(), Some(1));
    }
    assert_eq!(
        field(report(&result), &["canmet", "passed"]).as_bool(),
        Some(false)
    );
    std::fs::write(root.join(INPUT), vec![b'x'; 1024 * 1024 + 1]).expect("oversized source");
    assert_eq!(
        execute(&root).refusal.expect("size refusal").code,
        "FMINE-REF-506"
    );
    let mut too_many = header;
    for i in 0..4097 {
        let _ = std::fmt::Write::write_fmt(
            &mut too_many,
            format_args!("[[observation]]\nsample_id = \"S{i}\"\nvalue = \"100\"\n"),
        );
    }
    std::fs::write(root.join(INPUT), too_many).expect("too many rows");
    assert_eq!(
        execute(&root).refusal.expect("row bound").code,
        "FMINE-REF-208"
    );
}
