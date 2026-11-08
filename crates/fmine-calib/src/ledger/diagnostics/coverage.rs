//! DESIGN (BD-048/049, plan §10.2): central 50/80/95% coverage of saved forecast assessments.
//! Determinism: DeterministicPerIsa (existing normal quantiles and empirical order statistics).
//! Missing levels are never interpolated; unsupported members remain visible beside each denominator.

use std::fmt::Write as _;

use fmine_core::codec::Encoder;
use fmine_core::hash::{Digest32, blake3_hash};
use fmine_evidence::Refusal;

use super::super::{
    ForecastLedger, ForecastPayload, MemberAssessment, MemberOutcome, OutcomeSelector, absent,
    floats, score_refusal,
};
use crate::scoring::SortedEmpirical;

pub const METHOD: &str = "Central 50/80/95 percent intervals use the frozen Gaussian distribution or inverse empirical CDF; explicit intervals and quantiles require exactly matching declared levels. Bounds are inclusive. Empirical ties may give more than the nominal mass. Completion bounds use all supported members, including unresolved outcomes; unsupported levels are counted separately. These are descriptive arithmetic bounds, not confidence intervals, calibration verdicts or independence claims.";
const LEVELS: [(u8, f64); 3] = [(50, 0.5), (80, 0.2), (95, 0.05)];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoverageStatus {
    Inside,
    Below,
    Above,
    Unresolved,
    EventDistribution,
    UnstatedLevel,
    MissingLevel,
    UnrepresentableInterval,
}

