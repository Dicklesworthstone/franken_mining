//! Hash-chain verification and deterministic `F7` torn-segment repair plan/apply (`BD-088`,
//! plan §26.11 F7, §30.2 `PO-ADIT-3`).
//!
//! History is never rewritten (`INV-003`):
//! - Clean sealed or unsealed segments produce a `NoOp` repair plan.
//! - A crash-torn trailing frame (`F7`) produces a `TruncateTornTail` repair plan with a canonical
//!   SHA-256 `plan_digest` binding path, original digest, verified byte prefix, retained record
//!   count, verified chain head, and the quarantine evidence file path (`<seg>.torn.<sha12>.evidence`).
//! - Applying a repair plan requires the exact matching `plan_digest` (`PO-ADIT-3`), keeps the
//!   complete original bytes in the evidence file FIRST ([`retain_repair_evidence`]), then truncates
//!   the segment to `verified_bytes`, syncs, and re-verifies from byte 0. A crash at any step leaves
//!   either the torn segment, which plans the same repair again, or the repaired one.
//! - Interior corruption, hash-chain breaks, fencing regressions, and tampered footers are REFUSED
//!   at both plan and apply time.

use std::fs::{File, OpenOptions};
use std::io::{ErrorKind, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use fmine_core::codec::Encoder;
use fmine_core::hash::{Digest32, sha256};

use crate::batch::LedgerError;
use crate::segment::{
    LockedFile, SegmentState, inspect_segment_bytes, read_bounded, sync_parent_dir,
};

const TAG_REPAIR_PLAN: [u8; 4] = *b"RPLN";
const FMT_REPAIR_PLAN_V1: u16 = 1;

/// Read-only verification summary for an `FMT-SEG-1` segment (`fmine doctor`, `BD-088`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SegmentVerification {
    /// Path inspected.
    pub path: PathBuf,
    /// Authority cell recorded in the segment header.
    pub cell_id: String,
    /// Lifecycle status (`"sealed"`, `"open_unsealed"`, or `"torn_tail"`).
    pub status: &'static str,
    /// Number of fully verified batches.
    pub record_count: u64,
    /// First sequence number in the segment header.
    pub first_seq: u64,
    /// Last verified sequence number (`None` if zero batches).
    pub last_seq: Option<u64>,
    /// Highest verified fencing epoch (`0` if zero batches).
    pub last_epoch: u64,
    /// Verified SHA-256 chain head.
    pub head_hash: Digest32,
    /// Verified RFC 6962 MMR root over the verified batch digests.
    pub mmr_root: Digest32,
    /// SHA-256 digest of the complete file bytes as read.
    pub file_sha256: Digest32,
    /// Verified prefix byte length.
    pub verified_bytes: u64,
    /// Total file byte length.
    pub total_bytes: u64,
    /// Trailing torn bytes (`0` when clean).
    pub torn_bytes: u64,
    /// Human-readable diagnostic note when `torn_bytes > 0`.
    pub issue: Option<String>,
}

/// Deterministic action described by a [`SegmentRepairPlan`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RepairAction {
    /// Segment is already clean; no modification needed.
    NoOp,
    /// Copy complete original bytes to `evidence_path`, then truncate to `verified_bytes` (`F7`).
    TruncateTornTail,
}

impl RepairAction {
    /// Stable lowercase identifier for JSON output and digest binding.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NoOp => "no_op",
            Self::TruncateTornTail => "truncate_torn_tail",
        }
    }
}

/// Deterministic `F7` repair plan bound by its canonical SHA-256 `plan_digest` (`PO-ADIT-3`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SegmentRepairPlan {
    /// Canonical SHA-256 digest binding every field of this plan.
    pub plan_digest: Digest32,
    /// Target segment path.
    pub target_path: PathBuf,
    /// Authority cell recorded in the segment header.
    pub cell_id: String,
    /// Planned action (`NoOp` or `TruncateTornTail`).
    pub action: RepairAction,
    /// SHA-256 digest of the original on-disk bytes at plan time.
    pub original_sha256: Digest32,
    /// Total byte length on disk at plan time.
    pub original_bytes: u64,
    /// Clean byte length that will remain after truncation.
    pub verified_bytes: u64,
    /// Number of trailing torn bytes removed (`0` for `NoOp`).
    pub torn_bytes: u64,
    /// Number of verified batches retained.
    pub retained_records: u64,
    /// Verified chain head hash retained.
    pub last_verified_head: Digest32,
    /// Verified MMR root retained.
    pub last_verified_mmr_root: Digest32,
    /// Path where complete original bytes will be archived before truncation (`None` for `NoOp`).
    pub evidence_path: Option<PathBuf>,
    /// Reason the repair is needed (or `"segment is clean"`).
    pub reason: String,
}

