//! DESIGN (BD-341.S4): source-bound cluster projection beside the unchanged dated track record.
//! No source, vintage or missing member is selected by its numerical value or bootstrap result.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use fmine_calib::guidance_scores::{self, clusters as kernel};
use fmine_core::{Digest32, Encoder, blake3_hash};
use fmine_evidence::{Refusal, RefusalCode};

use super::{
    Interval, Rat, TrackRecordReport, TrackRecordRow, js, normalized_basis, stated_guidance,
};

pub use kernel::Axis;

pub const SCHEMA: &str = "fmine.guidance.track-record.clusters.v1";
pub const NO_CLAIM: &str = "Descriptive site/year partitions with an explicitly selected one-axis percentile block bootstrap. Independent selected clusters are assumed, not established; simultaneous site and year dependence is not covered. The estimand gives each cluster equal weight. Sparse, incomplete or degenerate populations retain their evidence and have no sampling interval. No calibration, site ranking, effective-sample-size estimate, causal attribution or historical-registration claim.";

#[derive(Debug, Clone)]
pub struct MemberSources {
    pub id: String,
    pub guidance: Vec<String>,
    pub outcomes: Vec<String>,
    pub outcome_claims: Vec<String>,
    pub status: String,
}

#[derive(Debug, Clone)]
pub struct Group {
    pub metric: String,
    pub unit: String,
    pub basis: String,
    pub selector: &'static str,
    pub sources: Vec<MemberSources>,
    pub assessment: kernel::ClusterReport,
}

/// The immutable wrapper binds the entire original source report, both outcome selectors and inference policy.
#[derive(Debug, Clone)]
pub struct Report {
    source: TrackRecordReport,
    axis: Axis,
    groups: Vec<Group>,
    digest: Digest32,
}

fn error(message: &str) -> Refusal {
    Refusal::new(
        RefusalCode::FrozenRecordMutation507,
        format!("guidance cluster projection: {message}"),
    )
}

type Families<'a> =
    BTreeMap<(String, String, String), BTreeMap<(String, u16), Vec<&'a TrackRecordRow>>>;

/// The source adapter already verified bytes, dates, bases and reader agreement. Recheck its bound
/// interpretation, then retain every earliest-guidance family, including ties and unusable outcomes.
pub fn assess(source: TrackRecordReport, axis: Axis) -> Result<Report, Refusal> {
    if source.rows.iter().any(|row| {
        [&row.first_published, &row.latest_registered]
            .iter()
            .any(|outcome| outcome.score.is_some() && outcome.sources.is_empty())
    }) {
        return Err(error("scored outcome has no retained source citation"));
    }
    if source.digest != blake3_hash(source.body().as_bytes()) {
        return Err(error("base report differs from its bound digest"));
    }
    let mut families = Families::new();
    for row in source.rows.iter().filter(|row| row.earliest_registered) {
        families
            .entry((
                row.metric.clone(),
                row.guidance.unit.clone(),
                normalized_basis(&row.guidance, &row.metric),
            ))
            .or_default()
            .entry((row.site.clone(), row.year))
            .or_default()
            .push(row);
    }
    let mut groups = Vec::new();
    for ((metric, unit, basis), family) in families {
        for latest in [false, true] {
            let mut members = Vec::new();
            let mut sources = Vec::new();
            for ((site, year), rows) in &family {
                let id = format!("{site}/{year}");
                let mut member = kernel::Member {
                    id: id.clone(),
                    site: site.clone(),
                    year: *year,
                    guidance: None,
                    outcome: None,
                };
                let mut links = MemberSources {
                    id,
                    guidance: Vec::new(),
                    outcomes: Vec::new(),
                    outcome_claims: Vec::new(),
                    status: "ambiguous_earliest_guidance".into(),
                };
                for row in rows {
                    links.guidance.push(format!(
                        "{}@{}",
                        row.guidance.locator, row.guidance.doc_sha256
                    ));
                    let outcome = if latest {
                        &row.latest_registered
                    } else {
                        &row.first_published
                    };
                    for cell in &outcome.sources {
                        links.outcomes.push(format!(
                            "{}@{}",
                            cell.locator, cell.key.reporting_vintage.file_sha256
                        ));
                        links.outcome_claims.push(cell.claim.claim_id.to_string());
                    }
                    if rows.len() == 1 && !row.ambiguous_earliest {
                        let guidance = stated_guidance(&row.guidance)?;
                        let score = outcome
                            .printed
                            .map(|p| guidance_scores::score(guidance, p))
                            .transpose()
                            .map_err(|_| error("retained source outcome cannot be scored"))?;
                        if score != outcome.score
                            || (outcome.printed.is_some() && outcome.refusal.is_some())
                        {
                            return Err(error("retained outcome and source-bound score disagree"));
                        }
                        member.guidance = Some(guidance);
                        member.outcome = outcome.printed;
                        links.status = outcome.status.into();
                    }
                }
                links.guidance.sort();
                links.outcomes.sort();
                links.outcome_claims.sort();
                links.outcome_claims.dedup();
                members.push(member);
                sources.push(links);
            }
            groups.push(Group {
                metric: metric.clone(),
                unit: unit.clone(),
                basis: basis.clone(),
                selector: if latest {
                    "latest_registered"
                } else {
                    "first_published"
                },
                assessment: kernel::assess(&members, axis)?,
                sources,
            });
        }
    }
    let mut report = Report {
        source,
        axis,
        groups,
        digest: Digest32::default(),
    };
    let mut frame = Encoder::new_frame(*b"GTCL", 1);
    frame.put_str(&report.body());
    report.digest = blake3_hash(&frame.finish());
    Ok(report)
}

