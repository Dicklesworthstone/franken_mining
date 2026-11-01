//! `pdlp` against oracles that share no code with it (`tests/common`): vertex enumeration on tiny bounded LPs, and
//! brute force over all subsets on closure LPs, whose relaxation is integral. The certified bound is also checked
//! for validity at random multipliers (PO-PUSH-4).

mod common;

use common::{Dense, Rng, random_closure, random_dense};
use fmine_lp::pdlp::{PdlpOptions, PdlpStatus, certified_bound, solve};

fn tight() -> PdlpOptions {
    PdlpOptions {
        tolerance: 1e-9,
        max_iterations: 400_000,
        threads: 1,
    }
}

fn close(a: f64, b: f64, rel: f64) -> bool {
    (a - b).abs() <= rel * (1.0 + a.abs().max(b.abs()))
}

#[test]
fn tiny_lps_match_vertex_enumeration() {
    let mut rng = Rng(0x5eed_0001);
    let (mut solved, mut iters) = (0, 0u64);
    for case in 0..300 {
        let d = random_dense(&mut rng, false);
        let opt = d
            .vertex_optimum()
            .unwrap_or_else(|| panic!("case {case}: the generator made an infeasible LP"));
        let p = d.problem();
        let s = solve(&p, &tight()).unwrap();
        assert_eq!(
            s.status(),
            PdlpStatus::Optimal,
            "case {case}: {d:?} {:?}",
            s.progress()
        );
        let got = s.progress().primal_objective;
        assert!(
            close(got, opt, 1e-6),
            "case {case}: pdlp {got}, vertices {opt}, {d:?}"
        );
        assert!(
            p.max_violation(s.x()).unwrap() <= 1e-6,
            "case {case}: x violates by {}",
            p.max_violation(s.x()).unwrap()
        );
        let b = s
            .bound()
            .unwrap_or_else(|| panic!("case {case}: no bound"))
            .value();
        assert!(
            b <= opt + 1e-9 * (1.0 + opt.abs()),
            "case {case}: bound {b} above the optimum {opt}"
        );
        assert!(close(b, opt, 1e-6), "case {case}: bound {b} far from {opt}");
        solved += 1;
        iters += s.progress().iterations;
    }
    println!(
        "{{\"suite\":\"fmine-lp/pdlp\",\"case\":\"tiny\",\"solved\":{solved},\"iterations\":{iters}}}"
    );
}

#[test]
fn closure_lps_match_the_best_closure() {
    let mut rng = Rng(0x5eed_0002);
    for case in 0..150 {
        let (d, best) = random_closure(&mut rng);
        let p = d.problem();
        let s = solve(&p, &tight()).unwrap();
        assert_eq!(s.status(), PdlpStatus::Optimal, "case {case}");
        let got = -s.progress().primal_objective;
        assert!(
            close(got, best, 1e-6),
            "case {case}: pdlp {got}, brute force {best}"
        );
        let ub = -s.bound().unwrap().value();
        assert!(
            ub >= best - 1e-9 * (1.0 + best.abs()),
            "case {case}: bound {ub} below the best closure {best}"
        );
    }
}

/// Any multipliers at all, of any sign or size, give a bound no greater than the optimum, or none.
#[test]
fn certified_bound_is_valid_at_any_multipliers() {
    let mut rng = Rng(0x5eed_0003);
    let (mut bounds, mut none) = (0, 0);
    for case in 0..400 {
        let open = case % 2 == 1;
        let d = random_dense(&mut rng, open);
        // Truncating open tops shrinks the feasible set, so its optimum is at least the true one.
        let truncated = Dense {
            u: d.u
                .iter()
                .map(|&v| if v.is_finite() { v } else { 1e3 })
                .collect(),
            ..d.clone()
        };
        let opt = truncated.vertex_optimum().unwrap();
        let p = d.problem();
        for k in 0..25 {
            let scale = [1e-6, 1e-2, 1.0, 1e2, 1e6][k % 5];
            let y: Vec<f64> = (0..p.rows())
                .map(|_| rng.uniform(-1.0, 1.0) * scale)
                .collect();
            match certified_bound(&p, &y).unwrap() {
                Some(b) => {
                    bounds += 1;
                    assert!(
                        b.value() <= opt + 1e-9 * (1.0 + opt.abs()),
                        "case {case}: bound {} above {opt} at y = {y:?}, {d:?}",
                        b.value()
                    );
                }
                None => none += 1,
            }
        }
    }
    println!(
        "{{\"suite\":\"fmine-lp/pdlp\",\"case\":\"bound_validity\",\"bounds\":{bounds},\"none\":{none}}}"
    );
    assert!(bounds > 5_000, "only {bounds} bounds were finite");
}

#[test]
fn infeasible_problems_never_report_optimal() {
    // x1 + x2 ≥ 3 with both in [0, 1].
    let d = Dense {
        c: vec![1.0, 1.0],
        a: vec![vec![1.0, 1.0]],
        lo: vec![3.0],
        hi: vec![f64::INFINITY],
        l: vec![0.0, 0.0],
        u: vec![1.0, 1.0],
    };
    let o = PdlpOptions {
        tolerance: 1e-6,
        max_iterations: 20_000,
        threads: 1,
    };
    let s = solve(&d.problem(), &o).unwrap();
    assert_eq!(s.status(), PdlpStatus::IterationLimit);
}

fn transport(seed: u64, sources: usize, sinks: usize) -> Dense {
    let mut rng = Rng(seed);
    let n = sources * sinks;
    let supply: Vec<f64> = (0..sources).map(|_| rng.uniform(50.0, 150.0)).collect();
    let total: f64 = supply.iter().sum();
    let demand: Vec<f64> = (0..sinks).map(|_| total / sinks as f64 * 0.9).collect();
    let mut a = Vec::new();
    let (mut lo, mut hi) = (Vec::new(), Vec::new());
    for (s, &sup) in supply.iter().enumerate() {
        a.push(
            (0..n)
                .map(|j| f64::from(u8::from(j / sinks == s)))
                .collect(),
        );
        lo.push(f64::NEG_INFINITY);
        hi.push(sup);
    }
    for (t, &dem) in demand.iter().enumerate() {
        a.push(
            (0..n)
                .map(|j| f64::from(u8::from(j % sinks == t)))
                .collect(),
        );
        lo.push(dem);
        hi.push(f64::INFINITY);
    }
    Dense {
        c: (0..n).map(|_| rng.uniform(1.0, 10.0)).collect(),
        a,
        lo,
        hi,
        l: vec![0.0; n],
        u: vec![f64::INFINITY; n],
    }
}

/// Fixed-order sums make the run the same, bit for bit, at any thread count. The problem is big enough to use
/// several workers.
#[test]
fn any_thread_count_gives_the_same_bits() {
    let d = transport(9, 60, 1_200);
    let p = d.problem();
    assert!(
        p.nnz() >= 1 << 17,
        "too small to run in parallel: {}",
        p.nnz()
    );
    let run = |threads| {
        let o = PdlpOptions {
            tolerance: 1e-7,
            max_iterations: 640,
            threads,
        };
        solve(&p, &o).unwrap()
    };
    let one = run(1);
    for t in [2, 5] {
        let other = run(t);
        assert_eq!(one.progress().iterations, other.progress().iterations);
        let bits = |v: &[f64]| v.iter().map(|f| f.to_bits()).collect::<Vec<_>>();
        assert_eq!(bits(one.x()), bits(other.x()), "x differs at {t} threads");
        assert_eq!(bits(one.y()), bits(other.y()), "y differs at {t} threads");
    }
}
