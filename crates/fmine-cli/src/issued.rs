//! DESIGN (BD-347): issue freshly verified assurance bytes and gate retrieval of issued artifacts.
//! Metadata comes from the report and executable; callers supply only release authority, trust and purpose.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use fmine_claim::reliance::{IssuanceLane, IssuanceRecord, RoleId, TrustedTime};
use fmine_core::{Digest32, blake3_hash};
use fmine_evidence::{Refusal, RefusalCode};
use fmine_io::reliance::{Journal, ReleaseKey};
use fmine_public::Lane;
use fmine_public::sources::{is_issuer_name, is_through_bound, registry_path};

use crate::reliance_explicit::{MutationAuth, parse_digest, parse_time, resolve};
use crate::{Envelope, NextAction, escape_json, reliance_binding};

const NO_CLAIM: &str = "Records issuance or retrieves an existing issued copy through the local reliance gate. Caller-supplied clock and head pins are trust inputs, not external time proofs or signatures. Retrieval does not register another recipient or grant regulatory, reserve, safety or execution authority.";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Issue {
        issuer: Option<String>,
        through: Option<String>,
        output: PathBuf,
        auth: MutationAuth,
        recipient_role: RoleId,
        purpose: String,
    },
    Export {
        input: PathBuf,
        output: PathBuf,
        journal: PathBuf,
        head: Digest32,
        time: TrustedTime,
    },
}

impl Command {
    #[must_use]
    pub const fn verb(&self) -> &'static str {
        match self {
            Self::Issue { .. } => "assure issue",
            Self::Export { .. } => "reliance export",
        }
    }
}

/// Parse command tokens after the parent removes global workspace and output-format options.
pub fn parse(args: &[String], workspace: &Path) -> Result<Command, String> {
    let issue = args.first().map(String::as_str) == Some("assure")
        && args.get(1).map(String::as_str) == Some("issue");
    let export = args.first().map(String::as_str) == Some("reliance")
        && args.get(1).map(String::as_str) == Some("export");
    if !issue && !export {
        return Err("expected assure issue or reliance export".into());
    }
    let mut values = BTreeMap::new();
    let mut words = Vec::new();
    let mut synthetic = false;
    let mut index = 2;
    while index < args.len() {
        let flag = args[index].as_str();
        if !flag.starts_with("--") {
            words.push(args[index].clone());
            index += 1;
            continue;
        }
        if issue && flag == "--synthetic" {
            if synthetic {
                return Err("duplicate --synthetic".into());
            }
            synthetic = true;
            index += 1;
            continue;
        }
        let common = matches!(
            flag,
            "--output" | "--journal" | "--now" | "--uncertainty-ns"
        );
        let specific = if issue {
            matches!(
                flag,
                "--issuer" | "--through" | "--expected-head" | "--key" | "--role" | "--purpose"
            )
        } else {
            flag == "--head"
        };
        if !common && !specific {
            return Err(format!(
                "unexpected flag {flag} for issued-artifact command"
            ));
        }
        index += 1;
        let value = args
            .get(index)
            .filter(|v| !v.is_empty() && !v.starts_with("--"))
            .ok_or_else(|| format!("missing value for {flag}"))?;
        if values.insert(flag, value.as_str()).is_some() {
            return Err(format!("duplicate {flag}"));
        }
        index += 1;
    }
    let required = |flag: &str| {
        values
            .get(flag)
            .copied()
            .ok_or_else(|| format!("missing {flag}"))
    };
    let uncertainty = required("--uncertainty-ns")?;
    if !uncertainty.bytes().all(|b| b.is_ascii_digit()) {
        return Err("--uncertainty-ns requires an unsigned decimal integer".into());
    }
    let time = TrustedTime {
        now_ns: parse_time(required("--now")?)?,
        uncertainty_ns: uncertainty
            .parse()
            .map_err(|_| "--uncertainty-ns exceeds its integer range")?,
    };
    let output = PathBuf::from(required("--output")?);
    let journal = PathBuf::from(required("--journal")?);
    if export {
        if words.len() != 1 {
            return Err("reliance export requires exactly one input artifact path".into());
        }
        return Ok(Command::Export {
            input: PathBuf::from(&words[0]),
            output,
            journal,
            head: parse_digest(required("--head")?)?,
            time,
        });
    }
    if !words.is_empty() {
        return Err("assure issue accepts no positional arguments".into());
    }
    let issuer = values.get("--issuer").map(|s| (*s).to_string());
    if issuer.is_some() == synthetic {
        return Err("assure issue requires exactly one of --issuer or --synthetic".into());
    }
    if let Some(name) = &issuer
        && (!is_issuer_name(name) || !registry_path(workspace, name).is_file())
    {
        return Err(format!("unknown issuer '{name}'"));
    }
    let through = values.get("--through").map(|s| (*s).to_string());
    if let Some(bound) = &through
        && (synthetic || !is_through_bound(bound))
    {
        return Err("--through requires a registered issuer and a YYYYQn quarter".into());
    }
    let recipient_role = RoleId::parse(required("--role")?).map_err(|e| e.what)?;
    let purpose = required("--purpose")?;
    if purpose.trim().is_empty()
        || purpose.len() > fmine_claim::reliance::MAX_TEXT
        || purpose.chars().any(char::is_control)
    {
        return Err("--purpose must be bounded, nonempty text without control characters".into());
    }
    Ok(Command::Issue {
        issuer,
        through,
        output,
        auth: MutationAuth {
            journal,
            key: PathBuf::from(required("--key")?),
            expected_head: parse_digest(required("--expected-head")?)?,
            time,
        },
        recipient_role,
        purpose: purpose.into(),
    })
}

