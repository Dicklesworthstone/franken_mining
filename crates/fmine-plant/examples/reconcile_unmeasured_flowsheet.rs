//! Constructed stock drawdown with a genuinely unmeasured product stream.
//! The no-redundancy result is an inference, not a passed gross-error test.
//! Determinism: DeterministicPerIsa

use fmine_claim::{AdditivityClass, Claim, Confidentiality, Representation, SpatialSupport};
use fmine_core::{anchor::SourceAnchor, hash::sha256};
use fmine_evidence::{EpistemicStatus, EvidenceClass, KnowledgeState};
use fmine_plant::{
    flowsheet::{
        ElementKey, FlowsheetInput, FlowsheetScope, FlowsheetValue, MaterialAmounts, StocktakeKind,
        StreamId, StreamRoute, StreamSpec, UnitId, UnitKind, UnitSpec,
        reconciliation::{
            MaterialChannel,
            unmeasured::{
                ERROR_MODEL_METRIC, GrossErrorDiagnostic, UnmeasuredFlowModel, observation_order,
                reconcile_unmeasured_flowsheet,
            },
        },
    },
    reconciliation::{Kilograms, KilogramsSquared},
};
use fmine_time::ValidTime;
use fmine_units::ExactQty;
use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
};

type Result<T> = std::result::Result<T, Box<dyn Error>>;
fn q(s: &str) -> Result<ExactQty> {
    Ok(ExactQty::from_decimal_str(s)?)
}
fn gold() -> ElementKey {
    ElementKey {
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
            "crates/fmine-plant/examples/reconcile_unmeasured_flowsheet.rs",
            sha256(include_bytes!("reconcile_unmeasured_flowsheet.rs")).0,
            "constructed stock observations, explicit missing-stream declaration and assumed covariance",
        )],
    )
}
fn run() -> Result<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let ambiguous = match args.as_slice() {
        [] => false,
        [arg] if arg == "--ambiguous" => true,
        _ => return Err("usage: reconcile_unmeasured_flowsheet [--ambiguous]".into()),
    };
    let scope = FlowsheetScope {
        boundary: "synthetic-stock-drawdown/v1".into(),
        interval: ValidTime::new(0, 10)?,
    };
    let unit = UnitId::new("store")?;
    let product = StreamId::new("product")?;
    let mut unmeasured = BTreeSet::from([product.clone()]);
    let mut streams = vec![StreamSpec {
        id: product,
        route: StreamRoute::Product { from: unit.clone() },
    }];
    if ambiguous {
        let tails = StreamId::new("tails")?;
        unmeasured.insert(tails.clone());
        streams.push(StreamSpec {
            id: tails,
            route: StreamRoute::OtherOutflow { from: unit.clone() },
        });
    }
    let amount = |dry, water, au| -> Result<MaterialAmounts> {
        Ok(MaterialAmounts {
            dry_solids_kg: q(dry)?,
            water_kg: q(water)?,
            elements_kg: BTreeMap::from([(gold(), q(au)?)]),
        })
    };
    let values = [
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
            amounts: amount("100", "40", "2")?,
        },
        FlowsheetValue::Stocktake {
            unit,
            kind: StocktakeKind::Closing,
            amounts: amount("70", "25", "1.4")?,
        },
    ];
    let inputs: Vec<_> = values
        .into_iter()
        .map(|value| {
            let physical_time = match value {
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
            source(
                fmine_plant::flowsheet::INPUT_METRIC,
                input,
                &tag,
                additivity,
            )
        })
        .collect();
    let observations = observation_order(&inputs, &unmeasured)?;
    let n = observations.len();
    let mut covariance: Vec<_> = (0..n * n)
        .map(|i| {
            KilogramsSquared::new(if i / n != i % n {
                0.0
            } else {
                match observations[i / n].channel {
                    MaterialChannel::DrySolids => 4.0,
                    MaterialChannel::Water => 1.0,
                    MaterialChannel::Element(_) => 0.0004,
                }
            })
        })
        .collect();
    // Opening dry mass and gold error have declared +0.02 kg² covariance.
    covariance[2] = KilogramsSquared::new(0.02);
    covariance[2 * n] = KilogramsSquared::new(0.02);
    let model = UnmeasuredFlowModel {
        scope,
        declaration_id: inputs[0].claim_id,
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
        unmeasured_streams: unmeasured,
        covariance,
        unbiased_gaussian_errors: true,
        global_chi_square_limit: 9.0,
        normalized_adjustment_limit: None,
    };
    let tag = model.canonical_tag();
    let model = source(
        ERROR_MODEL_METRIC,
        model,
        &tag,
        AdditivityClass::NonAdditive,
    );
    let result = reconcile_unmeasured_flowsheet(&inputs, &model)?;
    let claim = result.admitted_claim()?;
    let d = &claim.value;
    let total = d.estimates_kg.len();
    println!(
        "SYNTHETIC conditional stock-drawdown inference; all source/model evidence is Assumed."
    );
    println!(
        "{} measured coordinates, {} inferred coordinates; no product measurement was supplied.",
        n,
        d.unmeasured.len()
    );
    match d.gross_error {
        GrossErrorDiagnostic::NoRedundancy => {
            println!("Gross-error test unavailable: there are no redundant constraints.");
        }
        GrossErrorDiagnostic::Available {
            chi_square,
            degrees_of_freedom,
        } => {
            println!(
                "Gross-error chi-square {chi_square}, degrees of freedom {degrees_of_freedom}"
            );
        }
    }
    for (j, key) in d.unmeasured.iter().enumerate() {
        let i = n + j;
        println!(
            "{} {:?}: inferred {:.9} kg, variance {:.12} kg²",
            key.stream.as_str(),
            key.channel,
            d.estimates_kg[i].value(),
            d.posterior_covariance[i * total + i].value()
        );
    }
    println!(
        "Inferred product dry/Au covariance: {:.12} kg²",
        d.posterior_covariance[n * total + n + 2].value()
    );
    for (channel, residual) in &d.boundary.reconciled_residuals_kg {
        println!(
            "Boundary {channel:?}: fitted residual {:.3e} kg",
            residual.value()
        );
    }
    println!("claim={}", claim.claim_id);
    Ok(())
}
fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
