//! `bd-kcpj`: feasible schedules for MineLib's CPIT instances. PDLP's LP relaxation ranks the blocks, TopoSort
//! (`fmine_optimize::toposort::greedy`) schedules them, and `toposort::check` checks the schedule. Its NPV is then set
//! against PDLP's certified upper bound and MineLib's published LP bound and best-known schedule.
//!
//! Ignored by default: MineLib's files are not in the repository (their licence is not recorded). Point
//! `FMINE_MINELIB_DIR` at a directory holding `<name>/<name>.cpit` and `<name>/<name>.prec`, list instances in
//! `FMINE_MINELIB_INSTANCES` (comma-separated, default `newman1`), and run
//! `cargo test --release -p xtask --test cpit_schedule -- --ignored --nocapture`. Optional: `FMINE_PDLP_TOL` (default
//! 1e-4), `FMINE_PDLP_MAX_SECONDS` (default 600) and `FMINE_PDLP_THREADS` (default 0, the machine's).
//!
//! 1. `toposort::cpit::parse` reads the pair. Values are at 10⁶, rounded half away from zero as in GATE-PUSH-PIT1;
//!    uses are rounded up and limits down at 10⁶, so the schedule also keeps the file's limits.
//! 2. The LP is KILL-PUSH-PDLP's, in "by" variables `w[b][t]`, built here from the parsed integers, with the discount
//!    counted from period 0 (`crates/fmine-lp/tests/minelib_cpit.rs` explains why).
//! 3. Block `b`'s expected number of periods before it is mined is `Σ_t (1 − w[b][t])`. That ranks it. A block the
//!    LP mines less than a threshold of by the last period is left out. Three thresholds are tried, and each schedule
//!    must pass the checker; the best NPV is reported, with all three.
//! 4. NPV is `Σ_t V_t / (1 + rate)^t` in `f64`, from the checker's exact per-period sums.
//!
//! Asserted: every schedule passes `toposort::check`, and no NPV exceeds PDLP's certified bound or MineLib's LP bound
//! (each allowing one unit for rounding). No gap threshold is asserted. GATE-PUSH-TOPO is set on PCPSP, not CPIT,
//! and there is no local search yet.
//!
//! With `FMINE_MINELIB_SOLS` pointing at the mirror's `sols/` files, MineLib's best-known schedule
//! (`<name>_cpit_*.sol`) is read too. It must pass the same checker, and its NPV, computed the same way, must equal
//! the published best-known value within one unit. That ties the checker and the NPV convention to an outside
//! figure.

// Test helpers outside `#[test]` functions: a malformed MineLib file must fail the test at once.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::Path;
use std::time::Instant;

use fmine_lp::pdlp::{LpProblem, PdlpOptions, solve_observed};
use fmine_numerics::sparse::Coo;
use fmine_optimize::pitflow::minelib::Rounding;
use fmine_optimize::toposort::{ScheduleProblem, check, cpit, greedy};

/// MineLib's Results page (Results.xhtml, mirrored at github.com/borelian/minelib, read 2026-11-01): the CPIT LP
/// upper bound and the best-known feasible schedule's value, in whole units.
const PUBLISHED: &[(&str, f64, f64)] = &[
    ("newman1", 24_486_184.0, 23_483_671.0),
    ("zuck_small", 854_182_396.0, 788_652_600.0),
    ("kd", 409_498_555.0, 396_858_193.0),
    ("zuck_medium", 710_641_410.0, 615_411_415.0),
    ("p4hd", 247_415_730.0, 246_138_696.0),
    ("marvin", 863_916_131.0, 820_726_048.0),
    ("w23", 400_653_199.0, 392_226_063.0),
    ("zuck_large", 57_389_094.0, 56_777_190.0),
    ("sm2", 1_648_051_083.0, 1_645_242_774.0),
    ("mclaughlin_limit", 1_078_979_501.0, 1_073_327_197.0),
    ("mclaughlin", 1_079_024_268.0, 1_073_327_197.0),
];

/// Places for values, uses and limits.
const DECIMALS: u32 = 6;
const SCALE: f64 = 1e6;

/// The by-period fraction a block must reach by the last period to be ranked.
const THRESHOLDS: [f64; 3] = [1e-6, 0.5, 0.9];

/// A MineLib CPIT solution: one `<block> <destination> <period> <fraction>` line per mined block, whole blocks only.
fn read_solution(text: &str, n: usize) -> Vec<Option<u32>> {
    let mut schedule = vec![None; n];
    for line in text.lines() {
        let f: Vec<&str> = line.split_whitespace().collect();
        if f.is_empty() || f[0].starts_with('%') {
            continue;
        }
        assert_eq!(f.len(), 4, "solution line {line:?}");
        assert_eq!(f[3].parse::<f64>().unwrap(), 1.0, "a part block: {line:?}");
        let b: usize = f[0].parse().unwrap();
        assert!(
            schedule[b].replace(f[2].parse().unwrap()).is_none(),
            "block {b} twice"
        );
    }
    schedule
}

