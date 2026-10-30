//! Exact rounding-interval feasibility (bd-4wvl.1, BD-340.S1; constructions, ratios and implied intervals
//! bd-4wvl.2, BD-340.S2): can a set of printed values all be roundings of true values that satisfy a set of
//! identities exactly?
//!
//! - Each printed value `v` with step `2h` stands for an interval of true values, set by a tie [`Convention`].
//! - Each [`Identity`] is a linear row (`Sum`, `Equal`, `Scale`) or a ratio `rate = factor·num/den`.
//! - A `Sum` family's [`Construction`] is declared in [`Rules`] before any run, never chosen per instance.
//!   `RoundedOnce` keeps the total in its own printed box. `ComponentSum` first requires the printed total to
//!   equal Σ ± printed lines exactly (otherwise the certificate is the residual), then drops the total's own
//!   box: the true total is Σ ± true lines and carries one half-width per line.
//! - A `Ratio` whose rate cell occurs in no other identity, and whose denominator keeps one sign on its
//!   interval, becomes the linear pair `r_lo·den ≤ factor·num ≤ r_hi·den` (reversed for a negative
//!   denominator), each side with a slack that carries the rate's open end. A shared rate cell is bilinear and
//!   gives `Indeterminate` until S6; a denominator interval that contains zero is refused.
//! - Cells linked by identities form components. Each is solved by an exact two-phase simplex over [`Rat`] with
//!   Bland's rule, so it terminates. Every variable maps to non-negative columns (shifted from a finite end, or
//!   split in two when free), and boxes are explicit rows `y + s = w`: the same polytope as a bounded-variable
//!   simplex, at a higher pivot cost. Open ends: one `t ∈ [0, 1]` is maximised with every open end at least `t`
//!   away; the cells are strictly feasible exactly when `t* > 0`.
//!
//! Outcomes: [`Outcome::Feasible`] with a witness, [`Outcome::Infeasible`] with a [`Certificate`] (Farkas
//! multipliers over the linearised rows, or a component-sum residual), or [`Outcome::Indeterminate`] (a cap, an
//! overflow, a bilinear ratio, or a checker rejection), never a claim. [`Outcome::NotRefuted`] is reserved for
//! relaxations (S6).
//!
//! Checkers are independent of the simplex. [`check_witness`] checks every cell against its interval (a
//! component-sum total has none) and every identity exactly, the ratio included, without the linearisation.
//! [`check_certificate`] recomputes a residual from the printed values, or forms `c = Σ λᵢ·aᵢ` and
//! `b* = Σ λᵢ·bᵢ` over the linearised rows and the exact range of `cᵀx` over the boxes, open and infinite ends
//! included, and accepts only when `b*` lies outside it. An outcome its checker rejects becomes
//! `Indeterminate { "checker rejected" }`. Both checkers share [`Convention::interval`] and the construction
//! rules with the solver; the certificate checker also shares the linearisation.
//!
//! Certificate multipliers come from the simplex duals of the rows. With the duals `p` of the phase that
//! stopped (phase 1 when the closed box is infeasible, the `t` phase when only open ends fail), `λ = p` gives
//! `λᵀb ≥ sup cᵀx` over the box, with the supremum either exceeded or reached only at an open end. Multipliers
//! are scaled so the first non-zero one, in row order, is ±1.
//!
//! [`implied_interval`] gives the exact interval a cell can take in its component, optionally with its own box
//! dropped (a dropped denominator keeps its printed sign). Each end is an exact optimum over the closure, then
//! re-decided by checked feasibility runs: no point beyond the end (a checked certificate), and the end itself
//! reached (a checked witness: a closed end) or not (a checked certificate: an open end).
//!
//! [`blame`] (bd-4wvl.3, BD-340.S3) says which printed values an infeasible component's conflict rests on:
//! inclusion-minimal conflict sets with checked certificates, and correction sets (labelled `Minimum` only after
//! an exhaustive search) with each cell's implied range, never a corrected value. [`presence_conflicts`] covers a
//! number set against a placeholder, which no interval can express.
//!
//! K0: std only, no I/O. Determinism: exact rational arithmetic, inputs sorted by locator and id before solving.

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use fmine_core::{Encoder, blake3_hash};
use fmine_units::{ExactQty, UnitsError};

/// An exact rational.
pub type Rat = ExactQty;

/// At most this many cells in one component; more gives `Indeterminate`.
pub const MAX_VARIABLES: usize = 256;
/// At most this many simplex pivots per run; more gives `Indeterminate`.
pub const MAX_PIVOTS: usize = 10_000;

const ZERO: Rat = ExactQty::from_integer(0);
const ONE: Rat = ExactQty::from_integer(1);
const MINUS_ONE: Rat = ExactQty::from_integer(-1);
const OVERFLOW: &str = "exact arithmetic overflowed";

/// Where a cell's printed value was read.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Scope {
    Visible,
    Hidden,
    Pdf,
}

/// One printed value. The half-width is `step / 2`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cell {
    /// `<file>:<sheet>!<A1>` or `<doc>:p<page>:<row>/<col>`.
    pub locator: String,
    pub sha256: String,
    /// The text as printed.
    pub token: String,
    pub printed: Rat,
    /// The rounding step; it must be positive. An unknown step is refused by the caller, never guessed here.
    pub step: Rat,
    pub scope: Scope,
}

/// How a true value maps to its printed value at step `2h`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Convention {
    /// `[v−h, v+h]`: agnostic to the tie rule (the default).
    Closed,
    /// `[v−h, v+h)`: a tie rounds up.
    HalfUp,
    /// `(v−h, v+h]`: a tie rounds down.
    HalfDown,
    /// A tie goes to the even neighbour: closed when `v/(2h)` is even, open both ends when it is odd.
    HalfEven,
    /// Toward zero: `[v, v+2h)` for `v > 0`, `(v−2h, v]` for `v < 0`, `(−2h, 2h)` for `v = 0`.
    Truncate,
}

/// An interval with its open ends flagged.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Interval {
    pub lo: Rat,
    pub hi: Rat,
    pub lo_strict: bool,
    pub hi_strict: bool,
}

impl Interval {
    /// Whether `x` lies in the interval, exactly.
    pub fn contains(&self, x: Rat) -> Result<bool, UnitsError> {
        Bound::of(*self).contains(x)
    }
}

/// Why an interval could not be formed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntervalError {
    /// The arithmetic overflowed: the component is `Indeterminate`.
    Overflow,
    /// Half-even needs `v` on the step grid to know its parity.
    OffGrid,
}

impl From<UnitsError> for IntervalError {
    fn from(_: UnitsError) -> Self {
        Self::Overflow
    }
}

impl Convention {
    /// The true values that print as `v` at half-width `h`.
    pub fn interval(self, v: Rat, h: Rat) -> Result<Interval, IntervalError> {
        let (lo, hi) = (v.checked_sub(h)?, v.checked_add(h)?);
        let closed = |lo_strict, hi_strict| Interval {
            lo,
            hi,
            lo_strict,
            hi_strict,
        };
        Ok(match self {
            Self::Closed => closed(false, false),
            Self::HalfUp => closed(false, true),
            Self::HalfDown => closed(true, false),
            Self::HalfEven => {
                let k = v.checked_div(h.checked_mul(Rat::from_integer(2))?)?;
                if k.den() != 1 {
                    return Err(IntervalError::OffGrid);
                }
                let odd = k.num() % 2 != 0;
                closed(odd, odd)
            }
            Self::Truncate => {
                let step = h.checked_mul(Rat::from_integer(2))?;
                match v.signum() {
                    1 => Interval {
                        lo: v,
                        hi: v.checked_add(step)?,
                        lo_strict: false,
                        hi_strict: true,
                    },
                    -1 => Interval {
                        lo: v.checked_sub(step)?,
                        hi: v,
                        lo_strict: true,
                        hi_strict: false,
                    },
                    _ => Interval {
                        lo: step.checked_neg()?,
                        hi: step,
                        lo_strict: true,
                        hi_strict: true,
                    },
                }
            }
        })
    }

    const fn tag(self) -> u8 {
        match self {
            Self::Closed => 0,
            Self::HalfUp => 1,
            Self::HalfDown => 2,
            Self::HalfEven => 3,
            Self::Truncate => 4,
        }
    }
}

/// One end of a range: where it is and whether it is excluded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct End {
    pub at: Rat,
    pub strict: bool,
}

/// A range whose ends may be infinite (`None`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Bound {
    pub lo: Option<End>,
    pub hi: Option<End>,
}

impl Bound {
    /// No end at all.
    pub const FREE: Self = Self { lo: None, hi: None };

    #[must_use]
    pub const fn of(iv: Interval) -> Self {
        Self {
            lo: Some(End {
                at: iv.lo,
                strict: iv.lo_strict,
            }),
            hi: Some(End {
                at: iv.hi,
                strict: iv.hi_strict,
            }),
        }
    }

    /// Whether `x` lies in the range, exactly.
    pub fn contains(&self, x: Rat) -> Result<bool, UnitsError> {
        if let Some(lo) = self.lo {
            match x.checked_cmp(lo.at)? {
                Ordering::Less => return Ok(false),
                Ordering::Equal if lo.strict => return Ok(false),
                _ => {}
            }
        }
        if let Some(hi) = self.hi {
            match x.checked_cmp(hi.at)? {
                Ordering::Greater => return Ok(false),
                Ordering::Equal if hi.strict => return Ok(false),
                _ => {}
            }
        }
        Ok(true)
    }

    fn is_empty(&self) -> Result<bool, UnitsError> {
        match (self.lo, self.hi) {
            (Some(lo), Some(hi)) => Ok(match lo.at.checked_cmp(hi.at)? {
                Ordering::Greater => true,
                Ordering::Equal => lo.strict || hi.strict,
                Ordering::Less => false,
            }),
            _ => Ok(false),
        }
    }

    /// The intersection.
    fn meet(self, other: Self) -> Result<Self, UnitsError> {
        let tighter =
            |a: Option<End>, b: Option<End>, lower: bool| -> Result<Option<End>, UnitsError> {
                Ok(match (a, b) {
                    (None, x) | (x, None) => x,
                    (Some(a), Some(b)) => Some(match a.at.checked_cmp(b.at)? {
                        Ordering::Equal => End {
                            at: a.at,
                            strict: a.strict || b.strict,
                        },
                        Ordering::Less => {
                            if lower {
                                b
                            } else {
                                a
                            }
                        }
                        Ordering::Greater => {
                            if lower {
                                a
                            } else {
                                b
                            }
                        }
                    }),
                })
            };
        Ok(Self {
            lo: tighter(self.lo, other.lo, true)?,
            hi: tighter(self.hi, other.hi, false)?,
        })
    }
}

/// `1` when every value of the range is positive, `-1` when every one is negative, `0` otherwise.
fn one_sign(b: Bound) -> Result<i8, UnitsError> {
    if let Some(lo) = b.lo {
        let s = lo.at.signum();
        if s > 0 || (s == 0 && lo.strict) {
            return Ok(1);
        }
    }
    if let Some(hi) = b.hi {
        let s = hi.at.signum();
        if s < 0 || (s == 0 && hi.strict) {
            return Ok(-1);
        }
    }
    Ok(0)
}

/// How a family's printed totals relate to its printed lines (S2). Registry data per family, declared before
/// any run; the kernel never chooses one per instance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Construction {
    /// The issuer adds its rounded lines: the printed total equals Σ ± printed lines exactly, and the true total
    /// is Σ ± true lines, so the total's own printed box does not bind.
    ComponentSum,
    /// The issuer rounds the true total once: the total lies in its own printed box.
    RoundedOnce,
}

impl Construction {
    const fn tag(self) -> u8 {
        match self {
            Self::ComponentSum => 0,
            Self::RoundedOnce => 1,
        }
    }
}

/// What a run assumes: the tie convention of every cell, and the construction of every family with `Sum`
/// identities. A `Sum` whose family has none is refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rules {
    pub convention: Convention,
    pub constructions: BTreeMap<String, Construction>,
}

impl Rules {
    #[must_use]
    pub const fn new(convention: Convention) -> Self {
        Self {
            convention,
            constructions: BTreeMap::new(),
        }
    }

    /// Declares `family`'s construction.
    #[must_use]
    pub fn with(mut self, family: &str, construction: Construction) -> Self {
        self.constructions.insert(family.to_string(), construction);
        self
    }

    fn construction(&self, identity: &Identity) -> Result<Construction, XfRefusal> {
        self.constructions
            .get(&identity.family)
            .copied()
            .ok_or_else(|| XfRefusal::NoConstruction {
                identity: identity.id.clone(),
                family: identity.family.clone(),
            })
    }
}

/// Whether an identity is closed by published cells alone.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Closure {
    /// The identity holds over the printed cells by definition: a conflict is a discrepancy.
    Definitional,
    /// The issuer may use unpublished quantities: a conflict names a missing fact.
    MayUseUnpublished,
}

/// The identity kinds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IdentityKind {
    /// `Σ signᵢ·lineᵢ = total`, each sign ±1.
    Sum {
        total: String,
        lines: Vec<String>,
        signs: Vec<i8>,
    },
    /// `a = b`: the same quantity across sheets or vintages.
    Equal { a: String, b: String },
    /// `target = factor·source`: an ownership share or a unit factor.
    Scale {
        target: String,
        source: String,
        factor: Rat,
    },
    /// `rate = factor·num/den` (the bead's `k`): a per-ounce rate is `1000·$M/koz` (S2).
    Ratio {
        rate: String,
        num: String,
        den: String,
        factor: Rat,
    },
}

/// One identity over cells.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Identity {
    pub id: String,
    pub family: String,
    pub closure: Closure,
    pub kind: IdentityKind,
}

/// A linear row `Σ coefficients·x = rhs`, coefficients by locator (none zero).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    pub coefficients: BTreeMap<String, Rat>,
    pub rhs: Rat,
}

impl Identity {
    /// The row of a linear identity: `Σ signᵢ·lineᵢ − total`, `a − b` or `target − factor·source`, all `= 0`.
    /// Coefficients of a repeated locator are added; a row whose coefficients all cancel is refused as
    /// degenerate. A `Ratio` has no single row (its linearisation depends on the rate's interval).
    pub fn row(&self) -> Result<Row, XfRefusal> {
        let degenerate = || XfRefusal::Degenerate {
            identity: self.id.clone(),
        };
        let mut terms: Vec<(&str, Rat)> = Vec::new();
        match &self.kind {
            IdentityKind::Sum {
                total,
                lines,
                signs,
            } => {
                if lines.len() != signs.len() || signs.iter().any(|s| s.abs() != 1) {
                    return Err(XfRefusal::BadSigns {
                        identity: self.id.clone(),
                    });
                }
                for (line, s) in lines.iter().zip(signs) {
                    terms.push((line, Rat::from_integer(i128::from(*s))));
                }
                terms.push((total, MINUS_ONE));
            }
            IdentityKind::Equal { a, b } => {
                terms.push((a, ONE));
                terms.push((b, MINUS_ONE));
            }
            IdentityKind::Scale {
                target,
                source,
                factor,
            } => {
                terms.push((target, ONE));
                terms.push((source, factor.checked_neg().map_err(|_| degenerate())?));
            }
            IdentityKind::Ratio { .. } => {
                return Err(XfRefusal::NotLinear {
                    identity: self.id.clone(),
                });
            }
        }
        let mut coefficients: BTreeMap<String, Rat> = BTreeMap::new();
        for (loc, a) in terms {
            let entry = coefficients.entry(loc.to_string()).or_insert(ZERO);
            *entry = entry.checked_add(a).map_err(|_| degenerate())?;
        }
        coefficients.retain(|_, a| *a != ZERO);
        if coefficients.is_empty() {
            return Err(degenerate());
        }
        Ok(Row {
            coefficients,
            rhs: ZERO,
        })
    }

    /// Every locator the identity names.
    fn locators(&self) -> Vec<&str> {
        match &self.kind {
            IdentityKind::Sum { total, lines, .. } => lines
                .iter()
                .map(String::as_str)
                .chain([total.as_str()])
                .collect(),
            IdentityKind::Equal { a, b } => vec![a.as_str(), b.as_str()],
            IdentityKind::Scale { target, source, .. } => vec![target.as_str(), source.as_str()],
            IdentityKind::Ratio { rate, num, den, .. } => {
                vec![rate.as_str(), num.as_str(), den.as_str()]
            }
        }
    }

    /// The refusals that need no cell values.
    fn validate(&self, rules: &Rules) -> Result<(), XfRefusal> {
        match &self.kind {
            IdentityKind::Ratio {
                rate,
                num,
                den,
                factor,
            } => {
                if rate == num || rate == den || num == den || *factor == ZERO {
                    return Err(XfRefusal::Degenerate {
                        identity: self.id.clone(),
                    });
                }
                Ok(())
            }
            IdentityKind::Sum { .. } => {
                rules.construction(self)?;
                self.row().map(|_| ())
            }
            _ => self.row().map(|_| ()),
        }
    }

    const fn closure_tag(&self) -> u8 {
        match self.closure {
            Closure::Definitional => 0,
            Closure::MayUseUnpublished => 1,
        }
    }
}

/// Input the kernel refuses rather than solve.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum XfRefusal {
    NonPositiveStep {
        locator: String,
    },
    OffGrid {
        locator: String,
    },
    DuplicateCell {
        locator: String,
    },
    DuplicateIdentity {
        id: String,
    },
    UnknownCell {
        identity: String,
        locator: String,
    },
    BadSigns {
        identity: String,
    },
    Degenerate {
        identity: String,
    },
    /// A `Sum` whose family has no declared construction.
    NoConstruction {
        identity: String,
        family: String,
    },
    /// A ratio's denominator interval contains zero.
    DenominatorSpansZero {
        identity: String,
        locator: String,
    },
    /// [`Identity::row`] of a `Ratio`.
    NotLinear {
        identity: String,
    },
    /// [`implied_interval`] of a locator that is not a given cell.
    UnknownVariable {
        locator: String,
    },
    /// [`presence_conflicts`]: a placeholder in an identity other than `Equal`.
    PlaceholderInIdentity {
        identity: String,
        locator: String,
    },
}

impl fmt::Display for XfRefusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NonPositiveStep { locator } => write!(f, "{locator}: the step must be positive"),
            Self::OffGrid { locator } => {
                write!(
                    f,
                    "{locator}: half-even needs the printed value on the step grid"
                )
            }
            Self::DuplicateCell { locator } => write!(f, "{locator} is given twice"),
            Self::DuplicateIdentity { id } => write!(f, "identity {id} is given twice"),
            Self::UnknownCell { identity, locator } => {
                write!(
                    f,
                    "identity {identity} names {locator}, which is not a given cell"
                )
            }
            Self::BadSigns { identity } => {
                write!(f, "identity {identity}: one sign of ±1 per line is needed")
            }
            Self::Degenerate { identity } => write!(f, "identity {identity} has no terms"),
            Self::NoConstruction { identity, family } => write!(
                f,
                "identity {identity}: family {family} has no declared construction"
            ),
            Self::DenominatorSpansZero { identity, locator } => write!(
                f,
                "identity {identity}: the interval of the denominator {locator} contains zero"
            ),
            Self::NotLinear { identity } => write!(f, "identity {identity} is a ratio, not a row"),
            Self::UnknownVariable { locator } => write!(f, "{locator} is not a given cell"),
            Self::PlaceholderInIdentity { identity, locator } => write!(
                f,
                "identity {identity}: the placeholder {locator} can only be in an Equal identity"
            ),
        }
    }
}

