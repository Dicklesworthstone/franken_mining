//! Synthetic decision choices retain the exact comparison rather than reevaluating mutable inputs.
//! These tests exercise local history only, with invented clocks and explicitly declared code identities.

#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use fmine_claim::forecast_journal::Entry;
use fmine_cli::decision_journal::{Command, execute_with_clock};
use fmine_cli::{CliCommand, Envelope, OutputFormat, execute_command, parse_args};
use fmine_core::hash::{Digest32, sha256};
use fmine_io::forecast::Journal;
use fmine_registry::toml_subset::{TomlValue, parse_subset};
use fmine_runtime::{ClockError, WallClock};

static NEXT: AtomicU64 = AtomicU64::new(0);
const MODEL: &str = include_str!("../../../tests/decisions/expansion.toml");
const REVISION: &str = "0123456789abcdef0123456789abcdef01234567";

struct Clock(i64);
impl WallClock for Clock {
    fn now_ns(&self) -> Result<i64, ClockError> {
        Ok(self.0)
    }
}

struct NoClock;
impl WallClock for NoClock {
    fn now_ns(&self) -> Result<i64, ClockError> {
        panic!("read-only history must not observe a clock")
    }
}

fn field<'a>(value: &'a str, key: &str) -> &'a str {
    value
        .split_once(&format!("\"{key}\":\""))
        .unwrap()
        .1
        .split('"')
        .next()
        .unwrap()
}

fn head(envelope: &Envelope) -> Digest32 {
    Digest32(fmine_calib::record::parse_hex32(field(&envelope.payload_json, "head")).unwrap())
}

