//! FACT: constructed processing-input oracles and source-binding controls.
//! These observations establish no empirical calibration or plant authorization.
//! Determinism: DeterministicPerIsa

use super::*;
use fmine_claim::{Confidentiality, Representation, SpatialSupport, UuTerm};
use fmine_core::{anchor::SourceAnchor, hash::sha256};
use fmine_evidence::{EpistemicStatus, EvidenceClass, KnowledgeState};

fn q(n: i128, d: i128) -> ExactQty {
    ExactQty::new(n, d).unwrap()
}
fn revision(id: &str) -> CardVersion {
    CardVersion {
        id: id.into(),
        revision: "constructed-v1".into(),
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
            "crates/fmine-plant/src/cards/fit/observations/tests.rs",
            sha256(include_bytes!("tests.rs")).0,
            "constructed historical processing inputs and recovery observations",
        )],
    )
}
fn build_model(formula: &str, multipliers: &[ExactQty]) -> RecoveryModel {
    let card = TestWorkCard {
        version: revision("test-work"),
        scope: RecoveryScope {
            circuit: "constructed-circuit".into(),
            material_domain: "constructed-ore".into(),
            dry_basis: "matched-dry-matter/v1".into(),
            element: "Au".into(),
        },
        valid_time: ValidTime {
            start_ns: 0,
            end_ns: 1000,
        },
        input_units: BTreeMap::from([(
            "g".into(),
            InputRequirement {
                measurand: "total-Au/dry".into(),
                unit: InputUnit::GramsPerDryMetricTonne,
                lower: q(0, 1),
                upper: q(10, 1),
                lower_inclusive: true,
                upper_inclusive: true,
            },
        )]),
        formula: formula.into(),
        output_scale: OutputScale::Fraction,
    };
    let tag = card.canonical_tag();
    let card = source(TEST_WORK_METRIC, card, &tag);
    let operations: Vec<_> = multipliers
        .iter()
        .enumerate()
        .map(|(index, multiplier)| {
            let value = OperationalFactor {
                version: revision(&format!("operation-{index}")),
                test_work: card.claim_id,
                multiplier: *multiplier,
            };
            let tag = value.canonical_tag();
            source(OPERATION_METRIC, value, &tag)
        })
        .collect();
    let correction = correction(RecoveryCorrection {
        version: revision("original-correction"),
        test_work: card.claim_id,
        additive_fraction: q(1, 20),
        upper_cap_fraction: Some(q(9, 10)),
        fit_validity: None,
    });
    let route = RouteAssumption {
        version: revision("route"),
        test_work: card.claim_id,
        route: "constructed-process".into(),
    };
    let tag = route.canonical_tag();
    let route = source(ROUTE_METRIC, route, &tag);
    RecoveryModel::compile(&card, &operations, &correction, &route).unwrap()
}
fn correction(value: RecoveryCorrection) -> Claim<RecoveryCorrection> {
    let tag = value.canonical_tag();
    source(CORRECTION_METRIC, value, &tag)
}
fn inputs(value: RecoveryInputs) -> Claim<RecoveryInputs> {
    let tag = value.canonical_tag();
    source(crate::cards::INPUT_METRIC, value, &tag)
}
fn observation(value: RecoveryObservation) -> Claim<RecoveryObservation> {
    let tag = value.canonical_tag();
    source(OBSERVATION_METRIC, value, &tag)
}
fn observed(model: &RecoveryModel, start_ns: i64, grade: ExactQty) -> Claim<RecoveryObservation> {
    observation(RecoveryObservation {
        id: format!("observation-{start_ns}"),
        inputs: inputs(RecoveryInputs {
            scope: model.test_work().value.scope.clone(),
            interval: ValidTime {
                start_ns,
                end_ns: start_ns + 100,
            },
            values: BTreeMap::from([(
                "g".into(),
                InputValue {
                    measurand: "total-Au/dry".into(),
                    unit: InputUnit::GramsPerDryMetricTonne,
                    value: grade,
                },
            )]),
        }),
        available_ns: start_ns + 110,
        observed_fraction: q(1, 4),
        uncensored: true,
    })
}
fn declaration(model: &RecoveryModel, pairs: Vec<Claim<RecoveryPair>>) -> Claim<CorrectionFit> {
    let value = CorrectionFit {
        version: revision("new-fitted-correction"),
        scope: model.test_work().value.scope.clone(),
        test_work: model.test_work().claim_id,
        operation_ids: model.operations().iter().map(|c| c.claim_id).collect(),
        route: model.route().claim_id,
        evidence_cut_ns: 400,
        target_interval: ValidTime {
            start_ns: 500,
            end_ns: 700,
        },
        alignment: FitAlignment::SameProcessingSupport,
        upper_cap_fraction: None,
        pairs,
    };
    let tag = value.canonical_tag();
    source(super::super::INPUT_METRIC, value, &tag)
}

