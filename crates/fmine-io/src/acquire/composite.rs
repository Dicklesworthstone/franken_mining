//! DESIGN (BD-166, BD-188): imported assay support feeds domain-respecting composites.
//! Determinism: BitwisePortable after exact source decimals are projected to binary64.
//!
//! Values stay in one declared unit, basis and method. Missing observations contribute no numeric value;
//! below-detection observations contribute bounds, never DL/2. Source observations remain beside every
//! kernel input. This projection creates no assay record, QA/QC disposition or authority write.

use std::collections::{BTreeMap, BTreeSet};

use fmine_claim::{AdditivityClass, SpatialSupport};
use fmine_drill::composite::{
    self, Bounds, CompositeSet, Domain, Input, Residual, Rule, Weighting,
};
use fmine_drill::join::Span;
use fmine_evidence::{Refusal, RefusalCode};
use fmine_units::ExactQty;

use super::AssayReading;
use super::drill::{PreparedAssay, PreparedHole};
use crate::dialects::ColumnBasis;
use crate::redact::REDACTED_MARKER;

/// Bound the kernel's piece allocation, including dropped residuals, across all holes.
pub const MAX_COMPOSITE_PIECES: usize = 20_000;

/// One explicitly selected measurand and its length-compositing rule.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompositeSpec {
    pub column: String,
    /// Declared source method; fills absent source labels as an assumption, never a measured fact.
    pub method: String,
    pub unit: String,
    pub basis: ColumnBasis,
    pub length_mm: u64,
    pub residual: Residual,
}

/// Non-overlapping domain runs covering the whole declared hole.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HoleDomains {
    pub hole_id: String,
    pub domains: Vec<Domain>,
}

/// An unchanged source observation and whether its method comes only from the declaration.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceObservation {
    pub source: PreparedAssay,
    pub method_assumed: bool,
}

/// One hole's composites and the source/coverage mapping that interprets them.
#[derive(Clone, Debug, PartialEq)]
pub struct PreparedComposites {
    pub hole_id: String,
    pub set: CompositeSet,
    /// Kernel `Composite.inputs` indexes this vector in canonical source-depth order.
    pub inputs: Vec<SourceObservation>,
    /// Source placeholders or absent selected cells; all other exact readings remain in each source.
    pub missing: Vec<SourceObservation>,
    /// Core with a numeric value or a censoring bound, including dropped residuals.
    pub sampled_mm: u64,
    /// All other core: placeholder spans, absent selected cells and gaps with no source observation.
    pub unsampled_mm: u64,
}

fn unusable(detail: impl Into<String>) -> Refusal {
    Refusal::new(RefusalCode::SourceCellUnusable208, detail)
}

fn physical(detail: impl Into<String>) -> Refusal {
    Refusal::new(RefusalCode::OutsidePhysicalDomain109, detail)
}

fn support(detail: impl Into<String>) -> Refusal {
    Refusal::new(RefusalCode::SupportMismatch102, detail)
}

fn label(value: &str, field: &str) -> Result<(), Refusal> {
    if value.is_empty() || value.trim() != value || value.contains(REDACTED_MARKER) {
        return Err(unusable(format!(
            "compositing {field} must be explicit, nonempty and unredacted"
        )));
    }
    Ok(())
}

fn pieces(length: u64, spec: &CompositeSpec) -> u128 {
    let length = u128::from(length);
    let step = u128::from(spec.length_mm);
    let (whole, rest) = (length / step, length % step);
    match spec.residual {
        Residual::Distribute => ((length + step / 2) / step).max(1),
        Residual::MergeBelow { min_mm } if whole > 0 && rest > 0 && rest < u128::from(min_mm) => {
            whole
        }
        _ => whole + u128::from(rest > 0),
    }
}

