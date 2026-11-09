//! Strict, source-bound declarations for two-stratum recovery decisions (BD-351.S1).
//! All imported values are internal assumptions; exact arithmetic does not qualify their evidence.
//! Determinism: Exact

use std::collections::BTreeSet;

use fmine_claim::{
    AdditivityClass, Claim, Confidentiality, Representation, SpatialSupport, UuTerm,
};
use fmine_core::anchor::SourceAnchor;
use fmine_core::hash::sha256;
use fmine_evidence::{
    EpistemicStatus, EvidenceClass, KnowledgeState, KuKind, Refusal, RefusalCode,
};
use fmine_registry::toml_subset::{TomlField, TomlValue, parse_subset};

use super::{INPUT_METRIC, Interval, Rat, RecoveryBoundModel, WeightBasis};

pub const SCHEMA: &str = "fmine.recovery-bounds.v1";
pub const ROBOT_DOCS_JSON: &str = concat!(
    "{\"schema\":\"fmine.recovery-bounds.v1\",\"command\":\"decide recovery-bounds <model-file>\",",
    "\"input\":\"Bounded UTF-8 worksheet; source SHA-256 and original field tokens retained\",",
    "\"fields\":[\"schema\",\"model_key\",\"question\",\"route\",\"metal\",\"additivity_basis\",\"untested_bound_basis\"],",
    "\"weights\":\"Exactly one weights row: contained_metal with lower/upper untested fraction, or dry_tonnes_and_grade with tested_tonnes, tested_grade_g_per_t, untested_tonnes, untested_grade_g_per_t, each suffixed _lower and _upper\",",
    "\"recovery\":\"Exactly two recovery rows, role tested or untested, with lower/upper fractions; explicit [0,1] physical bounds are permitted\",",
    "\"decision\":\"One decision row: threshold fraction and rule proceed_if_at_least; equality proceeds and stop is strictly below\",",
    "\"uu\":\"Exactly one uu row: id, description, trigger, positive finite downside_usd_m naming the untested-bound uncertainty\",",
    "\"interactions\":\"Any interaction row with kind refuses additive averaging\",",
    "\"quantities\":\"Quoted exact n/d or plain decimals, with no implicit units or normalization; closed independent intervals\",",
    "\"output\":\"Contained-metal weights, exact recovery bounds, eight endpoint witnesses, stable_proceed/stable_stop/unstable and attainable decisive test conditions\",",
    "\"scope\":\"Assumed internal decision support; no measured recovery, calibrated probabilities, spatial coverage inference, reserve classification or disclosure authority\",",
    "\"example\":\"tests/decisions/untested_ore.toml\",\"tonne_example\":\"tests/decisions/untested_ore_tonnes.toml\"}"
);
const INPUT: RefusalCode = RefusalCode::InputMissingOrUnreadable506;
const UU: RefusalCode = RefusalCode::UnbudgetedUnknowns204;

