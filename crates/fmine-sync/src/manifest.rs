//! Determinism: BitwisePortable
//!
//! Chunk want/have manifest anti-entropy (`BD-104`, plan §26.13 GB-36).
//!
//! GB-36 proves that plain chunk-digest manifests beat Merkle set reconciliation on the hot path
//! whenever the changed fraction exceeds `1 / (depth * fanout)` (for an 8 GiB / 2,048-chunk block
//! model with fanout 16 and depth 3, plain manifest = ~64 KiB vs 30%-changed Merkle diff = ~0.94 MB).
//! Merkle/MMR trees earn their keep as proofs (`fanout16_root`, `SyncCert`), not as diff engines.

use std::collections::BTreeSet;

use fmine_core::codec::{Decoder, Encoder};
use fmine_core::hash::Digest32;
use fmine_object::cas::{CasManifest, MAX_CHUNKS_PER_MANIFEST};
use fmine_object::merkle::{FANOUT, fanout16_root};

use crate::SyncError;

/// Canonical wire frame tag for chunk want/have manifests (`FMNE` + `SYNM` + CRC32C).
pub const TAG_SYNC_MANIFEST: [u8; 4] = *b"SYNM";
const FMT_SYNC_MANIFEST_V1: u16 = 1;

/// Compact plain chunk-digest manifest exchanged during object anti-entropy (`BD-104`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WantHaveManifest {
    pub root_digest: Digest32,
    pub total_bytes: u64,
    pub chunk_size: u32,
    pub chunks: Vec<(Digest32, u32)>,
}

impl WantHaveManifest {
    /// Extracts a compact want/have manifest from a verified [`CasManifest`].
    pub fn from_cas_manifest(manifest: &CasManifest) -> Result<Self, SyncError> {
        manifest.validate()?;
        let chunks = manifest.chunks.iter().map(|c| (c.sha256, c.len)).collect();
        Ok(Self {
            root_digest: manifest.root_digest,
            total_bytes: manifest.total_bytes,
            chunk_size: manifest.chunk_size,
            chunks,
        })
    }

    /// Builds a want/have manifest directly from `(chunk_digest, chunk_len)` pairs and computes
    /// the fanout-16 Merkle root so receivers validate authenticity on decode.
    pub fn from_chunk_digests(
        chunk_size: u32,
        chunks: Vec<(Digest32, u32)>,
    ) -> Result<Self, SyncError> {
        if chunk_size == 0 {
            return Err(SyncError::InvalidField {
                field: "chunk_size",
                detail: "must be > 0".to_string(),
            });
        }
        if chunks.len() > MAX_CHUNKS_PER_MANIFEST {
            return Err(SyncError::InvalidField {
                field: "chunks",
                detail: format!("count {} exceeds {MAX_CHUNKS_PER_MANIFEST}", chunks.len()),
            });
        }
        let mut total_bytes = 0u64;
        let mut digests = Vec::with_capacity(chunks.len());
        for &(d, len) in &chunks {
            if len == 0 {
                return Err(SyncError::InvalidField {
                    field: "chunk.len",
                    detail: "must be > 0".to_string(),
                });
            }
            total_bytes =
                total_bytes
                    .checked_add(u64::from(len))
                    .ok_or_else(|| SyncError::InvalidField {
                        field: "total_bytes",
                        detail: "u64 overflow".to_string(),
                    })?;
            digests.push(d);
        }
        Ok(Self {
            root_digest: fanout16_root(&digests),
            total_bytes,
            chunk_size,
            chunks,
        })
    }

