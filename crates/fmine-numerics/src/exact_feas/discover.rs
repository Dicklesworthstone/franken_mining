//! Discovery reports (bd-4wvl.4, BD-340.S4). A report, never a verdict: it changes no state.
//!
//! **Part 2, tie convention** ([`discover_tie_conventions`]): one convention per document, never per cell.
//! Every assignment of a strict convention to each document is tried; a quantity's true value lies in the meet
//! of its documents' intervals. An assignment fits properly, only at tie points, or not at all. A document is
//! identified when every proper fit gives it the same convention. The tie-agnostic `Closed` check is reported
//! apart.
//!
//! **Part 4, deck** ([`discover_deck`]): which registered price deck converted one row's gold and copper to GEO.
//! Every strict convention, one for the whole row, is tried against every deck; a deck is found when every proper
//! fit is on it. A found deck is a finding, never the row's registered deck.
//!
//! **Part 1, construction**: which of `ComponentSum` and `RoundedOnce` the printed footings of one family fit,
//! read from finished footings.
//!
//! - **Statistic.** The number of totals that foot exactly: the printed total equals the sum of its printed
//!   summands. `ComponentSum` accepts exactly those. `RoundedOnce` accepts a footing whose residual is within
//!   the half-steps of its boxes, `2·|residual| ≤ k + 1` in units of the common step.
//! - **Null.** Under `RoundedOnce` the printed total is the true total rounded once, and the true total is the
//!   sum of the true summands. With `N` the sum of the k printed summands and eᵢ their rounding errors, the
//!   total prints `round(N − Σeᵢ)`, which is `N` exactly when `|Σeᵢ| < ½`. For eᵢ i.i.d. uniform on (−½, ½)
//!   that is p₀(k), computed exactly from the Irwin–Hall distribution ([`p0`]): p₀(1) = 1, p₀(2) = 3/4,
//!   p₀(3) = 2/3, p₀(4) = 115/192. A placeholder summand is an exact 0 and is not among the k.
//! - **Report.** The expected count of exact footings under `RoundedOnce`, Σ p₀(kᵢ), exactly; the upper and lower
//!   tails of the observed count under that null (independent footings, a Poisson-binomial law), as natural
//!   logarithms computed in `f64` through `fmine_core::det`.
//! - **Decision**, declared before any real-file run, with α = 10⁻⁶ ([`ALPHA`]) and ε = 1/10
//!   ([`MAX_ERROR_RATE`]). A construction may reject some footings, as issuer errors, at a rate up to ε. With n
//!   footings it rejects m, it is ruled out when P(Binomial(n, ε) ≥ m) < α. `RoundedOnce` is also ruled out
//!   when either tail of the exact count is below α. Then:
//!   - only `ComponentSum` fits: [`Discovered::ComponentSum`];
//!   - only `RoundedOnce` fits: [`Discovered::RoundedOnce`];
//!   - both or neither: [`Discovered::Ambiguous`], carrying [`NON_IDENTIFIABLE`] (FMINE-REF-104, exit 3). One
//!     total that misses by a unit is as much one issuer error as one rounding, so it identifies nothing.
//!
//! A footing with more than [`MAX_K`] summands is indeterminate (its p₀ is not computed) and is counted apart.
//! The tails take the footings as independent draws, so the caller passes each distinct footing once: a table
//! reprinted as a later release's comparative is the same draw, not a second one.
//!
//! Determinism: DeterministicPerIsa (p₀ and the expected count are exact rationals; the tails are `f64` through
//! `fmine_core::det`, in a fixed order).

use std::cmp::Ordering;

use fmine_core::det;
use fmine_units::UnitsError;

use super::{Bound, Convention, End, Interval, IntervalError, Rat};

/// Footings with more summands are indeterminate.
pub const MAX_K: u32 = 20;

/// The refusal an ambiguous discovery carries: no single construction is identified.
pub const NON_IDENTIFIABLE: &str = "FMINE-REF-104";

/// The tail probability below which a construction is ruled out.
pub const ALPHA: f64 = 1e-6;

/// The highest rate of footings a construction may reject as issuer errors and still fit.
pub const MAX_ERROR_RATE: f64 = 0.1;

/// p₀(k) = P(|e₁ + … + e_k| < ½) for eᵢ i.i.d. uniform on (−½, ½), exactly; `None` past [`MAX_K`].
///
/// With S + k/2 Irwin–Hall on [0, k], p₀(k) = F((k + 1)/2) − F((k − 1)/2), where
/// k!·F(x) = Σ_{0 ≤ j ≤ x} (−1)ʲ C(k, j)(x − j)ᵏ. Multiplying by 2ᵏ keeps every term an integer.
#[must_use]
pub fn p0(k: u32) -> Option<Rat> {
    if k > MAX_K {
        return None;
    }
    if k == 0 {
        return Some(Rat::from_integer(1));
    }
    let kk = i128::from(k);
    // Σ_{0 ≤ j, 2j ≤ c} (−1)ʲ C(k, j)(c − 2j)ᵏ, for c = k + 1 and c = k − 1.
    let sum = |c: i128| -> Option<i128> {
        let mut total: i128 = 0;
        let mut binom: i128 = 1;
        let mut j: i128 = 0;
        while 2 * j <= c {
            let term = binom.checked_mul((c - 2 * j).checked_pow(k)?)?;
            total = if j % 2 == 0 {
                total.checked_add(term)?
            } else {
                total.checked_sub(term)?
            };
            binom = binom.checked_mul(kk - j)? / (j + 1);
            j += 1;
        }
        Some(total)
    };
    let num = sum(kk + 1)?.checked_sub(sum(kk - 1)?)?;
    let factorial = (1..=kk).try_fold(1_i128, i128::checked_mul)?;
    let den = factorial.checked_mul(1_i128.checked_shl(k)?)?;
    Rat::new(num, den).ok()
}

/// One decided footing: `k` numeric summands and `residual` = Σ printed summands − printed total, in units of
/// the step every operand shares.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Footing {
    pub k: u32,
    pub residual: i128,
}

/// What the footings of one family fit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Discovered {
    ComponentSum,
    RoundedOnce,
    /// No single construction is identified (FMINE-REF-104).
    Ambiguous,
}

impl Discovered {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ComponentSum => "component_sum",
            Self::RoundedOnce => "rounded_once",
            Self::Ambiguous => "ambiguous",
        }
    }

    /// [`NON_IDENTIFIABLE`] for an ambiguous discovery, else `None`.
    #[must_use]
    pub const fn reason_code(self) -> Option<&'static str> {
        match self {
            Self::Ambiguous => Some(NON_IDENTIFIABLE),
            Self::ComponentSum | Self::RoundedOnce => None,
        }
    }
}

/// The construction report of one family.
#[derive(Debug, Clone, PartialEq)]
pub struct ConstructionReport {
    /// Footings scored (at most [`MAX_K`] summands).
    pub n: usize,
    /// Scored footings whose residual is 0: what `ComponentSum` accepts.
    pub foot_exact: usize,
    /// Scored footings with `2·|residual| ≤ k + 1`: what `RoundedOnce` accepts.
    pub rounded_once_accepts: usize,
    /// Footings with more than [`MAX_K`] summands: indeterminate, not scored.
    pub indeterminate: usize,
    /// Σ p₀(kᵢ) over the scored footings: the expected exact count under `RoundedOnce`.
    pub expected_rounded_once: Rat,
    /// ln P(X ≥ `foot_exact`) under `RoundedOnce`.
    pub ln_upper_tail: f64,
    /// ln P(X ≤ `foot_exact`) under `RoundedOnce`.
    pub ln_lower_tail: f64,
    /// ln P(Binomial(n, ε) ≥ n − `foot_exact`): the footings `ComponentSum` rejects, as issuer errors.
    pub ln_component_sum_error_tail: f64,
    /// ln P(Binomial(n, ε) ≥ n − `rounded_once_accepts`): the footings `RoundedOnce` rejects.
    pub ln_rounded_once_error_tail: f64,
    pub discovered: Discovered,
}

