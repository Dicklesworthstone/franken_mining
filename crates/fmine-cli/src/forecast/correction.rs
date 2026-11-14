//! DESIGN (BD-049): render the explicit raw, fitted correction and unissued proposal with the saved
//! observations. No new outcome selection, journal append or inferred calibration is performed.

use std::fmt::Write as _;

use fmine_calib::ledger::correction::{CorrectionProposal, METHOD, NO_CLAIM};
use fmine_calib::{ForecastPayload, MemberOutcome};
use fmine_core::Digest32;
use fmine_evidence::Refusal;

use super::{Replay, js, payload_json, selector_name, utc};

pub(super) const ROBOT_DOCS_JSON: &str = r#"{"command":"forecast journal correction --journal <path> --assessment <digest> --forecast <alias> [--head <hex>]","operation":"Read-only unissued proposal; no host-clock sampling, outcome re-selection, journal append or replacement of an existing forecast","inputs":"One complete saved assessment of at least two positive Gaussian forecast/outcome pairs and a later unresolved raw Gaussian forecast; every model/family/author/domain/horizon/site/quantity/unit label must match exactly","chronology":"Target registration strictly follows the saved assessment cut, every training outcome boundary precedes target registration, and the target has no recorded outcome event; all target cohorts use the same exact alpha","method":"mu=mean(log(actual/raw_mean)); factor=exp(mu); r_i=abs(actual_i-factor*mean_i)/(factor*std_i*z_(1-alpha/2)); kappa=max(1,inverse_empirical_CDF(r,1-alpha)), rounded outward when inflated; corrected Gaussian mean=factor*raw_mean, std=factor*kappa*raw_std","scope":"Pooled within one complete segment only, not a hierarchical family/site estimator; sample log dispersion and fitted training coverage are descriptive","output":"Unchanged raw forecast, visible dimensionless correction, corrected proposal, selected source outcomes, all members and a separate COR1/v1 digest; uncalibrated and unissued, with no automatic registration or claimed out-of-sample coverage"}"#;

fn bounds(value: (f64, f64)) -> String {
    format!("[{},{}]", value.0, value.1)
}

pub(super) fn json(state: &Replay, report: &CorrectionProposal) -> Result<String, Refusal> {
    let members = report.members.iter().map(|member| {
        Ok(format!(
            "{{\"forecast\":{},\"forecast_id\":{},\"raw\":{},\"selected\":{},\"log_ratio\":{},\"raw_inside\":{},\"corrected_interval\":{},\"corrected_inside\":{}}}",
            js(state.alias(member.raw.forecast_id)), js(&member.raw.forecast_id.to_string()),
            payload_json(&member.raw.payload), state.member_json(&member.selected)?, member.log_ratio,
            member.raw_inside, bounds(member.corrected_interval), member.corrected_inside,
        ))
    }).collect::<Result<Vec<_>, Refusal>>()?.join(",");
    Ok(format!(
        "{{\"schema\":\"fmine.forecast-correction.v1\",\"method\":{},\"digest\":{},\"status\":\"unissued_proposal\",\"uncalibrated\":true,\"no_claim\":{},\"assessment_digest\":{},\"cohort_digest\":{},\"training_as_of\":{},\"selector\":{},\"alpha\":{},\"model_key\":{}{},\"n_training\":{},\"mean_log_ratio\":{},\"sample_log_std_dev\":{},\"bias_factor\":{},\"inflation\":{},\"required_training_hits\":{},\"raw_training_hits\":{},\"corrected_training_hits\":{},\"coverage_interpretation\":\"fitted_in_sample_only\",\"target\":{{\"forecast\":{},\"forecast_id\":{},\"registered_at\":{},\"outcome_due\":{},\"raw\":{},\"raw_interval\":{},\"corrected\":{},\"corrected_interval\":{}}},\"members\":[{members}]}}",
        js(METHOD),
        js(&report.digest.to_hex()),
        js(NO_CLAIM),
        js(&report.assessment_digest.to_hex()),
        js(&report.cohort_digest.to_hex()),
        utc(report.training_as_of_ns),
        js(selector_name(report.selector)),
        report.alpha,
        js(&report.raw.model_key),
        super::segments::metadata_field(Some(&report.metadata)),
        report.members.len(),
        report.mean_log_ratio,
        report.sample_log_std_dev,
        report.bias_factor,
        report.inflation,
        report.required_training_hits,
        report.raw_training_hits,
        report.corrected_training_hits,
        js(state.alias(report.raw.forecast_id)),
        js(&report.raw.forecast_id.to_string()),
        utc(report.raw.registered_ns),
        utc(report.raw.freeze_time_ns),
        payload_json(&report.raw.payload),
        bounds(report.raw_interval),
        payload_json(&report.corrected),
        bounds(report.corrected_interval),
    ))
}