/// Receipt returned after [`apply_segment_repair`] completes and re-verifies the repaired segment.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SegmentRepairReceipt {
    /// Verified plan digest that was executed.
    pub plan_digest: Digest32,
    /// Action executed.
    pub action: RepairAction,
    /// Target segment path.
    pub target_path: PathBuf,
    /// Path where the complete pre-repair original bytes were preserved (`None` for `NoOp`).
    pub evidence_path: Option<PathBuf>,
    /// SHA-256 of the preserved original bytes.
    pub original_sha256: Digest32,
    /// SHA-256 of the segment bytes after repair.
    pub repaired_sha256: Digest32,
    /// Final verified byte length.
    pub final_bytes: u64,
    /// Number of verified batches retained.
    pub retained_records: u64,
    /// Re-verified chain head hash after repair.
    pub verified_head: Digest32,
    /// Re-verified MMR root after repair.
    pub verified_mmr_root: Digest32,
}

/// Verify an `FMT-SEG-1` segment file on disk under a shared lock (`BD-088`).
pub fn verify_segment(path: &Path) -> Result<SegmentVerification, LedgerError> {
    let raw = File::open(path).map_err(|e| LedgerError::Io {
        operation: "open_verify_segment",
        detail: format!("{}: {e}", path.display()),
    })?;
    let mut locked = LockedFile::acquire(raw, false)?;
    let bytes = read_bounded(locked.file_mut(), path)?;
    verify_segment_bytes(path, &bytes)
}

/// Compute a deterministic `F7` repair plan for an `FMT-SEG-1` segment file (`BD-088`, `PO-ADIT-3`).
///
/// Refuses non-repairable interior corruption, hash-chain breaks, epoch regressions, or tampered footers.
pub fn plan_segment_repair(path: &Path) -> Result<SegmentRepairPlan, LedgerError> {
    let raw = File::open(path).map_err(|e| LedgerError::Io {
        operation: "open_plan_segment",
        detail: format!("{}: {e}", path.display()),
    })?;
    let mut locked = LockedFile::acquire(raw, false)?;
    let bytes = read_bounded(locked.file_mut(), path)?;
    plan_from_bytes(path, &bytes)
}

