//! bd-j2ay: netlib LPs read by `mps::read`, written back by `mps::write`, and solved by PDLP, against the optima
//! netlib publishes. Most columns are open above, so PDLP's bound certifies through bd-tcp4's implied bounds and,
//! where a column has none, its multiplier shift.
//!
//! Ignored by default: the files are not in the repository. Point `FMINE_NETLIB_DIR` at a directory holding the
//! `<name>.mps` files, uncompressed as HiGHS's `check/instances` carries them (MIT), and run
//! `cargo test --release -p fmine-lp --test netlib -- --ignored --nocapture`. `FMINE_NETLIB_INSTANCES` picks some,
//! comma-separated; the default is every one in [`PUBLISHED`], 35 small LPs chosen to include RANGES (BOEING1,
//! BOEING2, FORPLAN, SEBA) and BOUNDS sections.
//!
//! Every instance must read and write back to the same text, and any certified bound must lie at or below
//! netlib's optimum. Those not in [`KNOWN_GAPS`] must also certify within [`PINNED_GAP`] of it.
//!
//! The optima are netlib's (`lp/data/readme`, "Optimal Value", computed with MINOS 5.3), quoted to 11 significant
//! figures, so each comparison allows half a unit in the last figure.

// Test helpers outside `#[test]` functions: a malformed file must fail the test at once.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::Path;
use std::time::Instant;

use fmine_lp::mps;
use fmine_lp::pdlp::{PdlpOptions, certified_bound, solve};

/// netlib's published optimal values, by file name.
const PUBLISHED: &[(&str, f64)] = &[
    ("adlittle", 2.254_949_631_6e5),
    ("afiro", -4.647_531_428_6e2),
    ("bandm", -1.586_280_184_5e2),
    ("beaconfd", 3.359_248_580_7e4),
    ("blend", -3.081_214_984_6e1),
    ("boeing1", -3.352_135_675_1e2),
    ("boeing2", -3.150_187_280_2e2),
    ("bore3d", 1.373_080_394_2e3),
    ("brandy", 1.518_509_896_5e3),
    ("capri", 2.690_012_913_8e3),
    ("degen2", -1.435_178_000_0e3),
    ("e226", -1.875_192_906_6e1),
    ("etamacro", -7.557_152_177_4e2),
    ("finnis", 1.727_909_654_7e5),
    ("forplan", -6.642_187_395_3e2),
    ("grow7", -4.778_781_181_5e7),
    ("israel", -8.966_448_218_6e5),
    ("kb2", -1.749_900_129_9e3),
    ("lotfi", -2.526_470_606_2e1),
    ("recipe", -2.666_160_000_0e2),
    ("sc105", -5.220_206_121_2e1),
    ("sc50a", -6.457_507_705_9e1),
    ("sc50b", -7.000_000_000_0e1),
    ("scagr7", -2.331_389_254_8e6),
    ("scorpion", 1.878_124_822_7e3),
    ("scsd1", 8.666_666_674_3e0),
    ("sctap1", 1.412_250_000_0e3),
    ("seba", 1.571_160_000_0e4),
    ("share1b", -7.658_931_857_9e4),
    ("share2b", -4.157_322_407_4e2),
    ("stair", -2.512_669_511_9e2),
    ("standata", 1.257_699_500_0e3),
    ("stocfor1", -4.113_197_621_9e4),
    ("tuff", 2.921_477_650_9e-1),
    ("vtp.base", 1.298_314_624_6e5),
];

/// Instances with no certified bound, or a loose one, on 2026-11-03 (ε = 1e-9, one million steps; debug and release
/// builds gave the same bounds and step counts): FORPLAN stopped at the step limit 1.9% below its optimum, and the
/// others met PDLP's stopping rule but certified nothing. Each has columns open above that no single row bounds (a
/// re-run of the propagation found SCSD1 all 760, SCORPION 303 of 358, FINNIS 298 of 533), and the multiplier
/// shift did not give all of them a reduced cost of the right sign. A bound they do give must still lie at or below
/// the optimum.
const KNOWN_GAPS: &[&str] = &[
    "bore3d", "brandy", "capri", "finnis", "forplan", "lotfi", "scorpion", "scsd1", "share1b",
    "share2b", "stair",
];

/// Every other instance must certify within this of its optimum. Observed 2026-11-03: the 24 lay within 2.5e-7
/// (ETAMACRO 2.2e-7, SCAGR7 2.4e-7); ADLITTLE and E226 were 0.67 and 4.6 off before the multiplier shift.
const PINNED_GAP: f64 = 1e-6;

