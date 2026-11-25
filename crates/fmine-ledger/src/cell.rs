//! Determinism: BitwisePortable
//! Cell single-writer leases, monotonic fencing epochs (`BD-084`, `PO-BED-1`, `F6`),
//! and cross-cell gluing verification (`§26.3`, `F12`).
//!
//! Each authority cell has at most one lease-holding writer per epoch (`INV-CELL-1` / `INV-004`).
//! Lease transfer strictly increments `epoch + 1`. Any receiver or authority observing a batch with
//! `batch.epoch < highest_seen_epoch` refuses to apply it to the authority chain and records a
//! [`ConflictRecord`] in quarantine (`F6` split-brain; nothing is merged automatically).

use std::collections::BTreeMap;

use fmine_core::codec::{Decoder, Encoder};
use fmine_core::hash::Digest32;

use crate::batch::{CrossCellRef, DeltaBatch, GENESIS_PREV_HASH, LedgerError, MAX_ID_BYTES};
use crate::mmr::Mmr;

/// Format tag for persisted cell lease records (`FMNE` + `LEAS`).
pub const TAG_CELL_LEASE: [u8; 4] = *b"LEAS";

/// Current schema version for persisted cell lease records.
pub const FMT_LEASE_V1: u16 = 1;

/// Authoritative single-writer lease for a cell (`PO-BED-1`).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct CellLease {
    /// Authority cell identifier.
    pub cell_id: String,
    /// Active writer node / lease holder identifier.
    pub holder_id: String,
    /// Strictly positive monotonic fencing epoch (`>= 1`).
    pub epoch: u64,
    /// Sequence number at which this lease became active.
    pub acquired_seq: u64,
    /// Digest of the sealing batch from the prior epoch (`GENESIS_PREV_HASH` for epoch 1).
    pub prior_head: Digest32,
}

impl CellLease {
    /// Encode this lease as a canonical `FMNE` + `LEAS` + CRC32C frame.
    pub fn to_bytes(&self) -> Result<Vec<u8>, LedgerError> {
        self.validate()?;
        let mut enc = Encoder::new_frame(TAG_CELL_LEASE, FMT_LEASE_V1);
        enc.put_str(&self.cell_id);
        enc.put_str(&self.holder_id);
        enc.put_u64(self.epoch);
        enc.put_u64(self.acquired_seq);
        enc.put_fixed(self.prior_head.as_bytes());
        Ok(enc.finish())
    }

    /// Decode a canonical `FMNE` + `LEAS` frame and validate invariants.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, LedgerError> {
        let mut dec = Decoder::open_frame(bytes, TAG_CELL_LEASE, FMT_LEASE_V1, FMT_LEASE_V1)?;
        let cell_id = dec.get_str(MAX_ID_BYTES)?.to_string();
        let holder_id = dec.get_str(MAX_ID_BYTES)?.to_string();
        let epoch = dec.get_u64()?;
        let acquired_seq = dec.get_u64()?;
        let prior_head = Digest32(dec.get_fixed::<32>()?);
        dec.finish()?;
        let lease = Self {
            cell_id,
            holder_id,
            epoch,
            acquired_seq,
            prior_head,
        };
        lease.validate()?;
        Ok(lease)
    }

    fn validate(&self) -> Result<(), LedgerError> {
        if self.cell_id.is_empty() || self.cell_id.len() > MAX_ID_BYTES {
            return Err(LedgerError::InvalidField {
                field: "lease.cell_id",
                detail: format!("length {} outside 1..={MAX_ID_BYTES}", self.cell_id.len()),
            });
        }
        if self.holder_id.is_empty() || self.holder_id.len() > MAX_ID_BYTES {
            return Err(LedgerError::InvalidField {
                field: "lease.holder_id",
                detail: format!("length {} outside 1..={MAX_ID_BYTES}", self.holder_id.len()),
            });
        }
        if self.epoch == 0 {
            return Err(LedgerError::InvalidField {
                field: "lease.epoch",
                detail: "fencing epoch must be >= 1".to_string(),
            });
        }
        Ok(())
    }
}

