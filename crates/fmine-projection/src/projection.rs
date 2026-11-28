//! The projector framework (`BD-120`, plan §26.1): a [`Projector`] keeps a derived view of ledger cells, and a
//! [`Projection`] feeds it committed batches in order under per-cell high-water marks.
//!
//! - **Store and mark together.** [`Projector::apply`] writes a batch's effect and the cell's new
//!   [`HighWaterMark`] in one step, both or neither, and [`Projector::stored_marks`] reads the marks back from the
//!   projector's own store. [`Projection::open`] starts from those, so a projection that was stopped resumes at
//!   the batch after the last one its store holds, not at one an in-memory counter remembers.
//! - **In order, once.** [`Projection::feed`] applies batch `n` of a cell only when the mark says `n` batches are
//!   applied and the batch's `prev_hash` is the mark's head. An earlier batch is skipped
//!   ([`Fed::AlreadyApplied`]); the last applied batch must match the mark's digest. A later batch is a gap
//!   ([`ProjectionError::Gap`], `FMINE-REF-502`); a batch whose `prev_hash` is not the head is a fork
//!   ([`ProjectionError::ChainBreak`], `FMINE-REF-505`). Neither moves the mark.
//! - **Checked marks.** After an apply, the stored mark must be the new mark ([`ProjectionError::MarkNotRecorded`]).
//!   After a failed apply, the in-memory marks are reread from the store, which is what the projection holds.
//! - **Against the log.** [`Projection::status`] compares a cell's mark with an L0 ledger, and
//!   [`Projection::status_in`] with the digests of a segment ([`Status`]). Answers carry the marks they were read
//!   at ([`Projection::answer`]). A caller can require every cell to be current before reading
//!   ([`Projection::answer_current`]) or replay the authority into a fresh store ([`Projection::rebuild`]).
//!
//! A projection is derived and never consulted as truth: it can always be discarded and rebuilt from the log.

use std::collections::BTreeMap;
use std::fmt;

use fmine_core::hash::Digest32;
use fmine_evidence::{Refusal, RefusalCode};
use fmine_ledger::batch::{DeltaBatch, LedgerError};
use fmine_ledger::l0::{L0Error, L0Ledger};

use crate::mark::{Anchor, HighWaterMark, Stamped, Status};

/// A derived view of ledger cells, with its own store.
pub trait Projector {
    /// The store's error.
    type Error: fmt::Display;

    /// Apply `batch` and record `mark` as its cell's high-water mark, both or neither.
    ///
    /// # Errors
    /// The store's error; nothing of the batch and no mark may then be stored.
    fn apply(&mut self, batch: &DeltaBatch, mark: &HighWaterMark) -> Result<(), Self::Error>;

    /// The high-water marks the store holds, by cell, read from the store itself.
    ///
    /// # Errors
    /// The store's error.
    fn stored_marks(&self) -> Result<BTreeMap<String, HighWaterMark>, Self::Error>;
}

/// What [`Projection::feed`] did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fed {
    /// Applied; the cell's mark is now this.
    Applied(HighWaterMark),
    /// Already applied; nothing changed.
    AlreadyApplied,
}

/// A batch a projection cannot take, or a projector that failed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProjectionError {
    /// The batch is past the next one: earlier batches are missing (`FMINE-REF-502`).
    Gap {
        /// The cell.
        cell_id: String,
        /// The sequence the projection needs next.
        expected_seq: u64,
        /// The sequence offered.
        seq: u64,
    },
    /// The batch does not continue the projection's chain: a fork, or a projection that diverged (`FMINE-REF-505`).
    ChainBreak {
        /// The cell.
        cell_id: String,
        /// The batch's sequence.
        seq: u64,
        /// The digest the projection's mark names for that point of the chain.
        expected: Digest32,
        /// The digest the batch carries or has.
        actual: Digest32,
    },
    /// After an apply, the store does not hold the mark it was given (`FMINE-REF-505`).
    MarkNotRecorded {
        /// The cell.
        cell_id: String,
        /// The mark it was given.
        expected: HighWaterMark,
        /// The mark it holds.
        stored: HighWaterMark,
    },
    /// The saved mark names a different history or store state (`FMINE-REF-505`).
    Diverged {
        /// The affected cell.
        cell_id: String,
    },
    /// A freshness-required answer is missing authority batches (`FMINE-REF-502`).
    Behind {
        /// The affected cell.
        cell_id: String,
        /// How many batches remain to be applied.
        by: u64,
    },
    /// The projector's store failed (`FMINE-REF-506`).
    Store {
        /// Its error, as text.
        detail: String,
    },
    /// The batch cannot be digested (its own refusal code).
    Ledger(LedgerError),
    /// The ledger refused a read (its own refusal code).
    Source(L0Error),
}

