//! DESIGN (BD-225): strict transport-model admission, source ancestry and declared ambiguity controls.
//! Determinism: Exact

use super::tests::{INFORMATION, MODEL};
use super::*;

const ROBUSTNESS: &str = r#"
[[robustness]]
method = "wasserstein_1"
radius = "1/4"
metric_basis = "Synthetic dimensionless distance between the two declared regimes"
radius_basis = "Declared sensitivity budget; no statistical confidence claim"
[[transport_distance]]
from = "low"
to = "high"
value = "1/1"
[[transport_distance]]
from = "high"
to = "high"
value = "0/1"
[[transport_distance]]
from = "low"
to = "low"
value = "0/1"
[[transport_distance]]
from = "high"
to = "low"
value = "1/1"
"#;

fn source(text: &str) -> SourceAnchor {
    SourceAnchor::new(
        "/synthetic/transport.toml",
        sha256(text.as_bytes()).0,
        "declared model",
    )
}

fn declared(text: &str) -> DecisionInputs {
    parse(text, source(text)).expect("complete synthetic transport declaration")
}

fn refused(text: &str) -> Refusal {
    parse(text, source(text)).expect_err("malformed transport declaration refuses")
}

#[test]
fn transport_configuration_retains_every_original_matrix_row_and_unknown() {
    let text = format!("{MODEL}{ROBUSTNESS}");
    let input = declared(&text);
    let claim = input.robustness.as_ref().expect("declared robustness");
    claim.check_id().expect("canonical configuration Claim");
    assert_eq!(claim.value_tag(), claim.value.canonical_tag());
    assert_eq!(claim.value.model_key, "synthetic-choice");
    assert_eq!(claim.value.radius, Rat::new(1, 4).unwrap());
    assert_eq!(claim.evidence, EvidenceClass::Assumed);
    assert_eq!(claim.confidentiality, Confidentiality::internal());
    assert_eq!(claim.additivity, AdditivityClass::NonAdditive);
    assert_eq!(claim.state, KnowledgeState::Estimated);
    assert_eq!(claim.uu_terms, input.weights.uu_terms);
    assert_eq!(claim.anchors.len(), 5);
    for anchor in &claim.anchors {
        assert_eq!(anchor.artifact_sha256, sha256(text.as_bytes()).0);
        assert_eq!(anchor.artifact_path, "/synthetic/transport.toml");
    }
    let doc = parse_subset(&text).unwrap();
    for row in &doc.rows {
        let description = match row.table.as_str() {
            "robustness" => format!("robustness method, radius and bases, line {}", row.line),
            "transport_distance" => {
                let [from, to, _] =
                    strings(&row.fields, ["from", "to", "value"], row.line, INPUT).unwrap();
                format!("transport distance {from}/{to}, line {}", row.line)
            }
            _ => continue,
        };
        assert!(
            claim
                .anchors
                .iter()
                .any(|a| a.locator == format!("declared model; {description}"))
        );
    }
    assert_eq!(
        claim
            .value
            .distances
            .iter()
            .map(|r| (r.from_state.as_str(), r.to_state.as_str()))
            .collect::<Vec<_>>(),
        [
            ("high", "high"),
            ("high", "low"),
            ("low", "high"),
            ("low", "low")
        ]
    );
    let output = crate::card::evaluate(&input).unwrap();
    assert!(output.claim().parents.contains(&claim.claim_id));
    assert!(
        claim
            .anchors
            .iter()
            .all(|anchor| output.claim().anchors.contains(anchor))
    );
    assert_eq!(output.claim().evidence, EvidenceClass::Assumed);
}

