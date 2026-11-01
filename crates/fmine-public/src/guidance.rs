//! Public guidance completion (BD-342): source selection for the exact metric kernel.
//!
//! DESIGN: accepted two-reader quantities are `guidance_<site>_production_koz` and
//! `guidance_<site>_aisc_usd_per_oz`, with hyphens in the site key replaced by underscores, period `YYYYFY`,
//! and units `koz` / `usd_per_oz`. Basis is `100%` or `attributable:n/d`; the AISC basis additionally ends in
//! `;aisc=issuer-reported-per-sold-oz`. That explicit assertion compares the issuer's reported metric, never
//! an adjusted metric or an inferred WGC definition. Guidance limits are exact; actual cells use closed
//! half-last-place rounding intervals. Source publication dates are not guessed from workbook quarters.
//!
//! The sales scenario repeats the preceding year's FY-minus-YTD sales, with both cited vintages retained.
//! It is an assumption, not a forecast. T1 jointly replays complete original-vintage rolling windows,
//! preserving each source quarter and its sales/rate dependence. No usable history is T3. Neither tier
//! supplies a probability or a physical-capacity verdict. A separate optional T2 production comparison
//! binds same-period technical-report limits or declared plan/nameplate scenarios. Its absence or failure
//! remains T3/indeterminate and never substitutes for a joint production-cost witness.

use std::collections::BTreeSet;
use std::path::Path;

use fmine_claim::{
    AdditivityClass, Claim, Confidentiality, Representation, SpatialSupport, UuTerm,
};
use fmine_core::anchor::SourceAnchor;
use fmine_core::hash::{Digest32, blake3_hash, sha256};
use fmine_evidence::{
    EpistemicStatus, EvidenceClass, KnowledgeState, KuKind, Refusal, RefusalCode,
};
use fmine_metrics::guidance::{
    self as kernel, CompletionInputs, CompletionReport, Ownership, Period, Quantity, QuantityKind,
};
use fmine_numerics::exact_feas::{Convention, Interval, Rat};
use fmine_registry::sources::{is_date, is_quarter};
use fmine_registry::transcriptions::{Accepted, Agreement, Decimal, Locator, Token, parse_token};
use fmine_units::UnitsError;

use crate::js;
use crate::key::{KeyedCell, OwnershipBasis, PublicValue, Scope};
use crate::sources::{IssuerProfile, load_issuer_profile};
use crate::transcribe::{
    CheckedDocument, TranscriptionReport, load_transcription_quantities_as_of,
};

pub mod capacity;
pub mod history;
pub mod track_record;

const COST_BASIS: &str = "issuer-reported-per-sold-oz";
pub const NO_CLAIM: &str = "Arithmetic and finite historical-window comparisons under a declared sales assumption; no forecast, probability of meeting guidance, mine-wide physical-capacity conclusion or regulatory claim.";

type Result<T> = std::result::Result<T, Refusal>;

#[derive(Debug, Clone)]
pub struct GuidanceCompletionReport {
    pub inputs: CompletionInputs,
    pub completion: CompletionReport,
    pub history: kernel::quarters::QuarterlyHistoryReport,
    pub history_exclusions: Vec<history::HistoryExclusion>,
    pub history_sources: Vec<history::HistorySourceWindow>,
    pub technical: capacity::TechnicalAssessment,
    /// Digest of the deterministic report body, including the requested cut and every selected source.
    pub digest: Digest32,
    body: String,
}

impl GuidanceCompletionReport {
    #[must_use]
    pub fn to_json(&self) -> String {
        format!("{{\"digest\":{},{}}}", js(&self.digest.to_hex()), self.body)
    }
}

fn refuse(code: RefusalCode, why: impl Into<String>) -> Refusal {
    Refusal::new(code, format!("guidance completion: {}", why.into()))
}

fn exact<T>(r: std::result::Result<T, UnitsError>) -> Result<T> {
    r.map_err(|e| refuse(RefusalCode::NumericalCertificateFailed207, e.to_string()))
}

fn bounds(lo: Rat, hi: Rat) -> Interval {
    Interval {
        lo,
        hi,
        lo_strict: false,
        hi_strict: false,
    }
}

fn decimal(d: Decimal) -> Result<Rat> {
    let den = 10_i128.checked_pow(d.scale).ok_or_else(|| {
        refuse(
            RefusalCode::NumericalCertificateFailed207,
            "decimal scale overflow",
        )
    })?;
    exact(Rat::new(d.mantissa, den))
}

fn token_interval(token: Token, rounded: bool) -> Result<Interval> {
    match token {
        Token::Range(lo, hi) if !rounded => Ok(bounds(decimal(lo)?, decimal(hi)?)),
        Token::Point(d) if !rounded => Ok(bounds(decimal(d)?, decimal(d)?)),
        Token::Point(d) => {
            let half = exact(
                decimal(Decimal {
                    mantissa: 1,
                    scale: d.scale,
                })?
                .checked_div(Rat::from_integer(2)),
            )?;
            Convention::Closed.interval(decimal(d)?, half).map_err(|e| {
                refuse(
                    RefusalCode::NumericalCertificateFailed207,
                    format!("rounding interval: {e:?}"),
                )
            })
        }
        Token::Range(..) => Err(refuse(
            RefusalCode::SourceCellUnusable208,
            "an actual cell must print one number",
        )),
    }
}

