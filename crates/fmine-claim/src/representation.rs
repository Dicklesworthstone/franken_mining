//! Determinism: BitwisePortable
//! `Representation` variants, `DistSpec` parametric families, and canonical binary codec (`BD-041`,
//! plan §8.1).
//!
//! Covers every §8.1 epistemic representation variant without breaking existing `Claim<T>` callers:
//! - `Point` (deterministic exact/audited point);
//! - `Distribution(DistSpec)` (validated Normal, LogNormal, Beta, Gamma, StudentT, Uniform,
//!   Triangular, and Dirichlet specifications, using `fmine_core::det` for portable CDF/quantile);
//! - `Ensemble { index_space, artifact_sha256 }` (shared stochastic realization index, §9.5);
//! - `Quantiles(Vec<(u16, f64)>)` (strictly ascending basis-point probability grid `1..=9999`,
//!   non-decreasing quantile values);
//! - `Interval { lo, hi, kind }` (`Enclosure`, `Credible`, `Conformal`, `Manski`);
//! - `Category { scheme, code }` (classification label such as `"JORC Indicated"`, NEVER mapped to
//!   a numeric probability);
//! - `Projected { value, policy_id, of }` (book/disclosure point projected from an upstream `ClaimId`);
//! - `Refused { code, detail }` (explicit structured refusal carried as representation).
//!
//! Wire framing uses `FMNE` + `REPR` + version + CRC32C (`fmine_core::codec`). Non-finite numbers
//! and out-of-domain distribution parameters are refused, never clamped.

use core::fmt;

use fmine_core::codec::{CodecError, Decoder, Encoder};
use fmine_core::det::{exp, ln, norm_cdf, norm_ppf, sqrt};
use fmine_core::hash::Digest32;
use fmine_core::id::{ObjectId, PrefixKind};
use fmine_evidence::{Refusal, RefusalCode};

use crate::{ClaimId, Representation};

/// Format tag for `DistSpec` standalone frames (`FMNE` + `DIST`).
pub const TAG_DIST_SPEC: [u8; 4] = *b"DIST";

/// Format tag for `ClaimRepresentation` frames (`FMNE` + `REPR`).
pub const TAG_REPRESENTATION: [u8; 4] = *b"REPR";

/// Current schema version for `DistSpec` and `ClaimRepresentation` frames.
pub const FMT_REPR_V1: u16 = 1;

const MAX_REPR_STR_BYTES: usize = 256;
const MAX_QUANTILE_POINTS: usize = 1024;
const MAX_DIRICHLET_DIM: usize = 256;

/// Typed errors for [`DistSpec`] and [`ClaimRepresentation`] validation and framing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReprError {
    Codec(CodecError),
    InvalidTag { kind: &'static str, tag: u8 },
    InvalidParameter { field: &'static str, detail: String },
    InvalidRefusalCode(String),
}

impl ReprError {
    /// Maps validation and framing failures to registered `FMINE-REF-*` codes.
    #[must_use]
    pub fn to_refusal(&self) -> Refusal {
        match self {
            Self::InvalidParameter { .. } => {
                Refusal::new(RefusalCode::OutsidePhysicalDomain109, self.to_string())
            }
            Self::Codec(_) | Self::InvalidTag { .. } | Self::InvalidRefusalCode(_) => {
                Refusal::new(RefusalCode::DigestMismatch505, self.to_string())
            }
        }
    }
}

impl From<CodecError> for ReprError {
    fn from(value: CodecError) -> Self {
        Self::Codec(value)
    }
}

impl fmt::Display for ReprError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Codec(e) => write!(f, "representation codec error: {e}"),
            Self::InvalidTag { kind, tag } => write!(f, "invalid {kind} wire tag {tag}"),
            Self::InvalidParameter { field, detail } => {
                write!(f, "invalid representation parameter {field}: {detail}")
            }
            Self::InvalidRefusalCode(code) => {
                write!(f, "unregistered refusal code {code:?} in representation")
            }
        }
    }
}

impl std::error::Error for ReprError {}

fn check_finite(field: &'static str, v: f64) -> Result<(), ReprError> {
    if v.is_finite() {
        Ok(())
    } else {
        Err(ReprError::InvalidParameter {
            field,
            detail: format!("must be finite, got {v}"),
        })
    }
}

fn check_positive(field: &'static str, v: f64) -> Result<(), ReprError> {
    if v.is_finite() && v > 0.0 {
        Ok(())
    } else {
        Err(ReprError::InvalidParameter {
            field,
            detail: format!("must be finite and > 0, got {v}"),
        })
    }
}

