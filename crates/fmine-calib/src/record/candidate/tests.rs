//! Determinism: Exact

use super::*;
use crate::record::{parse_rules, persistence_interval};

fn i(n: i128) -> Rat {
    Rat::from_integer(n)
}

fn r(n: i128, d: i128) -> Rat {
    Rat::new(n, d).unwrap()
}

fn q(year: i32, quarter: u8) -> Quarter {
    Quarter { year, q: quarter }
}

fn rules() -> RecordRules {
    let mut rules =
        parse_rules(include_str!("../../../../../forecasts/fy2026/rules.toml")).unwrap();
    rules.sites = vec![
        rules.sites[0].clone(),
        rules.sites[1].clone(),
        rules.sites[5].clone(),
    ];
    rules.targets.truncate(1);
    rules.baselines.retain(|b| b.target == "production");
    for baseline in &mut rules.baselines {
        if matches!(baseline.kind, BaselineKind::Persistence { .. }) {
            baseline.kind = BaselineKind::Persistence {
                pool_from: q(2024, 1),
                pool_through: q(2024, 3),
                point: q(2024, 4),
            };
        }
    }
    rules
}

fn series() -> Vec<CandidateSeries> {
    vec![
        CandidateSeries {
            site: "Carlin".into(),
            target: "production".into(),
            history: vec![Some(i(100)), Some(i(110)), Some(i(132))],
            point: Some(i(200)),
        },
        CandidateSeries {
            site: "Cortez".into(),
            target: "production".into(),
            history: vec![Some(i(50)), Some(i(50)), Some(i(75))],
            point: Some(i(100)),
        },
    ]
}

#[test]
fn exact_empirical_candidate_and_comparator_agree_on_bounds() {
    // Independent order statistics: sorted ratios [1, 1.1, 1.2, 1.5] select ranks 1, 2 and 4.
    let rules = rules();
    let candidate = build_candidate(&rules, &series(), 2).unwrap();
    assert_eq!(candidate.model_id, MODEL_ID);
    assert_eq!(candidate.not_published_sites, ["Hemlo"]);
    let pool = &candidate.pools[0];
    assert_eq!(pool.ratios, [r(11, 10), r(6, 5), i(1), r(3, 2)]);
    assert!(pool.excluded_pairs.is_empty());
    assert_eq!(
        (pool.lower_ratio, pool.median_ratio, pool.upper_ratio),
        (i(1), r(11, 10), r(3, 2))
    );
    assert_eq!(
        candidate.exact_rows[0],
        ForecastRow {
            site: "Carlin".into(),
            target: "production".into(),
            point: i(220),
            lo: i(200),
            hi: i(300),
        }
    );
    assert_eq!(candidate.exact_rows[1].point, i(110));
    assert_eq!(candidate.exact_rows, candidate.serialized_rows);
    assert_eq!(
        candidate.csv,
        concat!(
            "site,target,point,p10,p90,uncalibrated\n",
            "Carlin,production,220.00,200.00,300.00,true\n",
            "Cortez,production,110.00,100.00,150.00,true\n",
        )
    );
    for (row, input) in candidate.exact_rows.iter().zip(series()) {
        assert_eq!(
            (row.lo, row.hi),
            persistence_interval(
                &pool.ratios,
                input.point.unwrap(),
                rules.alpha,
                rules.estimator
            )
            .unwrap()
        );
    }
    let mut shuffled = series();
    shuffled.reverse();
    assert_eq!(build_candidate(&rules, &shuffled, 2).unwrap(), candidate);
}

