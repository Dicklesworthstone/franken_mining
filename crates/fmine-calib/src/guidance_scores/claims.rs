//! Canonical Claim adapters for descriptive guidance scores (BD-341.S4).
//!
//! DESIGN: a printed guidance range has no stated probability level. Its normalized position is
//! descriptive, never a standardized residual or a calibration verdict. Printed-outcome hit counts
//! and rounding-aware completion bounds are separate. The binomial interval additionally assumes
//! independent, identically distributed hits; dependence, effective sample size and bias uncertainty
//! remain unestimated. The public adapter owns publication selection and source-byte authentication.
//! All score and summary arithmetic delegates to the parent module's existing numerical primitives.
//! Determinism: DeterministicPerIsa (exact scores; the IID binomial interval uses the parent kernel).

use std::collections::BTreeSet;

use fmine_claim::{AdditivityClass, Claim, Representation, SpatialSupport};
use fmine_core::{Encoder, blake3_hash};
use fmine_evidence::{EvidenceClass, KnowledgeState, Refusal, RefusalCode};
use fmine_numerics::exact_feas::{Interval, Rat};

pub use super::Side;
use super::{Guidance, GuidanceScoreError, PrintedOutcome, ScoringMember};
use crate::CalibrationFlag;

type Result<T> = std::result::Result<T, Refusal>;

/// An explicitly interpreted annual quantity. No unit or ownership conversion is implicit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SeriesKey {
    pub subject: String,
    pub metric: String,
    pub unit: String,
    pub basis: String,
    pub year: u16,
}

/// Exact issuer target; a point or one-sided bound has no normalized range position.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GuidanceTarget {
    Range { lo: Rat, hi: Rat },
    Point(Rat),
    AtMost(Rat),
    AtLeast(Rat),
}

