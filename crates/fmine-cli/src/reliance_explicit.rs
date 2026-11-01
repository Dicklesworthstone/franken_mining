//! DESIGN (BD-347): local reliance commands delegate every status and authority decision to the
//! canonical journal. An observed head and the host clock never become trust inputs automatically.
//! Read commands emit metadata only; recall never contacts recipients or changes an issued artifact.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use fmine_calib::record::{parse_hex32, parse_utc};
use fmine_claim::reliance::{
    DefectClass, Entry, Header, IssuanceRecord, RelianceStatus, RoleId, StatusRefresh,
    Supersession, TrustContext, TrustedTime, Withdrawal,
};
use fmine_core::hash::{Digest32, blake3_hash};
use fmine_core::id::{ObjectId, PrefixKind};
use fmine_evidence::{Refusal, RefusalCode};
use fmine_io::reliance::{Journal, ReleaseKey, read_artifact};

use crate::{Envelope, NextAction, escape_json};

const NO_CLAIM: &str = "Reports only this local issuance journal. A supplied head pin and bounded time are caller trust inputs, not a signature or an external time proof. Recall lists known issued copies; it does not contact recipients or establish that every copy was recalled.";

/// Owner capability and explicit time needed by every post-genesis mutation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MutationAuth {
    pub journal: PathBuf,
    pub key: PathBuf,
    pub expected_head: Digest32,
    pub time: TrustedTime,
}

/// A canonical digest is unambiguous; prefix a hex-named file with `./` to select its bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    Digest(Digest32),
    File(PathBuf),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecallSelector {
    Revision(String),
    Claim(ObjectId),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Init {
        journal: PathBuf,
        key: PathBuf,
        time: TrustedTime,
        max_age_ns: u64,
        max_skew_ns: u64,
    },
    Check {
        journal: PathBuf,
        target: Target,
        trust: TrustContext,
        as_of_ns: Option<i64>,
    },
    Recall {
        journal: PathBuf,
        head: Option<Digest32>,
        selector: RecallSelector,
    },
    Withdraw {
        auth: MutationAuth,
        output: Digest32,
        reason: String,
        defect_class: DefectClass,
        affected_claims: Vec<ObjectId>,
    },
    Supersede {
        auth: MutationAuth,
        output: Digest32,
        replacement: Digest32,
    },
    Refresh(MutationAuth),
}

impl Command {
    #[must_use]
    pub const fn verb(&self) -> &'static str {
        match self {
            Self::Init { .. } => "reliance init",
            Self::Check { .. } => "reliance check",
            Self::Recall { .. } => "recall",
            Self::Withdraw { .. } => "reliance withdraw",
            Self::Supersede { .. } => "reliance supersede",
            Self::Refresh(_) => "reliance refresh",
        }
    }
}

pub(crate) fn parse_digest(value: &str) -> Result<Digest32, String> {
    parse_hex32(value)
        .map(Digest32)
        .ok_or_else(|| "a digest requires exactly sixty-four lowercase hexadecimal digits".into())
}

pub(crate) fn parse_time(value: &str) -> Result<i64, String> {
    parse_utc(value)
        .and_then(|seconds| seconds.checked_mul(1_000_000_000))
        .filter(|ns| *ns >= 0)
        .ok_or_else(|| {
            "time requires a nonnegative, representable UTC instant YYYY-MM-DDTHH:MM:SSZ".into()
        })
}

pub(crate) fn resolve(workspace: &Path, path: &Path) -> PathBuf {
    workspace.join(path)
}

fn claim(value: &str) -> Result<ObjectId, String> {
    value
        .parse::<ObjectId>()
        .ok()
        .filter(|id| id.kind == PrefixKind::Claim)
        .ok_or_else(|| "--claim requires a canonical clm_ Claim id".into())
}

fn unsigned(value: &str, flag: &str) -> Result<u64, String> {
    if value.is_empty() || !value.bytes().all(|b| b.is_ascii_digit()) {
        return Err(format!("{flag} requires an unsigned decimal integer"));
    }
    value
        .parse()
        .map_err(|_| format!("{flag} exceeds its unsigned integer range"))
}

