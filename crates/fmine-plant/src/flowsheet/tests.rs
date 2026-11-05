//! Constructed mill/classifier circulation and inventories, with source/closure regressions.
//! Determinism: Exact

use super::*;
use fmine_claim::{LabelRegistry, UuTerm};
use fmine_core::{anchor::SourceAnchor, hash::sha256};
use fmine_evidence::AssertionSource;

fn q(value: &str) -> ExactQty {
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
fn copper() -> ElementKey {
    ElementKey {
        element: "Cu".into(),
        measurand: "total-Cu/all-phases".into(),
    }
}
fn amounts(dry: &str, water: &str, au: &str) -> MaterialAmounts {
    MaterialAmounts {
        dry_solids_kg: q(dry),
        water_kg: q(water),
        elements_kg: BTreeMap::from([(gold(), q(au))]),
    }
}
fn scope() -> FlowsheetScope {
    FlowsheetScope {
        boundary: "synthetic-grinding-circuit/v1".into(),
        interval: ValidTime::new(0, 3_600_000_000_000).unwrap(),
    }
}

fn bind(value: FlowsheetValue) -> Claim<FlowsheetInput> {
    let scope = scope();
    let physical_time = match &value {
        FlowsheetValue::Stocktake {
            kind: StocktakeKind::Opening,
            ..
        } => ValidTime::instant(scope.interval.start_ns),
        FlowsheetValue::Stocktake {
            kind: StocktakeKind::Closing,
            ..
        } => ValidTime::instant(scope.interval.end_ns),
        _ => scope.interval,
    };
    let additivity = value.additivity();
    let locator = format!("constructed mill/classifier fixture: {:?}", value.key());
    let input = FlowsheetInput {
        scope,
        physical_time,
        value,
    };
    let tag = input.canonical_tag();
    Claim::new(
        INPUT_METRIC,
        input,
        &tag,
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
            "crates/fmine-plant/src/flowsheet/tests.rs",
            sha256(include_bytes!("tests.rs")).0,
            locator,
        )],
    )
}

fn rebind(source: &mut Claim<FlowsheetInput>) {
    *source = Claim::new(
        source.metric_key.clone(),
        source.value.clone(),
        &source.value.canonical_tag(),
        source.representation.clone(),
        source.support,
        source.additivity,
        source.evidence,
        source.epistemic,
        source.state,
        source.confidentiality.clone(),
        source.uu_terms.clone(),
        source.parents.clone(),
        source.anchors.clone(),
    );
}

fn fixture() -> Vec<Claim<FlowsheetInput>> {
    // Dry solids: mill 20+100+60=150+30; classifier 10+150=60+80+5+15.
    // Water: mill 40+200+120=300+60; classifier 20+300=120+150+20+30.
    // Au: mill .2+1+.6=1.5+.3; classifier .1+1.5=.6+.8+.05+.15.
    let declaration = FlowsheetValue::Declaration {
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
        streams: vec![
            StreamSpec {
                id: stream("feed"),
                route: StreamRoute::Feed { to: unit("mill") },
            },
            StreamSpec {
                id: stream("mill-discharge"),
                route: StreamRoute::Internal {
                    from: unit("mill"),
                    to: unit("classifier"),
                    source_role: OutflowRole::Product,
                },
            },
            StreamSpec {
                id: stream("recycle"),
                route: StreamRoute::Internal {
                    from: unit("classifier"),
                    to: unit("mill"),
                    source_role: OutflowRole::Other,
                },
            },
            StreamSpec {
                id: stream("product"),
                route: StreamRoute::Product {
                    from: unit("classifier"),
                },
            },
            StreamSpec {
                id: stream("tails"),
                route: StreamRoute::OtherOutflow {
                    from: unit("classifier"),
                },
            },
        ],
        elements: vec![gold()],
        complete: true,
    };
    vec![
        declaration,
        FlowsheetValue::Transfer {
            stream: stream("feed"),
            amounts: amounts("100", "200", "1"),
        },
        FlowsheetValue::Transfer {
            stream: stream("mill-discharge"),
            amounts: amounts("150", "300", "1.5"),
        },
        FlowsheetValue::Transfer {
            stream: stream("recycle"),
            amounts: amounts("60", "120", "0.6"),
        },
        FlowsheetValue::Transfer {
            stream: stream("product"),
            amounts: amounts("80", "150", "0.8"),
        },
        FlowsheetValue::Transfer {
            stream: stream("tails"),
            amounts: amounts("5", "20", "0.05"),
        },
        FlowsheetValue::Stocktake {
            unit: unit("mill"),
            kind: StocktakeKind::Opening,
            amounts: amounts("20", "40", "0.2"),
        },
        FlowsheetValue::Stocktake {
            unit: unit("mill"),
            kind: StocktakeKind::Closing,
            amounts: amounts("30", "60", "0.3"),
        },
        FlowsheetValue::Stocktake {
            unit: unit("classifier"),
            kind: StocktakeKind::Opening,
            amounts: amounts("10", "20", "0.1"),
        },
        FlowsheetValue::Stocktake {
            unit: unit("classifier"),
            kind: StocktakeKind::Closing,
            amounts: amounts("15", "30", "0.15"),
        },
    ]
    .into_iter()
    .map(bind)
    .collect()
}