impl From<GuidanceTarget> for Guidance {
    fn from(target: GuidanceTarget) -> Self {
        match target {
            GuidanceTarget::Range { lo, hi } => Self::Range { lo, hi },
            GuidanceTarget::Point(value) => Self::Point { value },
            GuidanceTarget::AtMost(upper) => Self::AtMost { upper },
            GuidanceTarget::AtLeast(lower) => Self::AtLeast { lower },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuidanceValue {
    pub key: SeriesKey,
    pub target: GuidanceTarget,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutcomeValue {
    pub key: SeriesKey,
    pub point: Rat,
    /// Printed rounding interval, including the declared tie convention's open ends.
    pub rounding: Interval,
}

fn put_rat(e: &mut Encoder, v: Rat) {
    e.put_fixed(&v.num().to_le_bytes());
    e.put_fixed(&v.den().to_le_bytes());
}

fn put_interval(e: &mut Encoder, i: Interval) {
    put_rat(e, i.lo);
    put_rat(e, i.hi);
    e.put_u8(u8::from(i.lo_strict));
    e.put_u8(u8::from(i.hi_strict));
}

fn put_key(e: &mut Encoder, key: &SeriesKey) {
    e.put_str(&key.subject);
    e.put_str(&key.metric);
    e.put_str(&key.unit);
    e.put_str(&key.basis);
    e.put_u16(key.year);
}

impl GuidanceValue {
    #[must_use]
    pub fn canonical_tag(&self) -> String {
        let mut e = Encoder::new_frame(*b"GDT1", 1);
        put_key(&mut e, &self.key);
        match self.target {
            GuidanceTarget::Range { lo, hi } => {
                e.put_u8(0);
                put_rat(&mut e, lo);
                put_rat(&mut e, hi);
            }
            GuidanceTarget::Point(v) | GuidanceTarget::AtMost(v) | GuidanceTarget::AtLeast(v) => {
                e.put_u8(match self.target {
                    GuidanceTarget::Point(_) => 1,
                    GuidanceTarget::AtMost(_) => 2,
                    _ => 3,
                });
                put_rat(&mut e, v);
            }
        }
        blake3_hash(&e.finish()).to_hex()
    }
}

impl OutcomeValue {
    #[must_use]
    pub fn canonical_tag(&self) -> String {
        let mut e = Encoder::new_frame(*b"GDO1", 1);
        put_key(&mut e, &self.key);
        put_rat(&mut e, self.point);
        put_interval(&mut e, self.rounding);
        blake3_hash(&e.finish()).to_hex()
    }
}

/// A verdict over the complete printed rounding interval, not just its displayed point.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hit {
    CertainHit,
    CertainMiss,
    Boundary,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RangeScore {
    pub midpoint: Option<Rat>,
    pub half_width: Option<Rat>,
    pub z: Option<Rat>,
    pub z_bounds: Option<Interval>,
    /// Classification of the printed point. This can be `Inside` while `hit` is `Boundary`.
    pub side: Side,
    pub boundary: bool,
    pub hit: Hit,
    pub above_midpoint: Option<bool>,
}

fn optional_rat(e: &mut Encoder, x: Option<Rat>) {
    e.put_u8(u8::from(x.is_some()));
    if let Some(x) = x {
        put_rat(e, x);
    }
}

fn optional_interval(e: &mut Encoder, x: Option<Interval>) {
    e.put_u8(u8::from(x.is_some()));
    if let Some(x) = x {
        put_interval(e, x);
    }
}

impl RangeScore {
    #[must_use]
    pub fn canonical_tag(&self) -> String {
        let mut e = Encoder::new_frame(*b"GDS1", 1);
        for x in [self.midpoint, self.half_width, self.z] {
            optional_rat(&mut e, x);
        }
        optional_interval(&mut e, self.z_bounds);
        e.put_u8(self.side as u8);
        e.put_u8(u8::from(self.boundary));
        e.put_u8(self.hit as u8);
        e.put_u8(match self.above_midpoint {
            None => 0,
            Some(false) => 1,
            Some(true) => 2,
        });
        blake3_hash(&e.finish()).to_hex()
    }
}

fn refuse(code: RefusalCode, detail: impl Into<String>) -> Refusal {
    Refusal::new(code, format!("guidance score: {}", detail.into()))
}

fn arithmetic_error(error: GuidanceScoreError) -> Refusal {
    let code = match error {
        GuidanceScoreError::InvalidRange
        | GuidanceScoreError::InvalidOutcomeInterval
        | GuidanceScoreError::PrintedOutcomeOutsideInterval => {
            RefusalCode::OutsidePhysicalDomain109
        }
        GuidanceScoreError::InvalidMemberId | GuidanceScoreError::DuplicateMemberId => {
            RefusalCode::BasisMismatch105
        }
        GuidanceScoreError::ArithmeticOverflow | GuidanceScoreError::ReferenceIntervalFailed => {
            RefusalCode::NumericalCertificateFailed207
        }
    };
    refuse(code, error.to_string())
}

fn ready<T>(c: &Claim<T>, tag: &str) -> Result<()> {
    c.check_id()?;
    if c.value_tag() != tag {
        return Err(refuse(
            RefusalCode::FrozenRecordMutation507,
            "quantity differs from its canonical Claim value",
        ));
    }
    if c.stale || c.state == KnowledgeState::Stale {
        return Err(refuse(RefusalCode::StaleAnchor502, "stale source Claim"));
    }
    if c.calibration_failed {
        return Err(refuse(
            RefusalCode::CalibrationRequired205,
            "failed source calibration",
        ));
    }
    if matches!(c.representation, Representation::Placeholder { .. })
        || !matches!(c.state, KnowledgeState::Known | KnowledgeState::Estimated)
    {
        return Err(refuse(
            RefusalCode::SourceCellUnusable208,
            "unusable source Claim",
        ));
    }
    if c.support != SpatialSupport::PeriodAggregate {
        return Err(refuse(
            RefusalCode::SupportMismatch102,
            "annual aggregate required",
        ));
    }
    if c.anchors.is_empty()
        || c.anchors.iter().any(|a| {
            a.artifact_path.trim().is_empty()
                || a.locator.trim().is_empty()
                || a.artifact_sha256 == [0; 32]
        })
    {
        return Err(refuse(
            RefusalCode::NoClaimInDomain201,
            "source digest and locator required",
        ));
    }
    Ok(())
}

fn validate_guidance(c: &Claim<GuidanceValue>) -> Result<()> {
    ready(c, &c.value.canonical_tag())?;
    let key = &c.value.key;
    if key.year == 0
        || key.year > 9999
        || [&key.subject, &key.metric, &key.unit, &key.basis]
            .iter()
            .any(|s| s.trim().is_empty())
    {
        return Err(refuse(
            RefusalCode::UndeclaredUnitBasisOrTz107,
            "incomplete quantity interpretation",
        ));
    }
    super::validate_guidance(c.value.target.into()).map_err(arithmetic_error)
}

fn calculate(target: GuidanceTarget, outcome: &OutcomeValue) -> Result<RangeScore> {
    let s = super::score(
        target.into(),
        PrintedOutcome {
            value: outcome.point,
            interval: outcome.rounding,
        },
    )
    .map_err(arithmetic_error)?;
    let hit = if s.definitely_inside {
        Hit::CertainHit
    } else if !s.possibly_inside {
        Hit::CertainMiss
    } else {
        Hit::Boundary
    };
    Ok(RangeScore {
        midpoint: s.midpoint,
        half_width: s.half_width,
        z: s.z,
        z_bounds: s.z_interval,
        side: s.side,
        boundary: s.boundary,
        hit,
        above_midpoint: s.above_midpoint,
    })
}

fn source_join<T, U>(derived: &mut Claim<T>, source: &Claim<U>) {
    derived.evidence = derived.evidence.meet(source.evidence);
    derived.epistemic = derived.epistemic.and(source.epistemic);
    derived.state = derived.state.and(source.state);
    derived.confidentiality = derived.confidentiality.join(&source.confidentiality);
    derived.parents.push(source.claim_id);
    derived.anchors.extend(source.anchors.iter().cloned());
    for term in &source.uu_terms {
        if !derived.uu_terms.contains(term) {
            derived.uu_terms.push(term.clone());
        }
    }
}

fn finish<T>(mut c: Claim<T>, tag: &str) -> Claim<T> {
    c.parents.sort();
    c.parents.dedup();
    c.anchors.sort();
    c.anchors.dedup();
    c.uu_terms.sort_by(|a, b| {
        a.scenario_id
            .cmp(&b.scenario_id)
            .then(a.description.cmp(&b.description))
            .then(
                a.downside_exposure_usd_m
                    .total_cmp(&b.downside_exposure_usd_m),
            )
            .then(a.trigger_indicator.cmp(&b.trigger_indicator))
    });
    Claim::new(
        c.metric_key,
        c.value,
        tag,
        Representation::Deterministic,
        SpatialSupport::PeriodAggregate,
        AdditivityClass::NonAdditive,
        c.evidence,
        c.epistemic,
        c.state,
        c.confidentiality,
        c.uu_terms,
        c.parents,
        c.anchors,
    )
}

fn seed<T, U>(metric: &str, value: T, tag: &str, source: &Claim<U>) -> Claim<T> {
    Claim::new(
        metric,
        value,
        tag,
        Representation::Deterministic,
        SpatialSupport::PeriodAggregate,
        AdditivityClass::NonAdditive,
        EvidenceClass::Derived.meet(source.evidence),
        source.epistemic,
        source.state,
        source.confidentiality.clone(),
        source.uu_terms.clone(),
        vec![source.claim_id],
        source.anchors.clone(),
    )
}

fn validate_pair(guidance: &Claim<GuidanceValue>, outcome: &Claim<OutcomeValue>) -> Result<()> {
    ready(outcome, &outcome.value.canonical_tag())?;
    if guidance.value.key != outcome.value.key {
        return Err(refuse(
            RefusalCode::BasisMismatch105,
            "guidance and outcome keys differ",
        ));
    }
    if guidance.additivity != outcome.additivity {
        return Err(refuse(
            RefusalCode::NonAdditiveAverage103,
            "quantity additivity differs",
        ));
    }
    Ok(())
}

/// Scores one matched, cited annual outcome. This validates interpretation but does not select a vintage.
pub fn score(
    guidance: &Claim<GuidanceValue>,
    outcome: &Claim<OutcomeValue>,
) -> Result<Claim<RangeScore>> {
    validate_guidance(guidance)?;
    validate_pair(guidance, outcome)?;
    let value = calculate(guidance.value.target, &outcome.value)?;
    let tag = value.canonical_tag();
    let mut derived = seed("guidance.range_score", value, &tag, guidance);
    source_join(&mut derived, outcome);
    Ok(finish(derived, &tag))
}

/// One selected guidance member, including those without an admissible outcome.
#[derive(Debug, Clone, PartialEq)]
pub struct MemberInput {
    pub guidance: Claim<GuidanceValue>,
    pub outcome: Option<Claim<OutcomeValue>>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GuidanceSummary {
    pub metric: String,
    pub year: u16,
    pub unit: String,
    pub members: usize,
    pub resolved: usize,
    pub unresolved: usize,
    pub printed_hits: usize,
    pub certain_hits: usize,
    pub certain_misses: usize,
    pub boundary: usize,
    pub below: usize,
    pub above: usize,
    pub observed_hit_rate: Option<Rat>,
    /// All members stay in the denominator; bounds only over missing printed outcomes.
    pub whole_cohort_hit_rate_bounds: Interval,
    /// Completion bounds additionally allow either outcome at each rounding boundary.
    pub rounding_aware_hit_rate_bounds: Interval,
    /// Equal-tailed interval under an IID binomial model for resolved printed-point hits only.
    /// This is not dependence-adjusted or a confidence interval for the whole cohort.
    pub binomial_90: Option<(f64, f64)>,
    pub z_members: usize,
    /// Only resolved proper-range members enter these descriptive position summaries.
    pub mean_z: Option<Rat>,
    pub mean_z_rounding: Option<Interval>,
    pub above_midpoint: usize,
    pub share_above_midpoint: Option<Rat>,
    pub effective_sample_size: Option<Rat>,
    pub bias_confidence_interval: Option<Interval>,
    pub flag: CalibrationFlag,
}

impl GuidanceSummary {
    #[must_use]
    pub fn canonical_tag(&self) -> String {
        let mut e = Encoder::new_frame(*b"GDA1", 1);
        e.put_str(&self.metric);
        e.put_str(&self.unit);
        e.put_u16(self.year);
        for n in [
            self.members,
            self.resolved,
            self.unresolved,
            self.printed_hits,
            self.certain_hits,
            self.certain_misses,
            self.boundary,
            self.below,
            self.above,
            self.z_members,
            self.above_midpoint,
        ] {
            e.put_u64(n as u64);
        }
        for x in [
            self.observed_hit_rate,
            self.mean_z,
            self.share_above_midpoint,
            self.effective_sample_size,
        ] {
            optional_rat(&mut e, x);
        }
        for x in [
            self.whole_cohort_hit_rate_bounds,
            self.rounding_aware_hit_rate_bounds,
        ] {
            put_interval(&mut e, x);
        }
        optional_interval(&mut e, self.mean_z_rounding);
        optional_interval(&mut e, self.bias_confidence_interval);
        e.put_u8(u8::from(self.binomial_90.is_some()));
        if let Some((lo, hi)) = self.binomial_90 {
            e.put_u64(lo.to_bits());
            e.put_u64(hi.to_bits());
        }
        e.put_u8(self.flag as u8);
        blake3_hash(&e.finish()).to_hex()
    }
}

/// Summarizes one metric/year/unit and one adapter-selected publication cohort.
/// A subject may occur once. Revisions must be summarized separately, never as independent members.
pub fn summarize(members: &[MemberInput]) -> Result<Claim<GuidanceSummary>> {
    if members.len() > 100_000 {
        return Err(refuse(
            RefusalCode::NumericalCertificateFailed207,
            "cohort exceeds numerical budget",
        ));
    }
    let mut ordered: Vec<_> = members.iter().collect();
    ordered.sort_by_key(|a| a.guidance.claim_id);
    let first = ordered.first().ok_or_else(|| {
        refuse(
            RefusalCode::NoClaimInDomain201,
            "cannot summarize an empty guidance cohort",
        )
    })?;
    let key = &first.guidance.value.key;
    let mut numerical = Vec::with_capacity(ordered.len());
    let mut subjects = BTreeSet::new();
    for member in &ordered {
        validate_guidance(&member.guidance)?;
        let k = &member.guidance.value.key;
        if k.metric != key.metric || k.year != key.year || k.unit != key.unit {
            return Err(refuse(
                RefusalCode::SupportMismatch102,
                "mixed metric, year or unit in summary",
            ));
        }
        if !subjects.insert(&k.subject) {
            return Err(refuse(
                RefusalCode::BasisMismatch105,
                "duplicate subject in selected guidance cohort",
            ));
        }
        let outcome = member
            .outcome
            .as_ref()
            .map(|outcome| -> Result<PrintedOutcome> {
                validate_pair(&member.guidance, outcome)?;
                Ok(PrintedOutcome {
                    value: outcome.value.point,
                    interval: outcome.value.rounding,
                })
            })
            .transpose()?;
        numerical.push(ScoringMember {
            id: member.guidance.claim_id.to_string(),
            guidance: member.guidance.value.target.into(),
            outcome,
        });
    }
    let raw = super::summarize(&numerical).map_err(arithmetic_error)?;
    let require_bound = |value: Option<Interval>| {
        value.ok_or_else(|| {
            refuse(
                RefusalCode::NumericalCertificateFailed207,
                "nonempty cohort has no completion bound",
            )
        })
    };
    let summary = GuidanceSummary {
        metric: key.metric.clone(),
        year: key.year,
        unit: key.unit.clone(),
        members: raw.n_members,
        resolved: raw.n_resolved,
        unresolved: raw.n_unresolved,
        printed_hits: raw.inside,
        certain_hits: raw.definitely_inside,
        certain_misses: raw.n_resolved - raw.possibly_inside,
        boundary: raw.boundary,
        below: raw.below,
        above: raw.above,
        observed_hit_rate: raw.observed_hit_rate,
        whole_cohort_hit_rate_bounds: require_bound(raw.whole_cohort_hit_bounds)?,
        rounding_aware_hit_rate_bounds: require_bound(raw.rounding_aware_hit_bounds)?,
        binomial_90: raw.iid_reference_interval_90,
        z_members: raw.n_ranges,
        mean_z: raw.mean_z,
        mean_z_rounding: raw.mean_z_rounding_interval,
        above_midpoint: raw.above_midpoint,
        share_above_midpoint: raw.above_midpoint_share,
        effective_sample_size: None,
        bias_confidence_interval: None,
        flag: raw.flag,
    };
    let tag = summary.canonical_tag();
    let mut derived = seed(
        "guidance.descriptive_summary",
        summary,
        &tag,
        &first.guidance,
    );
    for member in &ordered {
        source_join(&mut derived, &member.guidance);
        if let Some(outcome) = &member.outcome {
            source_join(&mut derived, outcome);
        }
    }
    Ok(finish(derived, &tag))
}

#[cfg(test)]
mod tests {
    use super::*;
    use fmine_claim::Confidentiality;
    use fmine_core::anchor::SourceAnchor;
    use fmine_evidence::{EpistemicStatus, KuKind};

    const ZERO: Rat = Rat::from_integer(0);

    fn bounds(lo: Rat, hi: Rat) -> Interval {
        Interval {
            lo,
            hi,
            lo_strict: false,
            hi_strict: false,
        }
    }

    fn rat(n: i128, d: i128) -> Rat {
        Rat::new(n, d).expect("small rational")
    }

    fn key(subject: &str) -> SeriesKey {
        SeriesKey {
            subject: subject.into(),
            metric: "gold_production".into(),
            unit: "koz".into(),
            basis: "100%".into(),
            year: 2025,
        }
    }

    fn claim<T>(value: T, tag: &str, metric: &str) -> Claim<T> {
        Claim::new(
            metric,
            value,
            tag,
            Representation::Deterministic,
            SpatialSupport::PeriodAggregate,
            AdditivityClass::ExtensiveAdditive,
            EvidenceClass::Assumed,
            EpistemicStatus::KnownUnknown(KuKind::Parametric),
            KnowledgeState::Estimated,
            Confidentiality::public(),
            Vec::new(),
            Vec::new(),
            vec![SourceAnchor::new("synthetic-guidance.txt", [7; 32], metric)],
        )
    }

    fn input(subject: &str, target: GuidanceTarget, point: Rat, rounding: Interval) -> MemberInput {
        let guidance = GuidanceValue {
            key: key(subject),
            target,
        };
        let outcome = OutcomeValue {
            key: key(subject),
            point,
            rounding,
        };
        MemberInput {
            guidance: claim(
                guidance.clone(),
                &guidance.canonical_tag(),
                "synthetic.guidance",
            ),
            outcome: Some(claim(
                outcome.clone(),
                &outcome.canonical_tag(),
                "synthetic.outcome",
            )),
        }
    }

    fn range() -> GuidanceTarget {
        GuidanceTarget::Range {
            lo: rat(100, 1),
            hi: rat(120, 1),
        }
    }

    fn member_score(m: &MemberInput) -> Claim<RangeScore> {
        score(&m.guidance, m.outcome.as_ref().expect("outcome")).expect("score")
    }

    #[test]
    fn range_score_known_answer_z_three_halves() {
        let m = input(
            "invented-a",
            range(),
            rat(125, 1),
            bounds(rat(249, 2), rat(251, 2)),
        );
        let s = member_score(&m);
        assert_eq!(s.value.midpoint, Some(rat(110, 1)));
        assert_eq!(s.value.half_width, Some(rat(10, 1)));
        assert_eq!(s.value.z, Some(rat(3, 2)));
        assert_eq!(s.value.z_bounds, Some(bounds(rat(29, 20), rat(31, 20))));
        assert_eq!(
            (s.value.side, s.value.hit, s.value.boundary),
            (Side::Above, Hit::CertainMiss, false)
        );
        assert_eq!(s.evidence, EvidenceClass::Assumed);
        assert_eq!(s.parents.len(), 2);
        s.check_id().expect("derived Claim binds its body");
    }

    #[test]
    fn printed_boundary_is_not_silently_a_definite_hit() {
        let m = input(
            "invented-a",
            range(),
            rat(120, 1),
            bounds(rat(239, 2), rat(241, 2)),
        );
        let s = member_score(&m).value;
        assert_eq!(
            (s.side, s.hit, s.boundary),
            (Side::Inside, Hit::Boundary, true)
        );
        let r = summarize(&[m]).expect("summary").value;
        assert_eq!(r.observed_hit_rate, Some(rat(1, 1)));
        assert_eq!(r.whole_cohort_hit_rate_bounds, bounds(rat(1, 1), rat(1, 1)));
        assert_eq!(
            r.rounding_aware_hit_rate_bounds,
            bounds(rat(0, 1), rat(1, 1))
        );
    }

    #[test]
    fn point_and_one_sided_targets_have_no_z_and_respect_open_ends() {
        for (target, point, hit, side) in [
            (
                GuidanceTarget::Point(rat(100, 1)),
                rat(100, 1),
                Hit::CertainHit,
                Side::Inside,
            ),
            (
                GuidanceTarget::AtMost(rat(100, 1)),
                rat(99, 1),
                Hit::CertainHit,
                Side::Inside,
            ),
            (
                GuidanceTarget::AtLeast(rat(100, 1)),
                rat(99, 1),
                Hit::CertainMiss,
                Side::Below,
            ),
        ] {
            let s = member_score(&input("invented-a", target, point, bounds(point, point))).value;
            assert_eq!(
                (s.z, s.midpoint, s.half_width, s.z_bounds),
                (None, None, None, None)
            );
            assert_eq!((s.hit, s.side), (hit, side));
        }
        let target = GuidanceTarget::AtLeast(rat(100, 1));
        let mut m = input(
            "invented-a",
            target,
            rat(99, 1),
            bounds(rat(98, 1), rat(100, 1)),
        );
        assert_eq!(member_score(&m).value.hit, Hit::Boundary);
        let value = &mut m.outcome.as_mut().expect("outcome").value;
        value.rounding.hi_strict = true;
        m.outcome = Some(claim(
            value.clone(),
            &value.canonical_tag(),
            "synthetic.outcome",
        ));
        assert_eq!(member_score(&m).value.hit, Hit::CertainMiss);
    }

    #[test]
    fn exact_unit_change_preserves_dimensionless_scores() {
        let first = input(
            "invented-a",
            range(),
            rat(125, 1),
            bounds(rat(249, 2), rat(251, 2)),
        );
        // One koz is exactly one thousand troy ounces, each 31.1034768 grams.
        let factor = rat(311034768, 10000);
        let mut converted = first.clone();
        let GuidanceTarget::Range { lo, hi } = range() else {
            unreachable!()
        };
        let g = GuidanceValue {
            key: SeriesKey {
                unit: "grams".into(),
                ..key("invented-a")
            },
            target: GuidanceTarget::Range {
                lo: lo.checked_mul(factor).expect("lo"),
                hi: hi.checked_mul(factor).expect("hi"),
            },
        };
        converted.guidance = claim(g.clone(), &g.canonical_tag(), "synthetic.guidance");
        let v = &first.outcome.as_ref().expect("outcome").value;
        let o = OutcomeValue {
            key: g.key.clone(),
            point: v.point.checked_mul(factor).expect("point"),
            rounding: bounds(
                v.rounding.lo.checked_mul(factor).expect("lo"),
                v.rounding.hi.checked_mul(factor).expect("hi"),
            ),
        };
        converted.outcome = Some(claim(o.clone(), &o.canonical_tag(), "synthetic.outcome"));
        let a = member_score(&first).value;
        let b = member_score(&converted).value;
        assert_eq!(
            (a.z, a.z_bounds, a.side, a.hit),
            (b.z, b.z_bounds, b.side, b.hit)
        );
    }

    #[test]
    fn unresolved_members_remain_in_both_completion_bounds() {
        let a = input(
            "invented-a",
            range(),
            rat(110, 1),
            bounds(rat(110, 1), rat(110, 1)),
        );
        let mut b = input(
            "invented-b",
            range(),
            rat(125, 1),
            bounds(rat(125, 1), rat(125, 1)),
        );
        let before = summarize(&[a.clone(), b.clone()]).expect("summary").value;
        b.outcome = None;
        let after = summarize(&[a.clone(), b.clone()]).expect("summary").value;
        assert_eq!(
            (
                before.members,
                after.members,
                after.resolved,
                after.unresolved
            ),
            (2, 2, 1, 1)
        );
        assert_eq!(
            before.whole_cohort_hit_rate_bounds,
            bounds(rat(1, 2), rat(1, 2))
        );
        assert_eq!(
            after.whole_cohort_hit_rate_bounds,
            bounds(rat(1, 2), rat(1, 1))
        );
        assert_eq!(
            after.rounding_aware_hit_rate_bounds,
            bounds(rat(1, 2), rat(1, 1))
        );
        let mut no_outcome = a;
        no_outcome.outcome = None;
        let all_missing = summarize(&[no_outcome, b]).expect("summary").value;
        assert_eq!(all_missing.flag, CalibrationFlag::Unscored);
        assert_eq!(
            (
                all_missing.observed_hit_rate,
                all_missing.binomial_90,
                all_missing.mean_z
            ),
            (None, None, None)
        );
        assert_eq!(
            all_missing.whole_cohort_hit_rate_bounds,
            bounds(ZERO, rat(1, 1))
        );
    }

    #[test]
    fn binomial_edges_and_no_inferred_calibration_or_dependence() {
        for (point, expected) in [
            (125, (0.0, 0.2588655508930523)),
            (110, (0.7411344491069477, 1.0)),
        ] {
            let members: Vec<_> = (0..10)
                .map(|i| {
                    input(
                        &format!("invented-{i}"),
                        range(),
                        rat(point, 1),
                        bounds(rat(point, 1), rat(point, 1)),
                    )
                })
                .collect();
            let summary = summarize(&members).expect("summary").value;
            let actual = summary.binomial_90.expect("CP interval");
            assert!((actual.0 - expected.0).abs() < 1e-12);
            assert!((actual.1 - expected.1).abs() < 1e-12);
            assert_eq!(summary.flag, CalibrationFlag::Descriptive);
            assert_eq!(
                (
                    summary.effective_sample_size,
                    summary.bias_confidence_interval
                ),
                (None, None)
            );
        }
        let many: Vec<_> = (0..40)
            .map(|i| {
                input(
                    &format!("invented-{i}"),
                    range(),
                    rat(110, 1),
                    bounds(rat(110, 1), rat(110, 1)),
                )
            })
            .collect();
        assert_eq!(
            summarize(&many).expect("summary").value.flag,
            CalibrationFlag::Descriptive
        );
    }

    #[test]
    fn forged_values_bad_keys_and_duplicate_members_refuse() {
        let valid = input(
            "invented-a",
            range(),
            rat(110, 1),
            bounds(rat(110, 1), rat(110, 1)),
        );
        let mut forged = valid.clone();
        forged.outcome.as_mut().expect("outcome").value.point = rat(111, 1);
        assert_eq!(
            score(&forged.guidance, forged.outcome.as_ref().expect("outcome"))
                .expect_err("forgery")
                .code,
            "FMINE-REF-507"
        );
        let mut mismatched = valid.clone();
        let mut o = mismatched.outcome.take().expect("outcome").value;
        o.key.basis = "attributable:3/5".into();
        mismatched.outcome = Some(claim(o.clone(), &o.canonical_tag(), "synthetic.outcome"));
        assert_eq!(
            score(
                &mismatched.guidance,
                mismatched.outcome.as_ref().expect("outcome")
            )
            .expect_err("basis")
            .code,
            "FMINE-REF-105"
        );
        assert!(summarize(&[valid.clone(), valid]).is_err());
    }

    #[test]
    fn summary_claim_is_order_invariant_and_cites_unresolved_guidance() {
        let a = input(
            "invented-a",
            range(),
            rat(110, 1),
            bounds(rat(109, 1), rat(111, 1)),
        );
        let mut b = input(
            "invented-b",
            range(),
            rat(125, 1),
            bounds(rat(124, 1), rat(126, 1)),
        );
        b.outcome = None;
        let id = b.guidance.claim_id;
        let forward = summarize(&[a.clone(), b.clone()]).expect("summary");
        let reverse = summarize(&[b, a]).expect("summary");
        assert_eq!(forward, reverse);
        assert!(forward.parents.contains(&id));
        forward.check_id().expect("canonical summary");
    }
}
