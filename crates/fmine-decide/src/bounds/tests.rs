//! Exact recovery-bound oracles, source controls and deterministic sampled enclosures.
//! Determinism: Exact

use super::*;
use fmine_claim::{ClaimDag, Confidentiality, UuTerm};
use fmine_core::{anchor::SourceAnchor, philox4x32_10};

fn q(n: i128, d: i128) -> Rat {
    Rat::new(n, d).unwrap()
}
fn range(a: i128, b: i128, d: i128) -> Interval {
    Interval {
        lower: q(a, d),
        upper: q(b, d),
    }
}
fn model() -> RecoveryBoundModel {
    RecoveryBoundModel {
        model_key: "two-strata".into(),
        question: "Does every admitted completion meet the declared recovery threshold?".into(),
        route: "separate additive parcels".into(),
        metal: "Au".into(),
        additivity_basis: "synthetic independent parcel recovery, no blend interaction".into(),
        untested_bound_basis: "explicit synthetic bounds, not measured testwork".into(),
        weights: WeightBasis::ContainedMetal {
            untested_fraction: range(20, 20, 100),
        },
        tested_recovery: range(90, 90, 100),
        untested_recovery: range(40, 90, 100),
        threshold: q(85, 100),
        interactions: Vec::new(),
    }
}
fn input(m: RecoveryBoundModel) -> Claim<RecoveryBoundModel> {
    let tag = m.canonical_tag();
    Claim::new(
        INPUT_METRIC,
        m,
        &tag,
        Representation::Deterministic,
        SpatialSupport::PeriodAggregate,
        AdditivityClass::NonAdditive,
        EvidenceClass::Assumed,
        EpistemicStatus::KnownUnknown(KuKind::Parametric),
        KnowledgeState::Estimated,
        Confidentiality::internal(),
        vec![UuTerm {
            scenario_id: "untested-bound-failure".into(),
            description: "Untested recovery may lie outside the declared bounds".into(),
            downside_exposure_usd_m: 1.0,
            trigger_indicator: "targeted test outside the assumed interval".into(),
        }],
        Vec::new(),
        vec![SourceAnchor::new(
            "synthetic-bound",
            sha256(b"synthetic input").0,
            "test declaration",
        )],
    )
}
fn card(m: RecoveryBoundModel) -> BoundCard {
    evaluate(&input(m)).unwrap()
}
fn uncertain() -> RecoveryBoundModel {
    RecoveryBoundModel {
        weights: WeightBasis::ContainedMetal {
            untested_fraction: range(15, 25, 100),
        },
        tested_recovery: range(88, 92, 100),
        ..model()
    }
}

#[test]
fn known_answer_80_90_unstable_at_85() {
    let c = card(model());
    assert_eq!(c.summary().recovery, range(80, 90, 100));
    assert_eq!(c.summary().stability, Stability::Unstable);
    assert_eq!(
        c.summary().proceed_condition,
        TestCondition::RecoveryLowerAtLeast(q(13, 20))
    );
    assert_eq!(
        c.summary().stop_condition,
        TestCondition::RecoveryUpperBelow(q(13, 20))
    );
    assert_eq!(c.summary().vertices.len(), 8);
    assert!(
        c.summary()
            .vertices
            .iter()
            .any(|v| v.aggregate_recovery == q(4, 5))
    );
    assert!(
        c.summary()
            .vertices
            .iter()
            .any(|v| v.aggregate_recovery == q(9, 10))
    );
}

#[test]
fn tonne_trap_weights_three_sevenths_and_lower_bound_24_35() {
    let m = RecoveryBoundModel {
        weights: WeightBasis::DryTonnesAndGrade {
            tested_tonnes: range(80, 80, 1),
            tested_grade_g_per_t: range(1, 1, 1),
            untested_tonnes: range(20, 20, 1),
            untested_grade_g_per_t: range(3, 3, 1),
        },
        ..model()
    };
    let c = card(m);
    assert_eq!(c.summary().untested_metal_fraction, range(3, 3, 7));
    assert_eq!(
        c.summary().recovery,
        Interval {
            lower: q(24, 35),
            upper: q(9, 10)
        }
    );
    assert_ne!(c.summary().recovery.lower, q(4, 5));
}

