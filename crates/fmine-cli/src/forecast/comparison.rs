//! DESIGN: a read-only presentation of paired saved public assessments, retaining both source selections.

use std::fmt::Write as _;

use fmine_calib::ForecastPayload;
use fmine_calib::ledger::comparison::{PairedAssessment, PairedScore};
use fmine_numerics::exact_feas::Rat;

use super::{Replay, absent, js, selector_name, utc};
use fmine_evidence::Refusal;

pub(crate) const ROBOT_DOCS_JSON: &str = r#"{"command":"forecast journal compare --journal <path> --left <assessment-digest> --right <assessment-digest> [--score interval|crps] [--head <hex>]","inputs":"Two already persisted assessments in the same journal; no clock sampling, new assessment, outcome selection or journal append","compatibility":"Same frozen public issuer, metric, unit, ownership, calendar period, vintage permission, alpha, as-of cut and outcome selector","pairing":"Union of frozen subjects, with paired numeric scores only for identical outcome values and citations; missing forecasts, unresolved outcomes and selection mismatches stay visible","score":"Default interval: unchanged exact rational interval loss and PAIR/v1 identity. Opt-in crps: exact full-distribution CRPS for empirical and weighted_empirical forecasts only; other distributions refuse even for unmatched subjects. Both use left minus right, with negative favoring left, and observed means on identical-publication pairs only","completion":"Exact global finite loss-difference bounds for unresolved pairs. CRPS sweeps the union of support atoms: the difference is linear between atoms and constant outside both supports, without outcome-domain assumptions. Exact weights are never normalized or resampled. Whole-union averages are absent if any subject lacks a forecast or compatible selection, or checked arithmetic is unrepresentable","identity":"CRPS uses fmine.forecast-crps-comparison.v1 and PCRP/v1, retaining every frozen distribution and exact weight, selected event and source citation. Default interval schema and digests remain unchanged","interpretation":"Descriptive arithmetic bounds, not confidence intervals, sequential significance, calibration or predictive skill; model information sets may differ; original assessments and registration identities remain unchanged"}"#;

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
        let intervals = if report.score == PairedScore::Interval {
            format!(",\"left_interval\":{},\"right_interval\":{}", interval(member.left_interval), interval(member.right_interval))
        } else {
            String::new()
        };
        Ok(format!(
            "{{\"subject\":{},\"status\":{},\"left\":{left},\"right\":{right}{intervals},\"difference\":{},\"completion\":{},\"arithmetic_indeterminate\":{}}}",
            js(&member.subject), js(member.status.as_str()), optional(member.difference), bounds(member.completion),
            member.arithmetic_indeterminate,
        ))
    }).collect::<Result<Vec<_>, Refusal>>()?.join(",");
    let schema = match report.score {
        PairedScore::Interval => "fmine.forecast-comparison.v1",
        PairedScore::Crps => "fmine.forecast-crps-comparison.v1",
    };
    Ok(format!(
        "{{\"schema\":\"{schema}\",\"digest\":{},\"left_assessment\":{},\"right_assessment\":{},\"left_cohort\":{},\"right_cohort\":{},\"issuer\":{},\"metric\":{},\"unit\":{},\"ownership\":{},\"period\":{},\"alpha\":{},\"as_of\":{},\"selector\":{},\"method\":{},\"n_members\":{},\"n_paired\":{},\"n_unresolved\":{},\"n_unmatched\":{},\"n_arithmetic_indeterminate\":{},\"observed_status\":{},\"observed_mean_difference\":{},\"completion_status\":{},\"whole_population\":{},\"members\":[{members}]}}",
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
        js(report.score.method()),
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
    let title = match report.score {
        PairedScore::Interval => "Paired forecast assessment",
        PairedScore::Crps => "Paired full-distribution CRPS assessment",
    };
    let method = report.score.method();
    let alpha_label = match report.score {
        PairedScore::Interval => "Interval alpha",
        PairedScore::Crps => "Frozen alpha (compatibility only; CRPS uses all probability mass)",
    };
    let mut out = format!(
        "# {title}\n\n{method}\n\nComparison: `{}`\n\nLeft assessment: `{}`\n\nRight assessment: `{}`\n\nTarget: {} / {} / {} / {} / {}.\n\nAssessment cut: {}. Outcome selector: {}. {alpha_label}: {}.\n\nPaired outcomes: {}. Unresolved pairs: {}. Unmatched subjects or selections: {}.\n\nObserved mean difference: {} ({}).\n\nWhole-population completion: {} ({}).\n\n| Subject | Status | Left minus right | Completion |\n|---|---|---:|---|\n",
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

pub(super) fn markdown_crps(state: &Replay, report: &PairedAssessment) -> Result<String, Refusal> {
    let mut out = markdown(report);
    out.push_str("\n## Frozen predictive distributions\n");
    for member in &report.members {
        for (side, selected) in [("Left", &member.left), ("Right", &member.right)] {
            let Some(selected) = selected else {
                continue;
            };
            let forecast = state
                .ledger
                .forecast(selected.forecast_id)
                .ok_or_else(|| absent("paired CRPS forecast missing"))?;
            let _ = writeln!(
                out,
                "\n### {side} {}\n\nForecast `{}`; original atom order and exact probability mass.\n\n| Value | Probability |\n|---:|---:|",
                crate::guidance::text(&member.subject),
                selected.forecast_id
            );
            match &forecast.payload {
                ForecastPayload::Empirical { samples } => {
                    for value in samples {
                        let _ = writeln!(out, "| {value} | 1/{} |", samples.len());
                    }
                }
                ForecastPayload::WeightedEmpirical { samples, weights } => {
                    for (value, weight) in samples.iter().zip(weights) {
                        let _ = writeln!(out, "| {value} | {} |", display(*weight));
                    }
                }
                _ => return Err(super::invalid("paired CRPS distribution is unsupported")),
            }
        }
    }
    Ok(out)
}
