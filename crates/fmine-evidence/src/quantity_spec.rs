//! Stable-Rust `QuantitySpec` shim replacing `frankensim`'s nightly `fs-qty` (`BD-037`, plan §13.9, §27.1).
//!
//! `fs-evidence` uses `fs_qty::QuantitySpec` to distinguish dimension-compatible quantities that
//! carry incompatible semantic bases (for example dry mass vs wet mass, dry-basis vs wet-basis
//! moisture, or mass fraction vs stoichiometric concentration). Because `fs-qty` requires nightly
//! const-generic features (and is explicitly NOT lifted per §27.1), this module provides the
//! stable-Rust `QuantitySpec` descriptor with canonical byte encoding and `ValidityDomain` axis
//! typing.

use core::fmt;
use std::collections::BTreeMap;

use crate::color::{Color, Demotion, check_regime, demotion_estimator_identity};

/// Canonical fixed byte length of an encoded [`QuantitySpec`].
pub const QUANTITY_SPEC_ENCODED_LEN: usize = 10;
/// Current canonical encoding version for [`QuantitySpec`].
pub const QUANTITY_SPEC_ENCODING_VERSION: u8 = 1;

/// Six SI base dimension exponents `[L, M, T, Θ, I, N]` (length, mass, time, temperature,
/// electric current, amount of substance), matching `fs-qty::Dims`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Dims(pub [i8; 6]);

impl Dims {
    pub const DIMENSIONLESS: Self = Self([0, 0, 0, 0, 0, 0]);
    pub const LENGTH: Self = Self([1, 0, 0, 0, 0, 0]);
    pub const AREA: Self = Self([2, 0, 0, 0, 0, 0]);
    pub const VOLUME: Self = Self([3, 0, 0, 0, 0, 0]);
    pub const MASS: Self = Self([0, 1, 0, 0, 0, 0]);
    pub const TIME: Self = Self([0, 0, 1, 0, 0, 0]);
    pub const TEMPERATURE: Self = Self([0, 0, 0, 1, 0, 0]);
    pub const AMOUNT: Self = Self([0, 0, 0, 0, 0, 1]);
    pub const DENSITY: Self = Self([-3, 1, 0, 0, 0, 0]);
    pub const PRESSURE: Self = Self([-1, 1, -2, 0, 0, 0]);
    pub const ENERGY: Self = Self([2, 1, -2, 0, 0, 0]);
    pub const POWER: Self = Self([2, 1, -3, 0, 0, 0]);
}

/// Mining and engineering semantic kinds that refine raw physical dimensions (`BD-037`, §25).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum SemanticKind {
    /// Dry solid mass (`Dry` basis).
    DryMass = 1,
    /// Wet mass including pore moisture (`Wet` basis).
    WetMass = 2,
    /// Dimensionless mass ratio / ore grade (`g/t`, `wt%`, `ppm`).
    GradeMassRatio = 3,
    /// Dimensionless metallurgical recovery fraction in `[0, 1]`.
    RecoveryFraction = 4,
    /// Moisture expressed as `water / dry_mass` (may exceed 1).
    MoistureDryBasis = 5,
    /// Moisture expressed as `water / wet_mass` in `[0, 1)`.
    MoistureWetBasis = 6,
    /// In-situ / bulk solid density (`t/m³`).
    BulkDensity = 7,
    /// Absolute thermodynamic temperature (K).
    TemperatureKelvin = 8,
    /// Static gauge/absolute pressure (Pa, kPa, MPa).
    Pressure = 9,
    /// Specific or total energy (`kWh`, `GJ`, `J`).
    Energy = 10,
    /// Length / distance / drillhole depth (`m`).
    Length = 11,
    /// Volume (`m³`).
    Volume = 12,
    /// Aqueous concentration (`mg/L`, mass per volume).
    MassConcentration = 13,
}

