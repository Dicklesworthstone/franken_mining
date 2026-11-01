//! DESIGN (BD-347): local issued-output authority. Exact bytes and a fresh pinned status cut are
//! independent checks. A release capability identifies a local role, never physical human review.

use std::collections::BTreeMap;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

use fmine_calib::record::parse_utc;
use fmine_claim::ClaimId;
use fmine_claim::reliance::{
    DefectClass, Entry, Header, IssuanceLane, IssuanceRecord, RelianceCheck, RelianceStatus,
    RoleId, StatusRefresh, Supersession, TrustContext, TrustedTime, Withdrawal,
};
use fmine_core::hash::{Blake3, Digest32, blake3_hash};
use fmine_core::id::PrefixKind;
use fmine_evidence::{Refusal, RefusalCode};
use fmine_io::reliance::{Journal, MAX_ARTIFACT_BYTES, ReissueRequest, ReleaseKey, read_artifact};
use fmine_runtime::{HostWallClock, WallClock};

use crate::{Envelope, OutputFormat, escape_json, reliance_binding};

pub(crate) const VERBS_JSON: &str = concat!(
    r#""assure public --synthetic|--issuer <issuer> --output <new-path> --journal <path> --head <hex> --release-key <path> --trust-host-clock --recipient-role <role> --purpose review --code-revision <git-sha>","#,
    r#""reliance init --journal <path> --release-key <private-32-byte-file>","#,
    r#""reliance issue --journal <path> --head <hex> --release-key <path> --trust-host-clock --synthetic|--issuer <issuer> --output <new-path> --recipient-role <role> --purpose review --code-revision <git-sha>","#,
    r#""reliance check --journal <path> --digest <hex> [--artifact <path>] [--head <hex> --trust-host-clock] [--as-of <UTC>|--require-current]","#,
    r#""reliance reuse --journal <path> --artifact <path> --output <new-path> --head <hex> --trust-host-clock","#,
    r#""reliance reissue --journal <path> --head <hex> --release-key <path> --trust-host-clock --artifact <path> --output <new-path> --source-issuance <hex> --recipient-role <role> --purpose review","#,
    r#""reliance withdraw --journal <path> --head <hex> --release-key <path> --trust-host-clock --digest <hex> --defect code-defect|source-restated|scope-error --claims <ids>|--all-claims --reason <text>","#,
    r#""reliance supersede --journal <path> --head <hex> --release-key <path> --trust-host-clock --digest <hex> --replacement <hex>","#,
    r#""reliance refresh --journal <path> --head <hex> --release-key <path> --trust-host-clock","#,
    r#""reliance recall --journal <path> --code-revision <git-sha>|--claim <id>""#,
);
pub(crate) const ROBOT_DOCS_JSON: &str = r#"{"mode":"local_capability_authority","default_read":"unknown_without_trusted_head_and_time","pin":"exact final head obtained independently of untrusted journal bytes","clock":"explicit operator-declared bounded host clock; never external time proof","historical_check":"--as-of UTC queries history under a fresh trusted status cut and never grants current reuse","current_gate":"check --require-current refuses unknown, withdrawn and superseded outputs while retaining status diagnostics; excludes --as-of","artifact":"exact stored bytes including final newline; receipt is separate","assurance_export":"assure public --output requires issuance authority and recipient; --format selects artifact and receipt format","issuance_id":"BLAKE3 of the canonical issuance frame; distinct from artifact digest and journal head","revision":"issue and assure public --output declare the Git SHA; executable BLAKE3 measured separately; reissue preserves the selected producer metadata","reissue":"selects an exact source issuance and records the new recipient after a fresh current-status check","recall":"recorded local issuances only; reissue registers another recipient, reuse does not; no notifications","release_role":"release-owner capability; no human-review attestation","roles":["release-owner","sponsor","reviewer-pool-N","agent-session:<32 lowercase hex>"],"purpose":["analysis","review","decision-support"]}"#;
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
        as_of_ns: Option<i64>,
        require_current: bool,
    },
    Reuse {
        artifact: PathBuf,
        output: PathBuf,
    },
    Reissue {
        key: PathBuf,
        artifact: PathBuf,
        output: PathBuf,
        source_issuance: Digest32,
        recipient: RoleId,
        purpose: String,
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

fn purpose(fields: &mut BTreeMap<String, String>) -> Result<String, String> {
    let purpose = required(fields, "--purpose")?;
    if !matches!(purpose.as_str(), "analysis" | "review" | "decision-support") {
        return Err(
            "--purpose must be analysis, review or decision-support (no personal data)".to_string(),
        );
    }
    Ok(purpose)
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
    parse_operation(args, root, false)
}

/// An explicit assurance export shares the issuance parser and authority boundary.
pub(crate) fn parse_export(
    args: &[String],
    root: &Path,
) -> Result<(Command, OutputFormat, PathBuf), String> {
    parse_operation(args, root, true)
}

fn parse_operation(
    args: &[String],
    root: &Path,
    assurance_export: bool,
) -> Result<(Command, OutputFormat, PathBuf), String> {
    let mut fields = BTreeMap::new();
    let mut words = Vec::new();
    let mut i = 0;
    while let Some(arg) = args.get(i) {
        if arg.starts_with("--") {
            let value = if matches!(
                arg.as_str(),
                "--synthetic"
                    | "--trust-host-clock"
                    | "--all-claims"
                    | "--json"
                    | "--require-current"
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
    let verb = match words.as_slice() {
        ["reliance", verb] if !assurance_export => *verb,
        ["assure", "public"] if assurance_export => "issue",
        _ => {
            return Err(if assurance_export {
                "expected assure public with --output and issuance options".to_string()
            } else {
                "expected reliance init|issue|check|reuse|reissue|withdraw|supersede|refresh|recall"
                    .to_string()
            });
        }
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
    let operation = match verb {
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
            let purpose = purpose(&mut fields)?;
            Operation::Issue {
                key: PathBuf::from(required(&mut fields, "--release-key")?),
                output: PathBuf::from(required(&mut fields, "--output")?),
                issuer,
                through,
                purpose,
                recipient: role(&required(&mut fields, "--recipient-role")?)?,
                revision: revision(required(&mut fields, "--code-revision")?)?,
                artifact_format: if assurance_export {
                    format
                } else {
                    output_format(
                        &fields
                            .remove("--artifact-format")
                            .unwrap_or_else(|| "json".to_string()),
                    )?
                },
            }
        }
        "check" => {
            let as_of_ns = fields.remove("--as-of").map(|value| {
                parse_utc(&value)
                    .and_then(|seconds| seconds.checked_mul(1_000_000_000))
                    .filter(|ns| *ns >= 0)
                    .ok_or_else(|| "--as-of requires YYYY-MM-DDTHH:MM:SSZ within nonnegative i64 nanoseconds".to_string())
            }).transpose()?;
            let require_current = fields.remove("--require-current").is_some();
            if require_current && as_of_ns.is_some() {
                return Err(
                    "--as-of is historical inspection and cannot grant --require-current"
                        .to_string(),
                );
            }
            Operation::Check {
                digest: digest(&required(&mut fields, "--digest")?)?,
                artifact: fields.remove("--artifact").map(PathBuf::from),
                as_of_ns,
                require_current,
            }
        }
        "reuse" => Operation::Reuse {
            artifact: PathBuf::from(required(&mut fields, "--artifact")?),
            output: PathBuf::from(required(&mut fields, "--output")?),
        },
        "reissue" => Operation::Reissue {
            key: PathBuf::from(required(&mut fields, "--release-key")?),
            artifact: PathBuf::from(required(&mut fields, "--artifact")?),
            output: PathBuf::from(required(&mut fields, "--output")?),
            source_issuance: digest(&required(&mut fields, "--source-issuance")?)?,
            recipient: role(&required(&mut fields, "--recipient-role")?)?,
            purpose: purpose(&mut fields)?,
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
            | Operation::Reissue { .. }
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

fn unconfirmed_artifact(mut refusal: Refusal) -> Refusal {
    refusal.what.push_str(
        "; no confirmed new issuance receipt was produced; retain any destination artifact and resolve the journal before treating this export as registered",
    );
    refusal
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
        Operation::Reissue { .. } => "reissue",
        Operation::Withdraw { .. } => "withdraw",
        Operation::Supersede { .. } => "supersede",
        Operation::Refresh { .. } => "refresh",
        Operation::RecallRevision(_) | Operation::RecallClaim(_) => "recall",
    }
}

fn issue_json(issue: &IssuanceRecord) -> Result<String, Refusal> {
    let claims = issue
        .output_claims
        .iter()
        .map(|id| text(&id.to_string()))
        .collect::<Vec<_>>()
        .join(",");
    Ok(format!(
        "{{\"issuance_id\":{},\"issuance_id_algorithm\":\"blake3_canonical_issuance_frame\",\
         \"output_digest\":{},\"output_digest_algorithm\":\"blake3\",\"output_claims\":[{claims}],\
         \"anchor_id\":{},\"state_root\":{},\"code_revision\":{},\"code_revision_evidence\":\"recorded\",\
         \"binary_digest\":{},\"binary_digest_algorithm\":\"blake3\",\"input_manifest_root\":{},\
         \"lane\":{},\"confidentiality\":\"public\",\"public_taint\":{},\"recipient_role\":{},\
         \"purpose\":{},\"issued_at_ns\":{}}}",
        text(&issue.digest()?.to_hex()),
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
    ))
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
        "{{\"status\":{},\"current_reuse_allowed\":{},\"as_of_ns\":{},\"status_cut_ns\":{},\"reason\":{reason},\
         \"replacement_digest\":{replacement},\"affected_claims\":[{claims}]}}",
        text(check.status.as_str()),
        check.current_reuse_allowed,
        check
            .as_of_ns
            .map_or_else(|| "null".to_string(), |n| n.to_string()),
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
    if matches!(command.operation, Operation::Check { .. }) {
        let mut envelope = match inspect_check(command, root, &now_ns) {
            Ok((payload, None)) => Envelope::ok(verb, payload, Vec::new()),
            Ok((payload, Some(refusal))) => {
                let mut env = Envelope::refused(verb, refusal, Vec::new());
                env.payload_json = payload;
                env
            }
            Err(refusal) => Envelope::refused(verb, refusal, Vec::new()),
        };
        envelope.no_claim = Some(NO_CLAIM);
        return envelope;
    }
    let result = executable().and_then(|image| execute_with(command, root, &now_ns, &image));
    let mut envelope = match result {
        Ok(payload) => Envelope::ok(verb, payload, Vec::new()),
        Err(refusal) => Envelope::refused(verb, refusal, Vec::new()),
    };
    envelope.no_claim = Some(NO_CLAIM);
    envelope
}

/// Inspection preserves integrity diagnostics independently of freshness. A failed or untrusted clock
/// supplies no trusted time; it cannot prevent historical byte verification or silently grant reuse.
fn inspect_check(
    command: &Command,
    root: &Path,
    clock: &dyn Fn() -> Result<i64, Refusal>,
) -> Result<(String, Option<Refusal>), Refusal> {
    let Operation::Check {
        digest,
        artifact,
        as_of_ns,
        require_current,
    } = &command.operation
    else {
        return Err(io_error("operation is not a reliance check"));
    };
    let access = &command.access;
    // Check reports Unknown on a missing/mismatched pin. The same shared lock covers artifact reads.
    let journal = Journal::open_read(&absolute(root, &access.journal), None)?;
    let time = if access.trust_host_clock {
        clock()
            .ok()
            .and_then(|now| trusted_time(access, journal.ledger().header(), now))
    } else {
        None
    };
    let context = TrustContext {
        pinned_head: access.head,
        time,
    };
    let check = journal.ledger().check(*digest, *as_of_ns, &context);
    let verified = if let Some(artifact) = artifact {
        journal
            .ledger()
            .verify_artifact(*digest, &read_artifact(&absolute(root, artifact))?)?;
        true
    } else {
        false
    };
    let refusal = if *require_current {
        journal.ledger().gate_current_reuse(*digest, &context).err()
    } else {
        None
    };
    let payload = receipt(
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
    );
    Ok((payload, refusal))
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
        Operation::Check { .. } => inspect_check(command, root, clock)
            .and_then(|(payload, refusal)| refusal.map_or(Ok(payload), Err)),
        Operation::Reuse { artifact, output } => {
            let inspected = Journal::open_read(&path, None)?;
            if access.head != Some(inspected.head()) {
                return Err(Refusal::new(
                    RefusalCode::NotObservable202,
                    "current artifact reuse requires an independently trusted matching journal head",
                ));
            }
            let time = if access.trust_host_clock {
                trusted_time(access, inspected.ledger().header(), clock()?)
            } else {
                None
            }
            .ok_or_else(|| {
                Refusal::new(
                    RefusalCode::NotObservable202,
                    "current reuse requires an explicitly trusted bounded clock",
                )
            })?;
            // DESIGN: retain the first shared lock until the independently pinned handle is open;
            // cooperating writers cannot advance the journal between checking the pin and using it.
            let journal = Journal::open_read(&path, access.head)?;
            drop(inspected);
            let digest =
                journal.export_current(&absolute(root, artifact), &absolute(root, output), time)?;
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
                .collect::<Result<Vec<_>, _>>()?
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
        | Operation::Reissue { key, .. }
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
    if let Operation::Reissue {
        artifact,
        output,
        source_issuance,
        recipient,
        purpose,
        ..
    } = &command.operation
    {
        let time = trusted_time(access, journal.ledger().header(), clock()?).ok_or_else(|| {
            Refusal::new(
                RefusalCode::NotObservable202,
                "host clock bound was not explicitly trusted",
            )
        })?;
        let record = journal
            .reissue_current(
                &absolute(root, artifact),
                &absolute(root, output),
                ReissueRequest {
                    source_issuance: *source_issuance,
                    recipient_role: recipient.clone(),
                    purpose: purpose.clone(),
                },
                &key,
                time,
            )
            .map_err(unconfirmed_artifact)?;
        let Entry::Issued(issuance) = record.entry else {
            return Err(io_error("recipient export returned no issuance record"));
        };
        return Ok(receipt(
            "reissue",
            &journal,
            &format!(
                "\"source_issuance_id\":{},\"issuance\":{},\"output\":{},\"delivery_confirmed\":false",
                text(&source_issuance.to_hex()),
                issue_json(&issuance)?,
                text(&output.to_string_lossy()),
            ),
        ));
    }
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
    if let Some((path, bytes)) = artifact {
        let Entry::Issued(issue) = &entry else {
            return Err(io_error("artifact does not belong to an issuance"));
        };
        journal
            .issue_bytes(&path, &bytes, issue.clone(), &key, time)
            .map_err(unconfirmed_artifact)?;
    } else {
        journal.append(entry.clone(), &key, time)?;
    }
    let fields = match &entry {
        Entry::Issued(issue) => format!("\"issuance\":{}", issue_json(issue)?),
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
    use std::fs::OpenOptions;
    use std::io::Write;
    use std::os::unix::fs::{OpenOptionsExt, PermissionsExt, symlink};
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
    fn forwarding_selects_producer_record_and_cannot_refresh_a_stale_cut() {
        let root = fixture();
        issue(
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
            "sponsor",
            "json",
            OTHER_REVISION,
            "release.key",
            START + 2,
        )
        .unwrap();
        let source = {
            let journal = Journal::open_read(&root.join("journal.log"), None).unwrap();
            let records = journal.ledger().issuances();
            assert_eq!(records[0].output_digest, records[1].output_digest);
            assert_ne!(records[0].digest().unwrap(), records[1].digest().unwrap());
            (*records[1]).clone()
        };
        let forward = |destination: &str, key: &str, at: i64| {
            call(
                &root,
                &[
                    "reissue",
                    "--head",
                    &head(&root),
                    "--release-key",
                    key,
                    "--trust-host-clock",
                    "--artifact",
                    "first.json",
                    "--output",
                    destination,
                    "--source-issuance",
                    &source.digest().unwrap().to_hex(),
                    "--recipient-role",
                    "reviewer-pool-8",
                    "--purpose",
                    "analysis",
                ],
                at,
            )
        };
        let before = head(&root);
        assert!(forward("unauthorized.json", "wrong.key", START + 3).is_err());
        assert!(!root.join("unauthorized.json").exists());
        assert_eq!(head(&root), before);
        let stale = forward("stale.json", "release.key", START + 301_000_000_003).unwrap_err();
        assert_eq!(stale.code, "FMINE-REF-202");
        assert!(!root.join("stale.json").exists());
        assert_eq!(
            head(&root),
            before,
            "forwarding cannot refresh the cut before checking it"
        );
        let receipt = forward("copy.json", "release.key", START + 3).unwrap();
        assert!(receipt.contains(OTHER_REVISION));
        assert!(!receipt.contains(REVISION));
        let journal = Journal::open_read(&root.join("journal.log"), None).unwrap();
        assert_eq!(journal.ledger().recall_revision(REVISION).len(), 1);
        let selected = journal.ledger().recall_revision(OTHER_REVISION);
        assert_eq!(selected.len(), 2);
        assert_eq!(selected[1].binary_digest, source.binary_digest);
        assert_eq!(selected[1].input_manifest_root, source.input_manifest_root);
        assert_eq!(selected[1].recipient_role, RoleId::ReviewerPool(8));
        assert!(receipt.contains(&selected[1].digest().unwrap().to_hex()));
    }

    #[test]
    fn assurance_export_format_and_forwarding_options_are_strict() {
        let root = fixture();
        let common = [
            "--output",
            "report.md",
            "assure",
            "public",
            "--synthetic",
            "--journal",
            "journal.log",
            "--head",
            &head(&root),
            "--release-key",
            "release.key",
            "--trust-host-clock",
            "--recipient-role",
            "reviewer-pool-1",
            "--purpose",
            "review",
            "--code-revision",
            REVISION,
            "--format",
            "markdown",
        ]
        .map(str::to_string);
        let (parsed, format, workspace) = crate::parse_args(&common, &root).unwrap();
        let crate::CliCommand::AssureExport(command) = parsed else {
            panic!("expected authority export");
        };
        assert_eq!(format, OutputFormat::Markdown);
        let receipt = execute_with(
            &command,
            &workspace,
            &|| Ok(START + 1),
            &root.join("runtime-image"),
        )
        .unwrap();
        let artifact = std::fs::read_to_string(root.join("report.md")).unwrap();
        assert!(artifact.starts_with("# fmine `assure public` (ok)\n"));
        assert!(artifact.ends_with('\n'));
        assert!(receipt.contains(&blake3_hash(artifact.as_bytes()).to_hex()));
        for extra in [
            vec!["--artifact-format", "json"],
            vec!["--source-issuance", "00"],
            vec!["--json"],
        ] {
            let mut invalid = common.to_vec();
            invalid.extend(extra.into_iter().map(str::to_string));
            assert!(crate::parse_args(&invalid, &root).is_err());
        }
        for missing in [
            "--release-key",
            "--head",
            "--recipient-role",
            "--purpose",
            "--code-revision",
        ] {
            let mut invalid = common.to_vec();
            let i = invalid.iter().position(|v| v == missing).unwrap();
            invalid.drain(i..i + 2);
            assert!(crate::parse_args(&invalid, &root).is_err(), "{missing}");
        }
        let mut unknown_producer = [
            "reliance",
            "reissue",
            "--journal",
            "journal.log",
            "--head",
            &head(&root),
            "--release-key",
            "release.key",
            "--trust-host-clock",
            "--artifact",
            "report.md",
            "--output",
            "copy.md",
            "--source-issuance",
            &"0".repeat(64),
            "--recipient-role",
            "sponsor",
            "--purpose",
            "review",
        ]
        .map(str::to_string)
        .to_vec();
        unknown_producer.extend(["--code-revision".to_string(), OTHER_REVISION.to_string()]);
        assert!(
            crate::parse_args(&unknown_producer, &root).is_err(),
            "forwarding cannot relabel producer metadata"
        );
        assert!(!root.join("copy.md").exists());
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
        assert!(first.contains("\"code_revision_evidence\":\"recorded\""));
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
        let current_head = head(&root);
        for (destination, authority) in [
            ("no-clock-copy", vec!["--head", current_head.as_str()]),
            ("no-pin-copy", vec!["--trust-host-clock"]),
            (
                "mismatched-pin-copy",
                vec!["--head", old_head.as_str(), "--trust-host-clock"],
            ),
        ] {
            let mut args = vec![
                "reuse",
                "--artifact",
                "report.json",
                "--output",
                destination,
            ];
            args.extend(authority);
            assert_eq!(
                call(&root, &args, START + 2).unwrap_err().code,
                "FMINE-REF-202"
            );
            assert!(!root.join(destination).exists());
        }
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
    fn historical_checks_never_grant_reuse_and_current_gate_keeps_diagnostics() {
        let root = fixture();
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
        withdraw(&root, &output, START + 2_000_000_000).unwrap();
        let as_of = fmine_calib::record::format_utc(START / 1_000_000_000 + 1);
        let historical = call(
            &root,
            &[
                "check",
                "--digest",
                &output,
                "--artifact",
                "report.json",
                "--head",
                &head(&root),
                "--trust-host-clock",
                "--as-of",
                &as_of,
            ],
            START + 3_000_000_000,
        )
        .unwrap();
        assert!(historical.contains("\"status\":\"current\""));
        assert!(historical.contains("\"current_reuse_allowed\":false"));
        assert!(historical.contains("\"artifact_bytes_verified\":true"));
        assert!(historical.contains(&format!("\"as_of_ns\":{}", START + 1_000_000_000)));
        let args = [
            "reliance",
            "check",
            "--journal",
            "journal.log",
            "--digest",
            &output,
            "--artifact",
            "report.json",
            "--head",
            &head(&root),
            "--trust-host-clock",
            "--require-current",
        ]
        .into_iter()
        .map(str::to_string)
        .collect::<Vec<_>>();
        let (command, _, _) = parse(&args, &root).unwrap();
        let (payload, refusal) =
            inspect_check(&command, &root, &|| Ok(START + 3_000_000_000)).unwrap();
        assert_eq!(refusal.unwrap().code, "FMINE-REF-202");
        assert!(payload.contains("\"status\":\"withdrawn\""));
        assert!(payload.contains("\"artifact_bytes_verified\":true"));
        let mut offline = command.clone();
        offline.access.trust_host_clock = false;
        let envelope = execute(&offline, &root);
        assert_eq!(envelope.exit_code, 3);
        assert!(envelope.payload_json.contains("trusted_time_unavailable"));
        assert!(
            envelope
                .payload_json
                .contains("\"artifact_bytes_verified\":true")
        );
    }

    #[test]
    fn unavailable_clock_preserves_byte_verification_without_current_permission() {
        let root = fixture();
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
        let args = [
            "reliance",
            "check",
            "--journal",
            "journal.log",
            "--digest",
            &output,
            "--artifact",
            "report.json",
            "--head",
            &head(&root),
            "--trust-host-clock",
        ]
        .into_iter()
        .map(str::to_string)
        .collect::<Vec<_>>();
        let (command, _, _) = parse(&args, &root).unwrap();
        let (payload, refusal) =
            inspect_check(&command, &root, &|| Err(io_error("clock unavailable"))).unwrap();
        assert!(refusal.is_none());
        assert!(payload.contains("\"status\":\"unknown\""));
        assert!(payload.contains("trusted_time_unavailable"));
        assert!(payload.contains("\"artifact_bytes_verified\":true"));
        assert!(payload.contains("\"current_reuse_allowed\":false"));
        let mut offline = command;
        offline.access.trust_host_clock = false;
        let calls = std::cell::Cell::new(0);
        inspect_check(&offline, &root, &|| {
            calls.set(calls.get() + 1);
            Ok(START)
        })
        .unwrap();
        assert_eq!(calls.get(), 0);
    }

    #[test]
    fn canonical_artifact_paths_refuse_symlinks_and_create_private_copies() {
        let root = fixture();
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
        let pin = head(&root);
        assert_eq!(
            std::fs::metadata(root.join("report.json"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
        symlink(root.join("report.json"), root.join("linked.json")).unwrap();
        for args in [
            vec!["check", "--digest", &output, "--artifact", "linked.json"],
            vec![
                "reuse",
                "--artifact",
                "linked.json",
                "--output",
                "blocked.json",
                "--head",
                &pin,
                "--trust-host-clock",
            ],
        ] {
            assert_eq!(
                call(&root, &args, START + 2).unwrap_err().code,
                "FMINE-REF-506"
            );
        }
        assert!(!root.join("blocked.json").exists());
        assert_eq!(head(&root), pin);
        call(
            &root,
            &[
                "reuse",
                "--artifact",
                "report.json",
                "--output",
                "copy.json",
                "--head",
                &pin,
                "--trust-host-clock",
            ],
            START + 2,
        )
        .unwrap();
        assert_eq!(
            std::fs::metadata(root.join("copy.json"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
        assert_eq!(
            std::fs::read(root.join("report.json")).unwrap(),
            std::fs::read(root.join("copy.json")).unwrap()
        );
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
                "2000-01-01T00:00:00Z",
                "--require-current",
            ],
            vec![
                "reliance",
                "check",
                "--digest",
                &"a".repeat(64),
                "--as-of",
                "2300-01-01T00:00:00Z",
            ],
            vec![
                "reliance",
                "recall",
                "--code-revision",
                REVISION,
                "--as-of",
                "2000-01-01T00:00:00Z",
            ],
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
