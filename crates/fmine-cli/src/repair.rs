//! `fmine repair plan` and `fmine repair apply` (`BD-118`, plan §26.11 F7, §30.2 `PO-ADIT-3`).
//!
//! Supports:
//! - `FMT-SEG-1` Bedrock ledger segments (`--segment <path>` or auto-detected by `SEGH` magic);
//! - `FJR1` forecast journals and `RLJ1` reliance journals (`--journal <path> [--kind forecast|reliance]`).
//!
//! History is never rewritten (`INV-003`):
//! - Clean files produce a deterministic `no_op` plan.
//! - A crash-torn trailing frame/line (`F7`) produces a `truncate_torn_tail` plan bound by a
//!   canonical SHA-256 `plan_digest`.
//! - `repair apply <digest>` recomputes the current plan under an exclusive OS lock, refuses any
//!   digest mismatch (`FMINE-REF-505`, `PO-ADIT-3`) without writing a byte, archives complete
//!   pre-repair bytes to `<path>.torn.<sha12>.evidence` (synced, then renamed into place; a retry after
//!   a crash keeps an archive that already holds those bytes), then truncates to `verified_bytes` and
//!   re-verifies from byte 0.
//! - Interior corruption, hash-chain breaks, and tampered footers are refused (`FMINE-REF-505`).

use std::fs::{File, OpenOptions, TryLockError};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use fmine_claim::forecast_journal;
use fmine_core::codec::Encoder;
use fmine_core::hash::{Digest32, sha256};
use fmine_evidence::{Refusal, RefusalCode};
use fmine_ledger::verify::{
    RepairAction, SegmentRepairPlan, SegmentRepairReceipt, apply_segment_repair,
    plan_segment_repair, retain_repair_evidence,
};

use crate::{Envelope, NextAction, ROBOT_DOCS_WHY, escape_json};

/// CLI usage summary for `fmine repair plan`.
pub const PLAN_USAGE: &str =
    "repair plan (--segment <path> | --journal <path> [--kind forecast|reliance])";

/// CLI usage summary for `fmine repair apply`.
pub const APPLY_USAGE: &str = "repair apply <plan-digest-hex> (--segment <path> | --journal <path> [--kind forecast|reliance])";

/// Machine-readable documentation block for `fmine robot-docs`.
pub const ROBOT_DOCS_JSON: &str = concat!(
    "{\"schema\":\"fmine.repair-plan.v1\",",
    "\"targets\":[\"fmt_seg_1\",\"forecast_journal\",\"reliance_journal\"],",
    "\"proof_obligation\":\"PO-ADIT-3: repair apply refuses any mismatched plan digest before writing a byte; complete pre-repair bytes are synced to <path>.torn.<sha12>.evidence before truncating to the last verified record; interior corruption is never rewritten (INV-003, F7)\"}"
);

const TAG_JOURNAL_PLAN: [u8; 4] = *b"JPLN";
const FMT_JOURNAL_PLAN_V1: u16 = 1;

/// Explicit or auto-detected journal format kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JournalKind {
    Forecast,
    Reliance,
}

impl JournalKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Forecast => "forecast_journal",
            Self::Reliance => "reliance_journal",
        }
    }
}

/// Target selected by `repair plan` or `repair apply`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    Segment(PathBuf),
    Journal {
        path: PathBuf,
        kind: Option<JournalKind>,
    },
}

/// Parsed `repair` subcommand.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Plan(Target),
    Apply { digest_hex: String, target: Target },
}

/// Deterministic journal inspection & F7 repair plan shared with `fmine doctor`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JournalRepairPlan {
    pub plan_digest: Digest32,
    pub target_path: PathBuf,
    pub kind: JournalKind,
    pub kind_label: &'static str,
    pub action: RepairAction,
    pub original_sha256: Digest32,
    pub original_bytes: u64,
    pub verified_bytes: u64,
    pub torn_bytes: u64,
    pub retained_records: u64,
    pub last_verified_head: Digest32,
    pub evidence_path: Option<PathBuf>,
    pub reason: String,
}

struct LockedFile(File);

impl LockedFile {
    fn acquire(file: File, exclusive: bool) -> Result<Self, Refusal> {
        let res = if exclusive {
            file.try_lock()
        } else {
            file.try_lock_shared()
        };
        match res {
            Ok(()) => Ok(Self(file)),
            Err(TryLockError::WouldBlock) => Err(Refusal::new(
                RefusalCode::StaleAnchor502,
                "repair target is locked by another handle; retry when unlocked",
            )),
            Err(TryLockError::Error(err)) => Err(Refusal::new(
                RefusalCode::InputMissingOrUnreadable506,
                format!("file locking unavailable: {err}"),
            )),
        }
    }
}

