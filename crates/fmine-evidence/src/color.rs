//! Provenance: frankensim crates/fs-evidence/src/color.rs @ 20463525d9775a18c1b237d2676f364f367175ab; licence LicenseRef-MIT-OpenAI-Anthropic-Rider; Changes: ValidityDomain, ordered_bounds and rel_half_width moved in from fs-evidence lib.rs, cut to bounds-only; identity hashing uses fmine-core default-mode BLAKE3 over `domain || 0x00 || input` with this crate's own domain string, because fmine-core has no derive-key mode, so digests differ from upstream; `.expect` on length conversions replaced by `as u64`; regime_demotion loses its typed-axis branch; evd_006 and evd_008 ported to colours in the tests; Left out: typed axes and fs-qty, verified_from and its ColorError, color_of and the certificate types they need, card-set identities, payload_json, canonical_bytes, COLOR_ALGEBRA_VERSION
//!
//! The THREE-COLOR epistemic schema: every quantity carries a COLOR, **verified** (interval-certified numerics),
//! **validated** (anchored to data in a stated REGIME) or **estimated** (surrogates, judgment), and the composition
//! algebra is CONSERVATIVE BY TYPE: an estimate can never be laundered into a certificate, and validation is a
//! REGIONAL property that auto-demotes the moment execution leaves its regime (`§7.1`, NL-1, `BD-037`).

use std::collections::BTreeMap;

use fmine_core::hash::{Blake3, Digest32, blake3_hash};

/// Maximum byte length of a machine-readable color-provenance identity.
///
/// Composition replaces longer human-readable chains with a domain-separated
/// digest so conservative evidence propagation remains total and bounded.
pub const MAX_COLOR_IDENTITY_BYTES: usize = 256;

const COMPOSED_IDENTITY_DOMAIN: &str = "org.frankenmining.fmine-evidence.composed-identity.v1";
const DERIVED_IDENTITY_PREFIX: &str = "derived:v2:";

fn hash_domain(domain: &str, input: &[u8]) -> Digest32 {
    let mut framed = Vec::with_capacity(domain.len() + 1 + input.len());
    framed.extend_from_slice(domain.as_bytes());
    framed.push(0);
    framed.extend_from_slice(input);
    blake3_hash(&framed)
}

fn is_placeholder_token(value: &str) -> bool {
    [
        "-",
        "?",
        "n/a",
        "na",
        "none",
        "not run",
        "pending",
        "placeholder",
        "tbd",
        "todo",
        "unknown",
    ]
    .iter()
    .any(|placeholder| value.eq_ignore_ascii_case(placeholder))
}

/// Structural reason a machine-readable color identity is unusable.
#[must_use]
pub fn color_identity_reason(value: &str) -> Option<&'static str> {
    if value.len() > MAX_COLOR_IDENTITY_BYTES {
        return Some("too-long");
    }
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Some("blank")
    } else if trimmed != value {
        Some("surrounding-whitespace")
    } else if value.chars().any(char::is_control) {
        Some("control-character")
    } else if !value.bytes().all(|byte| {
        byte.is_ascii_alphanumeric()
            || matches!(byte, b'-' | b'_' | b'.' | b'/' | b':' | b'@' | b'+' | b'=')
    }) {
        Some("invalid-character")
    } else if is_placeholder_token(value) {
        Some("placeholder")
    } else {
        None
    }
}

fn bounded_pair_identity(hash_label: &str, left: &str, separator: &str, right: &str) -> String {
    if color_identity_reason(left).is_none() && color_identity_reason(right).is_none() {
        let readable = format!(
            "{DERIVED_IDENTITY_PREFIX}{hash_label}:{}:{left}{separator}{}:{right}",
            left.len(),
            right.len(),
        );
        if readable.len() <= MAX_COLOR_IDENTITY_BYTES {
            return readable;
        }
    }

    let label_hash = hash_domain(COMPOSED_IDENTITY_DOMAIN, hash_label.as_bytes());
    let left_hash = hash_domain(COMPOSED_IDENTITY_DOMAIN, left.as_bytes());
    let right_hash = hash_domain(COMPOSED_IDENTITY_DOMAIN, right.as_bytes());
    let mut identity = [0_u8; 96];
    identity[..32].copy_from_slice(label_hash.as_bytes());
    identity[32..64].copy_from_slice(left_hash.as_bytes());
    identity[64..].copy_from_slice(right_hash.as_bytes());
    format!(
        "{DERIVED_IDENTITY_PREFIX}{hash_label}:{}",
        hash_domain(COMPOSED_IDENTITY_DOMAIN, &identity)
    )
}

