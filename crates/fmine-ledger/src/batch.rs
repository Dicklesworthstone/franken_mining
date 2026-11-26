//! Determinism: BitwisePortable
//! `FMT-LEDGER-1` delta batch codec (`BD-081`, plan §26.5).
//!
//! Each batch carries `{cell_id, epoch, seq, prev_hash, anchor, deltas[], witnesses[], cross_cell_refs[]}`
//! framed with `FMNE` + `LDGR` + version + CRC32C via [`fmine_core::codec`]. Sequences are encoded in
//! strict canonical ascending order (`put_canonical_seq` / `get_canonical_seq`). The batch digest is
//! `SHA-256(canonical_bytes)` so the cell hash chain and RFC 6962 [`crate::mmr::Mmr`] align with ATP
//! Merkle hashing. Schema `v2` adds opt-in keyed-state witnesses; `v0` (without `cross_cell_refs`)
//! and `v1` retain their existing encodings and remain readable.

use core::fmt;

use fmine_core::anchor::LedgerAnchor;
use fmine_core::codec::{CodecError, Decoder, Encoder};
use fmine_core::hash::{Digest32, sha256};
use fmine_evidence::{Refusal, RefusalCode};

/// Format tag for `FMT-LEDGER-1` delta batches (`FMNE` + `LDGR`).
pub const TAG_LEDGER_BATCH: [u8; 4] = *b"LDGR";

/// Original schema version with cross-cell references for `FMT-LEDGER-1`.
pub const FMT_LEDGER_V1: u16 = 1;

/// Schema version admitting [`WitnessKind::KeyState`] (`BD-085`).
pub const FMT_LEDGER_V2: u16 = 2;

/// Minimum supported schema version (retained durable compatibility, plan §26.5).
pub const FMT_LEDGER_MIN_VERSION: u16 = 0;

/// Previous-hash sentinel for a cell's genesis batch (`seq == 0`).
pub const GENESIS_PREV_HASH: Digest32 = Digest32([0u8; 32]);

/// Maximum length of a `cell_id` or non-key witness `subject_id` in bytes.
pub const MAX_ID_BYTES: usize = 256;

/// Maximum length of a delta key in bytes.
pub const MAX_KEY_BYTES: usize = 512;

/// Maximum payload length for a single [`DeltaOp`] (4 MiB).
pub const MAX_DELTA_PAYLOAD_BYTES: usize = 4 * 1024 * 1024;

/// Maximum encoded length of an embedded `FMT-ANCHOR-1` frame.
pub const MAX_ANCHOR_FRAME_BYTES: usize = 4096;

/// Maximum number of deltas per batch.
pub const MAX_DELTAS_PER_BATCH: usize = 65_536;

/// Maximum number of witnesses per batch.
pub const MAX_WITNESSES_PER_BATCH: usize = 4096;

/// Maximum number of cross-cell references per batch.
pub const MAX_CROSS_CELL_REFS_PER_BATCH: usize = 4096;

/// Append-only delta operation kinds (`INV-003`: corrections supersede; nothing deletes history).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum DeltaKind {
    /// Introduce a new immutable authority object or record.
    Put = 1,
    /// Supersede an earlier authority object while retaining its history (`INV-003`).
    Supersede = 2,
    /// Reserve a durable obligation (`OB-*`, plan §26.7).
    ReserveObligation = 3,
    /// Resolve or abort a previously reserved durable obligation.
    ResolveObligation = 4,
    /// Record an authoritative lease transfer to `epoch + 1` (`PO-BED-1`).
    TransferLease = 5,
}

impl DeltaKind {
    fn from_u8(v: u8) -> Result<Self, LedgerError> {
        match v {
            1 => Ok(Self::Put),
            2 => Ok(Self::Supersede),
            3 => Ok(Self::ReserveObligation),
            4 => Ok(Self::ResolveObligation),
            5 => Ok(Self::TransferLease),
            other => Err(LedgerError::InvalidEnumTag {
                kind: "DeltaKind",
                tag: other,
            }),
        }
    }
}

/// Typed witness attached to a [`DeltaBatch`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum WitnessKind {
    /// Mass/metal/money conservation check witness (`INV-005`, `PO-BED-6`).
    Conservation = 1,
    /// Cross-cell gluing witness (`§26.3`, `F12`).
    Gluing = 2,
    /// Cryptographic signature or attestation digest.
    Signature = 3,
    /// Human authority-token verification witness (`F10`).
    AuthorityToken = 4,
    /// Telemetry continuity receipt witness (`§26.8`).
    Continuity = 5,
    /// Exact pre-batch version or absence of a cell-scoped key (`BD-085`, schema `v2`).
    KeyState = 6,
}