impl SemanticKind {
    /// Physical SI base dimensions implied by this semantic kind.
    #[must_use]
    pub const fn expected_dims(self) -> Dims {
        match self {
            Self::DryMass | Self::WetMass => Dims::MASS,
            Self::GradeMassRatio
            | Self::RecoveryFraction
            | Self::MoistureDryBasis
            | Self::MoistureWetBasis => Dims::DIMENSIONLESS,
            Self::BulkDensity | Self::MassConcentration => Dims::DENSITY,
            Self::TemperatureKelvin => Dims::TEMPERATURE,
            Self::Pressure => Dims::PRESSURE,
            Self::Energy => Dims::ENERGY,
            Self::Length => Dims::LENGTH,
            Self::Volume => Dims::VOLUME,
        }
    }

    const fn from_tag(tag: u8) -> Option<Self> {
        match tag {
            1 => Some(Self::DryMass),
            2 => Some(Self::WetMass),
            3 => Some(Self::GradeMassRatio),
            4 => Some(Self::RecoveryFraction),
            5 => Some(Self::MoistureDryBasis),
            6 => Some(Self::MoistureWetBasis),
            7 => Some(Self::BulkDensity),
            8 => Some(Self::TemperatureKelvin),
            9 => Some(Self::Pressure),
            10 => Some(Self::Energy),
            11 => Some(Self::Length),
            12 => Some(Self::Volume),
            13 => Some(Self::MassConcentration),
            _ => None,
        }
    }
}

/// Stable-Rust identity descriptor mirroring `fs_qty::QuantitySpec` (`BD-037`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum QuantitySpec {
    /// Six-base SI dimensions with no semantic-kind claim.
    Dimensional(Dims),
    /// Exact semantic kind whose physical dimensions are derived from [`SemanticKind::expected_dims`].
    Semantic(SemanticKind),
}

/// Decode refusal for [`QuantitySpec::from_canonical_bytes`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuantitySpecDecodeError {
    Length { actual: usize },
    Version { actual: u8 },
    Variant { actual: u8 },
    InvalidSemanticTag { actual: u8 },
    NonZeroPadding { index: usize, byte: u8 },
}

impl fmt::Display for QuantitySpecDecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Length { actual } => write!(
                f,
                "QuantitySpec token must be {QUANTITY_SPEC_ENCODED_LEN} bytes; found {actual}"
            ),
            Self::Version { actual } => write!(f, "unsupported QuantitySpec version {actual}"),
            Self::Variant { actual } => write!(f, "unknown QuantitySpec variant tag {actual}"),
            Self::InvalidSemanticTag { actual } => {
                write!(f, "unknown QuantitySpec semantic tag {actual}")
            }
            Self::NonZeroPadding { index, byte } => {
                write!(f, "non-zero padding byte 0x{byte:02x} at index {index}")
            }
        }
    }
}

impl std::error::Error for QuantitySpecDecodeError {}

impl QuantitySpec {
    /// Construct a dimension-only descriptor.
    #[must_use]
    pub const fn dimensional(dims: Dims) -> Self {
        Self::Dimensional(dims)
    }

    /// Construct a semantic descriptor.
    #[must_use]
    pub const fn semantic(kind: SemanticKind) -> Self {
        Self::Semantic(kind)
    }

    /// Physical SI base dimensions of this quantity descriptor.
    #[must_use]
    pub const fn dims(self) -> Dims {
        match self {
            Self::Dimensional(d) => d,
            Self::Semantic(k) => k.expected_dims(),
        }
    }

    /// Whether `self` and `other` are strictly compatible as axis/quantity descriptors.
    ///
    /// Two `Semantic` specs must have the exact same [`SemanticKind`] (so `DryMass` is never
    /// compatible with `WetMass`, and `MoistureDryBasis` is never compatible with `MoistureWetBasis`).
    /// A `Dimensional` spec is compatible only with another `Dimensional` spec of identical [`Dims`].
    #[must_use]
    pub const fn is_compatible_with(self, other: Self) -> bool {
        match (self, other) {
            (Self::Dimensional(a), Self::Dimensional(b)) => {
                a.0[0] == b.0[0]
                    && a.0[1] == b.0[1]
                    && a.0[2] == b.0[2]
                    && a.0[3] == b.0[3]
                    && a.0[4] == b.0[4]
                    && a.0[5] == b.0[5]
            }
            (Self::Semantic(a), Self::Semantic(b)) => (a as u8) == (b as u8),
            _ => false,
        }
    }

