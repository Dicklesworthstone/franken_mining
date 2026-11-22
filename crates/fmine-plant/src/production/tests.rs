//! FACT: invented coupled populations with hand-computed mass/energy answers.
//! Source bytes and uncertainty are declarations, not operating observations.
//! Determinism: DeterministicPerIsa

use super::*;
use crate::cards::{
    CardVersion, InputRequirement, InputValue, OutputScale, RecoveryCorrection, RecoveryInputs,
    RecoveryModel, RouteAssumption, TestWorkCard,
    ensemble::{RecoveryPopulation, RecoveryScenario},
};
use crate::grinding::{self, BondModel, GrindingScenario, Kilowatts, KwhPerDryTonne, Micrometres};
use fmine_claim::{Confidentiality, UuTerm};
use fmine_core::{anchor::SourceAnchor, hash::sha256};
use fmine_evidence::EpistemicStatus;
use fmine_time::ValidTime;

fn q(n: i128, d: i128) -> ExactQty {
    ExactQty::new(n, d).unwrap()
}
fn source<T>(metric: &str, value: T, tag: String) -> Claim<T> {
    Claim::new(
        metric,
        value,
        &tag,
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
            "crates/fmine-plant/src/production/tests.rs",
            sha256(include_bytes!("tests.rs")).0,
            "authored joint production assumptions",
        )],
    )
}
fn input_claim(value: ProductionInputs) -> Claim<ProductionInputs> {
    let tag = value.canonical_tag();
    source(INPUT_METRIC, value, tag)
}
fn supply_claim(value: ProductionSupply) -> Claim<ProductionSupply> {
    let tag = value.canonical_tag();
    source(SUPPLY_METRIC, value, tag)
}
fn supplied(inputs: &Claim<ProductionInputs>, tonnes: [ExactQty; 2]) -> Claim<ProductionSupply> {
    supply_claim(ProductionSupply {
        grinding_scope: inputs.value.grinding_scope.clone(),
        recovery_scope: inputs.value.recovery_scope.clone(),
        supply_basis: "Synthetic complete feed available before this exposure".into(),
        available_at_start: true,
        unchanged_operating_point: true,
        scenarios: inputs
            .value
            .scenarios
            .iter()
            .zip(tonnes)
            .map(|(state, available)| SupplyScenario {
                id: state.id.clone(),
                weight: state.weight,
                available_dry_tonnes: available,
            })
            .collect(),
    })
}
fn scope() -> GrindingScope {
    GrindingScope {
        boundary: "synthetic-mill-boundary".into(),
        circuit: "synthetic-circuit".into(),
        blend: "synthetic-hour-blend".into(),
        interval: ValidTime {
            start_ns: 0,
            end_ns: 3_600_000_000_000,
        },
    }
}
fn recovery_scope() -> RecoveryScope {
    RecoveryScope {
        circuit: "synthetic-circuit".into(),
        material_domain: "synthetic-domain".into(),
        dry_basis: "total-dry-matter/v1".into(),
        element: "Au".into(),
    }
}
fn grinding(utilization: [f64; 2]) -> GrindingReceipt {
    let value = BondModel {
        scope: scope(),
        id: "synthetic-bond".into(),
        version: "1".into(),
        work_index_test_basis: "authored metric work index".into(),
        validity_basis: "authored closed box".into(),
        complete_joint_population: true,
        work_index_domain: [KwhPerDryTonne::new(1.0), KwhPerDryTonne::new(100.0)],
        feed_size_domain: [Micrometres::new(100.0), Micrometres::new(2000.0)],
        product_size_domain: [Micrometres::new(1.0), Micrometres::new(500.0)],
        power_domain: [Kilowatts::new(0.0), Kilowatts::new(10_000.0)],
        utilization_domain: [grinding::Fraction::new(0.0), grinding::Fraction::new(1.0)],
    };
    let tag = value.canonical_tag();
    let model = source(grinding::MODEL_METRIC, value, tag);
    let rows = ["a", "b"]
        .into_iter()
        .enumerate()
        .map(|(index, id)| {
            let value = GrindingScenario {
                id: id.into(),
                scope: scope(),
                weight: q(1, 2),
                work_index: KwhPerDryTonne::new(if index == 0 { 10.0 } else { 20.0 }),
                feed_size: Micrometres::new(400.0),
                product_size: Micrometres::new(100.0),
                installed_power: Kilowatts::new(1000.0),
                utilization: grinding::Fraction::new(utilization[index]),
            };
            let tag = value.canonical_tag();
            source(grinding::INPUT_METRIC, value, tag)
        })
        .collect::<Vec<_>>();
    grinding::evaluate(&model, &rows).unwrap()
}
fn version(id: &str) -> CardVersion {
    CardVersion {
        id: id.into(),
        revision: "synthetic-v1".into(),
    }
}
fn recovery(
    grades: [ExactQty; 2],
    fractions: [ExactQty; 2],
    grade_unit: InputUnit,
) -> RecoveryEnsembleReceipt {
    let specifications = [
        ("g", "total-Au/dry", grade_unit, q(0, 1), q(10, 1)),
        (
            "p80",
            "passing-size",
            InputUnit::Micrometres,
            q(1, 1),
            q(500, 1),
        ),
        (
            "r",
            "declared-response",
            InputUnit::DryMassFraction,
            q(0, 1),
            q(1, 1),
        ),
    ];
    let value = TestWorkCard {
        version: version("test-work"),
        scope: recovery_scope(),
        valid_time: scope().interval,
        input_units: specifications
            .iter()
            .map(|(name, measurand, unit, low, high)| {
                (
                    name.to_string(),
                    InputRequirement {
                        measurand: measurand.to_string(),
                        unit: *unit,
                        lower: *low,
                        upper: *high,
                        lower_inclusive: true,
                        upper_inclusive: true,
                    },
                )
            })
            .collect(),
        formula: "r".into(),
        output_scale: OutputScale::Fraction,
    };
    let tag = value.canonical_tag();
    let test_work = source(cards::TEST_WORK_METRIC, value, tag);
    let value = RecoveryCorrection {
        version: version("correction"),
        test_work: test_work.claim_id,
        additive_fraction: ZERO,
        upper_cap_fraction: None,
    };
    let tag = value.canonical_tag();
    let correction = source(cards::CORRECTION_METRIC, value, tag);
    let value = RouteAssumption {
        version: version("route"),
        test_work: test_work.claim_id,
        route: "synthetic-route".into(),
    };
    let tag = value.canonical_tag();
    let route = source(cards::ROUTE_METRIC, value, tag);
    let model = RecoveryModel::compile(&test_work, &[], &correction, &route).unwrap();
    let scenarios = ["a", "b"]
        .into_iter()
        .enumerate()
        .map(|(index, id)| {
            let values = [grades[index], q(100, 1), fractions[index]];
            let value = RecoveryInputs {
                scope: recovery_scope(),
                interval: scope().interval,
                values: specifications
                    .iter()
                    .zip(values)
                    .map(|((name, measurand, unit, _, _), value)| {
                        (
                            name.to_string(),
                            InputValue {
                                measurand: measurand.to_string(),
                                unit: *unit,
                                value,
                            },
                        )
                    })
                    .collect(),
            };
            let tag = value.canonical_tag();
            RecoveryScenario {
                id: id.into(),
                weight: q(1, 2),
                inputs: source(cards::INPUT_METRIC, value, tag),
            }
        })
        .collect();
    let value = RecoveryPopulation { scenarios };
    let tag = value.canonical_tag();
    model
        .evaluate_ensemble(&source(cards::ensemble::INPUT_METRIC, value, tag))
        .unwrap()
}
fn declarations(grades: [ExactQty; 2], hours: [ExactQty; 2]) -> Claim<ProductionInputs> {
    input_claim(ProductionInputs {
        grinding_scope: scope(),
        recovery_scope: recovery_scope(),
        coupling_basis: "Declared synthetic blend has the named recovery material and dry state"
            .into(),
        grade_coordinate: "g".into(),
        product_size_coordinate: "p80".into(),
        scenarios: ["a", "b"]
            .into_iter()
            .enumerate()
            .map(|(i, id)| ProductionScenario {
                id: id.into(),
                weight: q(1, 2),
                grade_g_per_dry_t: grades[i],
                scheduled_hours: hours[i],
            })
            .collect(),
    })
}
fn fixture() -> (
    GrindingReceipt,
    RecoveryEnsembleReceipt,
    Claim<ProductionInputs>,
) {
    (
        grinding([0.8, 0.8]),
        recovery(
            [q(2, 1); 2],
            [q(9, 10), q(1, 2)],
            InputUnit::GramsPerDryMetricTonne,
        ),
        declarations([q(2, 1); 2], [ONE; 2]),
    )
}
fn close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() <= expected.abs().max(1.0) * 2e-14,
        "{actual} != {expected}"
    );
}

