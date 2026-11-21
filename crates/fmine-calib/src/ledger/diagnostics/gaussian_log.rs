//! DESIGN (BD-048/049): Gaussian log-density losses from an immutable saved assessment.
//! Determinism: DeterministicPerIsa
//! Uses the existing Gaussian scoring kernel.

use std::fmt::Write as _;

use fmine_core::{Digest32, Encoder, blake3_hash};
use fmine_evidence::Refusal;

use super::super::{
    ForecastLedger, ForecastPayload, MemberAssessment, MemberOutcome, OutcomeSelector, ScoreBounds,
    absent, floats, optional_float,
};
use crate::gaussian_log_score;

pub const METHOD: &str = "Gaussian negative log predictive density: ln(sigma) + ln(2*pi)/2 + (y-mu)^2/(2*sigma^2), in nats relative to the frozen target unit. Lower is better; negative density losses are valid. Changing the target unit by a positive factor adds its logarithm to the loss, so different units must not be compared. Only Gaussian members enter the explicitly named supported population; every other distribution remains visible. Missing outcomes retain their denominator and use the prospectively frozen outcome domain: the minimum is at the mean or nearest domain endpoint and the maximum at an endpoint. Without a domain the minimum is at the mean and the upper bound is unbounded. These are floating-point arithmetic completion bounds, not certified outward enclosures or confidence intervals. Unrepresentable arithmetic stays explicit and invalidates affected averages without dropping members. No calibration, independence or predictive-skill claim is supplied.";

#[derive(Debug, Clone, PartialEq)]
pub struct GaussianLogMember {
    pub member: MemberAssessment,
    pub distribution: &'static str,
    pub parameters: Option<(f64, f64)>,
    pub loss: Option<f64>,
    /// Absent for unsupported distributions or an unrepresentable loss/bound.
    pub completion: Option<ScoreBounds>,
}

