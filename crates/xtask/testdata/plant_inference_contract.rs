// FACT: independent stock-difference and constrained-recycle known answers
// exercised through the actual binary, without fabricated missing observations.
use fmine_io::omf::json::{self, Json};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

const DRAW: &str = include_str!("../../../tests/plant/flowsheet_infer.toml");
const RECYCLE: &str = include_str!("../../../tests/plant/flowsheet_reconcile.toml");
static SERIAL: AtomicUsize = AtomicUsize::new(0);

fn scratch(text: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "fmine-plant-inference-{}-{}.toml",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::write(&path, text).expect("write constructed worksheet");
    path
}
fn run_verb(path: &Path, verb: &str) -> (i32, Json, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_fmine"))
        .args(["plant", verb, "--input"])
        .arg(path)
        .arg("--json")
        .output()
        .expect("run actual fmine");
    assert!(
        output.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let text = String::from_utf8(output.stdout).unwrap();
    assert_eq!(text.lines().count(), 1, "one NDJSON envelope");
    let value = json::parse(text.as_bytes()).expect("finite valid JSON");
    let code = output.status.code().expect("normal process exit");
    assert_eq!(at(&value, &["exit_code"]).i64(), Some(i64::from(code)));
    for (_, flag) in at(&value, &["flags"]).as_object().unwrap() {
        assert_eq!(flag.as_bool(), Some(false));
    }
    (code, value, text)
}
fn run(path: &Path) -> (i32, Json, String) {
    run_verb(path, "infer")
}
fn at<'a>(value: &'a Json, path: &[&str]) -> &'a Json {
    path.iter().fold(value, |v, key| {
        v.get(key).unwrap_or_else(|| panic!("missing {key}: {v:?}"))
    })
}
fn close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() < 2e-9 * (1.0 + expected.abs()),
        "{actual} != {expected}"
    );
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
fn order(value: &Json) -> &[Json] {
    at(value, &["payload", "coordinate_order"])
        .as_array()
        .unwrap()
}
fn index(value: &Json, name: &str) -> usize {
    order(value)
        .iter()
        .position(|v| at(v, &["key"]).as_str() == Some(name))
        .unwrap()
}
fn estimate(value: &Json, key: &str) -> f64 {
    at(value, &["payload", "result", "estimates_kg"])
        .as_array()
        .unwrap()[index(value, key)]
    .f64()
    .unwrap()
}
fn posterior(value: &Json, left: &str, right: &str) -> f64 {
    let n = order(value).len();
    at(value, &["payload", "result", "posterior_covariance_kg2"])
        .as_array()
        .unwrap()[index(value, left) * n + index(value, right)]
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
    assert!(
        DRAW.contains(old),
        "mutation must target a real fixture row"
    );
    DRAW.replacen(old, new, 1)
}
fn refused(text: &str, expected: &str) -> Json {
    let (status, value, _) = run(&scratch(text));
    assert_ne!(status, 0, "{value:?}");
    assert_eq!(
        at(&value, &["refusal", "code"]).as_str(),
        Some(expected),
        "{value:?}"
    );
    assert_eq!(at(&value, &["payload", "admitted_claim"]), &Json::Null);
    value
}

// Derive a declared missing-observation input from the fixed measured fixture.
// This removes input rows only. All expected numeric answers below are independent.
fn missing_recycle(streams: &[&str]) -> String {
    let mut parts = RECYCLE.split("\n[[");
    let mut text = parts
        .next()
        .unwrap()
        .replace("fmine.plant-flowsheet.v1", "fmine.plant-unmeasured.v1");
    for block in parts {
        let omitted = streams.iter().any(|stream| {
            (block.starts_with("coordinate]]")
                && block.contains(&format!("observation = \"transfer:{stream}\"")))
                || (block.starts_with("covariance]]")
                    && block.contains(&format!("\"transfer:{stream}/")))
        });
        if !omitted {
            text.push_str("\n[[");
            text.push_str(block);
        }
    }
    for stream in streams {
        text.push_str(&format!("\n[[unmeasured]]\nstream = \"{stream}\"\n"));
    }
    text
}