impl Drop for LockedFile {
    fn drop(&mut self) {
        let _ = self.0.unlock();
    }
}

/// Parse `repair plan ...` or `repair apply ...` after global options are stripped.
pub fn parse(words: &[String]) -> Result<Command, String> {
    if words.first().map(String::as_str) != Some("repair") {
        return Err(format!(
            "expected 'fmine {PLAN_USAGE}' or 'fmine {APPLY_USAGE}'"
        ));
    }
    let Some(sub) = words.get(1).map(String::as_str) else {
        return Err(format!(
            "expected 'fmine {PLAN_USAGE}' or 'fmine {APPLY_USAGE}'"
        ));
    };
    let mut segment: Option<PathBuf> = None;
    let mut journal: Option<PathBuf> = None;
    let mut kind: Option<JournalKind> = None;
    let mut digest: Option<String> = None;

    let mut i = 2;
    while let Some(arg) = words.get(i) {
        match arg.as_str() {
            "--segment" => {
                i += 1;
                let value = words
                    .get(i)
                    .filter(|v| !v.is_empty() && !v.starts_with("--"))
                    .ok_or("missing value for --segment")?;
                segment = Some(PathBuf::from(value));
            }
            "--journal" => {
                i += 1;
                let value = words
                    .get(i)
                    .filter(|v| !v.is_empty() && !v.starts_with("--"))
                    .ok_or("missing value for --journal")?;
                journal = Some(PathBuf::from(value));
            }
            "--kind" => {
                i += 1;
                kind = match words.get(i).map(String::as_str) {
                    Some("forecast") => Some(JournalKind::Forecast),
                    Some("reliance") => Some(JournalKind::Reliance),
                    Some(other) => {
                        return Err(format!(
                            "unsupported --kind '{other}' (expected forecast|reliance)"
                        ));
                    }
                    None => return Err("missing value for --kind".to_string()),
                };
            }
            "--digest" => {
                if sub != "apply" {
                    return Err("--digest is only accepted by 'repair apply'".to_string());
                }
                i += 1;
                let value = words
                    .get(i)
                    .filter(|v| !v.is_empty() && !v.starts_with("--"))
                    .ok_or("missing value for --digest")?;
                digest = Some(value.clone());
            }
            flag if flag.starts_with("--") => return Err(format!("unknown flag '{flag}'")),
            positional => {
                if sub == "apply" && digest.is_none() {
                    digest = Some(positional.to_string());
                } else {
                    return Err(format!(
                        "unexpected argument '{positional}' for 'repair {sub}'"
                    ));
                }
            }
        }
        i += 1;
    }

    let target = match (segment, journal) {
        (Some(_), Some(_)) => {
            return Err("choose either --segment <path> or --journal <path>, not both".to_string());
        }
        (Some(path), None) => {
            if kind.is_some() {
                return Err("--kind applies only to --journal targets".to_string());
            }
            Target::Segment(path)
        }
        (None, Some(path)) => Target::Journal { path, kind },
        (None, None) => {
            let usage = if sub == "apply" {
                APPLY_USAGE
            } else {
                PLAN_USAGE
            };
            return Err(format!("expected 'fmine {usage}'"));
        }
    };

    match sub {
        "plan" => Ok(Command::Plan(target)),
        "apply" => {
            let digest_hex = digest.ok_or_else(|| format!("expected 'fmine {APPLY_USAGE}'"))?;
            Ok(Command::Apply { digest_hex, target })
        }
        other => Err(format!(
            "unknown repair subcommand '{other}' (expected plan|apply)"
        )),
    }
}

/// Execute `fmine repair plan` or `fmine repair apply`.
pub fn execute(cmd: &Command, workspace_root: &Path) -> Envelope {
    match cmd {
        Command::Plan(target) => execute_plan(target, workspace_root),
        Command::Apply { digest_hex, target } => execute_apply(digest_hex, target, workspace_root),
    }
}

fn resolve_path(raw: &Path, workspace_root: &Path) -> PathBuf {
    if raw.is_absolute() {
        raw.to_path_buf()
    } else {
        workspace_root.join(raw)
    }
}

