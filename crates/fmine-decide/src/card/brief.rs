//! DESIGN (BD-054, §9.6). The short view reads the same sealed finite-scenario Claim as the analyst report.
//! Determinism: Exact

use std::fmt::Write as _;

use fmine_numerics::risk::exact::compare;
use fmine_units::Money;

use super::downside::{DownsideLimit, PolicyScreen};
use super::information::{ChoiceKind, InformationStats, PolicyRisk};
use super::render::{md, ratio, short_decimal};
use super::{
    AlternativeStats, EvaluatedCard, ExactMoney, ONE, Rat, Result, ZERO, arithmetic, currency_name,
};

/// The action-specific range and the first uniform extra cost that would tie another act-now choice.
/// A missing challenger means the admitted comparison contained just one feasible action.
pub struct EpistemicOnePager<'a> {
    card: &'a EvaluatedCard,
    primary: &'a AlternativeStats,
    least_regret: &'a AlternativeStats,
    tie: Option<(ExactMoney, Vec<&'a AlternativeStats>)>,
}

fn write_responses<'a>(
    out: &mut String,
    heading: &str,
    option: &str,
    mut responses: impl Iterator<Item = (&'a str, &'a str)>,
) {
    const MAX_SHOWN: usize = 6;
    let rows = responses
        .by_ref()
        .take(MAX_SHOWN)
        .map(|(signal, action)| format!("{} → {}", md(signal), md(action)))
        .collect::<Vec<_>>();
    if !rows.is_empty() {
        let _ = writeln!(
            out,
            "**{heading} for {}.** {}.",
            md(option),
            rows.join(", ")
        );
        let remaining = responses.count();
        if remaining > 0 {
            let _ = writeln!(
                out,
                "See the full DecisionCard for the remaining {remaining}."
            );
        }
        out.push('\n');
    }
}

fn write_fixed_responses(out: &mut String, heading: &str, option: &InformationStats) {
    write_responses(
        out,
        heading,
        &option.option_id,
        option.signals.iter().filter_map(|signal| {
            Some((
                signal.signal_id.as_str(),
                signal.selected_action.as_deref()?,
            ))
        }),
    );
}

struct FeeHeadroom {
    extra_fee: Rat,
    limiting_payoff: Rat,
    breach_at_ceiling: Rat,
    breach_above_ceiling: Rat,
}

fn nominal_fee_headroom(limit: &DownsideLimit, policy: &PolicyRisk) -> Result<Option<FeeHeadroom>> {
    if limit.max_breach_probability == ONE {
        return Ok(None);
    }
    let floor = arithmetic(Rat::new(limit.floor.minor, Money::SCALE))?;
    let mut breached = ZERO;
    // The admitted law has one bin per payoff in ascending order. When a uniform fee
    // moves this bin to the floor, only the earlier bins breach the strict floor.
    for bin in &policy.distribution {
        let next = arithmetic(breached.checked_add(bin.probability))?;
        if compare(next, limit.max_breach_probability).is_gt() {
            let extra_fee = arithmetic(bin.net_payoff.major.checked_sub(floor))?;
            if extra_fee.signum() >= 0 {
                return Ok(Some(FeeHeadroom {
                    extra_fee,
                    limiting_payoff: bin.net_payoff.major,
                    breach_at_ceiling: breached,
                    breach_above_ceiling: next,
                }));
            }
            break;
        }
        breached = next;
    }
    Err(super::refusal(
        fmine_evidence::RefusalCode::NumericalCertificateFailed207,
        "eligible test policy has no nonnegative fee boundary for the declared nominal cap",
    ))
}

