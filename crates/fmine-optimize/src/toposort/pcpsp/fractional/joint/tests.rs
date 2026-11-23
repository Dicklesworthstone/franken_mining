//! Determinism: BitwisePortable

use super::*;

fn model(values: &[[i64; 2]], uses: &[Vec<[i64; 2]>], limits: &[Vec<i64>]) -> PcpspModel {
    PcpspModel {
        name: "joint period/route oracle".into(),
        periods: limits[0].len(),
        destinations: 2,
        values: values.iter().flatten().copied().collect(),
        uses: uses
            .iter()
            .map(|row| row.iter().flatten().copied().collect())
            .collect(),
        limits: limits.to_vec(),
        requires: vec![vec![]; values.len()],
        discount_rate: 0,
        rounded_values: 0,
        rounded_uses: 0,
        rounded_limits: 0,
    }
}

#[test]
fn joint_fraction_and_period_gain_crosses_the_fixed_fraction_capacity_barrier() {
    let m = model(&[[0, 20]], &[vec![[0, 4]]], &[vec![2, 3]]);
    let text = "0 0 1 0.25\n0 1 1 0.75\n";
    let original = candidate::parse_solution(text, &m, 2).unwrap();
    let fixed = super::super::reschedule(&m, &original, &[2, 1], 8).unwrap();
    assert_eq!(fixed.schedule, original);
    let p = improve(&m, &original, &[2, 1], 8).unwrap();
    assert_eq!((p.search.before, p.search.after), (1_500, 2_000));
    assert_eq!(
        (p.search.moves, p.search.period_moves, p.search.route_moves),
        (1, 1, 1)
    );
    assert_eq!(p.checked.used, [vec![200, 0]]);
    assert_eq!(
        candidate::write_solution(&p.schedule),
        "0 0 0 0.5\n0 1 0 0.5\n"
    );
    assert_eq!(candidate::write_solution(&original), text);
    assert_eq!(improve(&m, &original, &[2, 1], 8).unwrap(), p);
    // Independent enumeration at the requested hundredth grid. Each complete
    // block uses 4*x and earns 20*x*factor; no production checker is called.
    let oracle = [2_i64, 1]
        .into_iter()
        .enumerate()
        .flat_map(|(t, factor)| {
            (0..=100_i64)
                .filter(move |&x| 4 * x <= [2, 3][t] * 100)
                .map(move |x| 20 * x * factor)
        })
        .max()
        .unwrap();
    assert_eq!(p.search.after, i128::from(oracle));
}

#[test]
fn signed_rows_intersect_exact_lower_and_upper_fraction_limits() {
    let m = PcpspModel {
        name: "opposing signed bounds".into(),
        periods: 1,
        destinations: 3,
        values: vec![1, 3, 5],
        uses: vec![vec![0, 1, -1], vec![0, -1, 1]],
        limits: vec![vec![0], vec![0]],
        requires: vec![vec![]],
        discount_rate: 0,
        rounded_values: 0,
        rounded_uses: 0,
        rounded_limits: 0,
    };
    let original = candidate::parse_solution("0 0 0 1\n", &m, 1).unwrap();
    let p = improve(&m, &original, &[1], 4).unwrap();
    assert_eq!((p.search.before, p.search.after), (10, 40));
    assert_eq!((p.search.period_moves, p.search.route_moves), (0, 1));
    assert_eq!(p.checked.used, [vec![0], vec![0]]);
    assert_eq!(
        candidate::write_solution(&p.schedule),
        "0 1 0 0.5\n0 2 0 0.5\n"
    );
    // An odd denominator cannot be silently rounded across either signed row.
    let mut thirds = m;
    thirds.uses = vec![vec![0, 2, -1], vec![0, -2, 1]];
    let original = candidate::parse_solution("0 0 0 1\n", &thirds, 3).unwrap();
    assert_eq!(
        improve(&thirds, &original, &[1], 4).unwrap().schedule,
        original
    );
}

