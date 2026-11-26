//! Read-only approval and outcome worksheets for BD-230.
//! Determinism: Exact

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use fmine_calib::record::{format_utc, parse_hex32, parse_utc};
use fmine_capital::hindcast::{
    ActionApprovalMetrics, ApprovalEvidence, ApprovalSnapshot, FrozenApproval, HINDCAST_NO_CLAIM,
    HindcastCaseResult, HindcastScenario, RealizedApprovalOutcome, freeze_approval,
};
use fmine_claim::{AdditivityClass, Claim, Confidentiality, Representation, SpatialSupport};
use fmine_core::{Digest32, anchor::SourceAnchor, hash::sha256};
use fmine_evidence::{
    AssertionSource, EpistemicStatus, EvidenceClass, KnowledgeState, KuKind, Refusal, RefusalCode,
};
use fmine_io::omf::json::{Json, parse as parse_json};
use fmine_registry::toml_subset::{TomlDoc, parse_subset};
use fmine_time::{
    Anchor, CivilDate, NS_PER_DAY, NS_PER_SEC, QuadTemporalStamp, RecordTime, TimeEvidence,
    ValidTime,
};
use fmine_units::ExactQty;

use super::investment::{Fields, fields, get, invalid, number};
use crate::{Envelope, decision_journal, json_string as js, worksheet_input};

const VERB: &str = "decide hindcast";
const FREEZE: &str = "decide hindcast freeze <approval>";
const SCORE: &str =
    "decide hindcast score <approval> --outcome <outcome> --approval-digest <digest>";
const SCORE_RECORDED: &str =
    "decide hindcast score-recorded <journal> --head <digest> --decision <id> --outcome <outcome>";
pub(super) const APPROVAL_SCHEMA: &str = "fmine.capital-approval.v1";
const OUTCOME_SCHEMA: &str = "fmine.capital-outcome.v1";
const RECEIPT_PREFIX: &str = r#"{"schema":"fmine.capital-hindcast.v1","mode":"freeze","#;
const SOURCE_LOCATOR: &str = "declared capital approval source";
const RECORDED_NO_CLAIM: &str = "Conditional comparison using the retained worksheet and the pinned local decision journal. Pre-resolution timing uses the declared host-clock bound and the supplied outcome's physical validity. Payoffs for unchosen actions are counterfactual. Historical authentication, external time proof and authorization require separate evidence.";
const STAMP_FIELDS: &[&str] = &[
    "valid_start",
    "valid_end",
    "collected_at",
    "available_at",
    "recorded_at",
    "record_clock",
    "cell",
    "sequence",
];
const MAX_WORKSHEET_BYTES: u64 = 4 * 1024 * 1024;
type Result<T> = std::result::Result<T, Refusal>;

pub(crate) const ROBOT_DOCS_JSON: &str = r#"{"freeze":"decide hindcast freeze <approval>","score":"decide hindcast score <approval> --outcome <outcome> --approval-digest <digest>","score_recorded":"decide hindcast score-recorded <journal> --head <digest> --decision <id> --outcome <outcome>","approval_schema":"fmine.capital-approval.v1","outcome_schema":"fmine.capital-outcome.v1","examples":["tests/capital/hindcast_approval.toml","tests/capital/hindcast_outcome.toml"],"time":"Whole-second UTC timestamps, declared record-clock class and approval anchor","pin":"Both score modes verify the approval pin before opening the outcome. The outcome names the same case.","journal":"Record an explicit action from freeze journal_actions through decide journal record. Score from a pinned journal and decision ID. Physical outcome resolution must follow the latest possible journal write.","authority":"All hindcast modes are read-only. Source timestamps, evidence and clock bounds are declarations. Scoring can include supplied counterfactuals."}"#;

