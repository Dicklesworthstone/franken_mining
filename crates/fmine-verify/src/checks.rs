//! Offline cryptographic verification routines for `fmine-verify` (`BD-140`, plan §33.6).
//!
//! All checks are read-only and fail closed:
//! - Missing top-level input files/directories return [`VerifyError::MissingOrUnreadable`] (exit 5, `FMINE-REF-506`).
//! - Any CRC32C, digest, Merkle root, chunk, segment, or MMR consistency failure returns
//!   [`VerifyError::Tamper`] (exit 8, `FMINE-REF-505`).

use std::fmt;
use std::fs;
use std::io::ErrorKind;
use std::path::{Component, Path};

use fmine_core::cert::Certificate;
use fmine_core::hash::{Digest32, sha256};
use fmine_evidence::{Refusal, RefusalCode};
use fmine_ledger::mmr::{leaf_hash, verify_inclusion};
use fmine_ledger::verify::{SegmentVerification, verify_segment};
use fmine_object::cas::CasManifest;
use fmine_sync::sync_cert::SyncCert;

/// Typed verification error with exact CLI exit-code mapping (`BD-140`, plan §30.1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VerifyError {
    /// Any cryptographic, structural, digest, chunk, segment, or proof verification failure (exit 8).
    Tamper { kind: &'static str, detail: String },
    /// Top-level requested file or directory is missing or unreadable (exit 5).
    MissingOrUnreadable { path: String, detail: String },
    /// Malformed CLI arguments or hex digest syntax (exit 2).
    Usage(String),
}

impl VerifyError {
    /// Exact process exit code (`8` tamper, `5` missing/unreadable, `2` usage).
    #[must_use]
    pub const fn exit_code(&self) -> u8 {
        match self {
            Self::Tamper { .. } => 8,
            Self::MissingOrUnreadable { .. } => 5,
            Self::Usage(_) => 2,
        }
    }

    /// Maps non-usage errors to their registered `FMINE-REF-*` refusal.
    #[must_use]
    pub fn to_refusal(&self) -> Option<Refusal> {
        match self {
            Self::Tamper { .. } => Some(Refusal::new(
                RefusalCode::DigestMismatch505,
                self.to_string(),
            )),
            Self::MissingOrUnreadable { .. } => Some(Refusal::new(
                RefusalCode::InputMissingOrUnreadable506,
                self.to_string(),
            )),
            Self::Usage(_) => None,
        }
    }
}

impl fmt::Display for VerifyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Tamper { kind, detail } => write!(f, "{kind} verification failed: {detail}"),
            Self::MissingOrUnreadable { path, detail } => {
                write!(f, "missing or unreadable input '{path}': {detail}")
            }
            Self::Usage(msg) => write!(f, "usage error: {msg}"),
        }
    }
}

impl std::error::Error for VerifyError {}

/// Parse a strict 64-character lowercase hexadecimal SHA-256/BLAKE3 [`Digest32`].
pub fn parse_digest32_hex(hex: &str) -> Result<Digest32, VerifyError> {
    if hex.len() != 64
        || !hex
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(VerifyError::Usage(format!(
            "expected 64 lowercase hex characters for digest, got {hex:?}"
        )));
    }
    let mut out = [0u8; 32];
    for (idx, chunk) in hex.as_bytes().as_chunks::<2>().0.iter().enumerate() {
        let s = std::str::from_utf8(chunk)
            .map_err(|e| VerifyError::Usage(format!("invalid UTF-8 in hex digest: {e}")))?;
        out[idx] = u8::from_str_radix(s, 16)
            .map_err(|e| VerifyError::Usage(format!("invalid hex byte '{s}': {e}")))?;
    }
    Ok(Digest32(out))
}

fn read_input_bytes(path: &Path) -> Result<Vec<u8>, VerifyError> {
    fs::read(path).map_err(|e| VerifyError::MissingOrUnreadable {
        path: path.display().to_string(),
        detail: e.to_string(),
    })
}