impl ProjectionError {
    /// The refusal this error stands for.
    #[must_use]
    pub fn to_refusal(&self) -> Refusal {
        match self {
            Self::Gap { .. } | Self::Behind { .. } => {
                Refusal::new(RefusalCode::StaleAnchor502, self.to_string())
            }
            Self::ChainBreak { .. } | Self::MarkNotRecorded { .. } | Self::Diverged { .. } => {
                Refusal::new(RefusalCode::DigestMismatch505, self.to_string())
            }
            Self::Store { .. } => {
                Refusal::new(RefusalCode::InputMissingOrUnreadable506, self.to_string())
            }
            Self::Ledger(e) => e.to_refusal(),
            Self::Source(e) => e.to_refusal(),
        }
    }
}

impl fmt::Display for ProjectionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Gap {
                cell_id,
                expected_seq,
                seq,
            } => write!(
                f,
                "cell '{cell_id}': batch {seq} offered, batch {expected_seq} is next; feed the batches between"
            ),
            Self::ChainBreak {
                cell_id,
                seq,
                expected,
                actual,
            } => write!(
                f,
                "cell '{cell_id}': batch {seq} has {actual:?} where the projection's chain has {expected:?}; \
                 discard the projection and rebuild it from the ledger"
            ),
            Self::MarkNotRecorded {
                cell_id,
                expected,
                stored,
            } => write!(
                f,
                "cell '{cell_id}': the projector was given mark {expected:?} and stored {stored:?}"
            ),
            Self::Diverged { cell_id } => write!(
                f,
                "cell '{cell_id}': saved projection mark diverged; rebuild from the authority ledger"
            ),
            Self::Behind { cell_id, by } => write!(
                f,
                "cell '{cell_id}': projection is {by} batches behind; catch up to the authority ledger"
            ),
            Self::Store { detail } => write!(f, "projection store: {detail}"),
            Self::Ledger(e) => e.fmt(f),
            Self::Source(e) => e.fmt(f),
        }
    }
}

impl std::error::Error for ProjectionError {}

impl From<L0Error> for ProjectionError {
    fn from(e: L0Error) -> Self {
        Self::Source(e)
    }
}

impl From<LedgerError> for ProjectionError {
    fn from(e: LedgerError) -> Self {
        Self::Ledger(e)
    }
}

fn store_error(e: &impl fmt::Display) -> ProjectionError {
    ProjectionError::Store {
        detail: e.to_string(),
    }
}

/// A projector with the high-water marks its store holds.
#[derive(Debug)]
pub struct Projection<P> {
    projector: P,
    anchor: Anchor,
}

impl<P: Projector> Projection<P> {
    /// Start from the marks `projector`'s store holds.
    ///
    /// # Errors
    /// [`ProjectionError::Store`].
    pub fn open(projector: P) -> Result<Self, ProjectionError> {
        let marks = projector.stored_marks().map_err(|e| store_error(&e))?;
        Ok(Self {
            projector,
            anchor: Anchor { marks },
        })
    }

    /// The marks, one per cell applied.
    #[must_use]
    pub const fn anchor(&self) -> &Anchor {
        &self.anchor
    }

    /// The mark of `cell_id`.
    #[must_use]
    pub fn mark(&self, cell_id: &str) -> HighWaterMark {
        self.anchor.mark(cell_id)
    }

    /// The projector, to read from.
    #[must_use]
    pub const fn projector(&self) -> &P {
        &self.projector
    }

    /// The projector, for example to persist or discard it.
    #[must_use]
    pub fn into_projector(self) -> P {
        self.projector
    }

