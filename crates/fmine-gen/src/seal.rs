//! Determinism: BitwisePortable
//! Sealed ground-truth container and capability key (`BD-127`, `GATE-GEN-01`, plan §33.3).
//!
//! Engines under test receive observations only; ground truth stays inside [`SealedTruth<T>`],
//! whose payload is private and can only be opened with the matching [`SealKey`] minted by the
//! [`crate::harness::ScoringHarness`].
//!
//! Attempting to read the inner truth field directly fails at compile time:
//! ```compile_fail,E0616
//! fn cheat<T>(sealed: &fmine_gen::seal::SealedTruth<T>) -> &T {
//!     &sealed.truth
//! }
//! ```
//!
//! Attempting to forge a [`SealKey`] outside `fmine-gen` fails at compile time:
//! ```compile_fail,E0451
//! fn forge_key() -> fmine_gen::seal::SealKey {
//!     fmine_gen::seal::SealKey { secret: [0u8; 32] }
//! }
//! ```

use core::fmt;

use fmine_core::hash::{Digest32, blake3_hash, sha256};

/// Error returned when a seal cannot be opened with the supplied [`SealKey`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SealError {
    /// Provided `SealKey` does not match the harness that sealed this world (`GATE-GEN-01`).
    InvalidKey { world_id: String },
}

impl fmt::Display for SealError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidKey { world_id } => {
                write!(
                    f,
                    "GATE-GEN-01: unauthorized attempt to open sealed truth for world '{world_id}'"
                )
            }
        }
    }
}

impl std::error::Error for SealError {}

/// Unforgeable harness capability required to open a [`SealedTruth<T>`].
#[derive(Clone)]
pub struct SealKey {
    secret: [u8; 32],
}

impl SealKey {
    pub(crate) fn mint(seed_root: u64, harness_tag: &str) -> Self {
        let mut buf = Vec::with_capacity(16 + harness_tag.len());
        buf.extend_from_slice(b"FMINE-SEAL-V1:");
        buf.extend_from_slice(&seed_root.to_le_bytes());
        buf.extend_from_slice(harness_tag.as_bytes());
        Self {
            secret: blake3_hash(&buf).0,
        }
    }

    pub(crate) fn key_commitment(&self) -> Digest32 {
        sha256(&self.secret)
    }
}

impl fmt::Debug for SealKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SealKey(<redacted>)")
    }
}

/// Ground truth `T` sealed against inspection by engines under test (`GATE-GEN-01`).
///
/// `Debug` intentionally redacts the inner truth so logging a `SealedTruth` never leaks answers.
pub struct SealedTruth<T> {
    world_id: String,
    seed_root: u64,
    truth_commitment: Digest32,
    expected_key_commitment: Digest32,
    truth: T,
}

impl<T> SealedTruth<T> {
    pub(crate) fn seal(
        world_id: impl Into<String>,
        seed_root: u64,
        canonical_truth_bytes: &[u8],
        key: &SealKey,
        truth: T,
    ) -> Self {
        Self {
            world_id: world_id.into(),
            seed_root,
            truth_commitment: sha256(canonical_truth_bytes),
            expected_key_commitment: key.key_commitment(),
            truth,
        }
    }

    /// Public identifier of the synthetic world.
    #[must_use]
    pub fn world_id(&self) -> &str {
        &self.world_id
    }

    /// Deterministic seed root that generated this world.
    #[must_use]
    pub const fn seed_root(&self) -> u64 {
        self.seed_root
    }

    /// Public SHA-256 commitment digest of the sealed truth bytes (reveals no truth values).
    #[must_use]
    pub const fn truth_commitment(&self) -> Digest32 {
        self.truth_commitment
    }

    /// Open the seal using the matching harness [`SealKey`]. Refuses any mismatched key.
    pub fn open(&self, key: &SealKey) -> Result<&T, SealError> {
        if key.key_commitment() != self.expected_key_commitment {
            return Err(SealError::InvalidKey {
                world_id: self.world_id.clone(),
            });
        }
        Ok(&self.truth)
    }
}

impl<T> fmt::Debug for SealedTruth<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SealedTruth")
            .field("world_id", &self.world_id)
            .field("seed_root", &self.seed_root)
            .field("truth_commitment", &self.truth_commitment.to_hex())
            .field("truth", &"<sealed>")
            .finish()
    }
}