fn failure(inputs: &[Claim<FlowsheetInput>], code: RefusalCode) -> Refusal {
    let refusal = account_flowsheet(inputs).expect_err("no closed flowsheet receipt");
    assert_eq!(refusal.code, code.as_str(), "{}", refusal.what);
    refusal
}

#[test]
fn two_unit_recycle_closes_each_carrier_and_element_without_double_counting() {
    let inputs = fixture();
    let receipt = account_flowsheet(&inputs).unwrap();
    let result = receipt.summary();
    let mill = &result.units[&unit("mill")];
    let classifier = &result.units[&unit("classifier")];
    assert_eq!(mill.dry_solids.feed_kg, q("160"));
    assert_eq!(mill.dry_solids.product_kg, q("150"));
    assert_eq!(mill.dry_solids.inventory_change_kg, q("10"));
    assert_eq!(classifier.dry_solids.feed_kg, q("150"));
    assert_eq!(classifier.dry_solids.other_outflow_kg, q("65"));
    assert_eq!(classifier.dry_solids.inventory_change_kg, q("5"));
    assert_eq!(result.boundary.dry_solids.feed_kg, q("100"));
    assert_eq!(result.boundary.dry_solids.product_kg, q("80"));
    assert_eq!(result.boundary.dry_solids.other_outflow_kg, q("5"));
    assert_eq!(result.boundary.dry_solids.opening_kg, q("30"));
    assert_eq!(result.boundary.dry_solids.closing_kg, q("45"));
    assert_eq!(result.boundary.water.feed_kg, q("200"));
    assert_eq!(result.boundary.water.product_kg, q("150"));
    assert_eq!(result.boundary.water.other_outflow_kg, q("20"));
    assert_eq!(result.boundary.water.inventory_change_kg, q("30"));
    let au = result.boundary.elements[&gold()].summary();
    assert_eq!(au.feed_kg, q("1"));
    assert_eq!(au.product_kg, q("0.8"));
    assert_eq!(au.other_outflow_kg, q("0.05"));
    assert_eq!(au.inventory_change_kg, q("0.15"));
    assert_eq!(au.residual_kg, ZERO);
    assert_eq!(
        result.internal_streams,
        [stream("mill-discharge"), stream("recycle")]
    );
    for material in result.units.values().chain([&result.boundary]) {
        assert_eq!(material.dry_solids.residual_kg, ZERO);
        assert_eq!(material.water.residual_kg, ZERO);
        material.elements[&gold()].claim().check_id().unwrap();
    }
    assert_eq!(receipt.claim().parents.len(), inputs.len());
    assert_eq!(receipt.claim().anchors.len(), inputs.len());
    assert_eq!(receipt.claim().value_tag(), receipt.digest());
    receipt.claim().check_id().unwrap();
}

