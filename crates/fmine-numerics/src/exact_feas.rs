//! Exact rounding-interval feasibility (bd-4wvl.1, BD-340.S1): can a set of printed values all be roundings of
//! true values that satisfy a set of linear identities exactly?
//!
//! - Each printed value `v` with step `2h` stands for an interval of true values, set by a tie [`Convention`].
//! - Each [`Identity`] is a row `Σ aⱼ·xⱼ = 0` over cells (`Sum`, `Equal`, `Scale`).
//! - Cells linked by identities form components. Each is solved by an exact two-phase simplex over [`Rat`] with
//!   Bland's rule, so it terminates. Bounds are explicit rows `y + s = w` on `y = x − lo`: the same polytope as a
//!   bounded-variable simplex, at a higher pivot cost.
//! - Open interval ends: one extra `t ∈ [0, 1]` is maximised subject to `x ≥ lo + t` on open lower ends and
//!   `x ≤ hi − t` on open upper ends. The cells are strictly feasible exactly when `t* > 0`.
//!
//! Outcomes: [`Outcome::Feasible`] with a witness, [`Outcome::Infeasible`] with Farkas multipliers over the
//! identities, or [`Outcome::Indeterminate`] (a cap, an overflow, or a checker rejection), never a claim.
//! [`Outcome::NotRefuted`] is reserved for relaxations (S6).
//!
//! Checkers are independent of the simplex. [`check_witness`] checks every cell against its interval and every
//! identity exactly. [`check_certificate`] computes `c = Σ λᵢ·aᵢ` and `b* = Σ λᵢ·bᵢ`, then the exact range of
//! `cᵀx` over the box with its open ends; the certificate is valid when `b*` lies outside that range. An outcome
//! its checker rejects becomes `Indeterminate { "checker rejected" }`. Both checkers share only the linearisation
//! of identities ([`Identity::row`]) and [`Convention::interval`] with the solver.
//!
//! Certificate multipliers come from the simplex duals of the identity rows. With the duals `p` of the phase
//! that stopped (phase 1 when the closed box is infeasible, the `t` phase when only open ends fail), `λ = p`
//! gives `λᵀb ≥ sup cᵀx` over the box, with the supremum either exceeded or reached only at an open end.
//! Multipliers are scaled so the first non-zero one, by identity id, is ±1.
//!
//! K0: std only, no I/O. Determinism: exact rational arithmetic, inputs sorted by locator and id before solving.

use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::fmt;

use fmine_core::{Encoder, blake3_hash};
use fmine_units::{ExactQty, UnitsError};

/// An exact rational.
pub type Rat = ExactQty;

/// At most this many cells in one component; more gives `Indeterminate`.
pub const MAX_VARIABLES: usize = 256;
/// At most this many simplex pivots per component; more gives `Indeterminate`.
pub const MAX_PIVOTS: usize = 10_000;

const ZERO: Rat = ExactQty::from_integer(0);
const ONE: Rat = ExactQty::from_integer(1);

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
        let lo = x.checked_cmp(self.lo)?;
        let hi = x.checked_cmp(self.hi)?;
        Ok(
            (lo == Ordering::Greater || (lo == Ordering::Equal && !self.lo_strict))
                && (hi == Ordering::Less || (hi == Ordering::Equal && !self.hi_strict)),
        )
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

/// Whether an identity is closed by published cells alone.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Closure {
    /// The identity holds over the printed cells by definition: a conflict is a discrepancy.
    Definitional,
    /// The issuer may use unpublished quantities: a conflict names a missing fact.
    MayUseUnpublished,
}

