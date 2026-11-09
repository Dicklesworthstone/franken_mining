//! Independent known answers for advisory LP starts and exact plan selection.
//! Determinism: DeterministicPerIsa

use super::*;

fn point(model: &PcpspModel, periods: &[Option<usize>]) -> lp::Solution {
    let scope = lp::scope(model).unwrap();
    let n = scope.blocks.len();
    let by = n * model.periods();
    let mut x = vec![0.0; by * (1 + model.destinations())];
    for (i, &b) in scope.blocks.iter().enumerate() {
        if let Some(period) = periods[b] {
            for t in period..model.periods() {
                x[t * n + i] = 1.0;
            }
            x[by + (period * n + i) * model.destinations()] = 1.0;
        }
    }
    lp::Solution::new(model, &scope.blocks, &x).unwrap()
}

fn knapsack() -> PcpspModel {
    // Duplicate destinations prevent routing from changing this independent
    // mining-set example. A consumes all capacity for10; B+C earns18.
    model(
        &[[10, 10], [9, 9], [9, 9]],
        &[[10, 10], [5, 5], [5, 5]],
        &[10],
        "0",
        &[vec![], vec![], vec![]],
    )
}

#[test]
fn lp_starts_replace_a_value_ten_local_optimum_with_the_value_eighteen_plan() {
    let m = knapsack();
    let baseline = solve(&m, options(8), None).unwrap();
    assert_eq!(
        baseline.discounted_numerator,
        10_000 * i128::from(DISCOUNT_SCALE)
    );
    let guidance = point(&m, &[None, Some(0), Some(0)]);
    let p = solve_with_guidance(&m, options(8), None, &guidance).unwrap();
    assert_eq!(p.origin, "lp_guided");
    assert_eq!(p.discounted_numerator, 18_000 * i128::from(DISCOUNT_SCALE));
    assert_eq!(p.checked.period_values, [18_000]);
    assert_eq!(p.checked.used, [vec![10_000]]);
    assert_eq!(candidate::write_solution(&p.schedule), "1 0 0 1\n2 0 0 1\n");
    let search = p.guidance.as_ref().unwrap();
    assert_eq!((search.starts, search.searched, search.polished), (6, 1, 1));
    assert_eq!(search.best_numerator, p.discounted_numerator);
    assert_eq!(
        p.discounted_numerator / 1_000,
        exhaustive_whole(&m, &p.factors)
    );
    assert_certificate(&m, &p);
    assert_eq!(
        solve_with_guidance(&m, options(8), None, &guidance).unwrap(),
        p
    );
    assert_eq!(solve(&m, options(8), None).unwrap(), baseline);
}

#[test]
fn empty_or_row_infeasible_guidance_cannot_degrade_or_bypass_the_baseline() {
    let m = knapsack();
    let baseline = solve(&m, options(0), None).unwrap();
    let empty = point(&m, &[None, None, None]);
    let mut p = solve_with_guidance(&m, options(0), None, &empty).unwrap();
    assert_eq!(p.guidance.as_ref().unwrap().best_numerator, 0);
    p.guidance = None;
    assert_eq!(p, baseline);
    let baseline_searched = solve(&m, options(2), None).unwrap();
    let p = solve_with_guidance(&m, options(2), None, &empty).unwrap();
    assert!(p.discounted_numerator >= baseline_searched.discounted_numerator);
    assert_certificate(&m, &p);
    // Each share is in [0,1], but the mass-balance rows send each block twice
    // and the resource row exceeds capacity. Such a point is only guidance.
    let scope = lp::scope(&m).unwrap();
    let x = vec![1.0; scope.blocks.len() * m.periods() * (1 + m.destinations())];
    let malformed_rows = lp::Solution::new(&m, &scope.blocks, &x).unwrap();
    let p = solve_with_guidance(&m, options(2), None, &malformed_rows).unwrap();
    assert!(p.discounted_numerator >= baseline.discounted_numerator);
    assert_certificate(&m, &p);
}