#[test]
fn each_recycle_observation_is_the_same_parent_at_both_unit_endpoints() {
    let inputs = fixture();
    let recycle_id = inputs[3].claim_id;
    let receipt = account_flowsheet(&inputs).unwrap();
    let incoming = &receipt.summary().units[&unit("mill")].elements[&gold()].inputs()[2];
    let outgoing = &receipt.summary().units[&unit("classifier")].elements[&gold()].inputs()[4];
    assert!(incoming.parents.contains(&recycle_id));
    assert!(outgoing.parents.contains(&recycle_id));
    let outer = &receipt.summary().boundary.elements[&gold()];
    assert!(!outer.inputs()[2].parents.contains(&recycle_id));
    assert!(!outer.inputs()[4].parents.contains(&recycle_id));
}

#[test]
fn changing_internal_circulation_preserves_only_the_outer_totals() {
    let baseline = account_flowsheet(&fixture()).unwrap();
    let mut inputs = fixture();
    for (index, value) in [
        (2, amounts("175", "350", "1.75")),
        (3, amounts("85", "170", "0.85")),
    ] {
        if let FlowsheetValue::Transfer { amounts, .. } = &mut inputs[index].value.value {
            *amounts = value;
        }
        rebind(&mut inputs[index]);
    }
    let changed = account_flowsheet(&inputs).unwrap();
    assert_eq!(changed.summary().boundary, baseline.summary().boundary);
    assert_eq!(
        changed.summary().units[&unit("mill")].dry_solids.feed_kg,
        q("185")
    );
    assert_ne!(changed.digest(), baseline.digest());
}

#[test]
fn compensating_unit_errors_refuse_even_when_external_boundary_is_unchanged() {
    for component in 0..3 {
        let mut inputs = fixture();
        if let FlowsheetValue::Transfer { amounts, .. } = &mut inputs[2].value.value {
            match component {
                0 => amounts.dry_solids_kg = q("151"),
                1 => amounts.water_kg = q("301"),
                _ => *amounts.elements_kg.get_mut(&gold()).unwrap() = q("1.51"),
            }
        }
        rebind(&mut inputs[2]);
        let refusal = failure(&inputs, RefusalCode::Conservation101);
        assert!(refusal.what.contains("unit/classifier"), "{}", refusal.what);
    }
}

#[test]
fn every_declared_transfer_and_endpoint_stock_is_required_even_if_zero() {
    for missing in 0..fixture().len() {
        let mut inputs = fixture();
        inputs.remove(missing);
        failure(&inputs, RefusalCode::NoClaimInDomain201);
    }
    let mut inputs = fixture();
    if let FlowsheetValue::Declaration { complete, .. } = &mut inputs[0].value.value {
        *complete = false;
    }
    rebind(&mut inputs[0]);
    failure(&inputs, RefusalCode::NoClaimInDomain201);
}

#[test]
fn duplicate_observations_and_declared_ids_refuse() {
    for repeat in 0..fixture().len() {
        let mut inputs = fixture();
        inputs.push(inputs[repeat].clone());
        failure(&inputs, RefusalCode::BasisMismatch105);
    }
    for kind in 0..3 {
        let mut inputs = fixture();
        if let FlowsheetValue::Declaration {
            units,
            streams,
            elements,
            ..
        } = &mut inputs[0].value.value
        {
            match kind {
                0 => units.push(units[0].clone()),
                1 => streams.push(streams[0].clone()),
                _ => elements.push(elements[0].clone()),
            }
        }
        rebind(&mut inputs[0]);
        failure(&inputs, RefusalCode::BasisMismatch105);
    }
}

