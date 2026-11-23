//! DESIGN: read-only paired saved assessments with explicit public or supplied target admission.

use std::fmt::Write as _;

use fmine_calib::ForecastPayload;
use fmine_calib::ledger::comparison::gaussian::{
    GaussianPairedAssessment, METHOD as GAUSSIAN_METHOD,
};
use fmine_calib::ledger::comparison::supplied::SuppliedPairedAssessment;
use fmine_calib::ledger::comparison::{CrpsComparison, PairedAssessment, PairedScore};
use fmine_numerics::exact_feas::Rat;

use super::{Replay, absent, js, optional_number, selector_name, utc};
use fmine_evidence::Refusal;

pub(crate) const ROBOT_DOCS_JSON: &str = r#"{"command":"forecast journal compare --journal <path> --left <assessment-digest> --right <assessment-digest> [--score interval|crps|brier] [--targets public|supplied] [--head <hex>]","inputs":"Two already persisted assessments in the same journal; no clock sampling, new assessment, outcome selection or journal append","compatibility":"Default public: same frozen public issuer, metric, unit, ownership, calendar period, vintage permission, alpha, as-of cut and outcome selector","supplied_targets":"Explicit --targets supplied requires complete prospectively frozen metadata, identical resolution rule, eligible sources, quantity, unit, alpha, saved cut and selector. Pairing uses the unique full domain/site/horizon/quantity/unit tuple, with identical outcome deadlines for matching targets. Duplicate targets, legacy untagged forecasts and public-bound cohorts refuse; model/author labels do not establish physical identity. Target declarations and citations remain supplied and unverified","pairing":"Union of frozen targets, with paired numeric scores only for identical outcome values and complete citations; missing forecasts, unresolved outcomes and selection mismatches stay visible","score":"Default interval: unchanged exact rational interval loss and PAIR/v1 identity. Opt-in crps: exact full-distribution CRPS for empirical and weighted_empirical forecasts. Public comparisons additionally support homogeneous Gaussian forecasts under a separately typed approximate Gaussian comparison. Supplied CRPS requires complete finite distributions for every member, including unmatched forecasts. Opt-in brier requires explicit --targets supplied and complete Bernoulli forecasts throughout both cohorts. All use left minus right, with negative favoring left","completion":"Finite CRPS uses exact global extrema from a union-of-support CDF sweep. Public Gaussian CRPS uses approximate analytic extrema at both infinite-tail limits and the unique CDF crossing, or monotonicity at equal scales; these are not certified numerical enclosures. Neither needs an invented outcome domain. Whole-union averages are absent if any target lacks a forecast or compatible selection, or required arithmetic is unrepresentable","identity":"Public interval PAIR/v1, finite CRPS PCRP/v1 and Gaussian GCRP/v1 schemas and digests are unchanged. The explicit supplied lane uses fmine.forecast-supplied-comparison.v1 with SINT/v1, fmine.forecast-supplied-crps-comparison.v1 with SCRP/v1 or fmine.forecast-supplied-brier-comparison.v1 with SBRI/v1. Every frozen distribution, selected event and source citation remains available; no durable journal format changes","interpretation":"Descriptive arithmetic completion diagnostics, not confidence intervals, sequential significance, calibration or predictive skill; model information sets may differ; original assessments and registration identities remain unchanged","brier":"Exact Brier loss on original frozen Bernoulli probabilities, including zero and one; no log-score floor or clipping changes them. For unresolved compatible pairs, loss differences are affine in the binary outcome and their exact extrema occur at zero or one. Summed extrema give sharp full-population bounds over all binary completions. Equal forecasts contribute exactly zero, including deterministic events; unrepresentable checked arithmetic remains explicit"}"#;

pub(super) fn crps(
    state: &Replay,
    left: fmine_core::Digest32,
    right: fmine_core::Digest32,
) -> Result<(String, String), Refusal> {
    match state.ledger.compare_assessments_full_crps(left, right)? {
        CrpsComparison::Finite(report) => {
            Ok((json(state, &report)?, markdown_crps(state, &report)?))
        }
        CrpsComparison::Gaussian(report) => {
            Ok((gaussian_json(state, &report)?, gaussian_markdown(&report)))
        }
    }
}

pub(super) fn supplied(
    state: &Replay,
    left: fmine_core::Digest32,
    right: fmine_core::Digest32,
    score: PairedScore,
) -> Result<(String, String), Refusal> {
    let report = state.ledger.compare_supplied_assessments(left, right, score)?;
    Ok((supplied_json(state, &report)?, supplied_markdown(state, &report)?))
}

