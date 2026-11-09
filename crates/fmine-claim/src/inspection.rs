//! DESIGN (BD-040, BD-115): checked public Claim views shared by source and derived evidence.
//! Determinism: Exact (canonical bytes and ordered metadata; nonfinite diagnostics stay explicit).

use std::fmt::Write as _;

use fmine_core::Digest32;
use fmine_evidence::{Color, Refusal};

use crate::{Claim, Confidentiality};

/// Diagnostic views do not establish disclosure authority, calibration or complete ancestry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublicInspection {
    pub json: String,
    pub why: String,
}

fn js(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn finite(value: f64) -> String {
    if value.is_finite() {
        value.to_string()
    } else {
        "null".into()
    }
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

/// Retain the full canonical body beside a caller's already-encoded typed JSON value.
///
/// Public clearance and canonical identity are checked before any output. The caller owns the
/// value codec and must bind its interpretation to the Claim's value tag; this generic view does
/// not authenticate the value or the metadata that the current Claim body does not hash.
pub fn public_json<T>(
    claim: &Claim<T>,
    category: &str,
    value: impl AsRef<str>,
) -> Result<String, Refusal> {
    claim.verify_export_clearance(&Confidentiality::public())?;
    claim.check_id()?;
    let value = value.as_ref();
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
    Ok(format!(
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
        claim
            .parents
            .iter()
            .map(|id| js(&id.to_string()))
            .collect::<Vec<_>>()
            .join(","),
        claim
            .supersedes
            .map_or_else(|| "null".into(), |id| js(&id.to_string())),
        claim.stale,
        claim.calibration_failed
    ))
}

/// Add the recorded color and UU guard outcomes without converting them into decision authority.
/// Domain-specific interpretation stays with the caller, outside this substrate crate.
pub fn public_inspection<T>(
    claim: &Claim<T>,
    category: &str,
    value: &str,
    interpretation: &str,
) -> Result<PublicInspection, Refusal> {
    let json = public_json(claim, category, value)?;
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
        js(interpretation)
    );
    Ok(PublicInspection { json, why })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AdditivityClass, Representation, SpatialSupport, UuTerm};
    use fmine_core::anchor::SourceAnchor;
    use fmine_evidence::{EpistemicStatus, EvidenceClass, KnowledgeState, KuKind};

    fn source() -> Claim<i32> {
        Claim::new(
            "synthetic\namount",
            7,
            "7",
            Representation::Deterministic,
            SpatialSupport::PeriodAggregate,
            AdditivityClass::ExtensiveAdditive,
            EvidenceClass::Assumed,
            EpistemicStatus::KnownUnknown(KuKind::Parametric),
            KnowledgeState::Unknown,
            Confidentiality::public(),
            vec![],
            vec![],
            vec![SourceAnchor::new(
                "synthetic/\"cost\".xlsx",
                [0x11; 32],
                "Costs!A1",
            )],
        )
    }

    #[test]
    #[allow(clippy::expect_used)]
    fn public_inspection_keeps_identity_and_refusals_without_upgrading_source_evidence() {
        let claim = source();
        let before = claim.canonical_body();
        let view = public_inspection(&claim, "source\tcell", "7", "Known\nlimitations")
            .expect("public source inspection");
        assert_eq!(claim.canonical_body(), before);
        for retained in [
            "\"category\":\"source\\tcell\"",
            "\"metric_key\":\"synthetic\\namount\"",
            "\"artifact_path\":\"synthetic/\\\"cost\\\".xlsx\"",
            "\"value\":7,\"evidence\":\"Assumed\"",
            "\"knowledge\":\"Unknown\"",
        ] {
            assert!(view.json.contains(retained), "{retained}");
        }
        assert!(view.json.contains(&claim.claim_id.to_string()));
        assert!(view.why.contains("FMINE-REF-204"));
        assert!(view.why.contains("\"claim_guards_pass\":false"));
        assert!(
            view.why
                .contains("\"interpretation\":\"Known\\nlimitations\"")
        );
        let mut forged = claim.clone();
        forged.metric_key.push_str(".edited");
        assert_eq!(
            public_json(&forged, "source", "7")
                .expect_err("bad id")
                .code,
            "FMINE-REF-507"
        );
        let mut private = claim;
        private.confidentiality = Confidentiality::internal();
        assert_eq!(
            public_json(&private, "source", "7")
                .expect_err("private source")
                .code,
            "FMINE-REF-301"
        );
    }

    #[test]
    #[allow(clippy::expect_used)]
    fn inspection_retains_failed_calibration_staleness_and_nonfinite_unknown_exposure() {
        let mut claim = source();
        claim.stale = true;
        claim.calibration_failed = true;
        claim.uu_terms.push(UuTerm {
            scenario_id: "synthetic-unknown".into(),
            description: "Unbounded diagnostic, not a finite downside estimate".into(),
            downside_exposure_usd_m: f64::INFINITY,
            trigger_indicator: "unknown".into(),
        });
        let view = public_inspection(&claim, "diagnostic", "7", "No decision authority")
            .expect("recorded guard results remain inspectable");
        assert!(
            view.json
                .contains("\"downside_exposure_usd_m\":null,\"exposure_is_finite\":false")
        );
        assert!(
            view.json
                .contains("\"stale\":true,\"calibration_failed\":true")
        );
        assert!(view.why.contains("FMINE-REF-205"));
        assert!(view.why.contains("\"claim_guards_pass\":false"));
        assert!(view.why.contains("\"recorded_stale\":true"));
        assert!(
            view.why
                .contains("\"decision_context\":null,\"flip_margin\":null")
        );
    }
}
