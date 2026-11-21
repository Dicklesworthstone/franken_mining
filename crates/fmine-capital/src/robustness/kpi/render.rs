//! Source-complete KPI comparisons and finite threshold frontiers.
//! Determinism: Exact (all quantities remain rational strings).

use std::fmt::Write as _;

use super::*;
use crate::render::{claim_json, claim_markdown, ids, js, md, ratio, rq};

fn optional(value: Option<Rat>) -> String {
    value.map_or_else(|| "null".into(), rq)
}
fn shown(value: Option<Rat>) -> String {
    value.map_or_else(|| "unavailable".into(), ratio)
}

pub(super) fn design_json(design: &Design) -> String {
    let mut constraints: Vec<_> = design.constraints.iter().collect();
    constraints.sort_by(|a, b| a.id.cmp(&b.id));
    let constraints = constraints.into_iter().map(|c| {
        let mut baseline: Vec<_> = c.baseline.iter().collect();
        baseline.sort_by(|a, b| a.state_id.cmp(&b.state_id));
        let baseline = baseline.into_iter().map(|r| format!(
            "{{\"state_id\":{},\"numerator\":{},\"denominator\":{}}}", js(&r.state_id), rq(r.numerator), rq(r.denominator)
        )).collect::<Vec<_>>().join(",");
        let mut increments: Vec<_> = c.increments.iter().collect();
        increments.sort_by(|a, b| (&a.candidate_id, &a.state_id).cmp(&(&b.candidate_id, &b.state_id)));
        let increments = increments.into_iter().map(|r| format!(
            "{{\"candidate_id\":{},\"state_id\":{},\"numerator\":{},\"denominator\":{}}}", js(&r.candidate_id), js(&r.state_id), rq(r.numerator), rq(r.denominator)
        )).collect::<Vec<_>>().join(",");
        format!("{{\"id\":{},\"metric\":{},\"unit\":{},\"basis\":{},\"period\":{},\"aggregation\":{},\"direction\":{},\"limit\":{},\"baseline\":[{baseline}],\"increments\":[{increments}]}}",
            js(&c.id), js(c.metric.as_str()), js(c.metric.unit()), js(&c.basis), js(&c.period), js(c.aggregation.as_str()), js(c.direction.as_str()), rq(c.limit))
    }).collect::<Vec<_>>().join(",");
    format!(
        "{{\"model_key\":{},\"constraints\":[{constraints}]}}",
        js(&design.model_key)
    )
}

pub(super) fn summary_json(s: &Summary) -> String {
    let choices = s.choices.iter().map(|c| {
        let metrics = c.metrics.iter().map(|m| {
            let states = m.states.iter().map(|r| format!(
                "{{\"state_id\":{},\"numerator\":{},\"denominator\":{},\"ratio\":{}}}", js(&r.state_id), rq(r.numerator), rq(r.denominator), optional(r.ratio)
            )).collect::<Vec<_>>().join(",");
            format!("{{\"constraint_id\":{},\"states\":[{states}],\"threshold_value\":{},\"passes\":{}}}", js(&m.constraint_id), optional(m.threshold_value), m.passes)
        }).collect::<Vec<_>>().join(",");
        format!("{{\"bundle_id\":{},\"base_eligible\":{},\"objective_mean\":{},\"metrics\":[{metrics}],\"kpi_rejections\":{},\"feasible\":{}}}",
            js(&c.bundle_id), c.base_eligible, optional(c.objective_mean), ids(&c.kpi_rejections), c.feasible)
    }).collect::<Vec<_>>().join(",");
    let frontiers = s.frontiers.iter().map(|f| {
        let points = f.points.iter().map(|p| format!(
            "{{\"limit\":{},\"at_declared_limit\":{},\"feasible_count\":{},\"winners\":{},\"objective_mean\":{},\"opportunity_cost\":{}}}",
            rq(p.limit), p.at_declared_limit, p.feasible_count, ids(&p.winners), optional(p.objective_mean), optional(p.opportunity_cost)
        )).collect::<Vec<_>>().join(",");
        format!("{{\"constraint_id\":{},\"points\":[{points}]}}", js(&f.constraint_id))
    }).collect::<Vec<_>>().join(",");
    format!(
        "{{\"model_key\":{},\"objective_unit\":{},\"baseline_winners\":{},\"baseline_objective\":{},\"constrained_winners\":{},\"constrained_objective\":{},\"opportunity_cost\":{},\"choices\":[{choices}],\"frontiers\":[{frontiers}]}}",
        js(&s.model_key),
        js(&s.objective_unit),
        ids(&s.baseline_winners),
        optional(s.baseline_objective),
        ids(&s.constrained_winners),
        optional(s.constrained_objective),
        optional(s.opportunity_cost)
    )
}

