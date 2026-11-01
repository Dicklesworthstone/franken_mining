//! DESIGN (BD-347): exercise the real binary from issuance to withdrawal and attempted reuse.
//! The workbooks and owner key are invented; these are known-answer protocol tests, not live issuer proof.

#![cfg(target_os = "linux")]
#![allow(clippy::expect_used)] // Test fixtures fail with context; production keeps the workspace ban.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use fmine_claim::reliance::{IssuanceLane, IssuanceRecord, RoleId};
use fmine_core::hash::{Digest32, blake3_hash};
use fmine_io::reliance::Journal;

const CREATED: &str = "2030-01-01T00:00:00Z";
const ISSUED: &str = "2030-01-01T00:00:01Z";
const CHECKED: &str = "2030-01-01T00:00:02Z";
const WITHDRAWN: &str = "2030-01-01T00:00:03Z";
const AFTER: &str = "2030-01-01T00:00:04Z";

struct Workspace(PathBuf);

impl Workspace {
    fn new() -> Self {
        Self::with_policy(CREATED, 60_000_000_000, 2)
    }

    fn with_live_clock() -> (Self, String) {
        let seconds = i64::try_from(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("test host clock follows Unix epoch")
                .as_secs(),
        )
        .expect("test time fits nanosecond calendar");
        let created = fmine_calib::record::format_utc(seconds);
        // The explicit fixture uses the actual current second. Host-clock commands then use their own
        // current instant under this deliberately generous synthetic policy, without backdating.
        let workspace = Self::with_policy(&created, 600_000_000_000, 1_000_000_000);
        (workspace, created)
    }

    fn with_policy(created: &str, max_age_ns: u64, max_skew_ns: u64) -> Self {
        static SERIAL: AtomicU64 = AtomicU64::new(0);
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("test host clock follows the Unix epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "fmine-reliance-cli-{}-{}-{nonce}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed),
        ));
        fs::create_dir(&path).expect("create isolated lifecycle workspace");
        let out = Self(path);
        out.key("owner.key", 7);
        out.key("wrong.key", 8);
        out.run(
            &[
                "reliance",
                "init",
                "--journal",
                "reliance.log",
                "--key",
                "owner.key",
                "--now",
                created,
                "--uncertainty-ns",
                "1",
                "--max-age-ns",
                &max_age_ns.to_string(),
                "--max-skew-ns",
                &max_skew_ns.to_string(),
            ],
            0,
        );
        out
    }

    fn path(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }

    fn key(&self, name: &str, byte: u8) {
        let path = self.path(name);
        fs::write(&path, [byte; 32]).expect("write synthetic release key");
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))
            .expect("make synthetic release key private");
    }

    fn run_text(&self, args: &[&str], expected: i32) -> String {
        let output = Command::new(env!("CARGO_BIN_EXE_fmine"))
            .current_dir(&self.0)
            .args(args)
            .arg("--workspace")
            .arg(&self.0)
            .output()
            .expect("execute real fmine binary");
        let text = String::from_utf8(output.stdout).expect("CLI envelope is UTF-8");
        assert_eq!(
            output.status.code(),
            Some(expected),
            "{args:?}\nstdout: {text}\nstderr: {}",
            String::from_utf8_lossy(&output.stderr),
        );
        text
    }

    fn run(&self, args: &[&str], expected: i32) -> String {
        let text = self.run_text(args, expected);
        assert_eq!(text.lines().count(), 1, "one envelope: {text}");
        assert!(
            text.starts_with("{\"schema_version\":\"fmine.env.v1\","),
            "{text}"
        );
        assert!(
            text.contains(&format!("\"exit_code\":{expected},")),
            "{text}"
        );
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

    fn head(&self) -> Digest32 {
        Journal::open_read(&self.path("reliance.log"), None)
            .expect("read lifecycle journal head")
            .head()
    }

    fn records(&self) -> Vec<IssuanceRecord> {
        Journal::open_read(&self.path("reliance.log"), None)
            .expect("read lifecycle issuance records")
            .ledger()
            .issuances()
            .into_iter()
            .cloned()
            .collect()
    }

    fn issue(&self, output: &str, role: &str, time: &str, key: &str, expected: i32) -> String {
        self.run(
            &[
                "assure",
                "issue",
                "--synthetic",
                "--output",
                output,
                "--journal",
                "reliance.log",
                "--expected-head",
                &self.head().to_hex(),
                "--key",
                key,
                "--role",
                role,
                "--purpose",
                "invented review fixture",
                "--now",
                time,
                "--uncertainty-ns",
                "1",
            ],
            expected,
        )
    }

    fn issue_registered(
        &self,
        output: &str,
        through: Option<&str>,
        time: &str,
        expected: i32,
    ) -> String {
        let head = self.head().to_hex();
        let mut args = vec![
            "assure",
            "issue",
            "--issuer",
            "synthetic",
            "--output",
            output,
            "--journal",
            "reliance.log",
            "--expected-head",
            &head,
            "--key",
            "owner.key",
            "--role",
            "sponsor",
            "--purpose",
            "registered invented workbooks",
            "--now",
            time,
            "--uncertainty-ns",
            "1",
        ];
        if let Some(through) = through {
            args.extend(["--through", through]);
        }
        self.run(&args, expected)
    }

    fn check(&self, target: &str, now: &str, extra: &[&str]) -> String {
        let head = self.head().to_hex();
        let mut args = vec![
            "reliance",
            "check",
            target,
            "--journal",
            "reliance.log",
            "--head",
            &head,
            "--now",
            now,
            "--uncertainty-ns",
            "1",
        ];
        args.extend_from_slice(extra);
        self.run(&args, 0)
    }

    fn export(&self, source: &str, output: &str, now: &str, expected: i32) -> String {
        self.run(
            &[
                "reliance",
                "export",
                source,
                "--output",
                output,
                "--journal",
                "reliance.log",
                "--head",
                &self.head().to_hex(),
                "--now",
                now,
                "--uncertainty-ns",
                "1",
            ],
            expected,
        )
    }
}

