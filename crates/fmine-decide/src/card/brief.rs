//! DESIGN (BD-054, §9.6). The short view reads the same sealed finite-scenario Claim as the analyst report.
//! Determinism: Exact

use std::fmt::Write as _;

use super::information::ChoiceKind;
use super::render::{md, short_decimal};
use super::{AlternativeStats, EvaluatedCard, ExactMoney, Result, arithmetic, currency_name};

/// The action-specific range and the first uniform extra cost that would tie another act-now choice.
/// A missing challenger means the admitted comparison contained just one feasible action.
pub struct EpistemicOnePager<'a> {
    card: &'a EvaluatedCard,
    primary: &'a AlternativeStats,
    least_regret: &'a AlternativeStats,
    tie: Option<(ExactMoney, Vec<&'a AlternativeStats>)>,
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
                "**Test before committing.** Including the cost of the test and the delay, the highest modeled average value belongs to {winners}.\n"
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
        if let Some(risk) = &s.downside {
            out.push_str(&risk.decision_brief());
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
}
