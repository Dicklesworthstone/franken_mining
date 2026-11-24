//! DESIGN (BD-225/229): derive a bounded capital portfolio from retained investment calculations.
//! Determinism: DeterministicPerIsa (physical source calculations); portfolio arithmetic is exact.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::Path;

use fmine_capital::robustness::{MAX_TRANSPORT_STATES, METHOD, RobustnessDesign, StateDistance};
use fmine_capital::stability::{METHOD as STABILITY_METHOD, SelectionStabilityDesign};
use fmine_capital::{
    CandidateCashFlow, CapitalCandidate, CashPeriod, FundingSchedule, ProgramInputs, ProgramPolicy,
    StateCashFlow,
};
use fmine_claim::{
    AdditivityClass, Claim, Confidentiality, Representation, SpatialSupport, UuTerm,
};
use fmine_core::{ObjectId, anchor::SourceAnchor, codec::Encoder, hash::sha256};
use fmine_decide::card::WeightSet;
use fmine_evidence::{
    EpistemicStatus, EvidenceClass, KnowledgeState, KuKind, Refusal, RefusalCode,
};
use fmine_metrics::valuation::investment::{InvestmentReceipt, InvestmentSpec};
use fmine_metrics::valuation::{MonetaryRounding, round_declared_usd_cash};
use fmine_numerics::exact_feas::Rat;
use fmine_registry::toml_subset::{TomlField, TomlValue, parse_subset};
use fmine_time::CivilDate;
use fmine_units::{Currency, Money};

use crate::{Envelope, investment::LoadedInvestment, json_string as js, worksheet_input};

pub(super) const SCHEMA: &str = "fmine.capital-investments.v1";
const MAX_BYTES: usize = 64 * 1024;
const MAX_RETAINED_BYTES: usize = 64 * 1024 * 1024;
const CASH_SETTLEMENT: &str = "source_period_increment_at_cash_period_close";
const EXACT_CASH: &str = "exact_0_0001_usd";
const ROUNDED_CASH: &str = "nearest_ties_even_0_0001_usd";
const LIQUIDITY_NO_CLAIM: &str = "Source-derived static portfolio over declared discounted incremental cash windows. Optional source-bound funding pays initial capital once from opening cash and derives each candidate's nominal undiscounted operating increment from its original receipt. Each source period is assigned an explicit inclusive UTC cash window, with its increment settled at that window's close no earlier than the source period's end. Opening cash is at valuation time before initial capital; baseline cash excludes these candidate increments and initial capital. Every original state must meet the opening and period-end floors. The explicit cash-rounding policy either requires exact 0.0001 USD or rounds each nominal increment once to nearest ties even. Every unrounded amount and signed rounding delta is retained; opening cash, baseline flows and floors always require exact minor units. These settlement amounts never change already-net discounted values or joint signed-loss CVaR. Shared scenarios, additivity, settlement timing and baseline cash remain assumptions. No intra-period solvency, omitted cash, lifetime NAV, tax, financing, interaction model, calibrated probability or execution authority is inferred.";
const NO_CLAIM: &str = "Source-derived static portfolio over declared discounted incremental cash windows. Shared scenario meaning and additivity across distinct operations are explicit assumptions, not inferred independence. Candidates sharing a boundary and circuit must be mutually exclusive. Every source investment is recomputed in full; selected source alternatives need not be source winners. Initial capital is subtracted in investment returns once and separately constrains the budget. Portfolio risk is the signed-loss CVaR of the combined same-state outcomes; standalone risk summaries are never added. Candidate monetary rounding is retained before additive portfolio aggregation. Liquidity is not assessed in this comparison. No missing-period cash, terminal value, project lifetime, full NAV, financing, liquidity schedule, synergy, physical authorization or calibrated probability is inferred.";
type Result<T> = std::result::Result<T, Refusal>;
pub(super) type Fields<'a> = BTreeMap<&'a str, &'a str>;

pub(super) fn invalid(why: impl Into<String>) -> Refusal {
    Refusal::new(RefusalCode::BasisMismatch105, why)
}

pub(super) fn fields<'a>(
    input: &'a [TomlField],
    required: &[&str],
    optional: &[&str],
) -> Result<Fields<'a>> {
    let mut out = BTreeMap::new();
    for f in input {
        let TomlValue::Str(value) = &f.value else {
            return Err(invalid(format!("line {}: quoted values required", f.line)));
        };
        if !required.contains(&f.key.as_str()) && !optional.contains(&f.key.as_str()) {
            return Err(invalid(format!("line {}: unknown field {}", f.line, f.key)));
        }
        if value.is_empty()
            || value.trim() != value
            || value.len() > 4096
            || value.chars().any(char::is_control)
        {
            return Err(invalid(format!(
                "line {}: nonempty unpadded text within 4096 bytes required",
                f.line
            )));
        }
        out.insert(f.key.as_str(), value.as_str());
    }
    for k in required {
        if !out.contains_key(k) {
            return Err(invalid(format!("missing {k}")));
        }
    }
    Ok(out)
}

