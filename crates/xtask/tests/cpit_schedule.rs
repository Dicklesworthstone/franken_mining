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
//! 2. The LP is KILL-PUSH-PDLP's, in "by" variables `w[b][t]`, from `toposort::lp::relaxation` (`bd-ubzs`), with the
//!    discount counted from period 0 (`crates/fmine-lp/tests/minelib_cpit.rs` explains why). PDLP solves it here.
//! 3. `toposort::lp::starts` ranks block `b` by its expected number of periods before it is mined,
//!    `Σ_t (1 − w[b][t])`. A block the LP mines less than a threshold of by the last period is left out. Three
//!    thresholds are tried. Each gives two schedules: `greedy`'s, and `greedy_from`'s with each block released at its
//!    LP median period, the first by which the LP has mined half of it (bd-wife). Each schedule must pass the checker;
//!    the best NPV is reported, with all six.
//! 4. `toposort::lp::incumbent` runs `toposort::search::improve` (`bd-5lxu`) on each schedule, with `discount_factors`
//!    from the file's rate and at most `FMINE_SEARCH_ROUNDS` rounds (default 1,000). Each result must pass the checker
//!    too, and the one it picks must have the highest NPV.
//! 5. NPV is `Σ_t V_t / (1 + rate)^t` in `f64`, from the checker's exact per-period sums.
//!
//! Asserted: every schedule passes `toposort::check`, the search never lowers an NPV, the API's pick has the highest
//! NPV, and no NPV exceeds PDLP's certified bound or MineLib's LP bound (each allowing one unit for rounding). No gap
//! threshold is asserted.
//! GATE-PUSH-TOPO is set on PCPSP, not CPIT.
//!
//! `FMINE_CPIT_LP_CACHE`, if set, names a directory where each instance's LP solution and certified bound are kept
//! (`<name>-<tol>.lp`, little-endian `f64`s), so later runs skip PDLP. Such a run reports `"lp_status":"cached"`.
//! `FMINE_CPIT_SOLUTION_OUT`, if set, names a directory for each instance's best schedule in MineLib's solution format
//! (`<name>_cpit_fmine.sol`), which `crates/xtask/testdata/minelib_cpit_oracle.py` checks from the instance's files
//! alone.
//!
//! With `FMINE_MINELIB_SOLS` pointing at the mirror's `sols/` files, MineLib's best-known schedule
//! (`<name>_cpit_*.sol`) is read too. It must pass the same checker, and its NPV, computed the same way, must equal
//! the published best-known value within one unit. That ties the checker and the NPV convention to an outside
//! figure. It is read by `toposort::cpit::parse_solution` as an untrusted candidate (bd-3l5o, plan §15.4), and
//! `search::improve` then runs from it with the same factors and rounds. That search must pass the checker, must not
//! lower the NPV and must stay under both bounds. It is reported as `from_known`, and written beside the API's
//! schedule as `<name>_cpit_fmine_from_known.sol`. This warm start uses MineLib's own schedule; the API's schedule
//! above does not.

// Test helpers outside `#[test]` functions: a malformed MineLib file must fail the test at once.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::Path;
use std::time::Instant;

use fmine_lp::pdlp::{LpProblem, PdlpOptions, solve_observed};
use fmine_optimize::pitflow::minelib::Rounding;
use fmine_optimize::toposort::lp::{self, Shares, THRESHOLDS};
use fmine_optimize::toposort::search::{discount_factors, improve};
use fmine_optimize::toposort::{check, cpit};

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

