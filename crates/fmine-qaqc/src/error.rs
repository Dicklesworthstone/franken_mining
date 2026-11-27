//! Typed refusal errors for `fmine-qaqc` (`plan §8.4, §12`).
//! Determinism: BitwisePortable (pure error values mapped to registered `FMINE-REF-*` codes).

use std::fmt;

use fmine_evidence::{Refusal, RefusalCode};
use fmine_numerics::eproc::EProcError;

/// Why a Corebox QA/QC calculation or evaluation refused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum QaqcError {
    /// A parameter or value outside its physical or statistical domain (`FMINE-REF-109`).
    InvalidParameter {
        /// The field or parameter name.
        field: String,
        /// Why it was refused.
        detail: String,
    },
    /// Attempted DL/2 or constant substitution of a below-detection result (`G-CENS`, `FMINE-REF-109`).
    SubstitutionRefused {
        /// What substitution was attempted.
        detail: String,
    },
    /// Attempted to combine or compare results from different assay methods or analytes without
    /// an explicit `MeasurandMapping` (`PO-CORE-3`, `FMINE-REF-105`).
    MethodOrAnalyteMismatch {
        /// Expected analyte/method.
        expected: String,
        /// Conflicting analyte/method.
        actual: String,
    },
    /// A duplicate sample's mass or support does not match its original (`E374`, `FMINE-REF-102`).
    DuplicateSupportMismatch {
        /// Original sample id.
        original_sample_id: String,
        /// Duplicate sample id.
        duplicate_sample_id: String,
        /// Why support/mass mismatched.
        detail: String,
    },
    /// Stage decomposition or attribution is not identifiable from the supplied controls
    /// (for example `FieldDuplicateAbsent`, C4 P12, `FMINE-REF-104`).
    NonIdentifiable {
        /// What cannot be identified.
        detail: String,
    },
    /// Insufficient observations or pairs for the requested statistic (`FMINE-REF-201`).
    InsufficientData {
        /// The statistic or monitor name.
        context: String,
        /// Required minimum count.
        required: usize,
        /// Observed count.
        actual: usize,
    },
    /// Unusable or missing source record / censored observation where uncensored input is required
    /// (`FMINE-REF-208`).
    UnusableInput {
        /// Context or sample id.
        context: String,
        /// What was missing or unusable.
        detail: String,
    },
    /// Numerical estimation (for example censored MLE) failed to converge or became singular
    /// (`FMINE-REF-207`).
    NumericalFailure {
        /// Algorithm context.
        context: String,
        /// Diagnostic detail.
        detail: String,
    },
    /// Underlying e-process rejected an invalid domain input (`FMINE-REF-109`).
    EProcess {
        /// Detail from `fmine_numerics::eproc`.
        detail: String,
    },
}

impl QaqcError {
    pub(crate) fn invalid(field: &str, detail: impl Into<String>) -> Self {
        Self::InvalidParameter {
            field: field.to_string(),
            detail: detail.into(),
        }
    }

    /// Maps this error to its registered `FMINE-REF-*` refusal.
    #[must_use]
    pub fn to_refusal(&self) -> Refusal {
        let code = match self {
            Self::InvalidParameter { .. }
            | Self::SubstitutionRefused { .. }
            | Self::EProcess { .. } => RefusalCode::OutsidePhysicalDomain109,
            Self::MethodOrAnalyteMismatch { .. } => RefusalCode::BasisMismatch105,
            Self::DuplicateSupportMismatch { .. } => RefusalCode::SupportMismatch102,
            Self::NonIdentifiable { .. } => RefusalCode::NonIdentifiableAttribution104,
            Self::InsufficientData { .. } => RefusalCode::NoClaimInDomain201,
            Self::UnusableInput { .. } => RefusalCode::SourceCellUnusable208,
            Self::NumericalFailure { .. } => RefusalCode::NumericalCertificateFailed207,
        };
        Refusal::new(code, self.to_string())
    }
}

impl From<EProcError> for QaqcError {
    fn from(err: EProcError) -> Self {
        Self::EProcess {
            detail: err.to_string(),
        }
    }
}

impl fmt::Display for QaqcError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidParameter { field, detail } => write!(f, "{field}: {detail}"),
            Self::SubstitutionRefused { detail } => {
                write!(f, "G-CENS substitution refused: {detail}")
            }
            Self::MethodOrAnalyteMismatch { expected, actual } => write!(
                f,
                "PO-CORE-3 measurand mismatch: expected {expected}, got {actual}"
            ),
            Self::DuplicateSupportMismatch {
                original_sample_id,
                duplicate_sample_id,
                detail,
            } => write!(
                f,
                "duplicate '{duplicate_sample_id}' vs original '{original_sample_id}': {detail}"
            ),
            Self::NonIdentifiable { detail } => write!(f, "non-identifiable: {detail}"),
            Self::InsufficientData {
                context,
                required,
                actual,
            } => write!(
                f,
                "{context}: requires at least {required} observations, got {actual}"
            ),
            Self::UnusableInput { context, detail } => write!(f, "{context}: {detail}"),
            Self::NumericalFailure { context, detail } => {
                write!(f, "{context} numerical failure: {detail}")
            }
            Self::EProcess { detail } => write!(f, "e-process error: {detail}"),
        }
    }
}

impl std::error::Error for QaqcError {}