#[test]
fn vacated_blend_credits_and_both_precedence_directions_are_binding() {
    let m = model(
        &[[-4, -4], [10, 10]],
        &[vec![[-1, -1], [1, 1]]],
        &[vec![0, 0]],
    );
    let original = candidate::parse_solution("0 0 0 1\n1 0 0 1\n", &m, 1).unwrap();
    // Moving the negative-value credit later would raise NPV but leave its
    // positive-use partner infeasible in the old period.
    assert_eq!(
        improve(&m, &original, &[2, 1], 4).unwrap().schedule,
        original
    );

    let mut m = model(&[[0, 0], [10, 10]], &[vec![[0, 0], [0, 0]]], &[vec![0, 0]]);
    m.requires[1] = vec![0];
    let original = candidate::parse_solution("0 0 1 1\n1 0 1 1\n", &m, 1).unwrap();
    assert_eq!(
        improve(&m, &original, &[2, 1], 4).unwrap().schedule,
        original
    );
    m.values = vec![-4, -4, 10, 10];
    let original = candidate::parse_solution("0 0 0 1\n1 0 0 1\n", &m, 1).unwrap();
    assert_eq!(
        improve(&m, &original, &[2, 1], 4).unwrap().schedule,
        original
    );
}

#[test]
fn zero_rounds_empty_ties_and_exhausted_work_preserve_a_checked_incumbent() {
    let m = model(&[[0, 20]], &[vec![[0, 4]]], &[vec![2, 3]]);
    let original = candidate::parse_solution("0 0 1 0.25\n0 1 1 0.75\n", &m, 2).unwrap();
    assert_eq!(
        improve(&m, &original, &[2, 1], 0).unwrap().schedule,
        original
    );
    assert_eq!(
        improve(&m, &original, &[0, 0], 8).unwrap().schedule,
        original
    );
    for budget in 0..30 {
        let p = improve_bounded(&m, &original, &[2, 1], 8, budget).unwrap();
        assert!(p.search.work <= budget);
        assert!(p.search.after >= p.search.before);
        assert!(p.checked.feasible());
        assert_eq!(p.checked.discounted_value(&[2, 1]).unwrap(), p.search.after);
    }
    let stopped = improve_bounded(&m, &original, &[2, 1], 8, 0).unwrap();
    assert_eq!(stopped.schedule, original);
    assert!(stopped.search.work_exhausted);
    let empty = candidate::SplitSchedule::new(&m, vec![], 2).unwrap();
    let p = improve(&m, &empty, &[2, 1], 8).unwrap();
    assert_eq!(p.schedule, empty);
    assert_eq!(p.search.work, 0);
}

#[test]
fn same_period_routing_equal_factors_and_single_destination_remain_supported() {
    let m = model(&[[0, 20]], &[vec![[0, 4]]], &[vec![2, 3]]);
    let original = candidate::parse_solution("0 0 1 1\n", &m, 2).unwrap();
    let p = improve(&m, &original, &[1, 1], 4).unwrap();
    assert_eq!(p.search.after, 1_500);
    assert_eq!(p.search.period_moves, 0);
    assert_eq!(p.search.route_moves, 1);
    let single = PcpspModel {
        destinations: 1,
        values: vec![10],
        uses: vec![vec![1]],
        limits: vec![vec![1, 1]],
        requires: vec![vec![0]],
        ..m
    };
    let original = candidate::parse_solution("0 0 1 1\n", &single, 2).unwrap();
    let p = improve(&single, &original, &[2, 1], 4).unwrap();
    assert_eq!(p.search.after, 2_000);
    assert_eq!(p.search.route_moves, 0);
    assert_eq!(candidate::write_solution(&p.schedule), "0 0 0 1\n");
}