    /// Maps common registered `registries/units.toml` symbols to a canonical [`QuantitySpec`].
    #[must_use]
    pub fn from_unit_symbol(symbol: &str) -> Option<Self> {
        match symbol {
            "m" | "km" | "mm" => Some(Self::Semantic(SemanticKind::Length)),
            "m2" | "ha" => Some(Self::Dimensional(Dims::AREA)),
            "m3" | "L" => Some(Self::Semantic(SemanticKind::Volume)),
            "t" | "kt" | "Mt" | "kg" | "g" | "oz_t" | "koz" | "Moz" | "lb" | "Mlb" => {
                Some(Self::Semantic(SemanticKind::DryMass))
            }
            "g_per_t" | "wt_pct" | "ppm" | "pct" => {
                Some(Self::Semantic(SemanticKind::GradeMassRatio))
            }
            "t_per_m3" | "g_per_cm3" => Some(Self::Semantic(SemanticKind::BulkDensity)),
            "mg_per_L" => Some(Self::Semantic(SemanticKind::MassConcentration)),
            "kWh" | "MWh" | "GJ" | "J" => Some(Self::Semantic(SemanticKind::Energy)),
            "Pa" | "kPa" | "MPa" => Some(Self::Semantic(SemanticKind::Pressure)),
            "K" => Some(Self::Semantic(SemanticKind::TemperatureKelvin)),
            "s" | "min" | "h" | "d" => Some(Self::Dimensional(Dims::TIME)),
            _ => None,
        }
    }

    /// Canonical 10-byte encoding (`[version, variant, payload[8]]`) for content-addressed hashing.
    #[must_use]
    pub const fn canonical_bytes(self) -> [u8; QUANTITY_SPEC_ENCODED_LEN] {
        let mut out = [0u8; QUANTITY_SPEC_ENCODED_LEN];
        out[0] = QUANTITY_SPEC_ENCODING_VERSION;
        match self {
            Self::Dimensional(Dims(d)) => {
                out[1] = 1;
                out[2] = d[0] as u8;
                out[3] = d[1] as u8;
                out[4] = d[2] as u8;
                out[5] = d[3] as u8;
                out[6] = d[4] as u8;
                out[7] = d[5] as u8;
            }
            Self::Semantic(kind) => {
                out[1] = 2;
                out[2] = kind as u8;
            }
        }
        out
    }

    /// Decodes a canonical 10-byte token produced by [`Self::canonical_bytes`].
    pub fn from_canonical_bytes(bytes: &[u8]) -> Result<Self, QuantitySpecDecodeError> {
        if bytes.len() != QUANTITY_SPEC_ENCODED_LEN {
            return Err(QuantitySpecDecodeError::Length {
                actual: bytes.len(),
            });
        }
        if bytes[0] != QUANTITY_SPEC_ENCODING_VERSION {
            return Err(QuantitySpecDecodeError::Version { actual: bytes[0] });
        }
        match bytes[1] {
            1 => {
                for (index, &byte) in bytes.iter().enumerate().skip(8) {
                    if byte != 0 {
                        return Err(QuantitySpecDecodeError::NonZeroPadding { index, byte });
                    }
                }
                Ok(Self::Dimensional(Dims([
                    bytes[2] as i8,
                    bytes[3] as i8,
                    bytes[4] as i8,
                    bytes[5] as i8,
                    bytes[6] as i8,
                    bytes[7] as i8,
                ])))
            }
            2 => {
                for (index, &byte) in bytes.iter().enumerate().skip(3) {
                    if byte != 0 {
                        return Err(QuantitySpecDecodeError::NonZeroPadding { index, byte });
                    }
                }
                let kind = SemanticKind::from_tag(bytes[2])
                    .ok_or(QuantitySpecDecodeError::InvalidSemanticTag { actual: bytes[2] })?;
                Ok(Self::Semantic(kind))
            }
            other => Err(QuantitySpecDecodeError::Variant { actual: other }),
        }
    }
}