fn execute_plan(target: &Target, workspace_root: &Path) -> Envelope {
    match target {
        Target::Segment(raw) => {
            let path = resolve_path(raw, workspace_root);
            match plan_segment_repair(&path) {
                Ok(plan) => {
                    let next = next_actions_for_plan(
                        &plan.plan_digest,
                        plan.action,
                        &format!("--segment {}", raw.display()),
                    );
                    Envelope::ok("repair plan", segment_plan_json(&plan), next)
                }
                Err(err) => Envelope::refused(
                    "repair plan",
                    err.to_refusal(),
                    vec![NextAction::new("fmine doctor", "inspect diagnostic status")],
                ),
            }
        }
        Target::Journal { path: raw, kind } => {
            let path = resolve_path(raw, workspace_root);
            match inspect_journal_path(&path, *kind) {
                Ok(plan) => {
                    let kind_arg = match kind {
                        Some(JournalKind::Forecast) => " --kind forecast",
                        Some(JournalKind::Reliance) => " --kind reliance",
                        None => "",
                    };
                    let next = next_actions_for_plan(
                        &plan.plan_digest,
                        plan.action,
                        &format!("--journal {}{kind_arg}", raw.display()),
                    );
                    Envelope::ok("repair plan", journal_plan_json(&plan), next)
                }
                Err(refusal) => Envelope::refused(
                    "repair plan",
                    refusal,
                    vec![NextAction::new("fmine doctor", "inspect diagnostic status")],
                ),
            }
        }
    }
}

fn execute_apply(digest_hex: &str, target: &Target, workspace_root: &Path) -> Envelope {
    let expected_digest = match parse_digest32(digest_hex) {
        Ok(d) => d,
        Err(refusal) => {
            return Envelope::refused(
                "repair apply",
                refusal,
                vec![NextAction::new("fmine robot-docs", ROBOT_DOCS_WHY)],
            );
        }
    };
    match target {
        Target::Segment(raw) => {
            let path = resolve_path(raw, workspace_root);
            match apply_segment_repair(&path, expected_digest) {
                Ok(receipt) => Envelope::ok(
                    "repair apply",
                    segment_receipt_json(&receipt),
                    vec![NextAction::new(
                        format!("fmine doctor --segment {}", raw.display()),
                        "verify repaired segment chain and MMR root",
                    )],
                ),
                Err(err) => Envelope::refused(
                    "repair apply",
                    err.to_refusal(),
                    vec![NextAction::new(
                        format!("fmine repair plan --segment {}", raw.display()),
                        "recompute the current repair plan digest",
                    )],
                ),
            }
        }
        Target::Journal { path: raw, kind } => {
            let path = resolve_path(raw, workspace_root);
            match apply_journal_repair(&path, *kind, expected_digest) {
                Ok(payload) => Envelope::ok(
                    "repair apply",
                    payload,
                    vec![NextAction::new(
                        format!("fmine doctor --journal {}", raw.display()),
                        "verify repaired journal chain head",
                    )],
                ),
                Err(refusal) => Envelope::refused(
                    "repair apply",
                    refusal,
                    vec![NextAction::new(
                        format!("fmine repair plan --journal {}", raw.display()),
                        "recompute the current journal repair plan digest",
                    )],
                ),
            }
        }
    }
}

/// Inspect a `FJR1` forecast journal or `RLJ1` reliance journal for `doctor` and `repair plan`.
pub fn inspect_journal_path(
    path: &Path,
    explicit_kind: Option<JournalKind>,
) -> Result<JournalRepairPlan, Refusal> {
    let raw = File::open(path).map_err(|e| {
        Refusal::new(
            RefusalCode::InputMissingOrUnreadable506,
            format!("cannot open journal {}: {e}", path.display()),
        )
    })?;
    let mut locked = LockedFile::acquire(raw, false)?;
    let bytes = read_journal_bytes(&mut locked.0, path)?;
    plan_journal_from_bytes(path, &bytes, explicit_kind)
}