/// Apply a verified [`SegmentRepairPlan`] by its exact `expected_plan_digest` (`PO-ADIT-3`, `F7`).
///
/// Refuses with [`LedgerError::RepairDigestMismatch`] (touching zero bytes) if the segment changed
/// since planning or the supplied digest does not match.
pub fn apply_segment_repair(
    path: &Path,
    expected_plan_digest: Digest32,
) -> Result<SegmentRepairReceipt, LedgerError> {
    let raw = OpenOptions::new()
        .read(true)
        .write(true)
        .open(path)
        .map_err(|e| LedgerError::Io {
            operation: "open_apply_segment",
            detail: format!("{}: {e}", path.display()),
        })?;
    let mut locked = LockedFile::acquire(raw, true)?;
    let original_bytes = read_bounded(locked.file_mut(), path)?;
    let plan = plan_from_bytes(path, &original_bytes)?;
    if plan.plan_digest != expected_plan_digest {
        return Err(LedgerError::RepairDigestMismatch {
            expected: expected_plan_digest,
            actual: plan.plan_digest,
        });
    }

    if plan.action == RepairAction::NoOp {
        return Ok(SegmentRepairReceipt {
            plan_digest: plan.plan_digest,
            action: RepairAction::NoOp,
            target_path: plan.target_path,
            evidence_path: None,
            original_sha256: plan.original_sha256,
            repaired_sha256: plan.original_sha256,
            final_bytes: plan.original_bytes,
            retained_records: plan.retained_records,
            verified_head: plan.last_verified_head,
            verified_mmr_root: plan.last_verified_mmr_root,
        });
    }

    let Some(evidence_path) = plan.evidence_path.clone() else {
        return Err(LedgerError::InvalidField {
            field: "evidence_path",
            detail: "truncation plan lacked an evidence archive path".to_string(),
        });
    };

    // Retain complete original bytes BEFORE touching the target segment (`F7`).
    retain_repair_evidence(&evidence_path, &original_bytes)?;

    // Truncate target segment to verified prefix and fsync.
    locked
        .file_mut()
        .set_len(plan.verified_bytes)
        .and_then(|()| locked.file_mut().sync_all())
        .map_err(|e| LedgerError::Io {
            operation: "truncate_segment",
            detail: format!("{}: {e}", path.display()),
        })?;

    // Re-read and re-verify the repaired file from byte 0 while still holding the exclusive lock.
    locked
        .file_mut()
        .seek(SeekFrom::Start(0))
        .map_err(|e| LedgerError::Io {
            operation: "seek_verify",
            detail: e.to_string(),
        })?;
    let repaired_bytes = read_bounded(locked.file_mut(), path)?;
    let verified = verify_segment_bytes(path, &repaired_bytes)?;
    if verified.torn_bytes != 0 || verified.head_hash != plan.last_verified_head {
        return Err(LedgerError::InteriorCorruption {
            offset: verified.verified_bytes,
            detail: "post-truncation re-verification did not match planned verified prefix"
                .to_string(),
        });
    }

    Ok(SegmentRepairReceipt {
        plan_digest: plan.plan_digest,
        action: RepairAction::TruncateTornTail,
        target_path: plan.target_path,
        evidence_path: Some(evidence_path),
        original_sha256: plan.original_sha256,
        repaired_sha256: verified.file_sha256,
        final_bytes: verified.total_bytes,
        retained_records: verified.record_count,
        verified_head: verified.head_hash,
        verified_mmr_root: verified.mmr_root,
    })
}

/// Keeps `original` at `evidence_path`, complete and synced, before a repair touches its target (`F7`).
///
/// The caller holds the target's exclusive lock. A crash between this step and the truncation leaves the
/// target as it was, so the retry plans the same repair and the same evidence path: a file there that
/// already holds exactly `original` is kept as it is, and any other file there is refused. The bytes are
/// written first to `<evidence_path>.partial`, synced, and renamed into place, so `evidence_path` never
/// holds a partial copy. A `.partial` file left by a crash is a partial copy and is replaced; it is
/// removed and created afresh, so a link planted at that name is never followed.
pub fn retain_repair_evidence(evidence_path: &Path, original: &[u8]) -> Result<(), LedgerError> {
    let io = |operation: &'static str, path: &Path, e: std::io::Error| LedgerError::Io {
        operation,
        detail: format!("{}: {e}", path.display()),
    };
    match File::open(evidence_path) {
        Ok(existing) => {
            let mut kept = Vec::with_capacity(original.len());
            existing
                .take(original.len() as u64 + 1)
                .read_to_end(&mut kept)
                .map_err(|e| io("read_evidence_file", evidence_path, e))?;
            if kept == original {
                return Ok(());
            }
            return Err(LedgerError::InvalidField {
                field: "evidence_path",
                detail: format!(
                    "{} already exists and does not hold this repair's original bytes",
                    evidence_path.display()
                ),
            });
        }
        Err(e) if e.kind() == ErrorKind::NotFound => {}
        Err(e) => return Err(io("open_evidence_file", evidence_path, e)),
    }

    let mut partial = evidence_path.as_os_str().to_os_string();
    partial.push(".partial");
    let partial = PathBuf::from(partial);
    let mut opts = OpenOptions::new();
    opts.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        opts.mode(0o600);
    }
    let mut file = match opts.open(&partial) {
        Err(e) if e.kind() == ErrorKind::AlreadyExists => {
            std::fs::remove_file(&partial)
                .map_err(|e| io("remove_stale_partial_evidence", &partial, e))?;
            opts.open(&partial)
        }
        other => other,
    }
    .map_err(|e| io("create_partial_evidence", &partial, e))?;
    file.write_all(original)
        .and_then(|()| file.sync_all())
        .map_err(|e| io("write_partial_evidence", &partial, e))?;
    drop(file);
    std::fs::rename(&partial, evidence_path)
        .map_err(|e| io("rename_evidence_into_place", evidence_path, e))?;
    sync_parent_dir(evidence_path)
}