/// Split-brain conflict record opened when a lower-epoch batch arrives (`F6`, `PO-BED-1`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConflictRecord {
    /// Cell experiencing the epoch regression.
    pub cell_id: String,
    /// Lower epoch carried by the quarantined batch.
    pub observed_epoch: u64,
    /// Highest fencing epoch already observed for the cell.
    pub highest_seen_epoch: u64,
    /// Sequence number carried by the quarantined batch.
    pub batch_seq: u64,
    /// SHA-256 digest of the quarantined batch.
    pub batch_digest: Digest32,
    /// Human-readable reason for quarantine.
    pub reason: String,
}

/// Outcome of admitting a batch at a cell receiver (`PO-BED-1`, `F6`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AdmitOutcome {
    /// Batch passed fencing, sequence, and hash-chain checks and was appended.
    Committed {
        seq: u64,
        epoch: u64,
        batch_digest: Digest32,
        mmr_root: Digest32,
    },
    /// Batch carried an epoch below `highest_seen_epoch` and was quarantined without mutating state.
    Quarantined(ConflictRecord),
}

/// Single-cell authority state enforcing `PO-BED-1` leases, fencing epochs, hash chaining, and `F6` quarantine.
#[derive(Clone, Debug)]
pub struct CellAuthority {
    cell_id: String,
    active_lease: CellLease,
    highest_seen_epoch: u64,
    next_seq: u64,
    head_hash: Digest32,
    mmr: Mmr,
    batches: Vec<DeltaBatch>,
    digests: Vec<Digest32>,
    conflicts: Vec<ConflictRecord>,
}

impl CellAuthority {
    /// Initialize a new cell with its genesis writer lease at `epoch = 1`.
    pub fn new(
        cell_id: impl Into<String>,
        holder_id: impl Into<String>,
    ) -> Result<Self, LedgerError> {
        let cell_id = cell_id.into();
        let active_lease = CellLease {
            cell_id: cell_id.clone(),
            holder_id: holder_id.into(),
            epoch: 1,
            acquired_seq: 0,
            prior_head: GENESIS_PREV_HASH,
        };
        active_lease.validate()?;
        Ok(Self {
            cell_id,
            active_lease,
            highest_seen_epoch: 1,
            next_seq: 0,
            head_hash: GENESIS_PREV_HASH,
            mmr: Mmr::new(),
            batches: Vec::new(),
            digests: Vec::new(),
            conflicts: Vec::new(),
        })
    }

    /// Active writer lease for this cell.
    #[must_use]
    pub const fn active_lease(&self) -> &CellLease {
        &self.active_lease
    }

    /// Highest fencing epoch observed by this cell.
    #[must_use]
    pub const fn highest_seen_epoch(&self) -> u64 {
        self.highest_seen_epoch
    }

    /// Current committed sequence count (next expected `seq`).
    #[must_use]
    pub const fn next_seq(&self) -> u64 {
        self.next_seq
    }

    /// Current SHA-256 chain head (`GENESIS_PREV_HASH` if empty).
    #[must_use]
    pub const fn head_hash(&self) -> Digest32 {
        self.head_hash
    }

    /// Current RFC 6962 MMR root over committed batch digests.
    #[must_use]
    pub fn mmr_root(&self) -> Digest32 {
        self.mmr.root()
    }

    /// Underlying MMR for inclusion and consistency proofs.
    #[must_use]
    pub const fn mmr(&self) -> &Mmr {
        &self.mmr
    }

    /// All committed batches in sequence order.
    #[must_use]
    pub fn batches(&self) -> &[DeltaBatch] {
        &self.batches
    }

