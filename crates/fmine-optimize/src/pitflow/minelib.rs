//! MineLib's ultimate-pit text format, parsed into a [`ClosureProblem`] (bd-ip99). No I/O: the caller reads the
//! bytes and binds their digests.
//! Determinism: BitwisePortable (`i64` integer arithmetic on decimal tokens only; no floating point, hashing or
//! threads).
//!
//! A `.upit` file has the header lines `NAME:`, `TYPE: UPIT` and `NBLOCKS: n`, then `OBJECTIVE_FUNCTION:` and one
//! `<block> <value>` line per block, up to an optional `EOF`. A `.prec` file has one `<block> <count> <required>...`
//! line per block. Lines starting with `%` are comments. Every block must appear exactly once in each file; nothing
//! is defaulted.
//!
//! Values are decimals scaled to `i64` at a declared number of places. A value with more non-zero places is refused,
//! unless the caller asks for rounding: it is then rounded half away from zero and counted. Each rounding moves a
//! pit's value by at most half a unit in the last kept place, so the rounded problem's optimum lies within half the
//! count of such units of the file's. `Ceiling` and `Floor` round toward +∞ and −∞ instead, a step of under one unit
//! each.
//!
//! The `.prec` reader and the decimal scaling are shared with MineLib's CPIT reader, `toposort::cpit`.

use fmine_evidence::{Refusal, RefusalCode};

use super::ClosureProblem;

/// The most decimal places a value may be scaled by.
pub const MAX_DECIMALS: u32 = 9;

/// What to do with a value that has more non-zero places than the scale keeps.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rounding {
    Refuse,
    HalfAwayFromZero,
    /// Toward +∞: a resource use is never read smaller than the file's.
    Ceiling,
    /// Toward −∞: a capacity is never read larger than the file's.
    Floor,
}

/// A parsed UPIT pair.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpitModel {
    /// The `NAME:` header.
    pub name: String,
    /// Values at 10^`decimals` and the precedence arcs.
    pub problem: ClosureProblem,
    /// How many values were rounded.
    pub rounded: usize,
}

pub(crate) fn bad(detail: String) -> Refusal {
    Refusal::new(
        RefusalCode::InputMissingOrUnreadable506,
        format!("MineLib: {detail}"),
    )
}

/// The meaningful lines: trimmed, without blanks and `%` comments, with 1-based line numbers.
pub(crate) fn lines(text: &str) -> impl Iterator<Item = (usize, &str)> {
    text.lines()
        .enumerate()
        .map(|(i, l)| (i + 1, l.trim()))
        .filter(|(_, l)| !l.is_empty() && !l.starts_with('%'))
}

/// `"-2236.7886"` at 6 places is `(-2_236_788_600, false)`. The flag is set when non-zero places were rounded off.
///
/// # Errors
/// `FMINE-REF-506` (proposed) for a token that is not a plain decimal (`-`, digits, an optional `.` and digits), more
/// than [`MAX_DECIMALS`] places, a value outside `i64`, or non-zero places past the scale under [`Rounding::Refuse`].
pub fn scaled(token: &str, decimals: u32, rounding: Rounding) -> Result<(i64, bool), Refusal> {
    if decimals > MAX_DECIMALS {
        return Err(bad(format!(
            "{decimals} decimal places; at most {MAX_DECIMALS}"
        )));
    }
    let (neg, body) = match token.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, token),
    };
    let (int, frac) = body.split_once('.').unwrap_or((body, ""));
    if int.is_empty()
        || !int.bytes().all(|b| b.is_ascii_digit())
        || !frac.bytes().all(|b| b.is_ascii_digit())
    {
        return Err(bad(format!("value {token:?} is not a plain decimal")));
    }
    let places = decimals as usize;
    let (kept, dropped) = frac.split_at(frac.len().min(places));
    let overflow = || {
        bad(format!(
            "value {token:?} does not fit i64 at {decimals} places"
        ))
    };
    let mut magnitude: i64 = 0;
    for b in int
        .bytes()
        .chain(kept.bytes())
        .chain(std::iter::repeat_n(b'0', places - kept.len()))
    {
        magnitude = magnitude
            .checked_mul(10)
            .and_then(|m| m.checked_add(i64::from(b - b'0')))
            .ok_or_else(overflow)?;
    }
    let rounded = dropped.bytes().any(|b| b != b'0');
    if rounded {
        // `magnitude` is the value's absolute value truncated, so a step of one moves it away from zero.
        let away = match rounding {
            Rounding::Refuse => {
                return Err(bad(format!(
                    "value {token:?} has non-zero places past {decimals}; raise the places or ask for rounding"
                )));
            }
            Rounding::HalfAwayFromZero => dropped.bytes().next().is_some_and(|b| b >= b'5'),
            Rounding::Ceiling => !neg,
            Rounding::Floor => neg,
        };
        if away {
            magnitude = magnitude.checked_add(1).ok_or_else(overflow)?;
        }
    }
    Ok((if neg { -magnitude } else { magnitude }, rounded))
}