/// Why no assignment exists.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Certificate {
    /// Farkas multipliers by row id (an identity id, or `<id>#lo` / `<id>#hi` for a ratio's two rows), none
    /// zero.
    Farkas { multipliers: Vec<(String, Rat)> },
    /// A `ComponentSum` identity whose printed total minus Σ ± printed lines is `residual`, not zero.
    Residual { identity: String, residual: Rat },
}

/// The result for one component.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// A checked point: every cell in its interval, every identity exact.
    Feasible { witness: BTreeMap<String, Rat> },
    /// A checked certificate that no such point exists.
    Infeasible { certificate: Certificate },
    /// A relaxation found no conflict; never reported as consistent (S6).
    NotRefuted,
    /// No claim either way.
    Indeterminate { reason: String },
}

/// One connected component of cells and the identities linking them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Component {
    /// Sorted.
    pub cells: Vec<String>,
    /// Sorted.
    pub identities: Vec<String>,
    pub outcome: Outcome,
}

/// Solves every component of the problem.
pub fn solve(
    cells: &[Cell],
    identities: &[Identity],
    rules: &Rules,
) -> Result<Vec<Component>, XfRefusal> {
    solve_with_caps(cells, identities, rules, MAX_VARIABLES, MAX_PIVOTS)
}

/// [`solve`] with explicit caps.
pub fn solve_with_caps(
    cells: &[Cell],
    identities: &[Identity],
    rules: &Rules,
    max_variables: usize,
    max_pivots: usize,
) -> Result<Vec<Component>, XfRefusal> {
    let p = prepare(cells, identities, rules)?;
    let mut out = Vec::with_capacity(p.groups.len());
    for (cell_ix, id_ix) in &p.groups {
        let comp_cells: Vec<&Cell> = cell_ix.iter().map(|&i| p.cells[i]).collect();
        let comp_ids: Vec<&Identity> = id_ix.iter().map(|&i| p.identities[i]).collect();
        let outcome = if comp_cells.len() > max_variables {
            Outcome::Indeterminate {
                reason: format!(
                    "the component has {} cells, over the cap of {max_variables}",
                    comp_cells.len()
                ),
            }
        } else {
            decide(&comp_cells, &comp_ids, rules, max_pivots)?
        };
        out.push(Component {
            cells: comp_cells.iter().map(|c| c.locator.clone()).collect(),
            identities: comp_ids.iter().map(|i| i.id.clone()).collect(),
            outcome,
        });
    }
    Ok(out)
}

/// The problem sorted and validated, with its components as (cell indices, identity indices).
struct Prepared<'a> {
    cells: Vec<&'a Cell>,
    identities: Vec<&'a Identity>,
    groups: Vec<(Vec<usize>, Vec<usize>)>,
}

fn prepare<'a>(
    cells: &'a [Cell],
    identities: &'a [Identity],
    rules: &Rules,
) -> Result<Prepared<'a>, XfRefusal> {
    let mut cells: Vec<&Cell> = cells.iter().collect();
    cells.sort_by(|a, b| a.locator.cmp(&b.locator));
    let mut identities: Vec<&Identity> = identities.iter().collect();
    identities.sort_by(|a, b| a.id.cmp(&b.id));
    for w in cells.windows(2) {
        if w[0].locator == w[1].locator {
            return Err(XfRefusal::DuplicateCell {
                locator: w[0].locator.clone(),
            });
        }
    }
    for w in identities.windows(2) {
        if w[0].id == w[1].id {
            return Err(XfRefusal::DuplicateIdentity {
                id: w[0].id.clone(),
            });
        }
    }
    let index: BTreeMap<&str, usize> = cells
        .iter()
        .enumerate()
        .map(|(i, c)| (c.locator.as_str(), i))
        .collect();
    // Step and grid refusals up front, for every cell; an overflow is left to its component.
    let mut bounds = Vec::with_capacity(cells.len());
    for c in &cells {
        bounds.push(cell_bound(c, rules.convention)?);
    }
    for id in &identities {
        id.validate(rules)?;
        for loc in id.locators() {
            if !index.contains_key(loc) {
                return Err(XfRefusal::UnknownCell {
                    identity: id.id.clone(),
                    locator: loc.to_string(),
                });
            }
        }
        if let IdentityKind::Ratio { den, .. } = &id.kind
            && let Some(b) = bounds[index[den.as_str()]]
            && matches!(one_sign(b), Ok(0))
        {
            return Err(XfRefusal::DenominatorSpansZero {
                identity: id.id.clone(),
                locator: den.clone(),
            });
        }
    }

    // Components by union-find over cells linked by identities.
    fn find(parent: &mut [usize], mut i: usize) -> usize {
        while parent[i] != i {
            parent[i] = parent[parent[i]];
            i = parent[i];
        }
        i
    }
    let mut parent: Vec<usize> = (0..cells.len()).collect();
    for id in &identities {
        let mut locs = id.locators().into_iter().map(|l| index[l]);
        if let Some(first) = locs.next() {
            for other in locs {
                let (a, b) = (find(&mut parent, first), find(&mut parent, other));
                if a != b {
                    parent[a.max(b)] = a.min(b);
                }
            }
        }
    }
    let mut groups: BTreeMap<usize, (Vec<usize>, Vec<usize>)> = BTreeMap::new();
    for i in 0..cells.len() {
        let root = find(&mut parent, i);
        groups.entry(root).or_default().0.push(i);
    }
    for (r, id) in identities.iter().enumerate() {
        if let Some(loc) = id.locators().first() {
            let root = find(&mut parent, index[loc]);
            groups.entry(root).or_default().1.push(r);
        }
    }
    Ok(Prepared {
        cells,
        identities,
        groups: groups.into_values().collect(),
    })
}

/// A cell's printed box, `None` when forming it overflowed.
fn cell_bound(c: &Cell, convention: Convention) -> Result<Option<Bound>, XfRefusal> {
    if c.step.signum() <= 0 {
        return Err(XfRefusal::NonPositiveStep {
            locator: c.locator.clone(),
        });
    }
    let Ok(h) = c.step.checked_div(Rat::from_integer(2)) else {
        return Ok(None);
    };
    match convention.interval(c.printed, h) {
        Ok(iv) => Ok(Some(Bound::of(iv))),
        Err(IntervalError::Overflow) => Ok(None),
        Err(IntervalError::OffGrid) => Err(XfRefusal::OffGrid {
            locator: c.locator.clone(),
        }),
    }
}

fn indeterminate(reason: impl Into<String>) -> Outcome {
    Outcome::Indeterminate {
        reason: reason.into(),
    }
}

fn rejected() -> Outcome {
    indeterminate("checker rejected")
}

fn lp_indeterminate(e: LpError, max_pivots: usize) -> Outcome {
    match e {
        LpError::Overflow => indeterminate(OVERFLOW),
        LpError::PivotCap => indeterminate(format!("over the cap of {max_pivots} pivots")),
        LpError::Unbounded => indeterminate("the simplex reported an unbounded phase"),
    }
}

/// One component's outcome, reported only through its checker.
fn decide(
    cells: &[&Cell],
    identities: &[&Identity],
    rules: &Rules,
    max_pivots: usize,
) -> Result<Outcome, XfRefusal> {
    let none = Frame::default();
    let lin = match linearise(cells, identities, rules, &none)? {
        Lin::Indeterminate(reason) => return Ok(indeterminate(reason)),
        Lin::Residual { identity, residual } => {
            let certificate = Certificate::Residual { identity, residual };
            return Ok(
                match check_certificate_in(cells, identities, rules, &none, &certificate) {
                    Ok(_) => Outcome::Infeasible { certificate },
                    Err(_) => rejected(),
                },
            );
        }
        Lin::System(lin) => lin,
    };
    Ok(match feasibility(&lin, max_pivots) {
        Ok(Feasibility::Feasible(values)) => match witness_of(&lin, &values) {
            Ok(witness) => match check_witness_in(cells, identities, rules, &none, &witness) {
                Ok(()) => Outcome::Feasible { witness },
                Err(_) => rejected(),
            },
            Err(_) => indeterminate(OVERFLOW),
        },
        Ok(Feasibility::Infeasible(multipliers)) => {
            let certificate = Certificate::Farkas { multipliers };
            match check_certificate_in(cells, identities, rules, &none, &certificate) {
                Ok(_) => Outcome::Infeasible { certificate },
                Err(_) => rejected(),
            }
        }
        Err(e) => lp_indeterminate(e, max_pivots),
    })
}

/// One row of a linearised system: `Σ terms = rhs`, terms by variable index.
#[derive(Debug, Clone)]
struct LinRow {
    id: String,
    terms: Vec<(usize, Rat)>,
    rhs: Rat,
}

/// A component as linear rows over variables with ranges: the cells first, then one slack per ratio side.
#[derive(Debug, Clone)]
struct Linear {
    names: Vec<String>,
    bounds: Vec<Bound>,
    rows: Vec<LinRow>,
    /// Each ratio as (rate, num, den, factor) by variable index; the rate is eliminated from the rows.
    ratios: Vec<(usize, usize, usize, Rat)>,
    cells: usize,
}

enum Lin {
    System(Linear),
    Residual { identity: String, residual: Rat },
    Indeterminate(String),
}

/// The linearisation the solver and the certificate checker share. `overrides` replace cells' ranges after the
/// construction rules apply (used by [`implied_interval`]).
/// Whether `loc` is the denominator of a ratio.
fn is_den(identities: &[&Identity], loc: &str) -> bool {
    identities
        .iter()
        .any(|id| matches!(&id.kind, IdentityKind::Ratio { den, .. } if den == loc))
}

/// The range of a cell whose printed value is disregarded: free, except that a ratio's denominator keeps the
/// sign of its printed box, so the ratio stays linear.
fn freed_bound(printed: Bound, den: bool) -> Bound {
    let zero_open = Some(End {
        at: ZERO,
        strict: true,
    });
    match (den, one_sign(printed)) {
        (true, Ok(1)) => Bound {
            lo: zero_open,
            hi: None,
        },
        (true, Ok(-1)) => Bound {
            lo: None,
            hi: zero_open,
        },
        _ => Bound::FREE,
    }
}

/// What one run changes from the printed table. A cell in `unprinted` has its printed value disregarded: it has no
/// box, and no printed footing over it is required. `bounds` replaces a variable's range (an unprinted cell's, or a
/// range pinned by `implied_interval`).
#[derive(Debug, Clone, Default)]
struct Frame {
    unprinted: BTreeSet<String>,
    bounds: BTreeMap<String, Bound>,
}

impl Frame {
    /// Whether the identity has a cell whose printed value is disregarded.
    fn touches(&self, id: &Identity) -> bool {
        id.locators().iter().any(|l| self.unprinted.contains(*l))
    }

    /// This frame with `var`'s range replaced.
    fn pinned(&self, var: &str, bound: Bound) -> Self {
        let mut f = self.clone();
        f.bounds.insert(var.to_string(), bound);
        f
    }
}

fn linearise(
    cells: &[&Cell],
    identities: &[&Identity],
    rules: &Rules,
    frame: &Frame,
) -> Result<Lin, XfRefusal> {
    let mut index: BTreeMap<&str, usize> = BTreeMap::new();
    let mut printed: BTreeMap<&str, Rat> = BTreeMap::new();
    let mut bounds = Vec::with_capacity(cells.len());
    for (j, c) in cells.iter().enumerate() {
        if index.insert(c.locator.as_str(), j).is_some() {
            return Err(XfRefusal::DuplicateCell {
                locator: c.locator.clone(),
            });
        }
        printed.insert(c.locator.as_str(), c.printed);
        match cell_bound(c, rules.convention)? {
            Some(b) => bounds.push(b),
            None => return Ok(Lin::Indeterminate(OVERFLOW.to_string())),
        }
    }
    let mut uses: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    for id in identities {
        id.validate(rules)?;
        for loc in id.locators() {
            if !index.contains_key(loc) {
                return Err(XfRefusal::UnknownCell {
                    identity: id.id.clone(),
                    locator: loc.to_string(),
                });
            }
            uses.entry(loc).or_default().insert(id.id.as_str());
        }
    }
    // ComponentSum: the printed footing first (unless a cell's printed value is disregarded); then the total's own
    // box is dropped.
    let mut freed = Vec::new();
    for id in identities {
        if let IdentityKind::Sum { total, .. } = &id.kind
            && rules.construction(id)? == Construction::ComponentSum
        {
            if frame.touches(id) {
                freed.push(index[total.as_str()]);
                continue;
            }
            let Ok(gap) = printed_value(&id.row()?, &printed) else {
                return Ok(Lin::Indeterminate(OVERFLOW.to_string()));
            };
            if gap != ZERO {
                let Ok(residual) = gap.checked_neg() else {
                    return Ok(Lin::Indeterminate(OVERFLOW.to_string()));
                };
                return Ok(Lin::Residual {
                    identity: id.id.clone(),
                    residual,
                });
            }
            freed.push(index[total.as_str()]);
        }
    }
    let boxes = bounds.clone();
    for j in freed {
        bounds[j] = Bound::FREE;
    }
    for loc in &frame.unprinted {
        let Some(&j) = index.get(loc.as_str()) else {
            return Err(XfRefusal::UnknownVariable {
                locator: loc.clone(),
            });
        };
        bounds[j] = freed_bound(boxes[j], is_den(identities, loc));
    }
    for (loc, b) in &frame.bounds {
        let Some(&j) = index.get(loc.as_str()) else {
            return Err(XfRefusal::UnknownVariable {
                locator: loc.clone(),
            });
        };
        bounds[j] = *b;
    }
    let mut names: Vec<String> = cells.iter().map(|c| c.locator.clone()).collect();
    let mut rows = Vec::new();
    let mut ratios = Vec::new();
    for id in identities {
        let IdentityKind::Ratio {
            rate,
            num,
            den,
            factor,
        } = &id.kind
        else {
            let row = id.row()?;
            rows.push(LinRow {
                id: id.id.clone(),
                terms: row
                    .coefficients
                    .iter()
                    .map(|(l, a)| (index[l.as_str()], *a))
                    .collect(),
                rhs: row.rhs,
            });
            continue;
        };
        if uses[rate.as_str()].len() > 1 {
            return Ok(Lin::Indeterminate(format!(
                "bilinear: the rate cell {rate} of {} occurs in another identity",
                id.id
            )));
        }
        let (r, n, d) = (
            index[rate.as_str()],
            index[num.as_str()],
            index[den.as_str()],
        );
        // A missing end (a rate range overridden by `implied_interval`) gives no row on that side.
        let (r_lo, r_hi) = (bounds[r].lo, bounds[r].hi);
        let Ok(sign) = one_sign(bounds[d]) else {
            return Ok(Lin::Indeterminate(OVERFLOW.to_string()));
        };
        if sign == 0 {
            if let Some(b) = cell_bound(cells[d], rules.convention)?
                && matches!(one_sign(b), Ok(0))
            {
                return Err(XfRefusal::DenominatorSpansZero {
                    identity: id.id.clone(),
                    locator: den.clone(),
                });
            }
            return Ok(Lin::Indeterminate(format!(
                "the denominator {den} of {} is not of one sign",
                id.id
            )));
        }
        // The rate is eliminated: the two rows below hold it in its interval.
        bounds[r] = Bound::FREE;
        for (side, end, lower) in [("lo", r_lo, true), ("hi", r_hi, false)] {
            let Some(end) = end else {
                continue;
            };
            // s·(factor·num − end·den) − σ = 0, σ ≥ 0: s = +1 on the lower side of a positive denominator.
            let s = if (sign > 0) == lower { ONE } else { MINUS_ONE };
            let (Ok(a_num), Ok(a_den)) = (
                s.checked_mul(*factor),
                s.checked_mul(end.at).and_then(Rat::checked_neg),
            ) else {
                return Ok(Lin::Indeterminate(OVERFLOW.to_string()));
            };
            let slack = names.len();
            let row_id = format!("{}#{side}", id.id);
            names.push(row_id.clone());
            bounds.push(Bound {
                lo: Some(End {
                    at: ZERO,
                    strict: end.strict,
                }),
                hi: None,
            });
            let mut terms = vec![(n, a_num), (d, a_den), (slack, MINUS_ONE)];
            terms.retain(|(_, a)| *a != ZERO);
            rows.push(LinRow {
                id: row_id,
                terms,
                rhs: ZERO,
            });
        }
        ratios.push((r, n, d, *factor));
    }
    Ok(Lin::System(Linear {
        names,
        bounds,
        rows,
        ratios,
        cells: cells.len(),
    }))
}

/// `Σ coefficients·printed` of a row.
fn printed_value(row: &Row, printed: &BTreeMap<&str, Rat>) -> Result<Rat, UnitsError> {
    let mut v = ZERO;
    for (loc, a) in &row.coefficients {
        let x = printed
            .get(loc.as_str())
            .copied()
            .ok_or(UnitsError::Overflow)?;
        v = v.checked_add(a.checked_mul(x)?)?;
    }
    Ok(v)
}

