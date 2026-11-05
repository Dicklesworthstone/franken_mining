//! Constructed measured/latent flows; exact analytic answers and refusal controls.
//! Determinism: DeterministicPerIsa

use super::*;
use crate::flowsheet::reconciliation::BoundaryRole;
use crate::flowsheet::{StocktakeKind, StreamSpec, UnitKind, UnitSpec, account_flowsheet};
use fmine_claim::{Confidentiality, LabelRegistry, Representation, SpatialSupport, UuTerm};
use fmine_core::{anchor::SourceAnchor, hash::sha256};
use fmine_evidence::{EpistemicStatus, EvidenceClass, KnowledgeState};
use fmine_time::ValidTime;

fn q(s: &str) -> ExactQty {
    ExactQty::from_decimal_str(s).unwrap()
}
fn close(a: f64, b: f64) {
    assert!((a - b).abs() < 2e-9 * b.abs().max(1.0), "{a} != {b}");
}
fn stream(s: &str) -> StreamId {
    StreamId::new(s).unwrap()
}
fn gold() -> crate::flowsheet::ElementKey {
    crate::flowsheet::ElementKey {
        element: "Au".into(),
        measurand: "total-Au/all-phases".into(),
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
            "crates/fmine-plant/src/flowsheet/reconciliation/unmeasured/tests.rs",
            sha256(include_bytes!("tests.rs")).0,
            "constructed partial-flow measurements and declared error model",
        )],
    )
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
fn refresh_model(model: &mut Claim<UnmeasuredFlowModel>) {
    let tag = model.value.canonical_tag();
    refresh(model, &tag);
}
fn model(inputs: &[Claim<FlowsheetInput>], unknown: &[&str]) -> Claim<UnmeasuredFlowModel> {
    let unknown = unknown.iter().map(|s| stream(s)).collect();
    let observations = observation_order(inputs, &unknown).unwrap();
    let n = observations.len();
    let declaration = inputs
        .iter()
        .find(|s| matches!(s.value.value, FlowsheetValue::Declaration { .. }))
        .unwrap();
    let value = UnmeasuredFlowModel {
        scope: declaration.value.scope.clone(),
        declaration_id: declaration.claim_id,
        covariance: (0..n * n)
            .map(|i| {
                KilogramsSquared::new(if i / n != i % n {
                    0.0
                } else if matches!(observations[i / n].channel, MaterialChannel::Element(_)) {
                    1e-4
                } else {
                    1.0
                })
            })
            .collect(),
        closure_tolerances_kg: observations
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
        observations,
        unmeasured_streams: unknown,
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
fn partial_fixture(unknown: &[&str]) -> Vec<Claim<FlowsheetInput>> {
    let mut inputs = super::super::tests::fixture();
    inputs.retain(|source|!matches!(&source.value.value,FlowsheetValue::Transfer{stream,..} if unknown.contains(&stream.as_str())));
    inputs
}
fn amount(dry: &str, water: &str, au: &str) -> MaterialAmounts {
    MaterialAmounts {
        dry_solids_kg: q(dry),
        water_kg: q(water),
        elements_kg: BTreeMap::from([(gold(), q(au))]),
    }
}
fn storage(
    open: MaterialAmounts,
    closing: MaterialAmounts,
    parallel: bool,
) -> Vec<Claim<FlowsheetInput>> {
    let scope = FlowsheetScope {
        boundary: "synthetic-unmeasured-store/v1".into(),
        interval: ValidTime::new(0, 10).unwrap(),
    };
    let unit = UnitId::new("store").unwrap();
    let mut streams = vec![StreamSpec {
        id: stream("product"),
        route: StreamRoute::Product { from: unit.clone() },
    }];
    if parallel {
        streams.push(StreamSpec {
            id: stream("tails"),
            route: StreamRoute::OtherOutflow { from: unit.clone() },
        });
    }
    [
        FlowsheetValue::Declaration {
            units: vec![UnitSpec {
                id: unit.clone(),
                kind: UnitKind::Storage,
            }],
            streams,
            elements: vec![gold()],
            complete: true,
        },
        FlowsheetValue::Stocktake {
            unit: unit.clone(),
            kind: StocktakeKind::Opening,
            amounts: open,
        },
        FlowsheetValue::Stocktake {
            unit,
            kind: StocktakeKind::Closing,
            amounts: closing,
        },
    ]
    .into_iter()
    .map(|value| {
        let time = match value {
            FlowsheetValue::Stocktake {
                kind: StocktakeKind::Opening,
                ..
            } => ValidTime::instant(0),
            FlowsheetValue::Stocktake {
                kind: StocktakeKind::Closing,
                ..
            } => ValidTime::instant(10),
            _ => scope.interval,
        };
        let additivity = value.additivity();
        let input = FlowsheetInput {
            scope: scope.clone(),
            physical_time: time,
            value,
        };
        let tag = input.canonical_tag();
        source(crate::flowsheet::INPUT_METRIC, input, &tag, additivity)
    })
    .collect()
}

#[test]
fn missing_tails_in_shared_recycle_have_analytic_mean_joint_covariance_and_redundancy() {
    let inputs = partial_fixture(&["tails"]);
    let model = model(&inputs, &["tails"]);
    assert_eq!(
        account_flowsheet(&inputs).unwrap_err().code,
        "FMINE-REF-201"
    );
    let outcome = reconcile_unmeasured_flowsheet(&inputs, &model).unwrap();
    let d = &outcome.admitted_claim().unwrap().value;
    let n = d.observations.len();
    let total = d.estimates_kg.len();
    assert_eq!(n, 24);
    assert_eq!(total, 27);
    assert_eq!(d.unmeasured.len(), 3);
    assert_eq!(d.unmeasured[0].stream, stream("tails"));
    close(d.estimates_kg[n].value(), 5.4);
    close(d.estimates_kg[n + 1].value(), 20.0);
    close(d.estimates_kg[n + 2].value(), 0.05);
    close(d.posterior_covariance[n * total + n].value(), 21.0 / 5.0);
    let f=d.observations.iter().position(|key|key.channel==MaterialChannel::DrySolids && matches!(&key.location,super::super::ObservationLocation::Transfer(id) if id.as_str()=="feed")).unwrap();
    close(d.estimates_kg[f].value(), 100.8);
    close(d.posterior_covariance[f * total + n].value(), 2.0 / 5.0);
    assert_eq!(d.measured_adjustments_kg.len(), n);
    assert_eq!(d.normalized_adjustments.len(), n);
    let GrossErrorDiagnostic::Available {
        chi_square,
        degrees_of_freedom,
    } = &d.gross_error
    else {
        panic!("three redundant constraints")
    };
    close(*chi_square, 0.2);
    assert_eq!(*degrees_of_freedom, 3);
    let other = d
        .boundary
        .keys
        .iter()
        .position(|key| {
            key.role == BoundaryRole::OtherOutflow && key.channel == MaterialChannel::DrySolids
        })
        .unwrap();
    assert_eq!(d.boundary.observed_totals_kg[other], None);
    assert_eq!(d.boundary.observed_contributions_exact_kg[other], q("0"));
    close(d.boundary.estimates_kg[other].value(), 5.4);
    close(
        d.boundary.posterior_covariance[other * d.boundary.keys.len() + other].value(),
        4.2,
    );
    for row in &d.conservation {
        close(row.reconciled_residual_kg.value(), 0.0);
    }
    for value in d.boundary.reconciled_residuals_kg.values() {
        close(value.value(), 0.0);
    }
    let UnmeasuredFlowOutcome::Admitted(receipt) = outcome else {
        unreachable!()
    };
    assert_eq!(receipt.inputs().len(), 9);
    assert_eq!(receipt.error_model(), &model);
    assert_eq!(receipt.claim().evidence, EvidenceClass::Assumed);
}

#[test]
fn identifiable_zero_redundancy_reports_no_gross_error_test() {
    let inputs = storage(amount("10", "20", "1"), amount("7", "15", "0.8"), false);
    let mut model = model(&inputs, &["product"]);
    model.value.global_chi_square_limit = 1e-20;
    refresh_model(&mut model);
    let result = reconcile_unmeasured_flowsheet(&inputs, &model).unwrap();
    let d = &result.admitted_claim().unwrap().value;
    assert_eq!(d.gross_error, GrossErrorDiagnostic::NoRedundancy);
    assert!(d.normalized_adjustments.iter().all(Option::is_none));
    let n = d.observations.len();
    let total = d.estimates_kg.len();
    for (i, expected) in [3.0, 5.0, 0.2].into_iter().enumerate() {
        close(d.estimates_kg[n + i].value(), expected);
    }
    close(d.posterior_covariance[n * total + n].value(), 2.0);
    close(d.posterior_covariance[n * total].value(), 1.0);
    close(d.posterior_covariance[n * total + 3].value(), -1.0);
    for (fitted, observed) in d.estimates_kg[..n].iter().zip(&d.observed_kg) {
        assert_eq!(fitted, observed);
    }
}

#[test]
fn cross_scale_correlated_stock_errors_survive_zero_redundancy_inference() {
    let inputs = storage(amount("100", "40", "2"), amount("70", "25", "1.4"), false);
    let mut model = model(&inputs, &["product"]);
    let n = model.value.observations.len();
    for (i, variance) in [4.0, 1.0, 0.0004, 4.0, 1.0, 0.0004].into_iter().enumerate() {
        model.value.covariance[i * n + i] = KilogramsSquared::new(variance);
    }
    model.value.covariance[2] = KilogramsSquared::new(0.02);
    model.value.covariance[2 * n] = KilogramsSquared::new(0.02);
    refresh_model(&mut model);
    let result = reconcile_unmeasured_flowsheet(&inputs, &model).unwrap();
    let d = &result.admitted_claim().unwrap().value;
    assert_eq!(d.gross_error, GrossErrorDiagnostic::NoRedundancy);
    let total = d.estimates_kg.len();
    for (c, (mean, variance)) in [(30.0, 8.0), (15.0, 2.0), (0.6, 0.0008)]
        .into_iter()
        .enumerate()
    {
        close(d.estimates_kg[n + c].value(), mean);
        close(
            d.posterior_covariance[(n + c) * total + n + c].value(),
            variance,
        );
    }
    close(d.posterior_covariance[n * total + n + 2].value(), 0.02);
}

#[test]
fn unmeasured_cycle_and_parallel_outlets_refuse_without_fake_priors() {
    let inputs = partial_fixture(&["mill-discharge", "recycle"]);
    let model = model(&inputs, &["mill-discharge", "recycle"]);
    assert_eq!(
        reconcile_unmeasured_flowsheet(&inputs, &model)
            .unwrap_err()
            .code,
        "FMINE-REF-104"
    );
    let inputs = storage(amount("10", "20", "1"), amount("7", "15", "0.8"), true);
    let model = self::model(&inputs, &["product", "tails"]);
    assert_eq!(
        reconcile_unmeasured_flowsheet(&inputs, &model)
            .unwrap_err()
            .code,
        "FMINE-REF-104"
    );
}

#[test]
fn no_unannounced_missing_stream_stock_or_synthetic_measurement_is_admitted() {
    let inputs = partial_fixture(&["tails"]);
    let model = model(&inputs, &["tails"]);
    let mut absent = inputs.clone();
    absent.pop();
    assert_eq!(
        reconcile_unmeasured_flowsheet(&absent, &model)
            .unwrap_err()
            .code,
        "FMINE-REF-201"
    );
    let measured = super::super::tests::fixture();
    assert_eq!(
        reconcile_unmeasured_flowsheet(&measured, &model)
            .unwrap_err()
            .code,
        "FMINE-REF-105"
    );
    let undeclared = partial_fixture(&["tails", "product"]);
    assert_eq!(
        reconcile_unmeasured_flowsheet(&undeclared, &model)
            .unwrap_err()
            .code,
        "FMINE-REF-201"
    );
    let mut unknown = model.clone();
    unknown
        .value
        .unmeasured_streams
        .insert(stream("not-declared"));
    refresh_model(&mut unknown);
    assert_eq!(
        reconcile_unmeasured_flowsheet(&inputs, &unknown)
            .unwrap_err()
            .code,
        "FMINE-REF-105"
    );
    let mut fake_variance = model.clone();
    fake_variance
        .value
        .covariance
        .resize(27 * 27, KilogramsSquared::new(1.0));
    refresh_model(&mut fake_variance);
    assert_eq!(
        reconcile_unmeasured_flowsheet(&inputs, &fake_variance)
            .unwrap_err()
            .code,
        "FMINE-REF-105"
    );
}

#[test]
fn inferred_negative_or_impossible_elemental_mass_stays_rejected() {
    for (opening, closing, detail) in [
        (
            amount("7", "15", "0.8"),
            amount("10", "20", "1"),
            "negative",
        ),
        (
            amount("10", "20", "8"),
            amount("9", "15", "2"),
            "composition",
        ),
    ] {
        let inputs = storage(opening, closing, false);
        let model = model(&inputs, &["product"]);
        let result = reconcile_unmeasured_flowsheet(&inputs, &model).unwrap();
        let refusal = result.admitted_claim().unwrap_err();
        assert_eq!(refusal.code, "FMINE-REF-109");
        assert!(refusal.what.contains(detail), "{refusal:?}");
        let UnmeasuredFlowOutcome::Rejected(r) = result else {
            unreachable!()
        };
        assert_eq!(r.inputs().len(), 3);
        assert_eq!(r.error_model(), &model);
        assert_eq!(r.diagnostics().unmeasured.len(), 3);
    }
}

#[test]
fn residual_redundancy_still_rejects_gross_errors() {
    let mut inputs = partial_fixture(&["tails"]);
    let feed=inputs.iter_mut().find(|s|matches!(&s.value.value,FlowsheetValue::Transfer{stream,..} if stream.as_str()=="feed")).unwrap();
    if let FlowsheetValue::Transfer { amounts, .. } = &mut feed.value.value {
        amounts.dry_solids_kg = q("120");
    }
    let tag = feed.value.canonical_tag();
    refresh(feed, &tag);
    let model = model(&inputs, &["tails"]);
    let result = reconcile_unmeasured_flowsheet(&inputs, &model).unwrap();
    assert_eq!(result.admitted_claim().unwrap_err().code, "FMINE-REF-101");
    let GrossErrorDiagnostic::Available {
        chi_square,
        degrees_of_freedom,
    } = result.diagnostics().gross_error
    else {
        panic!("redundancy present")
    };
    close(chi_square, 80.0);
    assert_eq!(degrees_of_freedom, 3);
}

#[test]
fn model_bindings_tampering_covariance_and_gaussian_assumption_are_enforced() {
    let inputs = partial_fixture(&["tails"]);
    let original = model(&inputs, &["tails"]);
    for defect in 0..6 {
        let mut bad = original.clone();
        match defect {
            0 => {
                bad.value.observations.swap(0, 1);
                refresh_model(&mut bad);
            }
            1 => {
                bad.value.declaration_id = inputs[1].claim_id;
                refresh_model(&mut bad);
            }
            2 => bad.value.covariance[0] = KilogramsSquared::new(2.0),
            3 => {
                bad.value.unbiased_gaussian_errors = false;
                refresh_model(&mut bad);
            }
            4 => {
                bad.value.covariance[0] = KilogramsSquared::new(0.0);
                refresh_model(&mut bad);
            }
            _ => {
                bad.value
                    .closure_tolerances_kg
                    .remove(&MaterialChannel::Water);
                refresh_model(&mut bad);
            }
        };
        assert!(
            reconcile_unmeasured_flowsheet(&inputs, &bad).is_err(),
            "defect {defect}"
        );
    }
}

#[test]
fn missing_flow_declaration_keeps_provenance_labels_and_canonical_order() {
    let mut inputs = partial_fixture(&["tails"]);
    let mut model = model(&inputs, &["tails"]);
    let registry = LabelRegistry::new(["JV-A"], ["W-A"]).unwrap();
    model.confidentiality = Confidentiality::mnpi(registry.wall("W-A").unwrap());
    refresh_model(&mut model);
    let first = reconcile_unmeasured_flowsheet(&inputs, &model).unwrap();
    let claim = first.admitted_claim().unwrap();
    assert_eq!(claim.confidentiality, model.confidentiality);
    assert_eq!(claim.state, KnowledgeState::Estimated);
    assert!(claim.parents.contains(&model.claim_id));
    inputs.reverse();
    assert_eq!(
        first.admitted_claim().unwrap(),
        reconcile_unmeasured_flowsheet(&inputs, &model)
            .unwrap()
            .admitted_claim()
            .unwrap()
    );
    inputs[0].uu_terms.push(UuTerm {
        scenario_id: "unobserved-path".into(),
        description: "Constructed possible topology omission".into(),
        downside_exposure_usd_m: 1.0,
        trigger_indicator: "Independent stocktake".into(),
    });
    let changed = reconcile_unmeasured_flowsheet(&inputs, &model).unwrap();
    assert_ne!(changed.admitted_claim().unwrap().claim_id, claim.claim_id);
    assert_eq!(changed.admitted_claim().unwrap().uu_terms.len(), 1);
}
