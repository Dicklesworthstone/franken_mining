//! DESIGN (BD-049, plan §10.3): an explicit, unissued Gaussian correction proposal from a saved
//! assessment. Historical fitting never edits a forecast or confers calibration. The complete cohort
//! and exact model/metadata key are required; no favorable subset is selected.
//! Determinism: DeterministicPerIsa (fixed member order, `det` logarithm/exponential, canonical bits).

use fmine_core::{Digest32, Encoder, ObjectId, blake3_hash, det};
use fmine_evidence::Refusal;

use super::{
    ForecastLedger, ForecastMetadata, ForecastPayload, FrozenForecast, MemberAssessment,
    MemberOutcome, OutcomeSelector, absent, domain, floats, late, score_refusal,
};

mod series;
pub use series::{
    CorrectionSeriesProposal, CorrectionTrainingAssessment, SERIES_METHOD, SERIES_NO_CLAIM,
};

pub const METHOD: &str = "pooled-log-bias-gaussian-inflation-v1";
pub const NO_CLAIM: &str = "Unissued correction proposal fitted on one complete saved assessment, not a registered forecast, hierarchical fit or calibration result. The same model and full declared segment are required. All selected historical outcomes remain visible. Fitted coverage reuses training outcomes and is not out-of-sample evidence. No original forecast, outcome, assessment or journal record is changed; separate prospective registration and later scoring are required.";

/// One unchanged forecast and the particular outcome already selected by the immutable assessment.
#[derive(Debug, Clone, PartialEq)]
pub struct CorrectionMember {
    pub raw: FrozenForecast,
    pub selected: MemberAssessment,
    pub log_ratio: f64,
    pub corrected_interval: (f64, f64),
    pub raw_inside: bool,
    pub corrected_inside: bool,
}

/// A dimensioned predictive distribution stays attached to its complete frozen target and source cohort.
/// The factor and inflation are dimensionless; log dispersion describes the fitted ratios only.
#[derive(Debug, Clone, PartialEq)]
pub struct CorrectionProposal {
    pub assessment_digest: Digest32,
    pub cohort_digest: Digest32,
    pub training_as_of_ns: i64,
    pub selector: OutcomeSelector,
    pub alpha: f64,
    pub metadata: ForecastMetadata,
    pub members: Vec<CorrectionMember>,
    pub mean_log_ratio: f64,
    pub sample_log_std_dev: f64,
    pub bias_factor: f64,
    pub inflation: f64,
    pub required_training_hits: usize,
    pub raw_training_hits: usize,
    pub corrected_training_hits: usize,
    pub raw: FrozenForecast,
    pub raw_interval: (f64, f64),
    pub corrected: ForecastPayload,
    pub corrected_interval: (f64, f64),
    pub digest: Digest32,
}

struct FittedCorrection {
    members: Vec<CorrectionMember>,
    mean_log_ratio: f64,
    sample_log_std_dev: f64,
    bias_factor: f64,
    inflation: f64,
    required_training_hits: usize,
    raw_training_hits: usize,
    corrected_training_hits: usize,
    raw_interval: (f64, f64),
    corrected: ForecastPayload,
    corrected_interval: (f64, f64),
}

fn numeric(value: f64) -> Result<f64, Refusal> {
    if value.is_finite() {
        Ok(value)
    } else {
        Err(domain("correction arithmetic is not representable"))
    }
}

fn positive(value: f64) -> Result<f64, Refusal> {
    let value = numeric(value)?;
    if value > 0.0 {
        Ok(value)
    } else {
        Err(domain(
            "multiplicative correction requires strictly positive representable quantities",
        ))
    }
}

fn gaussian(payload: &ForecastPayload) -> Result<(f64, f64), Refusal> {
    let ForecastPayload::Gaussian { mean, std_dev } = payload else {
        return Err(domain(
            "correction requires Gaussian forecasts throughout the complete cohort",
        ));
    };
    Ok((positive(*mean)?, positive(*std_dev)?))
}

fn interval(payload: &ForecastPayload, alpha: f64) -> Result<(f64, f64), Refusal> {
    let bounds = payload
        .interval(alpha)
        .map_err(score_refusal)?
        .ok_or_else(|| domain("correction requires a continuous predictive interval"))?;
    if bounds.0 >= bounds.1 {
        return Err(domain(
            "correction Gaussian interval is not representable at its nonzero spread",
        ));
    }
    Ok(bounds)
}

fn corrected(
    raw: &ForecastPayload,
    factor: f64,
    inflation: f64,
) -> Result<ForecastPayload, Refusal> {
    let (mean, std_dev) = gaussian(raw)?;
    let result = ForecastPayload::Gaussian {
        mean: positive(mean * factor)?,
        std_dev: positive(positive(std_dev * factor)? * inflation)?,
    };
    result.validate()?;
    Ok(result)
}

