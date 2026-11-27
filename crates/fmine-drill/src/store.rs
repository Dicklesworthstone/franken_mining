//! Drillhole records on an authority-ledger cell: keys, the commit-time law, writes and reads (`BD-163`).
//! Determinism: BitwisePortable (integer, byte and ordered-map operations only).
//!
//! - **Keys.** A record lives at `drill/<kind>/<id>` ([`record_key`]). Its first version is a `Put`; each
//!   correction is a `Supersede` with a reason (plan §8.1, `INV-003`: corrections supersede, nothing is edited or
//!   deleted). The ledger itself refuses a `Put` over a recorded key (`KeyAlreadyExists`, `FMINE-REF-502`) and a
//!   `Supersede` of an unrecorded one (`KeyNotFound`, `FMINE-REF-201`).
//! - **Law.** [`drill_law`] runs at commit time on either rung: [`commit`] on the L0 writer path under a lease, and
//!   `SemanticSegmentWriter::append_with_law(&batch, drill_law)` on the durable segment, with the batch from
//!   [`batch_for`] and [`delta_ops`]. This crate does no IO; the durable append is its caller's. The law sees the
//!   cell before the batch and the whole batch, and refuses (`FMINE-REF-101`, nothing written) when, for any
//!   `drill/` key in the batch:
//!   - the payload is not one canonical version of a record of that key's kind and id;
//!   - a first version carries a correction, or a correction carries none;
//!   - the record names a record that is neither recorded nor written in the same batch (a survey of a hole with
//!     no collar; a result whose certificate is not recorded);
//!   - an assay result names a dispatch that does not list its sample, or that went to another lab;
//!   - a dispatch correction changes the lab or drops a sample. Either would break a result already admitted
//!     against it; a sample that never arrived is declared lost (`OB-LAB`, BD-172), not edited out;
//!   - a CRM or blank names a sample that was drilled, or a duplicate or umpire names inserted QC material
//!     ([`SampleType::QcMaterial`]) on either side; a sample correction cannot change which of the two it is;
//!   - a result is received, or a loss declared, before its dispatch was sent; a loss names a sample its dispatch
//!     does not list; a correction moves a result or a loss to another sample or dispatch (`OB-LAB`, BD-172);
//!   - a twin pair matches an interval of another hole than the side it is on, or a correction changes the holes
//!     it compares (BD-285);
//!   - a dispatch newly lists a sample whose material was discarded after QC at or before the dispatch time
//!     ([`crate::records::SampleDiscard`], BD-286): re-assay of it is impossible.
//!
//!   Ids never disappear, so a reference that held when it was written holds after any later correction.
//! - **Reads.** [`current`], [`history`] and [`as_of`] decode a key's versions from the ledger.

use std::collections::BTreeMap;
use std::fmt;

use fmine_core::anchor::{FMT_ANCHOR_V1, LedgerAnchor};
use fmine_core::hash::Digest32;
use fmine_evidence::Refusal;
use fmine_ledger::batch::{DeltaBatch, DeltaKind, DeltaOp, FMT_LEDGER_V2};
use fmine_ledger::cell::CellLease;
use fmine_ledger::l0::{L0Error, L0Ledger};

use crate::records::{
    AssayResult, Dispatch, DrillError, DrillRecord, QcKind, RecordKind, RecordVersion, Sample,
    SampleType, decode_version, encode_version,
};

/// Prefix of every drillhole record key.
pub const KEY_PREFIX: &str = "drill/";

/// The ledger key of record `id` of `kind`.
#[must_use]
pub fn record_key(kind: RecordKind, id: &str) -> String {
    format!("{KEY_PREFIX}{}/{id}", kind.code())
}

/// The kind and id a drillhole record key names, or `None` for any other key.
#[must_use]
pub fn parse_key(key: &str) -> Option<(RecordKind, &str)> {
    let rest = key.strip_prefix(KEY_PREFIX)?;
    let (code, id) = rest.split_once('/')?;
    Some((RecordKind::from_code(code)?, id))
}

