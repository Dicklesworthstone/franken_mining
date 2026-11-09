//! DESIGN: source-bound plant networks over the joint R1 owners.
//! The worksheet declares extensive kg, complete topology and every covariance
//! pair. Only inference may explicitly declare unmeasured streams or endpoint
//! inventories. No independence, missing channel or omission is inferred.
//! Determinism: DeterministicPerIsa

mod inference;

use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

use fmine_claim::{AdditivityClass, Claim};
use fmine_core::{Digest32, anchor::SourceAnchor};
use fmine_evidence::{Refusal, RefusalCode};
use fmine_numerics::reconciliation::{MAX_CONSTRAINTS, MAX_OBSERVATIONS};
use fmine_plant::flowsheet::{
    self, ElementKey, FlowsheetInput, FlowsheetScope, FlowsheetValue, MaterialAmounts, OutflowRole,
    StocktakeKind, StreamId, StreamRoute, StreamSpec, UnitId, UnitKind, UnitSpec,
    reconciliation::{
        self, BoundaryRole, FlowsheetErrorModel, FlowsheetReconciliation,
        FlowsheetReconciliationOutcome, MaterialChannel, ObservationKey, ObservationLocation,
        unmeasured::{
            self, UnmeasuredFlowModel,
            quantities::{self, UnmeasuredQuantityModel},
        },
    },
};
use fmine_plant::reconciliation::{Kilograms, KilogramsSquared};
use fmine_registry::toml_subset::{TomlRow, TomlValue, parse_subset};
use fmine_time::ValidTime;
use fmine_units::ExactQty;

use super::{
    MAX_BYTES, bad, claim_json, declared, exact, fields, finite, get, integer, number, numbers,
    quantity, quoted,
};
use crate::{Envelope, NextAction, worksheet_input};

const SCHEMA: &str = "fmine.plant-flowsheet.v1";
const BASIS: &str = "kg/dry-matter+water/total-elements/all-phases/full-boundary";
const VERB: &str = "plant flowsheet";
const NO_CLAIM: &str = "Conditional joint Gaussian reconciliation of submitted complete conservative extensive kg observations. Sources and error policy remain Assumed, Internal and uncalibrated. No missing-stream inference, sensor-bias identification, assay-error propagation, fitted recovery, P754 compliance, ledger commit or operating authorization is supplied.";
pub(super) const INFERENCE_ROBOT_DOCS_JSON: &str = inference::ROBOT_DOCS_JSON;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Measured,
    Unmeasured,
    Quantities,
}
impl Mode {
    fn schema(self) -> &'static str {
        match self {
            Self::Measured => SCHEMA,
            Self::Unmeasured => "fmine.plant-unmeasured.v1",
            Self::Quantities => "fmine.plant-unmeasured.v2",
        }
    }
    fn result_schema(self) -> &'static str {
        match self {
            Self::Measured => "fmine.plant-flowsheet-result.v1",
            Self::Unmeasured => "fmine.plant-unmeasured-result.v1",
            Self::Quantities => "fmine.plant-unmeasured-result.v2",
        }
    }
    fn verb(self) -> &'static str {
        match self {
            Self::Measured => VERB,
            Self::Unmeasured | Self::Quantities => "plant infer",
        }
    }
    fn calculation(self) -> &'static str {
        match self {
            Self::Measured => "conditional_gaussian_reconciliation",
            Self::Unmeasured | Self::Quantities => "conditional_gaussian_inference",
        }
    }
}

