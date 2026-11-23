//! Synthetic decision choices retain the exact comparison rather than reevaluating mutable inputs.
//! These tests exercise local history only, with invented clocks and explicitly declared code identities.

#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command as Process;
use std::sync::atomic::{AtomicU64, Ordering};

use fmine_claim::forecast_journal::Entry;
use fmine_cli::decision_journal::{
    Choice, Command, ForecastSelection, PolicySelection, execute_with_clock,
};
use fmine_cli::forecast_journal::{
    Command as ForecastCommand, execute_with_clock as forecast_with_clock,
};
use fmine_cli::{CliCommand, Envelope, OutputFormat, execute_command, parse_args};
use fmine_core::hash::{Digest32, sha256};
use fmine_io::forecast::Journal;
use fmine_io::omf::json::{Json, parse as parse_json};
use fmine_registry::toml_subset::{TomlValue, parse_subset};
use fmine_runtime::{ClockError, WallClock};

static NEXT: AtomicU64 = AtomicU64::new(0);
const MODEL: &str = include_str!("../../../tests/decisions/expansion.toml");
const INFORMATION: &str = include_str!("../../../tests/decisions/metallurgical_test.toml");
const CONSTRAINED: &str = include_str!("../../../tests/decisions/constrained_testwork.toml");
const SHIFT_FEED: &str = include_str!("../../../tests/decisions/stockpile_holdback.toml");
const REVISION: &str = "0123456789abcdef0123456789abcdef01234567";

