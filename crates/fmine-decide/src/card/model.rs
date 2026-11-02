//! Strict, source-bound declared decision models. Every imported value remains an internal assumption.
//! Determinism: Exact for probabilities and money; UU exposure uses the existing finite `UuTerm` field.

use std::collections::BTreeMap;

use fmine_claim::{
    AdditivityClass, Claim, Confidentiality, Representation, SpatialSupport, UuTerm,
};
use fmine_core::anchor::SourceAnchor;
use fmine_core::hash::sha256;
use fmine_evidence::{
    EpistemicStatus, EvidenceClass, KnowledgeState, KuKind, Refusal, RefusalCode,
};
use fmine_numerics::exact_feas::Rat;
use fmine_registry::toml_subset::{TomlField, TomlRow, TomlValue, parse_subset};
use fmine_units::{Currency, Money};

use super::{ActionPayoffs, ActionRole, DecisionInputs, InformationOption, Likelihood, WeightSet};

const INPUT: RefusalCode = RefusalCode::InputMissingOrUnreadable506;
const BASIS: RefusalCode = RefusalCode::BasisMismatch105;
const DOMAIN: RefusalCode = RefusalCode::OutsidePhysicalDomain109;
const UU: RefusalCode = RefusalCode::UnbudgetedUnknowns204;

struct DeclaredAction {
    label: String,
    role: ActionRole,
    line: usize,
}