fn domains_for(
    holes: &[PreparedHole],
    declared: &[HoleDomains],
    spec: &CompositeSpec,
) -> Result<BTreeMap<String, Vec<Domain>>, Refusal> {
    let mut by_hole = BTreeMap::new();
    for declaration in declared {
        label(&declaration.hole_id, "domain hole")?;
        let mut domains = declaration.domains.clone();
        domains.sort_by(|a, b| (a.span, &a.code).cmp(&(b.span, &b.code)));
        if by_hole
            .insert(declaration.hole_id.clone(), domains)
            .is_some()
        {
            return Err(unusable("duplicate domain declaration for a hole"));
        }
    }
    let mut names = BTreeSet::new();
    let mut count = 0_u128;
    for hole in holes {
        label(&hole.collar.hole_id, "hole identity")?;
        if !names.insert(hole.collar.hole_id.as_str()) {
            return Err(unusable("duplicate prepared hole"));
        }
        if hole.max_depth_mm == 0 || hole.max_depth_mm > (1_u64 << 53) {
            return Err(physical(
                "compositing MAXDEPTH must be positive and exactly representable in millimetres",
            ));
        }
        let domains = by_hole
            .get(&hole.collar.hole_id)
            .ok_or_else(|| unusable("every prepared hole requires explicit domain runs"))?;
        let mut cursor = 0;
        for domain in domains {
            label(&domain.code, "domain code")?;
            if domain.span.to_mm <= domain.span.from_mm || domain.span.to_mm > hole.max_depth_mm {
                return Err(physical("domain requires 0 <= FROM < TO <= MAXDEPTH"));
            }
            if domain.span.from_mm != cursor {
                return Err(support(
                    "domain runs must tile 0..MAXDEPTH without gaps or overlaps",
                ));
            }
            cursor = domain.span.to_mm;
            count += pieces(domain.span.length_mm(), spec);
            if count > MAX_COMPOSITE_PIECES as u128 {
                return Err(unusable(format!(
                    "compositing exceeds {MAX_COMPOSITE_PIECES} pieces including residuals; increase length or split on complete holes"
                )));
            }
        }
        if cursor != hole.max_depth_mm {
            return Err(support(
                "domain runs must cover the entire hole through MAXDEPTH",
            ));
        }
    }
    if by_hole.len() != names.len() {
        return Err(unusable(
            "domain declaration names a hole absent from prepared geometry",
        ));
    }
    Ok(by_hole)
}

fn value(reading: &AssayReading, spec: &CompositeSpec) -> Result<Option<Bounds>, Refusal> {
    let (qty, unit, basis, censored) = match reading {
        AssayReading::Measured { value, unit, basis } => (value, unit, basis, false),
        AssayReading::BelowDetection {
            detection_limit,
            unit,
            basis,
            ..
        } => (detection_limit, unit, basis, true),
        AssayReading::Placeholder { .. } => return Ok(None),
    };
    if unit != &spec.unit || basis != &spec.basis {
        return Err(Refusal::new(
            RefusalCode::UndeclaredUnitBasisOrTz107,
            "selected assay reading differs from the declared compositing unit or basis; no conversion is implicit",
        ));
    }
    let projected = qty.to_f64();
    if qty.signum() < 0
        || (censored && qty.signum() == 0)
        || !projected.is_finite()
        || (qty.signum() != 0 && projected == 0.0)
    {
        return Err(physical(
            "selected assay value or censoring bound has no valid nonnegative finite projection",
        ));
    }
    Ok(Some(Bounds {
        lo: if censored { 0.0 } else { projected },
        hi: projected,
    }))
}

