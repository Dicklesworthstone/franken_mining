//! Determinism: BitwisePortable (`i64` and `i128` integer arithmetic only; no floating point, hashing or threads).
//!
//! Nested pit shells by revenue factor (PO-PUSH-3, plan §15.4 and §15.9; `bd-nzwd`).
//!
//! At revenue factor λ = `num / den`, a block is worth `num · revenue − den · cost`. That is its value at λ scaled
//! by `den`, which leaves the best pit unchanged. Revenue must be non-negative; cost may have either sign.
//!
//! **Why a running union.** Let `A` be a best pit at λ and `B` a best pit at λ' > λ.
//! - `A ∩ B` and `A ∪ B` are closed, and these values are modular: `f(A ∪ B) + f(A ∩ B) = f(A) + f(B)` at every
//!   factor.
//! - So `f_λ'(A ∪ B) − f_λ'(B) = f_λ'(A) − f_λ'(A ∩ B) = [f_λ(A) − f_λ(A ∩ B)] + (λ' − λ) · Σ_{A∖B} revenue`.
//!   The bracket is ≥ 0 because `A` is best at λ, and the sum is ≥ 0 because revenue is.
//! - `A ∪ B` is therefore a best pit at λ'.
//!
//! Shell `k` is the union of the pit solved at factor `k` and shell `k − 1`. Shells therefore nest by construction
//! (PO-PUSH-3), and each is a best pit at its factor. [`nested_shells`] checks each shell's closure and its value
//! against the certified optimum, and refuses rather than return a shell that fails either check.
//!
//! Each factor is a fresh solve; nothing is warm-started yet.

use super::{ClosureProblem, NONE, check_value_guard, refuse, solve};
use fmine_evidence::Refusal;

/// An exact revenue factor `num / den`, both positive.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RevenueFactor {
    num: i64,
    den: i64,
}

impl RevenueFactor {
    /// # Errors
    /// `FMINE-REF-207` (proposed) unless both `num` and `den` are positive.
    pub fn new(num: i64, den: i64) -> Result<Self, Refusal> {
        if num <= 0 || den <= 0 {
            return Err(refuse(format!(
                "shells: revenue factor {num}/{den} must have a positive numerator and denominator"
            )));
        }
        Ok(Self { num, den })
    }

    /// The numerator.
    #[must_use]
    pub fn num(&self) -> i64 {
        self.num
    }

    /// The denominator: shell values at this factor are in units of `1 / den`.
    #[must_use]
    pub fn den(&self) -> i64 {
        self.den
    }

    fn below(self, other: Self) -> bool {
        i128::from(self.num) * i128::from(other.den) < i128::from(other.num) * i128::from(self.den)
    }
}

/// Block revenue and cost, with the precedence arcs, for a shell sweep.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShellModel {
    revenue: Vec<i64>,
    cost: Vec<i64>,
    arcs: ClosureProblem,
}

impl ShellModel {
    /// The arcs are in CSR form, as for [`ClosureProblem::new`].
    ///
    /// # Errors
    /// `FMINE-REF-207` (proposed):
    /// - when `revenue` and `cost` differ in length;
    /// - when a revenue is negative, since shells would then not nest;
    /// - in the cases of [`ClosureProblem::new`].
    pub fn new(
        revenue: Vec<i64>,
        cost: Vec<i64>,
        offsets: Vec<usize>,
        targets: Vec<u32>,
    ) -> Result<Self, Refusal> {
        if revenue.len() != cost.len() {
            return Err(refuse(format!(
                "shells: {} revenues for {} costs",
                revenue.len(),
                cost.len()
            )));
        }
        if let Some((b, &r)) = revenue.iter().enumerate().find(|&(_, &r)| r < 0) {
            return Err(refuse(format!(
                "shells: block {b} has negative revenue {r}; shells nest only when revenue is non-negative"
            )));
        }
        let arcs = ClosureProblem::new(vec![0; revenue.len()], offsets, targets)?;
        Ok(Self {
            revenue,
            cost,
            arcs,
        })
    }

    /// Builds the CSR from one list of required blocks per block.
    ///
    /// # Errors
    /// As [`ShellModel::new`], and `FMINE-REF-207` when there is not one list per block.
    pub fn from_requirements(
        revenue: Vec<i64>,
        cost: Vec<i64>,
        requires: &[Vec<u32>],
    ) -> Result<Self, Refusal> {
        let csr = ClosureProblem::from_requirements(vec![0; requires.len()], requires)?;
        Self::new(revenue, cost, csr.offsets, csr.targets)
    }

    /// Number of blocks.
    #[must_use]
    pub fn block_count(&self) -> usize {
        self.revenue.len()
    }

