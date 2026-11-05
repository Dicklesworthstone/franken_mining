//! Independent rational metal-covariance and aggregate-precision known answers.
//! All observations and error models are constructed, not plant calibration.
//! Determinism: DeterministicPerIsa

use super::*;
use crate::flowsheet::{StreamRoute, StreamSpec, UnitKind, UnitSpec, account_flowsheet};
use fmine_claim::Confidentiality;
use fmine_core::{anchor::SourceAnchor, hash::sha256};
use fmine_evidence::{EpistemicStatus, EvidenceClass};
use fmine_time::ValidTime;

fn q(value: &str) -> ExactQty {
    ExactQty::from_decimal_str(value).unwrap()
}
fn unit(name: &str) -> UnitId {
    UnitId::new(name).unwrap()
}
fn element(name: &str) -> ElementKey {
    ElementKey {
        element: name.into(),
        measurand: format!("total-{name}/all-phases"),
    }
}
fn scope() -> FlowsheetScope {
    FlowsheetScope {
        boundary: "synthetic-joint-oracle/v1".into(),
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
            "crates/fmine-plant/src/flowsheet/reconciliation/joint_oracles.rs",
            sha256(include_bytes!("joint_oracles.rs")).0,
            "constructed analytical observations and declared Gaussian error model",
        )],
    )
}
fn bind(value: FlowsheetValue) -> Claim<FlowsheetInput> {
    let physical_time = match &value {
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
fn bind_model(value: FlowsheetErrorModel) -> Claim<FlowsheetErrorModel> {
    let tag = value.canonical_tag();
    source(
        ERROR_MODEL_METRIC,
        value,
        &tag,
        AdditivityClass::NonAdditive,
    )
}
fn model(inputs: &[Claim<FlowsheetInput>]) -> FlowsheetErrorModel {
    let observations = observation_order(inputs).unwrap();
    let n = observations.len();
    FlowsheetErrorModel {
        scope: scope(),
        declaration_id: inputs[0].claim_id,
        closure_tolerances_kg: observations
            .iter()
            .map(|key| (key.channel.clone(), Kilograms::new(1e-9)))
            .collect(),
        observations,
        covariance: (0..n * n)
            .map(|i| KilogramsSquared::new(if i / n == i % n { 1.0 } else { 0.0 }))
            .collect(),
        unbiased_gaussian_errors: true,
        global_chi_square_limit: 9.0,
        normalized_adjustment_limit: None,
    }
}
fn close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() < 2e-10 * expected.abs().max(1.0),
        "{actual} != {expected}"
    );
}

#[test]
fn exact_binary_comparison_preserves_signed_rejected_estimates() {
    assert_eq!(represented_rational(-1.25).unwrap(), q("-1.25"));
    assert_eq!(represented_rational(1.25).unwrap(), q("1.25"));
    assert_eq!(represented_rational(-0.0).unwrap(), q("0"));
    assert_eq!(represented_rational(-8.0).unwrap(), q("-8"));
    assert_eq!(
        represented_rational(-1.25)
            .unwrap()
            .checked_add(represented_rational(2.0).unwrap())
            .unwrap(),
        q("0.75")
    );
}