/// The score mode carries both a separate outcome and a required approval pin.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Freeze(PathBuf),
    Score {
        approval: PathBuf,
        outcome: PathBuf,
        approval_digest: String,
    },
    ScoreRecorded {
        journal: PathBuf,
        head: Digest32,
        decision: String,
        outcome: PathBuf,
    },
}

pub(crate) fn parse(words: &[String]) -> std::result::Result<Command, String> {
    let usage =
        || format!("expected 'fmine {FREEZE}', 'fmine {SCORE}' or 'fmine {SCORE_RECORDED}'");
    let path = |word: &str| !word.trim().is_empty() && !word.starts_with("--");
    let [_, _, mode, input, rest @ ..] = words else {
        return Err(usage());
    };
    if !path(input) {
        return Err(usage());
    }
    match (mode.as_str(), rest) {
        ("freeze", []) => Ok(Command::Freeze(input.into())),
        ("score", [a, x, b, y]) => {
            let (outcome, digest) = match (a.as_str(), b.as_str()) {
                ("--outcome", "--approval-digest") => (x, y),
                ("--approval-digest", "--outcome") => (y, x),
                _ => return Err(usage()),
            };
            if !path(outcome) || parse_hex32(digest).is_none() {
                return Err(usage());
            }
            Ok(Command::Score {
                approval: input.into(),
                outcome: outcome.into(),
                approval_digest: digest.clone(),
            })
        }
        ("score-recorded", [_, _, _, _, _, _]) => {
            let fields: std::collections::BTreeMap<_, _> = rest
                .as_chunks::<2>()
                .0
                .iter()
                .map(|[flag, value]| (flag.as_str(), value.as_str()))
                .collect();
            let [Some(head), Some(decision), Some(outcome)] = ["--head", "--decision", "--outcome"]
                .map(|key| fields.get(key).copied().filter(|value| path(value)))
            else {
                return Err(usage());
            };
            Ok(Command::ScoreRecorded {
                journal: input.into(),
                head: parse_hex32(head).map(Digest32).ok_or_else(usage)?,
                decision: decision.into(),
                outcome: outcome.into(),
            })
        }
        _ => Err(usage()),
    }
}

fn parse_document(text: &str, schema: &str, tables: &[&str]) -> Result<TomlDoc> {
    let doc = parse_subset(text).map_err(|e| invalid(e.to_string()))?;
    if !matches!(doc.top_get("schema").map(|f| &f.value),
        Some(fmine_registry::toml_subset::TomlValue::Str(actual)) if actual == schema)
    {
        return Err(invalid(format!("requires schema {schema}")));
    }
    for row in &doc.rows {
        if !tables.contains(&row.table.as_str()) {
            return Err(invalid(format!(
                "line {}: unexpected table {}",
                row.line, row.table
            )));
        }
    }
    Ok(doc)
}

fn utc(f: &Fields<'_>, key: &str) -> Result<i64> {
    parse_utc(get(f, key)?)
        .and_then(|s| s.checked_mul(NS_PER_SEC))
        .ok_or_else(|| {
            invalid(format!(
                "{key} requires a representable YYYY-MM-DDTHH:MM:SSZ"
            ))
        })
}

fn date(ns: i64) -> CivilDate {
    CivilDate::from_epoch_days(ns.div_euclid(NS_PER_DAY))
}

fn integer<T: std::str::FromStr>(f: &Fields<'_>, key: &str) -> Result<T> {
    get(f, key)?
        .parse()
        .map_err(|_| invalid(format!("{key} requires an integer within range")))
}

fn exact(f: &Fields<'_>, key: &str) -> Result<ExactQty> {
    number(get(f, key)?).map_err(|e| invalid(format!("{key}: {}", e.what)))
}

fn interval(f: &Fields<'_>, start: &str, end: &str) -> Result<ValidTime> {
    ValidTime::new(utc(f, start)?, utc(f, end)?).map_err(|e| invalid(e.to_string()))
}

