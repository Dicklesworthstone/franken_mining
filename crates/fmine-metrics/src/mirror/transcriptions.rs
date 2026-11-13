//! DESIGN (BD-346): pure interpretation of agreed source rows into physical/capital mirror Claims.
//! The caller authenticates document bytes, publication availability and complete reader agreement.
//! This module binds the supplied document identity, dated perimeter and separately cited equity share.
//! Determinism: Exact

use std::collections::{BTreeMap, BTreeSet};

use fmine_claim::{AdditivityClass, Claim, Confidentiality, Representation, SpatialSupport};
use fmine_core::anchor::SourceAnchor;
use fmine_core::{Encoder, blake3_hash};
use fmine_evidence::{EpistemicStatus, EvidenceClass, KnowledgeState, Refusal, RefusalCode};
use fmine_numerics::exact_feas::{Interval, Rat};
use fmine_registry::sources::{DocumentRow, is_date};
use fmine_registry::transcriptions::{Accepted, Decimal, Locator, Token, parse_token};
use fmine_time::CivilDate;
use fmine_units::UnitsError;

use super::{
    CapexBasis, CapexScope, CapexTiming, LeaseTreatment, MirrorMetric, MirrorQuantity,
    MirrorReading, Perimeter, READING_METRIC,
};
use crate::guidance::{Ownership, Period};

type Result<T> = std::result::Result<T, Refusal>;

/// Source-bound interpretations for one reporter; no source bytes are read by this module.
#[derive(Debug, Clone, PartialEq)]
pub struct BoundReadings {
    pub ownership: Option<Claim<Rat>>,
    pub inputs: Vec<Claim<MirrorReading>>,
}

fn fail(code: RefusalCode, reason: impl Into<String>) -> Refusal {
    Refusal::new(code, format!("JV mirror: {}", reason.into()))
}

fn exact<T>(value: std::result::Result<T, UnitsError>) -> Result<T> {
    value.map_err(|e| fail(RefusalCode::NumericalCertificateFailed207, e.to_string()))
}

fn slug(s: &str) -> bool {
    !s.is_empty()
        && s.bytes()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-')
        && !s.starts_with('-')
        && !s.ends_with('-')
}

/// Registry period label for a calendar year, quarter or half-year.
#[must_use]
pub fn period_label(p: Period) -> String {
    let suffix = match (p.first_month, p.last_month) {
        (1, 12) => "FY",
        (1, 3) => "Q1",
        (4, 6) => "Q2",
        (7, 9) => "Q3",
        (10, 12) => "Q4",
        (1, 6) => "H1",
        (7, 12) => "H2",
        _ => "invalid",
    };
    format!("{}{suffix}", p.year)
}

fn date(s: &str) -> Result<CivilDate> {
    if !is_date(s) {
        return Err(fail(
            RefusalCode::SupportMismatch102,
            "invalid perimeter date",
        ));
    }
    CivilDate::new(
        s[..4].parse().unwrap_or(0),
        s[5..7].parse().unwrap_or(0),
        s[8..].parse().unwrap_or(0),
    )
    .map_err(|e| fail(RefusalCode::SupportMismatch102, e.to_string()))
}

#[derive(Debug, Clone)]
struct Context {
    mode: String,
    perimeter: Perimeter,
    anchor: SourceAnchor,
    capex_basis: Option<CapexBasis>,
    basis_source: Option<SourceAnchor>,
}

fn digest_bytes(s: &str) -> Result<[u8; 32]> {
    if s.len() != 64 || !s.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(fail(
            RefusalCode::DigestMismatch505,
            "invalid source SHA-256",
        ));
    }
    let mut out = [0; 32];
    for (i, b) in out.iter_mut().enumerate() {
        *b = u8::from_str_radix(&s[i * 2..i * 2 + 2], 16)
            .map_err(|_| fail(RefusalCode::DigestMismatch505, "invalid source SHA-256"))?;
    }
    Ok(out)
}

fn anchor(document: &DocumentRow, locator: String) -> Result<SourceAnchor> {
    Ok(SourceAnchor::new(
        format!("{}/{}", document.dir, document.file),
        digest_bytes(&document.sha256)?,
        locator,
    ))
}

