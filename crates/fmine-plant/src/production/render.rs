//! DESIGN: the same complete joined population in machine and human views.
//! Determinism: DeterministicPerIsa

use super::*;
use fmine_core::{Digest32, anchor::SourceAnchor};
use std::fmt::Write as _;

fn quoted(value: &str) -> String {
    let mut out = String::from("\"");
    for c in value.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c if c.is_control() => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
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

fn distribution<D: Copy, B: Copy>(value: &Distribution<Qty<D, B>>) -> String {
    format!(
        "{{\"mean\":{},\"minimum\":{},\"p10\":{},\"p50\":{},\"p90\":{},\"maximum\":{}}}",
        value.mean.value(),
        value.minimum.value(),
        value.p10.value(),
        value.p50.value(),
        value.p90.value(),
        value.maximum.value(),
    )
}

fn source_json(source: &SourceAnchor) -> String {
    format!(
        "{{\"path\":{},\"sha256\":{},\"locator\":{}}}",
        quoted(&source.artifact_path),
        quoted(&Digest32(source.artifact_sha256).to_hex()),
        quoted(&source.locator),
    )
}

fn claim_json<T>(claim: &Claim<T>) -> String {
    let mut body = String::new();
    for byte in claim.canonical_body() {
        let _ = write!(body, "{byte:02x}");
    }
    let sources = claim
        .anchors
        .iter()
        .map(source_json)
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
        concat!(
            "{{\"id\":{},\"metric\":{},\"value_tag\":{},\"canonical_body_hex\":{},",
            "\"representation\":{},\"support\":{},\"additivity\":{},\"evidence\":{},",
            "\"epistemic\":{},\"state\":{},\"confidentiality\":{},\"calibration\":\"uncalibrated\",",
            "\"stale\":{},\"calibration_failed\":{},\"supersedes\":{},",
            "\"uu_terms\":[{uu}],\"parents\":[{parents}],\"sources\":[{sources}]}}"
        ),
        quoted(&claim.claim_id.to_string()),
        quoted(&claim.metric_key),
        quoted(claim.value_tag()),
        quoted(&body),
        quoted(&format!("{:?}", claim.representation)),
        quoted(&format!("{:?}", claim.support)),
        quoted(&format!("{:?}", claim.additivity)),
        quoted(&format!("{:?}", claim.evidence)),
        quoted(&format!("{:?}", claim.epistemic)),
        quoted(&format!("{:?}", claim.state)),
        quoted(&claim.confidentiality.to_string()),
        claim.stale,
        claim.calibration_failed,
        claim
            .supersedes
            .map_or_else(|| "null".into(), |id| quoted(&id.to_string())),
        uu = uu,
        parents = parents,
        sources = sources,
    )
}

pub(super) fn worksheet_json(
    submitted: &str,
    source: &SourceAnchor,
    receipt: &ProductionReceipt,
) -> String {
    let summary = receipt.summary();
    let scope = &summary.grinding_scope;
    let recovery_scope = &summary.recovery_scope;
    let inputs = &receipt.inputs().value;
    let members = receipt.points().iter().map(|point| format!(
        concat!(
            "{{\"id\":{},\"weight\":{},\"grade_g_per_dry_t\":{},\"scheduled_hours\":{},",
            "\"grinding_source\":{},\"recovery_source\":{},\"throughput_dry_t_per_hour\":{},",
            "\"recovery_fraction\":{},\"dry_tonnes\":{},\"contained_metal_kg\":{},\"recovered_metal_kg\":{}}}"
        ),
        quoted(&point.scenario_id), exact(point.weight), exact(point.grade_g_per_dry_t),
        exact(point.scheduled_hours), quoted(&point.grinding_source.to_string()),
        quoted(&point.recovery_source.to_string()), point.throughput.value(),
        point.recovery_fraction.value(), point.dry_tonnes.value(),
        point.contained_metal.value(), point.recovered_metal.value(),
    )).collect::<Vec<_>>().join(",");
    format!(
        concat!(
            "{{\"schema\":\"fmine.plant-production-result.v1\",\"submitted_worksheet\":{},\"source\":{},",
            "\"scope\":{{\"boundary\":{},\"circuit\":{},\"blend\":{},\"material_domain\":{},",
            "\"dry_basis\":{},\"element\":{},\"start_ns\":\"{}\",\"end_ns\":\"{}\"}},",
            "\"coupling_basis\":{},\"grade_coordinate\":{},\"product_size_coordinate\":{},",
            "\"grinding_receipt_digest\":{},\"recovery_receipt_digest\":{},",
            "\"inputs_claim\":{},\"admitted_claim\":{},\"receipt_digest\":{},",
            "\"summary\":{{\"scenario_count\":{},\"throughput_dry_t_per_hour\":{},\"recovery_fraction\":{},",
            "\"dry_tonnes\":{},\"contained_metal_kg\":{},\"recovered_metal_kg\":{}}},",
            "\"scenarios\":[{members}],\"calibration\":\"uncalibrated\",",
            "\"representation_quantity\":\"recovered_metal_kg\",",
            "\"quantile_convention\":\"inverse CDF with exact cumulative weights; no interpolation\",",
            "\"no_claim\":{}}}"
        ),
        quoted(submitted),
        source_json(source),
        quoted(&scope.boundary),
        quoted(&scope.circuit),
        quoted(&scope.blend),
        quoted(&recovery_scope.material_domain),
        quoted(&recovery_scope.dry_basis),
        quoted(&recovery_scope.element),
        scope.interval.start_ns,
        scope.interval.end_ns,
        quoted(&inputs.coupling_basis),
        quoted(&inputs.grade_coordinate),
        quoted(&inputs.product_size_coordinate),
        quoted(receipt.grinding().digest()),
        quoted(receipt.recovery().digest()),
        claim_json(receipt.inputs()),
        claim_json(receipt.claim()),
        quoted(receipt.digest()),
        summary.scenario_count,
        distribution(&summary.throughput),
        distribution(&summary.recovery),
        distribution(&summary.dry_tonnes),
        distribution(&summary.contained_metal),
        distribution(&summary.recovered_metal),
        quoted(NO_CLAIM),
        members = members,
    )
}

