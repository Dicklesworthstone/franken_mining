//! The power study and the kill report (bd-4wvl.7, BD-340.S7).
//!
//! - **Injections** ([`Injection`]) change one printed value of a clean [`Generated`] table: its sign flipped; two
//!   adjacent, different digits of its count of steps swapped; a copy of the nearest other cell with the same
//!   step and another value; a share target printed at another share of [`SHARES`]; the value of a second
//!   vintage ([`generate_vintage`]); or ±1 to ±8 steps. The cell is uniform over the cells the kind applies to; a
//!   table with none is skipped and counted.
//! - **Bins** of |error| ÷ slack ([`BINS`]: [0, ½), [½, 1), [1, 2), [2, 4), ≥ 4; DESIGN, the owner may change
//!   them). The error is the change to the printed value. The slack is the sum of half-steps over the cells of
//!   the smallest identity naming the cell; a ratio's or a weighted mean's cells carry different units, so its
//!   bin is only a scale.
//! - **Detection.** A table is decided under its bracket ([`Generated::bracket`]: `Closed`, or `Truncate` for a
//!   truncated table) with its own construction: the construction is registry data, the tie rule is not known. The joint detector ([`joint_detects`]) solves the whole table and detects
//!   when some component is `Infeasible`. The baseline ([`baseline_detects`]) is today's single-identity check:
//!   each identity alone over its own cells, through the same kernel. A joint detection the baseline misses is
//!   joint-only. Alone, an identity keeps the box of a total another identity foots under `ComponentSum`, so the
//!   baseline can flag a clean table; the study counts those clean alarms apart.
//! - **Blame accuracy**, over the joint detections: the injected cell is in some correction set (`blame_hit`), and
//!   the correction sets are exactly `{injected cell}` (`blame_exact`).
//! - **Rates** are counts with their `n`; [`clopper_pearson`] gives the exact interval.
//! - **Kill report** ([`KillReport`]), K.5 row 3: "If it never finds more than the interval rule alone, it stays
//!   as BD-331's kernel and is not a product." [`KillReport::fired`] is computed, never set.
//!
//! A clean table the joint solver finds infeasible stops the study ([`PowerError::CleanConflict`]): it is a
//! soundness failure, not a data point.
//!
//! Determinism: DeterministicPerIsa (the counts are exact; the Clopper–Pearson bounds are `f64` through
//! `fmine_core::det`).

use std::collections::BTreeSet;
use std::fmt::Write as _;

use fmine_core::{PhiloxStream, RngKey, det};

use super::generator::{GenError, GenSpec, Generated, SHARES, generate, generate_vintage, round};
use super::{
    BlameOutcome, Cell, Identity, IdentityKind, Outcome, Rat, Rules, XfRefusal, blame, solve,
};

/// The bins of |error| ÷ slack, by lower edge: [0, ½), [½, 1), [1, 2), [2, 4), ≥ 4.
pub const BINS: [&str; 5] = ["[0,0.5)", "[0.5,1)", "[1,2)", "[2,4)", ">=4"];

/// The partition of every power-study key (BD-340.S7).
const PARTITION: u32 = 3407;

/// The ways one printed value is corrupted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Injection {
    SignFlip,
    Transposition,
    Neighbour,
    WrongShare,
    StaleVintage,
    Units,
}

impl Injection {
    pub const ALL: [Self; 6] = [
        Self::SignFlip,
        Self::Transposition,
        Self::Neighbour,
        Self::WrongShare,
        Self::StaleVintage,
        Self::Units,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SignFlip => "sign_flip",
            Self::Transposition => "transposition",
            Self::Neighbour => "neighbour",
            Self::WrongShare => "wrong_share",
            Self::StaleVintage => "stale_vintage",
            Self::Units => "units",
        }
    }
}

/// Why the study stopped.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PowerError {
    Gen(GenError),
    Refused(XfRefusal),
    /// The joint solver found a clean table infeasible: a soundness failure.
    CleanConflict {
        injection: Injection,
        table: u64,
    },
    /// A cell no identity names, a second vintage of another shape, or an overflow.
    Malformed(&'static str),
}

impl From<GenError> for PowerError {
    fn from(e: GenError) -> Self {
        Self::Gen(e)
    }
}

impl From<XfRefusal> for PowerError {
    fn from(e: XfRefusal) -> Self {
        Self::Refused(e)
    }
}

impl From<fmine_units::UnitsError> for PowerError {
    fn from(_: fmine_units::UnitsError) -> Self {
        Self::Malformed("exact arithmetic overflowed")
    }
}

/// A table with one printed value changed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Injected {
    pub cells: Vec<Cell>,
    /// The changed cell's locator.
    pub cell: String,
    /// |new − old| printed value.
    pub error: Rat,
    pub slack: Rat,
    /// An index into [`BINS`].
    pub bin: usize,
}

/// The digits of `m` (most significant first) and its sign.
fn digits(m: i128) -> (Vec<u8>, bool) {
    let negative = m < 0;
    let digits = m
        .unsigned_abs()
        .to_string()
        .bytes()
        .map(|b| b - b'0')
        .collect();
    (digits, negative)
}

/// The integer count of steps `printed / step`, when it is one.
fn steps_of(c: &Cell) -> Option<i128> {
    let k = c.printed.checked_div(c.step).ok()?;
    (k.den() == 1).then_some(k.num())
}

/// The positions `j` where digits `j` and `j + 1` differ.
fn swappable(c: &Cell) -> Vec<usize> {
    steps_of(c).map_or_else(Vec::new, |m| {
        let (d, _) = digits(m);
        (0..d.len().saturating_sub(1))
            .filter(|&j| d[j] != d[j + 1])
            .collect()
    })
}

