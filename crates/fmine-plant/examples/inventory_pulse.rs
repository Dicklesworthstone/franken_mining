//! A constructed pulse/hold-up schedule; no plant measurements or fitted RTD.
//!
//! Run: cargo run -p fmine-plant --example inventory_pulse
//! All values in SCHEDULE are kg of total Au across a full synthetic boundary.

use fmine_claim::{Claim, Confidentiality, Representation, SpatialSupport};
use fmine_core::{anchor::SourceAnchor, hash::sha256};
use fmine_evidence::{EpistemicStatus, EvidenceClass, KnowledgeState, Refusal};
use fmine_plant::inventory::{
    BalanceScope, BalanceValue, INPUT_METRIC, MetalBalanceInput, NO_CLAIM, account_interval,
};
use fmine_time::ValidTime;
use fmine_units::ExactQty;

// Independent constructed schedule: opening, feed, product, other outflow, closing.
// Each adjacent pair shares one stocktake; zeros are stated, never inferred.
const SCHEDULE: [(&str, [i128; 5]); 4] = [
    ("feed pulse", [0, 100, 30, 10, 60]),
    ("inventory drawdown", [60, 20, 60, 10, 10]),
    ("shutdown", [10, 0, 0, 0, 10]),
    ("final release", [10, 0, 8, 2, 0]),
];
const HOUR_NS: i64 = 3_600_000_000_000;

fn inputs(step: i64, values: [i128; 5]) -> Vec<Claim<MetalBalanceInput>> {
    let scope = BalanceScope {
        boundary: "synthetic-CIL-full-boundary/v1".into(),
        interval: ValidTime {
            start_ns: step * HOUR_NS,
            end_ns: (step + 1) * HOUR_NS,
        },
        element: "Au".into(),
        measurand: "total-Au/all-phases".into(),
    };
    let [opening, feed, product, other, closing] = values.map(ExactQty::from_integer);
    [
        BalanceValue::Population { complete: true },
        BalanceValue::OpeningInventory(opening),
        BalanceValue::Feed(feed),
        BalanceValue::Product(product),
        BalanceValue::OtherOutflow(other),
        BalanceValue::ClosingInventory(closing),
    ]
    .into_iter()
    .enumerate()
    .map(|(role, value)| {
        let physical_time = match value {
            BalanceValue::OpeningInventory(_) => ValidTime::instant(scope.interval.start_ns),
            BalanceValue::ClosingInventory(_) => ValidTime::instant(scope.interval.end_ns),
            _ => scope.interval,
        };
        let input = MetalBalanceInput {
            scope: scope.clone(),
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
            value.additivity(),
            EvidenceClass::Assumed,
            EpistemicStatus::KnownKnown,
            KnowledgeState::Known,
            Confidentiality::public(),
            vec![],
            vec![],
            vec![SourceAnchor::new(
                "crates/fmine-plant/examples/inventory_pulse.rs",
                sha256(include_bytes!("inventory_pulse.rs")).0,
                format!("SCHEDULE[{step}], declared role {role}; constructed complete boundary"),
            )],
        )
    })
    .collect()
}

fn ratio_text(value: Option<ExactQty>) -> String {
    value.map_or_else(
        || "absent (zero feed)".into(),
        |q| format!("{}/{}", q.num(), q.den()),
    )
}

fn main() -> Result<(), Refusal> {
    // Stage every result before publishing an output. No partial successful run
    // is printed if any interval has invalid input or fails exact closure.
    let receipts = SCHEDULE
        .iter()
        .enumerate()
        .map(|(step, (_, masses))| account_interval(&inputs(step as i64, *masses)))
        .collect::<Result<Vec<_>, _>>()?;
    println!(
        "SYNTHETIC elemental inventory: constructed pulse and retained stock, not a fitted plant model."
    );
    println!(
        "interval | feed Au kg | product Au kg | inventory change kg | product/feed diagnostic | residual kg"
    );
    for ((label, _), receipt) in SCHEDULE.iter().zip(&receipts) {
        let result = receipt.summary();
        println!(
            "{label} | {} | {} | {} | {} | {}",
            result.feed_kg.num(),
            result.product_kg.num(),
            result.inventory_change_kg.num(),
            ratio_text(result.product_to_feed_ratio),
            result.residual_kg.num()
        );
        println!(
            "  claim={} receipt={}",
            receipt.claim().claim_id,
            receipt.digest()
        );
    }
    println!(
        "The 3/1 same-interval ratio occurs while 50 kg leaves inventory; it is not 300% recovery."
    );
    println!("{NO_CLAIM}");
    Ok(())
}
