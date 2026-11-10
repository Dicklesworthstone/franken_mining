//! DESIGN: preserve original evidence, conditional joint covariance and rejected diagnostics.
//! Determinism: DeterministicPerIsa

use super::*;
use fmine_core::{Digest32, anchor::SourceAnchor};
use std::fmt::Write as _;

fn quoted(value: &str) -> String {
    let mut out = String::from("\"");
    for c in value.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c if c.is_control() => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}
fn escaped(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('|', "&#124;")
        .replace('`', "&#96;")
        .replace(['\n', '\r'], " ")
}
fn number(value: f64) -> String {
    if value == 0.0 {
        "0".into()
    } else {
        value.to_string()
    }
}
fn numbers(values: impl Iterator<Item = f64>) -> String {
    format!("[{}]", values.map(number).collect::<Vec<_>>().join(","))
}
fn exact(value: ExactQty) -> String {
    format!(
        "{{\"numerator\":\"{}\",\"denominator\":\"{}\"}}",
        value.num(),
        value.den()
    )
}
fn source_json(source: &SourceAnchor) -> String {
    format!(
        "{{\"path\":{},\"sha256\":{},\"locator\":{}}}",
        quoted(&source.artifact_path),
        quoted(&Digest32(source.artifact_sha256).to_hex()),
        quoted(&source.locator)
    )
}
pub(super) fn refusal_json(text: &str, source: &SourceAnchor, refusal: &Refusal) -> String {
    format!(
        concat!(
            "{{\"schema\":\"fmine.plant-trajectory-result.v1\",\"source\":{},\"submitted_worksheet\":{},",
            "\"source_inputs\":null,\"error_model\":null,\"result\":null,\"admitted_claim\":null,\"receipt_digest\":null,",
            "\"refusal\":{{\"code\":{},\"what\":{}}},\"no_claim\":{}}}"
        ),
        source_json(source),
        quoted(text),
        quoted(refusal.code),
        quoted(&refusal.what),
        quoted(worksheet::NO_CLAIM),
    )
}
fn claim<T>(c: &Claim<T>) -> String {
    let sources = c
        .anchors
        .iter()
        .map(source_json)
        .collect::<Vec<_>>()
        .join(",");
    let parents = c
        .parents
        .iter()
        .map(|id| quoted(&id.to_string()))
        .collect::<Vec<_>>()
        .join(",");
    let uu = c.uu_terms.iter().map(|u| format!(
        "{{\"scenario_id\":{},\"description\":{},\"downside_exposure_usd_m\":{},\"trigger_indicator\":{}}}",
        quoted(&u.scenario_id), quoted(&u.description), number(u.downside_exposure_usd_m), quoted(&u.trigger_indicator),
    )).collect::<Vec<_>>().join(",");
    let mut body = String::new();
    for b in c.canonical_body() {
        let _ = write!(body, "{b:02x}");
    }
    format!(
        concat!(
            "{{\"id\":{},\"metric\":{},\"value_tag\":{},\"canonical_body_hex\":{},",
            "\"representation\":{},\"support\":{},\"additivity\":{},\"evidence\":{},\"epistemic\":{},",
            "\"state\":{},\"confidentiality\":{},\"calibration\":\"uncalibrated\",\"stale\":{},",
            "\"calibration_failed\":{},\"supersedes\":{},\"uu_terms\":[{uu}],\"parents\":[{parents}],\"sources\":[{sources}]}}"
        ),
        quoted(&c.claim_id.to_string()),
        quoted(&c.metric_key),
        quoted(c.value_tag()),
        quoted(&body),
        quoted(&format!("{:?}", c.representation)),
        quoted(&format!("{:?}", c.support)),
        quoted(&format!("{:?}", c.additivity)),
        quoted(&format!("{:?}", c.evidence)),
        quoted(&format!("{:?}", c.epistemic)),
        quoted(&format!("{:?}", c.state)),
        quoted(&c.confidentiality.to_string()),
        c.stale,
        c.calibration_failed,
        c.supersedes
            .map_or_else(|| "null".into(), |id| quoted(&id.to_string())),
        uu = uu,
        parents = parents,
        sources = sources,
    )
}
fn input(c: &Claim<TrajectoryInput>) -> String {
    let t = c.value.physical_time;
    let role = match c.value.value {
        TrajectoryValue::Population { .. } => "population",
        TrajectoryValue::Stocktake(_) => "stocktake",
        TrajectoryValue::Inflow(_) => "inflow",
        TrajectoryValue::Outflow(_) => "outflow",
    };
    format!(
        "{{\"role\":{},\"start_ns\":\"{}\",\"end_ns\":\"{}\",\"kg_exact\":{},\"claim\":{}}}",
        quoted(role),
        t.start_ns,
        t.end_ns,
        c.value.value.mass().map_or_else(|| "null".into(), exact),
        claim(c)
    )
}
fn coordinate(key: &TrajectoryLocation, source: Option<ClaimId>) -> String {
    format!(
        "{{\"key\":{},\"source_claim_id\":{},\"unmeasured\":{}}}",
        quoted(&key.worksheet_key()),
        source.map_or_else(|| "null".into(), |id| quoted(&id.to_string())),
        source.is_none()
    )
}
fn stock(d: &InventoryTrajectory, time: i64, i: usize) -> String {
    let n = d.estimates_kg.len();
    let variance = d.posterior_covariance[i * n + i].value();
    let observed = d.observed_kg.get(i).map(|v| v.value());
    let source = d.observations.get(i).map(|key| key.source);
    format!(
        concat!(
            "{{\"at_ns\":\"{time}\",\"coordinate_index\":{i},\"source_claim_id\":{},",
            "\"observed_kg\":{},\"estimate_kg\":{},\"posterior_variance_kg2\":{},",
            "\"conditional_standard_deviation_kg\":{},\"unmeasured\":{}}}"
        ),
        source.map_or_else(|| "null".into(), |id| quoted(&id.to_string())),
        observed.map_or_else(|| "null".into(), number),
        number(d.estimates_kg[i].value()),
        number(variance),
        number(variance.sqrt()),
        source.is_none(),
        time = time,
        i = i,
    )
}
fn gross(diagnostic: &GrossErrorDiagnostic) -> String {
    match diagnostic {
        GrossErrorDiagnostic::NoRedundancy => {
            "{\"status\":\"no_redundancy\",\"chi_square\":null,\"degrees_of_freedom\":0}".into()
        }
        GrossErrorDiagnostic::Available {
            chi_square,
            degrees_of_freedom,
        } => format!(
            "{{\"status\":\"available\",\"chi_square\":{},\"degrees_of_freedom\":{degrees_of_freedom}}}",
            number(*chi_square)
        ),
    }
}

