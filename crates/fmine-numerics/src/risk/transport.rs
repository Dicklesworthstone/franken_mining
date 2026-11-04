//! DESIGN (BD-225, §22.5): finite-support Wasserstein worst expectations with exact certificates.
//! Determinism: Exact (checked rational arithmetic and canonical caller ordering).
//!
//! The ambiguity set transports the declared nominal weights within a declared proper metric and
//! cost budget. It adds no unseen states and estimates neither a radius nor calibrated uncertainty.
//! Callers own state identity, units, source binding and constraints over the complete original support.

use fmine_evidence::{Refusal, RefusalCode};
use fmine_units::{ExactQty as Rat, UnitsError};

use super::exact::compare;

pub const MAX_STATES: usize = 64;
const ZERO: Rat = Rat::from_integer(0);
const ONE: Rat = Rat::from_integer(1);
type Result<T> = std::result::Result<T, Refusal>;

fn invalid(why: &str) -> Refusal {
    Refusal::new(
        RefusalCode::OutsidePhysicalDomain109,
        format!("finite transport: {why}"),
    )
}

fn numerical(why: impl std::fmt::Display) -> Refusal {
    Refusal::new(
        RefusalCode::NumericalCertificateFailed207,
        format!("finite transport: {why}"),
    )
}

fn arithmetic<T>(value: std::result::Result<T, UnitsError>) -> Result<T> {
    value.map_err(numerical)
}

/// Immutable nominal law and proper metric, in the caller's canonical state order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransportPlan {
    weights: Vec<Rat>,
    costs: Vec<Vec<Rat>>,
    cost_order: Vec<Vec<usize>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransportArc {
    pub source: usize,
    pub target: usize,
    pub mass: Rat,
}

/// Exact primal/dual witnesses for max Σ γᵢⱼ vⱼ, with row mass pᵢ and Σ γᵢⱼ dᵢⱼ ≤ radius.
/// Destination weights retain every original state, including those receiving zero stressed mass.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransportCertificate {
    pub radius: Rat,
    pub worst_case_value: Rat,
    pub transport_cost: Rat,
    pub destination_weights: Vec<Rat>,
    pub arcs: Vec<TransportArc>,
    pub dual_lambda: Rat,
    pub dual_potentials: Vec<Rat>,
    pub dual_value: Rat,
    pub duality_gap: Rat,
}

struct Edge {
    source: usize,
    from: usize,
    to: usize,
    cost_step: Rat,
    slope: Rat,
}

impl TransportPlan {
    /// Checks all dimensions before allocation, then validates and sorts the metric once.
    /// Metric validation takes O(n³) comparisons; n is bounded by MAX_STATES.
    pub fn new(weights: &[Rat], costs: &[Vec<Rat>]) -> Result<Self> {
        let n = weights.len();
        if !(1..=MAX_STATES).contains(&n) {
            return Err(Refusal::new(
                RefusalCode::NoClaimInDomain201,
                "finite transport requires 1..=64 original states",
            ));
        }
        if costs.len() != n || costs.iter().any(|row| row.len() != n) {
            return Err(invalid(
                "costs must be a complete square matrix over the nominal law",
            ));
        }
        let mut total = ZERO;
        for &weight in weights {
            if weight.signum() <= 0 {
                return Err(invalid("nominal weights must be strictly positive"));
            }
            total = arithmetic(total.checked_add(weight))?;
        }
        if total != ONE {
            return Err(invalid("nominal weights must sum exactly to one"));
        }
        for (i, row) in costs.iter().enumerate() {
            for (j, &cost) in row.iter().enumerate() {
                if (i == j && cost != ZERO) || (i != j && cost.signum() <= 0) {
                    return Err(invalid(
                        "metric requires zero diagonal and positive distinct-state costs",
                    ));
                }
                if cost != costs[j][i] {
                    return Err(invalid("metric must be symmetric"));
                }
            }
        }
        for row in costs {
            for (j, &cost) in row.iter().enumerate() {
                for (k, &side) in row.iter().enumerate() {
                    // d(i,j) <= d(i,k) proves this triangle without adding two potentially huge
                    // costs. Otherwise d(i,j)-d(i,k) <= d(k,j) is the same exact inequality.
                    if compare(cost, side).is_gt()
                        && compare(arithmetic(cost.checked_sub(side))?, costs[k][j]).is_gt()
                    {
                        return Err(invalid("metric violates the triangle inequality"));
                    }
                }
            }
        }
        let cost_order = costs
            .iter()
            .map(|row| {
                let mut order: Vec<_> = (0..n).collect();
                order.sort_unstable_by(|&a, &b| compare(row[a], row[b]).then(a.cmp(&b)));
                order
            })
            .collect();
        Ok(Self {
            weights: weights.to_vec(),
            costs: costs.to_vec(),
            cost_order,
        })
    }

