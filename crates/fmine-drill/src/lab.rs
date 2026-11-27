//! `OB-LAB` (`BD-172`, plan §12.4, §26.7): every dispatched sample resolves to a received result or to a loss
//! declared with its QA/QC finding, so a missing sample is never silent.
//! Determinism: BitwisePortable (integer timestamps, ordered maps and strings).
//!
//! A [`LabBook`] folds a drill cell's committed batches, in sequence order, into `OB-LAB` obligations in an
//! `fmine_runtime` [`ObligationRegistry`], owned by the ingest region ([`OWNER`]):
//!
//! - a dispatch opens one obligation per sample it lists, at its dispatch time, and a correction that adds a
//!   sample opens one for that sample;
//! - the first result for a dispatched sample resolves its obligation at the result's receipt time, and results
//!   for further analytes of the same sample change nothing;
//! - a [`SampleLoss`] aborts it at the declaration time, with the finding as the reason.
//!
//! The commit-time law ([`crate::store::drill_law`]) keeps these consistent: a result or a loss names a sample
//! its dispatch lists, is not dated before the dispatch, and a correction cannot move it to another sample or
//! dispatch. A loss declared after the sample's result arrived, a result for a sample already declared lost, or a
//! second loss for the same sample is kept as a [`LabConflict`]; the obligation stays as it first closed.
//!
//! The registry's journal is the replay record: [`ObligationRegistry::replay`] of it, a fold of the same batches
//! from another copy of the log, or the same transitions written to a `DurableObligationRegistry` and reopened
//! all give the same obligations.

use std::fmt;

use fmine_evidence::{Refusal, RefusalCode};
use fmine_ledger::batch::{DeltaBatch, DeltaKind};
use fmine_runtime::obligations::{
    ObligationError, ObligationJournalEvent, ObligationKind, ObligationRegistry, ObligationState,
};
use fmine_runtime::region::RegionKind;

use crate::records::{DrillError, DrillRecord, SampleLoss, decode_version};
use crate::store::{KEY_PREFIX, parse_key};

/// The region that owns lab obligations: closing it with one open is refused (`PO-BED-5`).
pub const OWNER: RegionKind = RegionKind::Ingest;

/// The obligation id of `sample_id` sent in `dispatch_id`. Ids hold no `/`, so it names one pair.
#[must_use]
pub fn obligation_id(dispatch_id: &str, sample_id: &str) -> String {
    format!("OB-LAB/{dispatch_id}/{sample_id}")
}

/// Records that disagree about how a dispatched sample ended.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LabConflict {
    /// A loss declared for a sample whose result had arrived.
    LossAfterResult {
        /// The dispatch.
        dispatch_id: String,
        /// The sample.
        sample_id: String,
        /// The loss declaration.
        loss_id: String,
    },
    /// A result for a sample already declared lost.
    ResultAfterLoss {
        /// The dispatch.
        dispatch_id: String,
        /// The sample.
        sample_id: String,
        /// The result.
        result_id: String,
    },
    /// A second loss declaration for the same sample.
    RepeatedLoss {
        /// The dispatch.
        dispatch_id: String,
        /// The sample.
        sample_id: String,
        /// The later declaration.
        loss_id: String,
    },
}

/// A batch the book cannot take.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LabError {
    /// A batch of another cell (`FMINE-REF-109`).
    WrongCell {
        /// The book's cell.
        expected: String,
        /// The batch's.
        actual: String,
    },
    /// A batch out of sequence order (`FMINE-REF-502`).
    OutOfOrder {
        /// The sequence the book needs next.
        expected_seq: u64,
        /// The sequence offered.
        seq: u64,
    },
    /// A drill payload that does not decode (its own code).
    Record(DrillError),
    /// An obligation transition the registry refuses, for example a result for a sample no recorded dispatch
    /// sent, which the law would have refused (`FMINE-REF-505`).
    Obligation(ObligationError),
}

impl LabError {
    /// The refusal this error stands for.
    #[must_use]
    pub fn to_refusal(&self) -> Refusal {
        match self {
            Self::WrongCell { .. } => {
                Refusal::new(RefusalCode::OutsidePhysicalDomain109, self.to_string())
            }
            Self::OutOfOrder { .. } => Refusal::new(RefusalCode::StaleAnchor502, self.to_string()),
            Self::Record(e) => e.to_refusal(),
            Self::Obligation(_) => Refusal::new(RefusalCode::DigestMismatch505, self.to_string()),
        }
    }
}

impl fmt::Display for LabError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongCell { expected, actual } => {
                write!(
                    f,
                    "OB-LAB: a batch of cell '{actual}' offered to the book of '{expected}'"
                )
            }
            Self::OutOfOrder { expected_seq, seq } => {
                write!(
                    f,
                    "OB-LAB: batch {seq} offered, batch {expected_seq} is next"
                )
            }
            Self::Record(e) => write!(f, "OB-LAB: {e}"),
            Self::Obligation(e) => write!(f, "OB-LAB: {e}"),
        }
    }
}

impl std::error::Error for LabError {}

impl From<ObligationError> for LabError {
    fn from(e: ObligationError) -> Self {
        Self::Obligation(e)
    }
}

