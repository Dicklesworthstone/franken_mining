//! Strict declared GEO worksheets. Authentication binds the submitted bytes, not their factual accuracy.
//! Every imported Claim remains Assumed, Internal and uncalibrated.
//! Determinism: Exact

use std::collections::BTreeMap;

use fmine_claim::{Claim, Confidentiality, Representation, SpatialSupport};
use fmine_core::{anchor::SourceAnchor, hash::sha256};
use fmine_evidence::{
    EpistemicStatus, EvidenceClass, KnowledgeState, KuKind, Refusal, RefusalCode,
};
use fmine_registry::toml_subset::{TomlField, TomlValue, parse_subset};
use fmine_units::{Currency, ExactQty};

use super::{DeckKey, GeoInput, GeoReceipt, GeoValue, Metal, PriceDeck, Scope};
use crate::guidance::{Ownership, Period};

pub const SCHEMA: &str = "fmine.geo-worksheet.v1";
const MAX_BYTES: usize = 1_048_576;
type Fields<'a> = BTreeMap<&'a str, (&'a str, usize)>;

fn bad(line: usize, detail: impl AsRef<str>) -> Refusal {
    Refusal::new(
        RefusalCode::InputMissingOrUnreadable506,
        format!("GEO worksheet line {line}: {}", detail.as_ref()),
    )
}

fn fields<'a>(
    values: &'a [TomlField],
    required: &[&str],
    optional: &[&str],
    line: usize,
) -> Result<Fields<'a>, Refusal> {
    let mut fields = BTreeMap::new();
    for field in values {
        if !required.contains(&field.key.as_str()) && !optional.contains(&field.key.as_str()) {
            return Err(bad(field.line, format!("unsupported field {}", field.key)));
        }
        let TomlValue::Str(value) = &field.value else {
            return Err(bad(field.line, "values must be quoted strings"));
        };
        if !super::named(value) {
            return Err(bad(
                field.line,
                "values must be nonempty trimmed text, without controls, at most 4096 bytes",
            ));
        }
        if fields
            .insert(field.key.as_str(), (value.as_str(), field.line))
            .is_some()
        {
            return Err(bad(field.line, format!("duplicate field {}", field.key)));
        }
    }
    for key in required {
        if !fields.contains_key(key) {
            return Err(bad(line, format!("missing {key}")));
        }
    }
    Ok(fields)
}

fn get<'a>(fields: &Fields<'a>, name: &str) -> Result<(&'a str, usize), Refusal> {
    fields
        .get(name)
        .copied()
        .ok_or_else(|| bad(1, format!("missing {name}")))
}

fn number(text: &str, line: usize) -> Result<ExactQty, Refusal> {
    let invalid = || {
        bad(
            line,
            "expected an exact decimal or n/d with positive denominator",
        )
    };
    if let Some((n, d)) = text.split_once('/') {
        let n = n.parse::<i128>().map_err(|_| invalid())?;
        let d = d.parse::<i128>().map_err(|_| invalid())?;
        if d <= 0 {
            return Err(invalid());
        }
        return ExactQty::new(n, d).map_err(|_| invalid());
    }
    let digits = text.strip_prefix('-').unwrap_or(text);
    let (whole, fraction) = digits.split_once('.').unwrap_or((digits, ""));
    if whole.is_empty()
        || (digits.contains('.') && fraction.is_empty())
        || !whole
            .bytes()
            .chain(fraction.bytes())
            .all(|c| c.is_ascii_digit())
    {
        return Err(invalid());
    }
    ExactQty::from_decimal_str(text).map_err(|_| invalid())
}

fn amount(fields: &Fields<'_>, name: &str) -> Result<ExactQty, Refusal> {
    let (text, line) = get(fields, name)?;
    number(text, line)
}

fn optional_amount(fields: &Fields<'_>, name: &str) -> Result<Option<ExactQty>, Refusal> {
    fields
        .get(name)
        .map(|(text, line)| number(text, *line))
        .transpose()
}

fn metal(text: &str, line: usize) -> Result<Metal, Refusal> {
    Metal::ALL
        .into_iter()
        .find(|metal| metal.name() == text)
        .ok_or_else(|| bad(line, "metal must be gold, copper or silver"))
}

fn currency(text: &str, line: usize) -> Result<Currency, Refusal> {
    use Currency::*;
    [
        USD, CAD, AUD, ZAR, PKR, CLP, PEN, ARS, TZS, XAF, CDF, ZMW, PGK, DOP, SAR, EUR, JPY,
    ]
    .into_iter()
    .find(|currency| format!("{currency:?}") == text)
    .ok_or_else(|| {
        bad(
            line,
            "currency must be an explicit supported ISO code; no implicit FX",
        )
    })
}

fn period(text: &str, line: usize) -> Result<Period, Refusal> {
    let invalid = || bad(line, "period must be YYYYFY, YYYYQ1..Q4 or YYYYH1..H2");
    if !text.is_ascii() || text.len() != 6 || !text.as_bytes()[..4].iter().all(u8::is_ascii_digit) {
        return Err(invalid());
    }
    let year = text[..4].parse::<u16>().map_err(|_| invalid())?;
    let (first_month, last_month) = match &text[4..] {
        "FY" => (1, 12),
        "Q1" => (1, 3),
        "Q2" => (4, 6),
        "Q3" => (7, 9),
        "Q4" => (10, 12),
        "H1" => (1, 6),
        "H2" => (7, 12),
        _ => return Err(invalid()),
    };
    if year == 0 {
        return Err(invalid());
    }
    Ok(Period {
        year,
        first_month,
        last_month,
    })
}

