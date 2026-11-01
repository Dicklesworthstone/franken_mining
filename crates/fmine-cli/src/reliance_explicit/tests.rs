#![cfg(unix)]

use super::*;
use fmine_claim::Confidentiality;
use fmine_claim::reliance::IssuanceLane;
use fmine_core::LedgerAnchor;
use std::fs::OpenOptions;
use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);
const START: &str = "2030-01-01T00:00:00Z";

fn args(words: &[&str]) -> Vec<String> {
    words.iter().map(|s| (*s).into()).collect()
}

fn at(seconds: i64) -> TrustedTime {
    TrustedTime {
        now_ns: parse_time(START).expect("time") + seconds * 1_000_000_000,
        uncertainty_ns: 0,
    }
}

fn write_new(path: &Path, bytes: &[u8]) {
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
        .expect("new private file")
        .write_all(bytes)
        .expect("write file");
}

fn workspace() -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "fmine-reliance-explicit-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir(&root).expect("test workspace");
    write_new(&root.join("owner.key"), &[23; 32]);
    let command = parse(&args(&[
        "reliance",
        "init",
        "--journal",
        "journal",
        "--key",
        "owner.key",
        "--now",
        START,
        "--uncertainty-ns",
        "0",
        "--max-age-ns",
        "10000000000",
        "--max-skew-ns",
        "0",
    ]))
    .expect("parse init");
    let init = execute(&command, &root);
    assert_eq!(init.exit_code, 0, "{}", init.to_json());
    root
}

fn head(root: &Path) -> Digest32 {
    Journal::open_read(&root.join("journal"), None)
        .expect("read journal")
        .head()
}

fn claim_id(seed: u8) -> ObjectId {
    ObjectId::from_canonical_bytes(PrefixKind::Claim, &[seed])
}

fn issue(root: &Path, bytes: &[u8], seconds: i64, role: RoleId, revision: &str) -> Digest32 {
    let key = ReleaseKey::read(&root.join("owner.key")).expect("key");
    let mut journal = Journal::open_write(&root.join("journal"), head(root)).expect("writer");
    let digest = blake3_hash(bytes);
    journal
        .append(
            Entry::Issued(IssuanceRecord {
                output_digest: digest,
                output_claims: vec![claim_id(1)],
                anchor: LedgerAnchor {
                    format_version: 1,
                    cell_id: "CELL-SYNTHETIC".into(),
                    ledger_epoch: 1,
                    commit_sequence: 1,
                    adapter_registry_epoch: 1,
                    schema_epoch: 1,
                    policy_epoch: 1,
                    privacy_epoch: 1,
                    residency_epoch: 1,
                    state_root: blake3_hash(b"synthetic evidence"),
                },
                code_revision: revision.into(),
                binary_digest: blake3_hash(b"synthetic test binary"),
                input_manifest_root: blake3_hash(b"synthetic manifest"),
                lane: IssuanceLane::Synthetic,
                confidentiality: Confidentiality::public(),
                public_taint: false,
                recipient_role: role,
                purpose: "synthetic test".into(),
                issued_at_ns: at(seconds).now_ns,
            }),
            &key,
            at(seconds),
        )
        .expect("issued fixture");
    digest
}

fn check(
    root: &Path,
    digest: Digest32,
    pin: Option<Digest32>,
    time: Option<TrustedTime>,
    as_of: Option<i64>,
) -> Envelope {
    execute(
        &Command::Check {
            journal: "journal".into(),
            target: Target::Digest(digest),
            trust: TrustContext {
                pinned_head: pin,
                time,
            },
            as_of_ns: as_of,
        },
        root,
    )
}