struct Arguments {
    values: BTreeMap<String, String>,
    claims: Vec<ObjectId>,
    words: Vec<String>,
}

impl Arguments {
    fn read(tokens: &[String], allowed: &[&str], multiple_claims: bool) -> Result<Self, String> {
        let mut parsed = Self {
            values: BTreeMap::new(),
            claims: Vec::new(),
            words: Vec::new(),
        };
        let mut i = 0;
        while i < tokens.len() {
            let flag = &tokens[i];
            if !flag.starts_with("--") {
                parsed.words.push(flag.clone());
                i += 1;
                continue;
            }
            if !allowed.contains(&flag.as_str()) {
                return Err(format!("unexpected flag {flag} for this reliance command"));
            }
            i += 1;
            let value = tokens
                .get(i)
                .filter(|v| !v.starts_with("--") && !v.is_empty())
                .ok_or_else(|| format!("missing value for {flag}"))?;
            if flag == "--claim" {
                let id = claim(value)?;
                if (!multiple_claims && !parsed.claims.is_empty()) || parsed.claims.contains(&id) {
                    return Err("duplicate --claim".into());
                }
                parsed.claims.push(id);
            } else if parsed.values.insert(flag.clone(), value.clone()).is_some() {
                return Err(format!("duplicate {flag}"));
            }
            i += 1;
        }
        parsed.claims.sort_unstable();
        Ok(parsed)
    }

    fn required(&self, flag: &str) -> Result<&str, String> {
        self.values
            .get(flag)
            .map(String::as_str)
            .ok_or_else(|| format!("missing {flag}"))
    }

    fn optional(&self, flag: &str) -> Option<&str> {
        self.values.get(flag).map(String::as_str)
    }

    fn path(&self, flag: &str) -> Result<PathBuf, String> {
        self.required(flag).map(PathBuf::from)
    }

    fn uint(&self, flag: &str) -> Result<u64, String> {
        unsigned(self.required(flag)?, flag)
    }

    fn time(&self, required: bool) -> Result<Option<TrustedTime>, String> {
        match (self.optional("--now"), self.optional("--uncertainty-ns")) {
            (Some(now), Some(uncertainty)) => Ok(Some(TrustedTime {
                now_ns: parse_time(now)?,
                uncertainty_ns: unsigned(uncertainty, "--uncertainty-ns")?,
            })),
            (None, None) if !required => Ok(None),
            _ => Err(
                "--now and --uncertainty-ns must both be supplied as explicit trust inputs".into(),
            ),
        }
    }

    fn required_time(&self) -> Result<TrustedTime, String> {
        self.time(true)?
            .ok_or_else(|| "missing explicit trust time".into())
    }

    fn auth(&self) -> Result<MutationAuth, String> {
        Ok(MutationAuth {
            journal: self.path("--journal")?,
            key: self.path("--key")?,
            expected_head: parse_digest(self.required("--expected-head")?)?,
            time: self.required_time()?,
        })
    }

    fn words(&self, count: usize) -> Result<(), String> {
        if self.words.len() == count {
            Ok(())
        } else {
            Err(format!(
                "this reliance command requires {count} positional arguments"
            ))
        }
    }
}