fn composed_estimator_identity(left: &str, right: &str) -> String {
    bounded_pair_identity("composed", left, "+", right)
}

fn verified_estimator_identity(estimator: &str) -> String {
    bounded_pair_identity("composed-verified", estimator, "+", "verified")
}

fn validated_estimator_identity(estimator: &str, dataset: &str) -> String {
    bounded_pair_identity("composed-validated", estimator, "+", dataset)
}

fn composed_dataset_identity(left: &str, right: &str) -> String {
    bounded_pair_identity("datasets", left, "+", right)
}

fn verified_dataset_identity(dataset: &str) -> String {
    bounded_pair_identity("datasets-verified", dataset, "+", "verified")
}

fn disjoint_regime_identity(left: &str, right: &str) -> String {
    bounded_pair_identity("disjoint-regimes", left, "+", right)
}

/// Bounded canonical estimator identity for a validated-color regime exit.
///
/// Exposed so replay engines reconstruct the exact same derived color rather
/// than duplicating the identity grammar.
#[must_use]
pub fn demotion_estimator_identity(dataset: &str, axis: &str) -> String {
    bounded_pair_identity("regime-exit", dataset, "@", axis)
}

/// Canonical `(lo ≤ hi)` ordering that PROPAGATES NaN. `f64::min`/`max`
/// silently DISCARD a NaN operand; a NaN input yields a NaN interval that fails
/// closed at the color gate.
fn ordered_bounds(lo: f64, hi: f64) -> (f64, f64) {
    if lo.is_nan() || hi.is_nan() {
        (f64::NAN, f64::NAN)
    } else if lo <= hi {
        (lo, hi)
    } else {
        (hi, lo)
    }
}

/// A named-parameter validity box: which region of parameter space the
/// evidence is good for. Missing parameters are unconstrained.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ValidityDomain {
    bounds: BTreeMap<String, (f64, f64)>,
}

impl ValidityDomain {
    /// The unconstrained domain.
    #[must_use]
    pub fn unconstrained() -> Self {
        ValidityDomain::default()
    }

    /// The declared axis bounds (read-only view).
    #[must_use]
    pub fn bounds(&self) -> &BTreeMap<String, (f64, f64)> {
        &self.bounds
    }

    /// Constrain one parameter to `[lo, hi]`. Reversed finite endpoints
    /// normalize; NaN stays unusable.
    #[must_use]
    pub fn with(mut self, param: impl Into<String>, lo: f64, hi: f64) -> Self {
        self.bounds.insert(param.into(), ordered_bounds(lo, hi));
        self
    }

    /// Per-parameter intersection (the composition law: composed validity
    /// is EXACTLY the intersection — never wider).
    #[must_use]
    pub fn intersect(&self, other: &Self) -> Self {
        let mut out = self.bounds.clone();
        for (k, &(lo2, hi2)) in &other.bounds {
            out.entry(k.clone())
                .and_modify(|(lo, hi)| {
                    if lo.is_nan() || hi.is_nan() || lo2.is_nan() || hi2.is_nan() {
                        *lo = f64::NAN;
                        *hi = f64::NAN;
                    } else {
                        *lo = lo.max(lo2);
                        *hi = hi.min(hi2);
                    }
                })
                .or_insert((lo2, hi2));
        }
        ValidityDomain { bounds: out }
    }

    /// True when some parameter's interval is empty or unusable.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.bounds
            .values()
            .any(|&(lo, hi)| !lo.is_finite() || !hi.is_finite() || lo > hi)
    }
}

/// The epistemic color, with its color-specific payload.
#[derive(Debug, Clone, PartialEq)]
pub enum Color {
    /// Interval-certified numerics: the payload IS the bound.
    Verified {
        /// Certified lower bound.
        lo: f64,
        /// Certified upper bound.
        hi: f64,
    },
    /// Anchored to data INSIDE a stated regime.
    Validated {
        /// The region of feature space where the anchoring holds.
        regime: ValidityDomain,
        /// Identity of the anchoring dataset.
        dataset: String,
    },
    /// Surrogates, judgment, assumptions.
    Estimated {
        /// The estimator's identity.
        estimator: String,
        /// The estimator's own dispersion: the relative spread charged by
        /// EVERY contributing slice, rigorous or not (∞ = no spread claim).
        /// `0.0` therefore means "genuinely zero width", not "the operand was
        /// rigorous so it cost nothing".
        dispersion: f64,
    },
}

