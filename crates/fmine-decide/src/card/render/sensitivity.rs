//! DESIGN (BD-225/BD-227): exact ties under uniform action cost or shared-state weight stress.
//! Determinism: Exact
//!
//! Total variation is half the L1 change of the declared state weights. It is not a probability of
//! reversal, a Wasserstein distance, or protection against states absent from the supplied model.

use std::cmp::Ordering;
use std::collections::BTreeMap;

use crate::card::{
    AlternativeStats, DecisionSummary, ExactMoney, ONE, Result, ZERO, arithmetic, refusal,
};
use fmine_evidence::{Refusal, RefusalCode};
use fmine_numerics::exact_feas::Rat;
use fmine_units::Currency;

const NO_CLAIM: &str = "Conditional expected-value ties for the act-now alternatives, keeping their payoffs and existing shared state population fixed. Uniform cost applies to the displayed primary in every state. Weight stress permits nonnegative weights summing to one, including zero weights. Total variation is half the L1 weight change, not a probability of a decision flipping, a calibrated confidence level, a Wasserstein radius, or a bound on unrepresented unknowns. Information-option policies and minimax-regret choices are not reoptimized by this analysis.";

#[derive(Debug, Clone, PartialEq, Eq)]
struct WeightTransfer {
    from_state: String,
    to_state: String,
    mass: Rat,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct WeightBoundary {
    total_variation: Rat,
    transfers: Vec<WeightTransfer>,
    // For every stress radius r, gap reduction <= slope*r + intercept. Equality at the witness
    // proves minimality. An already tied baseline has no positive-radius dual certificate.
    dual: Option<(ExactMoney, ExactMoney)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ChallengerMargin {
    action_id: String,
    uniform_cost_to_tie: ExactMoney,
    minimum_state_advantage: ExactMoney,
    // A tie need not allow strict reversal: a pointwise weakly dominant primary can only tie.
    strict_overtake_possible: bool,
    // None means the primary has strictly positive advantage in every declared state.
    weight_boundary: Option<WeightBoundary>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DecisionSensitivity {
    primary: String,
    uniform_cost_to_tie: ExactMoney,
    cost_challengers: Vec<String>,
    total_variation_to_tie: Option<Rat>,
    weight_challengers: Vec<String>,
    comparisons: Vec<ChallengerMargin>,
}

#[derive(Debug, Clone)]
struct State {
    id: String,
    weight: Rat,
    advantage: Rat,
}

fn failed(message: &str) -> Refusal {
    refusal(RefusalCode::NumericalCertificateFailed207, message)
}

// Only admitted complete-state statistics reach this projection. The canonical summary binds either
// the exact sensitivity and witness or its explicit refusal; an overflow never hides an alternative.
fn assess(alternatives: &[AlternativeStats], primary_id: &str) -> Result<DecisionSensitivity> {
    let primary = alternatives
        .iter()
        .find(|a| a.action_id == primary_id)
        .ok_or_else(|| failed("sensitivity primary is absent"))?;
    let money = |major| ExactMoney {
        currency: primary.mean.currency,
        major,
    };
    let mut comparisons = Vec::new();
    for rival in alternatives.iter().filter(|a| a.action_id != primary_id) {
        if primary.states.len() != rival.states.len() || primary.mean.currency != rival.mean.currency {
            return Err(failed("sensitivity requires the same state population and currency"));
        }
        let states = primary
            .states
            .iter()
            .zip(&rival.states)
            .map(|(a, b)| {
                if a.state_id != b.state_id
                    || a.weight != b.weight
                    || a.payoff.currency != primary.mean.currency
                    || b.payoff.currency != primary.mean.currency
                {
                    return Err(failed("sensitivity states, weights or monetary basis differ"));
                }
                Ok(State {
                    id: a.state_id.clone(),
                    weight: a.weight,
                    advantage: arithmetic(a.payoff.major.checked_sub(b.payoff.major))?,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        let gap = arithmetic(primary.mean.major.checked_sub(rival.mean.major))?;
        let (minimum, boundary) = boundary(&states, gap, primary.mean.currency)?;
        comparisons.push(ChallengerMargin {
            action_id: rival.action_id.clone(),
            uniform_cost_to_tie: money(gap),
            minimum_state_advantage: money(minimum),
            strict_overtake_possible: minimum.signum() < 0,
            weight_boundary: boundary,
        });
    }
    comparisons.sort_by(|a, b| a.action_id.cmp(&b.action_id));
    let first = comparisons
        .first()
        .ok_or_else(|| failed("sensitivity has no challenger"))?;
    let mut cost = first.uniform_cost_to_tie.major;
    let mut radius = None;
    for row in &comparisons {
        if arithmetic(row.uniform_cost_to_tie.major.checked_cmp(cost))? == Ordering::Less {
            cost = row.uniform_cost_to_tie.major;
        }
        if let Some(bound) = &row.weight_boundary {
            let improves = match radius {
                None => true,
                Some(old) => arithmetic(bound.total_variation.checked_cmp(old))? == Ordering::Less,
            };
            if improves {
                radius = Some(bound.total_variation);
            }
        }
    }
    Ok(DecisionSensitivity {
        primary: primary_id.into(),
        uniform_cost_to_tie: money(cost),
        cost_challengers: comparisons
            .iter()
            .filter(|r| r.uniform_cost_to_tie.major == cost)
            .map(|r| r.action_id.clone())
            .collect(),
        total_variation_to_tie: radius,
        weight_challengers: comparisons
            .iter()
            .filter(|r| {
                r.weight_boundary
                    .as_ref()
                    .is_some_and(|b| Some(b.total_variation) == radius)
            })
            .map(|r| r.action_id.clone())
            .collect(),
        comparisons,
    })
}

// Fallible stable merge sort: never replace an overflowing rational comparison with a guessed order.
fn order(states: &[State], indices: &[usize]) -> Result<Vec<usize>> {
    if indices.len() < 2 {
        return Ok(indices.to_vec());
    }
    let mid = indices.len() / 2;
    let left = order(states, &indices[..mid])?;
    let right = order(states, &indices[mid..])?;
    let (mut i, mut j) = (0, 0);
    let mut out = Vec::with_capacity(indices.len());
    while i < left.len() && j < right.len() {
        let (a, b) = (&states[left[i]], &states[right[j]]);
        let cmp = arithmetic(b.advantage.checked_cmp(a.advantage))?.then(a.id.cmp(&b.id));
        if cmp != Ordering::Greater {
            out.push(left[i]);
            i += 1;
        } else {
            out.push(right[j]);
            j += 1;
        }
    }
    out.extend_from_slice(&left[i..]);
    out.extend_from_slice(&right[j..]);
    Ok(out)
}

fn boundary(
    states: &[State],
    gap: Rat,
    currency: Currency,
) -> Result<(Rat, Option<WeightBoundary>)> {
    if states.is_empty() || gap.signum() < 0 {
        return Err(failed("invalid sensitivity gap or population"));
    }
    let ranked = order(states, &(0..states.len()).collect::<Vec<_>>())?;
    let last = *ranked
        .last()
        .ok_or_else(|| failed("missing sensitivity state"))?;
    let minimum = states[last].advantage;
    // Canonical recipient, independent of input order and donor tie ordering.
    let recipient = states
        .iter()
        .enumerate()
        .filter(|(_, s)| s.advantage == minimum)
        .min_by(|(_, a), (_, b)| a.id.cmp(&b.id))
        .map(|(i, _)| i)
        .ok_or_else(|| failed("missing minimum-advantage state"))?;
    if gap == ZERO {
        return Ok((
            minimum,
            Some(WeightBoundary {
                total_variation: ZERO,
                transfers: vec![],
                dual: None,
            }),
        ));
    }
    if minimum.signum() > 0 {
        return Ok((minimum, None));
    }
    let (mut remaining, mut radius, mut slope) = (gap, ZERO, ZERO);
    let mut transfers = Vec::new();
    for index in ranked {
        let gain = arithmetic(states[index].advantage.checked_sub(minimum))?;
        if gain == ZERO || remaining == ZERO {
            break;
        }
        let needed = arithmetic(remaining.checked_div(gain))?;
        let take = if arithmetic(needed.checked_cmp(states[index].weight))? == Ordering::Greater {
            states[index].weight
        } else {
            needed
        };
        remaining = arithmetic(remaining.checked_sub(arithmetic(take.checked_mul(gain))?))?;
        radius = arithmetic(radius.checked_add(take))?;
        slope = gain;
        transfers.push(WeightTransfer {
            from_state: states[index].id.clone(),
            to_state: states[recipient].id.clone(),
            mass: take,
        });
    }
    if remaining != ZERO || slope.signum() <= 0 {
        return Err(failed("weight stress failed to reach the exact tie"));
    }
    let intercept = dual_intercept(states, minimum, slope)?;
    let money = |major| ExactMoney { currency, major };
    let result = WeightBoundary {
        total_variation: radius,
        transfers,
        dual: Some((money(slope), money(intercept))),
    };
    verify(states, gap, &result)?;
    Ok((minimum, Some(result)))
}

fn dual_intercept(states: &[State], minimum: Rat, slope: Rat) -> Result<Rat> {
    let mut intercept = ZERO;
    for state in states {
        let extra = arithmetic(arithmetic(state.advantage.checked_sub(minimum))?.checked_sub(slope))?;
        if extra.signum() > 0 {
            intercept = arithmetic(intercept.checked_add(arithmetic(state.weight.checked_mul(extra))?))?;
        }
    }
    Ok(intercept)
}

// For any q on this simplex, E_w[d]-E_q[d] <= lambda*TV(w,q)
// + sum_i w_i*max(d_i-min(d)-lambda,0). The primal tie and matching dual bound prove minimality.
fn verify(states: &[State], gap: Rat, proof: &WeightBoundary) -> Result<()> {
    let mut stressed: BTreeMap<_, _> = states.iter().map(|s| (s.id.as_str(), s.weight)).collect();
    let mut transfer_mass = ZERO;
    for transfer in &proof.transfers {
        if transfer.mass.signum() <= 0 || transfer.from_state == transfer.to_state {
            return Err(failed("invalid stress transfer"));
        }
        for (id, delta) in [
            (&transfer.from_state, arithmetic(transfer.mass.checked_neg())?),
            (&transfer.to_state, transfer.mass),
        ] {
            let weight = stressed
                .get_mut(id.as_str())
                .ok_or_else(|| failed("stress names an unknown state"))?;
            *weight = arithmetic(weight.checked_add(delta))?;
        }
        transfer_mass = arithmetic(transfer_mass.checked_add(transfer.mass))?;
    }
    let (mut sum, mut residual, mut l1, mut original_gap) = (ZERO, ZERO, ZERO, ZERO);
    let mut minimum = states
        .first()
        .ok_or_else(|| failed("empty stress population"))?
        .advantage;
    for state in states {
        let weight = stressed[state.id.as_str()];
        if weight.signum() < 0 {
            return Err(failed("stress has a negative weight"));
        }
        sum = arithmetic(sum.checked_add(weight))?;
        residual = arithmetic(residual.checked_add(arithmetic(weight.checked_mul(state.advantage))?))?;
        original_gap = arithmetic(original_gap.checked_add(arithmetic(
            state.weight.checked_mul(state.advantage),
        )?))?;
        let delta = arithmetic(weight.checked_sub(state.weight))?;
        let magnitude = if delta.signum() < 0 {
            arithmetic(delta.checked_neg())?
        } else {
            delta
        };
        l1 = arithmetic(l1.checked_add(magnitude))?;
        if arithmetic(state.advantage.checked_cmp(minimum))? == Ordering::Less {
            minimum = state.advantage;
        }
    }
    if original_gap != gap
        || sum != ONE
        || residual != ZERO
        || transfer_mass != proof.total_variation
        || arithmetic(l1.checked_div(Rat::from_integer(2)))? != proof.total_variation
    {
        return Err(failed("stress witness does not certify the declared exact tie and radius"));
    }
    let (slope, intercept) = proof
        .dual
        .ok_or_else(|| failed("stress lacks its dual bound"))?;
    if slope.major.signum() <= 0 {
        return Err(failed("stress dual slope must be positive"));
    }
    if intercept.major != dual_intercept(states, minimum, slope.major)?
        || arithmetic(arithmetic(slope.major.checked_mul(proof.total_variation))?.checked_add(intercept.major))? != gap
    {
        return Err(failed("stress dual bound does not establish minimality"));
    }
    Ok(())
}

fn boundary_json(value: &WeightBoundary) -> String {
    use super::{js, rq};
    let transfers = value.transfers.iter().map(|t| format!(
        "{{\"from_state\":{},\"to_state\":{},\"mass\":{}}}",
        js(&t.from_state), js(&t.to_state), rq(t.mass)
    )).collect::<Vec<_>>().join(",");
    let dual = value.dual.map_or_else(|| "null".into(), |(slope, intercept)| format!(
        "{{\"slope\":{},\"intercept\":{},\"bound\":\"gap_reduction_le_slope_times_total_variation_plus_intercept\"}}",
        rq(slope.major), rq(intercept.major)
    ));
    format!(
        "{{\"total_variation\":{},\"transfers\":[{transfers}],\"dual\":{dual}}}",
        rq(value.total_variation)
    )
}

pub(super) fn json(summary: &DecisionSummary) -> String {
    use super::{ids, js, rq};
    match assess(&summary.alternatives, &summary.expected_value_primary) {
        Err(error) => format!(
            "{{\"status\":\"refused\",\"code\":{},\"what\":{},\"no_claim\":{}}}",
            js(error.code), js(&error.what), js(NO_CLAIM)
        ),
        Ok(result) => {
            let rows = result.comparisons.iter().map(|r| format!(
                "{{\"action_id\":{},\"uniform_cost_to_tie\":{},\"minimum_state_advantage\":{},\"strict_overtake_possible\":{},\"weight_status\":{},\"weight_boundary\":{}}}",
                js(&r.action_id), rq(r.uniform_cost_to_tie.major), rq(r.minimum_state_advantage.major),
                r.strict_overtake_possible,
                js(if r.weight_boundary.is_some() { "tie_reachable" } else { "strict_pointwise_dominance" }),
                r.weight_boundary.as_ref().map_or_else(|| "null".into(), boundary_json)
            )).collect::<Vec<_>>().join(",");
            format!(
                "{{\"status\":\"ok\",\"primary\":{},\"currency\":{},\"monetary_scale\":\"major_currency_units\",\"uniform_cost_to_tie\":{},\"cost_challengers\":{},\"total_variation_to_tie\":{},\"weight_challengers\":{},\"comparisons\":[{rows}],\"no_claim\":{}}}",
                js(&result.primary), js(crate::card::currency_name(summary.currency)),
                rq(result.uniform_cost_to_tie.major), ids(&result.cost_challengers),
                result.total_variation_to_tie.map_or_else(|| "null".into(), rq),
                ids(&result.weight_challengers), js(NO_CLAIM)
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(n: i128, d: i128) -> Rat {
        Rat::new(n, d).unwrap()
    }
    fn states(values: &[i128], weights: &[Rat]) -> Vec<State> {
        values.iter().zip(weights).enumerate().map(|(i, (d, w))| State {
            id: format!("s{i}"), weight: *w, advantage: Rat::from_integer(*d),
        }).collect()
    }
    fn solve(states: &[State]) -> (Rat, Option<WeightBoundary>) {
        let gap = states.iter().fold(ZERO, |sum, s| {
            sum.checked_add(s.weight.checked_mul(s.advantage).unwrap()).unwrap()
        });
        boundary(states, gap, Currency::USD).unwrap()
    }

    #[test]
    fn exact_expansion_weight_boundary_and_dual_known_answer() {
        let s = states(&[300, -200], &[r(3, 5), r(2, 5)]);
        let (minimum, proof) = solve(&s);
        let p = proof.unwrap();
        assert_eq!(minimum, r(-200, 1));
        assert_eq!(p.total_variation, r(1, 5));
        let (slope, intercept) = p.dual.unwrap();
        assert_eq!(slope.major, r(500, 1));
        assert_eq!(intercept.major, ZERO);
        assert_eq!(p.transfers, vec![WeightTransfer {
            from_state: "s0".into(), to_state: "s1".into(), mass: r(1, 5),
        }]);
    }

    #[test]
    fn multiple_donors_have_fractional_boundary_and_nonzero_dual_intercept() {
        let s = states(&[10, 4, -1], &[r(1, 10), r(4, 5), r(1, 10)]);
        let p = solve(&s).1.unwrap();
        assert_eq!(p.total_variation, r(7, 10));
        assert_eq!(p.transfers.iter().map(|t| t.mass).collect::<Vec<_>>(), vec![r(1, 10), r(3, 5)]);
        let (slope, intercept) = p.dual.unwrap();
        assert_eq!(slope.major, r(5, 1));
        assert_eq!(intercept.major, r(3, 5));
        let mut shuffled = s.clone();
        shuffled.reverse();
        assert_eq!(solve(&shuffled).1.unwrap(), p);
        let mut edited = p.clone();
        edited.transfers[1].mass = r(1, 2);
        assert!(verify(&s, r(41, 10), &edited).is_err());
        let mut edited = p;
        edited.dual.as_mut().unwrap().1.major = ZERO;
        assert!(verify(&s, r(41, 10), &edited).is_err());
    }

    #[test]
    fn pointwise_dominance_tie_only_and_baseline_tie_are_distinct() {
        let w = [r(1, 2), r(1, 2)];
        assert!(solve(&states(&[1, 1], &w)).1.is_none());
        let tie_only = solve(&states(&[1, 0], &w));
        assert_eq!(tie_only.0, ZERO);
        assert_eq!(tie_only.1.unwrap().total_variation, r(1, 2));
        for values in [[1, -1], [0, 0]] {
            let p = solve(&states(&values, &w)).1.unwrap();
            assert_eq!(p.total_variation, ZERO);
            assert!(p.transfers.is_empty());
            assert!(p.dual.is_none());
        }
    }

    #[test]
    fn exact_radius_bounds_every_independent_three_state_grid_stress() {
        let w = [r(1, 3); 3];
        for a in -2..=2 {
            for b in -2..=2 {
                for c in -2..=2 {
                    if a + b + c < 0 {
                        continue;
                    }
                    let s = states(&[a, b, c], &w);
                    let (_, proof) = solve(&s);
                    for i in 0..=12 {
                        for j in 0..=12 - i {
                            let q = [r(i, 12), r(j, 12), r(12 - i - j, 12)];
                            let residual = q.iter().zip([a, b, c]).fold(ZERO, |sum, (p, d)| {
                                sum.checked_add(p.checked_mul(Rat::from_integer(d)).unwrap()).unwrap()
                            });
                            if residual.signum() <= 0 {
                                let l1 = q.iter().zip(w).fold(ZERO, |sum, (p, w)| {
                                    let v = p.checked_sub(w).unwrap();
                                    sum.checked_add(if v.signum() < 0 {
                                        v.checked_neg().unwrap()
                                    } else { v }).unwrap()
                                });
                                let tv = l1.checked_div(r(2, 1)).unwrap();
                                let bound = proof.as_ref().expect("reachable tie has a certificate");
                                assert!(tv.checked_cmp(bound.total_variation).unwrap() != Ordering::Less);
                            }
                        }
                    }
                }
            }
        }
    }
}
