//! `FMT-ANCHOR-1` LedgerAnchor with FSS fields + `cell_id` + `residency_epoch` (§26.5, BD-021).
//! Determinism: BitwisePortable

use crate::codec::{CodecError, Decoder, Encoder};
use crate::hash::{Digest32, blake3_hash};
use crate::id::{ObjectId, PrefixKind};

/// Current schema version for `FMT-ANCHOR-1`.
pub const FMT_ANCHOR_V1: u16 = 1;

const TAG_ANCHOR: [u8; 4] = *b"ANCR";

/// Provenance anchor pointing to a specific cell, page, or byte range inside a content-addressed artifact (`§8.1`).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SourceAnchor {
    /// Relative path or URI of the source artifact (e.g. `"corpus_of_input_docs/downloaded/mine_stats/Barrick_Q4_2025_AISC.xlsx"`).
    pub artifact_path: String,
    /// SHA-256 digest of the source artifact bytes.
    pub artifact_sha256: [u8; 32],
    /// Sheet/cell or section/page locator (e.g. `"Gold AISC!L14"`).
    pub locator: String,
}

impl SourceAnchor {
    /// Construct a new [`SourceAnchor`].
    #[must_use]
    pub fn new(
        artifact_path: impl Into<String>,
        artifact_sha256: [u8; 32],
        locator: impl Into<String>,
    ) -> Self {
        Self {
            artifact_path: artifact_path.into(),
            artifact_sha256,
            locator: locator.into(),
        }
    }
}

/// Authority high-water mark (`FMT-ANCHOR-1`, §26.5, BD-021).
/// Carries all FrankenSurveillance (`FSS`) `LedgerAnchor` fields with `site_lineage` generalized
/// to `cell_id`, plus `residency_epoch` (`W3`, §31.2).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct LedgerAnchor {
    /// Format version (`1` for `FMT-ANCHOR-1`; `0` supported for N-1 compatibility).
    pub format_version: u16,
    /// Authority cell identifier (`CELL-PUBLIC`, `CELL-SITE(lumwana)`, `CELL-DEC`, etc.).
    pub cell_id: String,
    /// Monotonic lease/fencing epoch for single-writer authority (`PO-BED-1`).
    pub ledger_epoch: u64,
    /// Monotonic batch commit sequence within the cell.
    pub commit_sequence: u64,
    /// Epoch of the adapter/importer registry (`registries/`).
    pub adapter_registry_epoch: u32,
    /// Epoch of the `S0` schema registry.
    pub schema_epoch: u32,
    /// Epoch of the active policy & metric-definition registry.
    pub policy_epoch: u32,
    /// Epoch of the MNPI/JV privacy & wall caveat rules.
    pub privacy_epoch: u32,
    /// Epoch of the host-state data-residency statute matrix (`W3`, `residency_epoch`).
    pub residency_epoch: u32,
    /// 32-byte Merkle Mountain Range (`MMR`) or state root at `commit_sequence`.
    pub state_root: Digest32,
}

impl LedgerAnchor {
    /// Encode this anchor as a canonical `FMT-ANCHOR-1` frame (`FMNE` + `ANCR` + CRC32C).
    #[must_use]
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut enc = Encoder::new_frame(TAG_ANCHOR, self.format_version);
        enc.put_str(&self.cell_id);
        enc.put_u64(self.ledger_epoch);
        enc.put_u64(self.commit_sequence);
        enc.put_u32(self.adapter_registry_epoch);
        enc.put_u32(self.schema_epoch);
        enc.put_u32(self.policy_epoch);
        enc.put_u32(self.privacy_epoch);
        if self.format_version >= 1 {
            enc.put_u32(self.residency_epoch);
        }
        enc.put_fixed(self.state_root.as_bytes());
        enc.finish()
    }

    /// Decode an `FMT-ANCHOR-1` frame (supporting both v1 and N-1 v0 frames without `residency_epoch`).
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, CodecError> {
        let mut dec = Decoder::open_frame(bytes, TAG_ANCHOR, 0, FMT_ANCHOR_V1)?;
        let ver = dec.version();
        let cell_id = dec.get_str(256)?.to_string();
        let ledger_epoch = dec.get_u64()?;
        let commit_sequence = dec.get_u64()?;
        let adapter_registry_epoch = dec.get_u32()?;
        let schema_epoch = dec.get_u32()?;
        let policy_epoch = dec.get_u32()?;
        let privacy_epoch = dec.get_u32()?;
        let residency_epoch = if ver >= 1 { dec.get_u32()? } else { 0 };
        let state_root = Digest32(dec.get_fixed::<32>()?);
        dec.finish()?;
        Ok(Self {
            format_version: ver,
            cell_id,
            ledger_epoch,
            commit_sequence,
            adapter_registry_epoch,
            schema_epoch,
            policy_epoch,
            privacy_epoch,
            residency_epoch,
            state_root,
        })
    }

    /// Compute the canonical `anc_...` [`ObjectId`] of this anchor.
    #[must_use]
    pub fn anchor_id(&self) -> ObjectId {
        ObjectId::from_canonical_bytes(PrefixKind::Anchor, &self.to_bytes())
    }

    /// Compute the 32-byte BLAKE3 digest of this anchor's canonical frame.
    #[must_use]
    pub fn digest(&self) -> Digest32 {
        blake3_hash(&self.to_bytes())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_anchor_v1_and_n_minus_1_roundtrip() {
        let v1 = LedgerAnchor {
            format_version: FMT_ANCHOR_V1,
            cell_id: "CELL-PUBLIC(barrick)".to_string(),
            ledger_epoch: 1,
            commit_sequence: 42,
            adapter_registry_epoch: 1,
            schema_epoch: 1,
            policy_epoch: 2,
            privacy_epoch: 1,
            residency_epoch: 3,
            state_root: blake3_hash(b"root-42"),
        };
        let bytes = v1.to_bytes();
        let decoded = LedgerAnchor::from_bytes(&bytes).unwrap();
        assert_eq!(v1, decoded);
        assert!(v1.anchor_id().to_string().starts_with("anc_"));

        // N-1 reader test (v0 without residency_epoch defaults residency_epoch to 0):
        let mut v0 = v1.clone();
        v0.format_version = 0;
        v0.residency_epoch = 0;
        let bytes_v0 = v0.to_bytes();
        let decoded_v0 = LedgerAnchor::from_bytes(&bytes_v0).unwrap();
        assert_eq!(decoded_v0.residency_epoch, 0);
        assert_eq!(decoded_v0.commit_sequence, 42);
    }
}
