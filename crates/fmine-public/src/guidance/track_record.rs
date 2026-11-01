//! DESIGN (BD-341): descriptive guidance assessments over an explicit registered evidence cut.
//! Every agreed guidance vintage remains visible. Only the earliest registered vintage enters the default
//! denominator; ties stay unresolved. An absent original annual release is never filled by a comparative.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::Path;

use fmine_calib::guidance_scores::{
    self, Guidance, GuidanceScore, GuidanceSummary, PrintedOutcome, ScoringMember, Side,
};
use fmine_claim::{AdditivityClass, Representation, SpatialSupport};
use fmine_core::hash::{Digest32, blake3_hash};
use fmine_evidence::{KnowledgeState, Refusal, RefusalCode};
use fmine_metrics::guidance::Ownership;
use fmine_numerics::exact_feas::{Interval, Rat};
use fmine_registry::sources::is_date;
use fmine_registry::transcriptions::{Accepted, Token, parse_token};

use crate::key::{self, KeyedCell, PeriodKind, PublicValue, Scope};
use crate::sources::{IssuerProfile, SourceKind, load_issuer_profile};
use crate::transcribe::{CheckedDocument, TranscriptionReport, load_transcription_prefixes};
use crate::{js, mine_ledger::rat_text};

pub const NO_CLAIM: &str = "Descriptive comparison of registered issuer guidance and reported outcomes; no forecast, calibration verdict, site ranking or proof of historical forecast registration. From public filings; no affiliation implied.";
const METRICS: [&str; 6] = [
    "production_koz",
    "production_kt",
    "aisc_usd_per_oz",
    "aisc_usd_per_lb",
    "tcc_usd_per_oz",
    "c1_usd_per_lb",
];
type Result<T> = std::result::Result<T, Refusal>;

#[derive(Debug, Clone)]
pub struct OutcomeAssessment {
    pub status: &'static str,
    /// Includes unusable and ambiguous candidates, so a missing score never hides its source evidence.
    pub sources: Vec<KeyedCell>,
    pub score: Option<GuidanceScore>,
    pub printed: Option<PrintedOutcome>,
    pub refusal: Option<Refusal>,
}

#[derive(Debug, Clone)]
pub struct TrackRecordRow {
    pub site: String,
    pub year: u16,
    pub metric: String,
    pub guidance: Accepted,
    pub document: CheckedDocument,
    pub earliest_registered: bool,
    pub ambiguous_earliest: bool,
    pub first_published: OutcomeAssessment,
    pub latest_registered: OutcomeAssessment,
}

#[derive(Debug, Clone)]
pub struct TrackRecordSummary {
    pub year: u16,
    pub metric: String,
    pub basis: String,
    pub selector: &'static str,
    pub members: Vec<String>,
    pub scores: GuidanceSummary,
}

#[derive(Debug, Clone)]
pub struct TrackRecordReport {
    pub issuer: String,
    pub synthetic: bool,
    pub as_of: String,
    pub workbooks_available_by: String,
    pub rows: Vec<TrackRecordRow>,
    pub summaries: Vec<TrackRecordSummary>,
    /// Unknown metric keys remain explicitly listed; they are outside this version's score population.
    pub unsupported: Vec<Accepted>,
    pub digest: Digest32,
}

fn refuse(code: RefusalCode, detail: impl Into<String>) -> Refusal {
    Refusal::new(code, format!("guidance track record: {}", detail.into()))
}

fn source_error(detail: impl Into<String>) -> Refusal {
    refuse(RefusalCode::SourceCellUnusable208, detail)
}

fn metric_key(quantity: &str) -> Option<(String, &'static str)> {
    let tail = quantity.strip_prefix("guidance_")?;
    METRICS.into_iter().find_map(|metric| {
        tail.strip_suffix(&format!("_{metric}"))
            .filter(|site| !site.is_empty())
            .map(|site| (site.replace('_', "-"), metric))
    })
}

fn unit(metric: &str) -> &'static str {
    if metric == "production_koz" {
        "koz"
    } else if metric == "production_kt" {
        "kt"
    } else if metric.ends_with("_per_lb") {
        "usd_per_lb"
    } else {
        "usd_per_oz"
    }
}

fn basis(row: &Accepted, metric: &str) -> Result<Ownership> {
    if row.unit != unit(metric) {
        return Err(refuse(
            RefusalCode::BasisMismatch105,
            format!(
                "{}: guidance unit {} differs from {}",
                row.locator,
                row.unit,
                unit(metric)
            ),
        ));
    }
    let ownership = if metric.starts_with("production_") {
        row.basis.as_str()
    } else {
        let family = metric.split('_').next().unwrap_or("");
        let denominator = if metric.ends_with("_per_lb") {
            "lb"
        } else {
            "oz"
        };
        row.basis.strip_suffix(&format!(";{family}=issuer-reported-per-sold-{denominator}"))
            .ok_or_else(|| refuse(RefusalCode::UndeclaredUnitBasisOrTz107,
                format!("{}: cost guidance must explicitly name the issuer-reported metric per sold {denominator}", row.locator)))?
    };
    super::ownership(ownership)
}

