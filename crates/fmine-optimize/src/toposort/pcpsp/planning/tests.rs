//! Small exact oracles for the assembled planner, independent of its greedy
//! search, candidate checker and Lagrangian-price selection.

//! Determinism: DeterministicPerIsa

use super::*;
use crate::pitflow::minelib::Rounding;
use crate::toposort::search::DISCOUNT_SCALE;

fn model(
    values: &[[i64; 2]],
    uses: &[[i64; 2]],
    limits: &[i64],
    rate: &str,
    requires: &[Vec<usize>],
) -> PcpspModel {
    assert_eq!(values.len(), uses.len());
    assert_eq!(values.len(), requires.len());
    let mut input = format!(
        "NAME: planner\nTYPE: PCPSP\nNBLOCKS: {}\nNPERIODS: {}\nNDESTINATIONS: 2\nNRESOURCE_SIDE_CONSTRAINTS: 1\nNGENERAL_SIDE_CONSTRAINTS: 0\nDISCOUNT_RATE: {rate}\nRESOURCE_CONSTRAINT_LIMITS:\n",
        values.len(),
        limits.len()
    );
    for (t, limit) in limits.iter().enumerate() {
        input.push_str(&format!("0 {t} L {limit}\n"));
    }
    input.push_str("OBJECTIVE_FUNCTION:\n");
    for (b, v) in values.iter().enumerate() {
        input.push_str(&format!("{b} {} {}\n", v[0], v[1]));
    }
    input.push_str("RESOURCE_CONSTRAINT_COEFFICIENTS:\n");
    for (b, row) in uses.iter().enumerate() {
        for (d, q) in row.iter().enumerate() {
            input.push_str(&format!("{b} {d} 0 {q}\n"));
        }
    }
    input.push_str("EOF\n");
    let mut prec = String::new();
    for (b, row) in requires.iter().enumerate() {
        prec.push_str(&format!("{b} {}", row.len()));
        for q in row {
            prec.push_str(&format!(" {q}"));
        }
        prec.push('\n');
    }
    super::super::parse(&input, &prec, 0, Rounding::Refuse, 0).unwrap()
}

fn options(rounds: usize) -> Options {
    Options {
        rounds,
        bound_steps: 32,
        share_decimals: 3,
    }
}

/// Enumerate each block's unmined or (period, destination) choice and apply
/// the mathematical constraints directly. No candidate or schedule checker is
/// called; in particular, signed resource uses are added without truncation.
fn exhaustive_whole(model: &PcpspModel, factors: &[i64]) -> i128 {
    let choices = 1 + model.periods() * model.destinations();
    let mut best = 0;
    for code in 0..choices.pow(model.blocks() as u32) {
        let mut rest = code;
        let states: Vec<_> = (0..model.blocks())
            .map(|_| {
                let choice = rest % choices;
                rest /= choices;
                if choice == 0 {
                    None
                } else {
                    Some((
                        (choice - 1) / model.destinations(),
                        (choice - 1) % model.destinations(),
                    ))
                }
            })
            .collect();
        let mut feasible = true;
        let mut used = vec![vec![0_i128; model.periods()]; model.resources()];
        let mut value = 0_i128;
        for (b, state) in states.iter().enumerate() {
            let Some((t, d)) = *state else { continue };
            for &q in model.requires(b).unwrap() {
                feasible &= states[q as usize].is_some_and(|(qt, _)| qt <= t);
            }
            value += i128::from(model.value(b, d).unwrap()) * i128::from(factors[t]);
            for (r, row) in used.iter_mut().enumerate() {
                row[t] += i128::from(model.uses(r, b, d).unwrap());
            }
        }
        for (r, row) in used.iter().enumerate() {
            for (t, &q) in row.iter().enumerate() {
                feasible &= q <= i128::from(model.limits(r).unwrap()[t]);
            }
        }
        if feasible {
            best = best.max(value);
        }
    }
    best
}

fn assert_certificate(model: &PcpspModel, plan: &Plan) {
    let all: Vec<_> = (0..model.blocks()).collect();
    let replay = lp::lagrangian_value(model, &all, &plan.factors, &plan.bound.prices).unwrap();
    assert_eq!(replay.bound, plan.bound.bound);
    assert!(plan.checked.feasible());
    assert_eq!(
        candidate::check(model, &plan.schedule).unwrap(),
        plan.checked
    );
    assert_eq!(
        plan.checked.discounted_value(&plan.factors).unwrap(),
        plan.discounted_numerator
    );
    assert_eq!(
        plan.bound.bound * i128::from(plan.schedule.scale()) - plan.discounted_numerator,
        plan.gap_numerator
    );
    assert!(plan.gap_numerator >= 0);
    let best_whole = exhaustive_whole(model, &plan.factors);
    assert!(plan.bound.history.iter().all(|&bound| bound >= best_whole));
    assert_eq!(plan.bound.history.iter().min(), Some(&plan.bound.bound));
}

