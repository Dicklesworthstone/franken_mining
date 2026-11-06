//! Constructed lot observations and independent exact/linearized known answers.
//! Determinism: DeterministicPerIsa (covariance); nominal arithmetic is Exact.

use super::*;
use crate::flowsheet::{
    StocktakeKind, StreamRoute, StreamSpec, UnitId, UnitKind, UnitSpec, account_flowsheet,
};
use fmine_claim::{Confidentiality, LabelRegistry, UuTerm};
use fmine_core::{anchor::SourceAnchor, hash::sha256};
use fmine_evidence::{EpistemicStatus, EvidenceClass, KnowledgeState};

fn q(s: &str) -> ExactQty {
    ExactQty::from_decimal_str(s).unwrap()
}
fn copper() -> ElementKey {
    ElementKey {
        element: "Cu".into(),
        measurand: "total-Cu/all-phases".into(),
    }
}
fn source<T>(
    metric: &str,
    value: T,
    tag: &str,
    additivity: AdditivityClass,
    support: SpatialSupport,
) -> Claim<T> {
    Claim::new(
        metric,
        value,
        tag,
        Representation::Deterministic,
        support,
        additivity,
        EvidenceClass::Assumed,
        EpistemicStatus::KnownKnown,
        KnowledgeState::Known,
        Confidentiality::public(),
        vec![],
        vec![],
        vec![SourceAnchor::new(
            "crates/fmine-plant/src/raw_measurements/tests.rs",
            sha256(include_bytes!("tests.rs")).0,
            format!("constructed source {tag}"),
        )],
    )
}
fn bind(input: RawTransferInput) -> Claim<RawTransferInput> {
    let tag = input.canonical_tag();
    let support = if matches!(input.value, RawTransferValue::Declaration { .. }) {
        SpatialSupport::PeriodAggregate
    } else {
        SpatialSupport::PlantLot
    };
    let additivity = input.value.additivity();
    source(INPUT_METRIC, input, &tag, additivity, support)
}
fn rebind<T: Clone>(input: &mut Claim<T>, tag: &str) {
    *input = Claim::new(
        input.metric_key.clone(),
        input.value.clone(),
        tag,
        input.representation.clone(),
        input.support,
        input.additivity,
        input.evidence,
        input.epistemic,
        input.state,
        input.confidentiality.clone(),
        input.uu_terms.clone(),
        input.parents.clone(),
        input.anchors.clone(),
    );
}
fn refresh(input: &mut Claim<RawTransferInput>) {
    rebind(input, &input.value.canonical_tag());
}
fn refresh_model(input: &mut Claim<RawTransferErrorModel>) {
    rebind(input, &input.value.canonical_tag());
}
fn lots(values: &[(&str, &str, &str, &str)]) -> Vec<Claim<RawTransferInput>> {
    let scope = FlowsheetScope {
        boundary: "constructed-lot-transfer/v1".into(),
        interval: ValidTime::new(0, 20).unwrap(),
    };
    let input = |value| RawTransferInput {
        scope: scope.clone(),
        stream: StreamId::new("feed").unwrap(),
        dry_basis: "matched-bulk-moisture-and-assay-dry-state/v1".into(),
        physical_time: scope.interval,
        value,
    };
    let mut result = vec![bind(input(RawTransferValue::Declaration {
        lot_ids: values.iter().map(|v| v.0.into()).collect(),
        elements: vec![copper()],
        complete: true,
        nonoverlapping_lots: true,
        moisture_is_h2o: true,
        assays_are_total_elemental: true,
    }))];
    // Distinct physical lots may traverse the same period; completeness and
    // nonoverlap are explicit source assertions, not inferred from timestamps.
    for &(name, mass, moisture, grade) in values {
        for value in [
            RawTransferValue::WetMass {
                lot: name.into(),
                kg: q(mass),
            },
            RawTransferValue::Moisture {
                lot: name.into(),
                wet_mass_fraction: q(moisture),
            },
            RawTransferValue::DryAssay {
                lot: name.into(),
                element: copper(),
                dry_mass_fraction: q(grade),
            },
        ] {
            result.push(bind(input(value)));
        }
    }
    result
}
fn fixture() -> Vec<Claim<RawTransferInput>> {
    lots(&[("L1", "100", "0.1", "0.01"), ("L2", "200", "0.2", "0.02")])
}
fn model(inputs: &[Claim<RawTransferInput>]) -> Claim<RawTransferErrorModel> {
    let admitted = admit(inputs).unwrap();
    let n = admitted.sources.len() - 1;
    let mut correlation = vec![0.0; n * n];
    for i in 0..n {
        correlation[i * n + i] = 1.0;
    }
    let mass_indices: Vec<_> = admitted.sources[1..]
        .iter()
        .enumerate()
        .filter_map(|(i, s)| matches!(s.value.value, RawTransferValue::WetMass { .. }).then_some(i))
        .collect();
    if mass_indices.len() >= 2 {
        let (a, b) = (mass_indices[0], mass_indices[1]);
        correlation[a * n + b] = 0.5;
        correlation[b * n + a] = 0.5;
    }
    let value = RawTransferErrorModel {
        scope: inputs[0].value.scope.clone(),
        stream: inputs[0].value.stream.clone(),
        declaration_id: admitted.sources[0].claim_id,
        observation_ids: observation_order(inputs).unwrap(),
        standard_uncertainties: admitted.sources[1..]
            .iter()
            .map(|s| match s.value.value {
                RawTransferValue::WetMass { .. } => {
                    StandardUncertainty::MassKg(Kilograms::new(1.0))
                }
                RawTransferValue::Moisture { .. } => StandardUncertainty::Fraction(0.01),
                _ => StandardUncertainty::Fraction(0.001),
            })
            .collect(),
        correlation,
        first_order_approximation_accepted: true,
    };
    let tag = value.canonical_tag();
    source(
        ERROR_MODEL_METRIC,
        value,
        &tag,
        AdditivityClass::NonAdditive,
        SpatialSupport::PeriodAggregate,
    )
}
fn close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() <= 2e-12 * expected.abs().max(1e-300),
        "{actual} != {expected}"
    );
}
fn refuses(
    inputs: &[Claim<RawTransferInput>],
    model: &Claim<RawTransferErrorModel>,
    code: RefusalCode,
) {
    assert_eq!(
        project_transfer(inputs, model).unwrap_err().code,
        code.as_str()
    );
}

