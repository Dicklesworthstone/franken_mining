//! Append-only `FMT-SEG-1` segment files (`BD-082`, plan §26.5, L1 production persistence).
//!
//! Layout on disk:
//! 1. `len: u32 (LE)` + Segment Header frame (`FMNE` + `SEGH` + version + `cell_id` + `first_seq` +
//!    `prev_segment_head` + `prev_mmr_peaks` + CRC32C).
//! 2. Zero or more `len: u32 (LE)` + `FMT-LEDGER-1` (`FMNE` + `LDGR` + ... + CRC32C) batch frames
//!    chaining `seq`, `prev_hash`, and monotonic fencing `epoch`.
//! 3. Optional sealed `len: u32 (LE)` + Segment Footer frame (`FMNE` + `SEGF` + version + counts +
//!    `head_hash` + `mmr_root` + `mmr_peaks` + CRC32C).

use std::collections::BTreeSet;
use std::fs::{File, OpenOptions, TryLockError};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;

use fmine_core::codec::{CodecError, Decoder, Encoder, MAX_FRAME_BYTES, crc32c};
use fmine_core::hash::{Digest32, sha256};

use crate::batch::{
    DeltaBatch, DeltaKind, GENESIS_PREV_HASH, LedgerError, MAX_ID_BYTES, TAG_LEDGER_BATCH, Witness,
    WitnessKind,
};
use crate::cell::AdmitOutcome;
use crate::l0::{CellSnapshot, L0Error, L0Ledger};
use crate::mmr::{Mmr, root_from_peaks};

/// Format tag for `FMT-SEG-1` segment header frames (`FMNE` + `SEGH`).
pub const TAG_SEGMENT_HEADER: [u8; 4] = *b"SEGH";

/// Format tag for `FMT-SEG-1` sealed segment footer frames (`FMNE` + `SEGF`).
pub const TAG_SEGMENT_FOOTER: [u8; 4] = *b"SEGF";

/// Current schema version for `FMT-SEG-1`.
pub const FMT_SEG_V1: u16 = 1;

/// Minimum supported schema version (`N-1` compatibility).
pub const FMT_SEG_MIN_VERSION: u16 = 0;

/// Maximum number of MMR peaks carried in a segment header or footer (64 bits cover `u64::MAX`).
pub const MAX_MMR_PEAKS: usize = 64;

/// Maximum segment file size loaded in a single bounded read (256 MiB).
pub const MAX_SEGMENT_BYTES: u64 = 256 * 1024 * 1024;

/// Advisory-locked file handle that explicitly unlocks on drop so a concurrently forked child
/// cannot pin the open file description's POSIX lock past this handle's lifetime.
#[derive(Debug)]
pub(crate) struct LockedFile(File);

impl LockedFile {
    pub(crate) fn acquire(file: File, exclusive: bool) -> Result<Self, LedgerError> {
        let res = if exclusive {
            file.try_lock()
        } else {
            file.try_lock_shared()
        };
        match res {
            Ok(()) => Ok(Self(file)),
            Err(TryLockError::WouldBlock) => Err(LedgerError::Io {
                operation: "lock",
                detail: "segment file is locked by another handle".to_string(),
            }),
            Err(TryLockError::Error(err)) => Err(LedgerError::Io {
                operation: "lock",
                detail: err.to_string(),
            }),
        }
    }

    pub(crate) fn file_mut(&mut self) -> &mut File {
        &mut self.0
    }
}

impl Drop for LockedFile {
    fn drop(&mut self) {
        let _ = self.0.unlock();
    }
}

/// Canonical `FMT-SEG-1` segment header (`FMNE` + `SEGH`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SegmentHeader {
    /// Format version (`1` for `FMT-SEG-1`; `0` for N-1 compatibility).
    pub format_version: u16,
    /// Authority cell owning this segment.
    pub cell_id: String,
    /// Sequence number of the first batch in this segment (`0` for genesis segment).
    pub first_seq: u64,
    /// Expected `prev_hash` of `first_seq` (`GENESIS_PREV_HASH` when `first_seq == 0`).
    pub prev_segment_head: Digest32,
    /// MMR peaks before `first_seq` (`v1` only; empty for genesis segment).
    pub prev_mmr_peaks: Vec<Digest32>,
}

impl SegmentHeader {
    /// Validate header invariants.
    pub fn validate(&self) -> Result<(), LedgerError> {
        if self.format_version > FMT_SEG_V1 {
            return Err(LedgerError::Codec(CodecError::UnsupportedVersion {
                actual: self.format_version,
                min_supported: FMT_SEG_MIN_VERSION,
                max_supported: FMT_SEG_V1,
            }));
        }
        if self.cell_id.is_empty() || self.cell_id.len() > MAX_ID_BYTES {
            return Err(LedgerError::InvalidField {
                field: "segment_header.cell_id",
                detail: format!("length {} outside 1..={MAX_ID_BYTES}", self.cell_id.len()),
            });
        }
        if self.first_seq == 0 && self.prev_segment_head != GENESIS_PREV_HASH {
            return Err(LedgerError::ChainContinuity {
                detail: "genesis segment (first_seq=0) must have zero prev_segment_head"
                    .to_string(),
            });
        }
        if self.first_seq > 0 && self.prev_segment_head == GENESIS_PREV_HASH {
            return Err(LedgerError::ChainContinuity {
                detail: format!(
                    "continuation segment (first_seq={}) must carry non-zero prev_segment_head",
                    self.first_seq
                ),
            });
        }
        if self.prev_mmr_peaks.len() > MAX_MMR_PEAKS {
            return Err(LedgerError::Codec(CodecError::LengthOutOfBounds {
                len: self.prev_mmr_peaks.len(),
                max: MAX_MMR_PEAKS,
            }));
        }
        if self.format_version == 0 && !self.prev_mmr_peaks.is_empty() {
            return Err(LedgerError::InvalidField {
                field: "segment_header.prev_mmr_peaks",
                detail: "FMT-SEG v0 does not carry prev_mmr_peaks".to_string(),
            });
        }
        Ok(())
    }

    /// Encode this header as a canonical `FMNE` + `SEGH` + CRC32C frame.
    pub fn to_bytes(&self) -> Result<Vec<u8>, LedgerError> {
        self.validate()?;
        let mut enc = Encoder::new_frame(TAG_SEGMENT_HEADER, self.format_version);
        enc.put_str(&self.cell_id);
        enc.put_u64(self.first_seq);
        enc.put_fixed(self.prev_segment_head.as_bytes());
        if self.format_version >= 1 {
            let count = u32::try_from(self.prev_mmr_peaks.len()).map_err(|_| {
                LedgerError::Codec(CodecError::LengthOutOfBounds {
                    len: self.prev_mmr_peaks.len(),
                    max: MAX_MMR_PEAKS,
                })
            })?;
            enc.put_u32(count);
            for peak in &self.prev_mmr_peaks {
                enc.put_fixed(peak.as_bytes());
            }
        }
        Ok(enc.finish())
    }

    /// Decode an `FMNE` + `SEGH` frame.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, LedgerError> {
        let mut dec =
            Decoder::open_frame(bytes, TAG_SEGMENT_HEADER, FMT_SEG_MIN_VERSION, FMT_SEG_V1)?;
        let format_version = dec.version();
        let cell_id = dec.get_str(MAX_ID_BYTES)?.to_string();
        let first_seq = dec.get_u64()?;
        let prev_segment_head = Digest32(dec.get_fixed::<32>()?);
        let mut prev_mmr_peaks = Vec::new();
        if format_version >= 1 {
            let count = dec.get_u32()? as usize;
            if count > MAX_MMR_PEAKS {
                return Err(LedgerError::Codec(CodecError::LengthOutOfBounds {
                    len: count,
                    max: MAX_MMR_PEAKS,
                }));
            }
            prev_mmr_peaks.reserve(count);
            for _ in 0..count {
                prev_mmr_peaks.push(Digest32(dec.get_fixed::<32>()?));
            }
        }
        dec.finish()?;
        let header = Self {
            format_version,
            cell_id,
            first_seq,
            prev_segment_head,
            prev_mmr_peaks,
        };
        header.validate()?;
        Ok(header)
    }
}

/// Canonical `FMT-SEG-1` sealed segment footer (`FMNE` + `SEGF`) carrying the MMR peak list (`BD-082`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SegmentFooter {
    /// Format version (`1` for `FMT-SEG-1`).
    pub format_version: u16,
    /// Number of batch records in this segment.
    pub record_count: u64,
    /// First sequence number in this segment.
    pub first_seq: u64,
    /// Sequence number of the final batch (`first_seq` when `record_count == 0`).
    pub last_seq: u64,
    /// Highest fencing epoch observed in this segment (`0` when `record_count == 0`).
    pub last_epoch: u64,
    /// SHA-256 head hash after the final batch (`prev_segment_head` when `record_count == 0`).
    pub head_hash: Digest32,
    /// RFC 6962 MMR root over this segment's batch digests (`root_from_peaks(&mmr_peaks)`).
    pub mmr_root: Digest32,
    /// RFC 6962 MMR peak list, largest peak first (`Mmr::peaks_at(record_count)`).
    pub mmr_peaks: Vec<Digest32>,
}

impl SegmentFooter {
    /// Encode this footer as a canonical `FMNE` + `SEGF` + CRC32C frame.
    pub fn to_bytes(&self) -> Result<Vec<u8>, LedgerError> {
        if self.mmr_peaks.len() > MAX_MMR_PEAKS {
            return Err(LedgerError::Codec(CodecError::LengthOutOfBounds {
                len: self.mmr_peaks.len(),
                max: MAX_MMR_PEAKS,
            }));
        }
        let mut enc = Encoder::new_frame(TAG_SEGMENT_FOOTER, self.format_version);
        enc.put_u64(self.record_count);
        enc.put_u64(self.first_seq);
        enc.put_u64(self.last_seq);
        enc.put_u64(self.last_epoch);
        enc.put_fixed(self.head_hash.as_bytes());
        enc.put_fixed(self.mmr_root.as_bytes());
        let count = u32::try_from(self.mmr_peaks.len()).map_err(|_| {
            LedgerError::Codec(CodecError::LengthOutOfBounds {
                len: self.mmr_peaks.len(),
                max: MAX_MMR_PEAKS,
            })
        })?;
        enc.put_u32(count);
        for peak in &self.mmr_peaks {
            enc.put_fixed(peak.as_bytes());
        }
        Ok(enc.finish())
    }

