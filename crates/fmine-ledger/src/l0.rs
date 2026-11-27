//! Determinism: BitwisePortable
//! L0 in-memory reference ledger and semantic oracle (`BD-080`, plan §26, `INV-003`, `INV-005`,
//! `PO-BED-1`, `PO-BED-5`, `PO-BED-6`).
//!
//! `L0Ledger` is the pure, allocation-bounded, deterministic reference semantics against which L1
//! disk-backed segments (`FMT-SEG-1`) and replication (`fmine-sync`) are checked:
//! - Single-writer monotonic fencing epochs per cell (`PO-BED-1`); lower epochs quarantine (`F6`).
//! - Strict append-only key history (`INV-003`): `Put` introduces a new key and refuses silent
//!   overwrite; `Supersede` appends a new immutable version while preserving every prior version.
//! - Point-in-time `as_of_seq(cell, seq)` snapshots return bit-identical key states before and
//!   after later commits (`GATE-PLUMB-E2`).
//! - Durable obligations (`OB-*`, `PO-BED-5`): `ReserveObligation` opens an obligation;
//!   `ResolveObligation` closes an open obligation and refuses unknown or duplicate closes.
//! - Conservation law hook (`PO-BED-6`, `INV-005`): an optional pre-commit conservation predicate
//!   atomically refuses violating batches before mutating any cell state.
//! - Key-state witnesses (`BD-085`): pre-batch absence or immutable version identity is checked
//!   against the keyed index before any delta is applied, including during replica replay.
//! - Cross-cell gluing (`F12`) and RFC 6962 MMR parity with L1 `FMT-SEG-1` segments.

use std::collections::{BTreeMap, BTreeSet};

use fmine_core::codec::Encoder;
use fmine_core::hash::{Digest32, sha256};
use fmine_evidence::{Refusal, RefusalCode};

use crate::batch::{
    DeltaBatch, DeltaKind, GENESIS_PREV_HASH, LedgerError, MAX_KEY_BYTES, Witness, WitnessKind,
};
use crate::cell::{
    AdmitOutcome, CellAuthority, CellLease, ConflictRecord, GluingReport, check_gluing,
};
use crate::mmr::{Mmr, MmrError};

/// One immutable historical version of a key recorded in an L0 cell (`INV-003`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeyVersion {
    /// Batch sequence number at which this version was committed.
    pub seq: u64,
    /// Fencing epoch of the committing batch.
    pub epoch: u64,
    /// Append-only operation kind (`Put` or `Supersede`).
    pub kind: DeltaKind,
    /// SHA-256 digest of `payload`.
    pub payload_digest: Digest32,
    /// Immutable payload bytes.
    pub payload: Vec<u8>,
}

/// Semantic error returned by [`L0Ledger`] (covers L1 [`LedgerError`] plus semantic law refusals).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum L0Error {
    /// Underlying authority / codec / fencing / hash-chain error.
    Ledger(LedgerError),
    /// Cell has not been registered in this L0 ledger.
    UnknownCell { cell_id: String },
    /// Cell already exists in this L0 ledger.
    DuplicateCell { cell_id: String },
    /// `DeltaKind::Put` attempted to overwrite an existing key without `Supersede` (`INV-003`).
    KeyAlreadyExists { cell_id: String, key: String },
    /// `DeltaKind::Supersede` targeted a key that does not exist (`INV-003`).
    KeyNotFound { cell_id: String, key: String },
    /// A pre-batch key-state witness no longer describes the current immutable version (`BD-085`).
    KeyWitnessMismatch {
        cell_id: String,
        key: String,
        expected: Digest32,
        actual: Digest32,
    },
    /// `ReserveObligation` attempted to reserve an obligation key that is already open (`PO-BED-5`).
    ObligationAlreadyOpen { cell_id: String, key: String },
    /// `ResolveObligation` targeted an obligation key that is not currently open (`PO-BED-5`).
    ObligationNotOpen { cell_id: String, key: String },
    /// Conservation law rejected the batch at commit (`INV-005`, `PO-BED-6`).
    ConservationViolation { cell_id: String, detail: String },
    /// The narrow combiner cannot commit a prepared batch at the current head (`BD-085`): its base is not a head
    /// this cell has had, it transfers the lease, or it supersedes a key it did not witness. The caller recomputes.
    NotRebasable { cell_id: String, detail: String },
    /// Requested `as_of_seq` exceeds the cell's committed sequence count.
    SequenceOutOfRange {
        cell_id: String,
        requested: u64,
        committed: u64,
    },
    /// MMR proof construction error.
    Mmr(MmrError),
}

impl L0Error {
    /// Maps any L0 semantic error to its registered `FMINE-REF-*` refusal.
    #[must_use]
    pub fn to_refusal(&self) -> Refusal {
        match self {
            Self::Ledger(e) => e.to_refusal(),
            Self::ConservationViolation { .. } => {
                Refusal::new(RefusalCode::Conservation101, self.to_string())
            }
            Self::UnknownCell { .. }
            | Self::KeyNotFound { .. }
            | Self::SequenceOutOfRange { .. } => {
                Refusal::new(RefusalCode::NoClaimInDomain201, self.to_string())
            }
            Self::DuplicateCell { .. }
            | Self::KeyAlreadyExists { .. }
            | Self::KeyWitnessMismatch { .. }
            | Self::NotRebasable { .. }
            | Self::ObligationAlreadyOpen { .. }
            | Self::ObligationNotOpen { .. } => {
                Refusal::new(RefusalCode::StaleAnchor502, self.to_string())
            }
            Self::Mmr(_) => Refusal::new(RefusalCode::DigestMismatch505, self.to_string()),
        }
    }
}