/// A malformed color payload. Claim-strength exceptions cannot authorize any
/// of these structural defects.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ColorPayloadError {
    /// A machine-readable identity is malformed.
    InvalidIdentity {
        /// Payload field (`dataset`, `axis`, or `estimator`).
        field: &'static str,
        /// Offending value.
        value: String,
        /// Stable reason.
        reason: &'static str,
    },
    /// A Verified interval contains NaN, is inverted, or is a degenerate point
    /// at infinity. Half-infinite (`[-inf, x]`, `[x, +inf]`) and whole-line
    /// (`[-inf, +inf]`) endpoints remain valid, possibly vacuous enclosures.
    InvalidVerifiedInterval {
        /// Stable reason.
        reason: &'static str,
    },
    /// A Validated regime is missing or unusable.
    InvalidValidatedRegime {
        /// Offending axis, or empty when no axis was declared.
        axis: String,
        /// Stable reason.
        reason: &'static str,
    },
    /// Estimated dispersion is NaN or negative.
    InvalidEstimatedDispersion {
        /// Stable reason.
        reason: &'static str,
    },
}

impl core::fmt::Display for ColorPayloadError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::InvalidIdentity {
                field,
                value,
                reason,
            } => write!(f, "invalid {field} identity {value:?}: {reason}"),
            Self::InvalidVerifiedInterval { reason } => {
                write!(f, "invalid Verified interval: {reason}")
            }
            Self::InvalidValidatedRegime { axis, reason } if axis.is_empty() => {
                write!(f, "invalid Validated regime: {reason}")
            }
            Self::InvalidValidatedRegime { axis, reason } => {
                write!(f, "invalid Validated regime axis {axis:?}: {reason}")
            }
            Self::InvalidEstimatedDispersion { reason } => {
                write!(f, "invalid Estimated dispersion: {reason}")
            }
        }
    }
}

impl std::error::Error for ColorPayloadError {}

/// Validate the shared structural invariant for one color payload.
///
/// Only the *ordered, non-degenerate* infinite Verified shapes are allowed:
/// `[-inf, x]`, `[x, +inf]`, and `[-inf, +inf]` are sound but vacuous
/// enclosures. NaN, inverted, and degenerate point-at-infinity intervals
/// (`[+inf, +inf]`, `[-inf, -inf]`) are never valid: the latter enclose no
/// real value at all, so admitting them mints an unsatisfiable certificate at
/// the strongest colour.
///
/// # Errors
/// Returns the exact malformed field and stable reason.
pub fn validate_color_payload(color: &Color) -> Result<(), ColorPayloadError> {
    match color {
        Color::Verified { lo, hi } => {
            if lo.is_nan() || hi.is_nan() {
                Err(ColorPayloadError::InvalidVerifiedInterval {
                    reason: "bounds contain NaN",
                })
            } else if lo > hi {
                Err(ColorPayloadError::InvalidVerifiedInterval {
                    reason: "lower bound exceeds upper bound",
                })
            } else if lo.is_infinite() && lo.to_bits() == hi.to_bits() {
                Err(ColorPayloadError::InvalidVerifiedInterval {
                    reason: "degenerate point interval at infinity encloses no real value",
                })
            } else {
                Ok(())
            }
        }
        Color::Validated { regime, dataset } => {
            if let Some(reason) = color_identity_reason(dataset) {
                return Err(ColorPayloadError::InvalidIdentity {
                    field: "dataset",
                    value: dataset.clone(),
                    reason,
                });
            }
            if regime.bounds().is_empty() {
                return Err(ColorPayloadError::InvalidValidatedRegime {
                    axis: String::new(),
                    reason: "at least one bounded axis is required",
                });
            }
            for (axis, (lo, hi)) in regime.bounds() {
                if let Some(reason) = color_identity_reason(axis) {
                    return Err(ColorPayloadError::InvalidIdentity {
                        field: "axis",
                        value: axis.clone(),
                        reason,
                    });
                }
                if !lo.is_finite() || !hi.is_finite() {
                    return Err(ColorPayloadError::InvalidValidatedRegime {
                        axis: axis.clone(),
                        reason: "bounds must be finite",
                    });
                }
                if lo > hi {
                    return Err(ColorPayloadError::InvalidValidatedRegime {
                        axis: axis.clone(),
                        reason: "lower bound exceeds upper bound",
                    });
                }
            }
            Ok(())
        }
        Color::Estimated {
            estimator,
            dispersion,
        } => {
            if let Some(reason) = color_identity_reason(estimator) {
                return Err(ColorPayloadError::InvalidIdentity {
                    field: "estimator",
                    value: estimator.clone(),
                    reason,
                });
            }
            if dispersion.is_nan() || *dispersion < 0.0 {
                Err(ColorPayloadError::InvalidEstimatedDispersion {
                    reason: "value is NaN or negative",
                })
            } else {
                Ok(())
            }
        }
    }
}

