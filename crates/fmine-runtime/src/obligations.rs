//! Determinism: BitwisePortable
//! Durable obligation registry (`OB-*`) and region-close leak guard (`BD-089`, `PO-BED-5`,
//! plan §26.6–§26.7).
//!
//! Tracks structured obligations (`OB-SEAL`, `OB-POST`, `OB-LAB`, `OB-COMMIT`, `OB-SYNC`) scoped
//! to owning regions. Every state transition (`Opened`, `Resolved`, `Aborted`) is recorded in an
//! append-only journal that replays deterministically after a crash. Closing a region while any
//! obligation owned by that region remains `Open` refuses with [`ObligationError::RegionLeak`]
//! (`PO-BED-5`: no silent obligation leak on region close).

use std::collections::BTreeMap;
use std::fmt;

use crate::region::RegionKind;

/// Registered Bedrock and domain obligation families (`BD-089`, plan §26.7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ObligationKind {
    /// CAS staging obligation: staged chunks must either root-publish or clean up (`OB-SEAL`).
    ObSeal,
    /// Finance bridge posting obligation: outbox intent must reconcile or abort (`OB-POST`).
    ObPost,
    /// Assay lab chain-of-custody obligation: dispatched -> received or declared lost (`OB-LAB`).
    ObLab,
    /// Environmental/social commitment obligation (`OB-COMMIT`).
    ObCommit,
    /// Cross-site sync transfer obligation (`OB-SYNC`).
    ObSync,
}

impl ObligationKind {
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::ObSeal => "OB-SEAL",
            Self::ObPost => "OB-POST",
            Self::ObLab => "OB-LAB",
            Self::ObCommit => "OB-COMMIT",
            Self::ObSync => "OB-SYNC",
        }
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
}

#[cfg(test)]
mod tests {
    use super::*;

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
