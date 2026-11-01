//! GATE-PUSH-PIT1 (plan §15.4, `BD-250`): MineLib UPIT instances solved exactly, integer-scaled, with the closure
//! verified by the solver's certificate and the value compared with an independent Dinic maximum flow.
//!
//! Ignored by default: MineLib's files are not in the repository (their licence is not recorded). Point
//! `FMINE_MINELIB_DIR` at a directory holding `<name>/<name>.upit` and `<name>/<name>.prec`, list instances in
//! `FMINE_MINELIB_INSTANCES` (comma-separated, default `newman1`), and run
//! `cargo test --release -p fmine-optimize --test minelib_upit -- --ignored --nocapture`.
//!
//! - Values are scaled by 10⁶. Most files fit exactly (newman1 uses up to five decimals, kd six, p4hd, w23 and
//!   mclaughlin_limit none). zuck_small, zuck_medium and zuck_large print float noise (`4362.94593000001`) and marvin
//!   carries ten to twelve decimals (`-59691.5738320245`). Scales fine enough for those would break the solver's 2⁶²
//!   guard, so such a value is rounded half away from zero to 10⁻⁶ and counted. Each rounding moves any pit's value by
//!   at most ½ × 10⁻⁶, so the optimum of the rounded problem is within `rounded / 2` (in 10⁻⁶ units) of the file's.
//! - pitflow must equal Dinic exactly on the problem as read.
//! - Each value must be within one whole unit, widened by that rounding bound, of MineLib's published UPIT optimum
//!   (`PUBLISHED`). The published figures are whole units, rounding mode unstated, so one unit covers either rounding
//!   or truncation. newman1 is also checked against 26,086,899.03, the optimum Gurobi reports in AMPL's MineLib
//!   notebook.

mod common;

use common::max_closure_value;
use fmine_optimize::pitflow::{ClosureProblem, solve};
use std::path::Path;
use std::time::Instant;

const SCALE_DIGITS: usize = 6;

/// MineLib's "UPIT value: optimal objective value of the problem", in whole units, from its Results page
/// (Results.xhtml, mirrored at github.com/borelian/minelib, read 2026-11-01).
const PUBLISHED: &[(&str, i64)] = &[
    ("newman1", 26_086_899),
    ("zuck_small", 1_422_726_898),
    ("kd", 652_195_037),
    ("zuck_medium", 1_075_124_490),
    ("p4hd", 293_373_256),
    ("marvin", 1_415_655_436),
    ("w23", 510_973_998),
    ("zuck_large", 122_220_280),
    ("sm2", 2_743_603_730),
    ("mclaughlin_limit", 1_495_726_474),
    ("mclaughlin", 1_495_886_962),
];

/// `"-2236.7886"` → `(-2_236_788_600, false)` at 10⁶. Past six decimals the value is rounded half away from zero and
/// the flag is set.
fn scaled(token: &str) -> Result<(i64, bool), String> {
    let (neg, body) = match token.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, token),
    };
    let (int, frac) = body.split_once('.').unwrap_or((body, ""));
    if int.is_empty()
        || !int.bytes().all(|b| b.is_ascii_digit())
        || !frac.bytes().all(|b| b.is_ascii_digit())
    {
        return Err(format!("value {token:?} is not a plain decimal"));
    }
    let (kept, dropped) = frac.split_at(frac.len().min(SCALE_DIGITS));
    let digits = format!("{int}{kept:0<width$}", width = SCALE_DIGITS);
    let mut v: i64 = digits
        .parse()
        .map_err(|e| format!("value {token:?}: {e}"))?;
    if dropped.bytes().next().is_some_and(|b| b >= b'5') {
        v = v
            .checked_add(1)
            .ok_or_else(|| format!("value {token:?} overflows"))?;
    }
    Ok((if neg { -v } else { v }, !dropped.is_empty()))
}

/// Block values at 10⁶, and how many of them were rounded.
fn read_upit(path: &Path) -> Result<(Vec<i64>, usize), String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut lines = text
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('%'));
    let mut n = None;
    for line in lines.by_ref() {
        if let Some(v) = line.strip_prefix("NBLOCKS:") {
            n = Some(
                v.trim()
                    .parse::<usize>()
                    .map_err(|e| format!("NBLOCKS: {e}"))?,
            );
        }
        if line.starts_with("OBJECTIVE_FUNCTION") {
            break;
        }
    }
    let n = n.ok_or("no NBLOCKS line")?;
    let mut weights = vec![None; n];
    let mut rounded = 0;
    for line in lines {
        if line == "EOF" {
            break;
        }
        let mut it = line.split_whitespace();
        let (Some(id), Some(value), None) = (it.next(), it.next(), it.next()) else {
            return Err(format!("objective line {line:?}"));
        };
        let id: usize = id.parse().map_err(|e| format!("block id {id:?}: {e}"))?;
        let slot = weights
            .get_mut(id)
            .ok_or_else(|| format!("block {id} beyond NBLOCKS {n}"))?;
        let (w, was_rounded) = scaled(value)?;
        rounded += usize::from(was_rounded);
        if slot.replace(w).is_some() {
            return Err(format!("block {id} given twice"));
        }
    }
    let weights = weights
        .into_iter()
        .enumerate()
        .map(|(b, w)| w.ok_or_else(|| format!("block {b} has no value")))
        .collect::<Result<_, _>>()?;
    Ok((weights, rounded))
}