fn inside(bounds: (f64, f64), value: f64) -> bool {
    bounds.0 <= value && value <= bounds.1
}

impl ForecastLedger {
    /// Fit an explicitly uncalibrated correction to an already saved complete assessment and show its
    /// effect on a later, unresolved raw forecast. Does not select outcomes again or advance any clock.
    /// The saved cohort's alpha is the sole fitted level; incompatible target-cohort levels refuse.
    pub fn correction_proposal(
        &self,
        assessment_digest: Digest32,
        raw_forecast_id: ObjectId,
    ) -> Result<CorrectionProposal, Refusal> {
        let assessment = self
            .assessments
            .values()
            .find(|report| report.digest == assessment_digest)
            .ok_or_else(|| absent("correction requires an existing immutable assessment"))?;
        let cohort = self
            .cohorts
            .get(&assessment.cohort)
            .ok_or_else(|| absent("correction training cohort is unavailable"))?;
        let raw = self
            .records
            .get(&raw_forecast_id)
            .ok_or_else(|| absent("correction target forecast is not registered"))?;
        if raw.registered_ns <= assessment.as_of_ns || raw.freeze_time_ns <= raw.registered_ns {
            return Err(late(
                "correction target must have been registered strictly after the saved training cut and before its outcome",
            ));
        }
        if self
            .outcomes
            .get(&raw_forecast_id)
            .is_some_and(|events| !events.is_empty())
        {
            return Err(late(
                "a correction proposal cannot target a forecast with any recorded outcome event",
            ));
        }
        let metadata = raw
            .metadata
            .as_ref()
            .ok_or_else(|| domain("correction requires prospectively declared target metadata"))?;
        gaussian(&raw.payload)?;
        let alpha = cohort.spec.policy.alpha;
        if self.cohorts.values().any(|target| {
            target.spec.members.contains(&raw_forecast_id)
                && target.spec.policy.alpha.to_bits() != alpha.to_bits()
        }) {
            return Err(domain(
                "correction target cohorts must use the training assessment's exact alpha",
            ));
        }
        let n = assessment.per_member.len();
        if n < 2 || assessment.summary.n_unresolved != 0 {
            return Err(domain(
                "correction needs at least two resolved members and no unresolved member in the complete frozen cohort",
            ));
        }
        let fit = self.fit_correction(raw, metadata, alpha, &assessment.per_member)?;
        let mut report = CorrectionProposal {
            assessment_digest,
            cohort_digest: cohort.digest,
            training_as_of_ns: assessment.as_of_ns,
            selector: assessment.selector,
            alpha,
            metadata: metadata.clone(),
            members: fit.members,
            mean_log_ratio: fit.mean_log_ratio,
            sample_log_std_dev: fit.sample_log_std_dev,
            bias_factor: fit.bias_factor,
            inflation: fit.inflation,
            required_training_hits: fit.required_training_hits,
            raw_training_hits: fit.raw_training_hits,
            corrected_training_hits: fit.corrected_training_hits,
            raw: raw.clone(),
            raw_interval: fit.raw_interval,
            corrected: fit.corrected,
            corrected_interval: fit.corrected_interval,
            digest: Digest32([0; 32]),
        };
        report.digest = digest(&report);
        Ok(report)
    }

