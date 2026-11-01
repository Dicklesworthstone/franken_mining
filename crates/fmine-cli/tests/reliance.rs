#![cfg(test)]
#![cfg(unix)]

//! BD-347: real-binary lifecycle and filesystem regressions on explicitly synthetic reports and keys.

use std::io::Write as _;
use std::os::unix::fs::{OpenOptionsExt as _, PermissionsExt as _, symlink};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use fmine_claim::reliance::IssuanceRecord;
use fmine_core::hash::{Digest32, blake3_hash};
use fmine_io::reliance::Journal;

static NEXT: AtomicU64 = AtomicU64::new(0);
const REVISION: &str = "1111111111111111111111111111111111111111";

struct Fixture {
    root: PathBuf,
    journal: PathBuf,
    key: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "fmine-reliance-cli-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&root).expect("new isolated test directory");
        let key = root.join("release.key");
        write_key(&key, 0x42);
        let fixture = Self {
            journal: root.join("issuance.log"),
            root,
            key,
        };
        fixture.run(
            &["reliance", "init", "--release-key", path(&fixture.key)],
            0,
        );
        fixture
    }

    fn command(&self, args: &[&str]) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_fmine"));
        command.args(args).args([
            "--journal",
            path(&self.journal),
            "--workspace",
            path(&self.root),
        ]);
        command
    }

    fn run(&self, args: &[&str], expected: i32) -> String {
        let output = self
            .command(args)
            .output()
            .expect("execute actual fmine binary");
        let text = String::from_utf8(output.stdout).expect("UTF-8 envelope");
        assert_eq!(
            output.status.code(),
            Some(expected),
            "{args:?}: {text}; stderr={}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(text.lines().count(), 1, "one NDJSON envelope: {text}");
        for flag in [
            "reserve_disclosure_claim",
            "execution_authorized",
            "geotechnical_signoff",
            "regulatory_compliance_claim",
        ] {
            assert!(text.contains(&format!("\"{flag}\":false")), "{text}");
        }
        text
    }

    fn head(&self) -> String {
        Journal::open_read(&self.journal, None)
            .expect("valid journal")
            .head()
            .to_hex()
    }

    fn mutate(&self, args: &[&str], expected: i32) -> String {
        let head = self.head();
        let mut args = args.to_vec();
        args.extend([
            "--release-key",
            path(&self.key),
            "--head",
            &head,
            "--trust-host-clock",
        ]);
        self.run(&args, expected)
    }

    fn trusted(&self, args: &[&str], expected: i32) -> String {
        let head = self.head();
        let mut args = args.to_vec();
        args.extend(["--head", &head, "--trust-host-clock"]);
        self.run(&args, expected)
    }

    fn issue(&self, name: &str, recipient: &str, format: &str) -> (PathBuf, IssuanceRecord) {
        let artifact = self.root.join(name);
        self.mutate(
            &[
                "reliance",
                "issue",
                "--synthetic",
                "--output",
                path(&artifact),
                "--recipient-role",
                recipient,
                "--purpose",
                "review",
                "--code-revision",
                REVISION,
                "--artifact-format",
                format,
            ],
            0,
        );
        let journal = Journal::open_read(&self.journal, None).expect("issued journal");
        let issuance = (*journal.ledger().issuances().last().expect("issuance")).clone();
        (artifact, issuance)
    }
}

fn path(path: &Path) -> &str {
    path.to_str().expect("UTF-8 test path")
}

fn write_key(path: &Path, byte: u8) {
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
        .expect("private synthetic key");
    file.write_all(&[byte; 32]).expect("key bytes");
}