#[test]
fn nonlinear_card_and_ordered_factors_derive_the_exact_binary64_baseline() {
    let model = build_model("(g^2 + g) / 4", &[q(4, 5), q(1, 2)]);
    let original = observed(&model, 100, q(1, 2));
    let receipt = derive_pair(&model, &original).unwrap();
    // (0.5² + 0.5)/4 = 3/16; binary64 *0.8 then *0.5 has this
    // independently decomposed ratio, which differs from the decimal 3/40.
    assert_eq!(receipt.test_work_fraction().value(), 0.1875);
    assert_eq!(receipt.operational_fractions().len(), 2);
    assert_eq!(
        receipt.operational_fractions()[0].value(),
        0.15000000000000002
    );
    assert_eq!(
        receipt.operational_fractions()[1].value(),
        0.07500000000000001
    );
    assert_eq!(
        receipt.claim().value.baseline_fraction,
        q(1_351_079_888_211_149, 18_014_398_509_481_984)
    );
    assert_ne!(receipt.claim().value.baseline_fraction, q(3, 40));
    assert_eq!(
        receipt.claim().value.baseline_fraction.to_f64().to_bits(),
        receipt.baseline_fraction().value().to_bits()
    );
    assert_eq!(receipt.model(), &model);
    assert_eq!(receipt.observation(), &original);
    assert_eq!(
        receipt.baseline_claim().value.interval,
        original.value.inputs.value.interval
    );
    assert_eq!(
        receipt.claim().value.baseline_model,
        Some(BaselineModelBinding::from_model(&model))
    );
    assert!(
        receipt
            .claim()
            .parents
            .contains(&receipt.baseline_claim().claim_id)
    );
    for parent in [
        model.test_work().claim_id,
        model.operations()[0].claim_id,
        model.operations()[1].claim_id,
        model.correction().claim_id,
        model.route().claim_id,
        original.claim_id,
        original.value.inputs.claim_id,
    ] {
        assert!(receipt.baseline_claim().parents.contains(&parent));
        assert!(receipt.claim().parents.contains(&parent));
    }
    assert_eq!(receipt, derive_pair(&model, &original).unwrap());
    let exponential = build_model("1 - exp(-g)", &[q(1, 2)]);
    let value = derive_pair(&exponential, &observed(&exponential, 100, q(1, 1))).unwrap();
    // Independent decimal exponential reference, not the kernel's formula path.
    assert!((value.test_work_fraction().value() - 0.6321205588285577).abs() < 2e-15);
    assert!((value.baseline_fraction().value() - 0.31606027941427883).abs() < 2e-15);
}

#[test]
fn original_offset_cap_and_future_fit_restrictions_never_enter_historical_baselines() {
    let model = build_model("g", &[q(1, 2)]);
    let original = observed(&model, 100, q(3, 1));
    let receipt = derive_pair(&model, &original).unwrap();
    // This uncapped baseline exceeds one, while the admitted nominal result is capped.
    assert_eq!(receipt.claim().value.baseline_fraction, q(3, 2));
    assert!(
        model
            .evaluate(&original.value.inputs)
            .unwrap()
            .claim()
            .value
            .cap_applied
    );
    let mut changed = model.correction().value.clone();
    changed.additive_fraction = q(-2, 1);
    changed.upper_cap_fraction = Some(q(0, 1));
    changed.fit_validity = Some(CorrectionFitValidity {
        evidence_cut_ns: 400,
        target_interval: ValidTime {
            start_ns: 500,
            end_ns: 700,
        },
        operation_ids: model.operations().iter().map(|c| c.claim_id).collect(),
        route: model.route().claim_id,
        fit_digest: "a".repeat(64),
    });
    let changed = model.with_correction(&correction(changed)).unwrap();
    assert_eq!(
        changed.evaluate(&original.value.inputs).unwrap_err().code,
        "FMINE-REF-203"
    );
    let derived = derive_pair(&changed, &original).unwrap();
    assert_eq!(derived.claim().value, receipt.claim().value);
    assert_eq!(derived.baseline_fraction(), receipt.baseline_fraction());
    assert_ne!(derived.digest(), receipt.digest());
    assert_ne!(derived.claim().claim_id, receipt.claim().claim_id);
    assert_eq!(derived.model().correction(), changed.correction());
}