    fn edge(&self, source: usize, from: usize, to: usize, values: &[Rat]) -> Result<Edge> {
        let cost_step = arithmetic(self.costs[source][to].checked_sub(self.costs[source][from]))?;
        let gain = arithmetic(values[to].checked_sub(values[from]))?;
        Ok(Edge {
            source,
            from,
            to,
            cost_step,
            slope: arithmetic(gain.checked_div(cost_step))?,
        })
    }

    fn edges(&self, values: &[Rat]) -> Result<Vec<Edge>> {
        let n = self.weights.len();
        let mut edges = Vec::with_capacity(n * n);
        for source in 0..n {
            let row = &self.costs[source];
            let mut points: Vec<usize> = Vec::with_capacity(n);
            for &target in &self.cost_order[source] {
                if let Some(last) = points.last_mut()
                    && row[*last] == row[target]
                {
                    if compare(values[target], values[*last]).is_gt() {
                        *last = target;
                    }
                } else {
                    points.push(target);
                }
            }
            let mut hull: Vec<usize> = Vec::with_capacity(n);
            for target in points {
                if hull
                    .last()
                    .is_some_and(|&last| !compare(values[target], values[last]).is_gt())
                {
                    continue;
                }
                while hull.len() >= 2 {
                    let last = hull[hull.len() - 1];
                    let previous = hull[hull.len() - 2];
                    if compare(
                        self.edge(source, previous, last, values)?.slope,
                        self.edge(source, last, target, values)?.slope,
                    )
                    .is_gt()
                    {
                        break;
                    }
                    hull.pop();
                }
                hull.push(target);
            }
            for pair in hull.windows(2) {
                edges.push(self.edge(source, pair[0], pair[1], values)?);
            }
        }
        edges.sort_unstable_by(|a, b| {
            compare(b.slope, a.slope)
                .then(a.source.cmp(&b.source))
                .then(a.to.cmp(&b.to))
        });
        Ok(edges)
    }