fn rat(value: Rat) -> String {
    js(&format!("{}/{}", value.num(), value.den()))
}
fn optional(value: Option<Rat>) -> String {
    value.map_or_else(|| "null".into(), rat)
}
fn interval(value: Option<Interval>) -> String {
    value.map_or_else(
        || "null".into(),
        |i| {
            format!(
                "{{\"lo\":{},\"hi\":{},\"lo_strict\":{},\"hi_strict\":{}}}",
                rat(i.lo),
                rat(i.hi),
                i.lo_strict,
                i.hi_strict
            )
        },
    )
}
fn strings(values: &[String]) -> String {
    values.iter().map(|s| js(s)).collect::<Vec<_>>().join(",")
}

fn cluster_json(c: &kernel::Cluster) -> String {
    format!(
        "{{\"id\":{},\"members\":[{}],\"n_members\":{},\"n_resolved\":{},\"n_unknown_guidance\":{},\"n_range_members\":{},\"n_resolved_ranges\":{},\"observed_hit_rate\":{},\"hit_completion_bounds\":{},\"rounding_hit_completion_bounds\":{},\"observed_mean_z\":{},\"observed_mean_z_rounding_bounds\":{}}}",
        js(&c.id),
        strings(&c.members),
        c.members.len(),
        c.resolved,
        c.unknown_guidance,
        c.range_members,
        c.resolved_ranges,
        optional(c.hit_rate),
        interval(Some(c.hit_completion)),
        interval(Some(c.rounding_hit_completion)),
        optional(c.observed_mean_z),
        interval(c.observed_mean_z_rounding),
    )
}

fn projection_json(p: &kernel::Projection) -> String {
    format!(
        "{{\"equal_cluster_point\":{},\"rounding_bounds\":{},\"completion_bounds\":{},\"nominal_90_percentile_interval\":{},\"percentile_interval_rounding_enclosure\":{},\"inference_unavailable\":{}}}",
        optional(p.point),
        interval(p.rounding_bounds),
        interval(p.completion_bounds),
        interval(p.interval_90),
        interval(p.interval_90_rounding_enclosure),
        p.unavailable.map_or_else(|| "null".into(), js),
    )
}

impl Report {
    #[must_use]
    pub fn groups(&self) -> &[Group] {
        &self.groups
    }
    #[must_use]
    pub fn source(&self) -> &TrackRecordReport {
        &self.source
    }
    #[must_use]
    pub fn digest(&self) -> Digest32 {
        self.digest
    }

