use super::*;

const FIXTURE: &str = include_str!("../../../tests/forecasts/cohort_synthetic.toml");
const CUT: &str = "2026-10-31T00:00:00Z";
const EARLY_CUT: &str = "2026-09-30T00:00:00Z";

fn report(text: &str, cutoff: &str, selector: OutcomeSelector) -> (Replay, Assessment) {
    let mut replay = Replay::parse(text).expect("valid synthetic transcript");
    let report = replay
        .assessment("synthetic-demo", time(cutoff).expect("cut"), selector)
        .expect("assessment");
    (replay, report)
}

#[test]
fn synthetic_restatements_keep_the_frozen_denominator_and_change_the_selected_score() {
    let (replay, first) = report(FIXTURE, CUT, OutcomeSelector::FirstPublished);
    let (_, latest) = report(FIXTURE, CUT, OutcomeSelector::LatestAsOf);
    assert_eq!(first.summary.n_members, 3);
    assert_eq!(first.summary.n_resolved, 2);
    assert_eq!(first.summary.n_unresolved, 1);
    assert_eq!(
        first.summary.unresolved_by_reason[&MissingReason::NotPublished],
        1
    );
    assert_eq!(
        first.summary.coverage.expect("coverage").whole_cohort,
        (2.0 / 3.0, 1.0)
    );
    assert_eq!(
        latest.summary.coverage.expect("coverage").whole_cohort,
        (1.0 / 3.0, 2.0 / 3.0)
    );
    assert_eq!(
        first.summary.interval_score.expect("score").observed_mean,
        Some(17.0)
    );
    assert_eq!(
        latest.summary.interval_score.expect("score").observed_mean,
        Some(27.0)
    );
    assert_eq!(
        first
            .summary
            .interval_score
            .expect("score")
            .whole_cohort
            .lower,
        18.0
    );
    assert_eq!(
        latest
            .summary
            .interval_score
            .expect("score")
            .whole_cohort
            .lower,
        74.0 / 3.0
    );
    assert_eq!(
        latest
            .summary
            .interval_score
            .expect("score")
            .whole_cohort
            .upper,
        None
    );
    assert_eq!(latest.summary.effective_n.lower, 2.0 / 1.5);
    assert_eq!(latest.summary.effective_n.upper, 2.0);
    assert_eq!(latest.summary.flag, CalibrationFlag::Descriptive);
    let id = replay.forecast_id("north-unit-cost").expect("north");
    let value = |a: &Assessment| match a
        .per_member
        .iter()
        .find(|m| m.forecast_id == id)
        .expect("member")
        .outcome
    {
        MemberOutcome::Resolved {
            value, event_seq, ..
        } => (value, event_seq),
        _ => panic!("published member"),
    };
    assert_eq!(value(&first), (1479.0, 1));
    assert_eq!(value(&latest), (1474.0, 2));
    assert_ne!(first.digest, latest.digest);
}

#[test]
fn checkpoints_replay_and_validate_a_stable_historical_digest() {
    let prefix = FIXTURE.rsplit_once("[[outcome]]").expect("restatement").0;
    let (_, before) = report(prefix, EARLY_CUT, OutcomeSelector::FirstPublished);
    let (after_replay, after) = report(FIXTURE, EARLY_CUT, OutcomeSelector::FirstPublished);
    assert_eq!(before.canonical_bytes(), after.canonical_bytes());
    assert_eq!(before.digest, after.digest);
    assert_eq!(after_replay.checkpoints[0].digest, after.digest);
    assert_ne!(sha256(prefix.as_bytes()), sha256(FIXTURE.as_bytes()));
    let pinned = FIXTURE.replace(
        "selector = \"first-published\"",
        &format!(
            "selector = \"first-published\"\nexpected_digest = \"{}\"",
            before.digest
        ),
    );
    let (replay, report) = report(&pinned, CUT, OutcomeSelector::LatestAsOf);
    assert_eq!(replay.checkpoints[0].expected, Some(before.digest));
    assert!(
        replay
            .render(&report, sha256(pinned.as_bytes()))
            .expect("JSON")
            .contains("\"expected_digest_matched\":true")
    );
    let bad = pinned.replace(&before.digest.to_hex(), &"0".repeat(64));
    assert_eq!(
        Replay::parse(&bad).expect_err("altered checkpoint").code,
        "FMINE-REF-505"
    );
}

