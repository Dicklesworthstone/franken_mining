//! Determinism: DeterministicPerIsa

use super::*;
use crate::toposort::search::DISCOUNT_SCALE;

fn model(
    values: &[[i64; 2]],
    uses: &[Vec<[i64; 2]>],
    caps: &[Vec<i64>],
    periods: usize,
) -> PcpspModel {
    assert_eq!(uses.len(), caps.len());
    assert!(uses.iter().all(|r| r.len() == values.len()));
    assert!(caps.iter().all(|r| r.len() == periods));
    PcpspModel {
        name: "fractional oracle".into(),
        periods,
        destinations: 2,
        values: values.iter().flatten().copied().collect(),
        uses: uses
            .iter()
            .map(|r| r.iter().flatten().copied().collect())
            .collect(),
        limits: caps.to_vec(),
        requires: vec![vec![]; values.len()],
        discount_rate: 0,
        rounded_values: 0,
        rounded_uses: 0,
        rounded_limits: 0,
    }
}

#[test]
fn active_master_column_supplies_the_profitable_half_that_whole_repair_loses() {
    let m = model(&[[0, 10]], &[vec![[0, 2]]], &[vec![1]], 1);
    assert!(price::reprice(&m, &[0], &[0], &[1]).is_none());
    let p = polish(&m, &[Some(0)], &[0], &[DISCOUNT_SCALE], 3).unwrap();
    assert_eq!(p.before, 0);
    assert_eq!(p.after, 5_000 * i128::from(DISCOUNT_SCALE));
    assert_eq!(p.checked.period_values, [5_000]);
    assert_eq!(p.checked.used, [vec![1_000]]);
    assert_eq!(p.improved_periods, 1);
    assert!(p.proposals <= m.resources() + 3);
    assert_eq!(
        candidate::write_solution(&p.schedule),
        "0 0 0 0.5\n0 1 0 0.5\n"
    );
    assert_eq!(
        candidate::parse_solution(&candidate::write_solution(&p.schedule), &m, 3).unwrap(),
        p.schedule
    );
    assert_eq!(
        polish(&m, &[Some(0)], &[0], &[DISCOUNT_SCALE], 3).unwrap(),
        p
    );
}

#[test]
fn exact_master_mixture_preserves_distinct_splits_across_binding_capacities() {
    let m = model(
        &[[2, 10], [2, 14]],
        &[vec![[0, 2], [0, 0]], vec![[0, 0], [0, 4]]],
        &[vec![1], vec![3]],
        1,
    );
    // For mill fractions x and y, 2x <= 1 and 4y <= 3. The objective
    // 4 + 8x + 12y has its unique maximum 17 at (1/2, 3/4).
    // A single line to a whole assignment can reach at most 14.
    assert!(price::reprice(&m, &[0, 1], &[0, 0], &[1, 3]).is_none());
    let p = polish(&m, &[Some(0); 2], &[0; 2], &[1], 3).unwrap();
    assert_eq!((p.before, p.after), (4_000, 17_000));
    assert_eq!(p.checked.used, [vec![1_000], vec![3_000]]);
    assert_eq!(
        candidate::write_solution(&p.schedule),
        "0 0 0 0.5\n0 1 0 0.5\n1 0 0 0.25\n1 1 0 0.75\n"
    );
    assert!(p.checked.feasible());
    assert_eq!(p.improved_periods, 1);
    assert_eq!(polish(&m, &[Some(0); 2], &[0; 2], &[1], 3).unwrap(), p);
    // The optimum's quarter share is not rounded to tenths. The previous
    // exact line remains available and supplies value 14 on that coarser grid.
    let coarse = polish(&m, &[Some(0); 2], &[0; 2], &[1], 1).unwrap();
    assert_eq!(coarse.after, 140);
    assert!(coarse.checked.feasible());
}

