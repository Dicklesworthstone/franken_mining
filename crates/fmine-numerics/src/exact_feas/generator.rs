//! Random clean tables for the exact-feasibility kernel (bd-4wvl.7, BD-340.S7): exact true values, printed under
//! one construction and one tie convention per table, with the true values kept for checks.
//!
//! - **Blocks** ([`Block`]), one connected component per table:
//!   - `Sum`: a tree of sums to depth 3, each line added or subtracted;
//!   - `HalfYear`: two segments' Q1 and Q2, their half years, and the quarter and half-year totals (S1's grid);
//!   - `Breakdowns`: one total broken down twice, the lines at the total's step or finer (COD-02's shape);
//!   - `Share`: Q1 + Q2 = H1 at 100%, and Q1 attributable at a share from [`SHARES`] (E113's shape);
//!   - `Ratio`: a rate `factor·(n₁ + n₂)/den` with a positive denominator, the rate in no other identity (S2);
//!   - `Reprint`: a sum whose total is printed again, at the same step or a tenth of it (`Equal`);
//!   - `WeightedMean`: a rate weighted over 2 or 3 parts (S6).
//!
//!   S5's roll-forward has no identity kind yet, so no block.
//! - **True values** are exact. A drawn value is `m·step/1000` for a uniform integer `m`, so a tie can occur by
//!   chance; a tie table puts every drawn value on a half step. A `Breakdowns` table's second breakdown splits
//!   the total in drawn proportions.
//! - **Printing** ([`round`]) is under one of `HalfUp`, `HalfDown`, `HalfEven` and `Truncate`; `Closed` is no
//!   rounding rule and is refused. `Closed` brackets every round-to-nearest rule but not truncation, so a
//!   truncated table's tie-agnostic reading is `Truncate` itself ([`Generated::bracket`]). Under `ComponentSum` every `Sum` total prints as Σ ± its printed lines, inner
//!   sums first; under `RoundedOnce` it prints its own true value rounded. A `Breakdowns` table is always
//!   `RoundedOnce`, since one printed total cannot foot two breakdowns.
//! - **Draws.** The shape (block, sizes, steps, signs, construction, convention, share, tie table) comes from the
//!   key's [`PhiloxStream`] from counter 0, the values from the same stream at counters set by the vintage.
//!   [`generate_vintage`] with another vintage (below 2¹⁶) gives the same shape with fresh values: a stale
//!   vintage. An integer below `n` is a 64-bit draw modulo `n` (a bias below 2⁻⁴⁰ for every `n` used).
//!
//! Determinism: Exact (rational arithmetic; the draws are Philox integers, no floating point).

use std::collections::BTreeMap;

use fmine_core::{PhiloxStream, RngKey};
use fmine_units::UnitsError;

use super::{Cell, Closure, Construction, Convention, Identity, IdentityKind, Rat, Rules, Scope};

/// The family of every `Sum` the generator writes: one construction per table.
pub const SUM_FAMILY: &str = "sum";

/// Ownership shares, as exact fractions: 0.45, 0.5, 0.6, 0.615, 0.8, 0.84, 0.897.
pub const SHARES: [(i128, i128); 7] = [
    (9, 20),
    (1, 2),
    (3, 5),
    (123, 200),
    (4, 5),
    (21, 25),
    (897, 1000),
];

/// A line of a `Sum` tree becomes a subtree only while the tree has at most this many cells.
pub const MAX_TREE_CELLS: usize = 12;

/// A drawn value is `m·step/GRID`.
const GRID: u64 = 1000;

/// The identity graphs a table can take.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Block {
    Sum,
    HalfYear,
    Breakdowns,
    Share,
    Ratio,
    Reprint,
    WeightedMean,
}

impl Block {
    pub const ALL: [Self; 7] = [
        Self::Sum,
        Self::HalfYear,
        Self::Breakdowns,
        Self::Share,
        Self::Ratio,
        Self::Reprint,
        Self::WeightedMean,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Sum => "sum",
            Self::HalfYear => "half_year",
            Self::Breakdowns => "breakdowns",
            Self::Share => "share",
            Self::Ratio => "ratio",
            Self::Reprint => "reprint",
            Self::WeightedMean => "weighted_mean",
        }
    }
}

/// What tables to draw. Each list is drawn from uniformly; none may be empty.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GenSpec {
    pub blocks: Vec<Block>,
    pub constructions: Vec<Construction>,
    /// Rounding rules; `Closed` is refused.
    pub conventions: Vec<Convention>,
    /// Whether a table may be a tie table (one in four when allowed).
    pub ties: bool,
    /// The deepest `Sum` tree, 1 to 3.
    pub max_depth: u32,
}

