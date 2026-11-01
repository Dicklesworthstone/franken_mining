//! Issued-output reliance (BD-347). Determinism: BitwisePortable.
//!
//! A hash chain establishes integrity, not authority or freshness. Replay never supplies a trusted clock,
//! status pin, or release capability. The I/O boundary must obtain those separately from the owner/runtime.
//! Historical artifact integrity remains checkable after withdrawal; only the current-use gate grants reuse.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use fmine_core::anchor::LedgerAnchor;
use fmine_core::codec::{Decoder, Encoder};
use fmine_core::hash::{Digest32, blake3_hash, sha256};
use fmine_core::id::{ObjectId, PrefixKind};
use fmine_evidence::{Refusal, RefusalCode};

use crate::Confidentiality;

pub const MAX_RECORD_BYTES: usize = 4 * 1024 * 1024;
pub const MAX_JOURNAL_BYTES: usize = 64 * 1024 * 1024;
pub const MAX_RECORDS: usize = 100_000;
pub const MAX_CLAIMS: usize = 100_000;
pub const MAX_TEXT: usize = 256;

/// Generic recipient categories only; no free-form person, email, address or contact field exists.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum RoleId {
    ReleaseOwner,
    Sponsor,
    ReviewerPool(u32),
    AgentSession([u8; 16]),
}

impl RoleId {
    pub fn parse(s: &str) -> Result<Self, Refusal> {
        let role = match s {
            "release-owner" => Self::ReleaseOwner,
            "sponsor" => Self::Sponsor,
            _ if s.starts_with("reviewer-pool-") => Self::ReviewerPool(
                s[14..]
                    .parse()
                    .map_err(|_| invalid("reviewer pool needs a numeric id"))?,
            ),
            _ if s.starts_with("agent-session:") => {
                let hex = &s[14..];
                if hex.len() != 32
                    || !hex
                        .bytes()
                        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
                {
                    return Err(invalid(
                        "agent session needs 32 lowercase hexadecimal digits",
                    ));
                }
                let mut bytes = [0; 16];
                for (i, b) in bytes.iter_mut().enumerate() {
                    *b = u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16)
                        .map_err(|_| invalid("invalid session id"))?;
                }
                Self::AgentSession(bytes)
            }
            _ => {
                return Err(invalid(
                    "recipient must be a generic release-owner, sponsor, reviewer-pool-N or agent-session:<hex> role",
                ));
            }
        };
        if role.to_string() != s {
            return Err(invalid("noncanonical generic role id"));
        }
        Ok(role)
    }
}

impl fmt::Display for RoleId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ReleaseOwner => f.write_str("release-owner"),
            Self::Sponsor => f.write_str("sponsor"),
            Self::ReviewerPool(n) => write!(f, "reviewer-pool-{n}"),
            Self::AgentSession(id) => {
                f.write_str("agent-session:")?;
                for b in id {
                    write!(f, "{b:02x}")?;
                }
                Ok(())
            }
        }
    }
}

/// Immutable owner policy. Its authentication comes from the separately trusted exact head pin.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Header {
    pub release_role: RoleId,
    pub release_key_sha256: Digest32,
    pub max_status_age_ns: u64,
    pub max_clock_skew_ns: u64,
    pub created_at_ns: i64,
}

