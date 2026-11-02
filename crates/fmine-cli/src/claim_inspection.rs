//! Reads of verified source projections, including the typed recovery model's retained Claim DAG.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use fmine_claim::{Claim, ClaimId, Confidentiality};
use fmine_core::Digest32;
use fmine_evidence::{Color, Refusal, RefusalCode};
use fmine_metrics::recovery::{
    Estimate, Fit, ModelProjection, Ownership, Period, Quantity, SquareRoot,
};
use fmine_numerics::exact_feas::Rat;
use fmine_public::{PublicAssuranceReport, recovery::RecoveryReport, sources::IssuerProfile};

use crate::{ClaimView, Envelope, NextAction, claim_actions, escape_json};

fn js(s: &str) -> String {
    format!("\"{}\"", escape_json(s))
}
fn ids(values: impl IntoIterator<Item = ClaimId>) -> String {
    values
        .into_iter()
        .map(|id| js(&id.to_string()))
        .collect::<Vec<_>>()
        .join(",")
}
fn refusal(r: &Refusal) -> String {
    format!(
        "{{\"code\":{},\"exit_code\":{},\"what\":{},\"fix\":{}}}",
        js(r.code),
        r.exit_code,
        js(&r.what),
        js(r.fix)
    )
}
fn guard(result: &Result<(), Refusal>) -> String {
    match result {
        Ok(()) => "{\"status\":\"passed\",\"refusal\":null}".into(),
        Err(r) => format!("{{\"status\":\"refused\",\"refusal\":{}}}", refusal(r)),
    }
}
fn finite(value: f64) -> String {
    if value.is_finite() {
        value.to_string()
    } else {
        "null".into()
    }
}
fn color(value: &Result<Color, Refusal>) -> String {
    let body = match value {
        Err(r) => return format!("{{\"status\":\"refused\",\"refusal\":{}}}", refusal(r)),
        Ok(Color::Estimated {
            estimator,
            dispersion,
        }) => format!(
            "\"kind\":\"estimated\",\"estimator\":{},\"dispersion\":{},\"dispersion_is_finite\":{}",
            js(estimator),
            finite(*dispersion),
            dispersion.is_finite()
        ),
        Ok(Color::Verified { lo, hi }) => format!(
            "\"kind\":\"verified\",\"lo\":{},\"hi\":{}",
            finite(*lo),
            finite(*hi)
        ),
        Ok(Color::Validated { regime, dataset }) => format!(
            "\"kind\":\"validated\",\"dataset\":{},\"bounds\":[{}]",
            js(dataset),
            regime
                .bounds()
                .iter()
                .map(|(axis, (lo, hi))| format!(
                    "{{\"axis\":{},\"lo\":{},\"hi\":{}}}",
                    js(axis),
                    finite(*lo),
                    finite(*hi)
                ))
                .collect::<Vec<_>>()
                .join(",")
        ),
    };
    format!("{{\"status\":\"available\",{body},\"refusal\":null}}")
}
fn rational(value: Rat) -> String {
    js(&format!("{}/{}", value.num(), value.den()))
}
fn estimate(value: Estimate) -> String {
    format!(
        "{{\"point\":{},\"lo\":{},\"hi\":{},\"lo_strict\":{},\"hi_strict\":{}}}",
        rational(value.point),
        rational(value.interval.lo),
        rational(value.interval.hi),
        value.interval.lo_strict,
        value.interval.hi_strict
    )
}
fn sqrt(value: SquareRoot) -> String {
    format!(
        "{{\"approx\":{},\"lo\":{},\"hi\":{},\"interpretation\":\"numerical square-root enclosure only\"}}",
        finite(value.approx),
        finite(value.enclosure[0]),
        finite(value.enclosure[1])
    )
}
fn ownership(value: Ownership) -> String {
    match value {
        Ownership::Full => "{\"kind\":\"full\"}".into(),
        Ownership::Attributable { share } => format!(
            "{{\"kind\":\"attributable\",\"share\":{}}}",
            rational(share)
        ),
    }
}
fn period(value: Period) -> String {
    format!(
        "{{\"year\":{},\"first_month\":{},\"last_month\":{}}}",
        value.year, value.first_month, value.last_month
    )
}
fn quantity(value: &Quantity) -> String {
    format!(
        "{{\"subject\":{},\"period\":{},\"ownership\":{},\"kind\":{},\"estimate\":{}}}",
        js(&value.subject),
        period(value.period),
        ownership(value.ownership),
        js(&format!("{:?}", value.kind)),
        estimate(Estimate {
            point: value.point,
            interval: value.interval
        })
    )
}
fn fit(value: &Fit) -> String {
    format!(
        "{{\"subject\":{},\"ownership\":{},\"training_years\":[{}],\"mean_head_percent\":{},\"constant_tails_percent\":{},\"intercept_percent\":{},\"slope\":{},\"residual_sample_variance_at_printed\":{},\"residual_sample_sd_at_printed\":{},\"regression_error_variance_at_printed\":{},\"regression_error_sd_at_printed\":{}}}",
        js(&value.subject),
        ownership(value.ownership),
        value
            .training_years
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(","),
        estimate(value.mean_head),
        estimate(value.constant_tails),
        estimate(value.intercept),
        estimate(value.slope),
        rational(value.residual_sample_variance_at_printed),
        sqrt(value.residual_sample_sd_at_printed),
        rational(value.regression_error_variance_at_printed),
        sqrt(value.regression_error_sd_at_printed)
    )
}
fn projection(value: &ModelProjection) -> String {
    format!(
        "{{\"subject\":{},\"period\":{},\"ownership\":{},\"model\":{},\"expected_recovery_percent\":{},\"gap_pp\":{},\"copper_difference_kt\":{},\"fraction_of_reported_output\":{}}}",
        js(&value.subject),
        period(value.period),
        ownership(value.ownership),
        js(&format!("{:?}", value.model)),
        estimate(value.expected_recovery_percent),
        estimate(value.gap_pp),
        estimate(value.copper_difference_kt),
        estimate(value.fraction_of_reported_output)
    )
}

