//! `pitflow` at scale on synthetic open-pit block models (`BD-266`). The models run from about 10⁶ arcs to 10⁷ blocks.
//! This answers INT-FNX-001's question (plan §27.4: precedence graphs of 10⁶–10⁷ arcs) for the in-house graph code, and
//! measures the second half of KILL-PUSH-PIT: 10⁷ blocks in 10 minutes or less (plan §15.4).
//!
//! Ignored by default, since the largest model needs several GB. Run
//! `cargo test --release -p fmine-optimize --test scale_pit -- --ignored --nocapture`. `FMINE_SCALE_MODELS` picks
//! models by name (comma-separated, default all three).
//!
//! - Every pit is certified by `verify` inside `solve`, which proves it optimal without an oracle.
//! - The smallest model is also checked against Dinic (`tests/common`).
//!
//! What green does not prove:
//! - anything about MineFlow, the other half of KILL-PUSH-PIT;
//! - anything about other machines;
//! - anything about models shaped unlike these, which use one 45° template and ellipsoid ore bodies.

mod common;

use common::{Rng, max_closure_value};
use fmine_optimize::pitflow::{ClosureProblem, solve};
use std::time::Instant;

/// KILL-PUSH-PIT's limit for 10⁷ blocks, in seconds.
const KILL_LIMIT_S: f64 = 600.0;

/// `(name, (nx, ny, nz))`. Arcs per layer below the top are `(3nx − 2)(3ny − 2)`.
const MODELS: [(&str, (usize, usize, usize)); 3] = [
    ("arcs_1e6", (60, 60, 32)),
    ("arcs_1e7", (180, 180, 36)),
    ("blocks_1e7", (500, 400, 50)),
];

const WASTE: i64 = 10;

/// One ore body per this many blocks.
const BLOCKS_PER_BODY: usize = 6_000;

/// A block model in CSR form, built without per-block lists so that 10⁷ blocks fit.
/// - Block `(i, j, k)` is `(k·ny + j)·nx + i`, with `k = 0` at the surface.
/// - Each block below the top requires the 3 × 3 blocks above it that exist.
/// - Waste costs [`WASTE`].
/// - There is one ore body per [`BLOCKS_PER_BODY`] blocks. Each is an integer ellipsoid with radii 3–9 across and
///   2–5 down, centred at any depth below the surface, and adds a grade of 5 to 25 times the waste cost.
fn model(rng: &mut Rng, (nx, ny, nz): (usize, usize, usize)) -> (Vec<i64>, Vec<usize>, Vec<u32>) {
    let idx = |i: usize, j: usize, k: usize| (k * ny + j) * nx + i;
    let mut weights = vec![-WASTE; nx * ny * nz];
    for _ in 0..(weights.len() / BLOCKS_PER_BODY).max(1) {
        let c = [
            rng.below(nx as u64) as i64,
            rng.below(ny as u64) as i64,
            1 + rng.below((nz - 1).max(1) as u64) as i64,
        ];
        let r = [
            3 + rng.below(7) as i64,
            3 + rng.below(7) as i64,
            2 + rng.below(4) as i64,
        ];
        let grade = 5 * WASTE + rng.below(20 * WASTE as u64 + 1) as i64;
        let r2 = [r[0] * r[0], r[1] * r[1], r[2] * r[2]];
        let span = |centre: i64, radius: i64, n: usize| {
            (centre - radius).max(0) as usize..=((centre + radius).min(n as i64 - 1)) as usize
        };
        for k in span(c[2], r[2], nz) {
            for j in span(c[1], r[1], ny) {
                for i in span(c[0], r[0], nx) {
                    let d = [i as i64 - c[0], j as i64 - c[1], k as i64 - c[2]];
                    // Inside when Σ (dᵢ/rᵢ)² ≤ 1, in integers: Σ dᵢ² Π_{j≠i} rⱼ² ≤ Π rᵢ².
                    let lhs = d[0] * d[0] * r2[1] * r2[2]
                        + d[1] * d[1] * r2[0] * r2[2]
                        + d[2] * d[2] * r2[0] * r2[1];
                    if lhs <= r2[0] * r2[1] * r2[2] {
                        weights[idx(i, j, k)] += grade;
                    }
                }
            }
        }
    }
    let arcs = (nz.saturating_sub(1)) * (3 * nx - 2) * (3 * ny - 2);
    let mut offsets = Vec::with_capacity(weights.len() + 1);
    offsets.push(0);
    let mut targets = Vec::with_capacity(arcs);
    for k in 0..nz {
        for j in 0..ny {
            for i in 0..nx {
                if k > 0 {
                    for jj in j.saturating_sub(1)..=(j + 1).min(ny - 1) {
                        for ii in i.saturating_sub(1)..=(i + 1).min(nx - 1) {
                            targets.push(idx(ii, jj, k - 1) as u32);
                        }
                    }
                }
                offsets.push(targets.len());
            }
        }
    }
    (weights, offsets, targets)
}

