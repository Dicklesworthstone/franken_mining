//! DESIGN: source-bound raw lots to exact stream totals and first-order covariance.
//! Complete population, support, units and every raw correlation are declarations.
//! All physical arithmetic and model admission belong to the existing plant owner.
//! Determinism: Exact nominal quantities; DeterministicPerIsa covariance.

use std::{collections::BTreeMap, path::Path};

use fmine_claim::{Claim, ClaimId, Confidentiality, Representation, SpatialSupport};
use fmine_core::{Digest32, anchor::SourceAnchor};
use fmine_evidence::{EpistemicStatus, EvidenceClass, KnowledgeState, Refusal, RefusalCode};
use fmine_numerics::reconciliation::MAX_OBSERVATIONS;
use fmine_plant::{
    flowsheet::{ElementKey, FlowsheetScope, StreamId},
    raw_measurements::{
        self, ProjectedChannel, RawTransferErrorModel, RawTransferInput, RawTransferValue,
        StandardUncertainty, TransferProjection,
    },
    reconciliation::Kilograms,
};
use fmine_registry::toml_subset::parse_subset;
use fmine_time::ValidTime;

use super::{
    MAX_BYTES, bad, claim_json, declared, exact, fields, finite, get, integer, number, numbers,
    quantity, quoted,
};
use crate::{Envelope, NextAction, worksheet_input};

const SCHEMA: &str = "fmine.plant-transfer.v1";
const VERB: &str = "plant transfer";
const MOISTURE_BASIS: &str = "water_over_wet_mass";
const ASSAY_BASIS: &str = "total_element/all_non_water_dry_matter";
const NO_CLAIM: &str = "Conditional raw-lot projection: exact nominal products and ratios, separately declared first-order covariance. Sources, support and error model remain Assumed, Internal and uncalibrated. No transformed Gaussian law, nonlinear confidence interval, sampling coverage, recovery estimate, reconciliation, P754 qualification or operating authorization is supplied.";

pub(super) const ROBOT_DOCS_JSON: &str = concat!(
    "{\"schema\":\"fmine.plant-transfer.v1\",\"command\":\"plant transfer --input <worksheet.toml>\",",
    "\"input\":\"Regular UTF-8 TOML at most 1 MiB; all values quoted; exact submitted bytes are SHA-256 bound\",",
    "\"fields\":[\"schema\",\"boundary\",\"stream\",\"dry_basis\",\"start_ns\",\"end_ns\",\"complete\",\"nonoverlapping_lots\",\"whole_lot_support\",\"moisture_is_h2o\",\"moisture_basis\",\"assay_basis\",\"first_order_approximation_accepted\",\"error_model_basis\"],",
    "\"bases\":\"moisture_basis=water_over_wet_mass; assay_basis=total_element/all_non_water_dry_matter; dry_basis names the matched moisture and assay dry state\",",
    "\"population\":\"complete, nonoverlapping_lots, whole_lot_support and moisture_is_h2o must be explicitly true for admission; every lot needs wet mass, moisture and every declared assay\",",
    "\"lots\":\"[[lot]] id; lots are explicitly declared physical partitions, not inferred from rows\",",
    "\"elements\":\"[[element]] symbol,measurand; total elemental assays of all non-water matter; H/O require a water-composition model and refuse\",",
    "\"measurements\":\"[[measurement]] lot,channel,value,unit,start_ns,end_ns,standard_uncertainty,uncertainty_unit; channel=wet_mass|moisture|declared element symbol\",",
    "\"units\":\"Both value and standard uncertainty must explicitly use kg for wet_mass and mass_fraction otherwise; no percent, tonne or g/t conversion is inferred\",",
    "\"support\":\"whole_lot_support=true required; measurements are PlantLot Claims; every observation of a lot has identical physical support within the full reporting interval\",",
    "\"quantities\":\"Nominal values are exact plain decimals or n/d; uncertainties and correlations are finite binary64 parameters; nonzero underflow refuses; all raw numeric tokens are retained\",",
    "\"correlations\":\"[[correlation]] left,right,value; keys are <lot>/<channel>; ALL unordered pairs including diagonal ones and zero cross terms are explicit; unit diagonal, symmetric, positive-definite full raw correlation\",",
    "\"approximation\":\"first_order_approximation_accepted=true is mandatory; covariance does not establish a Gaussian output law, coverage or a WLS model\",",
    "\"limits\":\"128 raw measured coordinates; nonnegative standard uncertainties including explicit zeros\",",
    "\"output\":\"Original tokens, source Claims and raw model; exact extensive kg and ratios of summed metal to summed dry matter; canonical covariance order, full first-order kg2 covariance, receipt and parent-linked nominal transfer Claim\",",
    "\"evidence\":\"Assumed, Internal and uncalibrated; no automatic reconciliation or operating authority\",",
    "\"example\":\"tests/plant/raw_transfer.toml\"}"
);

