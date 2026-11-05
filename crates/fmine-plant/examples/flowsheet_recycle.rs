//! Constructed mill/classifier recycle with changing carrier and gold inventories.
//! Run: cargo run -p fmine-plant --example flowsheet_recycle
//! Add `-- --hidden-gain` to inject offsetting unit errors while leaving external
//! feeds, products and stocks unchanged; no successful output is then printed.

use std::collections::BTreeMap;
use std::error::Error;

use fmine_claim::{Claim, Confidentiality, Representation, SpatialSupport};
use fmine_core::{anchor::SourceAnchor, hash::sha256};
use fmine_evidence::{EpistemicStatus, EvidenceClass, KnowledgeState};
use fmine_plant::flowsheet::{
    ElementKey, FlowsheetInput, FlowsheetScope, FlowsheetValue, INPUT_METRIC, MaterialAmounts,
    MaterialBalance, NO_CLAIM, OutflowRole, StocktakeKind, StreamId, StreamRoute, StreamSpec,
    UnitId, UnitKind, UnitSpec, account_flowsheet,
};
use fmine_time::ValidTime;
use fmine_units::ExactQty;

// Constructed kg of dry matter, water and total Au, respectively.
const TRANSFER_ROWS: [(&str, [&str; 3]); 5] = [
    ("feed", ["100", "200", "1"]),
    ("mill-discharge", ["150", "300", "1.5"]),
    ("recycle", ["60", "120", "0.6"]),
    ("product", ["80", "150", "0.8"]),
    ("tails", ["5", "20", "0.05"]),
];
const STOCK_ROWS: [(&str, StocktakeKind, [&str; 3]); 4] = [
    ("mill", StocktakeKind::Opening, ["20", "40", "0.2"]),
    ("mill", StocktakeKind::Closing, ["30", "60", "0.3"]),
    ("classifier", StocktakeKind::Opening, ["10", "20", "0.1"]),
    ("classifier", StocktakeKind::Closing, ["15", "30", "0.15"]),
];

fn amounts(values: [&str; 3], element: &ElementKey) -> Result<MaterialAmounts, Box<dyn Error>> {
    Ok(MaterialAmounts {
        dry_solids_kg: ExactQty::from_decimal_str(values[0])?,
        water_kg: ExactQty::from_decimal_str(values[1])?,
        elements_kg: BTreeMap::from([(element.clone(), ExactQty::from_decimal_str(values[2])?)]),
    })
}

fn source(scope: &FlowsheetScope, value: FlowsheetValue, locator: String) -> Claim<FlowsheetInput> {
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
    let input = FlowsheetInput {
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
        additivity,
        EvidenceClass::Assumed,
        EpistemicStatus::KnownKnown,
        KnowledgeState::Known,
        Confidentiality::public(),
        vec![],
        vec![],
        vec![SourceAnchor::new(
            "crates/fmine-plant/examples/flowsheet_recycle.rs",
            sha256(include_bytes!("flowsheet_recycle.rs")).0,
            locator,
        )],
    )
}

