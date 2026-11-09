//! DESIGN (BD-051): presentation of the prospectively frozen Gaussian residual CUSUM.
//! Source admission and the original-publication sequence remain owned by the ledger.

use std::fmt::Write as _;

use fmine_calib::ledger::{CusumMonitorAssessment, CusumMonitorPolicy};
use fmine_numerics::eproc::cusum::CusumSnapshot;

use super::{Assessment, MemberOutcome, Refusal, Replay, Row, display_or, js, missing_name};

pub(super) const FIELDS: [&str; 6] = [
    "cusum_reference_value",
    "cusum_threshold",
    "cusum_support",
    "cusum_reference_assumption",
    "cusum_tuning_basis",
    "cusum_order",
];

pub(super) const ROBOT_DOCS_JSON: &str = r#"{"cohort_fields":["cusum_reference_value","cusum_threshold","cusum_support","cusum_reference_assumption","cusum_tuning_basis","cusum_order"],"declaration":"All six fields required together before outcomes; positive finite reference value k and threshold h; complete comma-separated forecast alias order","payloads":"Gaussian only; residual z=(original outcome-mean)/std_dev","recurrence":"S_plus=max(0,S_plus+z-k); S_minus=max(0,S_minus-z-k); alarm at either sum >= h","sequence":"Contiguous original publications in frozen order; missing members block later observations; restatements never erase an alarm","evidence":"Each original outcome, citation, event digest, residual, two directional sums, maxima and first crossings are retained","interpretation":"Declared tuning and reference assumptions are unverified. No false-alarm probability, average run length, anytime guarantee, causal diagnosis or recalibration is inferred","journal":"Opt-in cohort freeze uses operation v8; compatible coverage and finite-domain policies remain frozen; omission preserves existing bytes","example":"tests/forecasts/cusum_synthetic.toml"}"#;

pub(super) fn parse(row: &Row<'_>, state: &Replay) -> Result<Option<CusumMonitorPolicy>, Refusal> {
    if !FIELDS.iter().any(|key| row.0.get(key).is_some()) {
        return Ok(None);
    }
    Ok(Some(CusumMonitorPolicy {
        reference_value: row.number("cusum_reference_value")?,
        threshold: row.number("cusum_threshold")?,
        support: row.string("cusum_support")?.to_string(),
        reference_assumption: row.string("cusum_reference_assumption")?.to_string(),
        tuning_basis: row.string("cusum_tuning_basis")?.to_string(),
        order: row
            .identifiers("cusum_order")?
            .iter()
            .map(|alias| state.forecast_id(alias))
            .collect::<Result<Vec<_>, _>>()?,
    }))
}

pub(super) fn policy_json(p: &CusumMonitorPolicy) -> String {
    format!(
        "{{\"reference_value\":{},\"threshold\":{},\"support\":{},\"reference_assumption\":{},\"tuning_basis\":{},\"order\":[{}]}}",
        p.reference_value,
        p.threshold,
        js(&p.support),
        js(&p.reference_assumption),
        js(&p.tuning_basis),
        p.order
            .iter()
            .map(|id| js(&id.to_string()))
            .collect::<Vec<_>>()
            .join(","),
    )
}

fn snapshot_json(s: CusumSnapshot) -> String {
    format!(
        "{{\"n\":{},\"upper\":{},\"lower\":{},\"max_upper\":{},\"max_lower\":{},\"threshold\":{},\"first_upper_crossing\":{},\"first_lower_crossing\":{},\"first_crossing\":{},\"alarmed\":{}}}",
        s.n,
        s.upper,
        s.lower,
        s.max_upper,
        s.max_lower,
        s.threshold,
        display_or(s.first_upper_crossing, "null"),
        display_or(s.first_lower_crossing, "null"),
        display_or(s.first_crossing(), "null"),
        s.alarmed(),
    )
}