fn opposing_routes(positive_use: i64) -> PcpspModel {
    PcpspModel {
        name: "opposite blend directions".into(),
        periods: 1,
        destinations: 3,
        values: vec![1, 3, 5],
        uses: vec![vec![0, positive_use, -1], vec![0, -positive_use, 1]],
        limits: vec![vec![0]; 2],
        requires: vec![vec![]],
        discount_rate: 0,
        rounded_values: 0,
        rounded_uses: 0,
        rounded_limits: 0,
    }
}

#[test]
fn a_joint_mixture_crosses_opposing_signed_rows_that_block_every_single_line() {
    let m = opposing_routes(1);
    // Both signed rows must be <= 0, hence route1 == route2. Neither
    // non-baseline destination is usable by itself, but their half/half mix
    // earns 4 rather than the baseline's 1. The block remains fully mined.
    assert!(price::reprice(&m, &[0], &[0], &[0, 0]).is_none());
    let p = polish(&m, &[Some(0)], &[0], &[DISCOUNT_SCALE], 3).unwrap();
    assert_eq!(p.after, 4_000 * i128::from(DISCOUNT_SCALE));
    assert_eq!(p.checked.used, [vec![0], vec![0]]);
    assert_eq!(
        candidate::write_solution(&p.schedule),
        "0 1 0 0.5\n0 2 0 0.5\n"
    );
    assert_eq!(candidate::check(&m, &p.schedule).unwrap(), p.checked);
    let whole = polish(&m, &[Some(0)], &[0], &[1], 0).unwrap();
    assert_eq!((whole.before, whole.after), (1, 1));

    // No exact decimal represents the full 1/3 : 2/3 mixture. Unresolved
    // arithmetic and work caps preserve the feasible baseline, not a rounded
    // violation of either tight signed constraint.
    for mut other in [opposing_routes(2), opposing_routes(1)] {
        if other.uses[0][1] == 1 {
            other.uses.resize(33, vec![0; 3]);
            other.limits.resize(33, vec![0]);
        }
        let fallback = polish(&other, &[Some(0)], &[0], &[1], 3).unwrap();
        assert_eq!((fallback.before, fallback.after), (1_000, 1_000));
        assert!(fallback.checked.feasible());
        assert_eq!(candidate::write_solution(&fallback.schedule), "0 0 0 1\n");
    }
}

#[test]
fn decimal_mixture_rechecks_mass_and_every_signed_row_without_tolerance() {
    let m = opposing_routes(1);
    let q = |n, d| Rat::new(n, d).unwrap();
    let good = vec![vec![q(0, 1), q(1, 2), q(1, 2)]];
    assert_eq!(
        decimal_mixture(&m, &[0], &[0, 0], &good, 1_000),
        Some((4_000, vec![vec![0, 500, 500]]))
    );
    for row in [
        vec![q(0, 1), q(499, 1_000), q(501, 1_000)],
        vec![q(0, 1), q(501, 1_000), q(499, 1_000)],
        vec![q(0, 1), q(1, 2), q(1, 4)],
        vec![q(-1, 1), q(1, 1), q(1, 1)],
        vec![q(0, 1), q(1, 3), q(2, 3)],
    ] {
        assert!(decimal_mixture(&m, &[0], &[0, 0], &[row], 1_000).is_none());
    }
    assert!(decimal_mixture(&m, &[0], &[0], &good, 1_000).is_none());
}

#[test]
fn all_signed_rows_bound_the_step_and_preserve_the_mining_periods() {
    let mut m = model(
        &[[-4, 12], [-2, 8], [-3, 21]],
        &[vec![[1, 1], [1, 1], [1, 1]], vec![[0, 2], [0, -1], [0, 0]]],
        &[vec![2, 2], vec![0, 0]],
        2,
    );
    m.requires[2] = vec![0];
    let p = polish(&m, &[Some(0), Some(0), Some(1)], &[0, 1, 1], &[1, 1], 3).unwrap();
    assert_eq!((p.before, p.after), (25_000, 33_000));
    assert_eq!(p.checked.period_values, [12_000, 21_000]);
    assert_eq!(p.checked.used, [vec![2_000, 1_000], vec![0, 0]]);
    assert_eq!(p.improved_periods, 1);
    assert_eq!(
        candidate::write_solution(&p.schedule),
        "0 0 0 0.5\n0 1 0 0.5\n1 1 0 1\n2 1 1 1\n"
    );
    assert!(p.checked.feasible());
}

