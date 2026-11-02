//! Canonical exact decision output and source-complete draft rendering.
//! Determinism: Exact (stored numeric metadata uses its exact bit representation).

use std::fmt::Write as _;

use fmine_claim::Claim;
use fmine_core::codec::Encoder;
use fmine_core::hash::sha256;
use fmine_evidence::{AoaStatus, AssertionSource, Color, EvidenceClass};

use super::*;

mod sensitivity;

fn js(value: &str) -> String {
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

fn ratio(value: Rat) -> String {
    format!("{}/{}", value.num(), value.den())
}
fn rq(value: Rat) -> String {
    js(&ratio(value))
}
fn ids(values: &[String]) -> String {
    format!(
        "[{}]",
        values.iter().map(|v| js(v)).collect::<Vec<_>>().join(",")
    )
}
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn evidence_name(value: EvidenceClass) -> &'static str {
    match value {
        EvidenceClass::Measured => "measured",
        EvidenceClass::Derived => "derived",
        EvidenceClass::ModelledCalibrated => "modelled_calibrated",
        EvidenceClass::ModelledUncalibrated => "modelled_uncalibrated",
        EvidenceClass::Elicited => "elicited",
        EvidenceClass::Assumed => "assumed",
        EvidenceClass::Transferred {
            aoa: AoaStatus::Inside,
        } => "transferred_inside",
        EvidenceClass::Transferred {
            aoa: AoaStatus::Outside,
        } => "transferred_outside",
        EvidenceClass::Transferred {
            aoa: AoaStatus::NotAssessed,
        } => "transferred_not_assessed",
        EvidenceClass::Assertion {
            source: AssertionSource::Agent,
        } => "assertion_agent",
        EvidenceClass::Assertion {
            source: AssertionSource::Extraction,
        } => "assertion_extraction",
        EvidenceClass::Assertion {
            source: AssertionSource::Issuer,
        } => "assertion_issuer",
        EvidenceClass::Assertion {
            source: AssertionSource::Secondary,
        } => "assertion_secondary",
    }
}

fn color_json(color: Color) -> String {
    match color {
        Color::Verified { lo, hi } => format!(
            "{{\"name\":\"verified\",\"lo_bits\":\"{:016x}\",\"hi_bits\":\"{:016x}\"}}",
            lo.to_bits(),
            hi.to_bits()
        ),
        Color::Validated { regime, dataset } => {
            let bounds = regime
                .bounds()
                .iter()
                .map(|(axis, (lo, hi))| {
                    format!(
                        "{{\"axis\":{},\"lo_bits\":\"{:016x}\",\"hi_bits\":\"{:016x}\"}}",
                        js(axis),
                        lo.to_bits(),
                        hi.to_bits()
                    )
                })
                .collect::<Vec<_>>()
                .join(",");
            format!(
                "{{\"name\":\"validated\",\"dataset\":{},\"regime\":[{bounds}]}}",
                js(&dataset)
            )
        }
        Color::Estimated {
            estimator,
            dispersion,
        } => format!(
            "{{\"name\":\"estimated\",\"estimator\":{},\"dispersion_bits\":\"{:016x}\"}}",
            js(&estimator),
            dispersion.to_bits()
        ),
    }
}