impl Drop for Workspace {
    fn drop(&mut self) {
        // This uniquely created temporary tree contains only this test's generated protocol artifacts.
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn status(text: &str, expected: &str, reusable: bool) {
    assert!(
        text.contains(&format!("\"reliance_status\":\"{expected}\"")),
        "{text}"
    );
    assert!(
        text.contains(&format!("\"current_reuse_allowed\":{reusable}")),
        "{text}"
    );
}

#[test]
fn issued_copies_are_recallable_and_withdrawal_blocks_reuse_without_destroying_evidence() {
    let w = Workspace::new();
    let genesis = w.head();
    w.issue("sponsor.json", "sponsor", ISSUED, "owner.key", 0);
    assert_ne!(w.head(), genesis);
    let bytes = fs::read(w.path("sponsor.json")).unwrap();
    let ordinary = w.run(&["assure", "public", "--synthetic"], 0);
    assert_eq!(
        bytes,
        ordinary.as_bytes(),
        "issuance retains the exact assurance envelope and LF"
    );
    let digest = blake3_hash(&bytes);
    let one = w.records();
    assert_eq!(one.len(), 1);
    assert_eq!(one[0].output_digest, digest);
    assert_eq!(one[0].lane, IssuanceLane::Synthetic);
    assert_eq!(one[0].recipient_role, RoleId::Sponsor);
    assert!(!one[0].output_claims.is_empty());
    assert!(!one[0].code_revision.is_empty());
    assert_eq!(
        one[0].binary_digest,
        blake3_hash(&fs::read(env!("CARGO_BIN_EXE_fmine")).unwrap()),
        "the issuance fingerprints the actual executing binary with the shared BLAKE3 convention"
    );
    assert_ne!(one[0].input_manifest_root, Digest32::default());
    assert_ne!(one[0].anchor.state_root, Digest32::default());
    let report = fmine_public::run_synthetic_assurance(&[]).unwrap();
    let mut claims: Vec<_> = report
        .public_claims
        .cells
        .values()
        .chain(report.mine_stats_claims.claims.iter())
        .flat_map(|cell| {
            std::iter::once(cell.claim.claim_id).chain(cell.claim.parents.iter().copied())
        })
        .collect();
    claims.sort_unstable();
    claims.dedup();
    assert_eq!(
        one[0].output_claims, claims,
        "issued Claim inventory comes from the report"
    );
    assert_eq!(one[0].anchor, report.ledger_anchor);

    w.issue("reviewer.json", "reviewer-pool-7", CHECKED, "owner.key", 0);
    assert_eq!(fs::read(w.path("reviewer.json")).unwrap(), bytes);
    assert_eq!(w.records().len(), 2);
    let current = w.check("sponsor.json", CHECKED, &[]);
    status(&current, "current", true);
    assert!(current.contains("\"artifact_verified\":true"), "{current}");
    assert!(current.contains("\"head_pin_matched\":true"), "{current}");
    assert_eq!(current, w.check("sponsor.json", CHECKED, &[]));

    let recall = w.run(
        &[
            "recall",
            "--journal",
            "reliance.log",
            "--revision",
            &one[0].code_revision,
        ],
        0,
    );
    assert!(recall.contains("\"copies\":2"), "{recall}");
    assert!(
        recall.contains("\"recipient_role\":\"sponsor\""),
        "{recall}"
    );
    assert!(
        recall.contains("\"recipient_role\":\"reviewer-pool-7\""),
        "{recall}"
    );
    assert!(
        recall.contains("\"current_status_assessed\":false"),
        "{recall}"
    );
    let claim = one[0].output_claims[0].to_string();
    let by_claim = w.run(
        &["recall", "--journal", "reliance.log", "--claim", &claim],
        0,
    );
    assert!(by_claim.contains("\"copies\":2"), "{by_claim}");

    let head = w.head();
    w.export("sponsor.json", "current-copy.json", CHECKED, 0);
    assert_eq!(fs::read(w.path("current-copy.json")).unwrap(), bytes);
    assert_eq!(w.head(), head, "retrieval is not another issuance");

    w.run(
        &[
            "reliance",
            "withdraw",
            &digest.to_hex(),
            "--journal",
            "reliance.log",
            "--expected-head",
            &head.to_hex(),
            "--key",
            "owner.key",
            "--now",
            WITHDRAWN,
            "--uncertainty-ns",
            "1",
            "--reason",
            "invented defect found",
            "--defect",
            "code-defect",
            "--claim",
            &claim,
        ],
        0,
    );
    let withdrawn = w.check("sponsor.json", AFTER, &[]);
    status(&withdrawn, "withdrawn", false);
    assert!(
        withdrawn.contains("\"artifact_verified\":true"),
        "{withdrawn}"
    );
    assert!(withdrawn.contains(&claim), "{withdrawn}");
    assert_eq!(fs::read(w.path("sponsor.json")).unwrap(), bytes);
    let refused = w.export("sponsor.json", "forbidden-copy.json", AFTER, 3);
    assert!(refused.contains("FMINE-REF-202"), "{refused}");
    assert!(!w.path("forbidden-copy.json").exists());

    let historical = w.check(&digest.to_hex(), AFTER, &["--as-of", ISSUED]);
    status(&historical, "current", false);
    assert!(
        historical.contains("\"historical_query\":true"),
        "{historical}"
    );
    let journal_bytes = fs::read(w.path("reliance.log")).unwrap();
    w.issue("reissued.json", "sponsor", AFTER, "owner.key", 3);
    assert!(!w.path("reissued.json").exists());
    assert_eq!(fs::read(w.path("reliance.log")).unwrap(), journal_bytes);
}

#[test]
fn missing_stale_future_or_uncertain_trust_cannot_authorize_current_reuse() {
    let w = Workspace::new();
    w.issue("report.json", "sponsor", ISSUED, "owner.key", 0);
    let head = w.head().to_hex();
    let absent_time = w.run(
        &[
            "reliance",
            "check",
            "report.json",
            "--journal",
            "reliance.log",
            "--head",
            &head,
        ],
        0,
    );
    status(&absent_time, "unknown", false);
    assert!(
        absent_time.contains("trusted_time_unavailable"),
        "{absent_time}"
    );
    let absent_pin = w.run(
        &[
            "reliance",
            "check",
            "report.json",
            "--journal",
            "reliance.log",
            "--now",
            CHECKED,
            "--uncertainty-ns",
            "1",
        ],
        0,
    );
    status(&absent_pin, "unknown", false);
    assert!(
        absent_pin.contains("trusted_pin_unavailable"),
        "{absent_pin}"
    );

    for (now, as_of, reason) in [
        (CREATED, None, "future_status_cut_or_clock_rollback"),
        ("2030-01-01T00:01:02Z", None, "stale_status_cut"),
        (CHECKED, Some(AFTER), "future_as_of"),
    ] {
        let extra: Vec<_> = as_of.into_iter().flat_map(|at| ["--as-of", at]).collect();
        let out = w.check("report.json", now, &extra);
        status(&out, "unknown", false);
        assert!(out.contains(reason), "{out}");
    }
    let uncertain = w.run(
        &[
            "reliance",
            "check",
            "report.json",
            "--journal",
            "reliance.log",
            "--head",
            &head,
            "--now",
            CHECKED,
            "--uncertainty-ns",
            "3",
        ],
        0,
    );
    status(&uncertain, "unknown", false);
    assert!(
        uncertain.contains("clock_uncertainty_exceeds_policy"),
        "{uncertain}"
    );
    let wrong_pin = w.run(
        &[
            "reliance",
            "check",
            "report.json",
            "--journal",
            "reliance.log",
            "--head",
            &Digest32([9; 32]).to_hex(),
            "--now",
            CHECKED,
            "--uncertainty-ns",
            "1",
        ],
        0,
    );
    status(&wrong_pin, "unknown", false);
    assert!(wrong_pin.contains("trusted_head_mismatch"), "{wrong_pin}");
    w.export("report.json", "stale-copy.json", "2030-01-01T00:01:02Z", 3);
    assert!(!w.path("stale-copy.json").exists());

    w.run(
        &[
            "reliance",
            "refresh",
            "--journal",
            "reliance.log",
            "--expected-head",
            &head,
            "--key",
            "owner.key",
            "--now",
            "2030-01-01T00:01:03Z",
            "--uncertainty-ns",
            "1",
        ],
        0,
    );
    status(
        &w.check("report.json", "2030-01-01T00:01:04Z", &[]),
        "current",
        true,
    );
    let stale_pin = w.run(
        &[
            "reliance",
            "check",
            "report.json",
            "--journal",
            "reliance.log",
            "--head",
            &head,
            "--now",
            "2030-01-01T00:01:04Z",
            "--uncertainty-ns",
            "1",
        ],
        0,
    );
    status(&stale_pin, "unknown", false);
    assert!(stale_pin.contains("trusted_head_mismatch"), "{stale_pin}");
}

#[test]
fn invalid_authority_and_existing_destinations_leave_journal_and_artifacts_unchanged() {
    let w = Workspace::new();
    let genesis = fs::read(w.path("reliance.log")).unwrap();
    let denied = w.issue("wrong-owner.json", "sponsor", ISSUED, "wrong.key", 7);
    assert!(denied.contains("FMINE-REF-304"), "{denied}");
    assert!(!w.path("wrong-owner.json").exists());
    assert_eq!(fs::read(w.path("reliance.log")).unwrap(), genesis);

    fs::write(w.path("existing.json"), b"existing evidence").unwrap();
    w.issue("existing.json", "sponsor", ISSUED, "owner.key", 5);
    assert_eq!(
        fs::read(w.path("existing.json")).unwrap(),
        b"existing evidence"
    );
    assert_eq!(fs::read(w.path("reliance.log")).unwrap(), genesis);

    let old_head = w.head();
    w.issue("report.json", "sponsor", ISSUED, "owner.key", 0);
    let issued_journal = fs::read(w.path("reliance.log")).unwrap();
    let refused = w.run(
        &[
            "reliance",
            "refresh",
            "--journal",
            "reliance.log",
            "--expected-head",
            &old_head.to_hex(),
            "--key",
            "owner.key",
            "--now",
            CHECKED,
            "--uncertainty-ns",
            "1",
        ],
        6,
    );
    assert!(refused.contains("FMINE-REF-502"), "{refused}");
    assert_eq!(fs::read(w.path("reliance.log")).unwrap(), issued_journal);
    w.export("report.json", "existing.json", CHECKED, 5);
    assert_eq!(
        fs::read(w.path("existing.json")).unwrap(),
        b"existing evidence"
    );
    assert_eq!(fs::read(w.path("reliance.log")).unwrap(), issued_journal);

    fs::write(w.path("unissued.json"), b"unregistered altered output").unwrap();
    let unissued = w.check("unissued.json", CHECKED, &[]);
    status(&unissued, "unknown", false);
    assert!(
        unissued.contains("\"artifact_verified\":false"),
        "{unissued}"
    );
    let refused = w.export("unissued.json", "unissued-copy.json", CHECKED, 3);
    assert!(refused.contains("FMINE-REF-201"), "{refused}");
    assert!(!w.path("unissued-copy.json").exists());
    assert_eq!(fs::read(w.path("reliance.log")).unwrap(), issued_journal);
}

#[test]
fn registered_source_issuance_preserves_lane_verifies_bytes_and_supports_supersession() {
    const FIRST: &str = "2030-07-01T00:00:01Z";
    const SECOND: &str = "2030-07-01T00:00:02Z";
    const SUPERSEDED: &str = "2030-07-01T00:00:03Z";
    const CHECK: &str = "2030-07-01T00:00:04Z";
    let w = Workspace::new();
    fmine_public::synthetic::write_issuer_workspace(&w.0).unwrap();
    w.issue_registered("q1.json", Some("2030Q1"), FIRST, 0);
    let first_bytes = fs::read(w.path("q1.json")).unwrap();
    let ordinary = w.run(
        &[
            "assure",
            "public",
            "--issuer",
            "synthetic",
            "--through",
            "2030Q1",
        ],
        0,
    );
    assert_eq!(first_bytes, ordinary.as_bytes());
    let first = w.records()[0].clone();
    assert_eq!(first.lane, IssuanceLane::Synthetic);

    w.issue_registered("q2.json", None, SECOND, 0);
    let second_bytes = fs::read(w.path("q2.json")).unwrap();
    assert_eq!(
        second_bytes,
        w.run(&["assure", "public", "--issuer", "synthetic"], 0)
            .as_bytes()
    );
    let second = w.records()[1].clone();
    assert_eq!(second.lane, IssuanceLane::Synthetic);
    assert_ne!(first.output_digest, second.output_digest);
    assert_ne!(first.input_manifest_root, second.input_manifest_root);
    assert_ne!(first.anchor.state_root, second.anchor.state_root);

    w.run(
        &[
            "reliance",
            "supersede",
            &first.output_digest.to_hex(),
            "--replacement",
            &second.output_digest.to_hex(),
            "--journal",
            "reliance.log",
            "--expected-head",
            &w.head().to_hex(),
            "--key",
            "owner.key",
            "--now",
            SUPERSEDED,
            "--uncertainty-ns",
            "1",
        ],
        0,
    );
    let old = w.check("q1.json", CHECK, &[]);
    status(&old, "superseded", false);
    assert!(
        old.contains(&format!(
            "\"replacement\":\"{}\"",
            second.output_digest.to_hex()
        )),
        "{old}"
    );
    assert!(old.contains("\"artifact_verified\":true"), "{old}");
    status(&w.check("q2.json", CHECK, &[]), "current", true);
    w.export("q1.json", "superseded-copy.json", CHECK, 3);
    assert!(!w.path("superseded-copy.json").exists());
    w.export("q2.json", "replacement-copy.json", CHECK, 0);
    assert_eq!(
        fs::read(w.path("replacement-copy.json")).unwrap(),
        second_bytes
    );
    assert_eq!(fs::read(w.path("q1.json")).unwrap(), first_bytes);

    let before = fs::read(w.path("reliance.log")).unwrap();
    let source = w
        .path("corpus")
        .join(fmine_public::synthetic::SYNTHETIC_STATS_FILE);
    fs::write(&source, b"altered registered workbook bytes").unwrap();
    let altered = w.issue_registered("altered-source.json", None, CHECK, 8);
    assert!(altered.contains("FMINE-REF-505"), "{altered}");
    assert!(
        altered.contains(fmine_public::synthetic::SYNTHETIC_STATS_FILE),
        "{altered}"
    );
    assert!(!w.path("altered-source.json").exists());
    assert_eq!(fs::read(w.path("reliance.log")).unwrap(), before);
    assert_eq!(fs::read(w.path("q1.json")).unwrap(), first_bytes);
    assert_eq!(fs::read(w.path("q2.json")).unwrap(), second_bytes);
}

#[test]
fn explicit_and_host_clock_commands_share_issuances_and_withdrawal_authority() {
    let seconds = i64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    )
    .unwrap();
    let created = fmine_calib::record::format_utc(seconds);
    let w = Workspace::with_policy(&created, 600_000_000_000, 1_000_000_000);
    w.issue("explicit.json", "sponsor", &created, "owner.key", 0);
    let first = w.records()[0].clone();
    let bytes = fs::read(w.path("explicit.json")).unwrap();
    let digest = first.output_digest.to_hex();
    let declared_revision = first
        .code_revision
        .strip_suffix("+dirty")
        .unwrap_or(&first.code_revision);

    // DESIGN: revision and recipient are per-copy metadata. The shared artifact's claims, anchor,
    // manifest and taint must agree before another interface can issue the same report bytes.
    let issued = w.run(
        &[
            "reliance",
            "issue",
            "--journal",
            "reliance.log",
            "--head",
            &w.head().to_hex(),
            "--release-key",
            "owner.key",
            "--trust-host-clock",
            "--synthetic",
            "--output",
            "host.json",
            "--recipient-role",
            "reviewer-pool-3",
            "--purpose",
            "review",
            "--code-revision",
            declared_revision,
        ],
        0,
    );
    assert!(
        issued.contains("\"binary_digest_algorithm\":\"blake3\""),
        "{issued}"
    );
    let copies = w.records();
    assert_eq!(copies.len(), 2);
    assert_eq!(fs::read(w.path("host.json")).unwrap(), bytes);
    assert_eq!(copies[1].output_digest, first.output_digest);
    assert_eq!(copies[1].output_claims, first.output_claims);
    assert_eq!(copies[1].anchor, first.anchor);
    assert_eq!(copies[1].input_manifest_root, first.input_manifest_root);
    assert_eq!(copies[1].binary_digest, first.binary_digest);
    assert_eq!(copies[1].lane, first.lane);
    assert_eq!(copies[1].confidentiality, first.confidentiality);
    assert_eq!(copies[1].public_taint, first.public_taint);
    assert_eq!(copies[1].recipient_role, RoleId::ReviewerPool(3));
    assert_eq!(copies[1].code_revision, declared_revision);

    let current = w.run(
        &[
            "reliance",
            "check",
            "--journal",
            "reliance.log",
            "--digest",
            &digest,
            "--artifact",
            "explicit.json",
            "--head",
            &w.head().to_hex(),
            "--trust-host-clock",
        ],
        0,
    );
    assert!(
        current.contains("\"reliance\":{\"status\":\"current\""),
        "{current}"
    );
    assert!(
        current.contains("\"current_reuse_allowed\":true"),
        "{current}"
    );
    assert!(
        current.contains("\"artifact_bytes_verified\":true"),
        "{current}"
    );
    let recall = w.run(
        &[
            "reliance",
            "recall",
            "--journal",
            "reliance.log",
            "--claim",
            &first.output_claims[0].to_string(),
        ],
        0,
    );
    assert!(recall.contains("\"known_copy_count\":2"), "{recall}");
    assert!(
        recall.contains("\"recipient_role\":\"sponsor\""),
        "{recall}"
    );
    assert!(
        recall.contains("\"recipient_role\":\"reviewer-pool-3\""),
        "{recall}"
    );
    assert!(recall.contains("\"notifications_sent\":0"), "{recall}");

    w.run(
        &[
            "reliance",
            "withdraw",
            "--journal",
            "reliance.log",
            "--head",
            &w.head().to_hex(),
            "--release-key",
            "owner.key",
            "--trust-host-clock",
            "--digest",
            &digest,
            "--all-claims",
            "--defect",
            "code-defect",
            "--reason",
            "invented shared-journal defect",
        ],
        0,
    );
    let withdrawn = w.run(
        &[
            "reliance",
            "check",
            "--journal",
            "reliance.log",
            "--digest",
            &digest,
            "--artifact",
            "explicit.json",
            "--head",
            &w.head().to_hex(),
            "--trust-host-clock",
        ],
        0,
    );
    assert!(
        withdrawn.contains("\"reliance\":{\"status\":\"withdrawn\""),
        "{withdrawn}"
    );
    assert!(
        withdrawn.contains("\"current_reuse_allowed\":false"),
        "{withdrawn}"
    );
    assert!(
        withdrawn.contains("\"artifact_bytes_verified\":true"),
        "{withdrawn}"
    );

    // Explicit CLI instants have second precision. The next second covers the host event's fractional
    // timestamp without sleeping, and remains within the deliberately generous test freshness policy.
    let seconds = i64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    )
    .unwrap();
    let after = fmine_calib::record::format_utc(seconds + 1);
    status(&w.check("explicit.json", &after, &[]), "withdrawn", false);
    w.export("explicit.json", "blocked-cross-interface.json", &after, 3);
    assert!(!w.path("blocked-cross-interface.json").exists());
    assert_eq!(fs::read(w.path("explicit.json")).unwrap(), bytes);
    assert_eq!(fs::read(w.path("host.json")).unwrap(), bytes);
}

