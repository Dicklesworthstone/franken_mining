//! Determinism: BitwisePortable
//! `FMT-CAS-1` chunk manifest codec (`FMNE` + `CASM` + CRC32C, `BD-090`, plan §26.5).

use core::fmt;

use fmine_core::codec::{CodecError, Decoder, Encoder};
use fmine_core::hash::{Digest32, sha256};
use fmine_evidence::{Refusal, RefusalCode};

use crate::merkle::fanout16_root;

/// Format tag for `FMT-CAS-1` manifests (`FMNE` + `CASM`).
pub const TAG_CAS_MANIFEST: [u8; 4] = *b"CASM";

/// Current schema version for `FMT-CAS-1`.
pub const FMT_CAS_V1: u16 = 1;

/// Minimum supported schema version (`N-1`).
pub const FMT_CAS_MIN_VERSION: u16 = 0;

/// Default chunk size (4 MiB, plan §26.5).
pub const DEFAULT_CHUNK_BYTES: usize = 4 * 1024 * 1024;

/// Maximum permitted chunk size (16 MiB).
pub const MAX_CHUNK_BYTES: usize = 16 * 1024 * 1024;

/// Maximum chunks permitted in one manifest.
pub const MAX_CHUNKS_PER_MANIFEST: usize = 1_048_576;

/// Maximum UTF-8 byte length of a `media_type` string.
pub const MAX_MEDIA_TYPE_BYTES: usize = 256;

/// Typed errors across `fmine-object` manifest, CAS store, and GC operations.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CasError {
    /// Canonical `FMNE` codec refusal.
    Codec(CodecError),
    /// Invalid field or structural invariant violation.
    InvalidField { field: &'static str, detail: String },
    /// Fanout-16 Merkle root mismatch.
    RootMismatch {
        expected: Digest32,
        actual: Digest32,
    },
    /// Required chunk file is missing on disk (`PO-BED-3` prevents publishing partial root).
    MissingChunk { sha256: Digest32 },
    /// Chunk file bytes on disk do not hash to its declared SHA-256.
    ChunkDigestMismatch {
        expected: Digest32,
        actual: Digest32,
    },
    /// Manifest root not found in published CAS store.
    ManifestNotFound { root: Digest32 },
    /// Filesystem I/O refusal.
    Io {
        operation: &'static str,
        detail: String,
    },
}

impl CasError {
    /// Convert to a registered `FMINE-REF-*` refusal.
    #[must_use]
    pub fn to_refusal(&self) -> Refusal {
        let code = match self {
            Self::Codec(_)
            | Self::InvalidField { .. }
            | Self::RootMismatch { .. }
            | Self::ChunkDigestMismatch { .. } => RefusalCode::DigestMismatch505,
            Self::MissingChunk { .. } | Self::ManifestNotFound { .. } | Self::Io { .. } => {
                RefusalCode::InputMissingOrUnreadable506
            }
        };
        Refusal::new(code, self.to_string())
    }
}

impl From<CodecError> for CasError {
    fn from(value: CodecError) -> Self {
        Self::Codec(value)
    }
}

impl fmt::Display for CasError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Codec(err) => write!(f, "FMT-CAS-1 codec error: {err}"),
            Self::InvalidField { field, detail } => write!(f, "invalid {field}: {detail}"),
            Self::RootMismatch { expected, actual } => {
                write!(
                    f,
                    "CAS fanout-16 root mismatch: stored {expected} != computed {actual}"
                )
            }
            Self::MissingChunk { sha256 } => write!(f, "missing CAS chunk {sha256}"),
            Self::ChunkDigestMismatch { expected, actual } => {
                write!(
                    f,
                    "CAS chunk digest mismatch: expected {expected}, computed {actual}"
                )
            }
            Self::ManifestNotFound { root } => write!(f, "CAS manifest root {root} not published"),
            Self::Io { operation, detail } => write!(f, "CAS I/O error ({operation}): {detail}"),
        }
    }
}

impl std::error::Error for CasError {}

/// One chunk reference inside a [`CasManifest`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ChunkRef {
    /// Byte offset inside the reassembled object.
    pub offset: u64,
    /// Byte length of this chunk (`> 0`).
    pub len: u32,
    /// SHA-256 digest of this chunk's raw bytes.
    pub sha256: Digest32,
}

