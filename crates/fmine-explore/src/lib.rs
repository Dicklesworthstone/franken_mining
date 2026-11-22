//! Determinism: BitwisePortable
//! Lodestar K1 exploration deposits, effort records, five-class absence taxonomy, and
//! cover-aware detection-thinning data model (`BD-231`, plan §11.3–§11.4).
//!
//! Zero bare `f64` parameters cross any public signature (L-6): all coordinates, cover depths,
//! probabilities, and thinning hazards use exact `fmine_units::ExactQty` and `fmine_crs::CrsId`.

pub mod absence;
pub mod model;

use core::fmt;

use fmine_crs::CrsId;
use fmine_evidence::{Refusal, RefusalCode};
use fmine_units::UnitsError;

/// Typed errors across `fmine-explore` data model validation and absence classification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExploreError {
    /// `G-CRS-1`: attempted to intersect or compare geometry across different `CrsId`s.
    CrsMismatch { left: CrsId, right: CrsId },
    /// Probability simplex (including `k*` novel-type slot) failed non-negativity or sum = 1.
    InvalidSimplex { detail: String },
    /// Invalid physical depth, mark, time interval, elicitation order, or duplicate identifier.
    InvalidRecord { field: &'static str, detail: String },
    /// Missing required `SourceAnchor`, locator, or aggregation rule identifier.
    MissingProvenance { field: &'static str, detail: String },
    /// Exact rational arithmetic overflow.
    Overflow,
}

impl ExploreError {
    /// Map any exploration error to its registered `FMINE-REF-*` refusal.
    #[must_use]
    pub fn to_refusal(&self) -> Refusal {
        match self {
            Self::CrsMismatch { .. } => Refusal::new(
                RefusalCode::CrsUnregisteredOrIncompatible106,
                self.to_string(),
            ),
            Self::MissingProvenance { .. } => {
                Refusal::new(RefusalCode::UndeclaredUnitBasisOrTz107, self.to_string())
            }
            Self::InvalidSimplex { .. } | Self::InvalidRecord { .. } | Self::Overflow => {
                Refusal::new(RefusalCode::OutsidePhysicalDomain109, self.to_string())
            }
        }
    }
}

impl From<UnitsError> for ExploreError {
    fn from(value: UnitsError) -> Self {
        match value {
            UnitsError::Overflow => Self::Overflow,
            other => Self::InvalidRecord {
                field: "exact_qty",
                detail: other.to_string(),
            },
        }
    }
}

impl fmt::Display for ExploreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CrsMismatch { left, right } => {
                write!(f, "G-CRS-1 CRS mismatch ({left:?} vs {right:?})")
            }
            Self::InvalidSimplex { detail } => write!(f, "invalid type simplex: {detail}"),
            Self::InvalidRecord { field, detail } => {
                write!(f, "invalid explore record ({field}): {detail}")
            }
            Self::MissingProvenance { field, detail } => {
                write!(f, "missing provenance ({field}): {detail}")
            }
            Self::Overflow => write!(f, "exact rational overflow in explore model"),
        }
    }
}

impl std::error::Error for ExploreError {}