#[test]
fn produces_a_positive_discounted_plan_and_replayable_global_certificate() {
    let m = model(
        &[[0, 3], [0, 5]],
        &[[0, 1], [0, 1]],
        &[1, 1],
        "1",
        &[vec![], vec![]],
    );
    let p = solve(&m, options(8), None).unwrap();
    assert_eq!(p.origin, "heuristic");
    assert_eq!(p.factors, [DISCOUNT_SCALE, DISCOUNT_SCALE / 2]);
    assert_eq!(p.checked.period_values, [5_000, 3_000]);
    assert_eq!(p.discounted_numerator, 6_500 * i128::from(DISCOUNT_SCALE));
    assert!(p.candidate.is_none());
    assert_certificate(&m, &p);
    assert_eq!(
        p.discounted_numerator / 1_000,
        exhaustive_whole(&m, &p.factors)
    );
    assert_eq!(solve(&m, options(8), None).unwrap(), p);
}

#[test]
fn exact_split_beats_every_whole_destination_and_is_retained_without_rounding() {
    // One block: dump value 0/use 0, mill value 10/use 2, capacity 1.
    // Every feasible whole-block plan is worth 0; a half-mill split is worth 5.
    let m = model(&[[0, 10]], &[[0, 2]], &[1], "0", &[vec![]]);
    let external = candidate::parse_solution("0 1 0 0.5\n0 0 0 0.5\n", &m, 3).unwrap();
    let p = solve(&m, options(8), Some(&external)).unwrap();
    assert_eq!(p.origin, "external_candidate");
    assert_eq!(p.schedule, external);
    assert_eq!(p.checked.period_values, [5_000]);
    assert_eq!(p.checked.used, [vec![1_000]]);
    assert_eq!(exhaustive_whole(&m, &p.factors), 0);
    assert_eq!(p.discounted_numerator, 5_000 * i128::from(DISCOUNT_SCALE));
    assert!(p.candidate.as_ref().unwrap().checked.feasible());
    assert_certificate(&m, &p);
    // At resource price 5 * F, either destination has zero reduced value;
    // the price of the capacity gives the exact continuous optimum 5 * F.
    let bound = lp::lagrangian_value(&m, &[0], &p.factors, &[vec![5 * DISCOUNT_SCALE]]).unwrap();
    assert_eq!(bound.bound, 5 * i128::from(DISCOUNT_SCALE));
    assert_eq!(p.gap_numerator, 0);
}

#[test]
fn generates_a_profitable_split_without_an_external_candidate() {
    let m = model(&[[0, 10]], &[[0, 2]], &[1], "0", &[vec![]]);
    let p = solve(&m, options(8), None).unwrap();
    assert_eq!(p.origin, "fractional_greedy");
    assert_eq!(p.discounted_numerator, 5_000 * i128::from(DISCOUNT_SCALE));
    assert_eq!(p.gap_numerator, 0);
    assert_eq!(
        candidate::write_solution(&p.schedule),
        "0 0 0 0.5\n0 1 0 0.5\n"
    );
    assert!(p.candidate.is_none());
    assert_certificate(&m, &p);
}

#[test]
fn initial_greedy_mining_set_is_polished_before_local_search_discards_a_loss_making_dump() {
    let m = model(&[[-2, 10]], &[[0, 2]], &[1], "0", &[vec![]]);
    let p = solve(&m, options(8), None).unwrap();
    assert_eq!(p.heuristic.after, 0);
    assert_eq!(p.heuristic.schedule, [None]);
    assert_eq!(p.origin, "fractional_greedy");
    assert_eq!(p.discounted_numerator, 4_000 * i128::from(DISCOUNT_SCALE));
    // The full-block split earns 4. The scheduling LP can mine only half the
    // block entirely to the mill and earn 5, avoiding the other half's dump
    // cost. Therefore this bound has a real relaxation gap, even at optimal
    // prices: mu=5*F makes the mill's reduced value zero and prices capacity at5.
    let hand = lp::lagrangian_value(&m, &[0], &p.factors, &[vec![5 * DISCOUNT_SCALE]]).unwrap();
    assert_eq!(hand.bound, 5 * i128::from(DISCOUNT_SCALE));
    assert!(p.bound.bound >= hand.bound);
    assert!(p.gap_numerator >= 1_000 * i128::from(DISCOUNT_SCALE));
    assert_certificate(&m, &p);
}

