//! SYNTHETIC endpoint-inventory answers derived from conservation and covariance.
//! Inferred stocks remain estimates, with no invented observation or source Claim.
//! Determinism: DeterministicPerIsa

#![cfg(test)]
#![allow(clippy::expect_used)]

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

use fmine_cli::{Envelope, OutputFormat, invoke};
use fmine_core::hash::sha256;
use fmine_io::omf::json::{Json, parse};

const CLOSING: &str = include_str!("../../../tests/plant/inventory_infer.toml");
const SHARED: &str = include_str!("../../../tests/plant/inventory_infer_transfer.toml");
const V1: &str = include_str!("../../../tests/plant/flowsheet_infer.toml");
static NEXT: AtomicUsize = AtomicUsize::new(0);

fn scratch() -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "fmine-plant-inventory-inference-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed),
    ));
    std::fs::create_dir_all(&path).expect("test directory");
    path
}
fn at_root(root: &Path, text: &str) -> (Envelope, Json) {
    std::fs::write(root.join("inventory.toml"), text).expect("exact worksheet bytes");
    let args = ["plant", "infer", "--input", "inventory.toml", "--json"].map(str::to_owned);
    let (envelope, format) = invoke(&args, root);
    assert_eq!(format, OutputFormat::Json);
    assert_eq!(envelope.to_json(), invoke(&args, root).0.to_json());
    let json = parse(envelope.payload_json.as_bytes()).expect("valid payload JSON");
    (envelope, json)
}
fn evaluate(text: &str) -> (Envelope, Json) {
    at_root(&scratch(), text)
}
fn at<'a>(value: &'a Json, path: &[&str]) -> &'a Json {
    path.iter()
        .fold(value, |value, key| value.get(key).expect(key))
}
fn array(value: &Json) -> &[Json] {
    value.as_array().expect("array")
}
fn number(value: &Json) -> f64 {
    value.f64().expect("finite number")
}
fn order(value: &Json) -> &[Json] {
    array(at(value, &["coordinate_order"]))
}
fn index(value: &Json, name: &str) -> usize {
    order(value)
        .iter()
        .position(|key| at(key, &["key"]).as_str() == Some(name))
        .expect("named coordinate")
}
fn estimate(value: &Json, name: &str) -> f64 {
    number(&array(at(value, &["result", "estimates_kg"]))[index(value, name)])
}
fn posterior(value: &Json, left: &str, right: &str) -> f64 {
    number(
        &array(at(value, &["result", "posterior_covariance_kg2"]))
            [index(value, left) * order(value).len() + index(value, right)],
    )
}
fn boundary_index(value: &Json, role: &str, channel: &str) -> usize {
    array(at(value, &["result", "boundary", "order"]))
        .iter()
        .position(|key| {
            at(key, &["role"]).as_str() == Some(role)
                && at(key, &["channel"]).as_str() == Some(channel)
        })
        .expect("boundary coordinate")
}
fn close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() <= 1e-10 * (1.0 + expected.abs()),
        "{actual} != {expected}"
    );
}
fn admitted(envelope: &Envelope) {
    assert_eq!(envelope.exit_code, 0, "{}", envelope.to_json());
    assert_eq!(envelope.status, "ok");
    assert!(envelope.certificate.is_none());
    assert!(
        envelope
            .no_claim
            .expect("conditional scope")
            .contains("uncalibrated")
    );
    let json = parse(envelope.to_json().as_bytes()).expect("envelope JSON");
    for (_, flag) in at(&json, &["flags"]).as_object().expect("flags") {
        assert_eq!(flag.as_bool(), Some(false));
    }
}
fn refused(text: &str, code: &str) -> Json {
    let (envelope, value) = evaluate(text);
    assert_ne!(envelope.exit_code, 0, "{}", envelope.to_json());
    assert_eq!(envelope.refusal.as_ref().expect("refusal").code, code);
    assert_eq!(at(&value, &["admitted_claim"]), &Json::Null);
    value
}
fn changed(text: &str, old: &str, new: &str) -> String {
    assert!(
        text.contains(old),
        "mutation must target a real fixture field"
    );
    text.replacen(old, new, 1)
}
fn coordinate(observation: &str, channel: &str, kg: &str, start: i64, end: i64) -> String {
    format!(
        "\n[[coordinate]]\nobservation = \"{observation}\"\nchannel = \"{channel}\"\nkg = \"{kg}\"\nstart_ns = \"{start}\"\nend_ns = \"{end}\"\n"
    )
}
fn omit_observation(text: &str, observation: &str) -> String {
    let mut blocks = text.split("\n[[");
    let mut result = blocks.next().expect("worksheet header").to_owned();
    for block in blocks {
        let omitted = (block.starts_with("coordinate]]")
            && block.contains(&format!("observation = \"{observation}\"")))
            || (block.starts_with("covariance]]") && block.contains(&format!("\"{observation}/")));
        if !omitted {
            result.push_str("\n[[");
            result.push_str(block);
        }
    }
    result
}
fn no_redundancy(value: &Json) {
    let result = at(value, &["result"]);
    assert_eq!(
        at(result, &["gross_error", "status"]).as_str(),
        Some("no_redundancy")
    );
    assert_eq!(at(result, &["gross_error", "chi_square"]), &Json::Null);
    assert_eq!(
        at(result, &["gross_error", "degrees_of_freedom"]).i64(),
        Some(0)
    );
    for value in array(at(result, &["measured_adjustments_kg"])) {
        close(number(value), 0.0);
    }
    for row in array(at(result, &["unit_conservation"])) {
        assert_eq!(at(row, &["has_unmeasured_terms"]).as_bool(), Some(true));
        close(number(at(row, &["reconciled_residual_kg"])), 0.0);
    }
    for row in array(at(result, &["boundary", "conservation"])) {
        close(number(at(row, &["reconciled_residual_kg"])), 0.0);
    }
}

