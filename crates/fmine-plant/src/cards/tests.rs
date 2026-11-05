//! FACT: constructed nominal-card known answers, not plant calibration.
//! Decimal exponential/logarithm references were evaluated independently at
//! 60-digit precision; equations are transcribed in corpus C2 §3.4 (E171).

use super::*;
use fmine_claim::{Confidentiality, Representation, SpatialSupport, UuTerm};
use fmine_core::{anchor::SourceAnchor, hash::sha256};
use fmine_evidence::{EpistemicStatus, EvidenceClass, KnowledgeState};

fn q(s: &str) -> ExactQty {
    ExactQty::from_decimal_str(s).unwrap()
}
fn version(id: &str) -> CardVersion {
    CardVersion {
        id: id.into(),
        revision: "constructed-v1".into(),
    }
}
fn scope() -> RecoveryScope {
    RecoveryScope {
        circuit: "constructed-circuit".into(),
        material_domain: "authored-ore-domain".into(),
        dry_basis: "matched-total-dry-matter/v1".into(),
        element: "Au".into(),
    }
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
            "crates/fmine-plant/src/cards/tests.rs",
            sha256(include_bytes!("tests.rs")).0,
            "constructed card, domains and source-layer controls",
        )],
    )
}
fn card(formula: &str, unit: InputUnit, scale: OutputScale) -> Claim<TestWorkCard> {
    let value = TestWorkCard {
        version: version("test-work"),
        scope: scope(),
        valid_time: ValidTime {
            start_ns: 0,
            end_ns: 100,
        },
        input_units: BTreeMap::from([(
            "Au".into(),
            InputRequirement {
                measurand: "total-Au/dry".into(),
                unit,
                lower: q("0"),
                upper: q("10"),
                lower_inclusive: true,
                upper_inclusive: true,
            },
        )]),
        formula: formula.into(),
        output_scale: scale,
    };
    let tag = value.canonical_tag();
    source(TEST_WORK_METRIC, value, &tag)
}
fn model_with(
    card: Claim<TestWorkCard>,
    multipliers: &[&str],
    addition: &str,
    cap: Option<&str>,
) -> Result<RecoveryModel> {
    let operations: Vec<_> = multipliers
        .iter()
        .enumerate()
        .map(|(i, x)| {
            let v = OperationalFactor {
                version: version(&format!("operation-{i}")),
                test_work: card.claim_id,
                multiplier: q(x),
            };
            let tag = v.canonical_tag();
            source(OPERATION_METRIC, v, &tag)
        })
        .collect();
    let c = RecoveryCorrection {
        version: version("correction"),
        test_work: card.claim_id,
        additive_fraction: q(addition),
        upper_cap_fraction: cap.map(q),
    };
    let tag = c.canonical_tag();
    let correction = source(CORRECTION_METRIC, c, &tag);
    let r = RouteAssumption {
        version: version("route"),
        test_work: card.claim_id,
        route: "roast-and-CIL".into(),
    };
    let tag = r.canonical_tag();
    let route = source(ROUTE_METRIC, r, &tag);
    RecoveryModel::compile(&card, &operations, &correction, &route)
}
fn model(formula: &str, unit: InputUnit, scale: OutputScale) -> RecoveryModel {
    model_with(card(formula, unit, scale), &[], "0", None).unwrap()
}
fn input_value(model: &RecoveryModel, value: ExactQty, unit: InputUnit) -> RecoveryInputs {
    RecoveryInputs {
        scope: model.test_work().value.scope.clone(),
        interval: ValidTime {
            start_ns: 10,
            end_ns: 20,
        },
        values: BTreeMap::from([(
            "Au".into(),
            InputValue {
                measurand: "total-Au/dry".into(),
                unit,
                value,
            },
        )]),
    }
}
fn inputs(value: RecoveryInputs) -> Claim<RecoveryInputs> {
    let tag = value.canonical_tag();
    source(INPUT_METRIC, value, &tag)
}
fn evaluate(model: &RecoveryModel, value: &str) -> RecoveryReceipt {
    let unit = model.test_work().value.input_units["Au"].unit;
    model
        .evaluate(&inputs(input_value(model, q(value), unit)))
        .unwrap()
}
fn close(actual: f64, expected: f64) {
    assert!((actual - expected).abs() < 2e-13, "{actual} != {expected}");
}