fn normalized_basis(row: &Accepted, metric: &str) -> String {
    match basis(row, metric) {
        Ok(Ownership::Full) => "100%".into(),
        Ok(Ownership::Attributable { share }) => {
            format!("attributable:{}/{}", share.num(), share.den())
        }
        Err(_) => row.basis.clone(),
    }
}

fn line(metric: &str, ownership: Ownership) -> String {
    if metric.starts_with("production_") {
        let metal = if metric == "production_koz" {
            "au"
        } else {
            "cu"
        };
        let share = if ownership == Ownership::Full {
            "100"
        } else {
            "attr"
        };
        format!("summary:{metal}_prod_{share}_{}", unit(metric))
    } else {
        format!("summary:{metric}")
    }
}

fn checked_document<'a>(
    profile: &IssuerProfile,
    report: &'a TranscriptionReport,
    row: &Accepted,
) -> Result<&'a CheckedDocument> {
    let documents: Vec<_> = report
        .documents
        .iter()
        .filter(|d| d.file == row.locator.doc)
        .collect();
    let [document] = documents.as_slice() else {
        return Err(source_error(format!(
            "{} needs one checked document",
            row.locator
        )));
    };
    let registered: Vec<_> = profile
        .documents
        .iter()
        .filter(|d| d.file == row.locator.doc)
        .collect();
    let [registered] = registered.as_slice() else {
        return Err(source_error(format!(
            "{} needs one registered document",
            row.locator
        )));
    };
    if row.doc_sha256 != document.sha256
        || document.sha256 != registered.sha256
        || document.published_at != registered.published_at
        || document.kind != registered.kind
        || document.published_at_basis != registered.published_at_basis
        || !is_date(&document.published_at)
        || row.readers.iter().collect::<BTreeSet<_>>().len() < 2
        || parse_token(&row.token).map_err(source_error)? != row.value
    {
        return Err(source_error(format!(
            "{}: source digest, date, token or two-reader agreement is inconsistent",
            row.locator
        )));
    }
    super::sha_bytes(&row.doc_sha256)?;
    Ok(document)
}

fn stated_guidance(row: &Accepted) -> Result<Guidance> {
    let range = super::token_interval(row.value, false)?;
    Ok(if range.lo == range.hi {
        Guidance::Point { value: range.lo }
    } else {
        Guidance::Range {
            lo: range.lo,
            hi: range.hi,
        }
    })
}