pub(super) fn get<'a>(f: &Fields<'a>, name: &str) -> Result<&'a str> {
    f.get(name)
        .copied()
        .ok_or_else(|| invalid(format!("missing {name}")))
}

pub(super) fn number(value: &str) -> Result<Rat> {
    if let Some((n, d)) = value.split_once('/') {
        let n = n
            .parse::<i128>()
            .map_err(|_| invalid("exact decimal or n/d required"))?;
        let d = d
            .parse::<i128>()
            .map_err(|_| invalid("exact decimal or n/d required"))?;
        if d <= 0 {
            return Err(invalid("positive rational denominator required"));
        }
        Rat::new(n, d).map_err(|_| invalid("rational exceeds supported range"))
    } else {
        Rat::from_decimal_str(value).map_err(|_| invalid("exact decimal or n/d required"))
    }
}

fn unsigned<T: std::str::FromStr>(value: &str) -> Result<T> {
    if !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(invalid(
            "selection stability controls require unsigned decimal integers",
        ));
    }
    value
        .parse()
        .map_err(|_| invalid("selection stability control exceeds its integer range"))
}

fn money(value: Rat) -> Result<Money> {
    let scaled = value
        .checked_mul(Rat::from_integer(Money::SCALE))
        .map_err(|_| invalid("USD amount overflows exact minor units"))?;
    if scaled.den() != 1 {
        return Err(invalid(
            "USD spending, cash and limits must be exactly representable to 0.0001 USD; no rounding",
        ));
    }
    Ok(Money::from_minor(Currency::USD, scaled.num()))
}

struct Candidate {
    id: String,
    label: String,
    investment: String,
    alternative: String,
    exclusion_group: Option<String>,
    requires: Vec<String>,
}

struct Declaration {
    model_key: String,
    question: String,
    shared_scenario_basis: String,
    additivity_basis: String,
    reference_class_basis: String,
    budget: Money,
    tail_mass: Rat,
    max_tail_loss: Option<Money>,
    candidates: Vec<Candidate>,
    funding_schedule: Option<FundingSchedule>,
    cash_rounding: Option<String>,
    robustness: Option<RobustnessDesign>,
    selection_stability: Option<SelectionStabilityDesign>,
    claim: Claim<()>,
}

