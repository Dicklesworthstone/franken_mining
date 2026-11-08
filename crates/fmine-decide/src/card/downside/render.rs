//! Complete downside probabilities, eligibility and probability-valued transport certificates.
//! Determinism: Exact

use super::*;
use crate::card::render::{ids, js, md, ratio, rq};
use std::fmt::Write as _;

fn choice_id(c: &DownsideChoiceId) -> String {
    format!("{{\"kind\":{},\"id\":{}}}", js(c.kind.as_str()), js(&c.id))
}

fn choice_ids(choices: &[DownsideChoiceId]) -> String {
    format!(
        "[{}]",
        choices.iter().map(choice_id).collect::<Vec<_>>().join(",")
    )
}

fn rational_option(value: Option<Rat>) -> String {
    value.map_or_else(|| "null".into(), rq)
}

fn rational_array(values: &[Rat]) -> String {
    format!(
        "[{}]",
        values.iter().map(|v| rq(*v)).collect::<Vec<_>>().join(",")
    )
}

fn money_text(value: Money) -> String {
    let absolute = value.minor.unsigned_abs();
    format!(
        "{}{}.{:04}",
        if value.minor < 0 { "-" } else { "" },
        absolute / Money::SCALE as u128,
        absolute % Money::SCALE as u128
    )
}

impl DownsideLimit {
    #[must_use]
    pub fn to_json(&self) -> String {
        format!(
            "{{\"model_key\":{},\"valuation_basis\":{},\"floor\":{{\"currency\":{},\"minor\":{}}},\"max_breach_probability\":{}}}",
            js(&self.model_key),
            js(&self.valuation_basis),
            js(currency_name(self.floor.ccy)),
            js(&self.floor.minor.to_string()),
            rq(self.max_breach_probability)
        )
    }
}

impl DownsideRanking {
    fn to_json(&self) -> String {
        format!(
            "{{\"status\":{},\"objective\":\"nominal_expected_value\",\"eligible_order\":{},\"winners\":{},\"best_expected_value\":{},\"expected_value_cost_of_limit\":{}}}",
            js(self.status.as_str()),
            choice_ids(&self.eligible_order),
            choice_ids(&self.winners),
            rational_option(self.best_expected_value),
            rational_option(self.expected_value_cost_of_limit)
        )
    }

    fn append_markdown(&self, out: &mut String, title: &str) {
        let _ = writeln!(out, "\n### {title}\n");
        if self.status == DownsideStatus::NoEligibleChoice {
            out.push_str("**No eligible choice.** Every declared choice fails this probability limit; no fallback is selected.\n");
            return;
        }
        let names = |choices: &[DownsideChoiceId]| {
            choices
                .iter()
                .map(|c| format!("{} / {}", c.kind.as_str(), c.id))
                .collect::<Vec<_>>()
                .join(", ")
        };
        let _ = writeln!(
            out,
            "**Eligible order by nominal expected value:** {}. **Co-winners:** {}.\n\nBest eligible nominal value: **{}**. Nominal expected-value cost of this limit: **{}**. All exact ties remain.",
            md(&names(&self.eligible_order)),
            md(&names(&self.winners)),
            self.best_expected_value
                .map_or_else(|| "unavailable".into(), ratio),
            self.expected_value_cost_of_limit
                .map_or_else(|| "unavailable".into(), ratio)
        );
    }
}

impl DownsideSummary {
    #[must_use]
    pub fn to_json(&self) -> String {
        let choices = self.choices.iter().map(|c| format!(
            "{{\"kind\":{},\"id\":{},\"nominal_mean\":{},\"conditional_breach_probabilities\":{},\"nominal_breach_probability\":{},\"nominal_eligible\":{},\"worst_case_breach_probability\":{},\"robust_eligible\":{},\"breach_certificate\":{}}}",
            js(c.choice.kind.as_str()), js(&c.choice.id), rq(c.nominal_mean),
            rational_array(&c.conditional_breach_probabilities), rq(c.nominal_breach_probability),
            c.nominal_eligible, rational_option(c.worst_case_breach_probability),
            c.robust_eligible.map_or_else(|| "null".into(), |v| v.to_string()),
            c.breach_certificate.as_ref().map_or_else(|| "null".into(), robustness::certificate_json)
        )).collect::<Vec<_>>().join(",");
        format!(
            "{{\"limit\":{},\"event\":\"realized_net_payoff_strictly_below_floor\",\"currency\":{},\"monetary_scale\":\"major_currency_units\",\"certificate_objective_unit\":\"probability\",\"state_ids\":{},\"nominal_weights\":{},\"choices\":[{choices}],\"nominal\":{},\"robust\":{},\"scope\":{}}}",
            self.limit.to_json(),
            js(currency_name(self.limit.floor.ccy)),
            ids(&self.state_ids),
            rational_array(&self.nominal_weights),
            self.nominal.to_json(),
            self.robust
                .as_ref()
                .map_or_else(|| "null".into(), DownsideRanking::to_json),
            js(SCOPE)
        )
    }

    #[must_use]
    pub(in crate::card) fn to_markdown(&self) -> String {
        let mut out = format!(
            "\n## Downside probability limit\n\n{SCOPE}\n\n**Breach:** realized net payoff < **{} {}**; equality is admitted. **Maximum probability:** **{}**, inclusive. Both eligible rankings use nominal expected value in {} major units.\n\n| Choice kind | Choice | Nominal mean | Nominal breach probability | Nominal screen | Worst breach probability | Stressed screen |\n|---|---|---:|---:|---|---:|---|\n",
            currency_name(self.limit.floor.ccy),
            money_text(self.limit.floor),
            ratio(self.limit.max_breach_probability),
            currency_name(self.limit.floor.ccy)
        );
        let status = |eligible| if eligible { "eligible" } else { "rejected" };
        for c in &self.choices {
            let _ = writeln!(
                out,
                "| {} | {} | {} | {} | {} | {} | {} |",
                c.choice.kind.as_str(),
                md(&c.choice.id),
                ratio(c.nominal_mean),
                ratio(c.nominal_breach_probability),
                status(c.nominal_eligible),
                c.worst_case_breach_probability
                    .map_or_else(|| "not requested".into(), ratio),
                c.robust_eligible.map_or("not requested", status)
            );
        }
        self.nominal
            .append_markdown(&mut out, "Nominal probability screen");
        if let Some(robust) = &self.robust {
            robust.append_markdown(&mut out, "Stressed probability screen");
        }
        out.push_str("\n### Conditional breach probabilities\n\nPolicy rows integrate the original realized state/signal outcomes; they do not threshold a conditional mean.\n\n| Choice kind | Choice | State | Breach probability within state |\n|---|---|---|---:|\n");
        for c in &self.choices {
            for (state, probability) in self
                .state_ids
                .iter()
                .zip(&c.conditional_breach_probabilities)
            {
                let _ = writeln!(
                    out,
                    "| {} | {} | {} | {} |",
                    c.choice.kind.as_str(),
                    md(&c.choice.id),
                    md(state),
                    ratio(*probability)
                );
            }
        }
        for c in &self.choices {
            if let Some(certificate) = &c.breach_certificate {
                robustness::append_certificate(
                    &mut out,
                    &format!("{} / {}", c.choice.kind.as_str(), c.choice.id),
                    "breach probability",
                    certificate,
                    &self.state_ids,
                    &self.nominal_weights,
                    "probability",
                );
            }
        }
        out
    }
}
