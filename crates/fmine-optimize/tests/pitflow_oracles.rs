//! `pitflow` against an independent Dinic maximum flow (`tests/common`), on random graphs and on block models
//! shaped like open pits (`BD-250`).
//!
//! What green does not prove: speed at plan scale (10⁷ blocks, KILL-PUSH-PIT), MineLib parity (`minelib_upit.rs`,
//! ignored), or anything about slope templates and nested shells, which are not built.

mod common;

use common::{Rng, block_model, max_closure_value};
use fmine_optimize::pitflow::{ClosureProblem, solve};

#[test]
fn random_graphs_match_dinic() {
    let mut rng = Rng(0x0250_D1C0);
    let mut cases = 0;
    let mut nonempty = 0;
    for round in 0..300u64 {
        let n = 20 + rng.below(280) as usize;
        let arcs = n * (1 + (round % 4) as usize);
        let weights: Vec<i64> = (0..n).map(|_| rng.below(2_001) as i64 - 1_000).collect();
        let mut requires = vec![Vec::new(); n];
        for _ in 0..arcs {
            let u = rng.below(n as u64) as usize;
            requires[u].push(rng.below(n as u64) as u32);
        }
        let expected = max_closure_value(&weights, &requires);
        let p = ClosureProblem::from_requirements(weights, &requires).unwrap();
        let s = solve(&p).unwrap();
        assert_eq!(s.value(), expected, "round {round}, n {n}");
        cases += 1;
        nonempty += usize::from(s.pit_size() > 0);
    }
    assert_eq!(cases, 300);
    assert!(nonempty > 200, "only {nonempty} non-empty pits");
}

/// Blocks below the surface need their 3 × 3 neighbours above (45° on a cubic grid). The pits are cones over ore
/// bodies, so the closures are deep and the trees long: unlike random graphs, this is the shape the solver exists for.
#[test]
fn block_models_match_dinic() {
    let mut rng = Rng(0x0250_B10C);
    let mut nonempty = 0;
    for (case, dims) in [
        (10, 10, 6),
        (16, 12, 8),
        (20, 20, 10),
        (24, 18, 12),
        (30, 30, 8),
    ]
    .into_iter()
    .enumerate()
    {
        for bodies in [1, 3, 6] {
            let (weights, requires) = block_model(&mut rng, dims, bodies, 10);
            let expected = max_closure_value(&weights, &requires);
            let p = ClosureProblem::from_requirements(weights, &requires).unwrap();
            let s = solve(&p).unwrap();
            assert_eq!(
                s.value(),
                expected,
                "case {case} {dims:?} with {bodies} bodies"
            );
            nonempty += usize::from(s.pit_size() > 0);
        }
    }
    assert!(
        nonempty >= 10,
        "only {nonempty} of 15 block models had a pit"
    );
}

/// A pit's blocks must include every block above them within the slope: a closed set.
#[test]
fn block_model_pit_is_a_union_of_cones() {
    let mut rng = Rng(9);
    let dims = (14, 14, 7);
    let (weights, requires) = block_model(&mut rng, dims, 4, 10);
    let p = ClosureProblem::from_requirements(weights, &requires).unwrap();
    let s = solve(&p).unwrap();
    assert!(s.pit_size() > 0);
    for (u, r) in requires.iter().enumerate() {
        if s.in_pit()[u] {
            assert!(
                r.iter().all(|&v| s.in_pit()[v as usize]),
                "block {u} lacks a block above it"
            );
        }
    }
}