#[test]
fn correlated_lots_match_independent_full_covariance_and_exact_material_oracles() {
    let inputs = fixture();
    let error = model(&inputs);
    let receipt = project_transfer(&inputs, &error).unwrap();
    let output = &receipt.claim().value;
    assert_eq!(output.amounts.dry_solids_kg, q("250"));
    assert_eq!(output.amounts.water_kg, q("50"));
    assert_eq!(output.amounts.elements_kg[&copper()], q("4.1"));
    assert_eq!(output.dry_grade_mass_fractions[&copper()], q("0.0164"));
    assert_ne!(output.dry_grade_mass_fractions[&copper()], q("0.015"));
    assert_ne!(
        output.dry_grade_mass_fractions[&copper()],
        ExactQty::new(1, 60).unwrap()
    );
    assert_eq!(
        output.covariance_order,
        vec![
            ProjectedChannel::DryMatter,
            ProjectedChannel::Water,
            ProjectedChannel::Element(copper())
        ]
    );
    // Independent rational expansion of J diag(sigma) R diag(sigma) J'.
    // Only the two weight errors have covariance 1/2 kg²; all other cross
    // terms are explicitly zero. These are not values returned by the kernel.
    let expected = [
        7.17, -4.62, 0.1217, -4.62, 5.07, -0.0842, 0.1217, -0.0842, 0.035881,
    ];
    for (actual, expected) in output.linearized_covariance.iter().zip(expected) {
        close(actual.value(), expected);
    }
    let covariance = &output.linearized_covariance;
    close(
        covariance[0].value() + 2.0 * covariance[1].value() + covariance[4].value(),
        3.0,
    );
    assert_eq!(receipt.inputs(), inputs);
    assert_eq!(receipt.error_model(), &error);
    assert_eq!(receipt.claim().parents.len(), inputs.len() + 1);
    assert_eq!(receipt.claim().state, KnowledgeState::Estimated);
    assert_eq!(receipt.transfer().parents, vec![receipt.claim().claim_id]);
    assert_eq!(
        receipt.transfer().additivity,
        AdditivityClass::ExtensiveAdditive
    );
    receipt.claim().check_id().unwrap();
    receipt.transfer().check_id().unwrap();

    // The exact nominal transfer is consumed by the existing exact graph owner;
    // its approximate covariance is not silently installed as a WLS model.
    let unit = UnitId::new("store").unwrap();
    let scope = output.scope.clone();
    let zero = MaterialAmounts {
        dry_solids_kg: ZERO,
        water_kg: ZERO,
        elements_kg: BTreeMap::from([(copper(), ZERO)]),
    };
    let mut graph = vec![receipt.transfer().clone()];
    for value in [
        FlowsheetValue::Declaration {
            units: vec![UnitSpec {
                id: unit.clone(),
                kind: UnitKind::Storage,
            }],
            streams: vec![StreamSpec {
                id: output.stream.clone(),
                route: StreamRoute::Feed { to: unit.clone() },
            }],
            elements: vec![copper()],
            complete: true,
        },
        FlowsheetValue::Stocktake {
            unit: unit.clone(),
            kind: StocktakeKind::Opening,
            amounts: zero,
        },
        FlowsheetValue::Stocktake {
            unit,
            kind: StocktakeKind::Closing,
            amounts: output.amounts.clone(),
        },
    ] {
        let physical_time = match value {
            FlowsheetValue::Stocktake {
                kind: StocktakeKind::Opening,
                ..
            } => ValidTime::instant(0),
            FlowsheetValue::Stocktake {
                kind: StocktakeKind::Closing,
                ..
            } => ValidTime::instant(20),
            _ => scope.interval,
        };
        let additivity = value.additivity();
        let value = FlowsheetInput {
            scope: scope.clone(),
            physical_time,
            value,
        };
        let tag = value.canonical_tag();
        graph.push(source(
            crate::flowsheet::INPUT_METRIC,
            value,
            &tag,
            additivity,
            SpatialSupport::PeriodAggregate,
        ));
    }
    let accounted = account_flowsheet(&graph).unwrap();
    assert_eq!(accounted.summary().boundary.dry_solids.residual_kg, ZERO);
}

