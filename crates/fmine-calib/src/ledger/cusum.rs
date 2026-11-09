//! DESIGN (BD-051, plan §10.4): prospectively frozen Gaussian residual CUSUM diagnostics.
//! Determinism: DeterministicPerIsa (fixed forecast order and finite floating-point recurrences).
//! A chosen threshold has no inferred false-alarm probability or average run length. Tuning and
//! reference assumptions remain explicit; an alarm neither recalibrates nor opens an authority action.

use std::collections::BTreeMap;

use fmine_core::codec::Encoder;
use fmine_core::id::ObjectId;
use fmine_evidence::{Refusal, RefusalCode};
use fmine_numerics::eproc::cusum::{CusumError, CusumSnapshot, MAX_OBSERVATIONS, TwoSidedCusum};

use super::{ForecastPayload, FrozenForecast, MemberAssessment, MemberOutcome, domain, floats};

/// Explicit monitoring choices, frozen with the cohort before any member outcome.
#[derive(Debug, Clone, PartialEq)]
pub struct CusumMonitorPolicy {
    /// Positive reference allowance in standardized-residual units (`k`, often half the target shift).
    pub reference_value: f64,
    /// Positive threshold `h`; reaching it in either direction records a sticky alarm.
    pub threshold: f64,
    /// Physical/statistical support of one forecast residual, including any relevant aggregation.
    pub support: String,
    /// Declared reference law and dependence assumptions for the standardized residuals.
    pub reference_assumption: String,
    /// Declared reason for k/h; a statement here does not verify an ARL or false-alarm claim.
    pub tuning_basis: String,
    /// Complete prospective member order, never reordered using observed outcomes or availability.
    pub order: Vec<ObjectId>,
}

impl CusumMonitorPolicy {
    pub(super) fn validate(&self) -> Result<(), Refusal> {
        for (name, text) in [
            ("support", &self.support),
            ("reference assumption", &self.reference_assumption),
            ("tuning basis", &self.tuning_basis),
        ] {
            if text.trim().is_empty() || text.len() > 4096 || text.chars().any(char::is_control) {
                return Err(domain(&format!(
                    "CUSUM {name} must be nonempty, within 4096 bytes and without control characters"
                )));
            }
        }
        if self.order.is_empty() || self.order.len() as u64 > MAX_OBSERVATIONS {
            return Err(domain(
                "CUSUM needs a nonempty bounded frozen observation order",
            ));
        }
        TwoSidedCusum::new(self.reference_value, self.threshold).map_err(numerical_refusal)?;
        Ok(())
    }

    pub(super) fn validate_members(&self, members: &[ObjectId]) -> Result<(), Refusal> {
        let mut ordered = self.order.clone();
        ordered.sort();
        if ordered != members {
            return Err(domain(
                "CUSUM observation order must contain every frozen cohort member exactly once",
            ));
        }
        Ok(())
    }

    pub(super) fn encode(&self, enc: &mut Encoder) {
        floats(enc, &[self.reference_value, self.threshold]);
        enc.put_str(&self.support);
        enc.put_str(&self.reference_assumption);
        enc.put_str(&self.tuning_basis);
        enc.put_u64(self.order.len() as u64);
        for id in &self.order {
            enc.put_fixed(&id.bytes);
        }
    }
}

/// One original publication, its frozen reference distribution and the resulting directional evidence.
#[derive(Debug, Clone, PartialEq)]
pub struct CusumObservation {
    pub member: MemberAssessment,
    pub mean: f64,
    pub std_dev: f64,
    pub standardized_residual: f64,
    pub snapshot: CusumSnapshot,
}

/// Diagnostic at the contiguous resolved prefix, even when the surrounding assessment selects restatements.
#[derive(Debug, Clone, PartialEq)]
pub struct CusumMonitorAssessment {
    pub policy: CusumMonitorPolicy,
    pub observations: Vec<CusumObservation>,
    pub blocked_by: Option<MemberAssessment>,
    /// All members after the blocker, excluding the blocker itself.
    pub n_deferred: usize,
    /// Original publications already resolved after the blocker, deliberately not consumed.
    pub n_deferred_resolved: usize,
    pub snapshot: CusumSnapshot,
}