    /// Encodes this manifest into canonical `FMNE` + `SYNM` + CRC32C wire bytes.
    #[must_use]
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut enc = Encoder::new_frame(TAG_SYNC_MANIFEST, FMT_SYNC_MANIFEST_V1);
        enc.put_fixed(self.root_digest.as_bytes());
        enc.put_u64(self.total_bytes);
        enc.put_u32(self.chunk_size);
        enc.put_u32(self.chunks.len() as u32);
        for &(digest, len) in &self.chunks {
            enc.put_fixed(digest.as_bytes());
            enc.put_u32(len);
        }
        enc.finish()
    }

    /// Decodes canonical `FMNE` + `SYNM` bytes and verifies fanout-16 root and total byte sum.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, SyncError> {
        let mut dec = Decoder::open_frame(
            bytes,
            TAG_SYNC_MANIFEST,
            FMT_SYNC_MANIFEST_V1,
            FMT_SYNC_MANIFEST_V1,
        )?;
        let root_digest = Digest32(dec.get_fixed::<32>()?);
        let total_bytes = dec.get_u64()?;
        let chunk_size = dec.get_u32()?;
        let count = dec.get_u32()? as usize;
        if count > MAX_CHUNKS_PER_MANIFEST {
            return Err(SyncError::InvalidField {
                field: "chunks",
                detail: format!("count {count} exceeds {MAX_CHUNKS_PER_MANIFEST}"),
            });
        }
        let mut chunks = Vec::with_capacity(count);
        let mut digests = Vec::with_capacity(count);
        let mut sum_bytes = 0u64;
        for _ in 0..count {
            let d = Digest32(dec.get_fixed::<32>()?);
            let len = dec.get_u32()?;
            if len == 0 {
                return Err(SyncError::InvalidField {
                    field: "chunk.len",
                    detail: "must be > 0".to_string(),
                });
            }
            sum_bytes =
                sum_bytes
                    .checked_add(u64::from(len))
                    .ok_or_else(|| SyncError::InvalidField {
                        field: "total_bytes",
                        detail: "u64 overflow".to_string(),
                    })?;
            digests.push(d);
            chunks.push((d, len));
        }
        dec.finish()?;
        if sum_bytes != total_bytes {
            return Err(SyncError::InvalidField {
                field: "total_bytes",
                detail: format!("header {total_bytes} != sum {sum_bytes}"),
            });
        }
        let computed = fanout16_root(&digests);
        if computed != root_digest {
            return Err(SyncError::InvalidField {
                field: "root_digest",
                detail: format!("header {root_digest} != fanout-16 root {computed}"),
            });
        }
        Ok(Self {
            root_digest,
            total_bytes,
            chunk_size,
            chunks,
        })
    }

    /// Raw digest-only wire payload size (`32 * N` bytes, matching GB-36's 64 KiB for 2,048 chunks).
    #[must_use]
    pub fn raw_digest_bytes(&self) -> usize {
        self.chunks.len() * 32
    }
}

/// Ordered list of missing chunks requested by a receiver after diffing against its local CAS store.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WantRequest {
    pub root_digest: Digest32,
    pub missing_chunks: Vec<Digest32>,
    pub missing_bytes: u64,
    pub already_have_chunks: usize,
}

/// Computes the exact deduplicated `WantRequest` for `remote` given the receiver's `local_have` set.
#[must_use]
pub fn diff_manifest(remote: &WantHaveManifest, local_have: &BTreeSet<Digest32>) -> WantRequest {
    let mut missing_chunks = Vec::new();
    let mut seen_missing = BTreeSet::new();
    let mut missing_bytes = 0u64;
    let mut already_have_chunks = 0usize;

    for &(digest, len) in &remote.chunks {
        if local_have.contains(&digest) {
            already_have_chunks += 1;
        } else if seen_missing.insert(digest) {
            missing_chunks.push(digest);
            missing_bytes = missing_bytes.saturating_add(u64::from(len));
        }
    }

    WantRequest {
        root_digest: remote.root_digest,
        missing_chunks,
        missing_bytes,
        already_have_chunks,
    }
}

/// Measured wire-size comparison proving plan §26.13 GB-36 (checked null on Merkle set reconciliation).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Gb36SizeReport {
    pub num_chunks: usize,
    pub changed_chunks: usize,
    pub tree_depth: usize,
    pub fanout: usize,
    /// Actual `FMNE` + `SYNM` encoded manifest bytes on the wire.
    pub encoded_manifest_bytes: usize,
    /// Raw digest table bytes (`num_chunks * 32`).
    pub raw_manifest_bytes: usize,
    /// Equivalent Merkle set-diff exchange bytes (`changed_chunks * tree_depth * fanout * 32`).
    pub merkle_diff_bytes: usize,
    /// Per-round 12-cell version-vector + 24-hash anti-entropy bytes.
    pub cell_anti_entropy_bytes: usize,
    /// True when sending the full encoded manifest uses fewer wire bytes than a Merkle diff.
    pub plain_manifest_wins: bool,
}

