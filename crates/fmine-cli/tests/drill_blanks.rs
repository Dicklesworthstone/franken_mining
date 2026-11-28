//! Synthetic source-to-blank checks (`BD-167`), including genuine indeterminacy from censoring.
//! Determinism: BitwisePortable.

#![allow(clippy::expect_used, clippy::panic)]

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use fmine_cli::{Envelope, OutputFormat, invoke};
use fmine_core::hash::sha256;
use fmine_io::omf::json::{Json, parse};

const BATCH: &str = include_str!("../../../tests/drill/blank_batch.toml");
const INPUT: &str = "blank_batch.toml";
static NEXT: AtomicU64 = AtomicU64::new(0);

fn fixture(text: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "fmine-blank-cli-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&root).expect("fixture directory");
    std::fs::write(root.join(INPUT), text).expect("synthetic batch");
    std::fs::canonicalize(root).expect("canonical root")
}

fn args() -> Vec<String> {
    ["drill", "blanks", "--input", INPUT]
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
    path.iter().fold(value, |v, key| v.get(key).expect(key))
}

fn report(value: &Json) -> &Json {
    field(value, &["payload", "report"])
}
fn rows(value: &Json) -> &[Json] {
    field(report(value), &["observations"])
        .as_array()
        .expect("rows")
}
fn statuses(value: &Json) -> Vec<&str> {
    rows(value)
        .iter()
        .map(|row| field(row, &["status"]).as_str().expect("status"))
        .collect()
}

#[test]
fn native_blank_command_preserves_every_row_denominator_and_bound() {
    let root = fixture(BATCH);
    let native = || {
        Command::new(env!("CARGO_BIN_EXE_fmine"))
            .current_dir(&root)
            .args(args())
            .output()
            .expect("native fmine")
    };
    let output = native();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim_end(),
        execute(&root).to_json()
    );
    assert_eq!(output.stdout, native().stdout);
    let result = successful(&root);
    assert_eq!(
        statuses(&result),
        [
            "pass_below_detection",
            "pass_measured",
            "indeterminate",
            "indeterminate",
            "indeterminate",
            "indeterminate",
            "fail_contamination",
            "pass_measured"
        ]
    );
    let report = report(&result);
    assert_eq!(field(report, &["n"]).u64(), Some(8));
    assert_eq!(field(report, &["passes"]).u64(), Some(3));
    assert_eq!(field(report, &["failures"]).u64(), Some(1));
    assert_eq!(field(report, &["indeterminate"]).u64(), Some(4));
    assert_eq!(field(report, &["below_detection"]).u64(), Some(2));
    assert_eq!(
        field(report, &["confirmed_failure_rate_pct"]).f64(),
        Some(12.5)
    );
    assert_eq!(
        field(report, &["possible_failure_rate_pct"]).f64(),
        Some(62.5)
    );
    assert_eq!(
        field(report, &["failure_rate_denominator"]).as_str(),
        Some("all_supplied_blanks")
    );
    let source_lines: Vec<_> = BATCH
        .lines()
        .enumerate()
        .filter_map(|(i, line)| (line == "[[observation]]").then_some(i + 1))
        .collect();
    for (i, row) in rows(&result).iter().enumerate() {
        assert_eq!(field(row, &["index"]).u64(), Some(i as u64));
        assert_eq!(
            field(row, &["source_line"]).u64(),
            Some(source_lines[i] as u64)
        );
        assert_eq!(
            field(row, &["sample_id"]).as_str(),
            Some(format!("BLANK-{:02}", i + 1).as_str())
        );
    }
    let first = &rows(&result)[0];
    assert_eq!(
        field(first, &["value", "kind"]).as_str(),
        Some("below_detection")
    );
    assert_eq!(field(first, &["value", "limit"]).f64(), Some(5e-9));
    assert_eq!(field(first, &["value", "substitution"]), &Json::Null);
    assert_eq!(
        field(&rows(&result)[3], &["preceding_sample", "value", "kind"]).as_str(),
        Some("above_upper")
    );
    assert_eq!(field(&rows(&result)[4], &["preceding_sample"]), &Json::Null);
    let labs = field(report, &["per_lab"]).as_array().expect("labs");
    assert_eq!(labs.len(), 2);
    for (lab, (name, passes, failures, unknown)) in labs
        .iter()
        .zip([("SYNTHETIC-A", 2, 0, 2), ("SYNTHETIC-B", 1, 1, 2)])
    {
        assert_eq!(field(lab, &["lab"]).as_str(), Some(name));
        assert_eq!(field(lab, &["n"]).u64(), Some(4));
        assert_eq!(field(lab, &["passes"]).u64(), Some(passes));
        assert_eq!(field(lab, &["failures"]).u64(), Some(failures));
        assert_eq!(field(lab, &["indeterminate"]).u64(), Some(unknown));
    }
    assert_eq!(
        field(report, &["source", "sha256"]).as_str(),
        Some(sha256(BATCH.as_bytes()).to_string().as_str())
    );
    assert_eq!(
        field(report, &["source", "bytes"]).u64(),
        Some(BATCH.len() as u64)
    );
    for (_, flag) in field(&result, &["flags"])
        .as_object()
        .expect("authority flags")
    {
        assert_eq!(flag.as_bool(), Some(false));
    }
    assert_eq!(
        field(&result, &["payload", "claim", "evidence"]).as_str(),
        Some("Assumed")
    );
    assert_eq!(
        field(&result, &["payload", "claim", "calibration"]).as_str(),
        Some("uncalibrated")
    );
    assert_eq!(
        field(&result, &["payload", "claim", "aggregate_score"]),
        &Json::Null
    );
    assert_eq!(
        std::fs::read(root.join(INPUT)).expect("source retained"),
        BATCH.as_bytes()
    );
    assert_eq!(
        std::fs::read_dir(root).expect("read-only command").count(),
        1
    );
}

