//! Constructed mill/classifier reconciliation with shared recycle observations.
//! No site data, calibration, P754 audit or process prediction is implied.
//! Determinism: DeterministicPerIsa

use fmine_claim::{AdditivityClass, Claim, Confidentiality, Representation, SpatialSupport};
use fmine_core::{anchor::SourceAnchor, hash::sha256};
use fmine_evidence::{EpistemicStatus, EvidenceClass, KnowledgeState};
use fmine_plant::{
    flowsheet::{
        ElementKey, FlowsheetInput, FlowsheetScope, FlowsheetValue, MaterialAmounts, OutflowRole,
        StocktakeKind, StreamId, StreamRoute, StreamSpec, UnitId, UnitKind, UnitSpec,
        reconciliation::{
            BoundaryRole, ERROR_MODEL_METRIC, FlowsheetErrorModel, MaterialChannel,
            ObservationLocation, observation_order, reconcile_flowsheet,
        },
    },
    reconciliation::{Kilograms, KilogramsSquared},
};
use fmine_time::ValidTime;
use fmine_units::ExactQty;
use std::{collections::BTreeMap, error::Error};

type Result<T> = std::result::Result<T, Box<dyn Error>>;
fn q(value: &str) -> Result<ExactQty> {
    Ok(ExactQty::from_decimal_str(value)?)
}
fn unit(name: &str) -> Result<UnitId> {
    Ok(UnitId::new(name)?)
}
fn stream(name: &str) -> Result<StreamId> {
    Ok(StreamId::new(name)?)
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
            "crates/fmine-plant/examples/reconcile_flowsheet.rs",
            sha256(include_bytes!("reconcile_flowsheet.rs")).0,
            "synthetic observations, correlated Gaussian error declaration and demonstration policy",
        )],
    )
}
fn bind(scope: &FlowsheetScope, value: FlowsheetValue) -> Claim<FlowsheetInput> {
    let physical_time = match value {
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
}
fn amounts(dry: &str, water: &str, au: &str) -> Result<MaterialAmounts> {
    Ok(MaterialAmounts {
        dry_solids_kg: q(dry)?,
        water_kg: q(water)?,
        elements_kg: BTreeMap::from([(gold(), q(au)?)]),
    })
}
fn run() -> Result<()> {
    let arguments: Vec<_> = std::env::args().skip(1).collect();
    let gross_error = match arguments.as_slice() {
        [] => false,
        [arg] if arg == "--gross-error" => true,
        _ => return Err("usage: reconcile_flowsheet [--gross-error]".into()),
    };
    let scope = FlowsheetScope {
        boundary: "synthetic-mill-classifier/v1".into(),
        interval: ValidTime::new(0, 3_600_000_000_000)?,
    };
    let routes = vec![
        ("feed", StreamRoute::Feed { to: unit("mill")? }),
        (
            "mill-discharge",
            StreamRoute::Internal {
                from: unit("mill")?,
                to: unit("classifier")?,
                source_role: OutflowRole::Product,
            },
        ),
        (
            "recycle",
            StreamRoute::Internal {
                from: unit("classifier")?,
                to: unit("mill")?,
                source_role: OutflowRole::Other,
            },
        ),
        (
            "product",
            StreamRoute::Product {
                from: unit("classifier")?,
            },
        ),
        (
            "tails",
            StreamRoute::OtherOutflow {
                from: unit("classifier")?,
            },
        ),
    ];
    let streams = routes
        .into_iter()
        .map(|(id, route)| {
            Ok(StreamSpec {
                id: stream(id)?,
                route,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    let mut inputs = vec![bind(
        &scope,
        FlowsheetValue::Declaration {
            units: vec![
                UnitSpec {
                    id: unit("mill")?,
                    kind: UnitKind::Mill,
                },
                UnitSpec {
                    id: unit("classifier")?,
                    kind: UnitKind::Classifier,
                },
            ],
            streams,
            elements: vec![gold()],
            complete: true,
        },
    )];
    for (id, dry, water, au) in [
        ("feed", if gross_error { "120" } else { "101" }, "200", "1"),
        ("mill-discharge", "150", "300", "1.5"),
        ("recycle", "60", "120", "0.6"),
        ("product", "80", "150", "0.8"),
        ("tails", "5", "20", "0.05"),
    ] {
        inputs.push(bind(
            &scope,
            FlowsheetValue::Transfer {
                stream: stream(id)?,
                amounts: amounts(dry, water, au)?,
            },
        ));
    }
    for (id, kind, dry, water, au) in [
        ("mill", StocktakeKind::Opening, "20", "40", "0.2"),
        ("mill", StocktakeKind::Closing, "30", "60", "0.3"),
        ("classifier", StocktakeKind::Opening, "10", "20", "0.1"),
        ("classifier", StocktakeKind::Closing, "15", "30", "0.15"),
    ] {
        inputs.push(bind(
            &scope,
            FlowsheetValue::Stocktake {
                unit: unit(id)?,
                kind,
                amounts: amounts(dry, water, au)?,
            },
        ));
    }
    let observations = observation_order(&inputs)?;
    let n = observations.len();
    let mut covariance: Vec<_> = (0..n * n)
        .map(|i| {
            KilogramsSquared::new(if i / n != i % n {
                0.0
            } else if matches!(observations[i / n].channel, MaterialChannel::Element(_)) {
                1e-4
            } else {
                1.0
            })
        })
        .collect();
    let feed = observations
        .iter()
        .position(|key| {
            matches!(&key.location, ObservationLocation::Transfer(s) if s.as_str() == "feed")
                && key.channel == MaterialChannel::DrySolids
        })
        .ok_or("missing feed coordinate")?;
    let product = observations
        .iter()
        .position(|key| {
            matches!(&key.location, ObservationLocation::Transfer(s) if s.as_str() == "product")
                && key.channel == MaterialChannel::DrySolids
        })
        .ok_or("missing product coordinate")?;
    // Explicit +0.25 kg² correlation between the two weighing observations.
    covariance[feed * n + product] = KilogramsSquared::new(0.25);
    covariance[product * n + feed] = KilogramsSquared::new(0.25);
    let model = FlowsheetErrorModel {
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
    let outcome = reconcile_flowsheet(&inputs, &model)?;
    let claim = outcome.admitted_claim()?; // Failure emits no success-shaped stdout.
    let d = &claim.value;
    println!(
        "SYNTHETIC CONDITIONAL RECONCILIATION — observed data and declared errors are Assumed"
    );
    println!(
        "{} coordinates, {} independent unit/channel constraints, chi-square {:.12}",
        d.observations.len(),
        d.degrees_of_freedom,
        d.chi_square
    );
    for name in ["feed", "mill-discharge", "recycle", "product", "tails"] {
        let i = d
            .observations
            .iter()
            .position(|key| {
                matches!(&key.location, ObservationLocation::Transfer(s) if s.as_str() == name)
                    && key.channel == MaterialChannel::DrySolids
            })
            .ok_or("missing stream coordinate")?;
        println!(
            "{name}: observed {:.9} kg dry, fitted {:.9}, adjustment {:+.9}",
            d.observed_kg[i].value(),
            d.estimates_kg[i].value(),
            d.adjustments_kg[i].value()
        );
    }
    let opening = d
        .boundary
        .keys
        .iter()
        .position(|key| {
            key.role == BoundaryRole::OpeningInventory && key.channel == MaterialChannel::DrySolids
        })
        .ok_or("missing opening boundary")?;
    let k = d.boundary.keys.len();
    println!(
        "All opening dry stocks: {:.9} kg, joint variance {:.12} kg²",
        d.boundary.estimates_kg[opening].value(),
        d.boundary.posterior_covariance[opening * k + opening].value()
    );
    for row in &d.boundary.conservation {
        println!(
            "Boundary {:?}: exact observed residual {}/{} kg, fitted residual {:.3e} kg",
            row.channel,
            row.raw_residual_exact_kg.num(),
            row.raw_residual_exact_kg.den(),
            row.reconciled_residual_kg.value()
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