/// Canonical `FMT-CAS-1` content-addressed object manifest (`BD-090`, plan §26.5).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct CasManifest {
    /// Schema version (`1` for `FMT-CAS-1`; `0` supported for N-1 compatibility).
    pub format_version: u16,
    /// Content media type (e.g. `"application/octet-stream"`, `"application/vnd.fmine.col1"`).
    pub media_type: String,
    /// Total byte length across all chunks.
    pub total_bytes: u64,
    /// Nominal chunk size used when slicing (`1..=MAX_CHUNK_BYTES`).
    pub chunk_size: u32,
    /// Fanout-16 Merkle root over `chunks[i].sha256`.
    pub root_digest: Digest32,
    /// Ordered contiguous chunk descriptors.
    pub chunks: Vec<ChunkRef>,
}

impl CasManifest {
    /// Build a canonical `FMT-CAS-1` manifest from chunk descriptors and compute its fanout-16 root.
    pub fn from_chunks(
        media_type: impl Into<String>,
        chunk_size: u32,
        chunks: Vec<ChunkRef>,
    ) -> Result<Self, CasError> {
        let digests: Vec<Digest32> = chunks.iter().map(|c| c.sha256).collect();
        let total_bytes = chunks
            .iter()
            .try_fold(0u64, |acc, c| acc.checked_add(u64::from(c.len)))
            .ok_or_else(|| CasError::InvalidField {
                field: "total_bytes",
                detail: "chunk lengths overflow u64".to_string(),
            })?;
        let manifest = Self {
            format_version: FMT_CAS_V1,
            media_type: media_type.into(),
            total_bytes,
            chunk_size,
            root_digest: fanout16_root(&digests),
            chunks,
        };
        manifest.validate()?;
        Ok(manifest)
    }

    /// Validate structural invariants, chunk contiguity, and fanout-16 Merkle root equality.
    pub fn validate(&self) -> Result<(), CasError> {
        if self.format_version > FMT_CAS_V1 {
            return Err(CasError::Codec(CodecError::UnsupportedVersion {
                actual: self.format_version,
                min_supported: FMT_CAS_MIN_VERSION,
                max_supported: FMT_CAS_V1,
            }));
        }
        if self.media_type.is_empty() || self.media_type.len() > MAX_MEDIA_TYPE_BYTES {
            return Err(CasError::InvalidField {
                field: "media_type",
                detail: format!(
                    "length {} outside 1..={MAX_MEDIA_TYPE_BYTES}",
                    self.media_type.len()
                ),
            });
        }
        if self.chunk_size == 0 || (self.chunk_size as usize) > MAX_CHUNK_BYTES {
            return Err(CasError::InvalidField {
                field: "chunk_size",
                detail: format!(
                    "chunk_size {} outside 1..={MAX_CHUNK_BYTES}",
                    self.chunk_size
                ),
            });
        }
        if self.chunks.len() > MAX_CHUNKS_PER_MANIFEST {
            return Err(CasError::Codec(CodecError::LengthOutOfBounds {
                len: self.chunks.len(),
                max: MAX_CHUNKS_PER_MANIFEST,
            }));
        }
        let mut cursor = 0u64;
        let mut digests = Vec::with_capacity(self.chunks.len());
        for (i, chunk) in self.chunks.iter().enumerate() {
            if chunk.len == 0 || chunk.len > self.chunk_size {
                return Err(CasError::InvalidField {
                    field: "chunk.len",
                    detail: format!(
                        "chunk {i} length {} outside 1..={}",
                        chunk.len, self.chunk_size
                    ),
                });
            }
            if chunk.offset != cursor {
                return Err(CasError::InvalidField {
                    field: "chunk.offset",
                    detail: format!("chunk {i} offset {} != expected {cursor}", chunk.offset),
                });
            }
            cursor =
                cursor
                    .checked_add(u64::from(chunk.len))
                    .ok_or_else(|| CasError::InvalidField {
                        field: "chunk.offset",
                        detail: "offset overflow".to_string(),
                    })?;
            digests.push(chunk.sha256);
        }
        if cursor != self.total_bytes {
            return Err(CasError::InvalidField {
                field: "total_bytes",
                detail: format!("declared {} != sum of chunks {cursor}", self.total_bytes),
            });
        }
        let computed = fanout16_root(&digests);
        if computed != self.root_digest {
            return Err(CasError::RootMismatch {
                expected: self.root_digest,
                actual: computed,
            });
        }
        Ok(())
    }