/// Measures actual `WantHaveManifest` wire bytes against the GB-36 Merkle set-diff formula.
pub fn measure_gb36_comparison(
    manifest: &WantHaveManifest,
    changed_chunks: usize,
    cells: usize,
    vv_hashes_per_cell: usize,
) -> Gb36SizeReport {
    let n = manifest.chunks.len();
    let tree_depth = fanout16_depth(n);
    let encoded_manifest_bytes = manifest.to_bytes().len();
    let raw_manifest_bytes = manifest.raw_digest_bytes();
    let merkle_diff_bytes = changed_chunks
        .saturating_mul(tree_depth)
        .saturating_mul(FANOUT)
        .saturating_mul(32);
    // GB-36: per cell, 24 B version-vector entry + vv_hashes_per_cell * 32 B.
    let cell_anti_entropy_bytes = cells.saturating_mul(24 + vv_hashes_per_cell * 32);

    Gb36SizeReport {
        num_chunks: n,
        changed_chunks,
        tree_depth,
        fanout: FANOUT,
        encoded_manifest_bytes,
        raw_manifest_bytes,
        merkle_diff_bytes,
        cell_anti_entropy_bytes,
        plain_manifest_wins: encoded_manifest_bytes < merkle_diff_bytes,
    }
}

fn fanout16_depth(n: usize) -> usize {
    if n <= 1 {
        return 1;
    }
    let mut depth = 0usize;
    let mut width = n;
    while width > 1 {
        width = width.div_ceil(FANOUT);
        depth += 1;
    }
    depth
}

#[cfg(test)]
mod tests {
    use super::*;
    use fmine_core::hash::sha256;

    /// BD-104 done-when (`GB-36 sizes measured`): verifies WantHaveManifest canonical codec,
    /// deduplicated diffing, tamper refusal, and the exact GB-36 8 GiB / 2,048-chunk & 12-cell
    /// wire-size comparison proving plain manifests beat Merkle set reconciliation at 30% churn.
    #[test]
    fn test_chunk_want_have_anti_entropy_and_gb36_wire_sizes() {
        // Build the exact GB-36 2,048-chunk (8 GiB at 4 MiB/chunk) block-model manifest.
        let chunk_size: u32 = 4 * 1024 * 1024;
        let chunks: Vec<(Digest32, u32)> = (0u32..2048)
            .map(|i| {
                let d = sha256(&i.to_le_bytes());
                (d, chunk_size)
            })
            .collect();
        let manifest =
            WantHaveManifest::from_chunk_digests(chunk_size, chunks.clone()).expect("manifest");
        assert_eq!(manifest.total_bytes, 2048 * u64::from(chunk_size));

        // Roundtrip canonical FMNE+SYNM frame and verify single-bit tamper refusal.
        let wire = manifest.to_bytes();
        let decoded = WantHaveManifest::from_bytes(&wire).expect("decode");
        assert_eq!(decoded, manifest);
        let mut tampered = wire.clone();
        tampered[40] ^= 0x01;
        assert!(WantHaveManifest::from_bytes(&tampered).is_err());

        // GB-36 exact size measurement: 2,048 chunks with 30% changed (d = 614), 12 cells × 24 hashes.
        let gb36 = measure_gb36_comparison(&manifest, 614, 12, 24);
        assert_eq!(gb36.num_chunks, 2048);
        assert_eq!(gb36.tree_depth, 3);
        assert_eq!(gb36.fanout, 16);
        assert_eq!(gb36.raw_manifest_bytes, 65_536); // exactly 64 KiB
        assert!(gb36.encoded_manifest_bytes < 75_000); // ~73.8 KiB including 4-byte chunk lens + frame
        assert_eq!(gb36.merkle_diff_bytes, 614 * 3 * 16 * 32); // 943,104 B (~0.94 MB)
        assert_eq!(gb36.cell_anti_entropy_bytes, 12 * (24 + 24 * 32)); // 9,504 B (~9.5 KB)
        assert!(
            gb36.plain_manifest_wins,
            "plain manifest ({}) must beat 30%-churn Merkle diff ({})",
            gb36.encoded_manifest_bytes, gb36.merkle_diff_bytes
        );

        // Anti-entropy diff with 70% local overlap (1,434 have, 614 want).
        let local_have: BTreeSet<Digest32> = chunks.iter().take(1434).map(|(d, _)| *d).collect();
        let want = diff_manifest(&decoded, &local_have);
        assert_eq!(want.already_have_chunks, 1434);
        assert_eq!(want.missing_chunks.len(), 614);
        assert_eq!(want.missing_bytes, 614 * u64::from(chunk_size));
        assert_eq!(want.root_digest, manifest.root_digest);
    }
}