#[test]
fn two_metal_posterior_matches_an_independent_rational_covariance_oracle() {
    let amounts = |gold| MaterialAmounts {
        dry_solids_kg: q("100"),
        water_kg: q("20"),
        elements_kg: BTreeMap::from([(element("Au"), q(gold)), (element("Cu"), q("20"))]),
    };
    let inputs = vec![
        bind(FlowsheetValue::Declaration {
            units: vec![UnitSpec {
                id: unit("store"),
                kind: UnitKind::Storage,
            }],
            streams: vec![],
            elements: vec![element("Au"), element("Cu")],
            complete: true,
        }),
        bind(FlowsheetValue::Stocktake {
            unit: unit("store"),
            kind: StocktakeKind::Opening,
            amounts: amounts("10"),
        }),
        bind(FlowsheetValue::Stocktake {
            unit: unit("store"),
            kind: StocktakeKind::Closing,
            amounts: amounts("11"),
        }),
    ];
    let mut error_model = model(&inputs);
    let n = error_model.observations.len();
    let index = |kind, symbol| {
        error_model
            .observations
            .iter()
            .position(|key| {
                key.location
                    == ObservationLocation::Stocktake {
                        unit: unit("store"),
                        kind,
                    }
                    && key.channel == MaterialChannel::Element(element(symbol))
            })
            .unwrap()
    };
    let oa = index(StocktakeKind::Opening, "Au");
    let oc = index(StocktakeKind::Opening, "Cu");
    let ca = index(StocktakeKind::Closing, "Au");
    let cc = index(StocktakeKind::Closing, "Cu");
    error_model.covariance[oa * n + oc] = KilogramsSquared::new(0.5);
    error_model.covariance[oc * n + oa] = KilogramsSquared::new(0.5);
    let outcome = reconcile_flowsheet(&inputs, &bind_model(error_model)).unwrap();
    let result = &outcome.admitted_claim().unwrap().value;
    // The Au/Cu residual covariance is [[2,1/2],[1/2,2]]; its independently
    // calculated inverse is [[8/15,-2/15],[-2/15,8/15]], with residual [-1,0].
    for i in [oa, ca] {
        close(result.estimates_kg[i].value(), 157.0 / 15.0);
    }
    for i in [oc, cc] {
        close(result.estimates_kg[i].value(), 302.0 / 15.0);
    }
    close(result.chi_square, 8.0 / 15.0);
    close(result.posterior_covariance[oa * n + oa].value(), 7.0 / 15.0);
    close(result.posterior_covariance[oa * n + oc].value(), 2.0 / 15.0);
    close(result.posterior_covariance[ca * n + cc].value(), 2.0 / 15.0);
    assert_eq!(result.degrees_of_freedom, 4);
    assert_eq!(result.conservation[3].raw_residual_exact_kg, q("0"));
    assert!(result.adjustments_kg[oc].value() > 0.0);
    assert_eq!(
        result.boundary.conservation[2].raw_residual_exact_kg,
        q("-1")
    );
    for residual in &result.boundary.conservation {
        close(residual.reconciled_residual_kg.value(), 0.0);
    }
}

fn closed_two_store_boundary(large_stock: &str) -> Vec<Claim<FlowsheetInput>> {
    let amounts = |dry| MaterialAmounts {
        dry_solids_kg: q(dry),
        water_kg: q("0"),
        elements_kg: BTreeMap::new(),
    };
    let mut inputs = vec![bind(FlowsheetValue::Declaration {
        units: vec![
            UnitSpec {
                id: unit("a"),
                kind: UnitKind::Storage,
            },
            UnitSpec {
                id: unit("b"),
                kind: UnitKind::Storage,
            },
        ],
        streams: vec![StreamSpec {
            id: StreamId::new("product").unwrap(),
            route: StreamRoute::Product { from: unit("b") },
        }],
        elements: vec![],
        complete: true,
    })];
    inputs.push(bind(FlowsheetValue::Transfer {
        stream: StreamId::new("product").unwrap(),
        amounts: amounts("1"),
    }));
    for (name, kind, dry) in [
        ("a", StocktakeKind::Opening, large_stock),
        ("a", StocktakeKind::Closing, large_stock),
        ("b", StocktakeKind::Opening, "1"),
        ("b", StocktakeKind::Closing, "0"),
    ] {
        inputs.push(bind(FlowsheetValue::Stocktake {
            unit: unit(name),
            kind,
            amounts: amounts(dry),
        }));
    }
    inputs
}