impl GenSpec {
    /// Every block, both constructions, the four rounding rules, tie tables, depth 3.
    #[must_use]
    pub fn all() -> Self {
        Self {
            blocks: Block::ALL.to_vec(),
            constructions: vec![Construction::ComponentSum, Construction::RoundedOnce],
            conventions: vec![
                Convention::HalfUp,
                Convention::HalfDown,
                Convention::HalfEven,
                Convention::Truncate,
            ],
            ties: true,
            max_depth: 3,
        }
    }
}

/// Why no table was drawn.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GenError {
    /// A list of the spec is empty, the depth is outside 1 to 3, or a step is not positive.
    BadSpec(&'static str),
    /// `Closed` brackets a printed value; it does not produce one.
    NotARoundingRule,
    /// The exact arithmetic overflowed.
    Overflow,
}

impl From<UnitsError> for GenError {
    fn from(_: UnitsError) -> Self {
        Self::Overflow
    }
}

/// One clean table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Generated {
    pub block: Block,
    pub cells: Vec<Cell>,
    pub identities: Vec<Identity>,
    /// The true value of each cell, in `cells` order.
    pub truth: Vec<Rat>,
    pub construction: Construction,
    /// The rule the table was printed under.
    pub convention: Convention,
    /// Whether every drawn value lies on a half step.
    pub tie_table: bool,
}

impl Generated {
    /// `convention` for every cell, and the table's construction for its sums.
    #[must_use]
    pub fn rules(&self, convention: Convention) -> Rules {
        Rules::new(convention).with(SUM_FAMILY, self.construction)
    }

    /// The convention of a reader who knows the rounding family but not the tie rule: `Closed` for a
    /// round-to-nearest table, `Truncate` for a truncated one (`Closed`, v ± h, does not contain [v, v + 2h)).
    #[must_use]
    pub fn bracket(&self) -> Convention {
        if self.convention == Convention::Truncate {
            Convention::Truncate
        } else {
            Convention::Closed
        }
    }

    /// The true values by locator.
    #[must_use]
    pub fn witness(&self) -> BTreeMap<String, Rat> {
        self.cells
            .iter()
            .zip(&self.truth)
            .map(|(c, x)| (c.locator.clone(), *x))
            .collect()
    }
}

/// Rounds `x` to a multiple of a positive `step` under a rounding rule: `HalfUp` and `HalfDown` send a tie toward
/// +∞ and −∞, `HalfEven` to the even multiple, and `Truncate` rounds toward zero. The printed value's interval
/// under the same rule ([`Convention::interval`]) contains `x`.
pub fn round(x: Rat, step: Rat, convention: Convention) -> Result<Rat, GenError> {
    if step.signum() <= 0 {
        return Err(GenError::BadSpec("a rounding step must be positive"));
    }
    let k = x.checked_div(step)?;
    let (num, den) = (k.num(), k.den());
    let floor = num.div_euclid(den);
    let rem = num.rem_euclid(den);
    // The fraction rem/den against one half.
    let half = rem.checked_mul(2).ok_or(GenError::Overflow)?.cmp(&den);
    let up = |f: i128| f.checked_add(1).ok_or(GenError::Overflow);
    let m = match convention {
        Convention::Closed => return Err(GenError::NotARoundingRule),
        Convention::HalfUp if half.is_ge() => up(floor)?,
        Convention::HalfDown if half.is_gt() => up(floor)?,
        Convention::HalfEven if half.is_gt() || (half.is_eq() && floor % 2 != 0) => up(floor)?,
        Convention::Truncate if num < 0 && rem != 0 => up(floor)?,
        _ => floor,
    };
    Ok(Rat::from_integer(m).checked_mul(step)?)
}

/// Draws from one counter range of the key's stream.
struct Draws<'a> {
    stream: &'a PhiloxStream,
    at: u64,
}

impl Draws<'_> {
    fn below(&mut self, n: u64) -> u64 {
        let b = self.stream.block_at(self.at);
        self.at = self.at.wrapping_add(1);
        (u64::from(b[0]) | (u64::from(b[1]) << 32)) % n.max(1)
    }

    /// An integer in `lo..=hi` (`lo` when `hi < lo`).
    fn between(&mut self, lo: u64, hi: u64) -> u64 {
        lo + self.below(hi.saturating_sub(lo).saturating_add(1))
    }

    fn pick<T: Copy>(&mut self, xs: &[T]) -> Result<T, GenError> {
        let i = usize::try_from(self.below(xs.len() as u64)).map_err(|_| GenError::Overflow)?;
        xs.get(i)
            .copied()
            .ok_or(GenError::BadSpec("every list of the spec needs an entry"))
    }
}

