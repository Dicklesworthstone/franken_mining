//! Interpretation of agreed registered filing readings for capital return.
//! Determinism: Exact
//!
//! The existing public loader authenticates document bytes and complete reader agreement. This pure
//! adapter binds the exact printed token, definition citation, reporting window and snapshot dates.
//! It admits issuer assertions, never measurements, and never infers tax or capital adjustments.

use std::collections::{BTreeMap, BTreeSet};

use fmine_claim::{Claim, Confidentiality, Representation, SpatialSupport};
use fmine_core::anchor::SourceAnchor;
use fmine_evidence::{EpistemicStatus, EvidenceClass, KnowledgeState, Refusal, RefusalCode};
use fmine_registry::sources::DocumentRow;
use fmine_registry::transcriptions::{Accepted, Locator, Token, parse_token};
use fmine_units::ExactQty;

use super::{Amount, Input, InputValue, SourceRecord, bad, exact, named, worksheet};
use crate::definitions::declared::Fields;
use crate::guidance::Period;

pub const ROLES: [&str; 4] = ["earnings", "opening", "closing", "average"];

#[must_use]
pub fn valid_series(series: &str) -> bool {
    !series.is_empty()
        && series.len() <= 128
        && series
            .bytes()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'_')
}

#[must_use]
pub fn quantities(series: &str) -> Vec<String> {
    ROLES
        .iter()
        .map(|role| format!("{series}_{role}"))
        .collect()
}

fn digest(text: &str) -> Result<[u8; 32], Refusal> {
    if text.len() != 64
        || !text
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(bad(
            RefusalCode::DigestMismatch505,
            "registered document needs a lowercase SHA-256",
        ));
    }
    let mut bytes = [0; 32];
    for (i, byte) in bytes.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&text[i * 2..i * 2 + 2], 16)
            .map_err(|_| bad(RefusalCode::DigestMismatch505, "invalid document SHA-256"))?;
    }
    if bytes == [0; 32] {
        return Err(bad(
            RefusalCode::DigestMismatch505,
            "missing source SHA-256",
        ));
    }
    Ok(bytes)
}

fn fields(row: &Accepted) -> Result<Fields<'_>, Refusal> {
    let mut fields = BTreeMap::new();
    for part in row.basis.split(';') {
        let (key, value) = part.split_once('=').ok_or_else(|| {
            bad(
                RefusalCode::BasisMismatch105,
                "source definition requires semicolon-separated key=value fields",
            )
        })?;
        if (!worksheet::REQUIRED.contains(&key)
            && !worksheet::OPTIONAL.contains(&key)
            && ![
                "snapshot",
                "definition_page",
                "definition_table",
                "definition_row",
                "definition_col",
            ]
            .contains(&key))
            || !named(key)
            || !named(value)
            || fields.insert(key, (value, 0)).is_some()
        {
            return Err(bad(
                RefusalCode::BasisMismatch105,
                "source definition contains an unknown, empty or duplicate field",
            ));
        }
    }
    Ok(fields)
}

fn claim(input: Input, anchors: Vec<SourceAnchor>) -> Claim<Input> {
    let tag = input.canonical_tag();
    let additivity = input.additivity();
    Claim::new(
        super::INPUT_METRIC,
        input,
        &tag,
        Representation::Deterministic,
        SpatialSupport::PeriodAggregate,
        additivity,
        EvidenceClass::ISSUER,
        EpistemicStatus::KnownKnown,
        KnowledgeState::Known,
        Confidentiality::public(),
        Vec::new(),
        Vec::new(),
        anchors,
    )
}