/// The two readers agree on both the complete perimeter and the separate page that states its scope.
fn context(
    row: &Accepted,
    document: &DocumentRow,
    capex_scope: Option<CapexScope>,
) -> Result<Context> {
    let mut parts = row.basis.split(';');
    let mode = parts.next().unwrap_or_default().to_string();
    let mut fields = BTreeMap::new();
    for part in parts {
        let (key, value) = part.split_once('=').ok_or_else(|| {
            fail(
                RefusalCode::NonIdentifiableAttribution104,
                "perimeter basis needs explicit key=value fields",
            )
        })?;
        let perimeter_field = matches!(
            key,
            "perimeter"
                | "members"
                | "valid"
                | "scope_page"
                | "scope_table"
                | "scope_row"
                | "scope_col"
        );
        let capex_field = capex_scope.is_some()
            && matches!(
                key,
                "timing"
                    | "leases"
                    | "definition_page"
                    | "definition_table"
                    | "definition_row"
                    | "definition_col"
            );
        if (!perimeter_field && !capex_field)
            || value.trim().is_empty()
            || fields.insert(key, value).is_some()
        {
            return Err(fail(
                RefusalCode::NonIdentifiableAttribution104,
                "unknown, empty or duplicate perimeter field",
            ));
        }
    }
    let get = |key| {
        fields.get(key).copied().ok_or_else(|| {
            fail(
                RefusalCode::NonIdentifiableAttribution104,
                format!("{} lacks {key}", row.locator),
            )
        })
    };
    let id = get("perimeter")?;
    let mut operations: Vec<String> = get("members")?.split('+').map(str::to_string).collect();
    if !slug(id) || operations.iter().any(|s| !slug(s)) {
        return Err(fail(
            RefusalCode::NonIdentifiableAttribution104,
            "perimeter and operation keys must be explicit slugs",
        ));
    }
    operations.sort();
    if operations.windows(2).any(|w| w[0] == w[1]) {
        return Err(fail(
            RefusalCode::NonIdentifiableAttribution104,
            "duplicate perimeter member",
        ));
    }
    let (from, through) = get("valid")?.split_once('/').ok_or_else(|| {
        fail(
            RefusalCode::SupportMismatch102,
            "perimeter valid needs inclusive YYYY-MM-DD/YYYY-MM-DD",
        )
    })?;
    let perimeter = Perimeter {
        id: id.into(),
        operations,
        valid_from: date(from)?,
        valid_through: date(through)?,
    };
    let page = get("scope_page")?
        .parse::<u64>()
        .ok()
        .filter(|p| *p > 0 && *p <= document.pages)
        .ok_or_else(|| {
            fail(
                RefusalCode::NonIdentifiableAttribution104,
                "perimeter scope page is outside the registered document",
            )
        })?;
    let locator = Locator {
        doc: document.file.clone(),
        page,
        table: get("scope_table")?.into(),
        row: get("scope_row")?.into(),
        col: get("scope_col")?.into(),
    };
    let (capex_basis, basis_source) = if let Some(scope) = capex_scope {
        let timing = match fields.get("timing").copied() {
            Some("cash") => CapexTiming::Cash,
            Some("accrual") => CapexTiming::Accrual,
            _ => {
                return Err(fail(
                    RefusalCode::NonIdentifiableAttribution104,
                    "missing or unstated capex timing basis",
                ));
            }
        };
        let leases = match fields.get("leases").copied() {
            Some("included") => LeaseTreatment::Included,
            Some("excluded") => LeaseTreatment::Excluded,
            _ => {
                return Err(fail(
                    RefusalCode::NonIdentifiableAttribution104,
                    "missing or unstated capex lease treatment",
                ));
            }
        };
        let page = get("definition_page")?
            .parse::<u64>()
            .ok()
            .filter(|p| *p > 0 && *p <= document.pages)
            .ok_or_else(|| {
                fail(
                    RefusalCode::NonIdentifiableAttribution104,
                    "capex definition page is outside the registered document",
                )
            })?;
        let definition = Locator {
            doc: document.file.clone(),
            page,
            table: get("definition_table")?.into(),
            row: get("definition_row")?.into(),
            col: get("definition_col")?.into(),
        };
        (
            Some(CapexBasis {
                scope,
                timing,
                leases,
            }),
            Some(anchor(document, definition.to_string())?),
        )
    } else {
        (None, None)
    };
    Ok(Context {
        mode,
        perimeter,
        anchor: anchor(document, locator.to_string())?,
        capex_basis,
        basis_source,
    })
}

