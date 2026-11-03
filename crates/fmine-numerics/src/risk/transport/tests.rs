//! DESIGN: independent finite transport vertices and exact primal/dual known answers.
//! Determinism: Exact (exhaustive rational cases; no floating oracle or randomized tolerances).

use super::*;

fn r(num: i128, den: i128) -> Rat {
    Rat::new(num, den).unwrap()
}

fn z(value: i128) -> Rat {
    Rat::from_integer(value)
}

fn line_metric(points: &[i128]) -> Vec<Vec<Rat>> {
    points
        .iter()
        .map(|a| points.iter().map(|b| z((a - b).abs())).collect())
        .collect()
}

fn sum(values: impl Iterator<Item = Rat>) -> Rat {
    values.fold(ZERO, |total, value| total.checked_add(value).unwrap())
}

fn check(plan: &TransportPlan, values: &[Rat], certificate: &TransportCertificate) {
    let n = plan.weights.len();
    assert_eq!(certificate.destination_weights.len(), n);
    assert!(certificate.arcs.len() <= n + 1);
    assert!(
        certificate
            .arcs
            .windows(2)
            .all(|p| (p[0].source, p[0].target) < (p[1].source, p[1].target))
    );
    let (mut rows, mut columns) = (vec![ZERO; n], vec![ZERO; n]);
    for arc in &certificate.arcs {
        assert!(arc.mass.signum() > 0);
        rows[arc.source] = rows[arc.source].checked_add(arc.mass).unwrap();
        columns[arc.target] = columns[arc.target].checked_add(arc.mass).unwrap();
    }
    assert_eq!(rows, plan.weights);
    assert_eq!(columns, certificate.destination_weights);
    assert_eq!(sum(columns.iter().copied()), ONE);
    let cost = sum(certificate.arcs.iter().map(|arc| {
        arc.mass
            .checked_mul(plan.costs[arc.source][arc.target])
            .unwrap()
    }));
    let value = sum(columns
        .iter()
        .zip(values)
        .map(|(mass, value)| mass.checked_mul(*value).unwrap()));
    assert_eq!(cost, certificate.transport_cost);
    assert!(!compare(cost, certificate.radius).is_gt());
    assert_eq!(value, certificate.worst_case_value);
    assert!(certificate.dual_lambda.signum() >= 0);
    for (i, row) in plan.costs.iter().enumerate() {
        for (j, cost) in row.iter().enumerate() {
            let bound = certificate.dual_potentials[i]
                .checked_add(certificate.dual_lambda.checked_mul(*cost).unwrap())
                .unwrap();
            assert!(!compare(values[j], bound).is_gt());
        }
    }
    let dual = sum(plan
        .weights
        .iter()
        .zip(&certificate.dual_potentials)
        .map(|(p, s)| p.checked_mul(*s).unwrap()))
    .checked_add(
        certificate
            .dual_lambda
            .checked_mul(certificate.radius)
            .unwrap(),
    )
    .unwrap();
    assert_eq!(dual, value);
    assert_eq!(certificate.dual_value, dual);
    assert_eq!(certificate.duality_gap, ZERO);
    assert_eq!(
        certificate
            .dual_lambda
            .checked_mul(certificate.radius.checked_sub(cost).unwrap())
            .unwrap(),
        ZERO
    );
}

