//! FACT: authored finite populations with hand-computed answers; no plant data
//! or probability calibration. The correlation control holds marginals fixed.
//! Determinism: DeterministicPerIsa

use super::*;
use fmine_claim::{Confidentiality, UuTerm};
use fmine_core::{anchor::SourceAnchor, hash::sha256};
use fmine_evidence::{EpistemicStatus, EvidenceClass};

fn q(value: &str) -> ExactQty {
    ExactQty::from_decimal_str(value).unwrap()
}
fn source<T>(metric: &str, value: T, tag: &str) -> Claim<T> {
    Claim::new(
        metric,
        value,
        tag,
        Representation::Deterministic,
        SpatialSupport::PeriodAggregate,
        AdditivityClass::NonAdditive,
        EvidenceClass::Assumed,
        EpistemicStatus::KnownKnown,
        KnowledgeState::Known,
        Confidentiality::public(),
        vec![],
        vec![],
        vec![SourceAnchor::new(
            "crates/fmine-plant/src/cards/ensemble/tests.rs",
            sha256(include_bytes!("tests.rs")).0,
            "authored joint recovery population; not operating observations",
        )],
    )
}
fn version(id: &str) -> CardVersion {
    CardVersion { id: id.into(), revision: "constructed-v1".into() }
}
fn model(formula: &str, names: &[&str], cap: Option<&str>) -> RecoveryModel {
    let value = TestWorkCard {
        version: version("test-work"),
        scope: RecoveryScope {
            circuit: "synthetic-circuit".into(), material_domain: "synthetic-domain".into(),
            dry_basis: "total-dry-matter/v1".into(), element: "Au".into(),
        },
        valid_time: ValidTime { start_ns: 0, end_ns: 100 },
        input_units: names.iter().map(|name| (name.to_string(), InputRequirement {
            measurand: format!("synthetic-{name}/dry"), unit: InputUnit::GramsPerDryMetricTonne,
            lower: q("0"), upper: q("10"), lower_inclusive: true, upper_inclusive: true,
        })).collect(),
        formula: formula.into(), output_scale: OutputScale::Fraction,
    };
    let tag = value.canonical_tag();
    let card = source(TEST_WORK_METRIC, value, &tag);
    let value = RecoveryCorrection {
        version: version("correction"), test_work: card.claim_id,
        additive_fraction: q("0"), upper_cap_fraction: cap.map(q),
    };
    let tag = value.canonical_tag();
    let correction = source(CORRECTION_METRIC, value, &tag);
    let value = RouteAssumption {
        version: version("route"), test_work: card.claim_id, route: "declared-process".into(),
    };
    let tag = value.canonical_tag();
    let route = source(ROUTE_METRIC, value, &tag);
    RecoveryModel::compile(&card, &[], &correction, &route).unwrap()
}
fn inputs(model: &RecoveryModel, values: &[(&str, &str)]) -> Claim<RecoveryInputs> {
    let value = RecoveryInputs {
        scope: model.test_work().value.scope.clone(),
        interval: ValidTime { start_ns: 10, end_ns: 20 },
        values: values.iter().map(|(name, value)| (name.to_string(), InputValue {
            measurand: format!("synthetic-{name}/dry"), unit: InputUnit::GramsPerDryMetricTonne,
            value: q(value),
        })).collect(),
    };
    input_claim(value)
}
fn input_claim(value: RecoveryInputs) -> Claim<RecoveryInputs> {
    let tag = value.canonical_tag();
    source(super::super::INPUT_METRIC, value, &tag)
}
fn population(value: RecoveryPopulation) -> Claim<RecoveryPopulation> {
    let tag = value.canonical_tag();
    source(INPUT_METRIC, value, &tag)
}
fn scenario(model: &RecoveryModel, id: &str, weight: &str, values: &[(&str, &str)]) -> RecoveryScenario {
    RecoveryScenario { id: id.into(), weight: q(weight), inputs: inputs(model, values) }
}
fn binary(model: &RecoveryModel, low: &str, high: &str) -> RecoveryPopulation {
    RecoveryPopulation { scenarios: vec![
        scenario(model, "low", "0.5", &[("g", low)]),
        scenario(model, "high", "0.5", &[("g", high)]),
    ] }
}
fn close(actual: f64, expected: f64) {
    assert!((actual - expected).abs() <= 2e-15, "{actual} != {expected}");
}