fn inputs(
    hidden_gain: bool,
    element: &ElementKey,
) -> Result<Vec<Claim<FlowsheetInput>>, Box<dyn Error>> {
    let scope = FlowsheetScope {
        boundary: "synthetic-mill-classifier/v1".into(),
        interval: ValidTime::new(0, 3_600_000_000_000)?,
    };
    let mill = UnitId::new("mill")?;
    let classifier = UnitId::new("classifier")?;
    let declaration = FlowsheetValue::Declaration {
        units: vec![
            UnitSpec {
                id: mill.clone(),
                kind: UnitKind::Mill,
            },
            UnitSpec {
                id: classifier.clone(),
                kind: UnitKind::Classifier,
            },
        ],
        streams: vec![
            StreamSpec {
                id: StreamId::new("feed")?,
                route: StreamRoute::Feed { to: mill.clone() },
            },
            StreamSpec {
                id: StreamId::new("mill-discharge")?,
                route: StreamRoute::Internal {
                    from: mill.clone(),
                    to: classifier.clone(),
                    source_role: OutflowRole::Product,
                },
            },
            StreamSpec {
                id: StreamId::new("recycle")?,
                route: StreamRoute::Internal {
                    from: classifier.clone(),
                    to: mill,
                    source_role: OutflowRole::Other,
                },
            },
            StreamSpec {
                id: StreamId::new("product")?,
                route: StreamRoute::Product {
                    from: classifier.clone(),
                },
            },
            StreamSpec {
                id: StreamId::new("tails")?,
                route: StreamRoute::OtherOutflow { from: classifier },
            },
        ],
        elements: vec![element.clone()],
        complete: true,
    };
    let mut inputs = vec![source(
        &scope,
        declaration,
        "inputs: constructed complete topology and channel declarations".into(),
    )];
    for (row, (id, values)) in TRANSFER_ROWS.iter().enumerate() {
        let mut amounts = amounts(*values, element)?;
        if hidden_gain && *id == "mill-discharge" {
            amounts.dry_solids_kg = amounts
                .dry_solids_kg
                .checked_add(ExactQty::from_integer(1))?;
        }
        let value = FlowsheetValue::Transfer {
            stream: StreamId::new(*id)?,
            amounts,
        };
        inputs.push(source(
            &scope,
            value,
            format!("TRANSFER_ROWS[{row}]; injected hidden-gain mode={hidden_gain}"),
        ));
    }
    for (row, (id, kind, values)) in STOCK_ROWS.iter().enumerate() {
        let value = FlowsheetValue::Stocktake {
            unit: UnitId::new(*id)?,
            kind: *kind,
            amounts: amounts(*values, element)?,
        };
        inputs.push(source(&scope, value, format!("STOCK_ROWS[{row}]")));
    }
    Ok(inputs)
}

fn number(q: ExactQty) -> String {
    if q.den() == 1 {
        q.num().to_string()
    } else {
        format!("{}/{}", q.num(), q.den())
    }
}

fn display(name: &str, balance: &MaterialBalance, element: &ElementKey) {
    for (component, carrier) in [
        ("dry solids", &balance.dry_solids),
        ("water", &balance.water),
    ] {
        println!(
            "{name}/{component} | {} | {} | {} | {} | {} | {}",
            number(carrier.opening_kg),
            number(carrier.feed_kg),
            number(carrier.product_kg),
            number(carrier.other_outflow_kg),
            number(carrier.closing_kg),
            number(carrier.residual_kg)
        );
    }
    if let Some(receipt) = balance.elements.get(element) {
        let gold = receipt.summary();
        println!(
            "{name}/Au | {} | {} | {} | {} | {} | {}",
            number(gold.opening_inventory_kg),
            number(gold.feed_kg),
            number(gold.product_kg),
            number(gold.other_outflow_kg),
            number(gold.closing_inventory_kg),
            number(gold.residual_kg)
        );
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let arguments: Vec<_> = std::env::args().skip(1).collect();
    let hidden_gain = match arguments.as_slice() {
        [] => false,
        [flag] if flag == "--hidden-gain" => true,
        _ => return Err("usage: flowsheet_recycle [--hidden-gain]".into()),
    };
    let element = ElementKey {
        element: "Au".into(),
        measurand: "total-Au/all-phases".into(),
    };
    let receipt = account_flowsheet(&inputs(hidden_gain, &element)?)?;
    println!(
        "SYNTHETIC mill/classifier recycle with changing inventories; quantities are exact kg."
    );
    println!("boundary/component | opening | feed | product | other outflow | closing | residual");
    for (unit, balance) in &receipt.summary().units {
        display(unit.as_str(), balance, &element);
    }
    display("whole plant", &receipt.summary().boundary, &element);
    println!(
        "Internal transfers cancelled once at each endpoint: {}",
        receipt
            .summary()
            .internal_streams
            .iter()
            .map(StreamId::as_str)
            .collect::<Vec<_>>()
            .join(", ")
    );
    println!(
        "New dry feed is 100 kg; the mill receives 160 kg including 60 kg of declared recycle."
    );
    println!(
        "claim={} receipt={}",
        receipt.claim().claim_id,
        receipt.digest()
    );
    println!("{NO_CLAIM}");
    Ok(())
}