// A product of row simplexes intersected with one budget halfspace has only deterministic vertices
// and budget-cut edges. Every edge splits exactly one row between two destinations. This enumerates
// those primal vertices directly, without hull construction, marginal slopes or dual minimization.
fn vertex_oracle(weights: &[Rat], costs: &[Vec<Rat>], values: &[Rat], radius: Rat) -> Rat {
    let n = weights.len();
    let mut best = None;
    let mut consider = |value| {
        if best.is_none_or(|old| compare(value, old).is_gt()) {
            best = Some(value);
        }
    };
    for code in 0..n.pow(n as u32) {
        let mut code = code;
        let assignment: Vec<_> = (0..n)
            .map(|_| {
                let target = code % n;
                code /= n;
                target
            })
            .collect();
        let cost = sum((0..n).map(|i| weights[i].checked_mul(costs[i][assignment[i]]).unwrap()));
        let value = sum((0..n).map(|i| weights[i].checked_mul(values[assignment[i]]).unwrap()));
        if !compare(cost, radius).is_gt() {
            consider(value);
        }
        for k in 0..n {
            if assignment[k] != 0 {
                continue;
            }
            let other_cost = cost
                .checked_sub(weights[k].checked_mul(costs[k][0]).unwrap())
                .unwrap();
            let other_value = value
                .checked_sub(weights[k].checked_mul(values[0]).unwrap())
                .unwrap();
            for a in 0..n {
                for b in a + 1..n {
                    let step = costs[k][a].checked_sub(costs[k][b]).unwrap();
                    if step == ZERO {
                        continue;
                    }
                    let mass = radius
                        .checked_sub(other_cost)
                        .unwrap()
                        .checked_sub(weights[k].checked_mul(costs[k][b]).unwrap())
                        .unwrap()
                        .checked_div(step)
                        .unwrap();
                    if mass.signum() >= 0 && !compare(mass, weights[k]).is_gt() {
                        let candidate = other_value
                            .checked_add(mass.checked_mul(values[a]).unwrap())
                            .unwrap()
                            .checked_add(
                                weights[k]
                                    .checked_sub(mass)
                                    .unwrap()
                                    .checked_mul(values[b])
                                    .unwrap(),
                            )
                            .unwrap();
                        consider(candidate);
                    }
                }
            }
        }
    }
    best.unwrap()
}

#[test]
fn two_state_loss_has_exact_zero_fractional_and_saturated_witnesses() {
    let plan = TransportPlan::new(&[r(1, 5), r(4, 5)], &line_metric(&[0, 1])).unwrap();
    let values = [z(4), z(-6)];
    for (radius, objective, first_mass, lambda, cost) in [
        (ZERO, z(-4), r(1, 5), z(10), ZERO),
        (r(1, 4), r(-3, 2), r(9, 20), z(10), r(1, 4)),
        (r(4, 5), z(4), ONE, ZERO, r(4, 5)),
        (z(7), z(4), ONE, ZERO, r(4, 5)),
    ] {
        let result = plan.maximize(&values, radius).unwrap();
        assert_eq!(result.worst_case_value, objective);
        assert_eq!(
            result.destination_weights,
            [first_mass, ONE.checked_sub(first_mass).unwrap()]
        );
        assert_eq!(result.dual_lambda, lambda);
        assert_eq!(result.transport_cost, cost);
        check(&plan, &values, &result);
    }
}

#[test]
fn hull_boundaries_equal_slopes_and_negative_constants_are_exact() {
    let plan = TransportPlan::new(&[r(1, 2), r(1, 3), r(1, 6)], &line_metric(&[0, 1, 3])).unwrap();
    let values = [ZERO, z(2), z(5)];
    for (radius, value) in [
        (ZERO, r(3, 2)),
        (r(1, 2), r(5, 2)),
        (ONE, r(13, 4)),
        (r(13, 6), z(5)),
    ] {
        let result = plan.maximize(&values, radius).unwrap();
        assert_eq!(result.worst_case_value, value);
        check(&plan, &values, &result);
    }
    let constant = plan.maximize(&[z(-7); 3], z(100)).unwrap();
    assert_eq!(constant.worst_case_value, z(-7));
    assert_eq!(constant.destination_weights, plan.weights);
    assert_eq!(constant.transport_cost, ZERO);
    assert_eq!(constant.dual_lambda, ZERO);
    let singleton = TransportPlan::new(&[ONE], &[vec![ZERO]]).unwrap();
    let result = singleton.maximize(&[z(-9)], z(100)).unwrap();
    assert_eq!(result.worst_case_value, z(-9));
    check(&singleton, &[z(-9)], &result);
}

