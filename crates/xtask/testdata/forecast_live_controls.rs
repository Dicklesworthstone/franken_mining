use super::*;

const REGISTER: &str = "[[forecast]]\nid = \"one\"\nmodel_key = \"synthetic\"\noutcome_due = \"2030-01-02T00:00:00Z\"\nkind = \"interval\"\nlo = \"40\"\nhi = \"50\"\nnominal = \"0.8\"\n";
const FREEZE: &str = "[[cohort]]\nname = \"test\"\nmembers = \"one\"\nresolution_rule = \"Invented source value\"\neligible_sources = \"synthetic:issuer\"\n";
const MONITOR: &str = "monitor_alarm_alpha = \"0.05\"\nmonitor_support = \"Synthetic intervals\"\nmonitor_conditional_null = \"Conditional hit probability is exactly 0.8\"\nmonitor_order = \"one\"\n";

fn semantic_record(request: String, observed_ns: i64) -> Record {
    Record {
        sequence: 1,
        previous: Digest32::default(),
        observed_ns,
        entry: Entry::Operation(request),
    }
}

#[test]
fn monitor_v5_freeze_binds_policy_without_reinterpreting_older_operations() {
    let start = time("2030-01-01T00:00:00Z").expect("date");
    let header = Header {
        cell_id: "synthetic".into(),
        clock_skew_ns: 7,
    };
    let request = format!("{FREEZE}{MONITOR}");
    let stored = version_request(&request, "cohort").expect("monitor writer");
    assert!(stored.starts_with(&format!("schema = \"{V5_SCHEMA}\"\n")));
    let mut state = Replay::default();
    apply(
        &mut state,
        &semantic_record(
            version_request(REGISTER, "forecast").expect("register"),
            start,
        ),
        &header,
        None,
    )
    .expect("registration");
    for schema in [None, Some(V2_SCHEMA), Some(V3_SCHEMA), Some(V4_SCHEMA)] {
        let older = schema.map_or_else(
            || request.clone(),
            |schema| format!("schema = \"{schema}\"\n{request}"),
        );
        assert_eq!(
            apply(
                &mut state,
                &semantic_record(older, start + 20),
                &header,
                None
            )
            .expect_err("old grammar cannot enable monitoring")
            .code,
            "FMINE-REF-107"
        );
        assert!(state.cohorts.is_empty());
        let event_policy = format!("{FREEZE}event_log_floor = \"0.01\"\n");
        let older = schema.map_or_else(
            || event_policy.clone(),
            |schema| format!("schema = \"{schema}\"\n{event_policy}"),
        );
        assert_eq!(
            apply(
                &mut state,
                &semantic_record(older, start + 20),
                &header,
                None
            )
            .expect_err("old grammar cannot enable event log scoring")
            .code,
            "FMINE-REF-107"
        );
    }
    apply(
        &mut state,
        &semantic_record(stored.clone(), start + 20),
        &header,
        Some("cohort"),
    )
    .expect("monitored freeze");
    let cohort = state
        .ledger
        .cohort(state.cohorts["test"])
        .expect("frozen cohort");
    assert_eq!(cohort.spec.frozen_at_ns, start + 27);
    assert_eq!(
        cohort
            .spec
            .policy
            .coverage_monitor
            .as_ref()
            .expect("policy")
            .order,
        vec![state.forecast_id("one").expect("one")]
    );
    assert!(cohort_json(cohort).contains("\"coverage_monitor\":{\"alarm_alpha\":0.05"));
    let mut counts = VersionCounts::default();
    counts.record(OperationVersion::V2);
    counts.record(OperationVersion::V5);
    assert_eq!(counts.policy(), "clock_upper_bound");
    assert!(counts.fields().contains("\"v5\":1"));

    let mut reopened = Replay::default();
    apply(
        &mut reopened,
        &semantic_record(
            version_request(REGISTER, "forecast").expect("register"),
            start,
        ),
        &header,
        None,
    )
    .expect("replayed registration");
    apply(
        &mut reopened,
        &semantic_record(stored, start + 20),
        &header,
        None,
    )
    .expect("replayed freeze");
    assert_eq!(state.cohorts, reopened.cohorts);
    let plain_v5 = format!("schema = \"{V5_SCHEMA}\"\n{FREEZE}");
    assert_eq!(
        apply(
            &mut reopened,
            &semantic_record(plain_v5, start + 30),
            &header,
            None
        )
        .expect_err("v5 needs an opt-in policy")
        .code,
        "FMINE-REF-107"
    );
}