    /// All committed batch digests in sequence order.
    #[must_use]
    pub fn digests(&self) -> &[Digest32] {
        &self.digests
    }

    /// Quarantined lower-epoch split-brain conflict records (`F6`).
    #[must_use]
    pub fn conflicts(&self) -> &[ConflictRecord] {
        &self.conflicts
    }

    /// Transfer the writer lease to `new_holder`, advancing the fencing epoch strictly by `+1` (`PO-BED-1`).
    /// The caller must persist the returned [`CellLease`] before emitting batches under the new epoch.
    pub fn transfer_lease(
        &mut self,
        new_holder: impl Into<String>,
    ) -> Result<CellLease, LedgerError> {
        let new_holder = new_holder.into();
        if new_holder.is_empty() || new_holder.len() > MAX_ID_BYTES {
            return Err(LedgerError::InvalidField {
                field: "lease.holder_id",
                detail: format!("length {} outside 1..={MAX_ID_BYTES}", new_holder.len()),
            });
        }
        let next_epoch =
            self.highest_seen_epoch
                .checked_add(1)
                .ok_or_else(|| LedgerError::LeaseViolation {
                    detail: "fencing epoch overflow".to_string(),
                })?;
        let lease = CellLease {
            cell_id: self.cell_id.clone(),
            holder_id: new_holder,
            epoch: next_epoch,
            acquired_seq: self.next_seq,
            prior_head: self.head_hash,
        };
        lease.validate()?;
        self.highest_seen_epoch = next_epoch;
        self.active_lease = lease.clone();
        Ok(lease)
    }

    /// Commit a batch produced by a writer holding `lease` (`PO-BED-1`).
    /// Refuses with [`LedgerError::EpochRegression`] or [`LedgerError::LeaseViolation`] if the lease is stale.
    pub fn commit_with_lease(
        &mut self,
        lease: &CellLease,
        batch: DeltaBatch,
    ) -> Result<Digest32, LedgerError> {
        if lease.cell_id != self.cell_id {
            return Err(LedgerError::LeaseViolation {
                detail: format!(
                    "lease cell '{}' != authority cell '{}'",
                    lease.cell_id, self.cell_id
                ),
            });
        }
        if lease.epoch < self.highest_seen_epoch {
            return Err(LedgerError::EpochRegression {
                cell_id: self.cell_id.clone(),
                observed: lease.epoch,
                highest: self.highest_seen_epoch,
            });
        }
        if lease != &self.active_lease {
            return Err(LedgerError::LeaseViolation {
                detail: format!(
                    "lease holder '{}' (epoch {}) is not active holder '{}' (epoch {})",
                    lease.holder_id,
                    lease.epoch,
                    self.active_lease.holder_id,
                    self.active_lease.epoch
                ),
            });
        }
        if batch.epoch != lease.epoch {
            return Err(LedgerError::LeaseViolation {
                detail: format!(
                    "batch epoch {} does not match held lease epoch {}",
                    batch.epoch, lease.epoch
                ),
            });
        }
        match self.admit_received_batch(batch)? {
            AdmitOutcome::Committed { batch_digest, .. } => Ok(batch_digest),
            AdmitOutcome::Quarantined(conflict) => Err(LedgerError::EpochRegression {
                cell_id: conflict.cell_id,
                observed: conflict.observed_epoch,
                highest: conflict.highest_seen_epoch,
            }),
        }
    }

