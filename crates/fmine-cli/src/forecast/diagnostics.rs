//! DESIGN (BD-048/049): display the saved assessment's descriptive diagnostics with its complete membership.
//! Determinism: Exact rendering of kernel-owned values; no outcome selection or statistical inference here.

use std::fmt::Write as _;

use fmine_calib::Assessment;
use fmine_calib::ledger::diagnostics::{
    AssessmentDiagnostics, BIN_RULE, DiagnosticValue, EMPIRICAL_METHOD, PIT_METHOD, SCOPE,
};
use fmine_evidence::Refusal;

use super::{Replay, display_or, js, optional_number, pair, selector_name, utc};

pub(super) const ROBOT_DOCS_JSON: &str = r#"{"scope":"Read-only descriptive projection of a saved assessment; no new outcomes or authority time","gaussian":"Per-member PIT and fixed-decile histogram via the existing normal CDF, retaining approximate values and tail saturation","bernoulli":"Fixed probability bins with full and resolved mean probabilities, observed event frequencies and unresolved-outcome completion bounds","empirical":"Exact below/equal/above sample counts and closed CDF bounds [below/n,(below+equal)/n]; separate definite/possible decile counts retain ties and missing outcomes without randomized PIT or a uniformity claim","bins":"[lower,upper), with 1 included in the final bin; exact edges go right; empirical fractions and bin tests use integers","missing":"Every unresolved and unsupported member remains explicit; empty-bin averages are null, never zero","identity":"Separate DGN1/v2 when empirical members exist, otherwise unchanged DGN1/v1; binds the exact parent assessment and diagnostic results without changing prior journal/cohort/assessment bytes","interpretation":"Descriptive only; no calibration verdict, independence estimate, confidence interval or predictive-skill claim"}"#;

fn selected(state: &Replay, report: &Assessment) -> Result<AssessmentDiagnostics, Refusal> {
    state.ledger.assessment_diagnostics(report.digest)
}

fn fraction(numerator: usize, denominator: usize) -> String {
    format!("{{\"num\":{numerator},\"den\":{denominator}}}")
}