#[test]
fn parsed_trust_inputs_never_default_or_accept_duplicates() {
    let digest = blake3_hash(b"test").to_hex();
    let base = args(&["reliance", "check", &digest, "--journal", "journal"]);
    let command = parse(&base).expect("untrusted read is valid");
    let Command::Check { trust, target, .. } = command else {
        panic!("check")
    };
    assert_eq!(trust, TrustContext::default());
    assert_eq!(target, Target::Digest(blake3_hash(b"test")));
    for additions in [
        args(&["--now", START]),
        args(&["--uncertainty-ns", "0"]),
        args(&["--head", &digest, "--head", &digest]),
        args(&["--key", "owner.key"]),
        args(&["--now", "2030-01-01", "--uncertainty-ns", "0"]),
    ] {
        let mut invalid = base.clone();
        invalid.extend(additions);
        assert!(parse(&invalid).is_err(), "{invalid:?}");
    }
    assert!(parse_time("3000-01-01T00:00:00Z").is_err());
    assert!(parse(&args(&["recall", "--journal", "journal"])).is_err());
    assert!(
        parse(&args(&[
            "recall",
            "--journal",
            "journal",
            "--claim",
            "anc_00000000000000000000000000000000"
        ]))
        .is_err()
    );
}

#[test]
fn initialization_validates_policy_and_never_replaces_a_journal() {
    for (age, skew, uncertainty) in [("0", "0", "0"), ("1", "2", "0"), ("2", "1", "2")] {
        assert!(
            parse(&args(&[
                "reliance",
                "init",
                "--journal",
                "journal",
                "--key",
                "owner.key",
                "--now",
                START,
                "--uncertainty-ns",
                uncertainty,
                "--max-age-ns",
                age,
                "--max-skew-ns",
                skew,
            ]))
            .is_err()
        );
    }
    let root = workspace();
    let before = std::fs::read(root.join("journal")).expect("journal bytes");
    let again = execute(
        &Command::Init {
            journal: "journal".into(),
            key: "owner.key".into(),
            time: at(1),
            max_age_ns: 10_000_000_000,
            max_skew_ns: 0,
        },
        &root,
    );
    assert_ne!(again.exit_code, 0);
    assert_eq!(
        std::fs::read(root.join("journal")).expect("journal bytes"),
        before
    );
}

#[test]
fn status_read_preserves_unknown_and_historical_boundaries() {
    let root = workspace();
    let digest = issue(&root, b"synthetic report", 1, RoleId::Sponsor, "revision-a");
    let pin = head(&root);
    let cases = [
        (None, None, "trusted_time_unavailable"),
        (None, Some(at(1)), "trusted_pin_unavailable"),
        (
            Some(blake3_hash(b"old pin")),
            Some(at(1)),
            "trusted_head_mismatch",
        ),
        (Some(pin), Some(at(12)), "stale_status_cut"),
        (
            Some(pin),
            Some(at(0)),
            "future_status_cut_or_clock_rollback",
        ),
    ];
    for (pin, now, reason) in cases {
        let result = check(&root, digest, pin, now, None);
        assert_eq!(result.exit_code, 0);
        assert!(
            result
                .payload_json
                .contains("\"reliance_status\":\"unknown\"")
        );
        assert!(
            result
                .payload_json
                .contains(&format!("\"reason\":\"{reason}\""))
        );
        assert!(
            result
                .payload_json
                .contains("\"current_reuse_allowed\":false")
        );
    }
    let current = check(&root, digest, Some(pin), Some(at(2)), None);
    assert!(
        current
            .payload_json
            .contains("\"current_reuse_allowed\":true")
    );
    assert_eq!(
        current.to_json(),
        check(&root, digest, Some(pin), Some(at(2)), None).to_json()
    );
    let historical = check(&root, digest, Some(pin), Some(at(2)), Some(at(1).now_ns));
    assert!(
        historical
            .payload_json
            .contains("\"reliance_status\":\"current\"")
    );
    assert!(
        historical
            .payload_json
            .contains("\"current_reuse_allowed\":false")
    );
    let unissued = check(
        &root,
        blake3_hash(b"unissued"),
        Some(pin),
        Some(at(2)),
        None,
    );
    assert!(unissued.payload_json.contains("\"reason\":\"unissued\""));
}