/// Reads `fmine.decision-model.v1` with the registry TOML subset, checking the supplied source digest.
///
/// Top-level strings: `schema`, `model_key`, `question`, `currency`, `valuation_basis`.
/// Array rows: `state { id, weight = "1/2" }`, `action { id, label, role }`,
/// `payoff { action, state, value = "-120.0001" }`, and
/// `uu { id, description, trigger, downside_usd_m = "35.0" }`.
/// Roles are `action` and `do_nothing`. Every action must name every state exactly once.
/// Money is expressed in major currency units with at most four fractional digits; it never passes through
/// floating point. UU exposure is explicitly in USD millions, independently of the payoff currency.
/// Optional `information_option { id, label, test_cost, delay_cost, delay_days }` rows name tests, with
/// `signal { option, id }` and `likelihood { option, state, signal, probability }` rows. Costs are quoted
/// monetary decimals; `delay_days` is an integer. Each conditional state row must explicitly include every
/// declared signal, including zero probabilities, and sum exactly to one. Costs are never inferred from days.
///
/// The source names these assumptions; its hash does not establish that they are measured or calibrated.
/// No evidence-upgrade, retrospective, confidentiality-downgrade or authority fields are accepted.
pub fn parse(text: &str, source: SourceAnchor) -> Result<DecisionInputs, Refusal> {
    if !clean(&source.artifact_path) || !clean(&source.locator) {
        return Err(bad(
            INPUT,
            1,
            "a nonempty source path and locator are required",
        ));
    }
    if source.artifact_sha256 != sha256(text.as_bytes()).0 {
        return Err(bad(
            RefusalCode::DigestMismatch505,
            1,
            "source SHA-256 does not match the exact model bytes",
        ));
    }
    let doc = parse_subset(text).map_err(|e| bad(INPUT, e.line, &e.reason))?;
    let [schema, model_key, question, currency, valuation_basis] = strings(
        &doc.top,
        [
            "schema",
            "model_key",
            "question",
            "currency",
            "valuation_basis",
        ],
        1,
        INPUT,
    )?;
    if schema != "fmine.decision-model.v1" {
        return Err(bad(INPUT, 1, "schema must be fmine.decision-model.v1"));
    }
    key(model_key, 1)?;
    let currency = currency_code(currency)?;
    let mut states = BTreeMap::new();
    let mut actions = BTreeMap::new();
    let mut payoffs = BTreeMap::new();
    let mut unknowns = BTreeMap::new();
    let mut information = InformationRows::default();
    for row in &doc.rows {
        match row.table.as_str() {
            "state" => {
                let [id, weight] = strings(&row.fields, ["id", "weight"], row.line, INPUT)?;
                key(id, row.line)?;
                if states
                    .insert(id.to_string(), probability(weight, row.line)?)
                    .is_some()
                {
                    return Err(bad(BASIS, row.line, &format!("duplicate state {id}")));
                }
            }
            "action" => {
                let [id, label, role] =
                    strings(&row.fields, ["id", "label", "role"], row.line, INPUT)?;
                key(id, row.line)?;
                let role = match role {
                    "action" => ActionRole::Action,
                    "do_nothing" => ActionRole::DoNothing,
                    _ => return Err(bad(INPUT, row.line, "role must be action or do_nothing")),
                };
                let action = DeclaredAction {
                    label: label.to_string(),
                    role,
                    line: row.line,
                };
                if actions.insert(id.to_string(), action).is_some() {
                    return Err(bad(BASIS, row.line, &format!("duplicate action {id}")));
                }
            }
            "payoff" => {
                let [action, state, value] =
                    strings(&row.fields, ["action", "state", "value"], row.line, INPUT)?;
                key(action, row.line)?;
                key(state, row.line)?;
                let money = decimal_money(value, currency, row.line)?;
                if payoffs
                    .insert((action.to_string(), state.to_string()), (money, row.line))
                    .is_some()
                {
                    return Err(bad(
                        BASIS,
                        row.line,
                        &format!("duplicate payoff {action}/{state}"),
                    ));
                }
            }
            "information_option" => {
                information.option(row, model_key, currency, valuation_basis)?
            }
            "signal" => information.signal(row)?,
            "likelihood" => information.likelihood(row)?,
            "uu" => {
                let [id, description, trigger, exposure] = strings(
                    &row.fields,
                    ["id", "description", "trigger", "downside_usd_m"],
                    row.line,
                    UU,
                )?;
                key(id, row.line)?;
                if decimal_parts(exposure).is_none() || exposure.starts_with('-') {
                    return Err(bad(
                        UU,
                        row.line,
                        "downside_usd_m must be a positive finite decimal",
                    ));
                }
                let exposure: f64 = exposure
                    .parse()
                    .map_err(|_| bad(UU, row.line, "invalid downside_usd_m"))?;
                if !exposure.is_finite() || exposure <= 0.0 {
                    return Err(bad(
                        UU,
                        row.line,
                        "downside_usd_m must be positive and finite",
                    ));
                }
                let term = UuTerm {
                    scenario_id: id.to_string(),
                    description: description.to_string(),
                    downside_exposure_usd_m: exposure,
                    trigger_indicator: trigger.to_string(),
                };
                if unknowns.insert(id.to_string(), term).is_some() {
                    return Err(bad(UU, row.line, &format!("duplicate UU id {id}")));
                }
            }
            other => return Err(bad(INPUT, row.line, &format!("unknown table {other}"))),
        }
    }
    if states.is_empty() {
        return Err(bad(INPUT, 1, "at least one state is required"));
    }
    let total = states
        .values()
        .try_fold(Rat::from_integer(0), |sum, weight| {
            sum.checked_add(*weight)
                .map_err(|_| bad(DOMAIN, 1, "exact weight sum overflowed"))
        })?;
    if total != Rat::from_integer(1) {
        return Err(bad(DOMAIN, 1, "state weights must sum exactly to one"));
    }
    if actions.len() < 2
        || actions
            .values()
            .filter(|a| a.role == ActionRole::DoNothing)
            .count()
            != 1
    {
        return Err(bad(
            BASIS,
            1,
            "at least two actions and exactly one do_nothing action are required",
        ));
    }
    if unknowns.is_empty() {
        return Err(bad(
            UU,
            1,
            "at least one explicit UU description, trigger and USD-million exposure is required",
        ));
    }
    for ((action, state), (_, line)) in &payoffs {
        if !actions.contains_key(action) || !states.contains_key(state) {
            return Err(bad(
                BASIS,
                *line,
                &format!("payoff references unknown action/state {action}/{state}"),
            ));
        }
    }
    let unknowns: Vec<_> = unknowns.into_values().collect();
    let mut claims = Vec::new();
    for (id, action) in actions {
        let mut values = Vec::new();
        for state in states.keys() {
            let (money, _) = payoffs
                .get(&(id.clone(), state.clone()))
                .ok_or_else(|| bad(BASIS, action.line, &format!("missing payoff {id}/{state}")))?;
            values.push((state.clone(), *money));
        }
        let value = ActionPayoffs {
            model_key: model_key.to_string(),
            action_id: id.clone(),
            label: action.label,
            role: action.role,
            currency,
            valuation_basis: valuation_basis.to_string(),
            values,
        };
        let tag = value.canonical_tag();
        claims.push(assumed(
            format!("decision.{model_key}.action.{id}"),
            value,
            &tag,
            AdditivityClass::ExtensiveAdditive,
            &unknowns,
            located(&source, &format!("action {id}, line {}", action.line)),
        ));
    }
    let information_options = information.finish(&states, &unknowns, &source)?;
    let weights = WeightSet {
        model_key: model_key.to_string(),
        question: question.to_string(),
        states: states.into_iter().collect(),
    };
    let tag = weights.canonical_tag();
    Ok(DecisionInputs {
        weights: assumed(
            format!("decision.{model_key}.weights"),
            weights,
            &tag,
            AdditivityClass::NonAdditive,
            &unknowns,
            located(&source, "state rows"),
        ),
        actions: claims,
        information_options,
    })
}

