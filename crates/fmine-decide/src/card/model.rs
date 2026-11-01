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
use fmine_registry::toml_subset::{TomlField, TomlValue, parse_subset};
use fmine_units::{Currency, Money};

use super::{ActionPayoffs, ActionRole, DecisionInputs, WeightSet};

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
    })
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
    let fail = || {
        bad(
            DOMAIN,
            line,
            "weight must be a positive exact integer fraction such as 1/2",
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
    if numerator <= 0 || denominator <= 0 {
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
            "payoff value must be a plain decimal with at most four fractional digits that fits Money",
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
}
