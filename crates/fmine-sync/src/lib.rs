//! Determinism: BitwisePortable
//!
//! Bedrock S1 offline-first site-to-enterprise replication (`fmine-sync`, plan §26, `BD-102`–`BD-108`):
//! - [`manifest`]: chunk want/have manifest anti-entropy and `GB-36` size measurement (`BD-104`);
//! - [`transport`]: QUIC/TLS 1.3 ATP transport policy with unconditional plaintext ban (`BD-105`, `S3(d)`);
//! - [`version_vector`]: per-cell watermarks and lag/staleness detection (`BD-102`, `F1`);
//! - [`sync_cert`]: RFC 6962 MMR prefix-consistency verification (`BD-103`, `PO-BED-2`);
//! - [`residency_plan`]: pre-transfer residency enforcement (`BD-107`, `FMINE-REF-303`);
//! - [`gluing`]: cross-cell anchor publication gluing gate (`BD-108`, `INV-012`, `F12`);
//! - [`receipt`]: durable `OB-SEAL` chunk receipts and hard-kill resume (`BD-106`, `PO-BED-3`, `S3(e)`).

pub mod gluing;
pub mod manifest;
pub mod receipt;
pub mod residency_plan;
pub mod sync_cert;
pub mod transport;
pub mod version_vector;

use std::fmt;

use fmine_authz::AuthzError;
use fmine_core::codec::CodecError;
use fmine_evidence::{Refusal, RefusalCode};
use fmine_ledger::mmr::MmrError;
use fmine_object::cas::CasError;

/// Typed errors across `fmine-sync` protocols.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyncError {
    Codec(CodecError),
    Cas(CasError),
    Authz(AuthzError),
    Mmr(MmrError),
    InvalidField { field: &'static str, detail: String },
    PlaintextBanned { scheme: &'static str },
    TlsPolicyViolation { detail: String },
    SyncCertInvalid { detail: String },
    StaleOrDiverged { cell_id: String, detail: String },
    IncompleteTransfer { missing_chunks: usize },
}

impl SyncError {
    /// Maps any sync error to its registered `FMINE-REF-*` refusal.
    #[must_use]
    pub fn to_refusal(&self) -> Refusal {
        match self {
            Self::Cas(e) => e.to_refusal(),
            Self::Authz(e) => e.to_refusal(),
            Self::StaleOrDiverged { .. } => {
                Refusal::new(RefusalCode::StaleAnchor502, self.to_string())
            }
            Self::IncompleteTransfer { .. } => {
                Refusal::new(RefusalCode::InputMissingOrUnreadable506, self.to_string())
            }
            Self::Codec(_)
            | Self::Mmr(_)
            | Self::InvalidField { .. }
            | Self::PlaintextBanned { .. }
            | Self::TlsPolicyViolation { .. }
            | Self::SyncCertInvalid { .. } => {
                Refusal::new(RefusalCode::DigestMismatch505, self.to_string())
            }
        }
    }
}

impl From<CodecError> for SyncError {
    fn from(value: CodecError) -> Self {
        Self::Codec(value)
    }
}

impl From<CasError> for SyncError {
    fn from(value: CasError) -> Self {
        Self::Cas(value)
    }
}

impl From<AuthzError> for SyncError {
    fn from(value: AuthzError) -> Self {
        Self::Authz(value)
    }
}

impl From<MmrError> for SyncError {
    fn from(value: MmrError) -> Self {
        Self::Mmr(value)
    }
}

impl fmt::Display for SyncError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Codec(e) => write!(f, "sync codec error: {e}"),
            Self::Cas(e) => write!(f, "sync CAS error: {e}"),
            Self::Authz(e) => write!(f, "sync authorization error: {e}"),
            Self::Mmr(e) => write!(f, "sync MMR verification error: {e}"),
            Self::InvalidField { field, detail } => write!(f, "invalid {field}: {detail}"),
            Self::PlaintextBanned { scheme } => {
                write!(
                    f,
                    "plaintext transport '{scheme}' is banned (plan §31.4 S3(d)); QUIC/TLS 1.3 over ATP is mandatory"
                )
            }
            Self::TlsPolicyViolation { detail } => write!(f, "TLS policy violation: {detail}"),
            Self::SyncCertInvalid { detail } => write!(f, "invalid SyncCert: {detail}"),
            Self::StaleOrDiverged { cell_id, detail } => {
                write!(f, "cell '{cell_id}' stale or diverged: {detail}")
            }
            Self::IncompleteTransfer { missing_chunks } => {
                write!(
                    f,
                    "transfer cannot finalize with {missing_chunks} unverified chunk(s) (PO-BED-3)"
                )
            }
        }
    }
}

impl std::error::Error for SyncError {}