#[test]
fn file_inspection_never_emits_bytes_or_guesses_a_missing_file() {
    let root = workspace();
    let bytes = b"synthetic test report contents retained only in the file";
    let digest = issue(&root, bytes, 1, RoleId::Sponsor, "revision-a");
    write_new(&root.join("report"), bytes);
    let mut command = Command::Check {
        journal: "journal".into(),
        target: Target::File("report".into()),
        trust: TrustContext {
            pinned_head: Some(head(&root)),
            time: Some(at(2)),
        },
        as_of_ns: None,
    };
    let result = execute(&command, &root);
    assert!(result.payload_json.contains("\"artifact_verified\":true"));
    assert!(result.payload_json.contains(&digest.to_hex()));
    assert!(
        !result
            .to_json()
            .contains(std::str::from_utf8(bytes).expect("UTF-8"))
    );
    if let Command::Check { target, .. } = &mut command {
        *target = Target::File("missing".into());
    }
    assert_eq!(
        execute(&command, &root).refusal.expect("missing file").code,
        "FMINE-REF-506"
    );
    let explicit = parse(&args(&[
        "reliance",
        "check",
        &format!("./{}", digest.to_hex()),
        "--journal",
        "journal",
    ]))
    .expect("file target");
    assert!(matches!(
        explicit,
        Command::Check {
            target: Target::File(_),
            ..
        }
    ));
}

#[test]
fn unauthorized_or_stale_mutation_leaves_journal_unchanged() {
    let root = workspace();
    let old_head = head(&root);
    let digest = issue(&root, b"synthetic report", 1, RoleId::Sponsor, "revision-a");
    write_new(&root.join("wrong.key"), &[24; 32]);
    let before = std::fs::read(root.join("journal")).expect("journal bytes");
    for (key, pin, expected_code) in [
        ("wrong.key", head(&root), "FMINE-REF-304"),
        ("owner.key", old_head, "FMINE-REF-502"),
    ] {
        let result = execute(
            &Command::Withdraw {
                auth: MutationAuth {
                    journal: "journal".into(),
                    key: key.into(),
                    expected_head: pin,
                    time: at(2),
                },
                output: digest,
                reason: "synthetic code defect".into(),
                defect_class: DefectClass::CodeDefect,
                affected_claims: vec![claim_id(1)],
            },
            &root,
        );
        assert_eq!(
            result.refusal.expect("refused mutation").code,
            expected_code
        );
        assert_eq!(
            std::fs::read(root.join("journal")).expect("journal bytes"),
            before
        );
    }
}

#[test]
fn recall_retains_known_copies_without_asserting_current_status() {
    let root = workspace();
    let output = issue(&root, b"synthetic report", 1, RoleId::Sponsor, "revision-a");
    issue(
        &root,
        b"synthetic report",
        2,
        RoleId::ReviewerPool(1),
        "revision-b",
    );
    issue(
        &root,
        b"another synthetic report",
        3,
        RoleId::Sponsor,
        "revision-c",
    );
    let command = parse(&args(&[
        "recall",
        "--journal",
        "journal",
        "--revision",
        "revision-a",
    ]))
    .expect("recall");
    let result = execute(&command, &root);
    assert!(result.payload_json.contains("\"copies\":1"));
    assert!(result.payload_json.contains(&output.to_hex()));
    assert!(!result.payload_json.contains("revision-b"));
    assert!(
        result
            .payload_json
            .contains("\"current_status_assessed\":false")
    );
    assert!(result.payload_json.contains("\"head_pin_matched\":false"));
    let by_claim = parse(&args(&[
        "recall",
        "--journal",
        "journal",
        "--claim",
        &claim_id(1).to_string(),
    ]))
    .expect("recall claim");
    assert!(
        execute(&by_claim, &root)
            .payload_json
            .contains("\"copies\":3")
    );
}
