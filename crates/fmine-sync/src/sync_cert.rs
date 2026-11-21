//! Determinism: BitwisePortable
//!
//! `SyncCert` MMR prefix-consistency verification (`BD-103`, plan §26.9 A2, `PO-BED-2`).
//!
//! Every replica apply checks an RFC 6962 consistency proof showing that the replica's current
//! MMR root at `from_seq` is an exact append-only prefix of the writer's MMR root at `to_seq`.
//! If verification fails, the apply refuses and marks the cell stale without mutating state.

use fmine_core::codec::{Decoder, Encoder};
use fmine_core::hash::Digest32;
use fmine_ledger::mmr::{Mmr, verify_consistency};

use crate::SyncError;

/// Canonical wire frame tag for `SyncCert` (`FMNE` + `SCER` + CRC32C).
pub const TAG_SYNC_CERT: [u8; 4] = *b"SCER";
const FMT_SYNC_CERT_V1: u16 = 1;
const MAX_PROOF_HASHES: usize = 128;

/// Prefix-consistency sync certificate (`plan §26.9 A2`, `PO-BED-2`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncCert {
    pub cell_id: String,
    pub epoch: u64,
    pub from_seq: u64,
    pub to_seq: u64,
    pub mmr_root_before: Digest32,
    pub mmr_root_after: Digest32,
    pub consistency_proof: Vec<Digest32>,
}

impl SyncCert {
    /// Builds a `SyncCert` directly from the writer's authoritative [`Mmr`] between sizes
    /// `from_seq` (`>= 1`) and `to_seq` (`>= from_seq`).
    pub fn build_from_mmr(
        cell_id: impl Into<String>,
        epoch: u64,
        mmr: &Mmr,
        from_seq: u64,
        to_seq: u64,
    ) -> Result<Self, SyncError> {
        let cell_id = cell_id.into();
        if cell_id.is_empty() || cell_id.len() > 256 {
            return Err(SyncError::InvalidField {
                field: "cell_id",
                detail: "must be 1..=256 bytes".to_string(),
            });
        }
        if epoch == 0 {
            return Err(SyncError::InvalidField {
                field: "epoch",
                detail: "must be >= 1".to_string(),
            });
        }
        let mmr_root_before = mmr.root_at(from_seq)?;
        let mmr_root_after = mmr.root_at(to_seq)?;
        let consistency_proof = mmr.consistency_proof(from_seq, to_seq)?;
        Ok(Self {
            cell_id,
            epoch,
            from_seq,
            to_seq,
            mmr_root_before,
            mmr_root_after,
            consistency_proof,
        })
    }

    /// Encodes `SyncCert` into canonical `FMNE` + `SCER` + CRC32C wire bytes.
    #[must_use]
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut enc = Encoder::new_frame(TAG_SYNC_CERT, FMT_SYNC_CERT_V1);
        enc.put_str(&self.cell_id);
        enc.put_u64(self.epoch);
        enc.put_u64(self.from_seq);
        enc.put_u64(self.to_seq);
        enc.put_fixed(self.mmr_root_before.as_bytes());
        enc.put_fixed(self.mmr_root_after.as_bytes());
        enc.put_u32(self.consistency_proof.len() as u32);
        for h in &self.consistency_proof {
            enc.put_fixed(h.as_bytes());
        }
        enc.finish()
    }

    /// Decodes canonical `FMNE` + `SCER` bytes.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, SyncError> {
        let mut dec =
            Decoder::open_frame(bytes, TAG_SYNC_CERT, FMT_SYNC_CERT_V1, FMT_SYNC_CERT_V1)?;
        let cell_id = dec.get_str(256)?.to_string();
        let epoch = dec.get_u64()?;
        let from_seq = dec.get_u64()?;
        let to_seq = dec.get_u64()?;
        let mmr_root_before = Digest32(dec.get_fixed::<32>()?);
        let mmr_root_after = Digest32(dec.get_fixed::<32>()?);
        let count = dec.get_u32()? as usize;
        if count > MAX_PROOF_HASHES {
            return Err(SyncError::SyncCertInvalid {
                detail: format!("proof length {count} exceeds {MAX_PROOF_HASHES}"),
            });
        }
        let mut consistency_proof = Vec::with_capacity(count);
        for _ in 0..count {
            consistency_proof.push(Digest32(dec.get_fixed::<32>()?));
        }
        dec.finish()?;
        Ok(Self {
            cell_id,
            epoch,
            from_seq,
            to_seq,
            mmr_root_before,
            mmr_root_after,
            consistency_proof,
        })
    }

    /// Verifies RFC 6962 consistency proof that `mmr_root_before` at `from_seq` is an exact prefix
    /// of `mmr_root_after` at `to_seq` (`PO-BED-2`).
    pub fn verify_prefix_consistency(&self) -> Result<(), SyncError> {
        if self.epoch == 0 {
            return Err(SyncError::SyncCertInvalid {
                detail: "epoch must be >= 1".to_string(),
            });
        }
        verify_consistency(
            self.from_seq,
            self.to_seq,
            &self.mmr_root_before,
            &self.mmr_root_after,
            &self.consistency_proof,
        )?;
        Ok(())
    }
}