#[test]
fn moisture_rounding_to_one_keeps_exact_dry_matter_and_nonzero_covariance() {
    let inputs = lots(&[("L1", "1", "0.999999999999999999", "0.25")]);
    assert_eq!(inputs[2].value.value.amount().unwrap().to_f64(), 1.0);
    let mut error = model(&inputs);
    error.value.standard_uncertainties = vec![
        StandardUncertainty::MassKg(Kilograms::new(1.0)),
        StandardUncertainty::Fraction(0.0),
        StandardUncertainty::Fraction(0.0),
    ];
    refresh_model(&mut error);
    let receipt = project_transfer(&inputs, &error).unwrap();
    let result = &receipt.claim().value;
    assert_eq!(result.amounts.dry_solids_kg, q("0.000000000000000001"));
    assert_eq!(result.amounts.water_kg, q("0.999999999999999999"));
    assert_eq!(
        result.amounts.elements_kg[&copper()],
        q("0.00000000000000000025")
    );
    close(result.linearized_covariance[0].value(), 1e-36);
    close(result.linearized_covariance[1].value(), 1e-18);
    close(result.linearized_covariance[8].value(), 6.25e-38);
}

#[test]
fn multiple_elements_are_distinct_carriers_and_require_a_physical_fraction_sum() {
    let mut inputs = lots(&[("L1", "100", "0.1", "0.01")]);
    let gold = ElementKey {
        element: "Au".into(),
        measurand: "total-Au/all-phases".into(),
    };
    if let RawTransferValue::Declaration { elements, .. } = &mut inputs[0].value.value {
        elements.push(gold.clone());
    }
    refresh(&mut inputs[0]);
    let mut gold_input = inputs[3].value.clone();
    gold_input.value = RawTransferValue::DryAssay {
        lot: "L1".into(),
        element: gold.clone(),
        dry_mass_fraction: q("0.005"),
    };
    inputs.push(bind(gold_input));
    let error = model(&inputs);
    let result = project_transfer(&inputs, &error).unwrap();
    assert_eq!(result.claim().value.amounts.elements_kg[&gold], q("0.45"));
    assert_eq!(
        result.claim().value.amounts.elements_kg[&copper()],
        q("0.9")
    );
    assert_eq!(result.claim().value.linearized_covariance.len(), 16);
    if let RawTransferValue::DryAssay {
        dry_mass_fraction, ..
    } = &mut inputs[4].value.value
    {
        *dry_mass_fraction = q("0.995");
    }
    refresh(&mut inputs[4]);
    refuses(&inputs, &error, RefusalCode::OutsidePhysicalDomain109);
}

