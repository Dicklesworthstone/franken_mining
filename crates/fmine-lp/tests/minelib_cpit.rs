//! KILL-PUSH-PDLP (plan §15.4, `BD-251`): PDLP on the LP relaxations of MineLib's CPIT instances, whose optima
//! MineLib publishes. These are the real-size, non-closure LPs the workspace can check against an outside figure.
//!
//! Ignored by default: MineLib's files are not in the repository (their licence is not recorded). Point
//! `FMINE_MINELIB_DIR` at a directory holding `<name>/<name>.cpit` and `<name>/<name>.prec`, list instances in
//! `FMINE_MINELIB_INSTANCES` (comma-separated, default `newman1`), and run
//! `cargo test --release -p fmine-lp --test minelib_cpit -- --ignored --nocapture`. Optional: `FMINE_PDLP_TOL`
//! (the solver's tolerance, default 1e-6; at 1e-4 the bound misses on several instances), `FMINE_PDLP_THREADS` (default 0, the machine's), `FMINE_PDLP_MAX_SECONDS` (default 1800, the
//! KILL time limit), `FMINE_CPIT_DISCOUNT_FROM` (default 0; see below).
//!
//! The LP, in "by" variables `w[b][t]` = the fraction of block `b` mined in periods `0..=t`, all in `[0, 1]`:
//! - maximise `Σ_b Σ_t p_bt (w[b][t] − w[b][t−1])`, with `p_bt = p_b / (1 + α)^(t + FROM)` for the file's periods
//!   `t = 0..T−1`. MineLib's format note writes `p_b / (1 + α)^t` with periods numbered from 1, which reads as
//!   `FROM = 1`. Its published optima match `FROM = 0`: on newman1 at 1e-7, `FROM = 0` certifies 24,486,185.05
//!   against the published 24,486,184, and `FROM = 1` gives 22,672,394.94, 7.4% short;
//! - `w[b][t] ≤ w[q][t]` for each predecessor `q` of `b` (the `.prec` file) and each period;
//! - `w[b][t−1] ≤ w[b][t]`;
//! - each resource row `Σ_b q_br (w[b][t] − w[b][t−1])` within its `L`, `G` or `I` limits.
//!
//! This is MineLib's CPIT relaxation (format note §5.1) after the change of variables `w[b][t] = Σ_{τ≤t} x[b][τ]`,
//! which keeps the optimum. The run must reach PDLP's stopping rule at `FMINE_PDLP_TOL` within the time limit, and
//! the certified upper bound must lie at or above MineLib's published LP optimum and within 1e-4 relative of it,
//! each allowing one unit for MineLib's rounding. That is KILL-PUSH-PDLP's 1e-4 gap measured from the certified
//! bound to the true optimum. The solver's own gap is not used: its primal point may break zero-right-hand-side
//! precedence rows by whole units while the residual, relative to `‖b‖`, which the capacities dominate, is
//! small. At the default 1e-4 tolerance the bound on kd was 1.05% above the optimum.

// Test helpers outside `#[test]` functions: a malformed MineLib file must fail the test at once.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::BTreeMap;
use std::path::Path;
use std::time::Instant;

use fmine_lp::pdlp::{LpProblem, PdlpOptions, PdlpStatus, solve_observed};
use fmine_numerics::sparse::Coo;

/// MineLib's "CPIT LP upper bound: objective value of the optimal LP relaxation of each CPIT instance", in whole
/// units, from its Results page (Results.xhtml, mirrored at github.com/borelian/minelib, read 2026-11-01).
const PUBLISHED_LP: &[(&str, f64)] = &[
    ("newman1", 24_486_184.0),
    ("zuck_small", 854_182_396.0),
    ("kd", 409_498_555.0),
    ("zuck_medium", 710_641_410.0),
    ("p4hd", 247_415_730.0),
    ("marvin", 863_916_131.0),
    ("w23", 400_653_199.0),
    ("zuck_large", 57_389_094.0),
    ("sm2", 1_648_051_083.0),
    ("mclaughlin_limit", 1_078_979_501.0),
    ("mclaughlin", 1_079_024_268.0),
];

