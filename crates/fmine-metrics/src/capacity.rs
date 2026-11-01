//! Exact gold-production exclusion bounds from stated physical limits (BD-342, COD-01).
//!
//! Dry feed is in million metric tonnes, head grade in grams of gold per dry metric tonne, and recovery
//! is a fraction. All three describe the full mine over the same remaining calendar-month support.
//! Their maximum product is converted with the statutory troy-ounce constant, then scaled by an explicit
//! attributable share if required. Simultaneous attainability of those maxima is never assumed: they give
//! an optimistic ceiling, not a feasible plan or a joint production-cost witness. Callers must authenticate
//! technical-report sources and their units; this kernel alone establishes no public T2 eligibility.
//! Determinism: Exact (checked rational arithmetic; canonical input tags and sorted parent/source identities).

use std::cmp::Ordering;

use fmine_claim::{AdditivityClass, Claim, Representation, SpatialSupport};
use fmine_core::{Encoder, blake3_hash};
use fmine_evidence::{EvidenceClass, KnowledgeState, Refusal, RefusalCode};
use fmine_numerics::exact_feas::{Interval, Rat};
use fmine_units::{TROY_OZ_TO_GRAMS_DEN, TROY_OZ_TO_GRAMS_NUM, UnitsError};

use crate::guidance::{Ownership, Period, Quantity, QuantityKind};

const ZERO: Rat = Rat::from_integer(0);
const ONE: Rat = Rat::from_integer(1);
type Result<T> = std::result::Result<T, Refusal>;

/// Physical meaning and unit together; no wet/dry, tonne/short-ton or percent/fraction conversion is implicit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PhysicalKind {
    RemainingDryFeedMt,
    HeadGradeGPerT,
    RecoveryFraction,
}

