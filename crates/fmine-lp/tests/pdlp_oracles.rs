//! `pdlp` against oracles that share no code with it (`tests/common`): vertex enumeration on tiny bounded LPs, and
//! brute force over all subsets on closure LPs, whose relaxation is integral. The certified bound is also checked
//! for validity at random multipliers (PO-PUSH-4).

mod common;

use common::{Dense, Rng, random_closure, random_cpit, random_dense, random_integer_lp};
use fmine_lp::pdlp::{PdlpOptions, PdlpStatus, certified_bound, certified_infeasibility, solve};

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

/// Each bound the rows imply for an open column holds at every feasible point (bd-tcp4). With `c = −eⱼ` and
/// `y = 0`, the certified bound is minus column `j`'s implied top, so it may not exceed the LP's own minimum of
/// `−xⱼ`, found by vertex enumeration with the open tops cut at 10⁶. An LP unbounded in `xⱼ` then has a minimum near
/// −10⁶, below any finite top. The mirrored LP (`x → −x`) turns each open top into an open bottom, which checks the
/// implied bottoms the same way through the other signs.
#[test]
fn implied_column_bounds_hold_at_every_feasible_point() {
    let mut rng = Rng(0x5eed_0006);
    let (mut implied, mut none) = (0, 0);
    for case in 0..300 {
        let d = random_dense(&mut rng, true);
        let n = d.c.len();
        let mirror = Dense {
            a: d.a.iter().map(|r| r.iter().map(|v| -v).collect()).collect(),
            l: d.u.iter().map(|v| -v).collect(),
            u: d.l.iter().map(|v| -v).collect(),
            ..d.clone()
        };
        for (lp, sign) in [(&d, -1.0), (&mirror, 1.0)] {
            let cut = Dense {
                l: lp.l.iter().map(|&v| v.max(-1e6)).collect(),
                u: lp.u.iter().map(|&v| v.min(1e6)).collect(),
                ..lp.clone()
            };
            for j in 0..n {
                if lp.l[j].is_finite() && lp.u[j].is_finite() {
                    continue;
                }
                let c: Vec<f64> = (0..n).map(|k| if k == j { sign } else { 0.0 }).collect();
                let p = Dense {
                    c: c.clone(),
                    ..lp.clone()
                }
                .problem();
                let opt = Dense { c, ..cut.clone() }.vertex_optimum().unwrap();
                match certified_bound(&p, &vec![0.0; p.rows()]).unwrap() {
                    Some(b) => {
                        implied += 1;
                        assert_eq!(b.implied_columns(), 1);
                        assert!(
                            b.value() <= opt + 1e-9 * (1.0 + opt.abs()),
                            "case {case}, column {j}, sign {sign}: bound {} above {opt}, {lp:?}",
                            b.value()
                        );
                    }
                    None => none += 1,
                }
            }
        }
    }
    println!(
        "{{\"suite\":\"fmine-lp/pdlp\",\"case\":\"implied_bounds\",\"implied\":{implied},\"none\":{none}}}"
    );
    assert!(implied > 100 && none > 10, "{implied} implied, {none} none");
}

/// The returned bound is certified from the multipliers the solution carries, and is the best seen along the run.
/// The solver is deterministic, so a run of `K` steps passes every evaluation that a run of `k < K` steps
/// ended on. Its bound must therefore be at least the certified bound of each shorter run's final multipliers, up to
/// the rounding between a dual objective and its certified value. All columns are boxed here, so the two agree.
/// The test counts the cases where the final multipliers alone would fail that, because a shorter run had
/// certified more, and requires some.
#[test]
fn bound_is_the_best_seen_and_reproducible_from_its_multipliers() {
    const STEPS: u64 = 12;
    let mut rng = Rng(0x5eed_0004);
    let (mut fell, mut certified) = (0, 0);
    let run = |p: &fmine_lp::pdlp::LpProblem, evals: u64| {
        let o = PdlpOptions {
            tolerance: 1e-12,
            max_iterations: 64 * evals,
            threads: 1,
        };
        solve(p, &o).unwrap()
    };
    for case in 0..240 {
        // The small CPIT-shaped LPs have no oracle optimum here; their bounds' validity is the other tests' job.
        let (p, opt) = match case % 3 {
            0 => {
                let d = random_dense(&mut rng, false);
                let opt = d.vertex_optimum().unwrap();
                (d.problem(), Some(opt))
            }
            1 => {
                let (d, best) = random_closure(&mut rng);
                (d.problem(), Some(-best))
            }
            _ => {
                let width = 6 + rng.below(10) as usize;
                let depth = 3 + rng.below(5) as usize;
                let periods = 2 + rng.below(4) as usize;
                (random_cpit(&mut rng, width, depth, periods), None)
            }
        };
        let s = run(&p, STEPS);
        assert_eq!(
            certified_bound(&p, s.bound_multipliers()).unwrap(),
            s.bound(),
            "case {case}: the bound does not come from its multipliers"
        );
        let b = s.bound().map_or(f64::NEG_INFINITY, |b| b.value());
        let end = certified_bound(&p, s.y())
            .unwrap()
            .map_or(f64::NEG_INFINITY, |e| e.value());
        let mut best_shorter = f64::NEG_INFINITY;
        for k in 1..STEPS {
            if let Some(e) = certified_bound(&p, run(&p, k).y()).unwrap() {
                best_shorter = best_shorter.max(e.value());
            }
        }
        let slack = |v: f64| 1e-9 * (1.0 + v.abs());
        assert!(b >= end, "case {case}: bound {b} below the final {end}");
        assert!(
            b >= best_shorter - slack(best_shorter),
            "case {case}: bound {b} below {best_shorter}, certified by a shorter run"
        );
        fell += usize::from(end < best_shorter - slack(best_shorter));
        if b.is_finite() {
            certified += 1;
            if let Some(opt) = opt {
                assert!(
                    b <= opt + slack(opt),
                    "case {case}: bound {b} above the optimum {opt}"
                );
            }
        }
    }
    println!(
        "{{\"suite\":\"fmine-lp/pdlp\",\"case\":\"best_bound\",\"certified\":{certified},\"final_fell_below_earlier\":{fell}}}"
    );
    assert!(
        fell > 0,
        "no case where the final multipliers certify less than earlier ones: this test cannot see the tracking"
    );
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
    let p = d.problem();
    let s = solve(&p, &o).unwrap();
    // Before bd-gj80 this ran to the iteration limit; now a Farkas ray proves it.
    assert_eq!(s.status(), PdlpStatus::Infeasible, "{:?}", s.progress());
    assert!(
        certified_infeasibility(&p, s.infeasibility_ray().unwrap())
            .unwrap()
            .is_some()
    );
}