fn binary() -> PathBuf {
    if let Some(path) = std::env::var_os("FMINE_TEST_BINARY") {
        return PathBuf::from(path);
    }
    let executable = std::env::current_exe().expect("current T executable");
    let profile = executable
        .ancestors()
        .find(|path| {
            path.file_name()
                .is_some_and(|name| name == "deps" || name == "build")
        })
        .and_then(Path::parent)
        .expect("Cargo profile directory; set FMINE_TEST_BINARY for another layout");
    profile.join(format!("fmine{}", std::env::consts::EXE_SUFFIX))
}

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
            choice: Choice::ActNow(action.into()),
            rationale: "Keep this explicit choice beside every compared alternative".into(),
            code_revision: REVISION.into(),
            forecasts: None,
        }
    }

    fn information(
        &self,
        expected_head: Digest32,
        id: &str,
        option_id: &str,
        policy: PolicySelection,
    ) -> Command {
        let mut command = self.record(expected_head, id, "unused");
        if let Command::Record { choice, .. } = &mut command {
            *choice = Choice::InformationOption {
                option_id: option_id.into(),
                policy,
            };
        }
        command
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

fn operation_fields(f: &Fixture, pin: Digest32, sequence: usize) -> Vec<(String, String)> {
    let saved = Journal::open_read(&f.journal, Some(pin)).unwrap();
    let Entry::Operation(operation) = &saved.records()[sequence].entry else {
        panic!("operation")
    };
    parse_subset(operation)
        .unwrap()
        .top
        .into_iter()
        .map(|field| {
            let TomlValue::Str(value) = field.value else {
                panic!("string field")
            };
            (field.key, value)
        })
        .collect()
}

fn retained_policy(f: &Fixture, pin: Digest32, sequence: usize) -> String {
    let fields = operation_fields(f, pin, sequence);
    let policy = if let Some((_, value)) = fields.iter().find(|(key, _)| key == "policy_json") {
        value.clone()
    } else {
        let value = &fields
            .iter()
            .find(|(key, _)| key == "policy_hex")
            .unwrap()
            .1;
        let bytes: Vec<u8> = (0..value.len())
            .step_by(2)
            .map(|index| u8::from_str_radix(&value[index..index + 2], 16).unwrap())
            .collect();
        String::from_utf8(bytes).unwrap()
    };
    assert_eq!(
        fields
            .iter()
            .find(|(key, _)| key == "policy_sha256")
            .unwrap()
            .1,
        sha256(policy.as_bytes()).to_hex()
    );
    policy
}

#[test]
fn information_policy_and_exact_costs_survive_mixed_v1_v3_history_and_source_edits() {
    let f = Fixture::new();
    let first = successful(execute_with_clock(
        &f.record(f.initial, "earlier-action", "hold"),
        &f.root,
        &Clock(200),
    ));
    let first_head = head(&first);
    let old_fields = operation_fields(&f, first_head, 1);
    assert_eq!(
        old_fields[0],
        ("schema".into(), "fmine.decision-operation.v1".into())
    );
    assert_eq!(old_fields[2], ("action".into(), "hold".into()));
    fs::write(&f.model, INFORMATION).unwrap();
    let next = successful(execute_with_clock(
        &f.information(
            first_head,
            "buy-testwork",
            "met_test",
            PolicySelection::ExpectedValue,
        ),
        &f.root,
        &Clock(300),
    ));
    let pin = head(&next);
    assert_eq!(operation_fields(&f, pin, 1), old_fields);
    let policy = retained_policy(&f, pin, 2);
    for expected in [
        "\"nominal_mean\":\"131200000/1\"",
        "\"test_cost\":\"6000000/1\"",
        "\"delay_cost\":\"3800000/1\"",
        "\"total_cost\":\"9800000/1\"",
        "\"nominal_loss_probability\":\"49/100\"",
        "\"signal_id\":\"high\",\"probability\":\"57/100\",\"action_id\":\"build\"",
        "\"signal_id\":\"low\",\"probability\":\"43/100\",\"action_id\":\"hold\"",
        "\"option_claim_id\":",
        "\"card_claim_id\":",
    ] {
        assert!(policy.contains(expected), "missing {expected}: {policy}");
    }
    let before = successful(f.show(Some(pin)));
    assert!(
        before
            .payload_json
            .contains("\"schema\":\"fmine.decision-journal.v2\"")
    );
    assert!(before.payload_json.contains("\"selected_action\":\"hold\""));
    assert!(
        before
            .payload_json
            .contains("\"selected_information_option\":\"met_test\"")
    );
    assert!(before.payload_json.contains("\"n_decisions\":2"));
    assert!(before.rendered_markdown.as_ref().unwrap().contains(&policy));
    fs::write(&f.model, "the original source is no longer available").unwrap();
    let after = successful(f.show(Some(pin)));
    assert_eq!(before.payload_json, after.payload_json);
    assert_eq!(before.rendered_markdown, after.rendered_markdown);
}

#[test]
fn legacy_v2_policy_bytes_reopen_and_accept_compact_v3_choices() {
    let f = Fixture::new();
    let model = INFORMATION.replace(
        "label = \"Metallurgical testwork\"",
        r#"label = "Metallurgical \"testwork\" \\ comparison""#,
    );
    fs::write(&f.model, model).unwrap();
    let saved = successful(execute_with_clock(
        &f.information(
            f.initial,
            "legacy-testwork",
            "met_test",
            PolicySelection::ExpectedValue,
        ),
        &f.root,
        &Clock(200),
    ));
    let policy = retained_policy(&f, head(&saved), 1);
    let fields = operation_fields(&f, head(&saved), 1);
    let mut operation = String::new();
    for (key, value) in fields {
        let (key, value) = match key.as_str() {
            "schema" => (key, "fmine.decision-operation.v2".into()),
            "policy_json" => ("policy_hex".into(), hex(value.as_bytes())),
            "card_json" => ("card_hex".into(), hex(value.as_bytes())),
            _ => (key, value),
        };
        let escaped = value.replace('\\', "\\\\").replace('"', "\\\"");
        operation.push_str(&format!("{key} = \"{escaped}\"\n"));
    }
    assert!(operation.len() <= fmine_io::forecast::MAX_REQUEST_BYTES);
    let journal = Journal::open_read(&f.journal, Some(head(&saved))).unwrap();
    let mut records = journal.records().to_vec();
    drop(journal);
    records[1].entry = Entry::Operation(operation);
    let legacy_head = records[1].chain_head().unwrap();
    let legacy_bytes: Vec<_> = records.iter().flat_map(|r| r.to_line().unwrap()).collect();
    fs::write(&f.journal, &legacy_bytes).unwrap();
    successful(f.show(Some(legacy_head)));
    assert_eq!(retained_policy(&f, legacy_head, 1), policy);
    let legacy_fields = operation_fields(&f, legacy_head, 1);
    let appended = successful(execute_with_clock(
        &f.information(
            legacy_head,
            "compact-testwork",
            "met_test",
            PolicySelection::ExpectedValue,
        ),
        &f.root,
        &Clock(300),
    ));
    let pin = head(&appended);
    assert!(fs::read(&f.journal).unwrap().starts_with(&legacy_bytes));
    assert_eq!(operation_fields(&f, pin, 1), legacy_fields);
    assert_eq!(
        operation_fields(&f, pin, 2)[0],
        ("schema".into(), "fmine.decision-operation.v3".into())
    );
    assert_eq!(retained_policy(&f, pin, 2), policy);
    successful(f.show(Some(pin)));
}

#[test]
fn downside_optimized_selection_retains_its_distinct_policy_and_declared_screen() {
    let f = Fixture::new();
    fs::write(&f.model, CONSTRAINED).unwrap();
    let fixed = successful(execute_with_clock(
        &f.information(
            f.initial,
            "fixed",
            "testwork",
            PolicySelection::ExpectedValue,
        ),
        &f.root,
        &Clock(200),
    ));
    let fields = operation_fields(&f, head(&fixed), 1);
    assert_eq!(
        fields[0],
        ("schema".into(), "fmine.decision-operation.v3".into())
    );
    assert!(fields.iter().any(|(key, _)| key == "card_json"));
    assert!(fields.iter().any(|(key, _)| key == "policy_json"));
    assert!(!fields.iter().any(|(key, _)| key == "card_hex"));
    assert!(!fields.iter().any(|(key, _)| key == "policy_hex"));
    let fixed_policy = retained_policy(&f, head(&fixed), 1);
    assert!(fixed_policy.contains("\"nominal_mean\":\"34/1\""));
    assert!(
        fixed_policy
            .contains("\"signal_id\":\"medium\",\"probability\":\"19/100\",\"action_id\":\"act\"")
    );
    let optimized = successful(execute_with_clock(
        &f.information(
            head(&fixed),
            "constrained",
            "testwork",
            PolicySelection::DownsideOptimized,
        ),
        &f.root,
        &Clock(300),
    ));
    let policy = retained_policy(&f, head(&optimized), 2);
    for expected in [
        "\"policy_selection\":\"downside-optimized\"",
        "\"nominal_mean\":\"57/2\"",
        "\"total_cost\":\"1/1\"",
        "\"screen\":\"nominal\"",
        "\"nominal_breach_probability\":\"1/100\"",
        "\"screened_breach_probability\":\"1/100\"",
        "\"signal_id\":\"medium\",\"action_id\":\"hold\"",
        "\"signal_id\":\"strong\",\"action_id\":\"act\"",
    ] {
        assert!(policy.contains(expected), "missing {expected}: {policy}");
    }
    assert_ne!(policy, fixed_policy);
    successful(f.show(Some(head(&optimized))));
}

#[test]
fn expected_value_selection_keeps_posterior_ties_and_unreachable_signals() {
    let f = Fixture::new();
    let tied = INFORMATION
        .replace("300000000.0000", "0.0000")
        .replace("-200000000.0000", "0.0000");
    let tied = format!(
        "{tied}\n[[signal]]\noption = \"met_test\"\nid = \"unreachable\"\n\n[[likelihood]]\noption = \"met_test\"\nstate = \"high\"\nsignal = \"unreachable\"\nprobability = \"0/1\"\n\n[[likelihood]]\noption = \"met_test\"\nstate = \"low\"\nsignal = \"unreachable\"\nprobability = \"0/1\"\n"
    );
    fs::write(&f.model, tied).unwrap();
    let saved = successful(execute_with_clock(
        &f.information(
            f.initial,
            "tied-test",
            "met_test",
            PolicySelection::ExpectedValue,
        ),
        &f.root,
        &Clock(200),
    ));
    let policy = retained_policy(&f, head(&saved), 1);
    assert!(policy.contains("\"posterior_best_actions\":[\"build\",\"hold\"]"));
    assert!(policy.contains("\"nominal_mean\":\"-9800000/1\""));
    assert!(policy.contains("\"signal_id\":\"unreachable\",\"probability\":\"0/1\",\"action_id\":null,\"posterior_best_actions\":[]"));
}

#[test]
fn missing_options_unavailable_policies_and_corrupt_policy_artifacts_refuse() {
    let f = Fixture::new();
    let command = f.information(
        f.initial,
        "test",
        "met_test",
        PolicySelection::ExpectedValue,
    );
    f.refuses_without_append(&command, 200, "FMINE-REF-107");
    fs::write(&f.model, INFORMATION).unwrap();
    f.refuses_without_append(
        &f.information(
            f.initial,
            "missing",
            "unknown",
            PolicySelection::ExpectedValue,
        ),
        200,
        "FMINE-REF-107",
    );
    f.refuses_without_append(
        &f.information(
            f.initial,
            "unconfigured",
            "met_test",
            PolicySelection::DownsideOptimized,
        ),
        200,
        "FMINE-REF-107",
    );
    let no_feasible = CONSTRAINED
        .replace("floor = \"-10\"", "floor = \"0\"")
        .replace(
            "max_breach_probability = \"1/20\"",
            "max_breach_probability = \"0/1\"",
        );
    fs::write(&f.model, no_feasible).unwrap();
    f.refuses_without_append(
        &f.information(
            f.initial,
            "infeasible",
            "testwork",
            PolicySelection::DownsideOptimized,
        ),
        200,
        "FMINE-REF-107",
    );
    fs::write(&f.model, INFORMATION).unwrap();
    let saved = successful(execute_with_clock(&command, &f.root, &Clock(200)));
    let journal = Journal::open_read(&f.journal, Some(head(&saved))).unwrap();
    let mut records = journal.records().to_vec();
    drop(journal);
    let Entry::Operation(operation) = &mut records[1].entry else {
        panic!("operation")
    };
    let parsed = parse_subset(operation).unwrap();
    let TomlValue::Str(old) = &parsed.top_get("policy_sha256").unwrap().value else {
        panic!("digest")
    };
    *operation = operation.replace(old, &"a".repeat(64));
    let path = f.root.join("corrupt-policy.journal");
    let bytes: Vec<_> = records.iter().flat_map(|r| r.to_line().unwrap()).collect();
    fs::write(&path, bytes).unwrap();
    let result = execute_with_clock(
        &Command::Show {
            journal: path,
            head: Some(records[1].chain_head().unwrap()),
        },
        &f.root,
        &NoClock,
    );
    assert_eq!(result.refusal.unwrap().code, "FMINE-REF-505");
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
    let initial = "0".repeat(64);
    let base = vec![
        "decide",
        "journal",
        "record",
        "--journal",
        "history",
        "--expected-head",
        &initial,
        "--model",
        "model.toml",
        "--decision",
        "decision",
        "--reason",
        "buy the declared information policy",
        "--code-revision",
        REVISION,
        "--trust-host-clock",
    ];
    let mut information = base.clone();
    information.extend([
        "--information-option",
        "met_test",
        "--policy",
        "expected-value",
    ]);
    let (parsed, _, _) = parse(&information).unwrap();
    assert!(matches!(
        parsed,
        CliCommand::DecisionJournal(Command::Record {
            choice: Choice::InformationOption {
                policy: PolicySelection::ExpectedValue,
                ..
            },
            ..
        })
    ));
    for choices in [
        vec![
            "--action",
            "build",
            "--information-option",
            "met_test",
            "--policy",
            "expected-value",
        ],
        vec!["--information-option", "met_test"],
        vec![
            "--information-option",
            "met_test",
            "--policy",
            "choose-for-me",
        ],
        vec!["--action", "build", "--policy", "expected-value"],
    ] {
        let mut args = base.clone();
        args.extend(choices);
        assert!(parse(&args).is_err());
    }
    let registry = include_str!("../../../registries/verbs.toml");
    let docs = successful(execute_command(&CliCommand::RobotDocs, &root));
    for verb in [
        "decide journal init",
        "decide journal record",
        "decide journal show",
        "decide journal assess",
    ] {
        assert!(registry.contains(&format!("path = \"{verb}\"")));
        assert!(docs.payload_json.contains(verb));
    }
}

fn json_field<'a>(value: &'a Json, keys: &[&str]) -> &'a Json {
    keys.iter().fold(value, |value, key| {
        value
            .get(key)
            .unwrap_or_else(|| panic!("missing {key}: {value:?}"))
    })
}

