//! MineLib's precedence-constrained production scheduling (PCPSP) text format, and schedules that send each mined
//! block whole to one destination (`bd-2rne`). No I/O: the caller reads the bytes and binds their digests.
//! Determinism: BitwisePortable (`i64` and `i128` integer arithmetic on decimal tokens only; no floating point, hashing
//! or threads). The **Price** move's `f64` guidance is in [`price`]; what it proposes is checked here in `i128`.
//!
//! A `.pcpsp` file is laid out as a `.cpit` file is ([`super::cpit`]), except for:
//! - `TYPE: PCPSP` and two more header lines, `NDESTINATIONS:` and `NGENERAL_SIDE_CONSTRAINTS:`;
//! - `OBJECTIVE_FUNCTION:` lines `<block> <value at destination 0> ... <value at destination D − 1>`;
//! - `RESOURCE_CONSTRAINT_COEFFICIENTS:` lines `<block> <destination> <resource> <use>`, at most one per triple; a
//!   triple not listed uses nothing.
//!
//! Values, uses and limits are scaled and rounded as in [`super::cpit`]. Values are scaled at `value_decimals` with the
//! caller's rounding, and uses and limits at `use_decimals`. A use with more places is rounded up (toward +∞, whatever
//! its sign) and a limit down, so a schedule within the rounded limits is within the file's. Each rounding is counted.
//!
//! - **Negative uses** are read: MineLib's w23 has them in its blending rows, where a block below the target grade
//!   makes room at the mill for one above it. Limits must not be negative, so mining nothing stays feasible.
//! - **Not read.** Each of these is refused with its reason, since [`ScheduleProblem`] holds none of them: general
//!   side constraints (a count other than 0), and `G` and `I` limits.
//! - **Whole blocks.** PCPSP lets a mined block be split between destinations. Here each mined block goes whole to one
//!   destination, so a schedule is feasible for PCPSP but can be worth less than the best split one.
//!   [`PcpspModel::with_destinations`] turns one destination per block into a [`ScheduleProblem`], so [`check`],
//!   [`super::greedy`] and [`super::search::improve`] apply unchanged.
//!
//! [`greedy`] is TopoSort with a fallback. A ranked block that fits at its destination in no period from its required
//! blocks' latest on goes to another destination, the most valuable first (the lowest of equals), in the earliest
//! period where one fits. Under [`super::greedy`] it would stay unmined, and so would every block that requires it. On
//! MineLib's w23, whose blending rows leave ore no room at the mill once the blend is spent, that stopped all mining
//! after period 2. A block mined at a loss this way and then required by nothing is dropped by the period moves.
//!
//! [`destinations`] changes the destinations of mined blocks, period by period:
//! - **Switch.** A block goes to its most valuable destination that keeps every limit in its period, when that is
//!   worth more than its current one.
//! - **Exchange.** Two blocks change together. One goes to its most valuable destination, worth more than its current
//!   one but not fitting alone. The other goes to a destination worth no more that uses no more of any resource and
//!   less of one. The pair must keep every limit, and the first must gain more than the second loses. Larger gains
//!   choose first, among the [`SWAP_SCAN`] smallest losses.
//! - **Price** (`bd-rbxh`). Once switches and exchanges stop in a period, all of its mined blocks are reassigned
//!   together ([`price`]). Row prices come from the period's destination LP, solved by Dantzig–Wolfe from the
//!   current assignment. The candidate assignments they give are repaired to every limit in `i128`. The best is
//!   taken if it is worth more. On w23, switches and exchanges stop where the blend allows no single block or pair
//!   into the mill. That left the period values 20.9% below the destination LP at the same mining periods. At most
//!   one Price move per period per call.
//!
//! Each move strictly raises its period's value, and moves are made only in periods whose discount factor is
//! positive, so each strictly raises the discounted value. [`destinations`] also gives each unmined block its most
//! valuable destination (the lowest of equals), which changes no value but lets the period moves' **Add** take the
//! block where it is worth mining. [`improve`] alternates [`super::search::improve_without_cones`] on the
//! periods with [`destinations`] until a cycle makes no move, then [`super::search::improve`], whose **Cone** move
//! takes a block earlier with what it requires, with [`destinations`] until a cycle makes no move again, or the
//! caller's limit (`bd-ynoj`). A cone takes room that destination switches may use better: allowed from the start, it
//! left MineLib's w23 9.9% lower. It ends with [`check`] and refuses rather than return a schedule that fails it or
//! is worth less than the one it was given.

use fmine_evidence::Refusal;

use super::cpit::MAX_RESOURCES;
use super::search::{self, Moves, SWAP_SCAN, discounted_value};
use super::{MAX_PERIODS, ScheduleProblem, check, refuse};
use crate::pitflow::ClosureProblem;
use crate::pitflow::minelib::{MAX_DECIMALS, Rounding, bad, lines, number, parse_prec, scaled};

pub mod lp;
pub mod price;

/// The most destinations a PCPSP file may declare.
pub const MAX_DESTINATIONS: usize = 16;

/// A parsed PCPSP pair.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PcpspModel {
    /// The `NAME:` header.
    pub name: String,
    periods: usize,
    destinations: usize,
    /// `values[b * destinations + d]`, at 10^`value_decimals`.
    values: Vec<i64>,
    /// `uses[r][b * destinations + d]`, at 10^`use_decimals`.
    uses: Vec<Vec<i64>>,
    /// `limits[r][t]`, at 10^`use_decimals`.
    limits: Vec<Vec<i64>>,
    requires: Vec<Vec<u32>>,
    /// The discount rate per period, times [`super::cpit::RATE_SCALE`].
    pub discount_rate: i64,
    /// How many values were rounded.
    pub rounded_values: usize,
    /// How many uses were rounded up.
    pub rounded_uses: usize,
    /// How many limits were rounded down.
    pub rounded_limits: usize,
}

impl PcpspModel {
    /// Number of blocks.
    #[must_use]
    pub fn blocks(&self) -> usize {
        self.requires.len()
    }

    /// Number of periods.
    #[must_use]
    pub fn periods(&self) -> usize {
        self.periods
    }

    /// Number of destinations.
    #[must_use]
    pub fn destinations(&self) -> usize {
        self.destinations
    }

    /// Number of resources.
    #[must_use]
    pub fn resources(&self) -> usize {
        self.uses.len()
    }

    fn slot(&self, b: usize, d: usize) -> Option<usize> {
        if b >= self.blocks() || d >= self.destinations {
            return None;
        }
        Some(b * self.destinations + d)
    }

    /// Block `b`'s value at destination `d`; `None` outside the model.
    #[must_use]
    pub fn value(&self, b: usize, d: usize) -> Option<i64> {
        self.slot(b, d).map(|s| self.values[s])
    }

    /// Block `b`'s use of resource `r` at destination `d`; `None` outside the model.
    #[must_use]
    pub fn uses(&self, r: usize, b: usize, d: usize) -> Option<i64> {
        let s = self.slot(b, d)?;
        self.uses.get(r).map(|row| row[s])
    }

    /// Resource `r`'s limit in each period; `None` when there is no resource `r`.
    #[must_use]
    pub fn limits(&self, r: usize) -> Option<&[i64]> {
        self.limits.get(r).map(Vec::as_slice)
    }

    /// The blocks `b` requires; `None` when there is no block `b`.
    #[must_use]
    pub fn requires(&self, b: usize) -> Option<&[u32]> {
        self.requires.get(b).map(Vec::as_slice)
    }