#[test]
fn unknown_endpoints_self_loops_and_undeclared_observations_refuse() {
    for self_loop in [false, true] {
        let mut inputs = fixture();
        if let FlowsheetValue::Declaration { streams, .. } = &mut inputs[0].value.value {
            streams[1].route = StreamRoute::Internal {
                from: unit("mill"),
                to: unit(if self_loop { "mill" } else { "undeclared" }),
                source_role: OutflowRole::Product,
            };
        }
        rebind(&mut inputs[0]);
        failure(&inputs, RefusalCode::BasisMismatch105);
    }
    let mut inputs = fixture();
    if let FlowsheetValue::Transfer { stream: id, .. } = &mut inputs[1].value.value {
        *id = stream("undeclared");
    }
    rebind(&mut inputs[1]);
    failure(&inputs, RefusalCode::NoClaimInDomain201);
    let mut inputs = fixture();
    if let FlowsheetValue::Stocktake { unit: id, .. } = &mut inputs[6].value.value {
        *id = unit("undeclared");
    }
    rebind(&mut inputs[6]);
    failure(&inputs, RefusalCode::NoClaimInDomain201);
}

#[test]
fn boundary_period_and_stocktake_support_are_not_interchangeable() {
    for defect in 0..5 {
        let mut inputs = fixture();
        match defect {
            0 => inputs[1].value.scope.boundary = "another-plant/v1".into(),
            1 => {
                inputs[1].value.scope.interval.end_ns += 1;
                inputs[1].value.physical_time.end_ns += 1;
            }
            2 => inputs[6].value.physical_time = inputs[6].value.scope.interval,
            3 => inputs[1].support = SpatialSupport::PlantLot,
            _ => inputs[1].value.physical_time.start_ns += 1,
        }
        rebind(&mut inputs[if defect == 2 { 6 } else { 1 }]);
        failure(&inputs, RefusalCode::SupportMismatch102);
    }
}

#[test]
fn elemental_schema_requires_explicit_zeros_and_one_measurand_per_element() {
    let mut inputs = fixture();
    if let FlowsheetValue::Declaration { elements, .. } = &mut inputs[0].value.value {
        elements.push(copper());
    }
    for source in &mut inputs {
        match &mut source.value.value {
            FlowsheetValue::Transfer { amounts, .. }
            | FlowsheetValue::Stocktake { amounts, .. } => {
                amounts.elements_kg.insert(copper(), ZERO);
            }
            _ => {}
        }
        rebind(source);
    }
    assert_eq!(
        account_flowsheet(&inputs)
            .unwrap()
            .summary()
            .boundary
            .elements
            .len(),
        2
    );
    if let FlowsheetValue::Transfer { amounts, .. } = &mut inputs[1].value.value {
        amounts.elements_kg.remove(&copper());
    }
    rebind(&mut inputs[1]);
    failure(&inputs, RefusalCode::BasisMismatch105);
    let mut inputs = fixture();
    if let FlowsheetValue::Declaration { elements, .. } = &mut inputs[0].value.value {
        elements.push(ElementKey {
            element: "Au".into(),
            measurand: "dissolved-Au".into(),
        });
    }
    rebind(&mut inputs[0]);
    failure(&inputs, RefusalCode::BasisMismatch105);
}

#[test]
fn negative_or_impossible_component_masses_and_overflow_refuse() {
    for amounts in [
        amounts("-1", "200", "1"),
        amounts("100", "-1", "1"),
        amounts("100", "200", "-1"),
        amounts("0", "0", "1"),
    ] {
        let mut inputs = fixture();
        inputs[1].value.value = FlowsheetValue::Transfer {
            stream: stream("feed"),
            amounts,
        };
        rebind(&mut inputs[1]);
        failure(&inputs, RefusalCode::OutsidePhysicalDomain109);
    }
    let mut inputs = fixture();
    if let FlowsheetValue::Transfer { amounts, .. } = &mut inputs[1].value.value {
        amounts.dry_solids_kg = ExactQty::from_integer(i128::MAX);
    }
    rebind(&mut inputs[1]);
    failure(&inputs, RefusalCode::NumericalCertificateFailed207);
}

