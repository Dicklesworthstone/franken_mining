//! DESIGN (BD-252, plan §15.6): exact blend of two divisible, already-mined parcels
//! with additive recoverable metal and impurity mass.
//! Determinism: BitwisePortable

use super::{GradeTonnageBin, LaneTerms, Rat, Refusal, arithmetic, compare, domain};

/// Impurity fractions on the same dry-mass basis, in ascending grade-bin order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImpurityLimit {
    pub maximum_mass_fraction: Rat,
    pub bin_mass_fractions: [Rat; 2],
}

/// Exact one-period solution, including idle feed when no blend earns a positive contribution.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BlendAllocation {
    pub selected_bins: [GradeTonnageBin; 2],
    pub dry_tonnes: Rat,
    pub recovered_metal: Rat,
    pub incremental_contribution: Rat,
    /// Weighted impurity mass fraction, undefined for zero feed.
    pub impurity_fraction: Option<Rat>,
}

type Pair = [Rat; 2];

fn dot([x, y]: Pair, [a, b]: Pair) -> Result<Rat, Refusal> {
    x.checked_mul(a)
        .and_then(|u| u.checked_add(y.checked_mul(b)?))
        .map_err(|_| arithmetic())
}

fn determinant([a, b]: Pair, [c, d]: Pair) -> Result<Rat, Refusal> {
    a.checked_mul(d)
        .and_then(|ad| ad.checked_sub(b.checked_mul(c)?))
        .map_err(|_| arithmetic())
}

pub(super) fn solve(
    evaluated: &[(GradeTonnageBin, Rat)],
    limit: ImpurityLimit,
    terms: LaneTerms,
    net_price: Rat,
    max_grade: Rat,
) -> Result<BlendAllocation, Refusal> {
    let &[(low, low_recovered), (high, high_recovered)] = evaluated else {
        return Err(domain(
            "an impurity-limited blend requires exactly two bins",
        ));
    };
    let zero = Rat::from_integer(0);
    let one = Rat::from_integer(1);
    let neg_one = Rat::from_integer(-1);
    let [low_assay, high_assay] = limit.bin_mass_fractions;
    let maximum = limit.maximum_mass_fraction;
    for assay in [low_assay, high_assay, maximum] {
        if assay.signum() < 0 || compare(assay, one)?.is_gt() {
            return Err(domain("dry-mass impurity fractions must be in [0, 1]"));
        }
    }
    let assays = [low_assay, high_assay];
    if max_grade.signum() <= 0 {
        return Err(domain("physical grade limit must be positive"));
    }
    for (bin, assay) in [(low, low_assay), (high, high_assay)] {
        let combined = bin
            .grade
            .checked_div(max_grade)
            .and_then(|metal_fraction| metal_fraction.checked_add(assay))
            .map_err(|_| arithmetic())?;
        if compare(combined, one)?.is_gt() {
            return Err(domain(
                "contained metal and impurity exceed the dry mass of a bin",
            ));
        }
    }
    let excess = [
        low_assay.checked_sub(maximum).map_err(|_| arithmetic())?,
        high_assay.checked_sub(maximum).map_err(|_| arithmetic())?,
    ];
    let mass = [one, one];
    let recovery = [low_recovered, high_recovered];
    let rows = [
        ([neg_one, zero], zero),
        ([zero, neg_one], zero),
        ([one, zero], low.dry_tonnes),
        ([zero, one], high.dry_tonnes),
        (mass, terms.mill_dry_tonnes_per_period),
        (recovery, terms.market_metal_units_per_period),
        (excess, zero),
    ];
    let mut best = [zero; 2];
    let mut best_dry = zero;
    let mut best_metal = zero;
    let mut best_contribution = zero;
    // The bounded polytope contains idle feed. A linear objective reaches
    // its maximum at a feasible intersection of two independent boundaries.
    for (i, &(a, upper_a)) in rows.iter().enumerate() {
        'vertex: for &(b, upper_b) in &rows[i + 1..] {
            let det = determinant(a, b)?;
            if det == zero {
                continue;
            }
            let rhs = [upper_a, upper_b];
            let x = determinant(rhs, [a[1], b[1]])?;
            let y = determinant([a[0], b[0]], rhs)?;
            let take = [
                x.checked_div(det).map_err(|_| arithmetic())?,
                y.checked_div(det).map_err(|_| arithmetic())?,
            ];
            for &(coefficients, upper) in &rows {
                if compare(dot(coefficients, take)?, upper)?.is_gt() {
                    continue 'vertex;
                }
            }
            let dry = dot(take, mass)?;
            let metal = dot(take, recovery)?;
            let contribution = terms.contribution(net_price, dry, metal)?;
            if compare(contribution, best_contribution)?.is_gt()
                || (contribution == best_contribution && compare(dry, best_dry)?.is_lt())
            {
                best = take;
                best_dry = dry;
                best_metal = metal;
                best_contribution = contribution;
            }
        }
    }
    let impurity_fraction = if best_dry == zero {
        None
    } else {
        Some(
            dot(best, assays)?
                .checked_div(best_dry)
                .map_err(|_| arithmetic())?,
        )
    };
    Ok(BlendAllocation {
        selected_bins: [
            GradeTonnageBin {
                dry_tonnes: best[0],
                ..low
            },
            GradeTonnageBin {
                dry_tonnes: best[1],
                ..high
            },
        ],
        dry_tonnes: best_dry,
        recovered_metal: best_metal,
        incremental_contribution: best_contribution,
        impurity_fraction,
    })
}
