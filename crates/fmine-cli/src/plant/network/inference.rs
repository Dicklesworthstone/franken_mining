//! DESIGN: render explicitly unmeasured-flow inference without presenting an
//! inferred coordinate as an observation or no redundancy as a passed test.
//! All numerical work belongs to the existing plant and numerical owners.
//! Determinism: DeterministicPerIsa

use fmine_plant::flowsheet::reconciliation::unmeasured::{
    GrossErrorDiagnostic, InferredFlowsheet, UnmeasuredFlowOutcome, reconcile_unmeasured_flowsheet,
};

use super::{
    Claim, Envelope, FlowsheetInput, RunOutput, UnmeasuredFlowModel, actions, boundary_role,
    channel_name, claim_json, coordinate_name, exact, location_name, number, numbers, policy_json,
    quoted,
};

pub(super) const NO_CLAIM: &str = "Conditional Gaussian inference of explicitly unmeasured extensive streams from the declared conservative graph and genuine observations. Inferred coordinates are estimates, not measurements. Sources and error policy remain Assumed, Internal and uncalibrated. NoRedundancy supplies no gross-error test; unique inference is not a passed measurement check. No invented observation, sensor-bias identification, missing-stock estimate, assay-error propagation, recovery, P754 compliance, ledger commit or operating authorization is supplied.";

pub(super) const ROBOT_DOCS_JSON: &str = concat!(
    "{\"schema\":\"fmine.plant-unmeasured.v1\",\"command\":\"plant infer --input <worksheet.toml>\",",
    "\"input\":\"Regular UTF-8 TOML at most 1 MiB; all values quoted; exact submitted bytes are SHA-256 bound\",",
    "\"fields\":[\"schema\",\"boundary\",\"basis\",\"start_ns\",\"end_ns\",\"complete\",\"unbiased_gaussian_errors\",\"global_chi_square_limit\",\"error_model_basis\"],",
    "\"optional_fields\":[\"normalized_adjustment_limit\"],",
    "\"basis\":\"kg/dry-matter+water/total-elements/all-phases/full-boundary\",",
    "\"shared_rows\":\"unit, stream, element, coordinate, covariance and tolerance have the same strict fields as plant_flowsheet\",",
    "\"unmeasured\":\"[[unmeasured]] stream; at least one unique declared stream required; ALL its dry, water and elemental coordinates must be absent, with no covariance entries for those coordinates\",",
    "\"population\":\"Complete topology and every measured stream plus both endpoint stocktakes of every unit remain explicit; only named unmeasured streams may omit observations\",",
    "\"covariance\":\"ALL unordered pairs of genuine observed coordinates, including explicit zeros; full positive-definite kg2 covariance; no priors, fabricated observations or variance floors for unmeasured streams\",",
    "\"limits\":\"128 total measured and inferred coordinates, 64 unit/channel rows; ambiguous cycles, parallel missing outlets and non-identifiable graphs refuse\",",
    "\"output\":\"Coordinate order is measured then inferred; every coordinate states its kind and inferred source_claim_id is null; full joint covariance follows this order, while adjustments and adjustment covariance cover only genuine observations\",",
    "\"boundary\":\"Observed contributions are not complete residuals where unmeasured terms exist; observed_totals_kg is null for boundary roles containing missing external flows, with fitted totals and full covariance retained\",",
    "\"gross_error\":\"status=available carries chi-square and degrees of freedom; status=no_redundancy carries null chi-square and zero degrees of freedom, never a passed gross-error test\",",
    "\"rejected\":\"Nonzero exit, null admitted_claim; a rejected candidate retains original observations, model, fitted quantities, covariance, diagnostics and receipt identity\",",
    "\"evidence\":\"Assumed, Internal and uncalibrated; no P754 qualification or empirical accuracy claim\",",
    "\"example\":\"tests/plant/flowsheet_infer.toml\"}"
);

pub(super) fn order_json(model: &UnmeasuredFlowModel) -> String {
    let measured = model.observations.iter().map(|key| {
        format!(
            "{{\"key\":{},\"observation\":{},\"channel\":{},\"kind\":\"measured\",\"source_claim_id\":{}}}",
            quoted(&coordinate_name(key)), quoted(&location_name(&key.location)),
            quoted(channel_name(&key.channel)), quoted(&key.source.to_string())
        )
    });
    let inferred = model.unmeasured_streams.iter().flat_map(|stream| {
        model.closure_tolerances_kg.keys().map(move |channel| {
            let observation = format!("transfer:{}", stream.as_str());
            format!(
                "{{\"key\":{},\"observation\":{},\"channel\":{},\"kind\":\"inferred\",\"source_claim_id\":null}}",
                quoted(&format!("{observation}/{}", channel_name(channel))),
                quoted(&observation), quoted(channel_name(channel))
            )
        })
    });
    format!(
        "[{}]",
        measured.chain(inferred).collect::<Vec<_>>().join(",")
    )
}