#[test]
fn signed_cost_points_reverse_bounds_and_zero_remains_a_complete_candidate() {
    for field in ["aisc_usd_per_oz", "c1_usd_per_lb", "aisc_usd_per_lb"] {
        let mut rules = rules();
        // The target alias remains "production": only the exact field/outcome pair grants signed support.
        rules.targets[0].field = field.into();
        rules.targets[0].outcome = super::super::Outcome::Q4Qtd;
        let positive = build_candidate(&rules, &series(), 2).unwrap();
        let mut inputs = series();
        inputs[0].point = Some(i(-200));
        inputs[1].point = Some(i(0));
        let candidate = build_candidate(&rules, &inputs, 2).unwrap();
        assert_eq!(candidate.pools, positive.pools);
        assert_eq!(candidate.not_published_sites, ["Hemlo"]);
        assert_eq!(
            (
                candidate.exact_rows[0].lo,
                candidate.exact_rows[0].point,
                candidate.exact_rows[0].hi
            ),
            (i(-300), i(-220), i(-200))
        );
        assert_eq!(
            (
                candidate.exact_rows[1].lo,
                candidate.exact_rows[1].point,
                candidate.exact_rows[1].hi
            ),
            (i(0), i(0), i(0))
        );
        assert_eq!(
            candidate.csv,
            concat!(
                "site,target,point,p10,p90,uncalibrated\n",
                "Carlin,production,-220.00,-300.00,-200.00,true\n",
                "Cortez,production,0.00,0.00,0.00,true\n",
            )
        );
        assert_eq!(
            parse_forecasts(candidate.csv.as_bytes(), &rules).unwrap(),
            candidate.exact_rows
        );
        for (row, input) in candidate.exact_rows.iter().zip(&inputs) {
            assert_eq!(
                (row.lo, row.hi),
                persistence_interval(
                    &candidate.pools[0].ratios,
                    input.point.unwrap(),
                    rules.alpha,
                    rules.estimator
                )
                .unwrap()
            );
        }

        inputs[0].history[1] = Some(i(-1));
        let excluded = build_candidate(&rules, &inputs, 2).unwrap();
        assert_eq!(excluded.pools[0].ratios, [i(1), r(3, 2)]);
        assert_eq!(excluded.pools[0].excluded_pairs.len(), 2);
        inputs[0].point = None;
        assert!(build_candidate(&rules, &inputs, 2).is_err());
    }
}

#[test]
fn nonpositive_points_cannot_borrow_cost_support_from_aliases_or_wrong_outcomes() {
    use super::super::Outcome;

    for (field, outcome) in [
        ("au_prod_100_koz", Outcome::FyMinus9m),
        ("cu_prod_100_kt", Outcome::FyMinus9m),
        ("aisc_usd_per_oz", Outcome::FyMinus9m),
        ("c1_usd_per_lb", Outcome::FyMinus9m),
        ("aisc_usd_per_lb", Outcome::FyMinus9m),
        ("c1_usd_per_oz", Outcome::Q4Qtd),
        ("unknown", Outcome::Q4Qtd),
    ] {
        let mut rules = rules();
        rules.targets[0].id = "unit_cost".into();
        rules.targets[0].field = field.into();
        rules.targets[0].outcome = outcome;
        for b in &mut rules.baselines {
            b.target = "unit_cost".into();
        }
        for point in [i(0), i(-1)] {
            let mut inputs = series();
            for s in &mut inputs {
                s.target = "unit_cost".into();
                s.point = Some(point);
            }
            assert!(build_candidate(&rules, &inputs, 2).is_err(), "{field}");
        }
    }
}

#[test]
fn decimal_export_encloses_exact_ratios_and_keeps_actual_scoring_rows() {
    let mut inputs = series();
    for s in &mut inputs {
        s.history = vec![Some(i(9)), Some(i(3)), Some(i(1))];
        s.point = Some(i(1));
    }
    let rules = rules();
    let candidate = build_candidate(&rules, &inputs, 2).unwrap();
    for (exact, written) in candidate.exact_rows.iter().zip(&candidate.serialized_rows) {
        assert_eq!(
            (exact.lo, exact.point, exact.hi),
            (r(1, 3), r(1, 3), r(1, 3))
        );
        assert_eq!(
            (written.lo, written.point, written.hi),
            (r(33, 100), r(33, 100), r(34, 100))
        );
        assert_eq!(
            crate::record::interval_score(exact.lo, exact.hi, rules.alpha, r(1, 3)).unwrap(),
            i(0)
        );
        assert_eq!(
            crate::record::interval_score(written.lo, written.hi, rules.alpha, r(1, 3)).unwrap(),
            r(1, 100)
        );
    }
    assert_eq!(
        parse_forecasts(candidate.csv.as_bytes(), &rules).unwrap(),
        candidate.serialized_rows
    );
    assert_eq!(decimal(r(1, 8), 2, Round::Nearest).unwrap(), "0.13");
    assert_eq!(decimal(r(3, 2), 0, Round::Nearest).unwrap(), "2");
    assert_eq!(decimal(r(1, 3), 0, Round::Down).unwrap(), "0");
    assert_eq!(decimal(r(1, 3), 0, Round::Up).unwrap(), "1");
}