impl From<LedgerError> for L0Error {
    fn from(value: LedgerError) -> Self {
        Self::Ledger(value)
    }
}

impl From<MmrError> for L0Error {
    fn from(value: MmrError) -> Self {
        Self::Mmr(value)
    }
}

impl core::fmt::Display for L0Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Ledger(e) => write!(f, "L0 ledger error: {e}"),
            Self::UnknownCell { cell_id } => write!(f, "unknown cell '{cell_id}'"),
            Self::NotRebasable { cell_id, detail } => write!(
                f,
                "prepared batch for '{cell_id}' cannot be committed at the current head: {detail}"
            ),
            Self::DuplicateCell { cell_id } => write!(f, "duplicate cell '{cell_id}'"),
            Self::KeyAlreadyExists { cell_id, key } => {
                write!(
                    f,
                    "append-only violation in '{cell_id}': Put on existing key '{key}' (use Supersede)"
                )
            }
            Self::KeyNotFound { cell_id, key } => {
                write!(
                    f,
                    "cannot supersede nonexistent key '{key}' in cell '{cell_id}'"
                )
            }
            Self::KeyWitnessMismatch {
                cell_id,
                key,
                expected,
                actual,
            } => write!(
                f,
                "stale key witness for '{key}' in cell '{cell_id}': expected {expected}, current {actual}"
            ),
            Self::ObligationAlreadyOpen { cell_id, key } => {
                write!(f, "obligation '{key}' is already open in cell '{cell_id}'")
            }
            Self::ObligationNotOpen { cell_id, key } => {
                write!(f, "obligation '{key}' is not open in cell '{cell_id}'")
            }
            Self::ConservationViolation { cell_id, detail } => {
                write!(f, "conservation refused commit in '{cell_id}': {detail}")
            }
            Self::SequenceOutOfRange {
                cell_id,
                requested,
                committed,
            } => write!(
                f,
                "requested as_of_seq {requested} exceeds committed count {committed} in '{cell_id}'"
            ),
            Self::Mmr(e) => write!(f, "L0 MMR error: {e}"),
        }
    }
}

impl std::error::Error for L0Error {}

/// Point-in-time semantic snapshot of a single L0 cell at a committed sequence count (`as_of_seq`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CellSnapshot {
    pub cell_id: String,
    pub committed_count: u64,
    /// Highest committed fencing epoch at `committed_count` (`1` when `committed_count == 0`).
    pub head_epoch: u64,
    pub head_hash: Digest32,
    pub mmr_root: Digest32,
    /// Latest immutable [`KeyVersion`] for every key existing at `committed_count`.
    pub live_keys: BTreeMap<String, KeyVersion>,
    /// Open obligations still unresolved at `committed_count` (`PO-BED-5`).
    pub open_obligations: BTreeSet<String>,
}

/// Reference state for one authority cell inside [`L0Ledger`].
#[derive(Clone, Debug)]
struct L0Cell {
    authority: CellAuthority,
    history: BTreeMap<String, Vec<KeyVersion>>,
    open_obligations: BTreeSet<String>,
    resolved_obligations: BTreeSet<String>,
}

impl L0Cell {
    fn new(cell_id: impl Into<String>, holder_id: impl Into<String>) -> Result<Self, L0Error> {
        Ok(Self {
            authority: CellAuthority::new(cell_id, holder_id)?,
            history: BTreeMap::new(),
            open_obligations: BTreeSet::new(),
            resolved_obligations: BTreeSet::new(),
        })
    }

    /// DESIGN: `KSTA/1` binds the key's version, not only its value or the cell's current head.
    fn key_witness(&self, key: &str) -> Result<Witness, L0Error> {
        if key.is_empty() || key.len() > MAX_KEY_BYTES {
            return Err(LedgerError::InvalidField {
                field: "witness.subject_id",
                detail: format!("key length {} outside 1..={MAX_KEY_BYTES}", key.len()),
            }
            .into());
        }
        let mut enc = Encoder::new_frame(*b"KSTA", 1);
        enc.put_str(&self.authority.active_lease().cell_id);
        enc.put_str(key);
        if let Some(version) = self.history.get(key).and_then(|versions| versions.last()) {
            enc.put_u8(1);
            enc.put_u64(version.seq);
            enc.put_u64(version.epoch);
            enc.put_u8(version.kind as u8);
            enc.put_fixed(version.payload_digest.as_bytes());
        } else {
            enc.put_u8(0);
        }
        Ok(Witness::new(
            WitnessKind::KeyState,
            key,
            sha256(&enc.finish()),
        ))
    }

    /// Pre-validate every semantic invariant of `batch` without mutating any state.
    fn check_semantics(&self, batch: &DeltaBatch) -> Result<(), L0Error> {
        for witness in &batch.witnesses {
            if witness.kind == WitnessKind::KeyState {
                let current = self.key_witness(&witness.subject_id)?;
                if witness.digest != current.digest {
                    return Err(L0Error::KeyWitnessMismatch {
                        cell_id: batch.cell_id.clone(),
                        key: witness.subject_id.clone(),
                        expected: witness.digest,
                        actual: current.digest,
                    });
                }
            }
        }
        // Only batch-local changes are staged; preflight does not copy the complete cell index.
        let mut new_keys = BTreeSet::new();
        let mut staged_obligations = BTreeMap::new();
        for op in &batch.deltas {
            match op.kind {
                DeltaKind::Put => {
                    if self.history.contains_key(&op.key) || !new_keys.insert(op.key.as_str()) {
                        return Err(L0Error::KeyAlreadyExists {
                            cell_id: batch.cell_id.clone(),
                            key: op.key.clone(),
                        });
                    }
                }
                DeltaKind::Supersede => {
                    if !self.history.contains_key(&op.key) && !new_keys.contains(op.key.as_str()) {
                        return Err(L0Error::KeyNotFound {
                            cell_id: batch.cell_id.clone(),
                            key: op.key.clone(),
                        });
                    }
                }
                DeltaKind::ReserveObligation => {
                    let is_open = staged_obligations
                        .get(op.key.as_str())
                        .copied()
                        .unwrap_or_else(|| self.open_obligations.contains(&op.key));
                    if is_open {
                        return Err(L0Error::ObligationAlreadyOpen {
                            cell_id: batch.cell_id.clone(),
                            key: op.key.clone(),
                        });
                    }
                    staged_obligations.insert(op.key.as_str(), true);
                }
                DeltaKind::ResolveObligation => {
                    let is_open = staged_obligations
                        .get(op.key.as_str())
                        .copied()
                        .unwrap_or_else(|| self.open_obligations.contains(&op.key));
                    if !is_open {
                        return Err(L0Error::ObligationNotOpen {
                            cell_id: batch.cell_id.clone(),
                            key: op.key.clone(),
                        });
                    }
                    staged_obligations.insert(op.key.as_str(), false);
                }
                DeltaKind::TransferLease => {}
            }
        }
        Ok(())
    }