fn ownership(text: &str) -> Result<Ownership> {
    if text == "100%" {
        return Ok(Ownership::Full);
    }
    let value = text
        .strip_prefix("attributable:")
        .and_then(|s| s.split_once('/'));
    if let Some((n, d)) = value {
        let parsed = n.parse::<i128>().ok().zip(d.parse::<i128>().ok());
        if let Some((n, d)) = parsed
            && n > 0
            && d > 0
            && n <= d
        {
            return Ok(Ownership::Attributable {
                share: exact(Rat::new(n, d))?,
            });
        }
    }
    Err(refuse(
        RefusalCode::UndeclaredUnitBasisOrTz107,
        format!("basis {text:?} must state 100% or attributable:n/d"),
    ))
}

fn cell_ownership(basis: OwnershipBasis) -> Result<Ownership> {
    match basis {
        OwnershipBasis::Consolidated100 => Ok(Ownership::Full),
        OwnershipBasis::Attributable { num, den, .. } => Ok(Ownership::Attributable {
            share: exact(Rat::new(i128::from(num), i128::from(den)))?,
        }),
        OwnershipBasis::Unprinted => Err(refuse(
            RefusalCode::UndeclaredUnitBasisOrTz107,
            "source ownership basis is unprinted; it cannot be treated as 100%",
        )),
    }
}

fn request(through: &str, as_of: &str) -> Result<(u16, u8)> {
    if !is_quarter(through) || !is_date(as_of) {
        return Err(refuse(
            RefusalCode::UndeclaredUnitBasisOrTz107,
            "expected through YYYYQn and as-of YYYY-MM-DD",
        ));
    }
    let year = through[..4]
        .parse::<u16>()
        .map_err(|_| refuse(RefusalCode::SupportMismatch102, "invalid year"))?;
    let quarter = through.as_bytes()[5] - b'0';
    if year <= 1 || quarter == 4 {
        return Err(refuse(
            RefusalCode::SupportMismatch102,
            "completion needs Q1, Q2 or Q3 and a preceding year",
        ));
    }
    let end = format!(
        "{year:04}-{:02}-{}",
        quarter * 3,
        if quarter == 1 { "31" } else { "30" }
    );
    if as_of < end.as_str() {
        return Err(refuse(
            RefusalCode::AssayLookahead108,
            "as-of precedes the end of the requested YTD actuals",
        ));
    }
    Ok((year, quarter * 3))
}

fn available_by(profile: &IssuerProfile, as_of: &str) -> Result<String> {
    let date = profile
        .retrieved_utc
        .split('/')
        .filter_map(|s| s.get(..10))
        .filter(|s| is_date(s))
        .max()
        .ok_or_else(|| {
            refuse(
                RefusalCode::AssayLookahead108,
                "workbook registry has no dated availability evidence",
            )
        })?;
    if date > as_of {
        return Err(refuse(
            RefusalCode::AssayLookahead108,
            format!(
                "workbook publication dates are unregistered; verified retrieval {date} does not support an as-of cut of {as_of}"
            ),
        ));
    }
    Ok(date.to_string())
}

fn site_key(subject: &str) -> String {
    subject
        // The registered Mine Stats header prints Zaldívar; its transcription key is ASCII "zaldivar".
        .replace(['í', 'Í'], "i")
        .to_ascii_lowercase()
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

fn guidance_quantities(site: &str) -> [String; 2] {
    let stem = format!("guidance_{}", site.replace('-', "_"));
    [
        format!("{stem}_production_koz"),
        format!("{stem}_aisc_usd_per_oz"),
    ]
}

fn guidance_rows<'a>(
    report: &'a TranscriptionReport,
    site: &str,
    year: u16,
    as_of: &str,
) -> Result<([&'a Accepted; 2], &'a CheckedDocument)> {
    let keys = guidance_quantities(site);
    let period = format!("{year}FY");
    if !report.agreement.is_consistent() {
        return Err(refuse(
            RefusalCode::NotObservable202,
            "guidance readers disagree",
        ));
    }
    let candidates: Vec<_> = report
        .documents
        .iter()
        .filter(|d| {
            is_date(&d.published_at)
                && d.published_at.as_str() <= as_of
                && report.agreement.accepted.iter().any(|a| {
                    a.locator.doc == d.file && a.period == period && keys.contains(&a.quantity)
                })
        })
        .collect();
    let latest = candidates
        .iter()
        .map(|d| d.published_at.as_str())
        .max()
        .ok_or_else(|| {
            refuse(
                RefusalCode::NoClaimInDomain201,
                format!("no two-reader {period} guidance for {site} published by {as_of}"),
            )
        })?;
    if report.agreement.single_reader.iter().any(|(at, _)| {
        report.documents.iter().any(|d| {
            d.file == at.doc
                && is_date(&d.published_at)
                && d.published_at.as_str() >= latest
                && d.published_at.as_str() <= as_of
        })
    }) {
        return Err(refuse(
            RefusalCode::NotObservable202,
            "a document at or after the selected guidance vintage has pending single-reader rows; complete agreement before selecting older guidance",
        ));
    }
    let docs: Vec<_> = candidates
        .into_iter()
        .filter(|d| d.published_at == latest)
        .collect();
    let [doc] = docs.as_slice() else {
        return Err(refuse(
            RefusalCode::NonIdentifiableAttribution104,
            "more than one guidance document has the latest publication date",
        ));
    };
    let doc = *doc;
    let row = |key: &str| -> Result<&Accepted> {
        let rows: Vec<_> = report
            .agreement
            .accepted
            .iter()
            .filter(|a| a.locator.doc == doc.file && a.period == period && a.quantity == key)
            .collect();
        let [a] = rows.as_slice() else {
            return Err(refuse(
                RefusalCode::NotObservable202,
                format!(
                    "latest guidance document {} needs exactly one agreed {key} row",
                    doc.file
                ),
            ));
        };
        if a.doc_sha256 != doc.sha256 || a.readers.iter().collect::<BTreeSet<_>>().len() < 2 {
            return Err(refuse(
                RefusalCode::NotObservable202,
                format!(
                    "{} lacks two distinct readers of the checked bytes",
                    a.locator
                ),
            ));
        }
        Ok(*a)
    };
    Ok(([row(&keys[0])?, row(&keys[1])?], doc))
}

fn sha_bytes(hex: &str) -> Result<[u8; 32]> {
    if hex.len() != 64 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(refuse(
            RefusalCode::DigestMismatch505,
            "invalid document sha256",
        ));
    }
    let mut out = [0; 32];
    for (index, byte) in out.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&hex[index * 2..index * 2 + 2], 16)
            .map_err(|_| refuse(RefusalCode::DigestMismatch505, "invalid document sha256"))?;
    }
    Ok(out)
}