/// One integer token.
pub(crate) fn number(token: &str, what: &str, line: usize) -> Result<usize, Refusal> {
    if token.is_empty() || !token.bytes().all(|b| b.is_ascii_digit()) {
        return Err(bad(format!(
            "line {line}: {what} {token:?} is not a non-negative integer"
        )));
    }
    token
        .parse()
        .map_err(|_| bad(format!("line {line}: {what} {token:?} is too large")))
}

/// The block values of a `.upit` text at 10^`decimals`, with the `NAME:` header and the rounding count.
fn parse_upit(
    text: &str,
    decimals: u32,
    rounding: Rounding,
) -> Result<(String, Vec<i64>, usize), Refusal> {
    let mut it = lines(text);
    let (mut name, mut kind, mut nblocks) = (None, None, None);
    for (line, l) in it.by_ref() {
        let (key, value) = l
            .split_once(':')
            .ok_or_else(|| bad(format!("line {line}: header {l:?} is not `KEY: value`")))?;
        let (key, value) = (key.trim(), value.trim());
        let fresh = match key {
            "NAME" => name.replace(value.to_string()).is_none(),
            "TYPE" => kind.replace(value.to_string()).is_none(),
            "NBLOCKS" => nblocks.replace(number(value, "NBLOCKS", line)?).is_none(),
            "OBJECTIVE_FUNCTION" if value.is_empty() => break,
            other => {
                return Err(bad(format!(
                    "line {line}: header {other:?} is not a UPIT header"
                )));
            }
        };
        if !fresh {
            return Err(bad(format!("line {line}: header {key:?} given twice")));
        }
    }
    let (Some(name), Some(kind), Some(n)) = (name, kind, nblocks) else {
        return Err(bad(
            "NAME, TYPE and NBLOCKS are all required before OBJECTIVE_FUNCTION".into(),
        ));
    };
    if kind != "UPIT" {
        return Err(bad(format!("TYPE is {kind:?}, not UPIT")));
    }
    let mut rows = Vec::new();
    let mut rounded = 0;
    for (line, l) in it {
        if l == "EOF" {
            break;
        }
        let mut tokens = l.split_whitespace();
        let (Some(id), Some(value), None) = (tokens.next(), tokens.next(), tokens.next()) else {
            return Err(bad(format!(
                "line {line}: objective line {l:?} is not `<block> <value>`"
            )));
        };
        let id = number(id, "block", line)?;
        let (w, was_rounded) = scaled(value, decimals, rounding)?;
        rounded += usize::from(was_rounded);
        rows.push((id, w));
    }
    if rows.len() != n {
        return Err(bad(format!(
            "{} objective lines for NBLOCKS {n}",
            rows.len()
        )));
    }
    let mut weights = vec![None; n];
    for (id, w) in rows {
        let slot = weights
            .get_mut(id)
            .ok_or_else(|| bad(format!("block {id} is not below NBLOCKS {n}")))?;
        if slot.replace(w).is_some() {
            return Err(bad(format!("block {id} has two values")));
        }
    }
    // Every id is below n and none repeats, so all n slots are filled.
    let weights = weights.into_iter().flatten().collect();
    Ok((name, weights, rounded))
}