/// Parse full command tokens after the parent removes global workspace and output-format options.
pub fn parse(args: &[String]) -> Result<Command, String> {
    if args.first().map(String::as_str) == Some("recall") {
        let a = Arguments::read(
            &args[1..],
            &["--journal", "--head", "--revision", "--claim"],
            false,
        )?;
        a.words(0)?;
        let selector = match (a.optional("--revision"), a.claims.as_slice()) {
            (Some(revision), []) => RecallSelector::Revision(revision.into()),
            (None, [id]) => RecallSelector::Claim(*id),
            _ => return Err("recall requires exactly one of --revision or --claim".into()),
        };
        return Ok(Command::Recall {
            journal: a.path("--journal")?,
            head: a.optional("--head").map(parse_digest).transpose()?,
            selector,
        });
    }
    if args.first().map(String::as_str) != Some("reliance") {
        return Err("expected reliance or recall".into());
    }
    let action = args
        .get(1)
        .map(String::as_str)
        .ok_or("missing reliance action")?;
    let common = [
        "--journal",
        "--expected-head",
        "--key",
        "--now",
        "--uncertainty-ns",
    ];
    match action {
        "init" => {
            let a = Arguments::read(
                &args[2..],
                &[
                    "--journal",
                    "--key",
                    "--now",
                    "--uncertainty-ns",
                    "--max-age-ns",
                    "--max-skew-ns",
                ],
                false,
            )?;
            a.words(0)?;
            let time = a.required_time()?;
            let max_age_ns = a.uint("--max-age-ns")?;
            let max_skew_ns = a.uint("--max-skew-ns")?;
            if max_age_ns == 0 || max_skew_ns > max_age_ns || time.uncertainty_ns > max_skew_ns {
                return Err("initial clock uncertainty must fit the skew policy, which must fit the positive status-age policy".into());
            }
            Ok(Command::Init {
                journal: a.path("--journal")?,
                key: a.path("--key")?,
                time,
                max_age_ns,
                max_skew_ns,
            })
        }
        "check" => {
            let a = Arguments::read(
                &args[2..],
                &[
                    "--journal",
                    "--head",
                    "--now",
                    "--uncertainty-ns",
                    "--as-of",
                ],
                false,
            )?;
            a.words(1)?;
            let target = match parse_hex32(&a.words[0]) {
                Some(bytes) => Target::Digest(Digest32(bytes)),
                None => Target::File(PathBuf::from(&a.words[0])),
            };
            Ok(Command::Check {
                journal: a.path("--journal")?,
                target,
                trust: TrustContext {
                    pinned_head: a.optional("--head").map(parse_digest).transpose()?,
                    time: a.time(false)?,
                },
                as_of_ns: a.optional("--as-of").map(parse_time).transpose()?,
            })
        }
        "withdraw" => {
            let mut flags = common.to_vec();
            flags.extend(["--reason", "--defect", "--claim"]);
            let a = Arguments::read(&args[2..], &flags, true)?;
            a.words(1)?;
            if a.claims.is_empty() {
                return Err("withdraw requires at least one affected --claim".into());
            }
            let defect_class = match a.required("--defect")? {
                "code-defect" => DefectClass::CodeDefect,
                "source-restated" => DefectClass::SourceRestated,
                "scope-error" => DefectClass::ScopeError,
                _ => {
                    return Err(
                        "--defect requires code-defect, source-restated or scope-error".into(),
                    );
                }
            };
            Ok(Command::Withdraw {
                auth: a.auth()?,
                output: parse_digest(&a.words[0])?,
                reason: a.required("--reason")?.into(),
                defect_class,
                affected_claims: a.claims,
            })
        }
        "supersede" => {
            let mut flags = common.to_vec();
            flags.push("--replacement");
            let a = Arguments::read(&args[2..], &flags, false)?;
            a.words(1)?;
            Ok(Command::Supersede {
                auth: a.auth()?,
                output: parse_digest(&a.words[0])?,
                replacement: parse_digest(a.required("--replacement")?)?,
            })
        }
        "refresh" => {
            let a = Arguments::read(&args[2..], &common, false)?;
            a.words(0)?;
            Ok(Command::Refresh(a.auth()?))
        }
        _ => Err(format!("unknown reliance action {action:?}")),
    }
}

fn js(value: &str) -> String {
    format!("\"{}\"", escape_json(value))
}

fn optional_digest(value: Option<Digest32>) -> String {
    value.map_or_else(|| "null".into(), |d| js(&d.to_hex()))
}

fn ids_json(ids: &[ObjectId]) -> String {
    format!(
        "[{}]",
        ids.iter()
            .map(|id| js(&id.to_string()))
            .collect::<Vec<_>>()
            .join(",")
    )
}

