//! DESIGN: exact worst upper-tail loss on the same declared finite transport ball.
//! Determinism: Exact
//! A bounded convex search must finish with matching primal/dual witnesses.

use super::*;
use std::collections::BTreeMap;

/// A hard bound, including the two endpoint evaluations. Exhaustion refuses an uncertified result.
pub const MAX_CVAR_ORACLE_CALLS: usize = 128;

/// A feasible stressed law and its worst-tail submass attain the same value as the global
/// Rockafellar--Uryasev/transport upper bound. `tail_masses` sum to `tail_mass`, not one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CvarCertificate {
    pub tail_mass: Rat,
    pub threshold: Rat,
    pub worst_case_cvar: Rat,
    pub tail_masses: Vec<Rat>,
    /// The objective is `(loss - threshold).max(0)`, in the caller's original state order.
    pub hinge_certificate: TransportCertificate,
    pub dual_value: Rat,
    pub duality_gap: Rat,
    pub oracle_calls: usize,
}

struct Point {
    threshold: Rat,
    value: Rat,
    lower_slope: Rat,
    upper_slope: Rat,
    certificate: TransportCertificate,
}

impl Point {
    fn stationary(&self) -> bool {
        self.lower_slope.signum() <= 0 && self.upper_slope.signum() >= 0
    }
    fn intercept(&self, slope: Rat) -> Result<Rat> {
        arithmetic(
            self.value
                .checked_sub(arithmetic(slope.checked_mul(self.threshold))?),
        )
    }
}

fn hinges(losses: &[Rat], threshold: Rat) -> Result<Vec<Rat>> {
    losses
        .iter()
        .map(|loss| {
            if compare(*loss, threshold).is_gt() {
                arithmetic(loss.checked_sub(threshold))
            } else {
                Ok(ZERO)
            }
        })
        .collect()
}

impl TransportPlan {
    fn tail_point(&self, losses: &[Rat], radius: Rat, alpha: Rat, threshold: Rat) -> Result<Point> {
        let certificate = self.maximize(&hinges(losses, threshold)?, radius)?;
        let (mut above, mut at_or_above) = (ZERO, ZERO);
        for (loss, mass) in losses.iter().zip(&certificate.destination_weights) {
            if compare(*loss, threshold).is_gt() {
                above = arithmetic(above.checked_add(*mass))?;
            }
            if !compare(*loss, threshold).is_lt() {
                at_or_above = arithmetic(at_or_above.checked_add(*mass))?;
            }
        }
        Ok(Point {
            threshold,
            value: arithmetic(
                threshold.checked_add(arithmetic(certificate.worst_case_value.checked_div(alpha))?),
            )?,
            lower_slope: arithmetic(ONE.checked_sub(arithmetic(at_or_above.checked_div(alpha))?))?,
            upper_slope: arithmetic(ONE.checked_sub(arithmetic(above.checked_div(alpha))?))?,
            certificate,
        })
    }

