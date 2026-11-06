//! DESIGN (BD-048/049): display the saved assessment's descriptive diagnostics with its complete membership.
//! Determinism: Exact rendering of kernel-owned values; no outcome selection or statistical inference here.

use std::fmt::Write as _;

use fmine_calib::Assessment;
use fmine_calib::ledger::diagnostics::{
    AssessmentDiagnostics, BIN_RULE, DiagnosticValue, PIT_METHOD, SCOPE,
};
use fmine_evidence::Refusal;

use super::{Replay, js, optional_number, pair, selector_name, utc};

pub(super) const ROBOT_DOCS_JSON: &str = r#"{"scope":"Read-only descriptive projection of a saved assessment; no new outcomes or authority time","gaussian":"Per-member PIT and fixed-decile histogram via the existing normal CDF, retaining approximate values and tail saturation","bernoulli":"Fixed probability bins with full and resolved mean probabilities, observed event frequencies and unresolved-outcome completion bounds","bins":"[lower,upper), with 1 included in the final bin; exact edges go right","missing":"Every unresolved and unsupported member remains explicit; empty-bin averages are null, never zero","identity":"Separate DGN1/v1 digest binds the exact parent assessment and all diagnostic results; prior journal/cohort/assessment bytes remain unchanged","interpretation":"Descriptive only; no calibration verdict, independence estimate, confidence interval or predictive-skill claim"}"#;

fn selected(state: &Replay, report: &Assessment) -> Result<AssessmentDiagnostics, Refusal> {
    state.ledger.assessment_diagnostics(report.digest)
}

pub(super) fn assessment_json(state: &Replay, report: &Assessment) -> Result<String, Refusal> {
    let diagnostics = selected(state, report)?;
    let members = diagnostics
        .members
        .iter()
        .map(|item| {
            let value = match item.value {
                DiagnosticValue::GaussianPit(pit) => format!(
                    "{{\"kind\":\"gaussian_pit\",\"value\":{}}}",
                    optional_number(pit)
                ),
                DiagnosticValue::EventProbability(probability) => {
                    format!("{{\"kind\":\"event_probability\",\"value\":{probability}}}")
                }
                DiagnosticValue::Unsupported(kind) => format!(
                    "{{\"kind\":\"unsupported\",\"distribution\":{},\"value\":null}}",
                    js(kind)
                ),
            };
            // This is the same member renderer as the parent, including selected-event hashes and source admission.
            state
                .member_json(&item.member)
                .map(|member| format!("{{\"member\":{member},\"diagnostic\":{value}}}"))
        })
        .collect::<Result<Vec<_>, _>>()?
        .join(",");
    let gaussian = diagnostics.gaussian_pit.as_ref().map_or_else(|| "null".into(), |pit| {
        let bins = pit.bins.iter().enumerate().map(|(i, bin)| format!(
            "{{\"lower\":{},\"upper\":{},\"upper_inclusive\":{},\"count\":{},\"whole_population_fraction\":{}}}",
            bin.lower, bin.upper, i == 9, bin.count, pair(bin.whole_population_fraction),
        )).collect::<Vec<_>>().join(",");
        format!("{{\"n_members\":{},\"n_resolved\":{},\"n_unresolved\":{},\"method\":{},\"bins\":[{bins}]}}", pit.n_members, pit.n_resolved, pit.n_unresolved, js(PIT_METHOD))
    });
    let bernoulli = diagnostics.bernoulli_reliability.as_ref().map_or_else(|| "null".into(), |reliability| {
        let bins = reliability.bins.iter().enumerate().map(|(i, bin)| format!(
            "{{\"lower\":{},\"upper\":{},\"upper_inclusive\":{},\"n_members\":{},\"n_resolved\":{},\"n_unresolved\":{},\"n_events\":{},\"mean_probability\":{},\"mean_resolved_probability\":{},\"observed_frequency\":{},\"whole_bin_frequency\":{}}}",
            bin.lower, bin.upper, i == 9, bin.n_members, bin.n_resolved, bin.n_members - bin.n_resolved,
            bin.n_events, optional_number(bin.mean_probability), optional_number(bin.mean_resolved_probability),
            optional_number(bin.observed_frequency), bin.whole_bin_frequency.map_or_else(|| "null".into(), pair),
        )).collect::<Vec<_>>().join(",");
        format!("{{\"n_members\":{},\"n_resolved\":{},\"n_unresolved\":{},\"bins\":[{bins}]}}", reliability.n_members, reliability.n_resolved, reliability.n_unresolved)
    });
    Ok(format!(
        ",\"diagnostics\":{{\"schema\":\"fmine.forecast-diagnostics.v1\",\"assessment_digest\":{},\"cohort\":{},\"digest\":{},\"as_of\":{},\"selector\":{},\"scope\":{},\"bin_rule\":{},\"n_members\":{},\"gaussian_pit\":{gaussian},\"bernoulli_reliability\":{bernoulli},\"members\":[{members}]}}",
        js(&diagnostics.assessment_digest.to_hex()),
        js(&diagnostics.cohort.to_hex()),
        js(&diagnostics.digest.to_hex()),
        utc(diagnostics.as_of_ns),
        js(selector_name(diagnostics.selector)),
        js(SCOPE),
        js(BIN_RULE),
        diagnostics.members.len(),
    ))
}