impl GaussianLogMember {
    pub fn status(&self) -> &'static str {
        if self.parameters.is_none() {
            "unsupported_distribution"
        } else if self.completion.is_none() {
            "arithmetic_indeterminate"
        } else if self.loss.is_some() {
            "resolved"
        } else {
            "unresolved"
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct GaussianLogDiagnostics {
    pub assessment_digest: Digest32,
    pub cohort: Digest32,
    pub as_of_ns: i64,
    pub selector: OutcomeSelector,
    pub outcome_bounds: Option<(f64, f64)>,
    pub members: Vec<GaussianLogMember>,
    pub n_supported: usize,
    pub n_resolved: usize,
    pub n_scored: usize,
    pub n_arithmetic_indeterminate: usize,
    pub observed_mean: Option<f64>,
    pub whole_supported: Option<ScoreBounds>,
    pub digest: Digest32,
}

fn finite(value: f64) -> Option<f64> {
    value.is_finite().then_some(value)
}

fn missing_bounds(mean: f64, std_dev: f64, domain: Option<(f64, f64)>) -> Option<ScoreBounds> {
    let (minimum_at, upper) = if let Some((lo, hi)) = domain {
        let left = gaussian_log_score(mean, std_dev, lo).ok()?;
        let right = gaussian_log_score(mean, std_dev, hi).ok()?;
        (mean.max(lo).min(hi), Some(left.max(right)))
    } else {
        (mean, None)
    };
    let lower = gaussian_log_score(mean, std_dev, minimum_at).ok()?;
    if upper.is_some_and(|upper| upper < lower) {
        return None;
    }
    Some(ScoreBounds { lower, upper })
}

impl ForecastLedger {
    /// Read-only GLS1/v1 projection; prior forecast, cohort, assessment and journal bytes are unchanged.
    pub fn gaussian_log_diagnostics(
        &self,
        digest: Digest32,
    ) -> Result<GaussianLogDiagnostics, Refusal> {
        let assessment = self
            .assessments
            .values()
            .find(|a| a.digest == digest)
            .ok_or_else(|| {
                absent("Gaussian log scores require an existing immutable assessment")
            })?;
        let outcome_bounds = self
            .cohorts
            .get(&assessment.cohort)
            .ok_or_else(|| absent("Gaussian log scores require the frozen parent cohort"))?
            .spec
            .policy
            .outcome_bounds;
        let mut members = Vec::with_capacity(assessment.per_member.len());
        for member in &assessment.per_member {
            let payload = &self
                .records
                .get(&member.forecast_id)
                .ok_or_else(|| absent("Gaussian log member's frozen forecast is unavailable"))?
                .payload;
            let (distribution, parameters) = match *payload {
                ForecastPayload::Gaussian { mean, std_dev } => ("gaussian", Some((mean, std_dev))),
                ForecastPayload::Interval { .. } => ("interval", None),
                ForecastPayload::Quantiles { .. } => ("quantiles", None),
                ForecastPayload::Bernoulli { .. } => ("bernoulli", None),
                ForecastPayload::Empirical { .. } => ("empirical", None),
                ForecastPayload::WeightedEmpirical { .. } => ("weighted_empirical", None),
            };
            let (loss, completion) = if let Some((mean, std_dev)) = parameters {
                match member.outcome {
                    MemberOutcome::Resolved { value, .. } => {
                        let loss = gaussian_log_score(mean, std_dev, value).ok();
                        (
                            loss,
                            loss.map(|loss| ScoreBounds {
                                lower: loss,
                                upper: Some(loss),
                            }),
                        )
                    }
                    MemberOutcome::Unresolved { .. } => {
                        (None, missing_bounds(mean, std_dev, outcome_bounds))
                    }
                }
            } else {
                (None, None)
            };
            members.push(GaussianLogMember {
                member: member.clone(),
                distribution,
                parameters,
                loss,
                completion,
            });
        }
        let supported: Vec<_> = members.iter().filter(|m| m.parameters.is_some()).collect();
        let resolved: Vec<_> = supported
            .iter()
            .copied()
            .filter(|m| matches!(m.member.outcome, MemberOutcome::Resolved { .. }))
            .collect();
        let observed_mean = if resolved.is_empty() {
            None
        } else {
            resolved
                .iter()
                .try_fold(0.0, |sum, m| finite(sum + m.loss?))
                .and_then(|sum| finite(sum / resolved.len() as f64))
        };
        let whole_supported = if supported.is_empty() {
            None
        } else {
            supported
                .iter()
                .try_fold(
                    ScoreBounds {
                        lower: 0.0,
                        upper: Some(0.0),
                    },
                    |sum, m| {
                        let b = m.completion?;
                        Some(ScoreBounds {
                            lower: finite(sum.lower + b.lower)?,
                            upper: match (sum.upper, b.upper) {
                                (Some(left), Some(right)) => Some(finite(left + right)?),
                                _ => None,
                            },
                        })
                    },
                )
                .map(|sum| ScoreBounds {
                    lower: sum.lower / supported.len() as f64,
                    upper: sum.upper.map(|upper| upper / supported.len() as f64),
                })
        };
        let mut report = GaussianLogDiagnostics {
            assessment_digest: digest,
            cohort: assessment.cohort,
            as_of_ns: assessment.as_of_ns,
            selector: assessment.selector,
            outcome_bounds,
            n_supported: supported.len(),
            n_resolved: resolved.len(),
            n_scored: supported.iter().filter(|m| m.loss.is_some()).count(),
            n_arithmetic_indeterminate: supported.iter().filter(|m| m.completion.is_none()).count(),
            members,
            observed_mean,
            whole_supported,
            digest: Digest32::default(),
        };
        report.digest = blake3_hash(&report.canonical_bytes());
        Ok(report)
    }
}

fn encode_bounds(enc: &mut Encoder, bounds: Option<ScoreBounds>) {
    enc.put_u8(u8::from(bounds.is_some()));
    if let Some(bounds) = bounds {
        floats(enc, &[bounds.lower]);
        optional_float(enc, bounds.upper);
    }
}

fn number(value: Option<f64>) -> String {
    value.map_or_else(|| "null".into(), |v| v.to_string())
}

fn bounds_json(bounds: Option<ScoreBounds>) -> String {
    bounds.map_or_else(
        || "null".into(),
        |b| {
            format!(
                "{{\"lower\":{},\"upper\":{},\"upper_unbounded\":{}}}",
                b.lower,
                number(b.upper),
                b.upper.is_none()
            )
        },
    )
}

fn bounds_text(bounds: Option<ScoreBounds>) -> String {
    bounds.map_or_else(
        || "unavailable".into(),
        |b| {
            format!(
                "[{}, {}]",
                b.lower,
                b.upper
                    .map_or_else(|| "unbounded".into(), |v| v.to_string())
            )
        },
    )
}

impl GaussianLogDiagnostics {
    pub fn observed_status(&self) -> &'static str {
        if self.n_resolved == 0 {
            "no_resolved_outcomes"
        } else if self.observed_mean.is_some() {
            "available"
        } else {
            "arithmetic_indeterminate"
        }
    }

    pub fn completion_status(&self) -> &'static str {
        if self.n_supported == 0 {
            "no_supported_members"
        } else if self.whole_supported.is_some() {
            "available"
        } else {
            "arithmetic_indeterminate"
        }
    }

