//! DESIGN (BD-225/229): derive a bounded capital portfolio from retained investment calculations.
//! Determinism: DeterministicPerIsa (physical source calculations); portfolio arithmetic is exact.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::Path;

use fmine_capital::{CapitalCandidate, ProgramInputs, ProgramPolicy};
use fmine_claim::{
    AdditivityClass, Claim, Confidentiality, Representation, SpatialSupport, UuTerm,
};
use fmine_core::{ObjectId, anchor::SourceAnchor, codec::Encoder, hash::sha256};
use fmine_decide::card::WeightSet;
use fmine_evidence::{
    EpistemicStatus, EvidenceClass, KnowledgeState, KuKind, Refusal, RefusalCode,
};
use fmine_metrics::valuation::investment::{InvestmentReceipt, InvestmentSpec};
use fmine_numerics::exact_feas::Rat;
use fmine_registry::toml_subset::{TomlField, TomlValue, parse_subset};
use fmine_units::{Currency, Money};

use crate::{Envelope, investment::LoadedInvestment, json_string as js, worksheet_input};

const SCHEMA: &str = "fmine.capital-investments.v1";
const MAX_BYTES: usize = 64 * 1024;
const MAX_RETAINED_BYTES: usize = 64 * 1024 * 1024;
const NO_CLAIM: &str = "Source-derived static portfolio over declared discounted incremental cash windows. Shared scenario meaning and additivity across distinct operations are explicit assumptions, not inferred independence. Candidates sharing a boundary and circuit must be mutually exclusive. Every source investment is recomputed in full; selected source alternatives need not be source winners. Initial capital is subtracted in investment returns once and separately constrains the budget. Portfolio risk is the signed-loss CVaR of the combined same-state outcomes; standalone risk summaries are never added. Candidate monetary rounding is retained before additive portfolio aggregation. No missing-period cash, terminal value, project lifetime, full NAV, financing, liquidity schedule, synergy, physical authorization or calibrated probability is inferred.";
type Result<T> = std::result::Result<T, Refusal>;
type Fields<'a> = BTreeMap<&'a str, &'a str>;

fn invalid(why: impl Into<String>) -> Refusal {
    Refusal::new(RefusalCode::BasisMismatch105, why)
}

fn fields<'a>(input: &'a [TomlField], required: &[&str], optional: &[&str]) -> Result<Fields<'a>> {
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

fn get<'a>(f: &Fields<'a>, name: &str) -> Result<&'a str> {
    f.get(name)
        .copied()
        .ok_or_else(|| invalid(format!("missing {name}")))
}

fn number(value: &str) -> Result<Rat> {
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

fn money(value: Rat) -> Result<Money> {
    let scaled = value
        .checked_mul(Rat::from_integer(Money::SCALE))
        .map_err(|_| invalid("USD amount overflows exact minor units"))?;
    if scaled.den() != 1 {
        return Err(invalid(
            "USD spending and limits must be exactly representable to 0.0001 USD; no rounding",
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
    claim: Claim<()>,
}

pub(super) fn matches(text: &str) -> bool {
    parse_subset(text).is_ok_and(|doc| {
        doc.top_get("schema")
            .is_some_and(|f| matches!(&f.value, TomlValue::Str(s) if s == SCHEMA))
    })
}

fn parse(text: &str, source: &SourceAnchor) -> Result<Declaration> {
    if text.len() > MAX_BYTES || sha256(text.as_bytes()).0 != source.artifact_sha256 {
        return Err(invalid(
            "capital investment manifest must be source-bound and at most 64 KiB",
        ));
    }
    let doc = parse_subset(text).map_err(|e| invalid(e.to_string()))?;
    if doc.rows.len() > 8 + 64 + 16 {
        return Err(invalid(
            "at most eight candidates, 64 dependencies and 16 UU rows",
        ));
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

fn compatible(reference: &InvestmentSpec, current: &InvestmentSpec) -> bool {
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

pub(super) fn execute(text: &str, source: &SourceAnchor, root: &Path) -> Result<Envelope> {
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
    let program = fmine_capital::evaluate(&ProgramInputs {
        policy,
        weights,
        candidates,
        funding: None,
        funding_schedule: None,
        entity_funding: None,
        equity: None,
        entity_valuations: Vec::new(),
        selection_stability: None,
        robustness: None,
    })?;
    let mut enc = Encoder::new_frame(*b"PCIR", 1);
    enc.put_str(text);
    enc.put_str(program.digest());
    for snapshot in &snapshots {
        enc.put_str(&sha256(snapshot.as_bytes()).to_hex());
    }
    let digest = sha256(&enc.finish()).to_hex();
    let payload = format!(
        "{{\"schema\":\"fmine.capital-investments-command.v1\",\"mode\":\"source_derived_investment_portfolio\",\"manifest\":{{\"source\":{},\"submitted_worksheet\":{}}},\"shared_scenario_basis\":{},\"additivity_basis\":{},\"candidate_sources\":[{}],\"sources\":[{}],\"program\":{},\"digest\":{}}}",
        worksheet_input::source_json(source, text.len()),
        js(text),
        js(&d.shared_scenario_basis),
        js(&d.additivity_basis),
        mappings.join(","),
        snapshots.join(","),
        program.to_json(),
        js(&digest)
    );
    let mut markdown = format!(
        "# Source-derived investment portfolio\n\n{NO_CLAIM}\n\n**Shared scenario assumption:** {}.\n\n**Additivity assumption:** {}.\n\n**Source receipt digest:** `{digest}`.\n\n## Investment sources\n\n| Candidate | Source manifest | Selected alternative | Investment digest |\n|---|---|---|---|\n",
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
    envelope.no_claim = Some(NO_CLAIM);
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
