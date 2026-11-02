//! MineLib CPIT pairs and TopoSort schedules (`fmine_optimize::toposort::{cpit, greedy, check}`; bd-kcpj).
//!
//! Input: a selector byte, then the `.cpit` text, a NUL, the `.prec` text, a NUL, and rank bytes. The selector's low
//! nibble picks the value places (mod 10), bit 4 asks for rounding, and its top three bits pick the use places.
//! Whatever parses is scheduled with one rank per block taken from the rank bytes in turn (255 leaves a block out;
//! no rank bytes ranks every block 0), and the result must hold up:
//! - `greedy` does not refuse a parsed problem, leaves unranked blocks unmined, and its schedule passes `check`;
//! - `check` agrees with feasibility written out here from the definition, on that schedule and, with at most 4
//!   blocks and 2 periods, on every schedule.
#![no_main]

use fmine_optimize::pitflow::minelib::Rounding;
use fmine_optimize::toposort::{ScheduleProblem, check, cpit, greedy};
use libfuzzer_sys::fuzz_target;

/// Every mined block's required blocks are mined no later, and no resource goes over its limit in any period.
fn feasible(p: &ScheduleProblem, s: &[Option<u32>]) -> bool {
    let n = p.blocks().node_count();
    let precedence = (0..n).all(|b| {
        s[b].is_none_or(|t| {
            p.blocks()
                .requires(b)
                .unwrap_or(&[])
                .iter()
                .all(|&q| s[q as usize].is_some_and(|tq| tq <= t))
        })
    });
    let capacity = (0..p.resources()).all(|r| {
        let (uses, caps) = (p.usage(r).unwrap_or(&[]), p.limits(r).unwrap_or(&[]));
        caps.iter().enumerate().all(|(t, &cap)| {
            let total: i128 = (0..n)
                .filter(|&b| s[b] == Some(t as u32))
                .map(|b| i128::from(uses[b]))
                .sum();
            total <= i128::from(cap)
        })
    });
    precedence && capacity
}

fuzz_target!(|data: &[u8]| {
    let Some((&selector, rest)) = data.split_first() else {
        return;
    };
    let mut parts = rest.splitn(3, |&b| b == 0);
    let (Some(cpit_bytes), Some(prec_bytes)) = (parts.next(), parts.next()) else {
        return;
    };
    let ranks = parts.next().unwrap_or(&[]);
    let (Ok(cpit_text), Ok(prec_text)) = (std::str::from_utf8(cpit_bytes), std::str::from_utf8(prec_bytes)) else {
        return;
    };
    let rounding = if selector & 0x10 == 0 {
        Rounding::Refuse
    } else {
        Rounding::HalfAwayFromZero
    };
    let Ok(model) = cpit::parse(
        cpit_text,
        prec_text,
        u32::from(selector & 0x0f) % 10,
        rounding,
        u32::from(selector >> 5),
    ) else {
        return;
    };
    let p = &model.problem;
    let n = p.blocks().node_count();
    let rank: Vec<Option<i64>> = (0..n)
        .map(|b| match ranks.get(b % ranks.len().max(1)) {
            None => Some(0),
            Some(255) => None,
            Some(&r) => Some(i64::from(r)),
        })
        .collect();
    let schedule = greedy(p, &rank).expect("greedy does not refuse a parsed problem");
    let c = check(p, &schedule).expect("one entry per block");
    assert!(c.feasible() && feasible(p, &schedule), "{c:?}");
    for (b, (r, t)) in rank.iter().zip(&schedule).enumerate() {
        assert!(r.is_some() || t.is_none(), "unranked block {b} is mined");
    }
    if n <= 4 && p.periods() <= 2 {
        let choices = p.periods() + 1;
        for mut code in 0..choices.pow(n as u32) {
            let s: Vec<Option<u32>> = (0..n)
                .map(|_| {
                    let pick = code % choices;
                    code /= choices;
                    (pick > 0).then(|| (pick - 1) as u32)
                })
                .collect();
            let verdict = check(p, &s).expect("one entry per block").feasible();
            assert_eq!(verdict, feasible(p, &s), "{s:?}");
        }
    }
});