#[test]
fn optional_robustness_preserves_nominal_statistics_and_information_results() {
    let baseline_text = format!("{MODEL}{INFORMATION}");
    let baseline = crate::card::evaluate(&declared(&baseline_text)).unwrap();
    assert!(baseline.inputs().robustness.is_none());
    assert!(baseline.summary().robustness.is_none());
    assert!(!baseline.to_json().contains("\"robustness\""));
    for radius in ["0/1", "1/4", "1/1"] {
        let text = format!(
            "{baseline_text}{}",
            ROBUSTNESS.replace("radius = \"1/4\"", &format!("radius = \"{radius}\""))
        );
        let card = crate::card::evaluate(&declared(&text)).unwrap();
        assert_eq!(card.summary().alternatives, baseline.summary().alternatives);
        assert_eq!(card.summary().information, baseline.summary().information);
        assert_eq!(
            card.summary().expected_value_winners,
            baseline.summary().expected_value_winners
        );
        let robust = card.summary().robustness.as_ref().unwrap();
        if radius == "0/1" {
            assert_eq!(
                robust.maximin_mean_winners,
                baseline.summary().expected_value_winners
            );
            for action in &robust.alternatives {
                assert_eq!(action.worst_case_mean, action.nominal_mean);
                assert_eq!(
                    action.worst_case_expected_regret,
                    action.nominal_mean_regret
                );
            }
        }
        if radius == "1/1" {
            assert_eq!(robust.maximin_mean_winners, ["hold"]);
            assert_eq!(robust.minimax_expected_regret_winners, ["expand"]);
            assert_eq!(
                robust.alternatives[0].worst_case_mean,
                Rat::new(-1_200_001, 10_000).unwrap()
            );
            assert_eq!(
                robust.alternatives[1].worst_case_mean,
                Rat::new(-1, 10_000).unwrap()
            );
        }
    }
}

#[test]
fn transport_controls_and_original_source_bytes_bind_identity() {
    let original = format!("{MODEL}{ROBUSTNESS}");
    let initial = declared(&original);
    let initial_claim = initial.robustness.as_ref().unwrap();
    let initial_card = crate::card::evaluate(&initial).unwrap();
    for (old, new) in [
        ("radius = \"1/4\"", "radius = \"1/2\""),
        (
            "Synthetic dimensionless distance between the two declared regimes",
            "Alternative declared dimensionless regime separation",
        ),
        (
            "Declared sensitivity budget; no statistical confidence claim",
            "Separate declared sensitivity budget with no calibration",
        ),
    ] {
        let changed = declared(&original.replace(old, new));
        assert_ne!(
            initial_claim.claim_id,
            changed.robustness.as_ref().unwrap().claim_id
        );
        assert_ne!(
            initial_claim.value_tag(),
            changed.robustness.as_ref().unwrap().value_tag()
        );
        assert_ne!(
            initial_card.digest(),
            crate::card::evaluate(&changed).unwrap().digest()
        );
    }
    let mut blocks = ROBUSTNESS.split("[[transport_distance]]");
    let declaration = blocks.next().unwrap();
    let reordered = format!(
        "{MODEL}{declaration}{}",
        blocks
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .map(|block| format!("[[transport_distance]]{block}"))
            .collect::<String>()
    );
    let reordered = declared(&reordered);
    assert_eq!(
        initial_claim.value,
        reordered.robustness.as_ref().unwrap().value
    );
    assert_eq!(
        initial_claim.value_tag(),
        reordered.robustness.as_ref().unwrap().value_tag()
    );
    assert_ne!(
        initial_claim.claim_id,
        reordered.robustness.as_ref().unwrap().claim_id
    );
    assert_eq!(
        initial_card.summary().robustness,
        crate::card::evaluate(&reordered)
            .unwrap()
            .summary()
            .robustness
    );
    let changed_bytes = original.replace("1/4", "1/2");
    assert_eq!(
        parse(&changed_bytes, source(&original)).unwrap_err().code,
        RefusalCode::DigestMismatch505.as_str()
    );
}

