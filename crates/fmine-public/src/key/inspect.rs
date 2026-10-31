//! DESIGN (BD-115): inspect the canonical public Claim without changing its evidence or inventing a
//! decision context. Public projections currently contain source leaves, so trace names that boundary.

use fmine_claim::ConfidentialityTier;
use fmine_core::hash::Digest32;
use fmine_evidence::{Color, Refusal};

use super::{KeyedCell, NilReason, PublicValue};
use crate::js;
use crate::sources::IssuerProfile;

fn refusal_json(refusal: &Refusal) -> String {
    format!(
        "{{\"code\":{},\"exit_code\":{},\"what\":{},\"fix\":{}}}",
        js(refusal.code),
        refusal.exit_code,
        js(&refusal.what),
        js(refusal.fix)
    )
}

fn guard_json(result: &Result<(), Refusal>) -> String {
    match result {
        Ok(()) => "{\"status\":\"passed\",\"refusal\":null}".into(),
        Err(refusal) => format!(
            "{{\"status\":\"refused\",\"refusal\":{}}}",
            refusal_json(refusal)
        ),
    }
}

fn finite_json(value: f64) -> String {
    if value.is_finite() {
        value.to_string()
    } else {
        "null".into()
    }
}

fn color_json(result: &Result<Color, Refusal>) -> String {
    let value = match result {
        Err(refusal) => {
            return format!(
                "{{\"status\":\"refused\",\"value\":null,\"refusal\":{}}}",
                refusal_json(refusal)
            );
        }
        Ok(Color::Estimated {
            estimator,
            dispersion,
        }) => format!(
            "{{\"kind\":\"estimated\",\"estimator\":{},\"dispersion\":{},\"dispersion_status\":{}}}",
            js(estimator),
            finite_json(*dispersion),
            js(if dispersion.is_finite() {
                "finite"
            } else if *dispersion == f64::INFINITY {
                "unbounded"
            } else {
                "invalid"
            })
        ),
        Ok(Color::Verified { lo, hi }) => format!(
            "{{\"kind\":\"verified\",\"lo\":{},\"hi\":{},\"lo_text\":{},\"hi_text\":{}}}",
            finite_json(*lo),
            finite_json(*hi),
            js(&lo.to_string()),
            js(&hi.to_string())
        ),
        Ok(Color::Validated { regime, dataset }) => {
            let bounds = regime
                .bounds()
                .iter()
                .map(|(axis, (lo, hi))| {
                    format!(
                        "{{\"axis\":{},\"lo\":{},\"hi\":{}}}",
                        js(axis),
                        finite_json(*lo),
                        finite_json(*hi)
                    )
                })
                .collect::<Vec<_>>()
                .join(",");
            format!(
                "{{\"kind\":\"validated\",\"dataset\":{},\"bounds\":[{bounds}]}}",
                js(dataset)
            )
        }
    };
    format!("{{\"status\":\"available\",\"value\":{value},\"refusal\":null}}")
}