#[derive(Default)]
struct InformationRows {
    options: BTreeMap<String, (InformationOption, usize)>,
    signals: BTreeMap<(String, String), usize>,
    likelihoods: BTreeMap<(String, String, String), (Rat, usize)>,
}

impl InformationRows {
    fn option(
        &mut self,
        row: &TomlRow,
        model_key: &str,
        currency: Currency,
        valuation_basis: &str,
    ) -> Result<(), Refusal> {
        let day_field = row
            .get("delay_days")
            .ok_or_else(|| bad(INPUT, row.line, "missing field delay_days"))?;
        let delay_days = match &day_field.value {
            TomlValue::Int(days) => u32::try_from(*days).map_err(|_| {
                bad(
                    DOMAIN,
                    day_field.line,
                    "delay_days must be in 0..=4294967295",
                )
            })?,
            _ => return Err(bad(INPUT, day_field.line, "delay_days must be an integer")),
        };
        let string_fields: Vec<_> = row
            .fields
            .iter()
            .filter(|f| f.key != "delay_days")
            .cloned()
            .collect();
        let [id, label, test_cost, delay_cost] = strings(
            &string_fields,
            ["id", "label", "test_cost", "delay_cost"],
            row.line,
            INPUT,
        )?;
        key(id, row.line)?;
        let test_cost = decimal_money(test_cost, currency, row.line)?;
        let delay_cost = decimal_money(delay_cost, currency, row.line)?;
        if test_cost.minor < 0 || delay_cost.minor < 0 {
            return Err(bad(
                DOMAIN,
                row.line,
                "information-option test and delay costs must be nonnegative",
            ));
        }
        let option = InformationOption {
            model_key: model_key.to_string(),
            option_id: id.to_string(),
            label: label.to_string(),
            currency,
            valuation_basis: valuation_basis.to_string(),
            test_cost,
            delay_cost,
            delay_days,
            signals: Vec::new(),
            likelihoods: Vec::new(),
        };
        if self
            .options
            .insert(id.to_string(), (option, row.line))
            .is_some()
        {
            return Err(bad(
                BASIS,
                row.line,
                &format!("duplicate information option {id}"),
            ));
        }
        Ok(())
    }

    fn signal(&mut self, row: &TomlRow) -> Result<(), Refusal> {
        let [option, id] = strings(&row.fields, ["option", "id"], row.line, INPUT)?;
        key(option, row.line)?;
        key(id, row.line)?;
        if self
            .signals
            .insert((option.to_string(), id.to_string()), row.line)
            .is_some()
        {
            return Err(bad(
                BASIS,
                row.line,
                &format!("duplicate signal {option}/{id}"),
            ));
        }
        Ok(())
    }

    fn likelihood(&mut self, row: &TomlRow) -> Result<(), Refusal> {
        let [option, state, signal, value] = strings(
            &row.fields,
            ["option", "state", "signal", "probability"],
            row.line,
            INPUT,
        )?;
        for id in [option, state, signal] {
            key(id, row.line)?;
        }
        let probability = fraction(value, row.line, true)?;
        if self
            .likelihoods
            .insert(
                (option.to_string(), state.to_string(), signal.to_string()),
                (probability, row.line),
            )
            .is_some()
        {
            return Err(bad(
                BASIS,
                row.line,
                &format!("duplicate likelihood {option}/{state}/{signal}"),
            ));
        }
        Ok(())
    }

