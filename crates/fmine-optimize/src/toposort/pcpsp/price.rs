//! Determinism: BitwisePortable (IEEE `+ − × ÷` and comparisons on `f64`, in a fixed order; no libm, hashing or
//! threads). The floats only guide: [`super::destinations`] applies a result only after checking it in `i128`.
//!
//! **Price**, the third destination move of [`super::destinations`] (`bd-rbxh`). It reassigns all of one period's
//! mined blocks at once, where switches and exchanges, which move one block or two, have stopped. On MineLib's w23,
//! whose four blending rows have limit 0 and signed uses, moving value into the mill takes many blocks moving together.
//!
//! 1. **Prices.** The period's destination LP sends each mined block to destinations in shares that sum to 1 and
//!    keeps every limit. Dantzig–Wolfe decomposition solves it. The master has one row per resource plus a convexity
//!    row, and each column is a whole assignment, given by its value and its use of each resource.
//!    - The first column is the current assignment, which keeps every limit, so the master starts feasible.
//!    - A dense revised simplex solves the master, with each row and the objective scaled by its largest magnitude.
//!    - Its row duals `λ ≥ 0` price the resources. The next column sends each block to the destination with the
//!      largest `value − λ · use`.
//!    - Columns are added until the new one's reduced cost is at most 10⁻⁹ of the largest column value, or there are
//!      [`PRICE_COLUMNS`].
//! 2. **Candidates.** Each candidate is a whole assignment, in this order:
//!    - At the last prices, each block goes to the destination with the largest `value − λ · use`, its current one
//!      among equals, then the lowest. An optimal LP point splits at most one block per binding row. On MineLib's w23,
//!      at exact LP duals, this assignment broke a limit by only one or two blocks' uses.
//!    - Exact prices tie wherever the LP splits a block, and the tie-break may then pick badly. So the master's last
//!      mixture is also rounded: each block goes to the destination with the largest total weight among the mixture's
//!      columns, its current one among equals, then the lowest.
//!    - Then each column of that mixture.
//! 3. **Repair**, in `i128`, of each candidate. While a limit is broken, one block moves to another destination.
//!    - The move must lower the excess over some limit and raise none.
//!    - Among such moves, it loses the least value per unit of excess removed. Each row's excess is measured in units of
//!      the largest use of that row by one block of the period.
//!    - At most [`REPAIR_MOVES`] moves.
//! 4. **Acceptance.** The repaired candidate worth the most, the first of equals, is returned. It must keep every limit
//!    and be worth strictly more than the current assignment.
//!
//! The simplex has no anti-cycling rule. A solve that reaches [`MASTER_STEPS`] steps or meets a pivot below 10⁻¹²
//! ends the pricing, and the last prices are used. Such a stop can only weaken the prices; step 4 still applies in
//! full.

use fmine_numerics::exact_feas::Rat;

use super::PcpspModel;

/// The most master columns, the current assignment's included.
pub const PRICE_COLUMNS: usize = 256;

/// The most simplex steps in one solve of the master.
pub const MASTER_STEPS: usize = 1024;

/// The most single-block moves in one repair.
pub const REPAIR_MOVES: usize = 256;

/// The simplex's least reduced cost on scaled data, and the pricing's stopping rule relative to the column values.
const TOLERANCE: f64 = 1e-9;

/// The smallest pivot the simplex accepts, on scaled data.
const PIVOT: f64 = 1e-12;

/// Exact reconstruction is optional guidance. Bound its dense rational work;
/// larger masters retain the existing whole-assignment proposals.
const EXACT_RESOURCES: usize = 32;

/// One solve of the master: a price per unit of each resource, the convexity row's dual, the scale of the column
/// values, and the optimal mixture as `(assignment, weight)` pairs with a positive weight, in basis order.
struct Prices {
    lambda: Vec<f64>,
    mu: f64,
    scale: f64,
    mixture: Vec<(usize, f64)>,
    basis: Vec<usize>,
}

