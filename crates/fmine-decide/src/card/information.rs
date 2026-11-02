//! Exact finite preposterior information values and signal-contingent policies (BD-227).
//! Determinism: Exact (rational weights, likelihoods, payoffs and costs; no simulation or clamping).
//!
//! Every option observes a named signal before choosing among the unchanged admitted actions. Comparing
//! expected values and marginal tail risks requires no cross-option joint signal distribution.
//! Cross-option regret and sequential/batched tests still require that unprovided dependence model.

use super::*;

mod policy_risk;
pub use policy_risk::{POLICY_RULE, PolicyPayoffMass, PolicyRisk, RISK_SCOPE};

pub const MAX_INFORMATION_OPTIONS: usize = 64;
pub const MAX_SIGNALS: usize = 256;
pub const MAX_INFORMATION_WORK: usize = 4_000_000;
pub const METHOD: &str = "exact_finite_enumeration";
pub const STANDARD_ERROR_NOTE: &str = "Not applicable: all finite states and signals are enumerated exactly; no Monte Carlo sampling occurs. This is conditional mathematical exactness, not a calibrated uncertainty estimate for the declared weights, likelihoods or payoffs, and does not pass GATE-HELM-02.";
pub const SCOPE: &str = "Choose an act-now action or buy at most one information option and follow its signal policy. Costs are paid once before the signal. Delay cost and duration are independently declared; no discounting or cash-flow rule is inferred. Each test policy has its own exact marginal payoff risk. Cross-option regret and combined tests remain undefined without a joint counterfactual signal model; the act-now regret comparison is unchanged.";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Likelihood {
    pub state_id: String,
    pub signal_id: String,
    pub probability: Rat,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InformationOption {
    pub model_key: String,
    pub option_id: String,
    pub label: String,
    pub currency: Currency,
    pub valuation_basis: String,
    pub test_cost: Money,
    pub delay_cost: Money,
    pub delay_days: u32,
    pub signals: Vec<String>,
    pub likelihoods: Vec<Likelihood>,
}

impl InformationOption {
    #[must_use]
    pub fn canonical_tag(&self) -> String {
        let mut enc = Encoder::new_frame(*b"DCIO", 1);
        for text in [
            &self.model_key,
            &self.option_id,
            &self.label,
            currency_name(self.currency),
            &self.valuation_basis,
        ] {
            enc.put_str(text);
        }
        for cost in [self.test_cost, self.delay_cost] {
            enc.put_str(currency_name(cost.ccy));
            enc.put_fixed(&cost.minor.to_le_bytes());
        }
        enc.put_u32(self.delay_days);
        let mut signals = self.signals.clone();
        signals.sort();
        enc.put_u64(signals.len() as u64);
        for signal in signals {
            enc.put_str(&signal);
        }
        let mut likelihoods = self.likelihoods.clone();
        sort_likelihoods(&mut likelihoods);
        enc.put_u64(likelihoods.len() as u64);
        for row in likelihoods {
            enc.put_str(&row.state_id);
            enc.put_str(&row.signal_id);
            put_rat(&mut enc, row.probability);
        }
        format!("decision-information/v1:{}", sha256(&enc.finish()))
    }
}

fn sort_likelihoods(rows: &mut [Likelihood]) {
    rows.sort_by(|a, b| {
        a.state_id
            .cmp(&b.state_id)
            .then(a.signal_id.cmp(&b.signal_id))
            .then(a.probability.num().cmp(&b.probability.num()))
            .then(a.probability.den().cmp(&b.probability.den()))
    });
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetStatus {
    Positive,
    Zero,
    Negative,
}
impl NetStatus {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Positive => "positive",
            Self::Zero => "zero",
            Self::Negative => "negative",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PosteriorActionValue {
    pub action_id: String,
    pub value: ExactMoney,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignalPolicy {
    pub signal_id: String,
    pub probability: Rat,
    pub posterior_state_weights: Option<Vec<(String, Rat)>>,
    pub posterior_action_values: Option<Vec<PosteriorActionValue>>,
    pub best_actions: Vec<String>,
    pub selected_action: Option<String>,
    pub conditional_value: Option<ExactMoney>,
    /// True only when the prior display-primary is strictly worse than the conditional co-winners.
    pub strict_primary_change: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InformationStats {
    pub option_id: String,
    pub label: String,
    pub test_cost: ExactMoney,
    pub delay_cost: ExactMoney,
    pub total_cost: ExactMoney,
    pub delay_days: u32,
    pub gross_value: ExactMoney,
    pub net_value: ExactMoney,
    pub evsi: ExactMoney,
    pub net_voi: ExactMoney,
    pub status: NetStatus,
    pub strict_change_probability: Rat,
    pub baseline_optimal_after_every_reachable_signal: bool,
    pub signals: Vec<SignalPolicy>,
    pub policy_risk: PolicyRisk,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ChoiceKind {
    ActNow,
    InformationOption,
}
impl ChoiceKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ActNow => "act_now",
            Self::InformationOption => "information_option",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpectedChoice {
    pub kind: ChoiceKind,
    pub id: String,
    pub expected_value: ExactMoney,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InformationSummary {
    pub baseline_primary: String,
    pub baseline_winners: Vec<String>,
    pub baseline_value: ExactMoney,
    pub perfect_information_value: ExactMoney,
    pub evpi: ExactMoney,
    pub options: Vec<InformationStats>,
    pub option_order: Vec<String>,
    pub option_winners: Vec<String>,
    pub overall_order: Vec<ExpectedChoice>,
    pub overall_winners: Vec<ExpectedChoice>,
}

pub(super) fn check_work(inputs: &DecisionInputs) -> Result<()> {
    if inputs.information_options.len() > MAX_INFORMATION_OPTIONS {
        return Err(refusal(
            RefusalCode::OutsideAoa203,
            format!("at most {MAX_INFORMATION_OPTIONS} information options are supported"),
        ));
    }
    let mut work = 0usize;
    for option in &inputs.information_options {
        let signals = option.value.signals.len();
        if signals == 0 || signals > MAX_SIGNALS {
            return Err(refusal(
                RefusalCode::OutsideAoa203,
                format!(
                    "{} requires 1..={MAX_SIGNALS} signals",
                    option.value.option_id
                ),
            ));
        }
        let cells = inputs
            .weights
            .value
            .states
            .len()
            .checked_mul(signals)
            .ok_or_else(|| {
                refusal(
                    RefusalCode::OutsideAoa203,
                    "information matrix size overflow",
                )
            })?;
        // Exact cardinality avoids cloning a huge malformed matrix before completeness can be checked.
        if option.value.likelihoods.len() != cells {
            return Err(refusal(
                RefusalCode::NoClaimInDomain201,
                format!(
                    "{} requires every state/signal likelihood, including zeros",
                    option.value.option_id
                ),
            ));
        }
        work = cells
            .checked_mul(inputs.actions.len())
            .and_then(|n| work.checked_add(n))
            .ok_or_else(|| refusal(RefusalCode::OutsideAoa203, "information work size overflow"))?;
        if work > MAX_INFORMATION_WORK {
            return Err(refusal(
                RefusalCode::OutsideAoa203,
                format!(
                    "information evaluation exceeds {MAX_INFORMATION_WORK} state/signal/action cells"
                ),
            ));
        }
    }
    Ok(())
}

fn validate(option: &InformationOption, base: &DecisionSummary) -> Result<()> {
    if !named(&option.option_id)
        || !named(&option.label)
        || option.model_key != base.model_key
        || option.currency != base.currency
        || option.valuation_basis != base.valuation_basis
    {
        return Err(refusal(
            RefusalCode::BasisMismatch105,
            "information option needs a name and the same model, currency and valuation basis",
        ));
    }
    for cost in [option.test_cost, option.delay_cost] {
        if cost.ccy != base.currency || cost.minor < 0 {
            return Err(refusal(
                RefusalCode::BasisMismatch105,
                format!(
                    "{} costs must be explicit nonnegative amounts in the model currency",
                    option.option_id
                ),
            ));
        }
    }
    let signals: BTreeSet<_> = option.signals.iter().collect();
    if signals.len() != option.signals.len() || signals.iter().any(|id| !named(id)) {
        return Err(refusal(
            RefusalCode::BasisMismatch105,
            "information signal ids must be distinct and named",
        ));
    }
    let mut totals: BTreeMap<_, Rat> = base.alternatives[0]
        .states
        .iter()
        .map(|s| (s.state_id.as_str(), ZERO))
        .collect();
    let mut pairs = BTreeSet::new();
    for row in &option.likelihoods {
        if !signals.contains(&row.signal_id) || !pairs.insert((&row.state_id, &row.signal_id)) {
            return Err(refusal(
                RefusalCode::BasisMismatch105,
                format!(
                    "{} has an unknown signal or duplicate state/signal pair",
                    option.option_id
                ),
            ));
        }
        let total = totals.get_mut(row.state_id.as_str()).ok_or_else(|| {
            refusal(
                RefusalCode::BasisMismatch105,
                format!(
                    "{} likelihood names unknown state {}",
                    option.option_id, row.state_id
                ),
            )
        })?;
        if row.probability.signum() < 0
            || arithmetic(row.probability.checked_cmp(ONE))? == Ordering::Greater
        {
            return Err(refusal(
                RefusalCode::OutsidePhysicalDomain109,
                "conditional signal likelihoods must lie in [0,1]",
            ));
        }
        *total = arithmetic(total.checked_add(row.probability))?;
    }
    if totals.values().any(|v| *v != ONE) {
        return Err(refusal(
            RefusalCode::OutsidePhysicalDomain109,
            format!(
                "{} likelihoods must sum exactly to one within every state; no normalization is implicit",
                option.option_id
            ),
        ));
    }
    Ok(())
}

fn evaluate_option(
    option: &InformationOption,
    base: &DecisionSummary,
    baseline: Rat,
    evpi: Rat,
) -> Result<InformationStats> {
    let money = |major| ExactMoney {
        currency: base.currency,
        major,
    };
    let likelihoods: BTreeMap<_, _> = option
        .likelihoods
        .iter()
        .map(|r| ((r.state_id.as_str(), r.signal_id.as_str()), r.probability))
        .collect();
    let mut gross = ZERO;
    let mut change_probability = ZERO;
    let mut policies = Vec::new();
    for signal in &option.signals {
        let mut probability = ZERO;
        let mut joint_states = Vec::new();
        for state in &base.alternatives[0].states {
            let mass = arithmetic(
                state
                    .weight
                    .checked_mul(likelihoods[&(state.state_id.as_str(), signal.as_str())]),
            )?;
            probability = arithmetic(probability.checked_add(mass))?;
            joint_states.push((state.state_id.clone(), mass));
        }
        if probability == ZERO {
            policies.push(SignalPolicy {
                signal_id: signal.clone(),
                probability,
                posterior_state_weights: None,
                posterior_action_values: None,
                best_actions: Vec::new(),
                selected_action: None,
                conditional_value: None,
                strict_primary_change: None,
            });
            continue;
        }
        let mut joint_values = Vec::new();
        let mut best: Option<Rat> = None;
        let mut best_actions = Vec::new();
        for action in &base.alternatives {
            let mut value = ZERO;
            for (state, (_, mass)) in action.states.iter().zip(&joint_states) {
                value = arithmetic(
                    value.checked_add(arithmetic(state.payoff.major.checked_mul(*mass))?),
                )?;
            }
            let ordering = match best {
                Some(old) => arithmetic(value.checked_cmp(old))?,
                None => Ordering::Greater,
            };
            if ordering == Ordering::Greater {
                best = Some(value);
                best_actions.clear();
            }
            if ordering != Ordering::Less {
                best_actions.push(action.action_id.clone());
            }
            joint_values.push(PosteriorActionValue {
                action_id: action.action_id.clone(),
                value: money(arithmetic(value.checked_div(probability))?),
            });
        }
        let best = best.ok_or_else(|| {
            refusal(
                RefusalCode::NumericalCertificateFailed207,
                "information policy lacks an action",
            )
        })?;
        gross = arithmetic(gross.checked_add(best))?;
        let changed = !best_actions.contains(&base.expected_value_primary);
        if changed {
            change_probability = arithmetic(change_probability.checked_add(probability))?;
        }
        let posterior = joint_states
            .into_iter()
            .map(|(id, mass)| arithmetic(mass.checked_div(probability)).map(|weight| (id, weight)))
            .collect::<Result<Vec<_>>>()?;
        policies.push(SignalPolicy {
            signal_id: signal.clone(),
            probability,
            posterior_state_weights: Some(posterior),
            posterior_action_values: Some(joint_values),
            selected_action: best_actions.first().cloned(),
            best_actions,
            conditional_value: Some(money(arithmetic(best.checked_div(probability))?)),
            strict_primary_change: Some(changed),
        });
    }
    let evsi = arithmetic(gross.checked_sub(baseline))?;
    if evsi.signum() < 0 || arithmetic(evsi.checked_cmp(evpi))? == Ordering::Greater {
        return Err(refusal(
            RefusalCode::NumericalCertificateFailed207,
            "enumerated EVSI lies outside [0, EVPI]",
        ));
    }
    // No positive-probability strict improvement means the same prior primary stays optimal everywhere.
    if (evsi == ZERO) != (change_probability == ZERO) {
        return Err(refusal(
            RefusalCode::NumericalCertificateFailed207,
            "zero EVSI disagrees with the exact signal-policy change check",
        ));
    }
    let test_cost = arithmetic(Rat::new(option.test_cost.minor, Money::SCALE))?;
    let delay_cost = arithmetic(Rat::new(option.delay_cost.minor, Money::SCALE))?;
    let total_cost = arithmetic(test_cost.checked_add(delay_cost))?;
    let net_voi = arithmetic(evsi.checked_sub(total_cost))?;
    let status = match net_voi.signum() {
        1 => NetStatus::Positive,
        0 => NetStatus::Zero,
        _ => NetStatus::Negative,
    };
    let policy_risk =
        policy_risk::evaluate(option, base, &policies, &likelihoods, total_cost, gross)?;
    Ok(InformationStats {
        option_id: option.option_id.clone(),
        label: option.label.clone(),
        test_cost: money(test_cost),
        delay_cost: money(delay_cost),
        total_cost: money(total_cost),
        delay_days: option.delay_days,
        gross_value: money(gross),
        net_value: money(arithmetic(gross.checked_sub(total_cost))?),
        evsi: money(evsi),
        net_voi: money(net_voi),
        status,
        strict_change_probability: change_probability,
        baseline_optimal_after_every_reachable_signal: change_probability == ZERO,
        signals: policies,
        policy_risk,
    })
}

fn rank(choices: Vec<ExpectedChoice>) -> Result<(Vec<ExpectedChoice>, Vec<ExpectedChoice>)> {
    let mut ordered: Vec<ExpectedChoice> = Vec::new();
    for candidate in choices {
        let mut index = ordered.len();
        for (i, old) in ordered.iter().enumerate() {
            let cmp = arithmetic(
                candidate
                    .expected_value
                    .major
                    .checked_cmp(old.expected_value.major),
            )?;
            if cmp == Ordering::Greater
                || (cmp == Ordering::Equal && (candidate.kind, &candidate.id) < (old.kind, &old.id))
            {
                index = i;
                break;
            }
        }
        ordered.insert(index, candidate);
    }
    let winners = if let Some(first) = ordered.first() {
        ordered
            .iter()
            .take_while(|c| c.expected_value == first.expected_value)
            .cloned()
            .collect()
    } else {
        Vec::new()
    };
    Ok((ordered, winners))
}

pub(super) fn assess(
    options: &mut [Claim<InformationOption>],
    base: &DecisionSummary,
) -> Result<(Option<InformationSummary>, ColorRank)> {
    if options.is_empty() {
        return Ok((None, ColorRank::Verified));
    }
    options.sort_by(|a, b| a.value.option_id.cmp(&b.value.option_id));
    let mut weakest = ColorRank::Verified;
    let mut ids = BTreeSet::new();
    for claim in options.iter_mut() {
        weakest = weakest.min(admit(
            claim,
            &claim.value.canonical_tag(),
            AdditivityClass::NonAdditive,
        )?);
        if !ids.insert(claim.value.option_id.clone()) {
            return Err(refusal(
                RefusalCode::BasisMismatch105,
                "information option ids must be distinct",
            ));
        }
        claim.value.signals.sort();
        sort_likelihoods(&mut claim.value.likelihoods);
        validate(&claim.value, base)?;
    }
    let baseline = base
        .alternatives
        .iter()
        .find(|a| a.action_id == base.expected_value_primary)
        .ok_or_else(|| {
            refusal(
                RefusalCode::NumericalCertificateFailed207,
                "missing base recommendation",
            )
        })?
        .mean;
    let mut perfect = ZERO;
    for state in &base.alternatives[0].states {
        perfect = arithmetic(perfect.checked_add(arithmetic(
            state.best_in_hindsight.major.checked_mul(state.weight),
        )?))?;
    }
    let evpi = arithmetic(perfect.checked_sub(baseline.major))?;
    if evpi.signum() < 0 {
        return Err(refusal(
            RefusalCode::NumericalCertificateFailed207,
            "perfect information value is below the prior optimum",
        ));
    }
    let stats = options
        .iter()
        .map(|option| evaluate_option(&option.value, base, baseline.major, evpi))
        .collect::<Result<Vec<_>>>()?;
    let candidates: Vec<_> = stats
        .iter()
        .map(|o| ExpectedChoice {
            kind: ChoiceKind::InformationOption,
            id: o.option_id.clone(),
            expected_value: o.net_value,
        })
        .collect();
    let (option_order, option_winners) = rank(candidates.clone())?;
    let mut combined = candidates;
    combined.extend(base.alternatives.iter().map(|a| ExpectedChoice {
        kind: ChoiceKind::ActNow,
        id: a.action_id.clone(),
        expected_value: a.mean,
    }));
    let (overall_order, overall_winners) = rank(combined)?;
    let money = |major| ExactMoney {
        currency: base.currency,
        major,
    };
    Ok((
        Some(InformationSummary {
            baseline_primary: base.expected_value_primary.clone(),
            baseline_winners: base.expected_value_winners.clone(),
            baseline_value: baseline,
            perfect_information_value: money(perfect),
            evpi: money(evpi),
            options: stats,
            option_order: option_order.into_iter().map(|c| c.id).collect(),
            option_winners: option_winners.into_iter().map(|c| c.id).collect(),
            overall_order,
            overall_winners,
        }),
        weakest,
    ))
}