/// ln(eᵃ + eᵇ), with −∞ as the log of 0.
fn ln_add(a: f64, b: f64) -> f64 {
    let (hi, lo) = if a >= b { (a, b) } else { (b, a) };
    if lo == f64::NEG_INFINITY {
        return hi;
    }
    hi + det::ln(1.0 + det::exp(lo - hi))
}

/// ln P(X = j) for j = 0..=n, X the number of successes of independent trials with the given probabilities.
fn ln_poisson_binomial(ps: &[Rat]) -> Vec<f64> {
    let mut ln_pmf = vec![0.0_f64];
    for p in ps {
        let ln_p = det::ln(p.to_f64());
        let ln_q = det::ln(1.0 - p.to_f64());
        let mut next = vec![f64::NEG_INFINITY; ln_pmf.len() + 1];
        for (j, &l) in ln_pmf.iter().enumerate() {
            next[j] = ln_add(next[j], l + ln_q);
            next[j + 1] = ln_add(next[j + 1], l + ln_p);
        }
        ln_pmf = next;
    }
    ln_pmf
}

/// ln P(Binomial(n, [`MAX_ERROR_RATE`]) ≥ m); 0 for m = 0, −∞ for m > n.
fn ln_error_tail(n: usize, m: usize) -> f64 {
    let (ln_e, ln_f) = (det::ln(MAX_ERROR_RATE), det::ln(1.0 - MAX_ERROR_RATE));
    // ln j! for j = 0..=n.
    let mut ln_fact = vec![0.0_f64; n + 1];
    for j in 1..=n {
        ln_fact[j] = ln_fact[j - 1] + det::ln(j as f64);
    }
    (m..=n).fold(f64::NEG_INFINITY, |acc, j| {
        let ln_choose = ln_fact[n] - ln_fact[j] - ln_fact[n - j];
        ln_add(acc, ln_choose + j as f64 * ln_e + (n - j) as f64 * ln_f)
    })
}

/// The construction report for one family's decided footings. An overflow in the expected count (it cannot
/// occur below [`MAX_K`] summands per footing and 10⁶ footings, but is checked) is an error.
pub fn discover_construction(footings: &[Footing]) -> Result<ConstructionReport, UnitsError> {
    let mut ps = Vec::new();
    let (mut foot_exact, mut rounded_once_accepts, mut indeterminate) = (0, 0, 0);
    let mut expected = Rat::from_integer(0);
    for f in footings {
        let Some(p) = p0(f.k) else {
            indeterminate += 1;
            continue;
        };
        expected = expected.checked_add(p)?;
        ps.push(p);
        if f.residual == 0 {
            foot_exact += 1;
        }
        let twice = f.residual.checked_abs().and_then(|a| a.checked_mul(2));
        if twice.is_some_and(|t| t <= i128::from(f.k) + 1) {
            rounded_once_accepts += 1;
        }
    }
    let ln_pmf = ln_poisson_binomial(&ps);
    let fold = |range: &[f64]| {
        range
            .iter()
            .fold(f64::NEG_INFINITY, |acc, &l| ln_add(acc, l))
    };
    let ln_upper_tail = fold(&ln_pmf[foot_exact..]);
    let ln_lower_tail = fold(&ln_pmf[..=foot_exact]);
    let n = ps.len();
    let ln_component_sum_error_tail = ln_error_tail(n, n - foot_exact);
    let ln_rounded_once_error_tail = ln_error_tail(n, n - rounded_once_accepts);
    let ln_alpha = det::ln(ALPHA);
    let component_sum_fits = ln_component_sum_error_tail >= ln_alpha;
    let rounded_once_fits = ln_rounded_once_error_tail >= ln_alpha
        && ln_upper_tail >= ln_alpha
        && ln_lower_tail >= ln_alpha;
    let discovered = match (n > 0, component_sum_fits, rounded_once_fits) {
        (true, true, false) => Discovered::ComponentSum,
        (true, false, true) => Discovered::RoundedOnce,
        _ => Discovered::Ambiguous,
    };
    Ok(ConstructionReport {
        n,
        foot_exact,
        rounded_once_accepts,
        indeterminate,
        expected_rounded_once: expected,
        ln_upper_tail,
        ln_lower_tail,
        ln_component_sum_error_tail,
        ln_rounded_once_error_tail,
        discovered,
    })
}

/// The conventions a tie discovery assigns, one per document: the four that decide a tie.
pub const STRICT_CONVENTIONS: [Convention; 4] = [
    Convention::HalfUp,
    Convention::HalfDown,
    Convention::HalfEven,
    Convention::Truncate,
];

/// At most this many documents in one tie discovery (4⁴ assignments).
pub const MAX_DOCUMENTS: usize = 4;

/// One quantity as one document prints it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Printed {
    pub document: String,
    pub quantity: String,
    pub printed: Rat,
    /// The rounding step: positive, since an exact value has no convention to find.
    pub step: Rat,
}

/// How one assignment fits every quantity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fit {
    /// Some quantity has no true value both documents round to their prints.
    Empty,
    /// Every quantity has one, but some only at a single tie point.
    Points,
    /// Every quantity has a range of true values of positive width.
    Proper,
}

/// One convention per document, and the true range it leaves each quantity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Assignment {
    /// By document, in name order.
    pub conventions: Vec<(String, Convention)>,
    /// By quantity, in name order: the meet of every document's interval.
    pub ranges: Vec<(String, Bound)>,
    pub fit: Fit,
}

/// What a tie discovery finds for one document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TieFinding {
    /// Every proper fit gives the document this convention.
    Identified(Convention),
    /// Proper fits give it several (FMINE-REF-104).
    Ambiguous(Vec<Convention>),
    /// No assignment fits properly.
    NoProperFit,
}

impl TieFinding {
    /// [`NON_IDENTIFIABLE`] for an ambiguous finding, else `None`.
    #[must_use]
    pub const fn reason_code(&self) -> Option<&'static str> {
        match self {
            Self::Ambiguous(_) => Some(NON_IDENTIFIABLE),
            Self::Identified(_) | Self::NoProperFit => None,
        }
    }
}

/// A tie discovery: every assignment of one strict convention per document, the tie-agnostic check with every
/// document `Closed`, and each document's finding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TieDiscovery {
    pub assignments: Vec<Assignment>,
    pub closed: Assignment,
    pub findings: Vec<(String, TieFinding)>,
    /// Quantities with no true value under `Closed` nor under any assignment, in name order. `Closed` alone does
    /// not decide: it holds every round-to-nearest interval, but a truncation interval [v, v + 2h) can reach past
    /// it when the documents' steps differ. No convention reconciles these prints: one of them is not a rounding
    /// of the same value (a misprint, or a different quantity under one key). They make every assignment `Empty`.
    pub conflicts: Vec<String>,
}