    /// Decode an `FMNE` + `SEGF` frame.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, LedgerError> {
        let mut dec =
            Decoder::open_frame(bytes, TAG_SEGMENT_FOOTER, FMT_SEG_MIN_VERSION, FMT_SEG_V1)?;
        let format_version = dec.version();
        let record_count = dec.get_u64()?;
        let first_seq = dec.get_u64()?;
        let last_seq = dec.get_u64()?;
        let last_epoch = dec.get_u64()?;
        let head_hash = Digest32(dec.get_fixed::<32>()?);
        let mmr_root = Digest32(dec.get_fixed::<32>()?);
        let count = dec.get_u32()? as usize;
        if count > MAX_MMR_PEAKS {
            return Err(LedgerError::Codec(CodecError::LengthOutOfBounds {
                len: count,
                max: MAX_MMR_PEAKS,
            }));
        }
        let mut mmr_peaks = Vec::with_capacity(count);
        for _ in 0..count {
            mmr_peaks.push(Digest32(dec.get_fixed::<32>()?));
        }
        dec.finish()?;
        Ok(Self {
            format_version,
            record_count,
            first_seq,
            last_seq,
            last_epoch,
            head_hash,
            mmr_root,
            mmr_peaks,
        })
    }
}

/// Complete structural inspection of an `FMT-SEG-1` byte buffer (`BD-082`, `BD-088`, `F7`).
#[derive(Clone, Debug)]
pub struct SegmentInspection {
    /// Validated segment header.
    pub header: SegmentHeader,
    /// Validated batches in sequence order up to the verified prefix.
    pub batches: Vec<DeltaBatch>,
    /// Canonical SHA-256 digests of `batches`.
    pub batch_digests: Vec<Digest32>,
    /// Replayed RFC 6962 MMR over `batch_digests`.
    pub mmr: Mmr,
    /// Last verified chain head (`header.prev_segment_head` if `batches` is empty).
    pub head_hash: Digest32,
    /// Highest verified fencing epoch (`0` if `batches` is empty).
    pub last_epoch: u64,
    /// Clean byte length of the verified prefix (header + verified batches + optional verified footer).
    pub verified_bytes: u64,
    /// Total byte length of the inspected buffer.
    pub total_bytes: u64,
    /// Sealed state or F7 torn-tail state.
    pub state: SegmentState,
}

impl SegmentInspection {
    /// DESIGN (BD-085): rebuild the keyed state of the verified prefix, checking append-only
    /// operations, obligations and key-state witnesses. A continuation needs its preceding log;
    /// an empty index is never substituted. This does not verify application laws or writer identity.
    pub fn replay_semantics(&self) -> Result<CellSnapshot, L0Error> {
        let ledger = replay_semantics(&self.header, &self.batches)?;
        let snapshot = ledger.as_of_seq(&self.header.cell_id, self.batches.len() as u64)?;
        if snapshot.head_hash != self.head_hash || snapshot.mmr_root != self.mmr.root() {
            return Err(LedgerError::ChainContinuity {
                detail: "semantic replay disagrees with the inspected head or MMR".to_string(),
            }
            .into());
        }
        Ok(snapshot)
    }
}

fn replay_semantics(header: &SegmentHeader, batches: &[DeltaBatch]) -> Result<L0Ledger, L0Error> {
    header.validate()?;
    if header.first_seq != 0 || !header.prev_mmr_peaks.is_empty() {
        return Err(LedgerError::ChainContinuity {
            detail: "semantic replay requires a complete genesis segment, not a continuation"
                .to_string(),
        }
        .into());
    }
    let mut ledger = L0Ledger::new();
    // DESIGN: replay is receiver admission. This placeholder grants no writer authority;
    // a caller must separately authenticate its persisted lease before submitting writes.
    ledger.create_cell(&header.cell_id, "segment-replay")?;
    for batch in batches {
        ledger.check_replica(batch)?;
        ledger.admit_replica(batch.clone())?;
    }
    Ok(ledger)
}

/// Lifecycle state observed when inspecting an `FMT-SEG-1` segment.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SegmentState {
    /// Segment ends with a verified `SEGF` footer matching replayed batches and MMR peaks.
    Sealed(SegmentFooter),
    /// Segment ends cleanly on a verified record (or header) boundary without a footer yet.
    OpenUnsealed,
    /// Segment has a crash-torn trailing frame after `verified_bytes` (`F7`); truncating to
    /// `verified_bytes` after retaining original bytes as evidence restores a clean prefix.
    TornTail {
        torn_offset: u64,
        torn_bytes: u64,
        reason: String,
    },
}

/// Pure, deterministic byte-level scanner for `FMT-SEG-1` (`BD-082`, `BD-088`, `F7`).
///
/// Refuses non-repairable interior corruption (`InteriorCorruption`), hash-chain breaks
/// (`ChainContinuity`), fencing epoch regressions (`EpochRegression`), and tampered footers
/// (`FooterMismatch`). Recognizes crash-torn trailing frames as [`SegmentState::TornTail`].
pub fn inspect_segment_bytes(bytes: &[u8]) -> Result<SegmentInspection, LedgerError> {
    if bytes.len() < 4 {
        return Err(LedgerError::InteriorCorruption {
            offset: 0,
            detail: format!(
                "segment shorter than 4-byte header length prefix ({} bytes)",
                bytes.len()
            ),
        });
    }
    let header_len = read_u32_le(bytes, 0) as usize;
    if !(14..=MAX_FRAME_BYTES).contains(&header_len) || 4 + header_len > bytes.len() {
        return Err(LedgerError::InteriorCorruption {
            offset: 0,
            detail: format!(
                "corrupt or incomplete segment header frame (declared {header_len} bytes, file is {} bytes)",
                bytes.len()
            ),
        });
    }
    let header = SegmentHeader::from_bytes(&bytes[4..4 + header_len]).map_err(|e| {
        LedgerError::InteriorCorruption {
            offset: 0,
            detail: format!("invalid segment header: {e}"),
        }
    })?;

    let mut cursor = 4 + header_len;
    let mut batches = Vec::new();
    let mut batch_digests = Vec::new();
    let mut mmr = Mmr::new();
    let mut expected_seq = header.first_seq;
    let mut head_hash = header.prev_segment_head;
    let mut last_epoch = 0u64;

    while cursor < bytes.len() {
        let remaining = bytes.len() - cursor;
        if remaining < 4 {
            return Ok(SegmentInspection {
                header,
                batches,
                batch_digests,
                mmr,
                head_hash,
                last_epoch,
                verified_bytes: cursor as u64,
                total_bytes: bytes.len() as u64,
                state: SegmentState::TornTail {
                    torn_offset: cursor as u64,
                    torn_bytes: remaining as u64,
                    reason: format!(
                        "truncated trailing frame length prefix ({remaining} bytes remaining)"
                    ),
                },
            });
        }
        let frame_len = read_u32_le(bytes, cursor) as usize;
        if !(14..=MAX_FRAME_BYTES).contains(&frame_len) {
            return torn_or_interior(
                bytes,
                cursor,
                remaining,
                format!("corrupt frame length {frame_len}"),
                header,
                batches,
                batch_digests,
                mmr,
                head_hash,
                last_epoch,
            );
        }
        let Some(frame_end) = cursor.checked_add(4).and_then(|s| s.checked_add(frame_len)) else {
            return Err(LedgerError::InteriorCorruption {
                offset: cursor as u64,
                detail: "frame end offset overflow".to_string(),
            });
        };
        if frame_end > bytes.len() {
            return torn_or_interior(
                bytes,
                cursor,
                remaining,
                format!(
                    "truncated trailing frame: declared {frame_len} payload bytes, only {} available",
                    remaining.saturating_sub(4)
                ),
                header,
                batches,
                batch_digests,
                mmr,
                head_hash,
                last_epoch,
            );
        }
        let frame = &bytes[cursor + 4..frame_end];
        let is_last_frame = frame_end == bytes.len();

        let tag = [frame[4], frame[5], frame[6], frame[7]];
        if &frame[0..4] == b"FMNE" && tag == TAG_SEGMENT_FOOTER {
            match SegmentFooter::from_bytes(frame) {
                Ok(footer) => {
                    if !is_last_frame {
                        return Err(LedgerError::InteriorCorruption {
                            offset: frame_end as u64,
                            detail: format!(
                                "{} trailing bytes follow sealed segment footer",
                                bytes.len() - frame_end
                            ),
                        });
                    }
                    verify_footer(&header, &batches, head_hash, last_epoch, &mmr, &footer)?;
                    return Ok(SegmentInspection {
                        header,
                        batches,
                        batch_digests,
                        mmr,
                        head_hash,
                        last_epoch,
                        verified_bytes: frame_end as u64,
                        total_bytes: bytes.len() as u64,
                        state: SegmentState::Sealed(footer),
                    });
                }
                Err(err) => {
                    return torn_or_interior(
                        bytes,
                        cursor,
                        remaining,
                        format!("corrupt or torn footer frame: {err}"),
                        header,
                        batches,
                        batch_digests,
                        mmr,
                        head_hash,
                        last_epoch,
                    );
                }
            }
        }

        if &frame[0..4] != b"FMNE" || tag != TAG_LEDGER_BATCH {
            return torn_or_interior(
                bytes,
                cursor,
                remaining,
                "corrupt frame magic or tag".to_string(),
                header,
                batches,
                batch_digests,
                mmr,
                head_hash,
                last_epoch,
            );
        }

        match DeltaBatch::from_bytes(frame) {
            Ok(batch) => {
                if batch.cell_id != header.cell_id {
                    return Err(LedgerError::ChainContinuity {
                        detail: format!(
                            "batch at byte {cursor} belongs to cell '{}' instead of segment cell '{}'",
                            batch.cell_id, header.cell_id
                        ),
                    });
                }
                if batch.seq != expected_seq {
                    return Err(LedgerError::ChainContinuity {
                        detail: format!(
                            "batch at byte {cursor} has seq {}, expected {expected_seq}",
                            batch.seq
                        ),
                    });
                }
                if batch.prev_hash != head_hash {
                    return Err(LedgerError::ChainContinuity {
                        detail: format!(
                            "batch seq {} at byte {cursor} has prev_hash {}, expected {head_hash}",
                            batch.seq, batch.prev_hash
                        ),
                    });
                }
                if batch.epoch < last_epoch {
                    return Err(LedgerError::EpochRegression {
                        cell_id: header.cell_id.clone(),
                        observed: batch.epoch,
                        highest: last_epoch,
                    });
                }
                let digest = sha256(frame);
                mmr.push(digest.as_bytes())
                    .map_err(|e| LedgerError::ChainContinuity {
                        detail: format!("MMR push failed at seq {}: {e}", batch.seq),
                    })?;
                last_epoch = batch.epoch;
                head_hash = digest;
                expected_seq =
                    expected_seq
                        .checked_add(1)
                        .ok_or_else(|| LedgerError::ChainContinuity {
                            detail: "segment sequence counter overflow".to_string(),
                        })?;
                batches.push(batch);
                batch_digests.push(digest);
                cursor = frame_end;
            }
            Err(err) => {
                return torn_or_interior(
                    bytes,
                    cursor,
                    remaining,
                    format!("corrupt or torn batch frame: {err}"),
                    header,
                    batches,
                    batch_digests,
                    mmr,
                    head_hash,
                    last_epoch,
                );
            }
        }
    }

    Ok(SegmentInspection {
        header,
        batches,
        batch_digests,
        mmr,
        head_hash,
        last_epoch,
        verified_bytes: cursor as u64,
        total_bytes: bytes.len() as u64,
        state: SegmentState::OpenUnsealed,
    })
}