#[test]
fn three_published_unit_conventions_match_independent_decimal_answers() {
    let cases = [
        (
            "92.03 - 37.36 * exp(-12.94 * (Au * 0.029167))",
            InputUnit::GramsPerDryMetricTonne,
            OutputScale::Percent,
            ["1", "5", "10"],
            [0.6641496116841243, 0.8636959347771791, 0.9117239288015274],
        ),
        (
            "(92.027536 - 37.35906 * EXP(-12.94386 * Au)) / 100.0",
            InputUnit::TroyOuncesPerDryShortTon,
            OutputScale::Fraction,
            ["0.1", "0.2", "0.3"],
            [0.8178868452737355, 0.8922141401838417, 0.9125847312251227],
        ),
        (
            "0.0754 * ln(Au) + 0.72708",
            InputUnit::GramsPerDryMetricTonne,
            OutputScale::Fraction,
            ["1", "5", "10"],
            [0.72708, 0.8484316185975312, 0.900694916011751],
        ),
    ];
    for (formula, unit, scale, grades, expected) in cases {
        let m = model(formula, unit, scale);
        for (g, known) in grades.into_iter().zip(expected) {
            let result = evaluate(&m, g);
            close(result.claim().value.recovery_fraction.value(), known);
            assert_eq!(result.claim().state, KnowledgeState::Estimated);
            assert_eq!(result.claim().evidence, EvidenceClass::Assumed);
        }
    }
}

#[test]
fn explicit_unit_conversion_works_but_implicit_substitution_refuses() {
    let metric = model(
        "60 + 2 * Au",
        InputUnit::GramsPerDryMetricTonne,
        OutputScale::Percent,
    );
    let customary = model(
        "60 + (480 / 7) * Au",
        InputUnit::TroyOuncesPerDryShortTon,
        OutputScale::Percent,
    );
    let g = q("5");
    let a = metric
        .evaluate(&inputs(input_value(
            &metric,
            g,
            InputUnit::GramsPerDryMetricTonne,
        )))
        .unwrap();
    let b = customary
        .evaluate(&inputs(input_value(
            &customary,
            g.g_per_t_to_oz_per_st().unwrap(),
            InputUnit::TroyOuncesPerDryShortTon,
        )))
        .unwrap();
    close(a.claim().value.recovery_fraction.value(), 0.7);
    close(
        a.claim().value.recovery_fraction.value(),
        b.claim().value.recovery_fraction.value(),
    );
    let wrong = inputs(input_value(
        &customary,
        g,
        InputUnit::GramsPerDryMetricTonne,
    ));
    assert_eq!(
        customary.evaluate(&wrong).unwrap_err().code,
        "FMINE-REF-107"
    );
}

#[test]
fn exact_validity_rejects_outside_point_even_when_it_rounds_to_the_boundary() {
    let m = model(
        "0.5",
        InputUnit::GramsPerDryMetricTonne,
        OutputScale::Fraction,
    );
    let outside = q("10.000000000000000001");
    assert_eq!(outside.to_f64(), 10.0);
    let x = inputs(input_value(&m, outside, InputUnit::GramsPerDryMetricTonne));
    assert_eq!(m.evaluate(&x).unwrap_err().code, "FMINE-REF-203");
    close(
        evaluate(&m, "10").claim().value.recovery_fraction.value(),
        0.5,
    );
}

#[test]
fn source_specific_spike_test_limits_retain_strict_boundaries() {
    for limit in ["40", "60"] {
        let mut c = card("0.9", InputUnit::SpikeTestPercent, OutputScale::Fraction).value;
        c.scope.material_domain = format!("authored-source-with-limit-{limit}");
        c.input_units.clear();
        c.input_units.insert(
            "PRI".into(),
            InputRequirement {
                measurand: "gold-adsorption-spike-test/percent".into(),
                unit: InputUnit::SpikeTestPercent,
                lower: q("0"),
                upper: q(limit),
                lower_inclusive: true,
                upper_inclusive: false,
            },
        );
        let tag = c.canonical_tag();
        let m = model_with(source(TEST_WORK_METRIC, c, &tag), &[], "0", None).unwrap();
        let mut v = RecoveryInputs {
            scope: m.test_work().value.scope.clone(),
            interval: ValidTime {
                start_ns: 10,
                end_ns: 20,
            },
            values: BTreeMap::from([(
                "PRI".into(),
                InputValue {
                    measurand: "gold-adsorption-spike-test/percent".into(),
                    unit: InputUnit::SpikeTestPercent,
                    value: q(limit).checked_sub(q("1")).unwrap(),
                },
            )]),
        };
        close(
            m.evaluate(&inputs(v.clone()))
                .unwrap()
                .claim()
                .value
                .recovery_fraction
                .value(),
            0.9,
        );
        v.values.get_mut("PRI").unwrap().value = q(limit);
        assert_eq!(
            m.evaluate(&inputs(v.clone())).unwrap_err().code,
            "FMINE-REF-203"
        );
        v.values.get_mut("PRI").unwrap().value = q("20");
        v.values.get_mut("PRI").unwrap().unit = InputUnit::DryMassPercent;
        assert_eq!(m.evaluate(&inputs(v)).unwrap_err().code, "FMINE-REF-107");
    }
}

