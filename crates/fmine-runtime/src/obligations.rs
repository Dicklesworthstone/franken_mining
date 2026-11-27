//! Determinism: BitwisePortable
//! Durable obligation registry (`OB-*`) and region-close leak guard (`BD-089`, `PO-BED-5`,
//! plan §26.6–§26.7).
//!
//! Tracks the plan's eight obligation families (`OB-SYNC-ACK`, `OB-SEAL`, `OB-POST`, `OB-REVIEW`, `OB-EFFECT`,
//! `OB-LAB`, `OB-TARP`, `OB-COMMIT`) scoped to owning regions. Every state transition (`Opened`, `Resolved`,
//! `Aborted`) is recorded in an append-only journal that replays deterministically. Closing a region while any
//! obligation owned by that region remains `Open` refuses with [`ObligationError::RegionLeak`] (`PO-BED-5`: no
//! silent obligation leak on region close).
//!
//! [`ObligationRegistry`] holds the journal in memory. [`DurableObligationRegistry`] writes each transition to a
//! ledger segment through `fmine-ledger`'s `SemanticSegmentWriter` before it counts, so the journal survives a
//! crash and replays on open.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::path::Path;

use fmine_core::anchor::{FMT_ANCHOR_V1, LedgerAnchor};
use fmine_core::codec::{Decoder, Encoder};
use fmine_ledger::batch::{DeltaBatch, DeltaKind, DeltaOp, FMT_LEDGER_V2};
use fmine_ledger::l0::L0Error;
use fmine_ledger::segment::SemanticSegmentWriter;

use crate::region::{REGIONS, RegionKind};

/// The durable obligation families of plan §26.7 (`BD-089`), in the plan's order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ObligationKind {
    /// A batch is shipped; resolves on an enterprise ack naming `(cell, seq, mmr_root)` (`OB-SYNC-ACK`).
    ObSyncAck,
    /// Chunks are staged; resolves when the root is published or the staged set is aborted (`OB-SEAL`).
    ObSeal,
    /// A posting slot is reserved; resolves on an FA `EntryRef` and certificate digest, or a recorded refusal
    /// (`OB-POST`).
    ObPost,
    /// A Workpaper is prepared; resolves on a human `Reviewed{against}` attestation or a rejection (`OB-REVIEW`).
    ObReview,
    /// An effect is prepared; resolves on a human authority-token decision, then reconciled actuals (`OB-EFFECT`).
    ObEffect,
    /// A sample is dispatched; resolves when the assay is received or the sample is declared lost (`OB-LAB`).
    ObLab,
    /// A trigger is raised; resolves when it is acknowledged and escalated per the TARP (`OB-TARP`).
    ObTarp,
    /// A commitment falls due; resolves on attested evidence of performance or a recorded miss (`OB-COMMIT`).
    ObCommit,
}

impl ObligationKind {
    /// Every family, in the plan's order.
    pub const ALL: [Self; 8] = [
        Self::ObSyncAck,
        Self::ObSeal,
        Self::ObPost,
        Self::ObReview,
        Self::ObEffect,
        Self::ObLab,
        Self::ObTarp,
        Self::ObCommit,
    ];

    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::ObSyncAck => "OB-SYNC-ACK",
            Self::ObSeal => "OB-SEAL",
            Self::ObPost => "OB-POST",
            Self::ObReview => "OB-REVIEW",
            Self::ObEffect => "OB-EFFECT",
            Self::ObLab => "OB-LAB",
            Self::ObTarp => "OB-TARP",
            Self::ObCommit => "OB-COMMIT",
        }
    }

    /// The family with this code, if any.
    #[must_use]
    pub fn from_code(code: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.code() == code)
    }
}

/// Lifecycle state of one durable obligation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ObligationState {
    Open,
    Resolved,
    Aborted,
}