/// Why a tie discovery was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TieRefusal {
    /// A step of zero or below, at `document`/`quantity`.
    NonPositiveStep(String, String),
    /// One document prints one quantity twice.
    Duplicate(String, String),
    /// More than [`MAX_DOCUMENTS`] documents, or fewer than two.
    DocumentCount(usize),
    /// A printed value off its own step's grid (half-even needs its parity), or an overflow.
    Interval(String, String, IntervalError),
}

/// A single point: both ends equal and closed.
fn is_point(b: &Bound) -> Result<bool, UnitsError> {
    Ok(match (b.lo, b.hi) {
        (Some(lo), Some(hi)) => {
            !lo.strict && !hi.strict && lo.at.checked_cmp(hi.at)? == Ordering::Equal
        }
        _ => false,
    })
}

/// Which tie convention each document prints under. Conventions are assigned per document, never per cell: an
/// assignment gives every quantity of a document the same one. Each quantity's true value lies in the meet of
/// its documents' intervals. A quantity printed by one document only constrains nothing and is still reported.
pub fn discover_tie_conventions(printed: &[Printed]) -> Result<TieDiscovery, TieRefusal> {
    let mut documents: Vec<String> = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    for p in printed {
        if p.step.signum() <= 0 {
            return Err(TieRefusal::NonPositiveStep(
                p.document.clone(),
                p.quantity.clone(),
            ));
        }
        if !seen.insert((p.document.clone(), p.quantity.clone())) {
            return Err(TieRefusal::Duplicate(
                p.document.clone(),
                p.quantity.clone(),
            ));
        }
        documents.push(p.document.clone());
    }
    documents.sort();
    documents.dedup();
    if !(2..=MAX_DOCUMENTS).contains(&documents.len()) {
        return Err(TieRefusal::DocumentCount(documents.len()));
    }
    let mut quantities: Vec<String> = printed.iter().map(|p| p.quantity.clone()).collect();
    quantities.sort();
    quantities.dedup();
    let assess = |pick: &dyn Fn(usize) -> Convention| -> Result<Assignment, TieRefusal> {
        let mut ranges = Vec::new();
        let mut fit = Fit::Proper;
        for q in &quantities {
            let mut range = Bound::FREE;
            for p in printed.iter().filter(|p| &p.quantity == q) {
                let d = documents.iter().position(|d| d == &p.document).unwrap_or(0);
                let refuse = |e| TieRefusal::Interval(p.document.clone(), q.clone(), e);
                let h = p
                    .step
                    .checked_div(Rat::from_integer(2))
                    .map_err(|e| refuse(e.into()))?;
                let iv = pick(d).interval(p.printed, h).map_err(refuse)?;
                range = range.meet(Bound::of(iv)).map_err(|e| refuse(e.into()))?;
            }
            let overflow = |e: UnitsError| TieRefusal::Interval(String::new(), q.clone(), e.into());
            if range.is_empty().map_err(overflow)? {
                fit = Fit::Empty;
            } else if fit == Fit::Proper && is_point(&range).map_err(overflow)? {
                fit = Fit::Points;
            }
            ranges.push((q.clone(), range));
        }
        let conventions = documents
            .iter()
            .enumerate()
            .map(|(d, name)| (name.clone(), pick(d)))
            .collect();
        Ok(Assignment {
            conventions,
            ranges,
            fit,
        })
    };
    let n = documents.len();
    let total = STRICT_CONVENTIONS.len().pow(n as u32);
    let mut assignments = Vec::with_capacity(total);
    for index in 0..total {
        // Document d takes digit d of `index` in base 4, the first document most significant.
        let pick = |d: usize| {
            let digit = (index / STRICT_CONVENTIONS.len().pow((n - 1 - d) as u32))
                % STRICT_CONVENTIONS.len();
            STRICT_CONVENTIONS[digit]
        };
        assignments.push(assess(&pick)?);
    }
    let closed = assess(&|_| Convention::Closed)?;
    let findings = documents
        .iter()
        .enumerate()
        .map(|(d, name)| {
            let mut found: Vec<Convention> = assignments
                .iter()
                .filter(|a| a.fit == Fit::Proper)
                .map(|a| a.conventions[d].1)
                .collect();
            found.sort();
            found.dedup();
            let finding = match found.as_slice() {
                [] => TieFinding::NoProperFit,
                [one] => TieFinding::Identified(*one),
                _ => TieFinding::Ambiguous(found),
            };
            (name.clone(), finding)
        })
        .collect();
    // `Closed` holds every round-to-nearest interval but not a truncation one, [v, v + 2h), which can reach past
    // it when the documents' steps differ; so a quantity is a conflict only when every assignment leaves it empty.
    let mut conflicts = Vec::new();
    for (i, (q, range)) in closed.ranges.iter().enumerate() {
        let overflow = |e: UnitsError| TieRefusal::Interval(String::new(), q.clone(), e.into());
        let mut empty = range.is_empty().map_err(overflow)?;
        for a in &assignments {
            if !empty {
                break;
            }
            if let Some((_, r)) = a.ranges.get(i) {
                empty = r.is_empty().map_err(overflow)?;
            }
        }
        if empty {
            conflicts.push(q.clone());
        }
    }
    Ok(TieDiscovery {
        assignments,
        closed,
        findings,
        conflicts,
    })
}

/// One printed factor of a share or unit relation: its value and rounding step (zero: exact).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Factor {
    pub printed: Rat,
    pub step: Rat,
}

/// Why a share or unit relation was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShareRefusal {
    /// A constant of zero or below, or a factor whose true range is not wholly positive.
    NotPositive,
    NegativeStep,
    Interval(IntervalError),
}

impl From<IntervalError> for ShareRefusal {
    fn from(e: IntervalError) -> Self {
        Self::Interval(e)
    }
}

impl From<UnitsError> for ShareRefusal {
    fn from(e: UnitsError) -> Self {
        Self::Interval(e.into())
    }
}

/// The true values a factor prints as, under `convention`; a zero step is the exact point.
fn factor_interval(f: Factor, convention: Convention) -> Result<Interval, ShareRefusal> {
    match f.step.signum() {
        -1 => Err(ShareRefusal::NegativeStep),
        0 => Ok(Interval {
            lo: f.printed,
            hi: f.printed,
            lo_strict: false,
            hi_strict: false,
        }),
        _ => Ok(convention.interval(f.printed, f.step.checked_div(Rat::from_integer(2))?)?),
    }
}

/// The shares s = c·Π numerators / Π denominators over the true values of printed factors, each a different
/// cell with a wholly positive range (part 3; x_t = s·x_s is one numerator over one denominator). The ratio
/// rises with each numerator and falls with each denominator, and the box is connected, so every value between
/// the corner ends is attained; an end is open when a factor end it comes from is open.
pub fn share_interval(
    c: Rat,
    numerators: &[Factor],
    denominators: &[Factor],
    convention: Convention,
) -> Result<Bound, ShareRefusal> {
    if c.signum() <= 0 {
        return Err(ShareRefusal::NotPositive);
    }
    let (mut lo, mut hi) = (
        End {
            at: c,
            strict: false,
        },
        End {
            at: c,
            strict: false,
        },
    );
    for (f, numerator) in numerators
        .iter()
        .map(|f| (f, true))
        .chain(denominators.iter().map(|f| (f, false)))
    {
        let iv = factor_interval(*f, convention)?;
        if iv.lo.signum() <= 0 {
            return Err(ShareRefusal::NotPositive);
        }
        let (to_lo, to_hi) = if numerator {
            ((iv.lo, iv.lo_strict), (iv.hi, iv.hi_strict))
        } else {
            ((iv.hi, iv.hi_strict), (iv.lo, iv.lo_strict))
        };
        let apply = |e: End, (at, strict): (Rat, bool)| -> Result<End, UnitsError> {
            Ok(End {
                at: if numerator {
                    e.at.checked_mul(at)?
                } else {
                    e.at.checked_div(at)?
                },
                strict: e.strict || strict,
            })
        };
        lo = apply(lo, to_lo)?;
        hi = apply(hi, to_hi)?;
    }
    Ok(Bound {
        lo: Some(lo),
        hi: Some(hi),
    })
}