/// A witness from the variables' values: the cells, with each eliminated rate set to `factor·num/den`.
fn witness_of(lin: &Linear, values: &[Rat]) -> Result<BTreeMap<String, Rat>, UnitsError> {
    let mut w: BTreeMap<String, Rat> = lin.names[..lin.cells]
        .iter()
        .cloned()
        .zip(values.iter().copied())
        .collect();
    for &(r, n, d, factor) in &lin.ratios {
        let rate = factor.checked_mul(values[n])?.checked_div(values[d])?;
        w.insert(lin.names[r].clone(), rate);
    }
    Ok(w)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LpError {
    Overflow,
    PivotCap,
    Unbounded,
}

impl From<UnitsError> for LpError {
    fn from(_: UnitsError) -> Self {
        Self::Overflow
    }
}

/// How a variable maps to non-negative columns.
#[derive(Debug, Clone, Copy)]
enum Column {
    /// `x = offset + col` (from a finite lower end) or `x = offset − col` (from a finite upper end only).
    Shift { offset: Rat, up: bool, col: usize },
    /// `x = pos − neg`: no finite end.
    Split { pos: usize, neg: usize },
}

/// A linear system in standard form: `A z = b, z ≥ 0`, the system's rows first.
struct Standard {
    rows: Vec<Vec<Rat>>,
    rhs: Vec<Rat>,
    n_struct: usize,
    maps: Vec<Column>,
    /// The `t` column, when open ends are kept strict.
    t: Option<usize>,
}

fn standardize(lin: &Linear, strict: bool) -> Result<Standard, LpError> {
    let mut n = 0;
    let mut maps = Vec::with_capacity(lin.bounds.len());
    let mut boxes: Vec<(usize, usize, Rat)> = Vec::new();
    // Columns that must stay at least t above zero: one per open end.
    let mut open: Vec<usize> = Vec::new();
    for b in &lin.bounds {
        match (b.lo, b.hi) {
            (Some(lo), hi) => {
                let col = n;
                n += 1;
                maps.push(Column::Shift {
                    offset: lo.at,
                    up: true,
                    col,
                });
                if lo.strict {
                    open.push(col);
                }
                if let Some(hi) = hi {
                    let s = n;
                    n += 1;
                    boxes.push((col, s, hi.at.checked_sub(lo.at)?));
                    if hi.strict {
                        open.push(s);
                    }
                }
            }
            (None, Some(hi)) => {
                let col = n;
                n += 1;
                maps.push(Column::Shift {
                    offset: hi.at,
                    up: false,
                    col,
                });
                if hi.strict {
                    open.push(col);
                }
            }
            (None, None) => {
                maps.push(Column::Split { pos: n, neg: n + 1 });
                n += 2;
            }
        }
    }
    let strict = strict && !open.is_empty();
    let t = strict.then_some(n);
    let first_u = n + 1;
    let r_col = first_u + open.len();
    let n_struct = if strict { r_col + 1 } else { n };

    let mut rows = Vec::new();
    let mut rhs = Vec::new();
    for row in &lin.rows {
        let mut line = vec![ZERO; n_struct];
        let mut b = row.rhs;
        for &(j, a) in &row.terms {
            match maps[j] {
                Column::Shift { offset, up, col } => {
                    let a_col = if up { a } else { a.checked_neg()? };
                    line[col] = line[col].checked_add(a_col)?;
                    b = b.checked_sub(a.checked_mul(offset)?)?;
                }
                Column::Split { pos, neg } => {
                    line[pos] = line[pos].checked_add(a)?;
                    line[neg] = line[neg].checked_sub(a)?;
                }
            }
        }
        rows.push(line);
        rhs.push(b);
    }
    for &(col, s, width) in &boxes {
        let mut line = vec![ZERO; n_struct];
        line[col] = ONE;
        line[s] = ONE;
        rows.push(line);
        rhs.push(width);
    }
    if let Some(t) = t {
        for (k, &c) in open.iter().enumerate() {
            let mut line = vec![ZERO; n_struct];
            line[c] = ONE;
            line[t] = MINUS_ONE;
            line[first_u + k] = MINUS_ONE;
            rows.push(line);
            rhs.push(ZERO);
        }
        let mut line = vec![ZERO; n_struct];
        line[t] = ONE;
        line[r_col] = ONE;
        rows.push(line);
        rhs.push(ONE);
    }
    Ok(Standard {
        rows,
        rhs,
        n_struct,
        maps,
        t,
    })
}

fn variable_value(maps: &[Column], tab: &Tableau, j: usize) -> Result<Rat, LpError> {
    Ok(match maps[j] {
        Column::Shift { offset, up, col } => {
            if up {
                offset.checked_add(tab.value(col))?
            } else {
                offset.checked_sub(tab.value(col))?
            }
        }
        Column::Split { pos, neg } => tab.value(pos).checked_sub(tab.value(neg))?,
    })
}

/// A dense simplex tableau for `min cᵀz, A z = b, z ≥ 0`, `b ≥ 0` after row flips, with one artificial column
/// per row after the structural ones (artificial `i` is column `n_struct + i`).
struct Tableau {
    a: Vec<Vec<Rat>>,
    b: Vec<Rat>,
    basis: Vec<usize>,
    /// Rows negated to make `b ≥ 0`.
    flipped: Vec<bool>,
    n_struct: usize,
    pivots: usize,
    max_pivots: usize,
}

impl Tableau {
    fn new(
        rows: Vec<Vec<Rat>>,
        rhs: Vec<Rat>,
        n_struct: usize,
        max_pivots: usize,
    ) -> Result<Self, LpError> {
        let m = rows.len();
        let mut a = Vec::with_capacity(m);
        let mut b = Vec::with_capacity(m);
        let mut flipped = Vec::with_capacity(m);
        for (i, (mut row, r)) in rows.into_iter().zip(rhs).enumerate() {
            let flip = r.signum() < 0;
            if flip {
                for v in &mut row {
                    *v = v.checked_neg()?;
                }
            }
            row.extend((0..m).map(|k| if k == i { ONE } else { ZERO }));
            a.push(row);
            b.push(if flip { r.checked_neg()? } else { r });
            flipped.push(flip);
        }
        Ok(Self {
            a,
            b,
            basis: (n_struct..n_struct + m).collect(),
            flipped,
            n_struct,
            pivots: 0,
            max_pivots,
        })
    }

    fn columns(&self) -> usize {
        self.n_struct + self.a.len()
    }

    fn reduced_cost(&self, cost: &[Rat], j: usize) -> Result<Rat, LpError> {
        let mut r = cost[j];
        for (i, &bi) in self.basis.iter().enumerate() {
            if cost[bi] != ZERO && self.a[i][j] != ZERO {
                r = r.checked_sub(cost[bi].checked_mul(self.a[i][j])?)?;
            }
        }
        Ok(r)
    }

    fn pivot(&mut self, row: usize, col: usize) -> Result<(), LpError> {
        if self.pivots >= self.max_pivots {
            return Err(LpError::PivotCap);
        }
        self.pivots += 1;
        let p = self.a[row][col];
        for v in &mut self.a[row] {
            *v = v.checked_div(p)?;
        }
        self.b[row] = self.b[row].checked_div(p)?;
        let pivot_row = self.a[row].clone();
        let pivot_b = self.b[row];
        for k in 0..self.a.len() {
            let f = self.a[k][col];
            if k == row || f == ZERO {
                continue;
            }
            for (v, pv) in self.a[k].iter_mut().zip(&pivot_row) {
                if *pv != ZERO {
                    *v = v.checked_sub(f.checked_mul(*pv)?)?;
                }
            }
            self.b[k] = self.b[k].checked_sub(f.checked_mul(pivot_b)?)?;
        }
        self.basis[row] = col;
        Ok(())
    }

    /// Bland's rule: the lowest-index improving column enters; ties in the ratio test go to the lowest-index
    /// basic variable.
    fn optimize(&mut self, cost: &[Rat], structural_only: bool) -> Result<(), LpError> {
        let limit = if structural_only {
            self.n_struct
        } else {
            self.columns()
        };
        loop {
            let mut entering = None;
            for j in 0..limit {
                if self.basis.contains(&j) {
                    continue;
                }
                if self.reduced_cost(cost, j)?.signum() < 0 {
                    entering = Some(j);
                    break;
                }
            }
            let Some(j) = entering else {
                return Ok(());
            };
            let mut leave: Option<(usize, Rat)> = None;
            for i in 0..self.a.len() {
                if self.a[i][j].signum() <= 0 {
                    continue;
                }
                let ratio = self.b[i].checked_div(self.a[i][j])?;
                let better = match leave {
                    None => true,
                    Some((l, best)) => match ratio.checked_cmp(best)? {
                        Ordering::Less => true,
                        Ordering::Equal => self.basis[i] < self.basis[l],
                        Ordering::Greater => false,
                    },
                };
                if better {
                    leave = Some((i, ratio));
                }
            }
            let Some((i, _)) = leave else {
                return Err(LpError::Unbounded);
            };
            self.pivot(i, j)?;
        }
    }

    fn objective(&self, cost: &[Rat]) -> Result<Rat, LpError> {
        let mut z = ZERO;
        for (i, &bi) in self.basis.iter().enumerate() {
            z = z.checked_add(cost[bi].checked_mul(self.b[i])?)?;
        }
        Ok(z)
    }

    fn value(&self, j: usize) -> Rat {
        self.basis
            .iter()
            .position(|&bi| bi == j)
            .map_or(ZERO, |i| self.b[i])
    }

    /// Pivots every basic artificial out on a structural column where its row has one; a row with none is
    /// redundant and keeps its artificial at zero.
    fn drive_out_artificials(&mut self) -> Result<(), LpError> {
        for i in 0..self.a.len() {
            if self.basis[i] < self.n_struct {
                continue;
            }
            if let Some(j) = (0..self.n_struct).find(|&j| self.a[i][j] != ZERO) {
                self.pivot(i, j)?;
            }
        }
        Ok(())
    }

    /// The duals of the original (un-flipped) rows: `π = c_Bᵀ B⁻¹`, read from the artificial columns.
    fn duals(&self, cost: &[Rat]) -> Result<Vec<Rat>, LpError> {
        let mut out = Vec::with_capacity(self.a.len());
        for i in 0..self.a.len() {
            let col = self.n_struct + i;
            let mut pi = ZERO;
            for (k, &bk) in self.basis.iter().enumerate() {
                if cost[bk] != ZERO && self.a[k][col] != ZERO {
                    pi = pi.checked_add(cost[bk].checked_mul(self.a[k][col])?)?;
                }
            }
            out.push(if self.flipped[i] {
                pi.checked_neg()?
            } else {
                pi
            });
        }
        Ok(out)
    }

    /// Phase 1: true when the artificials reach zero.
    fn phase_one(&mut self) -> Result<(bool, Vec<Rat>), LpError> {
        let cost: Vec<Rat> = (0..self.columns())
            .map(|j| if j < self.n_struct { ZERO } else { ONE })
            .collect();
        self.optimize(&cost, false)?;
        Ok((self.objective(&cost)?.signum() <= 0, cost))
    }
}

enum Feasibility {
    /// Every variable's value.
    Feasible(Vec<Rat>),
    /// Farkas multipliers by row id.
    Infeasible(Vec<(String, Rat)>),
}

/// Strict feasibility of the system, open ends included.
fn feasibility(lin: &Linear, max_pivots: usize) -> Result<Feasibility, LpError> {
    let Standard {
        rows,
        rhs,
        n_struct,
        maps,
        t,
    } = standardize(lin, true)?;
    let mut tab = Tableau::new(rows, rhs, n_struct, max_pivots)?;
    let (reached, phase1) = tab.phase_one()?;
    if !reached {
        let p = tab.duals(&phase1)?;
        return Ok(Feasibility::Infeasible(multipliers(&lin.rows, &p)?));
    }
    tab.drive_out_artificials()?;
    if let Some(t) = t {
        let mut cost = vec![ZERO; tab.columns()];
        cost[t] = MINUS_ONE;
        tab.optimize(&cost, true)?;
        if tab.value(t).signum() <= 0 {
            let p = tab.duals(&cost)?;
            return Ok(Feasibility::Infeasible(multipliers(&lin.rows, &p)?));
        }
    }
    let values = (0..lin.bounds.len())
        .map(|j| variable_value(&maps, &tab, j))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Feasibility::Feasible(values))
}

/// The exact minimum (or maximum) of variable `var` over the closure of the system; `None` when the closure is
/// empty.
fn optimum(
    lin: &Linear,
    var: usize,
    maximize: bool,
    max_pivots: usize,
) -> Result<Option<Rat>, LpError> {
    let Standard {
        rows,
        rhs,
        n_struct,
        maps,
        ..
    } = standardize(lin, false)?;
    let mut tab = Tableau::new(rows, rhs, n_struct, max_pivots)?;
    if !tab.phase_one()?.0 {
        return Ok(None);
    }
    tab.drive_out_artificials()?;
    let mut cost = vec![ZERO; tab.columns()];
    match maps[var] {
        // Minimising x = offset ± col minimises ±col; maximising minimises ∓col.
        Column::Shift { up, col, .. } => cost[col] = if up != maximize { ONE } else { MINUS_ONE },
        Column::Split { pos, neg } => {
            let (p, q) = if maximize {
                (MINUS_ONE, ONE)
            } else {
                (ONE, MINUS_ONE)
            };
            cost[pos] = p;
            cost[neg] = q;
        }
    }
    tab.optimize(&cost, true)?;
    Ok(Some(variable_value(&maps, &tab, var)?))
}

/// Multipliers by row id, scaled by a positive factor so the first non-zero one is ±1.
fn multipliers(rows: &[LinRow], p: &[Rat]) -> Result<Vec<(String, Rat)>, LpError> {
    let scale = match p.iter().take(rows.len()).find(|x| **x != ZERO) {
        Some(x) if x.signum() < 0 => x.checked_neg()?,
        Some(x) => *x,
        None => ONE,
    };
    let mut out = Vec::new();
    for (row, x) in rows.iter().zip(p) {
        if *x != ZERO {
            out.push((row.id.clone(), x.checked_div(scale)?));
        }
    }
    Ok(out)
}

/// Why a checker rejected an outcome.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckFailure(pub String);

fn fail<T>(reason: impl Into<String>) -> Result<T, CheckFailure> {
    Err(CheckFailure(reason.into()))
}

fn checked<T>(r: Result<T, UnitsError>) -> Result<T, CheckFailure> {
    r.map_err(|_| CheckFailure(OVERFLOW.to_string()))
}

/// Checks a witness: every cell has a value inside its interval (a component-sum total, which must foot in
/// printed values, has none), and every identity holds exactly, ratios included.
pub fn check_witness(
    cells: &[Cell],
    identities: &[Identity],
    rules: &Rules,
    witness: &BTreeMap<String, Rat>,
) -> Result<(), CheckFailure> {
    let cells: Vec<&Cell> = cells.iter().collect();
    let identities: Vec<&Identity> = identities.iter().collect();
    check_witness_in(&cells, &identities, rules, &Frame::default(), witness)
}

fn check_witness_in(
    cells: &[&Cell],
    identities: &[&Identity],
    rules: &Rules,
    frame: &Frame,
    witness: &BTreeMap<String, Rat>,
) -> Result<(), CheckFailure> {
    let mut bounds: BTreeMap<&str, Bound> = BTreeMap::new();
    let mut printed: BTreeMap<&str, Rat> = BTreeMap::new();
    for c in cells {
        let b = match cell_bound(c, rules.convention) {
            Ok(Some(b)) => b,
            Ok(None) => return fail(OVERFLOW),
            Err(e) => return fail(e.to_string()),
        };
        if bounds.insert(c.locator.as_str(), b).is_some() {
            return fail(format!("{} is given twice", c.locator));
        }
        printed.insert(c.locator.as_str(), c.printed);
    }
    for id in identities {
        if let IdentityKind::Sum { total, .. } = &id.kind
            && rules
                .construction(id)
                .map_err(|e| CheckFailure(e.to_string()))?
                == Construction::ComponentSum
        {
            let row = id.row().map_err(|e| CheckFailure(e.to_string()))?;
            if !frame.touches(id) && checked(printed_value(&row, &printed))? != ZERO {
                return fail(format!(
                    "identity {} does not foot in printed values",
                    id.id
                ));
            }
            bounds.insert(total.as_str(), Bound::FREE);
        }
    }
    for loc in &frame.unprinted {
        let Some(b) = bounds.get_mut(loc.as_str()) else {
            return fail(format!("{loc} is not a cell"));
        };
        let printed_box = cells
            .iter()
            .find(|c| c.locator == *loc)
            .map(|c| cell_bound(c, rules.convention));
        let Some(Ok(Some(printed_box))) = printed_box else {
            return fail(OVERFLOW);
        };
        *b = freed_bound(printed_box, is_den(identities, loc));
    }
    for (loc, b) in &frame.bounds {
        bounds.insert(loc.as_str(), *b);
    }
    let value = |loc: &str| -> Result<Rat, CheckFailure> {
        witness
            .get(loc)
            .copied()
            .ok_or_else(|| CheckFailure(format!("{loc} has no witness value")))
    };
    for (loc, b) in &bounds {
        if !checked(b.contains(value(loc)?))? {
            return fail(format!("{loc}: the witness value is outside its interval"));
        }
    }
    for id in identities {
        if let IdentityKind::Ratio {
            rate,
            num,
            den,
            factor,
        } = &id.kind
        {
            let (r, n, d) = (value(rate)?, value(num)?, value(den)?);
            if d == ZERO || checked(r.checked_mul(d))? != checked(factor.checked_mul(n))? {
                return fail(format!("ratio {} does not hold at the witness", id.id));
            }
            continue;
        }
        let row = id.row().map_err(|e| CheckFailure(e.to_string()))?;
        let mut sum = ZERO;
        for (loc, a) in &row.coefficients {
            sum = checked(sum.checked_add(checked(a.checked_mul(value(loc)?))?))?;
        }
        if sum != row.rhs {
            return fail(format!("identity {} does not hold at the witness", id.id));
        }
    }
    Ok(())
}

/// What a valid certificate certifies: `cᵀx` ranges over `range` on the boxes (for a residual, the printed
/// values), and `rhs = b*` lies outside it. `combination` is keyed by cell locator, or by row id for a ratio's
/// slack.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CertificateRange {
    pub combination: BTreeMap<String, Rat>,
    pub rhs: Rat,
    pub range: Bound,
}

/// Checks a certificate. A residual: the identity is a `ComponentSum` sum, and its printed total minus
/// Σ ± printed lines is the stated non-zero residual. Farkas multipliers: `c = Σ λᵢ·aᵢ`, `b* = Σ λᵢ·bᵢ` over the
/// linearised rows, and the exact range of `cᵀx` over the ranges of the variables, open and infinite ends
/// included; accepted only when `b*` lies outside it.
pub fn check_certificate(
    cells: &[Cell],
    identities: &[Identity],
    rules: &Rules,
    certificate: &Certificate,
) -> Result<CertificateRange, CheckFailure> {
    let mut cells: Vec<&Cell> = cells.iter().collect();
    cells.sort_by(|a, b| a.locator.cmp(&b.locator));
    let mut identities: Vec<&Identity> = identities.iter().collect();
    identities.sort_by(|a, b| a.id.cmp(&b.id));
    check_certificate_in(&cells, &identities, rules, &Frame::default(), certificate)
}

