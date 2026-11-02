use super::*;

const REGISTER: &str = "[[forecast]]\nid = \"one\"\nmodel_key = \"synthetic\"\noutcome_due = \"2030-01-02T00:00:00Z\"\nkind = \"interval\"\nlo = \"40\"\nhi = \"50\"\nnominal = \"0.8\"\n";
const FREEZE: &str = "[[cohort]]\nname = \"test\"\nmembers = \"one\"\nresolution_rule = \"Invented source value\"\neligible_sources = \"synthetic:issuer\"\n";

fn semantic_record(request: String, observed_ns: i64) -> Record {
    Record {
        sequence: 1,
        previous: Digest32::default(),
        observed_ns,
        entry: Entry::Operation(request),
    }
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