pub(super) fn json(submitted: &str, source: &SourceAnchor, outcome: &TrajectoryOutcome) -> String {
    let d = outcome.diagnostics();
    let m = &outcome.error_model().value;
    let order = d
        .observations
        .iter()
        .map(|key| coordinate(&key.location, Some(key.source)))
        .chain(
            d.unmeasured_stocktakes_ns
                .iter()
                .map(|t| coordinate(&TrajectoryLocation::Stocktake(*t), None)),
        )
        .collect::<Vec<_>>()
        .join(",");
    let stocks = d
        .boundaries_ns
        .iter()
        .filter_map(|t| d.stock_coordinate_index(*t).map(|i| stock(d, *t, i)))
        .collect::<Vec<_>>()
        .join(",");
    let periods = d.conservation.iter().map(|r| format!(
        "{{\"start_ns\":\"{}\",\"end_ns\":\"{}\",\"observed_contribution_exact_kg\":{},\"has_unmeasured_terms\":{},\"reconciled_residual_kg\":{}}}",
        r.interval.start_ns, r.interval.end_ns, exact(r.observed_contribution_exact_kg), r.has_unmeasured_terms, number(r.reconciled_residual_kg.value()),
    )).collect::<Vec<_>>().join(",");
    let observations = outcome
        .inputs()
        .iter()
        .map(input)
        .collect::<Vec<_>>()
        .join(",");
    let normalized = d
        .normalized_adjustments
        .iter()
        .map(|v| v.map_or_else(|| "null".into(), number))
        .collect::<Vec<_>>()
        .join(",");
    let rejection = outcome.refusal().map_or_else(
        || "null".into(),
        |r| {
            format!(
                "{{\"code\":{},\"what\":{}}}",
                quoted(r.code),
                quoted(&r.what)
            )
        },
    );
    format!(
        concat!(
            "{{\"schema\":\"fmine.plant-trajectory-result.v1\",\"source\":{},\"submitted_worksheet\":{},",
            "\"scope\":{{\"boundary\":{},\"element\":{},\"measurand\":{},\"basis\":{},\"start_ns\":\"{}\",\"end_ns\":\"{}\"}},",
            "\"source_inputs\":[{observations}],\"coordinate_order\":[{order}],\"measured_coordinate_count\":{},",
            "\"error_model\":{{\"claim\":{},\"basis\":{},\"covariance_kg2\":{},\"unbiased_gaussian_errors\":true,",
            "\"global_chi_square_limit\":{},\"normalized_adjustment_limit\":{},\"closure_tolerance_kg\":{}}},",
            "\"result\":{{\"stocks\":[{stocks}],\"observed_kg\":{},\"estimates_kg\":{},\"measured_adjustments_kg\":{},",
            "\"posterior_covariance_kg2\":{},\"measured_adjustment_covariance_kg2\":{},\"normalized_adjustments\":[{normalized}],",
            "\"period_conservation\":[{periods}],\"gross_error\":{}}},",
            "\"admitted_claim\":{},\"refusal\":{rejection},\"receipt_digest\":{},\"calibration\":\"uncalibrated\",\"no_claim\":{}}}"
        ),
        source_json(source),
        quoted(submitted),
        quoted(&d.scope.boundary),
        quoted(&d.scope.element),
        quoted(&d.scope.measurand),
        quoted(inventory::MASS_BASIS),
        d.scope.interval.start_ns,
        d.scope.interval.end_ns,
        d.observations.len(),
        claim(outcome.error_model()),
        quoted(&m.error_model_basis),
        numbers(m.covariance.iter().map(|v| v.value())),
        number(m.global_chi_square_limit),
        m.normalized_adjustment_limit
            .map_or_else(|| "null".into(), number),
        number(m.closure_tolerance_kg.value()),
        numbers(d.observed_kg.iter().map(|v| v.value())),
        numbers(d.estimates_kg.iter().map(|v| v.value())),
        numbers(d.measured_adjustments_kg.iter().map(|v| v.value())),
        numbers(d.posterior_covariance.iter().map(|v| v.value())),
        numbers(d.measured_adjustment_covariance.iter().map(|v| v.value())),
        gross(&d.gross_error),
        outcome
            .admitted_claim()
            .map_or_else(|_| "null".into(), claim),
        quoted(outcome.digest()),
        quoted(worksheet::NO_CLAIM),
        observations = observations,
        order = order,
        stocks = stocks,
        normalized = normalized,
        periods = periods,
        rejection = rejection,
    )
}

