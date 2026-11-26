//! Synthetic monitor-to-response controls; no real diagnosis or human authentication is asserted.
//! Determinism: BitwisePortable

use super::*;
use crate::ledger::CusumMonitorPolicy;
use crate::{
    AssessmentPolicy, CohortSpec, CoverageMonitorPolicy, ForecastPayload, OutcomeCitation,
    OutcomeKind, OutcomeSelector,
};
use fmine_core::hash::sha256;

fn citation(vintage: &str) -> OutcomeCitation {
    OutcomeCitation {
        source: "synthetic".into(),
        vintage: vintage.into(),
        locator: "synthetic:observation".into(),
        sha256: sha256(vintage.as_bytes()).0,
    }
}

fn cohort(
    ledger: &mut ForecastLedger,
    name: &str,
    registered_ns: i64,
    due_ns: i64,
    monitored: bool,
) -> (Digest32, Vec<ObjectId>) {
    let ids = (0..6)
        .map(|index| {
            ledger
                .register_payload(
                    format!("{name}:{index}"),
                    registered_ns,
                    due_ns,
                    ForecastPayload::Gaussian {
                        mean: 0.0,
                        std_dev: 1.0,
                    },
                )
                .expect("synthetic forecast")
        })
        .collect::<Vec<_>>();
    let policy = AssessmentPolicy {
        coverage_monitor: monitored.then(|| CoverageMonitorPolicy {
            alarm_alpha: 0.05,
            support: "synthetic period totals".into(),
            conditional_null: "declared independent Gaussian errors".into(),
            order: ids.clone(),
        }),
        cusum_monitor: monitored.then(|| CusumMonitorPolicy {
            reference_value: 0.5,
            threshold: 2.0,
            support: "synthetic period totals".into(),
            reference_assumption: "declared independent standard Gaussian errors".into(),
            tuning_basis: "arithmetic control; no false-alarm qualification".into(),
            order: ids.clone(),
        }),
        ..AssessmentPolicy::default()
    };
    let digest = ledger
        .freeze_cohort(CohortSpec {
            name: name.into(),
            frozen_at_ns: registered_ns + 1,
            members: ids.clone(),
            resolution_rule: "first synthetic observation".into(),
            eligible_sources: vec!["synthetic".into()],
            policy,
        })
        .expect("synthetic cohort");
    (digest, ids)
}

fn source() -> (ForecastLedger, Assessment, Vec<ObjectId>) {
    let mut ledger = ForecastLedger::new();
    let (digest, ids) = cohort(&mut ledger, "original", 1, 100, true);
    for (index, id) in ids.iter().enumerate() {
        ledger
            .append_outcome(
                *id,
                101,
                102,
                OutcomeKind::Published {
                    value: if index < 4 { 2.0 } else { 0.0 },
                    citation: citation("original"),
                },
            )
            .expect("original outcome");
    }
    let assessment = ledger
        .assess(digest, 103, OutcomeSelector::FirstPublished)
        .expect("original assessment")
        .clone();
    (ledger, assessment, ids)
}

fn opening(case_id: &str, opened_ns: i64) -> OpenCase {
    OpenCase {
        case_id: case_id.into(),
        owner_role: "site-metallurgist".into(),
        deadline_ns: opened_ns + 20,
        opened_ns,
        hypotheses: vec![
            "HYP-FEED: feed composition changed".into(),
            "HYP-ASSAY: sample preparation changed".into(),
        ],
        probes: vec![
            "Review retained sample split".into(),
            "Check feed log".into(),
        ],
    }
}

fn resolution(recorded_ns: i64) -> ResolveCase {
    ResolveCase {
        kind: ResolutionKind::DataDefect,
        human_role: "reviewing-metallurgist".into(),
        rationale: "A declared sample-label issue was reviewed".into(),
        evidence: "declared case note CASE-42; not authenticated".into(),
        recorded_ns,
    }
}

