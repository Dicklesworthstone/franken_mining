//! Constructed graph observations with an analytical two-unit WLS oracle.
//! Determinism: DeterministicPerIsa

use super::*;
use crate::flowsheet::{
    OutflowRole, StreamRoute, StreamSpec, UnitKind, UnitSpec, account_flowsheet,
};
use fmine_claim::{Confidentiality, LabelRegistry, UuTerm};
use fmine_core::{anchor::SourceAnchor, hash::sha256};
use fmine_evidence::{EpistemicStatus, EvidenceClass};
use fmine_time::ValidTime;

pub(super) fn q(value: &str) -> ExactQty {
    ExactQty::from_decimal_str(value).unwrap()
}
fn unit(name: &str) -> UnitId {
    UnitId::new(name).unwrap()
}
fn stream(name: &str) -> StreamId {
    StreamId::new(name).unwrap()
}
fn gold() -> ElementKey {
    ElementKey {
        element: "Au".into(),
        measurand: "total-Au/all-phases".into(),
    }
}
pub(super) fn close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() < 2e-10 * expected.abs().max(1.0),
        "{actual} != {expected}"
    );
}
fn scope() -> FlowsheetScope {
    FlowsheetScope {
        boundary: "synthetic-joint-circuit/v1".into(),
        interval: ValidTime::new(0, 10).unwrap(),
    }
}
fn source<T>(metric: &str, value: T, tag: &str, additivity: AdditivityClass) -> Claim<T> {
    Claim::new(
        metric,
        value,
        tag,
        Representation::Deterministic,
        SpatialSupport::PeriodAggregate,
        additivity,
        EvidenceClass::Assumed,
        EpistemicStatus::KnownKnown,
        KnowledgeState::Known,
        Confidentiality::public(),
        vec![],
        vec![],
        vec![SourceAnchor::new(
            "crates/fmine-plant/src/flowsheet/reconciliation/tests.rs",
            sha256(include_bytes!("tests.rs")).0,
            "constructed complete graph, analytical error model and declared test policy",
        )],
    )
}
fn bind(value: FlowsheetValue) -> Claim<FlowsheetInput> {
    let physical_time = match value {
        FlowsheetValue::Stocktake {
            kind: StocktakeKind::Opening,
            ..
        } => ValidTime::instant(0),
        FlowsheetValue::Stocktake {
            kind: StocktakeKind::Closing,
            ..
        } => ValidTime::instant(10),
        _ => scope().interval,
    };
    let additivity = value.additivity();
    let input = FlowsheetInput {
        scope: scope(),
        physical_time,
        value,
    };
    let tag = input.canonical_tag();
    source(super::super::INPUT_METRIC, input, &tag, additivity)
}
fn refresh<T: Clone>(claim: &mut Claim<T>, tag: &str) {
    *claim = Claim::new(
        claim.metric_key.clone(),
        claim.value.clone(),
        tag,
        claim.representation.clone(),
        claim.support,
        claim.additivity,
        claim.evidence,
        claim.epistemic,
        claim.state,
        claim.confidentiality.clone(),
        claim.uu_terms.clone(),
        claim.parents.clone(),
        claim.anchors.clone(),
    );
}
pub(super) fn refresh_model(model: &mut Claim<FlowsheetErrorModel>) {
    let tag = model.value.canonical_tag();
    refresh(model, &tag);
}
pub(super) fn amounts(dry: &str, water: &str, au: &str) -> MaterialAmounts {
    MaterialAmounts {
        dry_solids_kg: q(dry),
        water_kg: q(water),
        elements_kg: BTreeMap::from([(gold(), q(au))]),
    }
}
pub(super) fn fixture() -> Vec<Claim<FlowsheetInput>> {
    let routes = vec![
        ("feed", StreamRoute::Feed { to: unit("mill") }),
        (
            "mill-discharge",
            StreamRoute::Internal {
                from: unit("mill"),
                to: unit("classifier"),
                source_role: OutflowRole::Product,
            },
        ),
        (
            "recycle",
            StreamRoute::Internal {
                from: unit("classifier"),
                to: unit("mill"),
                source_role: OutflowRole::Other,
            },
        ),
        (
            "product",
            StreamRoute::Product {
                from: unit("classifier"),
            },
        ),
        (
            "tails",
            StreamRoute::OtherOutflow {
                from: unit("classifier"),
            },
        ),
    ];
    let mut inputs = vec![bind(FlowsheetValue::Declaration {
        units: vec![
            UnitSpec {
                id: unit("mill"),
                kind: UnitKind::Mill,
            },
            UnitSpec {
                id: unit("classifier"),
                kind: UnitKind::Classifier,
            },
        ],
        streams: routes
            .into_iter()
            .map(|(id, route)| StreamSpec {
                id: stream(id),
                route,
            })
            .collect(),
        elements: vec![gold()],
        complete: true,
    })];
    // The unperturbed dry feed is 100: raw mill residual is +1, classifier 0.
    for (id, dry, water, au) in [
        ("feed", "101", "200", "1"),
        ("mill-discharge", "150", "300", "1.5"),
        ("recycle", "60", "120", "0.6"),
        ("product", "80", "150", "0.8"),
        ("tails", "5", "20", "0.05"),
    ] {
        inputs.push(bind(FlowsheetValue::Transfer {
            stream: stream(id),
            amounts: amounts(dry, water, au),
        }));
    }
    for (id, kind, dry, water, au) in [
        ("mill", StocktakeKind::Opening, "20", "40", "0.2"),
        ("mill", StocktakeKind::Closing, "30", "60", "0.3"),
        ("classifier", StocktakeKind::Opening, "10", "20", "0.1"),
        ("classifier", StocktakeKind::Closing, "15", "30", "0.15"),
    ] {
        inputs.push(bind(FlowsheetValue::Stocktake {
            unit: unit(id),
            kind,
            amounts: amounts(dry, water, au),
        }));
    }
    inputs
}
pub(super) fn model(inputs: &[Claim<FlowsheetInput>]) -> Claim<FlowsheetErrorModel> {
    let keys = observation_order(inputs).unwrap();
    let n = keys.len();
    let covariance = (0..n * n)
        .map(|i| {
            KilogramsSquared::new(if i / n != i % n {
                0.0
            } else if matches!(keys[i / n].channel, MaterialChannel::Element(_)) {
                1e-4
            } else {
                1.0
            })
        })
        .collect();
    let declaration = inputs
        .iter()
        .find(|s| matches!(s.value.value, FlowsheetValue::Declaration { .. }))
        .unwrap();
    let value = FlowsheetErrorModel {
        scope: scope(),
        declaration_id: declaration.claim_id,
        closure_tolerances_kg: keys
            .iter()
            .map(|key| {
                (
                    key.channel.clone(),
                    Kilograms::new(if matches!(key.channel, MaterialChannel::Element(_)) {
                        1e-12
                    } else {
                        1e-9
                    }),
                )
            })
            .collect(),
        observations: keys,
        covariance,
        unbiased_gaussian_errors: true,
        global_chi_square_limit: 9.0,
        normalized_adjustment_limit: None,
    };
    let tag = value.canonical_tag();
    source(
        ERROR_MODEL_METRIC,
        value,
        &tag,
        AdditivityClass::NonAdditive,
    )
}
pub(super) fn index(keys: &[ObservationKey], name: &str, channel: MaterialChannel) -> usize {
    keys.iter()
        .position(|key| {
            key.location == ObservationLocation::Transfer(stream(name)) && key.channel == channel
        })
        .unwrap()
}
pub(super) fn storage(open: MaterialAmounts, close: MaterialAmounts) -> Vec<Claim<FlowsheetInput>> {
    let elements = open.elements_kg.keys().cloned().collect();
    vec![
        bind(FlowsheetValue::Declaration {
            units: vec![UnitSpec {
                id: unit("tank"),
                kind: UnitKind::Storage,
            }],
            streams: vec![],
            elements,
            complete: true,
        }),
        bind(FlowsheetValue::Stocktake {
            unit: unit("tank"),
            kind: StocktakeKind::Opening,
            amounts: open,
        }),
        bind(FlowsheetValue::Stocktake {
            unit: unit("tank"),
            kind: StocktakeKind::Closing,
            amounts: close,
        }),
    ]
}
pub(super) fn carriers(dry: &str, water: &str) -> MaterialAmounts {
    MaterialAmounts {
        dry_solids_kg: q(dry),
        water_kg: q(water),
        elements_kg: BTreeMap::new(),
    }
}

