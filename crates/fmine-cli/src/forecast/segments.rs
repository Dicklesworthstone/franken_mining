//! Prospective forecast identity and descriptive segmentation (BD-049, plan §10.1).
//! Rendering delegates every score and denominator to the calibration ledger.
//! Determinism: Exact (only renders the saved kernel values and identities).

use std::fmt::Write as _;

use fmine_calib::ledger::segments::{DescriptiveSummary, ForecastMetadata, SegmentedAssessment};
use fmine_calib::{Assessment, ScoreSummary};
use fmine_core::ObjectId;
use fmine_evidence::Refusal;

use super::{
    Replay, Row, coverage_json, display_or, js, missing_name, optional_number, score_json,
    selector_name, unresolved_json, utc,
};

pub(super) const FIELDS: [&str; 7] = [
    "family",
    "author_role",
    "domain",
    "horizon",
    "site",
    "quantity",
    "unit",
];
pub(super) const ROBOT_DOCS_JSON: &str = r#"{"forecast_fields":["family","author_role","domain","horizon","site","quantity","unit"],"declaration":"All seven quoted fields are required together at registration; use a role or model/agent identifier for author_role, never personal details in committed examples","identity":"Metadata is immutable and bound into the forecast and cohort identities; omission preserves existing record bytes","grouping":"Full model_key, family, author_role, domain, horizon, site, quantity and unit; no implicit unit conversion or dimensional pooling","output":"Descriptive scores, source member IDs and missing-outcome completion bounds per segment; no segment calibration verdict or inferred independence","journal":"Metadata registrations use operation v7; older operation schemas reject these fields"}"#;

pub(super) fn parse(row: &Row<'_>) -> Result<Option<ForecastMetadata>, Refusal> {
    if !FIELDS.iter().any(|key| row.0.get(key).is_some()) {
        return Ok(None);
    }
    Ok(Some(ForecastMetadata {
        family: row.string("family")?.into(),
        author_role: row.string("author_role")?.into(),
        domain: row.string("domain")?.into(),
        horizon: row.string("horizon")?.into(),
        site: row.string("site")?.into(),
        quantity: row.string("quantity")?.into(),
        unit: row.string("unit")?.into(),
    }))
}

fn metadata_json(metadata: &ForecastMetadata) -> String {
    format!(
        "{{\"family\":{},\"author_role\":{},\"domain\":{},\"horizon\":{},\"site\":{},\"quantity\":{},\"unit\":{}}}",
        js(&metadata.family),
        js(&metadata.author_role),
        js(&metadata.domain),
        js(&metadata.horizon),
        js(&metadata.site),
        js(&metadata.quantity),
        js(&metadata.unit),
    )
}

pub(super) fn metadata_field(metadata: Option<&ForecastMetadata>) -> String {
    metadata.map_or_else(String::new, |value| {
        format!(",\"metadata\":{}", metadata_json(value))
    })
}

fn selected(state: &Replay, report: &Assessment) -> Result<Option<SegmentedAssessment>, Refusal> {
    if !report.per_member.iter().any(|member| {
        state
            .ledger
            .forecast(member.forecast_id)
            .is_some_and(|forecast| forecast.metadata.is_some())
    }) {
        return Ok(None);
    }
    state.ledger.segmented_assessment(report.digest).map(Some)
}

fn ids(members: &[ObjectId]) -> String {
    members
        .iter()
        .map(|id| js(&id.to_string()))
        .collect::<Vec<_>>()
        .join(",")
}

fn summary_json(summary: &DescriptiveSummary) -> String {
    let reasons = unresolved_json(&summary.unresolved_by_reason);
    let coverage = coverage_json(summary.coverage);
    let event_log = summary.event_log.map_or_else(|| "null".into(), |log| format!(
        "{{\"probability_floor\":{},\"units\":\"nats\",\"strictly_proper\":false,\"score\":{}}}",
        log.probability_floor, score_json(Some(log.score)),
    ));
    format!(
        "{{\"n_members\":{},\"n_resolved\":{},\"n_unresolved\":{},\"unresolved_by_reason\":{{{reasons}}},\"coverage\":{coverage},\"crps\":{},\"gaussian_crps_bounds\":{},\"interval_score\":{},\"brier\":{},\"event_log\":{event_log},\"mean_standardized_bias\":{}}}",
        summary.n_members,
        summary.n_resolved,
        summary.n_unresolved,
        score_json(summary.crps),
        summary.gaussian_crps_bounds,
        score_json(summary.interval_score),
        score_json(summary.brier),
        optional_number(summary.mean_standardized_bias),
    )
}

