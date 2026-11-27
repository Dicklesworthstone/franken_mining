//! Length- or mass-weighted compositing within domains (`BD-166`, plan §12.2, PO-CORE-2).
//! Determinism: BitwisePortable (IEEE-754 addition, multiplication and division with compensated sums).
//!
//! Each domain run along the hole is cut into composites of the rule's length, starting at the run's top, so no
//! composite crosses a domain boundary. The piece left at the bottom of a run is handled as the rule declares:
//! kept, merged into the composite above, dropped (and reported), or spread by cutting the run into equal
//! composites. A composite takes every input that overlaps it, through the overlap join, in proportion to the
//! core they share:
//!
//! - an intensive quantity (grade) is the weighted mean, by shared length or by the shared part of each input's
//!   mass ([`Weighting`]);
//! - an extensive quantity (contained metal, mass) is summed, each input contributing its shared fraction;
//! - a non-additive quantity is refused (`FMINE-REF-103`, `INV-014`).
//!
//! Core with no input is not zero grade: a composite reports how much of it was sampled, and a composite with no
//! sampled core has no value. A censored input contributes its bounds (below detection `[0, limit]`, above the
//! upper limit `[limit, 1]`), so a composite's value is a pair of bounds and no value is substituted for a
//! censored one (guard G-CENS).
//!
//! Every set is checked for conservation (PO-CORE-2). Σ weight × value over the composites, the dropped ones
//! included, must equal the same sum over the in-domain part of the inputs, computed separately from the
//! input-to-domain join, to 1e-12 relative. Otherwise the set is refused (`FMINE-REF-101`).

use fmine_claim::{AdditivityClass, SpatialSupport};

use crate::join::{Span, overlaps, self_overlaps};
use crate::records::{AssayValue, DrillError};

/// Largest relative difference allowed between the input and composite totals (PO-CORE-2).
pub const CONSERVATION_TOLERANCE: f64 = 1e-12;

/// A value known to lie in `[lo, hi]`; `lo == hi` for a measured value.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bounds {
    /// Lower bound.
    pub lo: f64,
    /// Upper bound.
    pub hi: f64,
}

impl Bounds {
    /// The bounds of an assay value, as a mass fraction.
    #[must_use]
    pub const fn of(value: &AssayValue) -> Self {
        match *value {
            AssayValue::Measured { mass_fraction } => Self {
                lo: mass_fraction,
                hi: mass_fraction,
            },
            AssayValue::BelowDetection { limit } => Self { lo: 0.0, hi: limit },
            AssayValue::AboveUpper { limit } => Self { lo: limit, hi: 1.0 },
        }
    }
}

/// One input along the hole: an assayed sample or a finer composite.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Input {
    /// The core it covers.
    pub span: Span,
    /// Its value.
    pub value: Bounds,
    /// Its dry mass, kg; needed for mass weighting.
    pub mass_kg: Option<f64>,
}

/// A domain run along the hole.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Domain {
    /// Domain code.
    pub code: String,
    /// The run.
    pub span: Span,
}

/// How an intensive value is weighted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Weighting {
    /// By the core length each input shares with the composite.
    Length,
    /// By the part of each input's mass in the shared core, taken as spread evenly along the input.
    Mass,
}

/// What happens to the piece left at the bottom of a domain run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Residual {
    /// It is a composite of its own, shorter than the rest.
    Keep,
    /// Shorter than `min_mm`, it joins the composite above it; it is kept when there is none.
    MergeBelow {
        /// Shortest piece kept alone, mm.
        min_mm: u64,
    },
    /// Shorter than `min_mm`, it is dropped and reported in [`CompositeSet::dropped`].
    DropBelow {
        /// Shortest piece kept, mm.
        min_mm: u64,
    },
    /// The run is cut into the whole number of equal composites closest to the rule's length.
    Distribute,
}

