//! Read-only issuer guidance track record and its human rendition (BD-341.S6).

use std::fmt::Write as _;
use std::path::Path;

use fmine_core::Digest32;
use fmine_public::key::{KeyedCell, PublicValue, raw_token};

use crate::guidance::{interval, number, text};
use crate::track_record_data::{self as data, NO_CLAIM, OutcomeBinding, TrackRecordReport};
use crate::{DOCTOR_WHY, Envelope, NextAction};

pub(crate) fn execute(
    issuer: Option<&str>,
    site: Option<&str>,
    through: Option<&str>,
    root: &Path,
) -> Envelope {
    let report = match issuer {
        Some(issuer) => data::run(root, issuer, site, through),
        None if site.is_none() && through.is_none() => data::synthetic(),
        None => {
            return Envelope::usage_error(
                "track-record",
                "site and through selections require an issuer",
            );
        }
    };
    let actions = vec![NextAction::new("fmine doctor", DOCTOR_WHY)];
    match report {
        Ok(report) => {
            let mut envelope = Envelope::ok("track-record", report.to_json(), actions);
            envelope.no_claim = Some(NO_CLAIM);
            // The report digest commits to the selected records; a retrospective analysis does not create
            // a pre-outcome registration, external time proof or authority-ledger certificate.
            envelope.rendered_markdown = Some(render(&report));
            envelope
        }
        Err(refusal) => Envelope::refused("track-record", refusal, actions),
    }
}

fn printed(cell: &KeyedCell) -> String {
    let token = match &cell.claim.value {
        PublicValue::Number {
            displayed: Ok(value),
            ..
        } => value,
        other => raw_token(other),
    };
    format!("{} {}", text(token), text(&cell.key.unit))
}

fn source(out: &mut String, kind: &str, cell: &KeyedCell) {
    for anchor in &cell.claim.anchors {
        let _ = writeln!(
            out,
            "| {} | {} | `{}` |",
            text(kind),
            text(&cell.locator),
            Digest32(anchor.artifact_sha256).to_hex()
        );
    }
}

