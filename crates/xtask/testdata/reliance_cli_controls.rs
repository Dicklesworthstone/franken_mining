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
    assert!(artifact.starts_with("# Public assurance — synthetic\n"));
    let preview_args =
        ["assure", "public", "--synthetic", "--format", "markdown"].map(str::to_string);
    let (preview, exit) = crate::run(&preview_args, &root);
    assert_eq!(exit, 0);
    assert_eq!(artifact, format!("{preview}\n"));
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
    assert!(first.contains(&blake3_hash(b"synthetic executable image for binding test").to_hex()));
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
    let replacement = blake3_hash(&std::fs::read(root.join("replacement.md")).unwrap()).to_hex();
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
    let (payload, refusal) = inspect_check(&command, &root, &|| Ok(START + 3_000_000_000)).unwrap();
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