    fn body(&self) -> String {
        let groups = self.groups.iter().map(|g| {
            let a = &g.assessment;
            let sources = g.sources.iter().map(|s| format!(
                "{{\"id\":{},\"status\":{},\"guidance_sources\":[{}],\"outcome_sources\":[{}],\"outcome_claims\":[{}]}}",
                js(&s.id), js(&s.status), strings(&s.guidance), strings(&s.outcomes), strings(&s.outcome_claims),
            )).collect::<Vec<_>>().join(",");
            format!(
                "{{\"metric\":{},\"unit\":{},\"basis\":{},\"selector\":{},\"n_members\":{},\"n_unresolved\":{},\"n_year_clusters\":{},\"n_site_clusters\":{},\"members\":[{sources}],\"year_clusters\":[{}],\"site_clusters\":[{}],\"hit_rate\":{},\"mean_z\":{}}}",
                js(&g.metric), js(&g.unit), js(&g.basis), js(g.selector), a.n_members, a.n_unresolved, a.years.len(), a.sites.len(),
                a.years.iter().map(cluster_json).collect::<Vec<_>>().join(","), a.sites.iter().map(cluster_json).collect::<Vec<_>>().join(","),
                projection_json(&a.hit), projection_json(&a.bias),
            )
        }).collect::<Vec<_>>().join(",");
        format!(
            "\"schema\":\"{SCHEMA}\",\"source_report_digest\":{},\"selected_axis\":{},\"rules_version\":{},\"bootstrap_replicates\":{},\"seed\":{},\"minimum_clusters\":{},\"method\":{},\"assumption_verified\":false,\"two_way_cluster_robust\":false,\"effective_sample_size\":null,\"flag\":\"descriptive\",\"groups\":[{groups}],\"source_report\":{},\"no_claim\":{}",
            js(&self.source.digest.to_hex()),
            js(self.axis.as_str()),
            js(kernel::RULE),
            kernel::REPLICATES,
            js(&format!("{:016x}", kernel::SEED)),
            kernel::MIN_CLUSTERS,
            js(kernel::METHOD),
            self.source.to_json(),
            js(NO_CLAIM),
        )
    }

    #[must_use]
    pub fn to_json(&self) -> String {
        format!(
            "{{\"digest\":{},{}}}",
            js(&self.digest.to_hex()),
            self.body()
        )
    }