#[allow(
    clippy::too_many_arguments,
    reason = "source quantity plus its explicit Claim metadata"
)]
fn quantity_claim(
    metric: &str,
    value: Quantity,
    evidence: EvidenceClass,
    epistemic: EpistemicStatus,
    state: KnowledgeState,
    uu_terms: Vec<UuTerm>,
    parents: Vec<fmine_claim::ClaimId>,
    anchors: Vec<SourceAnchor>,
) -> Claim<Quantity> {
    let additivity = if value.kind == QuantityKind::AiscUsdPerSoldOz {
        AdditivityClass::IntensiveWeighted
    } else {
        AdditivityClass::ExtensiveAdditive
    };
    let tag = value.canonical_tag();
    Claim::new(
        metric,
        value,
        &tag,
        Representation::Deterministic,
        SpatialSupport::PeriodAggregate,
        additivity,
        evidence,
        epistemic,
        state,
        Confidentiality::public(),
        uu_terms,
        parents,
        anchors,
    )
}

fn guidance_claim(
    a: &Accepted,
    subject: &str,
    year: u16,
    kind: QuantityKind,
    synthetic: bool,
) -> Result<Claim<Quantity>> {
    let (basis, definition) = if kind == QuantityKind::AiscUsdPerSoldOz {
        let basis = a
            .basis
            .strip_suffix(&format!(";aisc={COST_BASIS}"))
            .ok_or_else(|| {
                refuse(
                    RefusalCode::UndeclaredUnitBasisOrTz107,
                    format!(
                        "{} must explicitly match the issuer-reported AISC per sold ounce",
                        a.locator
                    ),
                )
            })?;
        (basis, COST_BASIS)
    } else {
        (a.basis.as_str(), "")
    };
    let expected_unit = if kind == QuantityKind::ProductionKoz {
        "koz"
    } else {
        "usd_per_oz"
    };
    if a.unit != expected_unit {
        return Err(refuse(
            RefusalCode::BasisMismatch105,
            format!("{}: unit {:?} is not {expected_unit}", a.locator, a.unit),
        ));
    }
    Ok(quantity_claim(
        &a.quantity,
        Quantity {
            subject: subject.into(),
            period: Period {
                year,
                first_month: 1,
                last_month: 12,
            },
            ownership: ownership(basis)?,
            kind,
            definition: definition.into(),
            interval: token_interval(a.value, false)?,
        },
        if synthetic {
            EvidenceClass::Assumed
        } else {
            EvidenceClass::ISSUER
        },
        EpistemicStatus::KnownKnown,
        KnowledgeState::Known,
        vec![],
        vec![],
        vec![SourceAnchor::new(
            &a.locator.doc,
            sha_bytes(&a.doc_sha256)?,
            a.locator.to_string(),
        )],
    ))
}

fn select_cell<'a>(
    cells: &'a [KeyedCell],
    subject: &str,
    release: &str,
    period: &str,
    line: &str,
) -> Result<&'a KeyedCell> {
    let matches: Vec<_> = cells
        .iter()
        .filter(|c| {
            c.key.subject == subject
                && c.key.reporting_vintage.release == release
                && c.key.valid_period.code() == period
                && c.key.line.label == line
                && matches!(c.key.scope, Scope::Site)
        })
        .collect();
    match matches.as_slice() {
        [one] => Ok(one),
        [] => Err(refuse(
            RefusalCode::NoClaimInDomain201,
            format!("no {subject} {line} for {period} in release {release}"),
        )),
        _ => Err(refuse(
            RefusalCode::NonIdentifiableAttribution104,
            format!("multiple {subject} {line} cells for {period} in release {release}"),
        )),
    }
}

