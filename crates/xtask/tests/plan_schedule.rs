//! Real CLI scheduling with an independent exact Fraction and exhaustive-search oracle.
#![cfg(test)]

use std::path::PathBuf;
use std::process::Command;

use fmine_core::{
    cert::InputDigest,
    hash::sha256,
    plan_cert::{PlanCalibrationStatus, PlanCertificate, PlanColor, PlanHorizon},
};
use fmine_io::{
    forecast::decode_hex_field,
    omf::json::{Json, parse},
};
use fmine_units::ExactQty as Rat;

fn binary() -> PathBuf {
    std::env::var_os("FMINE_TEST_BINARY")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            let executable = std::env::current_exe().unwrap();
            executable
                .ancestors()
                .find(|p| p.file_name().is_some_and(|n| n == "deps" || n == "build"))
                .and_then(std::path::Path::parent)
                .unwrap()
                .join(format!("fmine{}", std::env::consts::EXE_SUFFIX))
        })
}

#[test]
fn split_candidates_and_global_bounds_match_independent_enumeration() {
    let output = Command::new("python3")
        .arg("-I")
        .arg(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("testdata/plan_schedule_oracle.py"))
        .arg(binary())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn ratio(value: &Json) -> Rat {
    let integer = |field| value.get(field).unwrap().as_str().unwrap().parse().unwrap();
    Rat::new(integer("numerator"), integer("denominator")).unwrap()
}

fn lane(root: &std::path::Path, input: &str, format: &str) -> std::process::Output {
    Command::new(binary())
        .args([
            "plan",
            "cutoff",
            "--input",
            input,
            "--format",
            format,
            "--workspace",
        ])
        .arg(root)
        .output()
        .unwrap()
}

fn lane_json(output: &std::process::Output) -> Json {
    let report = parse(&output.stdout).unwrap();
    assert_eq!(report.get("verb").unwrap().as_str(), Some("plan cutoff"));
    assert_eq!(
        report.get("exit_code").unwrap().i64(),
        output.status.code().map(i64::from)
    );
    assert!(
        report
            .get("flags")
            .unwrap()
            .as_object()
            .unwrap()
            .iter()
            .all(|(_, value)| { value.as_bool() == Some(false) })
    );
    report
}

#[test]
fn lane_recovery_curve_binary_distinguishes_exact_crossings_and_support_limits() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let input = "tests/planning/lane_recovery_curve.json";
    let original = std::fs::read_to_string(root.join(input)).unwrap();
    let result = lane(&root, input, "json");
    let report = lane_json(&result);
    assert!(result.status.success(), "{report:?}");
    assert_eq!(result.stdout, lane(&root, input, "json").stdout);
    let payload = report.get("payload").unwrap();
    let digest = sha256(original.as_bytes()).to_hex();
    assert_eq!(
        payload
            .get("source")
            .unwrap()
            .get("sha256")
            .unwrap()
            .as_str(),
        Some(digest.as_str())
    );
    let support = payload.get("recovery_curve_support").unwrap();
    assert_eq!(support.get("lower").unwrap().as_str(), Some("1/5"));
    assert_eq!(support.get("upper").unwrap().as_str(), Some("3/5"));
    let rows = payload.get("limiting_cutoffs").unwrap().as_array().unwrap();
    // Between the first two knots recovered grade equals feed grade minus 0.1.
    // The three required recovered grades are 20/100, 30/100 and 20/90.
    let expected = [
        ("3/10", "entire_band_clears"),
        ("2/5", "entire_band_below"),
        ("29/90", "straddles"),
    ];
    assert_eq!(rows.len(), expected.len());
    for (row, (grade, screen)) in rows.iter().zip(expected) {
        assert_eq!(row.get("cutoff_grade").unwrap().as_str(), Some(grade));
        assert_eq!(
            row.get("cutoff_status").unwrap().as_str(),
            Some("within_support")
        );
        assert_eq!(row.get("feed_grade_screen").unwrap().as_str(), Some(screen));
    }
    let markdown = lane(&root, input, "markdown");
    assert!(markdown.status.success());
    assert!(
        String::from_utf8(markdown.stdout)
            .unwrap()
            .contains("| market | 29/90 | Straddles |")
    );

    let scratch = std::env::temp_dir().join(format!(
        "fmine-lane-recovery-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&scratch).unwrap();
    let submit = |name: &str, text: &str| {
        std::fs::write(scratch.join(name), text).unwrap();
        lane_json(&lane(&scratch, name, "json"))
    };
    let (before_curve, _) = original.split_once("\"recovery_curve\":").unwrap();
    let declared = before_curve.replace("\"0.3\"", "\"0.32\"");
    let short = submit(
        "short.json",
        &format!(
            r#"{declared}"recovery_curve": [
            {{ "grade_per_dry_tonne": "0.32", "recovery_fraction": "0.75" }},
            {{ "grade_per_dry_tonne": "0.36", "recovery_fraction": "0.75" }}
        ]}}"#
        ),
    );
    let rows = short
        .get("payload")
        .unwrap()
        .get("limiting_cutoffs")
        .unwrap()
        .as_array()
        .unwrap();
    for (row, (status, screen)) in rows.iter().zip([
        ("all_supported_grades_clear", "entire_band_clears"),
        ("no_supported_grade_clears", "entire_band_below"),
        ("all_supported_grades_clear", "entire_band_clears"),
    ]) {
        assert_eq!(row.get("cutoff_grade"), Some(&Json::Null));
        assert_eq!(row.get("cutoff_status").unwrap().as_str(), Some(status));
        assert_eq!(row.get("feed_grade_screen").unwrap().as_str(), Some(screen));
    }
    let constant = submit(
        "constant.json",
        &format!(r#"{before_curve}"recovery_fraction":"0.75"}}"#),
    );
    let market = &constant
        .get("payload")
        .unwrap()
        .get("limiting_cutoffs")
        .unwrap()
        .as_array()
        .unwrap()[2];
    assert_eq!(market.get("cutoff_grade").unwrap().as_str(), Some("8/27"));
    assert_eq!(
        market.get("feed_grade_screen").unwrap().as_str(),
        Some("entire_band_clears")
    );
    assert!(market.get("cutoff_status").is_none());

    for (index, (from, to, code)) in [
        ("\"0.33\"", "\"0.61\"", "FMINE-REF-203"),
        ("\"0.3\"", "\"0.19\"", "FMINE-REF-203"),
        ("\"0.75\"", "\"0.1\"", "FMINE-REF-109"),
        ("\"0.75\"", "\"1.1\"", "FMINE-REF-109"),
        ("\"0.6\"", "\"0.4\"", "FMINE-REF-109"),
        ("\"0.6\"", "\"1000001\"", "FMINE-REF-109"),
        (
            "\"recovery_curve\":",
            "\"recovery_fraction\":\"0.75\",\"recovery_curve\":",
            "FMINE-REF-107",
        ),
    ]
    .into_iter()
    .enumerate()
    {
        assert!(original.contains(from));
        let out = submit(
            &format!("invalid-{index}.json"),
            &original.replace(from, to),
        );
        assert_eq!(out.get("payload"), Some(&Json::Null));
        assert_eq!(
            out.get("refusal").unwrap().get("code").unwrap().as_str(),
            Some(code)
        );
    }
}

