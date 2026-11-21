//! Determinism: BitwisePortable
//!
//! Bedrock security, confidentiality, fail-closed residency, and human effect-release authority
//! tokens (`BD-092`, `BD-093`, `BD-094`, `BD-095`; plan §31).

pub mod human_token;
pub mod macaroon;
pub mod residency;
pub mod wall;

use std::fmt;

use fmine_evidence::{Refusal, RefusalCode};

/// Structured authorization and security errors mapped onto registered `FMINE-REF-*` codes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthzError {
    /// S3(b): all-zero key rejected when constructing a signing key or key ring.
    ZeroKeyRejected { role: &'static str },
    /// S3(b): active and retired keys or key ids collide in a key ring.
    KeyRingCollision { detail: String },
    /// S3(b): token presented an all-zero signature (unconditionally rejected before HMAC check).
    ZeroSignatureRejected,
    /// S3(c): third-party caveats are permanently forbidden in FrankenMining tokens.
    ThirdPartyCaveatForbidden,
    /// Macaroon signature verification failed or key id is unknown.
    SignatureInvalid { detail: String },
    /// Wire framing or codec error in `FMT-MAC-1`.
    WireFormat { detail: String },
    /// Token has expired or is not yet valid at the verification instant.
    TokenTemporalInvalid { detail: String },
    /// `PO-SEC-3`: `HumanAuthorityToken` violation (agent principal, TTL > 15 min, wrong site/digest, replay, duplicate signer).
    HumanAuthorityRefused { detail: String },
    /// `INV-007` / §31.1: MNPI embargo or wall boundary refused.
    MnpiWallRefused { detail: String },
    /// §31.2: joint-venture wall boundary refused.
    VentureWallRefused { detail: String },
    /// §31.2 / `BD-094`: cross-border transfer refused by `registries/residency.toml` or fail-closed default.
    ResidencyRefused {
        origin: String,
        destination: String,
        data_class: String,
        detail: String,
    },
    /// Malformed or fail-open residency registry.
    ResidencyRegistryInvalid { line: usize, detail: String },
    /// Underlying filesystem read failure when loading a registry.
    Io { path: String, detail: String },
}

impl AuthzError {
    /// Converts this authorization error into its canonical registered [`Refusal`].
    #[must_use]
    pub fn to_refusal(&self) -> Refusal {
        match self {
            Self::MnpiWallRefused { detail } => {
                Refusal::new(RefusalCode::MnpiWall301, detail.clone())
            }
            Self::VentureWallRefused { detail } => {
                Refusal::new(RefusalCode::VentureWall302, detail.clone())
            }
            Self::ResidencyRefused {
                origin,
                destination,
                data_class,
                detail,
            } => Refusal::new(
                RefusalCode::Residency303,
                format!("{origin}->{destination} ({data_class}): {detail}"),
            ),
            Self::HumanAuthorityRefused { detail } | Self::TokenTemporalInvalid { detail } => {
                Refusal::new(RefusalCode::MissingHumanToken304, detail.clone())
            }
            Self::ZeroKeyRejected { .. }
            | Self::KeyRingCollision { .. }
            | Self::ZeroSignatureRejected
            | Self::ThirdPartyCaveatForbidden
            | Self::SignatureInvalid { .. }
            | Self::WireFormat { .. }
            | Self::ResidencyRegistryInvalid { .. }
            | Self::Io { .. } => Refusal::new(RefusalCode::DigestMismatch505, self.to_string()),
        }
    }
}

impl fmt::Display for AuthzError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroKeyRejected { role } => write!(f, "all-zero key rejected for {role}"),
            Self::KeyRingCollision { detail } => write!(f, "key ring collision: {detail}"),
            Self::ZeroSignatureRejected => {
                write!(
                    f,
                    "S3(b): all-zero macaroon signature unconditionally rejected"
                )
            }
            Self::ThirdPartyCaveatForbidden => {
                write!(
                    f,
                    "S3(c): third-party caveats are forbidden; use first-party caveats only"
                )
            }
            Self::SignatureInvalid { detail } => write!(f, "macaroon signature invalid: {detail}"),
            Self::WireFormat { detail } => write!(f, "FMT-MAC-1 wire error: {detail}"),
            Self::TokenTemporalInvalid { detail } => {
                write!(f, "token temporal check failed: {detail}")
            }
            Self::HumanAuthorityRefused { detail } => {
                write!(f, "PO-SEC-3 HumanAuthorityToken refused: {detail}")
            }
            Self::MnpiWallRefused { detail } => write!(f, "MNPI wall refused: {detail}"),
            Self::VentureWallRefused { detail } => write!(f, "venture wall refused: {detail}"),
            Self::ResidencyRefused {
                origin,
                destination,
                data_class,
                detail,
            } => write!(
                f,
                "residency refused transfer {origin}->{destination} ({data_class}): {detail}"
            ),
            Self::ResidencyRegistryInvalid { line, detail } => {
                write!(f, "residency registry line {line}: {detail}")
            }
            Self::Io { path, detail } => write!(f, "I/O error at {path}: {detail}"),
        }
    }
}

impl std::error::Error for AuthzError {}