#[test]
fn derived_pairs_fit_only_their_original_test_work_ordered_operations_and_route() {
    let model = build_model("(g^2 + g) / 4", &[q(4, 5), q(1, 2)]);
    let pairs: Vec<_> = [100, 200]
        .into_iter()
        .map(|start| {
            derive_pair(&model, &observed(&model, start, q(1, 2)))
                .unwrap()
                .claim()
                .clone()
        })
        .collect();
    let fit = fit_correction(&model, &declaration(&model, pairs.clone())).unwrap();
    // Both residuals equal 1/4 minus the represented baseline; variance is zero.
    assert_eq!(
        fit.summary().value.mean_residual_fraction,
        q(3_152_519_739_159_347, 18_014_398_509_481_984)
    );
    assert_eq!(fit.summary().value.sample_residual_variance, q(0, 1));
    let mut operations = model.operations().to_vec();
    operations.reverse();
    let reordered = RecoveryModel::compile(
        model.test_work(),
        &operations,
        model.correction(),
        model.route(),
    )
    .unwrap();
    let mut route = model.route().value.clone();
    route.route = "another-processing-route".into();
    let tag = route.canonical_tag();
    let rerouted = RecoveryModel::compile(
        model.test_work(),
        model.operations(),
        model.correction(),
        &source(ROUTE_METRIC, route, &tag),
    )
    .unwrap();
    let different_test_work = build_model("g", &[q(4, 5), q(1, 2)]);
    for changed in [reordered, rerouted, different_test_work] {
        assert_eq!(
            fit_correction(&changed, &declaration(&changed, pairs.clone()))
                .unwrap_err()
                .code,
            "FMINE-REF-102"
        );
    }
    // Replacing an old correction does not change the baseline's model binding.
    let mut old = model.correction().value.clone();
    old.additive_fraction = q(1, 8);
    let changed = model.with_correction(&correction(old)).unwrap();
    fit_correction(&changed, &declaration(&changed, pairs)).unwrap();
}

#[test]
fn full_original_envelopes_remain_bound_after_only_the_derived_pair_is_passed_on() {
    let model = build_model("g", &[]);
    let original = observed(&model, 100, q(1, 2));
    let receipt = derive_pair(&model, &original).unwrap();
    let mut old = model.correction().clone();
    old.supersedes = Some(model.route().claim_id);
    assert_eq!(old.claim_id, model.correction().claim_id);
    let changed = model.with_correction(&old).unwrap();
    let changed = derive_pair(&changed, &original).unwrap();
    assert_eq!(changed.claim().value, receipt.claim().value);
    assert_ne!(
        changed.baseline_claim().claim_id,
        receipt.baseline_claim().claim_id
    );
    assert_ne!(changed.claim().claim_id, receipt.claim().claim_id);
    let second = derive_pair(&model, &observed(&model, 200, q(1, 2))).unwrap();
    let fit = |first: &Claim<RecoveryPair>| {
        fit_correction(
            &model,
            &declaration(&model, vec![first.clone(), second.claim().clone()]),
        )
        .unwrap()
    };
    assert_ne!(fit(changed.claim()).digest(), fit(receipt.claim()).digest());
    let mut input_source = original.value.inputs.clone();
    input_source.supersedes = Some(model.route().claim_id);
    let mut altered = original.value.clone();
    altered.inputs = input_source;
    let altered = derive_pair(&model, &observation(altered)).unwrap();
    assert_eq!(altered.claim().value, receipt.claim().value);
    assert_ne!(altered.claim().claim_id, receipt.claim().claim_id);
    let changed_grade = derive_pair(&model, &observed(&model, 100, q(3, 4))).unwrap();
    assert_eq!(changed_grade.claim().value.baseline_fraction, q(3, 4));
    assert_ne!(changed_grade.digest(), receipt.digest());
}