#[test]
fn metals_cannot_hide_in_water_even_when_every_inventory_balance_closes() {
    for (dry, au, cu, valid) in [
        ("0", "1", "0", false),
        ("1", "0.75", "0.75", false),
        ("2", "0.75", "0.75", true),
    ] {
        let mut stock = amounts(dry, "100", au);
        stock.elements_kg.insert(copper(), q(cu));
        let inputs: Vec<_> = [
            FlowsheetValue::Declaration {
                units: vec![UnitSpec {
                    id: unit("storage"),
                    kind: UnitKind::Storage,
                }],
                streams: vec![],
                elements: vec![gold(), copper()],
                complete: true,
            },
            FlowsheetValue::Stocktake {
                unit: unit("storage"),
                kind: StocktakeKind::Opening,
                amounts: stock.clone(),
            },
            FlowsheetValue::Stocktake {
                unit: unit("storage"),
                kind: StocktakeKind::Closing,
                amounts: stock,
            },
        ]
        .into_iter()
        .map(bind)
        .collect();
        if valid {
            assert_eq!(
                account_flowsheet(&inputs)
                    .unwrap()
                    .summary()
                    .boundary
                    .dry_solids
                    .residual_kg,
                ZERO
            );
        } else {
            let refusal = failure(&inputs, RefusalCode::OutsidePhysicalDomain109);
            assert!(
                refusal.what.contains("non-water elements"),
                "{}",
                refusal.what
            );
        }
    }
}

#[test]
fn source_values_and_topology_cannot_mutate_under_unchanged_generic_claim_ids() {
    let mut inputs = fixture();
    if let FlowsheetValue::Transfer { amounts, .. } = &mut inputs[1].value.value {
        amounts.dry_solids_kg = q("101");
    }
    inputs[1].check_id().unwrap();
    failure(&inputs, RefusalCode::FrozenRecordMutation507);
    let mut inputs = fixture();
    if let FlowsheetValue::Declaration { streams, .. } = &mut inputs[0].value.value {
        streams[0].route = StreamRoute::Feed {
            to: unit("classifier"),
        };
    }
    inputs[0].check_id().unwrap();
    failure(&inputs, RefusalCode::FrozenRecordMutation507);
}

#[test]
fn original_source_admission_precedes_aggregation() {
    for defect in 0..6 {
        let mut inputs = fixture();
        let code = match defect {
            0 => {
                inputs[3].anchors.clear();
                RefusalCode::NoClaimInDomain201
            }
            1 => {
                inputs[3].state = KnowledgeState::Unknown;
                RefusalCode::NotObservable202
            }
            2 => {
                inputs[3].representation = Representation::Interval { lo: 0.0, hi: 1.0 };
                RefusalCode::NotObservable202
            }
            3 => {
                inputs[3].additivity = AdditivityClass::NonAdditive;
                RefusalCode::NonAdditiveAverage103
            }
            4 => RefusalCode::StaleAnchor502,
            _ => RefusalCode::CalibrationRequired205,
        };
        rebind(&mut inputs[3]);
        if defect == 4 {
            inputs[3].stale = true;
        }
        if defect == 5 {
            inputs[3].calibration_failed = true;
        }
        failure(&inputs, code);
    }
}

#[test]
fn internal_source_evidence_and_confidentiality_cannot_disappear_by_cancellation() {
    let mut inputs = fixture();
    for source in &mut inputs {
        source.evidence = EvidenceClass::Measured;
        rebind(source);
    }
    let labels = LabelRegistry::new([], ["W-CIRCUIT"]).unwrap();
    inputs[3].evidence = EvidenceClass::Assertion {
        source: AssertionSource::Extraction,
    };
    inputs[3].confidentiality = Confidentiality::mnpi(labels.wall("W-CIRCUIT").unwrap());
    inputs[3].state = KnowledgeState::Estimated;
    rebind(&mut inputs[3]);
    let receipt = account_flowsheet(&inputs).unwrap();
    assert_eq!(receipt.claim().evidence, inputs[3].evidence);
    assert_eq!(receipt.claim().state, KnowledgeState::Estimated);
    assert_eq!(receipt.claim().confidentiality, inputs[3].confidentiality);
    assert!(receipt.claim().color().is_err());
    assert_eq!(
        receipt
            .claim()
            .verify_export_clearance(&Confidentiality::public())
            .unwrap_err()
            .code,
        "FMINE-REF-301"
    );
    assert_eq!(
        receipt.summary().units[&unit("mill")].elements[&gold()]
            .claim()
            .evidence,
        inputs[3].evidence
    );
}

