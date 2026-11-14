//! Explicit journal choices over the existing evaluated card. No policy is inferred from a ranking.
//! Determinism: Exact

use super::{clean, invalid, js};
use fmine_decide::card::{EvaluatedCard, information};
use fmine_evidence::Refusal;
use fmine_numerics::exact_feas::Rat;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolicySelection {
    ExpectedValue,
    DownsideOptimized,
}

impl PolicySelection {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ExpectedValue => "expected-value",
            Self::DownsideOptimized => "downside-optimized",
        }
    }

    pub(super) fn parse(value: &str) -> Result<Self, String> {
        match value {
            "expected-value" => Ok(Self::ExpectedValue),
            "downside-optimized" => Ok(Self::DownsideOptimized),
            _ => Err("policy must be expected-value or downside-optimized".into()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Choice {
    ActNow(String),
    InformationOption {
        option_id: String,
        policy: PolicySelection,
    },
}

fn ratio(value: Rat) -> String {
    js(&format!("{}/{}", value.num(), value.den()))
}

impl Choice {
    pub(super) fn validate(&self) -> Result<(), Refusal> {
        let id = match self {
            Self::ActNow(action) => action,
            Self::InformationOption { option_id, .. } => option_id,
        };
        if !clean(id, 256) {
            return Err(invalid(
                "selected action or information option needs a bounded id",
            ));
        }
        Ok(())
    }

    pub(super) fn description(&self) -> String {
        match self {
            Self::ActNow(action) => format!("Selected action: {action}."),
            Self::InformationOption { option_id, policy } => format!(
                "Selected information option: {option_id}. Policy: {}.",
                policy.as_str()
            ),
        }
    }

    /// Retains a small exact selection receipt alongside the complete evaluated card. The receipt
    /// names the original option Claim and policy family; identical option ids never imply that the
    /// unconstrained and downside-optimized signal mappings are interchangeable.
    pub(super) fn snapshot(&self, evaluated: &EvaluatedCard) -> Result<Option<String>, Refusal> {
        self.validate()?;
        let summary = evaluated.summary();
        let (option_id, policy) = match self {
            Self::ActNow(action) => {
                if !summary
                    .alternatives
                    .iter()
                    .any(|row| row.action_id == *action)
                {
                    return Err(invalid(
                        "selected action is not an act-now alternative in the evaluated model",
                    ));
                }
                return Ok(None);
            }
            Self::InformationOption { option_id, policy } => (option_id, policy),
        };
        let option = summary
            .information
            .as_ref()
            .and_then(|information| {
                information
                    .options
                    .iter()
                    .find(|o| o.option_id == *option_id)
            })
            .ok_or_else(|| {
                invalid("selected information option is absent from the evaluated model")
            })?;
        let source = evaluated
            .inputs()
            .information_options
            .iter()
            .find(|o| o.value.option_id == *option_id)
            .ok_or_else(|| invalid("selected information option has no retained input Claim"))?;
        let (mapping, mean, loss, cvar, policy_details) = match policy {
            PolicySelection::ExpectedValue => {
                let mapping = option
                    .signals
                    .iter()
                    .map(|signal| {
                        let tied = signal.best_actions.iter().map(|id| js(id)).collect::<Vec<_>>().join(",");
                        format!(
                            "{{\"signal_id\":{},\"probability\":{},\"action_id\":{},\"posterior_best_actions\":[{tied}]}}",
                            js(&signal.signal_id), ratio(signal.probability),
                            signal.selected_action.as_deref().map_or_else(|| "null".into(), js),
                        )
                    })
                    .collect::<Vec<_>>();
                (
                    mapping,
                    option.net_value.major,
                    option.policy_risk.loss_probability,
                    option.policy_risk.cvar95_loss.major,
                    format!("\"tie_rule\":{}", js(information::POLICY_RULE)),
                )
            }
            PolicySelection::DownsideOptimized => {
                let search = summary
                    .downside
                    .as_ref()
                    .and_then(|d| d.policy_optimization.as_ref())
                    .ok_or_else(|| {
                        invalid("downside-optimized choice requires declared policy optimization")
                    })?;
                let row = search
                    .options
                    .iter()
                    .find(|row| row.option_id == *option_id)
                    .ok_or_else(|| invalid("selected option has no retained policy search"))?;
                let optimum = row.optimum.as_ref().ok_or_else(|| {
                    invalid("selected option has no feasible downside-optimized policy")
                })?;
                let mapping = optimum
                    .signal_actions
                    .iter()
                    .map(|(signal, action)| {
                        format!(
                            "{{\"signal_id\":{},\"action_id\":{}}}",
                            js(signal),
                            action.as_deref().map_or_else(|| "null".into(), js)
                        )
                    })
                    .collect::<Vec<_>>();
                (
                    mapping,
                    optimum.nominal_mean,
                    optimum.risk.loss_probability,
                    optimum.risk.cvar95_loss.major,
                    format!(
                        "\"tie_rule\":{},\"screen\":{},\"optimal_policy_count\":{},\"nominal_breach_probability\":{},\"screened_breach_probability\":{}",
                        js(fmine_decide::card::downside::policies::TIE_RULE),
                        js(search.configuration.screen.as_str()),
                        row.optimal_policy_count,
                        ratio(optimum.nominal_breach_probability),
                        ratio(optimum.screened_breach_probability),
                    ),
                )
            }
        };
        Ok(Some(format!(
            "{{\"schema\":\"fmine.decision-selected-policy.v1\",\"option_id\":{},\"policy_selection\":{},\"option_claim_id\":{},\"card_claim_id\":{},\"currency\":{},\"valuation_basis\":{},\"test_cost\":{},\"delay_cost\":{},\"total_cost\":{},\"delay_days\":{},\"nominal_mean\":{},\"nominal_loss_probability\":{},\"nominal_cvar95_loss\":{},\"signal_actions\":[{}],{policy_details}}}",
            js(option_id),
            js(policy.as_str()),
            js(&source.claim_id.to_string()),
            js(&evaluated.claim().claim_id.to_string()),
            js(&format!("{:?}", summary.currency)),
            js(&summary.valuation_basis),
            ratio(option.test_cost.major),
            ratio(option.delay_cost.major),
            ratio(option.total_cost.major),
            option.delay_days,
            ratio(mean),
            ratio(loss),
            ratio(cvar),
            mapping.join(","),
        )))
    }
}