/// The nearest other cell (by position: next, then previous, then two away, …) with the same step and another
/// printed value.
fn neighbour(cells: &[Cell], i: usize) -> Option<usize> {
    let c = &cells[i];
    (1..cells.len()).find_map(|d| {
        [i.checked_add(d), i.checked_sub(d)]
            .into_iter()
            .flatten()
            .filter(|&j| j < cells.len())
            .find(|&j| cells[j].step == c.step && cells[j].printed != c.printed)
    })
}

/// The share identity whose target is cell `i`: its source's index and its factor.
fn share_of(table: &Generated, i: usize) -> Option<(usize, Rat)> {
    let at = &table.cells[i].locator;
    table.identities.iter().find_map(|id| match &id.kind {
        IdentityKind::Scale {
            target,
            source,
            factor,
        } if target == at => table
            .cells
            .iter()
            .position(|c| &c.locator == source)
            .map(|s| (s, *factor)),
        _ => None,
    })
}

fn applies(kind: Injection, table: &Generated, second: Option<&Generated>, i: usize) -> bool {
    let c = &table.cells[i];
    match kind {
        Injection::SignFlip => c.printed.signum() != 0,
        Injection::Transposition => !swappable(c).is_empty(),
        Injection::Neighbour => neighbour(&table.cells, i).is_some(),
        Injection::WrongShare => share_of(table, i).is_some(),
        Injection::StaleVintage => second
            .and_then(|s| s.cells.get(i))
            .is_some_and(|s| s.printed != c.printed),
        Injection::Units => true,
    }
}

/// The slack of cell `i`: Σ half-steps over the cells of the smallest identity naming it.
fn slack(table: &Generated, i: usize) -> Result<Rat, PowerError> {
    let at = table.cells[i].locator.as_str();
    let smallest = table
        .identities
        .iter()
        .map(|id| id.locators().into_iter().collect::<BTreeSet<&str>>())
        .filter(|names| names.contains(at))
        .min_by_key(BTreeSet::len)
        .ok_or(PowerError::Malformed("a cell no identity names"))?;
    let two = Rat::from_integer(2);
    let mut s = Rat::from_integer(0);
    for c in table
        .cells
        .iter()
        .filter(|c| smallest.contains(c.locator.as_str()))
    {
        s = s.checked_add(c.step.checked_div(two)?)?;
    }
    Ok(s)
}

/// The bin of `error / slack`.
fn bin(error: Rat, slack: Rat) -> Result<usize, PowerError> {
    let r = error.checked_div(slack)?;
    let edges = [
        Rat::new(1, 2)?,
        Rat::from_integer(1),
        Rat::from_integer(2),
        Rat::from_integer(4),
    ];
    let mut b = 0;
    for e in edges {
        if r.checked_cmp(e)?.is_ge() {
            b += 1;
        }
    }
    Ok(b)
}

/// Applies `kind` at cell `i`. `choice` is a raw draw for the kind's own choice: the digit pair, the other share
/// (in [`SHARES`] order, skipping the true one and those that print the same), or the number of steps
/// (`1 + choice % 8`, negative when `choice / 8` is odd). `None` when the kind does not apply there.
pub fn inject_at(
    kind: Injection,
    table: &Generated,
    second: Option<&Generated>,
    i: usize,
    choice: u64,
) -> Result<Option<Injected>, PowerError> {
    if i >= table.cells.len() || !applies(kind, table, second, i) {
        return Ok(None);
    }
    let c = &table.cells[i];
    let pick = |n: usize| usize::try_from(choice % (n.max(1) as u64)).unwrap_or(0);
    let new = match kind {
        Injection::SignFlip => c.printed.checked_neg()?,
        Injection::Transposition => {
            let pairs = swappable(c);
            let (Some(m), Some(&j)) = (steps_of(c), pairs.get(pick(pairs.len()))) else {
                return Ok(None);
            };
            let (mut d, negative) = digits(m);
            d.swap(j, j + 1);
            let mut swapped: i128 = 0;
            for x in d {
                swapped = swapped
                    .checked_mul(10)
                    .and_then(|s| s.checked_add(i128::from(x)))
                    .ok_or(PowerError::Malformed("exact arithmetic overflowed"))?;
            }
            let swapped = if negative { -swapped } else { swapped };
            Rat::from_integer(swapped).checked_mul(c.step)?
        }
        Injection::Neighbour => match neighbour(&table.cells, i) {
            Some(j) => table.cells[j].printed,
            None => return Ok(None),
        },
        Injection::WrongShare => {
            let Some((source, factor)) = share_of(table, i) else {
                return Ok(None);
            };
            let mut others = Vec::new();
            for (n, d) in SHARES {
                let f = Rat::new(n, d)?;
                if f == factor {
                    continue;
                }
                let v = round(
                    f.checked_mul(table.truth[source])?,
                    c.step,
                    table.convention,
                )?;
                if v != c.printed {
                    others.push(v);
                }
            }
            match others.get(pick(others.len())) {
                Some(v) => *v,
                None => return Ok(None),
            }
        }
        Injection::StaleVintage => match second.and_then(|s| s.cells.get(i)) {
            Some(s) if s.locator == c.locator => s.printed,
            _ => return Err(PowerError::Malformed("a second vintage of another shape")),
        },
        Injection::Units => {
            let n = i128::from(1 + choice % 8);
            let n = if (choice / 8) % 2 == 1 { -n } else { n };
            c.printed
                .checked_add(Rat::from_integer(n).checked_mul(c.step)?)?
        }
    };
    let error = new.checked_sub(c.printed)?;
    let error = if error.signum() < 0 {
        error.checked_neg()?
    } else {
        error
    };
    if error.signum() == 0 {
        return Ok(None);
    }
    let slack = slack(table, i)?;
    let mut cells = table.cells.clone();
    cells[i].printed = new;
    cells[i].token = format!("{}/{}", new.num(), new.den());
    Ok(Some(Injected {
        cell: c.locator.clone(),
        cells,
        error,
        slack,
        bin: bin(error, slack)?,
    }))
}