/// Durable metadata for one obligation entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObligationRecord {
    pub obligation_id: String,
    pub kind: ObligationKind,
    pub owner_region: RegionKind,
    pub opened_at_ns: i64,
    pub closed_at_ns: Option<i64>,
    pub state: ObligationState,
    pub resolution_note: Option<String>,
}

/// Append-only journal event written for crash-safe obligation replay (`PO-BED-5`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ObligationJournalEvent {
    Opened {
        obligation_id: String,
        kind: ObligationKind,
        owner_region: RegionKind,
        at_ns: i64,
    },
    Resolved {
        obligation_id: String,
        at_ns: i64,
        note: String,
    },
    Aborted {
        obligation_id: String,
        at_ns: i64,
        reason: String,
    },
}

/// Typed errors raised by [`ObligationRegistry`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ObligationError {
    EmptyId,
    DuplicateId {
        obligation_id: String,
    },
    UnknownId {
        obligation_id: String,
    },
    AlreadyClosed {
        obligation_id: String,
        state: ObligationState,
    },
    TimeWentBack {
        obligation_id: String,
        opened_at_ns: i64,
        at_ns: i64,
    },
    /// `PO-BED-5`: attempted to close `region` while obligations remained `Open`.
    RegionLeak {
        region: RegionKind,
        open_obligation_ids: Vec<String>,
    },
    /// The ledger segment refused or failed. A refused transition is not recorded; after a persistence error the
    /// handle refuses further writes and the segment must be reopened.
    Store(L0Error),
    /// The segment holds a journal this build cannot replay, or one that disagrees with the ledger's own record of
    /// which obligations are open.
    Journal {
        detail: String,
    },
}

impl fmt::Display for ObligationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyId => write!(f, "obligation_id must be non-empty"),
            Self::DuplicateId { obligation_id } => {
                write!(f, "duplicate obligation_id '{obligation_id}'")
            }
            Self::UnknownId { obligation_id } => {
                write!(f, "unknown obligation_id '{obligation_id}'")
            }
            Self::AlreadyClosed {
                obligation_id,
                state,
            } => write!(
                f,
                "obligation '{obligation_id}' is already closed in state {state:?}"
            ),
            Self::TimeWentBack {
                obligation_id,
                opened_at_ns,
                at_ns,
            } => write!(
                f,
                "obligation '{obligation_id}' close time {at_ns} precedes opened_at_ns {opened_at_ns}"
            ),
            Self::RegionLeak {
                region,
                open_obligation_ids,
            } => write!(
                f,
                "PO-BED-5 leak: cannot close region {:?} with {} open obligation(s): {}",
                region,
                open_obligation_ids.len(),
                open_obligation_ids.join(", ")
            ),
            Self::Store(e) => write!(f, "obligation journal store: {e}"),
            Self::Journal { detail } => write!(f, "obligation journal: {detail}"),
        }
    }
}

impl std::error::Error for ObligationError {}

/// Durable obligation registry enforcing `PO-BED-5` (no region closes with open obligations)
/// and deterministic journal replay after crash (`BD-089`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ObligationRegistry {
    records: BTreeMap<String, ObligationRecord>,
    journal: Vec<ObligationJournalEvent>,
}

impl ObligationRegistry {
    /// Construct an empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Reconstruct registry state deterministically by replaying an append-only journal (`PO-BED-5`).
    pub fn replay(events: &[ObligationJournalEvent]) -> Result<Self, ObligationError> {
        let mut reg = Self::new();
        for ev in events {
            match ev {
                ObligationJournalEvent::Opened {
                    obligation_id,
                    kind,
                    owner_region,
                    at_ns,
                } => {
                    reg.open(obligation_id.clone(), *kind, *owner_region, *at_ns)?;
                }
                ObligationJournalEvent::Resolved {
                    obligation_id,
                    at_ns,
                    note,
                } => {
                    reg.resolve(obligation_id, *at_ns, note.clone())?;
                }
                ObligationJournalEvent::Aborted {
                    obligation_id,
                    at_ns,
                    reason,
                } => {
                    reg.abort(obligation_id, *at_ns, reason.clone())?;
                }
            }
        }
        Ok(reg)
    }

