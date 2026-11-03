//! DESIGN (BD-229, §22.5): exact weighted resampling and reusable rational payoff reductions.
//! Determinism: Exact (integer Philox draws, checked rational arithmetic, canonical caller ordering).
//!
//! Counts describe a conditional multinomial experiment, not calibrated model uncertainty. Callers own
//! state identity, units, source binding, constraints and selection; a zero count never waives a constraint.

use fmine_core::rand::{PhiloxStream, RngKey};
use fmine_evidence::{Refusal, RefusalCode};
use fmine_units::{ExactQty as Rat, UnitsError};

use super::exact::compare;

pub const MIN_REPLICATES: usize = 200;
pub const MAX_REPLICATES: usize = 2_000;
pub const MAX_STATES: usize = 256;
pub const MAX_DRAWS: usize = 4_096;
pub const MAX_SAMPLED_WORK: usize = 8_000_000;
const MAX_REJECTIONS: usize = 128;
const ZERO: Rat = Rat::from_integer(0);
const ONE: Rat = Rat::from_integer(1);
type Result<T> = std::result::Result<T, Refusal>;

fn invalid(why: &str) -> Refusal {
    Refusal::new(
        RefusalCode::OutsidePhysicalDomain109,
        format!("weighted bootstrap: {why}"),
    )
}

fn bounded(why: &str) -> Refusal {
    Refusal::new(
        RefusalCode::NoClaimInDomain201,
        format!("weighted bootstrap: {why}"),
    )
}

fn numerical(why: impl std::fmt::Display) -> Refusal {
    Refusal::new(
        RefusalCode::NumericalCertificateFailed207,
        format!("weighted bootstrap: {why}"),
    )
}

