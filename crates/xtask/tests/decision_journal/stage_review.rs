//! FACT: staged observations retain exactly the archived continuation and enforce causal order.
//! Determinism: Exact

use super::*;

const STAGED: &str = include_str!("../../../../tests/capital/staged_permit.toml");
const OBSERVED: &str = "2030-04-02T00:00:00Z";
const AVAILABLE: &str = "2030-04-02T00:00:02Z";

fn timestamp(time: &str) -> i64 {
    fmine_calib::record::parse_utc(time).unwrap() * 1_000_000_000
}

fn binary_json(f: &Fixture, args: &[&str]) -> Json {
    let result = Process::new(binary())
        .args(args)
        .args(["--workspace", f.root.to_str().unwrap(), "--json"])
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stdout)
    );
    parse_json(&result.stdout).unwrap()
}

fn initial(f: &Fixture, policy: &str, fixture: &str) -> Digest32 {
    fs::write(&f.model, fixture).unwrap();
    let preview = binary_json(f, &["decide", "program", f.model.to_str().unwrap()]);
    let actions = json_field(&preview, &["payload", "journal_actions"])
        .as_array()
        .unwrap();
    let action = actions
        .iter()
        .find(|row| row.get("policy_id").and_then(Json::as_str) == Some(policy))
        .unwrap();
    let action = json_field(action, &["id"]).as_str().unwrap();
    head(&successful(execute_with_clock(
        &f.record(f.initial, "initial-policy", action),
        &f.root,
        &Clock(200),
    )))
}

fn worksheet(head: Digest32, signal: Option<&str>) -> String {
    let signal = signal.map_or_else(String::new, |id| format!("observation = \"{id}\"\n"));
    format!(
        "schema = \"fmine.capital-stage-observation.v1\"\ninitial_decision = \"initial-policy\"\ninitial_head = \"{}\"\n{signal}observed_at = \"{OBSERVED}\"\navailable_at = \"{AVAILABLE}\"\nsource = \"Synthetic scope-matched permit review memorandum RV-42\"\nevidence = \"synthetic\"\n",
        head.to_hex()
    )
}

fn rows(report: &Json) -> &[Json] {
    json_field(report, &["decisions"]).as_array().unwrap()
}

fn artifact(row: &Json) -> Json {
    let text = json_field(row, &["snapshot", "artifact", "content"])
        .as_str()
        .unwrap();
    parse_json(text.as_bytes()).unwrap()
}

