//! Exact finite weighted payoff distributions for rational-valued objectives (BD-229, PO-CAP-2).
//! Determinism: Exact (checked rational arithmetic, canonical ordering, no rounding or sampling).
//!
//! Values are unitless to this mathematical kernel. Callers retain the quantity's units, scenario
//! identities, feasibility constraints and evidence. Positive payoff means gain; tail risk is signed
//! loss = -payoff, so an all-gain population has negative tail loss.

use std::cmp::Ordering;

use fmine_evidence::{Refusal, RefusalCode};
use fmine_units::{ExactQty as Rat, UnitsError};

pub const MAX_OUTCOMES: usize = 4096;
const ZERO: Rat = Rat::from_integer(0);
const ONE: Rat = Rat::from_integer(1);
type Result<T> = std::result::Result<T, Refusal>;

fn invalid(message: &str) -> Refusal {
    Refusal::new(
        RefusalCode::OutsidePhysicalDomain109,
        format!("exact weighted distribution: {message}"),
    )
}

fn arithmetic<T>(result: std::result::Result<T, UnitsError>) -> Result<T> {
    result.map_err(|error| {
        Refusal::new(
            RefusalCode::NumericalCertificateFailed207,
            format!("exact weighted distribution: {error}"),
        )
    })
}

/// Orders any two representable rational values without overflowing intermediate cross products.
/// The caller owns their common unit; comparison never converts or rounds either value.
#[must_use]
pub fn compare(a: Rat, b: Rat) -> Ordering {
    // Equal denominators need no cross product, including large exact integer payoffs.
    if a.den() == b.den() {
        a.num().cmp(&b.num())
    } else if a.signum() != b.signum() {
        a.signum().cmp(&b.signum())
    } else if let Ok(order) = a.checked_cmp(b) {
        order
    } else {
        // Euclidean continued fractions compare integer parts, then reciprocals of the remainders.
        // Each reciprocal reverses the order. Unsigned magnitudes also admit i128::MIN exactly.
        let (mut an, mut ad) = (a.num().unsigned_abs(), a.den().unsigned_abs());
        let (mut bn, mut bd) = (b.num().unsigned_abs(), b.den().unsigned_abs());
        let mut reverse = a.signum() < 0;
        loop {
            let order = (an / ad).cmp(&(bn / bd));
            if !order.is_eq() {
                return if reverse { order.reverse() } else { order };
            }
            let (ar, br) = (an % ad, bn % bd);
            if ar == 0 || br == 0 {
                let order = ar.cmp(&br);
                return if reverse { order.reverse() } else { order };
            }
            (an, ad) = (ad, ar);
            (bn, bd) = (bd, br);
            reverse = !reverse;
        }
    }
}

fn sorted(values: &[(Rat, Rat)]) -> Vec<(Rat, Rat)> {
    let mut values = values.to_vec();
    // The exact comparator permits the standard allocation-free O(n log n) sort. Weight order only
    // canonicalizes payoff ties; its numeric order has no economic meaning in this marginal law.
    values.sort_unstable_by(|a, b| {
        compare(a.0, b.0).then_with(|| (a.1.num(), a.1.den()).cmp(&(b.1.num(), b.1.den())))
    });
    values
}

/// An immutable complete finite payoff law with exact strictly positive unit-sum weights.
/// No input weight is inferred, normalized, truncated or rounded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExactDistribution {
    values: Vec<(Rat, Rat)>,
    mean: Rat,
}

