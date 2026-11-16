//! Strict source-bound CostDriverCard declarations; no IO, fitting data inferred from no field.
//! Determinism: Exact

use fmine_core::{anchor::SourceAnchor, hash::sha256};
use fmine_registry::toml_subset::parse_subset;

use super::super::declared::{Fields, field_anchors, fields, source_claim};
use super::*;

pub const SCHEMA: &str = "fmine.cost-driver.v1";
const MAX_BYTES: usize = 1024 * 1024;

fn parse_error(line: usize, detail: String) -> Refusal {
    bad(
        RefusalCode::InputMissingOrUnreadable506,
        &format!("worksheet line {line}: {detail}"),
    )
}
fn get<'a>(fields: &Fields<'a>, key: &str) -> Result<&'a str> {
    fields
        .get(key)
        .map(|f| f.0)
        .ok_or_else(|| parse_error(1, format!("missing {key}")))
}
fn quantity(fields: &Fields<'_>, key: &str) -> Result<Rat> {
    declared::quantity(get(fields, key)?)
        .ok_or_else(|| parse_error(1, format!("invalid exact quantity {key}")))
}
fn period(fields: &Fields<'_>, key: &str) -> Result<Period> {
    declared::period(get(fields, key)?)
        .ok_or_else(|| parse_error(1, format!("invalid calendar period {key}")))
}

/// All supplied inputs remain Assumed and Internal. Accounting references are declarations,
/// not proof that a statutory register was authenticated or that a forecast was registered.
pub fn evaluate(text: &str, source: SourceAnchor) -> Result<DriverReceipt> {
    if text.len() > MAX_BYTES {
        return Err(parse_error(1, "worksheet exceeds 1 MiB".into()));
    }
    if sha256(text.as_bytes()).0 != source.artifact_sha256 {
        return Err(bad(
            RefusalCode::DigestMismatch505,
            "worksheet differs from its SHA-256",
        ));
    }
    let doc = parse_subset(text).map_err(|e| parse_error(e.line, e.reason))?;
    if !(3..=128).contains(&doc.rows.len()) {
        return Err(parse_error(1, "3..128 observation rows required".into()));
    }
    let top = fields(
        &doc.top,
        &[
            "schema",
            "issuer",
            "subject",
            "basis",
            "ownership",
            "currency",
            "item",
            "driver",
            "unit",
            "period",
            "quantity",
            "domain_min",
            "domain_max",
            "valid_from",
            "valid_through",
            "information_cut",
            "error_basis",
            "monetary_rounding",
        ],
        &["ownership_share"],
        1,
        parse_error,
    )?;
    if get(&top, "schema")? != SCHEMA || get(&top, "currency")? != "USD" {
        return Err(parse_error(
            1,
            format!("schema must be {SCHEMA} and currency USD"),
        ));
    }
    let ownership = match (get(&top, "ownership")?, top.contains_key("ownership_share")) {
        ("full", false) => Ownership::Full,
        ("attributable", true) => Ownership::Attributable {
            share: quantity(&top, "ownership_share")?,
        },
        _ => {
            return Err(parse_error(
                1,
                "declare full ownership or attributable ownership with ownership_share".into(),
            ));
        }
    };
    let scope = Scope {
        issuer: get(&top, "issuer")?.into(),
        subject: get(&top, "subject")?.into(),
        period: period(&top, "period")?,
        ownership,
        basis: get(&top, "basis")?.into(),
    };
    let model = DriverInput {
        scope: scope.clone(),
        value: DriverValue::Model {
            item: get(&top, "item")?.into(),
            driver: get(&top, "driver")?.into(),
            unit: get(&top, "unit")?.into(),
            quantity_domain: [quantity(&top, "domain_min")?, quantity(&top, "domain_max")?],
            valid_from: period(&top, "valid_from")?,
            valid_through: period(&top, "valid_through")?,
            information_cut: period(&top, "information_cut")?,
            error_basis: get(&top, "error_basis")?.into(),
            monetary_rounding: get(&top, "monetary_rounding")?.into(),
        },
    };
    let claim = |input: DriverInput, anchors| {
        let tag = input.canonical_tag();
        source_claim(
            INPUT_METRIC,
            input,
            &tag,
            AdditivityClass::NonAdditive,
            Vec::new(),
            anchors,
        )
    };
    let mut inputs = vec![claim(model, field_anchors(&source, &doc.top, "model"))];
    inputs.push(claim(
        DriverInput {
            scope: scope.clone(),
            value: DriverValue::Target {
                quantity: quantity(&top, "quantity")?,
            },
        },
        field_anchors(&source, &doc.top, "target"),
    ));
    for row in &doc.rows {
        if row.table != "observation" {
            return Err(parse_error(
                row.line,
                format!("unknown table {}", row.table),
            ));
        }
        let f = fields(
            &row.fields,
            &[
                "period",
                "quantity",
                "usd",
                "available",
                "accounting_certificate",
            ],
            &[],
            row.line,
            parse_error,
        )?;
        let input = DriverInput {
            scope: Scope {
                period: period(&f, "period")?,
                ..scope.clone()
            },
            value: DriverValue::Observation {
                quantity: quantity(&f, "quantity")?,
                usd: super::super::worksheet::money(get(&f, "usd")?, row.line)?,
                available: period(&f, "available")?,
                accounting_certificate: get(&f, "accounting_certificate")?.into(),
            },
        };
        inputs.push(claim(
            input,
            field_anchors(&source, &row.fields, "observation"),
        ));
    }
    fit(&inputs)
}