#[test]
fn lane_cutoff_binary_replays_pinned_symbolic_ratios_and_grade_screens() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let input = "tests/planning/lane_cutoff.json";
    let result = lane(&root, input, "json");
    let report = lane_json(&result);
    assert!(result.status.success(), "{report:?}");
    assert_eq!(result.stdout, lane(&root, input, "json").stdout);
    let p = report.get("payload").unwrap();
    let source = p.get("source").unwrap();
    let bytes = std::fs::read(root.join(input)).unwrap();
    assert_eq!(source.get("bytes").unwrap().u64(), Some(bytes.len() as u64));
    let digest = sha256(&bytes).to_hex();
    assert_eq!(
        source.get("sha256").unwrap().as_str(),
        Some(digest.as_str())
    );
    for (key, expected) in [
        ("input_evidence", "synthetic"),
        ("calibration", "uncalibrated"),
        ("metal", "Au"),
        ("metal_unit", "g"),
        ("ore_mass_basis", "dry_metric_tonne"),
        ("ownership_basis", "100_percent_project"),
    ] {
        assert_eq!(p.get(key).unwrap().as_str(), Some(expected), "{key}");
    }
    let rows = p.get("limiting_cutoffs").unwrap().as_array().unwrap();
    let oracle = include_str!("../../../fixtures/fmine-derive/golden_formulas.txt");
    let expected = [
        ("lane_mine_cutoff", "mine", "entire_band_clears"),
        ("lane_mill_cutoff", "mill", "entire_band_below"),
        ("lane_refinery_cutoff", "market", "straddles"),
    ];
    assert_eq!(rows.len(), expected.len());
    for (row, (id, case, screen)) in rows.iter().zip(expected) {
        let expected_grade = oracle
            .lines()
            .find_map(|line| {
                let mut cols = line.split('\t');
                if cols.next() == Some("FORMULA") && cols.next() == Some(id) {
                    cols.nth(5)
                } else {
                    None
                }
            })
            .unwrap();
        assert_eq!(row.get("limiting_constraint").unwrap().as_str(), Some(case));
        assert_eq!(
            row.get("cutoff_grade").unwrap().as_str(),
            Some(expected_grade)
        );
        assert_eq!(row.get("feed_grade_screen").unwrap().as_str(), Some(screen));
    }
    let markdown = lane(&root, input, "markdown");
    assert!(markdown.status.success());
    let markdown = String::from_utf8(markdown.stdout).unwrap();
    assert!(markdown.contains("| mine | 1/5 | Entire band clears |"));
    assert!(markdown.contains("| mill | 4/15 | Entire band below |"));
    assert!(markdown.contains("| market | 5/23 | Straddles |"));
    assert!(markdown.contains(&digest));
    let robot = Command::new(binary())
        .args(["robot-docs", "--json"])
        .output()
        .unwrap();
    assert!(robot.status.success());
    assert!(
        parse(&robot.stdout)
            .unwrap()
            .get("payload")
            .unwrap()
            .get("plan_cutoff")
            .is_some()
    );
}