/// The S1 identity kinds; each is a homogeneous linear row.
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
    /// The row: `Σ signᵢ·lineᵢ − total`, `a − b` or `target − factor·source`, all `= 0`. Coefficients of a
    /// repeated locator are added; a row whose coefficients all cancel is refused as degenerate.
    pub fn row(&self) -> Result<Row, XfRefusal> {
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
                terms.push((total, Rat::from_integer(-1)));
            }
            IdentityKind::Equal { a, b } => {
                terms.push((a, ONE));
                terms.push((b, Rat::from_integer(-1)));
            }
            IdentityKind::Scale {
                target,
                source,
                factor,
            } => {
                terms.push((target, ONE));
                let neg = factor.checked_neg().map_err(|_| XfRefusal::Degenerate {
                    identity: self.id.clone(),
                })?;
                terms.push((source, neg));
            }
        }
        let mut coefficients: BTreeMap<String, Rat> = BTreeMap::new();
        for (loc, a) in terms {
            let entry = coefficients.entry(loc.to_string()).or_insert(ZERO);
            *entry = entry.checked_add(a).map_err(|_| XfRefusal::Degenerate {
                identity: self.id.clone(),
            })?;
        }
        coefficients.retain(|_, a| *a != ZERO);
        if coefficients.is_empty() {
            return Err(XfRefusal::Degenerate {
                identity: self.id.clone(),
            });
        }
        Ok(Row {
            coefficients,
            rhs: ZERO,
        })
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
    NonPositiveStep { locator: String },
    OffGrid { locator: String },
    DuplicateCell { locator: String },
    DuplicateIdentity { id: String },
    UnknownCell { identity: String, locator: String },
    BadSigns { identity: String },
    Degenerate { identity: String },
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
        }
    }
}

/// Farkas multipliers over identities, by id, none zero.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Certificate {
    pub multipliers: Vec<(String, Rat)>,
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
    convention: Convention,
) -> Result<Vec<Component>, XfRefusal> {
    solve_with_caps(cells, identities, convention, MAX_VARIABLES, MAX_PIVOTS)
}

