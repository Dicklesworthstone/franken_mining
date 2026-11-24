//! Strict declarations for capital-return assessments; submitted bytes prove provenance, not facts.
//! Determinism: Exact

use fmine_claim::Claim;
use fmine_core::{anchor::SourceAnchor, hash::sha256};
use fmine_evidence::{Refusal, RefusalCode};
use fmine_registry::toml_subset::parse_subset;
use fmine_units::{Currency, ExactQty};

use super::{
    Amount, Annualization, Applicability, CapitalBasis, CapitalReturnReceipt, Definition, Input,
    InputValue, Scope, TaxBasis,
};
use crate::definitions::declared::{Fields, field_anchors, fields, source_claim};
use crate::guidance::{Ownership, Period};

pub const SCHEMA: &str = "fmine.capital-return-worksheet.v1";
pub const MAX_BYTES: usize = 1_048_576;

pub(super) const REQUIRED: &[&str] = &[
    "subject",
    "ownership",
    "perimeter",
    "accounting_basis",
    "currency",
    "monetary_scale",
    "definition",
    "version",
    "earnings_basis",
    "tax_basis",
    "capital_employed_basis",
    "capital_basis",
    "annualization",
    "applicability",
];
pub(super) const OPTIONAL: &[&str] = &[
    "ownership_share",
    "annualization_factor",
    "annualization_basis",
    "exclusion_reason",
];

fn bad(line: usize, detail: impl AsRef<str>) -> Refusal {
    Refusal::new(
        RefusalCode::InputMissingOrUnreadable506,
        format!(
            "capital-return declaration line {line}: {}",
            detail.as_ref()
        ),
    )
}

pub(super) fn get<'a>(fields: &Fields<'a>, key: &str) -> Result<&'a str, Refusal> {
    fields
        .get(key)
        .map(|(value, _)| *value)
        .ok_or_else(|| bad(1, format!("missing {key}")))
}

fn number(text: &str) -> Result<ExactQty, Refusal> {
    crate::definitions::declared::quantity(text).ok_or_else(|| {
        bad(
            1,
            "expected an exact decimal or n/d with positive denominator",
        )
    })
}

pub(super) fn period(text: &str) -> Result<Period, Refusal> {
    crate::definitions::declared::period(text)
        .ok_or_else(|| bad(1, "period must be YYYYFY, YYYYQ1..Q4 or YYYYH1..H2"))
}

pub(super) fn date(text: &str) -> Result<fmine_time::CivilDate, Refusal> {
    if !fmine_registry::sources::is_date(text) {
        return Err(bad(1, "snapshot date must be a valid YYYY-MM-DD"));
    }
    let year = text[..4]
        .parse()
        .map_err(|_| bad(1, "invalid snapshot year"))?;
    let month = text[5..7]
        .parse()
        .map_err(|_| bad(1, "invalid snapshot month"))?;
    let day = text[8..]
        .parse()
        .map_err(|_| bad(1, "invalid snapshot day"))?;
    fmine_time::CivilDate::new(year, month, day).map_err(|e| bad(1, e.to_string()))
}

pub(super) fn context(
    f: &Fields<'_>,
    issuer: &str,
    period: Period,
) -> Result<(Scope, Definition), Refusal> {
    let ownership = match (get(f, "ownership")?, f.get("ownership_share")) {
        ("full", None) => Ownership::Full,
        ("attributable", Some((share, _))) => Ownership::Attributable {
            share: number(share)?,
        },
        _ => {
            return Err(bad(
                1,
                "ownership requires full without a share, or attributable with ownership_share",
            ));
        }
    };
    use Currency::*;
    let currency_text = get(f, "currency")?;
    let currency = [
        USD, CAD, AUD, ZAR, PKR, CLP, PEN, ARS, TZS, XAF, CDF, ZMW, PGK, DOP, SAR, EUR, JPY,
    ]
    .into_iter()
    .find(|c| format!("{c:?}") == currency_text)
    .ok_or_else(|| bad(1, "unsupported explicit ISO currency"))?;
    let monetary_scale = match get(f, "monetary_scale")? {
        "1" => 1,
        "1000" => 1_000,
        "1000000" => 1_000_000,
        _ => return Err(bad(1, "monetary_scale must be 1, 1000 or 1000000")),
    };
    let tax_basis = match get(f, "tax_basis")? {
        "pre_tax" => TaxBasis::PreTax,
        "after_tax" => TaxBasis::AfterTax,
        _ => return Err(bad(1, "tax_basis must be pre_tax or after_tax")),
    };
    let capital_basis = match get(f, "capital_basis")? {
        "opening" => CapitalBasis::Opening,
        "closing" => CapitalBasis::Closing,
        "opening_closing_mean" => CapitalBasis::OpeningClosingMean,
        "reported_average" => CapitalBasis::ReportedAverage,
        _ => return Err(bad(1, "unsupported capital_basis")),
    };
    let annualization = match (
        get(f, "annualization")?,
        f.get("annualization_factor"),
        f.get("annualization_basis"),
    ) {
        ("none", None, None) => Annualization::None,
        ("explicit", Some((factor, _)), Some((basis, _))) => Annualization::Explicit {
            multiplier: number(factor)?,
            basis: (*basis).into(),
        },
        _ => {
            return Err(bad(
                1,
                "annualization requires none without extra fields, or explicit with annualization_factor and annualization_basis",
            ));
        }
    };
    let applicability = match (get(f, "applicability")?, f.get("exclusion_reason")) {
        ("applicable", None) => Applicability::Applicable,
        ("excluded", Some((reason, _))) => Applicability::Excluded {
            reason: (*reason).into(),
        },
        _ => {
            return Err(bad(
                1,
                "applicability requires applicable without a reason, or excluded with exclusion_reason",
            ));
        }
    };
    let scope = Scope {
        issuer: issuer.into(),
        subject: get(f, "subject")?.into(),
        period,
        ownership,
        perimeter: get(f, "perimeter")?.into(),
        accounting_basis: get(f, "accounting_basis")?.into(),
        currency,
        monetary_scale,
    };
    super::validate_scope(&scope)?;
    let definition = Definition {
        id: get(f, "definition")?.into(),
        version: get(f, "version")?.into(),
        earnings_basis: get(f, "earnings_basis")?.into(),
        tax_basis,
        capital_employed_basis: get(f, "capital_employed_basis")?.into(),
        capital_basis,
        annualization,
        applicability,
    };
    Ok((scope, definition))
}

