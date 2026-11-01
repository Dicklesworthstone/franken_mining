//! DESIGN (BD-347): local issued-output authority. Exact bytes and a fresh pinned status cut are
//! independent checks. A release capability identifies a local role, never physical human review.

use std::collections::BTreeMap;
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use fmine_claim::ClaimId;
use fmine_claim::reliance::{
    DefectClass, Entry, Header, IssuanceLane, IssuanceRecord, RelianceCheck, RelianceStatus,
    RoleId, StatusRefresh, Supersession, TrustContext, TrustedTime, Withdrawal,
};
use fmine_core::hash::{Blake3, Digest32, blake3_hash};
use fmine_core::id::PrefixKind;
use fmine_evidence::{Refusal, RefusalCode};
use fmine_io::reliance::{Journal, ReleaseKey};
use fmine_runtime::{HostWallClock, WallClock};

use crate::{Envelope, OutputFormat, escape_json, reliance_binding};

const MAX_ARTIFACT_BYTES: u64 = 64 * 1024 * 1024;
pub(crate) const VERBS_JSON: &str = concat!(
    r#""reliance init --journal <path> --release-key <private-32-byte-file>","#,
    r#""reliance issue --journal <path> --head <hex> --release-key <path> --trust-host-clock --synthetic|--issuer <issuer> --output <new-path> --recipient-role <role> --purpose review --code-revision <git-sha>","#,
    r#""reliance check --journal <path> --digest <hex> [--artifact <path>] [--head <hex> --trust-host-clock]","#,
    r#""reliance reuse --journal <path> --artifact <path> --output <new-path> --head <hex> --trust-host-clock","#,
    r#""reliance withdraw --journal <path> --head <hex> --release-key <path> --trust-host-clock --digest <hex> --defect code-defect|source-restated|scope-error --claims <ids>|--all-claims --reason <text>","#,
    r#""reliance supersede --journal <path> --head <hex> --release-key <path> --trust-host-clock --digest <hex> --replacement <hex>","#,
    r#""reliance refresh --journal <path> --head <hex> --release-key <path> --trust-host-clock","#,
    r#""reliance recall --journal <path> --code-revision <git-sha>|--claim <id>""#,
);
pub(crate) const ROBOT_DOCS_JSON: &str = r#"{"mode":"local_capability_authority","default_read":"unknown_without_trusted_head_and_time","pin":"exact final head obtained independently of untrusted journal bytes","clock":"explicit operator-declared bounded host clock; never external time proof","artifact":"exact stored bytes including final newline; receipt is separate","revision":"caller-declared Git SHA; executable BLAKE3 measured separately","recall":"recorded local issuances only; reuse copies are not new recipient registrations; no notifications","release_role":"release-owner capability; no human-review attestation","roles":["release-owner","sponsor","reviewer-pool-N","agent-session:<32 lowercase hex>"],"purpose":["analysis","review","decision-support"]}"#;
const NO_CLAIM: &str = "Local report reliance only. A release key establishes local capability possession, not human review. Host-clock bounds are operator declarations, not external time proofs. Recall covers only recorded issuances; no external copies or recipients are contacted.";

/// Parsed local journal operation. Construction is through the strict CLI parser.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Command {
    access: Access,
    operation: Operation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Access {
    journal: PathBuf,
    head: Option<Digest32>,
    trust_host_clock: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Operation {
    Init {
        key: PathBuf,
        role: RoleId,
        max_age_ns: u64,
        max_skew_ns: u64,
    },
    Issue {
        key: PathBuf,
        output: PathBuf,
        issuer: Option<String>,
        through: Option<String>,
        recipient: RoleId,
        purpose: String,
        revision: String,
        artifact_format: OutputFormat,
    },
    Check {
        digest: Digest32,
        artifact: Option<PathBuf>,
    },
    Reuse {
        artifact: PathBuf,
        output: PathBuf,
    },
    Withdraw {
        key: PathBuf,
        digest: Digest32,
        reason: String,
        claims: Vec<ClaimId>,
        defect: DefectClass,
    },
    Supersede {
        key: PathBuf,
        digest: Digest32,
        replacement: Digest32,
    },
    Refresh {
        key: PathBuf,
    },
    RecallRevision(String),
    RecallClaim(ClaimId),
}

fn digest(text: &str) -> Result<Digest32, String> {
    if text.len() != 64
        || !text
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err("digest must be exactly 64 lowercase hexadecimal characters".to_string());
    }
    let mut bytes = [0; 32];
    for (i, byte) in bytes.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&text[i * 2..i * 2 + 2], 16).map_err(|e| e.to_string())?;
    }
    Ok(Digest32(bytes))
}

fn claim(text: &str) -> Result<ClaimId, String> {
    let id: ClaimId = text.parse().map_err(|e| format!("invalid Claim ID: {e}"))?;
    if id.kind != PrefixKind::Claim {
        return Err("expected a canonical clm_ Claim ID".to_string());
    }
    Ok(id)
}

fn role(text: &str) -> Result<RoleId, String> {
    RoleId::parse(text).map_err(|e| format!("invalid generic role: {}", e.what))
}

fn revision(text: String) -> Result<String, String> {
    if text.len() != 40
        || !text
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(
            "--code-revision requires a caller-declared full lowercase Git SHA (40 hex characters)"
                .to_string(),
        );
    }
    Ok(text)
}