fn parse(text: &str, source: &SourceAnchor) -> Result<Declaration> {
    if text.len() > MAX_BYTES || sha256(text.as_bytes()).0 != source.artifact_sha256 {
        return Err(invalid(
            "capital investment manifest must be source-bound and at most 64 KiB",
        ));
    }
    let doc = parse_subset(text).map_err(|e| invalid(e.to_string()))?;
    if doc.rows.len()
        > 8 + 64 + 16 + 1 + 16 + 256 * 17 + 2 + MAX_TRANSPORT_STATES * MAX_TRANSPORT_STATES
    {
        return Err(invalid("capital investment manifest has too many rows"));
    }
    let top = fields(
        &doc.top,
        &[
            "schema",
            "model_key",
            "question",
            "currency",
            "budget",
            "tail_mass",
            "payoff_convention",
            "shared_scenario_basis",
            "additivity_basis",
            "reference_class_basis",
        ],
        &["max_tail_loss"],
    )?;
    if get(&top, "schema")? != SCHEMA
        || get(&top, "currency")? != "USD"
        || get(&top, "payoff_convention")? != fmine_capital::CASH_PAYOFF_CONVENTION
    {
        return Err(invalid(
            "fmine.capital-investments.v1, USD and additive_discounted_incremental_cash required",
        ));
    }
    let (mut candidates, mut requires, mut unknowns) =
        (BTreeMap::new(), Vec::new(), BTreeMap::new());
    let mut funding_schedule = None;
    let mut cash_rounding = None;
    let mut robustness = None;
    let mut selection_stability = None;
    let mut distances = BTreeMap::new();
    let (mut cash_periods, mut opening_cash, mut baseline_flows) =
        (BTreeMap::new(), BTreeMap::new(), BTreeMap::new());
    let mut anchors = vec![source.clone()];
    for row in &doc.rows {
        match row.table.as_str() {
            "candidate" => {
                let f = fields(
                    &row.fields,
                    &["id", "label", "investment", "alternative"],
                    &["exclusion_group"],
                )?;
                let c = Candidate {
                    id: get(&f, "id")?.into(),
                    label: get(&f, "label")?.into(),
                    investment: get(&f, "investment")?.into(),
                    alternative: get(&f, "alternative")?.into(),
                    exclusion_group: f.get("exclusion_group").map(|s| (*s).into()),
                    requires: Vec::new(),
                };
                if candidates.insert(c.id.clone(), c).is_some() || candidates.len() > 8 {
                    return Err(invalid("one to eight uniquely named candidates required"));
                }
            }
            "requires" => {
                let f = fields(&row.fields, &["candidate", "prerequisite"], &[])?;
                requires.push((
                    get(&f, "candidate")?.to_owned(),
                    get(&f, "prerequisite")?.to_owned(),
                ));
                if requires.len() > 64 {
                    return Err(invalid("too many dependencies"));
                }
            }
            "uu" => {
                let f = fields(
                    &row.fields,
                    &["id", "description", "trigger", "downside_usd_m"],
                    &[],
                )?;
                let u = UuTerm {
                    scenario_id: get(&f, "id")?.into(),
                    description: get(&f, "description")?.into(),
                    trigger_indicator: get(&f, "trigger")?.into(),
                    downside_exposure_usd_m: number(get(&f, "downside_usd_m")?)?.to_f64(),
                };
                if unknowns.insert(u.scenario_id.clone(), u).is_some() || unknowns.len() > 16 {
                    return Err(invalid("one to sixteen unique UU declarations required"));
                }
            }
            "funding_schedule" => {
                let f = fields(
                    &row.fields,
                    &[
                        "cash_basis",
                        "opening_date",
                        "settlement_convention",
                        "cash_rounding",
                    ],
                    &[],
                )?;
                let rounding = get(&f, "cash_rounding")?;
                if funding_schedule.is_some()
                    || get(&f, "settlement_convention")? != CASH_SETTLEMENT
                    || !matches!(rounding, EXACT_CASH | ROUNDED_CASH)
                {
                    return Err(invalid(
                        "one funding_schedule with source_period_increment_at_cash_period_close and an explicit exact_0_0001_usd or nearest_ties_even_0_0001_usd policy required",
                    ));
                }
                cash_rounding = Some(rounding.to_owned());
                funding_schedule = Some(FundingSchedule {
                    model_key: get(&top, "model_key")?.into(),
                    currency: Currency::USD,
                    cash_basis: get(&f, "cash_basis")?.into(),
                    opening_date: get(&f, "opening_date")?.into(),
                    opening_cash: Vec::new(),
                    periods: Vec::new(),
                    baseline_flows: Vec::new(),
                    candidate_flows: Vec::new(),
                });
            }
            "cash_period" => {
                let f = fields(&row.fields, &["id", "cash_period", "minimum_cash"], &[])?;
                let p = CashPeriod {
                    id: get(&f, "id")?.into(),
                    cash_period: get(&f, "cash_period")?.into(),
                    minimum_cash: money(number(get(&f, "minimum_cash")?)?)?,
                };
                if cash_periods.insert(p.id.clone(), p).is_some()
                    || cash_periods.len() > fmine_capital::MAX_CASH_PERIODS
                {
                    return Err(invalid("one to sixteen unique cash periods required"));
                }
            }
            "opening_cash" => {
                let f = fields(&row.fields, &["state", "value"], &[])?;
                if opening_cash
                    .insert(
                        get(&f, "state")?.to_owned(),
                        money(number(get(&f, "value")?)?)?,
                    )
                    .is_some()
                    || opening_cash.len() > fmine_capital::MAX_STATES
                {
                    return Err(invalid("opening cash requires each state once"));
                }
            }
            "baseline_cash_flow" => {
                let f = fields(&row.fields, &["period", "state", "value"], &[])?;
                let flow = StateCashFlow {
                    period_id: get(&f, "period")?.into(),
                    state_id: get(&f, "state")?.into(),
                    value: money(number(get(&f, "value")?)?)?,
                };
                if baseline_flows
                    .insert((flow.period_id.clone(), flow.state_id.clone()), flow)
                    .is_some()
                    || baseline_flows.len()
                        > fmine_capital::MAX_CASH_PERIODS * fmine_capital::MAX_STATES
                {
                    return Err(invalid("baseline cash requires every period/state once"));
                }
            }
            "robustness" => {
                let f = fields(
                    &row.fields,
                    &[
                        "method",
                        "radius",
                        "metric_basis",
                        "radius_basis",
                        "support",
                        "distance_unit",
                    ],
                    &[],
                )?;
                if robustness.is_some()
                    || get(&f, "method")? != METHOD
                    || get(&f, "support")? != "declared_shared_scenarios"
                    || get(&f, "distance_unit")? != "dimensionless"
                {
                    return Err(invalid(
                        "one wasserstein_1 robustness declaration over declared_shared_scenarios with dimensionless distances required",
                    ));
                }
                let radius = number(get(&f, "radius")?)?;
                if radius.signum() < 0 {
                    return Err(invalid("transport radius must be nonnegative"));
                }
                robustness = Some(RobustnessDesign {
                    model_key: get(&top, "model_key")?.into(),
                    method: METHOD.into(),
                    radius,
                    metric_basis: get(&f, "metric_basis")?.into(),
                    radius_basis: get(&f, "radius_basis")?.into(),
                    distances: Vec::new(),
                });
            }
            "transport_distance" => {
                let f = fields(&row.fields, &["from", "to", "value"], &[])?;
                let distance = number(get(&f, "value")?)?;
                if distance.signum() < 0 {
                    return Err(invalid("transport distances must be nonnegative"));
                }
                if distances
                    .insert(
                        (get(&f, "from")?.to_owned(), get(&f, "to")?.to_owned()),
                        distance,
                    )
                    .is_some()
                    || distances.len() > MAX_TRANSPORT_STATES * MAX_TRANSPORT_STATES
                {
                    return Err(invalid(
                        "unique transport distances within a 64 by 64 state matrix required",
                    ));
                }
            }
            "selection_stability" => {
                let f = fields(
                    &row.fields,
                    &[
                        "method",
                        "seed",
                        "replicates",
                        "draws_per_replicate",
                        "sampling_basis",
                        "support",
                        "resampling_unit",
                    ],
                    &[],
                )?;
                if selection_stability.is_some()
                    || get(&f, "method")? != STABILITY_METHOD
                    || get(&f, "support")? != "declared_shared_scenarios"
                    || get(&f, "resampling_unit")? != "complete_joint_state"
                {
                    return Err(invalid(
                        "one weighted_multinomial selection_stability declaration over declared_shared_scenarios with complete_joint_state resampling required",
                    ));
                }
                let replicates: usize = unsigned(get(&f, "replicates")?)?;
                let draws_per_replicate: usize = unsigned(get(&f, "draws_per_replicate")?)?;
                if !(200..=2000).contains(&replicates) || !(1..=4096).contains(&draws_per_replicate)
                {
                    return Err(invalid(
                        "selection stability requires 200..=2000 replicates and 1..=4096 draws per replicate",
                    ));
                }
                selection_stability = Some(SelectionStabilityDesign {
                    model_key: get(&top, "model_key")?.into(),
                    method: STABILITY_METHOD.into(),
                    seed: unsigned(get(&f, "seed")?)?,
                    replicates,
                    draws_per_replicate,
                    sampling_basis: get(&f, "sampling_basis")?.into(),
                });
            }
            _ => return Err(invalid(format!("unsupported table {}", row.table))),
        }
        for f in &row.fields {
            anchors.push(SourceAnchor::new(
                &source.artifact_path,
                source.artifact_sha256,
                format!("line {}: {}.{}", f.line, row.table, f.key),
            ));
        }
    }
    if candidates.is_empty() || unknowns.is_empty() {
        return Err(invalid(
            "candidates and explicit portfolio UU assumptions are required",
        ));
    }
    if let Some(schedule) = &mut funding_schedule {
        schedule.periods = cash_periods.into_values().collect();
        schedule.opening_cash = opening_cash.into_iter().collect();
        schedule.baseline_flows = baseline_flows.into_values().collect();
    } else if !cash_periods.is_empty() || !opening_cash.is_empty() || !baseline_flows.is_empty() {
        return Err(invalid("cash rows require an explicit funding_schedule"));
    }
    if let Some(design) = &mut robustness {
        design.distances = distances
            .into_iter()
            .map(|((from_state, to_state), distance)| StateDistance {
                from_state,
                to_state,
                distance,
            })
            .collect();
    } else if !distances.is_empty() {
        return Err(invalid(
            "transport distances require an explicit robustness declaration",
        ));
    }
    for (candidate, prerequisite) in requires {
        if !candidates.contains_key(&prerequisite) {
            return Err(invalid(format!("unknown prerequisite {prerequisite}")));
        }
        candidates
            .get_mut(&candidate)
            .ok_or_else(|| invalid(format!("unknown candidate {candidate}")))?
            .requires
            .push(prerequisite);
    }
    for f in &doc.top {
        anchors.push(SourceAnchor::new(
            &source.artifact_path,
            source.artifact_sha256,
            format!("line {}: {}", f.line, f.key),
        ));
    }
    let claim = Claim::new(
        "capital.investment.source/1",
        (),
        &format!("capital-investment-source/v1:{}", sha256(text.as_bytes())),
        Representation::Deterministic,
        SpatialSupport::PeriodAggregate,
        AdditivityClass::NonAdditive,
        EvidenceClass::Assumed,
        EpistemicStatus::KnownUnknown(KuKind::Parametric),
        KnowledgeState::Estimated,
        Confidentiality::internal(),
        unknowns.into_values().collect(),
        Vec::new(),
        anchors,
    );
    Ok(Declaration {
        model_key: get(&top, "model_key")?.into(),
        question: get(&top, "question")?.into(),
        shared_scenario_basis: get(&top, "shared_scenario_basis")?.into(),
        additivity_basis: get(&top, "additivity_basis")?.into(),
        reference_class_basis: get(&top, "reference_class_basis")?.into(),
        budget: money(number(get(&top, "budget")?)?)?,
        tail_mass: number(get(&top, "tail_mass")?)?,
        max_tail_loss: top
            .get("max_tail_loss")
            .map(|s| money(number(s)?))
            .transpose()?,
        candidates: candidates.into_values().collect(),
        funding_schedule,
        cash_rounding,
        robustness,
        selection_stability,
        claim,
    })
}

