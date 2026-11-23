//! Determinism: Exact

use super::*;
use crate::record::{BaselineKind, parse_rules, persistence_interval};

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

fn seasonal_fixture() -> (RecordRules, Vec<CandidateSeries>) {
    let mut rules = rules();
    for baseline in &mut rules.baselines {
        if matches!(baseline.kind, BaselineKind::Persistence { .. }) {
            baseline.kind = BaselineKind::Persistence {
                pool_from: q(2023, 3),
                pool_through: q(2024, 4),
                point: q(2025, 3),
            };
        }
    }
    let mut inputs = series();
    inputs[0].history = [100, 200, 10_000, 1, 100, 150].map(|n| Some(i(n))).to_vec();
    inputs[1].history = [50, 25, 5_000, 1, 50, 50].map(|n| Some(i(n))).to_vec();
    (rules, inputs)
}

#[test]
fn seasonal_candidate_has_independent_exact_bounds_and_a_complete_transition_audit() {
    let (rules, inputs) = seasonal_fixture();
    let build = |values: &[CandidateSeries]| {
        build_candidate_with_model(&rules, values, 2, CandidateModel::SeasonalQuarterlyRatio)
            .unwrap()
    };
    let candidate = build(&inputs);
    assert_eq!(candidate.model_id, SEASONAL_MODEL_ID);
    let pool = &candidate.pools[0];
    // Independently selected Q3->Q4 ratios: Carlin [2, 3/2], Cortez [1/2, 1].
    // Sorted inverse ECDF at .1, .5, .9 yields 1/2, 1, 2; off-season extremes have no mass.
    assert_eq!(pool.kind, ChangeKind::Ratio);
    assert_eq!(pool.changes, vec![i(2), r(3, 2), r(1, 2), i(1)]);
    assert_eq!(pool.seasonal_destination_quarter, Some(4));
    assert!(pool.excluded_pairs.is_empty());
    assert_eq!(pool.off_season_pairs.len(), 6);
    assert_eq!(pool.off_season_pairs[0].from, q(2023, 4));
    assert_eq!(pool.off_season_pairs[0].to, q(2024, 1));
    assert_eq!(pool.off_season_pairs[0].before, Some(i(200)));
    assert_eq!(pool.off_season_pairs[0].after, Some(i(10_000)));
    assert_eq!(
        candidate.csv,
        concat!(
            "site,target,point,p10,p90,uncalibrated\n",
            "Carlin,production,200.00,100.00,400.00,true\n",
            "Cortez,production,100.00,50.00,200.00,true\n",
        )
    );
    assert_eq!(candidate.not_published_sites, vec!["Hemlo"]);
    let all_seasons = build_candidate(&rules, &inputs, 2).unwrap();
    assert_eq!(
        persistence_interval(
            &all_seasons.pools[0].changes,
            i(200),
            rules.alpha,
            rules.estimator,
        )
        .unwrap(),
        (r(1, 50), i(20_000)),
        "the registered all-season baseline retains its independent exact values"
    );
    assert_ne!(all_seasons.exact_rows, candidate.exact_rows);
    assert_eq!(all_seasons.pools[0].changes.len(), 10);
    assert!(all_seasons.pools[0].off_season_pairs.is_empty());
    assert_eq!(all_seasons.pools[0].seasonal_destination_quarter, None);
    let mut reordered = inputs.clone();
    reordered.reverse();
    assert_eq!(build(&reordered), candidate);
    let mut changed = inputs;
    changed[0].history[2] = Some(i(9));
    let changed = build(&changed);
    assert_eq!(changed.csv, candidate.csv);
    assert_ne!(changed.pools[0].off_season_pairs, pool.off_season_pairs);
}

