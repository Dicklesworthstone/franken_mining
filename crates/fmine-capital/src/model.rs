//! Source-bound `fmine.capital-program.v1` declared assumptions, using the strict registry TOML subset.
//! Determinism: Exact (money and weights; inherited UU exposures retain their exact floating bits).

use std::collections::BTreeMap;

use fmine_evidence::KuKind;
use fmine_registry::toml_subset::{TomlField, TomlValue, parse_subset};

use super::*;
use crate::recourse::{
    CandidateStage, DecisionStage, EvaluatedRecourseProgram, RecourseBasis, RecourseInputs,
    StateObservation,
};

mod entity;
#[cfg(test)]
mod tests;

const MAX_BYTES: usize = 4 * 1024 * 1024;
type Fields<'a> = BTreeMap<&'a str, (&'a str, usize)>;

fn input(line: usize, why: impl std::fmt::Display) -> Refusal {
    bad(
        RefusalCode::InputMissingOrUnreadable506,
        format!("line {line}: {why}"),
    )
}

fn fields<'a>(
    values: &'a [TomlField],
    required: &[&str],
    optional: &[&str],
    line: usize,
) -> Result<Fields<'a>> {
    let mut out = BTreeMap::new();
    for field in values {
        if !required.contains(&field.key.as_str()) && !optional.contains(&field.key.as_str()) {
            return Err(input(field.line, format!("unknown field {}", field.key)));
        }
        let TomlValue::Str(value) = &field.value else {
            return Err(input(field.line, "all fields must be quoted strings"));
        };
        if !named(value) {
            return Err(input(
                field.line,
                "strings must be nonempty, unpadded, free of control characters and at most 4096 bytes",
            ));
        }
        if [
            "model_key",
            "id",
            "candidate",
            "state",
            "period",
            "information_candidate",
            "observation",
            "prerequisite",
            "exclusion_group",
        ]
        .contains(&field.key.as_str())
            && !key(value)
        {
            return Err(input(
                field.line,
                "identifiers require 1..=128 ASCII letters, digits, dots, underscores or hyphens",
            ));
        }
        out.insert(field.key.as_str(), (value.as_str(), field.line));
    }
    for name in required {
        if !out.contains_key(name) {
            return Err(input(line, format!("missing field {name}")));
        }
    }
    Ok(out)
}

fn get<'a>(fields: &Fields<'a>, name: &str) -> Result<&'a str> {
    fields
        .get(name)
        .map(|(value, _)| *value)
        .ok_or_else(|| input(1, format!("missing {name}")))
}

fn decimal_parts(value: &str) -> Option<(bool, &str, &str)> {
    let (negative, digits) = value
        .strip_prefix('-')
        .map_or((false, value), |s| (true, s));
    let (whole, fraction) = digits.split_once('.').unwrap_or((digits, ""));
    if whole.is_empty()
        || !whole
            .bytes()
            .chain(fraction.bytes())
            .all(|b| b.is_ascii_digit())
        || (digits.contains('.') && fraction.is_empty())
    {
        return None;
    }
    Some((negative, whole, fraction))
}

fn money(value: &str, line: usize) -> Result<Money> {
    let fail = || {
        input(
            line,
            "money must be a plain quoted USD decimal with at most four fractional digits fitting signed i128 minor units",
        )
    };
    let (negative, whole, fraction) = decimal_parts(value).ok_or_else(fail)?;
    if fraction.len() > 4 {
        return Err(fail());
    }
    let mut minor = 0_i128;
    for digit in whole
        .bytes()
        .chain(fraction.bytes())
        .chain(std::iter::repeat_n(b'0', 4 - fraction.len()))
    {
        let digit = i128::from(digit - b'0');
        minor = minor
            .checked_mul(10)
            .and_then(|n| {
                if negative {
                    n.checked_sub(digit)
                } else {
                    n.checked_add(digit)
                }
            })
            .ok_or_else(fail)?;
    }
    Ok(Money::from_minor(Currency::USD, minor))
}

fn fraction(value: &str, line: usize) -> Result<Rat> {
    let fail = || {
        input(
            line,
            "weights and tail_mass must be exact positive n/d fractions in (0,1]",
        )
    };
    let (n, d) = value.split_once('/').ok_or_else(fail)?;
    if n.is_empty() || d.is_empty() || !n.bytes().chain(d.bytes()).all(|b| b.is_ascii_digit()) {
        return Err(fail());
    }
    let n: i128 = n.parse().map_err(|_| fail())?;
    let d: i128 = d.parse().map_err(|_| fail())?;
    if n <= 0 || d <= 0 || n > d {
        return Err(fail());
    }
    Rat::new(n, d).map_err(|_| fail())
}

