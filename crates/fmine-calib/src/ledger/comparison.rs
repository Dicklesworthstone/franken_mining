//! DESIGN (BD-048/049, plan §10.2): paired interval losses on two immutable public assessments.
//! Exact arithmetic uses the frozen binary values. Matching requires the same physical target and
//! selected publication, never just equal amounts. Missing and unmatched subjects stay in the population.
//! Determinism: DeterministicPerIsa (inherited predictive interval endpoints; exact rational losses).

use std::collections::{BTreeMap, BTreeSet};

use fmine_core::{Digest32, Encoder, blake3_hash};
use fmine_evidence::Refusal;
use fmine_numerics::exact_feas::Rat;

use super::binding::{PublicResolutionPolicy, PublicVintagePolicy};
use super::diagnostics::quantiles::exact;
use super::{
    Assessment, ForecastLedger, MemberAssessment, MemberOutcome, OutcomeSelector, absent, domain,
};
use crate::record::{interval_score, scoring::interval_score_difference_bounds};

pub const METHOD: &str = "Paired central-interval loss, left minus right, at the identical frozen alpha. Negative values favor the left cohort. Subjects, target, unit, ownership, calendar period, outcome selector and assessment cut must agree; paired observations require the identical citation and numeric value. The union of frozen subjects remains visible. Observed means use only identical selected publications, with every excluded subject named. Whole-population bounds exist only when all subjects have both forecasts and compatible outcome selections; unresolved pairs use exact global loss-difference extrema at the four interval endpoints. These bounds are arithmetic completion bounds, not confidence intervals. Forecast registration times and information sets may differ. No model-race significance, calibration, independence or predictive-skill claim is supplied.";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PairStatus {
    Paired,
    Unresolved,
    LeftForecastMissing,
    RightForecastMissing,
    OutcomeSelectionMismatch,
}

impl PairStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Paired => "paired",
            Self::Unresolved => "unresolved",
            Self::LeftForecastMissing => "left_forecast_missing",
            Self::RightForecastMissing => "right_forecast_missing",
            Self::OutcomeSelectionMismatch => "outcome_selection_mismatch",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PairedMember {
    pub subject: String,
    pub left: Option<MemberAssessment>,
    pub right: Option<MemberAssessment>,
    pub left_interval: Option<(f64, f64)>,
    pub right_interval: Option<(f64, f64)>,
    pub status: PairStatus,
    pub difference: Option<Rat>,
    pub completion: Option<(Rat, Rat)>,
    /// Exact i128 capacity failure is explicit, never a rounded score or a dropped subject.
    pub arithmetic_indeterminate: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PairedAssessment {
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
    pub members: Vec<PairedMember>,
    pub n_paired: usize,
    pub n_unresolved: usize,
    pub n_unmatched: usize,
    pub n_arithmetic_indeterminate: usize,
    /// Absent for an empty paired set or any unrepresentable paired loss/sum/mean.
    pub observed_mean_difference: Option<Rat>,
    /// Every subject in the union contributes, including unresolved pairs. No subset bound is substituted.
    pub whole_population: Option<(Rat, Rat)>,
    pub digest: Digest32,
}

fn saved(ledger: &ForecastLedger, digest: Digest32) -> Result<&Assessment, Refusal> {
    ledger
        .assessments
        .values()
        .find(|assessment| assessment.digest == digest)
        .ok_or_else(|| absent("paired comparison requires two existing immutable assessments"))
}

fn policy(
    ledger: &ForecastLedger,
    assessment: &Assessment,
) -> Result<(PublicResolutionPolicy, PublicVintagePolicy, f64), Refusal> {
    let cohort = ledger
        .cohorts
        .get(&assessment.cohort)
        .ok_or_else(|| absent("paired comparison cohort is unavailable"))?;
    let (policy, vintages) = PublicResolutionPolicy::parse_resolution_rule_with_vintages(
        &cohort.spec.resolution_rule,
    )?
    .ok_or_else(|| {
        domain("paired comparison needs prospectively frozen public targets, not inferred labels")
    })?;
    Ok((policy, vintages, cohort.spec.policy.alpha))
}

fn subjects<'a>(
    assessment: &'a Assessment,
    policy: &PublicResolutionPolicy,
) -> Result<BTreeMap<String, &'a MemberAssessment>, Refusal> {
    assessment
        .per_member
        .iter()
        .map(|member| {
            policy
                .members
                .get(&member.forecast_id)
                .map(|subject| (subject.clone(), member))
                .ok_or_else(|| absent("paired comparison member lacks its frozen public subject"))
        })
        .collect()
}

fn bounds(ledger: &ForecastLedger, member: &MemberAssessment, alpha: f64) -> Option<(f64, f64)> {
    ledger
        .records
        .get(&member.forecast_id)?
        .payload
        .interval(alpha)
        .ok()?
}

fn difference(left: (Rat, Rat), right: (Rat, Rat), alpha: Rat, value: Rat) -> Option<Rat> {
    interval_score(left.0, left.1, alpha, value)
        .ok()?
        .checked_sub(interval_score(right.0, right.1, alpha, value).ok()?)
        .ok()
}

fn scores(
    left: Option<(f64, f64)>,
    right: Option<(f64, f64)>,
    alpha: f64,
    value: Option<f64>,
) -> Option<(Rat, Rat)> {
    let (left, right) = (left?, right?);
    let left = (exact(left.0).ok()?, exact(left.1).ok()?);
    let right = (exact(right.0).ok()?, exact(right.1).ok()?);
    let alpha = exact(alpha).ok()?;
    match value {
        Some(value) => {
            let score = difference(left, right, alpha, exact(value).ok()?)?;
            Some((score, score))
        }
        None => interval_score_difference_bounds(left, right, alpha, alpha).ok(),
    }
}