#[test]
fn closing_inventory_matches_independent_sum_and_joint_covariance() {
    let (envelope, value) = evaluate(CLOSING);
    admitted(&envelope);
    assert_eq!(
        at(&value, &["schema"]).as_str(),
        Some("fmine.plant-unmeasured-result.v2")
    );
    assert_eq!(at(&value, &["result", "measured_count"]).i64(), Some(9));
    assert_eq!(at(&value, &["result", "inferred_count"]).i64(), Some(3));
    assert_eq!(
        array(at(&value, &["error_model", "covariance_kg2"])).len(),
        81
    );
    assert_eq!(
        array(at(&value, &["result", "posterior_covariance_kg2"])).len(),
        144
    );
    for (channel, mean, variance) in [
        ("dry", 110.0, 6.0),
        ("water", 55.0, 1.5),
        ("Au", 1.1, 0.0006),
    ] {
        let key = format!("closing:store/{channel}");
        close(estimate(&value, &key), mean);
        close(posterior(&value, &key, &key), variance);
    }
    close(
        posterior(&value, "closing:store/dry", "closing:store/Au"),
        0.02,
    );
    close(
        posterior(&value, "closing:store/dry", "opening:store/dry"),
        4.0,
    );
    close(
        posterior(&value, "closing:store/dry", "transfer:feed/dry"),
        1.0,
    );
    close(
        posterior(&value, "closing:store/dry", "transfer:product/dry"),
        -1.0,
    );
    no_redundancy(&value);
}

#[test]
fn shared_transfer_creates_negative_stock_covariance_and_cancels_at_boundary() {
    let (envelope, value) = evaluate(SHARED);
    admitted(&envelope);
    assert_eq!(at(&value, &["result", "inferred_count"]).i64(), Some(6));
    let boundary = at(&value, &["result", "boundary"]);
    let width = array(at(boundary, &["order"])).len();
    for (channel, mean, va, vb, cross, total_var) in [
        ("dry", 70.0, 5.0, 10.0, -1.0, 13.0),
        ("water", 35.0, 1.25, 2.5, -0.25, 3.25),
        ("Au", 0.7, 0.0005, 0.001, -0.0001, 0.0013),
    ] {
        let a = format!("closing:a/{channel}");
        let b = format!("closing:b/{channel}");
        close(estimate(&value, &a), mean);
        close(estimate(&value, &b), mean);
        close(posterior(&value, &a, &a), va);
        close(posterior(&value, &b, &b), vb);
        close(posterior(&value, &a, &b), cross);
        let closing = boundary_index(&value, "closing", channel);
        close(
            number(&array(at(boundary, &["estimates_kg"]))[closing]),
            2.0 * mean,
        );
        close(
            number(&array(at(boundary, &["posterior_covariance_kg2"]))[closing * width + closing]),
            total_var,
        );
        assert_eq!(
            &array(at(boundary, &["observed_totals_kg"]))[closing],
            &Json::Null
        );
        for role in ["feed", "product", "other_outflow"] {
            let position = boundary_index(&value, role, channel);
            close(
                number(&array(at(boundary, &["observed_totals_kg"]))[position]),
                0.0,
            );
            close(
                number(&array(at(boundary, &["estimates_kg"]))[position]),
                0.0,
            );
        }
    }
    no_redundancy(&value);
}