#[test]
fn exhaustive_small_laws_match_independent_transport_polytope_vertices() {
    let metrics = [
        line_metric(&[0, 1, 3]),
        vec![
            vec![ZERO, ONE, ONE],
            vec![ONE, ZERO, ONE],
            vec![ONE, ONE, ZERO],
        ],
    ];
    for costs in metrics {
        for weights in [[r(1, 3); 3], [r(1, 2), r(1, 3), r(1, 6)]] {
            let plan = TransportPlan::new(&weights, &costs).unwrap();
            for code in 0..27 {
                let values: Vec<_> = (0..3)
                    .map(|i| [z(-2), ZERO, z(3)][(code / 3_usize.pow(i)) % 3])
                    .collect();
                let mut previous = None;
                for radius in [ZERO, r(1, 7), r(1, 2), ONE, z(4)] {
                    let result = plan.maximize(&values, radius).unwrap();
                    assert_eq!(
                        result.worst_case_value,
                        vertex_oracle(&weights, &costs, &values, radius)
                    );
                    assert!(
                        previous
                            .is_none_or(|value| !compare(result.worst_case_value, value).is_lt())
                    );
                    previous = Some(result.worst_case_value);
                    check(&plan, &values, &result);
                }
            }
        }
    }
    // Four destinations exercise rows with three consecutive hull segments.
    let weights = [r(1, 2), r(1, 3), r(1, 7), r(1, 42)];
    let costs = line_metric(&[0, 1, 3, 7]);
    let values = [ZERO, z(4), z(6), z(8)];
    let plan = TransportPlan::new(&weights, &costs).unwrap();
    for radius in [ZERO, r(1, 2), ONE, z(2), z(7)] {
        let result = plan.maximize(&values, radius).unwrap();
        assert_eq!(
            result.worst_case_value,
            vertex_oracle(&weights, &costs, &values, radius)
        );
        check(&plan, &values, &result);
    }
}

#[test]
fn permutation_metric_scaling_and_positive_affine_values_preserve_the_problem() {
    let weights = [r(1, 2), r(1, 3), r(1, 6)];
    let costs = line_metric(&[0, 2, 5]);
    let values = [z(-3), z(4), z(8)];
    let radius = r(5, 7);
    let plan = TransportPlan::new(&weights, &costs).unwrap();
    let base = plan.maximize(&values, radius).unwrap();
    assert_eq!(plan.maximize(&values, radius).unwrap(), base);
    let perm = [2, 0, 1];
    let perm_weights: Vec<_> = perm.iter().map(|&i| weights[i]).collect();
    let perm_values: Vec<_> = perm.iter().map(|&i| values[i]).collect();
    let perm_costs: Vec<Vec<_>> = perm
        .iter()
        .map(|&i| perm.iter().map(|&j| costs[i][j]).collect())
        .collect();
    let changed = TransportPlan::new(&perm_weights, &perm_costs)
        .unwrap()
        .maximize(&perm_values, radius)
        .unwrap();
    assert_eq!(changed.worst_case_value, base.worst_case_value);
    assert_eq!(
        changed.destination_weights,
        perm.iter()
            .map(|&i| base.destination_weights[i])
            .collect::<Vec<_>>()
    );
    let scaled_costs: Vec<Vec<_>> = costs
        .iter()
        .map(|row| {
            row.iter()
                .map(|cost| cost.checked_mul(z(7)).unwrap())
                .collect()
        })
        .collect();
    let scaled = TransportPlan::new(&weights, &scaled_costs)
        .unwrap()
        .maximize(&values, radius.checked_mul(z(7)).unwrap())
        .unwrap();
    assert_eq!(scaled.worst_case_value, base.worst_case_value);
    assert_eq!(scaled.destination_weights, base.destination_weights);
    assert_eq!(
        scaled.dual_lambda.checked_mul(z(7)).unwrap(),
        base.dual_lambda
    );
    let affine: Vec<_> = values
        .iter()
        .map(|v| v.checked_mul(z(3)).unwrap().checked_add(z(-11)).unwrap())
        .collect();
    let affine_result = plan.maximize(&affine, radius).unwrap();
    assert_eq!(
        affine_result.worst_case_value,
        base.worst_case_value
            .checked_mul(z(3))
            .unwrap()
            .checked_add(z(-11))
            .unwrap()
    );
    assert_eq!(affine_result.destination_weights, base.destination_weights);
    assert_eq!(
        affine_result.dual_lambda,
        base.dual_lambda.checked_mul(z(3)).unwrap()
    );
}

