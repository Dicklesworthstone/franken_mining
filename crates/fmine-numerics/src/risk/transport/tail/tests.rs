//! Independent transport-polytope cuts and fractional-tail integration, not threshold search.
//! Determinism: Exact

use super::*;

fn r(n: i128, d: i128) -> Rat {
    Rat::new(n, d).unwrap()
}
fn sum(values: impl Iterator<Item = Rat>) -> Rat {
    values.fold(ZERO, |a, b| a.checked_add(b).unwrap())
}
fn metric(points: &[i128]) -> Vec<Vec<Rat>> {
    points
        .iter()
        .map(|a| points.iter().map(|b| r((a - b).abs(), 1)).collect())
        .collect()
}
fn tail(losses: &[Rat], weights: &[Rat], alpha: Rat) -> Rat {
    let mut order: Vec<_> = (0..losses.len()).collect();
    order.sort_by(|&a, &b| compare(losses[b], losses[a]));
    let mut left = alpha;
    let mut total = ZERO;
    for i in order {
        let take = if compare(left, weights[i]).is_lt() {
            left
        } else {
            weights[i]
        };
        left = left.checked_sub(take).unwrap();
        total = total
            .checked_add(take.checked_mul(losses[i]).unwrap())
            .unwrap();
    }
    assert_eq!(left, ZERO);
    total.checked_div(alpha).unwrap()
}

fn witness(plan: &TransportPlan, losses: &[Rat], c: &CvarCertificate) {
    let h = &c.hinge_certificate;
    let n = losses.len();
    let mut sources = vec![ZERO; n];
    let mut targets = vec![ZERO; n];
    let mut cost = ZERO;
    for a in &h.arcs {
        assert!(a.mass.signum() > 0);
        sources[a.source] = sources[a.source].checked_add(a.mass).unwrap();
        targets[a.target] = targets[a.target].checked_add(a.mass).unwrap();
        cost = cost
            .checked_add(a.mass.checked_mul(plan.costs[a.source][a.target]).unwrap())
            .unwrap();
    }
    assert_eq!(sources, plan.weights);
    assert_eq!(targets, h.destination_weights);
    assert_eq!(cost, h.transport_cost);
    assert!(!compare(cost, h.radius).is_gt());
    assert_eq!(sum(c.tail_masses.iter().copied()), c.tail_mass);
    for (mass, probability) in c.tail_masses.iter().zip(&targets) {
        assert!(mass.signum() >= 0 && !compare(*mass, *probability).is_gt());
    }
    let primal = sum(c
        .tail_masses
        .iter()
        .zip(losses)
        .map(|(p, l)| p.checked_mul(*l).unwrap()))
    .checked_div(c.tail_mass)
    .unwrap();
    assert_eq!(primal, c.worst_case_cvar);
    assert_eq!(primal, tail(losses, &targets, c.tail_mass));
    assert!(h.dual_lambda.signum() >= 0);
    for (i, costs) in plan.costs.iter().enumerate() {
        for (j, d) in costs.iter().enumerate() {
            let excess = losses[j].checked_sub(c.threshold).unwrap();
            let hinge = if excess.signum() > 0 { excess } else { ZERO };
            let bound = h.dual_potentials[i]
                .checked_add(h.dual_lambda.checked_mul(*d).unwrap())
                .unwrap();
            assert!(!compare(hinge, bound).is_gt());
        }
    }
    let dual = sum(plan
        .weights
        .iter()
        .zip(&h.dual_potentials)
        .map(|(p, z)| p.checked_mul(*z).unwrap()))
    .checked_add(h.radius.checked_mul(h.dual_lambda).unwrap())
    .unwrap()
    .checked_div(c.tail_mass)
    .unwrap()
    .checked_add(c.threshold)
    .unwrap();
    assert_eq!(dual, primal);
    assert_eq!(dual, c.dual_value);
    assert_eq!(c.duality_gap, ZERO);
    assert!((1..=MAX_CVAR_ORACLE_CALLS).contains(&c.oracle_calls));
}

#[test]
fn exact_tail_known_answers_include_zero_radius_saturation_gains_and_fractional_boundary() {
    let plan = TransportPlan::new(&[r(3, 4), r(1, 4)], &metric(&[0, 2])).unwrap();
    for losses in [[r(-20, 1), r(12, 1)], [r(-20, 1), r(-8, 1)], [r(7, 1); 2]] {
        for radius in [ZERO, r(1, 4), r(1, 2), r(3, 2), r(100, 1)] {
            let moved = radius.checked_div(r(2, 1)).unwrap();
            let high = if compare(moved, r(3, 4)).is_gt() {
                ONE
            } else {
                r(1, 4).checked_add(moved).unwrap()
            };
            for alpha in [r(1, 10), r(1, 2), ONE] {
                let result = plan.maximize_cvar(&losses, radius, alpha).unwrap();
                assert_eq!(
                    result.worst_case_cvar,
                    tail(&losses, &[ONE.checked_sub(high).unwrap(), high], alpha)
                );
                witness(&plan, &losses, &result);
            }
        }
    }
    for alpha in [ZERO, r(-1, 1), r(2, 1)] {
        assert!(plan.maximize_cvar(&[ZERO; 2], ZERO, alpha).is_err());
    }
    assert!(plan.maximize_cvar(&[ZERO], ZERO, ONE).is_err());
    assert!(plan.maximize_cvar(&[ZERO; 2], r(-1, 1), ONE).is_err());
}

