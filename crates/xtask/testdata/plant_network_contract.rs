// FACT: synthetic, hand-solvable network answers exercised through the actual CLI.
use fmine_io::omf::json::{self, Json};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

const NETWORK: &str = include_str!("../../../tests/plant/flowsheet_reconcile.toml");
static SERIAL: AtomicUsize = AtomicUsize::new(0);

fn scratch(text: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "fmine-plant-network-{}-{}.toml",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::write(&path, text).expect("write synthetic network");
    path
}
fn run(path: &Path) -> (i32, Json, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_fmine"))
        .args(["plant", "flowsheet", "--input"])
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
fn close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() < 1e-10 * (1.0 + expected.abs()),
        "{actual} != {expected}"
    );
}
fn order(value: &Json) -> &[Json] {
    at(value, &["payload", "coordinate_order"])
        .as_array()
        .unwrap()
}
fn index(value: &Json, key: &str) -> usize {
    order(value)
        .iter()
        .position(|v| v.get("key").and_then(Json::as_str) == Some(key))
        .unwrap()
}
fn estimate(value: &Json, key: &str) -> f64 {
    at(value, &["payload", "result", "estimates_kg"])
        .as_array()
        .unwrap()[index(value, key)]
    .f64()
    .unwrap()
}
fn covariance(left: &str, right: &str, kg2: &str) -> String {
    format!("\n[[covariance]]\nleft = \"{left}\"\nright = \"{right}\"\nkg2 = \"{kg2}\"\n")
}
fn coordinate(observation: &str, channel: &str, kg: &str, start: i64, end: i64) -> String {
    format!(
        "\n[[coordinate]]\nobservation = \"{observation}\"\nchannel = \"{channel}\"\nkg = \"{kg}\"\nstart_ns = \"{start}\"\nend_ns = \"{end}\"\n"
    )
}
fn changed(old: &str, new: &str) -> String {
    assert!(NETWORK.contains(old), "mutation must target the fixture");
    NETWORK.replacen(old, new, 1)
}
fn refused(text: &str, code: &str) -> Json {
    let (status, value, _) = run(&scratch(text));
    assert_ne!(status, 0, "{value:?}");
    assert_eq!(
        at(&value, &["refusal", "code"]).as_str(),
        Some(code),
        "{value:?}"
    );
    assert_eq!(at(&value, &["payload", "admitted_claim"]), &Json::Null);
    value
}

#[test]
fn recycle_fit_detects_hidden_unit_imbalance_and_matches_hand_oracle() {
    let path = scratch(NETWORK);
    let (status, value, first) = run(&path);
    assert_eq!(status, 0, "{value:?}");
    assert_eq!(at(&value, &["verb"]).as_str(), Some("plant flowsheet"));
    assert_eq!(
        at(&value, &["payload", "schema"]).as_str(),
        Some("fmine.plant-flowsheet-result.v1")
    );
    // One internal observation is shared by both endpoints, never duplicated.
    assert_eq!(order(&value).len(), 24);
    assert_eq!(
        at(&value, &["payload", "observations"])
            .as_array()
            .unwrap()
            .len(),
        8
    );
    assert_eq!(
        order(&value)
            .iter()
            .filter(|v| v.get("observation").and_then(Json::as_str) == Some("transfer:shared"))
            .count(),
        3
    );
    // For dry mass A V A' = [[5,-2],[-2,5]] and r = [-1,1].
    close(
        at(&value, &["payload", "result", "chi_square"])
            .f64()
            .unwrap(),
        2.0 / 7.0,
    );
    assert_eq!(
        at(&value, &["payload", "result", "degrees_of_freedom"]).u64(),
        Some(6)
    );
    for (key, expected) in [
        ("transfer:feed/dry", 100.0 + 1.0 / 7.0),
        ("transfer:shared/dry", 121.0 - 2.0 / 7.0),
        ("transfer:recycle/dry", 20.0 + 2.0 / 7.0),
        ("transfer:product/dry", 100.0 + 1.0 / 7.0),
        ("opening:a/dry", 10.0 + 1.0 / 7.0),
        ("closing:a/dry", 10.0 - 1.0 / 7.0),
        ("opening:b/dry", 10.0 - 1.0 / 7.0),
        ("closing:b/dry", 10.0 + 1.0 / 7.0),
        ("transfer:shared/Au", 12.0),
    ] {
        close(estimate(&value, key), expected);
    }
    let unit_rows = at(&value, &["payload", "result", "unit_conservation"])
        .as_array()
        .unwrap();
    for (unit, expected) in [("a", -1), ("b", 1)] {
        let row = unit_rows
            .iter()
            .find(|r| {
                r.get("unit").and_then(Json::as_str) == Some(unit)
                    && r.get("channel").and_then(Json::as_str) == Some("dry")
            })
            .unwrap();
        assert_eq!(exact(at(row, &["raw_residual_exact_kg"])), (expected, 1));
    }
    for row in unit_rows {
        close(at(row, &["reconciled_residual_kg"]).f64().unwrap(), 0.0);
    }
    let boundary = at(&value, &["payload", "result", "boundary"]);
    for row in at(boundary, &["conservation"]).as_array().unwrap() {
        assert_eq!(exact(at(row, &["raw_residual_exact_kg"])), (0, 1));
        close(at(row, &["reconciled_residual_kg"]).f64().unwrap(), 0.0);
    }
    assert_eq!(
        at(boundary, &["posterior_covariance_kg2"])
            .as_array()
            .unwrap()
            .len(),
        15 * 15
    );
    for field in ["posterior_covariance_kg2", "adjustment_covariance_kg2"] {
        assert_eq!(
            at(&value, &["payload", "result", field])
                .as_array()
                .unwrap()
                .len(),
            24 * 24
        );
    }
    assert_eq!(
        run(&path).2,
        first,
        "same bytes and path give identical receipt and JSON"
    );
}

