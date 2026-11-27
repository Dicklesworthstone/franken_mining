//! High-water marks and anchors (`BD-120`, plan §6.3 FMT-ANCHOR-1, §26.1).
//!
//! A projection's [`HighWaterMark`] for a cell says how many of the cell's batches it has applied and the digest
//! of the last one. It is the `prev_hash` the cell's next batch must carry, so a projection can tell a batch that
//! continues it from a gap or a fork, and can compare itself with the ledger by one digest
//! ([`fmine_ledger::l0::L0Ledger::prev_hash_at`]). An [`Anchor`] is the vector of those marks, one per cell; every
//! answer a projection gives names the anchor it was read at ([`Stamped`]).

use std::collections::BTreeMap;

use fmine_core::hash::Digest32;
use fmine_ledger::batch::GENESIS_PREV_HASH;

/// How far a projection has applied one cell.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct HighWaterMark {
    /// Batches applied: sequences `0..applied`.
    pub applied: u64,
    /// Digest of batch `applied - 1`; the genesis hash when nothing is applied.
    pub head: Digest32,
}

impl HighWaterMark {
    /// Nothing applied.
    pub const GENESIS: Self = Self {
        applied: 0,
        head: GENESIS_PREV_HASH,
    };
}

impl Default for HighWaterMark {
    fn default() -> Self {
        Self::GENESIS
    }
}

/// One high-water mark per cell. A cell with no entry is at [`HighWaterMark::GENESIS`].
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct Anchor {
    /// The marks, by cell id.
    pub marks: BTreeMap<String, HighWaterMark>,
}

impl Anchor {
    /// The mark of `cell_id`.
    #[must_use]
    pub fn mark(&self, cell_id: &str) -> HighWaterMark {
        self.marks.get(cell_id).copied().unwrap_or_default()
    }
}

/// An answer with the anchor it was read at (plan §26.11 F5: "answers state their high-water mark").
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Stamped<T> {
    /// The answer.
    pub value: T,
    /// The marks of the projection when it answered.
    pub anchor: Anchor,
}

/// Where a projection stands against the log it projects, for one cell.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    /// It has applied every committed batch.
    Current,
    /// It is a prefix of the log, `by` batches short.
    Behind {
        /// Committed batches not yet applied.
        by: u64,
    },
    /// Its mark is not on the log: it applied a batch the log does not hold, or more batches than the log has.
    /// It is discarded and rebuilt from the log (plan §26.11 F5).
    Diverged,
}

impl Status {
    /// The status of `mark` against a log whose batch digests, in sequence order, are `digests`.
    #[must_use]
    pub fn of(mark: HighWaterMark, digests: &[Digest32]) -> Self {
        let committed = u64::try_from(digests.len()).unwrap_or(u64::MAX);
        if mark.applied > committed {
            return Self::Diverged;
        }
        let expected = match mark.applied.checked_sub(1) {
            None => GENESIS_PREV_HASH,
            Some(last) => usize::try_from(last)
                .ok()
                .and_then(|i| digests.get(i).copied())
                .unwrap_or(GENESIS_PREV_HASH),
        };
        if expected != mark.head {
            Self::Diverged
        } else if mark.applied < committed {
            Self::Behind {
                by: committed - mark.applied,
            }
        } else {
            Self::Current
        }
    }
}
