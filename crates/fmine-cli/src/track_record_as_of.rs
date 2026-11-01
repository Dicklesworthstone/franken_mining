//! Descriptive guidance records, preserving original outcomes and subsequent appearances.

use std::collections::BTreeSet;
use std::fmt::Write as _;

use crate::track_record_binding::{NO_CLAIM, OutcomeAssessment, TrackRecordReport};
use fmine_calib::guidance_scores::claims::{GuidanceTarget, Hit, Side};
use fmine_core::Digest32;
use fmine_evidence::Refusal;

use crate::guidance::{interval, number, text};
use crate::{DOCTOR_WHY, Envelope, NextAction};

pub(crate) fn envelope(result: Result<TrackRecordReport, Refusal>) -> Envelope {
    let actions = vec![NextAction::new("fmine doctor", DOCTOR_WHY)];
    match result {
        Ok(report) => {
            let mut envelope = Envelope::ok("track-record", report.to_json(), actions);
            envelope.no_claim = Some(NO_CLAIM);
            envelope.rendered_markdown = Some(render(&report));
            // The report digest binds this projection; it does not create an authority-ledger entry.
            envelope
        }
        Err(refusal) => Envelope::refused("track-record", refusal, actions),
    }
}

fn target(value: GuidanceTarget) -> String {
    match value {
        GuidanceTarget::Range { lo, hi } => format!("{} to {}", number(lo), number(hi)),
        GuidanceTarget::Point(v) => number(v),
        GuidanceTarget::AtMost(v) => format!("at most {}", number(v)),
        GuidanceTarget::AtLeast(v) => format!("at least {}", number(v)),
    }
}

fn outcome(value: &OutcomeAssessment) -> String {
    match &value.outcome {
        Some(c) => number(c.value.point),
        None => format!(
            "Missing — {}",
            text(value.reason.as_deref().unwrap_or("no admissible outcome"))
        ),
    }
}

fn position(value: &OutcomeAssessment) -> String {
    let Some(c) = &value.score else {
        return "Unscored".into();
    };
    let s = &c.value;
    let side = match s.side {
        Side::Below => "below",
        Side::Inside => "inside",
        Side::Above => "above",
    };
    let rounding = match s.hit {
        Hit::CertainHit => "inside throughout the rounding interval",
        Hit::CertainMiss => "outside throughout the rounding interval",
        Hit::Boundary => "rounding interval crosses a guidance boundary",
    };
    let normalized = s.z.map_or_else(
        || "z undefined for this target".into(),
        |v| format!("z = {}", number(v)),
    );
    format!("{side}; {normalized}; {rounding}")
}