/// A value printed in an unknown unit, and the same quantity printed in the reference unit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnitPair {
    pub observed: Factor,
    pub reference: Factor,
}

/// What a unit discovery finds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UnitFinding {
    /// Exactly one candidate fits every pair.
    Identified(String),
    /// Several do (FMINE-REF-104).
    Ambiguous(Vec<String>),
    /// None does.
    NoFit,
}

impl UnitFinding {
    /// [`NON_IDENTIFIABLE`] for an ambiguous finding, else `None`.
    #[must_use]
    pub const fn reason_code(&self) -> Option<&'static str> {
        match self {
            Self::Ambiguous(_) => Some(NON_IDENTIFIABLE),
            Self::Identified(_) | Self::NoFit => None,
        }
    }
}

/// A unit discovery: per candidate, how many pairs it fits; and the finding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnitDiscovery {
    pub fits: Vec<(String, usize)>,
    pub pairs: usize,
    pub finding: UnitFinding,
}

/// Which unit the observed values are in (part 5). A candidate `(name, factor)` turns an observed value into
/// the reference unit; it fits a pair when factor × the observed range meets the reference range.
pub fn discover_unit(
    pairs: &[UnitPair],
    candidates: &[(String, Rat)],
    convention: Convention,
) -> Result<UnitDiscovery, ShareRefusal> {
    let mut fits = Vec::new();
    for (name, factor) in candidates {
        let mut fitted = 0;
        for p in pairs {
            let scaled = share_interval(*factor, &[p.observed], &[], convention)?;
            let reference = Bound::of(factor_interval(p.reference, convention)?);
            if !scaled.meet(reference)?.is_empty()? {
                fitted += 1;
            }
        }
        fits.push((name.clone(), fitted));
    }
    let all: Vec<String> = fits
        .iter()
        .filter(|(_, n)| !pairs.is_empty() && *n == pairs.len())
        .map(|(name, _)| name.clone())
        .collect();
    let finding = match all.as_slice() {
        [] => UnitFinding::NoFit,
        [one] => UnitFinding::Identified(one.clone()),
        _ => UnitFinding::Ambiguous(all),
    };
    Ok(UnitDiscovery {
        fits,
        pairs: pairs.len(),
        finding,
    })
}

/// A reserve price deck: gold in $/oz and copper in $/lb, exact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Deck {
    pub name: String,
    pub gold: Rat,
    pub copper: Rat,
}

/// One GEO cell of a table row and the gold and copper cells it converts: geo = gold + copper · (copper price /
/// gold price), gold and GEO in Moz and copper in Mlb.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GeoRelation {
    pub gold: Factor,
    pub copper: Factor,
    pub geo: Factor,
}

/// How one deck fits one row's relations under one convention.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeckFit {
    pub convention: Convention,
    pub deck: String,
    /// The worst over the row's relations.
    pub fit: Fit,
}

/// What a deck discovery finds for one row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeckFinding {
    /// Every proper fit is on this deck. Found, not registered: the deck the row states is the caller's to compare.
    Found(String),
    /// Proper fits are on several decks (FMINE-REF-104).
    Ambiguous(Vec<String>),
    /// No deck fits properly under any convention.
    NoFit,
}

impl DeckFinding {
    /// [`NON_IDENTIFIABLE`] for an ambiguous finding, else `None`.
    #[must_use]
    pub const fn reason_code(&self) -> Option<&'static str> {
        match self {
            Self::Ambiguous(_) => Some(NON_IDENTIFIABLE),
            Self::Found(_) | Self::NoFit => None,
        }
    }
}

/// A deck discovery: every strict convention against every deck, the finding, and the conventions under which
/// some deck fits properly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeckDiscovery {
    pub fits: Vec<DeckFit>,
    pub finding: DeckFinding,
    pub conventions: Vec<Convention>,
}

/// How one relation fits at copper/gold price ratio `r` (positive). The converted sum rises with both inputs,
/// so its range runs from the sum of the low ends to the sum of the high ends, each end open when an input end
/// is; it fits where it meets the GEO cell's range. A meet of one point fits only at a tie, unless exact inputs
/// make the sum that one point.
fn geo_fit(rel: &GeoRelation, r: Rat, convention: Convention) -> Result<Fit, ShareRefusal> {
    let g = factor_interval(rel.gold, convention)?;
    let c = factor_interval(rel.copper, convention)?;
    let sum = Bound {
        lo: Some(End {
            at: g.lo.checked_add(r.checked_mul(c.lo)?)?,
            strict: g.lo_strict || c.lo_strict,
        }),
        hi: Some(End {
            at: g.hi.checked_add(r.checked_mul(c.hi)?)?,
            strict: g.hi_strict || c.hi_strict,
        }),
    };
    let range = sum.meet(Bound::of(factor_interval(rel.geo, convention)?))?;
    Ok(if range.is_empty()? {
        Fit::Empty
    } else if is_point(&range)? && !is_point(&sum)? {
        Fit::Points
    } else {
        Fit::Proper
    })
}

/// Which registered deck one row's GEO cells were converted at (part 4). One convention for the whole row, the
/// deck's document, never per cell; every strict convention is tried against every deck. A deck is found when
/// every proper fit is on it.
pub fn discover_deck(
    relations: &[GeoRelation],
    decks: &[Deck],
) -> Result<DeckDiscovery, ShareRefusal> {
    let mut fits = Vec::with_capacity(STRICT_CONVENTIONS.len() * decks.len());
    for &convention in &STRICT_CONVENTIONS {
        for deck in decks {
            if deck.gold.signum() <= 0 || deck.copper.signum() <= 0 {
                return Err(ShareRefusal::NotPositive);
            }
            let r = deck.copper.checked_div(deck.gold)?;
            let mut fit = Fit::Proper;
            for rel in relations {
                match geo_fit(rel, r, convention)? {
                    Fit::Empty => fit = Fit::Empty,
                    Fit::Points if fit == Fit::Proper => fit = Fit::Points,
                    _ => {}
                }
            }
            fits.push(DeckFit {
                convention,
                deck: deck.name.clone(),
                fit,
            });
        }
    }
    let proper = || {
        fits.iter()
            .filter(|f| !relations.is_empty() && f.fit == Fit::Proper)
    };
    let mut found: Vec<String> = proper().map(|f| f.deck.clone()).collect();
    found.sort();
    found.dedup();
    let mut conventions: Vec<Convention> = proper().map(|f| f.convention).collect();
    conventions.sort();
    conventions.dedup();
    let finding = match found.as_slice() {
        [] => DeckFinding::NoFit,
        [one] => DeckFinding::Found(one.clone()),
        _ => DeckFinding::Ambiguous(found),
    };
    Ok(DeckDiscovery {
        fits,
        finding,
        conventions,
    })
}