#[test]
fn feed_choices_replay_the_original_comparison_and_keep_refusals_atomic() {
    let f = Fixture::new();
    fs::write(&f.model, SHIFT_FEED).unwrap();
    let (envelope, _) = fmine_cli::invoke(
        &[
            "decide".into(),
            "recovery-bounds".into(),
            f.model.display().to_string(),
        ],
        &f.root,
    );
    let comparison = successful(envelope);
    let json = parse_json(comparison.payload_json.as_bytes()).unwrap();
    let original_card = json.get("card").unwrap();
    for action in ["hold", "hold-back:", "hold-back:no-such-lot"] {
        f.refuses_without_append(
            &f.record(f.initial, "unknown", action),
            200,
            "FMINE-REF-107",
        );
    }
    f.refuses_without_append(
        &f.information(
            f.initial,
            "bad-policy",
            "base-stockpile",
            PolicySelection::ExpectedValue,
        ),
        200,
        "FMINE-REF-506",
    );
    let mut pin = f.initial;
    let actions = [
        "all-feed",
        "hold-back:high-grade-pocket",
        "hold-back:base-stockpile",
    ];
    for (i, action) in actions.iter().enumerate() {
        let recorded = successful(execute_with_clock(
            &f.record(pin, &format!("shift-{i}"), action),
            &f.root,
            &Clock(210 + 100 * i as i64),
        ));
        pin = head(&recorded);
    }
    fs::write(
        &f.model,
        include_str!("../../../tests/decisions/untested_ore.toml"),
    )
    .unwrap();
    f.refuses_without_append(&f.record(pin, "no-lots", "all-feed"), 600, "FMINE-REF-107");
    fs::write(
        &f.model,
        format!("{SHIFT_FEED}\n[[interaction]]\nkind = \"clay\"\n"),
    )
    .unwrap();
    f.refuses_without_append(&f.record(pin, "invalid", "all-feed"), 600, "FMINE-REF-103");
    let shown = successful(f.show(Some(pin)));
    let replay = parse_json(shown.payload_json.as_bytes()).unwrap();
    let rows = replay.get("decisions").unwrap().as_array().unwrap();
    assert_eq!(rows.len(), actions.len());
    for (row, action) in rows.iter().zip(actions) {
        let row = row.get("snapshot").unwrap();
        assert_eq!(row.get("selected_action").unwrap().as_str(), Some(action));
        assert_eq!(
            row.get("claim_id").unwrap(),
            json_field(original_card, &["result", "id"])
        );
        assert_eq!(
            json_field(row, &["model", "content"]).as_str(),
            Some(SHIFT_FEED)
        );
        let bytes = json_field(row, &["artifact", "content"]).as_str().unwrap();
        assert!(
            comparison
                .payload_json
                .ends_with(&format!(",\"card\":{bytes}}}"))
        );
        assert_eq!(&parse_json(bytes.as_bytes()).unwrap(), original_card);
        assert_eq!(
            json_field(row, &["artifact", "sha256"]).as_str().unwrap(),
            sha256(bytes.as_bytes()).to_hex()
        );
    }
}