// Original transport vertices: pure row assignments and budget-cut edges with at most one split row.
fn vertices(p: &[Rat], costs: &[Vec<Rat>], radius: Rat) -> Vec<Vec<Rat>> {
    let n = p.len();
    let mut out = Vec::new();
    for code in 0..n.pow(n as u32) {
        let mut code = code;
        let dest: Vec<_> = (0..n)
            .map(|_| {
                let j = code % n;
                code /= n;
                j
            })
            .collect();
        let cost = sum((0..n).map(|i| p[i].checked_mul(costs[i][dest[i]]).unwrap()));
        let mut q = vec![ZERO; n];
        for i in 0..n {
            q[dest[i]] = q[dest[i]].checked_add(p[i]).unwrap();
        }
        if !compare(cost, radius).is_gt() {
            out.push(q.clone());
        }
        for i in 0..n {
            if dest[i] != 0 {
                continue;
            }
            let others = cost
                .checked_sub(p[i].checked_mul(costs[i][0]).unwrap())
                .unwrap();
            for a in 0..n {
                for b in a + 1..n {
                    let step = costs[i][a].checked_sub(costs[i][b]).unwrap();
                    if step == ZERO {
                        continue;
                    }
                    let part = radius
                        .checked_sub(others)
                        .unwrap()
                        .checked_sub(p[i].checked_mul(costs[i][b]).unwrap())
                        .unwrap()
                        .checked_div(step)
                        .unwrap();
                    if part.signum() < 0 || compare(part, p[i]).is_gt() {
                        continue;
                    }
                    let mut row = q.clone();
                    row[0] = row[0].checked_sub(p[i]).unwrap();
                    row[a] = row[a].checked_add(part).unwrap();
                    row[b] = row[b].checked_add(p[i].checked_sub(part).unwrap()).unwrap();
                    out.push(row);
                }
            }
        }
    }
    out
}

// CVaR is linear within each cumulative-mass region. Cut original edges at every tail-mass
// boundary. Taking all vertex pairs includes all edges; extra pairs are feasible and harmless.
fn oracle(losses: &[Rat], laws: &[Vec<Rat>], alpha: Rat) -> Rat {
    let mut best = laws
        .iter()
        .map(|q| tail(losses, q, alpha))
        .max_by(|a, b| compare(*a, *b))
        .unwrap();
    for a in laws {
        for b in laws {
            for threshold in losses {
                let mass = |q: &[Rat]| {
                    sum(losses
                        .iter()
                        .zip(q)
                        .filter(|(l, _)| compare(**l, *threshold).is_gt())
                        .map(|(_, p)| *p))
                };
                let (ma, mb) = (mass(a), mass(b));
                if ma == mb {
                    continue;
                }
                let weight = alpha
                    .checked_sub(mb)
                    .unwrap()
                    .checked_div(ma.checked_sub(mb).unwrap())
                    .unwrap();
                if weight.signum() < 0 || compare(weight, ONE).is_gt() {
                    continue;
                }
                let rest = ONE.checked_sub(weight).unwrap();
                let q: Vec<_> = a
                    .iter()
                    .zip(b)
                    .map(|(x, y)| {
                        x.checked_mul(weight)
                            .unwrap()
                            .checked_add(y.checked_mul(rest).unwrap())
                            .unwrap()
                    })
                    .collect();
                let value = tail(losses, &q, alpha);
                if compare(value, best).is_gt() {
                    best = value;
                }
            }
        }
    }
    best
}

#[test]
fn three_state_tail_polytope_cuts_match_certified_threshold_search() {
    let p = [r(1, 4), r(1, 4), r(1, 2)];
    for points in [[0, 1, 3], [0, 2, 3]] {
        let costs = metric(&points);
        let plan = TransportPlan::new(&p, &costs).unwrap();
        for radius in [r(1, 8), r(1, 2), r(3, 4)] {
            let laws = vertices(&p, &costs, radius);
            for losses in [
                [r(13, 1), r(10, 1), ZERO],
                [r(-4, 1), r(7, 1), r(2, 1)],
                [r(5, 1), r(-3, 1), r(12, 1)],
            ] {
                for alpha in [r(1, 4), r(1, 2), r(2, 3), r(3, 4)] {
                    let result = plan.maximize_cvar(&losses, radius, alpha).unwrap();
                    assert_eq!(result.worst_case_cvar, oracle(&losses, &laws, alpha));
                    witness(&plan, &losses, &result);
                }
            }
        }
    }
    let plan = TransportPlan::new(&p, &metric(&[0, 1, 3])).unwrap();
    let c = plan
        .maximize_cvar(&[r(13, 1), r(10, 1), ZERO], r(1, 2), r(2, 3))
        .unwrap();
    assert_eq!(c.worst_case_cvar, r(95, 8));
    assert_eq!(c.threshold, r(4, 1)); // The minimax threshold need not be an observed loss.
}