#[test]
fn model_has_the_planned_arc_count_and_shape() {
    let mut rng = Rng(1);
    let (weights, offsets, targets) = model(&mut rng, (5, 4, 3));
    assert_eq!(weights.len(), 60);
    assert_eq!(targets.len(), 2 * 13 * 10);
    assert_eq!(offsets.len(), 61);
    // Block (2, 1, 1) is interior: nine requirements, all in layer 0 around (2, 1).
    let b = (4 + 1) * 5 + 2;
    let mut req: Vec<u32> = targets[offsets[b]..offsets[b + 1]].to_vec();
    req.sort_unstable();
    assert_eq!(req, vec![1, 2, 3, 6, 7, 8, 11, 12, 13]);
    // Surface blocks require nothing.
    assert!(offsets[..=20].iter().all(|&o| o == 0));
}

#[test]
#[ignore = "BD-266 / KILL-PUSH-PIT: up to 10^7 blocks; run in release"]
fn certified_pits_at_scale() {
    let names = std::env::var("FMINE_SCALE_MODELS")
        .unwrap_or_else(|_| "arcs_1e6,arcs_1e7,blocks_1e7".to_string());
    let mut solved = 0;
    for name in names.split(',').map(str::trim).filter(|s| !s.is_empty()) {
        let &(_, dims) = MODELS
            .iter()
            .find(|(n, _)| *n == name)
            .expect("a model named in MODELS");
        let mut rng = Rng(0x0266_5CA1 ^ dims.0 as u64);
        let t0 = Instant::now();
        let (weights, offsets, targets) = model(&mut rng, dims);
        let ore = weights.iter().filter(|&&w| w > 0).count();
        let p = ClosureProblem::new(weights, offsets, targets).unwrap();
        let build_s = t0.elapsed().as_secs_f64();
        let t1 = Instant::now();
        let s = solve(&p).unwrap();
        let solve_s = t1.elapsed().as_secs_f64();
        let st = s.stats();
        println!(
            "{{\"bead\":\"BD-266\",\"model\":\"{name}\",\"dims\":[{},{},{}],\"blocks\":{},\"arcs\":{},\
             \"ore_blocks\":{ore},\"value\":{},\"pit_blocks\":{},\"build_s\":{build_s:.3},\"solve_s\":{solve_s:.3},\
             \"mergers\":{},\"splits\":{},\"relabels\":{},\"max_label\":{}}}",
            dims.0,
            dims.1,
            dims.2,
            p.node_count(),
            p.arc_count(),
            s.value(),
            s.pit_size(),
            st.mergers,
            st.splits,
            st.relabels,
            st.max_label
        );
        assert!(
            s.pit_size() > 0 && s.value() > 0,
            "{name}: the model has no profitable pit"
        );
        if name == "arcs_1e6" {
            let requires: Vec<Vec<u32>> = (0..p.node_count())
                .map(|u| p.requires(u).unwrap_or_default().to_vec())
                .collect();
            assert_eq!(
                s.value(),
                max_closure_value(p.weights(), &requires),
                "{name}: pitflow against Dinic"
            );
        }
        if p.node_count() >= 10_000_000 {
            assert!(
                solve_s <= KILL_LIMIT_S,
                "{name}: {solve_s:.1} s is over KILL-PUSH-PIT's {KILL_LIMIT_S} s"
            );
        }
        solved += 1;
    }
    assert!(solved > 0, "no model named");
}