impl WitnessKind {
    fn from_u8(v: u8) -> Result<Self, LedgerError> {
        match v {
            1 => Ok(Self::Conservation),
            2 => Ok(Self::Gluing),
            3 => Ok(Self::Signature),
            4 => Ok(Self::AuthorityToken),
            5 => Ok(Self::Continuity),
            6 => Ok(Self::KeyState),
            other => Err(LedgerError::InvalidEnumTag {
                kind: "WitnessKind",
                tag: other,
            }),
        }
    }
}

/// One canonical, append-only mutation inside a [`DeltaBatch`].
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DeltaOp {
    /// Canonical cell-scoped key (non-empty, ≤ [`MAX_KEY_BYTES`]).
    pub key: String,
    /// Append-only operation kind.
    pub kind: DeltaKind,
    /// SHA-256 digest of `payload` bytes, checked on validate and decode.
    pub payload_digest: Digest32,
    /// Canonical opaque payload bytes (≤ [`MAX_DELTA_PAYLOAD_BYTES`]).
    pub payload: Vec<u8>,
}

impl DeltaOp {
    /// Construct a [`DeltaOp`] and compute its exact SHA-256 `payload_digest`.
    #[must_use]
    pub fn new(key: impl Into<String>, kind: DeltaKind, payload: Vec<u8>) -> Self {
        let payload_digest = sha256(&payload);
        Self {
            key: key.into(),
            kind,
            payload_digest,
            payload,
        }
    }

    fn validate(&self) -> Result<(), LedgerError> {
        if self.key.is_empty() || self.key.len() > MAX_KEY_BYTES {
            return Err(LedgerError::InvalidField {
                field: "delta.key",
                detail: format!("length {} outside 1..={MAX_KEY_BYTES}", self.key.len()),
            });
        }
        if self.payload.len() > MAX_DELTA_PAYLOAD_BYTES {
            return Err(LedgerError::InvalidField {
                field: "delta.payload",
                detail: format!(
                    "length {} exceeds {MAX_DELTA_PAYLOAD_BYTES}",
                    self.payload.len()
                ),
            });
        }
        let actual = sha256(&self.payload);
        if actual != self.payload_digest {
            return Err(LedgerError::PayloadDigestMismatch {
                key: self.key.clone(),
                expected: self.payload_digest,
                actual,
            });
        }
        Ok(())
    }
}

/// One canonical witness entry inside a [`DeltaBatch`].
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Witness {
    /// Witness classification.
    pub kind: WitnessKind,
    /// Subject identifier (non-empty, ≤ [`MAX_ID_BYTES`], or [`MAX_KEY_BYTES`] for `KeyState`).
    pub subject_id: String,
    /// 32-byte witness digest.
    pub digest: Digest32,
}

impl Witness {
    /// Construct a [`Witness`].
    #[must_use]
    pub fn new(kind: WitnessKind, subject_id: impl Into<String>, digest: Digest32) -> Self {
        Self {
            kind,
            subject_id: subject_id.into(),
            digest,
        }
    }

    fn validate(&self) -> Result<(), LedgerError> {
        let max = if self.kind == WitnessKind::KeyState {
            MAX_KEY_BYTES
        } else {
            MAX_ID_BYTES
        };
        if self.subject_id.is_empty() || self.subject_id.len() > max {
            return Err(LedgerError::InvalidField {
                field: "witness.subject_id",
                detail: format!("length {} outside 1..={max}", self.subject_id.len()),
            });
        }
        Ok(())
    }
}

/// Cross-cell reference `(cell, seq, digest)` (`§26.3`, `F12`).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CrossCellRef {
    /// Referenced cell identifier (non-empty, ≤ [`MAX_ID_BYTES`]).
    pub cell_id: String,
    /// Referenced batch sequence number inside `cell_id`.
    pub seq: u64,
    /// Expected SHA-256 batch digest at `(cell_id, seq)`.
    pub digest: Digest32,
}