#[test]
fn products_are_computed_per_shared_state_before_averaging() {
    let (grinding, recovery, inputs) = fixture();
    let receipt = evaluate(&grinding, &recovery, &inputs).unwrap();
    // Independent arithmetic: Q={800/5,800/10}; metal={160*2*.9,80*2*.5} grams.
    close(receipt.points()[0].dry_tonnes.value(), 160.0);
    close(receipt.points()[1].dry_tonnes.value(), 80.0);
    close(receipt.points()[0].recovered_metal.value(), 0.288);
    close(receipt.points()[1].recovered_metal.value(), 0.080);
    close(receipt.summary().contained_metal.mean.value(), 0.240);
    close(receipt.summary().recovered_metal.mean.value(), 0.184);
    close(receipt.summary().recovered_metal.p10.value(), 0.080);
    close(receipt.summary().recovered_metal.p50.value(), 0.080);
    close(receipt.summary().recovered_metal.p90.value(), 0.288);
    close(receipt.summary().throughput.mean.value(), 120.0);
    close(receipt.summary().recovery.mean.value(), 0.7);
    assert_ne!(
        receipt.summary().recovered_metal.mean.value(),
        120.0 * 2.0 * 0.7 / 1000.0
    );
    assert_eq!(
        receipt
            .points()
            .iter()
            .map(|p| p.weight)
            .collect::<Vec<_>>(),
        [q(1, 2); 2]
    );
    assert!(receipt.claim().parents.contains(&grinding.claim().claim_id));
    assert!(receipt.claim().parents.contains(&recovery.claim().claim_id));
    assert!(receipt.claim().parents.contains(&inputs.claim_id));
    assert_eq!(receipt.claim().evidence, EvidenceClass::Assumed);
}