#[test]
fn tightest_resource_uses_exact_decimal_floor_and_never_rounds_up() {
    let m = model(
        &[[0, 12]],
        &[vec![[0, 2]], vec![[0, 3]]],
        &[vec![1], vec![1]],
        1,
    );
    let p = polish(&m, &[Some(0)], &[0], &[1], 3).unwrap();
    assert_eq!(p.after, 3_996); // 12 * floor(1000 / 3), over scale 1000.
    assert_eq!(p.checked.used, [vec![666], vec![999]]);
    assert_eq!(
        candidate::write_solution(&p.schedule),
        "0 0 0 0.667\n0 1 0 0.333\n"
    );
    assert!(p.checked.feasible());
    let p = polish(&m, &[Some(0)], &[0], &[1], 0).unwrap();
    assert_eq!((p.before, p.after, p.improved_periods), (0, 0, 0));
    assert_eq!(candidate::write_solution(&p.schedule), "0 0 0 1\n");
}

#[test]
fn a_negative_dump_assignment_can_become_a_positive_exact_split() {
    let m = model(&[[-2, 10]], &[vec![[0, 2]]], &[vec![1]], 1);
    let p = polish(&m, &[Some(0)], &[0], &[1], 3).unwrap();
    assert_eq!((p.before, p.after), (-2_000, 4_000));
    assert_eq!(p.checked.used, [vec![1_000]]);
}

#[test]
fn blocked_nonprofitable_empty_and_zero_discount_cases_keep_the_checked_start() {
    let m = model(&[[0, 10]], &[vec![[0, 1]]], &[vec![0]], 1);
    let blocked = polish(&m, &[Some(0)], &[0], &[1], 3).unwrap();
    assert_eq!(
        (blocked.before, blocked.after, blocked.improved_periods),
        (0, 0, 0)
    );
    let m = model(&[[10, 2]], &[vec![[0, 1]]], &[vec![1]], 1);
    let best = polish(&m, &[Some(0)], &[0], &[1], 3).unwrap();
    assert_eq!(
        (best.before, best.after, best.improved_periods),
        (10_000, 10_000, 0)
    );
    let empty = polish(&m, &[None], &[0], &[1], 3).unwrap();
    assert_eq!((empty.after, empty.proposals), (0, 0));
    assert!(empty.schedule.assignments().is_empty());
    let zero = polish(&m, &[Some(0)], &[1], &[0], 3).unwrap();
    assert_eq!(
        (zero.after, zero.proposals, zero.improved_periods),
        (0, 0, 0)
    );
    assert_eq!(candidate::write_solution(&zero.schedule), "0 1 0 1\n");
}

#[test]
fn every_generated_split_is_monotone_feasible_and_below_an_exact_decimal_grid_oracle() {
    for seed in 0..24_i64 {
        let values: Vec<_> = (0..3).map(|b| [0, (seed * 7 + b * 3) % 11 - 2]).collect();
        let uses: Vec<Vec<_>> = (0..2)
            .map(|r| {
                (0..3)
                    .map(|b| [0, (seed + b * 2 + r * 3) % 7 - 3])
                    .collect()
            })
            .collect();
        let caps = vec![vec![seed % 3], vec![1 + seed % 2]];
        let m = model(&values, &uses, &caps, 1);
        let p = polish(&m, &[Some(0); 3], &[0; 3], &[1], 1).unwrap();
        // Independent 11^3 enumeration of every decimal-grid mill fraction;
        // the dump receives the complement, and all three blocks stay mined.
        let mut best = 0;
        for a in 0..=10 {
            for b in 0..=10 {
                for c in 0..=10 {
                    let q = [a, b, c];
                    let feasible = uses.iter().zip(&caps).all(|(row, cap)| {
                        row.iter().zip(q).map(|(u, x)| u[1] * x).sum::<i64>() <= cap[0] * 10
                    });
                    if feasible {
                        best = best.max(values.iter().zip(q).map(|(v, x)| v[1] * x).sum::<i64>());
                    }
                }
            }
        }
        assert!(p.checked.feasible(), "seed {seed}");
        assert!(
            p.after >= p.before && p.after <= i128::from(best),
            "seed {seed}: {} vs {best}",
            p.after
        );
        assert_eq!(p.checked.mined, 3);
        assert_eq!(p.checked.discounted_value(&[1]).unwrap(), p.after);
        assert!(p.proposals <= m.resources() + 3);
    }
}