fn write_fee_headroom(out: &mut String, option: &str, limit: &DownsideLimit, policy: &PolicyRisk) {
    let option = md(option);
    let currency = currency_name(limit.floor.ccy);
    match nominal_fee_headroom(limit, policy) {
        Ok(Some(boundary)) => {
            let _ = writeln!(
                out,
                "**Nominal downside fee headroom for {option}.** Up to {} {currency} more in test fees with responses and delay cost held fixed. The current {} {currency} outcome reaches the payoff floor at that fee. Breach probability is {} at the ceiling and {} immediately above it. Recheck the economic ranking at the revised fee.\n",
                short_decimal(boundary.extra_fee),
                short_decimal(boundary.limiting_payoff),
                ratio(boundary.breach_at_ceiling),
                ratio(boundary.breach_above_ceiling)
            );
        }
        Ok(None) => {
            let _ = writeln!(
                out,
                "**Nominal downside fee headroom for {option}.** The 100% nominal cap allows any additional fee for this policy. Recheck the economic ranking at the revised fee.\n"
            );
        }
        Err(e) => {
            let _ = writeln!(
                out,
                "**Nominal downside fee headroom for {option}.** Unavailable ({}). {}\n",
                e.code,
                md(&e.what)
            );
        }
    }
}

impl EvaluatedCard {
    pub fn executive_brief(&self) -> Result<EpistemicOnePager<'_>> {
        let s = self.summary();
        let action = |id: &str| {
            s.alternatives
                .iter()
                .find(|a| a.action_id == id)
                .ok_or_else(|| {
                    super::refusal(
                        fmine_evidence::RefusalCode::NumericalCertificateFailed207,
                        "ranked action missing from evaluated comparison",
                    )
                })
        };
        let primary = action(&s.expected_value_primary)?;
        let tie = if let Some(id) = &s.runner_up {
            let next = action(id)?;
            let cost = ExactMoney {
                currency: s.currency,
                major: arithmetic(primary.mean.major.checked_sub(next.mean.major))?,
            };
            let rivals = s
                .alternatives
                .iter()
                .filter(|a| a.action_id != primary.action_id && a.mean == next.mean)
                .collect();
            Some((cost, rivals))
        } else {
            None
        };
        Ok(EpistemicOnePager {
            card: self,
            primary,
            least_regret: action(&s.minimax_regret_primary)?,
            tie,
        })
    }
}

