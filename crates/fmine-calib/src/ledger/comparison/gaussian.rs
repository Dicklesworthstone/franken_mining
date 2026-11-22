//! DESIGN (BD-048/049, §10.2): Gaussian CRPS differences and missing-outcome global extrema.
//! Determinism: DeterministicPerIsa (fixed reductions and the existing deterministic transcendental owner).
//!
//! For D(y) = CRPS(L,y) - CRPS(R,y), D'(y) = 2(F_L(y)-F_R(y)). Equal scales give a
//! monotone difference. Otherwise the CDFs cross once, at the common standardized residual
//! z = (mu_R-mu_L)/(sigma_L-sigma_R), where D = (sigma_L-sigma_R) CRPS(N(0,1),z).
//! The two infinite-tail limits and this stationary value are the complete extremum candidates.
//! These floating-point evaluations are approximate analytic extrema, not certified enclosures.

use fmine_core::{Digest32, Encoder, blake3_hash, det};
use fmine_evidence::Refusal;

use super::super::{ForecastLedger, ForecastPayload, MemberAssessment, OutcomeSelector};
use super::{PairStatus, PairedInputs, SelectedPair, absent, domain, paired_inputs};

pub const METHOD: &str = "Paired Gaussian full-distribution CRPS, left minus right, at identical frozen public targets, ownership, vintage permissions, alpha, assessment cut and outcome selector. Negative differences favor the left forecast. Identical selected values and complete citations are required for observed pairs; the union retains unresolved, unmatched and source-mismatched subjects. Missing-outcome extrema use both infinite-tail limits and the unique normal-CDF crossing (or monotonicity at equal scales), without a declared outcome domain. Gaussian values and extrema are floating-point approximations, not exact rational results or certified numerical enclosures. They are arithmetic completion diagnostics, not confidence intervals, sequential significance, calibration or predictive skill. Registration times and information sets may differ.";

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GaussianParameters {
    pub mean: f64,
    pub std_dev: f64,
}

