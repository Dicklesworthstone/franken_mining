//! Determinism: Exact
//!
//! Offline filing ingest into `FMT-CAS-1` (`fmine-object::CasStore`) with SHA-256 verification and
//! digest-stable re-ingest (`BD-056`). Downloads are always manual; engines never fetch over network.

use std::fmt;
use std::fs;
use std::path::Path;

use fmine_core::hash::{Digest32, sha256};
use fmine_evidence::{Refusal, RefusalCode};
use fmine_object::cas::{CasError, DEFAULT_CHUNK_BYTES};
use fmine_object::store::CasStore;

/// Receipt emitted after offline filing ingest into `FMT-CAS-1` CAS.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IngestedFiling {
    pub media_type: String,
    pub total_bytes: u64,
    pub sha256_hex: String,
    pub cas_root: Digest32,
    pub chunk_count: usize,
    pub chunks_written: usize,
    pub chunks_reused: usize,
}

/// Errors raised when ingesting a local filing into CAS.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CasIngestError {
    Io {
        path: String,
        detail: String,
    },
    DigestMismatch {
        path: String,
        expected: String,
        actual: String,
    },
    Cas(CasError),
}

impl CasIngestError {
    #[must_use]
    pub fn to_refusal(&self) -> Refusal {
        match self {
            Self::Io { path, detail } => Refusal::new(
                RefusalCode::InputMissingOrUnreadable506,
                format!("offline filing {path}: {detail}"),
            ),
            Self::DigestMismatch {
                path,
                expected,
                actual,
            } => Refusal::new(
                RefusalCode::DigestMismatch505,
                format!("filing {path} sha256 mismatch: expected {expected}, got {actual}"),
            ),
            Self::Cas(e) => e.to_refusal(),
        }
    }
}

impl fmt::Display for CasIngestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { path, detail } => write!(f, "failed to read filing {path}: {detail}"),
            Self::DigestMismatch {
                path,
                expected,
                actual,
            } => write!(
                f,
                "filing {path} sha256 mismatch: expected {expected}, got {actual}"
            ),
            Self::Cas(e) => write!(f, "CAS ingest error: {e}"),
        }
    }
}

impl std::error::Error for CasIngestError {}

/// Ingests an offline filing on disk into `store` using `FMT-CAS-1` root-last publication.
///
/// If `expected_sha256` is supplied, verifies the exact lowercase SHA-256 hex digest BEFORE writing
/// any chunks or manifest (`FMINE-REF-505` on mismatch). Re-ingesting identical bytes reuses all
/// chunks and returns the exact same `cas_root` and `sha256_hex`.
pub fn ingest_file_into_cas(
    store: &CasStore,
    path: &Path,
    media_type: &str,
    expected_sha256: Option<&str>,
) -> Result<IngestedFiling, CasIngestError> {
    ingest_file_into_cas_with_chunk_size(
        store,
        path,
        media_type,
        expected_sha256,
        DEFAULT_CHUNK_BYTES,
    )
}

/// Same as [`ingest_file_into_cas`] with an explicit chunk size (for testing multi-chunk trees).
pub fn ingest_file_into_cas_with_chunk_size(
    store: &CasStore,
    path: &Path,
    media_type: &str,
    expected_sha256: Option<&str>,
    chunk_size: usize,
) -> Result<IngestedFiling, CasIngestError> {
    let bytes = fs::read(path).map_err(|e| CasIngestError::Io {
        path: path.display().to_string(),
        detail: e.to_string(),
    })?;
    let actual_hex = sha256(&bytes).to_hex();
    if let Some(expected) = expected_sha256
        && actual_hex != expected
    {
        return Err(CasIngestError::DigestMismatch {
            path: path.display().to_string(),
            expected: expected.to_string(),
            actual: actual_hex,
        });
    }
    let staged = store
        .stage_object(media_type, &bytes, chunk_size)
        .map_err(CasIngestError::Cas)?;
    let chunks_written = staged.chunks_written;
    let chunks_reused = staged.chunks_reused;
    let manifest = store.publish_staged(&staged).map_err(CasIngestError::Cas)?;
    Ok(IngestedFiling {
        media_type: manifest.media_type,
        total_bytes: manifest.total_bytes,
        sha256_hex: actual_hex,
        cas_root: manifest.root_digest,
        chunk_count: manifest.chunks.len(),
        chunks_written,
        chunks_reused,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cas_reingest_is_digest_stable() {
        let dir = std::env::temp_dir().join(format!("fmine-io-cas-ingest-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("mkdir");
        let cas_dir = dir.join("cas");
        let store = CasStore::open(&cas_dir).expect("open cas");

        let filing_path = dir.join("synthetic_mine_stats.xlsx");
        let payload: Vec<u8> = (0..10_000usize)
            .map(|i| ((i.wrapping_mul(2654435761) >> 16) ^ i) as u8)
            .collect();
        fs::write(&filing_path, &payload).expect("write filing");
        let expected_sha = sha256(&payload).to_hex();

        let first = ingest_file_into_cas_with_chunk_size(
            &store,
            &filing_path,
            "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
            Some(&expected_sha),
            1024,
        )
        .expect("first ingest");
        assert_eq!(first.total_bytes, payload.len() as u64);
        assert_eq!(first.sha256_hex, expected_sha);
        assert_eq!(first.chunk_count, 10);
        assert_eq!(first.chunks_written, 10);
        assert_eq!(first.chunks_reused, 0);

        // Re-ingest must be digest-stable and reuse 100% of existing chunks.
        let second = ingest_file_into_cas_with_chunk_size(
            &store,
            &filing_path,
            "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
            Some(&expected_sha),
            1024,
        )
        .expect("second ingest");
        assert_eq!(second.cas_root, first.cas_root);
        assert_eq!(second.sha256_hex, first.sha256_hex);
        assert_eq!(second.chunks_written, 0);
        assert_eq!(second.chunks_reused, 10);
        assert_eq!(
            store.read_object(&first.cas_root).expect("read back"),
            payload
        );

        // Mutated bytes against pinned digest refuse with FMINE-REF-505 before touching CAS.
        let mut tampered = payload;
        tampered[4096] ^= 0x01;
        fs::write(&filing_path, &tampered).expect("write tampered");
        let err = ingest_file_into_cas_with_chunk_size(
            &store,
            &filing_path,
            "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
            Some(&expected_sha),
            1024,
        )
        .expect_err("tampered refused");
        assert_eq!(
            (err.to_refusal().code, err.to_refusal().exit_code),
            ("FMINE-REF-505", 8)
        );

        let _ = fs::remove_dir_all(&dir);
    }
}