    // Both proposal paths fit the same complete population in its declared canonical order. Keeping
    // the arithmetic here preserves the original single-assessment calculation and COR1 bytes.
    fn fit_correction(
        &self,
        raw: &FrozenForecast,
        metadata: &ForecastMetadata,
        alpha: f64,
        selected_members: &[MemberAssessment],
    ) -> Result<FittedCorrection, Refusal> {
        let n = selected_members.len();
        if n < 2 {
            return Err(domain(
                "correction needs at least two resolved training members",
            ));
        }
        let mut members = Vec::with_capacity(n);
        let mut mean_log_ratio = 0.0;
        for selected in selected_members {
            let forecast = self
                .records
                .get(&selected.forecast_id)
                .ok_or_else(|| absent("correction training forecast is unavailable"))?;
            if forecast.model_key != raw.model_key || forecast.metadata.as_ref() != Some(metadata) {
                return Err(domain(
                    "correction requires the same model, family, author, domain, horizon, site, quantity and unit for every training member and target",
                ));
            }
            if forecast.freeze_time_ns >= raw.registered_ns {
                return Err(late(
                    "correction training outcome boundaries must precede target registration",
                ));
            }
            let (mean, _) = gaussian(&forecast.payload)?;
            let MemberOutcome::Resolved { value, .. } = &selected.outcome else {
                return Err(domain(
                    "an unresolved member cannot be excluded from correction fitting",
                ));
            };
            let value = positive(*value)?;
            // DESIGN: logarithm subtraction avoids overflowing a representable pair's ratio.
            let log_ratio = numeric(det::ln(value) - det::ln(mean))?;
            mean_log_ratio = numeric(mean_log_ratio + log_ratio / n as f64)?;
            members.push(CorrectionMember {
                raw: forecast.clone(),
                selected: selected.clone(),
                log_ratio,
                corrected_interval: (0.0, 0.0),
                raw_inside: inside(interval(&forecast.payload, alpha)?, value),
                corrected_inside: false,
            });
        }
        let sample_log_variance = members.iter().try_fold(0.0, |sum, member| {
            let residual = member.log_ratio - mean_log_ratio;
            numeric(sum + residual * residual / (n - 1) as f64)
        })?;
        let sample_log_std_dev = numeric(sample_log_variance.sqrt())?;
        let bias_factor = positive(det::exp(mean_log_ratio))?;
        let unit_gaussian = ForecastPayload::Gaussian {
            mean: 0.0,
            std_dev: 1.0,
        };
        let z = positive(interval(&unit_gaussian, alpha)?.1)?;
        let mut required_inflations = Vec::with_capacity(n);
        for member in &members {
            let (mean, std_dev) = gaussian(&member.raw.payload)?;
            let MemberOutcome::Resolved { value, .. } = member.selected.outcome else {
                return Err(domain("correction member unexpectedly unresolved"));
            };
            let center = positive(mean * bias_factor)?;
            let radius = positive(positive(std_dev * bias_factor)? * z)?;
            required_inflations.push(numeric((value - center).abs() / radius)?);
        }
        required_inflations.sort_by(f64::total_cmp);
        // DESIGN: inverse empirical CDF at 1-alpha, with one complete training denominator. The
        // explicit floor forbids narrowing after bias rescaling; a factor below one can still reduce
        // absolute raw width. Upward rounding protects the selected endpoint.
        let required_training_hits = ((1.0 - alpha) * n as f64).ceil() as usize;
        let quantile = *required_inflations
            .get(required_training_hits.saturating_sub(1))
            .ok_or_else(|| domain("correction empirical coverage rank is invalid"))?;
        let inflation = if quantile > 1.0 {
            positive(quantile.next_up())?
        } else {
            1.0
        };
        for member in &mut members {
            member.corrected_interval = interval(
                &corrected(&member.raw.payload, bias_factor, inflation)?,
                alpha,
            )?;
            let MemberOutcome::Resolved { value, .. } = member.selected.outcome else {
                return Err(domain("correction member unexpectedly unresolved"));
            };
            member.corrected_inside = inside(member.corrected_interval, value);
        }
        let raw_training_hits = members.iter().filter(|member| member.raw_inside).count();
        let corrected_training_hits = members
            .iter()
            .filter(|member| member.corrected_inside)
            .count();
        if corrected_training_hits < required_training_hits {
            return Err(domain(
                "floating-point interval endpoints cannot certify the fitted training coverage; no rounded success is emitted",
            ));
        }
        let corrected = corrected(&raw.payload, bias_factor, inflation)?;
        Ok(FittedCorrection {
            members,
            mean_log_ratio,
            sample_log_std_dev,
            bias_factor,
            inflation,
            required_training_hits,
            raw_training_hits,
            corrected_training_hits,
            raw_interval: interval(&raw.payload, alpha)?,
            corrected_interval: interval(&corrected, alpha)?,
            corrected,
        })
    }
}

fn digest(report: &CorrectionProposal) -> Digest32 {
    let mut enc = Encoder::new_frame(*b"COR1", 1);
    enc.put_str(METHOD);
    enc.put_str(NO_CLAIM);
    enc.put_fixed(&report.assessment_digest.0);
    enc.put_fixed(&report.cohort_digest.0);
    enc.put_i64(report.training_as_of_ns);
    enc.put_u8(match report.selector {
        OutcomeSelector::FirstPublished => 0,
        OutcomeSelector::LatestAsOf => 1,
    });
    report.metadata.encode(&mut enc);
    report.raw.encode(&mut enc);
    report.corrected.encode(&mut enc);
    floats(
        &mut enc,
        &[
            report.alpha,
            report.mean_log_ratio,
            report.sample_log_std_dev,
            report.bias_factor,
            report.inflation,
            report.raw_interval.0,
            report.raw_interval.1,
            report.corrected_interval.0,
            report.corrected_interval.1,
        ],
    );
    for count in [
        report.required_training_hits,
        report.raw_training_hits,
        report.corrected_training_hits,
        report.members.len(),
    ] {
        enc.put_u64(count as u64);
    }
    for member in &report.members {
        member.raw.encode(&mut enc);
        floats(
            &mut enc,
            &[
                member.log_ratio,
                member.corrected_interval.0,
                member.corrected_interval.1,
            ],
        );
        enc.put_u8(u8::from(member.raw_inside));
        enc.put_u8(u8::from(member.corrected_inside));
        member.selected.encode(&mut enc);
    }
    blake3_hash(&enc.finish())
}

#[cfg(test)]
mod tests;