    /// Open a new durable obligation owned by `owner_region`.
    pub fn open(
        &mut self,
        obligation_id: impl Into<String>,
        kind: ObligationKind,
        owner_region: RegionKind,
        at_ns: i64,
    ) -> Result<(), ObligationError> {
        let obligation_id = obligation_id.into();
        if obligation_id.trim().is_empty() {
            return Err(ObligationError::EmptyId);
        }
        if self.records.contains_key(&obligation_id) {
            return Err(ObligationError::DuplicateId { obligation_id });
        }
        self.records.insert(
            obligation_id.clone(),
            ObligationRecord {
                obligation_id: obligation_id.clone(),
                kind,
                owner_region,
                opened_at_ns: at_ns,
                closed_at_ns: None,
                state: ObligationState::Open,
                resolution_note: None,
            },
        );
        self.journal.push(ObligationJournalEvent::Opened {
            obligation_id,
            kind,
            owner_region,
            at_ns,
        });
        Ok(())
    }

    /// Mark an open obligation as `Resolved`.
    pub fn resolve(
        &mut self,
        obligation_id: &str,
        at_ns: i64,
        note: impl Into<String>,
    ) -> Result<(), ObligationError> {
        let rec =
            self.records
                .get_mut(obligation_id)
                .ok_or_else(|| ObligationError::UnknownId {
                    obligation_id: obligation_id.to_string(),
                })?;
        if rec.state != ObligationState::Open {
            return Err(ObligationError::AlreadyClosed {
                obligation_id: obligation_id.to_string(),
                state: rec.state,
            });
        }
        if at_ns < rec.opened_at_ns {
            return Err(ObligationError::TimeWentBack {
                obligation_id: obligation_id.to_string(),
                opened_at_ns: rec.opened_at_ns,
                at_ns,
            });
        }
        let note = note.into();
        rec.state = ObligationState::Resolved;
        rec.closed_at_ns = Some(at_ns);
        rec.resolution_note = Some(note.clone());
        self.journal.push(ObligationJournalEvent::Resolved {
            obligation_id: obligation_id.to_string(),
            at_ns,
            note,
        });
        Ok(())
    }

    /// Mark an open obligation as explicitly `Aborted` (never silently dropped).
    pub fn abort(
        &mut self,
        obligation_id: &str,
        at_ns: i64,
        reason: impl Into<String>,
    ) -> Result<(), ObligationError> {
        let rec =
            self.records
                .get_mut(obligation_id)
                .ok_or_else(|| ObligationError::UnknownId {
                    obligation_id: obligation_id.to_string(),
                })?;
        if rec.state != ObligationState::Open {
            return Err(ObligationError::AlreadyClosed {
                obligation_id: obligation_id.to_string(),
                state: rec.state,
            });
        }
        if at_ns < rec.opened_at_ns {
            return Err(ObligationError::TimeWentBack {
                obligation_id: obligation_id.to_string(),
                opened_at_ns: rec.opened_at_ns,
                at_ns,
            });
        }
        let reason = reason.into();
        rec.state = ObligationState::Aborted;
        rec.closed_at_ns = Some(at_ns);
        rec.resolution_note = Some(reason.clone());
        self.journal.push(ObligationJournalEvent::Aborted {
            obligation_id: obligation_id.to_string(),
            at_ns,
            reason,
        });
        Ok(())
    }

    /// Enforce `PO-BED-5`: closing `region` succeeds ONLY when zero obligations owned by `region`
    /// remain `Open`. Returns `Err(ObligationError::RegionLeak)` naming every leaked obligation.
    pub fn check_region_close(&self, region: RegionKind) -> Result<(), ObligationError> {
        let open: Vec<String> = self
            .records
            .values()
            .filter(|r| r.owner_region == region && r.state == ObligationState::Open)
            .map(|r| r.obligation_id.clone())
            .collect();
        if open.is_empty() {
            Ok(())
        } else {
            Err(ObligationError::RegionLeak {
                region,
                open_obligation_ids: open,
            })
        }
    }

