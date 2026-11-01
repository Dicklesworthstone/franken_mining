//! DESIGN (BD-341): descriptive guidance/outcome binding over authenticated public evidence.
//!
//! Guidance uses the existing two-reader transcription keys `guidance_<site>_<metric>`. Every dated
//! guidance appearance remains a member. Annual outcomes come from the same year's original Q4 release;
//! later comparatives never fill an absent or unusable original. No cost rate is subtracted or averaged.
//! Source verification belongs to `run`; `assess` is the pure projection over its supplied evidence.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use fmine_calib::guidance_scores::claims::{
    self as scores, GuidanceSummary, GuidanceTarget, GuidanceValue, MemberInput, OutcomeValue,
    RangeScore, SeriesKey, SummaryScope,
};
use fmine_claim::{AdditivityClass, Claim, Confidentiality, Representation, SpatialSupport};
use fmine_core::anchor::SourceAnchor;
use fmine_core::codec::Encoder;
use fmine_core::{Digest32, blake3_hash, sha256};
use fmine_evidence::{
    EpistemicStatus, EvidenceClass, KnowledgeState, KuKind, Refusal, RefusalCode,
};
use fmine_numerics::exact_feas::{Convention, Interval, Rat};
use fmine_registry::sources::{is_date, is_quarter};
use fmine_registry::transcriptions::{Accepted, Decimal, Token, parse_token};

use fmine_public::key::{
    KeyedCell, OwnershipBasis, PeriodKind, PublicValue, Scope, StatusKind, SubjectStatus,
};
use fmine_public::transcribe::{TranscriptionReport, load_guidance_transcription_report};

pub const NO_CLAIM: &str = "Descriptive comparison of stated guidance with separately identified published outcomes; no forecast, calibration verdict, site ranking or external time proof. From public filings; no affiliation implied.";
const VERSION: &str = "guidance.track_record/2";
type Result<T> = std::result::Result<T, Refusal>;

fn js(value: &str) -> String {
    format!("\"{}\"", crate::escape_json(value))
}

#[derive(Debug, Clone)]
pub struct OutcomeAssessment {
    pub kind: String,
    pub release: String,
    /// All candidates survive an ambiguous original; no arbitrary first cell is selected.
    pub source_claims: Vec<KeyedCell>,
    pub outcome: Option<Claim<OutcomeValue>>,
    pub score: Option<Claim<RangeScore>>,
    pub reason: Option<String>,
}

#[derive(Debug, Clone)]
pub struct TrackRecordRow {
    pub site: String,
    pub year: u16,
    pub metric: String,
    pub unit: String,
    pub basis: String,
    pub guidance: Claim<GuidanceValue>,
    pub guidance_source: Accepted,
    pub guidance_source_claim: Claim<Accepted>,
    pub published_at: String,
    pub published_at_basis: String,
    pub vintage_kind: String,
    pub original: OutcomeAssessment,
    pub later: Vec<OutcomeAssessment>,
}

#[derive(Debug, Clone)]
pub struct TrackRecordSummary {
    pub published_at: String,
    pub selector: String,
    pub basis: String,
    pub summary: Claim<GuidanceSummary>,
}

#[derive(Debug, Clone)]
pub struct TrackRecordReport {
    pub issuer: String,
    pub as_of: String,
    pub through: Option<String>,
    pub synthetic: bool,
    pub digest: Digest32,
    pub rows: Vec<TrackRecordRow>,
    pub summaries: Vec<TrackRecordSummary>,
    pub gate_fired: bool,
    pub resolved_years: usize,
    pub minimum_sites_per_year: usize,
    pub pending_transcriptions: Vec<String>,
    pub unsupported_guidance: Vec<Accepted>,
    pub source_refusals: Vec<fmine_public::mine_stats::CellRefusal>,
    pub source_claims_digest: Digest32,
    body: String,
}

/// Rows from a retrospective registered snapshot, without asserting an information-availability cut.
pub struct TrackRecordSnapshot {
    pub rows: Vec<TrackRecordRow>,
    pub pending_transcriptions: Vec<String>,
    pub unsupported_guidance: Vec<Accepted>,
}

/// Uses the same checked interpretation and scoring as dated replay; the unbounded cut is internal only.
pub fn assess_snapshot(
    profile: &fmine_public::sources::IssuerProfile,
    transcriptions: &TranscriptionReport,
    claims: &[KeyedCell],
    site: Option<&str>,
) -> Result<TrackRecordSnapshot> {
    let report = assess(TrackRecordInput {
        issuer: &profile.issuer,
        site,
        through: profile.through.as_deref(),
        as_of: "9999-12-31",
        available_by: "9999-12-31",
        transcriptions,
        claims,
        statuses: &profile.subject_statuses,
        synthetic: profile.issuer == fmine_public::synthetic::SYNTHETIC_ISSUER,
    })?;
    Ok(TrackRecordSnapshot {
        rows: report.rows,
        pending_transcriptions: report.pending_transcriptions,
        unsupported_guidance: report.unsupported_guidance,
    })
}

impl TrackRecordReport {
    #[must_use]
    pub fn to_json(&self) -> String {
        format!("{{\"digest\":{},{}}}", js(&self.digest.to_hex()), self.body)
    }
}

/// Already authenticated source inputs. The pure projection does not establish byte or time authenticity.
pub struct TrackRecordInput<'a> {
    pub issuer: &'a str,
    pub site: Option<&'a str>,
    pub through: Option<&'a str>,
    pub as_of: &'a str,
    pub available_by: &'a str,
    pub transcriptions: &'a TranscriptionReport,
    pub claims: &'a [KeyedCell],
    pub statuses: &'a [SubjectStatus],
    pub synthetic: bool,
}