#[test]
fn uncertain_weights_exact_bounds_and_decisive_test_thresholds() {
    let c = card(uncertain());
    assert_eq!(
        c.summary().recovery,
        Interval {
            lower: q(19, 25),
            upper: q(917, 1000)
        }
    );
    assert_eq!(
        c.summary().proceed_condition,
        TestCondition::RecoveryLowerAtLeast(q(19, 25))
    );
    assert_eq!(
        c.summary().stop_condition,
        TestCondition::RecoveryUpperBelow(q(34, 75))
    );
}

#[test]
fn ties_proceed_and_stop_condition_is_strict() {
    let mut m = uncertain();
    m.untested_recovery.lower = q(19, 25);
    let c = card(m);
    assert_eq!(c.summary().recovery.lower, q(17, 20));
    assert_eq!(c.summary().stability, Stability::StableProceed);
    assert_eq!(c.summary().proceed_condition, TestCondition::AlreadyStable);
    let mut m = uncertain();
    m.untested_recovery.upper = q(34, 75);
    let c = card(m.clone());
    assert_eq!(c.summary().recovery.upper, q(17, 20));
    assert_eq!(c.summary().stability, Stability::Unstable);
    m.untested_recovery.upper = q(34, 75).checked_sub(q(1, 1_000_000)).unwrap();
    let c = card(m);
    assert_eq!(c.summary().stability, Stability::StableStop);
    assert_eq!(c.summary().stop_condition, TestCondition::AlreadyStable);
    let mut m = model();
    m.weights = WeightBasis::ContainedMetal {
        untested_fraction: range(0, 0, 1),
    };
    m.threshold = q(9, 10);
    assert_eq!(card(m).summary().stability, Stability::StableProceed);
}

#[test]
fn testing_cannot_resolve_uncertainty_in_a_tested_only_endpoint() {
    let mut m = model();
    m.weights = WeightBasis::ContainedMetal {
        untested_fraction: range(0, 0, 1),
    };
    m.tested_recovery = range(80, 90, 100);
    let c = card(m.clone());
    assert_eq!(c.summary().stability, Stability::Unstable);
    assert!(matches!(
        c.summary().proceed_condition,
        TestCondition::Impossible { .. }
    ));
    assert!(matches!(
        c.summary().stop_condition,
        TestCondition::Impossible { .. }
    ));
    m.weights = WeightBasis::ContainedMetal {
        untested_fraction: range(0, 1, 2),
    };
    m.tested_recovery = range(90, 90, 100);
    let c = card(m);
    assert_eq!(
        c.summary().proceed_condition,
        TestCondition::RecoveryLowerAtLeast(q(4, 5))
    );
    assert!(matches!(
        c.summary().stop_condition,
        TestCondition::Impossible { .. }
    ));
}

#[test]
fn decisive_tests_never_require_values_outside_the_admitted_box() {
    let mut m = model();
    m.threshold = q(19, 20);
    let c = card(m);
    assert_eq!(c.summary().stability, Stability::StableStop);
    assert!(matches!(
        c.summary().proceed_condition,
        TestCondition::Impossible { .. }
    ));
    assert_eq!(c.summary().stop_condition, TestCondition::AlreadyStable);
    let mut m = model();
    m.weights = WeightBasis::ContainedMetal {
        untested_fraction: range(1, 1, 1),
    };
    m.untested_recovery = range(0, 1, 1);
    m.threshold = ONE;
    let c = card(m);
    assert_eq!(
        c.summary().proceed_condition,
        TestCondition::RecoveryLowerAtLeast(ONE)
    );
    assert_eq!(
        c.summary().stop_condition,
        TestCondition::RecoveryUpperBelow(ONE)
    );
}

fn scale(i: Interval, factor: Rat) -> Interval {
    Interval {
        lower: i.lower.checked_mul(factor).unwrap(),
        upper: i.upper.checked_mul(factor).unwrap(),
    }
}
#[test]
fn grade_intervals_propagate_and_common_unit_rescaling_preserves_metal_weights() {
    let basis = |tonnes, grade| WeightBasis::DryTonnesAndGrade {
        tested_tonnes: scale(range(80, 100, 1), tonnes),
        tested_grade_g_per_t: scale(range(1, 2, 1), grade),
        untested_tonnes: scale(range(20, 30, 1), tonnes),
        untested_grade_g_per_t: scale(range(2, 3, 1), grade),
    };
    let original = card(RecoveryBoundModel {
        weights: basis(ONE, ONE),
        ..model()
    });
    assert_eq!(
        original.summary().untested_metal_fraction,
        Interval {
            lower: q(1, 6),
            upper: q(9, 17)
        }
    );
    // Common unit conversions cancel in the ratio; there is no automatic conversion at the API.
    for (tonnes, grade) in [
        (q(1000, 1), ONE),
        (ONE, q(240, 7)),
        (q(45359237, 50000000), q(7, 240)),
    ] {
        let transformed = card(RecoveryBoundModel {
            weights: basis(tonnes, grade),
            ..model()
        });
        assert_eq!(original.summary(), transformed.summary());
    }
}