fn render(report: &TrackRecordReport) -> String {
    let label = if report.synthetic {
        "Invented synthetic evidence; these results are not issuer findings."
    } else {
        "From public filings; no affiliation implied."
    };
    let selection = report.through.as_deref().map_or_else(
        || "All registered workbook releases.".to_string(),
        |through| format!("Workbook releases through {}.", text(through)),
    );
    let mut out = format!(
        "# Guidance track record: {}\n\n{label}\n\n{}\n\n{selection} This is a retrospective comparison, not an as-of information cut.\n\n\
         Report BLAKE3: `{}`. Source inventory digest: `{}`.\n\n\
         ## Guidance and annual outcomes\n\n\
         Guidance vintage zero is the earliest registered guidance for that site, metric and year; later registered vintages remain separate. \
         A comparative outcome is not the original publication.\n\n\
         | Site and metric | Year; guidance date | Guidance | Selected outcome | Printed result | Normalized deviation z | Guidance vintage |\n\
         |---|---|---|---|---|---|---|\n",
        text(&report.issuer),
        text(NO_CLAIM),
        report.digest.to_hex(),
        report.source_digest.to_hex()
    );
    for row in &report.rows {
        let outcome = match &row.outcome {
            OutcomeBinding::Resolved { cell, comparative } => format!(
                "{}; {} ({})",
                printed(cell),
                text(&cell.key.reporting_vintage.release),
                if *comparative {
                    "comparative"
                } else {
                    "first published"
                }
            ),
            OutcomeBinding::Unresolved { code, reason } => {
                format!("Unresolved: {} — {}", text(code), text(reason))
            }
        };
        let result = row.score.as_ref().map_or_else(
            || "Unscored".to_string(),
            |score| {
                format!(
                    "{}{}",
                    score.side.as_str(),
                    if score.boundary {
                        "; rounding overlaps boundary"
                    } else {
                        ""
                    }
                )
            },
        );
        let z = row
            .score
            .as_ref()
            .and_then(|score| score.z)
            .map(number)
            .unwrap_or_else(|| "Undefined".into());
        let _ = writeln!(
            out,
            "| {} / {} | {}; {} | {} {} | {outcome} | {result} | {z} | {} |",
            text(&row.site),
            text(&row.metric),
            row.year,
            text(&row.published_at),
            text(&row.guidance.token),
            text(&row.guidance.unit),
            row.vintage
        );
    }
    out.push_str("\nFor a non-degenerate range, z is the printed outcome's distance from the guidance midpoint divided by the guidance half-width. It is not a statistical z-score. Point and one-sided guidance have no z. Printed rounding uncertainty remains distinct from sampling uncertainty.\n\n## Descriptive summaries\n\nEvery missing member remains in the completion-bound denominator. Rows with different units, ownership bases, guidance vintages or comparative-outcome status are not pooled.\n\n");
    for summary in &report.summaries {
        let scores = &summary.scores;
        let _ = writeln!(
            out,
            "### {} / {} / guidance vintage {}\n\n{}; {}; {}.\n\nMembers: {}; resolved: {}; unresolved: {}. Printed outcomes inside: {}; below: {}; above: {}; boundary-ambiguous: {}.\n",
            text(&summary.site),
            text(&summary.metric),
            summary.vintage,
            text(&summary.unit),
            text(&summary.basis),
            if summary.comparative {
                "comparative outcomes"
            } else {
                "first-published or unresolved outcomes"
            },
            scores.n_members,
            scores.n_resolved,
            scores.n_unresolved,
            scores.inside,
            scores.below,
            scores.above,
            scores.boundary
        );
        if let Some(rate) = scores.observed_hit_rate {
            let _ = writeln!(
                out,
                "Observed hit share among resolved members: {}.\n",
                number(rate)
            );
        }
        if let Some(bounds) = scores.whole_cohort_hit_bounds {
            let _ = writeln!(
                out,
                "Whole-cohort hit-share completion bounds: {}.\n",
                interval(bounds)
            );
        }
        if let Some(bounds) = scores.rounding_aware_hit_bounds {
            let _ = writeln!(
                out,
                "Completion bounds including printed rounding: {}.\n",
                interval(bounds)
            );
        }
        if let Some((lo, hi)) = scores.iid_reference_interval_90 {
            let _ = writeln!(
                out,
                "IID-only binomial reference interval at the stated ninety-percent level: ≈ {lo:.6} to {hi:.6}. This assumes independent, identically distributed hits; it does not account for common year or site effects.\n"
            );
        }
        if let Some(mean) = scores.mean_z {
            let _ = writeln!(
                out,
                "Mean normalized deviation among {} resolved ranges: {}.\n",
                scores.n_ranges,
                number(mean)
            );
        }
        if let Some(bounds) = scores.mean_z_rounding_interval {
            let _ = writeln!(
                out,
                "Mean-deviation bounds from printed rounding only: {}. These are not a confidence interval for bias.\n",
                interval(bounds)
            );
        }
    }
    out.push_str("## Later outcome appearances\n\nThese appearances remain visible and never overwrite the selected outcome or enter its score.\n\n| Site and metric | Year | Release | Printed outcome | Locator |\n|---|---|---|---|---|\n");
    for row in &report.rows {
        for cell in &row.later_outcomes {
            let _ = writeln!(
                out,
                "| {} / {} | {} | {} | {} | {} |",
                text(&row.site),
                text(&row.metric),
                row.year,
                text(&cell.key.reporting_vintage.release),
                printed(cell),
                text(&cell.locator)
            );
        }
    }
    if !report.source_refusals.is_empty() {
        out.push_str("\n## Source mapping refusals\n\nThese source cells did not produce usable canonical Claims. They are retained here so missing evidence is visible.\n\n| Locator | Reason |\n|---|---|\n");
        for refusal in &report.source_refusals {
            let _ = writeln!(
                out,
                "| {} | {} |",
                text(&refusal.locator),
                text(&refusal.reason)
            );
        }
    }
    out.push_str("\n## Source citations\n\n| Role | Locator | SHA-256 |\n|---|---|---|\n");
    for row in &report.rows {
        let _ = writeln!(
            out,
            "| Guidance | {} | `{}` |",
            text(&row.guidance.locator.to_string()),
            row.guidance.doc_sha256
        );
        if let OutcomeBinding::Resolved { cell, .. } = &row.outcome {
            source(&mut out, "Selected outcome", cell);
        }
        for cell in &row.later_outcomes {
            source(&mut out, "Later appearance", cell);
        }
    }
    out.push_str("\nGuidance ranges carry no invented probability level. Effective sample size and dependence-adjusted bias intervals are not estimated. All authority flags remain false.\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CliCommand, OutputFormat, execute_command, invoke, parse_args, run};
    use std::path::PathBuf;

    fn root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .expect("workspace root")
            .to_path_buf()
    }

    fn args(words: &[&str]) -> Vec<String> {
        words.iter().map(|s| (*s).to_string()).collect()
    }

    #[test]
    fn synthetic_command_reports_cited_scores_missing_members_and_false_authority() {
        let query = args(&["track-record", "--synthetic", "--json"]);
        let (envelope, format) = invoke(&query, &root());
        assert_eq!(format, OutputFormat::Json);
        assert_eq!(
            (envelope.status, envelope.exit_code),
            ("ok", 0),
            "{}",
            envelope.to_json()
        );
        assert_eq!(envelope.verb, "track-record");
        assert_eq!(envelope.no_claim, Some(NO_CLAIM));
        assert!(envelope.anchor_id.is_none() && envelope.certificate.is_none());
        let payload = &envelope.payload_json;
        assert!(payload.contains("\"schema\":\"fmine.guidance.track-record.v1\""));
        assert!(payload.contains("\"lane\":\"synthetic\""));
        assert!(payload.contains("\"z\":{\"num\":\"3\",\"den\":\"2\"}"));
        assert!(payload.contains("\"n_unresolved\":1"));
        assert!(payload.contains("\"state\":\"unresolved\""));
        assert!(payload.contains("\"sha256\":") && payload.contains("\"in\":["));
        assert!(payload.contains("\"calibration_verdict\":null"));
        assert!(payload.contains("no affiliation implied"));
        assert!(envelope.to_json().contains(crate::FLAGS_JSON));
        assert_eq!(run(&query, &root()), run(&query, &root()));
        assert_eq!(run(&query, &root()).0.lines().count(), 1);
        let digest = data::synthetic().expect("synthetic report").digest.to_hex();
        assert!(payload.contains(&digest));
    }

    #[test]
    fn selections_are_explicit_and_invalid_combinations_are_usage_errors() {
        let query = args(&[
            "track-record",
            "--issuer",
            "barrick",
            "--site",
            "north-mara",
            "--through",
            "2026Q2",
        ]);
        let (command, _, _) = parse_args(&query, &root()).expect("public selectors");
        assert_eq!(
            command,
            CliCommand::TrackRecord {
                issuer: Some("barrick".into()),
                site: Some("north-mara".into()),
                through: Some("2026Q2".into()),
            }
        );
        for query in [
            vec!["track-record"],
            vec!["track-record", "extra", "--synthetic"],
            vec!["track-record", "--issuer", "unknown-issuer"],
            vec!["track-record", "--issuer", "../barrick"],
            vec!["track-record", "--issuer", "barrick", "--synthetic"],
            vec!["track-record", "--synthetic", "--site", "north-mara"],
            vec!["track-record", "--synthetic", "--through", "2026Q2"],
            vec!["track-record", "--issuer", "barrick", "--site", " "],
            vec!["track-record", "--issuer", "barrick", "--through", "2026Q5"],
            vec!["track-record", "--synthetic", "--as-of", "2026-10-31"],
            vec!["track-record", "--synthetic", "--selector", "latest-as-of"],
            vec!["track-record", "--synthetic", "--rank"],
        ] {
            let (envelope, _) = invoke(&args(&query), &root());
            assert_eq!(
                (envelope.verb.as_str(), envelope.status, envelope.exit_code),
                ("track-record", "usage_error", 2),
                "{query:?}"
            );
        }
    }

    #[test]
    fn markdown_keeps_citations_uncertainty_and_literal_source_text() {
        let mut report = data::synthetic().expect("synthetic report");
        report.issuer = "<script>|[link](bad)\n# forged".into();
        let rendered = render(&report);
        assert!(rendered.contains("Guidance and annual outcomes"));
        assert!(rendered.contains("Whole-cohort hit-share completion bounds"));
        assert!(rendered.contains("Unresolved"));
        assert!(rendered.contains("1.5"));
        assert!(rendered.contains("Source citations"));
        assert!(rendered.contains(&report.rows[0].guidance.doc_sha256));
        assert!(!rendered.contains("<script>"));
        assert!(!rendered.contains("\n# forged"));
        assert!(rendered.contains("&lt;script&gt;\\|\\[link\\](bad) # forged"));
        assert_eq!(rendered, render(&report));
        let (printed, code) = run(
            &args(&["track-record", "--synthetic", "--format", "markdown"]),
            &root(),
        );
        assert_eq!(code, 0);
        assert!(printed.starts_with("# Guidance track record:"));
    }

    #[test]
    fn command_is_registered_and_documented() {
        let registry =
            std::fs::read_to_string(root().join("registries/verbs.toml")).expect("verb registry");
        assert!(registry.contains("path = \"track-record\""));
        let docs = execute_command(&CliCommand::RobotDocs, &root());
        assert!(
            docs.payload_json
                .contains("track-record --issuer <issuer> [--site <subject>] [--through YYYYQn]")
        );
        assert!(docs.payload_json.contains("track-record --synthetic"));
        assert!(docs.payload_json.contains("retrospective_descriptive"));
    }
}