fn supplied_statuses(report: &SuppliedPairedAssessment) -> (&'static str, &'static str) {
    let observed = if report.n_paired == 0 {
        "no_paired_outcomes"
    } else if report.observed_mean_difference.is_some() {
        "exact"
    } else {
        "arithmetic_indeterminate"
    };
    let completion = if report.n_unmatched > 0 {
        "unmatched_population_or_outcome_selection"
    } else if report.whole_population.is_some() {
        "exact"
    } else {
        "arithmetic_indeterminate"
    };
    (observed, completion)
}

fn supplied_json(state: &Replay, report: &SuppliedPairedAssessment) -> Result<String, Refusal> {
    let members = report.members.iter().map(|member| {
        let pair = &member.pair;
        let target = &member.target;
        let left = pair.left.as_ref().map(|m| state.member_json(m)).transpose()?.unwrap_or_else(|| "null".into());
        let right = pair.right.as_ref().map(|m| state.member_json(m)).transpose()?.unwrap_or_else(|| "null".into());
        let intervals = if report.score == PairedScore::Interval {
            format!(",\"left_interval\":{},\"right_interval\":{}", interval(pair.left_interval), interval(pair.right_interval))
        } else {
            String::new()
        };
        Ok(format!(
            "{{\"target\":{{\"domain\":{},\"site\":{},\"horizon\":{},\"quantity\":{},\"unit\":{},\"outcome_due\":{}}},\"status\":{},\"left\":{left},\"right\":{right}{intervals},\"difference\":{},\"completion\":{},\"arithmetic_indeterminate\":{}}}",
            js(&target.domain), js(&target.site), js(&target.horizon), js(&target.quantity), js(&target.unit),
            utc(member.outcome_due_ns), js(pair.status.as_str()), optional(pair.difference), bounds(pair.completion),
            pair.arithmetic_indeterminate,
        ))
    }).collect::<Result<Vec<_>, Refusal>>()?.join(",");
    let sources = report
        .eligible_sources
        .iter()
        .map(|source| js(source))
        .collect::<Vec<_>>()
        .join(",");
    let (schema, score) = match report.score {
        PairedScore::Interval => ("fmine.forecast-supplied-comparison.v1", "interval"),
        PairedScore::Crps => ("fmine.forecast-supplied-crps-comparison.v1", "crps"),
        PairedScore::Brier => ("fmine.forecast-supplied-brier-comparison.v1", "brier"),
    };
    let (observed_status, completion_status) = supplied_statuses(report);
    Ok(format!(
        "{{\"schema\":\"{schema}\",\"targets\":\"supplied\",\"source_citations\":\"supplied_not_verified\",\"score\":\"{score}\",\"digest\":{},\"left_assessment\":{},\"right_assessment\":{},\"left_cohort\":{},\"right_cohort\":{},\"resolution_rule\":{},\"eligible_sources\":[{sources}],\"quantity\":{},\"unit\":{},\"alpha\":{},\"as_of\":{},\"selector\":{},\"method\":{},\"n_members\":{},\"n_paired\":{},\"n_unresolved\":{},\"n_unmatched\":{},\"n_arithmetic_indeterminate\":{},\"observed_status\":{},\"observed_mean_difference\":{},\"completion_status\":{},\"whole_population\":{},\"members\":[{members}]}}",
        js(&report.digest.to_hex()),
        js(&report.left_assessment.to_hex()),
        js(&report.right_assessment.to_hex()),
        js(&report.left_cohort.to_hex()),
        js(&report.right_cohort.to_hex()),
        js(&report.resolution_rule),
        js(&report.quantity),
        js(&report.unit),
        report.alpha,
        utc(report.as_of_ns),
        js(selector_name(report.selector)),
        js(report.method()),
        report.members.len(),
        report.n_paired,
        report.n_unresolved,
        report.n_unmatched,
        report.n_arithmetic_indeterminate,
        js(observed_status),
        optional(report.observed_mean_difference),
        js(completion_status),
        bounds(report.whole_population),
    ))
}