#[test]
fn stock_drawdown_retains_joint_covariance_without_claiming_a_gross_error_test() {
    let path = scratch(DRAW);
    let (status, value, first) = run(&path);
    assert_eq!(status, 0, "{value:?}");
    assert_eq!(at(&value, &["verb"]).as_str(), Some("plant infer"));
    assert_eq!(
        at(&value, &["payload", "schema"]).as_str(),
        Some("fmine.plant-unmeasured-result.v1")
    );
    let result = at(&value, &["payload", "result"]);
    assert_eq!(
        at(result, &["coordinate_layout"]).as_str(),
        Some("measured_then_inferred")
    );
    assert_eq!(at(result, &["measured_count"]).i64(), Some(6));
    assert_eq!(at(result, &["inferred_count"]).i64(), Some(3));
    assert_eq!(order(&value).len(), 9);
    for (channel, expected, variance) in [
        ("dry", 30.0, 8.0),
        ("water", 15.0, 2.0),
        ("Au", 0.6, 0.0008),
    ] {
        let key = format!("transfer:product/{channel}");
        close(estimate(&value, &key), expected);
        close(posterior(&value, &key, &key), variance);
        let inferred = &order(&value)[index(&value, &key)];
        assert_eq!(at(inferred, &["kind"]).as_str(), Some("inferred"));
        assert_eq!(at(inferred, &["source_claim_id"]), &Json::Null);
    }
    close(
        posterior(&value, "transfer:product/dry", "transfer:product/Au"),
        0.02,
    );
    close(
        posterior(&value, "transfer:product/dry", "opening:store/dry"),
        4.0,
    );
    close(
        posterior(&value, "transfer:product/dry", "closing:store/dry"),
        -4.0,
    );
    assert_eq!(
        at(result, &["posterior_covariance_kg2"])
            .as_array()
            .unwrap()
            .len(),
        81
    );
    assert_eq!(
        at(result, &["measured_adjustment_covariance_kg2"])
            .as_array()
            .unwrap()
            .len(),
        36
    );
    for value in at(result, &["measured_adjustments_kg"]).as_array().unwrap() {
        close(value.f64().unwrap(), 0.0);
    }
    for value in at(result, &["normalized_adjustments"]).as_array().unwrap() {
        assert_eq!(value, &Json::Null);
    }
    assert_eq!(
        at(result, &["gross_error", "status"]).as_str(),
        Some("no_redundancy")
    );
    assert_eq!(at(result, &["gross_error", "chi_square"]), &Json::Null);
    assert_eq!(
        at(result, &["gross_error", "degrees_of_freedom"]).i64(),
        Some(0)
    );
    assert_eq!(run(&path).2, first);
    let tiny_policy = changed(
        "global_chi_square_limit = \"9\"",
        "global_chi_square_limit = \"1e-20\"",
    );
    let (status, tiny, _) = run(&scratch(&tiny_policy));
    assert_eq!(status, 0, "{tiny:?}");
    assert_eq!(
        at(&tiny, &["payload", "result", "gross_error", "chi_square"]),
        &Json::Null
    );
}