fn required(fields: &mut BTreeMap<String, String>, name: &str) -> Result<String, String> {
    fields.remove(name).ok_or_else(|| format!("missing {name}"))
}

fn scaled(
    fields: &mut BTreeMap<String, String>,
    name: &str,
    default: u64,
    factor: u64,
) -> Result<u64, String> {
    let value = fields
        .remove(name)
        .map_or(Ok(default), |v| v.parse::<u64>().map_err(|e| e.to_string()))?;
    value
        .checked_mul(factor)
        .ok_or_else(|| format!("{name} overflows nanoseconds"))
}

fn output_format(text: &str) -> Result<OutputFormat, String> {
    match text {
        "json" => Ok(OutputFormat::Json),
        "markdown" | "md" => Ok(OutputFormat::Markdown),
        _ => Err("format must be json or markdown".to_string()),
    }
}

/// Every option is consumed by its operation; no release, clock, or selector flag is silently ignored.
pub(crate) fn parse(
    args: &[String],
    root: &Path,
) -> Result<(Command, OutputFormat, PathBuf), String> {
    let mut fields = BTreeMap::new();
    let mut words = Vec::new();
    let mut i = 0;
    while let Some(arg) = args.get(i) {
        if arg.starts_with("--") {
            let value = if matches!(
                arg.as_str(),
                "--synthetic" | "--trust-host-clock" | "--all-claims" | "--json"
            ) {
                String::new()
            } else {
                i += 1;
                let value = args
                    .get(i)
                    .ok_or_else(|| format!("missing value for {arg}"))?;
                if value.starts_with("--") {
                    return Err(format!("missing value for {arg}"));
                }
                value.clone()
            };
            if fields.insert(arg.clone(), value).is_some() {
                return Err(format!("duplicate option {arg}"));
            }
        } else {
            words.push(arg.as_str());
        }
        i += 1;
    }
    let ["reliance", verb] = words.as_slice() else {
        return Err(
            "expected reliance init|issue|check|reuse|withdraw|supersede|refresh|recall"
                .to_string(),
        );
    };
    let format = output_format(
        &fields
            .remove("--format")
            .unwrap_or_else(|| "json".to_string()),
    )?;
    if fields.remove("--json").is_some() && format != OutputFormat::Json {
        return Err("--json conflicts with --format markdown".to_string());
    }
    let workspace = fields
        .remove("--workspace")
        .map_or_else(|| root.to_path_buf(), PathBuf::from);
    let journal = PathBuf::from(required(&mut fields, "--journal")?);
    let head = fields.remove("--head").map(|s| digest(&s)).transpose()?;
    let trust_host_clock = fields.remove("--trust-host-clock").is_some();
    let operation = match *verb {
        "init" => Operation::Init {
            key: PathBuf::from(required(&mut fields, "--release-key")?),
            role: role(
                &fields
                    .remove("--release-role")
                    .unwrap_or_else(|| "release-owner".to_string()),
            )?,
            max_age_ns: scaled(&mut fields, "--max-age-seconds", 300, 1_000_000_000)?,
            max_skew_ns: scaled(&mut fields, "--max-clock-skew-ms", 1_000, 1_000_000)?,
        },
        "issue" => {
            let issuer = fields.remove("--issuer");
            let synthetic = fields.remove("--synthetic").is_some();
            let through = fields.remove("--through");
            if issuer.is_some() == synthetic || (synthetic && through.is_some()) {
                return Err("issue requires exactly one of --issuer or --synthetic; --through requires --issuer".to_string());
            }
            if let Some(name) = &issuer
                && (!fmine_public::sources::is_issuer_name(name)
                    || !fmine_public::sources::registry_path(&workspace, name).is_file())
            {
                return Err(format!("unknown issuer '{name}'"));
            }
            if through
                .as_deref()
                .is_some_and(|q| !fmine_public::sources::is_through_bound(q))
            {
                return Err("--through must be YYYYQ1..YYYYQ4".to_string());
            }
            let purpose = required(&mut fields, "--purpose")?;
            if !matches!(purpose.as_str(), "analysis" | "review" | "decision-support") {
                return Err(
                    "--purpose must be analysis, review or decision-support (no personal data)"
                        .to_string(),
                );
            }
            Operation::Issue {
                key: PathBuf::from(required(&mut fields, "--release-key")?),
                output: PathBuf::from(required(&mut fields, "--output")?),
                issuer,
                through,
                purpose,
                recipient: role(&required(&mut fields, "--recipient-role")?)?,
                revision: revision(required(&mut fields, "--code-revision")?)?,
                artifact_format: output_format(
                    &fields
                        .remove("--artifact-format")
                        .unwrap_or_else(|| "json".to_string()),
                )?,
            }
        }
        "check" => Operation::Check {
            digest: digest(&required(&mut fields, "--digest")?)?,
            artifact: fields.remove("--artifact").map(PathBuf::from),
        },
        "reuse" => Operation::Reuse {
            artifact: PathBuf::from(required(&mut fields, "--artifact")?),
            output: PathBuf::from(required(&mut fields, "--output")?),
        },
        "withdraw" => {
            let all_claims = fields.remove("--all-claims").is_some();
            if all_claims == fields.contains_key("--claims") {
                return Err("withdraw requires --claims <comma-separated IDs> or an explicit --all-claims declaration".to_string());
            }
            let mut claims = fields
                .remove("--claims")
                .map(|s| s.split(',').map(claim).collect::<Result<Vec<_>, _>>())
                .transpose()?
                .unwrap_or_default();
            claims.sort_unstable();
            claims.dedup();
            let reason = required(&mut fields, "--reason")?;
            if reason.is_empty() || reason.len() > 256 || reason.chars().any(char::is_control) {
                return Err(
                    "--reason requires 1..256 bytes of single-line, non-personal explanation"
                        .to_string(),
                );
            }
            Operation::Withdraw {
                key: PathBuf::from(required(&mut fields, "--release-key")?),
                digest: digest(&required(&mut fields, "--digest")?)?,
                reason,
                claims,
                defect: match required(&mut fields, "--defect")?.as_str() {
                    "code-defect" => DefectClass::CodeDefect,
                    "source-restated" => DefectClass::SourceRestated,
                    "scope-error" => DefectClass::ScopeError,
                    _ => {
                        return Err(
                            "--defect must be code-defect, source-restated or scope-error"
                                .to_string(),
                        );
                    }
                },
            }
        }
        "supersede" => Operation::Supersede {
            key: PathBuf::from(required(&mut fields, "--release-key")?),
            digest: digest(&required(&mut fields, "--digest")?)?,
            replacement: digest(&required(&mut fields, "--replacement")?)?,
        },
        "refresh" => Operation::Refresh {
            key: PathBuf::from(required(&mut fields, "--release-key")?),
        },
        "recall" => match (fields.remove("--code-revision"), fields.remove("--claim")) {
            (Some(r), None) => Operation::RecallRevision(revision(r)?),
            (None, Some(c)) => Operation::RecallClaim(claim(&c)?),
            _ => {
                return Err("recall requires exactly one of --code-revision or --claim".to_string());
            }
        },
        _ => return Err(format!("unknown reliance operation '{verb}'")),
    };
    if let Some((flag, _)) = fields.first_key_value() {
        return Err(format!("option {flag} is not accepted by reliance {verb}"));
    }
    if matches!(operation, Operation::Init { .. }) && (head.is_some() || trust_host_clock) {
        return Err("init takes no --head or --trust-host-clock; genesis creates no current-reuse permission".to_string());
    }
    if matches!(
        operation,
        Operation::RecallRevision(_) | Operation::RecallClaim(_)
    ) && trust_host_clock
    {
        return Err("recall describes recorded copies and takes no --trust-host-clock".to_string());
    }
    if matches!(
        operation,
        Operation::Issue { .. }
            | Operation::Withdraw { .. }
            | Operation::Supersede { .. }
            | Operation::Refresh { .. }
    ) && (head.is_none() || !trust_host_clock)
    {
        return Err("journal changes require --head, --release-key and --trust-host-clock; no implicit authority or time proof".to_string());
    }
    Ok((
        Command {
            access: Access {
                journal,
                head,
                trust_host_clock,
            },
            operation,
        },
        format,
        workspace,
    ))
}