#[test]
fn signed_decimal_export_rounds_outward_with_ties_upward_and_no_negative_zero() {
    for (value, places, down, nearest, up) in [
        (r(-1, 8), 2, "-0.13", "-0.12", "-0.12"),
        (r(-5, 4), 1, "-1.3", "-1.2", "-1.2"),
        (r(-3, 2), 0, "-2", "-1", "-1"),
        (r(-1, 3), 0, "-1", "0", "0"),
        (r(-1, 1_000), 2, "-0.01", "0.00", "0.00"),
        (r(-5, 1_000), 2, "-0.01", "0.00", "0.00"),
        (r(-15, 1_000), 2, "-0.02", "-0.01", "-0.01"),
        (r(i128::MIN, i128::MAX), 0, "-2", "-1", "-1"),
    ] {
        assert_eq!(decimal(value, places, Round::Down).unwrap(), down);
        assert_eq!(decimal(value, places, Round::Nearest).unwrap(), nearest);
        assert_eq!(decimal(value, places, Round::Up).unwrap(), up);
    }
    assert_eq!(
        decimal(i(i128::MIN), 0, Round::Nearest).unwrap(),
        i128::MIN.to_string()
    );
    assert_eq!(
        decimal(r(i128::MIN, 10), 1, Round::Down).unwrap(),
        "-17014118346046923173168730371588410572.8"
    );
    assert_eq!(
        decimal(i(i128::MIN), 1, Round::Nearest),
        Err(RecordError::Overflow)
    );

    let mut rules = rules();
    rules.targets[0].field = "aisc_usd_per_oz".into();
    rules.targets[0].outcome = super::super::Outcome::Q4Qtd;
    let mut inputs = series();
    for s in &mut inputs {
        s.history = vec![Some(i(9)), Some(i(3)), Some(i(1))];
        s.point = Some(i(-1));
    }
    let candidate = build_candidate(&rules, &inputs, 2).unwrap();
    for (exact, written) in candidate.exact_rows.iter().zip(&candidate.serialized_rows) {
        assert_eq!(
            (exact.lo, exact.point, exact.hi),
            (r(-1, 3), r(-1, 3), r(-1, 3))
        );
        assert_eq!(
            (written.lo, written.point, written.hi),
            (r(-34, 100), r(-33, 100), r(-33, 100))
        );
    }
    assert_eq!(
        parse_forecasts(candidate.csv.as_bytes(), &rules).unwrap(),
        candidate.serialized_rows
    );
}

#[test]
fn all_growing_ratios_use_the_empirical_median_inside_the_interval() {
    let mut inputs = series();
    for s in &mut inputs {
        s.history = vec![Some(i(10)), Some(i(20)), Some(i(60))];
        s.point = Some(i(100));
    }
    let candidate = build_candidate(&rules(), &inputs, 0).unwrap();
    for row in candidate.exact_rows {
        assert_eq!((row.lo, row.point, row.hi), (i(200), i(200), i(300)));
    }
}

#[test]
fn missing_and_nonpositive_observations_omit_adjacent_pairs_without_bridging() {
    let mut inputs = series();
    inputs[0].history[1] = None;
    let candidate = build_candidate(&rules(), &inputs, 2).unwrap();
    let pool = &candidate.pools[0];
    assert_eq!(pool.ratios, [i(1), r(3, 2)]);
    assert_eq!(
        pool.excluded_pairs,
        [
            ExcludedPair {
                site: "Carlin".into(),
                from: q(2024, 1),
                to: q(2024, 2),
                before: Some(i(100)),
                after: None
            },
            ExcludedPair {
                site: "Carlin".into(),
                from: q(2024, 2),
                to: q(2024, 3),
                before: None,
                after: Some(i(132))
            },
        ]
    );
    for value in [i(0), i(-1)] {
        inputs[0].history[1] = Some(value);
        let candidate = build_candidate(&rules(), &inputs, 2).unwrap();
        assert_eq!(candidate.pools[0].ratios, [i(1), r(3, 2)]);
        assert_eq!(candidate.pools[0].excluded_pairs[0].after, Some(value));
        assert_eq!(candidate.pools[0].excluded_pairs[1].before, Some(value));
    }
}

#[test]
fn malformed_members_and_missing_points_refuse_the_whole_candidate() {
    let rules = rules();
    let mut cases = Vec::new();
    let mut input = series();
    input.pop();
    cases.push(input);
    let mut input = series();
    input.push(input[0].clone());
    cases.push(input);
    let mut input = series();
    input[0].site = "Hemlo".into();
    cases.push(input);
    let mut input = series();
    input[0].site = "unknown".into();
    cases.push(input);
    let mut input = series();
    input[0].target = "unknown".into();
    cases.push(input);
    let mut input = series();
    input[0].history.pop();
    cases.push(input);
    let mut input = series();
    input[0].history.push(Some(i(1)));
    cases.push(input);
    for point in [None, Some(i(0)), Some(i(-1))] {
        let mut input = series();
        input[0].point = point;
        cases.push(input);
    }
    let mut input = series();
    for s in &mut input {
        s.history = vec![None, None, None];
    }
    cases.push(input);
    for input in cases {
        assert!(build_candidate(&rules, &input, 2).is_err(), "{input:?}");
    }
    assert!(build_candidate(&rules, &series(), 13).is_err());
}