fn apply_journal_repair(
    path: &Path,
    explicit_kind: Option<JournalKind>,
    expected_digest: Digest32,
) -> Result<String, Refusal> {
    let raw = OpenOptions::new()
        .read(true)
        .write(true)
        .open(path)
        .map_err(|e| {
            Refusal::new(
                RefusalCode::InputMissingOrUnreadable506,
                format!("cannot open journal {}: {e}", path.display()),
            )
        })?;
    let mut locked = LockedFile::acquire(raw, true)?;
    let original_bytes = read_journal_bytes(&mut locked.0, path)?;
    let plan = plan_journal_from_bytes(path, &original_bytes, explicit_kind)?;
    if plan.plan_digest != expected_digest {
        return Err(Refusal::new(
            RefusalCode::DigestMismatch505,
            format!(
                "repair plan digest mismatch: requested {expected_digest}, current on-disk plan is {}",
                plan.plan_digest
            ),
        ));
    }

    if plan.action == RepairAction::NoOp {
        return Ok(journal_receipt_json(
            &plan,
            plan.original_sha256,
            plan.original_bytes,
            None,
        ));
    }

    let Some(evidence_path) = plan.evidence_path.clone() else {
        return Err(Refusal::new(
            RefusalCode::DigestMismatch505,
            "truncation plan lacked an evidence path",
        ));
    };

    // Archive complete pre-repair bytes BEFORE truncating (`F7`); a retry after a crash keeps them. Its
    // refusals carry the same codes as a segment repair's: a foreign file at the evidence path is 505, an
    // I/O failure 506.
    retain_repair_evidence(&evidence_path, &original_bytes).map_err(|e| e.to_refusal())?;

    // Truncate target journal to verified prefix, sync, and re-verify from byte 0.
    locked
        .0
        .set_len(plan.verified_bytes)
        .and_then(|()| locked.0.sync_all())
        .map_err(|e| {
            Refusal::new(
                RefusalCode::InputMissingOrUnreadable506,
                format!("cannot truncate journal {}: {e}", path.display()),
            )
        })?;
    locked.0.seek(SeekFrom::Start(0)).map_err(|e| {
        Refusal::new(
            RefusalCode::InputMissingOrUnreadable506,
            format!("cannot rewind repaired journal: {e}"),
        )
    })?;
    let repaired = read_journal_bytes(&mut locked.0, path)?;
    let (records, head) = verify_journal_exact(&repaired, plan.kind)?;
    if records != plan.retained_records || head != plan.last_verified_head {
        return Err(Refusal::new(
            RefusalCode::DigestMismatch505,
            "post-truncation journal verification disagreed with repair plan",
        ));
    }
    Ok(journal_receipt_json(
        &plan,
        sha256(&repaired),
        repaired.len() as u64,
        Some(&evidence_path),
    ))
}

fn plan_journal_from_bytes(
    path: &Path,
    bytes: &[u8],
    explicit_kind: Option<JournalKind>,
) -> Result<JournalRepairPlan, Refusal> {
    if bytes.is_empty() {
        return Err(Refusal::new(
            RefusalCode::DigestMismatch505,
            "journal file is empty; genesis record missing (non-repairable)",
        ));
    }
    let kind = match explicit_kind {
        Some(k) => k,
        None => detect_journal_kind(bytes)?,
    };
    let original_sha256 = sha256(bytes);

    if let Ok((records, head)) = verify_journal_exact(bytes, kind) {
        let plan_digest = compute_journal_plan_digest(
            path,
            kind,
            RepairAction::NoOp,
            &original_sha256,
            bytes.len() as u64,
            bytes.len() as u64,
            records,
            &head,
            None,
        );
        return Ok(JournalRepairPlan {
            plan_digest,
            target_path: path.to_path_buf(),
            kind,
            kind_label: kind.as_str(),
            action: RepairAction::NoOp,
            original_sha256,
            original_bytes: bytes.len() as u64,
            verified_bytes: bytes.len() as u64,
            torn_bytes: 0,
            retained_records: records,
            last_verified_head: head,
            evidence_path: None,
            reason: "journal hash chain verified cleanly".to_string(),
        });
    }

    // Attempt strictly F7 trailing-line truncation ONLY: drop either an unterminated final line
    // or a single corrupt trailing newline-terminated line, NEVER rewriting earlier records.
    let candidate_cut = if !bytes.ends_with(b"\n") {
        bytes.iter().rposition(|&b| b == b'\n').map(|idx| idx + 1)
    } else {
        bytes[..bytes.len() - 1]
            .iter()
            .rposition(|&b| b == b'\n')
            .map(|idx| idx + 1)
    };
    let Some(cut) = candidate_cut.filter(|&c| c > 0) else {
        return Err(Refusal::new(
            RefusalCode::DigestMismatch505,
            "journal genesis or interior corruption is non-repairable (history is never rewritten)",
        ));
    };
    let (records, head) = verify_journal_exact(&bytes[..cut], kind).map_err(|e| {
        Refusal::new(
            RefusalCode::DigestMismatch505,
            format!(
                "journal has non-repairable interior corruption before the final line: {}",
                e.what
            ),
        )
    })?;
    let torn_bytes = (bytes.len() - cut) as u64;
    let evidence_path = evidence_path_for(path, &original_sha256);
    let reason = if !bytes.ends_with(b"\n") {
        format!("unterminated torn trailing journal line ({torn_bytes} bytes)")
    } else {
        format!("corrupt trailing journal line ({torn_bytes} bytes)")
    };
    let plan_digest = compute_journal_plan_digest(
        path,
        kind,
        RepairAction::TruncateTornTail,
        &original_sha256,
        bytes.len() as u64,
        cut as u64,
        records,
        &head,
        Some(&evidence_path),
    );
    Ok(JournalRepairPlan {
        plan_digest,
        target_path: path.to_path_buf(),
        kind,
        kind_label: kind.as_str(),
        action: RepairAction::TruncateTornTail,
        original_sha256,
        original_bytes: bytes.len() as u64,
        verified_bytes: cut as u64,
        torn_bytes,
        retained_records: records,
        last_verified_head: head,
        evidence_path: Some(evidence_path),
        reason,
    })
}