pub(super) fn markdown(outcome: &TrajectoryOutcome) -> String {
    let d = outcome.diagnostics();
    let mut out = format!(
        "# Conditional inventory trajectory\n\n{}\n\nBoundary: {}. Measurand: {} ({}). Scope: {}–{} ns.\n\n",
        worksheet::NO_CLAIM,
        escaped(&d.scope.boundary),
        escaped(&d.scope.measurand),
        escaped(&d.scope.element),
        d.scope.interval.start_ns,
        d.scope.interval.end_ns
    );
    if let Some(refusal) = outcome.refusal() {
        let _ = writeln!(
            out,
            "**Rejected: {} — {}** No admitted Claim; the complete fitted candidate remains available for investigation.\n",
            refusal.code,
            escaped(&refusal.what)
        );
    }
    out.push_str("## Shared boundary stocks\n\nStandard deviations are conditional on the submitted Gaussian covariance, not empirical coverage. Each row is a single shared state.\n\n| Boundary time (ns) | Observed kg | Fitted kg | Conditional standard deviation (kg) | Status |\n|---|---:|---:|---:|---|\n");
    let count = d.estimates_kg.len();
    for time in &d.boundaries_ns {
        if let Some(i) = d.stock_coordinate_index(*time) {
            let observed = d
                .observed_kg
                .get(i)
                .map_or_else(|| "—".into(), |v| number(v.value()));
            let status = if i < d.observations.len() {
                "Measured, reconciled"
            } else {
                "Unmeasured, inferred"
            };
            let _ = writeln!(
                out,
                "| {time} | {observed} | {} | {} | {status} |",
                number(d.estimates_kg[i].value()),
                number(d.posterior_covariance[i * count + i].value().sqrt())
            );
        }
    }
    out.push_str("\n## Original observations and fitted adjustments\n\n| Coordinate | Observed kg | Fitted kg | Adjustment kg | Source Claim |\n|---|---:|---:|---:|---|\n");
    for (i, key) in d.observations.iter().enumerate() {
        let _ = writeln!(
            out,
            "| {} | {} | {} | {} | {} |",
            escaped(&key.location.worksheet_key()),
            number(d.observed_kg[i].value()),
            number(d.estimates_kg[i].value()),
            number(d.measured_adjustments_kg[i].value()),
            key.source
        );
    }
    out.push_str("\n## Conservation and gross-error diagnostics\n\n| Period (ns) | Exact observed contribution (kg) | Contains unmeasured stocks | Fitted residual (kg) |\n|---|---:|---|---:|\n");
    for row in &d.conservation {
        let q = row.observed_contribution_exact_kg;
        let _ = writeln!(
            out,
            "| {}–{} | {}/{} | {} | {} |",
            row.interval.start_ns,
            row.interval.end_ns,
            q.num(),
            q.den(),
            row.has_unmeasured_terms,
            number(row.reconciled_residual_kg.value())
        );
    }
    match d.gross_error {
        GrossErrorDiagnostic::NoRedundancy => out.push_str("\nNo redundant conservation information remains: the gross-error test is unavailable. Zero fitted adjustments are not evidence of measurement accuracy.\n"),
        GrossErrorDiagnostic::Available { chi_square, degrees_of_freedom } => {
            let _ = writeln!(out, "\nJoint chi-square: {}; degrees of freedom: {degrees_of_freedom}; declared limit: {}. Normalized adjustments are correlated diagnostics.\n", number(chi_square), number(outcome.error_model().value.global_chi_square_limit));
        }
    }
    let _ = writeln!(
        out,
        "\nFull cross-period and measured/latent posterior covariance, original source Claims, submitted worksheet and error model are retained in JSON.\n\nReceipt digest: `{}`.\n",
        outcome.digest()
    );
    out
}
