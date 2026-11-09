//! SYNTHETIC known answers for source-bound passive RTD command transport.
//! One-tank analytic solution and an independently constructed Erlang pulse.
//! Determinism: DeterministicPerIsa

#![cfg(test)]
#![allow(clippy::expect_used)]

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use fmine_cli::{Envelope, OutputFormat, invoke, run};
use fmine_core::hash::sha256;
use fmine_io::omf::json::{Json, parse};

const ONE: &str = include_str!("../../../tests/plant/rtd_one_tank.toml");
const PULSE: &str = include_str!("../../../tests/plant/rtd_shift_pulse.toml");
static NEXT: AtomicUsize = AtomicUsize::new(0);

fn scratch() -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "fmine-plant-rtd-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed),
    ));
    std::fs::create_dir_all(&path).expect("test directory");
    path
}
fn args() -> Vec<String> {
    ["plant", "rtd", "--input", "rtd.toml"].map(str::to_owned).to_vec()
}
fn at(root: &Path, text: &str) -> (Envelope, Json) {
    std::fs::write(root.join("rtd.toml"), text).expect("exact worksheet bytes");
    let (envelope, format) = invoke(&args(), root);
    assert_eq!(format, OutputFormat::Json);
    assert_eq!(envelope.to_json(), invoke(&args(), root).0.to_json());
    let json = parse(envelope.payload_json.as_bytes()).expect("valid payload JSON");
    (envelope, json)
}
fn evaluate(text: &str) -> (Envelope, Json) {
    at(&scratch(), text)
}
fn member<'a>(value: &'a Json, key: &str) -> &'a Json {
    value.get(key).expect(key)
}
fn array(value: &Json) -> &[Json] {
    value.as_array().expect("array")
}
fn number(value: &Json) -> f64 {
    value.f64().expect("finite number")
}
fn result(value: &Json) -> &Json {
    member(member(value, "transport"), "result")
}
fn intervals(value: &Json) -> &[Json] {
    array(member(result(value), "intervals"))
}
fn recoveries(value: &Json) -> &[Json] {
    array(member(member(member(member(value, "transport"), "recovery"), "result"), "intervals"))
}
fn mass(row: &Json, field: &str, channel: &str) -> f64 {
    number(member(member(row, field), channel))
}
fn close(actual: f64, expected: f64, tolerance: f64) {
    assert!((actual - expected).abs() <= tolerance, "{actual} != {expected}");
}
fn admitted(envelope: &Envelope) {
    assert_eq!(envelope.exit_code, 0, "{}", envelope.to_json());
    assert_eq!(envelope.status, "ok");
    assert!(envelope.certificate.is_none());
    let json = parse(envelope.to_json().as_bytes()).expect("envelope JSON");
    for (_, value) in member(&json, "flags").as_object().expect("flags") {
        assert_eq!(value.as_bool(), Some(false));
    }
    assert!(envelope.no_claim.expect("conditional scope").contains("uncalibrated"));
}
fn conservation(value: &Json) {
    for row in intervals(value) {
        for (channel, tolerance) in [("dry_kg", 1e-8), ("water_kg", 1e-8), ("element_kg", 1e-12)] {
            let independently_formed = mass(row, "opening_inventory_kg", channel)
                + mass(row, "feed_kg", channel)
                - mass(row, "total_outflow_kg", channel)
                - mass(row, "closing_inventory_kg", channel);
            close(independently_formed, 0.0, tolerance);
            close(mass(row, "balance_residual_kg", channel), 0.0, tolerance);
        }
    }
    for (channel, tolerance) in [("dry_kg", 1e-8), ("water_kg", 1e-8), ("element_kg", 1e-12)] {
        close(number(member(member(result(value), "whole_window_residual_kg"), channel)), 0.0, tolerance);
    }
}
fn refused(text: &str, code: &str) {
    let (envelope, json) = evaluate(text);
    assert_ne!(envelope.exit_code, 0, "{}", envelope.to_json());
    assert_eq!(envelope.refusal.as_ref().expect("refusal").code, code);
    let transport = member(&json, "transport");
    for field in ["admitted_claim", "receipt_digest", "result", "recovery"] {
        assert_eq!(member(transport, field), &Json::Null, "{field}");
    }
    assert_eq!(member(transport, "submitted_worksheet").as_str(), Some(text));
    assert!(envelope.rendered_markdown.is_none());
}