fn refusal(code: RefusalCode, what: impl Into<String>) -> Refusal {
    Refusal::new(code, format!("guidance track record: {}", what.into()))
}

fn exact<T, E: std::fmt::Display>(r: std::result::Result<T, E>) -> Result<T> {
    r.map_err(|e| refusal(RefusalCode::NumericalCertificateFailed207, e.to_string()))
}

fn decimal(d: Decimal) -> Result<Rat> {
    let den = 10_i128.checked_pow(d.scale).ok_or_else(|| {
        refusal(
            RefusalCode::NumericalCertificateFailed207,
            "decimal scale overflow",
        )
    })?;
    exact(Rat::new(d.mantissa, den))
}

fn digest(hex: &str) -> Result<[u8; 32]> {
    if hex.len() != 64 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(refusal(
            RefusalCode::DigestMismatch505,
            "invalid source digest",
        ));
    }
    let mut out = [0; 32];
    for (i, byte) in out.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&hex[2 * i..2 * i + 2], 16)
            .map_err(|_| refusal(RefusalCode::DigestMismatch505, "invalid source digest"))?;
    }
    if out == [0; 32] {
        return Err(refusal(
            RefusalCode::DigestMismatch505,
            "empty source digest",
        ));
    }
    Ok(out)
}

fn slug(s: &str) -> String {
    s.replace(['í', 'Í'], "i")
        .to_ascii_lowercase()
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

fn definition(metric: &str) -> Option<&'static str> {
    match metric {
        "aisc_usd_per_oz" => Some(";aisc=issuer-reported-per-sold-oz"),
        "aisc_usd_per_lb" => Some(";aisc=issuer-reported-per-sold-lb"),
        "tcc_usd_per_oz" => Some(";tcc=issuer-reported-per-sold-oz"),
        "c1_usd_per_lb" => Some(";c1=issuer-reported-per-sold-lb"),
        _ => None,
    }
}

fn basis(a: &Accepted, metric: &str) -> Result<(String, bool)> {
    let ownership = if let Some(suffix) = definition(metric) {
        a.basis.strip_suffix(suffix).ok_or_else(|| {
            refusal(
                RefusalCode::BasisMismatch105,
                format!("{} must state {suffix}", a.locator),
            )
        })?
    } else {
        a.basis.as_str()
    };
    let (canonical, full) = if ownership == "100%" {
        ("100%".into(), true)
    } else {
        let (n, d) = ownership
            .strip_prefix("attributable:")
            .and_then(|s| s.split_once('/'))
            .and_then(|(n, d)| Some((n.parse::<i128>().ok()?, d.parse::<i128>().ok()?)))
            .ok_or_else(|| {
                refusal(
                    RefusalCode::UndeclaredUnitBasisOrTz107,
                    "ownership needs 100% or attributable:n/d",
                )
            })?;
        if n <= 0 || d <= 0 || n > d {
            return Err(refusal(
                RefusalCode::BasisMismatch105,
                "ownership outside (0,1]",
            ));
        }
        let share = exact(Rat::new(n, d))?;
        (
            format!("attributable:{}/{}", share.num(), share.den()),
            false,
        )
    };
    Ok((
        format!("{canonical}{}", definition(metric).unwrap_or("")),
        full,
    ))
}

fn field(metric: &str, full: bool) -> (&'static str, String) {
    let share = if full { "100" } else { "attr" };
    match metric {
        "production_koz" => ("koz", format!("summary:au_prod_{share}_koz")),
        "production_kt" => ("kt", format!("summary:cu_prod_{share}_kt")),
        "aisc_usd_per_oz" | "tcc_usd_per_oz" => ("usd_per_oz", format!("summary:{metric}")),
        _ => ("usd_per_lb", format!("summary:{metric}")),
    }
}

fn actual_basis(c: &KeyedCell, metric: &str) -> Result<String> {
    let ownership = match c.key.ownership_basis {
        OwnershipBasis::Consolidated100 => "100%".into(),
        OwnershipBasis::Attributable { num, den, .. } if num > 0 && den > 0 && num <= den => {
            let share = exact(Rat::new(i128::from(num), i128::from(den)))?;
            format!("attributable:{}/{}", share.num(), share.den())
        }
        _ => {
            return Err(refusal(
                RefusalCode::BasisMismatch105,
                "outcome ownership is unprinted or invalid",
            ));
        }
    };
    Ok(format!("{ownership}{}", definition(metric).unwrap_or("")))
}

fn additive(metric: &str) -> AdditivityClass {
    if metric.starts_with("production_") {
        AdditivityClass::ExtensiveAdditive
    } else {
        AdditivityClass::IntensiveWeighted
    }
}

fn interpreted<T, U>(value: T, tag: &str, metric: &str, source: &Claim<U>) -> Claim<T> {
    Claim::new(
        metric,
        value,
        tag,
        Representation::Deterministic,
        source.support,
        source.additivity,
        EvidenceClass::Derived.meet(source.evidence),
        source.epistemic,
        source.state,
        source.confidentiality.clone(),
        source.uu_terms.clone(),
        vec![source.claim_id],
        source.anchors.clone(),
    )
}