#[test]
fn seasonal_selection_supports_each_quarter_and_q4_to_q1_year_rollover() {
    for (destination, expected) in [
        (1, vec![i(5), i(5)]),
        (2, vec![i(2), i(6), i(2), i(6)]),
        (3, vec![i(3), i(7), i(3), i(7)]),
        (4, vec![i(4), i(8), i(4), i(8)]),
    ] {
        let (mut rules, mut inputs) = seasonal_fixture();
        rules.targets[0].outcome = crate::record::Outcome::QuarterQtd;
        let point = q(2025, if destination == 1 { 4 } else { destination - 1 });
        for baseline in &mut rules.baselines {
            if matches!(baseline.kind, BaselineKind::Persistence { .. }) {
                baseline.kind = BaselineKind::Persistence {
                    pool_from: q(2023, 1),
                    pool_through: q(2024, 4),
                    point,
                };
            }
        }
        for input in &mut inputs {
            input.history = [2, 4, 12, 48, 240, 1440, 10_080, 80_640]
                .map(|n| Some(i(n)))
                .to_vec();
        }
        let candidate =
            build_candidate_with_model(&rules, &inputs, 2, CandidateModel::SeasonalQuarterlyRatio)
                .unwrap();
        let pool = &candidate.pools[0];
        assert_eq!(pool.changes, expected);
        assert_eq!(pool.seasonal_destination_quarter, Some(destination));
        assert_eq!(pool.changes.len() + pool.off_season_pairs.len(), 14);
        assert_eq!(
            rules.outcome_quarter().unwrap(),
            q(if destination == 1 { 2026 } else { 2025 }, destination)
        );
    }
}

#[test]
fn seasonal_missing_and_nonpositive_pairs_are_not_bridged_or_replaced_by_other_seasons() {
    let (rules, mut inputs) = seasonal_fixture();
    inputs[0].history[1] = None;
    inputs[1].history[1] = Some(i(0));
    let candidate =
        build_candidate_with_model(&rules, &inputs, 2, CandidateModel::SeasonalQuarterlyRatio)
            .unwrap();
    let pool = &candidate.pools[0];
    assert_eq!(pool.changes, vec![r(3, 2), i(1)]);
    assert_eq!(pool.excluded_pairs.len(), 2);
    assert_eq!(pool.excluded_pairs[0].after, None);
    assert_eq!(pool.excluded_pairs[1].after, Some(i(0)));
    assert_eq!(pool.off_season_pairs.len(), 6);
    assert_eq!(pool.off_season_pairs[0].before, None);
    inputs[0].history[5] = None;
    inputs[1].history[5] = Some(i(-1));
    assert!(build_candidate(&rules, &inputs, 2).is_ok());
    let error =
        build_candidate_with_model(&rules, &inputs, 2, CandidateModel::SeasonalQuarterlyRatio)
            .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("no positive adjacent-quarter ratio into Q4")
    );
}

#[test]
fn seasonal_cost_forecasts_retain_signed_and_zero_points() {
    let (mut rules, mut inputs) = seasonal_fixture();
    rules.targets[0].field = "aisc_usd_per_oz".into();
    rules.targets[0].outcome = crate::record::Outcome::Q4Qtd;
    inputs[0].point = Some(i(-200));
    inputs[1].point = Some(i(0));
    let candidate =
        build_candidate_with_model(&rules, &inputs, 2, CandidateModel::SeasonalQuarterlyRatio)
            .unwrap();
    assert_eq!(candidate.exact_rows[0].point, i(-200));
    assert_eq!(candidate.exact_rows[0].lo, i(-400));
    assert_eq!(candidate.exact_rows[0].hi, i(-100));
    assert_eq!(candidate.exact_rows[1].point, i(0));
    assert_eq!(candidate.exact_rows[1].lo, i(0));
    assert_eq!(candidate.exact_rows[1].hi, i(0));
}