/// The `OB-LAB` obligations of one drill cell, folded from its batches.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LabBook {
    cell_id: Option<String>,
    applied: u64,
    registry: ObligationRegistry,
    conflicts: Vec<LabConflict>,
}

impl LabBook {
    /// A book with nothing applied.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The book of `batches`, a cell's committed batches from sequence 0 in order.
    ///
    /// # Errors
    /// As [`LabBook::apply`].
    pub fn from_batches<'a>(
        batches: impl IntoIterator<Item = &'a DeltaBatch>,
    ) -> Result<Self, LabError> {
        let mut book = Self::new();
        for batch in batches {
            book.apply(batch)?;
        }
        Ok(book)
    }

    /// Apply the cell's next batch.
    ///
    /// # Errors
    /// [`LabError::WrongCell`], [`LabError::OutOfOrder`], [`LabError::Record`] for a drill payload that does not
    /// decode, or [`LabError::Obligation`]. A batch that errs may have applied part of its transitions.
    pub fn apply(&mut self, batch: &DeltaBatch) -> Result<(), LabError> {
        match &self.cell_id {
            Some(cell) if *cell != batch.cell_id => {
                return Err(LabError::WrongCell {
                    expected: cell.clone(),
                    actual: batch.cell_id.clone(),
                });
            }
            _ => {}
        }
        if batch.seq != self.applied {
            return Err(LabError::OutOfOrder {
                expected_seq: self.applied,
                seq: batch.seq,
            });
        }
        // Keys sort dispatch < loss < result, so a dispatch opens before a result or loss in the same batch
        // closes it.
        for op in &batch.deltas {
            if !op.key.starts_with(KEY_PREFIX) || parse_key(&op.key).is_none() {
                continue;
            }
            let version = decode_version(&op.payload).map_err(LabError::Record)?;
            let first = op.kind == DeltaKind::Put;
            match &version.record {
                DrillRecord::Dispatch(d) => {
                    for sample_id in &d.sample_ids {
                        let id = obligation_id(&d.dispatch_id, sample_id);
                        if self.registry.get(&id).is_none() {
                            self.registry.open(
                                id,
                                ObligationKind::ObLab,
                                OWNER,
                                d.dispatched_at_ns,
                            )?;
                        }
                    }
                }
                DrillRecord::AssayResult(r) if first => {
                    let id = obligation_id(&r.dispatch_id, &r.sample_id);
                    match self.registry.get(&id).map(|o| o.state) {
                        Some(ObligationState::Resolved) => {}
                        Some(ObligationState::Aborted) => {
                            self.conflicts.push(LabConflict::ResultAfterLoss {
                                dispatch_id: r.dispatch_id.clone(),
                                sample_id: r.sample_id.clone(),
                                result_id: r.result_id.clone(),
                            })
                        }
                        Some(ObligationState::Open) | None => self.registry.resolve(
                            &id,
                            r.received_at_ns,
                            format!("result '{}' received", r.result_id),
                        )?,
                    }
                }
                DrillRecord::SampleLoss(l) if first => self.declare_lost(l)?,
                _ => {}
            }
        }
        self.cell_id = Some(batch.cell_id.clone());
        self.applied += 1;
        Ok(())
    }

    fn declare_lost(&mut self, l: &SampleLoss) -> Result<(), LabError> {
        let id = obligation_id(&l.dispatch_id, &l.sample_id);
        let (dispatch_id, sample_id, loss_id) = (
            l.dispatch_id.clone(),
            l.sample_id.clone(),
            l.loss_id.clone(),
        );
        match self.registry.get(&id).map(|o| o.state) {
            Some(ObligationState::Resolved) => self.conflicts.push(LabConflict::LossAfterResult {
                dispatch_id,
                sample_id,
                loss_id,
            }),
            Some(ObligationState::Aborted) => self.conflicts.push(LabConflict::RepeatedLoss {
                dispatch_id,
                sample_id,
                loss_id,
            }),
            Some(ObligationState::Open) | None => self.registry.abort(
                &id,
                l.declared_at_ns,
                format!("declared lost ('{loss_id}'): {}", l.finding),
            )?,
        }
        Ok(())
    }

    /// Batches applied.
    #[must_use]
    pub const fn applied(&self) -> u64 {
        self.applied
    }

    /// The obligations.
    #[must_use]
    pub const fn registry(&self) -> &ObligationRegistry {
        &self.registry
    }

    /// The transitions, in order: the replay record.
    #[must_use]
    pub fn journal(&self) -> &[ObligationJournalEvent] {
        self.registry.journal()
    }

    /// Dispatched samples with neither a result nor a declared loss, as `(dispatch, sample)`, in id order.
    #[must_use]
    pub fn outstanding(&self) -> Vec<(String, String)> {
        let mut out: Vec<(String, String)> = self
            .registry
            .open_obligations()
            .into_iter()
            .filter_map(|o| {
                let rest = o.obligation_id.strip_prefix("OB-LAB/")?;
                let (dispatch, sample) = rest.split_once('/')?;
                Some((dispatch.to_string(), sample.to_string()))
            })
            .collect();
        out.sort();
        out
    }

    /// Records that disagree about how a sample ended, in the order they were met.
    #[must_use]
    pub fn conflicts(&self) -> &[LabConflict] {
        &self.conflicts
    }
}