fn put_f64(enc: &mut Encoder, v: f64) {
    // Normalize -0.0 to +0.0 for canonical bit-identical wire encoding.
    let canon = if v == 0.0 { 0.0 } else { v };
    enc.put_u64(canon.to_bits());
}

fn get_f64(dec: &mut Decoder<'_>, field: &'static str) -> Result<f64, ReprError> {
    let v = f64::from_bits(dec.get_u64()?);
    check_finite(field, v)?;
    Ok(v)
}

/// Parametric probability distribution specification (`BD-041`, plan §8.1).
///
/// Mirrors the `fsci-stats` family inventory with strict domain validation and `BitwisePortable`
/// canonical binary encoding + closed-form CDF/quantile helpers where available.
#[derive(Debug, Clone, PartialEq)]
pub enum DistSpec {
    Normal { mean: f64, std_dev: f64 },
    LogNormal { mu: f64, sigma: f64 },
    Beta { alpha: f64, beta: f64 },
    Gamma { shape: f64, scale: f64 },
    StudentT { df: f64, loc: f64, scale: f64 },
    Uniform { lo: f64, hi: f64 },
    Triangular { lo: f64, mode: f64, hi: f64 },
    Dirichlet { alphas: Vec<f64> },
}

impl DistSpec {
    /// Validate distribution parameters without clamping.
    pub fn validate(&self) -> Result<(), ReprError> {
        match self {
            Self::Normal { mean, std_dev } => {
                check_finite("normal.mean", *mean)?;
                check_positive("normal.std_dev", *std_dev)?;
            }
            Self::LogNormal { mu, sigma } => {
                check_finite("lognormal.mu", *mu)?;
                check_positive("lognormal.sigma", *sigma)?;
            }
            Self::Beta { alpha, beta } => {
                check_positive("beta.alpha", *alpha)?;
                check_positive("beta.beta", *beta)?;
            }
            Self::Gamma { shape, scale } => {
                check_positive("gamma.shape", *shape)?;
                check_positive("gamma.scale", *scale)?;
            }
            Self::StudentT { df, loc, scale } => {
                check_positive("student_t.df", *df)?;
                check_finite("student_t.loc", *loc)?;
                check_positive("student_t.scale", *scale)?;
            }
            Self::Uniform { lo, hi } => {
                check_finite("uniform.lo", *lo)?;
                check_finite("uniform.hi", *hi)?;
                if *lo >= *hi {
                    return Err(ReprError::InvalidParameter {
                        field: "uniform",
                        detail: format!("lo ({lo}) must be strictly less than hi ({hi})"),
                    });
                }
            }
            Self::Triangular { lo, mode, hi } => {
                check_finite("triangular.lo", *lo)?;
                check_finite("triangular.mode", *mode)?;
                check_finite("triangular.hi", *hi)?;
                if !(*lo <= *mode && *mode <= *hi && *lo < *hi) {
                    return Err(ReprError::InvalidParameter {
                        field: "triangular",
                        detail: format!(
                            "requires lo ({lo}) <= mode ({mode}) <= hi ({hi}) and lo < hi"
                        ),
                    });
                }
            }
            Self::Dirichlet { alphas } => {
                if !(2..=MAX_DIRICHLET_DIM).contains(&alphas.len()) {
                    return Err(ReprError::InvalidParameter {
                        field: "dirichlet.alphas",
                        detail: format!(
                            "dimension {} outside 2..={MAX_DIRICHLET_DIM}",
                            alphas.len()
                        ),
                    });
                }
                for &a in alphas {
                    check_positive("dirichlet.alpha", a)?;
                }
            }
        }
        Ok(())
    }

    /// Exact or `BitwisePortable` mean when defined (returns `None` for multivariate `Dirichlet`
    /// or `StudentT` with `df <= 1.0`).
    #[must_use]
    pub fn mean(&self) -> Option<f64> {
        if self.validate().is_err() {
            return None;
        }
        let val = match self {
            Self::Normal { mean, .. } => Some(*mean),
            Self::LogNormal { mu, sigma } => Some(exp(*mu + 0.5 * *sigma * *sigma)),
            Self::Beta { alpha, beta } => Some(*alpha / (*alpha + *beta)),
            Self::Gamma { shape, scale } => Some(*shape * *scale),
            Self::StudentT { df, loc, .. } => (*df > 1.0).then_some(*loc),
            Self::Uniform { lo, hi } => Some(0.5 * (*lo + *hi)),
            Self::Triangular { lo, mode, hi } => Some((*lo + *mode + *hi) / 3.0),
            Self::Dirichlet { .. } => None,
        };
        val.filter(|v| v.is_finite())
    }

