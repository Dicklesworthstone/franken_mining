//! Constructed source observations and analytical checks; no plant calibration.
//! Determinism: DeterministicPerIsa

use super::*;
use fmine_core::{anchor::SourceAnchor, hash::sha256};
use fmine_time::ValidTime;

fn q(s: &str) -> ExactQty {
    ExactQty::from_decimal_str(s).unwrap()
}
fn close(a: f64, b: f64) {
    assert!((a - b).abs() < 2e-10 * b.abs().max(1.0), "{a} != {b}");
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
            "crates/fmine-plant/src/reconciliation/tests.rs",
            sha256(include_bytes!("tests.rs")).0,
            "constructed observations and declared error model",
        )],
    )
}
fn setup(
    masses: [&str; 5],
    variance: [f64; 5],
) -> (Vec<Claim<MetalBalanceInput>>, Claim<InventoryErrorModel>) {
    let scope = BalanceScope {
        boundary: "synthetic-full-plant/v1".into(),
        interval: ValidTime::new(0, 10).unwrap(),
        element: "Au".into(),
        measurand: "total-Au/all-phases".into(),
    };
    let [o, f, p, w, c] = masses.map(q);
    let values = [
        BalanceValue::Population { complete: true },
        BalanceValue::OpeningInventory(o),
        BalanceValue::Feed(f),
        BalanceValue::Product(p),
        BalanceValue::OtherOutflow(w),
        BalanceValue::ClosingInventory(c),
    ];
    let inputs: Vec<_> = values
        .into_iter()
        .map(|value| {
            let physical_time = match value {
                BalanceValue::OpeningInventory(_) => ValidTime::instant(0),
                BalanceValue::ClosingInventory(_) => ValidTime::instant(10),
                _ => scope.interval,
            };
            let input = MetalBalanceInput {
                scope: scope.clone(),
                physical_time,
                value,
            };
            let tag = input.canonical_tag();
            source(inventory::INPUT_METRIC, input, &tag, value.additivity())
        })
        .collect();
    let model = InventoryErrorModel {
        scope,
        observation_ids: core::array::from_fn(|i| inputs[i + 1].claim_id),
        covariance: core::array::from_fn(|i| {
            KilogramsSquared::new(if i / 5 == i % 5 { variance[i / 5] } else { 0.0 })
        }),
        unbiased_gaussian_errors: true,
        global_chi_square_limit: 9.0,
        normalized_adjustment_limit: None,
        closure_tolerance_kg: Kilograms::new(1e-9),
    };
    let tag = model.canonical_tag();
    let model = source(
        ERROR_MODEL_METRIC,
        model,
        &tag,
        AdditivityClass::NonAdditive,
    );
    (inputs, model)
}
fn rebind_model(model: &mut Claim<InventoryErrorModel>) {
    let value = model.value.clone();
    let tag = value.canonical_tag();
    *model = source(
        ERROR_MODEL_METRIC,
        value,
        &tag,
        AdditivityClass::NonAdditive,
    );
}

#[test]
fn uncertain_inventory_closes_with_full_covariance_and_original_sources() {
    let (inputs, model) = setup(["10", "100", "72", "8", "31"], [1.0, 4.0, 1.0, 1.0, 9.0]);
    assert_eq!(
        inventory::account_interval(&inputs).unwrap_err().code,
        "FMINE-REF-101"
    );
    let result = reconcile_inventory(&inputs, &model).unwrap();
    let InventoryReconciliationOutcome::Admitted(receipt) = result else {
        panic!("must admit under declared error model")
    };
    let d = &receipt.claim().value;
    for (actual, expected) in d
        .estimates_kg
        .iter()
        .zip([10.0625, 100.25, 71.9375, 7.9375, 30.4375])
    {
        close(actual.value(), expected);
    }
    close(d.chi_square, 1.0 / 16.0);
    close(d.reconciled_residual_kg.value(), 0.0);
    assert_eq!(d.raw_residual_exact_kg, q("-1"));
    close(d.posterior_covariance[1].value(), -4.0 / 16.0);
    assert_eq!(receipt.inputs(), inputs);
    assert_eq!(receipt.error_model(), &model);
    assert_eq!(receipt.claim().evidence, EvidenceClass::Assumed);
    assert_eq!(receipt.claim().state, KnowledgeState::Estimated);
    assert_eq!(receipt.claim().parents.len(), 7);
    receipt.claim().check_id().unwrap();
}