#[test]
fn nonlinear_expectation_is_not_the_card_at_mean_input() {
    let model = model("g^2/4", &["g"], None);
    let receipt = model.evaluate_ensemble(&population(binary(&model, "0", "2"))).unwrap();
    let summary = &receipt.claim().value;
    close(summary.mean.value(), 0.5);
    close(summary.variance.value(), 0.25);
    assert_eq!([summary.minimum.value(), summary.p10.value(), summary.p50.value(),
        summary.p90.value(), summary.maximum.value()], [0.0, 0.0, 0.0, 1.0, 1.0]);
    close(model.evaluate(&inputs(&model, &[("g", "1")])).unwrap().claim().value.recovery_fraction.value(), 0.25);
    assert_eq!(summary.probability_capped, q("0"));
    assert_eq!(receipt.members().iter().map(|m| m.id()).collect::<Vec<_>>(), ["high", "low"]);
    assert!(matches!(receipt.claim().representation, Representation::EnsembleSummary { n: 2, mean: 0.5, .. }));
    assert_eq!(receipt.claim().state, KnowledgeState::Estimated);
    assert_eq!(receipt.claim().evidence, EvidenceClass::Assumed);
    assert!(receipt.claim().parents.contains(&receipt.population().claim_id));
    for member in receipt.members() {
        assert!(receipt.claim().parents.contains(&member.receipt().claim().claim_id));
        assert_eq!(member.receipt().model(), &model);
    }
}

#[test]
fn joint_pairing_changes_recovery_while_marginals_stay_the_same() {
    let model = model("x*y", &["x", "y"], None);
    let evaluate = |pairs: [[&str; 2]; 2]| {
        let scenarios = pairs.iter().enumerate().map(|(i, pair)|
            scenario(&model, &i.to_string(), "0.5", &[("x", pair[0]), ("y", pair[1])])
        ).collect();
        model.evaluate_ensemble(&population(RecoveryPopulation { scenarios })).unwrap()
    };
    let paired = evaluate([["0", "0"], ["1", "1"]]);
    let opposed = evaluate([["0", "1"], ["1", "0"]]);
    close(paired.claim().value.mean.value(), 0.5);
    close(opposed.claim().value.mean.value(), 0.0);
    close(opposed.claim().value.variance.value(), 0.0);
    assert_ne!(paired.digest(), opposed.digest());
}

#[test]
fn cap_is_applied_per_member_before_weighted_summary() {
    let model = model("g", &["g"], Some("0.8"));
    let receipt = model.evaluate_ensemble(&population(binary(&model, "0.6", "1.2"))).unwrap();
    close(receipt.claim().value.mean.value(), 0.7);
    close(receipt.claim().value.variance.value(), 0.01);
    assert_eq!(receipt.claim().value.probability_capped, q("0.5"));
    let high = &receipt.members()[0];
    close(high.receipt().claim().value.corrected_before_cap.value(), 1.2);
    close(high.receipt().claim().value.recovery_fraction.value(), 0.8);
    assert!(high.receipt().claim().value.cap_applied);
    assert_eq!(high.receipt().result_json(), "{\"test_work_fraction\":1.2,\"operational_fractions\":[],\"corrected_before_cap\":1.2,\"cap_applied\":true,\"recovery_fraction\":0.8,\"route\":\"declared-process\"}");
}

#[test]
fn exact_weight_boundaries_choose_lower_quantile_without_interpolation() {
    let model = model("g", &["g"], None);
    let scenarios = vec![
        scenario(&model, "first", "0.1", &[("g", "0.1")]),
        scenario(&model, "middle", "0.4", &[("g", "0.5")]),
        scenario(&model, "last", "0.5", &[("g", "0.9")]),
    ];
    let receipt = model.evaluate_ensemble(&population(RecoveryPopulation { scenarios })).unwrap();
    let value = &receipt.claim().value;
    assert_eq!([value.p10.value(), value.p50.value(), value.p90.value()], [0.1, 0.5, 0.9]);
    close(value.mean.value(), 0.66);
    close(value.variance.value(), 0.0704);
}

#[test]
fn member_permutation_preserves_claim_digest_and_both_renderings() {
    let model = model("g", &["g"], None);
    let a = binary(&model, "0.1", "0.9");
    let mut b = a.clone();
    b.scenarios.reverse();
    let a = model.evaluate_ensemble(&population(a)).unwrap();
    let b = model.evaluate_ensemble(&population(b)).unwrap();
    assert_eq!(a.digest(), b.digest());
    assert_eq!(a.claim(), b.claim());
    assert_eq!(a.to_json(), b.to_json());
    assert_eq!(a.to_markdown(), b.to_markdown());
    assert!(a.to_json().contains("\"calibration\":\"uncalibrated\""));
    assert!(a.to_json().contains("\"probability_capped\":{\"numerator\":\"0\",\"denominator\":\"1\"}"));
    assert!(a.to_markdown().contains("no interpolation"));
}

#[test]
fn invalid_weights_names_and_population_sizes_are_never_normalized() {
    let model = model("g", &["g"], None);
    for weight in ["0", "-0.5", "0.4", "1"] {
        let mut value = binary(&model, "0.1", "0.9");
        value.scenarios[0].weight = q(weight);
        assert_eq!(model.evaluate_ensemble(&population(value)).unwrap_err().code,
            RefusalCode::OutsidePhysicalDomain109.as_str());
    }
    let mut duplicate = binary(&model, "0.1", "0.9");
    duplicate.scenarios[1].id = duplicate.scenarios[0].id.clone();
    assert_eq!(model.evaluate_ensemble(&population(duplicate)).unwrap_err().code,
        RefusalCode::BasisMismatch105.as_str());
    for name in ["", " spaced", "line\nbreak"] {
        let mut value = binary(&model, "0.1", "0.9");
        value.scenarios[0].id = name.into();
        assert_eq!(model.evaluate_ensemble(&population(value)).unwrap_err().code,
            RefusalCode::NoClaimInDomain201.as_str());
    }
    for scenarios in [vec![], vec![scenario(&model, "one", "1", &[("g", "0.5")]); MAX_SCENARIOS + 1]] {
        assert_eq!(model.evaluate_ensemble(&population(RecoveryPopulation { scenarios })).unwrap_err().code,
            RefusalCode::NoClaimInDomain201.as_str());
    }
}

