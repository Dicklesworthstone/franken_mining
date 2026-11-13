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

fn tail_limit_json(limit: &RobustTailLimit) -> String {
    let c = &limit.certificate;
    format!(
        "{{\"constraint\":{},\"objective_unit\":{},\"limit\":{},\"nominal_tail_loss\":{},\"worst_case_tail_loss\":{},\"passes\":{},\"certificate\":{{\"tail_mass\":{},\"threshold\":{},\"worst_case_cvar\":{},\"tail_masses\":{},\"hinge_certificate\":{},\"dual_value\":{},\"duality_gap\":{},\"oracle_calls\":{}}}}}",
        js(&limit.constraint),
        js(&limit.objective_unit),
        rq(limit.limit),
        rq(limit.nominal_tail_loss),
        rq(limit.worst_case_tail_loss),
        limit.passes,
        rq(c.tail_mass),
        rq(c.threshold),
        rq(c.worst_case_cvar),
        rationals(&c.tail_masses),
        certificate_json(&c.hinge_certificate),
        rq(c.dual_value),
        rq(c.duality_gap),
        c.oracle_calls,
    )
}

impl RobustnessSummary {
    fn labels(
        &self,
    ) -> (
        &'static str,
        &'static str,
        &'static str,
        &'static str,
        &'static str,
        &'static str,
    ) {
        match self.scope {
            RobustnessScope::StaticBundles => (
                "nominally_feasible_static_bundles",
                "all_nominally_feasible_static_bundles",
                "bundle",
                "bundles",
                "bundle_id",
                NO_CLAIM,
            ),
            RobustnessScope::AdaptivePolicies => (
                "nominally_feasible_adaptive_policies",
                "all_nominally_feasible_adaptive_policies",
                "policy",
                "policies",
                "policy_id",
                POLICY_NO_CLAIM,
            ),
        }
    }