#[test]
fn opening_inventory_uses_reverse_balance_signs() {
    let input = CLOSING
        .replace("opening:store", "closing:store")
        .replace("kind = \"closing\"", "kind = \"opening\"")
        .replace(
            "start_ns = \"0\"\nend_ns = \"0\"",
            "start_ns = \"10\"\nend_ns = \"10\"",
        );
    let (envelope, value) = evaluate(&input);
    admitted(&envelope);
    for (channel, mean, variance) in [
        ("dry", 90.0, 6.0),
        ("water", 45.0, 1.5),
        ("Au", 0.9, 0.0006),
    ] {
        let key = format!("opening:store/{channel}");
        close(estimate(&value, &key), mean);
        close(posterior(&value, &key, &key), variance);
        let i = boundary_index(&value, "opening", channel);
        assert_eq!(
            &array(at(&value, &["result", "boundary", "observed_totals_kg"]))[i],
            &Json::Null
        );
    }
    close(
        posterior(&value, "opening:store/dry", "transfer:feed/dry"),
        -1.0,
    );
    close(
        posterior(&value, "opening:store/dry", "transfer:product/dry"),
        1.0,
    );
    no_redundancy(&value);
}

#[test]
fn inferred_stock_is_not_an_observation_or_a_fabricated_source_claim() {
    let (envelope, value) = evaluate(CLOSING);
    admitted(&envelope);
    let hash = sha256(CLOSING.as_bytes()).to_hex();
    assert_eq!(
        at(&value, &["source", "sha256"]).as_str(),
        Some(hash.as_str())
    );
    let observations = array(at(&value, &["observations"]));
    assert_eq!(observations.len(), 3);
    for row in observations {
        assert_ne!(at(row, &["observation"]).as_str(), Some("closing:store"));
    }
    let mut claims = vec![
        at(&value, &["graph", "claim"]),
        at(&value, &["error_model", "claim"]),
    ];
    claims.extend(observations.iter().map(|row| at(row, &["claim"])));
    let output = at(&value, &["admitted_claim"]);
    let parents = array(at(output, &["parents"]));
    for claim in &claims {
        assert!(parents.contains(at(claim, &["id"])));
    }
    claims.push(output);
    for claim in claims {
        assert_eq!(at(claim, &["evidence"]).as_str(), Some("Assumed"));
        assert_eq!(at(claim, &["confidentiality"]).as_str(), Some("Internal"));
        assert_eq!(at(claim, &["calibration"]).as_str(), Some("uncalibrated"));
        let sources = array(at(claim, &["sources"]));
        assert!(!sources.is_empty());
        for source in sources {
            assert_eq!(at(source, &["sha256"]).as_str(), Some(hash.as_str()));
            assert!(
                !at(source, &["locator"])
                    .as_str()
                    .expect("locator")
                    .is_empty()
            );
        }
    }
    for row in order(&value) {
        if at(row, &["observation"]).as_str() == Some("closing:store") {
            assert_eq!(at(row, &["kind"]).as_str(), Some("inferred"));
            assert_eq!(at(row, &["source_claim_id"]), &Json::Null);
        } else {
            assert_eq!(at(row, &["kind"]).as_str(), Some("measured"));
            assert!(at(row, &["source_claim_id"]).as_str().is_some());
        }
    }
    for channel in ["dry", "water", "Au"] {
        let i = boundary_index(&value, "closing", channel);
        let boundary = at(&value, &["result", "boundary"]);
        assert_eq!(
            &array(at(boundary, &["observed_totals_kg"]))[i],
            &Json::Null
        );
        let contribution = &array(at(boundary, &["observed_contributions_exact_kg"]))[i];
        assert_eq!(at(contribution, &["numerator"]).as_str(), Some("0"));
        assert_eq!(at(contribution, &["denominator"]).as_str(), Some("1"));
    }
}

