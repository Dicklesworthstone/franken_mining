//! PO-PUSH-3 (plan §15.9) as a property test (`bd-nzwd`): `pitflow::shells` nests its shells, and each one is a
//! closed pit worth the best value at its factor, which an independent Dinic maximum flow (`tests/common`) gives.
//!
//! What green does not prove:
//! - speed of a sweep, since each factor is a fresh solve;
//! - anything on MineLib, whose UPIT files give one value per block, with revenue and cost not split;
//! - anything about scheduling.

mod common;

use common::{Rng, block_model, max_closure_value};
use fmine_evidence::Refusal;
use fmine_optimize::pitflow::shells::{RevenueFactor, ShellModel, nested_shells};

/// Factors 1/4 to 3 in uneven steps, with denominators that differ.
fn factors() -> Result<Vec<RevenueFactor>, Refusal> {
    [
        (1, 4),
        (1, 2),
        (2, 3),
        (1, 1),
        (5, 4),
        (3, 2),
        (2, 1),
        (3, 1),
    ]
    .into_iter()
    .map(|(n, d)| RevenueFactor::new(n, d))
    .collect()
}

/// Checks one sweep against the oracle and returns the block counts, shell by shell.
fn check_sweep(
    revenue: &[i64],
    cost: &[i64],
    requires: &[Vec<u32>],
    label: &str,
) -> Result<Vec<usize>, Refusal> {
    let m = ShellModel::from_requirements(revenue.to_vec(), cost.to_vec(), requires)?;
    let fs = factors()?;
    let s = nested_shells(&m, &fs)?;
    assert_eq!(s.shells().len(), fs.len());
    for (k, (shell, f)) in s.shells().iter().zip(&fs).enumerate() {
        // The oracle computes the scaled values itself.
        let values: Vec<i64> = revenue
            .iter()
            .zip(cost)
            .map(|(&r, &c)| f.num() * r - f.den() * c)
            .collect();
        assert_eq!(
            shell.value,
            max_closure_value(&values, requires),
            "{label}: shell {k} against Dinic"
        );
        let inside: Vec<bool> = (0..revenue.len()).map(|b| s.contains(k, b)).collect();
        let worth: i64 = values
            .iter()
            .zip(&inside)
            .filter(|(_, i)| **i)
            .map(|(v, _)| v)
            .sum();
        assert_eq!(
            worth, shell.value,
            "{label}: shell {k}'s blocks add up to its value"
        );
        assert_eq!(inside.iter().filter(|&&i| i).count(), shell.blocks);
        for (u, r) in requires.iter().enumerate() {
            assert!(
                !inside[u] || r.iter().all(|&v| inside[v as usize]),
                "{label}: shell {k} holds {u} without what it requires"
            );
        }
        if k > 0 {
            assert!(
                (0..revenue.len()).all(|b| !s.contains(k - 1, b) || inside[b]),
                "{label}: shell {} is not inside shell {k}",
                k - 1
            );
        }
    }
    Ok(s.shells().iter().map(|x| x.blocks).collect())
}

#[test]
fn random_graphs_nest_and_each_shell_is_optimal() {
    let mut rng = Rng(0x00AE_5E11);
    let mut grew = 0;
    for round in 0..150u64 {
        let n = 10 + rng.below(140) as usize;
        let mut requires = vec![Vec::new(); n];
        for _ in 0..n * (1 + (round % 3) as usize) {
            let u = rng.below(n as u64) as usize;
            requires[u].push(rng.below(n as u64) as u32);
        }
        let revenue: Vec<i64> = (0..n)
            .map(|_| {
                if rng.below(3) == 0 {
                    rng.below(400) as i64
                } else {
                    0
                }
            })
            .collect();
        // Costs of either sign: a few blocks pay to be taken.
        let cost: Vec<i64> = (0..n).map(|_| rng.below(60) as i64 - 5).collect();
        let blocks = check_sweep(&revenue, &cost, &requires, &format!("round {round}")).unwrap();
        grew += usize::from(blocks.first() < blocks.last());
    }
    assert!(
        grew > 75,
        "shells grew across the sweep in only {grew} of 150 rounds"
    );
}

/// Ore value as revenue, a mining cost of 10 per block: the shells are the pit-by-pit sequence a planner reads.
#[test]
fn block_model_shells_grow_with_the_factor() {
    let mut rng = Rng(0x00AE_B10C);
    for dims in [(12, 12, 6), (20, 16, 8)] {
        let (weights, requires) = block_model(&mut rng, dims, 4, 10);
        let revenue: Vec<i64> = weights.iter().map(|&w| (w + 10).max(0)).collect();
        let cost = vec![10i64; weights.len()];
        let blocks = check_sweep(&revenue, &cost, &requires, &format!("{dims:?}")).unwrap();
        assert!(
            blocks.windows(2).all(|w| w[0] <= w[1]),
            "{dims:?}: {blocks:?}"
        );
        assert!(
            blocks.first() < blocks.last(),
            "{dims:?}: the sweep did not grow the pit: {blocks:?}"
        );
    }
}