#[test]
fn invalid_starts_and_unrepresentable_discounted_numerators_refuse() {
    let m = model(&[[0, 10]], &[vec![[0, 2]]], &[vec![1]], 1);
    assert!(
        polish(&m, &[Some(0)], &[1], &[1], 3)
            .unwrap_err()
            .what
            .contains("starting schedule is infeasible")
    );
    assert!(polish(&m, &[Some(0)], &[0], &[], 3).is_err());
    assert!(polish(&m, &[Some(0)], &[0], &[-1], 3).is_err());
    assert!(polish(&m, &[Some(0)], &[0], &[1], 10).is_err());
    assert!(polish(&m, &[Some(1)], &[0], &[1], 3).is_err());
    assert!(polish(&m, &[], &[0], &[1], 3).is_err());
    let huge = model(
        &[[0, 1_000_000_000_000_000_000]],
        &[vec![[0, 2]]],
        &[vec![1]],
        1,
    );
    assert!(
        polish(&huge, &[Some(0)], &[0], &[DISCOUNT_SCALE], 9)
            .unwrap_err()
            .what
            .contains("overflow")
    );
}

#[test]
fn exact_split_period_move_advances_value_without_rounding_its_routes() {
    let m = model(&[[0, 10]], &[vec![[0, 2]]], &[vec![1, 1]], 2);
    let start = candidate::parse_solution("0 0 1 0.5\n0 1 1 0.5\n", &m, 3).unwrap();
    let p = reschedule(&m, &start, &[10, 9], 8).unwrap();
    assert_eq!((p.search.before, p.search.after), (45_000, 50_000));
    assert_eq!(p.search.moves, 1);
    assert_eq!(p.search.pair_moves, 0);
    assert_eq!(p.checked.used, [vec![1_000, 0]]);
    assert_eq!(
        candidate::write_solution(&p.schedule),
        "0 0 0 0.5\n0 1 0 0.5\n"
    );
    assert_eq!(reschedule(&m, &start, &[10, 9], 8).unwrap(), p);
    assert_eq!(reschedule(&m, &start, &[10, 9], 0).unwrap().schedule, start);
    assert_eq!(reschedule(&m, &start, &[0, 0], 8).unwrap().schedule, start);
    assert_eq!(reschedule(&m, &start, &[1, 1], 8).unwrap().search.moves, 0);
    // A self requirement moves on both sides of its own arc.
    let mut self_arc = m.clone();
    self_arc.requires[0] = vec![0];
    assert_eq!(
        reschedule(&self_arc, &start, &[10, 9], 8).unwrap().schedule,
        p.schedule
    );
}

