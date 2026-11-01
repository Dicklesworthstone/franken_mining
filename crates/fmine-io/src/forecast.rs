//! DESIGN (BD-047): local append-only forecast storage over the S0 `FJR1` codec.
//! A writer holds an exclusive OS lock through replay, expected-head comparison, clock observation,
//! semantic validation, append and sync. Readers retain a shared lock. Only synced state is published.
//! The operation schema belongs to the caller; this store does not implement a second forecast ledger.
//! Local clocks, advisory locks and hash chains establish neither external time proof nor authenticity.

pub mod prefix;
pub mod timestamp;

use std::fs::{File, OpenOptions, TryLockError};
use std::io::{Read, Write};
use std::path::Path;

use fmine_claim::forecast_journal::{self, Entry, Header, Record, decode_journal};
use fmine_core::hash::Digest32;
use fmine_evidence::{Refusal, RefusalCode};
use fmine_runtime::WallClock;

pub const MAX_JOURNAL_BYTES: u64 = forecast_journal::MAX_JOURNAL_BYTES as u64;
pub const MAX_JOURNAL_RECORDS: usize = forecast_journal::MAX_RECORDS;
pub const MAX_REQUEST_BYTES: usize = forecast_journal::MAX_OPERATION_BYTES;

/// The committed snapshot and its OS lock. Dropping the handle releases the lock.
#[derive(Debug)]
pub struct Journal {
    file: File,
    header: Header,
    records: Vec<Record>,
    head: Digest32,
    byte_len: u64,
    writable: bool,
    pinned: bool,
    poisoned: bool,
}

impl Journal {
    /// Create a new private journal. The parent directory must already exist. The clock is sampled
    /// only after the new file is exclusively locked. A failure leaves any created bytes for inspection.
    pub fn create(
        path: &Path,
        cell_id: impl Into<String>,
        clock_skew_ns: u64,
        clock: &dyn WallClock,
    ) -> Result<Self, Refusal> {
        let header = Header {
            cell_id: cell_id.into(),
            clock_skew_ns,
        };
        header.validate()?;
        let mut options = OpenOptions::new();
        options.read(true).append(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt as _;
            options.mode(0o600);
        }
        let mut file = options
            .open(path)
            .map_err(|e| io_refusal("create", path, e))?;
        acquire(&file, true)?;
        let observed_ns = read_clock(clock)?;
        header.time_bounds(observed_ns)?;
        let record = Record {
            sequence: 0,
            previous: Digest32([0; 32]),
            observed_ns,
            entry: Entry::Genesis(header.clone()),
        };
        let line = record.to_line()?;
        file.write_all(&line)
            .map_err(|e| io_refusal("write genesis", path, e))?;
        file.sync_all()
            .map_err(|e| io_refusal("sync genesis", path, e))?;
        #[cfg(unix)]
        {
            let parent = path
                .parent()
                .filter(|p| !p.as_os_str().is_empty())
                .unwrap_or_else(|| Path::new("."));
            File::open(parent)
                .and_then(|dir| dir.sync_all())
                .map_err(|e| io_refusal("sync parent directory", parent, e))?;
        }
        Ok(Self {
            file,
            header,
            head: record.chain_head()?,
            records: vec![record],
            byte_len: line.len() as u64,
            writable: true,
            pinned: false,
            poisoned: false,
        })
    }

    /// Replay under a retained shared lock. `None` permits inspection but cannot detect prefix rollback.
    /// A supplied pin must equal the final head, not merely an earlier record's head.
    pub fn open_read(path: &Path, pinned_head: Option<Digest32>) -> Result<Self, Refusal> {
        Self::open(path, false, pinned_head)
    }

    /// Compare the independently retained final head while holding an exclusive lock through appends.
    pub fn open_write(path: &Path, expected_head: Digest32) -> Result<Self, Refusal> {
        Self::open(path, true, Some(expected_head))
    }