#[test]
fn additive_models_use_signed_cost_and_capital_history_at_zero_and_negative_points() {
    for field in [
        "aisc_usd_per_oz",
        "tcc_usd_per_oz",
        "c1_usd_per_lb",
        "aisc_usd_per_lb",
        "capex_usd_m",
        "capex_sustaining_usd_m",
        "capex_projects_usd_m",
    ] {
        let mut rules = rules();
        rules.targets[0].field = field.into();
        rules.targets[0].outcome = crate::record::Outcome::QuarterQtd;
        let mut inputs = series();
        inputs[0].history = [-10, 0, -5].map(|n| Some(i(n))).to_vec();
        inputs[1].history = [0, -20, 10].map(|n| Some(i(n))).to_vec();
        inputs[0].point = Some(i(0));
        inputs[1].point = Some(i(-40));
        let candidate =
            build_candidate_with_model(&rules, &inputs, 2, CandidateModel::PooledQuarterlyChange)
                .unwrap();
        assert_eq!(candidate.model_id, CHANGE_MODEL_ID);
        let pool = &candidate.pools[0];
        // Signed adjacent changes are [10,-5,-20,30]. The inverse ECDF
        // selects -20, -5, 30, independently of the sign of the latest value.
        assert_eq!(pool.kind, ChangeKind::Additive);
        assert_eq!(pool.changes, [i(10), i(-5), i(-20), i(30)]);
        assert_eq!(
            (pool.lower_change, pool.median_change, pool.upper_change),
            (i(-20), i(-5), i(30))
        );
        assert!(pool.excluded_pairs.is_empty());
        assert_eq!(
            candidate.csv,
            concat!(
                "site,target,point,p10,p90,uncalibrated\n",
                "Carlin,production,-5.00,-20.00,30.00,true\n",
                "Cortez,production,-45.00,-60.00,-10.00,true\n",
            )
        );
        assert_eq!(candidate.exact_rows, candidate.serialized_rows);
        assert_eq!(candidate.not_published_sites, ["Hemlo"]);
        assert!(
            build_candidate(&rules, &inputs, 2).is_err(),
            "ratio model remains unchanged"
        );
        inputs.reverse();
        assert_eq!(
            build_candidate_with_model(&rules, &inputs, 2, CandidateModel::PooledQuarterlyChange,)
                .unwrap(),
            candidate
        );
    }
}

#[test]
fn seasonal_additive_changes_keep_quarter_selection_and_missing_pairs_explicit() {
    let (rules, mut inputs) = seasonal_fixture();
    let build = |values: &[CandidateSeries]| {
        build_candidate_with_model(&rules, values, 2, CandidateModel::SeasonalQuarterlyChange)
    };
    let candidate = build(&inputs).unwrap();
    assert_eq!(candidate.model_id, SEASONAL_CHANGE_MODEL_ID);
    let pool = &candidate.pools[0];
    assert_eq!(pool.changes, [i(100), i(50), i(-25), i(0)]);
    assert_eq!(pool.seasonal_destination_quarter, Some(4));
    assert_eq!(pool.off_season_pairs.len(), 6);
    assert_eq!(
        (
            candidate.exact_rows[0].lo,
            candidate.exact_rows[0].point,
            candidate.exact_rows[0].hi
        ),
        (i(175), i(200), i(300))
    );
    inputs[0].history[2] = Some(i(9));
    assert_eq!(
        build(&inputs).unwrap().csv,
        candidate.csv,
        "off-season extremes carry no mass"
    );
    inputs[0].history[1] = None;
    let missing = build(&inputs).unwrap();
    assert_eq!(missing.pools[0].changes, [i(50), i(-25), i(0)]);
    assert_eq!(missing.pools[0].excluded_pairs.len(), 1);
    assert_eq!(missing.pools[0].excluded_pairs[0].after, None);
    for input in &mut inputs {
        input.history[1] = None;
        input.history[5] = None;
    }
    assert!(
        build(&inputs)
            .unwrap_err()
            .to_string()
            .contains("no admissible adjacent-quarter change into Q4")
    );
}

#[test]
fn additive_changes_do_not_bridge_gaps_or_discard_zero_production_history() {
    let rules = rules();
    let mut inputs = series();
    inputs[0].history = vec![Some(i(0)), None, Some(i(100))];
    inputs[1].history = [0, 1, 0].map(|n| Some(i(n))).to_vec();
    let candidate =
        build_candidate_with_model(&rules, &inputs, 2, CandidateModel::PooledQuarterlyChange)
            .unwrap();
    assert_eq!(candidate.pools[0].changes, [i(1), i(-1)]);
    assert_eq!(candidate.pools[0].excluded_pairs.len(), 2);
    assert_eq!(
        (candidate.exact_rows[1].lo, candidate.exact_rows[1].hi),
        (i(99), i(101))
    );
    inputs[0].history = [100, -1, 132].map(|n| Some(i(n))).to_vec();
    let negative =
        build_candidate_with_model(&rules, &inputs, 2, CandidateModel::PooledQuarterlyChange)
            .unwrap();
    assert_eq!(negative.pools[0].changes, candidate.pools[0].changes);
    assert_eq!(negative.pools[0].excluded_pairs[0].after, Some(i(-1)));
    assert_eq!(negative.pools[0].excluded_pairs[1].before, Some(i(-1)));
}