fn io_error(detail: impl Into<String>) -> Refusal {
    Refusal::new(RefusalCode::InputMissingOrUnreadable506, detail)
}

fn absolute(root: &Path, path: &Path) -> PathBuf {
    root.join(path)
}

fn now_ns() -> Result<i64, Refusal> {
    HostWallClock
        .now_ns()
        .map_err(|e| io_error(format!("host clock: {e}")))
}

fn trusted_time(access: &Access, header: &Header, now: i64) -> Option<TrustedTime> {
    access.trust_host_clock.then_some(TrustedTime {
        now_ns: now,
        uncertainty_ns: header.max_clock_skew_ns,
    })
}

fn text(value: &str) -> String {
    format!("\"{}\"", escape_json(value))
}

fn write_new(path: &Path, bytes: &[u8]) -> Result<(), Refusal> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|e| {
            io_error(format!(
                "cannot create new artifact {}: {e}",
                path.display()
            ))
        })?;
    file.write_all(bytes)
        .and_then(|()| file.sync_all())
        .map_err(|e| {
            io_error(format!(
                "artifact write incomplete at {}: {e}; no overwrite or removal attempted",
                path.display()
            ))
        })
}

fn read_artifact(path: &Path) -> Result<Vec<u8>, Refusal> {
    let file =
        File::open(path).map_err(|e| io_error(format!("artifact {}: {e}", path.display())))?;
    if !file
        .metadata()
        .map_err(|e| io_error(e.to_string()))?
        .is_file()
    {
        return Err(io_error("artifact must be a regular file"));
    }
    let mut bytes = Vec::new();
    file.take(MAX_ARTIFACT_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| io_error(e.to_string()))?;
    if bytes.len() as u64 > MAX_ARTIFACT_BYTES {
        return Err(io_error("artifact exceeds 64 MiB bound"));
    }
    Ok(bytes)
}

