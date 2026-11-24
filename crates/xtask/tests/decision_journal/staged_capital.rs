//! FACT: synthetic staged-policy choices retain the evaluated branches and preceding clock bound.
//! Determinism: Exact

use super::*;

const STAGED: &str = include_str!("../../../../tests/capital/staged_permit.toml");

fn cli(f: &Fixture, args: &[&str]) -> String {
    let output = Process::new(binary())
        .args(args)
        .args(["--workspace", f.root.to_str().unwrap()])
        .output()
        .unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(output.status.success(), "{stdout}");
    stdout
}

fn comparison(f: &Fixture) -> Json {
    let json = cli(
        f,
        &["decide", "program", f.model.to_str().unwrap(), "--json"],
    );
    parse_json(json.as_bytes()).unwrap()
}

fn action(report: &Json, policy: &str) -> String {
    let digest = json_field(report, &["payload", "program", "digest"])
        .as_str()
        .unwrap();
    format!("{policy}@{digest}")
}

fn observation_ns() -> i64 {
    fmine_calib::record::parse_utc("2030-04-02T00:00:00Z").unwrap() * 1_000_000_000
}

#[test]
fn staged_policy_command_keeps_the_reviewed_branches_and_reopens_after_model_changes() {
    let f = Fixture::new();
    let earlier = successful(execute_with_clock(
        &f.record(f.initial, "prior", "hold"),
        &f.root,
        &Clock(200),
    ));
    let prior = head(&earlier);
    let old_fields = operation_fields(&f, prior, 1);
    fs::write(&f.model, STAGED).unwrap();
    let report = comparison(&f);
    let offered = json_field(&report, &["payload", "journal_actions"])
        .as_array()
        .unwrap();
    let ids: Vec<_> = offered
        .iter()
        .map(|row| json_field(row, &["policy_id"]).as_str().unwrap())
        .collect();
    assert_eq!(ids, ["policy_000", "policy_002", "policy_004"]);
    let reviewed = action(&report, "policy_004");
    assert_eq!(
        json_field(&offered[2], &["id"]).as_str(),
        Some(reviewed.as_str())
    );
    let result = cli(
        &f,
        &[
            "decide",
            "journal",
            "record",
            "--journal",
            f.journal.to_str().unwrap(),
            "--expected-head",
            &prior.to_hex(),
            "--model",
            f.model.to_str().unwrap(),
            "--decision",
            "early-works-policy",
            "--action",
            &reviewed,
            "--reason",
            "Retain the staged commitment and clearance-dependent continuation",
            "--code-revision",
            REVISION,
            "--trust-host-clock",
            "--json",
        ],
    );
    let result = parse_json(result.as_bytes()).unwrap();
    let decisions = json_field(&result, &["payload", "decisions"])
        .as_array()
        .unwrap();
    assert_eq!(decisions.len(), 2);
    let snapshot = json_field(&decisions[1], &["snapshot"]);
    assert_eq!(
        json_field(snapshot, &["selected_action"]).as_str(),
        Some(reviewed.as_str())
    );
    assert_eq!(
        json_field(snapshot, &["model", "content"]).as_str(),
        Some(STAGED)
    );
    assert_eq!(
        json_field(snapshot, &["claim_id"]),
        json_field(&report, &["payload", "program", "claim", "id"])
    );
    let bytes = json_field(snapshot, &["artifact", "content"])
        .as_str()
        .unwrap();
    assert!(bytes.len() + STAGED.len() < 16_384);
    assert_eq!(
        json_field(snapshot, &["artifact", "sha256"]).as_str(),
        Some(sha256(bytes.as_bytes()).to_hex().as_str())
    );
    let saved = parse_json(bytes.as_bytes()).unwrap();
    assert_eq!(
        json_field(&saved, &["schema"]).as_str(),
        Some("fmine.capital-policy-decision.v1")
    );
    assert_eq!(
        json_field(&saved, &["comparison_digest"]),
        json_field(&report, &["payload", "program", "digest"])
    );
    assert_eq!(
        json_field(&saved, &["observation_date_utc"]).as_str(),
        Some("2030-04-02")
    );
    assert_eq!(
        saved.get("decision_stage").and_then(Json::as_str),
        Some("initial")
    );
    assert_eq!(
        saved.get("spending_unit").and_then(Json::as_str),
        Some("USD")
    );
    assert_eq!(
        saved.get("objective_unit").and_then(Json::as_str),
        Some("USD")
    );
    assert_eq!(
        saved.get("valuation_basis"),
        Some(json_field(
            &report,
            &[
                "payload",
                "program",
                "inputs",
                "information",
                "value",
                "valuation_basis"
            ]
        ))
    );
    let selected = json_field(&saved, &["policy"]);
    let policies = json_field(&report, &["payload", "program", "summary", "policies"])
        .as_array()
        .unwrap();
    assert_eq!(selected, &policies[4]);
    let branches = json_field(selected, &["branches"]).as_array().unwrap();
    assert_eq!(
        json_field(selected, &["initial_spend"]).as_str(),
        Some("6000000/1")
    );
    assert_eq!(
        json_field(&branches[0], &["selected"]).as_array().unwrap()[0].as_str(),
        Some("main")
    );
    assert!(
        json_field(&branches[1], &["selected"])
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert_eq!(saved.get("execution_authorized"), Some(&Json::Bool(false)));
    let pin = Digest32(
        fmine_calib::record::parse_hex32(
            json_field(&result, &["payload", "head"]).as_str().unwrap(),
        )
        .unwrap(),
    );
    assert_eq!(operation_fields(&f, pin, 1), old_fields);
    fs::write(&f.model, "changed after the initial committee meeting").unwrap();
    let replay = successful(f.show(Some(pin)));
    let replay = parse_json(replay.payload_json.as_bytes()).unwrap();
    assert_eq!(
        json_field(&replay, &["decisions"]).as_array().unwrap(),
        decisions
    );
}

#[test]
fn staged_policy_commit_and_replay_require_the_whole_clock_interval_before_observation() {
    let f = Fixture::new();
    fs::write(&f.model, STAGED).unwrap();
    let choice = action(&comparison(&f), "policy_000");
    let record = f.record(f.initial, "hold-before-review", &choice);
    let cutoff = observation_ns();
    for now in [cutoff - 10, cutoff, cutoff + 1] {
        f.refuses_without_append(&record, now, "FMINE-REF-107");
    }
    let saved = successful(execute_with_clock(&record, &f.root, &Clock(cutoff - 11)));
    let pin = head(&saved);
    successful(f.show(Some(pin)));
    let saved = Journal::open_read(&f.journal, Some(pin)).unwrap();
    let Entry::Operation(operation) = &saved.records()[1].entry else {
        panic!("operation")
    };
    let wrong_selector = operation.replacen("action = \"policy_000@", "action = \"policy_004@", 1);
    for (bytes, now, code) in [
        (operation.as_str(), cutoff, "FMINE-REF-107"),
        (wrong_selector.as_str(), cutoff - 11, "FMINE-REF-505"),
    ] {
        let other = Fixture::new();
        let mut writer = Journal::open_write(&other.journal, other.initial).unwrap();
        writer.append(bytes, &Clock(now), |_, _| Ok(())).unwrap();
        let pin = writer.head();
        drop(writer);
        assert_eq!(other.show(Some(pin)).refusal.unwrap().code, code);
    }
}

#[test]
fn staged_policy_journal_preserves_and_enforces_separate_equity_risk() {
    let f = Fixture::new();
    fs::write(
        &f.model,
        include_str!("../../../../tests/capital/entity_valuation.toml"),
    )
    .unwrap();
    let report = comparison(&f);
    let summary = json_field(&report, &["payload", "program", "summary"]);
    let winner = |set| {
        json_field(summary, &[set]).as_array().unwrap()[0]
            .as_str()
            .unwrap()
    };
    let rejected = action(&report, winner("unconstrained_winners"));
    f.refuses_without_append(
        &f.record(f.initial, "carve-out-breach", &rejected),
        300,
        "FMINE-REF-105",
    );
    let chosen = action(&report, winner("feasible_winners"));
    let saved = successful(execute_with_clock(
        &f.record(f.initial, "funded-deal", &chosen),
        &f.root,
        &Clock(300),
    ));
    let record = parse_json(saved.payload_json.as_bytes()).unwrap();
    let snapshots = json_field(&record, &["decisions"]).as_array().unwrap();
    let artifact = json_field(&snapshots[0], &["snapshot", "artifact", "content"])
        .as_str()
        .unwrap();
    let artifact = parse_json(artifact.as_bytes()).unwrap();
    assert_eq!(
        artifact.get("objective_unit").and_then(Json::as_str),
        Some("USD/share")
    );
    assert_eq!(
        artifact.get("spending_unit").and_then(Json::as_str),
        Some("USD")
    );
    let secondaries = json_field(&artifact, &["policy", "entity_valuations"])
        .as_array()
        .unwrap();
    assert_eq!(
        json_field(&secondaries[0], &["entity"]).as_str(),
        Some("carve-out")
    );
    assert_eq!(
        json_field(&secondaries[0], &["per_share_tail_loss"]).as_str(),
        Some("1/10")
    );
    assert_eq!(
        json_field(&secondaries[0], &["max_tail_loss_per_share"]).as_str(),
        Some("1/10")
    );
    successful(f.show(Some(head(&saved))));
}

#[test]
fn staged_journal_refuses_changed_inputs_and_ineligible_or_oversize_policies_atomically() {
    let f = Fixture::new();
    fs::write(&f.model, STAGED).unwrap();
    let original = comparison(&f);
    let reviewed = action(&original, "policy_004");
    f.refuses_without_append(
        &f.record(f.initial, "bare", "policy_004"),
        300,
        "FMINE-REF-105",
    );
    let blocked = action(&original, "policy_005");
    f.refuses_without_append(
        &f.record(f.initial, "pending-permit", &blocked),
        300,
        "FMINE-REF-105",
    );
    fs::write(
        &f.model,
        STAGED.replace("value = \"13000000\"", "value = \"12000000\""),
    )
    .unwrap();
    f.refuses_without_append(
        &f.record(f.initial, "wrong-comparison", &reviewed),
        300,
        "FMINE-REF-505",
    );

    let tail = STAGED.replace(
        "tail_mass = \"1/5\"",
        "tail_mass = \"1/5\"\nmax_tail_loss = \"4999999\"",
    );
    fs::write(&f.model, tail).unwrap();
    let rejected = action(&comparison(&f), "policy_004");
    f.refuses_without_append(
        &f.record(f.initial, "nominal-tail", &rejected),
        300,
        "FMINE-REF-105",
    );

    let mut stress = STAGED.replace(
        "tail_mass = \"1/5\"",
        "tail_mass = \"1/1\"\nmax_tail_loss = \"0\"",
    );
    stress.push_str("\n[[robustness]]\nmethod = \"wasserstein_1\"\nradius = \"1/4\"\nmetric_basis = \"Unit distance\"\nradius_basis = \"Synthetic weight stress\"\n");
    for from in ["cleared", "pending"] {
        for to in ["cleared", "pending"] {
            stress.push_str(&format!(
                "\n[[transport_distance]]\nfrom = \"{from}\"\nto = \"{to}\"\nvalue = \"{}/1\"\n",
                u8::from(from != to)
            ));
        }
    }
    fs::write(&f.model, stress).unwrap();
    let report = comparison(&f);
    assert_eq!(
        json_field(
            &report,
            &["payload", "program", "summary", "feasible_winners"]
        )
        .as_array()
        .unwrap()[0]
            .as_str(),
        Some("policy_004")
    );
    assert!(
        json_field(&report, &["payload", "journal_actions"])
            .as_array()
            .unwrap()
            .iter()
            .all(|a| a.get("policy_id").and_then(Json::as_str) != Some("policy_004"))
    );
    let rejected = action(&report, "policy_004");
    f.refuses_without_append(
        &f.record(f.initial, "robust-tail", &rejected),
        300,
        "FMINE-REF-105",
    );

    fs::write(&f.model, format!("{STAGED}#{}\n", "x".repeat(33_000))).unwrap();
    let oversized = action(&comparison(&f), "policy_004");
    f.refuses_without_append(
        &f.record(f.initial, "oversize", &oversized),
        300,
        "FMINE-REF-107",
    );
    successful(f.show(Some(f.initial)));
}