#[test]
fn boundary_missing_totals_are_distinct_from_observed_contributions_and_structural_zeros() {
    let (status, value, _) = run(&scratch(DRAW));
    assert_eq!(status, 0, "{value:?}");
    let boundary = at(&value, &["payload", "result", "boundary"]);
    let keys = at(boundary, &["order"]).as_array().unwrap();
    let position = |role: &str, channel: &str| {
        keys.iter()
            .position(|key| {
                at(key, &["role"]).as_str() == Some(role)
                    && at(key, &["channel"]).as_str() == Some(channel)
            })
            .unwrap()
    };
    let covariance = at(boundary, &["posterior_covariance_kg2"])
        .as_array()
        .unwrap();
    assert_eq!(keys.len(), 15);
    assert_eq!(covariance.len(), 15 * 15);
    let product_dry = position("product", "dry");
    let boundary_covariance =
        |left: usize, right: usize| covariance[left * keys.len() + right].f64().unwrap();
    close(boundary_covariance(product_dry, product_dry), 8.0);
    close(
        boundary_covariance(product_dry, position("product", "Au")),
        0.02,
    );
    close(
        boundary_covariance(product_dry, position("opening", "dry")),
        4.0,
    );
    close(
        boundary_covariance(product_dry, position("closing", "dry")),
        -4.0,
    );
    close(
        boundary_covariance(product_dry, position("feed", "dry")),
        0.0,
    );
    close(
        at(boundary, &["estimates_kg"]).as_array().unwrap()[product_dry]
            .f64()
            .unwrap(),
        30.0,
    );
    for (i, key) in keys.iter().enumerate() {
        let role = at(key, &["role"]).as_str().unwrap();
        let observed = &at(boundary, &["observed_totals_kg"]).as_array().unwrap()[i];
        if role == "product" {
            assert_eq!(observed, &Json::Null);
            assert_eq!(
                exact(
                    &at(boundary, &["observed_contributions_exact_kg"])
                        .as_array()
                        .unwrap()[i]
                ),
                (0, 1)
            );
        } else if role == "feed" || role == "other_outflow" {
            close(observed.f64().unwrap(), 0.0);
        }
    }
    for row in at(&value, &["payload", "result", "unit_conservation"])
        .as_array()
        .unwrap()
    {
        assert_eq!(at(row, &["has_unmeasured_terms"]).as_bool(), Some(true));
        assert!(row.get("raw_residual_exact_kg").is_none());
        close(at(row, &["reconciled_residual_kg"]).f64().unwrap(), 0.0);
    }
    let dry = at(&value, &["payload", "result", "unit_conservation"])
        .as_array()
        .unwrap()
        .iter()
        .find(|row| at(row, &["channel"]).as_str() == Some("dry"))
        .unwrap();
    assert_eq!(exact(at(dry, &["observed_contribution_exact_kg"])), (30, 1));
    for row in at(boundary, &["conservation"]).as_array().unwrap() {
        close(at(row, &["reconciled_residual_kg"]).f64().unwrap(), 0.0);
    }
}

#[test]
fn recycle_inference_matches_an_independent_redundant_balance_oracle() {
    let input = missing_recycle(&["product"]);
    let (status, value, _) = run(&scratch(&input));
    assert_eq!(status, 0, "{value:?}");
    // A's only remaining dry residual is -1 with variance 5. The five measured
    // terms move by +/-1/5. B then infers shared - recycle + opening - closing.
    close(estimate(&value, "transfer:feed/dry"), 100.2);
    close(estimate(&value, "transfer:shared/dry"), 120.8);
    close(estimate(&value, "transfer:recycle/dry"), 20.2);
    close(estimate(&value, "transfer:product/dry"), 100.6);
    close(estimate(&value, "transfer:product/water"), 20.0);
    close(estimate(&value, "transfer:product/Au"), 10.0);
    // Var(B's inferred product) = 4 - (-2)^2/5 = 16/5; Cov(feed,product)=2/5.
    close(
        posterior(&value, "transfer:product/dry", "transfer:product/dry"),
        16.0 / 5.0,
    );
    close(
        posterior(&value, "transfer:feed/dry", "transfer:product/dry"),
        2.0 / 5.0,
    );
    let gross = at(&value, &["payload", "result", "gross_error"]);
    assert_eq!(at(gross, &["status"]).as_str(), Some("available"));
    close(at(gross, &["chi_square"]).f64().unwrap(), 1.0 / 5.0);
    assert_eq!(at(gross, &["degrees_of_freedom"]).i64(), Some(3));
    assert_eq!(
        at(&value, &["payload", "result", "measured_count"]).i64(),
        Some(21)
    );
    assert_eq!(order(&value).len(), 24);
    assert_eq!(
        at(&value, &["payload", "error_model", "covariance_kg2"])
            .as_array()
            .unwrap()
            .len(),
        21 * 21
    );
    for row in at(&value, &["payload", "result", "unit_conservation"])
        .as_array()
        .unwrap()
    {
        close(at(row, &["reconciled_residual_kg"]).f64().unwrap(), 0.0);
    }
}