/// KILL-PUSH-PDLP's threshold (plan §15.4): the certified bound's relative gap to the published optimum.
const KILL_GAP: f64 = 1e-4;

struct Limit {
    lo: f64,
    hi: f64,
}

struct Cpit {
    blocks: usize,
    periods: usize,
    discount: f64,
    profit: Vec<f64>,
    /// `limits[r][t]`.
    limits: Vec<Vec<Limit>>,
    /// `usage[r]`: (block, coefficient).
    usage: Vec<Vec<(usize, f64)>>,
}

fn fields(line: &str) -> Vec<&str> {
    line.split([' ', '\t', ':'])
        .filter(|s| !s.is_empty())
        .collect()
}

fn num<T: std::str::FromStr>(s: &str, what: &str) -> T {
    s.parse()
        .unwrap_or_else(|_| panic!("{what}: cannot read {s:?}"))
}

fn read_cpit(text: &str) -> Cpit {
    let mut head: BTreeMap<String, String> = BTreeMap::new();
    let mut section = String::new();
    let mut limit_rows = Vec::new();
    let mut objective = Vec::new();
    let mut coefficients = Vec::new();
    for line in text.lines() {
        let f = fields(line);
        let Some(first) = f.first() else { continue };
        if first.starts_with('%') {
            continue;
        }
        if first.chars().next().is_some_and(|c| c.is_ascii_uppercase()) {
            if f.len() >= 2 {
                head.insert((*first).to_string(), f[1].to_string());
            }
            section = (*first).to_string();
            continue;
        }
        match section.as_str() {
            "RESOURCE_CONSTRAINT_LIMITS" => {
                limit_rows.push(f.iter().map(|s| s.to_string()).collect::<Vec<_>>())
            }
            "OBJECTIVE_FUNCTION" => {
                objective.push((num::<usize>(f[0], "block"), num::<f64>(f[1], "profit")))
            }
            "RESOURCE_CONSTRAINT_COEFFICIENTS" => coefficients.push((
                num::<usize>(f[0], "block"),
                num::<usize>(f[1], "resource"),
                num::<f64>(f[2], "q"),
            )),
            other => panic!("data line {line:?} in section {other:?}"),
        }
    }
    assert_eq!(head.get("TYPE").map(String::as_str), Some("CPIT"));
    let blocks: usize = num(&head["NBLOCKS"], "NBLOCKS");
    let periods: usize = num(&head["NPERIODS"], "NPERIODS");
    let resources: usize = num(&head["NRESOURCE_SIDE_CONSTRAINTS"], "resources");
    let discount: f64 = num(&head["DISCOUNT_RATE"], "DISCOUNT_RATE");
    let mut profit = vec![f64::NAN; blocks];
    for (b, p) in objective {
        profit[b] = p;
    }
    assert!(
        profit.iter().all(|p| p.is_finite()),
        "a block has no profit line"
    );
    let mut limits: Vec<Vec<Option<Limit>>> = (0..resources)
        .map(|_| (0..periods).map(|_| None).collect())
        .collect();
    for row in &limit_rows {
        let (r, t): (usize, usize) = (num(&row[0], "r"), num(&row[1], "t"));
        let v1: f64 = num(&row[3], "limit");
        let lim = match row[2].as_str() {
            "L" => Limit {
                lo: f64::NEG_INFINITY,
                hi: v1,
            },
            "G" => Limit {
                lo: v1,
                hi: f64::INFINITY,
            },
            "I" => Limit {
                lo: v1,
                hi: num(&row[4], "limit"),
            },
            k => panic!("limit kind {k:?}"),
        };
        limits[r][t] = Some(lim);
    }
    let limits = limits
        .into_iter()
        .map(|per| {
            per.into_iter()
                .map(|l| l.expect("every resource and period has a limit"))
                .collect()
        })
        .collect();
    let mut usage = vec![Vec::new(); resources];
    for (b, r, q) in coefficients {
        usage[r].push((b, q));
    }
    Cpit {
        blocks,
        periods,
        discount,
        profit,
        limits,
        usage,
    }
}