fn hash_file(path: &Path) -> Result<Digest32, Refusal> {
    let mut file =
        File::open(path).map_err(|e| io_error(format!("binary image {}: {e}", path.display())))?;
    if !file
        .metadata()
        .map_err(|e| io_error(e.to_string()))?
        .is_file()
    {
        return Err(io_error("binary image must be a regular file"));
    }
    let mut hash = Blake3::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let count = file
            .read(&mut buffer)
            .map_err(|e| io_error(e.to_string()))?;
        if count == 0 {
            return Ok(hash.finalize());
        }
        hash.update(&buffer[..count]);
    }
}

fn executable() -> Result<PathBuf, Refusal> {
    // Linux's image handle survives replacement of the original executable path.
    if cfg!(target_os = "linux") {
        return Ok(PathBuf::from("/proc/self/exe"));
    }
    std::env::current_exe().map_err(|e| io_error(format!("runtime executable: {e}")))
}

fn artifact_bytes(env: &Envelope, format: OutputFormat) -> Vec<u8> {
    let body = match format {
        OutputFormat::Json => env.to_json(),
        OutputFormat::Markdown => format!(
            "# fmine `{}` ({})\n\n```json\n{}\n```",
            env.verb,
            env.status,
            env.to_json()
        ),
    };
    // This exact buffer, including LF, is hashed and persisted. The issuance receipt is separate.
    format!("{body}\n").into_bytes()
}

fn operation_name(operation: &Operation) -> &'static str {
    match operation {
        Operation::Init { .. } => "init",
        Operation::Issue { .. } => "issue",
        Operation::Check { .. } => "check",
        Operation::Reuse { .. } => "reuse",
        Operation::Withdraw { .. } => "withdraw",
        Operation::Supersede { .. } => "supersede",
        Operation::Refresh { .. } => "refresh",
        Operation::RecallRevision(_) | Operation::RecallClaim(_) => "recall",
    }
}

fn issue_json(issue: &IssuanceRecord) -> String {
    let claims = issue
        .output_claims
        .iter()
        .map(|id| text(&id.to_string()))
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"output_digest\":{},\"output_digest_algorithm\":\"blake3\",\"output_claims\":[{claims}],\
         \"anchor_id\":{},\"state_root\":{},\"code_revision\":{},\"code_revision_evidence\":\"caller_declared\",\
         \"binary_digest\":{},\"binary_digest_algorithm\":\"blake3\",\"input_manifest_root\":{},\
         \"lane\":{},\"confidentiality\":\"public\",\"public_taint\":{},\"recipient_role\":{},\
         \"purpose\":{},\"issued_at_ns\":{}}}",
        text(&issue.output_digest.to_hex()),
        text(&issue.anchor.anchor_id().to_string()),
        text(&issue.anchor.state_root.to_hex()),
        text(&issue.code_revision),
        text(&issue.binary_digest.to_hex()),
        text(&issue.input_manifest_root.to_hex()),
        text(issue.lane.as_str()),
        issue.public_taint,
        text(&issue.recipient_role.to_string()),
        text(&issue.purpose),
        issue.issued_at_ns,
    )
}

fn check_json(check: &RelianceCheck) -> String {
    let (reason, claims) = match &check.status {
        RelianceStatus::Unknown { reason } => (text(reason.as_str()), Vec::new()),
        RelianceStatus::Withdrawn {
            reason,
            affected_claims,
            ..
        } => (text(reason), affected_claims.clone()),
        _ => ("null".to_string(), Vec::new()),
    };
    let replacement = check
        .status
        .replacement()
        .map_or_else(|| "null".to_string(), |d| text(&d.to_hex()));
    let claims = claims
        .iter()
        .map(|id| text(&id.to_string()))
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"status\":{},\"current_reuse_allowed\":{},\"status_cut_ns\":{},\"reason\":{reason},\
         \"replacement_digest\":{replacement},\"affected_claims\":[{claims}]}}",
        text(check.status.as_str()),
        check.current_reuse_allowed,
        check.status_cut_ns,
    )
}

fn receipt(operation: &str, journal: &Journal, fields: &str) -> String {
    format!(
        "{{\"schema\":\"fmine.reliance.v1\",\"operation\":{},\"journal_head\":{},\
        \"head_algorithm\":\"blake3_canonical_record_line\",{fields}}}",
        text(operation),
        text(&journal.head().to_hex())
    )
}

pub(crate) fn execute(command: &Command, root: &Path) -> Envelope {
    let verb = format!("reliance {}", operation_name(&command.operation));
    let result = executable().and_then(|image| execute_with(command, root, &now_ns, &image));
    let mut envelope = match result {
        Ok(payload) => Envelope::ok(verb, payload, Vec::new()),
        Err(refusal) => Envelope::refused(verb, refusal, Vec::new()),
    };
    envelope.no_claim = Some(NO_CLAIM);
    envelope
}