/// Verify raw `FMT-CERT-1` certificate bytes (`FMNE` + `CERT` + CRC32C + self-verifying digest).
pub fn verify_certificate_bytes(bytes: &[u8]) -> Result<Certificate, VerifyError> {
    Certificate::from_bytes(bytes).map_err(|e| VerifyError::Tamper {
        kind: "certificate",
        detail: e.to_string(),
    })
}

/// Verify an `FMT-CERT-1` certificate file on disk and, if `inputs_dir` is given, re-hash every
/// `InputDigest` locator under `inputs_dir` and require exact SHA-256 parity.
pub fn verify_certificate_file(
    path: &Path,
    inputs_dir: Option<&Path>,
) -> Result<Certificate, VerifyError> {
    let bytes = read_input_bytes(path)?;
    let cert = verify_certificate_bytes(&bytes)?;
    if let Some(base) = inputs_dir {
        if !base.is_dir() {
            return Err(VerifyError::MissingOrUnreadable {
                path: base.display().to_string(),
                detail: "inputs directory does not exist".to_string(),
            });
        }
        for inp in &cert.inputs {
            let rel = Path::new(&inp.locator);
            if rel.is_absolute()
                || rel.components().any(|c| {
                    matches!(
                        c,
                        Component::ParentDir | Component::RootDir | Component::Prefix(_)
                    )
                })
            {
                return Err(VerifyError::Tamper {
                    kind: "certificate_input",
                    detail: format!("unsafe input locator {:?}", inp.locator),
                });
            }
            let full = base.join(rel);
            let data = fs::read(&full).map_err(|e| VerifyError::Tamper {
                kind: "certificate_input",
                detail: format!(
                    "declared input '{}' ({}) unreadable: {e}",
                    inp.locator, inp.role
                ),
            })?;
            let actual = sha256(&data);
            if actual != inp.sha256 {
                return Err(VerifyError::Tamper {
                    kind: "certificate_input",
                    detail: format!(
                        "input '{}' ({}) digest mismatch: expected {}, actual {}",
                        inp.locator,
                        inp.role,
                        inp.sha256.to_hex(),
                        actual.to_hex()
                    ),
                });
            }
        }
    }
    Ok(cert)
}

/// Offline read-only verification of a published `FMT-CAS-1` object root (`PO-BED-3`):
/// decodes `manifests/<aa>/<root>.casm`, recomputes the fanout-16 Merkle root, and verifies every
/// chunk file on disk against its `ChunkRef` length and SHA-256 digest.
pub fn verify_cas_root(
    cas_dir: &Path,
    expected_root: &Digest32,
) -> Result<CasManifest, VerifyError> {
    if !cas_dir.is_dir() {
        return Err(VerifyError::MissingOrUnreadable {
            path: cas_dir.display().to_string(),
            detail: "CAS directory does not exist".to_string(),
        });
    }
    let root_hex = expected_root.to_hex();
    let manifest_path = cas_dir
        .join("manifests")
        .join(&root_hex[..2])
        .join(format!("{root_hex}.casm"));
    let manifest_bytes = read_input_bytes(&manifest_path)?;
    let manifest = CasManifest::from_bytes(&manifest_bytes).map_err(|e| VerifyError::Tamper {
        kind: "cas_manifest",
        detail: e.to_string(),
    })?;
    if manifest.root_digest != *expected_root {
        return Err(VerifyError::Tamper {
            kind: "cas_manifest",
            detail: format!(
                "manifest root {} does not match requested root {}",
                manifest.root_digest.to_hex(),
                expected_root.to_hex()
            ),
        });
    }
    for (idx, chunk) in manifest.chunks.iter().enumerate() {
        let hex = chunk.sha256.to_hex();
        let chunk_path = cas_dir
            .join("chunks")
            .join(&hex[..2])
            .join(format!("{hex}.chunk"));
        let data = fs::read(&chunk_path).map_err(|e| VerifyError::Tamper {
            kind: "cas_chunk",
            detail: format!("published chunk #{idx} ({hex}) missing or unreadable: {e}"),
        })?;
        if data.len() != chunk.len as usize {
            return Err(VerifyError::Tamper {
                kind: "cas_chunk",
                detail: format!(
                    "chunk #{idx} ({hex}) byte length mismatch: expected {}, actual {}",
                    chunk.len,
                    data.len()
                ),
            });
        }
        let actual = sha256(&data);
        if actual != chunk.sha256 {
            return Err(VerifyError::Tamper {
                kind: "cas_chunk",
                detail: format!(
                    "chunk #{idx} digest mismatch: expected {}, actual {}",
                    chunk.sha256.to_hex(),
                    actual.to_hex()
                ),
            });
        }
    }
    Ok(manifest)
}