/// Classifies the bytes from `cursor`, which do not form a verified record (`F7`).
///
/// They are a crash-torn tail unless a whole record starts at or after `cursor`: a length prefix, in range,
/// then that many bytes forming an `FMNE` frame tagged as an `FMT-LEDGER-1` batch or a segment footer whose
/// CRC verifies. Torn bytes cannot pass that CRC, so such a frame was written whole: history beyond the
/// damage, or a record this build cannot decode (a newer format version, say), even at `cursor` itself.
/// Either is refused as interior corruption and never truncated. A crash can leave any part of the in-flight
/// write on disk, even its later sectors without its length prefix, and a batch embeds its anchor as a
/// complete frame of its own, so finding `FMNE` alone in the tail proves nothing. The search is bounded: a
/// tail whose candidate frames cost too much to check is refused too ([`AfterDamage::TooManyCandidates`]).
#[allow(
    clippy::too_many_arguments,
    reason = "internal tail-vs-interior classifier"
)]
fn torn_or_interior(
    bytes: &[u8],
    cursor: usize,
    remaining: usize,
    reason: String,
    header: SegmentHeader,
    batches: Vec<DeltaBatch>,
    batch_digests: Vec<Digest32>,
    mmr: Mmr,
    head_hash: Digest32,
    last_epoch: u64,
) -> Result<SegmentInspection, LedgerError> {
    match after_damage(bytes, cursor) {
        AfterDamage::Nothing => {}
        AfterDamage::Record(offset) => {
            return Err(LedgerError::InteriorCorruption {
                offset: cursor as u64,
                detail: format!(
                    "{reason}; a whole batch or footer frame (its CRC verifies) starts at byte {offset}"
                ),
            });
        }
        AfterDamage::TooManyCandidates => {
            return Err(LedgerError::InteriorCorruption {
                offset: cursor as u64,
                detail: format!(
                    "{reason}; the bytes after it hold more overlapping candidate frames than a crash leaves"
                ),
            });
        }
    }
    Ok(SegmentInspection {
        header,
        batches,
        batch_digests,
        mmr,
        head_hash,
        last_epoch,
        verified_bytes: cursor as u64,
        total_bytes: bytes.len() as u64,
        state: SegmentState::TornTail {
            torn_offset: cursor as u64,
            torn_bytes: remaining as u64,
            reason,
        },
    })
}

/// What follows the unreadable bytes at `cursor` (see [`torn_or_interior`]).
#[derive(Debug, PartialEq, Eq)]
enum AfterDamage {
    /// No whole record starts at or after `cursor`: the bytes are a torn tail.
    Nothing,
    /// The offset of the length prefix of the first whole batch or footer record found.
    Record(usize),
    /// Checking the candidate frames would cost more than [`CANDIDATE_BYTES_PER_TAIL_BYTE`] times the bytes
    /// from `cursor`. Each check computes a CRC over the candidate's whole declared length, and candidates
    /// can overlap, so a crafted tail would otherwise take time quadratic in its size. Candidates that do not
    /// overlap cost at most the bytes from `cursor` in all, and a crash tail holds none outside its one write.
    /// Such a tail is refused, as interior corruption is, so the budget never lets damage read as torn.
    TooManyCandidates,
}

/// Check budget for [`after_damage`], in candidate frame bytes per byte from the damage to the end of the
/// file.
const CANDIDATE_BYTES_PER_TAIL_BYTE: usize = 2;

/// Looks for a whole batch or footer record whose length prefix starts at or after `cursor`. That includes
/// the record at `cursor` itself: it failed to decode, and if its CRC verifies, not because a crash tore it.
/// One that starts a byte to three bytes later overlaps its length prefix, as after a few stray bytes before
/// a record, and must still count.
fn after_damage(bytes: &[u8], cursor: usize) -> AfterDamage {
    let Some(first_magic) = cursor.checked_add(4) else {
        return AfterDamage::Nothing;
    };
    let Some(tail) = bytes.get(first_magic..) else {
        return AfterDamage::Nothing;
    };
    let mut budget = bytes
        .len()
        .saturating_sub(cursor)
        .saturating_mul(CANDIDATE_BYTES_PER_TAIL_BYTE);
    let candidates = tail.windows(4).enumerate();
    for (magic, _) in candidates.filter(|(_, w)| *w == b"FMNE") {
        let magic = first_magic + magic;
        let frame_len = read_u32_le(bytes, magic - 4) as usize;
        if !(14..=MAX_FRAME_BYTES).contains(&frame_len) {
            continue;
        }
        let Some(frame) = magic
            .checked_add(frame_len)
            .and_then(|end| bytes.get(magic..end))
        else {
            continue;
        };
        let tag = &frame[4..8];
        if tag != TAG_LEDGER_BATCH && tag != TAG_SEGMENT_FOOTER {
            continue;
        }
        let Some(left) = budget.checked_sub(frame_len) else {
            return AfterDamage::TooManyCandidates;
        };
        budget = left;
        // The frame CRC alone decides: a record that verifies but does not decode was still written whole.
        let (body, crc) = frame.split_at(frame_len - 4);
        if crc32c(body).to_le_bytes()[..] == *crc {
            return AfterDamage::Record(magic - 4);
        }
    }
    AfterDamage::Nothing
}

fn verify_footer(
    header: &SegmentHeader,
    batches: &[DeltaBatch],
    head_hash: Digest32,
    last_epoch: u64,
    mmr: &Mmr,
    footer: &SegmentFooter,
) -> Result<(), LedgerError> {
    let expected_count = batches.len() as u64;
    if footer.record_count != expected_count {
        return Err(LedgerError::FooterMismatch {
            detail: format!(
                "footer record_count {} != replayed {expected_count}",
                footer.record_count
            ),
        });
    }
    if footer.first_seq != header.first_seq {
        return Err(LedgerError::FooterMismatch {
            detail: format!(
                "footer first_seq {} != header first_seq {}",
                footer.first_seq, header.first_seq
            ),
        });
    }
    let expected_last_seq = batches.last().map_or(header.first_seq, |b| b.seq);
    if footer.last_seq != expected_last_seq {
        return Err(LedgerError::FooterMismatch {
            detail: format!(
                "footer last_seq {} != replayed {expected_last_seq}",
                footer.last_seq
            ),
        });
    }
    if footer.last_epoch != last_epoch {
        return Err(LedgerError::FooterMismatch {
            detail: format!(
                "footer last_epoch {} != replayed {last_epoch}",
                footer.last_epoch
            ),
        });
    }
    if footer.head_hash != head_hash {
        return Err(LedgerError::FooterMismatch {
            detail: format!(
                "footer head_hash {} != replayed {head_hash}",
                footer.head_hash
            ),
        });
    }
    let expected_peaks = mmr
        .peaks_at(mmr.len())
        .map_err(|e| LedgerError::FooterMismatch {
            detail: format!("failed to compute MMR peaks: {e}"),
        })?;
    let expected_root = mmr.root();
    if footer.mmr_peaks != expected_peaks {
        return Err(LedgerError::FooterMismatch {
            detail: format!(
                "footer MMR peaks ({} entries) do not match replayed peaks ({} entries)",
                footer.mmr_peaks.len(),
                expected_peaks.len()
            ),
        });
    }
    if footer.mmr_root != expected_root || root_from_peaks(&footer.mmr_peaks) != footer.mmr_root {
        return Err(LedgerError::FooterMismatch {
            detail: format!(
                "footer mmr_root {} != replayed {expected_root}",
                footer.mmr_root
            ),
        });
    }
    Ok(())
}

fn read_u32_le(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
    ])
}

fn write_framed(file: &mut File, frame: &[u8]) -> Result<(), LedgerError> {
    if frame.len() > MAX_FRAME_BYTES {
        return Err(LedgerError::Codec(CodecError::LengthOutOfBounds {
            len: frame.len(),
            max: MAX_FRAME_BYTES,
        }));
    }
    let position = file.stream_position().map_err(|e| LedgerError::Io {
        operation: "write_position",
        detail: e.to_string(),
    })?;
    let end = position.checked_add(frame.len() as u64 + 4);
    if end.is_none_or(|end| end > MAX_SEGMENT_BYTES) {
        return Err(LedgerError::InvalidField {
            field: "segment.size",
            detail: "append would exceed the bounded segment reader; rotate the segment"
                .to_string(),
        });
    }
    let len = u32::try_from(frame.len()).map_err(|_| {
        LedgerError::Codec(CodecError::LengthOutOfBounds {
            len: frame.len(),
            max: MAX_FRAME_BYTES,
        })
    })?;
    file.write_all(&len.to_le_bytes())
        .and_then(|()| file.write_all(frame))
        .and_then(|()| file.sync_all())
        .map_err(|e| LedgerError::Io {
            operation: "write_framed",
            detail: e.to_string(),
        })
}