fn verify_segment_bytes(path: &Path, bytes: &[u8]) -> Result<SegmentVerification, LedgerError> {
    let inspected = inspect_segment_bytes(bytes)?;
    let (status, torn_bytes, issue) = match &inspected.state {
        SegmentState::Sealed(_) => ("sealed", 0, None),
        SegmentState::OpenUnsealed => ("open_unsealed", 0, None),
        SegmentState::TornTail {
            torn_bytes, reason, ..
        } => ("torn_tail", *torn_bytes, Some(reason.clone())),
    };
    Ok(SegmentVerification {
        path: path.to_path_buf(),
        cell_id: inspected.header.cell_id,
        status,
        record_count: inspected.batches.len() as u64,
        first_seq: inspected.header.first_seq,
        last_seq: inspected.batches.last().map(|b| b.seq),
        last_epoch: inspected.last_epoch,
        head_hash: inspected.head_hash,
        mmr_root: inspected.mmr.root(),
        file_sha256: sha256(bytes),
        verified_bytes: inspected.verified_bytes,
        total_bytes: inspected.total_bytes,
        torn_bytes,
        issue,
    })
}

fn plan_from_bytes(path: &Path, bytes: &[u8]) -> Result<SegmentRepairPlan, LedgerError> {
    let inspected = inspect_segment_bytes(bytes)?;
    let original_sha256 = sha256(bytes);
    let (action, torn_bytes, evidence_path, reason) = match &inspected.state {
        SegmentState::Sealed(_) => (
            RepairAction::NoOp,
            0,
            None,
            "sealed segment verified cleanly".to_string(),
        ),
        SegmentState::OpenUnsealed => (
            RepairAction::NoOp,
            0,
            None,
            "unsealed segment verified cleanly on record boundary".to_string(),
        ),
        SegmentState::TornTail {
            torn_bytes, reason, ..
        } => (
            RepairAction::TruncateTornTail,
            *torn_bytes,
            Some(evidence_path_for(path, &original_sha256)),
            reason.clone(),
        ),
    };
    let mmr_root = inspected.mmr.root();
    let retained_records = inspected.batches.len() as u64;
    let plan_digest = compute_plan_digest(
        path,
        &inspected.header.cell_id,
        action,
        &original_sha256,
        inspected.total_bytes,
        inspected.verified_bytes,
        retained_records,
        &inspected.head_hash,
        &mmr_root,
        evidence_path.as_deref(),
    );
    Ok(SegmentRepairPlan {
        plan_digest,
        target_path: path.to_path_buf(),
        cell_id: inspected.header.cell_id,
        action,
        original_sha256,
        original_bytes: inspected.total_bytes,
        verified_bytes: inspected.verified_bytes,
        torn_bytes,
        retained_records,
        last_verified_head: inspected.head_hash,
        last_verified_mmr_root: mmr_root,
        evidence_path,
        reason,
    })
}

fn evidence_path_for(path: &Path, original_sha256: &Digest32) -> PathBuf {
    let short_hex = &original_sha256.to_hex()[..12];
    let mut os = path.as_os_str().to_os_string();
    os.push(format!(".torn.{short_hex}.evidence"));
    PathBuf::from(os)
}