#[cfg(test)]
mod tests {
    use fmine_core::rand::{PhiloxStream, RngKey};

    use super::*;

    fn rat(n: i128, d: i128) -> Rat {
        Rat::new(n, d).expect("rational")
    }

    #[test]
    fn test_p0_exact_values() {
        assert_eq!(p0(0), Some(rat(1, 1)));
        assert_eq!(p0(1), Some(rat(1, 1)));
        assert_eq!(p0(2), Some(rat(3, 4)));
        assert_eq!(p0(3), Some(rat(2, 3)));
        assert_eq!(p0(4), Some(rat(115, 192)));
        assert_eq!(p0(MAX_K + 1), None);
        // Every p₀ up to MAX_K is a probability that falls with k (the sum spreads out).
        let mut last = rat(1, 1);
        for k in 2..=MAX_K {
            let p = p0(k).expect("within MAX_K");
            assert!(p.signum() > 0, "p0({k}) > 0");
            assert_eq!(p.checked_cmp(last), Ok(std::cmp::Ordering::Less), "p0({k})");
            last = p;
        }
        // Σ p₀ over every k up to MAX_K, fifty times over (about the real ledger's size): the exact sum does not
        // overflow, since its denominator stays the lcm of the p₀ denominators.
        let mixed: Vec<Footing> = (0..50)
            .flat_map(|_| (0..=MAX_K).map(|k| Footing { k, residual: 0 }))
            .collect();
        let r = discover_construction(&mixed).expect("no overflow");
        assert_eq!(r.n, 1_050);
    }

    /// 10⁵ seeded Philox draws of k uniforms per k: the share with |Σ| < ½ is within 4σ of p₀(k).
    #[test]
    fn prop_p0_matches_seeded_monte_carlo() {
        let draws = 100_000_u32;
        for k in [1_u32, 2, 3, 4, 6, 9, 13, 20] {
            let mut rng = PhiloxStream::new(RngKey {
                seed_root: 0x4d30_4a53,
                partition: 4,
                domain: 4,
                var_group: k,
                realization: 0,
                brick: 0,
            });
            let hits = (0..draws)
                .filter(|_| {
                    let s: f64 = (0..k).map(|_| rng.next_f64_open01() - 0.5).sum();
                    s.abs() < 0.5
                })
                .count();
            let p = p0(k).expect("within MAX_K").to_f64();
            let observed = hits as f64 / f64::from(draws);
            let sigma = (p * (1.0 - p) / f64::from(draws)).sqrt().max(1e-9);
            assert!(
                (observed - p).abs() <= 4.0 * sigma + 1e-12,
                "k {k}: observed {observed}, p0 {p}"
            );
        }
    }

    #[test]
    fn test_poisson_binomial_tails_known_answers() {
        // Two fair trials: P(X ≥ 1) = 3/4, P(X ≥ 2) = 1/4, P(X ≤ 0) = 1/4.
        let half = rat(1, 2);
        let ln = ln_poisson_binomial(&[half, half]);
        let p: Vec<f64> = ln.iter().map(|&l| det::exp(l)).collect();
        for (got, want) in p.iter().zip([0.25, 0.5, 0.25]) {
            assert!((got - want).abs() < 1e-12, "{p:?}");
        }
        // 400 footings of two summands, all exact: P(X ≥ 400) = (3/4)^400, ln = 400·ln(3/4).
        let footings = vec![Footing { k: 2, residual: 0 }; 400];
        let r = discover_construction(&footings).expect("report");
        let want = 400.0 * det::ln(0.75);
        assert!(
            (r.ln_upper_tail - want).abs() < 1e-9 * want.abs(),
            "{}",
            r.ln_upper_tail
        );
        assert!(
            r.ln_lower_tail.abs() < 1e-9,
            "P(X ≤ n) = 1: {}",
            r.ln_lower_tail
        );
        assert_eq!(r.expected_rounded_once, rat(300, 1));
        assert_eq!(r.discovered, Discovered::ComponentSum);
    }

    /// Synthetic footings drawn under each construction are discovered as it.
    #[test]
    fn test_construction_discovery_synthetic_component_sum_and_rounded_once() {
        // ComponentSum: totals are the sum of the printed lines, so every residual is 0 (k from 2 to 8).
        let component: Vec<Footing> = (0..300)
            .map(|i| Footing {
                k: 2 + i % 7,
                residual: 0,
            })
            .collect();
        let r = discover_construction(&component).expect("report");
        assert_eq!((r.n, r.foot_exact, r.rounded_once_accepts), (300, 300, 300));
        assert_eq!(r.discovered, Discovered::ComponentSum);

        // RoundedOnce: true lines with fractional parts, each line and the total rounded once.
        let mut rng = PhiloxStream::new(RngKey {
            seed_root: 0x4d30_4a53,
            partition: 4,
            domain: 5,
            var_group: 0,
            realization: 0,
            brick: 0,
        });
        let rounded: Vec<Footing> = (0..300_u32)
            .map(|i| {
                let k = 2 + i % 7;
                let lines: Vec<f64> = (0..k).map(|_| rng.next_f64_open01() * 100.0).collect();
                let printed: i64 = lines.iter().map(|x| x.round() as i64).sum();
                let total = lines.iter().sum::<f64>().round() as i64;
                Footing {
                    k,
                    residual: i128::from(printed - total),
                }
            })
            .collect();
        let r = discover_construction(&rounded).expect("report");
        assert_eq!(
            r.rounded_once_accepts, 300,
            "every rounding-once residual is within the half-steps"
        );
        assert!(r.foot_exact < 300);
        assert_eq!(r.discovered, Discovered::RoundedOnce, "{r:?}");
    }

    #[test]
    fn test_ambiguous_discovery_refuses_ref_104() {
        // Few footings, all exact: both constructions accept all and three exact two-line totals are likely
        // under rounding once (27/64).
        let few = vec![Footing { k: 2, residual: 0 }; 3];
        let r = discover_construction(&few).expect("report");
        assert_eq!(r.discovered, Discovered::Ambiguous);
        assert_eq!(r.discovered.reason_code(), Some("FMINE-REF-104"));
        assert_eq!(Discovered::ComponentSum.reason_code(), None);
        assert_eq!(Discovered::RoundedOnce.reason_code(), None);
        // One three-line total that misses by a unit: one issuer error or one rounding, so nothing is found.
        let one = discover_construction(&[Footing { k: 3, residual: 1 }]).expect("report");
        assert_eq!((one.foot_exact, one.rounded_once_accepts), (0, 1));
        assert!((one.ln_component_sum_error_tail - det::ln(0.1)).abs() < 1e-12);
        assert_eq!(one.discovered, Discovered::Ambiguous, "{one:?}");
        // Exact footings far rarer than rounding once predicts: neither fits.
        let rare: Vec<Footing> = (0..200)
            .map(|i| Footing {
                k: 2,
                residual: i128::from(i % 50 != 0),
            })
            .collect();
        let r = discover_construction(&rare).expect("report");
        assert_eq!(r.foot_exact, 4);
        assert_eq!(r.discovered, Discovered::Ambiguous, "{r:?}");
        // Nothing scored: past MAX_K a footing is indeterminate.
        let r = discover_construction(&[Footing { k: 21, residual: 0 }]).expect("report");
        assert_eq!(
            (r.n, r.indeterminate, r.discovered),
            (0, 1, Discovered::Ambiguous)
        );
    }