/// Applies `kind` at a cell drawn uniformly from those it applies to; `None` when there is none.
pub fn inject(
    kind: Injection,
    table: &Generated,
    second: Option<&Generated>,
    rng: &mut PhiloxStream,
) -> Result<Option<Injected>, PowerError> {
    let eligible: Vec<usize> = (0..table.cells.len())
        .filter(|&i| applies(kind, table, second, i))
        .collect();
    if eligible.is_empty() {
        return Ok(None);
    }
    let k = usize::try_from(rng.next_u64() % eligible.len() as u64)
        .map_err(|_| PowerError::Malformed("exact arithmetic overflowed"))?;
    let choice = rng.next_u64();
    inject_at(kind, table, second, eligible[k], choice)
}

/// A detector: whether the printed values are found inconsistent.
pub type Detector = fn(&[Cell], &[Identity], &Rules) -> Result<bool, XfRefusal>;

/// Some component of the whole table is `Infeasible`.
pub fn joint_detects(
    cells: &[Cell],
    identities: &[Identity],
    rules: &Rules,
) -> Result<bool, XfRefusal> {
    Ok(solve(cells, identities, rules)?
        .iter()
        .any(|c| matches!(c.outcome, Outcome::Infeasible { .. })))
}

/// Some identity, alone over its own cells, is `Infeasible`: today's single-identity check.
pub fn baseline_detects(
    cells: &[Cell],
    identities: &[Identity],
    rules: &Rules,
) -> Result<bool, XfRefusal> {
    for id in identities {
        let names: BTreeSet<&str> = id.locators().into_iter().collect();
        let own: Vec<Cell> = cells
            .iter()
            .filter(|c| names.contains(c.locator.as_str()))
            .cloned()
            .collect();
        if joint_detects(&own, std::slice::from_ref(id), rules)? {
            return Ok(true);
        }
    }
    Ok(false)
}

/// `joint` detects and the baseline does not.
pub fn joint_only(
    cells: &[Cell],
    identities: &[Identity],
    rules: &Rules,
    joint: Detector,
) -> Result<bool, XfRefusal> {
    Ok(joint(cells, identities, rules)? && !baseline_detects(cells, identities, rules)?)
}

/// What blame says of an injected cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlameAccuracy {
    /// `hit`: the cell is in some correction set; `exact`: the correction sets are exactly `{cell}`.
    Blamed { hit: bool, exact: bool },
    /// No infeasible component, or blame made no claim.
    Indeterminate,
}

/// Blames the table and scores it against `cell`.
pub fn blame_accuracy(
    cells: &[Cell],
    identities: &[Identity],
    rules: &Rules,
    cell: &str,
) -> Result<BlameAccuracy, XfRefusal> {
    let found = blame(cells, identities, rules)?;
    let Some(b) = found.iter().find(|b| b.cells.iter().any(|c| c == cell)) else {
        return Ok(BlameAccuracy::Indeterminate);
    };
    Ok(match &b.outcome {
        BlameOutcome::Blamed(b) => BlameAccuracy::Blamed {
            hit: b
                .corrections
                .iter()
                .any(|s| s.cells.iter().any(|c| c == cell)),
            exact: b.corrections.len() == 1 && b.corrections[0].cells == [cell],
        },
        BlameOutcome::Indeterminate { .. } => BlameAccuracy::Indeterminate,
    })
}

/// Some infeasible component's blame has a correction set of one cell.
pub fn size1_blame(
    cells: &[Cell],
    identities: &[Identity],
    rules: &Rules,
) -> Result<bool, XfRefusal> {
    Ok(blame(cells, identities, rules)?
        .iter()
        .any(|b| match &b.outcome {
            BlameOutcome::Blamed(b) => b.corrections.iter().any(|s| s.cells.len() == 1),
            BlameOutcome::Indeterminate { .. } => false,
        }))
}