pub(super) fn assessment_json(state: &Replay, report: &Assessment) -> Result<String, Refusal> {
    let Some(m) = &report.cusum_monitor else {
        return Ok(String::new());
    };
    let observations = m.observations.iter().map(|o| {
        Ok(format!(
            "{{\"member\":{},\"mean\":{},\"std_dev\":{},\"standardized_residual\":{},\"snapshot\":{}}}",
            state.member_json(&o.member)?, o.mean, o.std_dev, o.standardized_residual, snapshot_json(o.snapshot),
        ))
    }).collect::<Result<Vec<_>, Refusal>>()?.join(",");
    let blocked = m
        .blocked_by
        .as_ref()
        .map(|b| state.member_json(b))
        .transpose()?
        .unwrap_or_else(|| "null".into());
    Ok(format!(
        ",\"cusum_monitor\":{{\"policy\":{},\"selector\":\"first-published\",\"assumption_verified\":false,\"tuning_verified\":false,\"false_alarm_probability\":null,\"average_run_length\":null,\"snapshot\":{},\"blocked_by\":{blocked},\"n_deferred\":{},\"n_deferred_resolved\":{},\"observations\":[{observations}]}}",
        policy_json(&m.policy),
        snapshot_json(m.snapshot),
        m.n_deferred,
        m.n_deferred_resolved,
    ))
}

pub(super) fn markdown(state: &Replay, m: &CusumMonitorAssessment) -> String {
    let text = |s: &str| {
        s.replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
            .replace('|', "&#124;")
            .replace('`', "&#96;")
            .replace(['\r', '\n'], " ")
    };
    let s = m.snapshot;
    let mut out = format!(
        "\n## Gaussian residual CUSUM\n\nDeclared support: {}. Reference assumption: {}. Tuning basis: {}.\n\nReference value k: {}; threshold h: {}. Residual z = (original outcome - frozen mean) / frozen standard deviation. The two sums follow max(0, previous sum + z - k) and max(0, previous sum - z - k); either alarms at h.\n\nOriginal observations: {}. Upper/lower sums: {} / {}; maxima: {} / {}. First upper/lower crossing: {} / {}. Alarmed: {}.\n\nTuning and reference assumptions are supplied, not verified. No false-alarm probability, average run length, anytime guarantee, causal diagnosis or recalibration is established. Restatements do not replace the original sequence or erase an alarm.\n\n| Order | Forecast | Mean / standard deviation | Original outcome and citation | Residual | Upper / lower | Event digest |\n|---|---|---|---|---|---|---|\n",
        text(&m.policy.support),
        text(&m.policy.reference_assumption),
        text(&m.policy.tuning_basis),
        m.policy.reference_value,
        m.policy.threshold,
        s.n,
        s.upper,
        s.lower,
        s.max_upper,
        s.max_lower,
        display_or(s.first_upper_crossing, "none"),
        display_or(s.first_lower_crossing, "none"),
        s.alarmed(),
    );
    for (i, o) in m.observations.iter().enumerate() {
        if let MemberOutcome::Resolved {
            value, citation, ..
        } = &o.member.outcome
        {
            let event = o
                .member
                .selected_event
                .map_or_else(|| "none".into(), |(_, d)| d.to_hex());
            let _ = writeln!(
                out,
                "| {} | {} | {} / {} | {}; {} / {} / {}; SHA-256 `{}` | {} | {} / {} | `{event}` |",
                i + 1,
                text(state.alias(o.member.forecast_id)),
                o.mean,
                o.std_dev,
                value,
                text(&citation.source),
                text(&citation.vintage),
                text(&citation.locator),
                fmine_core::hash::Digest32(citation.sha256).to_hex(),
                o.standardized_residual,
                o.snapshot.upper,
                o.snapshot.lower,
            );
        }
    }
    if let Some(b) = &m.blocked_by {
        let reason = match &b.outcome {
            MemberOutcome::Unresolved { reason } => missing_name(*reason),
            MemberOutcome::Resolved { .. } => "resolved",
        };
        let event = b
            .selected_event
            .map_or_else(|| "none".into(), |(_, d)| d.to_hex());
        let _ = writeln!(
            out,
            "\nSequence stops at unresolved forecast {} (reason: {}; event digest: `{event}`). {} later members are deferred, including {} with resolved original publications.",
            text(state.alias(b.forecast_id)),
            reason,
            m.n_deferred,
            m.n_deferred_resolved,
        );
    }
    out
}