    /// Admit a batch at a receiver (`PO-BED-1`, `F6`):
    /// - Lower-epoch batches are quarantined with a [`ConflictRecord`] and never applied.
    /// - Valid current/higher-epoch batches advance `highest_seen_epoch`, verify sequence & hash chain,
    ///   and append to the log and MMR.
    pub fn admit_received_batch(&mut self, batch: DeltaBatch) -> Result<AdmitOutcome, LedgerError> {
        batch.validate()?;
        if batch.cell_id != self.cell_id {
            return Err(LedgerError::ChainContinuity {
                detail: format!(
                    "batch cell '{}' != authority cell '{}'",
                    batch.cell_id, self.cell_id
                ),
            });
        }
        let batch_digest = batch.batch_digest()?;
        if batch.epoch < self.highest_seen_epoch {
            let conflict = ConflictRecord {
                cell_id: self.cell_id.clone(),
                observed_epoch: batch.epoch,
                highest_seen_epoch: self.highest_seen_epoch,
                batch_seq: batch.seq,
                batch_digest,
                reason: format!(
                    "split-brain epoch regression: batch epoch {} < highest seen {}",
                    batch.epoch, self.highest_seen_epoch
                ),
            };
            self.conflicts.push(conflict.clone());
            return Ok(AdmitOutcome::Quarantined(conflict));
        }
        if batch.seq != self.next_seq {
            return Err(LedgerError::ChainContinuity {
                detail: format!(
                    "expected seq {} in cell '{}', got {}",
                    self.next_seq, self.cell_id, batch.seq
                ),
            });
        }
        if batch.prev_hash != self.head_hash {
            return Err(LedgerError::ChainContinuity {
                detail: format!(
                    "expected prev_hash {} at seq {}, got {}",
                    self.head_hash, batch.seq, batch.prev_hash
                ),
            });
        }
        self.mmr
            .push(batch_digest.as_bytes())
            .map_err(|e| LedgerError::ChainContinuity {
                detail: format!("MMR append failed: {e}"),
            })?;
        self.highest_seen_epoch = batch.epoch;
        self.head_hash = batch_digest;
        self.next_seq =
            self.next_seq
                .checked_add(1)
                .ok_or_else(|| LedgerError::ChainContinuity {
                    detail: "sequence counter overflow".to_string(),
                })?;
        let seq = batch.seq;
        let epoch = batch.epoch;
        self.batches.push(batch);
        self.digests.push(batch_digest);
        Ok(AdmitOutcome::Committed {
            seq,
            epoch,
            batch_digest,
            mmr_root: self.mmr.root(),
        })
    }
}

/// Dangling or mismatched cross-cell reference detected by the gluing checker (`§26.3`, `F12`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GluingIssue {
    /// Referenced cell is absent from the enterprise anchor view (`not_observable`).
    MissingCell { cell_id: String, seq: u64 },
    /// Referenced sequence is ahead of the held high-water mark (`stale`).
    SeqAheadOfAnchor {
        cell_id: String,
        requested_seq: u64,
        held_len: u64,
    },
    /// Held digest at `(cell_id, seq)` does not match the reference digest.
    DigestMismatch {
        cell_id: String,
        seq: u64,
        expected: Digest32,
        held: Digest32,
    },
}

/// Result of running the enterprise cross-cell gluing checker (`§26.3`, `F12`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GluingReport {
    /// Every `(cell, seq, digest)` cross-cell reference resolved cleanly.
    Coherent { checked_refs: usize },
    /// One or more references dangled; dependent Claims must not be published (`F12`).
    Dangling(Vec<GluingIssue>),
}

/// Check that every [`CrossCellRef`] in `batch` resolves against the held per-cell batch digests (`§26.3`, `F12`).
#[must_use]
pub fn check_gluing(
    batch: &DeltaBatch,
    held_cells: &BTreeMap<String, Vec<Digest32>>,
) -> GluingReport {
    let mut issues = Vec::new();
    for cref in &batch.cross_cell_refs {
        check_single_ref(cref, held_cells, &mut issues);
    }
    if issues.is_empty() {
        GluingReport::Coherent {
            checked_refs: batch.cross_cell_refs.len(),
        }
    } else {
        GluingReport::Dangling(issues)
    }
}