#[test]
fn zero_uncertainty_is_explicit_and_lost_numeric_range_is_refused() {
    let inputs = fixture();
    let mut error = model(&inputs);
    for scale in &mut error.value.standard_uncertainties {
        *scale = match scale {
            StandardUncertainty::MassKg(_) => StandardUncertainty::MassKg(Kilograms::new(0.0)),
            _ => StandardUncertainty::Fraction(0.0),
        };
    }
    refresh_model(&mut error);
    assert!(
        project_transfer(&inputs, &error)
            .unwrap()
            .claim()
            .value
            .linearized_covariance
            .iter()
            .all(|v| v.value() == 0.0)
    );
    for value in [f64::from_bits(1), 1e-200, f64::MAX] {
        let mut error = error.clone();
        error.value.standard_uncertainties[1] = StandardUncertainty::Fraction(value);
        refresh_model(&mut error);
        refuses(&inputs, &error, RefusalCode::NumericalCertificateFailed207);
    }
    let inputs = lots(&[("L1", "1", "0.9", "0.1")]);
    let mut error = model(&inputs);
    error.value.standard_uncertainties[0] =
        StandardUncertainty::MassKg(Kilograms::new(f64::from_bits(1)));
    refresh_model(&mut error);
    refuses(&inputs, &error, RefusalCode::NumericalCertificateFailed207);
    let maximum = i128::MAX.to_string();
    let inputs = lots(&[("L1", &maximum, "0", "0"), ("L2", &maximum, "0", "0")]);
    refuses(
        &inputs,
        &model(&inputs),
        RefusalCode::NumericalCertificateFailed207,
    );
}

#[test]
fn undeclared_populations_wrong_support_and_unphysical_raw_inputs_refuse() {
    let original = fixture();
    let error = model(&original);
    for flag in 0..4 {
        let mut inputs = original.clone();
        if let RawTransferValue::Declaration {
            complete,
            nonoverlapping_lots,
            moisture_is_h2o,
            assays_are_total_elemental,
            ..
        } = &mut inputs[0].value.value
        {
            *match flag {
                0 => complete,
                1 => nonoverlapping_lots,
                2 => moisture_is_h2o,
                _ => assays_are_total_elemental,
            } = false;
        }
        refresh(&mut inputs[0]);
        refuses(&inputs, &error, RefusalCode::NoClaimInDomain201);
    }
    for slot in 0..original.len() {
        let mut inputs = original.clone();
        inputs[slot].support = if slot == 0 {
            SpatialSupport::PlantLot
        } else {
            SpatialSupport::PeriodAggregate
        };
        refuses(&inputs, &error, RefusalCode::SupportMismatch102);
    }
    for change in 0..3 {
        let mut inputs = original.clone();
        match change {
            0 => inputs[2].value.dry_basis.push_str("-other"),
            1 => inputs[2].value.physical_time.start_ns += 1,
            _ => inputs[2].value.scope.boundary.push_str("-other"),
        }
        refresh(&mut inputs[2]);
        refuses(&inputs, &error, RefusalCode::SupportMismatch102);
    }
    let mut missing = original.clone();
    missing.pop();
    refuses(&missing, &error, RefusalCode::BasisMismatch105);
    let mut duplicate = original.clone();
    duplicate[4] = duplicate[1].clone();
    refuses(&duplicate, &error, RefusalCode::BasisMismatch105);
    for (mass, moisture, grade) in [
        ("0", "0.1", "0.01"),
        ("-1", "0.1", "0.01"),
        ("100", "1", "0.01"),
        ("100", "-0.1", "0.01"),
        ("100", "0.1", "1.01"),
    ] {
        refuses(
            &lots(&[("L1", mass, moisture, grade)]),
            &error,
            RefusalCode::OutsidePhysicalDomain109,
        );
    }
}