fn read_prec(text: &str, blocks: usize) -> Vec<Vec<usize>> {
    let mut preds = vec![Vec::new(); blocks];
    for line in text.lines() {
        let f = fields(line);
        if f.is_empty() || f[0].starts_with('%') {
            continue;
        }
        let b: usize = num(f[0], "block");
        let n: usize = num(f[1], "count");
        assert_eq!(f.len(), n + 2, "block {b}");
        preds[b] = f[2..].iter().map(|s| num(s, "predecessor")).collect();
    }
    preds
}

/// The relaxation as a minimisation (costs negated), with its counts.
fn build(c: &Cpit, preds: &[Vec<usize>], from: i32) -> LpProblem {
    let (n, t_max) = (c.blocks, c.periods);
    let col = |b: usize, t: usize| t * n + b;
    let arcs: usize = preds.iter().map(Vec::len).sum();
    let rows = arcs * t_max + n * (t_max - 1) + c.usage.len() * t_max;
    let mut coo = Coo::new(rows, n * t_max).unwrap();
    let (mut lo, mut hi) = (Vec::with_capacity(rows), Vec::with_capacity(rows));
    let mut r = 0;
    for t in 0..t_max {
        for (b, ps) in preds.iter().enumerate() {
            for &q in ps {
                coo.push(r, col(b, t), 1.0).unwrap();
                coo.push(r, col(q, t), -1.0).unwrap();
                lo.push(f64::NEG_INFINITY);
                hi.push(0.0);
                r += 1;
            }
        }
    }
    for t in 1..t_max {
        for b in 0..n {
            coo.push(r, col(b, t - 1), 1.0).unwrap();
            coo.push(r, col(b, t), -1.0).unwrap();
            lo.push(f64::NEG_INFINITY);
            hi.push(0.0);
            r += 1;
        }
    }
    for (use_r, per_period) in c.usage.iter().zip(&c.limits) {
        for (t, limit) in per_period.iter().enumerate() {
            for &(b, q) in use_r {
                coo.push(r, col(b, t), q).unwrap();
                if t > 0 {
                    coo.push(r, col(b, t - 1), -q).unwrap();
                }
            }
            lo.push(limit.lo);
            hi.push(limit.hi);
            r += 1;
        }
    }
    assert_eq!(r, rows);
    let factor: Vec<f64> = (0..t_max)
        .map(|t| (1.0 + c.discount).powi(t as i32 + from).recip())
        .collect();
    let mut cost = vec![0.0; n * t_max];
    for b in 0..n {
        for t in 0..t_max {
            let here = c.profit[b] * factor[t];
            let next = if t + 1 < t_max {
                c.profit[b] * factor[t + 1]
            } else {
                0.0
            };
            cost[col(b, t)] = -(here - next);
        }
    }
    LpProblem::new(
        cost,
        &coo.assemble(),
        lo,
        hi,
        vec![0.0; n * t_max],
        vec![1.0; n * t_max],
    )
    .unwrap()
}