#[test]
fn original_model_and_input_confidentiality_and_unknowns_taint_the_pair() {
    let model = build_model("g", &[]);
    let original = observed(&model, 100, q(1, 2));
    let input = &original.value.inputs;
    let mut value = original.value.clone();
    value.inputs = Claim::new(
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
        vec![UuTerm {
            scenario_id: "input-alignment-gap".into(),
            description: "Unrepresented residence-time spread".into(),
            downside_exposure_usd_m: 1.0,
            trigger_indicator: "independent tracer survey".into(),
        }],
        input.parents.clone(),
        input.anchors.clone(),
    );
    let mut old = model.correction().clone();
    old.uu_terms.push(UuTerm {
        scenario_id: "old-model-gap".into(),
        description: "Retained uncertainty in the original model".into(),
        downside_exposure_usd_m: 2.0,
        trigger_indicator: "independent recovery reconciliation".into(),
    });
    let model = model.with_correction(&old).unwrap();
    let original = observation(value);
    let receipt = derive_pair(&model, &original).unwrap();
    assert_eq!(receipt.observation(), &original);
    assert_eq!(receipt.model().correction(), &old);
    assert_eq!(receipt.claim().confidentiality, Confidentiality::internal());
    assert_eq!(
        receipt.baseline_claim().confidentiality,
        Confidentiality::internal()
    );
    assert_eq!(receipt.claim().uu_terms.len(), 2);
    assert_eq!(receipt.claim().uu_terms, receipt.baseline_claim().uu_terms);
    assert_eq!(receipt.claim().evidence, EvidenceClass::Assumed);
    assert_eq!(receipt.claim().state, KnowledgeState::Estimated);
}

#[test]
fn historical_inputs_keep_the_nominal_scope_units_exact_bounds_and_source_checks() {
    let model = build_model("g", &[]);
    let original = observed(&model, 100, q(1, 2));
    let mut cases = Vec::new();
    let mut value = original.value.inputs.value.clone();
    value.scope.element = "Ag".into();
    cases.push((value, "FMINE-REF-102"));
    let mut value = original.value.inputs.value.clone();
    value.values.get_mut("g").unwrap().unit = InputUnit::TroyOuncesPerDryShortTon;
    cases.push((value, "FMINE-REF-107"));
    let mut value = original.value.inputs.value.clone();
    value.values.get_mut("g").unwrap().measurand = "another-measurand".into();
    cases.push((value, "FMINE-REF-107"));
    let mut value = original.value.inputs.value.clone();
    value.values.clear();
    cases.push((value, "FMINE-REF-201"));
    let mut value = original.value.inputs.value.clone();
    value.interval.start_ns = -1;
    cases.push((value, "FMINE-REF-203"));
    let mut value = original.value.inputs.value.clone();
    value.interval.end_ns = value.interval.start_ns;
    cases.push((value, "FMINE-REF-109"));
    let mut value = original.value.inputs.value.clone();
    // This is strictly above ten although conversion rounds to the valid edge.
    let beyond = q(10_000_000_000_000_000_001, 1_000_000_000_000_000_000);
    assert_eq!(beyond.to_f64(), 10.0);
    value.values.get_mut("g").unwrap().value = beyond;
    cases.push((value, "FMINE-REF-203"));
    for (inputs_value, expected) in cases {
        let mut value = original.value.clone();
        value.inputs = inputs(inputs_value);
        assert_eq!(
            derive_pair(&model, &observation(value)).unwrap_err().code,
            expected
        );
    }
    let mut mutated = original.value.clone();
    mutated.inputs.value.values.get_mut("g").unwrap().value = q(3, 4);
    assert_eq!(
        derive_pair(&model, &observation(mutated)).unwrap_err().code,
        "FMINE-REF-507"
    );
    let mut stale = original.value.clone();
    stale.inputs.stale = true;
    assert_eq!(
        derive_pair(&model, &observation(stale)).unwrap_err().code,
        "FMINE-REF-502"
    );
    let mut unsupported = original.value.clone();
    unsupported.inputs.support = SpatialSupport::PointCore;
    assert_eq!(
        derive_pair(&model, &observation(unsupported)).unwrap_err().code,
        "FMINE-REF-102"
    );
}