/// The two-sided `1 − alpha` Clopper–Pearson interval of `k` successes in `n`: the `p` where P(X ≥ k) and
/// P(X ≤ k) equal `alpha/2`, by bisection on the exact binomial tails (summed in log space). `(0, 1)` for
/// `n = 0`; `None` for `k > n` or `alpha` outside (0, 1).
#[must_use]
pub fn clopper_pearson(k: u64, n: u64, alpha: f64) -> Option<(f64, f64)> {
    if k > n || alpha.is_nan() || alpha <= 0.0 || alpha >= 1.0 {
        return None;
    }
    if n == 0 {
        return Some((0.0, 1.0));
    }
    let half = alpha / 2.0;
    // ln C(n, i) for i = 0..=n.
    let mut lnc = Vec::with_capacity(usize::try_from(n).ok()?.saturating_add(1));
    lnc.push(0.0);
    for i in 1..=n {
        let prev = lnc.last().copied().unwrap_or(0.0);
        lnc.push(prev + det::ln((n - i + 1) as f64) - det::ln(i as f64));
    }
    // P(lo ≤ X ≤ hi) at p.
    let tail = |p: f64, lo: u64, hi: u64| -> f64 {
        let (lp, lq) = (det::ln(p), det::ln(1.0 - p));
        let terms: Vec<f64> = (lo..=hi)
            .zip(lnc.iter().skip(usize::try_from(lo).unwrap_or(usize::MAX)))
            .map(|(i, c)| c + i as f64 * lp + (n - i) as f64 * lq)
            .collect();
        let m = terms.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        if m == f64::NEG_INFINITY {
            return 0.0;
        }
        det::exp(m) * terms.iter().map(|t| det::exp(t - m)).sum::<f64>()
    };
    // The p in (0, 1) where `f` crosses `half`; `rising` when `f` increases with p.
    let bisect = |f: &dyn Fn(f64) -> f64, rising: bool| -> f64 {
        let (mut lo, mut hi) = (0.0_f64, 1.0_f64);
        for _ in 0..200 {
            let mid = 0.5 * (lo + hi);
            if mid <= lo || mid >= hi {
                break;
            }
            if (f(mid) < half) == rising {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        0.5 * (lo + hi)
    };
    let lower = if k == 0 {
        0.0
    } else {
        bisect(&|p| tail(p, k, n), true)
    };
    let upper = if k == n {
        1.0
    } else {
        bisect(&|p| tail(p, 0, k), false)
    };
    Some((lower, upper))
}

/// The counts of one injection kind in one bin.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PowerCell {
    /// Injections in the bin.
    pub n: u64,
    pub joint: u64,
    pub baseline: u64,
    pub joint_only: u64,
    /// Injections with no infeasible component and some `Indeterminate` one.
    pub joint_indeterminate: u64,
    /// Over the `joint` detections.
    pub blame_hit: u64,
    pub blame_exact: u64,
    pub blame_indeterminate: u64,
}

/// One injection kind's row of the study.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KindRow {
    pub injection: Injection,
    /// Clean tables drawn, those with no cell the kind applies to (or whose change printed the same value), and
    /// the clean tables the baseline flags.
    pub tables: u64,
    pub skipped: u64,
    pub clean_baseline_alarms: u64,
    pub bins: [PowerCell; 5],
}

impl KindRow {
    #[must_use]
    pub fn joint_only(&self) -> u64 {
        self.bins.iter().map(|c| c.joint_only).sum()
    }

    #[must_use]
    pub fn injections(&self) -> u64 {
        self.bins.iter().map(|c| c.n).sum()
    }
}

/// What to run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PowerSpec {
    pub seed: u64,
    /// Injections per kind.
    pub per_kind: u64,
    pub generator: GenSpec,
    /// Clean tables drawn per kind before the kind stops short of `per_kind`.
    pub max_tables: u64,
}

/// The study.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PowerTable {
    pub seed: u64,
    pub rows: Vec<KindRow>,
}

fn key(seed: u64, injection: Injection, var_group: u32, table: u64) -> Result<RngKey, PowerError> {
    let position = Injection::ALL
        .iter()
        .position(|k| *k == injection)
        .and_then(|p| u32::try_from(p).ok())
        .ok_or(PowerError::Malformed("an unknown injection"))?;
    Ok(RngKey {
        seed_root: seed,
        partition: PARTITION,
        domain: position,
        var_group,
        realization: 0,
        brick: table,
    })
}

/// Runs the study with `joint` as the joint detector ([`joint_detects`] for the product; a test substitutes the
/// baseline to show the kill report then fires).
pub fn power_study(spec: &PowerSpec, joint: Detector) -> Result<PowerTable, PowerError> {
    let mut rows = Vec::new();
    for injection in Injection::ALL {
        let mut row = KindRow {
            injection,
            tables: 0,
            skipped: 0,
            clean_baseline_alarms: 0,
            bins: [PowerCell::default(); 5],
        };
        let mut rng = PhiloxStream::new(key(spec.seed, injection, 1, 0)?);
        while row.injections() < spec.per_kind && row.tables < spec.max_tables {
            let k = key(spec.seed, injection, 0, row.tables)?;
            row.tables += 1;
            let table = generate(k, &spec.generator)?;
            let rules = table.rules(table.bracket());
            if joint_detects(&table.cells, &table.identities, &rules)? {
                return Err(PowerError::CleanConflict {
                    injection,
                    table: row.tables - 1,
                });
            }
            if baseline_detects(&table.cells, &table.identities, &rules)? {
                row.clean_baseline_alarms += 1;
            }
            let second = if injection == Injection::StaleVintage {
                Some(generate_vintage(k, &spec.generator, 1)?)
            } else {
                None
            };
            let Some(inj) = inject(injection, &table, second.as_ref(), &mut rng)? else {
                row.skipped += 1;
                continue;
            };
            let cell = &mut row.bins[inj.bin];
            cell.n += 1;
            let ids = &table.identities;
            let found = joint(&inj.cells, ids, &rules)?;
            let base = baseline_detects(&inj.cells, ids, &rules)?;
            cell.baseline += u64::from(base);
            if found {
                cell.joint += 1;
                cell.joint_only += u64::from(!base);
                match blame_accuracy(&inj.cells, ids, &rules, &inj.cell)? {
                    BlameAccuracy::Blamed { hit, exact } => {
                        cell.blame_hit += u64::from(hit);
                        cell.blame_exact += u64::from(exact);
                    }
                    BlameAccuracy::Indeterminate => cell.blame_indeterminate += 1,
                }
            } else if solve(&inj.cells, ids, &rules)?
                .iter()
                .any(|c| matches!(c.outcome, Outcome::Indeterminate { .. }))
            {
                cell.joint_indeterminate += 1;
            }
        }
        rows.push(row);
    }
    Ok(PowerTable {
        seed: spec.seed,
        rows,
    })
}

