//! MineLib PCPSP pairs, whole-block destinations and their search (`fmine_optimize::toposort::pcpsp`; bd-2rne).
//!
//! Input: a selector byte, then the `.pcpsp` text, a NUL, the `.prec` text, a NUL, and choice bytes. The selector is
//! read as in the `cpit` target: value places mod 10 in the low nibble, bit 4 asks for rounding, and the top three
//! bits pick the use places. Block `b` takes choice byte `b` mod their count. Its low nibble mod the destination
//! count is the block's destination, and its high nibble its rank (15 leaves the block out; no choice bytes send every
//! block to destination 0 at rank 0). Whatever parses must hold up:
//! - `with_destinations` takes the choices, `greedy` does not refuse, and its schedule passes `check` and the
//!   feasibility written out here from the model;
//! - `pcpsp::greedy` (the fallback to other destinations) does not refuse, its schedule passes both under the
//!   destinations it returns, and those differ from the choices only for blocks it mined;
//! - `pcpsp::improve` takes that schedule; its result passes both, its value is the sum written out here, and it is
//!   worth no less than `pcpsp::greedy`'s;
//! - with at most 3 blocks, 2 periods and 2 destinations, it is worth no more than the best of every schedule and
//!   destination choice.
#![no_main]

use fmine_optimize::pitflow::minelib::Rounding;
use fmine_optimize::toposort::pcpsp::{self, PcpspModel};
use fmine_optimize::toposort::search::discount_factors;
use fmine_optimize::toposort::{check, cpit, greedy};
use libfuzzer_sys::fuzz_target;

/// Every mined block's required blocks are mined no later, and no resource goes over its limit in any period, with
/// each block's uses at its destination.
fn feasible(m: &PcpspModel, s: &[Option<u32>], dest: &[u32]) -> bool {
    let n = m.blocks();
    let precedence = (0..n).all(|b| {
        s[b].is_none_or(|t| {
            m.requires(b)
                .unwrap_or(&[])
                .iter()
                .all(|&q| s[q as usize].is_some_and(|tq| tq <= t))
        })
    });
    let capacity = (0..m.resources()).all(|r| {
        m.limits(r).unwrap_or(&[]).iter().enumerate().all(|(t, &cap)| {
            let total: i128 = (0..n)
                .filter(|&b| s[b] == Some(t as u32))
                .map(|b| i128::from(m.uses(r, b, dest[b] as usize).expect("a use per block and destination")))
                .sum();
            total <= i128::from(cap)
        })
    });
    precedence && capacity
}

/// `Σ value[b][dest[b]] · factor[t]` over the mined blocks.
fn value(m: &PcpspModel, s: &[Option<u32>], dest: &[u32], factors: &[i64]) -> i128 {
    (0..m.blocks())
        .filter_map(|b| {
            s[b].map(|t| {
                i128::from(m.value(b, dest[b] as usize).expect("a value per block and destination"))
                    * i128::from(factors[t as usize])
            })
        })
        .sum()
}

fuzz_target!(|data: &[u8]| {
    let Some((&selector, rest)) = data.split_first() else {
        return;
    };
    let mut parts = rest.splitn(3, |&b| b == 0);
    let (Some(pcpsp_bytes), Some(prec_bytes)) = (parts.next(), parts.next()) else {
        return;
    };
    let choices = parts.next().unwrap_or(&[]);
    let (Ok(pcpsp_text), Ok(prec_text)) = (std::str::from_utf8(pcpsp_bytes), std::str::from_utf8(prec_bytes)) else {
        return;
    };
    let rounding = if selector & 0x10 == 0 {
        Rounding::Refuse
    } else {
        Rounding::HalfAwayFromZero
    };
    let Ok(m) = pcpsp::parse(
        pcpsp_text,
        prec_text,
        u32::from(selector & 0x0f) % 10,
        rounding,
        u32::from(selector >> 5),
    ) else {
        return;
    };
    let (n, periods, dn) = (m.blocks(), m.periods(), m.destinations());
    let choice = |b: usize| choices.get(b % choices.len().max(1)).copied().unwrap_or(0);
    let dest: Vec<u32> = (0..n).map(|b| u32::from(choice(b) & 0x0f) % dn as u32).collect();
    let rank: Vec<Option<i64>> = (0..n)
        .map(|b| (choice(b) >> 4 != 15).then(|| i64::from(choice(b) >> 4)))
        .collect();
    let p = m.with_destinations(&dest).expect("one destination per block, each in range");
    let schedule = greedy(&p, &rank).expect("greedy does not refuse a parsed problem");
    let c = check(&p, &schedule).expect("one entry per block");
    assert!(c.feasible() && feasible(&m, &schedule, &dest), "{c:?}");
    // A negative rate is the only refusal here, and it is not this target's concern.
    let Ok(factors) = discount_factors(m.discount_rate, cpit::RATE_SCALE, periods) else {
        return;
    };
    let (schedule, start) = pcpsp::greedy(&m, &rank, &dest).expect("pcpsp::greedy does not refuse a parsed model");
    let c = check(&m.with_destinations(&start).expect("destinations in range"), &schedule).expect("one entry per block");
    assert!(c.feasible() && feasible(&m, &schedule, &start), "{c:?}");
    assert!((0..n).all(|b| start[b] == dest[b] || schedule[b].is_some()), "{start:?} {dest:?}");
    let dest = start;
    let i = pcpsp::improve(&m, &schedule, &dest, &factors, 16).expect("improve takes greedy's feasible schedule");
    let after = check(&m.with_destinations(&i.destination).expect("destinations in range"), &i.schedule)
        .expect("one entry per block");
    assert!(after.feasible() && feasible(&m, &i.schedule, &i.destination), "{after:?}");
    assert_eq!(i.before, value(&m, &schedule, &dest, &factors));
    assert_eq!(i.after, value(&m, &i.schedule, &i.destination, &factors));
    assert!(i.after >= i.before, "{} < {}", i.after, i.before);
    if n <= 3 && periods <= 2 && dn <= 2 {
        let per_block = (periods + 1) * dn;
        let mut best = 0_i128;
        for mut code in 0..per_block.pow(n as u32) {
            let (mut s, mut d) = (vec![None; n], vec![0_u32; n]);
            for b in 0..n {
                let pick = code % per_block;
                code /= per_block;
                d[b] = (pick % dn) as u32;
                s[b] = (pick / dn > 0).then(|| (pick / dn - 1) as u32);
            }
            if feasible(&m, &s, &d) {
                best = best.max(value(&m, &s, &d, &factors));
            }
        }
        assert!(i.after <= best, "{} above the optimum {best}", i.after);
    }
});