struct Predictions {
    root: PathBuf,
    journal: PathBuf,
    head: Digest32,
    requests: usize,
}

#[test]
fn compact_feed_record_keeps_an_old_prefix_and_refuses_an_over_capacity_shift() {
    let f = Fixture::new();
    let first = successful(execute_with_clock(
        &f.record(f.initial, "old", "hold"),
        &f.root,
        &Clock(200),
    ));
    let first_head = head(&first);
    let first_bytes = fs::read(&f.journal).unwrap();
    let (header, rows) = SHIFT_FEED.split_once("[[parcel]]").unwrap();
    let (parcels, rest) = rows.split_once("[[recovery]]").unwrap();
    let lots: Vec<_> = parcels.split("[[parcel]]").collect();
    let shift = |count: usize| {
        let mut text = header.to_owned();
        for i in 0..count {
            text.push_str("[[parcel]]");
            text.push_str(&lots[i % lots.len()].replacen("id = \"", &format!("id = \"{i}-"), 1));
        }
        format!("{text}[[recovery]]{rest}")
    };
    let eight = shift(8);
    fs::write(&f.model, &eight).unwrap();
    let saved = successful(execute_with_clock(
        &f.record(first_head, "eight-lots", "hold-back:2-high-grade-pocket"),
        &f.root,
        &Clock(300),
    ));
    let pin = head(&saved);
    assert!(fs::read(&f.journal).unwrap().starts_with(&first_bytes));
    assert_eq!(
        operation_fields(&f, pin, 1)[0].1,
        "fmine.decision-operation.v1"
    );
    let fields = operation_fields(&f, pin, 2);
    assert_eq!(fields[0].1, "fmine.decision-operation.v5");
    let card = &fields.iter().find(|(key, _)| key == "card_json").unwrap().1;
    assert!(card.len() * 2 > fmine_io::forecast::MAX_REQUEST_BYTES);
    assert!(!fields.iter().any(|(key, _)| key == "card_hex"));
    fs::write(&f.model, shift(30)).unwrap();
    f.refuses_without_append(
        &f.record(pin, "too-large", "all-feed"),
        400,
        "FMINE-REF-107",
    );
    let shown = successful(f.show(Some(pin)));
    let contents = parse_json(shown.payload_json.as_bytes()).unwrap();
    let row = &contents.get("decisions").unwrap().as_array().unwrap()[1];
    assert_eq!(
        json_field(row, &["snapshot", "model", "content"]).as_str(),
        Some(eight.as_str())
    );
    assert_eq!(
        json_field(row, &["snapshot", "artifact", "content"]).as_str(),
        Some(card.as_str())
    );
}

impl Predictions {
    fn new(root: &Path, name: &str) -> Self {
        let journal = root.join(format!("{name}.forecast"));
        let created = successful(forecast_with_clock(
            &ForecastCommand::Init {
                journal: journal.clone(),
                cell_id: name.into(),
                clock_skew_ns: 10,
            },
            root,
            &Clock(100),
        ));
        let mut predictions = Self {
            root: root.to_path_buf(),
            journal,
            head: head(&created),
            requests: 0,
        };
        for (index, alias) in ["f1", "f2", "f3"].iter().enumerate() {
            predictions.append(&format!(
                "[[forecast]]\nid = \"{alias}\"\nmodel_key = \"synthetic:{alias}\"\noutcome_due = \"1970-01-01T00:00:01Z\"\nkind = \"weighted_empirical\"\nsamples = \"0,4\"\nweights = \"1/4,3/4\"\n"), 200 + index as i64 * 100);
        }
        for (at, name, members) in [(500, "primary", "f1,f2"), (600, "secondary", "f3")] {
            predictions.append(&format!(
                "[[cohort]]\nname = \"{name}\"\nmembers = \"{members}\"\nresolution_rule = \"Invented complete observations\"\neligible_sources = \"synthetic:issuer\"\nalpha = \"0.2\"\noutcome_lo = \"0\"\noutcome_hi = \"4\"\n"), at);
        }
        predictions
    }

    fn append(&mut self, text: &str, at: i64) -> Json {
        let request = self
            .journal
            .with_extension(format!("request{}.toml", self.requests));
        self.requests += 1;
        fs::write(&request, text).unwrap();
        let command = match text.lines().next().unwrap() {
            "[[forecast]]" => ForecastCommand::Register {
                journal: self.journal.clone(),
                head: self.head,
                request,
            },
            "[[cohort]]" => ForecastCommand::Freeze {
                journal: self.journal.clone(),
                head: self.head,
                request,
            },
            "[[outcome]]" => ForecastCommand::Outcome {
                journal: self.journal.clone(),
                head: self.head,
                request,
            },
            _ => panic!("unknown synthetic request"),
        };
        self.execute(command, at)
    }

    fn execute(&mut self, command: ForecastCommand, at: i64) -> Json {
        let envelope = successful(forecast_with_clock(&command, &self.root, &Clock(at)));
        self.head = head(&envelope);
        parse_json(envelope.payload_json.as_bytes()).unwrap()
    }