/// No multipliers prove infeasible an LP that has a feasible point (bd-gj80). Each LP here has an integer point
/// that meets every bound exactly in `f64`, so a certificate would be a false proof. The multipliers tried are the
/// solver's final and bound multipliers, which would certify the optimum if the costs were not set aside, and
/// random ones at five scales.
#[test]
fn farkas_rays_never_certify_a_feasible_lp() {
    let mut rng = Rng(0x5eed_0007);
    let o = PdlpOptions {
        tolerance: 1e-9,
        max_iterations: 20_000,
        threads: 1,
    };
    let mut tried = 0;
    for case in 0..400 {
        let d = random_integer_lp(&mut rng, case % 2 == 1, false);
        let p = d.problem();
        let s = solve(&p, &o).unwrap();
        assert_ne!(s.status(), PdlpStatus::Infeasible, "case {case}: {d:?}");
        let mut ys = vec![s.y().to_vec(), s.bound_multipliers().to_vec()];
        for k in 0..20 {
            let scale = [1e-6, 1e-2, 1.0, 1e2, 1e6][k % 5];
            ys.push(
                (0..p.rows())
                    .map(|_| rng.uniform(-1.0, 1.0) * scale)
                    .collect(),
            );
        }
        for y in &ys {
            tried += 1;
            assert_eq!(
                certified_infeasibility(&p, y).unwrap(),
                None,
                "case {case}: y = {y:?}, {d:?}"
            );
        }
    }
    println!("{{\"suite\":\"fmine-lp/pdlp\",\"case\":\"farkas_validity\",\"tried\":{tried}}}");
}

/// LPs that integer multipliers prove infeasible, two or more rows at a time, and in which vertex enumeration finds
/// no feasible vertex, end with [`PdlpStatus::Infeasible`] and a ray that certifies again (bd-gj80).
#[test]
fn infeasible_lps_end_with_a_certified_ray() {
    let mut rng = Rng(0x5eed_0008);
    let (mut steps, mut most) = (0u64, 0u64);
    for case in 0..300 {
        let d = random_integer_lp(&mut rng, false, true);
        assert_eq!(
            d.vertex_optimum(),
            None,
            "case {case}: a feasible vertex, {d:?}"
        );
        let p = d.problem();
        let s = solve(&p, &tight()).unwrap();
        assert_eq!(
            s.status(),
            PdlpStatus::Infeasible,
            "case {case}: {:?}, {d:?}",
            s.progress()
        );
        let ray = s.infeasibility_ray().unwrap();
        assert!(
            certified_infeasibility(&p, ray)
                .unwrap()
                .is_some_and(|b| b.value() > 0.0),
            "case {case}"
        );
        steps += s.progress().iterations;
        most = most.max(s.progress().iterations);
    }
    println!(
        "{{\"suite\":\"fmine-lp/pdlp\",\"case\":\"farkas_detection\",\"steps\":{steps},\"most_steps\":{most}}}"
    );
}

/// As [`infeasible_lps_end_with_a_certified_ray`], with some columns open above (those whose combined row has
/// `gⱼ ≤ 0`, so the integer proof still holds). An open column with no top the rows imply needs `−aⱼ·y` of the right
/// sign exactly, which a ray from the iterates may miss.
#[test]
fn infeasible_lps_with_open_columns_end_with_a_certified_ray() {
    let mut rng = Rng(0x5eed_0009);
    let o = PdlpOptions {
        tolerance: 1e-9,
        max_iterations: 50_000,
        threads: 1,
    };
    let (mut proved, mut open, mut steps) = (0, 0, 0u64);
    for case in 0..300 {
        let d = random_integer_lp(&mut rng, true, true);
        open += d.u.iter().filter(|v| v.is_infinite()).count();
        let p = d.problem();
        let s = solve(&p, &o).unwrap();
        assert_ne!(s.status(), PdlpStatus::Optimal, "case {case}: {d:?}");
        if s.status() == PdlpStatus::Infeasible {
            assert!(
                certified_infeasibility(&p, s.infeasibility_ray().unwrap())
                    .unwrap()
                    .is_some(),
                "case {case}"
            );
            proved += 1;
            steps += s.progress().iterations;
        }
    }
    println!(
        "{{\"suite\":\"fmine-lp/pdlp\",\"case\":\"farkas_open\",\"proved\":{proved},\"of\":300,\"open_columns\":{open},\"steps\":{steps}}}"
    );
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