#[test]
fn identical_marginals_with_opposite_dependence_have_different_production() {
    let (grinding, original, inputs) = fixture();
    let reverse = recovery(
        [q(2, 1); 2],
        [q(1, 2), q(9, 10)],
        InputUnit::GramsPerDryMetricTonne,
    );
    let first = evaluate(&grinding, &original, &inputs).unwrap();
    let second = evaluate(&grinding, &reverse, &inputs).unwrap();
    assert_eq!(first.summary().throughput, second.summary().throughput);
    assert_eq!(first.summary().recovery, second.summary().recovery);
    close(first.summary().recovered_metal.mean.value(), 0.184);
    close(second.summary().recovered_metal.mean.value(), 0.152);
    assert_ne!(first.digest(), second.digest());
}

#[test]
fn unequal_exact_weights_keep_quantile_boundary_mass() {
    let (grinding, recovery, inputs) = fixture();
    let weights = [q(1, 10), q(9, 10)];
    let scenarios = grinding
        .scenarios()
        .iter()
        .zip(weights)
        .map(|(old, weight)| {
            let mut value = old.value.clone();
            value.weight = weight;
            let tag = value.canonical_tag();
            source(grinding::INPUT_METRIC, value, tag)
        })
        .collect::<Vec<_>>();
    let grinding = grinding::evaluate(grinding.model(), &scenarios).unwrap();
    let mut population = recovery.population().value.clone();
    for (scenario, weight) in population.scenarios.iter_mut().zip(weights) {
        scenario.weight = weight;
    }
    let tag = population.canonical_tag();
    let recovery = recovery
        .model()
        .evaluate_ensemble(&source(cards::ensemble::INPUT_METRIC, population, tag))
        .unwrap();
    let mut values = inputs.value;
    for (scenario, weight) in values.scenarios.iter_mut().zip(weights) {
        scenario.weight = weight;
    }
    let receipt = evaluate(&grinding, &recovery, &input_claim(values)).unwrap();
    close(receipt.summary().recovered_metal.mean.value(), 0.1008);
    close(receipt.summary().recovered_metal.p90.value(), 0.080);
    close(receipt.summary().recovered_metal.maximum.value(), 0.288);
}

#[test]
fn scheduled_hours_do_not_charge_utilization_again_and_zero_states_are_retained() {
    let (grinding, recovery, _) = fixture();
    let half_hour = declarations([q(2, 1); 2], [q(1, 2); 2]);
    let receipt = evaluate(&grinding, &recovery, &half_hour).unwrap();
    close(receipt.points()[0].dry_tonnes.value(), 80.0);
    close(receipt.summary().recovered_metal.mean.value(), 0.092);
    let no_hours = declarations([q(2, 1); 2], [ZERO; 2]);
    let receipt = evaluate(&grinding, &recovery, &no_hours).unwrap();
    assert_eq!(receipt.points().len(), 2);
    assert_eq!(receipt.summary().recovered_metal.maximum.value(), 0.0);

    let stopped = self::grinding([0.0, 0.8]);
    let full = declarations([q(2, 1); 2], [ONE; 2]);
    let receipt = evaluate(&stopped, &recovery, &full).unwrap();
    assert_eq!(receipt.points()[0].dry_tonnes.value(), 0.0);
    close(receipt.summary().recovered_metal.mean.value(), 0.040);
    let barren = self::recovery(
        [ZERO; 2],
        [q(9, 10), q(1, 2)],
        InputUnit::GramsPerDryMetricTonne,
    );
    let receipt = evaluate(&grinding, &barren, &declarations([ZERO; 2], [ONE; 2])).unwrap();
    close(receipt.summary().dry_tonnes.mean.value(), 120.0);
    assert_eq!(receipt.summary().contained_metal.maximum.value(), 0.0);
}

#[test]
fn missing_duplicate_and_differently_weighted_scenarios_refuse_the_whole_join() {
    let (grinding, recovery, source) = fixture();
    let mut value = source.value.clone();
    value.scenarios[0].id = "absent".into();
    assert_eq!(
        evaluate(&grinding, &recovery, &input_claim(value))
            .unwrap_err()
            .code,
        "FMINE-REF-105"
    );
    let mut value = source.value.clone();
    value.scenarios[0].id = "b".into();
    assert_eq!(
        evaluate(&grinding, &recovery, &input_claim(value))
            .unwrap_err()
            .code,
        "FMINE-REF-105"
    );
    let mut value = source.value.clone();
    value.scenarios[0].weight = q(3, 4);
    assert_eq!(
        evaluate(&grinding, &recovery, &input_claim(value))
            .unwrap_err()
            .code,
        "FMINE-REF-105"
    );
    let mut value = source.value.clone();
    value.scenarios.pop();
    assert_eq!(
        evaluate(&grinding, &recovery, &input_claim(value))
            .unwrap_err()
            .code,
        "FMINE-REF-105"
    );
    let mut value = source.value;
    value.scenarios.clear();
    assert_eq!(
        evaluate(&grinding, &recovery, &input_claim(value))
            .unwrap_err()
            .code,
        "FMINE-REF-201"
    );
}