    /// Encode this manifest as a canonical `FMNE` + `CASM` + CRC32C frame.
    pub fn to_bytes(&self) -> Result<Vec<u8>, CasError> {
        self.validate()?;
        let mut enc = Encoder::new_frame(TAG_CAS_MANIFEST, self.format_version);
        enc.put_str(&self.media_type);
        enc.put_u64(self.total_bytes);
        enc.put_u32(self.chunk_size);
        enc.put_fixed(self.root_digest.as_bytes());
        let count = u32::try_from(self.chunks.len()).map_err(|_| {
            CasError::Codec(CodecError::LengthOutOfBounds {
                len: self.chunks.len(),
                max: MAX_CHUNKS_PER_MANIFEST,
            })
        })?;
        enc.put_u32(count);
        for chunk in &self.chunks {
            enc.put_u64(chunk.offset);
            enc.put_u32(chunk.len);
            enc.put_fixed(chunk.sha256.as_bytes());
        }
        Ok(enc.finish())
    }

    /// Decode a canonical `FMT-CAS-1` (or N-1 `v0`) manifest frame and verify its Merkle root.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, CasError> {
        let mut dec =
            Decoder::open_frame(bytes, TAG_CAS_MANIFEST, FMT_CAS_MIN_VERSION, FMT_CAS_V1)?;
        let format_version = dec.version();
        let media_type = dec.get_str(MAX_MEDIA_TYPE_BYTES)?.to_string();
        let total_bytes = dec.get_u64()?;
        let chunk_size = dec.get_u32()?;
        let root_digest = Digest32(dec.get_fixed::<32>()?);
        let count = dec.get_u32()? as usize;
        if count > MAX_CHUNKS_PER_MANIFEST {
            return Err(CasError::Codec(CodecError::LengthOutOfBounds {
                len: count,
                max: MAX_CHUNKS_PER_MANIFEST,
            }));
        }
        let mut chunks = Vec::with_capacity(count.min(4096));
        for _ in 0..count {
            let offset = dec.get_u64()?;
            let len = dec.get_u32()?;
            let sha256 = Digest32(dec.get_fixed::<32>()?);
            chunks.push(ChunkRef {
                offset,
                len,
                sha256,
            });
        }
        dec.finish()?;
        let manifest = Self {
            format_version,
            media_type,
            total_bytes,
            chunk_size,
            root_digest,
            chunks,
        };
        manifest.validate()?;
        Ok(manifest)
    }

    /// Canonical SHA-256 digest of the serialized manifest frame.
    pub fn manifest_sha256(&self) -> Result<Digest32, CasError> {
        Ok(sha256(&self.to_bytes()?))
    }
}

/// Slice raw bytes into chunk descriptors using `chunk_size` (`1..=MAX_CHUNK_BYTES`).
pub fn chunk_bytes(bytes: &[u8], chunk_size: usize) -> Result<Vec<(ChunkRef, &[u8])>, CasError> {
    if chunk_size == 0 || chunk_size > MAX_CHUNK_BYTES {
        return Err(CasError::InvalidField {
            field: "chunk_size",
            detail: format!("chunk_size {chunk_size} outside 1..={MAX_CHUNK_BYTES}"),
        });
    }
    let mut out = Vec::with_capacity(bytes.len().div_ceil(chunk_size));
    let mut offset = 0u64;
    for slice in bytes.chunks(chunk_size) {
        let len = slice.len() as u32;
        out.push((
            ChunkRef {
                offset,
                len,
                sha256: sha256(slice),
            },
            slice,
        ));
        offset += u64::from(len);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fmt_cas_1_manifest_roundtrip_n_minus_1_and_tamper_refusal() {
        let payload: Vec<u8> = (0..10_000usize).map(|i| (i * 31) as u8).collect();
        let sliced = chunk_bytes(&payload, 512).expect("chunk");
        assert_eq!(sliced.len(), 20); // spans 2 fanout-16 levels!
        let refs: Vec<ChunkRef> = sliced.into_iter().map(|(r, _)| r).collect();
        let manifest = CasManifest::from_chunks("application/octet-stream", 512, refs)
            .expect("build manifest");

        let bytes = manifest.to_bytes().expect("encode");
        let decoded = CasManifest::from_bytes(&bytes).expect("decode");
        assert_eq!(decoded, manifest);
        assert_eq!(decoded.to_bytes().expect("re-encode"), bytes);

        // Single bit flip anywhere refuses.
        for i in 0..bytes.len() {
            let mut bad = bytes.clone();
            bad[i] ^= 0x01;
            assert!(CasManifest::from_bytes(&bad).is_err(), "bit flip at {i}");
        }

        // N-1 (v0) round-trip.
        let mut v0 = manifest;
        v0.format_version = 0;
        let v0_bytes = v0.to_bytes().expect("encode v0");
        assert_eq!(CasManifest::from_bytes(&v0_bytes).expect("decode v0"), v0);
    }
}