#[test]
fn lane_cutoff_binary_preserves_units_and_refuses_incomplete_or_invalid_basis() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let scratch = std::env::temp_dir().join(format!(
        "fmine-lane-cutoff-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&scratch).unwrap();
    let original = std::fs::read_to_string(root.join("tests/planning/lane_cutoff.json")).unwrap();
    let write = |name: &str, text: &str| {
        std::fs::write(scratch.join(name), text).unwrap();
        lane_json(&lane(&scratch, name, "json"))
    };
    let grams = write("grams.json", &original);
    let kilos_text = original
        .replace("\"g\"", "\"kg\"")
        .replace("\"100\"", "\"100000\"")
        .replace("\"5\"", "\"5000\"")
        .replace("\"7.5\"", "\"0.0075\"")
        .replace("\"0.2\"", "\"0.0002\"")
        .replace("\"0.24\"", "\"0.00024\"");
    let kilos = write("kilos.json", &kilos_text);
    let parsed_grade = |row: &Json| {
        let (n, d) = row
            .get("cutoff_grade")
            .unwrap()
            .as_str()
            .unwrap()
            .split_once('/')
            .unwrap();
        Rat::new(n.parse().unwrap(), d.parse().unwrap()).unwrap()
    };
    let payload = |json: Json| json.get("payload").unwrap().clone();
    let (grams, kilos) = (payload(grams), payload(kilos));
    assert_ne!(
        grams.get("source").unwrap().get("sha256"),
        kilos.get("source").unwrap().get("sha256")
    );
    for (g, kg) in grams
        .get("limiting_cutoffs")
        .unwrap()
        .as_array()
        .unwrap()
        .iter()
        .zip(kilos.get("limiting_cutoffs").unwrap().as_array().unwrap())
    {
        assert_eq!(
            parsed_grade(kg)
                .checked_mul(Rat::from_integer(1000))
                .unwrap(),
            parsed_grade(g)
        );
        assert_eq!(g.get("feed_grade_screen"), kg.get("feed_grade_screen"));
    }
    let no_band_text = original.replace(
        ",\n  \"feed_grade_per_dry_tonne\": { \"lower\": \"0.2\", \"upper\": \"0.24\" }",
        "",
    );
    let no_band = payload(write("no-band.json", &no_band_text));
    assert_eq!(no_band.get("feed_grade_per_dry_tonne"), Some(&Json::Null));
    for (unit, upper) in [
        ("g", "1000001"),
        ("kg", "1001"),
        ("oz_t", "32151"),
        ("lb_avdp", "2205"),
        ("t", "1.0001"),
    ] {
        let above_mass = original
            .replace("\"g\"", &format!("\"{unit}\""))
            .replace("\"0.24\"", &format!("\"{upper}\""));
        let refusal = write(&format!("mass-{unit}.json"), &above_mass);
        assert_eq!(
            refusal.get("status").unwrap().as_str(),
            Some("refused"),
            "{unit}"
        );
        assert_eq!(
            refusal
                .get("refusal")
                .unwrap()
                .get("code")
                .unwrap()
                .as_str(),
            Some("FMINE-REF-109")
        );
    }
    let pure = original
        .replace("\"g\"", "\"t\"")
        .replace("\"0.24\"", "\"1\"");
    assert_eq!(
        write("pure.json", &pure).get("status").unwrap().as_str(),
        Some("ok")
    );
    for (index, (old, new, code)) in [
        ("\"0.9\"", "\"1.01\"", "FMINE-REF-109"),
        ("\"7.5\"", "\"0.6\"", "FMINE-REF-109"),
        ("\"0.2\"", "\"0.25\"", "FMINE-REF-109"),
        (
            "\"dry_metric_tonne\"",
            "\"wet_metric_tonne\"",
            "FMINE-REF-105",
        ),
        (
            "\"100_percent_project\"",
            "\"attributable\"",
            "FMINE-REF-105",
        ),
        (
            "\"evidence\": \"synthetic\"",
            "\"evidence\": \"synthetic\", \"evidence\": \"assumed\"",
            "FMINE-REF-107",
        ),
        (
            "\"price_per_metal_unit\"",
            "\"price_per_short_ton\"",
            "FMINE-REF-107",
        ),
    ]
    .into_iter()
    .enumerate()
    {
        assert!(original.contains(old));
        let refusal = write(
            &format!("invalid-{index}.json"),
            &original.replace(old, new),
        );
        assert_eq!(refusal.get("status").unwrap().as_str(), Some("refused"));
        assert_eq!(refusal.get("payload"), Some(&Json::Null));
        assert_eq!(
            refusal
                .get("refusal")
                .unwrap()
                .get("code")
                .unwrap()
                .as_str(),
            Some(code)
        );
    }
}

#[test]
fn schedule_exports_a_source_bound_certificate_with_the_same_value_scale() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let model = "tests/planning/joint_fractional.pcpsp";
    let precedence = "tests/planning/joint_fractional.prec";
    let candidate = "tests/planning/joint_fractional.sol";
    let run = |decimals: &str, extra: &[&str]| {
        Command::new(binary())
            .current_dir(&root)
            .args([
                "plan",
                "schedule",
                "--pcpsp",
                model,
                "--prec",
                precedence,
                "--candidate",
                candidate,
                "--decimals",
                decimals,
                "--json",
            ])
            .args(extra)
            .output()
            .unwrap()
    };
    let expected_inputs = [
        ("candidate", candidate),
        ("pcpsp", model),
        ("precedence", precedence),
    ]
    .map(|(role, path)| {
        InputDigest::new(role, path, sha256(&std::fs::read(root.join(path)).unwrap()))
    });
    let options = [
        "--certificate-currency",
        "USD",
        "--certificate-horizon",
        "tactical",
    ];
    for decimals in ["0", "6"] {
        let baseline = run(decimals, &[]);
        let output = run(decimals, &options);
        assert!(baseline.status.success());
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stdout)
        );
        let baseline = parse(&baseline.stdout).unwrap();
        let report = parse(&output.stdout).unwrap();
        let base_payload = baseline.get("payload").unwrap();
        let payload = report.get("payload").unwrap();
        let mut original = payload.as_object().unwrap().to_vec();
        original.retain(|(key, _)| key != "plan_certificate");
        assert_eq!(Json::Object(original), *base_payload);
        assert_eq!(output.stdout, run(decimals, &options).stdout);
        let preview = payload.get("plan_certificate").unwrap();
        assert_eq!(
            preview.get("format").and_then(Json::as_str),
            Some("FMT-PCERT-1")
        );
        let bytes = decode_hex_field(preview.get("bytes_hex").unwrap().as_str().unwrap()).unwrap();
        let cert = PlanCertificate::from_bytes(&bytes).unwrap();
        assert_eq!(
            preview.get("digest").unwrap().as_str().unwrap(),
            cert.certificate_digest.to_hex()
        );
        assert_eq!(cert.inputs, expected_inputs);
        assert_eq!(cert.currency_code, "USD");
        assert_eq!(cert.horizon, PlanHorizon::Tactical);
        assert_eq!(cert.color, PlanColor::Estimated);
        assert_eq!(cert.calibration, PlanCalibrationStatus::Uncalibrated);
        assert!(cert.saa.is_none() && cert.vss.is_none());
        let scale = i128::try_from(cert.value_scale_denominator).unwrap();
        let selected = ratio(
            payload
                .get("selected")
                .unwrap()
                .get("discounted_value")
                .unwrap(),
        );
        let bound = payload.get("bound").unwrap();
        assert_eq!(selected, Rat::from_integer(30));
        assert_eq!(Rat::new(cert.primal_scaled, scale).unwrap(), selected);
        assert_eq!(
            Rat::new(cert.bound.unwrap().upper_scaled, scale).unwrap(),
            ratio(bound.get("value").unwrap())
        );
        assert_eq!(
            Rat::new(cert.opt_gap_numerator.unwrap(), scale).unwrap(),
            ratio(bound.get("gap").unwrap())
        );
    }
    assert_eq!(run("0", &options[..2]).status.code(), Some(2));
    assert_eq!(
        run(
            "0",
            &[
                "--certificate-currency",
                "usd",
                "--certificate-horizon",
                "tactical"
            ]
        )
        .status
        .code(),
        Some(2)
    );
}