pub(crate) fn sync_parent_dir(path: &Path) -> Result<(), LedgerError> {
    #[cfg(unix)]
    {
        let parent = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        File::open(parent)
            .and_then(|d| d.sync_all())
            .map_err(|e| LedgerError::Io {
                operation: "sync_parent_dir",
                detail: format!("{}: {e}", parent.display()),
            })?;
    }
    #[cfg(not(unix))]
    {
        let _ = path;
    }
    Ok(())
}

/// Exclusive append-only writer for one `FMT-SEG-1` segment file (`BD-082`).
#[derive(Debug)]
pub struct SegmentWriter {
    file: LockedFile,
    header: SegmentHeader,
    next_seq: u64,
    head_hash: Digest32,
    last_epoch: u64,
    record_count: u64,
    mmr: Mmr,
    sealed: bool,
    failed_write: bool,
}

impl SegmentWriter {
    /// Create a brand-new `FMT-SEG-1` segment file and sync its header (`create_new(true)`).
    pub fn create(
        path: &Path,
        cell_id: impl Into<String>,
        first_seq: u64,
        prev_segment_head: Digest32,
        prev_mmr_peaks: Vec<Digest32>,
    ) -> Result<Self, LedgerError> {
        let header = SegmentHeader {
            format_version: FMT_SEG_V1,
            cell_id: cell_id.into(),
            first_seq,
            prev_segment_head,
            prev_mmr_peaks,
        };
        let header_bytes = header.to_bytes()?;
        let mut opts = OpenOptions::new();
        opts.read(true).write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt as _;
            opts.mode(0o600);
        }
        let raw = opts.open(path).map_err(|e| LedgerError::Io {
            operation: "create_segment",
            detail: format!("{}: {e}", path.display()),
        })?;
        let mut file = LockedFile::acquire(raw, true)?;
        write_framed(file.file_mut(), &header_bytes)?;
        sync_parent_dir(path)?;
        Ok(Self {
            file,
            header,
            next_seq: first_seq,
            head_hash: prev_segment_head,
            last_epoch: 0,
            record_count: 0,
            mmr: Mmr::new(),
            sealed: false,
            failed_write: false,
        })
    }

    /// Re-open an unsealed clean `FMT-SEG-1` segment for appending under an exclusive lock.
    /// Refuses sealed segments, torn segments, or corrupted segments (forcing `repair plan` first).
    pub fn open_append(path: &Path) -> Result<Self, LedgerError> {
        Self::open_append_with_batches(path).map(|(writer, _)| writer)
    }

    fn open_append_with_batches(path: &Path) -> Result<(Self, Vec<DeltaBatch>), LedgerError> {
        let raw = OpenOptions::new()
            .read(true)
            .write(true)
            .open(path)
            .map_err(|e| LedgerError::Io {
                operation: "open_segment_append",
                detail: format!("{}: {e}", path.display()),
            })?;
        let mut file = LockedFile::acquire(raw, true)?;
        let bytes = read_bounded(file.file_mut(), path)?;
        let inspected = inspect_segment_bytes(&bytes)?;
        match inspected.state {
            SegmentState::OpenUnsealed => {
                file.file_mut()
                    .seek(SeekFrom::End(0))
                    .map_err(|e| LedgerError::Io {
                        operation: "seek_end",
                        detail: e.to_string(),
                    })?;
                let record_count = inspected.batches.len() as u64;
                let next_seq = inspected
                    .header
                    .first_seq
                    .checked_add(record_count)
                    .ok_or_else(|| LedgerError::ChainContinuity {
                        detail: "sequence overflow".to_string(),
                    })?;
                let writer = Self {
                    file,
                    header: inspected.header,
                    next_seq,
                    head_hash: inspected.head_hash,
                    last_epoch: inspected.last_epoch,
                    record_count,
                    mmr: inspected.mmr,
                    sealed: false,
                    failed_write: false,
                };
                Ok((writer, inspected.batches))
            }
            SegmentState::Sealed(_) => Err(LedgerError::ChainContinuity {
                detail: "cannot append to an already sealed segment".to_string(),
            }),
            SegmentState::TornTail {
                torn_offset,
                reason,
                ..
            } => Err(LedgerError::InteriorCorruption {
                offset: torn_offset,
                detail: format!(
                    "segment has a torn tail ({reason}); run `fmine repair plan` before resuming writes"
                ),
            }),
        }
    }

    /// Append one canonical `FMT-LEDGER-1` batch, syncing to disk before returning its digest.
    pub fn append(&mut self, batch: &DeltaBatch) -> Result<Digest32, LedgerError> {
        self.check_write_state()?;
        if self.sealed {
            return Err(LedgerError::ChainContinuity {
                detail: "segment is already sealed".to_string(),
            });
        }
        let frame = batch.to_bytes()?;
        if batch.cell_id != self.header.cell_id {
            return Err(LedgerError::ChainContinuity {
                detail: format!(
                    "batch cell '{}' != segment cell '{}'",
                    batch.cell_id, self.header.cell_id
                ),
            });
        }
        if batch.seq != self.next_seq {
            return Err(LedgerError::ChainContinuity {
                detail: format!("expected seq {}, got {}", self.next_seq, batch.seq),
            });
        }
        if batch.prev_hash != self.head_hash {
            return Err(LedgerError::ChainContinuity {
                detail: format!(
                    "expected prev_hash {}, got {}",
                    self.head_hash, batch.prev_hash
                ),
            });
        }
        if batch.epoch < self.last_epoch {
            return Err(LedgerError::EpochRegression {
                cell_id: self.header.cell_id.clone(),
                observed: batch.epoch,
                highest: self.last_epoch,
            });
        }
        let record_count =
            self.record_count
                .checked_add(1)
                .ok_or_else(|| LedgerError::ChainContinuity {
                    detail: "record_count overflow".to_string(),
                })?;
        let next_seq =
            self.next_seq
                .checked_add(1)
                .ok_or_else(|| LedgerError::ChainContinuity {
                    detail: "sequence overflow".to_string(),
                })?;
        let digest = sha256(&frame);
        // DESIGN: an I/O error can leave an unacknowledged frame on disk. Reopen and inspect
        // before retrying; appending another frame could turn a repairable tail into corruption.
        self.failed_write = true;
        write_framed(self.file.file_mut(), &frame)?;
        self.mmr
            .push(digest.as_bytes())
            .map_err(|e| LedgerError::ChainContinuity {
                detail: format!("MMR push failed: {e}"),
            })?;
        self.last_epoch = batch.epoch;
        self.head_hash = digest;
        self.record_count = record_count;
        self.next_seq = next_seq;
        self.failed_write = false;
        Ok(digest)
    }

    /// Seal the segment by writing its `SEGF` MMR peak footer and syncing to disk.
    pub fn seal(mut self) -> Result<SegmentFooter, LedgerError> {
        self.check_write_state()?;
        if self.sealed {
            return Err(LedgerError::ChainContinuity {
                detail: "segment is already sealed".to_string(),
            });
        }
        let mmr_peaks =
            self.mmr
                .peaks_at(self.mmr.len())
                .map_err(|e| LedgerError::FooterMismatch {
                    detail: format!("failed to extract MMR peaks: {e}"),
                })?;
        let mmr_root = self.mmr.root();
        let last_seq = if self.record_count == 0 {
            self.header.first_seq
        } else {
            self.next_seq.saturating_sub(1)
        };
        let footer = SegmentFooter {
            format_version: FMT_SEG_V1,
            record_count: self.record_count,
            first_seq: self.header.first_seq,
            last_seq,
            last_epoch: self.last_epoch,
            head_hash: self.head_hash,
            mmr_root,
            mmr_peaks,
        };
        let frame = footer.to_bytes()?;
        write_framed(self.file.file_mut(), &frame)?;
        self.sealed = true;
        Ok(footer)
    }

    /// Current chain head hash.
    #[must_use]
    pub const fn head_hash(&self) -> Digest32 {
        self.head_hash
    }

    /// Current RFC 6962 MMR root.
    #[must_use]
    pub fn mmr_root(&self) -> Digest32 {
        self.mmr.root()
    }

    fn check_write_state(&self) -> Result<(), LedgerError> {
        if self.failed_write {
            return Err(LedgerError::Io {
                operation: "resume_after_failed_write",
                detail: "a prior append failed; reopen and inspect the segment before writing"
                    .to_string(),
            });
        }
        Ok(())
    }
}

/// DESIGN (BD-085/BD-086): a single-segment combiner with the L0 keyed index rebuilt at open.
/// Checks run before persistence; the index advances only after the frame has been synced.
///
/// This is receiver-style semantic admission, not lease authentication. The embedding authority
/// must authenticate its persisted writer lease and provide application laws. Cross-cell references,
/// signatures and legacy witness digests are retained but are not certified by this writer.
#[derive(Debug)]
pub struct SemanticSegmentWriter {
    writer: SegmentWriter,
    ledger: L0Ledger,
    failed_commit: bool,
}

impl SemanticSegmentWriter {
    pub fn create(path: &Path, cell_id: impl Into<String>) -> Result<Self, L0Error> {
        let cell_id = cell_id.into();
        let mut ledger = L0Ledger::new();
        ledger.create_cell(&cell_id, "segment-replay")?;
        let writer = SegmentWriter::create(path, cell_id, 0, GENESIS_PREV_HASH, Vec::new())?;
        Ok(Self {
            writer,
            ledger,
            failed_commit: false,
        })
    }

    pub fn open_append(path: &Path) -> Result<Self, L0Error> {
        let (writer, batches) = SegmentWriter::open_append_with_batches(path)?;
        let ledger = replay_semantics(&writer.header, &batches)?;
        Ok(Self {
            writer,
            ledger,
            failed_commit: false,
        })
    }

    /// The index cannot be mutated independently of the durable log.
    #[must_use]
    pub const fn ledger(&self) -> &L0Ledger {
        &self.ledger
    }

    #[must_use]
    pub fn cell_id(&self) -> &str {
        &self.writer.header.cell_id
    }

    pub fn key_witness(&self, key: &str) -> Result<Witness, L0Error> {
        self.ledger.key_witness(self.cell_id(), key)
    }

    pub fn snapshot(&self) -> Result<CellSnapshot, L0Error> {
        self.ledger.as_of_seq(self.cell_id(), self.writer.next_seq)
    }