    fn apply_semantics(&mut self, batch: &DeltaBatch) {
        for op in &batch.deltas {
            match op.kind {
                DeltaKind::Put | DeltaKind::Supersede => {
                    self.history
                        .entry(op.key.clone())
                        .or_default()
                        .push(KeyVersion {
                            seq: batch.seq,
                            epoch: batch.epoch,
                            kind: op.kind,
                            payload_digest: op.payload_digest,
                            payload: op.payload.clone(),
                        });
                }
                DeltaKind::ReserveObligation => {
                    self.open_obligations.insert(op.key.clone());
                }
                DeltaKind::ResolveObligation => {
                    self.open_obligations.remove(&op.key);
                    self.resolved_obligations.insert(op.key.clone());
                }
                DeltaKind::TransferLease => {}
            }
        }
    }

    /// Reconstruct exact state after the first `committed_count` batches (`0..=next_seq`).
    fn snapshot_at(&self, committed_count: u64) -> Result<CellSnapshot, L0Error> {
        let cell_id = &self.authority.active_lease().cell_id;
        let total = self.authority.next_seq();
        if committed_count > total {
            return Err(L0Error::SequenceOutOfRange {
                cell_id: cell_id.clone(),
                requested: committed_count,
                committed: total,
            });
        }
        let mut live_keys = BTreeMap::new();
        let mut open_obligations = BTreeSet::new();
        let mut mmr = Mmr::new();
        let mut head_hash = GENESIS_PREV_HASH;
        let mut head_epoch = 1_u64;
        let count_usize =
            usize::try_from(committed_count).map_err(|_| L0Error::SequenceOutOfRange {
                cell_id: cell_id.clone(),
                requested: committed_count,
                committed: total,
            })?;
        for (idx, batch) in self
            .authority
            .batches()
            .iter()
            .take(count_usize)
            .enumerate()
        {
            let digest = self.authority.digests()[idx];
            head_hash = digest;
            head_epoch = batch.epoch;
            mmr.push(digest.as_bytes())?;
            for op in &batch.deltas {
                match op.kind {
                    DeltaKind::Put | DeltaKind::Supersede => {
                        live_keys.insert(
                            op.key.clone(),
                            KeyVersion {
                                seq: batch.seq,
                                epoch: batch.epoch,
                                kind: op.kind,
                                payload_digest: op.payload_digest,
                                payload: op.payload.clone(),
                            },
                        );
                    }
                    DeltaKind::ReserveObligation => {
                        open_obligations.insert(op.key.clone());
                    }
                    DeltaKind::ResolveObligation => {
                        open_obligations.remove(&op.key);
                    }
                    DeltaKind::TransferLease => {}
                }
            }
        }
        Ok(CellSnapshot {
            cell_id: cell_id.clone(),
            committed_count,
            head_epoch,
            head_hash,
            mmr_root: mmr.root(),
            live_keys,
            open_obligations,
        })
    }
}

/// Multi-cell L0 in-memory reference ledger (semantic oracle for L1 segments & sync, `BD-080`).
#[derive(Clone, Debug, Default)]
pub struct L0Ledger {
    cells: BTreeMap<String, L0Cell>,
}

impl L0Ledger {
    /// Construct an empty multi-cell L0 reference ledger.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a new authority cell with genesis writer lease at `epoch = 1`.
    pub fn create_cell(
        &mut self,
        cell_id: impl Into<String>,
        holder_id: impl Into<String>,
    ) -> Result<CellLease, L0Error> {
        let cell_id = cell_id.into();
        if self.cells.contains_key(&cell_id) {
            return Err(L0Error::DuplicateCell { cell_id });
        }
        let cell = L0Cell::new(cell_id.clone(), holder_id)?;
        let lease = cell.authority.active_lease().clone();
        self.cells.insert(cell_id, cell);
        Ok(lease)
    }

    /// Transfer a cell's single-writer lease to `new_holder` at `epoch + 1` (`PO-BED-1`).
    pub fn transfer_lease(
        &mut self,
        cell_id: &str,
        new_holder: impl Into<String>,
    ) -> Result<CellLease, L0Error> {
        let cell = self.cell_mut(cell_id)?;
        Ok(cell.authority.transfer_lease(new_holder)?)
    }