    /// Maximize the signed average loss in the upper `tail_mass` of every admissible stressed law.
    ///
    /// The threshold objective is `eta + max_transport E[(loss-eta)+] / tail_mass`. It is convex
    /// and piecewise linear. Exact supporting-line intersections bracket its minimum. If two
    /// different optimal laws supply opposite slopes, their exact mixture supplies a maximizing
    /// tail law; maximizing the mean loss first would not in general give this law.
    ///
    /// Every success independently checks a feasible transport witness, a tail submass bounded
    /// by its destination weights, and equality of that tail value with the global dual bound.
    /// Thus neither a convergence tolerance nor minimax interchange is trusted as a certificate.
    /// Work is bounded by `MAX_CVAR_ORACLE_CALLS` transport calls. Overflow or exhaustion refuses.
    pub fn maximize_cvar(
        &self,
        losses: &[Rat],
        radius: Rat,
        tail_mass: Rat,
    ) -> Result<CvarCertificate> {
        if losses.len() != self.weights.len()
            || radius.signum() < 0
            || tail_mass.signum() <= 0
            || compare(tail_mass, ONE).is_gt()
        {
            return Err(invalid(
                "CVaR requires every original loss, a nonnegative radius and tail mass in (0, 1]",
            ));
        }
        let low = losses
            .iter()
            .copied()
            .min_by(|a, b| compare(*a, *b))
            .ok_or_else(|| invalid("CVaR needs at least one loss"))?;
        let high = losses
            .iter()
            .copied()
            .max_by(|a, b| compare(*a, *b))
            .ok_or_else(|| invalid("CVaR needs at least one loss"))?;
        let mut left = self.tail_point(losses, radius, tail_mass, low)?;
        if left.stationary() {
            return self.certify_tail(losses, tail_mass, left, 1);
        }
        let mut right = self.tail_point(losses, radius, tail_mass, high)?;
        if right.stationary() {
            return self.certify_tail(losses, tail_mass, right, 2);
        }
        for calls in 3..=MAX_CVAR_ORACLE_CALLS {
            let (a, b) = (left.upper_slope, right.lower_slope);
            if a.signum() >= 0 || b.signum() <= 0 {
                return Err(numerical("CVaR supporting slopes do not bracket zero"));
            }
            let left_intercept = left.intercept(a)?;
            let right_intercept = right.intercept(b)?;
            let threshold = arithmetic(
                arithmetic(left_intercept.checked_sub(right_intercept))?
                    .checked_div(arithmetic(b.checked_sub(a))?),
            )?;
            if compare(threshold, left.threshold).is_lt()
                || compare(threshold, right.threshold).is_gt()
            {
                return Err(numerical(
                    "CVaR supporting intersection left its threshold bracket",
                ));
            }
            let lower =
                arithmetic(left_intercept.checked_add(arithmetic(a.checked_mul(threshold))?))?;
            let mut point = self.tail_point(losses, radius, tail_mass, threshold)?;
            if point.stationary() {
                return self.certify_tail(losses, tail_mass, point, calls);
            }
            match compare(point.value, lower) {
                std::cmp::Ordering::Less => {
                    return Err(numerical("CVaR oracle violates its supporting bound"));
                }
                std::cmp::Ordering::Equal => {
                    // Both supporting laws attain the hinge maximum at this threshold. Combine
                    // their slopes to zero, then recheck the mixed arcs against the original LP.
                    let weight = arithmetic(b.checked_div(arithmetic(b.checked_sub(a))?))?;
                    let complement = arithmetic(ONE.checked_sub(weight))?;
                    let mut arcs = BTreeMap::new();
                    for (law, share) in [
                        (&left.certificate, weight),
                        (&right.certificate, complement),
                    ] {
                        for arc in &law.arcs {
                            let mass = arithmetic(arc.mass.checked_mul(share))?;
                            let entry = arcs.entry((arc.source, arc.target)).or_insert(ZERO);
                            *entry = arithmetic(entry.checked_add(mass))?;
                        }
                    }
                    point.certificate = self.certify(
                        &hinges(losses, threshold)?,
                        radius,
                        point.certificate.dual_lambda,
                        arcs.into_iter()
                            .filter(|(_, mass)| mass.signum() > 0)
                            .map(|((source, target), mass)| TransportArc {
                                source,
                                target,
                                mass,
                            })
                            .collect(),
                    )?;
                    return self.certify_tail(losses, tail_mass, point, calls);
                }
                std::cmp::Ordering::Greater => {
                    if point.upper_slope.signum() < 0 {
                        left = point;
                    } else {
                        right = point;
                    }
                }
            }
        }
        Err(numerical(
            "CVaR exhausted its exact transport-oracle budget without a certificate",
        ))
    }

    fn certify_tail(
        &self,
        losses: &[Rat],
        alpha: Rat,
        point: Point,
        calls: usize,
    ) -> Result<CvarCertificate> {
        let hinge = self.certify(
            &hinges(losses, point.threshold)?,
            point.certificate.radius,
            point.certificate.dual_lambda,
            point.certificate.arcs,
        )?;
        let mut order: Vec<_> = (0..losses.len()).collect();
        order.sort_by(|&a, &b| compare(losses[b], losses[a]).then(a.cmp(&b)));
        let mut remaining = alpha;
        let mut tail_masses = vec![ZERO; losses.len()];
        let mut weighted = ZERO;
        for i in order {
            let mass = if compare(remaining, hinge.destination_weights[i]).is_lt() {
                remaining
            } else {
                hinge.destination_weights[i]
            };
            tail_masses[i] = mass;
            weighted = arithmetic(weighted.checked_add(arithmetic(mass.checked_mul(losses[i]))?))?;
            remaining = arithmetic(remaining.checked_sub(mass))?;
        }
        let primal = arithmetic(weighted.checked_div(alpha))?;
        let dual = arithmetic(
            point
                .threshold
                .checked_add(arithmetic(hinge.dual_value.checked_div(alpha))?),
        )?;
        if remaining != ZERO || primal != dual || dual != point.value {
            return Err(numerical(
                "CVaR feasible tail submass does not attain the global transport/threshold bound",
            ));
        }
        Ok(CvarCertificate {
            tail_mass: alpha,
            threshold: point.threshold,
            worst_case_cvar: primal,
            tail_masses,
            hinge_certificate: hinge,
            dual_value: dual,
            duality_gap: ZERO,
            oracle_calls: calls,
        })
    }
}

#[cfg(test)]
mod tests;