#[test]
fn declared_correlation_changes_the_weighted_adjustment() {
    let (inputs, mut model) = setup(["10", "100", "72", "8", "31"], [1.0, 4.0, 1.0, 1.0, 9.0]);
    model.value.covariance[9] = KilogramsSquared::new(3.0);
    model.value.covariance[21] = KilogramsSquared::new(3.0);
    rebind_model(&mut model);
    let result = reconcile_inventory(&inputs, &model).unwrap();
    let d = &result.admitted_claim().unwrap().value;
    close(d.chi_square, 0.1);
    close(d.estimates_kg[1].value(), 100.1);
    close(d.estimates_kg[4].value(), 30.4);
}

#[test]
fn gross_errors_remain_visible_without_an_admitted_balance_claim() {
    let (inputs, model) = setup(["10", "100", "72", "8", "50"], [1.0; 5]);
    let result = reconcile_inventory(&inputs, &model).unwrap();
    assert_eq!(result.admitted_claim().unwrap_err().code, "FMINE-REF-101");
    let InventoryReconciliationOutcome::Rejected(report) = result else {
        panic!("large imbalance must reject")
    };
    assert_eq!(report.inputs(), inputs);
    assert_eq!(report.error_model(), &model);
    close(report.diagnostics().chi_square, 80.0);
    assert_eq!(report.diagnostics().raw_residual_exact_kg, q("-20"));
    assert!(report.refusal().what.contains("all observations retained"));
    // One balance gives equal-magnitude standardized diagnostics; no unique
    // sensor can be blamed from this boundary equation alone.
    for z in report.diagnostics().normalized_adjustments {
        close(z.unwrap().abs(), 80.0_f64.sqrt());
    }
}

#[test]
fn declared_adjustment_policy_and_negative_fitted_mass_reject() {
    let (inputs, mut model) = setup(["10", "100", "72", "8", "31"], [1.0; 5]);
    model.value.normalized_adjustment_limit = Some(0.1);
    rebind_model(&mut model);
    assert_eq!(
        reconcile_inventory(&inputs, &model)
            .unwrap()
            .admitted_claim()
            .unwrap_err()
            .code,
        "FMINE-REF-101"
    );
    let (inputs, mut model) = setup(["0", "1", "0", "0", "0"], [100.0, 1.0, 1.0, 1.0, 1.0]);
    model.value.global_chi_square_limit = 1e6;
    rebind_model(&mut model);
    assert_eq!(
        reconcile_inventory(&inputs, &model)
            .unwrap()
            .admitted_claim()
            .unwrap_err()
            .code,
        "FMINE-REF-109"
    );
}

#[test]
fn model_payload_claim_identity_and_observation_binding_are_enforced() {
    let (inputs, model) = setup(["10", "100", "72", "8", "31"], [1.0; 5]);
    let mut wrong = model.clone();
    wrong.value.covariance[0] = KilogramsSquared::new(2.0);
    wrong.check_id().unwrap();
    assert_eq!(
        reconcile_inventory(&inputs, &wrong).unwrap_err().code,
        "FMINE-REF-507"
    );
    let mut wrong = model.clone();
    wrong.value.observation_ids.swap(0, 1);
    rebind_model(&mut wrong);
    assert_eq!(
        reconcile_inventory(&inputs, &wrong).unwrap_err().code,
        "FMINE-REF-102"
    );
    let mut wrong = model.clone();
    wrong.value.scope.element = "Cu".into();
    rebind_model(&mut wrong);
    assert_eq!(
        reconcile_inventory(&inputs, &wrong).unwrap_err().code,
        "FMINE-REF-102"
    );
    let mut changed = inputs.clone();
    changed[2].value.value = BalanceValue::Feed(q("101"));
    assert_eq!(
        reconcile_inventory(&changed, &model).unwrap_err().code,
        "FMINE-REF-507"
    );
}