    /// All currently open obligations across all regions (for `fmine doctor` / crash recovery).
    #[must_use]
    pub fn open_obligations(&self) -> Vec<&ObligationRecord> {
        self.records
            .values()
            .filter(|r| r.state == ObligationState::Open)
            .collect()
    }

    /// Read-only access to the append-only obligation journal.
    #[must_use]
    pub fn journal(&self) -> &[ObligationJournalEvent] {
        &self.journal
    }

    /// Look up one obligation by id.
    #[must_use]
    pub fn get(&self, obligation_id: &str) -> Option<&ObligationRecord> {
        self.records.get(obligation_id)
    }

    /// Take back the last transition, which the durable store refused.
    fn undo_last(&mut self) {
        match self.journal.pop() {
            Some(ObligationJournalEvent::Opened { obligation_id, .. }) => {
                self.records.remove(&obligation_id);
            }
            Some(
                ObligationJournalEvent::Resolved { obligation_id, .. }
                | ObligationJournalEvent::Aborted { obligation_id, .. },
            ) => {
                if let Some(rec) = self.records.get_mut(&obligation_id) {
                    rec.state = ObligationState::Open;
                    rec.closed_at_ns = None;
                    rec.resolution_note = None;
                }
            }
            None => {}
        }
    }
}

/// Every batch the registry writes is at this fencing epoch: one node owns its journal's segment.
const JOURNAL_EPOCH: u64 = 1;
const EVENT_TAG: [u8; 4] = *b"OBEV";
const EVENT_VERSION: u16 = 1;

/// The ledger key of journal event `index`. Zero-padded, so the keys sort in journal order.
fn event_key(index: usize) -> String {
    format!("ev/{index:016}")
}

/// The ledger obligation key, which the ledger itself reserves and resolves.
fn obligation_key(obligation_id: &str) -> String {
    format!("ob/{obligation_id}")
}

/// A region's stable name in the journal: its variant name.
fn region_name(region: RegionKind) -> String {
    format!("{region:?}")
}

fn region_from_name(name: &str) -> Option<RegionKind> {
    REGIONS
        .iter()
        .map(|spec| spec.kind)
        .find(|kind| region_name(*kind) == name)
}

/// One journal event as an `FMNE` frame (`OBEV`, version 1, CRC32C).
fn encode_event(event: &ObligationJournalEvent) -> Vec<u8> {
    let mut enc = Encoder::new_frame(EVENT_TAG, EVENT_VERSION);
    match event {
        ObligationJournalEvent::Opened {
            obligation_id,
            kind,
            owner_region,
            at_ns,
        } => {
            enc.put_u8(1);
            enc.put_str(obligation_id);
            enc.put_str(kind.code());
            enc.put_str(&region_name(*owner_region));
            enc.put_i64(*at_ns);
        }
        ObligationJournalEvent::Resolved {
            obligation_id,
            at_ns,
            note,
        } => {
            enc.put_u8(2);
            enc.put_str(obligation_id);
            enc.put_i64(*at_ns);
            enc.put_str(note);
        }
        ObligationJournalEvent::Aborted {
            obligation_id,
            at_ns,
            reason,
        } => {
            enc.put_u8(3);
            enc.put_str(obligation_id);
            enc.put_i64(*at_ns);
            enc.put_str(reason);
        }
    }
    enc.finish()
}

