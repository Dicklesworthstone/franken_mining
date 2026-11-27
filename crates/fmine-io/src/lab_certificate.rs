//! Determinism: Exact
//!
//! Lab certificate ingest with the original in CAS (`BD-171`, plan §12.1). The signed original is stored in
//! `FMT-CAS-1` (`fmine-object::CasStore`, root-last), and the certificate record names it twice: by the SHA-256 of
//! the original bytes and by the CAS root of the copy. [`verify_certificate`] reads the root back and checks that
//! it gives those bytes, so a record cannot point at a copy that was swapped, damaged or never stored.
//!
//! Ingesting the same bytes again gives the same root and writes no chunk. When the lab or LIMS states the
//! original's digest, a copy with another digest is refused before anything is stored.

use std::fmt;

use fmine_core::hash::{Digest32, sha256};
use fmine_drill::records::{DrillError, DrillRecord, LabCertificate};
use fmine_evidence::{Refusal, RefusalCode};
use fmine_object::cas::{CasError, DEFAULT_CHUNK_BYTES};
use fmine_object::store::CasStore;

/// A certificate original as received from the lab.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CertificateOriginal<'a> {
    /// Certificate id.
    pub certificate_id: &'a str,
    /// Issuing lab.
    pub lab: &'a str,
    /// Media type, for example `application/pdf`.
    pub media_type: &'a str,
    /// When it was issued, ns since the Unix epoch, UTC.
    pub issued_at_ns: i64,
    /// The original bytes.
    pub bytes: &'a [u8],
    /// The SHA-256 the lab or LIMS states for it, if it states one.
    pub stated_sha256: Option<Digest32>,
}

/// What ingesting an original did.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IngestedCertificate {
    /// The record to commit, linked to the copy by digest and root.
    pub record: LabCertificate,
    /// Bytes stored.
    pub total_bytes: u64,
    /// Chunks in the copy.
    pub chunk_count: usize,
}

/// A certificate that cannot be ingested or whose copy does not check.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CertificateError {
    /// No bytes (`FMINE-REF-506`).
    Empty {
        /// The certificate.
        certificate_id: String,
    },
    /// The bytes do not have the digest the lab stated, or the copy read back does not have the record's
    /// digest (`FMINE-REF-505`).
    DigestMismatch {
        /// The certificate.
        certificate_id: String,
        /// The digest expected.
        expected: Digest32,
        /// The digest found.
        actual: Digest32,
    },
    /// The copy is stored under another media type than the record's (`FMINE-REF-505`).
    MediaTypeMismatch {
        /// The certificate.
        certificate_id: String,
        /// The record's.
        record: String,
        /// The copy's.
        stored: String,
    },
    /// The record fails its own checks (its own code).
    Record(DrillError),
    /// The CAS store refused: for example no copy at the root, or a damaged chunk (its own code).
    Cas(CasError),
}

impl CertificateError {
    /// The refusal this error stands for.
    #[must_use]
    pub fn to_refusal(&self) -> Refusal {
        match self {
            Self::Empty { .. } => {
                Refusal::new(RefusalCode::InputMissingOrUnreadable506, self.to_string())
            }
            Self::DigestMismatch { .. } | Self::MediaTypeMismatch { .. } => {
                Refusal::new(RefusalCode::DigestMismatch505, self.to_string())
            }
            Self::Record(e) => e.to_refusal(),
            Self::Cas(e) => e.to_refusal(),
        }
    }
}

impl fmt::Display for CertificateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty { certificate_id } => write!(f, "certificate '{certificate_id}': no bytes"),
            Self::DigestMismatch {
                certificate_id,
                expected,
                actual,
            } => write!(
                f,
                "certificate '{certificate_id}': SHA-256 {} where {} is expected",
                actual.to_hex(),
                expected.to_hex()
            ),
            Self::MediaTypeMismatch {
                certificate_id,
                record,
                stored,
            } => write!(
                f,
                "certificate '{certificate_id}': the record says {record}, the copy is stored as {stored}"
            ),
            Self::Record(e) => write!(f, "{e}"),
            Self::Cas(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for CertificateError {}

impl From<CasError> for CertificateError {
    fn from(e: CasError) -> Self {
        Self::Cas(e)
    }
}

/// Store `original` in `store` and return the certificate record linked to the copy.
///
/// # Errors
/// [`CertificateError::Empty`], [`CertificateError::DigestMismatch`] against a stated digest (nothing is
/// stored), [`CertificateError::Record`] for an id, lab or media type the record refuses (nothing is stored), or
/// [`CertificateError::Cas`].
pub fn ingest_certificate(
    store: &CasStore,
    original: &CertificateOriginal<'_>,
) -> Result<IngestedCertificate, CertificateError> {
    let certificate_id = original.certificate_id.to_string();
    if original.bytes.is_empty() {
        return Err(CertificateError::Empty { certificate_id });
    }
    let digest = sha256(original.bytes);
    if let Some(stated) = original.stated_sha256
        && stated != digest
    {
        return Err(CertificateError::DigestMismatch {
            certificate_id,
            expected: stated,
            actual: digest,
        });
    }
    let mut record = LabCertificate {
        certificate_id,
        lab: original.lab.to_string(),
        original_sha256: digest,
        cas_root: Digest32([0; 32]),
        media_type: original.media_type.to_string(),
        issued_at_ns: original.issued_at_ns,
    };
    DrillRecord::LabCertificate(record.clone())
        .validate()
        .map_err(CertificateError::Record)?;
    let manifest = store.put_bytes(original.media_type, original.bytes, DEFAULT_CHUNK_BYTES)?;
    record.cas_root = manifest.root_digest;
    Ok(IngestedCertificate {
        record,
        total_bytes: manifest.total_bytes,
        chunk_count: manifest.chunks.len(),
    })
}

/// Check that `record`'s CAS root holds an original with the record's digest and media type. Returns its
/// length in bytes.
///
/// # Errors
/// [`CertificateError::Cas`] when the root is not published or a chunk is missing or damaged,
/// [`CertificateError::MediaTypeMismatch`], or [`CertificateError::DigestMismatch`] when the copy read back is
/// not the original the record names.
pub fn verify_certificate(
    store: &CasStore,
    record: &LabCertificate,
) -> Result<u64, CertificateError> {
    let manifest = store.get_manifest(&record.cas_root)?;
    if manifest.media_type != record.media_type {
        return Err(CertificateError::MediaTypeMismatch {
            certificate_id: record.certificate_id.clone(),
            record: record.media_type.clone(),
            stored: manifest.media_type,
        });
    }
    let bytes = store.read_object(&record.cas_root)?;
    let actual = sha256(&bytes);
    if actual != record.original_sha256 {
        return Err(CertificateError::DigestMismatch {
            certificate_id: record.certificate_id.clone(),
            expected: record.original_sha256,
            actual,
        });
    }
    Ok(manifest.total_bytes)
}