impl GaussianParameters {
    fn from_payload(payload: &ForecastPayload) -> Result<Self, Refusal> {
        match payload {
            ForecastPayload::Gaussian { mean, std_dev } => Ok(Self {
                mean: *mean,
                std_dev: *std_dev,
            }),
            _ => Err(domain(
                "Gaussian paired CRPS requires Gaussian forecasts for every subject, including unmatched members",
            )),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct GaussianPairedMember {
    pub subject: String,
    pub left: Option<MemberAssessment>,
    pub right: Option<MemberAssessment>,
    pub left_parameters: Option<GaussianParameters>,
    pub right_parameters: Option<GaussianParameters>,
    pub status: PairStatus,
    pub difference: Option<f64>,
    pub completion: Option<(f64, f64)>,
    /// A required finite result was not representable. The member remains in every population count.
    pub arithmetic_indeterminate: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GaussianPairedAssessment {
    pub left_assessment: Digest32,
    pub right_assessment: Digest32,
    pub left_cohort: Digest32,
    pub right_cohort: Digest32,
    pub issuer: String,
    pub metric: String,
    pub unit: String,
    pub ownership: String,
    pub period: String,
    pub alpha: f64,
    pub as_of_ns: i64,
    pub selector: OutcomeSelector,
    pub members: Vec<GaussianPairedMember>,
    pub n_paired: usize,
    pub n_unresolved: usize,
    pub n_unmatched: usize,
    pub n_arithmetic_indeterminate: usize,
    pub observed_mean_difference: Option<f64>,
    pub whole_population: Option<(f64, f64)>,
    pub digest: Digest32,
}

fn finite(value: f64) -> Option<f64> {
    value
        .is_finite()
        .then_some(if value == 0.0 { 0.0 } else { value })
}

fn inv_sqrt_pi() -> f64 {
    1.0 / det::sqrt(std::f64::consts::PI)
}

fn residual(mean: f64, scale: f64, outcome: f64) -> f64 {
    let gap = outcome - mean;
    if gap.is_finite() {
        (gap / scale).abs()
    } else {
        (outcome / scale - mean / scale).abs()
    }
}

// 2 sigma [phi(t) - t Phi(-t)], the nonnegative correction to |y-mu|-sigma/sqrt(pi).
// Directly subtracting two large CRPS values loses a finite forecast difference in remote tails.
fn tail_correction(scale: f64, t: f64) -> f64 {
    if t >= 55.0 {
        // Even sigma=f64::MAX cannot lift this tail above the smallest representable subnormal.
        return 0.0;
    }
    let normalizer = det::sqrt(2.0 * std::f64::consts::PI);
    if t < 4.0 {
        return scale * (2.0 * (det::exp(-0.5 * t * t) / normalizer - t * det::norm_cdf(-t)));
    }
    // Mills' continued fraction Q(t)/phi(t)=1/(t+1/(t+2/(t+...))). Writing
    // 1-t*Q/phi as 1/(t*tail+1) avoids cancellation and a saturated CDF. For t>=4,
    // 80 terms converge beyond double precision; the result remains explicitly approximate.
    let mut tail = t;
    for k in (2..=80).rev() {
        tail = t + f64::from(k) / tail;
    }
    let exponent = -0.5 * t * t;
    let scaled_density = if t < 37.0 {
        scale * det::exp(exponent)
    } else {
        // Preserve a tiny density multiplied by a large scale, without first underflowing exp.
        det::exp(det::ln(scale) + exponent)
    };
    scaled_density * ((2.0 / normalizer) / (t * tail + 1.0))
}

fn absolute_error_difference(left: f64, right: f64, outcome: f64) -> Option<f64> {
    if outcome <= left.min(right) {
        return finite(left - right);
    }
    if outcome >= left.max(right) {
        return finite(right - left);
    }
    // The outcome lies between the means. The two signed residuals have opposite signs,
    // so adding them preserves cancellation without subtracting two absolute errors.
    let (a, b) = (outcome - left, outcome - right);
    let sum = if a.is_finite() && b.is_finite() {
        a + b
    } else {
        2.0 * ((0.5 * outcome - 0.5 * left) + (0.5 * outcome - 0.5 * right))
    };
    finite(if left < right { sum } else { -sum })
}

fn difference(left: GaussianParameters, right: GaussianParameters, outcome: f64) -> Option<f64> {
    if left == right {
        return Some(0.0);
    }
    let absolute = absolute_error_difference(left.mean, right.mean, outcome)?;
    let spread = left.std_dev - right.std_dev;
    let tails = tail_correction(left.std_dev, residual(left.mean, left.std_dev, outcome))
        - tail_correction(right.std_dev, residual(right.mean, right.std_dev, outcome));
    finite((-spread).mul_add(inv_sqrt_pi(), absolute) + tails)
}

fn extrema(left: GaussianParameters, right: GaussianParameters) -> Option<(f64, f64)> {
    let means = finite(left.mean - right.mean)?;
    let scales = left.std_dev - right.std_dev;
    let low_tail = finite((-scales).mul_add(inv_sqrt_pi(), means))?;
    let high_tail = finite((-scales).mul_add(inv_sqrt_pi(), -means))?;
    let (mut lo, mut hi) = (low_tail.min(high_tail), low_tail.max(high_tail));
    if scales != 0.0 {
        let t = (means / scales).abs();
        // At the crossing D=scales*CRPS(N(0,1),t). Never form the physical crossing
        // or multiply an overflowing t by a tiny scale. The absolute-error part is +/-|mean gap|.
        let base = means.abs().copysign(scales);
        let correction = tail_correction(scales.abs(), t).copysign(scales);
        let at_crossing = finite((-scales).mul_add(inv_sqrt_pi(), base) + correction)?;
        lo = lo.min(at_crossing);
        hi = hi.max(at_crossing);
    }
    Some((lo, hi))
}

fn scores(
    left: GaussianParameters,
    right: GaussianParameters,
    outcome: Option<f64>,
) -> Option<(f64, f64)> {
    match outcome {
        Some(outcome) => difference(left, right, outcome).map(|value| (value, value)),
        None => extrema(left, right),
    }
}

fn mean(values: impl Iterator<Item = Option<f64>>) -> Option<f64> {
    let values = values.collect::<Option<Vec<_>>>()?;
    if values.is_empty() {
        return None;
    }
    let scale = values.iter().fold(0.0_f64, |s, x| s.max(x.abs()));
    if scale == 0.0 {
        return Some(0.0);
    }
    // Scale before summing so a representable average does not fail on an overflowing sum.
    let (mut sum, mut correction) = (0.0_f64, 0.0_f64);
    for value in &values {
        let next = *value / scale - correction;
        let total = sum + next;
        correction = (total - sum) - next;
        sum = total;
    }
    finite((sum / values.len() as f64) * scale)
}

impl ForecastLedger {
    /// Approximate Gaussian CRPS differences with analytic global completion extrema. Existing exact
    /// interval and finite-distribution comparison identities and all saved parents remain unchanged.
    pub fn compare_assessments_gaussian_crps(
        &self,
        left_digest: Digest32,
        right_digest: Digest32,
    ) -> Result<GaussianPairedAssessment, Refusal> {
        let PairedInputs {
            left,
            right,
            policy,
            alpha,
            members: selected,
        } = paired_inputs(self, left_digest, right_digest)?;
        let mut members = Vec::with_capacity(selected.len());
        for SelectedPair {
            subject,
            left: l,
            right: r,
            status,
            value,
        } in selected
        {
            let parameters =
                |member: Option<&MemberAssessment>| -> Result<Option<GaussianParameters>, Refusal> {
                    member
                        .map(|member| {
                            let forecast = self
                                .records
                                .get(&member.forecast_id)
                                .ok_or_else(|| absent("paired Gaussian forecast is unavailable"))?;
                            GaussianParameters::from_payload(&forecast.payload)
                        })
                        .transpose()
                };
            let lp = parameters(l)?;
            let rp = parameters(r)?;
            let eligible = matches!(status, PairStatus::Paired | PairStatus::Unresolved);
            let completion = eligible.then(|| scores(lp?, rp?, value)).flatten();
            members.push(GaussianPairedMember {
                subject,
                left: l.cloned(),
                right: r.cloned(),
                left_parameters: lp,
                right_parameters: rp,
                status,
                difference: (status == PairStatus::Paired)
                    .then(|| completion.map(|b| b.0))
                    .flatten(),
                completion,
                arithmetic_indeterminate: eligible && completion.is_none(),
            });
        }
        let n_paired = members
            .iter()
            .filter(|m| m.status == PairStatus::Paired)
            .count();
        let n_unresolved = members
            .iter()
            .filter(|m| m.status == PairStatus::Unresolved)
            .count();
        let observed_mean_difference = mean(
            members
                .iter()
                .filter(|m| m.status == PairStatus::Paired)
                .map(|m| m.difference),
        );
        let whole_population = mean(members.iter().map(|m| m.completion.map(|b| b.0)))
            .zip(mean(members.iter().map(|m| m.completion.map(|b| b.1))));
        let mut report = GaussianPairedAssessment {
            left_assessment: left.digest,
            right_assessment: right.digest,
            left_cohort: left.cohort,
            right_cohort: right.cohort,
            issuer: policy.issuer,
            metric: policy.metric.as_str().into(),
            unit: policy.metric.unit().into(),
            ownership: policy.ownership.label(),
            period: policy.period.label(),
            alpha,
            as_of_ns: left.as_of_ns,
            selector: left.selector,
            n_paired,
            n_unresolved,
            n_unmatched: members.len() - n_paired - n_unresolved,
            n_arithmetic_indeterminate: members
                .iter()
                .filter(|m| m.arithmetic_indeterminate)
                .count(),
            members,
            observed_mean_difference,
            whole_population,
            digest: Digest32::default(),
        };
        report.digest = blake3_hash(&report.canonical_bytes());
        Ok(report)
    }
}

impl GaussianPairedAssessment {
    fn canonical_bytes(&self) -> Vec<u8> {
        fn number(enc: &mut Encoder, value: Option<f64>) {
            enc.put_u8(u8::from(value.is_some()));
            if let Some(value) = value {
                enc.put_u64(value.to_bits());
            }
        }
        let mut enc = Encoder::new_frame(*b"GCRP", 1);
        enc.put_fixed(&self.left_assessment.0);
        enc.put_fixed(&self.right_assessment.0);
        enc.put_str(METHOD);
        enc.put_u64(self.members.len() as u64);
        for member in &self.members {
            enc.put_str(&member.subject);
            enc.put_str(member.status.as_str());
            enc.put_u8(u8::from(member.arithmetic_indeterminate));
            number(&mut enc, member.difference);
            number(&mut enc, member.completion.map(|b| b.0));
            number(&mut enc, member.completion.map(|b| b.1));
        }
        number(&mut enc, self.observed_mean_difference);
        number(&mut enc, self.whole_population.map(|b| b.0));
        number(&mut enc, self.whole_population.map(|b| b.1));
        enc.finish()
    }
}

#[cfg(test)]
mod tests;
