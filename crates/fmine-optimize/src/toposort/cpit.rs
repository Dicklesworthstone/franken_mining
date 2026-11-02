//! MineLib's constrained-pit (CPIT) text format, parsed into a [`ScheduleProblem`] (`bd-kcpj`). No I/O: the caller
//! reads the bytes and binds their digests.
//! Determinism: BitwisePortable (`i64` integer arithmetic on decimal tokens only; no floating point, hashing or
//! threads).
//!
//! A `.cpit` file has the header lines `NAME:`, `TYPE: CPIT`, `NBLOCKS:`, `NPERIODS:`, `NRESOURCE_SIDE_CONSTRAINTS:`
//! and `DISCOUNT_RATE:`, then three sections, each running to the next one or to an optional `EOF`:
//! - `RESOURCE_CONSTRAINT_LIMITS:`, one `<resource> <period> L <limit>` line per resource and period;
//! - `OBJECTIVE_FUNCTION:`, one `<block> <value>` line per block;
//! - `RESOURCE_CONSTRAINT_COEFFICIENTS:`, `<block> <resource> <use>` lines, at most one per pair; a pair not listed
//!   uses nothing.
//!
//! The precedences come from the instance's `.prec` file, read as for UPIT. Lines starting with `%` are comments.
//!
//! - **Limits.** Only `L` (upper) limits are read. MineLib's format also has `G` and `I` (lower and two-sided). None
//!   of its CPIT instances uses them, and [`ScheduleProblem`] has no lower limits, so they are refused.
//! - **Scaling.** Values are scaled as UPIT values are, at `value_decimals` with the caller's rounding. Uses and limits
//!   are scaled at `use_decimals`. A use with more places is rounded up and a limit down, and both are counted, so a
//!   schedule within the scaled limits is within the file's.
//! - **Discount.** The rate is kept exactly, at 10⁹. Discounting is the caller's.
//! - **Size.** At most [`MAX_RESOURCES`] resources, since uses are stored for every block and resource, and at most
//!   [`MAX_PERIODS`] periods.

use fmine_evidence::Refusal;

use super::{MAX_PERIODS, ScheduleProblem};
use crate::pitflow::ClosureProblem;
use crate::pitflow::minelib::{MAX_DECIMALS, Rounding, bad, lines, number, parse_prec, scaled};

/// The most resources a CPIT file may declare.
pub const MAX_RESOURCES: usize = 64;

/// The denominator of [`CpitModel::discount_rate`].
pub const RATE_SCALE: i64 = 1_000_000_000;

/// A parsed CPIT pair.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CpitModel {
    /// The `NAME:` header.
    pub name: String,
    /// Values at 10^`value_decimals`, uses and limits at 10^`use_decimals`, and the precedence arcs.
    pub problem: ScheduleProblem,
    /// The discount rate per period, times [`RATE_SCALE`].
    pub discount_rate: i64,
    /// How many values were rounded.
    pub rounded_values: usize,
    /// How many uses were rounded up.
    pub rounded_uses: usize,
    /// How many limits were rounded down.
    pub rounded_limits: usize,
}

const SECTIONS: [&str; 3] = [
    "RESOURCE_CONSTRAINT_LIMITS",
    "OBJECTIVE_FUNCTION",
    "RESOURCE_CONSTRAINT_COEFFICIENTS",
];

/// The header values, all required.
struct Header {
    name: String,
    blocks: usize,
    periods: usize,
    resources: usize,
    rate: i64,
}

/// A numbered line: `(line number, trimmed text)`.
type Line<'a> = (usize, &'a str);