#[test]
fn paired_split_periods_cross_opposing_blend_limits_at_the_enumerated_optimum() {
    let m = model(
        &[[0, 12], [0, 8]],
        &[vec![[0, 2], [0, -2]], vec![[0, -2], [0, 2]]],
        &[vec![0, 0], vec![0, 0]],
        2,
    );
    let start =
        candidate::parse_solution("0 0 1 0.5\n0 1 1 0.5\n1 0 1 0.5\n1 1 1 0.5\n", &m, 3).unwrap();
    // Independent enumeration on the two declared half-share streams: both
    // sides of the blend impose opposite zero limits. Separate moves cannot
    // leave either source or target balanced, although their joint move can.
    let factors = [10_i128, 5];
    let mut optimum = 0;
    let mut feasible = 0;
    for a in 0..2 {
        for b in 0..2 {
            let mut use_by_period = [0_i128; 2];
            use_by_period[a] += 1_000;
            use_by_period[b] -= 1_000;
            if use_by_period.iter().any(|&use_| use_ > 0 || -use_ > 0) {
                continue;
            }
            feasible += 1;
            optimum = optimum.max(6_000 * factors[a] + 4_000 * factors[b]);
        }
    }
    assert_eq!((feasible, optimum), (2, 100_000));
    let p = reschedule(&m, &start, &[10, 5], 8).unwrap();
    assert_eq!((p.search.before, p.search.after), (50_000, optimum));
    assert_eq!((p.search.moves, p.search.pair_moves), (1, 1));
    assert_eq!(p.checked.used, [vec![0, 0], vec![0, 0]]);
    assert_eq!(
        candidate::write_solution(&p.schedule),
        "0 0 0 0.5\n0 1 0 0.5\n1 0 0 0.5\n1 1 0 0.5\n"
    );
    assert_eq!(reschedule(&m, &start, &[10, 5], 8).unwrap(), p);
    assert_eq!(reschedule(&m, &start, &[10, 5], 0).unwrap().schedule, start);
}

#[test]
fn signed_credit_cannot_leave_a_blend_it_supports_but_can_delay_when_spare() {
    let mut m = model(
        &[[-4, -2], [12, 12]],
        &[vec![[0, -4], [2, 2]]],
        &[vec![0, 0]],
        2,
    );
    let start = candidate::parse_solution("0 0 0 0.5\n0 1 0 0.5\n1 0 0 1\n", &m, 3).unwrap();
    let p = reschedule(&m, &start, &[2, 1], 8).unwrap();
    assert_eq!(p.search.before, 18_000);
    assert_eq!(p.schedule, start); // Removing -2 leaves +2 above the old zero cap.
    assert_eq!(p.search.moves, 0);
    m.uses[0][2] = 0;
    m.uses[0][3] = 0;
    let p = reschedule(&m, &start, &[2, 1], 8).unwrap();
    assert_eq!((p.search.before, p.search.after), (18_000, 21_000));
    assert_eq!(p.checked.used, [vec![0, -2_000]]);
    assert_eq!(
        candidate::write_solution(&p.schedule),
        "0 0 1 0.5\n0 1 1 0.5\n1 0 0 1\n"
    );
}

#[test]
fn joint_precedence_moves_preserve_split_routes_and_receiving_capacity() {
    let mut m = model(&[[10, 10], [-4, -4]], &[], &[], 3);
    m.requires[0] = vec![1];
    let start = candidate::parse_solution("0 0 1 0.5\n0 1 1 0.5\n1 0 1 1\n", &m, 3).unwrap();
    let p = reschedule(&m, &start, &[3, 2, 1], 8).unwrap();
    // Neither block can move profitably alone. Their exact joint value can
    // advance without separating the profitable block from its support.
    assert_eq!((p.search.before, p.search.after), (12_000, 18_000));
    assert_eq!((p.search.moves, p.search.pair_moves), (1, 1));
    assert_eq!(
        candidate::write_solution(&p.schedule),
        "0 0 0 0.5\n0 1 0 0.5\n1 0 0 1\n"
    );
    m.requires[1] = vec![0];
    assert_eq!(
        reschedule(&m, &start, &[3, 2, 1], 8).unwrap().schedule,
        p.schedule
    );

    let m = model(&[[0, 10]], &[vec![[0, 2]]], &[vec![0, 1]], 2);
    let start = candidate::parse_solution("0 0 1 0.5\n0 1 1 0.5\n", &m, 3).unwrap();
    assert_eq!(reschedule(&m, &start, &[2, 1], 8).unwrap().schedule, start);
}

