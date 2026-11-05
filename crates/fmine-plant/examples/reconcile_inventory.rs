//! Constructed noisy stocktakes and flows, not a calibrated operating plant.
//! Run: cargo run -p fmine-plant --example reconcile_inventory
use fmine_claim::{AdditivityClass, Claim, Confidentiality, Representation, SpatialSupport};
use fmine_core::{anchor::SourceAnchor, hash::sha256};
use fmine_evidence::{EpistemicStatus, EvidenceClass, KnowledgeState, Refusal};
use fmine_plant::{
    inventory::{BalanceScope, BalanceValue, INPUT_METRIC, MetalBalanceInput},
    reconciliation::{
        ERROR_MODEL_METRIC, InventoryErrorModel, InventoryReconciliationOutcome, Kilograms,
        KilogramsSquared, reconcile_inventory,
    },
};
use fmine_time::ValidTime;
use fmine_units::ExactQty;

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
            "crates/fmine-plant/examples/reconcile_inventory.rs",
            sha256(include_bytes!("reconcile_inventory.rs")).0,
            "constructed masses, diagonal variances and illustrative chi-square threshold",
        )],
    )
}
fn example(closing: i128) -> Result<InventoryReconciliationOutcome, Refusal> {
    let scope = BalanceScope {
        boundary: "synthetic-Au-plant/v1".into(),
        interval: ValidTime {
            start_ns: 0,
            end_ns: 10,
        },
        element: "Au".into(),
        measurand: "total-Au/all-phases".into(),
    };
    let [o, f, p, w, c] = [10, 100, 72, 8, closing].map(ExactQty::from_integer);
    let inputs: Vec<_> = [
        BalanceValue::Population { complete: true },
        BalanceValue::OpeningInventory(o),
        BalanceValue::Feed(f),
        BalanceValue::Product(p),
        BalanceValue::OtherOutflow(w),
        BalanceValue::ClosingInventory(c),
    ]
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
        source(INPUT_METRIC, input, &tag, value.additivity())
    })
    .collect();
    let variances = [1.0, 4.0, 1.0, 1.0, 9.0];
    let model = InventoryErrorModel {
        scope,
        observation_ids: core::array::from_fn(|i| inputs[i + 1].claim_id),
        covariance: core::array::from_fn(|i| {
            KilogramsSquared::new(if i / 5 == i % 5 {
                variances[i / 5]
            } else {
                0.0
            })
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
    reconcile_inventory(&inputs, &model)
}
fn main() -> Result<(), Refusal> {
    let outcomes = [
        ("small discrepancy", example(31)?),
        ("gross discrepancy", example(50)?),
    ];
    println!(
        "SYNTHETIC inventory reconciliation with declared Gaussian errors; threshold is illustrative."
    );
    for (label, outcome) in outcomes {
        let d = outcome.diagnostics();
        println!(
            "{label}: raw residual={:.9} kg; chi-square={:.9}; degrees of freedom={}",
            d.raw_residual_kg.value(),
            d.chi_square,
            d.degrees_of_freedom
        );
        for (i, role) in ["opening", "feed", "product", "other outflow", "closing"]
            .iter()
            .enumerate()
        {
            println!(
                "  {role}: observed={:.9} kg; candidate={:.9} kg; adjustment={:.9} kg; candidate variance={:.9} kg^2",
                d.observed_kg[i].value(),
                d.estimates_kg[i].value(),
                d.adjustments_kg[i].value(),
                d.posterior_covariance[i * 5 + i].value()
            );
        }
        match outcome {
            InventoryReconciliationOutcome::Admitted(receipt) => println!(
                "  ADMITTED conditional balance: {}",
                receipt.claim().claim_id
            ),
            InventoryReconciliationOutcome::Rejected(report) => {
                println!("  REJECTED; no accounting claim: {}", report.refusal())
            }
        }
    }
    println!(
        "All original measurements remain in the receipt. A single boundary balance cannot identify the faulty instrument."
    );
    Ok(())
}
