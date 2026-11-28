//! Sample points of one support in one CRS (`BD-237`, `INV-014`).
//!
//! Determinism: BitwisePortable (`+ − × ÷` in sample order).
//!
//! A variogram compares values of one support: averaging or differencing values of different supports is refused
//! (`INV-014`). [`from_composites`] takes the composites of one domain and one declared support. It counts every
//! composite it leaves out, and why, and places each one it keeps at the trace point of its mid-depth.

use std::collections::BTreeSet;

use fmine_claim::SpatialSupport;
use fmine_drill::composite::CompositeSet;
use fmine_drill::desurvey::Trace;

use crate::GeostatError;

/// Most samples one set may hold.
pub const MAX_SAMPLES: usize = u32::MAX as usize;

/// One sample: a point and its value.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sample {
    /// Easting, m.
    pub east_m: f64,
    /// Northing, m.
    pub north_m: f64,
    /// Elevation, m.
    pub up_m: f64,
    /// Its value.
    pub value: f64,
    /// Its hole, as an index into [`SampleSet::holes`].
    pub hole: u32,
}

/// Samples of one support in one CRS.
#[derive(Clone, Debug, PartialEq)]
pub struct SampleSet {
    crs: String,
    support: SpatialSupport,
    holes: Vec<String>,
    samples: Vec<Sample>,
}

impl SampleSet {
    /// A set of `samples` in `crs`, all of `support`, from `holes`.
    ///
    /// # Errors
    /// [`GeostatError::Invalid`] for an empty CRS, a hole named twice, a sample whose hole is not in `holes`, a
    /// coordinate or value that is not finite, or more than [`MAX_SAMPLES`] samples.
    pub fn new(
        crs: impl Into<String>,
        support: SpatialSupport,
        holes: Vec<String>,
        samples: Vec<Sample>,
    ) -> Result<Self, GeostatError> {
        let crs = crs.into();
        if crs.is_empty() {
            return Err(GeostatError::invalid("samples.crs", "empty"));
        }
        let mut seen = BTreeSet::new();
        for hole in &holes {
            if !seen.insert(hole.as_str()) {
                return Err(GeostatError::invalid(
                    "samples.holes",
                    format!("hole '{hole}' named twice"),
                ));
            }
        }
        if samples.len() > MAX_SAMPLES {
            return Err(GeostatError::invalid(
                "samples",
                format!("{} samples; at most {MAX_SAMPLES}", samples.len()),
            ));
        }
        for (i, s) in samples.iter().enumerate() {
            if usize::try_from(s.hole).map_or(true, |h| h >= holes.len()) {
                return Err(GeostatError::invalid(
                    format!("samples[{i}].hole"),
                    format!("hole index {} with {} holes", s.hole, holes.len()),
                ));
            }
            for (name, v) in [
                ("east_m", s.east_m),
                ("north_m", s.north_m),
                ("up_m", s.up_m),
                ("value", s.value),
            ] {
                if !v.is_finite() {
                    return Err(GeostatError::invalid(
                        format!("samples[{i}].{name}"),
                        format!("{v} is not finite"),
                    ));
                }
            }
        }
        Ok(Self {
            crs,
            support,
            holes,
            samples,
        })
    }

    /// The CRS of every point.
    #[must_use]
    pub fn crs(&self) -> &str {
        &self.crs
    }

    /// The support of every value.
    #[must_use]
    pub const fn support(&self) -> SpatialSupport {
        self.support
    }

    /// The holes, by index.
    #[must_use]
    pub fn holes(&self) -> &[String] {
        &self.holes
    }

    /// The samples.
    #[must_use]
    pub fn samples(&self) -> &[Sample] {
        &self.samples
    }

    /// The population variance of the values, the usual reference for a model's total sill; `None` when empty.
    #[must_use]
    pub fn variance(&self) -> Option<f64> {
        if self.samples.is_empty() {
            return None;
        }
        let n = self.samples.len() as f64;
        let mean = self.samples.iter().map(|s| s.value).sum::<f64>() / n;
        let ss = self
            .samples
            .iter()
            .map(|s| (s.value - mean) * (s.value - mean))
            .sum::<f64>();
        Some(ss / n)
    }
}

/// What to do with a composite whose value is known only between bounds (a detection-limit result).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Censored {
    /// Refuse the set (`FMINE-REF-202`).
    Refuse,
    /// Take the lower bound.
    Lower,
    /// Take the upper bound.
    Upper,
    /// Take the middle of the bounds.
    Midpoint,
}

