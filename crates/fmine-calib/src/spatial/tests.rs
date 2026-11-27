//! DESIGN: synthetic known answers and source/chronology controls for the spatial adapter.
//! Determinism: DeterministicPerIsa

use super::*;

const POLICY: &str = r#"schema = "fmine.spatial-monitor.v1"
model_id = "synthetic-front-model"
target_unit = "g/t"
coordinate_frame = "local-metre-grid"
support = "mining_polygon"
nugget_var = "0.25"
sill_var = "0.75"
range_m = "30"
max_neighbors = 4
alpha = "0.05"
registered_ns = 10
"#;

fn sample(index: usize, x: f64, observed: f64) -> String {
    format!(
        r#"
[[observation]]
id = "polygon-{index}"
available_ns = {}
prediction_registered_ns = 20
x_m = "{x}"
y_m = "0"
z_m = "0"
prior_mean = "5"
observed_value = "{observed}"
target_unit = "g/t"
coordinate_frame = "local-metre-grid"
support = "mining_polygon"
locator = "synthetic survey:polygon-{index}"
"#,
        30 + index
    )
}

fn fixture() -> String {
    format!("{POLICY}{}{}", sample(0, 0.0, 6.0), sample(1, 30.0, 5.0))
}

fn anchor(text: &str) -> SourceAnchor {
    SourceAnchor::new(
        "synthetic/front.toml",
        sha256(text.as_bytes()).0,
        "complete supplied worksheet",
    )
}

fn run(text: &str) -> Result<SpatialReceipt> {
    evaluate(text, anchor(text))
}

#[test]
fn first_step_and_conditioned_second_step_match_gaussian_known_answers() {
    let receipt = run(&fixture()).expect("valid synthetic worksheet");
    let first = &receipt.rows()[0].value;
    assert_eq!(first.conditional_mean, 5.0);
    assert_eq!(first.conditional_std_dev, 1.0);
    assert_eq!(first.standardized_residual, 1.0);
    assert!((first.pit - 0.841_344_746_068_542_9).abs() < 1e-6);
    assert_eq!(first.location_log_wealth, 0.0);
    assert_eq!(first.dispersion_log_wealth, 0.0);

    // Independent two-dimensional Gaussian conditioning with C(30) = 0.75 exp(-1).
    let covariance = 0.75 * (-1.0_f64).exp();
    let expected_mean = 5.0 + covariance;
    let expected_sd = (1.0 - covariance * covariance).sqrt();
    let second = &receipt.rows()[1].value;
    assert!((second.conditional_mean - expected_mean).abs() < 1e-13);
    assert!((second.conditional_std_dev - expected_sd).abs() < 1e-13);
    assert!((second.standardized_residual - (5.0 - expected_mean) / expected_sd).abs() < 1e-13);
    assert!((second.log_threshold - 40.0_f64.ln()).abs() < 1e-13);
    assert_eq!(receipt.claim().value.observations, 2);
}

#[test]
fn receipt_keeps_complete_assumed_evidence_and_diagnostic_parent_chain() {
    let input = fixture();
    let receipt = run(&input).expect("worksheet");
    let repeated = run(&input).expect("repeated worksheet");
    assert_eq!(receipt.to_json(), repeated.to_json());
    assert_eq!(receipt.to_markdown(), repeated.to_markdown());
    assert_eq!(receipt.digest(), repeated.digest());
    assert_eq!(receipt.inputs().len(), 3);
    assert_eq!(receipt.claim().value_tag(), receipt.digest());
    for claim in receipt.inputs() {
        claim.check_id().expect("bound input");
        assert_eq!(claim.evidence, EvidenceClass::Assumed);
        assert_eq!(claim.confidentiality, Confidentiality::internal());
        assert_eq!(claim.anchors[0].artifact_sha256, sha256(input.as_bytes()).0);
    }
    for (index, claim) in receipt.rows().iter().enumerate() {
        claim.check_id().expect("bound result");
        assert_eq!(claim.evidence, EvidenceClass::Assumed);
        assert_eq!(claim.support, SpatialSupport::MiningPolygon);
        assert!(claim.parents.contains(&receipt.inputs()[0].claim_id));
        assert!(
            claim
                .parents
                .contains(&receipt.inputs()[index + 1].claim_id)
        );
        if index > 0 {
            assert!(claim.parents.contains(&receipt.rows()[index - 1].claim_id));
        }
    }
    receipt.claim().check_id().expect("bound summary");
    assert!(receipt.claim().verify_uu_slot_for_decision().is_err());
    assert!(
        receipt
            .to_json()
            .contains("\"calibration\":\"descriptive\"")
    );
    assert!(receipt.to_markdown().contains("not a calibration verdict"));
}