fn decimal(d: Decimal) -> Result<(Rat, Rat)> {
    let n = 10_i128.checked_pow(d.scale).ok_or_else(|| {
        fail(
            RefusalCode::NumericalCertificateFailed207,
            "decimal scale overflow",
        )
    })?;
    Ok((exact(Rat::new(d.mantissa, n))?, exact(Rat::new(1, n))?))
}

fn point(row: &Accepted) -> Result<(Rat, Rat)> {
    if row.readers.iter().collect::<BTreeSet<_>>().len() < 2
        || parse_token(&row.token).ok() != Some(row.value)
    {
        return Err(fail(
            RefusalCode::NotObservable202,
            "numeric source needs two agreeing readers and its original printed token",
        ));
    }
    let Token::Point(d) = row.value else {
        return Err(fail(
            RefusalCode::BasisMismatch105,
            "mirror requires a printed actual, not a guidance range",
        ));
    };
    decimal(d)
}

fn new_claim<T>(
    metric: &str,
    value: T,
    tag: &str,
    evidence: EvidenceClass,
    additivity: AdditivityClass,
    parents: Vec<fmine_claim::ClaimId>,
    mut anchors: Vec<SourceAnchor>,
) -> Claim<T> {
    anchors.sort();
    anchors.dedup();
    Claim::new(
        metric,
        value,
        tag,
        Representation::Deterministic,
        SpatialSupport::PeriodAggregate,
        additivity,
        evidence,
        EpistemicStatus::KnownKnown,
        KnowledgeState::Known,
        Confidentiality::public(),
        vec![],
        parents,
        anchors,
    )
}

fn share(
    row: &Accepted,
    document: &DocumentRow,
    period: Period,
    evidence: EvidenceClass,
) -> Result<(Claim<Rat>, Context)> {
    let ctx = context(row, document, None)?;
    if ctx.mode != "equity"
        || row.unit != "percent"
        || row.period != period_label(period)
        || row.locator.doc != document.file
        || row.doc_sha256 != document.sha256
        || row.locator.page == 0
        || row.locator.page > document.pages
    {
        return Err(fail(
            RefusalCode::BasisMismatch105,
            "ownership needs a separately cited exact equity percent for the requested period",
        ));
    }
    let (printed, _) = point(row)?;
    let value = exact(printed.checked_div(Rat::from_integer(100)))?;
    if value.signum() <= 0 || exact(value.checked_cmp(Rat::from_integer(1)))?.is_gt() {
        return Err(fail(
            RefusalCode::BasisMismatch105,
            "ownership percent must lie in (0,100]",
        ));
    }
    let mut e = Encoder::new_frame(*b"JVS1", 1);
    for s in [
        &row.quantity,
        &row.token,
        &row.basis,
        &row.period,
        &document.sha256,
        &document.published_at,
        &document.published_at_basis,
    ] {
        e.put_str(s);
    }
    let tag = blake3_hash(&e.finish()).to_hex();
    Ok((
        new_claim(
            "mirror.ownership_fraction",
            value,
            &tag,
            evidence,
            AdditivityClass::NonAdditive,
            vec![],
            vec![
                anchor(document, row.locator.to_string())?,
                ctx.anchor.clone(),
            ],
        ),
        ctx,
    ))
}

/// Full, attributable and ownership registry keys for a physical or capital metric.
#[must_use]
pub fn keys(subject: &str, period: Period, kind: MirrorMetric) -> [String; 3] {
    let base = format!(
        "mirror_{}_{}",
        subject.replace('-', "_"),
        period_label(period).to_ascii_lowercase()
    );
    let metric = kind.as_str();
    let unit = kind.unit().to_ascii_lowercase();
    [
        format!("{base}_{metric}_100_{unit}"),
        format!("{base}_{metric}_attr_{unit}"),
        format!("{base}_ownership_percent"),
    ]
}