#[test]
fn mismatched_scopes_and_unlinked_coordinate_values_never_multiply() {
    let (grinding, recovery, source) = fixture();
    let mut value = source.value.clone();
    value.grinding_scope.interval.end_ns += 1;
    assert_eq!(
        evaluate(&grinding, &recovery, &input_claim(value))
            .unwrap_err()
            .code,
        "FMINE-REF-102"
    );
    let mut value = source.value.clone();
    value.recovery_scope.material_domain = "other-ore".into();
    assert_eq!(
        evaluate(&grinding, &recovery, &input_claim(value))
            .unwrap_err()
            .code,
        "FMINE-REF-102"
    );
    let mut value = source.value.clone();
    value.grinding_scope.blend = "different-blend".into();
    assert_eq!(
        evaluate(&grinding, &recovery, &input_claim(value))
            .unwrap_err()
            .code,
        "FMINE-REF-102"
    );
    let mut value = source.value.clone();
    value.scenarios[0].grade_g_per_dry_t = q(3, 1);
    assert_eq!(
        evaluate(&grinding, &recovery, &input_claim(value))
            .unwrap_err()
            .code,
        "FMINE-REF-105"
    );
    let mut value = source.value.clone();
    value.product_size_coordinate = "g".into();
    assert_eq!(
        evaluate(&grinding, &recovery, &input_claim(value))
            .unwrap_err()
            .code,
        "FMINE-REF-107"
    );
    let mut value = source.value.clone();
    value.product_size_coordinate = "missing".into();
    assert_eq!(
        evaluate(&grinding, &recovery, &input_claim(value))
            .unwrap_err()
            .code,
        "FMINE-REF-201"
    );
    let percent = self::recovery([q(2, 1); 2], [q(9, 10), q(1, 2)], InputUnit::DryMassPercent);
    assert_eq!(
        evaluate(&grinding, &percent, &source).unwrap_err().code,
        "FMINE-REF-107"
    );
    let mut scenarios = grinding.scenarios().to_vec();
    let mut changed = scenarios[0].value.clone();
    changed.product_size = Micrometres::new(81.0);
    let tag = changed.canonical_tag();
    scenarios[0] = self::source(grinding::INPUT_METRIC, changed, tag);
    let changed_grinding = grinding::evaluate(grinding.model(), &scenarios).unwrap();
    assert_eq!(
        evaluate(&changed_grinding, &recovery, &source)
            .unwrap_err()
            .code,
        "FMINE-REF-105"
    );
}

#[test]
fn explicit_hours_and_material_domain_are_guarded_and_input_edits_are_detected() {
    let (grinding, recovery, source) = fixture();
    for (grade, hours) in [
        (q(-1, 1), ONE),
        (q(1_000_001, 1), ONE),
        (q(2, 1), q(-1, 1)),
        (q(2, 1), q(3, 2)),
    ] {
        let mut value = source.value.clone();
        value.scenarios[0].grade_g_per_dry_t = grade;
        value.scenarios[0].scheduled_hours = hours;
        assert_eq!(
            evaluate(&grinding, &recovery, &input_claim(value))
                .unwrap_err()
                .code,
            "FMINE-REF-109"
        );
    }
    let mut changed = source.clone();
    changed.value.scenarios[0].scheduled_hours = q(1, 2);
    assert_eq!(
        evaluate(&grinding, &recovery, &changed).unwrap_err().code,
        "FMINE-REF-507"
    );
    let mut stale = source.clone();
    stale.stale = true;
    assert_eq!(
        evaluate(&grinding, &recovery, &stale).unwrap_err().code,
        "FMINE-REF-502"
    );
    let mut unavailable = source.clone();
    unavailable.calibration_failed = true;
    assert_eq!(
        evaluate(&grinding, &recovery, &unavailable)
            .unwrap_err()
            .code,
        "FMINE-REF-205"
    );
    let mut value = source.value;
    value.coupling_basis.clear();
    assert_eq!(
        evaluate(&grinding, &recovery, &input_claim(value))
            .unwrap_err()
            .code,
        "FMINE-REF-107"
    );
}

#[test]
fn canonical_order_and_full_envelope_binding_are_preserved() {
    let (grinding, recovery, source) = fixture();
    let first = evaluate(&grinding, &recovery, &source).unwrap();
    let mut reversed = source.value.clone();
    reversed.scenarios.reverse();
    let second = evaluate(&grinding, &recovery, &input_claim(reversed)).unwrap();
    assert_eq!(first.digest(), second.digest());
    assert_eq!(first.claim(), second.claim());
    assert_eq!(first.points(), second.points());
    let mut with_uu = source;
    with_uu.uu_terms.push(UuTerm {
        scenario_id: "joint-link-risk".into(),
        description: "Declared simultaneous coupling may be wrong".into(),
        downside_exposure_usd_m: 1.0,
        trigger_indicator: "Independently observed production outside the declared population"
            .into(),
    });
    let third = evaluate(&grinding, &recovery, &with_uu).unwrap();
    assert_ne!(first.digest(), third.digest());
    assert_eq!(third.claim().uu_terms.len(), 1);
    assert_eq!(third.grinding().digest(), grinding.digest());
    assert_eq!(third.recovery().digest(), recovery.digest());
}