fn declared(
    scope: &Scope,
    value: GeoValue,
    source: &SourceAnchor,
    fields: &[TomlField],
    table: &str,
) -> Claim<GeoInput> {
    let value = GeoInput {
        scope: scope.clone(),
        value,
    };
    let tag = value.canonical_tag();
    let additivity = value.additivity();
    let anchors = fields
        .iter()
        .map(|field| {
            SourceAnchor::new(
                &source.artifact_path,
                source.artifact_sha256,
                format!("line {}: {table}.{}", field.line, field.key),
            )
        })
        .collect();
    Claim::new(
        super::INPUT_METRIC,
        value,
        &tag,
        Representation::Deterministic,
        SpatialSupport::PeriodAggregate,
        additivity,
        EvidenceClass::Assumed,
        EpistemicStatus::KnownUnknown(KuKind::Parametric),
        KnowledgeState::Known,
        Confidentiality::internal(),
        Vec::new(),
        Vec::new(),
        anchors,
    )
}

/// Compare all declared decks using the same exact production quantities. A source anchor whose digest
/// does not match text refuses. The parser permits no evidence, calibration or authority override.
pub fn evaluate(text: &str, source: SourceAnchor) -> Result<GeoReceipt, Refusal> {
    if text.len() > MAX_BYTES {
        return Err(bad(1, "worksheet exceeds one MiB"));
    }
    if sha256(text.as_bytes()).0 != source.artifact_sha256 {
        return Err(Refusal::new(
            RefusalCode::DigestMismatch505,
            "GEO worksheet bytes differ from the source SHA-256",
        ));
    }
    let doc = parse_subset(text).map_err(|e| bad(e.line, e.reason))?;
    if doc.rows.len() > super::MAX_DECKS + 3 {
        return Err(bad(
            1,
            "worksheet exceeds the finite quantity/deck population limit",
        ));
    }
    let top = fields(
        &doc.top,
        &[
            "schema",
            "issuer",
            "subject",
            "period",
            "ownership",
            "basis",
            "population",
            "currency",
            "metals",
            "reference_deck",
            "reference_version",
        ],
        &["ownership_share"],
        1,
    )?;
    if get(&top, "schema")?.0 != SCHEMA {
        return Err(bad(1, format!("schema must be {SCHEMA}")));
    }
    let ownership = match (get(&top, "ownership")?.0, top.get("ownership_share")) {
        ("full", None) => Ownership::Full,
        ("attributable", Some((share, line))) => Ownership::Attributable {
            share: number(share, *line)?,
        },
        _ => {
            return Err(bad(
                get(&top, "ownership")?.1,
                "ownership requires full without a share, or attributable with an exact ownership_share",
            ));
        }
    };
    let (period_text, period_line) = get(&top, "period")?;
    let (currency_text, currency_line) = get(&top, "currency")?;
    let scope = Scope {
        issuer: get(&top, "issuer")?.0.into(),
        subject: get(&top, "subject")?.0.into(),
        period: period(period_text, period_line)?,
        ownership,
        basis: get(&top, "basis")?.0.into(),
        population: get(&top, "population")?.0.into(),
        currency: currency(currency_text, currency_line)?,
    };
    let (metal_text, metal_line) = get(&top, "metals")?;
    let metals = metal_text
        .split(',')
        .map(|name| metal(name, metal_line))
        .collect::<Result<Vec<_>, _>>()?;
    let reference = DeckKey {
        id: get(&top, "reference_deck")?.0.into(),
        version: get(&top, "reference_version")?.0.into(),
    };
    let mut inputs = vec![declared(
        &scope,
        GeoValue::Population { metals, reference },
        &source,
        &doc.top,
        "population",
    )];
    for row in &doc.rows {
        let value = match row.table.as_str() {
            "quantity" => {
                let f = fields(&row.fields, &["metal", "unit", "amount"], &[], row.line)?;
                let (name, line) = get(&f, "metal")?;
                GeoValue::Quantity {
                    metal: metal(name, line)?,
                    unit: get(&f, "unit")?.0.into(),
                    amount: amount(&f, "amount")?,
                }
            }
            "deck" => {
                let f = fields(
                    &row.fields,
                    &["id", "version", "basis", "currency", "gold_price_per_oz"],
                    &["copper_price_per_lb", "silver_price_per_oz"],
                    row.line,
                )?;
                let (currency_text, currency_line) = get(&f, "currency")?;
                GeoValue::Deck(PriceDeck {
                    key: DeckKey {
                        id: get(&f, "id")?.0.into(),
                        version: get(&f, "version")?.0.into(),
                    },
                    basis: get(&f, "basis")?.0.into(),
                    currency: currency(currency_text, currency_line)?,
                    gold_per_oz: amount(&f, "gold_price_per_oz")?,
                    copper_per_lb: optional_amount(&f, "copper_price_per_lb")?,
                    silver_per_oz: optional_amount(&f, "silver_price_per_oz")?,
                })
            }
            _ => return Err(bad(row.line, format!("unknown table {}", row.table))),
        };
        inputs.push(declared(&scope, value, &source, &row.fields, &row.table));
    }
    super::evaluate(&inputs)
}