#[test]
fn cases_bind_first_original_crossing_and_deduplicate_across_restatements() {
    let (mut forecasts, assessment, ids) = source();
    let mut cases = InvestigationLedger::new();
    let cusum = cases
        .open(&assessment, MonitorKind::Cusum, opening("cusum", 104))
        .expect("CUSUM case")
        .clone();
    let coverage = cases
        .open(&assessment, MonitorKind::Coverage, opening("coverage", 104))
        .expect("coverage case")
        .clone();
    // k=0.5, h=2 and residuals 2,2 produce 1.5 then 3.0: the second original crosses.
    assert_eq!(cusum.trigger.first_crossing, 2);
    assert_eq!(cusum.trigger.member.forecast_id, ids[1]);
    // Three misses of the 80% interval give E_3 = 1 / (4 * 0.2^3) = 31.25 > 20.
    assert_eq!(coverage.trigger.first_crossing, 3);
    assert_eq!(coverage.trigger.member.forecast_id, ids[2]);
    for case in [&cusum, &coverage] {
        assert_eq!(case.assessment_digest, assessment.digest);
        assert_eq!(case.state(), "open");
        assert_eq!(case.digest, blake3_hash(&case.canonical_bytes()));
        assert!(matches!(
            &case.trigger.member.outcome,
            MemberOutcome::Resolved { value, citation, .. }
                if *value == 2.0 && citation.vintage == "original"
        ));
        assert!(!case.is_overdue(case.deadline_ns));
        assert!(case.is_overdue(case.deadline_ns + 1));
    }
    for id in &ids {
        forecasts
            .append_outcome(
                *id,
                105,
                106,
                OutcomeKind::Restated {
                    value: 0.0,
                    citation: citation("restated"),
                    supersedes: 1,
                },
            )
            .expect("explicit restatement");
    }
    let latest = forecasts
        .assess(assessment.cohort, 107, OutcomeSelector::LatestAsOf)
        .expect("restated assessment")
        .clone();
    assert_ne!(latest.digest, assessment.digest);
    assert!(latest.per_member.iter().all(|member| matches!(
        &member.outcome,
        MemberOutcome::Resolved { value, .. } if *value == 0.0
    )));
    for monitor in [MonitorKind::Coverage, MonitorKind::Cusum] {
        let before = cases.clone();
        assert_eq!(
            cases
                .open(&latest, monitor, opening("duplicate", 108))
                .expect_err("same original alarm")
                .code,
            "FMINE-REF-507"
        );
        assert_eq!(cases, before);
    }
    assert_eq!(cases.iter().count(), 2);
}

#[test]
fn invalid_opening_and_changed_assessments_are_atomic_and_do_not_reserve_ids_or_time() {
    let (_, assessment, _) = source();
    let mut cases = InvestigationLedger::new();
    let empty = cases.clone();
    let mut changed = assessment.clone();
    changed.summary.n_resolved -= 1;
    assert_eq!(
        cases
            .open(&changed, MonitorKind::Cusum, opening("case", 500))
            .expect_err("changed source assessment")
            .code,
        "FMINE-REF-505"
    );
    assert_eq!(cases, empty);
    let mut bad = Vec::new();
    let mut request = opening("case", 500);
    request.owner_role.clear();
    bad.push(request);
    let mut request = opening("case", 500);
    request.deadline_ns = request.opened_ns;
    bad.push(request);
    let mut request = opening("case", 500);
    request.hypotheses.truncate(1);
    bad.push(request);
    let mut request = opening("case", 500);
    request.hypotheses[1] = "HYP-FEED: another description".into();
    bad.push(request);
    let mut request = opening("case", 500);
    request.hypotheses[1] = "HYP-: missing label".into();
    bad.push(request);
    let mut request = opening("case", 500);
    request.hypotheses[1] = "HYP-UNEXPLAINED:".into();
    bad.push(request);
    let mut request = opening("case", 500);
    request.probes.clear();
    bad.push(request);
    let mut request = opening("case", 500);
    request.probes[1] = request.probes[0].clone();
    bad.push(request);
    let mut request = opening("case", 500);
    request.probes[0] = "Probe with\ncontrol character".into();
    bad.push(request);
    let mut request = opening("case", 500);
    request.owner_role = "x".repeat(129);
    bad.push(request);
    for request in bad {
        assert!(
            cases
                .open(&assessment, MonitorKind::Cusum, request)
                .is_err()
        );
        assert_eq!(cases, empty);
    }
    assert!(
        cases
            .open(&assessment, MonitorKind::Cusum, opening("case", 102))
            .is_err()
    );
    assert_eq!(cases, empty);
    cases
        .open(&assessment, MonitorKind::Cusum, opening("case", 104))
        .expect("refused future operations did not reserve id or time");
    let before = cases.clone();
    assert!(
        cases
            .open(&assessment, MonitorKind::Coverage, opening("case", 105))
            .is_err()
    );
    assert_eq!(cases, before);
}