/// The master: assignment columns, each its value and its use of every resource, and a basis kept between solves.
/// Column `j < R` is row `j`'s slack; column `R + k` is assignment `k`. Row `R` is the convexity row.
struct Master<'a> {
    caps: &'a [i128],
    columns: Vec<(f64, Vec<f64>)>,
    basis: Vec<usize>,
}

impl Master<'_> {
    /// Solves the master from the current basis; `None` at a pivot below [`PIVOT`] or after [`MASTER_STEPS`] steps.
    fn solve(&mut self) -> Option<Prices> {
        let rows = self.caps.len();
        let row_scale: Vec<f64> = self
            .caps
            .iter()
            .enumerate()
            .map(|(r, &cap)| {
                self.columns
                    .iter()
                    .fold((cap as f64).abs().max(1.0), |s, (_, u)| s.max(u[r].abs()))
            })
            .collect();
        let scale = self
            .columns
            .iter()
            .fold(1.0_f64, |s, (v, _)| s.max(v.abs()));
        // A column's scaled cost and entries.
        let column = |j: usize| -> (f64, Vec<f64>) {
            match j.checked_sub(rows) {
                None => {
                    let mut e = vec![0.0; rows + 1];
                    e[j] = 1.0;
                    (0.0, e)
                }
                Some(k) => {
                    let (v, u) = &self.columns[k];
                    let mut e: Vec<f64> = u.iter().zip(&row_scale).map(|(x, s)| x / s).collect();
                    e.push(1.0);
                    (v / scale, e)
                }
            }
        };
        let rhs: Vec<f64> = self
            .caps
            .iter()
            .zip(&row_scale)
            .map(|(&cap, s)| cap as f64 / s)
            .chain([1.0])
            .collect();
        for _ in 0..MASTER_STEPS {
            let basic: Vec<(f64, Vec<f64>)> = self.basis.iter().map(|&j| column(j)).collect();
            let inverse = invert(&basic)?;
            let x: Vec<f64> = inverse.iter().map(|row| dot(row, &rhs)).collect();
            let pi: Vec<f64> = (0..=rows)
                .map(|k| {
                    basic
                        .iter()
                        .zip(&inverse)
                        .map(|((c, _), row)| c * row[k])
                        .sum()
                })
                .collect();
            let mut entering: Option<(f64, usize)> = None;
            for j in (0..rows + self.columns.len()).filter(|j| !self.basis.contains(j)) {
                let (c, a) = column(j);
                let reduced = c - dot(&pi, &a);
                if reduced > entering.map_or(TOLERANCE, |(top, _)| top) {
                    entering = Some((reduced, j));
                }
            }
            let Some((_, q)) = entering else {
                return Some(Prices {
                    lambda: pi
                        .iter()
                        .zip(&row_scale)
                        .map(|(p, s)| p * scale / s)
                        .collect(),
                    mu: pi[rows] * scale,
                    scale,
                    mixture: self
                        .basis
                        .iter()
                        .zip(&x)
                        .filter_map(|(&j, &w)| {
                            j.checked_sub(rows).filter(|_| w > 0.0).map(|k| (k, w))
                        })
                        .collect(),
                    basis: self.basis.clone(),
                });
            };
            let a = column(q).1;
            let mut leaving: Option<(f64, usize)> = None;
            for (i, (row, &xi)) in inverse.iter().zip(&x).enumerate() {
                let step = dot(row, &a);
                if step > PIVOT {
                    // A basic value a rounding error below 0 is read as 0.
                    let ratio = xi.max(0.0) / step;
                    if leaving.is_none_or(|(least, _)| ratio < least) {
                        leaving = Some((ratio, i));
                    }
                }
            }
            // The convexity row bounds every column, so some row limits the step.
            let (_, i) = leaving?;
            self.basis[i] = q;
        }
        None
    }
}

