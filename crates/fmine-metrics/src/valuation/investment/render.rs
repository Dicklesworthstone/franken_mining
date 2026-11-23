//! DESIGN: retained exact cash components, source receipts and existing decision statistics.
//! Determinism: DeterministicPerIsa

use super::*;
use crate::valuation::render::{claim, js, md, monetary, shown};
use std::fmt::Write as _;

fn rat(q: Rat) -> String {
    format!(
        r#"{{"numerator":"{}","denominator":"{}"}}"#,
        q.num(),
        q.den()
    )
}

pub(super) fn spec(s: &InvestmentSpec) -> String {
    let mut periods = s.periods.iter().collect::<Vec<_>>();
    periods.sort_by_key(|p| &p.id);
    let periods = periods
        .iter()
        .map(|p| {
            format!(
                r#"{{"id":{},"start_ns":"{}","end_ns":"{}","discount_factor":{}}}"#,
                js(&p.id),
                p.start_ns,
                p.end_ns,
                rat(p.discount_factor)
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let mut alternatives = s.alternatives.iter().collect::<Vec<_>>();
    alternatives.sort_by_key(|a| &a.id);
    let alternatives = alternatives
        .iter()
        .map(|a| {
            format!(
                r#"{{"id":{},"label":{},"initial_capex_usd":{}}}"#,
                js(&a.id),
                js(&a.label),
                rat(a.initial_capex_usd)
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    // Existing declarations retain their bytes and identity when the optional screen is absent.
    let downside = s.downside_limit.as_ref().map_or_else(String::new, |limit| {
        format!(",\"downside_limit\":{}", limit.to_json())
    });
    let information = s
        .information_basis
        .as_ref()
        .map_or_else(String::new, |basis| {
            let mut options = s.information_options.iter().collect::<Vec<_>>();
            options.sort_by_key(|o| &o.option_id);
            let options = options
                .iter()
                .map(|o| {
                    format!(
                        r#"{{"id":{},"canonical_tag":{}}}"#,
                        js(&o.option_id),
                        js(&o.canonical_tag())
                    )
                })
                .collect::<Vec<_>>()
                .join(",");
            format!(
                r#", "information_basis":{},"information_options":[{}]"#,
                js(basis),
                options
            )
        });
    format!(
        concat!(
            r#"{{"model_key":{},"question":{},"shared_scenario_basis":{},"discount_basis":{},"cashflow_basis":{},"currency":{},"valuation_time_ns":"{}","boundary":{},"circuit":{},"material_domain":{},"dry_basis":{},"element":{},"commercial_support":{},"monetary_rounding":{},"periods":[{}],"alternatives":[{}]{}{}}}"#
        ),
        js(&s.model_key),
        js(&s.question),
        js(&s.shared_scenario_basis),
        js(&s.discount_basis),
        js(&s.cashflow_basis),
        js(&format!("{:?}", s.currency)),
        s.valuation_time_ns,
        js(&s.boundary),
        js(&s.circuit),
        js(&s.material_domain),
        js(&s.dry_basis),
        js(&s.element),
        js(&s.commercial_support),
        js(s.monetary_rounding.as_str()),
        periods,
        alternatives,
        downside,
        information
    )
}

impl InvestmentReceipt {
    pub fn to_json(&self) -> String {
        let flows = self.cashflows.iter().map(|p| format!(r#"{{"alternative_id":{},"period_id":{},"scenario_id":{},"weight":{},"operating_cash_usd":{},"do_nothing_cash_usd":{},"incremental_cash_usd":{},"discounted_incremental_cash_usd":{}}}"#,
            js(&p.alternative_id), js(&p.period_id), js(&p.scenario_id), rat(p.weight), rat(p.operating_cash_usd),
            rat(p.do_nothing_cash_usd), rat(p.incremental_cash_usd), rat(p.discounted_incremental_cash_usd))).collect::<Vec<_>>().join(",");
        let outcomes = self.outcomes.iter().map(|p| format!(r#"{{"alternative_id":{},"scenario_id":{},"weight":{},"discounted_operating_delta_usd":{},"unrounded_value_usd":{},"value_usd":{},"rounding_delta_usd":{}}}"#,
            js(&p.alternative_id), js(&p.scenario_id), rat(p.weight), rat(p.discounted_operating_delta_usd),
            rat(p.unrounded_value_usd), js(&monetary(p.value)), rat(p.rounding_delta_usd))).collect::<Vec<_>>().join(",");
        let bundles = self
            .bundles
            .iter()
            .map(|b| {
                format!(
                    r#"{{"alternative_id":{},"period_id":{},"valuation":{}}}"#,
                    js(&b.alternative_id),
                    js(&b.period_id),
                    b.valuation.to_json()
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        format!(
            r#"{{"schema":"fmine.production-investment.receipt.v1","valuation_basis":{},"declaration":{},"spec":{},"submitted_manifest":{},"cashflows":[{}],"outcomes":[{}],"bundles":[{}],"decision":{},"digest":{},"no_claim":{}}}"#,
            js(VALUATION_BASIS),
            claim(&self.declaration),
            spec(&self.declaration.value),
            self.submitted_manifest
                .as_deref()
                .map_or_else(|| "null".into(), js),
            flows,
            outcomes,
            bundles,
            self.decision.to_json(),
            js(&self.digest),
            js(NO_CLAIM)
        )
    }

    pub fn to_markdown(&self) -> String {
        let s = &self.declaration.value;
        let mut out = format!(
            "# Declared-period investment comparison\n\n{}\n\n{NO_CLAIM}\n\n**Shared scenario assumption:** {}.\n\n**Discount basis:** {}. **Valuation time:** {} ns. **Currency:** USD.\n\n**Boundary / circuit:** {} / {}. **Element:** {}. **Evidence:** {:?}.\n\n## Period assumptions\n\nEach discount factor multiplies that period's unrounded incremental cash directly. Gaps are outside the declared cash window; no rate or annualization is inferred.\n\n| Period | Start ns | End ns | Discount factor |\n|---|---:|---:|---:|\n",
            md(&s.question),
            md(&s.shared_scenario_basis),
            md(&s.discount_basis),
            s.valuation_time_ns,
            md(&s.boundary),
            md(&s.circuit),
            md(&s.element),
            self.decision.claim().evidence
        );
        let mut periods = s.periods.iter().collect::<Vec<_>>();
        periods.sort_by_key(|p| (p.start_ns, p.end_ns, &p.id));
        for p in periods {
            let _ = writeln!(
                out,
                "| {} | {} | {} | {} |",
                md(&p.id),
                p.start_ns,
                p.end_ns,
                shown(p.discount_factor)
            );
        }
        out.push_str("\n## Initial investment\n\nPaid once at valuation time and excluded from all operating worksheets.\n\n| Alternative | Initial capex USD |\n|---|---:|\n");
        let mut alternatives = s.alternatives.iter().collect::<Vec<_>>();
        alternatives.sort_by_key(|a| &a.id);
        for a in alternatives {
            let _ = writeln!(out, "| {} | {} |", md(&a.id), shown(a.initial_capex_usd));
        }
        out.push_str("\n## Exact period cash components\n\nOperating cash minus same-period do-nothing cash, then multiplied by the explicit discount factor.\n\n| Alternative | Period | State | Weight | Operating USD | Baseline USD | Incremental USD | Discounted incremental USD |\n|---|---|---|---:|---:|---:|---:|---:|\n");
        for p in &self.cashflows {
            let _ = writeln!(
                out,
                "| {} | {} | {} | {} | {} | {} | {} | {} |",
                md(&p.alternative_id),
                md(&p.period_id),
                md(&p.scenario_id),
                shown(p.weight),
                shown(p.operating_cash_usd),
                shown(p.do_nothing_cash_usd),
                shown(p.incremental_cash_usd),
                shown(p.discounted_incremental_cash_usd)
            );
        }
        out.push_str("\n## Investment outcomes\n\nInitial capex is subtracted after summing discounted increments. Money rounds once at the final investment boundary; prior operating-receipt rounding is not used. Delta means rounded minus unrounded USD.\n\n| Alternative | State | Discounted operating delta USD | Unrounded value USD | Final value USD | Rounding delta USD |\n|---|---|---:|---:|---:|---:|\n");
        for p in &self.outcomes {
            let _ = writeln!(
                out,
                "| {} | {} | {} | {} | {} | {} |",
                md(&p.alternative_id),
                md(&p.scenario_id),
                shown(p.discounted_operating_delta_usd),
                shown(p.unrounded_value_usd),
                monetary(p.value),
                shown(p.rounding_delta_usd)
            );
        }
        out.push_str("\n## Comparison of complete scenario paths\n\nCVaR95 is the largest 5% of signed loss = minus discounted incremental cash value. Do-nothing is zero incremental value.\n\n| Alternative | Mean USD | P10 / P50 / P90 USD | CVaR95 loss USD | Mean regret USD |\n|---|---:|---:|---:|---:|\n");
        for a in &self.decision.summary().alternatives {
            let _ = writeln!(
                out,
                "| {} | {} | {} / {} / {} | {} | {} |",
                md(&a.action_id),
                shown(a.mean.major),
                shown(a.p10_p50_p90[0].major),
                shown(a.p10_p50_p90[1].major),
                shown(a.p10_p50_p90[2].major),
                shown(a.cvar95_loss.major),
                shown(a.mean_regret.major)
            );
        }
        let _ = writeln!(
            out,
            "\n**Expected-value co-winners:** {}. **Minimax-regret co-winners:** {}.\n\n{}\n",
            md(&self.decision.summary().expected_value_winners.join(", ")),
            md(&self.decision.summary().minimax_regret_winners.join(", ")),
            card::TIE_RULE
        );
        if let Some(information) = self.decision.information_to_markdown() {
            out.push_str("\n## Information before the investment commitment\n\nThe source explicitly assumes that the test result arrives before choosing a whole investment and that every declared investment cash flow remains valid after the test delay. Test cost and delay cost are nonnegative USD present values at the declared valuation time, charged once on every signal branch, including a decision to do nothing. Delay days do not shift operating periods or infer lost cash, discounting or financing.\n");
            out.push_str(&information);
        }
        if let Some(screen) = &self.decision.summary().downside {
            let choice_name = |choice: &card::downside::DownsideChoiceId| {
                if s.information_options.is_empty() {
                    choice.id.clone()
                } else {
                    format!("{}:{}", choice.kind.as_str(), choice.id)
                }
            };
            let _ = writeln!(
                out,
                "\n## Investment downside probability limit\n\n**Breach:** final rounded discounted incremental value after initial capex < **{} USD**; equality is not a breach. **Maximum probability:** **{}**, inclusive. This screens the complete investment horizon, not individual-period cash. Probabilities remain conditional on the declared uncalibrated scenarios; the unconstrained ranking above is unchanged. The limit does not bound loss severity or funding needs.\n\n| Alternative | Breach probability | Eligibility |\n|---|---:|---|",
                monetary(screen.limit.floor),
                shown(screen.limit.max_breach_probability)
            );
            for choice in &screen.choices {
                let _ = writeln!(
                    out,
                    "| {} | {} | {} |",
                    md(&choice_name(&choice.choice)),
                    shown(choice.nominal_breach_probability),
                    if choice.nominal_eligible {
                        "eligible"
                    } else {
                        "rejected"
                    }
                );
            }
            if screen.nominal.winners.is_empty() {
                out.push_str("\n**No eligible choice.** Every declared alternative fails this probability limit; no fallback is selected.\n");
            } else {
                let winners = screen
                    .nominal
                    .winners
                    .iter()
                    .map(choice_name)
                    .collect::<Vec<_>>()
                    .join(", ");
                let _ = writeln!(
                    out,
                    "\n**Downside-eligible expected-value co-winners:** {}. **Expected-value cost of the limit:** {} USD. All exact ties remain.\n",
                    md(&winners),
                    screen
                        .nominal
                        .expected_value_cost_of_limit
                        .map_or_else(|| "unavailable".into(), shown)
                );
            }
        }
        out.push_str("\n## Source ancestry\n");
        let sources = self
            .decision
            .claim()
            .anchors
            .iter()
            .map(|a| (&a.artifact_path, a.artifact_sha256))
            .collect::<BTreeSet<_>>();
        for (path, hash) in sources {
            let _ = writeln!(
                out,
                "- {} — SHA-256 `{}`",
                md(path),
                fmine_core::Digest32(hash).to_hex()
            );
        }
        let _ = writeln!(
            out,
            "\nEvery original bundle and unrounded component remains in JSON. Investment receipt digest: `{}`.",
            self.digest
        );
        out
    }
}
