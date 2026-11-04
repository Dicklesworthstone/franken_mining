//! Frozen conditional coverage evidence for the forecast ledger (BD-051, plan §10.4).
//! Determinism: DeterministicPerIsa (fixed observation order and the numerical log-mixture kernel).
//!
//! DESIGN: the declared null is `P(hit_t | all prior observations) = 1 - alpha` at the
//! declared support, not marginal coverage or coverage of at least that probability. The
//! declaration is an assumption, not a verification by this kernel. Neither an effective
//! sample size nor a correlation sensitivity range establishes the conditional null.

use std::collections::BTreeMap;

use fmine_core::codec::Encoder;
use fmine_core::id::ObjectId;
use fmine_evidence::Refusal;
use fmine_numerics::eproc::coverage::{
    BernoulliCoverageProcess, CoverageError, CoverageSnapshot, MAX_OBSERVATIONS,
};

use super::{ForecastPayload, FrozenForecast, MemberAssessment, MemberOutcome, domain, floats};

/// A prospective monitoring declaration, frozen as part of the cohort before outcomes.
#[derive(Debug, Clone, PartialEq)]
pub struct CoverageMonitorPolicy {
    /// Anytime alarm probability under the declared conditional point null; distinct from interval alpha.
    pub alarm_alpha: f64,
    /// Physical/statistical support, for example non-overlapping monthly reconciled domain totals.
    pub support: String,
    /// Why exact conditional Bernoulli coverage is assumed at this support and frozen order.
    pub conditional_null: String,
    /// Complete predeclared observation sequence; must permute cohort members exactly.
    pub order: Vec<ObjectId>,
}

impl CoverageMonitorPolicy {
    pub(super) fn validate(&self, interval_alpha: f64) -> Result<(), Refusal> {
        if self.support.trim().is_empty() || self.conditional_null.trim().is_empty() {
            return Err(domain(
                "coverage monitoring needs a nonempty support and conditional-null assumption declaration",
            ));
        }
        if self.order.is_empty() || self.order.len() as u64 > MAX_OBSERVATIONS {
            return Err(domain(
                "coverage monitoring needs a nonempty bounded frozen observation order",
            ));
        }
        BernoulliCoverageProcess::new(1.0 - interval_alpha, self.alarm_alpha)
            .map_err(numerical_refusal)?;
        Ok(())
    }

    pub(super) fn validate_members(&self, members: &[ObjectId]) -> Result<(), Refusal> {
        let mut ordered_members = self.order.clone();
        ordered_members.sort();
        if ordered_members != members {
            return Err(domain(
                "coverage observation order must contain every frozen cohort member exactly once",
            ));
        }
        Ok(())
    }

    pub(super) fn encode(&self, enc: &mut Encoder) {
        floats(enc, &[self.alarm_alpha]);
        enc.put_str(&self.support);
        enc.put_str(&self.conditional_null);
        enc.put_u64(self.order.len() as u64);
        for member in &self.order {
            enc.put_fixed(&member.bytes);
        }
    }
}

/// One original publication consumed in the frozen observation order.
#[derive(Debug, Clone, PartialEq)]
pub struct CoverageObservation {
    /// Retains the original value, citation, event sequence and event digest for independent provenance checks.
    pub member: MemberAssessment,
    /// Frozen forecast's inclusive central interval in the forecast's own units.
    pub interval: (f64, f64),
    pub hit: bool,
}

/// Evidence at the contiguous resolved prefix. An alarm diagnoses the declared null; it does not recalibrate.
#[derive(Debug, Clone, PartialEq)]
pub struct CoverageMonitorAssessment {
    /// Retained beside the evidence so a renderer need not reconstruct the assumption from prose elsewhere.
    pub policy: CoverageMonitorPolicy,
    pub null_probability: f64,
    pub observations: Vec<CoverageObservation>,
    /// The first unresolved member, including an explicit missing-event hash when one exists.
    pub blocked_by: Option<MemberAssessment>,
    /// Members after the blocker, whether resolved or unresolved; the blocker itself is excluded.
    pub n_deferred: usize,
    /// Already resolved original publications after the blocker, deliberately not consumed.
    pub n_deferred_resolved: usize,
    /// Current and running maximum log evidence and the first crossing's one-based observation index.
    pub snapshot: CoverageSnapshot,
}

