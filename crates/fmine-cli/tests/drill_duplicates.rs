//! SYNTHETIC source-to-duplicate precision checks (`BD-167`, `BD-284`), without site qualification.

#![allow(clippy::expect_used, clippy::panic)]

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use fmine_cli::{Envelope, OutputFormat, invoke};
use fmine_core::hash::sha256;
use fmine_io::omf::json::{Json, parse};

const BATCH: &str = include_str!("../../../tests/drill/duplicate_batch.toml");
const INPUT: &str = "duplicate_batch.toml";
static NEXT: AtomicU64 = AtomicU64::new(0);

fn fixture(text: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "fmine-duplicates-cli-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&root).expect("fixture directory");
    std::fs::write(root.join(INPUT), text).expect("synthetic source");
    std::fs::canonicalize(root).expect("canonical root")
}

fn args() -> Vec<String> {
    ["drill", "duplicates", "--input", INPUT]
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

fn group<'a>(value: &'a Json, band: &str, stage: &str) -> &'a Json {
    field(report(value), &["groups"])
        .as_array()
        .expect("groups")
        .iter()
        .find(|group| {
            field(group, &["grade_band"]).as_str() == Some(band)
                && field(group, &["stage"]).as_str() == Some(stage)
        })
        .expect("named group")
}

fn number(value: &Json, key: &str) -> f64 {
    field(value, &[key]).f64().expect("finite numeric value")
}

fn near(got: f64, expected: f64) {
    assert!((got - expected).abs() < 1e-11, "{got:e} != {expected:e}");
}

fn flags(value: &Json) {
    let flags = field(value, &["flags"])
        .as_object()
        .expect("authority flags");
    assert_eq!(flags.len(), 4);
    assert!(flags.iter().all(|(_, flag)| flag.as_bool() == Some(false)));
    assert_eq!(field(value, &["anchor_id"]), &Json::Null);
    assert_eq!(field(value, &["certificate"]), &Json::Null);
}

#[test]
fn native_duplicate_command_is_deterministic_source_bound_and_read_only() {
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
    assert_eq!(first.stdout, native().stdout);
    let value = successful(&root);
    flags(&value);
    for (key, expected) in [
        ("evidence", "Assumed"),
        ("calibration", "uncalibrated"),
        ("confidentiality", "Internal"),
        ("additivity", "NonAdditive"),
    ] {
        assert_eq!(
            field(&value, &["payload", "claim", key]).as_str(),
            Some(expected)
        );
    }
    assert_eq!(
        field(&value, &["payload", "claim", "aggregate_score"]),
        &Json::Null
    );
    let r = report(&value);
    assert_eq!(field(r, &["unit"]).as_str(), Some("mass_fraction"));
    assert_eq!(field(r, &["basis"]).as_str(), Some("dry"));
    assert_eq!(
        field(r, &["source", "sha256"]).as_str(),
        Some(sha256(BATCH.as_bytes()).to_string().as_str())
    );
    assert_eq!(
        field(r, &["source", "bytes"]).u64(),
        Some(BATCH.len() as u64)
    );
    let lines: Vec<_> = BATCH
        .lines()
        .enumerate()
        .filter_map(|(i, line)| (line == "[[pair]]").then_some((i + 1) as u64))
        .collect();
    let rows = field(r, &["pairs"]).as_array().expect("all source pairs");
    assert_eq!(rows.len(), 10);
    for (i, row) in rows.iter().enumerate() {
        assert_eq!(field(row, &["index"]).u64(), Some(i as u64));
        assert_eq!(field(row, &["source_line"]).u64(), Some(lines[i]));
        assert_eq!(
            field(row, &["original_sample_id"]).as_str(),
            Some(format!("SYN-O{:02}", i + 1).as_str())
        );
        assert_eq!(
            field(row, &["duplicate_sample_id"]).as_str(),
            Some(format!("SYN-D{:02}", i + 1).as_str())
        );
    }
    assert_eq!(
        std::fs::read(root.join(INPUT)).expect("unchanged source"),
        BATCH.as_bytes()
    );
    assert_eq!(
        std::fs::read_dir(root)
            .expect("read-only source directory")
            .count(),
        1
    );
}