fn detect_journal_kind(bytes: &[u8]) -> Result<JournalKind, Refusal> {
    let first_line = bytes
        .split_inclusive(|&b| b == b'\n')
        .next()
        .unwrap_or(bytes);
    if verify_journal_exact(first_line, JournalKind::Forecast).is_ok() {
        return Ok(JournalKind::Forecast);
    }
    if verify_journal_exact(first_line, JournalKind::Reliance).is_ok() {
        return Ok(JournalKind::Reliance);
    }
    Err(Refusal::new(
        RefusalCode::DigestMismatch505,
        "cannot detect valid forecast (FJR1) or reliance (RLJ1) genesis line; supply --kind or inspect for genesis corruption",
    ))
}

fn verify_journal_exact(bytes: &[u8], kind: JournalKind) -> Result<(u64, Digest32), Refusal> {
    match kind {
        JournalKind::Forecast => {
            let records = forecast_journal::decode_journal(bytes)?;
            let last = records.last().ok_or_else(|| {
                Refusal::new(RefusalCode::DigestMismatch505, "empty forecast journal")
            })?;
            Ok((records.len() as u64, last.chain_head()?))
        }
        JournalKind::Reliance => {
            let ledger = fmine_io::reliance::decode_journal(bytes)?;
            Ok((ledger.records().len() as u64, ledger.head()))
        }
    }
}

fn read_journal_bytes(file: &mut File, path: &Path) -> Result<Vec<u8>, Refusal> {
    let limit = forecast_journal::MAX_JOURNAL_BYTES as u64;
    let mut bytes = Vec::new();
    Read::by_ref(file)
        .take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| {
            Refusal::new(
                RefusalCode::InputMissingOrUnreadable506,
                format!("cannot read journal {}: {e}", path.display()),
            )
        })?;
    if (bytes.len() as u64) > limit {
        return Err(Refusal::new(
            RefusalCode::DigestMismatch505,
            format!("journal {} exceeds {limit}-byte bound", path.display()),
        ));
    }
    Ok(bytes)
}

fn evidence_path_for(path: &Path, original_sha256: &Digest32) -> PathBuf {
    let short_hex = &original_sha256.to_hex()[..12];
    let mut os = path.as_os_str().to_os_string();
    os.push(format!(".torn.{short_hex}.evidence"));
    PathBuf::from(os)
}

#[allow(
    clippy::too_many_arguments,
    reason = "canonical journal repair plan digest binding"
)]
fn compute_journal_plan_digest(
    target_path: &Path,
    kind: JournalKind,
    action: RepairAction,
    original_sha256: &Digest32,
    original_bytes: u64,
    verified_bytes: u64,
    retained_records: u64,
    last_verified_head: &Digest32,
    evidence_path: Option<&Path>,
) -> Digest32 {
    let mut enc = Encoder::new_frame(TAG_JOURNAL_PLAN, FMT_JOURNAL_PLAN_V1);
    enc.put_str(&target_path.to_string_lossy());
    enc.put_str(kind.as_str());
    enc.put_str(action.as_str());
    enc.put_fixed(original_sha256.as_bytes());
    enc.put_u64(original_bytes);
    enc.put_u64(verified_bytes);
    enc.put_u64(retained_records);
    enc.put_fixed(last_verified_head.as_bytes());
    enc.put_str(
        &evidence_path
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_default(),
    );
    sha256(&enc.finish())
}