fn decode_event(bytes: &[u8]) -> Result<ObligationJournalEvent, String> {
    let mut dec = Decoder::open_frame(bytes, EVENT_TAG, EVENT_VERSION, EVENT_VERSION)
        .map_err(|e| e.to_string())?;
    // No string can be longer than the frame that holds it.
    let max = bytes.len();
    let text = |dec: &mut Decoder<'_>| {
        dec.get_str(max)
            .map(str::to_string)
            .map_err(|e| e.to_string())
    };
    let event = match dec.get_u8().map_err(|e| e.to_string())? {
        1 => {
            let obligation_id = text(&mut dec)?;
            let code = text(&mut dec)?;
            let kind = ObligationKind::from_code(&code)
                .ok_or_else(|| format!("unknown obligation family '{code}'"))?;
            let name = text(&mut dec)?;
            let owner_region =
                region_from_name(&name).ok_or_else(|| format!("unknown region '{name}'"))?;
            let at_ns = dec.get_i64().map_err(|e| e.to_string())?;
            ObligationJournalEvent::Opened {
                obligation_id,
                kind,
                owner_region,
                at_ns,
            }
        }
        tag @ (2 | 3) => {
            let obligation_id = text(&mut dec)?;
            let at_ns = dec.get_i64().map_err(|e| e.to_string())?;
            let words = text(&mut dec)?;
            if tag == 2 {
                ObligationJournalEvent::Resolved {
                    obligation_id,
                    at_ns,
                    note: words,
                }
            } else {
                ObligationJournalEvent::Aborted {
                    obligation_id,
                    at_ns,
                    reason: words,
                }
            }
        }
        tag => return Err(format!("unknown event tag {tag}")),
    };
    dec.finish().map_err(|e| e.to_string())?;
    Ok(event)
}

/// An [`ObligationRegistry`] whose journal lives in one ledger cell's `FMT-SEG-1` segment (`BD-089`, `PO-BED-5`).
///
/// Each transition is one ledger batch: a `Put` of the encoded event under `ev/<index>`, with a
/// `ReserveObligation` (opened) or `ResolveObligation` (resolved or aborted) of `ob/<id>`. The registry checks the
/// transition first; the batch is synced before the call returns, and a refused or failed append takes the
/// transition back. [`Self::open`] replays the events in order and requires the ledger's own open-obligation set to
/// equal the registry's. The segment brings its CRCs, hash chain, exclusive lock and torn-tail refusal: a torn
/// segment does not open until `fmine repair` has run, so a crash never drops an acknowledged transition silently.
#[derive(Debug)]
pub struct DurableObligationRegistry {
    writer: SemanticSegmentWriter,
    registry: ObligationRegistry,
}

impl DurableObligationRegistry {
    /// Create a new journal segment at `path` for ledger cell `cell_id`.
    pub fn create(path: &Path, cell_id: impl Into<String>) -> Result<Self, ObligationError> {
        let writer =
            SemanticSegmentWriter::create(path, cell_id).map_err(ObligationError::Store)?;
        Ok(Self {
            writer,
            registry: ObligationRegistry::new(),
        })
    }

    /// Reopen the journal at `path` and replay it (`PO-BED-5` replay).
    pub fn open(path: &Path) -> Result<Self, ObligationError> {
        let writer = SemanticSegmentWriter::open_append(path).map_err(ObligationError::Store)?;
        let snapshot = writer.snapshot().map_err(ObligationError::Store)?;
        let journal = |detail: String| ObligationError::Journal { detail };
        let mut events = Vec::with_capacity(snapshot.live_keys.len());
        for (index, (key, version)) in snapshot.live_keys.iter().enumerate() {
            if *key != event_key(index) {
                return Err(journal(format!("key '{key}' where event {index} belongs")));
            }
            if version.kind != DeltaKind::Put {
                return Err(journal(format!("event '{key}' was superseded")));
            }
            events
                .push(decode_event(&version.payload).map_err(|e| journal(format!("{key}: {e}")))?);
        }
        let registry = ObligationRegistry::replay(&events)
            .map_err(|e| journal(format!("replay refused: {e}")))?;
        let open: BTreeSet<String> = registry
            .open_obligations()
            .iter()
            .map(|rec| obligation_key(&rec.obligation_id))
            .collect();
        if open != snapshot.open_obligations {
            return Err(journal(format!(
                "the ledger holds open {:?}, the journal {open:?}",
                snapshot.open_obligations
            )));
        }
        Ok(Self { writer, registry })
    }