fn supplied_markdown(
    state: &Replay,
    report: &SuppliedPairedAssessment,
) -> Result<String, Refusal> {
    let text = crate::guidance::text;
    let score = match report.score {
        PairedScore::Interval => "central interval loss",
        PairedScore::Crps => "complete finite-distribution CRPS",
        PairedScore::Brier => "binary-event Brier loss on frozen probabilities",
    };
    let (observed_status, completion_status) = supplied_statuses(report);
    let method = report.method();
    let mut out = format!(
        "# Paired supplied-target assessment\n\n{method}\n\nScore: **{score}**. Source citations: **supplied, not verified**.\n\nComparison: `{}`.\n\nLeft assessment: `{}`. Right assessment: `{}`.\n\nFrozen rule: {}. Eligible sources: {}. Quantity: {} ({}).\n\nAssessment cut: {}. Selector: {}. Frozen alpha: {}.\n\nPaired outcomes: {}. Unresolved pairs: {}. Unmatched targets or selections: {}. Arithmetic indeterminate: {}.\n\nObserved mean difference: {} ({}).\n\nWhole-population completion: {} ({}).\n\n| Site | Domain | Horizon | Outcome due | Status | Left minus right | Completion |\n|---|---|---|---|---|---:|---|\n",
        report.digest,
        report.left_assessment,
        report.right_assessment,
        text(&report.resolution_rule),
        text(&report.eligible_sources.join(", ")),
        text(&report.quantity),
        text(&report.unit),
        utc(report.as_of_ns).trim_matches('"'),
        selector_name(report.selector),
        report.alpha,
        report.n_paired,
        report.n_unresolved,
        report.n_unmatched,
        report.n_arithmetic_indeterminate,
        report
            .observed_mean_difference
            .map_or_else(|| "unavailable".into(), display),
        observed_status,
        report.whole_population.map_or_else(
            || "unavailable".into(),
            |(lo, hi)| format!("[{}, {}]", display(lo), display(hi))
        ),
        completion_status,
    );
    for member in &report.members {
        let pair = &member.pair;
        let _ = writeln!(
            out,
            "| {} | {} | {} | {} | {}{} | {} | {} |",
            text(&member.target.site),
            text(&member.target.domain),
            text(&member.target.horizon),
            utc(member.outcome_due_ns).trim_matches('"'),
            pair.status.as_str(),
            if pair.arithmetic_indeterminate {
                "; arithmetic indeterminate"
            } else {
                ""
            },
            pair.difference.map_or_else(|| "—".into(), display),
            pair.completion.map_or_else(
                || "unavailable".into(),
                |(lo, hi)| format!("[{}, {}]", display(lo), display(hi))
            ),
        );
    }
    out.push_str("\n## Frozen distributions and supplied source evidence\n");
    for member in &report.members {
        for (side, selected) in [("Left", &member.pair.left), ("Right", &member.pair.right)] {
            if let Some(selected) = selected {
                let _ = writeln!(
                    out,
                    "\n### {side} {}\n\nComplete frozen forecast, metadata and selected event; citations remain supplied and unverified.\n\n```json\n{}\n```\n",
                    text(&member.pair.subject),
                    state.member_json(selected)?,
                );
            }
        }
    }
    Ok(out)
}

fn gaussian_bounds(value: Option<(f64, f64)>) -> String {
    value.map_or_else(
        || "null".into(),
        |(lo, hi)| format!("{{\"lower\":{lo},\"upper\":{hi}}}"),
    )
}

fn gaussian_status(present: bool, no_values: bool) -> &'static str {
    if no_values {
        "no_paired_outcomes"
    } else if present {
        "approximate"
    } else {
        "arithmetic_indeterminate"
    }
}