#[test]
fn unobservable_parallel_outlets_and_internal_cycles_refuse_without_fabricating_a_fit() {
    let parallel = format!(
        "{DRAW}\n[[stream]]\nid = \"tails\"\nkind = \"other_outflow\"\nfrom = \"store\"\n\n[[unmeasured]]\nstream = \"tails\"\n"
    );
    for text in [parallel, missing_recycle(&["shared", "recycle"])] {
        let value = refused(&text, "FMINE-REF-104");
        assert_eq!(at(&value, &["payload", "result"]), &Json::Null);
        assert_eq!(at(&value, &["payload", "receipt_digest"]), &Json::Null);
        assert!(
            at(&value, &["payload", "observations"])
                .as_array()
                .unwrap()
                .len()
                >= 2
        );
        assert!(
            at(&value, &["payload", "error_model", "covariance_kg2"])
                .as_array()
                .is_some()
        );
    }
}

#[test]
fn explicit_omissions_never_allow_missing_stocks_fake_observations_or_unknown_streams() {
    refused(
        &changed("[[unmeasured]]\nstream = \"product\"", ""),
        "FMINE-REF-201",
    );
    refused(
        &changed(
            "[[unmeasured]]\nstream = \"product\"",
            "[[unmeasured]]\nstream = \"absent\"",
        ),
        "FMINE-REF-105",
    );
    refused(
        &format!("{DRAW}\n[[unmeasured]]\nstream = \"product\"\n"),
        "FMINE-REF-506",
    );
    refused(
        &changed(
            "[[unmeasured]]\nstream = \"product\"",
            "[[unmeasured]]\nstream = \"product\"\nvariance = \"9999999\"",
        ),
        "FMINE-REF-506",
    );
    let mut measured = DRAW.to_string();
    for (channel, kg) in [("dry", "30"), ("water", "15"), ("Au", "0.6")] {
        measured.push_str(&coordinate("transfer:product", channel, kg, 0, 10));
    }
    refused(&measured, "FMINE-REF-105");
    refused(
        &format!(
            "{DRAW}{}",
            covariance("transfer:product/dry", "opening:store/dry", "0")
        ),
        "FMINE-REF-506",
    );
    let mut missing_stock = DRAW.to_string();
    for (channel, kg) in [("dry", "70"), ("water", "25"), ("Au", "1.4")] {
        missing_stock =
            missing_stock.replace(&coordinate("closing:store", channel, kg, 10, 10), "");
    }
    refused(&missing_stock, "FMINE-REF-201");
    refused(
        &changed(&coordinate("closing:store", "water", "25", 10, 10), ""),
        "FMINE-REF-506",
    );
    refused(
        &changed(
            &coordinate("closing:store", "water", "25", 10, 10),
            &coordinate("closing:store", "water", "25", 9, 9),
        ),
        "FMINE-REF-102",
    );
}