#[test]
fn direct_rule_mutations_cannot_skip_pool_and_universe_guards() {
    let original = rules();
    let mut cases = Vec::new();
    let mut changed = original.clone();
    changed.sites.push(changed.sites[0].clone());
    cases.push(changed);
    let mut changed = original.clone();
    changed.targets.push(changed.targets[0].clone());
    cases.push(changed);
    let mut changed = original.clone();
    changed.sites[0].name = "bad,site".into();
    cases.push(changed);
    let mut changed = original.clone();
    changed.targets.clear();
    cases.push(changed);
    let mut changed = original.clone();
    changed.estimator = Estimator::Linear;
    cases.push(changed);
    for alpha in [i(0), i(1), r(1, 3)] {
        let mut changed = original.clone();
        changed.alpha = alpha;
        cases.push(changed);
    }
    let persistence = original
        .baselines
        .iter()
        .find(|b| matches!(b.kind, BaselineKind::Persistence { .. }))
        .unwrap();
    let mut changed = original.clone();
    changed.baselines.push(persistence.clone());
    cases.push(changed);
    let mut changed = original.clone();
    changed
        .baselines
        .retain(|b| b.kind == BaselineKind::IssuerGuidance);
    cases.push(changed);
    for (start, end, point) in [
        (q(2024, 1), q(2024, 1), q(2024, 4)),
        (q(2024, 3), q(2024, 1), q(2024, 4)),
        (q(2024, 1), q(2024, 3), q(2024, 3)),
        (q(2024, 0), q(2024, 3), q(2024, 4)),
        (q(2024, 1), q(i32::MAX, 4), q(i32::MAX, 4)),
    ] {
        let mut changed = original.clone();
        for b in &mut changed.baselines {
            if matches!(b.kind, BaselineKind::Persistence { .. }) {
                b.kind = BaselineKind::Persistence {
                    pool_from: start,
                    pool_through: end,
                    point,
                };
            }
        }
        cases.push(changed);
    }
    for changed in cases {
        assert!(
            build_candidate(&changed, &series(), 2).is_err(),
            "{changed:?}"
        );
    }
}

#[test]
fn exact_overflow_refuses_instead_of_saturating_or_emitting_partial_rows() {
    let mut inputs = series();
    inputs[0].history = vec![Some(i(1)), Some(i(i128::MAX)), Some(i(1))];
    assert_eq!(
        build_candidate(&rules(), &inputs, 2),
        Err(RecordError::Overflow)
    );
    assert_eq!(
        decimal(i(i128::MAX), 12, Round::Nearest),
        Err(RecordError::Overflow)
    );
    // Adjacent, individually representable ratios have unrepresentable cross-products when sorted.
    assert_eq!(
        quantile(
            &[r(i128::MAX, i128::MAX - 1), r(i128::MAX - 1, i128::MAX - 2)],
            r(1, 2),
            Estimator::InverseEcdf
        ),
        Err(RecordError::Overflow)
    );
}

#[test]
fn decimal_enclosure_and_nearest_point_hold_over_a_rational_grid() {
    for numerator in -64..=64 {
        for denominator in 1..=31 {
            let value = r(numerator, denominator);
            for places in 0..=4 {
                let read = |direction| {
                    Rat::from_decimal_str(&decimal(value, places, direction).unwrap()).unwrap()
                };
                let (lo, point, hi) = (read(Round::Down), read(Round::Nearest), read(Round::Up));
                assert_ne!(lo.checked_cmp(value).unwrap(), Ordering::Greater);
                assert_ne!(hi.checked_cmp(value).unwrap(), Ordering::Less);
                assert_ne!(point.checked_cmp(lo).unwrap(), Ordering::Less);
                assert_ne!(point.checked_cmp(hi).unwrap(), Ordering::Greater);
                assert_ne!(
                    hi.checked_sub(lo)
                        .unwrap()
                        .checked_cmp(r(1, 10_i128.pow(places)))
                        .unwrap(),
                    Ordering::Greater
                );
            }
        }
    }
}