#[test]
fn one_tank_answer_closes_and_retains_exact_sources_and_distinct_recovery_ratios() {
    let root = scratch();
    let (envelope, json) = at(&root, ONE);
    admitted(&envelope);
    conservation(&json);
    let row = &intervals(&json)[0];
    close(mass(row, "total_outflow_kg", "element_kg"), 1.3678794411714423, 1e-12);
    close(mass(row, "closing_inventory_kg", "element_kg"), 1.6321205588285577, 1e-12);
    close(mass(row, "total_outflow_kg", "dry_kg"), 10.0, 1e-12);
    close(mass(row, "total_outflow_kg", "water_kg"), 20.0, 1e-12);
    let recovery = &recoveries(&json)[0];
    close(number(member(recovery, "lag_aligned_recovery")), 0.9, 1e-12);
    close(number(member(recovery, "same_interval_recovery")), 0.9316060279414279, 1e-12);
    let transport = member(&json, "transport");
    assert_eq!(member(transport, "submitted_worksheet").as_str(), Some(ONE));
    assert!(ONE.contains("dry_kg = \"10.000\""));
    let expected_hash = sha256(ONE.as_bytes()).to_hex();
    assert_eq!(member(member(&json, "source"), "sha256").as_str(), Some(expected_hash.as_str()));
    for key in ["model", "opening", "admitted_claim"] {
        let claim = member(transport, key);
        assert_eq!(member(claim, "evidence").as_str(), Some("Assumed"));
        assert_eq!(member(claim, "confidentiality").as_str(), Some("Internal"));
        assert_eq!(member(claim, "calibration").as_str(), Some("uncalibrated"));
        for anchor in array(member(claim, "sources")) {
            assert_eq!(member(anchor, "sha256").as_str(), Some(expected_hash.as_str()));
            assert!(!member(anchor, "locator").as_str().expect("locator").is_empty());
        }
    }
    assert!(array(member(member(transport, "admitted_claim"), "sources")).len() >= 3);
    let mut markdown_args = args();
    markdown_args.extend(["--format".into(), "markdown".into()]);
    let (markdown, code) = run(&markdown_args, &root);
    assert_eq!(code, 0);
    assert_eq!(Some(markdown.as_str()), envelope.rendered_markdown.as_deref());
    assert!(markdown.contains("Same-period recovery"));
    assert!(markdown.contains("Lag-aligned recovery"));
}

#[test]
fn eight_tank_pulse_keeps_constructed_recovery_under_changed_clock_durations() {
    let (envelope, json) = evaluate(PULSE);
    admitted(&envelope);
    conservation(&json);
    let expected = [0.9593274602546497, 0.8170337031358154, 0.7620153539130665, 0.831984567722057];
    for (row, ordinary) in recoveries(&json).iter().zip(expected) {
        close(number(member(row, "same_interval_recovery")), ordinary, 1e-12);
        close(number(member(row, "lag_aligned_recovery")), 0.88, 1e-12);
    }
    assert_eq!(intervals(&json).len(), 4);
    let stretched = PULSE.replace("172800000000000", "216000000000000");
    let (envelope, slow) = evaluate(&stretched);
    admitted(&envelope);
    conservation(&slow);
    for (nominal, shifted) in intervals(&json).iter().zip(intervals(&slow)) {
        assert_eq!(member(nominal, "total_outflow_kg"), member(shifted, "total_outflow_kg"));
        assert_eq!(member(nominal, "closing_inventory_kg"), member(shifted, "closing_inventory_kg"));
    }
    for row in recoveries(&slow) {
        close(number(member(row, "lag_aligned_recovery")), 0.88, 1e-12);
    }
}

#[test]
fn bypass_dead_stock_shutdown_and_zero_denominators_are_explicit() {
    let bypass = ONE.replace("bypass_fraction = \"0\"", "bypass_fraction = \"1\"");
    let (envelope, json) = evaluate(&bypass);
    admitted(&envelope);
    conservation(&json);
    let row = &intervals(&json)[0];
    close(mass(row, "bypass_kg", "element_kg"), 2.0, 0.0);
    close(mass(row, "tank_outflow_kg", "element_kg"), 0.0, 0.0);
    close(mass(row, "closing_inventory_kg", "element_kg"), 1.0, 0.0);
    let dead = ONE.replace("dead_fraction = \"0\"", "dead_fraction = \"0.25\"").replace(
        "tank = \"dead\"\ndry_kg = \"0\"\nwater_kg = \"0\"\nelement_kg = \"0\"",
        "tank = \"dead\"\ndry_kg = \"3\"\nwater_kg = \"4\"\nelement_kg = \"0.5\"",
    );
    let (envelope, json) = evaluate(&dead);
    admitted(&envelope);
    conservation(&json);
    close(number(member(member(result(&json), "dead_inventory_kg"), "element_kg")), 0.5, 0.0);
    let shutdown = ONE.replace(
        "slurry_volume_m3 = \"1\"\ndry_kg = \"10\"\nwater_kg = \"20\"\nelement_kg = \"2\"",
        "slurry_volume_m3 = \"0\"\ndry_kg = \"0\"\nwater_kg = \"0\"\nelement_kg = \"0\"",
    ).replace("element_kg = \"0.136787944117144232\"", "element_kg = \"0\"");
    let (envelope, json) = evaluate(&shutdown);
    admitted(&envelope);
    conservation(&json);
    let row = &intervals(&json)[0];
    close(mass(row, "total_outflow_kg", "element_kg"), 0.0, 0.0);
    close(mass(row, "closing_inventory_kg", "element_kg"), 1.0, 0.0);
    assert_eq!(member(&recoveries(&json)[0], "same_interval_recovery"), &Json::Null);
    assert_eq!(member(&recoveries(&json)[0], "lag_aligned_recovery"), &Json::Null);
}