#[test]
fn issued_reports_retain_exact_bytes_and_known_copies_after_withdrawal() {
    let f = Fixture::new();
    let (artifact, issued) = f.issue("first.ndjson", "sponsor", "json");
    let bytes = std::fs::read(&artifact).expect("issued bytes");
    let ordinary = Command::new(env!("CARGO_BIN_EXE_fmine"))
        .args(["assure", "public", "--synthetic"])
        .output()
        .expect("ordinary report");
    assert!(ordinary.status.success());
    assert_eq!(
        bytes, ordinary.stdout,
        "issuance preserves ordinary assurance bytes and LF"
    );
    assert_eq!(issued.output_digest, blake3_hash(&bytes));
    assert!(!issued.output_claims.is_empty());
    assert_eq!(
        std::fs::metadata(&artifact)
            .expect("artifact mode")
            .permissions()
            .mode()
            & 0o077,
        0
    );
    let (second_path, second) = f.issue("second.ndjson", "reviewer-pool-1", "json");
    assert_eq!(issued.output_digest, second.output_digest);
    assert_eq!(
        bytes,
        std::fs::read(&second_path).expect("second exact artifact")
    );
    let digest = issued.output_digest.to_hex();
    let claim = issued.output_claims[0].to_string();

    let unknown = f.run(&["reliance", "check", "--digest", &digest], 0);
    assert!(unknown.contains("\"status\":\"unknown\""), "{unknown}");
    assert!(unknown.contains("\"current_reuse_allowed\":false"));
    let current = f.trusted(
        &[
            "reliance",
            "check",
            "--digest",
            &digest,
            "--require-current",
        ],
        0,
    );
    assert!(
        current.contains("\"current_reuse_allowed\":true"),
        "{current}"
    );
    let copy = f.root.join("retrieved.ndjson");
    f.trusted(
        &[
            "reliance",
            "reuse",
            "--artifact",
            path(&artifact),
            "--output",
            path(&copy),
        ],
        0,
    );
    assert_eq!(bytes, std::fs::read(&copy).expect("exact current copy"));
    assert_eq!(
        std::fs::metadata(copy)
            .expect("copy mode")
            .permissions()
            .mode()
            & 0o077,
        0
    );

    f.mutate(
        &[
            "reliance",
            "withdraw",
            "--digest",
            &digest,
            "--claims",
            &claim,
            "--reason",
            "seeded-code-defect",
            "--defect",
            "code-defect",
        ],
        0,
    );
    let withdrawn = f.trusted(
        &[
            "reliance",
            "check",
            "--digest",
            &digest,
            "--require-current",
        ],
        3,
    );
    assert!(
        withdrawn.contains("withdrawn") && withdrawn.contains("FMINE-REF-202"),
        "{withdrawn}"
    );
    let historical = f.run(
        &[
            "reliance",
            "check",
            "--digest",
            &digest,
            "--artifact",
            path(&artifact),
        ],
        0,
    );
    assert!(
        historical.contains("\"artifact_bytes_verified\":true"),
        "{historical}"
    );
    assert!(historical.contains("\"current_reuse_allowed\":false"));
    let blocked = f.root.join("blocked.ndjson");
    f.trusted(
        &[
            "reliance",
            "reuse",
            "--artifact",
            path(&artifact),
            "--output",
            path(&blocked),
        ],
        3,
    );
    assert!(!blocked.exists());
    assert_eq!(
        bytes,
        std::fs::read(&artifact).expect("withdrawal preserves original bytes")
    );

    for selector in [["--code-revision", REVISION], ["--claim", &claim]] {
        let recalled = f.run(&["reliance", "recall", selector[0], selector[1]], 0);
        assert!(
            recalled.contains("sponsor") && recalled.contains("reviewer-pool-1"),
            "{recalled}"
        );
    }
    let journal = Journal::open_read(&f.journal, None).expect("history");
    assert_eq!(journal.ledger().copies(issued.output_digest).len(), 2);
    assert_eq!(
        journal.ledger().copies(issued.output_digest)[0].anchor,
        issued.anchor
    );
}

