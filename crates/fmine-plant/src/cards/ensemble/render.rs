//! DESIGN: complete weighted recovery results and source-bound conditional scope.
//! Determinism: Exact over stored member order and numerical values.

use super::*;
use fmine_core::Digest32;
use std::fmt::Write as _;

fn quoted(value: &str) -> String {
    let mut out = String::from("\"");
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            ch if ch.is_control() => {
                let _ = write!(out, "\\u{:04x}", ch as u32);
            }
            _ => out.push(ch),
        }
    }
    out.push('"');
    out
}
fn markdown(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('|', "&#124;")
        .replace('`', "&#96;")
        .replace(['\n', '\r'], " ")
}
fn exact(value: ExactQty) -> String {
    format!(
        "{{\"numerator\":\"{}\",\"denominator\":\"{}\"}}",
        value.num(),
        value.den()
    )
}
fn number(value: f64) -> String {
    if value == 0.0 {
        "0".into()
    } else {
        value.to_string()
    }
}
fn claim_json<T>(claim: &Claim<T>) -> String {
    let sources = claim
        .anchors
        .iter()
        .map(|source| {
            format!(
                "{{\"path\":{},\"sha256\":{},\"locator\":{}}}",
                quoted(&source.artifact_path),
                quoted(&Digest32(source.artifact_sha256).to_hex()),
                quoted(&source.locator),
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let parents = claim
        .parents
        .iter()
        .map(|id| quoted(&id.to_string()))
        .collect::<Vec<_>>()
        .join(",");
    let uu = claim.uu_terms.iter().map(|term| format!(
        "{{\"scenario_id\":{},\"description\":{},\"downside_exposure_usd_m\":{},\"trigger_indicator\":{}}}",
        quoted(&term.scenario_id), quoted(&term.description),
        term.downside_exposure_usd_m, quoted(&term.trigger_indicator),
    )).collect::<Vec<_>>().join(",");
    format!(
        "{{\"id\":{},\"metric\":{},\"value_tag\":{},\"representation\":{},\"support\":{},\"additivity\":{},\"evidence\":{},\"epistemic\":{},\"state\":{},\"confidentiality\":{},\"calibration\":\"uncalibrated\",\"calibration_failed\":{},\"stale\":{},\"supersedes\":{},\"uu_terms\":[{uu}],\"parents\":[{parents}],\"sources\":[{sources}]}}",
        quoted(&claim.claim_id.to_string()),
        quoted(&claim.metric_key),
        quoted(claim.value_tag()),
        quoted(&format!("{:?}", claim.representation)),
        quoted(&format!("{:?}", claim.support)),
        quoted(&format!("{:?}", claim.additivity)),
        quoted(&format!("{:?}", claim.evidence)),
        quoted(&format!("{:?}", claim.epistemic)),
        quoted(&format!("{:?}", claim.state)),
        quoted(&claim.confidentiality.to_string()),
        claim.calibration_failed,
        claim.stale,
        claim
            .supersedes
            .map_or_else(|| "null".into(), |id| quoted(&id.to_string())),
    )
}
fn unit(value: InputUnit) -> &'static str {
    match value {
        InputUnit::GramsPerDryMetricTonne => "g_per_dry_t",
        InputUnit::TroyOuncesPerDryShortTon => "oz_per_dry_st",
        InputUnit::DryMassPercent => "dry_mass_percent",
        InputUnit::DryMassFraction => "dry_mass_fraction",
        InputUnit::PartsPerMillionDryMass => "ppm_dry_mass",
        InputUnit::SpikeTestPercent => "spike_test_percent",
        InputUnit::Micrometres => "micrometres",
        InputUnit::Hours => "hours",
    }
}
fn member_json(member: &RecoveryMemberReceipt) -> String {
    let receipt = &member.receipt;
    let values = receipt
        .inputs()
        .value
        .values
        .iter()
        .map(|(name, value)| {
            format!(
                "{{\"name\":{},\"measurand\":{},\"unit\":{},\"value\":{}}}",
                quoted(name),
                quoted(&value.measurand),
                quoted(unit(value.unit)),
                exact(value.value),
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"id\":{},\"weight\":{},\"inputs\":{{\"claim\":{},\"values\":[{values}]}},\"admitted_claim\":{},\"receipt_digest\":{},\"result\":{}}}",
        quoted(&member.id),
        exact(member.weight),
        claim_json(receipt.inputs()),
        claim_json(receipt.claim()),
        quoted(receipt.digest()),
        receipt.result_json(),
    )
}

impl RecoveryReceipt {
    /// Shared nominal projection for one evaluation or a population member.
    /// Retains the existing recovery worksheet's exact field order and shape.
    pub fn result_json(&self) -> String {
        let result = &self.claim().value;
        let operations = result
            .operational_fractions
            .iter()
            .map(|value| number(value.value()))
            .collect::<Vec<_>>()
            .join(",");
        format!(
            "{{\"test_work_fraction\":{},\"operational_fractions\":[{operations}],\"corrected_before_cap\":{},\"cap_applied\":{},\"recovery_fraction\":{},\"route\":{}}}",
            number(result.test_work_fraction.value()),
            number(result.corrected_before_cap.value()),
            result.cap_applied,
            number(result.recovery_fraction.value()),
            quoted(&result.route),
        )
    }
}

impl RecoveryEnsembleReceipt {
    /// Deterministic normalized result. Submitted numeric tokens remain the
    /// importing adapter's responsibility; every exact value is retained here.
    pub fn to_json(&self) -> String {
        let value = &self.claim.value;
        let scope = &value.scope;
        let members = self
            .members
            .iter()
            .map(member_json)
            .collect::<Vec<_>>()
            .join(",");
        format!(
            "{{\"schema\":\"fmine.plant-recovery-ensemble.v1\",\"mode\":\"declared_joint_population\",\"calibration\":\"uncalibrated\",\"population_claim\":{},\"admitted_claim\":{},\"receipt_digest\":{},\"summary\":{{\"scope\":{{\"circuit\":{},\"material_domain\":{},\"dry_basis\":{},\"element\":{}}},\"start_ns\":\"{}\",\"end_ns\":\"{}\",\"route\":{},\"scenario_count\":{},\"mean\":{},\"variance\":{},\"minimum\":{},\"p10\":{},\"p50\":{},\"p90\":{},\"maximum\":{},\"probability_capped\":{}}},\"members\":[{members}],\"quantile_convention\":\"inverse empirical CDF; exact cumulative probability; no interpolation\",\"no_claim\":{}}}",
            claim_json(&self.population),
            claim_json(&self.claim),
            quoted(&self.digest),
            quoted(&scope.circuit),
            quoted(&scope.material_domain),
            quoted(&scope.dry_basis),
            quoted(&scope.element),
            value.interval.start_ns,
            value.interval.end_ns,
            quoted(&value.route),
            value.scenario_count,
            value.mean.value(),
            value.variance.value(),
            value.minimum.value(),
            value.p10.value(),
            value.p50.value(),
            value.p90.value(),
            value.maximum.value(),
            exact(value.probability_capped),
            quoted(NO_CLAIM),
        )
    }

    /// Human review of the same conditional distribution, retaining every named
    /// joint state and the source/receipt identities needed to inspect it.
    pub fn to_markdown(&self) -> String {
        let value = &self.claim.value;
        let mut out = format!(
            "### Declared joint recovery population\n\nCircuit: {}. Domain: {}. Element: {}. Dry basis: {}. Processing interval: {}–{} ns. Route: {}.\n\nMean recovery: {}. Population variance: {} fraction². P10/P50/P90: {} / {} / {}. Support: [{} , {}]. Exact cap-event probability: {}/{}.\n\n{} scenarios; probabilities are **uncalibrated**. Quantiles use the inverse empirical CDF with exact cumulative weights and no interpolation.\n\n| Scenario | Exact probability | Recovery fraction | Explicit cap applied | Input Claim | Evaluation Claim | Receipt |\n|---|---:|---:|---|---|---|---|\n",
            markdown(&value.scope.circuit),
            markdown(&value.scope.material_domain),
            markdown(&value.scope.element),
            markdown(&value.scope.dry_basis),
            value.interval.start_ns,
            value.interval.end_ns,
            markdown(&value.route),
            value.mean.value(),
            value.variance.value(),
            value.p10.value(),
            value.p50.value(),
            value.p90.value(),
            value.minimum.value(),
            value.maximum.value(),
            value.probability_capped.num(),
            value.probability_capped.den(),
            value.scenario_count,
        );
        for member in &self.members {
            let _ = writeln!(
                out,
                "| {} | {}/{} | {} | {} | {} | {} | {} |",
                markdown(&member.id),
                member.weight.num(),
                member.weight.den(),
                recovery(member),
                member.receipt.claim().value.cap_applied,
                member.receipt.inputs().claim_id,
                member.receipt.claim().claim_id,
                member.receipt.digest(),
            );
        }
        let _ = write!(
            out,
            "\nPopulation Claim: {}. Result Claim: {}. Receipt: {}. Evidence: {:?}; state: {:?}; confidentiality: {}.\n\nSources:\n\n",
            self.population.claim_id,
            self.claim.claim_id,
            self.digest,
            self.claim.evidence,
            self.claim.state,
            markdown(&self.claim.confidentiality.to_string()),
        );
        for source in &self.claim.anchors {
            let _ = writeln!(
                out,
                "- {} — {}; SHA-256 {}",
                markdown(&source.artifact_path),
                markdown(&source.locator),
                Digest32(source.artifact_sha256).to_hex(),
            );
        }
        let _ = write!(out, "\n{NO_CLAIM}\n");
        out
    }
}