struct Metadata {
    evidence: EvidenceClass,
    epistemic: EpistemicStatus,
    confidentiality: Confidentiality,
    uu: BTreeMap<String, UuTerm>,
    parents: BTreeSet<ObjectId>,
    anchors: BTreeSet<SourceAnchor>,
}

impl Metadata {
    fn new() -> Self {
        Self {
            evidence: EvidenceClass::Assumed,
            epistemic: EpistemicStatus::KnownUnknown(KuKind::Parametric),
            confidentiality: Confidentiality::internal(),
            uu: BTreeMap::new(),
            parents: BTreeSet::new(),
            anchors: BTreeSet::new(),
        }
    }
    fn include<T>(&mut self, claim: &Claim<T>) -> Result<()> {
        claim.check_id()?;
        if claim.stale
            || claim.calibration_failed
            || !matches!(
                claim.state,
                KnowledgeState::Known | KnowledgeState::Estimated
            )
        {
            return Err(invalid(
                "source Claim is stale, unusable or failed calibration",
            ));
        }
        self.evidence = self.evidence.meet(claim.evidence);
        self.epistemic = self.epistemic.and(claim.epistemic);
        self.confidentiality = self.confidentiality.join(&claim.confidentiality);
        self.parents.insert(claim.claim_id);
        self.anchors.extend(claim.anchors.iter().cloned());
        for u in &claim.uu_terms {
            if let Some(old) = self.uu.insert(u.scenario_id.clone(), u.clone())
                && old != *u
            {
                return Err(invalid(format!(
                    "conflicting UU definition {}",
                    u.scenario_id
                )));
            }
        }
        Ok(())
    }
    fn claim<T>(&self, metric: &str, value: T, tag: &str, add: AdditivityClass) -> Claim<T> {
        Claim::new(
            metric,
            value,
            tag,
            Representation::Deterministic,
            SpatialSupport::PeriodAggregate,
            add,
            self.evidence,
            self.epistemic,
            KnowledgeState::Estimated,
            self.confidentiality.clone(),
            self.uu.values().cloned().collect(),
            self.parents.iter().copied().collect(),
            self.anchors.iter().cloned().collect(),
        )
    }
}