/// Bind exactly one selected filing series. Rows use `<series>_earnings`, `_opening`, `_closing` or
/// `_average`. Each reader agrees on the complete definition and its separate cited page/table cell.
/// `period` is the common target window; opening/closing balance dates are explicit `snapshot` fields.
pub fn bind(
    issuer: &str,
    series: &str,
    period: Period,
    document: &DocumentRow,
    rows: &[Accepted],
) -> Result<Vec<Claim<Input>>, Refusal> {
    if !valid_series(series) || !named(issuer) || !(2..=3).contains(&rows.len()) {
        return Err(bad(
            RefusalCode::NoClaimInDomain201,
            "a named series and exactly two or three agreed monetary readings are required",
        ));
    }
    if !matches!(
        document.kind.as_str(),
        "mda" | "aif" | "partner_filing" | "information_circular"
    ) || !fmine_registry::sources::is_date(&document.published_at)
        || !document.published_at_basis.starts_with("FACT: ")
        || document.published_at_basis.len() <= 6
    {
        return Err(bad(
            RefusalCode::NoClaimInDomain201,
            "a registered financial filing and FACT publication evidence are required",
        ));
    }
    let sha = digest(&document.sha256)?;
    let path = format!("{}/{}", document.dir, document.file);
    let mut inputs = Vec::new();
    let mut common = None;
    let mut definition_anchors = BTreeSet::new();
    for row in rows {
        if row.locator.doc != document.file
            || row.doc_sha256 != document.sha256
            || row.locator.page == 0
            || row.locator.page > document.pages
            || [&row.locator.table, &row.locator.row, &row.locator.col]
                .iter()
                .any(|s| !named(s))
            || parse_token(&row.token).ok() != Some(row.value)
        {
            return Err(bad(
                RefusalCode::FrozenRecordMutation507,
                "agreed row differs from its document, locator or exact printed token",
            ));
        }
        let readers: BTreeSet<_> = row.readers.iter().collect();
        if readers.len() < 2
            || readers.len() != row.readers.len()
            || readers.iter().any(|r| !named(r))
        {
            return Err(bad(
                RefusalCode::NotObservable202,
                "at least two distinct agreeing readers are required",
            ));
        }
        if worksheet::period(&row.period)? != period {
            return Err(bad(
                RefusalCode::SupportMismatch102,
                "source reporting window differs from the selected window",
            ));
        }
        let f = fields(row)?;
        let (scope, definition) = worksheet::context(&f, issuer, period)?;
        // Preserve a stated exclusion even for an agreed numeric range. Exact-token and document
        // verification above still run before this nonnumeric refusal.
        super::validate_definition(&definition)?;
        if row.unit != scope.unit() {
            return Err(bad(
                RefusalCode::UndeclaredUnitBasisOrTz107,
                "source monetary unit must exactly match the declared currency and scale",
            ));
        }
        let (_, close) = super::boundaries(period);
        if worksheet::date(&document.published_at)? < close {
            return Err(bad(
                RefusalCode::AssayLookahead108,
                "actual capital-return filing cannot precede the target reporting close",
            ));
        }
        if let Some(prior) = &common {
            if prior != &(scope.clone(), definition.clone()) {
                return Err(bad(
                    RefusalCode::BasisMismatch105,
                    "source rows disagree on scope or the complete capital-return definition",
                ));
            }
        } else {
            common = Some((scope.clone(), definition));
        }
        let get = |key| worksheet::get(&f, key);
        let page = get("definition_page")?
            .parse::<u64>()
            .ok()
            .filter(|p| *p > 0 && *p <= document.pages)
            .ok_or_else(|| {
                bad(
                    RefusalCode::NoClaimInDomain201,
                    "definition citation page is outside the selected filing",
                )
            })?;
        let locator = Locator {
            doc: document.file.clone(),
            page,
            table: get("definition_table")?.into(),
            row: get("definition_row")?.into(),
            col: get("definition_col")?.into(),
        };
        let definition_source = SourceAnchor::new(&path, sha, locator.to_string());
        definition_anchors.insert(definition_source.clone());
        let Token::Point(point) = row.value else {
            return Err(bad(
                RefusalCode::SourceCellUnusable208,
                "earnings and capital require printed points with known precision",
            ));
        };
        let denominator = 10_i128.checked_pow(point.scale).ok_or_else(|| {
            bad(
                RefusalCode::NumericalCertificateFailed207,
                "printed precision overflows",
            )
        })?;
        let amount = Amount {
            value: exact(ExactQty::new(point.mantissa, denominator))?,
            rounding_step: exact(ExactQty::new(1, denominator))?,
        };
        let prefix = format!("{series}_");
        let role = row.quantity.strip_prefix(&prefix).ok_or_else(|| {
            bad(
                RefusalCode::BasisMismatch105,
                "source quantity is outside the selected series",
            )
        })?;
        let value = match role {
            "earnings" | "average" => {
                if f.contains_key("snapshot") {
                    return Err(bad(
                        RefusalCode::SupportMismatch102,
                        "period earnings/average cannot also carry a snapshot date",
                    ));
                }
                if role == "earnings" {
                    InputValue::Earnings { period, amount }
                } else {
                    InputValue::Average { period, amount }
                }
            }
            "opening" => InputValue::Opening {
                as_of: worksheet::date(get("snapshot")?)?,
                amount,
            },
            "closing" => InputValue::Closing {
                as_of: worksheet::date(get("snapshot")?)?,
                amount,
            },
            _ => {
                return Err(bad(
                    RefusalCode::BasisMismatch105,
                    "unsupported capital-return source role",
                ));
            }
        };
        let source_record = Some(SourceRecord {
            quantity: row.quantity.clone(),
            raw_token: row.token.clone(),
            basis: row.basis.clone(),
            readers: readers.into_iter().cloned().collect(),
            published_at: document.published_at.clone(),
            published_at_basis: document.published_at_basis.clone(),
        });
        let mut anchors = vec![
            SourceAnchor::new(&path, sha, row.locator.to_string()),
            definition_source,
        ];
        anchors.sort();
        anchors.dedup();
        inputs.push(claim(
            Input {
                scope,
                value,
                source_record,
            },
            anchors,
        ));
    }
    let (scope, definition) =
        common.ok_or_else(|| bad(RefusalCode::NoClaimInDomain201, "missing source definition"))?;
    inputs.push(claim(
        Input {
            scope,
            value: InputValue::Definition(definition),
            source_record: None,
        },
        definition_anchors.into_iter().collect(),
    ));
    Ok(inputs)
}
