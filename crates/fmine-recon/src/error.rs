//! Typed refusal errors for `fmine-recon` (plan §8.4, §20.2, §20.6, §20.11).
//! Determinism: BitwisePortable (pure error values mapped to registered `FMINE-REF-*` codes).

use std::fmt;

use fmine_evidence::{Refusal, RefusalCode};
use fmine_flow::FlowError;

/// Why a Touchstone reconciliation or gross-error operation refused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReconError {
    /// Period failed gross-error / conservation gate at commit (`INV-005`, `FMINE-REF-101`).
    ConservationGrossError {
        /// Diagnostic summary including top suspect measurement(s).
        detail: String,
    },
    /// Stockpile decoupling or missing inventory delta refused (`G-TOUCH-3`, `FMINE-REF-102`).
    StockpileDecouplingRefused {
        /// Node or path detail.
        detail: String,
    },
    /// Direct reconciliation of intensive grades refused (`G-TOUCH-1`, `FMINE-REF-103`).
    DirectGradeReconciliationRefused {
        /// What grade reconciliation was attempted.
        detail: String,
    },
    /// Attribution or unmeasured stream inference is non-identifiable (`G-TOUCH-2`, `FMINE-REF-104`).
    NonIdentifiable {
        /// Correlated bias pair or unobservable stream detail.
        detail: String,
    },
    /// F-factor rendered or certified without its definition version or interval (`G-TOUCH-4`, `FMINE-REF-107`).
    MissingDefinitionVersion {
        /// Diagnostic detail.
        detail: String,
    },
    /// Factor applied as a direct multiplier to a future estimate instead of a Strata bias prior (`G-TOUCH-5`, `FMINE-REF-109`).
    FactorMultiplierRefused {
        /// Diagnostic detail.
        detail: String,
    },
    /// Tampered, malformed, or digest-mismatched `ReconciliationCertificate` (`PO-TOUCH-6`, `FMINE-REF-505`).
    CertificateTampered {
        /// Diagnostic detail.
        detail: String,
    },
    /// Parameter or measurement outside physical or statistical domain (`FMINE-REF-109`).
    InvalidParameter {
        /// Field name.
        field: String,
        /// Reason for refusal.
        detail: String,
    },
    /// Insufficient conservation equations or degrees of freedom (`FMINE-REF-201`).
    InsufficientRedundancy {
        /// Operation context.
        context: String,
        /// Reason for refusal.
        detail: String,
    },
    /// Numerical audit, factorization, or convergence failure (`FMINE-REF-207`).
    NumericalFailure {
        /// Diagnostic detail.
        detail: String,
    },
    /// Underlying `fmine-flow` graph or conservation error.
    FlowGraph(FlowError),
    /// Upstream refusal emitted by `fmine-numerics::reconciliation`.
    UpstreamRefusal(Refusal),
}

impl ReconError {
    pub(crate) fn invalid(field: &str, detail: impl Into<String>) -> Self {
        Self::InvalidParameter {
            field: field.to_string(),
            detail: detail.into(),
        }
    }

    /// Maps this error to its registered `FMINE-REF-*` refusal.
    #[must_use]
    pub fn to_refusal(&self) -> Refusal {
        match self {
            Self::ConservationGrossError { .. } => {
                Refusal::new(RefusalCode::Conservation101, self.to_string())
            }
            Self::StockpileDecouplingRefused { .. } => {
                Refusal::new(RefusalCode::SupportMismatch102, self.to_string())
            }
            Self::DirectGradeReconciliationRefused { .. } => {
                Refusal::new(RefusalCode::NonAdditiveAverage103, self.to_string())
            }
            Self::NonIdentifiable { .. } => {
                Refusal::new(RefusalCode::NonIdentifiableAttribution104, self.to_string())
            }
            Self::MissingDefinitionVersion { .. } => {
                Refusal::new(RefusalCode::UndeclaredUnitBasisOrTz107, self.to_string())
            }
            Self::FactorMultiplierRefused { .. } => {
                Refusal::new(RefusalCode::OutsidePhysicalDomain109, self.to_string())
            }
            Self::CertificateTampered { .. } => {
                Refusal::new(RefusalCode::DigestMismatch505, self.to_string())
            }
            Self::InvalidParameter { .. } => {
                Refusal::new(RefusalCode::OutsidePhysicalDomain109, self.to_string())
            }
            Self::InsufficientRedundancy { .. } => {
                Refusal::new(RefusalCode::NoClaimInDomain201, self.to_string())
            }
            Self::NumericalFailure { .. } => {
                Refusal::new(RefusalCode::NumericalCertificateFailed207, self.to_string())
            }
            Self::FlowGraph(err) => err.to_refusal(),
            Self::UpstreamRefusal(refusal) => refusal.clone(),
        }
    }
}

impl From<FlowError> for ReconError {
    fn from(err: FlowError) -> Self {
        Self::FlowGraph(err)
    }
}

impl From<Refusal> for ReconError {
    fn from(refusal: Refusal) -> Self {
        Self::UpstreamRefusal(refusal)
    }
}

impl fmt::Display for ReconError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ConservationGrossError { detail } => {
                write!(f, "gross-error conservation gate refused commit: {detail}")
            }
            Self::StockpileDecouplingRefused { detail } => {
                write!(f, "G-TOUCH-3 stockpile decoupling refused: {detail}")
            }
            Self::DirectGradeReconciliationRefused { detail } => {
                write!(f, "G-TOUCH-1 direct grade reconciliation refused: {detail}")
            }
            Self::NonIdentifiable { detail } => {
                write!(f, "G-TOUCH-2 non-identifiable attribution/flow: {detail}")
            }
            Self::MissingDefinitionVersion { detail } => {
                write!(f, "G-TOUCH-4 missing F-factor definition version: {detail}")
            }
            Self::FactorMultiplierRefused { detail } => {
                write!(f, "G-TOUCH-5 factor multiplier refused: {detail}")
            }
            Self::CertificateTampered { detail } => {
                write!(f, "reconciliation certificate integrity failed: {detail}")
            }
            Self::InvalidParameter { field, detail } => write!(f, "{field}: {detail}"),
            Self::InsufficientRedundancy { context, detail } => {
                write!(f, "{context}: insufficient redundancy: {detail}")
            }
            Self::NumericalFailure { detail } => {
                write!(f, "reconciliation numerical failure: {detail}")
            }
            Self::FlowGraph(err) => write!(f, "flow graph error: {err}"),
            Self::UpstreamRefusal(refusal) => write!(f, "{}: {}", refusal.code, refusal.what),
        }
    }
}

impl std::error::Error for ReconError {}