#[test]
fn duplicate_known_answers_exclude_unfit_pairs_without_erasing_their_identity() {
    let value = successful(&fixture(BATCH));
    let r = report(&value);
    assert_eq!(field(r, &["input_pairs"]).u64(), Some(10));
    assert_eq!(field(r, &["valid_pairs"]).u64(), Some(6));
    let expected = [
        ("field", 1.0_f64 / 121.0 + 1.0 / 81.0, false),
        ("coarse_reject", 1.0 / 441.0 + 1.0 / 361.0, true),
        ("pulp", 1.0 / 10201.0 + 1.0 / 39601.0, true),
    ];
    for (stage, variance, passed) in expected {
        let group = group(&value, "low", stage);
        assert_eq!(
            field(group, &["precision_policy_passed"]).as_bool(),
            Some(passed)
        );
        let stats = field(group, &["statistics"]);
        assert_eq!(field(stats, &["valid_pairs"]).u64(), Some(2));
        near(number(stats, "cv_avr_pct"), 100.0 * variance.sqrt());
        near(number(stats, "relative_variance"), variance);
        near(
            number(stats, "hard_from_cv_pct"),
            100.0 * variance.sqrt() / 2.0_f64.sqrt(),
        );
        near(
            number(stats, "ampd_from_cv_pct"),
            100.0 * variance.sqrt() * 2.0_f64.sqrt(),
        );
        assert_eq!(field(stats, &["thompson_howarth"]), &Json::Null);
        assert_eq!(field(stats, &["unverified_support_pairs"]).u64(), Some(0));
    }
    let field_group = group(&value, "low", "field");
    let stats = field(field_group, &["statistics"]);
    assert_eq!(field(stats, &["input_pairs"]).u64(), Some(5));
    for key in [
        "excluded_censored",
        "excluded_zero_pairs",
        "mass_mismatches",
    ] {
        assert_eq!(field(stats, &[key]).u64(), Some(1));
    }
    let dispositions = field(field_group, &["pairs"])
        .as_array()
        .expect("pair eligibility");
    for (i, (pair, state)) in dispositions
        .iter()
        .zip([
            "included",
            "included",
            "excluded_censored",
            "excluded_both_zero",
            "excluded_mass_support_mismatch",
        ])
        .enumerate()
    {
        assert_eq!(field(pair, &["source_index"]).u64(), Some(i as u64));
        assert_eq!(field(pair, &["status"]).as_str(), Some(state));
        if i >= 2 {
            assert_eq!(field(pair, &["pair_hard_pct"]), &Json::Null);
        }
    }
    let rows = field(r, &["pairs"]).as_array().expect("source pairs");
    assert_eq!(
        field(&rows[2], &["duplicate", "kind"]).as_str(),
        Some("below_detection")
    );
    near(number(field(&rows[2], &["duplicate"]), "limit"), 0.000005);
    assert_eq!(field(&rows[2], &["duplicate", "substitution"]), &Json::Null);
    assert_eq!(field(&rows[4], &["duplicate_mass_g"]).u64(), Some(250));
    let decomposition = field(r, &["stage_decomposition"])
        .as_array()
        .expect("stage components")
        .iter()
        .find(|entry| field(entry, &["grade_band"]).as_str() == Some("low"))
        .expect("low band");
    assert_eq!(
        field(decomposition, &["status"]).as_str(),
        Some("identified")
    );
    near(
        number(decomposition, "analytical_relative_variance"),
        expected[2].1,
    );
    near(
        number(decomposition, "preparation_incremental_relative_variance"),
        expected[1].1 - expected[2].1,
    );
    near(
        number(decomposition, "field_incremental_relative_variance"),
        expected[0].1 - expected[1].1,
    );
}

#[test]
fn all_censored_groups_stay_unavailable_and_do_not_borrow_other_band_precision() {
    let value = successful(&fixture(BATCH));
    let unavailable = group(&value, "high", "pulp");
    assert_eq!(
        field(unavailable, &["status"]).as_str(),
        Some("unavailable")
    );
    assert_eq!(field(unavailable, &["statistics"]), &Json::Null);
    assert_eq!(
        field(unavailable, &["precision_policy_passed"]),
        &Json::Null
    );
    assert_eq!(
        field(unavailable, &["source_indices"])
            .as_array()
            .expect("unavailable source row")[0]
            .u64(),
        Some(9)
    );
    let high = field(report(&value), &["stage_decomposition"])
        .as_array()
        .expect("decomposition")
        .iter()
        .find(|entry| field(entry, &["grade_band"]).as_str() == Some("high"))
        .expect("high band");
    assert_eq!(
        field(high, &["status"]).as_str(),
        Some("field_duplicate_absent")
    );
    for key in [
        "analytical_relative_variance",
        "preparation_incremental_relative_variance",
        "field_incremental_relative_variance",
    ] {
        assert_eq!(field(high, &[key]), &Json::Null);
    }
    assert_eq!(
        field(high, &["unavailable_supplied_stages"])
            .as_array()
            .expect("unavailable stage")[0]
            .as_str(),
        Some("pulp")
    );
    let mut parts = BATCH.split("[[pair]]");
    let header = parts.next().expect("header");
    let pairs = parts.collect::<Vec<_>>();
    let only_censored = format!("{header}[[pair]]{}", pairs[9]);
    let censored_value = successful(&fixture(&only_censored));
    assert_eq!(
        field(report(&censored_value), &["valid_pairs"]).u64(),
        Some(0)
    );
    assert_eq!(
        field(group(&censored_value, "high", "pulp"), &["statistics"]),
        &Json::Null
    );
    let reordered = format!(
        "{header}{}",
        pairs
            .iter()
            .rev()
            .map(|pair| format!("[[pair]]{pair}"))
            .collect::<String>()
    );
    let reverse = successful(&fixture(&reordered));
    for stage in ["field", "coarse_reject", "pulp"] {
        assert_eq!(
            field(group(&value, "low", stage), &["statistics"]),
            field(group(&reverse, "low", stage), &["statistics"])
        );
    }
}