    fn dec(s: &str) -> Rat {
        Rat::from_decimal_str(s).expect("decimal")
    }

    fn printed(document: &str, quantity: &str, value: &str, step: &str) -> Printed {
        Printed {
            document: document.to_string(),
            quantity: quantity.to_string(),
            printed: dec(value),
            step: dec(step),
        }
    }

    fn end(at: &str, strict: bool) -> Option<super::super::End> {
        Some(super::super::End {
            at: dec(at),
            strict,
        })
    }

    fn assignment(t: &TieDiscovery, deck: Convention, mda: Convention) -> &Assignment {
        t.assignments
            .iter()
            .find(|a| a.conventions == [("deck".to_string(), deck), ("mda".to_string(), mda)])
            .expect("every pair is assessed")
    }

    /// E698 as the bead transcribes it: the Fourmile deck prints depletion 3.6 and net change 1.7 (Moz), the
    /// Q4 2025 MD&A 3.7 and 1.8, all at step 0.1.
    #[test]
    fn test_e698_convention_pairs_fixture() {
        use Convention::{Closed, HalfDown, HalfEven, HalfUp, Truncate};
        let cells = [
            printed("deck", "depletion", "3.6", "0.1"),
            printed("deck", "net_change", "1.7", "0.1"),
            printed("mda", "depletion", "3.7", "0.1"),
            printed("mda", "net_change", "1.8", "0.1"),
        ];
        let t = discover_tie_conventions(&cells).expect("discovery");
        assert_eq!(t.assignments.len(), 16);
        // No single strict convention fits both documents.
        for c in STRICT_CONVENTIONS {
            assert_eq!(assignment(&t, c, c).fit, Fit::Empty, "{c:?}");
        }
        // Deck Truncate with MD&A HalfUp: depletion in [3.65, 3.70), net change in [1.75, 1.80).
        let a = assignment(&t, Truncate, HalfUp);
        assert_eq!(a.fit, Fit::Proper);
        let range = |lo: (&str, bool), hi: (&str, bool)| Bound {
            lo: end(lo.0, lo.1),
            hi: end(hi.0, hi.1),
        };
        assert_eq!(
            a.ranges,
            [
                (
                    "depletion".to_string(),
                    range(("3.65", false), ("3.7", true))
                ),
                (
                    "net_change".to_string(),
                    range(("1.75", false), ("1.8", true))
                )
            ]
        );
        // Deck HalfDown with MD&A HalfUp, and Closed in both, fit only at the tie points.
        let points = [
            (
                "depletion".to_string(),
                range(("3.65", false), ("3.65", false)),
            ),
            (
                "net_change".to_string(),
                range(("1.75", false), ("1.75", false)),
            ),
        ];
        let a = assignment(&t, HalfDown, HalfUp);
        assert_eq!((a.fit, &a.ranges[..]), (Fit::Points, &points[..]));
        assert_eq!(
            t.closed.conventions,
            [("deck".to_string(), Closed), ("mda".to_string(), Closed)]
        );
        assert_eq!(
            (t.closed.fit, &t.closed.ranges[..]),
            (Fit::Points, &points[..])
        );
        // Only deck Truncate fits properly, with the MD&A at any of three: the deck's convention is found, the
        // MD&A's is not (FMINE-REF-104).
        let proper: Vec<(Convention, Convention)> = t
            .assignments
            .iter()
            .filter(|a| a.fit == Fit::Proper)
            .map(|a| (a.conventions[0].1, a.conventions[1].1))
            .collect();
        assert_eq!(
            proper,
            [
                (Truncate, HalfUp),
                (Truncate, HalfDown),
                (Truncate, HalfEven)
            ]
        );
        assert_eq!(
            t.findings,
            [
                ("deck".to_string(), TieFinding::Identified(Truncate)),
                (
                    "mda".to_string(),
                    TieFinding::Ambiguous(vec![HalfUp, HalfDown, HalfEven])
                )
            ]
        );
        assert_eq!(t.findings[1].1.reason_code(), Some("FMINE-REF-104"));
        assert_eq!(t.findings[0].1.reason_code(), None);
    }

    /// One convention per document: two quantities that would each fit under their own per-cell choice, but under
    /// no one choice per document, find no proper fit.
    #[test]
    fn test_convention_is_per_document_not_per_cell() {
        // q1 fits only with A truncating (3.6 then 3.7); q2 only with B truncating (1.8 in A, 1.7 in B). Each
        // needs the other document to round to nearest, so no one convention per document fits both.
        let cells = [
            printed("A", "q1", "3.6", "0.1"),
            printed("A", "q2", "1.8", "0.1"),
            printed("B", "q1", "3.7", "0.1"),
            printed("B", "q2", "1.7", "0.1"),
        ];
        let t = discover_tie_conventions(&cells).expect("discovery");
        assert!(t.assignments.iter().all(|a| a.fit != Fit::Proper));
        assert!(
            t.findings
                .iter()
                .all(|(_, f)| *f == TieFinding::NoProperFit)
        );
        // Every assignment gives both quantities of a document the same convention: the API has no per-cell
        // choice to make.
        for a in &t.assignments {
            assert_eq!(a.conventions.len(), 2);
        }
        // Each quantity alone fits properly (A Truncate, B HalfUp for q1): it is the per-document rule that fails.
        let alone =
            discover_tie_conventions(&[cells[0].clone(), cells[2].clone()]).expect("q1 alone");
        assert_eq!(
            alone.findings[0].1,
            TieFinding::Identified(Convention::Truncate)
        );
    }

    #[test]
    fn test_tie_discovery_refusals() {
        let one = [printed("A", "q", "3.6", "0.1")];
        assert_eq!(
            discover_tie_conventions(&one),
            Err(TieRefusal::DocumentCount(1))
        );
        let zero = [
            printed("A", "q", "3.6", "0"),
            printed("B", "q", "3.7", "0.1"),
        ];
        assert_eq!(
            discover_tie_conventions(&zero),
            Err(TieRefusal::NonPositiveStep("A".into(), "q".into()))
        );
        let twice = [
            printed("A", "q", "3.6", "0.1"),
            printed("A", "q", "3.7", "0.1"),
            printed("B", "q", "3.7", "0.1"),
        ];
        assert_eq!(
            discover_tie_conventions(&twice),
            Err(TieRefusal::Duplicate("A".into(), "q".into()))
        );
        // 3.65 is off the 0.1 grid: half-even cannot know its parity.
        let off = [
            printed("A", "q", "3.65", "0.1"),
            printed("B", "q", "3.7", "0.1"),
        ];
        assert!(matches!(
            discover_tie_conventions(&off),
            Err(TieRefusal::Interval(_, _, IntervalError::OffGrid))
        ));
        let many: Vec<Printed> = ["A", "B", "C", "D", "E"]
            .iter()
            .map(|d| printed(d, "q", "3.6", "0.1"))
            .collect();
        assert_eq!(
            discover_tie_conventions(&many),
            Err(TieRefusal::DocumentCount(5))
        );
    }