#[test]
fn stage_review_preserves_the_exact_frozen_branch_and_reopens_after_edit() {
    let f = Fixture::new();
    // The fixture's review date is in the past for the real host-clock command.
    let parent = initial(&f, "policy_004", &STAGED.replace("2030-", "2001-"));
    let old_parent = operation_fields(&f, parent, 1);
    fs::write(&f.model, MODEL).unwrap();
    let unrelated = successful(execute_with_clock(
        &f.record(parent, "intervening", "hold"),
        &f.root,
        &Clock(300),
    ));
    let current = head(&unrelated);
    let bad_parent =
        worksheet(current, Some("clearance-documented")).replace("initial-policy", "intervening");
    fs::write(&f.model, &bad_parent).unwrap();
    f.refuses_without_append(
        &f.record(current, "not-a-policy", "observe-stage"),
        timestamp(AVAILABLE) + 10,
        "FMINE-REF-105",
    );
    let source = worksheet(parent, Some("clearance-documented")).replace("2030-", "2001-");
    fs::write(&f.model, &source).unwrap();
    let done = binary_json(
        &f,
        &[
            "decide",
            "journal",
            "record",
            "--journal",
            f.journal.to_str().unwrap(),
            "--expected-head",
            &current.to_hex(),
            "--model",
            f.model.to_str().unwrap(),
            "--decision",
            "review-one",
            "--action",
            "observe-stage",
            "--reason",
            "Synthetic observed review of frozen capital policy",
            "--code-revision",
            REVISION,
            "--trust-host-clock",
        ],
    );
    let saved = json_field(&done, &["payload"]);
    let pin = Digest32(
        fmine_calib::record::parse_hex32(saved.get("head").and_then(Json::as_str).unwrap())
            .unwrap(),
    );
    assert_eq!(operation_fields(&f, pin, 1), old_parent);
    assert_eq!(rows(saved).len(), 3);
    let original = artifact(&rows(saved)[0]);
    let review = artifact(&rows(saved)[2]);
    for (field, value) in [
        ("initial_decision", "initial-policy"),
        ("observation", "clearance-documented"),
        ("observed_at", "2001-04-02T00:00:00Z"),
        ("available_at", "2001-04-02T00:00:02Z"),
        ("policy_id", "policy_004"),
        ("result", "matched_branch"),
        ("evidence", "synthetic"),
    ] {
        assert_eq!(review.get(field).and_then(Json::as_str), Some(value));
    }
    assert_eq!(
        review.get("comparison_digest"),
        original.get("comparison_digest")
    );
    assert_eq!(
        review.get("initial_policy_claim_id"),
        original.get("claim_id")
    );
    assert_eq!(
        review
            .get("observation_model_sha256")
            .and_then(Json::as_str),
        Some(sha256(source.as_bytes()).to_hex().as_str())
    );
    assert_eq!(review.get("execution_authorized"), Some(&Json::Bool(false)));
    let continuation = json_field(&review, &["modeled_continuation"]);
    assert_eq!(
        continuation
            .get("selected")
            .and_then(Json::as_array)
            .unwrap(),
        [Json::String("main".into())]
    );
    assert_eq!(
        continuation.get("spend").and_then(Json::as_str),
        Some("4000000/1")
    );
    assert_eq!(
        continuation.get("spending_unit").and_then(Json::as_str),
        Some("USD")
    );
    fs::write(
        &f.model,
        "# Synthetic observation source edited after the review\n",
    )
    .unwrap();
    let reopened = binary_json(
        &f,
        &[
            "decide",
            "journal",
            "show",
            "--journal",
            f.journal.to_str().unwrap(),
            "--head",
            &pin.to_hex(),
        ],
    );
    assert_eq!(rows(json_field(&reopened, &["payload"])), rows(saved));
    fs::write(&f.model, &source).unwrap();
    f.refuses_without_append(
        &f.record(pin, "another-review", "observe-stage"),
        timestamp(AVAILABLE) + 100,
        "FMINE-REF-507",
    );
}

#[test]
fn stage_review_distinguishes_a_planned_hold_an_unmodeled_signal_and_a_common_continuation() {
    for (policy, signal, status) in [
        (
            "policy_004",
            Some("no-clearance-documented"),
            "matched_branch",
        ),
        ("policy_004", Some("footprint-redesign"), "outside_policy"),
        ("policy_000", None, "common_continuation"),
    ] {
        let f = Fixture::new();
        let parent = initial(&f, policy, STAGED);
        fs::write(&f.model, worksheet(parent, signal)).unwrap();
        let done = successful(execute_with_clock(
            &f.record(parent, "review", "observe-stage"),
            &f.root,
            &Clock(timestamp(AVAILABLE) + 10),
        ));
        let result = parse_json(done.payload_json.as_bytes()).unwrap();
        let review = artifact(&rows(&result)[1]);
        assert_eq!(review.get("result").and_then(Json::as_str), Some(status));
        let continuation = json_field(&review, &["modeled_continuation"]);
        if status == "outside_policy" {
            assert_eq!(continuation, &Json::Null);
            assert_eq!(review.get("observation").and_then(Json::as_str), signal);
        } else {
            assert_eq!(
                continuation.get("spend").and_then(Json::as_str),
                Some("0/1")
            );
            assert!(
                continuation
                    .get("selected")
                    .and_then(Json::as_array)
                    .unwrap()
                    .is_empty()
            );
        }
    }
}

