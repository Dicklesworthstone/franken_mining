//! Determinism: BitwisePortable
//! Synthetic-truth world generator and sealed scoring harness (`BD-127`, plan §33.3, `GATE-GEN-01`).

pub mod biases;
pub mod cu_au_world;
pub mod det_families;
pub mod dispatch;
pub mod drillholes;
pub mod fa_book;
pub mod geology;
pub mod harness;
pub mod junior;
pub mod metamorphic;
pub mod mill_balance;
pub mod origin_coverage;
pub mod pilot_card;
pub mod pilot_power;
pub mod plant_twin;
pub mod review;
pub mod sbc;
pub mod seal;
pub mod worlds;

use core::fmt;

use fmine_evidence::{Refusal, RefusalCode};

/// Errors raised by synthetic world configuration, seal enforcement, or scoring.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GenError {
    /// Seal capability mismatch (`GATE-GEN-01`).
    Seal(seal::SealError),
    /// Invalid synthetic world configuration parameter.
    InvalidConfig { field: &'static str, detail: String },
    /// Prediction vector did not match world period count or contained non-finite bounds.
    InvalidPrediction { detail: String },
    /// Confounded bias pair in an identifiability probe cannot be separated (`FMINE-REF-104`).
    NonIdentifiableProbe { detail: String },
}

impl GenError {
    /// Maps generator/harness errors to registered `FMINE-REF-*` codes.
    #[must_use]
    pub fn to_refusal(&self) -> Refusal {
        match self {
            Self::Seal(_) => Refusal::new(RefusalCode::MnpiWall301, self.to_string()),
            Self::NonIdentifiableProbe { .. } => {
                Refusal::new(RefusalCode::NonIdentifiableAttribution104, self.to_string())
            }
            Self::InvalidConfig { .. } | Self::InvalidPrediction { .. } => {
                Refusal::new(RefusalCode::OutsidePhysicalDomain109, self.to_string())
            }
        }
    }
}

impl From<seal::SealError> for GenError {
    fn from(value: seal::SealError) -> Self {
        Self::Seal(value)
    }
}

impl fmt::Display for GenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Seal(e) => write!(f, "sealed-truth error: {e}"),
            Self::InvalidConfig { field, detail } => {
                write!(f, "invalid synthetic world config {field}: {detail}")
            }
            Self::InvalidPrediction { detail } => write!(f, "invalid engine prediction: {detail}"),
            Self::NonIdentifiableProbe { detail } => {
                write!(f, "FMINE-REF-104 non-identifiable probe: {detail}")
            }
        }
    }
}

impl std::error::Error for GenError {}