pub(super) fn model_json(model: &Claim<UnmeasuredFlowModel>, basis: &str) -> String {
    let value = &model.value;
    let streams = value
        .unmeasured_streams
        .iter()
        .map(|stream| quoted(stream.as_str()))
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"claim\":{},\"basis\":{},\"declaration_id\":{},\"unmeasured_streams\":[{}],\"covariance_order\":\"measured_coordinates_only\",{}}}",
        claim_json(model),
        quoted(basis),
        quoted(&value.declaration_id.to_string()),
        streams,
        policy_json(
            &value.covariance,
            value.global_chi_square_limit,
            value.normalized_adjustment_limit,
            &value.closure_tolerances_kg
        )
    )
}

fn diagnostics(value: &InferredFlowsheet) -> String {
    let units = value.conservation.iter().map(|row| {
        format!(
            "{{\"unit\":{},\"channel\":{},\"has_unmeasured_terms\":{},\"observed_contribution_exact_kg\":{},\"observed_contribution_kg\":{},\"reconciled_residual_kg\":{}}}",
            quoted(row.unit.as_str()), quoted(channel_name(&row.channel)), row.has_unmeasured_terms,
            exact(row.observed_contribution_exact_kg), number(row.observed_contribution_kg.value()),
            number(row.reconciled_residual_kg.value())
        )
    }).collect::<Vec<_>>().join(",");
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
    let boundary_contributions = value
        .boundary
        .observed_contributions_exact_kg
        .iter()
        .map(|q| exact(*q))
        .collect::<Vec<_>>()
        .join(",");
    let boundary_observed = value
        .boundary
        .observed_totals_kg
        .iter()
        .map(|q| q.map_or_else(|| "null".into(), |q| number(q.value())))
        .collect::<Vec<_>>()
        .join(",");
    let boundary_residuals = value
        .boundary
        .reconciled_residuals_kg
        .iter()
        .map(|(channel, q)| {
            format!(
                "{{\"channel\":{},\"reconciled_residual_kg\":{}}}",
                quoted(channel_name(channel)),
                number(q.value())
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let normalized = value
        .normalized_adjustments
        .iter()
        .map(|q| q.map_or_else(|| "null".into(), number))
        .collect::<Vec<_>>()
        .join(",");
    let gross_error = match &value.gross_error {
        GrossErrorDiagnostic::Available {
            chi_square,
            degrees_of_freedom,
        } => format!(
            "{{\"status\":\"available\",\"chi_square\":{},\"degrees_of_freedom\":{}}}",
            number(*chi_square),
            degrees_of_freedom
        ),
        GrossErrorDiagnostic::NoRedundancy => {
            "{\"status\":\"no_redundancy\",\"chi_square\":null,\"degrees_of_freedom\":0}".into()
        }
    };
    format!(
        "{{\"coordinate_layout\":\"measured_then_inferred\",\"measured_count\":{},\"inferred_count\":{},\"observed_kg\":{},\"estimates_kg\":{},\"measured_adjustments_kg\":{},\"posterior_covariance_kg2\":{},\"measured_adjustment_covariance_kg2\":{},\"normalized_adjustments\":[{normalized}],\"unit_conservation\":[{units}],\"boundary\":{{\"order\":[{boundary_order}],\"observed_contributions_exact_kg\":[{boundary_contributions}],\"observed_totals_kg\":[{boundary_observed}],\"estimates_kg\":{},\"posterior_covariance_kg2\":{},\"conservation\":[{boundary_residuals}]}},\"gross_error\":{gross_error}}}",
        value.observations.len(),
        value.unmeasured.len(),
        numbers(value.observed_kg.iter().map(|q| q.value())),
        numbers(value.estimates_kg.iter().map(|q| q.value())),
        numbers(value.measured_adjustments_kg.iter().map(|q| q.value())),
        numbers(value.posterior_covariance.iter().map(|q| q.value())),
        numbers(
            value
                .measured_adjustment_covariance
                .iter()
                .map(|q| q.value())
        ),
        numbers(value.boundary.estimates_kg.iter().map(|q| q.value())),
        numbers(
            value
                .boundary
                .posterior_covariance
                .iter()
                .map(|q| q.value())
        ),
    )
}

pub(super) fn run(
    inputs: &[Claim<FlowsheetInput>],
    model: &Claim<UnmeasuredFlowModel>,
) -> RunOutput {
    let verb = "plant infer";
    let (envelope, admitted, digest, result) = match reconcile_unmeasured_flowsheet(inputs, model) {
        Ok(UnmeasuredFlowOutcome::Admitted(receipt)) => (
            Envelope::ok(verb, String::new(), vec![]),
            claim_json(receipt.claim()),
            quoted(receipt.digest()),
            diagnostics(&receipt.claim().value),
        ),
        Ok(UnmeasuredFlowOutcome::Rejected(rejected)) => (
            Envelope::refused(verb, rejected.refusal().clone(), actions()),
            "null".into(),
            quoted(rejected.digest()),
            diagnostics(rejected.diagnostics()),
        ),
        Err(refusal) => (
            Envelope::refused(verb, refusal, actions()),
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