#[test]
fn v1_stream_inference_remains_compatible_and_v2_also_accepts_streams() {
    let (legacy, old) = evaluate(V1);
    admitted(&legacy);
    assert_eq!(
        at(&old, &["schema"]).as_str(),
        Some("fmine.plant-unmeasured-result.v1")
    );
    let input = V1.replace("fmine.plant-unmeasured.v1", "fmine.plant-unmeasured.v2");
    let (current, new) = evaluate(&input);
    admitted(&current);
    for (channel, mean, variance) in [
        ("dry", 30.0, 8.0),
        ("water", 15.0, 2.0),
        ("Au", 0.6, 0.0008),
    ] {
        let key = format!("transfer:product/{channel}");
        close(estimate(&old, &key), mean);
        close(estimate(&new, &key), mean);
        close(posterior(&old, &key, &key), variance);
        close(posterior(&new, &key, &key), variance);
    }
    let old_schema = CLOSING.replace("fmine.plant-unmeasured.v2", "fmine.plant-unmeasured.v1");
    refused(&old_schema, "FMINE-REF-506");
}

#[test]
fn admission_requires_explicit_endpoints_and_covariance_only_for_observations() {
    let missing = "[[unmeasured_stocktake]]\nunit = \"store\"\nkind = \"closing\"";
    for (old, new, code) in [
        (missing, "", "FMINE-REF-201"),
        (
            missing,
            "[[unmeasured_stocktake]]\nunit = \"absent\"\nkind = \"closing\"",
            "FMINE-REF-105",
        ),
        ("kind = \"closing\"", "kind = \"midpoint\"", "FMINE-REF-506"),
        (
            "kind = \"closing\"",
            "kind = \"closing\"\nvariance = \"9999\"",
            "FMINE-REF-506",
        ),
    ] {
        let value = refused(&changed(CLOSING, old, new), code);
        assert_eq!(at(&value, &["result"]), &Json::Null);
        assert_eq!(at(&value, &["receipt_digest"]), &Json::Null);
    }
    refused(&format!("{CLOSING}\n{missing}\n"), "FMINE-REF-506");
    let mut contradictory = CLOSING.to_owned();
    for (channel, kg) in [("dry", "110"), ("water", "55"), ("Au", "1.1")] {
        contradictory.push_str(&coordinate("closing:store", channel, kg, 10, 10));
    }
    refused(&contradictory, "FMINE-REF-105");
    refused(
        &format!(
            "{CLOSING}\n[[covariance]]\nleft = \"closing:store/dry\"\nright = \"opening:store/dry\"\nkg2 = \"0\"\n"
        ),
        "FMINE-REF-506",
    );
    refused(
        &changed(
            CLOSING,
            &coordinate("opening:store", "water", "50", 0, 0),
            "",
        ),
        "FMINE-REF-506",
    );
}

#[test]
fn rank_failures_never_make_an_arbitrary_inventory_or_an_invented_prior() {
    let two_endpoints = format!(
        "{}\n[[unmeasured_stocktake]]\nunit = \"store\"\nkind = \"opening\"\n",
        omit_observation(CLOSING, "opening:store"),
    );
    let missing_product = format!(
        "{}\n[[unmeasured]]\nstream = \"product\"\n",
        omit_observation(CLOSING, "transfer:product"),
    );
    for text in [two_endpoints, missing_product] {
        let value = refused(&text, "FMINE-REF-104");
        assert_eq!(at(&value, &["result"]), &Json::Null);
        assert_eq!(at(&value, &["receipt_digest"]), &Json::Null);
    }
}