#[test]
fn joint_recycle_fit_matches_two_by_two_analytic_oracle() {
    let inputs = fixture();
    assert_eq!(
        account_flowsheet(&inputs).unwrap_err().code,
        "FMINE-REF-101"
    );
    let model = model(&inputs);
    let outcome = reconcile_flowsheet(&inputs, &model).unwrap();
    let claim = outcome.admitted_claim().unwrap();
    let d = &claim.value;
    // For dry mass, A V A' = [[6,-2],[-2,5]], residual [0,1].
    // Its inverse gives multiplier [1/13,3/13] and chi-square 3/13.
    close(d.chi_square, 3.0 / 13.0);
    assert_eq!(d.degrees_of_freedom, 6); // 2 units × 3 channels; no outer duplicate rows.
    assert_eq!(d.observations.len(), 27); // 9 observations, NOT duplicated internal edges.
    for (name, expected) in [
        ("feed", -3.0 / 13.0),
        ("mill-discharge", 2.0 / 13.0),
        ("recycle", -2.0 / 13.0),
        ("product", 1.0 / 13.0),
        ("tails", 1.0 / 13.0),
    ] {
        let i = index(&d.observations, name, MaterialChannel::DrySolids);
        close(d.adjustments_kg[i].value(), expected);
    }
    let f = index(&d.observations, "feed", MaterialChannel::DrySolids);
    let m = index(
        &d.observations,
        "mill-discharge",
        MaterialChannel::DrySolids,
    );
    let n = d.observations.len();
    close(d.posterior_covariance[f * n + f].value(), 10.0 / 13.0);
    close(d.adjustment_covariance[f * n + f].value(), 3.0 / 13.0);
    close(d.posterior_covariance[f * n + m].value(), 2.0 / 13.0);
    close(d.boundary.posterior_covariance[0].value(), 37.0 / 26.0); // includes covariance between both opening stocks
    close(d.boundary.estimates_kg[3].value(), 101.0 - 3.0 / 13.0);
    for row in &d.conservation {
        close(row.reconciled_residual_kg.value(), 0.0);
    }
    for row in &d.boundary.conservation {
        close(row.reconciled_residual_kg.value(), 0.0);
    }
    assert_eq!(d.boundary.conservation[0].raw_residual_exact_kg, q("1"));
    assert_eq!(claim.evidence, EvidenceClass::Assumed);
    assert_eq!(claim.state, KnowledgeState::Estimated);
    assert_eq!(claim.parents.len(), inputs.len() + 1);
    let FlowsheetReconciliationOutcome::Admitted(receipt) = outcome else {
        unreachable!()
    };
    assert_eq!(receipt.error_model(), &model);
    for input in &inputs {
        assert!(receipt.inputs().contains(input));
    }
    assert_eq!(inputs, fixture());
}