#[test]
fn extreme_products_refuse_instead_of_emitting_nonfinite_or_silent_zero() {
    assert_eq!(product(f64::MAX, 2.0).unwrap_err().code, "FMINE-REF-207");
    assert_eq!(
        product(f64::MIN_POSITIVE, f64::MIN_POSITIVE)
            .unwrap_err()
            .code,
        "FMINE-REF-207"
    );
    assert_eq!(product(0.0, f64::MAX).unwrap(), 0.0);
}

#[test]
fn explicit_supply_preserves_joint_states_and_reports_capacity_and_unused_feed() {
    let (grinding, recovery, inputs) = fixture();
    let supply = supplied(&inputs, [q(100, 1), q(120, 1)]);
    let receipt = evaluate_supply_limited(&grinding, &recovery, &inputs, &supply).unwrap();
    // Independent min(160,100), min(80,120); grade 2, recovery .9/.5.
    close(receipt.summary().dry_tonnes.mean.value(), 90.0);
    close(receipt.summary().contained_metal.mean.value(), 0.18);
    close(receipt.summary().recovered_metal.mean.value(), 0.13);
    close(receipt.summary().throughput.mean.value(), 120.0);
    let a = receipt.points()[0].supply_use.as_ref().unwrap();
    let b = receipt.points()[1].supply_use.as_ref().unwrap();
    assert_eq!(a.binding_constraint, BindingConstraint::OreSupply);
    assert_eq!(b.binding_constraint, BindingConstraint::GrindingCapacity);
    close(a.capacity_dry_tonnes.value(), 160.0);
    close(a.unused_capacity_dry_tonnes.value(), 60.0);
    close(a.unused_dry_tonnes.value(), 0.0);
    close(a.used_scheduled_hours.value(), 0.625);
    close(a.effective_throughput.value(), 100.0);
    close(b.unused_dry_tonnes.value(), 40.0);
    close(b.unused_capacity_dry_tonnes.value(), 0.0);
    close(b.used_scheduled_hours.value(), 1.0);
    assert_eq!(receipt.supply(), Some(&supply));
    assert!(receipt.claim().parents.contains(&supply.claim_id));
    assert_eq!(receipt.claim().metric_key, SUPPLY_OUTPUT_METRIC);
    assert_eq!(receipt.claim().evidence, EvidenceClass::Assumed);
    let legacy = evaluate(&grinding, &recovery, &inputs).unwrap();
    assert!(legacy.supply().is_none());
    assert!(
        legacy
            .points()
            .iter()
            .all(|point| point.supply_use.is_none())
    );
    assert_eq!(legacy.claim().metric_key, OUTPUT_METRIC);
    close(legacy.summary().recovered_metal.mean.value(), 0.184);
    assert_ne!(receipt.digest(), legacy.digest());
}

#[test]
fn supply_zero_exposure_shutdown_and_exact_capacity_ties_keep_all_states() {
    let (grinding, recovery, inputs) = fixture();
    let supply = supplied(&inputs, [ZERO, q(80, 1)]);
    let receipt = evaluate_supply_limited(&grinding, &recovery, &inputs, &supply).unwrap();
    assert_eq!(receipt.points().len(), 2);
    assert_eq!(receipt.points()[0].dry_tonnes.value(), 0.0);
    let a = receipt.points()[0].supply_use.as_ref().unwrap();
    assert_eq!(a.binding_constraint, BindingConstraint::OreSupply);
    assert_eq!(a.used_scheduled_hours.value(), 0.0);
    assert_eq!(a.effective_throughput.value(), 0.0);
    assert_eq!(
        receipt.points()[1]
            .supply_use
            .as_ref()
            .unwrap()
            .binding_constraint,
        BindingConstraint::Both
    );

    let no_hours = declarations([q(2, 1); 2], [ZERO; 2]);
    let supply = supplied(&no_hours, [ZERO, q(100, 1)]);
    let receipt = evaluate_supply_limited(&grinding, &recovery, &no_hours, &supply).unwrap();
    for point in receipt.points() {
        assert_eq!(point.dry_tonnes.value(), 0.0);
        let use_ = point.supply_use.as_ref().unwrap();
        assert_eq!(use_.used_scheduled_hours.value(), 0.0);
        assert_eq!(use_.effective_throughput.value(), 0.0);
    }
    assert_eq!(
        receipt.points()[0]
            .supply_use
            .as_ref()
            .unwrap()
            .binding_constraint,
        BindingConstraint::Both
    );
    let stopped = self::grinding([0.0, 0.8]);
    let supply = supplied(&inputs, [q(100, 1); 2]);
    let receipt = evaluate_supply_limited(&stopped, &recovery, &inputs, &supply).unwrap();
    let a = receipt.points()[0].supply_use.as_ref().unwrap();
    assert_eq!(a.binding_constraint, BindingConstraint::GrindingCapacity);
    assert_eq!(a.unused_dry_tonnes.value(), 100.0);
    assert_eq!(a.used_scheduled_hours.value(), 0.0);
}