struct Node {
    parents: Vec<ClaimId>,
    anchored: bool,
    json: String,
    why: String,
}

/// Export the complete checked canonical Claim beside its caller-supplied typed value.
pub(crate) fn canonical_claim_json<T>(
    claim: &Claim<T>,
    category: &str,
    value: String,
) -> Result<String, Refusal> {
    node(claim, category, value).map(|node| node.json)
}

/// Keep the typed value beside the exact canonical body; reading never upgrades its evidence.
fn node<T>(claim: &Claim<T>, category: &str, value: String) -> Result<Node, Refusal> {
    claim.verify_export_clearance(&Confidentiality::public())?;
    claim.check_id()?;
    let mut body = String::new();
    for byte in claim.canonical_body() {
        let _ = write!(body, "{byte:02x}");
    }
    let anchors = claim
        .anchors
        .iter()
        .map(|a| {
            format!(
                "{{\"artifact_path\":{},\"sha256\":{},\"locator\":{}}}",
                js(&a.artifact_path),
                js(&Digest32(a.artifact_sha256).to_hex()),
                js(&a.locator)
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let uu = claim.uu_terms.iter().map(|u| format!("{{\"scenario_id\":{},\"description\":{},\"downside_exposure_usd_m\":{},\"exposure_is_finite\":{},\"trigger_indicator\":{}}}", js(&u.scenario_id), js(&u.description), finite(u.downside_exposure_usd_m), u.downside_exposure_usd_m.is_finite(), js(&u.trigger_indicator))).collect::<Vec<_>>().join(",");
    let json = format!(
        "{{\"claim_id\":{},\"category\":{},\"metric_key\":{},\"value_tag\":{},\"canonical_body_hex\":{},\"value\":{value},\"evidence\":{},\"epistemic\":{},\"knowledge\":{},\"representation\":{},\"support\":{},\"additivity\":{},\"confidentiality\":{},\"parents\":[{}],\"anchors\":[{anchors}],\"uu_terms\":[{uu}],\"supersedes\":{},\"stale\":{},\"calibration_failed\":{}}}",
        js(&claim.claim_id.to_string()),
        js(category),
        js(&claim.metric_key),
        js(claim.value_tag()),
        js(&body),
        js(&format!("{:?}", claim.evidence)),
        js(&format!("{:?}", claim.epistemic)),
        js(&format!("{:?}", claim.state)),
        js(&format!("{:?}", claim.representation)),
        js(&format!("{:?}", claim.support)),
        js(&format!("{:?}", claim.additivity)),
        js(&claim.confidentiality.to_string()),
        ids(claim.parents.iter().copied()),
        claim
            .supersedes
            .map_or_else(|| "null".into(), |id| js(&id.to_string())),
        claim.stale,
        claim.calibration_failed
    );
    let color_result = claim.color();
    let uu_guard = claim.verify_uu_slot_for_decision();
    let first_refusal = uu_guard
        .as_ref()
        .err()
        .or_else(|| color_result.as_ref().err());
    let why = format!(
        "{{\"schema\":\"fmine.claim.why.v1\",\"color\":{},\"uu_guard\":{},\"public_export_guard\":{},\"canonical_id_guard\":{},\"decision_admission\":{{\"scope\":\"Claim color and UU guards only\",\"claim_guards_pass\":{},\"first_refusal\":{},\"decision_context\":null,\"flip_margin\":null}},\"calibration\":{{\"failed\":{},\"assessment\":null}},\"freshness\":{{\"recorded_stale\":{},\"assessment\":\"not_assessed\"}},\"interpretation\":{}}}",
        color(&color_result),
        guard(&uu_guard),
        guard(&Ok(())),
        guard(&Ok(())),
        first_refusal.is_none(),
        first_refusal.map_or_else(|| "null".into(), refusal),
        claim.calibration_failed,
        claim.stale,
        js(fmine_metrics::recovery::MODEL_CAVEAT)
    );
    Ok(Node {
        parents: claim.parents.clone(),
        anchored: !claim.anchors.is_empty(),
        json,
        why,
    })
}

fn inventory(report: &RecoveryReport) -> Result<BTreeMap<ClaimId, Node>, Refusal> {
    let mut nodes = BTreeMap::new();
    nodes.insert(
        report.ownership.claim_id,
        node(
            &report.ownership,
            "ownership",
            format!(
                "{{\"unit\":\"percent\",\"point\":{}}}",
                rational(report.ownership.value)
            ),
        )?,
    );
    nodes.insert(
        report.fit.claim_id,
        node(&report.fit, "fit", fit(&report.fit.value))?,
    );
    for row in &report.training {
        for claim in [&row.head, &row.recovery] {
            nodes.insert(
                claim.claim_id,
                node(claim, "training", quantity(&claim.value))?,
            );
        }
    }
    for row in &report.quarters {
        for cell in &row.inputs {
            nodes.insert(
                cell.claim.claim_id,
                node(&cell.claim, "source", cell.to_json())?,
            );
        }
        if let Some(operating) = &row.operating {
            for claim in [
                &operating.head,
                &operating.recovery,
                &operating.processed,
                &operating.produced,
            ] {
                nodes.insert(
                    claim.claim_id,
                    node(claim, "operating", quantity(&claim.value))?,
                );
            }
        }
        if let Ok(models) = &row.outcome {
            for claim in [&models.constant_tails, &models.grade_dependent_tails] {
                nodes.insert(
                    claim.claim_id,
                    node(claim, "projection", projection(&claim.value))?,
                );
            }
        }
    }
    Ok(nodes)
}

fn trace(id: ClaimId, nodes: &BTreeMap<ClaimId, Node>) -> String {
    let (mut visited, mut unresolved, mut unanchored) =
        (BTreeSet::new(), BTreeSet::new(), BTreeSet::new());
    let mut pending = vec![id];
    while let Some(id) = pending.pop() {
        if !visited.insert(id) {
            continue;
        }
        match nodes.get(&id) {
            Some(node) => {
                pending.extend(node.parents.iter().copied());
                if node.parents.is_empty() && !node.anchored {
                    unanchored.insert(id);
                }
            }
            None => {
                unresolved.insert(id);
            }
        }
    }
    let graph = visited
        .iter()
        .filter_map(|id| nodes.get(id).map(|node| node.json.as_str()))
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"schema\":\"fmine.claim.trace.v1\",\"scope\":\"selected_recovery_claim_dag\",\"root\":{},\"complete_for_claim\":{},\"nodes\":[{graph}],\"unresolved_parents\":[{}],\"unanchored_leaves\":[{}],\"boundary\":\"Verified workbook cells and agreed PDF transcriptions are source assertions; no upstream measurement history or dependency-invalidation replay is asserted.\"}}",
        js(&id.to_string()),
        unresolved.is_empty() && unanchored.is_empty(),
        ids(unresolved),
        ids(unanchored)
    )
}

fn recovery(
    report: &PublicAssuranceReport,
    id: &str,
    view: ClaimView,
    selector: &str,
) -> Result<Option<Envelope>, Refusal> {
    let Some(recovery) = &report.recovery else {
        return Ok(None);
    };
    let Ok(id_value) = id.parse::<ClaimId>() else {
        return Ok(None);
    };
    let nodes = inventory(recovery)?;
    let Some(node) = nodes.get(&id_value) else {
        return Ok(None);
    };
    let inspection = match view {
        ClaimView::Show => String::new(),
        ClaimView::Why => format!(",\"why\":{}", node.why),
        ClaimView::Trace => format!(",\"trace\":{}", trace(id_value, &nodes)),
    };
    let payload = format!(
        "{{\"schema\":\"fmine.claim_read.v1\",\"issuer\":{},\"lane\":{},\"read_mode\":\"verified_recovery_projection\",\"recovery_digest\":{},\"source_identity_anchor_id\":{},\"source_identity_state_root\":{},\"recovery_in_identity_certificate\":false,\"descriptive_only\":true,\"rules_version\":{},\"claim\":{}{inspection}}}",
        js(&report.issuer),
        js(report.lane.as_str()),
        js(&recovery.digest().to_hex()),
        js(&report.ledger_anchor.anchor_id().to_string()),
        js(&report.ledger_anchor.state_root.to_hex()),
        js(fmine_public::recovery::RULES_VERSION),
        node.json
    );
    let mut envelope = Envelope::ok(view.verb(), payload, claim_actions(id, view, selector));
    envelope.no_claim = Some(fmine_metrics::recovery::NO_CLAIM);
    Ok(Some(envelope))
}

/// The workbook branch retains its established payload and certificate; recovery has a separate digest.
pub(super) fn selected(
    report: &PublicAssuranceReport,
    profile: &IssuerProfile,
    id: &str,
    view: ClaimView,
    selector: &str,
) -> Envelope {
    let Some(cell) = report
        .public_claims
        .cells
        .values()
        .find(|cell| cell.claim.claim_id.to_string() == id)
    else {
        return match recovery(report, id, view, selector) {
            Ok(Some(envelope)) => envelope,
            result => Envelope::refused(
                view.verb(),
                result.err().unwrap_or_else(|| {
                    Refusal::new(
                        RefusalCode::NoClaimInDomain201,
                        format!(
                            "claim '{id}' is absent from the selected {} snapshot",
                            report.issuer
                        ),
                    )
                }),
                vec![NextAction::new(
                    format!("fmine assure public {selector}"),
                    "the verified source snapshot containing these public Claims",
                )],
            ),
        };
    };
    let digest = fmine_public::key::claims_digest(report.public_claims.cells.values()).to_hex();
    let inspection = match view {
        ClaimView::Show => String::new(),
        ClaimView::Why => format!(",\"why\":{}", fmine_public::key::inspect::why_json(cell)),
        ClaimView::Trace => format!(
            ",\"trace\":{}",
            fmine_public::key::inspect::trace_json(cell, profile)
        ),
    };
    let payload = format!(
        "{{\"schema\":\"fmine.claim_read.v1\",\"issuer\":\"{}\",\"lane\":\"{}\",\"read_mode\":\"verified_source_projection\",\"claims_digest\":\"{digest}\",\"claim\":{},\"stale\":{},\"calibration_failed\":{},\"decision_uu_present\":{}{inspection}}}",
        escape_json(&report.issuer),
        report.lane.as_str(),
        cell.to_json(),
        cell.claim.stale,
        cell.claim.calibration_failed,
        !cell.claim.uu_terms.is_empty()
    );
    Envelope::report(
        view.verb(),
        report.ledger_anchor.anchor_id().to_string(),
        report.ledger_anchor.state_root.to_hex(),
        payload,
        claim_actions(id, view, selector),
    )
}

include!("../../xtask/testdata/cli_claim_inspection_unit_tests.rs");