#[test]
fn off_diagonal_errors_change_the_coupled_fit_and_cross_element_inference() {
    let inputs = fixture();
    let mut model = model(&inputs);
    let n = model.value.observations.len();
    let f = index(
        &model.value.observations,
        "feed",
        MaterialChannel::DrySolids,
    );
    let p = index(
        &model.value.observations,
        "product",
        MaterialChannel::DrySolids,
    );
    model.value.covariance[f * n + p] = KilogramsSquared::new(0.25);
    model.value.covariance[p * n + f] = KilogramsSquared::new(0.25);
    refresh_model(&mut model);
    let outcome = reconcile_flowsheet(&inputs, &model).unwrap();
    let d = &outcome.admitted_claim().unwrap().value;
    // A V A' = [[6,-9/4],[-9/4,5]]. Correlation is not discarded.
    close(d.chi_square, 32.0 / 133.0);
    close(d.adjustments_kg[f].value(), -29.0 / 133.0);
    close(d.adjustments_kg[p].value(), 4.0 / 133.0);
    let au = index(
        &model.value.observations,
        "feed",
        MaterialChannel::Element(gold()),
    );
    model.value.covariance[f * n + au] = KilogramsSquared::new(0.005);
    model.value.covariance[au * n + f] = KilogramsSquared::new(0.005);
    refresh_model(&mut model);
    let coupled = reconcile_flowsheet(&inputs, &model).unwrap();
    let d = &coupled.admitted_claim().unwrap().value;
    assert!(d.adjustments_kg[au].value().abs() > 1e-6);
    assert!(d.posterior_covariance[f * n + au].value().abs() > 1e-6);
    for row in &d.conservation {
        close(row.reconciled_residual_kg.value(), 0.0);
    }
}

