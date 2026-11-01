//! Synthetic known answers and rounding-box containment for the recovery kernel.
//! Determinism: DeterministicPerIsa (the complete output includes square-root summaries).

use super::*;
use fmine_core::anchor::SourceAnchor;

fn q(n: i128, d: i128) -> Rat {
    Rat::new(n, d).unwrap()
}

fn source(kind: QuantityKind, year: u16, point: Rat, half: Rat, annual: bool) -> Claim<Quantity> {
    let value = Quantity {
        subject: "Invented copper site".into(),
        period: Period {
            year,
            first_month: 1,
            last_month: if annual { 12 } else { 3 },
        },
        ownership: Ownership::Full,
        kind,
        point,
        interval: bounds(
            point.checked_sub(half).unwrap(),
            point.checked_add(half).unwrap(),
        ),
    };
    let tag = value.canonical_tag();
    Claim::new(
        format!("synthetic.{kind:?}.{year}"),
        value,
        &tag,
        Representation::Deterministic,
        SpatialSupport::PeriodAggregate,
        kind.additivity(),
        EvidenceClass::Measured,
        EpistemicStatus::KnownKnown,
        KnowledgeState::Known,
        ConfidentialityTier::Public,
        Vec::new(),
        Vec::new(),
        vec![SourceAnchor::new(
            "synthetic/recovery",
            [7; 32],
            format!("{year}:{kind:?}"),
        )],
    )
}

/// Independently hand-computable synthetic observations: implied tails are 1/5, 3/10, 1/2.
/// No historical issuer rows are embedded in these fixtures.
fn training(half_head: Rat, half_recovery: Rat) -> Vec<TrainingRow> {
    [(1, q(80, 1)), (2, q(85, 1)), (4, q(175, 2))]
        .into_iter()
        .enumerate()
        .map(|(i, (h, r))| TrainingRow {
            head: source(
                QuantityKind::HeadGradePercent,
                2000 + i as u16,
                q(h, 1),
                half_head,
                true,
            ),
            recovery: source(
                QuantityKind::RecoveryPercent,
                2000 + i as u16,
                r,
                half_recovery,
                true,
            ),
        })
        .collect()
}

fn operating() -> OperatingRow {
    OperatingRow {
        head: source(QuantityKind::HeadGradePercent, 2003, q(2, 1), ZERO, false),
        recovery: source(QuantityKind::RecoveryPercent, 2003, q(82, 1), ZERO, false),
        processed: source(QuantityKind::ProcessedKt, 2003, q(1000, 1), ZERO, false),
        produced: source(QuantityKind::CopperProducedKt, 2003, q(82, 5), ZERO, false),
    }
}

fn estimates(value: &ModelProjection) -> [Estimate; 4] {
    [
        value.expected_recovery_percent,
        value.gap_pp,
        value.copper_difference_kt,
        value.fraction_of_reported_output,
    ]
}

fn encloses(outer: Interval, inner: Interval) {
    assert!(
        outer.contains(inner.lo).unwrap(),
        "{outer:?} must contain {inner:?}"
    );
    assert!(
        outer.contains(inner.hi).unwrap(),
        "{outer:?} must contain {inner:?}"
    );
}

#[test]
fn independent_exact_fit_and_two_model_known_answers() {
    let fitted = fit(&training(ZERO, ZERO)).unwrap();
    assert_eq!(fitted.value.constant_tails.point, q(1, 3));
    assert_eq!(fitted.value.intercept.point, q(1, 10));
    assert_eq!(fitted.value.slope.point, q(1, 10));
    assert_eq!(fitted.value.residual_sample_variance_at_printed, ZERO);
    assert_eq!(fitted.value.regression_error_variance_at_printed, ZERO);
    assert_eq!(
        fitted.value.residual_sample_sd_at_printed.enclosure,
        [0.0, 0.0]
    );
    let result = project(&fitted, &operating()).unwrap();
    let a = estimates(&result.constant_tails.value);
    let b = estimates(&result.grade_dependent_tails.value);
    assert_eq!(
        a.map(|v| v.point),
        [q(250, 3), q(4, 3), q(4, 15), q(2, 123)]
    );
    assert_eq!(b.map(|v| v.point), [q(85, 1), q(3, 1), q(3, 5), q(3, 82)]);
    for e in a.into_iter().chain(b) {
        assert_eq!(e.interval, bounds(e.point, e.point));
    }
}

