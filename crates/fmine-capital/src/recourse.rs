//! DESIGN (BD-229, §23): bounded two-stage policies over explicitly declared information sets.
//! Determinism: Exact (static subsets remain the complete independent comparison population).

use super::*;
use crate::entity::valuation::EntityValuationAssessment;

mod render;
#[cfg(test)]
mod tests;

pub const MAX_OBSERVATIONS: usize = 4;
pub const MAX_POLICIES: usize = 256;
pub const ENTITY_CAPITAL_CALL_NO_CLAIM: &str = "Exact bounded two-stage comparison with explicitly declared pre-money subscription calls, entity cash and independent already-net NAV inputs. A policy responds only to purchased observations. Initial commitments and their future capital calls persist after abandonment; a recourse candidate cannot activate a call before observation. Every active call debits its full planned parent funding, credits parent plus outside cash to the JV and updates ownership using the declared pre-money value and explicit sequence. Updated shares govern current and later dividends, without inferring terminal NAV or equity share counts. Calls settle before modeled debt service and pre-distribution floors. Underfunded calls and unaffordable parent contributions remain structural failures; inactive and rejected call ledgers remain visible. Only each realized subset's same-state cash, ownership, debt and rejection rows enter a policy. Optional debt and every declared valuation constraint remain binding. Risk uses the complete unconditional policy; net adaptive value is separate from the static comparison and is not added to NAV again. Inputs remain uncalibrated assumptions; no actual JV agreement, penalty dilution, automatic top-up, subscription rollback, transaction reconciliation, further decision stages, intra-phase liquidity guarantee, historical validation, regulatory claim or execution authorization is supplied.";
pub const ENTITY_VALUATION_NO_CLAIM: &str = "Exact bounded two-stage comparison of complete declared entity NAV and share-count matrices on one shared scenario index. The primary equity entity supplies the sole optimization objective; each additional entity retains its own valuation basis, actual-share denominator, statistics and optional signed tail-loss ceiling. All entities require positive ending shares on every realized path. Risk limits are applied jointly to each complete unconditional policy distribution; entity NAV/share values are never added, averaged or inferred from cash ownership. Initial NAV and share commitments survive every continuation, including abandonment. Only a realized subset's same-state ledgers and structural failures enter that policy; a static subset's whole-distribution risk result is not inherited. Dated funding, entity cash and any declared debt constraints remain binding, and cash movements are not charged to already-net NAV again. Decisions respond only to purchased declared observations. Net adaptive value compares the same primary objective under the same joint limits and remains separate from NAV. Inputs remain uncalibrated assumptions; no actual deal terms, automatic contractual ownership dilution, further decision stages, historical validation, selection frequency, regulatory claim or execution authorization is inferred.";
pub const NO_CLAIM: &str = "Exact bounded two-stage comparison conditional on the declared shared scenarios, paid information gate, observation classes, stage assignments, dated cash flows and net NAV inputs. Initial commitments and their full declared effects are retained in every continuation. A policy can respond only to the declared observation after purchasing the information gate; otherwise one common continuation applies to every state. Risk is evaluated over each complete unconditional policy distribution. Net adaptive value is reported separately against the best static subset under the same objective and risk constraint; it is already included in policy outcomes and must not be added again. Cash checks cover only opening and declared period-end checkpoints. Input weights and information assumptions are uncalibrated. No further stages, implicit information, intra-period payment ordering, entity waterfalls, execution authorization, regulatory claim, selection-frequency estimate or historical validation is supplied.";
pub const ENTITY_NO_CLAIM: &str = "Exact bounded two-stage comparison conditional on declared shared scenarios, paid information, observation classes, stage assignments and net NAV for the objective entity. Initial commitments and all their effects remain in every continuation. Decisions depend only on purchased declared observations; global risk uses the complete unconditional policy. Entity cash follows explicit native flows, fixed funding transfers and gated child-first ownership distributions, with outside shareholders' cash leaving the modeled graph. Floors are checked at initial opening, before distributions and at closing; later receipts never erase earlier breaches. A paid information gate must incur negative net native external cash before observation; an internal transfer alone is not payment. No partner top-up, dilution, debt-service covenant, fiscal rule, permission or second valuation denominator is inferred. Net adaptive value is separate from the full static comparison and must not be added to NAV again. Inputs remain uncalibrated assumptions. No further stages, intra-phase payment ordering, execution authorization, regulatory claim, selection frequency or historical validation is supplied.";
pub const ENTITY_DEBT_NO_CLAIM: &str = "Exact bounded two-stage comparison conditional on declared shared scenarios, paid information, observation classes, stage assignments, objective-entity NAV and external senior debt terms. Initial commitments and later debt obligations remain in every continuation, including abandonment. Recourse debt draws, scheduled-principal increments and CFADS increments are zero before observation. Coverage derives from separately declared CFADS and mandatory service; it excludes principal sweeps and never creates cash. Period rates apply to opening principal plus draws, full mandatory service precedes cash-floor checks, and each entity sweeps cash above its floor before its child-first ownership distribution. Zero-service distribution permission is explicit. Only the realized subset's same-state debt and cash ledgers enter a policy. Earlier funding failures remain binding; cash and principal conservation are exact. Paid information consumes native external cash; internal transfers and CFADS alone do not pay for it. Risk uses the complete unconditional policy and net adaptive value remains separate from the full static comparison. Debt cash is not charged to already-net NAV again. Inputs remain uncalibrated assumptions; no actual lending agreement, annualization, arrears, default remedy, waiver, partner top-up, ownership dilution, additional decision stages, second valuation denominator, historical validation, selection frequency, regulatory claim or execution authorization is inferred.";

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
    pub funding_schedule: Option<FundingScheduleAssessment>,
    pub entity_funding: Option<EntityFundingAssessment>,
    pub equity: Option<EquityAssessment>,
    pub entity_valuations: Vec<EntityValuationAssessment>,
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
    pub selection_stability: Option<stability::SelectionStabilitySummary>,
    pub robustness: Option<robustness::RobustnessSummary>,
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
        no_claim_for(&self.inputs)
    }
}

