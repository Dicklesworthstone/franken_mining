//! DESIGN (BD-048/181, plan §10.2): explicit paired scoring of saved supplied-outcome cohorts.
//! Physical identity comes from complete metadata frozen before outcomes, never from equal values or
//! model names. These declarations and outcome citations are supplied, not independently verified.
//! Determinism: DeterministicPerIsa (inherited interval endpoints; exact rational finite losses).

use std::collections::{BTreeMap, BTreeSet};

use fmine_core::{Digest32, Encoder, blake3_hash};
use fmine_evidence::Refusal;
use fmine_numerics::exact_feas::Rat;

use super::super::binding::PublicResolutionPolicy;
use super::super::{
    Assessment, Cohort, ForecastLedger, ForecastPayload, MemberAssessment, OutcomeSelector,
};
use super::gaussian::{self, GaussianPairedMember};
use super::{
    PairStatus, PairedMember, PairedScore, SelectedPair, absent, domain, saved, score_member,
    select_pair,
};

pub const METHOD: &str = "Explicit supplied-target paired scoring, left minus right. Physical targets are the exact domain, site, horizon, quantity and unit labels frozen with each forecast; matched targets also require identical outcome deadlines. Each cohort must contain at most one forecast per complete target. Frozen resolution rule, eligible sources, quantity, unit, alpha, assessment cut and selector must agree. Public-bound and unclassified legacy cohorts refuse. Observed pairs require identical selected numeric values and complete citations; equality alone does not establish physical support. Missing forecasts, unresolved outcomes and source-selection mismatches remain in the union. The existing exact interval or finite-distribution CRPS owner computes scores and missing-outcome extrema. Whole-population completion bounds require a forecast and compatible selection on both sides for every target. These are descriptive arithmetic bounds, not confidence intervals, model-race significance, calibration or predictive skill. Target declarations and outcome citations remain supplied and unverified; no authenticated public-outcome claim is implied. Model families, authors, registration times and information sets may differ.";

pub const GAUSSIAN_METHOD: &str = "Explicit supplied-target paired Gaussian full-distribution CRPS, left minus right. Negative differences favor the left forecast. Complete frozen domain, site, horizon, quantity and unit labels identify each target; matched targets require identical outcome deadlines. Resolution rules, eligible sources, alpha, assessment cut and selector must agree. Every forecast, including unmatched targets, must be Gaussian. Observed pairs require identical selected values and complete citations. The union retains unresolved, unmatched and source-mismatched targets. Missing-outcome extrema use both infinite-tail limits and the unique normal-CDF crossing, or monotonicity at equal scales. The existing Gaussian owner computes approximate values and extrema, not exact rational results or certified numerical enclosures. Whole-population completion requires both forecasts and compatible selections for every target. These are descriptive arithmetic diagnostics, not confidence intervals, sequential significance, calibration or predictive skill. Target declarations and citations remain supplied and unverified; model families, authors, registration times and information sets may differ. Saved assessments and durable journal bytes remain unchanged.";

/// Complete prospective target labels, kept as separate fields to avoid ambiguous concatenation.
/// A declared horizon is not parsed into a calendar period or converted to another physical support.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct SuppliedTarget {
    pub domain: String,
    pub site: String,
    pub horizon: String,
    pub quantity: String,
    pub unit: String,
}

impl SuppliedTarget {
    fn encode(&self, enc: &mut Encoder) {
        for field in [
            &self.domain,
            &self.site,
            &self.horizon,
            &self.quantity,
            &self.unit,
        ] {
            enc.put_str(field);
        }
    }