fn share_fraction(value: &str, line: usize) -> Result<Rat> {
    let fail = || {
        input(
            line,
            "share quantities and per-share limits must be exact signed n/d fractions with a positive denominator fitting i128",
        )
    };
    let (n, d) = value.split_once('/').ok_or_else(fail)?;
    let digits = n.strip_prefix('-').unwrap_or(n);
    if digits.is_empty()
        || d.is_empty()
        || !digits.bytes().chain(d.bytes()).all(|b| b.is_ascii_digit())
    {
        return Err(fail());
    }
    let n: i128 = n.parse().map_err(|_| fail())?;
    let d: i128 = d.parse().map_err(|_| fail())?;
    if d <= 0 {
        return Err(fail());
    }
    Rat::new(n, d).map_err(|_| fail())
}

fn located(source: &SourceAnchor, line: usize, section: &str) -> SourceAnchor {
    SourceAnchor::new(
        &source.artifact_path,
        source.artifact_sha256,
        format!("{}; line {line}: {section}", source.locator),
    )
}

fn assumed<T>(
    metric: String,
    value: T,
    tag: &str,
    additivity: AdditivityClass,
    unknowns: &[UuTerm],
    anchors: Vec<SourceAnchor>,
) -> Claim<T> {
    Claim::new(
        metric,
        value,
        tag,
        Representation::Deterministic,
        SpatialSupport::PeriodAggregate,
        additivity,
        EvidenceClass::Assumed,
        EpistemicStatus::KnownUnknown(KuKind::Parametric),
        KnowledgeState::Estimated,
        Confidentiality::internal(),
        unknowns.to_vec(),
        vec![],
        anchors,
    )
}

/// Parses a complete source-bound static program; the source names assumptions, not calibrated evidence.
/// Top-level fields: schema/model_key/question/currency/valuation_basis/budget/tail_mass/payoff_convention/
/// reference_class_basis, plus optional max_tail_loss. Rows: state, candidate, payoff, requires and uu.
/// Optional funding/available_cash/cash_requirement rows declare a separate complete cash-window model.
/// Alternatively, funding_schedule/cash_period/opening_cash/baseline_cash_flow/candidate_cash_flow rows
/// declare signed cash flows at a complete sequence of dated checkpoints.
/// Optional equity_basis/baseline_equity_nav/candidate_share_change rows declare exact equity ownership
/// for the additive_equity_nav_with_share_changes convention and a separate USD/share risk limit.
/// An explicit entity_funding declaration instead supplies separate entity periods, cash matrices,
/// floors, fixed transfers and gated ownership distributions; it excludes both other funding modes.
/// A candidate requires id/label/cost/reference_class_basis and may name one exclusion_group.
/// Costs constrain spending only; candidate payoff values are already net delta-NAV in major USD.
pub fn parse(text: &str, source: SourceAnchor) -> Result<ProgramInputs> {
    parse_model(text, source, false).map(|(program, _)| program)
}

/// An evaluated model keeps static and explicitly staged decision objects distinct.
#[derive(Debug, Clone, PartialEq)]
pub enum EvaluatedModel {
    Static(Box<EvaluatedProgram>),
    Recourse(Box<EvaluatedRecourseProgram>),
}

impl EvaluatedModel {
    #[must_use]
    pub fn to_json(&self) -> String {
        match self {
            Self::Static(program) => program.to_json(),
            Self::Recourse(program) => program.to_json(),
        }
    }

    #[must_use]
    pub fn to_markdown(&self) -> String {
        match self {
            Self::Static(program) => program.to_markdown(),
            Self::Recourse(program) => program.to_markdown(),
        }
    }

    #[must_use]
    pub fn no_claim(&self) -> &'static str {
        match self {
            Self::Static(program) => program.no_claim(),
            Self::Recourse(program) => program.no_claim(),
        }
    }

    #[must_use]
    pub fn mode(&self) -> &'static str {
        match self {
            Self::Static(_) => "declared_static_capital_comparison",
            Self::Recourse(_) => "declared_two_stage_capital_comparison",
        }
    }
}