fn env_or<T: std::str::FromStr>(key: &str, default: T) -> T {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

#[test]
#[ignore = "needs MineLib's CPIT files (FMINE_MINELIB_DIR); release build, minutes per instance"]
fn kill_push_pdlp_on_minelib_cpit_lp_relaxations() {
    let dir = std::env::var("FMINE_MINELIB_DIR").expect("FMINE_MINELIB_DIR");
    let names = std::env::var("FMINE_MINELIB_INSTANCES").unwrap_or_else(|_| "newman1".to_string());
    let tol: f64 = env_or("FMINE_PDLP_TOL", 1e-6);
    let threads: usize = env_or("FMINE_PDLP_THREADS", 0);
    let max_seconds: f64 = env_or("FMINE_PDLP_MAX_SECONDS", 1800.0);
    let from: i32 = env_or("FMINE_CPIT_DISCOUNT_FROM", 0);
    let mut failures = Vec::new();
    for name in names.split(',').map(str::trim).filter(|s| !s.is_empty()) {
        let base = Path::new(&dir).join(name);
        let t0 = Instant::now();
        let cpit = read_cpit(&std::fs::read_to_string(base.join(format!("{name}.cpit"))).unwrap());
        let preds = read_prec(
            &std::fs::read_to_string(base.join(format!("{name}.prec"))).unwrap(),
            cpit.blocks,
        );
        let p = build(&cpit, &preds, from);
        let build_s = t0.elapsed().as_secs_f64();
        let t1 = Instant::now();
        let options = PdlpOptions {
            tolerance: tol,
            max_iterations: u64::MAX,
            threads,
        };
        let published = PUBLISHED_LP
            .iter()
            .find(|(n, _)| *n == name)
            .map(|&(_, v)| v);
        let mut evals = 0u64;
        // When the (unrounded) dual objective first came within KILL_GAP of the published optimum: how early the
        // bound was good, which the stopping rule cannot see.
        let mut dual_within_gap_s: Option<f64> = None;
        let s = solve_observed(&p, &options, &mut |pr| {
            evals += 1;
            let secs = t1.elapsed().as_secs_f64();
            let dual = -pr.dual_objective;
            if let Some(pubd) = published
                && dual_within_gap_s.is_none()
                && (dual - pubd - 1.0) / pubd <= KILL_GAP
            {
                dual_within_gap_s = Some(secs);
            }
            if evals.is_power_of_two() || evals.is_multiple_of(500) {
                println!(
                    "{{\"progress\":\"{name}\",\"s\":{secs:.1},\"it\":{},\"restarts\":{},\"rel_p\":{:.2e},\"rel_d\":{:.2e},\"rel_gap\":{:.2e},\"value\":{:.1},\"dual\":{dual:.1}}}",
                    pr.iterations, pr.restarts, pr.relative_primal_residual, pr.relative_dual_residual, pr.relative_gap, -pr.primal_objective
                );
            }
            secs < max_seconds
        })
        .unwrap();
        let solve_s = t1.elapsed().as_secs_f64();
        let pr = s.progress();
        let ub = s.bound().map(|b| -b.value());
        let violation = p.max_violation(s.x()).unwrap();
        println!(
            "{{\"bead\":\"BD-251\",\"instance\":\"{name}\",\"blocks\":{},\"periods\":{},\"rows\":{},\"cols\":{},\"nnz\":{},\"build_s\":{build_s:.1},\"solve_s\":{solve_s:.1},\"status\":\"{:?}\",\"iterations\":{},\"restarts\":{},\"rel_p\":{:.2e},\"rel_d\":{:.2e},\"rel_gap\":{:.2e},\"lp_value\":{:.3},\"certified_ub\":{},\"published\":{},\"ub_rel_to_published\":{},\"dual_within_gap_s\":{},\"max_violation\":{violation:.3e},\"tol\":{tol},\"discount_from\":{from}}}",
            cpit.blocks,
            cpit.periods,
            p.rows(),
            p.cols(),
            p.nnz(),
            s.status(),
            pr.iterations,
            pr.restarts,
            pr.relative_primal_residual,
            pr.relative_dual_residual,
            pr.relative_gap,
            -pr.primal_objective,
            ub.map_or("null".to_string(), |v| format!("{v:.3}")),
            published.map_or("null".to_string(), |v| format!("{v:.0}")),
            match (ub, published) {
                (Some(u), Some(p)) => format!("{:.3e}", (u - p) / p),
                _ => "null".to_string(),
            },
            dual_within_gap_s.map_or("null".to_string(), |v| format!("{v:.1}")),
        );
        if s.status() != PdlpStatus::Optimal {
            failures.push(format!("{name}: {:?} after {solve_s:.0} s", s.status()));
        }
        match (ub, published) {
            (Some(u), Some(pubd)) => {
                if u < pubd - 1.0 {
                    failures.push(format!(
                        "{name}: certified bound {u} is below the published optimum {pubd}"
                    ));
                }
                if (u - pubd - 1.0) / pubd > KILL_GAP {
                    failures.push(format!(
                        "{name}: certified bound {u} is not within {KILL_GAP} of {pubd}"
                    ));
                }
            }
            (None, _) => failures.push(format!("{name}: no certified bound")),
            (_, None) => failures.push(format!("{name}: no published optimum")),
        }
    }
    assert!(failures.is_empty(), "{failures:#?}");
}