    /// `BitwisePortable` closed-form CDF for univariate families that admit elementary evaluation
    /// via `fmine_core::det` (`Normal`, `LogNormal`, `Uniform`, `Triangular`).
    pub fn cdf(&self, x: f64) -> Result<Option<f64>, ReprError> {
        self.validate()?;
        check_finite("x", x)?;
        let out = match self {
            Self::Normal { mean, std_dev } => Some(norm_cdf((x - *mean) / *std_dev)),
            Self::LogNormal { mu, sigma } => {
                if x <= 0.0 {
                    Some(0.0)
                } else {
                    Some(norm_cdf((ln(x) - *mu) / *sigma))
                }
            }
            Self::Uniform { lo, hi } => {
                if x <= *lo {
                    Some(0.0)
                } else if x >= *hi {
                    Some(1.0)
                } else {
                    Some((x - *lo) / (*hi - *lo))
                }
            }
            Self::Triangular { lo, mode, hi } => {
                if x <= *lo {
                    Some(0.0)
                } else if x >= *hi {
                    Some(1.0)
                } else if x <= *mode {
                    let denom = (*hi - *lo) * (*mode - *lo);
                    Some(if denom == 0.0 {
                        0.0
                    } else {
                        (x - *lo) * (x - *lo) / denom
                    })
                } else {
                    let denom = (*hi - *lo) * (*hi - *mode);
                    Some(if denom == 0.0 {
                        1.0
                    } else {
                        1.0 - (*hi - x) * (*hi - x) / denom
                    })
                }
            }
            Self::Beta { .. }
            | Self::Gamma { .. }
            | Self::StudentT { .. }
            | Self::Dirichlet { .. } => None,
        };
        Ok(out)
    }

    /// `BitwisePortable` closed-form inverse CDF / quantile for `p ∈ (0, 1)`.
    pub fn quantile(&self, p: f64) -> Result<Option<f64>, ReprError> {
        self.validate()?;
        if !(p.is_finite() && p > 0.0 && p < 1.0) {
            return Err(ReprError::InvalidParameter {
                field: "p",
                detail: format!("probability must lie strictly inside (0, 1), got {p}"),
            });
        }
        let out = match self {
            Self::Normal { mean, std_dev } => Some(*mean + *std_dev * norm_ppf(p)),
            Self::LogNormal { mu, sigma } => Some(exp(*mu + *sigma * norm_ppf(p))),
            Self::Uniform { lo, hi } => Some(*lo + p * (*hi - *lo)),
            Self::Triangular { lo, mode, hi } => {
                let fc = (*mode - *lo) / (*hi - *lo);
                if p < fc {
                    Some(*lo + sqrt(p * (*hi - *lo) * (*mode - *lo)))
                } else {
                    Some(*hi - sqrt((1.0 - p) * (*hi - *lo) * (*hi - *mode)))
                }
            }
            Self::Beta { .. }
            | Self::Gamma { .. }
            | Self::StudentT { .. }
            | Self::Dirichlet { .. } => None,
        };
        if let Some(v) = out
            && !v.is_finite()
        {
            return Err(ReprError::InvalidParameter {
                field: "quantile",
                detail: format!("quantile evaluation overflowed to non-finite value {v}"),
            });
        }
        Ok(out)
    }

    fn encode_into(&self, enc: &mut Encoder) -> Result<(), ReprError> {
        self.validate()?;
        match self {
            Self::Normal { mean, std_dev } => {
                enc.put_u8(1);
                put_f64(enc, *mean);
                put_f64(enc, *std_dev);
            }
            Self::LogNormal { mu, sigma } => {
                enc.put_u8(2);
                put_f64(enc, *mu);
                put_f64(enc, *sigma);
            }
            Self::Beta { alpha, beta } => {
                enc.put_u8(3);
                put_f64(enc, *alpha);
                put_f64(enc, *beta);
            }
            Self::Gamma { shape, scale } => {
                enc.put_u8(4);
                put_f64(enc, *shape);
                put_f64(enc, *scale);
            }
            Self::StudentT { df, loc, scale } => {
                enc.put_u8(5);
                put_f64(enc, *df);
                put_f64(enc, *loc);
                put_f64(enc, *scale);
            }
            Self::Uniform { lo, hi } => {
                enc.put_u8(6);
                put_f64(enc, *lo);
                put_f64(enc, *hi);
            }
            Self::Triangular { lo, mode, hi } => {
                enc.put_u8(7);
                put_f64(enc, *lo);
                put_f64(enc, *mode);
                put_f64(enc, *hi);
            }
            Self::Dirichlet { alphas } => {
                enc.put_u8(8);
                let count = u32::try_from(alphas.len()).map_err(|_| {
                    ReprError::Codec(CodecError::LengthOutOfBounds {
                        len: alphas.len(),
                        max: MAX_DIRICHLET_DIM,
                    })
                })?;
                enc.put_u32(count);
                for &a in alphas {
                    put_f64(enc, a);
                }
            }
        }
        Ok(())
    }