impl EvaluatedKpiProgram {
    pub fn to_json(&self) -> String {
        let summary = summary_json(self.summary());
        format!(
            "{{\"schema\":\"fmine.capital-kpi.v1\",\"digest\":{},\"summary\":{summary},\"claim\":{},\"design\":{},\"static_program\":{},\"no_claim\":{},\"reserve_disclosure_claim\":false,\"execution_authorized\":false,\"geotechnical_signoff\":false,\"regulatory_compliance_claim\":false}}",
            js(self.digest()),
            claim_json(self.claim(), &summary),
            claim_json(self.design(), &design_json(&self.design().value)),
            self.static_program().to_json(),
            js(NO_CLAIM)
        )
    }

    pub fn to_markdown(&self) -> String {
        let s = self.summary();
        let mut out = format!(
            "# KPI-constrained capital comparison\n\n{NO_CLAIM}\n\nPrimary objective unit: **{}**.\n\n",
            s.objective_unit
        );
        let _ = writeln!(
            out,
            "| Comparison | All exact co-winners | Expected objective |\n|---|---|---:|\n| Original structural and risk limits | {} | {} |\n| All declared KPI limits | {} | {} |\n\nExpected opportunity cost of all KPI limits: **{} {}**.\n",
            md(&s.baseline_winners.join(", ")),
            shown(s.baseline_objective),
            md(&s.constrained_winners.join(", ")),
            shown(s.constrained_objective),
            shown(s.opportunity_cost),
            s.objective_unit
        );
        out.push_str("An empty winner set means no feasible program; its cost remains unavailable. The threshold frontiers hold every other KPI limit fixed. They report finite economic differences, with no LP dual or derivative interpretation.\n");
        claim_markdown(&mut out, self.claim());
        out.push_str("\n## Declared KPI definitions\n\n");
        claim_markdown(&mut out, self.design());
        for c in &self.design().value.constraints {
            let _ = writeln!(
                out,
                "\n### {}\n\nMetric: `{}` in `{}`; period `{}`; aggregation `{}`; inclusive `{}` limit **{}**.\n\nBasis: {}.\n\n| Baseline state | Numerator | Denominator |\n|---|---:|---:|",
                md(&c.id),
                c.metric.as_str(),
                c.metric.unit(),
                md(&c.period),
                c.aggregation.as_str(),
                c.direction.as_str(),
                ratio(c.limit),
                md(&c.basis)
            );
            for r in &c.baseline {
                let _ = writeln!(
                    out,
                    "| {} | {} | {} |",
                    md(&r.state_id),
                    ratio(r.numerator),
                    ratio(r.denominator)
                );
            }
            out.push_str("\n| Candidate | State | Numerator increment | Denominator increment |\n|---|---|---:|---:|\n");
            for r in &c.increments {
                let _ = writeln!(
                    out,
                    "| {} | {} | {} | {} |",
                    md(&r.candidate_id),
                    md(&r.state_id),
                    ratio(r.numerator),
                    ratio(r.denominator)
                );
            }
        }
        out.push_str("\n## Complete portfolio KPI results\n\n| Bundle | Original eligibility | KPI | Threshold value | KPI passes | All limits feasible |\n|---|---|---|---:|---|---|\n");
        for c in &s.choices {
            for m in &c.metrics {
                let _ = writeln!(
                    out,
                    "| {} | {} | {} | {} | {} | {} |",
                    md(&c.bundle_id),
                    c.base_eligible,
                    md(&m.constraint_id),
                    shown(m.threshold_value),
                    m.passes,
                    c.feasible
                );
            }
        }
        out.push_str("\n### Whole-bundle state components\n\n| Bundle | KPI | State | Numerator | Denominator | Ratio |\n|---|---|---|---:|---:|---:|\n");
        for c in &s.choices {
            for m in &c.metrics {
                for r in &m.states {
                    let _ = writeln!(
                        out,
                        "| {} | {} | {} | {} | {} | {} |",
                        md(&c.bundle_id),
                        md(&m.constraint_id),
                        md(&r.state_id),
                        ratio(r.numerator),
                        ratio(r.denominator),
                        shown(r.ratio)
                    );
                }
            }
        }
        out.push_str("\n## Threshold opportunity-cost frontiers\n\nRows run from tighter to looser inclusive limits. Between adjacent thresholds the feasible set is unchanged. Beyond the tightest attainable threshold no bundle is feasible; beyond the loosest all bundles satisfying the other limits are admitted.\n");
        for f in &s.frontiers {
            let _ = writeln!(
                out,
                "\n### {}\n\n| Limit | Declared limit | Feasible count | All exact co-winners | Expected objective | Opportunity cost |\n|---:|---|---:|---|---:|---:|",
                md(&f.constraint_id)
            );
            for p in &f.points {
                let _ = writeln!(
                    out,
                    "| {} | {} | {} | {} | {} | {} |",
                    ratio(p.limit),
                    p.at_declared_limit,
                    p.feasible_count,
                    md(&p.winners.join(", ")),
                    shown(p.objective_mean),
                    shown(p.opportunity_cost)
                );
            }
        }
        let _ = writeln!(
            out,
            "\nDigest: `{}`. All authority flags remain false.\n\n## Original capital comparison\n\n{}",
            self.digest(),
            self.static_program().to_markdown()
        );
        out
    }
}