/// One list of required blocks per block, from a `.prec` text over `n` blocks.
pub(crate) fn parse_prec(text: &str, n: usize) -> Result<Vec<Vec<u32>>, Refusal> {
    let mut requires: Vec<Option<Vec<u32>>> = vec![None; n];
    for (line, l) in lines(text) {
        let mut tokens = l.split_whitespace();
        let id = number(tokens.next().unwrap_or(""), "block", line)?;
        let count = number(tokens.next().unwrap_or(""), "count", line)?;
        let mut list = tokens
            .map(|t| {
                let p = number(t, "required block", line)?;
                if p >= n || p == id {
                    return Err(bad(format!(
                        "line {line}: block {id} requires {p}, not another of {n} blocks"
                    )));
                }
                u32::try_from(p)
                    .map_err(|_| bad(format!("line {line}: block {p} does not fit u32")))
            })
            .collect::<Result<Vec<_>, _>>()?;
        if list.len() != count {
            return Err(bad(format!(
                "line {line}: block {id} declares {count} but lists {}",
                list.len()
            )));
        }
        list.sort_unstable();
        if list.windows(2).any(|w| w[0] == w[1]) {
            return Err(bad(format!(
                "line {line}: block {id} lists a required block twice"
            )));
        }
        let slot = requires
            .get_mut(id)
            .ok_or_else(|| bad(format!("line {line}: block {id} is not one of {n} blocks")))?;
        if slot.replace(list).is_some() {
            return Err(bad(format!(
                "line {line}: block {id} has two precedence lines"
            )));
        }
    }
    requires
        .into_iter()
        .enumerate()
        .map(|(b, r)| r.ok_or_else(|| bad(format!("block {b} has no precedence line"))))
        .collect()
}

