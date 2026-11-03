//! Complete exact transport results and their conditional interpretation.
//! Determinism: Exact

use super::super::render::{ids, js, md, ratio, rq};
use super::*;
use std::fmt::Write as _;

fn rationals(values: &[Rat]) -> String {
    format!(
        "[{}]",
        values.iter().map(|v| rq(*v)).collect::<Vec<_>>().join(",")
    )
}

impl RobustnessDesign {
    #[must_use]
    pub fn to_json(&self) -> String {
        let mut distances: Vec<_> = self.distances.iter().collect();
        distances.sort_by(|a, b| (&a.from_state, &a.to_state).cmp(&(&b.from_state, &b.to_state)));
        let distances = distances
            .iter()
            .map(|r| {
                format!(
                    "{{\"from_state\":{},\"to_state\":{},\"distance\":{}}}",
                    js(&r.from_state),
                    js(&r.to_state),
                    rq(r.distance),
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        format!(
            "{{\"model_key\":{},\"method\":{},\"radius\":{},\"metric_basis\":{},\"radius_basis\":{},\"distances\":[{distances}]}}",
            js(&self.model_key),
            js(&self.method),
            rq(self.radius),
            js(&self.metric_basis),
            js(&self.radius_basis),
        )
    }
}

fn certificate_json(c: &TransportCertificate) -> String {
    let arcs = c
        .arcs
        .iter()
        .map(|a| {
            format!(
                "{{\"source\":{},\"target\":{},\"mass\":{}}}",
                a.source,
                a.target,
                rq(a.mass),
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
        let alternatives = self.alternatives.iter().map(|a| format!(
            "{{\"action_id\":{},\"nominal_mean\":{},\"worst_case_mean\":{},\"nominal_mean_regret\":{},\"worst_case_expected_regret\":{},\"loss_certificate\":{},\"regret_certificate\":{}}}",
            js(&a.action_id), rq(a.nominal_mean), rq(a.worst_case_mean), rq(a.nominal_mean_regret),
            rq(a.worst_case_expected_regret), certificate_json(&a.loss_certificate), certificate_json(&a.regret_certificate),
        )).collect::<Vec<_>>().join(",");
        format!(
            "{{\"design\":{},\"scope\":\"act_now_actions\",\"distance_unit\":\"dimensionless\",\"currency\":{},\"monetary_scale\":\"major_currency_units\",\"state_ids\":{},\"nominal_weights\":{},\"nominal_expected_value_winners\":{},\"maximin_mean_order\":{},\"maximin_mean_winners\":{},\"minimax_expected_regret_order\":{},\"minimax_expected_regret_winners\":{},\"nominal_winner_survives_maximin\":{},\"nominal_winner_survives_minimax_regret\":{},\"alternatives\":[{alternatives}],\"no_claim\":{}}}",
            self.design.to_json(),
            js(currency_name(self.currency)),
            ids(&self.state_ids),
            rationals(&self.nominal_weights),
            ids(&self.nominal_expected_value_winners),
            ids(&self.maximin_mean_order),
            ids(&self.maximin_mean_winners),
            ids(&self.minimax_expected_regret_order),
            ids(&self.minimax_expected_regret_winners),
            self.nominal_winner_survives_maximin,
            self.nominal_winner_survives_minimax_regret,
            js(NO_CLAIM),
        )
    }

    /// Only the sealed EvaluatedCard renders Markdown; a freely constructed summary is not admitted.
    #[must_use]
    pub(in crate::card) fn to_markdown(&self) -> String {
        let mut out = format!(
            "\n## Wasserstein robustness\n\n{NO_CLAIM}\n\n**Scope:** act-now actions. **Radius:** {} in the declared dimensionless metric.\n\n**Metric basis:** {}\n\n**Radius basis:** {}\n\n**Maximin expected-value co-winners:** {}. **Minimax expected-regret co-winners:** {}.\n\nAll amounts use {} major units. Regret is best-in-hindsight payoff in each original state minus the action payoff, before changing weights. Each criterion has its own adversary; the two distributions must not be combined.\n\n| Action | Nominal mean | Worst-case mean | Nominal mean regret | Worst-case expected regret |\n|---|---:|---:|---:|---:|\n",
            ratio(self.design.radius),
            md(&self.design.metric_basis),
            md(&self.design.radius_basis),
            md(&self.maximin_mean_winners.join(", ")),
            md(&self.minimax_expected_regret_winners.join(", ")),
            currency_name(self.currency),
        );
        for a in &self.alternatives {
            let _ = writeln!(
                out,
                "| {} | {} | {} | {} | {} |",
                md(&a.action_id),
                ratio(a.nominal_mean),
                ratio(a.worst_case_mean),
                ratio(a.nominal_mean_regret),
                ratio(a.worst_case_expected_regret)
            );
        }
        let _ = writeln!(
            out,
            "\nA nominal expected-value co-winner survives maximin: **{}**; survives minimax expected regret: **{}**. All exact ties are retained.\n\n### Declared metric\n\n| From state | To state | Distance |\n|---|---|---:|",
            self.nominal_winner_survives_maximin, self.nominal_winner_survives_minimax_regret
        );
        for d in &self.design.distances {
            let _ = writeln!(
                out,
                "| {} | {} | {} |",
                md(&d.from_state),
                md(&d.to_state),
                ratio(d.distance)
            );
        }
        for a in &self.alternatives {
            for (kind, c) in [
                ("signed loss (negative payoff)", &a.loss_certificate),
                ("same-state regret", &a.regret_certificate),
            ] {
                let _ = writeln!(
                    out,
                    "\n### Transport witness: {} / {}\n\nMaximum expected {}: **{}**. Transport cost: **{}** of radius **{}**. Dual multiplier: **{}** major currency units per metric unit. Dual objective: **{}**; exact duality gap: **{}**.\n\n| State | Nominal mass | Adversarial mass | Dual potential |\n|---|---:|---:|---:|",
                    md(&a.action_id),
                    kind,
                    kind,
                    ratio(c.worst_case_value),
                    ratio(c.transport_cost),
                    ratio(c.radius),
                    ratio(c.dual_lambda),
                    ratio(c.dual_value),
                    ratio(c.duality_gap)
                );
                for (i, state) in self.state_ids.iter().enumerate() {
                    let _ = writeln!(
                        out,
                        "| {} | {} | {} | {} |",
                        md(state),
                        ratio(self.nominal_weights[i]),
                        ratio(c.destination_weights[i]),
                        ratio(c.dual_potentials[i])
                    );
                }
                out.push_str("\nEvery positive transport arc follows. Unlisted arcs have zero mass.\n\n| From state | To state | Transported mass |\n|---|---|---:|\n");
                for arc in &c.arcs {
                    let _ = writeln!(
                        out,
                        "| {} | {} | {} |",
                        md(&self.state_ids[arc.source]),
                        md(&self.state_ids[arc.target]),
                        ratio(arc.mass)
                    );
                }
            }
        }
        out
    }
}