#[test]
fn gross_error_and_declared_adjustment_policy_preserve_rejected_evidence() {
    let inputs = fixture();
    for individual in [false, true] {
        let mut model = model(&inputs);
        if individual {
            model.value.normalized_adjustment_limit = Some(0.1);
        } else {
            model.value.global_chi_square_limit = 0.1;
        }
        refresh_model(&mut model);
        let result = reconcile_flowsheet(&inputs, &model).unwrap();
        assert_eq!(result.admitted_claim().unwrap_err().code, "FMINE-REF-101");
        let FlowsheetReconciliationOutcome::Rejected(rejection) = result else {
            panic!("expected rejection")
        };
        assert_eq!(rejection.inputs().len(), inputs.len());
        assert_eq!(rejection.error_model(), &model);
        assert_eq!(rejection.diagnostics().observations.len(), 27);
        close(rejection.diagnostics().chi_square, 3.0 / 13.0);
        assert!(!rejection.digest().is_empty());
    }
}

#[test]
fn negative_and_impossible_fitted_compositions_are_rejected_without_clamping() {
    let inputs = storage(carriers("0", "1"), carriers("10", "1"));
    let mut model = model(&inputs);
    model.value.global_chi_square_limit = 1e9;
    model.value.covariance[1] = KilogramsSquared::new(-0.9);
    model.value.covariance[4] = KilogramsSquared::new(-0.9);
    refresh_model(&mut model);
    let result = reconcile_flowsheet(&inputs, &model).unwrap();
    assert_eq!(result.admitted_claim().unwrap_err().code, "FMINE-REF-109");
    assert!(
        result
            .diagnostics()
            .estimates_kg
            .iter()
            .any(|v| v.value() < 0.0)
    );

    // Each observed vector is possible. Separate precision declarations favour
    // the first dry mass and the second gold mass, so a linear fit is impossible.
    let inputs = storage(amounts("1", "0", "0.1"), amounts("100", "0", "100"));
    let mut model = self::model(&inputs);
    model.value.global_chi_square_limit = 1e9;
    let n = model.value.observations.len();
    for (i, variance) in [0.0001, 1.0, 100.0, 100.0, 1.0, 0.0001]
        .into_iter()
        .enumerate()
    {
        model.value.covariance[i * n + i] = KilogramsSquared::new(variance);
    }
    refresh_model(&mut model);
    let result = reconcile_flowsheet(&inputs, &model).unwrap();
    let error = result.admitted_claim().unwrap_err();
    assert_eq!(error.code, "FMINE-REF-109");
    assert!(error.what.contains("composition"));
    assert!(
        result.diagnostics().estimates_kg[2].value() > result.diagnostics().estimates_kg[0].value()
    );
}