pub(super) fn assessment_json(state: &Replay, report: &Assessment) -> Result<String, Refusal> {
    let Some(segmented) = selected(state, report)? else {
        return Ok(String::new());
    };
    let groups = segmented
        .groups
        .iter()
        .map(|group| {
            format!(
                "{{\"model_key\":{},\"metadata\":{},\"members\":[{}],\"summary\":{}}}",
                js(&group.segment.model_key),
                metadata_json(&group.segment.metadata),
                ids(&group.members),
                summary_json(&group.summary),
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    Ok(format!(
        ",\"segmented_assessment\":{{\"schema\":\"fmine.forecast-segments.v1\",\"assessment_digest\":{},\"digest\":{},\"as_of\":{},\"selector\":{},\"alpha\":{},\"n_members\":{},\"interpretation\":\"descriptive_only\",\"bounds_kind\":\"whole_segment_completion_bounds_not_confidence_intervals\",\"groups\":[{groups}],\"unclassified_members\":[{}]}}",
        js(&segmented.assessment_digest.to_hex()),
        js(&segmented.digest.to_hex()),
        utc(segmented.as_of_ns),
        js(selector_name(segmented.selector)),
        segmented.alpha,
        segmented.n_members,
        ids(&segmented.unclassified_members),
    ))
}

fn score_markdown(out: &mut String, name: &str, score: Option<ScoreSummary>, unit: &str) {
    if let Some(score) = score {
        let _ = writeln!(
            out,
            "| {name} | {} | {} | {} | {} | {} |",
            score.n_scored,
            display_or(score.observed_mean, "unscored"),
            score.whole_cohort.lower,
            display_or(score.whole_cohort.upper, "unbounded"),
            crate::guidance::text(unit),
        );
    }
}

pub(super) fn assessment_markdown(
    out: &mut String,
    state: &Replay,
    report: &Assessment,
) -> Result<(), Refusal> {
    use crate::guidance::text;
    let Some(segmented) = selected(state, report)? else {
        return Ok(());
    };
    let _ = writeln!(
        out,
        "\n## Forecast segments\n\nAll grouping labels were frozen with each forecast. These are descriptive comparisons; no subgroup calibration verdict, independence estimate or ranking is inferred. Missing outcomes remain in each segment's completion bounds, which are not confidence intervals.\n\nParent assessment: `{}`. Segment report: `{}`. Alpha: {}. Members: {}.\n",
        segmented.assessment_digest.to_hex(),
        segmented.digest.to_hex(),
        segmented.alpha,
        segmented.n_members,
    );
    for group in &segmented.groups {
        let m = &group.segment.metadata;
        let s = &group.summary;
        let _ = writeln!(
            out,
            "### {} / {}\n\nFamily: {}. Author role: {}. Domain: {}. Horizon: {}. Quantity and basis: {}. Unit: {}.\n\nMembers: {}; resolved: {}; unresolved: {}.\n\nForecast IDs: {}.\n",
            text(&group.segment.model_key),
            text(&m.site),
            text(&m.family),
            text(&m.author_role),
            text(&m.domain),
            text(&m.horizon),
            text(&m.quantity),
            text(&m.unit),
            s.n_members,
            s.n_resolved,
            s.n_unresolved,
            group
                .members
                .iter()
                .map(|id| format!("`{id}`"))
                .collect::<Vec<_>>()
                .join(", "),
        );
        if let Some(coverage) = s.coverage {
            let _ = writeln!(
                out,
                "Interval hits: {}. Observed fraction: {}. Full-segment coverage completion bounds: [{}, {}].\n",
                coverage.inside,
                display_or(coverage.observed_fraction, "unscored"),
                coverage.whole_cohort.0,
                coverage.whole_cohort.1,
            );
        }
        for (reason, count) in &s.unresolved_by_reason {
            let _ = writeln!(out, "Unresolved {}: {count}.\n", missing_name(*reason));
        }
        out.push_str("| Score | Scored members | Observed mean | Completion lower | Completion upper | Unit |\n|---|---|---|---|---|---|\n");
        score_markdown(out, "CRPS", s.crps, &m.unit);
        score_markdown(out, "Interval score", s.interval_score, &m.unit);
        score_markdown(out, "Brier", s.brier, "dimensionless");
        if let Some(log) = s.event_log {
            score_markdown(out, "Capped event log loss", Some(log.score), "nats");
            let _ = writeln!(
                out,
                "\nFrozen log-loss floor: {}. The cap removes strict propriety in the floored tails; probabilities and Brier scores remain unchanged.\n",
                log.probability_floor,
            );
        }
        if let Some(bias) = s.mean_standardized_bias {
            let _ = writeln!(out, "\nMean standardized Gaussian bias: {bias}.\n");
        }
    }
    Ok(())
}