fn score_source(
    profile: &IssuerProfile,
    row: &Accepted,
    metric: &str,
    source: &KeyedCell,
) -> Result<(GuidanceScore, PrintedOutcome)> {
    source.claim.check_id()?;
    if source.claim.value_tag() != key::canonical_value_tag(&source.key, &source.claim.value) {
        return Err(refuse(
            RefusalCode::FrozenRecordMutation507,
            "source key, token or typed interpretation differs from its Claim",
        ));
    }
    if !source.claim.confidentiality.is_public() {
        return Err(refuse(RefusalCode::MnpiWall301, "non-public outcome"));
    }
    if source.claim.stale || source.claim.state == KnowledgeState::Stale {
        return Err(refuse(RefusalCode::StaleAnchor502, "stale outcome"));
    }
    if source.claim.calibration_failed {
        return Err(refuse(
            RefusalCode::CalibrationRequired205,
            "failed-calibration outcome",
        ));
    }
    if !matches!(
        source.claim.state,
        KnowledgeState::Known | KnowledgeState::Estimated
    ) || matches!(
        source.claim.representation,
        Representation::Placeholder { .. }
    ) {
        return Err(refuse(
            RefusalCode::NotObservable202,
            "outcome is unknown or a placeholder",
        ));
    }
    let expected_additivity = if metric.starts_with("production_") {
        AdditivityClass::ExtensiveAdditive
    } else {
        AdditivityClass::IntensiveWeighted
    };
    if source.claim.support != SpatialSupport::PeriodAggregate
        || source.claim.additivity != expected_additivity
    {
        return Err(refuse(
            RefusalCode::SupportMismatch102,
            "outcome support or additivity differs from guidance",
        ));
    }
    if source.key.issuer != profile.issuer
        || source.key.unit != row.unit
        || super::cell_ownership(source.key.ownership_basis)? != basis(row, metric)?
        || source.key.line.occurrence != 0
    {
        return Err(refuse(
            RefusalCode::BasisMismatch105,
            "outcome issuer, ownership, unit or row occurrence differs from guidance",
        ));
    }
    let digest = super::sha_bytes(&source.key.reporting_vintage.file_sha256)?;
    if !source.claim.anchors.iter().any(|a| {
        a.artifact_sha256 == digest
            && Path::new(&a.artifact_path)
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|file| source.locator == format!("{file}:{}", a.locator))
    }) || !profile.sources.iter().any(|s| {
        s.kind == SourceKind::MineStats
            && s.quarter == source.key.reporting_vintage.release
            && s.sha256_hex == source.key.reporting_vintage.file_sha256
            && source.locator.starts_with(&format!("{}:", s.file_name))
    }) {
        return Err(refuse(
            RefusalCode::DigestMismatch505,
            "outcome lacks its registered source binding",
        ));
    }
    let PublicValue::Number {
        displayed: Ok(text),
        under_status: None,
        ..
    } = &source.claim.value
    else {
        return Err(source_error(
            "outcome is missing, unreadable, withheld or disposed",
        ));
    };
    let Token::Point(point) = parse_token(text).map_err(source_error)? else {
        return Err(source_error("outcome needs one displayed number"));
    };
    let printed = PrintedOutcome {
        value: super::decimal(point)?,
        interval: super::token_interval(Token::Point(point), true)?,
    };
    if metric.starts_with("production_")
        && (super::token_interval(row.value, false)?.lo.num() < 0 || printed.value.num() < 0)
    {
        return Err(refuse(
            RefusalCode::OutsidePhysicalDomain109,
            "production guidance and observed production must be nonnegative",
        ));
    }
    let score = guidance_scores::score(stated_guidance(row)?, printed)
        .map_err(|e| refuse(RefusalCode::NumericalCertificateFailed207, e.to_string()))?;
    Ok((score, printed))
}

#[allow(
    clippy::too_many_arguments,
    reason = "source selector is one site/metric/year/vintage request"
)]
fn outcome(
    profile: &IssuerProfile,
    row: &Accepted,
    site: &str,
    metric: &str,
    year: u16,
    cells: &[KeyedCell],
    latest: bool,
    as_of: &str,
) -> OutcomeAssessment {
    let failure = |status, sources, error| OutcomeAssessment {
        status,
        sources,
        score: None,
        printed: None,
        refusal: Some(error),
    };
    let ownership = match basis(row, metric) {
        Ok(basis) => basis,
        Err(error) => return failure("basis_unresolved", Vec::new(), error),
    };
    let expected = line(metric, ownership);
    let original_release = format!("{year:04}Q4");
    let candidates: Vec<_> = cells
        .iter()
        .filter(|c| {
            super::site_key(&c.key.subject) == site
                && matches!(c.key.scope, Scope::Site)
                && c.key.valid_period.year == year
                && c.key.valid_period.kind == PeriodKind::FullYear
                && c.key.line.label == expected
                && c.key.reporting_vintage.release >= original_release
                && release_end(&c.key.reporting_vintage.release)
                    .is_some_and(|end| end.as_str() <= as_of)
        })
        .collect();
    let selected = if latest {
        candidates
            .iter()
            .map(|c| c.key.reporting_vintage.release.as_str())
            .max()
    } else {
        Some(original_release.as_str())
    };
    let mut sources: Vec<_> = candidates
        .iter()
        .filter(|c| Some(c.key.reporting_vintage.release.as_str()) == selected)
        .map(|c| (*c).clone())
        .collect();
    sources.sort_by(|a, b| (&a.key, &a.locator).cmp(&(&b.key, &b.locator)));
    let [source] = sources.as_slice() else {
        let status = if sources.is_empty() {
            "not_published_or_not_registered"
        } else {
            "ambiguous_outcome"
        };
        let code = if sources.is_empty() {
            RefusalCode::NoClaimInDomain201
        } else {
            RefusalCode::NonIdentifiableAttribution104
        };
        return failure(
            status,
            sources,
            refuse(
                code,
                format!(
                    "{site} {year}FY {metric}: expected one {} outcome",
                    if latest {
                        "latest registered"
                    } else {
                        "original-release"
                    }
                ),
            ),
        );
    };
    let status = if source.key.reporting_vintage.release == original_release {
        "original_annual_release"
    } else if candidates
        .iter()
        .any(|c| c.key.reporting_vintage.release == original_release)
    {
        "later_comparative"
    } else {
        "comparative_only_original_missing"
    };
    match score_source(profile, row, metric, source) {
        Ok((score, printed)) => OutcomeAssessment {
            status,
            sources,
            score: Some(score),
            printed: Some(printed),
            refusal: None,
        },
        Err(error) => failure("unusable_outcome", sources, error),
    }
}