impl ExactDistribution {
    /// Admits `(payoff, weight)` pairs and sorts them canonically once for repeated tail/quantile queries.
    /// Values retain arbitrary exact rational precision; they never pass through fixed-decimal Money.
    pub fn new(values: &[(Rat, Rat)]) -> Result<Self> {
        if values.is_empty() || values.len() > MAX_OUTCOMES {
            return Err(Refusal::new(
                RefusalCode::NoClaimInDomain201,
                format!("exact weighted distribution requires 1..={MAX_OUTCOMES} outcomes"),
            ));
        }
        if values.iter().any(|(_, weight)| weight.signum() <= 0) {
            return Err(invalid("weights must be strictly positive"));
        }
        let values = sorted(values);
        let mut mass = ZERO;
        for (_, weight) in &values {
            mass = arithmetic(mass.checked_add(*weight))?;
        }
        if mass != ONE {
            return Err(invalid("weights must sum exactly to one"));
        }
        // Equal payoff atoms are one point in the marginal law. Combine their weights before
        // multiplying, avoiding needless overflow and repeated work without changing any mass.
        let mut combined: Vec<(Rat, Rat)> = Vec::with_capacity(values.len());
        for (payoff, weight) in values {
            if let Some((previous, total)) = combined.last_mut()
                && *previous == payoff
            {
                *total = arithmetic(total.checked_add(weight))?;
            } else {
                combined.push((payoff, weight));
            }
        }
        let values = combined;
        let mut mean = ZERO;
        for (payoff, weight) in &values {
            mean = arithmetic(mean.checked_add(arithmetic(payoff.checked_mul(*weight))?))?;
        }
        Ok(Self { values, mean })
    }

    /// The exact weighted mean payoff, in the caller's original payoff unit.
    #[must_use]
    pub fn mean(&self) -> Rat {
        self.mean
    }

    /// Weighted inverse CDF: the first payoff whose cumulative mass reaches `probability` in `(0,1]`.
    pub fn quantile(&self, probability: Rat) -> Result<Rat> {
        if probability.signum() <= 0 || compare(probability, ONE).is_gt() {
            return Err(invalid("quantile probability must lie in (0,1]"));
        }
        let mut mass = ZERO;
        for (payoff, weight) in &self.values {
            mass = arithmetic(mass.checked_add(*weight))?;
            if !compare(mass, probability).is_lt() {
                return Ok(*payoff);
            }
        }
        Err(invalid(
            "admitted distribution lacks the requested quantile mass",
        ))
    }