fn compact_malformed_color_identity(label: &str, color: &Color) -> String {
    let mut hasher = Blake3::new();
    hasher.update(hash_domain(COMPOSED_IDENTITY_DOMAIN, label.as_bytes()).as_bytes());
    match color {
        Color::Verified { lo, hi } => {
            hasher.update(&[0]);
            hasher.update(&lo.to_bits().to_le_bytes());
            hasher.update(&hi.to_bits().to_le_bytes());
        }
        Color::Validated { regime, dataset } => {
            hasher.update(&[1]);
            hasher.update(hash_domain(COMPOSED_IDENTITY_DOMAIN, dataset.as_bytes()).as_bytes());
            hasher.update(&(regime.bounds().len() as u64).to_le_bytes());
            for (axis, (lo, hi)) in regime.bounds() {
                hasher.update(hash_domain(COMPOSED_IDENTITY_DOMAIN, axis.as_bytes()).as_bytes());
                hasher.update(&lo.to_bits().to_le_bytes());
                hasher.update(&hi.to_bits().to_le_bytes());
            }
        }
        Color::Estimated {
            estimator,
            dispersion,
        } => {
            hasher.update(&[2]);
            hasher.update(hash_domain(COMPOSED_IDENTITY_DOMAIN, estimator.as_bytes()).as_bytes());
            hasher.update(&dispersion.to_bits().to_le_bytes());
        }
    }
    let payload_hash = hasher.finalize();
    format!(
        "{DERIVED_IDENTITY_PREFIX}{label}:{}",
        hash_domain(COMPOSED_IDENTITY_DOMAIN, payload_hash.as_bytes())
    )
}

/// Lattice rank (higher = stronger claim). The composition result can
/// never OUTRANK the weakest operand — that is the no-laundering law.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ColorRank {
    /// Estimated.
    Estimated,
    /// Validated.
    Validated,
    /// Verified.
    Verified,
}

impl Color {
    /// The lattice rank.
    #[must_use]
    pub fn rank(&self) -> ColorRank {
        match self {
            Color::Verified { .. } => ColorRank::Verified,
            Color::Validated { .. } => ColorRank::Validated,
            Color::Estimated { .. } => ColorRank::Estimated,
        }
    }

    /// Stable name for ledger rows.
    #[must_use]
    pub fn name(&self) -> &'static str {
        match self {
            Color::Verified { .. } => "verified",
            Color::Validated { .. } => "validated",
            Color::Estimated { .. } => "estimated",
        }
    }
}

/// How verified intervals combine under the ledger operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntervalOp {
    /// Sum of quantities: bounds add.
    Add,
    /// Product: the four-corner hull.
    Mul,
    /// The UNION of both operand intervals, `[min(lo), max(hi)]`.
    ///
    /// This is an enclosure of the result ONLY for operations whose value
    /// always lies between its operands — min/max, selection, convex
    /// combination, and envelope folds. It is **not** a valid enclosure for
    /// difference, ratio, or any other operation that can leave the operand
    /// hull: composing `[10, 10]` with `[-10, -10]` under a subtraction yields
    /// `[-10, 10]`, which does NOT contain the true difference `20`.
    Hull,
}

/// A demotion record (validated → estimated on regime exit).
#[derive(Debug, Clone, PartialEq)]
pub struct Demotion {
    /// The dataset whose regime was exited.
    pub dataset: String,
    /// The axis that left its range.
    pub axis: String,
    /// The offending state value.
    pub value: f64,
}