fn release_end(release: &str) -> Option<String> {
    if !fmine_registry::sources::is_quarter(release) {
        return None;
    }
    let end = match release.as_bytes()[5] {
        b'1' => "03-31",
        b'2' => "06-30",
        b'3' => "09-30",
        _ => "12-31",
    };
    Some(format!("{}-{end}", &release[..4]))
}

fn target_status(
    profile: &IssuerProfile,
    site: &str,
    year: u16,
) -> Option<(&'static str, Refusal)> {
    profile.subject_statuses.iter().find_map(|s| {
        if super::site_key(&s.subject) != site { return None; }
        let (start, status, code) = match s.kind {
            // from_date records the first FULL month after disposal; invert it for annual target support.
            key::StatusKind::Disposed => {
                let closed_month = if s.first.1 == 1 {
                    (s.first.0.checked_sub(1)?, 12)
                } else { (s.first.0, s.first.1 - 1) };
                (closed_month, "definition_changed", RefusalCode::BasisMismatch105)
            }
            key::StatusKind::Withheld => (s.first, "withheld_target", RefusalCode::NotObservable202),
            // Maintenance alone does not truncate the target: an observed annual zero remains an outcome.
            key::StatusKind::CareAndMaintenance => return None,
        };
        if start > (year, 12) || s.last.is_some_and(|last| last < (year, 1)) { return None; }
        Some((status, refuse(code, format!(
            "{site} {year}FY overlaps registered {:?} status from {:04}-{:02}; annual target support is unresolved, including unflagged printed stub values; evidence: {}",
            s.kind, start.0, start.1, s.evidence,
        ))))
    })
}