#[test]
fn originals_full_error_model_sources_and_claim_taint_survive_the_fit() {
    let (status, value, _) = run(&scratch(NETWORK));
    assert_eq!(status, 0, "{value:?}");
    let digest = fmine_core::hash::sha256(NETWORK.as_bytes()).to_hex();
    assert_eq!(
        at(&value, &["payload", "source", "sha256"]).as_str(),
        Some(digest.as_str())
    );
    assert_eq!(
        at(&value, &["payload", "source", "bytes"]).u64(),
        Some(NETWORK.len() as u64)
    );
    assert_eq!(
        at(&value, &["payload", "error_model", "covariance_kg2"])
            .as_array()
            .unwrap()
            .len(),
        576
    );
    let observations = at(&value, &["payload", "observations"]).as_array().unwrap();
    let shared = observations
        .iter()
        .find(|v| v.get("observation").and_then(Json::as_str) == Some("transfer:shared"))
        .unwrap();
    let raw_dry = at(shared, &["coordinates"])
        .as_array()
        .unwrap()
        .iter()
        .find(|v| v.get("channel").and_then(Json::as_str) == Some("dry"))
        .unwrap();
    assert_eq!(at(raw_dry, &["raw_kg"]).as_str(), Some("121.000"));
    assert_eq!(exact(at(raw_dry, &["kg"])), (121, 1));
    for observation in observations {
        let sources = at(observation, &["claim", "sources"]).as_array().unwrap();
        assert_eq!(sources.len(), 1);
        assert_eq!(at(&sources[0], &["sha256"]).as_str(), Some(digest.as_str()));
        for key in order(&value)
            .iter()
            .filter(|key| key.get("observation") == observation.get("observation"))
        {
            assert_eq!(
                key.get("source_claim_id"),
                observation.get("source_claim_id")
            );
        }
    }
    let claim = at(&value, &["payload", "admitted_claim"]);
    assert_eq!(at(claim, &["evidence"]).as_str(), Some("Assumed"));
    assert_eq!(at(claim, &["state"]).as_str(), Some("Estimated"));
    assert_eq!(at(claim, &["confidentiality"]).as_str(), Some("Internal"));
    assert_eq!(at(claim, &["calibration"]).as_str(), Some("uncalibrated"));
    assert_eq!(at(claim, &["parents"]).as_array().unwrap().len(), 10);
}

#[test]
fn full_cross_channel_covariance_changes_both_channels_jointly() {
    let input = changed(
        &covariance("transfer:feed/dry", "transfer:feed/Au", "0"),
        &covariance("transfer:feed/dry", "transfer:feed/Au", "0.5"),
    );
    let (status, value, _) = run(&scratch(&input));
    assert_eq!(status, 0, "{value:?}");
    // Exact block elimination of the four dry/Au residual rows gives multipliers
    // [-252,247,30,12]/1739; the zero raw Au residual still changes its fit.
    close(
        at(&value, &["payload", "result", "chi_square"])
            .f64()
            .unwrap(),
        499.0 / 1739.0,
    );
    close(
        estimate(&value, "transfer:feed/dry"),
        100.0 + 237.0 / 1739.0,
    );
    close(estimate(&value, "transfer:feed/Au"), 10.0 + 96.0 / 1739.0);
    close(estimate(&value, "transfer:shared/Au"), 12.0 + 18.0 / 1739.0);
    let (i, j) = (
        index(&value, "transfer:feed/dry"),
        index(&value, "transfer:feed/Au"),
    );
    let matrix = at(&value, &["payload", "error_model", "covariance_kg2"])
        .as_array()
        .unwrap();
    close(matrix[i * 24 + j].f64().unwrap(), 0.5);
    close(matrix[j * 24 + i].f64().unwrap(), 0.5);
    let fitted = at(&value, &["payload", "result", "posterior_covariance_kg2"])
        .as_array()
        .unwrap();
    assert!(fitted[i * 24 + j].f64().unwrap().abs() > 0.01);
}