#[test]
fn constant_source_cards_need_no_fabricated_grade_input() {
    let mut c = card(
        "84",
        InputUnit::GramsPerDryMetricTonne,
        OutputScale::Percent,
    )
    .value;
    c.input_units.clear();
    let tag = c.canonical_tag();
    let m = model_with(source(TEST_WORK_METRIC, c, &tag), &[], "0", None).unwrap();
    let v = RecoveryInputs {
        scope: m.test_work().value.scope.clone(),
        interval: ValidTime {
            start_ns: 10,
            end_ns: 20,
        },
        values: BTreeMap::new(),
    };
    close(
        m.evaluate(&inputs(v))
            .unwrap()
            .claim()
            .value
            .recovery_fraction
            .value(),
        0.84,
    );
}

#[test]
fn four_layers_retain_intermediates_and_explicit_cap() {
    let base = card(
        "80",
        InputUnit::GramsPerDryMetricTonne,
        OutputScale::Percent,
    );
    let m = model_with(base, &["0.9", "1.1"], "0.03", Some("0.8")).unwrap();
    let receipt = evaluate(&m, "5");
    let v = &receipt.claim().value;
    close(v.test_work_fraction.value(), 0.8);
    close(v.operational_fractions[0].value(), 0.72);
    close(v.operational_fractions[1].value(), 0.792);
    close(v.corrected_before_cap.value(), 0.822);
    close(v.recovery_fraction.value(), 0.8);
    assert!(v.cap_applied);
    assert_eq!(v.route, "roast-and-CIL");
    assert_eq!(receipt.claim().parents.len(), 6);
    assert_eq!(receipt.model().operations()[1].value.multiplier, q("1.1"));
    assert_eq!(receipt, evaluate(&m, "5"));
}

#[test]
fn layers_cannot_be_retargeted_or_silently_reused_on_another_version() {
    let a = model(
        "0.8",
        InputUnit::GramsPerDryMetricTonne,
        OutputScale::Fraction,
    );
    let b = card(
        "0.9",
        InputUnit::GramsPerDryMetricTonne,
        OutputScale::Fraction,
    );
    assert_eq!(
        RecoveryModel::compile(&b, a.operations(), a.correction(), a.route())
            .unwrap_err()
            .code,
        "FMINE-REF-507"
    );
    let factor = OperationalFactor {
        version: version("one-stage"),
        test_work: a.test_work().claim_id,
        multiplier: q("0.95"),
    };
    let tag = factor.canonical_tag();
    let factor = source(OPERATION_METRIC, factor, &tag);
    assert!(
        RecoveryModel::compile(
            a.test_work(),
            &[factor.clone(), factor],
            a.correction(),
            a.route()
        )
        .is_err()
    );
}

#[test]
fn missing_unit_scope_time_and_measurand_support_refuse() {
    let m = model(
        "0.8",
        InputUnit::GramsPerDryMetricTonne,
        OutputScale::Fraction,
    );
    let original = input_value(&m, q("5"), InputUnit::GramsPerDryMetricTonne);
    for mutation in 0..5 {
        let mut v = original.clone();
        match mutation {
            0 => v.scope.dry_basis = "wet-basis".into(),
            1 => v.interval.end_ns = 101,
            2 => v.values.get_mut("Au").unwrap().measurand = "cyanide-soluble-Au".into(),
            3 => {
                v.values.clear();
            }
            _ => {
                v.values.insert("undeclared".into(), v.values["Au"].clone());
            }
        }
        assert!(m.evaluate(&inputs(v)).is_err());
    }
    let mut sample = inputs(original);
    sample.support = SpatialSupport::PlantLot;
    assert_eq!(m.evaluate(&sample).unwrap_err().code, "FMINE-REF-102");
}

#[test]
fn unparseable_unbound_or_unbounded_formulas_refuse_at_compile() {
    for bad in [
        "(0.9 * (Au + 1))) / Au",
        "unknown + 1",
        "0.8; 0.9",
        "Au ** 2",
        "Au ^ 17",
        "sin(Au)",
        "1e999",
        "0.1e-999",
        "0.5 +",
    ] {
        assert!(
            model_with(
                card(
                    bad,
                    InputUnit::GramsPerDryMetricTonne,
                    OutputScale::Fraction
                ),
                &[],
                "0",
                None
            )
            .is_err(),
            "accepted {bad}"
        );
    }
    let nested = format!("{}Au{}", "(".repeat(40), ")".repeat(40));
    assert!(
        model_with(
            card(
                &nested,
                InputUnit::GramsPerDryMetricTonne,
                OutputScale::Fraction
            ),
            &[],
            "0",
            None
        )
        .is_err()
    );
    let overwork = std::iter::repeat_n("0.001", 300)
        .collect::<Vec<_>>()
        .join("+");
    assert!(
        model_with(
            card(
                &overwork,
                InputUnit::GramsPerDryMetricTonne,
                OutputScale::Fraction
            ),
            &[],
            "0",
            None
        )
        .is_err()
    );
}

