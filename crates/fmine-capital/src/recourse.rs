//! DESIGN (BD-229, §23): bounded two-stage policies over explicitly declared information sets.
//! Determinism: Exact (static subsets remain the complete independent comparison population).

use super::*;

mod render;
#[cfg(test)]
mod tests;

pub const MAX_OBSERVATIONS: usize = 4;
pub const MAX_POLICIES: usize = 256;
pub const NO_CLAIM: &str = "Exact bounded two-stage comparison conditional on the declared shared scenarios, paid information gate, observation classes, stage assignments, dated cash flows and net NAV inputs. Initial commitments and their full declared effects are retained in every continuation. A policy can respond only to the declared observation after purchasing the information gate; otherwise one common continuation applies to every state. Risk is evaluated over each complete unconditional policy distribution. Net adaptive value is reported separately against the best static subset under the same objective and risk constraint; it is already included in policy outcomes and must not be added again. Cash checks cover only opening and declared period-end checkpoints. Input weights and information assumptions are uncalibrated. No further stages, implicit information, intra-period payment ordering, entity waterfalls, execution authorization, regulatory claim, selection-frequency estimate or historical validation is supplied.";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecisionStage {
    Initial,
    Recourse,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CandidateStage {
    pub candidate_id: String,
    pub stage: DecisionStage,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateObservation {
    pub state_id: String,
    pub observation_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecourseBasis {
    pub model_key: String,
    pub currency: Currency,
    pub valuation_basis: String,
    pub information_candidate: String,
    pub observation_date: String,
    pub information_basis: String,
    pub candidate_stages: Vec<CandidateStage>,
    pub observations: Vec<StateObservation>,
}

impl RecourseBasis {
    #[must_use]
    pub fn canonical_tag(&self) -> String {
        super::render::value_tag("capital-recourse-basis/v1", &render::basis_json(self))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RecourseInputs {
    pub program: ProgramInputs,
    pub information: Claim<RecourseBasis>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicyBranch {
    /// `None` is the common continuation when the information gate was not selected.
    pub observation_id: Option<String>,
    pub probability: Rat,
    /// Only second-stage candidates; initial commitments are listed separately.
    pub selected: Vec<String>,
    pub spend: ExactMoney,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicyState {
    pub state_id: String,
    pub observation_id: Option<String>,
    /// The retained static subset whose same-state values are realized on this path.
    pub bundle_id: String,
    pub selected: Vec<String>,
    pub spend: ExactMoney,
    pub budget_slack: ExactMoney,
    pub net_nav: ExactMoney,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicyRejection {
    pub state_id: String,
    pub reason: StructuralRejection,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicyChoice {
    pub policy_id: String,
    pub initial_selected: Vec<String>,
    pub initial_spend: ExactMoney,
    pub information_acquired: bool,
    pub branches: Vec<PolicyBranch>,
    pub states: Vec<PolicyState>,
    pub structural_rejections: Vec<PolicyRejection>,
    pub funding_schedule: FundingScheduleAssessment,
    pub equity: Option<EquityAssessment>,
    pub statistics: Option<AlternativeStats>,
    pub tail_loss: Option<ExactMoney>,
    pub per_share_statistics: Option<PerShareStats>,
    pub per_share_tail_loss: Option<ExactPerShare>,
    pub risk_rejected: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecourseSummary {
    pub model_key: String,
    pub question: String,
    pub currency: Currency,
    pub information: RecourseBasis,
    pub per_share_objective: bool,
    pub policies: Vec<PolicyChoice>,
    pub unconstrained_winners: Vec<String>,
    pub feasible_winners: Vec<String>,
    pub expected_value_cost_of_risk_limit: Option<ExactMoney>,
    pub expected_per_share_cost_of_risk_limit: Option<ExactPerShare>,
    /// Best risk-feasible policy minus best risk-feasible static subset; absent if either is missing.
    pub net_adaptive_value: Option<ExactMoney>,
    pub net_adaptive_value_per_share: Option<ExactPerShare>,
    pub unconstrained_net_adaptive_value: Option<ExactMoney>,
    pub unconstrained_net_adaptive_value_per_share: Option<ExactPerShare>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EvaluatedRecourseProgram {
    inputs: RecourseInputs,
    static_program: EvaluatedProgram,
    claim: Claim<RecourseSummary>,
    digest: String,
}

impl EvaluatedRecourseProgram {
    #[must_use]
    pub fn inputs(&self) -> &RecourseInputs {
        &self.inputs
    }
    #[must_use]
    pub fn static_program(&self) -> &EvaluatedProgram {
        &self.static_program
    }
    #[must_use]
    pub fn summary(&self) -> &RecourseSummary {
        &self.claim.value
    }
    #[must_use]
    pub fn claim(&self) -> &Claim<RecourseSummary> {
        &self.claim
    }
    #[must_use]
    pub fn digest(&self) -> &str {
        &self.digest
    }
    #[must_use]
    pub fn no_claim(&self) -> &'static str {
        NO_CLAIM
    }
}

fn invalid(why: impl Into<String>) -> Refusal {
    bad(RefusalCode::BasisMismatch105, why)
}

fn internal(why: impl Into<String>) -> Refusal {
    bad(RefusalCode::NumericalCertificateFailed207, why)
}

struct Admission {
    metadata: Metadata,
    policy_count: usize,
}

/// Work is bounded before either the information tag or any static catalog is constructed.
fn admit_inputs(inputs: &RecourseInputs) -> Result<Admission> {
    let program = &inputs.program;
    let basis = &inputs.information.value;
    let candidate_count = program.candidates.len();
    let state_count = program.weights.value.states.len();
    if !(1..=MAX_CANDIDATES).contains(&candidate_count)
        || !(1..=MAX_STATES).contains(&state_count)
        || basis.candidate_stages.len() != candidate_count
        || basis.observations.len() != state_count
    {
        return Err(bad(
            RefusalCode::NoClaimInDomain201,
            "two-stage programs require bounded complete candidate-stage and state-observation declarations",
        ));
    }
    let schedule = &program
        .funding_schedule
        .as_ref()
        .ok_or_else(|| {
            bad(
                RefusalCode::NoClaimInDomain201,
                "two-stage programs require a dated funding schedule",
            )
        })?
        .value;
    if !(2..=MAX_CASH_PERIODS).contains(&schedule.periods.len()) {
        return Err(bad(
            RefusalCode::NoClaimInDomain201,
            "two-stage programs require 2..=16 cash periods with a declared observation boundary",
        ));
    }
    let candidate_ids: BTreeSet<_> = program
        .candidates
        .iter()
        .map(|c| c.value.candidate_id.as_str())
        .collect();
    let state_ids: BTreeSet<_> = program
        .weights
        .value
        .states
        .iter()
        .map(|(id, _)| id.as_str())
        .collect();
    let mut stages = BTreeMap::new();
    for row in &basis.candidate_stages {
        if !candidate_ids.contains(row.candidate_id.as_str())
            || stages
                .insert(row.candidate_id.as_str(), row.stage)
                .is_some()
        {
            return Err(invalid(
                "each known candidate needs exactly one initial or recourse stage",
            ));
        }
    }
    let mut observed_states = BTreeSet::new();
    let mut observations = BTreeSet::new();
    for row in &basis.observations {
        if !state_ids.contains(row.state_id.as_str())
            || !observed_states.insert(row.state_id.as_str())
            || !key(&row.observation_id)
        {
            return Err(invalid(
                "each shared state needs exactly one bounded named observation",
            ));
        }
        observations.insert(row.observation_id.as_str());
    }
    let initial_count = stages
        .values()
        .filter(|stage| **stage == DecisionStage::Initial)
        .count();
    let recourse_count = stages
        .values()
        .filter(|stage| **stage == DecisionStage::Recourse)
        .count();
    if initial_count == 0 || recourse_count == 0 || observations.len() > MAX_OBSERVATIONS {
        return Err(bad(
            RefusalCode::NoClaimInDomain201,
            "two-stage programs require both stages and 1..=4 populated observation classes",
        ));
    }
    if stages.get(basis.information_candidate.as_str()) != Some(&DecisionStage::Initial) {
        return Err(invalid(
            "the information candidate must be a known initial-stage candidate",
        ));
    }
    // The inactive gate leaves one common continuation; the active gate permits one per observation.
    let continuations = 1_usize << recourse_count;
    let adaptive = continuations
        .checked_pow(observations.len() as u32)
        .ok_or_else(|| {
            bad(
                RefusalCode::NoClaimInDomain201,
                "recourse policy count exceeds the bounded domain",
            )
        })?;
    let policy_count = (1_usize << (initial_count - 1))
        .checked_mul(continuations.checked_add(adaptive).ok_or_else(|| {
            bad(
                RefusalCode::NoClaimInDomain201,
                "recourse policy count exceeds the bounded domain",
            )
        })?)
        .ok_or_else(|| {
            bad(
                RefusalCode::NoClaimInDomain201,
                "recourse policy count exceeds the bounded domain",
            )
        })?;
    let checkpoints = (1_usize << candidate_count)
        .checked_add(policy_count)
        .and_then(|count| count.checked_mul(state_count))
        .and_then(|count| count.checked_mul(schedule.periods.len() + 1));
    if policy_count > MAX_POLICIES || checkpoints.is_none_or(|count| count > MAX_CASH_CHECKPOINTS) {
        return Err(bad(
            RefusalCode::NoClaimInDomain201,
            "two-stage programs admit at most 256 policies and 65536 combined static/policy cash checkpoints",
        ));
    }
    let mut metadata = super::validate(program)?;
    if basis.model_key != program.policy.value.model_key
        || basis.currency != program.policy.value.currency
        || basis.valuation_basis != program.policy.value.valuation_basis
        || !named(&basis.information_basis)
    {
        return Err(invalid(
            "information must share the program model/currency/valuation basis and declare its own basis",
        ));
    }
    if !fmine_registry::sources::is_date(&basis.observation_date) {
        return Err(bad(
            RefusalCode::UndeclaredUnitBasisOrTz107,
            "observation_date must be a valid YYYY-MM-DD date",
        ));
    }
    let mut periods: Vec<_> = schedule.periods.iter().collect();
    periods.sort_by(|a, b| (&a.cash_period, &a.id).cmp(&(&b.cash_period, &b.id)));
    let boundary = periods.iter().position(|period| {
        period_dates(&period.cash_period).is_some_and(|(start, _)| start == basis.observation_date)
    });
    let Some(boundary) = boundary.filter(|index| *index > 0) else {
        return Err(bad(
            RefusalCode::UndeclaredUnitBasisOrTz107,
            "observation_date must start a noninitial declared cash period",
        ));
    };
    let earlier_periods: BTreeSet<_> = periods
        .iter()
        .take(boundary)
        .map(|p| p.id.as_str())
        .collect();
    for candidate in &program.candidates {
        if stages.get(candidate.value.candidate_id.as_str()) == Some(&DecisionStage::Initial)
            && candidate
                .value
                .requires
                .iter()
                .any(|required| stages.get(required.as_str()) == Some(&DecisionStage::Recourse))
        {
            return Err(invalid(
                "an initial commitment cannot require a later recourse candidate",
            ));
        }
    }
    let gate = program
        .candidates
        .iter()
        .find(|c| c.value.candidate_id == basis.information_candidate)
        .ok_or_else(|| internal("admitted information candidate is missing"))?;
    if gate.value.cost.minor <= 0
        || gate
            .value
            .payoffs
            .iter()
            .any(|(_, payoff)| payoff.minor >= 0)
    {
        return Err(invalid(
            "the information gate needs positive committed cost and strictly negative net NAV in every state",
        ));
    }
    let mut gate_flows = Vec::new();
    for flow in &schedule.candidate_flows {
        if !earlier_periods.contains(flow.period_id.as_str()) {
            continue;
        }
        if stages.get(flow.candidate_id.as_str()) == Some(&DecisionStage::Recourse)
            && flow.value.minor != 0
        {
            return Err(invalid(
                "recourse candidates must have explicit zero cash flow before observation",
            ));
        }
        if flow.candidate_id == basis.information_candidate {
            gate_flows.push(flow);
        }
    }
    let period_order: BTreeMap<_, _> = periods
        .iter()
        .enumerate()
        .map(|(index, p)| (p.id.as_str(), index))
        .collect();
    gate_flows.sort_by(|a, b| {
        (period_order.get(a.period_id.as_str()), &a.state_id)
            .cmp(&(period_order.get(b.period_id.as_str()), &b.state_id))
    });
    let mut gate_cash: BTreeMap<&str, i128> = state_ids.iter().map(|state| (*state, 0)).collect();
    for flow in gate_flows {
        let cash = gate_cash
            .get_mut(flow.state_id.as_str())
            .ok_or_else(|| internal("admitted gate cash flow lost its shared state"))?;
        *cash = checked_minor_add(*cash, flow.value.minor)?;
    }
    if gate_cash.values().any(|cash| *cash >= 0) {
        return Err(invalid(
            "the information gate must incur strictly negative net cash before observation in every state",
        ));
    }
    admit(
        &inputs.information,
        &basis.canonical_tag(),
        AdditivityClass::NonAdditive,
    )?;
    metadata.include(&inputs.information)?;
    Ok(Admission {
        metadata,
        policy_count,
    })
}

struct Observation {
    id: String,
    probability: Rat,
}

struct Layout {
    initial_mask: usize,
    recourse_mask: usize,
    gate_bit: usize,
    observations: Vec<Observation>,
    state_observations: Vec<usize>,
}

impl Layout {
    fn new(inputs: &RecourseInputs) -> Result<Self> {
        let stages: BTreeMap<_, _> = inputs
            .information
            .value
            .candidate_stages
            .iter()
            .map(|c| (c.candidate_id.as_str(), c.stage))
            .collect();
        let (mut initial_mask, mut recourse_mask, mut gate_bit) = (0, 0, 0);
        for (index, candidate) in inputs.program.candidates.iter().enumerate() {
            let bit = 1 << index;
            match stages.get(candidate.value.candidate_id.as_str()) {
                Some(DecisionStage::Initial) => initial_mask |= bit,
                Some(DecisionStage::Recourse) => recourse_mask |= bit,
                None => return Err(internal("admitted candidate stage is missing")),
            }
            if candidate.value.candidate_id == inputs.information.value.information_candidate {
                gate_bit = bit;
            }
        }
        let state_observations: BTreeMap<_, _> = inputs
            .information
            .value
            .observations
            .iter()
            .map(|row| (row.state_id.as_str(), row.observation_id.as_str()))
            .collect();
        let mut masses = BTreeMap::new();
        for (state, weight) in &inputs.program.weights.value.states {
            let observation = state_observations
                .get(state.as_str())
                .ok_or_else(|| internal("admitted state observation is missing"))?;
            let mass = masses.entry(*observation).or_insert(ZERO);
            *mass = arithmetic(mass.checked_add(*weight))?;
        }
        let observations: Vec<_> = masses
            .into_iter()
            .map(|(id, probability)| Observation {
                id: id.into(),
                probability,
            })
            .collect();
        let indexes: BTreeMap<_, _> = observations
            .iter()
            .enumerate()
            .map(|(i, observation)| (observation.id.as_str(), i))
            .collect();
        let state_observations = inputs
            .program
            .weights
            .value
            .states
            .iter()
            .map(|(state, _)| {
                let observation = state_observations
                    .get(state.as_str())
                    .ok_or_else(|| internal("admitted state observation is missing"))?;
                indexes
                    .get(observation)
                    .copied()
                    .ok_or_else(|| internal("admitted observation index is missing"))
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(Self {
            initial_mask,
            recourse_mask,
            gate_bit,
            observations,
            state_observations,
        })
    }
}

fn rejection_state(reason: &StructuralRejection) -> Option<&str> {
    match reason {
        StructuralRejection::LiquidityShortfall { state_id, .. }
        | StructuralRejection::CashScheduleShortfall { state_id, .. }
        | StructuralRejection::NonpositiveShareCount { state_id, .. } => Some(state_id),
        StructuralRejection::BudgetExceeded
        | StructuralRejection::MissingPrerequisite { .. }
        | StructuralRejection::MutualExclusion { .. } => None,
    }
}

struct CatalogState<'a> {
    net_nav: Money,
    rejections: Vec<&'a StructuralRejection>,
    checkpoints: Vec<&'a CashScheduleCheckpoint>,
    equity: Option<&'a EquityState>,
}

struct Catalog<'a> {
    program: &'a EvaluatedProgram,
    states: Vec<Vec<CatalogState<'a>>>,
    checkpoints_per_state: usize,
}

impl<'a> Catalog<'a> {
    fn new(program: &'a EvaluatedProgram) -> Result<Self> {
        let inputs = program.inputs();
        let state_count = inputs.weights.value.states.len();
        let checkpoints_per_state = inputs
            .funding_schedule
            .as_ref()
            .ok_or_else(|| internal("admitted recourse program lost its funding schedule"))?
            .value
            .periods
            .len()
            + 1;
        let mut states = Vec::with_capacity(program.summary().choices.len());
        for (mask, choice) in program.summary().choices.iter().enumerate() {
            let schedule = choice
                .funding_schedule
                .as_ref()
                .ok_or_else(|| internal("static subset lost its complete cash checkpoints"))?;
            if schedule.checkpoints.len() != state_count * checkpoints_per_state {
                return Err(internal(
                    "static subset has an incomplete cash checkpoint matrix",
                ));
            }
            let mut general = Vec::new();
            let mut by_state: BTreeMap<&str, Vec<&StructuralRejection>> = BTreeMap::new();
            for reason in &choice.structural_rejections {
                if let Some(state) = rejection_state(reason) {
                    by_state.entry(state).or_default().push(reason);
                } else {
                    general.push(reason);
                }
            }
            let mut rows = Vec::with_capacity(state_count);
            for (state_index, (state, _)) in inputs.weights.value.states.iter().enumerate() {
                // Even a rejected policy retains its full realized NAV, so cache every subset/state.
                let mut net_nav = 0_i128;
                for (candidate_index, candidate) in inputs.candidates.iter().enumerate() {
                    if mask & (1 << candidate_index) != 0 {
                        let value = candidate
                            .value
                            .payoffs
                            .get(state_index)
                            .ok_or_else(|| internal("static candidate lost a shared-state payoff"))?
                            .1;
                        net_nav = checked_minor_add(net_nav, value.minor)?;
                    }
                }
                let mut rejections = general.clone();
                if let Some(specific) = by_state.get(state.as_str()) {
                    rejections.extend(specific.iter().copied());
                }
                let mut checkpoints = Vec::with_capacity(checkpoints_per_state);
                for checkpoint in 0..checkpoints_per_state {
                    let row = schedule
                        .checkpoints
                        .get(checkpoint * state_count + state_index)
                        .ok_or_else(|| internal("static cash checkpoint index is missing"))?;
                    if row.state_id != *state {
                        return Err(internal("static cash checkpoint state order changed"));
                    }
                    checkpoints.push(row);
                }
                let equity = choice
                    .equity
                    .as_ref()
                    .map(|assessment| {
                        let row = assessment
                            .states
                            .get(state_index)
                            .ok_or_else(|| internal("static equity state index is missing"))?;
                        if row.state_id != *state {
                            return Err(internal("static equity state order changed"));
                        }
                        Ok(row)
                    })
                    .transpose()?;
                rows.push(CatalogState {
                    net_nav: Money::from_minor(inputs.policy.value.currency, net_nav),
                    rejections,
                    checkpoints,
                    equity,
                });
            }
            states.push(rows);
        }
        Ok(Self {
            program,
            states,
            checkpoints_per_state,
        })
    }

    fn choice(&self, mask: usize) -> Result<&ProgramChoice> {
        self.program
            .summary()
            .choices
            .get(mask)
            .ok_or_else(|| internal("static subset index is missing"))
    }

    fn state(&self, mask: usize, state: usize) -> Result<&CatalogState<'a>> {
        self.states
            .get(mask)
            .and_then(|rows| rows.get(state))
            .ok_or_else(|| internal("static subset/state index is missing"))
    }
}

struct BuiltPolicy {
    choice: PolicyChoice,
    comparison: ProgramChoice,
    action: Option<Claim<ActionPayoffs>>,
}

fn build_policy(
    catalog: &Catalog<'_>,
    layout: &Layout,
    metadata: &Metadata,
    ordinal: usize,
    initial_mask: usize,
    continuations: &[usize],
) -> Result<BuiltPolicy> {
    let inputs = catalog.program.inputs();
    let currency = inputs.policy.value.currency;
    let money = |major| ExactMoney { currency, major };
    let acquired = initial_mask & layout.gate_bit != 0;
    if continuations.len()
        != if acquired {
            layout.observations.len()
        } else {
            1
        }
    {
        return Err(internal(
            "policy does not cover exactly its available observations",
        ));
    }
    let initial = catalog.choice(initial_mask)?;
    let mut branches = Vec::with_capacity(continuations.len());
    for (index, mask) in continuations.iter().enumerate() {
        let continuation = catalog.choice(*mask)?;
        let (observation_id, probability) = if acquired {
            let observation = layout
                .observations
                .get(index)
                .ok_or_else(|| internal("policy observation index is missing"))?;
            (Some(observation.id.clone()), observation.probability)
        } else {
            (None, ONE)
        };
        branches.push(PolicyBranch {
            observation_id,
            probability,
            selected: continuation.selected.clone(),
            spend: continuation.spend,
        });
    }
    let state_count = inputs.weights.value.states.len();
    let mut states = Vec::with_capacity(state_count);
    let mut masks = Vec::with_capacity(state_count);
    let mut structural_rejections = Vec::new();
    let mut equity = inputs.equity.as_ref().map(|_| EquityAssessment {
        states: Vec::with_capacity(state_count),
    });
    let mut values = Vec::with_capacity(state_count);
    for (state_index, (state, _)) in inputs.weights.value.states.iter().enumerate() {
        let observation_index = if acquired {
            *layout
                .state_observations
                .get(state_index)
                .ok_or_else(|| internal("policy state-observation index is missing"))?
        } else {
            0
        };
        let continuation = *continuations
            .get(observation_index)
            .ok_or_else(|| internal("policy continuation index is missing"))?;
        let mask = initial_mask | continuation;
        let source_choice = catalog.choice(mask)?;
        let source = catalog.state(mask, state_index)?;
        let observation_id = branches
            .get(observation_index)
            .ok_or_else(|| internal("policy branch index is missing"))?
            .observation_id
            .clone();
        masks.push(mask);
        values.push((state.clone(), source.net_nav));
        states.push(PolicyState {
            state_id: state.clone(),
            observation_id,
            bundle_id: source_choice.bundle_id.clone(),
            selected: source_choice.selected.clone(),
            spend: source_choice.spend,
            budget_slack: source_choice.budget_slack,
            net_nav: money(amount(source.net_nav)?),
        });
        structural_rejections.extend(source.rejections.iter().map(|reason| PolicyRejection {
            state_id: state.clone(),
            reason: (*reason).clone(),
        }));
        if let Some(assessment) = &mut equity {
            assessment.states.push(
                source
                    .equity
                    .ok_or_else(|| internal("policy lost a declared equity state"))?
                    .clone(),
            );
        }
    }
    let mut checkpoints = Vec::with_capacity(state_count * catalog.checkpoints_per_state);
    let mut minimum_slack = None;
    for checkpoint_index in 0..catalog.checkpoints_per_state {
        for (state_index, mask) in masks.iter().enumerate() {
            let source = catalog.state(*mask, state_index)?;
            let checkpoint = *source
                .checkpoints
                .get(checkpoint_index)
                .ok_or_else(|| internal("policy cash checkpoint index is missing"))?;
            let slack = checkpoint.slack.major;
            if minimum_slack.is_none_or(|old| exact_compare(slack, old).is_lt()) {
                minimum_slack = Some(slack);
            }
            checkpoints.push(checkpoint.clone());
        }
    }
    let funding_schedule = FundingScheduleAssessment {
        checkpoints,
        minimum_slack: money(
            minimum_slack.ok_or_else(|| internal("policy lacks cash checkpoints"))?,
        ),
    };
    let policy_id = format!("policy_{ordinal:03x}");
    let is_hold = states.iter().all(|state| state.selected.is_empty());
    let action = if structural_rejections.is_empty() {
        let payoff = ActionPayoffs {
            model_key: inputs.policy.value.model_key.clone(),
            action_id: policy_id.clone(),
            label: if is_hold {
                "Do nothing".into()
            } else {
                policy_id.clone()
            },
            role: if is_hold {
                ActionRole::DoNothing
            } else {
                ActionRole::Action
            },
            currency,
            valuation_basis: inputs.policy.value.valuation_basis.clone(),
            values,
        };
        let tag = payoff.canonical_tag();
        Some(metadata.claim(
            "capital.recourse-policy.net-delta-nav/1",
            payoff,
            &tag,
            AdditivityClass::ExtensiveAdditive,
            true,
        ))
    } else {
        None
    };
    // These private rows feed only the existing statistics kernel. Published branch spending and
    // selections live in PolicyChoice; no union of mutually exclusive paths is presented as a subset.
    let comparison = ProgramChoice {
        bundle_id: policy_id.clone(),
        selected: if is_hold {
            vec![]
        } else {
            vec![policy_id.clone()]
        },
        spend: initial.spend,
        budget_slack: initial.budget_slack,
        funding: None,
        funding_schedule: None,
        equity: equity.clone(),
        structural_rejections: structural_rejections
            .iter()
            .map(|r| r.reason.clone())
            .collect(),
        statistics: None,
        tail_loss: None,
        per_share_statistics: None,
        per_share_tail_loss: None,
        risk_rejected: false,
    };
    let choice = PolicyChoice {
        policy_id,
        initial_selected: initial.selected.clone(),
        initial_spend: initial.spend,
        information_acquired: acquired,
        branches,
        states,
        structural_rejections,
        funding_schedule,
        equity,
        statistics: None,
        tail_loss: None,
        per_share_statistics: None,
        per_share_tail_loss: None,
        risk_rejected: false,
    };
    Ok(BuiltPolicy {
        choice,
        comparison,
        action,
    })
}

fn difference(best: Option<Rat>, benchmark: Option<Rat>) -> Result<Option<Rat>> {
    match (best, benchmark) {
        (Some(best), Some(benchmark)) => {
            let value = arithmetic(best.checked_sub(benchmark))?;
            if value.signum() < 0 {
                return Err(internal(
                    "a nested comparison population produced a negative value difference",
                ));
            }
            Ok(Some(value))
        }
        _ => Ok(None),
    }
}

fn objective_units(
    value: Option<Rat>,
    currency: Currency,
    per_share: bool,
) -> (Option<ExactMoney>, Option<ExactPerShare>) {
    if per_share {
        (None, value.map(|major| ExactPerShare { currency, major }))
    } else {
        (value.map(|major| ExactMoney { currency, major }), None)
    }
}

/// Enumerates every admitted two-stage policy, preserving every infeasible policy and realized path.
/// Observation classes restrict decisions, while the complete unconditional state vector determines risk.
pub fn evaluate(inputs: &RecourseInputs) -> Result<EvaluatedRecourseProgram> {
    let Admission {
        metadata,
        policy_count,
    } = admit_inputs(inputs)?;
    let static_program = super::evaluate(&inputs.program)?;
    let mut information = inputs.information.clone();
    information
        .value
        .candidate_stages
        .sort_by(|a, b| a.candidate_id.cmp(&b.candidate_id));
    information
        .value
        .observations
        .sort_by(|a, b| a.state_id.cmp(&b.state_id));
    let inputs = RecourseInputs {
        program: static_program.inputs().clone(),
        information,
    };
    let layout = Layout::new(&inputs)?;
    let catalog = Catalog::new(&static_program)?;
    let subsets = 1_usize << inputs.program.candidates.len();
    let continuations: Vec<_> = (0..subsets)
        .filter(|mask| mask & !layout.recourse_mask == 0)
        .collect();
    let mut policies = Vec::with_capacity(policy_count);
    let mut comparisons = Vec::with_capacity(policy_count);
    let mut actions = Vec::with_capacity(policy_count);
    for initial_mask in (0..subsets).filter(|mask| mask & !layout.initial_mask == 0) {
        let branch_count = if initial_mask & layout.gate_bit != 0 {
            layout.observations.len()
        } else {
            1
        };
        let combinations = continuations
            .len()
            .checked_pow(branch_count as u32)
            .ok_or_else(|| internal("admitted continuation count overflowed"))?;
        for code in 0..combinations {
            let mut branches = Vec::with_capacity(branch_count);
            let mut divisor = combinations;
            for _ in 0..branch_count {
                divisor /= continuations.len();
                let index = (code / divisor) % continuations.len();
                branches.push(
                    *continuations
                        .get(index)
                        .ok_or_else(|| internal("admitted continuation assignment is missing"))?,
                );
            }
            let BuiltPolicy {
                choice,
                comparison,
                action,
            } = build_policy(
                &catalog,
                &layout,
                &metadata,
                policies.len(),
                initial_mask,
                &branches,
            )?;
            policies.push(choice);
            comparisons.push(comparison);
            if let Some(action) = action {
                actions.push(action);
            }
        }
    }
    if policies.len() != policy_count {
        return Err(internal(
            "enumeration did not preserve the complete declared policy population",
        ));
    }
    super::populate_statistics(&mut comparisons, &actions, &inputs.program)?;
    let per_share_objective = inputs.program.equity.is_some();
    let (unconstrained_winners, before) = super::winners(&comparisons, false, per_share_objective)?;
    let (feasible_winners, after) = super::winners(&comparisons, true, per_share_objective)?;
    let (_, static_before) = super::winners(
        &static_program.summary().choices,
        false,
        per_share_objective,
    )?;
    let (_, static_after) =
        super::winners(&static_program.summary().choices, true, per_share_objective)?;
    let currency = inputs.program.policy.value.currency;
    let (expected_value_cost_of_risk_limit, expected_per_share_cost_of_risk_limit) =
        objective_units(difference(before, after)?, currency, per_share_objective);
    let (net_adaptive_value, net_adaptive_value_per_share) = objective_units(
        difference(after, static_after)?,
        currency,
        per_share_objective,
    );
    let (unconstrained_net_adaptive_value, unconstrained_net_adaptive_value_per_share) =
        objective_units(
            difference(before, static_before)?,
            currency,
            per_share_objective,
        );
    for (policy, comparison) in policies.iter_mut().zip(comparisons) {
        policy.statistics = comparison.statistics;
        policy.tail_loss = comparison.tail_loss;
        policy.per_share_statistics = comparison.per_share_statistics;
        policy.per_share_tail_loss = comparison.per_share_tail_loss;
        policy.risk_rejected = comparison.risk_rejected;
    }
    let summary = RecourseSummary {
        model_key: inputs.program.policy.value.model_key.clone(),
        question: inputs.program.weights.value.question.clone(),
        currency,
        information: inputs.information.value.clone(),
        per_share_objective,
        policies,
        unconstrained_winners,
        feasible_winners,
        expected_value_cost_of_risk_limit,
        expected_per_share_cost_of_risk_limit,
        net_adaptive_value,
        net_adaptive_value_per_share,
        unconstrained_net_adaptive_value,
        unconstrained_net_adaptive_value_per_share,
    };
    let mut binding = Encoder::new_frame(*b"CAPR", 1);
    binding.put_str(static_program.digest());
    binding.put_str(&static_program.to_json());
    binding.put_str(&super::render::claim_json(
        &inputs.information,
        &render::basis_json(&inputs.information.value),
    ));
    binding.put_str(&render::summary_json(&summary));
    binding.put_str(NO_CLAIM);
    let digest = sha256(&binding.finish()).to_hex();
    let tag = format!("capital-recourse/v1:{digest}");
    let claim = metadata.claim(
        "capital.two-stage-program/1",
        summary,
        &tag,
        AdditivityClass::NonAdditive,
        false,
    );
    Ok(EvaluatedRecourseProgram {
        inputs,
        static_program,
        claim,
        digest,
    })
}