#[allow(
    clippy::too_many_arguments,
    reason = "canonical repair plan digest binding"
)]
fn compute_plan_digest(
    target_path: &Path,
    cell_id: &str,
    action: RepairAction,
    original_sha256: &Digest32,
    original_bytes: u64,
    verified_bytes: u64,
    retained_records: u64,
    last_verified_head: &Digest32,
    last_verified_mmr_root: &Digest32,
    evidence_path: Option<&Path>,
) -> Digest32 {
    let mut enc = Encoder::new_frame(TAG_REPAIR_PLAN, FMT_REPAIR_PLAN_V1);
    enc.put_str(&target_path.to_string_lossy());
    enc.put_str(cell_id);
    enc.put_str(action.as_str());
    enc.put_fixed(original_sha256.as_bytes());
    enc.put_u64(original_bytes);
    enc.put_u64(verified_bytes);
    enc.put_u64(retained_records);
    enc.put_fixed(last_verified_head.as_bytes());
    enc.put_fixed(last_verified_mmr_root.as_bytes());
    enc.put_str(
        &evidence_path
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_default(),
    );
    sha256(&enc.finish())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::batch::GENESIS_PREV_HASH;
    use crate::batch::tests::sample_batch;
    use crate::segment::{SegmentReader, SegmentWriter};

    fn scratch_dir(tag: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("fmine-ledger-verify-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("scratch dir");
        dir
    }

    #[test]
    fn test_f7_torn_tail_repair_retains_evidence_enforces_digest_and_resumes_cleanly() {
        let dir = scratch_dir("f7");
        let seg = dir.join("cell-site.fmseg");

        let mut writer =
            SegmentWriter::create(&seg, "CELL-SITE(lumwana)", 0, GENESIS_PREV_HASH, Vec::new())
                .expect("create");
        let b0 = sample_batch("CELL-SITE(lumwana)", 1, 0, GENESIS_PREV_HASH);
        let d0 = writer.append(&b0).expect("b0");
        let b1 = sample_batch("CELL-SITE(lumwana)", 1, 1, d0);
        let d1 = writer.append(&b1).expect("b1");
        let clean_len = std::fs::metadata(&seg).expect("meta").len();
        drop(writer);

        // Simulate crash mid-write of batch 2 (partial length + partial frame bytes).
        let b2 = sample_batch("CELL-SITE(lumwana)", 1, 2, d1);
        let b2_bytes = b2.to_bytes().expect("b2 bytes");
        {
            let mut f = OpenOptions::new().append(true).open(&seg).expect("open");
            f.write_all(&(b2_bytes.len() as u32).to_le_bytes())
                .expect("len");
            f.write_all(&b2_bytes[..b2_bytes.len() / 2])
                .expect("partial frame");
            f.sync_all().expect("sync");
        }
        let torn_bytes_snapshot = std::fs::read(&seg).expect("read torn");

        // Verify reports torn_tail and refuses open_append until repaired.
        let v = verify_segment(&seg).expect("verify torn");
        assert_eq!(v.status, "torn_tail");
        assert_eq!(v.record_count, 2);
        assert_eq!(v.head_hash, d1);
        assert_eq!(v.verified_bytes, clean_len);
        assert!(v.torn_bytes > 0);
        assert!(SegmentWriter::open_append(&seg).is_err());

        // Plan is deterministic.
        let plan1 = plan_segment_repair(&seg).expect("plan 1");
        let plan2 = plan_segment_repair(&seg).expect("plan 2");
        assert_eq!(plan1, plan2);
        assert_eq!(plan1.action, RepairAction::TruncateTornTail);
        assert_eq!(plan1.verified_bytes, clean_len);
        assert_eq!(plan1.retained_records, 2);
        assert_eq!(plan1.last_verified_head, d1);

        // PO-ADIT-3: wrong digest MUST refuse without writing evidence or modifying target!
        let wrong = sha256(b"not-the-plan");
        assert!(matches!(
            apply_segment_repair(&seg, wrong),
            Err(LedgerError::RepairDigestMismatch { .. })
        ));
        assert_eq!(std::fs::read(&seg).expect("unchanged"), torn_bytes_snapshot);
        assert!(
            !plan1
                .evidence_path
                .as_ref()
                .expect("evidence path")
                .exists()
        );

        // Exact digest applies cleanly, preserves complete original bytes, truncates, re-verifies.
        let receipt = apply_segment_repair(&seg, plan1.plan_digest).expect("apply repair");
        assert_eq!(receipt.action, RepairAction::TruncateTornTail);
        assert_eq!(receipt.final_bytes, clean_len);
        assert_eq!(receipt.retained_records, 2);
        assert_eq!(receipt.verified_head, d1);
        let ev_path = receipt.evidence_path.expect("evidence written");
        assert_eq!(
            std::fs::read(&ev_path).expect("read evidence"),
            torn_bytes_snapshot
        );

        // Re-opening for append and continuing with batch 2 succeeds cleanly!
        let mut resumed = SegmentWriter::open_append(&seg).expect("resume append");
        let d2 = resumed.append(&b2).expect("append b2");
        let footer = resumed.seal().expect("seal");
        assert_eq!(footer.record_count, 3);
        assert_eq!(footer.head_hash, d2);
        let final_read = SegmentReader::open(&seg).expect("read final");
        assert_eq!(final_read.batches.len(), 3);

        // Clean segment produces a NoOp plan.
        let clean_plan = plan_segment_repair(&seg).expect("clean plan");
        assert_eq!(clean_plan.action, RepairAction::NoOp);
        assert_eq!(clean_plan.torn_bytes, 0);

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Two batches, then half of a third as a crash leaves it. Returns the torn bytes and the clean length.
    fn torn_segment(seg: &Path) -> (Vec<u8>, u64) {
        let cell = "CELL-SITE(lumwana)";
        let mut writer =
            SegmentWriter::create(seg, cell, 0, GENESIS_PREV_HASH, Vec::new()).expect("create");
        let d0 = writer
            .append(&sample_batch(cell, 1, 0, GENESIS_PREV_HASH))
            .expect("b0");
        let d1 = writer.append(&sample_batch(cell, 1, 1, d0)).expect("b1");
        drop(writer);
        let clean_len = std::fs::metadata(seg).expect("meta").len();
        let b2 = sample_batch(cell, 1, 2, d1).to_bytes().expect("b2 bytes");
        let mut f = OpenOptions::new().append(true).open(seg).expect("open");
        f.write_all(&(b2.len() as u32).to_le_bytes()).expect("len");
        f.write_all(&b2[..b2.len() / 2]).expect("partial frame");
        f.sync_all().expect("sync");
        (std::fs::read(seg).expect("read torn"), clean_len)
    }

    /// A crash while the evidence is written, or after it is kept and before the truncation, must not
    /// wedge the repair: the retry plans the same repair and completes it (`F7`).
    #[test]
    fn test_f7_repair_completes_after_a_crash_during_repair() {
        let dir = scratch_dir("f7-crash-in-repair");
        let seg = dir.join("cell-site.fmseg");
        let (torn, clean_len) = torn_segment(&seg);
        let plan = plan_segment_repair(&seg).expect("plan");
        let evidence = plan.evidence_path.clone().expect("evidence path");
        let mut partial = evidence.as_os_str().to_os_string();
        partial.push(".partial");
        let partial = PathBuf::from(partial);

        // A crash while writing the evidence leaves part of a copy at the partial path.
        std::fs::write(&partial, &torn[..torn.len() / 3]).expect("partial copy");
        // A crash just after the evidence is kept leaves it complete and the segment untouched.
        retain_repair_evidence(&evidence, &torn).expect("keep evidence");
        assert!(
            !partial.exists(),
            "the partial copy was replaced and renamed"
        );
        assert_eq!(std::fs::read(&evidence).expect("evidence"), torn);
        assert_eq!(std::fs::read(&seg).expect("segment"), torn);

        assert_eq!(plan_segment_repair(&seg).expect("plan again"), plan);
        let receipt = apply_segment_repair(&seg, plan.plan_digest).expect("retry completes");
        assert_eq!(receipt.final_bytes, clean_len);
        assert_eq!(receipt.retained_records, 2);
        assert_eq!(std::fs::read(&evidence).expect("evidence kept"), torn);

        // Some other file at the evidence path is refused, and the segment is not touched.
        let seg2 = dir.join("cell-site-2.fmseg");
        let (torn2, _) = torn_segment(&seg2);
        let plan2 = plan_segment_repair(&seg2).expect("plan 2");
        let evidence2 = plan2.evidence_path.clone().expect("evidence path 2");
        std::fs::write(&evidence2, b"not this repair's bytes").expect("foreign file");
        assert!(matches!(
            apply_segment_repair(&seg2, plan2.plan_digest),
            Err(LedgerError::InvalidField {
                field: "evidence_path",
                ..
            })
        ));
        assert_eq!(std::fs::read(&seg2).expect("segment 2"), torn2);
        assert_eq!(
            std::fs::read(&evidence2).expect("foreign file kept"),
            b"not this repair's bytes"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }
}