#[test]
fn invalid_models_states_sources_and_covariance_refuse() {
    let (inputs, model) = setup(["10", "100", "72", "8", "31"], [1.0; 5]);
    for defect in 0..8 {
        let mut wrong = model.clone();
        match defect {
            0 => wrong.value.unbiased_gaussian_errors = false,
            1 => wrong.value.covariance[0] = KilogramsSquared::new(0.0),
            2 => wrong.value.covariance[1] = KilogramsSquared::new(0.5),
            3 => {
                wrong.value.covariance[1] = KilogramsSquared::new(2.0);
                wrong.value.covariance[5] = KilogramsSquared::new(2.0);
            }
            4 => wrong.value.global_chi_square_limit = f64::INFINITY,
            5 => wrong.value.normalized_adjustment_limit = Some(0.0),
            6 => wrong.value.closure_tolerance_kg = Kilograms::new(-1.0),
            _ => wrong.value.covariance[0] = KilogramsSquared::new(f64::NAN),
        }
        rebind_model(&mut wrong);
        assert!(reconcile_inventory(&inputs, &wrong).is_err());
    }
    let mut wrong = model.clone();
    wrong.stale = true;
    assert_eq!(
        reconcile_inventory(&inputs, &wrong).unwrap_err().code,
        "FMINE-REF-502"
    );
    let mut wrong = model.clone();
    wrong.calibration_failed = true;
    assert_eq!(
        reconcile_inventory(&inputs, &wrong).unwrap_err().code,
        "FMINE-REF-205"
    );
    let mut wrong = model.clone();
    wrong.anchors.clear();
    assert!(reconcile_inventory(&inputs, &wrong).is_err());
    assert!(reconcile_inventory(&inputs[..5], &model).is_err());
}

#[test]
fn model_confidentiality_and_evidence_participate_in_the_result() {
    use fmine_claim::LabelRegistry;
    let (inputs, mut model) = setup(["10", "100", "72", "8", "31"], [1.0; 5]);
    let registry = LabelRegistry::new(["JV-A"], ["W-A"]).unwrap();
    let restricted = Confidentiality::mnpi(registry.wall("W-A").unwrap());
    let value = model.value.clone();
    let tag = value.canonical_tag();
    model = Claim::new(
        ERROR_MODEL_METRIC,
        value,
        &tag,
        Representation::Deterministic,
        SpatialSupport::PeriodAggregate,
        AdditivityClass::NonAdditive,
        EvidenceClass::Assumed,
        EpistemicStatus::KnownKnown,
        KnowledgeState::Known,
        restricted.clone(),
        vec![],
        vec![],
        model.anchors.clone(),
    );
    let result = reconcile_inventory(&inputs, &model).unwrap();
    let claim = result.admitted_claim().unwrap();
    assert_eq!(claim.confidentiality, restricted);
    assert_eq!(
        claim
            .verify_export_clearance(&Confidentiality::public())
            .unwrap_err()
            .code,
        "FMINE-REF-301"
    );
}

#[test]
fn original_exact_imbalance_cannot_disappear_during_float_conversion() {
    let (inputs, model) = setup(
        ["10", "10000000000000001", "10000000000000000", "0", "10"],
        [1e-4; 5],
    );
    let refusal = reconcile_inventory(&inputs, &model).unwrap_err();
    assert_eq!(refusal.code, "FMINE-REF-207");
    assert!(refusal.what.contains("conversion"));
}

#[test]
fn decimal_observations_and_input_order_retain_repeatable_receipts() {
    let (mut inputs, model) = setup(["0.1", "0.2", "0.24", "0.01", "0.04"], [1e-4; 5]);
    let result = reconcile_inventory(&inputs, &model).unwrap();
    assert!(result.admitted_claim().is_ok());
    assert_eq!(result.diagnostics().raw_residual_exact_kg, q("0.01"));
    inputs.reverse();
    assert_eq!(reconcile_inventory(&inputs, &model).unwrap(), result);
}