fn gaussian_json(state: &Replay, report: &GaussianPairedAssessment) -> Result<String, Refusal> {
    let members = report.members.iter().map(|member| {
        let left = member.left.as_ref().map(|m| state.member_json(m)).transpose()?.unwrap_or_else(|| "null".into());
        let right = member.right.as_ref().map(|m| state.member_json(m)).transpose()?.unwrap_or_else(|| "null".into());
        Ok(format!(
            "{{\"subject\":{},\"status\":{},\"left\":{left},\"right\":{right},\"difference\":{},\"completion\":{},\"arithmetic_indeterminate\":{}}}",
            js(&member.subject), js(member.status.as_str()), optional_number(member.difference),
            gaussian_bounds(member.completion), member.arithmetic_indeterminate,
        ))
    }).collect::<Result<Vec<_>, Refusal>>()?.join(",");
    let completion_status = if report.n_unmatched > 0 {
        "unmatched_population_or_outcome_selection"
    } else {
        gaussian_status(report.whole_population.is_some(), false)
    };
    Ok(format!(
        "{{\"schema\":\"fmine.forecast-gaussian-crps-comparison.v1\",\"arithmetic\":\"approximate\",\"digest\":{},\"left_assessment\":{},\"right_assessment\":{},\"left_cohort\":{},\"right_cohort\":{},\"issuer\":{},\"metric\":{},\"unit\":{},\"ownership\":{},\"period\":{},\"alpha\":{},\"as_of\":{},\"selector\":{},\"method\":{},\"n_members\":{},\"n_paired\":{},\"n_unresolved\":{},\"n_unmatched\":{},\"n_arithmetic_indeterminate\":{},\"observed_status\":{},\"observed_mean_difference\":{},\"completion_status\":{},\"whole_population\":{},\"members\":[{members}]}}",
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
        js(GAUSSIAN_METHOD),
        report.members.len(),
        report.n_paired,
        report.n_unresolved,
        report.n_unmatched,
        report.n_arithmetic_indeterminate,
        js(gaussian_status(
            report.observed_mean_difference.is_some(),
            report.n_paired == 0
        )),
        optional_number(report.observed_mean_difference),
        js(completion_status),
        gaussian_bounds(report.whole_population),
    ))
}

fn gaussian_markdown(report: &GaussianPairedAssessment) -> String {
    let text = crate::guidance::text;
    let display =
        |value: Option<f64>| value.map_or_else(|| "unavailable".into(), |x| x.to_string());
    let mut out = format!(
        "# Paired Gaussian CRPS assessment\n\n{GAUSSIAN_METHOD}\n\nArithmetic: **approximate**. The completion extrema are not certified numerical enclosures.\n\nComparison: `{}`\n\nLeft assessment: `{}`\n\nRight assessment: `{}`\n\nTarget: {} / {} / {} / {} / {}.\n\nAssessment cut: {}. Outcome selector: {}. Frozen alpha: {} (compatibility only; CRPS uses the complete distribution).\n\nPaired outcomes: {}. Unresolved pairs: {}. Unmatched subjects or selections: {}. Arithmetic indeterminate: {}.\n\nObserved mean difference: {}.\n\nWhole-population completion: {}.\n\n| Subject | Status | Left minus right | Completion |\n|---|---|---:|---|\n",
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
        report.n_arithmetic_indeterminate,
        display(report.observed_mean_difference),
        report
            .whole_population
            .map_or_else(|| "unavailable".into(), |(lo, hi)| format!("[{lo}, {hi}]")),
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
            display(member.difference),
            member
                .completion
                .map_or_else(|| "unavailable".into(), |(lo, hi)| format!("[{lo}, {hi}]"))
        );
    }
    out.push_str("\n## Frozen Gaussian distributions and selected source evidence\n");
    for member in &report.members {
        for (side, parameters, selected) in [
            ("Left", member.left_parameters, member.left.as_ref()),
            ("Right", member.right_parameters, member.right.as_ref()),
        ] {
            if let Some(parameters) = parameters {
                let _ = writeln!(
                    out,
                    "\n{side} {}: Gaussian mean {}, standard deviation {} ({}).",
                    text(&member.subject),
                    parameters.mean,
                    parameters.std_dev,
                    text(&report.unit)
                );
            }
            if let Some(selected) = selected {
                let _ = writeln!(out, "Forecast `{}`.", selected.forecast_id);
                if let super::MemberOutcome::Resolved { citation, .. } = &selected.outcome {
                    let _ = writeln!(
                        out,
                        "{} / {} / {}; SHA-256 `{}`.",
                        text(&citation.source),
                        text(&citation.vintage),
                        text(&citation.locator),
                        fmine_core::Digest32(citation.sha256)
                    );
                }
            }
        }
    }
    out
}

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
        PairedScore::Brier => {
            return Err(super::invalid(
                "Brier comparison requires explicit supplied targets",
            ));
        }
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
        PairedScore::Brier => "Paired binary-event Brier assessment",
    };
    let method = report.score.method();
    let alpha_label = match report.score {
        PairedScore::Interval => "Interval alpha",
        PairedScore::Crps => "Frozen alpha (compatibility only; CRPS uses all probability mass)",
        PairedScore::Brier => {
            "Frozen alpha (compatibility only; Brier uses binary event probabilities)"
        }
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