#[test]
fn ordinary_assurance_output_registers_the_exact_selected_format() {
    let (w, _) = Workspace::with_live_clock();
    let revision = env!("FMINE_BUILD_REVISION").trim_end_matches("+dirty");
    let binary_digest = blake3_hash(&fs::read(env!("CARGO_BIN_EXE_fmine")).unwrap());
    let genesis = w.head();
    w.run(
        &[
            "assure",
            "public",
            "--synthetic",
            "--output",
            "missing-authority.json",
        ],
        2,
    );
    assert!(!w.path("missing-authority.json").exists());
    assert_eq!(w.head(), genesis);
    assert!(w.records().is_empty());

    for (index, (format, output)) in [("json", "report.json"), ("markdown", "report.md")]
        .into_iter()
        .enumerate()
    {
        let head = w.head();
        let preview_args = ["assure", "public", "--synthetic", "--format", format];
        let preview = if format == "json" {
            w.run(&preview_args, 0)
        } else {
            w.run_text(&preview_args, 0)
        };
        assert_eq!(w.head(), head, "an ordinary preview creates no issuance");
        let head = head.to_hex();
        let args = [
            "assure",
            "public",
            "--synthetic",
            "--output",
            output,
            "--journal",
            "reliance.log",
            "--head",
            &head,
            "--release-key",
            "owner.key",
            "--trust-host-clock",
            "--recipient-role",
            "sponsor",
            "--purpose",
            "review",
            "--code-revision",
            revision,
            "--format",
            format,
        ];
        let receipt = if format == "json" {
            w.run(&args, 0)
        } else {
            w.run_text(&args, 0)
        };
        let bytes = fs::read(w.path(output)).unwrap();
        assert_eq!(
            bytes,
            preview.as_bytes(),
            "selected artifact preserves exact report and LF"
        );
        assert_ne!(
            bytes,
            receipt.as_bytes(),
            "receipt is separate from the issued report"
        );
        let records = w.records();
        assert_eq!(records.len(), index + 1);
        let issued = &records[index];
        assert_eq!(issued.output_digest, blake3_hash(&bytes));
        assert_eq!(issued.code_revision, revision);
        assert_eq!(issued.binary_digest, binary_digest);
        assert_eq!(issued.recipient_role, RoleId::Sponsor);
        assert_eq!(issued.lane, IssuanceLane::Synthetic);
        assert!(!issued.output_claims.is_empty());
        assert_ne!(issued.anchor.state_root, Digest32::default());
        assert!(
            receipt.contains(&format!("\"issuance_id\":\"{}\"", issued.digest().unwrap())),
            "{receipt}"
        );
        assert!(
            receipt.contains(&format!("\"output_digest\":\"{}\"", issued.output_digest)),
            "{receipt}"
        );
        if format == "markdown" {
            assert!(
                preview.starts_with("# fmine `assure public` (ok)\n"),
                "{preview}"
            );
            assert!(
                receipt.starts_with("# fmine `assure public` (ok)\n"),
                "{receipt}"
            );
            assert!(receipt.contains("\"operation\":\"issue\""), "{receipt}");
            assert!(preview.ends_with("```\n"));
            assert!(receipt.ends_with("```\n"));
        }
    }
    let before = fs::read(w.path("reliance.log")).unwrap();
    let retained = fs::read(w.path("report.json")).unwrap();
    w.run(
        &[
            "assure",
            "public",
            "--synthetic",
            "--output",
            "report.json",
            "--journal",
            "reliance.log",
            "--head",
            &w.head().to_hex(),
            "--release-key",
            "owner.key",
            "--trust-host-clock",
            "--recipient-role",
            "reviewer-pool-8",
            "--purpose",
            "review",
            "--code-revision",
            revision,
        ],
        5,
    );
    assert_eq!(fs::read(w.path("reliance.log")).unwrap(), before);
    assert_eq!(fs::read(w.path("report.json")).unwrap(), retained);
    assert_eq!(w.records().len(), 2);
}