#[test]
fn unavailable_and_not_alarmed_monitors_cannot_open_cases() {
    let mut forecasts = ForecastLedger::new();
    let (digest, ids) = cohort(&mut forecasts, "null", 1, 100, true);
    for id in ids {
        forecasts
            .append_outcome(
                id,
                101,
                102,
                OutcomeKind::Published {
                    value: 0.0,
                    citation: citation("null"),
                },
            )
            .expect("inside interval");
    }
    let assessment = forecasts
        .assess(digest, 103, OutcomeSelector::FirstPublished)
        .expect("no alarm")
        .clone();
    let mut cases = InvestigationLedger::new();
    for monitor in [MonitorKind::Coverage, MonitorKind::Cusum] {
        assert!(
            cases
                .open(&assessment, monitor, opening(monitor.as_str(), 104))
                .is_err()
        );
    }
    let (digest, _) = cohort(&mut forecasts, "unmonitored", 110, 200, false);
    let assessment = forecasts
        .assess(digest, 112, OutcomeSelector::FirstPublished)
        .expect("unmonitored assessment");
    for monitor in [MonitorKind::Coverage, MonitorKind::Cusum] {
        assert!(
            cases
                .open(assessment, monitor, opening(monitor.as_str(), 113))
                .is_err()
        );
    }
    assert_eq!(cases, InvestigationLedger::new());
}

#[test]
fn resolution_records_every_declared_outcome_without_revising_forecast_or_case_origin() {
    let (forecasts, assessment, _) = source();
    for kind in [
        ResolutionKind::HypothesisAdded,
        ResolutionKind::Recalibrated,
        ResolutionKind::DataDefect,
        ResolutionKind::FalseAlarmLogged,
    ] {
        let mut cases = InvestigationLedger::new();
        let opened = cases
            .open(&assessment, MonitorKind::Cusum, opening("case", 104))
            .expect("open")
            .clone();
        let mut request = resolution(150);
        request.kind = kind;
        let resolved = cases
            .resolve("case", request)
            .expect("late review remains explicit")
            .clone();
        assert_eq!(resolved.state(), "resolved");
        assert!(!resolved.is_overdue(200));
        assert_eq!(resolved.trigger, opened.trigger);
        assert_eq!(resolved.opening_digest, opened.opening_digest);
        assert_ne!(resolved.digest, opened.digest);
        assert_eq!(resolved.resolution.as_ref().expect("resolution").kind, kind);
        assert_eq!(resolved.digest, blake3_hash(&resolved.canonical_bytes()));
        let before = cases.clone();
        assert!(cases.resolve("case", resolution(151)).is_err());
        assert_eq!(cases, before);
        assert_eq!(
            ResolutionKind::parse(kind.as_str()).expect("round trip"),
            kind
        );
    }
    assert!(
        forecasts
            .cohort(assessment.cohort)
            .expect("original cohort")
            .spec
            .policy
            .cusum_monitor
            .is_some()
    );
    assert!(
        assessment
            .cusum_monitor
            .as_ref()
            .expect("original monitor")
            .snapshot
            .first_crossing()
            .is_some()
    );
    assert!(ResolutionKind::parse("automatic_retrain").is_err());
    assert!(MonitorKind::parse("invented").is_err());
}

