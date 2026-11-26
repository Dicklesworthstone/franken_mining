//! Determinism: Exact
//!
//! Offline filing ingest into `FMT-CAS-1` (`fmine-object::CasStore`) with SHA-256 verification and
//! digest-stable re-ingest (`BD-056`). Downloads are always manual; engines never fetch over network.

use std::collections::BTreeSet;
use std::fmt;
use std::fs;
use std::path::Path;

use fmine_core::codec::Encoder;
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

/// Specification for one offline filing registered in a source registry (`BD-055` -> `BD-056`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OfflineFilingSpec<'a> {
    pub filing_id: &'a str,
    pub path: &'a Path,
    pub media_type: &'a str,
    pub expected_sha256: &'a str,
    pub expected_bytes: Option<u64>,
    pub accession: Option<&'a str>,
}

/// Receipt for one registered filing ingested into CAS.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisteredFilingReceipt {
    pub filing_id: String,
    pub accession: Option<String>,
    pub ingested: IngestedFiling,
}

/// Canonical order-independent batch receipt for a set of offline filings ingested into CAS.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilingBatchReceipt {
    pub entries: Vec<RegisteredFilingReceipt>,
    pub batch_digest: Digest32,
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
    ByteLengthMismatch {
        path: String,
        expected: u64,
        actual: u64,
    },
    InvalidInput {
        detail: String,
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
            Self::ByteLengthMismatch {
                path,
                expected,
                actual,
            } => Refusal::new(
                RefusalCode::DigestMismatch505,
                format!("filing {path} byte length mismatch: expected {expected}, got {actual}"),
            ),
            Self::InvalidInput { detail } => {
                Refusal::new(RefusalCode::UndeclaredUnitBasisOrTz107, detail.clone())
            }
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
            Self::ByteLengthMismatch {
                path,
                expected,
                actual,
            } => write!(
                f,
                "filing {path} byte length mismatch: expected {expected}, got {actual}"
            ),
            Self::InvalidInput { detail } => write!(f, "invalid CAS ingest input: {detail}"),
            Self::Cas(e) => write!(f, "CAS ingest error: {e}"),
        }
    }
}

impl std::error::Error for CasIngestError {}

fn is_lower_hex64(s: &str) -> bool {
    s.len() == 64 && s.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

fn validate_ingest_args(
    media_type: &str,
    expected_sha256: Option<&str>,
    chunk_size: usize,
) -> Result<(), CasIngestError> {
    if media_type.trim().is_empty() || media_type != media_type.trim() {
        return Err(CasIngestError::InvalidInput {
            detail: format!("media_type must be non-empty and trimmed, got {media_type:?}"),
        });
    }
    if chunk_size == 0 {
        return Err(CasIngestError::InvalidInput {
            detail: "chunk_size must be > 0".to_string(),
        });
    }
    if let Some(expected) = expected_sha256
        && !is_lower_hex64(expected)
    {
        return Err(CasIngestError::InvalidInput {
            detail: format!("expected_sha256 must be 64 lowercase hex chars, got {expected:?}"),
        });
    }
    Ok(())
}

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
    validate_ingest_args(media_type, expected_sha256, chunk_size)?;
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
    publish_verified_bytes(store, media_type, &bytes, actual_hex, chunk_size)
}