fn read_prec(path: &Path, n: usize) -> Result<Vec<Vec<u32>>, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut requires: Vec<Option<Vec<u32>>> = vec![None; n];
    for line in text
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('%'))
    {
        let nums: Vec<usize> = line
            .split_whitespace()
            .map(|t| t.parse::<usize>().map_err(|e| format!("{t:?}: {e}")))
            .collect::<Result<_, _>>()?;
        let (&id, rest) = nums.split_first().ok_or("empty precedence line")?;
        let (&count, preds) = rest
            .split_first()
            .ok_or_else(|| format!("block {id}: no count"))?;
        if preds.len() != count || id >= n || preds.iter().any(|&p| p >= n) {
            return Err(format!("precedence line {line:?} does not fit {n} blocks"));
        }
        if requires[id]
            .replace(preds.iter().map(|&p| p as u32).collect())
            .is_some()
        {
            return Err(format!("block {id} listed twice"));
        }
    }
    Ok(requires
        .into_iter()
        .map(Option::unwrap_or_default)
        .collect())
}

#[test]
fn scaled_parsing_is_exact_to_six_places_and_flags_rounding_past_them() {
    assert_eq!(scaled("-2236.7886"), Ok((-2_236_788_600, false)));
    assert_eq!(scaled("5680.47075"), Ok((5_680_470_750, false)));
    assert_eq!(scaled("0"), Ok((0, false)));
    assert_eq!(scaled("7."), Ok((7_000_000, false)));
    assert_eq!(scaled("4362.94593000001"), Ok((4_362_945_930, true)));
    assert_eq!(scaled("402.182999999999"), Ok((402_183_000, true)));
    assert_eq!(scaled("-59691.5738320245"), Ok((-59_691_573_832, true)));
    assert_eq!(scaled("-1.2345675"), Ok((-1_234_568, true)));
    assert!(scaled("1e5").is_err());
    assert!(scaled(".5").is_err());
    assert!(scaled("--1").is_err());
}

#[test]
#[ignore = "GATE-PUSH-PIT1: needs MineLib files in FMINE_MINELIB_DIR"]
fn minelib_upit_exact_with_verified_closure() {
    let dir =
        std::env::var("FMINE_MINELIB_DIR").expect("FMINE_MINELIB_DIR names the MineLib directory");
    let names = std::env::var("FMINE_MINELIB_INSTANCES").unwrap_or_else(|_| "newman1".to_string());
    let mut solved = 0;
    for name in names.split(',').map(str::trim).filter(|s| !s.is_empty()) {
        let base = Path::new(&dir).join(name);
        let (weights, rounded) = read_upit(&base.join(format!("{name}.upit"))).unwrap();
        let requires = read_prec(&base.join(format!("{name}.prec")), weights.len()).unwrap();
        let t0 = Instant::now();
        let oracle = max_closure_value(&weights, &requires);
        let oracle_s = t0.elapsed().as_secs_f64();
        let p = ClosureProblem::from_requirements(weights, &requires).unwrap();
        let t1 = Instant::now();
        let s = solve(&p).unwrap();
        let solve_s = t1.elapsed().as_secs_f64();
        let st = s.stats();
        println!(
            "{{\"gate\":\"GATE-PUSH-PIT1\",\"instance\":\"{name}\",\"blocks\":{},\"arcs\":{},\"value_e6\":{},\
             \"rounded_values\":{rounded},\"pit_blocks\":{},\"solve_s\":{solve_s:.3},\"oracle_s\":{oracle_s:.3},\
             \"mergers\":{},\"splits\":{},\"relabels\":{},\"max_label\":{}}}",
            p.node_count(),
            p.arc_count(),
            s.value(),
            s.pit_size(),
            st.mergers,
            st.splits,
            st.relabels,
            st.max_label
        );
        assert_eq!(s.value(), oracle, "{name}: pitflow against Dinic");
        let published = PUBLISHED
            .iter()
            .find(|(n, _)| *n == name)
            .map(|&(_, v)| v)
            .expect("instance has a published MineLib optimum");
        let gap = (i128::from(s.value()) - i128::from(published) * 1_000_000).abs();
        let tolerance = 1_000_000 + (rounded as i128 + 1) / 2;
        assert!(
            gap <= tolerance,
            "{name}: {} × 10⁻⁶ is {gap} × 10⁻⁶ from MineLib's published {published}, beyond {tolerance}",
            s.value()
        );
        if name == "newman1" {
            // 26,086,899.03 in cents; the scaled value is in units of 10⁻⁶, rounded half up to cents.
            let cents = (i128::from(s.value()) + 5_000).div_euclid(10_000);
            assert_eq!(
                cents, 2_608_689_903,
                "newman1 against Gurobi's reported optimum"
            );
        }
        solved += 1;
    }
    assert!(solved > 0, "no instance named");
}