/// `toposort::lp`'s relaxation, handed to PDLP.
fn to_lp(r: lp::Relaxation) -> LpProblem {
    LpProblem::new(r.cost, &r.matrix, r.row_lo, r.row_hi, r.col_lo, r.col_hi).unwrap()
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
        let cache = std::env::var("FMINE_CPIT_LP_CACHE")
            .ok()
            .map(|d| Path::new(&d).join(format!("{name}-{tol}.lp")));
        let cached = cache
            .as_ref()
            .and_then(|path| std::fs::read(path).ok())
            .map(|bytes| {
                let f: Vec<f64> = bytes
                    .as_chunks::<8>()
                    .0
                    .iter()
                    .map(|c| f64::from_le_bytes(*c))
                    .collect();
                assert_eq!(
                    f.len(),
                    2 + n * periods,
                    "{name}: cached LP of the wrong size"
                );
                f
            });
        let (x, certified, lp_s, lp_status) = if let Some(f) = cached {
            let certified = (!f[0].is_nan()).then_some(f[0]);
            (f[2..].to_vec(), certified, f[1], "cached".to_string())
        } else {
            let relaxed =
                to_lp(lp::relaxation(p, model.discount_rate, cpit::RATE_SCALE, DECIMALS).unwrap());
            let t0 = Instant::now();
            let options = PdlpOptions {
                tolerance: tol,
                max_iterations: u64::MAX,
                threads,
            };
            let s = solve_observed(&relaxed, &options, &mut |_| {
                t0.elapsed().as_secs_f64() < max_seconds
            })
            .unwrap();
            let lp_s = t0.elapsed().as_secs_f64();
            let certified = s.bound().map(|b| -b.value());
            if let Some(path) = &cache {
                let head = [certified.unwrap_or(f64::NAN), lp_s];
                let bytes: Vec<u8> = head
                    .iter()
                    .chain(s.x())
                    .flat_map(|v| v.to_le_bytes())
                    .collect();
                std::fs::write(path, bytes).unwrap();
            }
            (s.x().to_vec(), certified, lp_s, format!("{:?}", s.status()))
        };
        let shares = Shares::new(n, periods, x).unwrap();
        let (published_lp, best_known) = PUBLISHED
            .iter()
            .find(|(k, _, _)| *k == name)
            .map(|&(_, l, b)| (l, b))
            .unwrap_or((f64::NAN, f64::NAN));
        let factors = discount_factors(model.discount_rate, cpit::RATE_SCALE, periods).unwrap();
        let rounds: usize = env_or("FMINE_SEARCH_ROUNDS", 1_000);
        let t1 = Instant::now();
        let starts = lp::starts(p, &shares).unwrap();
        let greedy_s = t1.elapsed().as_secs_f64();
        let t2 = Instant::now();
        let found = lp::incumbent(p, &starts, &factors, rounds).unwrap();
        let search_s = t2.elapsed().as_secs_f64();
        let mut npvs = Vec::new();
        let mut mined = Vec::new();
        let mut improved = Vec::new();
        let mut moves = Vec::new();
        for (start, i) in starts.iter().zip(&found.searched) {
            let threshold = format!(
                "{}{}",
                start.threshold,
                if start.released { ", LP release" } else { "" }
            );
            let c = check(p, &start.schedule).unwrap();
            if !c.feasible() {
                failures.push(format!(
                    "{name}: threshold {threshold}: {} unmet, over {:?}",
                    c.unmet, c.over_limit
                ));
            }
            let before = npv(&c.period_values, rate);
            npvs.push(before);
            mined.push(c.mined);
            let c = check(p, &i.schedule).unwrap();
            if !c.feasible() {
                failures.push(format!(
                    "{name}: threshold {threshold}, searched: {} unmet, over {:?}",
                    c.unmet, c.over_limit
                ));
            }
            let after = npv(&c.period_values, rate);
            // Every move raises the exact value; allow only `f64` summation noise.
            if after < before - 1e-9 * before.abs().max(1.0) {
                failures.push(format!(
                    "{name}: threshold {threshold}: the search lowered NPV from {before} to {after}"
                ));
            }
            improved.push(after);
            moves.push((i.rounds, i.moves));
        }
        // The API picks by its integer discounted value; it must be the highest NPV here too, up to `f64` noise.
        let best = improved[found.best];
        let top = improved.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        if best < top - 1e-9 * top.abs().max(1.0) {
            failures.push(format!(
                "{name}: the API's best schedule is worth {best}, below the {top} of another"
            ));
        }
        // MineLib's solution format, so the schedule can be checked without this code
        // (crates/xtask/testdata/minelib_cpit_oracle.py).
        let out = std::env::var("FMINE_CPIT_SOLUTION_OUT").ok();
        if let Some(dir) = &out {
            std::fs::write(
                Path::new(dir).join(format!("{name}_cpit_fmine.sol")),
                cpit::write_solution(found.schedule()),
            )
            .unwrap();
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
        // MineLib's schedule as an untrusted candidate (bd-3l5o): read, checked, then searched from, so that it can
        // become the incumbent.
        let mut from_known = None;
        let known_npv = known.map(|path| {
            let schedule =
                cpit::parse_solution(&std::fs::read_to_string(&path).unwrap(), p).unwrap();
            let c = check(p, &schedule).unwrap();
            if !c.feasible() {
                failures.push(format!(
                    "{name}: MineLib's schedule {}: {} unmet, over {:?}",
                    path.display(),
                    c.unmet,
                    c.over_limit
                ));
                return npv(&c.period_values, rate);
            }
            let v = npv(&c.period_values, rate);
            if (v - best_known).abs() > 1.0 {
                failures.push(format!(
                    "{name}: MineLib's schedule is worth {v} here, not its published {best_known}"
                ));
            }
            let t = Instant::now();
            let i = improve(p, &schedule, &factors, rounds).unwrap();
            let c = check(p, &i.schedule).unwrap();
            let after = npv(&c.period_values, rate);
            if !c.feasible() || after < v - 1e-9 * v.abs().max(1.0) {
                failures.push(format!(
                    "{name}: the search from MineLib's schedule gave {after} from {v}, feasible {}",
                    c.feasible()
                ));
            }
            if certified.is_some_and(|u| after > u + 1.0) || after > published_lp + 1.0 {
                failures.push(format!(
                    "{name}: the search from MineLib's schedule gave {after}, above a bound ({certified:?}, \
                     {published_lp})"
                ));
            }
            if let Some(dir) = &out {
                std::fs::write(
                    Path::new(dir).join(format!("{name}_cpit_fmine_from_known.sol")),
                    cpit::write_solution(&i.schedule),
                )
                .unwrap();
            }
            from_known = Some((after, i.rounds, t.elapsed().as_secs_f64()));
            v
        });
        let greedy_best = npvs.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        let rel = |bound: f64| (bound - best) / bound;
        let list = |v: &[f64]| {
            v.iter()
                .map(|v| format!("{v:.3}"))
                .collect::<Vec<_>>()
                .join(",")
        };
        let moves = moves
            .iter()
            .map(|(r, m)| {
                format!(
                    "{{\"rounds\":{r},\"earlier\":{},\"cones\":{},\"swaps\":{},\"added\":{},\"later\":{},\"dropped\":{}}}",
                    m.earlier, m.cones, m.swaps, m.added, m.later, m.dropped
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        println!(
            "{{\"bead\":\"bd-5lxu\",\"instance\":\"{name}\",\"blocks\":{n},\"periods\":{periods},\"resources\":{},\"rounded\":[{},{},{}],\"tol\":{tol},\"lp_status\":\"{lp_status}\",\"lp_s\":{lp_s:.1},\"certified_ub\":{},\"published_lp\":{published_lp:.0},\"best_known\":{best_known:.0},\"thresholds\":{THRESHOLDS:?},\"per_threshold\":[\"earliest\",\"lp_release\"],\"npv\":[{}],\"mined\":{mined:?},\"greedy_s\":{greedy_s:.2},\"searched\":[{}],\"moves\":[{moves}],\"search_s\":{search_s:.2},\"greedy_best\":{greedy_best:.3},\"best_npv\":{best:.3},\"gap_to_certified\":{},\"gap_to_published_lp\":{:.4e},\"of_best_known\":{:.4},\"best_known_rechecked\":{},\"from_known\":{}}}",
            p.resources(),
            model.rounded_values,
            model.rounded_uses,
            model.rounded_limits,
            certified.map_or("null".to_string(), |u| format!("{u:.3}")),
            list(&npvs),
            list(&improved),
            certified.map_or("null".to_string(), |u| format!("{:.4e}", rel(u))),
            rel(published_lp),
            best / best_known,
            known_npv.map_or("null".to_string(), |v| format!("{v:.3}")),
            from_known.map_or("null".to_string(), |(v, r, s)| format!(
                "{{\"npv\":{v:.3},\"of_best_known\":{:.6},\"rounds\":{r},\"search_s\":{s:.2}}}",
                v / best_known
            )),
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
