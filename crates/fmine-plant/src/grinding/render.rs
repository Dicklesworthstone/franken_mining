//! DESIGN: complete conditional grinding results, declared domains and source Claims.
//! Determinism: DeterministicPerIsa

use super::*;
use fmine_core::Digest32;
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
fn number(value: f64) -> String {
    if value == 0.0 {
        "0".into()
    } else {
        value.to_string()
    }
}
fn exact(value: ExactQty) -> String {
    format!(
        "{{\"numerator\":\"{}\",\"denominator\":\"{}\"}}",
        value.num(),
        value.den()
    )
}
fn claim_json<T>(c: &Claim<T>) -> String {
    let sources = c
        .anchors
        .iter()
        .map(|a| {
            format!(
                "{{\"path\":{},\"sha256\":{},\"locator\":{}}}",
                quoted(&a.artifact_path),
                quoted(&Digest32(a.artifact_sha256).to_hex()),
                quoted(&a.locator),
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let parents = c
        .parents
        .iter()
        .map(|id| quoted(&id.to_string()))
        .collect::<Vec<_>>()
        .join(",");
    let uu = c.uu_terms.iter().map(|u| format!(
        "{{\"scenario_id\":{},\"description\":{},\"downside_exposure_usd_m\":{},\"trigger_indicator\":{}}}",
        quoted(&u.scenario_id), quoted(&u.description), number(u.downside_exposure_usd_m), quoted(&u.trigger_indicator),
    )).collect::<Vec<_>>().join(",");
    let mut body = String::new();
    for b in c.canonical_body() {
        let _ = write!(body, "{b:02x}");
    }
    format!(
        concat!(
            "{{\"id\":{},\"metric\":{},\"value_tag\":{},\"canonical_body_hex\":{},",
            "\"representation\":{},\"support\":{},\"additivity\":{},\"evidence\":{},",
            "\"epistemic\":{},\"state\":{},\"confidentiality\":{},\"calibration\":\"uncalibrated\",",
            "\"stale\":{},\"calibration_failed\":{},\"supersedes\":{},\"uu_terms\":[{uu}],\"parents\":[{parents}],\"sources\":[{sources}]}}"
        ),
        quoted(&c.claim_id.to_string()),
        quoted(&c.metric_key),
        quoted(c.value_tag()),
        quoted(&body),
        quoted(&format!("{:?}", c.representation)),
        quoted(&format!("{:?}", c.support)),
        quoted(&format!("{:?}", c.additivity)),
        quoted(&format!("{:?}", c.evidence)),
        quoted(&format!("{:?}", c.epistemic)),
        quoted(&format!("{:?}", c.state)),
        quoted(&c.confidentiality.to_string()),
        c.stale,
        c.calibration_failed,
        c.supersedes
            .map_or_else(|| "null".into(), |id| quoted(&id.to_string())),
        uu = uu,
        parents = parents,
        sources = sources,
    )
}
fn distribution<D: Copy>(d: &Distribution<Qty<D, Dry>>) -> String {
    format!(
        "{{\"mean\":{},\"minimum\":{},\"p10\":{},\"p50\":{},\"p90\":{},\"maximum\":{}}}",
        number(d.mean.value()),
        number(d.minimum.value()),
        number(d.p10.value()),
        number(d.p50.value()),
        number(d.p90.value()),
        number(d.maximum.value()),
    )
}
fn model_json(c: &Claim<BondModel>) -> String {
    let m = &c.value;
    let s = &m.scope;
    let domains = m
        .domains()
        .iter()
        .enumerate()
        .map(|(i, [lo, hi])| {
            format!(
                "{}:{{\"minimum\":{},\"maximum\":{}}}",
                quoted(AXES[i]),
                number(*lo),
                number(*hi)
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!(
        concat!(
            "{{\"id\":{},\"version\":{},\"family\":{},\"scope\":{{\"boundary\":{},\"circuit\":{},\"blend\":{},",
            "\"start_ns\":\"{}\",\"end_ns\":\"{}\"}},\"work_index_test_basis\":{},\"validity_basis\":{},",
            "\"complete_joint_population\":{},\"domains\":{{{domains}}},\"claim\":{}}}"
        ),
        quoted(&m.id),
        quoted(&m.version),
        quoted(MODEL_FAMILY),
        quoted(&s.boundary),
        quoted(&s.circuit),
        quoted(&s.blend),
        s.interval.start_ns,
        s.interval.end_ns,
        quoted(&m.work_index_test_basis),
        quoted(&m.validity_basis),
        m.complete_joint_population,
        claim_json(c),
        domains = domains,
    )
}
fn member_json(input: &Claim<GrindingScenario>, p: &GrindingPoint) -> String {
    let s = &input.value;
    format!(
        concat!(
            "{{\"id\":{},\"weight\":{},\"inputs\":{{\"work_index_kwh_per_dry_t\":{},\"f80_microns\":{},",
            "\"p80_microns\":{},\"installed_power_kw\":{},\"utilization\":{}}},\"input_claim\":{},",
            "\"specific_energy_kwh_per_dry_t\":{},\"effective_power_kw\":{},\"throughput_dry_t_per_hour\":{},\"state\":{}}}"
        ),
        quoted(&s.id),
        exact(s.weight),
        number(s.work_index.value()),
        number(s.feed_size.value()),
        number(s.product_size.value()),
        number(s.installed_power.value()),
        number(s.utilization.value()),
        claim_json(input),
        number(p.specific_energy.value()),
        number(p.effective_power.value()),
        number(p.throughput.value()),
        quoted(p.state.as_str()),
    )
}

pub(super) fn worksheet_json(submitted: &str, receipt: &GrindingReceipt) -> String {
    let r = receipt.summary();
    let members = receipt
        .scenarios()
        .iter()
        .zip(receipt.points())
        .map(|(s, p)| member_json(s, p))
        .collect::<Vec<_>>()
        .join(",");
    format!(
        concat!(
            "{{\"schema\":\"fmine.plant-grinding-result.v1\",\"submitted_worksheet\":{},",
            "\"model\":{},\"scenarios\":[{members}],\"summary\":{{\"scenario_count\":{},",
            "\"specific_energy_kwh_per_dry_t\":{},\"throughput_dry_t_per_hour\":{},\"shutdown_probability\":{}}},",
            "\"admitted_claim\":{},\"receipt_digest\":{},\"calibration\":\"uncalibrated\",",
            "\"units\":{{\"work_index\":\"kWh/dry_metric_tonne\",\"specific_energy\":\"kWh/dry_metric_tonne\",",
            "\"f80\":\"micron\",\"p80\":\"micron\",\"power\":\"kW\",\"utilization\":\"fraction\",\"throughput\":\"dry_metric_tonne/hour\"}},",
            "\"quantile_convention\":\"inverse empirical CDF with exact cumulative weights; no interpolation\",",
            "\"representation_quantity\":\"throughput_dry_t_per_hour\",\"no_claim\":{}}}"
        ),
        quoted(submitted),
        model_json(receipt.model()),
        r.scenario_count,
        distribution(&r.specific_energy),
        distribution(&r.throughput),
        exact(r.shutdown_probability),
        claim_json(receipt.claim()),
        quoted(receipt.digest()),
        quoted(worksheet::NO_CLAIM),
        members = members,
    )
}

pub(super) fn worksheet_markdown(receipt: &GrindingReceipt) -> String {
    let r = receipt.summary();
    let m = &receipt.model().value;
    let mut out = format!(
        "# Declared Bond grinding energy and throughput\n\n{}\n\n\
         Boundary: {}. Circuit: {}. Processing blend: {}. Interval: {}–{} ns.\n\n\
         Card: {} version {}. Work-index test basis: {}. Validity basis: {}.\n\n\
         The model uses E = 10 Wi (1/sqrt(P80) - 1/sqrt(F80)) and Q = power × utilization / E.\n\n\
         ## Conditional scenario distribution\n\n\
         Statistics summarize {} declared alternative states of this blend. Their probabilities and quantiles are **uncalibrated**.\n\n\
         | Quantity | Mean | Minimum | P10 | P50 | P90 | Maximum |\n\
         |---|---:|---:|---:|---:|---:|---:|\n",
        worksheet::NO_CLAIM,
        markdown(&r.scope.boundary),
        markdown(&r.scope.circuit),
        markdown(&r.scope.blend),
        r.scope.interval.start_ns,
        r.scope.interval.end_ns,
        markdown(&m.id),
        markdown(&m.version),
        markdown(&m.work_index_test_basis),
        markdown(&m.validity_basis),
        r.scenario_count,
    );
    for (label, values) in [
        (
            "Specific energy (kWh/dry metric tonne)",
            [
                r.specific_energy.mean.value(),
                r.specific_energy.minimum.value(),
                r.specific_energy.p10.value(),
                r.specific_energy.p50.value(),
                r.specific_energy.p90.value(),
                r.specific_energy.maximum.value(),
            ],
        ),
        (
            "Throughput (dry metric tonnes/hour)",
            [
                r.throughput.mean.value(),
                r.throughput.minimum.value(),
                r.throughput.p10.value(),
                r.throughput.p50.value(),
                r.throughput.p90.value(),
                r.throughput.maximum.value(),
            ],
        ),
    ] {
        let _ = writeln!(
            out,
            "| {label} | {} | {} | {} | {} | {} | {} |",
            values[0], values[1], values[2], values[3], values[4], values[5]
        );
    }
    let _ = writeln!(
        out,
        "\nShutdown probability: {}/{}. Result Claim: {}.\n",
        r.shutdown_probability.num(),
        r.shutdown_probability.den(),
        receipt.claim().claim_id
    );
    out.push_str("## Complete joint inputs and outputs\n\nWi and E use kWh per dry metric tonne; F80/P80 are microns; power is kW; Q is dry metric tonnes/hour.\n\n| Scenario | Exact probability | Wi | F80 | P80 | Installed power | Utilization | E | Q | State | Input Claim |\n|---|---:|---:|---:|---:|---:|---:|---:|---:|---|---|\n");
    for (input, p) in receipt.scenarios().iter().zip(receipt.points()) {
        let s = &input.value;
        let _ = writeln!(
            out,
            "| {} | {}/{} | {} | {} | {} | {} | {} | {} | {} | {} | {} |",
            markdown(&s.id),
            s.weight.num(),
            s.weight.den(),
            s.work_index.value(),
            s.feed_size.value(),
            s.product_size.value(),
            s.installed_power.value(),
            s.utilization.value(),
            p.specific_energy.value(),
            p.throughput.value(),
            p.state.as_str(),
            input.claim_id
        );
    }
    out.push_str("\n## Declared closed validity ranges\n\n| Coordinate | Minimum | Maximum |\n|---|---:|---:|\n");
    for (axis, [lo, hi]) in m.domains().iter().enumerate() {
        let _ = writeln!(out, "| {} | {lo} | {hi} |", AXES[axis]);
    }
    let _ = writeln!(
        out,
        "\nModel Claim: {}.\n\n## Source evidence\n",
        receipt.model().claim_id
    );
    for (id, anchors) in std::iter::once((receipt.model().claim_id, &receipt.model().anchors))
        .chain(receipt.scenarios().iter().map(|s| (s.claim_id, &s.anchors)))
    {
        for a in anchors {
            let _ = writeln!(
                out,
                "- {id}: {} at {}; SHA-256 {}.",
                markdown(&a.artifact_path),
                markdown(&a.locator),
                Digest32(a.artifact_sha256).to_hex()
            );
        }
    }
    let _ = writeln!(out, "\nReceipt digest: {}.\n", receipt.digest());
    out
}
