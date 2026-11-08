//! Strict declared GEO worksheets. Authentication binds the submitted bytes, not their factual accuracy.
//! Every imported Claim remains Assumed, Internal and uncalibrated.
//! Determinism: Exact

use fmine_claim::Claim;
use fmine_core::{anchor::SourceAnchor, hash::sha256};
use fmine_evidence::{Refusal, RefusalCode};
use fmine_registry::toml_subset::{TomlField, parse_subset};
use fmine_units::{Currency, ExactQty};

use super::{DeckKey, GeoInput, GeoReceipt, GeoValue, Metal, PriceDeck, Scope};
use crate::definitions::declared::{Fields, field_anchors, fields, source_claim};
use crate::guidance::{Ownership, Period};

pub const SCHEMA: &str = "fmine.geo-worksheet.v1";
const MAX_BYTES: usize = 1_048_576;

fn bad(line: usize, detail: impl AsRef<str>) -> Refusal {
    Refusal::new(
        RefusalCode::InputMissingOrUnreadable506,
        format!("GEO worksheet line {line}: {}", detail.as_ref()),
    )
}

fn get<'a>(fields: &Fields<'a>, name: &str) -> Result<(&'a str, usize), Refusal> {
    fields
        .get(name)
        .copied()
        .ok_or_else(|| bad(1, format!("missing {name}")))
}

fn number(text: &str, line: usize) -> Result<ExactQty, Refusal> {
    crate::definitions::declared::quantity(text).ok_or_else(|| {
        bad(
            line,
            "expected an exact decimal or n/d with positive denominator",
        )
    })
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
    crate::definitions::declared::period(text)
        .ok_or_else(|| bad(line, "period must be YYYYFY, YYYYQ1..Q4 or YYYYH1..H2"))
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
    source_claim(
        super::INPUT_METRIC,
        value,
        &tag,
        additivity,
        Vec::new(),
        field_anchors(source, fields, table),
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
        bad,
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
                let f = fields(
                    &row.fields,
                    &["metal", "unit", "amount"],
                    &[],
                    row.line,
                    bad,
                )?;
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
                    bad,
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