impl PhysicalKind {
    fn additivity(self) -> AdditivityClass {
        if self == Self::RemainingDryFeedMt {
            AdditivityClass::ExtensiveAdditive
        } else {
            AdditivityClass::IntensiveWeighted
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhysicalLimit {
    pub subject: String,
    pub period: Period,
    /// Inputs must be `Full`: attributable tonnage or grade is not a physical full-mine limit.
    pub ownership: Ownership,
    pub kind: PhysicalKind,
    /// Explicit closed bounds. Only upper ends determine the optimistic production ceiling.
    pub interval: Interval,
}

impl PhysicalLimit {
    /// Binds gold, dry metric-tonne basis, physical quantity, period, ownership and exact bounds.
    #[must_use]
    pub fn canonical_tag(&self) -> String {
        let mut e = Encoder::new_frame(*b"GCP1", 1);
        e.put_str("gold;dry-metric-tonne;physical-quantity");
        e.put_str(&self.subject);
        e.put_u16(self.period.year);
        e.put_u8(self.period.first_month);
        e.put_u8(self.period.last_month);
        e.put_u8(match self.kind {
            PhysicalKind::RemainingDryFeedMt => 0,
            PhysicalKind::HeadGradeGPerT => 1,
            PhysicalKind::RecoveryFraction => 2,
        });
        match self.ownership {
            Ownership::Full => e.put_u8(0),
            Ownership::Attributable { share } => {
                e.put_u8(1);
                e.put_fixed(&share.num().to_le_bytes());
                e.put_fixed(&share.den().to_le_bytes());
            }
        }
        for r in [self.interval.lo, self.interval.hi] {
            e.put_fixed(&r.num().to_le_bytes());
            e.put_fixed(&r.den().to_le_bytes());
        }
        e.put_u8(u8::from(self.interval.lo_strict));
        e.put_u8(u8::from(self.interval.hi_strict));
        blake3_hash(&e.finish()).to_hex()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CapacityLimits {
    pub feed: Claim<PhysicalLimit>,
    pub grade: Claim<PhysicalLimit>,
    pub recovery: Claim<PhysicalLimit>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapacityVerdict {
    /// Even the lowest remaining requirement is above the most optimistic stated ceiling.
    ExceedsStatedProductionCeiling,
    /// This bound does not exclude a value in the requirement interval; it supplies no feasibility witness.
    NotExcluded,
    /// The entire signed remaining-production interval is nonpositive.
    AlreadySatisfied,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CapacityReport {
    /// Exact point-valued upper ceiling in gold koz on the required production's ownership basis.
    pub maximum_production: Claim<Quantity>,
    /// Algebraic full-mine grade required at the maximum feed and recovery. Signed bounds are retained:
    /// a nonpositive requirement is already delivered, not a predicted negative physical head grade.
    pub optimistic_required_grade: Claim<PhysicalLimit>,
    pub verdict: CapacityVerdict,
}

fn refuse<T>(code: RefusalCode, what: impl Into<String>) -> Result<T> {
    Err(Refusal::new(
        code,
        format!("production capacity: {}", what.into()),
    ))
}

fn exact<T>(value: std::result::Result<T, UnitsError>) -> Result<T> {
    value.map_err(|e| {
        Refusal::new(
            RefusalCode::NumericalCertificateFailed207,
            format!("production capacity: exact arithmetic {e}"),
        )
    })
}

fn ready<T>(c: &Claim<T>, additivity: AdditivityClass) -> Result<()> {
    if matches!(c.representation, Representation::Placeholder { .. }) {
        return refuse(RefusalCode::SourceCellUnusable208, "placeholder input");
    }
    if c.stale || c.state == KnowledgeState::Stale {
        return refuse(RefusalCode::StaleAnchor502, "stale input");
    }
    if c.calibration_failed {
        return refuse(RefusalCode::CalibrationRequired205, "failed calibration");
    }
    if !matches!(c.state, KnowledgeState::Known | KnowledgeState::Estimated) {
        return refuse(RefusalCode::NotObservable202, "unknown or unusable input");
    }
    if c.support != SpatialSupport::PeriodAggregate {
        return refuse(
            RefusalCode::SupportMismatch102,
            "all inputs need period-aggregate support",
        );
    }
    if c.additivity != additivity {
        return refuse(
            RefusalCode::NonAdditiveAverage103,
            "input additivity does not match its quantity",
        );
    }
    if c.anchors.is_empty()
        || c.anchors.iter().any(|a| {
            a.artifact_path.trim().is_empty()
                || a.locator.trim().is_empty()
                || a.artifact_sha256 == [0; 32]
        })
    {
        return refuse(
            RefusalCode::NoClaimInDomain201,
            "every input needs a source digest and locator",
        );
    }
    Ok(())
}

fn interval(i: Interval, signed: bool) -> Result<()> {
    if i.lo_strict || i.hi_strict {
        return refuse(
            RefusalCode::OutsideAoa203,
            "explicitly closed bounds required",
        );
    }
    if exact(i.lo.checked_cmp(i.hi))? == Ordering::Greater || (!signed && i.lo.signum() < 0) {
        return refuse(
            RefusalCode::OutsidePhysicalDomain109,
            "reversed or negative physical bounds",
        );
    }
    Ok(())
}

fn validate(required: &Claim<Quantity>, limits: &CapacityLimits) -> Result<Rat> {
    ready(required, AdditivityClass::ExtensiveAdditive)?;
    let q = &required.value;
    if q.kind != QuantityKind::ProductionKoz {
        return refuse(
            RefusalCode::BasisMismatch105,
            "required quantity must be gold production in koz",
        );
    }
    if q.subject.trim().is_empty() {
        return refuse(
            RefusalCode::UndeclaredUnitBasisOrTz107,
            "subject is undeclared",
        );
    }
    let p = q.period;
    if p.year == 0
        || p.year > 9999
        || p.first_month == 0
        || p.first_month > p.last_month
        || p.last_month > 12
    {
        return refuse(
            RefusalCode::SupportMismatch102,
            "invalid calendar-month support",
        );
    }
    interval(q.interval, true)?;
    let share = match q.ownership {
        Ownership::Full => ONE,
        Ownership::Attributable { share } => share,
    };
    if share.signum() <= 0 || exact(share.checked_cmp(ONE))? == Ordering::Greater {
        return refuse(
            RefusalCode::BasisMismatch105,
            "attributable share must be in (0, 1]",
        );
    }
    for (c, kind) in [
        (&limits.feed, PhysicalKind::RemainingDryFeedMt),
        (&limits.grade, PhysicalKind::HeadGradeGPerT),
        (&limits.recovery, PhysicalKind::RecoveryFraction),
    ] {
        ready(c, kind.additivity())?;
        if c.value.kind != kind
            || c.value.ownership != Ownership::Full
            || c.value.subject != q.subject
        {
            return refuse(
                RefusalCode::BasisMismatch105,
                "physical limit needs its declared unit, full-mine basis and the same subject",
            );
        }
        if c.value.period != q.period {
            return refuse(
                RefusalCode::SupportMismatch102,
                "physical limit and remaining requirement cover different months",
            );
        }
        interval(c.value.interval, false)?;
    }
    if exact(limits.recovery.value.interval.hi.checked_cmp(ONE))? == Ordering::Greater {
        return refuse(
            RefusalCode::OutsidePhysicalDomain109,
            "recovery fraction exceeds one",
        );
    }
    if limits.feed.value.interval.hi == ZERO || limits.recovery.value.interval.hi == ZERO {
        return refuse(
            RefusalCode::OutsidePhysicalDomain109,
            "zero maximum feed or recovery makes required grade undefined",
        );
    }
    Ok(share)
}

fn derived<T>(
    metric: &str,
    value: T,
    tag: &str,
    additivity: AdditivityClass,
    required: &Claim<Quantity>,
    limits: &CapacityLimits,
) -> Claim<T> {
    let mut evidence = EvidenceClass::Derived.meet(required.evidence);
    let mut epistemic = required.epistemic;
    let mut state = required.state;
    let mut confidentiality = required.confidentiality;
    let mut uu_terms = required.uu_terms.clone();
    let mut anchors = required.anchors.clone();
    let mut parents = vec![required.claim_id];
    for p in [&limits.feed, &limits.grade, &limits.recovery] {
        evidence = evidence.meet(p.evidence);
        epistemic = epistemic.and(p.epistemic);
        state = state.and(p.state);
        confidentiality = confidentiality.combine(p.confidentiality);
        for term in &p.uu_terms {
            if !uu_terms.iter().any(|u| u.scenario_id == term.scenario_id) {
                uu_terms.push(term.clone());
            }
        }
        anchors.extend(p.anchors.iter().cloned());
        parents.push(p.claim_id);
    }
    anchors.sort();
    anchors.dedup();
    parents.sort();
    parents.dedup();
    Claim::new(
        metric,
        value,
        tag,
        Representation::Deterministic,
        SpatialSupport::PeriodAggregate,
        additivity,
        evidence,
        epistemic,
        state,
        confidentiality,
        uu_terms,
        parents,
        anchors,
    )
}

/// Compares signed remaining gold production with the most optimistic stated physical ceiling.
/// This is an exclusion bound under supplied assumptions, never a physical feasibility or cost witness.
pub fn compare_production(
    required_remaining: &Claim<Quantity>,
    limits: &CapacityLimits,
) -> Result<CapacityReport> {
    let share = validate(required_remaining, limits)?;
    // Mt × g/t gives 10^6 grams; dividing by 10^3 ounces/koz leaves the factor 1000.
    let grams_per_oz = exact(Rat::new(TROY_OZ_TO_GRAMS_NUM, TROY_OZ_TO_GRAMS_DEN))?;
    let feed_recovery = exact(
        limits
            .feed
            .value
            .interval
            .hi
            .checked_mul(limits.recovery.value.interval.hi),
    )?;
    let full_koz_per_grade = exact(
        exact(feed_recovery.checked_mul(Rat::from_integer(1000)))?.checked_div(grams_per_oz),
    )?;
    let scaled_koz_per_grade = exact(full_koz_per_grade.checked_mul(share))?;
    let ceiling = exact(scaled_koz_per_grade.checked_mul(limits.grade.value.interval.hi))?;
    let q = &required_remaining.value;
    let maximum = Quantity {
        subject: q.subject.clone(),
        period: q.period,
        ownership: q.ownership,
        kind: QuantityKind::ProductionKoz,
        definition:
            "optimistic ceiling from stated full-mine dry feed, gold head grade and recovery".into(),
        interval: Interval {
            lo: ceiling,
            hi: ceiling,
            lo_strict: false,
            hi_strict: false,
        },
    };
    let needed = PhysicalLimit {
        subject: q.subject.clone(),
        period: q.period,
        ownership: Ownership::Full,
        kind: PhysicalKind::HeadGradeGPerT,
        interval: Interval {
            lo: exact(q.interval.lo.checked_div(scaled_koz_per_grade))?,
            hi: exact(q.interval.hi.checked_div(scaled_koz_per_grade))?,
            lo_strict: false,
            hi_strict: false,
        },
    };
    let verdict = if q.interval.hi.signum() <= 0 {
        CapacityVerdict::AlreadySatisfied
    } else if exact(q.interval.lo.checked_cmp(ceiling))? == Ordering::Greater {
        CapacityVerdict::ExceedsStatedProductionCeiling
    } else {
        CapacityVerdict::NotExcluded
    };
    Ok(CapacityReport {
        maximum_production: derived(
            "capacity.gold.maximum_production/1",
            maximum.clone(),
            &maximum.canonical_tag(),
            AdditivityClass::ExtensiveAdditive,
            required_remaining,
            limits,
        ),
        optimistic_required_grade: derived(
            "capacity.gold.optimistic_required_grade/1",
            needed.clone(),
            &needed.canonical_tag(),
            AdditivityClass::IntensiveWeighted,
            required_remaining,
            limits,
        ),
        verdict,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use fmine_claim::{ConfidentialityTier, UuTerm};
    use fmine_core::anchor::SourceAnchor;
    use fmine_evidence::EpistemicStatus;

    fn r(n: i128, d: i128) -> Rat {
        Rat::new(n, d).unwrap()
    }
    fn closed(lo: Rat, hi: Rat) -> Interval {
        Interval {
            lo,
            hi,
            lo_strict: false,
            hi_strict: false,
        }
    }
    fn point(n: i128) -> Interval {
        closed(r(n, 1), r(n, 1))
    }
    fn cited<T>(key: &str, value: T, tag: &str, additivity: AdditivityClass) -> Claim<T> {
        Claim::new(
            key,
            value,
            tag,
            Representation::Deterministic,
            SpatialSupport::PeriodAggregate,
            additivity,
            EvidenceClass::ISSUER,
            EpistemicStatus::KnownKnown,
            KnowledgeState::Known,
            ConfidentialityTier::Public,
            vec![],
            vec![],
            vec![SourceAnchor::new(
                format!("synthetic/{key}.txt"),
                [7; 32],
                "invented physical bound",
            )],
        )
    }
    fn requirement(bounds: Interval, ownership: Ownership) -> Claim<Quantity> {
        let q = Quantity {
            subject: "Example Mine".into(),
            period: Period {
                year: 2030,
                first_month: 7,
                last_month: 12,
            },
            ownership,
            kind: QuantityKind::ProductionKoz,
            definition: String::new(),
            interval: bounds,
        };
        cited(
            "remaining_production",
            q.clone(),
            &q.canonical_tag(),
            AdditivityClass::ExtensiveAdditive,
        )
    }
    fn limits() -> CapacityLimits {
        let c = |key: &str, kind, interval| {
            let q = PhysicalLimit {
                subject: "Example Mine".into(),
                period: Period {
                    year: 2030,
                    first_month: 7,
                    last_month: 12,
                },
                ownership: Ownership::Full,
                kind,
                interval,
            };
            cited(key, q.clone(), &q.canonical_tag(), kind.additivity())
        };
        CapacityLimits {
            feed: c("feed", PhysicalKind::RemainingDryFeedMt, point(10)),
            grade: c("grade", PhysicalKind::HeadGradeGPerT, point(2)),
            recovery: c(
                "recovery",
                PhysicalKind::RecoveryFraction,
                closed(r(9, 10), r(9, 10)),
            ),
        }
    }

    #[test]
    fn cod_01_ceiling_and_required_grade_are_exact() {
        // Target 1000 koz minus delivered 400 koz, with 10 Mt, 2 g/t and 90% recovery.
        let required = requirement(point(1000 - 400), Ownership::Full);
        let report = compare_production(&required, &limits()).unwrap();
        assert_eq!(
            report.verdict,
            CapacityVerdict::ExceedsStatedProductionCeiling
        );
        assert_eq!(
            report.maximum_production.value.interval,
            closed(r(3_750_000_000, 6_479_891), r(3_750_000_000, 6_479_891))
        );
        assert_eq!(
            report.optimistic_required_grade.value.interval,
            closed(r(6_479_891, 3_125_000), r(6_479_891, 3_125_000))
        );
        assert_eq!(report, compare_production(&required, &limits()).unwrap());
        let mut changed = limits();
        changed.grade.value.interval = point(3);
        assert_eq!(
            compare_production(&required, &changed).unwrap().verdict,
            CapacityVerdict::NotExcluded
        );
    }

    #[test]
    fn ownership_scales_production_not_the_physical_limits() {
        let report = compare_production(
            &requirement(point(600), Ownership::Attributable { share: r(4, 5) }),
            &limits(),
        )
        .unwrap();
        assert_eq!(
            report.maximum_production.value.interval.hi,
            r(3_000_000_000, 6_479_891)
        );
        assert_eq!(
            report.optimistic_required_grade.value.interval.lo,
            r(6_479_891, 2_500_000)
        );
        let mut bad = limits();
        bad.feed.value.ownership = Ownership::Attributable { share: r(4, 5) };
        assert_eq!(
            compare_production(&requirement(point(600), Ownership::Full), &bad)
                .unwrap_err()
                .code,
            "FMINE-REF-105"
        );
        for share in [ZERO, r(-1, 2), r(11, 10)] {
            assert_eq!(
                compare_production(
                    &requirement(point(600), Ownership::Attributable { share }),
                    &limits()
                )
                .unwrap_err()
                .code,
                "FMINE-REF-105"
            );
        }
    }

    #[test]
    fn signed_requirements_and_closed_ceiling_boundaries_are_preserved() {
        let l = limits();
        let cap = r(3_750_000_000, 6_479_891);
        for bounds in [
            closed(cap, cap),
            closed(r(500, 1), r(600, 1)),
            closed(r(-1, 1), r(1, 1)),
        ] {
            assert_eq!(
                compare_production(&requirement(bounds, Ownership::Full), &l)
                    .unwrap()
                    .verdict,
                CapacityVerdict::NotExcluded
            );
        }
        let report = compare_production(
            &requirement(closed(r(-10, 1), r(-1, 1)), Ownership::Full),
            &l,
        )
        .unwrap();
        assert_eq!(report.verdict, CapacityVerdict::AlreadySatisfied);
        assert!(report.optimistic_required_grade.value.interval.hi.signum() < 0);
        assert_eq!(
            report
                .optimistic_required_grade
                .value
                .interval
                .lo
                .checked_mul(r(10, 1))
                .unwrap(),
            report
                .optimistic_required_grade
                .value
                .interval
                .hi
                .checked_mul(r(100, 1))
                .unwrap()
        );
    }

    #[test]
    fn rounding_uses_upper_physical_bounds_and_never_averages_rates() {
        let mut l = limits();
        l.feed.value.interval = closed(r(19, 2), r(21, 2));
        l.grade.value.interval = closed(r(195, 100), r(205, 100));
        l.recovery.value.interval = closed(r(895, 1000), r(905, 1000));
        let report = compare_production(&requirement(point(600), Ownership::Full), &l).unwrap();
        let expected = r(21, 2)
            .checked_mul(r(205, 100))
            .unwrap()
            .checked_mul(r(905, 1000))
            .unwrap()
            .checked_mul(r(1000, 1))
            .unwrap()
            .checked_div(r(311_034_768, 10_000_000))
            .unwrap();
        assert_eq!(report.maximum_production.value.interval.hi, expected);
        assert_eq!(report.verdict, CapacityVerdict::NotExcluded);
    }

    #[test]
    fn support_subject_unit_and_additivity_mismatches_refuse() {
        let required = requirement(point(600), Ownership::Full);
        let mut l = limits();
        l.feed.value.period.first_month = 1;
        assert_eq!(
            compare_production(&required, &l).unwrap_err().code,
            "FMINE-REF-102"
        );
        l = limits();
        l.grade.value.subject = "Other Mine".into();
        assert_eq!(
            compare_production(&required, &l).unwrap_err().code,
            "FMINE-REF-105"
        );
        l = limits();
        l.grade.value.kind = PhysicalKind::RecoveryFraction;
        assert_eq!(
            compare_production(&required, &l).unwrap_err().code,
            "FMINE-REF-105"
        );
        l = limits();
        l.grade.additivity = AdditivityClass::ExtensiveAdditive;
        assert_eq!(
            compare_production(&required, &l).unwrap_err().code,
            "FMINE-REF-103"
        );
        l = limits();
        l.feed.support = SpatialSupport::PointCore;
        assert_eq!(
            compare_production(&required, &l).unwrap_err().code,
            "FMINE-REF-102"
        );
    }

    #[test]
    fn unknown_stale_placeholder_uncited_and_failed_calibration_refuse() {
        let required = requirement(point(600), Ownership::Full);
        let mut l = limits();
        l.feed.stale = true;
        assert_eq!(
            compare_production(&required, &l).unwrap_err().code,
            "FMINE-REF-502"
        );
        l = limits();
        l.feed.state = KnowledgeState::Unknown;
        assert_eq!(
            compare_production(&required, &l).unwrap_err().code,
            "FMINE-REF-202"
        );
        l = limits();
        l.feed.representation = Representation::Placeholder {
            token: "n/a".into(),
        };
        assert_eq!(
            compare_production(&required, &l).unwrap_err().code,
            "FMINE-REF-208"
        );
        l = limits();
        l.recovery.anchors.clear();
        assert_eq!(
            compare_production(&required, &l).unwrap_err().code,
            "FMINE-REF-201"
        );
        l = limits();
        l.grade.calibration_failed = true;
        assert_eq!(
            compare_production(&required, &l).unwrap_err().code,
            "FMINE-REF-205"
        );
    }

    #[test]
    fn zero_denominators_invalid_domains_and_overflow_refuse() {
        let required = requirement(point(600), Ownership::Full);
        for kind in [
            PhysicalKind::RemainingDryFeedMt,
            PhysicalKind::RecoveryFraction,
        ] {
            let mut l = limits();
            let c = if kind == PhysicalKind::RemainingDryFeedMt {
                &mut l.feed
            } else {
                &mut l.recovery
            };
            c.value.interval = point(0);
            assert_eq!(
                compare_production(&required, &l).unwrap_err().code,
                "FMINE-REF-109"
            );
        }
        let mut l = limits();
        l.recovery.value.interval = closed(ZERO, r(11, 10));
        assert_eq!(
            compare_production(&required, &l).unwrap_err().code,
            "FMINE-REF-109"
        );
        l = limits();
        l.grade.value.interval = closed(r(-1, 1), ONE);
        assert_eq!(
            compare_production(&required, &l).unwrap_err().code,
            "FMINE-REF-109"
        );
        l = limits();
        l.feed.value.interval.lo_strict = true;
        assert_eq!(
            compare_production(&required, &l).unwrap_err().code,
            "FMINE-REF-203"
        );
        l = limits();
        l.grade.value.interval = point(0);
        assert_eq!(
            compare_production(&required, &l)
                .unwrap()
                .maximum_production
                .value
                .interval,
            point(0)
        );
        l = limits();
        l.feed.value.interval = point(i128::MAX);
        assert_eq!(
            compare_production(&required, &l).unwrap_err().code,
            "FMINE-REF-207"
        );
    }

    #[test]
    fn derived_claims_keep_uncertainty_confidentiality_and_source_identity() {
        let mut required = requirement(point(600), Ownership::Full);
        let mut l = limits();
        required.evidence = EvidenceClass::Assumed;
        l.feed.epistemic = EpistemicStatus::UnmodelledBudget;
        l.grade.state = KnowledgeState::Estimated;
        l.recovery.confidentiality = ConfidentialityTier::BoardEyesOnly;
        let term = UuTerm {
            scenario_id: "feed-disruption".into(),
            description: "synthetic risk".into(),
            downside_exposure_usd_m: 1.0,
            trigger_indicator: "stockpile movement".into(),
        };
        required.uu_terms.push(term.clone());
        l.feed.uu_terms.push(term.clone());
        let report = compare_production(&required, &l).unwrap();
        let c = &report.maximum_production;
        assert_eq!(
            c.evidence,
            EvidenceClass::Assumed.meet(EvidenceClass::ISSUER)
        );
        assert_eq!(c.epistemic, EpistemicStatus::UnmodelledBudget);
        assert_eq!(c.state, KnowledgeState::Estimated);
        assert_eq!(c.confidentiality, ConfidentialityTier::BoardEyesOnly);
        assert_eq!(c.uu_terms, vec![term]);
        assert_eq!(c.anchors.len(), 4);
        assert_eq!(c.parents.len(), 4);
        assert_eq!(report.optimistic_required_grade.parents, c.parents);
        let before = l.feed.value.canonical_tag();
        l.feed.value.kind = PhysicalKind::HeadGradeGPerT;
        assert_ne!(before, l.feed.value.canonical_tag());
        required.evidence = EvidenceClass::Measured;
        l = limits();
        for claim in [&mut l.feed, &mut l.grade, &mut l.recovery] {
            claim.evidence = EvidenceClass::Measured;
        }
        let derived = compare_production(&required, &l).unwrap();
        assert_eq!(derived.maximum_production.evidence, EvidenceClass::Derived);
        assert_eq!(
            derived.optimistic_required_grade.evidence,
            EvidenceClass::Derived
        );
    }
}
