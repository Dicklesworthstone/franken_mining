//! DESIGN (BD-049): render the explicit raw, fitted correction and unissued proposal with the saved
//! observations. No new outcome selection, journal append or inferred calibration is performed.

use std::fmt::Write as _;

use fmine_calib::ledger::correction::{
    CorrectionProposal, CorrectionSeriesProposal, METHOD, NO_CLAIM, SERIES_METHOD, SERIES_NO_CLAIM,
};
use fmine_calib::{Cohort, ForecastPayload, FrozenForecast, MemberOutcome};
use fmine_core::{Digest32, ObjectId};
use fmine_evidence::Refusal;

use super::{Replay, js, payload_json, selector_name, utc};

pub(super) mod registration;

pub(super) const ROBOT_DOCS_JSON: &str = r#"{"command":"forecast journal correction --journal <path> (--assessment <digest> | --assessments <digest,digest,...>) --forecast <alias> [--head <hex>]","operation":"Read-only unissued proposal; no host-clock sampling, outcome re-selection, journal append or replacement of an existing forecast","inputs":"Exactly one selector: --assessment retains one complete saved cohort with at least two positive Gaussian forecast/outcome pairs; --assessments requires at least two distinct saved assessments and retains every member. Every model/family/author/domain/horizon/site/quantity/unit label must match the later unresolved raw Gaussian target","series":"Repeated assessment, cohort, forecast or cited observation refuses. Every parent uses the same exact alpha and outcome selector. Public parents and frozen target cohorts retain the same issuer, metric, ownership, vintage permission and calendar period kind, with advancing nonoverlapping supports. Supplied parents retain the same resolution rule and eligible sources; supplied/public mixing refuses","chronology":"Target registration strictly follows every saved training cut and training outcome boundary; series additionally verifies selected-event availability and record times. The target has no recorded outcome event; all target cohorts use the same exact alpha","method":"mu=mean(log(actual/raw_mean)); factor=exp(mu); r_i=abs(actual_i-factor*mean_i)/(factor*std_i*z_(1-alpha/2)); kappa=max(1,inverse_empirical_CDF(r,1-alpha)), rounded outward when inflated; corrected Gaussian mean=factor*raw_mean, std=factor*kappa*raw_std","scope":"Pooled within one declared segment, not a hierarchical family/site estimator. Explicitly selecting complete reports does not establish a complete historical record or independent observations. Sample log dispersion and fitted training coverage are descriptive","output":"Unchanged raw forecast, visible dimensionless correction, corrected proposal and every selected source outcome. Singular mode preserves fmine.forecast-correction.v1 and COR1/v1; series uses fmine.forecast-correction-series.v1 and COR2/v1, retaining full parent/target cohort policies and membership. Uncalibrated and unissued, with no automatic registration or claimed out-of-sample coverage"}"#;

#[derive(Debug, Clone, PartialEq)]
pub(super) enum Proposal {
    Single(CorrectionProposal),
    Series(CorrectionSeriesProposal),
}

impl Proposal {
    pub(super) fn digest(&self) -> Digest32 {
        match self {
            Self::Single(report) => report.digest,
            Self::Series(report) => report.digest,
        }
    }

    pub(super) fn raw(&self) -> &FrozenForecast {
        match self {
            Self::Single(report) => &report.raw,
            Self::Series(report) => &report.raw,
        }
    }

    pub(super) fn corrected(&self) -> &ForecastPayload {
        match self {
            Self::Single(report) => &report.corrected,
            Self::Series(report) => &report.corrected,
        }
    }

    pub(super) fn alpha(&self) -> f64 {
        match self {
            Self::Single(report) => report.alpha,
            Self::Series(report) => report.alpha,
        }
    }

    pub(super) fn json(&self, state: &Replay) -> Result<String, Refusal> {
        match self {
            Self::Single(report) => json(state, report),
            Self::Series(report) => series_json(state, report),
        }
    }

    pub(super) fn markdown(&self, state: &Replay) -> String {
        match self {
            Self::Single(report) => markdown(state, report),
            Self::Series(report) => series_markdown(state, report),
        }
    }
}

