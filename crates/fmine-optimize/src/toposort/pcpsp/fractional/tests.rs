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