#[test]
fn markdown_retains_inputs_diagnostics_monitor_history_and_claim_bindings() {
    let input = fixture()
        .replace("y_m = \"0\"", "y_m = \"12.5\"")
        .replace("z_m = \"0\"", "z_m = \"-3\"");
    let receipt = run(&input).expect("full review fixture");
    let report = receipt.to_markdown();
    for expected in [
        "| 1 | polygon-0 | 0 | 12.5 | -3 | 5 | 6 |",
        "| 2 | polygon-1 | 30 | 12.5 | -3 | 5 | 5 |",
        "| 1 | 20 | 30 | synthetic survey:polygon-0 |",
        "| 2 | 20 | 31 | synthetic survey:polygon-1 |",
        "Observations: 2. Summary location alarm: false. Summary dispersion alarm: false.",
        "nonnegative UTC Unix nanoseconds",
        "Assumed/Internal with known-unknown epistemic status",
        "complete supplied worksheet; policy",
        "Current log wealth | Peak log wealth | Log threshold | Alarm",
        METHOD,
        NO_CLAIM,
        receipt.digest(),
    ] {
        assert!(
            report.contains(expected),
            "missing report field: {expected}"
        );
    }
    // Distinct current/peak values make an accidental substitution visible.
    assert_ne!(
        receipt.rows()[1].value.location_log_wealth,
        receipt.rows()[1].value.location_max_log_wealth
    );
    for claim in receipt.rows() {
        let row = &claim.value;
        for expected in [
            format!(
                "| {} | {} | {} | {} | {} |",
                row.sequence,
                row.conditional_mean,
                row.conditional_std_dev,
                row.standardized_residual,
                row.pit
            ),
            format!(
                "| {} | Location | {} | {} | {} | {} |",
                row.sequence,
                row.location_log_wealth,
                row.location_max_log_wealth,
                row.log_threshold,
                row.location_alarm
            ),
            format!(
                "| {} | Dispersion | {} | {} | {} | {} |",
                row.sequence,
                row.dispersion_log_wealth,
                row.dispersion_max_log_wealth,
                row.log_threshold,
                row.dispersion_alarm
            ),
        ] {
            assert!(report.contains(&expected), "missing report row: {expected}");
        }
    }
    for (claim_id, metric, tag, parents, anchors) in receipt
        .inputs()
        .iter()
        .map(|claim| {
            (
                claim.claim_id,
                &claim.metric_key,
                claim.value_tag(),
                &claim.parents,
                &claim.anchors,
            )
        })
        .chain(receipt.rows().iter().map(|claim| {
            (
                claim.claim_id,
                &claim.metric_key,
                claim.value_tag(),
                &claim.parents,
                &claim.anchors,
            )
        }))
        .chain(std::iter::once((
            receipt.claim().claim_id,
            &receipt.claim().metric_key,
            receipt.claim().value_tag(),
            &receipt.claim().parents,
            &receipt.claim().anchors,
        )))
    {
        assert!(report.contains(&claim_id.to_string()));
        assert!(report.contains(metric));
        assert!(report.contains(tag));
        for parent in parents {
            assert!(report.contains(&parent.to_string()));
        }
        for anchor in anchors {
            assert!(report.contains(&anchor.artifact_path));
            assert!(report.contains(&Digest32(anchor.artifact_sha256).to_hex()));
            assert!(report.contains(&anchor.locator));
        }
    }
}

#[test]
fn model_policy_source_and_observation_changes_move_receipt_identity() {
    let input = fixture();
    let reference = run(&input).expect("reference");
    for changed in [
        input.replace("synthetic-front-model", "another-model"),
        input.replace("g/t", "percent"),
        input.replace("local-metre-grid", "another-metre-grid"),
        input.replace("mining_polygon", "point_core"),
        input.replace("nugget_var = \"0.25\"", "nugget_var = \"0.5\""),
        input.replace("sill_var = \"0.75\"", "sill_var = \"1.0\""),
        input.replace("range_m = \"30\"", "range_m = \"40\""),
        input.replace("max_neighbors = 4", "max_neighbors = 8"),
        input.replace("alpha = \"0.05\"", "alpha = \"0.1\""),
        input.replace("registered_ns = 10", "registered_ns = 11"),
        input.replace(
            "prediction_registered_ns = 20",
            "prediction_registered_ns = 21",
        ),
        input.replace("observed_value = \"6\"", "observed_value = \"7\""),
        input.replace("synthetic survey:polygon-0", "corrected survey:polygon-0"),
        format!("{input}\n# retained worksheet note\n"),
    ] {
        let changed = run(&changed).expect("valid changed declaration");
        assert_ne!(reference.digest(), changed.digest());
        assert_ne!(reference.claim().claim_id, changed.claim().claim_id);
    }
    let mut other_source = anchor(&input);
    other_source.artifact_path = "another/front.toml".into();
    let other = evaluate(&input, other_source).expect("different source identity");
    assert_ne!(reference.digest(), other.digest());
}