/// [`solve`] with explicit caps.
pub fn solve_with_caps(
    cells: &[Cell],
    identities: &[Identity],
    convention: Convention,
    max_variables: usize,
    max_pivots: usize,
) -> Result<Vec<Component>, XfRefusal> {
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
    let mut rows = Vec::with_capacity(identities.len());
    for id in &identities {
        let row = id.row()?;
        for loc in row.coefficients.keys() {
            if !index.contains_key(loc.as_str()) {
                return Err(XfRefusal::UnknownCell {
                    identity: id.id.clone(),
                    locator: loc.clone(),
                });
            }
        }
        rows.push(row);
    }
    // Intervals: an off-grid half-even value or a non-positive step refuses; an overflow leaves the cell without
    // an interval, and its component is indeterminate.
    let mut intervals: Vec<Option<Interval>> = Vec::with_capacity(cells.len());
    for c in &cells {
        if c.step.signum() <= 0 {
            return Err(XfRefusal::NonPositiveStep {
                locator: c.locator.clone(),
            });
        }
        let half = c.step.checked_div(Rat::from_integer(2));
        match half
            .map_err(IntervalError::from)
            .and_then(|h| convention.interval(c.printed, h))
        {
            Ok(iv) => intervals.push(Some(iv)),
            Err(IntervalError::Overflow) => intervals.push(None),
            Err(IntervalError::OffGrid) => {
                return Err(XfRefusal::OffGrid {
                    locator: c.locator.clone(),
                });
            }
        }
    }

    // Components by union-find over cells linked by rows.
    let mut parent: Vec<usize> = (0..cells.len()).collect();
    fn find(parent: &mut [usize], mut i: usize) -> usize {
        while parent[i] != i {
            parent[i] = parent[parent[i]];
            i = parent[i];
        }
        i
    }
    for row in &rows {
        let mut locs = row.coefficients.keys().map(|l| index[l.as_str()]);
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
    for (r, row) in rows.iter().enumerate() {
        if let Some(loc) = row.coefficients.keys().next() {
            let root = find(&mut parent, index[loc.as_str()]);
            groups.entry(root).or_default().1.push(r);
        }
    }

    let mut out = Vec::with_capacity(groups.len());
    for (cell_ix, row_ix) in groups.into_values() {
        let comp_cells: Vec<&Cell> = cell_ix.iter().map(|&i| cells[i]).collect();
        let comp_ids: Vec<&Identity> = row_ix.iter().map(|&r| identities[r]).collect();
        let outcome = if cell_ix.len() > max_variables {
            Outcome::Indeterminate {
                reason: format!(
                    "the component has {} cells, over the cap of {max_variables}",
                    cell_ix.len()
                ),
            }
        } else if let Some(iv) = cell_ix
            .iter()
            .map(|&i| intervals[i])
            .collect::<Option<Vec<_>>>()
        {
            let comp_rows: Vec<&Row> = row_ix.iter().map(|&r| &rows[r]).collect();
            let solved = solve_component(&comp_cells, &iv, &comp_ids, &comp_rows, max_pivots);
            let owned_cells: Vec<Cell> = comp_cells.iter().map(|c| (*c).clone()).collect();
            let owned_ids: Vec<Identity> = comp_ids.iter().map(|i| (*i).clone()).collect();
            match solved {
                Ok(Outcome::Feasible { witness }) => {
                    match check_witness(&owned_cells, &owned_ids, convention, &witness) {
                        Ok(()) => Outcome::Feasible { witness },
                        Err(_) => rejected(),
                    }
                }
                Ok(Outcome::Infeasible { certificate }) => match check_certificate(
                    &owned_cells,
                    &owned_ids,
                    convention,
                    &certificate.multipliers,
                ) {
                    Ok(_) => Outcome::Infeasible { certificate },
                    Err(_) => rejected(),
                },
                Ok(other) => other,
                Err(LpError::Overflow) => Outcome::Indeterminate {
                    reason: "exact arithmetic overflowed".to_string(),
                },
                Err(LpError::PivotCap) => Outcome::Indeterminate {
                    reason: format!("over the cap of {max_pivots} pivots"),
                },
                Err(LpError::Unbounded) => Outcome::Indeterminate {
                    reason: "the simplex reported an unbounded phase".to_string(),
                },
            }
        } else {
            Outcome::Indeterminate {
                reason: "exact arithmetic overflowed".to_string(),
            }
        };
        out.push(Component {
            cells: comp_cells.iter().map(|c| c.locator.clone()).collect(),
            identities: comp_ids.iter().map(|i| i.id.clone()).collect(),
            outcome,
        });
    }
    Ok(out)
}

fn rejected() -> Outcome {
    Outcome::Indeterminate {
        reason: "checker rejected".to_string(),
    }
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
}

fn solve_component(
    cells: &[&Cell],
    intervals: &[Interval],
    identities: &[&Identity],
    rows: &[&Row],
    max_pivots: usize,
) -> Result<Outcome, LpError> {
    let n = cells.len();
    let pos: BTreeMap<&str, usize> = cells
        .iter()
        .enumerate()
        .map(|(j, c)| (c.locator.as_str(), j))
        .collect();
    let strict_lo: Vec<usize> = (0..n).filter(|&j| intervals[j].lo_strict).collect();
    let strict_hi: Vec<usize> = (0..n).filter(|&j| intervals[j].hi_strict).collect();
    let strict = !strict_lo.is_empty() || !strict_hi.is_empty();

    // Columns: y (n), s (n), then with open ends t, u (per open lower end), v (per open upper end), r.
    let t_col = 2 * n;
    let u0 = t_col + 1;
    let v0 = u0 + strict_lo.len();
    let r_col = v0 + strict_hi.len();
    let n_struct = if strict { r_col + 1 } else { 2 * n };

    let mut a_rows: Vec<Vec<Rat>> = Vec::new();
    let mut rhs: Vec<Rat> = Vec::new();
    for row in rows {
        let mut line = vec![ZERO; n_struct];
        let mut b = row.rhs;
        for (loc, coef) in &row.coefficients {
            let j = pos[loc.as_str()];
            line[j] = *coef;
            b = b.checked_sub(coef.checked_mul(intervals[j].lo)?)?;
        }
        a_rows.push(line);
        rhs.push(b);
    }
    for (j, iv) in intervals.iter().enumerate() {
        let mut line = vec![ZERO; n_struct];
        line[j] = ONE;
        line[n + j] = ONE;
        a_rows.push(line);
        rhs.push(iv.hi.checked_sub(iv.lo)?);
    }
    if strict {
        let minus = Rat::from_integer(-1);
        for (k, &j) in strict_lo.iter().enumerate() {
            let mut line = vec![ZERO; n_struct];
            line[j] = ONE;
            line[t_col] = minus;
            line[u0 + k] = minus;
            a_rows.push(line);
            rhs.push(ZERO);
        }
        for (k, &j) in strict_hi.iter().enumerate() {
            let mut line = vec![ZERO; n_struct];
            line[n + j] = ONE;
            line[t_col] = minus;
            line[v0 + k] = minus;
            a_rows.push(line);
            rhs.push(ZERO);
        }
        let mut line = vec![ZERO; n_struct];
        line[t_col] = ONE;
        line[r_col] = ONE;
        a_rows.push(line);
        rhs.push(ONE);
    }

    let mut tab = Tableau::new(a_rows, rhs, n_struct, max_pivots)?;
    let m = tab.a.len();
    let phase1: Vec<Rat> = (0..n_struct + m)
        .map(|j| if j < n_struct { ZERO } else { ONE })
        .collect();
    tab.optimize(&phase1, false)?;
    if tab.objective(&phase1)?.signum() > 0 {
        let p = tab.duals(&phase1)?;
        return Ok(Outcome::Infeasible {
            certificate: certificate(identities, &p[..rows.len()])?,
        });
    }
    tab.drive_out_artificials()?;
    if strict {
        let mut phase2 = vec![ZERO; n_struct + m];
        phase2[t_col] = Rat::from_integer(-1);
        tab.optimize(&phase2, true)?;
        if tab.value(t_col).signum() <= 0 {
            let p = tab.duals(&phase2)?;
            return Ok(Outcome::Infeasible {
                certificate: certificate(identities, &p[..rows.len()])?,
            });
        }
    }
    let mut witness = BTreeMap::new();
    for (j, c) in cells.iter().enumerate() {
        witness.insert(
            c.locator.clone(),
            intervals[j].lo.checked_add(tab.value(j))?,
        );
    }
    Ok(Outcome::Feasible { witness })
}

/// Multipliers by identity id, scaled by a positive factor so the first non-zero one is ±1.
fn certificate(identities: &[&Identity], p: &[Rat]) -> Result<Certificate, LpError> {
    let scale = p.iter().find(|x| **x != ZERO).map(|x| {
        if x.signum() < 0 {
            x.checked_neg()
        } else {
            Ok(*x)
        }
    });
    let scale = match scale {
        Some(s) => s?,
        None => ONE,
    };
    let mut multipliers = Vec::new();
    for (id, x) in identities.iter().zip(p) {
        if *x != ZERO {
            multipliers.push((id.id.clone(), x.checked_div(scale)?));
        }
    }
    Ok(Certificate { multipliers })
}

/// Why a checker rejected an outcome.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckFailure(pub String);

fn fail<T>(reason: impl Into<String>) -> Result<T, CheckFailure> {
    Err(CheckFailure(reason.into()))
}

fn checked<T>(r: Result<T, UnitsError>) -> Result<T, CheckFailure> {
    r.map_err(|_| CheckFailure("exact arithmetic overflowed".to_string()))
}

fn cell_intervals(
    cells: &[Cell],
    convention: Convention,
) -> Result<BTreeMap<&str, Interval>, CheckFailure> {
    let mut out = BTreeMap::new();
    for c in cells {
        if c.step.signum() <= 0 {
            return fail(format!("{}: the step must be positive", c.locator));
        }
        let h = checked(c.step.checked_div(Rat::from_integer(2)))?;
        let iv = convention
            .interval(c.printed, h)
            .map_err(|e| CheckFailure(format!("{}: no interval ({e:?})", c.locator)))?;
        if out.insert(c.locator.as_str(), iv).is_some() {
            return fail(format!("{} is given twice", c.locator));
        }
    }
    Ok(out)
}

/// Checks a witness: every cell has a value inside its interval, and every identity holds exactly.
pub fn check_witness(
    cells: &[Cell],
    identities: &[Identity],
    convention: Convention,
    witness: &BTreeMap<String, Rat>,
) -> Result<(), CheckFailure> {
    let intervals = cell_intervals(cells, convention)?;
    for (loc, iv) in &intervals {
        let Some(x) = witness.get(*loc) else {
            return fail(format!("{loc} has no witness value"));
        };
        if !checked(iv.contains(*x))? {
            return fail(format!("{loc}: the witness value is outside its interval"));
        }
    }
    for id in identities {
        let row = id.row().map_err(|e| CheckFailure(e.to_string()))?;
        let mut sum = ZERO;
        for (loc, a) in &row.coefficients {
            let Some(x) = witness.get(loc) else {
                return fail(format!(
                    "identity {} names {loc}, which has no witness value",
                    id.id
                ));
            };
            sum = checked(sum.checked_add(checked(a.checked_mul(*x))?))?;
        }
        if sum != row.rhs {
            return fail(format!("identity {} does not hold at the witness", id.id));
        }
    }
    Ok(())
}

/// The combination a valid certificate certifies: `cᵀx` ranges over `range` on the box, and `rhs = b*` lies
/// outside it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CertificateRange {
    pub combination: BTreeMap<String, Rat>,
    pub rhs: Rat,
    pub range: Interval,
}