#[test]
fn writer_combines_monitor_and_public_vintage_policies_in_v5() {
    let combined = format!("{FREEZE}{MONITOR}public_vintages = \"registered_comparatives\"\n");
    assert!(
        version_request(&combined, "cohort")
            .expect("combined version")
            .starts_with(&format!("schema = \"{V5_SCHEMA}\"\n"))
    );
    let event_log = format!("{FREEZE}event_log_floor = \"0.01\"\n");
    assert!(
        version_request(&event_log, "cohort")
            .expect("event log version")
            .starts_with(&format!("schema = \"{V5_SCHEMA}\"\n"))
    );
    assert!(
        version_request(FREEZE, "cohort")
            .expect("unchanged plain freeze")
            .starts_with(&format!("schema = \"{V2_SCHEMA}\"\n"))
    );
}

#[test]
fn event_log_journal_persists_scores_and_retrieves_the_original_after_restatement() {
    use std::sync::atomic::{AtomicU64, Ordering};

    struct Clock(i64);
    impl WallClock for Clock {
        fn now_ns(&self) -> Result<i64, fmine_runtime::ClockError> {
            Ok(self.0)
        }
    }
    static SERIAL: AtomicU64 = AtomicU64::new(0);
    let root = std::env::temp_dir().join(format!(
        "fmine-event-log-journal-{}-{}",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed),
    ));
    std::fs::create_dir(&root).expect("synthetic journal workspace");
    let journal = PathBuf::from("history");
    let request = PathBuf::from("request.toml");
    let head = || {
        Journal::open_read(&root.join(&journal), None)
            .expect("reopen journal")
            .head()
    };
    let run = |command: Command, observed: i64| {
        let result = execute_with_clock(&command, &root, &Clock(observed));
        assert_eq!(result.exit_code, 0, "{:?}", result.refusal);
        result
    };
    let start = time("2030-01-01T00:00:00Z").expect("start");
    run(
        Command::Init {
            journal: journal.clone(),
            cell_id: "synthetic-event-log".into(),
            clock_skew_ns: 7,
        },
        start,
    );
    let registration = "[[forecast]]\nid = \"one\"\nmodel_key = \"synthetic-binary\"\noutcome_due = \"2030-01-02T00:00:00Z\"\nkind = \"bernoulli\"\nprobability = \"0.25\"\n";
    std::fs::write(root.join(&request), registration).expect("registration request");
    run(
        Command::Register {
            journal: journal.clone(),
            head: head(),
            request: request.clone(),
        },
        start + 10,
    );
    std::fs::write(
        root.join(&request),
        format!("{FREEZE}event_log_floor = \"0.01\"\n"),
    )
    .expect("freeze request");
    let frozen = run(
        Command::Freeze {
            journal: journal.clone(),
            head: head(),
            request: request.clone(),
        },
        start + 20,
    );
    assert!(frozen.payload_json.contains("\"v5\":1"));
    assert!(frozen.payload_json.contains("\"event_log_floor\":0.01"));
    let original = "[[outcome]]\nforecast = \"one\"\nkind = \"published\"\navailable_at = \"2030-01-03T00:00:00Z\"\nvalue = \"1\"\nsource = \"synthetic:issuer\"\nvintage = \"first\"\nlocator = \"Invented!A1\"\nsha256 = \"1111111111111111111111111111111111111111111111111111111111111111\"\n";
    std::fs::write(root.join(&request), original).expect("original outcome");
    run(
        Command::Outcome {
            journal: journal.clone(),
            head: head(),
            request: request.clone(),
        },
        time("2030-01-03T00:00:00Z").expect("publication") + 7,
    );
    let assessed = run(
        Command::Assess {
            journal: journal.clone(),
            head: head(),
            cohort: "test".into(),
            as_of: "2030-01-04T00:00:00Z".into(),
            selector: "first-published".into(),
            expected_digest: None,
        },
        time("2030-01-04T00:00:00Z").expect("cut") + 7,
    );
    assert!(
        assessed
            .payload_json
            .contains("\"event_log\":{\"probability_floor\":0.01,\"units\":\"nats\"")
    );
    let markdown = assessed.rendered_markdown.expect("human score report");
    assert!(markdown.contains("## Event log score"));
    assert!(markdown.contains("Frozen probability floor: 0.01"));
    assert!(markdown.contains("in nats. Scored: 1 of 1"));
    assert!(markdown.contains("removes strict propriety in the floored tails"));
    let saved_digest = {
        let reopened =
            Journal::open_read(&root.join(&journal), None).expect("assessment persisted");
        let state = replay(&reopened).expect("semantic replay");
        state.checkpoints[0].digest
    };
    let restated = original
        .replace("\"published\"", "\"restated\"")
        .replace("2030-01-03", "2030-01-05")
        .replace("value = \"1\"", "value = \"0\"")
        .replace("vintage = \"first\"", "vintage = \"restated\"");
    std::fs::write(root.join(&request), format!("{restated}supersedes = 1\n"))
        .expect("restatement");
    run(
        Command::Outcome {
            journal: journal.clone(),
            head: head(),
            request,
        },
        time("2030-01-05T00:00:00Z").expect("restatement date") + 7,
    );
    let latest = run(
        Command::Assess {
            journal: journal.clone(),
            head: head(),
            cohort: "test".into(),
            as_of: "2030-01-06T00:00:00Z".into(),
            selector: "latest-as-of".into(),
            expected_digest: None,
        },
        time("2030-01-06T00:00:00Z").expect("latest cut") + 7,
    );
    assert!(latest.payload_json.contains("\"observed_mean\":0.287"));
    let before = std::fs::read(root.join(&journal)).expect("saved journal bytes");
    let retrieved = run(
        Command::Report {
            journal: journal.clone(),
            head: Some(head()),
            assessment: saved_digest,
            provenance: None,
            timestamp: None,
        },
        -1,
    );
    assert!(retrieved.payload_json.contains(&saved_digest.to_hex()));
    assert!(retrieved.payload_json.contains("\"observed_mean\":1.386"));
    assert!(retrieved.payload_json.contains("\"value\":1"));
    assert!(retrieved.payload_json.contains("\"vintage\":\"first\""));
    assert_eq!(
        std::fs::read(root.join(journal)).expect("read-only retrieval"),
        before
    );
}