/// An identity other than a `Sum`, over cell indices.
enum Other {
    Scale {
        target: usize,
        source: usize,
        factor: Rat,
    },
    Ratio {
        rate: usize,
        num: usize,
        den: usize,
        factor: Rat,
    },
    Equal {
        a: usize,
        b: usize,
    },
    Mean {
        rate: usize,
        parts: Vec<(usize, usize)>,
    },
}

/// A `Sum` over cell indices: its id, its total, and its signed lines.
type SumRow = (String, usize, Vec<(usize, i8)>);

/// One table under construction: names, steps and true values, the sums in footing order, the other identities.
struct Table<'a> {
    values: Draws<'a>,
    tie: bool,
    names: Vec<String>,
    steps: Vec<Rat>,
    truth: Vec<Rat>,
    sums: Vec<SumRow>,
    others: Vec<(&'static str, &'static str, Other)>,
}

impl Table<'_> {
    /// A drawn value in `[lo, hi]` steps, as `m·step/1000` (on a half step in a tie table).
    fn draw(&mut self, step: Rat, lo: u64, hi: u64) -> Result<Rat, GenError> {
        let m = if self.tie {
            self.values.between(lo, hi.saturating_sub(1)) * GRID + GRID / 2
        } else {
            self.values.between(lo * GRID, hi * GRID)
        };
        Ok(Rat::new(i128::from(m), i128::from(GRID))?.checked_mul(step)?)
    }

    fn cell(&mut self, name: String, step: Rat, x: Rat) -> usize {
        self.names.push(name);
        self.steps.push(step);
        self.truth.push(x);
        self.names.len() - 1
    }

    fn leaf(&mut self, name: String, step: Rat, lo: u64, hi: u64) -> Result<usize, GenError> {
        let x = self.draw(step, lo, hi)?;
        Ok(self.cell(name, step, x))
    }

    fn signed_sum(&self, lines: &[(usize, i8)]) -> Result<Rat, GenError> {
        let mut x = Rat::from_integer(0);
        for &(i, s) in lines {
            let v = self.truth[i];
            x = if s < 0 {
                x.checked_sub(v)?
            } else {
                x.checked_add(v)?
            };
        }
        Ok(x)
    }

    /// A total named `name` (also its identity's id) of `lines`, true value Σ sign·line. It is pushed after its
    /// lines, so the sums stay in footing order.
    fn total(&mut self, name: &str, step: Rat, lines: &[(usize, i8)]) -> Result<usize, GenError> {
        let x = self.signed_sum(lines)?;
        let at = self.cell(name.to_string(), step, x);
        self.sums.push((name.to_string(), at, lines.to_vec()));
        Ok(at)
    }
}

/// A clean table drawn from `key` ([`generate_vintage`] at vintage 0).
pub fn generate(key: RngKey, spec: &GenSpec) -> Result<Generated, GenError> {
    generate_vintage(key, spec, 0)
}

/// The table of `key` with the true values of `vintage`: every vintage of a key has the same shape.
pub fn generate_vintage(key: RngKey, spec: &GenSpec, vintage: u32) -> Result<Generated, GenError> {
    if !(1..=3).contains(&spec.max_depth) {
        return Err(GenError::BadSpec("the depth is 1 to 3"));
    }
    if spec.conventions.contains(&Convention::Closed) {
        return Err(GenError::NotARoundingRule);
    }
    let stream = PhiloxStream::new(key);
    let mut shape = Draws {
        stream: &stream,
        at: 0,
    };
    let block = shape.pick(&spec.blocks)?;
    let convention = shape.pick(&spec.conventions)?;
    let mut construction = shape.pick(&spec.constructions)?;
    if block == Block::Breakdowns {
        construction = Construction::RoundedOnce;
    }
    let tie = spec.ties && shape.below(4) == 0;
    let step = shape.pick(&[(1, 1), (1, 10), (1, 100)])?;
    let step = Rat::new(step.0, step.1)?;
    let mut t = Table {
        values: Draws {
            stream: &stream,
            at: (u64::from(vintage) + 1) << 48,
        },
        tie,
        names: Vec::new(),
        steps: Vec::new(),
        truth: Vec::new(),
        sums: Vec::new(),
        others: Vec::new(),
    };
    match block {
        Block::Sum => {
            let depth = shape.between(1, u64::from(spec.max_depth));
            tree(&mut t, &mut shape, "s", step, depth)?;
        }
        Block::HalfYear => half_year(&mut t, step)?,
        Block::Breakdowns => {
            let fine = shape.pick(&[1, 10, 100])?;
            let (ka, kb) = (shape.between(2, 4), shape.between(2, 4));
            breakdowns(&mut t, step, fine, ka, kb)?;
        }
        Block::Share => {
            let (n, d) = shape.pick(&SHARES)?;
            share(&mut t, step, Rat::new(n, d)?)?;
        }
        Block::Ratio => {
            let thousand = shape.below(2) == 0;
            ratio(&mut t, step, thousand)?;
        }
        Block::Reprint => {
            let k = shape.between(2, 4);
            let signs: Vec<i8> = (0..k)
                .map(|_| if shape.below(4) == 0 { -1 } else { 1 })
                .collect();
            let finer = shape.below(2) == 0;
            reprint(&mut t, step, &signs, finer)?;
        }
        Block::WeightedMean => {
            let parts = shape.between(2, 3);
            weighted_mean(&mut t, step, parts)?;
        }
    }
    print(t, block, construction, convention)
}