    /// Each block's value at `factor`, scaled by its denominator: `num · revenue − den · cost`.
    ///
    /// # Errors
    /// `FMINE-REF-207` (proposed) when a value overflows `i64` or the values break [`super::ABS_VALUE_SUM_LIMIT`].
    pub fn values_at(&self, factor: RevenueFactor) -> Result<Vec<i64>, Refusal> {
        let values = self
            .revenue
            .iter()
            .zip(&self.cost)
            .enumerate()
            .map(|(b, (&r, &c))| {
                r.checked_mul(factor.num)
                    .zip(c.checked_mul(factor.den))
                    .and_then(|(gain, spend)| gain.checked_sub(spend))
                    .ok_or_else(|| {
                        refuse(format!(
                            "shells: block {b}'s value overflows i64 at factor {}/{}",
                            factor.num, factor.den
                        ))
                    })
            })
            .collect::<Result<Vec<_>, _>>()?;
        check_value_guard(&values)?;
        Ok(values)
    }
}

/// One shell of a sweep.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShellSummary {
    /// The revenue factor.
    pub factor: RevenueFactor,
    /// The shell's value at its factor, in units of `1 / factor.den()`: the best value of any pit there.
    pub value: i64,
    /// Blocks in the shell.
    pub blocks: usize,
    /// Blocks in the pit solved at this factor. Fewer than `blocks` only where tied pits differ.
    pub solved_pit_blocks: usize,
}

/// Nested shells, one per factor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NestedShells {
    first_shell: Vec<u32>,
    shells: Vec<ShellSummary>,
}

impl NestedShells {
    /// For each block, the index of the first shell that holds it, or `u32::MAX` if none does. Block `b` is in shell
    /// `k` exactly when `first_shell()[b] <= k`.
    #[must_use]
    pub fn first_shell(&self) -> &[u32] {
        &self.first_shell
    }

    /// The shells, in factor order.
    #[must_use]
    pub fn shells(&self) -> &[ShellSummary] {
        &self.shells
    }

    /// Whether block `b` is in shell `k`; `false` for a block or shell that does not exist.
    #[must_use]
    pub fn contains(&self, k: usize, b: usize) -> bool {
        k < self.shells.len() && self.first_shell.get(b).is_some_and(|&f| (f as usize) <= k)
    }
}