    /// Commit a batch under the active writer lease with an optional conservation check (`PO-BED-6`).
    ///
    /// Atomic: if conservation, fencing, chain continuity, or append-only key/obligation semantics
    /// fail, ZERO state is mutated.
    pub fn commit_with_conservation<F>(
        &mut self,
        lease: &CellLease,
        batch: DeltaBatch,
        check_conservation: F,
    ) -> Result<Digest32, L0Error>
    where
        F: FnOnce(&DeltaBatch) -> Result<(), String>,
    {
        if let Err(detail) = check_conservation(&batch) {
            return Err(L0Error::ConservationViolation {
                cell_id: batch.cell_id,
                detail,
            });
        }
        let cell = self.cell_mut(&lease.cell_id)?;
        if lease.epoch < cell.authority.highest_seen_epoch() {
            return Err(LedgerError::EpochRegression {
                cell_id: lease.cell_id.clone(),
                observed: lease.epoch,
                highest: cell.authority.highest_seen_epoch(),
            }
            .into());
        }
        if lease != cell.authority.active_lease() || batch.cell_id != lease.cell_id {
            return Err(LedgerError::LeaseViolation {
                detail: format!(
                    "lease/batch mismatch for cell '{}': batch cell '{}', lease holder '{}' (epoch {})",
                    lease.cell_id, batch.cell_id, lease.holder_id, lease.epoch
                ),
            }
            .into());
        }
        cell.authority.check_received_batch(&batch)?;
        cell.check_semantics(&batch)?;
        let digest = cell.authority.commit_with_lease(lease, batch.clone())?;
        cell.apply_semantics(&batch);
        Ok(digest)
    }

    /// Commit a batch under the active writer lease (no extra conservation law).
    pub fn commit(&mut self, lease: &CellLease, batch: DeltaBatch) -> Result<Digest32, L0Error> {
        self.commit_with_conservation(lease, batch, |_| Ok(()))
    }

    /// Read the current `Put`/`Supersede` version or absence as an opt-in schema-v2 witness.
    ///
    /// Unrelated cell updates do not invalidate this witness. Superseding the same key does,
    /// even when the new payload bytes equal an earlier version. Obligation state is separate.
    pub fn key_witness(&self, cell_id: &str, key: &str) -> Result<Witness, L0Error> {
        self.cell(cell_id)?.key_witness(key)
    }

    /// Pure admission used before a durable append; stale epochs refuse rather than quarantine.
    pub(crate) fn check_replica(&self, batch: &DeltaBatch) -> Result<(), L0Error> {
        let cell = self.cell(&batch.cell_id)?;
        cell.authority.check_received_batch(batch)?;
        cell.check_semantics(batch)
    }

    /// Replica admission path: quarantines lower-epoch batches (`F6`) without mutating authority state.
    pub fn admit_replica(&mut self, batch: DeltaBatch) -> Result<AdmitOutcome, L0Error> {
        let cell = self.cell_mut(&batch.cell_id)?;
        // Check fencing first so lower-epoch batches quarantine even if keys collided.
        if batch.epoch < cell.authority.highest_seen_epoch() {
            return Ok(cell.authority.admit_received_batch(batch)?);
        }
        cell.authority.check_received_batch(&batch)?;
        cell.check_semantics(&batch)?;
        let outcome = cell.authority.admit_received_batch(batch.clone())?;
        if matches!(outcome, AdmitOutcome::Committed { .. }) {
            cell.apply_semantics(&batch);
        }
        Ok(outcome)
    }

    /// Point-in-time snapshot of `cell_id` at `committed_count` (`0..=next_seq`).
    pub fn as_of_seq(&self, cell_id: &str, committed_count: u64) -> Result<CellSnapshot, L0Error> {
        self.cell(cell_id)?.snapshot_at(committed_count)
    }

    /// Full append-only version history for `key` in `cell_id` (`INV-003`).
    pub fn key_history(&self, cell_id: &str, key: &str) -> Result<&[KeyVersion], L0Error> {
        let cell = self.cell(cell_id)?;
        Ok(cell.history.get(key).map_or(&[], Vec::as_slice))
    }

    /// Currently open obligations in `cell_id` (`PO-BED-5` quiescence check).
    pub fn open_obligations(&self, cell_id: &str) -> Result<&BTreeSet<String>, L0Error> {
        Ok(&self.cell(cell_id)?.open_obligations)
    }

    /// Quarantined lower-epoch conflict records in `cell_id` (`F6`).
    pub fn conflicts(&self, cell_id: &str) -> Result<&[ConflictRecord], L0Error> {
        Ok(self.cell(cell_id)?.authority.conflicts())
    }

    /// Current chain head digest for `cell_id`.
    pub fn head_hash(&self, cell_id: &str) -> Result<Digest32, L0Error> {
        Ok(self.cell(cell_id)?.authority.head_hash())
    }

    /// The `prev_hash` a batch at `seq` carries in `cell_id`: the genesis hash at 0, else the digest of batch
    /// `seq - 1`. Refuses a `seq` past the next one.
    pub fn prev_hash_at(&self, cell_id: &str, seq: u64) -> Result<Digest32, L0Error> {
        let authority = &self.cell(cell_id)?.authority;
        let committed = authority.next_seq();
        if seq > committed {
            return Err(L0Error::SequenceOutOfRange {
                cell_id: cell_id.to_string(),
                requested: seq,
                committed,
            });
        }
        let Some(last) = seq.checked_sub(1) else {
            return Ok(GENESIS_PREV_HASH);
        };
        usize::try_from(last)
            .ok()
            .and_then(|i| authority.digests().get(i).copied())
            .ok_or_else(|| L0Error::SequenceOutOfRange {
                cell_id: cell_id.to_string(),
                requested: seq,
                committed,
            })
    }

    /// Current RFC 6962 MMR root for `cell_id`.
    pub fn mmr_root(&self, cell_id: &str) -> Result<Digest32, L0Error> {
        Ok(self.cell(cell_id)?.authority.mmr_root())
    }

