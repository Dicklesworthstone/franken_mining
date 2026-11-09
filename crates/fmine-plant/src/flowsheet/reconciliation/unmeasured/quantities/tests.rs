//! Independent single-window inventory answers, joint covariance and refusals.
//! Determinism: DeterministicPerIsa

use super::*;
use crate::flowsheet::reconciliation::BoundaryRole;
use crate::flowsheet::reconciliation::unmeasured::{
    GrossErrorDiagnostic, UnmeasuredFlowModel, reconcile_unmeasured_flowsheet,
};
use crate::flowsheet::{
    ElementKey, FlowsheetValue, MaterialAmounts, OutflowRole, StreamId, StreamRoute, StreamSpec,
    UnitId, UnitKind, UnitSpec,
};
use fmine_claim::{Confidentiality, LabelRegistry, Representation, SpatialSupport, UuTerm};
use fmine_core::{anchor::SourceAnchor, hash::sha256};
use fmine_evidence::{EpistemicStatus, EvidenceClass, KnowledgeState};
use fmine_time::ValidTime;
use fmine_units::ExactQty;

fn q(value: &str) -> ExactQty {
    ExactQty::from_decimal_str(value).unwrap()
}
fn close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() < 2e-9 * expected.abs().max(1.0),
        "{actual} != {expected}"
    );
}
fn unit(value: &str) -> UnitId {
    UnitId::new(value).unwrap()
}
fn stream(value: &str) -> StreamId {
    StreamId::new(value).unwrap()
}
fn gold() -> ElementKey {
    ElementKey {
        element: "Au".into(),
        measurand: "total-Au/all-phases".into(),
    }
}
fn stock(name: &str, kind: StocktakeKind) -> ObservationLocation {
    ObservationLocation::Stocktake {
        unit: unit(name),
        kind,
    }
}
fn transfer(name: &str) -> ObservationLocation {
    ObservationLocation::Transfer(stream(name))
}
fn amount(dry: &str, water: &str, au: &str) -> MaterialAmounts {
    MaterialAmounts {
        dry_solids_kg: q(dry),
        water_kg: q(water),
        elements_kg: BTreeMap::from([(gold(), q(au))]),
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
            "crates/fmine-plant/src/flowsheet/reconciliation/unmeasured/quantities/tests.rs",
            sha256(include_bytes!("tests.rs")).0,
            "constructed endpoint observations and explicit measured error covariance",
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
fn refresh_model(model: &mut Claim<UnmeasuredQuantityModel>) {
    let tag = model.value.canonical_tag();
    refresh(model, &tag);
}
fn inputs(
    units: &[&str],
    streams: Vec<StreamSpec>,
    rows: Vec<(ObservationLocation, MaterialAmounts)>,
) -> Vec<Claim<FlowsheetInput>> {
    let scope = FlowsheetScope {
        boundary: "constructed-endpoint-inference/v1".into(),
        interval: ValidTime::new(0, 10).unwrap(),
    };
    let declaration = FlowsheetValue::Declaration {
        units: units
            .iter()
            .map(|id| UnitSpec {
                id: unit(id),
                kind: UnitKind::Storage,
            })
            .collect(),
        streams,
        elements: vec![gold()],
        complete: true,
    };
    std::iter::once(declaration)
        .chain(rows.into_iter().map(|(location, amounts)| match location {
            ObservationLocation::Transfer(stream) => FlowsheetValue::Transfer { stream, amounts },
            ObservationLocation::Stocktake { unit, kind } => FlowsheetValue::Stocktake {
                unit,
                kind,
                amounts,
            },
        }))
        .map(|value| {
            let physical_time = match &value {
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
                physical_time,
                value,
            };
            let tag = input.canonical_tag();
            source(crate::flowsheet::INPUT_METRIC, input, &tag, additivity)
        })
        .collect()
}
fn location(input: &Claim<FlowsheetInput>) -> Option<ObservationLocation> {
    match &input.value.value {
        FlowsheetValue::Transfer { stream, .. } => {
            Some(ObservationLocation::Transfer(stream.clone()))
        }
        FlowsheetValue::Stocktake { unit, kind, .. } => Some(ObservationLocation::Stocktake {
            unit: unit.clone(),
            kind: *kind,
        }),
        FlowsheetValue::Declaration { .. } => None,
    }
}
fn omit(inputs: &mut Vec<Claim<FlowsheetInput>>, unknown: &BTreeSet<ObservationLocation>) {
    inputs.retain(|input| location(input).is_none_or(|key| !unknown.contains(&key)));
}
fn single() -> Vec<Claim<FlowsheetInput>> {
    inputs(
        &["store"],
        vec![
            StreamSpec {
                id: stream("feed"),
                route: StreamRoute::Feed { to: unit("store") },
            },
            StreamSpec {
                id: stream("product"),
                route: StreamRoute::Product {
                    from: unit("store"),
                },
            },
        ],
        vec![
            (
                stock("store", StocktakeKind::Opening),
                amount("10", "2", "0.1"),
            ),
            (transfer("feed"), amount("100", "20", "1")),
            (transfer("product"), amount("80", "16", "0.8")),
            (
                stock("store", StocktakeKind::Closing),
                amount("30", "6", "0.3"),
            ),
        ],
    )
}
fn two_units(redundant: bool) -> Vec<Claim<FlowsheetInput>> {
    inputs(
        &["A", "B"],
        vec![
            StreamSpec {
                id: stream("feed"),
                route: StreamRoute::Feed { to: unit("A") },
            },
            StreamSpec {
                id: stream("internal"),
                route: StreamRoute::Internal {
                    from: unit("A"),
                    to: unit("B"),
                    source_role: OutflowRole::Product,
                },
            },
            StreamSpec {
                id: stream("product"),
                route: StreamRoute::Product { from: unit("B") },
            },
        ],
        vec![
            (stock("A", StocktakeKind::Opening), amount("10", "2", "0.1")),
            (transfer("feed"), amount("100", "20", "1")),
            (stock("A", StocktakeKind::Closing), amount("20", "4", "0.2")),
            (stock("B", StocktakeKind::Opening), amount("5", "1", "0.05")),
            (transfer("internal"), amount("80", "16", "0.8")),
            (
                transfer("product"),
                if redundant {
                    amount("70", "14", "0.7")
                } else {
                    amount("80", "16", "0.8")
                },
            ),
            (
                stock("B", StocktakeKind::Closing),
                if redundant {
                    amount("10", "3", "0.15")
                } else {
                    amount("15", "3", "0.15")
                },
            ),
        ],
    )
}
fn model(
    inputs: &[Claim<FlowsheetInput>],
    unknown: BTreeSet<ObservationLocation>,
) -> Claim<UnmeasuredQuantityModel> {
    let observations = observation_order_for_quantities(inputs, &unknown).unwrap();
    let declaration = inputs
        .iter()
        .find(|s| matches!(s.value.value, FlowsheetValue::Declaration { .. }))
        .unwrap();
    let n = observations.len();
    let value = UnmeasuredQuantityModel {
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
        unmeasured: unknown,
        unbiased_gaussian_errors: true,
        global_chi_square_limit: 10.0,
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
fn measured(d: &InferredQuantities, location: &ObservationLocation) -> usize {
    d.observations
        .iter()
        .position(|key| &key.location == location && key.channel == MaterialChannel::DrySolids)
        .unwrap()
}
fn latent(d: &InferredQuantities, location: &ObservationLocation) -> usize {
    d.observations.len()
        + d.unmeasured
            .iter()
            .position(|key| &key.location == location && key.channel == MaterialChannel::DrySolids)
            .unwrap()
}
fn covariance(d: &InferredQuantities, a: usize, b: usize) -> f64 {
    d.posterior_covariance[a * d.estimates_kg.len() + b].value()
}

#[test]
fn opening_and_closing_have_analytic_signed_covariance_without_fake_observations() {
    for (kind, expected, feed_sign, product_sign, boundary_role) in [
        (
            StocktakeKind::Closing,
            30.0,
            1.0,
            -1.0,
            BoundaryRole::ClosingInventory,
        ),
        (
            StocktakeKind::Opening,
            10.0,
            -1.0,
            1.0,
            BoundaryRole::OpeningInventory,
        ),
    ] {
        let unknown = BTreeSet::from([stock("store", kind)]);
        let mut inputs = single();
        omit(&mut inputs, &unknown);
        let mut model = model(&inputs, unknown);
        let n = model.value.observations.len();
        for (i, key) in model.value.observations.iter().enumerate() {
            if key.channel == MaterialChannel::DrySolids {
                model.value.covariance[i * n + i] =
                    KilogramsSquared::new(if key.location == transfer("feed") {
                        4.0
                    } else if key.location == transfer("product") {
                        9.0
                    } else {
                        1.0
                    });
            }
        }
        refresh_model(&mut model);
        let result = reconcile_unmeasured_quantities(&inputs, &model).unwrap();
        let d = &result.admitted_claim().unwrap().value;
        assert_eq!(d.gross_error, GrossErrorDiagnostic::NoRedundancy);
        assert_eq!(d.observations.len(), 9);
        assert_eq!(d.unmeasured.len(), 3);
        assert!(
            d.measured_adjustments_kg
                .iter()
                .all(|v| v.value().abs() < 1e-10)
        );
        let u = latent(d, &stock("store", kind));
        close(d.estimates_kg[u].value(), expected);
        close(covariance(d, u, u), 14.0);
        close(
            covariance(d, u, measured(d, &transfer("feed"))),
            feed_sign * 4.0,
        );
        close(
            covariance(d, u, measured(d, &transfer("product"))),
            product_sign * 9.0,
        );
        let opposite = if kind == StocktakeKind::Closing {
            StocktakeKind::Opening
        } else {
            StocktakeKind::Closing
        };
        close(
            covariance(d, u, measured(d, &stock("store", opposite))),
            1.0,
        );
        let b = d
            .boundary
            .keys
            .iter()
            .position(|key| key.role == boundary_role && key.channel == MaterialChannel::DrySolids)
            .unwrap();
        assert_eq!(d.boundary.observed_totals_kg[b], None);
        assert_eq!(d.boundary.observed_contributions_exact_kg[b], q("0"));
        close(d.boundary.estimates_kg[b].value(), expected);
        close(
            d.boundary.posterior_covariance[b * d.boundary.keys.len() + b].value(),
            14.0,
        );
        assert!(d.conservation.iter().all(|row| row.has_unmeasured_terms));
    }
}

#[test]
fn internal_flow_and_endpoint_are_jointly_identifiable_with_negative_cross_covariance() {
    // T = closing_B + product - opening_B = 90; closing_A = opening_A + feed - T = 20.
    // Independent unit variances give Var(T)=3, Var(closing_A)=5 and Cov=-3.
    let unknown = BTreeSet::from([transfer("internal"), stock("A", StocktakeKind::Closing)]);
    let mut inputs = two_units(false);
    omit(&mut inputs, &unknown);
    let model = model(&inputs, unknown);
    let result = reconcile_unmeasured_quantities(&inputs, &model).unwrap();
    let d = &result.admitted_claim().unwrap().value;
    let t = latent(d, &transfer("internal"));
    let c = latent(d, &stock("A", StocktakeKind::Closing));
    assert_eq!(t, d.observations.len());
    assert_eq!(c, t + 3);
    close(d.estimates_kg[t].value(), 90.0);
    close(d.estimates_kg[c].value(), 20.0);
    close(covariance(d, t, t), 3.0);
    close(covariance(d, c, c), 5.0);
    close(covariance(d, t, c), -3.0);
    close(covariance(d, c, t), -3.0);
    assert_eq!(d.gross_error, GrossErrorDiagnostic::NoRedundancy);
    let closing = d
        .boundary
        .keys
        .iter()
        .position(|key| {
            key.role == BoundaryRole::ClosingInventory && key.channel == MaterialChannel::DrySolids
        })
        .unwrap();
    assert_eq!(d.boundary.observed_totals_kg[closing], None);
    assert_eq!(d.boundary.observed_contributions_exact_kg[closing], q("15"));
    close(d.boundary.estimates_kg[closing].value(), 35.0);
    // closing_A and measured closing_B are anticorrelated; summing diagonal
    // variances alone would incorrectly report 6 instead of 4.
    close(
        d.boundary.posterior_covariance[closing * d.boundary.keys.len() + closing].value(),
        4.0,
    );
    for residual in d.boundary.reconciled_residuals_kg.values() {
        close(residual.value(), 0.0);
    }
}

#[test]
fn remaining_redundancy_adjusts_observations_and_can_reject_the_whole_inference() {
    let unknown = BTreeSet::from([stock("A", StocktakeKind::Closing)]);
    let mut inputs = two_units(true);
    omit(&mut inputs, &unknown);
    let mut model = model(&inputs, unknown);
    let result = reconcile_unmeasured_quantities(&inputs, &model).unwrap();
    let d = &result.admitted_claim().unwrap().value;
    let t = measured(d, &transfer("internal"));
    let c = latent(d, &stock("A", StocktakeKind::Closing));
    close(d.estimates_kg[t].value(), 78.75);
    close(d.estimates_kg[c].value(), 31.25);
    close(covariance(d, c, c), 2.75);
    close(covariance(d, t, c), -0.75);
    let GrossErrorDiagnostic::Available {
        chi_square,
        degrees_of_freedom,
    } = d.gross_error
    else {
        panic!("three remaining channel constraints")
    };
    close(chi_square, 6.25);
    assert_eq!(degrees_of_freedom, 3);
    model.value.global_chi_square_limit = 1.0;
    refresh_model(&mut model);
    let rejected = reconcile_unmeasured_quantities(&inputs, &model).unwrap();
    assert_eq!(rejected.admitted_claim().unwrap_err().code, "FMINE-REF-101");
    assert_eq!(rejected.diagnostics(), d);
    let UnmeasuredQuantityOutcome::Rejected(receipt) = rejected else {
        unreachable!()
    };
    assert_eq!(receipt.inputs().len(), inputs.len());
    assert_eq!(receipt.error_model(), &model);
}

#[test]
fn endpoint_levels_and_parallel_unknown_terms_require_identifiability() {
    let both = BTreeSet::from([
        stock("A", StocktakeKind::Opening),
        stock("A", StocktakeKind::Closing),
    ]);
    let mut inputs = two_units(true);
    omit(&mut inputs, &both);
    let refusal = observation_order_for_quantities(&inputs, &both).unwrap_err();
    assert_eq!(refusal.code, "FMINE-REF-104");
    assert!(refusal.what.contains("both endpoint"));

    // Two units give six conservation rows for six unknown coordinates; the
    // refusal is column dependence, not merely too many unknowns.
    let parallel = BTreeSet::from([transfer("feed"), stock("A", StocktakeKind::Closing)]);
    let mut inputs = two_units(true);
    omit(&mut inputs, &parallel);
    let model = model(&inputs, parallel);
    assert_eq!(
        reconcile_unmeasured_quantities(&inputs, &model)
            .unwrap_err()
            .code,
        "FMINE-REF-104"
    );
}

#[test]
fn omissions_conflicts_support_and_synthetic_covariance_are_refused() {
    let unknown = BTreeSet::from([stock("store", StocktakeKind::Closing)]);
    let mut inputs = single();
    omit(&mut inputs, &unknown);
    let model = model(&inputs, unknown);
    assert_eq!(
        observation_order_for_quantities(&inputs, &BTreeSet::new())
            .unwrap_err()
            .code,
        "FMINE-REF-201"
    );
    assert_eq!(
        reconcile_unmeasured_quantities(&single(), &model)
            .unwrap_err()
            .code,
        "FMINE-REF-105"
    );
    let mut undeclared = model.clone();
    undeclared
        .value
        .unmeasured
        .insert(stock("missing-unit", StocktakeKind::Closing));
    refresh_model(&mut undeclared);
    assert_eq!(
        reconcile_unmeasured_quantities(&inputs, &undeclared)
            .unwrap_err()
            .code,
        "FMINE-REF-105"
    );
    let mut fake_covariance = model.clone();
    fake_covariance
        .value
        .covariance
        .resize(12 * 12, KilogramsSquared::new(1.0));
    refresh_model(&mut fake_covariance);
    assert_eq!(
        reconcile_unmeasured_quantities(&inputs, &fake_covariance)
            .unwrap_err()
            .code,
        "FMINE-REF-105"
    );
    let opening = inputs
        .iter_mut()
        .find(|s| location(s) == Some(stock("store", StocktakeKind::Opening)))
        .unwrap();
    opening.value.physical_time = ValidTime::instant(1);
    let tag = opening.value.canonical_tag();
    refresh(opening, &tag);
    assert_eq!(
        reconcile_unmeasured_quantities(&inputs, &model)
            .unwrap_err()
            .code,
        "FMINE-REF-102"
    );
}

#[test]
fn negative_or_impossible_stock_composition_keeps_diagnostics_without_an_admitted_claim() {
    for (open, feed, product, detail) in [
        (
            amount("1", "2", "0.01"),
            amount("1", "2", "0.01"),
            amount("3", "1", "0.01"),
            "negative",
        ),
        (
            amount("10", "2", "8"),
            amount("10", "2", "8"),
            amount("19", "1", "2"),
            "composition",
        ),
    ] {
        let mut inputs = single();
        let unknown = BTreeSet::from([stock("store", StocktakeKind::Closing)]);
        omit(&mut inputs, &unknown);
        for (where_, amounts) in [
            (stock("store", StocktakeKind::Opening), open),
            (transfer("feed"), feed),
            (transfer("product"), product),
        ] {
            let input = inputs
                .iter_mut()
                .find(|s| location(s) == Some(where_.clone()))
                .unwrap();
            match &mut input.value.value {
                FlowsheetValue::Transfer {
                    amounts: current, ..
                }
                | FlowsheetValue::Stocktake {
                    amounts: current, ..
                } => *current = amounts,
                _ => unreachable!(),
            }
            let tag = input.value.canonical_tag();
            refresh(input, &tag);
        }
        let model = model(&inputs, unknown);
        let result = reconcile_unmeasured_quantities(&inputs, &model).unwrap();
        let refusal = result.admitted_claim().unwrap_err();
        assert_eq!(refusal.code, "FMINE-REF-109");
        assert!(refusal.what.contains(detail), "{refusal:?}");
        let UnmeasuredQuantityOutcome::Rejected(receipt) = result else {
            unreachable!()
        };
        assert_eq!(receipt.inputs().len(), inputs.len());
        assert_eq!(receipt.diagnostics().unmeasured.len(), 3);
    }
}

#[test]
fn quantity_model_binds_typed_identity_observation_order_and_all_source_envelopes() {
    let unknown = BTreeSet::from([stock("store", StocktakeKind::Closing)]);
    let mut inputs = single();
    omit(&mut inputs, &unknown);
    let mut model = model(&inputs, unknown);
    let registry = LabelRegistry::new(["JV-A"], ["W-A"]).unwrap();
    model.confidentiality = Confidentiality::mnpi(registry.wall("W-A").unwrap());
    refresh_model(&mut model);
    inputs[1].uu_terms.push(UuTerm {
        scenario_id: "constructed-omitted-inventory-risk".into(),
        description: "Independent stocktake may identify omitted held material".into(),
        downside_exposure_usd_m: 1.0,
        trigger_indicator: "Independent stocktake".into(),
    });
    let first = reconcile_unmeasured_quantities(&inputs, &model).unwrap();
    let claim = first.admitted_claim().unwrap();
    assert_eq!(claim.metric_key, OUTPUT_METRIC);
    assert_eq!(claim.state, KnowledgeState::Estimated);
    assert_eq!(claim.evidence, EvidenceClass::Assumed);
    assert_eq!(claim.confidentiality, model.confidentiality);
    assert_eq!(claim.uu_terms.len(), 1);
    let parents: BTreeSet<_> = inputs
        .iter()
        .map(|s| s.claim_id)
        .chain([model.claim_id])
        .collect();
    assert_eq!(
        claim.parents.iter().copied().collect::<BTreeSet<_>>(),
        parents
    );
    inputs.reverse();
    assert_eq!(
        first,
        reconcile_unmeasured_quantities(&inputs, &model).unwrap()
    );
    let mut altered_inputs = inputs.clone();
    altered_inputs[0].anchors[0]
        .locator
        .push_str("; independent source location");
    let tag = altered_inputs[0].value.canonical_tag();
    refresh(&mut altered_inputs[0], &tag);
    assert_eq!(
        reconcile_unmeasured_quantities(&altered_inputs, &model)
            .unwrap_err()
            .code,
        "FMINE-REF-102"
    );
    let mut rebound = model.clone();
    rebound.value.observations =
        observation_order_for_quantities(&altered_inputs, &rebound.value.unmeasured).unwrap();
    refresh_model(&mut rebound);
    let changed = reconcile_unmeasured_quantities(&altered_inputs, &rebound).unwrap();
    assert_ne!(claim.claim_id, changed.admitted_claim().unwrap().claim_id);
    let mut typed_change = model.clone();
    typed_change.value.unmeasured = BTreeSet::from([stock("store", StocktakeKind::Opening)]);
    assert_ne!(
        typed_change.value.canonical_tag(),
        model.value.canonical_tag()
    );
    let mut stale_model = model.clone();
    stale_model.value.covariance[0] = KilogramsSquared::new(2.0);
    assert_eq!(
        reconcile_unmeasured_quantities(&inputs, &stale_model)
            .unwrap_err()
            .code,
        "FMINE-REF-507"
    );
    let mut reordered = model.clone();
    reordered.value.observations.swap(0, 1);
    refresh_model(&mut reordered);
    assert_eq!(
        reconcile_unmeasured_quantities(&inputs, &reordered)
            .unwrap_err()
            .code,
        "FMINE-REF-102"
    );
    let mut stale_source = inputs.clone();
    stale_source[0].stale = true;
    assert_eq!(
        reconcile_unmeasured_quantities(&stale_source, &model)
            .unwrap_err()
            .code,
        "FMINE-REF-502"
    );
}

#[test]
fn generalized_transfer_only_path_matches_legacy_numerics_with_distinct_schema() {
    let unknown = BTreeSet::from([transfer("product")]);
    let mut inputs = single();
    omit(&mut inputs, &unknown);
    let model = model(&inputs, unknown);
    let value = UnmeasuredFlowModel {
        scope: model.value.scope.clone(),
        declaration_id: model.value.declaration_id,
        observations: model.value.observations.clone(),
        unmeasured_streams: BTreeSet::from([stream("product")]),
        covariance: model.value.covariance.clone(),
        unbiased_gaussian_errors: true,
        global_chi_square_limit: model.value.global_chi_square_limit,
        normalized_adjustment_limit: None,
        closure_tolerances_kg: model.value.closure_tolerances_kg.clone(),
    };
    let tag = value.canonical_tag();
    let legacy_model = source(
        super::super::ERROR_MODEL_METRIC,
        value,
        &tag,
        AdditivityClass::NonAdditive,
    );
    let old = reconcile_unmeasured_flowsheet(&inputs, &legacy_model).unwrap();
    let new = reconcile_unmeasured_quantities(&inputs, &model).unwrap();
    let a = &old.admitted_claim().unwrap().value;
    let b = &new.admitted_claim().unwrap().value;
    assert_eq!(a.observations, b.observations);
    assert_eq!(a.estimates_kg, b.estimates_kg);
    assert_eq!(a.posterior_covariance, b.posterior_covariance);
    assert_eq!(a.measured_adjustments_kg, b.measured_adjustments_kg);
    assert_eq!(a.normalized_adjustments, b.normalized_adjustments);
    assert_eq!(a.conservation, b.conservation);
    assert_eq!(a.boundary, b.boundary);
    assert_eq!(a.gross_error, b.gross_error);
    assert_eq!(
        old.admitted_claim().unwrap().metric_key,
        super::super::OUTPUT_METRIC
    );
    assert_ne!(
        old.admitted_claim().unwrap().claim_id,
        new.admitted_claim().unwrap().claim_id
    );
}