/// Composite imported assay observations within explicitly complete domain runs.
///
/// `holes` come from [`super::drill::prepare_geometry`]. The caller also checks the selected column in the
/// original import report for PointCore support; `AssayReading` retains units and bases but not support.
/// Numeric readings must match the declaration's unit and basis, and every present method label must match.
/// Absent methods are marked as assumptions. The selected column must exist somewhere in the source, even
/// if only as a placeholder. Missing support is reported, not imputed or included in the weighted mean.
/// No source is changed, and no composite crosses a domain contact.
pub fn prepare_composites(
    holes: &[PreparedHole],
    spec: &CompositeSpec,
    domains: &[HoleDomains],
) -> Result<Vec<PreparedComposites>, Refusal> {
    for (field, value) in [
        ("column", &spec.column),
        ("method", &spec.method),
        ("unit", &spec.unit),
    ] {
        label(value, field)?;
    }
    if spec.length_mm == 0 {
        return Err(physical("composite length must be positive"));
    }
    if holes.is_empty()
        || !holes
            .iter()
            .flat_map(|h| &h.assays)
            .any(|a| a.source.assays.contains_key(&spec.column))
    {
        return Err(unusable(
            "selected compositing column is absent from every source observation",
        ));
    }
    let domains = domains_for(holes, domains, spec)?;
    let rule = Rule {
        length_mm: spec.length_mm,
        weighting: Weighting::Length,
        residual: spec.residual,
        additivity: AdditivityClass::IntensiveWeighted,
        input_support: SpatialSupport::PointCore,
    };
    let mut order = holes.iter().collect::<Vec<_>>();
    order.sort_by(|a, b| a.collar.hole_id.cmp(&b.collar.hole_id));
    let mut result = Vec::with_capacity(order.len());
    let mut rows = BTreeSet::new();
    for hole in order {
        let mut observations = hole.assays.iter().collect::<Vec<_>>();
        observations.sort_by_key(|a| (a.from_mm, a.to_mm, a.source.row_index));
        let mut previous_end = 0;
        let mut inputs = Vec::new();
        let mut sources = Vec::new();
        let mut missing = Vec::new();
        let mut sampled_mm = 0_u64;
        for observation in observations {
            let source = &observation.source;
            if source.hole_id != hole.collar.hole_id
                || source.row_index == 0
                || !rows.insert(source.row_index)
            {
                return Err(unusable(
                    "an assay observation has a wrong hole or duplicate/invalid source row",
                ));
            }
            if observation.from_mm >= observation.to_mm || observation.to_mm > hole.max_depth_mm {
                return Err(physical(
                    "assay support requires 0 <= FROM < TO <= MAXDEPTH",
                ));
            }
            if observation.from_mm < previous_end {
                return Err(support(
                    "source assay intervals overlap, including missing observations",
                ));
            }
            previous_end = observation.to_mm;
            if !ExactQty::new(i128::from(observation.from_mm), 1000)
                .is_ok_and(|m| m == source.from_m)
                || !ExactQty::new(i128::from(observation.to_mm), 1000)
                    .is_ok_and(|m| m == source.to_m)
            {
                return Err(unusable(
                    "prepared assay endpoints differ from the exact source interval",
                ));
            }
            if source.method.as_ref().is_some_and(|m| m != &spec.method) {
                return Err(unusable(format!(
                    "assay row {} has a different method; methods cannot be silently merged",
                    source.row_index
                )));
            }
            let preserved = SourceObservation {
                source: observation.clone(),
                method_assumed: source.method.is_none(),
            };
            let bounds = source
                .assays
                .get(&spec.column)
                .map(|r| value(r, spec))
                .transpose()?
                .flatten();
            if let Some(value) = bounds {
                let span = Span {
                    from_mm: observation.from_mm,
                    to_mm: observation.to_mm,
                };
                sampled_mm += span.length_mm();
                inputs.push(Input {
                    span,
                    value,
                    mass_kg: None,
                });
                sources.push(preserved);
            } else {
                missing.push(preserved);
            }
        }
        let runs = domains
            .get(&hole.collar.hole_id)
            .ok_or_else(|| unusable("prepared hole lost its domain declaration"))?;
        let set = composite::composite(runs, &inputs, &rule).map_err(|e| e.to_refusal())?;
        result.push(PreparedComposites {
            hole_id: hole.collar.hole_id.clone(),
            set,
            inputs: sources,
            missing,
            sampled_mm,
            unsampled_mm: hole.max_depth_mm - sampled_mm,
        });
    }
    Ok(result)
}