/// The live owner checks that every parent is a persisted checkpoint before requesting a proposal.
pub(super) fn proposal(
    state: &Replay,
    assessments: &[Digest32],
    series: bool,
    raw_id: ObjectId,
) -> Result<Proposal, Refusal> {
    match (series, assessments) {
        (false, [assessment]) => state
            .ledger
            .correction_proposal(*assessment, raw_id)
            .map(Proposal::Single),
        (true, [_, _, ..]) => state
            .ledger
            .correction_series_proposal(assessments, raw_id)
            .map(Proposal::Series),
        (false, _) => Err(super::invalid(
            "singular correction requires exactly one saved assessment",
        )),
        (true, _) => Err(super::invalid(
            "correction series requires at least two saved assessments",
        )),
    }
}

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

fn cohort_members_json(state: &Replay, cohort: &Cohort) -> String {
    cohort
        .spec
        .members
        .iter()
        .map(|id| {
            format!(
                "{{\"forecast\":{},\"forecast_id\":{}}}",
                js(state.alias(*id)),
                js(&id.to_string())
            )
        })
        .collect::<Vec<_>>()
        .join(",")
}

fn series_json(state: &Replay, report: &CorrectionSeriesProposal) -> Result<String, Refusal> {
    let parents = report.parents.iter().map(|parent| {
        format!(
            "{{\"assessment_digest\":{},\"as_of\":{},\"selector\":{},\"cohort\":{},\"members\":[{}]}}",
            js(&parent.assessment_digest.to_hex()),
            utc(parent.as_of_ns),
            js(selector_name(parent.selector)),
            super::cohort_json(&parent.cohort),
            cohort_members_json(state, &parent.cohort),
        )
    }).collect::<Vec<_>>().join(",");
    let targets = report
        .target_cohorts
        .iter()
        .map(|cohort| {
            format!(
                "{{\"cohort\":{},\"members\":[{}]}}",
                super::cohort_json(cohort),
                cohort_members_json(state, cohort),
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let members = report.members.iter().map(|member| {
        Ok(format!(
            "{{\"forecast\":{},\"forecast_id\":{},\"raw\":{},\"selected\":{},\"log_ratio\":{},\"raw_inside\":{},\"corrected_interval\":{},\"corrected_inside\":{}}}",
            js(state.alias(member.raw.forecast_id)), js(&member.raw.forecast_id.to_string()),
            payload_json(&member.raw.payload), state.member_json(&member.selected)?, member.log_ratio,
            member.raw_inside, bounds(member.corrected_interval), member.corrected_inside,
        ))
    }).collect::<Result<Vec<_>, Refusal>>()?.join(",");
    Ok(format!(
        "{{\"schema\":\"fmine.forecast-correction-series.v1\",\"method\":{},\"digest\":{},\"digest_encoding\":\"COR2/v1\",\"status\":\"unissued_proposal\",\"uncalibrated\":true,\"no_claim\":{},\"parents\":[{parents}],\"target_cohorts\":[{targets}],\"training_as_of\":{},\"selector\":{},\"alpha\":{},\"model_key\":{}{},\"n_assessments\":{},\"n_training\":{},\"mean_log_ratio\":{},\"sample_log_std_dev\":{},\"bias_factor\":{},\"inflation\":{},\"required_training_hits\":{},\"raw_training_hits\":{},\"corrected_training_hits\":{},\"coverage_interpretation\":\"fitted_in_sample_only\",\"target\":{{\"forecast\":{},\"forecast_id\":{},\"registered_at\":{},\"outcome_due\":{},\"raw\":{},\"raw_interval\":{},\"corrected\":{},\"corrected_interval\":{}}},\"members\":[{members}]}}",
        js(SERIES_METHOD),
        js(&report.digest.to_hex()),
        js(SERIES_NO_CLAIM),
        utc(report.training_as_of_ns),
        js(selector_name(report.selector)),
        report.alpha,
        js(&report.raw.model_key),
        super::segments::metadata_field(Some(&report.metadata)),
        report.parents.len(),
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

fn cohort_markdown(out: &mut String, state: &Replay, cohort: &Cohort) {
    let text = crate::guidance::text;
    let members = cohort
        .spec
        .members
        .iter()
        .map(|id| format!("{} (`{id}`)", text(state.alias(*id))))
        .collect::<Vec<_>>()
        .join(", ");
    let _ = writeln!(
        out,
        "Frozen cohort: {} (`{}`). Complete membership: {members}.\n\nFull frozen definition and policies:\n\n```json\n{}\n```\n",
        text(&cohort.spec.name),
        cohort.digest,
        super::cohort_json(cohort),
    );
}

fn series_markdown(state: &Replay, report: &CorrectionSeriesProposal) -> String {
    let text = crate::guidance::text;
    let metadata = &report.metadata;
    let mut out = format!(
        "# Forecast correction series proposal\n\n**Unissued and uncalibrated.** {SERIES_NO_CLAIM}\n\nMethod: `{SERIES_METHOD}`. Correction digest (COR2/v1): `{}`.\n\nSaved assessments: {}. Latest training cut: {}; selector: {}.\n\nModel: {}. Family: {}. Author role: {}. Domain: {}. Horizon: {}. Site: {}. Quantity: {}. Unit: {}.\n\n## Raw, correction and corrected proposal\n\nTarget: {} (`{}`), registered {}, outcome due {}.\n\n| Quantity | Value |\n|---|---|\n| Unchanged raw Gaussian | {} |\n| Raw central interval | {} |\n| Mean log(actual/raw mean) | {} |\n| Sample log-ratio standard deviation | {} |\n| Multiplicative bias factor | {} |\n| Scale inflation after bias rescaling | {} |\n| Corrected Gaussian proposal | {} |\n| Corrected central interval | {} |\n\nIntervals use alpha {}. The bias factor is exp(mean(log(actual/raw mean))). Inflation is the inverse empirical (1-alpha) quantile of abs(actual-factor*mean)/(factor*std*z), floored at one, with outward rounding when inflated. Both corrected mean and standard deviation retain the declared unit. Zero fitted log dispersion does not collapse the predictive width. Inflation cannot narrow the bias-rescaled width; a factor below one can still shrink absolute width relative to the raw forecast.\n\n## Complete selected training population\n\nRaw hits: {}/{}. Fitted corrected hits: {}/{}; required empirical rank: {}. These are in-sample counts and do not demonstrate calibrated future coverage. Every member of each selected assessment is retained; the selected assessments do not claim to be a complete historical record.\n\n| Forecast | Raw Gaussian | Log ratio | Raw hit | Corrected interval | Corrected hit |\n|---|---|---:|---|---|---|\n",
        report.digest,
        report.parents.len(),
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
    out.push_str("\n## Saved training assessments and frozen policies\n\n");
    for (index, parent) in report.parents.iter().enumerate() {
        let _ = writeln!(
            out,
            "### Assessment {}\n\nDigest: `{}`. Cut: {}. Selector: {}.\n",
            index + 1,
            parent.assessment_digest,
            utc(parent.as_of_ns).trim_matches('"'),
            selector_name(parent.selector),
        );
        cohort_markdown(&mut out, state, &parent.cohort);
    }
    out.push_str("\n## Frozen target cohorts\n\n");
    if report.target_cohorts.is_empty() {
        out.push_str("No frozen target cohort was present for this supplied-data proposal.\n\n");
    }
    for cohort in &report.target_cohorts {
        cohort_markdown(&mut out, state, cohort);
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
                event_digest,
            );
            if let Some(event) = state
                .ledger
                .outcomes(member.raw.forecast_id)
                .iter()
                .find(|event| event.seq == *event_seq && event.digest == *event_digest)
            {
                let _ = writeln!(
                    out,
                    "  Available {}; recorded {}. Previous event digest: `{}`.",
                    utc(event.availability_time_ns).trim_matches('"'),
                    utc(event.record_time_ns).trim_matches('"'),
                    event.previous_digest,
                );
            }
            if let Some(admission) = state
                .source_admissions
                .get(&(member.raw.forecast_id, *event_seq))
            {
                let _ = writeln!(
                    out,
                    "\nRetained public source admission:\n\n```json\n{}\n```\n",
                    admission.to_json(),
                );
            }
        }
    }
    out
}