    pub fn append(&mut self, batch: &DeltaBatch) -> Result<Digest32, L0Error> {
        self.append_with_law(batch, |_, _| Ok(()))
    }

    /// A law sees the pre-commit index and complete batch. Rejections leave disk and index
    /// unchanged. After an uncertain persistence error this handle refuses further writes.
    pub fn append_with_law<F>(&mut self, batch: &DeltaBatch, check: F) -> Result<Digest32, L0Error>
    where
        F: FnOnce(&L0Ledger, &DeltaBatch) -> Result<(), String>,
    {
        self.check_commit_state()?;
        self.ledger.check_replica(batch)?;
        check(&self.ledger, batch).map_err(|detail| L0Error::ConservationViolation {
            cell_id: batch.cell_id.clone(),
            detail,
        })?;
        self.failed_commit = true;
        let digest = self.writer.append(batch)?;
        match self.ledger.admit_replica(batch.clone())? {
            AdmitOutcome::Committed { batch_digest, .. } if batch_digest == digest => {
                self.failed_commit = false;
                Ok(digest)
            }
            _ => Err(LedgerError::ChainContinuity {
                detail: "durable append disagrees with preflight semantic admission".to_string(),
            }
            .into()),
        }
    }

    /// The narrow combiner (plan §26.1, `BD-085`) without an application law; see
    /// [`Self::commit_prepared_with_law`].
    pub fn commit_prepared(
        &mut self,
        prepared: &DeltaBatch,
    ) -> Result<(Digest32, DeltaBatch), L0Error> {
        self.commit_prepared_with_law(prepared, |_, _| Ok(()))
    }

    /// The narrow combiner (plan §26.1, `BD-085`): commits `prepared`, which a caller computed against some head
    /// this cell has had, at the current head. This is optimistic concurrency: the batch commits if every key it
    /// witnessed is unchanged, and otherwise refuses with [`L0Error::KeyWitnessMismatch`] (`FMINE-REF-502`) so the
    /// caller recomputes.
    ///
    /// A batch already at the current head is appended as given. Otherwise only `seq`, `prev_hash` and
    /// `anchor.commit_sequence` are re-stamped. The epoch, the anchor's registry epochs and its `state_root`, which
    /// keeps naming the state the caller computed against, are kept. Refused with [`L0Error::NotRebasable`]
    /// before anything is written: a base `(seq, prev_hash)` that is not a head of this cell; a lease transfer
    /// computed at an older head; a `Supersede` of a key the batch has no `KeyState` witness for, which could
    /// overwrite a version the caller never read. Returns the digest and the batch as committed.
    pub fn commit_prepared_with_law<F>(
        &mut self,
        prepared: &DeltaBatch,
        check: F,
    ) -> Result<(Digest32, DeltaBatch), L0Error>
    where
        F: FnOnce(&L0Ledger, &DeltaBatch) -> Result<(), String>,
    {
        let refuse = |detail: String| L0Error::NotRebasable {
            cell_id: prepared.cell_id.clone(),
            detail,
        };
        let next_seq = self.writer.next_seq;
        if prepared.seq > next_seq {
            return Err(refuse(format!(
                "its seq {} is past the next seq {next_seq}",
                prepared.seq
            )));
        }
        if self.ledger.prev_hash_at(&prepared.cell_id, prepared.seq)? != prepared.prev_hash {
            return Err(refuse(format!(
                "its prev_hash is not this cell's head at seq {}",
                prepared.seq
            )));
        }
        let batch = if prepared.seq == next_seq {
            prepared.clone()
        } else {
            let witnessed: BTreeSet<&str> = prepared
                .witnesses
                .iter()
                .filter(|w| w.kind == WitnessKind::KeyState)
                .map(|w| w.subject_id.as_str())
                .collect();
            for op in &prepared.deltas {
                if op.kind == DeltaKind::TransferLease {
                    return Err(refuse(format!(
                        "it transfers the lease, computed at seq {}",
                        prepared.seq
                    )));
                }
                if op.kind == DeltaKind::Supersede && !witnessed.contains(op.key.as_str()) {
                    return Err(refuse(format!(
                        "it supersedes '{}' without a KeyState witness for it",
                        op.key
                    )));
                }
            }
            let mut batch = prepared.clone();
            batch.seq = next_seq;
            batch.prev_hash = self.ledger.head_hash(&prepared.cell_id)?;
            batch.anchor.commit_sequence = next_seq;
            batch
        };
        let digest = self.append_with_law(&batch, check)?;
        Ok((digest, batch))
    }

    pub fn seal(self) -> Result<SegmentFooter, L0Error> {
        self.check_commit_state()?;
        Ok(self.writer.seal()?)
    }

    fn check_commit_state(&self) -> Result<(), L0Error> {
        if self.failed_commit {
            return Err(LedgerError::Io {
                operation: "resume_after_failed_commit",
                detail: "reopen and inspect the segment after an uncertain durable commit"
                    .to_string(),
            }
            .into());
        }
        Ok(())
    }
}

/// Read-only `FMT-SEG-1` segment inspector holding a shared OS lock during read.
pub struct SegmentReader;

impl SegmentReader {
    /// Read and inspect an `FMT-SEG-1` segment file on disk under a shared lock.
    pub fn open(path: &Path) -> Result<SegmentInspection, LedgerError> {
        let raw = File::open(path).map_err(|e| LedgerError::Io {
            operation: "open_segment_read",
            detail: format!("{}: {e}", path.display()),
        })?;
        let mut locked = LockedFile::acquire(raw, false)?;
        let bytes = read_bounded(locked.file_mut(), path)?;
        inspect_segment_bytes(&bytes)
    }
}