fn value(number: Option<f64>) -> String {
    number.map_or_else(|| "unavailable".into(), |value| value.to_string())
}

pub(super) fn assessment_markdown(
    out: &mut String,
    state: &Replay,
    report: &Assessment,
) -> Result<(), Refusal> {
    use crate::guidance::text;
    let diagnostics = selected(state, report)?;
    let _ = writeln!(
        out,
        "\n## Predictive diagnostics\n\n{SCOPE}\n\n{BIN_RULE}\n\nParent assessment: `{}`. Diagnostic report: `{}`.\n",
        diagnostics.assessment_digest, diagnostics.digest
    );
    if let Some(pit) = &diagnostics.gaussian_pit {
        let _ = writeln!(
            out,
            "### Gaussian PIT histogram\n\n{PIT_METHOD}\n\nGaussian population: {}; resolved: {}; unresolved: {}. These fractions use the complete Gaussian population; other distribution types are retained below.\n\n| PIT bin | Resolved count | Completion lower | Completion upper |\n|---|---:|---:|---:|",
            pit.n_members, pit.n_resolved, pit.n_unresolved
        );
        for (index, bin) in pit.bins.iter().enumerate() {
            let _ = writeln!(
                out,
                "| [{}, {}{} | {} | {} | {} |",
                bin.lower,
                bin.upper,
                if index == 9 { "]" } else { ")" },
                bin.count,
                bin.whole_population_fraction.0,
                bin.whole_population_fraction.1
            );
        }
    }
    if let Some(reliability) = &diagnostics.bernoulli_reliability {
        let _ = writeln!(
            out,
            "\n### Bernoulli reliability bins\n\nPopulation: {}; resolved: {}; unresolved: {}. Compare the resolved mean probability with the observed event frequency; the full mean and completion bounds retain pending outcomes. Empty-bin averages remain unavailable.\n\n| Probability bin | Resolved / all | Events | Mean probability (all) | Mean probability (resolved) | Observed frequency | Completion bounds |\n|---|---:|---:|---:|---:|---:|---|",
            reliability.n_members, reliability.n_resolved, reliability.n_unresolved
        );
        for (index, bin) in reliability.bins.iter().enumerate() {
            let bounds = bin.whole_bin_frequency.map_or_else(
                || "unavailable".into(),
                |(lower, upper)| format!("[{lower}, {upper}]"),
            );
            let _ = writeln!(
                out,
                "| [{}, {}{} | {} / {} | {} | {} | {} | {} | {} |",
                bin.lower,
                bin.upper,
                if index == 9 { "]" } else { ")" },
                bin.n_resolved,
                bin.n_members,
                bin.n_events,
                value(bin.mean_probability),
                value(bin.mean_resolved_probability),
                value(bin.observed_frequency),
                bounds
            );
        }
    }
    out.push_str("\n### Diagnostic membership\n\nEach forecast retains the exact selected outcome and source shown in the parent assessment. Unsupported distributions are not dropped or assigned an invented CDF.\n\n| Forecast | Diagnostic | Value |\n|---|---|---|\n");
    for item in &diagnostics.members {
        let (kind, value) = match &item.value {
            DiagnosticValue::GaussianPit(pit) => ("Gaussian PIT".into(), value(*pit)),
            DiagnosticValue::EventProbability(probability) => {
                ("Bernoulli probability".into(), probability.to_string())
            }
            DiagnosticValue::Unsupported(kind) => {
                (format!("unsupported ({kind})"), "unavailable".into())
            }
        };
        let alias = state
            .aliases
            .get(&item.member.forecast_id)
            .map_or("unknown", String::as_str);
        let _ = writeln!(
            out,
            "| {} (`{}`) | {} | {} |",
            text(alias),
            item.member.forecast_id,
            text(&kind),
            value
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::forecast::{Replay, time};
    use fmine_calib::OutcomeSelector;
    use fmine_core::hash::Digest32;

    #[test]
    fn renderers_retain_sources_missing_members_and_descriptive_boundaries() {
        let mut state = Replay::parse(
            r#"
schema = "fmine.forecast-ledger.v1"
lane = "synthetic"
[[forecast]]
id = "hit"
model_key = "synthetic-model"
registered_at = "2026-01-01T00:00:00Z"
outcome_due = "2026-02-01T00:00:00Z"
kind = "bernoulli"
probability = "0.15"
[[forecast]]
id = "pending"
model_key = "synthetic-model"
registered_at = "2026-01-01T00:00:00Z"
outcome_due = "2026-02-02T00:00:00Z"
kind = "bernoulli"
probability = "0.19"
[[cohort]]
name = "events"
frozen_at = "2026-01-02T00:00:00Z"
members = "hit,pending"
resolution_rule = "synthetic binary event"
eligible_sources = "synthetic"
[[outcome]]
forecast = "hit"
available_at = "2026-02-01T00:00:01Z"
recorded_at = "2026-02-01T00:00:01Z"
kind = "published"
value = "1"
source = "synthetic"
vintage = "first"
locator = "synthetic:whole-event"
sha256 = "0000000000000000000000000000000000000000000000000000000000000001"
"#,
        )
        .unwrap();
        let report = state
            .assessment(
                "events",
                time("2026-02-01T00:00:02Z").unwrap(),
                OutcomeSelector::FirstPublished,
            )
            .unwrap();
        let json = assessment_json(&state, &report).unwrap();
        assert!(json.contains("\"schema\":\"fmine.forecast-diagnostics.v1\""));
        assert!(json.contains("\"gaussian_pit\":null"));
        assert!(json.contains("\"n_members\":2,\"n_resolved\":1,\"n_unresolved\":1"));
        assert!(json.contains("\"whole_bin_frequency\":[0.5,1]"));
        assert!(json.contains("synthetic:whole-event"));
        assert!(json.contains("\"reason\":\"pending\""));
        assert!(json.contains("\"selected_event\":{\"seq\":1,"));
        assert!(!json.contains("\"calibration_flag\""));
        let mut markdown = String::new();
        assessment_markdown(&mut markdown, &state, &report).unwrap();
        assert!(markdown.contains("Bernoulli reliability bins"));
        assert!(markdown.contains("not calibration verdicts"));
        assert!(markdown.contains("pending (`"));
        assert!(markdown.contains(&report.digest.to_hex()));
        let rendered = state.render(&report, Digest32::default()).unwrap();
        assert!(rendered.contains(&json));
    }
}
