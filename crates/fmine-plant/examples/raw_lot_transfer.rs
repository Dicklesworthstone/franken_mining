//! Constructed complete lot transfer, not a calibrated sampler or operating plant.
//! Run: cargo run -p fmine-plant --example raw_lot_transfer
//! Determinism: exact nominal arithmetic; DeterministicPerIsa covariance.

use fmine_claim::{AdditivityClass, Claim, Confidentiality, Representation, SpatialSupport};
use fmine_core::{anchor::SourceAnchor, hash::sha256};
use fmine_evidence::{EpistemicStatus, EvidenceClass, KnowledgeState, Refusal, RefusalCode};
use fmine_plant::{
    flowsheet::{ElementKey, FlowsheetScope, StreamId},
    raw_measurements::{
        ERROR_MODEL_METRIC, INPUT_METRIC, RawTransferErrorModel, RawTransferInput,
        RawTransferValue, StandardUncertainty, observation_order, project_transfer,
    },
    reconciliation::Kilograms,
};
use fmine_time::ValidTime;
use fmine_units::ExactQty;

fn q(value: &str) -> Result<ExactQty, Refusal> {
    ExactQty::from_decimal_str(value).map_err(|_| {
        Refusal::new(
            RefusalCode::OutsidePhysicalDomain109,
            "invalid constructed exact decimal",
        )
    })
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
            "crates/fmine-plant/examples/raw_lot_transfer.rs",
            sha256(include_bytes!("raw_lot_transfer.rs")).0,
            "constructed complete lots, total assays and full cross-lot error correlation",
        )],
    )
}

fn main() -> Result<(), Refusal> {
    let scope = FlowsheetScope {
        boundary: "synthetic-raw-lot-feed/v1".into(),
        interval: ValidTime {
            start_ns: 0,
            end_ns: 20,
        },
    };
    let stream = StreamId::new("feed")?;
    let copper = ElementKey {
        element: "Cu".into(),
        measurand: "total-Cu/all-phases".into(),
    };
    let mut values = vec![RawTransferValue::Declaration {
        lot_ids: vec!["lot-1".into(), "lot-2".into()],
        elements: vec![copper.clone()],
        complete: true,
        nonoverlapping_lots: true,
        moisture_is_h2o: true,
        assays_are_total_elemental: true,
    }];
    for (lot, mass, moisture, grade) in [
        ("lot-1", "100", "0.1", "0.01"),
        ("lot-2", "200", "0.2", "0.02"),
    ] {
        values.extend([
            RawTransferValue::WetMass {
                lot: lot.into(),
                kg: q(mass)?,
            },
            RawTransferValue::Moisture {
                lot: lot.into(),
                wet_mass_fraction: q(moisture)?,
            },
            RawTransferValue::DryAssay {
                lot: lot.into(),
                element: copper.clone(),
                dry_mass_fraction: q(grade)?,
            },
        ]);
    }
    let inputs: Vec<_> = values
        .into_iter()
        .map(|value| {
            let support = if matches!(value, RawTransferValue::Declaration { .. }) {
                SpatialSupport::PeriodAggregate
            } else {
                SpatialSupport::PlantLot
            };
            let additivity = value.additivity();
            let input = RawTransferInput {
                scope: scope.clone(),
                stream: stream.clone(),
                dry_basis: "synthetic-matched-moisture-and-assay-dry-state/v1".into(),
                physical_time: scope.interval,
                value,
            };
            let tag = input.canonical_tag();
            source(INPUT_METRIC, input, &tag, additivity, support)
        })
        .collect();
    let mut correlation = vec![0.0; 36];
    for i in 0..6 {
        correlation[6 * i + i] = 1.0;
    }
    // Explicit 0.5 correlation between lot weights; every other off-diagonal
    // is explicitly zero in this constructed model, not inferred by the API.
    correlation[3] = 0.5;
    correlation[18] = 0.5;
    let value = RawTransferErrorModel {
        scope,
        stream,
        declaration_id: inputs[0].claim_id,
        observation_ids: observation_order(&inputs)?,
        standard_uncertainties: vec![
            StandardUncertainty::MassKg(Kilograms::new(1.0)),
            StandardUncertainty::Fraction(0.01),
            StandardUncertainty::Fraction(0.001),
            StandardUncertainty::MassKg(Kilograms::new(1.0)),
            StandardUncertainty::Fraction(0.01),
            StandardUncertainty::Fraction(0.001),
        ],
        correlation,
        first_order_approximation_accepted: true,
    };
    let tag = value.canonical_tag();
    let model = source(
        ERROR_MODEL_METRIC,
        value,
        &tag,
        AdditivityClass::NonAdditive,
        SpatialSupport::PeriodAggregate,
    );
    let receipt = project_transfer(&inputs, &model)?;
    let result = &receipt.claim().value;
    println!(
        "SYNTHETIC lot projection: exact nominal products; explicitly first-order covariance."
    );
    println!(
        "Dry matter = {:.6} kg; H2O = {:.6} kg; Cu = {:.6} kg",
        result.amounts.dry_solids_kg.to_f64(),
        result.amounts.water_kg.to_f64(),
        result.amounts.elements_kg[&copper].to_f64()
    );
    println!(
        "Aggregate dry Cu grade = {:.6}% (ratio of summed Cu to summed dry matter)",
        result.dry_grade_mass_fractions[&copper].to_f64() * 100.0
    );
    println!("First-order covariance in kg^2; row/column order: dry matter, H2O, Cu");
    for row in result.linearized_covariance.as_chunks::<3>().0 {
        println!(
            "  [{:.6}, {:.6}, {:.6}]",
            row[0].value(),
            row[1].value(),
            row[2].value()
        );
    }
    println!(
        "Nominal flowsheet transfer: {}",
        receipt.transfer().claim_id
    );
    println!("Parent projection: {}", receipt.claim().claim_id);
    println!("Receipt: {}", receipt.digest());
    println!(
        "No Gaussian transformed law, sampling coverage or calibration is inferred. A graph WLS model must be separately declared."
    );
    Ok(())
}