#[test]
fn source_mismatch_and_incompatible_unit_frame_or_support_refuse() {
    let input = fixture();
    let altered = input.replace("observed_value = \"6\"", "observed_value = \"7\"");
    assert_eq!(
        evaluate(&altered, anchor(&input))
            .expect_err("changed bytes")
            .code,
        "FMINE-REF-505"
    );
    for (old, new) in [
        ("target_unit = \"g/t\"", "target_unit = \"percent\""),
        (
            "coordinate_frame = \"local-metre-grid\"",
            "coordinate_frame = \"other-frame\"",
        ),
        ("support = \"mining_polygon\"", "support = \"point_core\""),
    ] {
        let changed = input.replacen(old, new, 1);
        assert_eq!(
            run(&changed).expect_err("incompatible observation").code,
            "FMINE-REF-102"
        );
    }
}

#[test]
fn chronology_unknown_fields_duplicates_and_nonfinite_values_refuse() {
    let input = fixture();
    for changed in [
        input.replace("available_ns = 31", "available_ns = 30"),
        input.replace("available_ns = 31", "available_ns = 29"),
        input.replace(
            "prediction_registered_ns = 20",
            "prediction_registered_ns = 30",
        ),
        input.replace("registered_ns = 10", "registered_ns = 21"),
    ] {
        assert_eq!(
            run(&changed).expect_err("invalid chronology").code,
            "FMINE-REF-206"
        );
    }
    for changed in [
        input.replace("id = \"polygon-1\"", "id = \"polygon-0\""),
        input.replace("[[observation]]", "[[unrecognized]]"),
        input.replace("observed_value = \"6\"", "observed_value = \"NaN\""),
        input.replace("observed_value = \"6\"", "observed_value = \"inf\""),
        input.replace("available_ns = 30", "available_ns = -1"),
        input.replace(
            "range_m = \"30\"",
            "range_m = \"30\"\nsecret_knob = \"yes\"",
        ),
    ] {
        assert!(run(&changed).is_err(), "{changed}");
    }
    for changed in [
        input.replace("alpha = \"0.05\"", "alpha = \"0\""),
        input.replace("max_neighbors = 4", "max_neighbors = 17"),
        input.replace("nugget_var = \"0.25\"", "nugget_var = \"0\""),
    ] {
        assert_eq!(
            run(&changed).expect_err("out-of-domain policy").code,
            "FMINE-REF-109"
        );
    }
}

#[test]
fn finite_overflow_empty_and_oversized_populations_never_emit_partial_success() {
    let input = fixture();
    let overflow = input
        .replace("nugget_var = \"0.25\"", "nugget_var = \"1e308\"")
        .replace("sill_var = \"0.75\"", "sill_var = \"1e308\"");
    assert_eq!(
        run(&overflow).expect_err("total variance overflow").code,
        "FMINE-REF-207"
    );
    let overflow = input
        .replace("prior_mean = \"5\"", "prior_mean = \"-1e308\"")
        .replace("observed_value = \"6\"", "observed_value = \"1e308\"");
    assert_eq!(
        run(&overflow).expect_err("residual overflow").code,
        "FMINE-REF-207"
    );
    let overflow = input
        .replace("x_m = \"0\"", "x_m = \"-1e308\"")
        .replace("x_m = \"30\"", "x_m = \"1e308\"");
    assert_eq!(
        run(&overflow)
            .expect_err("coordinate distance overflow")
            .code,
        "FMINE-REF-207"
    );
    assert!(run(POLICY).is_err());
    let mut oversized = POLICY.to_owned();
    for index in 0..=MAX_OBSERVATIONS {
        oversized.push_str(&sample(index, index as f64, 5.0));
    }
    assert!(run(&oversized).is_err());
    let bytes = " ".repeat(MAX_BYTES as usize + 1);
    assert!(run(&bytes).is_err());
}