#[test]
fn physical_failures_preserve_unclipped_inventory_and_rejected_receipt() {
    for (product, closing) in [("130", -10.0), ("119.5", 0.5)] {
        let text = changed(
            CLOSING,
            &coordinate("transfer:product", "dry", "10", 0, 10),
            &coordinate("transfer:product", "dry", product, 0, 10),
        );
        let value = refused(&text, "FMINE-REF-109");
        close(estimate(&value, "closing:store/dry"), closing);
        assert!(at(&value, &["receipt_digest"]).as_str().is_some());
        assert_eq!(array(at(&value, &["observations"])).len(), 3);
        assert_eq!(
            array(at(&value, &["result", "posterior_covariance_kg2"])).len(),
            144
        );
        assert_eq!(
            at(&value, &["result", "gross_error", "chi_square"]),
            &Json::Null
        );
    }
}

#[test]
fn equivalent_source_tokens_change_receipt_identity_without_changing_inference() {
    let root = scratch();
    let (first, a) = at_root(&root, CLOSING);
    let equivalent = changed(CLOSING, "100.000", "100.0");
    let (second, b) = at_root(&root, &equivalent);
    admitted(&first);
    admitted(&second);
    assert_ne!(at(&a, &["source", "sha256"]), at(&b, &["source", "sha256"]));
    assert_ne!(at(&a, &["receipt_digest"]), at(&b, &["receipt_digest"]));
    for key in [
        "estimates_kg",
        "posterior_covariance_kg2",
        "measured_adjustments_kg",
        "boundary",
        "gross_error",
    ] {
        assert_eq!(at(&a, &["result", key]), at(&b, &["result", key]));
    }
}

fn binary() -> PathBuf {
    if let Some(path) = std::env::var_os("FMINE_TEST_BINARY") {
        return PathBuf::from(path);
    }
    let executable = std::env::current_exe().expect("current test executable");
    let profile = executable
        .ancestors()
        .find(|path| {
            path.file_name()
                .is_some_and(|name| name == "deps" || name == "build")
        })
        .and_then(Path::parent)
        .expect("Cargo profile directory; otherwise set FMINE_TEST_BINARY");
    profile.join(format!("fmine{}", std::env::consts::EXE_SUFFIX))
}

#[test]
fn real_binary_matches_library_for_admitted_and_refused_inventory_worksheets() {
    let negative = changed(
        CLOSING,
        &coordinate("transfer:product", "dry", "10", 0, 10),
        &coordinate("transfer:product", "dry", "130", 0, 10),
    );
    let unknown_field = changed(
        CLOSING,
        "complete = \"true\"",
        "complete = \"true\"\nunknown = \"1\"",
    );
    for (text, closing, refusal) in [
        (CLOSING, Some(110.0), None),
        (negative.as_str(), Some(-10.0), Some("FMINE-REF-109")),
        (unknown_field.as_str(), None, Some("FMINE-REF-506")),
    ] {
        let root = scratch();
        let (expected, _) = at_root(&root, text);
        let output = Command::new(binary())
            .args([
                "plant",
                "infer",
                "--input",
                "inventory.toml",
                "--json",
                "--workspace",
            ])
            .arg(&root)
            .output()
            .expect(
                "execute actual inventory inference command; build fmine or set FMINE_TEST_BINARY",
            );
        assert!(
            output.stderr.is_empty(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(output.status.code(), Some(i32::from(expected.exit_code)));
        let stdout = String::from_utf8(output.stdout).expect("UTF-8 NDJSON");
        assert_eq!(stdout.lines().count(), 1);
        assert_eq!(stdout, format!("{}\n", expected.to_json()));
        let envelope = parse(stdout.as_bytes()).expect("finite valid NDJSON envelope");
        let payload = at(&envelope, &["payload"]);
        assert_eq!(
            at(payload, &["schema"]).as_str(),
            Some("fmine.plant-unmeasured-result.v2")
        );
        match refusal {
            Some(code) => {
                assert_ne!(expected.exit_code, 0);
                assert_eq!(at(&envelope, &["refusal", "code"]).as_str(), Some(code));
                assert_eq!(at(payload, &["admitted_claim"]), &Json::Null);
            }
            None => {
                assert_eq!(expected.exit_code, 0);
                assert_eq!(at(&envelope, &["status"]).as_str(), Some("ok"));
            }
        }
        if let Some(closing) = closing {
            close(estimate(payload, "closing:store/dry"), closing);
        } else {
            assert_eq!(at(payload, &["result"]), &Json::Null);
        }
    }
}