/// One change to the drillhole records.
#[derive(Clone, Debug, PartialEq)]
pub enum Change {
    /// The first version of a record.
    Put(DrillRecord),
    /// A correction of a recorded record, with its reason.
    Supersede {
        /// The corrected record; same kind and id as the one it replaces.
        record: DrillRecord,
        /// Why.
        reason: String,
    },
}

impl Change {
    /// The record this change writes.
    #[must_use]
    pub const fn record(&self) -> &DrillRecord {
        match self {
            Self::Put(record) | Self::Supersede { record, .. } => record,
        }
    }

    /// Its ledger key.
    #[must_use]
    pub fn key(&self) -> String {
        let record = self.record();
        record_key(record.kind(), record.id())
    }

    /// Its ledger operation.
    ///
    /// # Errors
    /// The record's or the reason's [`DrillError`].
    pub fn to_op(&self) -> Result<DeltaOp, DrillError> {
        let (kind, reason) = match self {
            Self::Put(_) => (DeltaKind::Put, None),
            Self::Supersede { reason, .. } => (DeltaKind::Supersede, Some(reason.as_str())),
        };
        Ok(DeltaOp::new(
            self.key(),
            kind,
            encode_version(self.record(), reason)?,
        ))
    }
}

/// A drillhole write or read that did not happen.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StoreError {
    /// A record, a reason or a batch that cannot be written, or stored bytes that cannot be read.
    Record(DrillError),
    /// The ledger refused, including [`drill_law`]'s refusals (`L0Error::ConservationViolation`).
    Ledger(L0Error),
}

impl StoreError {
    /// The refusal this error stands for.
    #[must_use]
    pub fn to_refusal(&self) -> Refusal {
        match self {
            Self::Record(e) => e.to_refusal(),
            Self::Ledger(e) => e.to_refusal(),
        }
    }
}

impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Record(e) => e.fmt(f),
            Self::Ledger(e) => e.fmt(f),
        }
    }
}

impl std::error::Error for StoreError {}

impl From<DrillError> for StoreError {
    fn from(e: DrillError) -> Self {
        Self::Record(e)
    }
}

impl From<L0Error> for StoreError {
    fn from(e: L0Error) -> Self {
        Self::Ledger(e)
    }
}

/// The ledger operations for `changes`, in canonical order.
///
/// # Errors
/// [`StoreError::Record`] for an invalid record or reason, or for two changes to one record in one batch.
pub fn delta_ops(changes: &[Change]) -> Result<Vec<DeltaOp>, StoreError> {
    let mut ops = Vec::with_capacity(changes.len());
    for change in changes {
        ops.push(change.to_op()?);
    }
    ops.sort();
    if let Some(w) = ops.windows(2).find(|w| w[0].key == w[1].key) {
        return Err(StoreError::Record(DrillError::Invalid {
            field: "batch".to_string(),
            detail: format!("two changes to '{}' in one batch", w[0].key),
        }));
    }
    Ok(ops)
}

/// The next batch of `cell_id` in `ledger`, at `epoch`, holding `deltas` (sorted here).
///
/// # Errors
/// [`StoreError::Ledger`] for an unknown cell.
pub fn batch_for(
    ledger: &L0Ledger,
    cell_id: &str,
    epoch: u64,
    mut deltas: Vec<DeltaOp>,
) -> Result<DeltaBatch, StoreError> {
    let seq = ledger.committed_count(cell_id)?;
    let prev_hash = ledger.head_hash(cell_id)?;
    deltas.sort();
    Ok(DeltaBatch {
        format_version: FMT_LEDGER_V2,
        cell_id: cell_id.to_string(),
        epoch,
        seq,
        prev_hash,
        anchor: LedgerAnchor {
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
        },
        deltas,
        witnesses: Vec::new(),
        cross_cell_refs: Vec::new(),
    })
}

/// Commit `changes` on the L0 writer path under `lease`, with [`drill_law`].
///
/// # Errors
/// [`StoreError::Record`] before anything reaches the ledger; [`StoreError::Ledger`] for the ledger's own
/// refusals and the law's. Nothing changes on any error.
pub fn commit(
    ledger: &mut L0Ledger,
    lease: &CellLease,
    changes: &[Change],
) -> Result<Digest32, StoreError> {
    let batch = batch_for(ledger, &lease.cell_id, lease.epoch, delta_ops(changes)?)?;
    Ok(ledger.commit_with_conservation(lease, batch, drill_law)?)
}