fn declared(scope: &Scope, value: InputValue, anchors: Vec<SourceAnchor>) -> Claim<Input> {
    let input = Input {
        scope: scope.clone(),
        value,
        source_record: None,
    };
    let tag = input.canonical_tag();
    let additivity = input.additivity();
    source_claim(
        super::INPUT_METRIC,
        input,
        &tag,
        additivity,
        Vec::new(),
        anchors,
    )
}

/// Evaluate a strict, source-hashed declaration. All imported Claims remain Assumed and Internal.
pub fn evaluate(text: &str, source: SourceAnchor) -> Result<CapitalReturnReceipt, Refusal> {
    if text.len() > MAX_BYTES {
        return Err(bad(1, "worksheet exceeds one MiB"));
    }
    if sha256(text.as_bytes()).0 != source.artifact_sha256 {
        return Err(Refusal::new(
            RefusalCode::DigestMismatch505,
            "capital-return worksheet bytes differ from source SHA-256",
        ));
    }
    let doc = parse_subset(text).map_err(|e| bad(e.line, e.reason))?;
    if !(2..=3).contains(&doc.rows.len()) {
        return Err(bad(
            1,
            "worksheet needs earnings and exactly the selected one or two capital observations",
        ));
    }
    let mut required = REQUIRED.to_vec();
    required.extend_from_slice(&["schema", "issuer", "period"]);
    let f = fields(&doc.top, &required, OPTIONAL, 1, bad)?;
    if get(&f, "schema")? != SCHEMA {
        return Err(bad(1, format!("schema must be {SCHEMA}")));
    }
    let (scope, definition) = context(&f, get(&f, "issuer")?, period(get(&f, "period")?)?)?;
    let mut inputs = vec![declared(
        &scope,
        InputValue::Definition(definition),
        field_anchors(&source, &doc.top, "definition"),
    )];
    for row in &doc.rows {
        let time_field = match row.table.as_str() {
            "earnings" | "average" => "period",
            "opening" | "closing" => "as_of",
            _ => {
                return Err(bad(
                    row.line,
                    format!("unsupported input role {}", row.table),
                ));
            }
        };
        let f = fields(
            &row.fields,
            &[time_field, "amount", "rounding_step"],
            &[],
            row.line,
            bad,
        )?;
        let amount = Amount {
            value: number(get(&f, "amount")?)?,
            rounding_step: number(get(&f, "rounding_step")?)?,
        };
        let value = match row.table.as_str() {
            "earnings" => InputValue::Earnings {
                period: period(get(&f, "period")?)?,
                amount,
            },
            "average" => InputValue::Average {
                period: period(get(&f, "period")?)?,
                amount,
            },
            "opening" => InputValue::Opening {
                as_of: date(get(&f, "as_of")?)?,
                amount,
            },
            "closing" => InputValue::Closing {
                as_of: date(get(&f, "as_of")?)?,
                amount,
            },
            _ => return Err(bad(row.line, "unsupported input role")),
        };
        inputs.push(declared(
            &scope,
            value,
            field_anchors(&source, &row.fields, &row.table),
        ));
    }
    super::evaluate(&inputs)
}