/// `Σ_t V_t / (1 + rate)^t`, periods counted from 0.
fn npv(period_values: &[i128], rate: f64) -> f64 {
    period_values
        .iter()
        .enumerate()
        .map(|(t, &v)| v as f64 / SCALE / (1.0 + rate).powi(t as i32))
        .sum()
}

fn env_or<T: std::str::FromStr>(key: &str, default: T) -> T {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

/// The relaxation as a minimisation (costs negated), from the parsed problem.
fn relaxation(p: &ScheduleProblem, rate: f64) -> LpProblem {
    let blocks = p.blocks();
    let (n, periods) = (blocks.node_count(), p.periods());
    let col = |b: usize, t: usize| t * n + b;
    let arcs: usize = (0..n).map(|b| blocks.requires(b).unwrap().len()).sum();
    let rows = arcs * periods + n * (periods - 1) + p.resources() * periods;
    let mut coo = Coo::new(rows, n * periods).unwrap();
    let (mut lo, mut hi) = (Vec::with_capacity(rows), Vec::with_capacity(rows));
    let mut r = 0;
    for t in 0..periods {
        for b in 0..n {
            for &q in blocks.requires(b).unwrap() {
                coo.push(r, col(b, t), 1.0).unwrap();
                coo.push(r, col(q as usize, t), -1.0).unwrap();
                lo.push(f64::NEG_INFINITY);
                hi.push(0.0);
                r += 1;
            }
        }
    }
    for t in 1..periods {
        for b in 0..n {
            coo.push(r, col(b, t - 1), 1.0).unwrap();
            coo.push(r, col(b, t), -1.0).unwrap();
            lo.push(f64::NEG_INFINITY);
            hi.push(0.0);
            r += 1;
        }
    }
    for res in 0..p.resources() {
        let (uses, caps) = (p.usage(res).unwrap(), p.limits(res).unwrap());
        for (t, &cap) in caps.iter().enumerate() {
            for (b, &q) in uses.iter().enumerate().filter(|(_, q)| **q != 0) {
                let q = q as f64 / SCALE;
                coo.push(r, col(b, t), q).unwrap();
                if t > 0 {
                    coo.push(r, col(b, t - 1), -q).unwrap();
                }
            }
            lo.push(f64::NEG_INFINITY);
            hi.push(cap as f64 / SCALE);
            r += 1;
        }
    }
    assert_eq!(r, rows);
    let factor: Vec<f64> = (0..periods)
        .map(|t| (1.0 + rate).powi(t as i32).recip())
        .collect();
    let mut cost = vec![0.0; n * periods];
    for (b, &w) in blocks.weights().iter().enumerate() {
        let profit = w as f64 / SCALE;
        for t in 0..periods {
            let next = factor.get(t + 1).map_or(0.0, |f| profit * f);
            cost[col(b, t)] = -(profit * factor[t] - next);
        }
    }
    LpProblem::new(
        cost,
        &coo.assemble(),
        lo,
        hi,
        vec![0.0; n * periods],
        vec![1.0; n * periods],
    )
    .unwrap()
}

#[test]
#[ignore = "needs MineLib's CPIT files (FMINE_MINELIB_DIR); release build, seconds to minutes per instance"]
fn minelib_cpit_schedules_pass_the_checker_under_the_certified_bound() {
    let dir = std::env::var("FMINE_MINELIB_DIR").expect("FMINE_MINELIB_DIR");
    let names = std::env::var("FMINE_MINELIB_INSTANCES").unwrap_or_else(|_| "newman1".to_string());
    let tol: f64 = env_or("FMINE_PDLP_TOL", 1e-4);
    let max_seconds: f64 = env_or("FMINE_PDLP_MAX_SECONDS", 600.0);
    let threads: usize = env_or("FMINE_PDLP_THREADS", 0);
    let mut failures = Vec::new();
    for name in names.split(',').map(str::trim).filter(|s| !s.is_empty()) {
        let base = Path::new(&dir).join(name);
        let read = |ext: &str| std::fs::read_to_string(base.join(format!("{name}.{ext}"))).unwrap();
        let model = cpit::parse(
            &read("cpit"),
            &read("prec"),
            DECIMALS,
            Rounding::HalfAwayFromZero,
            DECIMALS,
        )
        .unwrap();
        let p = &model.problem;
        let (n, periods) = (p.blocks().node_count(), p.periods());
        let rate = model.discount_rate as f64 / cpit::RATE_SCALE as f64;
        let lp = relaxation(p, rate);
        let t0 = Instant::now();
        let options = PdlpOptions {
            tolerance: tol,
            max_iterations: u64::MAX,
            threads,
        };
        let s = solve_observed(&lp, &options, &mut |_| {
            t0.elapsed().as_secs_f64() < max_seconds
        })
        .unwrap();
        let lp_s = t0.elapsed().as_secs_f64();
        let certified = s.bound().map(|b| -b.value());
        let x = s.x();
        let w = |b: usize, t: usize| x[t * n + b].clamp(0.0, 1.0);
        let expected: Vec<f64> = (0..n)
            .map(|b| (0..periods).map(|t| 1.0 - w(b, t)).sum())
            .collect();
        let (published_lp, best_known) = PUBLISHED
            .iter()
            .find(|(k, _, _)| *k == name)
            .map(|&(_, l, b)| (l, b))
            .unwrap_or((f64::NAN, f64::NAN));
        let t1 = Instant::now();
        let mut npvs = Vec::new();
        let mut mined = Vec::new();
        for threshold in THRESHOLDS {
            let rank: Vec<Option<i64>> = (0..n)
                .map(|b| {
                    (w(b, periods - 1) >= threshold).then(|| (expected[b] * 1e9).round() as i64)
                })
                .collect();
            let schedule = greedy(p, &rank).unwrap();
            let c = check(p, &schedule).unwrap();
            if !c.feasible() {
                failures.push(format!(
                    "{name}: threshold {threshold}: {} unmet, over {:?}",
                    c.unmet, c.over_limit
                ));
            }
            npvs.push(npv(&c.period_values, rate));
            mined.push(c.mined);
        }
        let sols = std::env::var("FMINE_MINELIB_SOLS").ok();
        let known = sols.as_deref().and_then(|d| {
            std::fs::read_dir(d).unwrap().find_map(|e| {
                let path = e.unwrap().path();
                let file = path.file_name()?.to_str()?.to_string();
                (file.starts_with(&format!("{name}_cpit_")) && file.ends_with(".sol"))
                    .then_some(path)
            })
        });
        let known_npv = known.map(|path| {
            let schedule = read_solution(&std::fs::read_to_string(&path).unwrap(), n);
            let c = check(p, &schedule).unwrap();
            if !c.feasible() {
                failures.push(format!(
                    "{name}: MineLib's schedule {}: {} unmet, over {:?}",
                    path.display(),
                    c.unmet,
                    c.over_limit
                ));
            }
            let v = npv(&c.period_values, rate);
            if (v - best_known).abs() > 1.0 {
                failures.push(format!(
                    "{name}: MineLib's schedule is worth {v} here, not its published {best_known}"
                ));
            }
            v
        });
        let greedy_s = t1.elapsed().as_secs_f64();
        let best = npvs.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        let rel = |bound: f64| (bound - best) / bound;
        println!(
            "{{\"bead\":\"bd-kcpj\",\"instance\":\"{name}\",\"blocks\":{n},\"periods\":{periods},\"resources\":{},\"rounded\":[{},{},{}],\"tol\":{tol},\"lp_status\":\"{:?}\",\"lp_s\":{lp_s:.1},\"certified_ub\":{},\"published_lp\":{published_lp:.0},\"best_known\":{best_known:.0},\"thresholds\":{THRESHOLDS:?},\"npv\":[{}],\"mined\":{mined:?},\"greedy_s\":{greedy_s:.2},\"best_npv\":{best:.3},\"gap_to_certified\":{},\"gap_to_published_lp\":{:.4e},\"of_best_known\":{:.4},\"best_known_rechecked\":{}}}",
            p.resources(),
            model.rounded_values,
            model.rounded_uses,
            model.rounded_limits,
            s.status(),
            certified.map_or("null".to_string(), |u| format!("{u:.3}")),
            npvs.iter()
                .map(|v| format!("{v:.3}"))
                .collect::<Vec<_>>()
                .join(","),
            certified.map_or("null".to_string(), |u| format!("{:.4e}", rel(u))),
            rel(published_lp),
            best / best_known,
            known_npv.map_or("null".to_string(), |v| format!("{v:.3}")),
        );
        match certified {
            Some(u) if best > u + 1.0 => failures.push(format!(
                "{name}: NPV {best} is above the certified bound {u}"
            )),
            None => failures.push(format!("{name}: no certified bound")),
            _ => {}
        }
        if best > published_lp + 1.0 {
            failures.push(format!(
                "{name}: NPV {best} is above MineLib's LP bound {published_lp}"
            ));
        }
    }
    assert!(failures.is_empty(), "{failures:#?}");
}