fn guidance(
    a: &Accepted,
    subject: &str,
    metric: &str,
    year: u16,
    published_at: &str,
    synthetic: bool,
) -> Result<(Claim<Accepted>, Claim<GuidanceValue>)> {
    if parse_token(&a.token).ok() != Some(a.value) {
        return Err(refusal(
            RefusalCode::SourceCellUnusable208,
            "guidance token and interpreted value disagree",
        ));
    }
    let (basis, full) = basis(a, metric)?;
    if a.unit != field(metric, full).0 {
        return Err(refusal(
            RefusalCode::BasisMismatch105,
            format!("{}: guidance unit does not match {metric}", a.locator),
        ));
    }
    let target = match a.value {
        Token::Point(d) => GuidanceTarget::Point(decimal(d)?),
        Token::Range(lo, hi) => {
            let (lo, hi) = (decimal(lo)?, decimal(hi)?);
            if lo == hi {
                GuidanceTarget::Point(lo)
            } else {
                GuidanceTarget::Range { lo, hi }
            }
        }
    };
    let value = GuidanceValue {
        key: SeriesKey {
            subject: subject.into(),
            metric: metric.into(),
            unit: a.unit.clone(),
            basis,
            year,
        },
        target,
    };
    let tag = value.canonical_tag();
    let mut enc = Encoder::new_frame(*b"GTRW", 1);
    for field in [
        a.locator.to_string(),
        a.doc_sha256.clone(),
        a.token.clone(),
        a.quantity.clone(),
        a.period.clone(),
        a.unit.clone(),
        a.basis.clone(),
        published_at.into(),
    ] {
        enc.put_str(&field);
    }
    let source = Claim::new(
        "guidance.published_statement",
        a.clone(),
        &blake3_hash(&enc.finish()).to_hex(),
        Representation::Deterministic,
        SpatialSupport::PeriodAggregate,
        additive(metric),
        if synthetic {
            EvidenceClass::Assumed
        } else {
            EvidenceClass::ISSUER
        },
        if synthetic {
            EpistemicStatus::KnownUnknown(KuKind::Parametric)
        } else {
            EpistemicStatus::KnownKnown
        },
        if synthetic {
            KnowledgeState::Estimated
        } else {
            KnowledgeState::Known
        },
        Confidentiality::public(),
        vec![],
        vec![],
        vec![SourceAnchor::new(
            &a.locator.doc,
            digest(&a.doc_sha256)?,
            a.locator.to_string(),
        )],
    );
    let target = interpreted(value, &tag, "guidance.interpreted_target", &source);
    Ok((source, target))
}

fn actual(c: &KeyedCell, expected: &SeriesKey) -> Result<Claim<OutcomeValue>> {
    c.claim.check_id()?;
    if c.claim.metric_key != c.key.line.label
        || c.claim.value_tag() != fmine_public::key::canonical_value_tag(&c.key, &c.claim.value)
    {
        return Err(refusal(
            RefusalCode::FrozenRecordMutation507,
            "source key or value differs from its Claim identity",
        ));
    }
    if !c.claim.confidentiality.is_public() {
        return Err(refusal(RefusalCode::MnpiWall301, "non-public outcome"));
    }
    if !c
        .claim
        .anchors
        .iter()
        .any(|a| Digest32(a.artifact_sha256).to_hex() == c.key.reporting_vintage.file_sha256)
    {
        return Err(refusal(
            RefusalCode::DigestMismatch505,
            "outcome vintage digest has no matching source anchor",
        ));
    }
    let PublicValue::Number {
        displayed: Ok(text),
        under_status: None,
        ..
    } = &c.claim.value
    else {
        return Err(refusal(
            RefusalCode::SourceCellUnusable208,
            format!(
                "{}: {}",
                c.locator,
                fmine_public::key::mine_stats::value_kind(&c.claim.value)
            ),
        ));
    };
    let synthetic_number = c.key.issuer == fmine_public::synthetic::SYNTHETIC_ISSUER
        && c.claim.evidence == EvidenceClass::Assumed
        && c.claim.state == KnowledgeState::Unknown
        && matches!(c.claim.representation, Representation::Deterministic);
    if c.claim.stale {
        return Err(refusal(RefusalCode::StaleAnchor502, "stale outcome source"));
    }
    if matches!(c.claim.representation, Representation::Placeholder { .. }) {
        return Err(refusal(
            RefusalCode::SourceCellUnusable208,
            "outcome is an unresolved placeholder",
        ));
    }
    if c.claim.calibration_failed
        || (!synthetic_number
            && !matches!(
                c.claim.state,
                KnowledgeState::Known | KnowledgeState::Estimated
            ))
    {
        return Err(refusal(
            RefusalCode::NotObservable202,
            "stale, unknown or failed-calibration outcome",
        ));
    }
    let Token::Point(d) =
        parse_token(text).map_err(|e| refusal(RefusalCode::SourceCellUnusable208, e))?
    else {
        return Err(refusal(
            RefusalCode::SourceCellUnusable208,
            "outcome is not one printed value",
        ));
    };
    let point = decimal(d)?;
    if expected.metric.starts_with("production_") && point.num() < 0 {
        return Err(refusal(
            RefusalCode::SourceCellUnusable208,
            "production cannot be negative",
        ));
    }
    let half = exact(
        decimal(Decimal {
            mantissa: 1,
            scale: d.scale,
        })?
        .checked_div(Rat::from_integer(2)),
    )?;
    let rounding = Convention::Closed
        .interval(point, half)
        .map_err(|e| refusal(RefusalCode::NumericalCertificateFailed207, format!("{e:?}")))?;
    let value = OutcomeValue {
        key: SeriesKey {
            subject: c.key.subject.clone(),
            metric: expected.metric.clone(),
            unit: c.key.unit.clone(),
            basis: actual_basis(c, &expected.metric)?,
            year: c.key.valid_period.year,
        },
        point,
        rounding,
    };
    let tag = value.canonical_tag();
    Ok(interpreted(
        value,
        &tag,
        "guidance.interpreted_outcome",
        &c.claim,
    ))
}

fn status_reason(statuses: &[SubjectStatus], subject: &str, year: u16) -> Option<String> {
    statuses.iter().find(|s| s.subject == subject && s.first <= (year + 1, 1)
        && s.last.is_none_or(|last| last >= (year,1))
        && (s.first <= (year,12) || s.kind == StatusKind::Disposed))
        .map(|s| format!("annual support intersects {:?}; no unchanged full-year outcome is established ({})", s.kind, s.evidence))
}

