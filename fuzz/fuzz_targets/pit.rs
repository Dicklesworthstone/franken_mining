//! MineLib UPIT pairs and candidate pits (`fmine_optimize::pitflow::{minelib, candidate}`; bd-ip99, bd-nq06).
//!
//! Input: a selector byte, then text: the `.upit` file, a NUL, the `.prec` file, a NUL, and a candidate pit. The
//! selector's low nibble picks the decimal places (mod 10) and bit 4 asks for rounding. Whatever parses is solved,
//! and the result must hold up:
//! - `verify` re-derives the solver's value from its certificate;
//! - the solver's own pit, checked as a candidate, is closed and worth exactly that value;
//! - a closed candidate is never worth more than the certified optimum;
//! - with at most 12 blocks, the value equals the best closed set by enumeration.
#![no_main]

use fmine_optimize::pitflow::{self, candidate, minelib};
use libfuzzer_sys::fuzz_target;

/// The most blocks enumerated: 2¹² closed-set checks per input.
const ENUMERATE: usize = 12;

fuzz_target!(|data: &[u8]| {
    let Some((&selector, rest)) = data.split_first() else {
        return;
    };
    let Ok(text) = std::str::from_utf8(rest) else {
        return;
    };
    let mut parts = text.splitn(3, '\0');
    let (upit, prec, pick) = (
        parts.next().unwrap_or(""),
        parts.next().unwrap_or(""),
        parts.next().unwrap_or(""),
    );
    let rounding = if selector & 0x10 == 0 {
        minelib::Rounding::Refuse
    } else {
        minelib::Rounding::HalfAwayFromZero
    };
    let Ok(model) = minelib::parse(upit, prec, u32::from(selector & 0x0f) % 10, rounding) else {
        return;
    };
    let problem = &model.problem;
    let Ok(pit) = pitflow::solve(problem) else {
        return;
    };
    assert_eq!(
        pitflow::verify(problem, pit.in_pit(), pit.arc_flow()).ok(),
        Some(pit.value())
    );
    let own = candidate::check(problem, pit.in_pit()).expect("the solver's pit has one entry per block");
    assert!(own.closed(), "the certified pit is not closed: {own:?}");
    assert_eq!(own.value, pit.value());
    if let Ok(in_set) = candidate::parse_ids(pick, problem.node_count()) {
        let other = candidate::check(problem, &in_set).expect("membership has one entry per block");
        assert!(!other.closed() || other.value <= pit.value(), "{other:?} beats {}", pit.value());
    }
    let n = problem.node_count();
    if n <= ENUMERATE {
        let best = (0u32..1 << n)
            .filter(|set| {
                (0..n).all(|b| {
                    (set >> b) & 1 == 0
                        || problem
                            .requires(b)
                            .unwrap_or(&[])
                            .iter()
                            .all(|&p| (set >> p) & 1 == 1)
                })
            })
            .map(|set| {
                (0..n)
                    .filter(|&b| (set >> b) & 1 == 1)
                    .map(|b| i128::from(problem.weights()[b]))
                    .sum::<i128>()
            })
            .max();
        assert_eq!(best, Some(i128::from(pit.value())));
    }
});
