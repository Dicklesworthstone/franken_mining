//! Known-answer synthetic inventories and admission regressions; no site calibration claim.
//! Determinism: Exact

use super::*;
use fmine_claim::{LabelRegistry, UuTerm};
use fmine_core::anchor::SourceAnchor;
use fmine_core::hash::sha256;
use fmine_evidence::{AoaStatus, AssertionSource, KuKind, color::ValidityDomain};

fn q(value: &str) -> ExactQty {
    ExactQty::from_decimal_str(value).expect("synthetic exact quantity")
}

fn rebind(input: &mut Claim<MetalBalanceInput>) {
    *input = Claim::new(
        input.metric_key.clone(),
        input.value.clone(),
        &input.value.canonical_tag(),
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

fn interval(masses: [&str; 5]) -> Vec<Claim<MetalBalanceInput>> {
    let scope = BalanceScope {
        boundary: "synthetic-cil-boundary/v1".into(),
        interval: ValidTime::new(0, 3_600_000_000_000).unwrap(),
        element: "Au".into(),
        measurand: "total-Au/all-phases".into(),
    };
    let [opening, feed, product, other, closing] = masses.map(q);
    [
        BalanceValue::Population { complete: true },
        BalanceValue::OpeningInventory(opening),
        BalanceValue::Feed(feed),
        BalanceValue::Product(product),
        BalanceValue::OtherOutflow(other),
        BalanceValue::ClosingInventory(closing),
    ]
    .into_iter()
    .map(|value| {
        let input = MetalBalanceInput {
            scope: scope.clone(),
            physical_time: value.expected_time(scope.interval),
            value,
        };
        let tag = input.canonical_tag();
        Claim::new(
            INPUT_METRIC,
            input,
            &tag,
            Representation::Deterministic,
            SpatialSupport::PeriodAggregate,
            value.additivity(),
            EvidenceClass::Assumed,
            EpistemicStatus::KnownKnown,
            KnowledgeState::Known,
            Confidentiality::public(),
            vec![],
            vec![],
            vec![SourceAnchor::new(
                "crates/fmine-plant/src/inventory/tests.rs",
                sha256(include_bytes!("tests.rs")).0,
                format!("synthetic interval role {}", value.key()),
            )],
        )
    })
    .collect()
}

fn closed() -> Vec<Claim<MetalBalanceInput>> {
    // Hand-accounted: 10 + 100 = 72 + 8 + 30; 20 kg remains in circuit.
    interval(["10", "100", "72", "8", "30"])
}

fn refusal(inputs: &[Claim<MetalBalanceInput>], code: RefusalCode) -> Refusal {
    let refusal = account_interval(inputs).expect_err("must not emit a closed receipt");
    assert_eq!(refusal.code, code.as_str());
    refusal
}

#[test]
fn exact_known_answer_retains_physical_inventory_and_source_claims() {
    let inputs = closed();
    let receipt = account_interval(&inputs).unwrap();
    let result = receipt.summary();
    assert_eq!(result.opening_inventory_kg, q("10"));
    assert_eq!(result.feed_kg, q("100"));
    assert_eq!(result.product_kg, q("72"));
    assert_eq!(result.other_outflow_kg, q("8"));
    assert_eq!(result.closing_inventory_kg, q("30"));
    assert_eq!(result.inventory_change_kg, q("20"));
    assert_eq!(result.residual_kg, ZERO);
    assert_eq!(result.product_to_feed_ratio, Some(q("0.72")));
    assert_eq!(receipt.inputs(), inputs);
    assert_eq!(receipt.claim().parents.len(), 6);
    assert_eq!(receipt.claim().anchors.len(), 6);
    assert_eq!(receipt.claim().metric_key, OUTPUT_METRIC);
    assert_eq!(receipt.claim().additivity, AdditivityClass::NonAdditive);
    assert_eq!(receipt.claim().value_tag(), receipt.digest());
    receipt.claim().check_id().unwrap();
}

#[test]
fn pulse_and_drawdown_do_not_clamp_an_apparent_ratio_over_one() {
    // Synthetic pulse leaves 60 kg inside; the next interval releases 50 kg of it.
    let pulse = account_interval(&interval(["0", "100", "30", "10", "60"])).unwrap();
    let flush = account_interval(&interval(["60", "20", "60", "10", "10"])).unwrap();
    assert_eq!(pulse.summary().inventory_change_kg, q("60"));
    assert_eq!(pulse.summary().product_to_feed_ratio, Some(q("0.3")));
    assert_eq!(flush.summary().inventory_change_kg, q("-50"));
    assert_eq!(flush.summary().product_to_feed_ratio, Some(q("3")));
    assert_eq!(
        pulse.summary().closing_inventory_kg,
        flush.summary().opening_inventory_kg
    );
    assert_eq!(flush.summary().residual_kg, ZERO);
}

#[test]
fn zero_feed_shutdown_and_inventory_release_have_no_division_or_recovery_claim() {
    for masses in [
        ["10", "0", "0", "0", "10"],
        ["10", "0", "8", "2", "0"],
        ["0"; 5],
    ] {
        let result = account_interval(&interval(masses)).unwrap();
        assert_eq!(result.summary().product_to_feed_ratio, None);
        assert_eq!(result.summary().residual_kg, ZERO);
    }
}

#[test]
fn exact_decimals_close_and_even_a_small_unbalanced_residual_refuses() {
    let inputs = interval(["0.1", "0.2", "0.25", "0.01", "0.04"]);
    let receipt = account_interval(&inputs).unwrap();
    assert_eq!(receipt.summary().inventory_change_kg, q("-0.06"));
    assert_eq!(receipt.summary().product_to_feed_ratio, Some(q("1.25")));
    for (closing, numerator) in [
        ("0.040000001", "-1/1000000000"),
        ("0.039999999", "1/1000000000"),
    ] {
        let failure = refusal(
            &interval(["0.1", "0.2", "0.25", "0.01", closing]),
            RefusalCode::Conservation101,
        );
        assert!(failure.what.contains(numerator), "{}", failure.what);
    }
}

#[test]
fn complete_population_and_each_explicit_mass_role_are_required() {
    for missing in 0..6 {
        let mut inputs = closed();
        inputs.remove(missing);
        refusal(&inputs, RefusalCode::NoClaimInDomain201);
    }
    let mut inputs = closed();
    inputs.push(inputs[2].clone());
    refusal(&inputs, RefusalCode::NoClaimInDomain201);
    let mut inputs = closed();
    inputs[5] = inputs[1].clone();
    refusal(&inputs, RefusalCode::BasisMismatch105);
    let mut inputs = closed();
    inputs[0].value.value = BalanceValue::Population { complete: false };
    rebind(&mut inputs[0]);
    refusal(&inputs, RefusalCode::NoClaimInDomain201);
}

#[test]
fn boundary_element_measurand_and_interval_cannot_be_mixed() {
    for field in 0..4 {
        let mut inputs = closed();
        let source = &mut inputs[2].value;
        match field {
            0 => source.scope.boundary = "synthetic-cil-boundary/v2".into(),
            1 => source.scope.element = "Cu".into(),
            2 => source.scope.measurand = "dissolved-Au".into(),
            _ => {
                source.scope.interval.end_ns += 1;
                source.physical_time = source.scope.interval;
            }
        }
        rebind(&mut inputs[2]);
        refusal(&inputs, RefusalCode::SupportMismatch102);
    }
}

#[test]
fn stocktakes_are_endpoint_instants_and_flows_cover_the_exact_interval() {
    for role in 0..6 {
        let mut inputs = closed();
        inputs[role].value.physical_time.start_ns += 1;
        rebind(&mut inputs[role]);
        refusal(&inputs, RefusalCode::SupportMismatch102);
    }
    let mut inputs = closed();
    inputs[1].value.physical_time = inputs[1].value.scope.interval;
    rebind(&mut inputs[1]);
    refusal(&inputs, RefusalCode::SupportMismatch102);
    let mut inputs = closed();
    inputs[2].support = SpatialSupport::PlantLot;
    refusal(&inputs, RefusalCode::SupportMismatch102);
    let mut inputs = closed();
    inputs[2].additivity = AdditivityClass::IntensiveWeighted;
    refusal(&inputs, RefusalCode::NonAdditiveAverage103);
}

#[test]
fn typed_value_mutation_is_caught_even_when_generic_claim_id_still_matches() {
    let mut inputs = closed();
    inputs[2].value.value = BalanceValue::Feed(q("101"));
    inputs[2].check_id().unwrap(); // Generic bodies bind the tag, not this typed value.
    refusal(&inputs, RefusalCode::FrozenRecordMutation507);
    let mut inputs = closed();
    inputs[2].value.scope.measurand = "changed-measurand".into();
    inputs[2].check_id().unwrap();
    refusal(&inputs, RefusalCode::FrozenRecordMutation507);
    let mut inputs = closed();
    inputs[2].value.physical_time.end_ns += 1;
    refusal(&inputs, RefusalCode::FrozenRecordMutation507);
    let mut inputs = closed();
    inputs[2].anchors[0].locator = "different observation".into();
    refusal(&inputs, RefusalCode::FrozenRecordMutation507);
    let mut inputs = closed();
    inputs[2].metric_key = "unrelated.metric".into();
    rebind(&mut inputs[2]);
    refusal(&inputs, RefusalCode::FrozenRecordMutation507);
}

#[test]
fn unknown_refused_stale_and_uncertain_sources_cannot_supply_numeric_totals() {
    for state in [
        KnowledgeState::Unknown,
        KnowledgeState::Conflicted,
        KnowledgeState::NotObservable,
        KnowledgeState::Redacted,
        KnowledgeState::Indeterminate,
        KnowledgeState::NotApplicable,
    ] {
        let mut inputs = closed();
        inputs[2].state = state;
        rebind(&mut inputs[2]);
        refusal(&inputs, RefusalCode::NotObservable202);
    }
    let mut inputs = closed();
    inputs[2].stale = true;
    refusal(&inputs, RefusalCode::StaleAnchor502);
    let mut inputs = closed();
    inputs[2].state = KnowledgeState::Stale;
    rebind(&mut inputs[2]);
    refusal(&inputs, RefusalCode::StaleAnchor502);
    let mut inputs = closed();
    inputs[2].calibration_failed = true;
    refusal(&inputs, RefusalCode::CalibrationRequired205);
    let mut inputs = closed();
    inputs[2].representation = Representation::Interval {
        lo: 99.0,
        hi: 101.0,
    };
    refusal(&inputs, RefusalCode::NotObservable202);
}

#[test]
fn source_anchors_and_scope_names_are_not_optional() {
    for defect in 0..4 {
        let mut inputs = closed();
        match defect {
            0 => inputs[2].anchors.clear(),
            1 => inputs[2].anchors[0].artifact_sha256 = [0; 32],
            2 => inputs[2].anchors[0].locator.clear(),
            _ => inputs[2].anchors[0].artifact_path = " source-with-space ".into(),
        }
        rebind(&mut inputs[2]);
        refusal(&inputs, RefusalCode::NoClaimInDomain201);
    }
    for invalid in ["", "au", "Au ", "Au\n", "gold"] {
        let mut inputs = closed();
        inputs[2].value.scope.element = invalid.into();
        rebind(&mut inputs[2]);
        refusal(&inputs, RefusalCode::UndeclaredUnitBasisOrTz107);
    }
    for blank_boundary in [true, false] {
        let mut inputs = closed();
        if blank_boundary {
            inputs[2].value.scope.boundary.clear();
        } else {
            inputs[2].value.scope.measurand.clear();
        }
        rebind(&mut inputs[2]);
        refusal(&inputs, RefusalCode::UndeclaredUnitBasisOrTz107);
    }
}

#[test]
fn negative_physical_totals_bad_intervals_and_exact_overflow_refuse() {
    for role in 0..5 {
        let mut masses = ["10", "100", "72", "8", "30"];
        masses[role] = "-1";
        refusal(&interval(masses), RefusalCode::OutsidePhysicalDomain109);
    }
    let mut inputs = closed();
    inputs[2].value.scope.interval.end_ns = 0;
    rebind(&mut inputs[2]);
    refusal(&inputs, RefusalCode::OutsidePhysicalDomain109);
    let largest = i128::MAX.to_string();
    refusal(
        &interval([&largest, "1", "1", "0", &largest]),
        RefusalCode::NumericalCertificateFailed207,
    );
}

#[test]
fn evidence_never_strengthens_and_assertions_keep_their_colour_refusal() {
    for evidence in [
        EvidenceClass::Measured,
        EvidenceClass::Derived,
        EvidenceClass::ModelledCalibrated,
        EvidenceClass::ModelledUncalibrated,
        EvidenceClass::Transferred {
            aoa: AoaStatus::Inside,
        },
        EvidenceClass::Elicited,
        EvidenceClass::Assumed,
        EvidenceClass::Assertion {
            source: AssertionSource::Extraction,
        },
    ] {
        let mut inputs = closed();
        for input in &mut inputs {
            input.evidence = EvidenceClass::Measured;
            rebind(input);
        }
        inputs[0].evidence = evidence; // The completeness declaration participates too.
        inputs[0].state = KnowledgeState::Estimated;
        inputs[0].epistemic = EpistemicStatus::KnownUnknown(KuKind::Parametric);
        rebind(&mut inputs[0]);
        let receipt = account_interval(&inputs).unwrap();
        let result = receipt.claim();
        assert_eq!(result.evidence, EvidenceClass::Derived.meet(evidence));
        assert!(
            inputs
                .iter()
                .all(|i| result.evidence.no_stronger_than(i.evidence))
        );
        assert_eq!(result.state, KnowledgeState::Estimated);
        assert_eq!(
            result.epistemic,
            EpistemicStatus::KnownUnknown(KuKind::Parametric)
        );
        if matches!(evidence, EvidenceClass::Assertion { .. }) {
            assert!(result.color().is_err());
        }
    }
}

#[test]
fn confidentiality_joins_every_input_including_the_completeness_declaration() {
    let registry = LabelRegistry::new(["JV-A"], ["W-A"]).unwrap();
    let wall = Confidentiality::mnpi(registry.wall("W-A").unwrap());
    let venture = Confidentiality::venture(registry.venture("JV-A").unwrap());
    let mut inputs = closed();
    inputs[0].confidentiality = wall.clone();
    inputs[5].confidentiality = venture.clone();
    rebind(&mut inputs[0]);
    rebind(&mut inputs[5]);
    let receipt = account_interval(&inputs).unwrap();
    let output = receipt.claim();
    assert_eq!(output.confidentiality, wall.join(&venture));
    assert_eq!(
        output
            .verify_export_clearance(&Confidentiality::public())
            .unwrap_err()
            .code,
        "FMINE-REF-301"
    );
    assert_eq!(
        output.verify_export_clearance(&wall).unwrap_err().code,
        "FMINE-REF-302"
    );
    output
        .verify_export_clearance(&wall.join(&venture))
        .unwrap();
}

fn uu() -> UuTerm {
    UuTerm {
        scenario_id: "unobserved-holdup".into(),
        description: "Constructed inventory omitted from a source stocktake".into(),
        downside_exposure_usd_m: 1.0,
        trigger_indicator: "Independent stocktake disagreement".into(),
    }
}

#[test]
fn uu_metadata_is_retained_conflicts_refuse_and_unhashed_changes_alter_receipts() {
    let inputs = closed();
    let baseline = account_interval(&inputs).unwrap();
    let mut inputs = inputs;
    let original_id = inputs[2].claim_id;
    inputs[2].uu_terms.push(uu());
    inputs[4].uu_terms.push(uu());
    assert_eq!(inputs[2].claim_id, original_id);
    let with_uu = account_interval(&inputs).unwrap();
    assert_eq!(with_uu.claim().uu_terms, vec![uu()]);
    assert_ne!(with_uu.digest(), baseline.digest());
    inputs[4].uu_terms[0].description = "Conflicting scenario definition".into();
    refusal(&inputs, RefusalCode::UnbudgetedUnknowns204);
    inputs[4].uu_terms[0] = uu();
    inputs[4].uu_terms[0].downside_exposure_usd_m = f64::NAN;
    refusal(&inputs, RefusalCode::UnbudgetedUnknowns204);
}

#[test]
fn calibration_metadata_changes_identity_without_promoting_balance_to_validated() {
    let mut inputs = closed();
    for input in &mut inputs {
        input.evidence = EvidenceClass::Measured;
        rebind(input);
    }
    let baseline = account_interval(&inputs).unwrap();
    let id = inputs[2].claim_id;
    inputs[2] = inputs[2].clone().with_regime(
        ValidityDomain::unconstrained().with("assay-range", 0.0, 100.0),
        "synthetic-assay-method/v1",
    );
    assert_eq!(inputs[2].claim_id, id);
    let calibrated_input = account_interval(&inputs).unwrap();
    assert_ne!(baseline.digest(), calibrated_input.digest());
    assert!(matches!(
        calibrated_input.claim().color().unwrap(),
        Color::Estimated { .. }
    ));
    inputs[2] = inputs[2]
        .clone()
        .with_regime(ValidityDomain::unconstrained(), "invalid empty regime");
    refusal(&inputs, RefusalCode::CalibrationRequired205);
}

#[test]
fn receipt_identity_is_input_order_independent_and_immutable() {
    let mut inputs = closed();
    let baseline = account_interval(&inputs).unwrap();
    for _ in 0..6 {
        inputs.rotate_left(1);
        assert_eq!(account_interval(&inputs).unwrap(), baseline);
        inputs.reverse();
        assert_eq!(account_interval(&inputs).unwrap(), baseline);
        inputs.reverse();
    }
    let mut cloned_output = baseline.claim().clone();
    cloned_output.value.product_kg = q("999");
    let mut cloned_input = baseline.inputs()[2].clone();
    cloned_input.value.value = BalanceValue::Feed(q("999"));
    assert_eq!(baseline.summary().product_kg, q("72"));
    assert_eq!(
        baseline.inputs()[2].value.value,
        BalanceValue::Feed(q("100"))
    );
    assert_eq!(account_interval(&inputs).unwrap(), baseline);
}