/// Half a unit in the 11th significant figure of `v`.
fn half_unit(v: f64) -> f64 {
    0.5 * 10f64.powi(v.abs().log10().floor() as i32 - 10)
}

/// `v` as a JSON value: `null` when absent.
fn json<T: std::fmt::Display>(v: Option<T>) -> String {
    v.map_or_else(|| "null".to_string(), |v| v.to_string())
}

/// One instance's run: `Err` names what failed.
fn run(dir: &Path, name: &str, published: f64) -> Result<(), String> {
    let text = std::fs::read_to_string(dir.join(format!("{name}.mps")))
        .map_err(|e| format!("open: {e}"))?;
    let m = mps::read(&text).map_err(|e| format!("read: {}", e.what))?;
    let p = &m.problem;
    // Written and read back, the problem writes the same text again: nothing was lost on the way.
    let written = mps::write("netlib", p).map_err(|e| format!("write: {}", e.what))?;
    let again = mps::read(&written).map_err(|e| format!("read back: {}", e.what))?;
    if mps::write("netlib", &again.problem).map_err(|e| format!("write again: {}", e.what))?
        != written
    {
        return Err("the written text does not read back to itself".into());
    }
    if m.maximise {
        return Err("netlib's LPs are minimisations".into());
    }

    let t = Instant::now();
    let s = solve(
        p,
        &PdlpOptions {
            tolerance: 1e-9,
            max_iterations: 1_000_000,
            threads: 1,
        },
    )
    .map_err(|e| format!("solve: {}", e.what))?;
    let seconds = t.elapsed().as_secs_f64();
    let bound = s.bound();
    let value = bound.map(|b| b.value());
    let gap = value.map(|b| (published - b) / published.abs());
    // Which multipliers the bound came from: the final ones certify on their own, or not.
    let final_y = certified_bound(p, s.y()).unwrap().map(|b| b.value());
    println!(
        r#"{{"test":"netlib","instance":"{name}","rows":{},"cols":{},"nnz":{},"offset":{},"status":"{:?}","iterations":{},"seconds":{seconds:.3},"bound":{},"implied_columns":{},"repaired_rows":{},"final_y_bound":{},"dual_objective":{},"relative_dual_residual":{:e},"primal":{},"max_violation":{:e},"published":{published},"gap":{}}}"#,
        p.rows(),
        p.cols(),
        p.nnz(),
        m.offset,
        s.status(),
        s.progress().iterations,
        json(value),
        json(bound.map(|b| b.implied_columns())),
        json(bound.map(|b| b.repaired_rows())),
        json(final_y),
        s.progress().dual_objective,
        s.progress().relative_dual_residual,
        p.objective(s.x()).unwrap(),
        p.max_violation(s.x()).unwrap(),
        json(gap),
    );
    let known_gap = KNOWN_GAPS.contains(&name);
    let (Some(value), Some(gap)) = (value, gap) else {
        return if known_gap {
            Ok(())
        } else {
            Err("no certified bound".into())
        };
    };
    if value > published + half_unit(published) {
        return Err(format!(
            "certified bound {value} ABOVE the published optimum {published}"
        ));
    }
    if !known_gap && gap > PINNED_GAP {
        return Err(format!(
            "certified bound {value} is {gap:e} below the published optimum"
        ));
    }
    if name == "e226" && m.offset != 7.113 {
        // The RHS on E226's objective row gives the constant 7.113. The published optimum leaves it out: the
        // bound on c·x alone is within the pinned gap of it, and c·x with the constant is −11.64.
        return Err(format!("objective constant {}, not 7.113", m.offset));
    }
    Ok(())
}

#[test]
#[ignore = "needs netlib's MPS files (FMINE_NETLIB_DIR); release build"]
fn netlib_lps_certify_at_or_below_their_published_optima() {
    let dir =
        std::env::var("FMINE_NETLIB_DIR").expect("FMINE_NETLIB_DIR names the netlib MPS directory");
    let names: Vec<String> = match std::env::var("FMINE_NETLIB_INSTANCES") {
        Ok(list) => list.split(',').map(|s| s.trim().to_string()).collect(),
        Err(_) => PUBLISHED.iter().map(|(n, _)| n.to_string()).collect(),
    };
    let mut failures = Vec::new();
    for name in &names {
        let published = PUBLISHED
            .iter()
            .find(|(n, _)| n == name)
            .unwrap_or_else(|| panic!("{name} has no published optimum here"))
            .1;
        if let Err(why) = run(Path::new(&dir), name, published) {
            println!(r#"{{"test":"netlib","instance":"{name}","failed":{why:?}}}"#);
            failures.push(format!("{name}: {why}"));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {}: {failures:#?}",
        failures.len(),
        names.len()
    );
}