/// Bind already agreed, authenticated rows without IO. Caller-supplied evidence is retained, including
/// `Assumed` for invented fixtures. Complete reader agreement and publication eligibility remain the
/// caller's obligation; these accepted rows cannot prove that a rejected reading was not omitted.
pub fn bind(
    subject: &str,
    period: Period,
    kind: MirrorMetric,
    issuer: &str,
    document: &DocumentRow,
    rows: &[Accepted],
    evidence: EvidenceClass,
) -> Result<BoundReadings> {
    let keys = keys(subject, period, kind);
    let lookup = |key: &str| -> Result<Option<&Accepted>> {
        let matches: Vec<_> = rows.iter().filter(|r| r.quantity == key).collect();
        match matches.as_slice() {
            [] => Ok(None),
            [r] => Ok(Some(*r)),
            _ => Err(fail(
                RefusalCode::NotObservable202,
                format!("multiple readings for {key}"),
            )),
        }
    };
    let full = lookup(&keys[0])?;
    let attr = lookup(&keys[1])?;
    if full.is_none() && attr.is_none() {
        return Err(fail(
            RefusalCode::NoClaimInDomain201,
            format!("{issuer}/{} has no agreed requested metric", document.file),
        ));
    }
    let ownership = if attr.is_some() {
        Some(share(
            lookup(&keys[2])?.ok_or_else(|| {
                fail(
                    RefusalCode::NonIdentifiableAttribution104,
                    format!(
                        "{issuer}/{} lacks the cited ownership percent",
                        document.file
                    ),
                )
            })?,
            document,
            period,
            evidence,
        )?)
    } else {
        None
    };
    let mut claims = Vec::new();
    for (row, attributable) in [(full, false), (attr, true)] {
        let Some(row) = row else {
            continue;
        };
        if row.unit != kind.unit()
            || row.period != period_label(period)
            || row.locator.doc != document.file
            || row.doc_sha256 != document.sha256
            || row.locator.page == 0
            || row.locator.page > document.pages
        {
            return Err(fail(
                RefusalCode::BasisMismatch105,
                format!(
                    "{} needs the selected document, exact requested period and {}",
                    row.locator,
                    kind.unit()
                ),
            ));
        }
        let ctx = context(
            row,
            document,
            match kind {
                MirrorMetric::Capex(scope) => Some(scope),
                _ => None,
            },
        )?;
        if ctx.mode != if attributable { "attributable" } else { "100%" } {
            return Err(fail(
                RefusalCode::BasisMismatch105,
                "quantity key and ownership basis disagree",
            ));
        }
        let numeric_source = anchor(document, row.locator.to_string())?;
        let mut anchors = vec![numeric_source.clone(), ctx.anchor];
        anchors.extend(ctx.basis_source.iter().cloned());
        let mut parents = Vec::new();
        let ownership_basis = if attributable {
            let (share, share_context) = ownership.as_ref().ok_or_else(|| {
                fail(
                    RefusalCode::NonIdentifiableAttribution104,
                    "missing ownership",
                )
            })?;
            if share_context.perimeter != ctx.perimeter {
                return Err(fail(
                    RefusalCode::NonIdentifiableAttribution104,
                    "ownership and numeric row need the same complete dated perimeter",
                ));
            }
            anchors.extend(share.anchors.clone());
            parents.push(share.claim_id);
            Ownership::Attributable { share: share.value }
        } else {
            Ownership::Full
        };
        let (value, step) = point(row)?;
        if !kind.is_capex() && value.signum() < 0 {
            return Err(fail(
                RefusalCode::BasisMismatch105,
                "negative printed production/sales is not a physical actual",
            ));
        }
        let half = exact(step.checked_div(Rat::from_integer(2)))?;
        let interval = Interval {
            lo: exact(value.checked_sub(half))?,
            hi: exact(value.checked_add(half))?,
            lo_strict: false,
            hi_strict: false,
        };
        let value = MirrorReading {
            reporter: issuer.into(),
            perimeter: ctx.perimeter,
            source: numeric_source,
            basis_source: ctx.basis_source,
            quantity: MirrorQuantity {
                subject: subject.into(),
                period,
                ownership: ownership_basis,
                kind,
                capex_basis: ctx.capex_basis,
                interval,
            },
        };
        let tag = value.canonical_tag();
        claims.push(new_claim(
            READING_METRIC,
            value,
            &tag,
            evidence,
            AdditivityClass::ExtensiveAdditive,
            parents,
            anchors,
        ));
    }
    Ok(BoundReadings {
        ownership: ownership.map(|s| s.0),
        inputs: claims,
    })
}

#[cfg(test)]
mod tests {
    include!("../../../xtask/testdata/metrics_mirror_transcriptions_controls.rs");
}