pub(super) const ROBOT_DOCS_JSON: &str = concat!(
    "{\"schema\":\"fmine.plant-flowsheet.v1\",\"command\":\"plant flowsheet --input <worksheet.toml>\",",
    "\"input\":\"Regular UTF-8 TOML at most 1 MiB; all values quoted; exact submitted bytes are SHA-256 bound\",",
    "\"fields\":[\"schema\",\"boundary\",\"basis\",\"start_ns\",\"end_ns\",\"complete\",\"unbiased_gaussian_errors\",\"global_chi_square_limit\",\"error_model_basis\"],",
    "\"optional_fields\":[\"normalized_adjustment_limit\"],",
    "\"basis\":\"kg/dry-matter+water/total-elements/all-phases/full-boundary\",",
    "\"units\":\"[[unit]] id,kind; kind=mill,classifier,separator,mixer,storage\",",
    "\"streams\":\"[[stream]] id,kind; feed requires to, internal requires from,to,source_role=product|other, product or other_outflow requires from; irrelevant endpoint fields refuse\",",
    "\"elements\":\"[[element]] symbol,measurand; exactly one total all-phase measurand per element; elements overlap carriers\",",
    "\"coordinates\":\"[[coordinate]] observation,channel,kg,start_ns,end_ns; observation=transfer:<stream>|opening:<unit>|closing:<unit>; channel=dry|water|declared element symbol\",",
    "\"population\":\"complete=true and unbiased_gaussian_errors=true required; every stream and both stocks per unit need dry, water and every declared element, including explicit zeros; an internal stream is observed once\",",
    "\"times\":\"Quoted signed integer nanoseconds on one declared axis; transfer spans the full interval; opening/closing stocks are exact endpoint instants; all coordinates of one observation share that time\",",
    "\"quantities\":\"Quoted exact plain decimals or n/d with positive denominator; dry means all non-water matter including dissolved residue, water means H2O, elements are totals in all phases\",",
    "\"tolerances\":\"[[tolerance]] channel,kg; exactly one finite nonnegative kg allowance per channel\",",
    "\"covariance\":\"[[covariance]] left,right,kg2; keys are <observation>/<channel>; ALL unordered coordinate pairs including explicit zero cross terms required; full positive-definite kg2 matrix, never implicit independence\",",
    "\"limits\":\"128 observed coordinates and 64 independent unit/channel rows; positive finite rejection limits are caller-declared policy\",",
    "\"output\":\"Original exact coordinates and source Claims, complete graph, canonical matrix order, full model and fitted covariance, unit/outer residuals, chi-square; rejected candidates retain diagnostics and null admitted_claim\",",
    "\"evidence\":\"Assumed, Internal and uncalibrated; no P754 compliance or operating authorization\",",
    "\"example\":\"tests/plant/flowsheet_reconcile.toml\"}"
);

struct RawCoordinate {
    channel: MaterialChannel,
    value: ExactQty,
    raw: String,
    line: usize,
}
struct Group {
    time: ValidTime,
    coordinates: BTreeMap<MaterialChannel, RawCoordinate>,
}
struct Original {
    observation: String,
    coordinates: Vec<RawCoordinate>,
}
struct Worksheet {
    inputs: Vec<Claim<FlowsheetInput>>,
    originals: Vec<Original>,
    model: ErrorModel,
    model_basis: String,
}
enum ErrorModel {
    Measured(Claim<FlowsheetErrorModel>),
    Unmeasured(Claim<UnmeasuredFlowModel>),
    Quantities(Claim<UnmeasuredQuantityModel>),
}

struct RunOutput {
    envelope: Envelope,
    admitted: String,
    digest: String,
    result: String,
}

fn channel_name(channel: &MaterialChannel) -> &str {
    match channel {
        MaterialChannel::DrySolids => "dry",
        MaterialChannel::Water => "water",
        MaterialChannel::Element(element) => &element.element,
    }
}
fn channel(
    value: (&str, usize),
    elements: &BTreeMap<String, ElementKey>,
) -> Result<MaterialChannel, Refusal> {
    match value.0 {
        "dry" => Ok(MaterialChannel::DrySolids),
        "water" => Ok(MaterialChannel::Water),
        name => elements
            .get(name)
            .cloned()
            .map(MaterialChannel::Element)
            .ok_or_else(|| bad(value.1, format!("undeclared material channel {name}"))),
    }
}
fn location(value: (&str, usize)) -> Result<ObservationLocation, Refusal> {
    match value.0.split_once(':') {
        Some(("transfer", name)) => Ok(ObservationLocation::Transfer(StreamId::new(name)?)),
        Some(("opening", name)) => Ok(ObservationLocation::Stocktake {
            unit: UnitId::new(name)?,
            kind: StocktakeKind::Opening,
        }),
        Some(("closing", name)) => Ok(ObservationLocation::Stocktake {
            unit: UnitId::new(name)?,
            kind: StocktakeKind::Closing,
        }),
        _ => Err(bad(
            value.1,
            "observation must name transfer:<stream>, opening:<unit> or closing:<unit>",
        )),
    }
}
fn location_name(location: &ObservationLocation) -> String {
    match location {
        ObservationLocation::Transfer(stream) => format!("transfer:{}", stream.as_str()),
        ObservationLocation::Stocktake { unit, kind } => format!(
            "{}:{}",
            match kind {
                StocktakeKind::Opening => "opening",
                StocktakeKind::Closing => "closing",
            },
            unit.as_str()
        ),
    }
}
fn coordinate_name(key: &ObservationKey) -> String {
    format!(
        "{}/{}",
        location_name(&key.location),
        channel_name(&key.channel)
    )
}
fn unit_kind(value: (&str, usize)) -> Result<UnitKind, Refusal> {
    match value.0 {
        "mill" => Ok(UnitKind::Mill),
        "classifier" => Ok(UnitKind::Classifier),
        "separator" => Ok(UnitKind::Separator),
        "mixer" => Ok(UnitKind::Mixer),
        "storage" => Ok(UnitKind::Storage),
        _ => Err(bad(value.1, "unknown conservative unit kind")),
    }
}
fn stream_row(row: &TomlRow) -> Result<StreamSpec, Refusal> {
    let f = fields(
        &row.fields,
        &["id", "kind"],
        &["from", "to", "source_role"],
        row.line,
    )?;
    let kind = get(&f, "kind")?;
    let required: &[&str] = match kind.0 {
        "feed" => &["id", "kind", "to"],
        "internal" => &["id", "kind", "from", "to", "source_role"],
        "product" | "other_outflow" => &["id", "kind", "from"],
        _ => {
            return Err(bad(
                kind.1,
                "stream kind must be feed, internal, product or other_outflow",
            ));
        }
    };
    let f = fields(&row.fields, required, &[], row.line)?;
    let route = match kind.0 {
        "feed" => StreamRoute::Feed {
            to: UnitId::new(get(&f, "to")?.0)?,
        },
        "internal" => StreamRoute::Internal {
            from: UnitId::new(get(&f, "from")?.0)?,
            to: UnitId::new(get(&f, "to")?.0)?,
            source_role: match get(&f, "source_role")?.0 {
                "product" => OutflowRole::Product,
                "other" => OutflowRole::Other,
                _ => {
                    return Err(bad(
                        row.line,
                        "internal source_role must be product or other",
                    ));
                }
            },
        },
        "product" => StreamRoute::Product {
            from: UnitId::new(get(&f, "from")?.0)?,
        },
        _ => StreamRoute::OtherOutflow {
            from: UnitId::new(get(&f, "from")?.0)?,
        },
    };
    Ok(StreamSpec {
        id: StreamId::new(get(&f, "id")?.0)?,
        route,
    })
}