#[test]
fn residual_sample_sd_and_regression_error_use_distinct_denominators() {
    // Independent three-point calculation: h=[1,2,3], t=[.1,.3,.3], a=1/30, b=1/10.
    // Residuals [-1/30,1/15,-1/30] have SSE=1/150; n-1 and n-2 give 1/300 and 1/150.
    let rows = [(1, 90), (2, 85), (3, 90)]
        .into_iter()
        .enumerate()
        .map(|(i, (h, r))| TrainingRow {
            head: source(
                QuantityKind::HeadGradePercent,
                2000 + i as u16,
                q(h, 1),
                ZERO,
                true,
            ),
            recovery: source(
                QuantityKind::RecoveryPercent,
                2000 + i as u16,
                q(r, 1),
                ZERO,
                true,
            ),
        })
        .collect::<Vec<_>>();
    let f = fit(&rows).unwrap().value;
    assert_eq!(f.intercept.point, q(1, 30));
    assert_eq!(f.slope.point, q(1, 10));
    assert_eq!(f.residual_sample_variance_at_printed, q(1, 300));
    assert_eq!(f.regression_error_variance_at_printed, q(1, 150));
    for (root, independent) in [
        (f.residual_sample_sd_at_printed, 0.057_735_026_918_962_576),
        (f.regression_error_sd_at_printed, 0.081_649_658_092_772_61),
    ] {
        assert!(root.enclosure[0] <= independent && independent <= root.enclosure[1]);
        assert!((root.approx - independent).abs() < 1e-16);
    }
}

#[test]
fn rounding_enclosures_contain_every_synthetic_training_and_operating_corner() {
    let rows = training(q(1, 100), q(1, 10));
    let fit_box = fit(&rows).unwrap();
    let mut row_box = operating();
    for (c, half) in [
        (&mut row_box.head, q(1, 100)),
        (&mut row_box.recovery, q(1, 2)),
        (&mut row_box.processed, q(1, 2)),
        (&mut row_box.produced, q(1, 10)),
    ] {
        c.value.interval = bounds(
            c.value.point.checked_sub(half).unwrap(),
            c.value.point.checked_add(half).unwrap(),
        );
    }
    let projection_box = project(&fit_box, &row_box).unwrap();
    for training_mask in 0..64 {
        let mut corners = rows.clone();
        for (i, c) in corners
            .iter_mut()
            .flat_map(|r| [&mut r.head, &mut r.recovery])
            .enumerate()
        {
            let point = if (training_mask >> i) & 1 == 0 {
                c.value.interval.lo
            } else {
                c.value.interval.hi
            };
            c.value.point = point;
            c.value.interval = bounds(point, point);
        }
        let fitted = fit(&corners).unwrap();
        for (outer, inner) in [
            (fit_box.value.mean_head, fitted.value.mean_head),
            (fit_box.value.constant_tails, fitted.value.constant_tails),
            (fit_box.value.intercept, fitted.value.intercept),
            (fit_box.value.slope, fitted.value.slope),
        ] {
            encloses(outer.interval, inner.interval);
        }
        for operating_mask in 0..16 {
            let mut corner = row_box.clone();
            for (i, c) in [
                &mut corner.head,
                &mut corner.recovery,
                &mut corner.processed,
                &mut corner.produced,
            ]
            .into_iter()
            .enumerate()
            {
                let point = if (operating_mask >> i) & 1 == 0 {
                    c.value.interval.lo
                } else {
                    c.value.interval.hi
                };
                c.value.point = point;
                c.value.interval = bounds(point, point);
            }
            let actual = project(&fitted, &corner).unwrap();
            for (outer, inner) in [
                (
                    &projection_box.constant_tails.value,
                    &actual.constant_tails.value,
                ),
                (
                    &projection_box.grade_dependent_tails.value,
                    &actual.grade_dependent_tails.value,
                ),
            ] {
                for (outer, inner) in estimates(outer).into_iter().zip(estimates(inner)) {
                    encloses(outer.interval, inner.interval);
                }
            }
        }
    }
}

