//! Source-bound `fmine.capital-program.v1` declared assumptions, using the strict registry TOML subset.
//! Determinism: Exact (money and weights; inherited UU exposures retain their exact floating bits).

use std::collections::BTreeMap;

use fmine_evidence::KuKind;
use fmine_registry::toml_subset::{TomlField, TomlValue, parse_subset};

use super::*;

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
/// A candidate requires id/label/cost/reference_class_basis and may name one exclusion_group.
/// Costs constrain spending only; candidate payoff values are already net delta-NAV in major USD.
pub fn parse(text: &str, source: SourceAnchor) -> Result<ProgramInputs> {
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
    if doc.rows.len() > 4096 {
        return Err(input(1, "model exceeds 4096 source rows"));
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
            other => return Err(input(row.line, format!("unknown table {other}"))),
        }
    }
    if candidates.len() > MAX_CANDIDATES || states.len() > MAX_STATES || unknowns.len() > 64 {
        return Err(input(
            1,
            "exceeds 8 candidates, 256 states or 64 UU scenarios",
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
    };
    validate(&inputs)?;
    Ok(inputs)
}