#[test]
fn stage_review_refuses_bad_dates_wrong_heads_and_unavailable_evidence_atomically() {
    let f = Fixture::new();
    let parent = initial(&f, "policy_004", STAGED);
    let observation = worksheet(parent, Some("clearance-documented"));
    let commit = timestamp(AVAILABLE) + 10;
    for (text, action, time, code) in [
        (
            observation.clone(),
            "observe-stage",
            commit - 1,
            "FMINE-REF-107",
        ),
        (observation.clone(), "main", commit, "FMINE-REF-107"),
        (
            observation.replace(OBSERVED, "2030-04-01T23:59:59Z"),
            "observe-stage",
            commit,
            "FMINE-REF-107",
        ),
        (
            observation.replace(AVAILABLE, "2030-04-01T23:59:59Z"),
            "observe-stage",
            commit,
            "FMINE-REF-107",
        ),
        (
            observation.replace(&parent.to_hex(), &f.initial.to_hex()),
            "observe-stage",
            commit,
            "FMINE-REF-505",
        ),
        (
            observation.replace("initial-policy", "not-recorded"),
            "observe-stage",
            commit,
            "FMINE-REF-505",
        ),
        (
            worksheet(parent, None),
            "observe-stage",
            commit,
            "FMINE-REF-107",
        ),
        (
            observation.replace("evidence = \"synthetic\"", "evidence = \"measured\""),
            "observe-stage",
            commit,
            "FMINE-REF-107",
        ),
        (
            format!(
                "{observation}#{}",
                "x".repeat(32_768 - observation.len() - 1)
            ),
            "observe-stage",
            commit,
            "FMINE-REF-107",
        ),
    ] {
        fs::write(&f.model, text).unwrap();
        f.refuses_without_append(&f.record(parent, "review", action), time, code);
    }
    successful(f.show(Some(parent)));
}

#[test]
fn stage_replay_refuses_a_different_continuation_even_in_a_rebuilt_chain() {
    let f = Fixture::new();
    let parent = initial(&f, "policy_004", STAGED);
    let prefix = fs::read(&f.journal).unwrap();
    fs::write(&f.model, worksheet(parent, Some("clearance-documented"))).unwrap();
    let time = timestamp(AVAILABLE) + 10;
    let saved = successful(execute_with_clock(
        &f.record(parent, "review", "observe-stage"),
        &f.root,
        &Clock(time),
    ));
    let fields = operation_fields(&f, head(&saved), 2);
    let report = parse_json(saved.payload_json.as_bytes()).unwrap();
    let bytes = json_field(&rows(&report)[1], &["snapshot", "artifact", "content"])
        .as_str()
        .unwrap();
    let edited = bytes.replace("4000000/1", "9000000/1");
    assert_ne!(edited, bytes);
    let rewritten = fields
        .iter()
        .map(|(key, value)| {
            let value = match key.as_str() {
                "card_sha256" => sha256(edited.as_bytes()).to_hex(),
                "card_hex" => hex(edited.as_bytes()),
                "card_json" => edited.clone(),
                _ => value.clone(),
            };
            format!(
                "{key} = \"{}\"\n",
                value.replace('\\', "\\\\").replace('"', "\\\"")
            )
        })
        .collect::<String>();
    let altered = f.root.join("different-branch.journal");
    fs::write(&altered, prefix).unwrap();
    let mut journal = Journal::open_write(&altered, parent).unwrap();
    journal
        .append(&rewritten, &Clock(time), |_, _| Ok(()))
        .unwrap();
    let pin = journal.head();
    drop(journal);
    let result = execute_with_clock(
        &Command::Show {
            journal: altered,
            head: Some(pin),
        },
        &f.root,
        &NoClock,
    );
    assert_eq!(result.refusal.unwrap().code, "FMINE-REF-505");
}