#[test]
fn monitor_markdown_displays_original_intervals_and_the_missing_reason() {
    let fixture = include_str!("../../../tests/forecasts/cohort_synthetic.toml");
    let monitor = MONITOR.replace(
        "monitor_order = \"one\"",
        "monitor_order = \"north-unit-cost,south-unit-cost,central-unit-cost\"",
    );
    let text = fixture.replace(
        "rho_high = \"0.5\"",
        &format!("rho_high = \"0.5\"\n{monitor}"),
    );
    let mut state = Replay::parse(&text).expect("monitored synthetic transcript");
    let assessment = state
        .assessment(
            "synthetic-demo",
            time("2026-10-31T00:00:00Z").expect("cut"),
            super::super::OutcomeSelector::LatestAsOf,
        )
        .expect("restated assessment");
    let header = Header {
        cell_id: "synthetic".into(),
        clock_skew_ns: 0,
    };
    let rendered = markdown(
        &header,
        Digest32::default(),
        &state,
        "assess",
        Some(&assessment),
        4,
        VersionCounts([0, 0, 0, 0, 1, 0, 0, 0, 0, 0]),
    )
    .expect("readable saved assessment");
    let monitor = rendered
        .split_once("## Sequential coverage monitor")
        .expect("human monitor")
        .1;
    assert!(monitor.contains("[1476, 1490]"));
    assert!(monitor.contains("1479; synthetic:issuer / synthetic-first / Invented!A1"));
    assert!(!monitor.contains("1474"));
    assert!(monitor.contains("reason: not_published"));
    assert!(
        monitor.contains(
            "1 later members are deferred, including 1 with resolved original publications"
        )
    );
}