/// A `Sum` tree: 2 to 4 lines at the root, 2 or 3 below. A line is a subtree one time in three, while depth and
/// [`MAX_TREE_CELLS`] allow, and is subtracted one time in four.
fn tree(
    t: &mut Table<'_>,
    shape: &mut Draws<'_>,
    name: &str,
    step: Rat,
    depth: u64,
) -> Result<usize, GenError> {
    let k = if name == "s" {
        shape.between(2, 4)
    } else {
        shape.between(2, 3)
    };
    let mut lines = Vec::new();
    for i in 0..k {
        let child = format!("{name}.{i}");
        let sign = if shape.below(4) == 0 { -1 } else { 1 };
        let room = t.names.len() <= MAX_TREE_CELLS;
        let at = if depth > 1 && room && shape.below(3) == 0 {
            tree(t, shape, &child, step, depth - 1)?
        } else {
            t.leaf(child, step, 0, 1000)?
        };
        lines.push((at, sign));
    }
    t.total(name, step, &lines)
}

fn half_year(t: &mut Table<'_>, step: Rat) -> Result<(), GenError> {
    let mut q = Vec::new();
    for name in ["aQ1", "aQ2", "bQ1", "bQ2"] {
        q.push(t.leaf(name.to_string(), step, 0, 1000)?);
    }
    let a = t.total("aH1", step, &[(q[0], 1), (q[1], 1)])?;
    let b = t.total("bH1", step, &[(q[2], 1), (q[3], 1)])?;
    let q1 = t.total("tQ1", step, &[(q[0], 1), (q[2], 1)])?;
    let q2 = t.total("tQ2", step, &[(q[1], 1), (q[3], 1)])?;
    let h = t.total("tH1", step, &[(a, 1), (b, 1)])?;
    t.sums.push(("tH1q".to_string(), h, vec![(q1, 1), (q2, 1)]));
    Ok(())
}

/// One total `T` = Σ aᵢ = Σ bⱼ, its lines at `step / fine`. The `a` lines are drawn (at most 1000 total steps
/// in all); the `b` lines split `T` in proportions w₁ : … : w_kb, each wⱼ in 1..=1000.
fn breakdowns(t: &mut Table<'_>, step: Rat, fine: i128, ka: u64, kb: u64) -> Result<(), GenError> {
    let line_step = step.checked_div(Rat::from_integer(fine))?;
    let mut a = Vec::new();
    for i in 0..ka {
        let x = t.draw(step, 0, 1000 / ka)?;
        a.push((t.cell(format!("a{i}"), line_step, x), 1));
    }
    let tx = t.signed_sum(&a)?;
    let w: Vec<u64> = (0..kb).map(|_| t.values.between(1, 1000)).collect();
    let wsum = Rat::from_integer(i128::from(w.iter().sum::<u64>()));
    let mut b = Vec::new();
    for (j, wj) in w.iter().enumerate() {
        let x = tx
            .checked_mul(Rat::from_integer(i128::from(*wj)))?
            .checked_div(wsum)?;
        b.push((t.cell(format!("b{j}"), line_step, x), 1));
    }
    let total = t.cell("T".to_string(), step, tx);
    t.sums.push(("T=a".to_string(), total, a));
    t.sums.push(("T=b".to_string(), total, b));
    Ok(())
}

