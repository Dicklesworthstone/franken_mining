//! Exact finite-scenario draft decisions (BD-225, plan §22.5, G-HELM-2/7).
//! Determinism: Exact (rational monetary arithmetic; inherited UU/colour metadata is retained).
//!
//! All alternatives use the complete same named state population. Weights are declared, not inferred
//! probabilities. Exact conditional results do not establish calibration, physical feasibility or authority.

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};

use fmine_claim::{
    AdditivityClass, Claim, Confidentiality, Representation, SpatialSupport, UuTerm,
};
use fmine_core::codec::Encoder;
use fmine_core::hash::sha256;
use fmine_evidence::color::{ColorRank, validate_color_payload};
use fmine_evidence::{EpistemicStatus, EvidenceClass, KnowledgeState, Refusal, RefusalCode};
use fmine_numerics::exact_feas::Rat;
use fmine_units::{Currency, Money, UnitsError};

pub mod information;
pub mod model;
mod render;

pub use information::{
    InformationOption, InformationSummary, Likelihood, MAX_INFORMATION_OPTIONS,
    MAX_INFORMATION_WORK, MAX_SIGNALS,
};

type Result<T> = std::result::Result<T, Refusal>;
const ZERO: Rat = Rat::from_integer(0);
const ONE: Rat = Rat::from_integer(1);
pub const MAX_STATES: usize = 4096;
pub const MAX_ACTIONS: usize = 256;
pub const NO_CLAIM: &str = "Draft comparison conditional on the declared complete finite state/payoff model. State weights and information-option likelihoods are not calibrated probabilities; monetary quantiles, CVaR and exact enumerated information values describe this model only. No forecast, Monte Carlo VOI accuracy claim, hard-constraint or safety assessment, historical evidence-cut verification, sign-off, reserve disclosure or execution authorization is supplied.";
pub const TIE_RULE: &str = "All exact co-winners are retained; ascending action_id or option_id orders equal values for presentation only. Equal overall expected values list act-now choices before information options. Primary and runner-up do not assert a unique preferred action when tied.";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WeightSet {
    pub model_key: String,
    pub question: String,
    pub states: Vec<(String, Rat)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionRole {
    Action,
    DoNothing,
}

impl ActionRole {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Action => "action",
            Self::DoNothing => "do_nothing",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionPayoffs {
    pub model_key: String,
    pub action_id: String,
    pub label: String,
    pub role: ActionRole,
    pub currency: Currency,
    /// Shared declared NPV interpretation, including valuation date, real/nominal and ownership basis.
    pub valuation_basis: String,
    pub values: Vec<(String, Money)>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DecisionInputs {
    pub weights: Claim<WeightSet>,
    pub actions: Vec<Claim<ActionPayoffs>>,
    pub information_options: Vec<Claim<InformationOption>>,
}

fn put_rat(enc: &mut Encoder, value: Rat) {
    enc.put_fixed(&value.num().to_le_bytes());
    enc.put_fixed(&value.den().to_le_bytes());
}

impl WeightSet {
    #[must_use]
    pub fn canonical_tag(&self) -> String {
        let mut enc = Encoder::new_frame(*b"DCWT", 1);
        enc.put_str(&self.model_key);
        enc.put_str(&self.question);
        let mut states = self.states.clone();
        states.sort_by(|a, b| {
            a.0.cmp(&b.0)
                .then(a.1.num().cmp(&b.1.num()))
                .then(a.1.den().cmp(&b.1.den()))
        });
        enc.put_u64(states.len() as u64);
        for (id, weight) in states {
            enc.put_str(&id);
            put_rat(&mut enc, weight);
        }
        format!("decision-weights/v1:{}", sha256(&enc.finish()))
    }
}

impl ActionPayoffs {
    #[must_use]
    pub fn canonical_tag(&self) -> String {
        let mut enc = Encoder::new_frame(*b"DCAC", 1);
        for text in [
            &self.model_key,
            &self.action_id,
            &self.label,
            self.role.as_str(),
            currency_name(self.currency),
            &self.valuation_basis,
        ] {
            enc.put_str(text);
        }
        let mut values = self.values.clone();
        values.sort_by(|a, b| {
            a.0.cmp(&b.0)
                .then(a.1.ccy.cmp(&b.1.ccy))
                .then(a.1.minor.cmp(&b.1.minor))
        });
        enc.put_u64(values.len() as u64);
        for (id, value) in values {
            enc.put_str(&id);
            enc.put_str(currency_name(value.ccy));
            enc.put_fixed(&value.minor.to_le_bytes());
        }
        format!("decision-action/v1:{}", sha256(&enc.finish()))
    }
}

/// A monetary result in exact major currency units; no implicit minor-unit rounding occurs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExactMoney {
    pub currency: Currency,
    pub major: Rat,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateResult {
    pub state_id: String,
    pub weight: Rat,
    pub payoff: ExactMoney,
    pub best_in_hindsight: ExactMoney,
    pub regret: ExactMoney,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AlternativeStats {
    pub action_id: String,
    pub label: String,
    pub role: ActionRole,
    pub mean: ExactMoney,
    pub p10_p50_p90: [ExactMoney; 3],
    /// Upper 5% weighted tail of signed loss = -payoff, including a fractional boundary mass.
    pub cvar95_loss: ExactMoney,
    pub mean_regret: ExactMoney,
    pub max_regret: ExactMoney,
    pub states: Vec<StateResult>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecisionSummary {
    pub model_key: String,
    pub question: String,
    pub currency: Currency,
    pub valuation_basis: String,
    pub alternatives: Vec<AlternativeStats>,
    pub expected_value_order: Vec<String>,
    pub expected_value_winners: Vec<String>,
    pub expected_value_primary: String,
    pub runner_up: String,
    pub minimax_regret_order: Vec<String>,
    pub minimax_regret_winners: Vec<String>,
    pub minimax_regret_primary: String,
    /// Expected-value comparisons for buying at most one test. The base risk/regret matrix stays separate.
    pub information: Option<InformationSummary>,
}

/// Sealed computed comparison; input and output snapshots have no mutable accessors.
#[derive(Debug, Clone, PartialEq)]
pub struct EvaluatedCard {
    inputs: DecisionInputs,
    claim: Claim<DecisionSummary>,
    weakest_input_color: ColorRank,
}

impl EvaluatedCard {
    #[must_use]
    pub fn inputs(&self) -> &DecisionInputs {
        &self.inputs
    }
    #[must_use]
    pub fn claim(&self) -> &Claim<DecisionSummary> {
        &self.claim
    }
    #[must_use]
    pub fn summary(&self) -> &DecisionSummary {
        &self.claim.value
    }
    #[must_use]
    pub fn weakest_input_color(&self) -> ColorRank {
        self.weakest_input_color
    }
}

fn refusal(code: RefusalCode, message: impl Into<String>) -> Refusal {
    Refusal::new(code, format!("decision comparison: {}", message.into()))
}

fn arithmetic<T>(value: std::result::Result<T, UnitsError>) -> Result<T> {
    value.map_err(|e| {
        refusal(
            RefusalCode::NumericalCertificateFailed207,
            format!("exact arithmetic: {e}"),
        )
    })
}

fn named(value: &str) -> bool {
    !value.is_empty() && value.trim() == value && !value.chars().any(char::is_control)
}

fn admit<T>(claim: &Claim<T>, tag: &str, additivity: AdditivityClass) -> Result<ColorRank> {
    claim.check_id()?;
    if claim.value_tag() != tag {
        return Err(refusal(
            RefusalCode::FrozenRecordMutation507,
            format!(
                "{}: typed value differs from its canonical tag",
                claim.metric_key
            ),
        ));
    }
    if claim.stale || claim.state == KnowledgeState::Stale {
        return Err(refusal(RefusalCode::StaleAnchor502, &claim.metric_key));
    }
    if !matches!(
        claim.state,
        KnowledgeState::Known | KnowledgeState::Estimated
    ) {
        return Err(refusal(
            RefusalCode::NotObservable202,
            format!("{}: unusable knowledge state", claim.metric_key),
        ));
    }
    if claim.representation != Representation::Deterministic {
        return Err(refusal(
            RefusalCode::SourceCellUnusable208,
            "this model needs explicit deterministic state values, not a placeholder or summary distribution",
        ));
    }
    if claim.support != SpatialSupport::PeriodAggregate {
        return Err(refusal(
            RefusalCode::SupportMismatch102,
            "decision inputs require common period-aggregate support",
        ));
    }
    if claim.additivity != additivity {
        return Err(refusal(
            RefusalCode::NonAdditiveAverage103,
            "weights are non-additive and monetary state payoffs are extensive",
        ));
    }
    if !named(&claim.metric_key)
        || claim.anchors.is_empty()
        || claim
            .anchors
            .iter()
            .any(|a| !named(&a.artifact_path) || !named(&a.locator) || a.artifact_sha256 == [0; 32])
    {
        return Err(refusal(
            RefusalCode::NoClaimInDomain201,
            "every input needs a named Claim, source digest and locator",
        ));
    }
    claim.verify_uu_slot_for_decision()?;
    if claim.uu_terms.is_empty()
        || claim.uu_terms.iter().any(|u| {
            !named(&u.scenario_id)
                || !named(&u.description)
                || !named(&u.trigger_indicator)
                || !u.downside_exposure_usd_m.is_finite()
                || u.downside_exposure_usd_m <= 0.0
        })
    {
        return Err(refusal(
            RefusalCode::UnbudgetedUnknowns204,
            "every input needs explicit named UU scenarios with a trigger and positive finite downside exposure",
        ));
    }
    let color = claim.color()?;
    validate_color_payload(&color).map_err(|e| {
        refusal(
            RefusalCode::CalibrationRequired205,
            format!("{}: invalid colour payload {e:?}", claim.metric_key),
        )
    })?;
    Ok(color.rank())
}

fn quantile(sorted: &[(Rat, Rat)], probability: Rat) -> Result<Rat> {
    let mut mass = ZERO;
    for (value, weight) in sorted {
        mass = arithmetic(mass.checked_add(*weight))?;
        if arithmetic(mass.checked_cmp(probability))? != Ordering::Less {
            return Ok(*value);
        }
    }
    Err(refusal(
        RefusalCode::NumericalCertificateFailed207,
        "quantile failed to reach its required mass",
    ))
}

fn statistics(
    action: &ActionPayoffs,
    states: &[(String, Rat)],
    best: &BTreeMap<String, Money>,
) -> Result<AlternativeStats> {
    let values: BTreeMap<_, _> = action
        .values
        .iter()
        .map(|(id, value)| (id.as_str(), *value))
        .collect();
    let money = |major| ExactMoney {
        currency: action.currency,
        major,
    };
    let mut rows = Vec::new();
    let (mut mean, mut mean_regret, mut max_regret) = (ZERO, ZERO, ZERO);
    let mut distribution = Vec::new();
    for (id, weight) in states {
        let payoff = arithmetic(Rat::new(values[id.as_str()].minor, Money::SCALE))?;
        let optimum = arithmetic(Rat::new(best[id].minor, Money::SCALE))?;
        let regret = arithmetic(optimum.checked_sub(payoff))?;
        if regret.signum() < 0 {
            return Err(refusal(
                RefusalCode::NumericalCertificateFailed207,
                "same-state regret became negative",
            ));
        }
        mean = arithmetic(mean.checked_add(arithmetic(payoff.checked_mul(*weight))?))?;
        mean_regret =
            arithmetic(mean_regret.checked_add(arithmetic(regret.checked_mul(*weight))?))?;
        if arithmetic(regret.checked_cmp(max_regret))? == Ordering::Greater {
            max_regret = regret;
        }
        rows.push(StateResult {
            state_id: id.clone(),
            weight: *weight,
            payoff: money(payoff),
            best_in_hindsight: money(optimum),
            regret: money(regret),
        });
        // The original exact minor integers can be sorted without a potentially overflowing cross product.
        distribution.push((values[id.as_str()].minor, payoff, *weight));
    }
    distribution.sort_by_key(|row| row.0);
    let sorted: Vec<_> = distribution
        .iter()
        .map(|(_, value, weight)| (*value, *weight))
        .collect();
    let probabilities = [
        arithmetic(Rat::new(1, 10))?,
        arithmetic(Rat::new(1, 2))?,
        arithmetic(Rat::new(9, 10))?,
    ];
    let mut quantiles = [money(ZERO); 3];
    for (index, p) in probabilities.into_iter().enumerate() {
        quantiles[index] = money(quantile(&sorted, p)?);
    }
    let tail_mass = arithmetic(Rat::new(1, 20))?;
    let (mut remaining, mut integral) = (tail_mass, ZERO);
    // Smallest payoff is largest signed loss. Never replace an all-gain negative loss with zero.
    for (payoff, weight) in sorted {
        if remaining == ZERO {
            break;
        }
        let take = if arithmetic(weight.checked_cmp(remaining))? == Ordering::Greater {
            remaining
        } else {
            weight
        };
        integral = arithmetic(integral.checked_add(arithmetic(
            arithmetic(payoff.checked_neg())?.checked_mul(take),
        )?))?;
        remaining = arithmetic(remaining.checked_sub(take))?;
    }
    if remaining != ZERO {
        return Err(refusal(
            RefusalCode::NumericalCertificateFailed207,
            "CVaR tail lacks full mass",
        ));
    }
    Ok(AlternativeStats {
        action_id: action.action_id.clone(),
        label: action.label.clone(),
        role: action.role,
        mean: money(mean),
        p10_p50_p90: quantiles,
        cvar95_loss: money(arithmetic(integral.checked_div(tail_mass))?),
        mean_regret: money(mean_regret),
        max_regret: money(max_regret),
        states: rows,
    })
}

fn ranking(alternatives: &[AlternativeStats], minimax: bool) -> Result<(Vec<String>, Vec<String>)> {
    let value = |a: &AlternativeStats| {
        if minimax {
            a.max_regret.major
        } else {
            a.mean.major
        }
    };
    let mut ordered: Vec<&AlternativeStats> = Vec::new();
    for candidate in alternatives {
        let mut index = ordered.len();
        for (i, incumbent) in ordered.iter().enumerate() {
            let mut cmp = arithmetic(value(candidate).checked_cmp(value(incumbent)))?;
            if !minimax {
                cmp = cmp.reverse();
            }
            if cmp == Ordering::Less
                || (cmp == Ordering::Equal && candidate.action_id < incumbent.action_id)
            {
                index = i;
                break;
            }
        }
        ordered.insert(index, candidate);
    }
    let Some(first) = ordered.first() else {
        return Err(refusal(RefusalCode::NoClaimInDomain201, "no alternatives"));
    };
    let winners = ordered
        .iter()
        .filter(|a| value(a) == value(first))
        .map(|a| a.action_id.clone())
        .collect();
    Ok((
        ordered.iter().map(|a| a.action_id.clone()).collect(),
        winners,
    ))
}

/// Computes every alternative and its regret from the same complete named state matrix.
/// Any unusable material input refuses the comparison; actions and states are never dropped or repaired.
pub fn evaluate(inputs: &DecisionInputs) -> Result<EvaluatedCard> {
    if inputs.actions.len() < 2
        || inputs.actions.len() > MAX_ACTIONS
        || inputs.weights.value.states.is_empty()
        || inputs.weights.value.states.len() > MAX_STATES
    {
        return Err(refusal(
            RefusalCode::NoClaimInDomain201,
            format!("requires 2..={MAX_ACTIONS} actions and 1..={MAX_STATES} states"),
        ));
    }
    information::check_work(inputs)?;
    let mut inputs = inputs.clone();
    let mut weakest = admit(
        &inputs.weights,
        &inputs.weights.value.canonical_tag(),
        AdditivityClass::NonAdditive,
    )?;
    let weights = &inputs.weights.value;
    if !named(&weights.model_key) || !named(&weights.question) {
        return Err(refusal(
            RefusalCode::UndeclaredUnitBasisOrTz107,
            "model key and question are required",
        ));
    }
    let mut states = BTreeMap::new();
    for (id, weight) in &weights.states {
        if !named(id) || states.insert(id.clone(), *weight).is_some() || weight.signum() <= 0 {
            return Err(refusal(
                RefusalCode::OutsidePhysicalDomain109,
                "states need distinct names and strictly positive weights",
            ));
        }
    }
    let mut total = ZERO;
    for weight in states.values() {
        total = arithmetic(total.checked_add(*weight))?;
    }
    if total != ONE {
        return Err(refusal(
            RefusalCode::OutsidePhysicalDomain109,
            "weights must sum exactly to one; no normalization is implicit",
        ));
    }
    inputs.weights.value.states = states
        .iter()
        .map(|(id, weight)| (id.clone(), *weight))
        .collect();
    inputs
        .actions
        .sort_by(|a, b| a.value.action_id.cmp(&b.value.action_id));
    let (currency, basis) = (
        inputs.actions[0].value.currency,
        inputs.actions[0].value.valuation_basis.clone(),
    );
    let mut ids = BTreeSet::new();
    let mut do_nothing = 0;
    let mut best: BTreeMap<String, Money> = BTreeMap::new();
    for claim in &mut inputs.actions {
        weakest = weakest.min(admit(
            claim,
            &claim.value.canonical_tag(),
            AdditivityClass::ExtensiveAdditive,
        )?);
        let action = &mut claim.value;
        if !named(&action.action_id)
            || !named(&action.label)
            || !named(&action.valuation_basis)
            || !ids.insert(action.action_id.clone())
        {
            return Err(refusal(
                RefusalCode::BasisMismatch105,
                "actions need distinct ids, labels and an explicit valuation basis",
            ));
        }
        if action.model_key != inputs.weights.value.model_key
            || action.currency != currency
            || action.valuation_basis != basis
        {
            return Err(refusal(
                RefusalCode::BasisMismatch105,
                "model key, currency and valuation basis must agree across every action",
            ));
        }
        do_nothing += usize::from(action.role == ActionRole::DoNothing);
        let mut present = BTreeSet::new();
        for (id, value) in &action.values {
            if !states.contains_key(id) || !present.insert(id.clone()) || value.ccy != currency {
                return Err(refusal(
                    RefusalCode::BasisMismatch105,
                    format!(
                        "{} has duplicate/foreign states or a currency mismatch",
                        action.action_id
                    ),
                ));
            }
            best.entry(id.clone())
                .and_modify(|old| {
                    if value.minor > old.minor {
                        *old = *value;
                    }
                })
                .or_insert(*value);
        }
        if present.len() != states.len() {
            return Err(refusal(
                RefusalCode::NoClaimInDomain201,
                format!(
                    "{} lacks a payoff for every named state; no alternative can be dropped",
                    action.action_id
                ),
            ));
        }
        action.values.sort_by(|a, b| a.0.cmp(&b.0));
    }
    if do_nothing != 1 {
        return Err(refusal(
            RefusalCode::NoClaimInDomain201,
            "exactly one explicit do_nothing action is required",
        ));
    }
    let alternatives = inputs
        .actions
        .iter()
        .map(|a| statistics(&a.value, &inputs.weights.value.states, &best))
        .collect::<Result<Vec<_>>>()?;
    let (expected_value_order, expected_value_winners) = ranking(&alternatives, false)?;
    let (minimax_regret_order, minimax_regret_winners) = ranking(&alternatives, true)?;
    let mut summary = DecisionSummary {
        model_key: inputs.weights.value.model_key.clone(),
        question: inputs.weights.value.question.clone(),
        currency,
        valuation_basis: basis,
        alternatives,
        expected_value_primary: expected_value_order[0].clone(),
        runner_up: expected_value_order[1].clone(),
        expected_value_order,
        expected_value_winners,
        minimax_regret_primary: minimax_regret_order[0].clone(),
        minimax_regret_order,
        minimax_regret_winners,
        information: None,
    };
    let (information, information_color) =
        information::assess(&mut inputs.information_options, &summary)?;
    summary.information = information;
    weakest = weakest.min(information_color);
    let mut binding = Encoder::new_frame(*b"DCCL", 1);
    binding.put_str(&render::summary_json(&summary));
    binding.put_str(&render::inputs_json(&inputs));
    let tag = format!("decision-comparison/v1:{}", sha256(&binding.finish()));
    let mut metadata = Metadata::default();
    metadata.include(&inputs.weights)?;
    for action in &inputs.actions {
        metadata.include(action)?;
    }
    for option in &inputs.information_options {
        metadata.include(option)?;
    }
    let claim = Claim::new(
        "decision.finite-scenario/1",
        summary,
        &tag,
        Representation::Deterministic,
        SpatialSupport::PeriodAggregate,
        AdditivityClass::NonAdditive,
        metadata.evidence,
        metadata.epistemic,
        metadata.state,
        metadata.confidentiality,
        metadata.uu.into_values().collect(),
        metadata.parents.into_iter().collect(),
        metadata.anchors.into_iter().collect(),
    );
    Ok(EvaluatedCard {
        inputs,
        claim,
        weakest_input_color: weakest,
    })
}

struct Metadata {
    evidence: EvidenceClass,
    epistemic: EpistemicStatus,
    state: KnowledgeState,
    confidentiality: Confidentiality,
    uu: BTreeMap<String, UuTerm>,
    parents: BTreeSet<fmine_core::ObjectId>,
    anchors: BTreeSet<fmine_core::anchor::SourceAnchor>,
}

impl Default for Metadata {
    fn default() -> Self {
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
}

impl Metadata {
    fn include<T>(&mut self, claim: &Claim<T>) -> Result<()> {
        self.evidence = self.evidence.meet(claim.evidence);
        self.epistemic = self.epistemic.and(claim.epistemic);
        self.state = self.state.and(claim.state);
        self.confidentiality = self.confidentiality.join(&claim.confidentiality);
        self.parents.insert(claim.claim_id);
        self.anchors.extend(claim.anchors.iter().cloned());
        for term in &claim.uu_terms {
            if let Some(previous) = self.uu.insert(term.scenario_id.clone(), term.clone())
                && previous != *term
            {
                return Err(refusal(
                    RefusalCode::UnbudgetedUnknowns204,
                    format!("UU id {} has competing definitions", term.scenario_id),
                ));
            }
        }
        Ok(())
    }
}

pub(super) const fn currency_name(currency: Currency) -> &'static str {
    match currency {
        Currency::USD => "USD",
        Currency::CAD => "CAD",
        Currency::AUD => "AUD",
        Currency::ZAR => "ZAR",
        Currency::PKR => "PKR",
        Currency::CLP => "CLP",
        Currency::PEN => "PEN",
        Currency::ARS => "ARS",
        Currency::TZS => "TZS",
        Currency::XAF => "XAF",
        Currency::CDF => "CDF",
        Currency::ZMW => "ZMW",
        Currency::PGK => "PGK",
        Currency::DOP => "DOP",
        Currency::SAR => "SAR",
        Currency::EUR => "EUR",
        Currency::JPY => "JPY",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fmine_core::anchor::SourceAnchor;
    use fmine_evidence::{KuKind, color::ValidityDomain};

    fn rat(num: i128, den: i128) -> Rat {
        Rat::new(num, den).unwrap()
    }
    fn money(major: i128) -> Money {
        Money::from_major(Currency::USD, major)
    }
    fn uu() -> UuTerm {
        UuTerm {
            scenario_id: "model_omission".into(),
            description: "Invented unmodelled commitment cost".into(),
            downside_exposure_usd_m: 35.0,
            trigger_indicator: "scope changes".into(),
        }
    }

    fn claim<T>(
        key: &str,
        value: T,
        tag: &str,
        additivity: AdditivityClass,
        evidence: EvidenceClass,
    ) -> Claim<T> {
        Claim::new(
            key,
            value,
            tag,
            Representation::Deterministic,
            SpatialSupport::PeriodAggregate,
            additivity,
            evidence,
            EpistemicStatus::KnownUnknown(KuKind::Parametric),
            KnowledgeState::Estimated,
            Confidentiality::internal(),
            vec![uu()],
            vec![],
            vec![SourceAnchor::new("invented-model.toml", [7; 32], key)],
        )
    }

    fn weight_claim(value: WeightSet) -> Claim<WeightSet> {
        let tag = value.canonical_tag();
        claim(
            "weights",
            value,
            &tag,
            AdditivityClass::NonAdditive,
            EvidenceClass::Assumed,
        )
    }
    fn action_claim(value: ActionPayoffs) -> Claim<ActionPayoffs> {
        let tag = value.canonical_tag();
        let key = value.action_id.clone();
        claim(
            &key,
            value,
            &tag,
            AdditivityClass::ExtensiveAdditive,
            EvidenceClass::Assumed,
        )
    }

    fn fixture() -> DecisionInputs {
        let weights = weight_claim(WeightSet {
            model_key: "gb29_arithmetic".into(),
            question: "Build or do nothing under these invented inputs?".into(),
            states: vec![("high".into(), rat(3, 5)), ("low".into(), rat(2, 5))],
        });
        let action = |id: &str, role, high, low| {
            action_claim(ActionPayoffs {
                model_key: weights.value.model_key.clone(),
                action_id: id.into(),
                label: id.into(),
                role,
                currency: Currency::USD,
                valuation_basis: "Invented real USD at decision date; full project NPV".into(),
                values: vec![("high".into(), money(high)), ("low".into(), money(low))],
            })
        };
        let actions = vec![
            action("build", ActionRole::Action, 300_000_000, -200_000_000),
            action("do_nothing", ActionRole::DoNothing, 0, 0),
        ];
        DecisionInputs {
            weights,
            actions,
            information_options: Vec::new(),
        }
    }

    fn stats<'a>(card: &'a EvaluatedCard, id: &str) -> &'a AlternativeStats {
        card.summary()
            .alternatives
            .iter()
            .find(|a| a.action_id == id)
            .unwrap()
    }

    #[test]
    fn gb29_exact_payoffs_regret_quantiles_and_signed_loss() {
        let inputs = fixture();
        let card = evaluate(&inputs).unwrap();
        let build = stats(&card, "build");
        assert_eq!(build.mean.major, rat(100_000_000, 1));
        assert_eq!(build.mean_regret.major, rat(80_000_000, 1));
        assert_eq!(build.max_regret.major, rat(200_000_000, 1));
        assert_eq!(
            build.p10_p50_p90.map(|v| v.major),
            [
                rat(-200_000_000, 1),
                rat(300_000_000, 1),
                rat(300_000_000, 1)
            ]
        );
        assert_eq!(build.cvar95_loss.major, rat(200_000_000, 1));
        let nothing = stats(&card, "do_nothing");
        assert_eq!(nothing.mean.major, ZERO);
        assert_eq!(nothing.mean_regret.major, rat(180_000_000, 1));
        assert_eq!(nothing.max_regret.major, rat(300_000_000, 1));
        assert_eq!(nothing.cvar95_loss.major, ZERO);
        assert_eq!(card.summary().expected_value_winners, ["build"]);
        assert_eq!(card.summary().minimax_regret_winners, ["build"]);
        assert_eq!(card.claim().parents.len(), 3);
        assert_eq!(card.claim().evidence, EvidenceClass::Assumed);
        assert_eq!(card.claim().confidentiality, Confidentiality::internal());
        assert_eq!(card.inputs(), &inputs);
        let json = card.to_json();
        assert!(json.contains("\"mean\":\"100000000/1\""));
        assert!(json.contains("\"execution_authorized\":false"));
        assert!(json.contains("\"downside_bits\":\"4041800000000000\""));
        let markdown = card.to_markdown();
        for text in [
            "signed loss = −NPV",
            "invented-model.toml",
            "state weights",
            "300000000/1",
        ] {
            assert!(
                markdown.to_lowercase().contains(&text.to_lowercase()),
                "{text}"
            );
        }
    }

    #[test]
    fn safe_alternative_separates_mean_choice_from_minimax_regret() {
        let mut inputs = fixture();
        let mut safe = inputs.actions[0].value.clone();
        safe.action_id = "safe".into();
        safe.label = "Safe conditional payoff".into();
        for (_, value) in &mut safe.values {
            *value = money(55_000_000);
        }
        inputs.actions.push(action_claim(safe));
        let card = evaluate(&inputs).unwrap();
        assert_eq!(
            card.summary().expected_value_order,
            ["build", "safe", "do_nothing"]
        );
        assert_eq!(
            card.summary().minimax_regret_order,
            ["safe", "build", "do_nothing"]
        );
        assert_eq!(stats(&card, "build").max_regret.major, rat(255_000_000, 1));
        assert_eq!(stats(&card, "build").mean_regret.major, rat(102_000_000, 1));
        assert_eq!(stats(&card, "safe").max_regret.major, rat(245_000_000, 1));
        assert_eq!(stats(&card, "safe").mean_regret.major, rat(147_000_000, 1));
        assert_eq!(
            card.claim().parents.len(),
            4,
            "unselected actions remain material parents"
        );
    }

    #[test]
    fn cvar_integrates_fractional_boundary_mass_and_does_not_clamp_gains() {
        let mut inputs = fixture();
        let mut weights = inputs.weights.value.clone();
        weights.states = vec![
            ("a".into(), rat(1, 50)),
            ("b".into(), rat(3, 50)),
            ("c".into(), rat(23, 25)),
        ];
        inputs.weights = weight_claim(weights);
        for (index, action) in inputs.actions.iter_mut().enumerate() {
            let mut value = action.value.clone();
            value.values = ["a", "b", "c"]
                .into_iter()
                .zip(if index == 0 {
                    [-100, -50, 10]
                } else {
                    [0, 0, 0]
                })
                .map(|(id, amount)| (id.into(), money(amount)))
                .collect();
            *action = action_claim(value);
        }
        let card = evaluate(&inputs).unwrap();
        assert_eq!(stats(&card, "build").cvar95_loss.major, rat(70, 1));
        assert_eq!(stats(&card, "build").mean.major, rat(21, 5));
        for action in &mut inputs.actions {
            let mut value = action.value.clone();
            for (_, amount) in &mut value.values {
                *amount = amount.checked_add(money(200)).unwrap();
            }
            *action = action_claim(value);
        }
        let shifted = evaluate(&inputs).unwrap();
        assert_eq!(stats(&shifted, "build").cvar95_loss.major, rat(-130, 1));
        assert_eq!(
            stats(&shifted, "do_nothing").cvar95_loss.major,
            rat(-200, 1)
        );
        assert_eq!(
            stats(&shifted, "build").mean_regret,
            stats(&card, "build").mean_regret
        );
    }

    #[test]
    fn all_ties_are_visible_and_named_population_permutations_are_stable() {
        let mut inputs = fixture();
        for action in &mut inputs.actions {
            let mut value = action.value.clone();
            for (_, amount) in &mut value.values {
                *amount = money(10);
            }
            *action = action_claim(value);
        }
        let card = evaluate(&inputs).unwrap();
        assert_eq!(
            card.summary().expected_value_winners,
            ["build", "do_nothing"]
        );
        assert_eq!(
            card.summary().minimax_regret_winners,
            ["build", "do_nothing"]
        );
        assert_eq!(card.summary().expected_value_primary, "build");
        assert_eq!(card.summary().runner_up, "do_nothing");
        inputs.actions.reverse();
        inputs.weights.value.states.reverse();
        for action in &mut inputs.actions {
            action.value.values.reverse();
        }
        let reordered = evaluate(&inputs).unwrap();
        assert_eq!(card.to_json(), reordered.to_json());
        assert_eq!(card.digest(), reordered.digest());
        assert!(
            card.summary()
                .alternatives
                .iter()
                .all(|a| a.max_regret.major == ZERO)
        );
    }

    #[test]
    fn a_dominated_action_never_changes_existing_same_state_regrets() {
        let mut inputs = fixture();
        let before = evaluate(&inputs).unwrap();
        let mut bad = inputs.actions[0].value.clone();
        bad.action_id = "dominated".into();
        bad.label = "Always worse".into();
        for (_, amount) in &mut bad.values {
            *amount = money(-300_000_000);
        }
        inputs.actions.push(action_claim(bad));
        let after = evaluate(&inputs).unwrap();
        for id in ["build", "do_nothing"] {
            assert_eq!(stats(&before, id), stats(&after, id));
        }
        assert_ne!(
            before.digest(),
            after.digest(),
            "even a dominated action is bound into the complete model"
        );
    }

    #[test]
    fn malformed_complete_population_and_claim_admission_refuse() {
        for mutation in 0..18 {
            let mut inputs = fixture();
            match mutation {
                0 => {
                    inputs.actions.pop();
                }
                1 => {
                    let mut w = inputs.weights.value.clone();
                    w.states[0].1 = ZERO;
                    inputs.weights = weight_claim(w);
                }
                2 => {
                    let mut w = inputs.weights.value.clone();
                    w.states[0].1 = rat(1, 2);
                    inputs.weights = weight_claim(w);
                }
                3 => {
                    let mut w = inputs.weights.value.clone();
                    w.states[1].0 = "high".into();
                    inputs.weights = weight_claim(w);
                }
                4 => {
                    let mut a = inputs.actions[0].value.clone();
                    a.values.pop();
                    inputs.actions[0] = action_claim(a);
                }
                5 => {
                    let mut a = inputs.actions[0].value.clone();
                    a.values[1].0 = "high".into();
                    inputs.actions[0] = action_claim(a);
                }
                6 => {
                    let mut a = inputs.actions[0].value.clone();
                    a.values[1].0 = "foreign".into();
                    inputs.actions[0] = action_claim(a);
                }
                7 => {
                    let mut a = inputs.actions[0].value.clone();
                    a.role = ActionRole::DoNothing;
                    inputs.actions[0] = action_claim(a);
                }
                8 => {
                    let mut a = inputs.actions[1].value.clone();
                    a.role = ActionRole::Action;
                    inputs.actions[1] = action_claim(a);
                }
                9 => {
                    let mut a = inputs.actions[0].value.clone();
                    a.values[0].1.ccy = Currency::CAD;
                    inputs.actions[0] = action_claim(a);
                }
                10 => {
                    let mut a = inputs.actions[0].value.clone();
                    a.valuation_basis.push_str("; different date");
                    inputs.actions[0] = action_claim(a);
                }
                11 => {
                    inputs.actions[0].value.values[0].1 = money(999);
                }
                12 => {
                    inputs.actions[0].stale = true;
                }
                13 => {
                    inputs.actions[0].representation = Representation::Placeholder {
                        token: "pending".into(),
                    };
                }
                14 => {
                    inputs.weights.support = SpatialSupport::PlantLot;
                }
                15 => {
                    inputs.weights.additivity = AdditivityClass::ExtensiveAdditive;
                }
                16 => {
                    inputs.actions[0].uu_terms.clear();
                }
                17 => {
                    inputs.actions[0].calibration_failed = true;
                }
                _ => unreachable!(),
            }
            assert!(evaluate(&inputs).is_err(), "mutation {mutation}");
        }
    }

    #[test]
    fn evidence_colour_all_inputs_and_nonhashed_metadata_are_retained() {
        let mut inputs = fixture();
        // A high-evidence recommended action cannot hide an assumed alternative or weight vector.
        let a = inputs.actions[0].value.clone();
        let tag = a.canonical_tag();
        inputs.actions[0] = claim(
            "build",
            a,
            &tag,
            AdditivityClass::ExtensiveAdditive,
            EvidenceClass::ModelledCalibrated,
        )
        .with_regime(
            ValidityDomain::unconstrained().with("gold_price_usd_per_oz", 1000.0, 4000.0),
            "invented-regime",
        );
        let card = evaluate(&inputs).unwrap();
        assert_eq!(card.claim().evidence, EvidenceClass::Assumed);
        assert_eq!(card.weakest_input_color(), ColorRank::Estimated);
        let mut changed = inputs.clone();
        // Generic Claim identity omits UU. The report must still change for the exact stored exposure.
        for term in &mut changed.weights.uu_terms {
            term.downside_exposure_usd_m = 35.000_000_000_001;
        }
        for action in &mut changed.actions {
            action.uu_terms = changed.weights.uu_terms.clone();
        }
        assert_eq!(changed.weights.claim_id, inputs.weights.claim_id);
        let changed_card = evaluate(&changed).unwrap();
        assert_ne!(changed_card.digest(), card.digest());
        assert_ne!(changed_card.claim().claim_id, card.claim().claim_id);
        let mut conflict = inputs;
        conflict.actions[1].uu_terms[0].description = "Competing definition of the same UU".into();
        assert_eq!(evaluate(&conflict).unwrap_err().code, "FMINE-REF-204");
    }

    #[test]
    fn overflow_refuses_instead_of_saturating_or_dropping_an_action() {
        let mut inputs = fixture();
        for (index, action) in inputs.actions.iter_mut().enumerate() {
            let mut value = action.value.clone();
            for (_, amount) in &mut value.values {
                *amount = Money::from_minor(
                    Currency::USD,
                    if index == 0 { i128::MAX } else { -i128::MAX },
                );
            }
            *action = action_claim(value);
        }
        assert_eq!(evaluate(&inputs).unwrap_err().code, "FMINE-REF-207");
    }

    fn information_claim(value: InformationOption) -> Claim<InformationOption> {
        let tag = value.canonical_tag();
        let key = format!("information.{}", value.option_id);
        claim(
            &key,
            value,
            &tag,
            AdditivityClass::NonAdditive,
            EvidenceClass::Assumed,
        )
    }

    fn test_option(inputs: &DecisionInputs, id: &str, accuracy: Rat) -> InformationOption {
        let wrong = ONE.checked_sub(accuracy).unwrap();
        InformationOption {
            model_key: inputs.weights.value.model_key.clone(),
            option_id: id.into(),
            label: format!("Invented {id}"),
            currency: Currency::USD,
            valuation_basis: inputs.actions[0].value.valuation_basis.clone(),
            test_cost: money(6_000_000),
            delay_cost: money(3_800_000),
            delay_days: 180,
            signals: vec!["bad".into(), "good".into()],
            likelihoods: [
                ("high", "good", accuracy),
                ("high", "bad", wrong),
                ("low", "good", wrong),
                ("low", "bad", accuracy),
            ]
            .into_iter()
            .map(|(state, signal, probability)| Likelihood {
                state_id: state.into(),
                signal_id: signal.into(),
                probability,
            })
            .collect(),
        }
    }

    fn information(card: &EvaluatedCard) -> &InformationSummary {
        card.summary().information.as_ref().unwrap()
    }

    fn signal<'a>(
        option: &'a information::InformationStats,
        id: &str,
    ) -> &'a information::SignalPolicy {
        option.signals.iter().find(|s| s.signal_id == id).unwrap()
    }

    #[test]
    fn gb29_exact_preposterior_value_and_costs_retain_base_risk() {
        let mut inputs = fixture();
        let base = evaluate(&inputs).unwrap();
        assert!(base.summary().information.is_none());
        assert!(base.to_json().contains("\"information\":null"));
        inputs
            .information_options
            .push(information_claim(test_option(
                &inputs,
                "met_test",
                rat(17, 20),
            )));
        let card = evaluate(&inputs).unwrap();
        let info = information(&card);
        let option = &info.options[0];
        assert_eq!(info.baseline_value.major, rat(100_000_000, 1));
        assert_eq!(info.perfect_information_value.major, rat(180_000_000, 1));
        assert_eq!(info.evpi.major, rat(80_000_000, 1));
        assert_eq!(option.gross_value.major, rat(141_000_000, 1));
        assert_eq!(option.evsi.major, rat(41_000_000, 1));
        assert_eq!(option.total_cost.major, rat(9_800_000, 1));
        assert_eq!(option.net_value.major, rat(131_200_000, 1));
        assert_eq!(option.net_voi.major, rat(31_200_000, 1));
        assert_eq!(option.strict_change_probability, rat(43, 100));
        let good = signal(option, "good");
        assert_eq!(good.probability, rat(57, 100));
        assert_eq!(
            good.posterior_state_weights.as_ref().unwrap()[0],
            ("high".into(), rat(17, 19))
        );
        assert_eq!(
            good.conditional_value.unwrap().major,
            rat(4_700_000_000, 19)
        );
        assert_eq!(good.best_actions, ["build"]);
        let bad = signal(option, "bad");
        assert_eq!(
            bad.posterior_state_weights.as_ref().unwrap()[0],
            ("high".into(), rat(9, 43))
        );
        assert_eq!(bad.best_actions, ["do_nothing"]);
        assert_eq!(bad.strict_primary_change, Some(true));
        assert_eq!(
            info.overall_winners[0].kind,
            information::ChoiceKind::InformationOption
        );
        assert_eq!(card.summary().alternatives, base.summary().alternatives);
        assert_eq!(card.claim().parents.len(), 4);
        assert!(card.to_json().contains("\"monte_carlo_se\":null"));
        assert!(card.to_markdown().contains("P(signal | state)"));
    }

    #[test]
    fn exact_no_change_threshold_and_neighbor_keep_posterior_ties() {
        for (prior, evsi, changes) in [
            (rat(34, 43), ZERO, ZERO),
            (rat(39, 50), rat(2_300_000, 1), rat(38, 125)),
        ] {
            let mut inputs = fixture();
            let mut weights = inputs.weights.value.clone();
            weights.states[0].1 = prior;
            weights.states[1].1 = ONE.checked_sub(prior).unwrap();
            inputs.weights = weight_claim(weights);
            inputs
                .information_options
                .push(information_claim(test_option(&inputs, "test", rat(17, 20))));
            let card = evaluate(&inputs).unwrap();
            let option = &information(&card).options[0];
            assert_eq!(option.evsi.major, evsi);
            assert_eq!(option.strict_change_probability, changes);
            assert_eq!(
                option.net_voi.major,
                evsi.checked_sub(rat(9_800_000, 1)).unwrap()
            );
            assert_eq!(
                option.baseline_optimal_after_every_reachable_signal,
                evsi == ZERO
            );
            if evsi == ZERO {
                assert_eq!(signal(option, "bad").best_actions, ["build", "do_nothing"]);
                assert_eq!(signal(option, "bad").strict_primary_change, Some(false));
            }
        }
    }

    #[test]
    fn perfect_uninformative_unreachable_and_expensive_options() {
        let mut inputs = fixture();
        let mut perfect = test_option(&inputs, "perfect", ONE);
        perfect.test_cost = money(0);
        perfect.delay_cost = money(0);
        perfect.signals.push("impossible".into());
        for state in ["high", "low"] {
            perfect.likelihoods.push(Likelihood {
                state_id: state.into(),
                signal_id: "impossible".into(),
                probability: ZERO,
            });
        }
        inputs.information_options.push(information_claim(perfect));
        let card = evaluate(&inputs).unwrap();
        let option = &information(&card).options[0];
        assert_eq!(option.evsi, information(&card).evpi);
        let impossible = signal(option, "impossible");
        assert_eq!(impossible.probability, ZERO);
        assert_eq!(impossible.posterior_state_weights, None);
        assert_eq!(impossible.posterior_action_values, None);
        assert!(impossible.best_actions.is_empty());
        assert_eq!(impossible.conditional_value, None);
        assert_eq!(impossible.strict_primary_change, None);

        let mut uninformative = test_option(&inputs, "uninformative", rat(1, 2));
        uninformative.test_cost = money(0);
        uninformative.delay_cost = money(0);
        inputs.information_options = vec![information_claim(uninformative)];
        let tied = evaluate(&inputs).unwrap();
        let info = information(&tied);
        assert_eq!(info.options[0].evsi.major, ZERO);
        assert_eq!(info.options[0].status, information::NetStatus::Zero);
        assert_eq!(info.overall_winners.len(), 2);
        assert_eq!(
            info.overall_winners[0].kind,
            information::ChoiceKind::ActNow
        );
        assert_eq!(
            info.overall_winners[1].kind,
            information::ChoiceKind::InformationOption
        );

        let mut expensive = test_option(&inputs, "expensive", rat(17, 20));
        expensive.test_cost = money(60_000_000);
        expensive.delay_cost = money(40_000_000);
        expensive.delay_days = 0; // Duration does not infer a cost or discount rule.
        inputs.information_options = vec![information_claim(expensive)];
        let costly = evaluate(&inputs).unwrap();
        let info = information(&costly);
        assert_eq!(info.options[0].gross_value.major, rat(141_000_000, 1));
        assert_eq!(info.options[0].net_value.major, rat(41_000_000, 1));
        assert_eq!(info.options[0].net_voi.major, rat(-59_000_000, 1));
        assert_eq!(info.options[0].status, information::NetStatus::Negative);
        assert_eq!(info.overall_winners[0].id, "build");
    }

    #[test]
    fn negative_payoffs_and_prior_ties_do_not_fabricate_zero_or_zero_evsi() {
        let mut inputs = fixture();
        let mut weights = inputs.weights.value.clone();
        for (_, weight) in &mut weights.states {
            *weight = rat(1, 2);
        }
        inputs.weights = weight_claim(weights);
        for (index, action) in inputs.actions.iter_mut().enumerate() {
            let mut value = action.value.clone();
            value.values[0].1 = money(if index == 0 { -10 } else { -100 });
            value.values[1].1 = money(if index == 0 { -100 } else { -10 });
            *action = action_claim(value);
        }
        let mut perfect = test_option(&inputs, "perfect", ONE);
        perfect.test_cost = money(1);
        perfect.delay_cost = money(2);
        inputs.information_options = vec![information_claim(perfect)];
        let card = evaluate(&inputs).unwrap();
        let info = information(&card);
        assert_eq!(info.baseline_winners, ["build", "do_nothing"]);
        assert_eq!(info.baseline_value.major, rat(-55, 1));
        assert_eq!(info.options[0].gross_value.major, rat(-10, 1));
        assert_eq!(info.options[0].evsi.major, rat(45, 1));
        assert_eq!(info.options[0].net_value.major, rat(-13, 1));
        assert_eq!(info.options[0].strict_change_probability, rat(1, 2));
        assert_eq!(info.overall_winners[0].id, "perfect");
    }

    #[test]
    fn garbling_never_increases_information_value() {
        let mut inputs = fixture();
        for (id, accuracy) in [
            ("precise", rat(9, 10)),
            ("garbled", rat(17, 20)),
            ("noise", rat(1, 2)),
        ] {
            let mut option = test_option(&inputs, id, accuracy);
            option.test_cost = money(0);
            option.delay_cost = money(0);
            inputs.information_options.push(information_claim(option));
        }
        let card = evaluate(&inputs).unwrap();
        let info = information(&card);
        assert_eq!(info.option_order, ["precise", "garbled", "noise"]);
        let by_id = |id: &str| {
            info.options
                .iter()
                .find(|o| o.option_id == id)
                .unwrap()
                .evsi
                .major
        };
        assert_eq!(by_id("precise"), rat(54_000_000, 1));
        assert_eq!(by_id("garbled"), rat(41_000_000, 1));
        assert_eq!(by_id("noise"), ZERO);
    }

    #[test]
    fn invalid_material_information_never_drops_the_option() {
        for mutation in 0..13 {
            let mut inputs = fixture();
            let mut value = test_option(&inputs, "test", rat(17, 20));
            match mutation {
                0 => {
                    value.likelihoods.pop();
                }
                1 => {
                    value.likelihoods[0] = value.likelihoods[1].clone();
                }
                2 => {
                    value.likelihoods[0].state_id = "foreign".into();
                }
                3 => {
                    value.likelihoods[0].signal_id = "foreign".into();
                }
                4 => {
                    value.likelihoods[0].probability = rat(-1, 20);
                }
                5 => {
                    value.likelihoods[0].probability = rat(21, 20);
                }
                6 => {
                    value.likelihoods[0].probability = rat(16, 20);
                }
                7 => {
                    value.signals[0] = value.signals[1].clone();
                }
                8 => {
                    value.valuation_basis.push_str(" different basis");
                }
                9 => {
                    value.test_cost.ccy = Currency::CAD;
                }
                10 => {
                    value.delay_cost = money(-1);
                }
                11 => {
                    value.model_key = "another model".into();
                }
                12 => {
                    value.option_id.clear();
                }
                _ => unreachable!(),
            }
            inputs.information_options.push(information_claim(value));
            assert!(evaluate(&inputs).is_err(), "mutation {mutation}");
        }
        let mut inputs = fixture();
        let option = information_claim(test_option(&inputs, "test", rat(17, 20)));
        inputs.information_options = vec![option.clone(), option.clone()];
        assert!(evaluate(&inputs).is_err());
        inputs.information_options = vec![option; MAX_INFORMATION_OPTIONS + 1];
        assert_eq!(evaluate(&inputs).unwrap_err().code, "FMINE-REF-203");
        inputs.information_options.truncate(1);
        inputs.information_options[0]
            .value
            .signals
            .resize(MAX_SIGNALS + 1, "signal".into());
        assert_eq!(evaluate(&inputs).unwrap_err().code, "FMINE-REF-203");
    }

    #[test]
    fn all_information_claims_and_nonhashed_metadata_bind_even_losing_options() {
        let mut inputs = fixture();
        let weights = inputs.weights.value.clone();
        let tag = weights.canonical_tag();
        inputs.weights = claim(
            "weights",
            weights,
            &tag,
            AdditivityClass::NonAdditive,
            EvidenceClass::ModelledCalibrated,
        )
        .with_regime(
            ValidityDomain::unconstrained().with("declared_model_axis", 0.0, 1.0),
            "declared-regime",
        );
        for action in &mut inputs.actions {
            let value = action.value.clone();
            let tag = value.canonical_tag();
            let key = value.action_id.clone();
            *action = claim(
                &key,
                value,
                &tag,
                AdditivityClass::ExtensiveAdditive,
                EvidenceClass::ModelledCalibrated,
            )
            .with_regime(
                ValidityDomain::unconstrained().with("declared_model_axis", 0.0, 1.0),
                "declared-regime",
            );
        }
        for (id, accuracy) in [("winner", rat(17, 20)), ("loser", rat(1, 2))] {
            let mut option = information_claim(test_option(&inputs, id, accuracy));
            option.uu_terms[0].scenario_id = format!("option_{id}");
            inputs.information_options.push(option);
        }
        let card = evaluate(&inputs).unwrap();
        assert_eq!(card.claim().evidence, EvidenceClass::Assumed);
        assert_eq!(card.weakest_input_color(), ColorRank::Estimated);
        assert_eq!(card.claim().parents.len(), 5);
        assert_eq!(card.claim().uu_terms.len(), 3);
        assert_eq!(information(&card).overall_winners[0].id, "winner");
        let mut changed = inputs.clone();
        changed.information_options[1].uu_terms[0]
            .description
            .push_str(" changed");
        assert_eq!(
            changed.information_options[1].claim_id,
            inputs.information_options[1].claim_id
        );
        let changed_card = evaluate(&changed).unwrap();
        assert_ne!(changed_card.digest(), card.digest());
        assert_ne!(changed_card.claim().claim_id, card.claim().claim_id);
        assert_eq!(changed_card.summary(), card.summary());
        changed.information_options[1].stale = true;
        assert_eq!(evaluate(&changed).unwrap_err().code, "FMINE-REF-502");
        let mut tampered = inputs.clone();
        tampered.information_options[1].value.delay_cost = money(0);
        assert_eq!(evaluate(&tampered).unwrap_err().code, "FMINE-REF-507");
        inputs.information_options.reverse();
        inputs.actions.reverse();
        inputs.weights.value.states.reverse();
        for a in &mut inputs.actions {
            a.value.values.reverse();
        }
        for o in &mut inputs.information_options {
            o.value.signals.reverse();
            o.value.likelihoods.reverse();
        }
        assert_eq!(evaluate(&inputs).unwrap().to_json(), card.to_json());
    }
}