pub(crate) fn read_bounded(file: &mut File, path: &Path) -> Result<Vec<u8>, LedgerError> {
    let mut bytes = Vec::new();
    Read::by_ref(file)
        .take(MAX_SEGMENT_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| LedgerError::Io {
            operation: "read_segment",
            detail: format!("{}: {e}", path.display()),
        })?;
    if (bytes.len() as u64) > MAX_SEGMENT_BYTES {
        return Err(LedgerError::Codec(CodecError::LengthOutOfBounds {
            len: bytes.len(),
            max: MAX_SEGMENT_BYTES as usize,
        }));
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::batch::tests::sample_batch;
    use crate::batch::{DeltaKind, DeltaOp, FMT_LEDGER_V2};
    use crate::mmr::{leaf_hash, verify_consistency, verify_inclusion};

    fn scratch_path(tag: &str) -> std::path::PathBuf {
        let dir =
            std::env::temp_dir().join(format!("fmine-ledger-seg-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("scratch dir");
        dir.join("seg-000001.fmseg")
    }

    fn semantic_batch(
        seq: u64,
        prev: Digest32,
        deltas: Vec<DeltaOp>,
        witnesses: Vec<Witness>,
    ) -> DeltaBatch {
        let mut batch = sample_batch("CELL-EVID(test)", 1, seq, prev);
        batch.format_version = FMT_LEDGER_V2;
        batch.deltas = deltas;
        batch.deltas.sort();
        batch.witnesses = witnesses;
        batch.witnesses.sort();
        batch.cross_cell_refs.clear();
        batch
    }

    #[test]
    fn semantic_writer_reopens_with_exact_l0_history_obligations_and_read_witnesses() {
        let path = scratch_path("semantic-parity");
        let mut writer = SemanticSegmentWriter::create(&path, "CELL-EVID(test)").unwrap();
        let mut reference = L0Ledger::new();
        let lease = reference.create_cell("CELL-EVID(test)", "writer").unwrap();
        let absent = writer.key_witness("grade").unwrap();
        let first = semantic_batch(
            0,
            GENESIS_PREV_HASH,
            vec![
                DeltaOp::new("grade", DeltaKind::Put, b"42".to_vec()),
                DeltaOp::new("OB-001", DeltaKind::ReserveObligation, Vec::new()),
            ],
            vec![absent],
        );
        let head = writer.append(&first).unwrap();
        assert_eq!(head, reference.commit(&lease, first).unwrap());
        let grade_read = writer.key_witness("grade").unwrap();
        let unrelated = semantic_batch(
            1,
            head,
            vec![DeltaOp::new("other", DeltaKind::Put, b"unchanged".to_vec())],
            vec![grade_read.clone()],
        );
        let head = writer.append(&unrelated).unwrap();
        reference.commit(&lease, unrelated).unwrap();
        let before = writer.snapshot().unwrap();
        assert_eq!(before, reference.as_of_seq("CELL-EVID(test)", 2).unwrap());
        drop(writer);

        let mut writer = SemanticSegmentWriter::open_append(&path).unwrap();
        assert_eq!(writer.snapshot().unwrap(), before);
        assert_eq!(writer.key_witness("grade").unwrap(), grade_read);
        let next = semantic_batch(
            2,
            head,
            vec![
                DeltaOp::new("grade", DeltaKind::Supersede, b"43".to_vec()),
                DeltaOp::new("OB-001", DeltaKind::ResolveObligation, Vec::new()),
            ],
            vec![grade_read],
        );
        writer
            .append_with_law(&next, |state, batch| {
                let old = state.key_history(&batch.cell_id, "grade").unwrap();
                assert_eq!(old.last().unwrap().payload, b"42");
                assert!(
                    state
                        .open_obligations(&batch.cell_id)
                        .unwrap()
                        .contains("OB-001")
                );
                Ok(())
            })
            .unwrap();
        reference.commit(&lease, next).unwrap();
        let expected = reference.as_of_seq("CELL-EVID(test)", 3).unwrap();
        assert_eq!(writer.snapshot().unwrap(), expected);
        assert_eq!(
            writer
                .ledger()
                .key_history(writer.cell_id(), "grade")
                .unwrap()
                .len(),
            2
        );
        writer.seal().unwrap();
        assert_eq!(
            SegmentReader::open(&path)
                .unwrap()
                .replay_semantics()
                .unwrap(),
            expected
        );
    }

    #[test]
    fn narrow_combiner_commits_prepared_batches_whose_reads_hold_and_refuses_stale_reads() {
        let path = scratch_path("combiner");
        let mut writer = SemanticSegmentWriter::create(&path, "CELL-EVID(test)").unwrap();
        let genesis = semantic_batch(
            0,
            GENESIS_PREV_HASH,
            vec![
                DeltaOp::new("grade", DeltaKind::Put, b"42".to_vec()),
                DeltaOp::new("tonnes", DeltaKind::Put, b"100".to_vec()),
            ],
            Vec::new(),
        );
        let base = writer.append(&genesis).unwrap();
        // Three callers compute against the same head, so each prepares seq 1.
        let grade_read = writer.key_witness("grade").unwrap();
        let tonnes_read = writer.key_witness("tonnes").unwrap();
        let supersede_grade = semantic_batch(
            1,
            base,
            vec![DeltaOp::new("grade", DeltaKind::Supersede, b"43".to_vec())],
            vec![grade_read.clone()],
        );
        let from_tonnes = semantic_batch(
            1,
            base,
            vec![DeltaOp::new("milled", DeltaKind::Put, b"100".to_vec())],
            vec![tonnes_read.clone()],
        );
        let from_grade = semantic_batch(
            1,
            base,
            vec![DeltaOp::new("metal", DeltaKind::Put, b"4200".to_vec())],
            vec![grade_read],
        );

        let (_, committed) = writer.commit_prepared(&supersede_grade).unwrap();
        assert_eq!(
            committed, supersede_grade,
            "a batch at the head commits as given"
        );

        // Its read was not touched: re-stamped onto the new head, nothing else changed.
        let head = writer.ledger().head_hash("CELL-EVID(test)").unwrap();
        let (digest, rebased) = writer.commit_prepared(&from_tonnes).unwrap();
        assert_eq!(
            (
                rebased.seq,
                rebased.prev_hash,
                rebased.anchor.commit_sequence
            ),
            (2, head, 2)
        );
        let mut unstamped = rebased.clone();
        unstamped.seq = 1;
        unstamped.prev_hash = base;
        unstamped.anchor.commit_sequence = 1;
        assert_eq!(
            unstamped, from_tonnes,
            "only seq, prev_hash and commit_sequence move"
        );
        assert_eq!(
            digest,
            writer.ledger().head_hash("CELL-EVID(test)").unwrap()
        );

        // Its read was changed: refused before anything is written, and the caller recomputes.
        let bytes = std::fs::read(&path).unwrap();
        let state = writer.snapshot().unwrap();
        let err = writer.commit_prepared(&from_grade).unwrap_err();
        assert!(
            matches!(&err, L0Error::KeyWitnessMismatch { key, .. } if key == "grade"),
            "{err}"
        );
        assert_eq!(err.to_refusal().code, "FMINE-REF-502");
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
        assert_eq!(writer.snapshot().unwrap(), state);
        let recomputed = semantic_batch(
            3,
            writer.ledger().head_hash("CELL-EVID(test)").unwrap(),
            vec![DeltaOp::new("metal", DeltaKind::Put, b"4300".to_vec())],
            vec![writer.key_witness("grade").unwrap()],
        );
        writer.commit_prepared(&recomputed).unwrap();
        let before = writer.snapshot().unwrap();
        drop(writer);

        // The keyed index is rebuilt at open: the stale read still refuses and a fresh one still rebases.
        let mut writer = SemanticSegmentWriter::open_append(&path).unwrap();
        assert_eq!(writer.snapshot().unwrap(), before);
        assert!(matches!(
            writer.commit_prepared(&from_grade),
            Err(L0Error::KeyWitnessMismatch { .. })
        ));
        let late = semantic_batch(
            1,
            base,
            vec![DeltaOp::new("hauled", DeltaKind::Put, b"100".to_vec())],
            vec![tonnes_read],
        );
        assert_eq!(writer.commit_prepared(&late).unwrap().1.seq, 4);
        let expected = writer.snapshot().unwrap();
        writer.seal().unwrap();
        assert_eq!(
            SegmentReader::open(&path)
                .unwrap()
                .replay_semantics()
                .unwrap(),
            expected
        );
    }

    #[test]
    fn narrow_combiner_refuses_rebases_that_could_lose_an_update() {
        let path = scratch_path("combiner-refuse");
        let mut writer = SemanticSegmentWriter::create(&path, "CELL-EVID(test)").unwrap();
        let genesis = semantic_batch(
            0,
            GENESIS_PREV_HASH,
            vec![DeltaOp::new("grade", DeltaKind::Put, b"42".to_vec())],
            Vec::new(),
        );
        let base = writer.append(&genesis).unwrap();
        let witnessed = semantic_batch(
            1,
            base,
            vec![DeltaOp::new("grade", DeltaKind::Supersede, b"43".to_vec())],
            vec![writer.key_witness("grade").unwrap()],
        );
        let blind = semantic_batch(
            1,
            base,
            vec![DeltaOp::new("grade", DeltaKind::Supersede, b"44".to_vec())],
            Vec::new(),
        );
        let lease = semantic_batch(
            1,
            base,
            vec![DeltaOp::new("lease", DeltaKind::TransferLease, Vec::new())],
            Vec::new(),
        );
        writer.commit_prepared(&witnessed).unwrap();

        let bytes = std::fs::read(&path).unwrap();
        let state = writer.snapshot().unwrap();
        let mut forked = witnessed.clone();
        forked.prev_hash = sha256(b"another history");
        let mut ahead = witnessed;
        ahead.seq = 5;
        ahead.anchor.commit_sequence = 5;
        for (name, prepared) in [
            ("blind supersede", &blind),
            ("lease transfer", &lease),
            ("forked base", &forked),
            ("future seq", &ahead),
        ] {
            let err = writer.commit_prepared(prepared).unwrap_err();
            assert!(matches!(err, L0Error::NotRebasable { .. }), "{name}: {err}");
            assert_eq!(err.to_refusal().code, "FMINE-REF-502", "{name}");
            assert_eq!(std::fs::read(&path).unwrap(), bytes, "{name}");
            assert_eq!(writer.snapshot().unwrap(), state, "{name}");
        }

        // Control: re-stamped by hand, the blind supersede passes plain append and replaces 43 with 44 though its
        // writer never read 43. The combiner's witness rule refuses that lost update.
        let mut by_hand = blind;
        by_hand.seq = 2;
        by_hand.prev_hash = writer.ledger().head_hash("CELL-EVID(test)").unwrap();
        by_hand.anchor.commit_sequence = 2;
        writer.append(&by_hand).unwrap();
        let versions = writer
            .ledger()
            .key_history("CELL-EVID(test)", "grade")
            .unwrap();
        assert_eq!(versions.last().unwrap().payload, b"44");

        // A batch prepared under an earlier lease epoch is refused once a later epoch has committed.
        let head = writer.ledger().head_hash("CELL-EVID(test)").unwrap();
        let old_epoch = semantic_batch(
            3,
            head,
            vec![DeltaOp::new("metal", DeltaKind::Put, b"4400".to_vec())],
            vec![writer.key_witness("grade").unwrap()],
        );
        let mut new_epoch = semantic_batch(
            3,
            head,
            vec![DeltaOp::new("assay", DeltaKind::Put, b"1".to_vec())],
            Vec::new(),
        );
        new_epoch.epoch = 2;
        new_epoch.anchor.ledger_epoch = 2;
        writer.commit_prepared(&new_epoch).unwrap();
        let bytes = std::fs::read(&path).unwrap();
        let err = writer.commit_prepared(&old_epoch).unwrap_err();
        assert!(
            matches!(err, L0Error::Ledger(LedgerError::EpochRegression { .. })),
            "{err}"
        );
        assert_eq!(err.to_refusal().code, "FMINE-REF-502");
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
    }

    #[test]
    fn semantic_refusals_leave_durable_bytes_index_and_obligations_unchanged() {
        let path = scratch_path("semantic-refuse");
        let mut writer = SemanticSegmentWriter::create(&path, "CELL-EVID(test)").unwrap();
        let absent = writer.key_witness("grade").unwrap();
        let first = semantic_batch(
            0,
            GENESIS_PREV_HASH,
            vec![DeltaOp::new("grade", DeltaKind::Put, b"42".to_vec())],
            vec![absent.clone()],
        );
        let head = writer.append(&first).unwrap();
        let bytes = std::fs::read(&path).unwrap();
        let state = writer.snapshot().unwrap();
        let stale = semantic_batch(
            1,
            head,
            vec![DeltaOp::new("next", DeltaKind::Put, b"result".to_vec())],
            vec![absent],
        );
        assert_eq!(
            writer.append(&stale).unwrap_err().to_refusal().code,
            "FMINE-REF-502"
        );
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
        assert_eq!(writer.snapshot().unwrap(), state);
        let mut current = stale;
        current.witnesses = vec![writer.key_witness("grade").unwrap()];
        assert!(matches!(
            writer.append_with_law(&current, |_, _| Err(
                "declared conservation law failed".into()
            )),
            Err(L0Error::ConservationViolation { .. })
        ));
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
        assert_eq!(writer.snapshot().unwrap(), state);

        let invalid = semantic_batch(
            1,
            head,
            vec![
                DeltaOp::new("a-new", DeltaKind::Put, b"must not appear".to_vec()),
                DeltaOp::new("z-missing", DeltaKind::Supersede, Vec::new()),
            ],
            Vec::new(),
        );
        assert!(matches!(
            writer.append(&invalid),
            Err(L0Error::KeyNotFound { .. })
        ));
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
        assert_eq!(writer.snapshot().unwrap(), state);
        writer.append(&current).unwrap();
        assert!(writer.snapshot().unwrap().live_keys.contains_key("next"));
    }

    #[test]
    fn commit_time_law_reads_pre_commit_state_and_refuses_alike_on_both_rungs() {
        // BD-086: one law, run by L0 and by the durable writer. It reads the grade the ledger holds before the
        // commit and refuses a batch that lowers it.
        fn grade_never_falls(state: &L0Ledger, batch: &DeltaBatch) -> Result<(), String> {
            let held = state
                .key_history(&batch.cell_id, "grade")
                .map_err(|e| e.to_string())?;
            let Some(held) = held.last() else {
                return Ok(());
            };
            match batch.deltas.iter().find(|op| op.key == "grade") {
                Some(op) if op.payload < held.payload => Err(format!(
                    "grade falls from {:?} to {:?}",
                    held.payload, op.payload
                )),
                _ => Ok(()),
            }
        }
        let path = scratch_path("law-parity");
        let mut writer = SemanticSegmentWriter::create(&path, "CELL-EVID(test)").unwrap();
        let mut reference = L0Ledger::new();
        let lease = reference.create_cell("CELL-EVID(test)", "writer").unwrap();
        let first = semantic_batch(
            0,
            GENESIS_PREV_HASH,
            vec![DeltaOp::new("grade", DeltaKind::Put, b"42".to_vec())],
            Vec::new(),
        );
        let head = writer.append_with_law(&first, grade_never_falls).unwrap();
        assert_eq!(
            reference
                .commit_with_conservation(&lease, first, grade_never_falls)
                .unwrap(),
            head
        );
        let bytes = std::fs::read(&path).unwrap();
        let state = writer.snapshot().unwrap();
        let read = writer.key_witness("grade").unwrap();
        let supersede = |payload: &[u8]| {
            semantic_batch(
                1,
                head,
                vec![DeltaOp::new(
                    "grade",
                    DeltaKind::Supersede,
                    payload.to_vec(),
                )],
                vec![read.clone()],
            )
        };

        // The law refuses on both rungs with FMINE-REF-101, and neither changes.
        let lower = supersede(b"41");
        let durable = writer
            .append_with_law(&lower, grade_never_falls)
            .unwrap_err();
        let oracle = reference
            .commit_with_conservation(&lease, lower, grade_never_falls)
            .unwrap_err();
        assert_eq!(durable.to_refusal().code, "FMINE-REF-101");
        assert_eq!(oracle.to_refusal().code, "FMINE-REF-101");
        assert_eq!(oracle.to_string(), durable.to_string());
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
        assert_eq!(writer.snapshot().unwrap(), state);
        assert_eq!(reference.as_of_seq("CELL-EVID(test)", 1).unwrap(), state);
        assert_eq!(reference.committed_count("CELL-EVID(test)").unwrap(), 1);

        // A batch the ledger refuses anyway is refused for that reason on both rungs: the law never sees it.
        let duplicate = semantic_batch(
            1,
            head,
            vec![DeltaOp::new("grade", DeltaKind::Put, b"40".to_vec())],
            Vec::new(),
        );
        let durable = writer
            .append_with_law(&duplicate, |_, _| {
                panic!("the law saw an inadmissible batch")
            })
            .unwrap_err();
        let oracle = reference
            .commit_with_conservation(&lease, duplicate, |_, _| {
                panic!("the law saw an inadmissible batch")
            })
            .unwrap_err();
        assert!(matches!(durable, L0Error::KeyAlreadyExists { .. }));
        assert_eq!(oracle.to_refusal().code, durable.to_refusal().code);
        assert_eq!(std::fs::read(&path).unwrap(), bytes);

        // A batch the law admits commits on both with one digest and one state.
        let higher = supersede(b"43");
        let digest = writer.append_with_law(&higher, grade_never_falls).unwrap();
        assert_eq!(
            reference
                .commit_with_conservation(&lease, higher, grade_never_falls)
                .unwrap(),
            digest
        );
        assert_eq!(
            writer.snapshot().unwrap(),
            reference.as_of_seq("CELL-EVID(test)", 2).unwrap()
        );
    }

    #[test]
    fn semantic_open_refuses_hash_valid_stale_history_without_changing_it() {
        let path = scratch_path("semantic-corrupt");
        let mut state = L0Ledger::new();
        let lease = state.create_cell("CELL-EVID(test)", "writer").unwrap();
        let absent = state.key_witness("CELL-EVID(test)", "grade").unwrap();
        let first = semantic_batch(
            0,
            GENESIS_PREV_HASH,
            vec![DeltaOp::new("grade", DeltaKind::Put, b"42".to_vec())],
            vec![absent.clone()],
        );
        let mut raw =
            SegmentWriter::create(&path, "CELL-EVID(test)", 0, GENESIS_PREV_HASH, Vec::new())
                .unwrap();
        let head = raw.append(&first).unwrap();
        state.commit(&lease, first).unwrap();
        let stale = semantic_batch(
            1,
            head,
            vec![DeltaOp::new(
                "result",
                DeltaKind::Put,
                b"invalid derivation".to_vec(),
            )],
            vec![absent],
        );
        raw.append(&stale).unwrap();
        drop(raw);
        let bytes = std::fs::read(&path).unwrap();
        let inspected = SegmentReader::open(&path).unwrap();
        assert_eq!(inspected.batches.len(), 2);
        assert!(matches!(
            inspected.replay_semantics(),
            Err(L0Error::KeyWitnessMismatch { .. })
        ));
        assert!(matches!(
            SemanticSegmentWriter::open_append(&path),
            Err(L0Error::KeyWitnessMismatch { .. })
        ));
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
    }

    #[test]
    fn unacknowledged_complete_append_replays_once_and_retries_cannot_duplicate_it() {
        let path = scratch_path("semantic-unacknowledged");
        let mut writer = SemanticSegmentWriter::create(&path, "CELL-EVID(test)").unwrap();
        let batch = semantic_batch(
            0,
            GENESIS_PREV_HASH,
            vec![
                DeltaOp::new("grade", DeltaKind::Put, b"42".to_vec()),
                DeltaOp::new("OB-001", DeltaKind::ReserveObligation, Vec::new()),
            ],
            vec![writer.key_witness("grade").unwrap()],
        );
        // FACT: a real, fully synced frame, followed by process loss before index admission.
        writer.writer.append(&batch).unwrap();
        assert_eq!(
            writer.ledger().committed_count(writer.cell_id()).unwrap(),
            0
        );
        drop(writer);
        let bytes = std::fs::read(&path).unwrap();
        let mut reopened = SemanticSegmentWriter::open_append(&path).unwrap();
        let snapshot = reopened.snapshot().unwrap();
        assert_eq!(snapshot.committed_count, 1);
        assert_eq!(snapshot.live_keys.len(), 1);
        assert!(snapshot.open_obligations.contains("OB-001"));
        assert!(reopened.append(&batch).is_err());
        assert_eq!(reopened.snapshot().unwrap(), snapshot);
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
    }

    #[test]
    fn failed_storage_write_requires_reopen_and_never_advances_the_semantic_index() {
        let path = scratch_path("semantic-io-failure");
        let mut writer = SemanticSegmentWriter::create(&path, "CELL-EVID(test)").unwrap();
        let batch = semantic_batch(
            0,
            GENESIS_PREV_HASH,
            vec![DeltaOp::new("grade", DeltaKind::Put, b"42".to_vec())],
            Vec::new(),
        );
        let before = std::fs::read(&path).unwrap();
        // DESIGN: fault injection through a real read-only descriptor; writes return an OS error.
        let readonly = File::open(&path).unwrap();
        writer.writer.file.0 = readonly;
        assert!(matches!(
            writer.append(&batch),
            Err(L0Error::Ledger(LedgerError::Io { .. }))
        ));
        assert_eq!(
            writer.ledger().committed_count(writer.cell_id()).unwrap(),
            0
        );
        assert_eq!(std::fs::read(&path).unwrap(), before);
        assert!(matches!(
            writer.append(&batch),
            Err(L0Error::Ledger(LedgerError::Io {
                operation: "resume_after_failed_commit",
                ..
            }))
        ));
        assert!(writer.seal().is_err());
        let mut reopened = SemanticSegmentWriter::open_append(&path).unwrap();
        reopened.append(&batch).unwrap();
        assert_eq!(
            reopened.snapshot().unwrap().live_keys["grade"].payload,
            b"42"
        );
    }

    #[test]
    fn test_segment_write_seal_verify_mmr_proofs_and_reopen() {
        let path = scratch_path("seal");
        let mut writer = SegmentWriter::create(
            &path,
            "CELL-SITE(lumwana)",
            0,
            GENESIS_PREV_HASH,
            Vec::new(),
        )
        .expect("create segment");

        let mut prev = GENESIS_PREV_HASH;
        let mut digests = Vec::new();
        for seq in 0..5u64 {
            let epoch = if seq < 3 { 1 } else { 2 };
            let b = sample_batch("CELL-SITE(lumwana)", epoch, seq, prev);
            prev = writer.append(&b).expect("append batch");
            digests.push(prev);
        }
        let footer = writer.seal().expect("seal segment");
        assert_eq!(footer.record_count, 5);
        assert_eq!(footer.first_seq, 0);
        assert_eq!(footer.last_seq, 4);
        assert_eq!(footer.last_epoch, 2);
        assert_eq!(footer.head_hash, prev);
        assert_eq!(root_from_peaks(&footer.mmr_peaks), footer.mmr_root);

        let inspected = SegmentReader::open(&path).expect("read sealed segment");
        assert_eq!(inspected.state, SegmentState::Sealed(footer.clone()));
        assert_eq!(inspected.batches.len(), 5);
        assert_eq!(inspected.batch_digests, digests);

        // Verify RFC 6962 inclusion & consistency proofs straight from replayed segment MMR.
        for (i, d) in digests.iter().enumerate() {
            let proof = inspected
                .mmr
                .inclusion_proof(i as u64, 5)
                .expect("inclusion proof");
            verify_inclusion(
                i as u64,
                5,
                &leaf_hash(d.as_bytes()),
                &footer.mmr_root,
                &proof,
            )
            .expect("verify inclusion");
        }
        let root_3 = inspected.mmr.root_at(3).expect("root at 3");
        let cproof = inspected
            .mmr
            .consistency_proof(3, 5)
            .expect("consistency proof");
        verify_consistency(3, 5, &root_3, &footer.mmr_root, &cproof).expect("verify consistency");

        // Appending to a sealed segment is refused.
        assert!(SegmentWriter::open_append(&path).is_err());
        let _ = std::fs::remove_dir_all(path.parent().expect("parent"));
    }

    #[test]
    fn test_every_post_header_truncation_is_clean_or_recoverable_torn_tail_and_interior_flip_refuses()
     {
        let path = scratch_path("torn");
        let mut writer = SegmentWriter::create(
            &path,
            "CELL-PUBLIC(barrick)",
            0,
            GENESIS_PREV_HASH,
            Vec::new(),
        )
        .expect("create");
        let b0 = sample_batch("CELL-PUBLIC(barrick)", 1, 0, GENESIS_PREV_HASH);
        let d0 = writer.append(&b0).expect("b0");
        let b1 = sample_batch("CELL-PUBLIC(barrick)", 1, 1, d0);
        let d1 = writer.append(&b1).expect("b1");
        let b2 = sample_batch("CELL-PUBLIC(barrick)", 1, 2, d1);
        let _d2 = writer.append(&b2).expect("b2");
        let _footer = writer.seal().expect("seal");

        let full = std::fs::read(&path).expect("read bytes");
        let header_end = 4 + read_u32_le(&full, 0) as usize;
        assert!(header_end < full.len());

        // Truncate at EVERY byte after the header: must be OpenUnsealed (on exact boundary),
        // Sealed (full length), or TornTail (mid-frame). Never panics, never loses verified prefix!
        let mut torn_seen = 0usize;
        for cut in header_end..=full.len() {
            let res = inspect_segment_bytes(&full[..cut]).expect("post-header prefix inspects");
            match res.state {
                SegmentState::TornTail {
                    torn_offset,
                    torn_bytes,
                    ..
                } => {
                    assert_eq!(torn_offset, res.verified_bytes);
                    assert_eq!(torn_offset + torn_bytes, cut as u64);
                    torn_seen += 1;
                }
                SegmentState::OpenUnsealed | SegmentState::Sealed(_) => {
                    assert_eq!(res.verified_bytes, cut as u64);
                }
            }
        }
        assert!(torn_seen > 100, "expected many torn tail offsets");

        // Flipping a byte inside batch 0 (interior, with batches 1, 2, footer after it) MUST refuse
        // as non-repairable InteriorCorruption — never silently truncate batches 1 and 2!
        let mut interior = full.clone();
        interior[header_end + 20] ^= 0x40;
        assert!(matches!(
            inspect_segment_bytes(&interior),
            Err(LedgerError::InteriorCorruption { .. })
        ));

        let _ = std::fs::remove_dir_all(path.parent().expect("parent"));
    }

    /// A crash can persist the later sectors of an append and lose the one holding its length prefix
    /// (`F7`). The frame then follows a zero length, and the anchor it embeds is a frame of its own;
    /// neither is a record, so this is a torn tail to repair, not interior corruption.
    #[test]
    fn test_lost_length_prefix_with_persisted_frame_is_torn_tail() {
        let path = scratch_path("lostlen");
        let mut writer = SegmentWriter::create(
            &path,
            "CELL-SITE(lumwana)",
            0,
            GENESIS_PREV_HASH,
            Vec::new(),
        )
        .expect("create");
        let d0 = writer
            .append(&sample_batch("CELL-SITE(lumwana)", 1, 0, GENESIS_PREV_HASH))
            .expect("b0");
        drop(writer);
        let clean = std::fs::read(&path).expect("read");
        let b1 = sample_batch("CELL-SITE(lumwana)", 1, 1, d0)
            .to_bytes()
            .expect("b1 bytes");
        assert!(
            b1[4..].windows(4).any(|w| w == b"FMNE"),
            "the anchor is a nested frame"
        );

        for lost_head in [0, 8, 64] {
            let mut torn = clean.clone();
            torn.extend_from_slice(&[0u8; 4]);
            torn.extend_from_slice(&[0u8; 64][..lost_head]);
            torn.extend_from_slice(&b1[lost_head..]);
            let inspected = inspect_segment_bytes(&torn).expect("a torn tail, not interior");
            assert!(
                matches!(inspected.state, SegmentState::TornTail { .. }),
                "lost_head {lost_head}: {:?}",
                inspected.state
            );
            assert_eq!(inspected.verified_bytes, clean.len() as u64);
            assert_eq!(inspected.batches.len(), 1);
        }
        let _ = std::fs::remove_dir_all(path.parent().expect("parent"));
    }

    /// A damaged length prefix on an interior record that points past the end of the file must not read as
    /// a torn tail, or repair would truncate the whole records after it (`INV-003`).
    #[test]
    fn test_overlong_interior_length_prefix_refuses_instead_of_truncating() {
        let path = scratch_path("overlong");
        let mut writer = SegmentWriter::create(
            &path,
            "CELL-SITE(lumwana)",
            0,
            GENESIS_PREV_HASH,
            Vec::new(),
        )
        .expect("create");
        let mut prev = GENESIS_PREV_HASH;
        for seq in 0..3 {
            prev = writer
                .append(&sample_batch("CELL-SITE(lumwana)", 1, seq, prev))
                .expect("append");
        }
        writer.seal().expect("seal");
        let full = std::fs::read(&path).expect("read");
        let header_end = 4 + read_u32_le(&full, 0) as usize;

        let mut damaged = full.clone();
        let overlong = u32::try_from(full.len()).expect("small file");
        damaged[header_end..header_end + 4].copy_from_slice(&overlong.to_le_bytes());
        match inspect_segment_bytes(&damaged) {
            Err(LedgerError::InteriorCorruption { offset, .. }) => {
                assert_eq!(offset, header_end as u64);
            }
            other => panic!("expected interior corruption, got {other:?}"),
        }
        let _ = std::fs::remove_dir_all(path.parent().expect("parent"));
    }

    /// Candidate frames after damage can overlap, and each check computes a CRC over its whole declared
    /// length. A crafted tail of overlapping candidates is refused within the check budget, so a hostile
    /// segment cannot make inspection take time quadratic in its size.
    #[test]
    fn test_crafted_overlapping_candidates_refuse_within_the_check_budget() {
        const UNITS: usize = 4096;
        let path = scratch_path("crafted");
        let mut writer = SegmentWriter::create(
            &path,
            "CELL-SITE(lumwana)",
            0,
            GENESIS_PREV_HASH,
            Vec::new(),
        )
        .expect("create");
        writer
            .append(&sample_batch("CELL-SITE(lumwana)", 1, 0, GENESIS_PREV_HASH))
            .expect("b0");
        drop(writer);
        let clean = std::fs::read(&path).expect("read");

        // A zero length prefix, then 16-byte units: a length prefix declaring a frame that runs to the end of
        // the file, and the start of a batch frame. Each unit is a candidate overlapping every unit after it.
        let total = clean.len() + 4 + 16 * UNITS;
        let mut crafted = clean.clone();
        crafted.extend_from_slice(&[0u8; 4]);
        for _ in 0..UNITS {
            let magic = crafted.len() + 4;
            let frame_len = u32::try_from(total - magic).expect("small file");
            crafted.extend_from_slice(&frame_len.to_le_bytes());
            crafted.extend_from_slice(b"FMNE");
            crafted.extend_from_slice(&TAG_LEDGER_BATCH);
            crafted.extend_from_slice(&[0u8; 4]);
        }
        assert_eq!(crafted.len(), total);

        let cursor = clean.len();
        assert_eq!(
            after_damage(&crafted, cursor),
            AfterDamage::TooManyCandidates
        );
        match inspect_segment_bytes(&crafted) {
            Err(LedgerError::InteriorCorruption { offset, .. }) => {
                assert_eq!(offset, cursor as u64);
            }
            other => panic!("expected interior corruption, got {other:?}"),
        }
        let _ = std::fs::remove_dir_all(path.parent().expect("parent"));
    }

    /// A few stray bytes before a whole record put its length prefix one to three bytes after the damage,
    /// overlapping the unreadable length read there. That record still follows the damage, so the segment is
    /// refused, never truncated before it (`INV-003`).
    #[test]
    fn test_stray_bytes_before_a_whole_record_refuse_instead_of_truncating() {
        let path = scratch_path("stray");
        let mut writer = SegmentWriter::create(
            &path,
            "CELL-SITE(lumwana)",
            0,
            GENESIS_PREV_HASH,
            Vec::new(),
        )
        .expect("create");
        let mut prev = GENESIS_PREV_HASH;
        let mut ends = Vec::new();
        for seq in 0..2 {
            prev = writer
                .append(&sample_batch("CELL-SITE(lumwana)", 1, seq, prev))
                .expect("append");
            ends.push(std::fs::metadata(&path).expect("len").len() as usize);
        }
        writer.seal().expect("seal");
        let full = std::fs::read(&path).expect("read");

        // Before the second batch, then before the footer.
        for at in ends {
            for stray in 1..=7_usize {
                let mut damaged = full[..at].to_vec();
                damaged.extend(std::iter::repeat_n(0xA5, stray));
                damaged.extend_from_slice(&full[at..]);
                match inspect_segment_bytes(&damaged) {
                    Err(LedgerError::InteriorCorruption { offset, .. }) => {
                        assert_eq!(offset, at as u64, "{stray} stray bytes at {at}");
                    }
                    other => panic!(
                        "{stray} stray bytes at {at}: expected interior corruption, got {other:?}"
                    ),
                }
            }
        }
        let _ = std::fs::remove_dir_all(path.parent().expect("parent"));
    }

    /// A last record whose frame CRC verifies was written whole, even if this build cannot decode it (here a
    /// newer `FMT-LEDGER-1` version). It is refused, never read as a torn tail that repair would truncate.
    #[test]
    fn test_whole_last_record_this_build_cannot_decode_refuses_instead_of_truncating() {
        let path = scratch_path("newer");
        let mut writer = SegmentWriter::create(
            &path,
            "CELL-SITE(lumwana)",
            0,
            GENESIS_PREV_HASH,
            Vec::new(),
        )
        .expect("create");
        let d0 = writer
            .append(&sample_batch("CELL-SITE(lumwana)", 1, 0, GENESIS_PREV_HASH))
            .expect("b0");
        drop(writer);
        let clean = std::fs::read(&path).expect("read");

        let mut newer = sample_batch("CELL-SITE(lumwana)", 1, 1, d0)
            .to_bytes()
            .expect("b1 bytes");
        newer[8..10].copy_from_slice(&(FMT_LEDGER_V2 + 1).to_le_bytes());
        let body_end = newer.len() - 4;
        let crc = crc32c(&newer[..body_end]);
        newer[body_end..].copy_from_slice(&crc.to_le_bytes());
        assert!(matches!(
            DeltaBatch::from_bytes(&newer),
            Err(LedgerError::Codec(CodecError::UnsupportedVersion { .. }))
        ));

        let mut extended = clean.clone();
        extended.extend_from_slice(&u32::try_from(newer.len()).expect("small").to_le_bytes());
        extended.extend_from_slice(&newer);
        match inspect_segment_bytes(&extended) {
            Err(LedgerError::InteriorCorruption { offset, .. }) => {
                assert_eq!(offset, clean.len() as u64);
            }
            other => panic!("expected interior corruption, got {other:?}"),
        }
        let _ = std::fs::remove_dir_all(path.parent().expect("parent"));
    }
}