#[test]
fn explicit_receipt_alias_forwards_original_provenance_and_withdrawal_blocks_new_recipient() {
    let (w, created) = Workspace::with_live_clock();
    let receipt = w.issue("original.json", "sponsor", &created, "owner.key", 0);
    let source_id = receipt
        .split_once("\"issuance_id\":\"")
        .expect("explicit receipt exposes the interoperable issuance ID")
        .1
        .split('"')
        .next()
        .expect("issuance ID terminator");
    let first = w.records()[0].clone();
    assert_eq!(source_id, first.digest().unwrap().to_hex());
    assert!(
        receipt.contains(&format!("\"issuance_digest\":\"{source_id}\"")),
        "{receipt}"
    );
    let bytes = fs::read(w.path("original.json")).unwrap();
    assert_eq!(first.output_digest, blake3_hash(&bytes));
    assert_eq!(
        first.binary_digest,
        blake3_hash(&fs::read(env!("CARGO_BIN_EXE_fmine")).unwrap())
    );
    let forwarded = w.run(
        &[
            "reliance",
            "reissue",
            "--journal",
            "reliance.log",
            "--head",
            &w.head().to_hex(),
            "--release-key",
            "owner.key",
            "--trust-host-clock",
            "--artifact",
            "original.json",
            "--output",
            "forwarded.json",
            "--source-issuance",
            source_id,
            "--recipient-role",
            "reviewer-pool-9",
            "--purpose",
            "review",
        ],
        0,
    );
    assert!(
        forwarded.contains(&format!("\"source_issuance_id\":\"{source_id}\"")),
        "{forwarded}"
    );
    assert!(
        forwarded.contains("\"delivery_confirmed\":false"),
        "{forwarded}"
    );
    assert_eq!(fs::read(w.path("forwarded.json")).unwrap(), bytes);
    assert_eq!(fs::read(w.path("original.json")).unwrap(), bytes);
    let records = w.records();
    assert_eq!(records.len(), 2);
    assert_eq!(records[0], first);
    let second = &records[1];
    assert!(second.issued_at_ns >= first.issued_at_ns);
    let mut expected = first.clone();
    expected.recipient_role = RoleId::ReviewerPool(9);
    expected.purpose = "review".to_string();
    expected.issued_at_ns = second.issued_at_ns;
    assert_eq!(
        *second, expected,
        "forwarding preserves every original producer field"
    );
    assert_ne!(second.digest().unwrap(), first.digest().unwrap());
    assert!(
        forwarded.contains(&format!("\"issuance_id\":\"{}\"", second.digest().unwrap())),
        "{forwarded}"
    );

    let recall = w.run(
        &[
            "recall",
            "--journal",
            "reliance.log",
            "--revision",
            &first.code_revision,
        ],
        0,
    );
    assert!(recall.contains("\"copies\":2"), "{recall}");
    for record in &records {
        let id = record.digest().unwrap();
        assert!(
            recall.contains(&format!("\"issuance_digest\":\"{id}\"")),
            "{recall}"
        );
        assert!(
            recall.contains(&format!("\"issuance_id\":\"{id}\"")),
            "{recall}"
        );
        assert!(
            recall.contains(&format!("\"recipient_role\":\"{}\"", record.recipient_role)),
            "{recall}"
        );
    }
    let by_claim = w.run(
        &[
            "reliance",
            "recall",
            "--journal",
            "reliance.log",
            "--claim",
            &first.output_claims[0].to_string(),
        ],
        0,
    );
    assert!(by_claim.contains("\"known_copy_count\":2"), "{by_claim}");
    w.run(
        &[
            "reliance",
            "withdraw",
            "--journal",
            "reliance.log",
            "--head",
            &w.head().to_hex(),
            "--release-key",
            "owner.key",
            "--trust-host-clock",
            "--digest",
            &first.output_digest.to_hex(),
            "--all-claims",
            "--defect",
            "code-defect",
            "--reason",
            "invented forwarding source defect",
        ],
        0,
    );
    let head = w.head();
    let journal = fs::read(w.path("reliance.log")).unwrap();
    let refused = w.run(
        &[
            "reliance",
            "reissue",
            "--journal",
            "reliance.log",
            "--head",
            &head.to_hex(),
            "--release-key",
            "owner.key",
            "--trust-host-clock",
            "--artifact",
            "forwarded.json",
            "--output",
            "blocked.json",
            "--source-issuance",
            source_id,
            "--recipient-role",
            "reviewer-pool-10",
            "--purpose",
            "review",
        ],
        3,
    );
    assert!(refused.contains("FMINE-REF-202"), "{refused}");
    assert!(!w.path("blocked.json").exists());
    assert_eq!(w.head(), head);
    assert_eq!(fs::read(w.path("reliance.log")).unwrap(), journal);
    assert_eq!(w.records(), records);
    assert_eq!(fs::read(w.path("original.json")).unwrap(), bytes);
    assert_eq!(fs::read(w.path("forwarded.json")).unwrap(), bytes);
}