    fn decode_from(dec: &mut Decoder<'_>) -> Result<Self, ReprError> {
        let tag = dec.get_u8()?;
        let spec = match tag {
            1 => Self::Normal {
                mean: get_f64(dec, "normal.mean")?,
                std_dev: get_f64(dec, "normal.std_dev")?,
            },
            2 => Self::LogNormal {
                mu: get_f64(dec, "lognormal.mu")?,
                sigma: get_f64(dec, "lognormal.sigma")?,
            },
            3 => Self::Beta {
                alpha: get_f64(dec, "beta.alpha")?,
                beta: get_f64(dec, "beta.beta")?,
            },
            4 => Self::Gamma {
                shape: get_f64(dec, "gamma.shape")?,
                scale: get_f64(dec, "gamma.scale")?,
            },
            5 => Self::StudentT {
                df: get_f64(dec, "student_t.df")?,
                loc: get_f64(dec, "student_t.loc")?,
                scale: get_f64(dec, "student_t.scale")?,
            },
            6 => Self::Uniform {
                lo: get_f64(dec, "uniform.lo")?,
                hi: get_f64(dec, "uniform.hi")?,
            },
            7 => Self::Triangular {
                lo: get_f64(dec, "triangular.lo")?,
                mode: get_f64(dec, "triangular.mode")?,
                hi: get_f64(dec, "triangular.hi")?,
            },
            8 => {
                let count = dec.get_u32()? as usize;
                if !(2..=MAX_DIRICHLET_DIM).contains(&count) {
                    return Err(ReprError::Codec(CodecError::LengthOutOfBounds {
                        len: count,
                        max: MAX_DIRICHLET_DIM,
                    }));
                }
                let mut alphas = Vec::with_capacity(count);
                for _ in 0..count {
                    alphas.push(get_f64(dec, "dirichlet.alpha")?);
                }
                Self::Dirichlet { alphas }
            }
            other => {
                return Err(ReprError::InvalidTag {
                    kind: "DistSpec",
                    tag: other,
                });
            }
        };
        spec.validate()?;
        Ok(spec)
    }

    /// Encode this `DistSpec` as a standalone canonical `FMNE` + `DIST` + CRC32C frame.
    pub fn to_bytes(&self) -> Result<Vec<u8>, ReprError> {
        let mut enc = Encoder::new_frame(TAG_DIST_SPEC, FMT_REPR_V1);
        self.encode_into(&mut enc)?;
        Ok(enc.finish())
    }

    /// Decode and validate a canonical `FMNE` + `DIST` frame.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, ReprError> {
        let mut dec = Decoder::open_frame(bytes, TAG_DIST_SPEC, FMT_REPR_V1, FMT_REPR_V1)?;
        let spec = Self::decode_from(&mut dec)?;
        dec.finish()?;
        Ok(spec)
    }
}

/// Semantics of an interval representation (plan §8.1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IntervalKind {
    /// Deterministic enclosure (interval arithmetic or rounding bounds).
    Enclosure,
    /// Bayesian credible interval at nominal level in basis points (`1..=9999`, e.g. `8000` = 80%).
    Credible { level_bps: u16 },
    /// Distribution-free conformal prediction interval anchored to a calibration record.
    Conformal {
        level_bps: u16,
        calibration_id: ObjectId,
    },
    /// Partial-identification Manski bounds (no distribution assumed inside `[lo, hi]`).
    Manski,
}

impl IntervalKind {
    fn validate(&self) -> Result<(), ReprError> {
        match self {
            Self::Enclosure | Self::Manski => Ok(()),
            Self::Credible { level_bps } => {
                if (1..=9999).contains(level_bps) {
                    Ok(())
                } else {
                    Err(ReprError::InvalidParameter {
                        field: "interval.level_bps",
                        detail: format!("level_bps {level_bps} outside 1..=9999"),
                    })
                }
            }
            Self::Conformal {
                level_bps,
                calibration_id,
            } => {
                if !(1..=9999).contains(level_bps) {
                    return Err(ReprError::InvalidParameter {
                        field: "interval.level_bps",
                        detail: format!("level_bps {level_bps} outside 1..=9999"),
                    });
                }
                if calibration_id.kind != PrefixKind::Calibration {
                    return Err(ReprError::InvalidParameter {
                        field: "interval.calibration_id",
                        detail: format!(
                            "expected calibration ObjectId (PrefixKind::Calibration), got {:?}",
                            calibration_id.kind
                        ),
                    });
                }
                Ok(())
            }
        }
    }
}