fn parse(
    text: &str,
    source: &SourceAnchor,
    requested_mode: &mut Mode,
) -> Result<Worksheet, Refusal> {
    let document = parse_subset(text).map_err(|e| bad(e.line, e.reason))?;
    if *requested_mode == Mode::Unmeasured
        && document.top_get("schema").is_some_and(|field| {
            matches!(&field.value, TomlValue::Str(schema) if schema == Mode::Quantities.schema())
        })
    {
        *requested_mode = Mode::Quantities;
    }
    let mode = *requested_mode;
    if document.rows.len() > MAX_OBSERVATIONS * (MAX_OBSERVATIONS + 1) / 2 + 5 * MAX_OBSERVATIONS {
        return Err(bad(
            1,
            "flowsheet worksheet exceeds the bounded row population",
        ));
    }
    let top = fields(
        &document.top,
        &[
            "schema",
            "boundary",
            "basis",
            "start_ns",
            "end_ns",
            "complete",
            "unbiased_gaussian_errors",
            "global_chi_square_limit",
            "error_model_basis",
        ],
        &["normalized_adjustment_limit"],
        1,
    )?;
    let schema = get(&top, "schema")?.0;
    if schema != mode.schema() {
        return Err(bad(1, format!("schema must be {}", mode.schema())));
    }
    if get(&top, "basis")?.0 != BASIS {
        return Err(Refusal::new(
            RefusalCode::UndeclaredUnitBasisOrTz107,
            format!("flowsheet basis must be {BASIS}"),
        ));
    }
    if get(&top, "complete")?.0 != "true" || get(&top, "unbiased_gaussian_errors")?.0 != "true" {
        return Err(bad(
            1,
            "complete and unbiased_gaussian_errors must both be explicitly true",
        ));
    }
    let scope = FlowsheetScope {
        boundary: get(&top, "boundary")?.0.into(),
        interval: ValidTime {
            start_ns: integer(get(&top, "start_ns")?)?,
            end_ns: integer(get(&top, "end_ns")?)?,
        },
    };
    let mut units = BTreeMap::new();
    let mut streams = BTreeMap::new();
    let mut elements = BTreeMap::new();
    let mut unmeasured = BTreeSet::new();
    let mut unmeasured_quantities = BTreeSet::new();
    for row in &document.rows {
        match row.table.as_str() {
            "unit" => {
                let f = fields(&row.fields, &["id", "kind"], &[], row.line)?;
                let value = UnitSpec {
                    id: UnitId::new(get(&f, "id")?.0)?,
                    kind: unit_kind(get(&f, "kind")?)?,
                };
                if units.insert(value.id.clone(), value).is_some() {
                    return Err(bad(row.line, "duplicate unit"));
                }
            }
            "stream" => {
                let value = stream_row(row)?;
                if streams.insert(value.id.clone(), value).is_some() {
                    return Err(bad(row.line, "duplicate stream"));
                }
            }
            "element" => {
                let f = fields(&row.fields, &["symbol", "measurand"], &[], row.line)?;
                let value = ElementKey {
                    element: get(&f, "symbol")?.0.into(),
                    measurand: get(&f, "measurand")?.0.into(),
                };
                if elements.insert(value.element.clone(), value).is_some() {
                    return Err(bad(row.line, "duplicate element identity"));
                }
            }
            "coordinate" | "covariance" | "tolerance" => {}
            "unmeasured" if mode != Mode::Measured => {
                let f = fields(&row.fields, &["stream"], &[], row.line)?;
                if !unmeasured.insert(StreamId::new(get(&f, "stream")?.0)?) {
                    return Err(bad(row.line, "duplicate unmeasured stream declaration"));
                }
            }
            "unmeasured_stocktake" if mode == Mode::Quantities => {
                let f = fields(&row.fields, &["unit", "kind"], &[], row.line)?;
                let kind = match get(&f, "kind")?.0 {
                    "opening" => StocktakeKind::Opening,
                    "closing" => StocktakeKind::Closing,
                    _ => {
                        return Err(bad(
                            row.line,
                            "unmeasured stocktake kind must be opening or closing",
                        ));
                    }
                };
                let location = ObservationLocation::Stocktake {
                    unit: UnitId::new(get(&f, "unit")?.0)?,
                    kind,
                };
                if !unmeasured_quantities.insert(location) {
                    return Err(bad(row.line, "duplicate unmeasured stocktake declaration"));
                }
            }
            _ => return Err(bad(row.line, format!("unsupported table {}", row.table))),
        }
    }
    unmeasured_quantities.extend(
        unmeasured
            .iter()
            .cloned()
            .map(ObservationLocation::Transfer),
    );
    if mode != Mode::Measured && unmeasured_quantities.is_empty() {
        return Err(Refusal::new(
            RefusalCode::NoClaimInDomain201,
            "plant infer requires at least one explicitly unmeasured stream or endpoint stocktake (stocktakes require v2)",
        ));
    }
    let width = elements.len() + 2;
    if (streams.len() + 2 * units.len()) > MAX_OBSERVATIONS / width
        || units.len() > MAX_CONSTRAINTS / width
    {
        return Err(bad(
            1,
            "flowsheet exceeds 128 coordinates or 64 independent unit/channel rows",
        ));
    }
    let declaration = FlowsheetInput {
        scope: scope.clone(),
        physical_time: scope.interval,
        value: FlowsheetValue::Declaration {
            units: units.into_values().collect(),
            streams: streams.into_values().collect(),
            elements: elements.values().cloned().collect(),
            complete: true,
        },
    };
    let tag = declaration.canonical_tag();
    let mut inputs = vec![declared(
        declaration,
        &tag,
        flowsheet::INPUT_METRIC,
        AdditivityClass::NonAdditive,
        source,
        format!(
            "complete scope line {}; all unit, stream and element declarations",
            get(&top, "complete")?.1
        ),
    )];
    let mut groups: BTreeMap<ObservationLocation, Group> = BTreeMap::new();
    let mut count = 0;
    for row in document.rows.iter().filter(|row| row.table == "coordinate") {
        count += 1;
        if count > MAX_OBSERVATIONS {
            return Err(bad(row.line, "at most 128 measured coordinates"));
        }
        let f = fields(
            &row.fields,
            &["observation", "channel", "kg", "start_ns", "end_ns"],
            &[],
            row.line,
        )?;
        let location = location(get(&f, "observation")?)?;
        let channel = channel(get(&f, "channel")?, &elements)?;
        let raw = get(&f, "kg")?;
        let time = ValidTime {
            start_ns: integer(get(&f, "start_ns")?)?,
            end_ns: integer(get(&f, "end_ns")?)?,
        };
        let group = groups.entry(location).or_insert_with(|| Group {
            time,
            coordinates: BTreeMap::new(),
        });
        if group.time != time {
            return Err(Refusal::new(
                RefusalCode::SupportMismatch102,
                "coordinates of one observation disagree on physical time",
            ));
        }
        if group
            .coordinates
            .insert(
                channel.clone(),
                RawCoordinate {
                    channel,
                    value: quantity(raw)?,
                    raw: raw.0.into(),
                    line: row.line,
                },
            )
            .is_some()
        {
            return Err(bad(row.line, "duplicate observation/channel coordinate"));
        }
    }
    let mut channels = vec![MaterialChannel::DrySolids, MaterialChannel::Water];
    channels.extend(elements.values().cloned().map(MaterialChannel::Element));
    let mut originals = Vec::with_capacity(groups.len());
    for (location, group) in groups {
        if !group.coordinates.keys().eq(channels.iter()) {
            return Err(bad(
                1,
                format!(
                    "{} requires dry, water and exactly every declared element, including zeros",
                    location_name(&location)
                ),
            ));
        }
        let amount = |key: &MaterialChannel| {
            group
                .coordinates
                .get(key)
                .map(|c| c.value)
                .ok_or_else(|| bad(1, "missing material coordinate"))
        };
        let amounts = MaterialAmounts {
            dry_solids_kg: amount(&MaterialChannel::DrySolids)?,
            water_kg: amount(&MaterialChannel::Water)?,
            elements_kg: elements
                .values()
                .map(|e| Ok((e.clone(), amount(&MaterialChannel::Element(e.clone()))?)))
                .collect::<Result<_, Refusal>>()?,
        };
        let observation = location_name(&location);
        let value = match location {
            ObservationLocation::Transfer(stream) => FlowsheetValue::Transfer { stream, amounts },
            ObservationLocation::Stocktake { unit, kind } => FlowsheetValue::Stocktake {
                unit,
                kind,
                amounts,
            },
        };
        let input = FlowsheetInput {
            scope: scope.clone(),
            physical_time: group.time,
            value,
        };
        let tag = input.canonical_tag();
        let claim = declared(
            input,
            &tag,
            flowsheet::INPUT_METRIC,
            AdditivityClass::ExtensiveAdditive,
            source,
            format!(
                "{observation}; coordinate rows {}",
                group
                    .coordinates
                    .values()
                    .map(|c| c.line.to_string())
                    .collect::<Vec<_>>()
                    .join(",")
            ),
        );
        originals.push(Original {
            observation,
            coordinates: group.coordinates.into_values().collect(),
        });
        inputs.push(claim);
    }
    let order = match mode {
        Mode::Measured => reconciliation::observation_order(&inputs)?,
        Mode::Unmeasured => unmeasured::observation_order(&inputs, &unmeasured)?,
        Mode::Quantities => {
            quantities::observation_order_for_quantities(&inputs, &unmeasured_quantities)?
        }
    };
    let n = order.len();
    let positions: BTreeMap<_, _> = order
        .iter()
        .enumerate()
        .map(|(i, key)| (coordinate_name(key), i))
        .collect();
    let mut covariance = vec![None; n * n];
    let mut tolerances = BTreeMap::new();
    for row in &document.rows {
        match row.table.as_str() {
            "covariance" => {
                let f = fields(&row.fields, &["left", "right", "kg2"], &[], row.line)?;
                let index = |name: &str| -> Result<usize, Refusal> {
                    let key = get(&f, name)?;
                    positions.get(key.0).copied().ok_or_else(|| {
                        bad(key.1, format!("unknown covariance coordinate {}", key.0))
                    })
                };
                let (i, j) = (index("left")?, index("right")?);
                if covariance[i * n + j].is_some() {
                    return Err(bad(
                        row.line,
                        "duplicate covariance pair, including reversed pairs",
                    ));
                }
                let value = finite(get(&f, "kg2")?)?;
                covariance[i * n + j] = Some(KilogramsSquared::new(value));
                covariance[j * n + i] = Some(KilogramsSquared::new(value));
            }
            "tolerance" => {
                let f = fields(&row.fields, &["channel", "kg"], &[], row.line)?;
                let channel = channel(get(&f, "channel")?, &elements)?;
                if tolerances
                    .insert(channel, Kilograms::new(finite(get(&f, "kg")?)?))
                    .is_some()
                {
                    return Err(bad(row.line, "duplicate channel tolerance"));
                }
            }
            _ => {}
        }
    }
    if !tolerances.keys().eq(channels.iter()) {
        return Err(bad(
            1,
            "exactly one declared kg tolerance per material channel is required",
        ));
    }
    let covariance = covariance.into_iter().map(|value| value.ok_or_else(|| bad(1, "every unordered covariance pair is required, including explicit zero cross terms"))).collect::<Result<_, _>>()?;
    let global_chi_square_limit = finite(get(&top, "global_chi_square_limit")?)?;
    let normalized_adjustment_limit = top
        .get("normalized_adjustment_limit")
        .copied()
        .map(finite)
        .transpose()?;
    let basis = get(&top, "error_model_basis")?;
    let locator = format!(
        "error_model_basis line {}; all explicit covariance pairs, channel tolerances and rejection limits",
        basis.1
    );
    let model = match mode {
        Mode::Measured => {
            let value = FlowsheetErrorModel {
                scope,
                declaration_id: inputs[0].claim_id,
                observations: order,
                covariance,
                unbiased_gaussian_errors: true,
                global_chi_square_limit,
                normalized_adjustment_limit,
                closure_tolerances_kg: tolerances,
            };
            let tag = value.canonical_tag();
            ErrorModel::Measured(declared(
                value,
                &tag,
                reconciliation::ERROR_MODEL_METRIC,
                AdditivityClass::NonAdditive,
                source,
                locator,
            ))
        }
        Mode::Unmeasured => {
            let value = UnmeasuredFlowModel {
                scope,
                declaration_id: inputs[0].claim_id,
                observations: order,
                covariance,
                unmeasured_streams: unmeasured,
                unbiased_gaussian_errors: true,
                global_chi_square_limit,
                normalized_adjustment_limit,
                closure_tolerances_kg: tolerances,
            };
            let tag = value.canonical_tag();
            ErrorModel::Unmeasured(declared(
                value,
                &tag,
                unmeasured::ERROR_MODEL_METRIC,
                AdditivityClass::NonAdditive,
                source,
                locator,
            ))
        }
        Mode::Quantities => {
            let value = UnmeasuredQuantityModel {
                scope,
                declaration_id: inputs[0].claim_id,
                observations: order,
                covariance,
                unmeasured: unmeasured_quantities,
                unbiased_gaussian_errors: true,
                global_chi_square_limit,
                normalized_adjustment_limit,
                closure_tolerances_kg: tolerances,
            };
            let tag = value.canonical_tag();
            ErrorModel::Quantities(declared(
                value,
                &tag,
                quantities::ERROR_MODEL_METRIC,
                AdditivityClass::NonAdditive,
                source,
                locator,
            ))
        }
    };
    Ok(Worksheet {
        inputs,
        originals,
        model,
        model_basis: basis.0.into(),
    })
}

