//! DESIGN (BD-051, plan §10.5): durable responses to saved original-publication monitor alarms.
//! Cases record declared responsibility and human review; they neither authenticate a reviewer nor retrain.

use std::fmt::Write as _;

use fmine_calib::ledger::investigation::{MonitorKind, OpenCase, ResolutionKind, ResolveCase};
use fmine_evidence::Refusal;

use super::{Replay, Row, hex, invalid, js, live, utc};

pub(super) const ROBOT_DOCS_JSON: &str = r#"{"schema":"fmine.forecast-investigations.v1","commands":["forecast journal investigate","forecast journal resolve-investigation","forecast journal rearm-investigation"],"writer":"Each takes --journal, --head, --request and --trust-host-clock. The writer assigns all event times; requests cannot backdate them.","open_table":"investigation","open_fields":["id","assessment","monitor","owner_role","deadline","hypotheses","probes"],"source":"assessment is the exact digest of an already persisted, result-bound assessment in this journal. monitor is coverage or cusum. The original first crossing and outcome event are derived from that assessment, including when its descriptive scores use restatements. A repeated original alarm cannot open another case.","declarations":"deadline is a future UTC instant. hypotheses is a pipe-separated list of at least two HYP-labelled competing explanations; probes is a pipe-separated list in the owner's declared priority order. No expected-loss reduction or VOI ranking is computed or independently verified.","resolution_table":"investigation_resolution","resolution_fields":["id","kind","human_review","human_role","rationale","evidence"],"resolution_kinds":["hypothesis_added","recalibrated","data_defect","false_alarm_logged"],"review":"human_review must explicitly be human. This records an unverified role declaration and cited explanation, not authentication, permission or proof that a corrective action occurred. It does not amend any forecast or clear an alarm.","rearm_table":"investigation_rearm","rearm_fields":["id","cohort"],"rearm":"After resolution, link a distinct newly frozen prospective cohort with the same monitor kind and no recorded outcomes. Original monitoring history and calibration flags are never reset.","read":"forecast journal inspect exposes retained cases, original alarm evidence, resolutions, successor cohorts and digests without sampling time. Old journal operation bytes and saved assessment digests remain unchanged; these transitions use operation v18."}"#;

fn declarations(row: &Row<'_>, key: &str) -> Result<Vec<String>, Refusal> {
    let values: Vec<_> = row.string(key)?.split('|').map(str::trim).collect();
    if values.iter().any(|value| value.is_empty()) {
        return Err(invalid(format!(
            "{key} must not contain an empty declaration"
        )));
    }
    Ok(values.into_iter().map(str::to_string).collect())
}

pub(super) fn apply(
    state: &mut Replay,
    row: Row<'_>,
    lower_ns: i64,
    upper_ns: i64,
) -> Result<(), Refusal> {
    match row.0.table.as_str() {
        "investigation" => {
            row.allowed(
                &[
                    "id",
                    "assessment",
                    "monitor",
                    "owner_role",
                    "deadline",
                    "hypotheses",
                    "probes",
                ],
                &[],
            )?;
            let digest = hex(row.string("assessment")?)?;
            if !live::digest_is_bound(state, digest) {
                return Err(invalid(
                    "an investigation requires a persisted, result-bound assessment",
                ));
            }
            let report = live::persisted_report(state, digest)?;
            if report.as_of_ns > lower_ns {
                return Err(invalid(
                    "investigation evidence cut exceeds the writer's earliest time",
                ));
            }
            let monitor = MonitorKind::parse(row.string("monitor")?)?;
            state.investigations.open(
                &report,
                monitor,
                OpenCase {
                    case_id: row.string("id")?.into(),
                    owner_role: row.string("owner_role")?.into(),
                    deadline_ns: row.time("deadline")?,
                    opened_ns: upper_ns,
                    hypotheses: declarations(&row, "hypotheses")?,
                    probes: declarations(&row, "probes")?,
                },
            )?;
        }
        "investigation_resolution" => {
            row.allowed(
                &[
                    "id",
                    "kind",
                    "human_review",
                    "human_role",
                    "rationale",
                    "evidence",
                ],
                &[],
            )?;
            if row.string("human_review")? != "human" {
                return Err(invalid(
                    "case resolution requires an explicit human-review declaration",
                ));
            }
            state.investigations.resolve(
                row.string("id")?,
                ResolveCase {
                    kind: ResolutionKind::parse(row.string("kind")?)?,
                    human_role: row.string("human_role")?.into(),
                    rationale: row.string("rationale")?.into(),
                    evidence: row.string("evidence")?.into(),
                    recorded_ns: upper_ns,
                },
            )?;
        }
        "investigation_rearm" => {
            row.allowed(&["id", "cohort"], &[])?;
            let name = row.string("cohort")?;
            let cohort = state.cohorts.get(name).copied().ok_or_else(|| {
                invalid("investigation successor cohort is not frozen in this journal")
            })?;
            // Physical-interval forecasts must remain prospective at rearm as well as at freeze.
            // Their ordinary outcome deadline is later than the beginning of the measured interval.
            super::assumption::validate_cohort(state, name, upper_ns)?;
            let members = &state
                .ledger
                .cohort(cohort)
                .ok_or_else(|| invalid("investigation successor cohort is missing"))?
                .spec
                .members;
            if members.iter().any(|id| {
                state
                    .plant_registration_starts
                    .get(id)
                    .is_some_and(|start| upper_ns >= *start)
            }) {
                return Err(invalid(
                    "investigation rearm must precede every plant physical interval's start",
                ));
            }
            state
                .investigations
                .rearm(row.string("id")?, &state.ledger, cohort, upper_ns)?;
        }
        _ => return Err(invalid("unknown investigation operation")),
    }
    Ok(())
}