    /// The schedule problem in which block `b` goes whole to `destination[b]`, with that destination's value and uses.
    ///
    /// # Errors
    /// `FMINE-REF-207` (proposed) when `destination` does not have one entry per block or names a destination past
    /// the last, and as [`ClosureProblem::from_requirements`] and [`ScheduleProblem::new`].
    pub fn with_destinations(&self, destination: &[u32]) -> Result<ScheduleProblem, Refusal> {
        let n = self.blocks();
        if destination.len() != n {
            return Err(refuse(format!(
                "pcpsp: {} destinations for {n} blocks",
                destination.len()
            )));
        }
        if let Some(b) = destination
            .iter()
            .position(|&d| d as usize >= self.destinations)
        {
            return Err(refuse(format!(
                "pcpsp: block {b} goes to destination {}, but there are {}",
                destination[b], self.destinations
            )));
        }
        let pick = |b: usize| b * self.destinations + destination[b] as usize;
        let values = (0..n).map(|b| self.values[pick(b)]).collect();
        let usage = self
            .uses
            .iter()
            .map(|row| (0..n).map(|b| row[pick(b)]).collect())
            .collect();
        let blocks = ClosureProblem::from_requirements(values, &self.requires)?;
        ScheduleProblem::new(blocks, self.periods, usage, self.limits.clone())
    }
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
    destinations: usize,
    resources: usize,
    rate: i64,
}

/// A numbered line: `(line number, trimmed text)`.
type Line<'a> = (usize, &'a str);

/// The header, and the first section's line, which ends it.
fn header<'a>(
    it: &mut impl Iterator<Item = Line<'a>>,
) -> Result<(Header, Option<Line<'a>>), Refusal> {
    let (mut name, mut kind, mut blocks, mut periods) = (None, None, None, None);
    let (mut destinations, mut resources, mut general, mut rate) = (None, None, None, None);
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
            "NDESTINATIONS" => destinations
                .replace(number(value, "NDESTINATIONS", line)?)
                .is_none(),
            "NRESOURCE_SIDE_CONSTRAINTS" => resources
                .replace(number(value, "NRESOURCE_SIDE_CONSTRAINTS", line)?)
                .is_none(),
            "NGENERAL_SIDE_CONSTRAINTS" => general
                .replace(number(value, "NGENERAL_SIDE_CONSTRAINTS", line)?)
                .is_none(),
            "DISCOUNT_RATE" => rate
                .replace(scaled(value, MAX_DECIMALS, Rounding::Refuse)?.0)
                .is_none(),
            other => {
                return Err(bad(format!(
                    "line {line}: header {other:?} is not a PCPSP header"
                )));
            }
        };
        if !fresh {
            return Err(bad(format!("line {line}: header {key:?} given twice")));
        }
    }
    let (
        Some(name),
        Some(kind),
        Some(blocks),
        Some(periods),
        Some(destinations),
        Some(resources),
        Some(general),
        Some(rate),
    ) = (
        name,
        kind,
        blocks,
        periods,
        destinations,
        resources,
        general,
        rate,
    )
    else {
        return Err(bad("NAME, TYPE, NBLOCKS, NPERIODS, NDESTINATIONS, NRESOURCE_SIDE_CONSTRAINTS, NGENERAL_SIDE_CONSTRAINTS and DISCOUNT_RATE are all required before the first section".into()));
    };
    if kind != "PCPSP" {
        return Err(bad(format!("TYPE is {kind:?}, not PCPSP")));
    }
    if periods == 0 || periods > MAX_PERIODS {
        return Err(bad(format!(
            "NPERIODS is {periods}; at least 1 and at most {MAX_PERIODS}"
        )));
    }
    if destinations == 0 || destinations > MAX_DESTINATIONS {
        return Err(bad(format!(
            "NDESTINATIONS is {destinations}; at least 1 and at most {MAX_DESTINATIONS}"
        )));
    }
    if resources > MAX_RESOURCES {
        return Err(bad(format!(
            "{resources} resources; at most {MAX_RESOURCES}"
        )));
    }
    if general != 0 {
        return Err(bad(format!(
            "{general} general side constraints; they are not read, so only 0 is"
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
            destinations,
            resources,
            rate,
        },
        first_section,
    ))
}