#[test]
fn observation_timing_censoring_physical_values_and_identity_are_admitted() {
    let model = build_model("g", &[]);
    let original = observed(&model, 100, q(1, 2));
    for (kind, expected) in [
        (0, "FMINE-REF-203"),
        (1, "FMINE-REF-202"),
        (2, "FMINE-REF-109"),
        (3, "FMINE-REF-109"),
        (4, "FMINE-REF-201"),
    ] {
        let mut value = original.value.clone();
        match kind {
            0 => value.available_ns = 199,
            1 => value.uncensored = false,
            2 => value.observed_fraction = q(-1, 100),
            3 => value.observed_fraction = q(101, 100),
            _ => value.id = " ".into(),
        }
        assert_eq!(
            derive_pair(&model, &observation(value)).unwrap_err().code,
            expected
        );
    }
    let mut mutated = original.clone();
    mutated.value.available_ns += 1;
    assert_eq!(
        derive_pair(&model, &mutated).unwrap_err().code,
        "FMINE-REF-507"
    );
}

#[test]
fn no_decimal_fallback_hides_numerical_range_or_nonphysical_formula_failures() {
    for (formula, expected) in [
        ("1e-100", "FMINE-REF-207"),
        ("exp(1000)", "FMINE-REF-207"),
        ("exp(-1000)", "FMINE-REF-207"),
        ("-g", "FMINE-REF-109"),
    ] {
        let model = build_model(formula, &[]);
        assert_eq!(
            derive_pair(&model, &observed(&model, 100, q(1, 2))).unwrap_err().code,
            expected
        );
    }
    let model = build_model("1e-300", &[q(1, 100_000_000_000_000_000_000_000_000_000)]);
    assert_eq!(
        derive_pair(&model, &observed(&model, 100, q(1, 2))).unwrap_err().code,
        "FMINE-REF-207"
    );
}

#[test]
fn legacy_pair_tags_and_nominal_receipt_bytes_preserve_their_original_frames() {
    let model = build_model("(g^2 + g) / 4", &[q(4, 5), q(1, 2)]);
    let original = observed(&model, 100, q(1, 2));
    let derived = derive_pair(&model, &original).unwrap();
    let mut legacy = derived.claim().value.clone();
    legacy.baseline_model = None;
    let mut enc = Encoder::new_frame(*b"RCPF", 1);
    enc.put_str(&legacy.id);
    put_time(&mut enc, legacy.interval);
    enc.put_i64(legacy.available_ns);
    put_exact(&mut enc, legacy.baseline_fraction);
    put_exact(&mut enc, legacy.observed_fraction);
    enc.put_u8(u8::from(legacy.uncensored));
    assert_eq!(legacy.canonical_tag(), blake3_hash(&enc.finish()).to_hex());
    assert_ne!(legacy.canonical_tag(), derived.claim().value.canonical_tag());
    // The pre-existing RCER/1 layout and independent arithmetic answers remain
    // the oracle for nominal evaluation after extracting the shared baseline.
    let mut enc = Encoder::new_frame(*b"RCER", 1);
    inventory::put_source_envelope(&mut enc, model.test_work(), 0);
    enc.put_u64(2);
    for operation in model.operations() {
        inventory::put_source_envelope(&mut enc, operation, 1);
    }
    inventory::put_source_envelope(&mut enc, model.correction(), 2);
    inventory::put_source_envelope(&mut enc, model.route(), 3);
    inventory::put_source_envelope(&mut enc, &original.value.inputs, 4);
    for intermediate in [0.1875_f64, 0.15000000000000002, 0.07500000000000001, 0.125] {
        enc.put_u64(intermediate.to_bits());
    }
    enc.put_u8(0);
    enc.put_u64(0.125_f64.to_bits());
    let nominal = model.evaluate(&original.value.inputs).unwrap();
    assert_eq!(nominal.digest(), blake3_hash(&enc.finish()).to_hex());
    assert_eq!(
        nominal.claim().value.test_work_fraction,
        derived.test_work_fraction()
    );
    assert_eq!(
        nominal.claim().value.operational_fractions,
        derived.operational_fractions()
    );
    assert_eq!(nominal.claim().value.corrected_before_cap.value(), 0.125);
    assert_eq!(nominal.claim().value.recovery_fraction.value(), 0.125);
    assert!(!nominal.claim().value.cap_applied);
}