    fn label(&self) -> String {
        format!("{} / {} / {}", self.site, self.domain, self.horizon)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SuppliedPairedMember {
    pub target: SuppliedTarget,
    pub outcome_due_ns: i64,
    pub pair: PairedMember,
}

/// A distinct read-only receipt; no existing public comparison or durable journal bytes change.
#[derive(Debug, Clone, PartialEq)]
pub struct SuppliedPairedAssessment {
    pub score: PairedScore,
    pub left_assessment: Digest32,
    pub right_assessment: Digest32,
    pub left_cohort: Digest32,
    pub right_cohort: Digest32,
    pub resolution_rule: String,
    pub eligible_sources: Vec<String>,
    pub quantity: String,
    pub unit: String,
    pub alpha: f64,
    pub as_of_ns: i64,
    pub selector: OutcomeSelector,
    pub members: Vec<SuppliedPairedMember>,
    pub n_paired: usize,
    pub n_unresolved: usize,
    pub n_unmatched: usize,
    pub n_arithmetic_indeterminate: usize,
    pub observed_mean_difference: Option<Rat>,
    pub whole_population: Option<(Rat, Rat)>,
    pub digest: Digest32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SuppliedGaussianMember {
    pub target: SuppliedTarget,
    pub outcome_due_ns: i64,
    pub pair: GaussianPairedMember,
}

/// Approximate Gaussian results remain separate from the exact finite-distribution receipt.
#[derive(Debug, Clone, PartialEq)]
pub struct SuppliedGaussianAssessment {
    pub left_assessment: Digest32,
    pub right_assessment: Digest32,
    pub left_cohort: Digest32,
    pub right_cohort: Digest32,
    pub resolution_rule: String,
    pub eligible_sources: Vec<String>,
    pub quantity: String,
    pub unit: String,
    pub alpha: f64,
    pub as_of_ns: i64,
    pub selector: OutcomeSelector,
    pub members: Vec<SuppliedGaussianMember>,
    pub n_paired: usize,
    pub n_unresolved: usize,
    pub n_unmatched: usize,
    pub n_arithmetic_indeterminate: usize,
    pub observed_mean_difference: Option<f64>,
    pub whole_population: Option<(f64, f64)>,
    pub digest: Digest32,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SuppliedCrpsComparison {
    Finite(SuppliedPairedAssessment),
    Gaussian(SuppliedGaussianAssessment),
}

struct SuppliedSelection<'a> {
    target: SuppliedTarget,
    outcome_due_ns: i64,
    pair: SelectedPair<'a>,
}

struct SuppliedInputs<'a> {
    left: &'a Assessment,
    right: &'a Assessment,
    cohort: &'a Cohort,
    quantity: String,
    unit: String,
    members: Vec<SuppliedSelection<'a>>,
}

fn cohort<'a>(ledger: &'a ForecastLedger, assessment: &Assessment) -> Result<&'a Cohort, Refusal> {
    let cohort = ledger
        .cohorts
        .get(&assessment.cohort)
        .ok_or_else(|| absent("supplied comparison cohort is unavailable"))?;
    if PublicResolutionPolicy::parse_resolution_rule_with_vintages(&cohort.spec.resolution_rule)?
        .is_some()
    {
        return Err(domain(
            "supplied-target comparison refuses public-bound cohorts; use public comparison",
        ));
    }
    Ok(cohort)
}

fn targets<'a>(
    ledger: &ForecastLedger,
    assessment: &'a Assessment,
) -> Result<BTreeMap<SuppliedTarget, (&'a MemberAssessment, i64)>, Refusal> {
    let mut targets = BTreeMap::new();
    for member in &assessment.per_member {
        let forecast = ledger
            .records
            .get(&member.forecast_id)
            .ok_or_else(|| absent("supplied comparison forecast is unavailable"))?;
        let metadata = forecast.metadata.as_ref().ok_or_else(|| {
            domain("supplied comparison needs complete prospectively frozen target metadata")
        })?;
        let target = SuppliedTarget {
            domain: metadata.domain.clone(),
            site: metadata.site.clone(),
            horizon: metadata.horizon.clone(),
            quantity: metadata.quantity.clone(),
            unit: metadata.unit.clone(),
        };
        if targets
            .insert(target, (member, forecast.freeze_time_ns))
            .is_some()
        {
            return Err(domain(
                "supplied comparison requires one forecast per complete physical target in each cohort",
            ));
        }
    }
    Ok(targets)
}