pub(super) fn worksheet_markdown(receipt: &ProductionReceipt) -> String {
    let summary = receipt.summary();
    let scope = &summary.grinding_scope;
    let inputs = &receipt.inputs().value;
    let mut out = format!(
        "# Declared joint plant production\n\n{NO_CLAIM}\n\nBoundary: {}. Circuit: {}. Blend: {}. Element: {}. Physical interval: {}–{} ns.\n\nCoupling basis: {}. Grade coordinate: {}. Product-size coordinate: {}.\n\n## Conditional scenario distribution\n\nProbabilities remain **uncalibrated**. Quantiles use exact cumulative scenario weights without interpolation. Products are calculated within each shared scenario before averaging.\n\n| Quantity | Mean | Minimum | P10 | P50 | P90 | Maximum |\n|---|---:|---:|---:|---:|---:|---:|\n",
        markdown(&scope.boundary),
        markdown(&scope.circuit),
        markdown(&scope.blend),
        markdown(&summary.recovery_scope.element),
        scope.interval.start_ns,
        scope.interval.end_ns,
        markdown(&inputs.coupling_basis),
        markdown(&inputs.grade_coordinate),
        markdown(&inputs.product_size_coordinate),
    );
    macro_rules! row {
        ($label:expr, $distribution:expr) => {{
            let d = $distribution;
            let _ = writeln!(
                out,
                "| {} | {} | {} | {} | {} | {} | {} |",
                $label,
                d.mean.value(),
                d.minimum.value(),
                d.p10.value(),
                d.p50.value(),
                d.p90.value(),
                d.maximum.value()
            );
        }};
    }
    row!("Throughput (dry metric tonnes/hour)", &summary.throughput);
    row!("Recovery (fraction)", &summary.recovery);
    row!("Dry metric tonnes", &summary.dry_tonnes);
    row!("Contained elemental kg", &summary.contained_metal);
    row!("Recovered elemental kg", &summary.recovered_metal);
    out.push_str("\n## Complete shared scenarios\n\nScheduled hours include the utilization already applied by the grinding model.\n\n| Scenario | Exact probability | Grade (g/dry t) | Scheduled hours | Throughput (dry t/h) | Recovery | Dry tonnes | Contained kg | Recovered kg |\n|---|---:|---:|---:|---:|---:|---:|---:|---:|\n");
    for point in receipt.points() {
        let _ = writeln!(
            out,
            "| {} | {}/{} | {}/{} | {}/{} | {} | {} | {} | {} | {} |",
            markdown(&point.scenario_id),
            point.weight.num(),
            point.weight.den(),
            point.grade_g_per_dry_t.num(),
            point.grade_g_per_dry_t.den(),
            point.scheduled_hours.num(),
            point.scheduled_hours.den(),
            point.throughput.value(),
            point.recovery_fraction.value(),
            point.dry_tonnes.value(),
            point.contained_metal.value(),
            point.recovered_metal.value(),
        );
    }
    let _ = write!(
        out,
        "\n## Source evidence\n\nInput Claim: {}. Result Claim: {}.\n\n",
        receipt.inputs().claim_id,
        receipt.claim().claim_id
    );
    for source in &receipt.claim().anchors {
        let _ = writeln!(
            out,
            "- {} at {}; SHA-256 {}.",
            markdown(&source.artifact_path),
            markdown(&source.locator),
            Digest32(source.artifact_sha256).to_hex()
        );
    }
    let _ = write!(
        out,
        "\nGrinding receipt: {}. Recovery receipt: {}. Production receipt: {}.\n",
        receipt.grinding().digest(),
        receipt.recovery().digest(),
        receipt.digest()
    );
    out
}