#[test]
fn assessment_checkpoint_seals_survive_replay_without_sorting_away_backdates() {
    let backdated = FIXTURE
        .replace("2026-10-15T00:00:00Z", "2026-09-20T00:00:00Z")
        .replace("2026-10-15T01:00:00Z", "2026-09-20T01:00:00Z");
    let refusal = Replay::parse(&backdated).expect_err("write into sealed September cut");
    assert_eq!(refusal.code, "FMINE-REF-206");
    assert!(refusal.what.contains("[[outcome]]"));
    assert!(refusal.what.contains("assessed cut"));
    let added_forecast = "\n[[forecast]]\nid = \"hidden\"\nmodel_key = \"hidden/model\"\nregistered_at = \"2026-09-01T00:00:00Z\"\noutcome_due = \"2026-12-31T00:00:00Z\"\nkind = \"bernoulli\"\nprobability = \"0.5\"\n";
    let prefix = FIXTURE.rsplit_once("[[outcome]]").expect("last outcome").0;
    assert_eq!(
        Replay::parse(&format!("{prefix}{added_forecast}"))
            .expect_err("global sealed cut")
            .code,
        "FMINE-REF-206"
    );
}

fn single(payload: &str, policy: &str, outcome: &str) -> String {
    format!(
        r#"schema = "fmine.forecast-ledger.v1"
lane = "synthetic"
[[forecast]]
id = "one"
model_key = "synthetic/model"
registered_at = "2026-01-01T00:00:00Z"
outcome_due = "2026-03-31T00:00:00Z"
{payload}
[[cohort]]
name = "synthetic-demo"
frozen_at = "2026-01-02T00:00:00Z"
members = "one"
resolution_rule = "Synthetic exact outcome"
eligible_sources = "synthetic"
{policy}
{outcome}
"#
    )
}

fn published(value: &str) -> String {
    format!(
        r#"[[outcome]]
forecast = "one"
kind = "published"
available_at = "2026-04-01T00:00:00Z"
recorded_at = "2026-04-02T00:00:00Z"
value = "{value}"
source = "synthetic"
vintage = "v1"
locator = "Invented!A1"
sha256 = "1111111111111111111111111111111111111111111111111111111111111111"
"#
    )
}

#[test]
fn all_distribution_types_use_the_existing_kernel_and_frozen_policy() {
    let gaussian = single(
        "kind = \"gaussian\"\nmean = \"0\"\nstd_dev = \"1\"",
        "",
        &published("0"),
    );
    let (_, a) = report(&gaussian, CUT, OutcomeSelector::LatestAsOf);
    let crps = a.summary.crps.expect("CRPS").observed_mean.expect("scored");
    assert!((crps - 0.233_694_977_255_109_13).abs() < 1e-12);
    assert_eq!(
        a.summary.coverage.expect("coverage").observed_fraction,
        Some(1.0)
    );
    let quantiles = single(
        "kind = \"quantiles\"\nlevels = \"0.1,0.5,0.9\"\nvalues = \"-1,0,1\"",
        "",
        &published("0"),
    );
    let (replay, a) = report(&quantiles, CUT, OutcomeSelector::LatestAsOf);
    assert_eq!(
        a.summary.interval_score.expect("score").observed_mean,
        Some(2.0)
    );
    assert_eq!(a.summary.crps, None);
    assert!(
        replay
            .render(&a, sha256(quantiles.as_bytes()))
            .expect("render")
            .contains("\"levels\":[0.1,0.5,0.9]")
    );
    let bernoulli = single(
        "kind = \"bernoulli\"\nprobability = \"0.25\"",
        "",
        &published("1"),
    );
    let (_, a) = report(&bernoulli, CUT, OutcomeSelector::LatestAsOf);
    assert_eq!(a.summary.brier.expect("Brier").observed_mean, Some(0.5625));
    assert_eq!(a.summary.coverage, None);
    assert_eq!(a.summary.flag, CalibrationFlag::Descriptive);
    let bounded = single(
        "kind = \"interval\"\nlo = \"-1\"\nhi = \"1\"",
        "outcome_lo = \"-2\"\noutcome_hi = \"2\"",
        "",
    );
    let (_, a) = report(&bounded, CUT, OutcomeSelector::LatestAsOf);
    let score = a.summary.interval_score.expect("bounded score");
    assert_eq!(score.n_scored, 0);
    assert_eq!(score.observed_mean, None);
    assert_eq!(score.whole_cohort.lower, 2.0);
    assert_eq!(score.whole_cohort.upper, Some(12.0));
    assert_eq!(a.summary.flag, CalibrationFlag::Unscored);
}

