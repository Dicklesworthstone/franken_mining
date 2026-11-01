//! Real-binary acceptance for versioned forecast admission across process restarts.
//! Synthetic forecasts and the declared local clock bound are not external timestamp proof.

#![allow(clippy::expect_used, clippy::panic)]

use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use fmine_calib::record::format_utc;
use fmine_claim::forecast_journal::Entry;
use fmine_io::forecast::Journal;

const SKEW_NS: i64 = 1_000_000;
const FORECAST: &str = "[[forecast]]\nid = \"f1\"\nmodel_key = \"synthetic:binary\"\noutcome_due = \"2200-01-01T00:00:00Z\"\nkind = \"interval\"\nlo = \"40\"\nhi = \"50\"\nnominal = \"0.8\"\n";
const COHORT: &str = "[[cohort]]\nname = \"one-site\"\nmembers = \"f1\"\nresolution_rule = \"Invented output\"\neligible_sources = \"synthetic:issuer\"\n";

// These controlled envelopes use unescaped scalar fields; no extra JSON dependency is needed.
fn field<'a>(json: &'a str, key: &str) -> &'a str {
    json.split_once(&format!("\"{key}\":"))
        .unwrap_or_else(|| panic!("missing {key}: {json}"))
        .1
        .split([',', '}'])
        .next()
        .expect("scalar")
        .trim_matches('"')
}

struct Workspace(PathBuf);
impl Workspace {
    fn new() -> Self {
        static SERIAL: AtomicU64 = AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "fmine-forecast-v2-binary-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).expect("fresh synthetic workspace");
        Self(root)
    }
    fn run(&self, args: &[&str], exit: i32) -> String {
        let output = Command::new(env!("CARGO_BIN_EXE_fmine"))
            .args(["--json", "--workspace"])
            .arg(&self.0)
            .args(["forecast", "journal"])
            .args(args)
            .output()
            .expect("run fmine binary");
        let json = String::from_utf8(output.stdout).expect("UTF-8 envelope");
        assert_eq!(
            output.status.code(),
            Some(exit),
            "{args:?}: {json}\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(json.lines().count(), 1, "one envelope: {json}");
        assert_eq!(field(&json, "exit_code"), exit.to_string());
        json
    }
    fn init(&self) -> String {
        self.run(
            &[
                "init",
                "--journal",
                "journal",
                "--cell",
                "synthetic-binary",
                "--max-clock-skew-ms",
                "1",
                "--trust-host-clock",
            ],
            0,
        )
    }
    fn append(&self, verb: &str, head: &str, request: &str, exit: i32) -> String {
        fs::write(self.0.join("request.toml"), request).expect("synthetic request");
        self.run(
            &[
                verb,
                "--journal",
                "journal",
                "--head",
                head,
                "--request",
                "request.toml",
                "--trust-host-clock",
            ],
            exit,
        )
    }
    fn bytes(&self) -> Vec<u8> {
        fs::read(self.0.join("journal")).expect("committed journal")
    }
}

#[test]
fn binary_v2_registration_and_freeze_survive_restart_with_exact_upper_times() {
    let workspace = Workspace::new();
    let init = workspace.init();
    let registered = workspace.append("register", field(&init, "head"), FORECAST, 0);
    let frozen = workspace.append("freeze", field(&registered, "head"), COHORT, 0);
    let before = workspace.bytes();
    let inspected = workspace.run(
        &[
            "inspect",
            "--journal",
            "journal",
            "--head",
            field(&frozen, "head"),
        ],
        0,
    );
    assert_eq!(field(&inspected, "head"), field(&frozen, "head"));
    assert_eq!(
        field(&inspected, "forecast_id"),
        field(&registered, "forecast_id")
    );
    assert_eq!(field(&inspected, "record_time_policy"), "clock_upper_bound");
    assert!(inspected.contains("\"operation_version_counts\":{\"legacy_v1\":0,\"v2\":2}"));
    assert_eq!(field(&inspected, "n_forecasts"), "1");
    assert_eq!(field(&inspected, "n_cohorts"), "1");
    assert_eq!(workspace.bytes(), before);

    // Inspect the actual committed records after all subprocesses have exited; never hold a lock across spawn.
    let journal = Journal::open_read(&workspace.0.join("journal"), None).expect("durable records");
    assert_eq!(journal.header().clock_skew_ns, SKEW_NS as u64);
    assert_eq!(journal.records().len(), 3);
    for record in &journal.records()[1..] {
        let Entry::Operation(request) = &record.entry else {
            panic!("operation")
        };
        assert!(request.starts_with("schema = \"fmine.forecast-operation.v2\"\n"));
    }
    let upper = journal.records()[1].observed_ns + SKEW_NS;
    assert_eq!(field(&registered, "registered_ns"), upper.to_string());
    assert_eq!(field(&inspected, "registered_ns"), upper.to_string());
    let upper = journal.records()[2].observed_ns + SKEW_NS;
    let whole = format_utc(upper / 1_000_000_000);
    let expected = if upper % 1_000_000_000 == 0 {
        whole
    } else {
        format!(
            "{}.{:09}Z",
            whole.trim_end_matches('Z'),
            upper % 1_000_000_000
        )
    };
    assert_eq!(field(&inspected, "frozen_at"), expected);
}

#[test]
fn binary_rejects_caller_time_version_and_stale_head_without_changing_history() {
    let workspace = Workspace::new();
    let init = workspace.init();
    let registered = workspace.append("register", field(&init, "head"), FORECAST, 0);
    let head = field(&registered, "head");
    let before = workspace.bytes();
    let next = FORECAST.replace("\"f1\"", "\"f2\"");
    for (verb, request) in [
        ("register", format!("{next}registered_at = \"2001-01-01T00:00:00Z\"\n")),
        ("freeze", format!("{COHORT}frozen_at = \"2001-01-01T00:00:00Z\"\n")),
        ("outcome", "[[outcome]]\nforecast = \"f1\"\nkind = \"missing\"\navailable_at = \"2001-01-01T00:00:00Z\"\nrecorded_at = \"2001-01-01T00:00:00Z\"\nreason = \"not_published\"\n".into()),
        ("register", format!("schema = \"fmine.forecast-ledger.v1\"\n{next}")),
        ("register", format!("schema = \"fmine.forecast-operation.v2\"\n{next}")),
        ("register", format!("clock_skew_ns = 0\n{next}")),
        ("register", format!("{next}{COHORT}")),
    ] {
        let refused = workspace.append(verb, head, &request, 3);
        assert!(refused.contains("FMINE-REF-107"), "{refused}");
        assert_eq!(workspace.bytes(), before);
    }
    let stale = workspace.append("freeze", field(&init, "head"), COHORT, 6);
    assert!(stale.contains("FMINE-REF-502"), "{stale}");
    assert_eq!(workspace.bytes(), before);
    workspace.run(
        &[
            "freeze",
            "--journal",
            "journal",
            "--head",
            head,
            "--request",
            "request.toml",
        ],
        2,
    );
    assert_eq!(workspace.bytes(), before);
    let reopened = workspace.run(&["inspect", "--journal", "journal", "--head", head], 0);
    assert_eq!(field(&reopened, "head"), head);
    assert_eq!(field(&reopened, "n_operations"), "1");
}