fn check_certificate_in(
    cells: &[&Cell],
    identities: &[&Identity],
    rules: &Rules,
    frame: &Frame,
    certificate: &Certificate,
) -> Result<CertificateRange, CheckFailure> {
    let multipliers = match certificate {
        Certificate::Residual { identity, residual } => {
            let Some(id) = identities.iter().find(|i| i.id == *identity) else {
                return fail(format!("residual for unknown identity {identity}"));
            };
            if !matches!(id.kind, IdentityKind::Sum { .. })
                || rules.construction(id) != Ok(Construction::ComponentSum)
            {
                return fail(format!("{identity} is not a component-sum identity"));
            }
            if frame.touches(id) {
                return fail(format!(
                    "{identity} has a cell whose printed value is disregarded"
                ));
            }
            let printed: BTreeMap<&str, Rat> = cells
                .iter()
                .map(|c| (c.locator.as_str(), c.printed))
                .collect();
            let row = id.row().map_err(|e| CheckFailure(e.to_string()))?;
            let gap = checked(printed_value(&row, &printed))?;
            if gap == ZERO || checked(gap.checked_neg())? != *residual {
                return fail(format!(
                    "{identity}: the stated residual does not recompute"
                ));
            }
            let point = Some(End {
                at: gap,
                strict: false,
            });
            return Ok(CertificateRange {
                combination: row.coefficients,
                rhs: row.rhs,
                range: Bound {
                    lo: point,
                    hi: point,
                },
            });
        }
        Certificate::Farkas { multipliers } => multipliers,
    };
    let lin = match linearise(cells, identities, rules, frame) {
        Ok(Lin::System(lin)) => lin,
        Ok(Lin::Residual { identity, .. }) => {
            return fail(format!(
                "{identity} does not foot: its residual is the certificate"
            ));
        }
        Ok(Lin::Indeterminate(reason)) => return fail(reason),
        Err(e) => return fail(e.to_string()),
    };
    let rows: BTreeMap<&str, &LinRow> = lin.rows.iter().map(|r| (r.id.as_str(), r)).collect();
    let mut combination: BTreeMap<usize, Rat> = BTreeMap::new();
    let mut rhs = ZERO;
    for (id, lambda) in multipliers {
        let Some(row) = rows.get(id.as_str()) else {
            return fail(format!("multiplier for unknown row {id}"));
        };
        for &(j, a) in &row.terms {
            let e = combination.entry(j).or_insert(ZERO);
            *e = checked(e.checked_add(checked(lambda.checked_mul(a))?))?;
        }
        rhs = checked(rhs.checked_add(checked(lambda.checked_mul(row.rhs))?))?;
    }
    combination.retain(|_, c| *c != ZERO);
    let origin = Some(End {
        at: ZERO,
        strict: false,
    });
    let mut range = Bound {
        lo: origin,
        hi: origin,
    };
    let add = |acc: Option<End>, c: Rat, end: Option<End>| -> Result<Option<End>, CheckFailure> {
        Ok(match (acc, end) {
            (Some(acc), Some(end)) => Some(End {
                at: checked(acc.at.checked_add(checked(c.checked_mul(end.at))?))?,
                strict: acc.strict || end.strict,
            }),
            _ => None,
        })
    };
    for (&j, &c) in &combination {
        let b = lin.bounds[j];
        let (low, high) = if c.signum() > 0 {
            (b.lo, b.hi)
        } else {
            (b.hi, b.lo)
        };
        range.lo = add(range.lo, c, low)?;
        range.hi = add(range.hi, c, high)?;
    }
    let below = match range.lo {
        Some(lo) => match checked(rhs.checked_cmp(lo.at))? {
            Ordering::Less => true,
            Ordering::Equal => lo.strict,
            Ordering::Greater => false,
        },
        None => false,
    };
    let above = match range.hi {
        Some(hi) => match checked(rhs.checked_cmp(hi.at))? {
            Ordering::Greater => true,
            Ordering::Equal => hi.strict,
            Ordering::Less => false,
        },
        None => false,
    };
    if !(below || above) {
        return fail("b* lies inside the range of the combination");
    }
    Ok(CertificateRange {
        combination: combination
            .into_iter()
            .map(|(j, c)| (lin.names[j].clone(), c))
            .collect(),
        rhs,
        range,
    })
}

/// Why [`implied_interval`] gives no interval.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImpliedError {
    Refused(XfRefusal),
    /// The component has no point (with the cell's own box, or without it when dropped).
    Infeasible,
    /// The cell is unbounded in at least one direction.
    Unbounded,
    /// No claim: a cap, an overflow, a bilinear cell, a rate over a denominator without a finite range of one
    /// sign, or a checker rejection.
    Indeterminate(String),
}

impl From<XfRefusal> for ImpliedError {
    fn from(r: XfRefusal) -> Self {
        Self::Refused(r)
    }
}

fn lp_implied(e: LpError) -> ImpliedError {
    match e {
        LpError::Overflow => ImpliedError::Indeterminate(OVERFLOW.to_string()),
        LpError::PivotCap => {
            ImpliedError::Indeterminate(format!("over the cap of {MAX_PIVOTS} pivots"))
        }
        LpError::Unbounded => ImpliedError::Unbounded,
    }
}

/// The exact interval `var` can take in its component (S2). With `free_own_box`, the cell's own printed box is
/// dropped (a ratio's denominator keeps its printed sign): the "implied" value of a cell from the others.
/// Without it, the "un-rounding" interval of a cell in a feasible component. A ratio's rate gets the ratio's
/// exact range (Charnes–Cooper, [`rate_extreme`]) cut by its own box. Never a published value.
pub fn implied_interval(
    cells: &[Cell],
    identities: &[Identity],
    rules: &Rules,
    var: &str,
    free_own_box: bool,
) -> Result<Interval, ImpliedError> {
    let p = prepare(cells, identities, rules)?;
    let Some(pos) = p.cells.iter().position(|c| c.locator == var) else {
        return Err(XfRefusal::UnknownVariable {
            locator: var.to_string(),
        }
        .into());
    };
    let Some((cell_ix, id_ix)) = p.groups.iter().find(|(c, _)| c.contains(&pos)) else {
        return Err(ImpliedError::Indeterminate(format!(
            "{var} is in no component"
        )));
    };
    if cell_ix.len() > MAX_VARIABLES {
        return Err(ImpliedError::Indeterminate(format!(
            "the component has {} cells, over the cap of {MAX_VARIABLES}",
            cell_ix.len()
        )));
    }
    let cells: Vec<&Cell> = cell_ix.iter().map(|&i| p.cells[i]).collect();
    let identities: Vec<&Identity> = id_ix.iter().map(|&i| p.identities[i]).collect();
    let mut frame = Frame::default();
    if free_own_box {
        frame.unprinted.insert(var.to_string());
    }
    implied_in(&cells, &identities, rules, var, &frame)
}

/// [`implied_interval`] of `var` in one component under `frame`; `var`'s own box is dropped when it is unprinted.
fn implied_in(
    cells: &[&Cell],
    identities: &[&Identity],
    rules: &Rules,
    var: &str,
    frame: &Frame,
) -> Result<Interval, ImpliedError> {
    let overflow = || ImpliedError::Indeterminate(OVERFLOW.to_string());
    let Some(cell) = cells.iter().find(|c| c.locator == var) else {
        return Err(XfRefusal::UnknownVariable {
            locator: var.to_string(),
        }
        .into());
    };
    let printed = cell_bound(cell, rules.convention)?.ok_or_else(overflow)?;
    let is_rate = identities
        .iter()
        .any(|id| matches!(&id.kind, IdentityKind::Ratio { rate, .. } if rate == var));
    // A rate's range is the ratio's, so its rows are dropped here; its own box (unless disregarded) cuts below.
    let run = if is_rate {
        frame.pinned(var, Bound::FREE)
    } else {
        frame.clone()
    };
    let lin = match linearise(cells, identities, rules, &run)? {
        Lin::System(lin) => lin,
        Lin::Residual { .. } => return Err(ImpliedError::Infeasible),
        Lin::Indeterminate(reason) => return Err(ImpliedError::Indeterminate(reason)),
    };
    let j = lin.names.iter().position(|n| n == var).ok_or_else(|| {
        ImpliedError::Indeterminate(format!("{var} is not a variable of its component"))
    })?;
    let (base, lo, hi) = if is_rate {
        let base = if frame.unprinted.contains(var) {
            Bound::FREE
        } else {
            printed
        };
        let lo = rate_extreme(&lin, j, false)?.ok_or(ImpliedError::Infeasible)?;
        let hi = rate_extreme(&lin, j, true)?.ok_or(ImpliedError::Infeasible)?;
        // The ratio's range cut by the rate's own box: the image of the convex region is an interval.
        let cmp = |a: Rat, b: Rat| a.checked_cmp(b).map_err(|_| overflow());
        let lo = match base.lo {
            Some(e) if cmp(lo, e.at)? == Ordering::Less => e.at,
            _ => lo,
        };
        let hi = match base.hi {
            Some(e) if cmp(hi, e.at)? == Ordering::Greater => e.at,
            _ => hi,
        };
        if cmp(lo, hi)? == Ordering::Greater {
            return Err(ImpliedError::Infeasible);
        }
        (base, lo, hi)
    } else {
        let lo = optimum(&lin, j, false, MAX_PIVOTS)
            .map_err(lp_implied)?
            .ok_or(ImpliedError::Infeasible)?;
        let hi = optimum(&lin, j, true, MAX_PIVOTS)
            .map_err(lp_implied)?
            .ok_or(ImpliedError::Infeasible)?;
        (lin.bounds[j], lo, hi)
    };
    let lo_strict = settle(cells, identities, rules, frame, var, base, (lo, true))?;
    let hi_strict = settle(cells, identities, rules, frame, var, base, (hi, false))?;
    // A single point with an open end is no point: the closure touched, the region did not.
    if lo == hi && (lo_strict || hi_strict) {
        return Err(ImpliedError::Infeasible);
    }
    Ok(Interval {
        lo,
        hi,
        lo_strict,
        hi_strict,
    })
}

/// The exact minimum (or maximum), over the closure, of `factor·num/den` for the ratio whose rate is variable
/// `r` (its rows dropped). Charnes–Cooper: with `u = 1/|den|` and `z = u·x`, every row `Σ a·x = b` becomes
/// `Σ a·z − b·u = 0`, every end `lo ≤ x` (`x ≤ hi`) becomes `z − lo·u ≥ 0` (`hi·u − z ≥ 0`), `z_den = ±1`, and
/// the objective is the linear `±factor·z_num`. The denominator's range must be finite and of one sign, which
/// rules out `u = 0` in the closure.
fn rate_extreme(lin: &Linear, r: usize, maximize: bool) -> Result<Option<Rat>, ImpliedError> {
    let indeterminate = |reason: &str| ImpliedError::Indeterminate(reason.to_string());
    let overflow = |_| indeterminate(OVERFLOW);
    let Some(&(_, n, d, factor)) = lin.ratios.iter().find(|(rate, ..)| *rate == r) else {
        return Err(indeterminate("no ratio has this rate"));
    };
    let den = lin.bounds[d];
    let sign = match (den.lo, den.hi, one_sign(den).map_err(overflow)?) {
        (Some(_), Some(_), 1) => ONE,
        (Some(_), Some(_), -1) => MINUS_ONE,
        _ => {
            return Err(indeterminate(
                "the denominator has no finite range of one sign",
            ));
        }
    };
    let nv = lin.bounds.len();
    let (u, w) = (nv, nv + 1);
    let mut names: Vec<String> = lin.names.iter().map(|x| format!("z:{x}")).collect();
    names.extend(["u".to_string(), "w".to_string()]);
    let mut bounds = vec![Bound::FREE; nv];
    bounds.push(Bound {
        lo: Some(End {
            at: ZERO,
            strict: true,
        }),
        hi: None,
    });
    bounds.push(Bound::FREE);
    let mut rows = Vec::new();
    for row in &lin.rows {
        let mut terms = row.terms.clone();
        if row.rhs != ZERO {
            terms.push((u, row.rhs.checked_neg().map_err(overflow)?));
        }
        rows.push(LinRow {
            id: row.id.clone(),
            terms,
            rhs: ZERO,
        });
    }
    for (j, b) in lin.bounds.iter().enumerate() {
        for (end, lower) in [(b.lo, true), (b.hi, false)] {
            let Some(end) = end else {
                continue;
            };
            // z − lo·u − σ = 0 or hi·u − z − σ = 0, with σ ≥ 0 keeping the end's openness.
            let slack = names.len();
            names.push(format!(
                "{}#{}",
                lin.names[j],
                if lower { "lo" } else { "hi" }
            ));
            bounds.push(Bound {
                lo: Some(End {
                    at: ZERO,
                    strict: end.strict,
                }),
                hi: None,
            });
            let (a_z, a_u) = if lower {
                (ONE, end.at.checked_neg().map_err(overflow)?)
            } else {
                (MINUS_ONE, end.at)
            };
            let mut terms = vec![(j, a_z), (u, a_u), (slack, MINUS_ONE)];
            terms.retain(|(_, a)| *a != ZERO);
            rows.push(LinRow {
                id: names[slack].clone(),
                terms,
                rhs: ZERO,
            });
        }
    }
    rows.push(LinRow {
        id: "den".to_string(),
        terms: vec![(d, ONE)],
        rhs: sign,
    });
    // w = sign·factor·z_num = factor·num/den.
    let a_num = sign
        .checked_mul(factor)
        .and_then(Rat::checked_neg)
        .map_err(overflow)?;
    rows.push(LinRow {
        id: "objective".to_string(),
        terms: vec![(w, ONE), (n, a_num)],
        rhs: ZERO,
    });
    let transformed = Linear {
        names,
        bounds,
        rows,
        ratios: Vec::new(),
        cells: 0,
    };
    optimum(&transformed, w, maximize, MAX_PIVOTS).map_err(lp_implied)
}

/// Re-decides one end `(m, lower)` of an implied interval by checked runs, and returns whether the end is open.
fn settle(
    cells: &[&Cell],
    identities: &[&Identity],
    rules: &Rules,
    frame: &Frame,
    var: &str,
    base: Bound,
    (m, lower): (Rat, bool),
) -> Result<bool, ImpliedError> {
    let reached = |bound| -> Result<bool, ImpliedError> {
        let run = checked_run(cells, identities, rules, &frame.pinned(var, bound))?;
        Ok(matches!(run, Checked::Feasible))
    };
    let overflow = |_| ImpliedError::Indeterminate(OVERFLOW.to_string());
    let at_m = |strict| Some(End { at: m, strict });
    let past = if lower {
        Bound {
            lo: None,
            hi: at_m(true),
        }
    } else {
        Bound {
            lo: at_m(true),
            hi: None,
        }
    };
    let beyond = base.meet(past).map_err(overflow)?;
    if !beyond.is_empty().map_err(overflow)? && reached(beyond)? {
        return Err(ImpliedError::Indeterminate(
            "checker rejected: a point lies beyond the optimum".to_string(),
        ));
    }
    let point = base
        .meet(Bound {
            lo: at_m(false),
            hi: at_m(false),
        })
        .map_err(overflow)?;
    if point.is_empty().map_err(overflow)? {
        return Ok(true);
    }
    Ok(!reached(point)?)
}

/// A feasibility run that passed its checker.
enum Checked {
    Feasible,
    Infeasible(Certificate),
}

/// A feasibility run under `frame`, reported only through its checker.
fn checked_run(
    cells: &[&Cell],
    identities: &[&Identity],
    rules: &Rules,
    frame: &Frame,
) -> Result<Checked, ImpliedError> {
    let rejected = |_| ImpliedError::Indeterminate("checker rejected".to_string());
    let lin = match linearise(cells, identities, rules, frame)? {
        Lin::System(lin) => lin,
        Lin::Residual { identity, residual } => {
            let certificate = Certificate::Residual { identity, residual };
            check_certificate_in(cells, identities, rules, frame, &certificate)
                .map_err(rejected)?;
            return Ok(Checked::Infeasible(certificate));
        }
        Lin::Indeterminate(reason) => return Err(ImpliedError::Indeterminate(reason)),
    };
    match feasibility(&lin, MAX_PIVOTS).map_err(lp_implied)? {
        Feasibility::Feasible(values) => {
            let witness = witness_of(&lin, &values)
                .map_err(|_| ImpliedError::Indeterminate(OVERFLOW.to_string()))?;
            check_witness_in(cells, identities, rules, frame, &witness).map_err(rejected)?;
            Ok(Checked::Feasible)
        }
        Feasibility::Infeasible(multipliers) => {
            let certificate = Certificate::Farkas { multipliers };
            check_certificate_in(cells, identities, rules, frame, &certificate)
                .map_err(rejected)?;
            Ok(Checked::Infeasible(certificate))
        }
    }
}

/// Above this many cells, a component's correction search is greedy and labelled `InclusionMinimalOnly` (S3).
pub const MAX_EXHAUSTIVE_CELLS: usize = 20;
/// Checked runs one component's blame may spend before the exhaustive search gives way to the greedy one.
pub const MAX_BLAME_RUNS: usize = 10_000;

/// How far the correction search went (S3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CorrectionLabel {
    /// Every set of the smallest size that makes the component feasible: a search over all of its cells, run to
    /// completion.
    Minimum,
    /// One set that makes the component feasible and from which no cell can be dropped. A smaller one may exist.
    InclusionMinimalOnly,
}

/// Cells whose printed values, disregarded together, make the component feasible, each with the range it can then
/// take (with the whole set disregarded). A range of consistent values, never a corrected value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CorrectionSet {
    pub cells: Vec<String>,
    pub implied: Vec<(String, Result<Interval, ImpliedError>)>,
}

/// An inclusion-minimal infeasible set of printed cells. With every other cell's printed value disregarded the
/// component is still infeasible, as `certificate` shows ([`check_conflict_set`]); disregarding any member as well
/// makes it feasible.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConflictSet {
    pub cells: Vec<String>,
    pub certificate: Certificate,
}

/// Which printed values an infeasible component's conflict rests on (S3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Blame {
    pub conflicts: Vec<ConflictSet>,
    pub corrections: Vec<CorrectionSet>,
    pub label: CorrectionLabel,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BlameOutcome {
    Blamed(Blame),
    /// No claim: a cap, an overflow, or a checker rejection.
    Indeterminate {
        reason: String,
    },
}

/// The blame of one component that [`solve`] finds infeasible.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComponentBlame {
    pub cells: Vec<String>,
    pub identities: Vec<String>,
    pub outcome: BlameOutcome,
}

/// Blames every component that [`solve`] finds infeasible; feasible and indeterminate components get none.
///
/// Identities are hard; printed values are soft, and "disregarding" a cell drops its box and any printed footing
/// over it. Conflict sets come from a deletion filter (disregard each cell in turn while the rest stays infeasible),
/// run once per identity with that identity's cells last. Correction sets: with at most [`MAX_EXHAUSTIVE_CELLS`]
/// cells, every subset by size, skipping those that miss a known conflict set, until a size restores feasibility;
/// all sets of that size are reported, labelled `Minimum`. Otherwise, or past [`MAX_BLAME_RUNS`], one greedy
/// hitting set made inclusion-minimal, labelled `InclusionMinimalOnly`. Every feasibility decision is a checked run.
pub fn blame(
    cells: &[Cell],
    identities: &[Identity],
    rules: &Rules,
) -> Result<Vec<ComponentBlame>, XfRefusal> {
    blame_with_caps(
        cells,
        identities,
        rules,
        MAX_EXHAUSTIVE_CELLS,
        MAX_BLAME_RUNS,
    )
}

