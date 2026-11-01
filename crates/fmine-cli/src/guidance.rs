//! Human-readable guidance requirements and the exact historical comparison behind them.

use std::collections::BTreeSet;
use std::fmt::Write as _;

use fmine_claim::Claim;
use fmine_core::Digest32;
use fmine_metrics::guidance::{EnvelopeTier, HistoryVerdict, Ownership, Period, Quantity};
use fmine_numerics::exact_feas::{Interval, Rat};
use fmine_public::guidance::{GuidanceCompletionReport, NO_CLAIM};

pub(super) fn text(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('\\', "\\\\")
        .replace('|', "\\|")
        .replace('`', "\\`")
        .replace('*', "\\*")
        .replace('_', "\\_")
        .replace('[', "\\[")
        .replace(']', "\\]")
        .replace(['\n', '\r'], " ")
}

/// Prefer an exact terminating decimal; retain a nonterminating rational and label its display approximation.
pub(super) fn number(value: Rat) -> String {
    let (num, den) = (value.num(), value.den());
    let mut scale = 1_i128;
    for places in 0..=6 {
        if scale % den == 0
            && let Some(scaled) = num.checked_mul(scale / den)
        {
            if places == 0 {
                return scaled.to_string();
            }
            let magnitude = scaled.unsigned_abs();
            let base = scale.unsigned_abs();
            let fraction = format!("{:0places$}", magnitude % base);
            return format!(
                "{}{}.{}",
                if scaled < 0 { "−" } else { "" },
                magnitude / base,
                fraction.trim_end_matches('0')
            );
        }
        scale *= 10;
    }
    format!("{num}/{den} (≈ {:.2})", value.to_f64())
}

pub(super) fn interval(value: Interval) -> String {
    if value.lo == value.hi {
        number(value.lo)
    } else {
        format!("{} to {}", number(value.lo), number(value.hi))
    }
}

fn period(value: Period) -> String {
    format!(
        "{} months {}–{}",
        value.year, value.first_month, value.last_month
    )
}

fn possibility(value: Option<bool>) -> &'static str {
    match value {
        Some(true) => "Yes",
        Some(false) => "No",
        None => "Indeterminate",
    }
}

fn row(out: &mut String, name: &str, claim: &Claim<Quantity>, unit: &str) {
    let _ = writeln!(
        out,
        "| {name} | {} {unit} | {} | `{}` |",
        interval(claim.value.interval),
        text(&format!("{:?}", claim.evidence)),
        claim.claim_id
    );
}

pub(crate) fn render(report: &GuidanceCompletionReport, selection: &str) -> String {
    render_inner(report, selection, None)
}

pub(crate) fn render_with_capacity(
    report: &GuidanceCompletionReport,
    selection: &str,
    capacity_markdown: &str,
) -> String {
    render_inner(report, selection, Some(capacity_markdown))
}