    /// Read the projector with the marks it was read at.
    pub fn answer<T>(&self, read: impl FnOnce(&P) -> T) -> Stamped<T> {
        Stamped {
            value: read(&self.projector),
            anchor: self.anchor.clone(),
        }
    }

    /// Read only after checking the stored anchor against every cell of `ledger`.
    /// The store may still refuse an individual index read. The caller must
    /// hold the same single-writer ownership as for `feed`.
    ///
    /// # Errors
    /// The read closure does not run if marks cannot be read, differ from the
    /// in-memory anchor, are behind, or name a cell or history outside `ledger`.
    pub fn answer_current<T>(
        &self,
        ledger: &L0Ledger,
        read: impl FnOnce(&P) -> T,
    ) -> Result<Stamped<T>, ProjectionError> {
        let marks = self.projector.stored_marks().map_err(|e| store_error(&e))?;
        for cell_id in marks.keys().chain(self.anchor.marks.keys()) {
            if marks.get(cell_id) != self.anchor.marks.get(cell_id)
                || ledger.committed_count(cell_id).is_err()
            {
                return Err(ProjectionError::Diverged {
                    cell_id: cell_id.clone(),
                });
            }
        }
        for cell_id in ledger.cell_ids() {
            match self.status(ledger, cell_id)? {
                Status::Current => {}
                Status::Behind { by } => {
                    return Err(ProjectionError::Behind {
                        cell_id: cell_id.to_string(),
                        by,
                    });
                }
                Status::Diverged => {
                    return Err(ProjectionError::Diverged {
                        cell_id: cell_id.to_string(),
                    });
                }
            }
        }
        Ok(self.answer(read))
    }

    /// Replay every authority cell into a newly initialized, empty projector.
    /// The caller owns the replacement and is responsible for supplying an
    /// empty store. The old projection can remain available for inspection.
    ///
    /// # Errors
    /// Refuses a replacement with existing marks. A replay or validation
    /// failure returns no replacement and may leave incomplete staging data.
    pub fn rebuild(ledger: &L0Ledger, empty: P) -> Result<Self, ProjectionError> {
        let mut replacement = Self::open(empty)?;
        if let Some(cell_id) = replacement.anchor.marks.keys().next() {
            return Err(ProjectionError::Store {
                detail: format!("replacement must be new and empty; found a mark for '{cell_id}'"),
            });
        }
        for cell_id in ledger.cell_ids() {
            replacement.catch_up(ledger.batches(cell_id)?)?;
        }
        replacement.answer_current(ledger, |_| ())?;
        Ok(replacement)
    }

    /// Apply `batch` if it is the next one of its cell; see the module documentation.
    ///
    /// # Errors
    /// [`ProjectionError::Gap`], [`ProjectionError::ChainBreak`], [`ProjectionError::MarkNotRecorded`],
    /// [`ProjectionError::Store`] or [`ProjectionError::Ledger`]. The cell's mark moves only on success.
    pub fn feed(&mut self, batch: &DeltaBatch) -> Result<Fed, ProjectionError> {
        let cell_id = batch.cell_id.as_str();
        let mark = self.mark(cell_id);
        let digest = batch.batch_digest()?;
        if batch.seq < mark.applied {
            if batch.seq + 1 == mark.applied && digest != mark.head {
                return Err(ProjectionError::ChainBreak {
                    cell_id: cell_id.to_string(),
                    seq: batch.seq,
                    expected: mark.head,
                    actual: digest,
                });
            }
            return Ok(Fed::AlreadyApplied);
        }
        if batch.seq > mark.applied {
            return Err(ProjectionError::Gap {
                cell_id: cell_id.to_string(),
                expected_seq: mark.applied,
                seq: batch.seq,
            });
        }
        if batch.prev_hash != mark.head {
            return Err(ProjectionError::ChainBreak {
                cell_id: cell_id.to_string(),
                seq: batch.seq,
                expected: mark.head,
                actual: batch.prev_hash,
            });
        }
        let Some(applied) = mark.applied.checked_add(1) else {
            return Err(ProjectionError::Store {
                detail: format!(
                    "cell '{cell_id}': batch sequence {} is the last there can be",
                    batch.seq
                ),
            });
        };
        let next = HighWaterMark {
            applied,
            head: digest,
        };
        if let Err(e) = self.projector.apply(batch, &next) {
            // The store is what the projection holds; follow it, whatever the failed apply left.
            if let Ok(marks) = self.projector.stored_marks() {
                self.anchor.marks = marks;
            }
            return Err(store_error(&e));
        }
        let marks = self.projector.stored_marks().map_err(|e| store_error(&e))?;
        let stored = marks.get(cell_id).copied().unwrap_or_default();
        self.anchor.marks = marks;
        if stored != next {
            return Err(ProjectionError::MarkNotRecorded {
                cell_id: cell_id.to_string(),
                expected: next,
                stored,
            });
        }
        Ok(Fed::Applied(next))
    }