#[test]
fn rearm_requires_resolved_case_and_a_fresh_prospective_monitor_cohort() {
    let (mut forecasts, assessment, _) = source();
    let mut cases = InvestigationLedger::new();
    cases
        .open(&assessment, MonitorKind::Cusum, opening("case", 104))
        .expect("open");
    let before = cases.clone();
    assert!(
        cases
            .rearm("case", &forecasts, assessment.cohort, 105)
            .is_err()
    );
    assert_eq!(cases, before);
    let (early, _) = cohort(&mut forecasts, "frozen-before-resolution", 110, 300, true);
    let resolved = cases
        .resolve("case", resolution(120))
        .expect("review")
        .clone();
    for digest in [assessment.cohort, early, Digest32([0; 32])] {
        let before = cases.clone();
        assert!(cases.rearm("case", &forecasts, digest, 121).is_err());
        assert_eq!(cases, before);
    }
    let (unmonitored, _) = cohort(&mut forecasts, "no-monitor", 125, 300, false);
    let before = cases.clone();
    assert!(cases.rearm("case", &forecasts, unmonitored, 127).is_err());
    assert_eq!(cases, before);
    let (next, ids) = cohort(&mut forecasts, "next-monitor", 130, 300, true);
    for at in [119, 130, 300] {
        let before = cases.clone();
        assert!(cases.rearm("case", &forecasts, next, at).is_err());
        assert_eq!(cases, before);
    }
    let mut with_outcome = forecasts.clone();
    with_outcome
        .append_outcome(
            ids[0],
            301,
            302,
            OutcomeKind::Published {
                value: 5.0,
                citation: citation("new-observation"),
            },
        )
        .expect("new cohort observation");
    let before = cases.clone();
    assert!(cases.rearm("case", &with_outcome, next, 132).is_err());
    assert_eq!(cases, before);
    let rearmed = cases
        .rearm("case", &forecasts, next, 132)
        .expect("new prospective cohort")
        .clone();
    assert_eq!(rearmed.state(), "rearmed");
    assert_eq!(rearmed.rearm.as_ref().expect("rearm").cohort, next);
    assert_eq!(rearmed.trigger, resolved.trigger);
    assert_eq!(rearmed.resolution, resolved.resolution);
    assert_ne!(rearmed.digest, resolved.digest);
    assert_eq!(rearmed.digest, blake3_hash(&rearmed.canonical_bytes()));
    let before = cases.clone();
    assert!(cases.rearm("case", &forecasts, next, 133).is_err());
    assert_eq!(cases, before);
    assert!(
        assessment
            .cusum_monitor
            .as_ref()
            .expect("original monitor")
            .snapshot
            .first_crossing()
            .is_some()
    );
}

#[test]
fn each_retained_declaration_is_bound_and_failed_resolution_preserves_chronology() {
    let (_, assessment, _) = source();
    let mut cases = InvestigationLedger::new();
    let opened = cases
        .open(&assessment, MonitorKind::Cusum, opening("case", 104))
        .expect("open")
        .clone();
    let before = cases.clone();
    let mut empty_evidence = resolution(500);
    empty_evidence.evidence.clear();
    assert!(cases.resolve("case", empty_evidence).is_err());
    assert_eq!(cases, before);
    assert!(cases.resolve("unknown", resolution(500)).is_err());
    assert_eq!(cases, before);
    assert!(cases.resolve("case", resolution(103)).is_err());
    assert_eq!(cases, before);
    let resolved = cases
        .resolve("case", resolution(105))
        .expect("refused operations did not advance time")
        .clone();
    let mut changed = resolved.clone();
    changed
        .resolution
        .as_mut()
        .expect("resolution")
        .human_role
        .push_str("-changed");
    assert_ne!(changed.canonical_bytes(), resolved.canonical_bytes());
    let mut changed = opened.clone();
    changed.owner_role.push_str("-changed");
    assert_ne!(changed.canonical_bytes(), opened.canonical_bytes());
    let mut changed = opened.clone();
    changed.deadline_ns += 1;
    assert_ne!(changed.canonical_bytes(), opened.canonical_bytes());
    let mut changed = opened.clone();
    changed.hypotheses[0].push_str(" further");
    assert_ne!(changed.canonical_bytes(), opened.canonical_bytes());
    let mut changed = opened.clone();
    changed.probes.swap(0, 1);
    assert_ne!(changed.canonical_bytes(), opened.canonical_bytes());
    let mut replay = InvestigationLedger::new();
    replay
        .open(&assessment, MonitorKind::Cusum, opening("case", 104))
        .expect("replay opening");
    replay
        .resolve("case", resolution(105))
        .expect("replay review");
    assert_eq!(replay, cases);
}