#[test]
fn paired_cycles_respect_external_predecessors_and_dependants() {
    let mut m = model(
        &[[0, 0], [0, 10], [0, 8], [0, 0]],
        &[
            vec![[1, 1], [0, 0], [0, 0], [0, 0]],
            vec![[0, 0], [0, 0], [0, 0], [1, 1]],
        ],
        &[vec![0, 1, 0], vec![0, 1, 0]],
        3,
    );
    m.requires[1] = vec![0, 2];
    m.requires[2] = vec![1];
    m.requires[3] = vec![2];
    let start = candidate::parse_solution(
        "0 0 1 1\n1 0 1 0.5\n1 1 1 0.5\n2 0 1 0.5\n2 1 1 0.5\n3 0 1 1\n",
        &m,
        3,
    )
    .unwrap();
    // Blocks 0 and 3 are pinned by separate period-specific capacity rows.
    // The positive cycle cannot advance past 0 even when moved atomically.
    let p = reschedule(&m, &start, &[3, 2, 1], 8).unwrap();
    assert_eq!(p.schedule, start);
    assert_eq!(p.search.moves, 0);
    // The same pair with negative values cannot defer past dependant 3.
    m.values[3] = -10;
    m.values[5] = -8;
    let p = reschedule(&m, &start, &[3, 2, 1], 8).unwrap();
    assert_eq!(p.schedule, start);
    assert_eq!(p.search.moves, 0);
}

#[test]
fn pair_removal_checks_signed_credits_in_both_original_periods() {
    let m = model(
        &[[0, 10], [0, 8], [0, 0], [0, 0]],
        &[
            vec![[0, 2], [0, -2], [-1, -1], [1, 1]],
            vec![[0, 0], [0, 0], [1, 1], [0, 0]],
            vec![[0, 0], [0, 0], [0, 0], [1, 1]],
        ],
        &[vec![0, 0, 0], vec![0, 1, 0], vec![0, 0, 1]],
        3,
    );
    let start = candidate::parse_solution(
        "0 0 1 0.5\n0 1 1 0.5\n1 0 2 0.5\n1 1 2 0.5\n2 0 1 1\n3 0 2 1\n",
        &m,
        3,
    )
    .unwrap();
    // The profitable pair 0+1 balances at its destination, but its removed
    // negative credit would leave block 3 above the old period's zero cap.
    // Its existing partners 2 and 3 cannot move because their own rows pin them.
    let p = reschedule(&m, &start, &[3, 2, 1], 8).unwrap();
    assert_eq!(p.schedule, start);
    assert_eq!(p.search.pair_moves, 0);
    assert_eq!(p.checked.used[0], [0, 0, 0]);
}

#[test]
fn period_search_bounds_work_and_retains_every_fully_checked_improvement() {
    let m = model(&[[0, 10]], &[vec![[0, 2]]], &[vec![1, 1, 1]], 3);
    let start = candidate::parse_solution("0 0 2 0.5\n0 1 2 0.5\n", &m, 3).unwrap();
    let stopped = reschedule_bounded(&m, &start, &[3, 2, 1], 8, 0).unwrap();
    assert_eq!(stopped.schedule, start);
    assert!(stopped.search.work_exhausted);
    assert_eq!((stopped.search.work, stopped.search.trials), (0, 0));
    // One block scan and one target (including its resource row) fit. The
    // complete feasible move is retained when the next trial meets the cap.
    let partial = reschedule_bounded(&m, &start, &[3, 2, 1], 8, 3).unwrap();
    assert_eq!((partial.search.work, partial.search.trials), (3, 1));
    assert!(partial.search.work_exhausted && partial.checked.feasible());
    assert_eq!(
        (partial.search.before, partial.search.after),
        (5_000, 15_000)
    );
    assert_eq!(partial.search.moves, 1);
}