#[test]
fn writer_adds_v2_but_clients_cannot_select_a_stored_version() {
    let stored = version_request(REGISTER, "forecast").expect("writer v2 marker");
    assert_eq!(stored, format!("schema = \"{V2_SCHEMA}\"\n{REGISTER}"));
    assert_eq!(
        operation_version(&parse_subset(&stored).expect("v2 doc")).expect("v2"),
        OperationVersion::V2
    );
    assert_eq!(
        operation_version(&parse_subset(REGISTER).expect("legacy doc")).expect("v1"),
        OperationVersion::LegacyV1
    );
    for request in [
        stored,
        format!("schema = \"unknown\"\n{REGISTER}"),
        format!("{REGISTER}\n{FREEZE}"),
        FREEZE.to_string(),
    ] {
        assert_eq!(
            version_request(&request, "forecast")
                .expect_err("client grammar")
                .code,
            "FMINE-REF-107"
        );
    }
    let unknown =
        parse_subset(&format!("schema = \"unknown\"\n{REGISTER}")).expect("unknown schema syntax");
    assert_eq!(
        operation_version(&unknown)
            .expect_err("unknown stored policy")
            .code,
        "FMINE-REF-107"
    );
    let oversized = format!(
        "{REGISTER}#{}",
        "x".repeat(MAX_OPERATION_BYTES - REGISTER.len() - 1)
    );
    assert_eq!(oversized.len(), MAX_OPERATION_BYTES);
    assert_eq!(
        version_request(&oversized, "forecast")
            .expect_err("header also counts")
            .code,
        "FMINE-REF-107"
    );
}

#[test]
fn versioned_authority_times_change_ids_without_reinterpreting_legacy_records() {
    let now = time("2030-01-01T00:00:00Z").expect("date") + 123;
    let header = Header {
        cell_id: "synthetic".into(),
        clock_skew_ns: 7,
    };
    let mut ids = Vec::new();
    for v2 in [false, true] {
        let mut state = Replay::default();
        let request = if v2 {
            version_request(REGISTER, "forecast").expect("v2")
        } else {
            REGISTER.into()
        };
        apply(
            &mut state,
            &semantic_record(request, now),
            &header,
            Some("forecast"),
        )
        .expect("registration");
        let id = state.forecast_id("one").expect("id");
        let authority = now + if v2 { 7 } else { 0 };
        assert_eq!(
            state.ledger.forecast(id).expect("forecast").registered_ns,
            authority
        );
        ids.push(id);
        let request = if v2 {
            version_request(FREEZE, "cohort").expect("v2")
        } else {
            FREEZE.into()
        };
        apply(
            &mut state,
            &semantic_record(request, now + 20),
            &header,
            Some("cohort"),
        )
        .expect("freeze");
        let cohort = state.cohorts["test"];
        assert_eq!(
            state
                .ledger
                .cohort(cohort)
                .expect("cohort")
                .spec
                .frozen_at_ns,
            authority + 20
        );
    }
    assert_ne!(ids[0], ids[1]);
    let mut legacy = Replay::default();
    apply(
        &mut legacy,
        &semantic_record(REGISTER.into(), now),
        &header,
        None,
    )
    .expect("legacy reread");
    assert_eq!(
        legacy.forecast_id("one").expect("unchanged legacy id"),
        ids[0]
    );
}