    #[must_use]
    pub fn to_markdown(&self) -> String {
        let mut out = format!(
            "# Guidance cluster uncertainty\n\n{NO_CLAIM}\n\nSelected independent-cluster assumption: **{}**. {method}\n\nThis projection's nominal intervals are approximate sampling intervals. They are not the legacy report's IID binomial intervals. Rounding enclosures additionally propagate the source intervals through the same draws; they are not another confidence level.\n\n",
            self.axis.as_str(),
            method = kernel::METHOD
        );
        let shown = |x: Option<Rat>| {
            x.map_or_else(
                || "unavailable".into(),
                |v| format!("{}/{}", v.num(), v.den()),
            )
        };
        let bounds = |x: Option<Interval>| {
            x.map_or_else(
                || "unavailable".into(),
                |i| format!("[{}, {}]", shown(Some(i.lo)), shown(Some(i.hi))),
            )
        };
        let text = |s: &str| s.replace('|', "\\|").replace(['\r', '\n'], " ");
        for group in &self.groups {
            let a = &group.assessment;
            let _ = writeln!(
                out,
                "## {} / {} / {}\n\n{} members; {} unresolved; {} year clusters; {} site clusters. Unit: {}.\n\n| Statistic | Equal-cluster mean | Nominal 90% sampling interval | Rounding enclosure of interval | Unavailable reason |\n|---|---|---|---|---|",
                text(&group.metric),
                text(&group.basis),
                group.selector,
                a.n_members,
                a.n_unresolved,
                a.years.len(),
                a.sites.len(),
                text(&group.unit)
            );
            for (name, p) in [
                ("Printed guidance hit rate", &a.hit),
                ("Normalized range position z", &a.bias),
            ] {
                let _ = writeln!(
                    out,
                    "| {name} | {} | {} | {} | {} |",
                    shown(p.point),
                    bounds(p.interval_90),
                    bounds(p.interval_90_rounding_enclosure),
                    p.unavailable.unwrap_or("—")
                );
            }
            let _ = writeln!(
                out,
                "\nEqual-cluster missing-outcome hit bounds: {}. Rounding-aware bounds: {}. These describe the registered finite population, not a sampling confidence interval.\n",
                bounds(a.hit.completion_bounds),
                bounds(a.hit.rounding_bounds)
            );
            out.push_str("| Partition | Cluster | Declared / resolved members | Range / resolved range members | Observed mean z |\n|---|---|---|---|---|\n");
            for (axis, clusters) in [("year", &a.years), ("site", &a.sites)] {
                for c in clusters {
                    let _ = writeln!(
                        out,
                        "| {axis} | {} | {} / {} | {} / {} | {} |",
                        text(&c.id),
                        c.members.len(),
                        c.resolved,
                        c.range_members,
                        c.resolved_ranges,
                        shown(c.observed_mean_z)
                    );
                }
            }
            out.push('\n');
        }
        let _ = writeln!(
            out,
            "Cluster projection digest: `{}`. Unchanged base report digest: `{}`.\n\n## Complete source comparison\n\n{}",
            self.digest.to_hex(),
            self.source.digest.to_hex(),
            self.source.to_markdown()
        );
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_bound_projection_keeps_original_missing_and_later_appearance_separate() {
        let source = super::super::synthetic().unwrap();
        let bytes = source.to_json();
        let projection = assess(source.clone(), Axis::Year).unwrap();
        assert_eq!(projection.source().to_json(), bytes);
        assert_eq!(projection.groups.len(), 2);
        assert!(
            projection
                .groups
                .iter()
                .all(|g| g.assessment.n_members == 2 && g.assessment.n_unresolved == 1)
        );
        assert_ne!(
            projection.groups[0].assessment.years[0].observed_mean_z,
            projection.groups[1].assessment.years[0].observed_mean_z
        );
        assert!(
            projection
                .groups
                .iter()
                .all(|g| g.assessment.bias.unavailable == Some("unresolved_members"))
        );
        let json = projection.to_json();
        assert!(json.contains(&source.digest.to_hex()));
        assert!(json.contains("\"two_way_cluster_robust\":false"));
        assert!(json.contains(&source.rows[0].guidance.doc_sha256));
        assert!(
            projection
                .to_markdown()
                .contains("Complete source comparison")
        );
        assert_eq!(
            projection.to_json(),
            assess(source.clone(), Axis::Year).unwrap().to_json()
        );
        assert_ne!(
            projection.digest(),
            assess(source.clone(), Axis::Site).unwrap().digest()
        );
        let mut tampered = source;
        tampered.rows[0].guidance.token = "200 - 300".into();
        assert_eq!(
            assess(tampered, Axis::Year).unwrap_err().code,
            "FMINE-REF-507"
        );
    }

    #[test]
    fn cached_printed_outcome_cannot_override_the_source_bound_score() {
        let mut source = super::super::synthetic().unwrap();
        let outcome = &mut source.rows[0].first_published;
        let value = Rat::from_integer(115);
        outcome.printed = Some(guidance_scores::PrintedOutcome {
            value,
            interval: Interval {
                lo: value,
                hi: value,
                lo_strict: false,
                hi_strict: false,
            },
        });
        assert_eq!(
            assess(source, Axis::Year).unwrap_err().code,
            "FMINE-REF-507"
        );

        // The legacy renderer indexes the cited source of a score. Refuse malformed public
        // report structures before recomputing their body, so tampering never reaches that index.
        let mut source = super::super::synthetic().unwrap();
        assert!(source.rows[0].first_published.score.is_some());
        source.rows[0].first_published.sources.clear();
        assert_eq!(
            assess(source, Axis::Year).unwrap_err().code,
            "FMINE-REF-507"
        );
    }
}