pub(super) fn compatible(reference: &InvestmentSpec, current: &InvestmentSpec) -> bool {
    let mut a = reference.periods.clone();
    let mut b = current.periods.clone();
    a.sort_by(|x, y| x.id.cmp(&y.id));
    b.sort_by(|x, y| x.id.cmp(&y.id));
    reference.currency == current.currency
        && reference.valuation_time_ns == current.valuation_time_ns
        && reference.discount_basis == current.discount_basis
        && reference.cashflow_basis == current.cashflow_basis
        && reference.monetary_rounding == current.monetary_rounding
        && a == b
}

fn selected<'a>(
    loaded: &'a BTreeMap<String, LoadedInvestment>,
    candidate: &Candidate,
) -> Result<&'a InvestmentReceipt> {
    loaded
        .get(&candidate.investment)
        .map(LoadedInvestment::receipt)
        .ok_or_else(|| invalid("missing investment source"))
}

fn utc_date(ns: i64) -> String {
    let date = CivilDate::from_epoch_days(ns.div_euclid(86_400_000_000_000));
    format!("{:04}-{:02}-{:02}", date.year, date.month, date.day)
}

fn derive_funding(
    declaration: &Declaration,
    loaded: &BTreeMap<String, LoadedInvestment>,
    reference: &InvestmentReceipt,
    metadata: &Metadata,
) -> Result<(Option<Claim<FundingSchedule>>, String)> {
    let Some(mut schedule) = declaration.funding_schedule.clone() else {
        return Ok((None, String::new()));
    };
    let rounding = declaration
        .cash_rounding
        .as_deref()
        .ok_or_else(|| invalid("source cash schedule is missing its rounding policy"))?;
    let spec = &reference.declaration().value;
    let states = reference.decision().inputs().weights.value.states.len();
    let work = (1_usize << declaration.candidates.len())
        .checked_mul(states)
        .and_then(|n| n.checked_mul(spec.periods.len() + 1));
    if work.is_none_or(|n| n > fmine_capital::MAX_CASH_CHECKPOINTS) {
        return Err(invalid(
            "source cash schedules exceed 65536 cash checkpoints",
        ));
    }
    if schedule.opening_date != utc_date(spec.valuation_time_ns)
        || schedule.periods.len() != spec.periods.len()
    {
        return Err(invalid(
            "opening_date must be the UTC valuation date and every source period needs one cash period",
        ));
    }
    for source_period in &spec.periods {
        let cash_period = schedule
            .periods
            .iter()
            .find(|p| p.id == source_period.id)
            .ok_or_else(|| invalid("cash period ids must match every source investment period"))?;
        let (_, close) = cash_period
            .cash_period
            .split_once('/')
            .ok_or_else(|| invalid("cash periods require inclusive YYYY-MM-DD/YYYY-MM-DD dates"))?;
        let last_ns = source_period
            .end_ns
            .checked_sub(1)
            .ok_or_else(|| invalid("source period end is outside the supported time range"))?;
        if !fmine_registry::sources::is_date(close) || close < utc_date(last_ns).as_str() {
            return Err(invalid(
                "cash settlement cannot precede the end of its source operating period",
            ));
        }
    }
    let mut rows = Vec::new();
    for candidate in &declaration.candidates {
        for flow in selected(loaded, candidate)?
            .cashflows()
            .iter()
            .filter(|flow| flow.alternative_id == candidate.alternative)
        {
            let (value, delta) = if rounding == EXACT_CASH {
                (money(flow.incremental_cash_usd)?, Rat::from_integer(0))
            } else {
                round_declared_usd_cash(
                    flow.incremental_cash_usd,
                    MonetaryRounding::NearestTiesEvenFourPlaces,
                )?
            };
            let rounded = Rat::new(value.minor, Money::SCALE)
                .map_err(|_| invalid("nominal cash exceeds the supported rational range"))?;
            rows.push(format!(
                "{{\"candidate_id\":{},\"period_id\":{},\"state_id\":{},\"unrounded_increment_usd\":{},\"cash_increment_usd\":{},\"rounding_delta_usd\":{}}}",
                js(&candidate.id), js(&flow.period_id), js(&flow.scenario_id),
                rational(flow.incremental_cash_usd), rational(rounded), rational(delta)
            ));
            schedule.candidate_flows.push(CandidateCashFlow {
                period_id: flow.period_id.clone(),
                candidate_id: candidate.id.clone(),
                state_id: flow.scenario_id.clone(),
                value,
            });
        }
    }
    let tag = schedule.canonical_tag();
    let claim = metadata.claim(
        "capital.investment.funding_schedule/1",
        schedule,
        &tag,
        AdditivityClass::NonAdditive,
    );
    let derivation = format!(
        "{{\"schema\":\"fmine.investment-nominal-cash.v1\",\"settlement_convention\":{},\"cash_rounding\":{},\"rows\":[{}]}}",
        js(CASH_SETTLEMENT),
        js(rounding),
        rows.join(",")
    );
    Ok((Some(claim), derivation))
}