/// Reads one explicit weight basis, tested and untested recovery bounds, one threshold and one UU term.
/// Fractions and plain decimals are quoted exact quantities. Dry-tonne weights require both grades in
/// grams per dry metric tonne. An untested physical range must be declared explicitly as [0,1].
/// The verified source and field locators retain original numeric tokens, including decimal precision.
pub fn parse(text: &str, source: SourceAnchor) -> Result<Claim<RecoveryBoundModel>, Refusal> {
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
            "source SHA-256 does not match the exact worksheet bytes",
        ));
    }
    let doc = parse_subset(text).map_err(|e| bad(INPUT, e.line, &e.reason))?;
    let [
        schema,
        model_key,
        question,
        route,
        metal,
        additivity_basis,
        untested_bound_basis,
    ] = strings(
        &doc.top,
        [
            "schema",
            "model_key",
            "question",
            "route",
            "metal",
            "additivity_basis",
            "untested_bound_basis",
        ],
        1,
        INPUT,
    )?;
    if schema != SCHEMA {
        return Err(bad(INPUT, 1, "schema must be fmine.recovery-bounds.v1"));
    }
    identifier(model_key, 1)?;
    let mut weights = None;
    let mut tested = None;
    let mut untested = None;
    let mut threshold = None;
    let mut unknown = None;
    let mut interactions = BTreeSet::new();
    let mut anchors = vec![source.clone()];
    for row in &doc.rows {
        match row.table.as_str() {
            "weights" => {
                if weights.is_some() {
                    return Err(bad(INPUT, row.line, "exactly one weights row is permitted"));
                }
                let basis = row
                    .get("basis")
                    .ok_or_else(|| bad(INPUT, row.line, "missing basis"))?;
                weights = Some(match &basis.value {
                    TomlValue::Str(value) if value == "contained_metal" => {
                        let [_, lower, upper] =
                            strings(&row.fields, ["basis", "lower", "upper"], row.line, INPUT)?;
                        WeightBasis::ContainedMetal {
                            untested_fraction: interval(lower, upper, row.line)?,
                        }
                    }
                    TomlValue::Str(value) if value == "dry_tonnes_and_grade" => {
                        let [_, tt0, tt1, tg0, tg1, ut0, ut1, ug0, ug1] = strings(
                            &row.fields,
                            [
                                "basis",
                                "tested_tonnes_lower",
                                "tested_tonnes_upper",
                                "tested_grade_g_per_t_lower",
                                "tested_grade_g_per_t_upper",
                                "untested_tonnes_lower",
                                "untested_tonnes_upper",
                                "untested_grade_g_per_t_lower",
                                "untested_grade_g_per_t_upper",
                            ],
                            row.line,
                            INPUT,
                        )?;
                        WeightBasis::DryTonnesAndGrade {
                            tested_tonnes: interval(tt0, tt1, row.line)?,
                            tested_grade_g_per_t: interval(tg0, tg1, row.line)?,
                            untested_tonnes: interval(ut0, ut1, row.line)?,
                            untested_grade_g_per_t: interval(ug0, ug1, row.line)?,
                        }
                    }
                    _ => {
                        return Err(bad(
                            INPUT,
                            basis.line,
                            "basis must be contained_metal or dry_tonnes_and_grade",
                        ));
                    }
                });
            }
            "recovery" => {
                let [role, lower, upper] =
                    strings(&row.fields, ["role", "lower", "upper"], row.line, INPUT)?;
                let slot = match role {
                    "tested" => &mut tested,
                    "untested" => &mut untested,
                    _ => {
                        return Err(bad(
                            INPUT,
                            row.line,
                            "recovery role must be tested or untested",
                        ));
                    }
                };
                if slot.is_some() {
                    return Err(bad(INPUT, row.line, "duplicate recovery role"));
                }
                *slot = Some(interval(lower, upper, row.line)?);
            }
            "decision" => {
                if threshold.is_some() {
                    return Err(bad(
                        INPUT,
                        row.line,
                        "exactly one decision row is permitted",
                    ));
                }
                let [rule, value] = strings(&row.fields, ["rule", "threshold"], row.line, INPUT)?;
                if rule != "proceed_if_at_least" {
                    return Err(bad(INPUT, row.line, "rule must be proceed_if_at_least"));
                }
                threshold = Some(rational(value, row.line)?);
            }
            "interaction" => {
                let [kind] = strings(&row.fields, ["kind"], row.line, INPUT)?;
                if !interactions.insert(kind.to_string()) {
                    return Err(bad(INPUT, row.line, "duplicate interaction"));
                }
            }
            "uu" => {
                if unknown.is_some() {
                    return Err(bad(
                        UU,
                        row.line,
                        "exactly one explicit UU term is required",
                    ));
                }
                let [id, description, trigger, exposure] = strings(
                    &row.fields,
                    ["id", "description", "trigger", "downside_usd_m"],
                    row.line,
                    UU,
                )?;
                identifier(id, row.line)?;
                if !plain_decimal(exposure) {
                    return Err(bad(
                        UU,
                        row.line,
                        "downside_usd_m must be a positive finite decimal",
                    ));
                }
                let exposure: f64 = exposure
                    .parse()
                    .map_err(|_| bad(UU, row.line, "invalid UU exposure"))?;
                if !exposure.is_finite() || exposure <= 0.0 {
                    return Err(bad(
                        UU,
                        row.line,
                        "downside_usd_m must be positive and finite",
                    ));
                }
                unknown = Some(UuTerm {
                    scenario_id: id.into(),
                    description: description.into(),
                    trigger_indicator: trigger.into(),
                    downside_exposure_usd_m: exposure,
                });
            }
            other => return Err(bad(INPUT, row.line, &format!("unknown table {other}"))),
        }
        for field in &row.fields {
            if let TomlValue::Str(value) = &field.value {
                anchors.push(SourceAnchor::new(
                    &source.artifact_path,
                    source.artifact_sha256,
                    format!(
                        "{}; {}, line {}: {} = {}",
                        source.locator, row.table, field.line, field.key, value
                    ),
                ));
            }
        }
    }
    let model = RecoveryBoundModel {
        model_key: model_key.into(),
        question: question.into(),
        route: route.into(),
        metal: metal.into(),
        additivity_basis: additivity_basis.into(),
        untested_bound_basis: untested_bound_basis.into(),
        weights: weights.ok_or_else(|| bad(INPUT, 1, "missing weights row"))?,
        tested_recovery: tested.ok_or_else(|| bad(INPUT, 1, "missing tested recovery row"))?,
        untested_recovery: untested
            .ok_or_else(|| bad(INPUT, 1, "missing untested recovery row"))?,
        threshold: threshold.ok_or_else(|| bad(INPUT, 1, "missing decision row"))?,
        interactions: interactions.into_iter().collect(),
    };
    let uu = unknown.ok_or_else(|| bad(UU, 1, "one explicit UU term is required"))?;
    let tag = model.canonical_tag();
    Ok(Claim::new(
        INPUT_METRIC,
        model,
        &tag,
        Representation::Deterministic,
        SpatialSupport::PeriodAggregate,
        AdditivityClass::NonAdditive,
        EvidenceClass::Assumed,
        EpistemicStatus::KnownUnknown(KuKind::Parametric),
        KnowledgeState::Estimated,
        Confidentiality::internal(),
        vec![uu],
        Vec::new(),
        anchors,
    ))
}

