//! DESIGN: source-bound plant networks over the joint R1 owners.
//! The worksheet declares extensive kg, complete topology and every covariance
//! pair. Only inference may explicitly declare unmeasured streams or endpoint
//! inventories. No independence, missing channel or omission is inferred.
//! Determinism: DeterministicPerIsa

mod inference;

use std::{collections::BTreeMap, path::Path};

use fmine_claim::Claim;
use fmine_core::{Digest32, anchor::SourceAnchor};
use fmine_evidence::Refusal;
use fmine_plant::flowsheet::{
    FlowsheetInput, FlowsheetValue, OutflowRole, StreamRoute, UnitKind,
    reconciliation::{
        self, BoundaryRole, FlowsheetErrorModel, FlowsheetReconciliation,
        FlowsheetReconciliationOutcome, MaterialChannel, ObservationKey, ObservationLocation,
        unmeasured::{UnmeasuredFlowModel, quantities::UnmeasuredQuantityModel},
    },
    worksheet::{self, BASIS, ErrorModel, Original, channel_name, coordinate_name, location_name},
};
use fmine_plant::reconciliation::{Kilograms, KilogramsSquared};
use fmine_units::ExactQty;

use super::{MAX_BYTES, claim_json, exact, number, numbers, quoted};
use crate::{Envelope, NextAction, worksheet_input};

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

struct RunOutput {
    envelope: Envelope,
    admitted: String,
    digest: String,
    result: String,
}

fn parse(
    text: &str,
    source: &SourceAnchor,
    requested_mode: &mut Mode,
) -> Result<worksheet::Worksheet, Refusal> {
    let mut mode = match *requested_mode {
        Mode::Measured => worksheet::Mode::Measured,
        Mode::Unmeasured => worksheet::Mode::Unmeasured,
        Mode::Quantities => worksheet::Mode::Quantities,
    };
    let parsed = worksheet::parse(text, source, &mut mode);
    // Preserve the schema selected by admission even when a later field refuses.
    *requested_mode = match mode {
        worksheet::Mode::Measured => Mode::Measured,
        worksheet::Mode::Unmeasured => Mode::Unmeasured,
        worksheet::Mode::Quantities => Mode::Quantities,
    };
    parsed
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