/// [`blame`] with explicit caps.
pub fn blame_with_caps(
    cells: &[Cell],
    identities: &[Identity],
    rules: &Rules,
    exhaustive_cells: usize,
    max_runs: usize,
) -> Result<Vec<ComponentBlame>, XfRefusal> {
    let p = prepare(cells, identities, rules)?;
    let mut out = Vec::new();
    for (cell_ix, id_ix) in &p.groups {
        let comp_cells: Vec<&Cell> = cell_ix.iter().map(|&i| p.cells[i]).collect();
        let comp_ids: Vec<&Identity> = id_ix.iter().map(|&i| p.identities[i]).collect();
        if comp_cells.len() > MAX_VARIABLES
            || !matches!(
                decide(&comp_cells, &comp_ids, rules, MAX_PIVOTS)?,
                Outcome::Infeasible { .. }
            )
        {
            continue;
        }
        let mut search = Search {
            cells: &comp_cells,
            identities: &comp_ids,
            rules,
            runs: 0,
            max_runs,
        };
        let outcome = match search.blame(exhaustive_cells) {
            Ok(b) => BlameOutcome::Blamed(b),
            Err(reason) => BlameOutcome::Indeterminate { reason },
        };
        out.push(ComponentBlame {
            cells: comp_cells.iter().map(|c| c.locator.clone()).collect(),
            identities: comp_ids.iter().map(|i| i.id.clone()).collect(),
            outcome,
        });
    }
    Ok(out)
}

/// One component's blame search: checked runs with some printed values disregarded.
struct Search<'a> {
    cells: &'a [&'a Cell],
    identities: &'a [&'a Identity],
    rules: &'a Rules,
    runs: usize,
    max_runs: usize,
}

impl<'a> Search<'a> {
    fn run(&mut self, unprinted: &BTreeSet<&str>) -> Result<Checked, String> {
        self.runs += 1;
        let frame = Frame {
            unprinted: unprinted.iter().map(|s| (*s).to_string()).collect(),
            bounds: BTreeMap::new(),
        };
        checked_run(self.cells, self.identities, self.rules, &frame).map_err(|e| match e {
            ImpliedError::Indeterminate(reason) => reason,
            other => format!("{other:?}"),
        })
    }

    fn feasible(&mut self, unprinted: &BTreeSet<&str>) -> Result<bool, String> {
        Ok(matches!(self.run(unprinted)?, Checked::Feasible))
    }

    /// The deletion filter from `freed` (infeasible): disregards each cell of `order` in turn, keeping it
    /// disregarded while the rest stays infeasible. The cells of `order` left are an inclusion-minimal infeasible
    /// set, because disregarding more cells only relaxes the system.
    fn filter(
        &mut self,
        mut freed: BTreeSet<&'a str>,
        order: &[&'a str],
    ) -> Result<Vec<&'a str>, String> {
        for &c in order {
            if freed.insert(c) && self.feasible(&freed)? {
                freed.remove(c);
            }
        }
        let mut kept: Vec<&str> = order
            .iter()
            .copied()
            .filter(|c| !freed.contains(c))
            .collect();
        kept.sort_unstable();
        Ok(kept)
    }

    /// Records `set` (sorted) with its certificate, unless already known.
    fn record(&mut self, set: Vec<&str>, conflicts: &mut Vec<ConflictSet>) -> Result<(), String> {
        if conflicts
            .iter()
            .any(|c| c.cells.iter().map(String::as_str).eq(set.iter().copied()))
        {
            return Ok(());
        }
        let others: BTreeSet<&str> = self
            .cells
            .iter()
            .map(|c| c.locator.as_str())
            .filter(|c| !set.contains(c))
            .collect();
        let Checked::Infeasible(certificate) = self.run(&others)? else {
            return Err("checker rejected: a conflict set is feasible".to_string());
        };
        conflicts.push(ConflictSet {
            cells: set.iter().map(|s| (*s).to_string()).collect(),
            certificate,
        });
        Ok(())
    }

    fn blame(&mut self, exhaustive_cells: usize) -> Result<Blame, String> {
        let all: Vec<&'a str> = self.cells.iter().map(|c| c.locator.as_str()).collect();
        let mut conflicts = Vec::new();
        for id in self.identities {
            let own = id.locators();
            let order: Vec<&str> = all
                .iter()
                .copied()
                .filter(|c| !own.contains(c))
                .chain(all.iter().copied().filter(|c| own.contains(c)))
                .collect();
            let set = self.filter(BTreeSet::new(), &order)?;
            self.record(set, &mut conflicts)?;
        }
        let exhaustive = if all.len() <= exhaustive_cells {
            self.exhaustive(&all, &conflicts)?
        } else {
            None
        };
        let (sets, label) = match exhaustive {
            Some(sets) => (sets, CorrectionLabel::Minimum),
            None => (
                vec![self.greedy(&all, &mut conflicts)?],
                CorrectionLabel::InclusionMinimalOnly,
            ),
        };
        let corrections = sets
            .into_iter()
            .map(|set| {
                let frame = Frame {
                    unprinted: set.iter().map(|s| (*s).to_string()).collect(),
                    bounds: BTreeMap::new(),
                };
                let implied = set
                    .iter()
                    .map(|c| {
                        let range = implied_in(self.cells, self.identities, self.rules, c, &frame);
                        ((*c).to_string(), range)
                    })
                    .collect();
                CorrectionSet {
                    cells: set.iter().map(|s| (*s).to_string()).collect(),
                    implied,
                }
            })
            .collect();
        Ok(Blame {
            conflicts,
            corrections,
            label,
        })
    }

    /// Every smallest set that restores feasibility, or `None` past the run cap.
    fn exhaustive(
        &mut self,
        all: &[&'a str],
        conflicts: &[ConflictSet],
    ) -> Result<Option<Vec<Vec<&'a str>>>, String> {
        let n = all.len();
        for size in 1..=n {
            let mut found = Vec::new();
            let mut idx: Vec<usize> = (0..size).collect();
            loop {
                let set: BTreeSet<&str> = idx.iter().map(|&i| all[i]).collect();
                // A set that misses a conflict set leaves it infeasible.
                let hits = conflicts
                    .iter()
                    .all(|c| c.cells.iter().any(|x| set.contains(x.as_str())));
                if hits {
                    if self.runs >= self.max_runs {
                        return Ok(None);
                    }
                    if self.feasible(&set)? {
                        found.push(set.into_iter().collect());
                    }
                }
                if !next_combination(&mut idx, n) {
                    break;
                }
            }
            if !found.is_empty() {
                return Ok(Some(found));
            }
        }
        Err("disregarding every printed value leaves the component infeasible".to_string())
    }

    /// One inclusion-minimal set that restores feasibility: hit the known conflict sets greedily, find another
    /// conflict set when that is not enough, and finally drop every cell the set does not need.
    fn greedy(
        &mut self,
        all: &[&'a str],
        conflicts: &mut Vec<ConflictSet>,
    ) -> Result<Vec<&'a str>, String> {
        let mut set: BTreeSet<&'a str> = BTreeSet::new();
        loop {
            loop {
                let unhit: Vec<&ConflictSet> = conflicts
                    .iter()
                    .filter(|c| !c.cells.iter().any(|x| set.contains(x.as_str())))
                    .collect();
                if unhit.is_empty() {
                    break;
                }
                let in_unhit = |c: &str| {
                    unhit
                        .iter()
                        .filter(|u| u.cells.iter().any(|x| x == c))
                        .count()
                };
                // The cell in most unhit sets; ties go to the first locator.
                let Some(best) = all
                    .iter()
                    .copied()
                    .filter(|c| !set.contains(c))
                    .max_by_key(|c| (in_unhit(c), std::cmp::Reverse(*c)))
                else {
                    return Err("no cell hits a conflict set".to_string());
                };
                set.insert(best);
            }
            if self.feasible(&set)? {
                break;
            }
            let order: Vec<&str> = all.iter().copied().filter(|c| !set.contains(c)).collect();
            let found = self.filter(set.clone(), &order)?;
            self.record(found, conflicts)?;
        }
        for c in set.clone() {
            set.remove(c);
            if !self.feasible(&set)? {
                set.insert(c);
            }
        }
        Ok(set.into_iter().collect())
    }
}

/// Advances `idx` (strictly increasing indices below `n`) to the next combination in lexicographic order.
fn next_combination(idx: &mut [usize], n: usize) -> bool {
    let k = idx.len();
    for i in (0..k).rev() {
        if idx[i] < n - k + i {
            idx[i] += 1;
            let base = idx[i];
            for (off, slot) in idx[i + 1..].iter_mut().enumerate() {
                *slot = base + off + 1;
            }
            return true;
        }
    }
    false
}

/// Re-checks a conflict set: its certificate, with every other cell of its component disregarded.
pub fn check_conflict_set(
    cells: &[Cell],
    identities: &[Identity],
    rules: &Rules,
    set: &ConflictSet,
) -> Result<CertificateRange, CheckFailure> {
    let p = prepare(cells, identities, rules).map_err(|e| CheckFailure(e.to_string()))?;
    let Some(first) = set.cells.first() else {
        return fail("an empty conflict set");
    };
    let Some(pos) = p.cells.iter().position(|c| c.locator == *first) else {
        return fail(format!("{first} is not a given cell"));
    };
    let Some((cell_ix, id_ix)) = p.groups.iter().find(|(c, _)| c.contains(&pos)) else {
        return fail(format!("{first} is in no component"));
    };
    let comp_cells: Vec<&Cell> = cell_ix.iter().map(|&i| p.cells[i]).collect();
    let comp_ids: Vec<&Identity> = id_ix.iter().map(|&i| p.identities[i]).collect();
    for loc in &set.cells {
        if !comp_cells.iter().any(|c| c.locator == *loc) {
            return fail(format!("{loc} is not in the component of {first}"));
        }
    }
    let frame = Frame {
        unprinted: comp_cells
            .iter()
            .filter(|c| !set.cells.contains(&c.locator))
            .map(|c| c.locator.clone())
            .collect(),
        bounds: BTreeMap::new(),
    };
    check_certificate_in(&comp_cells, &comp_ids, rules, &frame, &set.certificate)
}

/// A cell that prints a placeholder (such as `—`) where a number could stand (S3, E116).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Placeholder {
    pub locator: String,
    pub sha256: String,
    /// The placeholder exactly as stored.
    pub token: String,
}

/// How far a signature goes: never further than a hypothesis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignatureLabel {
    Hypothesis,
}

/// Another cell that prints exactly the located cell's value: a pattern worth checking, never the cause.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Signature {
    pub label: SignatureLabel,
    pub equal_to: String,
}

/// A conflict outside the linear system: an `Equal` identity between a printed number and a placeholder. It is
/// located on the number, the one side with a printed value to disregard; the placeholder has no box.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PresenceConflict {
    pub identity: String,
    pub located: String,
    pub placeholder: String,
    pub signatures: Vec<Signature>,
}

/// Every presence conflict among `identities` (sorted by identity id). An `Equal` identity between two placeholders
/// holds; a placeholder in any other identity is refused. Each conflict's signatures are the other given cells that
/// print exactly the located value, labelled [`SignatureLabel::Hypothesis`].
pub fn presence_conflicts(
    cells: &[Cell],
    placeholders: &[Placeholder],
    identities: &[Identity],
) -> Result<Vec<PresenceConflict>, XfRefusal> {
    let mut known: BTreeSet<&str> = BTreeSet::new();
    for loc in cells
        .iter()
        .map(|c| c.locator.as_str())
        .chain(placeholders.iter().map(|p| p.locator.as_str()))
    {
        if !known.insert(loc) {
            return Err(XfRefusal::DuplicateCell {
                locator: loc.to_string(),
            });
        }
    }
    let is_placeholder = |loc: &str| placeholders.iter().any(|p| p.locator == loc);
    let mut identities: Vec<&Identity> = identities.iter().collect();
    identities.sort_by(|a, b| a.id.cmp(&b.id));
    let mut out = Vec::new();
    for id in identities {
        for loc in id.locators() {
            if !known.contains(loc) {
                return Err(XfRefusal::UnknownCell {
                    identity: id.id.clone(),
                    locator: loc.to_string(),
                });
            }
        }
        let Some(placeholder) = id.locators().into_iter().find(|l| is_placeholder(l)) else {
            continue;
        };
        let IdentityKind::Equal { a, b } = &id.kind else {
            return Err(XfRefusal::PlaceholderInIdentity {
                identity: id.id.clone(),
                locator: placeholder.to_string(),
            });
        };
        let (number, placeholder) = match (is_placeholder(a), is_placeholder(b)) {
            (true, true) => continue,
            (true, false) => (b, a),
            _ => (a, b),
        };
        let Some(located) = cells.iter().find(|c| c.locator == *number) else {
            continue;
        };
        let mut signatures: Vec<Signature> = cells
            .iter()
            .filter(|c| c.locator != located.locator && c.printed == located.printed)
            .map(|c| Signature {
                label: SignatureLabel::Hypothesis,
                equal_to: c.locator.clone(),
            })
            .collect();
        signatures.sort_by(|x, y| x.equal_to.cmp(&y.equal_to));
        out.push(PresenceConflict {
            identity: id.id.clone(),
            located: located.locator.clone(),
            placeholder: placeholder.clone(),
            signatures,
        });
    }
    Ok(out)
}

fn put_rat(enc: &mut Encoder, q: Rat) {
    enc.put_i128(q.num());
    enc.put_i128(q.den());
}

/// The certificate's canonical digest (hex BLAKE3 of an `XFCT` v2 frame): the convention; the construction of
/// every family with `Sum` identities; the cells sorted by locator (locator, sha256, token, printed, step,
/// scope); the identities sorted by id (id, family, closure, kind and its cells and factor); then the
/// certificate (Farkas multipliers sorted by row id, or the residual). v1 (bd-4wvl.1) had no constructions.
pub fn certificate_digest(
    cells: &[Cell],
    identities: &[Identity],
    rules: &Rules,
    certificate: &Certificate,
) -> Result<String, XfRefusal> {
    let mut cells: Vec<&Cell> = cells.iter().collect();
    cells.sort_by(|a, b| a.locator.cmp(&b.locator));
    let mut identities: Vec<&Identity> = identities.iter().collect();
    identities.sort_by(|a, b| a.id.cmp(&b.id));
    let mut constructions: BTreeMap<&str, Construction> = BTreeMap::new();
    for id in &identities {
        if matches!(id.kind, IdentityKind::Sum { .. }) {
            constructions.insert(id.family.as_str(), rules.construction(id)?);
        }
    }
    let mut enc = Encoder::new_frame(*b"XFCT", 2);
    enc.put_u8(rules.convention.tag());
    enc.put_u64(constructions.len() as u64);
    for (family, c) in &constructions {
        enc.put_str(family);
        enc.put_u8(c.tag());
    }
    enc.put_u64(cells.len() as u64);
    for c in cells {
        enc.put_str(&c.locator);
        enc.put_str(&c.sha256);
        enc.put_str(&c.token);
        put_rat(&mut enc, c.printed);
        put_rat(&mut enc, c.step);
        enc.put_u8(match c.scope {
            Scope::Visible => 0,
            Scope::Hidden => 1,
            Scope::Pdf => 2,
        });
    }
    enc.put_u64(identities.len() as u64);
    for id in identities {
        enc.put_str(&id.id);
        enc.put_str(&id.family);
        enc.put_u8(id.closure_tag());
        match &id.kind {
            IdentityKind::Sum {
                total,
                lines,
                signs,
            } => {
                enc.put_u8(0);
                enc.put_str(total);
                enc.put_u64(lines.len() as u64);
                for (line, s) in lines.iter().zip(signs) {
                    enc.put_str(line);
                    enc.put_u8(u8::from(*s > 0));
                }
            }
            IdentityKind::Equal { a, b } => {
                enc.put_u8(1);
                enc.put_str(a);
                enc.put_str(b);
            }
            IdentityKind::Scale {
                target,
                source,
                factor,
            } => {
                enc.put_u8(2);
                enc.put_str(target);
                enc.put_str(source);
                put_rat(&mut enc, *factor);
            }
            IdentityKind::Ratio {
                rate,
                num,
                den,
                factor,
            } => {
                enc.put_u8(3);
                enc.put_str(rate);
                enc.put_str(num);
                enc.put_str(den);
                put_rat(&mut enc, *factor);
            }
        }
    }
    match certificate {
        Certificate::Farkas { multipliers } => {
            let sorted: BTreeMap<&str, Rat> = multipliers
                .iter()
                .map(|(id, q)| (id.as_str(), *q))
                .collect();
            enc.put_u8(0);
            enc.put_u64(sorted.len() as u64);
            for (id, q) in sorted {
                enc.put_str(id);
                put_rat(&mut enc, q);
            }
        }
        Certificate::Residual { identity, residual } => {
            enc.put_u8(1);
            enc.put_str(identity);
            put_rat(&mut enc, *residual);
        }
    }
    Ok(blake3_hash(&enc.finish()).to_hex())
}

#[cfg(test)]
mod tests {
    use super::*;
    use fmine_core::{PhiloxStream, RngKey};

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

    /// S1's rules: every sum rounded once, as each S1 known answer assumes.
    fn rules(convention: Convention) -> Rules {
        Rules::new(convention).with("sum", Construction::RoundedOnce)
    }

    fn component_sum(convention: Convention) -> Rules {
        Rules::new(convention).with("sum", Construction::ComponentSum)
    }

    fn sum(id: &str, total: &str, lines: &[&str]) -> Identity {
        Identity {
            id: id.to_string(),
            family: "sum".to_string(),
            closure: Closure::Definitional,
            kind: IdentityKind::Sum {
                total: total.to_string(),
                lines: lines.iter().map(|s| s.to_string()).collect(),
                signs: vec![1; lines.len()],
            },
        }
    }

    fn equal(id: &str, a: &str, b: &str) -> Identity {
        Identity {
            id: id.to_string(),
            family: "equal".to_string(),
            closure: Closure::Definitional,
            kind: IdentityKind::Equal {
                a: a.to_string(),
                b: b.to_string(),
            },
        }
    }

    fn ratio(id: &str, rate: &str, num: &str, den: &str, factor: Rat) -> Identity {
        Identity {
            id: id.to_string(),
            family: "rate".to_string(),
            closure: Closure::MayUseUnpublished,
            kind: IdentityKind::Ratio {
                rate: rate.to_string(),
                num: num.to_string(),
                den: den.to_string(),
                factor,
            },
        }
    }

    fn only(components: Vec<Component>) -> Outcome {
        assert_eq!(components.len(), 1, "{components:?}");
        components.into_iter().next().expect("one").outcome
    }

    fn farkas(outcome: Outcome) -> Vec<(String, Rat)> {
        match outcome {
            Outcome::Infeasible {
                certificate: Certificate::Farkas { multipliers },
            } => multipliers,
            other => panic!("not a Farkas conflict: {other:?}"),
        }
    }

    fn closed(lo: Rat, hi: Rat) -> Bound {
        Bound::of(Interval {
            lo,
            hi,
            lo_strict: false,
            hi_strict: false,
        })
    }