#[test]
fn markdown_explains_precision_limits_and_source_changes_change_receipts() {
    let root = fixture(BATCH);
    let first = successful(&root);
    let mut markdown_args = args();
    markdown_args.extend(["--format".into(), "markdown".into()]);
    let (envelope, format) = invoke(&markdown_args, &root);
    assert_eq!(format, OutputFormat::Markdown);
    assert_eq!(envelope.exit_code, 0, "{}", envelope.to_json());
    let markdown = envelope.rendered_markdown.expect("human assessment");
    for text in [
        "Duplicate precision assessment",
        "Grade band",
        "CV_AVR",
        "coarse_reject",
        "low",
        "high",
        "unavailable",
        "Omitted uncertainty:",
        "Thompson–Howarth",
        "causal geological/sampling nugget split",
        "SHA-256",
        "synthetic-duplicate-policy",
    ] {
        assert!(markdown.contains(text), "missing {text}");
    }
    let changed = format!("{BATCH}\n# Additional synthetic source declaration.\n");
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
        field(report(&first), &["groups"]),
        field(report(&second), &["groups"])
    );
    assert_eq!(
        field(report(&second), &["source", "sha256"]).as_str(),
        Some(sha256(changed.as_bytes()).to_string().as_str())
    );
}

#[test]
fn strict_support_and_invalid_declarations_refuse_without_partial_results() {
    let root = fixture(&BATCH.replace(
        "refuse_on_mass_mismatch = \"false\"",
        "refuse_on_mass_mismatch = \"true\"",
    ));
    let strict = execute(&root);
    assert_eq!(strict.status, "refused");
    assert_eq!(
        strict.refusal.expect("support refusal").code,
        "FMINE-REF-102"
    );
    assert_eq!(strict.payload_json, "null");
    let cases = [
        BATCH.replace("unit = \"mass_fraction\"", "unit = \"ppm\""),
        BATCH.replace("basis = \"dry\"", "basis = \"unknown\""),
        BATCH.replace("grade_band = \"low\"\n", ""),
        BATCH.replace("original_mass_g = \"500\"", "original_mass_g = \"1e-400\""),
        BATCH.replace("duplicate_mass_g = \"250\"", "duplicate_mass_g = \"NaN\""),
        BATCH.replace(
            "original_sample_type = \"half_core\"",
            "original_sample_type = \"qc_material\"",
        ),
        BATCH.replace(
            "duplicate_sample_id = \"SYN-D02\"",
            "duplicate_sample_id = \"SYN-D01\"",
        ),
        BATCH.replace(
            "original_sample_id = \"SYN-O02\"",
            "original_sample_id = \"SYN-O01\"",
        ),
        BATCH.replace(
            "original_sample_id = \"SYN-O02\"",
            "original_sample_id = \"SYN-D01\"",
        ),
        BATCH.replace("cv_limit_pct = \"10\"", "cv_limit_pct = \"-1\""),
        BATCH.replace("duplicate = \"<0.00001\"", "duplicate = \"<0\""),
        BATCH.replace("original = \">0.001\"", "original = \">1\""),
        BATCH.replace("[[pair]]", "[[unrecognized]]"),
    ];
    for input in cases {
        std::fs::write(root.join(INPUT), input).expect("invalid source");
        let result = execute(&root);
        assert_eq!(result.status, "refused", "{}", result.to_json());
        assert!(matches!(
            result.refusal.as_ref().expect("typed refusal").code,
            "FMINE-REF-208" | "FMINE-REF-109"
        ));
        assert_eq!(result.payload_json, "null");
        flags(&parse(result.to_json().as_bytes()).expect("refusal JSON"));
    }
}