/// Assesses already authenticated source projections. `run` performs the byte checks; this adapter also
/// verifies each selected row's registry/Claim binding. It never registers a retrospective forecast.
pub fn assess(
    profile: &IssuerProfile,
    transcripts: &TranscriptionReport,
    cells: &[KeyedCell],
    site: Option<&str>,
    as_of: &str,
) -> Result<TrackRecordReport> {
    if !is_date(as_of) {
        return Err(refuse(
            RefusalCode::UndeclaredUnitBasisOrTz107,
            "as-of must be YYYY-MM-DD",
        ));
    }
    let availability = super::available_by(profile, as_of)?;
    if !transcripts.agreement.is_complete_for_prefix("guidance_") {
        return Err(refuse(
            RefusalCode::NotObservable202,
            "complete two-reader agreement before assessing the guidance population",
        ));
    }
    let requested = site.map(super::site_key);
    let mut rows = Vec::new();
    let mut unsupported = Vec::new();
    for a in &transcripts.agreement.accepted {
        if !a.quantity.starts_with("guidance_") {
            continue;
        }
        if profile
            .documents
            .iter()
            .any(|d| d.file == a.locator.doc && d.published_at.as_str() > as_of)
        {
            continue;
        }
        let document = checked_document(profile, transcripts, a)?;
        if document.published_at.as_str() > as_of {
            continue;
        }
        let Some((site, metric)) = metric_key(&a.quantity) else {
            unsupported.push(a.clone());
            continue;
        };
        if requested.as_ref().is_some_and(|s| s != &site) {
            continue;
        }
        let period = key::parse_period_code(&a.period)
            .filter(|p| p.kind == PeriodKind::FullYear && p.year != 0)
            .ok_or_else(|| {
                refuse(
                    RefusalCode::SupportMismatch102,
                    format!("{}: guidance needs a full calendar year", a.locator),
                )
            })?;
        let mut first = outcome(profile, a, &site, metric, period.year, cells, false, as_of);
        let mut latest = outcome(profile, a, &site, metric, period.year, cells, true, as_of);
        if let Some((status, error)) = target_status(profile, &site, period.year) {
            for outcome in [&mut first, &mut latest] {
                outcome.score = None;
                outcome.printed = None;
                outcome.status = status;
                outcome.refusal = Some(error.clone());
            }
        }
        if document.published_at > format!("{:04}-12-31", period.year) {
            for outcome in [&mut first, &mut latest] {
                outcome.score = None;
                outcome.printed = None;
                outcome.status = "guidance_timing_unproven";
                outcome.refusal = Some(refuse(
                    RefusalCode::AssayLookahead108,
                    "registered exact-byte guidance availability is after the target year; no historical score is asserted",
                ));
            }
        }
        rows.push(TrackRecordRow {
            site,
            year: period.year,
            metric: metric.into(),
            guidance: a.clone(),
            document: document.clone(),
            earliest_registered: false,
            ambiguous_earliest: false,
            first_published: first,
            latest_registered: latest,
        });
    }
    if rows.is_empty() {
        return Err(refuse(
            RefusalCode::NoClaimInDomain201,
            "no supported, agreed guidance rows at the requested cut and site",
        ));
    }
    rows.sort_by(|a, b| {
        (
            &a.site,
            a.year,
            &a.metric,
            &a.guidance.basis,
            &a.document.published_at,
            &a.guidance.locator,
        )
            .cmp(&(
                &b.site,
                b.year,
                &b.metric,
                &b.guidance.basis,
                &b.document.published_at,
                &b.guidance.locator,
            ))
    });
    unsupported.sort_by(|a, b| a.locator.cmp(&b.locator));
    let mut families = BTreeMap::<(String, u16, String, String), Vec<usize>>::new();
    for (i, r) in rows.iter().enumerate() {
        families
            .entry((
                r.site.clone(),
                r.year,
                r.metric.clone(),
                normalized_basis(&r.guidance, &r.metric),
            ))
            .or_default()
            .push(i);
    }
    let mut groups = BTreeMap::<(u16, String, String), Vec<(ScoringMember, ScoringMember)>>::new();
    for ((site, year, metric, basis), indexes) in families {
        let date = indexes
            .iter()
            .map(|i| rows[*i].document.published_at.as_str())
            .min()
            .unwrap_or("")
            .to_string();
        let initial: Vec<_> = indexes
            .iter()
            .copied()
            .filter(|i| rows[*i].document.published_at == date)
            .collect();
        for i in &initial {
            rows[*i].earliest_registered = true;
            rows[*i].ambiguous_earliest = initial.len() != 1;
        }
        let (first, latest) = if let [i] = initial.as_slice() {
            (
                rows[*i].first_published.printed,
                rows[*i].latest_registered.printed,
            )
        } else {
            (None, None)
        };
        let guidance = stated_guidance(&rows[initial[0]].guidance)?;
        groups.entry((year, metric, basis)).or_default().push((
            ScoringMember {
                id: site.clone(),
                guidance,
                outcome: first,
            },
            ScoringMember {
                id: site,
                guidance,
                outcome: latest,
            },
        ));
    }
    let mut summaries = Vec::new();
    for ((year, metric, basis), members) in groups {
        for latest in [false, true] {
            let scores: Vec<_> = members
                .iter()
                .map(|(first, last)| if latest { last.clone() } else { first.clone() })
                .collect();
            summaries.push(TrackRecordSummary {
                year,
                metric: metric.clone(),
                basis: basis.clone(),
                selector: if latest {
                    "latest_registered"
                } else {
                    "first_published"
                },
                members: members.iter().map(|(first, _)| first.id.clone()).collect(),
                scores: guidance_scores::summarize(&scores).map_err(|e| {
                    refuse(RefusalCode::NumericalCertificateFailed207, e.to_string())
                })?,
            });
        }
    }
    let mut report = TrackRecordReport {
        issuer: profile.issuer.clone(),
        synthetic: profile.issuer == crate::synthetic::SYNTHETIC_ISSUER,
        as_of: as_of.into(),
        workbooks_available_by: availability,
        rows,
        summaries,
        unsupported,
        digest: blake3_hash(&[]),
    };
    report.digest = blake3_hash(report.body().as_bytes());
    Ok(report)
}

/// Loads and verifies the registered documents and workbooks at an explicit evidence cut.
pub fn run(
    workspace: &Path,
    issuer: &str,
    site: Option<&str>,
    as_of: &str,
) -> Result<TrackRecordReport> {
    let profile = load_issuer_profile(workspace, issuer)?;
    super::available_by(&profile, as_of)?;
    let cells = key::mine_stats::load_claims(&profile, workspace)?;
    let transcripts = load_transcription_prefixes(&profile, workspace, &["guidance_"], Some(as_of))?.ok_or_else(||
        refuse(RefusalCode::NoClaimInDomain201, format!("registries/transcriptions/{issuer}.toml is absent; register two independent readings of the guidance")))?;
    assess(&profile, &transcripts, &cells.claims, site, as_of)
}