fn copy_json(copy: &IssuanceRecord) -> Result<String, Refusal> {
    Ok(format!(
        "{{\"issuance_digest\":{},\"output_digest\":{},\"output_claims\":{},\"anchor_id\":{},\"state_root\":{},\"code_revision\":{},\"binary_digest\":{},\"input_manifest_root\":{},\"lane\":{},\"public_taint\":{},\"recipient_role\":{},\"purpose\":{},\"issued_at_ns\":{}}}",
        js(&copy.digest()?.to_hex()),
        js(&copy.output_digest.to_hex()),
        ids_json(&copy.output_claims),
        js(&copy.anchor.anchor_id().to_string()),
        js(&copy.anchor.state_root.to_hex()),
        js(&copy.code_revision),
        js(&copy.binary_digest.to_hex()),
        js(&copy.input_manifest_root.to_hex()),
        js(copy.lane.as_str()),
        copy.public_taint,
        js(&copy.recipient_role.to_string()),
        js(&copy.purpose),
        copy.issued_at_ns,
    ))
}

fn copies_json(copies: &[&IssuanceRecord]) -> Result<String, Refusal> {
    Ok(format!(
        "[{}]",
        copies
            .iter()
            .map(|copy| copy_json(copy))
            .collect::<Result<Vec<_>, _>>()?
            .join(",")
    ))
}

fn mutation(auth: &MutationAuth, entry: Entry, workspace: &Path) -> Result<String, Refusal> {
    let key = ReleaseKey::read(&resolve(workspace, &auth.key))?;
    let mut journal = Journal::open_write(&resolve(workspace, &auth.journal), auth.expected_head)?;
    let event = match &entry {
        Entry::Withdrawn(_) => "withdrawn",
        Entry::Superseded(_) => "superseded",
        Entry::Refreshed(_) => "refreshed",
        _ => "unsupported",
    };
    let record = journal.append(entry, &key, auth.time)?;
    Ok(format!(
        "{{\"schema\":\"fmine.reliance.mutation.v1\",\"event\":{},\"previous_head\":{},\"journal_head\":{},\"record_digest\":{},\"records\":{},\"status_cut_ns\":{}}}",
        js(event),
        js(&auth.expected_head.to_hex()),
        js(&journal.head().to_hex()),
        js(&record.digest()?.to_hex()),
        journal.ledger().records().len(),
        journal.ledger().status_cut_ns(),
    ))
}

