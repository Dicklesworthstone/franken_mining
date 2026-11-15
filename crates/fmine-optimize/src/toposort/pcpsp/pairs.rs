//! DESIGN: atomic two-block moves for complementary signed blends (BD-306/319).
//! Each pair enters one common period; its routes are chosen jointly. The exact
//! change is checked in all affected periods, including any blending credits
//! released from the old periods. No temporary infeasible state is admitted.
//! Determinism: BitwisePortable (integer comparisons and fixed visitation order).

use super::{PAIR_SCAN, PAIR_WORK, PcpspModel};

pub(super) fn improve(
    model: &PcpspModel,
    schedule: &mut [Option<u32>],
    destination: &mut [u32],
    factors: &[i64],
    used: &mut [Vec<i128>],
    dependents: (&[usize], &[u32]),
) -> usize {
    improve_bounded(
        model,
        schedule,
        destination,
        factors,
        used,
        dependents,
        PAIR_WORK,
    )
}

/// The caller has checked dimensions, the initial schedule, and nonnegative
/// nonincreasing factors. Resource totals and reverse adjacency are reused from
/// the single-block pass; a stalled pass has not changed either. Work exhaustion
/// keeps all admitted improvements.
fn improve_bounded(
    model: &PcpspModel,
    schedule: &mut [Option<u32>],
    destination: &mut [u32],
    factors: &[i64],
    used: &mut [Vec<i128>],
    dependents: (&[usize], &[u32]),
    mut remaining: usize,
) -> usize {
    if model.blocks() < 2 || remaining == 0 {
        return 0;
    }
    let (offsets, dependents) = dependents;
    let dn = model.destinations();
    let mut moved = 0;
    'anchors: for b in 0..model.blocks() {
        for c in b + 1..model.blocks().min(b.saturating_add(PAIR_SCAN + 1)) {
            if !spend(&mut remaining) {
                break 'anchors;
            }
            let pair = [b, c];
            let Some((first, last)) = period_bounds(model, schedule, offsets, dependents, pair)
            else {
                continue;
            };
            let old = [schedule[b], schedule[c]];
            let old_columns = [
                b * dn + destination[b] as usize,
                c * dn + destination[c] as usize,
            ];
            let old_value: i128 = old
                .iter()
                .zip(old_columns)
                .filter_map(|(&t, column)| {
                    t.map(|t| i128::from(model.values[column]) * i128::from(factors[t as usize]))
                })
                .sum();
            let max_value: i128 = pair
                .iter()
                .map(|&q| {
                    i128::from(
                        model.values[q * dn..(q + 1) * dn]
                            .iter()
                            .copied()
                            .max()
                            .unwrap_or(0),
                    )
                })
                .sum();
            let mut best_value = old_value;
            let mut best = None;
            'periods: for (t, &factor) in factors.iter().enumerate().take(last + 1).skip(first) {
                if !spend(&mut remaining) {
                    break;
                }
                if max_value * i128::from(factor) <= best_value {
                    // A nonnegative upper value only falls as time advances.
                    if max_value >= 0 {
                        break;
                    }
                    continue;
                }
                for db in 0..dn {
                    for dc in 0..dn {
                        if !spend(&mut remaining) {
                            break 'periods;
                        }
                        let columns = [b * dn + db, c * dn + dc];
                        let value = (i128::from(model.values[columns[0]])
                            + i128::from(model.values[columns[1]]))
                            * i128::from(factor);
                        if value > best_value && fits(model, used, old, old_columns, t, columns) {
                            best_value = value;
                            best = Some((t, columns, [db, dc]));
                        }
                    }
                }
            }
            if let Some((t, columns, routes)) = best {
                for (r, row) in used.iter_mut().enumerate() {
                    for i in 0..2 {
                        if let Some(previous) = old[i] {
                            row[previous as usize] -= i128::from(model.uses[r][old_columns[i]]);
                        }
                        row[t] += i128::from(model.uses[r][columns[i]]);
                    }
                }
                for i in 0..2 {
                    schedule[pair[i]] = Some(t as u32);
                    destination[pair[i]] = routes[i] as u32;
                }
                moved += 2;
            }
            if remaining == 0 {
                break 'anchors;
            }
        }
    }
    moved
}

fn spend(remaining: &mut usize) -> bool {
    if *remaining == 0 {
        false
    } else {
        *remaining -= 1;
        true
    }
}

fn period_bounds(
    model: &PcpspModel,
    schedule: &[Option<u32>],
    offsets: &[usize],
    dependents: &[u32],
    pair: [usize; 2],
) -> Option<(usize, usize)> {
    let mut first = 0;
    let mut last = model.periods() - 1;
    for b in pair {
        for &q in &model.requires[b] {
            let q = q as usize;
            if !pair.contains(&q) {
                first = first.max(schedule[q]? as usize);
            }
        }
        for &q in &dependents[offsets[b]..offsets[b + 1]] {
            let q = q as usize;
            if !pair.contains(&q)
                && let Some(t) = schedule[q]
            {
                last = last.min(t as usize);
            }
        }
    }
    (first <= last).then_some((first, last))
}

fn fits(
    model: &PcpspModel,
    used: &[Vec<i128>],
    old: [Option<u32>; 2],
    old_columns: [usize; 2],
    target: usize,
    columns: [usize; 2],
) -> bool {
    let affected = [
        old[0].map(|t| t as usize),
        old[1].map(|t| t as usize),
        Some(target),
    ];
    for (i, period) in affected.iter().enumerate() {
        let Some(t) = *period else { continue };
        if affected[..i].contains(period) {
            continue;
        }
        for (r, row) in used.iter().enumerate() {
            let mut total = row[t];
            for j in 0..2 {
                if old[j] == Some(t as u32) {
                    total -= i128::from(model.uses[r][old_columns[j]]);
                }
                if t == target {
                    total += i128::from(model.uses[r][columns[j]]);
                }
            }
            if total > i128::from(model.limits[r][t]) {
                return false;
            }
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::toposort::check;
    use crate::toposort::pcpsp::tests::{complementary_blend_model, factors};

    #[test]
    fn work_limit_never_partially_applies_a_pair() {
        let model = complementary_blend_model();
        let factors = factors(2);
        for budget in 0..16 {
            let mut schedule = vec![None, None];
            let mut destinations = vec![0, 0];
            let problem = model.with_destinations(&destinations).unwrap();
            let mut used = check(&problem, &schedule).unwrap().used;
            let (offsets, dependents) = crate::toposort::dependents(problem.blocks()).unwrap();
            let moved = improve_bounded(
                &model,
                &mut schedule,
                &mut destinations,
                &factors,
                &mut used,
                (&offsets, &dependents),
                budget,
            );
            assert!(matches!(moved, 0 | 2));
            assert_eq!(schedule[0], schedule[1]);
            let checked =
                check(&model.with_destinations(&destinations).unwrap(), &schedule).unwrap();
            assert!(checked.feasible());
        }
    }
}
