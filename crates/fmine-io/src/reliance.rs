//! Local append-only reliance journal (BD-347).
//!
//! Each line is a lowercase hexadecimal S0 `JournalRecord` frame followed by LF. Its successor carries
//! BLAKE3 of that exact line. A truncated tail, noncanonical line, illegal transition or wrong head refuses;
//! this adapter never repairs, overwrites or truncates a journal. A shared reader lock must remain alive
//! through any gated export. A writer holds an exclusive lock through replay, expected-head comparison,
//! capability validation, append and `sync_all`.
//!
//! OS locks coordinate cooperating local processes; they are not signatures or protection against an actor
//! who can replace files. A consumer's independently trusted final-head pin is required for current reuse.
//! No clock is inferred here: every append takes the owner's explicit `TrustedTime` policy input.

use std::fs::{File, OpenOptions, TryLockError};
use std::io::{Read, Write};
use std::path::Path;

use fmine_claim::reliance::{
    Entry, Header, IssuanceRecord, JournalRecord, MAX_RECORD_BYTES, ReleaseCapability,
    RelianceLedger, RoleId, TrustContext, TrustedTime,
};
use fmine_core::hash::{Digest32, blake3_hash, sha256};
use fmine_evidence::{Refusal, RefusalCode};

/// Total on-disk journal bound, checked before and during reads and before each append.
pub const MAX_JOURNAL_BYTES: u64 = 64 * 1024 * 1024;
/// Bound on replay work, independent of the byte limit.
pub const MAX_JOURNAL_RECORDS: usize = 100_000;
/// Exact issued-artifact bytes retained in memory for one verified write or current-use copy.
pub const MAX_ARTIFACT_BYTES: u64 = 64 * 1024 * 1024;

/// Register a new generic recipient while retaining one exact source issuance's producer provenance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReissueRequest {
    /// `IssuanceRecord::digest()`, not the output-byte digest or journal head.
    pub source_issuance: Digest32,
    pub recipient_role: RoleId,
    pub purpose: String,
}

/// A local bearer capability loaded from an exact 32-byte private file. Debug never exposes its bytes.
pub struct ReleaseKey([u8; 32]);

impl std::fmt::Debug for ReleaseKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ReleaseKey([REDACTED])")
    }
}

impl ReleaseKey {
    /// Read a regular, non-symlink key file with no Unix group/other permissions. No text/hex fallback.
    /// Platforms whose private-file permissions this adapter cannot verify refuse this operation.
    pub fn read(path: &Path) -> Result<Self, Refusal> {
        let metadata = std::fs::symlink_metadata(path)
            .map_err(|e| io_refusal("read release-key metadata", path, &e))?;
        private_key_file(&metadata)?;
        let mut file = File::open(path).map_err(|e| io_refusal("open release key", path, &e))?;
        let opened = file
            .metadata()
            .map_err(|e| io_refusal("read opened release-key metadata", path, &e))?;
        private_key_file(&opened)?;
        let mut bytes = [0_u8; 33];
        let mut used = 0;
        while used < bytes.len() {
            let count = file
                .read(&mut bytes[used..])
                .map_err(|e| io_refusal("read release key", path, &e))?;
            if count == 0 {
                break;
            }
            used += count;
        }
        if used != 32 {
            return Err(authority(
                "release key must contain exactly thirty-two raw bytes",
            ));
        }
        let mut key = [0_u8; 32];
        key.copy_from_slice(&bytes[..32]);
        if key == [0; 32] {
            return Err(authority("release key must not be all zero bytes"));
        }
        Ok(Self(key))
    }

    /// Public commitment for a genesis header; the secret itself has no accessor.
    #[must_use]
    pub fn sha256(&self) -> Digest32 {
        sha256(&self.0)
    }
}

fn private_key_file(metadata: &std::fs::Metadata) -> Result<(), Refusal> {
    if !metadata.is_file() || metadata.len() != 32 {
        return Err(authority("release key must be an exact-size regular file"));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        if metadata.permissions().mode() & 0o077 != 0 {
            return Err(authority("release-key file permits group or other access"));
        }
        Ok(())
    }
    #[cfg(not(unix))]
    {
        Err(authority(
            "private release-key permissions are not supported on this platform",
        ))
    }
}

/// A fully replayed journal whose shared or exclusive OS lock remains held until this handle is dropped.
/// The ledger is immutable to callers. No method promotes an unpinned read to consumer trust.
#[derive(Debug)]
pub struct Journal {
    file: File,
    ledger: RelianceLedger,
    byte_len: u64,
    writable: bool,
    pinned: bool,
    poisoned: bool,
}

struct PreparedAppend {
    ledger: RelianceLedger,
    record: JournalRecord,
    line: Vec<u8>,
    byte_len: u64,
}

impl Journal {
    /// Create a new journal containing exactly one validated genesis record. An existing path is refused.
    /// The parent directory must already exist; the caller chooses its trust and access policy.
    pub fn create(path: &Path, header: Header) -> Result<Self, Refusal> {
        let record = JournalRecord {
            previous: Digest32([0; 32]),
            entry: Entry::Genesis(header),
        };
        let ledger = RelianceLedger::from_records(std::slice::from_ref(&record))?;
        let line = encode_line(&record)?;
        let mut options = OpenOptions::new();
        options.read(true).append(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt as _;
            options.mode(0o600);
        }
        let mut file = options
            .open(path)
            .map_err(|e| io_refusal("create reliance journal", path, &e))?;
        acquire(&file, true)?;
        file.write_all(&line)
            .map_err(|e| io_refusal("write reliance genesis", path, &e))?;
        file.sync_all()
            .map_err(|e| io_refusal("sync reliance genesis", path, &e))?;
        #[cfg(unix)]
        {
            let parent = path
                .parent()
                .filter(|p| !p.as_os_str().is_empty())
                .unwrap_or_else(|| Path::new("."));
            File::open(parent)
                .and_then(|dir| dir.sync_all())
                .map_err(|e| io_refusal("sync reliance parent directory", parent, &e))?;
        }
        Ok(Self {
            file,
            ledger,
            byte_len: line.len() as u64,
            writable: true,
            pinned: false,
            poisoned: false,
        })
    }

    /// Replay under a shared lock. A supplied pin must equal the final head, never merely an ancestor.
    /// `None` allows historical inspection/recall only; it supplies no trust for current reuse.
    pub fn open_read(path: &Path, pinned_head: Option<Digest32>) -> Result<Self, Refusal> {
        Self::open(path, false, pinned_head)
    }