#[test]
fn covariance_and_error_policy_are_complete_explicit_and_range_checked() {
    let pair = covariance("opening:store/dry", "opening:store/water", "0");
    refused(&changed(&pair, ""), "FMINE-REF-506");
    refused(
        &format!(
            "{DRAW}{}",
            covariance("opening:store/water", "opening:store/dry", "0")
        ),
        "FMINE-REF-506",
    );
    for (token, code) in [("NaN", "FMINE-REF-506"), ("1e-9999", "FMINE-REF-207")] {
        refused(
            &changed(
                &pair,
                &covariance("opening:store/dry", "opening:store/water", token),
            ),
            code,
        );
    }
    let zero = changed(
        &pair,
        &covariance("opening:store/dry", "opening:store/water", "0e-9999"),
    );
    assert_eq!(run(&scratch(&zero)).0, 0);
    let indefinite = changed(
        &covariance("opening:store/dry", "opening:store/Au", "0.02"),
        &covariance("opening:store/dry", "opening:store/Au", "1"),
    );
    let value = refused(&indefinite, "FMINE-REF-207");
    assert_eq!(
        at(&value, &["payload", "error_model", "covariance_kg2"])
            .as_array()
            .unwrap()
            .len(),
        36
    );
    for (old, new, code) in [
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
        ("kg = \"1e-9\"", "kg = \"-1\"", "FMINE-REF-109"),
        (
            "[[tolerance]]\nchannel = \"Au\"\nkg = \"1e-10\"",
            "",
            "FMINE-REF-506",
        ),
    ] {
        refused(&changed(old, new), code);
    }
}

#[test]
fn negative_inferred_quantity_retains_rejected_diagnostics_and_original_evidence() {
    let input = changed(
        &coordinate("closing:store", "dry", "70", 10, 10),
        &coordinate("closing:store", "dry", "110", 10, 10),
    );
    let value = refused(&input, "FMINE-REF-109");
    close(estimate(&value, "transfer:product/dry"), -10.0);
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
        2
    );
    assert_eq!(
        at(&value, &["payload", "result", "gross_error", "chi_square"]),
        &Json::Null
    );
    assert_eq!(
        at(&value, &["payload", "result", "posterior_covariance_kg2"])
            .as_array()
            .unwrap()
            .len(),
        81
    );
}

#[test]
fn global_and_adjustment_rejection_policies_preserve_the_inferred_candidate() {
    let input = missing_recycle(&["product"]);
    for policy in [
        "global_chi_square_limit = \"0.1\"",
        "global_chi_square_limit = \"9\"\nnormalized_adjustment_limit = \"0.1\"",
    ] {
        let value = refused(
            &input.replace("global_chi_square_limit = \"9\"", policy),
            "FMINE-REF-101",
        );
        close(estimate(&value, "transfer:product/dry"), 100.6);
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
            7
        );
        close(
            at(&value, &["payload", "result", "gross_error", "chi_square"])
                .f64()
                .unwrap(),
            0.2,
        );
    }
}