/// Explain the recorded evidence and the exact Claim guards used by decision-card admission. Passing
/// those guards alone is not a decision: no actions, flip margin or calibration assessment are invented.
#[must_use]
pub fn why_json(cell: &KeyedCell) -> String {
    let claim = &cell.claim;
    let color = claim.color();
    let uu_guard = claim.verify_uu_slot_for_decision();
    let first_refusal = uu_guard.as_ref().err().or_else(|| color.as_ref().err());
    let uu_terms = claim
        .uu_terms
        .iter()
        .map(|term| {
            format!(
                "{{\"scenario_id\":{},\"description\":{},\"downside_exposure_usd_m\":{},\
         \"exposure_is_finite\":{},\"trigger_indicator\":{}}}",
                js(&term.scenario_id),
                js(&term.description),
                finite_json(term.downside_exposure_usd_m),
                term.downside_exposure_usd_m.is_finite(),
                js(&term.trigger_indicator)
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let interpretation = match &claim.value {
        PublicValue::Number {
            displayed: Err(_), ..
        } => "The stored numeric token is preserved; its display format was refused.",
        PublicValue::Number {
            under_status: Some(_),
            ..
        } => {
            "The nonzero source number is preserved and flagged with the registered subject status."
        }
        PublicValue::Number { .. } => {
            "The source number and its displayed text are preserved; extraction does not verify its truth."
        }
        PublicValue::Nil {
            reason: NilReason::Disposed,
            ..
        } => {
            "The registered disposal status makes this token a missing observation, not numeric zero."
        }
        PublicValue::Nil {
            reason: NilReason::Withheld,
            ..
        } => {
            "The registered withholding status makes this token a missing observation, not numeric zero."
        }
        PublicValue::Nil {
            reason: NilReason::Unknown,
            ..
        } => "The source placeholder does not provide a numeric observation.",
        PublicValue::NotApplicable { .. } => {
            "The source token states that the figure does not apply."
        }
        PublicValue::Missing => "No source cell supplies this figure; no numeric value is imputed.",
    };
    format!(
        "{{\"schema\":\"fmine.claim.why.v1\",\"claim\":{},\"interpretation\":{},\
         \"evidence_class\":{},\"epistemic_status\":{},\"knowledge_state\":{},\
         \"representation\":{},\"support\":{},\"additivity\":{},\"confidentiality\":{},\
         \"color\":{},\"uu_terms\":[{uu_terms}],\"uu_guard\":{},\"public_export_guard\":{},\
         \"decision_admission\":{{\"scope\":\"Claim color and UU guards only\",\"claim_guards_pass\":{},\
         \"first_refusal\":{},\"decision_context\":null,\"flip_margin\":null}},\
         \"calibration\":{{\"failed\":{},\"assessment\":null}},\
         \"freshness\":{{\"recorded_stale\":{},\"assessment\":\"not_assessed\",\
         \"reason\":\"This source projection has no clock or dependency-invalidation replay.\"}}}}",
        cell.to_json(),
        js(interpretation),
        js(&format!("{:?}", claim.evidence)),
        js(&format!("{:?}", claim.epistemic)),
        js(&format!("{:?}", claim.state)),
        js(&format!("{:?}", claim.representation)),
        js(&format!("{:?}", claim.support)),
        js(&format!("{:?}", claim.additivity)),
        js(&format!("{:?}", claim.confidentiality)),
        color_json(&color),
        guard_json(&uu_guard),
        guard_json(&claim.verify_export_clearance(ConfidentialityTier::Public)),
        first_refusal.is_none(),
        first_refusal.map_or_else(|| "null".into(), refusal_json),
        claim.calibration_failed,
        claim.stale,
    )
}

/// Trace a public source leaf to its exact workbook anchors and matching registry rows. Parent IDs are
/// retained but marked unresolved: this projection is not a store of arbitrary derived Claim DAGs.
#[must_use]
pub fn trace_json(cell: &KeyedCell, profile: &IssuerProfile) -> String {
    let claim = &cell.claim;
    let anchors = claim.anchors.iter().map(|anchor| {
        let digest = Digest32(anchor.artifact_sha256).to_hex();
        let filename = std::path::Path::new(&anchor.artifact_path).file_name().and_then(|p| p.to_str());
        let registered = profile.sources.iter().find(|source| {
            Some(source.file_name.as_str()) == filename && source.sha256_hex == digest
                && source.quarter == cell.key.reporting_vintage.release
        });
        let source = registered.map_or_else(|| "null".into(), |source| format!(
            "{{\"file\":{},\"kind\":{},\"release\":{},\"url\":{},\"url_basis\":{},\"retrieved_utc\":{}}}",
            js(&source.file_name), js(source.kind.as_str()), js(&source.quarter), js(&source.url),
            js(&source.url_basis), if profile.retrieved_utc.is_empty() { "null".into() }
                else { js(&profile.retrieved_utc) }
        ));
        format!(
            "{{\"artifact_path\":{},\"sha256\":{},\"locator\":{},\"registered_source\":{source}}}",
            js(&anchor.artifact_path), js(&digest), js(&anchor.locator)
        )
    }).collect::<Vec<_>>().join(",");
    let parents = claim
        .parents
        .iter()
        .map(|id| js(&id.to_string()))
        .collect::<Vec<_>>()
        .join(",");
    let complete = claim.parents.is_empty() && !claim.anchors.is_empty();
    format!(
        "{{\"schema\":\"fmine.claim.trace.v1\",\"claim\":{},\"scope\":{},\
         \"complete_for_claim\":{complete},\"parents\":[{parents}],\"unresolved_parents\":[{parents}],\
         \"anchors\":[{anchors}],\"supersedes\":{},\"recorded_stale\":{},\
         \"boundary\":\"Source cells and their registered interpretation are the leaves of this public projection; no upstream measurement history or dependency replay is asserted.\"}}",
        cell.to_json(),
        js(if complete {
            "source_leaf"
        } else {
            "partial_provenance"
        }),
        claim
            .supersedes
            .map_or_else(|| "null".into(), |id| js(&id.to_string())),
        claim.stale,
    )
}