    fn finish(
        self,
        states: &BTreeMap<String, Rat>,
        unknowns: &[UuTerm],
        source: &SourceAnchor,
    ) -> Result<Vec<Claim<InformationOption>>, Refusal> {
        if self.options.len() > super::MAX_INFORMATION_OPTIONS {
            return Err(bad(
                DOMAIN,
                1,
                "information-option count exceeds the evaluator's supported limit",
            ));
        }
        for ((option, signal), line) in &self.signals {
            if !self.options.contains_key(option) {
                return Err(bad(
                    BASIS,
                    *line,
                    &format!("signal {signal} names unknown information option {option}"),
                ));
            }
        }
        for ((option, state, signal), (_, line)) in &self.likelihoods {
            if !self.options.contains_key(option)
                || !states.contains_key(state)
                || !self.signals.contains_key(&(option.clone(), signal.clone()))
            {
                return Err(bad(
                    BASIS,
                    *line,
                    &format!(
                        "likelihood references unknown option/state/signal {option}/{state}/{signal}"
                    ),
                ));
            }
        }
        let mut claims = Vec::new();
        for (id, (mut option, line)) in self.options {
            option.signals = self
                .signals
                .keys()
                .filter(|(owner, _)| owner == &id)
                .map(|(_, signal)| signal.clone())
                .collect();
            if option.signals.is_empty() {
                return Err(bad(
                    BASIS,
                    line,
                    &format!("information option {id} needs at least one declared signal"),
                ));
            }
            if option.signals.len() > super::MAX_SIGNALS {
                return Err(bad(
                    DOMAIN,
                    line,
                    &format!("information option {id} exceeds the supported signal limit"),
                ));
            }
            for state in states.keys() {
                let mut total = Rat::from_integer(0);
                for signal in &option.signals {
                    let (probability, _) = self.likelihoods.get(&(id.clone(), state.clone(), signal.clone()))
                        .ok_or_else(|| bad(BASIS, line, &format!("missing likelihood {id}/{state}/{signal}; zeros must be explicit")))?;
                    total = total.checked_add(*probability).map_err(|_| {
                        bad(DOMAIN, line, "exact conditional likelihood sum overflowed")
                    })?;
                    option.likelihoods.push(Likelihood {
                        state_id: state.clone(),
                        signal_id: signal.clone(),
                        probability: *probability,
                    });
                }
                if total != Rat::from_integer(1) {
                    return Err(bad(
                        DOMAIN,
                        line,
                        &format!("likelihoods for {id}/{state} must sum exactly to one"),
                    ));
                }
            }
            let tag = option.canonical_tag();
            claims.push(assumed(
                format!("decision.{}.information.{id}", option.model_key),
                option,
                &tag,
                AdditivityClass::NonAdditive,
                unknowns,
                located(source, &format!("information option {id}, line {line}")),
            ));
        }
        Ok(claims)
    }
}

fn assumed<T>(
    metric: String,
    value: T,
    tag: &str,
    additivity: AdditivityClass,
    unknowns: &[UuTerm],
    source: SourceAnchor,
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
        Vec::new(),
        vec![source],
    )
}

fn located(source: &SourceAnchor, section: &str) -> SourceAnchor {
    SourceAnchor::new(
        &source.artifact_path,
        source.artifact_sha256,
        format!("{}; {section}", source.locator),
    )
}

fn clean(value: &str) -> bool {
    !value.is_empty() && value.trim() == value && !value.chars().any(char::is_control)
}

fn strings<'a, const N: usize>(
    fields: &'a [TomlField],
    names: [&str; N],
    line: usize,
    code: RefusalCode,
) -> Result<[&'a str; N], Refusal> {
    for field in fields {
        if !names.contains(&field.key.as_str()) {
            return Err(bad(
                INPUT,
                field.line,
                &format!("unknown field {}", field.key),
            ));
        }
    }
    let mut values = [""; N];
    for (i, name) in names.iter().enumerate() {
        let field = fields
            .iter()
            .find(|f| f.key == *name)
            .ok_or_else(|| bad(code, line, &format!("missing field {name}")))?;
        match &field.value {
            TomlValue::Str(value) if clean(value) => values[i] = value,
            _ => {
                return Err(bad(
                    code,
                    field.line,
                    &format!(
                        "{name} must be a nonempty, unpadded string without control characters"
                    ),
                ));
            }
        }
    }
    Ok(values)
}

fn key(value: &str, line: usize) -> Result<(), Refusal> {
    if value
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'.'))
    {
        Ok(())
    } else {
        Err(bad(
            INPUT,
            line,
            "identifiers admit only ASCII letters, digits, dot, underscore and hyphen",
        ))
    }
}

fn probability(value: &str, line: usize) -> Result<Rat, Refusal> {
    fraction(value, line, false)
}

fn fraction(value: &str, line: usize, allow_zero: bool) -> Result<Rat, Refusal> {
    let fail = || {
        bad(
            DOMAIN,
            line,
            if allow_zero {
                "likelihood probability must be an exact integer fraction in [0,1], including explicit 0/1"
            } else {
                "weight must be a positive exact integer fraction such as 1/2"
            },
        )
    };
    let (numerator, denominator) = value.split_once('/').ok_or_else(fail)?;
    if numerator.is_empty()
        || denominator.is_empty()
        || !numerator
            .bytes()
            .chain(denominator.bytes())
            .all(|b| b.is_ascii_digit())
    {
        return Err(fail());
    }
    let numerator: i128 = numerator.parse().map_err(|_| fail())?;
    let denominator: i128 = denominator.parse().map_err(|_| fail())?;
    if denominator <= 0 || numerator > denominator || (!allow_zero && numerator == 0) {
        return Err(fail());
    }
    Rat::new(numerator, denominator).map_err(|_| fail())
}