#[test]
fn available_feed_requires_complete_matching_sources_and_explicit_physical_assumptions() {
    let (grinding, recovery, inputs) = fixture();
    let supply = supplied(&inputs, [q(100, 1); 2]);
    let run = |value: ProductionSupply| {
        evaluate_supply_limited(&grinding, &recovery, &inputs, &supply_claim(value))
            .unwrap_err()
            .code
    };
    let mut changed = supply.value.clone();
    changed.scenarios.pop();
    assert_eq!(run(changed), "FMINE-REF-105");
    let mut changed = supply.value.clone();
    changed.scenarios[0].id = "b".into();
    assert_eq!(run(changed), "FMINE-REF-105");
    let mut changed = supply.value.clone();
    changed.scenarios[0].id = "other".into();
    assert_eq!(run(changed), "FMINE-REF-105");
    let mut changed = supply.value.clone();
    changed.scenarios[0].weight = q(1, 3);
    assert_eq!(run(changed), "FMINE-REF-105");
    let mut changed = supply.value.clone();
    changed.scenarios[0].available_dry_tonnes = q(-1, 1);
    assert_eq!(run(changed), "FMINE-REF-109");
    let mut changed = supply.value.clone();
    changed.grinding_scope.blend = "another-feed".into();
    assert_eq!(run(changed), "FMINE-REF-102");
    let mut changed = supply.value.clone();
    changed.recovery_scope.dry_basis = "unmatched-moisture-state".into();
    assert_eq!(run(changed), "FMINE-REF-102");
    let mut changed = supply.value.clone();
    changed.available_at_start = false;
    assert_eq!(run(changed), "FMINE-REF-201");
    let mut changed = supply.value.clone();
    changed.unchanged_operating_point = false;
    assert_eq!(run(changed), "FMINE-REF-201");
    let mut changed = supply.value.clone();
    changed.supply_basis.clear();
    assert_eq!(run(changed), "FMINE-REF-201");
    let mut tampered = supply.clone();
    tampered.value.scenarios[0].available_dry_tonnes = q(99, 1);
    assert_eq!(
        evaluate_supply_limited(&grinding, &recovery, &inputs, &tampered)
            .unwrap_err()
            .code,
        "FMINE-REF-507"
    );
    let mut stale = supply.clone();
    stale.stale = true;
    assert_eq!(
        evaluate_supply_limited(&grinding, &recovery, &inputs, &stale)
            .unwrap_err()
            .code,
        "FMINE-REF-502"
    );
    let mut failed = supply;
    failed.calibration_failed = true;
    assert_eq!(
        evaluate_supply_limited(&grinding, &recovery, &inputs, &failed)
            .unwrap_err()
            .code,
        "FMINE-REF-205"
    );
    // Rounding must not silently turn unequal exact supply/capacity into a tie.
    assert_eq!(
        supply_use(
            q(1_600_000_000_000_000_001, 10_000_000_000_000_000),
            160.0,
            160.0,
            ONE
        )
        .unwrap_err()
        .code,
        "FMINE-REF-207"
    );
}

#[test]
fn supply_population_order_and_full_source_envelope_bind_the_receipt() {
    let (grinding, recovery, inputs) = fixture();
    let supply = supplied(&inputs, [q(100, 1); 2]);
    let first = evaluate_supply_limited(&grinding, &recovery, &inputs, &supply).unwrap();
    let mut value = supply.value.clone();
    value.scenarios.reverse();
    let second =
        evaluate_supply_limited(&grinding, &recovery, &inputs, &supply_claim(value)).unwrap();
    assert_eq!(first.digest(), second.digest());
    assert_eq!(first.points(), second.points());
    let mut restricted = Claim::new(
        SUPPLY_METRIC,
        supply.value.clone(),
        supply.value_tag(),
        Representation::Deterministic,
        SpatialSupport::PeriodAggregate,
        AdditivityClass::NonAdditive,
        EvidenceClass::Assumed,
        EpistemicStatus::KnownKnown,
        KnowledgeState::Known,
        Confidentiality::internal(),
        vec![],
        vec![],
        supply.anchors.clone(),
    );
    restricted.uu_terms.push(UuTerm {
        scenario_id: "feed-readiness".into(),
        description: "Declared feed may arrive late".into(),
        downside_exposure_usd_m: 1.0,
        trigger_indicator: "Ore unavailable when scheduled processing starts".into(),
    });
    let changed = evaluate_supply_limited(&grinding, &recovery, &inputs, &restricted).unwrap();
    assert_ne!(first.digest(), changed.digest());
    assert_eq!(changed.points(), first.points());
    assert_eq!(changed.claim().confidentiality, Confidentiality::internal());
    assert_eq!(changed.claim().uu_terms.len(), 1);
}