    #[must_use]
    pub fn to_json(&self) -> String {
        let (scope, reference, singular, plural, identity, no_claim) = self.labels();
        let bundles = self
            .bundles
            .iter()
            .map(|row| {
                let tail_limits = row.tail_limits.iter().map(tail_limit_json).collect::<Vec<_>>().join(",");
                format!(
                    "{{\"{identity}\":{},\"nominal_mean\":{},\"worst_case_mean\":{},\"nominal_mean_regret\":{},\"worst_case_expected_regret\":{},\"loss_certificate\":{},\"regret_certificate\":{},\"robust_tail_feasible\":{},\"tail_limits\":[{tail_limits}]}}",
                    js(&row.bundle_id),
                    rq(row.nominal_mean),
                    rq(row.worst_case_mean),
                    rq(row.nominal_mean_regret),
                    rq(row.worst_case_expected_regret),
                    certificate_json(&row.loss_certificate),
                    certificate_json(&row.regret_certificate),
                    row.robust_tail_feasible,
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        format!(
            "{{\"scope\":\"{scope}\",\"status\":{},\"objective_unit\":{},\"regret_reference\":\"{reference}\",\"eligibility_basis\":\"original_nominal_structural_and_tail_loss_screens\",\"stressed_tail_loss_feasibility_certified\":{},\"declared_tail_limits\":{},\"robust_tail_feasible\":{},\"robust_tail_rejections\":{},\"robust_tail_maximin_mean_winners\":{},\"robust_tail_minimax_expected_regret_winners\":{},\"design\":{},\"state_ids\":{},\"nominal_weights\":{},\"structural_rejections\":{},\"nominal_risk_rejections\":{},\"nominal_expected_value_winners\":{},\"maximin_mean_winners\":{},\"minimax_expected_regret_winners\":{},\"nominal_winner_survives_maximin\":{},\"nominal_winner_survives_minimax_regret\":{},\"{plural}\":[{bundles}],\"no_claim\":{}}}",
            js(&if self.bundles.is_empty() {
                format!("no_nominally_feasible_{singular}")
            } else {
                "ok".into()
            }),
            js(&self.objective_unit),
            self.declared_tail_limits > 0,
            self.declared_tail_limits,
            ids(&self.robust_tail_feasible),
            ids(&self.robust_tail_rejections),
            ids(&self.robust_tail_maximin_mean_winners),
            ids(&self.robust_tail_minimax_expected_regret_winners),
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
            js(no_claim),
        )
    }

    #[must_use]
    pub fn to_markdown(&self) -> String {
        let (_, _, singular, _, _, no_claim) = self.labels();
        let (heading, column) = match self.scope {
            RobustnessScope::StaticBundles => ("Capital scenario-weight robustness", "Bundle"),
            RobustnessScope::AdaptivePolicies => {
                ("Capital policy scenario-weight robustness", "Policy")
            }
        };
        let names = |values: &[String]| {
            if values.is_empty() {
                "none".into()
            } else {
                md(&values.join(", "))
            }
        };
        let mut out = format!(
            "\n## {heading}\n\n{no_claim}\n\n**Objective unit:** {}. **Stress radius:** {}.\n\n**Metric basis:** {}.\n\n**Radius basis:** {}.\n\n**Nominal expected-value co-winners:** {}. **Maximin mean co-winners:** {}. **Minimax expected-regret co-winners:** {}.\n\n**Structural exclusions:** {}. **Nominal risk exclusions:** {}. Their original complete assessment rows retain the reasons and binding limits. No excluded {singular} enters this stress comparison or its hindsight benchmark.\n\nUnlike the preceding nominal regret columns, regret here compares only the nominally feasible population. Loss certificates maximize the negative primary payoff; regret certificates maximize same-state regret. The adversaries are separate.\n\n| {column} | Nominal mean | Worst-case mean | Nominal expected regret | Worst-case expected regret |\n|---|---:|---:|---:|---:|\n",
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
            let _ = writeln!(
                out,
                "\nNo nominally feasible {singular} exists; no stress winner or fallback is supplied."
            );
        }
        if self.declared_tail_limits == 0 {
            out.push_str("\nNo tail-loss limit was declared; no risk cap is inferred.\n");
        } else {
            let _ = writeln!(
                out,
                "\n### Tail-loss limits under every admissible stressed law\n\n**Robust-tail feasible:** {}. **Robust-tail exclusions:** {}.\n\n**Robust-tail maximin mean co-winners:** {}. **Robust-tail minimax expected-regret co-winners:** {}. Regret retains the original nominally feasible benchmark. Nominal selections remain separate. Equality to a cap is feasible. Each limit has its own maximizing law; a mean-loss adversary is not reused as a tail-risk certificate.\n\n| {column} | Constraint | Unit | Nominal tail loss | Worst tail loss | Limit | Passes |\n|---|---|---|---:|---:|---:|---|",
                names(&self.robust_tail_feasible),
                names(&self.robust_tail_rejections),
                names(&self.robust_tail_maximin_mean_winners),
                names(&self.robust_tail_minimax_expected_regret_winners)
            );
            for row in &self.bundles {
                for limit in &row.tail_limits {
                    let _ = writeln!(
                        out,
                        "| {} | {} | {} | {} | {} | {} | {} |",
                        md(&row.bundle_id),
                        md(&limit.constraint),
                        md(&limit.objective_unit),
                        ratio(limit.nominal_tail_loss),
                        ratio(limit.worst_case_tail_loss),
                        ratio(limit.limit),
                        limit.passes
                    );
                }
            }
            if self.robust_tail_feasible.is_empty() {
                out.push_str("\nNo declared choice satisfies every tail-loss limit throughout this ambiguity set; no fallback is supplied.\n");
            }
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
            for limit in &row.tail_limits {
                let c = &limit.certificate;
                let _ = writeln!(
                    out,
                    "\n### {}: {} tail-risk certificate\n\nTail mass: {}. Threshold: {}. Worst CVaR: {} {}. Global dual bound: {}. Duality gap: {}. Exact transport calls: {}.\n\nThe transport objective below is positive excess above the threshold. Its dual, divided by tail mass and added to the threshold, bounds every admissible law. The retained tail submass attains that bound.\n",
                    md(&row.bundle_id),
                    md(&limit.constraint),
                    ratio(c.tail_mass),
                    ratio(c.threshold),
                    ratio(c.worst_case_cvar),
                    md(&limit.objective_unit),
                    ratio(c.dual_value),
                    ratio(c.duality_gap),
                    c.oracle_calls
                );
                append_certificate(
                    &mut out,
                    &self.state_ids,
                    &self.nominal_weights,
                    &c.hinge_certificate,
                );
                out.push_str("\n| State | Worst-tail submass |\n|---|---:|\n");
                for (state, mass) in self.state_ids.iter().zip(&c.tail_masses) {
                    let _ = writeln!(out, "| {} | {} |", md(state), ratio(*mass));
                }
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