fn midpoint(i: Interval) -> Rat {
    i.lower
        .checked_add(i.upper)
        .unwrap()
        .checked_div(q(2, 1))
        .unwrap()
}
fn inside(i: Interval, value: Rat) -> bool {
    !value.checked_cmp(i.lower).unwrap().is_lt() && !value.checked_cmp(i.upper).unwrap().is_gt()
}
#[test]
fn narrowing_any_input_cannot_widen_the_bounds() {
    let m = uncertain();
    let outer = card(m.clone()).summary().recovery;
    for axis in 0..3 {
        for end in 0..2 {
            let mut narrowed = m.clone();
            let i = match axis {
                0 => match &mut narrowed.weights {
                    WeightBasis::ContainedMetal { untested_fraction } => untested_fraction,
                    _ => unreachable!(),
                },
                1 => &mut narrowed.tested_recovery,
                _ => &mut narrowed.untested_recovery,
            };
            let mid = midpoint(*i);
            if end == 0 {
                i.lower = mid;
            } else {
                i.upper = mid;
            }
            let inner = card(narrowed).summary().recovery;
            assert!(inside(outer, inner.lower) && inside(outer, inner.upper));
        }
    }
}

#[test]
fn enclosure_over_ten_thousand_philox_completions() {
    let sample = |i: Interval, n: u32| {
        i.lower
            .checked_add(
                i.upper
                    .checked_sub(i.lower)
                    .unwrap()
                    .checked_mul(q(i128::from(n % 1001), 1000))
                    .unwrap(),
            )
            .unwrap()
    };
    for box_id in 0..100_u32 {
        let random = philox4x32_10([box_id, 0, 0, 0], [351, 2026]);
        let i = |a: u32, b: u32| {
            range(
                i128::from(a.min(b) % 51),
                50 + i128::from(a.max(b) % 51),
                100,
            )
        };
        let w = i(random[0], random[1]);
        let t = i(random[1], random[2]);
        let u = i(random[2], random[3]);
        let c = card(RecoveryBoundModel {
            weights: WeightBasis::ContainedMetal {
                untested_fraction: w,
            },
            tested_recovery: t,
            untested_recovery: u,
            ..model()
        });
        for draw in 0..100_u32 {
            let random = philox4x32_10([box_id, draw, 0, 0], [351, 2027]);
            let (w, t, u) = (
                sample(w, random[0]),
                sample(t, random[1]),
                sample(u, random[2]),
            );
            // Independently expand R = R_T + w(R_U - R_T).
            let truth = t
                .checked_add(w.checked_mul(u.checked_sub(t).unwrap()).unwrap())
                .unwrap();
            assert!(inside(c.summary().recovery, truth));
        }
    }
}