fn paired_inputs(
    ledger: &ForecastLedger,
    left_digest: Digest32,
    right_digest: Digest32,
) -> Result<SuppliedInputs<'_>, Refusal> {
    let (left, right) = (saved(ledger, left_digest)?, saved(ledger, right_digest)?);
    let (lc, rc) = (cohort(ledger, left)?, cohort(ledger, right)?);
    if left.as_of_ns != right.as_of_ns || left.selector != right.selector {
        return Err(domain(
            "supplied comparisons require the same assessment cut and outcome selector",
        ));
    }
    if lc.spec.resolution_rule != rc.spec.resolution_rule
        || lc.spec.eligible_sources != rc.spec.eligible_sources
        || lc.spec.policy.alpha.to_bits() != rc.spec.policy.alpha.to_bits()
    {
        return Err(domain(
            "supplied comparisons require identical frozen resolution rules, eligible sources and alpha",
        ));
    }
    let (lt, rt) = (targets(ledger, left)?, targets(ledger, right)?);
    let first = lt
        .keys()
        .next()
        .ok_or_else(|| absent("supplied comparison has no frozen targets"))?;
    if lt
        .keys()
        .chain(rt.keys())
        .any(|target| target.quantity != first.quantity || target.unit != first.unit)
    {
        return Err(domain(
            "supplied comparisons require identical frozen quantities and units",
        ));
    }
    let names: BTreeSet<_> = lt.keys().chain(rt.keys()).cloned().collect();
    let mut members = Vec::with_capacity(names.len());
    for target in names {
        let l = lt.get(&target).copied();
        let r = rt.get(&target).copied();
        if let (Some((_, a)), Some((_, b))) = (l, r)
            && a != b
        {
            return Err(domain(
                "matched supplied targets require identical frozen outcome deadlines",
            ));
        }
        let outcome_due_ns = l
            .or(r)
            .ok_or_else(|| absent("supplied target has no forecast"))?
            .1;
        let pair = select_pair(target.label(), l.map(|x| x.0), r.map(|x| x.0));
        members.push(SuppliedSelection {
            target,
            outcome_due_ns,
            pair,
        });
    }
    Ok(SuppliedInputs {
        left,
        right,
        cohort: lc,
        quantity: first.quantity.clone(),
        unit: first.unit.clone(),
        members,
    })
}

impl ForecastLedger {
    /// Explicitly compare two existing supplied-outcome assessments. This is never a fallback from
    /// public admission. Only frozen metadata determines the complete matching population.
    /// CRPS requires finite empirical distributions on every member, including unmatched forecasts.
    /// Brier requires Bernoulli events throughout and uses their original probabilities without a floor.
    pub fn compare_supplied_assessments(
        &self,
        left_digest: Digest32,
        right_digest: Digest32,
        score: PairedScore,
    ) -> Result<SuppliedPairedAssessment, Refusal> {
        let SuppliedInputs {
            left,
            right,
            cohort: lc,
            quantity,
            unit,
            members: selected,
        } = paired_inputs(self, left_digest, right_digest)?;
        let mut members = Vec::with_capacity(selected.len());
        for SuppliedSelection {
            target,
            outcome_due_ns,
            pair,
        } in selected
        {
            let pair = score_member(self, pair, lc.spec.policy.alpha, score)?;
            members.push(SuppliedPairedMember {
                target,
                outcome_due_ns,
                pair,
            });
        }
        let n_paired = members
            .iter()
            .filter(|m| m.pair.status == PairStatus::Paired)
            .count();
        let n_unresolved = members
            .iter()
            .filter(|m| m.pair.status == PairStatus::Unresolved)
            .count();
        let observed_mean_difference = if n_paired == 0 {
            None
        } else {
            members
                .iter()
                .filter(|m| m.pair.status == PairStatus::Paired)
                .try_fold(Rat::from_integer(0), |sum, member| {
                    sum.checked_add(member.pair.difference?).ok()
                })
                .and_then(|sum| sum.checked_div(Rat::from_integer(n_paired as i128)).ok())
        };
        let whole_population = members
            .iter()
            .try_fold(
                (Rat::from_integer(0), Rat::from_integer(0)),
                |(lo, hi), member| {
                    let bounds = member.pair.completion?;
                    Some((
                        lo.checked_add(bounds.0).ok()?,
                        hi.checked_add(bounds.1).ok()?,
                    ))
                },
            )
            .and_then(|(lo, hi)| {
                let n = Rat::from_integer(members.len() as i128);
                Some((lo.checked_div(n).ok()?, hi.checked_div(n).ok()?))
            });
        let mut result = SuppliedPairedAssessment {
            score,
            left_assessment: left.digest,
            right_assessment: right.digest,
            left_cohort: left.cohort,
            right_cohort: right.cohort,
            resolution_rule: lc.spec.resolution_rule.clone(),
            eligible_sources: lc.spec.eligible_sources.clone(),
            quantity,
            unit,
            alpha: lc.spec.policy.alpha,
            as_of_ns: left.as_of_ns,
            selector: left.selector,
            n_paired,
            n_unresolved,
            n_unmatched: members.len() - n_paired - n_unresolved,
            n_arithmetic_indeterminate: members
                .iter()
                .filter(|m| m.pair.arithmetic_indeterminate)
                .count(),
            members,
            observed_mean_difference,
            whole_population,
            digest: Digest32::default(),
        };
        result.digest = blake3_hash(&result.canonical_bytes());
        Ok(result)
    }