fn stamp(f: &Fields<'_>) -> Result<QuadTemporalStamp> {
    let clock = match (get(f, "record_clock")?, f.contains_key("ntp_max_skew_ms")) {
        ("host_synced", false) => TimeEvidence::HostSynced,
        ("offline_uncertain", false) => TimeEvidence::OfflineUncertain,
        ("ntp_bounded", true) => TimeEvidence::NtpBounded {
            max_skew_ms: integer(f, "ntp_max_skew_ms")?,
        },
        _ => {
            return Err(invalid(
                "record_clock requires host_synced, offline_uncertain, or ntp_bounded with ntp_max_skew_ms",
            ));
        }
    };
    let record = RecordTime::new(
        get(f, "cell")?,
        integer(f, "sequence")?,
        utc(f, "recorded_at")?,
        clock,
    );
    QuadTemporalStamp::new(
        interval(f, "valid_start", "valid_end")?,
        utc(f, "collected_at")?,
        utc(f, "available_at")?,
        record,
    )
    .map_err(|e| invalid(e.to_string()))
}

fn approval(text: &str) -> Result<ApprovalSnapshot> {
    let doc = parse_document(text, APPROVAL_SCHEMA, &["anchor", "evidence", "scenario"])?;
    let f = fields(
        &doc.top,
        &[
            "schema",
            "case_id",
            "project_name",
            "approval_cut",
            "physical_start",
            "physical_end",
            "scenario_basis",
            "baseline_capex_usd_m",
            "shares_millions",
            "static_npv_usd_m",
            "static_irr",
            "hurdle_irr",
            "cvar_tail_mass",
            "max_cvar_loss_usd_per_share",
        ],
        &[],
    )?;
    let cut = utc(&f, "approval_cut")?;
    let marks = doc
        .rows_of("anchor")
        .map(|row| {
            let f = fields(&row.fields, &["cell", "sequence"], &[])?;
            Ok((get(&f, "cell")?, integer(&f, "sequence")?))
        })
        .collect::<Result<Vec<_>>>()?;
    let approval_anchor = Anchor::new(marks).map_err(|e| invalid(e.to_string()))?;
    let evidence_keys = [&["id", "label", "class"][..], STAMP_FIELDS].concat();
    let approval_evidence = doc
        .rows_of("evidence")
        .map(|row| {
            let f = fields(&row.fields, &evidence_keys, &["ntp_max_skew_ms"])?;
            let evidence_class = match get(&f, "class")? {
                "assumed" => EvidenceClass::Assumed,
                "modelled_uncalibrated" => EvidenceClass::ModelledUncalibrated,
                "issuer_assertion" => EvidenceClass::ISSUER,
                "secondary_assertion" => EvidenceClass::Assertion {
                    source: AssertionSource::Secondary,
                },
                _ => return Err(invalid("unknown approval evidence class")),
            };
            Ok(ApprovalEvidence {
                evidence_id: get(&f, "id")?.to_owned(),
                label: get(&f, "label")?.to_owned(),
                evidence_class,
                stamp: stamp(&f)?,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    let scenarios = doc
        .rows_of("scenario")
        .map(|row| {
            let f = fields(
                &row.fields,
                &[
                    "id",
                    "weight",
                    "full_nav_usd_m",
                    "staged_nav_usd_m",
                    "defer_nav_usd_m",
                    "full_liquidity_slack_usd_m",
                    "staged_liquidity_slack_usd_m",
                    "defer_liquidity_slack_usd_m",
                ],
                &[],
            )?;
            Ok(HindcastScenario {
                scenario_id: get(&f, "id")?.to_owned(),
                weight: exact(&f, "weight")?,
                full_commit_delta_nav_usd_m: exact(&f, "full_nav_usd_m")?,
                staged_recourse_delta_nav_usd_m: exact(&f, "staged_nav_usd_m")?,
                defer_delta_nav_usd_m: exact(&f, "defer_nav_usd_m")?,
                full_commit_liquidity_slack_usd_m: exact(&f, "full_liquidity_slack_usd_m")?,
                staged_recourse_liquidity_slack_usd_m: exact(&f, "staged_liquidity_slack_usd_m")?,
                defer_liquidity_slack_usd_m: exact(&f, "defer_liquidity_slack_usd_m")?,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    let is_synthetic_scenario_grid = match get(&f, "scenario_basis")? {
        "synthetic" => true,
        "declared" => false,
        _ => return Err(invalid("scenario_basis requires synthetic or declared")),
    };
    Ok(ApprovalSnapshot {
        case_id: get(&f, "case_id")?.to_owned(),
        project_name: get(&f, "project_name")?.to_owned(),
        approval_date: date(cut),
        approval_as_of_ns: cut,
        approval_anchor,
        physical_horizon: interval(&f, "physical_start", "physical_end")?,
        is_synthetic_scenario_grid,
        approval_evidence,
        baseline_capex_usd_m: exact(&f, "baseline_capex_usd_m")?,
        shares_millions: exact(&f, "shares_millions")?,
        h7a_static_npv_usd_m: exact(&f, "static_npv_usd_m")?,
        h7a_static_irr: exact(&f, "static_irr")?,
        h7a_hurdle_irr: exact(&f, "hurdle_irr")?,
        scenarios,
        cvar_tail_mass: exact(&f, "cvar_tail_mass")?,
        max_cvar_loss_per_share_usd: exact(&f, "max_cvar_loss_usd_per_share")?,
        realized_outcome: (),
    })
}

fn outcome(text: &str, case_id: &str) -> Result<RealizedApprovalOutcome> {
    let doc = parse_document(text, OUTCOME_SCHEMA, &["evidence"])?;
    let keys = [
        &[
            "schema",
            "case_id",
            "capex_usd_m",
            "schedule_slip_months",
            "full_nav_usd_m",
            "staged_nav_usd_m",
            "defer_nav_usd_m",
        ][..],
        STAMP_FIELDS,
    ]
    .concat();
    let f = fields(&doc.top, &keys, &["ntp_max_skew_ms"])?;
    if get(&f, "case_id")? != case_id {
        return Err(invalid("outcome case_id differs from the frozen approval"));
    }
    let outcome_evidence_ids = doc
        .rows_of("evidence")
        .map(|row| {
            let f = fields(&row.fields, &["id"], &[])?;
            Ok(get(&f, "id")?.to_owned())
        })
        .collect::<Result<Vec<_>>>()?;
    let realized_stamp = stamp(&f)?;
    Ok(RealizedApprovalOutcome {
        realized_date: date(realized_stamp.physical_valid_time().end_ns),
        realized_stamp,
        realized_capex_usd_m: exact(&f, "capex_usd_m")?,
        schedule_slip_months: integer(&f, "schedule_slip_months")?,
        realized_full_commit_delta_nav_usd_m: exact(&f, "full_nav_usd_m")?,
        realized_staged_recourse_delta_nav_usd_m: exact(&f, "staged_nav_usd_m")?,
        realized_defer_delta_nav_usd_m: exact(&f, "defer_nav_usd_m")?,
        outcome_evidence_ids,
    })
}

fn qty(value: ExactQty) -> String {
    if value.den() == 1 {
        value.num().to_string()
    } else {
        format!("{}/{}", value.num(), value.den())
    }
}

fn ids(values: &[String]) -> String {
    values
        .iter()
        .map(|value| js(value))
        .collect::<Vec<_>>()
        .join(",")
}

fn action_json(m: &ActionApprovalMetrics) -> String {
    format!(
        concat!(
            "{{\"action\":\"{}\",\"expected_delta_nav_usd_per_share\":\"{}\",",
            "\"cvar_loss_usd_per_share\":\"{}\",\"minimum_liquidity_slack_usd_m\":\"{}\",",
            "\"liquidity_feasible\":{},\"cvar_feasible\":{},\"feasible\":{}}}"
        ),
        m.action.as_str(),
        qty(m.expected_delta_nav_per_share_usd),
        qty(m.cvar_loss_per_share_usd),
        qty(m.min_liquidity_slack_usd_m),
        m.liquidity_feasible,
        m.cvar_feasible,
        m.feasible,
    )
}

fn score_json(score: &HindcastCaseResult, source: &str) -> String {
    format!(
        concat!(
            "{{\"outcome_digest\":\"{}\",\"source\":{source},\"evidence_ids\":[{}],",
            "\"capex_multiple\":\"{}\",\"schedule_slip_months\":{},",
            "\"h7a_outcome_usd_per_share\":\"{}\",\"h7d_outcome_usd_per_share\":\"{}\",",
            "\"h7a_regret_usd_per_share\":\"{}\",\"h7d_regret_usd_per_share\":\"{}\",",
            "\"h7d_advantage_usd_per_share\":\"{}\",\"full_commit_outcome_within_approval_band\":{}}}"
        ),
        score.outcome_digest,
        ids(&score.outcome_evidence_ids),
        qty(score.realized_capex_multiple),
        score.schedule_slip_months,
        qty(score.h7a_realized_delta_nav_per_share_usd),
        qty(score.h7d_realized_delta_nav_per_share_usd),
        qty(score.h7a_regret_per_share_usd),
        qty(score.h7d_regret_per_share_usd),
        qty(score.h7d_advantage_per_share_usd),
        score.realized_within_approval_band,
        source = source,
    )
}

struct Score {
    result: HindcastCaseResult,
    source: SourceAnchor,
    bytes: usize,
}

fn markdown(
    snapshot: &ApprovalSnapshot,
    frozen: &FrozenApproval,
    source: &SourceAnchor,
    approval_cut: &str,
    score: Option<&Score>,
    registered: Option<&decision_journal::RecordedHindcast>,
) -> String {
    let (h7a, h7d) = frozen.decisions();
    let mut md = format!(
        "# Capital hindcast for {}\n\nApproval cut {approval_cut}\n\nApproval digest `{}`\n\nApproval worksheet SHA-256 `{}`\n\n\
         - H7a chose `{}` at an IRR hurdle of {}.\n- H7d chose `{}` with a worst-tail mass of {} and a loss limit of {} USD/share.\n\n\
         | Action | Expected ΔNAV in USD/share | Tail loss USD/share | Minimum liquidity slack USD millions | Feasible |\n\
         |---|---:|---:|---:|---|\n",
        worksheet_input::markdown_cell(&snapshot.project_name),
        frozen.digest(),
        Digest32(source.artifact_sha256),
        h7a.as_str(),
        qty(snapshot.h7a_hurdle_irr),
        h7d.as_str(),
        qty(snapshot.cvar_tail_mass),
        qty(snapshot.max_cvar_loss_per_share_usd),
    );
    for m in frozen.action_metrics() {
        let _ = writeln!(
            md,
            "| `{}` | {} | {} | {} | {} |",
            m.action.as_str(),
            qty(m.expected_delta_nav_per_share_usd),
            qty(m.cvar_loss_per_share_usd),
            qty(m.min_liquidity_slack_usd_m),
            m.feasible
        );
    }
    if let Some(registered) = registered {
        let _ = writeln!(
            md,
            "\nDecision `{}` recorded `{}` at journal record `{}`.\n\nVerified journal head `{}`. The latest possible record time is {} ns since the Unix epoch.",
            worksheet_input::markdown_cell(&registered.decision),
            worksheet_input::markdown_cell(&registered.action),
            registered.record_head,
            registered.journal_head,
            registered.latest_ns,
        );
    }
    if let Some(Score {
        result: score,
        source,
        ..
    }) = score
    {
        let _ = writeln!(
            md,
            "\nOutcome digest `{}`\n\nOutcome worksheet SHA-256 `{}`\n\nSupplied H7a payoff {} USD/share. Supplied H7d payoff {} USD/share.\n\nCapex multiple {}. Schedule slip {} months.",
            score.outcome_digest,
            Digest32(source.artifact_sha256),
            qty(score.h7a_realized_delta_nav_per_share_usd),
            qty(score.h7d_realized_delta_nav_per_share_usd),
            qty(score.realized_capex_multiple),
            score.schedule_slip_months
        );
    }
    let _ = write!(md, "\n{}", no_claim(registered));
    md
}

fn no_claim(registered: Option<&decision_journal::RecordedHindcast>) -> &'static str {
    if registered.is_some() {
        RECORDED_NO_CLAIM
    } else {
        HINDCAST_NO_CLAIM
    }
}

fn report(
    snapshot: &ApprovalSnapshot,
    frozen: &FrozenApproval,
    source: &SourceAnchor,
    approval_bytes: usize,
    score: Option<&Score>,
    registered: Option<&decision_journal::RecordedHindcast>,
) -> Envelope {
    let digest = frozen.digest().to_hex();
    let score_payload = score.map_or_else(
        || "null".to_owned(),
        |score| {
            score_json(
                &score.result,
                &worksheet_input::source_json(&score.source, score.bytes),
            )
        },
    );
    let (h7a, h7d) = frozen.decisions();
    let approval_cut = format_utc(frozen.approval_as_of_ns().div_euclid(NS_PER_SEC));
    let actions = frozen
        .action_metrics()
        .iter()
        .map(action_json)
        .collect::<Vec<_>>()
        .join(",");
    let (mode, extra) = match (registered, score) {
        (Some(recorded), _) => (
            "score-recorded",
            format!(
                ",\"registration\":{{\"journal_head\":\"{}\",\"record_head\":\"{}\",\"decision_id\":{},\"recorded_action\":{},\"time_bounds_ns\":[\"{}\",\"{}\"]}}",
                recorded.journal_head,
                recorded.record_head,
                js(&recorded.decision),
                js(&recorded.action),
                recorded.earliest_ns,
                recorded.latest_ns,
            ),
        ),
        (None, Some(_)) => ("score", String::new()),
        (None, None) => {
            let choices = frozen
                .action_metrics()
                .iter()
                .map(|m| format!("\"{}@{digest}\"", m.action.as_str()))
                .collect::<Vec<_>>()
                .join(",");
            ("freeze", format!(",\"journal_actions\":[{choices}]"))
        }
    };
    let payload = format!(
        concat!(
            "{{\"schema\":\"fmine.capital-hindcast.v1\",\"mode\":\"{mode}\",\"case_id\":{},",
            "\"project_name\":{},\"approval_cut\":\"{approval_cut}\",\"approval_digest\":\"{digest}\",",
            "\"approval_source\":{},\"approval_evidence_ids\":[{}],\"synthetic_scenarios\":{},",
            "\"policy\":{{\"static_npv_usd_m\":\"{}\",\"static_irr\":\"{}\",\"hurdle_irr\":\"{}\",",
            "\"cvar_tail_mass\":\"{}\",\"max_cvar_loss_usd_per_share\":\"{}\"}},",
            "\"h7a_choice\":\"{}\",\"h7d_choice\":\"{}\",\"actions\":[{actions}],\"score\":{score_payload}{extra}}}"
        ),
        js(&snapshot.case_id),
        js(&snapshot.project_name),
        worksheet_input::source_json(source, approval_bytes),
        ids(frozen.admitted_evidence_ids()),
        frozen.is_synthetic_scenario_grid(),
        qty(snapshot.h7a_static_npv_usd_m),
        qty(snapshot.h7a_static_irr),
        qty(snapshot.h7a_hurdle_irr),
        qty(snapshot.cvar_tail_mass),
        qty(snapshot.max_cvar_loss_per_share_usd),
        h7a.as_str(),
        h7d.as_str(),
        mode = mode,
        approval_cut = approval_cut,
        digest = digest,
        actions = actions,
        score_payload = score_payload,
        extra = extra,
    );
    let mut envelope = Envelope::ok(VERB, payload, Vec::new());
    envelope.no_claim = Some(no_claim(registered));
    envelope.rendered_markdown = Some(markdown(
        snapshot,
        frozen,
        source,
        &approval_cut,
        score,
        registered,
    ));
    envelope
}

fn approval_source(path: &str, model: &str) -> SourceAnchor {
    SourceAnchor::new(path, sha256(model.as_bytes()).0, SOURCE_LOCATOR)
}

fn receipt_claim(card: &str, source: SourceAnchor) -> String {
    let tag = format!("capital-hindcast-receipt/v1:{}", sha256(card.as_bytes()));
    Claim::new(
        "decision.capital-hindcast-receipt/1",
        (),
        &tag,
        Representation::Deterministic,
        SpatialSupport::PeriodAggregate,
        AdditivityClass::NonAdditive,
        EvidenceClass::Assumed,
        EpistemicStatus::KnownUnknown(KuKind::Parametric),
        KnowledgeState::Estimated,
        Confidentiality::internal(),
        Vec::new(),
        Vec::new(),
        vec![source],
    )
    .claim_id
    .to_string()
}

fn check_pin(frozen: &FrozenApproval, expected: &str) -> Result<()> {
    let digest = frozen.digest().to_hex();
    if expected != digest {
        return Err(Refusal::new(
            RefusalCode::FrozenRecordMutation507,
            format!("approval digest changed: expected {expected}, found {digest}"),
        ));
    }
    Ok(())
}

fn check_reviewed_action(frozen: &FrozenApproval, action: &str) -> Result<()> {
    let unavailable = || {
        Refusal::new(
            RefusalCode::BasisMismatch105,
            "select a digest-bound action from decide hindcast freeze --json journal_actions",
        )
    };
    let (choice, pin) = action.split_once('@').ok_or_else(unavailable)?;
    if !frozen
        .action_metrics()
        .iter()
        .any(|m| m.action.as_str() == choice)
    {
        return Err(unavailable());
    }
    check_pin(frozen, pin)
}

pub(super) fn journal_snapshot(
    model: &str,
    source: &SourceAnchor,
    action: &str,
) -> Result<(String, String)> {
    let snapshot = approval(model)?;
    let frozen = freeze_approval(&snapshot)?;
    check_reviewed_action(&frozen, action)?;
    let source = approval_source(&source.artifact_path, model);
    let card = report(&snapshot, &frozen, &source, model.len(), None, None).payload_json;
    let claim_id = receipt_claim(&card, source);
    Ok((card, claim_id))
}

pub(crate) fn is_receipt(card: &str) -> bool {
    card.starts_with(RECEIPT_PREFIX)
}

fn check_approval_cut(earliest_ns: i64, cut: i64) -> Result<()> {
    if earliest_ns < cut {
        return Err(Refusal::new(
            RefusalCode::AssayLookahead108,
            "the earliest possible journal write precedes the approval cut",
        ));
    }
    Ok(())
}

pub(crate) fn validate_receipt(
    model: &str,
    path: &str,
    card: &str,
    action: &str,
    claim_id: &str,
    earliest_ns: i64,
) -> Result<()> {
    if !is_receipt(card) {
        return Ok(());
    }
    let mismatch = || {
        Refusal::new(
            RefusalCode::DigestMismatch505,
            "retained hindcast receipt disagrees with its action, worksheet or receipt Claim",
        )
    };
    let receipt = parse_json(card.as_bytes()).map_err(|_| mismatch())?;
    let pin = receipt
        .get("approval_digest")
        .and_then(Json::as_str)
        .filter(|pin| parse_hex32(pin).is_some())
        .ok_or_else(mismatch)?;
    let cut = receipt
        .get("approval_cut")
        .and_then(Json::as_str)
        .and_then(parse_utc)
        .and_then(|second| second.checked_mul(NS_PER_SEC))
        .ok_or_else(mismatch)?;
    let stored_source = receipt.get("approval_source").ok_or_else(mismatch)?;
    let source = approval_source(path, model);
    if stored_source.get("path").and_then(Json::as_str) != Some(path)
        || stored_source.get("sha256").and_then(Json::as_str)
            != Some(Digest32(source.artifact_sha256).to_hex().as_str())
        || !action
            .split_once('@')
            .is_some_and(|(_, digest)| digest == pin)
        || !receipt
            .get("journal_actions")
            .and_then(Json::as_array)
            .is_some_and(|actions| actions.iter().any(|choice| choice.as_str() == Some(action)))
        || claim_id != receipt_claim(card, source)
    {
        return Err(mismatch());
    }
    check_approval_cut(earliest_ns, cut)
}

fn score(
    path: &Path,
    snapshot: &ApprovalSnapshot,
    frozen: &FrozenApproval,
    latest_record_ns: Option<i64>,
) -> Result<Score> {
    let (text, source) = worksheet_input::read(path, "capital outcome", MAX_WORKSHEET_BYTES)?;
    let realized = outcome(&text, &snapshot.case_id)?;
    let resolution = realized.realized_stamp.physical_valid_time().end_ns;
    if latest_record_ns.is_some_and(|latest| resolution <= latest) {
        return Err(Refusal::new(
            RefusalCode::AssayLookahead108,
            "physical outcome resolution must follow the latest possible journal write",
        ));
    }
    Ok(Score {
        result: frozen.score(&realized)?,
        source,
        bytes: text.len(),
    })
}

fn evaluate_worksheet(path: &Path, scoring: Option<(&Path, &str)>) -> Result<Envelope> {
    let (text, source) = worksheet_input::read(path, "capital approval", MAX_WORKSHEET_BYTES)?;
    let snapshot = approval(&text)?;
    let frozen = freeze_approval(&snapshot)?;
    let score = if let Some((outcome, pin)) = scoring {
        check_pin(&frozen, pin)?;
        Some(score(outcome, &snapshot, &frozen, None)?)
    } else {
        None
    };
    Ok(report(
        &snapshot,
        &frozen,
        &source,
        text.len(),
        score.as_ref(),
        None,
    ))
}

fn evaluate(command: &Command, root: &Path) -> Result<Envelope> {
    match command {
        Command::Freeze(path) => evaluate_worksheet(&root.join(path), None),
        Command::Score {
            approval,
            outcome,
            approval_digest,
        } => evaluate_worksheet(
            &root.join(approval),
            Some((&root.join(outcome), approval_digest)),
        ),
        Command::ScoreRecorded {
            journal,
            head,
            decision,
            outcome,
        } => {
            let recorded =
                decision_journal::recorded_hindcast(&root.join(journal), *head, decision)?;
            let snapshot = approval(&recorded.model)?;
            let frozen = freeze_approval(&snapshot)?;
            check_reviewed_action(&frozen, &recorded.action)?;
            check_approval_cut(recorded.earliest_ns, frozen.approval_as_of_ns())?;
            let source = approval_source(&recorded.model_path, &recorded.model);
            let score = score(
                &root.join(outcome),
                &snapshot,
                &frozen,
                Some(recorded.latest_ns),
            )?;
            Ok(report(
                &snapshot,
                &frozen,
                &source,
                recorded.model.len(),
                Some(&score),
                Some(&recorded),
            ))
        }
    }
}

pub(crate) fn execute(command: &Command, root: &Path) -> Envelope {
    evaluate(command, root).unwrap_or_else(|e| Envelope::refused(VERB, e, Vec::new()))
}