/// `Σ a[i] b[i]`, in index order.
fn dot(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

/// The rows of `B⁻¹`, for `B` given by its columns (each a cost and its entries), by Gauss–Jordan elimination with
/// partial pivoting; `None` at a pivot below [`PIVOT`].
fn invert(basic: &[(f64, Vec<f64>)]) -> Option<Vec<Vec<f64>>> {
    let m = basic.len();
    let mut a: Vec<Vec<f64>> = (0..m)
        .map(|i| basic.iter().map(|(_, e)| e[i]).collect())
        .collect();
    let mut inverse: Vec<Vec<f64>> = (0..m)
        .map(|i| (0..m).map(|k| if i == k { 1.0 } else { 0.0 }).collect())
        .collect();
    for k in 0..m {
        let p = (k..m).fold(k, |p, i| if a[i][k].abs() > a[p][k].abs() { i } else { p });
        if a[p][k].abs() < PIVOT {
            return None;
        }
        a.swap(k, p);
        inverse.swap(k, p);
        let pivot = a[k][k];
        a[k].iter_mut().for_each(|x| *x /= pivot);
        inverse[k].iter_mut().for_each(|x| *x /= pivot);
        let (ak, ik) = (a[k].clone(), inverse[k].clone());
        for (i, (ai, ii)) in a.iter_mut().zip(inverse.iter_mut()).enumerate() {
            if i != k {
                let f = ai[k];
                ai.iter_mut().zip(&ak).for_each(|(x, y)| *x -= f * y);
                ii.iter_mut().zip(&ik).for_each(|(x, y)| *x -= f * y);
            }
        }
    }
    Some(inverse)
}

/// The exact value of `blocks` sent to `pick` (in order), and their use of each resource.
fn column(model: &PcpspModel, blocks: &[usize], pick: &[usize]) -> (i128, Vec<i128>) {
    let dn = model.destinations;
    let value = blocks
        .iter()
        .zip(pick)
        .map(|(&b, &d)| i128::from(model.values[b * dn + d]))
        .sum();
    let used = model
        .uses
        .iter()
        .map(|row| {
            blocks
                .iter()
                .zip(pick)
                .map(|(&b, &d)| i128::from(row[b * dn + d]))
                .sum()
        })
        .collect();
    (value, used)
}

/// Each block's destination with the largest `value − λ · use`: its current one among equals, then the lowest.
fn assign(model: &PcpspModel, blocks: &[usize], destination: &[u32], lambda: &[f64]) -> Vec<usize> {
    let dn = model.destinations;
    blocks
        .iter()
        .map(|&b| {
            let priced = |d: usize| {
                model.values[b * dn + d] as f64
                    - model
                        .uses
                        .iter()
                        .zip(lambda)
                        .map(|(row, l)| l * row[b * dn + d] as f64)
                        .sum::<f64>()
            };
            let current = destination[b] as usize;
            (0..dn)
                .fold((current, priced(current)), |(best, top), d| {
                    let p = priced(d);
                    if p > top { (d, p) } else { (best, top) }
                })
                .0
        })
        .collect()
}

/// Step 3 of the module notes, changing `pick` in place. `None` when no move lowers an excess without raising
/// another, or a limit is still broken after [`REPAIR_MOVES`] moves.
fn repair(model: &PcpspModel, blocks: &[usize], pick: &mut [usize], caps: &[i128]) -> Option<()> {
    let dn = model.destinations;
    let (_, mut used) = column(model, blocks, pick);
    let unit: Vec<f64> = model
        .uses
        .iter()
        .map(|row| {
            let most = blocks
                .iter()
                .flat_map(|&b| (0..dn).map(move |d| row[b * dn + d].unsigned_abs()))
                .max()
                .unwrap_or(0);
            most.max(1) as f64
        })
        .collect();
    for _ in 0..=REPAIR_MOVES {
        let excess: Vec<i128> = used.iter().zip(caps).map(|(u, c)| (u - c).max(0)).collect();
        if excess.iter().all(|&e| e == 0) {
            return Some(());
        }
        let mut best: Option<(f64, usize, usize)> = None;
        for (i, (&b, &c)) in blocks.iter().zip(pick.iter()).enumerate() {
            for d in (0..dn).filter(|&d| d != c) {
                let (mut lowered, mut raised, mut removed) = (false, false, 0.0);
                for (((row, u), cap), (e, w)) in model
                    .uses
                    .iter()
                    .zip(&used)
                    .zip(caps)
                    .zip(excess.iter().zip(&unit))
                {
                    let delta = i128::from(row[b * dn + d]) - i128::from(row[b * dn + c]);
                    let after = (u + delta - cap).max(0);
                    lowered |= after < *e;
                    raised |= after > *e;
                    removed += (e - after) as f64 / w;
                }
                if raised || !lowered {
                    continue;
                }
                let loss =
                    i128::from(model.values[b * dn + c]) - i128::from(model.values[b * dn + d]);
                let ratio = loss as f64 / removed;
                if best.is_none_or(|(least, _, _)| ratio < least) {
                    best = Some((ratio, i, d));
                }
            }
        }
        let (_, i, d) = best?;
        let (b, c) = (blocks[i], pick[i]);
        for (u, row) in used.iter_mut().zip(&model.uses) {
            *u += i128::from(row[b * dn + d]) - i128::from(row[b * dn + c]);
        }
        pick[i] = d;
    }
    None
}

/// A float-priced basis is only a candidate. Solve it again from integer
/// resource columns, including its slacks, and reject negative basic values.
/// Singular or unrepresentable arithmetic supplies no mixture, never an
/// approximately feasible set of weights.
fn exact_basis(columns: &[Vec<i128>], caps: &[i128], basis: &[usize]) -> Option<Vec<Rat>> {
    let rows = caps.len();
    if rows > EXACT_RESOURCES || basis.len() != rows + 1 {
        return None;
    }
    let zero = Rat::from_integer(0);
    let one = Rat::from_integer(1);
    let mut a = vec![vec![zero; rows + 2]; rows + 1];
    for (j, &column) in basis.iter().enumerate() {
        if column < rows {
            a[column][j] = one;
        } else {
            let uses = columns.get(column - rows)?;
            if uses.len() != rows {
                return None;
            }
            for (row, &used) in a.iter_mut().zip(uses) {
                row[j] = Rat::from_integer(used);
            }
            a[rows][j] = one;
        }
    }
    for (row, &cap) in a.iter_mut().zip(caps) {
        row[rows + 1] = Rat::from_integer(cap);
    }
    a[rows][rows + 1] = one;
    for k in 0..=rows {
        let pivot = (k..=rows).find(|&i| a[i][k].signum() != 0)?;
        a.swap(k, pivot);
        let divisor = a[k][k];
        for value in &mut a[k][k..] {
            *value = value.checked_div(divisor).ok()?;
        }
        let pivot_row = a[k].clone();
        for (i, row) in a.iter_mut().enumerate().filter(|(i, _)| *i != k) {
            let multiplier = row[k];
            if multiplier.signum() == 0 {
                continue;
            }
            for (value, &other) in row[k..].iter_mut().zip(&pivot_row[k..]) {
                *value = value
                    .checked_sub(other.checked_mul(multiplier).ok()?)
                    .ok()?;
            }
            debug_assert_eq!(row[k], zero, "row {i}");
        }
    }
    let weights: Vec<_> = a.iter().map(|row| row[rows + 1]).collect();
    weights.iter().all(|w| w.signum() >= 0).then_some(weights)
}

/// Whole assignments still feed repair and line search. Exact destination
/// shares additionally preserve mixtures whose useful columns cannot be used
/// separately, for example opposite signed-blend directions at a tight limit.
pub(super) struct Proposals {
    pub directions: Vec<Vec<usize>>,
    pub shares: Option<Vec<Vec<Rat>>>,
}

/// Deterministic pre-repair proposals: the last priced assignment, the mixture's
/// modal assignment, then its active columns in basis order. At most resources
/// plus three assignments are retained. Whole repair and exact fractional
/// interpolation share this pricing pass; neither treats its floats as a
/// feasibility certificate. A tied final argmax may be unprofitable while an
/// active column still provides a profitable fractional direction.
pub(super) fn proposals(
    model: &PcpspModel,
    blocks: &[usize],
    destination: &[u32],
    caps: &[i128],
    exact_shares: bool,
) -> Option<Proposals> {
    let current: Vec<usize> = blocks.iter().map(|&b| destination[b] as usize).collect();
    let (now, used) = column(model, blocks, &current);
    if blocks.is_empty() || used.iter().zip(caps).any(|(u, c)| u > c) {
        return None;
    }
    let float =
        |(v, u): (i128, Vec<i128>)| (v as f64, u.iter().map(|&x| x as f64).collect::<Vec<f64>>());
    let mut master = Master {
        caps,
        columns: vec![float((now, used))],
        basis: (0..=caps.len()).collect(),
    };
    // The prices each column was priced at; the first column is the current assignment. A column is regenerated from
    // its prices rather than kept, so the master holds `R + 1` numbers per column, not one per block.
    let mut priced_at: Vec<Option<Vec<f64>>> = vec![None];
    let regenerate = |k: usize, at: &[Option<Vec<f64>>]| match &at[k] {
        None => current.clone(),
        Some(lambda) => assign(model, blocks, destination, lambda),
    };
    let mut last = None;
    while master.columns.len() < PRICE_COLUMNS {
        let Some(prices) = master.solve() else {
            break;
        };
        let pick = assign(model, blocks, destination, &prices.lambda);
        let (v, u) = float(column(model, blocks, &pick));
        let reduced = v - dot(&prices.lambda, &u) - prices.mu;
        let done = reduced <= TOLERANCE * prices.scale.max(v.abs());
        if !done {
            master.columns.push((v, u));
            priced_at.push(Some(prices.lambda.clone()));
        }
        // Keep the basis of this successful solve, not one partially pivoted
        // by a later solve that fails or reaches its iteration budget.
        last = Some((pick, prices.mixture, prices.basis));
        if done {
            break;
        }
    }
    let (pick, mixture, basis) = last?;
    let dn = model.destinations;
    let mut weight = vec![0.0_f64; blocks.len() * dn];
    let mut candidates = vec![pick];
    let columns: Vec<Vec<usize>> = mixture
        .iter()
        .map(|&(k, _)| regenerate(k, &priced_at))
        .collect();
    for (picked, &(_, w)) in columns.iter().zip(&mixture) {
        for (i, &d) in picked.iter().enumerate() {
            weight[i * dn + d] += w;
        }
    }
    candidates.push(
        current
            .iter()
            .enumerate()
            .map(|(i, &c)| {
                (0..dn).fold(c, |best, d| {
                    if weight[i * dn + d] > weight[i * dn + best] {
                        d
                    } else {
                        best
                    }
                })
            })
            .collect(),
    );
    candidates.extend(columns);
    let shares = (|| {
        if !exact_shares || caps.len() > EXACT_RESOURCES {
            return None;
        }
        let mut exact_columns = vec![Vec::new(); master.columns.len()];
        let mut assignments = Vec::new();
        for (j, &column) in basis.iter().enumerate() {
            if let Some(k) = column.checked_sub(caps.len()) {
                let picked = regenerate(k, &priced_at);
                exact_columns[k] = self::column(model, blocks, &picked).1;
                assignments.push((j, picked));
            }
        }
        let weights = exact_basis(&exact_columns, caps, &basis)?;
        let mut shares = vec![vec![Rat::from_integer(0); dn]; blocks.len()];
        for (j, picked) in assignments {
            for (row, d) in shares.iter_mut().zip(picked) {
                row[d] = row[d].checked_add(weights[j]).ok()?;
            }
        }
        for row in &shares {
            let total = row
                .iter()
                .try_fold(Rat::from_integer(0), |a, &b| a.checked_add(b).ok())?;
            if total != Rat::from_integer(1) {
                return None;
            }
        }
        Some(shares)
    })();
    Some(Proposals {
        directions: candidates,
        shares,
    })
}

/// The **Price** move on one period (see the module notes): `blocks` are the period's mined blocks, `destination`
/// every block's current destination, and `caps` the period's limits. Returns the new destination of each of `blocks`,
/// in order, when the new assignment keeps every limit and is worth strictly more; otherwise `None`.
pub(super) fn reprice(
    model: &PcpspModel,
    blocks: &[usize],
    destination: &[u32],
    caps: &[i128],
) -> Option<Vec<usize>> {
    let candidates = proposals(model, blocks, destination, caps, false)?;
    let current: Vec<usize> = blocks.iter().map(|&b| destination[b] as usize).collect();
    let (now, _) = column(model, blocks, &current);
    let mut best: Option<(i128, Vec<usize>)> = None;
    for mut pick in candidates.directions {
        if repair(model, blocks, &mut pick, caps).is_none() {
            continue;
        }
        let (value, used) = column(model, blocks, &pick);
        if value > best.as_ref().map_or(now, |(top, _)| *top)
            && used.iter().zip(caps).all(|(u, c)| u <= c)
        {
            best = Some((value, pick));
        }
    }
    best.map(|(_, pick)| pick)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pitflow::minelib::Rounding;
    use crate::toposort::pcpsp::parse;

    // One period; destination 0 is the dump, 1 the mill. Resource 0 is a blend with limit 0: blocks 0 and 1 are above
    // the target (+2 each, worth 3 more at the mill), block 2 below it (−4, worth 5 less at the mill). All three at the
    // mill gain 3 + 3 − 5 = 1. One block above the target cannot go alone, and with block 2 it gains 3 − 5 < 0.
    const THREE: &str = "NAME: three\nTYPE: PCPSP\nNBLOCKS: 3\nNPERIODS: 1\nNDESTINATIONS: 2\nNRESOURCE_SIDE_CONSTRAINTS: 1\nNGENERAL_SIDE_CONSTRAINTS: 0\nDISCOUNT_RATE: 0\nRESOURCE_CONSTRAINT_LIMITS:\n0 0 L 0\nOBJECTIVE_FUNCTION:\n0 0 3\n1 0 3\n2 0 -5\nRESOURCE_CONSTRAINT_COEFFICIENTS:\n0 1 0 2\n1 1 0 2\n2 1 0 -4\nEOF\n";

    #[test]
    fn exact_reconstruction_checks_slacks_and_refuses_singular_or_oversized_bases() {
        let columns = vec![vec![0], vec![4]];
        assert_eq!(
            exact_basis(&columns, &[2], &[1, 2]),
            Some(vec![Rat::new(1, 2).unwrap(); 2])
        );
        // A zero-cost slack remains part of feasibility: a whole use-4
        // assignment under capacity 2 has slack -2 and must not be admitted.
        assert!(exact_basis(&columns, &[2], &[0, 2]).is_none());
        assert!(exact_basis(&columns, &[2], &[1, 1]).is_none());
        assert!(exact_basis(&columns, &[2], &[1, 3]).is_none());
        assert!(exact_basis(&[vec![]], &[2], &[0, 1]).is_none());
        let many = vec![0; EXACT_RESOURCES + 1];
        assert!(
            exact_basis(
                std::slice::from_ref(&many),
                &many,
                &(0..=many.len()).collect::<Vec<_>>()
            )
            .is_none()
        );
    }

    #[test]
    fn three_blocks_go_to_the_mill_together() {
        let m = parse(THREE, "0 0\n1 0\n2 0\n", 0, Rounding::Refuse, 0).unwrap();
        assert_eq!(
            reprice(&m, &[0, 1, 2], &[0, 0, 0], &[0]),
            Some(vec![1, 1, 1])
        );
        // Already there: nothing worth more.
        assert_eq!(reprice(&m, &[0, 1, 2], &[1, 1, 1], &[0]), None);
        // Without block 2 there is no room in the blend: the dump is best.
        assert_eq!(reprice(&m, &[0, 1], &[0, 0, 0], &[0]), None);
        // An assignment that breaks a limit is not a start.
        assert_eq!(reprice(&m, &[0, 1, 2], &[1, 0, 0], &[0]), None);
        // With room for one block above the target and nothing below it, one goes: the master mixes both at the mill
        // with both at the dump, and the repair sends back the lower of equals.
        assert_eq!(reprice(&m, &[0, 1], &[0, 0, 0], &[2]), Some(vec![0, 1]));
    }

    #[test]
    fn each_block_goes_where_value_less_priced_use_is_largest() {
        let m = parse(THREE, "0 0\n1 0\n2 0\n", 0, Rounding::Refuse, 0).unwrap();
        // Priced at 1.4 per unit of blend: at the mill blocks 0 and 1 are worth 3 − 2.8 = 0.2 and block 2 −5 + 5.6 =
        // 0.6, against 0 at the dump.
        assert_eq!(assign(&m, &[0, 1, 2], &[0, 0, 0], &[1.4]), vec![1, 1, 1]);
        // At 2: blocks 0 and 1 are worth −1 at the mill, block 2 is worth 3.
        assert_eq!(assign(&m, &[0, 1, 2], &[0, 0, 0], &[2.0]), vec![0, 0, 1]);
        // At 1.5 blocks 0 and 1 are worth 0 at either destination and stay where they are.
        assert_eq!(assign(&m, &[0, 1, 2], &[1, 0, 0], &[1.5]), vec![1, 0, 1]);
        // Unpriced, each block goes to its most valuable destination.
        assert_eq!(assign(&m, &[0, 1, 2], &[0, 0, 0], &[0.0]), vec![1, 1, 0]);
    }

    #[test]
    fn repair_lowers_the_excess_at_the_least_loss() {
        let m = parse(THREE, "0 0\n1 0\n2 0\n", 0, Rounding::Refuse, 0).unwrap();
        // Blocks 0 and 1 at the mill without block 2 put the blend 4 over; one of them goes back to the dump, losing
        // 3, which removes only 2 of the 4, so the other follows.
        let mut pick = vec![1, 1];
        assert_eq!(repair(&m, &[0, 1], &mut pick, &[0]), Some(()));
        assert_eq!(pick, vec![0, 0]);
        // With block 2 in the period, sending it to the mill loses 5 and clears all 4 units of excess, while sending
        // back block 0 or 1 loses 3 for 2 of them. Measured in the largest use, 4, that is 5 per unit against 6: block 2
        // moves.
        let mut pick = vec![1, 1, 0];
        assert_eq!(repair(&m, &[0, 1, 2], &mut pick, &[0]), Some(()));
        assert_eq!(pick, vec![1, 1, 1]);
    }

    #[test]
    fn the_inverse_and_the_master_on_a_small_case() {
        let inverse = invert(&[(0.0, vec![2.0, 0.0]), (0.0, vec![1.0, 1.0])]).unwrap();
        assert_eq!(inverse, vec![vec![0.5, -0.5], vec![0.0, 1.0]]);
        assert!(invert(&[(0.0, vec![1.0, 1.0]), (0.0, vec![2.0, 2.0])]).is_none());
        // Two columns, one resource with limit 2: the first uses 0 and is worth 0, the second uses 4 and is worth 8.
        // The master mixes them half and half; the resource is priced at 2 per unit and the convexity row at 0.
        let caps = [2_i128];
        let mut master = Master {
            caps: &caps,
            columns: vec![(0.0, vec![0.0]), (8.0, vec![4.0])],
            basis: vec![0, 1],
        };
        let p = master.solve().unwrap();
        assert_eq!(
            (p.lambda, p.mu, p.scale, p.mixture),
            (vec![2.0], 0.0, 8.0, vec![(1, 0.5), (0, 0.5)])
        );
    }
}