/// The header, and the first section's line, which ends it.
fn header<'a>(
    it: &mut impl Iterator<Item = Line<'a>>,
) -> Result<(Header, Option<Line<'a>>), Refusal> {
    let (mut name, mut kind, mut blocks, mut periods, mut resources, mut rate) =
        (None, None, None, None, None, None);
    let mut first_section = None;
    for (line, l) in it.by_ref() {
        let (key, value) = l
            .split_once(':')
            .ok_or_else(|| bad(format!("line {line}: header {l:?} is not `KEY: value`")))?;
        let (key, value) = (key.trim(), value.trim());
        if value.is_empty() {
            first_section = Some((line, key));
            break;
        }
        let fresh = match key {
            "NAME" => name.replace(value.to_string()).is_none(),
            "TYPE" => kind.replace(value.to_string()).is_none(),
            "NBLOCKS" => blocks.replace(number(value, "NBLOCKS", line)?).is_none(),
            "NPERIODS" => periods.replace(number(value, "NPERIODS", line)?).is_none(),
            "NRESOURCE_SIDE_CONSTRAINTS" => resources
                .replace(number(value, "NRESOURCE_SIDE_CONSTRAINTS", line)?)
                .is_none(),
            "DISCOUNT_RATE" => rate
                .replace(scaled(value, MAX_DECIMALS, Rounding::Refuse)?.0)
                .is_none(),
            other => {
                return Err(bad(format!(
                    "line {line}: header {other:?} is not a CPIT header"
                )));
            }
        };
        if !fresh {
            return Err(bad(format!("line {line}: header {key:?} given twice")));
        }
    }
    let (Some(name), Some(kind), Some(blocks), Some(periods), Some(resources), Some(rate)) =
        (name, kind, blocks, periods, resources, rate)
    else {
        return Err(bad("NAME, TYPE, NBLOCKS, NPERIODS, NRESOURCE_SIDE_CONSTRAINTS and DISCOUNT_RATE are all required before the first section".into()));
    };
    if kind != "CPIT" {
        return Err(bad(format!("TYPE is {kind:?}, not CPIT")));
    }
    if periods == 0 || periods > MAX_PERIODS {
        return Err(bad(format!(
            "NPERIODS is {periods}; at least 1 and at most {MAX_PERIODS}"
        )));
    }
    if resources > MAX_RESOURCES {
        return Err(bad(format!(
            "{resources} resources; at most {MAX_RESOURCES}"
        )));
    }
    if rate < 0 {
        return Err(bad(format!("DISCOUNT_RATE {rate} is negative")));
    }
    Ok((
        Header {
            name,
            blocks,
            periods,
            resources,
            rate,
        },
        first_section,
    ))
}