#[test]
fn declaration_coordinate_binding_and_semantic_mutation_are_enforced() {
    let inputs = fixture();
    let original = model(&inputs);
    let mut changed = original.clone();
    changed.value.observations.swap(0, 1);
    refresh_model(&mut changed);
    assert_eq!(
        reconcile_flowsheet(&inputs, &changed).unwrap_err().code,
        "FMINE-REF-102"
    );
    changed = original.clone();
    changed.value.declaration_id = inputs[1].claim_id;
    refresh_model(&mut changed);
    assert_eq!(
        reconcile_flowsheet(&inputs, &changed).unwrap_err().code,
        "FMINE-REF-102"
    );
    changed = original.clone();
    changed.value.covariance[0] = KilogramsSquared::new(5.0);
    assert!(changed.check_id().is_ok()); // generic Claim identity cannot detect payload mutation
    assert!(reconcile_flowsheet(&inputs, &changed).is_err());
    let mut changed_inputs = inputs.clone();
    if let FlowsheetValue::Transfer { amounts, .. } = &mut changed_inputs[1].value.value {
        amounts.water_kg = q("201");
    }
    assert!(changed_inputs[1].check_id().is_ok());
    assert!(reconcile_flowsheet(&changed_inputs, &original).is_err());
}

#[test]
fn malformed_covariance_or_missing_tolerances_never_return_a_candidate() {
    let inputs = fixture();
    for defect in 0..7 {
        let mut model = model(&inputs);
        let n = model.value.observations.len();
        match defect {
            0 => model.value.covariance[0] = KilogramsSquared::new(0.0),
            1 => model.value.covariance[1] = KilogramsSquared::new(0.5),
            2 => {
                model.value.covariance[1] = KilogramsSquared::new(2.0);
                model.value.covariance[n] = KilogramsSquared::new(2.0);
            }
            3 => {
                model.value.covariance.pop();
            }
            4 => {
                model
                    .value
                    .closure_tolerances_kg
                    .remove(&MaterialChannel::Water);
            }
            5 => {
                model
                    .value
                    .closure_tolerances_kg
                    .insert(MaterialChannel::Water, Kilograms::new(-1.0));
            }
            _ => model.value.unbiased_gaussian_errors = false,
        }
        refresh_model(&mut model);
        assert!(
            reconcile_flowsheet(&inputs, &model).is_err(),
            "defect {defect}"
        );
    }
}

#[test]
fn shared_admission_refuses_missing_measurements_and_incompatible_support() {
    let inputs = fixture();
    let model = model(&inputs);
    for omitted in [0, 1, inputs.len() - 1] {
        let mut incomplete = inputs.clone();
        incomplete.remove(omitted);
        assert!(reconcile_flowsheet(&incomplete, &model).is_err());
    }
    let mut changed = inputs.clone();
    changed[1].value.physical_time = ValidTime::new(0, 9).unwrap();
    let tag = changed[1].value.canonical_tag();
    refresh(&mut changed[1], &tag);
    assert_eq!(
        reconcile_flowsheet(&changed, &model).unwrap_err().code,
        "FMINE-REF-102"
    );
    changed = inputs.clone();
    changed[2].anchors.clear();
    let tag = changed[2].value.canonical_tag();
    refresh(&mut changed[2], &tag);
    assert!(reconcile_flowsheet(&changed, &model).is_err());
}

#[test]
fn exact_imbalance_and_unrepresentable_adjustments_cannot_disappear() {
    let inputs = storage(
        carriers("9007199254740992", "0"),
        carriers("9007199254740993", "0"),
    );
    let model = model(&inputs);
    assert_eq!(
        reconcile_flowsheet(&inputs, &model).unwrap_err().code,
        "FMINE-REF-207"
    );
}