impl CrossCellRef {
    /// Construct a [`CrossCellRef`].
    #[must_use]
    pub fn new(cell_id: impl Into<String>, seq: u64, digest: Digest32) -> Self {
        Self {
            cell_id: cell_id.into(),
            seq,
            digest,
        }
    }

    fn validate(&self) -> Result<(), LedgerError> {
        if self.cell_id.is_empty() || self.cell_id.len() > MAX_ID_BYTES {
            return Err(LedgerError::InvalidField {
                field: "cross_cell_ref.cell_id",
                detail: format!("length {} outside 1..={MAX_ID_BYTES}", self.cell_id.len()),
            });
        }
        Ok(())
    }
}

/// Canonical `FMT-LEDGER-1` delta batch (`BD-081`, plan §26.5).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct DeltaBatch {
    /// Format version (`2` admits keyed-state witnesses; `0` and `1` retain legacy encodings).
    pub format_version: u16,
    /// Authority cell identifier (`CELL-PUBLIC`, `CELL-SITE(...)`, `CELL-DEC`, etc.).
    pub cell_id: String,
    /// Monotonic lease/fencing epoch (`>= 1`, `PO-BED-1`).
    pub epoch: u64,
    /// Monotonic batch sequence within `cell_id` (0-based).
    pub seq: u64,
    /// SHA-256 digest of batch `seq - 1`, or [`GENESIS_PREV_HASH`] when `seq == 0`.
    pub prev_hash: Digest32,
    /// Embedded `FMT-ANCHOR-1` high-water mark matching `(cell_id, epoch, seq)`.
    pub anchor: LedgerAnchor,
    /// Strictly ascending canonical sequence of append-only [`DeltaOp`] records.
    pub deltas: Vec<DeltaOp>,
    /// Strictly ascending canonical sequence of [`Witness`] records.
    pub witnesses: Vec<Witness>,
    /// Strictly ascending canonical sequence of [`CrossCellRef`] triples (`v1`/`v2`; empty in `v0`).
    pub cross_cell_refs: Vec<CrossCellRef>,
}

/// Typed errors across `fmine-ledger` codec, segment, lease, and verification operations.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LedgerError {
    /// Underlying canonical `FMNE` frame codec error.
    Codec(CodecError),
    /// Unknown discriminant byte for a ledger enum.
    InvalidEnumTag { kind: &'static str, tag: u8 },
    /// Field violates structural bounds or invariants.
    InvalidField { field: &'static str, detail: String },
    /// A `DeltaOp`'s `payload_digest` does not equal `SHA-256(payload)`.
    PayloadDigestMismatch {
        key: String,
        expected: Digest32,
        actual: Digest32,
    },
    /// Embedded `LedgerAnchor` disagrees with the batch `(cell_id, epoch, seq)`.
    AnchorMismatch { detail: String },
    /// Batch sequence, prev_hash, or cell does not chain onto the preceding committed batch.
    ChainContinuity { detail: String },
    /// Fencing epoch regressed below the highest epoch observed for the cell (`PO-BED-1`, `F6`).
    EpochRegression {
        cell_id: String,
        observed: u64,
        highest: u64,
    },
    /// Active lease holder mismatch or unleased write attempt (`PO-BED-1`).
    LeaseViolation { detail: String },
    /// Segment MMR footer disagrees with replayed batch digests (`BD-082`).
    FooterMismatch { detail: String },
    /// Non-repairable interior corruption in a segment (`F7` / `INV-003`).
    InteriorCorruption { offset: u64, detail: String },
    /// Repair apply digest does not match the current on-disk repair plan (`PO-ADIT-3`).
    RepairDigestMismatch {
        expected: Digest32,
        actual: Digest32,
    },
    /// Filesystem I/O or lock refusal.
    Io {
        operation: &'static str,
        detail: String,
    },
}

impl LedgerError {
    /// Map this error to a registered `FMINE-REF-*` refusal without losing diagnostic context.
    #[must_use]
    pub fn to_refusal(&self) -> Refusal {
        let code = match self {
            Self::EpochRegression { .. } | Self::LeaseViolation { .. } => {
                RefusalCode::StaleAnchor502
            }
            Self::Codec(_)
            | Self::InvalidEnumTag { .. }
            | Self::InvalidField { .. }
            | Self::PayloadDigestMismatch { .. }
            | Self::AnchorMismatch { .. }
            | Self::ChainContinuity { .. }
            | Self::FooterMismatch { .. }
            | Self::InteriorCorruption { .. }
            | Self::RepairDigestMismatch { .. } => RefusalCode::DigestMismatch505,
            Self::Io { .. } => RefusalCode::InputMissingOrUnreadable506,
        };
        Refusal::new(code, self.to_string())
    }
}

impl From<CodecError> for LedgerError {
    fn from(value: CodecError) -> Self {
        Self::Codec(value)
    }
}

impl fmt::Display for LedgerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Codec(err) => write!(f, "ledger codec error: {err}"),
            Self::InvalidEnumTag { kind, tag } => write!(f, "invalid {kind} tag: {tag}"),
            Self::InvalidField { field, detail } => write!(f, "invalid {field}: {detail}"),
            Self::PayloadDigestMismatch {
                key,
                expected,
                actual,
            } => write!(
                f,
                "delta '{key}' payload digest mismatch: stored {expected} != computed {actual}"
            ),
            Self::AnchorMismatch { detail } => write!(f, "batch anchor mismatch: {detail}"),
            Self::ChainContinuity { detail } => write!(f, "ledger hash-chain break: {detail}"),
            Self::EpochRegression {
                cell_id,
                observed,
                highest,
            } => write!(
                f,
                "fencing epoch regression in cell '{cell_id}': observed epoch {observed} < highest seen {highest}"
            ),
            Self::LeaseViolation { detail } => write!(f, "cell lease violation: {detail}"),
            Self::FooterMismatch { detail } => write!(f, "segment footer mismatch: {detail}"),
            Self::InteriorCorruption { offset, detail } => {
                write!(
                    f,
                    "non-repairable interior segment corruption at byte {offset}: {detail}"
                )
            }
            Self::RepairDigestMismatch { expected, actual } => write!(
                f,
                "repair plan digest mismatch: requested {expected}, current on-disk plan is {actual}"
            ),
            Self::Io { operation, detail } => write!(f, "ledger I/O error ({operation}): {detail}"),
        }
    }
}