#[test]
fn availability_cutoff_binds_an_inclusive_prefix_and_ignores_future_values() {
    let input = fixture().replace("available_ns = 31", "available_ns = 41");
    let full = run(&input).expect("full worksheet");
    let assess = |text: &str, cutoff| evaluate_as_of(text, anchor(text), cutoff);
    let prior = assess(&input, 30).expect("first observation exactly at cutoff");
    assert_eq!(prior.rows().len(), 1);
    assert_eq!(prior.inputs().len(), 2);
    assert_eq!(prior.rows()[0].value, full.rows()[0].value);
    assert_ne!(
        prior.digest(),
        assess(&input, 35)
            .expect("same population, later cutoff")
            .digest()
    );
    assert_eq!(
        prior.to_json(),
        assess(&input, 30)
            .expect("same worksheet and cutoff")
            .to_json()
    );
    assert_eq!(
        assess(&input, 41)
            .expect("second observation at cutoff")
            .rows()
            .len(),
        2
    );

    let changed = input.replace(
        "observed_value = \"5\"",
        "observed_value = \"future-value\"",
    );
    let isolated = assess(&changed, 30).expect("future value outside the cutoff");
    assert_eq!(prior.rows()[0].value, isolated.rows()[0].value);
    assert_eq!(prior.claim().value, isolated.claim().value);
    assert_ne!(prior.digest(), isolated.digest());
    for format in [isolated.to_json(), isolated.to_markdown()] {
        assert!(!format.contains("future-value"));
        assert!(!format.contains("polygon-1"));
    }
    assert!(assess(&changed, 41).is_err());
    assert!(evaluate_as_of(&changed, anchor(&input), 30).is_err());

    let empty = assess(&input, 10).expect("policy available, no outcomes yet");
    assert_eq!(empty.claim().value.observations, 0);
    assert!(empty.claim().value.first_alarm.is_none());
    assert_eq!(empty.claim().parents, vec![empty.inputs()[0].claim_id]);
    assert!(
        empty
            .to_markdown()
            .contains("No observations were available")
    );
    assert_eq!(assess(&input, 9).unwrap_err().code, "FMINE-REF-206");
    let reordered = format!("{input}{}", sample(2, 60.0, 5.0));
    assert_eq!(assess(&reordered, 30).unwrap_err().code, "FMINE-REF-206");
    let no_time = input.replace("available_ns = 41", "available_ns = \"unknown\"");
    assert!(assess(&no_time, 30).is_err());
}

#[test]
fn first_alarm_survives_later_observations_and_matches_existing_monitor() {
    let mut text = POLICY.to_owned();
    for index in 0..80 {
        let observed = if index < 40 { 11.0 } else { -1.0 };
        text.push_str(&sample(index, 10_000.0 * index as f64, observed));
    }
    let receipt = run(&text).expect("shifted synthetic front");
    let first = receipt.claim().value.first_alarm.as_ref().expect("alarm");
    assert!(first.sequence > 1 && first.sequence < 40);
    assert_eq!(
        first.observation_id,
        format!("polygon-{}", first.sequence - 1)
    );
    let mut monitor =
        SpatialFrontPitMonitor::new(0.25, 0.75, Qty::new(30.0), 4, 0.05).expect("same policy");
    for claim in receipt.rows() {
        let row = &claim.value;
        let (pit, alarm) = monitor
            .observe_sample(row.observation.sample)
            .expect("same observation");
        assert_eq!(row.pit.to_bits(), pit.to_bits());
        assert_eq!(row.location_alarm || row.dispersion_alarm, alarm);
        assert_eq!(
            row.location_log_wealth.to_bits(),
            monitor.location_eproc.log_wealth().to_bits()
        );
        assert_eq!(
            row.dispersion_log_wealth.to_bits(),
            monitor.dispersion_eproc.log_wealth().to_bits()
        );
        if row.sequence >= first.sequence {
            assert!(alarm, "an alarm cannot clear");
        }
    }
    assert_eq!(receipt.rows().len(), 80);
    let mut prefix = POLICY.to_owned();
    for index in 0..40 {
        prefix.push_str(&sample(index, 10_000.0 * index as f64, 11.0));
    }
    let prefix_receipt = run(&prefix).expect("independently assessed prefix");
    assert_eq!(
        prefix_receipt.claim().value.first_alarm.as_ref(),
        Some(first)
    );
    let before = evaluate_as_of(&text, anchor(&text), first.available_ns - 1)
        .expect("before threshold crossing");
    assert_eq!(before.rows().len(), first.sequence - 1);
    assert!(before.claim().value.first_alarm.is_none());
    let at = evaluate_as_of(&text, anchor(&text), first.available_ns)
        .expect("threshold crossing exactly at cutoff");
    assert_eq!(at.rows().len(), first.sequence);
    assert_eq!(at.claim().value.first_alarm.as_ref(), Some(first));
}

#[test]
fn output_escapes_source_text_and_keeps_exact_timestamp_strings() {
    let input = fixture()
        .replace("synthetic-front-model", "model <tag>|name")
        .replace("available_ns = 30", "available_ns = 9007199254740993")
        .replace("available_ns = 31", "available_ns = 9007199254740994");
    let receipt = run(&input).expect("large exact nanoseconds");
    let json = receipt.to_json();
    assert!(json.contains("\"available_ns\":\"9007199254740993\""));
    let markdown = receipt.to_markdown();
    assert!(markdown.contains("model &lt;tag&gt;\\|name"));
    assert!(markdown.contains("| 1 | 20 | 9007199254740993 |"));
    assert!(markdown.contains("| 2 | 20 | 9007199254740994 |"));
}