    fn open(path: &Path, writable: bool, pinned_head: Option<Digest32>) -> Result<Self, Refusal> {
        let before =
            std::fs::symlink_metadata(path).map_err(|e| io_refusal("read metadata", path, e))?;
        regular(&before)?;
        let mut file = OpenOptions::new()
            .read(true)
            .append(writable)
            .open(path)
            .map_err(|e| io_refusal("open", path, e))?;
        acquire(&file, writable)?;
        let metadata = file
            .metadata()
            .map_err(|e| io_refusal("read opened metadata", path, e))?;
        regular(&metadata)?;
        let mut bytes = Vec::new();
        Read::by_ref(&mut file)
            .take(MAX_JOURNAL_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| io_refusal("read", path, e))?;
        let records = decode_journal(&bytes)?;
        let Some(Record {
            entry: Entry::Genesis(header),
            ..
        }) = records.first()
        else {
            return Err(integrity("validated journal lacks genesis"));
        };
        let head = records
            .last()
            .ok_or_else(|| integrity("empty journal"))?
            .chain_head()?;
        if pinned_head.is_some_and(|pin| pin != head) {
            return Err(integrity(format!(
                "expected final head differs from {head}"
            )));
        }
        Ok(Self {
            file,
            header: header.clone(),
            records,
            head,
            byte_len: bytes.len() as u64,
            writable,
            pinned: pinned_head.is_some(),
            poisoned: false,
        })
    }

    #[must_use]
    pub fn records(&self) -> &[Record] {
        &self.records
    }

    #[must_use]
    pub const fn header(&self) -> &Header {
        &self.header
    }

    #[must_use]
    pub const fn head(&self) -> Digest32 {
        self.head
    }

    /// True only while the current snapshot still equals the pin supplied at open.
    #[must_use]
    pub const fn is_pinned(&self) -> bool {
        self.pinned && !self.poisoned
    }

    /// Stamp and validate under the writer lock, append, sync, then publish. The callback should validate
    /// a staged semantic replay; it must not publish its own changes before this call succeeds.
    /// A validator refusal leaves both disk bytes and this snapshot unchanged. A write/sync failure
    /// poisons this handle and retains the prior snapshot; no retry, rollback, repair or truncation occurs.
    pub fn append(
        &mut self,
        request: &str,
        clock: &dyn WallClock,
        validate: impl FnOnce(&Record, &Header) -> Result<(), Refusal>,
    ) -> Result<Record, Refusal> {
        if !self.writable || self.poisoned {
            return Err(integrity("append requires a healthy exclusive writer"));
        }
        if self.records.len() >= MAX_JOURNAL_RECORDS {
            return Err(integrity("record bound reached"));
        }
        if request.is_empty() || request.len() > MAX_REQUEST_BYTES {
            return Err(Refusal::new(
                RefusalCode::UndeclaredUnitBasisOrTz107,
                "forecast journal: operation must contain 1..=65536 UTF-8 bytes",
            ));
        }
        if self
            .file
            .metadata()
            .map_err(|e| integrity(format!("cannot verify append size: {e}")))?
            .len()
            != self.byte_len
        {
            self.poisoned = true;
            return Err(integrity("journal length changed outside this writer"));
        }
        let observed_ns = read_clock(clock)?;
        self.header.time_bounds(observed_ns)?;
        let last = self
            .records
            .last()
            .ok_or_else(|| integrity("missing committed record"))?;
        if observed_ns < last.observed_ns {
            return Err(integrity("observed wall time regressed"));
        }
        let record = Record {
            sequence: last
                .sequence
                .checked_add(1)
                .ok_or_else(|| integrity("sequence overflow"))?,
            previous: self.head,
            observed_ns,
            entry: Entry::Operation(request.to_owned()),
        };
        let line = record.to_line()?;
        let next_len = self
            .byte_len
            .checked_add(line.len() as u64)
            .filter(|n| *n <= MAX_JOURNAL_BYTES)
            .ok_or_else(|| integrity("byte bound reached"))?;
        let head = record.chain_head()?;
        validate(&record, &self.header)?;
        self.poisoned = true;
        self.file.write_all(&line).map_err(|e| {
            integrity(format!(
                "append failed; retained bytes require inspection: {e}"
            ))
        })?;
        self.file.sync_all().map_err(|e| {
            integrity(format!(
                "sync failed; retained bytes require inspection: {e}"
            ))
        })?;
        self.records.push(record.clone());
        self.head = head;
        self.byte_len = next_len;
        self.pinned = false;
        self.poisoned = false;
        Ok(record)
    }
}

