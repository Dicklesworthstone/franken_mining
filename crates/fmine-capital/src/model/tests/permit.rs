//! FACT: synthetic staged permit known answers through the source-bound public entry point.
//! Determinism: Exact

use super::*;
use crate::permit::GateState;

const CASE: &str = include_str!("../../../../../tests/capital/staged_permit.toml");

fn report(text: &str) -> EvaluatedRecourseProgram {
    let EvaluatedModel::Recourse(report) = evaluate(text, source(text)).unwrap() else {
        panic!("staged permit report required")
    };
    *report
}

fn review<'a>(report: &'a EvaluatedRecourseProgram, state: &str) -> &'a crate::permit::Review {
    &report.static_program().summary().permit_reviews[&("NRC-42".into(), state.into())]
}

#[test]
fn permit_blocks_precommitment_and_keeps_the_paid_option_in_the_pending_path() {
    let output = report(CASE);
    let summary = output.summary();
    assert_eq!(
        summary.net_adaptive_value.unwrap().major,
        Rat::from_integer(1_500_000)
    );
    assert_eq!(summary.feasible_winners.len(), 1);
    let winner = summary
        .policies
        .iter()
        .find(|p| summary.feasible_winners.contains(&p.policy_id))
        .unwrap();
    assert_eq!(winner.initial_selected, ["early"]);
    assert_eq!(winner.initial_spend.major, Rat::from_integer(6_000_000));
    assert_eq!(winner.states[0].selected, ["early", "main"]);
    assert_eq!(winner.states[0].net_nav.major, Rat::from_integer(8_000_000));
    assert_eq!(winner.states[1].selected, ["early"]);
    assert_eq!(
        winner.states[1].net_nav.major,
        Rat::from_integer(-5_000_000)
    );
    assert_eq!(winner.states[1].spend.major, Rat::from_integer(6_000_000));
    let permit = output.inputs().program.permits.as_ref().unwrap();
    assert!(output.claim().parents.contains(&permit.claim_id));
    assert!(
        output
            .static_program()
            .claim()
            .parents
            .contains(&permit.claim_id)
    );
    let baseline = output.static_program().summary();
    assert_eq!(baseline.feasible_winners, ["bundle_00"]);
    let committed = baseline
        .choices
        .iter()
        .find(|c| c.selected == ["early", "main"])
        .unwrap();
    assert!(
        matches!(committed.structural_rejections.as_slice(), [StructuralRejection::PermitUnavailable { state_id, .. }] if state_id == "pending")
    );
    let pending = review(&output, "pending");
    assert_eq!(pending.state, GateState::Elapsed);
    assert!(pending.deeming_review_due);
    assert_eq!(pending.clock_deadline.as_deref(), Some("2030-04-01"));
    assert_eq!(pending.paused_days, 10);
    assert_eq!(
        review(&output, "cleared").state,
        GateState::PermissionDocumented
    );
}

#[test]
fn shared_observations_and_reopened_or_refused_scope_cannot_unlock_main_works() {
    for (text, status) in [
        (
            CASE.replace("\"no-clearance-documented\"", "\"clearance-documented\""),
            GateState::PermissionDocumented,
        ),
        (
            CASE.replacen(
                "scope_change_pct = \"7/1\"",
                "scope_change_pct = \"10/1\"",
                1,
            ),
            GateState::RequiresReopenReview,
        ),
        (
            CASE.replace(
                "status = \"deeming_confirmed\"",
                "status = \"written_refusal\"",
            ),
            GateState::Refused,
        ),
    ] {
        let output = report(&text);
        assert_eq!(review(&output, "cleared").state, status);
        assert_eq!(output.summary().net_adaptive_value.unwrap().major, ZERO);
        for policy in output
            .summary()
            .policies
            .iter()
            .filter(|p| p.structural_rejections.is_empty())
        {
            assert!(
                policy
                    .states
                    .iter()
                    .all(|state| !state.selected.iter().any(|id| id == "main"))
            );
        }
    }
    let valid = CASE.replace(
        "status = \"deeming_confirmed\"",
        "status = \"written_grant\"",
    );
    assert_eq!(
        report(&valid).summary().net_adaptive_value.unwrap().major,
        Rat::from_integer(1_500_000)
    );
    let all_cleared = valid.replace("status = \"pending\"", "status = \"written_grant\"");
    let all_cleared = report(&all_cleared);
    assert_eq!(
        all_cleared.summary().net_adaptive_value.unwrap().major,
        ZERO
    );
    assert_eq!(
        all_cleared.static_program().summary().feasible_winners,
        ["bundle_03"]
    );
}

#[test]
fn main_works_require_every_bound_application_and_retain_unavailable_review_details() {
    let rows = CASE.split_once("[[permit]]").unwrap().1;
    let secondary = rows.replace("NRC-42", "NRC-43").replace(
        "status = \"deeming_confirmed\"",
        "status = \"written_refusal\"",
    );
    let both = report(&format!("{CASE}\n[[permit]]{secondary}"));
    assert_eq!(both.summary().net_adaptive_value.unwrap().major, ZERO);
    let static_summary = both.static_program().summary();
    assert_eq!(static_summary.permit_reviews.len(), 4);
    assert_eq!(
        static_summary.permit_reviews[&("NRC-43".into(), "cleared".into())].state,
        GateState::Refused
    );
    for policy in &both.summary().policies {
        for state in policy
            .states
            .iter()
            .filter(|s| s.selected.iter().any(|candidate| candidate == "main"))
        {
            assert!(policy.structural_rejections.iter().any(|r| r.state_id == state.state_id &&
                matches!(&r.reason, StructuralRejection::PermitUnavailable { application, .. } if application == "NRC-43")));
        }
    }
}

#[test]
fn malformed_or_unavailable_permit_conditions_refuse_before_optimization() {
    let (prefix, remaining) = CASE
        .split_once("[[permit_state]]\napplication = \"NRC-42\"\nstate = \"pending\"")
        .unwrap();
    let (_, tail) = remaining.split_once("[[permit_pause]]").unwrap();
    let without_pending = format!("{prefix}[[permit_pause]]{tail}");
    for (text, message) in [
        (
            CASE.replacen(
                "effective_on = \"2030-04-01\"",
                "effective_on = \"2030-03-31\"",
                1,
            ),
            "at or after",
        ),
        (
            CASE.replacen("known_on = \"2030-04-02\"", "known_on = \"2030-04-03\"", 1),
            "available at the decision cutoff",
        ),
        (
            CASE.replace(
                "[[stage_permit]]\ncandidate = \"main\"",
                "[[stage_permit]]\ncandidate = \"early\"",
            ),
            "recourse-stage candidate",
        ),
        (
            CASE.replacen("resumed_on = \"2030-03-02\"", "resumed_on = \"open\"", 1),
            "running clock",
        ),
        (
            format!("{CASE}\n[[stage_permit]]\ncandidate = \"main\"\napplication = \"NRC-42\"\n"),
            "duplicate candidate permit",
        ),
        (without_pending, "complete application-by-state matrix"),
    ] {
        let refusal = evaluate(&text, source(&text)).unwrap_err();
        assert!(refusal.what.contains(message), "{}", refusal.what);
    }
}