/// Complete §8.1 representation algebra with canonical `FMNE` + `REPR` + CRC32C round-trip codec.
#[derive(Debug, Clone, PartialEq)]
pub enum ClaimRepresentation {
    Point,
    Distribution(DistSpec),
    Ensemble {
        index_space: ObjectId,
        artifact_sha256: Digest32,
    },
    /// `(probability_bps, quantile_value)` with strictly ascending `probability_bps ∈ 1..=9999`
    /// and non-decreasing `quantile_value`.
    Quantiles(Vec<(u16, f64)>),
    Interval {
        lo: f64,
        hi: f64,
        kind: IntervalKind,
    },
    /// Discrete classification label (e.g. `"JORC"` / `"Indicated"`); never mapped to probability.
    Category {
        scheme: String,
        code: String,
    },
    /// Scalar projection required by a book or disclosure, linked back to its source `ClaimId`.
    Projected {
        value: f64,
        policy_id: String,
        of: ClaimId,
    },
    /// Structured refusal carried directly as the claim's representation.
    Refused {
        code: RefusalCode,
        detail: String,
    },
}

impl ClaimRepresentation {
    /// Validate invariants without clamping or reordering.
    pub fn validate(&self) -> Result<(), ReprError> {
        match self {
            Self::Point | Self::Ensemble { .. } => Ok(()),
            Self::Distribution(spec) => spec.validate(),
            Self::Quantiles(pts) => {
                if pts.is_empty() || pts.len() > MAX_QUANTILE_POINTS {
                    return Err(ReprError::InvalidParameter {
                        field: "quantiles",
                        detail: format!("count {} outside 1..={MAX_QUANTILE_POINTS}", pts.len()),
                    });
                }
                let mut prev: Option<(u16, f64)> = None;
                for &(p_bps, v) in pts {
                    if !(1..=9999).contains(&p_bps) {
                        return Err(ReprError::InvalidParameter {
                            field: "quantiles.p_bps",
                            detail: format!("probability {p_bps} bps outside 1..=9999"),
                        });
                    }
                    check_finite("quantiles.value", v)?;
                    if let Some((prev_p, prev_v)) = prev {
                        if p_bps <= prev_p {
                            return Err(ReprError::InvalidParameter {
                                field: "quantiles.p_bps",
                                detail: format!("must strictly ascend: {p_bps} after {prev_p}"),
                            });
                        }
                        if v < prev_v {
                            return Err(ReprError::InvalidParameter {
                                field: "quantiles.value",
                                detail: format!("must be non-decreasing: {v} after {prev_v}"),
                            });
                        }
                    }
                    prev = Some((p_bps, v));
                }
                Ok(())
            }
            Self::Interval { lo, hi, kind } => {
                check_finite("interval.lo", *lo)?;
                check_finite("interval.hi", *hi)?;
                if *lo > *hi {
                    return Err(ReprError::InvalidParameter {
                        field: "interval",
                        detail: format!("lo ({lo}) > hi ({hi})"),
                    });
                }
                kind.validate()
            }
            Self::Category { scheme, code } => {
                if scheme.is_empty() || scheme.len() > MAX_REPR_STR_BYTES {
                    return Err(ReprError::InvalidParameter {
                        field: "category.scheme",
                        detail: format!("length {} outside 1..={MAX_REPR_STR_BYTES}", scheme.len()),
                    });
                }
                if code.is_empty() || code.len() > MAX_REPR_STR_BYTES {
                    return Err(ReprError::InvalidParameter {
                        field: "category.code",
                        detail: format!("length {} outside 1..={MAX_REPR_STR_BYTES}", code.len()),
                    });
                }
                Ok(())
            }
            Self::Projected {
                of,
                value,
                policy_id,
                ..
            } => {
                if of.kind != PrefixKind::Claim {
                    return Err(ReprError::InvalidParameter {
                        field: "projected.of",
                        detail: format!(
                            "expected claim ObjectId (PrefixKind::Claim), got {:?}",
                            of.kind
                        ),
                    });
                }
                check_finite("projected.value", *value)?;
                if policy_id.is_empty() || policy_id.len() > MAX_REPR_STR_BYTES {
                    return Err(ReprError::InvalidParameter {
                        field: "projected.policy_id",
                        detail: format!(
                            "length {} outside 1..={MAX_REPR_STR_BYTES}",
                            policy_id.len()
                        ),
                    });
                }
                Ok(())
            }
            Self::Refused { detail, .. } => {
                if detail.len() > MAX_REPR_STR_BYTES {
                    return Err(ReprError::InvalidParameter {
                        field: "refused.detail",
                        detail: format!("length {} exceeds {MAX_REPR_STR_BYTES}", detail.len()),
                    });
                }
                Ok(())
            }
        }
    }