fn render(report: &TrackRecordReport) -> String {
    let lane = if report.synthetic {
        "Synthetic invented evidence. This example is not an issuer finding."
    } else {
        "From public filings; no affiliation implied."
    };
    let mut out = format!(
        "# Guidance track record: {}\n\n{}\n\nEvidence as of {}; workbook release bound: {}.\n\n{}\n\n\
         Report BLAKE3: `{}`. This projection does not register or externally timestamp a forecast.\n\n",
        text(&report.issuer),
        lane,
        text(&report.as_of),
        report
            .through
            .as_deref()
            .map_or("all registered releases".into(), text),
        text(NO_CLAIM),
        report.digest.to_hex(),
    );
    if report.gate_fired {
        let _ = writeln!(
            out,
            "**Portfolio evidence gate not met.** {} resolved years; minimum {} sites per resolved year. \
             These are individual site descriptions. The sample does not establish forecasting skill.\n",
            report.resolved_years, report.minimum_sites_per_year
        );
    }
    out.push_str(
        "## Original annual outcomes\n\nGuidance revisions remain separate. A later comparative never fills a missing original outcome. \
         Values retain their stated units and ownership bases; normalized position z describes distance from the guidance midpoint in half-range units.\n\n",
    );
    for row in &report.rows {
        let _ = writeln!(
            out,
            "### {} · {} · {}\n\nGuidance published {} ({}); basis: {}.\n\n\
             | Guidance | Original outcome | Comparison |\n|---|---|---|\n| {} {} | {} | {} |\n\n\
             Guidance Claim: `{}`. Source: {}; SHA-256 `{}`.\n",
            text(&row.site),
            row.year,
            text(&row.metric),
            text(&row.published_at),
            text(&row.vintage_kind),
            text(&row.basis),
            target(row.guidance.value.target),
            text(&row.unit),
            outcome(&row.original),
            position(&row.original),
            row.guidance.claim_id,
            text(&row.guidance_source.locator.to_string()),
            text(&row.guidance_source.doc_sha256),
        );
        if let Some(c) = &row.original.outcome {
            let _ = writeln!(
                out,
                "Original release: {}; outcome Claim: `{}`; printed-rounding interval: {} {}.\n",
                text(&row.original.release),
                c.claim_id,
                interval(c.value.rounding),
                text(&row.unit)
            );
        }
    }
    out.push_str("## Restatements and comparative appearances\n\nThese observations have their own scores and do not enter the original-outcome summaries. A comparative-only observation does not establish the first published outcome.\n\n| Site, metric, basis and guidance vintage | Outcome release | Appearance | Outcome | Comparison |\n|---|---|---|---|---|\n");
    let mut later_count = 0;
    for row in &report.rows {
        for later in &row.later {
            later_count += 1;
            let _ = writeln!(
                out,
                "| {} · {} · {} · {} · {} | {} | {} | {} {} | {} |",
                text(&row.site),
                text(&row.metric),
                text(&row.basis),
                row.year,
                text(&row.published_at),
                text(&later.release),
                text(&later.kind),
                outcome(later),
                text(&row.unit),
                position(later),
            );
        }
    }
    if later_count == 0 {
        out.push_str("| No later appearances in this selection | — | — | — | — |\n");
    }
    out.push_str("\n## Descriptive cohorts\n\nEach summary uses one metric, annual period, canonical ownership/definition basis and guidance publication date. Missing members remain in the denominator. Whole-cohort bounds allow every missing outcome to miss or hit; rounding-aware bounds additionally allow either result at boundary cases.\n\n");
    for group in &report.summaries {
        let s = &group.summary.value;
        let period = match &s.scope {
            fmine_calib::guidance_scores::claims::SummaryScope::Annual { year } => year.to_string(),
            fmine_calib::guidance_scores::claims::SummaryScope::SiteHistory { years, .. } => years
                .iter()
                .map(u16::to_string)
                .collect::<Vec<_>>()
                .join(", "),
        };
        let _ = writeln!(
            out,
            "### {} · {} · {} · guidance published {}\n\n\
             | Members | Resolved | Missing | Printed hits | Rounding boundary |\n|---|---|---|---|---|\n| {} | {} | {} | {} | {} |\n\n\
             Whole-cohort hit-rate bounds: **{}**. Rounding-aware bounds: **{}**.\n",
            text(&s.metric),
            period,
            text(&group.basis),
            text(&group.published_at),
            s.members,
            s.resolved,
            s.unresolved,
            s.printed_hits,
            s.boundary,
            interval(s.whole_cohort_hit_rate_bounds),
            interval(s.rounding_aware_hit_rate_bounds),
        );
        if let Some(mean) = s.mean_z {
            let _ = writeln!(
                out,
                "Mean normalized position among {} resolved range targets: {}. This excludes unresolved outcomes and is not a confidence interval.\n",
                s.z_members,
                number(mean)
            );
        }
        if let Some((lo, hi)) = s.binomial_90 {
            let _ = writeln!(
                out,
                "Conditional binomial interval at the ninety-percent level: approximately {lo:.6} to {hi:.6}. \
                 This assumes independent, identically distributed hits among resolved members. Shared site and year shocks may invalidate that assumption.\n"
            );
        }
        if let Some(bounds) = s.binomial_90_exact {
            let _ = writeln!(
                out,
                "Certified rational enclosure of the IID reference endpoints: {}.\n",
                interval(bounds)
            );
        }
    }
    out.push_str("Effective sample size and bias confidence intervals have not been estimated. No price-conditional split is supplied without verified price assumptions and the site's royalty proportionality evidence.\n\n");
    if !report.pending_transcriptions.is_empty() {
        out.push_str("## Pending reader agreement\n\nThese locators need a second independent reading before they can enter a scored cohort:\n\n");
        for locator in &report.pending_transcriptions {
            let _ = writeln!(out, "- {}", text(locator));
        }
        out.push('\n');
    }
    if !report.unsupported_guidance.is_empty() {
        out.push_str("## Guidance awaiting metric support\n\nThese agreed source rows remain visible but do not enter the supported-metric cohorts.\n\n| Quantity | Printed target | Source locator | SHA-256 |\n|---|---|---|---|\n");
        for value in &report.unsupported_guidance {
            let _ = writeln!(
                out,
                "| {} | {} {} | {} | `{}` |",
                text(&value.quantity),
                text(&value.token),
                text(&value.unit),
                text(&value.locator.to_string()),
                text(&value.doc_sha256)
            );
        }
        out.push('\n');
    }
    let mut anchors = BTreeSet::new();
    for row in &report.rows {
        anchors.extend(row.guidance.anchors.iter());
        for appearance in std::iter::once(&row.original).chain(&row.later) {
            for source in &appearance.source_claims {
                anchors.extend(source.claim.anchors.iter());
            }
        }
    }
    out.push_str("## Source citations\n\n| Source | Locator | SHA-256 |\n|---|---|---|\n");
    for anchor in anchors {
        let _ = writeln!(
            out,
            "| {} | {} | `{}` |",
            text(&anchor.artifact_path),
            text(&anchor.locator),
            Digest32(anchor.artifact_sha256).to_hex(),
        );
    }
    out
}
