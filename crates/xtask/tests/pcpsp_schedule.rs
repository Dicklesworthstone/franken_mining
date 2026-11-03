//! `bd-2rne`: feasible schedules for MineLib's PCPSP instances, and GATE-PUSH-TOPO (plan §16: gap ≤ 3% on ≥ 80% of
//! PCPSP instances, ≤ 5% on all; plan §15.4 names MineLib's best-known values as the oracle).
//!
//! Ignored by default: MineLib's files are not in the repository (their licence is not recorded). Point
//! `FMINE_MINELIB_DIR` at a directory holding `<name>/<name>.pcpsp` and `<name>/<name>.prec`, list instances in
//! `FMINE_MINELIB_INSTANCES` (comma-separated, default `newman1`), and run
//! `cargo test --release -p xtask --test pcpsp_schedule -- --ignored --nocapture`. Optional: `FMINE_PDLP_TOL` (default
//! 1e-4), `FMINE_PDLP_MAX_SECONDS` (default 600), `FMINE_PDLP_THREADS` (default 0, the machine's) and
//! `FMINE_SEARCH_ROUNDS` (default 1,000).
//!
//! 1. `toposort::pcpsp::parse` reads the pair, values at 10⁶ rounded half away from zero, uses rounded up and limits
//!    down at 10⁶. A refused instance is reported and counts against the gate.
//! 2. `toposort::pcpsp::lp::scope` (`bd-ubzs`) gives the LP's blocks: the ultimate pit `P` under each block's most
//!    valuable destination, its certificate checked, or every block when a use is negative (w23's blending rows).
//!    Its module notes show why cutting the LP down to `P` leaves its optimum, and the schedule problem's, unchanged,
//!    so PDLP's certified bound on the smaller LP is a bound for the whole instance. Blocks outside `P` start unmined;
//!    the search may still add them.
//! 3. `toposort::pcpsp::lp::relaxation` formulates the LP over those blocks: "by" variables `w[b][t]` as for CPIT
//!    (`crates/xtask/tests/cpit_schedule.rs`), and `y[b][d][t]`, the share of block `b` sent to destination `d` in
//!    period `t`, with `Σ_d y[b][d][t] = w[b][t] − w[b][t − 1]`. Limits apply to `Σ_{b,d} use[r][b][d] · y[b][d][t]`.
//!    The value is `Σ value[b][d] · y[b][d][t] / (1 + rate)^t`, periods from 0. PDLP solves it here, and its dual
//!    bound is certified.
//! 4. `toposort::pcpsp::lp::Solution` sends each block whole to the destination the LP sends most of it to (its most
//!    valuable one if the LP mines none of it). `starts` ranks blocks by their LP expected period, as for CPIT, and
//!    leaves them out below a final weight of 10⁻⁶, 0.5 or 0.9. Each threshold gives two schedules, and `incumbent`
//!    searches each with `pcpsp::improve`. One is from `pcpsp::greedy`, which sends a block that fits nowhere at its
//!    destination to another one rather than leave it, and every block under it, unmined. The other is from
//!    `pcpsp::greedy_from`, with each block released at its LP median period, the first by which the LP has mined half
//!    of it (bd-tll7). Earliest-first placement uses up w23's blocks below its blend targets early and leaves the ones
//!    above with nothing to blend with.
//! 5. NPV is `Σ_t V_t / (1 + rate)^t` in `f64`, from `check`'s exact per-period sums under the final destinations.
//!
//! Asserted: every schedule passes `check`, `pcpsp::improve` never lowers an NPV, `incumbent`'s pick has the highest
//! NPV, and no NPV exceeds PDLP's certified bound or MineLib's LP bound (each allowing one unit for rounding). The
//! gate's verdict is printed, not asserted: the plan says to report the honest gap.
//!
//! `FMINE_PCPSP_LP_CACHE` keeps each LP solution and bound (`<name>-<tol>.pit.pcpsp-lp`, or `.all.` for an LP over
//! every block; little-endian `f64`s), so later runs skip PDLP. `FMINE_PCPSP_SOLUTION_OUT` names a directory for each instance's best schedule in MineLib's format
//! (`<name>_pcpsp_fmine.sol`), which `crates/xtask/testdata/minelib_pcpsp_oracle.py` checks from the files alone. With
//! `FMINE_MINELIB_SOLS`, MineLib's own PCPSP schedule (`<name>_pcpsp_*.sol`, blocks split between destinations to six
//! places) is valued here in `f64` and reported beside its published value.