fn parse_digest32(hex: &str) -> Result<Digest32, Refusal> {
    let bytes = hex.as_bytes();
    if bytes.len() != 64 {
        return Err(Refusal::new(
            RefusalCode::DigestMismatch505,
            format!(
                "expected 64-character lowercase SHA-256 hex digest, got {} bytes",
                bytes.len()
            ),
        ));
    }
    let mut out = [0u8; 32];
    for (i, slot) in out.iter_mut().enumerate() {
        let hi = nibble(bytes[i * 2]).ok_or_else(|| {
            Refusal::new(
                RefusalCode::DigestMismatch505,
                "plan digest must be lowercase hexadecimal",
            )
        })?;
        let lo = nibble(bytes[i * 2 + 1]).ok_or_else(|| {
            Refusal::new(
                RefusalCode::DigestMismatch505,
                "plan digest must be lowercase hexadecimal",
            )
        })?;
        *slot = (hi << 4) | lo;
    }
    Ok(Digest32(out))
}

const fn nibble(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        _ => None,
    }
}

fn next_actions_for_plan(
    digest: &Digest32,
    action: RepairAction,
    target_flags: &str,
) -> Vec<NextAction> {
    match action {
        RepairAction::NoOp => vec![NextAction::new(
            "fmine doctor",
            "target verified cleanly; no repair apply needed",
        )],
        RepairAction::TruncateTornTail => vec![NextAction::new(
            format!("fmine repair apply {digest} {target_flags}"),
            "archive original torn bytes as evidence and truncate to the last verified record",
        )],
    }
}

fn segment_plan_json(plan: &SegmentRepairPlan) -> String {
    format!(
        "{{\"schema\":\"fmine.repair-plan.v1\",\"target_kind\":\"fmt_seg_1\",\"target_path\":\"{}\",\"cell_id\":\"{}\",\"action\":\"{}\",\"plan_digest\":\"{}\",\"original_sha256\":\"{}\",\"original_bytes\":{},\"verified_bytes\":{},\"torn_bytes\":{},\"retained_records\":{},\"last_verified_head\":\"{}\",\"last_verified_mmr_root\":\"{}\",\"evidence_path\":{},\"reason\":\"{}\"}}",
        escape_json(&plan.target_path.to_string_lossy()),
        escape_json(&plan.cell_id),
        plan.action.as_str(),
        plan.plan_digest,
        plan.original_sha256,
        plan.original_bytes,
        plan.verified_bytes,
        plan.torn_bytes,
        plan.retained_records,
        plan.last_verified_head,
        plan.last_verified_mmr_root,
        plan.evidence_path.as_ref().map_or_else(
            || "null".to_string(),
            |p| format!("\"{}\"", escape_json(&p.to_string_lossy()))
        ),
        escape_json(&plan.reason)
    )
}

fn segment_receipt_json(r: &SegmentRepairReceipt) -> String {
    format!(
        "{{\"schema\":\"fmine.repair-receipt.v1\",\"target_kind\":\"fmt_seg_1\",\"target_path\":\"{}\",\"action\":\"{}\",\"plan_digest\":\"{}\",\"original_sha256\":\"{}\",\"repaired_sha256\":\"{}\",\"final_bytes\":{},\"retained_records\":{},\"verified_head\":\"{}\",\"verified_mmr_root\":\"{}\",\"evidence_path\":{}}}",
        escape_json(&r.target_path.to_string_lossy()),
        r.action.as_str(),
        r.plan_digest,
        r.original_sha256,
        r.repaired_sha256,
        r.final_bytes,
        r.retained_records,
        r.verified_head,
        r.verified_mmr_root,
        r.evidence_path.as_ref().map_or_else(
            || "null".to_string(),
            |p| format!("\"{}\"", escape_json(&p.to_string_lossy()))
        )
    )
}

fn journal_plan_json(plan: &JournalRepairPlan) -> String {
    format!(
        "{{\"schema\":\"fmine.repair-plan.v1\",\"target_kind\":\"{}\",\"target_path\":\"{}\",\"action\":\"{}\",\"plan_digest\":\"{}\",\"original_sha256\":\"{}\",\"original_bytes\":{},\"verified_bytes\":{},\"torn_bytes\":{},\"retained_records\":{},\"last_verified_head\":\"{}\",\"evidence_path\":{},\"reason\":\"{}\"}}",
        plan.kind_label,
        escape_json(&plan.target_path.to_string_lossy()),
        plan.action.as_str(),
        plan.plan_digest,
        plan.original_sha256,
        plan.original_bytes,
        plan.verified_bytes,
        plan.torn_bytes,
        plan.retained_records,
        plan.last_verified_head,
        plan.evidence_path.as_ref().map_or_else(
            || "null".to_string(),
            |p| format!("\"{}\"", escape_json(&p.to_string_lossy()))
        ),
        escape_json(&plan.reason)
    )
}