    fn selection(&self, cohorts: &[&str]) -> ForecastSelection {
        ForecastSelection {
            journal: self.journal.clone(),
            head: self.head,
            cohorts: cohorts.iter().map(|s| (*s).into()).collect(),
        }
    }

    fn publish(&mut self) {
        self.append(&format!(
            "[[outcome]]\nforecast = \"f1\"\nkind = \"published\"\navailable_at = \"1970-01-01T00:00:02Z\"\nvalue = \"1\"\nsource = \"synthetic:issuer\"\nvintage = \"invented-original\"\nlocator = \"Synthetic!A1\"\nsha256 = \"{}\"\n", "ab".repeat(32)), 2_000_000_020);
        self.append("[[outcome]]\nforecast = \"f2\"\nkind = \"missing\"\navailable_at = \"1970-01-01T00:00:02Z\"\nreason = \"not_published\"\n", 2_000_000_040);
    }

    fn assess(&mut self, cohort: &str, cut: &str, at: i64) -> (Digest32, Json) {
        let report = self.execute(
            ForecastCommand::Assess {
                journal: self.journal.clone(),
                head: self.head,
                cohort: cohort.into(),
                as_of: cut.into(),
                selector: "first-published".into(),
                expected_digest: None,
            },
            at,
        );
        let digest = Digest32(
            fmine_calib::record::parse_hex32(
                json_field(&report, &["report", "assessment", "digest"])
                    .as_str()
                    .unwrap(),
            )
            .unwrap(),
        );
        (digest, report)
    }
}

fn linked_record(f: &Fixture, selection: ForecastSelection) -> Command {
    let mut command = f.record(f.initial, "prospective-choice", "build");
    let Command::Record { forecasts, .. } = &mut command else {
        panic!("record")
    };
    *forecasts = Some(selection);
    command
}

fn decision_review(
    f: &Fixture,
    decision_head: Digest32,
    p: &Predictions,
    assessments: Vec<Digest32>,
) -> Command {
    Command::Assess {
        journal: f.journal.clone(),
        head: decision_head,
        decision_id: "prospective-choice".into(),
        forecast_journal: p.journal.clone(),
        forecast_head: p.head,
        assessments,
    }
}

#[test]
fn a_feed_decision_keeps_its_frozen_forecasts_and_reads_the_saved_assessment() {
    let f = Fixture::new();
    let mut p = Predictions::new(&f.root, "shift-forecasts");
    fs::write(&f.model, SHIFT_FEED).unwrap();
    let mut command = linked_record(&f, p.selection(&["primary"]));
    let Command::Record { choice, .. } = &mut command else {
        panic!("record")
    };
    *choice = Choice::ActNow("hold-back:high-grade-pocket".into());
    let recorded = successful(execute_with_clock(&command, &f.root, &Clock(1_000)));
    let pinned_decision = head(&recorded);
    let frozen_forecasts = p.head;
    p.publish();
    let (assessment, _) = p.assess("primary", "1970-01-01T00:00:03Z", 4_000_000_000);
    fs::write(&f.model, MODEL).unwrap();
    let before = (fs::read(&f.journal).unwrap(), fs::read(&p.journal).unwrap());
    let review = successful(execute_with_clock(
        &decision_review(&f, pinned_decision, &p, vec![assessment]),
        &f.root,
        &NoClock,
    ));
    let result = parse_json(review.payload_json.as_bytes()).unwrap();
    assert_eq!(
        json_field(&result, &["decision", "selected_action"]).as_str(),
        Some("hold-back:high-grade-pocket")
    );
    assert_eq!(
        json_field(&result, &["decision", "model", "content"]).as_str(),
        Some(SHIFT_FEED)
    );
    assert!(review.payload_json.contains(&frozen_forecasts.to_hex()));
    assert!(review.payload_json.contains(&assessment.to_hex()));
    assert_eq!(
        json_field(&result, &["n_selected_assessments"]).u64(),
        Some(1)
    );
    assert_eq!(
        before,
        (fs::read(&f.journal).unwrap(), fs::read(&p.journal).unwrap())
    );
}

#[test]
fn decision_review_lists_selected_verdicts_in_saved_order_before_the_reports() {
    let f = Fixture::new();
    let mut p = Predictions::new(&f.root, "shift-predictions");
    let decision = successful(execute_with_clock(
        &linked_record(&f, p.selection(&["primary", "secondary"])),
        &f.root,
        &Clock(1_000),
    ));
    let (pending, _) = p.assess("primary", "1970-01-01T00:00:01Z", 1_500_000_000);
    p.publish();
    let (updated, _) = p.assess("primary", "1970-01-01T00:00:03Z", 4_000_000_000);
    let (unused, _) = p.assess("secondary", "1970-01-01T00:00:03Z", 4_000_000_020);
    let (duplicate, _) = p.assess("primary", "1970-01-01T00:00:01Z", 4_000_000_040);
    assert_eq!(pending, duplicate);
    let bytes = (fs::read(&f.journal).unwrap(), fs::read(&p.journal).unwrap());
    let reviewed = successful(execute_with_clock(
        &decision_review(&f, head(&decision), &p, vec![updated, pending]),
        &f.root,
        &NoClock,
    ));
    let markdown = reviewed.rendered_markdown.unwrap();
    let (overview, _) = markdown
        .split_once("\n## primary\n")
        .expect("overview before details");
    let rows: Vec<_> = overview
        .lines()
        .filter(|line| line.starts_with("| primary |"))
        .collect();
    assert_eq!(rows.len(), 2);
    assert!(rows[0].contains(&format!("first-published | unscored | 0 / 2 | `{pending}`")));
    assert!(rows[1].contains(&format!(
        "first-published | descriptive | 1 / 2 | `{updated}`"
    )));
    assert!(!overview.contains(&unused.to_hex()));
    assert!(
        markdown
            .split_once("\n## secondary\n")
            .unwrap()
            .1
            .contains("no_assessment_selected")
    );
    assert_eq!(
        bytes,
        (fs::read(&f.journal).unwrap(), fs::read(&p.journal).unwrap())
    );
}

