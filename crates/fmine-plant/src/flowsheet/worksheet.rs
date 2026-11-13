//! DESIGN: pure source-bound worksheet admission for conservative flowsheets.
//! CLI adapters own bounded file reads, evaluation and response rendering.
//! Exact quantities, explicit covariance and original source locators are retained.
//! Determinism: DeterministicPerIsa

use std::collections::{BTreeMap, BTreeSet};

use fmine_claim::{AdditivityClass, Claim, Confidentiality, Representation, SpatialSupport};
use fmine_core::anchor::SourceAnchor;
use fmine_evidence::{EpistemicStatus, EvidenceClass, KnowledgeState, Refusal, RefusalCode};
use fmine_numerics::reconciliation::{MAX_CONSTRAINTS, MAX_OBSERVATIONS};
use fmine_registry::toml_subset::{TomlField, TomlRow, TomlValue, parse_subset};
use fmine_time::ValidTime;
use fmine_units::ExactQty;

use crate::flowsheet::{
    self, ElementKey, FlowsheetInput, FlowsheetScope, FlowsheetValue, MaterialAmounts, OutflowRole,
    StocktakeKind, StreamId, StreamRoute, StreamSpec, UnitId, UnitKind, UnitSpec,
    reconciliation::{
        self, FlowsheetErrorModel, MaterialChannel, ObservationKey, ObservationLocation,
        unmeasured::{
            self, UnmeasuredFlowModel,
            quantities::{self, UnmeasuredQuantityModel},
        },
    },
};
use crate::reconciliation::{Kilograms, KilogramsSquared};

const SCHEMA: &str = "fmine.plant-flowsheet.v1";
/// Explicit conserved-material basis required by all supported worksheet schemas.
pub const BASIS: &str = "kg/dry-matter+water/total-elements/all-phases/full-boundary";

/// The explicitly selected worksheet family; v2 additionally admits unmeasured endpoint stocks.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Mode {
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
}

type Fields<'a> = BTreeMap<&'a str, (&'a str, usize)>;
fn bad(line: usize, why: impl AsRef<str>) -> Refusal {
    Refusal::new(
        RefusalCode::InputMissingOrUnreadable506,
        format!("plant worksheet line {line}: {}", why.as_ref()),
    )
}
fn fields<'a>(
    input: &'a [TomlField],
    required: &[&str],
    optional: &[&str],
    line: usize,
) -> Result<Fields<'a>, Refusal> {
    let mut result = BTreeMap::new();
    for field in input {
        if !required.contains(&field.key.as_str()) && !optional.contains(&field.key.as_str()) {
            return Err(bad(field.line, format!("unsupported field {}", field.key)));
        }
        let TomlValue::Str(value) = &field.value else {
            return Err(bad(field.line, "values must be quoted strings"));
        };
        if value.is_empty()
            || value.trim() != value
            || value.len() > 4096
            || value.chars().any(char::is_control)
        {
            return Err(bad(
                field.line,
                "values must be nonempty trimmed text of at most 4096 bytes",
            ));
        }
        result.insert(field.key.as_str(), (value.as_str(), field.line));
    }
    for key in required {
        if !result.contains_key(key) {
            return Err(bad(line, format!("missing {key}")));
        }
    }
    Ok(result)
}
fn get<'a>(fields: &Fields<'a>, key: &str) -> Result<(&'a str, usize), Refusal> {
    fields
        .get(key)
        .copied()
        .ok_or_else(|| bad(1, format!("missing {key}")))
}
fn integer(value: (&str, usize)) -> Result<i64, Refusal> {
    value.0.parse().map_err(|_| {
        bad(
            value.1,
            "expected a signed 64-bit integer time in nanoseconds",
        )
    })
}
fn finite(value: (&str, usize)) -> Result<f64, Refusal> {
    let parsed = value
        .0
        .parse::<f64>()
        .map_err(|_| bad(value.1, "expected a finite number"))?;
    if !parsed.is_finite() {
        return Err(bad(value.1, "expected a finite number"));
    }
    let mantissa = value.0.split(['e', 'E']).next().unwrap_or(value.0);
    if parsed == 0.0 && mantissa.bytes().any(|byte| matches!(byte, b'1'..=b'9')) {
        return Err(Refusal::new(
            RefusalCode::NumericalCertificateFailed207,
            format!(
                "plant worksheet line {}: nonzero decimal underflows binary64",
                value.1
            ),
        ));
    }
    Ok(parsed)
}
fn quantity(value: (&str, usize)) -> Result<ExactQty, Refusal> {
    let fail = || {
        bad(
            value.1,
            "expected a plain exact decimal or n/d with a positive denominator",
        )
    };
    if let Some((n, d)) = value.0.split_once('/') {
        let n = n.parse::<i128>().map_err(|_| fail())?;
        let d = d.parse::<i128>().map_err(|_| fail())?;
        if d <= 0 {
            return Err(fail());
        }
        return ExactQty::new(n, d).map_err(|_| fail());
    }
    let digits = value.0.strip_prefix('-').unwrap_or(value.0);
    let (whole, fractional) = digits.split_once('.').unwrap_or((digits, ""));
    if whole.is_empty()
        || !whole
            .bytes()
            .chain(fractional.bytes())
            .all(|b| b.is_ascii_digit())
        || (digits.contains('.') && fractional.is_empty())
    {
        return Err(fail());
    }
    ExactQty::from_decimal_str(value.0).map_err(|_| fail())
}
fn declared<T>(
    value: T,
    tag: &str,
    metric: &str,
    additivity: AdditivityClass,
    source: &SourceAnchor,
    locator: String,
) -> Claim<T> {
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
        Confidentiality::internal(),
        vec![],
        vec![],
        vec![SourceAnchor::new(
            &source.artifact_path,
            source.artifact_sha256,
            locator,
        )],
    )
}

/// Exact submitted coordinate and its original token and worksheet line.
pub struct RawCoordinate {
    pub channel: MaterialChannel,
    pub value: ExactQty,
    pub raw: String,
    pub line: usize,
}
struct Group {
    time: ValidTime,
    coordinates: BTreeMap<MaterialChannel, RawCoordinate>,
}
/// Original coordinates retained beside the source-bound observation Claim.
pub struct Original {
    pub observation: String,
    pub coordinates: Vec<RawCoordinate>,
}
/// Parsed declarations and observations; no reconciliation or file read is performed here.
pub struct Worksheet {
    pub inputs: Vec<Claim<FlowsheetInput>>,
    pub originals: Vec<Original>,
    pub model: ErrorModel,
    pub model_basis: String,
}
/// Explicit measured or unmeasured error model, with its source Claim unchanged.
pub enum ErrorModel {
    Measured(Claim<FlowsheetErrorModel>),
    Unmeasured(Claim<UnmeasuredFlowModel>),
    Quantities(Claim<UnmeasuredQuantityModel>),
}

/// Canonical worksheet material-channel name.
#[must_use]
pub fn channel_name(channel: &MaterialChannel) -> &str {
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
/// Canonical worksheet observation-location name.
#[must_use]
pub fn location_name(location: &ObservationLocation) -> String {
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
/// Canonical covariance coordinate key, shared by admission and presentation.
#[must_use]
pub fn coordinate_name(key: &ObservationKey) -> String {
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

/// Parse the declared flowsheet without IO, preserving tokens, line locators and source Claims.
/// A v2 unmeasured schema updates the requested mode before any subsequent refusal.
pub fn parse(
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