fn rat(value: Rat) -> String {
    js(&rat_text(value))
}
fn optional_rat(value: Option<Rat>) -> String {
    value.map_or_else(|| "null".into(), rat)
}
fn interval(value: Option<Interval>) -> String {
    value.map_or_else(
        || "null".into(),
        |i| {
            format!(
                "{{\"lo\":{},\"hi\":{},\"lo_strict\":{},\"hi_strict\":{}}}",
                rat(i.lo),
                rat(i.hi),
                i.lo_strict,
                i.hi_strict,
            )
        },
    )
}
fn side(value: Side) -> &'static str {
    match value {
        Side::Below => "below",
        Side::Inside => "inside",
        Side::Above => "above",
    }
}
fn accepted_json(a: &Accepted, d: &CheckedDocument) -> String {
    format!(
        "{{\"at\":{},\"sha256\":{},\"token\":{},\"quantity\":{},\"unit\":{},\"basis\":{},\"period\":{},\"published_at\":{},\"published_at_basis\":{},\"readers\":[{}]}}",
        js(&a.locator.to_string()),
        js(&a.doc_sha256),
        js(&a.token),
        js(&a.quantity),
        js(&a.unit),
        js(&a.basis),
        js(&a.period),
        js(&d.published_at),
        js(&d.published_at_basis),
        a.readers
            .iter()
            .map(|r| js(r))
            .collect::<Vec<_>>()
            .join(",")
    )
}
fn outcome_json(o: &OutcomeAssessment, a: &Accepted) -> String {
    let score = o.score.map_or_else(|| "null".into(), |s| format!(
        "{{\"rule\":\"guidance.stated-set.exact/1\",\"inputs\":[{},{}],\"midpoint\":{},\"half_width\":{},\"z\":{},\"z_rounding\":{},\"side\":{},\"boundary\":{},\"definitely_inside\":{},\"possibly_inside\":{},\"above_midpoint\":{}}}",
        js(&a.locator.to_string()),js(&o.sources[0].locator),optional_rat(s.midpoint),optional_rat(s.half_width),optional_rat(s.z),
        interval(s.z_interval),js(side(s.side)),s.boundary,s.definitely_inside,s.possibly_inside,s.above_midpoint.map_or_else(|| "null".into(), |b| b.to_string())));
    let error = o.refusal.as_ref().map_or_else(
        || "null".into(),
        |e| {
            format!(
                "{{\"code\":{},\"what\":{},\"fix\":{}}}",
                js(e.code),
                js(&e.what),
                js(e.fix)
            )
        },
    );
    format!(
        "{{\"status\":{},\"score\":{score},\"refusal\":{error},\"sources\":[{}]}}",
        js(o.status),
        o.sources
            .iter()
            .map(KeyedCell::to_json)
            .collect::<Vec<_>>()
            .join(",")
    )
}