fn share(t: &mut Table<'_>, step: Rat, factor: Rat) -> Result<(), GenError> {
    let q1 = t.leaf("Q1".to_string(), step, 0, 1000)?;
    let q2 = t.leaf("Q2".to_string(), step, 0, 1000)?;
    t.total("H1", step, &[(q1, 1), (q2, 1)])?;
    let x = factor.checked_mul(t.truth[q1])?;
    let target = t.cell("Q1att".to_string(), step, x);
    let kind = Other::Scale {
        target,
        source: q1,
        factor,
    };
    t.others.push(("share", "share", kind));
    Ok(())
}

/// `rate = factor·(n₁ + n₂)/den`: factor 1000 (a per-ounce rate of $M over koz, printed to 1) or 1 (printed to
/// a thousandth); the denominator lies in [100, 1000] steps.
fn ratio(t: &mut Table<'_>, step: Rat, thousand: bool) -> Result<(), GenError> {
    let n1 = t.leaf("n1".to_string(), step, 0, 1000)?;
    let n2 = t.leaf("n2".to_string(), step, 0, 1000)?;
    let num = t.total("num", step, &[(n1, 1), (n2, 1)])?;
    let den = t.leaf("den".to_string(), step, 100, 1000)?;
    let (factor, rate_step) = if thousand {
        (Rat::from_integer(1000), Rat::from_integer(1))
    } else {
        (Rat::from_integer(1), Rat::new(1, 1000)?)
    };
    let x = factor
        .checked_mul(t.truth[num])?
        .checked_div(t.truth[den])?;
    let rate = t.cell("rate".to_string(), rate_step, x);
    let kind = Other::Ratio {
        rate,
        num,
        den,
        factor,
    };
    t.others.push(("unit_rate", "ratio", kind));
    Ok(())
}

fn reprint(t: &mut Table<'_>, step: Rat, signs: &[i8], finer: bool) -> Result<(), GenError> {
    let mut lines = Vec::new();
    for (i, s) in signs.iter().enumerate() {
        lines.push((t.leaf(format!("l{i}"), step, 0, 1000)?, *s));
    }
    let total = t.total("T", step, &lines)?;
    let again_step = if finer {
        step.checked_div(Rat::from_integer(10))?
    } else {
        step
    };
    let x = t.truth[total];
    let again = t.cell("T'".to_string(), again_step, x);
    let kind = Other::Equal { a: total, b: again };
    t.others.push(("reprint", "reprint", kind));
    Ok(())
}

/// `rate·Σwᵢ = Σ rateᵢ·wᵢ`: weights in [100, 1000] steps, part rates in [500, 2000] printed to 1.
fn weighted_mean(t: &mut Table<'_>, step: Rat, parts: u64) -> Result<(), GenError> {
    let one = Rat::from_integer(1);
    let (mut wsum, mut rwsum) = (Rat::from_integer(0), Rat::from_integer(0));
    let mut pairs = Vec::new();
    for i in 0..parts {
        let r = t.leaf(format!("r{i}"), one, 500, 2000)?;
        let w = t.leaf(format!("w{i}"), step, 100, 1000)?;
        wsum = wsum.checked_add(t.truth[w])?;
        rwsum = rwsum.checked_add(t.truth[r].checked_mul(t.truth[w])?)?;
        pairs.push((r, w));
    }
    let rate = t.cell("rate".to_string(), one, rwsum.checked_div(wsum)?);
    t.others
        .push(("mean", "mean", Other::Mean { rate, parts: pairs }));
    Ok(())
}