    pub fn canonical_bytes(&self) -> Vec<u8> {
        let mut enc = Encoder::new_frame(*b"GLS1", 1);
        enc.put_str(METHOD);
        enc.put_fixed(&self.assessment_digest.0);
        enc.put_fixed(&self.cohort.0);
        enc.put_i64(self.as_of_ns);
        enc.put_u8(u8::from(self.selector == OutcomeSelector::LatestAsOf));
        enc.put_u8(u8::from(self.outcome_bounds.is_some()));
        if let Some((lo, hi)) = self.outcome_bounds {
            floats(&mut enc, &[lo, hi]);
        }
        for count in [
            self.n_supported,
            self.n_resolved,
            self.n_scored,
            self.n_arithmetic_indeterminate,
        ] {
            enc.put_u64(count as u64);
        }
        optional_float(&mut enc, self.observed_mean);
        encode_bounds(&mut enc, self.whole_supported);
        enc.put_u64(self.members.len() as u64);
        for member in &self.members {
            member.member.encode(&mut enc);
            enc.put_str(member.distribution);
            enc.put_u8(u8::from(member.parameters.is_some()));
            if let Some((mean, std_dev)) = member.parameters {
                floats(&mut enc, &[mean, std_dev]);
            }
            optional_float(&mut enc, member.loss);
            encode_bounds(&mut enc, member.completion);
        }
        enc.finish()
    }

    pub fn to_json(&self) -> String {
        let members = self.members.iter().map(|m| {
            let event = m.member.selected_event.map_or_else(|| "null".into(), |(seq, digest)| format!("{{\"seq\":{seq},\"digest\":\"{digest}\"}}"));
            let parameters = m.parameters.map_or_else(|| "null".into(), |(mean, std_dev)| format!("{{\"mean\":{mean},\"std_dev\":{std_dev}}}"));
            format!("{{\"forecast_id\":\"{}\",\"selected_event\":{event},\"distribution\":\"{}\",\"parameters\":{parameters},\"status\":\"{}\",\"loss\":{},\"completion\":{}}}",
                m.member.forecast_id, m.distribution, m.status(), number(m.loss), bounds_json(m.completion))
        }).collect::<Vec<_>>().join(",");
        let selector = match self.selector {
            OutcomeSelector::FirstPublished => "first-published",
            OutcomeSelector::LatestAsOf => "latest-as-of",
        };
        let domain = self
            .outcome_bounds
            .map_or_else(|| "null".into(), |(lo, hi)| format!("[{lo},{hi}]"));
        format!(
            "{{\"schema\":\"fmine.forecast-gaussian-log.v1\",\"assessment_digest\":\"{}\",\"cohort\":\"{}\",\"digest\":\"{}\",\"as_of_ns\":{},\"selector\":\"{selector}\",\"method\":\"{METHOD}\",\"outcome_bounds\":{domain},\"n_members\":{},\"n_supported\":{},\"n_unsupported\":{},\"n_resolved\":{},\"n_unresolved\":{},\"n_scored\":{},\"n_arithmetic_indeterminate\":{},\"observed_status\":\"{}\",\"observed_mean\":{},\"completion_status\":\"{}\",\"whole_supported\":{},\"members\":[{members}]}}",
            self.assessment_digest,
            self.cohort,
            self.digest,
            self.as_of_ns,
            self.members.len(),
            self.n_supported,
            self.members.len() - self.n_supported,
            self.n_resolved,
            self.n_supported - self.n_resolved,
            self.n_scored,
            self.n_arithmetic_indeterminate,
            self.observed_status(),
            number(self.observed_mean),
            self.completion_status(),
            bounds_json(self.whole_supported)
        )
    }

    pub fn to_markdown(&self) -> String {
        let mut out = format!(
            "\n### Gaussian log-density losses\n\n{METHOD}\n\nParent assessment: `{}`. Gaussian log report: `{}`.\n\nSupported Gaussian members: {}; unsupported distributions: {}; resolved: {}; unresolved: {}; scored: {}; arithmetic indeterminate: {}.\n\nObserved mean: {} ({}). Whole-supported completion: {} ({}).\n\n| Forecast | Distribution | Mean | Standard deviation | Status | Log-density loss | Completion bounds |\n|---|---|---:|---:|---|---:|---|\n",
            self.assessment_digest,
            self.digest,
            self.n_supported,
            self.members.len() - self.n_supported,
            self.n_resolved,
            self.n_supported - self.n_resolved,
            self.n_scored,
            self.n_arithmetic_indeterminate,
            self.observed_mean
                .map_or_else(|| "unavailable".into(), |v| v.to_string()),
            self.observed_status(),
            bounds_text(self.whole_supported),
            self.completion_status()
        );
        for m in &self.members {
            let _ = writeln!(
                out,
                "| `{}` | {} | {} | {} | {} | {} | {} |",
                m.member.forecast_id,
                m.distribution,
                number(m.parameters.map(|p| p.0)),
                number(m.parameters.map(|p| p.1)),
                m.status(),
                number(m.loss),
                bounds_text(m.completion)
            );
        }
        out
    }
}