#[test]
fn unobserved_members_remain_pending_or_overdue_with_bounded_brier() {
    let input = single("kind = \"bernoulli\"\nprobability = \"0.25\"", "", "");
    let (_, pending) = report(&input, "2026-02-01T00:00:00Z", OutcomeSelector::LatestAsOf);
    let (_, overdue) = report(&input, CUT, OutcomeSelector::LatestAsOf);
    assert_eq!(
        pending.summary.unresolved_by_reason[&MissingReason::Pending],
        1
    );
    assert_eq!(
        overdue.summary.unresolved_by_reason[&MissingReason::Overdue],
        1
    );
    assert_eq!(overdue.per_member[0].selected_event, None);
    let score = overdue.summary.brier.expect("Brier bounds");
    assert_eq!(score.whole_cohort.lower, 0.0625);
    assert_eq!(score.whole_cohort.upper, Some(0.5625));
    assert_eq!(score.observed_mean, None);
}

#[test]
fn selection_needs_both_availability_and_record_time() {
    let input = single(
        "kind = \"bernoulli\"\nprobability = \"0.25\"",
        "",
        &published("1"),
    );
    let (_, before) = report(&input, "2026-04-01T12:00:00Z", OutcomeSelector::LatestAsOf);
    let (_, after) = report(&input, "2026-04-02T00:00:00Z", OutcomeSelector::LatestAsOf);
    assert_eq!(before.summary.n_resolved, 0);
    assert_eq!(after.summary.n_resolved, 1);
    assert_ne!(before.digest, after.digest);
}

#[test]
fn strict_import_rejects_ambiguous_or_unsupported_transcripts() {
    let cases = [
        FIXTURE.replace(SCHEMA, "v0"),
        FIXTURE.replace("lane = \"synthetic\"", "lane = \"public\""),
        FIXTURE.replace("lane = \"synthetic\"", ""),
        FIXTURE.replacen("[[forecast]]", "[[forecats]]", 1),
        FIXTURE.replacen("lo = \"1476\"", "lo = 1476", 1),
        FIXTURE.replacen("lo = \"1476\"", "lo = \"NaN\"", 1),
        FIXTURE.replacen("lo = \"1476\"", "lo = \"inf\"", 1),
        FIXTURE.replacen("lo = \"1476\"", "probability = \"0.2\"", 1),
        FIXTURE.replacen("id = \"north-unit-cost\"", "id = \"north,unit-cost\"", 1),
        FIXTURE.replacen("2026-07-01T00:00:00Z", "2026-02-30T00:00:00Z", 1),
        FIXTURE.replacen("2026-07-01T00:00:00Z", "2026-07-01T00:00:00+00:00", 1),
        FIXTURE.replacen("2026-07-01T00:00:00Z", "9999-07-01T00:00:00Z", 1),
        FIXTURE.replace(
            "eligible_sources = \"synthetic:issuer\"",
            "eligible_sources = \"synthetic:issuer,\"",
        ),
        FIXTURE.replace("min_calibration_n = 35", "min_calibration_n = \"35\""),
        FIXTURE.replace("min_calibration_n = 35", "min_calibration_n = -1"),
        FIXTURE.replace("rho_low = \"0\"", "outcome_lo = \"0\""),
        FIXTURE.replace("reason = \"not_published\"", "reason = \"lost\""),
        FIXTURE.replace("supersedes = 1", "supersedes = \"1\""),
        FIXTURE.replace("supersedes = 1", "supersedes = -1"),
        FIXTURE.replace("selector = \"first-published\"", "selector = \"latest\""),
        FIXTURE.replace(
            "sha256 = \"2222222222222222222222222222222222222222222222222222222222222222\"",
            "sha256 = \"short\"",
        ),
    ];
    for input in cases {
        let e = Replay::parse(&input).expect_err("invalid transcript");
        assert_eq!(e.code, "FMINE-REF-107", "{}", e.what);
    }
}