    /// Maximizes an expected value over the finite-support Wasserstein ball.
    ///
    /// Each source row has a concave upper hull in (transport cost, destination value). Spending
    /// budget in descending hull slopes is the exact separable fractional-knapsack solution. Row
    /// slopes strictly decrease, so their order respects each row's preceding segments. Only the
    /// final segment can split a source mass: the witness has at most n+1 positive arcs.
    /// Work and memory are O(n² log n) and O(n²), with no iterative convergence tolerance.
    pub fn maximize(&self, values: &[Rat], radius: Rat) -> Result<TransportCertificate> {
        if values.len() != self.weights.len() || radius.signum() < 0 {
            return Err(invalid(
                "values must cover every original state and radius must be nonnegative",
            ));
        }
        let edges = self.edges(values)?;
        let mut targets: Vec<_> = (0..self.weights.len()).collect();
        let mut split = None;
        // Track spent cost rather than subtracting full segments from a potentially enormous slack
        // budget. Only the final partial segment needs a remaining-budget difference.
        let mut spent = ZERO;
        let mut lambda = ZERO;
        for edge in edges {
            if spent == radius {
                lambda = edge.slope;
                break;
            }
            let capacity = arithmetic(self.weights[edge.source].checked_mul(edge.cost_step))?;
            let next_cost = arithmetic(spent.checked_add(capacity))?;
            if compare(next_cost, radius).is_gt() {
                let remaining = arithmetic(radius.checked_sub(spent))?;
                let moved = arithmetic(remaining.checked_div(edge.cost_step))?;
                targets[edge.source] = edge.from;
                split = Some((edge.source, edge.to, moved));
                lambda = edge.slope;
                break;
            }
            targets[edge.source] = edge.to;
            spent = next_cost;
        }
        let mut arcs = Vec::with_capacity(self.weights.len() + 1);
        for (source, &target) in targets.iter().enumerate() {
            let mut mass = self.weights[source];
            if let Some((split_source, split_target, moved)) = split
                && split_source == source
            {
                mass = arithmetic(mass.checked_sub(moved))?;
                arcs.push(TransportArc {
                    source,
                    target: split_target,
                    mass: moved,
                });
            }
            arcs.push(TransportArc {
                source,
                target,
                mass,
            });
        }
        arcs.sort_unstable_by_key(|arc| (arc.source, arc.target));
        self.certify(values, radius, lambda, arcs)
    }

    // This checker uses only the original LP, not the hulls, marginal slopes or spent-budget state.
    fn certify(
        &self,
        values: &[Rat],
        radius: Rat,
        lambda: Rat,
        arcs: Vec<TransportArc>,
    ) -> Result<TransportCertificate> {
        let n = self.weights.len();
        let mut rows = vec![ZERO; n];
        let mut destinations = vec![ZERO; n];
        let (mut cost, mut primal) = (ZERO, ZERO);
        for arc in &arcs {
            if arc.source >= n || arc.target >= n || arc.mass.signum() <= 0 {
                return Err(numerical("invalid primal transport arc"));
            }
            rows[arc.source] = arithmetic(rows[arc.source].checked_add(arc.mass))?;
            destinations[arc.target] = arithmetic(destinations[arc.target].checked_add(arc.mass))?;
            cost = arithmetic(cost.checked_add(arithmetic(
                arc.mass.checked_mul(self.costs[arc.source][arc.target]),
            )?))?;
            primal = arithmetic(
                primal.checked_add(arithmetic(arc.mass.checked_mul(values[arc.target]))?),
            )?;
        }
        if rows != self.weights || compare(cost, radius).is_gt() || lambda.signum() < 0 {
            return Err(numerical("primal mass, budget or dual multiplier failed"));
        }
        let mut dual = arithmetic(lambda.checked_mul(radius))?;
        let mut potentials = Vec::with_capacity(n);
        let (mut destination_total, mut destination_value) = (ZERO, ZERO);
        for (i, row) in self.costs.iter().enumerate() {
            let mut potential = values[i];
            for (j, &distance) in row.iter().enumerate() {
                let bound =
                    arithmetic(values[j].checked_sub(arithmetic(lambda.checked_mul(distance))?))?;
                if compare(bound, potential).is_gt() {
                    potential = bound;
                }
            }
            potentials.push(potential);
            dual =
                arithmetic(dual.checked_add(arithmetic(self.weights[i].checked_mul(potential))?))?;
            destination_total = arithmetic(destination_total.checked_add(destinations[i]))?;
            destination_value = arithmetic(
                destination_value.checked_add(arithmetic(destinations[i].checked_mul(values[i]))?),
            )?;
        }
        if destination_total != ONE || destination_value != primal || dual != primal {
            return Err(numerical(
                "transport objective or exact primal/dual identity failed",
            ));
        }
        Ok(TransportCertificate {
            radius,
            worst_case_value: primal,
            transport_cost: cost,
            destination_weights: destinations,
            arcs,
            dual_lambda: lambda,
            dual_potentials: potentials,
            dual_value: dual,
            duality_gap: ZERO,
        })
    }
}

#[cfg(test)]
mod tests;