struct Observation {
    key: String,
    claim: Claim<RawTransferInput>,
    raw_value: String,
    unit: &'static str,
    raw_start: String,
    raw_end: String,
    raw_uncertainty: String,
    uncertainty: StandardUncertainty,
    line: usize,
}
struct Correlation {
    left: String,
    right: String,
    raw_value: String,
    value: f64,
    line: usize,
}
struct Worksheet {
    declaration: Claim<RawTransferInput>,
    observations: Vec<Observation>,
    correlations: Vec<Correlation>,
    correlation: Vec<f64>,
    model_basis: String,
    model_line: usize,
    first_order: bool,
    raw_start: String,
    raw_end: String,
}

fn boolean(value: (&str, usize)) -> Result<bool, Refusal> {
    match value.0 {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => Err(bad(value.1, "expected quoted true or false")),
    }
}
fn bind(
    input: RawTransferInput,
    source: &SourceAnchor,
    locator: String,
) -> Claim<RawTransferInput> {
    let support = if matches!(input.value, RawTransferValue::Declaration { .. }) {
        SpatialSupport::PeriodAggregate
    } else {
        SpatialSupport::PlantLot
    };
    let tag = input.canonical_tag();
    let additivity = input.value.additivity();
    Claim::new(
        raw_measurements::INPUT_METRIC,
        input,
        &tag,
        Representation::Deterministic,
        support,
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
fn parse(text: &str, source: &SourceAnchor) -> Result<Worksheet, Refusal> {
    let document = parse_subset(text).map_err(|e| bad(e.line, e.reason))?;
    if document.rows.len() > MAX_OBSERVATIONS * (MAX_OBSERVATIONS + 1) / 2 + 3 * MAX_OBSERVATIONS {
        return Err(bad(
            1,
            "raw-transfer worksheet exceeds the bounded row population",
        ));
    }
    let top = fields(
        &document.top,
        &[
            "schema",
            "boundary",
            "stream",
            "dry_basis",
            "start_ns",
            "end_ns",
            "complete",
            "nonoverlapping_lots",
            "whole_lot_support",
            "moisture_is_h2o",
            "moisture_basis",
            "assay_basis",
            "first_order_approximation_accepted",
            "error_model_basis",
        ],
        &[],
        1,
    )?;
    if get(&top, "schema")?.0 != SCHEMA {
        return Err(bad(1, format!("schema must be {SCHEMA}")));
    }
    if get(&top, "moisture_basis")?.0 != MOISTURE_BASIS
        || get(&top, "assay_basis")?.0 != ASSAY_BASIS
    {
        return Err(Refusal::new(
            RefusalCode::BasisMismatch105,
            "declare water/wet-mass moisture and total-element assays over all non-water dry matter",
        ));
    }
    if !boolean(get(&top, "whole_lot_support")?)? {
        return Err(Refusal::new(
            RefusalCode::NoClaimInDomain201,
            "explicit whole_lot_support=true is required; no sample-to-lot extrapolation is inferred",
        ));
    }
    let start = get(&top, "start_ns")?;
    let end = get(&top, "end_ns")?;
    let scope = FlowsheetScope {
        boundary: get(&top, "boundary")?.0.into(),
        interval: ValidTime {
            start_ns: integer(start)?,
            end_ns: integer(end)?,
        },
    };
    let stream = StreamId::new(get(&top, "stream")?.0)?;
    let dry_basis = get(&top, "dry_basis")?.0;
    let mut lots = BTreeMap::new();
    let mut elements = BTreeMap::new();
    for row in &document.rows {
        match row.table.as_str() {
            "lot" => {
                let f = fields(&row.fields, &["id"], &[], row.line)?;
                if lots.insert(get(&f, "id")?.0.to_owned(), row.line).is_some() {
                    return Err(bad(row.line, "duplicate declared lot"));
                }
            }
            "element" => {
                let f = fields(&row.fields, &["symbol", "measurand"], &[], row.line)?;
                let element = ElementKey {
                    element: get(&f, "symbol")?.0.into(),
                    measurand: get(&f, "measurand")?.0.into(),
                };
                if elements.insert(element.element.clone(), element).is_some() {
                    return Err(bad(row.line, "duplicate declared elemental identity"));
                }
            }
            "measurement" | "correlation" => {}
            _ => return Err(bad(row.line, format!("unsupported table {}", row.table))),
        }
    }
    if lots.len() > MAX_OBSERVATIONS / (elements.len() + 2) {
        return Err(bad(1, "at most 128 declared raw measurement coordinates"));
    }
    let declaration = bind(
        RawTransferInput {
            scope: scope.clone(),
            stream: stream.clone(),
            dry_basis: dry_basis.into(),
            physical_time: scope.interval,
            value: RawTransferValue::Declaration {
                lot_ids: lots.keys().cloned().collect(),
                elements: elements.values().cloned().collect(),
                complete: boolean(get(&top, "complete")?)?,
                nonoverlapping_lots: boolean(get(&top, "nonoverlapping_lots")?)?,
                moisture_is_h2o: boolean(get(&top, "moisture_is_h2o")?)?,
                assays_are_total_elemental: true,
            },
        },
        source,
        format!(
            "complete population line {}; all lot/element rows and physical-basis declarations",
            get(&top, "complete")?.1
        ),
    );
    let mut observations = Vec::new();
    let mut positions = BTreeMap::new();
    for row in document.rows.iter().filter(|r| r.table == "measurement") {
        if observations.len() == MAX_OBSERVATIONS {
            return Err(bad(row.line, "at most 128 raw measurement coordinates"));
        }
        let f = fields(
            &row.fields,
            &[
                "lot",
                "channel",
                "value",
                "unit",
                "start_ns",
                "end_ns",
                "standard_uncertainty",
                "uncertainty_unit",
            ],
            &[],
            row.line,
        )?;
        let lot = get(&f, "lot")?.0;
        if !lots.contains_key(lot) {
            return Err(bad(row.line, "measurement names an undeclared lot"));
        }
        let channel = get(&f, "channel")?;
        let raw_value = get(&f, "value")?;
        let q = quantity(raw_value)?;
        let (value, unit) = match channel.0 {
            "wet_mass" => (
                RawTransferValue::WetMass {
                    lot: lot.into(),
                    kg: q,
                },
                "kg",
            ),
            "moisture" => (
                RawTransferValue::Moisture {
                    lot: lot.into(),
                    wet_mass_fraction: q,
                },
                "mass_fraction",
            ),
            symbol => (
                RawTransferValue::DryAssay {
                    lot: lot.into(),
                    element: elements
                        .get(symbol)
                        .cloned()
                        .ok_or_else(|| bad(channel.1, "undeclared assay element"))?,
                    dry_mass_fraction: q,
                },
                "mass_fraction",
            ),
        };
        if get(&f, "unit")?.0 != unit || get(&f, "uncertainty_unit")?.0 != unit {
            return Err(Refusal::new(
                RefusalCode::UndeclaredUnitBasisOrTz107,
                format!(
                    "plant transfer line {}: both value and standard uncertainty for {} require explicit {} units",
                    row.line, channel.0, unit
                ),
            ));
        }
        let raw_uncertainty = get(&f, "standard_uncertainty")?;
        let sigma = finite(raw_uncertainty)?;
        let uncertainty = if unit == "kg" {
            StandardUncertainty::MassKg(Kilograms::new(sigma))
        } else {
            StandardUncertainty::Fraction(sigma)
        };
        let start = get(&f, "start_ns")?;
        let end = get(&f, "end_ns")?;
        let key = format!("{lot}/{}", channel.0);
        if positions.insert(key.clone(), observations.len()).is_some() {
            return Err(bad(row.line, "duplicate lot/channel measurement"));
        }
        let input = RawTransferInput {
            scope: scope.clone(),
            stream: stream.clone(),
            dry_basis: dry_basis.into(),
            physical_time: ValidTime {
                start_ns: integer(start)?,
                end_ns: integer(end)?,
            },
            value,
        };
        observations.push(Observation { key, claim: bind(input, source,
            format!("measurement row line {}; whole-lot support and matched dry-state basis declarations", row.line)),
            raw_value: raw_value.0.into(), unit, raw_start: start.0.into(), raw_end: end.0.into(),
            raw_uncertainty: raw_uncertainty.0.into(), uncertainty, line: row.line });
    }
    let n = observations.len();
    let mut correlation = vec![None; n * n];
    let mut correlations = Vec::new();
    for row in document.rows.iter().filter(|r| r.table == "correlation") {
        let f = fields(&row.fields, &["left", "right", "value"], &[], row.line)?;
        let left = get(&f, "left")?;
        let right = get(&f, "right")?;
        let i = *positions
            .get(left.0)
            .ok_or_else(|| bad(left.1, "unknown left correlation coordinate"))?;
        let j = *positions
            .get(right.0)
            .ok_or_else(|| bad(right.1, "unknown right correlation coordinate"))?;
        if correlation[i * n + j].is_some() {
            return Err(bad(
                row.line,
                "duplicate correlation pair, including reversed pairs",
            ));
        }
        let raw = get(&f, "value")?;
        let value = finite(raw)?;
        correlation[i * n + j] = Some(value);
        correlation[j * n + i] = Some(value);
        correlations.push(Correlation {
            left: left.0.into(),
            right: right.0.into(),
            raw_value: raw.0.into(),
            value,
            line: row.line,
        });
    }
    Ok(Worksheet { declaration, observations, correlations,
        correlation: correlation.into_iter().map(|v| v.ok_or_else(|| bad(1, "every unordered raw correlation pair is required, including diagonal ones and zero cross terms"))).collect::<Result<_, _>>()?,
        model_basis: get(&top, "error_model_basis")?.0.into(), model_line: get(&top, "error_model_basis")?.1,
        first_order: boolean(get(&top, "first_order_approximation_accepted")?)?,
        raw_start: start.0.into(), raw_end: end.0.into() })
}

fn model(
    worksheet: &Worksheet,
    inputs: &[Claim<RawTransferInput>],
    source: &SourceAnchor,
) -> Result<(Claim<RawTransferErrorModel>, Vec<usize>), Refusal> {
    let ids = raw_measurements::observation_order(inputs)?;
    let positions: BTreeMap<ClaimId, usize> = worksheet
        .observations
        .iter()
        .enumerate()
        .map(|(i, row)| (row.claim.claim_id, i))
        .collect();
    let order = ids
        .iter()
        .map(|id| {
            positions
                .get(id)
                .copied()
                .ok_or_else(|| bad(1, "canonical raw coordinate lacks its submitted source"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let n = order.len();
    let original = &worksheet.declaration.value;
    let value = RawTransferErrorModel {
        scope: original.scope.clone(),
        stream: original.stream.clone(),
        declaration_id: worksheet.declaration.claim_id,
        observation_ids: ids,
        standard_uncertainties: order
            .iter()
            .map(|&i| worksheet.observations[i].uncertainty)
            .collect(),
        correlation: order
            .iter()
            .flat_map(|&i| order.iter().map(move |&j| worksheet.correlation[i * n + j]))
            .collect(),
        first_order_approximation_accepted: worksheet.first_order,
    };
    let tag = value.canonical_tag();
    Ok((
        declared(
            value,
            &tag,
            raw_measurements::ERROR_MODEL_METRIC,
            fmine_claim::AdditivityClass::NonAdditive,
            source,
            format!(
                "error_model_basis line {}; all standard uncertainties, raw correlations and first-order applicability declaration",
                worksheet.model_line
            ),
        ),
        order,
    ))
}
fn scalar(value: StandardUncertainty) -> f64 {
    match value {
        StandardUncertainty::MassKg(q) => q.value(),
        StandardUncertainty::Fraction(q) => q,
    }
}
fn observation_json(row: &Observation) -> String {
    let (lot, channel, value) = match &row.claim.value.value {
        RawTransferValue::WetMass { lot, kg } => (lot.as_str(), "wet_mass", *kg),
        RawTransferValue::Moisture {
            lot,
            wet_mass_fraction,
        } => (lot.as_str(), "moisture", *wet_mass_fraction),
        RawTransferValue::DryAssay {
            lot,
            element,
            dry_mass_fraction,
        } => (lot.as_str(), element.element.as_str(), *dry_mass_fraction),
        RawTransferValue::Declaration { .. } => return "null".into(),
    };
    let time = row.claim.value.physical_time;
    format!(
        concat!(
            "{{\"key\":{},\"lot\":{},\"channel\":{},\"raw_value\":{},\"value\":{},\"unit\":{},",
            "\"start_ns\":\"{}\",\"end_ns\":\"{}\",\"raw_start_ns\":{},\"raw_end_ns\":{},",
            "\"raw_standard_uncertainty\":{},\"standard_uncertainty\":{},\"uncertainty_unit\":{},\"support\":\"PlantLot\",\"line\":{},\"claim\":{}}}"
        ),
        quoted(&row.key),
        quoted(lot),
        quoted(channel),
        quoted(&row.raw_value),
        exact(value),
        quoted(row.unit),
        time.start_ns,
        time.end_ns,
        quoted(&row.raw_start),
        quoted(&row.raw_end),
        quoted(&row.raw_uncertainty),
        number(scalar(row.uncertainty)),
        quoted(row.unit),
        row.line,
        claim_json(&row.claim)
    )
}
fn declaration_json(worksheet: &Worksheet) -> String {
    let RawTransferValue::Declaration {
        lot_ids,
        elements,
        complete,
        nonoverlapping_lots,
        moisture_is_h2o,
        assays_are_total_elemental,
    } = &worksheet.declaration.value.value
    else {
        return "null".into();
    };
    let lots = lot_ids
        .iter()
        .map(|id| quoted(id))
        .collect::<Vec<_>>()
        .join(",");
    let elements = elements
        .iter()
        .map(|e| {
            format!(
                "{{\"element\":{},\"measurand\":{}}}",
                quoted(&e.element),
                quoted(&e.measurand)
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"lots\":[{lots}],\"elements\":[{elements}],\"complete\":{complete},\"nonoverlapping_lots\":{nonoverlapping_lots},\"moisture_is_h2o\":{moisture_is_h2o},\"assays_are_total_elemental\":{assays_are_total_elemental},\"support\":\"PeriodAggregate\",\"claim\":{}}}",
        claim_json(&worksheet.declaration)
    )
}
fn model_json(
    worksheet: &Worksheet,
    model: Option<&Claim<RawTransferErrorModel>>,
    order: &[usize],
) -> String {
    let coordinates = order
        .iter()
        .map(|&i| {
            let row = &worksheet.observations[i];
            format!(
                "{{\"key\":{},\"source_claim_id\":{}}}",
                quoted(&row.key),
                quoted(&row.claim.claim_id.to_string())
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let uncertainties = order
        .iter()
        .map(|&i| {
            let row = &worksheet.observations[i];
            format!(
                "{{\"key\":{},\"raw_value\":{},\"value\":{},\"unit\":{}}}",
                quoted(&row.key),
                quoted(&row.raw_uncertainty),
                number(scalar(row.uncertainty)),
                quoted(row.unit)
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let declarations = worksheet
        .correlations
        .iter()
        .map(|row| {
            format!(
                "{{\"left\":{},\"right\":{},\"raw_value\":{},\"value\":{},\"line\":{}}}",
                quoted(&row.left),
                quoted(&row.right),
                quoted(&row.raw_value),
                number(row.value),
                row.line
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let correlation = model.map_or(worksheet.correlation.as_slice(), |m| {
        m.value.correlation.as_slice()
    });
    format!(
        concat!(
            "{{\"basis\":{},\"first_order_approximation_accepted\":{},\"canonical_binding\":{},\"coordinate_order\":[{coordinates}],",
            "\"standard_uncertainties\":[{uncertainties}],\"correlation_layout\":\"row_major\",\"correlation\":{},\"declared_correlations\":[{declarations}],\"claim\":{}}}"
        ),
        quoted(&worksheet.model_basis),
        worksheet.first_order,
        model.is_some(),
        numbers(correlation.iter().copied()),
        model.map_or_else(|| "null".into(), claim_json),
        coordinates = coordinates,
        uncertainties = uncertainties,
        declarations = declarations
    )
}
fn projection_json(value: &TransferProjection) -> String {
    let elements = value
        .amounts
        .elements_kg
        .iter()
        .map(|(element, kg)| {
            format!(
                "{{\"element\":{},\"measurand\":{},\"kg\":{}}}",
                quoted(&element.element),
                quoted(&element.measurand),
                exact(*kg)
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let grades = value
        .dry_grade_mass_fractions
        .iter()
        .map(|(element, fraction)| {
            format!(
                "{{\"element\":{},\"measurand\":{},\"mass_fraction\":{}}}",
                quoted(&element.element),
                quoted(&element.measurand),
                exact(*fraction)
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let order = value
        .covariance_order
        .iter()
        .map(|channel| {
            quoted(match channel {
                ProjectedChannel::DryMatter => "dry",
                ProjectedChannel::Water => "water",
                ProjectedChannel::Element(e) => &e.element,
            })
        })
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"dry_matter_kg\":{},\"water_kg\":{},\"elements_kg\":[{elements}],\"dry_grade_mass_fractions\":[{grades}],\"covariance_order\":[{order}],\"covariance_layout\":\"row_major\",\"linearized_covariance_kg2\":{},\"covariance_approximation\":\"first_order\",\"transformed_distribution\":null}}",
        exact(value.amounts.dry_solids_kg),
        exact(value.amounts.water_kg),
        numbers(value.linearized_covariance.iter().map(|q| q.value()))
    )
}
fn actions() -> Vec<NextAction> {
    vec![NextAction::new(
        "fmine robot-docs",
        "inspect complete lot population, raw support, matched dry basis, uncertainties and every correlation pair",
    )]
}
fn finish(mut envelope: Envelope) -> Envelope {
    envelope.no_claim = Some(NO_CLAIM);
    envelope
}
fn failed_input(refusal: Refusal, source: Option<&str>) -> Envelope {
    let mut envelope = Envelope::refused(VERB, refusal, actions());
    envelope.payload_json = format!(
        "{{\"schema\":\"fmine.plant-transfer-result.v1\",\"source\":{},\"admitted_claim\":null,\"nominal_transfer\":null,\"receipt_digest\":null,\"result\":null}}",
        source.unwrap_or("null")
    );
    finish(envelope)
}

pub(super) fn execute(input: &Path, root: &Path) -> Envelope {
    let (text, source) =
        match worksheet_input::read(&root.join(input), "raw plant transfer worksheet", MAX_BYTES) {
            Ok(value) => value,
            Err(refusal) => return failed_input(refusal, None),
        };
    let source_json = format!(
        "{{\"path\":{},\"sha256\":{},\"bytes\":{}}}",
        quoted(&source.artifact_path),
        quoted(&Digest32(source.artifact_sha256).to_hex()),
        text.len()
    );
    let worksheet = match parse(&text, &source) {
        Ok(value) => value,
        Err(refusal) => return failed_input(refusal, Some(&source_json)),
    };
    let inputs: Vec<_> = std::iter::once(worksheet.declaration.clone())
        .chain(worksheet.observations.iter().map(|row| row.claim.clone()))
        .collect();
    let bound_model = model(&worksheet, &inputs, &source);
    let fallback_order: Vec<_> = (0..worksheet.observations.len()).collect();
    let (model, order) = match &bound_model {
        Ok((model, order)) => (Some(model), order.as_slice()),
        Err(_) => (None, fallback_order.as_slice()),
    };
    let outcome = bound_model
        .as_ref()
        .map_err(Clone::clone)
        .and_then(|(model, _)| raw_measurements::project_transfer(&inputs, model));
    let (mut envelope, admitted, transfer, digest, result) = match outcome {
        Ok(receipt) => {
            let value = &receipt.claim().value;
            let transfer = format!(
                "{{\"stream\":{},\"start_ns\":\"{}\",\"end_ns\":\"{}\",\"nominal_amounts_source\":\"result\",\"error_model_adoption\":\"requires_separate_declaration\",\"claim\":{}}}",
                quoted(value.stream.as_str()),
                value.scope.interval.start_ns,
                value.scope.interval.end_ns,
                claim_json(receipt.transfer())
            );
            (
                Envelope::ok(VERB, String::new(), vec![]),
                claim_json(receipt.claim()),
                transfer,
                quoted(receipt.digest()),
                projection_json(value),
            )
        }
        Err(refusal) => (
            Envelope::refused(VERB, refusal, actions()),
            "null".into(),
            "null".into(),
            "null".into(),
            "null".into(),
        ),
    };
    let declaration = &worksheet.declaration.value;
    let scope = format!(
        "{{\"boundary\":{},\"stream\":{},\"dry_basis\":{},\"start_ns\":\"{}\",\"end_ns\":\"{}\",\"raw_start_ns\":{},\"raw_end_ns\":{},\"whole_lot_support\":true,\"moisture_basis\":{},\"assay_basis\":{}}}",
        quoted(&declaration.scope.boundary),
        quoted(declaration.stream.as_str()),
        quoted(&declaration.dry_basis),
        declaration.scope.interval.start_ns,
        declaration.scope.interval.end_ns,
        quoted(&worksheet.raw_start),
        quoted(&worksheet.raw_end),
        quoted(MOISTURE_BASIS),
        quoted(ASSAY_BASIS)
    );
    envelope.payload_json = format!(
        concat!(
            "{{\"schema\":\"fmine.plant-transfer-result.v1\",\"mode\":\"exact_nominal_and_first_order_covariance\",\"source\":{source_json},\"scope\":{scope},",
            "\"declaration\":{},\"observations\":[{}],\"error_model\":{},\"admitted_claim\":{admitted},\"nominal_transfer\":{transfer},\"receipt_digest\":{digest},\"result\":{result}}}"
        ),
        declaration_json(&worksheet),
        worksheet
            .observations
            .iter()
            .map(observation_json)
            .collect::<Vec<_>>()
            .join(","),
        model_json(&worksheet, model, order),
        source_json = source_json,
        scope = scope,
        admitted = admitted,
        transfer = transfer,
        digest = digest,
        result = result
    );
    finish(envelope)
}
