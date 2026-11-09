//! Complete deterministic transport declarations, comparison quantities and exact witnesses.
//! Determinism: Exact

use std::fmt::Write as _;

use super::*;
use crate::render::{ids, js, md, ratio, rq};

fn rationals(values: &[Rat]) -> String {
    format!(
        "[{}]",
        values
            .iter()
            .map(|value| rq(*value))
            .collect::<Vec<_>>()
            .join(",")
    )
}

fn certificate_json(c: &TransportCertificate) -> String {
    let arcs = c
        .arcs
        .iter()
        .map(|arc| {
            format!(
                "{{\"source\":{},\"target\":{},\"mass\":{}}}",
                arc.source,
                arc.target,
                rq(arc.mass)
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"radius\":{},\"worst_case_value\":{},\"transport_cost\":{},\"destination_weights\":{},\"arcs\":[{arcs}],\"dual_lambda\":{},\"dual_potentials\":{},\"dual_value\":{},\"duality_gap\":{}}}",
        rq(c.radius),
        rq(c.worst_case_value),
        rq(c.transport_cost),
        rationals(&c.destination_weights),
        rq(c.dual_lambda),
        rationals(&c.dual_potentials),
        rq(c.dual_value),
        rq(c.duality_gap),
    )
}

impl RobustnessSummary {
    #[must_use]
    pub fn to_json(&self) -> String {
        let bundles = self
            .bundles
            .iter()
            .map(|row| {
                format!(
                    "{{\"bundle_id\":{},\"nominal_mean\":{},\"worst_case_mean\":{},\"nominal_mean_regret\":{},\"worst_case_expected_regret\":{},\"loss_certificate\":{},\"regret_certificate\":{}}}",
                    js(&row.bundle_id),
                    rq(row.nominal_mean),
                    rq(row.worst_case_mean),
                    rq(row.nominal_mean_regret),
                    rq(row.worst_case_expected_regret),
                    certificate_json(&row.loss_certificate),
                    certificate_json(&row.regret_certificate),
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        format!(
            "{{\"scope\":\"nominally_feasible_static_bundles\",\"status\":{},\"objective_unit\":{},\"regret_reference\":\"all_nominally_feasible_static_bundles\",\"eligibility_basis\":\"original_nominal_structural_and_tail_loss_screens\",\"stressed_tail_loss_feasibility_certified\":false,\"design\":{},\"state_ids\":{},\"nominal_weights\":{},\"structural_rejections\":{},\"nominal_risk_rejections\":{},\"nominal_expected_value_winners\":{},\"maximin_mean_winners\":{},\"minimax_expected_regret_winners\":{},\"nominal_winner_survives_maximin\":{},\"nominal_winner_survives_minimax_regret\":{},\"bundles\":[{bundles}],\"no_claim\":{}}}",
            js(if self.bundles.is_empty() {
                "no_nominally_feasible_bundle"
            } else {
                "ok"
            }),
            js(&self.objective_unit),
            self.design.to_json(),
            ids(&self.state_ids),
            rationals(&self.nominal_weights),
            ids(&self.structural_rejections),
            ids(&self.nominal_risk_rejections),
            ids(&self.nominal_expected_value_winners),
            ids(&self.maximin_mean_winners),
            ids(&self.minimax_expected_regret_winners),
            self.nominal_winner_survives_maximin,
            self.nominal_winner_survives_minimax_regret,
            js(NO_CLAIM),
        )
    }

    #[must_use]
    pub fn to_markdown(&self) -> String {
        let names = |values: &[String]| {
            if values.is_empty() {
                "none".into()
            } else {
                md(&values.join(", "))
            }
        };
        let mut out = format!(
            "\n## Capital scenario-weight robustness\n\n{NO_CLAIM}\n\n**Objective unit:** {}. **Stress radius:** {}.\n\n**Metric basis:** {}.\n\n**Radius basis:** {}.\n\n**Nominal expected-value co-winners:** {}. **Maximin mean co-winners:** {}. **Minimax expected-regret co-winners:** {}.\n\n**Structural exclusions:** {}. **Nominal risk exclusions:** {}. Their original complete assessment rows retain the reasons and binding limits. No excluded bundle enters this stress comparison or its hindsight benchmark.\n\nThe nominal tail-loss limits have not been certified under the stressed laws. Unlike the preceding nominal regret columns, regret here compares only the nominally feasible population. Loss certificates maximize the negative primary payoff; regret certificates maximize same-state regret. The adversaries are separate.\n\n| Bundle | Nominal mean | Worst-case mean | Nominal expected regret | Worst-case expected regret |\n|---|---:|---:|---:|---:|\n",
            md(&self.objective_unit),
            ratio(self.design.radius),
            md(&self.design.metric_basis),
            md(&self.design.radius_basis),
            names(&self.nominal_expected_value_winners),
            names(&self.maximin_mean_winners),
            names(&self.minimax_expected_regret_winners),
            names(&self.structural_rejections),
            names(&self.nominal_risk_rejections),
        );
        for row in &self.bundles {
            let _ = writeln!(
                out,
                "| {} | {} | {} | {} | {} |",
                md(&row.bundle_id),
                ratio(row.nominal_mean),
                ratio(row.worst_case_mean),
                ratio(row.nominal_mean_regret),
                ratio(row.worst_case_expected_regret),
            );
        }
        if self.bundles.is_empty() {
            out.push_str("\nNo nominally feasible bundle exists; no stress winner or fallback is supplied.\n");
        }
        out.push_str("\n### Declared dimensionless transport geometry\n\n| From state | To state | Distance |\n|---|---|---:|\n");
        for row in &self.design.distances {
            let _ = writeln!(
                out,
                "| {} | {} | {} |",
                md(&row.from_state),
                md(&row.to_state),
                ratio(row.distance),
            );
        }
        for row in &self.bundles {
            for (criterion, certificate) in [
                ("signed loss", &row.loss_certificate),
                ("same-state regret", &row.regret_certificate),
            ] {
                let _ = writeln!(
                    out,
                    "\n### {}: {criterion} certificate\n",
                    md(&row.bundle_id)
                );
                append_certificate(
                    &mut out,
                    &self.state_ids,
                    &self.nominal_weights,
                    certificate,
                );
            }
        }
        out
    }
}

fn append_certificate(
    out: &mut String,
    states: &[String],
    nominal: &[Rat],
    c: &TransportCertificate,
) {
    let _ = writeln!(
        out,
        "Primal objective: {}. Dual objective: {}. Duality gap: {}. Transport cost: {}. Radius: {}. Dual multiplier: {}.\n",
        ratio(c.worst_case_value),
        ratio(c.dual_value),
        ratio(c.duality_gap),
        ratio(c.transport_cost),
        ratio(c.radius),
        ratio(c.dual_lambda),
    );
    out.push_str("| State | Nominal source mass | Adversarial destination mass | Source dual potential |\n|---|---:|---:|---:|\n");
    for (i, state) in states.iter().enumerate() {
        let _ = writeln!(
            out,
            "| {} | {} | {} | {} |",
            md(state),
            ratio(nominal[i]),
            ratio(c.destination_weights[i]),
            ratio(c.dual_potentials[i]),
        );
    }
    out.push_str("\nEvery positive arc follows; unlisted arcs have zero mass.\n\n| From state | To state | Transported mass |\n|---|---|---:|\n");
    for arc in &c.arcs {
        let _ = writeln!(
            out,
            "| {} | {} | {} |",
            md(&states[arc.source]),
            md(&states[arc.target]),
            ratio(arc.mass),
        );
    }
}