#[test]
fn widening_rounding_never_narrows_model_outputs() {
    let narrow = fit(&training(q(1, 1000), q(1, 100))).unwrap();
    let wide = fit(&training(q(1, 100), q(1, 10))).unwrap();
    assert_eq!(narrow.value.intercept.point, wide.value.intercept.point);
    assert_eq!(narrow.value.slope.point, wide.value.slope.point);
    for (a, b) in [
        (wide.value.constant_tails, narrow.value.constant_tails),
        (wide.value.intercept, narrow.value.intercept),
        (wide.value.slope, narrow.value.slope),
    ] {
        encloses(a.interval, b.interval);
    }
    let a = project(&narrow, &operating()).unwrap();
    let b = project(&wide, &operating()).unwrap();
    for (a, b) in [
        (&a.constant_tails.value, &b.constant_tails.value),
        (
            &a.grade_dependent_tails.value,
            &b.grade_dependent_tails.value,
        ),
    ] {
        for (inner, outer) in estimates(a).into_iter().zip(estimates(b)) {
            encloses(outer.interval, inner.interval);
        }
    }
}

#[test]
fn negative_gaps_and_extrapolated_recovery_are_not_clamped() {
    let f = fit(&training(ZERO, ZERO)).unwrap();
    let mut row = operating();
    row.recovery.value.point = q(95, 1);
    row.recovery.value.interval = bounds(q(95, 1), q(95, 1));
    let negative = project(&f, &row).unwrap();
    for p in [
        &negative.constant_tails.value,
        &negative.grade_dependent_tails.value,
    ] {
        assert!(p.gap_pp.point.signum() < 0);
        assert!(p.copper_difference_kt.point.signum() < 0);
        assert!(p.fraction_of_reported_output.point.signum() < 0);
    }
    let rows = [(1, 70), (2, 90), (3, 100)]
        .into_iter()
        .enumerate()
        .map(|(i, (h, r))| TrainingRow {
            head: source(
                QuantityKind::HeadGradePercent,
                2000 + i as u16,
                q(h, 1),
                ZERO,
                true,
            ),
            recovery: source(
                QuantityKind::RecoveryPercent,
                2000 + i as u16,
                q(r, 1),
                ZERO,
                true,
            ),
        })
        .collect::<Vec<_>>();
    row.head.value.point = q(4, 1);
    row.head.value.interval = bounds(q(4, 1), q(4, 1));
    let extrapolated = project(&fit(&rows).unwrap(), &row).unwrap();
    assert_eq!(
        extrapolated
            .grade_dependent_tails
            .value
            .expected_recovery_percent
            .point,
        q(310, 3)
    );
}

#[test]
fn ordering_is_deterministic_and_evidence_cannot_be_strengthened() {
    let mut rows = training(ZERO, ZERO);
    rows[0].head.evidence = EvidenceClass::ISSUER;
    rows[1].head.confidentiality = ConfidentialityTier::JvPartner;
    let f = fit(&rows).unwrap();
    rows.reverse();
    assert_eq!(fit(&rows).unwrap(), f);
    let p = project(&f, &operating()).unwrap().grade_dependent_tails;
    assert_eq!(f.evidence, EvidenceClass::ISSUER);
    assert_eq!(p.evidence, EvidenceClass::ISSUER);
    assert_eq!(p.state, KnowledgeState::Estimated);
    assert_eq!(p.confidentiality, ConfidentialityTier::JvPartner);
    assert_eq!(f.anchors.len(), 6);
    assert_eq!(p.anchors.len(), 10);
    assert_eq!(p.parents.len(), 5);
    assert!(p.parents.contains(&f.claim_id));
    assert!(p.uu_terms.iter().any(|u| u.description == MODEL_CAVEAT));
    assert_eq!(p.verify_uu_slot_for_decision(), Ok(()));
    let before = f.claim_id;
    rows[0].recovery.value.point = q(87, 1);
    rows[0].recovery.value.interval = bounds(q(87, 1), q(87, 1));
    let changed = fit(&rows).unwrap();
    assert_ne!(changed.claim_id, before);
    assert_ne!(changed.value.slope.point, f.value.slope.point);
    assert_ne!(
        project(&changed, &operating())
            .unwrap()
            .grade_dependent_tails
            .claim_id,
        p.claim_id
    );
}