// Test helpers outside `#[test]` functions: a malformed MineLib file must fail the test at once.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::Path;
use std::time::Instant;

use fmine_lp::pdlp::{LpProblem, PdlpOptions, solve_observed};
use fmine_optimize::pitflow::minelib::Rounding;
use fmine_optimize::toposort::lp;
use fmine_optimize::toposort::lp::THRESHOLDS;
use fmine_optimize::toposort::pcpsp::lp::{Solution, incumbent, relaxation, scope, starts};
use fmine_optimize::toposort::pcpsp::{self, PcpspModel};
use fmine_optimize::toposort::search::discount_factors;
use fmine_optimize::toposort::{check, cpit};

/// MineLib's Results page (Results.xhtml, mirrored at github.com/borelian/minelib, read 2026-11-02): the PCPSP LP
/// upper bound and the best-known feasible schedule's value, in whole units. w23's best-known value is 0: no feasible
/// schedule is published.
const PUBLISHED: &[(&str, f64, f64)] = &[
    ("newman1", 24_486_549.0, 23_658_230.0),
    ("zuck_small", 905_878_172.0, 872_372_967.0),
    ("kd", 410_891_003.0, 406_871_207.0),
    ("zuck_medium", 750_519_109.0, 675_931_038.0),
    ("marvin", 911_704_665.0, 885_968_070.0),
    ("w23", 387_693_394.0, 0.0),
    ("zuck_large", 57_938_790.0, 57_334_014.0),
    ("sm2", 1_652_394_327.0, 1_650_439_213.0),
    ("mclaughlin_limit", 1_324_829_727.0, 1_321_662_551.0),
    ("mclaughlin", 1_512_971_680.0, 1_510_126_435.0),
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

/// `toposort::pcpsp::lp`'s relaxation, handed to PDLP.
fn to_lp(r: lp::Relaxation) -> LpProblem {
    LpProblem::new(r.cost, &r.matrix, r.row_lo, r.row_hi, r.col_lo, r.col_hi).unwrap()
}

/// What MineLib's own split schedule is worth here, in `f64`, and how far it is from whole and within limits.
struct Known {
    npv: f64,
    /// Largest `|Σ_d y − 1|` over mined blocks.
    worst_whole: f64,
    /// Largest use over limit, relative to the limit.
    worst_over: f64,
    /// Requirements with the required block unmined or mined later.
    unmet: usize,
}

fn value_known(m: &PcpspModel, text: &str, rate: f64) -> Known {
    let n = m.blocks();
    let mut period = vec![None; n];
    let mut share = vec![0.0_f64; n];
    let mut used = vec![vec![0.0_f64; m.periods()]; m.resources()];
    let mut npv = 0.0;
    for line in text.lines() {
        let f: Vec<&str> = line.split_whitespace().collect();
        if f.is_empty() || f[0].starts_with('%') {
            continue;
        }
        assert_eq!(f.len(), 4, "solution line {line:?}");
        let (b, d, t): (usize, usize, usize) = (
            f[0].parse().unwrap(),
            f[1].parse().unwrap(),
            f[2].parse().unwrap(),
        );
        let y: f64 = f[3].parse().unwrap();
        assert!(
            period[b].replace(t).is_none_or(|was| was == t),
            "block {b} in two periods"
        );
        share[b] += y;
        npv += y * m.value(b, d).unwrap() as f64 / SCALE / (1.0 + rate).powi(t as i32);
        for (r, row) in used.iter_mut().enumerate() {
            row[t] += y * m.uses(r, b, d).unwrap() as f64;
        }
    }
    let worst_whole = (0..n)
        .filter(|&b| period[b].is_some())
        .map(|b| (share[b] - 1.0).abs())
        .fold(0.0, f64::max);
    let mut worst_over = 0.0_f64;
    for (r, row) in used.iter().enumerate() {
        for (t, &u) in row.iter().enumerate() {
            let cap = m.limits(r).unwrap()[t] as f64;
            worst_over = worst_over.max((u - cap) / cap.max(1.0));
        }
    }
    let unmet = (0..n)
        .filter_map(|b| period[b].map(|t| (b, t)))
        .map(|(b, t)| {
            m.requires(b)
                .unwrap()
                .iter()
                .filter(|&&q| period[q as usize].is_none_or(|tq| tq > t))
                .count()
        })
        .sum();
    Known {
        npv,
        worst_whole,
        worst_over,
        unmet,
    }
}

#[test]
#[ignore = "needs MineLib's PCPSP files (FMINE_MINELIB_DIR); release build, minutes per instance"]
fn minelib_pcpsp_schedules_pass_the_checker_and_report_gate_push_topo() {
    let dir = std::env::var("FMINE_MINELIB_DIR").expect("FMINE_MINELIB_DIR");
    let names = std::env::var("FMINE_MINELIB_INSTANCES").unwrap_or_else(|_| "newman1".to_string());
    let tol: f64 = env_or("FMINE_PDLP_TOL", 1e-4);
    let max_seconds: f64 = env_or("FMINE_PDLP_MAX_SECONDS", 600.0);
    let threads: usize = env_or("FMINE_PDLP_THREADS", 0);
    let rounds: usize = env_or("FMINE_SEARCH_ROUNDS", 1_000);
    let mut failures = Vec::new();
    // `(instance, gap to best known, best known is 0)`; the gap is `None` for an instance refused or without a
    // published value.
    let mut gate: Vec<(String, Option<f64>, bool)> = Vec::new();
    for name in names.split(',').map(str::trim).filter(|s| !s.is_empty()) {
        let base = Path::new(&dir).join(name);
        let read = |ext: &str| std::fs::read_to_string(base.join(format!("{name}.{ext}"))).unwrap();
        let model = match pcpsp::parse(
            &read("pcpsp"),
            &read("prec"),
            DECIMALS,
            Rounding::HalfAwayFromZero,
            DECIMALS,
        ) {
            Ok(m) => m,
            Err(e) => {
                println!(
                    "{{\"bead\":\"bd-2rne\",\"instance\":\"{name}\",\"refused\":{:?}}}",
                    format!("{} {}", e.code, e.what)
                );
                gate.push((name.to_string(), None, false));
                continue;
            }
        };
        let (n, periods, dn) = (model.blocks(), model.periods(), model.destinations());
        let rate = model.discount_rate as f64 / cpit::RATE_SCALE as f64;
        let t_pit = Instant::now();
        let lp_scope = scope(&model).unwrap();
        let pit_s = t_pit.elapsed().as_secs_f64();
        let (pit, pit_blocks, pit_value) =
            (&lp_scope.blocks, lp_scope.pit_blocks, lp_scope.pit_value);
        let columns = pit.len() * periods * (1 + dn);
        let kind = if lp_scope.whole { "all" } else { "pit" };
        let cache = std::env::var("FMINE_PCPSP_LP_CACHE")
            .ok()
            .map(|d| Path::new(&d).join(format!("{name}-{tol}.{kind}.pcpsp-lp")));
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
                assert_eq!(f.len(), 2 + columns, "{name}: cached LP of the wrong size");
                f
            });
        let (x, certified, lp_s, lp_status) = if let Some(f) = cached {
            let certified = (!f[0].is_nan()).then_some(f[0]);
            (f[2..].to_vec(), certified, f[1], "cached".to_string())
        } else {
            let relaxed = to_lp(relaxation(&model, pit, DECIMALS).unwrap());
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
        // A block outside the pit has no column: it is never mined in the LP.
        let solution = Solution::new(&model, pit, &x).unwrap();
        let (published_lp, best_known) = PUBLISHED
            .iter()
            .find(|(k, _, _)| *k == name)
            .map(|&(_, l, b)| (l, b))
            .unwrap_or((f64::NAN, f64::NAN));
        let factors = discount_factors(model.discount_rate, cpit::RATE_SCALE, periods).unwrap();
        let t1 = Instant::now();
        let starts = starts(&model, &solution).unwrap();
        let greedy_s = t1.elapsed().as_secs_f64();
        let t2 = Instant::now();
        let found = incumbent(&model, &starts, &factors, rounds).unwrap();
        let search_s = t2.elapsed().as_secs_f64();
        let mut npvs = Vec::new();
        let mut improved = Vec::new();
        let mut moves = Vec::new();
        for (start, i) in starts.iter().zip(&found.searched) {
            let at = format!(
                "threshold {}{}",
                start.threshold,
                if start.released { ", LP release" } else { "" }
            );
            let c = check(
                &model.with_destinations(&start.destination).unwrap(),
                &start.schedule,
            )
            .unwrap();
            if !c.feasible() {
                failures.push(format!("{name}: {at}: greedy's schedule fails check"));
            }
            let before = npv(&c.period_values, rate);
            npvs.push(before);
            let c = check(
                &model.with_destinations(&i.destination).unwrap(),
                &i.schedule,
            )
            .unwrap();
            if !c.feasible() {
                failures.push(format!("{name}: {at}: the searched schedule fails check"));
            }
            let after = npv(&c.period_values, rate);
            if after < before - 1e-9 * before.abs().max(1.0) {
                failures.push(format!(
                    "{name}: {at}: the search lowered NPV from {before} to {after}"
                ));
            }
            improved.push(after);
            moves.push((i.cycles, i.moves, i.destination_moves));
        }
        // The API picks by its integer discounted value; it must be the highest NPV here too, up to `f64` noise.
        let best = improved[found.best];
        let top = improved.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        if best < top - 1e-9 * top.abs().max(1.0) {
            failures.push(format!(
                "{name}: the API's best schedule is worth {best}, below the {top} of another"
            ));
        }
        if let (Ok(dir), Some(chosen)) = (std::env::var("FMINE_PCPSP_SOLUTION_OUT"), found.chosen())
        {
            let text: String = chosen
                .schedule
                .iter()
                .zip(&chosen.destination)
                .enumerate()
                .filter_map(|(b, (t, d))| t.map(|t| format!("{b} {d} {t} 1.000000\n")))
                .collect();
            std::fs::write(
                Path::new(&dir).join(format!("{name}_pcpsp_fmine.sol")),
                text,
            )
            .unwrap();
        }
        let known = std::env::var("FMINE_MINELIB_SOLS").ok().and_then(|d| {
            std::fs::read_dir(d).unwrap().find_map(|e| {
                let path = e.unwrap().path();
                let file = path.file_name()?.to_str()?.to_string();
                (file.starts_with(&format!("{name}_pcpsp_")) && file.ends_with(".sol"))
                    .then(|| value_known(&model, &std::fs::read_to_string(&path).unwrap(), rate))
            })
        });
        let rel = |bound: f64| (bound - best) / bound;
        let list = |v: &[f64]| {
            v.iter()
                .map(|v| format!("{v:.3}"))
                .collect::<Vec<_>>()
                .join(",")
        };
        let moves = moves
            .iter()
            .map(|(cycles, m, d)| {
                format!(
                    "{{\"cycles\":{cycles},\"earlier\":{},\"cones\":{},\"swaps\":{},\"added\":{},\"later\":{},\"dropped\":{},\"switches\":{},\"exchanges\":{},\"priced\":{},\"unmined\":{}}}",
                    m.earlier, m.cones, m.swaps, m.added, m.later, m.dropped, d.switches, d.exchanges, d.priced, d.unmined
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        let of_best_known = (best_known > 0.0).then(|| best / best_known);
        // A best-known value of 0 (no schedule published) is met by any schedule worth at least that; the gate line
        // names such instances.
        let gap = match of_best_known {
            Some(r) => Some(1.0 - r),
            None => (best_known == 0.0 && best >= 0.0).then_some(0.0),
        };
        gate.push((name.to_string(), gap, best_known == 0.0));
        println!(
            "{{\"bead\":\"bd-2rne\",\"instance\":\"{name}\",\"blocks\":{n},\"periods\":{periods},\"destinations\":{dn},\"resources\":{},\"rounded\":[{},{},{}],\"pit_blocks\":{pit_blocks},\"lp_blocks\":{},\"pit_value\":{:.3},\"pit_s\":{pit_s:.2},\"tol\":{tol},\"lp_status\":\"{lp_status}\",\"lp_s\":{lp_s:.1},\"certified_ub\":{},\"published_lp\":{published_lp:.0},\"best_known\":{best_known:.0},\"thresholds\":{THRESHOLDS:?},\"per_threshold\":[\"earliest\",\"lp_release\"],\"npv\":[{}],\"greedy_s\":{greedy_s:.2},\"searched\":[{}],\"moves\":[{moves}],\"search_s\":{search_s:.2},\"best_npv\":{best:.3},\"gap_to_certified\":{},\"gap_to_published_lp\":{:.4e},\"of_best_known\":{},\"known\":{}}}",
            model.resources(),
            model.rounded_values,
            model.rounded_uses,
            model.rounded_limits,
            pit.len(),
            pit_value as f64 / SCALE,
            certified.map_or("null".to_string(), |u| format!("{u:.3}")),
            list(&npvs),
            list(&improved),
            certified.map_or("null".to_string(), |u| format!("{:.4e}", rel(u))),
            rel(published_lp),
            of_best_known.map_or("null".to_string(), |v| format!("{v:.4}")),
            known.map_or("null".to_string(), |k| format!(
                "{{\"npv\":{:.3},\"worst_whole\":{:.1e},\"worst_over\":{:.1e},\"unmet\":{}}}",
                k.npv, k.worst_whole, k.worst_over, k.unmet
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
    // GATE-PUSH-TOPO over the instances run: within 3% of best known on at least 80%, within 5% on all. An instance
    // refused or without a published value has no gap, and so is neither. One whose best known is 0 has gap 0 when
    // its schedule is worth at least 0, and is listed apart: its gap to the certified bound is the one that informs.
    let within = |limit: f64| {
        gate.iter()
            .filter(|(_, g, _)| g.is_some_and(|g| g <= limit))
            .count()
    };
    let worst = gate
        .iter()
        .map(|(_, g, _)| g.unwrap_or(f64::INFINITY))
        .fold(f64::NEG_INFINITY, f64::max);
    let (three, five, total) = (within(0.03), within(0.05), gate.len());
    let no_gap: Vec<&str> = gate
        .iter()
        .filter(|(_, g, _)| g.is_none())
        .map(|(k, _, _)| k.as_str())
        .collect();
    let zero: Vec<&str> = gate
        .iter()
        .filter(|(_, _, zero)| *zero)
        .map(|(k, _, _)| k.as_str())
        .collect();
    println!(
        "{{\"bead\":\"bd-2rne\",\"gate\":\"GATE-PUSH-TOPO\",\"instances\":{total},\"within_3pct\":{three},\"within_5pct\":{five},\"worst_gap\":{},\"passes\":{},\"no_gap\":{:?},\"best_known_zero\":{:?}}}",
        if worst.is_finite() {
            format!("{worst:.4}")
        } else {
            "null".to_string()
        },
        total > 0 && three * 5 >= total * 4 && five == total,
        no_gap,
        zero,
    );
    assert!(failures.is_empty(), "{failures:#?}");
}