fn uu() -> UuTerm {
    UuTerm {
        scenario_id: "unmeasured-leak".into(),
        description: "Constructed omitted source path".into(),
        downside_exposure_usd_m: 1.0,
        trigger_indicator: "Stocktake disagreement".into(),
    }
}

#[test]
fn source_envelope_changes_bind_the_graph_and_conflicting_uu_refuses() {
    let mut inputs = fixture();
    let baseline = account_flowsheet(&inputs).unwrap();
    let id = inputs[3].claim_id;
    inputs[3].uu_terms.push(uu());
    assert_eq!(inputs[3].claim_id, id);
    let changed = account_flowsheet(&inputs).unwrap();
    assert_ne!(changed.digest(), baseline.digest());
    assert_eq!(changed.claim().uu_terms, vec![uu()]);
    inputs[1].uu_terms.push(uu());
    inputs[1].uu_terms[0].description = "Conflicting definition".into();
    failure(&inputs, RefusalCode::UnbudgetedUnknowns204);
}

#[test]
fn declaration_and_observation_permutations_preserve_canonical_result() {
    let mut inputs = fixture();
    let baseline = account_flowsheet(&inputs).unwrap();
    for _ in 0..inputs.len() {
        inputs.rotate_left(1);
        assert_eq!(account_flowsheet(&inputs).unwrap(), baseline);
        inputs.reverse();
        assert_eq!(account_flowsheet(&inputs).unwrap(), baseline);
        inputs.reverse();
    }
    let mut inputs = fixture();
    let id = inputs[0].claim_id;
    if let FlowsheetValue::Declaration {
        units,
        streams,
        elements,
        ..
    } = &mut inputs[0].value.value
    {
        units.reverse();
        streams.reverse();
        elements.reverse();
    }
    rebind(&mut inputs[0]);
    assert_eq!(inputs[0].claim_id, id);
    let changed = account_flowsheet(&inputs).unwrap();
    assert_eq!(changed.digest(), baseline.digest());
    assert_eq!(changed.summary(), baseline.summary());
}

#[test]
fn declared_isolated_storage_has_structural_zero_flows_with_stocktake_authority() {
    let inputs: Vec<_> = [
        FlowsheetValue::Declaration {
            units: vec![UnitSpec {
                id: unit("storage"),
                kind: UnitKind::Storage,
            }],
            streams: vec![],
            elements: vec![gold()],
            complete: true,
        },
        FlowsheetValue::Stocktake {
            unit: unit("storage"),
            kind: StocktakeKind::Opening,
            amounts: amounts("10", "20", "0.1"),
        },
        FlowsheetValue::Stocktake {
            unit: unit("storage"),
            kind: StocktakeKind::Closing,
            amounts: amounts("10", "20", "0.1"),
        },
    ]
    .into_iter()
    .map(bind)
    .collect();
    let receipt = account_flowsheet(&inputs).unwrap();
    let outer = &receipt.summary().boundary;
    assert_eq!(outer.dry_solids.feed_kg, ZERO);
    assert_eq!(outer.dry_solids.inventory_change_kg, ZERO);
    assert_eq!(
        outer.elements[&gold()].summary().product_to_feed_ratio,
        None
    );
    let zero_feed = &outer.elements[&gold()].inputs()[2];
    assert_eq!(zero_feed.parents, [inputs[0].claim_id]);
    assert!(!zero_feed.anchors.is_empty());
}

#[test]
fn unit_and_stream_ids_are_distinct_and_require_valid_names() {
    for bad in ["", "unit / name", " unit", "a\n"] {
        assert!(UnitId::new(bad).is_err());
        assert!(StreamId::new(bad).is_err());
    }
    assert_eq!(
        UnitId::new("ball-mill_1.v2").unwrap().as_str(),
        "ball-mill_1.v2"
    );
}