fn successful(envelope: Envelope) -> Envelope {
    assert_eq!(envelope.exit_code, 0, "{envelope:?}");
    for flag in [
        "reserve_disclosure_claim",
        "execution_authorized",
        "geotechnical_signoff",
        "regulatory_compliance_claim",
    ] {
        assert!(envelope.to_json().contains(&format!("\"{flag}\":false")));
    }
    envelope
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

struct Fixture {
    root: PathBuf,
    journal: PathBuf,
    model: PathBuf,
    initial: Digest32,
}

impl Fixture {
    fn new() -> Self {
        let root = loop {
            let candidate = std::env::temp_dir().join(format!(
                "fmine-decision-journal-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            match fs::create_dir(&candidate) {
                Ok(()) => break candidate,
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => panic!("cannot create synthetic decision fixture: {error}"),
            }
        };
        let journal = root.join("decisions.journal");
        let model = root.join("model.toml");
        fs::write(&model, MODEL).unwrap();
        let created = successful(execute_with_clock(
            &Command::Init {
                journal: journal.clone(),
                cell_id: "synthetic-decisions".into(),
                clock_skew_ns: 10,
            },
            &root,
            &Clock(100),
        ));
        assert!(created.payload_json.contains("\"n_decisions\":0"));
        Self {
            root,
            journal,
            model,
            initial: head(&created),
        }
    }

    fn record(&self, expected_head: Digest32, id: &str, action: &str) -> Command {
        Command::Record {
            journal: self.journal.clone(),
            expected_head,
            model: self.model.clone(),
            decision_id: id.into(),
            action: action.into(),
            rationale: "Keep this explicit choice beside every compared alternative".into(),
            code_revision: REVISION.into(),
        }
    }

    fn show(&self, pin: Option<Digest32>) -> Envelope {
        execute_with_clock(
            &Command::Show {
                journal: self.journal.clone(),
                head: pin,
            },
            &self.root,
            &NoClock,
        )
    }

    fn refuses_without_append(&self, command: &Command, time: i64, code: &str) {
        let before = fs::read(&self.journal).unwrap();
        let envelope = execute_with_clock(command, &self.root, &Clock(time));
        assert_ne!(envelope.exit_code, 0, "{envelope:?}");
        assert_eq!(
            envelope.refusal.as_ref().unwrap().code,
            code,
            "{envelope:?}"
        );
        assert_eq!(fs::read(&self.journal).unwrap(), before);
    }
}

#[test]
fn exact_model_and_card_survive_source_changes_and_choices_append() {
    let f = Fixture::new();
    let comparison = successful(execute_command(
        &CliCommand::DecideCard {
            model: f.model.clone(),
        },
        &f.root,
    ));
    let expected_card = comparison
        .payload_json
        .split_once(",\"card\":")
        .unwrap()
        .1
        .strip_suffix('}')
        .unwrap()
        .to_owned();
    let first = successful(execute_with_clock(
        &f.record(f.initial, "choice-one", "smaller"),
        &f.root,
        &Clock(200),
    ));
    let first_head = head(&first);
    assert!(
        first
            .payload_json
            .contains("\"time_bounds_ns\":[\"190\",\"210\"]")
    );
    assert!(
        first
            .payload_json
            .contains("\"selected_action\":\"smaller\"")
    );
    assert!(
        first
            .payload_json
            .contains(&sha256(expected_card.as_bytes()).to_hex())
    );
    let saved = Journal::open_read(&f.journal, Some(first_head)).unwrap();
    let Entry::Operation(operation) = &saved.records()[1].entry else {
        panic!("operation")
    };
    let parsed = parse_subset(operation).unwrap();
    assert_eq!(
        parsed.top_get("model_hex").unwrap().value,
        TomlValue::Str(hex(MODEL.as_bytes()))
    );
    assert_eq!(
        parsed.top_get("card_hex").unwrap().value,
        TomlValue::Str(hex(expected_card.as_bytes()))
    );
    assert_eq!(
        parsed.top_get("claim_id").unwrap().value,
        TomlValue::Str(field(&expected_card, "id").into())
    );
    drop(saved);
    let before = successful(f.show(Some(first_head)));
    fs::write(&f.model, "This is no longer a decision model").unwrap();
    let after = successful(f.show(Some(first_head)));
    assert_eq!(before.payload_json, after.payload_json);
    assert_eq!(before.rendered_markdown, after.rendered_markdown);
    assert!(after.payload_json.contains("\"final_head_pinned\":true"));
    fs::write(&f.model, MODEL).unwrap();
    let second = successful(execute_with_clock(
        &f.record(first_head, "choice-two", "hold"),
        &f.root,
        &Clock(300),
    ));
    let final_report = successful(f.show(Some(head(&second))));
    assert!(final_report.payload_json.contains("\"n_decisions\":2"));
    assert!(
        final_report
            .payload_json
            .contains("\"selected_action\":\"smaller\"")
    );
    assert!(
        final_report
            .payload_json
            .contains("\"selected_action\":\"hold\"")
    );
    assert!(
        final_report
            .no_claim
            .unwrap()
            .contains("not a new evaluation")
    );
}

#[test]
fn stale_heads_duplicate_ids_and_invalid_choices_never_append() {
    let f = Fixture::new();
    let first = successful(execute_with_clock(
        &f.record(f.initial, "choice-one", "build"),
        &f.root,
        &Clock(200),
    ));
    let pin = head(&first);
    f.refuses_without_append(
        &f.record(f.initial, "choice-two", "hold"),
        300,
        "FMINE-REF-502",
    );
    f.refuses_without_append(&f.record(pin, "choice-one", "hold"), 300, "FMINE-REF-507");
    f.refuses_without_append(
        &f.record(pin, "choice-two", "unknown"),
        300,
        "FMINE-REF-107",
    );
    let mut bad_reason = f.record(pin, "choice-two", "build");
    if let Command::Record { rationale, .. } = &mut bad_reason {
        *rationale = " ".into();
    }
    f.refuses_without_append(&bad_reason, 300, "FMINE-REF-107");
    let mut bad_revision = f.record(pin, "choice-two", "build");
    if let Command::Record { code_revision, .. } = &mut bad_revision {
        *code_revision = "main".into();
    }
    f.refuses_without_append(&bad_revision, 300, "FMINE-REF-107");
    f.refuses_without_append(&f.record(pin, "choice-two", "hold"), 199, "FMINE-REF-502");
    successful(f.show(Some(pin)));
}

#[test]
fn oversized_or_invalid_models_refuse_before_append() {
    let f = Fixture::new();
    let command = f.record(f.initial, "choice", "build");
    fs::write(&f.model, format!("{MODEL}\n# {}\n", "x".repeat(40_000))).unwrap();
    f.refuses_without_append(&command, 200, "FMINE-REF-107");
    fs::write(&f.model, "not a model").unwrap();
    let before = fs::read(&f.journal).unwrap();
    assert_ne!(
        execute_with_clock(&command, &f.root, &Clock(200)).exit_code,
        0
    );
    assert_eq!(fs::read(&f.journal).unwrap(), before);
}

#[test]
fn retained_content_digest_corruption_and_truncated_prefixes_refuse() {
    let f = Fixture::new();
    let first = successful(execute_with_clock(
        &f.record(f.initial, "choice", "build"),
        &f.root,
        &Clock(200),
    ));
    let pin = head(&first);
    let saved = Journal::open_read(&f.journal, Some(pin)).unwrap();
    let records = saved.records().to_vec();
    drop(saved);

    let prefix = f.root.join("prefix.journal");
    fs::write(&prefix, records[0].to_line().unwrap()).unwrap();
    let check = |path: &Path, head| {
        execute_with_clock(
            &Command::Show {
                journal: path.into(),
                head,
            },
            &f.root,
            &NoClock,
        )
    };
    assert_ne!(check(&prefix, Some(pin)).exit_code, 0);
    let unpinned = successful(check(&prefix, None));
    assert!(!unpinned.warnings.is_empty());

    let broken = f.root.join("truncated.journal");
    let bytes = fs::read(&f.journal).unwrap();
    fs::write(&broken, &bytes[..bytes.len() - 1]).unwrap();
    assert_ne!(check(&broken, None).exit_code, 0);

    let changed = f.root.join("changed.journal");
    let mut last = records[1].clone();
    let Entry::Operation(operation) = &mut last.entry else {
        panic!("operation")
    };
    let parsed = parse_subset(operation).unwrap();
    let TomlValue::Str(old_digest) = &parsed.top_get("card_sha256").unwrap().value else {
        panic!("digest")
    };
    *operation = operation.replace(old_digest, &"a".repeat(64));
    let mut bytes = records[0].to_line().unwrap();
    bytes.extend(last.to_line().unwrap());
    fs::write(&changed, bytes).unwrap();
    let refusal = check(&changed, Some(last.chain_head().unwrap()));
    assert_eq!(refusal.refusal.unwrap().code, "FMINE-REF-505");
}

#[test]
fn another_journal_protocol_and_existing_init_path_are_refused() {
    let f = Fixture::new();
    let other = f.root.join("forecasts.journal");
    let journal = Journal::create(&other, "forecast-cell", 10, &Clock(100)).unwrap();
    let pin = journal.head();
    drop(journal);
    let result = execute_with_clock(
        &Command::Show {
            journal: other,
            head: Some(pin),
        },
        &f.root,
        &NoClock,
    );
    assert_eq!(result.refusal.unwrap().code, "FMINE-REF-505");
    f.refuses_without_append(
        &Command::Init {
            journal: f.journal.clone(),
            cell_id: "replacement".into(),
            clock_skew_ns: 0,
        },
        200,
        "FMINE-REF-506",
    );
}

#[test]
fn independently_retained_head_rejects_a_self_consistently_rewritten_card() {
    let f = Fixture::new();
    let first = successful(execute_with_clock(
        &f.record(f.initial, "choice", "build"),
        &f.root,
        &Clock(200),
    ));
    let pin = head(&first);
    // Rebuild a complete alternative history through the real writer, so every Claim, card,
    // content and journal hash is valid. It is nevertheless not the independently pinned history.
    let replacement = Fixture::new();
    assert_eq!(replacement.initial, f.initial);
    let altered = MODEL.replace("Which invented", "Changed invented");
    assert_ne!(altered, MODEL);
    fs::write(&replacement.model, altered).unwrap();
    let rebuilt = successful(execute_with_clock(
        &replacement.record(replacement.initial, "choice", "build"),
        &replacement.root,
        &Clock(200),
    ));
    assert_ne!(head(&rebuilt), pin);
    let forged = f.root.join("rewritten.journal");
    fs::write(&forged, fs::read(&replacement.journal).unwrap()).unwrap();
    let check = |head| {
        execute_with_clock(
            &Command::Show {
                journal: forged.clone(),
                head,
            },
            &f.root,
            &NoClock,
        )
    };
    let refused = check(Some(pin));
    assert_eq!(refused.refusal.unwrap().code, "FMINE-REF-502");
    let unpinned = successful(check(None));
    assert!(
        unpinned
            .warnings
            .iter()
            .any(|w| w.contains("whole-history rewriting"))
    );
    assert!(
        unpinned
            .no_claim
            .unwrap()
            .contains("not independent evaluation provenance")
    );
}

#[test]
fn cli_requires_explicit_mutation_policy_and_registers_all_operations() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let parse = |words: &[&str]| {
        parse_args(
            &words.iter().map(|s| (*s).to_string()).collect::<Vec<_>>(),
            &root,
        )
    };
    let (command, format, _) = parse(&[
        "decide",
        "journal",
        "show",
        "--journal",
        "history",
        "--format",
        "markdown",
    ])
    .unwrap();
    assert!(matches!(
        command,
        CliCommand::DecisionJournal(Command::Show { head: None, .. })
    ));
    assert_eq!(format, OutputFormat::Markdown);
    assert!(
        parse(&[
            "decide",
            "journal",
            "init",
            "--journal",
            "history",
            "--cell",
            "test"
        ])
        .is_err()
    );
    assert!(
        parse(&[
            "decide",
            "journal",
            "show",
            "--journal",
            "history",
            "--reason",
            "unused"
        ])
        .is_err()
    );
    assert!(
        parse(&[
            "decide",
            "journal",
            "record",
            "--journal",
            "history",
            "--expected-head",
            &"0".repeat(64),
            "--model",
            "model.toml",
            "--decision",
            "decision",
            "--reason",
            "why",
            "--code-revision",
            REVISION,
            "--trust-host-clock"
        ])
        .is_err()
    );
    let registry = include_str!("../../../registries/verbs.toml");
    let docs = successful(execute_command(&CliCommand::RobotDocs, &root));
    for verb in [
        "decide journal init",
        "decide journal record",
        "decide journal show",
    ] {
        assert!(registry.contains(&format!("path = \"{verb}\"")));
        assert!(docs.payload_json.contains(verb));
    }
}