fn interval(lower: &str, upper: &str, line: usize) -> Result<Interval, Refusal> {
    Ok(Interval {
        lower: rational(lower, line)?,
        upper: rational(upper, line)?,
    })
}

fn rational(value: &str, line: usize) -> Result<Rat, Refusal> {
    let malformed = || {
        bad(
            INPUT,
            line,
            "quantity must be a quoted exact fraction or plain decimal",
        )
    };
    if let Some((n, d)) = value.split_once('/') {
        if !integer(n) || d.is_empty() || !d.bytes().all(|b| b.is_ascii_digit()) {
            return Err(malformed());
        }
        let n: i128 = n.parse().map_err(|_| malformed())?;
        let d: i128 = d.parse().map_err(|_| malformed())?;
        if d == 0 {
            return Err(malformed());
        }
        Rat::new(n, d).map_err(|_| malformed())
    } else if plain_decimal(value) {
        Rat::from_decimal_str(value).map_err(|_| {
            bad(
                RefusalCode::NumericalCertificateFailed207,
                line,
                "exact decimal overflow",
            )
        })
    } else {
        Err(malformed())
    }
}

fn integer(value: &str) -> bool {
    let digits = value.strip_prefix('-').unwrap_or(value);
    !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit())
}

fn plain_decimal(value: &str) -> bool {
    let digits = value.strip_prefix('-').unwrap_or(value);
    match digits.split_once('.') {
        Some((whole, fraction)) => {
            !whole.is_empty()
                && !fraction.is_empty()
                && whole
                    .bytes()
                    .chain(fraction.bytes())
                    .all(|b| b.is_ascii_digit())
        }
        None => integer(value),
    }
}

fn clean(value: &str) -> bool {
    !value.is_empty() && value.trim() == value && !value.chars().any(char::is_control)
}

fn identifier(value: &str, line: usize) -> Result<(), Refusal> {
    if value
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'.'))
    {
        Ok(())
    } else {
        Err(bad(
            INPUT,
            line,
            "identifiers admit ASCII letters, digits, dot, underscore and hyphen",
        ))
    }
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
                        "{name} must be a nonempty unpadded string without control characters"
                    ),
                ));
            }
        }
    }
    Ok(values)
}

fn bad(code: RefusalCode, line: usize, why: &str) -> Refusal {
    Refusal::new(
        code,
        format!("recovery-bounds worksheet line {line}: {why}"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    const MODEL: &str = include_str!("../../../../tests/decisions/untested_ore.toml");

    fn source(text: &str) -> SourceAnchor {
        SourceAnchor::new(
            "invented.toml",
            sha256(text.as_bytes()).0,
            "complete declared worksheet",
        )
    }

    #[test]
    fn source_bytes_and_original_tokens_bind_the_assumed_input() {
        let claim = parse(MODEL, source(MODEL)).unwrap();
        claim.check_id().unwrap();
        assert_eq!(claim.evidence, EvidenceClass::Assumed);
        assert_eq!(claim.confidentiality, Confidentiality::internal());
        assert_eq!(claim.uu_terms.len(), 1);
        assert!(
            claim
                .anchors
                .iter()
                .any(|a| a.locator.contains("lower = 0.88"))
        );
        assert!(
            claim
                .anchors
                .iter()
                .all(|a| a.artifact_sha256 == sha256(MODEL.as_bytes()).0)
        );
        assert_eq!(
            parse(&format!("{MODEL}\n"), source(MODEL))
                .unwrap_err()
                .code,
            "FMINE-REF-505"
        );
        let altered = format!("{MODEL}\n# additional source declaration\n");
        let next = parse(&altered, source(&altered)).unwrap();
        assert_eq!(claim.value, next.value);
        assert_ne!(claim.claim_id, next.claim_id);
    }

    #[test]
    fn declarations_never_infer_missing_or_upgraded_evidence() {
        for text in [
            MODEL.replace("schema =", "evidence = \"Measured\"\nschema ="),
            MODEL.replace("[[weights]]", "[[weights]]\n[[weights]]"),
            MODEL.replace("role = \"untested\"", "role = \"tested\""),
            MODEL.replace(
                "rule = \"proceed_if_at_least\"",
                "rule = \"proceed_if_above\"",
            ),
            MODEL.replace("threshold = \"0.85\"", "threshold = 85"),
            MODEL.replace("lower = \"0.88\"", "lower = \"NaN\""),
            MODEL.replace("lower = \"0.88\"", "lower = \"1/0\""),
            MODEL.replace("[[uu]]", "[[unrecognized]]"),
        ] {
            assert!(parse(&text, source(&text)).is_err(), "{text}");
        }
        for value in ["0", "-1", "NaN", "1e308", "1/2"] {
            let text = MODEL.replace(
                "downside_usd_m = \"10\"",
                &format!("downside_usd_m = \"{value}\""),
            );
            assert_eq!(
                parse(&text, source(&text)).unwrap_err().code,
                "FMINE-REF-204"
            );
        }
    }
}
