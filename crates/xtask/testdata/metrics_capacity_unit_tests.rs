#[cfg(test)]
mod tests {
    use super::*;
    use fmine_claim::{Confidentiality, LabelRegistry, UuTerm};
    use fmine_core::anchor::SourceAnchor;
    use fmine_evidence::EpistemicStatus;

    use crate::guidance::QuantityKind;

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
            Confidentiality::public(),
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

    fn copper_requirement(bounds: Interval, ownership: Ownership) -> Claim<Quantity> {
        let mut q = requirement(bounds, ownership).value;
        q.kind = QuantityKind::ProductionKt;
        cited(
            "remaining_copper_production",
            q.clone(),
            &q.canonical_tag(),
            AdditivityClass::ExtensiveAdditive,
        )
    }

    fn copper_limits() -> CapacityLimits {
        let mut l = limits();
        for (c, kind, interval) in [
            (
                &mut l.feed,
                PhysicalKind::RemainingCopperDryFeedMt,
                point(10),
            ),
            (
                &mut l.grade,
                PhysicalKind::CopperHeadGradeFraction,
                closed(r(1, 200), r(1, 200)),
            ),
            (
                &mut l.recovery,
                PhysicalKind::CopperRecoveryFraction,
                closed(r(9, 10), r(9, 10)),
            ),
        ] {
            let mut value = c.value.clone();
            value.kind = kind;
            value.interval = interval;
            *c = cited(
                &format!("copper_{}", c.metric_key),
                value.clone(),
                &value.canonical_tag(),
                kind.additivity(),
            );
        }
        l
    }

    #[test]
    fn copper_ceiling_uses_dry_mass_and_explicit_ownership_without_gold_conversion() {
        // Independent mass balance: 10 million dry t at 0.5% Cu contains 50,000 t of Cu;
        // 90% recovery gives 45,000 t = 45 kt. An 80% share gives 36 kt.
        let l = copper_limits();
        let required = copper_requirement(closed(r(40, 1), r(60, 1)), Ownership::Full);
        let report = compare_production(&required, &l).unwrap();
        assert_eq!(
            report.maximum_production.value.kind,
            QuantityKind::ProductionKt
        );
        assert_eq!(report.maximum_production.value.interval, point(45));
        assert_eq!(report.verdict, CapacityVerdict::NotExcluded);
        assert_eq!(
            report.optimistic_required_grade.value.kind,
            PhysicalKind::CopperHeadGradeFraction
        );
        assert_eq!(
            report.optimistic_required_grade.value.interval,
            closed(r(1, 225), r(1, 150))
        );
        let attributable = compare_production(
            &copper_requirement(
                closed(r(40, 1), r(60, 1)),
                Ownership::Attributable { share: r(4, 5) },
            ),
            &l,
        )
        .unwrap();
        assert_eq!(attributable.maximum_production.value.interval, point(36));
        assert_eq!(
            attributable.optimistic_required_grade.value.interval,
            closed(r(1, 180), r(1, 120))
        );
        assert_eq!(
            attributable.verdict,
            CapacityVerdict::ExceedsStatedProductionCeiling
        );
        assert_eq!(report, compare_production(&required, &l).unwrap());
        assert_eq!(report.maximum_production.parents.len(), 4);
        assert_eq!(report.maximum_production.anchors.len(), 4);
        assert_eq!(
            report.optimistic_required_grade.parents,
            report.maximum_production.parents
        );
        assert_eq!(report.maximum_production.evidence, EvidenceClass::ISSUER);
    }