impl TrackRecordReport {
    fn body(&self) -> String {
        let rows = self.rows.iter().map(|r| format!("{{\"site\":{},\"year\":{},\"metric\":{},\"earliest_registered\":{},\"ambiguous_earliest\":{},\"guidance\":{},\"first_published\":{},\"latest_registered\":{}}}",
            js(&r.site),r.year,js(&r.metric),r.earliest_registered,r.ambiguous_earliest,accepted_json(&r.guidance,&r.document),outcome_json(&r.first_published,&r.guidance),outcome_json(&r.latest_registered,&r.guidance))).collect::<Vec<_>>().join(",");
        let summaries = self.summaries.iter().map(|g| { let s=&g.scores; format!("{{\"year\":{},\"metric\":{},\"basis\":{},\"selector\":{},\"members\":[{}],\"rule\":\"guidance.descriptive.summary/1\",\"n_members\":{},\"n_scored\":{},\"n_unresolved\":{},\"n_inside\":{},\"n_below\":{},\"n_above\":{},\"n_boundary\":{},\"n_z\":{},\"hit_rate\":{},\"whole_cohort_bounds\":{},\"rounding_aware_hit_bounds\":{},\"cp90_iid\":{},\"mean_z\":{},\"mean_z_rounding\":{},\"share_above_midpoint\":{},\"flag\":{}}}",
            g.year,js(&g.metric),js(&g.basis),js(g.selector),g.members.iter().map(|s|js(s)).collect::<Vec<_>>().join(","),s.n_members,s.n_resolved,s.n_unresolved,s.inside,s.below,s.above,s.boundary,s.n_ranges,
            optional_rat(s.observed_hit_rate),interval(s.whole_cohort_hit_bounds),interval(s.rounding_aware_hit_bounds),interval(s.iid_reference_interval_90_exact),optional_rat(s.mean_z),interval(s.mean_z_rounding_interval),optional_rat(s.above_midpoint_share),js(if s.n_resolved==0{"unscored"}else{"descriptive"})) }).collect::<Vec<_>>().join(",");
        let unsupported=self.unsupported.iter().map(|a|format!("{{\"at\":{},\"sha256\":{},\"quantity\":{},\"reason\":\"unsupported guidance metric\"}}",js(&a.locator.to_string()),js(&a.doc_sha256),js(&a.quantity))).collect::<Vec<_>>().join(",");
        format!(
            "\"schema\":\"fmine.guidance.track-record.as-of.v1\",\"issuer\":{},\"lane\":{},\"gate_eligible\":false,\"as_of\":{},\"workbooks_available_by\":{},\"population\":\"supported agreed guidance rows in the supplied registry; historical completeness unassessed\",\"guidance_selector\":\"earliest registered document per site, fiscal year, metric and basis; later vintages shown separately\",\"time_proof\":\"not supplied; source dates and conservative exact-byte availability are reported, no retrospective forecast registration\",\"rows\":[{rows}],\"summaries\":[{summaries}],\"unsupported_guidance\":[{unsupported}],\"warnings\":[\"Clopper-Pearson bounds are an IID Bernoulli reference for resolved printed-value hits only; they do not account for selective missingness or outcome rounding. Cross-site and year dependence is unmodelled. Effective sample size is not estimated.\",\"Mean-z bounds propagate printed rounding only, not sampling uncertainty or a confidence interval for bias.\",\"Missing original annual releases remain unresolved in the first-published denominator; later comparatives are separate assessments.\",\"Cost scores are unconditional: gold-price adjustments are not implemented, and a miss does not identify controllable operating causes.\"],\"no_claim\":{}",
            js(&self.issuer),
            js(if self.synthetic {
                "synthetic"
            } else {
                "public"
            }),
            js(&self.as_of),
            js(&self.workbooks_available_by),
            js(NO_CLAIM)
        )
    }
    #[must_use]
    pub fn to_json(&self) -> String {
        format!(
            "{{\"digest\":{},{}}}",
            js(&self.digest.to_hex()),
            self.body()
        )
    }
    #[must_use]
    pub fn to_markdown(&self) -> String {
        let clean = |s: &str| s.replace('|', "\\|").replace(['\r', '\n'], " ");
        let mut out = format!(
            "# Guidance track record — {}\n\nAs of {}. {}\n\nPopulation: supported agreed rows in the supplied registry; historical completeness unassessed. Workbook bytes available by {}. {}\n\n| Site | Year | Metric and basis | Guidance vintage | Guidance | Original outcome | Latest outcome |\n|---|---|---|---|---|---|---|\n",
            clean(&self.issuer),
            self.as_of,
            if self.synthetic {
                "Synthetic invented evidence; not gate evidence."
            } else {
                "From public filings; no affiliation implied."
            },
            self.workbooks_available_by,
            NO_CLAIM
        );
        for r in &self.rows {
            let value = |o: &OutcomeAssessment| {
                o.score.map_or_else(
                    || {
                        format!(
                            "unresolved: {}{}",
                            o.status,
                            o.refusal.as_ref().map_or_else(String::new, |e| format!(
                                " ({}: {})",
                                e.code,
                                clean(&e.what)
                            ),)
                        )
                    },
                    |s| {
                        format!(
                            "{}; {}; z {}; {}{}",
                            o.printed
                                .map_or_else(|| "unavailable".into(), |p| rat_text(p.value)),
                            side(s.side),
                            s.z.map_or_else(|| "undefined".into(), rat_text),
                            o.status,
                            if s.boundary {
                                "; rounding boundary"
                            } else {
                                ""
                            },
                        )
                    },
                )
            };
            let _ = writeln!(
                out,
                "| {} | {} | {} / {} | {}{} | {} | {} | {} |",
                clean(&r.site),
                r.year,
                clean(&r.metric),
                clean(&r.guidance.basis),
                r.document.published_at,
                if r.ambiguous_earliest {
                    " (ambiguous earliest)"
                } else if r.earliest_registered {
                    " (earliest registered)"
                } else {
                    " (later registered)"
                },
                clean(&r.guidance.token),
                value(&r.first_published),
                value(&r.latest_registered)
            );
        }
        out.push_str("\n## Sources\n\n");
        for r in &self.rows {
            let _ = writeln!(
                out,
                "\nGuidance source: `{}`; SHA-256 `{}`. {}",
                clean(&r.guidance.locator.to_string()),
                r.guidance.doc_sha256,
                clean(&r.document.published_at_basis)
            );
            for source in &r.first_published.sources {
                let _ = writeln!(
                    out,
                    "\nOriginal outcome source: `{}`; SHA-256 `{}`; Claim `{}`.",
                    clean(&source.locator),
                    source.key.reporting_vintage.file_sha256,
                    source.claim.claim_id
                );
            }
            for source in &r.latest_registered.sources {
                let _ = writeln!(
                    out,
                    "\nLatest outcome source: `{}`; SHA-256 `{}`; Claim `{}`.",
                    clean(&source.locator),
                    source.key.reporting_vintage.file_sha256,
                    source.claim.claim_id
                );
            }
        }
        out.push_str("\n## Earliest registered guidance summaries\n\n");
        let shown = |value: Option<Interval>| {
            value.map_or_else(
                || "undefined".into(),
                |i| format!("[{}, {}]", rat_text(i.lo), rat_text(i.hi)),
            )
        };
        for g in &self.summaries {
            let s = &g.scores;
            let _ = writeln!(
                out,
                "- {} {} ({}; {}): {} inside / {} scored; {} unresolved of {} members. Whole-cohort hit bounds {}; rounding-aware bounds {}. IID 90% reference interval {}. Mean z {} over {} ranges; rounding-only bounds {}.\n",
                g.year,
                g.metric,
                clean(&g.basis),
                g.selector,
                s.inside,
                s.n_resolved,
                s.n_unresolved,
                s.n_members,
                shown(s.whole_cohort_hit_bounds),
                shown(s.rounding_aware_hit_bounds),
                shown(s.iid_reference_interval_90_exact),
                s.mean_z.map_or_else(|| "undefined".into(), rat_text),
                s.n_ranges,
                shown(s.mean_z_rounding_interval),
            );
        }
        let _ = write!(
            out,
            "\nExact ratios are retained. IID reference intervals apply only to resolved printed-value hits; they do not account for selective missingness or outcome rounding. Cross-site and year dependence, effective sample size and bias sampling uncertainty are not estimated. Rounding bounds do not measure forecast uncertainty. Cost scores are unconditional: gold-price adjustments are not implemented, and a miss does not identify controllable operating causes.\n\nReport digest: `{}`.\n",
            self.digest.to_hex()
        );
        out
    }
}