impl CoverageStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Inside => "inside",
            Self::Below => "below",
            Self::Above => "above",
            Self::Unresolved => "unresolved",
            Self::EventDistribution => "event_distribution",
            Self::UnstatedLevel => "unstated_nominal_level",
            Self::MissingLevel => "level_not_supplied",
            Self::UnrepresentableInterval => "unrepresentable_interval",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CoverageInterval {
    /// Absent exactly when the frozen payload cannot supply this central interval.
    pub bounds: Option<(f64, f64)>,
    pub status: CoverageStatus,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CoverageMember {
    /// Exact selection and source evidence from the parent, never a newly selected outcome.
    pub member: MemberAssessment,
    /// Fixed order: 50%, 80%, 95%.
    pub intervals: [CoverageInterval; 3],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CoverageLevel {
    pub percent: u8,
    pub n_supported: usize,
    pub n_unsupported: usize,
    pub n_unresolved: usize,
    pub inside: usize,
    pub below: usize,
    pub above: usize,
}

impl CoverageLevel {
    pub fn n_resolved(&self) -> usize {
        self.inside + self.below + self.above
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CoverageDiagnostics {
    pub assessment_digest: Digest32,
    pub cohort: Digest32,
    pub as_of_ns: i64,
    pub selector: OutcomeSelector,
    pub members: Vec<CoverageMember>,
    pub levels: [CoverageLevel; 3],
    pub digest: Digest32,
}

fn interval(
    payload: &ForecastPayload,
    empirical: Option<&SortedEmpirical>,
    outcome: Option<f64>,
    (percent, alpha): (u8, f64),
) -> CoverageInterval {
    use CoverageStatus::*;
    let unavailable = |status| CoverageInterval {
        bounds: None,
        status,
    };
    let bounds = match payload {
        ForecastPayload::Bernoulli { .. } => return unavailable(EventDistribution),
        ForecastPayload::Interval { nominal: None, .. } => return unavailable(UnstatedLevel),
        ForecastPayload::Interval { lo, hi, nominal } => {
            if *nominal != Some(f64::from(percent) / 100.0) {
                return unavailable(MissingLevel);
            }
            Some((*lo, *hi))
        }
        ForecastPayload::Quantiles { levels, values } => {
            let at = |level| levels.iter().position(|p| *p == level).map(|i| values[i]);
            match (at(alpha / 2.0), at(1.0 - alpha / 2.0)) {
                (Some(lo), Some(hi)) => Some((lo, hi)),
                _ => return unavailable(MissingLevel),
            }
        }
        ForecastPayload::Empirical { .. } => empirical.and_then(|e| e.interval(alpha).ok()),
        ForecastPayload::Gaussian { .. } => payload.interval(alpha).ok().flatten(),
    };
    let Some((lo, hi)) = bounds.filter(|(lo, hi)| lo.is_finite() && hi.is_finite()) else {
        return unavailable(UnrepresentableInterval);
    };
    CoverageInterval {
        bounds: Some((lo, hi)),
        status: match outcome {
            None => Unresolved,
            Some(y) if y < lo => Below,
            Some(y) if y > hi => Above,
            Some(_) => Inside,
        },
    }
}

impl ForecastLedger {
    /// Read-only CVG1/v1 projection. Prior forecast, cohort, assessment and DGN1 identities are unchanged.
    pub fn coverage_diagnostics(&self, digest: Digest32) -> Result<CoverageDiagnostics, Refusal> {
        let assessment = self
            .assessments
            .values()
            .find(|a| a.digest == digest)
            .ok_or_else(|| {
                absent("coverage diagnostics require an existing immutable assessment")
            })?;
        let mut levels = LEVELS.map(|(percent, _)| CoverageLevel {
            percent,
            n_supported: 0,
            n_unsupported: 0,
            n_unresolved: 0,
            inside: 0,
            below: 0,
            above: 0,
        });
        let mut members = Vec::with_capacity(assessment.per_member.len());
        for member in &assessment.per_member {
            let payload = &self
                .records
                .get(&member.forecast_id)
                .ok_or_else(|| absent("coverage member's frozen forecast is unavailable"))?
                .payload;
            let outcome = match member.outcome {
                MemberOutcome::Resolved { value, .. } => Some(value),
                MemberOutcome::Unresolved { .. } => None,
            };
            let empirical = match payload {
                ForecastPayload::Empirical { samples } => {
                    Some(SortedEmpirical::new(samples).map_err(score_refusal)?)
                }
                _ => None,
            };
            let intervals =
                LEVELS.map(|level| interval(payload, empirical.as_ref(), outcome, level));
            for (summary, value) in levels.iter_mut().zip(intervals) {
                summary.n_supported += usize::from(value.bounds.is_some());
                summary.n_unsupported += usize::from(value.bounds.is_none());
                match value.status {
                    CoverageStatus::Inside => summary.inside += 1,
                    CoverageStatus::Below => summary.below += 1,
                    CoverageStatus::Above => summary.above += 1,
                    CoverageStatus::Unresolved => summary.n_unresolved += 1,
                    _ => {}
                }
            }
            members.push(CoverageMember {
                member: member.clone(),
                intervals,
            });
        }
        let mut report = CoverageDiagnostics {
            assessment_digest: digest,
            cohort: assessment.cohort,
            as_of_ns: assessment.as_of_ns,
            selector: assessment.selector,
            members,
            levels,
            digest: Digest32::default(),
        };
        report.digest = blake3_hash(&report.canonical_bytes());
        Ok(report)
    }
}

impl CoverageDiagnostics {
    pub fn canonical_bytes(&self) -> Vec<u8> {
        let mut enc = Encoder::new_frame(*b"CVG1", 1);
        enc.put_str(METHOD);
        enc.put_fixed(&self.assessment_digest.0);
        enc.put_fixed(&self.cohort.0);
        enc.put_i64(self.as_of_ns);
        enc.put_u8(u8::from(self.selector == OutcomeSelector::LatestAsOf));
        for level in self.levels {
            enc.put_u8(level.percent);
            for count in [
                level.n_supported,
                level.n_unsupported,
                level.n_unresolved,
                level.inside,
                level.below,
                level.above,
            ] {
                enc.put_u64(count as u64);
            }
        }
        enc.put_u64(self.members.len() as u64);
        for member in &self.members {
            member.member.encode(&mut enc);
            for value in member.intervals {
                enc.put_str(value.status.as_str());
                enc.put_u8(u8::from(value.bounds.is_some()));
                if let Some((lo, hi)) = value.bounds {
                    floats(&mut enc, &[lo, hi]);
                }
            }
        }
        enc.finish()
    }

    /// Exact count fractions; zero supported or resolved populations are null, never 0/0 or zero coverage.
    pub fn to_json(&self) -> String {
        let fraction = |num: usize, den: usize| {
            if den == 0 {
                "null".into()
            } else {
                format!("{{\"num\":{num},\"den\":{den}}}")
            }
        };
        let levels = self.levels.iter().map(|s| format!(
            "{{\"percent\":{},\"n_supported\":{},\"n_unsupported\":{},\"n_resolved\":{},\"n_unresolved\":{},\"inside\":{},\"below\":{},\"above\":{},\"observed_fraction\":{},\"whole_supported_fraction\":{{\"lower\":{},\"upper\":{}}}}}",
            s.percent, s.n_supported, s.n_unsupported, s.n_resolved(), s.n_unresolved, s.inside, s.below, s.above,
            fraction(s.inside, s.n_resolved()), fraction(s.inside, s.n_supported), fraction(s.inside + s.n_unresolved, s.n_supported),
        )).collect::<Vec<_>>().join(",");
        let members = self
            .members
            .iter()
            .map(|m| {
                let intervals = LEVELS
                    .iter()
                    .zip(m.intervals)
                    .map(|((percent, _), v)| {
                        let bounds = v
                            .bounds
                            .map_or_else(|| "null".into(), |(lo, hi)| format!("[{lo},{hi}]"));
                        format!(
                            "{{\"percent\":{percent},\"bounds\":{bounds},\"status\":\"{}\"}}",
                            v.status.as_str()
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(",");
                format!(
                    "{{\"forecast_id\":\"{}\",\"intervals\":[{intervals}]}}",
                    m.member.forecast_id
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        let selector = match self.selector {
            OutcomeSelector::FirstPublished => "first-published",
            OutcomeSelector::LatestAsOf => "latest-as-of",
        };
        format!(
            "{{\"schema\":\"fmine.forecast-coverage.v1\",\"assessment_digest\":\"{}\",\"cohort\":\"{}\",\"digest\":\"{}\",\"as_of_ns\":{},\"selector\":\"{selector}\",\"method\":\"{METHOD}\",\"n_members\":{},\"levels\":[{levels}],\"members\":[{members}]}}",
            self.assessment_digest,
            self.cohort,
            self.digest,
            self.as_of_ns,
            self.members.len()
        )
    }

    pub fn to_markdown(&self) -> String {
        let mut out = format!(
            "\n### Central interval coverage\n\n{METHOD}\n\nParent assessment: `{}`. Coverage report: `{}`.\n\n| Level | Supported / all | Resolved | Inside | Below | Above | Unresolved | Completion bounds |\n|---|---:|---:|---:|---:|---:|---:|---|\n",
            self.assessment_digest, self.digest
        );
        for s in self.levels {
            let bounds = if s.n_supported == 0 {
                "unavailable".into()
            } else {
                format!(
                    "[{}/{}, {}/{}]",
                    s.inside,
                    s.n_supported,
                    s.inside + s.n_unresolved,
                    s.n_supported
                )
            };
            let _ = writeln!(
                out,
                "| {}% | {} / {} | {} | {} | {} | {} | {} | {bounds} |",
                s.percent,
                s.n_supported,
                self.members.len(),
                s.n_resolved(),
                s.inside,
                s.below,
                s.above,
                s.n_unresolved
            );
        }
        out.push_str("\nEvery forecast links to its exact selected outcome and source in the parent assessment.\n\n| Forecast | Level | Inclusive interval | Result |\n|---|---:|---|---|\n");
        for m in &self.members {
            for ((percent, _), v) in LEVELS.iter().zip(m.intervals) {
                let bounds = v
                    .bounds
                    .map_or_else(|| "unavailable".into(), |(lo, hi)| format!("[{lo}, {hi}]"));
                let _ = writeln!(
                    out,
                    "| `{}` | {percent}% | {bounds} | {} |",
                    m.member.forecast_id,
                    v.status.as_str()
                );
            }
        }
        out
    }
}