fn arithmetic<T>(value: std::result::Result<T, UnitsError>) -> Result<T> {
    value.map_err(numerical)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BootstrapPlan {
    pub seed: u64,
    pub replicates: usize,
    /// An explicit experiment size; it is not an inferred effective independent sample count.
    pub draws_per_replicate: usize,
}

impl BootstrapPlan {
    /// Checks all allocation and sampling bounds before inspecting or serializing full input vectors.
    pub fn validate(self, state_count: usize) -> Result<()> {
        if !(1..=MAX_STATES).contains(&state_count)
            || !(MIN_REPLICATES..=MAX_REPLICATES).contains(&self.replicates)
            || !(1..=MAX_DRAWS).contains(&self.draws_per_replicate)
            || self
                .replicates
                .checked_mul(self.draws_per_replicate)
                .is_none_or(|work| work > MAX_SAMPLED_WORK)
        {
            return Err(bounded(
                "requires 1..=256 states, 200..=2000 replicates, 1..=4096 draws each and at most 8000000 total draws",
            ));
        }
        Ok(())
    }
}

enum Node {
    Leaf(usize),
    Branch {
        left: usize,
        right: usize,
        numerator: u128,
        denominator: u128,
    },
}

fn build_tree(weights: &[Rat], offset: usize, nodes: &mut Vec<Node>) -> Result<(usize, Rat)> {
    if weights.len() == 1 {
        let index = nodes.len();
        nodes.push(Node::Leaf(offset));
        return Ok((index, weights[0]));
    }
    let midpoint = weights.len() / 2;
    let (left, left_mass) = build_tree(&weights[..midpoint], offset, nodes)?;
    let (right, right_mass) = build_tree(&weights[midpoint..], offset + midpoint, nodes)?;
    let mass = arithmetic(left_mass.checked_add(right_mass))?;
    let conditional = arithmetic(left_mass.checked_div(mass))?;
    let index = nodes.len();
    nodes.push(Node::Branch {
        left,
        right,
        numerator: conditional.num() as u128,
        denominator: conditional.den() as u128,
    });
    Ok((index, mass))
}

fn bounded_u128(bound: u128, draw: &mut impl FnMut() -> u128) -> Result<u128> {
    if bound == 0 {
        return Err(invalid("integer sampling bound must be positive"));
    }
    if bound == 1 {
        return Ok(0);
    }
    // [threshold, 2^128) contains an exact multiple of bound values. Rejected draws are not folded
    // into a boundary state; doing so would bias rational probabilities whose denominator is not 2^k.
    let threshold = bound.wrapping_neg() % bound;
    for _ in 0..MAX_REJECTIONS {
        let value = draw();
        if value >= threshold {
            return Ok(value % bound);
        }
    }
    Err(numerical(
        "integer rejection sampler exhausted its attempt bound",
    ))
}

/// Samples with replacement from the exact unit-sum law, retaining a count for every original state.
///
/// The caller provides canonical state order. Each replicate owns a Philox stream, so extending the
/// replicate count or draw count preserves existing replicate prefixes. A balanced conditional tree
/// costs O(log states) branch draws and requires no common-denominator product or floating conversion.
pub fn sample_counts(weights: &[Rat], plan: BootstrapPlan) -> Result<Vec<Vec<u32>>> {
    plan.validate(weights.len())?;
    if weights.iter().any(|weight| weight.signum() <= 0) {
        return Err(invalid("state weights must be strictly positive"));
    }
    let mut nodes = Vec::with_capacity(2 * weights.len() - 1);
    let (root, total) = build_tree(weights, 0, &mut nodes)?;
    if total != ONE {
        return Err(invalid("state weights must sum exactly to one"));
    }
    let mut samples = Vec::with_capacity(plan.replicates);
    for replicate in 0..plan.replicates {
        let mut rng = PhiloxStream::new(RngKey {
            seed_root: plan.seed,
            partition: u32::from_le_bytes(*b"BOOT"),
            domain: 1,
            var_group: 0,
            realization: replicate as u32,
            brick: 0,
        });
        let mut draw = || u128::from(rng.next_u64()) | (u128::from(rng.next_u64()) << 64);
        let mut counts = vec![0_u32; weights.len()];
        for _ in 0..plan.draws_per_replicate {
            let mut index = root;
            loop {
                match nodes[index] {
                    Node::Leaf(state) => {
                        counts[state] += 1;
                        break;
                    }
                    Node::Branch {
                        left,
                        right,
                        numerator,
                        denominator,
                    } => {
                        index = if bounded_u128(denominator, &mut draw)? < numerator {
                            left
                        } else {
                            right
                        };
                    }
                }
            }
        }
        samples.push(counts);
    }
    Ok(samples)
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PayoffGroup {
    value: Rat,
    indices: Vec<usize>,
}

/// Immutable sorted payoff groups. Reweighting never sorts or rebuilds the original state outcomes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedPayoffs {
    state_count: usize,
    groups: Vec<PayoffGroup>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResampledStats {
    pub mean: Rat,
    /// Signed loss = -payoff in the requested worst tail, in the caller's original payoff unit.
    pub tail_loss: Option<Rat>,
}

impl PreparedPayoffs {
    pub fn new(values: &[Rat]) -> Result<Self> {
        if !(1..=MAX_STATES).contains(&values.len()) {
            return Err(bounded("payoff reductions require 1..=256 states"));
        }
        let mut order: Vec<_> = (0..values.len()).collect();
        order.sort_unstable_by(|&a, &b| compare(values[a], values[b]).then(a.cmp(&b)));
        let mut groups: Vec<PayoffGroup> = Vec::with_capacity(values.len());
        for index in order {
            if let Some(previous) = groups.last_mut()
                && previous.value == values[index]
            {
                previous.indices.push(index);
            } else {
                groups.push(PayoffGroup {
                    value: values[index],
                    indices: vec![index],
                });
            }
        }
        Ok(Self {
            state_count: values.len(),
            groups,
        })
    }

    /// Reduces exact count weights, including zero-count original states and a fractional tail atom.
    /// There is no payoff rounding, weight smoothing, risk-cap application or feasibility waiver.
    pub fn evaluate(&self, counts: &[u32], tail_mass: Option<Rat>) -> Result<ResampledStats> {
        if counts.len() != self.state_count {
            return Err(invalid("counts must retain every prepared state"));
        }
        if tail_mass.is_some_and(|mass| mass.signum() <= 0 || compare(mass, ONE).is_gt()) {
            return Err(invalid("tail mass must lie in (0,1]"));
        }
        let total: u64 = counts.iter().map(|&count| u64::from(count)).sum();
        if total == 0 || total > MAX_DRAWS as u64 {
            return Err(bounded("a count vector must sum to 1..=4096"));
        }
        let (mut mean, mut remaining, mut tail_integral) = (ZERO, tail_mass.unwrap_or(ZERO), ZERO);
        for group in &self.groups {
            let count: u64 = group
                .indices
                .iter()
                .map(|&index| u64::from(counts[index]))
                .sum();
            if count == 0 {
                continue;
            }
            let weight = arithmetic(Rat::new(i128::from(count), i128::from(total)))?;
            mean = arithmetic(mean.checked_add(arithmetic(group.value.checked_mul(weight))?))?;
            if remaining != ZERO {
                let take = if compare(weight, remaining).is_gt() {
                    remaining
                } else {
                    weight
                };
                tail_integral = arithmetic(
                    tail_integral.checked_add(arithmetic(group.value.checked_mul(take))?),
                )?;
                remaining = arithmetic(remaining.checked_sub(take))?;
            }
        }
        if remaining != ZERO {
            return Err(numerical(
                "count vector did not supply the requested tail mass",
            ));
        }
        let tail_loss = tail_mass
            .map(|mass| arithmetic(arithmetic(tail_integral.checked_div(mass))?.checked_neg()))
            .transpose()?;
        Ok(ResampledStats { mean, tail_loss })
    }
}

#[cfg(test)]
mod tests;