fn js(text: &str) -> String {
    format!("\"{}\"", escape_json(text))
}

#[cfg(target_os = "linux")]
fn input_error(detail: impl Into<String>) -> Refusal {
    Refusal::new(RefusalCode::InputMissingOrUnreadable506, detail)
}

fn checked_revision(revision: &str) -> Result<&str, Refusal> {
    let sha = revision.strip_suffix("+dirty").unwrap_or(revision);
    if sha.len() != 40
        || !sha
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(Refusal::new(
            RefusalCode::NotObservable202,
            "build revision is unavailable; build from the versioned repository before issuing reports",
        ));
    }
    Ok(revision)
}

fn build_identity() -> Result<(String, Digest32), Refusal> {
    Ok((
        checked_revision(env!("FMINE_BUILD_REVISION"))?.into(),
        executing_image_digest()?,
    ))
}

#[cfg(target_os = "linux")]
fn executing_image_digest() -> Result<Digest32, Refusal> {
    // DESIGN: this kernel link opens the running executable's inode, including after its pathname is
    // replaced by another build. Reading current_exe() by pathname could instead fingerprint that build.
    let mut file = std::fs::File::open("/proc/self/exe")
        .map_err(|e| input_error(format!("open executing image: {e}")))?;
    let metadata = file
        .metadata()
        .map_err(|e| input_error(format!("inspect executing image: {e}")))?;
    if !metadata.is_file() || metadata.len() == 0 {
        return Err(input_error(
            "executing image is not a nonempty regular file",
        ));
    }
    // DESIGN: a debug executable can exceed the report-artifact limit. The pinned inode's length is
    // the image read budget; fixed-size chunks keep memory independent of debug-symbol size.
    let digest = digest_image_reader(&mut file, metadata.len())?;
    if file
        .metadata()
        .map_err(|e| input_error(format!("reinspect executing image: {e}")))?
        .len()
        != metadata.len()
    {
        return Err(input_error(
            "executing image changed length while fingerprinting",
        ));
    }
    Ok(digest)
}

#[cfg(target_os = "linux")]
fn digest_image_reader(
    reader: impl std::io::Read,
    expected_bytes: u64,
) -> Result<Digest32, Refusal> {
    use fmine_core::hash::Blake3;
    use std::io::{ErrorKind, Read as _};

    let limit = expected_bytes
        .checked_add(1)
        .filter(|_| expected_bytes != 0)
        .ok_or_else(|| input_error("executing image has no supported byte budget"))?;
    let mut reader = reader.take(limit);
    let mut remaining = expected_bytes;
    let mut hasher = Blake3::new();
    let mut buffer = [0_u8; 16 * 1024];
    loop {
        let count = match reader.read(&mut buffer) {
            Ok(0) => break,
            Ok(count) => count,
            Err(error) if error.kind() == ErrorKind::Interrupted => continue,
            Err(error) => return Err(input_error(format!("read executing image: {error}"))),
        };
        remaining = remaining
            .checked_sub(count as u64)
            .ok_or_else(|| input_error("executing image changed length while fingerprinting"))?;
        hasher.update(&buffer[..count]);
    }
    if remaining != 0 {
        return Err(input_error(
            "executing image changed length while fingerprinting",
        ));
    }
    Ok(hasher.finalize())
}