#[test]
fn pair_budget_exhaustion_never_applies_half_of_a_blend() {
    let m = model(
        &[[0, 12], [0, 8]],
        &[vec![[0, 2], [0, -2]], vec![[0, -2], [0, 2]]],
        &[vec![0, 0, 0], vec![0, 0, 0]],
        3,
    );
    let start =
        candidate::parse_solution("0 0 2 0.5\n0 1 2 0.5\n1 0 2 0.5\n1 1 2 0.5\n", &m, 3).unwrap();
    for budget in 0..=32 {
        let p = reschedule_bounded(&m, &start, &[3, 2, 1], 8, budget).unwrap();
        let period = p.schedule.assignments()[0].period;
        assert!(p.schedule.assignments().iter().all(|a| a.period == period));
        assert!(p.checked.feasible() && p.search.after >= p.search.before);
        assert!(p.search.work <= budget && p.search.pair_moves <= 1);
    }
    // Four single-block period trials, the pair scan and its first target
    // fit; stopping before the second target retains the fully checked pair.
    let p = reschedule_bounded(&m, &start, &[3, 2, 1], 8, 22).unwrap();
    assert_eq!((p.search.work, p.search.trials), (22, 5));
    assert!(p.search.work_exhausted);
    assert_eq!((p.search.moves, p.search.pair_moves), (1, 1));
    assert_eq!((p.search.before, p.search.after), (10_000, 30_000));
}

#[test]
fn period_pair_scan_is_bounded_in_mined_id_order() {
    let count = PERIOD_PAIR_SCAN + 2;
    let mut values = vec![[0, 0]; count];
    values[0] = [0, 12];
    values[count - 1] = [0, 8];
    let mut row = vec![[0, 0]; count];
    row[0] = [0, 2];
    row[count - 1] = [0, -2];
    let opposite = row.iter().map(|v| [-v[0], -v[1]]).collect();
    let m = model(&values, &[row, opposite], &[vec![0, 0], vec![0, 0]], 2);
    let shares = (0..count)
        .flat_map(|b| {
            (0..2).map(move |d| candidate::DestinationShare {
                block: b as u32,
                destination: d,
                period: 1,
                fraction: 5,
            })
        })
        .collect();
    let start = candidate::SplitSchedule::new(&m, shares, 1).unwrap();
    let p = reschedule(&m, &start, &[2, 1], 8).unwrap();
    assert_eq!(p.schedule, start);
    assert_eq!(p.search.pair_moves, 0);
    assert!(!p.search.work_exhausted && p.search.work <= PERIOD_WORK);
}