#[test]
fn and_or_policy_changes_and_unknown_predecessors_change_the_actual_verdicts() {
    let root = fixture(BATCH);
    let and = successful(&root);
    let or_source = BATCH.replace("rule = \"carryover_and\"", "rule = \"carryover_or\"");
    std::fs::write(root.join(INPUT), &or_source).expect("explicit OR rule");
    let or = successful(&root);
    assert_eq!(
        statuses(&or),
        [
            "pass_below_detection",
            "fail_contamination",
            "indeterminate",
            "fail_contamination",
            "fail_contamination",
            "indeterminate",
            "fail_contamination",
            "pass_measured"
        ]
    );
    assert_eq!(field(report(&or), &["passes"]).u64(), Some(2));
    assert_eq!(field(report(&or), &["failures"]).u64(), Some(4));
    assert_eq!(field(report(&or), &["indeterminate"]).u64(), Some(2));
    assert_eq!(
        field(report(&or), &["confirmed_failure_rate_pct"]).f64(),
        Some(50.0)
    );
    assert_eq!(
        field(report(&or), &["possible_failure_rate_pct"]).f64(),
        Some(75.0)
    );
    assert_eq!(
        field(report(&and), &["policy", "mode"]).as_str(),
        Some("carryover_and")
    );
    assert_eq!(
        field(report(&or), &["policy", "mode"]).as_str(),
        Some("carryover_or")
    );
    assert_ne!(
        field(&and, &["payload", "claim", "id"]),
        field(&or, &["payload", "claim", "id"])
    );
    assert_ne!(
        field(&and, &["payload", "receipt_sha256"]),
        field(&or, &["payload", "receipt_sha256"])
    );
}