fn rational(value: Rat) -> String {
    js(&format!("{}/{}", value.num(), value.den()))
}

pub(super) fn execute(
    text: &str,
    source: &SourceAnchor,
    root: &Path,
    brief: bool,
) -> Result<Envelope> {
    let d = parse(text, source)?;
    let mut loaded = BTreeMap::new();
    let mut snapshots = Vec::new();
    let mut retained = 0_usize;
    for c in &d.candidates {
        if let std::collections::btree_map::Entry::Vacant(entry) =
            loaded.entry(c.investment.clone())
        {
            let investment = crate::investment::load(Path::new(&c.investment), root)?;
            let snapshot = investment.to_json();
            retained = retained
                .checked_add(snapshot.len())
                .ok_or_else(|| invalid("source snapshot size overflow"))?;
            if retained > MAX_RETAINED_BYTES {
                return Err(invalid("capital investment source snapshots exceed 64 MiB"));
            }
            snapshots.push(format!(
                "{{\"investment_manifest\":{},\"calculation\":{snapshot}}}",
                js(&c.investment)
            ));
            entry.insert(investment);
        }
    }
    let reference = loaded
        .values()
        .next()
        .ok_or_else(|| invalid("no investment sources"))?
        .receipt();
    let spec = &reference.declaration().value;
    let states = &reference.decision().inputs().weights.value.states;
    let mut all = Metadata::new();
    all.include(&d.claim)?;
    for investment in loaded.values() {
        let r = investment.receipt();
        let s = &r.declaration().value;
        if s.downside_limit.is_some() {
            return Err(invalid(
                "source investment downside limits are not portfolio constraints; use an unscreened source and declare the joint portfolio tail-loss limit",
            ));
        }
        if !s.information_options.is_empty() {
            return Err(invalid(
                "source investment information options are not portfolio candidates; a joint signal and contingent portfolio policy must be declared before combining testwork with investments",
            ));
        }
        if s.shared_scenario_basis != d.shared_scenario_basis
            || !compatible(spec, s)
            || r.decision().inputs().weights.value.states != *states
        {
            return Err(invalid(
                "investment sources require identical complete shared states/weights, declared scenario meaning, valuation time, periods and explicit discount/cash bases",
            ));
        }
        all.include(r.declaration())?;
        all.include(r.decision().claim())?;
    }
    for (i, a) in d.candidates.iter().enumerate() {
        let sa = &selected(&loaded, a)?.declaration().value;
        for b in &d.candidates[i + 1..] {
            let sb = &selected(&loaded, b)?.declaration().value;
            if sa.boundary == sb.boundary
                && sa.circuit == sb.circuit
                && (a.exclusion_group.is_none() || a.exclusion_group != b.exclusion_group)
            {
                return Err(invalid(format!(
                    "overlapping operating support for {} and {} requires one explicit shared exclusion group",
                    a.id, b.id
                )));
            }
        }
    }
    let mut candidates = Vec::new();
    let mut mappings = Vec::new();
    for c in &d.candidates {
        let r = selected(&loaded, c)?;
        let alternative = r
            .declaration()
            .value
            .alternatives
            .iter()
            .find(|a| a.id == c.alternative)
            .ok_or_else(|| invalid(format!("unknown source alternative {}", c.alternative)))?;
        let candidate = CapitalCandidate {
            model_key: d.model_key.clone(),
            candidate_id: c.id.clone(),
            label: c.label.clone(),
            currency: Currency::USD,
            valuation_basis: fmine_capital::CASH_VALUATION_BASIS.into(),
            cost: money(alternative.initial_capex_usd)?,
            payoffs: r
                .outcomes()
                .iter()
                .filter(|p| p.alternative_id == c.alternative)
                .map(|p| (p.scenario_id.clone(), p.value))
                .collect(),
            requires: c.requires.clone(),
            exclusion_group: c.exclusion_group.clone(),
            reference_class_basis: d.reference_class_basis.clone(),
        };
        let mut meta = Metadata::new();
        meta.include(&d.claim)?;
        meta.include(r.declaration())?;
        meta.include(r.decision().claim())?;
        let tag = candidate.canonical_tag();
        candidates.push(meta.claim(
            "capital.investment.candidate/1",
            candidate,
            &tag,
            AdditivityClass::ExtensiveAdditive,
        ));
        mappings.push(format!("{{\"candidate_id\":{},\"investment_manifest\":{},\"alternative_id\":{},\"investment_digest\":{}}}",
            js(&c.id), js(&c.investment), js(&c.alternative), js(r.digest())));
    }
    let policy = ProgramPolicy {
        model_key: d.model_key.clone(),
        currency: Currency::USD,
        valuation_basis: fmine_capital::CASH_VALUATION_BASIS.into(),
        budget: d.budget,
        tail_mass: d.tail_mass,
        max_tail_loss: d.max_tail_loss,
        payoff_convention: fmine_capital::CASH_PAYOFF_CONVENTION.into(),
        reference_class_basis: d.reference_class_basis.clone(),
    };
    let tag = policy.canonical_tag();
    let policy = all.claim(
        "capital.investment.policy/1",
        policy,
        &tag,
        AdditivityClass::NonAdditive,
    );
    let weights = WeightSet {
        model_key: d.model_key.clone(),
        question: d.question.clone(),
        states: states.clone(),
    };
    let tag = weights.canonical_tag();
    let weights = all.claim(
        "capital.investment.weights/1",
        weights,
        &tag,
        AdditivityClass::NonAdditive,
    );
    let (funding_schedule, cash_derivation) = derive_funding(&d, &loaded, reference, &all)?;
    let robustness = d.robustness.as_ref().map(|design| {
        all.claim(
            "capital.investment.robustness/1",
            design.clone(),
            &design.canonical_tag(),
            AdditivityClass::NonAdditive,
        )
    });
    let selection_stability = d.selection_stability.as_ref().map(|design| {
        all.claim(
            "capital.investment.selection-stability/1",
            design.clone(),
            &design.canonical_tag(),
            AdditivityClass::NonAdditive,
        )
    });
    let no_claim = if funding_schedule.is_some() {
        LIQUIDITY_NO_CLAIM
    } else {
        NO_CLAIM
    };
    let program = fmine_capital::evaluate(&ProgramInputs {
        policy,
        weights,
        candidates,
        funding: None,
        funding_schedule,
        entity_funding: None,
        equity: None,
        entity_valuations: Vec::new(),
        selection_stability,
        robustness,
    })?;
    let mut enc = Encoder::new_frame(*b"PCIR", 1);
    enc.put_str(text);
    enc.put_str(program.digest());
    for snapshot in &snapshots {
        enc.put_str(&sha256(snapshot.as_bytes()).to_hex());
    }
    let cash_field = if cash_derivation.is_empty() {
        String::new()
    } else {
        enc.put_str(&cash_derivation);
        format!(",\"source_cash\":{cash_derivation}")
    };
    let digest = sha256(&enc.finish()).to_hex();
    let payload = format!(
        "{{\"schema\":\"fmine.capital-investments-command.v1\",\"mode\":\"source_derived_investment_portfolio\",\"manifest\":{{\"source\":{},\"submitted_worksheet\":{}}},\"shared_scenario_basis\":{},\"additivity_basis\":{},\"candidate_sources\":[{}],\"sources\":[{}],\"program\":{}{cash_field},\"digest\":{}}}",
        worksheet_input::source_json(source, text.len()),
        js(text),
        js(&d.shared_scenario_basis),
        js(&d.additivity_basis),
        mappings.join(","),
        snapshots.join(","),
        program.to_json(),
        js(&digest)
    );
    if brief {
        let mut envelope = Envelope::ok("decide program", payload, Vec::new());
        envelope.no_claim = Some(no_claim);
        envelope.rendered_markdown = Some(format!(
            "{}\n\n**Shared scenario assumption** {}\n\n**Additivity assumption** {}\n\nSource receipt digest `{digest}`.\n\n{no_claim}\n",
            program.to_brief()?,
            md(&d.shared_scenario_basis),
            md(&d.additivity_basis),
        ));
        return Ok(envelope);
    }
    let rounding_notice = d.cash_rounding.as_ref().map_or_else(String::new, |rounding| {
        format!("**Cash settlement rounding:** `{}`. Unrounded cash and signed rounding deltas are retained in the JSON source-cash rows.\n\n", md(rounding))
    });
    let mut markdown = format!(
        "# Source-derived investment portfolio\n\n{rounding_notice}{no_claim}\n\n**Shared scenario assumption:** {}.\n\n**Additivity assumption:** {}.\n\n**Source receipt digest:** `{digest}`.\n\n## Investment sources\n\n| Candidate | Source manifest | Selected alternative | Investment digest |\n|---|---|---|---|\n",
        md(&d.shared_scenario_basis),
        md(&d.additivity_basis)
    );
    for c in &d.candidates {
        let _ = writeln!(
            markdown,
            "| {} | {} | {} | {} |",
            md(&c.id),
            md(&c.investment),
            md(&c.alternative),
            selected(&loaded, c)?.digest()
        );
    }
    markdown.push('\n');
    markdown.push_str(&program.to_markdown());
    let mut envelope = Envelope::ok("decide program", payload, Vec::new());
    envelope.no_claim = Some(no_claim);
    envelope.rendered_markdown = Some(markdown);
    Ok(envelope)
}

fn md(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('|', "\\|")
        .replace('`', "\\`")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}
