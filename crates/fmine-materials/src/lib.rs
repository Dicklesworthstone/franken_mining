//! Determinism: BitwisePortable
//! Strata K0 sealed geomaterial corpora (`frankensim.matdb-manifest.v1`) and L3 empirical
//! correlation cards (`BD-241`, plan §13.10).
//!
//! Zero bare `f64` parameters cross any public signature (L-6): all material parameters,
//! intervals, and correlation coefficients use exact `fmine_units::ExactQty`.

pub mod cards;
pub mod corpus;

use core::fmt;

use fmine_evidence::{Refusal, RefusalCode};
use fmine_units::UnitsError;

/// Typed errors across `fmine-materials` corpus sealing, lookup, and correlation card evaluation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MaterialsError {
    /// Sealed corpus manifest bytes or digest no longer match (`StoreError::CorpusChanged`, §13.10).
    CorpusChanged {
        corpus_id: String,
        expected_hex: String,
        actual_hex: String,
    },
    /// Requested material id or property key is absent from the sealed corpus (`§13.10`).
    NoClaimInDomain { material_id: String, detail: String },
    /// Correlation card input lies outside its declared validity box (`FMINE-REF-203`).
    OutsideValidityBox { card_id: String, detail: String },
    /// Reader confidentiality clearance is insufficient for the requested entry (`INV-007`).
    ConfidentialityDenied { material_id: String, detail: String },
    /// Input unit does not match the card's declared input unit (`FMINE-REF-105`).
    UnitMismatch { expected: String, actual: String },
    /// Invalid material specification, duplicate id, missing provenance, or invalid evidence class.
    InvalidSpec { field: &'static str, detail: String },
    /// Exact rational arithmetic overflow.
    Overflow,
}

impl MaterialsError {
    /// Map any materials error to its registered `FMINE-REF-*` refusal.
    #[must_use]
    pub fn to_refusal(&self) -> Refusal {
        match self {
            Self::CorpusChanged { .. } => {
                Refusal::new(RefusalCode::DigestMismatch505, self.to_string())
            }
            Self::NoClaimInDomain { .. } => {
                Refusal::new(RefusalCode::NoClaimInDomain201, self.to_string())
            }
            Self::OutsideValidityBox { .. } => {
                Refusal::new(RefusalCode::OutsideAoa203, self.to_string())
            }
            Self::ConfidentialityDenied { .. } => {
                Refusal::new(RefusalCode::MnpiWall301, self.to_string())
            }
            Self::UnitMismatch { .. } => {
                Refusal::new(RefusalCode::BasisMismatch105, self.to_string())
            }
            Self::InvalidSpec { .. } | Self::Overflow => {
                Refusal::new(RefusalCode::OutsidePhysicalDomain109, self.to_string())
            }
        }
    }
}

impl From<UnitsError> for MaterialsError {
    fn from(value: UnitsError) -> Self {
        match value {
            UnitsError::Overflow => Self::Overflow,
            other => Self::InvalidSpec {
                field: "exact_qty",
                detail: other.to_string(),
            },
        }
    }
}

impl fmt::Display for MaterialsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CorpusChanged {
                corpus_id,
                expected_hex,
                actual_hex,
            } => write!(
                f,
                "CorpusChanged: sealed corpus '{corpus_id}' digest mismatch (expected {expected_hex}, actual {actual_hex})"
            ),
            Self::NoClaimInDomain {
                material_id,
                detail,
            } => write!(
                f,
                "no claim in domain for material '{material_id}': {detail}"
            ),
            Self::OutsideValidityBox { card_id, detail } => {
                write!(
                    f,
                    "correlation card '{card_id}' outside validity box: {detail}"
                )
            }
            Self::ConfidentialityDenied {
                material_id,
                detail,
            } => write!(
                f,
                "confidentiality wall denied access to material '{material_id}': {detail}"
            ),
            Self::UnitMismatch { expected, actual } => {
                write!(f, "unit mismatch: expected '{expected}', got '{actual}'")
            }
            Self::InvalidSpec { field, detail } => {
                write!(f, "invalid material spec ({field}): {detail}")
            }
            Self::Overflow => write!(f, "exact rational overflow in materials kernel"),
        }
    }
}

impl std::error::Error for MaterialsError {}