fn distribution(value: &ForecastPayload) -> String {
    match value {
        ForecastPayload::Gaussian { mean, std_dev } => {
            format!("mean {mean}; standard deviation {std_dev}")
        }
        _ => "unsupported".into(),
    }
}

pub(super) fn markdown(state: &Replay, report: &CorrectionProposal) -> String {
    let text = crate::guidance::text;
    let metadata = &report.metadata;
    let mut out = format!(
        "# Forecast correction proposal\n\n**Unissued and uncalibrated.** {NO_CLAIM}\n\nMethod: `{METHOD}`. Correction digest: `{}`.\n\nSaved assessment: `{}`. Training cut: {}; selector: {}.\n\nModel: {}. Family: {}. Author role: {}. Domain: {}. Horizon: {}. Site: {}. Quantity: {}. Unit: {}.\n\n## Raw, correction and corrected proposal\n\nTarget: {} (`{}`), registered {}, outcome due {}.\n\n| Quantity | Value |\n|---|---|\n| Unchanged raw Gaussian | {} |\n| Raw central interval | {} |\n| Mean log(actual/raw mean) | {} |\n| Sample log-ratio standard deviation | {} |\n| Multiplicative bias factor | {} |\n| Scale inflation after bias rescaling | {} |\n| Corrected Gaussian proposal | {} |\n| Corrected central interval | {} |\n\nIntervals use alpha {}. The bias factor is exp(mean(log(actual/raw mean))). Inflation is the inverse empirical (1-alpha) quantile of abs(actual-factor*mean)/(factor*std*z), floored at one, with outward rounding when inflated. Both corrected mean and standard deviation retain the declared unit. Zero fitted log dispersion does not collapse the predictive width. Inflation cannot narrow the bias-rescaled width; a factor below one can still shrink absolute width relative to the raw forecast.\n\n## Complete training population\n\nRaw hits: {}/{}. Fitted corrected hits: {}/{}; required empirical rank: {}. These are in-sample counts and do not demonstrate calibrated future coverage.\n\n| Forecast | Raw Gaussian | Log ratio | Raw hit | Corrected interval | Corrected hit |\n|---|---|---:|---|---|---|\n",
        report.digest,
        report.assessment_digest,
        utc(report.training_as_of_ns).trim_matches('"'),
        selector_name(report.selector),
        text(&report.raw.model_key),
        text(&metadata.family),
        text(&metadata.author_role),
        text(&metadata.domain),
        text(&metadata.horizon),
        text(&metadata.site),
        text(&metadata.quantity),
        text(&metadata.unit),
        text(state.alias(report.raw.forecast_id)),
        report.raw.forecast_id,
        utc(report.raw.registered_ns).trim_matches('"'),
        utc(report.raw.freeze_time_ns).trim_matches('"'),
        distribution(&report.raw.payload),
        bounds(report.raw_interval),
        report.mean_log_ratio,
        report.sample_log_std_dev,
        report.bias_factor,
        report.inflation,
        distribution(&report.corrected),
        bounds(report.corrected_interval),
        report.alpha,
        report.raw_training_hits,
        report.members.len(),
        report.corrected_training_hits,
        report.members.len(),
        report.required_training_hits,
    );
    for member in &report.members {
        let _ = writeln!(
            out,
            "| {} | {} | {} | {} | {} | {} |",
            text(state.alias(member.raw.forecast_id)),
            distribution(&member.raw.payload),
            member.log_ratio,
            member.raw_inside,
            bounds(member.corrected_interval),
            member.corrected_inside
        );
    }
    out.push_str("\n## Selected source evidence\n\n");
    for member in &report.members {
        if let MemberOutcome::Resolved {
            value,
            event_seq,
            event_digest,
            citation,
        } = &member.selected.outcome
        {
            let _ = writeln!(
                out,
                "- {}: value {} {}; {} / {} / {}; SHA-256 `{}`; selected event {} (`{}`).",
                text(state.alias(member.raw.forecast_id)),
                value,
                text(&metadata.unit),
                text(&citation.source),
                text(&citation.vintage),
                text(&citation.locator),
                Digest32(citation.sha256),
                event_seq,
                event_digest
            );
        }
    }
    out
}