    /// The replayed registry.
    #[must_use]
    pub const fn registry(&self) -> &ObligationRegistry {
        &self.registry
    }

    /// See [`ObligationRegistry::check_region_close`].
    pub fn check_region_close(&self, region: RegionKind) -> Result<(), ObligationError> {
        self.registry.check_region_close(region)
    }

    /// Open an obligation and persist it; see [`ObligationRegistry::open`].
    pub fn open_obligation(
        &mut self,
        obligation_id: impl Into<String>,
        kind: ObligationKind,
        owner_region: RegionKind,
        at_ns: i64,
    ) -> Result<(), ObligationError> {
        let obligation_id = obligation_id.into();
        self.registry
            .open(obligation_id.clone(), kind, owner_region, at_ns)?;
        self.persist_last(DeltaKind::ReserveObligation, &obligation_id)
    }

    /// Resolve an obligation and persist it; see [`ObligationRegistry::resolve`].
    pub fn resolve(
        &mut self,
        obligation_id: &str,
        at_ns: i64,
        note: impl Into<String>,
    ) -> Result<(), ObligationError> {
        self.registry.resolve(obligation_id, at_ns, note)?;
        self.persist_last(DeltaKind::ResolveObligation, obligation_id)
    }

    /// Abort an obligation and persist it; see [`ObligationRegistry::abort`].
    pub fn abort(
        &mut self,
        obligation_id: &str,
        at_ns: i64,
        reason: impl Into<String>,
    ) -> Result<(), ObligationError> {
        self.registry.abort(obligation_id, at_ns, reason)?;
        self.persist_last(DeltaKind::ResolveObligation, obligation_id)
    }

    fn persist_last(
        &mut self,
        kind: DeltaKind,
        obligation_id: &str,
    ) -> Result<(), ObligationError> {
        let appended = self.append_last(kind, obligation_id);
        if appended.is_err() {
            self.registry.undo_last();
        }
        appended
    }