/// The executable and clock are runtime dependencies. CLI arguments can select neither a historical
/// timestamp nor an arbitrary binary digest; tests inject small synthetic images and deterministic clocks.
fn execute_with(
    command: &Command,
    root: &Path,
    clock: &dyn Fn() -> Result<i64, Refusal>,
    image: &Path,
) -> Result<String, Refusal> {
    let access = &command.access;
    let path = absolute(root, &access.journal);
    match &command.operation {
        Operation::Init {
            key,
            role,
            max_age_ns,
            max_skew_ns,
        } => {
            let key = ReleaseKey::read(&absolute(root, key))?;
            let header = Header {
                release_role: role.clone(),
                release_key_sha256: key.sha256(),
                max_status_age_ns: *max_age_ns,
                max_clock_skew_ns: *max_skew_ns,
                created_at_ns: clock()?,
            };
            let journal = Journal::create(&path, header)?;
            Ok(receipt(
                "init",
                &journal,
                "\"current_reuse_allowed\":false,\"pin_instruction\":\"retain this head through an independently trusted channel; the log cannot authenticate itself\"",
            ))
        }
        Operation::Check { digest, artifact } => {
            // Let the kernel report Unknown on a missing or mismatched pin rather than promoting a log
            // simply because it parses. The shared lock covers artifact reads and the status decision.
            let journal = Journal::open_read(&path, None)?;
            let time = trusted_time(access, journal.ledger().header(), clock()?);
            let context = TrustContext {
                pinned_head: access.head,
                time,
            };
            let check = journal.ledger().check(*digest, None, &context);
            let verified = if let Some(artifact) = artifact {
                let bytes = read_artifact(&absolute(root, artifact))?;
                journal.ledger().verify_artifact(*digest, &bytes)?;
                true
            } else {
                false
            };
            Ok(receipt(
                "check",
                &journal,
                &format!(
                    "\"output_digest\":{},\"artifact_bytes_verified\":{verified},\"head_verified\":{},\"clock_evidence\":{},\"reliance\":{}",
                    text(&digest.to_hex()),
                    access.head == Some(journal.head()),
                    text(if time.is_some() {
                        "operator_declared_bounded_host_clock"
                    } else {
                        "offline_uncertain"
                    }),
                    check_json(&check)
                ),
            ))
        }
        Operation::Reuse { artifact, output } => {
            let journal = Journal::open_read(&path, None)?;
            // Read once from one file handle; these same bytes are hashed and written under the journal
            // read lock. Replacing the source path between verification and copying cannot change them.
            let bytes = read_artifact(&absolute(root, artifact))?;
            let digest = blake3_hash(&bytes);
            let context = TrustContext {
                pinned_head: access.head,
                time: trusted_time(access, journal.ledger().header(), clock()?),
            };
            journal.ledger().gate_current_reuse(digest, &context)?;
            journal.ledger().verify_artifact(digest, &bytes)?;
            write_new(&absolute(root, output), &bytes)?;
            Ok(receipt(
                "reuse",
                &journal,
                &format!(
                    "\"output_digest\":{},\"artifact_bytes_verified\":true,\"current_reuse_allowed\":true,\"output\":{}",
                    text(&digest.to_hex()),
                    text(&output.to_string_lossy())
                ),
            ))
        }
        Operation::RecallRevision(_) | Operation::RecallClaim(_) => {
            let journal = Journal::open_read(&path, access.head)?;
            let records = match &command.operation {
                Operation::RecallRevision(revision) => journal.ledger().recall_revision(revision),
                Operation::RecallClaim(claim) => journal.ledger().recall_claim(*claim),
                _ => Vec::new(),
            };
            let copies = records
                .iter()
                .map(|i| issue_json(i))
                .collect::<Vec<_>>()
                .join(",");
            Ok(receipt(
                "recall",
                &journal,
                &format!(
                    "\"scope\":\"recorded_local_issuances_only\",\"head_verified\":{},\"known_copy_count\":{},\"notifications_sent\":0,\"copies\":[{copies}]",
                    access.head == Some(journal.head()),
                    records.len()
                ),
            ))
        }
        _ => execute_mutation(command, root, clock, image),
    }
}