/// Checks a [`Color::Validated`] regime when both the regime axes and the current readings carry
/// explicit [`QuantitySpec`] descriptors (`BD-037`).
///
/// Any axis whose supplied reading has an incompatible [`QuantitySpec`] (for example `WetMass`
/// passed to a `DryMass` axis, or `MoistureWetBasis` passed to `MoistureDryBasis`) auto-demotes
/// the colour to [`Color::Estimated`] with `∞` dispersion and returns a [`Demotion`] naming the
/// mismatched axis; compatible readings delegate directly to [`check_regime`].
#[must_use]
pub fn check_regime_typed(
    color: &Color,
    axis_specs: &BTreeMap<String, QuantitySpec>,
    readings: &BTreeMap<String, (f64, QuantitySpec)>,
) -> (Color, Option<Demotion>) {
    if let Color::Validated { regime, dataset } = color {
        for axis in regime.bounds().keys() {
            if let Some(&expected_spec) = axis_specs.get(axis) {
                match readings.get(axis) {
                    Some(&(val, actual_spec)) if !expected_spec.is_compatible_with(actual_spec) => {
                        return (
                            Color::Estimated {
                                estimator: demotion_estimator_identity(dataset, axis),
                                dispersion: f64::INFINITY,
                            },
                            Some(Demotion {
                                dataset: dataset.clone(),
                                axis: axis.clone(),
                                value: val,
                            }),
                        );
                    }
                    _ => {}
                }
            }
        }
    }
    let plain: BTreeMap<String, f64> = readings.iter().map(|(k, &(v, _))| (k.clone(), v)).collect();
    check_regime(color, &plain)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color::ValidityDomain;

    #[test]
    fn test_quantity_spec_shim_distinguishes_semantic_bases_and_roundtrips() {
        let dry = QuantitySpec::semantic(SemanticKind::DryMass);
        let wet = QuantitySpec::semantic(SemanticKind::WetMass);
        assert_eq!(dry.dims(), Dims::MASS);
        assert_eq!(wet.dims(), Dims::MASS);
        // Same physical dimensions, incompatible semantic bases!
        assert!(!dry.is_compatible_with(wet));
        assert!(dry.is_compatible_with(QuantitySpec::from_unit_symbol("t").expect("t")));

        let m_dry = QuantitySpec::semantic(SemanticKind::MoistureDryBasis);
        let m_wet = QuantitySpec::semantic(SemanticKind::MoistureWetBasis);
        assert_eq!(m_dry.dims(), Dims::DIMENSIONLESS);
        assert!(!m_dry.is_compatible_with(m_wet));

        let dim_mass = QuantitySpec::dimensional(Dims::MASS);
        assert!(!dim_mass.is_compatible_with(dry));

        // Canonical 10-byte roundtrip and tamper rejection.
        for spec in [dry, wet, m_dry, m_wet, dim_mass] {
            let enc = spec.canonical_bytes();
            assert_eq!(QuantitySpec::from_canonical_bytes(&enc), Ok(spec));
        }
        let mut bad_pad = dry.canonical_bytes();
        bad_pad[9] = 1;
        assert_eq!(
            QuantitySpec::from_canonical_bytes(&bad_pad),
            Err(QuantitySpecDecodeError::NonZeroPadding { index: 9, byte: 1 })
        );

        // Typed regime check: compatible spec stays Validated, wet-basis reading on dry-basis axis demotes!
        let validated = Color::Validated {
            regime: ValidityDomain::unconstrained().with("moisture", 0.01, 0.20),
            dataset: "lab-moisture-2026".to_string(),
        };
        let specs = BTreeMap::from([("moisture".to_string(), m_dry)]);
        let ok_read = BTreeMap::from([("moisture".to_string(), (0.08, m_dry))]);
        let (ok_col, ok_dem) = check_regime_typed(&validated, &specs, &ok_read);
        assert!(ok_dem.is_none());
        assert_eq!(ok_col, validated);

        let bad_read = BTreeMap::from([("moisture".to_string(), (0.08, m_wet))]);
        let (bad_col, bad_dem) = check_regime_typed(&validated, &specs, &bad_read);
        assert!(bad_dem.is_some());
        assert!(matches!(
            bad_col,
            Color::Estimated { dispersion, .. } if dispersion.is_infinite()
        ));
    }
}