impl std::error::Error for LedgerError {}

impl DeltaBatch {
    /// Validate all structural, digest, anchor-binding, and canonical-order invariants.
    pub fn validate(&self) -> Result<(), LedgerError> {
        if self.format_version > FMT_LEDGER_V2 {
            return Err(LedgerError::Codec(CodecError::UnsupportedVersion {
                actual: self.format_version,
                min_supported: FMT_LEDGER_MIN_VERSION,
                max_supported: FMT_LEDGER_V2,
            }));
        }
        if self.cell_id.is_empty() || self.cell_id.len() > MAX_ID_BYTES {
            return Err(LedgerError::InvalidField {
                field: "cell_id",
                detail: format!("length {} outside 1..={MAX_ID_BYTES}", self.cell_id.len()),
            });
        }
        if self.epoch == 0 {
            return Err(LedgerError::InvalidField {
                field: "epoch",
                detail: "fencing epoch must be >= 1".to_string(),
            });
        }
        if self.seq == 0 && self.prev_hash != GENESIS_PREV_HASH {
            return Err(LedgerError::ChainContinuity {
                detail: "genesis batch (seq=0) must carry zero prev_hash".to_string(),
            });
        }
        if self.seq > 0 && self.prev_hash == GENESIS_PREV_HASH {
            return Err(LedgerError::ChainContinuity {
                detail: format!(
                    "non-genesis batch (seq={}) cannot carry zero prev_hash",
                    self.seq
                ),
            });
        }
        if self.anchor.cell_id != self.cell_id {
            return Err(LedgerError::AnchorMismatch {
                detail: format!(
                    "anchor cell_id '{}' != batch cell_id '{}'",
                    self.anchor.cell_id, self.cell_id
                ),
            });
        }
        if self.anchor.ledger_epoch != self.epoch {
            return Err(LedgerError::AnchorMismatch {
                detail: format!(
                    "anchor ledger_epoch {} != batch epoch {}",
                    self.anchor.ledger_epoch, self.epoch
                ),
            });
        }
        if self.anchor.commit_sequence != self.seq {
            return Err(LedgerError::AnchorMismatch {
                detail: format!(
                    "anchor commit_sequence {} != batch seq {}",
                    self.anchor.commit_sequence, self.seq
                ),
            });
        }
        if self.deltas.len() > MAX_DELTAS_PER_BATCH {
            return Err(LedgerError::Codec(CodecError::LengthOutOfBounds {
                len: self.deltas.len(),
                max: MAX_DELTAS_PER_BATCH,
            }));
        }
        if self.witnesses.len() > MAX_WITNESSES_PER_BATCH {
            return Err(LedgerError::Codec(CodecError::LengthOutOfBounds {
                len: self.witnesses.len(),
                max: MAX_WITNESSES_PER_BATCH,
            }));
        }
        if self.cross_cell_refs.len() > MAX_CROSS_CELL_REFS_PER_BATCH {
            return Err(LedgerError::Codec(CodecError::LengthOutOfBounds {
                len: self.cross_cell_refs.len(),
                max: MAX_CROSS_CELL_REFS_PER_BATCH,
            }));
        }
        if self.format_version == 0 && !self.cross_cell_refs.is_empty() {
            return Err(LedgerError::InvalidField {
                field: "cross_cell_refs",
                detail: "FMT-LEDGER v0 does not encode cross_cell_refs".to_string(),
            });
        }
        for delta in &self.deltas {
            delta.validate()?;
        }
        if self.deltas.windows(2).any(|w| w[0] >= w[1]) {
            return Err(LedgerError::Codec(CodecError::NonCanonicalOrder));
        }
        for witness in &self.witnesses {
            witness.validate()?;
            if witness.kind == WitnessKind::KeyState && self.format_version < FMT_LEDGER_V2 {
                return Err(LedgerError::InvalidField {
                    field: "witness.kind",
                    detail: "KeyState witnesses require FMT-LEDGER v2".to_string(),
                });
            }
        }
        if self.witnesses.windows(2).any(|w| w[0] >= w[1]) {
            return Err(LedgerError::Codec(CodecError::NonCanonicalOrder));
        }
        if self.witnesses.windows(2).any(|w| {
            w[0].kind == WitnessKind::KeyState
                && w[1].kind == WitnessKind::KeyState
                && w[0].subject_id == w[1].subject_id
        }) {
            return Err(LedgerError::InvalidField {
                field: "witness.subject_id",
                detail: "a key may have only one pre-batch KeyState witness".to_string(),
            });
        }
        for cref in &self.cross_cell_refs {
            cref.validate()?;
        }
        if self.cross_cell_refs.windows(2).any(|w| w[0] >= w[1]) {
            return Err(LedgerError::Codec(CodecError::NonCanonicalOrder));
        }
        Ok(())
    }