fn release_visible(release: &str, as_of: &str) -> bool {
    is_quarter(release)
        && match release.as_bytes()[5] {
            b'1' => format!("{}-03-31", &release[..4]),
            b'2' => format!("{}-06-30", &release[..4]),
            b'3' => format!("{}-09-30", &release[..4]),
            _ => format!("{}-12-31", &release[..4]),
        }
        .as_str()
            < as_of
}

fn appearance(
    g: &Claim<GuidanceValue>,
    kind: &str,
    release: &str,
    cells: Vec<KeyedCell>,
    reason: Option<String>,
) -> OutcomeAssessment {
    let mut out = OutcomeAssessment {
        kind: kind.into(),
        release: release.into(),
        source_claims: cells,
        outcome: None,
        score: None,
        reason,
    };
    if out.reason.is_some() {
        return out;
    }
    let result = match out.source_claims.as_slice() {
        [one] => actual(one, &g.value.key).and_then(|a| scores::score(g, &a).map(|s| (a, s))),
        [] => Err(refusal(
            RefusalCode::NoClaimInDomain201,
            "original annual outcome is absent; no later comparative is substituted",
        )),
        _ => Err(refusal(
            RefusalCode::NonIdentifiableAttribution104,
            "multiple annual assertions in this release",
        )),
    };
    match result {
        Ok((a, s)) => {
            out.outcome = Some(a);
            out.score = Some(s);
        }
        Err(e) => out.reason = Some(format!("{}: {}", e.code, e.what)),
    }
    out
}