fn execute_mutation(
    command: &Command,
    root: &Path,
    clock: &dyn Fn() -> Result<i64, Refusal>,
    image: &Path,
) -> Result<String, Refusal> {
    let access = &command.access;
    let key_path = match &command.operation {
        Operation::Issue { key, .. }
        | Operation::Withdraw { key, .. }
        | Operation::Supersede { key, .. }
        | Operation::Refresh { key } => key,
        _ => return Err(io_error("operation is not a journal mutation")),
    };
    let key = ReleaseKey::read(&absolute(root, key_path))?;
    let head = access.head.ok_or_else(|| {
        Refusal::new(
            RefusalCode::NotObservable202,
            "missing independently pinned head",
        )
    })?;
    let mut journal = Journal::open_write(&absolute(root, &access.journal), head)?;
    let by_role = journal.ledger().header().release_role.clone();
    let mut artifact = None;
    let entry = match &command.operation {
        Operation::Issue {
            output,
            issuer,
            through,
            recipient,
            purpose,
            revision,
            artifact_format,
            ..
        } => {
            let prepared = reliance_binding::prepare(root, issuer.as_deref(), through.as_deref())?;
            let bytes = artifact_bytes(&prepared.envelope, *artifact_format);
            if bytes.len() as u64 > MAX_ARTIFACT_BYTES {
                return Err(io_error("rendered report exceeds 64 MiB artifact bound"));
            }
            let binary_digest = hash_file(image)?;
            let issue = IssuanceRecord {
                output_digest: blake3_hash(&bytes),
                output_claims: prepared.claim_ids,
                anchor: prepared.anchor,
                code_revision: revision.clone(),
                binary_digest,
                input_manifest_root: prepared.input_manifest_root,
                lane: if prepared.lane == fmine_public::Lane::Public {
                    IssuanceLane::Public
                } else {
                    IssuanceLane::Synthetic
                },
                confidentiality: prepared.taint,
                public_taint: prepared.lane == fmine_public::Lane::Public,
                recipient_role: recipient.clone(),
                purpose: purpose.clone(),
                issued_at_ns: clock()?,
            };
            artifact = Some((absolute(root, output), bytes));
            Entry::Issued(issue)
        }
        Operation::Withdraw {
            digest,
            reason,
            claims,
            defect,
            ..
        } => Entry::Withdrawn(Withdrawal {
            output_digest: *digest,
            reason: reason.clone(),
            affected_claims: if claims.is_empty() {
                journal
                    .ledger()
                    .copies(*digest)
                    .first()
                    .map_or_else(Vec::new, |i| i.output_claims.clone())
            } else {
                claims.clone()
            },
            defect_class: *defect,
            by_role,
            at_ns: clock()?,
        }),
        Operation::Supersede {
            digest,
            replacement,
            ..
        } => Entry::Superseded(Supersession {
            output_digest: *digest,
            by_digest: *replacement,
            by_role,
            at_ns: clock()?,
        }),
        Operation::Refresh { .. } => Entry::Refreshed(StatusRefresh {
            by_role,
            at_ns: clock()?,
        }),
        _ => return Err(io_error("operation is not a journal mutation")),
    };
    let time = trusted_time(access, journal.ledger().header(), entry.at_ns()).ok_or_else(|| {
        Refusal::new(
            RefusalCode::NotObservable202,
            "host clock bound was not explicitly trusted",
        )
    })?;
    journal.validate_append(&entry, &key, time)?;
    if let Some((path, bytes)) = &artifact {
        write_new(path, bytes)?;
    }
    if let Err(error) = journal.append(entry.clone(), &key, time) {
        if artifact.is_some() {
            return Err(Refusal::new(
                RefusalCode::InputMissingOrUnreadable506,
                format!(
                    "artifact bytes were saved but issuance durability is unconfirmed: {}; retain the artifact and resolve the journal before any reuse",
                    error.what
                ),
            ));
        }
        return Err(error);
    }
    let fields = match &entry {
        Entry::Issued(issue) => format!("\"issuance\":{}", issue_json(issue)),
        _ => "\"status_event_recorded\":true".to_string(),
    };
    Ok(receipt(
        operation_name(&command.operation),
        &journal,
        &fields,
    ))
}