/// Atomically pre-flight and ingest a batch of offline filings registered under `BD-055` into
/// `FMT-CAS-1` CAS (`BD-056`).
///
/// Every spec is validated and every file is read and checked against `expected_bytes` and
/// `expected_sha256` BEFORE publishing any chunks or manifests, so a missing or tampered file
/// leaves `store` unchanged. Output entries are canonically sorted by `filing_id` and bound to
/// an order-independent `CIB1` SHA-256 `batch_digest`.
pub fn ingest_registered_filings(
    store: &CasStore,
    specs: &[OfflineFilingSpec<'_>],
    chunk_size: usize,
) -> Result<FilingBatchReceipt, CasIngestError> {
    if specs.is_empty() {
        return Err(CasIngestError::InvalidInput {
            detail: "at least one OfflineFilingSpec is required".to_string(),
        });
    }
    let mut seen_ids = BTreeSet::new();
    let mut verified: Vec<(&OfflineFilingSpec<'_>, Vec<u8>, String)> =
        Vec::with_capacity(specs.len());

    // Pre-flight phase: validate all metadata, byte lengths, and SHA-256 pins before writing CAS.
    for spec in specs {
        if spec.filing_id.trim().is_empty() || spec.filing_id != spec.filing_id.trim() {
            return Err(CasIngestError::InvalidInput {
                detail: format!(
                    "filing_id must be non-empty and trimmed, got {:?}",
                    spec.filing_id
                ),
            });
        }
        if !seen_ids.insert(spec.filing_id) {
            return Err(CasIngestError::InvalidInput {
                detail: format!("duplicate filing_id {:?}", spec.filing_id),
            });
        }
        if let Some(acc) = spec.accession
            && (acc.trim().is_empty() || acc != acc.trim())
        {
            return Err(CasIngestError::InvalidInput {
                detail: format!("accession must be non-empty and trimmed, got {acc:?}"),
            });
        }
        validate_ingest_args(spec.media_type, Some(spec.expected_sha256), chunk_size)?;
        let bytes = fs::read(spec.path).map_err(|e| CasIngestError::Io {
            path: spec.path.display().to_string(),
            detail: e.to_string(),
        })?;
        let actual_len = bytes.len() as u64;
        if let Some(expected_len) = spec.expected_bytes
            && actual_len != expected_len
        {
            return Err(CasIngestError::ByteLengthMismatch {
                path: spec.path.display().to_string(),
                expected: expected_len,
                actual: actual_len,
            });
        }
        let actual_hex = sha256(&bytes).to_hex();
        if actual_hex != spec.expected_sha256 {
            return Err(CasIngestError::DigestMismatch {
                path: spec.path.display().to_string(),
                expected: spec.expected_sha256.to_string(),
                actual: actual_hex,
            });
        }
        verified.push((spec, bytes, actual_hex));
    }

    // Publish in canonical filing_id order.
    verified.sort_by(|a, b| a.0.filing_id.cmp(b.0.filing_id));
    let mut entries = Vec::with_capacity(verified.len());
    let mut enc = Encoder::new_frame(*b"CIB1", 1);
    enc.put_u64(verified.len() as u64);

    for (spec, bytes, actual_hex) in verified {
        let ingested =
            publish_verified_bytes(store, spec.media_type, &bytes, actual_hex, chunk_size)?;
        enc.put_str(spec.filing_id);
        enc.put_u8(u8::from(spec.accession.is_some()));
        if let Some(acc) = spec.accession {
            enc.put_str(acc);
        }
        enc.put_str(&ingested.media_type);
        enc.put_u64(ingested.total_bytes);
        enc.put_str(&ingested.sha256_hex);
        enc.put_bytes(ingested.cas_root.as_bytes());
        entries.push(RegisteredFilingReceipt {
            filing_id: spec.filing_id.to_string(),
            accession: spec.accession.map(ToString::to_string),
            ingested,
        });
    }

    Ok(FilingBatchReceipt {
        entries,
        batch_digest: sha256(&enc.finish()),
    })
}

fn publish_verified_bytes(
    store: &CasStore,
    media_type: &str,
    bytes: &[u8],
    actual_hex: String,
    chunk_size: usize,
) -> Result<IngestedFiling, CasIngestError> {
    let staged = store
        .stage_object(media_type, bytes, chunk_size)
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
    use std::path::PathBuf;

    struct TempDirGuard(PathBuf);

    impl TempDirGuard {
        fn new(tag: &str) -> Self {
            let dir = std::env::temp_dir()
                .join(format!("fmine-io-cas-ingest-{tag}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&dir);
            fs::create_dir_all(&dir).expect("mkdir");
            Self(dir)
        }
    }

    impl Drop for TempDirGuard {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn test_cas_reingest_is_digest_stable() {
        let guard = TempDirGuard::new("single");
        let cas_dir = guard.0.join("cas");
        let store = CasStore::open(&cas_dir).expect("open cas");

        let filing_path = guard.0.join("synthetic_mine_stats.xlsx");
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
    }

    #[test]
    fn test_cas_batch_registered_filings_atomic_preflight_and_order_invariant_reingest() {
        let guard = TempDirGuard::new("batch");
        let cas_dir = guard.0.join("cas");
        let store = CasStore::open(&cas_dir).expect("open cas");

        let p1 = guard.0.join("q2_2026_mine_stats.xlsx");
        let p2 = guard.0.join("q2_2026_6k_reconciliation.pdf");
        let bytes1: Vec<u8> = (0..3_000usize)
            .map(|i| ((i.wrapping_mul(2_654_435_761) >> 16) ^ i) as u8)
            .collect();
        let bytes2: Vec<u8> = (0..2_500usize)
            .map(|i| ((i.wrapping_mul(2_246_822_519) >> 16) ^ (i >> 3) ^ 0x5a) as u8)
            .collect();
        fs::write(&p1, &bytes1).expect("write p1");
        fs::write(&p2, &bytes2).expect("write p2");
        let sha1 = sha256(&bytes1).to_hex();
        let sha2 = sha256(&bytes2).to_hex();

        // 1. Atomic pre-flight: second file has wrong byte length -> refuses FMINE-REF-505 and
        //    leaves first file unpublished in CAS.
        let bad_specs = [
            OfflineFilingSpec {
                filing_id: "barrick-2026Q2-mine-stats",
                path: &p1,
                media_type: "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
                expected_sha256: &sha1,
                expected_bytes: Some(bytes1.len() as u64),
                accession: Some("0001193125-26-200001"),
            },
            OfflineFilingSpec {
                filing_id: "barrick-2026Q2-6k",
                path: &p2,
                media_type: "application/pdf",
                expected_sha256: &sha2,
                expected_bytes: Some(999),
                accession: Some("0001193125-26-200001"),
            },
        ];
        let len_err = ingest_registered_filings(&store, &bad_specs, 1024)
            .expect_err("byte mismatch must refuse");
        assert_eq!(len_err.to_refusal().code, "FMINE-REF-505");

        // 2. Missing file refuses with FMINE-REF-506; malformed hex or blank media_type with FMINE-REF-107.
        let missing = guard.0.join("does_not_exist.xlsx");
        let missing_err = ingest_file_into_cas(&store, &missing, "application/pdf", Some(&sha1))
            .expect_err("missing refuses");
        assert_eq!(missing_err.to_refusal().code, "FMINE-REF-506");
        let bad_hex_err = ingest_file_into_cas(&store, &p1, "application/pdf", Some("NOT-HEX"))
            .expect_err("bad hex refuses");
        assert_eq!(bad_hex_err.to_refusal().code, "FMINE-REF-107");
        let bad_mt_err = ingest_file_into_cas(&store, &p1, "   ", Some(&sha1))
            .expect_err("blank media_type refuses");
        assert_eq!(bad_mt_err.to_refusal().code, "FMINE-REF-107");

        // 3. Valid batch ingest publishes both filings and emits canonical digest.
        let good_specs = [
            OfflineFilingSpec {
                filing_id: "barrick-2026Q2-mine-stats",
                path: &p1,
                media_type: "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
                expected_sha256: &sha1,
                expected_bytes: Some(bytes1.len() as u64),
                accession: Some("0001193125-26-200001"),
            },
            OfflineFilingSpec {
                filing_id: "barrick-2026Q2-6k",
                path: &p2,
                media_type: "application/pdf",
                expected_sha256: &sha2,
                expected_bytes: Some(bytes2.len() as u64),
                accession: Some("0001193125-26-200001"),
            },
        ];
        let batch_a =
            ingest_registered_filings(&store, &good_specs, 1024).expect("valid batch ingest");
        assert_eq!(batch_a.entries.len(), 2);
        // Canonically sorted by filing_id ("...-6k" before "...-mine-stats").
        assert_eq!(batch_a.entries[0].filing_id, "barrick-2026Q2-6k");
        assert_eq!(batch_a.entries[1].filing_id, "barrick-2026Q2-mine-stats");
        assert!(
            batch_a
                .entries
                .iter()
                .all(|e| e.ingested.chunks_written > 0 && e.ingested.chunks_reused == 0)
        );

        // 4. Re-ingest in reversed input order produces byte-identical batch_digest & cas_roots,
        //    with zero newly written chunks.
        let reversed_specs = [good_specs[1], good_specs[0]];
        let batch_b = ingest_registered_filings(&store, &reversed_specs, 1024)
            .expect("reversed batch re-ingest");
        assert_eq!(batch_b.batch_digest, batch_a.batch_digest);
        for (a, b) in batch_a.entries.iter().zip(batch_b.entries.iter()) {
            assert_eq!(a.ingested.cas_root, b.ingested.cas_root);
            assert_eq!(b.ingested.chunks_written, 0);
            assert_eq!(b.ingested.chunks_reused, a.ingested.chunk_count);
        }
    }
}