#[test]
fn decision_forecasts_retain_complete_members_and_retrieve_saved_weighted_scores() {
    struct LockedForecast<'a>(&'a Predictions);
    impl WallClock for LockedForecast<'_> {
        fn now_ns(&self) -> Result<i64, ClockError> {
            assert!(Journal::open_write(&self.0.journal, self.0.head).is_err());
            Ok(1_000)
        }
    }
    let f = Fixture::new();
    let mut p = Predictions::new(&f.root, "predictions");
    let forecast_prefix = p.head;
    let recorded = successful(execute_with_clock(
        &linked_record(&f, p.selection(&["secondary", "primary"])),
        &f.root,
        &LockedForecast(&p),
    ));
    let decision_head = head(&recorded);
    assert_eq!(
        operation_fields(&f, decision_head, 1)[0].1,
        "fmine.decision-operation.v4"
    );
    let shown = successful(f.show(Some(decision_head)));
    assert!(shown.payload_json.contains(&forecast_prefix.to_hex()));
    assert!(shown.payload_json.contains("fmine.decision-journal.v3"));
    let decision_json = parse_json(shown.payload_json.as_bytes()).unwrap();
    let snapshot = json_field(&decision_json, &["decisions"])
        .as_array()
        .unwrap()[0]
        .get("snapshot")
        .unwrap();
    let cohorts = json_field(snapshot, &["forecast_commitments", "cohorts"])
        .as_array()
        .unwrap();
    assert_eq!(cohorts.len(), 2);
    assert_eq!(
        cohorts[0].get("members").unwrap().as_array().unwrap().len(),
        2
    );
    assert_eq!(
        cohorts[1].get("members").unwrap().as_array().unwrap().len(),
        1
    );

    p.publish();
    let (assessment, original) = p.assess("primary", "1970-01-01T00:00:03Z", 4_000_000_000);
    fs::write(&f.model, "the reviewed model is no longer present here").unwrap();
    let decision_bytes = fs::read(&f.journal).unwrap();
    let forecast_bytes = fs::read(&p.journal).unwrap();
    let command = decision_review(&f, decision_head, &p, vec![assessment]);
    let reviewed = successful(execute_with_clock(&command, &f.root, &NoClock));
    assert_eq!(fs::read(&f.journal).unwrap(), decision_bytes);
    assert_eq!(fs::read(&p.journal).unwrap(), forecast_bytes);
    let review = parse_json(reviewed.payload_json.as_bytes()).unwrap();
    assert_eq!(review.get("read_only"), Some(&Json::Bool(true)));
    assert_eq!(review.get("decision"), Some(snapshot));
    assert_eq!(
        review
            .get("n_cohorts_without_selected_assessment")
            .unwrap()
            .u64(),
        Some(1)
    );
    let rows = review.get("forecasts").unwrap().as_array().unwrap();
    assert_eq!(rows.len(), 2);
    let saved = &rows[0].get("assessments").unwrap().as_array().unwrap()[0];
    assert_eq!(saved, original.get("report").unwrap());
    let scores = json_field(saved, &["assessment", "summary"]);
    assert_eq!(scores.get("n_members").unwrap().u64(), Some(2));
    assert_eq!(scores.get("n_resolved").unwrap().u64(), Some(1));
    assert_eq!(scores.get("n_unresolved").unwrap().u64(), Some(1));
    // Independent discrete CRPS: .25*1 + .75*3 - .25*.75*4 = 1.75.
    assert_eq!(
        json_field(scores, &["crps", "observed_mean"]).f64(),
        Some(1.75)
    );
    assert_eq!(
        json_field(scores, &["crps", "whole_cohort", "lower"]).f64(),
        Some(1.0)
    );
    assert_eq!(
        json_field(scores, &["crps", "whole_cohort", "upper"]).f64(),
        Some(2.0)
    );
    assert_eq!(
        rows[1].get("status").unwrap().as_str(),
        Some("no_assessment_selected")
    );
    assert!(reviewed.rendered_markdown.unwrap().contains("secondary"));
    assert_eq!(
        successful(f.show(Some(decision_head))).payload_json,
        shown.payload_json
    );

    let assert_known_scores = |report: &Json| {
        let summary = json_field(report, &["assessment", "summary"]);
        assert_eq!(summary.get("n_members").unwrap().u64(), Some(2));
        assert_eq!(summary.get("n_resolved").unwrap().u64(), Some(1));
        assert_eq!(summary.get("n_unresolved").unwrap().u64(), Some(1));
        assert_eq!(
            json_field(summary, &["crps", "observed_mean"]).f64(),
            Some(1.75)
        );
        assert_eq!(
            json_field(summary, &["crps", "whole_cohort", "lower"]).f64(),
            Some(1.0)
        );
        assert_eq!(
            json_field(summary, &["crps", "whole_cohort", "upper"]).f64(),
            Some(2.0)
        );
    };
    for format in ["json", "markdown"] {
        let output = Process::new(binary())
            .args(["decide", "journal", "assess", "--journal"])
            .arg(&f.journal)
            .args([
                "--head",
                &decision_head.to_hex(),
                "--decision",
                "prospective-choice",
                "--forecast-journal",
            ])
            .arg(&p.journal)
            .args([
                "--forecast-head",
                &p.head.to_hex(),
                "--assessments",
                &assessment.to_hex(),
                "--format",
                format,
                "--workspace",
            ])
            .arg(&f.root)
            .output()
            .expect("execute actual decision forecast review");
        assert_eq!(
            output.status.code(),
            Some(0),
            "{format}: {}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(fs::read(&f.journal).unwrap(), decision_bytes);
        assert_eq!(fs::read(&p.journal).unwrap(), forecast_bytes);
        if format == "json" {
            let envelope = parse_json(&output.stdout).unwrap();
            let payload = envelope.get("payload").unwrap();
            assert_eq!(payload.get("read_only"), Some(&Json::Bool(true)));
            assert_eq!(
                json_field(payload, &["decision", "selected_action"]).as_str(),
                Some("build")
            );
            assert_eq!(payload.get("n_committed_cohorts").unwrap().u64(), Some(2));
            assert_eq!(
                payload
                    .get("n_cohorts_without_selected_assessment")
                    .unwrap()
                    .u64(),
                Some(1)
            );
            let rows = payload.get("forecasts").unwrap().as_array().unwrap();
            assert_eq!(rows.len(), 2);
            assert_known_scores(&rows[0].get("assessments").unwrap().as_array().unwrap()[0]);
            assert_eq!(
                json_field(&rows[1], &["cohort", "name"]).as_str(),
                Some("secondary")
            );
            assert_eq!(
                rows[1].get("status").unwrap().as_str(),
                Some("no_assessment_selected")
            );
            assert!(
                rows[1]
                    .get("assessments")
                    .unwrap()
                    .as_array()
                    .unwrap()
                    .is_empty()
            );
        } else {
            let markdown = String::from_utf8(output.stdout).unwrap();
            assert!(markdown.contains("# Forecast outcomes for decision prospective-choice"));
            assert!(markdown.contains(&format!("| primary | 1970-01-01T00:00:03Z | first-published | descriptive | 1 / 2 | `{assessment}` | persisted_checkpoint |")));
            let secondary = markdown.split_once("\n## secondary\n").unwrap().1;
            assert!(secondary.contains("Status: `no_assessment_selected`."));
            let report = markdown
                .split_once("```json\n")
                .unwrap()
                .1
                .split_once("\n```")
                .unwrap()
                .0;
            assert_known_scores(&parse_json(report.as_bytes()).unwrap());
        }
    }
}