/// Prints every cell under `convention`, foots the sums under `ComponentSum`, and writes the identities.
fn print(
    t: Table<'_>,
    block: Block,
    construction: Construction,
    convention: Convention,
) -> Result<Generated, GenError> {
    let mut printed = Vec::with_capacity(t.truth.len());
    for (x, step) in t.truth.iter().zip(&t.steps) {
        printed.push(round(*x, *step, convention)?);
    }
    if construction == Construction::ComponentSum {
        for (_, total, lines) in &t.sums {
            let mut v = Rat::from_integer(0);
            for &(i, s) in lines {
                v = if s < 0 {
                    v.checked_sub(printed[i])?
                } else {
                    v.checked_add(printed[i])?
                };
            }
            printed[*total] = v;
        }
    }
    let cells: Vec<Cell> = t
        .names
        .iter()
        .zip(&printed)
        .zip(&t.steps)
        .map(|((name, p), step)| Cell {
            locator: format!("gen:{}!{name}", block.as_str()),
            sha256: "0".repeat(64),
            token: format!("{}/{}", p.num(), p.den()),
            printed: *p,
            step: *step,
            scope: Scope::Visible,
        })
        .collect();
    let loc = |i: usize| cells[i].locator.clone();
    let identity = |id: &str, family: &str, kind: IdentityKind| Identity {
        id: id.to_string(),
        family: family.to_string(),
        closure: Closure::Definitional,
        kind,
    };
    let mut identities: Vec<Identity> = t
        .sums
        .iter()
        .map(|(id, total, lines)| {
            let kind = IdentityKind::Sum {
                total: loc(*total),
                lines: lines.iter().map(|&(i, _)| loc(i)).collect(),
                signs: lines.iter().map(|&(_, s)| s).collect(),
            };
            identity(id, SUM_FAMILY, kind)
        })
        .collect();
    for (id, family, other) in &t.others {
        let kind = match other {
            Other::Scale {
                target,
                source,
                factor,
            } => IdentityKind::Scale {
                target: loc(*target),
                source: loc(*source),
                factor: *factor,
            },
            Other::Ratio {
                rate,
                num,
                den,
                factor,
            } => IdentityKind::Ratio {
                rate: loc(*rate),
                num: loc(*num),
                den: loc(*den),
                factor: *factor,
            },
            Other::Equal { a, b } => IdentityKind::Equal {
                a: loc(*a),
                b: loc(*b),
            },
            Other::Mean { rate, parts } => IdentityKind::WeightedMean {
                rate: loc(*rate),
                parts: parts.iter().map(|&(r, w)| (loc(r), loc(w))).collect(),
            },
        };
        identities.push(identity(id, family, kind));
    }
    Ok(Generated {
        block,
        cells,
        identities,
        truth: t.truth,
        construction,
        convention,
        tie_table: t.tie,
    })
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};

    use fmine_core::det;

    use super::*;
    use crate::exact_feas::discover::p0;
    use crate::exact_feas::{Certificate, Outcome, check_witness, solve};

    fn key(table: u64) -> RngKey {
        RngKey {
            seed_root: 3407,
            partition: 1,
            domain: 0,
            var_group: 0,
            realization: 0,
            brick: table,
        }
    }

    fn q(n: i128, d: i128) -> Rat {
        Rat::new(n, d).expect("rational")
    }

    /// Each identity evaluated on the true values in plain rational arithmetic (no kernel code).
    fn holds(id: &Identity, x: &BTreeMap<String, Rat>) -> bool {
        let v = |l: &String| x[l];
        let add = |a: Rat, b: Rat| a.checked_add(b).expect("add");
        let mul = |a: Rat, b: Rat| a.checked_mul(b).expect("mul");
        match &id.kind {
            IdentityKind::Sum {
                total,
                lines,
                signs,
            } => {
                let s = lines.iter().zip(signs).fold(q(0, 1), |s, (l, g)| {
                    add(s, mul(v(l), Rat::from_integer(i128::from(*g))))
                });
                s == v(total)
            }
            IdentityKind::Equal { a, b } => v(a) == v(b),
            IdentityKind::Scale {
                target,
                source,
                factor,
            } => v(target) == mul(*factor, v(source)),
            IdentityKind::Ratio {
                rate,
                num,
                den,
                factor,
            } => mul(v(rate), v(den)) == mul(*factor, v(num)),
            IdentityKind::WeightedMean { rate, parts } => {
                let w = parts.iter().fold(q(0, 1), |s, (_, w)| add(s, v(w)));
                let rw = parts
                    .iter()
                    .fold(q(0, 1), |s, (r, w)| add(s, mul(v(r), v(w))));
                mul(v(rate), w) == rw
            }
        }
    }

    #[test]
    fn test_generator_deterministic_for_key() {
        let spec = GenSpec::all();
        let shape = |g: &Generated| {
            let cells: Vec<(String, Rat)> = g
                .cells
                .iter()
                .map(|c| (c.locator.clone(), c.step))
                .collect();
            (
                g.block,
                g.construction,
                g.convention,
                g.tie_table,
                g.identities.len(),
                cells,
            )
        };
        let (mut blocks, mut fresh) = (BTreeSet::new(), 0);
        for t in 0..300 {
            let a = generate(key(t), &spec).expect("a table");
            assert_eq!(a, generate(key(t), &spec).expect("a table"), "table {t}");
            let b = generate_vintage(key(t), &spec, 1).expect("a table");
            assert_eq!(shape(&a), shape(&b), "table {t}: a vintage keeps the shape");
            fresh += usize::from(a.truth != b.truth);
            blocks.insert(a.block);
        }
        assert_ne!(
            generate(key(0), &spec).expect("a table"),
            generate(key(1), &spec).expect("a table")
        );
        assert_eq!(fresh, 300, "every second vintage draws fresh values");
        assert_eq!(blocks.len(), Block::ALL.len());
        // The spec is checked before any draw.
        let closed = GenSpec {
            conventions: vec![Convention::Closed],
            ..GenSpec::all()
        };
        assert_eq!(generate(key(0), &closed), Err(GenError::NotARoundingRule));
        let empty = GenSpec {
            blocks: Vec::new(),
            ..GenSpec::all()
        };
        assert!(matches!(
            generate(key(0), &empty),
            Err(GenError::BadSpec(_))
        ));
    }

    /// `round` and `Convention::interval` are separate code: the printed value is the one multiple whose interval
    /// holds `x`, ties included.
    #[test]
    fn test_round_lands_in_its_interval() {
        let step = q(1, 10);
        let h = q(1, 20);
        for conv in [
            Convention::HalfUp,
            Convention::HalfDown,
            Convention::HalfEven,
            Convention::Truncate,
        ] {
            for m in -400..=400 {
                // x = m/80: every twentieth of a step, so ties (odd multiples of 1/20) and grid points occur.
                let x = q(m, 80);
                let v = round(x, step, conv).expect("rounds");
                let holding: Vec<Rat> = (-1..=1)
                    .map(|d| {
                        v.checked_add(step.checked_mul(Rat::from_integer(d)).expect("mul"))
                            .expect("add")
                    })
                    .filter(|w| {
                        conv.interval(*w, h)
                            .expect("interval")
                            .contains(x)
                            .expect("cmp")
                    })
                    .collect();
                assert_eq!(holding, vec![v], "{conv:?} x={m}/80");
            }
        }
        assert_eq!(
            round(q(1, 2), q(1, 1), Convention::Closed),
            Err(GenError::NotARoundingRule)
        );
        assert!(round(q(1, 2), q(0, 1), Convention::HalfUp).is_err());
        // Known values: 2.5 → 3, 2, 2, 2; −2.5 → −2, −3, −2, −2.
        let r = |x: Rat, c| round(x, q(1, 1), c).expect("rounds");
        assert_eq!(
            [
                Convention::HalfUp,
                Convention::HalfDown,
                Convention::HalfEven,
                Convention::Truncate
            ]
            .map(|c| (r(q(5, 2), c), r(q(-5, 2), c))),
            [
                (q(3, 1), q(-2, 1)),
                (q(2, 1), q(-3, 1)),
                (q(2, 1), q(-2, 1)),
                (q(2, 1), q(-2, 1))
            ]
        );
    }

    /// Every identity holds exactly on the true values; every printed value is its true value rounded, except a
    /// total footed under `ComponentSum`, which is Σ ± its printed lines. `check_witness` agrees under the
    /// generating rule and under `Closed`.
    #[test]
    fn test_generator_truth_satisfies_identities_exactly() {
        let spec = GenSpec::all();
        for t in 0..2000 {
            let g = generate(key(t), &spec).expect("a table");
            let x = g.witness();
            for id in &g.identities {
                assert!(
                    holds(id, &x),
                    "table {t} {:?}: {} fails on the truth",
                    g.block,
                    id.id
                );
            }
            let footed: BTreeSet<&str> = g
                .identities
                .iter()
                .filter_map(|id| match &id.kind {
                    IdentityKind::Sum { total, .. }
                        if g.construction == Construction::ComponentSum =>
                    {
                        Some(total.as_str())
                    }
                    _ => None,
                })
                .collect();
            for (c, truth) in g.cells.iter().zip(&g.truth) {
                if !footed.contains(c.locator.as_str()) {
                    assert_eq!(
                        round(*truth, c.step, g.convention),
                        Ok(c.printed),
                        "table {t}: {}",
                        c.locator
                    );
                }
            }
            for conv in [g.convention, g.bracket()] {
                assert_eq!(
                    check_witness(&g.cells, &g.identities, &g.rules(conv), &x),
                    Ok(()),
                    "table {t} {:?} {conv:?}",
                    g.block
                );
            }
        }
    }

    /// Soundness (S1, grown in S7): clean tables of every kind S1–S6 has landed are never certified inconsistent,
    /// under the rule they were printed with or under their bracket (`Closed`, or `Truncate` for a truncated
    /// table), with their own construction.
    #[test]
    fn prop_clean_generated_tables_have_no_certified_conflict() {
        let spec = GenSpec::all();
        let tables = 10_000;
        let mut tally: BTreeMap<&str, u64> = BTreeMap::new();
        let (mut blocks, mut constructions, mut conventions, mut ties) =
            (BTreeMap::new(), BTreeSet::new(), BTreeSet::new(), 0);
        for t in 0..tables {
            let g = generate(key(t), &spec).expect("a table");
            *blocks.entry(g.block.as_str()).or_insert(0_u64) += 1;
            constructions.insert(g.construction);
            conventions.insert(g.convention);
            ties += u64::from(g.tie_table);
            for conv in [g.bracket(), g.convention] {
                for c in solve(&g.cells, &g.identities, &g.rules(conv)).expect("solves") {
                    let k = match c.outcome {
                        Outcome::Feasible { .. } => "feasible",
                        Outcome::NotRefuted => "not_refuted",
                        Outcome::Indeterminate { .. } => "indeterminate",
                        Outcome::Infeasible { .. } => {
                            panic!(
                                "table {t} {:?} {conv:?}: a clean table gave {:?}",
                                g.block, c.outcome
                            )
                        }
                    };
                    *tally.entry(k).or_insert(0) += 1;
                }
            }
        }
        let runs: u64 = tally.values().sum();
        let indeterminate = tally.get("indeterminate").copied().unwrap_or(0);
        let by_block: Vec<String> = blocks.iter().map(|(b, n)| format!("\"{b}\":{n}")).collect();
        println!(
            "{{\"kind\":\"soundness\",\"tables\":{tables},\"seed\":3407,\"runs\":{runs},\"feasible\":{},\
             \"not_refuted\":{},\"indeterminate\":{indeterminate},\"infeasible\":0,\"indeterminate_n\":{runs},\
             \"tie_tables\":{ties},\"by_block\":{{{}}}}}",
            tally.get("feasible").copied().unwrap_or(0),
            tally.get("not_refuted").copied().unwrap_or(0),
            by_block.join(",")
        );
        assert_eq!(blocks.len(), Block::ALL.len());
        assert_eq!(constructions.len(), 2);
        assert_eq!(conventions.len(), 4);
        assert!(ties > 0);
        // Each table is one component, solved twice.
        assert_eq!(runs, 2 * tables);
    }

    /// Metamorphic (S2 with S4's p₀): a table printed `RoundedOnce` and solved as `ComponentSum` conflicts exactly
    /// when its total does not foot, which for k round-to-nearest lines happens with probability 1 − p₀(k).
    /// Truncation is not round-to-nearest, so it is left out.
    #[test]
    fn test_rounded_once_as_component_sum_conflicts_at_one_minus_p0() {
        let spec = GenSpec {
            blocks: vec![Block::Sum],
            constructions: vec![Construction::RoundedOnce],
            conventions: vec![
                Convention::HalfUp,
                Convention::HalfDown,
                Convention::HalfEven,
            ],
            ties: false,
            max_depth: 1,
        };
        let as_component_sum =
            Rules::new(Convention::Closed).with(SUM_FAMILY, Construction::ComponentSum);
        let tables = 4000;
        let (mut conflicts, mut expected, mut var) = (0_u64, 0.0_f64, 0.0_f64);
        for t in 0..tables {
            let g = generate(key(t), &spec).expect("a table");
            let [id] = &g.identities[..] else {
                panic!("one sum per table");
            };
            let IdentityKind::Sum { lines, .. } = &id.kind else {
                panic!("a sum");
            };
            let k = u32::try_from(lines.len()).expect("small");
            let p = p0(k).expect("p0").to_f64();
            expected += 1.0 - p;
            var += p * (1.0 - p);
            match &solve(&g.cells, &g.identities, &as_component_sum).expect("solves")[..] {
                [c] => match &c.outcome {
                    Outcome::Infeasible {
                        certificate: Certificate::Residual { .. },
                    } => conflicts += 1,
                    Outcome::Feasible { .. } => {}
                    other => panic!("table {t}: {other:?}"),
                },
                other => panic!("table {t}: {other:?}"),
            }
        }
        let sigma = det::sqrt(var);
        let z = (conflicts as f64 - expected) / sigma;
        println!(
            "{{\"kind\":\"metamorphic\",\"tables\":{tables},\"seed\":3407,\"conflicts\":{conflicts},\
             \"expected\":{expected:.2},\"sigma\":{sigma:.2},\"z\":{z:.3}}}"
        );
        assert!(z.abs() <= 4.0, "z = {z}");
    }
}