    /// Replay and compare the expected final head while holding one exclusive lock through later appends.
    pub fn open_write(path: &Path, expected_head: Digest32) -> Result<Self, Refusal> {
        Self::open(path, true, Some(expected_head))
    }

    fn open(path: &Path, writable: bool, pin: Option<Digest32>) -> Result<Self, Refusal> {
        let metadata = std::fs::symlink_metadata(path)
            .map_err(|e| io_refusal("read reliance-journal metadata", path, &e))?;
        regular_journal(&metadata)?;
        let mut file = OpenOptions::new()
            .read(true)
            .append(writable)
            .open(path)
            .map_err(|e| io_refusal("open reliance journal", path, &e))?;
        acquire(&file, writable)?;
        let opened = file
            .metadata()
            .map_err(|e| io_refusal("read opened reliance-journal metadata", path, &e))?;
        regular_journal(&opened)?;
        let mut bytes = Vec::new();
        Read::by_ref(&mut file)
            .take(MAX_JOURNAL_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| io_refusal("read reliance journal", path, &e))?;
        let ledger = decode_journal(&bytes)?;
        if let Some(expected) = pin
            && expected != ledger.head()
        {
            return Err(integrity(format!(
                "reliance final head differs: expected {expected}, found {}",
                ledger.head()
            )));
        }
        Ok(Self {
            file,
            ledger,
            byte_len: bytes.len() as u64,
            writable,
            pinned: pin.is_some(),
            poisoned: false,
        })
    }

    /// The fully replayed committed state. Keep this journal handle alive through any gated operation.
    #[must_use]
    pub const fn ledger(&self) -> &RelianceLedger {
        &self.ledger
    }

    /// BLAKE3 of the final canonical hex line, including LF.
    #[must_use]
    pub fn head(&self) -> Digest32 {
        self.ledger.head()
    }

    /// Whether this read matched an independently supplied exact final-head pin.
    #[must_use]
    pub const fn is_pinned(&self) -> bool {
        self.pinned && !self.poisoned
    }

    /// Validate an issuance before the caller creates its output file, while retaining the writer lock.
    /// This writes nothing. A later append failure leaves that output unissued and unusable as current.
    pub fn validate_append(
        &self,
        entry: &Entry,
        key: &ReleaseKey,
        time: TrustedTime,
    ) -> Result<(), Refusal> {
        self.prepare_append(entry.clone(), key, time).map(|_| ())
    }

    /// Create the exact artifact and durably record its issuance under this writer's exclusive lock.
    /// DESIGN: the caller derives Claims, anchor, inputs and build identity from its verified report;
    /// this boundary verifies the supplied output digest against the actual bytes before any write.
    /// A failed artifact write leaves the journal unchanged. A failed journal append leaves the artifact
    /// unissued; it must not be distributed as current, and no automatic deletion or retry is attempted.
    pub fn issue_bytes(
        &mut self,
        output_path: &Path,
        bytes: &[u8],
        issuance: IssuanceRecord,
        key: &ReleaseKey,
        time: TrustedTime,
    ) -> Result<JournalRecord, Refusal> {
        artifact_bound(bytes.len() as u64)?;
        if blake3_hash(bytes) != issuance.output_digest {
            return Err(Refusal::new(
                RefusalCode::DigestMismatch505,
                "issuance digest does not match the exact output bytes",
            ));
        }
        let next = self.prepare_append(Entry::Issued(issuance), key, time)?;
        write_artifact_new(output_path, bytes)?;
        self.commit_append(next)
    }

    /// Forward current bytes to a registered recipient under a pinned exclusive journal lock.
    /// DESIGN: select the exact source receipt and preserve all producer provenance. The current-use gate
    /// runs against the existing head before the new issuance can advance the status cut. Only recipient,
    /// purpose and issuance time change; this registers a local copy, not delivery or a new human sign-off.
    pub fn reissue_current(
        &mut self,
        source_path: &Path,
        destination_path: &Path,
        request: ReissueRequest,
        key: &ReleaseKey,
        time: TrustedTime,
    ) -> Result<JournalRecord, Refusal> {
        if !self.writable {
            return Err(authority(
                "registered forwarding requires an exclusive writer handle",
            ));
        }
        if self.poisoned {
            return Err(integrity(
                "reliance writer is poisoned after an incomplete append",
            ));
        }
        if !self.is_pinned() {
            return Err(Refusal::new(
                RefusalCode::NotObservable202,
                "registered forwarding requires an independently pinned final head",
            ));
        }
        let mut issuance = self
            .ledger
            .issuance(request.source_issuance)
            .cloned()
            .ok_or_else(|| {
                Refusal::new(
                    RefusalCode::NoClaimInDomain201,
                    "source issuance identity is not registered in this journal",
                )
            })?;
        let bytes = read_artifact(source_path)?;
        self.ledger
            .verify_artifact(issuance.output_digest, &bytes)?;
        self.ledger.gate_current_reuse(
            issuance.output_digest,
            &TrustContext {
                pinned_head: Some(self.head()),
                time: Some(time),
            },
        )?;
        issuance.recipient_role = request.recipient_role;
        issuance.purpose = request.purpose;
        issuance.issued_at_ns = time.now_ns;
        self.issue_bytes(destination_path, &bytes, issuance, key, time)
    }

    /// Copy an already issued artifact for current reuse, keeping this journal lock throughout the copy.
    /// DESIGN: only an independently supplied final-head pin plus bounded trusted time can grant reuse.
    /// The source is read once; those same verified bytes are written using `create_new` and synced.
    /// This is retrieval of an existing issuance, not a new recipient record or a freshness receipt.
    pub fn export_current(
        &self,
        source_path: &Path,
        destination_path: &Path,
        time: TrustedTime,
    ) -> Result<Digest32, Refusal> {
        if !self.is_pinned() {
            return Err(Refusal::new(
                RefusalCode::NotObservable202,
                "current artifact reuse requires an independently pinned journal read",
            ));
        }
        let bytes = read_artifact(source_path)?;
        let digest = blake3_hash(&bytes);
        self.ledger.verify_artifact(digest, &bytes)?;
        self.ledger.gate_current_reuse(
            digest,
            &TrustContext {
                pinned_head: Some(self.head()),
                time: Some(time),
            },
        )?;
        write_artifact_new(destination_path, &bytes)?;
        Ok(digest)
    }