#[test]
fn stale_pins_bad_authority_tampered_artifacts_and_withdrawn_replacements_fail_closed() {
    let f = Fixture::new();
    let stale_head = f.head();
    let (artifact, issued) = f.issue("original.ndjson", "sponsor", "json");
    let digest = issued.output_digest.to_hex();
    let before = std::fs::read(&f.journal).expect("before rejected writes");
    f.run(
        &[
            "reliance",
            "refresh",
            "--release-key",
            path(&f.key),
            "--head",
            &stale_head,
            "--trust-host-clock",
        ],
        6,
    );
    let bad_key = f.root.join("wrong.key");
    write_key(&bad_key, 0x43);
    f.run(
        &[
            "reliance",
            "refresh",
            "--release-key",
            path(&bad_key),
            "--head",
            &f.head(),
            "--trust-host-clock",
        ],
        7,
    );
    assert_eq!(
        before,
        std::fs::read(&f.journal).expect("failed writes preserve every byte")
    );
    let mismatched = f.run(
        &[
            "reliance",
            "check",
            "--digest",
            &digest,
            "--head",
            &stale_head,
            "--trust-host-clock",
        ],
        0,
    );
    assert!(
        mismatched.contains("trusted_head_mismatch")
            && mismatched.contains("\"current_reuse_allowed\":false")
    );
    let original = std::fs::read(&artifact).expect("original artifact");
    f.mutate(
        &[
            "reliance",
            "issue",
            "--synthetic",
            "--output",
            path(&artifact),
            "--recipient-role",
            "sponsor",
            "--purpose",
            "review",
            "--code-revision",
            REVISION,
        ],
        5,
    );
    assert_eq!(
        original,
        std::fs::read(&artifact).expect("existing artifact was not overwritten")
    );
    assert_eq!(
        before,
        std::fs::read(&f.journal).expect("failed reissue never appends")
    );

    let (replacement, new_issue) = f.issue("replacement.md", "sponsor", "markdown");
    assert_ne!(issued.output_digest, new_issue.output_digest);
    let new_digest = new_issue.output_digest.to_hex();
    f.mutate(
        &[
            "reliance",
            "supersede",
            "--digest",
            &digest,
            "--replacement",
            &new_digest,
        ],
        0,
    );
    let superseded = f.trusted(
        &[
            "reliance",
            "check",
            "--digest",
            &digest,
            "--require-current",
        ],
        3,
    );
    assert!(
        superseded.contains("superseded") && superseded.contains(&new_digest),
        "{superseded}"
    );
    f.mutate(
        &[
            "reliance",
            "withdraw",
            "--digest",
            &new_digest,
            "--all-claims",
            "--reason",
            "replacement-also-defective",
            "--defect",
            "scope-error",
        ],
        0,
    );
    f.trusted(
        &[
            "reliance",
            "check",
            "--digest",
            &new_digest,
            "--require-current",
        ],
        3,
    );
    f.run(
        &[
            "reliance",
            "check",
            "--digest",
            &new_digest,
            "--artifact",
            path(&replacement),
        ],
        0,
    );
    let tampered = f.root.join("tampered.ndjson");
    std::fs::write(&tampered, b"not the issued bytes").expect("separate tampered artifact");
    f.run(
        &[
            "reliance",
            "check",
            "--digest",
            &digest,
            "--artifact",
            path(&tampered),
        ],
        8,
    );
    let absent = Digest32([0x7f; 32]).to_hex();
    let unknown = f.trusted(&["reliance", "check", "--digest", &absent], 0);
    assert!(unknown.contains("unissued") && unknown.contains("\"current_reuse_allowed\":false"));
}

#[test]
fn special_files_are_refused_without_blocking_the_journal_or_following_links() {
    let f = Fixture::new();
    let (artifact, issued) = f.issue("original.ndjson", "sponsor", "json");
    let digest = issued.output_digest.to_hex();
    let link = f.root.join("artifact-link");
    symlink(&artifact, &link).expect("synthetic artifact link");
    f.run(
        &[
            "reliance",
            "check",
            "--digest",
            &digest,
            "--artifact",
            path(&link),
        ],
        5,
    );
    let fifo = f.root.join("artifact-fifo");
    assert!(
        Command::new("mkfifo")
            .arg(&fifo)
            .status()
            .expect("create FIFO fixture")
            .success()
    );
    let mut child = f
        .command(&[
            "reliance",
            "check",
            "--digest",
            &digest,
            "--artifact",
            path(&fifo),
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("check FIFO in child");
    let started = Instant::now();
    while child.try_wait().expect("child status").is_none() {
        if started.elapsed() > Duration::from_secs(5) {
            child.kill().expect("stop only the blocked test child");
            let _ = child.wait();
            panic!("artifact type must be refused before opening a FIFO under the journal lock");
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    let result = child.wait_with_output().expect("FIFO refusal");
    assert_eq!(
        result.status.code(),
        Some(5),
        "{}",
        String::from_utf8_lossy(&result.stdout)
    );
    assert!(String::from_utf8_lossy(&result.stdout).contains("FMINE-REF-506"));
    f.mutate(
        &[
            "reliance",
            "withdraw",
            "--digest",
            &digest,
            "--all-claims",
            "--reason",
            "lock-remains-available",
            "--defect",
            "code-defect",
        ],
        0,
    );
}