/// Checks a certificate: computes `c = Σ λᵢ·aᵢ`, `b* = Σ λᵢ·bᵢ` and the exact range of `cᵀx` over the cells'
/// intervals, open ends included, and accepts only when `b*` lies outside it.
pub fn check_certificate(
    cells: &[Cell],
    identities: &[Identity],
    convention: Convention,
    multipliers: &[(String, Rat)],
) -> Result<CertificateRange, CheckFailure> {
    let intervals = cell_intervals(cells, convention)?;
    let by_id: BTreeMap<&str, &Identity> = identities.iter().map(|i| (i.id.as_str(), i)).collect();
    let mut combination: BTreeMap<String, Rat> = BTreeMap::new();
    let mut rhs = ZERO;
    for (id, lambda) in multipliers {
        let Some(identity) = by_id.get(id.as_str()) else {
            return fail(format!("multiplier for unknown identity {id}"));
        };
        let row = identity.row().map_err(|e| CheckFailure(e.to_string()))?;
        for (loc, a) in &row.coefficients {
            let e = combination.entry(loc.clone()).or_insert(ZERO);
            *e = checked(e.checked_add(checked(lambda.checked_mul(*a))?))?;
        }
        rhs = checked(rhs.checked_add(checked(lambda.checked_mul(row.rhs))?))?;
    }
    combination.retain(|_, c| *c != ZERO);
    let mut range = Interval {
        lo: ZERO,
        hi: ZERO,
        lo_strict: false,
        hi_strict: false,
    };
    for (loc, c) in &combination {
        let Some(iv) = intervals.get(loc.as_str()) else {
            return fail(format!("{loc} is not a given cell"));
        };
        let (lo, hi, lo_strict, hi_strict) = if c.signum() > 0 {
            (iv.lo, iv.hi, iv.lo_strict, iv.hi_strict)
        } else {
            (iv.hi, iv.lo, iv.hi_strict, iv.lo_strict)
        };
        range.lo = checked(range.lo.checked_add(checked(c.checked_mul(lo))?))?;
        range.hi = checked(range.hi.checked_add(checked(c.checked_mul(hi))?))?;
        range.lo_strict |= lo_strict;
        range.hi_strict |= hi_strict;
    }
    let below = checked(rhs.checked_cmp(range.lo))?;
    let above = checked(rhs.checked_cmp(range.hi))?;
    let outside = below == Ordering::Less
        || (below == Ordering::Equal && range.lo_strict)
        || above == Ordering::Greater
        || (above == Ordering::Equal && range.hi_strict);
    if !outside {
        return fail("b* lies inside the range of the combination");
    }
    Ok(CertificateRange {
        combination,
        rhs,
        range,
    })
}