    /// Validate authority and the transition, append one complete line, then sync before publishing its head.
    /// A partial write or failed sync poisons this handle; it cannot retry or repair the journal.
    pub fn append(
        &mut self,
        entry: Entry,
        key: &ReleaseKey,
        time: TrustedTime,
    ) -> Result<JournalRecord, Refusal> {
        let next = self.prepare_append(entry, key, time)?;
        self.commit_append(next)
    }

    fn commit_append(&mut self, next: PreparedAppend) -> Result<JournalRecord, Refusal> {
        self.poisoned = true;
        self.file.write_all(&next.line).map_err(|e| {
            integrity(format!(
                "reliance append failed; retained bytes require inspection: {e}"
            ))
        })?;
        self.file.sync_all().map_err(|e| {
            integrity(format!(
                "reliance sync failed; retained bytes require inspection: {e}"
            ))
        })?;
        self.ledger = next.ledger;
        self.byte_len = next.byte_len;
        self.pinned = false;
        self.poisoned = false;
        Ok(next.record)
    }

    fn prepare_append(
        &self,
        entry: Entry,
        key: &ReleaseKey,
        time: TrustedTime,
    ) -> Result<PreparedAppend, Refusal> {
        if !self.writable {
            return Err(authority(
                "reliance append requires an exclusive writer handle",
            ));
        }
        if self.poisoned {
            return Err(integrity(
                "reliance writer is poisoned after an incomplete append",
            ));
        }
        if self.ledger.records().len() >= MAX_JOURNAL_RECORDS {
            return Err(integrity("reliance journal exceeds its record bound"));
        }
        let capability = ReleaseCapability::verify_key(&key.0, self.ledger.header())?;
        let mut next = self.ledger.clone();
        let record = next.append(entry, &capability, time)?;
        let line = encode_line(&record)?;
        let new_len = self
            .byte_len
            .checked_add(line.len() as u64)
            .filter(|n| *n <= MAX_JOURNAL_BYTES)
            .ok_or_else(|| integrity("reliance journal exceeds its byte bound"))?;
        Ok(PreparedAppend {
            ledger: next,
            record,
            line,
            byte_len: new_len,
        })
    }
}

fn artifact_bound(length: u64) -> Result<(), Refusal> {
    if length > MAX_ARTIFACT_BYTES {
        return Err(Refusal::new(
            RefusalCode::InputMissingOrUnreadable506,
            "issued artifact exceeds its byte bound",
        ));
    }
    Ok(())
}

/// Read exact artifact bytes once with a fixed size bound; never follow a symlink or read a special file.
pub fn read_artifact(path: &Path) -> Result<Vec<u8>, Refusal> {
    let metadata = std::fs::symlink_metadata(path)
        .map_err(|e| io_refusal("read artifact metadata", path, &e))?;
    if !metadata.is_file() {
        return Err(Refusal::new(
            RefusalCode::InputMissingOrUnreadable506,
            format!("artifact is not a regular file: {}", path.display()),
        ));
    }
    artifact_bound(metadata.len())?;
    let file = File::open(path).map_err(|e| io_refusal("open artifact", path, &e))?;
    let metadata = file
        .metadata()
        .map_err(|e| io_refusal("read opened artifact metadata", path, &e))?;
    if !metadata.is_file() {
        return Err(Refusal::new(
            RefusalCode::InputMissingOrUnreadable506,
            format!("opened artifact is not a regular file: {}", path.display()),
        ));
    }
    artifact_bound(metadata.len())?;
    let mut bytes = Vec::new();
    file.take(MAX_ARTIFACT_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| io_refusal("read artifact", path, &e))?;
    artifact_bound(bytes.len() as u64)?;
    Ok(bytes)
}

fn write_artifact_new(path: &Path, bytes: &[u8]) -> Result<(), Refusal> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.mode(0o600);
    }
    let mut file = options
        .open(path)
        .map_err(|e| io_refusal("create issued artifact", path, &e))?;
    file.write_all(bytes)
        .and_then(|()| file.sync_all())
        .map_err(|e| io_refusal("write and sync issued artifact", path, &e))?;
    #[cfg(unix)]
    {
        let parent = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        File::open(parent)
            .and_then(|dir| dir.sync_all())
            .map_err(|e| io_refusal("sync artifact parent directory", parent, &e))?;
    }
    Ok(())
}

fn regular_journal(metadata: &std::fs::Metadata) -> Result<(), Refusal> {
    if !metadata.is_file() || metadata.len() > MAX_JOURNAL_BYTES {
        return Err(integrity("reliance journal is not a bounded regular file"));
    }
    Ok(())
}

fn acquire(file: &File, exclusive: bool) -> Result<(), Refusal> {
    let result = if exclusive {
        file.try_lock()
    } else {
        file.try_lock_shared()
    };
    result.map_err(|e| match e {
        TryLockError::WouldBlock => {
            integrity("reliance journal is locked; retry with the current head")
        }
        TryLockError::Error(e) => integrity(format!("reliance file locking unavailable: {e}")),
    })
}

fn encode_line(record: &JournalRecord) -> Result<Vec<u8>, Refusal> {
    let frame = record.to_bytes()?;
    if frame.len() > MAX_RECORD_BYTES {
        return Err(integrity("reliance record exceeds its byte bound"));
    }
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut line = Vec::with_capacity(frame.len() * 2 + 1);
    for byte in frame {
        line.push(HEX[usize::from(byte >> 4)]);
        line.push(HEX[usize::from(byte & 15)]);
    }
    line.push(b'\n');
    Ok(line)
}

fn decode_journal(bytes: &[u8]) -> Result<RelianceLedger, Refusal> {
    if bytes.len() as u64 > MAX_JOURNAL_BYTES {
        return Err(integrity("reliance journal exceeds its byte bound"));
    }
    let Some(body) = bytes.strip_suffix(b"\n") else {
        return Err(integrity("reliance journal has an incomplete final line"));
    };
    let mut records = Vec::new();
    for line in body.split(|b| *b == b'\n') {
        if records.len() >= MAX_JOURNAL_RECORDS {
            return Err(integrity("reliance journal exceeds its record bound"));
        }
        if line.is_empty() || line.len() % 2 != 0 || line.len() > 2 * MAX_RECORD_BYTES {
            return Err(integrity("reliance journal contains an invalid-sized line"));
        }
        let mut frame = Vec::with_capacity(line.len() / 2);
        for pair in line.as_chunks::<2>().0 {
            let (Some(high), Some(low)) = (nibble(pair[0]), nibble(pair[1])) else {
                return Err(integrity(
                    "reliance journal requires canonical lowercase hex lines",
                ));
            };
            frame.push((high << 4) | low);
        }
        let record = JournalRecord::from_bytes(&frame)?;
        if record.to_bytes()? != frame {
            return Err(integrity("reliance journal contains a noncanonical frame"));
        }
        records.push(record);
    }
    RelianceLedger::from_records(&records)
}