    /// Total committed batch count (`next_seq`) for `cell_id`.
    pub fn committed_count(&self, cell_id: &str) -> Result<u64, L0Error> {
        Ok(self.cell(cell_id)?.authority.next_seq())
    }

    /// Build an RFC 6962 consistency proof between `from_size` and `to_size` for `cell_id`.
    pub fn consistency_proof(
        &self,
        cell_id: &str,
        from_size: u64,
        to_size: u64,
    ) -> Result<Vec<Digest32>, L0Error> {
        Ok(self
            .cell(cell_id)?
            .authority
            .mmr()
            .consistency_proof(from_size, to_size)?)
    }

    /// Verify cross-cell anchor gluing (`F12`) across all cells currently in this L0 ledger.
    #[must_use]
    pub fn check_all_gluing(&self) -> GluingReport {
        let views: BTreeMap<String, Vec<Digest32>> = self
            .cells
            .iter()
            .map(|(id, cell)| (id.clone(), cell.authority.digests().to_vec()))
            .collect();
        let mut total_checked = 0usize;
        let mut all_issues = Vec::new();
        for cell in self.cells.values() {
            for batch in cell.authority.batches() {
                match check_gluing(batch, &views) {
                    GluingReport::Coherent { checked_refs } => total_checked += checked_refs,
                    GluingReport::Dangling(issues) => all_issues.extend(issues),
                }
            }
        }
        if all_issues.is_empty() {
            GluingReport::Coherent {
                checked_refs: total_checked,
            }
        } else {
            GluingReport::Dangling(all_issues)
        }
    }

    fn cell(&self, cell_id: &str) -> Result<&L0Cell, L0Error> {
        self.cells.get(cell_id).ok_or_else(|| L0Error::UnknownCell {
            cell_id: cell_id.to_string(),
        })
    }