#[test]
fn decision_forecast_admission_enforces_clock_bounds_frozen_members_and_current_head() {
    let f = Fixture::new();
    let mut p = Predictions::new(&f.root, "prospective");
    let selection = p.selection(&["primary"]);
    let original = fs::read(&f.journal).unwrap();
    for at in [500, 620, 999_999_990, 1_000_000_000] {
        let result = execute_with_clock(&linked_record(&f, selection.clone()), &f.root, &Clock(at));
        assert_ne!(result.exit_code, 0, "{at}: {}", result.to_json());
        assert_eq!(fs::read(&f.journal).unwrap(), original);
    }
    for cohorts in [vec![], vec!["primary", "primary"], vec!["unknown"]] {
        let result = execute_with_clock(
            &linked_record(&f, p.selection(&cohorts)),
            &f.root,
            &Clock(1_000),
        );
        assert_ne!(result.exit_code, 0);
        assert_eq!(fs::read(&f.journal).unwrap(), original);
    }
    p.append("[[forecast]]\nid = \"later\"\nmodel_key = \"synthetic:later\"\noutcome_due = \"1970-01-01T00:00:01Z\"\nkind = \"empirical\"\nsamples = \"1\"\n", 700);
    let stale = execute_with_clock(&linked_record(&f, selection), &f.root, &NoClock);
    assert_ne!(stale.exit_code, 0);
    assert_eq!(fs::read(&f.journal).unwrap(), original);
    p.publish();
    let resolved = execute_with_clock(
        &linked_record(&f, p.selection(&["primary"])),
        &f.root,
        &NoClock,
    );
    assert_ne!(resolved.exit_code, 0);
    assert!(
        resolved
            .refusal
            .unwrap()
            .what
            .contains("recorded member outcome")
    );
    assert_eq!(fs::read(&f.journal).unwrap(), original);
}

#[test]
fn decision_review_rejects_uncommitted_unsaved_duplicate_and_different_history_scores() {
    let f = Fixture::new();
    let mut p = Predictions::new(&f.root, "linked");
    let recorded = successful(execute_with_clock(
        &linked_record(&f, p.selection(&["primary"])),
        &f.root,
        &Clock(1_000),
    ));
    let pin = head(&recorded);
    p.publish();
    let (primary, _) = p.assess("primary", "1970-01-01T00:00:03Z", 4_000_000_000);
    let (secondary, _) = p.assess("secondary", "1970-01-01T00:00:03Z", 4_000_000_020);
    let bytes = fs::read(&f.journal).unwrap();
    for digests in [
        vec![secondary],
        vec![Digest32([9; 32])],
        vec![primary, primary],
        vec![],
    ] {
        let rejected =
            execute_with_clock(&decision_review(&f, pin, &p, digests), &f.root, &NoClock);
        assert_ne!(rejected.exit_code, 0);
        assert_eq!(fs::read(&f.journal).unwrap(), bytes);
    }
    let other = Predictions::new(&f.root, "same-labels-other-history");
    let rejected = execute_with_clock(
        &decision_review(&f, pin, &other, vec![primary]),
        &f.root,
        &NoClock,
    );
    assert_ne!(rejected.exit_code, 0);
    assert!(rejected.refusal.unwrap().what.contains("prefix"));
    successful(execute_with_clock(
        &decision_review(&f, pin, &p, vec![primary]),
        &f.root,
        &NoClock,
    ));
}