/// Forms the deterministic projection over supplied checked sources, with no I/O and no inferred timestamps.
pub fn assess(input: TrackRecordInput<'_>) -> Result<TrackRecordReport> {
    if input.synthetic != (input.issuer == fmine_public::synthetic::SYNTHETIC_ISSUER) {
        return Err(refusal(
            RefusalCode::BasisMismatch105,
            "synthetic lane and issuer disagree",
        ));
    }
    if input
        .claims
        .iter()
        .any(|c| !c.claim.confidentiality.is_public())
    {
        return Err(refusal(
            RefusalCode::MnpiWall301,
            "non-public Claim supplied to a public report",
        ));
    }
    if !is_date(input.as_of)
        || !is_date(input.available_by)
        || input.available_by > input.as_of
        || input.through.is_some_and(|q| !is_quarter(q))
    {
        return Err(refusal(
            RefusalCode::AssayLookahead108,
            "invalid cut or source availability after as-of",
        ));
    }
    if !input.transcriptions.agreement.is_consistent()
        && !input
            .transcriptions
            .agreement
            .is_complete_for_prefix("guidance_")
    {
        return Err(refusal(
            RefusalCode::NotObservable202,
            "guidance readers disagree",
        ));
    }
    if input.claims.iter().any(|c| c.key.issuer != input.issuer) {
        return Err(refusal(
            RefusalCode::BasisMismatch105,
            "source Claims belong to another issuer",
        ));
    }
    let mut rows = Vec::new();
    let mut unsupported_guidance = Vec::new();
    for a in &input.transcriptions.agreement.accepted {
        let Some(rest) = a.quantity.strip_prefix("guidance_") else {
            continue;
        };
        let metrics = [
            "production_koz",
            "production_kt",
            "aisc_usd_per_oz",
            "aisc_usd_per_lb",
            "tcc_usd_per_oz",
            "c1_usd_per_lb",
        ];
        if input
            .site
            .is_some_and(|s| !rest.starts_with(&format!("{}_", slug(s).replace('-', "_"))))
        {
            continue;
        }
        let mut documents = input
            .transcriptions
            .documents
            .iter()
            .filter(|d| d.file == a.locator.doc);
        let doc = documents.next().ok_or_else(|| {
            refusal(
                RefusalCode::DigestMismatch505,
                "guidance document was not checked",
            )
        })?;
        if documents.next().is_some() {
            return Err(refusal(
                RefusalCode::NotObservable202,
                "guidance needs one checked document",
            ));
        }
        if !is_date(&doc.published_at) {
            return Err(refusal(
                RefusalCode::AssayLookahead108,
                "guidance publication date missing",
            ));
        }
        if doc.published_at.as_str() > input.as_of {
            continue;
        }
        if doc.sha256 != a.doc_sha256 || a.readers.iter().collect::<BTreeSet<_>>().len() < 2 {
            return Err(refusal(
                RefusalCode::NotObservable202,
                "guidance needs two agreeing readers of checked document bytes",
            ));
        }
        let Some((site, metric)) = metrics.iter().find_map(|m| {
            rest.strip_suffix(&format!("_{m}"))
                .map(|s| (s.replace('_', "-"), *m))
        }) else {
            unsupported_guidance.push(a.clone());
            continue;
        };
        if input.site.is_some_and(|s| slug(s) != site) {
            continue;
        }
        let period = fmine_public::key::parse_period_code(&a.period)
            .filter(|p| p.kind == PeriodKind::FullYear)
            .ok_or_else(|| {
                refusal(
                    RefusalCode::SupportMismatch102,
                    "guidance period must be a full calendar year",
                )
            })?;
        let year = period.year;
        let subjects: BTreeSet<_> = input
            .claims
            .iter()
            .filter(|c| slug(&c.key.subject) == site && matches!(c.key.scope, Scope::Site))
            .map(|c| c.key.subject.as_str())
            .collect();
        let subject = match subjects.iter().copied().collect::<Vec<_>>().as_slice() {
            [one] => (*one).to_string(),
            [] => a.locator.row.clone(),
            _ => {
                return Err(refusal(
                    RefusalCode::NonIdentifiableAttribution104,
                    "site selector matches multiple subjects",
                ));
            }
        };
        if slug(&subject) != site {
            return Err(refusal(
                RefusalCode::SupportMismatch102,
                format!("{}: guidance site key and row label disagree", a.locator),
            ));
        }
        let (guidance_source_claim, g) = guidance(
            a,
            &subject,
            metric,
            year,
            &doc.published_at,
            input.synthetic,
        )?;
        let full = g.value.key.basis.starts_with("100%");
        let line = field(metric, full).1;
        let original_release = format!("{year:04}Q4");
        let mut by_release: BTreeMap<String, Vec<KeyedCell>> = BTreeMap::new();
        for c in input.claims.iter().filter(|c| {
            c.key.subject == subject
                && c.key.valid_period == period
                && c.key.line.label == line
                && matches!(c.key.scope, Scope::Site)
                && c.key.reporting_vintage.release >= original_release
                && release_visible(&c.key.reporting_vintage.release, input.as_of)
                && input
                    .through
                    .is_none_or(|q| c.key.reporting_vintage.release.as_str() <= q)
        }) {
            by_release
                .entry(c.key.reporting_vintage.release.clone())
                .or_default()
                .push(c.clone());
        }
        for cells in by_release.values_mut() {
            cells.sort_by(|a, b| {
                (&a.key, &a.locator, a.claim.claim_id).cmp(&(&b.key, &b.locator, b.claim.claim_id))
            });
        }
        // Publication dates for outcome workbooks are not independently registered. Requiring guidance
        // before the annual period closes is conservative and does not mistake retrieval for publication.
        let reason = if doc.published_at >= format!("{year:04}-12-31") {
            Some(
                "guidance was not published before the annual period closed; no prospective score"
                    .into(),
            )
        } else if input.as_of <= format!("{year:04}-12-31").as_str() {
            Some("annual outcome is not yet due at this as-of cut".into())
        } else {
            status_reason(input.statuses, &subject, year)
        };
        let original = appearance(
            &g,
            "first_published",
            &original_release,
            by_release.remove(&original_release).unwrap_or_default(),
            reason.clone(),
        );
        let later = by_release
            .into_iter()
            .map(|(release, cells)| {
                appearance(
                    &g,
                    if original.outcome.is_some() {
                        "later_comparative"
                    } else {
                        "resolved_on_comparative_vintage"
                    },
                    &release,
                    cells,
                    reason.clone(),
                )
            })
            .collect();
        rows.push(TrackRecordRow {
            site: subject,
            year,
            metric: metric.into(),
            unit: a.unit.clone(),
            basis: g.value.key.basis.clone(),
            guidance: g,
            guidance_source: a.clone(),
            guidance_source_claim,
            published_at: doc.published_at.clone(),
            published_at_basis: doc.published_at_basis.clone(),
            vintage_kind: String::new(),
            original,
            later,
        });
    }
    if rows.is_empty() {
        let code = if input.transcriptions.agreement.accepted.is_empty()
            && !input.transcriptions.agreement.single_reader.is_empty()
        {
            RefusalCode::NotObservable202
        } else {
            RefusalCode::NoClaimInDomain201
        };
        return Err(refusal(
            code,
            "no agreed, supported guidance rows match the requested selection",
        ));
    }
    rows.sort_by(|a, b| {
        (
            &a.site,
            a.year,
            &a.metric,
            &a.published_at,
            &a.guidance_source.locator,
        )
            .cmp(&(
                &b.site,
                b.year,
                &b.metric,
                &b.published_at,
                &b.guidance_source.locator,
            ))
    });
    type Family = (String, u16, String, String, String);
    type Cohort = (String, u16, String, String, String);
    let mut previous: BTreeMap<Family, GuidanceValue> = BTreeMap::new();
    let mut unique = BTreeSet::new();
    let mut cohorts: BTreeMap<Cohort, Vec<MemberInput>> = BTreeMap::new();
    let mut resolved_sites: BTreeMap<u16, BTreeSet<String>> = BTreeMap::new();
    for row in &mut rows {
        let key = (
            row.site.clone(),
            row.year,
            row.metric.clone(),
            row.unit.clone(),
            row.basis.clone(),
        );
        if !unique.insert((key.clone(), row.published_at.clone())) {
            return Err(refusal(
                RefusalCode::NonIdentifiableAttribution104,
                "multiple guidance rows share one site, metric, unit, ownership/definition basis, year and publication date",
            ));
        }
        row.vintage_kind = match previous.insert(key, row.guidance.value.clone()) {
            None => "initial_registered",
            Some(old) if old == row.guidance.value => "reaffirmed",
            Some(_) => "revised",
        }
        .into();
        if row.original.outcome.is_some() {
            resolved_sites
                .entry(row.year)
                .or_default()
                .insert(row.site.clone());
        }
        cohorts
            .entry((
                row.metric.clone(),
                row.year,
                row.unit.clone(),
                row.published_at.clone(),
                row.basis.clone(),
            ))
            .or_default()
            .push(MemberInput {
                guidance: row.guidance.clone(),
                outcome: row.original.outcome.clone(),
            });
    }
    let mut summaries = Vec::new();
    for ((_, _, _, published_at, basis), members) in cohorts {
        summaries.push(TrackRecordSummary {
            published_at,
            selector: "first_published".into(),
            basis,
            summary: scores::summarize(&members)?,
        });
    }
    let resolved_years = resolved_sites.len();
    let minimum_sites_per_year = resolved_sites
        .values()
        .map(BTreeSet::len)
        .min()
        .unwrap_or(0);
    let mut pending_transcriptions: Vec<_> = input
        .transcriptions
        .agreement
        .single_reader
        .iter()
        .map(|(at, _)| at.to_string())
        .collect();
    pending_transcriptions.sort();
    unsupported_guidance.sort_by(|a, b| (&a.locator, &a.quantity).cmp(&(&b.locator, &b.quantity)));
    let mut report = TrackRecordReport {
        issuer: input.issuer.into(),
        as_of: input.as_of.into(),
        through: input.through.map(String::from),
        synthetic: input.synthetic,
        digest: Digest32([0; 32]),
        rows,
        summaries,
        gate_fired: resolved_years < 3 || minimum_sites_per_year < 10,
        resolved_years,
        minimum_sites_per_year,
        pending_transcriptions,
        unsupported_guidance,
        source_refusals: vec![],
        source_claims_digest: fmine_public::key::claims_digest(input.claims),
        body: String::new(),
    };
    report.body = report_body(&report, input.available_by);
    report.digest = blake3_hash(report.body.as_bytes());
    Ok(report)
}

