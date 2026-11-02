//! DESIGN (BD-229, §23): bounded exact static capital selection on a declared shared scenario index.
//! Determinism: Exact (inherited floating evidence metadata is bound by its exact bits).
//!
//! Spending constrains the budget; the supplied additive delta-NAV payoffs already include capital costs.
//! Neither marginal CVaRs nor individually optimal projects are combined into a portfolio risk estimate.

use std::collections::{BTreeMap, BTreeSet};

use fmine_claim::{
    AdditivityClass, Claim, Confidentiality, Representation, SpatialSupport, UuTerm,
};
use fmine_core::ObjectId;
use fmine_core::anchor::SourceAnchor;
use fmine_core::codec::Encoder;
use fmine_core::hash::sha256;
use fmine_decide::card::{
    self, ActionPayoffs, ActionRole, AlternativeStats, DecisionInputs, ExactMoney, StateResult,
    WeightSet,
};
use fmine_evidence::color::validate_color_payload;
use fmine_evidence::{EpistemicStatus, EvidenceClass, KnowledgeState, Refusal, RefusalCode};
use fmine_numerics::exact_feas::Rat;
use fmine_units::{Currency, Money, UnitsError};

pub mod model;
mod render;

pub const MAX_CANDIDATES: usize = 8;
pub const MAX_STATES: usize = 256;
pub const MAX_TEXT_BYTES: usize = 4096;
pub const MAX_IDENTIFIER_BYTES: usize = 128;
pub const PAYOFF_CONVENTION: &str = "additive_net_delta_nav";
pub const NO_CLAIM: &str = "Static comparison conditional on the declared complete shared-scenario model, additive net delta-NAV payoffs, spending constraints and reference-class adjustment bases. Input weights are uncalibrated assumptions. The selected set is optimal only among the enumerated declared subsets. No project interactions, staged recourse, entity waterfalls, variable share counts, physical feasibility, retrospective evidence, selection-frequency estimate, regulatory claim or execution authorization is supplied. Costs constrain spending and are not deducted again from net delta-NAV.";
const ZERO: Rat = Rat::from_integer(0);
const ONE: Rat = Rat::from_integer(1);
type Result<T> = std::result::Result<T, Refusal>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramPolicy {
    pub model_key: String,
    pub currency: Currency,
    pub valuation_basis: String,
    pub budget: Money,
    /// Upper tail mass of signed loss = -delta NAV; 1/5 means worst 20%.
    pub tail_mass: Rat,
    pub max_tail_loss: Option<Money>,
    pub payoff_convention: String,
    pub reference_class_basis: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapitalCandidate {
    pub model_key: String,
    pub candidate_id: String,
    pub label: String,
    pub currency: Currency,
    pub valuation_basis: String,
    pub cost: Money,
    pub payoffs: Vec<(String, Money)>,
    pub requires: Vec<String>,
    pub exclusion_group: Option<String>,
    pub reference_class_basis: String,
}

impl ProgramPolicy {
    #[must_use]
    pub fn canonical_tag(&self) -> String {
        render::value_tag("capital-policy/v1", &render::policy_json(self))
    }
}
impl CapitalCandidate {
    #[must_use]
    pub fn canonical_tag(&self) -> String {
        render::value_tag("capital-candidate/v1", &render::candidate_json(self))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProgramInputs {
    pub policy: Claim<ProgramPolicy>,
    pub weights: Claim<WeightSet>,
    pub candidates: Vec<Claim<CapitalCandidate>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StructuralRejection {
    BudgetExceeded,
    MissingPrerequisite {
        candidate: String,
        prerequisite: String,
    },
    MutualExclusion {
        group: String,
        candidates: Vec<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramChoice {
    pub bundle_id: String,
    pub selected: Vec<String>,
    pub spend: ExactMoney,
    pub budget_slack: ExactMoney,
    pub structural_rejections: Vec<StructuralRejection>,
    /// None for a structurally rejected subset; no rejected subset is silently removed.
    pub statistics: Option<AlternativeStats>,
    pub tail_loss: Option<ExactMoney>,
    pub risk_rejected: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramSummary {
    pub model_key: String,
    pub question: String,
    pub policy: ProgramPolicy,
    pub choices: Vec<ProgramChoice>,
    /// Budget, prerequisites and exclusions applied, before the optional tail-loss limit.
    pub unconstrained_winners: Vec<String>,
    pub feasible_winners: Vec<String>,
    /// Absent when no subset meets the risk limit; negative values cannot occur.
    pub expected_value_cost_of_risk_limit: Option<ExactMoney>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EvaluatedProgram {
    inputs: ProgramInputs,
    claim: Claim<ProgramSummary>,
    digest: String,
}
impl EvaluatedProgram {
    #[must_use]
    pub fn inputs(&self) -> &ProgramInputs {
        &self.inputs
    }
    #[must_use]
    pub fn summary(&self) -> &ProgramSummary {
        &self.claim.value
    }
    #[must_use]
    pub fn claim(&self) -> &Claim<ProgramSummary> {
        &self.claim
    }
    #[must_use]
    pub fn digest(&self) -> &str {
        &self.digest
    }
}

fn bad(code: RefusalCode, why: impl Into<String>) -> Refusal {
    Refusal::new(code, format!("capital program: {}", why.into()))
}
fn arithmetic<T>(value: std::result::Result<T, UnitsError>) -> Result<T> {
    value.map_err(|e| bad(RefusalCode::NumericalCertificateFailed207, e.to_string()))
}
fn named(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_TEXT_BYTES
        && value.trim() == value
        && !value.chars().any(char::is_control)
}
fn key(value: &str) -> bool {
    named(value)
        && value.len() <= MAX_IDENTIFIER_BYTES
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'.'))
}
fn amount(value: Money) -> Result<Rat> {
    arithmetic(Rat::new(value.minor, Money::SCALE))
}
fn checked_minor_add(a: i128, b: i128) -> Result<i128> {
    a.checked_add(b).ok_or_else(|| {
        bad(
            RefusalCode::NumericalCertificateFailed207,
            "monetary sum overflowed",
        )
    })
}

fn admit<T>(claim: &Claim<T>, tag: &str, additivity: AdditivityClass) -> Result<()> {
    claim.check_id()?;
    if claim.value_tag() != tag {
        return Err(bad(
            RefusalCode::FrozenRecordMutation507,
            "typed input differs from its canonical value tag",
        ));
    }
    if claim.stale || claim.state == KnowledgeState::Stale {
        return Err(bad(RefusalCode::StaleAnchor502, "stale input"));
    }
    if !matches!(
        claim.state,
        KnowledgeState::Known | KnowledgeState::Estimated
    ) {
        return Err(bad(
            RefusalCode::NotObservable202,
            "input state is unusable",
        ));
    }
    if claim.representation != Representation::Deterministic
        || claim.support != SpatialSupport::PeriodAggregate
        || claim.additivity != additivity
    {
        return Err(bad(
            RefusalCode::SupportMismatch102,
            "inputs require deterministic period aggregates with explicit monetary or non-additive support",
        ));
    }
    if !named(&claim.metric_key)
        || claim.anchors.is_empty()
        || claim
            .anchors
            .iter()
            .any(|a| !named(&a.artifact_path) || !named(&a.locator) || a.artifact_sha256 == [0; 32])
    {
        return Err(bad(
            RefusalCode::NoClaimInDomain201,
            "every input needs a named Claim and source digest/locator",
        ));
    }
    claim.verify_uu_slot_for_decision()?;
    if claim.uu_terms.is_empty()
        || claim.uu_terms.iter().any(|u| {
            !key(&u.scenario_id)
                || !named(&u.description)
                || !named(&u.trigger_indicator)
                || !u.downside_exposure_usd_m.is_finite()
                || u.downside_exposure_usd_m <= 0.0
        })
    {
        return Err(bad(
            RefusalCode::UnbudgetedUnknowns204,
            "each input needs named UU scenarios with positive finite exposure and a trigger",
        ));
    }
    validate_color_payload(&claim.color()?).map_err(|e| {
        bad(
            RefusalCode::CalibrationRequired205,
            format!("invalid input colour: {e:?}"),
        )
    })?;
    Ok(())
}

struct Metadata {
    evidence: EvidenceClass,
    epistemic: EpistemicStatus,
    state: KnowledgeState,
    confidentiality: Confidentiality,
    uu: BTreeMap<String, UuTerm>,
    parents: BTreeSet<ObjectId>,
    anchors: BTreeSet<SourceAnchor>,
}
impl Metadata {
    fn new() -> Self {
        Self {
            evidence: EvidenceClass::Derived,
            epistemic: EpistemicStatus::KnownKnown,
            state: KnowledgeState::Known,
            confidentiality: Confidentiality::public(),
            uu: BTreeMap::new(),
            parents: BTreeSet::new(),
            anchors: BTreeSet::new(),
        }
    }
    fn include<T>(&mut self, claim: &Claim<T>) -> Result<()> {
        self.evidence = self.evidence.meet(claim.evidence);
        self.epistemic = self.epistemic.and(claim.epistemic);
        self.state = self.state.and(claim.state);
        self.confidentiality = self.confidentiality.join(&claim.confidentiality);
        self.parents.insert(claim.claim_id);
        self.anchors.extend(claim.anchors.iter().cloned());
        for u in &claim.uu_terms {
            if let Some(old) = self.uu.insert(u.scenario_id.clone(), u.clone())
                && old != *u
            {
                return Err(bad(
                    RefusalCode::UnbudgetedUnknowns204,
                    format!("UU {} has conflicting definitions", u.scenario_id),
                ));
            }
        }
        Ok(())
    }
    fn claim<T>(
        &self,
        metric: &str,
        value: T,
        tag: &str,
        additivity: AdditivityClass,
        compact: bool,
    ) -> Claim<T> {
        // Internal bundle calculations keep every input parent and one anchor per source artifact. The
        // public result below keeps every original locator and full input Claim; avoid copying thousands
        // of identical source locators into each of the 256 temporary bundle inputs.
        let anchors = if compact {
            let mut sources = BTreeMap::new();
            for anchor in &self.anchors {
                sources
                    .entry((&anchor.artifact_path, anchor.artifact_sha256))
                    .or_insert_with(|| anchor.clone());
            }
            sources.into_values().collect()
        } else {
            self.anchors.iter().cloned().collect()
        };
        Claim::new(
            metric,
            value,
            tag,
            Representation::Deterministic,
            SpatialSupport::PeriodAggregate,
            additivity,
            self.evidence,
            self.epistemic,
            self.state,
            self.confidentiality.clone(),
            self.uu.values().cloned().collect(),
            self.parents.iter().copied().collect(),
            anchors,
        )
    }
}

fn validate(inputs: &ProgramInputs) -> Result<Metadata> {
    if !(1..=MAX_CANDIDATES).contains(&inputs.candidates.len())
        || !(1..=MAX_STATES).contains(&inputs.weights.value.states.len())
    {
        return Err(bad(
            RefusalCode::NoClaimInDomain201,
            "requires 1..=8 candidates and 1..=256 shared states",
        ));
    }
    let policy = &inputs.policy.value;
    admit(
        &inputs.policy,
        &policy.canonical_tag(),
        AdditivityClass::NonAdditive,
    )?;
    admit(
        &inputs.weights,
        &inputs.weights.value.canonical_tag(),
        AdditivityClass::NonAdditive,
    )?;
    if !key(&policy.model_key)
        || policy.model_key != inputs.weights.value.model_key
        || !named(&inputs.weights.value.question)
        || !named(&policy.valuation_basis)
        || !named(&policy.reference_class_basis)
        || policy.payoff_convention != PAYOFF_CONVENTION
    {
        return Err(bad(
            RefusalCode::BasisMismatch105,
            "a common named model, valuation basis, reference-class basis and additive_net_delta_nav convention are required",
        ));
    }
    if policy.currency != Currency::USD
        || policy.budget.ccy != policy.currency
        || policy
            .max_tail_loss
            .is_some_and(|v| v.ccy != policy.currency)
    {
        return Err(bad(
            RefusalCode::UndeclaredUnitBasisOrTz107,
            "this slice uses explicit USD major units throughout; no currency conversion",
        ));
    }
    if policy.budget.minor < 0
        || policy.tail_mass.signum() <= 0
        || arithmetic(policy.tail_mass.checked_cmp(ONE))?.is_gt()
    {
        return Err(bad(
            RefusalCode::OutsidePhysicalDomain109,
            "budget must be nonnegative and tail mass must lie in (0,1]",
        ));
    }
    let mut states = BTreeSet::new();
    let mut weight_sum = ZERO;
    for (id, weight) in &inputs.weights.value.states {
        if !key(id) || !states.insert(id.as_str()) || weight.signum() <= 0 {
            return Err(bad(
                RefusalCode::OutsidePhysicalDomain109,
                "distinct named states need strictly positive exact weights",
            ));
        }
        weight_sum = arithmetic(weight_sum.checked_add(*weight))?;
    }
    if weight_sum != ONE {
        return Err(bad(
            RefusalCode::OutsidePhysicalDomain109,
            "weights must sum exactly to one; no implicit normalization",
        ));
    }
    let mut metadata = Metadata::new();
    metadata.include(&inputs.policy)?;
    metadata.include(&inputs.weights)?;
    let mut candidates = BTreeMap::new();
    for (i, claim) in inputs.candidates.iter().enumerate() {
        let c = &claim.value;
        admit(
            claim,
            &c.canonical_tag(),
            AdditivityClass::ExtensiveAdditive,
        )?;
        metadata.include(claim)?;
        if !key(&c.candidate_id)
            || !named(&c.label)
            || !named(&c.reference_class_basis)
            || c.exclusion_group.as_deref().is_some_and(|g| !key(g))
            || candidates.insert(c.candidate_id.as_str(), i).is_some()
        {
            return Err(bad(
                RefusalCode::BasisMismatch105,
                "candidate ids must be unique; labels, adjustment bases and any exclusion-group names are required",
            ));
        }
        if c.model_key != policy.model_key
            || c.currency != policy.currency
            || c.valuation_basis != policy.valuation_basis
            || c.cost.ccy != policy.currency
            || c.cost.minor < 0
        {
            return Err(bad(
                RefusalCode::BasisMismatch105,
                "candidate model, currency, valuation basis and nonnegative spending must agree with policy",
            ));
        }
        let mut present = BTreeSet::new();
        for (id, payoff) in &c.payoffs {
            if !states.contains(id.as_str())
                || !present.insert(id.as_str())
                || payoff.ccy != policy.currency
            {
                return Err(bad(
                    RefusalCode::BasisMismatch105,
                    "duplicate/foreign payoff state or currency mismatch",
                ));
            }
        }
        if present != states {
            return Err(bad(
                RefusalCode::NoClaimInDomain201,
                format!("{} lacks a payoff for every declared state", c.candidate_id),
            ));
        }
    }
    let mut reach = vec![0_u16; inputs.candidates.len()];
    for (i, c) in inputs.candidates.iter().enumerate() {
        let mut seen = BTreeSet::new();
        for required in &c.value.requires {
            let j = candidates.get(required.as_str()).ok_or_else(|| {
                bad(
                    RefusalCode::BasisMismatch105,
                    format!("unknown prerequisite {required}"),
                )
            })?;
            if !seen.insert(required) {
                return Err(bad(RefusalCode::BasisMismatch105, "duplicate prerequisite"));
            }
            reach[i] |= 1_u16 << j;
        }
    }
    for k in 0..reach.len() {
        for i in 0..reach.len() {
            if reach[i] & (1 << k) != 0 {
                let next = reach[k];
                reach[i] |= next;
            }
        }
    }
    if reach
        .iter()
        .enumerate()
        .any(|(i, mask)| mask & (1 << i) != 0)
    {
        return Err(bad(
            RefusalCode::BasisMismatch105,
            "prerequisite graph contains a cycle",
        ));
    }
    Ok(metadata)
}

/// Exact upper-tail signed loss from the complete joint payoff vector, including fractional boundary mass.
fn tail_loss(action: &ActionPayoffs, weights: &WeightSet, tail: Rat) -> Result<Rat> {
    let weights: BTreeMap<_, _> = weights
        .states
        .iter()
        .map(|(id, w)| (id.as_str(), *w))
        .collect();
    let mut values = action.values.clone();
    values.sort_by(|a, b| a.1.minor.cmp(&b.1.minor).then(a.0.cmp(&b.0)));
    let (mut remaining, mut sum) = (tail, ZERO);
    for (state, value) in values {
        if remaining == ZERO {
            break;
        }
        let weight = weights.get(state.as_str()).ok_or_else(|| {
            bad(
                RefusalCode::NumericalCertificateFailed207,
                "missing admitted state weight",
            )
        })?;
        let take = if arithmetic(weight.checked_cmp(remaining))?.is_gt() {
            remaining
        } else {
            *weight
        };
        sum = arithmetic(sum.checked_add(arithmetic(amount(value)?.checked_mul(take))?))?;
        remaining = arithmetic(remaining.checked_sub(take))?;
    }
    if remaining != ZERO {
        return Err(bad(
            RefusalCode::NumericalCertificateFailed207,
            "tail mass was not covered",
        ));
    }
    arithmetic(arithmetic(sum.checked_div(tail))?.checked_neg())
}

fn zero_statistics(action: &ActionPayoffs, weights: &WeightSet) -> AlternativeStats {
    let zero = ExactMoney {
        currency: action.currency,
        major: ZERO,
    };
    AlternativeStats {
        action_id: action.action_id.clone(),
        label: action.label.clone(),
        role: ActionRole::DoNothing,
        mean: zero,
        p10_p50_p90: [zero; 3],
        cvar95_loss: zero,
        mean_regret: zero,
        max_regret: zero,
        states: weights
            .states
            .iter()
            .map(|(id, w)| StateResult {
                state_id: id.clone(),
                weight: *w,
                payoff: zero,
                best_in_hindsight: zero,
                regret: zero,
            })
            .collect(),
    }
}

fn winners(choices: &[ProgramChoice], risk: bool) -> Result<(Vec<String>, Option<Rat>)> {
    let mut best = None;
    let mut ids = Vec::new();
    for row in choices {
        if risk && row.risk_rejected {
            continue;
        }
        let Some(stats) = &row.statistics else {
            continue;
        };
        let cmp = match best {
            None => std::cmp::Ordering::Greater,
            Some(old) => arithmetic(stats.mean.major.checked_cmp(old))?,
        };
        if cmp.is_gt() {
            best = Some(stats.mean.major);
            ids.clear();
        }
        if !cmp.is_lt() {
            ids.push(row.bundle_id.clone());
        }
    }
    Ok((ids, best))
}

/// Enumerates all declared binary subsets; infeasibility remains in the returned population.
pub fn evaluate(inputs: &ProgramInputs) -> Result<EvaluatedProgram> {
    let metadata = validate(inputs)?;
    let mut inputs = inputs.clone();
    inputs
        .candidates
        .sort_by(|a, b| a.value.candidate_id.cmp(&b.value.candidate_id));
    inputs.weights.value.states.sort_by(|a, b| a.0.cmp(&b.0));
    for candidate in &mut inputs.candidates {
        candidate.value.payoffs.sort_by(|a, b| a.0.cmp(&b.0));
        candidate.value.requires.sort();
    }
    let policy = &inputs.policy.value;
    let money = |major| ExactMoney {
        currency: policy.currency,
        major,
    };
    let mut choices = Vec::new();
    let mut actions = Vec::new();
    for mask in 0..(1_usize << inputs.candidates.len()) {
        let selected: Vec<_> = inputs
            .candidates
            .iter()
            .enumerate()
            .filter(|(i, _)| mask & (1 << i) != 0)
            .map(|(_, c)| &c.value)
            .collect();
        let ids: Vec<_> = selected.iter().map(|c| c.candidate_id.clone()).collect();
        let spend = selected
            .iter()
            .try_fold(0_i128, |sum, c| checked_minor_add(sum, c.cost.minor))?;
        let mut rejected = Vec::new();
        if spend > policy.budget.minor {
            rejected.push(StructuralRejection::BudgetExceeded);
        }
        let mut groups: BTreeMap<&str, Vec<String>> = BTreeMap::new();
        for c in &selected {
            for required in &c.requires {
                if !ids.contains(required) {
                    rejected.push(StructuralRejection::MissingPrerequisite {
                        candidate: c.candidate_id.clone(),
                        prerequisite: required.clone(),
                    });
                }
            }
            if let Some(group) = &c.exclusion_group {
                groups
                    .entry(group)
                    .or_default()
                    .push(c.candidate_id.clone());
            }
        }
        for (group, candidates) in groups {
            if candidates.len() > 1 {
                rejected.push(StructuralRejection::MutualExclusion {
                    group: group.into(),
                    candidates,
                });
            }
        }
        let bundle_id = format!("bundle_{mask:02x}");
        if rejected.is_empty() {
            let values = inputs
                .weights
                .value
                .states
                .iter()
                .enumerate()
                .map(|(i, (state, _))| {
                    let minor = selected.iter().try_fold(0_i128, |sum, c| {
                        checked_minor_add(sum, c.payoffs[i].1.minor)
                    })?;
                    Ok((state.clone(), Money::from_minor(policy.currency, minor)))
                })
                .collect::<Result<Vec<_>>>()?;
            let action = ActionPayoffs {
                model_key: policy.model_key.clone(),
                action_id: bundle_id.clone(),
                label: if ids.is_empty() {
                    "Do nothing".into()
                } else {
                    ids.join(" + ")
                },
                role: if ids.is_empty() {
                    ActionRole::DoNothing
                } else {
                    ActionRole::Action
                },
                currency: policy.currency,
                valuation_basis: policy.valuation_basis.clone(),
                values,
            };
            let tag = action.canonical_tag();
            actions.push(metadata.claim(
                "capital.bundle.net-delta-nav/1",
                action,
                &tag,
                AdditivityClass::ExtensiveAdditive,
                true,
            ));
        }
        let spend = arithmetic(Rat::new(spend, Money::SCALE))?;
        choices.push(ProgramChoice {
            bundle_id,
            selected: ids,
            spend: money(spend),
            budget_slack: money(arithmetic(amount(policy.budget)?.checked_sub(spend))?),
            structural_rejections: rejected,
            statistics: None,
            tail_loss: None,
            risk_rejected: false,
        });
    }
    let stats = if actions.len() == 1 {
        vec![zero_statistics(&actions[0].value, &inputs.weights.value)]
    } else {
        card::evaluate(&DecisionInputs {
            weights: inputs.weights.clone(),
            actions: actions.clone(),
            information_options: vec![],
        })?
        .summary()
        .alternatives
        .clone()
    };
    let mut statistics: BTreeMap<_, _> = stats
        .into_iter()
        .map(|s| (s.action_id.clone(), s))
        .collect();
    let mut tails = BTreeMap::new();
    for action in &actions {
        tails.insert(
            action.value.action_id.clone(),
            tail_loss(&action.value, &inputs.weights.value, policy.tail_mass)?,
        );
    }
    for row in &mut choices {
        if let Some(stats) = statistics.remove(&row.bundle_id) {
            let tail = tails.remove(&row.bundle_id).ok_or_else(|| {
                bad(
                    RefusalCode::NumericalCertificateFailed207,
                    "missing computed tail",
                )
            })?;
            row.risk_rejected = match policy.max_tail_loss {
                Some(cap) => arithmetic(tail.checked_cmp(amount(cap)?))?.is_gt(),
                None => false,
            };
            row.tail_loss = Some(money(tail));
            row.statistics = Some(stats);
        }
    }
    let (unconstrained_winners, before) = winners(&choices, false)?;
    let (feasible_winners, after) = winners(&choices, true)?;
    let risk_cost = match (before, after) {
        (Some(a), Some(b)) => Some(money(arithmetic(a.checked_sub(b))?)),
        _ => None,
    };
    let summary = ProgramSummary {
        model_key: policy.model_key.clone(),
        question: inputs.weights.value.question.clone(),
        policy: policy.clone(),
        choices,
        unconstrained_winners,
        feasible_winners,
        expected_value_cost_of_risk_limit: risk_cost,
    };
    let mut binding = Encoder::new_frame(*b"CAPP", 1);
    binding.put_str(&render::inputs_json(&inputs));
    binding.put_str(&render::summary_json(&summary));
    binding.put_str(NO_CLAIM);
    let digest = sha256(&binding.finish()).to_hex();
    let tag = format!("capital-program/v1:{digest}");
    let claim = metadata.claim(
        "capital.static-program/1",
        summary,
        &tag,
        AdditivityClass::NonAdditive,
        false,
    );
    Ok(EvaluatedProgram {
        inputs,
        claim,
        digest,
    })
}

#[cfg(test)]
mod tests;