#[test]
fn whole_boundary_projection_cannot_lose_a_kilogram_that_unit_balances_retain() {
    let inputs = closed_two_store_boundary("10000000000000000");
    let exact = account_flowsheet(&inputs).unwrap();
    assert_eq!(
        exact.summary().boundary.dry_solids.opening_kg,
        q("10000000000000001")
    );
    assert_eq!(exact.summary().boundary.dry_solids.residual_kg, q("0"));
    // Every original coordinate is exactly representable and every unit closes.
    // The displayed opening-stock sum rounds 10^16 + 1 down to 10^16, while the
    // 1 kg product remains. The independent boundary audit must catch that loss.
    let refusal = reconcile_flowsheet(&inputs, &bind_model(model(&inputs))).unwrap_err();
    assert_eq!(refusal.code, "FMINE-REF-207");
    assert!(refusal.what.contains("original exact boundary total"));

    let inputs = closed_two_store_boundary("1000000");
    let outcome = reconcile_flowsheet(&inputs, &bind_model(model(&inputs))).unwrap();
    let result = &outcome.admitted_claim().unwrap().value;
    close(result.chi_square, 0.0);
    assert_eq!(result.degrees_of_freedom, 4);
    close(
        result.boundary.conservation[0]
            .reconciled_residual_kg
            .value(),
        0.0,
    );
    assert_eq!(
        result.boundary.conservation[0].raw_residual_exact_kg,
        q("0")
    );
}

#[test]
fn matching_boundary_total_losses_cannot_cancel_between_opening_and_closing() {
    let inputs = |large_stock, small_opening, small_closing| {
        let mut inputs = vec![bind(FlowsheetValue::Declaration {
            units: vec![
                UnitSpec {
                    id: unit("a"),
                    kind: UnitKind::Storage,
                },
                UnitSpec {
                    id: unit("b"),
                    kind: UnitKind::Storage,
                },
            ],
            streams: vec![],
            elements: vec![],
            complete: true,
        })];
        for (name, opening, closing) in [
            ("a", large_stock, large_stock),
            ("b", small_opening, small_closing),
        ] {
            for (kind, dry) in [
                (StocktakeKind::Opening, opening),
                (StocktakeKind::Closing, closing),
            ] {
                inputs.push(bind(FlowsheetValue::Stocktake {
                    unit: unit(name),
                    kind,
                    amounts: MaterialAmounts {
                        dry_solids_kg: q(dry),
                        water_kg: q("0"),
                        elements_kg: BTreeMap::new(),
                    },
                }));
            }
        }
        inputs
    };
    let large = inputs("10000000000000000", "1", "1");
    let exact = account_flowsheet(&large).unwrap();
    let boundary = &exact.summary().boundary.dry_solids;
    assert_eq!(boundary.opening_kg, q("10000000000000001"));
    assert_eq!(boundary.closing_kg, q("10000000000000001"));
    assert_eq!(boundary.residual_kg, q("0"));
    // Both displayed totals round down by 1 kg. Their residual is still zero,
    // so only an independent exact comparison of each total detects the loss.
    let refusal = reconcile_flowsheet(&large, &bind_model(model(&large))).unwrap_err();
    assert_eq!(refusal.code, "FMINE-REF-207");
    assert!(refusal.what.contains("original exact boundary total"));

    // These original totals, 10^16 + 2 and 10^16 + 4, are representable.
    // Equal stocktake variances fit the small store to 3 kg at both times.
    // Each fitted aggregate rounds upward by 1 kg, although the errors cancel.
    let fitted_loss = inputs("10000000000000000", "2", "4");
    let refusal = reconcile_flowsheet(&fitted_loss, &bind_model(model(&fitted_loss))).unwrap_err();
    assert_eq!(refusal.code, "FMINE-REF-207");
    assert!(refusal.what.contains("represented fitted boundary total"));

    let small = inputs("1000000", "1", "1");
    let outcome = reconcile_flowsheet(&small, &bind_model(model(&small))).unwrap();
    let result = &outcome.admitted_claim().unwrap().value;
    close(result.chi_square, 0.0);
    for (i, key) in result.boundary.keys.iter().enumerate() {
        if key.channel == MaterialChannel::DrySolids
            && matches!(
                key.role,
                BoundaryRole::OpeningInventory | BoundaryRole::ClosingInventory
            )
        {
            close(result.boundary.observed_kg[i].value(), 1000001.0);
            close(result.boundary.estimates_kg[i].value(), 1000001.0);
        }
    }
    close(
        result.boundary.conservation[0]
            .reconciled_residual_kg
            .value(),
        0.0,
    );
}