/// A compositing rule.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rule {
    /// Composite length, mm.
    pub length_mm: u64,
    /// Weighting of an intensive value.
    pub weighting: Weighting,
    /// Handling of the residual piece.
    pub residual: Residual,
    /// The quantity's additivity.
    pub additivity: AdditivityClass,
    /// The inputs' support: core samples or finer composites.
    pub input_support: SpatialSupport,
}

/// One composite.
#[derive(Clone, Debug, PartialEq)]
pub struct Composite {
    /// Its domain.
    pub domain: String,
    /// Its span.
    pub span: Span,
    /// Core in it that has an input, mm.
    pub sampled_mm: u64,
    /// The weight its value rests on: sampled length (mm) or sampled mass (kg) for an intensive value, sampled
    /// length (mm) for an extensive one.
    pub weight: f64,
    /// Its value; none when no core in it was sampled.
    pub value: Option<Bounds>,
    /// Its support.
    pub support: SpatialSupport,
    /// The inputs it takes, by index.
    pub inputs: Vec<usize>,
}

/// The conservation check of a set (PO-CORE-2).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Conservation {
    /// Σ weight × value over the in-domain inputs, lower and upper bounds.
    pub inputs: Bounds,
    /// The same over the composites, dropped ones included.
    pub composites: Bounds,
    /// The larger relative difference of the two.
    pub relative_error: f64,
}

/// The composites of one hole.
#[derive(Clone, Debug, PartialEq)]
pub struct CompositeSet {
    /// The composites, by depth.
    pub composites: Vec<Composite>,
    /// Residual pieces dropped under [`Residual::DropBelow`], by depth.
    pub dropped: Vec<Composite>,
    /// Its conservation check.
    pub conservation: Conservation,
}

/// Neumaier compensated sum.
#[derive(Clone, Copy, Default)]
struct Sum {
    total: f64,
    carry: f64,
}

impl Sum {
    fn add(&mut self, x: f64) {
        let t = self.total + x;
        if self.total.abs() >= x.abs() {
            self.carry += (self.total - t) + x;
        } else {
            self.carry += (x - t) + self.total;
        }
        self.total = t;
    }

    fn value(self) -> f64 {
        self.total + self.carry
    }
}

#[allow(clippy::cast_precision_loss)]
fn mm(v: u64) -> f64 {
    v as f64
}

fn check_rule(rule: &Rule) -> Result<(), DrillError> {
    if rule.length_mm == 0 {
        return Err(DrillError::invalid(
            "composite.length_mm",
            "a composite length is positive",
        ));
    }
    match rule.additivity {
        AdditivityClass::NonAdditive => {
            return Err(DrillError::NonAdditive {
                detail: "a non-additive quantity has no composite value (INV-014)".to_string(),
            });
        }
        AdditivityClass::ExtensiveAdditive | AdditivityClass::IntensiveWeighted => {}
    }
    match rule.input_support {
        SpatialSupport::PointCore | SpatialSupport::Composite { .. } => Ok(()),
        other => Err(DrillError::SupportMismatch {
            detail: format!(
                "downhole composites are made from core samples or composites, not {other:?}"
            ),
        }),
    }
}

fn check_inputs(inputs: &[Input], rule: &Rule) -> Result<(), DrillError> {
    let spans: Vec<Span> = inputs.iter().map(|i| i.span).collect();
    if let Some(o) = self_overlaps(&spans)?.first() {
        return Err(DrillError::invalid(
            "composite.inputs",
            format!(
                "inputs {} and {} both cover {} to {} mm; one value per core",
                o.left, o.right, o.span.from_mm, o.span.to_mm
            ),
        ));
    }
    for (i, input) in inputs.iter().enumerate() {
        let v = input.value;
        if !v.lo.is_finite() || !v.hi.is_finite() || v.lo > v.hi {
            return Err(DrillError::invalid(
                "composite.inputs",
                format!(
                    "input {i} has bounds [{}, {}]; finite bounds with lo <= hi",
                    v.lo, v.hi
                ),
            ));
        }
        if rule.weighting == Weighting::Mass
            && rule.additivity == AdditivityClass::IntensiveWeighted
            && !input.mass_kg.is_some_and(|m| m.is_finite() && m > 0.0)
        {
            return Err(DrillError::invalid(
                "composite.inputs",
                format!("input {i} has no positive mass; mass weighting needs every input's mass"),
            ));
        }
    }
    Ok(())
}