/// One certified, nested shell per factor.
///
/// # Errors
/// `FMINE-REF-207` (proposed):
/// - when the factors are not strictly increasing, or there are `u32::MAX` or more of them;
/// - in the cases of [`ShellModel::values_at`] and [`solve`];
/// - when a shell is not closed, or is worth less than the best pit at its factor. The construction rules both out,
///   so either means a defect.
pub fn nested_shells(
    model: &ShellModel,
    factors: &[RevenueFactor],
) -> Result<NestedShells, Refusal> {
    if factors.len() >= NONE as usize {
        return Err(refuse(format!(
            "shells: {} factors; there must be fewer than {NONE}",
            factors.len()
        )));
    }
    if let Some(w) = factors.windows(2).find(|w| !w[0].below(w[1])) {
        return Err(refuse(format!(
            "shells: factors must strictly increase, but {}/{} is followed by {}/{}",
            w[0].num, w[0].den, w[1].num, w[1].den
        )));
    }
    let n = model.block_count();
    let mut problem = model.arcs.clone();
    let mut first_shell = vec![NONE; n];
    let mut shells = Vec::with_capacity(factors.len());
    for (k, &factor) in (0u32..).zip(factors) {
        problem.weights = model.values_at(factor)?;
        let pit = solve(&problem)?;
        for (slot, &inside) in first_shell.iter_mut().zip(pit.in_pit()) {
            if inside && *slot == NONE {
                *slot = k;
            }
        }
        let in_shell = |b: usize| first_shell[b] <= k;
        if let Some((_, u, v)) = problem
            .arcs()
            .find(|&(_, u, v)| in_shell(u) && !in_shell(v))
        {
            return Err(refuse(format!(
                "shells: shell {k} is not closed; it holds {u} but not {v}, which {u} requires"
            )));
        }
        let value: i128 = problem
            .weights
            .iter()
            .enumerate()
            .filter(|&(b, _)| in_shell(b))
            .map(|(_, &w)| i128::from(w))
            .sum();
        if value != i128::from(pit.value()) {
            return Err(refuse(format!(
                "shells: shell {k} at factor {}/{} is worth {value}, but the best pit there is worth {}",
                factor.num,
                factor.den,
                pit.value()
            )));
        }
        shells.push(ShellSummary {
            factor,
            value: pit.value(),
            blocks: (0..n).filter(|&b| in_shell(b)).count(),
            solved_pit_blocks: pit.pit_size(),
        });
    }
    Ok(NestedShells {
        first_shell,
        shells,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn factor(num: i64, den: i64) -> RevenueFactor {
        RevenueFactor::new(num, den).unwrap()
    }

    /// A vertical section 16 wide and 5 deep under 45° slopes: each block below the top needs the three above it.
    fn section(revenue: &[(usize, i64)]) -> ShellModel {
        let (w, depth) = (16usize, 5usize);
        let idx = |i: usize, k: usize| k * w + i;
        let mut rev = vec![0i64; w * depth];
        for &(b, r) in revenue {
            rev[b] = r;
        }
        let mut lists = vec![Vec::new(); w * depth];
        for k in 1..depth {
            for i in 0..w {
                for ii in i.saturating_sub(1)..=(i + 1).min(w - 1) {
                    lists[idx(i, k)].push(idx(ii, k - 1) as u32);
                }
            }
        }
        ShellModel::from_requirements(rev, vec![1; w * depth], &lists).unwrap()
    }

    /// Two ore blocks worth 10 at revenue factor 1, every block costing 1:
    /// - a shallow one at (3, 1), whose cone holds 4 blocks, pays above λ = 0.4;
    /// - a deep one at (12, 3), whose cone holds 16 blocks, pays above λ = 1.6.
    #[test]
    fn two_ore_blocks_enter_at_their_factors() {
        let (shallow, deep) = (16 + 3, 3 * 16 + 12);
        let m = section(&[(shallow, 10), (deep, 10)]);
        let factors = [factor(1, 4), factor(1, 2), factor(1, 1), factor(2, 1)];
        let s = nested_shells(&m, &factors).unwrap();
        let blocks: Vec<usize> = s.shells().iter().map(|x| x.blocks).collect();
        assert_eq!(blocks, vec![0, 4, 4, 20]);
        // At 1/2: ore worth 1·10 − 2·1 = 8, three waste blocks at −2 each.
        let values: Vec<i64> = s.shells().iter().map(|x| x.value).collect();
        assert_eq!(values, vec![0, 2, 6, 20]);
        assert_eq!(s.first_shell()[shallow], 1);
        assert_eq!(s.first_shell()[deep], 3);
        assert!(s.contains(1, shallow) && !s.contains(0, shallow) && !s.contains(2, deep));
        assert!(!s.contains(4, shallow) && !s.contains(0, 10_000));
        assert_eq!(s.first_shell()[0], NONE);
    }

    fn code<T>(r: Result<T, Refusal>) -> Option<&'static str> {
        r.err().map(|e| e.code)
    }

    #[test]
    fn bad_input_is_refused() {
        assert_eq!(code(RevenueFactor::new(0, 1)), Some("FMINE-REF-207"));
        assert_eq!(code(RevenueFactor::new(1, 0)), Some("FMINE-REF-207"));
        assert_eq!(code(RevenueFactor::new(-1, -2)), Some("FMINE-REF-207"));
        let empty: &[Vec<u32>] = &[vec![], vec![]];
        assert_eq!(
            code(ShellModel::from_requirements(
                vec![1, -1],
                vec![0, 0],
                empty
            )),
            Some("FMINE-REF-207")
        );
        assert_eq!(
            code(ShellModel::from_requirements(vec![1], vec![0, 0], empty)),
            Some("FMINE-REF-207")
        );
        assert_eq!(
            code(ShellModel::from_requirements(
                vec![1, 1],
                vec![0, 0],
                &[vec![5], vec![]]
            )),
            Some("FMINE-REF-207")
        );
        let m = ShellModel::from_requirements(vec![3, 1], vec![1, 1], empty).unwrap();
        // Equal factors (1/2 and 2/4) and decreasing ones.
        assert_eq!(
            code(nested_shells(&m, &[factor(1, 2), factor(2, 4)])),
            Some("FMINE-REF-207")
        );
        assert_eq!(
            code(nested_shells(&m, &[factor(2, 1), factor(1, 1)])),
            Some("FMINE-REF-207")
        );
        // A value past i64, and values whose sum reaches the 2^62 guard: 2^62 − 1 plus 1.
        let big = ShellModel::from_requirements(vec![i64::MAX / 2, 1], vec![0, 0], empty).unwrap();
        assert_eq!(
            code(nested_shells(&big, &[factor(3, 1)])),
            Some("FMINE-REF-207")
        );
        assert_eq!(
            code(nested_shells(&big, &[factor(1, 1)])),
            Some("FMINE-REF-207")
        );
        assert!(nested_shells(&m, &[]).unwrap().shells().is_empty());
    }

    /// Negative revenue, let past the constructor, breaks the union rule. The check refuses instead of returning a
    /// shell that is not optimal: a lone block worth `1 − λ` is in the pit at λ = 1/2 and out of it at λ = 2.
    #[test]
    fn negative_revenue_past_the_constructor_is_caught_by_the_value_check() {
        let mut m = ShellModel::from_requirements(vec![0], vec![0], &[vec![]]).unwrap();
        m.revenue = vec![-1];
        m.cost = vec![-1];
        let r = nested_shells(&m, &[factor(1, 2), factor(2, 1)]);
        let e = r.err().unwrap();
        assert_eq!(e.code, "FMINE-REF-207");
        assert!(e.what.contains("shell 1"), "{}", e.what);
    }
}