#[cfg(not(target_os = "linux"))]
fn executing_image_digest() -> Result<Digest32, Refusal> {
    Err(Refusal::new(
        RefusalCode::NotObservable202,
        "this platform has no supported executing-image identity; report issuance requires a verified running binary",
    ))
}

fn execute_payload(command: &Command, workspace: &Path) -> Result<String, Refusal> {
    match command {
        Command::Issue {
            issuer,
            through,
            output,
            auth,
            recipient_role,
            purpose,
        } => {
            let key = ReleaseKey::read(&resolve(workspace, &auth.key))?;
            let prepared =
                reliance_binding::prepare(workspace, issuer.as_deref(), through.as_deref())?;
            let (code_revision, binary_digest) = build_identity()?;
            let bytes = format!("{}\n", prepared.envelope.to_json()).into_bytes();
            let issuance = IssuanceRecord {
                output_digest: blake3_hash(&bytes),
                output_claims: prepared.claim_ids,
                anchor: prepared.anchor,
                code_revision,
                binary_digest,
                input_manifest_root: prepared.input_manifest_root,
                lane: match prepared.lane {
                    Lane::Public => IssuanceLane::Public,
                    Lane::Synthetic => IssuanceLane::Synthetic,
                },
                confidentiality: prepared.taint,
                public_taint: prepared.lane == Lane::Public,
                recipient_role: recipient_role.clone(),
                purpose: purpose.clone(),
                issued_at_ns: auth.time.now_ns,
            };
            let digest = issuance.output_digest;
            let issuance_digest = issuance.digest()?;
            let result = format!(
                "{{\"schema\":\"fmine.reliance.issue.v1\",\"output_digest\":{},\"issuance_digest\":{},\"issuance_id\":{},\"output_claims\":[{}],\"anchor_id\":{},\"state_root\":{},\"input_manifest_root\":{},\"code_revision\":{},\"code_revision_evidence\":\"build_time\",\"binary_digest\":{},\"binary_digest_algorithm\":\"blake3\",\"lane\":{},\"public_taint\":{},\"recipient_role\":{},\"purpose\":{},\"issued_at_ns\":{}",
                js(&digest.to_hex()),
                js(&issuance_digest.to_hex()),
                js(&issuance_digest.to_hex()),
                issuance
                    .output_claims
                    .iter()
                    .map(|id| js(&id.to_string()))
                    .collect::<Vec<_>>()
                    .join(","),
                js(&issuance.anchor.anchor_id().to_string()),
                js(&issuance.anchor.state_root.to_hex()),
                js(&issuance.input_manifest_root.to_hex()),
                js(&issuance.code_revision),
                js(&binary_digest.to_hex()),
                js(issuance.lane.as_str()),
                issuance.public_taint,
                js(&recipient_role.to_string()),
                js(purpose),
                auth.time.now_ns,
            );
            // DESIGN: verification does not block status readers. Compare the expected head only after
            // projection, then retain the exclusive lock through artifact creation and issuance commit.
            let mut journal =
                Journal::open_write(&resolve(workspace, &auth.journal), auth.expected_head)?;
            journal.issue_bytes(
                &resolve(workspace, output),
                &bytes,
                issuance,
                &key,
                auth.time,
            )?;
            Ok(format!(
                "{result},\"previous_head\":{},\"journal_head\":{},\"artifact_bytes\":{}}}",
                js(&auth.expected_head.to_hex()),
                js(&journal.head().to_hex()),
                bytes.len()
            ))
        }
        Command::Export {
            input,
            output,
            journal,
            head,
            time,
        } => {
            let journal = Journal::open_read(&resolve(workspace, journal), Some(*head))?;
            let digest = journal.export_current(
                &resolve(workspace, input),
                &resolve(workspace, output),
                *time,
            )?;
            Ok(format!(
                "{{\"schema\":\"fmine.reliance.export.v1\",\"output_digest\":{},\"journal_head\":{},\"current_reuse_allowed\":true,\"new_recipient_registered\":false}}",
                js(&digest.to_hex()),
                js(&journal.head().to_hex())
            ))
        }
    }
}