    /// Encode this batch as a canonical `FMNE` + `LDGR` + CRC32C frame.
    pub fn to_bytes(&self) -> Result<Vec<u8>, LedgerError> {
        self.validate()?;
        let anchor_bytes = self.anchor.to_bytes();
        if anchor_bytes.len() > MAX_ANCHOR_FRAME_BYTES {
            return Err(LedgerError::Codec(CodecError::LengthOutOfBounds {
                len: anchor_bytes.len(),
                max: MAX_ANCHOR_FRAME_BYTES,
            }));
        }
        let mut enc = Encoder::new_frame(TAG_LEDGER_BATCH, self.format_version);
        enc.put_str(&self.cell_id);
        enc.put_u64(self.epoch);
        enc.put_u64(self.seq);
        enc.put_fixed(self.prev_hash.as_bytes());
        enc.put_bytes(&anchor_bytes);
        enc.put_canonical_seq(&self.deltas, |e, d| {
            e.put_str(&d.key);
            e.put_u8(d.kind as u8);
            e.put_fixed(d.payload_digest.as_bytes());
            e.put_bytes(&d.payload);
        })?;
        enc.put_canonical_seq(&self.witnesses, |e, w| {
            e.put_u8(w.kind as u8);
            e.put_str(&w.subject_id);
            e.put_fixed(w.digest.as_bytes());
        })?;
        if self.format_version >= 1 {
            enc.put_canonical_seq(&self.cross_cell_refs, |e, r| {
                e.put_str(&r.cell_id);
                e.put_u64(r.seq);
                e.put_fixed(r.digest.as_bytes());
            })?;
        }
        Ok(enc.finish())
    }

