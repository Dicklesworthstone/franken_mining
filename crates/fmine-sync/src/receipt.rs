//! Determinism: Exact
//!
//! Durable `OB-SEAL` chunk receipts and hard-kill resumable transfer (`BD-106`, plan §26.7 S3(e),
//! `PO-BED-3`).
//!
//! Three-phase transfer lifecycle:
//! 1. *Request:* reserve transfer lease & `OB-SEAL` durable receipt before moving chunks.
//! 2. *Drain:* stream chunks into [`CasStore`] staging, verifying each chunk digest and recording
//!    its receipt durably; a hard kill mid-drain leaves no published root (`PO-BED-3`) and resumes
//!    from the receipt without relying on ATP 0.6.0 journaling (defect S3(e)).
//! 3. *Finalize:* publish the root only when 100% of chunks are verified, release the transfer
//!    lease, and return a [`QuiescenceProof`].

use std::collections::BTreeSet;
use std::fs;
use std::io::Write;
use std::path::Path;

use fmine_core::codec::{Decoder, Encoder};
use fmine_core::hash::{Digest32, sha256};
use fmine_object::cas::{CasError, CasManifest, ChunkRef};
use fmine_object::store::CasStore;

use crate::SyncError;

/// Canonical wire frame tag for persisted `OB-SEAL` transfer receipts (`FMNE` + `OBSR` + CRC32C).
pub const TAG_OB_SEAL_RECEIPT: [u8; 4] = *b"OBSR";
const FMT_OBSR_V1: u16 = 1;

/// Phase of a resumable `OB-SEAL` transfer (`BD-106`, plan §26.7).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransferPhase {
    Requested,
    Draining,
    Finalized,
}

impl TransferPhase {
    const fn tag(self) -> u8 {
        match self {
            Self::Requested => 1,
            Self::Draining => 2,
            Self::Finalized => 3,
        }
    }

    fn from_tag(tag: u8) -> Result<Self, SyncError> {
        match tag {
            1 => Ok(Self::Requested),
            2 => Ok(Self::Draining),
            3 => Ok(Self::Finalized),
            other => Err(SyncError::InvalidField {
                field: "phase",
                detail: format!("unknown TransferPhase tag {other}"),
            }),
        }
    }
}

/// Durable `OB-SEAL` chunk receipt persisted across crashes (`BD-106`, `S3(e)`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObSealReceipt {
    pub transfer_id: String,
    pub lease_id: String,
    pub phase: TransferPhase,
    pub manifest: CasManifest,
    pub verified_chunks: BTreeSet<Digest32>,
}

impl ObSealReceipt {
    /// Encodes the receipt as canonical `FMNE` + `OBSR` + CRC32C bytes.
    pub fn to_bytes(&self) -> Result<Vec<u8>, SyncError> {
        let manifest_bytes = self.manifest.to_bytes()?;
        let mut enc = Encoder::new_frame(TAG_OB_SEAL_RECEIPT, FMT_OBSR_V1);
        enc.put_str(&self.transfer_id);
        enc.put_str(&self.lease_id);
        enc.put_u8(self.phase.tag());
        enc.put_bytes(&manifest_bytes);
        enc.put_u32(self.verified_chunks.len() as u32);
        for d in &self.verified_chunks {
            enc.put_fixed(d.as_bytes());
        }
        Ok(enc.finish())
    }