/// The composites of one domain run, with whether each is dropped.
fn pieces(run: Span, rule: &Rule) -> Vec<(Span, bool)> {
    let (from, len, step) = (run.from_mm, run.length_mm(), rule.length_mm);
    let span = |a: u64, b: u64| Span {
        from_mm: a,
        to_mm: b,
    };
    if rule.residual == Residual::Distribute {
        let n = ((len + step / 2) / step).max(1);
        let cut = |i: u64| {
            let offset = (u128::from(i) * u128::from(len) + u128::from(n) / 2) / u128::from(n);
            from + u64::try_from(offset).unwrap_or(len)
        };
        return (0..n).map(|i| (span(cut(i), cut(i + 1)), false)).collect();
    }
    let whole = len / step;
    let rest = len % step;
    let mut out: Vec<(Span, bool)> = (0..whole)
        .map(|i| (span(from + i * step, from + (i + 1) * step), false))
        .collect();
    if rest > 0 {
        let tail = span(from + whole * step, run.to_mm);
        match rule.residual {
            Residual::MergeBelow { min_mm } if rest < min_mm => match out.last_mut() {
                Some((last, _)) => last.to_mm = run.to_mm,
                None => out.push((tail, false)),
            },
            Residual::DropBelow { min_mm } if rest < min_mm => out.push((tail, true)),
            _ => out.push((tail, false)),
        }
    }
    out
}

/// The weight and the bounds-times-weight an input adds for `shared` core of it.
fn share(input: &Input, shared: Span, rule: &Rule) -> (f64, f64, f64) {
    let fraction = mm(shared.length_mm()) / mm(input.span.length_mm());
    match rule.additivity {
        AdditivityClass::IntensiveWeighted => {
            let w = match rule.weighting {
                Weighting::Length => mm(shared.length_mm()),
                Weighting::Mass => input.mass_kg.unwrap_or(0.0) * fraction,
            };
            (w, w * input.value.lo, w * input.value.hi)
        }
        AdditivityClass::ExtensiveAdditive | AdditivityClass::NonAdditive => (
            mm(shared.length_mm()),
            input.value.lo * fraction,
            input.value.hi * fraction,
        ),
    }
}

fn relative(a: f64, b: f64) -> f64 {
    let scale = a.abs().max(b.abs());
    if scale == 0.0 {
        0.0
    } else {
        (a - b).abs() / scale
    }
}

