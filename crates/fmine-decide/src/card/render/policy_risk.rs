//! Exact marginal policy laws; all amounts are already derived and bound by the card's Claim.
//! Determinism: Exact

use crate::card::information::{POLICY_RULE, PolicyRisk, RISK_SCOPE};

use super::{currency_name, ids, js, rq};

pub(super) fn json(risk: &PolicyRisk) -> String {
    let distribution = risk
        .distribution
        .iter()
        .map(|bin| {
            format!(
                "{{\"probability\":{},\"gross_payoff\":{},\"net_payoff\":{}}}",
                rq(bin.probability),
                rq(bin.gross_payoff.major),
                rq(bin.net_payoff.major)
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"schema\":\"fmine.policy-risk.v1\",\"method\":\"exact_state_signal_enumeration\",\"currency\":{},\"monetary_scale\":\"major_currency_units\",\"mean\":{},\"p10_p50_p90\":[{},{},{}],\"cvar95_loss\":{},\"loss_probability\":{},\"distribution\":[{distribution}],\"reachable_tie_signals\":{},\"policy_rule\":{},\"scope\":{}}}",
        js(currency_name(risk.mean.currency)),
        rq(risk.mean.major),
        rq(risk.p10_p50_p90[0].major),
        rq(risk.p10_p50_p90[1].major),
        rq(risk.p10_p50_p90[2].major),
        rq(risk.cvar95_loss.major),
        rq(risk.loss_probability),
        ids(&risk.reachable_tie_signals),
        js(POLICY_RULE),
        js(RISK_SCOPE)
    )
}