    /// Decodes a canonical `FMNE` + `OBSR` receipt.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, SyncError> {
        let mut dec = Decoder::open_frame(bytes, TAG_OB_SEAL_RECEIPT, FMT_OBSR_V1, FMT_OBSR_V1)?;
        let transfer_id = dec.get_str(256)?.to_string();
        let lease_id = dec.get_str(256)?.to_string();
        if transfer_id.trim().is_empty() || lease_id.trim().is_empty() {
            return Err(SyncError::InvalidField {
                field: "transfer_id/lease_id",
                detail: "must be non-empty".to_string(),
            });
        }
        let phase = TransferPhase::from_tag(dec.get_u8()?)?;
        let manifest_bytes = dec.get_bytes(16 * 1024 * 1024)?;
        let manifest = CasManifest::from_bytes(manifest_bytes)?;
        manifest.validate()?;
        let manifest_digests: BTreeSet<Digest32> =
            manifest.chunks.iter().map(|c| c.sha256).collect();
        let count = dec.get_u32()? as usize;
        if count > manifest_digests.len() {
            return Err(SyncError::InvalidField {
                field: "verified_chunks",
                detail: format!(
                    "count {count} exceeds distinct manifest chunks {}",
                    manifest_digests.len()
                ),
            });
        }
        let mut verified_chunks = BTreeSet::new();
        let mut prev_digest: Option<Digest32> = None;
        for _ in 0..count {
            let d = Digest32(dec.get_fixed::<32>()?);
            if let Some(prev) = prev_digest
                && prev >= d
            {
                return Err(SyncError::InvalidField {
                    field: "verified_chunks",
                    detail: "duplicate or out-of-canonical-order verified chunk digest".to_string(),
                });
            }
            if !manifest_digests.contains(&d) {
                return Err(SyncError::InvalidField {
                    field: "verified_chunks",
                    detail: format!("verified chunk {d} is not referenced by manifest"),
                });
            }
            prev_digest = Some(d);
            verified_chunks.insert(d);
        }
        dec.finish()?;
        if phase == TransferPhase::Finalized && verified_chunks.len() != manifest_digests.len() {
            return Err(SyncError::InvalidField {
                field: "phase",
                detail: "finalized OB-SEAL receipt has unverified manifest chunks".to_string(),
            });
        }
        Ok(Self {
            transfer_id,
            lease_id,
            phase,
            manifest,
            verified_chunks,
        })
    }

    /// Returns the ordered list of chunks not yet durably verified in this transfer.
    #[must_use]
    pub fn remaining_chunks(&self) -> Vec<ChunkRef> {
        self.manifest
            .chunks
            .iter()
            .copied()
            .filter(|c| !self.verified_chunks.contains(&c.sha256))
            .collect()
    }
}

/// Quiescence proof emitted only when finalize publishes the complete verified root and releases
/// the `OB-SEAL` obligation (`BD-106`, `PO-BED-5`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuiescenceProof {
    pub transfer_id: String,
    pub lease_id: String,
    pub published_root: Digest32,
    pub total_chunks: usize,
    pub proof_digest: Digest32,
}

/// Crash-safe request -> drain -> finalize transfer engine backed by [`CasStore`] and a durable
/// `OB-SEAL` receipt file (`BD-106`).
#[derive(Debug, Clone)]
pub struct ResumableTransfer {
    receipt: ObSealReceipt,
}

impl ResumableTransfer {
    /// Phase 1 (*Request*): reserve transfer lease and persist the initial `OB-SEAL` receipt.
    pub fn start_request(
        transfer_id: impl Into<String>,
        lease_id: impl Into<String>,
        manifest: CasManifest,
        receipt_path: &Path,
    ) -> Result<Self, SyncError> {
        manifest.validate()?;
        let transfer_id = transfer_id.into();
        let lease_id = lease_id.into();
        if transfer_id.trim().is_empty() || lease_id.trim().is_empty() {
            return Err(SyncError::InvalidField {
                field: "transfer_id/lease_id",
                detail: "must be non-empty".to_string(),
            });
        }
        let receipt = ObSealReceipt {
            transfer_id,
            lease_id,
            phase: TransferPhase::Requested,
            manifest,
            verified_chunks: BTreeSet::new(),
        };
        persist_receipt(&receipt, receipt_path)?;
        Ok(Self { receipt })
    }

    /// Resumes an interrupted transfer directly from its persisted `OB-SEAL` receipt file after a
    /// hard kill (`S3(e)`).
    pub fn resume_from_receipt(receipt_path: &Path) -> Result<Self, SyncError> {
        let bytes = fs::read(receipt_path).map_err(|e| {
            SyncError::Cas(CasError::Io {
                operation: "read ob-seal receipt",
                detail: e.to_string(),
            })
        })?;
        let receipt = ObSealReceipt::from_bytes(&bytes)?;
        Ok(Self { receipt })
    }

    #[must_use]
    pub const fn receipt(&self) -> &ObSealReceipt {
        &self.receipt
    }