#[test]
fn malformed_laws_metrics_and_value_dimensions_refuse() {
    let metric = line_metric(&[0, 1]);
    for weights in [vec![ZERO, ONE], vec![z(-1), z(2)], vec![r(1, 3); 2]] {
        assert_eq!(
            TransportPlan::new(&weights, &metric).unwrap_err().code,
            "FMINE-REF-109"
        );
    }
    for costs in [
        vec![vec![ZERO]],
        vec![vec![ONE, ONE], vec![ONE, ZERO]],
        vec![vec![ZERO, ZERO], vec![ZERO, ZERO]],
        vec![vec![ZERO, z(-1)], vec![z(-1), ZERO]],
        vec![vec![ZERO, ONE], vec![z(2), ZERO]],
    ] {
        assert_eq!(
            TransportPlan::new(&[r(1, 2); 2], &costs).unwrap_err().code,
            "FMINE-REF-109"
        );
    }
    let broken_triangle = vec![
        vec![ZERO, ONE, z(3)],
        vec![ONE, ZERO, ONE],
        vec![z(3), ONE, ZERO],
    ];
    assert_eq!(
        TransportPlan::new(&[r(1, 3); 3], &broken_triangle)
            .unwrap_err()
            .code,
        "FMINE-REF-109"
    );
    let plan = TransportPlan::new(&[r(1, 2); 2], &metric).unwrap();
    assert_eq!(
        plan.maximize(&[ZERO], ONE).unwrap_err().code,
        "FMINE-REF-109"
    );
    assert_eq!(
        plan.maximize(&[ZERO, ONE], z(-1)).unwrap_err().code,
        "FMINE-REF-109"
    );
}

#[test]
fn bounds_extreme_metrics_and_unrepresentable_arithmetic_are_explicit() {
    assert_eq!(
        TransportPlan::new(&[], &[]).unwrap_err().code,
        "FMINE-REF-201"
    );
    assert_eq!(
        TransportPlan::new(&vec![r(1, 65); 65], &[])
            .unwrap_err()
            .code,
        "FMINE-REF-201"
    );
    let costs: Vec<Vec<_>> = (0..MAX_STATES)
        .map(|i| {
            (0..MAX_STATES)
                .map(|j| if i == j { ZERO } else { ONE })
                .collect()
        })
        .collect();
    let plan = TransportPlan::new(&vec![r(1, MAX_STATES as i128); MAX_STATES], &costs).unwrap();
    let values: Vec<_> = (0..MAX_STATES).map(|i| z(i as i128)).collect();
    let result = plan.maximize(&values, ONE).unwrap();
    assert_eq!(result.worst_case_value, z((MAX_STATES - 1) as i128));
    check(&plan, &values, &result);
    let huge = vec![vec![ZERO, z(i128::MAX)], vec![z(i128::MAX), ZERO]];
    let plan = TransportPlan::new(&[r(1, 2); 2], &huge).unwrap();
    let result = plan.maximize(&[ZERO, z(2)], z(i128::MAX)).unwrap();
    assert_eq!(result.worst_case_value, z(2));
    assert_eq!(result.transport_cost, r(i128::MAX, 2));
    let plan = TransportPlan::new(&[r(1, 2); 2], &line_metric(&[0, 1])).unwrap();
    assert_eq!(
        plan.maximize(&[z(i128::MIN), z(i128::MAX)], ONE)
            .unwrap_err()
            .code,
        "FMINE-REF-207"
    );
    let tiny = vec![vec![ZERO, r(1, i128::MAX)], vec![r(1, i128::MAX), ZERO]];
    let plan = TransportPlan::new(&[r(1, 2); 2], &tiny).unwrap();
    assert_eq!(
        plan.maximize(&[ZERO, ONE], ONE).unwrap_err().code,
        "FMINE-REF-207"
    );
}

#[test]
fn certificate_checker_rejects_infeasible_or_suboptimal_primal_witnesses() {
    let plan = TransportPlan::new(&[r(1, 2); 2], &line_metric(&[0, 1])).unwrap();
    let values = [ZERO, z(2)];
    let diagonal = vec![
        TransportArc {
            source: 0,
            target: 0,
            mass: r(1, 2),
        },
        TransportArc {
            source: 1,
            target: 1,
            mass: r(1, 2),
        },
    ];
    assert_eq!(
        plan.certify(&values, ONE, ZERO, diagonal.clone())
            .unwrap_err()
            .code,
        "FMINE-REF-207"
    );
    let mut bad_mass = diagonal.clone();
    bad_mass[0].mass = ONE;
    assert_eq!(
        plan.certify(&values, ZERO, z(2), bad_mass)
            .unwrap_err()
            .code,
        "FMINE-REF-207"
    );
    let mut over_budget = diagonal.clone();
    over_budget[0].target = 1;
    assert_eq!(
        plan.certify(&values, ZERO, z(2), over_budget)
            .unwrap_err()
            .code,
        "FMINE-REF-207"
    );
    assert_eq!(
        plan.certify(&values, ZERO, z(-1), diagonal)
            .unwrap_err()
            .code,
        "FMINE-REF-207"
    );
}