/// Parses a `.cpit` and `.prec` pair into a schedule problem.
///
/// # Errors
/// `FMINE-REF-506` (proposed) for any malformed, missing, duplicate or out-of-range header, section, limit, value,
/// use or precedence line; a limit kind other than `L`; a negative use or limit; more than [`MAX_RESOURCES`]
/// resources; and a value that needs rounding under [`Rounding::Refuse`]. `FMINE-REF-207` (proposed) when the
/// problem fails [`ClosureProblem::new`]'s size or overflow guard or [`ScheduleProblem::new`].
pub fn parse(
    cpit: &str,
    prec: &str,
    value_decimals: u32,
    value_rounding: Rounding,
    use_decimals: u32,
) -> Result<CpitModel, Refusal> {
    let mut it = lines(cpit);
    let (head, first_section) = header(&mut it)?;
    let (n, periods, resources) = (head.blocks, head.periods, head.resources);
    let mut limit_rows = Vec::new();
    let mut value_rows = Vec::new();
    let mut use_rows = Vec::new();
    let mut seen = [false; 3];
    let mut section: Option<usize> = None;
    let mut open = |line: usize, key: &str| -> Result<usize, Refusal> {
        let s = SECTIONS
            .iter()
            .position(|&k| k == key)
            .ok_or_else(|| bad(format!("line {line}: {key:?} is not a CPIT section")))?;
        if std::mem::replace(&mut seen[s], true) {
            return Err(bad(format!("line {line}: section {key} given twice")));
        }
        Ok(s)
    };
    if let Some((line, key)) = first_section {
        section = Some(open(line, key)?);
    }
    let (mut rounded_values, mut rounded_uses, mut rounded_limits) = (0, 0, 0);
    for (line, l) in it {
        if l == "EOF" {
            break;
        }
        if let Some((key, rest)) = l.split_once(':') {
            if !rest.trim().is_empty() {
                return Err(bad(format!(
                    "line {line}: {l:?} is neither a section nor a data line"
                )));
            }
            section = Some(open(line, key.trim())?);
            continue;
        }
        let tokens: Vec<&str> = l.split_whitespace().collect();
        match (section, tokens.as_slice()) {
            (Some(0), [r, t, kind, limit]) => {
                if *kind != "L" {
                    return Err(bad(format!(
                        "line {line}: limit kind {kind:?}; only L (an upper limit) is read"
                    )));
                }
                let (cap, was) = scaled(limit, use_decimals, Rounding::Floor)?;
                rounded_limits += usize::from(was);
                limit_rows.push((
                    line,
                    number(r, "resource", line)?,
                    number(t, "period", line)?,
                    cap,
                ));
            }
            (Some(1), [b, value]) => {
                let (w, was) = scaled(value, value_decimals, value_rounding)?;
                rounded_values += usize::from(was);
                value_rows.push((line, number(b, "block", line)?, w));
            }
            (Some(2), [b, r, q]) => {
                let (q, was) = scaled(q, use_decimals, Rounding::Ceiling)?;
                rounded_uses += usize::from(was);
                use_rows.push((
                    line,
                    number(b, "block", line)?,
                    number(r, "resource", line)?,
                    q,
                ));
            }
            (None, _) => {
                return Err(bad(format!(
                    "line {line}: data line {l:?} before any section"
                )));
            }
            (Some(s), _) => {
                return Err(bad(format!(
                    "line {line}: {l:?} is not a {} line",
                    SECTIONS[s]
                )));
            }
        }
    }
    if let Some(s) = seen.iter().position(|&x| !x) {
        return Err(bad(format!("section {} is missing", SECTIONS[s])));
    }
    // Sized only from lines that are present, so a large header alone allocates nothing.
    if Some(limit_rows.len()) != resources.checked_mul(periods) {
        return Err(bad(format!(
            "{} limit lines for {resources} resources over {periods} periods",
            limit_rows.len()
        )));
    }
    // Not `vec![vec![None; periods]; resources]`, which builds one row even when there are no resources.
    let mut limits: Vec<Vec<Option<i64>>> = (0..resources).map(|_| vec![None; periods]).collect();
    for (line, r, t, cap) in limit_rows {
        let slot = limits
            .get_mut(r)
            .and_then(|row| row.get_mut(t))
            .ok_or_else(|| {
                bad(format!(
                    "line {line}: resource {r}, period {t} is outside {resources} resources and {periods} periods"
                ))
            })?;
        if cap < 0 {
            return Err(bad(format!("line {line}: limit {cap} is negative")));
        }
        if slot.replace(cap).is_some() {
            return Err(bad(format!(
                "line {line}: resource {r}, period {t} has two limits"
            )));
        }
    }
    // As many lines as slots and none repeated, so every slot is filled.
    let limits: Vec<Vec<i64>> = limits
        .into_iter()
        .map(|row| row.into_iter().flatten().collect())
        .collect();
    if value_rows.len() != n {
        return Err(bad(format!(
            "{} objective lines for NBLOCKS {n}",
            value_rows.len()
        )));
    }
    let mut values = vec![None; n];
    for (line, b, w) in value_rows {
        let slot = values
            .get_mut(b)
            .ok_or_else(|| bad(format!("line {line}: block {b} is not below NBLOCKS {n}")))?;
        if slot.replace(w).is_some() {
            return Err(bad(format!("line {line}: block {b} has two values")));
        }
    }
    let values: Vec<i64> = values.into_iter().flatten().collect();
    let mut usage = vec![vec![0_i64; n]; resources];
    let mut listed = vec![vec![false; n]; resources];
    for (line, b, r, q) in use_rows {
        let (Some(slot), Some(was)) = (
            usage.get_mut(r).and_then(|row| row.get_mut(b)),
            listed.get_mut(r).and_then(|row| row.get_mut(b)),
        ) else {
            return Err(bad(format!(
                "line {line}: block {b}, resource {r} is outside {n} blocks and {resources} resources"
            )));
        };
        if q < 0 {
            return Err(bad(format!("line {line}: use {q} is negative")));
        }
        if std::mem::replace(was, true) {
            return Err(bad(format!(
                "line {line}: block {b}, resource {r} has two uses"
            )));
        }
        *slot = q;
    }
    let requires = parse_prec(prec, n)?;
    let blocks = ClosureProblem::from_requirements(values, &requires)?;
    Ok(CpitModel {
        name: head.name,
        problem: ScheduleProblem::new(blocks, periods, usage, limits)?,
        discount_rate: head.rate,
        rounded_values,
        rounded_uses,
        rounded_limits,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::toposort::{check, greedy};

    // Two blocks on the surface and one under both; one resource of 2 per period over two periods.
    const CPIT: &str = "NAME: tiny\nTYPE: CPIT\nNBLOCKS: 3\nNPERIODS: 2\nNRESOURCE_SIDE_CONSTRAINTS: 1\nDISCOUNT_RATE: 0.08\nRESOURCE_CONSTRAINT_LIMITS:\n0 0 L 2\n0 1 L 2\nOBJECTIVE_FUNCTION:\n% waste, waste, ore\n0 -1.5\n1 -2\n2 9.25\nRESOURCE_CONSTRAINT_COEFFICIENTS:\n0 0 1\n1 0 1\n2 0 1.5\nEOF\n";
    const PREC: &str = "0 0\n1 0\n2 2 0 1\n";

    fn refused(r: Result<CpitModel, Refusal>) -> String {
        match r {
            Ok(m) => panic!("expected a refusal, got {m:?}"),
            Err(e) => format!("{} {}", e.code, e.what),
        }
    }

    #[test]
    fn tiny_pair_reads_exactly_and_schedules() {
        let m = parse(CPIT, PREC, 2, Rounding::Refuse, 1).unwrap();
        assert_eq!(m.name, "tiny");
        assert_eq!(m.discount_rate, 80_000_000);
        let p = &m.problem;
        assert_eq!(p.blocks().weights(), &[-150, -200, 925]);
        assert_eq!((p.periods(), p.resources()), (2, 1));
        assert_eq!(p.usage(0), Some(&[10, 10, 15][..]));
        assert_eq!(p.limits(0), Some(&[20, 20][..]));
        assert_eq!(
            (m.rounded_values, m.rounded_uses, m.rounded_limits),
            (0, 0, 0)
        );
        // Both surface blocks fill period 0; the ore block follows in period 1.
        let s = greedy(p, &[Some(0); 3]).unwrap();
        assert_eq!(s, vec![Some(0), Some(0), Some(1)]);
        assert_eq!(check(p, &s).unwrap().period_values, vec![-350, 925]);
    }

    #[test]
    fn uses_round_up_and_limits_round_down() {
        let text = CPIT
            .replace("0 1 L 2\n", "0 1 L 2.25\n")
            .replace("2 0 1.5\n", "2 0 1.51\n");
        let m = parse(&text, PREC, 2, Rounding::Refuse, 1).unwrap();
        assert_eq!(m.problem.usage(0), Some(&[10, 10, 16][..]));
        assert_eq!(m.problem.limits(0), Some(&[20, 22][..]));
        assert_eq!((m.rounded_uses, m.rounded_limits), (1, 1));
    }

    #[test]
    fn every_malformed_pair_refuses() {
        let r = Rounding::HalfAwayFromZero;
        let cases = [
            (CPIT.replace("TYPE: CPIT", "TYPE: UPIT"), "not CPIT"),
            (CPIT.replace("DISCOUNT_RATE: 0.08\n", ""), "all required"),
            (CPIT.replace("NPERIODS: 2", "NPERIODS: 0"), "NPERIODS is 0"),
            (
                CPIT.replace("NPERIODS: 2", "NPERIODS: 65537"),
                "NPERIODS is 65537",
            ),
            // The fuzz target's first crash: no resources, so no limit lines bound the periods.
            (
                "NAME: t\nTYPE: CPIT\nNBLOCKS: 3\nNPERIODS: 4444444444444444444\nNRESOURCE_SIDE_CONSTRAINTS: 0\nDISCOUNT_RATE: 0.08\nRESOURCE_CONSTRAINT_LIMITS:\nOBJECTIVE_FUNCTION:\n0 5\n1 -2\n2 9\nRESOURCE_CONSTRAINT_COEFFICIENTS:\nEOF\n".to_string(),
                "at most 65536",
            ),
            (
                CPIT.replace("NBLOCKS: 3", "NBLOCKS: 3\nNBLOCKS: 3"),
                "given twice",
            ),
            (
                CPIT.replace("NBLOCKS: 3", "NBLOCKS: 3\nSHAPE: 1"),
                "not a CPIT header",
            ),
            (
                CPIT.replace("DISCOUNT_RATE: 0.08", "DISCOUNT_RATE: -0.08"),
                "negative",
            ),
            (
                CPIT.replace(
                    "NRESOURCE_SIDE_CONSTRAINTS: 1",
                    "NRESOURCE_SIDE_CONSTRAINTS: 65",
                ),
                "at most 64",
            ),
            (CPIT.replace("0 1 L 2", "0 1 G 2"), "only L"),
            (CPIT.replace("0 1 L 2\n", ""), "1 limit lines"),
            (CPIT.replace("0 1 L 2", "0 0 L 2"), "two limits"),
            (CPIT.replace("0 1 L 2", "0 2 L 2"), "outside"),
            (CPIT.replace("0 1 L 2", "0 1 L -2"), "negative"),
            (CPIT.replace("2 9.25\n", ""), "2 objective lines"),
            (CPIT.replace("2 9.25", "1 9.25"), "two values"),
            (CPIT.replace("2 9.25", "3 9.25"), "not below NBLOCKS"),
            (
                CPIT.replace("2 9.25", "2 9.25 1"),
                "not a OBJECTIVE_FUNCTION line",
            ),
            (CPIT.replace("1 0 1\n", "0 0 1\n"), "two uses"),
            (CPIT.replace("1 0 1\n", "1 1 1\n"), "outside"),
            (CPIT.replace("1 0 1\n", "1 0 -1\n"), "negative"),
            (
                CPIT.replace("OBJECTIVE_FUNCTION:", "OBJECTIVES:"),
                "not a CPIT section",
            ),
            (
                CPIT.replace("RESOURCE_CONSTRAINT_COEFFICIENTS:", "OBJECTIVE_FUNCTION:"),
                "given twice",
            ),
            (
                CPIT.replace(
                    "RESOURCE_CONSTRAINT_COEFFICIENTS:\n0 0 1\n1 0 1\n2 0 1.5\n",
                    "",
                ),
                "RESOURCE_CONSTRAINT_COEFFICIENTS is missing",
            ),
            (CPIT.replace("0 0 1\n", "0: 0 1\n"), "neither a section"),
        ];
        for (text, why) in cases {
            let e = refused(parse(&text, PREC, 2, r, 1));
            assert!(
                e.starts_with("FMINE-REF-506 ") && e.contains(why),
                "{why}: {e}"
            );
        }
        let e = refused(parse(CPIT, &PREC.replace("2 2 0 1", "2 1 0 1"), 2, r, 1));
        assert!(e.contains("declares 1"), "{e}");
        let e = refused(parse(
            &CPIT.replace("2 9.25", "2 9.255"),
            PREC,
            2,
            Rounding::Refuse,
            1,
        ));
        assert!(e.contains("past 2"), "{e}");
    }
}