#[test]
fn precedence_scientific_literals_and_integer_powers_have_known_answers() {
    let names = vec!["x".into()];
    for (expr, expected) in [
        ("-x^2", -4.0),
        ("(-x)^2", 4.0),
        ("x^-2 + 2e-1", 0.45),
        ("exp(0) + ln(exp(1))", 2.0),
        ("0e-999 + .5", 0.5),
    ] {
        close(
            formula::Formula::compile(expr, &names)
                .unwrap()
                .evaluate(&[2.0])
                .unwrap(),
            expected,
        );
    }
}

#[test]
fn arithmetic_domain_and_physical_recovery_are_not_silently_repaired() {
    for (expression, value) in [
        ("ln(Au)", "0"),
        ("1 / Au", "0"),
        ("exp(Au * 1000)", "5"),
        ("exp(-Au * 1000)", "5"),
        ("1.1", "5"),
        ("-0.1", "5"),
    ] {
        let m = model(
            expression,
            InputUnit::GramsPerDryMetricTonne,
            OutputScale::Fraction,
        );
        let i = inputs(input_value(&m, q(value), InputUnit::GramsPerDryMetricTonne));
        assert!(m.evaluate(&i).is_err());
    }
    let capped = model_with(
        card(
            "1.1",
            InputUnit::GramsPerDryMetricTonne,
            OutputScale::Fraction,
        ),
        &[],
        "0",
        Some("0.95"),
    )
    .unwrap();
    assert!(evaluate(&capped, "5").claim().value.cap_applied);
    let negative = model_with(
        card(
            "-0.1",
            InputUnit::GramsPerDryMetricTonne,
            OutputScale::Fraction,
        ),
        &[],
        "0.2",
        None,
    )
    .unwrap();
    assert!(
        negative
            .evaluate(&inputs(input_value(
                &negative,
                q("5"),
                InputUnit::GramsPerDryMetricTonne
            )))
            .is_err()
    );
}

#[test]
fn source_mutation_staleness_and_failed_calibration_remain_refusals() {
    let mut c = card(
        "0.8",
        InputUnit::GramsPerDryMetricTonne,
        OutputScale::Fraction,
    );
    c.value.formula = "0.9".into();
    assert_eq!(
        model_with(c, &[], "0", None).unwrap_err().code,
        "FMINE-REF-507"
    );
    let m = model(
        "0.8",
        InputUnit::GramsPerDryMetricTonne,
        OutputScale::Fraction,
    );
    let original = inputs(input_value(&m, q("5"), InputUnit::GramsPerDryMetricTonne));
    let mut stale = original.clone();
    stale.stale = true;
    assert_eq!(m.evaluate(&stale).unwrap_err().code, "FMINE-REF-502");
    let mut failed = original;
    failed.calibration_failed = true;
    assert_eq!(m.evaluate(&failed).unwrap_err().code, "FMINE-REF-205");
}

#[test]
fn metadata_and_versioned_receipts_preserve_source_restrictions() {
    let m = model(
        "0.8",
        InputUnit::GramsPerDryMetricTonne,
        OutputScale::Fraction,
    );
    let v = input_value(&m, q("5"), InputUnit::GramsPerDryMetricTonne);
    let tag = v.canonical_tag();
    let mut input = source(INPUT_METRIC, v, &tag);
    input.uu_terms.push(UuTerm {
        scenario_id: "ore-transfer".into(),
        description: "constructed support-transfer uncertainty".into(),
        downside_exposure_usd_m: 1.0,
        trigger_indicator: "independent recovery measurements".into(),
    });
    let input = Claim::new(
        &input.metric_key,
        input.value.clone(),
        input.value_tag(),
        input.representation.clone(),
        input.support,
        input.additivity,
        input.evidence,
        input.epistemic,
        input.state,
        Confidentiality::internal(),
        input.uu_terms.clone(),
        input.parents.clone(),
        input.anchors.clone(),
    );
    let r = m.evaluate(&input).unwrap();
    assert_eq!(r.claim().uu_terms, input.uu_terms);
    assert_eq!(r.claim().confidentiality, Confidentiality::internal());
    assert_eq!(r.inputs(), &input);
    let mut route = m.route().value.clone();
    route.version.revision = "constructed-v2".into();
    let tag = route.canonical_tag();
    let route = source(ROUTE_METRIC, route, &tag);
    let changed =
        RecoveryModel::compile(m.test_work(), m.operations(), m.correction(), &route).unwrap();
    assert_ne!(r.digest(), changed.evaluate(&input).unwrap().digest());
}