impl ForecastLedger {
    /// Compare already saved assessments without selecting outcomes again or advancing authority time.
    /// Same-target subjects are matched by their frozen source policy, not their model name or value.
    pub fn compare_assessments(
        &self,
        left_digest: Digest32,
        right_digest: Digest32,
    ) -> Result<PairedAssessment, Refusal> {
        let (left, right) = (saved(self, left_digest)?, saved(self, right_digest)?);
        let ((lp, lv, la), (rp, rv, ra)) = (policy(self, left)?, policy(self, right)?);
        if left.as_of_ns != right.as_of_ns || left.selector != right.selector {
            return Err(domain(
                "paired comparisons require the same assessment cut and outcome selector",
            ));
        }
        if lp.issuer != rp.issuer
            || lp.metric != rp.metric
            || lp.period != rp.period
            || lp.ownership != rp.ownership
            || lv != rv
            || la.to_bits() != ra.to_bits()
        {
            return Err(domain(
                "paired comparisons require identical frozen target, ownership, vintage permission and alpha",
            ));
        }
        let (ls, rs) = (subjects(left, &lp)?, subjects(right, &rp)?);
        let names: BTreeSet<_> = ls.keys().chain(rs.keys()).cloned().collect();
        let mut members = Vec::with_capacity(names.len());
        for subject in names {
            let l = ls.get(&subject).copied();
            let r = rs.get(&subject).copied();
            let (status, value) = match (l, r) {
                (None, _) => (PairStatus::LeftForecastMissing, None),
                (_, None) => (PairStatus::RightForecastMissing, None),
                (Some(l), Some(r)) => match (&l.outcome, &r.outcome) {
                    (
                        MemberOutcome::Resolved {
                            value: a,
                            citation: ac,
                            ..
                        },
                        MemberOutcome::Resolved {
                            value: b,
                            citation: bc,
                            ..
                        },
                    ) if a == b && ac == bc => (PairStatus::Paired, Some(*a)),
                    (MemberOutcome::Unresolved { .. }, MemberOutcome::Unresolved { .. }) => {
                        (PairStatus::Unresolved, None)
                    }
                    _ => (PairStatus::OutcomeSelectionMismatch, None),
                },
            };
            let left_interval = l.and_then(|member| bounds(self, member, la));
            let right_interval = r.and_then(|member| bounds(self, member, la));
            let eligible = matches!(status, PairStatus::Paired | PairStatus::Unresolved);
            let completion = eligible
                .then(|| scores(left_interval, right_interval, la, value))
                .flatten();
            members.push(PairedMember {
                subject,
                left: l.cloned(),
                right: r.cloned(),
                left_interval,
                right_interval,
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
        let observed_mean_difference = if n_paired == 0 {
            None
        } else {
            members
                .iter()
                .filter(|m| m.status == PairStatus::Paired)
                .try_fold(Rat::from_integer(0), |sum, member| {
                    sum.checked_add(member.difference?).ok()
                })
                .and_then(|sum| sum.checked_div(Rat::from_integer(n_paired as i128)).ok())
        };
        let whole_population = if members.is_empty() {
            None
        } else {
            members
                .iter()
                .try_fold(
                    (Rat::from_integer(0), Rat::from_integer(0)),
                    |(lo, hi), member| {
                        let bounds = member.completion?;
                        Some((
                            lo.checked_add(bounds.0).ok()?,
                            hi.checked_add(bounds.1).ok()?,
                        ))
                    },
                )
                .and_then(|(lo, hi)| {
                    let n = Rat::from_integer(members.len() as i128);
                    Some((lo.checked_div(n).ok()?, hi.checked_div(n).ok()?))
                })
        };
        let mut result = PairedAssessment {
            left_assessment: left.digest,
            right_assessment: right.digest,
            left_cohort: left.cohort,
            right_cohort: right.cohort,
            issuer: lp.issuer,
            metric: lp.metric.as_str().into(),
            unit: lp.metric.unit().into(),
            ownership: lp.ownership.label(),
            period: lp.period.label(),
            alpha: la,
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
        result.digest = blake3_hash(&result.canonical_bytes());
        Ok(result)
    }
}

impl PairedAssessment {
    fn canonical_bytes(&self) -> Vec<u8> {
        fn rat(enc: &mut Encoder, value: Option<Rat>) {
            enc.put_u8(u8::from(value.is_some()));
            if let Some(value) = value {
                enc.put_i128(value.num());
                enc.put_i128(value.den());
            }
        }
        // Each immutable parent already binds full membership, distributions, policies and citations.
        let mut enc = Encoder::new_frame(*b"PAIR", 1);
        enc.put_fixed(&self.left_assessment.0);
        enc.put_fixed(&self.right_assessment.0);
        enc.put_str(METHOD);
        enc.put_u64(self.members.len() as u64);
        for member in &self.members {
            enc.put_str(&member.subject);
            enc.put_str(member.status.as_str());
            enc.put_u8(u8::from(member.arithmetic_indeterminate));
            for interval in [member.left_interval, member.right_interval] {
                enc.put_u8(u8::from(interval.is_some()));
                if let Some((lo, hi)) = interval {
                    enc.put_u64(lo.to_bits());
                    enc.put_u64(hi.to_bits());
                }
            }
            rat(&mut enc, member.difference);
            rat(&mut enc, member.completion.map(|b| b.0));
            rat(&mut enc, member.completion.map(|b| b.1));
        }
        rat(&mut enc, self.observed_mean_difference);
        rat(&mut enc, self.whole_population.map(|b| b.0));
        rat(&mut enc, self.whole_population.map(|b| b.1));
        enc.finish()
    }
}