pub fn execute(command: &Command, workspace: &Path) -> Envelope {
    let actions = vec![NextAction::new(
        "fmine robot-docs",
        "issued-artifact commands and trust inputs",
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
mod tests {
    use super::*;

    #[test]
    fn issuance_requires_a_complete_build_revision_without_inventing_metadata() {
        let clean = "1".repeat(40);
        assert_eq!(checked_revision(&clean).expect("revision"), clean);
        let dirty = format!("{clean}+dirty");
        assert_eq!(checked_revision(&dirty).expect("dirty revision"), dirty);
        for invalid in [
            "unknown",
            "",
            "1234567",
            "unversioned+dirty",
            "ZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZ",
        ] {
            assert_eq!(
                checked_revision(invalid)
                    .expect_err("unknown provenance")
                    .code,
                "FMINE-REF-202"
            );
        }
    }

    #[cfg(target_os = "linux")]
    mod image_identity {
        use super::*;
        use std::io::{self, Cursor, Read};

        struct Chunked<R> {
            inner: R,
            interrupt_next: bool,
            total: u64,
            largest_request: usize,
        }

        impl<R: Read> Read for Chunked<R> {
            fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
                self.largest_request = self.largest_request.max(buffer.len());
                if self.interrupt_next {
                    self.interrupt_next = false;
                    return Err(io::ErrorKind::Interrupted.into());
                }
                let count = buffer.len().min(4093);
                let read = self.inner.read(&mut buffer[..count])?;
                self.total += read as u64;
                Ok(read)
            }
        }

        fn chunked<R: Read>(inner: R) -> Chunked<R> {
            Chunked {
                inner,
                interrupt_next: true,
                total: 0,
                largest_request: 0,
            }
        }

        #[test]
        fn streaming_image_identity_matches_exact_bytes_across_read_boundaries() {
            for length in [
                1, 63, 64, 65, 1023, 1024, 1025, 16_383, 16_384, 16_385, 32_769,
            ] {
                let bytes: Vec<u8> = (0..length).map(|i| (i % 251) as u8).collect();
                let mut reader = chunked(Cursor::new(&bytes));
                let got = digest_image_reader(&mut reader, bytes.len() as u64)
                    .expect("complete interrupted/short reads");
                assert_eq!(got, blake3_hash(&bytes), "length {length}");
                assert_eq!(reader.total, length as u64);
                assert!(reader.largest_request <= 16 * 1024);
            }
        }

        #[test]
        fn executable_identity_is_not_limited_by_report_artifact_size() {
            let length = fmine_io::reliance::MAX_ARTIFACT_BYTES + 1;
            let mut reader = chunked(io::repeat(0xa5).take(length));
            digest_image_reader(&mut reader, length)
                .expect("running images may exceed the independent report-artifact limit");
            assert_eq!(reader.total, length);
            assert!(reader.largest_request <= 16 * 1024);
        }

        #[test]
        fn image_identity_refuses_length_changes_and_read_failures() {
            for expected in [0, 2, 4, u64::MAX] {
                let mut reader = Cursor::new(b"abc");
                let error = digest_image_reader(&mut reader, expected)
                    .expect_err("length must be exactly supported");
                assert_eq!(error.code, "FMINE-REF-506");
                assert!(reader.position() <= expected.saturating_add(1));
            }

            struct ReadFailure;
            impl Read for ReadFailure {
                fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
                    Err(io::Error::other("injected image read failure"))
                }
            }
            // A failure after all declared bytes still must not be accepted as end-of-file.
            let error = digest_image_reader(Cursor::new(b"abc").chain(ReadFailure), 3)
                .expect_err("read errors do not prove EOF");
            assert_eq!(error.code, "FMINE-REF-506");
            assert!(error.what.contains("injected image read failure"));
        }
    }
}