/// Loads and verifies registered workbooks and the locally supplied two-reader transcription registry.
pub fn run(
    root: &Path,
    issuer: &str,
    site: Option<&str>,
    through: Option<&str>,
    as_of: &str,
) -> Result<TrackRecordReport> {
    if issuer == "synthetic" {
        return Err(refusal(
            RefusalCode::NoClaimInDomain201,
            "use the explicit synthetic lane",
        ));
    }
    if !is_date(as_of) {
        return Err(refusal(
            RefusalCode::AssayLookahead108,
            "as-of must be YYYY-MM-DD",
        ));
    }
    let mut profile = fmine_public::sources::load_issuer_profile(root, issuer)?;
    if let Some(q) = through {
        profile = profile.through(q)?;
    }
    let available_by = profile
        .retrieved_utc
        .split('/')
        .filter_map(|s| s.get(..10))
        .filter(|s| is_date(s))
        .max()
        .ok_or_else(|| {
            refusal(
                RefusalCode::AssayLookahead108,
                "workbook availability evidence is absent",
            )
        })?
        .to_string();
    if available_by.as_str() > as_of {
        return Err(refusal(
            RefusalCode::AssayLookahead108,
            format!("verified retrieval {available_by} cannot support earlier as-of {as_of}"),
        ));
    }
    let claims = fmine_public::key::mine_stats::load_claims(&profile, root)?;
    let transcripts = load_guidance_transcription_report(&profile,root,Some(as_of))?.ok_or_else(|| refusal(RefusalCode::NoClaimInDomain201,format!("registries/transcriptions/{issuer}.toml is absent; supply two independent readings of dated guidance")))?;
    if !transcripts.agreement.is_complete_for_prefix("guidance_") {
        return Err(refusal(
            RefusalCode::NotObservable202,
            "complete two-reader agreement before selecting the guidance population",
        ));
    }
    let mut report = assess(TrackRecordInput {
        issuer,
        site,
        through,
        as_of,
        available_by: &available_by,
        transcriptions: &transcripts,
        claims: &claims.claims,
        statuses: &profile.subject_statuses,
        synthetic: false,
    })?;
    report.source_refusals = claims.refusals;
    report.body = report_body(&report, &available_by);
    report.digest = blake3_hash(report.body.as_bytes());
    Ok(report)
}

fn rational(r: Rat) -> String {
    js(&format!("{}/{}", r.num(), r.den()))
}
fn optional(r: Option<Rat>) -> String {
    r.map_or_else(|| "null".into(), rational)
}
fn interval(i: Interval) -> String {
    format!(
        "{{\"lo\":{},\"hi\":{},\"lo_strict\":{},\"hi_strict\":{}}}",
        rational(i.lo),
        rational(i.hi),
        i.lo_strict,
        i.hi_strict
    )
}