    fn append_last(&mut self, kind: DeltaKind, obligation_id: &str) -> Result<(), ObligationError> {
        let index =
            self.registry
                .journal
                .len()
                .checked_sub(1)
                .ok_or_else(|| ObligationError::Journal {
                    detail: "no transition to persist".to_string(),
                })?;
        let cell_id = self.writer.cell_id().to_string();
        let state = self.writer.ledger();
        let seq = state
            .committed_count(&cell_id)
            .map_err(ObligationError::Store)?;
        let prev_hash = state.head_hash(&cell_id).map_err(ObligationError::Store)?;
        let mut deltas = vec![
            DeltaOp::new(
                event_key(index),
                DeltaKind::Put,
                encode_event(&self.registry.journal[index]),
            ),
            DeltaOp::new(obligation_key(obligation_id), kind, Vec::new()),
        ];
        deltas.sort();
        let batch = DeltaBatch {
            format_version: FMT_LEDGER_V2,
            cell_id: cell_id.clone(),
            epoch: JOURNAL_EPOCH,
            seq,
            prev_hash,
            anchor: LedgerAnchor {
                format_version: FMT_ANCHOR_V1,
                cell_id,
                ledger_epoch: JOURNAL_EPOCH,
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
        };
        self.writer
            .append(&batch)
            .map(|_| ())
            .map_err(ObligationError::Store)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CELL: &str = "CELL-OB(node-1)";

    fn scratch(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "fmine-runtime-obligations-{tag}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir.join("obligations.fmseg")
    }

    #[test]
    fn test_po_bed_5_durable_journal_replays_after_a_crash() {
        let path = scratch("replay");
        let mut reg = DurableObligationRegistry::create(&path, CELL).unwrap();
        // One obligation of every family, each owned by a different region.
        for (i, kind) in ObligationKind::ALL.into_iter().enumerate() {
            let at = 1_000 + i64::try_from(i).unwrap();
            reg.open_obligation(format!("{}-{i}", kind.code()), kind, REGIONS[i].kind, at)
                .unwrap();
        }
        reg.resolve("OB-SEAL-1", 2_000, "manifest published")
            .unwrap();
        reg.abort("OB-POST-2", 2_100, "reconciled as already_recorded")
            .unwrap();
        let before = reg.registry().clone();
        drop(reg); // The process stops here without closing anything.

        let mut reopened = DurableObligationRegistry::open(&path).unwrap();
        assert_eq!(reopened.registry(), &before);
        assert_eq!(reopened.registry().open_obligations().len(), 6);
        // The region that owns the open OB-SYNC-ACK still refuses to close after the crash.
        assert_eq!(
            reopened.check_region_close(REGIONS[0].kind),
            Err(ObligationError::RegionLeak {
                region: REGIONS[0].kind,
                open_obligation_ids: vec!["OB-SYNC-ACK-0".to_string()],
            })
        );
        reopened.check_region_close(REGIONS[1].kind).unwrap();
        reopened
            .resolve("OB-SYNC-ACK-0", 3_000, "enterprise ack (cell, 7, mmr_root)")
            .unwrap();
        reopened.check_region_close(REGIONS[0].kind).unwrap();
        let after = reopened.registry().clone();
        drop(reopened);

        let again = DurableObligationRegistry::open(&path).unwrap();
        assert_eq!(again.registry(), &after);
        assert_eq!(again.registry().journal().len(), 11);
        assert_eq!(
            again.registry().get("OB-SYNC-ACK-0").unwrap().state,
            ObligationState::Resolved
        );
    }

    #[test]
    fn a_refused_transition_is_neither_written_nor_held() {
        let path = scratch("refuse");
        let mut reg = DurableObligationRegistry::create(&path, CELL).unwrap();
        reg.open_obligation(
            "ob-lab-1",
            ObligationKind::ObLab,
            RegionKind::ObjectStore,
            10,
        )
        .unwrap();
        let bytes = std::fs::read(&path).unwrap();
        let held = reg.registry().clone();

        assert!(matches!(
            reg.open_obligation(
                "ob-lab-1",
                ObligationKind::ObLab,
                RegionKind::ObjectStore,
                11
            ),
            Err(ObligationError::DuplicateId { .. })
        ));
        assert!(matches!(
            reg.resolve("ob-lab-2", 20, "received"),
            Err(ObligationError::UnknownId { .. })
        ));
        assert!(matches!(
            reg.resolve("ob-lab-1", 5, "received"),
            Err(ObligationError::TimeWentBack { .. })
        ));
        // The ledger refuses an id too long for its key; the registry takes the transition back.
        let long = "x".repeat(fmine_ledger::batch::MAX_KEY_BYTES);
        assert!(matches!(
            reg.open_obligation(
                long.clone(),
                ObligationKind::ObLab,
                RegionKind::ObjectStore,
                30
            ),
            Err(ObligationError::Store(_))
        ));
        assert!(reg.registry().get(&long).is_none());
        assert_eq!(reg.registry(), &held);
        assert_eq!(std::fs::read(&path).unwrap(), bytes);

        // The handle still writes after a refusal, and the file replays to what it holds.
        reg.resolve("ob-lab-1", 40, "assay received").unwrap();
        let resolved = reg.registry().clone();
        drop(reg);
        assert_eq!(
            DurableObligationRegistry::open(&path).unwrap().registry(),
            &resolved
        );
    }

    #[test]
    fn a_torn_journal_does_not_open() {
        let path = scratch("torn");
        let mut reg = DurableObligationRegistry::create(&path, CELL).unwrap();
        reg.open_obligation(
            "ob-tarp-1",
            ObligationKind::ObTarp,
            RegionKind::ObjectStore,
            10,
        )
        .unwrap();
        reg.resolve("ob-tarp-1", 20, "acknowledged and escalated")
            .unwrap();
        drop(reg);
        // The last record cut short, as a crash during its append leaves it.
        let bytes = std::fs::read(&path).unwrap();
        let torn = &bytes[..bytes.len() - 5];
        std::fs::write(&path, torn).unwrap();
        let err = DurableObligationRegistry::open(&path).unwrap_err();
        assert!(matches!(err, ObligationError::Store(_)), "{err}");
        assert!(err.to_string().contains("torn tail"), "{err}");
        assert_eq!(std::fs::read(&path).unwrap(), torn);
    }

    #[test]
    fn every_family_region_and_event_round_trips() {
        for kind in ObligationKind::ALL {
            assert_eq!(ObligationKind::from_code(kind.code()), Some(kind));
        }
        assert_eq!(ObligationKind::from_code("OB-SYNC"), None);
        for spec in REGIONS {
            assert_eq!(region_from_name(&region_name(spec.kind)), Some(spec.kind));
        }
        let events = [
            ObligationJournalEvent::Opened {
                obligation_id: "ob-commit-1".to_string(),
                kind: ObligationKind::ObCommit,
                owner_region: RegionKind::FinanceBridge,
                at_ns: -5,
            },
            ObligationJournalEvent::Resolved {
                obligation_id: "ob-commit-1".to_string(),
                at_ns: i64::MAX,
                note: "attested: 12 \u{b5}g/m\u{b3}".to_string(),
            },
            ObligationJournalEvent::Aborted {
                obligation_id: "ob-commit-2".to_string(),
                at_ns: 0,
                reason: String::new(),
            },
        ];
        for event in &events {
            assert_eq!(decode_event(&encode_event(event)).unwrap(), *event);
        }
        let mut flipped = encode_event(&events[0]);
        flipped[16] ^= 1;
        assert!(decode_event(&flipped).is_err());
    }

    #[test]
    fn test_po_bed_5_obligation_registry_leak_guard_and_crash_replay() {
        let mut reg = ObligationRegistry::new();
        reg.open(
            "ob-seal-001",
            ObligationKind::ObSeal,
            RegionKind::ObjectStore,
            1_000,
        )
        .unwrap();
        reg.open(
            "ob-post-001",
            ObligationKind::ObPost,
            RegionKind::FinanceBridge,
            1_500,
        )
        .unwrap();

        // Closing ObjectStore while ob-seal-001 is still open refuses with PO-BED-5 RegionLeak!
        let leak = reg.check_region_close(RegionKind::ObjectStore).unwrap_err();
        assert_eq!(
            leak,
            ObligationError::RegionLeak {
                region: RegionKind::ObjectStore,
                open_obligation_ids: vec!["ob-seal-001".to_string()],
            }
        );

        // Resolving ob-seal-001 allows ObjectStore to close cleanly; double-resolve refused.
        reg.resolve("ob-seal-001", 2_000, "manifest published")
            .unwrap();
        reg.check_region_close(RegionKind::ObjectStore).unwrap();
        assert!(matches!(
            reg.resolve("ob-seal-001", 2_500, "dup"),
            Err(ObligationError::AlreadyClosed { .. })
        ));

        // Crash & replay from journal preserves exact open/resolved state (ob-post-001 still pending recovery!).
        let replayed = ObligationRegistry::replay(reg.journal()).unwrap();
        assert_eq!(replayed, reg);
        assert_eq!(replayed.open_obligations().len(), 1);
        assert_eq!(replayed.open_obligations()[0].obligation_id, "ob-post-001");

        // Explicit abort closes ob-post-001 cleanly -> FinanceBridge may now close.
        let mut recovered = replayed;
        recovered
            .abort("ob-post-001", 3_000, "reconciled as already_recorded")
            .unwrap();
        recovered
            .check_region_close(RegionKind::FinanceBridge)
            .unwrap();
        assert!(recovered.open_obligations().is_empty());
    }
}