#[test]
fn v2_assessments_require_a_checked_digest_and_legacy_unbound_history_stays_readable() {
    let header = Header {
        cell_id: "synthetic".into(),
        clock_skew_ns: 7,
    };
    let start = time("2030-01-01T00:00:00Z").expect("date");
    let cut = time("2030-01-03T00:00:00Z").expect("cut");
    let mut state = Replay::default();
    apply(
        &mut state,
        &semantic_record(REGISTER.into(), start),
        &header,
        None,
    )
    .expect("legacy forecast");
    apply(
        &mut state,
        &semantic_record(FREEZE.into(), start + 20),
        &header,
        None,
    )
    .expect("legacy cohort");
    let request = "[[assessment]]\ncohort = \"test\"\nas_of = \"2030-01-03T00:00:00Z\"\nselector = \"first-published\"\n";
    let missing = version_request(request, "assessment").expect("v2 request");
    assert_eq!(
        apply(
            &mut state,
            &semantic_record(missing, cut + 7),
            &header,
            None
        )
        .expect_err("v2 must pin")
        .code,
        "FMINE-REF-107"
    );
    assert!(state.checkpoints.is_empty());
    let report = apply(
        &mut state,
        &semantic_record(request.into(), cut + 7),
        &header,
        None,
    )
    .expect("legacy checkpoint")
    .expect("assessment");
    assert!(state.checkpoints[0].expected.is_none());
    let bound = format!(
        "{request}expected_digest = {}\n",
        js(&report.digest.to_hex())
    );
    let bound = version_request(&bound, "assessment").expect("v2 pin");
    let replayed = apply(&mut state, &semantic_record(bound, cut + 8), &header, None)
        .expect("v2 checked checkpoint")
        .expect("assessment");
    assert_eq!(replayed.digest, report.digest);
    let wrong = format!("{request}expected_digest = \"{}\"\n", "00".repeat(32));
    let wrong = version_request(&wrong, "assessment").expect("v2 wrong pin syntax");
    assert_eq!(
        apply(&mut state, &semantic_record(wrong, cut + 9), &header, None)
            .expect_err("wrong pin")
            .code,
        "FMINE-REF-505"
    );
}

#[test]
fn generated_assessment_values_preserve_names_without_interpreting_toml() {
    let name = "cohort \\\"first\\\" \\ desk #1";
    let request = format!(
        "[[assessment]]\ncohort = {}\n",
        toml_string(name).expect("safe name")
    );
    let doc = parse_subset(&request).expect("one assessment table");
    assert!(doc.top.is_empty());
    assert_eq!(doc.rows.len(), 1);
    assert_eq!(Row(&doc.rows[0]).string("cohort").expect("cohort"), name);
    for value in [
        "name\n[[forecast]]",
        "name\rnext",
        "name\tpart",
        "name\0tail",
    ] {
        assert_eq!(
            toml_string(value).expect_err("control character").code,
            "FMINE-REF-107"
        );
    }
}

#[test]
fn observed_registration_nanoseconds_are_preserved_without_timestamp_reparsing() {
    let observed_ns = time("2030-01-02T00:00:00Z").expect("date") + 123_456_789;
    let mut state = Replay::default();
    let header = Header {
        cell_id: "synthetic".into(),
        clock_skew_ns: 0,
    };
    let record = Record {
        sequence: 1,
        previous: Digest32::default(),
        observed_ns,
        entry: Entry::Operation("[[forecast]]\nid = \"one\"\nmodel_key = \"synthetic\"\noutcome_due = \"2031-01-01T00:00:00Z\"\nkind = \"bernoulli\"\nprobability = \"0.5\"\n".into()),
    };
    apply(&mut state, &record, &header, Some("forecast")).expect("live registration");
    let id = state.forecast_id("one").expect("registered id");
    let registered = state.ledger.forecast(id).expect("registered forecast");
    assert_eq!(registered.registered_ns, observed_ns);
    assert_eq!(
        utc(registered.registered_ns),
        "\"2030-01-02T00:00:00.123456789Z\""
    );
}