/// The live version at `key` before the batch, decoded, if any.
fn recorded(ledger: &L0Ledger, cell_id: &str, key: &str) -> Result<Option<RecordVersion>, String> {
    let versions = ledger
        .key_history(cell_id, key)
        .map_err(|e| e.to_string())?;
    versions
        .last()
        .map(|v| decode_version(&v.payload).map_err(|e| format!("recorded '{key}': {e}")))
        .transpose()
}

/// The commit-time law for drillhole records; see the module documentation.
///
/// # Errors
/// The first violation found, as text; the ledger reports it as `ConservationViolation` (`FMINE-REF-101`).
pub fn drill_law(ledger: &L0Ledger, batch: &DeltaBatch) -> Result<(), String> {
    let cell_id = batch.cell_id.as_str();
    let mut staged: BTreeMap<&str, RecordVersion> = BTreeMap::new();
    for op in &batch.deltas {
        let Some(rest) = op.key.strip_prefix(KEY_PREFIX) else {
            continue;
        };
        let Some((kind, id)) = parse_key(&op.key) else {
            return Err(format!("'{}': no drill record kind '{rest}'", op.key));
        };
        let version = decode_version(&op.payload).map_err(|e| format!("'{}': {e}", op.key))?;
        if version.record.kind() != kind || version.record.id() != id {
            return Err(format!(
                "'{}' holds {} '{}'",
                op.key,
                version.record.kind().code(),
                version.record.id()
            ));
        }
        match (op.kind, &version.correction) {
            (DeltaKind::Put, None) | (DeltaKind::Supersede, Some(_)) => {}
            (DeltaKind::Put, Some(_)) => {
                return Err(format!(
                    "'{}': a first version carries no correction",
                    op.key
                ));
            }
            (DeltaKind::Supersede, None) => {
                return Err(format!("'{}': a correction needs its reason", op.key));
            }
            (other, _) => {
                return Err(format!(
                    "'{}': a drill record is written by Put or Supersede, not {other:?}",
                    op.key
                ));
            }
        }
        if op.kind == DeltaKind::Supersede
            && let Some(old) = recorded(ledger, cell_id, &op.key)?
        {
            match (&old.record, &version.record) {
                (DrillRecord::Dispatch(old), DrillRecord::Dispatch(new)) => {
                    check_dispatch_correction(old, new)?;
                }
                (DrillRecord::AssayResult(old), DrillRecord::AssayResult(new))
                    if (&old.sample_id, &old.dispatch_id) != (&new.sample_id, &new.dispatch_id) =>
                {
                    return Err(format!(
                        "result '{}': a correction cannot change its sample or dispatch",
                        new.result_id
                    ));
                }
                (DrillRecord::SampleLoss(old), DrillRecord::SampleLoss(new))
                    if (&old.sample_id, &old.dispatch_id) != (&new.sample_id, &new.dispatch_id) =>
                {
                    return Err(format!(
                        "loss '{}': a correction cannot change its sample or dispatch",
                        new.loss_id
                    ));
                }
                (DrillRecord::TwinPair(old), DrillRecord::TwinPair(new))
                    if (&old.original_hole_id, &old.twin_hole_id)
                        != (&new.original_hole_id, &new.twin_hole_id) =>
                {
                    return Err(format!(
                        "twin '{}': a correction cannot change the holes it compares",
                        new.twin_id
                    ));
                }
                (DrillRecord::Sample(old), DrillRecord::Sample(new))
                    if is_qc_material(old) != is_qc_material(new) =>
                {
                    return Err(format!(
                        "sample '{}': a correction cannot change whether it is inserted QC material",
                        new.sample_id
                    ));
                }
                _ => {}
            }
        }
        if staged.insert(op.key.as_str(), version).is_some() {
            return Err(format!("two versions of '{}' in one batch", op.key));
        }
    }
    for (key, version) in &staged {
        for (kind, id) in version.record.references() {
            let target = record_key(kind, id);
            if !staged.contains_key(target.as_str())
                && ledger
                    .key_history(cell_id, &target)
                    .map_err(|e| e.to_string())?
                    .is_empty()
            {
                return Err(format!(
                    "'{key}' names {} '{id}', which is not recorded",
                    kind.code()
                ));
            }
        }
        let live = |kind: RecordKind, id: &str| -> Result<DrillRecord, String> {
            let target = record_key(kind, id);
            match staged.get(target.as_str()) {
                Some(v) => Ok(v.record.clone()),
                None => recorded(ledger, cell_id, &target)?
                    .map(|v| v.record)
                    .ok_or_else(|| {
                        format!(
                            "'{key}' names {} '{id}', which is not recorded",
                            kind.code()
                        )
                    }),
            }
        };
        match &version.record {
            DrillRecord::AssayResult(result) => {
                if let DrillRecord::Dispatch(dispatch) =
                    live(RecordKind::Dispatch, &result.dispatch_id)?
                {
                    check_result_dispatch(key, result, &dispatch)?;
                }
            }
            DrillRecord::SampleLoss(loss) => {
                if let DrillRecord::Dispatch(dispatch) =
                    live(RecordKind::Dispatch, &loss.dispatch_id)?
                {
                    check_listed(key, &dispatch, &loss.sample_id)?;
                    check_after_dispatch(key, "declared lost", loss.declared_at_ns, &dispatch)?;
                }
            }
            DrillRecord::Dispatch(dispatch) => {
                // A correction keeps the samples it already listed; only newly listed ones are checked.
                let before = match recorded(ledger, cell_id, key)? {
                    Some(RecordVersion {
                        record: DrillRecord::Dispatch(d),
                        ..
                    }) => d.sample_ids,
                    _ => Vec::new(),
                };
                for sample_id in &dispatch.sample_ids {
                    if before.binary_search(sample_id).is_ok() {
                        continue;
                    }
                    let discard_key = record_key(RecordKind::SampleDiscard, sample_id);
                    let discard = match staged.get(discard_key.as_str()) {
                        Some(v) => Some(v.record.clone()),
                        None => recorded(ledger, cell_id, &discard_key)?.map(|v| v.record),
                    };
                    if let Some(DrillRecord::SampleDiscard(d)) = discard
                        && d.discarded_at_ns <= dispatch.dispatched_at_ns
                    {
                        return Err(format!(
                            "'{key}': sample '{sample_id}' was discarded after QC ({}); its material cannot be sent",
                            d.reason
                        ));
                    }
                }
            }
            DrillRecord::TwinPair(twin) => {
                for m in &twin.matches {
                    for (id, hole) in [
                        (&m.original_interval_id, &twin.original_hole_id),
                        (&m.twin_interval_id, &twin.twin_hole_id),
                    ] {
                        match live(RecordKind::Interval, id)? {
                            DrillRecord::Interval(interval) if interval.hole_id == *hole => {}
                            DrillRecord::Interval(interval) => {
                                return Err(format!(
                                    "'{key}': interval '{id}' is in hole '{}', not '{hole}'",
                                    interval.hole_id
                                ));
                            }
                            other => {
                                return Err(format!(
                                    "'{key}': '{id}' holds a {}",
                                    other.kind().code()
                                ));
                            }
                        }
                    }
                }
            }
            DrillRecord::QcInsertion(qc) => {
                let qc_material = |id: &str| -> Result<bool, String> {
                    match live(RecordKind::Sample, id)? {
                        DrillRecord::Sample(sample) => Ok(is_qc_material(&sample)),
                        other => Err(format!("'{key}': '{id}' holds a {}", other.kind().code())),
                    }
                };
                match &qc.kind {
                    QcKind::Crm { .. } | QcKind::Blank { .. } => {
                        if !qc_material(&qc.sample_id)? {
                            return Err(format!(
                                "'{key}': a CRM or blank is inserted QC material, and sample '{}' was drilled",
                                qc.sample_id
                            ));
                        }
                    }
                    QcKind::Duplicate {
                        original_sample_id, ..
                    }
                    | QcKind::Umpire {
                        original_sample_id, ..
                    } => {
                        for id in [&qc.sample_id, original_sample_id] {
                            if qc_material(id)? {
                                return Err(format!(
                                    "'{key}': a duplicate or umpire checks a drilled sample, and '{id}' is \
                                     inserted QC material"
                                ));
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }
    Ok(())
}

fn is_qc_material(sample: &Sample) -> bool {
    sample.sample_type == SampleType::QcMaterial
}

fn check_dispatch_correction(old: &Dispatch, new: &Dispatch) -> Result<(), String> {
    if old.lab != new.lab {
        return Err(format!(
            "dispatch '{}': a correction cannot move it from lab '{}' to '{}'",
            new.dispatch_id, old.lab, new.lab
        ));
    }
    if let Some(dropped) = old
        .sample_ids
        .iter()
        .find(|s| new.sample_ids.binary_search(s).is_err())
    {
        return Err(format!(
            "dispatch '{}': a correction cannot drop sample '{dropped}'; declare it lost",
            new.dispatch_id
        ));
    }
    Ok(())
}

fn check_listed(key: &str, dispatch: &Dispatch, sample_id: &str) -> Result<(), String> {
    if dispatch
        .sample_ids
        .binary_search_by(|s| s.as_str().cmp(sample_id))
        .is_err()
    {
        return Err(format!(
            "'{key}': dispatch '{}' does not list sample '{sample_id}'",
            dispatch.dispatch_id
        ));
    }
    Ok(())
}

fn check_after_dispatch(
    key: &str,
    what: &str,
    at_ns: i64,
    dispatch: &Dispatch,
) -> Result<(), String> {
    if at_ns < dispatch.dispatched_at_ns {
        return Err(format!(
            "'{key}': {what} at {at_ns} ns, before dispatch '{}' left at {} ns",
            dispatch.dispatch_id, dispatch.dispatched_at_ns
        ));
    }
    Ok(())
}

fn check_result_dispatch(
    key: &str,
    result: &AssayResult,
    dispatch: &Dispatch,
) -> Result<(), String> {
    check_listed(key, dispatch, &result.sample_id)?;
    check_after_dispatch(key, "received", result.received_at_ns, dispatch)?;
    if dispatch.lab != result.lab {
        return Err(format!(
            "'{key}': reported by lab '{}', dispatched to lab '{}'",
            result.lab, dispatch.lab
        ));
    }
    Ok(())
}

/// One stored version of a record.
#[derive(Clone, Debug, PartialEq)]
pub struct StoredVersion {
    /// Sequence of the batch that wrote it.
    pub seq: u64,
    /// Fencing epoch of that batch.
    pub epoch: u64,
    /// The version.
    pub version: RecordVersion,
}

/// Every version of record `id` of `kind` in `cell_id`, oldest first; empty if it was never recorded.
///
/// # Errors
/// [`StoreError::Ledger`] for an unknown cell; [`StoreError::Record`] for stored bytes that do not decode.
pub fn history(
    ledger: &L0Ledger,
    cell_id: &str,
    kind: RecordKind,
    id: &str,
) -> Result<Vec<StoredVersion>, StoreError> {
    ledger
        .key_history(cell_id, &record_key(kind, id))?
        .iter()
        .map(|v| {
            Ok(StoredVersion {
                seq: v.seq,
                epoch: v.epoch,
                version: decode_version(&v.payload)?,
            })
        })
        .collect()
}

/// The live version of record `id` of `kind`, if recorded.
///
/// # Errors
/// As [`history`].
pub fn current(
    ledger: &L0Ledger,
    cell_id: &str,
    kind: RecordKind,
    id: &str,
) -> Result<Option<RecordVersion>, StoreError> {
    Ok(history(ledger, cell_id, kind, id)?.pop().map(|v| v.version))
}

/// The version of record `id` of `kind` that was live once `committed_count` batches had committed.
///
/// # Errors
/// As [`history`].
pub fn as_of(
    ledger: &L0Ledger,
    cell_id: &str,
    committed_count: u64,
    kind: RecordKind,
    id: &str,
) -> Result<Option<RecordVersion>, StoreError> {
    Ok(history(ledger, cell_id, kind, id)?
        .into_iter()
        .rev()
        .find(|v| v.seq < committed_count)
        .map(|v| v.version))
}