fn execute_payload(command: &Command, workspace: &Path) -> Result<String, Refusal> {
    match command {
        Command::Init {
            journal,
            key,
            time,
            max_age_ns,
            max_skew_ns,
        } => {
            // Direct callers receive the same clock-policy validation as parsed CLI invocations.
            if time.now_ns < 0 || time.uncertainty_ns > *max_skew_ns {
                return Err(Refusal::new(
                    RefusalCode::MissingHumanToken304,
                    "initial time is outside the owner clock policy",
                ));
            }
            let key = ReleaseKey::read(&resolve(workspace, key))?;
            let journal = Journal::create(
                &resolve(workspace, journal),
                Header {
                    release_role: RoleId::ReleaseOwner,
                    release_key_sha256: key.sha256(),
                    max_status_age_ns: *max_age_ns,
                    max_clock_skew_ns: *max_skew_ns,
                    created_at_ns: time.now_ns,
                },
            )?;
            Ok(format!(
                "{{\"schema\":\"fmine.reliance.init.v1\",\"journal_head\":{},\"release_role\":\"release-owner\",\"max_status_age_ns\":{},\"max_clock_skew_ns\":{},\"status_cut_ns\":{},\"records\":1}}",
                js(&journal.head().to_hex()),
                max_age_ns,
                max_skew_ns,
                time.now_ns,
            ))
        }
        Command::Check {
            journal,
            target,
            trust,
            as_of_ns,
        } => {
            let journal = Journal::open_read(&resolve(workspace, journal), None)?;
            let (digest, bytes) = match target {
                Target::Digest(digest) => (*digest, None),
                Target::File(path) => {
                    let bytes = read_artifact(&resolve(workspace, path))?;
                    (blake3_hash(&bytes), Some(bytes))
                }
            };
            let copies = journal.ledger().copies(digest);
            let verified = match &bytes {
                Some(bytes) if !copies.is_empty() => {
                    journal.ledger().verify_artifact(digest, bytes)?;
                    "true"
                }
                Some(_) => "false",
                None => "null",
            };
            let check = journal.ledger().check(digest, *as_of_ns, trust);
            let (reason, affected) = match &check.status {
                RelianceStatus::Unknown { reason } => (js(reason.as_str()), "[]".into()),
                RelianceStatus::Withdrawn {
                    reason,
                    affected_claims,
                    ..
                } => (js(reason), ids_json(affected_claims)),
                _ => ("null".into(), "[]".into()),
            };
            Ok(format!(
                "{{\"schema\":\"fmine.reliance.check.v1\",\"output_digest\":{},\"reliance_status\":{},\"reason\":{},\"affected_claims\":{},\"replacement\":{},\"current_reuse_allowed\":{},\"historical_query\":{},\"as_of_ns\":{},\"status_cut_ns\":{},\"journal_head\":{},\"head_pin_matched\":{},\"artifact_verified\":{},\"issued_copies\":{}}}",
                js(&digest.to_hex()),
                js(check.status.as_str()),
                reason,
                affected,
                optional_digest(check.status.replacement()),
                check.current_reuse_allowed,
                as_of_ns.is_some(),
                check
                    .as_of_ns
                    .map_or_else(|| "null".into(), |n| n.to_string()),
                check.status_cut_ns,
                js(&journal.head().to_hex()),
                trust.pinned_head == Some(journal.head()),
                verified,
                copies_json(&copies)?,
            ))
        }
        Command::Recall {
            journal,
            head,
            selector,
        } => {
            let journal = Journal::open_read(&resolve(workspace, journal), *head)?;
            let (kind, value, copies) = match selector {
                RecallSelector::Revision(revision) => (
                    "revision",
                    revision.clone(),
                    journal.ledger().recall_revision(revision),
                ),
                RecallSelector::Claim(claim) => (
                    "claim",
                    claim.to_string(),
                    journal.ledger().recall_claim(*claim),
                ),
            };
            Ok(format!(
                "{{\"schema\":\"fmine.reliance.recall.v1\",\"selector\":{{\"kind\":{},\"value\":{}}},\"journal_head\":{},\"head_pin_matched\":{},\"current_status_assessed\":false,\"copies\":{},\"issued_copies\":{}}}",
                js(kind),
                js(&value),
                js(&journal.head().to_hex()),
                journal.is_pinned(),
                copies.len(),
                copies_json(&copies)?,
            ))
        }
        Command::Withdraw {
            auth,
            output,
            reason,
            defect_class,
            affected_claims,
        } => mutation(
            auth,
            Entry::Withdrawn(Withdrawal {
                output_digest: *output,
                reason: reason.clone(),
                affected_claims: affected_claims.clone(),
                defect_class: *defect_class,
                by_role: RoleId::ReleaseOwner,
                at_ns: auth.time.now_ns,
            }),
            workspace,
        ),
        Command::Supersede {
            auth,
            output,
            replacement,
        } => mutation(
            auth,
            Entry::Superseded(Supersession {
                output_digest: *output,
                by_digest: *replacement,
                by_role: RoleId::ReleaseOwner,
                at_ns: auth.time.now_ns,
            }),
            workspace,
        ),
        Command::Refresh(auth) => mutation(
            auth,
            Entry::Refreshed(StatusRefresh {
                by_role: RoleId::ReleaseOwner,
                at_ns: auth.time.now_ns,
            }),
            workspace,
        ),
    }
}

pub fn execute(command: &Command, workspace: &Path) -> Envelope {
    let actions = vec![NextAction::new(
        "fmine robot-docs",
        "reliance trust inputs and commands",
    )];
    match execute_payload(command, workspace) {
        Ok(payload) => {
            let mut envelope = Envelope::ok(command.verb(), payload, actions);
            envelope.no_claim = Some(NO_CLAIM);
            envelope
        }
        Err(refusal) => Envelope::refused(command.verb(), refusal, actions),
    }
}

#[cfg(test)]
mod tests;