/// Deterministic invented guidance, a missing outcome and a later comparative through this same adapter.
pub fn synthetic() -> Result<TrackRecordReport> {
    use crate::sources::{DocumentRow, RegisteredSource};
    use fmine_core::hash::sha256;
    use fmine_registry::transcriptions::{Agreement, Locator};
    let (mut transcripts, templates) = super::synthetic_evidence()?;
    let mut profile = crate::synthetic::synthetic_profile();
    profile.retrieved_utc = "2032-02-01T00:00:00Z".into();
    let document = transcripts.documents[0].clone();
    profile.documents = vec![DocumentRow {
        file: document.file.clone(),
        dir: "synthetic".into(),
        kind: document.kind.clone(),
        url: "https://example.invalid/guidance".into(),
        url_basis: "synthetic".into(),
        published_at: document.published_at.clone(),
        published_at_basis: document.published_at_basis.clone(),
        pages: 1,
        bytes: 1,
        sha256: document.sha256.clone(),
    }];
    let mut guidance = transcripts.agreement.accepted[0].clone();
    guidance.token = "100 - 120".into();
    guidance.value = parse_token(&guidance.token).map_err(source_error)?;
    let mut pending = guidance.clone();
    pending.quantity = "guidance_missing_mine_production_koz".into();
    pending.locator = Locator {
        row: "Missing Mine".into(),
        ..pending.locator
    };
    transcripts.agreement = Agreement {
        accepted: vec![guidance, pending],
        ..Agreement::default()
    };
    let template = &templates[0];
    let mut cells = Vec::new();
    for (release, token) in [("2030Q4", "125"), ("2031Q4", "115")] {
        let file = format!("Synthetic_{release}.xlsx");
        let digest = sha256(format!("synthetic|{release}|{token}").as_bytes());
        let mut c = template.clone();
        c.key.valid_period = key::Period {
            year: 2030,
            kind: PeriodKind::FullYear,
        };
        c.key.reporting_vintage.release = release.into();
        c.key.reporting_vintage.file_sha256 = digest.to_hex();
        c.locator = format!("{file}:YTD!C4");
        let value = PublicValue::Number {
            stored: token.into(),
            displayed: Ok(token.into()),
            under_status: None,
        };
        let tag = key::canonical_value_tag(&c.key, &value);
        c.claim = fmine_claim::Claim::new(
            &c.key.line.label,
            value,
            &tag,
            Representation::Deterministic,
            SpatialSupport::PeriodAggregate,
            AdditivityClass::ExtensiveAdditive,
            template.claim.evidence,
            template.claim.epistemic,
            template.claim.state,
            template.claim.confidentiality.clone(),
            vec![],
            vec![],
            vec![fmine_core::anchor::SourceAnchor::new(
                &file, digest.0, "YTD!C4",
            )],
        );
        profile.sources.push(RegisteredSource {
            file_name: file,
            kind: SourceKind::MineStats,
            quarter: release.into(),
            url: "https://example.invalid/source".into(),
            url_basis: "synthetic".into(),
            bytes: 1,
            sha256_hex: digest.to_hex(),
        });
        cells.push(c);
    }
    assess(&profile, &transcripts, &cells, None, "2032-02-01")
}