fn no_claim_for(inputs: &RecourseInputs) -> &'static str {
    if inputs.program.selection_stability.is_some() {
        return stability::NO_CLAIM;
    }
    if inputs
        .program
        .entity_funding
        .as_ref()
        .is_some_and(|claim| claim.value.capital_calls.is_some())
    {
        return ENTITY_CAPITAL_CALL_NO_CLAIM;
    }
    if !inputs.program.entity_valuations.is_empty() {
        return ENTITY_VALUATION_NO_CLAIM;
    }
    if inputs
        .program
        .entity_funding
        .as_ref()
        .is_some_and(|claim| claim.value.debt.is_some())
    {
        ENTITY_DEBT_NO_CLAIM
    } else if inputs.program.entity_funding.is_some() {
        ENTITY_NO_CLAIM
    } else {
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
    transport: Option<TransportPlan>,
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
    let schedule = program.funding_schedule.as_ref().map(|claim| &claim.value);
    let entities = program.entity_funding.as_ref().map(|claim| &claim.value);
    let period_count = match (schedule, entities) {
        (Some(schedule), None) => schedule.periods.len(),
        (None, Some(entities)) => entities.periods.len(),
        _ => {
            return Err(bad(
                RefusalCode::NoClaimInDomain201,
                "two-stage programs require exactly one dated funding schedule or entity funding declaration",
            ));
        }
    };
    if !(2..=MAX_CASH_PERIODS).contains(&period_count) {
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
    let work_per_state = match entities {
        Some(funding) => entity::work_per_state(funding)?,
        None => period_count + 1,
    };
    let checkpoints = (1_usize << candidate_count)
        .checked_add(policy_count)
        .and_then(|count| count.checked_mul(state_count))
        .and_then(|count| count.checked_mul(work_per_state));
    if policy_count > MAX_POLICIES || checkpoints.is_none_or(|count| count > MAX_CASH_CHECKPOINTS) {
        return Err(bad(
            RefusalCode::NoClaimInDomain201,
            "two-stage programs admit at most 256 policies and 65536 combined static/policy cash ledger rows",
        ));
    }
    entity::valuation::check_work_bound(program, (1_usize << candidate_count) + policy_count)?;
    stability::check_work(program, (1_usize << candidate_count) + policy_count)?;
    robustness::check_population_work(program, (1_usize << candidate_count) + policy_count)?;
    let (mut metadata, transport) = super::validate(program)?;
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
    let mut periods: Vec<_> = match (schedule, entities) {
        (Some(schedule), None) => schedule
            .periods
            .iter()
            .map(|period| (period.id.as_str(), period.cash_period.as_str()))
            .collect(),
        (None, Some(entities)) => entities
            .periods
            .iter()
            .map(|period| (period.id.as_str(), period.cash_period.as_str()))
            .collect(),
        _ => return Err(internal("admitted recourse funding mode changed")),
    };
    periods.sort_by(|a, b| (a.1, a.0).cmp(&(b.1, b.0)));
    let boundary = periods.iter().position(|period| {
        period_dates(period.1).is_some_and(|(start, _)| start == basis.observation_date)
    });
    let Some(boundary) = boundary.filter(|index| *index > 0) else {
        return Err(bad(
            RefusalCode::UndeclaredUnitBasisOrTz107,
            "observation_date must start a noninitial declared cash period",
        ));
    };
    let earlier_periods: BTreeSet<_> = periods.iter().take(boundary).map(|p| p.0).collect();
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
    // Internal transfers conserve group cash. Only native external flows establish a paid gate.
    let native_flows: Vec<_> = match (schedule, entities) {
        (Some(schedule), None) => schedule
            .candidate_flows
            .iter()
            .map(|flow| {
                (
                    flow.period_id.as_str(),
                    "",
                    flow.candidate_id.as_str(),
                    flow.state_id.as_str(),
                    flow.value,
                )
            })
            .collect(),
        (None, Some(entities)) => entities
            .candidate_flows
            .iter()
            .map(|flow| {
                (
                    flow.period_id.as_str(),
                    flow.entity_id.as_str(),
                    flow.candidate_id.as_str(),
                    flow.state_id.as_str(),
                    flow.value,
                )
            })
            .collect(),
        _ => return Err(internal("admitted recourse funding mode changed")),
    };
    let mut gate_flows = Vec::new();
    for (period_id, entity_id, candidate_id, state_id, value) in native_flows {
        if !earlier_periods.contains(period_id) {
            continue;
        }
        if stages.get(candidate_id) == Some(&DecisionStage::Recourse) && value.minor != 0 {
            return Err(invalid(
                "recourse candidates must have explicit zero cash flow before observation",
            ));
        }
        if candidate_id == basis.information_candidate {
            gate_flows.push((period_id, entity_id, state_id, value));
        }
    }
    if let Some(entities) = entities {
        if let Some(calls) = &entities.capital_calls {
            for call in &calls.calls {
                if earlier_periods.contains(call.period_id.as_str())
                    && call.candidate_id.as_deref().is_some_and(|candidate| {
                        stages.get(candidate) == Some(&DecisionStage::Recourse)
                    })
                {
                    return Err(invalid(
                        "a recourse candidate cannot activate a capital call before observation",
                    ));
                }
            }
        }
        for transfer in &entities.transfers {
            if earlier_periods.contains(transfer.period_id.as_str())
                && transfer.candidate_id.as_deref().is_some_and(|candidate| {
                    stages.get(candidate) == Some(&DecisionStage::Recourse)
                })
                && transfer.amounts.iter().any(|(_, amount)| amount.minor != 0)
            {
                return Err(invalid(
                    "recourse candidates must have explicit zero entity transfers before observation",
                ));
            }
        }
        if let Some(debt) = &entities.debt {
            for flow in &debt.candidate_flows {
                if earlier_periods.contains(flow.period_id.as_str())
                    && stages.get(flow.candidate_id.as_str()) == Some(&DecisionStage::Recourse)
                    && (flow.draw.minor != 0
                        || flow.principal_payment_cap.minor != 0
                        || flow.cfads.minor != 0)
                {
                    return Err(invalid(
                        "recourse debt draws, principal payment caps and CFADS increments must each be explicitly zero before observation",
                    ));
                }
            }
        }
    }
    let period_order: BTreeMap<_, _> = periods
        .iter()
        .enumerate()
        .map(|(index, p)| (p.0, index))
        .collect();
    gate_flows
        .sort_by(|a, b| (period_order.get(a.0), a.1, a.2).cmp(&(period_order.get(b.0), b.1, b.2)));
    let mut gate_cash: BTreeMap<&str, i128> = state_ids.iter().map(|state| (*state, 0)).collect();
    for (_, _, state_id, value) in gate_flows {
        let cash = gate_cash
            .get_mut(state_id)
            .ok_or_else(|| internal("admitted gate cash flow lost its shared state"))?;
        *cash = checked_minor_add(*cash, value.minor)?;
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
        transport,
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
        | StructuralRejection::EntityCashShortfall { state_id, .. }
        | StructuralRejection::EntityCapitalCallShortfall { state_id, .. }
        | StructuralRejection::NonpositiveShareCount { state_id, .. }
        | StructuralRejection::EntityNonpositiveShareCount { state_id, .. } => Some(state_id),
        StructuralRejection::BudgetExceeded
        | StructuralRejection::MissingPrerequisite { .. }
        | StructuralRejection::MutualExclusion { .. } => None,
    }
}

struct CatalogState<'a> {
    net_nav: Money,
    rejections: Vec<&'a StructuralRejection>,
    checkpoints: Vec<&'a CashScheduleCheckpoint>,
    entity_funding: Option<&'a entity::EntityFundingState>,
    equity: Option<&'a EquityState>,
    entity_valuations: Vec<&'a EquityState>,
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
            .map_or(0, |claim| claim.value.periods.len() + 1);
        if checkpoints_per_state == 0 && inputs.entity_funding.is_none() {
            return Err(internal("admitted recourse program lost its dated funding"));
        }
        let mut states = Vec::with_capacity(program.summary().choices.len());
        for (mask, choice) in program.summary().choices.iter().enumerate() {
            let schedule = choice.funding_schedule.as_ref();
            if schedule.map_or(0, |assessment| assessment.checkpoints.len())
                != state_count * checkpoints_per_state
                || choice
                    .entity_funding
                    .as_ref()
                    .map_or(0, |assessment| assessment.states.len())
                    != if inputs.entity_funding.is_some() {
                        state_count
                    } else {
                        0
                    }
            {
                return Err(internal(
                    "static subset has an incomplete cash checkpoint matrix",
                ));
            }
            if choice.entity_valuations.len() != inputs.entity_valuations.len() {
                return Err(internal("static subset lost an entity valuation"));
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
                        .ok_or_else(|| internal("static subset lost its cash schedule"))?
                        .checkpoints
                        .get(checkpoint * state_count + state_index)
                        .ok_or_else(|| internal("static cash checkpoint index is missing"))?;
                    if row.state_id != *state {
                        return Err(internal("static cash checkpoint state order changed"));
                    }
                    checkpoints.push(row);
                }
                let entity_funding = choice
                    .entity_funding
                    .as_ref()
                    .map(|assessment| {
                        let row = assessment.states.get(state_index).ok_or_else(|| {
                            internal("static entity funding state index is missing")
                        })?;
                        if row.state_id != *state {
                            return Err(internal("static entity funding state order changed"));
                        }
                        Ok(row)
                    })
                    .transpose()?;
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
                let mut entity_valuations = Vec::with_capacity(inputs.entity_valuations.len());
                for (assessment, declaration) in choice
                    .entity_valuations
                    .iter()
                    .zip(&inputs.entity_valuations)
                {
                    if assessment.entity != declaration.value.equity.entity
                        || assessment.share_class != declaration.value.equity.share_class
                        || assessment.equity.states.len() != state_count
                    {
                        return Err(internal("static entity valuation population changed"));
                    }
                    let row = assessment.equity.states.get(state_index).ok_or_else(|| {
                        internal("static entity valuation state index is missing")
                    })?;
                    if row.state_id != *state {
                        return Err(internal("static entity valuation state order changed"));
                    }
                    entity_valuations.push(row);
                }
                rows.push(CatalogState {
                    net_nav: Money::from_minor(inputs.policy.value.currency, net_nav),
                    rejections,
                    checkpoints,
                    entity_funding,
                    equity,
                    entity_valuations,
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
    let mut entity_valuations: Vec<_> = inputs
        .entity_valuations
        .iter()
        .map(|claim| EntityValuationAssessment {
            entity: claim.value.equity.entity.clone(),
            share_class: claim.value.equity.share_class.clone(),
            equity: EquityAssessment {
                states: Vec::with_capacity(state_count),
            },
            per_share_statistics: None,
            per_share_tail_loss: None,
            max_tail_loss_per_share: claim.value.equity.max_tail_loss_per_share.map(|major| {
                ExactPerShare {
                    currency: claim.value.equity.currency,
                    major,
                }
            }),
            risk_rejected: false,
        })
        .collect();
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
        for (assessment, state) in entity_valuations.iter_mut().zip(&source.entity_valuations) {
            assessment.equity.states.push((*state).clone());
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
    let funding_schedule = if inputs.funding_schedule.is_some() {
        Some(FundingScheduleAssessment {
            checkpoints,
            minimum_slack: money(
                minimum_slack.ok_or_else(|| internal("policy lacks cash checkpoints"))?,
            ),
        })
    } else {
        None
    };
    let entity_funding = if inputs.entity_funding.is_some() {
        let mut states = Vec::with_capacity(state_count);
        let mut minimum_slack = None;
        for (state_index, mask) in masks.iter().enumerate() {
            let source = catalog
                .state(*mask, state_index)?
                .entity_funding
                .ok_or_else(|| internal("policy lost a declared entity funding state"))?;
            let slack = source.minimum_slack.major;
            if minimum_slack.is_none_or(|old| exact_compare(slack, old).is_lt()) {
                minimum_slack = Some(slack);
            }
            states.push(source.clone());
        }
        Some(EntityFundingAssessment {
            states,
            minimum_slack: money(
                minimum_slack.ok_or_else(|| internal("policy lacks entity cash checkpoints"))?,
            ),
        })
    } else {
        None
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
        entity_funding: None,
        equity: equity.clone(),
        entity_valuations: entity_valuations.clone(),
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
        entity_funding,
        equity,
        entity_valuations,
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
        transport,
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
    let selection_stability = stability::assess(
        &inputs.program,
        &comparisons,
        Some(&policies),
        &metadata,
        &feasible_winners,
    )?;
    let robustness = robustness::assess_policies(
        &inputs.program,
        &comparisons,
        &feasible_winners,
        transport.as_ref(),
    )?;
    for (policy, comparison) in policies.iter_mut().zip(comparisons) {
        policy.statistics = comparison.statistics;
        policy.tail_loss = comparison.tail_loss;
        policy.per_share_statistics = comparison.per_share_statistics;
        policy.per_share_tail_loss = comparison.per_share_tail_loss;
        policy.entity_valuations = comparison.entity_valuations;
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
        selection_stability,
        robustness,
    };
    let mut binding = Encoder::new_frame(*b"CAPR", 1);
    binding.put_str(static_program.digest());
    binding.put_str(&static_program.to_json());
    binding.put_str(&super::render::claim_json(
        &inputs.information,
        &render::basis_json(&inputs.information.value),
    ));
    binding.put_str(&render::summary_json(&summary));
    binding.put_str(no_claim_for(&inputs));
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