fn order_json(order: &[ObservationKey]) -> String {
    format!(
        "[{}]",
        order
            .iter()
            .map(|key| format!(
                "{{\"key\":{},\"observation\":{},\"channel\":{},\"source_claim_id\":{}}}",
                quoted(&coordinate_name(key)),
                quoted(&location_name(&key.location)),
                quoted(channel_name(&key.channel)),
                quoted(&key.source.to_string())
            ))
            .collect::<Vec<_>>()
            .join(",")
    )
}
fn originals_json(originals: &[Original], inputs: &[Claim<FlowsheetInput>]) -> String {
    format!("[{}]", originals.iter().zip(&inputs[1..]).map(|(original, claim)| {
        let coordinates = original.coordinates.iter().map(|c| format!(
            "{{\"channel\":{},\"raw_kg\":{},\"kg\":{},\"line\":{}}}",
            quoted(channel_name(&c.channel)), quoted(&c.raw), exact(c.value), c.line)).collect::<Vec<_>>().join(",");
        format!("{{\"observation\":{},\"source_claim_id\":{},\"start_ns\":\"{}\",\"end_ns\":\"{}\",\"coordinates\":[{}],\"claim\":{}}}",
            quoted(&original.observation), quoted(&claim.claim_id.to_string()), claim.value.physical_time.start_ns,
            claim.value.physical_time.end_ns, coordinates, claim_json(claim))
    }).collect::<Vec<_>>().join(","))
}
fn graph_json(declaration: &Claim<FlowsheetInput>) -> String {
    let FlowsheetValue::Declaration {
        units,
        streams,
        elements,
        complete,
    } = &declaration.value.value
    else {
        return "null".into();
    };
    let units = units
        .iter()
        .map(|u| {
            format!(
                "{{\"id\":{},\"kind\":{}}}",
                quoted(u.id.as_str()),
                quoted(match u.kind {
                    UnitKind::Mill => "mill",
                    UnitKind::Classifier => "classifier",
                    UnitKind::Separator => "separator",
                    UnitKind::Mixer => "mixer",
                    UnitKind::Storage => "storage",
                })
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let streams = streams
        .iter()
        .map(|s| {
            let route = match &s.route {
                StreamRoute::Feed { to } => {
                    format!("\"kind\":\"feed\",\"to\":{}", quoted(to.as_str()))
                }
                StreamRoute::Product { from } => {
                    format!("\"kind\":\"product\",\"from\":{}", quoted(from.as_str()))
                }
                StreamRoute::OtherOutflow { from } => format!(
                    "\"kind\":\"other_outflow\",\"from\":{}",
                    quoted(from.as_str())
                ),
                StreamRoute::Internal {
                    from,
                    to,
                    source_role,
                } => format!(
                    "\"kind\":\"internal\",\"from\":{},\"to\":{},\"source_role\":{}",
                    quoted(from.as_str()),
                    quoted(to.as_str()),
                    quoted(match source_role {
                        OutflowRole::Product => "product",
                        OutflowRole::Other => "other",
                    })
                ),
            };
            format!("{{\"id\":{},{route}}}", quoted(s.id.as_str()))
        })
        .collect::<Vec<_>>()
        .join(",");
    let elements = elements
        .iter()
        .map(|e| {
            format!(
                "{{\"symbol\":{},\"measurand\":{}}}",
                quoted(&e.element),
                quoted(&e.measurand)
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"complete\":{complete},\"units\":[{units}],\"streams\":[{streams}],\"elements\":[{elements}],\"claim\":{}}}",
        claim_json(declaration)
    )
}
fn boundary_role(role: BoundaryRole) -> &'static str {
    match role {
        BoundaryRole::OpeningInventory => "opening",
        BoundaryRole::Feed => "feed",
        BoundaryRole::Product => "product",
        BoundaryRole::OtherOutflow => "other_outflow",
        BoundaryRole::ClosingInventory => "closing",
    }
}
fn residual_json(
    channel: &MaterialChannel,
    raw: ExactQty,
    observed: Kilograms,
    fitted: Kilograms,
) -> String {
    format!(
        "\"channel\":{},\"raw_residual_exact_kg\":{},\"raw_residual_kg\":{},\"reconciled_residual_kg\":{}",
        quoted(channel_name(channel)),
        exact(raw),
        number(observed.value()),
        number(fitted.value())
    )
}
fn diagnostics(value: &FlowsheetReconciliation) -> String {
    let units = value
        .conservation
        .iter()
        .map(|r| {
            format!(
                "{{\"unit\":{},{}}}",
                quoted(r.unit.as_str()),
                residual_json(
                    &r.channel,
                    r.raw_residual_exact_kg,
                    r.raw_residual_kg,
                    r.reconciled_residual_kg
                )
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let boundary_order = value
        .boundary
        .keys
        .iter()
        .map(|key| {
            format!(
                "{{\"role\":{},\"channel\":{}}}",
                quoted(boundary_role(key.role)),
                quoted(channel_name(&key.channel))
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let boundary_residuals = value
        .boundary
        .conservation
        .iter()
        .map(|r| {
            format!(
                "{{{}}}",
                residual_json(
                    &r.channel,
                    r.raw_residual_exact_kg,
                    r.raw_residual_kg,
                    r.reconciled_residual_kg
                )
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let normalized = value
        .normalized_adjustments
        .iter()
        .map(|v| v.map_or_else(|| "null".into(), number))
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"observed_kg\":{},\"estimates_kg\":{},\"adjustments_kg\":{},\"posterior_covariance_kg2\":{},\"adjustment_covariance_kg2\":{},\"normalized_adjustments\":[{normalized}],\"unit_conservation\":[{units}],\"boundary\":{{\"order\":[{boundary_order}],\"observed_kg\":{},\"estimates_kg\":{},\"posterior_covariance_kg2\":{},\"conservation\":[{boundary_residuals}]}},\"chi_square\":{},\"degrees_of_freedom\":{}}}",
        numbers(value.observed_kg.iter().map(|q| q.value())),
        numbers(value.estimates_kg.iter().map(|q| q.value())),
        numbers(value.adjustments_kg.iter().map(|q| q.value())),
        numbers(value.posterior_covariance.iter().map(|q| q.value())),
        numbers(value.adjustment_covariance.iter().map(|q| q.value())),
        numbers(value.boundary.observed_kg.iter().map(|q| q.value())),
        numbers(value.boundary.estimates_kg.iter().map(|q| q.value())),
        numbers(
            value
                .boundary
                .posterior_covariance
                .iter()
                .map(|q| q.value())
        ),
        number(value.chi_square),
        value.degrees_of_freedom
    )
}
fn actions() -> Vec<NextAction> {
    vec![NextAction::new(
        "fmine robot-docs",
        "inspect complete graph, observation support, every covariance pair and channel tolerance",
    )]
}
fn finish(mut envelope: Envelope, mode: Mode) -> Envelope {
    envelope.no_claim = Some(match mode {
        Mode::Measured => NO_CLAIM,
        Mode::Unmeasured | Mode::Quantities => inference::NO_CLAIM,
    });
    envelope
}

fn measured_result(
    inputs: &[Claim<FlowsheetInput>],
    model: &Claim<FlowsheetErrorModel>,
) -> RunOutput {
    let (envelope, admitted, digest, result) =
        match reconciliation::reconcile_flowsheet(inputs, model) {
            Ok(FlowsheetReconciliationOutcome::Admitted(receipt)) => (
                Envelope::ok(VERB, String::new(), vec![]),
                claim_json(receipt.claim()),
                quoted(receipt.digest()),
                diagnostics(&receipt.claim().value),
            ),
            Ok(FlowsheetReconciliationOutcome::Rejected(rejected)) => (
                Envelope::refused(VERB, rejected.refusal().clone(), actions()),
                "null".into(),
                quoted(rejected.digest()),
                diagnostics(rejected.diagnostics()),
            ),
            Err(refusal) => (
                Envelope::refused(VERB, refusal, actions()),
                "null".into(),
                "null".into(),
                "null".into(),
            ),
        };
    RunOutput {
        envelope,
        admitted,
        digest,
        result,
    }
}

fn policy_json(
    covariance: &[KilogramsSquared],
    global_limit: f64,
    adjustment_limit: Option<f64>,
    closure_tolerances: &BTreeMap<MaterialChannel, Kilograms>,
) -> String {
    let tolerances = closure_tolerances
        .iter()
        .map(|(channel, q)| {
            format!(
                "{{\"channel\":{},\"kg\":{}}}",
                quoted(channel_name(channel)),
                number(q.value())
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "\"covariance_kg2\":{},\"unbiased_gaussian_errors\":true,\"global_chi_square_limit\":{},\"normalized_adjustment_limit\":{},\"closure_tolerances_kg\":[{}]",
        numbers(covariance.iter().map(|q| q.value())),
        number(global_limit),
        adjustment_limit.map_or_else(|| "null".into(), number),
        tolerances
    )
}

pub(super) fn execute(input: &Path, root: &Path) -> Envelope {
    execute_mode(input, root, Mode::Measured)
}

pub(super) fn execute_unmeasured(input: &Path, root: &Path) -> Envelope {
    execute_mode(input, root, Mode::Unmeasured)
}

fn execute_mode(input: &Path, root: &Path, mut mode: Mode) -> Envelope {
    let schema = quoted(mode.result_schema());
    let (text, source) = match worksheet_input::read(
        &root.join(input),
        "plant flowsheet worksheet",
        MAX_BYTES,
    ) {
        Ok(value) => value,
        Err(refusal) => {
            let mut envelope = Envelope::refused(mode.verb(), refusal, actions());
            envelope.payload_json = format!(
                "{{\"schema\":{schema},\"admitted_claim\":null,\"receipt_digest\":null,\"result\":null}}"
            );
            return finish(envelope, mode);
        }
    };
    let source_json = format!(
        "{{\"path\":{},\"sha256\":{},\"bytes\":{}}}",
        quoted(&source.artifact_path),
        quoted(&Digest32(source.artifact_sha256).to_hex()),
        text.len()
    );
    let worksheet = parse(&text, &source, &mut mode);
    let schema = quoted(mode.result_schema());
    let worksheet = match worksheet {
        Ok(value) => value,
        Err(refusal) => {
            let mut envelope = Envelope::refused(mode.verb(), refusal, actions());
            envelope.payload_json = format!(
                "{{\"schema\":{schema},\"source\":{source_json},\"admitted_claim\":null,\"receipt_digest\":null,\"result\":null}}"
            );
            return finish(envelope, mode);
        }
    };
    let (output, model_json, coordinate_order) = match &worksheet.model {
        ErrorModel::Measured(model) => {
            let value = &model.value;
            let model_json = format!(
                "{{\"claim\":{},\"basis\":{},\"declaration_id\":{},{}}}",
                claim_json(model),
                quoted(&worksheet.model_basis),
                quoted(&value.declaration_id.to_string()),
                policy_json(
                    &value.covariance,
                    value.global_chi_square_limit,
                    value.normalized_adjustment_limit,
                    &value.closure_tolerances_kg
                )
            );
            (
                measured_result(&worksheet.inputs, model),
                model_json,
                order_json(&value.observations),
            )
        }
        ErrorModel::Unmeasured(model) => (
            inference::run(&worksheet.inputs, model),
            inference::model_json(model, &worksheet.model_basis),
            inference::order_json(&model.value),
        ),
        ErrorModel::Quantities(model) => (
            inference::run_quantities(&worksheet.inputs, model),
            inference::quantity_model_json(model, &worksheet.model_basis),
            inference::quantity_order_json(&model.value),
        ),
    };
    let RunOutput {
        mut envelope,
        admitted,
        digest,
        result,
    } = output;
    let scope = &worksheet.inputs[0].value.scope;
    let scope = format!(
        "{{\"boundary\":{},\"basis\":{},\"start_ns\":\"{}\",\"end_ns\":\"{}\"}}",
        quoted(&scope.boundary),
        quoted(BASIS),
        scope.interval.start_ns,
        scope.interval.end_ns
    );
    envelope.payload_json = format!(
        "{{\"schema\":{schema},\"mode\":{},\"source\":{source_json},\"scope\":{scope},\"graph\":{},\"observations\":{},\"coordinate_order\":{coordinate_order},\"covariance_layout\":\"row_major\",\"error_model\":{model_json},\"admitted_claim\":{admitted},\"receipt_digest\":{digest},\"result\":{result}}}",
        quoted(mode.calculation()),
        graph_json(&worksheet.inputs[0]),
        originals_json(&worksheet.originals, &worksheet.inputs),
    );
    finish(envelope, mode)
}