/// Evaluates the exact source bytes, dispatching to recourse only when explicitly declared.
/// The static-only `parse` entry point continues to reject every recourse table.
pub fn evaluate(text: &str, source: SourceAnchor) -> Result<EvaluatedModel> {
    let (program, information) = parse_model(text, source, true)?;
    match information {
        Some(information) => crate::recourse::evaluate(&RecourseInputs {
            program,
            information,
        })
        .map(|program| EvaluatedModel::Recourse(Box::new(program))),
        None => super::evaluate(&program).map(|program| EvaluatedModel::Static(Box::new(program))),
    }
}

fn parse_model(
    text: &str,
    source: SourceAnchor,
    allow_recourse: bool,
) -> Result<(ProgramInputs, Option<Claim<RecourseBasis>>)> {
    if text.len() > MAX_BYTES {
        return Err(input(1, "model exceeds the 4 MiB parser limit"));
    }
    if !named(&source.artifact_path) || !named(&source.locator) {
        return Err(input(1, "source path and locator are required"));
    }
    if source.artifact_sha256 != sha256(text.as_bytes()).0 {
        return Err(bad(
            RefusalCode::DigestMismatch505,
            "source SHA-256 differs from exact input bytes",
        ));
    }
    let doc = parse_subset(text).map_err(|e| input(e.line, e.reason))?;
    if doc.rows.len() > 45_056 {
        return Err(input(1, "model exceeds 45056 source rows"));
    }
    let top = fields(
        &doc.top,
        &[
            "schema",
            "model_key",
            "question",
            "currency",
            "valuation_basis",
            "budget",
            "tail_mass",
            "payoff_convention",
            "reference_class_basis",
        ],
        &["max_tail_loss"],
        1,
    )?;
    if get(&top, "schema")? != "fmine.capital-program.v1" {
        return Err(input(1, "schema must be fmine.capital-program.v1"));
    }
    if get(&top, "currency")? != "USD" {
        return Err(bad(
            RefusalCode::UndeclaredUnitBasisOrTz107,
            "currency must be USD; no implicit conversion",
        ));
    }
    let model_key = get(&top, "model_key")?;
    let basis = get(&top, "valuation_basis")?;
    let policy = ProgramPolicy {
        model_key: model_key.into(),
        currency: Currency::USD,
        valuation_basis: basis.into(),
        budget: money(get(&top, "budget")?, 1)?,
        tail_mass: fraction(get(&top, "tail_mass")?, 1)?,
        max_tail_loss: top
            .get("max_tail_loss")
            .map(|(v, line)| money(v, *line))
            .transpose()?,
        payoff_convention: get(&top, "payoff_convention")?.into(),
        reference_class_basis: get(&top, "reference_class_basis")?.into(),
    };
    let mut states = BTreeMap::new();
    let mut candidates = BTreeMap::new();
    let mut payoffs = BTreeMap::new();
    let mut requirements = BTreeMap::new();
    let mut unknowns = BTreeMap::new();
    let mut weight_anchors = Vec::new();
    let mut candidate_anchors: BTreeMap<String, Vec<SourceAnchor>> = BTreeMap::new();
    let mut uu_anchors = Vec::new();
    let mut funding: Option<ProgramFunding> = None;
    let mut available_cash = BTreeMap::new();
    let mut cash_requirements = BTreeMap::new();
    let mut funding_anchors = Vec::new();
    let mut funding_schedule: Option<FundingSchedule> = None;
    let mut cash_periods = BTreeMap::new();
    let mut opening_cash = BTreeMap::new();
    let mut baseline_flows = BTreeMap::new();
    let mut candidate_flows = BTreeMap::new();
    let mut schedule_anchors = Vec::new();
    let mut equity: Option<EquityBasis> = None;
    let mut baseline_nav = BTreeMap::new();
    let mut share_changes = BTreeMap::new();
    let mut equity_anchors = Vec::new();
    let mut information: Option<RecourseBasis> = None;
    let mut candidate_stages = BTreeMap::new();
    let mut observations = BTreeMap::new();
    let mut information_anchors = Vec::new();
    let mut entity_rows = entity::EntityRows::default();
    for row in &doc.rows {
        match row.table.as_str() {
            "state" => {
                let f = fields(&row.fields, &["id", "weight"], &[], row.line)?;
                let id = get(&f, "id")?;
                if states
                    .insert(id.to_string(), fraction(get(&f, "weight")?, row.line)?)
                    .is_some()
                {
                    return Err(input(row.line, "duplicate state"));
                }
                weight_anchors.push(located(&source, row.line, &format!("state {id}")));
            }
            "candidate" => {
                let f = fields(
                    &row.fields,
                    &["id", "label", "cost", "reference_class_basis"],
                    &["exclusion_group"],
                    row.line,
                )?;
                let id = get(&f, "id")?;
                let c = CapitalCandidate {
                    model_key: model_key.into(),
                    candidate_id: id.into(),
                    label: get(&f, "label")?.into(),
                    currency: Currency::USD,
                    valuation_basis: basis.into(),
                    cost: money(get(&f, "cost")?, row.line)?,
                    payoffs: vec![],
                    requires: vec![],
                    exclusion_group: f.get("exclusion_group").map(|(v, _)| (*v).into()),
                    reference_class_basis: get(&f, "reference_class_basis")?.into(),
                };
                if candidates.insert(id.to_string(), c).is_some() {
                    return Err(input(row.line, "duplicate candidate"));
                }
                candidate_anchors
                    .entry(id.into())
                    .or_default()
                    .push(located(&source, row.line, &format!("candidate {id}")));
            }
            "payoff" => {
                let f = fields(&row.fields, &["candidate", "state", "value"], &[], row.line)?;
                let id = get(&f, "candidate")?;
                let state = get(&f, "state")?;
                if payoffs
                    .insert(
                        (id.to_string(), state.to_string()),
                        money(get(&f, "value")?, row.line)?,
                    )
                    .is_some()
                {
                    return Err(input(row.line, "duplicate candidate/state payoff"));
                }
                candidate_anchors
                    .entry(id.into())
                    .or_default()
                    .push(located(&source, row.line, &format!("payoff {id}/{state}")));
            }
            "requires" => {
                let f = fields(&row.fields, &["candidate", "prerequisite"], &[], row.line)?;
                let id = get(&f, "candidate")?;
                let required = get(&f, "prerequisite")?;
                if requirements
                    .insert((id.to_string(), required.to_string()), row.line)
                    .is_some()
                {
                    return Err(input(row.line, "duplicate prerequisite"));
                }
                candidate_anchors
                    .entry(id.into())
                    .or_default()
                    .push(located(
                        &source,
                        row.line,
                        &format!("prerequisite {id}/{required}"),
                    ));
            }
            "funding" => {
                let f = fields(
                    &row.fields,
                    &["cash_period", "cash_basis", "minimum_cash"],
                    &[],
                    row.line,
                )?;
                if funding.is_some() || funding_schedule.is_some() || entity_rows.is_declared() {
                    return Err(input(
                        row.line,
                        "only one funding, funding_schedule or entity_funding declaration is allowed",
                    ));
                }
                funding = Some(ProgramFunding {
                    model_key: model_key.into(),
                    currency: Currency::USD,
                    cash_period: get(&f, "cash_period")?.into(),
                    cash_basis: get(&f, "cash_basis")?.into(),
                    minimum_cash: money(get(&f, "minimum_cash")?, row.line)?,
                    available_cash: Vec::new(),
                    requirements: Vec::new(),
                });
                funding_anchors.push(located(
                    &source,
                    row.line,
                    "funding cash window, basis and minimum cash",
                ));
            }
            "available_cash" => {
                let f = fields(&row.fields, &["state", "value"], &[], row.line)?;
                let state = get(&f, "state")?;
                if available_cash
                    .insert(state.to_string(), money(get(&f, "value")?, row.line)?)
                    .is_some()
                {
                    return Err(input(row.line, "duplicate available-cash state"));
                }
                funding_anchors.push(located(
                    &source,
                    row.line,
                    &format!("available cash {state}"),
                ));
            }
            "cash_requirement" => {
                let f = fields(&row.fields, &["candidate", "state", "value"], &[], row.line)?;
                let candidate = get(&f, "candidate")?;
                let state = get(&f, "state")?;
                if cash_requirements
                    .insert(
                        (candidate.to_string(), state.to_string()),
                        money(get(&f, "value")?, row.line)?,
                    )
                    .is_some()
                {
                    return Err(input(
                        row.line,
                        "duplicate candidate/state cash requirement",
                    ));
                }
                funding_anchors.push(located(
                    &source,
                    row.line,
                    &format!("cash requirement {candidate}/{state}"),
                ));
            }
            "funding_schedule" => {
                let f = fields(&row.fields, &["cash_basis", "opening_date"], &[], row.line)?;
                if funding.is_some() || funding_schedule.is_some() || entity_rows.is_declared() {
                    return Err(input(
                        row.line,
                        "only one funding, funding_schedule or entity_funding declaration is allowed",
                    ));
                }
                funding_schedule = Some(FundingSchedule {
                    model_key: model_key.into(),
                    currency: Currency::USD,
                    cash_basis: get(&f, "cash_basis")?.into(),
                    opening_date: get(&f, "opening_date")?.into(),
                    opening_cash: Vec::new(),
                    periods: Vec::new(),
                    baseline_flows: Vec::new(),
                    candidate_flows: Vec::new(),
                });
                schedule_anchors.push(located(
                    &source,
                    row.line,
                    "funding schedule opening date and cash basis",
                ));
            }
            "cash_period" => {
                let f = fields(
                    &row.fields,
                    &["id", "cash_period", "minimum_cash"],
                    &[],
                    row.line,
                )?;
                let id = get(&f, "id")?;
                let period = CashPeriod {
                    id: id.into(),
                    cash_period: get(&f, "cash_period")?.into(),
                    minimum_cash: money(get(&f, "minimum_cash")?, row.line)?,
                };
                if cash_periods.insert(id.to_string(), period).is_some() {
                    return Err(input(row.line, "duplicate cash period"));
                }
                schedule_anchors.push(located(&source, row.line, &format!("cash period {id}")));
            }
            "opening_cash" => {
                let f = fields(&row.fields, &["state", "value"], &[], row.line)?;
                let state = get(&f, "state")?;
                if opening_cash
                    .insert(state.to_string(), money(get(&f, "value")?, row.line)?)
                    .is_some()
                {
                    return Err(input(row.line, "duplicate opening-cash state"));
                }
                schedule_anchors.push(located(&source, row.line, &format!("opening cash {state}")));
            }
            "baseline_cash_flow" => {
                let f = fields(&row.fields, &["period", "state", "value"], &[], row.line)?;
                let period = get(&f, "period")?;
                let state = get(&f, "state")?;
                if baseline_flows
                    .insert(
                        (period.to_string(), state.to_string()),
                        money(get(&f, "value")?, row.line)?,
                    )
                    .is_some()
                {
                    return Err(input(row.line, "duplicate period/state baseline cash flow"));
                }
                schedule_anchors.push(located(
                    &source,
                    row.line,
                    &format!("baseline cash flow {period}/{state}"),
                ));
            }
            "candidate_cash_flow" => {
                let f = fields(
                    &row.fields,
                    &["period", "candidate", "state", "value"],
                    &[],
                    row.line,
                )?;
                let period = get(&f, "period")?;
                let candidate = get(&f, "candidate")?;
                let state = get(&f, "state")?;
                if candidate_flows
                    .insert(
                        (period.to_string(), candidate.to_string(), state.to_string()),
                        money(get(&f, "value")?, row.line)?,
                    )
                    .is_some()
                {
                    return Err(input(
                        row.line,
                        "duplicate period/candidate/state cash flow",
                    ));
                }
                schedule_anchors.push(located(
                    &source,
                    row.line,
                    &format!("candidate cash flow {period}/{candidate}/{state}"),
                ));
            }
            "equity_basis" => {
                let f = fields(
                    &row.fields,
                    &[
                        "entity",
                        "share_class",
                        "shares_per_unit",
                        "initial_share_units",
                    ],
                    &["max_tail_loss_per_share"],
                    row.line,
                )?;
                if equity.is_some() {
                    return Err(input(
                        row.line,
                        "only one equity_basis declaration is allowed",
                    ));
                }
                equity = Some(EquityBasis {
                    model_key: model_key.into(),
                    currency: Currency::USD,
                    valuation_basis: basis.into(),
                    entity: get(&f, "entity")?.into(),
                    share_class: get(&f, "share_class")?.into(),
                    shares_per_unit: share_fraction(get(&f, "shares_per_unit")?, row.line)?,
                    initial_share_units: share_fraction(get(&f, "initial_share_units")?, row.line)?,
                    baseline_nav: Vec::new(),
                    share_changes: Vec::new(),
                    max_tail_loss_per_share: f
                        .get("max_tail_loss_per_share")
                        .map(|(value, line)| share_fraction(value, *line))
                        .transpose()?,
                });
                equity_anchors.push(located(
                    &source,
                    row.line,
                    "equity basis and per-share risk limit",
                ));
            }
            "baseline_equity_nav" => {
                let f = fields(&row.fields, &["state", "value"], &[], row.line)?;
                let state = get(&f, "state")?;
                if baseline_nav
                    .insert(state.to_string(), money(get(&f, "value")?, row.line)?)
                    .is_some()
                {
                    return Err(input(row.line, "duplicate baseline equity NAV state"));
                }
                equity_anchors.push(located(
                    &source,
                    row.line,
                    &format!("baseline equity NAV {state}"),
                ));
            }
            "candidate_share_change" => {
                let f = fields(&row.fields, &["candidate", "state", "value"], &[], row.line)?;
                let candidate = get(&f, "candidate")?;
                let state = get(&f, "state")?;
                if share_changes
                    .insert(
                        (candidate.to_string(), state.to_string()),
                        share_fraction(get(&f, "value")?, row.line)?,
                    )
                    .is_some()
                {
                    return Err(input(row.line, "duplicate candidate/state share change"));
                }
                equity_anchors.push(located(
                    &source,
                    row.line,
                    &format!("candidate share change {candidate}/{state}"),
                ));
            }
            "recourse" if allow_recourse => {
                let f = fields(
                    &row.fields,
                    &[
                        "information_candidate",
                        "observation_date",
                        "information_basis",
                    ],
                    &[],
                    row.line,
                )?;
                if information.is_some() {
                    return Err(input(row.line, "only one recourse declaration is allowed"));
                }
                information = Some(RecourseBasis {
                    model_key: model_key.into(),
                    currency: Currency::USD,
                    valuation_basis: basis.into(),
                    information_candidate: get(&f, "information_candidate")?.into(),
                    observation_date: get(&f, "observation_date")?.into(),
                    information_basis: get(&f, "information_basis")?.into(),
                    candidate_stages: Vec::new(),
                    observations: Vec::new(),
                });
                information_anchors.push(located(
                    &source,
                    row.line,
                    "recourse information gate, observation date and basis",
                ));
            }
            "candidate_stage" if allow_recourse => {
                let f = fields(&row.fields, &["candidate", "stage"], &[], row.line)?;
                let candidate = get(&f, "candidate")?;
                let stage = match get(&f, "stage")? {
                    "initial" => DecisionStage::Initial,
                    "recourse" => DecisionStage::Recourse,
                    _ => return Err(input(row.line, "stage must be initial or recourse")),
                };
                if candidate_stages
                    .insert(candidate.to_string(), stage)
                    .is_some()
                {
                    return Err(input(row.line, "duplicate candidate stage"));
                }
                information_anchors.push(located(
                    &source,
                    row.line,
                    &format!("candidate stage {candidate}"),
                ));
            }
            "state_observation" if allow_recourse => {
                let f = fields(&row.fields, &["state", "observation"], &[], row.line)?;
                let state = get(&f, "state")?;
                let observation = get(&f, "observation")?;
                if observations
                    .insert(state.to_string(), observation.to_string())
                    .is_some()
                {
                    return Err(input(row.line, "duplicate state observation"));
                }
                information_anchors.push(located(
                    &source,
                    row.line,
                    &format!("state observation {state}/{observation}"),
                ));
            }
            "uu" => {
                let f = fields(
                    &row.fields,
                    &["id", "description", "trigger", "downside_usd_m"],
                    &[],
                    row.line,
                )?;
                let id = get(&f, "id")?;
                let raw = get(&f, "downside_usd_m")?;
                if decimal_parts(raw).is_none() || raw.starts_with('-') {
                    return Err(bad(
                        RefusalCode::UnbudgetedUnknowns204,
                        "UU downside must be a positive finite decimal in USD millions",
                    ));
                }
                let exposure: f64 = raw
                    .parse()
                    .map_err(|_| input(row.line, "invalid UU exposure"))?;
                if !exposure.is_finite() || exposure <= 0.0 {
                    return Err(bad(
                        RefusalCode::UnbudgetedUnknowns204,
                        "UU downside must be positive and finite",
                    ));
                }
                let term = UuTerm {
                    scenario_id: id.into(),
                    description: get(&f, "description")?.into(),
                    trigger_indicator: get(&f, "trigger")?.into(),
                    downside_exposure_usd_m: exposure,
                };
                if unknowns.insert(id.to_string(), term).is_some() {
                    return Err(input(row.line, "duplicate UU id"));
                }
                uu_anchors.push(located(&source, row.line, &format!("UU {id}")));
            }
            other if entity::handles_table(other) => {
                if other == "entity_funding" && (funding.is_some() || funding_schedule.is_some()) {
                    return Err(input(
                        row.line,
                        "only one funding, funding_schedule or entity_funding declaration is allowed",
                    ));
                }
                entity_rows.push(row, &source)?;
            }
            other => return Err(input(row.line, format!("unknown table {other}"))),
        }
    }
    if candidates.len() > MAX_CANDIDATES
        || states.len() > MAX_STATES
        || unknowns.len() > 64
        || cash_periods.len() > MAX_CASH_PERIODS
    {
        return Err(input(
            1,
            "exceeds 8 candidates, 256 states, 16 cash periods or 64 UU scenarios",
        ));
    }
    for ((id, state), payoff) in payoffs {
        if !states.contains_key(&state) {
            return Err(input(1, format!("payoff names unknown state {state}")));
        }
        candidates
            .get_mut(&id)
            .ok_or_else(|| input(1, format!("payoff names unknown candidate {id}")))?
            .payoffs
            .push((state, payoff));
    }
    for ((id, required), line) in requirements {
        if !candidates.contains_key(&required) {
            return Err(input(line, format!("unknown prerequisite {required}")));
        }
        candidates
            .get_mut(&id)
            .ok_or_else(|| {
                input(
                    line,
                    format!("prerequisite row names unknown candidate {id}"),
                )
            })?
            .requires
            .push(required);
    }
    let unknowns: Vec<_> = unknowns.into_values().collect();
    let entity_funding = entity_rows.finish(entity::EntityContext {
        policy: &policy,
        states: &states,
        candidates: &candidates,
        unknowns: &unknowns,
        uu_anchors: &uu_anchors,
        top: &doc.top,
        source: &source,
    })?;
    let funding = if let Some(mut declaration) = funding {
        declaration.available_cash = available_cash.into_iter().collect();
        declaration.requirements = cash_requirements
            .into_iter()
            .map(|((candidate_id, state_id), value)| CashRequirement {
                candidate_id,
                state_id,
                value,
            })
            .collect();
        let tag = declaration.canonical_tag();
        funding_anchors.extend(uu_anchors.iter().cloned());
        funding_anchors.extend(
            doc.top
                .iter()
                .filter(|f| matches!(f.key.as_str(), "model_key" | "currency"))
                .map(|f| located(&source, f.line, &f.key)),
        );
        Some(assumed(
            format!("capital.{model_key}.funding"),
            declaration,
            &tag,
            AdditivityClass::NonAdditive,
            &unknowns,
            funding_anchors,
        ))
    } else {
        if !available_cash.is_empty() || !cash_requirements.is_empty() {
            return Err(input(
                1,
                "cash rows require an explicit funding declaration with period, basis and minimum cash",
            ));
        }
        None
    };
    let funding_schedule = if let Some(mut declaration) = funding_schedule {
        declaration.opening_cash = opening_cash.into_iter().collect();
        declaration.periods = cash_periods.into_values().collect();
        declaration
            .periods
            .sort_by(|a, b| (&a.cash_period, &a.id).cmp(&(&b.cash_period, &b.id)));
        declaration.baseline_flows = baseline_flows
            .into_iter()
            .map(|((period_id, state_id), value)| StateCashFlow {
                period_id,
                state_id,
                value,
            })
            .collect();
        declaration.candidate_flows = candidate_flows
            .into_iter()
            .map(
                |((period_id, candidate_id, state_id), value)| CandidateCashFlow {
                    period_id,
                    candidate_id,
                    state_id,
                    value,
                },
            )
            .collect();
        validate_schedule_bounds(&declaration, states.len(), candidates.len())?;
        let tag = declaration.canonical_tag();
        schedule_anchors.extend(uu_anchors.iter().cloned());
        schedule_anchors.extend(
            doc.top
                .iter()
                .filter(|f| matches!(f.key.as_str(), "model_key" | "currency"))
                .map(|f| located(&source, f.line, &f.key)),
        );
        Some(assumed(
            format!("capital.{model_key}.funding_schedule"),
            declaration,
            &tag,
            AdditivityClass::NonAdditive,
            &unknowns,
            schedule_anchors,
        ))
    } else {
        if !cash_periods.is_empty()
            || !opening_cash.is_empty()
            || !baseline_flows.is_empty()
            || !candidate_flows.is_empty()
        {
            return Err(input(
                1,
                "dated cash rows require an explicit funding_schedule with opening date and cash basis",
            ));
        }
        None
    };
    let equity = if let Some(mut declaration) = equity {
        declaration.baseline_nav = baseline_nav.into_iter().collect();
        declaration.share_changes = share_changes
            .into_iter()
            .map(|((candidate_id, state_id), units)| CandidateShareChange {
                candidate_id,
                state_id,
                units,
            })
            .collect();
        validate_equity_bounds(&declaration, states.len(), candidates.len())?;
        let tag = declaration.canonical_tag();
        equity_anchors.extend(uu_anchors.iter().cloned());
        equity_anchors.extend(
            doc.top
                .iter()
                .filter(|f| {
                    matches!(
                        f.key.as_str(),
                        "model_key" | "currency" | "valuation_basis" | "payoff_convention"
                    )
                })
                .map(|f| located(&source, f.line, &f.key)),
        );
        Some(assumed(
            format!("capital.{model_key}.equity"),
            declaration,
            &tag,
            AdditivityClass::NonAdditive,
            &unknowns,
            equity_anchors,
        ))
    } else {
        if !baseline_nav.is_empty() || !share_changes.is_empty() {
            return Err(input(
                1,
                "equity rows require an explicit equity_basis with entity, share class, share scale and initial share units",
            ));
        }
        None
    };
    let information = if let Some(mut declaration) = information {
        if candidate_stages.len() != candidates.len() || observations.len() != states.len() {
            return Err(bad(
                RefusalCode::NoClaimInDomain201,
                "recourse requires every candidate stage and every shared-state observation",
            ));
        }
        for candidate in candidate_stages.keys() {
            if !candidates.contains_key(candidate) {
                return Err(input(
                    1,
                    format!("stage names unknown candidate {candidate}"),
                ));
            }
        }
        for state in observations.keys() {
            if !states.contains_key(state) {
                return Err(input(1, format!("observation names unknown state {state}")));
            }
        }
        declaration.candidate_stages = candidate_stages
            .into_iter()
            .map(|(candidate_id, stage)| CandidateStage {
                candidate_id,
                stage,
            })
            .collect();
        declaration.observations = observations
            .into_iter()
            .map(|(state_id, observation_id)| StateObservation {
                state_id,
                observation_id,
            })
            .collect();
        let tag = declaration.canonical_tag();
        information_anchors.extend(uu_anchors.iter().cloned());
        information_anchors.extend(
            doc.top
                .iter()
                .filter(|f| matches!(f.key.as_str(), "model_key" | "currency" | "valuation_basis"))
                .map(|f| located(&source, f.line, &f.key)),
        );
        Some(assumed(
            format!("capital.{model_key}.information"),
            declaration,
            &tag,
            AdditivityClass::NonAdditive,
            &unknowns,
            information_anchors,
        ))
    } else {
        if !candidate_stages.is_empty() || !observations.is_empty() {
            return Err(input(
                1,
                "stage and observation rows require an explicit recourse declaration",
            ));
        }
        None
    };
    let mut policy_anchors: Vec<_> = doc
        .top
        .iter()
        .map(|f| located(&source, f.line, &f.key))
        .collect();
    policy_anchors.extend(uu_anchors.iter().cloned());
    weight_anchors.extend(uu_anchors.iter().cloned());
    let tag = policy.canonical_tag();
    let policy = assumed(
        format!("capital.{model_key}.policy"),
        policy,
        &tag,
        AdditivityClass::NonAdditive,
        &unknowns,
        policy_anchors,
    );
    let weights = WeightSet {
        model_key: model_key.into(),
        question: get(&top, "question")?.into(),
        states: states.into_iter().collect(),
    };
    let tag = weights.canonical_tag();
    let weights = assumed(
        format!("capital.{model_key}.weights"),
        weights,
        &tag,
        AdditivityClass::NonAdditive,
        &unknowns,
        weight_anchors,
    );
    let mut claims = Vec::new();
    for (id, candidate) in candidates {
        let tag = candidate.canonical_tag();
        let mut anchors = candidate_anchors
            .remove(&id)
            .ok_or_else(|| input(1, "missing candidate anchors"))?;
        anchors.extend(uu_anchors.iter().cloned());
        claims.push(assumed(
            format!("capital.{model_key}.candidate.{id}"),
            candidate,
            &tag,
            AdditivityClass::ExtensiveAdditive,
            &unknowns,
            anchors,
        ));
    }
    let inputs = ProgramInputs {
        policy,
        weights,
        candidates: claims,
        funding,
        funding_schedule,
        entity_funding,
        equity,
    };
    validate(&inputs)?;
    Ok((inputs, information))
}