#[test]
fn infeasible_outside_candidate_is_reported_and_cannot_replace_the_incumbent() {
    let m = model(&[[0, 10]], &[[0, 2]], &[1], "0", &[vec![]]);
    for text in ["0 1 0 1\n", "0 0 0 0.499\n0 1 0 0.5\n"] {
        let external = candidate::parse_solution(text, &m, 3).unwrap();
        let p = solve(&m, options(4), Some(&external)).unwrap();
        assert_eq!(p.origin, "fractional_greedy");
        assert_eq!(p.discounted_numerator, 5_000 * i128::from(DISCOUNT_SCALE));
        assert_eq!(
            candidate::write_solution(&p.schedule),
            "0 0 0 0.5\n0 1 0 0.5\n"
        );
        let assessment = p.candidate.as_ref().unwrap();
        assert!(!assessment.checked.feasible());
        assert!(assessment.discounted_numerator > 0);
        assert_certificate(&m, &p);
    }
}

#[test]
fn whole_candidate_seeds_destination_search_that_the_greedy_start_cannot_reach() {
    // The two blocks mutually require one another. They can legally be mined
    // together, but Kahn's greedy has no starting node. The outside plan supplies
    // that feasible mining period; destination search improves its value 2 to 10.
    let m = model(
        &[[1, 5], [1, 5]],
        &[[0, 0], [0, 0]],
        &[0],
        "0",
        &[vec![1], vec![0]],
    );
    let external = candidate::parse_solution("0 0 0 1\n1 0 0 1\n", &m, 3).unwrap();
    let without_whole_search = solve(&m, options(0), Some(&external)).unwrap();
    assert_eq!(without_whole_search.origin, "fractional_candidate");
    assert_eq!(
        without_whole_search.discounted_numerator,
        10_000 * i128::from(DISCOUNT_SCALE)
    );
    let p = solve(&m, options(8), Some(&external)).unwrap();
    assert_eq!(p.origin, "improved_candidate");
    assert_eq!(p.heuristic.after, 0);
    assert_eq!(p.discounted_numerator, 10_000 * i128::from(DISCOUNT_SCALE));
    assert_eq!(
        p.candidate.as_ref().unwrap().discounted_numerator,
        2_000 * i128::from(DISCOUNT_SCALE)
    );
    assert_eq!(candidate::write_solution(&p.schedule), "0 1 0 1\n1 1 0 1\n");
    assert_certificate(&m, &p);
}

#[test]
fn an_admitted_whole_mining_cycle_can_seed_a_fraction_the_internal_start_cannot_reach() {
    let m = model(
        &[[0, 10], [0, 0]],
        &[[0, 2], [0, 0]],
        &[1],
        "0",
        &[vec![1], vec![0]],
    );
    let external = candidate::parse_solution("0 0 0 1\n1 0 0 1\n", &m, 3).unwrap();
    let p = solve(&m, options(8), Some(&external)).unwrap();
    assert_eq!(p.heuristic.schedule, [None, None]);
    assert_eq!(p.origin, "fractional_candidate");
    assert_eq!(p.discounted_numerator, 5_000 * i128::from(DISCOUNT_SCALE));
    assert_eq!(p.candidate.as_ref().unwrap().discounted_numerator, 0);
    assert_eq!(p.gap_numerator, 0);
    assert_eq!(
        candidate::write_solution(&p.schedule),
        "0 0 0 0.5\n0 1 0 0.5\n1 0 0 1\n"
    );
    assert_certificate(&m, &p);
}