#[test]
fn raw_tokens_source_identity_and_claim_colour_survive_order_and_byte_changes() {
    let path = scratch(DRAW);
    let (_, first, _) = run(&path);
    let source_hash = at(&first, &["payload", "source", "sha256"])
        .as_str()
        .unwrap();
    assert_eq!(source_hash.len(), 64);
    let observations = at(&first, &["payload", "observations"]).as_array().unwrap();
    let opening = observations
        .iter()
        .find(|v| at(v, &["observation"]).as_str() == Some("opening:store"))
        .unwrap();
    let dry = at(opening, &["coordinates"])
        .as_array()
        .unwrap()
        .iter()
        .find(|v| at(v, &["channel"]).as_str() == Some("dry"))
        .unwrap();
    assert_eq!(at(dry, &["raw_kg"]).as_str(), Some("100.000"));
    assert_eq!(exact(at(dry, &["kg"])), (100, 1));
    for claim in observations.iter().map(|v| at(v, &["claim"])).chain([
        at(&first, &["payload", "graph", "claim"]),
        at(&first, &["payload", "error_model", "claim"]),
    ]) {
        assert_eq!(at(claim, &["evidence"]).as_str(), Some("Assumed"));
        assert_eq!(at(claim, &["confidentiality"]).as_str(), Some("Internal"));
        assert_eq!(at(claim, &["calibration"]).as_str(), Some("uncalibrated"));
        for anchor in at(claim, &["sources"]).as_array().unwrap() {
            assert_eq!(at(anchor, &["sha256"]).as_str(), Some(source_hash));
        }
    }
    let admitted = at(&first, &["payload", "admitted_claim"]);
    assert_eq!(at(admitted, &["evidence"]).as_str(), Some("Assumed"));
    assert_eq!(at(admitted, &["state"]).as_str(), Some("Estimated"));
    assert_eq!(at(admitted, &["parents"]).as_array().unwrap().len(), 4);
    std::fs::write(&path, format!("{DRAW}\n# retained source-byte mutation\n")).unwrap();
    let (_, changed_bytes, _) = run(&path);
    assert_ne!(
        at(&first, &["payload", "source", "sha256"]),
        at(&changed_bytes, &["payload", "source", "sha256"])
    );
    assert_ne!(
        at(&first, &["payload", "receipt_digest"]),
        at(&changed_bytes, &["payload", "receipt_digest"])
    );
    assert_eq!(
        at(&first, &["payload", "result"]),
        at(&changed_bytes, &["payload", "result"])
    );
    let mut blocks = DRAW.split("\n[[");
    let top = blocks.next().unwrap();
    let mut rows: Vec<_> = blocks.collect();
    rows.reverse();
    let permuted = format!("{top}\n[[{}", rows.join("\n[["));
    let (status, reordered, _) = run(&scratch(&permuted));
    assert_eq!(status, 0, "{reordered:?}");
    assert_eq!(
        at(&first, &["payload", "result"]),
        at(&reordered, &["payload", "result"])
    );
    assert_eq!(
        at(&first, &["payload", "error_model", "covariance_kg2"]),
        at(&reordered, &["payload", "error_model", "covariance_kg2"])
    );
    let numeric = changed("kg = \"100.000\"", "kg = \"101.000\"");
    let (status, changed_numeric, _) = run(&scratch(&numeric));
    assert_eq!(status, 0, "{changed_numeric:?}");
    close(estimate(&changed_numeric, "transfer:product/dry"), 31.0);
}

#[test]
fn inference_is_explicit_in_robot_docs_command_schema_and_file_refusals() {
    let output = Command::new(env!("CARGO_BIN_EXE_fmine"))
        .arg("robot-docs")
        .output()
        .unwrap();
    assert!(output.status.success());
    let docs = json::parse(&output.stdout).unwrap();
    assert_eq!(
        at(&docs, &["payload", "plant_inference", "schema"]).as_str(),
        Some("fmine.plant-unmeasured.v1")
    );
    assert!(
        at(&docs, &["payload", "plant_inference", "gross_error"])
            .as_str()
            .unwrap()
            .contains("no_redundancy")
    );
    let path = scratch(DRAW);
    let (code, value, _) = run_verb(&path, "flowsheet");
    assert_ne!(code, 0);
    assert_eq!(
        at(&value, &["refusal", "code"]).as_str(),
        Some("FMINE-REF-506")
    );
    let undeclared = DRAW
        .replace("fmine.plant-unmeasured.v1", "fmine.plant-flowsheet.v1")
        .replace("[[unmeasured]]\nstream = \"product\"", "");
    let (code, value, _) = run_verb(&scratch(&undeclared), "flowsheet");
    assert_ne!(code, 0);
    assert_eq!(
        at(&value, &["refusal", "code"]).as_str(),
        Some("FMINE-REF-201")
    );
    let missing = path.with_extension("missing");
    let (code, value, _) = run(&missing);
    assert_ne!(code, 0);
    assert_eq!(
        at(&value, &["refusal", "code"]).as_str(),
        Some("FMINE-REF-506")
    );
    assert_eq!(at(&value, &["payload", "result"]), &Json::Null);
    let oversized = "#".repeat(1024 * 1024 + 1);
    refused(&oversized, "FMINE-REF-506");
}
