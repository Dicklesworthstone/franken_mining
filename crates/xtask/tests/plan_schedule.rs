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