    fn verdict(o: &Outcome) -> &'static str {
        match o {
            Outcome::Feasible { .. } => "feasible",
            Outcome::Infeasible { .. } => "infeasible",
            Outcome::NotRefuted => "not_refuted",
            Outcome::Indeterminate { .. } => "indeterminate",
        }
    }

    #[test]
    fn test_convention_intervals() {
        let iv = |lo, hi, lo_strict, hi_strict| Interval {
            lo,
            hi,
            lo_strict,
            hi_strict,
        };
        let (v, h) = (int(3), q(1, 2));
        let (lo, hi) = (q(5, 2), q(7, 2));
        assert_eq!(
            Convention::Closed.interval(v, h),
            Ok(iv(lo, hi, false, false))
        );
        assert_eq!(
            Convention::HalfUp.interval(v, h),
            Ok(iv(lo, hi, false, true))
        );
        assert_eq!(
            Convention::HalfDown.interval(v, h),
            Ok(iv(lo, hi, true, false))
        );
        assert_eq!(
            Convention::Truncate.interval(v, h),
            Ok(iv(int(3), int(4), false, true))
        );
        // Half-even: 3 is odd at step 1, so both ends round away from it; 4 is even and keeps both.
        assert_eq!(
            Convention::HalfEven.interval(v, h),
            Ok(iv(lo, hi, true, true))
        );
        assert_eq!(
            Convention::HalfEven.interval(int(4), h),
            Ok(iv(q(7, 2), q(9, 2), false, false))
        );
        assert_eq!(
            Convention::HalfEven.interval(q(5, 2), h),
            Err(IntervalError::OffGrid)
        );
        // Truncation toward zero.
        assert_eq!(
            Convention::Truncate.interval(int(-3), h),
            Ok(iv(int(-4), int(-3), true, false))
        );
        assert_eq!(
            Convention::Truncate.interval(int(0), h),
            Ok(iv(int(-1), int(1), true, true))
        );
        let half_up = iv(lo, hi, false, true);
        assert_eq!(half_up.contains(q(5, 2)), Ok(true));
        assert_eq!(half_up.contains(q(7, 2)), Ok(false));
    }

    fn e114() -> (Vec<Cell>, Vec<Identity>) {
        // E114 (FACT): Turquoise Ridge prints 148 for Q1 2026, 130 for Q2 2026 and 281 for H1 2026, step 1.
        (
            vec![
                cell("TR:Q1 2026", int(148), int(1)),
                cell("TR:Q2 2026", int(130), int(1)),
                cell("TR:H1 2026", int(281), int(1)),
            ],
            vec![sum("e114", "TR:H1 2026", &["TR:Q1 2026", "TR:Q2 2026"])],
        )
    }

    #[test]
    fn test_e114_turquoise_ridge_certificate() {
        let (cells, ids) = e114();
        let r = rules(Convention::Closed);
        let multipliers = farkas(only(solve(&cells, &ids, &r).expect("solves")));
        assert_eq!(multipliers, vec![("e114".to_string(), int(1))]);
        let checked = check_certificate(&cells, &ids, &r, &Certificate::Farkas { multipliers })
            .expect("the checker accepts");
        // Q1 + Q2 − H1 over [147.5, 148.5] + [129.5, 130.5] − [280.5, 281.5] = [−4.5, −1.5], excluding 0.
        assert_eq!(checked.range, closed(q(-9, 2), q(-3, 2)));
        assert_eq!(checked.rhs, int(0));
    }

    /// Regression pin, not an oracle: the digest of this implementation's E114 certificate frame. The frame is
    /// v2 (bd-4wvl.2), which binds the constructions; v1's pin (7258ed7a…) no longer applies to it.
    #[test]
    fn test_certificate_digest_golden_e114() {
        let (cells, ids) = e114();
        let certificate = Certificate::Farkas {
            multipliers: vec![("e114".to_string(), int(1))],
        };
        let digest = certificate_digest(&cells, &ids, &rules(Convention::Closed), &certificate)
            .expect("digest");
        assert_eq!(
            digest,
            "22c48c04f697b9e76dfab5003041415c642508c619763586a1d9e57faf24b2dc"
        );
        // The same certificate under another construction is another frame.
        let other = certificate_digest(
            &cells,
            &ids,
            &component_sum(Convention::Closed),
            &certificate,
        )
        .expect("digest");
        assert_ne!(digest, other);
    }

    fn cod02() -> (Vec<Cell>, Identity, Identity) {
        // A total of 10 (±0.5) broken down as 5.20 + 5.20 and as 4.80 + 4.80 (±0.005 per line).
        let cent = q(1, 100);
        (
            vec![
                cell("T", int(10), int(1)),
                cell("a1", q(26, 5), cent),
                cell("a2", q(26, 5), cent),
                cell("b1", q(24, 5), cent),
                cell("b2", q(24, 5), cent),
            ],
            sum("A", "T", &["a1", "a2"]),
            sum("B", "T", &["b1", "b2"]),
        )
    }

    #[test]
    fn test_cod02_shared_total_joint_conflict() {
        let (cells, a, b) = cod02();
        let r = rules(Convention::Closed);
        for (id, lines) in [(&a, [1, 2]), (&b, [3, 4])] {
            let part = vec![
                cells[0].clone(),
                cells[lines[0]].clone(),
                cells[lines[1]].clone(),
            ];
            let ids = vec![id.clone()];
            let Outcome::Feasible { witness } = only(solve(&part, &ids, &r).expect("solves"))
            else {
                panic!("{} alone must be feasible", id.id);
            };
            assert_eq!(check_witness(&part, &ids, &r, &witness), Ok(()));
        }
        let ids = vec![a, b];
        let multipliers = farkas(only(solve(&cells, &ids, &r).expect("solves")));
        assert!(check_certificate(&cells, &ids, &r, &Certificate::Farkas { multipliers }).is_ok());
        // The hand certificate λ = (−1, +1): −a1 − a2 + b1 + b2 over the boxes is [−0.82, −0.78].
        let hand = Certificate::Farkas {
            multipliers: vec![("A".to_string(), int(-1)), ("B".to_string(), int(1))],
        };
        let checked = check_certificate(&cells, &ids, &r, &hand).expect("valid");
        assert_eq!(checked.range, closed(q(-41, 50), q(-39, 50)));
        assert!(!checked.combination.contains_key("T"));
    }

    #[test]
    fn test_tie_point_feasible_only_under_closed() {
        let cells = vec![cell("a", int(1), int(1)), cell("b", int(2), int(1))];
        let ids = vec![equal("tie", "a", "b")];
        let Outcome::Feasible { witness } =
            only(solve(&cells, &ids, &rules(Convention::Closed)).expect("solves"))
        else {
            panic!("closed must be feasible");
        };
        assert_eq!(witness.get("a"), Some(&q(3, 2)));
        assert_eq!(witness.get("b"), Some(&q(3, 2)));
        for convention in [Convention::HalfUp, Convention::HalfDown] {
            let r = rules(convention);
            let multipliers = farkas(only(solve(&cells, &ids, &r).expect("solves")));
            let checked = check_certificate(&cells, &ids, &r, &Certificate::Farkas { multipliers })
                .expect("the checker accepts");
            // The conflict sits exactly on an open end of the range.
            let open = |e: Option<End>| e.is_some_and(|e| e.strict);
            assert!(
                open(checked.range.lo) || open(checked.range.hi),
                "{checked:?}"
            );
        }
    }

    #[test]
    fn test_witness_checker_rejects_tampered_witness() {
        let cells = vec![cell("a", int(1), int(1)), cell("b", int(2), int(1))];
        let ids = vec![equal("tie", "a", "b")];
        let (closed_rules, half_up) = (rules(Convention::Closed), rules(Convention::HalfUp));
        let w = |a, b| BTreeMap::from([("a".to_string(), a), ("b".to_string(), b)]);
        assert_eq!(
            check_witness(&cells, &ids, &closed_rules, &w(q(3, 2), q(3, 2))),
            Ok(())
        );
        // Outside a's interval.
        assert!(check_witness(&cells, &ids, &closed_rules, &w(q(8, 5), q(8, 5))).is_err());
        // Inside both intervals, but a ≠ b.
        assert!(check_witness(&cells, &ids, &closed_rules, &w(q(7, 5), q(3, 2))).is_err());
        // 1.5 is on a's open upper end under half-up.
        assert!(check_witness(&cells, &ids, &half_up, &w(q(3, 2), q(3, 2))).is_err());
        // A missing value.
        let partial = BTreeMap::from([("a".to_string(), q(3, 2))]);
        assert!(check_witness(&cells, &ids, &closed_rules, &partial).is_err());
        // A ratio is checked as a ratio: 2000.4 lies in R's box, but 2000.4·50 ≠ 1000·100.
        let rc = vec![
            cell("R", int(2000), int(1)),
            cell("N", int(100), int(1)),
            cell("Q", int(50), int(1)),
        ];
        let rid = vec![ratio("rate", "R", "N", "Q", int(1000))];
        let rw = |r, n, d| {
            BTreeMap::from([
                ("R".to_string(), r),
                ("N".to_string(), n),
                ("Q".to_string(), d),
            ])
        };
        let plain = Rules::new(Convention::Closed);
        assert_eq!(
            check_witness(&rc, &rid, &plain, &rw(int(2000), int(100), int(50))),
            Ok(())
        );
        assert!(check_witness(&rc, &rid, &plain, &rw(q(10002, 5), int(100), int(50))).is_err());
    }

    #[test]
    fn test_certificate_checker_rejects_wrong_sign_multiplier() {
        let (cells, ids) = e114();
        let r = rules(Convention::Closed);
        let lambda = |pairs: &[(&str, i128)]| Certificate::Farkas {
            multipliers: pairs
                .iter()
                .map(|(id, x)| (id.to_string(), int(*x)))
                .collect(),
        };
        // −(Q1 + Q2 − H1) ranges over [1.5, 4.5]: 0 is outside, but on the other side, so −1 is also valid…
        let checked = check_certificate(&cells, &ids, &r, &lambda(&[("e114", -1)])).expect("valid");
        assert_eq!(checked.range, closed(q(3, 2), q(9, 2)));
        // …while a feasible pair has no valid certificate of either sign.
        let tie = vec![cell("a", int(1), int(1)), cell("b", int(2), int(1))];
        let tie_ids = vec![equal("tie", "a", "b")];
        for x in [1, -1] {
            assert!(check_certificate(&tie, &tie_ids, &r, &lambda(&[("tie", x)])).is_err());
        }
        // No multipliers: the range is {0}, which holds b* = 0.
        assert!(check_certificate(&cells, &ids, &r, &lambda(&[])).is_err());
        // An unknown identity.
        assert!(check_certificate(&cells, &ids, &r, &lambda(&[("nope", 1)])).is_err());
        // COD-02 with the same sign on both breakdowns: 2T is not cancelled and 0 falls inside.
        let (cod_cells, a, b) = cod02();
        let same = lambda(&[("A", -1), ("B", -1)]);
        assert!(check_certificate(&cod_cells, &[a, b], &r, &same).is_err());
    }

    #[test]
    fn test_certificate_digest_invariant_under_row_order() {
        let (cells, a, b) = cod02();
        let forward = vec![a.clone(), b.clone()];
        let backward = vec![b, a];
        let mut shuffled = cells.clone();
        shuffled.reverse();
        let r = rules(Convention::Closed);
        let run = |cells: &[Cell], ids: &[Identity]| {
            let certificate = Certificate::Farkas {
                multipliers: farkas(only(solve(cells, ids, &r).expect("solves"))),
            };
            let digest = certificate_digest(cells, ids, &r, &certificate).expect("digest");
            (certificate, digest)
        };
        assert_eq!(run(&cells, &forward), run(&shuffled, &backward));
    }

    #[test]
    fn test_caps_give_indeterminate() {
        let cells: Vec<Cell> = (0..257)
            .map(|i| cell(&format!("c{i:03}"), int(1), int(1)))
            .collect();
        let ids: Vec<Identity> = (1..257)
            .map(|i| {
                equal(
                    &format!("e{i:03}"),
                    &format!("c{:03}", i - 1),
                    &format!("c{i:03}"),
                )
            })
            .collect();
        let r = rules(Convention::Closed);
        let Outcome::Indeterminate { reason } = only(solve(&cells, &ids, &r).expect("solves"))
        else {
            panic!("257 cells must be indeterminate");
        };
        assert!(reason.contains("257 cells"), "{reason}");
        let (cells, ids) = e114();
        let Outcome::Indeterminate { reason } =
            only(solve_with_caps(&cells, &ids, &r, MAX_VARIABLES, 1).expect("solves"))
        else {
            panic!("one pivot cannot settle E114");
        };
        assert!(reason.contains("pivots"), "{reason}");
    }

    #[test]
    fn test_overflow_gives_indeterminate() {
        // i128::MAX − 1/6 does not fit: the cell's interval overflows.
        let cells = vec![
            cell("huge", int(i128::MAX), q(1, 3)),
            cell("small", int(1), int(1)),
        ];
        let ids = vec![equal("link", "huge", "small")];
        let Outcome::Indeterminate { reason } =
            only(solve(&cells, &ids, &rules(Convention::Closed)).expect("solves"))
        else {
            panic!("an overflow must be indeterminate");
        };
        assert!(reason.contains("overflow"), "{reason}");
    }

    #[test]
    fn test_refusals_name_the_input() {
        let r = rules(Convention::Closed);
        let (mut cells, ids) = e114();
        cells[0].step = int(0);
        assert!(matches!(
            solve(&cells, &ids, &r),
            Err(XfRefusal::NonPositiveStep { .. })
        ));
        let (cells, mut ids) = e114();
        ids.push(sum("e114", "TR:H1 2026", &["TR:Q1 2026"]));
        assert!(matches!(
            solve(&cells, &ids, &r),
            Err(XfRefusal::DuplicateIdentity { .. })
        ));
        let (cells, ids) = e114();
        // A sum family with no declared construction.
        assert!(matches!(
            solve(&cells, &ids, &Rules::new(Convention::Closed)),
            Err(XfRefusal::NoConstruction { .. })
        ));
        let ids = vec![equal("x", "TR:Q1 2026", "nowhere")];
        assert!(matches!(
            solve(&cells, &ids, &r),
            Err(XfRefusal::UnknownCell { .. })
        ));
        let ids = vec![equal("self", "TR:Q1 2026", "TR:Q1 2026")];
        assert!(matches!(
            solve(&cells, &ids, &r),
            Err(XfRefusal::Degenerate { .. })
        ));
        let ids = vec![ratio("r", "TR:Q1 2026", "TR:Q1 2026", "TR:Q2 2026", int(1))];
        assert!(matches!(
            solve(&cells, &ids, &r),
            Err(XfRefusal::Degenerate { .. })
        ));
        assert!(matches!(
            ratio("r", "a", "b", "c", int(1)).row(),
            Err(XfRefusal::NotLinear { .. })
        ));
        let half = vec![cell("h", q(5, 2), int(1))];
        assert!(matches!(
            solve(&half, &[], &rules(Convention::HalfEven)),
            Err(XfRefusal::OffGrid { .. })
        ));
    }

    /// Rounds `x` half-up to a multiple of `step`.
    fn round_half_up(x: Rat, step: Rat) -> Rat {
        let k = x
            .checked_div(step)
            .expect("div")
            .checked_add(q(1, 2))
            .expect("add");
        int(k.num().div_euclid(k.den()))
            .checked_mul(step)
            .expect("mul")
    }

    /// Soundness: tables printed from exact true values, rounded half-up, are never certified inconsistent,
    /// under Closed or HalfUp. Kinds: sums of 2–5 lines, a 2×2 grid of quarters and lines with row, column and
    /// half-year totals, an ownership share, and exact ties (true values on a half step).
    #[test]
    fn prop_clean_generated_tables_have_no_certified_conflict() {
        let mut rng = PhiloxStream::new(RngKey {
            seed_root: 340,
            partition: 1,
            domain: 0,
            var_group: 0,
            realization: 0,
            brick: 0,
        });
        let mut next = |n: u64| (rng.next_u64() % n) as i128;
        let (mut feasible, mut indeterminate) = (0_usize, 0_usize);
        let tables = 10_000;
        for t in 0..tables {
            let step = [int(1), q(1, 10), q(1, 100)][next(3) as usize];
            let convention = if t % 2 == 0 {
                Convention::Closed
            } else {
                Convention::HalfUp
            };
            let tie = next(4) == 0;
            let kind = next(3);
            let k_lines = 2 + next(4) as usize;
            let factor = [q(49, 200), q(3, 5), q(123, 200)][next(3) as usize];
            let mut draw = || {
                let k = next(20_001) - 10_000;
                if tie {
                    // A true value on a half step: k·step + step/2.
                    int(k)
                        .checked_mul(step)
                        .expect("mul")
                        .checked_add(step.checked_div(int(2)).expect("half"))
                        .expect("add")
                } else {
                    q(k, 7).checked_mul(step).expect("mul")
                }
            };
            let mut truth: Vec<(String, Rat)> = Vec::new();
            let mut ids: Vec<Identity> = Vec::new();
            match kind {
                0 => {
                    let mut total = int(0);
                    let mut lines = Vec::new();
                    for i in 0..k_lines {
                        let x = draw();
                        total = total.checked_add(x).expect("add");
                        truth.push((format!("l{i}"), x));
                        lines.push(format!("l{i}"));
                    }
                    truth.push(("total".to_string(), total));
                    let refs: Vec<&str> = lines.iter().map(String::as_str).collect();
                    ids.push(sum("rows", "total", &refs));
                }
                1 => {
                    let (a1, a2, b1, b2) = (draw(), draw(), draw(), draw());
                    let add = |x: Rat, y: Rat| x.checked_add(y).expect("add");
                    for (name, x) in [
                        ("aQ1", a1),
                        ("aQ2", a2),
                        ("bQ1", b1),
                        ("bQ2", b2),
                        ("aH1", add(a1, a2)),
                        ("bH1", add(b1, b2)),
                        ("tQ1", add(a1, b1)),
                        ("tQ2", add(a2, b2)),
                        ("tH1", add(add(a1, a2), add(b1, b2))),
                    ] {
                        truth.push((name.to_string(), x));
                    }
                    ids.push(sum("aH1", "aH1", &["aQ1", "aQ2"]));
                    ids.push(sum("bH1", "bH1", &["bQ1", "bQ2"]));
                    ids.push(sum("tQ1", "tQ1", &["aQ1", "bQ1"]));
                    ids.push(sum("tQ2", "tQ2", &["aQ2", "bQ2"]));
                    ids.push(sum("tH1", "tH1", &["aH1", "bH1"]));
                    ids.push(sum("tH1q", "tH1", &["tQ1", "tQ2"]));
                }
                _ => {
                    let source = draw();
                    truth.push(("source".to_string(), source));
                    truth.push((
                        "target".to_string(),
                        factor.checked_mul(source).expect("mul"),
                    ));
                    ids.push(Identity {
                        id: "share".to_string(),
                        family: "share".to_string(),
                        closure: Closure::Definitional,
                        kind: IdentityKind::Scale {
                            target: "target".to_string(),
                            source: "source".to_string(),
                            factor,
                        },
                    });
                }
            }
            let cells: Vec<Cell> = truth
                .iter()
                .map(|(name, x)| cell(name, round_half_up(*x, step), step))
                .collect();
            for c in solve(&cells, &ids, &rules(convention)).expect("solves") {
                match c.outcome {
                    Outcome::Feasible { .. } => feasible += 1,
                    Outcome::Indeterminate { .. } => indeterminate += 1,
                    other => panic!("table {t}: a clean table gave {other:?}"),
                }
            }
        }
        // Counts are of components: every table here is one connected component.
        let share = indeterminate as f64 / (feasible + indeterminate) as f64;
        println!(
            "{{\"tables\":{tables},\"seed\":340,\"feasible\":{feasible},\"infeasible\":0,\
             \"indeterminate\":{indeterminate},\"indeterminate_share\":{share}}}"
        );
        assert_eq!(feasible + indeterminate, tables);
    }

    /// S2 known answer: lines printed 1 and 1, total printed 3, step 1.
    #[test]
    fn test_rounded_once_vs_component_sum_known_answer() {
        let cells = vec![
            cell("l1", int(1), int(1)),
            cell("l2", int(1), int(1)),
            cell("t", int(3), int(1)),
        ];
        let ids = vec![sum("foot", "t", &["l1", "l2"])];
        let (adds, once) = (component_sum(Convention::Closed), rules(Convention::Closed));
        // ComponentSum: 3 − (1 + 1) = 1, a residual in printed values.
        let residual = Certificate::Residual {
            identity: "foot".to_string(),
            residual: int(1),
        };
        assert_eq!(
            only(solve(&cells, &ids, &adds).expect("solves")),
            Outcome::Infeasible {
                certificate: residual.clone()
            }
        );
        let checked = check_certificate(&cells, &ids, &adds, &residual).expect("accepts");
        // The row l1 + l2 − t at the printed values is −1.
        assert_eq!(checked.range, closed(int(-1), int(-1)));
        // RoundedOnce: the lines span [1, 3] and meet the total's [2.5, 3.5].
        let Outcome::Feasible { witness } = only(solve(&cells, &ids, &once).expect("solves"))
        else {
            panic!("rounded once must be feasible");
        };
        assert_eq!(check_witness(&cells, &ids, &once, &witness), Ok(()));
        // A wrong residual, and a residual under RoundedOnce, are rejected.
        let wrong = Certificate::Residual {
            identity: "foot".to_string(),
            residual: int(2),
        };
        assert!(check_certificate(&cells, &ids, &adds, &wrong).is_err());
        assert!(check_certificate(&cells, &ids, &once, &residual).is_err());

        // Under ComponentSum a footed total's own box does not bind: t = 1 + 1 printed 2 can equal x ∈ [2.95, 3.05]
        // (t ∈ [1, 3] as the sum of its lines), while rounded once t ∈ [1.5, 2.5] cannot.
        let cells = vec![
            cell("l1", int(1), int(1)),
            cell("l2", int(1), int(1)),
            cell("t", int(2), int(1)),
            cell("x", int(3), q(1, 10)),
        ];
        let ids = vec![sum("foot", "t", &["l1", "l2"]), equal("same", "t", "x")];
        let Outcome::Feasible { witness } = only(solve(&cells, &ids, &adds).expect("solves"))
        else {
            panic!("component sum must be feasible");
        };
        assert_eq!(check_witness(&cells, &ids, &adds, &witness), Ok(()));
        assert!(
            !closed(q(3, 2), q(5, 2))
                .contains(witness["t"])
                .expect("cmp")
        );
        assert_eq!(
            verdict(&only(solve(&cells, &ids, &once).expect("solves"))),
            "infeasible"
        );
    }

    const VELADERO_SHA256: &str =
        "2ce7b8e46b36d745228487e683b221101c4db3019ba67acedea96967486d6755";

    fn veladero_locator(a1: &str) -> String {
        format!("Barrick_Q4_2025_AISC.xlsx:Q4 2025!{a1}")
    }

    /// A cell of E691 (`Barrick_Q4_2025_AISC.xlsx` `'Q4 2025'` column J, Veladero; sha256 as registered in
    /// `registries/sources/barrick.toml`): only the values E691 quotes.
    fn veladero(a1: &str, printed: i128, token: &str) -> Cell {
        Cell {
            locator: veladero_locator(a1),
            sha256: VELADERO_SHA256.to_string(),
            token: token.to_string(),
            printed: int(printed),
            step: int(1),
            scope: Scope::Visible,
        }
    }

    fn e691() -> (Vec<Cell>, Vec<Identity>) {
        // E691 (FACT): cost of sales J5 67 ($M), ounces sold J18 47 (koz), cost of sales per ounce J19 1,526.
        (
            vec![
                veladero("J5", 67, "67"),
                veladero("J18", 47, "47"),
                veladero("J19", 1526, "1,526"),
            ],
            vec![ratio(
                "cos_per_oz",
                &veladero_locator("J19"),
                &veladero_locator("J5"),
                &veladero_locator("J18"),
                int(1000),
            )],
        )
    }

    #[test]
    fn test_e691_veladero_implied_q() {
        let (mut cells, mut ids) = e691();
        let r = Rules::new(Convention::Closed);
        let ounces = veladero_locator("J18");
        // As printed, no ounce figure in [46.5, 47.5] fits: a checked conflict.
        assert!(!farkas(only(solve(&cells, &ids, &r).expect("solves"))).is_empty());
        // Q freed: Q = 1000·N/R over N ∈ [66.5, 67.5], R ∈ [1525.5, 1526.5] is [66500/1526.5, 67500/1525.5]
        // = [133000/3053, 5000/113] ≈ [43.56, 44.25], both ends reached.
        let implied = implied_interval(&cells, &ids, &r, &ounces, true).expect("implied");
        assert_eq!(
            implied,
            Interval {
                lo: q(133_000, 3053),
                hi: q(5000, 113),
                lo_strict: false,
                hi_strict: false
            }
        );
        // It lies wholly below 47's own box [46.5, 47.5].
        assert_eq!(implied.hi.checked_cmp(q(93, 2)), Ok(Ordering::Less));
        // With their own box the ounces have no value at all.
        assert_eq!(
            implied_interval(&cells, &ids, &r, &ounces, false),
            Err(ImpliedError::Infeasible)
        );
        // Adding the other two rates, each numerator with aisc.rs's k half-widths. k counts the numeric line
        // cells (`aisc.rs` `Calc::Rate`): all of J5:J9 for total cash costs (k = 5) and all of J5:J9 and J11:J16
        // for AISC (k = 11), read locally from the workbook's cell types, not values.
        let with_rates = |cells: &mut Vec<Cell>, ids: &mut Vec<Identity>, aisc_k: i128| {
            cells.push(Cell {
                step: int(5),
                ..veladero("J10", 39, "39")
            });
            cells.push(veladero("J20", 886, "886"));
            cells.push(Cell {
                step: int(aisc_k),
                ..veladero("J17", 84, "84")
            });
            cells.push(veladero("J21", 1915, "1,915"));
            for (id, rate, num) in [("tcc_per_oz", "J20", "J10"), ("aisc_per_oz", "J21", "J17")] {
                ids.push(ratio(
                    id,
                    &veladero_locator(rate),
                    &veladero_locator(num),
                    &ounces,
                    int(1000),
                ));
            }
        };
        // TCC 39 ± 2.5 at 886 and AISC 84 ± 5.5 at 1,915 allow Q in [1000·36.5/886.5, 1000·41.5/885.5] and
        // [1000·78.5/1915.5, 1000·89.5/1914.5], both ⊇ the interval: it is unchanged, as E691's inference says.
        let (mut all_cells, mut all_ids) = (cells.clone(), ids.clone());
        with_rates(&mut all_cells, &mut all_ids, 11);
        assert_eq!(
            implied_interval(&all_cells, &all_ids, &r, &ounces, true),
            Ok(implied)
        );
        // The inference needs k ≥ 2 for AISC: at k = 1 its [1000·83.5/1915.5, 1000·84.5/1914.5] binds on both
        // sides, giving [167000/3831, 169000/3829] ≈ [43.59, 44.14].
        with_rates(&mut cells, &mut ids, 1);
        assert_eq!(
            implied_interval(&cells, &ids, &r, &ounces, true),
            Ok(Interval {
                lo: q(167_000, 3831),
                hi: q(169_000, 3829),
                lo_strict: false,
                hi_strict: false
            })
        );
    }

    #[test]
    fn test_implied_interval_unrounding_and_open_ends() {
        // Q1 148 + Q2 130 = H1 279, step 1: Q1 = H1 − Q2 ∈ [278.5 − 130.5, 279.5 − 129.5] = [148, 150].
        let cells = vec![
            cell("Q1", int(148), int(1)),
            cell("Q2", int(130), int(1)),
            cell("H1", int(279), int(1)),
        ];
        let ids = vec![sum("h1", "H1", &["Q1", "Q2"])];
        let iv = |lo, hi, lo_strict, hi_strict| Interval {
            lo,
            hi,
            lo_strict,
            hi_strict,
        };
        let (closed_rules, half_up) = (rules(Convention::Closed), rules(Convention::HalfUp));
        // Closed: Q1's own box [147.5, 148.5] cuts it to [148, 148.5]; freed, it is [148, 150].
        assert_eq!(
            implied_interval(&cells, &ids, &closed_rules, "Q1", false),
            Ok(iv(int(148), q(297, 2), false, false))
        );
        assert_eq!(
            implied_interval(&cells, &ids, &closed_rules, "Q1", true),
            Ok(iv(int(148), int(150), false, false))
        );
        // Half-up: Q2 < 130.5 strictly, so Q1 > 148; Q1 < 148.5 by its own box. Freed: H1 < 279.5 as well, so
        // Q1 < 150. Every end is open.
        assert_eq!(
            implied_interval(&cells, &ids, &half_up, "Q1", false),
            Ok(iv(int(148), q(297, 2), true, true))
        );
        assert_eq!(
            implied_interval(&cells, &ids, &half_up, "Q1", true),
            Ok(iv(int(148), int(150), true, true))
        );
        // A lone freed cell is unbounded; an unknown cell is refused.
        let lone = vec![cell("x", int(1), int(1))];
        assert_eq!(
            implied_interval(&lone, &[], &closed_rules, "x", true),
            Err(ImpliedError::Unbounded)
        );
        assert!(matches!(
            implied_interval(&lone, &[], &closed_rules, "y", true),
            Err(ImpliedError::Refused(XfRefusal::UnknownVariable { .. }))
        ));
        // E691 from the rate side: 1000·N/Q over N ∈ [66.5, 67.5], Q ∈ [46.5, 47.5] is [1400, 45000/31]
        // (≈ 1451.61), wholly below 1,526's box, which is therefore empty.
        let (cells, ids) = e691();
        let cost_per_oz = veladero_locator("J19");
        assert_eq!(
            implied_interval(&cells, &ids, &closed_rules, &cost_per_oz, true),
            Ok(iv(int(1400), q(45_000, 31), false, false))
        );
        assert_eq!(
            implied_interval(&cells, &ids, &closed_rules, &cost_per_oz, false),
            Err(ImpliedError::Infeasible)
        );
    }

    /// A rate's range by Charnes–Cooper, with its ends settled by checked runs.
    #[test]
    fn test_rate_implied_interval_charnes_cooper() {
        let cells = vec![
            cell("R", int(2000), int(1)),
            cell("N", int(100), int(1)),
            cell("Q", int(50), int(1)),
        ];
        let ids = vec![ratio("rate", "R", "N", "Q", int(1000))];
        let iv = |lo, hi, lo_strict, hi_strict| Interval {
            lo,
            hi,
            lo_strict,
            hi_strict,
        };
        let (closed_rules, half_up) = (
            Rules::new(Convention::Closed),
            Rules::new(Convention::HalfUp),
        );
        // Closed: [1000·99.5/50.5, 1000·100.5/49.5] = [199000/101, 201000/99], cut by R's [1999.5, 2000.5].
        let range = (q(199_000, 101), q(201_000, 99));
        assert_eq!(
            implied_interval(&cells, &ids, &closed_rules, "R", true),
            Ok(iv(range.0, range.1, false, false))
        );
        assert_eq!(
            implied_interval(&cells, &ids, &closed_rules, "R", false),
            Ok(iv(q(3999, 2), q(4001, 2), false, false))
        );
        // Half-up: N < 100.5 and Q < 50.5, so neither extreme is reached; R's own box is [1999.5, 2000.5).
        assert_eq!(
            implied_interval(&cells, &ids, &half_up, "R", true),
            Ok(iv(range.0, range.1, true, true))
        );
        assert_eq!(
            implied_interval(&cells, &ids, &half_up, "R", false),
            Ok(iv(q(3999, 2), q(4001, 2), false, true))
        );
        // A negative denominator: 1000·N/Q over N ∈ [99.5, 100.5], Q ∈ [−50.5, −49.5].
        let negative = vec![
            cell("R", int(-2000), int(1)),
            cell("N", int(100), int(1)),
            cell("Q", int(-50), int(1)),
        ];
        assert_eq!(
            implied_interval(&negative, &ids, &closed_rules, "R", true),
            Ok(iv(q(-201_000, 99), q(-199_000, 101), false, false))
        );
        // A rate whose ratio range only touches its own box at an open end has no value: under half-up,
        // R printed 2031 has box [2030.5, 2031.5), beyond the ratio's supremum 201000/99 ≈ 2030.30.
        let beyond = vec![
            cell("R", int(2031), int(1)),
            cell("N", int(100), int(1)),
            cell("Q", int(50), int(1)),
        ];
        assert_eq!(
            implied_interval(&beyond, &ids, &half_up, "R", false),
            Err(ImpliedError::Infeasible)
        );
        // Every reported end agrees with a feasibility run that pins the rate there.
        for (r, convention) in [
            (closed_rules, Convention::Closed),
            (half_up, Convention::HalfUp),
        ] {
            let implied = implied_interval(&cells, &ids, &r, "R", true).expect("implied");
            for (end, strict) in [
                (implied.lo, implied.lo_strict),
                (implied.hi, implied.hi_strict),
            ] {
                let pinned = vec![
                    cell("R", end, q(1, 1_000_000)),
                    cells[1].clone(),
                    cells[2].clone(),
                ];
                // A box of ±1/2·10⁻⁶ round the end meets the region on its inside either way.
                let outcome = only(solve(&pinned, &ids, &Rules::new(convention)).expect("solves"));
                assert_eq!(
                    verdict(&outcome),
                    "feasible",
                    "{convention:?} end {end:?} strict {strict}"
                );
            }
        }
    }

    /// "t carries k half-widths": a component-sum numerator over its k lines gives the same verdict as the
    /// printed numerator at step k (`aisc.rs`'s N ± k/2), whenever the lines foot.
    #[test]
    fn prop_component_sum_numerator_matches_k_half_widths() {
        let mut rng = PhiloxStream::new(RngKey {
            seed_root: 340,
            partition: 2,
            domain: 0,
            var_group: 0,
            realization: 0,
            brick: 0,
        });
        let mut next = |n: u64| (rng.next_u64() % n) as i128;
        let (adds, plain) = (
            component_sum(Convention::Closed),
            Rules::new(Convention::Closed),
        );
        let rate = ratio("rate", "R", "N", "Q", int(1000));
        let (mut feasible, mut infeasible) = (0_usize, 0_usize);
        let cases = 2_000;
        for case in 0..cases {
            let k = 1 + next(6);
            let lines: Vec<i128> = (0..k).map(|_| next(351) - 50).collect();
            let n: i128 = lines.iter().sum();
            let ounces = 1 + next(300);
            // r within about twice the rate range's half-width of 1000·n/q.
            let spread = 2 + (1000 * k) / ounces + (1000 * n.abs()) / (ounces * ounces);
            let r = (1000 * n).div_euclid(ounces) + next((2 * spread + 1) as u64) - spread;
            let names: Vec<String> = (0..k).map(|i| format!("l{i}")).collect();
            let mut cells: Vec<Cell> = names
                .iter()
                .zip(&lines)
                .map(|(name, &x)| cell(name, int(x), int(1)))
                .collect();
            cells.extend([
                cell("N", int(n), int(1)),
                cell("Q", int(ounces), int(1)),
                cell("R", int(r), int(1)),
            ]);
            let refs: Vec<&str> = names.iter().map(String::as_str).collect();
            let ids = vec![sum("num", "N", &refs), rate.clone()];
            let by_lines = verdict(&only(solve(&cells, &ids, &adds).expect("solves")));
            let narrow = vec![
                cell("N", int(n), int(k)),
                cell("Q", int(ounces), int(1)),
                cell("R", int(r), int(1)),
            ];
            let by_k = verdict(&only(
                solve(&narrow, std::slice::from_ref(&rate), &plain).expect("solves"),
            ));
            assert_eq!(
                by_lines, by_k,
                "case {case}: lines {lines:?}, q {ounces}, r {r}"
            );
            match by_lines {
                "feasible" => feasible += 1,
                "infeasible" => infeasible += 1,
                other => panic!("case {case}: {other}"),
            }
        }
        println!(
            "{{\"test\":\"prop_component_sum_numerator\",\"cases\":{cases},\"seed\":340,\
             \"feasible\":{feasible},\"infeasible\":{infeasible},\"disagree\":0}}"
        );
        assert!(feasible > cases / 10 && infeasible > cases / 10);
    }

    #[test]
    fn test_shared_rate_cell_is_not_linearised() {
        let cells = vec![
            cell("R", int(2000), int(1)),
            cell("N", int(100), int(1)),
            cell("Q", int(50), int(1)),
            cell("S", int(2000), int(1)),
            cell("N2", int(100), int(1)),
            cell("Q2", int(50), int(1)),
        ];
        let r = Rules::new(Convention::Closed);
        let base = ratio("rate", "R", "N", "Q", int(1000));
        for other in [
            equal("same", "R", "S"),
            ratio("again", "R", "N2", "Q2", int(1000)),
        ] {
            let ids = vec![base.clone(), other];
            let components = solve(&cells, &ids, &r).expect("solves");
            let with_rate: Vec<&Component> = components
                .iter()
                .filter(|c| c.cells.iter().any(|l| l == "R"))
                .collect();
            assert_eq!(with_rate.len(), 1);
            match &with_rate[0].outcome {
                Outcome::Indeterminate { reason } => {
                    assert!(reason.contains("bilinear"), "{reason}")
                }
                other => panic!("a shared rate cell gave {other:?}"),
            }
            assert!(matches!(
                implied_interval(&cells, &ids, &r, "N", true),
                Err(ImpliedError::Indeterminate(reason)) if reason.contains("bilinear")
            ));
        }
    }

    #[test]
    fn test_den_interval_through_zero_is_refused() {
        let ids = vec![ratio("rate", "R", "N", "Q", int(1000))];
        let r = Rules::new(Convention::Closed);
        // [−0.5, 0.5], [0, 2] and [−0.75, 0.25] all hold zero.
        for (printed, step) in [(int(0), int(1)), (int(1), int(2)), (q(-1, 4), int(1))] {
            let cells = vec![
                cell("R", int(2000), int(1)),
                cell("N", int(100), int(1)),
                cell("Q", printed, step),
            ];
            assert!(matches!(
                solve(&cells, &ids, &r),
                Err(XfRefusal::DenominatorSpansZero { .. })
            ));
            assert!(matches!(
                implied_interval(&cells, &ids, &r, "N", true),
                Err(ImpliedError::Refused(
                    XfRefusal::DenominatorSpansZero { .. }
                ))
            ));
        }
        // Half-down at 1, step 2, is (0, 2]: zero is excluded, so the ratio is linearised (N = 1, Q = 0.5).
        let cells = vec![
            cell("R", int(2000), int(1)),
            cell("N", int(1), int(1)),
            cell("Q", int(1), int(2)),
        ];
        let outcome = only(solve(&cells, &ids, &Rules::new(Convention::HalfDown)).expect("solves"));
        assert_eq!(verdict(&outcome), "feasible", "{outcome:?}");
    }

    #[test]
    fn test_verdicts_invariant_under_row_permutation_and_exact_scaling() {
        let thousand = int(1000);
        // Every printed value and step ×1000 (or only those named).
        let scaled = |cells: &[Cell], which: Option<&[String]>| -> Vec<Cell> {
            cells
                .iter()
                .map(|c| {
                    let mut c = c.clone();
                    if which.is_none_or(|w| w.contains(&c.locator)) {
                        c.printed = c.printed.checked_mul(thousand).expect("mul");
                        c.step = c.step.checked_mul(thousand).expect("mul");
                    }
                    c
                })
                .collect()
        };
        let (e114_cells, e114_ids) = e114();
        let (cod_cells, a, b) = cod02();
        let foot = (
            vec![
                cell("l1", int(1), int(1)),
                cell("l2", int(1), int(1)),
                cell("t", int(3), int(1)),
            ],
            vec![sum("foot", "t", &["l1", "l2"])],
        );
        let tie = (
            vec![cell("a", int(1), int(1)), cell("b", int(2), int(1))],
            vec![equal("tie", "a", "b")],
        );
        let (e691_cells, e691_ids) = e691();
        // E691 is scaled in its quantities only ($M → $k, koz → oz): the rate is unchanged.
        let e691_scaled = [veladero_locator("J5"), veladero_locator("J18")];
        // (name, cells, identities, rules, cells to scale (all when None), expected verdict)
        type Case<'a> = (
            &'a str,
            Vec<Cell>,
            Vec<Identity>,
            Rules,
            Option<&'a [String]>,
            &'a str,
        );
        let cases: Vec<Case> = vec![
            (
                "e114",
                e114_cells,
                e114_ids,
                rules(Convention::Closed),
                None,
                "infeasible",
            ),
            (
                "cod02",
                cod_cells,
                vec![a, b],
                rules(Convention::Closed),
                None,
                "infeasible",
            ),
            (
                "foot/component",
                foot.0.clone(),
                foot.1.clone(),
                component_sum(Convention::Closed),
                None,
                "infeasible",
            ),
            (
                "foot/once",
                foot.0,
                foot.1,
                rules(Convention::Closed),
                None,
                "feasible",
            ),
            (
                "tie/closed",
                tie.0.clone(),
                tie.1.clone(),
                rules(Convention::Closed),
                None,
                "feasible",
            ),
            (
                "tie/half-up",
                tie.0,
                tie.1,
                rules(Convention::HalfUp),
                None,
                "infeasible",
            ),
            (
                "e691",
                e691_cells.clone(),
                e691_ids.clone(),
                Rules::new(Convention::Closed),
                Some(&e691_scaled[..]),
                "infeasible",
            ),
        ];
        for (name, cells, ids, r, which, expected) in &cases {
            let base = verdict(&only(solve(cells, ids, r).expect("solves")));
            assert_eq!(base, *expected, "{name}");
            let mut rev_cells = cells.clone();
            rev_cells.reverse();
            let mut rev_ids = ids.clone();
            rev_ids.reverse();
            assert_eq!(
                verdict(&only(solve(&rev_cells, &rev_ids, r).expect("solves"))),
                base,
                "{name} reversed"
            );
            assert_eq!(
                verdict(&only(
                    solve(&scaled(cells, *which), ids, r).expect("solves")
                )),
                base,
                "{name} ×1000"
            );
        }
        // The implied ounces scale exactly: koz → oz.
        let r = Rules::new(Convention::Closed);
        let ounces = veladero_locator("J18");
        let implied = implied_interval(
            &scaled(&e691_cells, Some(&e691_scaled[..])),
            &e691_ids,
            &r,
            &ounces,
            true,
        )
        .expect("implied");
        assert_eq!(
            (implied.lo, implied.hi),
            (q(133_000_000, 3053), q(5_000_000, 113))
        );
    }

    /// A checked run with the printed values of `unprinted` disregarded.
    fn feasible_without(cells: &[Cell], ids: &[Identity], r: &Rules, unprinted: &[&str]) -> bool {
        let cells: Vec<&Cell> = cells.iter().collect();
        let ids: Vec<&Identity> = ids.iter().collect();
        let frame = Frame {
            unprinted: unprinted.iter().map(|s| (*s).to_string()).collect(),
            bounds: BTreeMap::new(),
        };
        matches!(
            checked_run(&cells, &ids, r, &frame).expect("a checked run"),
            Checked::Feasible
        )
    }

    fn blamed(outcome: &BlameOutcome) -> &Blame {
        match outcome {
            BlameOutcome::Blamed(b) => b,
            other => panic!("not blamed: {other:?}"),
        }
    }

    /// Every subset of `all` of size `k`.
    fn subsets<'a>(all: &[&'a str], k: usize) -> Vec<Vec<&'a str>> {
        let mut out = Vec::new();
        let mut idx: Vec<usize> = (0..k).collect();
        if k > all.len() {
            return out;
        }
        loop {
            out.push(idx.iter().map(|&i| all[i]).collect());
            if !next_combination(&mut idx, all.len()) {
                return out;
            }
        }
    }

    /// Blame on seeded perturbed tables (a sum of 2–4 lines, a 2×2 grid with row, column and half-year totals,
    /// a share), each table one component, checked against the definitions by brute force:
    /// - every conflict set is certified ([`check_conflict_set`]), and disregarding any member makes it feasible;
    /// - every correction set restores feasibility, and dropping any member undoes that;
    /// - under `Minimum`, no smaller subset restores feasibility, and the reported sets are every subset of their
    ///   size that does. The brute force tries all subsets, without the search's hitting-set pruning.
    #[test]
    fn prop_deletion_filter_set_is_inclusion_minimal() {
        let mut rng = PhiloxStream::new(RngKey {
            seed_root: 340,
            partition: 4,
            domain: 0,
            var_group: 0,
            realization: 0,
            brick: 0,
        });
        let mut next = |n: u64| (rng.next_u64() % n) as i128;
        let r = rules(Convention::Closed);
        let (tables, mut blamed_tables, mut conflicts, mut multi_conflict) = (400, 0, 0, 0);
        for t in 0..tables {
            let kind = next(3);
            let k_lines = 2 + next(3) as usize;
            let mut draw = || next(201) - 100;
            let mut truth: Vec<(String, i128)> = Vec::new();
            let mut ids: Vec<Identity> = Vec::new();
            match kind {
                0 => {
                    let lines: Vec<i128> = (0..k_lines).map(|_| draw()).collect();
                    let names: Vec<String> = (0..k_lines).map(|i| format!("l{i}")).collect();
                    for (n, x) in names.iter().zip(&lines) {
                        truth.push((n.clone(), *x));
                    }
                    truth.push(("total".to_string(), lines.iter().sum()));
                    let refs: Vec<&str> = names.iter().map(String::as_str).collect();
                    ids.push(sum("rows", "total", &refs));
                }
                1 => {
                    let (a1, a2, b1, b2) = (draw(), draw(), draw(), draw());
                    for (name, x) in [
                        ("aQ1", a1),
                        ("aQ2", a2),
                        ("bQ1", b1),
                        ("bQ2", b2),
                        ("aH1", a1 + a2),
                        ("bH1", b1 + b2),
                        ("tQ1", a1 + b1),
                        ("tQ2", a2 + b2),
                        ("tH1", a1 + a2 + b1 + b2),
                    ] {
                        truth.push((name.to_string(), x));
                    }
                    ids.push(sum("aH1", "aH1", &["aQ1", "aQ2"]));
                    ids.push(sum("bH1", "bH1", &["bQ1", "bQ2"]));
                    ids.push(sum("tQ1", "tQ1", &["aQ1", "bQ1"]));
                    ids.push(sum("tQ2", "tQ2", &["aQ2", "bQ2"]));
                    ids.push(sum("tH1", "tH1", &["aH1", "bH1"]));
                }
                _ => {
                    // 100% and an attributable 50% share, in two quarters and the half year.
                    let (s1, s2) = (2 * draw(), 2 * draw());
                    for (name, x) in [
                        ("q1", s1),
                        ("q2", s2),
                        ("h1", s1 + s2),
                        ("q1a", s1 / 2),
                        ("q2a", s2 / 2),
                    ] {
                        truth.push((name.to_string(), x));
                    }
                    ids.push(sum("h1", "h1", &["q1", "q2"]));
                    for (id, target, source) in [("s1", "q1a", "q1"), ("s2", "q2a", "q2")] {
                        ids.push(Identity {
                            id: id.to_string(),
                            family: "share".to_string(),
                            closure: Closure::Definitional,
                            kind: IdentityKind::Scale {
                                target: target.to_string(),
                                source: source.to_string(),
                                factor: q(1, 2),
                            },
                        });
                    }
                }
            }
            // Misprint one or two cells by 2 to 5 steps.
            let misprints = 1 + next(2) as usize;
            for _ in 0..misprints {
                let i = next(truth.len() as u64) as usize;
                let shift = (2 + next(4)) * if next(2) == 0 { 1 } else { -1 };
                truth[i].1 += shift;
            }
            let cells: Vec<Cell> = truth
                .iter()
                .map(|(name, x)| cell(name, int(*x), int(1)))
                .collect();
            let all: Vec<&str> = cells.iter().map(|c| c.locator.as_str()).collect();
            let found = blame(&cells, &ids, &r).expect("blames");
            let solved_infeasible = solve(&cells, &ids, &r)
                .expect("solves")
                .iter()
                .any(|c| verdict(&c.outcome) == "infeasible");
            assert_eq!(found.len(), usize::from(solved_infeasible), "table {t}");
            let Some(component) = found.first() else {
                continue;
            };
            blamed_tables += 1;
            let b = blamed(&component.outcome);
            assert_eq!(b.label, CorrectionLabel::Minimum, "table {t}");
            assert!(!b.conflicts.is_empty(), "table {t}");
            conflicts += b.conflicts.len();
            multi_conflict += usize::from(b.conflicts.len() > 1);
            for set in &b.conflicts {
                assert!(
                    check_conflict_set(&cells, &ids, &r, set).is_ok(),
                    "table {t}: {set:?}"
                );
                let others: Vec<&str> = all
                    .iter()
                    .copied()
                    .filter(|c| !set.cells.iter().any(|s| s == c))
                    .collect();
                assert!(!feasible_without(&cells, &ids, &r, &others), "table {t}");
                for m in &set.cells {
                    let mut freed = others.clone();
                    freed.push(m);
                    assert!(
                        feasible_without(&cells, &ids, &r, &freed),
                        "table {t}: {m} is not needed in {:?}",
                        set.cells
                    );
                }
            }
            let size = b.corrections[0].cells.len();
            for set in &b.corrections {
                assert_eq!(set.cells.len(), size, "table {t}");
                let cells_of: Vec<&str> = set.cells.iter().map(String::as_str).collect();
                assert!(feasible_without(&cells, &ids, &r, &cells_of), "table {t}");
                for m in &cells_of {
                    let rest: Vec<&str> = cells_of.iter().copied().filter(|c| c != m).collect();
                    assert!(!feasible_without(&cells, &ids, &r, &rest), "table {t}");
                }
            }
            for k in 1..size {
                for s in subsets(&all, k) {
                    assert!(
                        !feasible_without(&cells, &ids, &r, &s),
                        "table {t}: {s:?} is smaller"
                    );
                }
            }
            let mut every: Vec<Vec<String>> = subsets(&all, size)
                .into_iter()
                .filter(|s| feasible_without(&cells, &ids, &r, s))
                .map(|s| {
                    let mut s: Vec<String> = s.iter().map(|x| (*x).to_string()).collect();
                    s.sort();
                    s
                })
                .collect();
            every.sort();
            let mut reported: Vec<Vec<String>> =
                b.corrections.iter().map(|c| c.cells.clone()).collect();
            reported.sort();
            assert_eq!(reported, every, "table {t}");
        }
        println!(
            "{{\"test\":\"prop_deletion_filter\",\"tables\":{tables},\"seed\":340,\"blamed\":{blamed_tables},\
             \"conflict_sets\":{conflicts},\"multi_conflict\":{multi_conflict},\"violations\":0}}"
        );
        assert!(blamed_tables > tables / 2 && multi_conflict > tables / 20);
    }

    #[test]
    fn test_correction_label_minimum_only_when_exhaustive() {
        // A chain c00 = c01 = … = c20, every cell printed 1 but c20 printed 5: 21 candidate cells.
        let chain = |n: usize| -> (Vec<Cell>, Vec<Identity>) {
            let cells = (0..n)
                .map(|i| {
                    cell(
                        &format!("c{i:02}"),
                        int(if i + 1 == n { 5 } else { 1 }),
                        int(1),
                    )
                })
                .collect();
            let ids = (1..n)
                .map(|i| {
                    equal(
                        &format!("e{i:02}"),
                        &format!("c{:02}", i - 1),
                        &format!("c{i:02}"),
                    )
                })
                .collect();
            (cells, ids)
        };
        let r = Rules::new(Convention::Closed);
        let (cells, ids) = chain(21);
        let only_blame = |found: Vec<ComponentBlame>| {
            assert_eq!(found.len(), 1);
            blamed(&found[0].outcome).clone()
        };
        let greedy = only_blame(blame(&cells, &ids, &r).expect("blames"));
        assert_eq!(greedy.label, CorrectionLabel::InclusionMinimalOnly);
        assert_eq!(greedy.corrections.len(), 1);
        assert_eq!(greedy.corrections[0].cells, vec!["c20".to_string()]);
        assert_eq!(
            greedy.corrections[0].implied,
            vec![(
                "c20".to_string(),
                Ok(Interval {
                    lo: q(1, 2),
                    hi: q(3, 2),
                    lo_strict: false,
                    hi_strict: false
                })
            )]
        );
        // Every conflict pairs c20 with one other cell. The seeded filter finds 19 of the 20: c00 is disregarded
        // first in every seeded order, so {c00, c20} is never kept. The correction search does not need them all.
        assert_eq!(greedy.conflicts.len(), 19);
        assert!(
            greedy
                .conflicts
                .iter()
                .all(|c| c.cells.len() == 2 && c.cells[1] == "c20")
        );
        // The same 21 cells searched exhaustively: the same set, now labelled minimum.
        let exhaustive =
            only_blame(blame_with_caps(&cells, &ids, &r, 21, MAX_BLAME_RUNS).expect("blames"));
        assert_eq!(exhaustive.label, CorrectionLabel::Minimum);
        assert_eq!(exhaustive.corrections, greedy.corrections);
        // Twenty cells, but the run cap reached before the search completes: not minimum.
        let (cells, ids) = chain(20);
        let capped = only_blame(blame_with_caps(&cells, &ids, &r, 20, 1).expect("blames"));
        assert_eq!(capped.label, CorrectionLabel::InclusionMinimalOnly);
        let full = only_blame(blame(&cells, &ids, &r).expect("blames"));
        assert_eq!(full.label, CorrectionLabel::Minimum);
    }

    /// Disregarding a cell of a component-sum footing drops the whole printed footing (the cell's printed value is
    /// not re-derived from it), so each footing cell alone is a minimum correction: the total then ranges over
    /// the sum of the lines, while a line is unbounded.
    #[test]
    fn test_component_sum_blame_disregards_the_footing() {
        let cells = vec![
            cell("l1", int(1), int(1)),
            cell("l2", int(1), int(1)),
            cell("t", int(3), int(1)),
        ];
        let ids = vec![sum("foot", "t", &["l1", "l2"])];
        let r = component_sum(Convention::Closed);
        let found = blame(&cells, &ids, &r).expect("blames");
        let b = blamed(&found[0].outcome);
        assert_eq!(b.conflicts.len(), 1);
        assert_eq!(b.conflicts[0].cells, vec!["l1", "l2", "t"]);
        assert_eq!(
            b.conflicts[0].certificate,
            Certificate::Residual {
                identity: "foot".to_string(),
                residual: int(1)
            }
        );
        assert!(check_conflict_set(&cells, &ids, &r, &b.conflicts[0]).is_ok());
        assert_eq!(b.label, CorrectionLabel::Minimum);
        let sets: Vec<&Vec<String>> = b.corrections.iter().map(|c| &c.cells).collect();
        assert_eq!(
            sets,
            [
                &vec!["l1".to_string()],
                &vec!["l2".to_string()],
                &vec!["t".to_string()]
            ]
        );
        assert_eq!(
            b.corrections[2].implied,
            vec![(
                "t".to_string(),
                Ok(Interval {
                    lo: int(1),
                    hi: int(3),
                    lo_strict: false,
                    hi_strict: false
                })
            )]
        );
        assert_eq!(b.corrections[0].implied[0].1, Err(ImpliedError::Unbounded));
        // A feasible problem has no blame.
        let fine = vec![
            cell("l1", int(1), int(1)),
            cell("l2", int(1), int(1)),
            cell("t", int(2), int(1)),
        ];
        assert!(blame(&fine, &ids, &r).expect("blames").is_empty());
    }

    #[test]
    fn test_presence_conflicts_refuse_and_locate() {
        let cells = vec![cell("x", int(7), int(1)), cell("y", int(7), int(1))];
        let dash = |loc: &str| Placeholder {
            locator: loc.to_string(),
            sha256: "0".repeat(64),
            token: "—".to_string(),
        };
        let found = presence_conflicts(&cells, &[dash("p")], &[equal("eq", "p", "x")]).expect("ok");
        assert_eq!(
            found,
            vec![PresenceConflict {
                identity: "eq".to_string(),
                located: "x".to_string(),
                placeholder: "p".to_string(),
                signatures: vec![Signature {
                    label: SignatureLabel::Hypothesis,
                    equal_to: "y".to_string()
                }],
            }]
        );
        // Two placeholders agree; a placeholder in a sum is refused; an unknown locator is refused.
        let both = [dash("p"), dash("p2")];
        assert!(
            presence_conflicts(&cells, &both, &[equal("eq", "p", "p2")])
                .expect("ok")
                .is_empty()
        );
        assert!(matches!(
            presence_conflicts(&cells, &both, &[sum("s", "x", &["p", "y"])]),
            Err(XfRefusal::PlaceholderInIdentity { .. })
        ));
        assert!(matches!(
            presence_conflicts(&cells, &both, &[equal("eq", "p", "z")]),
            Err(XfRefusal::UnknownCell { .. })
        ));
        assert!(matches!(
            presence_conflicts(&cells, &[dash("x")], &[]),
            Err(XfRefusal::DuplicateCell { .. })
        ));
    }
}