    /// Phase 2 (*Drain*): verifies a received chunk against its expected manifest digest, writes it
    /// into `store`'s content-addressed chunk pool (WITHOUT publishing any root), and updates the
    /// durable `OB-SEAL` receipt.
    pub fn drain_chunk(
        &mut self,
        store: &CasStore,
        expected_digest: Digest32,
        chunk_bytes: &[u8],
        receipt_path: &Path,
    ) -> Result<(), SyncError> {
        if self.receipt.phase == TransferPhase::Finalized {
            return Err(SyncError::InvalidField {
                field: "phase",
                detail: "cannot drain chunks into an already finalized transfer".to_string(),
            });
        }
        let expected_ref = self
            .receipt
            .manifest
            .chunks
            .iter()
            .find(|c| c.sha256 == expected_digest)
            .copied()
            .ok_or_else(|| SyncError::InvalidField {
                field: "chunk_digest",
                detail: format!("chunk {expected_digest} is not part of manifest"),
            })?;
        if chunk_bytes.len() != expected_ref.len as usize {
            return Err(SyncError::InvalidField {
                field: "chunk_bytes",
                detail: format!(
                    "length {} != manifest chunk len {}",
                    chunk_bytes.len(),
                    expected_ref.len
                ),
            });
        }
        let actual = sha256(chunk_bytes);
        if actual != expected_digest {
            return Err(SyncError::Cas(CasError::ChunkDigestMismatch {
                expected: expected_digest,
                actual,
            }));
        }
        store.put_verified_chunk(expected_digest, chunk_bytes)?;
        let mut next = self.receipt.clone();
        next.phase = TransferPhase::Draining;
        next.verified_chunks.insert(expected_digest);
        persist_receipt(&next, receipt_path)?;
        self.receipt = next;
        Ok(())
    }

    /// Phase 3 (*Finalize*): refuses if any chunk is still missing (`PO-BED-3`); otherwise publishes
    /// the root atomically in `CasStore`, marks the receipt `Finalized`, and emits a
    /// [`QuiescenceProof`].
    pub fn finalize(
        &mut self,
        store: &CasStore,
        receipt_path: &Path,
    ) -> Result<QuiescenceProof, SyncError> {
        let missing = self.receipt.remaining_chunks();
        if !missing.is_empty() {
            return Err(SyncError::IncompleteTransfer {
                missing_chunks: missing.len(),
            });
        }
        let published = store.publish_manifest_if_complete(&self.receipt.manifest)?;
        let mut next = self.receipt.clone();
        next.phase = TransferPhase::Finalized;
        persist_receipt(&next, receipt_path)?;
        self.receipt = next;

        let mut enc = Encoder::new_frame(*b"QPRF", 1);
        enc.put_str(&self.receipt.transfer_id);
        enc.put_str(&self.receipt.lease_id);
        enc.put_fixed(published.root_digest.as_bytes());
        enc.put_u32(published.chunks.len() as u32);
        let proof_digest = sha256(&enc.finish());

        Ok(QuiescenceProof {
            transfer_id: self.receipt.transfer_id.clone(),
            lease_id: self.receipt.lease_id.clone(),
            published_root: published.root_digest,
            total_chunks: published.chunks.len(),
            proof_digest,
        })
    }
}