fn regular(metadata: &std::fs::Metadata) -> Result<(), Refusal> {
    if !metadata.is_file() || metadata.len() > MAX_JOURNAL_BYTES {
        return Err(integrity(
            "journal must be a bounded regular non-symlink file",
        ));
    }
    Ok(())
}

fn acquire(file: &File, exclusive: bool) -> Result<(), Refusal> {
    let result = if exclusive {
        file.try_lock()
    } else {
        file.try_lock_shared()
    };
    result.map_err(|error| match error {
        TryLockError::WouldBlock => integrity("journal is locked; retry with the current head"),
        TryLockError::Error(e) => integrity(format!("file locking unavailable: {e}")),
    })
}

fn read_clock(clock: &dyn WallClock) -> Result<i64, Refusal> {
    clock
        .now_ns()
        .map_err(|e| integrity(format!("wall clock unavailable: {e}")))
}

fn integrity(detail: impl std::fmt::Display) -> Refusal {
    Refusal::new(
        RefusalCode::StaleAnchor502,
        format!("forecast journal: {detail}"),
    )
}

fn io_refusal(operation: &str, path: &Path, error: std::io::Error) -> Refusal {
    Refusal::new(
        RefusalCode::InputMissingOrUnreadable506,
        format!(
            "forecast journal: {operation} at {}: {error}",
            path.display()
        ),
    )
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use fmine_runtime::ClockError;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    struct Clock(i64);
    impl WallClock for Clock {
        fn now_ns(&self) -> Result<i64, ClockError> {
            Ok(self.0)
        }
    }

    fn path(name: &str) -> PathBuf {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "fmine-forecast-{}-{nanos}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir.join(name)
    }

    #[test]
    fn restart_preserves_exact_operations_and_pinned_final_head() {
        let path = path("journal");
        let mut journal = Journal::create(&path, "cell-one", 5, &Clock(100)).unwrap();
        let initial = journal.head();
        let genesis_bytes = std::fs::read(&path).unwrap();
        let operation = journal
            .append("register = \"é\"\n", &Clock(101), |record, header| {
                assert_eq!(record.sequence, 1);
                assert_eq!(record.previous, initial);
                assert_eq!(record.time_bounds(header)?, (96, 106));
                Ok(())
            })
            .unwrap();
        assert!(std::fs::read(&path).unwrap().starts_with(&genesis_bytes));
        let head = journal.head();
        let records = journal.records().to_vec();
        assert_eq!(head, operation.chain_head().unwrap());
        drop(journal);
        let read = Journal::open_read(&path, Some(head)).unwrap();
        assert_eq!(read.records(), records);
        assert_eq!(read.header().cell_id, "cell-one");
        assert!(read.is_pinned());
        drop(read);
        assert!(Journal::open_read(&path, Some(initial)).is_err());
        assert!(Journal::open_write(&path, initial).is_err());
        let mut writer = Journal::open_write(&path, head).unwrap();
        assert_eq!(
            writer
                .append("next", &Clock(101), |_, _| Ok(()))
                .unwrap()
                .sequence,
            2
        );
        assert!(!writer.is_pinned());
    }

    #[test]
    fn held_locks_exclude_writers_and_create_never_replaces() {
        let path = path("journal");
        let writer = Journal::create(&path, "cell", 0, &Clock(1)).unwrap();
        let head = writer.head();
        assert!(Journal::open_read(&path, None).is_err());
        assert!(Journal::open_write(&path, head).is_err());
        assert!(Journal::create(&path, "other", 0, &Clock(2)).is_err());
        drop(writer);
        let mut reader = Journal::open_read(&path, Some(head)).unwrap();
        let second = Journal::open_read(&path, None).unwrap();
        assert!(
            reader
                .append("forbidden", &Clock(2), |_, _| Ok(()))
                .is_err()
        );
        assert!(Journal::open_write(&path, head).is_err());
        drop(reader);
        drop(second);
        assert!(Journal::open_write(&path, head).is_ok());
    }

    #[test]
    fn invalid_semantics_clock_and_bounds_do_not_change_committed_bytes() {
        let path = path("journal");
        let mut journal = Journal::create(&path, "cell", 3, &Clock(100)).unwrap();
        let bytes = std::fs::read(&path).unwrap();
        let head = journal.head();
        let refusal = Refusal::new(RefusalCode::LateRegistration206, "test: too late");
        assert_eq!(
            journal
                .append("late", &Clock(101), |_, _| Err(refusal.clone()))
                .unwrap_err(),
            refusal
        );
        assert!(
            journal
                .append("rollback", &Clock(99), |_, _| panic!(
                    "clock must refuse first"
                ))
                .is_err()
        );
        assert!(
            journal
                .append("overflow", &Clock(i64::MAX), |_, _| panic!(
                    "bounds must refuse first"
                ))
                .is_err()
        );
        assert!(
            journal
                .append(
                    &"x".repeat(MAX_REQUEST_BYTES + 1),
                    &Clock(101),
                    |_, _| panic!("size must refuse first")
                )
                .is_err()
        );
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
        assert_eq!(journal.head(), head);
        assert_eq!(journal.records().len(), 1);
        assert!(journal.append("valid", &Clock(101), |_, _| Ok(())).is_ok());
    }

    #[test]
    fn corruption_torn_tail_and_prefix_rollback_refuse_without_repair() {
        let path = path("journal");
        let mut journal = Journal::create(&path, "cell", 0, &Clock(1)).unwrap();
        let prefix = std::fs::read(&path).unwrap();
        journal.append("first", &Clock(2), |_, _| Ok(())).unwrap();
        let head = journal.head();
        let original = std::fs::read(&path).unwrap();
        drop(journal);
        let mut corrupt = original.clone();
        corrupt[20] = if corrupt[20] == b'0' { b'1' } else { b'0' };
        for bytes in [corrupt, original[..original.len() - 1].to_vec(), prefix] {
            std::fs::write(&path, &bytes).unwrap();
            assert!(Journal::open_read(&path, Some(head)).is_err());
            assert!(Journal::open_write(&path, head).is_err());
            assert_eq!(std::fs::read(&path).unwrap(), bytes);
        }
        // Without an independently retained pin a valid prefix can only be internally consistent.
        assert!(Journal::open_read(&path, None).is_ok());
    }

    #[test]
    fn append_failure_poisoning_keeps_the_last_committed_snapshot() {
        let path = path("journal");
        let mut journal = Journal::create(&path, "cell", 0, &Clock(1)).unwrap();
        let head = journal.head();
        let bytes = std::fs::read(&path).unwrap();
        // A real read-only descriptor exercises write failure without a test-only writer framework.
        journal.file = File::open(&path).unwrap();
        assert!(journal.append("first", &Clock(2), |_, _| Ok(())).is_err());
        assert!(journal.poisoned);
        assert_eq!(journal.head(), head);
        assert_eq!(journal.records().len(), 1);
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
        assert!(
            journal
                .append("retry", &Clock(3), |_, _| panic!(
                    "poison must refuse first"
                ))
                .is_err()
        );
    }

    #[test]
    fn special_and_oversized_files_are_not_journals() {
        let path = path("oversized");
        let file = File::create(&path).unwrap();
        file.set_len(MAX_JOURNAL_BYTES + 1).unwrap();
        assert!(Journal::open_read(&path, None).is_err());
        assert!(Journal::open_read(path.parent().unwrap(), None).is_err());
        #[cfg(unix)]
        {
            let link = path.with_extension("link");
            std::os::unix::fs::symlink(&path, &link).unwrap();
            assert!(Journal::open_read(&link, None).is_err());
        }
    }
}