#[test]
fn negative_value_blend_helper_is_in_the_global_bound_even_outside_the_ultimate_pit() {
    // Value-10 ore consumes +1 blend unit. An independent value-(-1) helper
    // supplies -1; capacity is zero. The only positive feasible whole plan is 9.
    let m = model(
        &[[0, 10], [-1, -1]],
        &[[0, 1], [-1, -1]],
        &[0],
        "0",
        &[vec![], vec![]],
    );
    let scope = lp::scope(&m).unwrap();
    assert!(scope.whole);
    assert_eq!(scope.pit_blocks, 1);
    let external = candidate::parse_solution("0 1 0 1\n1 0 0 1\n", &m, 3).unwrap();
    let p = solve(&m, options(4), Some(&external)).unwrap();
    assert_eq!(p.discounted_numerator, 9_000 * i128::from(DISCOUNT_SCALE));
    assert_eq!(p.checked.used, [vec![0]]);
    assert_eq!(p.bound.history[0], 10 * i128::from(DISCOUNT_SCALE));
    assert_eq!(p.bound.bound, 9 * i128::from(DISCOUNT_SCALE));
    assert_eq!(p.gap_numerator, 0);
    assert_eq!(
        exhaustive_whole(&m, &p.factors),
        9 * i128::from(DISCOUNT_SCALE)
    );
    assert_certificate(&m, &p);
}

#[test]
fn every_price_iterate_bounds_an_independent_exhaustive_oracle_on_signed_tiny_models() {
    // Deterministic varied coefficients, costs, signed resources, precedence
    // and discounting. Five choices per block give 125 schedules per model.
    for seed in 0..24_i64 {
        let values: Vec<_> = (0..3)
            .map(|b| [-(b + seed % 3), (seed * 7 + b * 3) % 13 - 3])
            .collect();
        let uses: Vec<_> = (0..3)
            .map(|b| {
                [
                    if seed % 2 == 0 && b == 1 { -1 } else { 0 },
                    (seed + 2 * b) % 4 - 1,
                ]
            })
            .collect();
        let requires = if seed % 3 == 0 {
            vec![vec![], vec![], vec![0]]
        } else {
            vec![vec![], vec![], vec![]]
        };
        let m = model(&values, &uses, &[1, 2], "0.25", &requires);
        let p = solve(
            &m,
            Options {
                rounds: 2,
                bound_steps: 8,
                share_decimals: 2,
            },
            None,
        )
        .unwrap();
        assert_certificate(&m, &p);
        // Whole selected plans remain bounded by the integer oracle. An
        // internally generated fractional plan may legitimately exceed it.
        let oracle = exhaustive_whole(&m, &p.factors);
        if p.schedule
            .assignments()
            .iter()
            .all(|a| a.fraction == p.schedule.scale())
        {
            assert!(p.discounted_numerator <= oracle * 100, "seed {seed}");
        } else {
            assert!(matches!(
                p.origin,
                "fractional_greedy" | "fractional_improved"
            ));
        }
    }
}

#[test]
fn no_positive_economics_returns_a_checked_empty_plan() {
    let m = model(
        &[[-5, -2], [-3, -1]],
        &[[1, 1], [1, 1]],
        &[2],
        "0",
        &[vec![], vec![]],
    );
    let p = solve(&m, options(3), None).unwrap();
    assert_eq!(p.origin, "empty");
    assert_eq!(p.discounted_numerator, 0);
    assert_eq!(p.bound.bound, 0);
    assert_eq!(p.gap_numerator, 0);
    assert!(p.schedule.assignments().is_empty());
    assert_certificate(&m, &p);
}

#[test]
fn work_scale_and_exact_objective_overflow_refuse() {
    let m = model(&[[0, 10]], &[[0, 2]], &[1], "0", &[vec![]]);
    for invalid in [
        Options {
            rounds: 1_001,
            ..options(1)
        },
        Options {
            bound_steps: 0,
            ..options(1)
        },
        Options {
            bound_steps: 4_097,
            ..options(1)
        },
        Options {
            share_decimals: 10,
            ..options(1)
        },
    ] {
        assert!(
            solve(&m, invalid, None)
                .unwrap_err()
                .what
                .contains("rounds must")
        );
    }
    let other_scale = candidate::parse_solution("0 0 0 1\n", &m, 2).unwrap();
    assert!(
        solve(&m, options(1), Some(&other_scale))
            .unwrap_err()
            .what
            .contains("share scale differs")
    );
    let zero_precision = solve(
        &m,
        Options {
            share_decimals: 0,
            ..options(0)
        },
        None,
    )
    .unwrap();
    assert_eq!(zero_precision.schedule.scale(), 1);
    assert_certificate(&m, &zero_precision);
    let huge = model(
        &[[0, 1_000_000_000_000_000_000]],
        &[[0, 1]],
        &[1],
        "0",
        &[vec![]],
    );
    let error = solve(
        &huge,
        Options {
            share_decimals: 9,
            ..options(0)
        },
        None,
    )
    .unwrap_err();
    assert!(error.what.contains("overflows i128"), "{error}");
}