fn actual_claim(c: &KeyedCell, kind: QuantityKind) -> Result<Claim<Quantity>> {
    if matches!(c.claim.representation, Representation::Placeholder { .. }) {
        return Err(refuse(
            RefusalCode::SourceCellUnusable208,
            format!("{}: a placeholder cannot supply an actual", c.locator),
        ));
    }
    if !c.claim.confidentiality.is_public() {
        return Err(refuse(
            RefusalCode::MnpiWall301,
            format!(
                "{}: non-public actual cannot enter public guidance",
                c.locator
            ),
        ));
    }
    if c.claim.stale || c.claim.calibration_failed {
        return Err(refuse(
            if c.claim.stale {
                RefusalCode::StaleAnchor502
            } else {
                RefusalCode::CalibrationRequired205
            },
            format!("{}: stale or failed-calibration source Claim", c.locator),
        ));
    }
    if !matches!(
        c.claim.state,
        KnowledgeState::Known | KnowledgeState::Estimated
    ) {
        return Err(refuse(
            RefusalCode::NotObservable202,
            format!("{}: source Claim is not usable", c.locator),
        ));
    }
    if c.claim.support != SpatialSupport::PeriodAggregate {
        return Err(refuse(
            RefusalCode::SupportMismatch102,
            "actual needs period-aggregate support",
        ));
    }
    let additivity = if kind == QuantityKind::AiscUsdPerSoldOz {
        AdditivityClass::IntensiveWeighted
    } else {
        AdditivityClass::ExtensiveAdditive
    };
    if c.claim.additivity != additivity {
        return Err(refuse(
            RefusalCode::NonAdditiveAverage103,
            "actual additivity does not match its quantity",
        ));
    }
    let PublicValue::Number {
        displayed: Ok(text),
        under_status: None,
        ..
    } = &c.claim.value
    else {
        return Err(refuse(
            RefusalCode::SourceCellUnusable208,
            format!(
                "{}: missing, unreadable or under a subject-status restriction",
                c.locator
            ),
        ));
    };
    let unit = if kind == QuantityKind::AiscUsdPerSoldOz {
        "usd_per_oz"
    } else {
        "koz"
    };
    if c.key.unit != unit {
        return Err(refuse(
            RefusalCode::BasisMismatch105,
            format!("{}: expected {unit}, found {}", c.locator, c.key.unit),
        ));
    }
    let token = parse_token(text).map_err(|e| {
        refuse(
            RefusalCode::SourceCellUnusable208,
            format!("{}: {e}", c.locator),
        )
    })?;
    let ((year, first_month), (_, last_month)) = c.key.valid_period.months();
    Ok(quantity_claim(
        &c.key.line.label,
        Quantity {
            subject: c.key.subject.clone(),
            period: Period {
                year,
                first_month,
                last_month,
            },
            ownership: cell_ownership(c.key.ownership_basis)?,
            kind,
            definition: if kind == QuantityKind::AiscUsdPerSoldOz {
                COST_BASIS.into()
            } else {
                String::new()
            },
            interval: token_interval(token, true)?,
        },
        c.claim.evidence,
        c.claim.epistemic,
        c.claim.state,
        c.claim.uu_terms.clone(),
        vec![c.claim.claim_id],
        c.claim.anchors.clone(),
    ))
}

fn period_code(year: u16, months: u8) -> String {
    format!(
        "{year}{}",
        match months {
            3 => "Q1",
            6 => "H1",
            9 => "9M",
            _ => "FY",
        }
    )
}

