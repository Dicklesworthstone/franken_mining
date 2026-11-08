//! A checked PCPSP incumbent and a replayed global Lagrangian bound (plan §15.4).
//! Determinism: DeterministicPerIsa (Price supplies floating-point guidance; acceptance and bounds are exact).
//!
//! External destination splits are retained exactly. Whole-block candidates can also seed local search;
//! fractional candidates are never rounded into a different plan. The bound includes every model block,
//! including negative-value material that may make a signed blending row feasible.

use fmine_evidence::Refusal;

use super::{PcpspImprovement, PcpspModel, candidate, greedy, improve, lp};
use crate::toposort::{cpit::RATE_SCALE, lp::Lagrangian, search};

/// Bounded work requested by the caller. Zero search rounds preserves the checked starting schedule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Options {
    pub rounds: usize,
    pub bound_steps: usize,
    pub share_decimals: u32,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            rounds: 20,
            bound_steps: 64,
            share_decimals: 9,
        }
    }
}

/// The original external candidate's findings; an infeasible candidate cannot become the incumbent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CandidateAssessment {
    pub checked: candidate::SplitCheck,
    /// Numerator over `share_scale * DISCOUNT_SCALE`, in the model's integer value units.
    pub discounted_numerator: i128,
}

/// Positive capability: a feasible schedule, its independently recomputed value, and a global upper bound.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plan {
    pub schedule: candidate::SplitSchedule,
    pub checked: candidate::SplitCheck,
    pub discounted_numerator: i128,
    pub origin: &'static str,
    pub factors: Vec<i64>,
    pub heuristic: PcpspImprovement,
    pub candidate: Option<CandidateAssessment>,
    pub bound: Lagrangian,
    /// `(bound * share_scale) - discounted_numerator`; same denominator as the selected value.
    pub gap_numerator: i128,
}

fn refuse(detail: &str) -> Refusal {
    crate::toposort::refuse(detail.into())
}
fn overflow() -> Refusal {
    refuse("pcpsp plan: exact objective or bound overflows i128")
}

/// Generate and improve a whole-block schedule, check an optional outside schedule, keep the best feasible
/// incumbent (including the empty plan), then independently replay the best Lagrangian prices.
///
/// The result optimizes only the supplied, rounded integer model and the explicit integer discount factors.
/// A nonzero bound gap is not a proof of suboptimality; the bound may be loose. Input calibration and sampling
/// error are outside this numerical certificate.
///
/// # Errors
/// `FMINE-REF-207` for invalid work settings, an external share scale different from the requested scale,
/// overflowing exact arithmetic, or a failed feasibility/value/bound verification; parser and solver refusals
/// are preserved. At most 1,000 search rounds and 4,096 bound steps are accepted.
pub fn solve(
    model: &PcpspModel,
    options: Options,
    outside: Option<&candidate::SplitSchedule>,
) -> Result<Plan, Refusal> {
    if options.rounds > 1_000
        || !(1..=4_096).contains(&options.bound_steps)
        || options.share_decimals > 9
    {
        return Err(refuse(
            "pcpsp plan: rounds must be 0..=1000, bound steps 1..=4096, share decimals 0..=9",
        ));
    }
    let scale = 10_i64.pow(options.share_decimals);
    if outside.is_some_and(|s| s.scale() != scale) {
        return Err(refuse(
            "pcpsp plan: external candidate share scale differs from the requested scale",
        ));
    }
    let factors = search::discount_factors(model.discount_rate, RATE_SCALE, model.periods())?;
    let scope = lp::scope(model)?;
    let mut destination = Vec::with_capacity(model.blocks());
    for b in 0..model.blocks() {
        let d = (0..model.destinations())
            .max_by_key(|&d| (model.value(b, d), std::cmp::Reverse(d)))
            .ok_or_else(|| refuse("pcpsp plan: block has no destination"))?;
        destination.push(d as u32);
    }
    let mut ordered = scope.blocks;
    ordered.sort_by_key(|&b| {
        (
            std::cmp::Reverse(model.value(b, destination[b] as usize)),
            b,
        )
    });
    let mut ranks = vec![None; model.blocks()];
    for (rank, b) in ordered.into_iter().enumerate() {
        ranks[b] = Some(i64::try_from(rank).map_err(|_| overflow())?);
    }
    let (start, destinations) = greedy(model, &ranks, &destination)?;
    let heuristic = improve(model, &start, &destinations, &factors, options.rounds)?;
    let mut selected = candidate::from_whole(
        model,
        &vec![None; model.blocks()],
        &destination,
        options.share_decimals,
    )?;
    let mut value = 0_i128;
    let mut origin = "empty";
    if heuristic.after > 0 {
        selected = candidate::from_whole(
            model,
            &heuristic.schedule,
            &heuristic.destination,
            options.share_decimals,
        )?;
        value = heuristic
            .after
            .checked_mul(i128::from(scale))
            .ok_or_else(overflow)?;
        origin = "heuristic";
    }
    let mut assessment = None;
    if let Some(external) = outside {
        let checked = candidate::check(model, external)?;
        let external_value = checked.discounted_value(&factors)?;
        if checked.feasible() {
            if external_value > value {
                selected = external.clone();
                value = external_value;
                origin = "external_candidate";
            }
            // A whole-block candidate is a useful warm start. Genuine splits retain their original policy.
            if external.assignments().iter().all(|a| a.fraction == scale) {
                let mut periods = vec![None; model.blocks()];
                let mut destinations = destination.clone();
                for a in external.assignments() {
                    periods[a.block as usize] = Some(a.period);
                    destinations[a.block as usize] = a.destination;
                }
                let improved = improve(model, &periods, &destinations, &factors, options.rounds)?;
                let improved_value = improved
                    .after
                    .checked_mul(i128::from(scale))
                    .ok_or_else(overflow)?;
                if improved_value > value {
                    selected = candidate::from_whole(
                        model,
                        &improved.schedule,
                        &improved.destination,
                        options.share_decimals,
                    )?;
                    value = improved_value;
                    origin = "improved_candidate";
                }
            }
        }
        assessment = Some(CandidateAssessment {
            checked,
            discounted_numerator: external_value,
        });
    }
    let checked = candidate::check(model, &selected)?;
    if !checked.feasible() || checked.discounted_value(&factors)? != value {
        return Err(refuse(
            "pcpsp plan: selected incumbent failed independent feasibility or value recomputation",
        ));
    }
    // Full model scope is essential: the certificate must also cover any supplied external schedule.
    let blocks: Vec<usize> = (0..model.blocks()).collect();
    let bound = lp::lagrangian_bound(
        model,
        &blocks,
        &factors,
        value / i128::from(scale),
        options.bound_steps,
    )?;
    let replay = lp::lagrangian_value(model, &blocks, &factors, &bound.prices)?;
    if replay.bound != bound.bound {
        return Err(refuse(
            "pcpsp plan: final Lagrangian prices do not reproduce the reported bound",
        ));
    }
    let gap = bound
        .bound
        .checked_mul(i128::from(scale))
        .and_then(|b| b.checked_sub(value))
        .ok_or_else(overflow)?;
    if gap < 0 {
        return Err(refuse(
            "pcpsp plan: a feasible incumbent exceeds the replayed upper bound",
        ));
    }
    Ok(Plan {
        schedule: selected,
        checked,
        discounted_numerator: value,
        origin,
        factors,
        heuristic,
        candidate: assessment,
        bound,
        gap_numerator: gap,
    })
}

#[cfg(test)]
mod tests;