fn decimal_parts(value: &str) -> Option<(bool, &str, &str)> {
    let negative = value.starts_with('-');
    let digits = value.strip_prefix('-').unwrap_or(value);
    let (whole, fraction) = match digits.split_once('.') {
        Some((whole, fraction)) if !fraction.is_empty() => (whole, fraction),
        Some(_) => return None,
        None => (digits, ""),
    };
    if whole.is_empty()
        || (whole.len() > 1 && whole.starts_with('0'))
        || !whole
            .bytes()
            .chain(fraction.bytes())
            .all(|b| b.is_ascii_digit())
    {
        return None;
    }
    Some((negative, whole, fraction))
}

fn decimal_money(value: &str, currency: Currency, line: usize) -> Result<Money, Refusal> {
    let fail = || {
        bad(
            INPUT,
            line,
            "monetary value must be a plain decimal with at most four fractional digits that fits Money",
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
            .and_then(|m| {
                if negative {
                    m.checked_sub(digit)
                } else {
                    m.checked_add(digit)
                }
            })
            .ok_or_else(fail)?;
    }
    Ok(Money::from_minor(currency, minor))
}

fn currency_code(value: &str) -> Result<Currency, Refusal> {
    Ok(match value {
        "USD" => Currency::USD,
        "CAD" => Currency::CAD,
        "AUD" => Currency::AUD,
        "ZAR" => Currency::ZAR,
        "PKR" => Currency::PKR,
        "CLP" => Currency::CLP,
        "PEN" => Currency::PEN,
        "ARS" => Currency::ARS,
        "TZS" => Currency::TZS,
        "XAF" => Currency::XAF,
        "CDF" => Currency::CDF,
        "ZMW" => Currency::ZMW,
        "PGK" => Currency::PGK,
        "DOP" => Currency::DOP,
        "SAR" => Currency::SAR,
        "EUR" => Currency::EUR,
        "JPY" => Currency::JPY,
        _ => {
            return Err(bad(
                INPUT,
                1,
                "currency must name an existing uppercase Currency code",
            ));
        }
    })
}

fn bad(code: RefusalCode, line: usize, message: &str) -> Refusal {
    Refusal::new(code, format!("decision model line {line}: {message}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    const MODEL: &str = r#"schema = "fmine.decision-model.v1"
model_key = "synthetic-choice"
question = "Synthetic: expand or hold?"
currency = "USD"
valuation_basis = "synthetic present value at declared cut"
[[state]]
id = "low"
weight = "1/3"
[[state]]
id = "high"
weight = "2/3"
[[action]]
id = "expand"
label = "Expand"
role = "action"
[[action]]
id = "hold"
label = "Hold"
role = "do_nothing"
[[payoff]]
action = "expand"
state = "low"
value = "-120.0001"
[[payoff]]
action = "expand"
state = "high"
value = "250.1234"
[[payoff]]
action = "hold"
state = "low"
value = "0"
[[payoff]]
action = "hold"
state = "high"
value = "-0.0001"
[[uu]]
id = "UU-synthetic"
description = "Synthetic unmodelled interruption"
trigger = "Declared synthetic monitoring threshold"
downside_usd_m = "0.001"
"#;

    const INFORMATION: &str = r#"
[[information_option]]
id = "assay"
label = "Invented metallurgical test"
test_cost = "6000000"
delay_cost = "3800000.0001"
delay_days = 180
[[signal]]
option = "assay"
id = "positive"
[[signal]]
option = "assay"
id = "negative"
[[likelihood]]
option = "assay"
state = "high"
signal = "positive"
probability = "17/20"
[[likelihood]]
option = "assay"
state = "high"
signal = "negative"
probability = "3/20"
[[likelihood]]
option = "assay"
state = "low"
signal = "positive"
probability = "3/20"
[[likelihood]]
option = "assay"
state = "low"
signal = "negative"
probability = "17/20"
"#;

    fn source(text: &str) -> SourceAnchor {
        SourceAnchor::new(
            "/synthetic/choice.toml",
            sha256(text.as_bytes()).0,
            "declared model",
        )
    }

    fn refusal(text: &str) -> Refusal {
        parse(text, source(text)).expect_err("invalid declared model must refuse")
    }

    #[test]
    fn exact_payoffs_and_source_bound_assumption_claims() {
        let inputs = parse(MODEL, source(MODEL)).expect("complete invented model");
        assert!(inputs.information_options.is_empty());
        assert_eq!(
            inputs.weights.value.states[0],
            ("high".to_string(), Rat::new(2, 3).unwrap())
        );
        assert_eq!(
            inputs.weights.value_tag(),
            inputs.weights.value.canonical_tag()
        );
        assert_eq!(inputs.weights.evidence, EvidenceClass::Assumed);
        assert_eq!(inputs.weights.state, KnowledgeState::Estimated);
        assert_eq!(
            inputs.weights.epistemic,
            EpistemicStatus::KnownUnknown(KuKind::Parametric)
        );
        assert_eq!(inputs.weights.confidentiality, Confidentiality::internal());
        assert_eq!(inputs.weights.support, SpatialSupport::PeriodAggregate);
        assert_eq!(inputs.weights.additivity, AdditivityClass::NonAdditive);
        for action in &inputs.actions {
            action.check_id().expect("valid canonical Claim");
            assert_eq!(action.value_tag(), action.value.canonical_tag());
            assert_eq!(action.evidence, EvidenceClass::Assumed);
            assert_eq!(action.representation, Representation::Deterministic);
            assert_eq!(action.additivity, AdditivityClass::ExtensiveAdditive);
            assert_eq!(action.anchors[0].artifact_path, "/synthetic/choice.toml");
            assert_eq!(
                action.anchors[0].artifact_sha256,
                sha256(MODEL.as_bytes()).0
            );
            assert!(
                action.anchors[0]
                    .locator
                    .starts_with("declared model; action ")
            );
            assert_eq!(action.uu_terms[0].downside_exposure_usd_m, 0.001);
        }
        assert_eq!(inputs.actions[0].value.values[0].1.minor, 2_501_234);
        assert_eq!(inputs.actions[0].value.values[1].1.minor, -1_200_001);
        assert_eq!(inputs.actions[1].value.values[0].1.minor, -1);
    }

    #[test]
    fn money_is_exact_and_never_rounds_extra_digits_or_overflow() {
        for value in [
            "1.00001",
            "1e6",
            "+1",
            "01",
            "1.",
            ".25",
            "1,000",
            "NaN",
            "17014118346046923173168730371588411",
        ] {
            assert_eq!(
                refusal(&MODEL.replace("-120.0001", value)).code,
                INPUT.as_str(),
                "{value}"
            );
        }
        assert_eq!(
            decimal_money("17014118346046923173168730371588410.5727", Currency::USD, 1)
                .unwrap()
                .minor,
            i128::MAX
        );
        assert_eq!(
            decimal_money(
                "-17014118346046923173168730371588410.5728",
                Currency::USD,
                1
            )
            .unwrap()
            .minor,
            i128::MIN
        );
    }

    #[test]
    fn weights_roles_and_complete_matrix_are_required() {
        for weight in ["0/3", "-1/3", "1/0", "1.0/3", "1/2", "0.3333333333333333"] {
            assert_eq!(
                refusal(&MODEL.replace("1/3", weight)).code,
                DOMAIN.as_str(),
                "{weight}"
            );
        }
        assert_eq!(
            refusal(&MODEL.replace("role = \"do_nothing\"", "role = \"action\"")).code,
            BASIS.as_str()
        );
        assert_eq!(
            refusal(&MODEL.replace("state = \"low\"", "state = \"unknown\"")).code,
            BASIS.as_str()
        );
        let missing = MODEL.replace(
            "[[payoff]]\naction = \"hold\"\nstate = \"low\"\nvalue = \"0\"\n",
            "",
        );
        assert!(refusal(&missing).what.contains("missing payoff hold/low"));
        let duplicate =
            format!("{MODEL}\n[[payoff]]\naction = \"hold\"\nstate = \"low\"\nvalue = \"0\"\n");
        assert!(
            refusal(&duplicate)
                .what
                .contains("duplicate payoff hold/low")
        );
    }

    #[test]
    fn no_evidence_upgrades_or_implicit_unknowns() {
        for extra in [
            "evidence = \"measured\"",
            "calibrated = \"yes\"",
            "authoritative = \"true\"",
            "as_known_at = \"2020-01-01\"",
        ] {
            assert_eq!(refusal(&format!("{extra}\n{MODEL}")).code, INPUT.as_str());
        }
        for exposure in ["0", "-1", "NaN", "1e309"] {
            assert_eq!(
                refusal(&MODEL.replace(
                    "downside_usd_m = \"0.001\"",
                    &format!("downside_usd_m = \"{exposure}\"")
                ))
                .code,
                UU.as_str()
            );
        }
        assert_eq!(
            refusal(MODEL.split("[[uu]]").next().unwrap()).code,
            UU.as_str()
        );
        assert_eq!(
            refusal(&MODEL.replace(
                "trigger = \"Declared synthetic monitoring threshold\"\n",
                ""
            ))
            .code,
            UU.as_str()
        );
    }

    #[test]
    fn source_bytes_and_provenance_are_bound_without_inventing_validation() {
        let changed = MODEL.replace("250.1234", "250.1235");
        assert_eq!(
            parse(&changed, source(MODEL)).unwrap_err().code,
            RefusalCode::DigestMismatch505.as_str()
        );
        let original = parse(MODEL, source(MODEL)).unwrap();
        let changed = parse(&changed, source(&changed)).unwrap();
        assert_ne!(original.actions[0].claim_id, changed.actions[0].claim_id);
        let reordered = MODEL.replace(
            "id = \"low\"\nweight = \"1/3\"\n[[state]]\nid = \"high\"\nweight = \"2/3\"",
            "id = \"high\"\nweight = \"2/3\"\n[[state]]\nid = \"low\"\nweight = \"1/3\"",
        );
        let reordered = parse(&reordered, source(&reordered)).unwrap();
        assert_eq!(original.weights.value_tag(), reordered.weights.value_tag());
        assert_ne!(original.weights.claim_id, reordered.weights.claim_id);
        assert_eq!(original.actions[0].value, reordered.actions[0].value);
        let mut missing_path = source(MODEL);
        missing_path.artifact_path.clear();
        assert_eq!(parse(MODEL, missing_path).unwrap_err().code, INPUT.as_str());
    }

    #[test]
    fn information_options_keep_exact_costs_likelihoods_and_source_assumptions() {
        let text = format!("{MODEL}{INFORMATION}");
        let inputs = parse(&text, source(&text)).unwrap();
        let option = &inputs.information_options[0];
        option.check_id().unwrap();
        assert_eq!(option.value_tag(), option.value.canonical_tag());
        assert_eq!(option.evidence, EvidenceClass::Assumed);
        assert_eq!(option.confidentiality, Confidentiality::internal());
        assert_eq!(option.additivity, AdditivityClass::NonAdditive);
        assert_eq!(option.uu_terms, inputs.weights.uu_terms);
        assert_eq!(option.value.model_key, inputs.weights.value.model_key);
        assert_eq!(
            option.value.valuation_basis,
            inputs.actions[0].value.valuation_basis
        );
        assert_eq!(option.value.test_cost.minor, 60_000_000_000);
        assert_eq!(option.value.delay_cost.minor, 38_000_000_001);
        assert_eq!(option.value.delay_days, 180);
        assert_eq!(option.value.signals, ["negative", "positive"]);
        assert_eq!(option.value.likelihoods.len(), 4);
        assert_eq!(option.value.likelihoods[0].state_id, "high");
        assert_eq!(
            option.value.likelihoods[0].probability,
            Rat::new(3, 20).unwrap()
        );
        assert_eq!(option.anchors[0].artifact_sha256, sha256(text.as_bytes()).0);
        assert!(
            option.anchors[0]
                .locator
                .starts_with("declared model; information option assay, line ")
        );
        // Neither the declared cost nor its effect on NPV is inferred from the duration.
        let immediate = text.replace("delay_days = 180", "delay_days = 0");
        let immediate = parse(&immediate, source(&immediate)).unwrap();
        assert_eq!(
            immediate.information_options[0].value.delay_cost,
            option.value.delay_cost
        );
    }

    #[test]
    fn explicit_zero_likelihoods_are_retained_and_never_fill_missing_cells() {
        let extra = "\n[[signal]]\noption = \"assay\"\nid = \"unreachable\"\n";
        let zero = |state| {
            format!(
                "\n[[likelihood]]\noption = \"assay\"\nstate = \"{state}\"\nsignal = \"unreachable\"\nprobability = \"0/1\"\n"
            )
        };
        let missing = format!("{MODEL}{INFORMATION}{extra}{}", zero("high"));
        let rejected = refusal(&missing);
        assert_eq!(rejected.code, BASIS.as_str());
        assert!(
            rejected
                .what
                .contains("missing likelihood assay/low/unreachable")
        );
        let complete = format!("{missing}{}", zero("low"));
        let inputs = parse(&complete, source(&complete)).unwrap();
        let option = &inputs.information_options[0].value;
        assert_eq!(option.signals.len(), 3);
        assert_eq!(option.likelihoods.len(), 6);
        assert_eq!(
            option
                .likelihoods
                .iter()
                .filter(|r| r.probability == Rat::from_integer(0))
                .count(),
            2
        );
        let zero_state = complete.replace("weight = \"1/3\"", "weight = \"0/1\"");
        assert_eq!(refusal(&zero_state).code, DOMAIN.as_str());
    }

    #[test]
    fn orphan_duplicate_and_foreign_information_rows_refuse() {
        let complete = format!("{MODEL}{INFORMATION}");
        let option = INFORMATION.split("[[signal]]").next().unwrap();
        let signal = "\n[[signal]]\noption = \"assay\"\nid = \"positive\"\n";
        let likelihood = "\n[[likelihood]]\noption = \"assay\"\nstate = \"high\"\nsignal = \"positive\"\nprobability = \"17/20\"\n";
        for text in [
            format!("{MODEL}{signal}"),
            format!("{MODEL}{likelihood}"),
            format!("{MODEL}{option}"),
            format!("{complete}{option}"),
            format!("{complete}{signal}"),
            format!("{complete}{likelihood}"),
            complete.replace("option = \"assay\"", "option = \"missing\""),
            complete.replace("signal = \"positive\"", "signal = \"missing\""),
            complete.replace("state = \"high\"\nsignal", "state = \"missing\"\nsignal"),
        ] {
            assert_eq!(refusal(&text).code, BASIS.as_str(), "{text}");
        }
    }

    #[test]
    fn information_probability_cost_and_duration_domains_are_explicit() {
        let complete = format!("{MODEL}{INFORMATION}");
        for (old, new) in [
            ("probability = \"17/20\"", "probability = \"21/20\""),
            ("probability = \"17/20\"", "probability = \"-1/20\""),
            ("probability = \"17/20\"", "probability = \"17/0\""),
            ("probability = \"17/20\"", "probability = \"16/20\""),
            ("test_cost = \"6000000\"", "test_cost = \"-0.0001\""),
            ("delay_cost = \"3800000.0001\"", "delay_cost = \"-1\""),
            ("delay_days = 180", "delay_days = -1"),
            ("delay_days = 180", "delay_days = 4294967296"),
        ] {
            assert_eq!(
                refusal(&complete.replace(old, new)).code,
                DOMAIN.as_str(),
                "{new}"
            );
        }
        for (old, new) in [
            ("delay_days = 180", "delay_days = \"180\""),
            (
                "delay_days = 180",
                "delay_days = 180\nevidence = \"calibrated\"",
            ),
            ("test_cost = \"6000000\"\n", ""),
            ("delay_cost = \"3800000.0001\"", "delay_cost = \"1.00001\""),
        ] {
            assert_eq!(
                refusal(&complete.replace(old, new)).code,
                INPUT.as_str(),
                "{new}"
            );
        }
        let limit = complete.replace("delay_days = 180", "delay_days = 4294967295");
        assert_eq!(
            parse(&limit, source(&limit)).unwrap().information_options[0]
                .value
                .delay_days,
            u32::MAX
        );
    }

    #[test]
    fn information_row_order_is_canonical_but_source_and_amount_changes_remain_bound() {
        let text = format!("{MODEL}{INFORMATION}");
        let original = parse(&text, source(&text)).unwrap();
        let mut rows: Vec<_> = INFORMATION.split("[[").skip(1).collect();
        rows.reverse();
        let reordered = format!(
            "{MODEL}{}",
            rows.iter()
                .map(|row| format!("[[{row}"))
                .collect::<String>()
        );
        let reordered = parse(&reordered, source(&reordered)).unwrap();
        assert_eq!(
            original.information_options[0].value,
            reordered.information_options[0].value
        );
        assert_eq!(
            original.information_options[0].value_tag(),
            reordered.information_options[0].value_tag()
        );
        assert_ne!(
            original.information_options[0].claim_id,
            reordered.information_options[0].claim_id
        );
        let changed = text.replace("3800000.0001", "3800000.0002");
        assert_eq!(
            parse(&changed, source(&text)).unwrap_err().code,
            RefusalCode::DigestMismatch505.as_str()
        );
        let changed = parse(&changed, source(&changed)).unwrap();
        assert_ne!(
            original.information_options[0].value_tag(),
            changed.information_options[0].value_tag()
        );
        assert_ne!(
            original.information_options[0].claim_id,
            changed.information_options[0].claim_id
        );
    }
}