/// The relative half-width an interval `[lo, hi]` contributes to a composed
/// `Estimated` dispersion, against its own midpoint magnitude. `+inf` when no
/// finite reference magnitude exists to relativize against — never a silent
/// zero.
fn interval_dispersion(lo: f64, hi: f64) -> f64 {
    let reference = lo / 2.0 + hi / 2.0;
    if !reference.is_finite() || lo.is_nan() || hi.is_nan() || lo > hi {
        return f64::INFINITY;
    }
    let half = 0.5 * (hi - lo);
    let relative = half / reference.abs().max(f64::MIN_POSITIVE);
    if half.is_finite() && half >= 0.0 && relative.is_finite() {
        relative
    } else {
        f64::INFINITY
    }
}

fn normalize_composition_input(color: &Color) -> Color {
    let Err(error) = validate_color_payload(color) else {
        return color.clone();
    };
    let label = match error {
        ColorPayloadError::InvalidIdentity { .. } => "invalid-color-identity",
        ColorPayloadError::InvalidVerifiedInterval { .. } => "invalid-verified-interval",
        ColorPayloadError::InvalidValidatedRegime { .. } => "invalid-validity-regime",
        ColorPayloadError::InvalidEstimatedDispersion { .. } => "invalid-estimated-dispersion",
    };
    Color::Estimated {
        estimator: compact_malformed_color_identity(label, color),
        dispersion: f64::INFINITY,
    }
}

/// Outward-round an arithmetic interval endpoint pair (one ulp each way) so a
/// composed enclosure stays a TRUE enclosure under round-to-nearest — otherwise
/// the lower bound could round UP (or the upper bound DOWN) and EXCLUDE the
/// exact result, laundering the very precision a `Verified` color certifies. A
/// NaN endpoint (∞ + −∞) degrades to the whole real line, the only sound
/// enclosure.
fn outward_round(lo: f64, hi: f64) -> (f64, f64) {
    if lo.is_nan() || hi.is_nan() {
        (f64::NEG_INFINITY, f64::INFINITY)
    } else {
        (lo.next_down(), hi.next_up())
    }
}