#[test]
fn missing_stale_or_invalid_input_claims_refuse() {
    let base = training(ZERO, ZERO);
    let mut rows = base.clone();
    rows[0].head.anchors.clear();
    assert_eq!(fit(&rows).unwrap_err().code, "FMINE-REF-201");
    let mut rows = base.clone();
    rows[0].head.stale = true;
    assert_eq!(fit(&rows).unwrap_err().code, "FMINE-REF-502");
    let mut rows = base.clone();
    rows[0].head.state = KnowledgeState::Unknown;
    assert_eq!(fit(&rows).unwrap_err().code, "FMINE-REF-202");
    let mut rows = base.clone();
    rows[0].head.calibration_failed = true;
    assert_eq!(fit(&rows).unwrap_err().code, "FMINE-REF-205");
    let mut rows = base.clone();
    rows[0].head.representation = Representation::Placeholder {
        token: "n/a".into(),
    };
    assert_eq!(fit(&rows).unwrap_err().code, "FMINE-REF-208");
    let mut rows = base.clone();
    rows[0].head.value.ownership = Ownership::Attributable { share: q(1, 1) };
    assert_eq!(fit(&rows).unwrap_err().code, "FMINE-REF-105");
    let mut rows = base.clone();
    rows[0].head.value.kind = QuantityKind::RecoveryPercent;
    assert_eq!(fit(&rows).unwrap_err().code, "FMINE-REF-105");
    let mut rows = base.clone();
    rows[0].head.additivity = AdditivityClass::ExtensiveAdditive;
    assert_eq!(fit(&rows).unwrap_err().code, "FMINE-REF-103");
    let mut rows = base.clone();
    rows[0].head.value.period.last_month = 3;
    assert_eq!(fit(&rows).unwrap_err().code, "FMINE-REF-102");
    let mut rows = base.clone();
    rows[0].head.value.interval.lo_strict = true;
    assert_eq!(fit(&rows).unwrap_err().code, "FMINE-REF-203");
    let mut rows = base.clone();
    rows[0].head.value.point = q(100, 1);
    assert_eq!(fit(&rows).unwrap_err().code, "FMINE-REF-109");
    let mut rows = base.clone();
    rows[0].head.value.point = ZERO;
    rows[0].head.value.interval = bounds(ZERO, ZERO);
    assert_eq!(fit(&rows).unwrap_err().code, "FMINE-REF-109");
    let mut rows = base;
    rows[0].recovery.value.point = q(101, 1);
    rows[0].recovery.value.interval = bounds(q(101, 1), q(101, 1));
    assert_eq!(fit(&rows).unwrap_err().code, "FMINE-REF-109");
}

#[test]
fn degenerate_or_ambiguous_designs_and_incompatible_quarters_refuse() {
    let base = training(ZERO, ZERO);
    assert_eq!(fit(&base[..2]).unwrap_err().code, "FMINE-REF-104");
    let mut rows = base.clone();
    rows[1] = rows[0].clone();
    assert_eq!(fit(&rows).unwrap_err().code, "FMINE-REF-104");
    let mut rows = base.clone();
    for row in &mut rows {
        row.head.value.point = ONE;
        row.head.value.interval = bounds(ONE, ONE);
    }
    assert_eq!(fit(&rows).unwrap_err().code, "FMINE-REF-104");
    let mut rows = base.clone();
    for row in &mut rows {
        row.head.value.interval = bounds(q(1, 2), q(9, 2));
    }
    assert_eq!(fit(&rows).unwrap_err().code, "FMINE-REF-104");
    let mut f = fit(&base).unwrap();
    let row = operating();
    f.stale = true;
    assert_eq!(project(&f, &row).unwrap_err().code, "FMINE-REF-502");
    f.stale = false;
    let mut row = operating();
    row.head.value.subject = "Another site".into();
    assert_eq!(project(&f, &row).unwrap_err().code, "FMINE-REF-105");
    let mut row = operating();
    row.produced.value.point = ZERO;
    row.produced.value.interval = bounds(ZERO, ZERO);
    assert_eq!(project(&f, &row).unwrap_err().code, "FMINE-REF-109");
    let mut row = operating();
    for c in [
        &mut row.head,
        &mut row.recovery,
        &mut row.processed,
        &mut row.produced,
    ] {
        c.value.period.year = 2002;
    }
    assert_eq!(project(&f, &row).unwrap_err().code, "FMINE-REF-108");
    let mut row = operating();
    for c in [
        &mut row.head,
        &mut row.recovery,
        &mut row.processed,
        &mut row.produced,
    ] {
        c.value.period.last_month = 6;
    }
    assert_eq!(project(&f, &row).unwrap_err().code, "FMINE-REF-102");
}

#[test]
fn exact_arithmetic_overflow_is_a_typed_refusal() {
    let f = fit(&training(ZERO, ZERO)).unwrap();
    let mut row = operating();
    row.processed.value.point = Rat::from_integer(i128::MAX);
    row.processed.value.interval = bounds(row.processed.value.point, row.processed.value.point);
    assert_eq!(project(&f, &row).unwrap_err().code, "FMINE-REF-207");
}