    /// E692's shape: one document prints a quantity as 6.7 and a later one as 79. No rounding reconciles them, so
    /// the quantity is a conflict and every assignment is `Empty`; a quantity both print alike is no conflict.
    #[test]
    fn test_tie_conflicts_name_irreconcilable_quantities() {
        let cells = [
            printed("first", "net_change_2024", "6.7", "0.1"),
            printed("later", "net_change_2024", "79", "1"),
            printed("first", "net_change_2023", "6.7", "0.1"),
            printed("later", "net_change_2023", "6.7", "0.1"),
        ];
        let t = discover_tie_conventions(&cells).expect("discovery");
        assert_eq!(t.conflicts, ["net_change_2024"]);
        assert!(t.assignments.iter().all(|a| a.fit == Fit::Empty));
        assert!(
            t.findings
                .iter()
                .all(|(_, f)| *f == TieFinding::NoProperFit)
        );
        // Without the conflicting quantity the same documents fit, and no conflict is named.
        let t = discover_tie_conventions(&cells[2..]).expect("discovery");
        assert!(t.conflicts.is_empty());
        assert!(t.assignments.iter().any(|a| a.fit == Fit::Proper));
        // Prints a step apart touch at one closed point: no conflict, though no strict pair may fit properly.
        let touching = [
            printed("first", "q", "3.6", "0.1"),
            printed("later", "q", "3.7", "0.1"),
        ];
        assert!(
            discover_tie_conventions(&touching)
                .expect("discovery")
                .conflicts
                .is_empty()
        );
    }

    /// `Closed` holds every round-to-nearest interval but not a truncation one. The deck's 3.6 at a tenth,
    /// truncated, is [3.6, 3.7); the MD&A's 3.68 at a hundredth is [3.675, 3.685] closed. Their closed meet is
    /// empty, yet a truncating deck fits properly, so the quantity is no conflict.
    #[test]
    fn test_truncation_reconciles_what_closed_cannot() {
        use Convention::{HalfDown, HalfEven, HalfUp, Truncate};
        let cells = [
            printed("deck", "q", "3.6", "0.1"),
            printed("mda", "q", "3.68", "0.01"),
        ];
        let t = discover_tie_conventions(&cells).expect("discovery");
        assert_eq!(t.closed.fit, Fit::Empty);
        assert!(t.conflicts.is_empty(), "{:?}", t.conflicts);
        let proper: Vec<(Convention, Convention)> = t
            .assignments
            .iter()
            .filter(|a| a.fit == Fit::Proper)
            .map(|a| (a.conventions[0].1, a.conventions[1].1))
            .collect();
        assert_eq!(
            proper,
            [
                (Truncate, HalfUp),
                (Truncate, HalfDown),
                (Truncate, HalfEven),
                (Truncate, Truncate)
            ]
        );
        assert_eq!(t.findings[0].1, TieFinding::Identified(Truncate));
    }

    fn whole(n: i128) -> Factor {
        Factor {
            printed: Rat::from_integer(n),
            step: Rat::from_integer(1),
        }
    }

    /// E819, North Mara Q2 2026: cost of sales 117, ounces 37 (koz), cost of sales per ounce 2,736. The share s
    /// with r = 1000·s·N/Q lies in [2735.5·36.5/117,500, 2736.5·37.5/116,500] = [0.8498, 0.8808] to four places,
    /// which excludes the registered 0.84.
    #[test]
    fn test_e819_share_interval() {
        let thousandth = rat(1, 1000);
        let s = share_interval(
            thousandth,
            &[whole(2736), whole(37)],
            &[whole(117)],
            Convention::Closed,
        )
        .expect("interval");
        let (lo, hi) = (s.lo.expect("lo"), s.hi.expect("hi"));
        assert_eq!((lo.at, lo.strict), (rat(399_383, 470_000), false));
        assert_eq!((hi.at, hi.strict), (rat(410_475, 466_000), false));
        // To four places, nearest: 0.8498 and 0.8808.
        let within = |x: Rat, a: Rat, b: Rat| {
            x.checked_cmp(a) != Ok(Ordering::Less) && x.checked_cmp(b) == Ok(Ordering::Less)
        };
        assert!(within(lo.at, rat(84_975, 100_000), rat(84_985, 100_000)));
        assert!(within(hi.at, rat(88_075, 100_000), rat(88_085, 100_000)));
        assert_eq!(s.contains(rat(84, 100)), Ok(false), "the registered 0.84");
        assert_eq!(s.contains(rat(85, 100)), Ok(true));
        assert_eq!(s.contains(rat(88, 100)), Ok(true));
        assert_eq!(s.contains(rat(89, 100)), Ok(false));
        // Half-up: the lower end comes from a denominator's open upper end, the upper from numerators' open ends.
        let s = share_interval(
            thousandth,
            &[whole(2736), whole(37)],
            &[whole(117)],
            Convention::HalfUp,
        )
        .expect("interval");
        assert_eq!(
            (s.lo.map(|e| e.strict), s.hi.map(|e| e.strict)),
            (Some(true), Some(true))
        );
        // x_t = s·x_s, one numerator over one denominator: 45 of 100 is s ∈ [44.5/100.5, 45.5/99.5].
        let s = share_interval(rat(1, 1), &[whole(45)], &[whole(100)], Convention::Closed)
            .expect("x_t");
        assert_eq!(
            (s.lo.map(|e| e.at), s.hi.map(|e| e.at)),
            (Some(rat(89, 201)), Some(rat(91, 199)))
        );
        assert_eq!(
            share_interval(rat(0, 1), &[whole(45)], &[], Convention::Closed),
            Err(ShareRefusal::NotPositive)
        );
        assert_eq!(
            share_interval(rat(1, 1), &[whole(0)], &[], Convention::Closed),
            Err(ShareRefusal::NotPositive),
            "a printed 0 may be a true negative"
        );
        let negative = Factor {
            printed: rat(5, 1),
            step: rat(-1, 1),
        };
        assert_eq!(
            share_interval(rat(1, 1), &[negative], &[], Convention::Closed),
            Err(ShareRefusal::NegativeStep)
        );
    }

    /// E463 in shape: 2024 copper rows labelled "(millions of pounds)" print the summary's kilotonnes (30 and 29
    /// here, synthetic). Under kt every pair fits; under Mlb (1 Mlb = 45,359,237/10⁸ kt, from
    /// `AVDP_LB_TO_GRAMS_NUM`) none does.
    #[test]
    fn test_e463_unit_kt_fixture() {
        use fmine_units::{AVDP_LB_TO_GRAMS_DEN, AVDP_LB_TO_GRAMS_NUM};
        // kt per Mlb: 10⁶ lb at NUM/DEN g each, over 10⁹ g per kt.
        let kt_per_mlb = rat(AVDP_LB_TO_GRAMS_NUM, AVDP_LB_TO_GRAMS_DEN * 1000);
        assert_eq!(kt_per_mlb, rat(45_359_237, 100_000_000));
        let pair = |v: i128| UnitPair {
            observed: whole(v),
            reference: whole(v),
        };
        let candidates = [
            ("kt".to_string(), rat(1, 1)),
            ("Mlb".to_string(), kt_per_mlb),
        ];
        let u =
            discover_unit(&[pair(30), pair(29)], &candidates, Convention::Closed).expect("unit");
        assert_eq!(u.fits, [("kt".to_string(), 2), ("Mlb".to_string(), 0)]);
        assert_eq!(u.finding, UnitFinding::Identified("kt".to_string()));
        assert_eq!(u.finding.reason_code(), None);
        // Candidates within rounding of each other cannot be told apart.
        let close = [
            ("a".to_string(), rat(1, 1)),
            ("b".to_string(), rat(101, 100)),
        ];
        let u = discover_unit(&[pair(30)], &close, Convention::Closed).expect("unit");
        assert_eq!(
            u.finding,
            UnitFinding::Ambiguous(vec!["a".to_string(), "b".to_string()])
        );
        assert_eq!(u.finding.reason_code(), Some("FMINE-REF-104"));
        let u = discover_unit(&[], &candidates, Convention::Closed).expect("no pairs");
        assert_eq!(u.finding, UnitFinding::NoFit);
    }