#[test]
fn supply_conversion_cannot_reverse_a_binding_constraint() {
    // Independent rational comparison to binary64 0.1 = 3602879701896397/2^55.
    // Casting this numerator/denominator separately rounds the ratio upward
    // across capacity, even though the exact source is strictly below it.
    let below = q(
        2_294_532_357_039_142_209_280,
        22_945_323_570_391_420_819_079,
    );
    assert!(
        below
            .checked_cmp(crate::reconciliation::represented_rational(0.1).unwrap())
            .unwrap()
            .is_lt()
    );
    assert!(below.to_f64() > 0.1);
    assert_eq!(
        supply_use(below, 0.1, 0.1, ONE).unwrap_err().code,
        "FMINE-REF-207"
    );
    let above = q(375_569_884_728_771_033, 3_755_698_847_287_710_121);
    assert!(
        above
            .checked_cmp(crate::reconciliation::represented_rational(0.1).unwrap())
            .unwrap()
            .is_gt()
    );
    assert!(above.to_f64() < 0.1);
    assert_eq!(
        supply_use(above, 0.1, 0.1, ONE).unwrap_err().code,
        "FMINE-REF-207"
    );
}

#[test]
fn source_bound_worksheets_join_both_models_and_preserve_the_tonne_grade_trap() {
    let grinding_text = include_str!("../../../../tests/plant/grinding_joint.toml");
    let recovery_text = include_str!("../../../../tests/plant/production_recovery.toml");
    let production_text = include_str!("../../../../tests/plant/production_joint.toml");
    let anchor = |name: &str, text: &str| {
        SourceAnchor::new(
            name,
            sha256(text.as_bytes()).0,
            "authored synthetic worksheet",
        )
    };
    let grinding =
        grinding::worksheet::evaluate(grinding_text, &anchor("synthetic/grinding", grinding_text))
            .unwrap();
    let recovery =
        cards::worksheet::evaluate(recovery_text, &anchor("synthetic/recovery", recovery_text))
            .unwrap();
    assert!(recovery.refusal().is_none());
    let recovery = recovery.ensemble_receipt().unwrap();
    let result = worksheet::evaluate(
        production_text,
        &anchor("synthetic/production", production_text),
        grinding.receipt(),
        recovery,
    )
    .unwrap();
    close(result.receipt().summary().dry_tonnes.mean.value(), 200.0);
    close(
        result.receipt().summary().contained_metal.mean.value(),
        0.32,
    );
    close(
        result.receipt().summary().recovered_metal.mean.value(),
        0.24,
    );
    assert_ne!(
        result.receipt().summary().recovered_metal.mean.value(),
        0.375
    );
    assert_eq!(
        result.receipt().claim().confidentiality,
        Confidentiality::internal()
    );

    let sections = production_text.split("[[scenario]]").collect::<Vec<_>>();
    assert_eq!(sections.len(), 3);
    let incomplete = format!("{}[[scenario]]{}", sections[0], sections[1]);
    assert_eq!(
        worksheet::evaluate(
            &incomplete,
            &anchor("synthetic/production", &incomplete),
            grinding.receipt(),
            recovery,
        )
        .err()
        .unwrap()
        .code,
        "FMINE-REF-105"
    );
    let reordered = format!(
        "{}[[scenario]]{}\n[[scenario]]{}",
        sections[0], sections[2], sections[1]
    );
    let permuted = worksheet::evaluate(
        &reordered,
        &anchor("synthetic/production", &reordered),
        grinding.receipt(),
        recovery,
    )
    .unwrap();
    assert_eq!(result.receipt().summary(), permuted.receipt().summary());
    assert_eq!(result.receipt().points(), permuted.receipt().points());
    // The raw source changed, so the source-bound identity changes with it.
    assert_ne!(result.receipt().digest(), permuted.receipt().digest());

    let changed = production_text.replace("grade_g_per_dry_t = \"1\"", "grade_g_per_dry_t = \"2\"");
    assert_eq!(
        worksheet::evaluate(
            &changed,
            &anchor("synthetic/production", production_text),
            grinding.receipt(),
            recovery
        )
        .err()
        .unwrap()
        .code,
        "FMINE-REF-505"
    );
    assert_eq!(
        worksheet::evaluate(
            &changed,
            &anchor("synthetic/production", &changed),
            grinding.receipt(),
            recovery
        )
        .err()
        .unwrap()
        .code,
        "FMINE-REF-105"
    );
}