#[test]
fn versioned_policy_and_source_identity_are_visible_in_json_and_markdown() {
    let root = fixture(BATCH);
    let first = successful(&root);
    assert_eq!(
        field(report(&first), &["policy_id"]).as_str(),
        Some("synthetic.blank.carryover")
    );
    assert_eq!(
        field(report(&first), &["policy_version"]).as_str(),
        Some("1")
    );
    let mut markdown_args = args();
    markdown_args.extend(["--format".into(), "markdown".into()]);
    let (envelope, format) = invoke(&markdown_args, &root);
    assert_eq!(format, OutputFormat::Markdown);
    assert_eq!(envelope.exit_code, 0);
    let markdown = envelope.rendered_markdown.expect("human report");
    for text in [
        "Blank contamination assessment",
        "8 supplied",
        "4 indeterminate",
        "Denominator includes every supplied blank",
        "synthetic.blank.carryover@1",
        "BLANK-01",
        "BLANK-08",
        "Omitted uncertainty",
        "SHA-256",
    ] {
        assert!(markdown.contains(text), "missing {text}");
    }
    let changed = BATCH.replace("policy_version = \"1\"", "policy_version = \"2\"");
    std::fs::write(root.join(INPUT), changed).expect("new policy revision");
    let second = successful(&root);
    assert_eq!(statuses(&first), statuses(&second));
    assert_ne!(
        field(&first, &["payload", "claim", "id"]),
        field(&second, &["payload", "claim", "id"])
    );
    assert_ne!(
        field(&first, &["payload", "receipt_sha256"]),
        field(&second, &["payload", "receipt_sha256"])
    );
}

#[test]
fn dl_rule_uses_reported_censoring_limits_and_has_exact_strict_boundary_behavior() {
    let top = BATCH.split("[[observation]]").next().expect("top fields")
        .replace("rule = \"carryover_and\"\nabs_threshold = \"0.00000002\"\ncarryover_fraction = \"0.01\"", "rule = \"dl_multiple\"\ndetection_limit = \"0.125\"\ndl_multiple = \"2\"");
    let rows = ["<0.25", "<0.5", ">0.25", "0.25"].iter().enumerate().map(|(i, value)| format!(
        "\n[[observation]]\nresult_id = \"R{i}\"\nsample_id = \"S{i}\"\nlab = \"SYNTHETIC\"\nbatch = \"B\"\nstage = \"pulp\"\nvalue = \"{value}\"\n"
    )).collect::<String>();
    let root = fixture(&format!("{top}{rows}"));
    let result = successful(&root);
    assert_eq!(
        statuses(&result),
        [
            "pass_below_detection",
            "indeterminate",
            "fail_contamination",
            "pass_measured"
        ]
    );
    assert_eq!(
        field(report(&result), &["confirmed_failure_rate_pct"]).f64(),
        Some(25.0)
    );
    assert_eq!(
        field(report(&result), &["possible_failure_rate_pct"]).f64(),
        Some(50.0)
    );
}

#[test]
fn malformed_or_ambiguous_declarations_refuse_before_scoring() {
    for changed in [
        BATCH.replace("unit = \"mass_fraction\"", "unit = \"ppm\""),
        BATCH.replace("basis = \"dry\"", "basis = \"unspecified\""),
        BATCH.replace("policy_source = \"Synthetic exercise with explicit AND logic; not an approved site policy\"\n", ""),
        BATCH.replace("carryover_fraction = \"0.01\"", "carryover_fraction = \"1e-400\""),
        BATCH.replace("value = \"0.000000025\"", "value = \"NSS\""),
        BATCH.replace("value = \"<0.000000005\"", "value = \"<0\""),
        BATCH.replace("value = \">0.0000001\"", "value = \">1\""),
        BATCH.replace("sample_id = \"BLANK-02\"", "sample_id = \"BLANK-01\""),
        BATCH.replace("preceding_sample_id = \"ORE-01\"\n", ""),
        BATCH.replace("preceding_sample_id = \"ORE-01\"", "preceding_sample_id = \"BLANK-01\""),
        BATCH.replace("carryover_fraction = \"0.01\"", "carryover_fraction = \"0.01\"\ndl_multiple = \"5\""),
    ] {
        let root = fixture(&changed);
        let result = execute(&root);
        assert_ne!(result.exit_code, 0, "{}", result.to_json());
        assert_eq!(std::fs::read(root.join(INPUT)).expect("unchanged refusal source"), changed.as_bytes());
        assert_eq!(std::fs::read_dir(root).expect("no side effects").count(), 1);
    }
}