fn render_inner(
    report: &GuidanceCompletionReport,
    selection: &str,
    capacity_markdown: Option<&str>,
) -> String {
    let no_claim = if capacity_markdown.is_some() {
        "Arithmetic and finite historical-window comparisons under a declared sales assumption. The separately requested production-ceiling comparison follows below. No forecast, probability of meeting guidance, joint production-cost feasibility or regulatory claim."
    } else {
        NO_CLAIM
    };
    let inputs = &report.inputs;
    let tier = match report.history.tier {
        EnvelopeTier::T1 => "T1 — finite historical windows",
        EnvelopeTier::T3 => "T3 — necessary arithmetic conditions only",
    };
    let verdict = match report.history.verdict {
        HistoryVerdict::WithinEnvelope => {
            "A supplied historical window has an exact joint witness."
        }
        HistoryVerdict::ExceedsEnvelope => {
            "No supplied historical window satisfies the joint requirements under the stated sales scenario."
        }
        HistoryVerdict::Indeterminate => {
            "The supplied history does not establish a joint comparison; inspect the exclusions and indeterminate checks below."
        }
    };
    let ownership = match inputs.production_guidance.value.ownership {
        Ownership::Full => "100% production basis".to_string(),
        Ownership::Attributable { share } => {
            format!("Attributable production basis, share {}", number(share))
        }
    };
    let mut out = format!(
        "# Guidance completion: {}\n\n{}\n\n{}\n\n**{tier}.** {verdict}\n\n{}; remaining support: {}.\n\n\
         **Sales assumption:** repeat the preceding year's FY-minus-YTD sold ounces for the remaining months. \
         This assumption remains separate from production. Every historical comparison is conditional on that sales interval.\n\n\
         Report BLAKE3: `{}`. No forecast registration or ledger certificate is created.\n\n\
         ## Requirements\n\n| Quantity | Exact bounds | Evidence | Claim |\n|---|---|---|---|\n",
        text(&inputs.production_guidance.value.subject),
        text(selection),
        text(no_claim),
        ownership,
        period(inputs.remaining_sold.value.period),
        report.digest.to_hex()
    );
    for (name, claim, unit) in [
        (
            "Remaining production",
            &report.completion.remaining_production,
            "koz",
        ),
        ("Assumed remaining sales", &inputs.remaining_sold, "koz"),
        (
            "YTD AISC spending",
            &report.completion.ytd_aisc_spend,
            "USD million",
        ),
        (
            "Required remaining AISC spending",
            &report.completion.remaining_aisc_spend,
            "USD million",
        ),
        (
            "Required remaining AISC",
            &report.completion.remaining_aisc_rate,
            "USD per sold oz",
        ),
    ] {
        row(&mut out, name, claim, unit);
    }
    out.push_str("\nClosed bounds retain printed rounding uncertainty. Approximate decimal displays are marked ≈; the accompanying fractions remain exact. Signed requirements are retained. The Evidence column preserves the weakest input evidence; it does not mean the issuer asserted the computed requirement or the replay assumption.\n\n## Historical joint comparison\n\nA marginal production success and a marginal cost success do not establish joint feasibility. Only a checked joint witness does. Exclusion is limited to the supplied finite historical windows; it is not a mine-capacity conclusion or a forecast.\n\n");
    if report.history.checks.is_empty() {
        out.push_str("No complete admissible historical window was supplied.\n\n");
    } else {
        out.push_str("| Historical window | Same season | Production possible | Cost possible | Joint result |\n|---|---|---|---|---|\n");
        for check in &report.history.checks {
            let joint = match &check.witness {
                Some(witness) => format!("Checked witness `{}`", witness.claim_id),
                None if check.indeterminate.is_empty() => "Excluded within this window".into(),
                None => format!("Indeterminate: {}", text(&check.indeterminate.join("; "))),
            };
            let _ = writeln!(
                out,
                "| {} | {} | {} | {} | {joint} |",
                text(&check.id),
                if check.same_season { "Yes" } else { "No" },
                possibility(check.production_possible),
                possibility(check.cost_possible)
            );
        }
        out.push('\n');
        for check in &report.history.checks {
            if let Some(witness) = &check.witness {
                let value = &witness.value;
                let _ = writeln!(
                    out,
                    "### Checked replay: {}\n\nAnnual production: {} koz; annual AISC: {} USD per sold oz.\n\n| Original support | Hypothetical replay | Production (koz) | Sales (koz) | AISC (USD per sold oz) |\n|---|---|---|---|---|",
                    text(&check.id),
                    number(value.completion.annual_production_koz),
                    number(value.completion.annual_aisc_usd_per_oz)
                );
                for quarter in &value.quarters {
                    let _ = writeln!(
                        out,
                        "| {} | {} | {} | {} | {} |",
                        period(quarter.source_period),
                        period(quarter.replay_period),
                        number(quarter.production_koz),
                        number(quarter.sold_koz),
                        number(quarter.aisc_usd_per_oz)
                    );
                }
                out.push('\n');
            }
        }
    }
    if !report.history_exclusions.is_empty() {
        out.push_str("### Excluded source windows\n\n| Window | Code | Missing or incompatible evidence |\n|---|---|---|\n");
        for item in &report.history_exclusions {
            let _ = writeln!(
                out,
                "| {} | {} | {} |",
                text(&item.id),
                text(&item.code),
                text(&item.reason)
            );
        }
        out.push('\n');
    }
    out.push_str("## Guidance and actuals\n\n| Quantity | Exact bounds | Evidence | Claim |\n|---|---|---|---|\n");
    for (name, claim, unit) in [
        (
            "Annual production guidance",
            &inputs.production_guidance,
            "koz",
        ),
        (
            "Annual AISC guidance",
            &inputs.aisc_guidance,
            "USD per sold oz",
        ),
        ("YTD production", &inputs.ytd_production, "koz"),
        ("YTD sales", &inputs.ytd_sold, "koz"),
        ("YTD AISC", &inputs.ytd_aisc, "USD per sold oz"),
    ] {
        row(&mut out, name, claim, unit);
    }
    let mut anchors = BTreeSet::new();
    for claim in [
        &inputs.production_guidance,
        &inputs.aisc_guidance,
        &inputs.ytd_production,
        &inputs.ytd_sold,
        &inputs.ytd_aisc,
        &inputs.remaining_sold,
    ] {
        anchors.extend(claim.anchors.iter());
    }
    for window in &report.history_sources {
        for source in &window.source_claims {
            anchors.extend(source.claim.anchors.iter());
        }
    }
    out.push_str("\n## Source citations\n\n| Source | Locator | SHA-256 |\n|---|---|---|\n");
    for anchor in anchors {
        let _ = writeln!(
            out,
            "| {} | {} | `{}` |",
            text(&anchor.artifact_path),
            text(&anchor.locator),
            Digest32(anchor.artifact_sha256).to_hex()
        );
    }
    if let Some(capacity) = capacity_markdown {
        out.push_str("\nAll four INV-002 authority flags remain false. Historical seasonality remains context only; the explicit physical limits below supply a separate production-only exclusion test.\n\n");
        out.push_str(capacity);
    } else {
        out.push_str("\nAll four INV-002 authority flags remain false. Historical seasonality is context only; no probability of meeting guidance or technical-report capacity claim is made.\n");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_bounds_do_not_become_unlabelled_rounded_decimals() {
        assert_eq!(number(Rat::new(263, 2).unwrap()), "131.5");
        assert_eq!(number(Rat::new(-1, 8).unwrap()), "−0.125");
        assert_eq!(number(Rat::new(1, 3).unwrap()), "1/3 (≈ 0.33)");
    }

    #[test]
    fn source_text_cannot_add_markdown_structure_or_html() {
        let report = fmine_public::guidance::run_synthetic().unwrap();
        let rendered = render(&report, "<script>|[link](bad)\n# forged");
        assert!(!rendered.contains("<script>"));
        assert!(!rendered.contains("\n# forged"));
        assert!(rendered.contains("&lt;script&gt;\\|\\[link\\](bad) # forged"));
        assert!(rendered.contains("Original support"));
        assert!(rendered.contains("Hypothetical replay"));
        assert_eq!(rendered, render(&report, "<script>|[link](bad)\n# forged"));
    }

    #[test]
    fn explicit_capacity_selection_is_scoped_and_documented() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(std::path::Path::parent)
            .expect("workspace");
        let args = |words: &[&str]| {
            words
                .iter()
                .map(|word| (*word).to_string())
                .collect::<Vec<_>>()
        };
        let base = [
            "guidance",
            "completion",
            "--issuer",
            "barrick",
            "--site",
            "north-mara",
            "--through",
            "2026Q2",
            "--as-of",
            "2026-10-31",
        ];
        let mut selected = args(&base);
        selected.extend(args(&["--capacity-document", "Mine-technical-report.pdf"]));
        let (command, _, _) = crate::parse_args(&selected, root).expect("capacity selection");
        assert!(matches!(command, crate::CliCommand::GuidanceCompletion {
            capacity_document: Some(ref document), ..
        } if document == "Mine-technical-report.pdf"));
        let (command, _, _) = crate::parse_args(&args(&base), root).expect("ordinary guidance");
        assert!(matches!(
            command,
            crate::CliCommand::GuidanceCompletion {
                capacity_document: None,
                ..
            }
        ));
        for invalid in [
            args(&["doctor", "--capacity-document", "Report.pdf"]),
            args(&[
                "track-record",
                "--synthetic",
                "--capacity-document",
                "Report.pdf",
            ]),
            args(&[
                "guidance",
                "completion",
                "--synthetic",
                "--capacity-document",
                "Report.pdf",
            ]),
            {
                let mut invalid = args(&base);
                invalid.push("--capacity-document".into());
                invalid
            },
            {
                let mut invalid = args(&base);
                invalid.extend(args(&["--capacity-document", " "]));
                invalid
            },
        ] {
            let (envelope, _) = crate::invoke(&invalid, root);
            assert_eq!(envelope.exit_code, 2, "{invalid:?}");
            assert_eq!(envelope.status, "usage_error");
        }
        let docs = crate::execute_command(&crate::CliCommand::RobotDocs, root);
        assert!(
            docs.payload_json
                .contains("--capacity-document <registered file>")
        );
    }

    #[test]
    fn capacity_section_keeps_the_historical_comparison_and_scopes_its_boundary() {
        let report = fmine_public::guidance::run_synthetic().expect("fixture");
        let ordinary = render(&report, "Synthetic");
        let with_capacity = render_with_capacity(
            &report,
            "Synthetic",
            "## T2 production ceiling\n\nNot excluded.",
        );
        assert!(with_capacity.contains("Historical joint comparison"));
        assert!(with_capacity.contains("T2 production ceiling"));
        assert!(with_capacity.contains("Not excluded."));
        assert!(with_capacity.contains("explicit physical limits"));
        assert!(!with_capacity.contains(
            "no probability of meeting guidance or technical-report capacity claim is made"
        ));
        assert!(ordinary.contains(
            "no probability of meeting guidance or technical-report capacity claim is made"
        ));
    }
}