fn persist_receipt(receipt: &ObSealReceipt, path: &Path) -> Result<(), SyncError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| {
            SyncError::Cas(CasError::Io {
                operation: "create receipt parent dir",
                detail: e.to_string(),
            })
        })?;
    }
    let bytes = receipt.to_bytes()?;
    let tmp = path.with_extension("obsr.tmp");
    {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&tmp)
            .map_err(|e| {
                SyncError::Cas(CasError::Io {
                    operation: "open ob-seal receipt tmp",
                    detail: e.to_string(),
                })
            })?;
        file.write_all(&bytes).map_err(|e| {
            SyncError::Cas(CasError::Io {
                operation: "write ob-seal receipt tmp",
                detail: e.to_string(),
            })
        })?;
        file.sync_all().map_err(|e| {
            SyncError::Cas(CasError::Io {
                operation: "sync ob-seal receipt tmp",
                detail: e.to_string(),
            })
        })?;
    }
    fs::rename(&tmp, path).map_err(|e| {
        SyncError::Cas(CasError::Io {
            operation: "rename ob-seal receipt",
            detail: e.to_string(),
        })
    })?;
    if let Some(parent) = path.parent()
        && let Ok(dir) = fs::File::open(parent)
    {
        let _ = dir.sync_all();
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use fmine_object::cas::chunk_bytes;

    /// BD-106 done-when (`Resume after hard kill`): starts a 5-chunk transfer, drains 2 chunks,
    /// simulates a hard kill (drops transfer handle), verifies root is still invisible in CasStore
    /// (`PO-BED-3`), resumes from the persisted `OB-SEAL` receipt, refuses premature finalize,
    /// drains remaining 3 chunks, finalizes with QuiescenceProof, and reads identical payload.
    #[test]
    fn test_ob_seal_chunk_receipts_resume_after_hard_kill_request_drain_finalize() {
        let dir = std::env::temp_dir().join(format!("fmine-sync-obsr-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let store = CasStore::open(dir.join("cas")).expect("cas");
        let receipt_path = dir.join("receipts/xfer-001.obsr");

        // Build a 5-chunk non-periodic payload.
        let payload: Vec<u8> = (0usize..5_000)
            .map(|i| ((i as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) >> 56) as u8)
            .collect();
        let sliced = chunk_bytes(&payload, 1_024).expect("chunk");
        let refs: Vec<ChunkRef> = sliced.iter().map(|(c, _)| *c).collect();
        let manifest =
            CasManifest::from_chunks("application/octet-stream", 1_024, refs).expect("manifest");
        let chunks: Vec<(ChunkRef, &[u8])> = sliced;
        assert_eq!(chunks.len(), 5);

        // Phase 1 & partial Phase 2 before simulated hard kill.
        {
            let mut xfer = ResumableTransfer::start_request(
                "xfer-001",
                "lease-lumwana-9",
                manifest.clone(),
                &receipt_path,
            )
            .expect("start");
            assert_eq!(xfer.receipt().phase, TransferPhase::Requested);
            for (cref, bytes) in chunks.iter().take(2) {
                xfer.drain_chunk(&store, cref.sha256, bytes, &receipt_path)
                    .expect("drain chunk");
            }
            assert_eq!(xfer.receipt().phase, TransferPhase::Draining);
            assert_eq!(xfer.receipt().remaining_chunks().len(), 3);
            // Simulated hard kill: xfer dropped here!
        }

        // PO-BED-3: partial transfer root MUST NOT be visible in CasStore.
        assert!(!store.has_manifest(&manifest.root_digest));

        // Resume from durable OB-SEAL receipt after restart.
        let mut resumed = ResumableTransfer::resume_from_receipt(&receipt_path).expect("resume");
        assert_eq!(resumed.receipt().verified_chunks.len(), 2);
        let remaining = resumed.receipt().remaining_chunks();
        assert_eq!(remaining.len(), 3);

        // Premature finalize MUST refuse while 3 chunks remain.
        let err = resumed
            .finalize(&store, &receipt_path)
            .expect_err("premature finalize must refuse");
        assert!(matches!(
            err,
            SyncError::IncompleteTransfer { missing_chunks: 3 }
        ));
        assert!(!store.has_manifest(&manifest.root_digest));

        // Drain only the remaining 3 chunks and finalize cleanly.
        for cref in remaining {
            let (_, bytes) = chunks
                .iter()
                .find(|(c, _)| c.sha256 == cref.sha256)
                .expect("chunk bytes");
            resumed
                .drain_chunk(&store, cref.sha256, bytes, &receipt_path)
                .expect("drain resumed chunk");
        }
        let proof = resumed.finalize(&store, &receipt_path).expect("finalize");
        assert_eq!(proof.published_root, manifest.root_digest);
        assert_eq!(proof.total_chunks, 5);
        assert_eq!(resumed.receipt().phase, TransferPhase::Finalized);
        assert!(store.has_manifest(&manifest.root_digest));
        assert_eq!(
            store.read_object(&manifest.root_digest).expect("read"),
            payload
        );

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_ob_seal_real_subprocess_hard_kill_and_corrupt_chunk_refusals() {
        const CHILD_DIR_ENV: &str = "FMINE_TEST_OB_SEAL_KILL_DIR";
        let payload: Vec<u8> = (0usize..4_000)
            .map(|i| ((i as u64).wrapping_mul(0x517C_C1B7_2722_0A95) >> 56) as u8)
            .collect();
        let sliced = chunk_bytes(&payload, 1_024).expect("chunk");
        let refs: Vec<ChunkRef> = sliced.iter().map(|(c, _)| *c).collect();
        let manifest =
            CasManifest::from_chunks("application/octet-stream", 1_024, refs).expect("manifest");
        assert_eq!(sliced.len(), 4);

        if let Ok(child_dir) = std::env::var(CHILD_DIR_ENV) {
            let dir = std::path::PathBuf::from(child_dir);
            let store = CasStore::open(dir.join("cas")).expect("child cas");
            let receipt_path = dir.join("receipts/xfer-kill.obsr");
            let mut xfer = ResumableTransfer::start_request(
                "xfer-kill",
                "lease-kibali-4",
                manifest,
                &receipt_path,
            )
            .expect("child start");
            // Corrupt chunk (bit flip) refuses without poisoning receipt or CAS store.
            let mut bad_bytes = sliced[0].1.to_vec();
            bad_bytes[0] ^= 0xFF;
            assert!(matches!(
                xfer.drain_chunk(&store, sliced[0].0.sha256, &bad_bytes, &receipt_path),
                Err(SyncError::Cas(CasError::ChunkDigestMismatch { .. }))
            ));
            assert!(xfer.receipt().verified_chunks.is_empty());

            // Drain 2 of 4 chunks durably, then terminate immediately (no Drop).
            for (cref, bytes) in sliced.iter().take(2) {
                xfer.drain_chunk(&store, cref.sha256, bytes, &receipt_path)
                    .expect("child drain");
            }
            std::process::exit(0);
        }

        let dir = std::env::temp_dir().join(format!("fmine-sync-obsr-kill-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("mkdir");

        let status = std::process::Command::new(std::env::current_exe().expect("exe"))
            .arg("--exact")
            .arg(
                "receipt::tests::test_ob_seal_real_subprocess_hard_kill_and_corrupt_chunk_refusals",
            )
            .env(CHILD_DIR_ENV, &dir)
            .status()
            .expect("spawn child");
        assert!(status.success(), "child process failed: {status}");

        let store = CasStore::open(dir.join("cas")).expect("parent cas");
        let receipt_path = dir.join("receipts/xfer-kill.obsr");
        // PO-BED-3: after child abrupt exit, root MUST NOT be published.
        assert!(!store.has_manifest(&manifest.root_digest));

        let mut resumed = ResumableTransfer::resume_from_receipt(&receipt_path).expect("resume");
        assert_eq!(resumed.receipt().verified_chunks.len(), 2);
        let remaining = resumed.receipt().remaining_chunks();
        assert_eq!(remaining.len(), 2);

        for cref in remaining {
            let (_, bytes) = sliced
                .iter()
                .find(|(c, _)| c.sha256 == cref.sha256)
                .expect("chunk");
            resumed
                .drain_chunk(&store, cref.sha256, bytes, &receipt_path)
                .expect("parent drain");
        }
        let proof = resumed.finalize(&store, &receipt_path).expect("finalize");
        assert_eq!(proof.published_root, manifest.root_digest);
        assert_eq!(
            store.read_object(&manifest.root_digest).expect("read"),
            payload
        );

        // Draining after Finalized must refuse.
        assert!(matches!(
            resumed.drain_chunk(&store, sliced[0].0.sha256, sliced[0].1, &receipt_path),
            Err(SyncError::InvalidField { field: "phase", .. })
        ));

        // Corrupt receipt decoding checks: finalized with missing chunks, foreign chunk, duplicate chunk.
        let mut incomplete_finalized = ObSealReceipt {
            transfer_id: "xfer-bad".into(),
            lease_id: "lease-1".into(),
            phase: TransferPhase::Finalized,
            manifest: manifest.clone(),
            verified_chunks: BTreeSet::new(),
        };
        assert!(matches!(
            ObSealReceipt::from_bytes(&incomplete_finalized.to_bytes().expect("enc")),
            Err(SyncError::InvalidField { field: "phase", .. })
        ));
        incomplete_finalized.phase = TransferPhase::Draining;
        incomplete_finalized
            .verified_chunks
            .insert(sha256(b"not-in-manifest"));
        assert!(matches!(
            ObSealReceipt::from_bytes(&incomplete_finalized.to_bytes().expect("enc")),
            Err(SyncError::InvalidField {
                field: "verified_chunks",
                ..
            })
        ));

        let _ = fs::remove_dir_all(&dir);
    }
}
