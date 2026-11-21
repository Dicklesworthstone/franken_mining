//! Append-only `FMT-SEG-1` segment files (`BD-082`, plan §26.5, L1 production persistence).
//!
//! Layout on disk:
//! 1. `len: u32 (LE)` + Segment Header frame (`FMNE` + `SEGH` + version + `cell_id` + `first_seq` +
//!    `prev_segment_head` + `prev_mmr_peaks` + CRC32C).
//! 2. Zero or more `len: u32 (LE)` + `FMT-LEDGER-1` (`FMNE` + `LDGR` + ... + CRC32C) batch frames
//!    chaining `seq`, `prev_hash`, and monotonic fencing `epoch`.
//! 3. Optional sealed `len: u32 (LE)` + Segment Footer frame (`FMNE` + `SEGF` + version + counts +
//!    `head_hash` + `mmr_root` + `mmr_peaks` + CRC32C).

use std::fs::{File, OpenOptions, TryLockError};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;

use fmine_core::codec::{CodecError, Decoder, Encoder, MAX_FRAME_BYTES};
use fmine_core::hash::{Digest32, sha256};

use crate::batch::{DeltaBatch, GENESIS_PREV_HASH, LedgerError, MAX_ID_BYTES, TAG_LEDGER_BATCH};
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
                        "truncated trailing frame: declared {frame_len} payload bytes, only {} available",
                        remaining.saturating_sub(4)
                    ),
                },
            });
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
                    if is_last_frame {
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
                                reason: format!("corrupt or torn trailing footer frame: {err}"),
                            },
                        });
                    }
                    return Err(LedgerError::InteriorCorruption {
                        offset: cursor as u64,
                        detail: format!("corrupt interior footer frame: {err}"),
                    });
                }
            }
        }

        if &frame[0..4] != b"FMNE" || tag != TAG_LEDGER_BATCH {
            if is_last_frame {
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
                        reason: "corrupt trailing frame magic or tag".to_string(),
                    },
                });
            }
            return Err(LedgerError::InteriorCorruption {
                offset: cursor as u64,
                detail: "corrupt interior frame magic or tag".to_string(),
            });
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
                if is_last_frame {
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
                            reason: format!("corrupt trailing batch frame: {err}"),
                        },
                    });
                }
                return Err(LedgerError::InteriorCorruption {
                    offset: cursor as u64,
                    detail: format!("corrupt interior batch frame: {err}"),
                });
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
    // Only treat a bad length prefix as a torn tail if no further `FMNE` frame magic appears after it.
    let tail = &bytes[cursor + 4..];
    if tail.windows(4).any(|w| w == b"FMNE") {
        return Err(LedgerError::InteriorCorruption {
            offset: cursor as u64,
            detail: format!("{reason} with subsequent FMNE frames present"),
        });
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
        })
    }

    /// Re-open an unsealed clean `FMT-SEG-1` segment for appending under an exclusive lock.
    /// Refuses sealed segments, torn segments, or corrupted segments (forcing `repair plan` first).
    pub fn open_append(path: &Path) -> Result<Self, LedgerError> {
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
                Ok(Self {
                    file,
                    header: inspected.header,
                    next_seq,
                    head_hash: inspected.head_hash,
                    last_epoch: inspected.last_epoch,
                    record_count,
                    mmr: inspected.mmr,
                    sealed: false,
                })
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
        let digest = sha256(&frame);
        write_framed(self.file.file_mut(), &frame)?;
        self.mmr
            .push(digest.as_bytes())
            .map_err(|e| LedgerError::ChainContinuity {
                detail: format!("MMR push failed: {e}"),
            })?;
        self.last_epoch = batch.epoch;
        self.head_hash = digest;
        self.record_count =
            self.record_count
                .checked_add(1)
                .ok_or_else(|| LedgerError::ChainContinuity {
                    detail: "record_count overflow".to_string(),
                })?;
        self.next_seq =
            self.next_seq
                .checked_add(1)
                .ok_or_else(|| LedgerError::ChainContinuity {
                    detail: "sequence overflow".to_string(),
                })?;
        Ok(digest)
    }

    /// Seal the segment by writing its `SEGF` MMR peak footer and syncing to disk.
    pub fn seal(mut self) -> Result<SegmentFooter, LedgerError> {
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
    use crate::mmr::{leaf_hash, verify_consistency, verify_inclusion};

    fn scratch_path(tag: &str) -> std::path::PathBuf {
        let dir =
            std::env::temp_dir().join(format!("fmine-ledger-seg-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("scratch dir");
        dir.join("seg-000001.fmseg")
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
}