/// Composite `inputs` along one hole within `domains` under `rule`.
///
/// # Errors
/// - [`DrillError::Invalid`]: a zero composite length; domains or inputs that overlap each other or do not end
///   past where they start; bounds that are not finite or not ordered; an input without a positive mass under
///   mass weighting; a composite longer than `u32::MAX` mm.
/// - [`DrillError::NonAdditive`]: a non-additive quantity.
/// - [`DrillError::SupportMismatch`]: inputs that are neither core samples nor composites.
/// - [`DrillError::Conservation`]: the composites do not conserve the inputs to [`CONSERVATION_TOLERANCE`].
pub fn composite(
    domains: &[Domain],
    inputs: &[Input],
    rule: &Rule,
) -> Result<CompositeSet, DrillError> {
    check_rule(rule)?;
    let runs: Vec<Span> = domains.iter().map(|d| d.span).collect();
    if let Some(o) = self_overlaps(&runs)?.first() {
        return Err(DrillError::invalid(
            "composite.domains",
            format!(
                "domains '{}' and '{}' both cover {} to {} mm",
                domains[o.left].code, domains[o.right].code, o.span.from_mm, o.span.to_mm
            ),
        ));
    }
    check_inputs(inputs, rule)?;

    let mut order: Vec<usize> = (0..domains.len()).collect();
    order.sort_by_key(|&d| domains[d].span);
    let mut cut: Vec<(usize, Span, bool)> = Vec::new();
    for d in order {
        cut.extend(
            pieces(domains[d].span, rule)
                .into_iter()
                .map(|(s, drop)| (d, s, drop)),
        );
    }
    let spans: Vec<Span> = cut.iter().map(|&(_, s, _)| s).collect();
    let input_spans: Vec<Span> = inputs.iter().map(|i| i.span).collect();

    let mut built: Vec<(Composite, bool)> = Vec::with_capacity(cut.len());
    for &(d, span, drop) in &cut {
        let support = SpatialSupport::Composite {
            length_mm: u32::try_from(span.length_mm()).map_err(|_| {
                DrillError::invalid("composite.length_mm", "a composite longer than u32::MAX mm")
            })?,
        };
        built.push((
            Composite {
                domain: domains[d].code.clone(),
                span,
                sampled_mm: 0,
                weight: 0.0,
                value: None,
                support,
                inputs: Vec::new(),
            },
            drop,
        ));
    }
    let mut sums = vec![(Sum::default(), Sum::default(), Sum::default()); built.len()];
    for o in overlaps(&spans, &input_spans)? {
        let (w, lo, hi) = share(&inputs[o.right], o.span, rule);
        let (c, _) = &mut built[o.left];
        c.sampled_mm += o.span.length_mm();
        c.inputs.push(o.right);
        let s = &mut sums[o.left];
        s.0.add(w);
        s.1.add(lo);
        s.2.add(hi);
    }
    let mut out_lo = Sum::default();
    let mut out_hi = Sum::default();
    for ((c, _), (w, lo, hi)) in built.iter_mut().zip(&sums) {
        c.weight = w.value();
        if c.inputs.is_empty() {
            continue;
        }
        let value = match rule.additivity {
            AdditivityClass::IntensiveWeighted => Bounds {
                lo: lo.value() / c.weight,
                hi: hi.value() / c.weight,
            },
            AdditivityClass::ExtensiveAdditive | AdditivityClass::NonAdditive => Bounds {
                lo: lo.value(),
                hi: hi.value(),
            },
        };
        let k = match rule.additivity {
            AdditivityClass::IntensiveWeighted => c.weight,
            AdditivityClass::ExtensiveAdditive | AdditivityClass::NonAdditive => 1.0,
        };
        out_lo.add(k * value.lo);
        out_hi.add(k * value.hi);
        c.value = Some(value);
    }

    // The inputs' in-domain totals, from the input-to-domain join rather than from the composites.
    let mut in_lo = Sum::default();
    let mut in_hi = Sum::default();
    for o in overlaps(&runs, &input_spans)? {
        let (_, lo, hi) = share(&inputs[o.right], o.span, rule);
        in_lo.add(lo);
        in_hi.add(hi);
    }
    let conservation = Conservation {
        inputs: Bounds {
            lo: in_lo.value(),
            hi: in_hi.value(),
        },
        composites: Bounds {
            lo: out_lo.value(),
            hi: out_hi.value(),
        },
        relative_error: relative(in_lo.value(), out_lo.value())
            .max(relative(in_hi.value(), out_hi.value())),
    };
    if conservation.relative_error.is_nan() || conservation.relative_error > CONSERVATION_TOLERANCE
    {
        return Err(DrillError::Conservation {
            detail: format!(
                "inputs total [{}, {}], composites [{}, {}]: relative error {:e}",
                conservation.inputs.lo,
                conservation.inputs.hi,
                conservation.composites.lo,
                conservation.composites.hi,
                conservation.relative_error
            ),
        });
    }
    let (dropped, kept): (Vec<_>, Vec<_>) = built.into_iter().partition(|(_, drop)| *drop);
    Ok(CompositeSet {
        composites: kept.into_iter().map(|(c, _)| c).collect(),
        dropped: dropped.into_iter().map(|(c, _)| c).collect(),
        conservation,
    })
}