/// A sample set built from composites, with the count of every composite left out.
#[derive(Clone, Debug, PartialEq)]
pub struct FromComposites {
    /// The samples.
    pub set: SampleSet,
    /// Composites of the domain whose support is not the declared one, such as short residual pieces.
    pub other_support: usize,
    /// Composites of the domain with no sampled core.
    pub unsampled: usize,
    /// Composites of the domain whose value rests on only part of their core.
    pub partly_sampled: usize,
    /// Censored composites kept under [`Censored::Lower`], [`Censored::Upper`] or [`Censored::Midpoint`].
    pub censored: usize,
}

/// The composites of `domain` with support `support` from each hole's trace and composite set, as samples at
/// their mid-depth points (to the millimetre, rounded down).
///
/// Dropped residual pieces ([`CompositeSet::dropped`]) are not composites of the set and are not counted.
///
/// # Errors
/// - [`GeostatError::CrsMismatch`]: traces in more than one CRS.
/// - [`GeostatError::Invalid`]: no holes, an empty domain or CRS, a hole given twice, or a point or value that is
///   not finite.
/// - [`GeostatError::NotObservable`]: a censored composite under [`Censored::Refuse`].
pub fn from_composites(
    holes: &[(&Trace, &CompositeSet)],
    domain: &str,
    support: SpatialSupport,
    censored: Censored,
) -> Result<FromComposites, GeostatError> {
    if domain.is_empty() {
        return Err(GeostatError::invalid("composites.domain", "empty"));
    }
    let Some((first, _)) = holes.first() else {
        return Err(GeostatError::invalid("composites", "no holes"));
    };
    let crs = first.crs().to_string();
    if crs.is_empty() {
        return Err(GeostatError::invalid("composites.crs", "empty"));
    }
    let mut out = FromComposites {
        set: SampleSet {
            crs: crs.clone(),
            support,
            holes: Vec::with_capacity(holes.len()),
            samples: Vec::new(),
        },
        other_support: 0,
        unsampled: 0,
        partly_sampled: 0,
        censored: 0,
    };
    let mut seen = BTreeSet::new();
    for (h, (trace, set)) in holes.iter().enumerate() {
        if trace.crs() != crs {
            return Err(GeostatError::CrsMismatch {
                first: crs,
                other: trace.crs().to_string(),
            });
        }
        if !seen.insert(trace.hole_id()) {
            return Err(GeostatError::invalid(
                "composites",
                format!("hole '{}' given twice", trace.hole_id()),
            ));
        }
        out.set.holes.push(trace.hole_id().to_string());
        let hole = u32::try_from(h)
            .map_err(|_| GeostatError::invalid("composites", "more than u32::MAX holes"))?;
        for c in set.composites.iter().filter(|c| c.domain == domain) {
            if c.support != support {
                out.other_support += 1;
                continue;
            }
            let Some(bounds) = c.value else {
                out.unsampled += 1;
                continue;
            };
            if c.sampled_mm < c.span.length_mm() {
                out.partly_sampled += 1;
                continue;
            }
            let value = if bounds.lo == bounds.hi {
                bounds.lo
            } else {
                out.censored += 1;
                match censored {
                    Censored::Refuse => {
                        return Err(GeostatError::not_observable(format!(
                            "hole '{}' composite {} to {} mm: censored between {} and {}",
                            trace.hole_id(),
                            c.span.from_mm,
                            c.span.to_mm,
                            bounds.lo,
                            bounds.hi
                        )));
                    }
                    Censored::Lower => bounds.lo,
                    Censored::Upper => bounds.hi,
                    Censored::Midpoint => 0.5 * (bounds.lo + bounds.hi),
                }
            };
            let mid_mm = c.span.from_mm + (c.span.to_mm - c.span.from_mm) / 2;
            let p = trace.at(mid_mm).point;
            if ![p.easting_m, p.northing_m, p.elevation_m, value]
                .iter()
                .all(|v| v.is_finite())
            {
                return Err(GeostatError::invalid(
                    "composites",
                    format!(
                        "hole '{}' composite {} to {} mm: a point or value that is not finite",
                        trace.hole_id(),
                        c.span.from_mm,
                        c.span.to_mm
                    ),
                ));
            }
            if out.set.samples.len() == MAX_SAMPLES {
                return Err(GeostatError::invalid(
                    "composites",
                    format!("more than {MAX_SAMPLES} samples"),
                ));
            }
            out.set.samples.push(Sample {
                east_m: p.easting_m,
                north_m: p.northing_m,
                up_m: p.elevation_m,
                value,
                hole,
            });
        }
    }
    Ok(out)
}