#[test]
fn additive_forecasts_refuse_negative_production_before_decimal_rounding() {
    let mut rules = rules();
    let mut inputs = series();
    for input in &mut inputs {
        input.history = vec![Some(i(1)), Some(r(499, 500)), Some(r(249, 250))];
        input.point = Some(r(1, 1000));
    }
    let error =
        build_candidate_with_model(&rules, &inputs, 0, CandidateModel::PooledQuarterlyChange)
            .unwrap_err();
    assert!(error.to_string().contains("nonnegative production domain"));
    // A cost-like alias does not authorize negative physical production.
    rules.targets[0].field = "unknown".into();
    assert!(
        build_candidate_with_model(&rules, &inputs, 2, CandidateModel::PooledQuarterlyChange,)
            .unwrap_err()
            .to_string()
            .contains("supported field/outcome")
    );
}

#[test]
fn additive_changes_keep_exact_rounding_translation_and_overflow_refusals() {
    let mut rules = rules();
    rules.targets[0].field = "aisc_usd_per_oz".into();
    rules.targets[0].outcome = crate::record::Outcome::Q4Qtd;
    let mut inputs = series();
    for input in &mut inputs {
        input.history = vec![Some(r(-1, 3)), Some(i(0)), Some(r(-1, 3))];
        input.point = Some(i(0));
    }
    let build = |inputs: &[CandidateSeries]| {
        build_candidate_with_model(&rules, inputs, 2, CandidateModel::PooledQuarterlyChange)
    };
    let candidate = build(&inputs).unwrap();
    assert_eq!(
        (
            candidate.exact_rows[0].lo,
            candidate.exact_rows[0].point,
            candidate.exact_rows[0].hi
        ),
        (r(-1, 3), r(-1, 3), r(1, 3))
    );
    assert_eq!(
        (
            candidate.serialized_rows[0].lo,
            candidate.serialized_rows[0].point,
            candidate.serialized_rows[0].hi
        ),
        (r(-34, 100), r(-33, 100), r(34, 100))
    );
    for input in &mut inputs {
        for value in input.history.iter_mut().chain([&mut input.point]) {
            *value = value.map(|v| v.checked_add(i(10)).unwrap());
        }
    }
    let shifted = build(&inputs).unwrap();
    assert_eq!(shifted.pools, candidate.pools);
    assert_eq!(shifted.exact_rows[0].lo, r(29, 3));
    inputs[0].history = vec![Some(i(-i128::MAX)), Some(i(i128::MAX)), Some(i(0))];
    assert_eq!(build(&inputs), Err(RecordError::Overflow));
    for input in &mut inputs {
        input.history = vec![Some(i(0)), Some(i(1)), Some(i(2))];
        input.point = Some(i(i128::MAX));
    }
    assert_eq!(build(&inputs), Err(RecordError::Overflow));
}

#[test]
fn exact_empirical_candidate_and_comparator_agree_on_bounds() {
    // Independent order statistics: sorted ratios [1, 1.1, 1.2, 1.5] select ranks 1, 2 and 4.
    let rules = rules();
    let candidate = build_candidate(&rules, &series(), 2).unwrap();
    assert_eq!(candidate.model_id, MODEL_ID);
    assert_eq!(candidate.not_published_sites, ["Hemlo"]);
    let pool = &candidate.pools[0];
    assert_eq!(pool.kind, ChangeKind::Ratio);
    assert_eq!(pool.changes, [r(11, 10), r(6, 5), i(1), r(3, 2)]);
    assert!(pool.excluded_pairs.is_empty());
    assert_eq!(
        (pool.lower_change, pool.median_change, pool.upper_change),
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
                &pool.changes,
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
                    &candidate.pools[0].changes,
                    input.point.unwrap(),
                    rules.alpha,
                    rules.estimator
                )
                .unwrap()
            );
        }

        inputs[0].history[1] = Some(i(-1));
        let excluded = build_candidate(&rules, &inputs, 2).unwrap();
        assert_eq!(excluded.pools[0].changes, [i(1), r(3, 2)]);
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
    assert_eq!(pool.changes, [i(1), r(3, 2)]);
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
        assert_eq!(candidate.pools[0].changes, [i(1), r(3, 2)]);
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