#[test]
fn every_invalid_member_refuses_even_a_small_probability_state() {
    let model = model("g/10", &["g"], None);
    for (case, expected) in [(0, RefusalCode::OutsideAoa203), (1, RefusalCode::SupportMismatch102),
        (2, RefusalCode::SupportMismatch102), (3, RefusalCode::UndeclaredUnitBasisOrTz107),
        (4, RefusalCode::NoClaimInDomain201)] {
        let mut value = binary(&model, "1", "2");
        value.scenarios[0].weight = q("0.000001");
        value.scenarios[1].weight = q("0.999999");
        let mut input = value.scenarios[0].inputs.value.clone();
        match case {
            0 => input.values.get_mut("g").unwrap().value = q("11"),
            1 => input.interval.end_ns = 21,
            2 => input.scope.circuit = "other-circuit".into(),
            3 => input.values.get_mut("g").unwrap().unit = InputUnit::DryMassPercent,
            _ => { input.values.clear(); }
        }
        value.scenarios[0].inputs = input_claim(input);
        assert_eq!(model.evaluate_ensemble(&population(value)).unwrap_err().code, expected.as_str());
    }
}

#[test]
fn source_metadata_joins_and_typed_population_mutation_refuses() {
    let model = model("g", &["g"], None);
    let mut value = binary(&model, "0.1", "0.9");
    let old = &value.scenarios[0].inputs;
    value.scenarios[0].inputs = Claim::new(
        &old.metric_key, old.value.clone(), old.value_tag(), Representation::Deterministic,
        old.support, old.additivity, old.evidence, old.epistemic, old.state,
        Confidentiality::internal(), vec![UuTerm { scenario_id: "transfer".into(),
            description: "authored structural transfer risk".into(), downside_exposure_usd_m: 1.0,
            trigger_indicator: "change in mineralogy".into() }], vec![], old.anchors.clone(),
    );
    let mut declaration = population(value);
    let receipt = model.evaluate_ensemble(&declaration).unwrap();
    assert_eq!(receipt.claim().confidentiality, Confidentiality::internal());
    assert_eq!(receipt.claim().uu_terms.len(), 1);
    assert!(receipt.to_json().contains("authored structural transfer risk"));
    declaration.value.scenarios[0].weight = q("0.6");
    assert_eq!(model.evaluate_ensemble(&declaration).unwrap_err().code,
        RefusalCode::FrozenRecordMutation507.as_str());
    let mut declaration = receipt.population().clone();
    declaration.value.scenarios[0].inputs.value.values.get_mut("g").unwrap().value = q("0.2");
    assert_eq!(model.evaluate_ensemble(&declaration).unwrap_err().code,
        RefusalCode::FrozenRecordMutation507.as_str());
}

#[test]
fn centered_variance_preserves_small_spread_and_rare_realizations() {
    let model = model("g", &["g"], None);
    let receipt = model.evaluate_ensemble(&population(binary(&model, "0.5", "0.50000001"))).unwrap();
    let variance = receipt.claim().value.variance.value();
    assert!((variance / 2.5e-17 - 1.0).abs() < 2e-8, "{variance}");
    let mut value = binary(&model, "0", "1");
    value.scenarios[0].weight = q("0.999999999999999999999999999999");
    value.scenarios[1].weight = q("0.000000000000000000000000000001");
    let receipt = model.evaluate_ensemble(&population(value)).unwrap();
    assert!((receipt.claim().value.mean.value() / 1e-30 - 1.0).abs() < 1e-14);
    assert!((receipt.claim().value.variance.value() / 1e-30 - 1.0).abs() < 1e-14);
    assert_eq!(receipt.claim().value.p90.value(), 0.0);
}

#[test]
fn renderers_escape_names_and_retain_complete_member_evaluations() {
    let model = model("g", &["g"], None);
    let mut value = binary(&model, "0.1", "0.9");
    value.scenarios[0].id = "low|<tag>\"\\name".into();
    let receipt = model.evaluate_ensemble(&population(value)).unwrap();
    let json = receipt.to_json();
    assert!(json.contains("low|<tag>\\\"\\\\name"));
    assert_eq!(json.matches("\"corrected_before_cap\"").count(), 2);
    assert!(receipt.to_markdown().contains("low&#124;&lt;tag&gt;"));
    for member in receipt.members() {
        assert!(json.contains(member.receipt().digest()));
        assert!(json.contains(&member.receipt().inputs().claim_id.to_string()));
    }
}
