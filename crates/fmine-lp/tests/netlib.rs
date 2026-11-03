//! bd-j2ay: netlib LPs read by `mps::read`, written back by `mps::write`, and solved by PDLP, against the optima
//! netlib publishes. Every column of these four is open above. PDLP's bound certifies on them through bd-tcp4's
//! implied bounds and, where a column has none, its multiplier shift.
//!
//! Ignored by default: the files are not in the repository. Point `FMINE_NETLIB_DIR` at a directory holding
//! `afiro.mps`, `adlittle.mps`, `e226.mps` and `israel.mps`, uncompressed as HiGHS's `check/instances` carries
//! them (MIT), and run `cargo test --release -p fmine-lp --test netlib -- --ignored --nocapture`.
//!
//! The optima are netlib's (`lp/data/readme`, "Optimal Value", computed with MINOS 5.3), quoted to 11 significant
//! figures, so each comparison allows half a unit in the last figure.

// Test helpers outside `#[test]` functions: a malformed file must fail the test at once.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::Path;
use std::time::Instant;

use fmine_lp::mps;
use fmine_lp::pdlp::{PdlpOptions, certified_bound, solve};

/// netlib's published optimal values.
const PUBLISHED: &[(&str, f64)] = &[
    ("afiro", -4.647_531_428_6e2),
    ("adlittle", 2.254_949_631_6e5),
    ("e226", -1.875_192_906_6e1),
    ("israel", -8.966_448_218_6e5),
];

/// Half a unit in the 11th significant figure of `v`.
fn half_unit(v: f64) -> f64 {
    0.5 * 10f64.powi(v.abs().log10().floor() as i32 - 10)
}

#[test]
#[ignore = "needs netlib's MPS files (FMINE_NETLIB_DIR); release build"]
fn netlib_lps_certify_at_or_below_their_published_optima() {
    let dir =
        std::env::var("FMINE_NETLIB_DIR").expect("FMINE_NETLIB_DIR names the netlib MPS directory");
    for &(name, published) in PUBLISHED {
        let text = std::fs::read_to_string(Path::new(&dir).join(format!("{name}.mps"))).unwrap();
        let m = mps::read(&text).unwrap();
        let p = &m.problem;
        // Written and read back, the problem writes the same text again: nothing was lost on the way.
        let written = mps::write(name, p).unwrap();
        assert_eq!(
            mps::write(name, &mps::read(&written).unwrap().problem).unwrap(),
            written,
            "{name}"
        );

        let t = Instant::now();
        let s = solve(
            p,
            &PdlpOptions {
                tolerance: 1e-9,
                max_iterations: 2_000_000,
                threads: 1,
            },
        )
        .unwrap();
        let seconds = t.elapsed().as_secs_f64();
        let bound = s
            .bound()
            .unwrap_or_else(|| panic!("{name}: no certified bound after {:?}", s.progress()));
        let primal = p.objective(s.x()).unwrap();
        let gap = (published - bound.value()) / published.abs();
        // Which multipliers the bound came from: the final ones certify on their own, or not.
        let final_y = certified_bound(p, s.y()).unwrap().map(|b| b.value());
        println!(
            r#"{{"test":"netlib","instance":"{name}","rows":{},"cols":{},"nnz":{},"maximise":{},"offset":{},"status":"{:?}","iterations":{},"seconds":{seconds:.3},"bound":{},"implied_columns":{},"repaired_rows":{},"final_y_bound":{final_y:?},"dual_objective":{},"relative_dual_residual":{:e},"primal":{primal},"max_violation":{},"published":{published},"gap":{gap:e}}}"#,
            p.rows(),
            p.cols(),
            p.nnz(),
            m.maximise,
            m.offset,
            s.status(),
            s.progress().iterations,
            bound.value(),
            bound.implied_columns(),
            bound.repaired_rows(),
            s.progress().dual_objective,
            s.progress().relative_dual_residual,
            p.max_violation(s.x()).unwrap(),
        );
        assert!(!m.maximise, "{name}: netlib's LPs are minimisations");
        assert!(
            bound.value() <= published + half_unit(published),
            "{name}: certified bound {} above the published optimum {published}",
            bound.value()
        );
        // Observed 2026-11-03: every gap within 3.4e-8. 1e-6 holds them with room, and fails a bound that lost the
        // multiplier shift (ADLITTLE 0.67 and E226 4.6 without it).
        assert!(
            gap <= 1e-6,
            "{name}: certified bound {} is {gap:e} below the published optimum",
            bound.value()
        );
        if name == "e226" {
            // The RHS on E226's objective row gives the constant 7.113. The published optimum leaves it out: the
            // bound on c·x alone is within 1e-6 of it, and c·x with the constant is −11.64.
            assert_eq!(m.offset, 7.113);
        }
    }
}