#[test]
fn decision_review_requires_a_persisted_score_digest_and_original_complete_membership() {
    let f = Fixture::new();
    let mut p = Predictions::new(&f.root, "integrity");
    let recorded = successful(execute_with_clock(
        &linked_record(&f, p.selection(&["primary"])),
        &f.root,
        &Clock(1_000),
    ));
    let pin = head(&recorded);
    p.publish();
    let (assessment, _) = p.assess("primary", "1970-01-01T00:00:03Z", 4_000_000_000);

    // Legacy checkpoints replay, but an omitted original result digest does not certify a saved score.
    let mut forecast = Journal::open_write(&p.journal, p.head).unwrap();
    forecast.append("[[assessment]]\ncohort = \"primary\"\nas_of = \"1970-01-01T00:00:05Z\"\nselector = \"first-published\"\n", &Clock(6_000_000_000), |_, _| Ok(())).unwrap();
    p.head = forecast.head();
    drop(forecast);
    let inspected = successful(forecast_with_clock(
        &ForecastCommand::Inspect {
            journal: p.journal.clone(),
            head: Some(p.head),
        },
        &f.root,
        &NoClock,
    ));
    let inspected = parse_json(inspected.payload_json.as_bytes()).unwrap();
    let last = inspected
        .get("assessments")
        .unwrap()
        .as_array()
        .unwrap()
        .last()
        .unwrap();
    let unbound = Digest32(
        fmine_calib::record::parse_hex32(last.get("digest").unwrap().as_str().unwrap()).unwrap(),
    );
    let rejected = execute_with_clock(
        &decision_review(&f, pin, &p, vec![unbound]),
        &f.root,
        &NoClock,
    );
    assert_ne!(rejected.exit_code, 0);
    assert!(
        rejected
            .refusal
            .unwrap()
            .what
            .contains("persisted result digest")
    );

    // Even a separately pinned, rehashed decision copy cannot replace its committed member population.
    let fields = operation_fields(&f, pin, 1);
    let mut edited = String::new();
    for (key, mut value) in fields {
        if key == "forecast_commitments_hex" {
            let bytes = (0..value.len())
                .step_by(2)
                .map(|index| u8::from_str_radix(&value[index..index + 2], 16).unwrap())
                .collect::<Vec<_>>();
            let text = String::from_utf8(bytes).unwrap();
            let members = text
                .lines()
                .find(|line| line.starts_with("members = "))
                .unwrap();
            let member = members
                .strip_prefix("members = \"")
                .unwrap()
                .split(',')
                .next()
                .unwrap();
            value = hex(text
                .replace(members, &format!("members = \"{member}\""))
                .as_bytes());
        }
        let escaped = value.replace('\\', "\\\\").replace('"', "\\\"");
        edited.push_str(&format!("{key} = \"{escaped}\"\n"));
    }
    let source = Journal::open_read(&f.journal, Some(pin)).unwrap();
    let mut records = source.records().to_vec();
    drop(source);
    records[1].entry = Entry::Operation(edited);
    let altered_pin = records[1].chain_head().unwrap();
    let altered = f.root.join("altered-decision.journal");
    fs::write(
        &altered,
        records
            .iter()
            .flat_map(|record| record.to_line().unwrap())
            .collect::<Vec<_>>(),
    )
    .unwrap();
    let mut command = decision_review(&f, pin, &p, vec![assessment]);
    let Command::Assess { journal, head, .. } = &mut command else {
        panic!("assess")
    };
    *journal = altered;
    *head = altered_pin;
    let rejected = execute_with_clock(&command, &f.root, &NoClock);
    assert_ne!(rejected.exit_code, 0);
    assert!(
        rejected
            .refusal
            .unwrap()
            .what
            .contains("complete membership")
    );
    successful(execute_with_clock(
        &decision_review(&f, pin, &p, vec![assessment]),
        &f.root,
        &NoClock,
    ));
}

#[test]
fn linked_information_choice_keeps_old_decision_bytes_and_original_policy() {
    let f = Fixture::new();
    let p = Predictions::new(&f.root, "information");
    let old = successful(execute_with_clock(
        &f.record(f.initial, "original-action", "hold"),
        &f.root,
        &Clock(200),
    ));
    let old_bytes = fs::read(&f.journal).unwrap();
    fs::write(&f.model, INFORMATION).unwrap();
    let mut command = f.information(
        head(&old),
        "buy-testwork",
        "met_test",
        PolicySelection::ExpectedValue,
    );
    let Command::Record { forecasts, .. } = &mut command else {
        panic!("record")
    };
    *forecasts = Some(p.selection(&["primary"]));
    let saved = successful(execute_with_clock(&command, &f.root, &Clock(1_000)));
    assert!(fs::read(&f.journal).unwrap().starts_with(&old_bytes));
    assert_eq!(
        operation_fields(&f, head(&saved), 1)[0].1,
        "fmine.decision-operation.v1"
    );
    assert_eq!(
        operation_fields(&f, head(&saved), 2)[0].1,
        "fmine.decision-operation.v4"
    );
    let policy = retained_policy(&f, head(&saved), 2);
    assert!(policy.contains("\"nominal_mean\":\"131200000/1\""));
    assert!(policy.contains("\"total_cost\":\"9800000/1\""));
    successful(f.show(Some(head(&saved))));
    let mut review = decision_review(&f, head(&saved), &p, vec![Digest32([9; 32])]);
    let Command::Assess { decision_id, .. } = &mut review else {
        panic!("assess")
    };
    *decision_id = "original-action".into();
    let refusal = execute_with_clock(&review, &f.root, &NoClock);
    assert_ne!(refusal.exit_code, 0);
    assert!(
        refusal
            .refusal
            .unwrap()
            .what
            .contains("cannot be attached after recording")
    );
}

#[test]
fn decision_forecast_cli_requires_complete_pins_and_read_only_review() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let pin = "ab".repeat(32);
    let base = [
        "decide",
        "journal",
        "assess",
        "--journal",
        "choices",
        "--head",
        &pin,
        "--decision",
        "choice",
        "--forecast-journal",
        "predictions",
        "--forecast-head",
        &pin,
        "--assessments",
        &pin,
    ];
    let args = base.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>();
    let (command, _, _) = parse_args(&args, &root).unwrap();
    assert!(matches!(
        command,
        CliCommand::DecisionJournal(Command::Assess { .. })
    ));
    for flag in ["--head", "--forecast-head", "--assessments"] {
        let mut omitted = args.clone();
        let position = omitted.iter().position(|value| value == flag).unwrap();
        omitted.drain(position..position + 2);
        assert!(parse_args(&omitted, &root).is_err());
    }
    let mut clock = args.clone();
    clock.push("--trust-host-clock".into());
    assert!(parse_args(&clock, &root).is_err());
    let record = [
        "decide",
        "journal",
        "record",
        "--journal",
        "choices",
        "--expected-head",
        &pin,
        "--model",
        "model.toml",
        "--decision",
        "choice",
        "--action",
        "hold",
        "--reason",
        "why",
        "--code-revision",
        REVISION,
        "--trust-host-clock",
        "--forecast-journal",
        "predictions",
        "--forecast-head",
        &pin,
        "--forecast-cohorts",
        "primary,secondary",
    ];
    let args = record.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>();
    assert!(matches!(
        parse_args(&args, &root).unwrap().0,
        CliCommand::DecisionJournal(Command::Record {
            forecasts: Some(_),
            ..
        })
    ));
    for flag in [
        "--forecast-head",
        "--forecast-cohorts",
        "--forecast-journal",
    ] {
        let mut omitted = args.clone();
        let position = omitted.iter().position(|value| value == flag).unwrap();
        omitted.drain(position..position + 2);
        assert!(parse_args(&omitted, &root).is_err());
    }
}