#[test]
fn fixed_fraction_period_search_matches_independent_small_model_constraints() {
    let mut cases = 0;
    let mut improvements = 0;
    for seed in 0..48_i64 {
        let values: Vec<_> = (0..3)
            .map(|b| [(seed + b * 3) % 11 - 5, (seed * 3 + b * 5) % 17 - 6])
            .collect();
        let uses: Vec<Vec<_>> = (0..2)
            .map(|r| {
                (0..3)
                    .map(|b| [(seed + b + r) % 5 - 2, (seed * 3 + b * 2 + r) % 7 - 3])
                    .collect()
            })
            .collect();
        let caps = vec![vec![1, 1, 2], vec![0, 1, 1]];
        let mut m = model(&values, &uses, &caps, 3);
        if seed % 3 == 0 {
            m.requires[2] = vec![0];
        }
        let fractions = [1 + seed % 9, 1 + (seed * 3) % 9, 1 + (seed * 7) % 9];
        let factors = [10_i64, 7, 4];
        // Independent arithmetic on the declared values and signed uses. No
        // candidate checker or search helper contributes to this oracle.
        let oracle = |periods: &[usize]| -> Option<i128> {
            let mut resource = vec![vec![0_i64; 3]; 2];
            let mut value = 0_i128;
            for b in 0..3 {
                if m.requires[b]
                    .iter()
                    .any(|&q| periods[q as usize] > periods[b])
                {
                    return None;
                }
                let shares = [10 - fractions[b], fractions[b]];
                for (d, &part) in shares.iter().enumerate() {
                    value += i128::from(values[b][d] * part * factors[periods[b]]);
                    for r in 0..2 {
                        resource[r][periods[b]] += uses[r][b][d] * part;
                    }
                }
            }
            if (0..2).any(|r| (0..3).any(|t| resource[r][t] > caps[r][t] * 10)) {
                None
            } else {
                Some(value)
            }
        };
        let feasible: Vec<_> = (0..27)
            .filter_map(|i| {
                let periods = vec![i % 3, (i / 3) % 3, i / 9];
                oracle(&periods).map(|v| (periods, v))
            })
            .collect();
        if feasible.is_empty() {
            continue;
        }
        let (periods, before) = &feasible[seed as usize % feasible.len()];
        let assignments = (0..3)
            .flat_map(|b| {
                (0..2).map(move |d| candidate::DestinationShare {
                    block: b as u32,
                    destination: d as u32,
                    period: periods[b] as u32,
                    fraction: if d == 0 {
                        10 - fractions[b]
                    } else {
                        fractions[b]
                    },
                })
            })
            .collect();
        let start = candidate::SplitSchedule::new(&m, assignments, 1).unwrap();
        let p = reschedule(&m, &start, &factors, 100).unwrap();
        let mut after_periods = vec![0; 3];
        for a in p.schedule.assignments() {
            after_periods[a.block as usize] = a.period as usize;
        }
        assert_eq!(p.search.before, *before);
        assert_eq!(oracle(&after_periods), Some(p.search.after));
        assert!(
            p.search.after >= *before
                && p.search.after <= feasible.iter().map(|(_, v)| *v).max().unwrap()
        );
        assert!(p.checked.feasible() && !p.search.work_exhausted && p.search.work <= PERIOD_WORK);
        let routes = |s: &candidate::SplitSchedule| {
            s.assignments()
                .iter()
                .map(|a| (a.block, a.destination, a.fraction))
                .collect::<Vec<_>>()
        };
        assert_eq!(routes(&p.schedule), routes(&start));
        for b in 0..3 {
            for t in 0..3 {
                let mut neighbor = after_periods.clone();
                neighbor[b] = t;
                assert!(
                    oracle(&neighbor).is_none_or(|v| v <= p.search.after),
                    "seed {seed}"
                );
                for c in b + 1..3 {
                    neighbor[c] = t;
                    assert!(
                        oracle(&neighbor).is_none_or(|v| v <= p.search.after),
                        "pair {b}/{c}, seed {seed}"
                    );
                    neighbor[c] = after_periods[c];
                }
            }
        }
        improvements += usize::from(p.search.after > *before);
        cases += 1;
    }
    assert!(
        cases >= 24 && improvements >= 8,
        "{cases} cases, {improvements} improvements"
    );
}

#[test]
fn fixed_fraction_period_search_refuses_bad_inputs_without_normalizing_them() {
    let m = model(&[[0, 10]], &[vec![[0, 2]]], &[vec![1, 1]], 2);
    let s = candidate::parse_solution("0 0 1 0.5\n0 1 1 0.5\n", &m, 3).unwrap();
    for factors in [vec![], vec![1], vec![1, -1]] {
        assert!(reschedule(&m, &s, &factors, 8).is_err());
    }
    assert!(reschedule(&m, &s, &[2, 1], 1_001).is_err());
    let incomplete = candidate::parse_solution("0 0 1 0.4\n0 1 1 0.5\n", &m, 3).unwrap();
    assert!(
        reschedule(&m, &incomplete, &[2, 1], 8)
            .unwrap_err()
            .what
            .contains("infeasible")
    );
    let exceeded = candidate::parse_solution("0 0 1 0.4\n0 1 1 0.6\n", &m, 3).unwrap();
    assert!(
        reschedule(&m, &exceeded, &[2, 1], 8)
            .unwrap_err()
            .what
            .contains("infeasible")
    );
    let huge = model(&[[i64::MAX, i64::MAX]], &[], &[], 2);
    let s = candidate::parse_solution("0 0 0 0.5\n0 1 0 0.5\n", &huge, 9).unwrap();
    assert!(
        reschedule(&huge, &s, &[i64::MAX, 1], 8)
            .unwrap_err()
            .what
            .contains("overflow")
    );
}