#[test]
fn a_better_external_split_and_an_equal_external_split_survive_guidance() {
    // Kahn's starts cannot enter this same-period cycle. The supplied mining
    // set has a profitable half-mill split and must remain available.
    let m = model(
        &[[0, 10], [0, 0]],
        &[[0, 2], [0, 0]],
        &[1],
        "0",
        &[vec![1], vec![0]],
    );
    let external = candidate::parse_solution("0 0 0 0.5\n0 1 0 0.5\n1 0 0 1\n", &m, 3).unwrap();
    let guidance = point(&m, &[Some(0), Some(0)]);
    let p = solve_with_guidance(&m, options(8), Some(&external), &guidance).unwrap();
    assert_eq!(p.origin, "external_candidate");
    assert_eq!(p.schedule, external);
    assert_eq!(p.discounted_numerator, 5_000 * i128::from(DISCOUNT_SCALE));
    assert_eq!(p.guidance.as_ref().unwrap().best_numerator, 0);
    assert_certificate(&m, &p);

    let m = model(&[[0, 10]], &[[0, 2]], &[1], "0", &[vec![]]);
    let external = candidate::parse_solution("0 0 0 0.5\n0 1 0 0.5\n", &m, 3).unwrap();
    let p = solve_with_guidance(&m, options(8), Some(&external), &point(&m, &[Some(0)])).unwrap();
    assert_eq!(p.origin, "external_candidate");
    assert_eq!(p.schedule, external);
    assert_eq!(
        p.guidance.as_ref().unwrap().best_numerator,
        p.discounted_numerator
    );
    assert_certificate(&m, &p);
}

#[test]
fn signed_rows_keep_the_negative_value_helper_in_guidance_and_the_global_bound() {
    let m = model(
        &[[-2, -2], [10, 10]],
        &[[-1, -1], [1, 1]],
        &[0],
        "0",
        &[vec![], vec![]],
    );
    assert!(lp::scope(&m).unwrap().whole);
    let p = solve_with_guidance(&m, options(4), None, &point(&m, &[Some(0), Some(0)])).unwrap();
    assert_eq!(p.checked.mined, 2);
    assert_eq!(p.checked.used, [vec![0]]);
    assert_eq!(p.discounted_numerator, 8_000 * i128::from(DISCOUNT_SCALE));
    assert_certificate(&m, &p);
}

#[test]
fn invalid_shares_and_guidance_dimensions_refuse_without_clamping() {
    let m = knapsack();
    for invalid in [f64::NAN, f64::INFINITY, -f64::EPSILON, 1.0 + f64::EPSILON] {
        let mut x = vec![0.0; 9];
        x[0] = invalid;
        assert!(
            lp::Solution::new(&m, &[0, 1, 2], &x)
                .unwrap_err()
                .what
                .contains("not in [0, 1]")
        );
    }
    assert!(lp::Solution::new(&m, &[0, 1, 2], &[0.0; 8]).is_err());
    let other = model(&[[0, 1]], &[[0, 1]], &[1], "0", &[vec![]]);
    let wrong_shape = point(&other, &[Some(0)]);
    assert!(
        solve_with_guidance(&m, options(2), None, &wrong_shape)
            .unwrap_err()
            .what
            .contains("a solution for 1 blocks")
    );
}

#[test]
fn varied_signed_guidance_preserves_the_incumbent_and_independent_bound() {
    for seed in 0..12_i64 {
        let values: Vec<_> = (0..3)
            .map(|b| [b - 3, (seed * 7 + b * 3) % 13 - 3])
            .collect();
        let uses: Vec<_> = (0..3)
            .map(|b| [if b == 1 { -1 } else { 0 }, (seed + 2 * b) % 4 - 1])
            .collect();
        let m = model(&values, &uses, &[1, 2], "0.25", &[vec![], vec![], vec![0]]);
        let opts = Options {
            rounds: 2,
            bound_steps: 8,
            share_decimals: 2,
        };
        let baseline = solve(&m, opts, None).unwrap();
        let guidance = point(&m, &[Some(0), Some(1), Some(1)]);
        let p = solve_with_guidance(&m, opts, None, &guidance).unwrap();
        assert!(
            p.discounted_numerator >= baseline.discounted_numerator,
            "seed {seed}"
        );
        assert_certificate(&m, &p);
    }
}