#[test]
fn kernel_refusals_keep_their_original_codes_at_the_io_boundary() {
    let cases = [
        (
            FIXTURE.replacen("id = \"central-unit-cost\"", "id = \"north-unit-cost\"", 1),
            "FMINE-REF-507",
        ),
        (
            FIXTURE.replace(
                "members = \"north-unit-cost, central-unit-cost, south-unit-cost\"",
                "members = \"unknown\"",
            ),
            "FMINE-REF-201",
        ),
        (
            FIXTURE.replace(
                "members = \"north-unit-cost, central-unit-cost, south-unit-cost\"",
                "members = \"north-unit-cost,north-unit-cost\"",
            ),
            "FMINE-REF-109",
        ),
        (
            FIXTURE.replacen(
                "source = \"synthetic:issuer\"",
                "source = \"ineligible\"",
                1,
            ),
            "FMINE-REF-201",
        ),
        (
            FIXTURE.replace("min_calibration_n = 35", "min_calibration_n = 1"),
            "FMINE-REF-109",
        ),
        (
            FIXTURE.replace("supersedes = 1", "supersedes = 0"),
            "FMINE-REF-507",
        ),
        (
            FIXTURE.replace("supersedes = 1", "supersedes = 2"),
            "FMINE-REF-507",
        ),
        (
            FIXTURE.replace("2026-10-15T00:00:00Z", "2026-10-16T00:00:00Z"),
            "FMINE-REF-206",
        ),
        (
            FIXTURE.replace(
                "frozen_at = \"2026-07-02T00:00:00Z\"",
                "frozen_at = \"2026-09-01T00:00:00Z\"",
            ),
            "FMINE-REF-206",
        ),
    ];
    for (input, code) in cases {
        let refusal = Replay::parse(&input).expect_err("kernel rejection");
        assert_eq!(refusal.code, code, "{}", refusal.what);
        assert!(refusal.what.contains("line "));
    }
    let mut replay = Replay::parse(FIXTURE).expect("fixture");
    assert_eq!(
        replay
            .assessment(
                "absent",
                time(CUT).expect("time"),
                OutcomeSelector::FirstPublished
            )
            .expect_err("unknown cohort")
            .code,
        "FMINE-REF-201"
    );
    assert_eq!(
        replay
            .assessment(
                "synthetic-demo",
                time("2026-01-01T00:00:00Z").expect("time"),
                OutcomeSelector::FirstPublished
            )
            .expect_err("before freeze")
            .code,
        "FMINE-REF-206"
    );
}

#[test]
fn file_adapter_is_deterministic_read_only_and_explicit_about_unverified_inputs() {
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/forecasts/cohort_synthetic.toml");
    let before = std::fs::read(&path).expect("fixture bytes");
    let output =
        assess(&path, "synthetic-demo", CUT, "latest-as-of", None).expect("file assessment");
    assert_eq!(
        output,
        assess(&path, "synthetic-demo", CUT, "latest-as-of", None).expect("repeated")
    );
    assert_eq!(before, std::fs::read(&path).expect("still identical"));
    for expected in [
        "\"lane\":\"synthetic\"",
        "\"mode\":\"read_only_replay\"",
        "\"authority_times\":\"supplied_not_verified\"",
        "\"source_citations\":\"supplied_not_verified\"",
        "\"external_time_proof\":\"not_verified\"",
        "\"temporal_validation\":{\"status\":\"not_checked\"",
        "\"calibration_flag\":\"descriptive\"",
        "\"upper\":null,\"upper_unbounded\":true",
        "\"bounds_kind\":\"whole_cohort_completion_bounds_not_confidence_intervals\"",
        "\"reason\":\"not_published\"",
        "\"kind\":\"restated\",\"supersedes\":1",
    ] {
        assert!(output.contains(expected), "missing {expected}");
    }
    assert!(output.contains(&sha256(&before).to_hex()));
    let supplied = FIXTURE.replace("lane = \"synthetic\"", "lane = \"supplied\"");
    let (replay, a) = report(&supplied, CUT, OutcomeSelector::LatestAsOf);
    assert!(
        replay
            .render(&a, sha256(supplied.as_bytes()))
            .expect("supplied report")
            .contains("\"lane\":\"supplied\"")
    );
    assert_eq!(
        assess(
            &path.with_file_name("does-not-exist.toml"),
            "synthetic-demo",
            CUT,
            "latest-as-of",
            None
        )
        .expect_err("absent file")
        .refusal
        .code,
        "FMINE-REF-506"
    );
    assert_eq!(
        assess(&path, "synthetic-demo", CUT, "latest", None)
            .expect_err("bad selector")
            .refusal
            .code,
        "FMINE-REF-107"
    );
    assert_eq!(
        js("quoted \"locator\"\n\\"),
        "\"quoted \\\"locator\\\"\\n\\\\\""
    );
}