const fn nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        _ => None,
    }
}

fn integrity(detail: impl Into<String>) -> Refusal {
    Refusal::new(RefusalCode::StaleAnchor502, detail)
}

fn authority(detail: impl Into<String>) -> Refusal {
    Refusal::new(RefusalCode::MissingHumanToken304, detail)
}

fn io_refusal(operation: &str, path: &Path, error: &std::io::Error) -> Refusal {
    Refusal::new(
        RefusalCode::InputMissingOrUnreadable506,
        format!("{operation} at {}: {error}", path.display()),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use fmine_claim::Confidentiality;
    use fmine_claim::reliance::{
        DefectClass, IssuanceLane, IssuanceRecord, RoleId, StatusRefresh, Supersession, Withdrawal,
    };
    use fmine_core::LedgerAnchor;
    use fmine_core::hash::blake3_hash;
    use fmine_core::id::{ObjectId, PrefixKind};
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::{Arc, Barrier};

    static NEXT: AtomicU64 = AtomicU64::new(0);

    fn workspace() -> PathBuf {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("test clock")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "fmine-reliance-{}-{nonce}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).expect("new test workspace");
        path
    }

    fn write_new(path: &Path, bytes: &[u8]) {
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt as _;
            options.mode(0o600);
        }
        options
            .open(path)
            .expect("new fixture")
            .write_all(bytes)
            .expect("fixture bytes");
    }

    fn key() -> ReleaseKey {
        ReleaseKey([0x7b; 32])
    }

    fn header() -> Header {
        Header {
            release_role: RoleId::ReleaseOwner,
            release_key_sha256: key().sha256(),
            max_status_age_ns: 100,
            max_clock_skew_ns: 5,
            created_at_ns: 0,
        }
    }

    fn time(now_ns: i64) -> TrustedTime {
        TrustedTime {
            now_ns,
            uncertainty_ns: 0,
        }
    }

    fn refresh(at_ns: i64) -> Entry {
        Entry::Refreshed(StatusRefresh {
            by_role: RoleId::ReleaseOwner,
            at_ns,
        })
    }

    fn issuance() -> IssuanceRecord {
        IssuanceRecord {
            output_digest: blake3_hash(b"synthetic output\n"),
            output_claims: vec![ObjectId::from_raw(PrefixKind::Claim, [1; 16])],
            anchor: LedgerAnchor {
                format_version: 1,
                cell_id: "CELL-SYNTHETIC".to_string(),
                ledger_epoch: 1,
                commit_sequence: 1,
                adapter_registry_epoch: 1,
                schema_epoch: 1,
                policy_epoch: 1,
                privacy_epoch: 1,
                residency_epoch: 1,
                state_root: blake3_hash(b"synthetic state"),
            },
            code_revision: "1111111".to_string(),
            binary_digest: blake3_hash(b"synthetic executable"),
            input_manifest_root: blake3_hash(b"synthetic inputs"),
            lane: IssuanceLane::Synthetic,
            confidentiality: Confidentiality::public(),
            public_taint: false,
            recipient_role: RoleId::Sponsor,
            purpose: "synthetic review".to_string(),
            issued_at_ns: 10,
        }
    }

    fn reissue_request(source: &IssuanceRecord) -> ReissueRequest {
        ReissueRequest {
            source_issuance: source.digest().expect("source issuance identity"),
            recipient_role: RoleId::ReviewerPool(7),
            purpose: "synthetic forwarded review".to_string(),
        }
    }

    #[test]
    fn issue_and_withdraw_preserve_every_prior_byte_and_never_store_the_key() {
        let path = workspace().join("journal");
        let mut journal = Journal::create(&path, header()).expect("create");
        let genesis = std::fs::read(&path).expect("genesis bytes");
        let issued = issuance();
        let entry = Entry::Issued(issued.clone());
        journal
            .validate_append(&entry, &key(), time(10))
            .expect("preflight");
        assert_eq!(std::fs::read(&path).expect("read"), genesis);
        journal.append(entry, &key(), time(10)).expect("issue");
        let before = std::fs::read(&path).expect("issued bytes");
        assert!(before.starts_with(&genesis));
        let withdrawal = Entry::Withdrawn(Withdrawal {
            output_digest: issued.output_digest,
            reason: "synthetic code substitution".to_string(),
            affected_claims: issued.output_claims,
            defect_class: DefectClass::CodeDefect,
            by_role: RoleId::ReleaseOwner,
            at_ns: 20,
        });
        journal
            .append(withdrawal, &key(), time(20))
            .expect("withdraw");
        let head = journal.head();
        assert!(
            !journal.is_pinned(),
            "a newly written head is not an external trust pin"
        );
        let bytes = std::fs::read(&path).expect("withdrawn bytes");
        assert!(bytes.starts_with(&before));
        assert!(!bytes.windows(32).any(|w| w == [0x7b; 32]));
        assert!(!String::from_utf8_lossy(&bytes).contains(&"7b".repeat(32)));
        assert_eq!(format!("{:?}", key()), "ReleaseKey([REDACTED])");
        let line =
            encode_line(journal.ledger().records().last().expect("last record")).expect("line");
        assert_eq!(head, blake3_hash(&line));
        drop(journal);
        let read = Journal::open_read(&path, Some(head)).expect("pinned replay");
        assert!(read.is_pinned());
        assert_eq!(read.ledger().records().len(), 3);
        assert!(matches!(
            read.ledger().records()[2].entry,
            Entry::Withdrawn(_)
        ));
        assert_eq!(issued.output_digest, blake3_hash(b"synthetic output\n"));
    }

    #[test]
    fn exact_artifact_issue_and_current_copy_refuse_after_withdrawal() {
        let dir = workspace();
        let path = dir.join("journal");
        let output = dir.join("issued");
        let copied = dir.join("current-copy");
        let blocked = dir.join("withdrawn-copy");
        let bytes = b"synthetic output\n";
        let issued = issuance();
        let mut journal = Journal::create(&path, header()).expect("create");
        journal
            .issue_bytes(&output, bytes, issued.clone(), &key(), time(10))
            .expect("durable issuance");
        let head = journal.head();
        let journal_bytes = std::fs::read(&path).expect("committed journal");
        assert_eq!(std::fs::read(&output).expect("artifact"), bytes);
        assert_eq!(journal.ledger().copies(issued.output_digest), vec![&issued]);
        assert_eq!(
            journal
                .export_current(&output, &copied, time(11))
                .expect_err("new head is not an independent pin")
                .code,
            "FMINE-REF-202"
        );
        assert!(!copied.exists());
        drop(journal);
        let reader = Journal::open_read(&path, Some(head)).expect("independently pinned reader");
        assert_eq!(
            reader
                .export_current(&output, &copied, time(11))
                .expect("current retrieval"),
            issued.output_digest
        );
        assert_eq!(std::fs::read(&copied).expect("exact copy"), bytes);
        assert_eq!(
            std::fs::read(&path).expect("read-only journal"),
            journal_bytes
        );
        assert!(
            Journal::open_write(&path, head).is_err(),
            "reader lock retained"
        );
        drop(reader);
        let mut writer = Journal::open_write(&path, head).expect("owner after copy");
        writer
            .append(
                Entry::Withdrawn(Withdrawal {
                    output_digest: issued.output_digest,
                    reason: "synthetic code defect".to_string(),
                    affected_claims: issued.output_claims.clone(),
                    defect_class: DefectClass::CodeDefect,
                    by_role: RoleId::ReleaseOwner,
                    at_ns: 20,
                }),
                &key(),
                time(20),
            )
            .expect("withdraw");
        let withdrawn_head = writer.head();
        drop(writer);
        let reader = Journal::open_read(&path, Some(withdrawn_head)).expect("current status pin");
        reader
            .ledger()
            .verify_artifact(
                issued.output_digest,
                &std::fs::read(&output).expect("old artifact"),
            )
            .expect("historical bytes still verify");
        assert_eq!(
            reader.ledger().copies(issued.output_digest)[0].anchor,
            issued.anchor
        );
        assert_eq!(
            reader
                .export_current(&output, &blocked, time(21))
                .expect_err("withdrawn bytes cannot be current")
                .code,
            "FMINE-REF-202"
        );
        assert!(!blocked.exists());
        assert_eq!(std::fs::read(&output).expect("original preserved"), bytes);
    }

    #[test]
    fn registered_forwarding_preserves_the_selected_producer_and_recall_population() {
        let dir = workspace();
        let path = dir.join("journal");
        let source = dir.join("first-producer");
        let output = dir.join("forwarded");
        let bytes = b"synthetic output\n";
        let first = issuance();
        let mut second = first.clone();
        second.code_revision = "2222222".to_string();
        second.binary_digest = blake3_hash(b"second synthetic executable");
        second.issued_at_ns = 11;
        let mut journal = Journal::create(&path, header()).expect("create");
        journal
            .issue_bytes(&source, bytes, first.clone(), &key(), time(10))
            .expect("first producer");
        journal
            .issue_bytes(
                &dir.join("second-producer"),
                bytes,
                second.clone(),
                &key(),
                time(11),
            )
            .expect("same bytes from a different producer");
        let prior_head = journal.head();
        let prior_bytes = std::fs::read(&path).expect("prior history");
        drop(journal);
        let mut writer = Journal::open_write(&path, prior_head).expect("pinned writer");
        let request = reissue_request(&first);
        let mut expected = first.clone();
        expected.recipient_role = request.recipient_role.clone();
        expected.purpose = request.purpose.clone();
        expected.issued_at_ns = 12;
        let record = writer
            .reissue_current(&source, &output, request, &key(), time(12))
            .expect("registered forwarding");
        assert_eq!(record.entry, Entry::Issued(expected.clone()));
        assert_eq!(std::fs::read(&output).expect("forwarded bytes"), bytes);
        assert_eq!(std::fs::read(&source).expect("original bytes"), bytes);
        assert!(
            std::fs::read(&path)
                .expect("appended history")
                .starts_with(&prior_bytes)
        );
        assert_eq!(
            writer.ledger().copies(first.output_digest),
            vec![&first, &second, &expected]
        );
        assert_eq!(
            writer.ledger().recall_revision(&first.code_revision),
            vec![&first, &expected]
        );
        assert_eq!(
            writer.ledger().recall_revision(&second.code_revision),
            vec![&second]
        );
        assert!(
            !writer.is_pinned(),
            "new issuance does not grant a new external pin"
        );
        assert!(
            Journal::open_read(&path, None).is_err(),
            "writer lock remains held"
        );
        let head = writer.head();
        drop(writer);
        let replayed = Journal::open_read(&path, Some(head)).expect("replay registered copy");
        assert_eq!(
            replayed
                .ledger()
                .issuance(first.digest().expect("first ID")),
            Some(&first)
        );
        assert_eq!(
            replayed
                .ledger()
                .issuance(second.digest().expect("second ID")),
            Some(&second)
        );
        assert_eq!(
            replayed
                .ledger()
                .issuance(expected.digest().expect("new ID")),
            Some(&expected)
        );
    }

    #[test]
    fn registered_forwarding_preflight_never_creates_output_or_refreshes_invalid_state() {
        let dir = workspace();
        let path = dir.join("journal");
        let source = dir.join("source");
        let altered = dir.join("altered");
        let missing = dir.join("missing");
        let output = dir.join("forwarded");
        let issued = issuance();
        let id = issued.digest().expect("receipt ID");
        let mut journal = Journal::create(&path, header()).expect("create");
        journal
            .issue_bytes(
                &source,
                b"synthetic output\n",
                issued.clone(),
                &key(),
                time(10),
            )
            .expect("issue");
        let head = journal.head();
        let before = std::fs::read(&path).expect("prior journal");
        drop(journal);
        write_new(&altered, b"changed synthetic output\n");
        let mut writer = Journal::open_write(&path, head).expect("pinned writer");
        for (source_id, source_path, purpose, release_key, clock, code) in [
            (
                Digest32::default(),
                &source,
                "review",
                key(),
                time(11),
                "FMINE-REF-201",
            ),
            (
                issued.output_digest,
                &source,
                "review",
                key(),
                time(11),
                "FMINE-REF-201",
            ),
            (id, &missing, "review", key(), time(11), "FMINE-REF-506"),
            (id, &altered, "review", key(), time(11), "FMINE-REF-505"),
            (
                id,
                &source,
                "review",
                ReleaseKey([9; 32]),
                time(11),
                "FMINE-REF-304",
            ),
            (id, &source, "review", key(), time(106), "FMINE-REF-202"),
            (id, &source, "review", key(), time(9), "FMINE-REF-202"),
            (
                id,
                &source,
                "review",
                key(),
                TrustedTime {
                    now_ns: 11,
                    uncertainty_ns: 6,
                },
                "FMINE-REF-202",
            ),
            (id, &source, "", key(), time(11), "FMINE-REF-107"),
        ] {
            let request = ReissueRequest {
                source_issuance: source_id,
                recipient_role: RoleId::ReviewerPool(7),
                purpose: purpose.to_string(),
            };
            assert_eq!(
                writer
                    .reissue_current(source_path, &output, request, &release_key, clock)
                    .expect_err("invalid forwarding")
                    .code,
                code
            );
            assert!(!output.exists(), "preflight must precede output creation");
            assert_eq!(writer.head(), head);
            assert_eq!(writer.ledger().status_cut_ns(), 10);
            assert_eq!(writer.ledger().issuances(), vec![&issued]);
            assert_eq!(std::fs::read(&path).expect("unchanged history"), before);
        }
        write_new(&output, b"existing destination");
        assert_eq!(
            writer
                .reissue_current(&source, &output, reissue_request(&issued), &key(), time(11))
                .expect_err("must not overwrite")
                .code,
            "FMINE-REF-506"
        );
        assert_eq!(
            std::fs::read(&output).expect("retained destination"),
            b"existing destination"
        );
        assert_eq!(writer.head(), head);
        assert_eq!(std::fs::read(&path).expect("unchanged history"), before);
    }

    #[test]
    fn registered_forwarding_requires_a_pinned_healthy_writer() {
        let dir = workspace();
        let path = dir.join("journal");
        let source = dir.join("source");
        let output = dir.join("forwarded");
        let issued = issuance();
        let mut journal = Journal::create(&path, header()).expect("create");
        journal
            .issue_bytes(
                &source,
                b"synthetic output\n",
                issued.clone(),
                &key(),
                time(10),
            )
            .expect("issue");
        let head = journal.head();
        let before = std::fs::read(&path).expect("prior history");
        assert_eq!(
            journal
                .reissue_current(&source, &output, reissue_request(&issued), &key(), time(11))
                .expect_err("written head is not a pin")
                .code,
            "FMINE-REF-202"
        );
        drop(journal);
        let mut reader = Journal::open_read(&path, Some(head)).expect("pinned reader");
        assert_eq!(
            reader
                .reissue_current(&source, &output, reissue_request(&issued), &key(), time(11))
                .expect_err("reader cannot register forwarding")
                .code,
            "FMINE-REF-304"
        );
        assert!(!output.exists());
        drop(reader);
        let mut writer = Journal::open_write(&path, head).expect("pinned writer");
        writer.poisoned = true;
        assert_eq!(
            writer
                .reissue_current(&source, &output, reissue_request(&issued), &key(), time(11))
                .expect_err("poisoned writer cannot create output")
                .code,
            "FMINE-REF-502"
        );
        assert!(!output.exists());
        assert_eq!(writer.head(), head);
        assert_eq!(std::fs::read(&path).expect("unchanged history"), before);
    }

    #[test]
    fn registered_forwarding_never_reissues_withdrawn_or_superseded_bytes() {
        for superseded in [false, true] {
            let dir = workspace();
            let path = dir.join("journal");
            let source = dir.join("source");
            let output = dir.join("forwarded");
            let issued = issuance();
            let mut journal = Journal::create(&path, header()).expect("create");
            journal
                .issue_bytes(
                    &source,
                    b"synthetic output\n",
                    issued.clone(),
                    &key(),
                    time(10),
                )
                .expect("issue");
            let entry = if superseded {
                let mut replacement = issued.clone();
                replacement.output_digest = blake3_hash(b"replacement\n");
                replacement.issued_at_ns = 11;
                journal
                    .issue_bytes(
                        &dir.join("replacement"),
                        b"replacement\n",
                        replacement.clone(),
                        &key(),
                        time(11),
                    )
                    .expect("issue replacement");
                Entry::Superseded(Supersession {
                    output_digest: issued.output_digest,
                    by_digest: replacement.output_digest,
                    by_role: RoleId::ReleaseOwner,
                    at_ns: 12,
                })
            } else {
                Entry::Withdrawn(Withdrawal {
                    output_digest: issued.output_digest,
                    reason: "synthetic code defect".to_string(),
                    affected_claims: issued.output_claims.clone(),
                    defect_class: DefectClass::CodeDefect,
                    by_role: RoleId::ReleaseOwner,
                    at_ns: 12,
                })
            };
            journal
                .append(entry, &key(), time(12))
                .expect("terminal status");
            let head = journal.head();
            let before = std::fs::read(&path).expect("terminal history");
            drop(journal);
            let mut writer = Journal::open_write(&path, head).expect("pinned writer");
            assert_eq!(
                writer
                    .reissue_current(&source, &output, reissue_request(&issued), &key(), time(13))
                    .expect_err("terminal status cannot be refreshed by forwarding")
                    .code,
                "FMINE-REF-202"
            );
            assert!(!output.exists());
            assert_eq!(writer.head(), head);
            assert_eq!(writer.ledger().status_cut_ns(), 12);
            assert_eq!(writer.ledger().copies(issued.output_digest), vec![&issued]);
            assert_eq!(std::fs::read(&path).expect("unchanged history"), before);
        }
    }

    #[test]
    fn issue_preflight_rejects_digest_key_time_and_existing_destination_without_append() {
        let dir = workspace();
        let path = dir.join("journal");
        let mut journal = Journal::create(&path, header()).expect("create");
        let before = std::fs::read(&path).expect("genesis");
        let output = dir.join("invalid");
        for (bytes, signing_key, clock, code) in [
            (&b"different bytes"[..], key(), time(10), "FMINE-REF-505"),
            (
                &b"synthetic output\n"[..],
                ReleaseKey([9; 32]),
                time(10),
                "FMINE-REF-304",
            ),
            (&b"synthetic output\n"[..], key(), time(11), "FMINE-REF-304"),
        ] {
            assert_eq!(
                journal
                    .issue_bytes(&output, bytes, issuance(), &signing_key, clock)
                    .expect_err("preflight")
                    .code,
                code
            );
            assert!(!output.exists());
            assert_eq!(std::fs::read(&path).expect("unchanged journal"), before);
        }
        write_new(&output, b"unrelated data");
        assert_eq!(
            journal
                .issue_bytes(&output, b"synthetic output\n", issuance(), &key(), time(10))
                .expect_err("must not overwrite")
                .code,
            "FMINE-REF-506"
        );
        assert_eq!(
            std::fs::read(&output).expect("existing data"),
            b"unrelated data"
        );
        assert_eq!(std::fs::read(&path).expect("unchanged journal"), before);
        assert_eq!(journal.ledger().records().len(), 1);
        assert!(!journal.poisoned, "no journal write was attempted");
    }

    #[test]
    fn current_copy_requires_fresh_bounded_trust_and_never_overwrites() {
        let dir = workspace();
        let path = dir.join("journal");
        let output = dir.join("issued");
        let copy = dir.join("copy");
        let mut journal = Journal::create(&path, header()).expect("create");
        journal
            .issue_bytes(&output, b"synthetic output\n", issuance(), &key(), time(10))
            .expect("issue");
        let head = journal.head();
        drop(journal);
        let reader = Journal::open_read(&path, None).expect("untrusted historical inspection");
        assert!(reader.export_current(&output, &copy, time(11)).is_err());
        assert!(!copy.exists());
        drop(reader);
        let reader = Journal::open_read(&path, Some(head)).expect("pinned reader");
        for clock in [
            time(9),
            time(106),
            TrustedTime {
                now_ns: 11,
                uncertainty_ns: 6,
            },
        ] {
            assert_eq!(
                reader
                    .export_current(&output, &copy, clock)
                    .expect_err("untrusted current use")
                    .code,
                "FMINE-REF-202"
            );
            assert!(!copy.exists());
        }
        let altered = dir.join("altered");
        write_new(&altered, b"altered artifact");
        assert!(reader.export_current(&altered, &copy, time(11)).is_err());
        assert!(!copy.exists());
        write_new(&copy, b"existing destination");
        assert_eq!(
            reader
                .export_current(&output, &copy, time(11))
                .expect_err("no overwrite")
                .code,
            "FMINE-REF-506"
        );
        assert_eq!(
            std::fs::read(copy).expect("preserved"),
            b"existing destination"
        );
    }

    #[test]
    fn failed_issuance_append_retains_artifact_without_granting_reuse() {
        let dir = workspace();
        let path = dir.join("journal");
        let output = dir.join("unissued");
        let mut journal = Journal::create(&path, header()).expect("create");
        let head = journal.head();
        let before = std::fs::read(&path).expect("genesis");
        journal.file = File::open(&path).expect("real read-only descriptor forces append failure");
        assert!(
            journal
                .issue_bytes(&output, b"synthetic output\n", issuance(), &key(), time(10))
                .is_err()
        );
        assert!(journal.poisoned);
        assert_eq!(journal.head(), head);
        assert_eq!(std::fs::read(&path).expect("journal unchanged"), before);
        assert_eq!(
            std::fs::read(&output).expect("retain unissued bytes"),
            b"synthetic output\n"
        );
        drop(journal);
        let reader = Journal::open_read(&path, Some(head)).expect("verified retained journal");
        assert!(reader.ledger().copies(issuance().output_digest).is_empty());
        assert!(
            reader
                .export_current(&output, &dir.join("blocked"), time(11))
                .is_err()
        );
        assert!(!dir.join("blocked").exists());
    }

    #[test]
    fn artifact_reads_refuse_special_files_and_oversized_files_before_allocating() {
        let dir = workspace();
        assert!(read_artifact(&dir).is_err());
        let oversized = dir.join("oversized");
        let file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&oversized)
            .expect("sparse file");
        file.set_len(MAX_ARTIFACT_BYTES + 1)
            .expect("oversized sparse file");
        assert_eq!(
            read_artifact(&oversized).expect_err("bound").code,
            "FMINE-REF-506"
        );
    }

    #[test]
    fn wrong_key_and_invalid_preflight_create_no_append() {
        let path = workspace().join("journal");
        let mut journal = Journal::create(&path, header()).expect("create");
        let before = std::fs::read(&path).expect("before");
        let wrong = ReleaseKey([0; 32]);
        let error = journal
            .validate_append(&refresh(10), &wrong, time(10))
            .expect_err("wrong key");
        assert_eq!(error.code, "FMINE-REF-304");
        assert!(journal.append(refresh(10), &wrong, time(10)).is_err());
        assert!(
            journal
                .validate_append(&refresh(10), &key(), time(11))
                .is_err()
        );
        assert_eq!(std::fs::read(&path).expect("unchanged"), before);
        assert_eq!(journal.ledger().records().len(), 1);
        journal
            .append(refresh(10), &key(), time(10))
            .expect("valid key still works");
    }

    #[test]
    fn held_reader_blocks_withdrawal_writer_and_cas_rejects_old_heads() {
        let path = workspace().join("journal");
        let journal = Journal::create(&path, header()).expect("create");
        let initial = journal.head();
        assert!(
            Journal::open_read(&path, Some(initial)).is_err(),
            "writer holds OS lock"
        );
        drop(journal);
        let reader = Journal::open_read(&path, Some(initial)).expect("reader");
        let other = Journal::open_read(&path, Some(initial)).expect("another shared reader");
        assert!(
            Journal::open_write(&path, initial).is_err(),
            "no write during gated use"
        );
        drop(other);
        drop(reader);
        let mut writer = Journal::open_write(&path, initial).expect("writer after readers finish");
        writer
            .append(refresh(10), &key(), time(10))
            .expect("refresh");
        let head = writer.head();
        drop(writer);
        assert_eq!(
            Journal::open_write(&path, initial).expect_err("CAS").code,
            "FMINE-REF-502"
        );
        assert_eq!(
            Journal::open_read(&path, Some(initial))
                .expect_err("ancestor pin")
                .code,
            "FMINE-REF-502"
        );
        let mut reader = Journal::open_read(&path, Some(head)).expect("current pin");
        assert!(
            reader.append(refresh(20), &key(), time(20)).is_err(),
            "read-only handle"
        );
        drop(reader);
        assert!(
            !Journal::open_read(&path, None)
                .expect("inspection")
                .is_pinned()
        );
    }

    #[test]
    fn concurrent_same_head_writers_commit_exactly_once() {
        let path = workspace().join("journal");
        let journal = Journal::create(&path, header()).expect("create");
        let head = journal.head();
        drop(journal);
        let barrier = Arc::new(Barrier::new(3));
        let handles: Vec<_> = (0..3)
            .map(|_| {
                let path = path.clone();
                let barrier = Arc::clone(&barrier);
                std::thread::spawn(move || {
                    barrier.wait();
                    Journal::open_write(&path, head)
                        .and_then(|mut journal| journal.append(refresh(10), &key(), time(10)))
                })
            })
            .collect();
        let results: Vec<_> = handles
            .into_iter()
            .map(|h| h.join().expect("writer thread"))
            .collect();
        assert_eq!(results.iter().filter(|r| r.is_ok()).count(), 1);
        let journal = Journal::open_read(&path, None).expect("unforked journal");
        assert_eq!(journal.ledger().records().len(), 2);
        assert_eq!(journal.ledger().records()[1].previous, head);
    }

    #[test]
    fn corruption_torn_append_reordering_and_replayed_prefix_fail_closed() {
        let dir = workspace();
        let path = dir.join("intact");
        let mut journal = Journal::create(&path, header()).expect("create");
        journal
            .append(refresh(10), &key(), time(10))
            .expect("first refresh");
        journal
            .append(refresh(20), &key(), time(20))
            .expect("second refresh");
        let head = journal.head();
        let lines: Vec<Vec<u8>> = journal
            .ledger()
            .records()
            .iter()
            .map(|r| encode_line(r).expect("canonical line"))
            .collect();
        drop(journal);
        let bytes = lines.concat();
        let mut bad = Vec::new();
        bad.push(bytes[..bytes.len() - 1].to_vec());
        bad.push([bytes.as_slice(), b"00"].concat());
        let mut upper = bytes.clone();
        let at = upper
            .iter()
            .position(|b| (b'a'..=b'f').contains(b))
            .expect("hex letter");
        upper[at] = upper[at].to_ascii_uppercase();
        bad.push(upper);
        let mut changed = bytes.clone();
        changed[0] = if changed[0] == b'0' { b'1' } else { b'0' };
        bad.push(changed);
        bad.push([lines[0].as_slice(), lines[2].as_slice()].concat());
        bad.push([bytes.as_slice(), lines[2].as_slice()].concat());
        bad.push(
            [
                lines[1].as_slice(),
                lines[0].as_slice(),
                lines[2].as_slice(),
            ]
            .concat(),
        );
        bad.push([bytes.as_slice(), b"\n"].concat());
        bad.push(lines[0].clone());
        // A structurally valid new line cannot establish trust in its own extended head.
        let forged = JournalRecord {
            previous: head,
            entry: refresh(30),
        };
        bad.push(
            [
                bytes.as_slice(),
                encode_line(&forged).expect("frame").as_slice(),
            ]
            .concat(),
        );
        for (i, bytes) in bad.iter().enumerate() {
            let path = dir.join(format!("bad-{i}"));
            write_new(&path, bytes);
            assert_eq!(
                Journal::open_read(&path, Some(head))
                    .expect_err("must refuse")
                    .code,
                "FMINE-REF-502",
                "case {i}"
            );
            assert_eq!(std::fs::read(path).expect("retained evidence"), *bytes);
        }
        let mut oversized = vec![b'0'; 2 * MAX_RECORD_BYTES + 2];
        oversized.push(b'\n');
        assert!(decode_journal(&oversized).is_err());
        assert!(decode_journal(b"0\n").is_err());
        assert!(decode_journal(b"\n").is_err());
    }

    #[test]
    fn failed_write_poisoning_preserves_committed_state_and_never_repairs() {
        let path = workspace().join("journal");
        let mut journal = Journal::create(&path, header()).expect("create");
        let head = journal.head();
        let before = std::fs::read(&path).expect("before");
        // A real read-only descriptor forces write_all to fail before the state can be published.
        journal.file = File::open(&path).expect("read-only descriptor");
        assert!(journal.append(refresh(10), &key(), time(10)).is_err());
        assert!(journal.poisoned);
        assert_eq!(journal.head(), head);
        assert_eq!(journal.ledger().records().len(), 1);
        assert!(journal.append(refresh(20), &key(), time(20)).is_err());
        assert_eq!(std::fs::read(&path).expect("retained bytes"), before);
    }

    #[cfg(unix)]
    #[test]
    fn key_loader_refuses_shared_permissions_wrong_lengths_and_symlinks() {
        use std::os::unix::fs::{PermissionsExt as _, symlink};
        let dir = workspace();
        let private = dir.join("private-key");
        write_new(&private, &[0x7b; 32]);
        assert_eq!(
            ReleaseKey::read(&private).expect("private key").sha256(),
            key().sha256()
        );
        let shared = dir.join("shared-key");
        write_new(&shared, &[0x7b; 32]);
        std::fs::set_permissions(&shared, std::fs::Permissions::from_mode(0o644))
            .expect("permissions");
        assert_eq!(
            ReleaseKey::read(&shared)
                .expect_err("shared permissions")
                .code,
            "FMINE-REF-304"
        );
        for length in [0, 31, 33, 64] {
            let path = dir.join(format!("length-{length}"));
            write_new(&path, &vec![0x7b; length]);
            assert!(ReleaseKey::read(&path).is_err());
        }
        let zero = dir.join("zero-key");
        write_new(&zero, &[0; 32]);
        assert_eq!(
            ReleaseKey::read(&zero)
                .expect_err("unusable genesis key")
                .code,
            "FMINE-REF-304"
        );
        let link = dir.join("key-link");
        symlink(&private, &link).expect("test symlink");
        assert!(ReleaseKey::read(&link).is_err());
        let journal_path = dir.join("journal");
        let journal = Journal::create(&journal_path, header()).expect("journal");
        let head = journal.head();
        drop(journal);
        let link = dir.join("journal-link");
        symlink(&journal_path, &link).expect("journal symlink");
        assert!(Journal::open_read(&link, Some(head)).is_err());
    }

    #[test]
    fn creating_existing_path_never_overwrites_it() {
        let path = workspace().join("existing");
        write_new(&path, b"unrelated existing bytes\n");
        assert!(Journal::create(&path, header()).is_err());
        assert_eq!(
            std::fs::read(path).expect("existing bytes"),
            b"unrelated existing bytes\n"
        );
    }
}