#[test]
fn transport_rows_require_one_complete_explicit_configuration() {
    let declaration = ROBUSTNESS.split("[[transport_distance]]").next().unwrap();
    let first_row = "[[transport_distance]]\nfrom = \"low\"\nto = \"high\"\nvalue = \"1/1\"\n";
    let text = format!("{MODEL}{ROBUSTNESS}");
    assert!(
        refused(&format!("{text}{declaration}"))
            .what
            .contains("only one robustness")
    );
    assert!(
        refused(&format!("{text}{first_row}"))
            .what
            .contains("duplicate transport distance")
    );
    assert!(
        refused(&format!("{MODEL}{first_row}"))
            .what
            .contains("require a robustness declaration")
    );
    assert!(
        refused(&text.replace(first_row, ""))
            .what
            .contains("missing transport distance low/high")
    );
    assert!(
        refused(&text.replace("from = \"low\"", "from = \"foreign\""))
            .what
            .contains("unknown state")
    );
    assert!(
        refused(&format!("{MODEL}{declaration}"))
            .what
            .contains("missing transport distance")
    );
    for change in [
        ROBUSTNESS.replace("method = \"wasserstein_1\"", "method = \"wasserstein_2\""),
        ROBUSTNESS.replace("radius = \"1/4\"", "radius = 1"),
        ROBUSTNESS.replace("radius = \"1/4\"\n", ""),
        ROBUSTNESS.replace(
            "radius = \"1/4\"",
            "radius = \"1/4\"\nconfidence = \"0.95\"",
        ),
        ROBUSTNESS.replace("value = \"1/1\"", "value = 1"),
        ROBUSTNESS.replace("from = \"low\"", "from = \" low\""),
        ROBUSTNESS.replace(
            "radius_basis = \"Declared sensitivity budget; no statistical confidence claim\"",
            "radius_basis = \"\"",
        ),
        ROBUSTNESS.replace("to = \"high\"", "to = \"high\"\ncalibrated = \"yes\""),
    ] {
        assert_eq!(refused(&format!("{MODEL}{change}")).code, INPUT.as_str());
    }
    for invalid in [
        "-1/4",
        "+1/4",
        "0.25",
        "1",
        "1/0",
        "0/0",
        "1//2",
        "1/ 2",
        "1e2/1",
        "170141183460469231731687303715884105728/1",
    ] {
        assert_eq!(
            refused(&text.replace("radius = \"1/4\"", &format!("radius = \"{invalid}\""))).code,
            DOMAIN.as_str(),
            "{invalid}"
        );
        assert_eq!(
            refused(&text.replace("value = \"1/1\"", &format!("value = \"{invalid}\""))).code,
            DOMAIN.as_str(),
            "{invalid}"
        );
    }
}

#[test]
fn transport_input_bounds_precede_metric_evaluation_and_invalid_metrics_refuse() {
    let text = format!("{MODEL}{ROBUSTNESS}");
    for changed in [
        text.replace("value = \"0/1\"", "value = \"1/1\""),
        text.replace("value = \"1/1\"", "value = \"0/1\""),
        text.replacen("value = \"1/1\"", "value = \"2/1\"", 1),
    ] {
        let inputs = declared(&changed);
        assert!(crate::card::evaluate(&inputs).is_err());
    }
    let declaration = ROBUSTNESS.split("[[transport_distance]]").next().unwrap();
    let mut rows = robustness::Rows::default();
    let doc = parse_subset(declaration).unwrap();
    rows.declaration(&doc.rows[0], "synthetic-choice").unwrap();
    let states: BTreeMap<_, _> = (0..65)
        .map(|i| (format!("state_{i:02}"), Rat::new(1, 65).unwrap()))
        .collect();
    let refusal = rows
        .finish(
            &states,
            2,
            &declared(MODEL).weights.uu_terms,
            &source(&text),
        )
        .unwrap_err();
    assert_eq!(refusal.code, DOMAIN.as_str());
    assert!(refusal.what.contains("64 states"));
    let oversized = ROBUSTNESS.replace(
        "Synthetic dimensionless distance between the two declared regimes",
        &"m".repeat(4097),
    );
    assert_eq!(refused(&format!("{MODEL}{oversized}")).code, INPUT.as_str());
}