/// The TOTAL, conservative pairwise composition: the result never OUTRANKS
/// the weaker operand (no laundering). Malformed payloads and disjoint
/// validated regimes may demote further. Verified bounds combine per `op`
/// (`Add`/`Mul` are outward-rounded to preserve enclosure; `Hull` is exact
/// endpoint selection and is an enclosure only for the between-the-operands
/// operations named on [`IntervalOp::Hull`] — `compose` cannot check which
/// operation the caller meant, so it does not certify enclosure for `Hull`),
/// validated regimes INTERSECT (both anchors must hold), and estimated
/// dispersions add (conservative), charging every operand its own width.
#[must_use]
pub fn compose(a: &Color, b: &Color, op: IntervalOp) -> Color {
    let a = normalize_composition_input(a);
    let b = normalize_composition_input(b);
    match (&a, &b) {
        (Color::Verified { lo: a0, hi: a1 }, Color::Verified { lo: b0, hi: b1 }) => {
            let (lo, hi) = match op {
                IntervalOp::Add => outward_round(a0 + b0, a1 + b1),
                IntervalOp::Mul => {
                    let c = [a0 * b0, a0 * b1, a1 * b0, a1 * b1];
                    if c.iter().any(|x| x.is_nan()) {
                        // A NaN corner (0×∞) is indeterminate AND the min/max
                        // fold below would SILENTLY DROP it (f64::min/max ignore
                        // NaN), reporting a bogus tight interval. The whole real
                        // line is the only sound enclosure.
                        (f64::NEG_INFINITY, f64::INFINITY)
                    } else {
                        outward_round(
                            c.iter().copied().fold(f64::INFINITY, f64::min),
                            c.iter().copied().fold(f64::NEG_INFINITY, f64::max),
                        )
                    }
                }
                // Hull is exact endpoint selection (no arithmetic → no rounding).
                IntervalOp::Hull => (a0.min(*b0), a1.max(*b1)),
            };
            Color::Verified { lo, hi }
        }
        // Validated ⊕ verified stays validated (the weaker anchor);
        // validated ⊕ validated intersects regimes.
        (Color::Validated { regime, dataset }, Color::Verified { .. })
        | (Color::Verified { .. }, Color::Validated { regime, dataset }) => Color::Validated {
            regime: regime.clone(),
            dataset: verified_dataset_identity(dataset),
        },
        (
            Color::Validated {
                regime: r1,
                dataset: d1,
            },
            Color::Validated {
                regime: r2,
                dataset: d2,
            },
        ) => {
            let regime = intersect_domains(r1, r2);
            if regime.is_empty() {
                // Mutually unsatisfiable regimes: there is NO state where both
                // anchors hold, so the composition cannot honestly stay
                // Validated. There is also no defensible spread claim, so the
                // demotion carries infinite dispersion just like a regime exit.
                Color::Estimated {
                    estimator: disjoint_regime_identity(d1, d2),
                    dispersion: f64::INFINITY,
                }
            } else {
                Color::Validated {
                    regime,
                    dataset: composed_dataset_identity(d1, d2),
                }
            }
        }
        // Anything ⊕ estimated → estimated. No exceptions here; the
        // waiver door lives at the LEDGER, in provenance.
        (
            Color::Estimated {
                estimator,
                dispersion,
            },
            other,
        )
        | (
            other,
            Color::Estimated {
                estimator,
                dispersion,
            },
        ) => {
            // A Verified/Validated operand is NOT a zero-width operand. A
            // `Verified{0.0, 1e6}` carries 1e6 of absolute uncertainty, and a
            // `Validated` colour states a regime and a dataset but no width at
            // all. A Verified operand is charged its own relative half-width,
            // and a width-less Validated operand collapses the spread claim to
            // `+inf`, the same "no defensible spread claim" encoding the
            // disjoint-regime demotion above already uses.
            let (other_disp, composed_identity) = match other {
                Color::Estimated {
                    estimator: e2,
                    dispersion: v,
                } => (*v, composed_estimator_identity(estimator, e2)),
                Color::Verified { lo, hi } => (
                    interval_dispersion(*lo, *hi),
                    verified_estimator_identity(estimator),
                ),
                Color::Validated { dataset, .. } => (
                    f64::INFINITY,
                    validated_estimator_identity(estimator, dataset),
                ),
            };
            Color::Estimated {
                estimator: composed_identity,
                dispersion: dispersion + other_disp,
            }
        }
    }
}

/// Intersection of two validity domains (axis-wise; an axis present in
/// either constrains the result — both anchors must hold). Delegates to
/// [`ValidityDomain::intersect`], which PRESERVES emptiness (`lo > hi`) on a
/// disjoint axis so callers can detect an unsatisfiable regime via
/// [`ValidityDomain::is_empty`].
#[must_use]
pub fn intersect_domains(a: &ValidityDomain, b: &ValidityDomain) -> ValidityDomain {
    a.intersect(b)
}

/// Check a validated color against the CURRENT execution state:
/// inside the regime → unchanged; outside (or unreported axis) →
/// AUTOMATIC DEMOTION to estimated, with the flag returned. Verified
/// and estimated colors pass through untouched.
#[must_use]
pub fn check_regime(color: &Color, state: &BTreeMap<String, f64>) -> (Color, Option<Demotion>) {
    let Some(reason) = regime_demotion(color, state) else {
        return (color.clone(), None);
    };
    (
        Color::Estimated {
            estimator: demotion_estimator_identity(&reason.dataset, &reason.axis),
            dispersion: f64::INFINITY,
        },
        Some(reason),
    )
}