impl CoverageMonitorAssessment {
    pub(super) fn encode(&self, enc: &mut Encoder) {
        self.policy.encode(enc);
        floats(enc, &[self.null_probability]);
        enc.put_u64(self.observations.len() as u64);
        for observation in &self.observations {
            observation.member.encode(enc);
            floats(enc, &[observation.interval.0, observation.interval.1]);
            enc.put_u8(u8::from(observation.hit));
        }
        enc.put_u8(u8::from(self.blocked_by.is_some()));
        if let Some(blocker) = &self.blocked_by {
            blocker.encode(enc);
        }
        enc.put_u64(self.n_deferred as u64);
        enc.put_u64(self.n_deferred_resolved as u64);
        let state = self.snapshot;
        enc.put_u64(state.n);
        enc.put_u64(state.hits);
        floats(
            enc,
            &[
                state.log_e_value,
                state.max_log_e_value,
                state.log_threshold,
            ],
        );
        enc.put_u8(u8::from(state.first_crossing.is_some()));
        if let Some(index) = state.first_crossing {
            enc.put_u64(index);
        }
    }
}

/// DESIGN: only probability-bearing continuous central intervals have this exact point-null meaning.
/// Empirical/discrete interval inclusion need not have mass `1-alpha`; no hidden randomization is added.
pub(super) fn monitor_interval(
    payload: &ForecastPayload,
    alpha: f64,
) -> Result<(f64, f64), Refusal> {
    match payload {
        ForecastPayload::Gaussian { .. } => {}
        ForecastPayload::Interval { nominal, .. } => {
            if *nominal != Some(1.0 - alpha) {
                return Err(domain(
                    "coverage monitoring requires an explicit interval probability exactly matching 1-alpha",
                ));
            }
        }
        ForecastPayload::Quantiles { levels, values } => {
            let at = |level| {
                levels
                    .iter()
                    .position(|p| *p == level)
                    .and_then(|i| values.get(i).copied())
                    .ok_or_else(|| domain(
                        "coverage monitoring requires exact central quantile levels alpha/2 and 1-alpha/2",
                    ))
            };
            // DESIGN: the descriptive scorer accepts a tolerance; exact monitoring must not accidentally
            // select a neighboring approximate level before the exact central level.
            let pair = (at(alpha / 2.0)?, at(1.0 - alpha / 2.0)?);
            return if pair.0.is_finite() && pair.1.is_finite() && (pair.1 - pair.0).is_finite() {
                Ok(pair)
            } else {
                Err(domain("coverage monitor interval is not finite"))
            };
        }
        ForecastPayload::Bernoulli { .. } | ForecastPayload::Empirical { .. } => {
            return Err(domain(
                "coverage monitoring's exact conditional point null does not support discrete or empirical forecasts",
            ));
        }
    }
    payload
        .interval(alpha)
        .map_err(super::score_refusal)?
        .ok_or_else(|| {
            domain("coverage monitoring requires a probability-bearing central interval")
        })
}

pub(super) fn assess(
    policy: &CoverageMonitorPolicy,
    alpha: f64,
    records: &[&FrozenForecast],
    originals: &[MemberAssessment],
) -> Result<CoverageMonitorAssessment, Refusal> {
    policy.validate(alpha)?;
    let members: Vec<ObjectId> = records.iter().map(|r| r.forecast_id).collect();
    policy.validate_members(&members)?;
    let forecasts: BTreeMap<_, _> = records.iter().map(|r| (r.forecast_id, *r)).collect();
    let outcomes: BTreeMap<_, _> = originals.iter().map(|m| (m.forecast_id, m)).collect();
    let mut process = BernoulliCoverageProcess::new(1.0 - alpha, policy.alarm_alpha)
        .map_err(numerical_refusal)?;
    let mut observations = Vec::new();
    let mut blocked_by = None;
    let mut n_deferred = 0;
    let mut n_deferred_resolved = 0;
    for id in &policy.order {
        let original = outcomes
            .get(id)
            .ok_or_else(|| domain("coverage monitor original member unavailable"))?;
        let record = forecasts
            .get(id)
            .ok_or_else(|| domain("coverage monitor frozen forecast unavailable"))?;
        let interval = monitor_interval(&record.payload, alpha)?;
        if blocked_by.is_some() {
            n_deferred += 1;
            n_deferred_resolved +=
                usize::from(matches!(original.outcome, MemberOutcome::Resolved { .. }));
            continue;
        }
        match original.outcome {
            MemberOutcome::Resolved { value, .. } => {
                let hit = value >= interval.0 && value <= interval.1;
                process.observe(hit).map_err(numerical_refusal)?;
                observations.push(CoverageObservation {
                    member: (*original).clone(),
                    interval,
                    hit,
                });
            }
            MemberOutcome::Unresolved { .. } => blocked_by = Some((*original).clone()),
        }
    }
    Ok(CoverageMonitorAssessment {
        policy: policy.clone(),
        null_probability: 1.0 - alpha,
        observations,
        blocked_by,
        n_deferred,
        n_deferred_resolved,
        snapshot: process.snapshot(),
    })
}

fn numerical_refusal(error: CoverageError) -> Refusal {
    domain(&format!("coverage monitor refused: {error}"))
}

#[cfg(test)]
mod tests;