    /// Decode a canonical `FMT-LEDGER-1` schema `v0`, `v1` or `v2` frame and validate all invariants.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, LedgerError> {
        let mut dec = Decoder::open_frame(
            bytes,
            TAG_LEDGER_BATCH,
            FMT_LEDGER_MIN_VERSION,
            FMT_LEDGER_V2,
        )?;
        let format_version = dec.version();
        let cell_id = dec.get_str(MAX_ID_BYTES)?.to_string();
        let epoch = dec.get_u64()?;
        let seq = dec.get_u64()?;
        let prev_hash = Digest32(dec.get_fixed::<32>()?);
        let anchor_bytes = dec.get_bytes(MAX_ANCHOR_FRAME_BYTES)?;
        let anchor = LedgerAnchor::from_bytes(anchor_bytes)?;

        let mut delta_err: Option<LedgerError> = None;
        let deltas = dec.get_canonical_seq(MAX_DELTAS_PER_BATCH, |d| {
            let key = d.get_str(MAX_KEY_BYTES)?.to_string();
            let raw_kind = d.get_u8()?;
            let kind = match DeltaKind::from_u8(raw_kind) {
                Ok(k) => k,
                Err(e) => {
                    if delta_err.is_none() {
                        delta_err = Some(e);
                    }
                    DeltaKind::Put
                }
            };
            let payload_digest = Digest32(d.get_fixed::<32>()?);
            let payload = d.get_bytes(MAX_DELTA_PAYLOAD_BYTES)?.to_vec();
            Ok(DeltaOp {
                key,
                kind,
                payload_digest,
                payload,
            })
        })?;
        if let Some(err) = delta_err {
            return Err(err);
        }

        let mut witness_err: Option<LedgerError> = None;
        let witnesses = dec.get_canonical_seq(MAX_WITNESSES_PER_BATCH, |d| {
            let raw_kind = d.get_u8()?;
            let kind = match WitnessKind::from_u8(raw_kind) {
                Ok(k) => k,
                Err(e) => {
                    if witness_err.is_none() {
                        witness_err = Some(e);
                    }
                    WitnessKind::Conservation
                }
            };
            let max = if kind == WitnessKind::KeyState && format_version >= FMT_LEDGER_V2 {
                MAX_KEY_BYTES
            } else {
                MAX_ID_BYTES
            };
            let subject_id = d.get_str(max)?.to_string();
            let digest = Digest32(d.get_fixed::<32>()?);
            Ok(Witness {
                kind,
                subject_id,
                digest,
            })
        })?;
        if let Some(err) = witness_err {
            return Err(err);
        }

        let cross_cell_refs = if format_version >= 1 {
            dec.get_canonical_seq(MAX_CROSS_CELL_REFS_PER_BATCH, |d| {
                let cell_id = d.get_str(MAX_ID_BYTES)?.to_string();
                let seq = d.get_u64()?;
                let digest = Digest32(d.get_fixed::<32>()?);
                Ok(CrossCellRef {
                    cell_id,
                    seq,
                    digest,
                })
            })?
        } else {
            Vec::new()
        };