fn cp(k: u64, n: u64) -> (f64, f64) {
    clopper_pearson(k, n, 0.05).unwrap_or((f64::NAN, f64::NAN))
}

impl PowerTable {
    /// Joint-only detections by injection kind.
    #[must_use]
    pub fn joint_only_by_kind(&self) -> Vec<(String, u64)> {
        self.rows
            .iter()
            .map(|r| (r.injection.as_str().to_string(), r.joint_only()))
            .collect()
    }

    /// One `{"kind":"power",…}` line per kind and bin, then one `{"kind":"power_tables",…}` line per kind. Every
    /// rate is a count with its `n`; `cp_*` are 95% Clopper–Pearson bounds (joint, then baseline).
    #[must_use]
    pub fn log_lines(&self) -> Vec<String> {
        let mut out = Vec::new();
        for r in &self.rows {
            for (b, c) in r.bins.iter().enumerate() {
                let (lo, hi) = cp(c.joint, c.n);
                let (blo, bhi) = cp(c.baseline, c.n);
                let mut s = String::new();
                let _ = write!(
                    s,
                    "{{\"kind\":\"power\",\"injection\":\"{}\",\"bin\":\"{}\",\"n\":{},\"joint_detected\":{},\
                     \"baseline_detected\":{},\"joint_only\":{},\"cp_lo\":{lo:.4},\"cp_hi\":{hi:.4},\
                     \"baseline_cp_lo\":{blo:.4},\"baseline_cp_hi\":{bhi:.4},\"blame_n\":{},\"blame_hit\":{},\
                     \"blame_exact\":{},\"blame_indeterminate\":{},\"joint_indeterminate\":{},\"seed\":{}}}",
                    r.injection.as_str(),
                    BINS[b],
                    c.n,
                    c.joint,
                    c.baseline,
                    c.joint_only,
                    c.joint,
                    c.blame_hit,
                    c.blame_exact,
                    c.blame_indeterminate,
                    c.joint_indeterminate,
                    self.seed
                );
                out.push(s);
            }
        }
        for r in &self.rows {
            out.push(format!(
                "{{\"kind\":\"power_tables\",\"injection\":\"{}\",\"tables\":{},\"skipped\":{},\
                 \"injections\":{},\"clean_baseline_alarms\":{},\"seed\":{}}}",
                r.injection.as_str(),
                r.tables,
                r.skipped,
                r.injections(),
                r.clean_baseline_alarms,
                self.seed
            ));
        }
        out
    }
}

/// K.5 row 3's measures (BD-340.S7).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KillReport {
    /// Joint-only detections by source: each injection kind of the study, and each named known answer (COD-02).
    pub joint_only_detections: Vec<(String, u64)>,
    /// Whether blame gives a one-cell correction set on each known answer; `None` while its fixture has not
    /// landed (E144 waits on S5).
    pub size1: Vec<(String, Option<bool>)>,
    /// Whether discovery identifies a construction, convention, share, deck or unit on each known answer; `None`
    /// while its discovery has not landed (E147 waits on S4 part 4).
    pub discoveries: Vec<(String, Option<bool>)>,
    /// Beads whose production code reads an implied interval.
    pub implied_consumers: Vec<String>,
}

impl KillReport {
    /// True iff all four measures are empty: no joint-only detection, no one-cell blame, no discovery and no
    /// consumer. An entry still `None` counts as empty (DESIGN's reading of "nothing beyond"; the owner confirms
    /// it before S8).
    #[must_use]
    pub fn fired(&self) -> bool {
        self.joint_only_detections.iter().all(|(_, n)| *n == 0)
            && self.size1.iter().all(|(_, b)| *b != Some(true))
            && self.discoveries.iter().all(|(_, b)| *b != Some(true))
            && self.implied_consumers.is_empty()
    }

    /// The known answers not yet filled.
    #[must_use]
    pub fn pending(&self) -> Vec<&str> {
        self.size1
            .iter()
            .chain(&self.discoveries)
            .filter(|(_, b)| b.is_none())
            .map(|(e, _)| e.as_str())
            .collect()
    }