#[cfg(all(test, unix))]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::os::unix::fs::OpenOptionsExt;
    use std::sync::atomic::{AtomicU64, Ordering};

    const REVISION: &str = "1111111111111111111111111111111111111111";
    const OTHER_REVISION: &str = "2222222222222222222222222222222222222222";
    const START: i64 = 1_800_000_000_000_000_000;
    static SERIAL: AtomicU64 = AtomicU64::new(0);

    fn fixture() -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "fmine-reliance-cli-{}-{}-{}",
            std::process::id(),
            now_ns().unwrap(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&root).unwrap();
        for (path, value) in [("release.key", 19), ("wrong.key", 27)] {
            let mut f = OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(root.join(path))
                .unwrap();
            f.write_all(&[value; 32]).unwrap();
        }
        // Explicit test runtime image; production obtains its own executable, never an argument.
        std::fs::write(
            root.join("runtime-image"),
            b"synthetic executable image for binding test",
        )
        .unwrap();
        call(&root, &["init", "--release-key", "release.key"], START).unwrap();
        root
    }

    fn call(root: &Path, words: &[&str], at: i64) -> Result<String, Refusal> {
        let args: Vec<String> = ["reliance"]
            .into_iter()
            .chain(words.iter().copied())
            .chain(["--journal", "journal.log"])
            .map(str::to_string)
            .collect();
        let (command, _, workspace) = parse(&args, root).unwrap();
        execute_with(
            &command,
            &workspace,
            &|| Ok(at),
            &root.join("runtime-image"),
        )
    }

    fn head(root: &Path) -> String {
        Journal::open_read(&root.join("journal.log"), None)
            .unwrap()
            .head()
            .to_hex()
    }

    fn issue(
        root: &Path,
        output: &str,
        recipient: &str,
        format: &str,
        rev: &str,
        key: &str,
        at: i64,
    ) -> Result<String, Refusal> {
        call(
            root,
            &[
                "issue",
                "--synthetic",
                "--release-key",
                key,
                "--head",
                &head(root),
                "--trust-host-clock",
                "--output",
                output,
                "--recipient-role",
                recipient,
                "--purpose",
                "review",
                "--code-revision",
                rev,
                "--artifact-format",
                format,
            ],
            at,
        )
    }

    fn withdraw(root: &Path, digest: &str, at: i64) -> Result<String, Refusal> {
        call(
            root,
            &[
                "withdraw",
                "--digest",
                digest,
                "--release-key",
                "release.key",
                "--head",
                &head(root),
                "--trust-host-clock",
                "--defect",
                "code-defect",
                "--all-claims",
                "--reason",
                "Defective interpretation with unchanged source bytes",
            ],
            at,
        )
    }

    #[test]
    fn issue_recall_withdraw_and_gate_exact_bytes_end_to_end() {
        let root = fixture();
        let first = issue(
            &root,
            "first.json",
            "sponsor",
            "json",
            REVISION,
            "release.key",
            START + 1,
        )
        .unwrap();
        issue(
            &root,
            "second.json",
            "reviewer-pool-2",
            "json",
            REVISION,
            "release.key",
            START + 2,
        )
        .unwrap();
        let bytes = std::fs::read(root.join("first.json")).unwrap();
        assert_eq!(bytes, std::fs::read(root.join("second.json")).unwrap());
        assert_eq!(bytes.last(), Some(&b'\n'));
        let output = blake3_hash(&bytes).to_hex();
        assert!(first.contains(&output));
        assert!(
            first.contains(&blake3_hash(b"synthetic executable image for binding test").to_hex())
        );
        assert!(first.contains("\"code_revision_evidence\":\"caller_declared\""));
        let recall = call(&root, &["recall", "--code-revision", REVISION], START + 2).unwrap();
        assert!(recall.contains("\"known_copy_count\":2"));
        assert!(recall.contains("\"notifications_sent\":0"));
        let clockless = call(
            &root,
            &["check", "--digest", &output, "--head", &head(&root)],
            START + 2,
        )
        .unwrap();
        assert!(clockless.contains("\"status\":\"unknown\""));
        assert!(clockless.contains("trusted_time_unavailable"));
        let current = call(
            &root,
            &[
                "check",
                "--digest",
                &output,
                "--artifact",
                "first.json",
                "--head",
                &head(&root),
                "--trust-host-clock",
            ],
            START + 2,
        )
        .unwrap();
        assert!(current.contains("\"artifact_bytes_verified\":true"));
        assert!(current.contains("\"current_reuse_allowed\":true"));
        call(
            &root,
            &[
                "reuse",
                "--artifact",
                "first.json",
                "--output",
                "permitted.json",
                "--head",
                &head(&root),
                "--trust-host-clock",
            ],
            START + 2,
        )
        .unwrap();
        assert_eq!(bytes, std::fs::read(root.join("permitted.json")).unwrap());
        withdraw(&root, &output, START + 3).unwrap();
        let withdrawn = call(
            &root,
            &[
                "check",
                "--digest",
                &output,
                "--artifact",
                "first.json",
                "--head",
                &head(&root),
                "--trust-host-clock",
            ],
            START + 3,
        )
        .unwrap();
        assert!(withdrawn.contains("\"status\":\"withdrawn\""));
        assert!(withdrawn.contains("\"artifact_bytes_verified\":true"));
        let refusal = call(
            &root,
            &[
                "reuse",
                "--artifact",
                "first.json",
                "--output",
                "blocked.json",
                "--head",
                &head(&root),
                "--trust-host-clock",
            ],
            START + 3,
        )
        .unwrap_err();
        assert_eq!(refusal.code, "FMINE-REF-202");
        assert!(!root.join("blocked.json").exists());
        assert_eq!(bytes, std::fs::read(root.join("first.json")).unwrap());
        let record = Journal::open_read(&root.join("journal.log"), None).unwrap();
        let original = record.ledger().copies(digest(&output).unwrap())[0];
        assert!(!original.output_claims.is_empty());
        assert_eq!(
            record
                .ledger()
                .recall_claim(original.output_claims[0])
                .len(),
            2
        );
    }

    #[test]
    fn replacement_can_be_withdrawn_and_cannot_resurrect_original() {
        let root = fixture();
        issue(
            &root,
            "original.json",
            "sponsor",
            "json",
            REVISION,
            "release.key",
            START + 1,
        )
        .unwrap();
        issue(
            &root,
            "replacement.md",
            "sponsor",
            "markdown",
            OTHER_REVISION,
            "release.key",
            START + 2,
        )
        .unwrap();
        let original = blake3_hash(&std::fs::read(root.join("original.json")).unwrap()).to_hex();
        let replacement =
            blake3_hash(&std::fs::read(root.join("replacement.md")).unwrap()).to_hex();
        assert_ne!(original, replacement);
        call(
            &root,
            &[
                "supersede",
                "--digest",
                &original,
                "--replacement",
                &replacement,
                "--release-key",
                "release.key",
                "--head",
                &head(&root),
                "--trust-host-clock",
            ],
            START + 3,
        )
        .unwrap();
        let old = call(
            &root,
            &[
                "check",
                "--digest",
                &original,
                "--head",
                &head(&root),
                "--trust-host-clock",
            ],
            START + 3,
        )
        .unwrap();
        assert!(old.contains("\"status\":\"superseded\""));
        assert!(old.contains(&replacement));
        withdraw(&root, &replacement, START + 4).unwrap();
        for source in ["original.json", "replacement.md"] {
            let error = call(
                &root,
                &[
                    "reuse",
                    "--artifact",
                    source,
                    "--output",
                    "denied-copy",
                    "--head",
                    &head(&root),
                    "--trust-host-clock",
                ],
                START + 4,
            )
            .unwrap_err();
            assert_eq!(error.code, "FMINE-REF-202");
        }
        assert!(
            issue(
                &root,
                "resurrected.json",
                "reviewer-pool-2",
                "json",
                OTHER_REVISION,
                "release.key",
                START + 5
            )
            .is_err()
        );
        assert!(!root.join("resurrected.json").exists());
        let recall = call(
            &root,
            &["recall", "--code-revision", OTHER_REVISION],
            START + 5,
        )
        .unwrap();
        assert!(recall.contains("\"known_copy_count\":1"));
    }

    #[test]
    fn bad_key_existing_output_and_tampering_do_not_issue_or_overwrite() {
        let root = fixture();
        let initial = std::fs::read(root.join("journal.log")).unwrap();
        let err = issue(
            &root,
            "bad-key.json",
            "sponsor",
            "json",
            REVISION,
            "wrong.key",
            START + 1,
        )
        .unwrap_err();
        assert_eq!(err.code, "FMINE-REF-304");
        assert!(!root.join("bad-key.json").exists());
        assert_eq!(initial, std::fs::read(root.join("journal.log")).unwrap());
        std::fs::write(root.join("reserved.json"), b"existing artifact remains").unwrap();
        assert!(
            issue(
                &root,
                "reserved.json",
                "sponsor",
                "json",
                REVISION,
                "release.key",
                START + 1
            )
            .is_err()
        );
        assert_eq!(
            std::fs::read(root.join("reserved.json")).unwrap(),
            b"existing artifact remains"
        );
        assert_eq!(initial, std::fs::read(root.join("journal.log")).unwrap());
        issue(
            &root,
            "good.json",
            "sponsor",
            "json",
            REVISION,
            "release.key",
            START + 1,
        )
        .unwrap();
        let expected = blake3_hash(&std::fs::read(root.join("good.json")).unwrap()).to_hex();
        std::fs::write(root.join("altered.json"), b"altered content").unwrap();
        assert_eq!(
            call(
                &root,
                &["check", "--digest", &expected, "--artifact", "altered.json"],
                START + 2
            )
            .unwrap_err()
            .code,
            "FMINE-REF-505"
        );
    }

    #[test]
    fn missing_stale_and_mismatched_authority_never_copy_and_refresh_is_explicit() {
        let root = fixture();
        let old_head = head(&root);
        issue(
            &root,
            "report.json",
            "sponsor",
            "json",
            REVISION,
            "release.key",
            START + 1,
        )
        .unwrap();
        let output = blake3_hash(&std::fs::read(root.join("report.json")).unwrap()).to_hex();
        for args in [
            vec!["check", "--digest", &output, "--trust-host-clock"],
            vec![
                "check",
                "--digest",
                &output,
                "--head",
                &old_head,
                "--trust-host-clock",
            ],
        ] {
            assert!(
                call(&root, &args, START + 2)
                    .unwrap()
                    .contains("\"status\":\"unknown\"")
            );
        }
        let later = START + 301_000_000_000;
        let stale = call(
            &root,
            &[
                "check",
                "--digest",
                &output,
                "--head",
                &head(&root),
                "--trust-host-clock",
            ],
            later,
        )
        .unwrap();
        assert!(stale.contains("stale_status_cut"));
        assert_eq!(
            call(
                &root,
                &[
                    "reuse",
                    "--artifact",
                    "report.json",
                    "--output",
                    "stale-copy",
                    "--head",
                    &head(&root),
                    "--trust-host-clock"
                ],
                later
            )
            .unwrap_err()
            .code,
            "FMINE-REF-202"
        );
        assert!(!root.join("stale-copy").exists());
        call(
            &root,
            &[
                "refresh",
                "--release-key",
                "release.key",
                "--head",
                &head(&root),
                "--trust-host-clock",
            ],
            later,
        )
        .unwrap();
        let fresh = call(
            &root,
            &[
                "check",
                "--digest",
                &output,
                "--head",
                &head(&root),
                "--trust-host-clock",
            ],
            later,
        )
        .unwrap();
        assert!(fresh.contains("\"current_reuse_allowed\":true"));
    }

    #[test]
    fn parser_rejects_implicit_authority_names_backdates_and_unused_flags() {
        let root = Path::new(".");
        let args = [
            "--json",
            "reliance",
            "check",
            "--digest",
            &"a".repeat(64),
            "--journal",
            "journal",
        ]
        .into_iter()
        .map(str::to_string)
        .collect::<Vec<_>>();
        let (command, format, _) = crate::parse_args(&args, root).unwrap();
        assert!(matches!(command, crate::CliCommand::Reliance(_)));
        assert_eq!(format, OutputFormat::Json);
        for words in [
            vec![
                "reliance",
                "check",
                "--digest",
                &"a".repeat(64),
                "--as-of",
                "2000-01-01",
            ],
            vec!["reliance", "refresh", "--release-key", "key"],
            vec![
                "reliance",
                "init",
                "--release-key",
                "key",
                "--release-role",
                "person@example.invalid",
            ],
            vec![
                "reliance",
                "recall",
                "--code-revision",
                REVISION,
                "--trust-host-clock",
            ],
            vec![
                "reliance",
                "check",
                "--digest",
                &"a".repeat(64),
                "--digest",
                &"b".repeat(64),
            ],
        ] {
            let args = words
                .into_iter()
                .chain(["--journal", "journal"])
                .map(str::to_string)
                .collect::<Vec<_>>();
            assert!(parse(&args, root).is_err(), "{args:?}");
        }
    }
}