#[test]
fn declaration_and_input_permutations_have_identical_receipts() {
    let mut inputs = fixture();
    let model = model(&inputs);
    let first = reconcile_flowsheet(&inputs, &model).unwrap();
    if let FlowsheetValue::Declaration { units, streams, .. } = &mut inputs[0].value.value {
        units.reverse();
        streams.reverse();
    }
    inputs.reverse();
    let second = reconcile_flowsheet(&inputs, &model).unwrap();
    assert_eq!(
        first.admitted_claim().unwrap(),
        second.admitted_claim().unwrap()
    );
    let (
        FlowsheetReconciliationOutcome::Admitted(first),
        FlowsheetReconciliationOutcome::Admitted(second),
    ) = (first, second)
    else {
        unreachable!()
    };
    assert_eq!(first.digest(), second.digest());
}

#[test]
fn model_and_canceled_internal_sources_keep_labels_and_envelope_identity() {
    let mut inputs = fixture();
    let mut model = model(&inputs);
    let first = reconcile_flowsheet(&inputs, &model).unwrap();
    let registry = LabelRegistry::new(["JV-A"], ["W-A"]).unwrap();
    let restricted = Confidentiality::mnpi(registry.wall("W-A").unwrap());
    model.confidentiality = restricted.clone();
    refresh_model(&mut model);
    let term = UuTerm {
        scenario_id: "stocktake-gap".into(),
        description: "Constructed omitted holdup".into(),
        downside_exposure_usd_m: 1.0,
        trigger_indicator: "Independent stocktake".into(),
    };
    // ClaimId omits this field; the receipt must still bind it, even on recycle.
    let recycle = inputs.iter_mut().find(|s| matches!(&s.value.value, FlowsheetValue::Transfer { stream: id, .. } if id.as_str() == "recycle")).unwrap();
    recycle.uu_terms.push(term.clone());
    let result = reconcile_flowsheet(&inputs, &model).unwrap();
    let claim = result.admitted_claim().unwrap();
    assert_eq!(claim.confidentiality, restricted);
    assert_eq!(claim.uu_terms, vec![term.clone()]);
    assert_ne!(claim.claim_id, first.admitted_claim().unwrap().claim_id);
    assert_eq!(
        claim
            .verify_export_clearance(&Confidentiality::public())
            .unwrap_err()
            .code,
        "FMINE-REF-301"
    );
    let mut conflict = term;
    conflict.description = "Incompatible same-named scenario".into();
    model.uu_terms.push(conflict);
    assert_eq!(
        reconcile_flowsheet(&inputs, &model).unwrap_err().code,
        "FMINE-REF-204"
    );
}

#[test]
fn isolated_storage_has_shared_stock_covariance_and_structural_zero_flow_roles() {
    let inputs = storage(carriers("10", "20"), carriers("12", "20"));
    let model = model(&inputs);
    let result = reconcile_flowsheet(&inputs, &model).unwrap();
    let d = &result.admitted_claim().unwrap().value;
    close(d.estimates_kg[0].value(), 11.0);
    close(d.estimates_kg[2].value(), 11.0);
    close(d.posterior_covariance[2].value(), 0.5);
    assert_eq!(d.degrees_of_freedom, 2);
    for (i, key) in d.boundary.keys.iter().enumerate() {
        if matches!(
            key.role,
            BoundaryRole::Feed | BoundaryRole::Product | BoundaryRole::OtherOutflow
        ) {
            assert_eq!(d.boundary.estimates_kg[i].value(), 0.0);
            assert!(
                d.boundary.posterior_covariance[i * 10..(i + 1) * 10]
                    .iter()
                    .all(|v| v.value() == 0.0)
            );
        }
    }
}