fn journal_receipt_json(
    plan: &JournalRepairPlan,
    repaired_sha256: Digest32,
    final_bytes: u64,
    evidence_path: Option<&Path>,
) -> String {
    format!(
        "{{\"schema\":\"fmine.repair-receipt.v1\",\"target_kind\":\"{}\",\"target_path\":\"{}\",\"action\":\"{}\",\"plan_digest\":\"{}\",\"original_sha256\":\"{}\",\"repaired_sha256\":\"{}\",\"final_bytes\":{},\"retained_records\":{},\"verified_head\":\"{}\",\"evidence_path\":{}}}",
        plan.kind_label,
        escape_json(&plan.target_path.to_string_lossy()),
        plan.action.as_str(),
        plan.plan_digest,
        plan.original_sha256,
        repaired_sha256,
        final_bytes,
        plan.retained_records,
        plan.last_verified_head,
        evidence_path.map_or_else(
            || "null".to_string(),
            |p| format!("\"{}\"", escape_json(&p.to_string_lossy()))
        )
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    use fmine_core::anchor::{FMT_ANCHOR_V1, LedgerAnchor};
    use fmine_ledger::batch::{DeltaBatch, DeltaKind, DeltaOp, FMT_LEDGER_V1, GENESIS_PREV_HASH};
    use fmine_ledger::segment::SegmentWriter;
    use fmine_runtime::{ClockError, WallClock};

    struct FixedClock(i64);
    impl WallClock for FixedClock {
        fn now_ns(&self) -> Result<i64, ClockError> {
            Ok(self.0)
        }
    }

    fn scratch_dir(tag: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("fmine-cli-repair-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("scratch");
        dir
    }

    fn workspace_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|p| p.parent())
            .expect("workspace root")
            .to_path_buf()
    }

    #[test]
    fn test_po_adit_3_repair_plan_and_apply_on_torn_segment_and_journal() {
        let dir = scratch_dir("both");
        let root = workspace_root();

        // 1) FMT-SEG-1 torn segment end-to-end through CLI repair plan / apply / doctor.
        let seg_path = dir.join("site.fmseg");
        let mut writer = SegmentWriter::create(
            &seg_path,
            "CELL-SITE(lumwana)",
            0,
            GENESIS_PREV_HASH,
            Vec::new(),
        )
        .expect("create segment");
        let anchor = LedgerAnchor {
            format_version: FMT_ANCHOR_V1,
            cell_id: "CELL-SITE(lumwana)".to_string(),
            ledger_epoch: 1,
            commit_sequence: 0,
            adapter_registry_epoch: 1,
            schema_epoch: 1,
            policy_epoch: 1,
            privacy_epoch: 1,
            residency_epoch: 1,
            state_root: sha256(b"s0"),
        };
        let b0 = DeltaBatch {
            format_version: FMT_LEDGER_V1,
            cell_id: "CELL-SITE(lumwana)".to_string(),
            epoch: 1,
            seq: 0,
            prev_hash: GENESIS_PREV_HASH,
            anchor,
            deltas: vec![DeltaOp::new("k0", DeltaKind::Put, b"v0".to_vec())],
            witnesses: Vec::new(),
            cross_cell_refs: Vec::new(),
        };
        writer.append(&b0).expect("append b0");
        drop(writer);

        {
            let mut f = OpenOptions::new()
                .append(true)
                .open(&seg_path)
                .expect("open");
            f.write_all(&[99, 0, 0, 0, b'F', b'M', b'N']).expect("tear");
            f.sync_all().expect("sync");
        }

        let plan_cmd = parse(&[
            "repair".into(),
            "plan".into(),
            "--segment".into(),
            seg_path.to_string_lossy().into_owned(),
        ])
        .expect("parse plan");
        let plan_env = execute(&plan_cmd, &root);
        assert_eq!(plan_env.exit_code, 0);
        assert!(
            plan_env
                .payload_json
                .contains("\"action\":\"truncate_torn_tail\"")
        );
        let digest = extract_json_field(&plan_env.payload_json, "plan_digest");
        assert_eq!(digest.len(), 64);

        // Wrong digest MUST refuse with FMINE-REF-505 (exit 8) and leave file torn.
        let bad_apply = parse(&[
            "repair".into(),
            "apply".into(),
            "0".repeat(64),
            "--segment".into(),
            seg_path.to_string_lossy().into_owned(),
        ])
        .expect("parse bad apply");
        let bad_env = execute(&bad_apply, &root);
        assert_eq!(bad_env.exit_code, 8);
        assert_eq!(
            bad_env.refusal.as_ref().map(|r| r.code),
            Some("FMINE-REF-505")
        );

        // Exact digest repairs cleanly and doctor then reports ok:true.
        let ok_apply = parse(&[
            "repair".into(),
            "apply".into(),
            digest,
            "--segment".into(),
            seg_path.to_string_lossy().into_owned(),
        ])
        .expect("parse apply");
        let ok_env = execute(&ok_apply, &root);
        assert_eq!(ok_env.exit_code, 0);
        assert!(ok_env.payload_json.contains("\"retained_records\":1"));

        let doc_cmd = crate::doctor::parse(&[
            "doctor".into(),
            "--segment".into(),
            seg_path.to_string_lossy().into_owned(),
        ])
        .expect("parse doctor");
        let doc_env = crate::doctor::execute(&doc_cmd, &root);
        assert_eq!(doc_env.exit_code, 0);
        assert!(doc_env.payload_json.contains("\"ok\":true"));

        // 2) FJR1 forecast journal torn tail end-to-end.
        let j_path = dir.join("forecast.fjr");
        let mut j = fmine_io::forecast::Journal::create(
            &j_path,
            "CELL-PUBLIC(barrick)",
            1_000_000,
            &FixedClock(1_700_000_000_000_000_000),
        )
        .expect("create journal");
        j.append(
            "op-1",
            &FixedClock(1_700_000_001_000_000_000),
            |_, _| Ok(()),
        )
        .expect("append");
        drop(j);

        {
            let mut f = OpenOptions::new().append(true).open(&j_path).expect("open");
            f.write_all(b"464d4e45deadbeef").expect("tear line");
            f.sync_all().expect("sync");
        }
        let j_plan_cmd = parse(&[
            "repair".into(),
            "plan".into(),
            "--journal".into(),
            j_path.to_string_lossy().into_owned(),
        ])
        .expect("parse j plan");
        let j_plan_env = execute(&j_plan_cmd, &root);
        assert_eq!(j_plan_env.exit_code, 0);
        assert!(
            j_plan_env
                .payload_json
                .contains("\"target_kind\":\"forecast_journal\"")
        );
        let j_digest = extract_json_field(&j_plan_env.payload_json, "plan_digest");

        let j_apply_cmd = parse(&[
            "repair".into(),
            "apply".into(),
            j_digest,
            "--journal".into(),
            j_path.to_string_lossy().into_owned(),
        ])
        .expect("parse j apply");
        let j_apply_env = execute(&j_apply_cmd, &root);
        assert_eq!(j_apply_env.exit_code, 0);
        assert!(j_apply_env.payload_json.contains("\"retained_records\":2"));
        assert!(fmine_io::forecast::Journal::open_read(&j_path, None).is_ok());

        // A foreign file at the next repair's evidence path is refused with the code a segment repair
        // gives (FMINE-REF-505), and the torn journal is left as it is.
        {
            let mut f = OpenOptions::new().append(true).open(&j_path).expect("open");
            f.write_all(b"464d4e45deadbeef").expect("tear line again");
            f.sync_all().expect("sync");
        }
        let torn_again = std::fs::read(&j_path).expect("read torn journal");
        let j_plan2_env = execute(&j_plan_cmd, &root);
        assert_eq!(j_plan2_env.exit_code, 0);
        let j_evidence = extract_json_field(&j_plan2_env.payload_json, "evidence_path");
        std::fs::write(&j_evidence, b"not this repair's bytes").expect("foreign evidence file");
        let j_apply2_cmd = parse(&[
            "repair".into(),
            "apply".into(),
            extract_json_field(&j_plan2_env.payload_json, "plan_digest"),
            "--journal".into(),
            j_path.to_string_lossy().into_owned(),
        ])
        .expect("parse j apply 2");
        let j_apply2_env = execute(&j_apply2_cmd, &root);
        assert_eq!(j_apply2_env.exit_code, 8);
        assert_eq!(
            j_apply2_env.refusal.as_ref().map(|r| r.code),
            Some("FMINE-REF-505")
        );
        assert_eq!(std::fs::read(&j_path).expect("journal kept"), torn_again);
        assert_eq!(
            std::fs::read(&j_evidence).expect("foreign file kept"),
            b"not this repair's bytes"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    fn extract_json_field(json: &str, key: &str) -> String {
        let needle = format!("\"{key}\":\"");
        let start = json.find(&needle).expect("key present") + needle.len();
        let end = json[start..].find('"').expect("closing quote") + start;
        json[start..end].to_string()
    }
}