/// Verify raw `SyncCert` (`FMNE` + `SCER`) bytes and its RFC 6962 MMR consistency proof (`PO-BED-2`).
pub fn verify_sync_cert_bytes(bytes: &[u8]) -> Result<SyncCert, VerifyError> {
    let cert = SyncCert::from_bytes(bytes).map_err(|e| VerifyError::Tamper {
        kind: "synccert",
        detail: e.to_string(),
    })?;
    cert.verify_prefix_consistency()
        .map_err(|e| VerifyError::Tamper {
            kind: "synccert_mmr",
            detail: e.to_string(),
        })?;
    Ok(cert)
}

/// Verify a `SyncCert` file on disk (`PO-BED-2`).
pub fn verify_sync_cert_file(path: &Path) -> Result<SyncCert, VerifyError> {
    let bytes = read_input_bytes(path)?;
    verify_sync_cert_bytes(&bytes)
}

/// Verify an `FMT-SEG-1` ledger segment file on disk (header, batch CRC32C, hash chain, fencing
/// epochs, MMR footer). Any torn tail or corruption is refused with [`VerifyError::Tamper`].
pub fn verify_segment_file(path: &Path) -> Result<SegmentVerification, VerifyError> {
    match fs::metadata(path) {
        Ok(m) if m.is_file() => {}
        Ok(_) => {
            return Err(VerifyError::MissingOrUnreadable {
                path: path.display().to_string(),
                detail: "path is not a regular file".to_string(),
            });
        }
        Err(e) if e.kind() == ErrorKind::NotFound => {
            return Err(VerifyError::MissingOrUnreadable {
                path: path.display().to_string(),
                detail: e.to_string(),
            });
        }
        Err(e) => {
            return Err(VerifyError::MissingOrUnreadable {
                path: path.display().to_string(),
                detail: e.to_string(),
            });
        }
    }
    let report = verify_segment(path).map_err(|e| VerifyError::Tamper {
        kind: "segment",
        detail: e.to_string(),
    })?;
    if report.torn_bytes > 0 || report.issue.is_some() {
        return Err(VerifyError::Tamper {
            kind: "segment",
            detail: format!(
                "segment has {} unverified trailing bytes ({})",
                report.torn_bytes,
                report.issue.as_deref().unwrap_or("torn tail")
            ),
        });
    }
    Ok(report)
}

/// Offline RFC 6962 MMR inclusion proof check helper.
pub fn verify_mmr_inclusion(
    leaf_index: u64,
    tree_size: u64,
    leaf_data: &[u8],
    expected_root: &Digest32,
    proof: &[Digest32],
) -> Result<(), VerifyError> {
    let leaf = leaf_hash(leaf_data);
    verify_inclusion(leaf_index, tree_size, &leaf, expected_root, proof).map_err(|e| {
        VerifyError::Tamper {
            kind: "mmr_inclusion",
            detail: e.to_string(),
        }
    })
}
