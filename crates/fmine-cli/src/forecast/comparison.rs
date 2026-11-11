//! DESIGN: a read-only presentation of paired saved public assessments, retaining both source selections.

use std::fmt::Write as _;

use fmine_calib::ledger::comparison::{METHOD, PairedAssessment};
use fmine_numerics::exact_feas::Rat;

use super::{Replay, js, selector_name, utc};
use fmine_evidence::Refusal;

pub(crate) const ROBOT_DOCS_JSON: &str = r#"{"command":"forecast journal compare --journal <path> --left <assessment-digest> --right <assessment-digest> [--head <hex>]","inputs":"Two already persisted assessments in the same journal; no clock sampling, new assessment, outcome selection or journal append","compatibility":"Same frozen public issuer, metric, unit, ownership, calendar period, vintage permission, alpha, as-of cut and outcome selector","pairing":"Union of frozen subjects, with paired numeric scores only for identical outcome values and citations; missing forecasts, unresolved outcomes and selection mismatches stay visible","score":"Exact rational interval loss, left minus right, from the saved binary values and central-interval endpoints; negative favors left; observed mean on the identical-publication pairs only","completion":"Exact global finite loss-difference bounds for unresolved pairs with common alpha; whole-union average absent if any subject lacks a forecast or compatible selection, or arithmetic is unrepresentable","interpretation":"Descriptive only, no predictive-skill claim or guarantee of identical model information sets; original assessments and registration identities remain unchanged"}"#;

fn rat(value: Rat) -> String {
    format!(
        "{{\"num\":\"{}\",\"den\":\"{}\"}}",
        value.num(),
        value.den()
    )
}

fn optional(value: Option<Rat>) -> String {
    value.map_or_else(|| "null".into(), rat)
}

fn bounds(value: Option<(Rat, Rat)>) -> String {
    value.map_or_else(
        || "null".into(),
        |(lo, hi)| format!("{{\"lower\":{},\"upper\":{}}}", rat(lo), rat(hi)),
    )
}

fn interval(value: Option<(f64, f64)>) -> String {
    value.map_or_else(|| "null".into(), |(lo, hi)| format!("[{lo},{hi}]"))
}

fn mean_status(report: &PairedAssessment) -> &'static str {
    if report.n_paired == 0 {
        "no_paired_outcomes"
    } else if report.observed_mean_difference.is_some() {
        "exact"
    } else {
        "arithmetic_indeterminate"
    }
}

fn completion_status(report: &PairedAssessment) -> &'static str {
    if report.n_unmatched > 0 {
        "unmatched_population_or_outcome_selection"
    } else if report.whole_population.is_some() {
        "exact"
    } else {
        "arithmetic_indeterminate"
    }
}

pub(super) fn json(state: &Replay, report: &PairedAssessment) -> Result<String, Refusal> {
    let members = report.members.iter().map(|member| {
        let left = member.left.as_ref().map(|m| state.member_json(m)).transpose()?.unwrap_or_else(|| "null".into());
        let right = member.right.as_ref().map(|m| state.member_json(m)).transpose()?.unwrap_or_else(|| "null".into());
        Ok(format!(
            "{{\"subject\":{},\"status\":{},\"left\":{left},\"right\":{right},\"left_interval\":{},\"right_interval\":{},\"difference\":{},\"completion\":{},\"arithmetic_indeterminate\":{}}}",
            js(&member.subject), js(member.status.as_str()), interval(member.left_interval),
            interval(member.right_interval), optional(member.difference), bounds(member.completion),
            member.arithmetic_indeterminate,
        ))
    }).collect::<Result<Vec<_>, Refusal>>()?.join(",");
    Ok(format!(
        "{{\"schema\":\"fmine.forecast-comparison.v1\",\"digest\":{},\"left_assessment\":{},\"right_assessment\":{},\"left_cohort\":{},\"right_cohort\":{},\"issuer\":{},\"metric\":{},\"unit\":{},\"ownership\":{},\"period\":{},\"alpha\":{},\"as_of\":{},\"selector\":{},\"method\":{},\"n_members\":{},\"n_paired\":{},\"n_unresolved\":{},\"n_unmatched\":{},\"n_arithmetic_indeterminate\":{},\"observed_status\":{},\"observed_mean_difference\":{},\"completion_status\":{},\"whole_population\":{},\"members\":[{members}]}}",
        js(&report.digest.to_hex()),
        js(&report.left_assessment.to_hex()),
        js(&report.right_assessment.to_hex()),
        js(&report.left_cohort.to_hex()),
        js(&report.right_cohort.to_hex()),
        js(&report.issuer),
        js(&report.metric),
        js(&report.unit),
        js(&report.ownership),
        js(&report.period),
        report.alpha,
        utc(report.as_of_ns),
        js(selector_name(report.selector)),
        js(METHOD),
        report.members.len(),
        report.n_paired,
        report.n_unresolved,
        report.n_unmatched,
        report.n_arithmetic_indeterminate,
        js(mean_status(report)),
        optional(report.observed_mean_difference),
        js(completion_status(report)),
        bounds(report.whole_population),
    ))
}

