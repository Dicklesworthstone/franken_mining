//! DESIGN: strict source-bound economic assumptions for an admitted production population.
//! Determinism: DeterministicPerIsa (the inherited production receipt fixes the numerical inputs).

use super::*;
use fmine_registry::toml_subset::{TomlField, TomlValue, parse_subset};

pub const SCHEMA: &str = "fmine.production-valuation.v1";
pub const MAX_BYTES: u64 = 1024 * 1024;
pub const NO_CLAIM: &str = super::NO_CLAIM;
type Fields<'a> = BTreeMap<&'a str, (&'a str, usize)>;

fn invalid(line: usize, message: impl AsRef<str>) -> Refusal {
    bad(
        RefusalCode::InputMissingOrUnreadable506,
        format!("worksheet line {line}: {}", message.as_ref()),
    )
}

fn fields<'a>(input: &'a [TomlField], required: &[&str], line: usize) -> Result<Fields<'a>> {
    let mut result = BTreeMap::new();
    for field in input {
        if !required.contains(&field.key.as_str()) {
            return Err(invalid(
                field.line,
                format!("unsupported field {}", field.key),
            ));
        }
        let TomlValue::Str(value) = &field.value else {
            return Err(invalid(field.line, "values must be quoted strings"));
        };
        if !named(value) {
            return Err(invalid(
                field.line,
                "nonempty trimmed text within 4096 bytes required",
            ));
        }
        if result
            .insert(field.key.as_str(), (value.as_str(), field.line))
            .is_some()
        {
            return Err(invalid(field.line, "duplicate field"));
        }
    }
    for key in required {
        if !result.contains_key(key) {
            return Err(invalid(line, format!("missing {key}")));
        }
    }
    Ok(result)
}

fn get<'a>(fields: &Fields<'a>, key: &str) -> Result<(&'a str, usize)> {
    fields
        .get(key)
        .copied()
        .ok_or_else(|| invalid(1, format!("missing {key}")))
}

fn number(fields: &Fields<'_>, key: &str) -> Result<Rat> {
    let (value, line) = get(fields, key)?;
    let result = if let Some((n, d)) = value.split_once('/') {
        n.parse::<i128>()
            .ok()
            .zip(d.parse::<i128>().ok())
            .filter(|(_, d)| *d > 0)
            .and_then(|(n, d)| Rat::new(n, d).ok())
    } else {
        let digits = value.strip_prefix('-').unwrap_or(value);
        let (whole, fraction) = digits.split_once('.').unwrap_or((digits, ""));
        let valid = !whole.is_empty()
            && whole
                .bytes()
                .chain(fraction.bytes())
                .all(|b| b.is_ascii_digit())
            && (!digits.contains('.') || !fraction.is_empty());
        valid.then(|| Rat::from_decimal_str(value).ok()).flatten()
    };
    result.ok_or_else(|| {
        invalid(
            line,
            format!("{key} needs an exact decimal or n/d with positive denominator"),
        )
    })
}

