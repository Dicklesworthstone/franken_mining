//! What variography refuses, and the registered refusal each error stands for.
//!
//! Determinism: BitwisePortable (no floating-point arithmetic).

use std::fmt;

use fmine_evidence::{Refusal, RefusalCode};

/// A refused variography input or fit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GeostatError {
    /// A parameter or value outside its domain (`FMINE-REF-109`).
    Invalid {
        /// The parameter, for example `lags.width_m`.
        field: String,
        /// What is wrong with it.
        detail: String,
    },
    /// Samples in more than one CRS (`FMINE-REF-106`).
    CrsMismatch {
        /// The first sample set's CRS.
        first: String,
        /// The CRS that differs from it.
        other: String,
    },
    /// A value or parameter the data do not determine: a censored value under [`crate::samples::Censored::Refuse`],
    /// or a fit with fewer points than parameters or a free range no direction sees (`FMINE-REF-202`).
    NotObservable {
        /// What cannot be determined.
        detail: String,
    },
    /// A fit that did not converge within its evaluation budget (`FMINE-REF-207`).
    NoConvergence {
        /// Where it stopped.
        detail: String,
    },
}

impl GeostatError {
    pub(crate) fn invalid(field: impl Into<String>, detail: impl Into<String>) -> Self {
        Self::Invalid {
            field: field.into(),
            detail: detail.into(),
        }
    }

    pub(crate) fn not_observable(detail: impl Into<String>) -> Self {
        Self::NotObservable {
            detail: detail.into(),
        }
    }

    /// The refusal this error stands for.
    #[must_use]
    pub fn to_refusal(&self) -> Refusal {
        let code = match self {
            Self::Invalid { .. } => RefusalCode::OutsidePhysicalDomain109,
            Self::CrsMismatch { .. } => RefusalCode::CrsUnregisteredOrIncompatible106,
            Self::NotObservable { .. } => RefusalCode::NotObservable202,
            Self::NoConvergence { .. } => RefusalCode::NumericalCertificateFailed207,
        };
        Refusal::new(code, self.to_string())
    }
}

impl fmt::Display for GeostatError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Invalid { field, detail } => write!(f, "{field}: {detail}"),
            Self::CrsMismatch { first, other } => {
                write!(f, "samples in {first} and in {other}; one CRS is required")
            }
            Self::NotObservable { detail } | Self::NoConvergence { detail } => f.write_str(detail),
        }
    }
}

impl std::error::Error for GeostatError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_each_error_names_its_registered_refusal() {
        let cases = [
            (GeostatError::invalid("lags.count", "zero"), "FMINE-REF-109"),
            (
                GeostatError::CrsMismatch {
                    first: "EPSG:32750".to_string(),
                    other: "EPSG:32751".to_string(),
                },
                "FMINE-REF-106",
            ),
            (GeostatError::not_observable("minor range"), "FMINE-REF-202"),
            (
                GeostatError::NoConvergence {
                    detail: "budget".to_string(),
                },
                "FMINE-REF-207",
            ),
        ];
        for (error, code) in cases {
            let refusal = error.to_refusal();
            assert_eq!(refusal.code, code, "{error}");
            assert!(
                refusal.what.ends_with(&error.to_string()),
                "{}",
                refusal.what
            );
        }
    }
}