    /// Mean signed loss in the worst `tail_mass` of payoff outcomes, with exact fractional boundary mass.
    /// `tail_mass = 1/20` gives CVaR95; a full-population tail gives the negative mean payoff.
    pub fn tail_loss(&self, tail_mass: Rat) -> Result<Rat> {
        if tail_mass.signum() <= 0 || compare(tail_mass, ONE).is_gt() {
            return Err(invalid("tail mass must lie in (0,1]"));
        }
        let (mut remaining, mut integral) = (tail_mass, ZERO);
        for (payoff, weight) in &self.values {
            if remaining == ZERO {
                break;
            }
            let take = if compare(*weight, remaining).is_gt() {
                remaining
            } else {
                *weight
            };
            integral = arithmetic(integral.checked_add(arithmetic(
                arithmetic(payoff.checked_neg())?.checked_mul(take),
            )?))?;
            remaining = arithmetic(remaining.checked_sub(take))?;
        }
        if remaining != ZERO {
            return Err(invalid(
                "admitted distribution lacks the requested tail mass",
            ));
        }
        arithmetic(integral.checked_div(tail_mass))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(num: i128, den: i128) -> Rat {
        Rat::new(num, den).unwrap()
    }

    #[test]
    fn exact_per_share_values_keep_nonterminating_fractions_and_split_tail_mass() {
        let d = ExactDistribution::new(&[
            (r(-1, 3), r(1, 10)),
            (r(-1, 6), r(1, 5)),
            (r(2, 9), r(7, 10)),
        ])
        .unwrap();
        assert_eq!(d.mean(), r(4, 45));
        assert_eq!(d.quantile(r(1, 10)).unwrap(), r(-1, 3));
        assert_eq!(d.quantile(r(3, 10)).unwrap(), r(-1, 6));
        assert_eq!(d.quantile(r(1, 2)).unwrap(), r(2, 9));
        assert_eq!(d.quantile(r(9, 10)).unwrap(), r(2, 9));
        assert_eq!(d.quantile(ONE).unwrap(), r(2, 9));
        assert_eq!(d.tail_loss(r(1, 5)).unwrap(), r(1, 4));
        assert_eq!(d.tail_loss(r(1, 20)).unwrap(), r(1, 3));
        assert_eq!(d.tail_loss(ONE).unwrap(), r(-4, 45));
    }

    #[test]
    fn singleton_and_all_gain_distributions_keep_negative_signed_tail_loss() {
        for payoff in [r(2, 9), ZERO, r(-1, 3)] {
            let d = ExactDistribution::new(&[(payoff, ONE)]).unwrap();
            assert_eq!(d.mean(), payoff);
            for mass in [r(1, 1000), r(1, 20), r(1, 2), ONE] {
                assert_eq!(d.quantile(mass).unwrap(), payoff);
                assert_eq!(d.tail_loss(mass).unwrap(), payoff.checked_neg().unwrap());
            }
        }
    }

    #[test]
    fn permutation_and_splitting_equal_payoff_mass_preserve_the_law() {
        let mut rows = vec![(r(-1, 3), r(1, 3)), (r(2, 9), r(1, 6)), (r(2, 9), r(1, 2))];
        let original = ExactDistribution::new(&rows).unwrap();
        rows.reverse();
        assert_eq!(ExactDistribution::new(&rows).unwrap(), original);
        rows.rotate_left(1);
        assert_eq!(ExactDistribution::new(&rows).unwrap(), original);
        let merged = ExactDistribution::new(&[(r(-1, 3), r(1, 3)), (r(2, 9), r(2, 3))]).unwrap();
        assert_eq!(merged, original);
        assert_eq!(merged.mean(), original.mean());
        for n in 1..=30 {
            let mass = r(n, 30);
            assert_eq!(
                merged.quantile(mass).unwrap(),
                original.quantile(mass).unwrap()
            );
            assert_eq!(
                merged.tail_loss(mass).unwrap(),
                original.tail_loss(mass).unwrap()
            );
        }
    }

    #[test]
    fn tail_integral_matches_an_independent_equal_mass_expansion() {
        // Expand integer masses into equiprobable atoms, then sum selected losses directly.
        // This oracle shares neither the production sorting implementation nor its fractional walk.
        for a in -3..=3 {
            for b in -2..=2 {
                for c in -1..=1 {
                    let values = [r(a, 3), r(b, 7), r(c, 11)];
                    let d = ExactDistribution::new(&[
                        (values[0], r(1, 6)),
                        (values[1], r(1, 3)),
                        (values[2], r(1, 2)),
                    ])
                    .unwrap();
                    let mut atoms = [
                        values[0], values[1], values[1], values[2], values[2], values[2],
                    ];
                    atoms.sort_by(|x, y| x.checked_cmp(*y).unwrap());
                    let sum = atoms
                        .iter()
                        .try_fold(ZERO, |s, v| s.checked_add(*v))
                        .unwrap();
                    assert_eq!(d.mean(), sum.checked_div(r(6, 1)).unwrap());
                    for n in 1..=6 {
                        let mass = r(n as i128, 6);
                        let sum = atoms[..n]
                            .iter()
                            .try_fold(ZERO, |s, v| s.checked_add(*v))
                            .unwrap();
                        let expected = sum
                            .checked_div(r(n as i128, 1))
                            .unwrap()
                            .checked_neg()
                            .unwrap();
                        assert_eq!(d.tail_loss(mass).unwrap(), expected);
                        assert_eq!(d.quantile(mass).unwrap(), atoms[n - 1]);
                    }
                }
            }
        }
    }

    #[test]
    fn invalid_populations_and_probabilities_are_never_normalized_or_dropped() {
        for rows in [
            vec![],
            vec![(ONE, ZERO)],
            vec![(ONE, r(-1, 2))],
            vec![(ONE, r(9, 10))],
            vec![(ONE, r(11, 10))],
        ] {
            assert!(ExactDistribution::new(&rows).is_err());
        }
        let d = ExactDistribution::new(&[(ONE, ONE)]).unwrap();
        for p in [ZERO, r(-1, 2), r(11, 10)] {
            assert!(d.quantile(p).is_err());
            assert!(d.tail_loss(p).is_err());
        }
        let rows = vec![(ONE, r(1, MAX_OUTCOMES as i128)); MAX_OUTCOMES];
        assert_eq!(ExactDistribution::new(&rows).unwrap().mean(), ONE);
        let too_many = vec![(ONE, r(1, (MAX_OUTCOMES + 1) as i128)); MAX_OUTCOMES + 1];
        assert_eq!(
            ExactDistribution::new(&too_many).unwrap_err().code,
            "FMINE-REF-201"
        );
    }

    #[test]
    fn unrepresentable_means_and_losses_refuse_exactly() {
        let large = r(i128::MAX, 2);
        assert_eq!(
            ExactDistribution::new(&[(large, r(1, 2)), (r(i128::MAX - 1, 3), r(1, 2))])
                .unwrap_err()
                .code,
            "FMINE-REF-207"
        );
        assert_eq!(
            ExactDistribution::new(&[(r(i128::MAX, 1), r(2, 3)), (ZERO, r(1, 3))])
                .unwrap_err()
                .code,
            "FMINE-REF-207"
        );
        let minimum = ExactDistribution::new(&[(r(i128::MIN, 1), ONE)]).unwrap();
        assert_eq!(minimum.tail_loss(ONE).unwrap_err().code, "FMINE-REF-207");
        // Equal-denominator ordering avoids a needless cross product for representable large inputs.
        let d = ExactDistribution::new(&[(large, r(1, 2)), (large, r(1, 2))]).unwrap();
        assert_eq!(d.mean(), large);
    }

    #[test]
    fn exact_comparison_handles_large_denominators_and_signed_extremes() {
        let m = i128::MAX;
        let a = r(m - 1, m);
        let b = r(m - 2, m - 1);
        assert!(a.checked_cmp(b).is_err());
        assert_eq!(compare(a, b), Ordering::Greater);
        assert_eq!(compare(b, a), Ordering::Less);
        assert_eq!(compare(a, a), Ordering::Equal);
        assert_eq!(
            compare(a.checked_neg().unwrap(), b.checked_neg().unwrap()),
            Ordering::Less
        );
        assert_eq!(compare(r(i128::MIN, m), r(-m, m - 1)), Ordering::Greater);
        assert_eq!(compare(r(i128::MIN, 1), r(-m, 2)), Ordering::Less);
        assert_eq!(compare(r(i128::MIN, 1), r(m, 1)), Ordering::Less);

        // Both fractions are realizable NAV/share deltas with billions of actual shares.
        let delta = r(6_000_000_142_009_998_129, 297_000_001_293_000_001_400);
        let better = r(2_571_428_717_575_712_683, 127_285_714_797_000_000_500);
        assert!(delta.checked_cmp(delta).is_err());
        assert_eq!(compare(delta, delta), Ordering::Equal);
        assert!(delta.checked_cmp(better).is_err());
        assert_eq!(compare(delta, better), Ordering::Less);
        assert_eq!(compare(better, delta), Ordering::Greater);
        assert_eq!(
            compare(delta.checked_neg().unwrap(), better.checked_neg().unwrap()),
            Ordering::Greater
        );
        for an in -12..=12 {
            for ad in 1..=17 {
                for bn in -12..=12 {
                    for bd in 1..=17 {
                        let (a, b) = (r(an, ad), r(bn, bd));
                        assert_eq!(compare(a, b), a.checked_cmp(b).unwrap());
                    }
                }
            }
        }
    }
}