#[test]
fn every_single_block_solution_matches_independent_signed_grid_enumeration() {
    for seed in 0..100_i64 {
        let values = [seed % 7 - 3, (seed * 3) % 13 - 5];
        let use_rows = [[0, seed % 9 - 4], [0, (seed * 5) % 11 - 5]];
        let caps = [[seed % 3, (seed + 1) % 4], [seed % 2, (seed + 2) % 3]];
        let m = model(
            &[values],
            &[vec![use_rows[0]], vec![use_rows[1]]],
            &[caps[0].to_vec(), caps[1].to_vec()],
        );
        let original = candidate::parse_solution("0 0 1 1\n", &m, 1).unwrap();
        let p = improve(&m, &original, &[3, 2], 4).unwrap();
        let mut oracle = i128::MIN;
        for (t, factor) in [3, 2].into_iter().enumerate() {
            for x in 0..=10_i64 {
                if (0..2).all(|r| use_rows[r][0] * (10 - x) + use_rows[r][1] * x <= caps[r][t] * 10)
                {
                    oracle =
                        oracle.max(i128::from((values[0] * (10 - x) + values[1] * x) * factor));
                }
            }
        }
        assert_eq!(p.search.after, oracle, "seed {seed}");
        assert!(p.checked.feasible());
    }
}

#[test]
fn multiple_block_results_preserve_feasibility_and_stay_below_an_independent_global_oracle() {
    for seed in 0..12_i64 {
        let values: Vec<_> = (0..3).map(|b| [0, (seed + b * 7) % 13 - 3]).collect();
        let uses: Vec<Vec<_>> = (0..2)
            .map(|r| (0..3).map(|b| [0, (seed + b * 3 + r) % 9 - 4]).collect())
            .collect();
        let caps = vec![vec![seed % 3, 2], vec![1, seed % 2]];
        let mut m = model(&values, &uses, &caps);
        m.requires[2] = vec![0];
        let original = candidate::parse_solution("0 0 1 1\n1 0 1 1\n2 0 1 1\n", &m, 1).unwrap();
        let p = improve(&m, &original, &[3, 2], 8).unwrap();
        let mut oracle = 0_i64;
        for periods in 0..8 {
            let t = [periods & 1, (periods >> 1) & 1, (periods >> 2) & 1];
            if t[0] > t[2] {
                continue;
            }
            for shares in 0..1331 {
                let x = [shares % 11, (shares / 11) % 11, shares / 121];
                if (0..2).all(|r| {
                    (0..2).all(|period| {
                        (0..3)
                            .filter(|&b| t[b] == period)
                            .map(|b| uses[r][b][1] * x[b])
                            .sum::<i64>()
                            <= caps[r][period] * 10
                    })
                }) {
                    oracle = oracle.max((0..3).map(|b| values[b][1] * x[b] * [3, 2][t[b]]).sum());
                }
            }
        }
        assert!(
            p.search.after >= p.search.before && p.search.after <= i128::from(oracle),
            "seed {seed}"
        );
        assert!(p.checked.feasible());
        assert_eq!(p.checked.mined, 3);
        assert_eq!(p.checked.discounted_value(&[3, 2]).unwrap(), p.search.after);
        assert_eq!(improve(&m, &original, &[3, 2], 8).unwrap(), p);
    }
}

#[test]
fn invalid_starts_factor_dimensions_rounds_and_overflow_refuse() {
    let m = model(&[[0, 20]], &[vec![[0, 4]]], &[vec![2, 3]]);
    let original = candidate::parse_solution("0 0 1 1\n", &m, 2).unwrap();
    assert!(improve(&m, &original, &[1], 8).is_err());
    assert!(improve(&m, &original, &[2, -1], 8).is_err());
    assert!(improve(&m, &original, &[2, 1], 1_001).is_err());
    let infeasible = candidate::parse_solution("0 1 0 1\n", &m, 2).unwrap();
    assert!(improve(&m, &infeasible, &[2, 1], 8).is_err());
    let incomplete = candidate::parse_solution("0 0 1 0.5\n", &m, 2).unwrap();
    assert!(improve(&m, &incomplete, &[2, 1], 8).is_err());
    let huge = model(
        &[[0, 1_000_000_000_000_000_000]],
        &[vec![[0, 0]]],
        &[vec![1, 1]],
    );
    let original = candidate::parse_solution("0 0 1 1\n", &huge, 9).unwrap();
    assert!(
        improve(&huge, &original, &[i64::MAX, 1], 8)
            .unwrap_err()
            .what
            .contains("overflows")
    );
}