/// Determine whether a validated color exits its regime without cloning the
/// color payload. This is the borrowed preflight used by admission layers that
/// must bound a multi-parent fold before constructing the derived regime.
#[must_use]
pub fn regime_demotion(color: &Color, state: &BTreeMap<String, f64>) -> Option<Demotion> {
    let Color::Validated { regime, dataset } = color else {
        return None;
    };
    if regime.bounds().is_empty() {
        return Some(Demotion {
            dataset: dataset.clone(),
            axis: "<undeclared-regime>".to_string(),
            value: f64::NAN,
        });
    }
    for (axis, &(lo, hi)) in regime.bounds() {
        let Some(&v) = state.get(axis) else {
            return Some(Demotion {
                dataset: dataset.clone(),
                axis: axis.clone(),
                value: f64::NAN,
            });
        };
        if !lo.is_finite() || !hi.is_finite() || lo > hi || !v.is_finite() || v < lo || v > hi {
            return Some(Demotion {
                dataset: dataset.clone(),
                axis: axis.clone(),
                value: v,
            });
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use fmine_core::rand::{PhiloxStream, RngKey};

    const OPS: [IntervalOp; 3] = [IntervalOp::Add, IntervalOp::Mul, IntervalOp::Hull];

    fn regime(lo: f64, hi: f64) -> ValidityDomain {
        ValidityDomain::unconstrained().with("feed_grade", lo, hi)
    }

    fn state(value: f64) -> BTreeMap<String, f64> {
        BTreeMap::from([("feed_grade".to_string(), value)])
    }

    fn random_color(rng: &mut PhiloxStream) -> Color {
        let x = rng.next_f64_open01() * 10.0 - 5.0;
        let width = rng.next_f64_open01() * 3.0;
        match rng.next_u64() % 8 {
            0..=2 => Color::Verified {
                lo: x,
                hi: x + width,
            },
            3 | 4 => Color::Validated {
                regime: regime(x, x + width),
                dataset: format!("qaqc-{}", rng.next_u64() % 5),
            },
            5 | 6 => Color::Estimated {
                estimator: format!("model-{}", rng.next_u64() % 5),
                dispersion: width,
            },
            // A malformed payload: composition must demote it, never trust it.
            _ => Color::Verified { lo: x + 1.0, hi: x },
        }
    }

    /// The guard test of this lift, for every `IntervalOp`.
    #[test]
    fn test_compose_never_promotes() {
        const PAIRS: usize = 10_000;
        let key = RngKey {
            seed_root: 0x0009_D0C5_0037,
            partition: 2,
            domain: 0,
            var_group: 0,
            realization: 0,
            brick: 0,
        };
        let mut rng = PhiloxStream::new(key);
        let mut violations = 0_u32;
        for _ in 0..PAIRS {
            let (a, b) = (random_color(&mut rng), random_color(&mut rng));
            for op in OPS {
                let (ab, ba) = (compose(&a, &b, op), compose(&b, &a, op));
                let weakest = normalize_composition_input(&a)
                    .rank()
                    .min(normalize_composition_input(&b).rank());
                if ab.rank() > weakest || ba.rank() > weakest {
                    violations += 1;
                }
            }
        }
        println!(
            "{{\"test\":\"compose_never_promotes\",\"pairs\":{PAIRS},\"ops\":{},\"rng_key\":\"{:016x}\",\
             \"violations\":{violations}}}",
            OPS.len(),
            key.seed_root
        );
        assert_eq!(violations, 0);
    }

    /// Ported from upstream `evd_008_verified_compose_stays_a_true_enclosure`.
    #[test]
    fn test_verified_add_encloses_exact_sum() {
        let a = Color::Verified { lo: 0.1, hi: 0.1 };
        let b = Color::Verified { lo: 0.2, hi: 0.2 };
        let Color::Verified { lo, hi } = compose(&a, &b, IntervalOp::Add) else {
            panic!("Verified ⊕ Verified must stay Verified");
        };
        let mid = 0.1_f64 + 0.2_f64;
        assert!(lo < mid && mid < hi);
        assert_eq!(lo, mid.next_down());
        assert_eq!(hi, mid.next_up());

        let zero = Color::Verified { lo: 0.0, hi: 0.0 };
        let whole = Color::Verified {
            lo: f64::NEG_INFINITY,
            hi: f64::INFINITY,
        };
        let Color::Verified { lo, hi } = compose(&zero, &whole, IntervalOp::Mul) else {
            panic!("stays Verified");
        };
        assert!(lo == f64::NEG_INFINITY && hi == f64::INFINITY);
    }

    /// `evd_006_certified_discipline_composes` restated on colours: a rigorous chain stays rigorous, and an estimate
    /// anywhere poisons everything downstream.
    #[test]
    fn test_certified_discipline_composes() {
        let exact = Color::Verified { lo: 2.0, hi: 2.0 };
        let enclosed = Color::Verified { lo: 2.9, hi: 3.1 };
        let chain = compose(&exact, &enclosed, IntervalOp::Mul);
        let Color::Verified { lo, hi } = chain else {
            panic!("rigorous chain stays Verified");
        };
        assert!(lo <= 5.8 && 6.2 <= hi);

        let estimate = Color::Estimated {
            estimator: "surrogate".to_string(),
            dispersion: 0.1,
        };
        let poisoned = compose(&estimate, &enclosed, IntervalOp::Add);
        assert_eq!(poisoned.rank(), ColorRank::Estimated);
        let downstream = compose(&poisoned, &exact, IntervalOp::Mul);
        assert_eq!(downstream.rank(), ColorRank::Estimated);
        // The enclosure's own width is charged, never treated as zero.
        let Color::Estimated { dispersion, .. } = poisoned else {
            panic!("estimate ⊕ verified stays Estimated");
        };
        assert!(dispersion > 0.1);
    }

    #[test]
    fn test_check_regime_demotes_validated_outside_domain() {
        let validated = Color::Validated {
            regime: regime(1.0, 3.0),
            dataset: "qaqc-2026".to_string(),
        };
        let (inside, flag) = check_regime(&validated, &state(2.0));
        assert_eq!((inside, flag), (validated.clone(), None));

        let (outside, flag) = check_regime(&validated, &state(3.5));
        assert_eq!(outside.rank(), ColorRank::Estimated);
        let Some(demotion) = flag else {
            panic!("leaving the regime must be reported");
        };
        assert_eq!(demotion.axis, "feed_grade");
        assert_eq!(demotion.value, 3.5);

        // An unreported axis and a non-finite reading both demote.
        for reading in [BTreeMap::new(), state(f64::NAN), state(f64::INFINITY)] {
            assert_eq!(
                check_regime(&validated, &reading).0.rank(),
                ColorRank::Estimated
            );
        }
        // Verified and estimated pass through untouched.
        let verified = Color::Verified { lo: 1.0, hi: 2.0 };
        assert_eq!(check_regime(&verified, &state(9.0)), (verified, None));
    }

    #[test]
    fn test_hull_is_not_used_for_difference() {
        let plus = Color::Verified { lo: 10.0, hi: 10.0 };
        let minus = Color::Verified {
            lo: -10.0,
            hi: -10.0,
        };
        let Color::Verified { lo, hi } = compose(&plus, &minus, IntervalOp::Hull) else {
            panic!("Hull of two Verified stays Verified");
        };
        assert_eq!((lo, hi), (-10.0, 10.0));
        // The true difference is 20, outside the hull: `Hull` is no enclosure for a difference.
        assert!(!(lo..=hi).contains(&(10.0_f64 - -10.0)));
    }

    #[test]
    fn test_disjoint_regimes_demote_and_overlapping_regimes_intersect() {
        let a = Color::Validated {
            regime: regime(1.0, 2.0),
            dataset: "qaqc-a".to_string(),
        };
        let b = Color::Validated {
            regime: regime(3.0, 4.0),
            dataset: "qaqc-b".to_string(),
        };
        let c = Color::Validated {
            regime: regime(1.5, 4.0),
            dataset: "qaqc-c".to_string(),
        };
        assert!(matches!(
            compose(&a, &b, IntervalOp::Add),
            Color::Estimated { dispersion, .. } if dispersion.is_infinite()
        ));
        let Color::Validated { regime: joint, .. } = compose(&a, &c, IntervalOp::Add) else {
            panic!("overlapping regimes stay Validated");
        };
        assert_eq!(joint.bounds()["feed_grade"], (1.5, 2.0));
    }

    #[test]
    fn test_malformed_payloads_demote_and_long_identities_stay_bounded() {
        let inverted = Color::Verified { lo: 2.0, hi: 1.0 };
        let good = Color::Verified { lo: 1.0, hi: 2.0 };
        let composed = compose(&inverted, &good, IntervalOp::Add);
        assert!(matches!(
            composed,
            Color::Estimated { dispersion, .. } if dispersion.is_infinite()
        ));
        assert_eq!(validate_color_payload(&composed), Ok(()));

        let long = Color::Estimated {
            estimator: "m".repeat(200),
            dispersion: 0.1,
        };
        let Color::Estimated { estimator, .. } = compose(&long, &long, IntervalOp::Add) else {
            panic!("estimate ⊕ estimate stays Estimated");
        };
        assert!(estimator.len() <= MAX_COLOR_IDENTITY_BYTES);
        assert_eq!(color_identity_reason(&estimator), None);
    }
}