    /// `{"kind":"kill_report",…}`; `elapsed_s` is the caller's wall time.
    #[must_use]
    pub fn log_line(&self, seed: u64, elapsed_s: f64) -> String {
        let counts: Vec<String> = self
            .joint_only_detections
            .iter()
            .map(|(k, n)| format!("[\"{k}\",{n}]"))
            .collect();
        let flags = |xs: &[(String, Option<bool>)]| -> String {
            xs.iter()
                .map(|(k, b)| {
                    let v = b.map_or_else(|| "null".to_string(), |b| b.to_string());
                    format!("[\"{k}\",{v}]")
                })
                .collect::<Vec<_>>()
                .join(",")
        };
        let consumers: Vec<String> = self
            .implied_consumers
            .iter()
            .map(|b| format!("\"{b}\""))
            .collect();
        let pending: Vec<String> = self.pending().iter().map(|e| format!("\"{e}\"")).collect();
        format!(
            "{{\"kind\":\"kill_report\",\"joint_only_detections\":[{}],\"size1\":[{}],\"discoveries\":[{}],\
             \"implied_consumers\":[{}],\"fired\":{},\"pending\":[{}],\"seed\":{seed},\"elapsed_s\":{elapsed_s:.1}}}",
            counts.join(","),
            flags(&self.size1),
            flags(&self.discoveries),
            consumers.join(","),
            self.fired(),
            pending.join(",")
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::exact_feas::generator::{Block, SUM_FAMILY};
    use crate::exact_feas::{Closure, Construction, Convention, Scope};

    fn q(n: i128, d: i128) -> Rat {
        Rat::new(n, d).expect("rational")
    }

    fn int(n: i128) -> Rat {
        Rat::from_integer(n)
    }

    fn cell(locator: &str, printed: Rat, step: Rat) -> Cell {
        Cell {
            locator: locator.to_string(),
            sha256: "0".repeat(64),
            token: format!("{}/{}", printed.num(), printed.den()),
            printed,
            step,
            scope: Scope::Visible,
        }
    }

    fn sum(id: &str, family: &str, total: &str, lines: &[&str]) -> Identity {
        Identity {
            id: id.to_string(),
            family: family.to_string(),
            closure: Closure::Definitional,
            kind: IdentityKind::Sum {
                total: total.to_string(),
                lines: lines.iter().map(|l| (*l).to_string()).collect(),
                signs: vec![1; lines.len()],
            },
        }
    }

    fn rounded_once() -> Rules {
        Rules::new(Convention::Closed).with(SUM_FAMILY, Construction::RoundedOnce)
    }

    /// A hand-made table in E113's shape, half-up at step 1: Q1 (true `q1`), Q2 249.9, H1 = Q1 + Q2, and Q1
    /// attributable at 0.615.
    fn handmade(q1: Rat) -> Generated {
        let one = int(1);
        let q2 = q(2499, 10);
        let truth = vec![
            q1,
            q2,
            q1.checked_add(q2).expect("add"),
            q(123, 200).checked_mul(q1).expect("mul"),
        ];
        let names = ["h:Q1", "h:Q2", "h:H1", "h:Q1att"];
        let cells = names
            .iter()
            .zip(&truth)
            .map(|(n, x)| cell(n, round(*x, one, Convention::HalfUp).expect("rounds"), one))
            .collect();
        Generated {
            block: Block::Share,
            cells,
            identities: vec![
                sum("H1", SUM_FAMILY, "h:H1", &["h:Q1", "h:Q2"]),
                Identity {
                    id: "share".to_string(),
                    family: "share".to_string(),
                    closure: Closure::Definitional,
                    kind: IdentityKind::Scale {
                        target: "h:Q1att".to_string(),
                        source: "h:Q1".to_string(),
                        factor: q(123, 200),
                    },
                },
            ],
            truth,
            construction: Construction::RoundedOnce,
            convention: Convention::HalfUp,
            tie_table: false,
        }
    }

    #[test]
    fn test_clopper_pearson_known_answers() {
        // Plan K.5 row 20: 0 of 5 gives an upper bound of 1 − 0.025^(1/5) ≈ 0.5218; 5 of 5 a lower bound of
        // 0.025^(1/5) ≈ 0.4782.
        let root = det::exp(det::ln(0.025) / 5.0);
        let (lo, hi) = clopper_pearson(0, 5, 0.05).expect("an interval");
        assert_eq!(lo, 0.0);
        assert!((hi - (1.0 - root)).abs() < 1e-9, "{hi}");
        assert!((hi - 0.5218).abs() < 1e-4);
        let (lo, hi) = clopper_pearson(5, 5, 0.05).expect("an interval");
        assert!((lo - root).abs() < 1e-9, "{lo}");
        assert!((lo - 0.4782).abs() < 1e-4);
        assert_eq!(hi, 1.0);
        // R's binom.test(3, 10): 0.06673951 to 0.65245285.
        let (lo, hi) = clopper_pearson(3, 10, 0.05).expect("an interval");
        assert!((lo - 0.066_739_51).abs() < 1e-7, "{lo}");
        assert!((hi - 0.652_452_85).abs() < 1e-7, "{hi}");
        // The interval of k in n mirrors that of n − k, and holds k/n.
        for (k, n) in [(37, 100), (1, 1000), (999, 1000), (500, 1000)] {
            let (lo, hi) = clopper_pearson(k, n, 0.05).expect("an interval");
            let (mlo, mhi) = clopper_pearson(n - k, n, 0.05).expect("an interval");
            assert!((lo - (1.0 - mhi)).abs() < 1e-9 && (hi - (1.0 - mlo)).abs() < 1e-9);
            let p = k as f64 / n as f64;
            assert!(lo < p && p < hi, "{k}/{n}");
        }
        assert_eq!(clopper_pearson(0, 0, 0.05), Some((0.0, 1.0)));
        assert_eq!(clopper_pearson(6, 5, 0.05), None);
        for alpha in [0.0, 1.0, f64::NAN, -0.1] {
            assert_eq!(clopper_pearson(1, 5, alpha), None);
        }
    }

    /// Each kind on a hand-made table, at a cell where it changes an identity's verdict: every one is detected.
    #[test]
    fn test_each_injection_kind_detected_on_handmade_case() {
        let table = handmade(q(3002, 10));
        let stale = handmade(q(2804, 10));
        let r = rounded_once();
        let printed: Vec<Rat> = table.cells.iter().map(|c| c.printed).collect();
        assert_eq!(printed, vec![int(300), int(250), int(550), int(185)]);
        assert!(!joint_detects(&table.cells, &table.identities, &r).expect("solves"));
        let cases = [
            (Injection::SignFlip, 0, 0, int(-300)),
            (Injection::Transposition, 0, 0, int(30)),
            (Injection::Neighbour, 0, 0, int(250)),
            // 0.45·300.2 = 135.09, the first other share.
            (Injection::WrongShare, 3, 0, int(135)),
            (Injection::StaleVintage, 0, 0, int(280)),
            // choice 7: +8 steps.
            (Injection::Units, 0, 7, int(308)),
        ];
        for (kind, at, choice, want) in cases {
            let inj = inject_at(kind, &table, Some(&stale), at, choice)
                .expect("injects")
                .expect("applies");
            assert_eq!(inj.cells[at].printed, want, "{kind:?}");
            assert_eq!(inj.cell, table.cells[at].locator);
            assert!(
                joint_detects(&inj.cells, &table.identities, &r).expect("solves"),
                "{kind:?} is not detected"
            );
        }
        // Q1 is in the share (two cells) and the half year (three): its slack is 1, so a 50 change is ≥ 4.
        let inj = inject_at(Injection::Neighbour, &table, None, 0, 0)
            .expect("injects")
            .expect("applies");
        assert_eq!((inj.error, inj.slack, inj.bin), (int(50), int(1), 4));
        // Q1 alone is blamed: with it disregarded both identities hold.
        assert_eq!(
            blame_accuracy(&inj.cells, &table.identities, &r, &inj.cell).expect("blames"),
            BlameAccuracy::Blamed {
                hit: true,
                exact: true
            }
        );
        // A kind that does not apply gives none: no share has Q1 as its target, and a stale vintage needs one.
        assert_eq!(
            inject_at(Injection::WrongShare, &table, None, 0, 0),
            Ok(None)
        );
        assert_eq!(
            inject_at(Injection::StaleVintage, &table, None, 0, 0),
            Ok(None)
        );
        assert_eq!(inject_at(Injection::Units, &table, None, 9, 0), Ok(None));
    }

    /// COD-02: a total of 10 broken down as 5.20 + 5.20 and as 4.80 + 4.80. Each breakdown alone fits the total;
    /// together they cannot. The joint solver detects it and the single-identity baseline does not.
    #[test]
    fn test_baseline_misses_cod02_joint_conflict() {
        let cent = q(1, 100);
        let cells = vec![
            cell("T", int(10), int(1)),
            cell("a1", q(26, 5), cent),
            cell("a2", q(26, 5), cent),
            cell("b1", q(24, 5), cent),
            cell("b2", q(24, 5), cent),
        ];
        let ids = vec![
            sum("A", SUM_FAMILY, "T", &["a1", "a2"]),
            sum("B", SUM_FAMILY, "T", &["b1", "b2"]),
        ];
        let r = rounded_once();
        assert!(joint_detects(&cells, &ids, &r).expect("solves"));
        assert!(!baseline_detects(&cells, &ids, &r).expect("solves"));
        assert!(joint_only(&cells, &ids, &r, joint_detects).expect("solves"));
        assert!(!joint_only(&cells, &ids, &r, baseline_detects).expect("solves"));
    }

    /// A lone sum 100 + 200 + 300 = 600 with 200 printed 208: blame hits the injected line, but every one of the
    /// four cells alone restores the sum, so the correction is not exact. One identity cannot say which cell.
    #[test]
    fn test_blame_hits_injected_cell_on_single_sum() {
        let one = int(1);
        let truth = vec![q(1002, 10), q(1999, 10), q(3003, 10), q(6004, 10)];
        let names = ["l1", "l2", "l3", "T"];
        let cells: Vec<Cell> = names
            .iter()
            .zip(&truth)
            .map(|(n, x)| cell(n, round(*x, one, Convention::HalfUp).expect("rounds"), one))
            .collect();
        let table = Generated {
            block: Block::Sum,
            cells,
            identities: vec![sum("s", SUM_FAMILY, "T", &["l1", "l2", "l3"])],
            truth,
            construction: Construction::RoundedOnce,
            convention: Convention::HalfUp,
            tie_table: false,
        };
        let r = rounded_once();
        let inj = inject_at(Injection::Units, &table, None, 1, 7)
            .expect("injects")
            .expect("applies");
        assert_eq!(
            (inj.cells[1].printed, inj.slack, inj.bin),
            (int(208), int(2), 4)
        );
        assert!(joint_detects(&inj.cells, &table.identities, &r).expect("solves"));
        assert_eq!(
            blame_accuracy(&inj.cells, &table.identities, &r, "l2").expect("blames"),
            BlameAccuracy::Blamed {
                hit: true,
                exact: false
            }
        );
        let found = blame(&inj.cells, &table.identities, &r).expect("blames");
        let BlameOutcome::Blamed(b) = &found[0].outcome else {
            panic!("blamed");
        };
        assert_eq!(b.corrections.len(), 4);
    }

    fn empty_report() -> KillReport {
        KillReport {
            joint_only_detections: Injection::ALL
                .iter()
                .map(|k| (k.as_str().to_string(), 0))
                .chain([("COD-02".to_string(), 0)])
                .collect(),
            size1: vec![
                ("E113".to_string(), Some(false)),
                ("E114".to_string(), Some(false)),
                ("E144".to_string(), None),
            ],
            discoveries: vec![
                ("E147".to_string(), None),
                ("E698".to_string(), Some(false)),
                ("E815".to_string(), Some(false)),
            ],
            implied_consumers: Vec::new(),
        }
    }

    #[test]
    fn test_kill_report_fires_when_all_measures_empty() {
        let r = empty_report();
        assert!(r.fired());
        assert_eq!(r.pending(), ["E144", "E147"]);
        let line = r.log_line(3407, 1.0);
        assert!(
            line.contains("\"fired\":true") && line.contains("[\"E144\",null]"),
            "{line}"
        );
        let all_empty = KillReport {
            joint_only_detections: Vec::new(),
            size1: Vec::new(),
            discoveries: Vec::new(),
            implied_consumers: Vec::new(),
        };
        assert!(all_empty.fired());
        // Any one measure that is not empty keeps the kernel.
        let mut a = empty_report();
        a.joint_only_detections[3].1 = 1;
        let mut b = empty_report();
        b.size1[1].1 = Some(true);
        let mut c = empty_report();
        c.discoveries[2].1 = Some(true);
        let mut d = empty_report();
        d.implied_consumers.push("BD-331".to_string());
        for r in [a, b, c, d] {
            assert!(!r.fired(), "{r:?}");
            assert!(r.log_line(3407, 1.0).contains("\"fired\":false"));
        }
    }

    /// E113 (Carlin, 61.5%): Q1 283 at 100% and 173 attributable, Q2 280, H1 561. Blame finds Q1's 100% figure
    /// alone, a one-cell correction, so the report does not fire.
    #[test]
    fn test_kill_report_not_fired_by_e113_size1() {
        let one = int(1);
        let cells = vec![
            cell("Q1:QTD!B5", int(283), one),
            cell("Q1:QTD!C5", int(173), one),
            cell("Q2:QTD!B5", int(280), one),
            cell("Q2:YTD!B5", int(561), one),
        ];
        let ids = vec![
            sum(
                "carlin_h1",
                "half_year",
                "Q2:YTD!B5",
                &["Q1:QTD!B5", "Q2:QTD!B5"],
            ),
            Identity {
                id: "carlin_q1_share".to_string(),
                family: "share".to_string(),
                closure: Closure::Definitional,
                kind: IdentityKind::Scale {
                    target: "Q1:QTD!C5".to_string(),
                    source: "Q1:QTD!B5".to_string(),
                    factor: q(123, 200),
                },
            },
        ];
        let r = Rules::new(Convention::Closed).with("half_year", Construction::RoundedOnce);
        let size1 = size1_blame(&cells, &ids, &r).expect("blames");
        assert!(size1);
        let report = KillReport {
            joint_only_detections: Vec::new(),
            size1: vec![("E113".to_string(), Some(size1))],
            discoveries: Vec::new(),
            implied_consumers: Vec::new(),
        };
        assert!(!report.fired());
        // With Q1 printed 281 the component is feasible: no blame, so no one-cell correction.
        let mut fixed = cells.clone();
        fixed[0].printed = int(281);
        assert!(!size1_blame(&fixed, &ids, &r).expect("blames"));
    }

    fn quick(per_kind: u64) -> PowerSpec {
        PowerSpec {
            seed: 3407,
            per_kind,
            generator: GenSpec::all(),
            max_tables: 100 * per_kind,
        }
    }

    /// 10² injections per kind: every kind reaches its count, and every count is bounded by its denominator.
    #[test]
    fn test_power_study_quick() {
        let study = power_study(&quick(100), joint_detects).expect("the study runs");
        for line in study.log_lines() {
            println!("{line}");
        }
        assert_eq!(study.rows.len(), Injection::ALL.len());
        for r in &study.rows {
            assert_eq!(r.injections(), 100, "{:?}", r.injection);
            for c in &r.bins {
                assert!(c.joint <= c.n && c.baseline <= c.n && c.joint_only <= c.joint);
                assert!(c.blame_exact <= c.blame_hit);
                assert!(c.blame_hit + c.blame_indeterminate <= c.joint);
                assert!(c.joint_indeterminate <= c.n - c.joint);
            }
        }
        assert_eq!(study.log_lines().len(), 6 * 5 + 6);
    }

    /// Mutation: with the baseline standing in for the joint solver, no detection is joint-only (COD-02's
    /// included) and, with the other measures empty, the report fires. The real solver at the same seed does not,
    /// so the substitution is what fires it.
    #[test]
    fn test_mutation_joint_solve_replaced_by_baseline_fires_kill() {
        let cent = q(1, 100);
        let cells = vec![
            cell("T", int(10), int(1)),
            cell("a1", q(26, 5), cent),
            cell("a2", q(26, 5), cent),
            cell("b1", q(24, 5), cent),
            cell("b2", q(24, 5), cent),
        ];
        let ids = vec![
            sum("A", SUM_FAMILY, "T", &["a1", "a2"]),
            sum("B", SUM_FAMILY, "T", &["b1", "b2"]),
        ];
        let report = |joint: Detector| -> KillReport {
            let study = power_study(&quick(20), joint).expect("the study runs");
            let cod02 = joint_only(&cells, &ids, &rounded_once(), joint).expect("solves");
            let mut joint_only = study.joint_only_by_kind();
            joint_only.push(("COD-02".to_string(), u64::from(cod02)));
            KillReport {
                joint_only_detections: joint_only,
                size1: Vec::new(),
                discoveries: Vec::new(),
                implied_consumers: Vec::new(),
            }
        };
        let mutated = report(baseline_detects);
        assert!(mutated.joint_only_detections.iter().all(|(_, n)| *n == 0));
        assert!(mutated.fired());
        let real = report(joint_detects);
        assert!(
            real.joint_only_detections
                .contains(&("COD-02".to_string(), 1))
        );
        assert!(!real.fired());
        // The study is a function of its seed.
        assert_eq!(
            power_study(&quick(5), joint_detects).expect("the study runs"),
            power_study(&quick(5), joint_detects).expect("the study runs")
        );
    }
}