/// Parses a `.pcpsp` and `.prec` pair.
///
/// # Errors
/// `FMINE-REF-506` (proposed) for any malformed, missing, duplicate or out-of-range header, section, limit, value,
/// use or precedence line; a limit kind other than `L`; general side constraints; a negative limit; more than
/// [`MAX_RESOURCES`] resources or [`MAX_DESTINATIONS`] destinations; and a value that needs rounding under
/// [`Rounding::Refuse`]. `FMINE-REF-207` (proposed) when the precedences fail [`ClosureProblem::new`]'s size guard,
/// or when the blocks' values largest in magnitude, one per block, sum to [`crate::pitflow::ABS_VALUE_SUM_LIMIT`]
/// or more, so that some choice of destinations could not be scheduled.
pub fn parse(
    pcpsp: &str,
    prec: &str,
    value_decimals: u32,
    value_rounding: Rounding,
    use_decimals: u32,
) -> Result<PcpspModel, Refusal> {
    let mut it = lines(pcpsp);
    let (head, first_section) = header(&mut it)?;
    let (n, periods, dn, resources) =
        (head.blocks, head.periods, head.destinations, head.resources);
    let mut limit_rows = Vec::new();
    let mut value_rows = Vec::new();
    let mut value_list = Vec::new();
    let mut use_rows = Vec::new();
    let mut seen = [false; 3];
    let mut section: Option<usize> = None;
    let mut open = |line: usize, key: &str| -> Result<usize, Refusal> {
        let s = SECTIONS.iter().position(|&k| k == key).ok_or_else(|| {
            bad(format!(
                "line {line}: {key:?} is not a PCPSP section read here"
            ))
        })?;
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
            (Some(1), [b, values @ ..]) if values.len() == dn => {
                for value in values {
                    let (w, was) = scaled(value, value_decimals, value_rounding)?;
                    rounded_values += usize::from(was);
                    value_list.push(w);
                }
                value_rows.push((line, number(b, "block", line)?));
            }
            (Some(2), [b, d, r, q]) => {
                let (q, was) = scaled(q, use_decimals, Rounding::Ceiling)?;
                rounded_uses += usize::from(was);
                use_rows.push((
                    line,
                    number(b, "block", line)?,
                    number(d, "destination", line)?,
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
                    "line {line}: {l:?} is not a {} line for {dn} destinations",
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
    // `n` objective lines of `dn` values each are present, so `n * dn` is `value_list.len()`.
    let mut values = vec![0_i64; value_list.len()];
    let mut placed = vec![false; n];
    for ((line, b), row) in value_rows.into_iter().zip(value_list.chunks(dn)) {
        let was = placed
            .get_mut(b)
            .ok_or_else(|| bad(format!("line {line}: block {b} is not below NBLOCKS {n}")))?;
        if std::mem::replace(was, true) {
            return Err(bad(format!("line {line}: block {b} has two value lines")));
        }
        values[b * dn..(b + 1) * dn].copy_from_slice(row);
    }
    let mut uses = vec![vec![0_i64; n * dn]; resources];
    let mut listed = vec![vec![false; n * dn]; resources];
    for (line, b, d, r, q) in use_rows {
        let at = (b < n && d < dn).then(|| b * dn + d);
        let (Some(slot), Some(was)) = (
            at.and_then(|s| uses.get_mut(r).and_then(|row| row.get_mut(s))),
            at.and_then(|s| listed.get_mut(r).and_then(|row| row.get_mut(s))),
        ) else {
            return Err(bad(format!(
                "line {line}: block {b}, destination {d}, resource {r} is outside {n} blocks, {dn} destinations and {resources} resources"
            )));
        };
        if std::mem::replace(was, true) {
            return Err(bad(format!(
                "line {line}: block {b}, destination {d}, resource {r} has two uses"
            )));
        }
        *slot = q;
    }
    let requires = parse_prec(prec, n)?;
    // The arcs' shape and size, once, with each block at its value largest in magnitude. `with_destinations` builds
    // the problem for each choice of destinations, and no choice can then pass pitflow's limit on the sum of absolute
    // values. `saturating_abs` differs from `abs` only at `i64::MIN`, which is far past that limit either way.
    let largest: Vec<i64> = values
        .chunks(dn)
        .map(|v| v.iter().map(|x| x.saturating_abs()).max().unwrap_or(0))
        .collect();
    ClosureProblem::from_requirements(largest, &requires)?;
    Ok(PcpspModel {
        name: head.name,
        periods,
        destinations: dn,
        values,
        uses,
        limits,
        requires,
        discount_rate: head.rate,
        rounded_values,
        rounded_uses,
        rounded_limits,
    })
}

/// How many moves of each kind [`destinations`] made.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DestinationMoves {
    /// Blocks that went to a more valuable destination alone.
    pub switches: usize,
    /// Pairs of blocks that changed destination together.
    pub exchanges: usize,
    /// Periods whose mined blocks were reassigned together from row prices ([`price`]).
    pub priced: usize,
    /// Unmined blocks given their most valuable destination.
    pub unmined: usize,
}

/// Whether `used` plus the changes keeps every limit. Each change is `(block, from, to)`.
fn fits(
    model: &PcpspModel,
    used: &[i128],
    caps: &[i128],
    changes: &[(usize, usize, usize)],
) -> bool {
    (0..model.resources()).all(|r| {
        let row = &model.uses[r];
        let delta: i128 = changes
            .iter()
            .map(|&(b, from, to)| {
                let dn = model.destinations;
                i128::from(row[b * dn + to]) - i128::from(row[b * dn + from])
            })
            .sum();
        used[r] + delta <= caps[r]
    })
}

/// Applies one change to `used` and `destination`.
fn apply(
    model: &PcpspModel,
    used: &mut [i128],
    destination: &mut [u32],
    (b, from, to): (usize, usize, usize),
) -> Result<(), Refusal> {
    let dn = model.destinations;
    for (u, row) in used.iter_mut().zip(&model.uses) {
        *u += i128::from(row[b * dn + to]) - i128::from(row[b * dn + from]);
    }
    destination[b] = u32::try_from(to)
        .map_err(|_| refuse(format!("pcpsp: destination {to} does not fit u32")))?;
    Ok(())
}

/// TopoSort with a fallback to other destinations (see the module notes). `rank` is as for [`super::greedy`], and
/// `destination[b]` is the destination block `b` tries first. Returns the schedule and each block's destination,
/// which differs from `destination` only for blocks that fell back.
///
/// # Errors
/// `FMINE-REF-207` (proposed) when `rank` or `destination` does not have one entry per block, when a destination is
/// out of range, or, as a defect, when the result fails [`check`].
pub fn greedy(
    model: &PcpspModel,
    rank: &[Option<i64>],
    destination: &[u32],
) -> Result<(Vec<Option<u32>>, Vec<u32>), Refusal> {
    greedy_from(model, rank, destination, &vec![0; model.blocks()])
}

/// [`greedy`] with a release period per block: block `b` is mined in period `release[b]` or later, and otherwise as
/// [`greedy`] places it. A release keeps a block from taking room in an earlier period that the blocks ranked after it
/// would use better, such as a period the caller's LP fills with a blend.
///
/// # Errors
/// As [`greedy`], and `FMINE-REF-207` (proposed) when `release` does not have one entry per block or names a period
/// past the last.
pub fn greedy_from(
    model: &PcpspModel,
    rank: &[Option<i64>],
    destination: &[u32],
    release: &[u32],
) -> Result<(Vec<Option<u32>>, Vec<u32>), Refusal> {
    let start = model.with_destinations(destination)?;
    let (periods, dn) = (model.periods, model.destinations);
    if release.len() != model.blocks() {
        return Err(refuse(format!(
            "pcpsp greedy: {} release periods for {} blocks",
            release.len(),
            model.blocks()
        )));
    }
    if let Some(b) = release.iter().position(|&t| t as usize >= periods) {
        return Err(refuse(format!(
            "pcpsp greedy: block {b}'s release period {} is past the last of {periods}",
            release[b]
        )));
    }
    let mut dest = destination.to_vec();
    let mut used = vec![vec![0_i128; periods]; model.resources()];
    let schedule = super::kahn(start.blocks(), rank, |b, from| {
        let from = from.max(release[b] as usize);
        let fits = |d: usize, t: usize| {
            used.iter()
                .zip(&model.uses)
                .zip(&model.limits)
                .all(|((row, uses), caps)| {
                    row[t] + i128::from(uses[b * dn + d]) <= i128::from(caps[t])
                })
        };
        let first = dest[b] as usize;
        let mut others: Vec<usize> = (0..dn).filter(|&d| d != first).collect();
        others.sort_by_key(|&d| (std::cmp::Reverse(model.values[b * dn + d]), d));
        let (t, d) = (from..periods)
            .find(|&t| fits(first, t))
            .map(|t| (t, first))
            .or_else(|| {
                (from..periods).find_map(|t| others.iter().find(|&&d| fits(d, t)).map(|&d| (t, d)))
            })?;
        // `d` is below `dn`, which is at most `MAX_DESTINATIONS`, so it fits.
        dest[b] = u32::try_from(d).ok()?;
        for (row, uses) in used.iter_mut().zip(&model.uses) {
            row[t] += i128::from(uses[b * dn + d]);
        }
        Some(t)
    })?;
    let end = model.with_destinations(&dest)?;
    let checked = check(&end, &schedule)?;
    if !checked.feasible() {
        return Err(refuse(format!(
            "pcpsp greedy: defect, the schedule fails its check ({} unmet requirements, limits exceeded at {:?})",
            checked.unmet, checked.over_limit
        )));
    }
    Ok((schedule, dest))
}

/// Destination moves on a feasible schedule (see the module notes). `destination` is updated in place.
///
/// # Errors
/// `FMINE-REF-207` (proposed) when `schedule` and `destination` fail [`check`] or do not fit the model, when `factors`
/// does not have one per period, or, as a defect, when the result fails [`check`] or is worth less.
pub fn destinations(
    model: &PcpspModel,
    schedule: &[Option<u32>],
    destination: &mut [u32],
    factors: &[i64],
) -> Result<DestinationMoves, Refusal> {
    let start = model.with_destinations(destination)?;
    let first = check(&start, schedule)?;
    if !first.feasible() {
        return Err(refuse(format!(
            "destinations: the schedule given fails its check ({} unmet requirements, limits exceeded at {:?})",
            first.unmet, first.over_limit
        )));
    }
    if factors.len() != model.periods {
        return Err(refuse(format!(
            "destinations: {} factors for {} periods",
            factors.len(),
            model.periods
        )));
    }
    let before = discounted_value(&start, schedule, factors)?;
    let dn = model.destinations;
    let value = |b: usize, d: usize| i128::from(model.values[b * dn + d]);
    let mut by_period: Vec<Vec<usize>> = vec![Vec::new(); model.periods];
    for (b, slot) in schedule.iter().enumerate() {
        if let Some(t) = *slot {
            by_period[t as usize].push(b);
        }
    }
    let mut moves = DestinationMoves::default();
    for (b, slot) in schedule.iter().enumerate() {
        let best = (0..dn).max_by_key(|&d| (value(b, d), std::cmp::Reverse(d)));
        if let (None, Some(d)) = (slot, best)
            && d != destination[b] as usize
        {
            destination[b] = u32::try_from(d)
                .map_err(|_| refuse(format!("pcpsp: destination {d} does not fit u32")))?;
            moves.unmined += 1;
        }
    }
    let mut touched = vec![false; model.blocks()];
    for (t, mined) in by_period.iter().enumerate() {
        if factors[t] <= 0 || mined.is_empty() {
            continue;
        }
        let mut used: Vec<i128> = first.used.iter().map(|row| row[t]).collect();
        let caps: Vec<i128> = model.limits.iter().map(|row| i128::from(row[t])).collect();
        let mut priced = false;
        loop {
            let mut moved = false;
            for &b in mined {
                let c = destination[b] as usize;
                let best = (0..dn)
                    .filter(|&d| {
                        value(b, d) > value(b, c) && fits(model, &used, &caps, &[(b, c, d)])
                    })
                    .max_by_key(|&d| (value(b, d), std::cmp::Reverse(d)));
                if let Some(d) = best {
                    apply(model, &mut used, destination, (b, c, d))?;
                    moves.switches += 1;
                    moved = true;
                }
            }
            // `(gain, block, to)`, largest gain first; `(loss, block, to)`, smallest loss first.
            let mut ups = Vec::new();
            let mut downs = Vec::new();
            for &b in mined {
                let c = destination[b] as usize;
                if let Some(d) = (0..dn)
                    .filter(|&d| value(b, d) > value(b, c))
                    .max_by_key(|&d| (value(b, d), std::cmp::Reverse(d)))
                {
                    ups.push((value(b, d) - value(b, c), b, d));
                }
                let frees = |d: usize| {
                    let at = |r: usize, x: usize| model.uses[r][b * dn + x];
                    (0..model.resources()).all(|r| at(r, d) <= at(r, c))
                        && (0..model.resources()).any(|r| at(r, d) < at(r, c))
                };
                if let Some(d) = (0..dn)
                    .filter(|&d| d != c && value(b, d) <= value(b, c) && frees(d))
                    .min_by_key(|&d| (value(b, c) - value(b, d), d))
                {
                    downs.push((value(b, c) - value(b, d), b, d));
                }
            }
            ups.sort_by_key(|&(gain, b, _)| (std::cmp::Reverse(gain), b));
            downs.sort_by_key(|&(loss, a, _)| (loss, a));
            for &(gain, b, d) in &ups {
                if touched[b] {
                    continue;
                }
                let c = destination[b] as usize;
                let mut scanned = 0;
                for &(loss, a, e) in &downs {
                    if loss >= gain || scanned == SWAP_SCAN {
                        break;
                    }
                    if a == b || touched[a] {
                        continue;
                    }
                    scanned += 1;
                    let ca = destination[a] as usize;
                    if fits(model, &used, &caps, &[(b, c, d), (a, ca, e)]) {
                        apply(model, &mut used, destination, (b, c, d))?;
                        apply(model, &mut used, destination, (a, ca, e))?;
                        touched[a] = true;
                        touched[b] = true;
                        moves.exchanges += 1;
                        moved = true;
                        break;
                    }
                }
            }
            for &b in mined {
                touched[b] = false;
            }
            // Price, once per period, when switches and exchanges have stopped (`bd-rbxh`).
            if !moved && !priced {
                priced = true;
                if let Some(pick) = price::reprice(model, mined, destination, &caps) {
                    for (&b, &d) in mined.iter().zip(&pick) {
                        let c = destination[b] as usize;
                        if d != c {
                            apply(model, &mut used, destination, (b, c, d))?;
                        }
                    }
                    moves.priced += 1;
                    moved = true;
                }
            }
            if !moved {
                break;
            }
        }
    }
    let end = model.with_destinations(destination)?;
    let last = check(&end, schedule)?;
    let after = discounted_value(&end, schedule, factors)?;
    if !last.feasible() || after < before {
        return Err(refuse(format!(
            "destinations: defect, the result is {} and worth {after} against {before}",
            if last.feasible() {
                "feasible"
            } else {
                "infeasible"
            }
        )));
    }
    Ok(moves)
}

/// What [`improve`] returns.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PcpspImprovement {
    /// The improved schedule. With `destination`, it passes [`check`].
    pub schedule: Vec<Option<u32>>,
    /// The improved destinations.
    pub destination: Vec<u32>,
    /// The discounted value of the schedule and destinations given.
    pub before: i128,
    /// The discounted value of the result; never below `before`.
    pub after: i128,
    /// Cycles of period moves then destination moves, the last of them the one without a move unless the limit
    /// stopped it.
    pub cycles: usize,
    /// Period moves, from [`super::search::improve_without_cones`] and then [`super::search::improve`].
    pub moves: Moves,
    /// Destination moves, from [`destinations`].
    pub destination_moves: DestinationMoves,
}

/// Period moves and destination moves in turn, from a feasible schedule (see the module notes). `max_rounds` limits
/// the cycles and each period search's rounds.
///
/// # Errors
/// `FMINE-REF-207` (proposed) when the schedule and destinations given fail [`check`] or do not fit the model, when
/// `factors` does not have one non-negative factor per period or rises, or, as a defect, when the result fails
/// [`check`] or is worth less than the input.
pub fn improve(
    model: &PcpspModel,
    schedule: &[Option<u32>],
    destination: &[u32],
    factors: &[i64],
    max_rounds: usize,
) -> Result<PcpspImprovement, Refusal> {
    let start = model.with_destinations(destination)?;
    let first = check(&start, schedule)?;
    if !first.feasible() {
        return Err(refuse(format!(
            "pcpsp improve: the schedule given fails its check ({} unmet requirements, limits exceeded at {:?})",
            first.unmet, first.over_limit
        )));
    }
    let before = discounted_value(&start, schedule, factors)?;
    let mut schedule = schedule.to_vec();
    let mut destination = destination.to_vec();
    let mut moves = Moves::default();
    let mut destination_moves = DestinationMoves::default();
    let mut cycles = 0;
    // Cones only once period and destination moves have both stopped without them (`bd-ynoj`).
    let mut cones = false;
    while cycles < max_rounds {
        cycles += 1;
        let problem = model.with_destinations(&destination)?;
        let timed = if cones {
            search::improve(&problem, &schedule, factors, max_rounds)?
        } else {
            search::improve_without_cones(&problem, &schedule, factors, max_rounds)?
        };
        let m = timed.moves;
        moves.later += m.later;
        moves.earlier += m.earlier;
        moves.cones += m.cones;
        moves.swaps += m.swaps;
        moves.added += m.added;
        moves.dropped += m.dropped;
        schedule = timed.schedule;
        let d = destinations(model, &schedule, &mut destination, factors)?;
        destination_moves.switches += d.switches;
        destination_moves.exchanges += d.exchanges;
        destination_moves.priced += d.priced;
        destination_moves.unmined += d.unmined;
        if m == Moves::default() && d == DestinationMoves::default() {
            if cones {
                break;
            }
            cones = true;
        }
    }
    let end = model.with_destinations(&destination)?;
    let last = check(&end, &schedule)?;
    let after = discounted_value(&end, &schedule, factors)?;
    if !last.feasible() || after < before {
        return Err(refuse(format!(
            "pcpsp improve: defect, the result is {} and worth {after} against {before}",
            if last.feasible() {
                "feasible"
            } else {
                "infeasible"
            }
        )));
    }
    Ok(PcpspImprovement {
        schedule,
        destination,
        before,
        after,
        cycles,
        moves,
        destination_moves,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::toposort::cpit::RATE_SCALE;
    use crate::toposort::greedy;
    use crate::toposort::search::discount_factors;
    use crate::toposort::tests::lcg;

    // Two waste blocks over one ore block. Destination 0 is the dump and 1 the mill. Resource 0 is mining, used alike
    // at both; resource 1 is the mill, used only at destination 1.
    const PCPSP: &str = "NAME: tiny\nTYPE: PCPSP\nNBLOCKS: 3\nNPERIODS: 2\nNDESTINATIONS: 2\nNRESOURCE_SIDE_CONSTRAINTS: 2\nNGENERAL_SIDE_CONSTRAINTS: 0\nDISCOUNT_RATE: 0.1\nRESOURCE_CONSTRAINT_LIMITS:\n0 0 L 2\n0 1 L 2\n1 0 L 1\n1 1 L 1\nOBJECTIVE_FUNCTION:\n% dump, mill\n0 -1 -3\n1 -1 -3\n2 -2 9.5\nRESOURCE_CONSTRAINT_COEFFICIENTS:\n0 0 0 1\n0 1 0 1\n0 1 1 1\n1 0 0 1\n1 1 0 1\n1 1 1 1\n2 0 0 1\n2 1 0 1\n2 1 1 1\nEOF\n";
    const PREC: &str = "0 0\n1 0\n2 2 0 1\n";

    fn refused(r: Result<PcpspModel, Refusal>) -> String {
        match r {
            Ok(m) => panic!("expected a refusal, got {m:?}"),
            Err(e) => format!("{} {}", e.code, e.what),
        }
    }

    fn factors(periods: usize) -> Vec<i64> {
        discount_factors(100_000_000, RATE_SCALE, periods).unwrap()
    }

    #[test]
    fn tiny_pair_reads_exactly_and_schedules_by_destination() {
        let m = parse(PCPSP, PREC, 1, Rounding::Refuse, 0).unwrap();
        assert_eq!((m.name.as_str(), m.discount_rate), ("tiny", 100_000_000));
        assert_eq!(
            (m.blocks(), m.periods(), m.destinations(), m.resources()),
            (3, 2, 2, 2)
        );
        assert_eq!(
            (0..3)
                .flat_map(|b| (0..2).map(move |d| (b, d)))
                .map(|(b, d)| m.value(b, d).unwrap())
                .collect::<Vec<_>>(),
            vec![-10, -30, -10, -30, -20, 95]
        );
        assert_eq!(
            (
                m.uses(0, 2, 0),
                m.uses(0, 2, 1),
                m.uses(1, 2, 0),
                m.uses(1, 2, 1)
            ),
            (Some(1), Some(1), Some(0), Some(1))
        );
        assert_eq!(
            (m.value(3, 0), m.value(0, 2), m.uses(2, 0, 0)),
            (None, None, None)
        );
        assert_eq!(
            (m.limits(0), m.limits(1)),
            (Some(&[2, 2][..]), Some(&[1, 1][..]))
        );
        assert_eq!(m.requires(2), Some(&[0, 1][..]));
        let p = m.with_destinations(&[0, 0, 1]).unwrap();
        assert_eq!(p.blocks().weights(), &[-10, -10, 95]);
        assert_eq!(
            (p.usage(0), p.usage(1)),
            (Some(&[1, 1, 1][..]), Some(&[0, 0, 1][..]))
        );
        // Both waste blocks fill period 0's mining; the ore follows in period 1, to the mill.
        let s = greedy(&p, &[Some(0); 3]).unwrap();
        assert_eq!(s, vec![Some(0), Some(0), Some(1)]);
        assert_eq!(check(&p, &s).unwrap().period_values, vec![-20, 95]);
        assert!(m.with_destinations(&[0, 0]).is_err());
        assert!(m.with_destinations(&[0, 0, 2]).is_err());
    }

    #[test]
    fn every_malformed_pair_refuses() {
        let r = Rounding::HalfAwayFromZero;
        let cases = [
            (PCPSP.replace("TYPE: PCPSP", "TYPE: CPIT"), "not PCPSP"),
            (PCPSP.replace("NDESTINATIONS: 2\n", ""), "all required"),
            (
                PCPSP.replace("NDESTINATIONS: 2", "NDESTINATIONS: 0"),
                "NDESTINATIONS is 0",
            ),
            (
                PCPSP.replace("NDESTINATIONS: 2", "NDESTINATIONS: 17"),
                "at most 16",
            ),
            (
                PCPSP.replace("NPERIODS: 2", "NPERIODS: 65537"),
                "NPERIODS is 65537",
            ),
            (
                PCPSP.replace(
                    "NGENERAL_SIDE_CONSTRAINTS: 0",
                    "NGENERAL_SIDE_CONSTRAINTS: 1",
                ),
                "general side constraints",
            ),
            (
                PCPSP.replace("NBLOCKS: 3", "NBLOCKS: 3\nNBLOCKS: 3"),
                "given twice",
            ),
            (
                PCPSP.replace("DISCOUNT_RATE: 0.1", "DISCOUNT_RATE: -0.1"),
                "negative",
            ),
            (
                PCPSP.replace(
                    "NRESOURCE_SIDE_CONSTRAINTS: 2",
                    "NRESOURCE_SIDE_CONSTRAINTS: 65",
                ),
                "at most 64",
            ),
            (PCPSP.replace("1 1 L 1", "1 1 G 1"), "only L"),
            (PCPSP.replace("1 1 L 1\n", ""), "3 limit lines"),
            (PCPSP.replace("2 -2 9.5", "2 -2"), "for 2 destinations"),
            (
                PCPSP.replace("2 -2 9.5", "2 -2 9.5 1"),
                "for 2 destinations",
            ),
            (PCPSP.replace("2 -2 9.5", "1 -2 9.5"), "two value lines"),
            (PCPSP.replace("2 -2 9.5", "3 -2 9.5"), "not below NBLOCKS"),
            (PCPSP.replace("2 1 1 1\n", "2 2 1 1\n"), "outside"),
            (PCPSP.replace("2 1 1 1\n", "2 1 2 1\n"), "outside"),
            (PCPSP.replace("2 1 1 1\n", "2 1 0 1\n"), "two uses"),
            (PCPSP.replace("1 1 L 1", "1 1 L -1"), "negative"),
            (
                PCPSP.replace("OBJECTIVE_FUNCTION:", "GENERAL_CONSTRAINT_LIMITS:"),
                "not a PCPSP section read here",
            ),
            (
                PCPSP.replace("0 0 0 1\n", "0: 0 0 1\n"),
                "neither a section",
            ),
        ];
        for (text, why) in cases {
            let e = refused(parse(&text, PREC, 1, r, 0));
            assert!(
                e.starts_with("FMINE-REF-506 ") && e.contains(why),
                "{why}: {e}"
            );
        }
        let e = refused(parse(
            &PCPSP.replace("2 -2 9.5", "2 -2 9.55"),
            PREC,
            1,
            Rounding::Refuse,
            0,
        ));
        assert!(e.contains("past 1"), "{e}");
        // Found by the pcpsp fuzz target: the mill value alone passes pitflow's limit on absolute values. With the
        // dump chosen the problem was built, and the search's move to the mill was then refused. Now parse refuses.
        // At one place the other blocks' largest magnitudes are 30 and 95, so block 0's mill value at 10 times
        // 461168601842738777 leaves the sum 9 below 2^62, and at 10 times 461168601842738779 it is 11 above.
        for (value, within) in [
            ("777777777777777773", false),
            ("461168601842738779", false),
            ("-461168601842738779", false),
            ("461168601842738777", true),
        ] {
            let text = PCPSP.replace("0 -1 -3", &format!("0 -1 {value}"));
            match parse(&text, PREC, 1, r, 0) {
                Ok(m) => assert!(within, "{value}: read, {m:?}"),
                Err(e) => {
                    let e = format!("{} {}", e.code, e.what);
                    assert!(
                        !within && e.starts_with("FMINE-REF-207 ") && e.contains("not below 2^62"),
                        "{value}: {e}"
                    );
                }
            }
        }
    }

    #[test]
    fn blending_rows_read_signed_uses_and_schedule_within_the_blend() {
        // Resource 0 is the mill (two blocks); resource 1 a blend with limit 0. Block 1 is below the target and makes
        // room for one block above it: block 0 (worth 4, +1) or block 2 (worth 3, +0.5), not both.
        let blend = "NAME: blend\nTYPE: PCPSP\nNBLOCKS: 3\nNPERIODS: 1\nNDESTINATIONS: 2\nNRESOURCE_SIDE_CONSTRAINTS: 2\nNGENERAL_SIDE_CONSTRAINTS: 0\nDISCOUNT_RATE: 0\nRESOURCE_CONSTRAINT_LIMITS:\n0 0 L 2\n1 0 L 0\nOBJECTIVE_FUNCTION:\n0 -1 4\n1 -1 1\n2 -1 3\nRESOURCE_CONSTRAINT_COEFFICIENTS:\n0 1 0 1\n0 1 1 1\n1 1 0 1\n1 1 1 -1.25\n2 1 0 1\n2 1 1 0.5\nEOF\n";
        let m = parse(blend, "0 0\n1 0\n2 0\n", 0, Rounding::Refuse, 1).unwrap();
        // −1.25 at one place rounds toward +∞, to −1.2: less room, never more.
        assert_eq!(
            (
                m.uses(1, 0, 1),
                m.uses(1, 1, 1),
                m.uses(1, 2, 1),
                m.uses(1, 1, 0)
            ),
            (Some(10), Some(-12), Some(5), Some(0))
        );
        assert_eq!(m.rounded_uses, 1);
        let d = vec![1, 1, 1];
        let p = m.with_destinations(&d).unwrap();
        // Ranked above block 1, blocks 0 and 2 find the blend full and stay unmined; the search then adds block 0.
        let s = greedy(&p, &[Some(0), Some(2), Some(1)]).unwrap();
        assert_eq!(s, vec![None, Some(0), None]);
        // With the fallback they go to the dump instead, mined at a loss, and the search drops them.
        let (s2, d2) = super::greedy(&m, &[Some(0), Some(2), Some(1)], &d).unwrap();
        assert_eq!((s2.clone(), d2.clone()), (vec![Some(0); 3], vec![0, 1, 0]));
        let i2 = improve(&m, &s2, &d2, &factors(1), 10).unwrap();
        assert_eq!(i2.schedule, vec![Some(0), Some(0), None]);
        let i = improve(&m, &s, &d, &factors(1), 10).unwrap();
        assert_eq!(i.schedule, vec![Some(0), Some(0), None]);
        let c = check(&m.with_destinations(&i.destination).unwrap(), &i.schedule).unwrap();
        assert!(c.feasible());
        // Uses are at one place: the mill is full (20 of 20) and the blend 0.2 under its target.
        assert_eq!(
            (c.period_values, c.used),
            (vec![5], vec![vec![20], vec![-2]])
        );
    }

    #[test]
    fn switches_and_exchanges_on_known_cases() {
        let m = parse(PCPSP, PREC, 1, Rounding::Refuse, 0).unwrap();
        let s = vec![Some(0), Some(0), Some(1)];
        // Switch: the ore goes to the mill, which has room in period 1.
        let mut d = vec![0, 0, 0];
        let moves = destinations(&m, &s, &mut d, &factors(2)).unwrap();
        assert_eq!(
            (d, moves),
            (
                vec![0, 0, 1],
                DestinationMoves {
                    switches: 1,
                    exchanges: 0,
                    priced: 0,
                    unmined: 0
                }
            )
        );
        // Not where the discount factor is 0.
        let mut d = vec![0, 0, 0];
        let moves = destinations(&m, &s, &mut d, &[1, 0]).unwrap();
        assert_eq!((d, moves), (vec![0, 0, 0], DestinationMoves::default()));
        // Unmined, the ore is given the mill, where it is worth mining; the period moves can then add it.
        let mut d = vec![0, 0, 0];
        let moves = destinations(&m, &[Some(0), Some(0), None], &mut d, &factors(2)).unwrap();
        assert_eq!(
            (d, moves),
            (
                vec![0, 0, 1],
                DestinationMoves {
                    switches: 0,
                    exchanges: 0,
                    priced: 0,
                    unmined: 1
                }
            )
        );
        // Exchange: one mill slot; the block worth 0.5 more at the mill makes way for the one worth 0.8 more.
        let two = "NAME: two\nTYPE: PCPSP\nNBLOCKS: 2\nNPERIODS: 1\nNDESTINATIONS: 2\nNRESOURCE_SIDE_CONSTRAINTS: 1\nNGENERAL_SIDE_CONSTRAINTS: 0\nDISCOUNT_RATE: 0\nRESOURCE_CONSTRAINT_LIMITS:\n0 0 L 1\nOBJECTIVE_FUNCTION:\n0 0 0.5\n1 0 0.8\nRESOURCE_CONSTRAINT_COEFFICIENTS:\n0 1 0 1\n1 1 0 1\n";
        let m = parse(two, "0 0\n1 0\n", 1, Rounding::Refuse, 0).unwrap();
        let mut d = vec![1, 0];
        let moves = destinations(&m, &[Some(0), Some(0)], &mut d, &[1]).unwrap();
        assert_eq!(
            (d, moves),
            (
                vec![0, 1],
                DestinationMoves {
                    switches: 0,
                    exchanges: 1,
                    priced: 0,
                    unmined: 0
                }
            )
        );
        // An infeasible input refuses.
        let mut d = vec![1, 1];
        assert!(destinations(&m, &[Some(0), Some(0)], &mut d, &[1]).is_err());
    }

    /// A random model: up to five blocks, each requiring some lower-numbered blocks, two or three destinations.
    fn random_model(seed: u64) -> PcpspModel {
        let mut r = lcg(seed);
        let n = 2 + r(4) as usize;
        let periods = 1 + r(2) as usize;
        let dn = 2 + r(2) as usize;
        let resources = r(3) as usize;
        let requires: Vec<Vec<u32>> = (0..n)
            .map(|b| (0..b as u32).filter(|_| r(3) == 0).collect())
            .collect();
        PcpspModel {
            name: format!("random {seed}"),
            periods,
            destinations: dn,
            values: (0..n * dn).map(|_| r(21) as i64 - 10).collect(),
            uses: (0..resources)
                .map(|_| (0..n * dn).map(|_| r(4) as i64).collect())
                .collect(),
            limits: (0..resources)
                .map(|_| (0..periods).map(|_| r(7) as i64).collect())
                .collect(),
            requires,
            discount_rate: 100_000_000,
            rounded_values: 0,
            rounded_uses: 0,
            rounded_limits: 0,
        }
    }

    /// [`random_model`] with one more resource whose uses run from −3 to 3 and whose limits from 0 to 2.
    fn random_signed_model(seed: u64) -> PcpspModel {
        let mut m = random_model(seed);
        let mut r = lcg(seed ^ 0x3C3C);
        let cells = m.blocks() * m.destinations();
        m.uses.push((0..cells).map(|_| r(7) as i64 - 3).collect());
        m.limits
            .push((0..m.periods()).map(|_| r(3) as i64).collect());
        m
    }

    /// The discounted value of a choice of period and destination per block, from the definition, or `None` when it
    /// breaks a requirement or a limit.
    fn worth(m: &PcpspModel, s: &[Option<u32>], d: &[u32], f: &[i64]) -> Option<i128> {
        let n = m.blocks();
        for b in 0..n {
            if let Some(t) = s[b] {
                for &q in m.requires(b).unwrap() {
                    if s[q as usize].is_none_or(|tq| tq > t) {
                        return None;
                    }
                }
            }
        }
        for r in 0..m.resources() {
            for t in 0..m.periods() {
                let used: i64 = (0..n)
                    .filter(|&b| s[b] == Some(t as u32))
                    .map(|b| m.uses(r, b, d[b] as usize).unwrap())
                    .sum();
                if used > m.limits(r).unwrap()[t] {
                    return None;
                }
            }
        }
        Some(
            (0..n)
                .filter_map(|b| {
                    s[b].map(|t| {
                        i128::from(m.value(b, d[b] as usize).unwrap()) * i128::from(f[t as usize])
                    })
                })
                .sum(),
        )
    }

    /// The best value over every period (or none) and destination for every block.
    fn optimum(m: &PcpspModel, f: &[i64]) -> i128 {
        let (n, periods, dn) = (m.blocks(), m.periods(), m.destinations());
        let choices = 1 + periods * dn;
        (0..choices.pow(n as u32))
            .filter_map(|mut code| {
                let (mut s, mut d) = (vec![None; n], vec![0_u32; n]);
                for b in 0..n {
                    let c = code % choices;
                    code /= choices;
                    if c > 0 {
                        s[b] = Some(((c - 1) / dn) as u32);
                        d[b] = ((c - 1) % dn) as u32;
                    }
                }
                worth(m, &s, &d, f)
            })
            .max()
            .unwrap_or(0)
    }

    /// Greedy under random destinations and ranks, then [`improve`]: feasible and valued as the definition says,
    /// never worth less, never more than the exhaustive optimum, and a second call moves nothing. Returns the values
    /// before and after, the optimum, and whether an exchange was made.
    fn improve_case(m: &PcpspModel, seed: u64) -> (i128, i128, i128, bool) {
        let mut r = lcg(seed ^ 0x5eed);
        let f = factors(m.periods());
        let d: Vec<u32> = (0..m.blocks())
            .map(|_| r(m.destinations() as u64) as u32)
            .collect();
        let rank: Vec<Option<i64>> = (0..m.blocks())
            .map(|_| (r(5) > 0).then(|| r(10) as i64))
            .collect();
        let s = greedy(&m.with_destinations(&d).unwrap(), &rank).unwrap();
        let i = improve(m, &s, &d, &f, 1_000).unwrap();
        let before = worth(m, &s, &d, &f).unwrap();
        let after = worth(m, &i.schedule, &i.destination, &f);
        assert_eq!(after, Some(i.after), "seed {seed}: infeasible or misvalued");
        assert_eq!(before, i.before, "seed {seed}");
        let best = optimum(m, &f);
        assert!(
            before <= i.after && i.after <= best,
            "seed {seed}: {before} {} {best}",
            i.after
        );
        let again = improve(m, &i.schedule, &i.destination, &f, 1_000).unwrap();
        assert_eq!(
            (again.moves, again.destination_moves, again.after),
            (Moves::default(), DestinationMoves::default(), i.after),
            "seed {seed}: a second call moved"
        );
        (before, i.after, best, i.destination_moves.exchanges > 0)
    }

    #[test]
    fn improve_is_feasible_monotone_and_bounded_by_the_exhaustive_optimum() {
        let (mut improved, mut optimal_before, mut optimal_after, mut exchanged) = (0, 0, 0, 0);
        let cases = 300;
        for seed in 0..cases {
            let (before, after, best, exchange) = improve_case(&random_model(seed), seed);
            improved += usize::from(after > before);
            optimal_before += usize::from(before == best);
            optimal_after += usize::from(after == best);
            exchanged += usize::from(exchange);
        }
        println!(
            "{{\"bead\":\"bd-2rne\",\"cases\":{cases},\"improved\":{improved},\"optimal_greedy\":{optimal_before},\"optimal_after\":{optimal_after},\"with_exchanges\":{exchanged}}}"
        );
        assert!(improved > 0 && exchanged > 0 && optimal_after > optimal_before);
    }

    /// The fallback greedy on random models, with and without a signed resource: feasible by the definition, and a
    /// destination changes only for a block it mined. Without negative uses, a ranked block it leaves unmined has a
    /// required block unmined or fits at no destination in any period from theirs on.
    #[test]
    fn the_fallback_greedy_is_feasible_and_falls_back_only_when_stuck() {
        let (mut fell_back, mut mined_more) = (0, 0);
        for seed in 0..300 {
            for (m, signed) in [
                (random_model(seed), false),
                (random_signed_model(seed), true),
            ] {
                let mut r = lcg(seed ^ 0x5eed);
                let n = m.blocks();
                let d: Vec<u32> = (0..n).map(|_| r(m.destinations() as u64) as u32).collect();
                let rank: Vec<Option<i64>> =
                    (0..n).map(|_| (r(5) > 0).then(|| r(10) as i64)).collect();
                let (s, dest) = super::greedy(&m, &rank, &d).unwrap();
                let f = factors(m.periods());
                assert!(
                    worth(&m, &s, &dest, &f).is_some(),
                    "seed {seed} {signed}: {s:?} {dest:?}"
                );
                for b in 0..n {
                    assert!(dest[b] == d[b] || s[b].is_some(), "seed {seed}: block {b}");
                }
                fell_back += usize::from(dest != d);
                let plain = greedy(&m.with_destinations(&d).unwrap(), &rank).unwrap();
                mined_more += usize::from(
                    s.iter().filter(|t| t.is_some()).count()
                        > plain.iter().filter(|t| t.is_some()).count(),
                );
                if signed {
                    continue;
                }
                for b in 0..n {
                    let Some(from) = m
                        .requires(b)
                        .unwrap()
                        .iter()
                        .try_fold(0, |e, &q| s[q as usize].map(|t| e.max(t as usize)))
                    else {
                        continue;
                    };
                    if s[b].is_some() || rank[b].is_none() {
                        continue;
                    }
                    for t in from..m.periods() {
                        for x in 0..m.destinations() {
                            let (mut trial, mut td) = (s.clone(), dest.clone());
                            trial[b] = Some(t as u32);
                            td[b] = x as u32;
                            assert!(
                                worth(&m, &trial, &td, &f).is_none(),
                                "seed {seed}: block {b} fits destination {x} in period {t}"
                            );
                        }
                    }
                }
            }
        }
        assert!(
            fell_back > 50 && mined_more > 50,
            "{fell_back} {mined_more}"
        );
    }

    /// [`greedy_from`] on random models, with and without a signed resource, under random releases: feasible by the
    /// definition, no block mined before its release, a destination changed only for a block it mined, and with every
    /// release 0 the same as [`super::greedy`].
    #[test]
    fn released_greedy_is_feasible_and_mines_nothing_before_its_release() {
        let mut moved = 0;
        for seed in 0..300 {
            for m in [random_model(seed), random_signed_model(seed)] {
                let mut r = lcg(seed ^ 0x7e1e);
                let n = m.blocks();
                let d: Vec<u32> = (0..n).map(|_| r(m.destinations() as u64) as u32).collect();
                let rank: Vec<Option<i64>> =
                    (0..n).map(|_| (r(5) > 0).then(|| r(10) as i64)).collect();
                let release: Vec<u32> = (0..n).map(|_| r(m.periods() as u64) as u32).collect();
                let (s, dest) = super::greedy_from(&m, &rank, &d, &release).unwrap();
                let f = factors(m.periods());
                assert!(
                    worth(&m, &s, &dest, &f).is_some(),
                    "seed {seed}: {s:?} {dest:?}"
                );
                for b in 0..n {
                    assert!(
                        s[b].is_none_or(|t| t >= release[b]),
                        "seed {seed}: block {b} mined before its release"
                    );
                    assert!(dest[b] == d[b] || s[b].is_some(), "seed {seed}: block {b}");
                }
                let plain = super::greedy(&m, &rank, &d).unwrap();
                assert_eq!(
                    super::greedy_from(&m, &rank, &d, &vec![0; n]).unwrap(),
                    plain,
                    "seed {seed}"
                );
                moved += usize::from(s != plain.0);
            }
        }
        assert!(moved > 50, "{moved}");
    }

    #[test]
    fn a_release_keeps_a_blend_partner_for_a_later_period() {
        // Destination 1 is the mill: two blocks a period (resource 0), and a blend with limit 0 (resource 1). Blocks 0
        // and 1 are below the target (−1, worth 1), blocks 2 and 3 above it (+1, worth 4). The dump takes anything.
        let blend = "NAME: release\nTYPE: PCPSP\nNBLOCKS: 4\nNPERIODS: 2\nNDESTINATIONS: 2\nNRESOURCE_SIDE_CONSTRAINTS: 2\nNGENERAL_SIDE_CONSTRAINTS: 0\nDISCOUNT_RATE: 0\nRESOURCE_CONSTRAINT_LIMITS:\n0 0 L 2\n0 1 L 2\n1 0 L 0\n1 1 L 0\nOBJECTIVE_FUNCTION:\n0 -1 1\n1 -1 1\n2 -1 4\n3 -1 4\nRESOURCE_CONSTRAINT_COEFFICIENTS:\n0 1 0 1\n0 1 1 -1\n1 1 0 1\n1 1 1 -1\n2 1 0 1\n2 1 1 1\n3 1 0 1\n3 1 1 1\nEOF\n";
        let m = parse(blend, "0 0\n1 0\n2 0\n3 0\n", 0, Rounding::Refuse, 0).unwrap();
        let rank = [Some(0), Some(1), Some(2), Some(3)];
        let d = [1, 1, 1, 1];
        // Earliest first, both blocks below the target fill period 0's mill; the blocks above it then find the mill
        // full in period 0 and nothing to blend with in period 1, so both go to the dump.
        let (s, dest) = super::greedy(&m, &rank, &d).unwrap();
        assert_eq!(
            (s.clone(), dest.clone()),
            (vec![Some(0); 4], vec![1, 1, 0, 0])
        );
        let c = check(&m.with_destinations(&dest).unwrap(), &s).unwrap();
        assert_eq!(c.period_values, vec![0, 0]);
        // Released to period 1, block 1 waits there for block 3, and every block is milled.
        let (s, dest) = super::greedy_from(&m, &rank, &d, &[0, 1, 0, 1]).unwrap();
        assert_eq!(
            (s.clone(), dest.clone()),
            (vec![Some(0), Some(1), Some(0), Some(1)], vec![1; 4])
        );
        let c = check(&m.with_destinations(&dest).unwrap(), &s).unwrap();
        assert!(c.feasible());
        assert_eq!(
            (c.period_values, c.used),
            (vec![5, 5], vec![vec![2, 2], vec![0, 0]])
        );
    }

    /// [`destinations`] on greedy schedules of random models, with and without a signed resource. The result is
    /// feasible by the definition and never worth less. In each period it is worth at most the best whole-block
    /// destinations for the blocks mined there, found by enumeration. A second call moves nothing. Reports how many
    /// periods were left at that best, before and after, and how many Price moves were made.
    #[test]
    fn destinations_with_price_are_feasible_monotone_and_bounded_per_period() {
        let (mut priced, mut periods, mut best_before, mut best_after) = (0, 0, 0, 0);
        for seed in 0..300 {
            for m in [random_model(seed), random_signed_model(seed)] {
                let mut r = lcg(seed ^ 0xD0D0);
                let (n, dn) = (m.blocks(), m.destinations());
                let d: Vec<u32> = (0..n).map(|_| r(dn as u64) as u32).collect();
                let rank: Vec<Option<i64>> = (0..n).map(|_| Some(r(10) as i64)).collect();
                let (s, start) = super::greedy(&m, &rank, &d).unwrap();
                let f = factors(m.periods());
                let mut dest = start.clone();
                let moves = destinations(&m, &s, &mut dest, &f).unwrap();
                let before = worth(&m, &s, &start, &f).unwrap();
                let after = worth(&m, &s, &dest, &f);
                assert!(
                    after.is_some_and(|a| a >= before),
                    "seed {seed}: {after:?} against {before}"
                );
                priced += moves.priced;
                for t in 0..m.periods() {
                    let mined: Vec<usize> = (0..n).filter(|&b| s[b] == Some(t as u32)).collect();
                    // Other periods keep `dest`, so the whole schedule's value ranks period t's choices.
                    let best = (0..dn.pow(mined.len() as u32))
                        .filter_map(|mut code| {
                            let mut trial = dest.clone();
                            for &b in &mined {
                                trial[b] = (code % dn) as u32;
                                code /= dn;
                            }
                            worth(&m, &s, &trial, &f)
                        })
                        .max()
                        .unwrap();
                    let mut was = dest.clone();
                    for &b in &mined {
                        was[b] = start[b];
                    }
                    let (now, then) = (after.unwrap(), worth(&m, &s, &was, &f).unwrap());
                    assert!(now <= best, "seed {seed} period {t}: {now} above {best}");
                    periods += 1;
                    best_before += usize::from(then == best);
                    best_after += usize::from(now == best);
                }
                let mut again = dest.clone();
                assert_eq!(
                    destinations(&m, &s, &mut again, &f).unwrap(),
                    DestinationMoves::default(),
                    "seed {seed}: a second call moved"
                );
            }
        }
        println!(
            "{{\"bead\":\"bd-rbxh\",\"periods\":{periods},\"at_best_before\":{best_before},\"at_best_after\":{best_after},\"priced\":{priced}}}"
        );
        assert!(priced > 0 && best_after > best_before);
    }

    /// The same with one more resource whose uses may be negative.
    #[test]
    fn improve_keeps_every_limit_with_signed_uses() {
        let (mut improved, mut optimal_before, mut optimal_after) = (0, 0, 0);
        let cases = 300;
        for seed in 0..cases {
            let (before, after, best, _) = improve_case(&random_signed_model(seed), seed);
            improved += usize::from(after > before);
            optimal_before += usize::from(before == best);
            optimal_after += usize::from(after == best);
        }
        println!(
            "{{\"bead\":\"bd-ixfw\",\"cases\":{cases},\"improved\":{improved},\"optimal_greedy\":{optimal_before},\"optimal_after\":{optimal_after}}}"
        );
        assert!(improved > 0 && optimal_after > optimal_before);
    }

    #[test]
    fn bad_inputs_refuse() {
        let m = parse(PCPSP, PREC, 1, Rounding::Refuse, 0).unwrap();
        let f = factors(2);
        let s = vec![Some(0), Some(0), Some(1)];
        // Two mill blocks in period 0 would exceed the mill.
        assert!(improve(&m, &[Some(0), Some(1), Some(1)], &[0, 1, 1], &f, 10).is_err());
        assert!(improve(&m, &s, &[0, 0], &f, 10).is_err());
        assert!(improve(&m, &s, &[0, 0, 1], &f[..1], 10).is_err());
        assert!(improve(&m, &s, &[0, 0, 1], &[1, 2], 10).is_err());
        let mut d = vec![0, 0, 1];
        assert!(destinations(&m, &s, &mut d, &f[..1]).is_err());
        let i = improve(&m, &s, &[0, 0, 0], &f, 0).unwrap();
        assert_eq!((i.cycles, i.before, i.after), (0, i.before, i.before));
        let rank = [Some(0), Some(1), Some(2)];
        assert!(super::greedy_from(&m, &rank, &[0, 0, 1], &[0, 0]).is_err());
        assert!(super::greedy_from(&m, &rank, &[0, 0, 1], &[0, 2, 0]).is_err());
        assert!(super::greedy_from(&m, &rank, &[0, 0, 1], &[0, 1, 1]).is_ok());
    }
}