fn encode_snapshot(snapshot: CusumSnapshot, enc: &mut Encoder) {
    enc.put_u64(snapshot.n);
    floats(
        enc,
        &[
            snapshot.upper,
            snapshot.lower,
            snapshot.max_upper,
            snapshot.max_lower,
            snapshot.threshold,
        ],
    );
    for crossing in [snapshot.first_upper_crossing, snapshot.first_lower_crossing] {
        enc.put_u8(u8::from(crossing.is_some()));
        if let Some(n) = crossing {
            enc.put_u64(n);
        }
    }
}

impl CusumMonitorAssessment {
    pub(super) fn encode(&self, enc: &mut Encoder) {
        self.policy.encode(enc);
        enc.put_u64(self.observations.len() as u64);
        for observation in &self.observations {
            observation.member.encode(enc);
            floats(
                enc,
                &[
                    observation.mean,
                    observation.std_dev,
                    observation.standardized_residual,
                ],
            );
            encode_snapshot(observation.snapshot, enc);
        }
        enc.put_u8(u8::from(self.blocked_by.is_some()));
        if let Some(blocker) = &self.blocked_by {
            blocker.encode(enc);
        }
        enc.put_u64(self.n_deferred as u64);
        enc.put_u64(self.n_deferred_resolved as u64);
        encode_snapshot(self.snapshot, enc);
    }
}

pub(super) fn gaussian_reference(payload: &ForecastPayload) -> Result<(f64, f64), Refusal> {
    if let ForecastPayload::Gaussian { mean, std_dev } = payload
        && mean.is_finite()
        && std_dev.is_finite()
        && *std_dev > 0.0
    {
        Ok((*mean, *std_dev))
    } else {
        Err(domain(
            "residual CUSUM requires every frozen forecast to be Gaussian",
        ))
    }
}

pub(super) fn assess(
    policy: &CusumMonitorPolicy,
    records: &[&FrozenForecast],
    originals: &[MemberAssessment],
) -> Result<CusumMonitorAssessment, Refusal> {
    policy.validate()?;
    let members: Vec<_> = records.iter().map(|record| record.forecast_id).collect();
    policy.validate_members(&members)?;
    let forecasts: BTreeMap<_, _> = records
        .iter()
        .map(|record| (record.forecast_id, *record))
        .collect();
    let outcomes: BTreeMap<_, _> = originals
        .iter()
        .map(|member| (member.forecast_id, member))
        .collect();
    let mut process =
        TwoSidedCusum::new(policy.reference_value, policy.threshold).map_err(numerical_refusal)?;
    let mut observations = Vec::new();
    let mut blocked_by = None;
    let mut n_deferred = 0;
    let mut n_deferred_resolved = 0;
    for id in &policy.order {
        let record = forecasts
            .get(id)
            .ok_or_else(|| domain("CUSUM frozen forecast unavailable"))?;
        let original = outcomes
            .get(id)
            .ok_or_else(|| domain("CUSUM original member unavailable"))?;
        let (mean, std_dev) = gaussian_reference(&record.payload)?;
        if blocked_by.is_some() {
            n_deferred += 1;
            n_deferred_resolved +=
                usize::from(matches!(original.outcome, MemberOutcome::Resolved { .. }));
            continue;
        }
        match original.outcome {
            MemberOutcome::Resolved { value, .. } => {
                let standardized_residual = crate::standardized_residual(mean, std_dev, value);
                if !standardized_residual.is_finite() {
                    return Err(numerical_refusal(CusumError::NumericalFailure));
                }
                let snapshot = process
                    .observe(standardized_residual)
                    .map_err(numerical_refusal)?;
                observations.push(CusumObservation {
                    member: (*original).clone(),
                    mean,
                    std_dev,
                    standardized_residual,
                    snapshot,
                });
            }
            MemberOutcome::Unresolved { .. } => blocked_by = Some((*original).clone()),
        }
    }
    Ok(CusumMonitorAssessment {
        policy: policy.clone(),
        observations,
        blocked_by,
        n_deferred,
        n_deferred_resolved,
        snapshot: process.snapshot(),
    })
}

fn numerical_refusal(error: CusumError) -> Refusal {
    let what = format!("CUSUM monitor refused: {error}");
    if matches!(error, CusumError::NumericalFailure) {
        Refusal::new(RefusalCode::NumericalCertificateFailed207, what)
    } else {
        domain(&what)
    }
}

#[cfg(test)]
mod tests;