fn fraction_bounds(lower: usize, upper: usize, denominator: usize) -> String {
    format!(
        "{{\"lower\":{},\"upper\":{}}}",
        fraction(lower, denominator),
        fraction(upper, denominator)
    )
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
                DiagnosticValue::EmpiricalRank { n_samples, counts } => {
                    let fields = counts.map_or_else(
                        || {
                            "\"below\":null,\"equal\":null,\"above\":null,\"cdf_bounds\":null"
                                .into()
                        },
                        |counts| {
                            format!(
                                "\"below\":{},\"equal\":{},\"above\":{},\"cdf_bounds\":{}",
                                counts.below,
                                counts.equal,
                                counts.above,
                                fraction_bounds(
                                    counts.below,
                                    counts.below + counts.equal,
                                    n_samples,
                                ),
                            )
                        },
                    );
                    format!("{{\"kind\":\"empirical_rank\",\"n_samples\":{n_samples},{fields}}}")
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
    let empirical = diagnostics.empirical_rank.as_ref().map_or_else(String::new, |ranks| {
        let bins = ranks.bins.iter().map(|bin| format!(
            "{{\"lower\":{},\"upper\":{},\"upper_inclusive\":{},\"resolved_definite_count\":{},\"resolved_possible_count\":{},\"whole_population_fraction\":{}}}",
            fraction(usize::from(bin.lower_decile), 10), fraction(usize::from(bin.upper_decile), 10),
            bin.upper_decile == 10, bin.definite_count, bin.possible_count,
            fraction_bounds(bin.definite_count, bin.possible_count + ranks.n_unresolved, ranks.n_members),
        )).collect::<Vec<_>>().join(",");
        format!(",\"empirical_rank\":{{\"n_members\":{},\"n_resolved\":{},\"n_unresolved\":{},\"method\":{},\"bins\":[{bins}]}}", ranks.n_members, ranks.n_resolved, ranks.n_unresolved, js(EMPIRICAL_METHOD))
    });
    let version = if diagnostics.empirical_rank.is_some() {
        2
    } else {
        1
    };
    Ok(format!(
        ",\"diagnostics\":{{\"schema\":\"fmine.forecast-diagnostics.v{version}\",\"assessment_digest\":{},\"cohort\":{},\"digest\":{},\"as_of\":{},\"selector\":{},\"scope\":{},\"bin_rule\":{},\"n_members\":{},\"gaussian_pit\":{gaussian},\"bernoulli_reliability\":{bernoulli}{empirical},\"members\":[{members}]}}",
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
                display_or(bin.mean_probability, "unavailable"),
                display_or(bin.mean_resolved_probability, "unavailable"),
                display_or(bin.observed_frequency, "unavailable"),
                bounds
            );
        }
    }
    if let Some(ranks) = &diagnostics.empirical_rank {
        let _ = writeln!(
            out,
            "\n### Empirical CDF rank intervals\n\n{EMPIRICAL_METHOD}\n\nEmpirical population: {}; resolved: {}; unresolved: {}. This population is separate from the Gaussian PIT histogram. Fractions retain their exact count denominators.\n\n| Rank bin | Definite resolved | Possible resolved | Whole-population completion bounds |\n|---|---:|---:|---|",
            ranks.n_members, ranks.n_resolved, ranks.n_unresolved
        );
        for bin in &ranks.bins {
            let _ = writeln!(
                out,
                "| [{}/10, {}/10{} | {} | {} | [{}/{}, {}/{}] |",
                bin.lower_decile,
                bin.upper_decile,
                if bin.upper_decile == 10 { "]" } else { ")" },
                bin.definite_count,
                bin.possible_count,
                bin.definite_count,
                ranks.n_members,
                bin.possible_count + ranks.n_unresolved,
                ranks.n_members
            );
        }
    }
    out.push_str("\n### Diagnostic membership\n\nEach forecast retains the exact selected outcome and source shown in the parent assessment. Unsupported distributions are not dropped or assigned an invented CDF.\n\n| Forecast | Diagnostic | Value |\n|---|---|---|\n");
    for item in &diagnostics.members {
        let (kind, value) = match &item.value {
            DiagnosticValue::GaussianPit(pit) => {
                ("Gaussian PIT".into(), display_or(*pit, "unavailable"))
            }
            DiagnosticValue::EventProbability(probability) => {
                ("Bernoulli probability".into(), probability.to_string())
            }
            DiagnosticValue::EmpiricalRank { n_samples, counts } => (
                "Empirical CDF bounds".into(),
                counts.map_or_else(
                    || format!("unavailable (n={n_samples})"),
                    |counts| {
                        format!(
                            "[{}/{n_samples}, {}/{n_samples}] (below {}; equal {}; above {})",
                            counts.below,
                            counts.below + counts.equal,
                            counts.below,
                            counts.equal,
                            counts.above,
                        )
                    },
                ),
            ),
            DiagnosticValue::Unsupported(kind) => {
                (format!("unsupported ({kind})"), "unavailable".into())
            }
        };
        let alias = state.alias(item.member.forecast_id);
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

    fn fixture(
        name: &str,
        rule: &str,
        locator: &str,
        forecasts: &[(&str, u8, &str)],
        outcomes: &[(&str, &str)],
    ) -> (Replay, Assessment) {
        let mut ledger =
            String::from("schema = \"fmine.forecast-ledger.v1\"\nlane = \"synthetic\"\n");
        for (id, due_day, payload) in forecasts {
            let _ = writeln!(
                ledger,
                r#"[[forecast]]
id = "{id}"
model_key = "synthetic-model"
registered_at = "2026-01-01T00:00:00Z"
outcome_due = "2026-02-{due_day:02}T00:00:00Z"
{payload}"#
            );
        }
        let members = forecasts
            .iter()
            .map(|(id, _, _)| *id)
            .collect::<Vec<_>>()
            .join(",");
        let _ = writeln!(
            ledger,
            r#"[[cohort]]
name = "{name}"
frozen_at = "2026-01-02T00:00:00Z"
members = "{members}"
resolution_rule = "{rule}"
eligible_sources = "synthetic""#
        );
        for (forecast, value) in outcomes {
            let _ = writeln!(
                ledger,
                r#"[[outcome]]
forecast = "{forecast}"
available_at = "2026-02-01T00:00:01Z"
recorded_at = "2026-02-01T00:00:01Z"
kind = "published"
value = "{value}"
source = "synthetic"
vintage = "first"
locator = "{locator}"
sha256 = "0000000000000000000000000000000000000000000000000000000000000001""#
            );
        }
        let mut state = Replay::parse(&ledger).unwrap();
        let report = state
            .assessment(
                name,
                time("2026-02-01T00:00:02Z").unwrap(),
                OutcomeSelector::FirstPublished,
            )
            .unwrap();
        (state, report)
    }

    fn render(state: &Replay, report: &Assessment) -> (String, String) {
        let json = assessment_json(state, report).unwrap();
        let mut markdown = String::new();
        assessment_markdown(&mut markdown, state, report).unwrap();
        assert!(json.contains("\"selected_event\":{\"seq\":1,"));
        assert!(!json.contains("\"calibration_flag\""));
        assert!(
            state
                .render(report, Digest32::default())
                .unwrap()
                .contains(&json)
        );
        (json, markdown)
    }

    #[test]
    fn empirical_renderers_keep_exact_ties_missing_counts_and_separate_gaussian_population() {
        let (state, report) = fixture(
            "mixed",
            "synthetic continuous outcome",
            "synthetic:whole",
            &[
                ("tied", 1, "kind = \"empirical\"\nsamples = \"8,10,10,12\""),
                (
                    "missing",
                    1,
                    "kind = \"empirical\"\nsamples = \"10,10,10,10\"",
                ),
                (
                    "normal",
                    1,
                    "kind = \"gaussian\"\nmean = \"10\"\nstd_dev = \"2\"",
                ),
            ],
            &[("tied", "10"), ("normal", "10")],
        );
        let (json, markdown) = render(&state, &report);
        assert!(json.contains("\"schema\":\"fmine.forecast-diagnostics.v2\""));
        assert!(
            json.contains("\"gaussian_pit\":{\"n_members\":1,\"n_resolved\":1,\"n_unresolved\":0,")
        );
        assert!(
            json.contains(
                "\"empirical_rank\":{\"n_members\":2,\"n_resolved\":1,\"n_unresolved\":1,"
            )
        );
        assert!(json.contains("\"n_samples\":4,\"below\":1,\"equal\":2,\"above\":1,"));
        assert!(json.contains(concat!(
            "\"cdf_bounds\":{\"lower\":{\"num\":1,\"den\":4},",
            "\"upper\":{\"num\":3,\"den\":4}}"
        )));
        assert!(json.contains(concat!(
            "\"n_samples\":4,\"below\":null,\"equal\":null,",
            "\"above\":null,\"cdf_bounds\":null"
        )));
        assert!(json.contains(concat!(
            "\"whole_population_fraction\":{\"lower\":{\"num\":0,\"den\":2},",
            "\"upper\":{\"num\":2,\"den\":2}}"
        )));
        assert!(json.contains("synthetic:whole"));
        assert!(markdown.contains("### Empirical CDF rank intervals"));
        assert!(markdown.contains("Empirical population: 2; resolved: 1; unresolved: 1."));
        assert!(markdown.contains("[1/4, 3/4] (below 1; equal 2; above 1)"));
        assert!(markdown.contains("unavailable (n=4)"));
        assert!(markdown.contains("no randomized PIT, uniformity or calibration"));
    }

    #[test]
    fn renderers_retain_sources_missing_members_and_descriptive_boundaries() {
        let (state, report) = fixture(
            "events",
            "synthetic binary event",
            "synthetic:whole-event",
            &[
                ("hit", 1, "kind = \"bernoulli\"\nprobability = \"0.15\""),
                ("pending", 2, "kind = \"bernoulli\"\nprobability = \"0.19\""),
            ],
            &[("hit", "1")],
        );
        let (json, markdown) = render(&state, &report);
        assert!(json.contains("\"schema\":\"fmine.forecast-diagnostics.v1\""));
        assert!(json.contains("\"gaussian_pit\":null"));
        assert!(!json.contains("\"empirical_rank\""));
        assert!(json.contains("\"n_members\":2,\"n_resolved\":1,\"n_unresolved\":1"));
        assert!(json.contains("\"whole_bin_frequency\":[0.5,1]"));
        assert!(json.contains("synthetic:whole-event"));
        assert!(json.contains("\"reason\":\"pending\""));
        assert!(markdown.contains("Bernoulli reliability bins"));
        assert!(markdown.contains("not calibration verdicts"));
        assert!(markdown.contains("pending (`"));
        assert!(markdown.contains(&report.digest.to_hex()));
    }
}