impl EpistemicOnePager<'_> {
    /// The full comparison retains every action and the detailed source rows behind the output Claim.
    #[must_use]
    pub fn to_markdown(&self) -> String {
        let s = self.card.summary();
        let claim = self.card.claim();
        let currency = currency_name(s.currency);
        let money = |m: ExactMoney| {
            let n = m.major.num();
            if m.major.den() == 1 && n != 0 && n % 1_000_000 == 0 {
                format!("{} million {currency}", n / 1_000_000)
            } else {
                format!("{} {currency}", short_decimal(m.major))
            }
        };
        let label = |a: &AlternativeStats| format!("{} ({})", md(&a.label), md(&a.action_id));
        let names = |ids: &[String]| {
            s.alternatives
                .iter()
                .filter(|a| ids.contains(&a.action_id))
                .map(label)
                .collect::<Vec<_>>()
                .join(", ")
        };
        let a = self.primary;
        let mut out = format!("# {}\n\n{}\n\n", md(&s.question), md(&s.valuation_basis));
        if let Some(risk) = &s.downside {
            out.push_str(&risk.decision_brief());
            if let Some(search) = &risk.policy_optimization {
                let heading = match search.configuration.screen {
                    PolicyScreen::Nominal => {
                        "Optimized responses within the nominal downside screen"
                    }
                    PolicyScreen::Wasserstein => {
                        "Optimized responses within the stressed downside screen"
                    }
                };
                for winner in &search.overall.winners {
                    if winner.kind == ChoiceKind::InformationOption
                        && let Some(policy) = search
                            .options
                            .iter()
                            .find(|o| o.option_id == winner.id)
                            .and_then(|o| o.optimum.as_ref())
                    {
                        write_responses(
                            &mut out,
                            heading,
                            &winner.id,
                            policy.signal_actions.iter().filter_map(|(signal, action)| {
                                Some((signal.as_str(), action.as_deref()?))
                            }),
                        );
                        if search.configuration.screen == PolicyScreen::Nominal {
                            write_fee_headroom(&mut out, &winner.id, &risk.limit, &policy.risk);
                        }
                    }
                }
            } else if let Some(info) = &s.information {
                let (heading, ranking) = match &risk.robust {
                    Some(stressed) => (
                        "Fixed responses within the stressed downside screen",
                        stressed,
                    ),
                    None => (
                        "Fixed responses within the nominal downside screen",
                        &risk.nominal,
                    ),
                };
                for option in info.options.iter().filter(|o| {
                    ranking
                        .winners
                        .iter()
                        .any(|w| w.kind == ChoiceKind::InformationOption && w.id == o.option_id)
                }) {
                    write_fixed_responses(&mut out, heading, option);
                    if risk.robust.is_none() {
                        write_fee_headroom(
                            &mut out,
                            &option.option_id,
                            &risk.limit,
                            &option.policy_risk,
                        );
                    }
                }
            }
            out.push_str("## Unrestricted nominal comparison\n\n");
        }
        let _ = writeln!(
            out,
            "**Act-now comparison.** The highest modeled average value belongs to {}. The smallest worst-case opportunity loss is {} with {}.\n",
            names(&s.expected_value_winners),
            money(self.least_regret.max_regret),
            names(&s.minimax_regret_winners)
        );
        let [lo, mid, hi] = a.p10_p50_p90;
        let _ = writeln!(
            out,
            "**{} ({}).** The modeled 10th, 50th and 90th percentile payoffs are {}, {} and {}. The modeled average is {}. Choosing this action can forgo up to {} compared with the best action in hindsight.\n",
            md(&a.label),
            md(&a.action_id),
            money(lo),
            money(mid),
            money(hi),
            money(a.mean),
            money(a.max_regret)
        );
        if let Some((cost, rivals)) = &self.tie {
            let other = rivals
                .iter()
                .map(|other| label(other))
                .collect::<Vec<_>>()
                .join(", ");
            if cost.major.signum() == 0 {
                let _ = writeln!(
                    out,
                    "**What changes the choice.** {} already ties {other} on average value. Any extra cost charged only to {} in every state breaks that tie.\n",
                    md(&a.action_id),
                    md(&a.action_id)
                );
            } else {
                let _ = writeln!(
                    out,
                    "**What changes the choice.** An additional cost of {} for {} in every state ties {other} on average value.\n",
                    money(*cost),
                    md(&a.action_id)
                );
            }
        } else {
            out.push_str("Only one feasible act-now choice was evaluated.\n\n");
        }
        if let Some(info) = &s.information {
            let winners = info
                .overall_winners
                .iter()
                .map(|w| {
                    let kind = match w.kind {
                        ChoiceKind::ActNow => "act now",
                        ChoiceKind::InformationOption => "buy information",
                    };
                    format!("{} ({kind})", md(&w.id))
                })
                .collect::<Vec<_>>()
                .join(", ");
            let _ = writeln!(
                out,
                "**Nominal testwork comparison.** Including the cost of the test and the delay, {}the highest modeled average value belongs to {winners}.\n",
                if s.downside.is_some() {
                    "before applying the downside limit, "
                } else {
                    ""
                }
            );
            for option in info
                .options
                .iter()
                .filter(|o| info.option_winners.contains(&o.option_id))
            {
                let _ = writeln!(
                    out,
                    "- {} costs {} including {} for {} days of delay. Its net information value over acting now is {}.",
                    md(&option.label),
                    money(option.total_cost),
                    money(option.delay_cost),
                    option.delay_days,
                    money(option.net_voi)
                );
            }
            out.push('\n');
            if s.downside.is_none() {
                for option in info.options.iter().filter(|o| {
                    info.overall_winners
                        .iter()
                        .any(|w| w.kind == ChoiceKind::InformationOption && w.id == o.option_id)
                }) {
                    write_fixed_responses(&mut out, "Nominal responses", option);
                }
            }
        }
        if let Some(robust) = &s.robustness {
            let _ = writeln!(
                out,
                "**Stressed state weights.** The highest worst-case mean among the act-now choices belongs to {}.\n",
                names(&robust.maximin_mean_winners)
            );
            if let Some(fixed) = &robust.fixed_policy_comparison {
                let winners = fixed
                    .maximin_mean_winners
                    .iter()
                    .map(|w| {
                        let kind = match w.kind {
                            ChoiceKind::ActNow => "act now",
                            ChoiceKind::InformationOption => "fixed test policy",
                        };
                        format!("{} ({kind})", md(&w.id))
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                let _ = writeln!(
                    out,
                    "Including fixed test responses, the highest worst-case mean belongs to {winners}.\n"
                );
            }
        }
        for u in &claim.uu_terms {
            let _ = writeln!(
                out,
                "**Outside the model.** {}. The stated trigger is “{}”. The declared downside exposure is {} million USD.\n",
                md(&u.description),
                md(&u.trigger_indicator),
                u.downside_exposure_usd_m
            );
        }
        let _ = writeln!(
            out,
            "The evidence class is {}. The state weights and any test probabilities are uncalibrated. Internal decision support. Output Claim `{}`.",
            super::render::evidence_name(claim.evidence),
            claim.claim_id
        );
        out
    }
}

#[cfg(test)]
mod tests {
    use super::super::{evaluate, model};
    use fmine_core::{anchor::SourceAnchor, hash::sha256};

    #[test]
    fn a_zero_cost_boundary_preserves_all_tied_immediate_choices() {
        let text = include_str!("../../../../tests/decisions/expansion.toml")
            .replace("55000000", "100000000");
        let anchor = SourceAnchor {
            artifact_path: "synthetic-tie.toml".into(),
            artifact_sha256: sha256(text.as_bytes()).0,
            locator: "whole worksheet".into(),
        };
        let inputs = model::parse(&text, anchor).expect("valid tie model");
        let card = evaluate(&inputs).expect("evaluate");
        let brief = card.executive_brief().expect("bind brief").to_markdown();
        assert!(brief.contains("highest modeled average value belongs to Build the expansion (build), Smaller fixed-return option (smaller)"));
        assert!(brief.contains("already ties Smaller fixed-return option (smaller)"));
    }

    #[test]
    fn response_preview_skips_unreachable_signals_and_names_its_limit() {
        let fixture = include_str!("../../../../tests/decisions/metallurgical_test.toml");
        let mut text = fixture.to_owned();
        for signal in [
            "unreachable",
            "result-a",
            "result-b",
            "result-c",
            "result-d",
            "result-e",
        ] {
            text.push_str(&format!(
                r#"
[[signal]]
option = "met_test"
id = "{signal}"
"#
            ));
            for (state, mass) in [("high", "17/120"), ("low", "1/40")] {
                let probability = if signal == "unreachable" { "0/1" } else { mass };
                text.push_str(&format!(
                    r#"
[[likelihood]]
option = "met_test"
state = "{state}"
signal = "{signal}"
probability = "{probability}"
"#
                ));
            }
        }
        // Split the original reachable high signal into six identical readings.
        let text = text
            .replace(
                "signal = \"high\"\nprobability = \"17/20\"",
                "signal = \"high\"\nprobability = \"17/120\"",
            )
            .replace(
                "signal = \"high\"\nprobability = \"3/20\"",
                "signal = \"high\"\nprobability = \"1/40\"",
            );
        let anchor = SourceAnchor {
            artifact_path: "synthetic-response-preview.toml".into(),
            artifact_sha256: sha256(text.as_bytes()).0,
            locator: "whole worksheet".into(),
        };
        let inputs = model::parse(&text, anchor).expect("valid signal split");
        let card = evaluate(&inputs).expect("evaluate");
        let brief = card.executive_brief().expect("bind brief").to_markdown();
        assert_eq!(brief.matches(" → ").count(), 6);
        assert!(!brief.contains("unreachable"));
        assert!(brief.contains("See the full DecisionCard for the remaining 1."));
    }
}
