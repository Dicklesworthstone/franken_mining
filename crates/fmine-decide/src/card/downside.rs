//! Source-bound downside limits over complete act-now and fixed information-policy populations.
//! Determinism: Exact
//! The event is realized net payoff strictly below the declared monetary floor.

use super::*;
use fmine_numerics::risk::exact::compare;
use fmine_numerics::risk::transport::{TransportCertificate, TransportPlan};
use information::ChoiceKind;

mod render;
#[cfg(test)]
mod tests;

pub const SCOPE: &str = "Conditional downside screening over the complete declared act-now actions and displayed fixed one-test policies. A breach means realized net payoff strictly below the monetary floor; equality is not a breach. Information-policy probabilities integrate the original signal likelihoods and charge test and delay costs once in every outcome, including do nothing. Nominal and, when declared, Wasserstein worst-case breach probabilities have separate eligibility results. Both eligible rankings maximize nominal expected value; the stressed probability screen does not change that objective. Every rejected choice remains visible, all ties survive, and an empty eligible set has no fallback. This does not reoptimize signal policies, infer test accuracy or calibrated probabilities, bound loss severity or CVaR, certify physical feasibility, or authorize execution.";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DownsideLimit {
    pub model_key: String,
    pub valuation_basis: String,
    pub floor: Money,
    pub max_breach_probability: Rat,
}