#[allow(
    clippy::too_many_arguments,
    reason = "requested evidence cut, required sources and independently scoped optional technical evidence"
)]
fn assemble(
    issuer: &str,
    site: &str,
    through: &str,
    as_of: &str,
    availability: &str,
    transcripts: &TranscriptionReport,
    cells: &[KeyedCell],
    technical_source: Result<Option<&TranscriptionReport>>,
    synthetic: bool,
) -> Result<GuidanceCompletionReport> {
    let (year, months) = request(through, as_of)?;
    let subjects: BTreeSet<_> = cells
        .iter()
        .filter(|c| site_key(&c.key.subject) == site)
        .map(|c| c.key.subject.as_str())
        .collect();
    let subject = match subjects.iter().copied().collect::<Vec<_>>().as_slice() {
        [one] => *one,
        _ => {
            return Err(refuse(
                RefusalCode::NoClaimInDomain201,
                format!("site key {site:?} does not identify one source subject"),
            ));
        }
    };
    let (guidance, doc) = guidance_rows(transcripts, site, year, as_of)?;
    let production_guidance = guidance_claim(
        guidance[0],
        subject,
        year,
        QuantityKind::ProductionKoz,
        synthetic,
    )?;
    let aisc_guidance = guidance_claim(
        guidance[1],
        subject,
        year,
        QuantityKind::AiscUsdPerSoldOz,
        synthetic,
    )?;
    let prod_line = if production_guidance.value.ownership == Ownership::Full {
        "summary:au_prod_100_koz"
    } else {
        "summary:au_prod_attr_koz"
    };
    let ytd = period_code(year, months);
    let production = select_cell(cells, subject, through, &ytd, prod_line)?;
    let sold = select_cell(cells, subject, through, &ytd, "summary:au_sold_attr_koz")?;
    let aisc = select_cell(cells, subject, through, &ytd, "summary:aisc_usd_per_oz")?;
    let prior = year - 1;
    let historic_ytd = select_cell(
        cells,
        subject,
        &format!("{prior}Q{}", months / 3),
        &period_code(prior, months),
        "summary:au_sold_attr_koz",
    )?;
    let historic_fy = select_cell(
        cells,
        subject,
        &format!("{prior}Q4"),
        &format!("{prior}FY"),
        "summary:au_sold_attr_koz",
    )?;
    let (fy, hytd) = (
        actual_claim(historic_fy, QuantityKind::SoldKoz)?,
        actual_claim(historic_ytd, QuantityKind::SoldKoz)?,
    );
    if fy.value.ownership != hytd.value.ownership
        || fy.value.ownership != production_guidance.value.ownership
    {
        return Err(refuse(
            RefusalCode::BasisMismatch105,
            "historical sales and current guidance have different ownership bases; a divestment or share change needs an explicit bridge",
        ));
    }
    let mut anchors = fy.anchors.clone();
    anchors.extend(hytd.anchors.iter().cloned());
    let mut uu_terms = fy.uu_terms.clone();
    for term in &hytd.uu_terms {
        if !uu_terms.iter().any(|u| u.scenario_id == term.scenario_id) {
            uu_terms.push(term.clone());
        }
    }
    let remaining_sold = quantity_claim(
        "guidance.remaining_sold.historical_persistence/1",
        Quantity {
            subject: subject.into(),
            period: Period {
                year,
                first_month: months + 1,
                last_month: 12,
            },
            ownership: fy.value.ownership,
            kind: QuantityKind::SoldKoz,
            definition: String::new(),
            interval: bounds(
                exact(fy.value.interval.lo.checked_sub(hytd.value.interval.hi))?,
                exact(fy.value.interval.hi.checked_sub(hytd.value.interval.lo))?,
            ),
        },
        EvidenceClass::Assumed.meet(fy.evidence).meet(hytd.evidence),
        EpistemicStatus::KnownUnknown(KuKind::Parametric)
            .and(fy.epistemic)
            .and(hytd.epistemic),
        KnowledgeState::Estimated,
        uu_terms,
        vec![fy.claim_id, hytd.claim_id],
        anchors,
    );
    let inputs = CompletionInputs {
        production_guidance,
        aisc_guidance,
        ytd_production: actual_claim(production, QuantityKind::ProductionKoz)?,
        ytd_sold: actual_claim(sold, QuantityKind::SoldKoz)?,
        ytd_aisc: actual_claim(aisc, QuantityKind::AiscUsdPerSoldOz)?,
        remaining_sold,
    };
    let completion = kernel::complete(&inputs)?;
    let historical = history::compare(&inputs, cells)?;
    let technical = capacity::assess(
        &completion.remaining_production,
        technical_source,
        site,
        as_of,
        synthetic,
    );
    let selected = [production, sold, aisc, historic_fy, historic_ytd];
    let sources = selected
        .iter()
        .map(|c| c.to_json())
        .collect::<Vec<_>>()
        .join(",");
    let rows = guidance.iter().map(|a| format!("{{\"locator\":{},\"sha256\":{},\"token\":{},\"quantity\":{},\"unit\":{},\"basis\":{},\"period\":{},\"reader_count\":{}}}", js(&a.locator.to_string()), js(&a.doc_sha256), js(&a.token), js(&a.quantity), js(&a.unit), js(&a.basis), js(&a.period), a.readers.len())).collect::<Vec<_>>().join(",");
    let frontier = completion
        .frontier
        .iter()
        .map(|p| {
            format!(
                "{{\"annual_aisc\":{},\"remaining_aisc_spend\":{},\"remaining_aisc_rate\":{}}}",
                quantity_json(&p.annual_aisc),
                quantity_json(&p.remaining_aisc_spend),
                quantity_json(&p.remaining_aisc_rate)
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let body = format!(
        "\"schema\":\"fmine.guidance.completion.v1\",\"lane\":{},\"gate_eligible\":false,\"issuer\":{},\"site\":{},\"through\":{},\"as_of\":{},\"workbooks_available_by\":{},\"availability_basis\":\"verified registry retrieval; workbook publication dates unregistered\",\"guidance_published_at\":{},\"guidance_published_at_basis\":{},\"envelope_tier\":{},\"verdict\":{},\"sales_scenario\":\"ASSUMPTION: repeat preceding-year FY-minus-YTD sold ounces, using that year's corresponding registered releases; not a forecast\",\"seasonality\":\"Same-season rolling windows are marked as calendar context only; all equal-duration windows enter the historical comparison.\",\"inputs\":{{\"production_guidance\":{},\"aisc_guidance\":{},\"ytd_production\":{},\"ytd_sold\":{},\"ytd_aisc\":{},\"remaining_sold\":{}}},\"requirements\":{{\"remaining_production\":{},\"ytd_aisc_spend\":{},\"remaining_aisc_rate\":{},\"remaining_aisc_spend\":{}}},\"frontier\":[{}],\"spending_requirement\":{},\"guidance_sources\":[{}],\"source_claims\":[{}],\"historical_sales\":[{},{}],\"history\":{},\"technical\":{},\"warnings\":[\"Historical remaining sales subtract two published vintages; restatements and future inventory changes are not predicted.\",\"AISC comparisons require the two-reader assertion of the same issuer-reported definition; adjusted guidance is not supported.\",\"Historical joint tests and technical production comparisons retain separate tiers and assumptions; no technical input supplies a joint production-cost witness.\"],\"no_claim\":{}",
        js(if synthetic { "synthetic" } else { "public" }),
        js(issuer),
        js(site),
        js(through),
        js(as_of),
        js(availability),
        js(&doc.published_at),
        js(&doc.published_at_basis),
        js(history::tier(&historical.comparison)),
        js(history::verdict(&historical.comparison)),
        quantity_json(&inputs.production_guidance),
        quantity_json(&inputs.aisc_guidance),
        quantity_json(&inputs.ytd_production),
        quantity_json(&inputs.ytd_sold),
        quantity_json(&inputs.ytd_aisc),
        quantity_json(&inputs.remaining_sold),
        quantity_json(&completion.remaining_production),
        quantity_json(&completion.ytd_aisc_spend),
        quantity_json(&completion.remaining_aisc_rate),
        quantity_json(&completion.remaining_aisc_spend),
        frontier,
        js(match completion.spending_requirement {
            kernel::SpendingRequirement::Nonnegative => "nonnegative",
            kernel::SpendingRequirement::IncludesNegative => "includes_negative",
            kernel::SpendingRequirement::EntirelyNegative => "entirely_negative",
        }),
        rows,
        sources,
        quantity_json(&fy),
        quantity_json(&hytd),
        history::to_json(&historical),
        capacity::to_json(&technical),
        js(NO_CLAIM)
    );
    Ok(GuidanceCompletionReport {
        inputs,
        completion,
        history: historical.comparison,
        history_exclusions: historical.exclusions,
        history_sources: historical.sources,
        technical,
        digest: blake3_hash(body.as_bytes()),
        body,
    })
}

fn claim_fields<T>(c: &Claim<T>) -> String {
    let anchors = c
        .anchors
        .iter()
        .map(|a| {
            format!(
                "{{\"file\":{},\"sha256\":{},\"locator\":{}}}",
                js(&a.artifact_path),
                js(&Digest32(a.artifact_sha256).to_hex()),
                js(&a.locator)
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let parents = c
        .parents
        .iter()
        .map(|p| js(&p.to_string()))
        .collect::<Vec<_>>()
        .join(",");
    let uu_terms = c.uu_terms.iter().map(|u| format!("{{\"scenario_id\":{},\"description\":{},\"downside_exposure_usd_m\":{},\"trigger_indicator\":{}}}", js(&u.scenario_id), js(&u.description), if u.downside_exposure_usd_m.is_finite() { u.downside_exposure_usd_m.to_string() } else { "null".into() }, js(&u.trigger_indicator))).collect::<Vec<_>>().join(",");
    format!(
        "\"claim_id\":{},\"metric\":{},\"representation\":\"deterministic_exact_set\",\"support\":\"period_aggregate\",\"additivity\":{},\"evidence\":{},\"epistemic\":{},\"knowledge\":{},\"confidentiality\":\"public\",\"calibration\":\"not_a_probability\",\"uu_terms\":[{}],\"parents\":[{}],\"anchors\":[{}]",
        js(&c.claim_id.to_string()),
        js(&c.metric_key),
        js(&format!("{:?}", c.additivity)),
        js(&format!("{:?}", c.evidence)),
        js(&format!("{:?}", c.epistemic)),
        js(&format!("{:?}", c.state)),
        uu_terms,
        parents,
        anchors
    )
}

fn quantity_json(c: &Claim<Quantity>) -> String {
    let q = &c.value;
    let rat = |r: Rat| js(&format!("{}/{}", r.num(), r.den()));
    let basis = match q.ownership {
        Ownership::Full => "{\"kind\":\"100_percent\"}".into(),
        Ownership::Attributable { share } => {
            format!("{{\"kind\":\"attributable\",\"share\":{}}}", rat(share))
        }
    };
    format!(
        "{{{},\"subject\":{},\"year\":{},\"first_month\":{},\"last_month\":{},\"ownership_basis\":{},\"kind\":{},\"definition\":{},\"interval\":{{\"lo\":{},\"hi\":{},\"closed\":true}}}}",
        claim_fields(c),
        js(&q.subject),
        q.period.year,
        q.period.first_month,
        q.period.last_month,
        basis,
        js(match q.kind {
            QuantityKind::ProductionKoz => "production_koz",
            QuantityKind::SoldKoz => "sold_koz",
            QuantityKind::AiscUsdPerSoldOz => "aisc_usd_per_sold_oz",
            QuantityKind::AiscSpendUsdMillion => "aisc_spend_usd_m",
        }),
        js(&q.definition),
        rat(q.interval.lo),
        rat(q.interval.hi)
    )
}

/// Reads registered bytes only. Missing guidance transcriptions refuse; no fixture replaces an issuer input.
pub fn run_public(
    root: &Path,
    issuer: &str,
    site: &str,
    through: &str,
    as_of: &str,
) -> Result<GuidanceCompletionReport> {
    request(through, as_of)?;
    if issuer == crate::synthetic::SYNTHETIC_ISSUER {
        return Err(refuse(
            RefusalCode::NoClaimInDomain201,
            "use the explicitly synthetic guidance run for invented inputs",
        ));
    }
    let profile = load_issuer_profile(root, issuer)?.through(through)?;
    let availability = available_by(&profile, as_of)?;
    let transcripts = load_transcription_quantities_as_of(&profile, root, as_of, &guidance_quantities(site), None)?.ok_or_else(|| refuse(RefusalCode::NoClaimInDomain201, format!("registries/transcriptions/{issuer}.toml is absent; two readers must transcribe the guidance with registered document hashes and publication dates")))?;
    let technical = load_transcription_quantities_as_of(
        &profile,
        root,
        as_of,
        &capacity::quantities(site),
        None,
    );
    let claims = crate::key::mine_stats::load_claims(&profile, root)?;
    assemble(
        issuer,
        site,
        through,
        as_of,
        &availability,
        &transcripts,
        &claims.claims,
        technical.as_ref().map(Option::as_ref).map_err(Clone::clone),
        false,
    )
}

/// A deterministic invented example through the same source-selection and completion path, never gate evidence.
pub fn run_synthetic() -> Result<GuidanceCompletionReport> {
    let (transcripts, cells) = synthetic_evidence()?;
    let technical = capacity::synthetic_report()?;
    assemble(
        "synthetic",
        "example-mine",
        "2030Q2",
        "2030-07-31",
        "2030-07-31",
        &transcripts,
        &cells,
        Ok(Some(&technical)),
        true,
    )
}

fn synthetic_evidence() -> Result<(TranscriptionReport, Vec<KeyedCell>)> {
    use crate::key::{LineKey, PublicKey, ShareSource, Vintage};
    let digest = sha256(b"synthetic guidance 2030: production 200-230 koz; AISC 1520-1680 USD per sold ounce; attributable 4/5");
    let mut accepted = Vec::new();
    for (suffix, token, unit, basis) in [
        ("production_koz", "200 - 230", "koz", "attributable:4/5"),
        (
            "aisc_usd_per_oz",
            "1520 - 1680",
            "usd_per_oz",
            "attributable:4/5;aisc=issuer-reported-per-sold-oz",
        ),
    ] {
        accepted.push(Accepted {
            locator: Locator {
                doc: "synthetic-guidance.txt".into(),
                page: 1,
                table: "Invented annual guidance".into(),
                row: "Example Mine".into(),
                col: suffix.into(),
            },
            doc_sha256: digest.to_hex(),
            token: token.into(),
            value: parse_token(token).map_err(crate::input_refusal)?,
            quantity: format!("guidance_example_mine_{suffix}"),
            unit: unit.into(),
            basis: basis.into(),
            period: "2030FY".into(),
            readers: vec!["synthetic-reader-a".into(), "synthetic-reader-b".into()],
        });
    }
    let report = TranscriptionReport {
        registry: "synthetic:invented-two-reader-agreement".into(),
        documents: vec![CheckedDocument {
            file: "synthetic-guidance.txt".into(),
            kind: "synthetic".into(),
            published_at: "2030-01-15".into(),
            published_at_basis: "DESIGN: invented publication date".into(),
            sha256: digest.to_hex(),
        }],
        entries: 4,
        agreement: Agreement {
            accepted,
            ..Agreement::default()
        },
        tie: None,
    };
    let mut cells = Vec::new();
    for (release, period, field, token) in [
        ("2030Q2", "2030H1", "au_prod_attr_koz", "68"),
        ("2030Q2", "2030H1", "au_sold_attr_koz", "60"),
        ("2030Q2", "2030H1", "aisc_usd_per_oz", "2283"),
        ("2029Q4", "2029FY", "au_sold_attr_koz", "210"),
        ("2029Q2", "2029H1", "au_sold_attr_koz", "70"),
        ("2029Q3", "2029Q3", "au_prod_attr_koz", "70"),
        ("2029Q3", "2029Q3", "au_sold_attr_koz", "70"),
        ("2029Q3", "2029Q3", "aisc_usd_per_oz", "1300"),
        ("2029Q4", "2029Q4", "au_prod_attr_koz", "70"),
        ("2029Q4", "2029Q4", "au_sold_attr_koz", "70"),
        ("2029Q4", "2029Q4", "aisc_usd_per_oz", "1350"),
    ] {
        let bytes = format!("synthetic|{release}|{period}|{field}|{token}|attributable:4/5");
        let digest = sha256(bytes.as_bytes());
        let key = PublicKey {
            issuer: "synthetic".into(),
            subject: "Example Mine".into(),
            valid_period: crate::key::parse_period_code(period)
                .ok_or_else(|| crate::input_refusal("synthetic period"))?,
            reporting_vintage: Vintage {
                release: release.into(),
                file_sha256: digest.to_hex(),
            },
            ownership_basis: OwnershipBasis::Attributable {
                num: 4,
                den: 5,
                source: ShareSource::Printed,
            },
            scope: Scope::Site,
            unit: if field == "aisc_usd_per_oz" {
                "usd_per_oz".into()
            } else {
                "koz".into()
            },
            line: LineKey {
                label: format!("summary:{field}"),
                occurrence: 0,
            },
        };
        let path = format!("synthetic/{release}/{field}.txt");
        let claim = Claim::new(
            &key.line.label,
            PublicValue::Number {
                stored: token.into(),
                displayed: Ok(token.into()),
                under_status: None,
            },
            &key.canonical_tag(token),
            Representation::Deterministic,
            SpatialSupport::PeriodAggregate,
            if field == "aisc_usd_per_oz" {
                AdditivityClass::IntensiveWeighted
            } else {
                AdditivityClass::ExtensiveAdditive
            },
            EvidenceClass::Assumed,
            EpistemicStatus::KnownUnknown(KuKind::Parametric),
            KnowledgeState::Estimated,
            Confidentiality::public(),
            vec![],
            vec![],
            vec![SourceAnchor::new(&path, digest.0, "invented value")],
        );
        cells.push(KeyedCell {
            locator: path,
            printed_basis: key.ownership_basis,
            key,
            claim,
            basis_evidence: None,
            status_evidence: None,
            label_as_printed: None,
            unit_evidence: None,
        });
    }
    Ok((report, cells))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn render(t: &TranscriptionReport, cells: &[KeyedCell]) -> Result<GuidanceCompletionReport> {
        assemble(
            "synthetic",
            "example-mine",
            "2030Q2",
            "2030-07-31",
            "2030-07-31",
            t,
            cells,
            Ok(None),
            true,
        )
    }

    // The public synthetic completion contract is exercised in xtask/tests/public_guidance_history.rs.

    #[test]
    fn latest_guidance_obeys_cutoff_and_ambiguity_refuses() {
        let (mut t, cells) = synthetic_evidence().unwrap();
        let original = render(&t, &cells).unwrap().digest;
        let mut later = t.documents[0].clone();
        later.file = "future-guidance.txt".into();
        later.published_at = "2030-08-01".into();
        let mut rows = t.agreement.accepted.clone();
        for row in &mut rows {
            row.locator.doc = later.file.clone();
            row.token = "1 - 2".into();
            row.value = parse_token(&row.token).unwrap();
        }
        t.documents.push(later);
        t.agreement.accepted.extend(rows);
        assert_eq!(render(&t, &cells).unwrap().digest, original);
        t.documents[1].published_at = "2030-01-15".into();
        assert_eq!(render(&t, &cells).unwrap_err().code, "FMINE-REF-104");
        t.documents[1].published_at = "2030-07-01".into();
        assert_ne!(render(&t, &cells).unwrap().digest, original);
    }

    #[test]
    fn missing_or_unagreed_guidance_never_falls_back_to_earlier_values() {
        let (mut t, cells) = synthetic_evidence().unwrap();
        let original = render(&t, &cells).unwrap().digest;
        let mut pending_doc = t.documents[0].clone();
        pending_doc.file = "pending-guidance.txt".into();
        pending_doc.published_at = "2030-08-01".into();
        let mut pending = t.agreement.accepted[0].locator.clone();
        pending.doc = pending_doc.file.clone();
        t.documents.push(pending_doc);
        t.agreement
            .single_reader
            .push((pending, "first-reader".into()));
        assert_eq!(render(&t, &cells).unwrap().digest, original);
        t.documents[1].published_at = "2030-07-01".into();
        assert_eq!(render(&t, &cells).unwrap_err().code, "FMINE-REF-202");
        t.agreement.single_reader.clear();
        t.agreement.accepted[0].readers = vec!["only-one".into()];
        assert_eq!(render(&t, &cells).unwrap_err().code, "FMINE-REF-202");
        t.agreement.accepted.clear();
        assert_eq!(render(&t, &cells).unwrap_err().code, "FMINE-REF-201");
    }

    #[test]
    fn refuses_basis_status_ambiguity_and_zero_sales_instead_of_guessing() {
        let (t, cells) = synthetic_evidence().unwrap();
        let mut changed = cells.clone();
        changed[1].key.ownership_basis = OwnershipBasis::Unprinted;
        assert_eq!(render(&t, &changed).unwrap_err().code, "FMINE-REF-107");
        changed = cells.clone();
        changed[4].key.ownership_basis = OwnershipBasis::Consolidated100;
        assert_eq!(render(&t, &changed).unwrap_err().code, "FMINE-REF-105");
        changed = cells.clone();
        if let PublicValue::Number { under_status, .. } = &mut changed[0].claim.value {
            *under_status = Some(crate::key::StatusKind::Disposed);
        }
        assert_eq!(render(&t, &changed).unwrap_err().code, "FMINE-REF-208");
        changed = cells.clone();
        changed.push(cells[0].clone());
        assert_eq!(render(&t, &changed).unwrap_err().code, "FMINE-REF-104");
        changed = cells.clone();
        changed[3].claim.value = changed[4].claim.value.clone();
        assert!(render(&t, &changed).is_err());
    }

    #[test]
    fn source_mutation_moves_requirements_and_future_vintages_do_not() {
        let (t, mut cells) = synthetic_evidence().unwrap();
        let before = render(&t, &cells).unwrap();
        let mut future = cells[0].clone();
        future.key.reporting_vintage.release = "2031Q2".into();
        cells.push(future);
        assert_eq!(before.digest, render(&t, &cells).unwrap().digest);
        cells[3].claim.value = PublicValue::Number {
            stored: "220".into(),
            displayed: Ok("220".into()),
            under_status: None,
        };
        let after = render(&t, &cells).unwrap();
        assert_ne!(before.digest, after.digest);
        assert_ne!(
            before.completion.remaining_aisc_rate.value.interval,
            after.completion.remaining_aisc_rate.value.interval
        );
    }

    #[test]
    fn converted_claims_keep_uncertainty_and_refuse_changed_support() {
        let (t, mut cells) = synthetic_evidence().unwrap();
        cells[3].claim.epistemic = EpistemicStatus::UnmodelledBudget;
        cells[3].claim.uu_terms.push(UuTerm {
            scenario_id: "synthetic-sales-interruption".into(),
            description: "Invented historical accounting uncertainty".into(),
            downside_exposure_usd_m: 1.0,
            trigger_indicator: "source revision".into(),
        });
        let report = render(&t, &cells).unwrap();
        assert_eq!(
            report.inputs.remaining_sold.epistemic,
            EpistemicStatus::UnmodelledBudget
        );
        assert_eq!(
            report.completion.remaining_aisc_rate.uu_terms,
            cells[3].claim.uu_terms
        );
        assert!(report.to_json().contains("synthetic-sales-interruption"));
        cells[0].claim.support = SpatialSupport::PointCore;
        assert_eq!(render(&t, &cells).unwrap_err().code, "FMINE-REF-102");
        cells[0].claim.support = SpatialSupport::PeriodAggregate;
        cells[0].claim.additivity = AdditivityClass::IntensiveWeighted;
        assert_eq!(render(&t, &cells).unwrap_err().code, "FMINE-REF-103");
        cells[0].claim.stale = true;
        assert_eq!(render(&t, &cells).unwrap_err().code, "FMINE-REF-502");
        cells[0].claim.stale = false;
        cells[0].claim.calibration_failed = true;
        assert_eq!(render(&t, &cells).unwrap_err().code, "FMINE-REF-205");
    }

    #[test]
    fn invalid_dates_and_cuts_before_verified_availability_refuse() {
        assert!(request("2030Q2", "2030-02-30").is_err());
        assert_eq!(
            request("2030Q2", "2030-06-29").unwrap_err().code,
            "FMINE-REF-108"
        );
        assert!(request("2030Q4", "2031-01-31").is_err());
        let mut profile = crate::synthetic::synthetic_profile();
        profile.retrieved_utc = "2030-07-31T00:00:00Z".into();
        assert_eq!(
            available_by(&profile, "2030-07-30").unwrap_err().code,
            "FMINE-REF-108"
        );
        assert_eq!(available_by(&profile, "2030-07-31").unwrap(), "2030-07-31");
    }
}