/// Includes metadata omitted by generic Claim identity: support, representation, UU, current flags,
/// supersession and the computed colour. The actual typed value is separately emitted by each caller.
fn claim_json<T>(claim: &Claim<T>, value: &str) -> String {
    let anchors = claim
        .anchors
        .iter()
        .map(|a| {
            format!(
                "{{\"artifact_path\":{},\"sha256\":{},\"locator\":{}}}",
                js(&a.artifact_path),
                js(&hex(&a.artifact_sha256)),
                js(&a.locator)
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let uu = claim.uu_terms.iter().map(|u| format!("{{\"scenario_id\":{},\"description\":{},\"downside_exposure_usd_m\":{},\"downside_bits\":\"{:016x}\",\"trigger_indicator\":{}}}", js(&u.scenario_id), js(&u.description), u.downside_exposure_usd_m, u.downside_exposure_usd_m.to_bits(), js(&u.trigger_indicator))).collect::<Vec<_>>().join(",");
    let color = match claim.color() {
        Ok(color) => color_json(color),
        Err(error) => format!("{{\"refusal\":{}}}", js(&error.what)),
    };
    let additivity = match claim.additivity {
        AdditivityClass::ExtensiveAdditive => "extensive_additive",
        AdditivityClass::IntensiveWeighted => "intensive_weighted",
        AdditivityClass::NonAdditive => "non_additive",
    };
    format!(
        "{{\"id\":{},\"metric_key\":{},\"value_tag\":{},\"value\":{value},\"representation\":\"deterministic\",\"support\":\"period_aggregate\",\"additivity\":{},\"evidence\":{},\"evidence_tag\":{},\"epistemic_tag\":{},\"state\":{},\"state_tag\":{},\"confidentiality\":{},\"uu_terms\":[{uu}],\"parents\":{},\"anchors\":[{anchors}],\"supersedes\":{},\"stale\":{},\"calibration_failed\":{},\"color\":{color}}}",
        js(&claim.claim_id.to_string()),
        js(&claim.metric_key),
        js(claim.value_tag()),
        js(additivity),
        js(evidence_name(claim.evidence)),
        js(&hex(&claim.evidence.tag())),
        js(&hex(&claim.epistemic.tag())),
        js(if claim.state == KnowledgeState::Known {
            "known"
        } else {
            "estimated"
        }),
        claim.state.tag(),
        js(&claim.confidentiality.to_string()),
        ids(&claim
            .parents
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()),
        claim
            .supersedes
            .map_or_else(|| "null".into(), |id| js(&id.to_string())),
        claim.stale,
        claim.calibration_failed
    )
}

fn choice_json(c: &information::ExpectedChoice) -> String {
    format!(
        "{{\"kind\":{},\"id\":{},\"expected_value\":{}}}",
        js(c.kind.as_str()),
        js(&c.id),
        rq(c.expected_value.major)
    )
}

fn signal_json(s: &information::SignalPolicy) -> String {
    let weights = s.posterior_state_weights.as_ref().map_or_else(
        || "null".into(),
        |values| {
            format!(
                "[{}]",
                values
                    .iter()
                    .map(|(state, p)| format!(
                        "{{\"state_id\":{},\"probability\":{}}}",
                        js(state),
                        rq(*p)
                    ))
                    .collect::<Vec<_>>()
                    .join(",")
            )
        },
    );
    let actions = s.posterior_action_values.as_ref().map_or_else(
        || "null".into(),
        |values| {
            format!(
                "[{}]",
                values
                    .iter()
                    .map(|a| format!(
                        "{{\"action_id\":{},\"value\":{}}}",
                        js(&a.action_id),
                        rq(a.value.major)
                    ))
                    .collect::<Vec<_>>()
                    .join(",")
            )
        },
    );
    format!(
        "{{\"signal_id\":{},\"probability\":{},\"reachable\":{},\"posterior_state_weights\":{weights},\"posterior_action_values\":{actions},\"best_actions\":{},\"selected_action\":{},\"conditional_value\":{},\"strict_primary_change\":{}}}",
        js(&s.signal_id),
        rq(s.probability),
        s.probability != ZERO,
        ids(&s.best_actions),
        s.selected_action
            .as_ref()
            .map_or_else(|| "null".into(), |id| js(id)),
        s.conditional_value
            .map_or_else(|| "null".into(), |v| rq(v.major)),
        s.strict_primary_change
            .map_or_else(|| "null".into(), |v| v.to_string())
    )
}

fn option_stats_json(o: &information::InformationStats) -> String {
    let signals = o
        .signals
        .iter()
        .map(signal_json)
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"option_id\":{},\"label\":{},\"test_cost\":{},\"delay_cost\":{},\"total_cost\":{},\"delay_days\":{},\"gross_value\":{},\"net_value\":{},\"evsi\":{},\"net_voi\":{},\"status\":{},\"strict_change_probability\":{},\"baseline_optimal_after_every_reachable_signal\":{},\"signals\":[{signals}]}}",
        js(&o.option_id),
        js(&o.label),
        rq(o.test_cost.major),
        rq(o.delay_cost.major),
        rq(o.total_cost.major),
        o.delay_days,
        rq(o.gross_value.major),
        rq(o.net_value.major),
        rq(o.evsi.major),
        rq(o.net_voi.major),
        js(o.status.as_str()),
        rq(o.strict_change_probability),
        o.baseline_optimal_after_every_reachable_signal
    )
}

fn information_json(i: &InformationSummary) -> String {
    let options = i
        .options
        .iter()
        .map(option_stats_json)
        .collect::<Vec<_>>()
        .join(",");
    let order = i
        .overall_order
        .iter()
        .map(choice_json)
        .collect::<Vec<_>>()
        .join(",");
    let winners = i
        .overall_winners
        .iter()
        .map(choice_json)
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"method\":{},\"monte_carlo_se\":null,\"standard_error_note\":{},\"baseline_primary\":{},\"baseline_winners\":{},\"baseline_value\":{},\"perfect_information_value\":{},\"evpi\":{},\"options\":[{options}],\"option_order\":{},\"option_winners\":{},\"overall_order\":[{order}],\"overall_winners\":[{winners}],\"scope\":{}}}",
        js(information::METHOD),
        js(information::STANDARD_ERROR_NOTE),
        js(&i.baseline_primary),
        ids(&i.baseline_winners),
        rq(i.baseline_value.major),
        rq(i.perfect_information_value.major),
        rq(i.evpi.major),
        ids(&i.option_order),
        ids(&i.option_winners),
        js(information::SCOPE)
    )
}

fn money_json(m: Money) -> String {
    format!(
        "{{\"currency\":{},\"minor\":{}}}",
        js(currency_name(m.ccy)),
        js(&m.minor.to_string())
    )
}

fn information_input_json(o: &InformationOption) -> String {
    let likelihoods = o
        .likelihoods
        .iter()
        .map(|l| {
            format!(
                "{{\"state_id\":{},\"signal_id\":{},\"probability\":{}}}",
                js(&l.state_id),
                js(&l.signal_id),
                rq(l.probability)
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"model_key\":{},\"option_id\":{},\"label\":{},\"currency\":{},\"valuation_basis\":{},\"test_cost\":{},\"delay_cost\":{},\"delay_days\":{},\"signals\":{},\"likelihoods\":[{likelihoods}]}}",
        js(&o.model_key),
        js(&o.option_id),
        js(&o.label),
        js(currency_name(o.currency)),
        js(&o.valuation_basis),
        money_json(o.test_cost),
        money_json(o.delay_cost),
        o.delay_days,
        ids(&o.signals)
    )
}

pub(super) fn summary_json(s: &DecisionSummary) -> String {
    let alternatives = s.alternatives.iter().map(|a| {
        let states = a.states.iter().map(|r| format!("{{\"state_id\":{},\"weight\":{},\"payoff\":{},\"best_in_hindsight\":{},\"regret\":{}}}", js(&r.state_id), rq(r.weight), rq(r.payoff.major), rq(r.best_in_hindsight.major), rq(r.regret.major))).collect::<Vec<_>>().join(",");
        format!("{{\"action_id\":{},\"label\":{},\"role\":{},\"mean\":{},\"p10_p50_p90\":[{},{},{}],\"cvar95_loss\":{},\"mean_regret\":{},\"max_regret\":{},\"states\":[{states}]}}", js(&a.action_id), js(&a.label), js(a.role.as_str()), rq(a.mean.major), rq(a.p10_p50_p90[0].major), rq(a.p10_p50_p90[1].major), rq(a.p10_p50_p90[2].major), rq(a.cvar95_loss.major), rq(a.mean_regret.major), rq(a.max_regret.major))
    }).collect::<Vec<_>>().join(",");
    format!(
        "{{\"model_key\":{},\"question\":{},\"currency\":{},\"valuation_basis\":{},\"monetary_scale\":\"major_currency_units\",\"expected_value_order\":{},\"expected_value_winners\":{},\"expected_value_primary\":{},\"runner_up\":{},\"minimax_regret_order\":{},\"minimax_regret_winners\":{},\"minimax_regret_primary\":{},\"alternatives\":[{alternatives}],\"information\":{},\"sensitivity\":{}}}",
        js(&s.model_key),
        js(&s.question),
        js(currency_name(s.currency)),
        js(&s.valuation_basis),
        ids(&s.expected_value_order),
        ids(&s.expected_value_winners),
        js(&s.expected_value_primary),
        js(&s.runner_up),
        ids(&s.minimax_regret_order),
        ids(&s.minimax_regret_winners),
        js(&s.minimax_regret_primary),
        s.information
            .as_ref()
            .map_or_else(|| "null".into(), information_json),
        sensitivity::json(s)
    )
}

fn weights_json(w: &WeightSet) -> String {
    let states = w
        .states
        .iter()
        .map(|(id, weight)| format!("{{\"state_id\":{},\"weight\":{}}}", js(id), rq(*weight)))
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"model_key\":{},\"question\":{},\"states\":[{states}]}}",
        js(&w.model_key),
        js(&w.question)
    )
}

fn action_json(a: &ActionPayoffs) -> String {
    let values = a
        .values
        .iter()
        .map(|(id, value)| {
            format!(
                "{{\"state_id\":{},\"currency\":{},\"minor\":{}}}",
                js(id),
                js(currency_name(value.ccy)),
                js(&value.minor.to_string())
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"model_key\":{},\"action_id\":{},\"label\":{},\"role\":{},\"currency\":{},\"valuation_basis\":{},\"money_scale\":10000,\"values\":[{values}]}}",
        js(&a.model_key),
        js(&a.action_id),
        js(&a.label),
        js(a.role.as_str()),
        js(currency_name(a.currency)),
        js(&a.valuation_basis)
    )
}

fn color_name(rank: ColorRank) -> &'static str {
    match rank {
        ColorRank::Estimated => "estimated",
        ColorRank::Validated => "validated",
        ColorRank::Verified => "verified",
    }
}

pub(super) fn inputs_json(inputs: &DecisionInputs) -> String {
    let actions = inputs
        .actions
        .iter()
        .map(|a| claim_json(a, &action_json(&a.value)))
        .collect::<Vec<_>>()
        .join(",");
    let options = inputs
        .information_options
        .iter()
        .map(|o| claim_json(o, &information_input_json(&o.value)))
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"weights\":{},\"actions\":[{actions}],\"information_options\":[{options}]}}",
        claim_json(&inputs.weights, &weights_json(&inputs.weights.value))
    )
}

fn body_digest(body: &str) -> String {
    let mut enc = Encoder::new_frame(*b"DCDR", 1);
    enc.put_str(body);
    sha256(&enc.finish()).to_hex()
}

impl EvaluatedCard {
    fn body(&self) -> String {
        let summary = summary_json(self.summary());
        format!(
            "\"schema\":\"fmine.decision-card.v1\",\"mode\":\"finite_scenario_draft\",\"summary\":{summary},\"output_claim\":{},\"weakest_input_color\":{},\"input_claims\":{},\"tie_rule\":{},\"no_claim\":{},\"flags\":{{\"reserve_disclosure_claim\":false,\"execution_authorized\":false,\"geotechnical_signoff\":false,\"regulatory_compliance_claim\":false}}",
            claim_json(&self.claim, &summary),
            js(color_name(self.weakest_input_color)),
            inputs_json(&self.inputs),
            js(TIE_RULE),
            js(NO_CLAIM)
        )
    }

    /// Domain-separated digest binds all input values and metadata, the rules and the complete results.
    #[must_use]
    pub fn digest(&self) -> String {
        body_digest(&self.body())
    }

    #[must_use]
    pub fn to_json(&self) -> String {
        let body = self.body();
        format!("{{{body},\"digest\":{}}}", js(&body_digest(&body)))
    }

    #[must_use]
    pub fn to_markdown(&self) -> String {
        let s = self.summary();
        let mut out = format!(
            "# Draft decision comparison: {}\n\n{NO_CLAIM}\n\n**Model:** {}. **Currency:** {} in major units. **Valuation basis:** {}.\n\n**Evidence:** {}; **output colour:** estimated (no calibrated output is asserted); weakest input colour: {}.\n\n**Act-now expected-value co-winners:** {}. **Displayed primary / runner-up:** {} / {}.\n\n**Act-now minimax-regret co-winners:** {}.\n\n{TIE_RULE}\n\n",
            md(&s.question),
            md(&s.model_key),
            currency_name(s.currency),
            md(&s.valuation_basis),
            evidence_name(self.claim.evidence),
            color_name(self.weakest_input_color),
            md(&s.expected_value_winners.join(", ")),
            md(&s.expected_value_primary),
            md(&s.runner_up),
            md(&s.minimax_regret_winners.join(", "))
        );
        out.push_str("All amounts below are exact rational major currency units. Quantiles use the weighted inverse CDF. CVaR95 integrates the largest 5% of **signed loss = −NPV**, so it may be negative when all outcomes gain.\n\n| Action | Role | Mean | P10 / P50 / P90 | CVaR95 loss | Mean regret | Maximum regret |\n|---|---|---:|---|---:|---:|---:|\n");
        for a in &s.alternatives {
            let _ = writeln!(
                out,
                "| {} ({}) | {} | {} | {} / {} / {} | {} | {} | {} |",
                md(&a.label),
                md(&a.action_id),
                a.role.as_str(),
                ratio(a.mean.major),
                ratio(a.p10_p50_p90[0].major),
                ratio(a.p10_p50_p90[1].major),
                ratio(a.p10_p50_p90[2].major),
                ratio(a.cvar95_loss.major),
                ratio(a.mean_regret.major),
                ratio(a.max_regret.major)
            );
        }
        out.push_str(&sensitivity::markdown(s));
        out.push_str("\n## Same-state payoff and regret\n\n| Action | State | Weight | Payoff | Best in hindsight | Regret |\n|---|---|---:|---:|---:|---:|\n");
        for a in &s.alternatives {
            for r in &a.states {
                let _ = writeln!(
                    out,
                    "| {} | {} | {} | {} | {} | {} |",
                    md(&a.action_id),
                    md(&r.state_id),
                    ratio(r.weight),
                    ratio(r.payoff.major),
                    ratio(r.best_in_hindsight.major),
                    ratio(r.regret.major)
                );
            }
        }
        if let Some(information) = &s.information {
            information_markdown(&mut out, information, &self.inputs.information_options);
        }
        out.push_str("\n## Declared unknown unknowns\n\n");
        for u in &self.claim.uu_terms {
            let _ = writeln!(
                out,
                "- {}: {}; declared downside {} USD million; trigger {}.",
                md(&u.scenario_id),
                md(&u.description),
                u.downside_exposure_usd_m,
                md(&u.trigger_indicator)
            );
        }
        out.push_str("\n## Complete input Claims and citations\n\n");
        source_markdown(&mut out, &self.inputs.weights);
        for a in &self.inputs.actions {
            source_markdown(&mut out, a);
        }
        for option in &self.inputs.information_options {
            source_markdown(&mut out, option);
        }
        let _ = write!(
            out,
            "\nOutput Claim: `{}`. Confidentiality: {}.\n\nDigest: `{}`\n",
            self.claim.claim_id,
            md(&self.claim.confidentiality.to_string()),
            self.digest()
        );
        out
    }
}

fn information_markdown(
    out: &mut String,
    i: &InformationSummary,
    inputs: &[Claim<InformationOption>],
) {
    let _ = write!(
        out,
        "\n## Value of information in the declared finite model\n\n**Method:** {}. **Monte Carlo standard error:** not applicable. {}\n\n{}\n\n**Act-now baseline:** {} (co-winners: {}), exact value {}. **Perfect-information value:** {}. **EVPI:** {}.\n\nStrict change is measured relative to the fixed displayed baseline action {}; a posterior tie retaining that action is not a strict change.\n\n",
        information::METHOD,
        information::STANDARD_ERROR_NOTE,
        information::SCOPE,
        md(&i.baseline_primary),
        md(&i.baseline_winners.join(", ")),
        ratio(i.baseline_value.major),
        ratio(i.perfect_information_value.major),
        ratio(i.evpi.major),
        md(&i.baseline_primary)
    );
    out.push_str("| Option | Test cost | Delay cost | Delay days | Gross policy value | EVSI | Net policy value | Net VOI | Net status | Strict change probability |\n|---|---:|---:|---:|---:|---:|---:|---:|---|---:|\n");
    for o in &i.options {
        let _ = writeln!(
            out,
            "| {} ({}) | {} | {} | {} | {} | {} | {} | {} | {} | {} |",
            md(&o.label),
            md(&o.option_id),
            ratio(o.test_cost.major),
            ratio(o.delay_cost.major),
            o.delay_days,
            ratio(o.gross_value.major),
            ratio(o.evsi.major),
            ratio(o.net_value.major),
            ratio(o.net_voi.major),
            o.status.as_str(),
            ratio(o.strict_change_probability)
        );
    }
    let choices = |values: &[information::ExpectedChoice]| {
        values
            .iter()
            .map(|c| {
                format!(
                    "{} {} ({})",
                    c.kind.as_str(),
                    md(&c.id),
                    ratio(c.expected_value.major)
                )
            })
            .collect::<Vec<_>>()
            .join(", ")
    };
    let _ = write!(
        out,
        "\n**Information-option co-winners by net expected value:** {}.\n\n**Overall expected-value co-winners:** {}.\n\n**Overall expected-value order:** {}.\n\nEqual overall values retain all co-winners; act-now choices precede information options for presentation only. This expected-value comparison buys at most one option and supplies no test-policy risk or regret ranking.\n",
        md(&i.option_winners.join(", ")),
        choices(&i.overall_winners),
        choices(&i.overall_order)
    );
    for o in &i.options {
        if o.baseline_optimal_after_every_reachable_signal {
            let _ = writeln!(
                out,
                "\nFor option {}, the act-now baseline remains optimal after every reachable result; gross EVSI is zero.",
                md(&o.option_id)
            );
        }
        let _ = write!(
            out,
            "\n### Conditional policy: {}\n\n| Signal | Probability | Posterior state weights | Conditional action values | Co-winners | Displayed action | Conditional best value | Strict primary change |\n|---|---:|---|---|---|---|---:|---|\n",
            md(&o.option_id)
        );
        for s in &o.signals {
            let weights = s.posterior_state_weights.as_ref().map_or_else(
                || "unreachable".into(),
                |values| {
                    values
                        .iter()
                        .map(|(id, p)| format!("{}: {}", md(id), ratio(*p)))
                        .collect::<Vec<_>>()
                        .join("; ")
                },
            );
            let values = s.posterior_action_values.as_ref().map_or_else(
                || "not applicable".into(),
                |values| {
                    values
                        .iter()
                        .map(|a| format!("{}: {}", md(&a.action_id), ratio(a.value.major)))
                        .collect::<Vec<_>>()
                        .join("; ")
                },
            );
            let _ = writeln!(
                out,
                "| {} | {} | {} | {} | {} | {} | {} | {} |",
                md(&s.signal_id),
                ratio(s.probability),
                weights,
                values,
                if s.best_actions.is_empty() {
                    "none".into()
                } else {
                    md(&s.best_actions.join(", "))
                },
                s.selected_action
                    .as_ref()
                    .map_or_else(|| "not applicable".into(), |id| md(id)),
                s.conditional_value
                    .map_or_else(|| "not applicable".into(), |v| ratio(v.major)),
                s.strict_primary_change
                    .map_or_else(|| "not applicable".into(), |v| v.to_string())
            );
        }
    }
    out.push_str("\n### Complete declared likelihood matrices\n\nEach entry is P(signal | state), not a posterior state probability.\n\n| Option | State | Signal | Declared conditional probability |\n|---|---|---|---:|\n");
    for input in inputs {
        for l in &input.value.likelihoods {
            let _ = writeln!(
                out,
                "| {} | {} | {} | {} |",
                md(&input.value.option_id),
                md(&l.state_id),
                md(&l.signal_id),
                ratio(l.probability)
            );
        }
    }
}

fn md(s: &str) -> String {
    let mut out = String::new();
    for ch in s.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '|' => out.push_str("&#124;"),
            '`' => out.push_str("&#96;"),
            '[' => out.push_str("&#91;"),
            ']' => out.push_str("&#93;"),
            '\n' | '\r' => out.push(' '),
            '\\' | '*' | '_' | '#' => {
                out.push('\\');
                out.push(ch);
            }
            _ => out.push(ch),
        }
    }
    out
}

fn source_markdown<T>(out: &mut String, claim: &Claim<T>) {
    let _ = writeln!(
        out,
        "- Claim `{}` — {}; evidence {}; confidentiality {}:",
        claim.claim_id,
        md(&claim.metric_key),
        evidence_name(claim.evidence),
        md(&claim.confidentiality.to_string())
    );
    for source in &claim.anchors {
        let _ = writeln!(
            out,
            "  - {} at {}; SHA-256 `{}`.",
            md(&source.artifact_path),
            md(&source.locator),
            hex(&source.artifact_sha256)
        );
    }
}
