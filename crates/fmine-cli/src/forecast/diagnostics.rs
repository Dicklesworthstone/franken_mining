//! DESIGN (BD-048/049): display the saved assessment's descriptive diagnostics with its complete membership.
//! Determinism: Exact rendering of kernel-owned values; no outcome selection or statistical inference here.

use std::fmt::Write as _;

use fmine_calib::Assessment;
use fmine_calib::ledger::diagnostics::{
    BIN_RULE, DiagnosticValue, EMPIRICAL_METHOD, PIT_METHOD, SCOPE,
};
use fmine_evidence::Refusal;

use super::{Replay, display_or, js, optional_number, pair, selector_name, utc};

pub(super) const ROBOT_DOCS_JSON: &str = r#"{"scope":"Read-only descriptive projection of a saved assessment; no new outcomes or authority time","gaussian":"Per-member PIT and fixed-decile histogram via the existing normal CDF, retaining approximate values and tail saturation","bernoulli":"Fixed probability bins with full and resolved mean probabilities, observed event frequencies and unresolved-outcome completion bounds","empirical":"Exact below/equal/above sample counts and closed CDF bounds [below/n,(below+equal)/n]; separate definite/possible decile counts retain ties and missing outcomes without randomized PIT or a uniformity claim","coverage":"Separate CVG1/v1 projection of central 50/80/95 percent interval coverage; exact declared levels, per-member bounds and miss sides, supported/unresolved/unsupported populations and count-fraction completion bounds","quantile_losses":"Separate QNT1/v1 supplemental twice-pinball loss at every supplied level; exact checked rationals of frozen binary values, per-level populations and frozen-domain completion bounds; arithmetic indeterminacy never drops members or changes the parent interval score; JSON rational integers are strings","bins":"[lower,upper), with 1 included in the final bin; exact edges go right; empirical fractions and bin tests use integers","missing":"Every unresolved and unsupported member remains explicit; empty-bin averages are null, never zero","identity":"Separate DGN1/v2 when empirical members exist, otherwise unchanged DGN1/v1; binds the exact parent assessment and diagnostic results without changing prior journal/cohort/assessment bytes","interpretation":"Descriptive only; no calibration verdict, independence estimate, confidence interval or predictive-skill claim"}"#;

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
    let diagnostics = state.ledger.assessment_diagnostics(report.digest)?;
    let coverage = state.ledger.coverage_diagnostics(report.digest)?.to_json();
    let quantiles = state.ledger.quantile_diagnostics(report.digest)?.to_json();
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
    let version = diagnostics.version();
    Ok(format!(
        ",\"diagnostics\":{{\"schema\":\"fmine.forecast-diagnostics.v{version}\",\"assessment_digest\":{},\"cohort\":{},\"digest\":{},\"as_of\":{},\"selector\":{},\"scope\":{},\"bin_rule\":{},\"n_members\":{},\"gaussian_pit\":{gaussian},\"bernoulli_reliability\":{bernoulli}{empirical},\"members\":[{members}]}},\"interval_coverage\":{coverage},\"quantile_losses\":{quantiles}",
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
    let diagnostics = state.ledger.assessment_diagnostics(report.digest)?;
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
    out.push_str(
        &state
            .ledger
            .coverage_diagnostics(report.digest)?
            .to_markdown(),
    );
    out.push_str(
        &state
            .ledger
            .quantile_diagnostics(report.digest)?
            .to_markdown(),
    );
    Ok(())
}

// Public report-rendering acceptance checks live in xtask/tests/forecast_live.rs.