#[test]
fn supply_worksheet_is_opt_in_strict_and_source_bound_with_legacy_output_preserved() {
    let grinding_text = include_str!("../../../../tests/plant/grinding_joint.toml");
    let recovery_text = include_str!("../../../../tests/plant/production_recovery.toml");
    let legacy_text = include_str!("../../../../tests/plant/production_joint.toml");
    let supplied_text = include_str!("../../../../tests/plant/production_supply_limited.toml");
    let anchor = |name: &str, text: &str| {
        SourceAnchor::new(
            name,
            sha256(text.as_bytes()).0,
            "authored synthetic worksheet",
        )
    };
    let grinding =
        grinding::worksheet::evaluate(grinding_text, &anchor("grinding", grinding_text)).unwrap();
    let recovery =
        cards::worksheet::evaluate(recovery_text, &anchor("recovery", recovery_text)).unwrap();
    let ensemble = recovery.ensemble_receipt().unwrap();
    let run = |text: &str| {
        worksheet::evaluate(
            text,
            &anchor("production", text),
            grinding.receipt(),
            ensemble,
        )
    };
    let supplied = run(supplied_text).unwrap();
    close(supplied.receipt().summary().dry_tonnes.mean.value(), 90.0);
    close(
        supplied.receipt().summary().contained_metal.mean.value(),
        0.21,
    );
    close(
        supplied.receipt().summary().recovered_metal.mean.value(),
        0.174,
    );
    assert!(
        supplied
            .to_json()
            .contains("fmine.plant-production-result.v2")
    );
    assert!(
        supplied
            .to_json()
            .contains("\"binding_constraint\":\"ore_supply\"")
    );
    assert!(
        supplied
            .to_markdown()
            .contains("Available feed and binding constraints")
    );

    let legacy = run(legacy_text).unwrap();
    assert!(
        legacy
            .to_json()
            .contains("fmine.plant-production-result.v1")
    );
    assert!(!legacy.to_json().contains("\"supply\":"));
    assert!(!legacy.to_json().contains("\"supply_use\":"));
    assert!(legacy.receipt().supply().is_none());
    close(
        legacy.receipt().summary().recovered_metal.mean.value(),
        0.24,
    );

    for text in [
        supplied_text.replace("supply_unit = \"dry_metric_tonnes\"\n", ""),
        supplied_text.replace(
            "supply_unit = \"dry_metric_tonnes\"",
            "supply_unit = \"wet_metric_tonnes\"",
        ),
        supplied_text.replace("available_at_start", "arrives_during_shift"),
        supplied_text.replace(
            "reduce_exposure_at_declared_operating_point",
            "throttle_circuit",
        ),
        supplied_text.replacen("available_dry_tonnes = \"100\"\n", "", 1),
        supplied_text.replace(worksheet::SUPPLY_SCHEMA, worksheet::SCHEMA),
        legacy_text.replace(
            "scheduled_hours = \"1\"",
            "scheduled_hours = \"1\"\navailable_dry_tonnes = \"100\"",
        ),
    ] {
        assert_eq!(run(&text).err().unwrap().code, "FMINE-REF-506", "{text}");
    }
    let changed = supplied_text.replace(
        "available_dry_tonnes = \"100\"",
        "available_dry_tonnes = \"-1\"",
    );
    assert_eq!(run(&changed).err().unwrap().code, "FMINE-REF-109");
    assert_eq!(
        worksheet::evaluate(
            &changed,
            &anchor("production", supplied_text),
            grinding.receipt(),
            ensemble
        )
        .err()
        .unwrap()
        .code,
        "FMINE-REF-505"
    );
    let changed = format!("{supplied_text}\n# same calculation, distinct retained source bytes\n");
    let changed = run(&changed).unwrap();
    assert_eq!(supplied.receipt().points(), changed.receipt().points());
    assert_ne!(supplied.receipt().digest(), changed.receipt().digest());
}

#[test]
fn conditional_corrections_remain_paired_with_grinding_before_metal_aggregation() {
    use crate::cards::ensemble::{CORRECTIONS_METRIC, RecoveryCorrections};

    let (grinding, _, inputs) = fixture();
    let common = recovery(
        [q(2, 1); 2],
        [q(7, 10); 2],
        InputUnit::GramsPerDryMetricTonne,
    );
    let corrected = |deltas: [ExactQty; 2]| {
        let scenarios = ["a", "b"]
            .into_iter()
            .zip(deltas)
            .map(|(id, additive_fraction)| {
                let value = RecoveryCorrection {
                    version: version(id),
                    test_work: common.model().test_work().claim_id,
                    additive_fraction,
                    upper_cap_fraction: None,
                };
                let tag = value.canonical_tag();
                (id.to_string(), source(cards::CORRECTION_METRIC, value, tag))
            })
            .collect();
        let value = RecoveryCorrections {
            population_id: common.population().claim_id,
            scenarios,
        };
        let tag = value.canonical_tag();
        common
            .model()
            .evaluate_ensemble_with_corrections(
                common.population(),
                &source(CORRECTIONS_METRIC, value, tag),
            )
            .unwrap()
    };
    let paired = corrected([q(1, 5), q(-1, 5)]);
    let opposed = corrected([q(-1, 5), q(1, 5)]);
    let first = evaluate(&grinding, &paired, &inputs).unwrap();
    let second = evaluate(&grinding, &opposed, &inputs).unwrap();
    assert_eq!(first.summary().throughput, second.summary().throughput);
    assert_eq!(first.summary().recovery, second.summary().recovery);
    // Independent arithmetic: .5*(.32*.9+.16*.5) versus .5*(.32*.5+.16*.9).
    close(first.summary().recovered_metal.mean.value(), 0.184);
    close(second.summary().recovered_metal.mean.value(), 0.152);
    assert_ne!(first.digest(), second.digest());
    assert_eq!(first.recovery().corrections(), paired.corrections());
}