fn display(value: Rat) -> String {
    if value.den() == 1 {
        value.num().to_string()
    } else {
        format!("{}/{}", value.num(), value.den())
    }
}

pub(super) fn markdown(report: &PairedAssessment) -> String {
    let text = crate::guidance::text;
    let mut out = format!(
        "# Paired forecast assessment\n\n{METHOD}\n\nComparison: `{}`\n\nLeft assessment: `{}`\n\nRight assessment: `{}`\n\nTarget: {} / {} / {} / {} / {}.\n\nAssessment cut: {}. Outcome selector: {}. Interval alpha: {}.\n\nPaired outcomes: {}. Unresolved pairs: {}. Unmatched subjects or selections: {}.\n\nObserved mean difference: {} ({}).\n\nWhole-population completion: {} ({}).\n\n| Subject | Status | Left minus right | Completion |\n|---|---|---:|---|\n",
        report.digest,
        report.left_assessment,
        report.right_assessment,
        text(&report.issuer),
        text(&report.metric),
        text(&report.unit),
        text(&report.ownership),
        text(&report.period),
        utc(report.as_of_ns).trim_matches('"'),
        selector_name(report.selector),
        report.alpha,
        report.n_paired,
        report.n_unresolved,
        report.n_unmatched,
        report
            .observed_mean_difference
            .map_or_else(|| "unavailable".into(), display),
        mean_status(report),
        report.whole_population.map_or_else(
            || "unavailable".into(),
            |(lo, hi)| format!("[{}, {}]", display(lo), display(hi))
        ),
        completion_status(report),
    );
    for member in &report.members {
        let _ = writeln!(
            out,
            "| {} | {}{} | {} | {} |",
            text(&member.subject),
            member.status.as_str(),
            if member.arithmetic_indeterminate {
                "; arithmetic indeterminate"
            } else {
                ""
            },
            member.difference.map_or_else(|| "—".into(), display),
            member.completion.map_or_else(
                || "unavailable".into(),
                |(lo, hi)| format!("[{}, {}]", display(lo), display(hi))
            )
        );
    }
    out.push_str("\n## Selected source evidence\n");
    for member in &report.members {
        // Retain both citations in human output as in JSON; equality alone is not matching evidence.
        for (side, selected) in [("Left", &member.left), ("Right", &member.right)] {
            if let Some(selected) = selected
                && let super::MemberOutcome::Resolved { citation, .. } = &selected.outcome
            {
                let _ = writeln!(
                    out,
                    "\n{side} {}: forecast `{}`, {} / {} / {}; SHA-256 `{}`.\n",
                    text(&member.subject),
                    selected.forecast_id,
                    text(&citation.source),
                    text(&citation.vintage),
                    text(&citation.locator),
                    fmine_core::Digest32(citation.sha256)
                );
            }
        }
    }
    out
}
