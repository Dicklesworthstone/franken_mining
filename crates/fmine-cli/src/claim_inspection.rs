//! Reads of verified source projections, including the typed recovery model's retained Claim DAG.

use fmine_claim::{Claim, ClaimId};
use fmine_evidence::{Refusal, RefusalCode};
use fmine_metrics::recovery::render::{fit, projection, quantity, rational};
use fmine_public::{PublicAssuranceReport, recovery::RecoveryReport, sources::IssuerProfile};
use std::collections::{BTreeMap, BTreeSet};

use crate::{ClaimView, Envelope, NextAction, claim_actions, escape_json, json_string as js};
fn ids(values: impl IntoIterator<Item = ClaimId>) -> String {
    values
        .into_iter()
        .map(|id| js(&id.to_string()))
        .collect::<Vec<_>>()
        .join(",")
}
struct Node {
    parents: Vec<ClaimId>,
    anchored: bool,
    json: String,
    why: String,
}

/// Export the complete checked canonical Claim beside its caller-supplied typed value.
pub(crate) use fmine_claim::inspection::public_json as canonical_claim_json;

/// Keep the typed value beside the exact canonical body; reading never upgrades its evidence.
fn node<T>(claim: &Claim<T>, category: &str, value: String) -> Result<Node, Refusal> {
    let view = fmine_claim::inspection::public_inspection(
        claim,
        category,
        &value,
        fmine_metrics::recovery::MODEL_CAVEAT,
    )?;
    Ok(Node {
        parents: claim.parents.clone(),
        anchored: !claim.anchors.is_empty(),
        json: view.json,
        why: view.why,
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