/// Parses a `.upit` and `.prec` pair into a closure problem.
///
/// # Errors
/// `FMINE-REF-506` (proposed) for any malformed, missing, duplicate or out-of-range header, value or line, and for
/// a value that needs rounding under [`Rounding::Refuse`]; `FMINE-REF-207` (proposed) when the problem fails
/// [`ClosureProblem::new`]'s size or overflow guard.
pub fn parse(
    upit: &str,
    prec: &str,
    decimals: u32,
    rounding: Rounding,
) -> Result<UpitModel, Refusal> {
    let (name, weights, rounded) = parse_upit(upit, decimals, rounding)?;
    let requires = parse_prec(prec, weights.len())?;
    Ok(UpitModel {
        name,
        problem: ClosureProblem::from_requirements(weights, &requires)?,
        rounded,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pitflow::solve;

    const UPIT: &str = "NAME: tiny\nTYPE: UPIT\nNBLOCKS: 4\nOBJECTIVE_FUNCTION:\n% surface then one bench\n0 -1.5\n1 -2\n2 -1\n3 6.25\nEOF\n";
    // Block 3 sits under blocks 0, 1 and 2.
    const PREC: &str = "0 0\n1 0\n2 0\n3 3 0 1 2\n";

    fn refused(r: Result<UpitModel, Refusal>) -> String {
        match r {
            Ok(m) => panic!("expected a refusal, got {m:?}"),
            Err(e) => format!("{} {}", e.code, e.what),
        }
    }

    #[test]
    fn scaled_is_exact_and_rounds_only_when_asked() {
        let h = Rounding::HalfAwayFromZero;
        assert_eq!(
            scaled("-2236.7886", 6, Rounding::Refuse).ok(),
            Some((-2_236_788_600, false))
        );
        assert_eq!(
            scaled("7.", 6, Rounding::Refuse).ok(),
            Some((7_000_000, false))
        );
        assert_eq!(
            scaled("1.2500000", 2, Rounding::Refuse).ok(),
            Some((125, false))
        );
        assert_eq!(
            scaled("402.182999999999", 6, h).ok(),
            Some((402_183_000, true))
        );
        assert_eq!(scaled("-1.2345675", 6, h).ok(), Some((-1_234_568, true)));
        assert_eq!(scaled("-1.2345674", 6, h).ok(), Some((-1_234_567, true)));
        assert_eq!(scaled("0.0000005", 6, h).ok(), Some((1, true)));
        let (up, down) = (Rounding::Ceiling, Rounding::Floor);
        assert_eq!(scaled("1.2345671", 6, up).ok(), Some((1_234_568, true)));
        assert_eq!(scaled("-1.2345679", 6, up).ok(), Some((-1_234_567, true)));
        assert_eq!(scaled("1.2345679", 6, down).ok(), Some((1_234_567, true)));
        assert_eq!(scaled("-1.2345671", 6, down).ok(), Some((-1_234_568, true)));
        assert_eq!(scaled("1.2340000", 3, up).ok(), Some((1_234, false)));
        assert_eq!(
            scaled("9223372036854775807", 0, Rounding::Refuse).ok(),
            Some((i64::MAX, false))
        );
        assert!(scaled("402.182999999999", 6, Rounding::Refuse).is_err());
        assert!(scaled("9223372036854775807.5", 0, h).is_err());
        assert!(scaled("9223372036854775808", 0, Rounding::Refuse).is_err());
        for t in ["1e5", ".5", "--1", "+1", "1.2.3", "", "-", "1,5"] {
            assert!(scaled(t, 6, h).is_err(), "{t:?}");
        }
        assert!(scaled("1", MAX_DECIMALS + 1, h).is_err());
    }

    #[test]
    fn tiny_pair_solves_to_its_hand_optimum() {
        let m = parse(UPIT, PREC, 2, Rounding::Refuse).expect("tiny parses");
        assert_eq!((m.name.as_str(), m.rounded), ("tiny", 0));
        assert_eq!(m.problem.weights(), &[-150, -200, -100, 625]);
        assert_eq!(m.problem.requires(3), Some(&[0, 1, 2][..]));
        // Taking 3 needs 0, 1 and 2: 6.25 − 1.5 − 2 − 1 = 1.75 > 0, so all four blocks are the pit.
        let pit = solve(&m.problem).expect("solves");
        assert_eq!((pit.value(), pit.pit_size()), (175, 4));
    }

    #[test]
    fn every_malformed_pair_refuses() {
        let r = Rounding::HalfAwayFromZero;
        let cases = [
            (
                UPIT.replace("TYPE: UPIT", "TYPE: CPIT"),
                PREC.to_string(),
                "not UPIT",
            ),
            (
                UPIT.replace("NAME: tiny\n", ""),
                PREC.to_string(),
                "all required",
            ),
            (
                UPIT.replace("NBLOCKS: 4", "NBLOCKS: 5"),
                PREC.to_string(),
                "for NBLOCKS 5",
            ),
            (
                UPIT.replace("NBLOCKS: 4", "NBLOCKS: 4\nNPERIODS: 2"),
                PREC.to_string(),
                "not a UPIT header",
            ),
            (
                UPIT.replace("NAME: tiny", "NAME: tiny\nNAME: again"),
                PREC.to_string(),
                "given twice",
            ),
            (
                UPIT.replace("3 6.25", "1 6.25"),
                PREC.to_string(),
                "two values",
            ),
            (
                UPIT.replace("3 6.25", "4 6.25"),
                PREC.to_string(),
                "not below NBLOCKS",
            ),
            (
                UPIT.replace("3 6.25", "3 6.25 7"),
                PREC.to_string(),
                "is not `<block> <value>`",
            ),
            (
                UPIT.replace("3 6.25", "3 6.255"),
                PREC.to_string(),
                "past 2",
            ),
            (
                UPIT.to_string(),
                PREC.replace("3 3 0 1 2", "3 2 0 1 2"),
                "declares 2",
            ),
            (
                UPIT.to_string(),
                PREC.replace("3 3 0 1 2", "3 3 0 1 1"),
                "twice",
            ),
            (
                UPIT.to_string(),
                PREC.replace("3 3 0 1 2", "3 3 0 1 3"),
                "requires 3",
            ),
            (
                UPIT.to_string(),
                PREC.replace("3 3 0 1 2", "3 3 0 1 4"),
                "requires 4",
            ),
            (
                UPIT.to_string(),
                PREC.replace("2 0\n", ""),
                "block 2 has no precedence line",
            ),
            (
                UPIT.to_string(),
                PREC.replace("2 0\n", "2 0\n2 0\n"),
                "two precedence lines",
            ),
            (UPIT.to_string(), PREC.replace("1 0", "1 x"), "count \"x\""),
        ];
        for (upit, prec, why) in cases {
            let rounding = if why == "past 2" { Rounding::Refuse } else { r };
            let e = refused(parse(&upit, &prec, 2, rounding));
            assert!(
                e.starts_with("FMINE-REF-506 ") && e.contains(why),
                "{why}: {e}"
            );
        }
        // With rounding asked for, the same value is kept and counted.
        let m = parse(&UPIT.replace("3 6.25", "3 6.255"), PREC, 2, r).expect("rounds");
        assert_eq!((m.problem.weights()[3], m.rounded), (626, 1));
    }
}