#[test]
fn error_model_requires_matching_sources_dimensions_and_valid_declared_correlation() {
    let inputs = fixture();
    let original = model(&inputs);
    for change in 0..4 {
        let mut error = original.clone();
        match change {
            0 => error.value.observation_ids.swap(0, 1),
            1 => error.value.declaration_id = inputs[1].claim_id,
            2 => error.value.stream = StreamId::new("other").unwrap(),
            _ => error.value.scope.boundary.push_str("-other"),
        }
        refresh_model(&mut error);
        refuses(&inputs, &error, RefusalCode::SupportMismatch102);
    }
    for change in 0..3 {
        let mut error = original.clone();
        match change {
            0 => {
                error.value.correlation.pop();
            }
            1 => {
                error.value.standard_uncertainties.pop();
            }
            _ => error.value.standard_uncertainties[0] = StandardUncertainty::Fraction(1.0),
        }
        refresh_model(&mut error);
        refuses(&inputs, &error, RefusalCode::BasisMismatch105);
    }
    for value in [f64::NAN, f64::INFINITY, -0.01] {
        let mut error = original.clone();
        error.value.standard_uncertainties[1] = StandardUncertainty::Fraction(value);
        refresh_model(&mut error);
        refuses(&inputs, &error, RefusalCode::OutsidePhysicalDomain109);
    }
    for value in [0.0, 0.5, f64::NAN, f64::INFINITY, 1.1] {
        let mut error = original.clone();
        error.value.correlation[0] = value;
        refresh_model(&mut error);
        refuses(&inputs, &error, RefusalCode::OutsidePhysicalDomain109);
    }
    for asymmetric in [true, false] {
        let mut error = original.clone();
        error.value.correlation[1] = 1.0;
        if !asymmetric {
            error.value.correlation[6] = 1.0;
        }
        refresh_model(&mut error);
        refuses(&inputs, &error, RefusalCode::NumericalCertificateFailed207);
    }
    let mut error = original;
    error.value.first_order_approximation_accepted = false;
    refresh_model(&mut error);
    refuses(&inputs, &error, RefusalCode::NoClaimInDomain201);
}

#[test]
fn raw_payloads_and_covariance_models_are_rebound_independently_of_claim_ids() {
    let inputs = fixture();
    let error = model(&inputs);
    let mut changed = inputs.clone();
    if let RawTransferValue::WetMass { kg, .. } = &mut changed[1].value.value {
        *kg = q("101");
    }
    changed[1].check_id().unwrap();
    refuses(&changed, &error, RefusalCode::FrozenRecordMutation507);
    let mut changed = error.clone();
    changed.value.correlation[3] = 0.4;
    changed.check_id().unwrap();
    refuses(&inputs, &changed, RefusalCode::FrozenRecordMutation507);
    for slot in 0..inputs.len() {
        let mut changed = inputs.clone();
        changed[slot].stale = true;
        refuses(&changed, &error, RefusalCode::StaleAnchor502);
    }
    let mut changed = error.clone();
    changed.calibration_failed = true;
    refuses(&inputs, &changed, RefusalCode::CalibrationRequired205);
    let mut changed = inputs.clone();
    changed[2].anchors.clear();
    refresh(&mut changed[2]);
    refuses(&changed, &error, RefusalCode::NoClaimInDomain201);
}

#[test]
fn receipt_identity_and_transfer_preserve_complete_source_metadata() {
    let mut inputs = fixture();
    let mut error = model(&inputs);
    let baseline = project_transfer(&inputs, &error).unwrap();
    inputs.reverse();
    assert_eq!(project_transfer(&inputs, &error).unwrap(), baseline);
    inputs.reverse();
    let term = UuTerm {
        scenario_id: "segregated-material".into(),
        description: "Constructed unrepresentative assay risk".into(),
        downside_exposure_usd_m: 1.0,
        trigger_indicator: "Full-material resampling differs".into(),
    };
    inputs[2].uu_terms.push(term.clone());
    let changed = project_transfer(&inputs, &error).unwrap();
    assert_ne!(changed.digest(), baseline.digest());
    assert_ne!(changed.transfer().claim_id, baseline.transfer().claim_id);
    assert_eq!(changed.transfer().uu_terms, vec![term.clone()]);
    error.uu_terms.push(term);
    assert!(project_transfer(&inputs, &error).is_ok());
    error.uu_terms[0].description.push_str("-different");
    refuses(&inputs, &error, RefusalCode::UnbudgetedUnknowns204);

    let mut inputs = fixture();
    let labels = LabelRegistry::new(["JV-A"], ["W-A"]).unwrap();
    inputs[3].confidentiality = Confidentiality::venture(labels.venture("JV-A").unwrap());
    inputs[3].state = KnowledgeState::Estimated;
    refresh(&mut inputs[3]);
    let mut error = model(&inputs);
    error.confidentiality = Confidentiality::mnpi(labels.wall("W-A").unwrap());
    refresh_model(&mut error);
    let output = project_transfer(&inputs, &error).unwrap();
    assert_eq!(output.claim().evidence, EvidenceClass::Assumed);
    assert_eq!(output.transfer().state, KnowledgeState::Estimated);
    assert_eq!(
        output.transfer().confidentiality,
        inputs[3].confidentiality.join(&error.confidentiality)
    );
    assert!(
        output
            .transfer()
            .verify_export_clearance(&Confidentiality::public())
            .is_err()
    );
}