impl DownsideLimit {
    #[must_use]
    pub fn canonical_tag(&self) -> String {
        let mut enc = Encoder::new_frame(*b"DCDL", 1);
        enc.put_str(&self.model_key);
        enc.put_str(&self.valuation_basis);
        enc.put_str(currency_name(self.floor.ccy));
        enc.put_fixed(&self.floor.minor.to_le_bytes());
        put_rat(&mut enc, self.max_breach_probability);
        format!("decision-downside-limit/v1:{}", sha256(&enc.finish()))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DownsideChoiceId {
    pub kind: ChoiceKind,
    pub id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DownsideChoice {
    pub choice: DownsideChoiceId,
    pub nominal_mean: Rat,
    /// Original canonical state order. Policy values integrate realized signal outcomes.
    pub conditional_breach_probabilities: Vec<Rat>,
    pub nominal_breach_probability: Rat,
    pub nominal_eligible: bool,
    pub worst_case_breach_probability: Option<Rat>,
    pub robust_eligible: Option<bool>,
    /// Probability objective; its dual multiplier is probability per dimensionless metric unit.
    pub breach_certificate: Option<TransportCertificate>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DownsideStatus {
    EligibleChoices,
    NoEligibleChoice,
}

impl DownsideStatus {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::EligibleChoices => "eligible_choices",
            Self::NoEligibleChoice => "no_eligible_choice",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DownsideRanking {
    pub status: DownsideStatus,
    pub eligible_order: Vec<DownsideChoiceId>,
    pub winners: Vec<DownsideChoiceId>,
    pub best_expected_value: Option<Rat>,
    /// Unconstrained maximum nominal value minus the best value passing this screen.
    pub expected_value_cost_of_limit: Option<Rat>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DownsideSummary {
    pub limit: DownsideLimit,
    pub state_ids: Vec<String>,
    pub nominal_weights: Vec<Rat>,
    pub choices: Vec<DownsideChoice>,
    pub nominal: DownsideRanking,
    pub robust: Option<DownsideRanking>,
}

pub(super) fn admit_limit(
    configuration: &mut Option<Claim<DownsideLimit>>,
    summary: &DecisionSummary,
) -> Result<ColorRank> {
    let Some(claim) = configuration else {
        return Ok(ColorRank::Verified);
    };
    let color = admit(
        claim,
        &claim.value.canonical_tag(),
        AdditivityClass::NonAdditive,
    )?;
    let limit = &claim.value;
    if limit.model_key != summary.model_key
        || limit.valuation_basis != summary.valuation_basis
        || limit.floor.ccy != summary.currency
    {
        return Err(refusal(
            RefusalCode::BasisMismatch105,
            "downside limit must share the model, currency and valuation basis",
        ));
    }
    if limit.max_breach_probability.signum() < 0
        || compare(limit.max_breach_probability, ONE).is_gt()
    {
        return Err(refusal(
            RefusalCode::OutsidePhysicalDomain109,
            "maximum downside breach probability must lie in [0,1]",
        ));
    }
    Ok(color)
}

fn failed(detail: &str) -> Refusal {
    refusal(RefusalCode::NumericalCertificateFailed207, detail)
}

fn ranking(choices: &[DownsideChoice], robust: bool) -> Result<DownsideRanking> {
    let baseline = choices
        .iter()
        .map(|c| c.nominal_mean)
        .max_by(|a, b| compare(*a, *b))
        .ok_or_else(|| failed("downside screening has no original choice"))?;
    let mut eligible: Vec<_> = choices
        .iter()
        .filter(|c| {
            if robust {
                c.robust_eligible == Some(true)
            } else {
                c.nominal_eligible
            }
        })
        .collect();
    eligible.sort_by(|a, b| {
        compare(b.nominal_mean, a.nominal_mean)
            .then((a.choice.kind, &a.choice.id).cmp(&(b.choice.kind, &b.choice.id)))
    });
    let best_expected_value = eligible.first().map(|c| c.nominal_mean);
    let winners = eligible
        .iter()
        .take_while(|c| Some(c.nominal_mean) == best_expected_value)
        .map(|c| c.choice.clone())
        .collect();
    Ok(DownsideRanking {
        status: if eligible.is_empty() {
            DownsideStatus::NoEligibleChoice
        } else {
            DownsideStatus::EligibleChoices
        },
        eligible_order: eligible.iter().map(|c| c.choice.clone()).collect(),
        winners,
        best_expected_value,
        expected_value_cost_of_limit: best_expected_value
            .map(|value| arithmetic(baseline.checked_sub(value)))
            .transpose()?,
    })
}

/// The limit has passed Claim admission; any transport plan is the same admitted geometry already
/// used for the card's robust values. No policy or original action is dropped by this screen.
pub(super) fn assess(
    configuration: Option<&Claim<DownsideLimit>>,
    options: &[Claim<InformationOption>],
    summary: &DecisionSummary,
    plan: Option<&TransportPlan>,
) -> Result<Option<DownsideSummary>> {
    let Some(claim) = configuration else {
        return Ok(None);
    };
    if plan.is_some() != summary.robustness.is_some() {
        return Err(failed(
            "downside screening lost its admitted transport geometry",
        ));
    }
    let states = &summary
        .alternatives
        .first()
        .ok_or_else(|| failed("downside screening needs the complete state population"))?
        .states;
    let floor = arithmetic(Rat::new(claim.value.floor.minor, Money::SCALE))?;
    let mut choices = Vec::with_capacity(summary.alternatives.len() + options.len());
    for action in &summary.alternatives {
        let conditional = action
            .states
            .iter()
            .map(|s| {
                if compare(s.payoff.major, floor).is_lt() {
                    ONE
                } else {
                    ZERO
                }
            })
            .collect();
        choices.push(evaluate_choice(
            DownsideChoiceId {
                kind: ChoiceKind::ActNow,
                id: action.action_id.clone(),
            },
            action.mean.major,
            conditional,
            states,
            &claim.value,
            plan,
            summary.robustness.as_ref().map(|r| r.design.radius),
        )?);
    }
    if let Some(information) = &summary.information {
        let original: BTreeMap<_, _> = options
            .iter()
            .map(|c| (c.value.option_id.as_str(), &c.value))
            .collect();
        if information.options.len() != options.len() {
            return Err(failed("downside screening lost an information option"));
        }
        for policy in &information.options {
            let option = original.get(policy.option_id.as_str()).ok_or_else(|| {
                failed("downside screening cannot find its original information option")
            })?;
            let conditional =
                robustness::conditional_breach_probabilities(option, policy, summary, floor)?;
            choices.push(evaluate_choice(
                DownsideChoiceId {
                    kind: ChoiceKind::InformationOption,
                    id: policy.option_id.clone(),
                },
                policy.net_value.major,
                conditional,
                states,
                &claim.value,
                plan,
                summary.robustness.as_ref().map(|r| r.design.radius),
            )?);
        }
    } else if !options.is_empty() {
        return Err(failed(
            "downside screening lacks the original information policies",
        ));
    }
    let nominal = ranking(&choices, false)?;
    let robust = plan.map(|_| ranking(&choices, true)).transpose()?;
    Ok(Some(DownsideSummary {
        limit: claim.value.clone(),
        state_ids: states.iter().map(|s| s.state_id.clone()).collect(),
        nominal_weights: states.iter().map(|s| s.weight).collect(),
        choices,
        nominal,
        robust,
    }))
}

fn evaluate_choice(
    choice: DownsideChoiceId,
    nominal_mean: Rat,
    conditional_breach_probabilities: Vec<Rat>,
    states: &[StateResult],
    limit: &DownsideLimit,
    plan: Option<&TransportPlan>,
    radius: Option<Rat>,
) -> Result<DownsideChoice> {
    if conditional_breach_probabilities.len() != states.len() {
        return Err(failed("downside choice lost the common state support"));
    }
    let mut nominal = ZERO;
    for (state, &probability) in states.iter().zip(&conditional_breach_probabilities) {
        if probability.signum() < 0 || compare(probability, ONE).is_gt() {
            return Err(failed("conditional downside probability is outside [0,1]"));
        }
        nominal =
            arithmetic(nominal.checked_add(arithmetic(state.weight.checked_mul(probability))?))?;
    }
    let certificate = plan
        .zip(radius)
        .map(|(plan, radius)| plan.maximize(&conditional_breach_probabilities, radius))
        .transpose()?;
    let worst = certificate.as_ref().map(|c| c.worst_case_value);
    if worst.is_some_and(|p| compare(p, nominal).is_lt() || compare(p, ONE).is_gt()) {
        return Err(failed(
            "worst downside probability must enclose nominal and remain at most one",
        ));
    }
    Ok(DownsideChoice {
        choice,
        nominal_mean,
        conditional_breach_probabilities,
        nominal_breach_probability: nominal,
        nominal_eligible: !compare(nominal, limit.max_breach_probability).is_gt(),
        worst_case_breach_probability: worst,
        robust_eligible: worst.map(|p| !compare(p, limit.max_breach_probability).is_gt()),
        breach_certificate: certificate,
    })
}