    /// Feed `batches` in order, skipping those already applied. Returns how many were applied.
    ///
    /// # Errors
    /// The first [`Projection::feed`] error; the batches before it stay applied.
    pub fn catch_up<'a>(
        &mut self,
        batches: impl IntoIterator<Item = &'a DeltaBatch>,
    ) -> Result<u64, ProjectionError> {
        let mut applied = 0;
        for batch in batches {
            if let Fed::Applied(_) = self.feed(batch)? {
                applied += 1;
            }
        }
        Ok(applied)
    }

    /// Catch `cell_id` up with `ledger`.
    ///
    /// # Errors
    /// [`ProjectionError::Source`] for an unknown cell, [`ProjectionError::Diverged`]
    /// for a non-prefix mark or disagreement with the persisted mark, then as [`Projection::catch_up`].
    pub fn catch_up_with(
        &mut self,
        ledger: &L0Ledger,
        cell_id: &str,
    ) -> Result<u64, ProjectionError> {
        if self.status(ledger, cell_id)? == Status::Diverged
            || self
                .projector
                .stored_marks()
                .map_err(|e| store_error(&e))?
                .get(cell_id)
                != self.anchor.marks.get(cell_id)
        {
            return Err(ProjectionError::Diverged {
                cell_id: cell_id.to_string(),
            });
        }
        let batches = ledger.batches(cell_id)?;
        let from = usize::try_from(self.mark(cell_id).applied)
            .ok()
            .and_then(|index| batches.get(index..))
            .ok_or_else(|| ProjectionError::Diverged {
                cell_id: cell_id.to_string(),
            })?;
        self.catch_up(from)
    }

    /// Where `cell_id`'s mark stands against `ledger`.
    ///
    /// # Errors
    /// [`ProjectionError::Source`] for an unknown cell.
    pub fn status(&self, ledger: &L0Ledger, cell_id: &str) -> Result<Status, ProjectionError> {
        let mark = self.mark(cell_id);
        let committed = ledger.committed_count(cell_id)?;
        if mark.applied > committed {
            return Ok(Status::Diverged);
        }
        if ledger.prev_hash_at(cell_id, mark.applied)? != mark.head {
            return Ok(Status::Diverged);
        }
        Ok(if mark.applied < committed {
            Status::Behind {
                by: committed - mark.applied,
            }
        } else {
            Status::Current
        })
    }

    /// Where `cell_id`'s mark stands against a log whose batch digests, in order, are `digests` (for example a
    /// segment's, from `SegmentReader::open`).
    #[must_use]
    pub fn status_in(&self, cell_id: &str, digests: &[Digest32]) -> Status {
        Status::of(self.mark(cell_id), digests)
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl Projection<crate::drill::DrillStore> {
    /// Rebuild into `drill.db` inside a newly created directory. A new
    /// directory isolates the replacement from any existing store or sidecar.
    ///
    /// # Errors
    /// An existing path, a creation or store failure, or an error during replay.
    pub fn rebuild_drill_in(
        ledger: &L0Ledger,
        directory: impl AsRef<std::path::Path>,
    ) -> Result<Self, ProjectionError> {
        let directory = directory.as_ref();
        std::fs::create_dir(directory).map_err(|e| ProjectionError::Store {
            detail: format!("create replacement directory {}: {e}", directory.display()),
        })?;
        let store = crate::drill::DrillStore::open(directory.join("drill.db"))
            .map_err(|e| store_error(&e))?;
        Self::rebuild(ledger, store)
    }
}