    /// Convert a legacy compact [`Representation`] into a validated [`ClaimRepresentation`].
    pub fn try_from_compact(repr: &Representation) -> Result<Self, ReprError> {
        let out = match repr {
            Representation::Deterministic => Self::Point,
            Representation::Gaussian { mean, std_dev } => Self::Distribution(DistSpec::Normal {
                mean: *mean,
                std_dev: *std_dev,
            }),
            Representation::Quantiles { p10, p50, p90 }
            | Representation::EnsembleSummary { p10, p50, p90, .. } => {
                Self::Quantiles(vec![(1000, *p10), (5000, *p50), (9000, *p90)])
            }
            Representation::Interval { lo, hi } => Self::Interval {
                lo: *lo,
                hi: *hi,
                kind: IntervalKind::Enclosure,
            },
            Representation::Placeholder { token } => Self::Category {
                scheme: "placeholder".to_string(),
                code: token.clone(),
            },
        };
        out.validate()?;
        Ok(out)
    }

    /// Encode this representation into a canonical `FMNE` + `REPR` + CRC32C frame.
    pub fn to_bytes(&self) -> Result<Vec<u8>, ReprError> {
        self.validate()?;
        let mut enc = Encoder::new_frame(TAG_REPRESENTATION, FMT_REPR_V1);
        match self {
            Self::Point => enc.put_u8(1),
            Self::Distribution(spec) => {
                enc.put_u8(2);
                spec.encode_into(&mut enc)?;
            }
            Self::Ensemble {
                index_space,
                artifact_sha256,
            } => {
                enc.put_u8(3);
                enc.put_str(&index_space.to_string());
                enc.put_fixed(artifact_sha256.as_bytes());
            }
            Self::Quantiles(pts) => {
                enc.put_u8(4);
                let count = u32::try_from(pts.len()).map_err(|_| {
                    ReprError::Codec(CodecError::LengthOutOfBounds {
                        len: pts.len(),
                        max: MAX_QUANTILE_POINTS,
                    })
                })?;
                enc.put_u32(count);
                for &(p_bps, v) in pts {
                    enc.put_u16(p_bps);
                    put_f64(&mut enc, v);
                }
            }
            Self::Interval { lo, hi, kind } => {
                enc.put_u8(5);
                put_f64(&mut enc, *lo);
                put_f64(&mut enc, *hi);
                match kind {
                    IntervalKind::Enclosure => enc.put_u8(1),
                    IntervalKind::Credible { level_bps } => {
                        enc.put_u8(2);
                        enc.put_u16(*level_bps);
                    }
                    IntervalKind::Conformal {
                        level_bps,
                        calibration_id,
                    } => {
                        enc.put_u8(3);
                        enc.put_u16(*level_bps);
                        enc.put_str(&calibration_id.to_string());
                    }
                    IntervalKind::Manski => enc.put_u8(4),
                }
            }
            Self::Category { scheme, code } => {
                enc.put_u8(6);
                enc.put_str(scheme);
                enc.put_str(code);
            }
            Self::Projected {
                value,
                policy_id,
                of,
            } => {
                enc.put_u8(7);
                put_f64(&mut enc, *value);
                enc.put_str(policy_id);
                enc.put_str(&of.to_string());
            }
            Self::Refused { code, detail } => {
                enc.put_u8(8);
                enc.put_str(code.as_str());
                enc.put_str(detail);
            }
        }
        Ok(enc.finish())
    }

