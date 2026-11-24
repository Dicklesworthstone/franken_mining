//! DESIGN (bd-w770): source-bound staged commitments in the existing decision journal.
//! Determinism: Exact

use fmine_capital::recourse::{EvaluatedRecourseProgram, PolicyChoice};
use fmine_core::Digest32;
use fmine_core::anchor::SourceAnchor;
use fmine_core::hash::sha256;
use fmine_evidence::{Refusal, RefusalCode};
use fmine_io::omf::json::{Json, parse};

use crate::json_string as js;

const PREFIX: &str = r#"{"schema":"fmine.capital-policy-decision.v1","#;

fn eligible(program: &EvaluatedRecourseProgram) -> impl Iterator<Item = &PolicyChoice> {
    let summary = program.summary();
    summary.policies.iter().filter(|policy| {
        policy.structural_rejections.is_empty()
            && policy.statistics.is_some()
            && !policy.risk_rejected
            && summary.robustness.as_ref().is_none_or(|stress| {
                stress.declared_tail_limits == 0
                    || stress.robust_tail_feasible.contains(&policy.policy_id)
            })
    })
}

fn ids<'a>(values: impl IntoIterator<Item = &'a str>) -> String {
    format!(
        "[{}]",
        values.into_iter().map(js).collect::<Vec<_>>().join(",")
    )
}

pub(super) fn actions_json(program: &EvaluatedRecourseProgram) -> String {
    let actions = eligible(program)
        .map(|policy| {
            let id = format!("{}@{}", policy.policy_id, program.digest());
            format!(
                r#"{{"id":{},"policy_id":{},"initial_selected":{}}}"#,
                js(&id),
                js(&policy.policy_id),
                ids(policy.initial_selected.iter().map(String::as_str)),
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!("[{actions}]")
}

pub(super) fn snapshot(
    program: &EvaluatedRecourseProgram,
    source: &SourceAnchor,
    action: &str,
) -> Result<(String, String), Refusal> {
    let unavailable = || {
        Refusal::new(
            RefusalCode::BasisMismatch105,
            "staged capital journal action must be an eligible digest-bound id from decide program --json journal_actions",
        )
    };
    let (id, reviewed) = action.split_once('@').ok_or_else(unavailable)?;
    if reviewed != program.digest() {
        return Err(Refusal::new(
            RefusalCode::DigestMismatch505,
            "staged capital comparison differs from the reviewed action id; evaluate the worksheet again before recording",
        ));
    }
    let eligible_ids: Vec<_> = eligible(program)
        .map(|policy| policy.policy_id.as_str())
        .collect();
    if !eligible_ids.contains(&id) {
        return Err(unavailable());
    }
    let policy = program.policy_json(id).ok_or_else(unavailable)?;
    let claim_id = program.claim().claim_id.to_string();
    let summary = program.summary();
    let winners = ids(summary.feasible_winners.iter().map(String::as_str));
    let objective_unit = if summary.per_share_objective {
        "USD/share"
    } else {
        "USD"
    };
    let artifact = format!(
        r#"{PREFIX}"decision_stage":"initial","model_sha256":{},"comparison_digest":{},"claim_id":{},"observation_date_utc":{},"spending_unit":"USD","objective_unit":{},"valuation_basis":{},"eligible_policy_ids":{},"nominal_winners":{winners},"policy":{policy},"reserve_disclosure_claim":false,"execution_authorized":false,"geotechnical_signoff":false,"regulatory_compliance_claim":false}}"#,
        js(&Digest32(source.artifact_sha256).to_hex()),
        js(reviewed),
        js(&claim_id),
        js(&summary.information.observation_date),
        js(objective_unit),
        js(&summary.information.valuation_basis),
        ids(eligible_ids),
    );
    Ok((artifact, claim_id))
}

/// Check the existing retained artifact at its recorded clock bound without opening its source file.
pub(crate) fn validate_at(
    card: &str,
    model: &str,
    action: &str,
    claim_id: &str,
    latest_ns: i64,
) -> Result<(), Refusal> {
    if !card.starts_with(PREFIX) {
        return Ok(());
    }
    let mismatch = || {
        Refusal::new(
            RefusalCode::DigestMismatch505,
            "retained staged decision disagrees with its action, worksheet or comparison Claim",
        )
    };
    let retained = parse(card.as_bytes()).map_err(|_| mismatch())?;
    let (id, comparison) = action.split_once('@').ok_or_else(mismatch)?;
    let eligible = retained
        .get("eligible_policy_ids")
        .and_then(Json::as_array)
        .is_some_and(|ids| ids.iter().any(|v| v.as_str() == Some(id)));
    if !eligible
        || retained.get("decision_stage").and_then(Json::as_str) != Some("initial")
        || retained.get("comparison_digest").and_then(Json::as_str) != Some(comparison)
        || retained.get("claim_id").and_then(Json::as_str) != Some(claim_id)
        || retained.get("model_sha256").and_then(Json::as_str)
            != Some(sha256(model.as_bytes()).to_hex().as_str())
        || retained
            .get("policy")
            .and_then(|p| p.get("policy_id"))
            .and_then(Json::as_str)
            != Some(id)
    {
        return Err(mismatch());
    }
    let cutoff = retained
        .get("observation_date_utc")
        .and_then(Json::as_str)
        .and_then(fmine_calib::record::parse_date)
        .ok_or_else(mismatch)?;
    if i128::from(latest_ns) >= i128::from(cutoff) * 86_400_000_000_000 {
        return Err(Refusal::new(
            RefusalCode::UndeclaredUnitBasisOrTz107,
            "the latest possible recording time must precede the staged observation date (UTC)",
        ));
    }
    Ok(())
}