/// Parse exact source bytes and evaluate the complete coupled calculation.
/// Supplied terms remain Assumed, Internal and uncalibrated. All units are part
/// of the schema/field names; no currency, tonne, element or ownership conversion occurs.
pub fn evaluate(
    text: &str,
    source: &SourceAnchor,
    production: &ProductionReceipt,
) -> Result<ValuationReceipt> {
    if text.len() as u64 > MAX_BYTES {
        return Err(invalid(1, "worksheet exceeds 1 MiB"));
    }
    if !named(&source.artifact_path) || !named(&source.locator) {
        return Err(invalid(1, "source path and locator required"));
    }
    if sha256(text.as_bytes()).0 != source.artifact_sha256 {
        return Err(bad(
            RefusalCode::DigestMismatch505,
            "commercial source SHA-256 mismatch",
        ));
    }
    let doc = parse_subset(text).map_err(|e| invalid(e.line, e.reason))?;
    if doc.rows.len() > card::MAX_STATES + 64 {
        return Err(invalid(
            1,
            "at most 4096 scenarios and 64 UU rows are accepted",
        ));
    }
    let top = fields(
        &doc.top,
        &[
            "schema",
            "model_key",
            "question",
            "valuation_basis",
            "action_id",
            "action_label",
            "boundary",
            "circuit",
            "material_domain",
            "dry_basis",
            "element",
            "start_ns",
            "end_ns",
            "commercial_support",
            "physical_to_commercial_basis",
            "monetary_rounding",
        ],
        1,
    )?;
    if get(&top, "schema")?.0 != SCHEMA {
        return Err(invalid(1, format!("schema must be {SCHEMA}")));
    }
    let monetary_rounding = MonetaryRounding::NearestTiesEvenFourPlaces;
    if get(&top, "monetary_rounding")?.0 != monetary_rounding.as_str() {
        return Err(invalid(
            get(&top, "monetary_rounding")?.1,
            format!("monetary_rounding must be {}", monetary_rounding.as_str()),
        ));
    }
    let time = |key| {
        let (value, line) = get(&top, key)?;
        value
            .parse::<i64>()
            .map_err(|_| invalid(line, "time must be an i64 nanosecond integer"))
    };
    let mut anchors = vec![source.clone()];
    let mut scenarios = Vec::new();
    let mut uu = BTreeMap::new();
    for row in &doc.rows {
        match row.table.as_str() {
            "scenario" => {
                let f = fields(
                    &row.fields,
                    &[
                        "id",
                        "weight",
                        "payable_fraction",
                        "usd_per_kg",
                        "variable_usd_per_dry_t",
                        "fixed_usd",
                        "royalty_fraction",
                        "do_nothing_usd",
                    ],
                    row.line,
                )?;
                scenarios.push(ScenarioTerms {
                    id: get(&f, "id")?.0.into(),
                    weight: number(&f, "weight")?,
                    payable_fraction: number(&f, "payable_fraction")?,
                    usd_per_kg: number(&f, "usd_per_kg")?,
                    variable_usd_per_dry_t: number(&f, "variable_usd_per_dry_t")?,
                    fixed_usd: number(&f, "fixed_usd")?,
                    royalty_fraction: number(&f, "royalty_fraction")?,
                    do_nothing_usd: number(&f, "do_nothing_usd")?,
                });
            }
            "uu" => {
                let f = fields(
                    &row.fields,
                    &["id", "description", "trigger", "downside_usd_m"],
                    row.line,
                )?;
                let amount = number(&f, "downside_usd_m")?.to_f64();
                if !amount.is_finite() || amount <= 0.0 {
                    return Err(invalid(
                        get(&f, "downside_usd_m")?.1,
                        "UU exposure must be positive and finite",
                    ));
                }
                let id = get(&f, "id")?.0;
                if uu
                    .insert(
                        id.to_string(),
                        UuTerm {
                            scenario_id: id.into(),
                            description: get(&f, "description")?.0.into(),
                            trigger_indicator: get(&f, "trigger")?.0.into(),
                            downside_exposure_usd_m: amount,
                        },
                    )
                    .is_some()
                    || uu.len() > 64
                {
                    return Err(invalid(row.line, "UU ids must be unique; at most 64 terms"));
                }
            }
            _ => {
                return Err(invalid(
                    row.line,
                    format!("unsupported table {}", row.table),
                ));
            }
        }
        for field in &row.fields {
            anchors.push(SourceAnchor::new(
                &source.artifact_path,
                source.artifact_sha256,
                format!("line {}: {}.{}", field.line, row.table, field.key),
            ));
        }
    }
    for field in &doc.top {
        anchors.push(SourceAnchor::new(
            &source.artifact_path,
            source.artifact_sha256,
            format!("line {}: {}", field.line, field.key),
        ));
    }
    let terms = ValuationTerms {
        model_key: get(&top, "model_key")?.0.into(),
        question: get(&top, "question")?.0.into(),
        valuation_basis: get(&top, "valuation_basis")?.0.into(),
        action_id: get(&top, "action_id")?.0.into(),
        action_label: get(&top, "action_label")?.0.into(),
        boundary: get(&top, "boundary")?.0.into(),
        circuit: get(&top, "circuit")?.0.into(),
        material_domain: get(&top, "material_domain")?.0.into(),
        dry_basis: get(&top, "dry_basis")?.0.into(),
        element: get(&top, "element")?.0.into(),
        start_ns: time("start_ns")?,
        end_ns: time("end_ns")?,
        commercial_support: get(&top, "commercial_support")?.0.into(),
        physical_to_commercial_basis: get(&top, "physical_to_commercial_basis")?.0.into(),
        monetary_rounding,
        scenarios,
    };
    let tag = terms.canonical_tag();
    let claim = Claim::new(
        INPUT_METRIC,
        terms,
        &tag,
        Representation::Deterministic,
        SpatialSupport::PeriodAggregate,
        AdditivityClass::NonAdditive,
        EvidenceClass::Assumed,
        EpistemicStatus::KnownUnknown(KuKind::Parametric),
        KnowledgeState::Known,
        Confidentiality::internal(),
        uu.into_values().collect(),
        vec![],
        anchors,
    );
    let mut result = super::evaluate(production, &claim)?;
    result.submitted_worksheet = Some(text.into());
    Ok(result)
}