#[test]
fn gross_error_preserves_rejected_candidate_and_every_original() {
    let input = changed(
        &coordinate("transfer:shared", "dry", "121.000", 0, 10),
        &coordinate("transfer:shared", "dry", "130", 0, 10),
    );
    let value = refused(&input, "FMINE-REF-101");
    close(
        at(&value, &["payload", "result", "chi_square"])
            .f64()
            .unwrap(),
        200.0 / 7.0,
    );
    assert_eq!(
        at(&value, &["payload", "observations"])
            .as_array()
            .unwrap()
            .len(),
        8
    );
    assert!(
        at(&value, &["payload", "receipt_digest"])
            .as_str()
            .is_some()
    );
    assert_eq!(
        at(
            &value,
            &["payload", "error_model", "global_chi_square_limit"]
        )
        .f64(),
        Some(9.0)
    );
    for row in at(&value, &["payload", "result", "boundary", "conservation"])
        .as_array()
        .unwrap()
    {
        assert_eq!(exact(at(row, &["raw_residual_exact_kg"])), (0, 1));
    }
}

#[test]
fn physically_impossible_fitted_composition_is_retained_without_a_claim() {
    let input = changed(
        &coordinate("transfer:shared", "Au", "12", 0, 10),
        &coordinate("transfer:shared", "Au", "121", 0, 10),
    )
    .replace(
        "global_chi_square_limit = \"9\"",
        "global_chi_square_limit = \"1000000000\"",
    );
    let value = refused(&input, "FMINE-REF-109");
    assert!(
        estimate(&value, "closing:a/Au") < 0.0,
        "no silent clipping of fitted metal"
    );
    assert!(
        at(&value, &["payload", "receipt_digest"])
            .as_str()
            .is_some()
    );
    assert_eq!(
        at(&value, &["payload", "observations"])
            .as_array()
            .unwrap()
            .len(),
        8
    );
}

#[test]
fn covariance_never_defaults_missing_cross_terms_or_accepts_duplicate_pairs() {
    let pair = covariance("transfer:feed/dry", "transfer:feed/water", "0");
    refused(&changed(&pair, ""), "FMINE-REF-506");
    refused(
        &format!(
            "{NETWORK}{}",
            covariance("transfer:feed/water", "transfer:feed/dry", "0")
        ),
        "FMINE-REF-506",
    );
    refused(
        &changed(
            &pair,
            &covariance("transfer:missing/dry", "transfer:feed/water", "0"),
        ),
        "FMINE-REF-506",
    );
    refused(
        &changed(
            &pair,
            &covariance("transfer:feed/dry", "transfer:feed/water", "NaN"),
        ),
        "FMINE-REF-506",
    );
    // A nonzero declared covariance may never underflow into implicit independence.
    refused(
        &changed(
            &pair,
            &covariance("transfer:feed/dry", "transfer:feed/water", "1e-9999"),
        ),
        "FMINE-REF-207",
    );
    let explicit_zero = changed(
        &pair,
        &covariance("transfer:feed/dry", "transfer:feed/water", "0e-9999"),
    );
    let (status, zero, _) = run(&scratch(&explicit_zero));
    assert_eq!(status, 0, "{zero:?}");
    close(
        at(&zero, &["payload", "result", "chi_square"])
            .f64()
            .unwrap(),
        2.0 / 7.0,
    );
    // A fully supplied but indefinite matrix remains available for diagnosis.
    let value = refused(
        &changed(
            &pair,
            &covariance("transfer:feed/dry", "transfer:feed/water", "2"),
        ),
        "FMINE-REF-207",
    );
    assert_eq!(
        at(&value, &["payload", "error_model", "covariance_kg2"])
            .as_array()
            .unwrap()
            .len(),
        576
    );
    assert_eq!(at(&value, &["payload", "result"]), &Json::Null);
}