#[test]
fn incomplete_inputs_false_assumptions_and_invalid_observations_refuse_atomically() {
    let dead_row = "[[opening]]\ntank = \"dead\"\ndry_kg = \"0\"\nwater_kg = \"0\"\nelement_kg = \"0\"\n";
    refused(&ONE.replace(dead_row, ""), "FMINE-REF-506");
    refused(&format!("{ONE}\n{dead_row}"), "FMINE-REF-506");
    refused(&ONE.replace("tank = \"1\"", "tank = \"2\""), "FMINE-REF-506");
    refused(&ONE.replace("opening_ns = \"0\"", "opening_ns = \"1\""), "FMINE-REF-102");
    refused(&ONE.replace("max_flow_m3_per_s = \"2\"", "max_flow_m3_per_s = \"0.5\""), "FMINE-REF-203");
    refused(
        &PULSE.replace("max_kernel_steps = \"64\"", "max_kernel_steps = \"1\""),
        "FMINE-REF-207",
    );
    for key in [
        "common_conservative_transport", "piecewise_constant_feed_per_volume",
        "delay_then_separation", "complete_tail_population", "all_non_tail_element_is_recovered",
    ] {
        refused(&ONE.replace(&format!("{key} = \"true\""), &format!("{key} = \"false\"")), "FMINE-REF-201");
    }
    let without_tail = ONE.split("[[tail]]").next().expect("before tail");
    refused(without_tail, "FMINE-REF-102");
    let tail = "\n[[tail]]\nstart_ns = \"0\"\nend_ns = \"1000000000\"\nelement_kg = \"0\"\n";
    refused(&format!("{ONE}{tail}"), "FMINE-REF-102");
    refused(&ONE.replace("0.136787944117144232", "2"), "FMINE-REF-109");
    refused(&ONE.replace("volume_m3 = \"1\"", "volume_m3 = \"1e-999\""), "FMINE-REF-207");
    refused(&ONE.replace("dry_kg = \"10.000\"", "dry_kg = \"1.1e-2147483648\""), "FMINE-REF-506");
    refused(
        &ONE.replace("[[feed]]\nstart_ns = \"0\"", "[[feed]]\nstart_ns = \"1\""),
        "FMINE-REF-102",
    );
    let without_recovery = ONE.split("[[recovery]]").next().expect("transport-only source");
    let (envelope, json) = evaluate(without_recovery);
    admitted(&envelope);
    assert_eq!(member(member(&json, "transport"), "recovery"), &Json::Null);
}

#[test]
fn source_tokens_and_recipe_mutations_change_receipts_and_docs_expose_the_command() {
    let root = scratch();
    let (first, original) = at(&root, ONE);
    admitted(&first);
    let (spelled, equivalent) = at(&root, &ONE.replace("10.000", "10.0"));
    admitted(&spelled);
    for field in ["final_tanks_kg", "dead_inventory_kg", "whole_window_residual_kg", "kernel_steps"] {
        assert_eq!(member(result(&original), field), member(result(&equivalent), field));
    }
    for (left, right) in intervals(&original).iter().zip(intervals(&equivalent)) {
        for field in ["opening_inventory_kg", "feed_kg", "bypass_kg", "tank_outflow_kg", "total_outflow_kg", "closing_inventory_kg", "balance_residual_kg"] {
            assert_eq!(member(left, field), member(right, field));
        }
    }
    let a = member(member(&original, "transport"), "receipt_digest");
    let b = member(member(&equivalent, "transport"), "receipt_digest");
    assert_ne!(a, b);
    assert_ne!(member(&original, "source"), member(&equivalent, "source"));
    let (changed, altered) = at(
        &root,
        &ONE.replace("\nvolume_m3 = \"1\"\n", "\nvolume_m3 = \"2\"\n"),
    );
    admitted(&changed);
    assert_ne!(a, member(member(&altered, "transport"), "receipt_digest"));
    assert_ne!(
        member(&intervals(&original)[0], "total_outflow_kg"),
        member(&intervals(&altered)[0], "total_outflow_kg"),
    );
    let (docs, _) = invoke(&["robot-docs".into()], &root);
    assert_eq!(docs.exit_code, 0);
    assert!(docs.payload_json.contains("plant rtd --input"));
    assert!(docs.payload_json.contains("\"plant_rtd\""));
    assert!(docs.payload_json.contains("fmine.plant-rtd.v1"));
    let (usage, _) = invoke(&["plant".into(), "rtd".into()], &root);
    assert_eq!(usage.exit_code, 2);
    assert!(usage.to_json().contains("plant rtd --input"));
}