fn put_rat(enc: &mut Encoder, q: Rat) {
    enc.put_i128(q.num());
    enc.put_i128(q.den());
}

/// The certificate's canonical digest (hex BLAKE3 of an `XFCT` v1 frame): the convention; the cells sorted by
/// locator (locator, sha256, token, printed, step, scope); the identities sorted by id (id, family, closure,
/// row coefficients by locator, rhs, multiplier, zero when absent).
pub fn certificate_digest(
    cells: &[Cell],
    identities: &[Identity],
    convention: Convention,
    certificate: &Certificate,
) -> Result<String, XfRefusal> {
    let mut cells: Vec<&Cell> = cells.iter().collect();
    cells.sort_by(|a, b| a.locator.cmp(&b.locator));
    let mut identities: Vec<&Identity> = identities.iter().collect();
    identities.sort_by(|a, b| a.id.cmp(&b.id));
    let lambda: BTreeMap<&str, Rat> = certificate
        .multipliers
        .iter()
        .map(|(id, q)| (id.as_str(), *q))
        .collect();
    let mut enc = Encoder::new_frame(*b"XFCT", 1);
    enc.put_u8(convention.tag());
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
        let row = id.row()?;
        enc.put_str(&id.id);
        enc.put_str(&id.family);
        enc.put_u8(id.closure_tag());
        enc.put_u64(row.coefficients.len() as u64);
        for (loc, a) in &row.coefficients {
            enc.put_str(loc);
            put_rat(&mut enc, *a);
        }
        put_rat(&mut enc, row.rhs);
        put_rat(
            &mut enc,
            lambda.get(id.id.as_str()).copied().unwrap_or(ZERO),
        );
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

    fn only(components: Vec<Component>) -> Outcome {
        assert_eq!(components.len(), 1, "{components:?}");
        components.into_iter().next().expect("one").outcome
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
        let Outcome::Infeasible { certificate } =
            only(solve(&cells, &ids, Convention::Closed).expect("solves"))
        else {
            panic!("E114 must be infeasible");
        };
        assert_eq!(certificate.multipliers, vec![("e114".to_string(), int(1))]);
        let checked = check_certificate(&cells, &ids, Convention::Closed, &certificate.multipliers)
            .expect("the checker accepts");
        // Q1 + Q2 − H1 over [147.5, 148.5] + [129.5, 130.5] − [280.5, 281.5] = [−4.5, −1.5], excluding 0.
        assert_eq!(
            checked.range,
            Interval {
                lo: q(-9, 2),
                hi: q(-3, 2),
                lo_strict: false,
                hi_strict: false
            }
        );
        assert_eq!(checked.rhs, int(0));
    }

    /// Regression pin, not an oracle: the digest of this implementation's E114 certificate frame.
    #[test]
    fn test_certificate_digest_golden_e114() {
        let (cells, ids) = e114();
        let certificate = Certificate {
            multipliers: vec![("e114".to_string(), int(1))],
        };
        let digest =
            certificate_digest(&cells, &ids, Convention::Closed, &certificate).expect("digest");
        assert_eq!(
            digest,
            "7258ed7a5b0eef349bc86f0d98053b69158d10761dacd5b147eeb644439eb3c3"
        );
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
        for (id, lines) in [(&a, [1, 2]), (&b, [3, 4])] {
            let part = vec![
                cells[0].clone(),
                cells[lines[0]].clone(),
                cells[lines[1]].clone(),
            ];
            let ids = vec![id.clone()];
            let Outcome::Feasible { witness } =
                only(solve(&part, &ids, Convention::Closed).expect("solves"))
            else {
                panic!("{} alone must be feasible", id.id);
            };
            assert_eq!(
                check_witness(&part, &ids, Convention::Closed, &witness),
                Ok(())
            );
        }
        let ids = vec![a, b];
        let Outcome::Infeasible { certificate } =
            only(solve(&cells, &ids, Convention::Closed).expect("solves"))
        else {
            panic!("the two breakdowns together must conflict");
        };
        assert!(
            check_certificate(&cells, &ids, Convention::Closed, &certificate.multipliers).is_ok()
        );
        // The hand certificate λ = (−1, +1): −a1 − a2 + b1 + b2 over the boxes is [−0.82, −0.78].
        let hand = vec![("A".to_string(), int(-1)), ("B".to_string(), int(1))];
        let checked = check_certificate(&cells, &ids, Convention::Closed, &hand).expect("valid");
        assert_eq!(
            (checked.range.lo, checked.range.hi),
            (q(-41, 50), q(-39, 50))
        );
        assert!(!checked.combination.contains_key("T"));
    }

    #[test]
    fn test_tie_point_feasible_only_under_closed() {
        let cells = vec![cell("a", int(1), int(1)), cell("b", int(2), int(1))];
        let ids = vec![equal("tie", "a", "b")];
        let Outcome::Feasible { witness } =
            only(solve(&cells, &ids, Convention::Closed).expect("solves"))
        else {
            panic!("closed must be feasible");
        };
        assert_eq!(witness.get("a"), Some(&q(3, 2)));
        assert_eq!(witness.get("b"), Some(&q(3, 2)));
        for convention in [Convention::HalfUp, Convention::HalfDown] {
            let Outcome::Infeasible { certificate } =
                only(solve(&cells, &ids, convention).expect("solves"))
            else {
                panic!("{convention:?} must be infeasible");
            };
            let checked = check_certificate(&cells, &ids, convention, &certificate.multipliers)
                .expect("the checker accepts");
            // The conflict sits exactly on an open end of the range.
            assert!(
                checked.range.lo_strict || checked.range.hi_strict,
                "{checked:?}"
            );
        }
    }

    #[test]
    fn test_witness_checker_rejects_tampered_witness() {
        let cells = vec![cell("a", int(1), int(1)), cell("b", int(2), int(1))];
        let ids = vec![equal("tie", "a", "b")];
        let w = |a, b| BTreeMap::from([("a".to_string(), a), ("b".to_string(), b)]);
        assert_eq!(
            check_witness(&cells, &ids, Convention::Closed, &w(q(3, 2), q(3, 2))),
            Ok(())
        );
        // Outside a's interval.
        assert!(check_witness(&cells, &ids, Convention::Closed, &w(q(8, 5), q(8, 5))).is_err());
        // Inside both intervals, but a ≠ b.
        assert!(check_witness(&cells, &ids, Convention::Closed, &w(q(7, 5), q(3, 2))).is_err());
        // 1.5 is on a's open upper end under half-up.
        assert!(check_witness(&cells, &ids, Convention::HalfUp, &w(q(3, 2), q(3, 2))).is_err());
        // A missing value.
        let partial = BTreeMap::from([("a".to_string(), q(3, 2))]);
        assert!(check_witness(&cells, &ids, Convention::Closed, &partial).is_err());
    }

    #[test]
    fn test_certificate_checker_rejects_wrong_sign_multiplier() {
        let (cells, ids) = e114();
        // −(Q1 + Q2 − H1) ranges over [1.5, 4.5]: 0 is outside, but on the other side, so −1 is also valid…
        let neg = vec![("e114".to_string(), int(-1))];
        let checked = check_certificate(&cells, &ids, Convention::Closed, &neg).expect("valid");
        assert_eq!((checked.range.lo, checked.range.hi), (q(3, 2), q(9, 2)));
        // …while a feasible pair has no valid certificate of either sign.
        let tie = vec![cell("a", int(1), int(1)), cell("b", int(2), int(1))];
        let tie_ids = vec![equal("tie", "a", "b")];
        for lambda in [int(1), int(-1)] {
            let m = vec![("tie".to_string(), lambda)];
            assert!(check_certificate(&tie, &tie_ids, Convention::Closed, &m).is_err());
        }
        // No multipliers: the range is {0}, which holds b* = 0.
        assert!(check_certificate(&cells, &ids, Convention::Closed, &[]).is_err());
        // An unknown identity.
        let unknown = vec![("nope".to_string(), int(1))];
        assert!(check_certificate(&cells, &ids, Convention::Closed, &unknown).is_err());
        // COD-02 with the same sign on both breakdowns: 2T is not cancelled and 0 falls inside.
        let (cod_cells, a, b) = cod02();
        let same = vec![("A".to_string(), int(-1)), ("B".to_string(), int(-1))];
        assert!(check_certificate(&cod_cells, &[a, b], Convention::Closed, &same).is_err());
    }

    #[test]
    fn test_certificate_digest_invariant_under_row_order() {
        let (cells, a, b) = cod02();
        let forward = vec![a.clone(), b.clone()];
        let backward = vec![b, a];
        let mut shuffled = cells.clone();
        shuffled.reverse();
        let run = |cells: &[Cell], ids: &[Identity]| {
            let Outcome::Infeasible { certificate } =
                only(solve(cells, ids, Convention::Closed).expect("solves"))
            else {
                panic!("infeasible");
            };
            let digest =
                certificate_digest(cells, ids, Convention::Closed, &certificate).expect("digest");
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
        let Outcome::Indeterminate { reason } =
            only(solve(&cells, &ids, Convention::Closed).expect("solves"))
        else {
            panic!("257 cells must be indeterminate");
        };
        assert!(reason.contains("257 cells"), "{reason}");
        let (cells, ids) = e114();
        let Outcome::Indeterminate { reason } = only(
            solve_with_caps(&cells, &ids, Convention::Closed, MAX_VARIABLES, 1).expect("solves"),
        ) else {
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
            only(solve(&cells, &ids, Convention::Closed).expect("solves"))
        else {
            panic!("an overflow must be indeterminate");
        };
        assert!(reason.contains("overflow"), "{reason}");
    }

    #[test]
    fn test_refusals_name_the_input() {
        let (mut cells, ids) = e114();
        cells[0].step = int(0);
        assert!(matches!(
            solve(&cells, &ids, Convention::Closed),
            Err(XfRefusal::NonPositiveStep { .. })
        ));
        let (cells, mut ids) = e114();
        ids.push(sum("e114", "TR:H1 2026", &["TR:Q1 2026"]));
        assert!(matches!(
            solve(&cells, &ids, Convention::Closed),
            Err(XfRefusal::DuplicateIdentity { .. })
        ));
        let (cells, _) = e114();
        let ids = vec![equal("x", "TR:Q1 2026", "nowhere")];
        assert!(matches!(
            solve(&cells, &ids, Convention::Closed),
            Err(XfRefusal::UnknownCell { .. })
        ));
        let ids = vec![equal("self", "TR:Q1 2026", "TR:Q1 2026")];
        assert!(matches!(
            solve(&cells, &ids, Convention::Closed),
            Err(XfRefusal::Degenerate { .. })
        ));
        let half = vec![cell("h", q(5, 2), int(1))];
        assert!(matches!(
            solve(&half, &[], Convention::HalfEven),
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
            for c in solve(&cells, &ids, convention).expect("solves") {
                match c.outcome {
                    Outcome::Feasible { .. } => feasible += 1,
                    Outcome::Indeterminate { .. } => indeterminate += 1,
                    other => panic!("table {t}: a clean table gave {other:?}"),
                }
            }
        }
        println!(
            "{{\"test\":\"prop_clean_generated_tables\",\"tables\":{tables},\"components_feasible\":{feasible},\
             \"components_indeterminate\":{indeterminate}}}"
        );
        assert!(feasible > 0);
    }
}