#[test]
fn interactions_invalid_physical_domains_and_unrepresentable_arithmetic_refuse() {
    for interaction in ["preg_robbing", "clay", "throughput_coupling"] {
        let mut m = model();
        m.interactions.push(interaction.into());
        assert_eq!(
            evaluate(&input(m)).unwrap_err().code,
            RefusalCode::NonAdditiveAverage103.as_str()
        );
    }
    for bad_interval in [range(-1, 1, 2), range(2, 1, 2), range(0, 2, 1)] {
        let mut m = model();
        m.untested_recovery = bad_interval;
        assert_eq!(
            evaluate(&input(m)).unwrap_err().code,
            RefusalCode::OutsidePhysicalDomain109.as_str()
        );
    }
    for tonnes in [range(0, 0, 1), range(0, 1, 1), range(-1, 1, 1)] {
        let m = RecoveryBoundModel {
            weights: WeightBasis::DryTonnesAndGrade {
                tested_tonnes: tonnes,
                tested_grade_g_per_t: range(1, 1, 1),
                untested_tonnes: range(0, 1, 1),
                untested_grade_g_per_t: range(1, 1, 1),
            },
            ..model()
        };
        assert_eq!(
            evaluate(&input(m)).unwrap_err().code,
            RefusalCode::OutsidePhysicalDomain109.as_str()
        );
    }
    let m = RecoveryBoundModel {
        weights: WeightBasis::DryTonnesAndGrade {
            tested_tonnes: range(i128::MAX, i128::MAX, 1),
            tested_grade_g_per_t: range(2, 2, 1),
            untested_tonnes: range(1, 1, 1),
            untested_grade_g_per_t: range(1, 1, 1),
        },
        ..model()
    };
    assert_eq!(
        evaluate(&input(m)).unwrap_err().code,
        RefusalCode::NumericalCertificateFailed207.as_str()
    );
}

#[test]
fn incorrect_narrow_bounds_exclude_truth_without_becoming_evidence() {
    let narrow = card(model());
    let truth = q(37, 50); // 0.8 * 0.9 + 0.2 * 0.1, outside the declared untested range.
    assert!(!inside(narrow.summary().recovery, truth));
    let broad = card(RecoveryBoundModel {
        untested_recovery: range(0, 1, 1),
        ..model()
    });
    assert!(inside(broad.summary().recovery, truth));
    assert_eq!(narrow.claim().evidence, EvidenceClass::Assumed);
    assert_eq!(broad.claim().evidence, EvidenceClass::Assumed);
}

#[test]
fn claim_metadata_is_bound_and_result_cannot_be_disclosed_as_measured() {
    let original = input(model());
    let first = evaluate(&original).unwrap();
    assert_eq!(first.claim().parents, vec![original.claim_id]);
    assert_eq!(first.claim().uu_terms, original.uu_terms);
    assert_eq!(first.claim().anchors, original.anchors);
    assert_eq!(first.claim().confidentiality, original.confidentiality);
    let mut changed = original.clone();
    changed.uu_terms[0].downside_exposure_usd_m = 1.0000000000001;
    assert_eq!(changed.claim_id, original.claim_id); // generic Claim does not bind UU yet
    assert_ne!(evaluate(&changed).unwrap().digest(), first.digest());
    let mut forged = original.clone();
    forged.value.threshold = q(1, 2);
    assert_eq!(
        evaluate(&forged).unwrap_err().code,
        RefusalCode::FrozenRecordMutation507.as_str()
    );
    let mut stale = original.clone();
    stale.stale = true;
    assert_eq!(
        evaluate(&stale).unwrap_err().code,
        RefusalCode::StaleAnchor502.as_str()
    );
    let mut support = original.clone();
    support.support = SpatialSupport::PointCore;
    assert_eq!(
        evaluate(&support).unwrap_err().code,
        RefusalCode::SupportMismatch102.as_str()
    );
    let mut uu = original.clone();
    uu.uu_terms.clear();
    assert_eq!(
        evaluate(&uu).unwrap_err().code,
        RefusalCode::UnbudgetedUnknowns204.as_str()
    );
    let mut graph = ClaimDag::new();
    graph.insert(&original).unwrap();
    graph.insert(first.claim()).unwrap();
    assert!(graph.guard_disclosure_export(first.claim()).is_err());
    for class in [
        EvidenceClass::Measured,
        EvidenceClass::Derived,
        EvidenceClass::Elicited,
        EvidenceClass::ModelledCalibrated,
    ] {
        let m = model();
        let mut source = input(m.clone());
        source.evidence = class;
        let source = Claim::new(
            INPUT_METRIC,
            m.clone(),
            &m.canonical_tag(),
            source.representation,
            source.support,
            source.additivity,
            class,
            source.epistemic,
            source.state,
            source.confidentiality,
            source.uu_terms,
            source.parents,
            source.anchors,
        );
        let c = evaluate(&source).unwrap();
        assert!(c.claim().evidence.no_stronger_than(class));
        assert!(!matches!(
            c.claim().evidence,
            EvidenceClass::Measured | EvidenceClass::Derived | EvidenceClass::ModelledCalibrated
        ));
        assert!(ClaimDag::new().guard_disclosure_export(c.claim()).is_err());
    }
}