    fn cell_mut(&mut self, cell_id: &str) -> Result<&mut L0Cell, L0Error> {
        self.cells
            .get_mut(cell_id)
            .ok_or_else(|| L0Error::UnknownCell {
                cell_id: cell_id.to_string(),
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fmine_core::anchor::{FMT_ANCHOR_V1, LedgerAnchor};
    use fmine_core::hash::sha256;

    use crate::batch::{CrossCellRef, DeltaOp, FMT_LEDGER_V1, FMT_LEDGER_V2};
    use crate::mmr::verify_consistency;
    use crate::segment::SegmentWriter;
    use crate::verify::verify_segment;

    fn mk_batch(
        cell_id: &str,
        epoch: u64,
        seq: u64,
        prev_hash: Digest32,
        deltas: Vec<DeltaOp>,
        cross_cell_refs: Vec<CrossCellRef>,
    ) -> DeltaBatch {
        let anchor = LedgerAnchor {
            format_version: FMT_ANCHOR_V1,
            cell_id: cell_id.to_string(),
            ledger_epoch: epoch,
            commit_sequence: seq,
            adapter_registry_epoch: 1,
            schema_epoch: 1,
            policy_epoch: 1,
            privacy_epoch: 1,
            residency_epoch: 1,
            state_root: prev_hash,
        };
        DeltaBatch {
            format_version: FMT_LEDGER_V1,
            cell_id: cell_id.to_string(),
            epoch,
            seq,
            prev_hash,
            anchor,
            deltas,
            witnesses: Vec::new(),
            cross_cell_refs,
        }
    }

    fn next_witness_batch(
        ledger: &L0Ledger,
        lease: &CellLease,
        mut deltas: Vec<DeltaOp>,
        mut witnesses: Vec<Witness>,
    ) -> DeltaBatch {
        deltas.sort();
        witnesses.sort();
        let mut batch = mk_batch(
            &lease.cell_id,
            lease.epoch,
            ledger.committed_count(&lease.cell_id).unwrap(),
            ledger.head_hash(&lease.cell_id).unwrap(),
            deltas,
            Vec::new(),
        );
        batch.format_version = FMT_LEDGER_V2;
        batch.witnesses = witnesses;
        batch
    }

    fn current(ledger: &L0Ledger, cell_id: &str) -> CellSnapshot {
        ledger
            .as_of_seq(cell_id, ledger.committed_count(cell_id).unwrap())
            .unwrap()
    }

    #[test]
    fn key_witness_known_answers_bind_cell_key_absence_and_full_version() {
        let mut ledger = L0Ledger::new();
        let lease = ledger.create_cell("CELL-EVID(test)", "writer").unwrap();
        ledger.create_cell("CELL-EVID(other)", "writer").unwrap();
        let absent = ledger.key_witness(&lease.cell_id, "sample/a").unwrap();
        // FACT: independent Python struct/CRC32C/SHA-256 encoding of KSTA/1 fields.
        assert_eq!(
            absent.digest.to_hex(),
            "4560ba1761c81757fdac20cbe4c336ca6bd5cc835da591af1b86143b2b6b9423"
        );
        assert_ne!(
            absent.digest,
            ledger
                .key_witness("CELL-EVID(other)", "sample/a")
                .unwrap()
                .digest
        );
        assert_ne!(
            absent.digest,
            ledger
                .key_witness(&lease.cell_id, "sample/b")
                .unwrap()
                .digest
        );
        let batch = next_witness_batch(
            &ledger,
            &lease,
            vec![DeltaOp::new("sample/a", DeltaKind::Put, b"first".to_vec())],
            vec![absent],
        );
        ledger.commit(&lease, batch).unwrap();
        let present = ledger.key_witness(&lease.cell_id, "sample/a").unwrap();
        assert_eq!(
            present.digest.to_hex(),
            "b22735ff1fba71d208f84ab146a350ba3af61ed3ae71c43fdbc3486825f77ce4"
        );
        assert!(ledger.key_witness(&lease.cell_id, "").is_err());
        assert!(
            ledger
                .key_witness(&lease.cell_id, &"k".repeat(MAX_KEY_BYTES + 1))
                .is_err()
        );
        assert!(ledger.key_witness("CELL-UNKNOWN", "sample/a").is_err());
        assert!(
            ledger
                .key_witness(&lease.cell_id, &"k".repeat(MAX_KEY_BYTES))
                .is_ok()
        );
    }

    #[test]
    fn key_witness_ignores_unrelated_writes_and_detects_absent_to_present() {
        let mut ledger = L0Ledger::new();
        let lease = ledger.create_cell("CELL-EVID(test)", "writer").unwrap();
        let absent = ledger.key_witness(&lease.cell_id, "sample/a").unwrap();
        let unrelated = next_witness_batch(
            &ledger,
            &lease,
            vec![DeltaOp::new("sample/b", DeltaKind::Put, b"other".to_vec())],
            Vec::new(),
        );
        ledger.commit(&lease, unrelated).unwrap();
        assert_eq!(
            ledger.key_witness(&lease.cell_id, "sample/a").unwrap(),
            absent
        );
        let create = next_witness_batch(
            &ledger,
            &lease,
            vec![DeltaOp::new("sample/a", DeltaKind::Put, b"first".to_vec())],
            vec![absent.clone()],
        );
        ledger.commit(&lease, create).unwrap();
        let stale = next_witness_batch(&ledger, &lease, Vec::new(), vec![absent]);
        let before = current(&ledger, &lease.cell_id);
        let error = ledger.commit(&lease, stale).unwrap_err();
        assert!(matches!(error, L0Error::KeyWitnessMismatch { .. }));
        assert_eq!(error.to_refusal().code, "FMINE-REF-502");
        assert_eq!(current(&ledger, &lease.cell_id), before);

        let present = ledger.key_witness(&lease.cell_id, "sample/a").unwrap();
        let unrelated = next_witness_batch(
            &ledger,
            &lease,
            vec![DeltaOp::new(
                "sample/b",
                DeltaKind::Supersede,
                b"changed".to_vec(),
            )],
            Vec::new(),
        );
        ledger.commit(&lease, unrelated).unwrap();
        let read_only = next_witness_batch(&ledger, &lease, Vec::new(), vec![present]);
        ledger.commit(&lease, read_only).unwrap();
    }

    #[test]
    fn same_payload_aba_refuses_without_mutating_keys_obligations_or_replica_state() {
        let mut ledger = L0Ledger::new();
        let lease = ledger.create_cell("CELL-FLOW(test)", "writer").unwrap();
        let initial = next_witness_batch(
            &ledger,
            &lease,
            vec![
                DeltaOp::new("sample/a", DeltaKind::Put, b"first".to_vec()),
                DeltaOp::new("ob/open", DeltaKind::ReserveObligation, Vec::new()),
            ],
            Vec::new(),
        );
        ledger.commit(&lease, initial).unwrap();
        let same_value = next_witness_batch(
            &ledger,
            &lease,
            vec![DeltaOp::new(
                "sample/a",
                DeltaKind::Supersede,
                b"first".to_vec(),
            )],
            Vec::new(),
        );
        ledger.commit(&lease, same_value).unwrap();
        let stale = ledger.key_witness(&lease.cell_id, "sample/a").unwrap();
        let old_version = ledger
            .key_history(&lease.cell_id, "sample/a")
            .unwrap()
            .last()
            .unwrap()
            .clone();
        for payload in [b"second".to_vec(), b"first".to_vec()] {
            let batch = next_witness_batch(
                &ledger,
                &lease,
                vec![DeltaOp::new("sample/a", DeltaKind::Supersede, payload)],
                Vec::new(),
            );
            ledger.commit(&lease, batch).unwrap();
        }
        let history = ledger
            .key_history(&lease.cell_id, "sample/a")
            .unwrap()
            .to_vec();
        let restored = history.last().unwrap();
        assert_eq!(
            (
                old_version.epoch,
                old_version.kind,
                old_version.payload_digest
            ),
            (restored.epoch, restored.kind, restored.payload_digest)
        );
        assert_ne!(old_version.seq, restored.seq);
        assert_ne!(
            ledger.key_witness(&lease.cell_id, "sample/a").unwrap(),
            stale
        );
        let bad = next_witness_batch(
            &ledger,
            &lease,
            vec![
                DeltaOp::new("sample/a", DeltaKind::Supersede, b"not committed".to_vec()),
                DeltaOp::new("ob/new", DeltaKind::ReserveObligation, Vec::new()),
                DeltaOp::new("ob/open", DeltaKind::ResolveObligation, Vec::new()),
            ],
            vec![stale],
        );
        let before = current(&ledger, &lease.cell_id);
        for error in [
            ledger.check_replica(&bad).unwrap_err(),
            ledger.commit(&lease, bad.clone()).unwrap_err(),
            ledger.admit_replica(bad.clone()).unwrap_err(),
        ] {
            assert!(matches!(error, L0Error::KeyWitnessMismatch { .. }));
            assert_eq!(error.to_refusal().code, "FMINE-REF-502");
        }
        assert_eq!(current(&ledger, &lease.cell_id), before);
        assert_eq!(
            ledger.key_history(&lease.cell_id, "sample/a").unwrap(),
            history
        );
        assert!(ledger.conflicts(&lease.cell_id).unwrap().is_empty());
        let mut good = bad;
        good.witnesses = vec![ledger.key_witness(&lease.cell_id, "sample/a").unwrap()];
        ledger.commit(&lease, good).unwrap();
        assert_eq!(
            ledger.open_obligations(&lease.cell_id).unwrap(),
            &BTreeSet::from(["ob/new".into()])
        );
        let duplicate_resolution = next_witness_batch(
            &ledger,
            &lease,
            vec![
                DeltaOp::new("ob/new", DeltaKind::ResolveObligation, b"one".to_vec()),
                DeltaOp::new("ob/new", DeltaKind::ResolveObligation, b"two".to_vec()),
            ],
            Vec::new(),
        );
        let before = current(&ledger, &lease.cell_id);
        assert!(matches!(
            ledger.commit(&lease, duplicate_resolution),
            Err(L0Error::ObligationNotOpen { .. })
        ));
        assert_eq!(current(&ledger, &lease.cell_id), before);
    }

    #[test]
    fn witnesses_use_pre_batch_state_and_replay_preserves_legacy_and_ordered_semantics() {
        let mut writer = L0Ledger::new();
        let lease = writer.create_cell("CELL-EVID(test)", "writer").unwrap();
        let mut replica = L0Ledger::new();
        replica.create_cell(&lease.cell_id, "writer").unwrap();
        let initial = mk_batch(
            &lease.cell_id,
            lease.epoch,
            0,
            GENESIS_PREV_HASH,
            vec![DeltaOp::new("sample/a", DeltaKind::Put, b"first".to_vec())],
            Vec::new(),
        );
        writer.commit(&lease, initial.clone()).unwrap();
        replica.admit_replica(initial).unwrap();
        let witnesses = vec![
            writer.key_witness(&lease.cell_id, "sample/a").unwrap(),
            writer.key_witness(&lease.cell_id, "sample/b").unwrap(),
        ];
        let batch = next_witness_batch(
            &writer,
            &lease,
            vec![
                DeltaOp::new("sample/a", DeltaKind::Supersede, b"revised".to_vec()),
                DeltaOp::new("sample/b", DeltaKind::Put, b"new".to_vec()),
                DeltaOp::new("sample/b", DeltaKind::Supersede, b"final".to_vec()),
                DeltaOp::new("ob/one", DeltaKind::ReserveObligation, Vec::new()),
                DeltaOp::new("ob/one", DeltaKind::ResolveObligation, Vec::new()),
            ],
            witnesses,
        );
        let encoded = batch.to_bytes().unwrap();
        writer.commit(&lease, batch).unwrap();
        let decoded = DeltaBatch::from_bytes(&encoded).unwrap();
        replica.check_replica(&decoded).unwrap();
        replica.admit_replica(decoded).unwrap();
        assert_eq!(
            current(&writer, &lease.cell_id),
            current(&replica, &lease.cell_id)
        );
        assert_eq!(
            writer
                .key_history(&lease.cell_id, "sample/b")
                .unwrap()
                .len(),
            2
        );
        assert!(writer.open_obligations(&lease.cell_id).unwrap().is_empty());
        assert_eq!(
            writer.key_witness(&lease.cell_id, "sample/b").unwrap(),
            replica.key_witness(&lease.cell_id, "sample/b").unwrap()
        );
        let duplicate_reservation = next_witness_batch(
            &writer,
            &lease,
            vec![
                DeltaOp::new("ob/one", DeltaKind::ReserveObligation, b"one".to_vec()),
                DeltaOp::new("ob/one", DeltaKind::ReserveObligation, b"two".to_vec()),
            ],
            Vec::new(),
        );
        let before = current(&writer, &lease.cell_id);
        assert!(matches!(
            writer.commit(&lease, duplicate_reservation),
            Err(L0Error::ObligationAlreadyOpen { .. })
        ));
        assert_eq!(current(&writer, &lease.cell_id), before);
    }

    #[test]
    fn test_l0_semantic_oracle_suite_and_l1_segment_parity() {
        let mut l0 = L0Ledger::new();
        let lease1 = l0
            .create_cell("CELL-FLOW(site-a)", "writer-1")
            .expect("create cell A");
        let lease_b = l0
            .create_cell("CELL-EVID(site-a)", "writer-1")
            .expect("create cell B");

        // 1. Put + ReserveObligation in seq 0.
        let b0 = mk_batch(
            "CELL-FLOW(site-a)",
            1,
            0,
            GENESIS_PREV_HASH,
            vec![
                DeltaOp::new("lot/001", DeltaKind::Put, b"dry_t=100".to_vec()),
                DeltaOp::new(
                    "ob/seal-1",
                    DeltaKind::ReserveObligation,
                    b"pending".to_vec(),
                ),
            ],
            Vec::new(),
        );
        let d0 = l0.commit(&lease1, b0.clone()).expect("commit b0");
        assert_eq!(l0.open_obligations("CELL-FLOW(site-a)").unwrap().len(), 1);

        // Snapshot at seq 1 (after b0) before any supersession.
        let snap1_before = l0.as_of_seq("CELL-FLOW(site-a)", 1).expect("snap1");
        assert_eq!(snap1_before.live_keys["lot/001"].payload, b"dry_t=100");
        assert!(snap1_before.open_obligations.contains("ob/seal-1"));

        // 2. Duplicate Put on existing key must refuse without mutating state (INV-003).
        let dup_put = mk_batch(
            "CELL-FLOW(site-a)",
            1,
            1,
            d0,
            vec![DeltaOp::new(
                "lot/001",
                DeltaKind::Put,
                b"overwrite".to_vec(),
            )],
            Vec::new(),
        );
        assert!(matches!(
            l0.commit(&lease1, dup_put),
            Err(L0Error::KeyAlreadyExists { .. })
        ));
        assert_eq!(l0.committed_count("CELL-FLOW(site-a)").unwrap(), 1);

        // 3. Conservation refusal (PO-BED-6) refuses atomically without mutating state.
        let bad_cons = mk_batch(
            "CELL-FLOW(site-a)",
            1,
            1,
            d0,
            vec![DeltaOp::new(
                "lot/002",
                DeltaKind::Put,
                b"unbalanced".to_vec(),
            )],
            Vec::new(),
        );
        let cons_err = l0
            .commit_with_conservation(&lease1, bad_cons, |_| Err("in - out = +5 t".to_string()))
            .unwrap_err();
        assert_eq!(cons_err.to_refusal().code, "FMINE-REF-101");
        assert_eq!(l0.committed_count("CELL-FLOW(site-a)").unwrap(), 1);

        // 4. Lease transfer to epoch 2 (PO-BED-1) + Supersede + ResolveObligation.
        let lease2 = l0
            .transfer_lease("CELL-FLOW(site-a)", "writer-2")
            .expect("transfer lease");
        assert_eq!(lease2.epoch, 2);

        // Old lease1 must be rejected.
        let stale_b = mk_batch(
            "CELL-FLOW(site-a)",
            1,
            1,
            d0,
            vec![DeltaOp::new(
                "lot/001",
                DeltaKind::Supersede,
                b"stale".to_vec(),
            )],
            Vec::new(),
        );
        assert!(l0.commit(&lease1, stale_b.clone()).is_err());

        // Replica admission of stale epoch 1 batch quarantines it cleanly (F6).
        let outcome = l0.admit_replica(stale_b).expect("quarantine");
        assert!(matches!(outcome, AdmitOutcome::Quarantined(_)));
        assert_eq!(l0.conflicts("CELL-FLOW(site-a)").unwrap().len(), 1);

        // Commit valid seq 1 under lease2: supersede lot/001 and resolve ob/seal-1.
        let b1 = mk_batch(
            "CELL-FLOW(site-a)",
            2,
            1,
            d0,
            vec![
                DeltaOp::new("lot/001", DeltaKind::Supersede, b"dry_t=102".to_vec()),
                DeltaOp::new(
                    "ob/seal-1",
                    DeltaKind::ResolveObligation,
                    b"sealed".to_vec(),
                ),
            ],
            Vec::new(),
        );
        let d1 = l0.commit(&lease2, b1.clone()).expect("commit b1");
        assert!(l0.open_obligations("CELL-FLOW(site-a)").unwrap().is_empty());

        // Resolving an already-resolved obligation must refuse (PO-BED-5).
        let double_resolve = mk_batch(
            "CELL-FLOW(site-a)",
            2,
            2,
            d1,
            vec![DeltaOp::new(
                "ob/seal-1",
                DeltaKind::ResolveObligation,
                b"again".to_vec(),
            )],
            Vec::new(),
        );
        assert!(matches!(
            l0.commit(&lease2, double_resolve),
            Err(L0Error::ObligationNotOpen { .. })
        ));

        // 5. Historical immutability: as_of_seq(1) is bit-identical before & after b1; full history preserved.
        let snap1_after = l0.as_of_seq("CELL-FLOW(site-a)", 1).expect("snap1 after");
        assert_eq!(snap1_before, snap1_after);
        let hist = l0.key_history("CELL-FLOW(site-a)", "lot/001").unwrap();
        assert_eq!(hist.len(), 2);
        assert_eq!(hist[0].payload, b"dry_t=100");
        assert_eq!(hist[1].payload, b"dry_t=102");

        // 6. Cross-cell gluing (F12): CELL-EVID references CELL-FLOW seq 1.
        let b_evid = mk_batch(
            "CELL-EVID(site-a)",
            1,
            0,
            GENESIS_PREV_HASH,
            vec![DeltaOp::new("claim/001", DeltaKind::Put, b"c1".to_vec())],
            vec![CrossCellRef::new("CELL-FLOW(site-a)", 1, d1)],
        );
        l0.commit(&lease_b, b_evid).expect("commit evid");
        let gluing = l0.check_all_gluing();
        assert_eq!(gluing, GluingReport::Coherent { checked_refs: 1 });

        // 7. Consistency proof from 1 -> 2 verifies cleanly.
        let proof = l0
            .consistency_proof("CELL-FLOW(site-a)", 1, 2)
            .expect("proof");
        verify_consistency(
            1,
            2,
            &snap1_after.mmr_root,
            &l0.mmr_root("CELL-FLOW(site-a)").unwrap(),
            &proof,
        )
        .expect("RFC 6962 consistency");

        // 8. Differential parity oracle: replay b0, b1 into an L1 disk-backed FMT-SEG-1 segment
        //    and assert bit-identical head_hash, mmr_root, and record_count!
        let tmp = std::env::temp_dir().join(format!(
            "fmine-l0-parity-{}-{}.fmseg",
            std::process::id(),
            sha256(d1.as_bytes()).to_hex().get(..12).unwrap_or("0")
        ));
        let mut writer =
            SegmentWriter::create(&tmp, "CELL-FLOW(site-a)", 0, GENESIS_PREV_HASH, Vec::new())
                .expect("create segment");
        let seg_d0 = writer.append(&b0).expect("append b0");
        let seg_d1 = writer.append(&b1).expect("append b1");
        let _footer = writer.seal().expect("seal segment");
        let verified = verify_segment(&tmp).expect("verify L1 segment");
        let _ = std::fs::remove_file(&tmp);

        assert_eq!(seg_d0, d0);
        assert_eq!(seg_d1, d1);
        assert_eq!(
            verified.record_count,
            l0.committed_count("CELL-FLOW(site-a)").unwrap()
        );
        assert_eq!(
            verified.head_hash,
            l0.head_hash("CELL-FLOW(site-a)").unwrap()
        );
        assert_eq!(verified.mmr_root, l0.mmr_root("CELL-FLOW(site-a)").unwrap());
    }
}
