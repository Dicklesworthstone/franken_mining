//! DESIGN (BD-346): pure interpretation of agreed source rows into production/sales mirror Claims.
//! The caller authenticates document bytes, publication availability and complete reader agreement.
//! This module binds the supplied document identity, dated perimeter and separately cited equity share.
//! Determinism: Exact.

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

use super::{MirrorReading, Perimeter, READING_METRIC};
use crate::guidance::{Ownership, Period, Quantity, QuantityKind};

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
fn context(row: &Accepted, document: &DocumentRow) -> Result<Context> {
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
        if !matches!(
            key,
            "perimeter"
                | "members"
                | "valid"
                | "scope_page"
                | "scope_table"
                | "scope_row"
                | "scope_col"
        ) || value.is_empty()
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
    Ok(Context {
        mode,
        perimeter,
        anchor: anchor(document, locator.to_string())?,
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
    let ctx = context(row, document)?;
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

/// Full, attributable and ownership registry keys. `bind` accepts only produced/sold kinds.
#[must_use]
pub fn keys(subject: &str, period: Period, kind: QuantityKind) -> [String; 3] {
    let base = format!(
        "mirror_{}_{}",
        subject.replace('-', "_"),
        period_label(period).to_ascii_lowercase()
    );
    let metric = if kind == QuantityKind::ProductionKoz {
        "gold_produced"
    } else {
        "gold_sold"
    };
    [
        format!("{base}_{metric}_100_koz"),
        format!("{base}_{metric}_attr_koz"),
        format!("{base}_ownership_percent"),
    ]
}

/// Bind already agreed, authenticated rows without IO. Caller-supplied evidence is retained, including
/// `Assumed` for invented fixtures. Complete reader agreement and publication eligibility remain the
/// caller's obligation; these accepted rows cannot prove that a rejected reading was not omitted.
pub fn bind(
    subject: &str,
    period: Period,
    kind: QuantityKind,
    issuer: &str,
    document: &DocumentRow,
    rows: &[Accepted],
    evidence: EvidenceClass,
) -> Result<BoundReadings> {
    if !matches!(kind, QuantityKind::ProductionKoz | QuantityKind::SoldKoz) {
        return Err(fail(
            RefusalCode::BasisMismatch105,
            "mirror supports only gold produced or sold koz",
        ));
    }
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
        if row.unit != "koz"
            || row.period != period_label(period)
            || row.locator.doc != document.file
            || row.doc_sha256 != document.sha256
            || row.locator.page == 0
            || row.locator.page > document.pages
        {
            return Err(fail(
                RefusalCode::BasisMismatch105,
                format!(
                    "{} needs the selected document, exact requested period and gold koz",
                    row.locator
                ),
            ));
        }
        let ctx = context(row, document)?;
        if ctx.mode != if attributable { "attributable" } else { "100%" } {
            return Err(fail(
                RefusalCode::BasisMismatch105,
                "quantity key and ownership basis disagree",
            ));
        }
        let numeric_source = anchor(document, row.locator.to_string())?;
        let mut anchors = vec![numeric_source.clone(), ctx.anchor];
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
        if value.signum() < 0 {
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
            quantity: Quantity {
                subject: subject.into(),
                period,
                ownership: ownership_basis,
                kind,
                definition: String::new(),
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
    use super::*;

    const PERIOD: Period = Period {
        year: 2025,
        first_month: 1,
        last_month: 12,
    };

    fn fixture() -> (DocumentRow, Vec<Accepted>) {
        let document = DocumentRow {
            file: "Invented.pdf".into(),
            dir: "synthetic/alpha".into(),
            kind: "partner_filing".into(),
            url: "https://example.invalid/Invented.pdf".into(),
            url_basis: "DESIGN: invented source for a pure known-answer test".into(),
            published_at: "2026-01-15".into(),
            published_at_basis: "FACT: invented test date".into(),
            pages: 2,
            bytes: 1,
            sha256: "11".repeat(32),
        };
        let keys = keys("invented-jv", PERIOD, QuantityKind::ProductionKoz);
        let rows = [(&keys[1], "615", "attributable", "koz"), (&keys[2], "61.5", "equity", "percent")]
            .into_iter()
            .map(|(key, token, mode, unit)| Accepted {
                locator: Locator {
                    doc: document.file.clone(), page: 1, table: "Invented assertions".into(),
                    row: key.clone(), col: "2025".into(),
                },
                doc_sha256: document.sha256.clone(), token: token.into(),
                value: parse_token(token).expect("invented numeric token"), quantity: key.clone(),
                unit: unit.into(),
                basis: format!("{mode};perimeter=invented-jv;members=mine-a+mine-b;valid=2025-01-01/2025-12-31;scope_page=2;scope_table=Scope;scope_row=Operations;scope_col=2025"),
                period: period_label(PERIOD), readers: vec!["synthetic-a".into(), "synthetic-b".into()],
            }).collect();
        (document, rows)
    }

    fn bind_fixture(document: &DocumentRow, rows: &[Accepted]) -> Result<BoundReadings> {
        bind(
            "invented-jv",
            PERIOD,
            QuantityKind::ProductionKoz,
            "alpha",
            document,
            rows,
            EvidenceClass::Assumed,
        )
    }

    #[test]
    fn pure_binding_retains_exact_share_rounding_and_context_parents() {
        let (document, rows) = fixture();
        let bound = bind_fixture(&document, &rows).expect("bind agreed invented rows");
        let ownership = bound.ownership.expect("separately cited ownership");
        assert_eq!(
            ownership.value,
            Rat::new(123, 200).expect("exact legal share")
        );
        assert_eq!(ownership.evidence, EvidenceClass::Assumed);
        assert_eq!(bound.inputs.len(), 1);
        let input = &bound.inputs[0];
        assert_eq!(input.parents, vec![ownership.claim_id]);
        assert!(ownership.anchors.iter().all(|a| input.anchors.contains(a)));
        assert_eq!(
            input.value.quantity.ownership,
            Ownership::Attributable {
                share: ownership.value
            }
        );
        assert_eq!(
            input.value.quantity.interval,
            Interval {
                lo: Rat::new(1229, 2).expect("closed lower source edge"),
                hi: Rat::new(1231, 2).expect("closed upper source edge"),
                lo_strict: false,
                hi_strict: false,
            }
        );
        assert_eq!(input.evidence, EvidenceClass::Assumed);
        assert_eq!(input.value_tag(), input.value.canonical_tag());
        assert!(input.check_id().is_ok());
        assert_eq!(
            input.value.source.artifact_path,
            "synthetic/alpha/Invented.pdf"
        );
        assert!(
            input
                .anchors
                .iter()
                .any(|a| a.locator == "Invented.pdf:p2:Scope:Operations/2025")
        );
    }

    #[test]
    fn both_numeric_and_ownership_rows_must_match_the_declared_document() {
        let (document, rows) = fixture();
        for row in 0..rows.len() {
            for scenario in 0..4 {
                let mut altered = rows.clone();
                match scenario {
                    0 => altered[row].locator.doc = "Another.pdf".into(),
                    1 => altered[row].doc_sha256 = "22".repeat(32),
                    2 => altered[row].locator.page = 0,
                    _ => altered[row].locator.page = document.pages + 1,
                }
                assert_eq!(
                    bind_fixture(&document, &altered)
                        .expect_err("mismatched source metadata")
                        .code,
                    "FMINE-REF-105",
                    "row {row}, scenario {scenario}"
                );
            }
        }
        assert_eq!(
            bind(
                "invented-jv",
                PERIOD,
                QuantityKind::AiscUsdPerSoldOz,
                "alpha",
                &document,
                &rows,
                EvidenceClass::Assumed
            )
            .expect_err("costs cannot use a production mirror")
            .code,
            "FMINE-REF-105"
        );
    }
}