fn check_single_ref(
    cref: &CrossCellRef,
    held_cells: &BTreeMap<String, Vec<Digest32>>,
    issues: &mut Vec<GluingIssue>,
) {
    let Some(digests) = held_cells.get(&cref.cell_id) else {
        issues.push(GluingIssue::MissingCell {
            cell_id: cref.cell_id.clone(),
            seq: cref.seq,
        });
        return;
    };
    let Ok(idx) = usize::try_from(cref.seq) else {
        issues.push(GluingIssue::SeqAheadOfAnchor {
            cell_id: cref.cell_id.clone(),
            requested_seq: cref.seq,
            held_len: digests.len() as u64,
        });
        return;
    };
    let Some(&held) = digests.get(idx) else {
        issues.push(GluingIssue::SeqAheadOfAnchor {
            cell_id: cref.cell_id.clone(),
            requested_seq: cref.seq,
            held_len: digests.len() as u64,
        });
        return;
    };
    if held != cref.digest {
        issues.push(GluingIssue::DigestMismatch {
            cell_id: cref.cell_id.clone(),
            seq: cref.seq,
            expected: cref.digest,
            held,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::batch::tests::sample_batch;
    use fmine_core::hash::sha256;

    #[test]
    fn test_po_bed_1_lease_transfer_fencing_and_split_brain_quarantine() {
        let mut cell = CellAuthority::new("CELL-SITE(lumwana)", "node-a").expect("init cell");
        let lease_e1 = cell.active_lease().clone();

        // Lease binary round-trip.
        let lease_bytes = lease_e1.to_bytes().expect("encode lease");
        assert_eq!(
            CellLease::from_bytes(&lease_bytes).expect("decode lease"),
            lease_e1
        );

        let b0 = sample_batch("CELL-SITE(lumwana)", 1, 0, GENESIS_PREV_HASH);
        let d0 = cell.commit_with_lease(&lease_e1, b0).expect("commit seq 0");
        assert_eq!(cell.next_seq(), 1);
        assert_eq!(cell.head_hash(), d0);

        // Transfer lease to node-b -> epoch advances strictly to 2.
        let lease_e2 = cell.transfer_lease("node-b").expect("transfer lease");
        assert_eq!(lease_e2.epoch, 2);
        assert_eq!(lease_e2.acquired_seq, 1);
        assert_eq!(lease_e2.prior_head, d0);

        // Stale writer node-a attempting direct commit under epoch 1 is refused.
        let stale_b1 = sample_batch("CELL-SITE(lumwana)", 1, 1, d0);
        assert!(matches!(
            cell.commit_with_lease(&lease_e1, stale_b1.clone()),
            Err(LedgerError::EpochRegression {
                observed: 1,
                highest: 2,
                ..
            })
        ));
        assert_eq!(cell.next_seq(), 1);

        // Stale epoch-1 batch arriving at a receiver is quarantined with a ConflictRecord (F6),
        // leaving committed state untouched.
        let outcome = cell
            .admit_received_batch(stale_b1)
            .expect("quarantine outcome");
        assert!(matches!(
            outcome,
            AdmitOutcome::Quarantined(ConflictRecord {
                observed_epoch: 1,
                highest_seen_epoch: 2,
                batch_seq: 1,
                ..
            })
        ));
        assert_eq!(cell.conflicts().len(), 1);
        assert_eq!(cell.next_seq(), 1);
        assert_eq!(cell.head_hash(), d0);

        // New writer node-b commits cleanly at epoch 2.
        let b1 = sample_batch("CELL-SITE(lumwana)", 2, 1, d0);
        let d1 = cell.commit_with_lease(&lease_e2, b1).expect("commit seq 1");
        assert_eq!(cell.next_seq(), 2);
        assert_eq!(cell.head_hash(), d1);
        assert_eq!(cell.mmr().len(), 2);
    }

    /// `PO-BED-1`: one holder per epoch. A lease the cell did not grant is refused, whether it names
    /// another holder at the live epoch or the live holder at a later epoch, as is a lease for another
    /// cell and a batch whose epoch is not its lease's; none of them changes the cell.
    #[test]
    fn test_po_bed_1_one_holder_per_epoch_refuses_leases_the_cell_did_not_grant() {
        let mut cell = CellAuthority::new("CELL-SITE(lumwana)", "node-a").expect("init cell");
        let lease_e1 = cell.active_lease().clone();
        let d0 = cell
            .commit_with_lease(
                &lease_e1,
                sample_batch("CELL-SITE(lumwana)", 1, 0, GENESIS_PREV_HASH),
            )
            .expect("commit seq 0");
        let lease_e2 = cell.transfer_lease("node-b").expect("transfer lease");

        let mut forged = lease_e2.clone();
        forged.holder_id = "node-c".to_string();
        let mut other_cell = lease_e2.clone();
        other_cell.cell_id = "CELL-SITE(other)".to_string();
        let mut ahead = lease_e2.clone();
        ahead.epoch = 3;
        // Each batch carries its lease's own epoch, so the batch-epoch check cannot be what refuses it: the
        // cell-id check refuses `other_cell`, and the check that the lease is the one granted the other two.
        for lease in [&forged, &other_cell, &ahead] {
            assert!(
                matches!(
                    cell.commit_with_lease(
                        lease,
                        sample_batch("CELL-SITE(lumwana)", lease.epoch, 1, d0)
                    ),
                    Err(LedgerError::LeaseViolation { .. })
                ),
                "{lease:?}"
            );
        }
        assert!(matches!(
            cell.commit_with_lease(&lease_e2, sample_batch("CELL-SITE(lumwana)", 1, 1, d0)),
            Err(LedgerError::LeaseViolation { .. })
        ));
        assert_eq!(cell.next_seq(), 1);
        assert_eq!(cell.head_hash(), d0);
        assert_eq!(cell.highest_seen_epoch(), 2);
        assert!(cell.conflicts().is_empty());

        cell.commit_with_lease(&lease_e2, sample_batch("CELL-SITE(lumwana)", 2, 1, d0))
            .expect("the granted lease still commits");
        assert_eq!(cell.next_seq(), 2);
    }

    #[test]
    fn test_cross_cell_gluing_checker_detects_missing_stale_and_mismatched_refs() {
        let d_pub0 = sha256(b"ref-0");
        let mut batch = sample_batch("CELL-DEC", 1, 0, GENESIS_PREV_HASH);
        batch.cross_cell_refs = vec![CrossCellRef::new("CELL-PUBLIC(barrick)", 0, d_pub0)];

        let mut held = BTreeMap::new();
        assert_eq!(
            check_gluing(&batch, &held),
            GluingReport::Dangling(vec![GluingIssue::MissingCell {
                cell_id: "CELL-PUBLIC(barrick)".to_string(),
                seq: 0,
            }])
        );

        held.insert("CELL-PUBLIC(barrick)".to_string(), vec![]);
        assert_eq!(
            check_gluing(&batch, &held),
            GluingReport::Dangling(vec![GluingIssue::SeqAheadOfAnchor {
                cell_id: "CELL-PUBLIC(barrick)".to_string(),
                requested_seq: 0,
                held_len: 0,
            }])
        );

        held.insert("CELL-PUBLIC(barrick)".to_string(), vec![sha256(b"wrong")]);
        assert_eq!(
            check_gluing(&batch, &held),
            GluingReport::Dangling(vec![GluingIssue::DigestMismatch {
                cell_id: "CELL-PUBLIC(barrick)".to_string(),
                seq: 0,
                expected: d_pub0,
                held: sha256(b"wrong"),
            }])
        );

        held.insert("CELL-PUBLIC(barrick)".to_string(), vec![d_pub0]);
        assert_eq!(
            check_gluing(&batch, &held),
            GluingReport::Coherent { checked_refs: 1 }
        );
    }
}