impl Header {
    fn validate(&self) -> Result<(), Refusal> {
        if self.release_role != RoleId::ReleaseOwner {
            return Err(denied(
                "only the configured release-owner role can release journal changes",
            ));
        }
        nonzero(self.release_key_sha256, "release key digest")?;
        if self.created_at_ns < 0
            || self.max_status_age_ns == 0
            || self.max_clock_skew_ns > self.max_status_age_ns
        {
            return Err(invalid("invalid reliance clock/freshness policy"));
        }
        Ok(())
    }
    pub fn digest(&self) -> Result<Digest32, Refusal> {
        self.validate()?;
        let mut enc = Encoder::new_frame(*b"RLYH", 1);
        encode_header(&mut enc, self);
        Ok(blake3_hash(&enc.finish()))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IssuanceLane {
    Public,
    Synthetic,
}
impl IssuanceLane {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Public => "public",
            Self::Synthetic => "synthetic",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IssuanceRecord {
    /// BLAKE3 of the exact issued bytes, independently of journal framing.
    pub output_digest: Digest32,
    pub output_claims: Vec<ObjectId>,
    pub anchor: LedgerAnchor,
    pub code_revision: String,
    pub binary_digest: Digest32,
    pub input_manifest_root: Digest32,
    pub lane: IssuanceLane,
    pub confidentiality: Confidentiality,
    pub public_taint: bool,
    pub recipient_role: RoleId,
    pub purpose: String,
    pub issued_at_ns: i64,
}

impl IssuanceRecord {
    fn validate(&self) -> Result<(), Refusal> {
        nonzero(self.output_digest, "output digest")?;
        nonzero(self.binary_digest, "binary digest")?;
        nonzero(self.input_manifest_root, "input manifest root")?;
        nonzero(self.anchor.state_root, "actual ledger state root")?;
        claims(&self.output_claims)?;
        text(&self.code_revision)?;
        text(&self.purpose)?;
        text(&self.anchor.cell_id)?;
        if !self.confidentiality.is_public()
            || (self.lane == IssuanceLane::Public && !self.public_taint)
        {
            return Err(Refusal::new(
                RefusalCode::MnpiWall301,
                "public issuance cannot declassify private/walled inputs or omit public provenance taint",
            ));
        }
        let roundtrip = LedgerAnchor::from_bytes(&self.anchor.to_bytes()).map_err(codec)?;
        if roundtrip != self.anchor {
            return Err(invalid("noncanonical actual LedgerAnchor"));
        }
        if self.issued_at_ns < 0 {
            return Err(invalid("negative issuance time"));
        }
        Ok(())
    }
    /// Canonical issuance identity, distinct from its output-byte digest and journal frame/head.
    pub fn digest(&self) -> Result<Digest32, Refusal> {
        self.validate()?;
        let mut enc = Encoder::new_frame(*b"RLYI", 1);
        encode_issue(&mut enc, self);
        Ok(blake3_hash(&enc.finish()))
    }
    fn same_artifact(&self, other: &Self) -> bool {
        // Different builds can emit identical bytes. Revision/binary and recipient metadata belong to each
        // copy and remain individually recallable, while the artifact's Claims/anchor/taint stay immutable.
        self.output_claims == other.output_claims
            && self.anchor == other.anchor
            && self.input_manifest_root == other.input_manifest_root
            && self.lane == other.lane
            && self.confidentiality == other.confidentiality
            && self.public_taint == other.public_taint
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DefectClass {
    CodeDefect,
    SourceRestated,
    ScopeError,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Withdrawal {
    pub output_digest: Digest32,
    pub reason: String,
    pub affected_claims: Vec<ObjectId>,
    pub defect_class: DefectClass,
    pub by_role: RoleId,
    pub at_ns: i64,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Supersession {
    pub output_digest: Digest32,
    pub by_digest: Digest32,
    pub by_role: RoleId,
    pub at_ns: i64,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusRefresh {
    pub by_role: RoleId,
    pub at_ns: i64,
}
// Entries own a bounded issuance payload. Keeping it inline avoids an allocation per issued copy and keeps
// the public append/codec API uniform; journal/frame byte limits separately bound the resident population.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Entry {
    Genesis(Header),
    Issued(IssuanceRecord),
    Withdrawn(Withdrawal),
    Superseded(Supersession),
    Refreshed(StatusRefresh),
}

impl Entry {
    pub const fn at_ns(&self) -> i64 {
        match self {
            Self::Genesis(h) => h.created_at_ns,
            Self::Issued(i) => i.issued_at_ns,
            Self::Withdrawn(w) => w.at_ns,
            Self::Superseded(s) => s.at_ns,
            Self::Refreshed(r) => r.at_ns,
        }
    }
    fn role(&self) -> Option<&RoleId> {
        match self {
            Self::Withdrawn(w) => Some(&w.by_role),
            Self::Superseded(s) => Some(&s.by_role),
            Self::Refreshed(r) => Some(&r.by_role),
            _ => None,
        }
    }
    fn validate(&self) -> Result<(), Refusal> {
        if self.at_ns() < 0 {
            return Err(invalid("negative event time"));
        }
        match self {
            Self::Genesis(h) => h.validate(),
            Self::Issued(i) => i.validate(),
            Self::Withdrawn(w) => {
                nonzero(w.output_digest, "withdrawn output")?;
                text(&w.reason)?;
                claims(&w.affected_claims)
            }
            Self::Superseded(s) => {
                nonzero(s.output_digest, "superseded output")?;
                nonzero(s.by_digest, "replacement output")
            }
            Self::Refreshed(_) => Ok(()),
        }
    }
}

/// The previous field binds the exact canonical lowercase hex line plus LF, not an ancestor or bare frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JournalRecord {
    pub previous: Digest32,
    pub entry: Entry,
}

impl JournalRecord {
    pub fn to_bytes(&self) -> Result<Vec<u8>, Refusal> {
        self.entry.validate()?;
        let mut enc = Encoder::new_frame(*b"RLYJ", 1);
        enc.put_fixed(&self.previous.0);
        encode_entry(&mut enc, &self.entry);
        let bytes = enc.finish();
        if bytes.len() > MAX_RECORD_BYTES {
            return Err(invalid("reliance frame exceeds bound"));
        }
        Ok(bytes)
    }
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, Refusal> {
        if bytes.len() > MAX_RECORD_BYTES {
            return Err(invalid("reliance frame exceeds bound"));
        }
        let mut dec = Decoder::open_frame(bytes, *b"RLYJ", 1, 1).map_err(codec)?;
        let result = Self {
            previous: Digest32(dec.get_fixed().map_err(codec)?),
            entry: decode_entry(&mut dec)?,
        };
        dec.finish().map_err(codec)?;
        if result.to_bytes()? != bytes {
            return Err(invalid("noncanonical reliance frame"));
        }
        Ok(result)
    }
    pub fn digest(&self) -> Result<Digest32, Refusal> {
        Ok(blake3_hash(&self.to_bytes()?))
    }
    pub fn chain_head(&self) -> Result<Digest32, Refusal> {
        let bytes = self.to_bytes()?;
        let mut line = Vec::with_capacity(bytes.len() * 2 + 1);
        const HEX: &[u8; 16] = b"0123456789abcdef";
        for b in bytes {
            line.push(HEX[usize::from(b >> 4)]);
            line.push(HEX[usize::from(b & 15)]);
        }
        line.push(b'\n');
        Ok(blake3_hash(&line))
    }
}

/// Not serializable and with no public fields. Possession is checked against the owner policy's key digest;
/// knowledge of an actor string or the public journal alone cannot construct it. A forged journal/header is
/// still not an authority: a consumer separately requires an independently obtained exact head pin.
/// This proves local capability possession, not a human signature or external attestation.
#[derive(Debug)]
pub struct ReleaseCapability {
    header_digest: Digest32,
}
impl ReleaseCapability {
    pub fn verify_key(secret: &[u8; 32], header: &Header) -> Result<Self, Refusal> {
        header.validate()?;
        let actual = sha256(secret);
        let mismatch = actual
            .0
            .iter()
            .zip(header.release_key_sha256.0)
            .fold(0_u8, |x, (a, b)| x | (a ^ b));
        if *secret == [0; 32] || mismatch != 0 {
            return Err(denied("release key does not authorize this journal"));
        }
        Ok(Self {
            header_digest: header.digest()?,
        })
    }
}

/// Obtained by a trusted runtime, never decoded from a record or copied from its timestamp. A caller that
/// cannot bound its clock passes None; merely saying "host time" does not establish an external time proof.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TrustedTime {
    pub now_ns: i64,
    pub uncertainty_ns: u64,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TrustContext {
    pub pinned_head: Option<Digest32>,
    pub time: Option<TrustedTime>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnknownReason {
    Unissued,
    MissingTime,
    MissingPin,
    PinMismatch,
    ClockUncertain,
    ClockRollback,
    StaleStatusCut,
    FutureAsOf,
}
impl UnknownReason {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Unissued => "unissued",
            Self::MissingTime => "trusted_time_unavailable",
            Self::MissingPin => "trusted_pin_unavailable",
            Self::PinMismatch => "trusted_head_mismatch",
            Self::ClockUncertain => "clock_uncertainty_exceeds_policy",
            Self::ClockRollback => "future_status_cut_or_clock_rollback",
            Self::StaleStatusCut => "stale_status_cut",
            Self::FutureAsOf => "future_as_of",
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RelianceStatus {
    Current,
    Withdrawn {
        reason: String,
        affected_claims: Vec<ObjectId>,
        replacement: Option<Digest32>,
    },
    Superseded {
        by: Digest32,
    },
    Unknown {
        reason: UnknownReason,
    },
}
impl RelianceStatus {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Current => "current",
            Self::Withdrawn { .. } => "withdrawn",
            Self::Superseded { .. } => "superseded",
            Self::Unknown { .. } => "unknown",
        }
    }
    pub const fn replacement(&self) -> Option<Digest32> {
        match self {
            Self::Withdrawn { replacement, .. } => *replacement,
            Self::Superseded { by } => Some(*by),
            _ => None,
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelianceCheck {
    pub status: RelianceStatus,
    pub current_reuse_allowed: bool,
    pub as_of_ns: Option<i64>,
    pub status_cut_ns: i64,
}

#[derive(Debug, Clone)]
pub struct RelianceLedger {
    header: Header,
    records: Vec<JournalRecord>,
    head: Digest32,
    bytes: usize,
    issued: BTreeMap<Digest32, Vec<usize>>,
    issuance_ids: BTreeMap<Digest32, usize>,
    statuses: BTreeMap<Digest32, Vec<usize>>,
}

impl RelianceLedger {
    pub fn from_records(records: &[JournalRecord]) -> Result<Self, Refusal> {
        let Some(JournalRecord {
            entry: Entry::Genesis(header),
            ..
        }) = records.first()
        else {
            return Err(integrity("journal must begin with its owner policy"));
        };
        let mut log = Self {
            header: header.clone(),
            records: Vec::new(),
            head: Digest32::default(),
            bytes: 0,
            issued: BTreeMap::new(),
            issuance_ids: BTreeMap::new(),
            statuses: BTreeMap::new(),
        };
        for record in records {
            log.replay(record.clone())?;
        }
        Ok(log)
    }
    pub fn head(&self) -> Digest32 {
        self.head
    }
    pub fn header(&self) -> &Header {
        &self.header
    }
    pub fn records(&self) -> &[JournalRecord] {
        &self.records
    }
    pub fn status_cut_ns(&self) -> i64 {
        self.records
            .last()
            .map_or(self.header.created_at_ns, |r| r.entry.at_ns())
    }
    pub fn issuances(&self) -> Vec<&IssuanceRecord> {
        self.records
            .iter()
            .filter_map(|r| match &r.entry {
                Entry::Issued(i) => Some(i),
                _ => None,
            })
            .collect()
    }
    /// Select one exact receipt, including its producer provenance, rather than any copy of its bytes.
    /// DESIGN: this lookup index is rebuilt from canonical records; it adds no durable fields.
    pub fn issuance(&self, id: Digest32) -> Option<&IssuanceRecord> {
        self.issuance_ids
            .get(&id)
            .and_then(|index| self.records.get(*index))
            .and_then(|record| match &record.entry {
                Entry::Issued(issuance) => Some(issuance),
                _ => None,
            })
    }
    pub fn copies(&self, digest: Digest32) -> Vec<&IssuanceRecord> {
        self.issued
            .get(&digest)
            .into_iter()
            .flatten()
            .filter_map(|i| match &self.records[*i].entry {
                Entry::Issued(r) => Some(r),
                _ => None,
            })
            .collect()
    }
    pub fn recall_revision(&self, revision: &str) -> Vec<&IssuanceRecord> {
        self.issuances()
            .into_iter()
            .filter(|i| i.code_revision == revision)
            .collect()
    }
    pub fn recall_claim(&self, claim: ObjectId) -> Vec<&IssuanceRecord> {
        self.issuances()
            .into_iter()
            .filter(|i| i.output_claims.binary_search(&claim).is_ok())
            .collect()
    }
    pub fn verify_artifact(&self, digest: Digest32, bytes: &[u8]) -> Result<(), Refusal> {
        if !self.issued.contains_key(&digest) {
            return Err(absent("artifact was never issued in this journal"));
        }
        if blake3_hash(bytes) != digest {
            return Err(Refusal::new(
                RefusalCode::DigestMismatch505,
                "issued artifact bytes do not match their recorded BLAKE3",
            ));
        }
        Ok(())
    }

    /// Validates an existing untrusted history without conferring release or freshness authority.
    pub fn replay(&mut self, record: JournalRecord) -> Result<(), Refusal> {
        let frame = record.to_bytes()?;
        let total = self
            .bytes
            .checked_add(frame.len())
            .ok_or_else(|| invalid("journal length overflow"))?;
        if self.records.len() >= MAX_RECORDS || total > MAX_JOURNAL_BYTES {
            return Err(invalid("reliance journal exceeds replay bound"));
        }
        if record.previous != self.head {
            return Err(integrity(
                "reliance previous-line hash does not match the exact head",
            ));
        }
        if record.entry.at_ns() < self.status_cut_ns() {
            return Err(integrity("reliance event time regresses"));
        }
        if let Some(role) = record.entry.role()
            && role != &self.header.release_role
        {
            return Err(denied(
                "status event is not released by the configured owner role",
            ));
        }
        self.validate_transition(&record.entry)?;
        let head = record.chain_head()?;
        let index = self.records.len();
        match &record.entry {
            Entry::Issued(i) => {
                self.issuance_ids.insert(i.digest()?, index);
                self.issued.entry(i.output_digest).or_default().push(index);
            }
            Entry::Withdrawn(w) => {
                self.statuses
                    .entry(w.output_digest)
                    .or_default()
                    .push(index);
            }
            Entry::Superseded(s) => {
                self.statuses
                    .entry(s.output_digest)
                    .or_default()
                    .push(index);
            }
            _ => {}
        }
        self.records.push(record);
        self.head = head;
        self.bytes = total;
        Ok(())
    }

    /// The I/O adapter must hold its exclusive lock and verify its expected head before calling this.
    pub fn append(
        &mut self,
        entry: Entry,
        capability: &ReleaseCapability,
        time: TrustedTime,
    ) -> Result<JournalRecord, Refusal> {
        if capability.header_digest != self.header.digest()? {
            return Err(denied("release capability belongs to another owner policy"));
        }
        if time.now_ns < 0
            || time.uncertainty_ns > self.header.max_clock_skew_ns
            || entry.at_ns() != time.now_ns
        {
            return Err(denied(
                "live append needs the bounded runtime time, never a supplied historical timestamp",
            ));
        }
        let record = JournalRecord {
            previous: self.head,
            entry,
        };
        self.replay(record.clone())?;
        Ok(record)
    }

    fn validate_transition(&self, entry: &Entry) -> Result<(), Refusal> {
        match entry {
            Entry::Genesis(h) => {
                if !self.records.is_empty() || h != &self.header {
                    return Err(frozen("owner policy cannot be replaced"));
                }
            }
            Entry::Issued(issue) => {
                let copies = self.copies(issue.output_digest);
                if let Some(first) = copies.first() {
                    if !first.same_artifact(issue) {
                        return Err(frozen(
                            "identical artifact bytes cannot acquire different Claims, anchor, lane or taint",
                        ));
                    }
                    if copies.contains(&issue) {
                        return Err(frozen("duplicate issuance record"));
                    }
                    if self.historical_status(issue.output_digest, issue.issued_at_ns)
                        != RelianceStatus::Current
                    {
                        return Err(unavailable(
                            "a withdrawn or superseded artifact cannot be reissued as current",
                        ));
                    }
                }
            }
            Entry::Withdrawn(w) => {
                let copies = self.copies(w.output_digest);
                let first = copies
                    .first()
                    .ok_or_else(|| absent("cannot withdraw an unissued output"))?;
                if w.affected_claims
                    .iter()
                    .any(|id| first.output_claims.binary_search(id).is_err())
                {
                    return Err(absent(
                        "withdrawal names a Claim not present in the issued artifact",
                    ));
                }
            }
            Entry::Superseded(s) => {
                if !self.issued.contains_key(&s.output_digest)
                    || !self.issued.contains_key(&s.by_digest)
                {
                    return Err(absent(
                        "both superseded and replacement outputs must already be issued",
                    ));
                }
                if s.output_digest == s.by_digest {
                    return Err(frozen("an output cannot supersede itself"));
                }
                if self
                    .historical_status(s.output_digest, s.at_ns)
                    .replacement()
                    .is_some()
                {
                    return Err(frozen("an issued replacement link cannot be changed"));
                }
                if self.historical_status(s.by_digest, s.at_ns) != RelianceStatus::Current {
                    return Err(unavailable(
                        "replacement is withdrawn or superseded; cycles cannot establish current reuse",
                    ));
                }
            }
            Entry::Refreshed(_) => {}
        }
        if self.records.is_empty() && !matches!(entry, Entry::Genesis(_)) {
            return Err(integrity("missing journal genesis"));
        }
        Ok(())
    }

    fn historical_status(&self, digest: Digest32, at: i64) -> RelianceStatus {
        if !self.copies(digest).iter().any(|r| r.issued_at_ns <= at) {
            return RelianceStatus::Unknown {
                reason: UnknownReason::Unissued,
            };
        }
        let mut withdrawal = None;
        let mut affected = BTreeSet::new();
        let mut replacement = None;
        for index in self.statuses.get(&digest).into_iter().flatten() {
            let entry = &self.records[*index].entry;
            if entry.at_ns() > at {
                continue;
            }
            match entry {
                Entry::Withdrawn(w) => {
                    withdrawal = Some(w.reason.clone());
                    affected.extend(w.affected_claims.iter().copied());
                }
                Entry::Superseded(s) => replacement = Some(s.by_digest),
                _ => {}
            }
        }
        if let Some(reason) = withdrawal {
            RelianceStatus::Withdrawn {
                reason,
                affected_claims: affected.into_iter().collect(),
                replacement,
            }
        } else if let Some(by) = replacement {
            RelianceStatus::Superseded { by }
        } else {
            RelianceStatus::Current
        }
    }

    pub fn check(
        &self,
        digest: Digest32,
        as_of_ns: Option<i64>,
        trust: &TrustContext,
    ) -> RelianceCheck {
        let cut = self.status_cut_ns();
        let unknown = |reason| RelianceCheck {
            status: RelianceStatus::Unknown { reason },
            current_reuse_allowed: false,
            as_of_ns,
            status_cut_ns: cut,
        };
        let Some(time) = trust.time else {
            return unknown(UnknownReason::MissingTime);
        };
        let Some(pin) = trust.pinned_head else {
            return unknown(UnknownReason::MissingPin);
        };
        if pin != self.head {
            return unknown(UnknownReason::PinMismatch);
        }
        if time.uncertainty_ns > self.header.max_clock_skew_ns {
            return unknown(UnknownReason::ClockUncertain);
        }
        if time.now_ns < cut {
            return unknown(UnknownReason::ClockRollback);
        }
        // The stored cut's actual uncertainty is not recorded separately. Its immutable owner-policy
        // bound must be included as well as the reading clock's bound (fast writer + slow reader).
        let age = (i128::from(time.now_ns) - i128::from(cut))
            + i128::from(time.uncertainty_ns)
            + i128::from(self.header.max_clock_skew_ns);
        if age > i128::from(self.header.max_status_age_ns) {
            return unknown(UnknownReason::StaleStatusCut);
        }
        let at = as_of_ns.unwrap_or(time.now_ns);
        if at > time.now_ns {
            return unknown(UnknownReason::FutureAsOf);
        }
        let status = self.historical_status(digest, at);
        let current_reuse_allowed = as_of_ns.is_none() && status == RelianceStatus::Current;
        RelianceCheck {
            status,
            current_reuse_allowed,
            as_of_ns: Some(at),
            status_cut_ns: cut,
        }
    }

    pub fn gate_current_reuse(
        &self,
        digest: Digest32,
        trust: &TrustContext,
    ) -> Result<(), Refusal> {
        let check = self.check(digest, None, trust);
        if check.current_reuse_allowed {
            return Ok(());
        }
        let next = check.status.replacement().map_or_else(
            || "request fresh trusted status before reuse".to_string(),
            |d| format!("inspect replacement {d} and check its own status"),
        );
        Err(unavailable(format!(
            "output {digest} is {}; {next}",
            check.status.as_str()
        )))
    }
}

fn invalid(detail: impl Into<String>) -> Refusal {
    Refusal::new(RefusalCode::UndeclaredUnitBasisOrTz107, detail)
}
fn integrity(detail: impl Into<String>) -> Refusal {
    Refusal::new(RefusalCode::StaleAnchor502, detail)
}
fn denied(detail: impl Into<String>) -> Refusal {
    Refusal::new(RefusalCode::MissingHumanToken304, detail)
}
fn frozen(detail: impl Into<String>) -> Refusal {
    Refusal::new(RefusalCode::FrozenRecordMutation507, detail)
}
fn absent(detail: impl Into<String>) -> Refusal {
    Refusal::new(RefusalCode::NoClaimInDomain201, detail)
}
fn unavailable(detail: impl Into<String>) -> Refusal {
    Refusal::new(RefusalCode::NotObservable202, detail)
}
fn codec(error: impl fmt::Display) -> Refusal {
    integrity(format!("reliance codec: {error}"))
}
fn text(s: &str) -> Result<(), Refusal> {
    if s.trim().is_empty() || s.len() > MAX_TEXT || s.chars().any(char::is_control) {
        Err(invalid(
            "text must be nonempty, bounded and free of control characters",
        ))
    } else {
        Ok(())
    }
}
fn nonzero(d: Digest32, name: &str) -> Result<(), Refusal> {
    if d == Digest32::default() {
        Err(invalid(format!("{name} must not be an empty digest")))
    } else {
        Ok(())
    }
}
fn claims(ids: &[ObjectId]) -> Result<(), Refusal> {
    if ids.is_empty()
        || ids.len() > MAX_CLAIMS
        || ids
            .iter()
            .any(|id| id.kind != PrefixKind::Claim || id.bytes == [0; 16])
        || ids.windows(2).any(|w| w[0] >= w[1])
    {
        Err(invalid(
            "Claim ids must be nonempty, bounded, unique and sorted canonical clm_ ids",
        ))
    } else {
        Ok(())
    }
}
fn encode_claims(enc: &mut Encoder, ids: &[ObjectId]) {
    enc.put_u32(ids.len() as u32);
    for id in ids {
        enc.put_fixed(&id.bytes);
    }
}
fn decode_claims(dec: &mut Decoder<'_>) -> Result<Vec<ObjectId>, Refusal> {
    let n = dec.get_u32().map_err(codec)? as usize;
    if n == 0 || n > MAX_CLAIMS {
        return Err(invalid("Claim count outside bounded frame"));
    }
    let mut ids = Vec::with_capacity(n);
    for _ in 0..n {
        ids.push(ObjectId::from_raw(
            PrefixKind::Claim,
            dec.get_fixed().map_err(codec)?,
        ));
    }
    claims(&ids)?;
    Ok(ids)
}
fn encode_header(enc: &mut Encoder, h: &Header) {
    enc.put_str(&h.release_role.to_string());
    enc.put_fixed(&h.release_key_sha256.0);
    enc.put_u64(h.max_status_age_ns);
    enc.put_u64(h.max_clock_skew_ns);
    enc.put_i64(h.created_at_ns);
}
fn decode_header(dec: &mut Decoder<'_>) -> Result<Header, Refusal> {
    Ok(Header {
        release_role: RoleId::parse(dec.get_str(MAX_TEXT).map_err(codec)?)?,
        release_key_sha256: Digest32(dec.get_fixed().map_err(codec)?),
        max_status_age_ns: dec.get_u64().map_err(codec)?,
        max_clock_skew_ns: dec.get_u64().map_err(codec)?,
        created_at_ns: dec.get_i64().map_err(codec)?,
    })
}
fn encode_issue(enc: &mut Encoder, i: &IssuanceRecord) {
    enc.put_fixed(&i.output_digest.0);
    encode_claims(enc, &i.output_claims);
    enc.put_bytes(&i.anchor.to_bytes());
    enc.put_str(&i.code_revision);
    enc.put_fixed(&i.binary_digest.0);
    enc.put_fixed(&i.input_manifest_root.0);
    enc.put_u8(match i.lane {
        IssuanceLane::Public => 0,
        IssuanceLane::Synthetic => 1,
    });
    enc.put_u8(0); // Only Public is admitted; private labels cannot be silently decoded as Public.
    enc.put_u8(u8::from(i.public_taint));
    enc.put_str(&i.recipient_role.to_string());
    enc.put_str(&i.purpose);
    enc.put_i64(i.issued_at_ns);
}
fn decode_issue(dec: &mut Decoder<'_>) -> Result<IssuanceRecord, Refusal> {
    let output_digest = Digest32(dec.get_fixed().map_err(codec)?);
    let output_claims = decode_claims(dec)?;
    let anchor = LedgerAnchor::from_bytes(dec.get_bytes(1024).map_err(codec)?).map_err(codec)?;
    let code_revision = dec.get_str(MAX_TEXT).map_err(codec)?.to_string();
    let binary_digest = Digest32(dec.get_fixed().map_err(codec)?);
    let input_manifest_root = Digest32(dec.get_fixed().map_err(codec)?);
    let lane = match dec.get_u8().map_err(codec)? {
        0 => IssuanceLane::Public,
        1 => IssuanceLane::Synthetic,
        _ => return Err(invalid("unknown reliance lane")),
    };
    if dec.get_u8().map_err(codec)? != 0 {
        return Err(Refusal::new(
            RefusalCode::MnpiWall301,
            "reliance public export cannot decode a private label",
        ));
    }
    let public_taint = match dec.get_u8().map_err(codec)? {
        0 => false,
        1 => true,
        _ => return Err(invalid("noncanonical public taint boolean")),
    };
    Ok(IssuanceRecord {
        output_digest,
        output_claims,
        anchor,
        code_revision,
        binary_digest,
        input_manifest_root,
        lane,
        confidentiality: Confidentiality::public(),
        public_taint,
        recipient_role: RoleId::parse(dec.get_str(MAX_TEXT).map_err(codec)?)?,
        purpose: dec.get_str(MAX_TEXT).map_err(codec)?.to_string(),
        issued_at_ns: dec.get_i64().map_err(codec)?,
    })
}
fn encode_entry(enc: &mut Encoder, entry: &Entry) {
    match entry {
        Entry::Genesis(h) => {
            enc.put_u8(0);
            encode_header(enc, h);
        }
        Entry::Issued(i) => {
            enc.put_u8(1);
            encode_issue(enc, i);
        }
        Entry::Withdrawn(w) => {
            enc.put_u8(2);
            enc.put_fixed(&w.output_digest.0);
            enc.put_str(&w.reason);
            encode_claims(enc, &w.affected_claims);
            enc.put_u8(match w.defect_class {
                DefectClass::CodeDefect => 0,
                DefectClass::SourceRestated => 1,
                DefectClass::ScopeError => 2,
            });
            enc.put_str(&w.by_role.to_string());
            enc.put_i64(w.at_ns);
        }
        Entry::Superseded(s) => {
            enc.put_u8(3);
            enc.put_fixed(&s.output_digest.0);
            enc.put_fixed(&s.by_digest.0);
            enc.put_str(&s.by_role.to_string());
            enc.put_i64(s.at_ns);
        }
        Entry::Refreshed(r) => {
            enc.put_u8(4);
            enc.put_str(&r.by_role.to_string());
            enc.put_i64(r.at_ns);
        }
    }
}
fn decode_entry(dec: &mut Decoder<'_>) -> Result<Entry, Refusal> {
    Ok(match dec.get_u8().map_err(codec)? {
        0 => Entry::Genesis(decode_header(dec)?),
        1 => Entry::Issued(decode_issue(dec)?),
        2 => {
            let output_digest = Digest32(dec.get_fixed().map_err(codec)?);
            let reason = dec.get_str(MAX_TEXT).map_err(codec)?.to_string();
            let affected_claims = decode_claims(dec)?;
            let defect_class = match dec.get_u8().map_err(codec)? {
                0 => DefectClass::CodeDefect,
                1 => DefectClass::SourceRestated,
                2 => DefectClass::ScopeError,
                _ => return Err(invalid("unknown withdrawal defect class")),
            };
            Entry::Withdrawn(Withdrawal {
                output_digest,
                reason,
                affected_claims,
                defect_class,
                by_role: RoleId::parse(dec.get_str(MAX_TEXT).map_err(codec)?)?,
                at_ns: dec.get_i64().map_err(codec)?,
            })
        }
        3 => Entry::Superseded(Supersession {
            output_digest: Digest32(dec.get_fixed().map_err(codec)?),
            by_digest: Digest32(dec.get_fixed().map_err(codec)?),
            by_role: RoleId::parse(dec.get_str(MAX_TEXT).map_err(codec)?)?,
            at_ns: dec.get_i64().map_err(codec)?,
        }),
        4 => Entry::Refreshed(StatusRefresh {
            by_role: RoleId::parse(dec.get_str(MAX_TEXT).map_err(codec)?)?,
            at_ns: dec.get_i64().map_err(codec)?,
        }),
        _ => return Err(invalid("unknown reliance event tag")),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEY: [u8; 32] = [7; 32];
    fn claim(n: u8) -> ObjectId {
        ObjectId::from_raw(PrefixKind::Claim, [n; 16])
    }
    fn time(now_ns: i64) -> TrustedTime {
        TrustedTime {
            now_ns,
            uncertainty_ns: 5,
        }
    }
    fn setup() -> (RelianceLedger, ReleaseCapability) {
        let h = Header {
            release_role: RoleId::ReleaseOwner,
            release_key_sha256: sha256(&KEY),
            max_status_age_ns: 1000,
            max_clock_skew_ns: 10,
            created_at_ns: 100,
        };
        let capability = ReleaseCapability::verify_key(&KEY, &h).expect("owner capability");
        let log = RelianceLedger::from_records(&[JournalRecord {
            previous: Digest32::default(),
            entry: Entry::Genesis(h),
        }])
        .expect("genesis");
        (log, capability)
    }
    fn issue(bytes: &[u8], at: i64) -> IssuanceRecord {
        IssuanceRecord {
            output_digest: blake3_hash(bytes),
            output_claims: vec![claim(1), claim(2)],
            anchor: LedgerAnchor {
                format_version: 1,
                cell_id: "CELL-SYNTHETIC".into(),
                ledger_epoch: 1,
                commit_sequence: 42,
                adapter_registry_epoch: 1,
                schema_epoch: 1,
                policy_epoch: 1,
                privacy_epoch: 1,
                residency_epoch: 1,
                state_root: blake3_hash(b"unchanged synthetic source state"),
            },
            code_revision: "synthetic-revision-1".into(),
            binary_digest: blake3_hash(b"binary-1"),
            input_manifest_root: blake3_hash(b"unchanged synthetic input manifest"),
            lane: IssuanceLane::Synthetic,
            confidentiality: Confidentiality::public(),
            public_taint: false,
            recipient_role: RoleId::Sponsor,
            purpose: "synthetic internal review".into(),
            issued_at_ns: at,
        }
    }
    fn trust(log: &RelianceLedger, at: i64) -> TrustContext {
        TrustContext {
            pinned_head: Some(log.head()),
            time: Some(time(at)),
        }
    }
    fn withdraw(digest: Digest32, at: i64) -> Entry {
        Entry::Withdrawn(Withdrawal {
            output_digest: digest,
            reason: "synthetic revision substituted generated inputs".into(),
            affected_claims: vec![claim(1)],
            defect_class: DefectClass::CodeDefect,
            by_role: RoleId::ReleaseOwner,
            at_ns: at,
        })
    }
    fn replace(output: Digest32, by: Digest32, at: i64) -> Entry {
        Entry::Superseded(Supersession {
            output_digest: output,
            by_digest: by,
            by_role: RoleId::ReleaseOwner,
            at_ns: at,
        })
    }

    #[test]
    fn issuance_identity_selects_exact_provenance_and_is_rebuilt_on_replay() {
        let (mut log, cap) = setup();
        let first = issue(b"same synthetic bytes", 110);
        let mut second = first.clone();
        second.code_revision = "synthetic-revision-2".into();
        second.binary_digest = blake3_hash(b"binary-2");
        second.recipient_role = RoleId::ReviewerPool(2);
        second.issued_at_ns = 120;
        let first_id = first.digest().expect("first receipt");
        let second_id = second.digest().expect("second receipt");
        assert_ne!(first_id, second_id);
        assert_ne!(first_id, first.output_digest);
        for issuance in [&first, &second] {
            log.append(
                Entry::Issued(issuance.clone()),
                &cap,
                time(issuance.issued_at_ns),
            )
            .expect("issue exact provenance");
        }
        assert_eq!(log.issuance(first_id), Some(&first));
        assert_eq!(log.issuance(second_id), Some(&second));
        assert!(log.issuance(first.output_digest).is_none());
        assert!(log.issuance(Digest32::default()).is_none());
        let frames = log
            .records()
            .iter()
            .map(|record| {
                JournalRecord::from_bytes(&record.to_bytes().expect("canonical frame"))
                    .expect("decode unchanged format")
            })
            .collect::<Vec<_>>();
        let rebuilt = RelianceLedger::from_records(&frames).expect("rebuild lookup from frames");
        assert_eq!(rebuilt.head(), log.head());
        assert_eq!(rebuilt.issuance(first_id), Some(&first));
        assert_eq!(rebuilt.issuance(second_id), Some(&second));
        assert_eq!(rebuilt.recall_revision(&first.code_revision), vec![&first]);
        assert_eq!(
            rebuilt.recall_revision(&second.code_revision),
            vec![&second]
        );
        let head = log.head();
        assert!(
            log.append(Entry::Issued(second.clone()), &cap, time(120))
                .is_err()
        );
        assert_eq!(log.head(), head);
        assert_eq!(log.issuance(first_id), Some(&first));
        assert_eq!(log.issuance(second_id), Some(&second));
        assert_eq!(log.records().len(), 3);
    }

    #[test]
    fn withdrawn_current_reuse_refuses_while_original_bytes_and_anchor_still_verify() {
        let (mut log, cap) = setup();
        let bytes = b"invented synthetic report, never labelled as an issuer";
        let record = issue(bytes, 110);
        let output = record.output_digest;
        let anchor = record.anchor.clone();
        log.append(Entry::Issued(record), &cap, time(110))
            .expect("issue");
        log.gate_current_reuse(output, &trust(&log, 115))
            .expect("current reuse");
        log.append(withdraw(output, 120), &cap, time(120))
            .expect("withdraw");
        let checked = log.check(output, None, &trust(&log, 125));
        assert!(
            matches!(checked.status, RelianceStatus::Withdrawn { affected_claims, replacement: None, .. } if affected_claims == vec![claim(1)])
        );
        assert!(!checked.current_reuse_allowed);
        assert_eq!(
            log.gate_current_reuse(output, &trust(&log, 125))
                .expect_err("withdrawn reuse")
                .code,
            "FMINE-REF-202"
        );
        log.verify_artifact(output, bytes)
            .expect("historical integrity");
        assert_eq!(log.copies(output)[0].anchor, anchor);
        assert_eq!(
            log.verify_artifact(output, b"edited")
                .expect_err("edited artifact")
                .code,
            "FMINE-REF-505"
        );
        let historical = log.check(output, Some(115), &trust(&log, 125));
        assert_eq!(historical.status, RelianceStatus::Current);
        assert!(
            !historical.current_reuse_allowed,
            "historical status cannot grant current copying"
        );
        let head = log.head();
        let mut copy = issue(bytes, 130);
        copy.recipient_role = RoleId::ReviewerPool(1);
        assert_eq!(
            log.append(Entry::Issued(copy), &cap, time(130))
                .expect_err("no resurrection")
                .code,
            "FMINE-REF-202"
        );
        assert_eq!(log.head(), head);
    }

    #[test]
    fn offline_stale_future_and_replayed_status_never_become_current() {
        let (mut log, cap) = setup();
        let old_pin = log.head();
        let record = issue(b"synthetic", 200);
        let output = record.output_digest;
        log.append(Entry::Issued(record), &cap, time(200))
            .expect("issue");
        for (ctx, expected) in [
            (TrustContext::default(), UnknownReason::MissingTime),
            (
                TrustContext {
                    pinned_head: None,
                    time: Some(time(210)),
                },
                UnknownReason::MissingPin,
            ),
            (
                TrustContext {
                    pinned_head: Some(old_pin),
                    time: Some(time(210)),
                },
                UnknownReason::PinMismatch,
            ),
            (trust(&log, 199), UnknownReason::ClockRollback),
            (
                TrustContext {
                    pinned_head: Some(log.head()),
                    time: Some(TrustedTime {
                        now_ns: 210,
                        uncertainty_ns: 11,
                    }),
                },
                UnknownReason::ClockUncertain,
            ),
            (trust(&log, 1186), UnknownReason::StaleStatusCut),
        ] {
            assert_eq!(
                log.check(output, None, &ctx).status,
                RelianceStatus::Unknown { reason: expected }
            );
            assert_eq!(
                log.gate_current_reuse(output, &ctx)
                    .expect_err("unknown is not permission")
                    .code,
                "FMINE-REF-202"
            );
        }
        // Exact worst-case boundary includes fast issuing clock (+10) and slow reading clock (+5).
        assert_eq!(
            log.check(output, None, &trust(&log, 1185)).status,
            RelianceStatus::Current
        );
        let replay = RelianceLedger::from_records(log.records()).expect("replay");
        assert_eq!(
            replay.check(output, None, &trust(&replay, 1186)).status,
            RelianceStatus::Unknown {
                reason: UnknownReason::StaleStatusCut
            }
        );
        assert_eq!(
            log.check(output, Some(1190), &trust(&log, 210)).status,
            RelianceStatus::Unknown {
                reason: UnknownReason::FutureAsOf
            }
        );
        assert_eq!(
            log.check(blake3_hash(b"never issued"), None, &trust(&log, 210))
                .status,
            RelianceStatus::Unknown {
                reason: UnknownReason::Unissued
            }
        );
        log.append(
            Entry::Refreshed(StatusRefresh {
                by_role: RoleId::ReleaseOwner,
                at_ns: 1200,
            }),
            &cap,
            time(1200),
        )
        .expect("explicit authorized status refresh");
        log.gate_current_reuse(output, &trust(&log, 1205))
            .expect("fresh persisted cut");
        assert_eq!(
            log.check(
                output,
                None,
                &TrustContext {
                    pinned_head: Some(replay.head()),
                    time: Some(time(1205))
                }
            )
            .status,
            RelianceStatus::Unknown {
                reason: UnknownReason::PinMismatch
            }
        );
    }

    #[test]
    fn replacement_must_exist_and_cannot_form_a_cycle_or_resurrect_either_output() {
        let (mut log, cap) = setup();
        let a = issue(b"synthetic old", 110);
        let b = issue(b"synthetic replacement", 120);
        log.append(Entry::Issued(a.clone()), &cap, time(110))
            .expect("old");
        assert_eq!(
            log.append(
                replace(a.output_digest, b.output_digest, 115),
                &cap,
                time(115)
            )
            .expect_err("unissued replacement")
            .code,
            "FMINE-REF-201"
        );
        log.append(Entry::Issued(b.clone()), &cap, time(120))
            .expect("replacement");
        assert_eq!(
            log.append(
                replace(a.output_digest, a.output_digest, 125),
                &cap,
                time(125)
            )
            .expect_err("self link")
            .code,
            "FMINE-REF-507"
        );
        log.append(
            replace(a.output_digest, b.output_digest, 130),
            &cap,
            time(130),
        )
        .expect("supersede");
        assert_eq!(
            log.check(a.output_digest, None, &trust(&log, 130)).status,
            RelianceStatus::Superseded {
                by: b.output_digest
            }
        );
        assert_eq!(
            log.append(
                replace(b.output_digest, a.output_digest, 135),
                &cap,
                time(135)
            )
            .expect_err("cycle")
            .code,
            "FMINE-REF-202"
        );
        log.append(withdraw(b.output_digest, 140), &cap, time(140))
            .expect("withdraw replacement");
        assert!(
            log.gate_current_reuse(a.output_digest, &trust(&log, 145))
                .is_err()
        );
        assert!(
            log.gate_current_reuse(b.output_digest, &trust(&log, 145))
                .is_err()
        );
        log.verify_artifact(a.output_digest, b"synthetic old")
            .expect("old integrity");
        log.verify_artifact(b.output_digest, b"synthetic replacement")
            .expect("replacement integrity");
        let c = issue(b"synthetic corrected again", 150);
        log.append(Entry::Issued(c.clone()), &cap, time(150))
            .expect("new replacement");
        log.append(
            replace(b.output_digest, c.output_digest, 160),
            &cap,
            time(160),
        )
        .expect("replacement for withdrawal");
        assert!(
            matches!(log.check(b.output_digest, None, &trust(&log, 165)).status, RelianceStatus::Withdrawn { replacement: Some(d), .. } if d == c.output_digest)
        );
    }

    #[test]
    fn capabilities_clock_and_affected_claims_are_checked_before_mutation() {
        let (mut log, cap) = setup();
        assert_eq!(
            ReleaseCapability::verify_key(&[9; 32], log.header())
                .expect_err("wrong key")
                .code,
            "FMINE-REF-304"
        );
        let mut other = log.header().clone();
        other.max_status_age_ns += 1;
        let other_cap =
            ReleaseCapability::verify_key(&KEY, &other).expect("other policy capability");
        let record = issue(b"synthetic", 110);
        assert_eq!(
            log.append(Entry::Issued(record.clone()), &other_cap, time(110))
                .expect_err("wrong policy")
                .code,
            "FMINE-REF-304"
        );
        assert_eq!(
            log.append(Entry::Issued(record.clone()), &cap, time(111))
                .expect_err("supplied backdate")
                .code,
            "FMINE-REF-304"
        );
        log.append(Entry::Issued(record.clone()), &cap, time(110))
            .expect("issue");
        let head = log.head();
        let bad = Withdrawal {
            output_digest: record.output_digest,
            reason: "synthetic defect".into(),
            affected_claims: vec![claim(3)],
            defect_class: DefectClass::ScopeError,
            by_role: RoleId::ReleaseOwner,
            at_ns: 120,
        };
        assert_eq!(
            log.append(Entry::Withdrawn(bad), &cap, time(120))
                .expect_err("unaffected claim")
                .code,
            "FMINE-REF-201"
        );
        assert_eq!(
            log.append(
                Entry::Refreshed(StatusRefresh {
                    by_role: RoleId::Sponsor,
                    at_ns: 120
                }),
                &cap,
                time(120)
            )
            .expect_err("actor text is not authority")
            .code,
            "FMINE-REF-304"
        );
        assert_eq!(
            log.append(
                Entry::Refreshed(StatusRefresh {
                    by_role: RoleId::ReleaseOwner,
                    at_ns: 109
                }),
                &cap,
                time(109)
            )
            .expect_err("clock rollback")
            .code,
            "FMINE-REF-502"
        );
        assert_eq!(log.head(), head);
        assert_eq!(log.records().len(), 2);
    }

    #[test]
    fn exact_recall_keeps_recipient_copies_and_distinct_builds_of_identical_bytes() {
        let (mut log, cap) = setup();
        let mut expected = Vec::new();
        let mut seed = 91_u64;
        for n in 0..96 {
            seed = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
            let revision = 1 + seed % 3;
            let mut record = issue(format!("synthetic output {}", n / 2).as_bytes(), 110 + n);
            record.code_revision = format!("synthetic-revision-{revision}");
            record.binary_digest = blake3_hash(record.code_revision.as_bytes());
            record.output_claims = vec![claim(1 + (n / 2 % 5) as u8)];
            record.recipient_role = if n % 2 == 0 {
                RoleId::Sponsor
            } else {
                RoleId::ReviewerPool(1)
            };
            expected.push(record.clone());
            log.append(Entry::Issued(record), &cap, time(110 + n))
                .expect("recipient copy");
        }
        for revision in 1..=3 {
            let key = format!("synthetic-revision-{revision}");
            assert_eq!(
                log.recall_revision(&key),
                expected
                    .iter()
                    .filter(|i| i.code_revision == key)
                    .collect::<Vec<_>>()
            );
        }
        for id in 1..=6 {
            assert_eq!(
                log.recall_claim(claim(id)),
                expected
                    .iter()
                    .filter(|i| i.output_claims.contains(&claim(id)))
                    .collect::<Vec<_>>()
            );
        }
        let defective: BTreeSet<_> = log
            .recall_revision("synthetic-revision-2")
            .iter()
            .map(|i| i.output_digest)
            .collect();
        for (n, digest) in defective.iter().enumerate() {
            let affected_claims = log.copies(*digest)[0].output_claims.clone();
            log.append(
                Entry::Withdrawn(Withdrawal {
                    output_digest: *digest,
                    affected_claims,
                    reason: "synthetic code defect; source bytes unchanged".into(),
                    defect_class: DefectClass::CodeDefect,
                    by_role: RoleId::ReleaseOwner,
                    at_ns: 300 + n as i64,
                }),
                &cap,
                time(300 + n as i64),
            )
            .expect("recall defect");
        }
        assert_eq!(log.issuances().len(), 96);
        for record in &expected {
            assert_eq!(
                log.gate_current_reuse(record.output_digest, &trust(&log, 400))
                    .is_err(),
                defective.contains(&record.output_digest)
            );
            assert_eq!(record.input_manifest_root, expected[0].input_manifest_root);
            assert_eq!(record.anchor, expected[0].anchor);
        }
    }

    #[test]
    fn bounded_codec_preserves_all_bindings_and_tampering_does_not_create_trust() {
        let (mut log, cap) = setup();
        let record = issue(b"synthetic codec", 110);
        log.append(Entry::Issued(record.clone()), &cap, time(110))
            .expect("issue");
        log.append(withdraw(record.output_digest, 120), &cap, time(120))
            .expect("withdraw");
        for row in log.records() {
            let bytes = row.to_bytes().expect("canonical frame");
            assert_eq!(JournalRecord::from_bytes(&bytes).expect("decode"), *row);
            let line = bytes.iter().map(|b| format!("{b:02x}")).collect::<String>() + "\n";
            assert_eq!(
                row.chain_head().expect("line digest"),
                blake3_hash(line.as_bytes())
            );
            assert_ne!(
                row.digest().expect("frame digest"),
                row.chain_head().expect("line digest")
            );
            for n in 0..bytes.len() {
                let mut tampered = bytes.clone();
                tampered[n] ^= 1;
                assert_eq!(
                    JournalRecord::from_bytes(&tampered)
                        .expect_err("changed frame byte")
                        .code,
                    "FMINE-REF-502"
                );
            }
        }
        let mut reordered = log.records().to_vec();
        reordered.swap(1, 2);
        assert!(RelianceLedger::from_records(&reordered).is_err());
        let trusted = trust(&log, 125);
        let mut forged = log.records().to_vec();
        forged[2].entry = Entry::Refreshed(StatusRefresh {
            by_role: RoleId::ReleaseOwner,
            at_ns: 120,
        });
        let forged = RelianceLedger::from_records(&forged)
            .expect("self-consistent forged history is not authority");
        assert_eq!(
            forged.check(record.output_digest, None, &trusted).status,
            RelianceStatus::Unknown {
                reason: UnknownReason::PinMismatch
            }
        );
        assert!(
            forged
                .gate_current_reuse(record.output_digest, &TrustContext::default())
                .is_err()
        );
        let mut large = record.clone();
        large.output_claims = (1_u128..=32_273)
            .map(|n| ObjectId::from_raw(PrefixKind::Claim, n.to_be_bytes()))
            .collect();
        let row = JournalRecord {
            previous: log.head(),
            entry: Entry::Issued(large),
        };
        assert_eq!(
            JournalRecord::from_bytes(&row.to_bytes().expect("full corpus Claim count"))
                .expect("large bounded decode"),
            row
        );
        for bad in [
            vec![],
            vec![claim(1), claim(1)],
            vec![claim(2), claim(1)],
            vec![ObjectId::from_raw(PrefixKind::Anchor, [1; 16])],
        ] {
            let mut invalid = record.clone();
            invalid.output_claims = bad;
            assert!(invalid.digest().is_err());
        }
        let mut private = record.clone();
        private.confidentiality = Confidentiality::internal();
        assert_eq!(
            private.digest().expect_err("no declassification").code,
            "FMINE-REF-301"
        );
        for role in [
            "Alice",
            "alice@example.com",
            "reviewer-pool-Alice",
            "reviewer-pool-01",
            "agent-session:Alice",
        ] {
            assert!(RoleId::parse(role).is_err(), "{role}");
        }
        for role in [
            RoleId::Sponsor,
            RoleId::ReleaseOwner,
            RoleId::ReviewerPool(7),
            RoleId::AgentSession([0xab; 16]),
        ] {
            assert_eq!(
                RoleId::parse(&role.to_string()).expect("generic role"),
                role
            );
        }
    }
}