    /// E143's registered decks, gold $/oz over copper $/lb.
    fn decks() -> Vec<Deck> {
        [(1200, 275), (1300, 300), (1400, 300), (1500, 325)]
            .iter()
            .map(|&(gold, cents)| Deck {
                name: format!("{gold}/{}.{:02}", cents / 100, cents % 100),
                gold: rat(gold, 1),
                copper: rat(cents, 100),
            })
            .collect()
    }

    /// One GEO relation; values and steps in tenths.
    fn geo(gold: (i128, i128), copper: (i128, i128), geo: (i128, i128)) -> GeoRelation {
        let f = |(v, step): (i128, i128)| Factor {
            printed: rat(v, 10),
            step: rat(step, 10),
        };
        GeoRelation {
            gold: f(gold),
            copper: f(copper),
            geo: f(geo),
        }
    }

    fn fits_of<'a>(d: &'a DeckDiscovery, deck: &'a str) -> impl Iterator<Item = &'a DeckFit> {
        d.fits.iter().filter(move |f| f.deck == deck)
    }

    /// E147 on the Fourmile deck's 2024 row (p.14, as `planning/corpus/C1_barrick_public_stats.md` transcribes
    /// it): P&P 89 Moz, 40,201 Mlb, 176 GEO; net change 17.0 Moz, 28,542 Mlb, 79 GEO. The row states
    /// $1,400/$3.00. The net change fits $1,500/$3.25 only, and only under a round-to-nearest convention.
    #[test]
    fn test_e147_deck_found_fixture() {
        let pp = geo((890, 10), (402_010, 10), (1760, 10));
        let net = geo((170, 1), (285_420, 10), (790, 10));
        let d = discover_deck(&[pp, net], &decks()).expect("discovery");
        assert_eq!(d.finding, DeckFinding::Found("1500/3.25".to_string()));
        assert_eq!(d.finding.reason_code(), None);
        // The row's own deck is rejected under every convention.
        assert!(fits_of(&d, "1400/3.00").all(|f| f.fit == Fit::Empty));
        assert_eq!(fits_of(&d, "1400/3.00").count(), STRICT_CONVENTIONS.len());
        // Truncation, the convention S4 part 2 finds for this deck on its MD&A ties (E850), fits no deck: under
        // it the net change converts to [78.84, 78.94), short of [79, 80).
        assert_eq!(
            d.conventions,
            [
                Convention::HalfUp,
                Convention::HalfDown,
                Convention::HalfEven
            ]
        );
        assert!(
            d.fits
                .iter()
                .filter(|f| f.convention == Convention::Truncate)
                .all(|f| f.fit == Fit::Empty)
        );
        // The P&P relation alone fits both 2024 and 2025 decks: the net change is what tells them apart.
        let alone = discover_deck(&[pp], &decks()).expect("discovery");
        assert_eq!(
            alone.finding,
            DeckFinding::Ambiguous(vec!["1400/3.00".to_string(), "1500/3.25".to_string()])
        );
    }

    /// The deck's 2023 row (P&P 77, 12,391, 105; net change 5.0, 728, 6.7; stated $1,300/$3.00) fits both the
    /// 2020–21 and the 2022–23 decks: the copper term is too small to tell them apart.
    #[test]
    fn test_deck_ambiguity_refuses_ref_104() {
        let pp = geo((770, 10), (123_910, 10), (1050, 10));
        let net = geo((50, 1), (7280, 10), (67, 1));
        let d = discover_deck(&[pp, net], &decks()).expect("discovery");
        assert_eq!(
            d.finding,
            DeckFinding::Ambiguous(vec!["1200/2.75".to_string(), "1300/3.00".to_string()])
        );
        assert_eq!(d.finding.reason_code(), Some("FMINE-REF-104"));
        assert_eq!(d.conventions, STRICT_CONVENTIONS.to_vec());
    }

    #[test]
    fn test_deck_refusals_and_no_fit() {
        // The 2025 row as printed: copper net change +131 where its own roll-forward needs −131 (E144) fits no
        // deck; read as −131 it fits every one.
        let printed = geo((17, 1), (1310, 10), (14, 1));
        let d = discover_deck(&[printed], &decks()).expect("discovery");
        assert_eq!(d.finding, DeckFinding::NoFit);
        assert_eq!(d.finding.reason_code(), None);
        assert!(d.conventions.is_empty());
        let signed = geo((17, 1), (-1310, 10), (14, 1));
        let d = discover_deck(&[signed], &decks()).expect("discovery");
        assert_eq!(
            d.finding,
            DeckFinding::Ambiguous(decks().into_iter().map(|d| d.name).collect())
        );
        // No relation constrains nothing, and finds nothing.
        let d = discover_deck(&[], &decks()).expect("discovery");
        assert_eq!(d.finding, DeckFinding::NoFit);
        let free = Deck {
            name: "free".to_string(),
            gold: rat(0, 1),
            copper: rat(3, 1),
        };
        assert_eq!(
            discover_deck(&[printed], &[free]),
            Err(ShareRefusal::NotPositive)
        );
        let negative = geo((17, -1), (1310, 10), (14, 1));
        assert_eq!(
            discover_deck(&[negative], &decks()),
            Err(ShareRefusal::NegativeStep)
        );
    }

    #[test]
    fn test_deck_point_fits_are_not_proper() {
        let exact = |v: Rat| Factor {
            printed: v,
            step: rat(0, 1),
        };
        // Gold 1.0 at a tenth and GEO 1.2 at three tenths, no copper: under half-even both ranges are closed and
        // touch at 1.05 only; under every other convention they miss. Nothing is found.
        let touching = GeoRelation {
            gold: Factor {
                printed: rat(1, 1),
                step: rat(1, 10),
            },
            copper: exact(rat(0, 1)),
            geo: Factor {
                printed: rat(12, 10),
                step: rat(3, 10),
            },
        };
        let d = discover_deck(&[touching], &decks()).expect("discovery");
        assert!(d.fits.iter().all(|f| f.fit
            == if f.convention == Convention::HalfEven {
                Fit::Points
            } else {
                Fit::Empty
            }));
        assert_eq!(d.finding, DeckFinding::NoFit);
        // Exact inputs make the converted sum one point; inside the GEO cell's range that is a proper fit:
        // 1,000 · 3/1,300 = 2.3077 prints 2.31 at the 2022–23 deck and nowhere else (truncated it prints 2.30).
        let pinned = GeoRelation {
            gold: exact(rat(0, 1)),
            copper: exact(rat(1000, 1)),
            geo: Factor {
                printed: rat(231, 100),
                step: rat(1, 100),
            },
        };
        let d = discover_deck(&[pinned], &decks()).expect("discovery");
        assert_eq!(d.finding, DeckFinding::Found("1300/3.00".to_string()));
    }
}