    /// Decode and validate a canonical `FMNE` + `REPR` frame.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, ReprError> {
        let mut dec = Decoder::open_frame(bytes, TAG_REPRESENTATION, FMT_REPR_V1, FMT_REPR_V1)?;
        let tag = dec.get_u8()?;
        let repr = match tag {
            1 => Self::Point,
            2 => Self::Distribution(DistSpec::decode_from(&mut dec)?),
            3 => {
                let raw_id = dec.get_str(MAX_REPR_STR_BYTES)?;
                let index_space: ObjectId =
                    raw_id.parse().map_err(|_| ReprError::InvalidParameter {
                        field: "ensemble.index_space",
                        detail: format!("invalid ObjectId {raw_id:?}"),
                    })?;
                let artifact_sha256 = Digest32(dec.get_fixed::<32>()?);
                Self::Ensemble {
                    index_space,
                    artifact_sha256,
                }
            }
            4 => {
                let count = dec.get_u32()? as usize;
                if !(1..=MAX_QUANTILE_POINTS).contains(&count) {
                    return Err(ReprError::Codec(CodecError::LengthOutOfBounds {
                        len: count,
                        max: MAX_QUANTILE_POINTS,
                    }));
                }
                let mut pts = Vec::with_capacity(count.min(64));
                for _ in 0..count {
                    let p_bps = dec.get_u16()?;
                    let v = get_f64(&mut dec, "quantiles.value")?;
                    pts.push((p_bps, v));
                }
                Self::Quantiles(pts)
            }
            5 => {
                let lo = get_f64(&mut dec, "interval.lo")?;
                let hi = get_f64(&mut dec, "interval.hi")?;
                let kind = match dec.get_u8()? {
                    1 => IntervalKind::Enclosure,
                    2 => IntervalKind::Credible {
                        level_bps: dec.get_u16()?,
                    },
                    3 => {
                        let level_bps = dec.get_u16()?;
                        let raw_cal = dec.get_str(MAX_REPR_STR_BYTES)?;
                        let calibration_id: ObjectId =
                            raw_cal.parse().map_err(|_| ReprError::InvalidParameter {
                                field: "interval.calibration_id",
                                detail: format!("invalid ObjectId {raw_cal:?}"),
                            })?;
                        if calibration_id.kind != PrefixKind::Calibration {
                            return Err(ReprError::InvalidParameter {
                                field: "interval.calibration_id",
                                detail: format!("expected cal_ prefix, got {raw_cal:?}"),
                            });
                        }
                        IntervalKind::Conformal {
                            level_bps,
                            calibration_id,
                        }
                    }
                    4 => IntervalKind::Manski,
                    other => {
                        return Err(ReprError::InvalidTag {
                            kind: "IntervalKind",
                            tag: other,
                        });
                    }
                };
                Self::Interval { lo, hi, kind }
            }
            6 => Self::Category {
                scheme: dec.get_str(MAX_REPR_STR_BYTES)?.to_string(),
                code: dec.get_str(MAX_REPR_STR_BYTES)?.to_string(),
            },
            7 => {
                let value = get_f64(&mut dec, "projected.value")?;
                let policy_id = dec.get_str(MAX_REPR_STR_BYTES)?.to_string();
                let raw_of = dec.get_str(MAX_REPR_STR_BYTES)?;
                let of: ObjectId = raw_of.parse().map_err(|_| ReprError::InvalidParameter {
                    field: "projected.of",
                    detail: format!("invalid ClaimId {raw_of:?}"),
                })?;
                if of.kind != PrefixKind::Claim {
                    return Err(ReprError::InvalidParameter {
                        field: "projected.of",
                        detail: format!("expected clm_ prefix, got {raw_of:?}"),
                    });
                }
                Self::Projected {
                    value,
                    policy_id,
                    of,
                }
            }
            8 => {
                let raw_code = dec.get_str(MAX_REPR_STR_BYTES)?;
                let code = RefusalCode::parse(raw_code)
                    .ok_or_else(|| ReprError::InvalidRefusalCode(raw_code.to_string()))?;
                let detail = dec.get_str(MAX_REPR_STR_BYTES)?.to_string();
                Self::Refused { code, detail }
            }
            other => {
                return Err(ReprError::InvalidTag {
                    kind: "ClaimRepresentation",
                    tag: other,
                });
            }
        };
        dec.finish()?;
        repr.validate()?;
        Ok(repr)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fmine_core::hash::sha256;