/// Optional projection preserves the output of journals that contain no investigation operations.
pub(super) fn json_field(state: &Replay, recorded_ns: i64) -> Result<String, Refusal> {
    let mut cases = Vec::new();
    for case in state.investigations.iter() {
        let resolution = case.resolution.as_ref().map_or_else(|| "null".into(), |resolution| {
            format!(
                "{{\"kind\":{},\"human_role\":{},\"human_identity_verified\":false,\"rationale\":{},\"evidence\":{},\"recorded_ns\":{},\"digest\":{}}}",
                js(resolution.kind.as_str()), js(&resolution.human_role), js(&resolution.rationale),
                js(&resolution.evidence), resolution.recorded_ns, js(&resolution.digest.to_hex()),
            )
        });
        let rearm = case.rearm.as_ref().map_or_else(|| "null".into(), |rearm| {
            format!("{{\"cohort\":{},\"recorded_ns\":{},\"digest\":{}}}",
                js(&rearm.cohort.to_hex()), rearm.recorded_ns, js(&rearm.digest.to_hex()))
        });
        let strings = |values: &[String]| values.iter().map(|v| js(v)).collect::<Vec<_>>().join(",");
        cases.push(format!(
            "{{\"id\":{},\"state\":{},\"owner_role\":{},\"opened_ns\":{},\"deadline_ns\":{},\"overdue_at_recorded_time\":{},\"assessment_digest\":{},\"opening_digest\":{},\"digest\":{},\"trigger\":{{\"cohort\":{},\"monitor\":{},\"first_crossing\":{},\"member\":{}}},\"hypotheses\":[{}],\"probes_in_declared_order\":[{}],\"probe_ranking_verified\":false,\"resolution\":{resolution},\"rearm\":{rearm}}}",
            js(&case.case_id), js(case.state()), js(&case.owner_role), case.opened_ns, case.deadline_ns,
            case.is_overdue(recorded_ns), js(&case.assessment_digest.to_hex()), js(&case.opening_digest.to_hex()),
            js(&case.digest.to_hex()), js(&case.trigger.cohort.to_hex()), js(case.trigger.monitor.as_str()),
            case.trigger.first_crossing, state.member_json(&case.trigger.member)?,
            strings(&case.hypotheses), strings(&case.probes),
        ));
    }
    Ok(if cases.is_empty() {
        String::new()
    } else {
        format!(",\"investigations\":[{}]", cases.join(","))
    })
}

pub(super) fn markdown(state: &Replay, recorded_ns: i64) -> String {
    use crate::guidance::text;
    let mut out = String::new();
    for case in state.investigations.iter() {
        if out.is_empty() {
            out.push_str("\n## Monitor investigations\n\nOriginal alarm evidence is retained. Hypotheses, probe order and human-review roles are supplied declarations; neither diagnosis nor reviewer identity is verified. A resolution does not clear an alarm or recalibrate a model.\n");
        }
        let event = case
            .trigger
            .member
            .selected_event
            .map_or_else(|| "none".into(), |(_, digest)| digest.to_hex());
        let _ = writeln!(
            out,
            "\n### {} — {}\n\nOwner: {}. Deadline: {}. Overdue at the journal's last recorded time: {}.\n\nSaved assessment: {}. {} alarm at original observation {}; forecast {}; event {event}.\n\nOpening digest: {}. Current case digest: {}.\n\nCompeting explanations:",
            text(&case.case_id), case.state(), text(&case.owner_role), utc(case.deadline_ns),
            case.is_overdue(recorded_ns), case.assessment_digest, case.trigger.monitor.as_str(),
            case.trigger.first_crossing, text(state.alias(case.trigger.member.forecast_id)),
            case.opening_digest, case.digest
        );
        out.push('\n');
        for hypothesis in &case.hypotheses {
            let _ = writeln!(out, "- {}", text(hypothesis));
        }
        out.push_str("\nProbes in declared priority order (VOI not verified):\n\n");
        for (i, probe) in case.probes.iter().enumerate() {
            let _ = writeln!(out, "{}. {}", i + 1, text(probe));
        }
        if let Some(resolution) = &case.resolution {
            let _ = writeln!(
                out,
                "\nResolution: {}. Declared human role: {}. Rationale: {}. Evidence: {}. Recorded at {}.\n",
                resolution.kind.as_str(), text(&resolution.human_role), text(&resolution.rationale),
                text(&resolution.evidence), utc(resolution.recorded_ns)
            );
        }
        if let Some(rearm) = &case.rearm {
            let _ = writeln!(
                out,
                "\nRearmed through new prospective cohort {} at {}; the old monitor is unchanged.\n",
                rearm.cohort, utc(rearm.recorded_ns)
            );
        }
    }
    out
}