        dec.finish()?;
        let batch = Self {
            format_version,
            cell_id,
            epoch,
            seq,
            prev_hash,
            anchor,
            deltas,
            witnesses,
            cross_cell_refs,
        };
        batch.validate()?;
        Ok(batch)
    }

    /// Canonical SHA-256 digest of this batch's serialized `FMT-LEDGER-1` bytes.
    pub fn batch_digest(&self) -> Result<Digest32, LedgerError> {
        Ok(sha256(&self.to_bytes()?))
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use fmine_core::anchor::FMT_ANCHOR_V1;

    pub(crate) fn sample_anchor(cell: &str, epoch: u64, seq: u64) -> LedgerAnchor {
        LedgerAnchor {
            format_version: FMT_ANCHOR_V1,
            cell_id: cell.to_string(),
            ledger_epoch: epoch,
            commit_sequence: seq,
            adapter_registry_epoch: 1,
            schema_epoch: 1,
            policy_epoch: 1,
            privacy_epoch: 1,
            residency_epoch: 1,
            state_root: sha256(format!("{cell}:{epoch}:{seq}").as_bytes()),
        }
    }

    pub(crate) fn sample_batch(
        cell: &str,
        epoch: u64,
        seq: u64,
        prev_hash: Digest32,
    ) -> DeltaBatch {
        let mut deltas = vec![
            DeltaOp::new(
                format!("claim/a/{seq:04}"),
                DeltaKind::Put,
                format!("payload-a-{seq}").into_bytes(),
            ),
            DeltaOp::new(
                format!("claim/b/{seq:04}"),
                DeltaKind::Supersede,
                format!("payload-b-{seq}").into_bytes(),
            ),
        ];
        deltas.sort();
        let mut witnesses = vec![
            Witness::new(
                WitnessKind::Conservation,
                format!("bal/{seq:04}"),
                sha256(b"conservation"),
            ),
            Witness::new(
                WitnessKind::Signature,
                format!("sig/{seq:04}"),
                sha256(b"signature"),
            ),
        ];
        witnesses.sort();
        DeltaBatch {
            format_version: FMT_LEDGER_V1,
            cell_id: cell.to_string(),
            epoch,
            seq,
            prev_hash,
            anchor: sample_anchor(cell, epoch, seq),
            deltas,
            witnesses,
            cross_cell_refs: vec![CrossCellRef::new(
                "CELL-PUBLIC(barrick)",
                0,
                sha256(b"ref-0"),
            )],
        }
    }

    #[test]
    fn test_fmt_ledger_v1_and_n_minus_1_roundtrip_and_one_bit_flip_refusal() {
        let v1 = sample_batch("CELL-SITE(lumwana)", 1, 0, GENESIS_PREV_HASH);
        let bytes = v1.to_bytes().expect("encode v1");
        let decoded = DeltaBatch::from_bytes(&bytes).expect("decode v1");
        assert_eq!(decoded, v1);
        assert_eq!(decoded.to_bytes().expect("re-encode"), bytes);

        // Every single bit flip in the frame must be refused (CRC32C + bounds).
        for idx in 0..bytes.len() {
            let mut corrupted = bytes.clone();
            corrupted[idx] ^= 0x01;
            assert!(
                DeltaBatch::from_bytes(&corrupted).is_err(),
                "bit flip at byte {idx} must be refused"
            );
        }

        // N-1 (v0) round-trip compatibility.
        let mut v0 = v1;
        v0.format_version = 0;
        v0.cross_cell_refs.clear();
        let v0_bytes = v0.to_bytes().expect("encode v0");
        let v0_decoded = DeltaBatch::from_bytes(&v0_bytes).expect("decode v0");
        assert_eq!(v0_decoded, v0);
        assert_eq!(v0_decoded.format_version, 0);
    }

    #[test]
    fn test_fmt_ledger_refuses_disorder_tampered_payload_and_anchor_mismatch() {
        let mut bad_order = sample_batch("CELL-DEC", 1, 0, GENESIS_PREV_HASH);
        bad_order.deltas.reverse();
        assert_eq!(
            bad_order.to_bytes(),
            Err(LedgerError::Codec(CodecError::NonCanonicalOrder))
        );

        let mut bad_digest = sample_batch("CELL-DEC", 1, 0, GENESIS_PREV_HASH);
        bad_digest.deltas[0].payload.push(0xFF);
        assert!(matches!(
            bad_digest.to_bytes(),
            Err(LedgerError::PayloadDigestMismatch { .. })
        ));

        let mut bad_anchor = sample_batch("CELL-DEC", 1, 0, GENESIS_PREV_HASH);
        bad_anchor.anchor.commit_sequence = 9;
        assert!(matches!(
            bad_anchor.to_bytes(),
            Err(LedgerError::AnchorMismatch { .. })
        ));

        let mut bad_genesis = sample_batch("CELL-DEC", 1, 0, sha256(b"nonzero"));
        assert!(matches!(
            bad_genesis.to_bytes(),
            Err(LedgerError::ChainContinuity { .. })
        ));
        bad_genesis.seq = 1;
        bad_genesis.anchor.commit_sequence = 1;
        bad_genesis.prev_hash = GENESIS_PREV_HASH;
        assert!(matches!(
            bad_genesis.to_bytes(),
            Err(LedgerError::ChainContinuity { .. })
        ));
    }

    #[test]
    fn key_state_schema_is_opt_in_and_legacy_bytes_remain_pinned() {
        // FACT: independent Python struct/CRC32C/SHA-256 encoding of the unchanged v0/v1 fields.
        let mut legacy = sample_batch("CELL-SITE(lumwana)", 1, 0, GENESIS_PREV_HASH);
        let bytes = legacy.to_bytes().unwrap();
        assert_eq!(bytes.len(), 486);
        assert_eq!(
            sha256(&bytes).to_hex(),
            "65f03383ca40522e015bb2ef7939cefbba23bda4e36674a10aad91888e4f8cd7"
        );
        legacy.format_version = 0;
        legacy.cross_cell_refs.clear();
        let bytes = legacy.to_bytes().unwrap();
        assert_eq!(bytes.len(), 418);
        assert_eq!(
            sha256(&bytes).to_hex(),
            "0c560966aadbbaf94a233c42a1d31e88594d4147526ea7178138f2ab6fdf4153"
        );

        let mut batch = legacy;
        batch.format_version = FMT_LEDGER_V2;
        batch.witnesses.push(Witness::new(
            WitnessKind::KeyState,
            "sample/a",
            sha256(b"pre-batch-state"),
        ));
        let bytes = batch.to_bytes().unwrap();
        assert_eq!(DeltaBatch::from_bytes(&bytes).unwrap(), batch);
        for version in [0, FMT_LEDGER_V1] {
            batch.format_version = version;
            assert!(matches!(
                batch.to_bytes(),
                Err(LedgerError::InvalidField {
                    field: "witness.kind",
                    ..
                })
            ));

            // A valid CRC cannot downgrade the new witness into an unversioned old meaning.
            let mut downgraded = bytes.clone();
            downgraded[8..10].copy_from_slice(&version.to_le_bytes());
            if version == 0 {
                let refs_start = downgraded.len() - 8;
                downgraded.drain(refs_start..refs_start + 4);
            }
            let crc_at = downgraded.len() - 4;
            let crc = fmine_core::codec::crc32c(&downgraded[..crc_at]);
            downgraded[crc_at..].copy_from_slice(&crc.to_le_bytes());
            assert!(matches!(
                DeltaBatch::from_bytes(&downgraded),
                Err(LedgerError::InvalidField {
                    field: "witness.kind",
                    ..
                })
            ));
        }
    }

    #[test]
    fn key_state_subjects_are_unique_and_support_the_full_key_bound() {
        let mut batch = sample_batch("CELL-EVID", 1, 0, GENESIS_PREV_HASH);
        batch.format_version = FMT_LEDGER_V2;
        batch.witnesses = vec![
            Witness::new(WitnessKind::KeyState, "sample/a", Digest32([0; 32])),
            Witness::new(WitnessKind::KeyState, "sample/a", Digest32([1; 32])),
        ];
        assert!(matches!(
            batch.to_bytes(),
            Err(LedgerError::InvalidField {
                field: "witness.subject_id",
                ..
            })
        ));
        batch.witnesses[1].subject_id = "sample/b".to_string();
        let mut bytes = batch.to_bytes().unwrap();
        let at = bytes.windows(8).position(|w| w == b"sample/b").unwrap();
        bytes[at..at + 8].copy_from_slice(b"sample/a");
        let crc_at = bytes.len() - 4;
        let crc = fmine_core::codec::crc32c(&bytes[..crc_at]);
        bytes[crc_at..].copy_from_slice(&crc.to_le_bytes());
        assert!(matches!(
            DeltaBatch::from_bytes(&bytes),
            Err(LedgerError::InvalidField {
                field: "witness.subject_id",
                ..
            })
        ));
        batch.witnesses[1] = batch.witnesses[0].clone();
        assert_eq!(
            batch.to_bytes(),
            Err(LedgerError::Codec(CodecError::NonCanonicalOrder))
        );

        batch.witnesses.truncate(1);
        batch.witnesses[0].subject_id = "k".repeat(MAX_KEY_BYTES);
        let bytes = batch.to_bytes().unwrap();
        assert_eq!(DeltaBatch::from_bytes(&bytes).unwrap(), batch);
        batch.witnesses[0].subject_id.push('k');
        assert!(batch.to_bytes().is_err());
        batch.witnesses[0].subject_id = "k".repeat(MAX_ID_BYTES + 1);
        batch.witnesses[0].kind = WitnessKind::Conservation;
        assert!(batch.to_bytes().is_err());

        // Existing non-key witness kinds retain their old canonical identity and bounds.
        batch.format_version = FMT_LEDGER_V1;
        batch.witnesses = vec![
            Witness::new(WitnessKind::Conservation, "balance", Digest32([0; 32])),
            Witness::new(WitnessKind::Conservation, "balance", Digest32([1; 32])),
        ];
        let bytes = batch.to_bytes().unwrap();
        assert_eq!(DeltaBatch::from_bytes(&bytes).unwrap(), batch);
    }
}