    #[test]
    fn test_representation_and_dist_spec_canonical_roundtrip_and_domain_guards() {
        // 1. All 8 DistSpec families round-trip byte-for-byte.
        let dists = [
            DistSpec::Normal {
                mean: 1.85,
                std_dev: 0.25,
            },
            DistSpec::LogNormal {
                mu: 0.4,
                sigma: 0.35,
            },
            DistSpec::Beta {
                alpha: 2.5,
                beta: 5.0,
            },
            DistSpec::Gamma {
                shape: 3.0,
                scale: 12.5,
            },
            DistSpec::StudentT {
                df: 5.0,
                loc: 100.0,
                scale: 8.0,
            },
            DistSpec::Uniform { lo: 10.0, hi: 25.0 },
            DistSpec::Triangular {
                lo: 1.0,
                mode: 2.0,
                hi: 4.5,
            },
            DistSpec::Dirichlet {
                alphas: vec![1.0, 2.0, 3.5],
            },
        ];
        for dist in &dists {
            let bytes = dist.to_bytes().expect("encode dist");
            let decoded = DistSpec::from_bytes(&bytes).expect("decode dist");
            assert_eq!(&decoded, dist);
            assert_eq!(decoded.to_bytes().expect("re-encode"), bytes);
        }

        // Portable CDF / quantile inverse round-trip on Normal, LogNormal, Uniform, Triangular.
        for dist in [&dists[0], &dists[1], &dists[5], &dists[6]] {
            for p in [0.1, 0.25, 0.5, 0.75, 0.9] {
                let q = dist.quantile(p).expect("quantile").expect("closed form");
                let p_back = dist.cdf(q).expect("cdf").expect("closed form");
                assert!(
                    (p_back - p).abs() < 1e-6,
                    "CDF(quantile({p}))={p_back} for {dist:?}"
                );
            }
        }

        // 2. All 8 ClaimRepresentation variants round-trip cleanly.
        let scen_id = ObjectId::from_canonical_bytes(PrefixKind::Scenario, b"scen-2026");
        let cal_id = ObjectId::from_canonical_bytes(PrefixKind::Calibration, b"cal-2026");
        let clm_id = ObjectId::from_canonical_bytes(PrefixKind::Claim, b"clm-parent");
        let variants = [
            ClaimRepresentation::Point,
            ClaimRepresentation::Distribution(dists[0].clone()),
            ClaimRepresentation::Ensemble {
                index_space: scen_id,
                artifact_sha256: sha256(b"realizations-blob"),
            },
            ClaimRepresentation::Quantiles(vec![(1000, 1.2), (5000, 1.8), (9000, 2.6)]),
            ClaimRepresentation::Interval {
                lo: 1400.0,
                hi: 1520.0,
                kind: IntervalKind::Conformal {
                    level_bps: 8000,
                    calibration_id: cal_id,
                },
            },
            ClaimRepresentation::Interval {
                lo: 0.0,
                hi: 100.0,
                kind: IntervalKind::Manski,
            },
            ClaimRepresentation::Category {
                scheme: "JORC-2012".to_string(),
                code: "Indicated".to_string(),
            },
            ClaimRepresentation::Projected {
                value: 1460.0,
                policy_id: "disclosure.p50.v1".to_string(),
                of: clm_id,
            },
            ClaimRepresentation::Refused {
                code: RefusalCode::NoClaimInDomain201,
                detail: "outside drill spacing".to_string(),
            },
        ];
        for v in &variants {
            let bytes = v.to_bytes().expect("encode repr");
            let decoded = ClaimRepresentation::from_bytes(&bytes).expect("decode repr");
            assert_eq!(&decoded, v);
            assert_eq!(decoded.to_bytes().expect("re-encode repr"), bytes);
        }

        // 3. Compact Representation bridge round-trips.
        let compact = Representation::Gaussian {
            mean: 3.1,
            std_dev: 0.4,
        };
        let lifted = ClaimRepresentation::try_from_compact(&compact).expect("bridge");
        assert_eq!(
            lifted,
            ClaimRepresentation::Distribution(DistSpec::Normal {
                mean: 3.1,
                std_dev: 0.4,
            })
        );

        // 4. Out-of-domain parameters and crossing quantiles refuse (never clamp).
        assert!(
            DistSpec::Normal {
                mean: 0.0,
                std_dev: 0.0
            }
            .validate()
            .is_err()
        );
        assert!(DistSpec::Uniform { lo: 5.0, hi: 5.0 }.validate().is_err());
        assert!(
            DistSpec::Triangular {
                lo: 1.0,
                mode: 5.0,
                hi: 3.0
            }
            .validate()
            .is_err()
        );
        assert!(
            ClaimRepresentation::Quantiles(vec![(1000, 2.0), (5000, 1.5)])
                .validate()
                .is_err()
        );
        assert!(
            ClaimRepresentation::Interval {
                lo: 10.0,
                hi: 5.0,
                kind: IntervalKind::Enclosure
            }
            .validate()
            .is_err()
        );
        // Wrong ObjectId prefixes must fail validate() BEFORE to_bytes() (round-trip symmetry).
        assert!(
            ClaimRepresentation::Interval {
                lo: 1.0,
                hi: 2.0,
                kind: IntervalKind::Conformal {
                    level_bps: 8000,
                    calibration_id: clm_id, // wrong prefix: clm_ instead of cal_
                }
            }
            .to_bytes()
            .is_err()
        );
        assert!(
            ClaimRepresentation::Projected {
                value: 1.0,
                policy_id: "p50".to_string(),
                of: cal_id, // wrong prefix: cal_ instead of clm_
            }
            .to_bytes()
            .is_err()
        );
        // Overflowing LogNormal mean/quantile must not return Some(INFINITY).
        let huge_ln = DistSpec::LogNormal {
            mu: 1_000.0,
            sigma: 10.0,
        };
        assert_eq!(huge_ln.mean(), None);
        assert!(huge_ln.quantile(0.5).is_err());
    }
}