    #[test]
    fn copper_bounds_preserve_rounding_signs_and_closed_ceiling_boundary() {
        let mut l = copper_limits();
        l.feed.value.interval = closed(r(19, 2), r(21, 2));
        l.grade.value.interval = closed(r(9, 2000), r(11, 2000));
        l.recovery.value.interval = closed(r(89, 100), r(91, 100));
        // 10.5 Mt * 0.55% * 91% = 52.5525 kt. Lower corners cannot raise this bound.
        let expected = r(21021, 400);
        let at_boundary = compare_production(
            &copper_requirement(closed(expected, expected), Ownership::Full),
            &l,
        )
        .unwrap();
        assert_eq!(
            at_boundary.maximum_production.value.interval,
            closed(expected, expected)
        );
        assert_eq!(at_boundary.verdict, CapacityVerdict::NotExcluded);
        assert_eq!(
            at_boundary.optimistic_required_grade.value.interval,
            closed(r(11, 2000), r(11, 2000))
        );
        assert_eq!(
            compare_production(&copper_requirement(point(53), Ownership::Full), &l)
                .unwrap()
                .verdict,
            CapacityVerdict::ExceedsStatedProductionCeiling
        );
        let satisfied = compare_production(
            &copper_requirement(closed(r(-18, 1), r(-9, 1)), Ownership::Full),
            &copper_limits(),
        )
        .unwrap();
        assert_eq!(satisfied.verdict, CapacityVerdict::AlreadySatisfied);
        assert_eq!(
            satisfied.optimistic_required_grade.value.interval,
            closed(r(-1, 500), r(-1, 1000))
        );
        // Required grade may exceed 100%: retain the contradiction instead of clamping the result.
        let impossible = compare_production(
            &copper_requirement(point(10000), Ownership::Full),
            &copper_limits(),
        )
        .unwrap();
        assert_eq!(
            impossible.optimistic_required_grade.value.interval,
            closed(r(10, 9), r(10, 9))
        );
        assert_eq!(
            impossible.verdict,
            CapacityVerdict::ExceedsStatedProductionCeiling
        );
    }

    #[test]
    fn copper_refuses_cross_metal_factors_and_invalid_mass_fractions() {
        let copper = copper_requirement(point(45), Ownership::Full);
        let gold = requirement(point(45), Ownership::Full);
        assert_eq!(
            compare_production(&copper, &limits()).unwrap_err().code,
            "FMINE-REF-105"
        );
        assert_eq!(
            compare_production(&gold, &copper_limits())
                .unwrap_err()
                .code,
            "FMINE-REF-105"
        );
        for position in 0..3 {
            let mut l = copper_limits();
            let old = limits();
            match position {
                0 => l.feed = old.feed,
                1 => l.grade = old.grade,
                _ => l.recovery = old.recovery,
            }
            assert_eq!(
                compare_production(&copper, &l).unwrap_err().code,
                "FMINE-REF-105"
            );
        }
        for bad in [closed(r(-1, 100), ZERO), closed(ZERO, r(101, 100))] {
            let mut l = copper_limits();
            l.grade.value.interval = bad;
            assert_eq!(
                compare_production(&copper, &l).unwrap_err().code,
                "FMINE-REF-109"
            );
        }
        let mut l = copper_limits();
        l.grade.value.interval = closed(ZERO, ONE);
        assert_eq!(
            compare_production(&copper, &l)
                .unwrap()
                .maximum_production
                .value
                .interval,
            point(9000)
        );
        l.grade.value.interval = point(0);
        assert_eq!(
            compare_production(&copper, &l)
                .unwrap()
                .maximum_production
                .value
                .interval,
            point(0)
        );
        let mut sold = copper.clone();
        sold.value.kind = QuantityKind::SoldMillionLb;
        assert_eq!(
            compare_production(&sold, &l).unwrap_err().code,
            "FMINE-REF-105"
        );
    }

    #[test]
    fn copper_physical_claim_identity_keeps_metal_and_fraction_units_distinct() {
        let gold = limits();
        let copper = copper_limits();
        assert_ne!(
            gold.feed.value.canonical_tag(),
            copper.feed.value.canonical_tag()
        );
        assert_ne!(
            gold.recovery.value.canonical_tag(),
            copper.recovery.value.canonical_tag()
        );
        let mut same_number = gold.grade.value.clone();
        same_number.interval = copper.grade.value.interval;
        assert_ne!(
            same_number.canonical_tag(),
            copper.grade.value.canonical_tag()
        );
        let mut required = copper_requirement(point(45), Ownership::Full);
        required.evidence = EvidenceClass::Assumed;
        let report = compare_production(&required, &copper).unwrap();
        assert_eq!(
            report.maximum_production.evidence,
            EvidenceClass::Assumed.meet(EvidenceClass::ISSUER)
        );
        assert_eq!(
            report.maximum_production.parents,
            report.optimistic_required_grade.parents
        );
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
        let labels = LabelRegistry::new([], ["W-capacity-test"]).unwrap();
        let wall = Confidentiality::mnpi(labels.wall("W-capacity-test").unwrap());
        l.recovery.taint(&wall);
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
        assert_eq!(*c.confidentiality(), wall);
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