fn claim_fields<T>(c: &Claim<T>) -> String {
    let anchors = c
        .anchors
        .iter()
        .map(|a| {
            format!(
                "{{\"at\":{},\"file\":{},\"sha256\":{}}}",
                js(&a.locator),
                js(&a.artifact_path),
                js(&Digest32(a.artifact_sha256).to_hex())
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let parents = c
        .parents
        .iter()
        .map(|id| js(&id.to_string()))
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "\"claim_id\":{},\"metric_key\":{},\"evidence\":{},\"knowledge\":{},\"parents\":[{}],\"sources\":[{}]",
        js(&c.claim_id.to_string()),
        js(&c.metric_key),
        js(&format!("{:?}", c.evidence)),
        js(&format!("{:?}", c.state)),
        parents,
        anchors
    )
}

fn score_json(c: &Claim<RangeScore>) -> String {
    let s = &c.value;
    format!(
        "{{{},\"rule\":\"guidance.range_score\",\"rules_version\":{},\"side\":{},\"hit\":{},\"boundary\":{},\"midpoint\":{},\"half_width\":{},\"z\":{},\"z_rounding_bounds\":{},\"above_midpoint\":{}}}",
        claim_fields(c),
        js(VERSION),
        js(&format!("{:?}", s.side)),
        js(&format!("{:?}", s.hit)),
        s.boundary,
        optional(s.midpoint),
        optional(s.half_width),
        optional(s.z),
        s.z_bounds.map_or_else(|| "null".into(), interval),
        s.above_midpoint
            .map_or_else(|| "null".into(), |v| v.to_string())
    )
}

fn outcome_json(a: &OutcomeAssessment) -> String {
    let source = a
        .source_claims
        .iter()
        .map(KeyedCell::to_json)
        .collect::<Vec<_>>()
        .join(",");
    let outcome = a.outcome.as_ref().map_or_else(
        || "null".into(),
        |c| {
            format!(
                "{{{},\"point\":{},\"rounding\":{}}}",
                claim_fields(c),
                rational(c.value.point),
                interval(c.value.rounding)
            )
        },
    );
    format!(
        "{{\"kind\":{},\"release\":{},\"reason\":{},\"outcome\":{},\"score\":{},\"source_claims\":[{}]}}",
        js(&a.kind),
        js(&a.release),
        a.reason.as_deref().map_or_else(|| "null".into(), js),
        outcome,
        a.score.as_ref().map_or_else(|| "null".into(), score_json),
        source
    )
}

fn summary_json(s: &TrackRecordSummary) -> String {
    let v = &s.summary.value;
    let year = match &v.scope {
        SummaryScope::Annual { year } => year.to_string(),
        SummaryScope::SiteHistory { .. } => "null".into(),
    };
    let cp = v.binomial_90.map_or_else(|| "null".into(),|(lo,hi)| format!("{{\"lo\":{lo},\"hi\":{hi},\"assumption\":\"independent Bernoulli outcomes; not established for sites sharing year shocks\"}}"));
    let certified = v.binomial_90_exact.map_or_else(|| "null".into(), interval);
    let fields = format!(
        "{},\"binomial_90_exact\":{certified}",
        claim_fields(&s.summary)
    );
    format!(
        "{{{},\"rule\":\"guidance.descriptive_summary\",\"rules_version\":{},\"metric\":{},\"year\":{},\"unit\":{},\"basis\":{},\"published_at\":{},\"selector\":{},\"members\":{},\"resolved\":{},\"unresolved\":{},\"printed_hits\":{},\"certain_hits\":{},\"certain_misses\":{},\"boundary\":{},\"below\":{},\"above\":{},\"observed_hit_rate\":{},\"whole_cohort_hit_rate_bounds\":{},\"rounding_aware_hit_rate_bounds\":{},\"binomial_90\":{},\"z_members\":{},\"mean_z\":{},\"mean_z_rounding\":{},\"above_midpoint\":{},\"share_above_midpoint\":{},\"effective_sample_size\":null,\"bias_confidence_interval\":null,\"flag\":{}}}",
        fields,
        js(VERSION),
        js(&v.metric),
        year,
        js(&v.unit),
        js(&s.basis),
        js(&s.published_at),
        js(&s.selector),
        v.members,
        v.resolved,
        v.unresolved,
        v.printed_hits,
        v.certain_hits,
        v.certain_misses,
        v.boundary,
        v.below,
        v.above,
        optional(v.observed_hit_rate),
        interval(v.whole_cohort_hit_rate_bounds),
        interval(v.rounding_aware_hit_rate_bounds),
        cp,
        v.z_members,
        optional(v.mean_z),
        v.mean_z_rounding.map_or_else(|| "null".into(), interval),
        v.above_midpoint,
        optional(v.share_above_midpoint),
        js(&format!("{:?}", v.flag).to_ascii_lowercase())
    )
}

impl TrackRecordRow {
    /// Canonical source-bound row shared by retrospective and dated reports.
    #[must_use]
    pub fn to_json(&self) -> String {
        let a = &self.guidance_source;
        let later = self
            .later
            .iter()
            .map(outcome_json)
            .collect::<Vec<_>>()
            .join(",");
        format!(
            "{{\"site\":{},\"year\":{},\"metric\":{},\"unit\":{},\"basis\":{},\"published_at\":{},\"published_at_basis\":{},\"vintage_kind\":{},\"guidance\":{{{},\"token\":{},\"at\":{},\"sha256\":{},\"reader_count\":{},\"source_claim\":{{{},\"statement_id\":{}}}}},\"original\":{},\"later\":[{}],\"price_conditional\":{{\"state\":\"unsplit\",\"reason\":\"No verified price-assumption and site royalty-proportionality gate supplied\"}}}}",
            js(&self.site),
            self.year,
            js(&self.metric),
            js(&self.unit),
            js(&self.basis),
            js(&self.published_at),
            js(&self.published_at_basis),
            js(&self.vintage_kind),
            claim_fields(&self.guidance),
            js(&a.token),
            js(&a.locator.to_string()),
            js(&a.doc_sha256),
            a.readers.len(),
            claim_fields(&self.guidance_source_claim),
            js(self.guidance_source_claim.value_tag()),
            outcome_json(&self.original),
            later
        )
    }
}

fn report_body(r: &TrackRecordReport, available_by: &str) -> String {
    let source_refusals = r
        .source_refusals
        .iter()
        .map(|v| {
            format!(
                "{{\"locator\":{},\"reason\":{}}}",
                js(&v.locator),
                js(&v.reason)
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let version_and_sources = format!(
        "{},\"source_claims_digest\":{},\"source_refusals\":[{source_refusals}]",
        js(VERSION),
        js(&r.source_claims_digest.to_hex())
    );
    let rows = r
        .rows
        .iter()
        .map(TrackRecordRow::to_json)
        .collect::<Vec<_>>()
        .join(",");
    let summaries = r
        .summaries
        .iter()
        .map(summary_json)
        .collect::<Vec<_>>()
        .join(",");
    let pending = r
        .pending_transcriptions
        .iter()
        .map(|s| js(s))
        .collect::<Vec<_>>()
        .join(",");
    let unsupported = r.unsupported_guidance.iter().map(|a|format!("{{\"at\":{},\"sha256\":{},\"quantity\":{},\"token\":{},\"reason\":\"unsupported_metric\"}}",js(&a.locator.to_string()),js(&a.doc_sha256),js(&a.quantity),js(&a.token))).collect::<Vec<_>>().join(",");
    format!(
        "\"schema\":\"fmine.guidance.track_record.v1\",\"rules_version\":{},\"issuer\":{},\"as_of\":{},\"through\":{},\"lane\":{},\"gate_eligible\":false,\"available_by\":{},\"availability_basis\":\"verified workbook retrieval; guidance before annual close; workbook publication dates unregistered\",\"data_gate\":{{\"fired\":{},\"resolved_years\":{},\"minimum_sites_per_year\":{},\"delivery_scope\":{}}},\"rows\":[{}],\"summaries\":[{}],\"pending_transcriptions\":[{}],\"unsupported_guidance\":[{}],\"warnings\":[\"Every registered guidance vintage is retained; summary groups never pool revisions.\",\"Later comparative scores do not fill missing original outcomes or enter the first-published summary.\",\"Effective sample size and bias confidence intervals are not estimated; shared shocks may invalidate binomial intervals.\",\"This report does not freeze an authoritative cohort or externally timestamp any forecast.\"],\"limitations\":{}",
        version_and_sources,
        js(&r.issuer),
        js(&r.as_of),
        r.through.as_deref().map_or_else(|| "null".into(), js),
        js(if r.synthetic { "synthetic" } else { "public" }),
        js(available_by),
        r.gate_fired,
        r.resolved_years,
        r.minimum_sites_per_year,
        js(if r.gate_fired {
            "limited descriptive cohorts; portfolio evidence gate not met"
        } else {
            "descriptive site cohorts"
        }),
        rows,
        summaries,
        pending,
        unsupported,
        js(NO_CLAIM)
    )
}

/// Invented guidance and outcome Claims through the same projection; no synthetic input is gate evidence.
pub fn synthetic() -> Result<TrackRecordReport> {
    synthetic_as_of("2032-02-29")
}

/// The invented fixture under an explicit information cut; future observations remain absent.
pub fn synthetic_as_of(as_of: &str) -> Result<TrackRecordReport> {
    let (transcripts, cells) = synthetic_evidence()?;
    assess(TrackRecordInput {
        issuer: "synthetic",
        site: None,
        through: None,
        as_of,
        available_by: "2030-01-15",
        transcriptions: &transcripts,
        claims: &cells,
        statuses: &[],
        synthetic: true,
    })
}

/// Reusable invented evidence for public API integration tests. No public issuer figures are embedded.
pub fn synthetic_evidence() -> Result<(TranscriptionReport, Vec<KeyedCell>)> {
    use fmine_public::key::{LineKey, PublicKey, ShareSource, Vintage};
    use fmine_public::transcribe::CheckedDocument;
    use fmine_registry::transcriptions::{Agreement, Locator};
    let document = "synthetic-guidance.txt";
    let hash = sha256(b"Invented guidance: three mines, each 100 - 120 koz, attributable 4/5");
    let mut accepted = Vec::new();
    for site in ["Example Mine", "Boundary Mine", "Missing Mine"] {
        accepted.push(Accepted {
            locator: Locator {
                doc: document.into(),
                page: 1,
                table: "Invented guidance".into(),
                row: site.into(),
                col: "Production".into(),
            },
            doc_sha256: hash.to_hex(),
            token: "100 - 120".into(),
            value: parse_token("100 - 120")
                .map_err(|e| refusal(RefusalCode::SourceCellUnusable208, e))?,
            quantity: format!("guidance_{}_production_koz", slug(site).replace('-', "_")),
            unit: "koz".into(),
            basis: "attributable:4/5".into(),
            period: "2030FY".into(),
            readers: vec!["synthetic-reader-a".into(), "synthetic-reader-b".into()],
        });
    }
    let transcripts = TranscriptionReport {
        registry: "synthetic-only".into(),
        documents: vec![CheckedDocument {
            file: document.into(),
            kind: "synthetic".into(),
            published_at: "2030-01-15".into(),
            published_at_basis: "invented".into(),
            sha256: hash.to_hex(),
        }],
        entries: 6,
        agreement: Agreement {
            accepted,
            ..Agreement::default()
        },
        tie: None,
    };
    let mut cells = Vec::new();
    for (site, release, token) in [
        ("Example Mine", "2030Q4", "125"),
        ("Example Mine", "2031Q4", "115"),
        ("Boundary Mine", "2030Q4", "120"),
        ("Missing Mine", "2031Q4", "110"),
    ] {
        let hash = sha256(format!("invented {site}|{release}|{token}").as_bytes());
        let key = PublicKey {
            issuer: "synthetic".into(),
            subject: site.into(),
            valid_period: fmine_public::key::Period {
                year: 2030,
                kind: PeriodKind::FullYear,
            },
            reporting_vintage: Vintage {
                release: release.into(),
                file_sha256: hash.to_hex(),
            },
            ownership_basis: OwnershipBasis::Attributable {
                num: 4,
                den: 5,
                source: ShareSource::Printed,
            },
            scope: Scope::Site,
            unit: "koz".into(),
            line: LineKey {
                label: "summary:au_prod_attr_koz".into(),
                occurrence: 0,
            },
        };
        let path = format!("synthetic/{release}/{}/production.txt", slug(site));
        let value = PublicValue::Number {
            stored: token.into(),
            displayed: Ok(token.into()),
            under_status: None,
        };
        let tag = fmine_public::key::canonical_value_tag(&key, &value);
        let claim = Claim::new(
            "summary:au_prod_attr_koz",
            value,
            &tag,
            Representation::Deterministic,
            SpatialSupport::PeriodAggregate,
            AdditivityClass::ExtensiveAdditive,
            EvidenceClass::Assumed,
            EpistemicStatus::KnownUnknown(KuKind::Parametric),
            KnowledgeState::Estimated,
            Confidentiality::public(),
            vec![],
            vec![],
            vec![SourceAnchor::new(&path, hash.0, "invented annual outcome")],
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
    Ok((transcripts, cells))
}