/// Enterprise replica log watermark for one cell (`PO-BED-2`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplicaLogState {
    pub cell_id: String,
    pub highest_epoch: u64,
    pub committed_seq: u64,
    pub mmr_root: Digest32,
    pub stale: bool,
}

/// Result of verifying and applying a [`SyncCert`] onto a [`ReplicaLogState`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyncApplyOutcome {
    Advanced { new_seq: u64, new_root: Digest32 },
    AlreadyApplied,
}

impl ReplicaLogState {
    /// Verifies `cert` against this replica's current prefix and advances the watermark atomically.
    /// Any prefix mismatch, epoch regression, or invalid proof marks `self.stale = true` and refuses
    /// without mutating `committed_seq` or `mmr_root` (`PO-BED-2`).
    pub fn verify_and_apply(&mut self, cert: &SyncCert) -> Result<SyncApplyOutcome, SyncError> {
        if cert.cell_id != self.cell_id {
            return Err(SyncError::SyncCertInvalid {
                detail: format!(
                    "cert cell '{}' != replica cell '{}'",
                    cert.cell_id, self.cell_id
                ),
            });
        }
        if cert.epoch < self.highest_epoch {
            self.stale = true;
            return Err(SyncError::StaleOrDiverged {
                cell_id: self.cell_id.clone(),
                detail: format!(
                    "epoch regression: cert epoch {} < replica epoch {}",
                    cert.epoch, self.highest_epoch
                ),
            });
        }
        if cert.from_seq != self.committed_seq {
            self.stale = true;
            return Err(SyncError::SyncCertInvalid {
                detail: format!(
                    "cert from_seq {} != replica committed_seq {}",
                    cert.from_seq, self.committed_seq
                ),
            });
        }
        if cert.mmr_root_before != self.mmr_root {
            self.stale = true;
            return Err(SyncError::SyncCertInvalid {
                detail: format!(
                    "prefix root mismatch: cert before {} != replica root {}",
                    cert.mmr_root_before, self.mmr_root
                ),
            });
        }
        if let Err(err) = cert.verify_prefix_consistency() {
            self.stale = true;
            return Err(err);
        }
        if cert.to_seq == self.committed_seq {
            return Ok(SyncApplyOutcome::AlreadyApplied);
        }
        self.highest_epoch = cert.epoch;
        self.committed_seq = cert.to_seq;
        self.mmr_root = cert.mmr_root_after;
        self.stale = false;
        Ok(SyncApplyOutcome::Advanced {
            new_seq: self.committed_seq,
            new_root: self.mmr_root,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fmine_core::hash::sha256;

    /// BD-103 / PO-BED-2: verifies valid multi-step RFC 6962 SyncCert advances, codec roundtrip,
    /// and refusal + stale marking on forged history, tampered proof hash, or epoch regression.
    #[test]
    fn test_sync_cert_po_bed_2_verifies_prefix_and_refuses_forged_history() {
        let mut writer_mmr = Mmr::new();
        for i in 0u64..16 {
            let entry = sha256(&i.to_le_bytes());
            writer_mmr.push(entry.as_bytes()).expect("push");
        }
        let root_5 = writer_mmr.root_at(5).expect("root 5");
        let mut replica = ReplicaLogState {
            cell_id: "CELL-SITE(lumwana)".into(),
            highest_epoch: 1,
            committed_seq: 5,
            mmr_root: root_5,
            stale: false,
        };

        // Valid SyncCert advancing from 5 -> 16 (verified via RFC 6962 consistency proof).
        let cert = SyncCert::build_from_mmr("CELL-SITE(lumwana)", 1, &writer_mmr, 5, 16)
            .expect("build cert");
        let wire = cert.to_bytes();
        let decoded = SyncCert::from_bytes(&wire).expect("decode cert");
        assert_eq!(decoded, cert);

        // Tampered proof hash refuses and marks replica stale without advancing state!
        let mut forged = decoded.clone();
        forged.consistency_proof[0].0[0] ^= 0x80;
        let mut tampered_replica = replica.clone();
        assert!(tampered_replica.verify_and_apply(&forged).is_err());
        assert!(tampered_replica.stale);
        assert_eq!(tampered_replica.committed_seq, 5);
        assert_eq!(tampered_replica.mmr_root, root_5);

        // Valid apply advances cleanly and proves logarithmic proof size (<= ceil(log2(16)) + 1 = 5).
        assert!(decoded.consistency_proof.len() <= 5);
        let outcome = replica.verify_and_apply(&decoded).expect("apply valid");
        assert_eq!(
            outcome,
            SyncApplyOutcome::Advanced {
                new_seq: 16,
                new_root: writer_mmr.root(),
            }
        );
        assert!(!replica.stale);

        // Epoch regression refuses with FMINE-REF-502 and marks stale.
        let mut regressed = SyncCert::build_from_mmr("CELL-SITE(lumwana)", 2, &writer_mmr, 16, 16)
            .expect("build eq");
        replica.highest_epoch = 3;
        regressed.epoch = 2;
        let err = replica
            .verify_and_apply(&regressed)
            .expect_err("epoch regression");
        assert_eq!(err.to_refusal().code, "FMINE-REF-502");
        assert!(replica.stale);
    }
}