#[test]
fn strict_schema_and_explicit_error_policy_are_required() {
    for (old, new, code) in [
        (
            "fmine.plant-flowsheet.v1",
            "fmine.plant-flowsheet.v2",
            "FMINE-REF-506",
        ),
        (
            "kg/dry-matter+water/total-elements/all-phases/full-boundary",
            "tonnes/ownership",
            "FMINE-REF-107",
        ),
        (
            "complete = \"true\"",
            "complete = \"false\"",
            "FMINE-REF-506",
        ),
        (
            "unbiased_gaussian_errors = \"true\"",
            "unbiased_gaussian_errors = \"false\"",
            "FMINE-REF-506",
        ),
        (
            "global_chi_square_limit = \"9\"",
            "global_chi_square_limit = \"0\"",
            "FMINE-REF-109",
        ),
        ("kind = \"mill\"", "kind = \"reactor\"", "FMINE-REF-506"),
        (
            "kind = \"feed\"\nto = \"a\"",
            "kind = \"feed\"\nto = \"a\"\nfrom = \"b\"",
            "FMINE-REF-506",
        ),
        (
            "kind = \"feed\"\nto = \"a\"",
            "kind = \"feed\"\nto = \"undeclared\"",
            "FMINE-REF-105",
        ),
        (
            "[[tolerance]]\nchannel = \"water\"\nkg = \"1e-8\"",
            "",
            "FMINE-REF-506",
        ),
        ("kg = \"1e-8\"", "kg = \"-1\"", "FMINE-REF-109"),
        (
            "[[unit]]\nid = \"a\"",
            "[[unit]]\nid = \"a\"\nid = \"b\"",
            "FMINE-REF-506",
        ),
    ] {
        refused(&changed(old, new), code);
    }
    refused(
        &format!("{NETWORK}\n[[surprise]]\nvalue = \"1\"\n"),
        "FMINE-REF-506",
    );
    refused(
        &format!("{NETWORK}\n[[element]]\nsymbol = \"Au\"\nmeasurand = \"duplicate\"\n"),
        "FMINE-REF-506",
    );
}

#[test]
fn complete_population_and_physical_support_never_infer_missing_observations() {
    let shared = coordinate("transfer:shared", "Au", "12", 0, 10);
    refused(&changed(&shared, ""), "FMINE-REF-506");
    refused(&format!("{NETWORK}{shared}"), "FMINE-REF-506");
    refused(
        &changed(&shared, &coordinate("transfer:shared", "Au", "12", 0, 9)),
        "FMINE-REF-102",
    );
    let stock = coordinate("opening:a", "dry", "10", 0, 0);
    refused(
        &changed(&stock, &coordinate("opening:a", "dry", "10", 10, 10)),
        "FMINE-REF-102",
    );
    let mut missing = NETWORK.to_owned();
    for (channel, kg) in [("dry", "100"), ("water", "20"), ("Au", "10")] {
        missing = missing.replace(&coordinate("transfer:product", channel, kg, 0, 10), "");
    }
    refused(&missing, "FMINE-REF-201");
    refused(
        &changed(
            &coordinate("transfer:feed", "dry", "100", 0, 10),
            &coordinate("transfer:feed", "dry", "-1", 0, 10),
        ),
        "FMINE-REF-109",
    );
}

#[test]
fn worksheet_row_order_does_not_change_canonical_model_or_fit() {
    let mut parts = NETWORK.split("\n[[");
    let header = parts.next().unwrap();
    let mut rows: Vec<_> = parts.collect();
    rows.reverse();
    let reordered = format!("{header}\n[[{}", rows.join("\n[["));
    let (first_status, first, _) = run(&scratch(NETWORK));
    let (second_status, second, _) = run(&scratch(&reordered));
    assert_eq!((first_status, second_status), (0, 0), "{second:?}");
    assert_eq!(
        at(&first, &["payload", "result"]),
        at(&second, &["payload", "result"])
    );
    assert_eq!(
        at(&first, &["payload", "error_model", "covariance_kg2"]),
        at(&second, &["payload", "error_model", "covariance_kg2"])
    );
    assert_ne!(
        at(&first, &["payload", "source", "sha256"]),
        at(&second, &["payload", "source", "sha256"])
    );
    assert_ne!(
        at(&first, &["payload", "receipt_digest"]),
        at(&second, &["payload", "receipt_digest"])
    );
}

#[test]
fn oversized_and_missing_worksheets_fail_with_structured_null_claims() {
    let oversized = format!("#{}", "x".repeat(1_048_576));
    refused(&oversized, "FMINE-REF-506");
    let nonexistent = std::env::temp_dir().join(format!(
        "fmine-no-network-{}-{}.toml",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed)
    ));
    let (status, value, _) = run(&nonexistent);
    assert_ne!(status, 0);
    assert_eq!(
        at(&value, &["refusal", "code"]).as_str(),
        Some("FMINE-REF-506")
    );
    assert_eq!(at(&value, &["payload", "admitted_claim"]), &Json::Null);
}