    /// Dispatch homogeneous complete distributions. Each selected owner validates every member,
    /// including unmatched forecasts; Gaussian/finite mixtures never become a selected subset.
    pub fn compare_supplied_assessments_full_crps(
        &self,
        left_digest: Digest32,
        right_digest: Digest32,
    ) -> Result<SuppliedCrpsComparison, Refusal> {
        let first = saved(self, left_digest)?
            .per_member
            .first()
            .and_then(|member| self.records.get(&member.forecast_id))
            .ok_or_else(|| absent("supplied comparison has no frozen forecast"))?;
        if matches!(first.payload, ForecastPayload::Gaussian { .. }) {
            self.compare_supplied_assessments_gaussian_crps(left_digest, right_digest)
                .map(SuppliedCrpsComparison::Gaussian)
        } else {
            self.compare_supplied_assessments(left_digest, right_digest, PairedScore::Crps)
                .map(SuppliedCrpsComparison::Finite)
        }
    }

    /// Approximate Gaussian CRPS on the same prospectively frozen supplied-target population as the
    /// exact finite comparison. No public source authority is inferred from supplied citations.
    pub fn compare_supplied_assessments_gaussian_crps(
        &self,
        left_digest: Digest32,
        right_digest: Digest32,
    ) -> Result<SuppliedGaussianAssessment, Refusal> {
        let SuppliedInputs {
            left,
            right,
            cohort,
            quantity,
            unit,
            members: selected,
        } = paired_inputs(self, left_digest, right_digest)?;
        let mut members = Vec::with_capacity(selected.len());
        for SuppliedSelection {
            target,
            outcome_due_ns,
            pair,
        } in selected
        {
            members.push(SuppliedGaussianMember {
                target,
                outcome_due_ns,
                pair: gaussian::score_member(self, pair)?,
            });
        }
        let n_paired = members
            .iter()
            .filter(|m| m.pair.status == PairStatus::Paired)
            .count();
        let n_unresolved = members
            .iter()
            .filter(|m| m.pair.status == PairStatus::Unresolved)
            .count();
        let observed_mean_difference = gaussian::mean(
            members
                .iter()
                .filter(|m| m.pair.status == PairStatus::Paired)
                .map(|m| m.pair.difference),
        );
        let whole_population =
            gaussian::mean(members.iter().map(|m| m.pair.completion.map(|b| b.0))).zip(
                gaussian::mean(members.iter().map(|m| m.pair.completion.map(|b| b.1))),
            );
        let mut result = SuppliedGaussianAssessment {
            left_assessment: left.digest,
            right_assessment: right.digest,
            left_cohort: left.cohort,
            right_cohort: right.cohort,
            resolution_rule: cohort.spec.resolution_rule.clone(),
            eligible_sources: cohort.spec.eligible_sources.clone(),
            quantity,
            unit,
            alpha: cohort.spec.policy.alpha,
            as_of_ns: left.as_of_ns,
            selector: left.selector,
            n_paired,
            n_unresolved,
            n_unmatched: members.len() - n_paired - n_unresolved,
            n_arithmetic_indeterminate: members
                .iter()
                .filter(|m| m.pair.arithmetic_indeterminate)
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

impl SuppliedGaussianAssessment {
    fn canonical_bytes(&self) -> Vec<u8> {
        fn number(enc: &mut Encoder, value: Option<f64>) {
            enc.put_u8(u8::from(value.is_some()));
            if let Some(value) = value {
                enc.put_u64(value.to_bits());
            }
        }
        let mut enc = Encoder::new_frame(*b"SGCR", 1);
        enc.put_fixed(&self.left_assessment.0);
        enc.put_fixed(&self.right_assessment.0);
        enc.put_str(GAUSSIAN_METHOD);
        enc.put_u64(self.members.len() as u64);
        for member in &self.members {
            member.target.encode(&mut enc);
            enc.put_i64(member.outcome_due_ns);
            let pair = &member.pair;
            enc.put_str(pair.status.as_str());
            enc.put_u8(u8::from(pair.arithmetic_indeterminate));
            number(&mut enc, pair.difference);
            number(&mut enc, pair.completion.map(|b| b.0));
            number(&mut enc, pair.completion.map(|b| b.1));
        }
        number(&mut enc, self.observed_mean_difference);
        number(&mut enc, self.whole_population.map(|b| b.0));
        number(&mut enc, self.whole_population.map(|b| b.1));
        enc.finish()
    }
}

impl SuppliedPairedAssessment {
    /// Method text is part of the separate receipt; existing interval and CRPS bytes are unchanged.
    pub const fn method(&self) -> &'static str {
        match self.score {
            PairedScore::Brier => super::BRIER_METHOD,
            PairedScore::Interval | PairedScore::Crps => METHOD,
        }
    }

    fn canonical_bytes(&self) -> Vec<u8> {
        fn rat(enc: &mut Encoder, value: Option<Rat>) {
            enc.put_u8(u8::from(value.is_some()));
            if let Some(value) = value {
                enc.put_i128(value.num());
                enc.put_i128(value.den());
            }
        }
        let tag = match self.score {
            PairedScore::Interval => *b"SINT",
            PairedScore::Crps => *b"SCRP",
            PairedScore::Brier => *b"SBRI",
        };
        let mut enc = Encoder::new_frame(tag, 1);
        enc.put_fixed(&self.left_assessment.0);
        enc.put_fixed(&self.right_assessment.0);
        enc.put_str(self.method());
        enc.put_u64(self.members.len() as u64);
        for member in &self.members {
            member.target.encode(&mut enc);
            enc.put_i64(member.outcome_due_ns);
            let pair = &member.pair;
            enc.put_str(pair.status.as_str());
            enc.put_u8(u8::from(pair.arithmetic_indeterminate));
            for interval in [pair.left_interval, pair.right_interval] {
                enc.put_u8(u8::from(interval.is_some()));
                if let Some((lo, hi)) = interval {
                    enc.put_u64(lo.to_bits());
                    enc.put_u64(hi.to_bits());
                }
            }
            rat(&mut enc, pair.difference);
            rat(&mut enc, pair.completion.map(|b| b.0));
            rat(&mut enc, pair.completion.map(|b| b.1));
        }
        rat(&mut enc, self.observed_mean_difference);
        rat(&mut enc, self.whole_population.map(|b| b.0));
        rat(&mut enc, self.whole_population.map(|b| b.1));
        enc.finish()
    }
}
