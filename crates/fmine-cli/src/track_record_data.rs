//! Retrospective source binding for issuer guidance (BD-341).
//! Determinism: DeterministicPerIsa (exact binding; reference interval from the scoring kernel).
//!
//! DESIGN: retain each agreed guidance document and missing outcome. A later comparative is never
//! relabelled as original publication. This adapter does not backdate registrations in the forecast ledger.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use fmine_calib::guidance_scores::{
    self as scores, Guidance, GuidanceScore, GuidanceSummary, PrintedOutcome, ScoringMember,
};
use fmine_claim::{AdditivityClass, Claim, Confidentiality, Representation, SpatialSupport};
use fmine_core::anchor::SourceAnchor;
use fmine_core::codec::Encoder;
use fmine_core::hash::{Digest32, blake3_hash, sha256};
use fmine_evidence::{
    EpistemicStatus, EvidenceClass, KnowledgeState, KuKind, Refusal, RefusalCode,
};
use fmine_numerics::exact_feas::{Convention, Rat};
use fmine_public::key::{
    KeyedCell, OwnershipBasis, PeriodKind, PublicValue, Scope, StatusKind, SubjectStatus,
};
use fmine_public::sources::{IssuerProfile, SourceKind, load_issuer_profile};
use fmine_public::transcribe::{CheckedDocument, TranscriptionReport};
use fmine_registry::sources::is_date;
use fmine_registry::transcriptions::{Accepted, Agreement, Decimal, Locator, Token, parse_token};

fn js(text: &str) -> String {
    format!("\"{}\"", crate::escape_json(text))
}

pub const NO_CLAIM: &str = "Retrospective description of cited issuer guidance and reported outcomes. No forecast registration, calibration verdict, ranking, inferred probability or price adjustment.";
type Result<T> = std::result::Result<T, Refusal>;

#[derive(Debug, Clone)]
pub enum OutcomeBinding {
    Resolved {
        cell: Box<KeyedCell>,
        comparative: bool,
    },
    Unresolved {
        code: String,
        reason: String,
    },
}

#[derive(Debug, Clone)]
pub struct TrackRecordRow {
    pub id: String,
    pub site: String,
    pub metric: String,
    pub year: u16,
    pub guidance: Accepted,
    pub published_at: String,
    pub publication_basis: String,
    /// Zero is the earliest registered statement for this site, metric and year, not necessarily initial guidance.
    pub vintage: usize,
    /// Declared outcome population, retained when this member has no usable numeric outcome.
    pub comparative: bool,
    pub outcome: OutcomeBinding,
    /// Later appearances remain cited separately and never replace the selected outcome.
    pub later_outcomes: Vec<KeyedCell>,
    pub score: Option<GuidanceScore>,
}

#[derive(Debug, Clone)]
pub struct TrackRecordSummary {
    pub site: String,
    pub metric: String,
    pub unit: String,
    pub basis: String,
    pub vintage: usize,
    pub comparative: bool,
    pub members: Vec<String>,
    pub scores: GuidanceSummary,
}

#[derive(Debug, Clone)]
pub struct TrackRecordReport {
    pub issuer: String,
    pub synthetic: bool,
    pub through: Option<String>,
    pub digest: Digest32,
    pub source_digest: Digest32,
    pub source_refusals: Vec<fmine_public::mine_stats::CellRefusal>,
    pub rows: Vec<TrackRecordRow>,
    pub summaries: Vec<TrackRecordSummary>,
}

fn failure(code: RefusalCode, what: impl Into<String>) -> Refusal {
    Refusal::new(code, format!("guidance track record: {}", what.into()))
}

fn unresolved(code: RefusalCode, reason: impl Into<String>) -> OutcomeBinding {
    let refusal = failure(code, reason);
    OutcomeBinding::Unresolved {
        code: refusal.code.into(),
        reason: refusal.what,
    }
}

fn rule_error(error: impl std::fmt::Display) -> Refusal {
    failure(
        RefusalCode::NumericalCertificateFailed207,
        error.to_string(),
    )
}

fn decimal(d: Decimal) -> Result<Rat> {
    let den = 10_i128
        .checked_pow(d.scale)
        .ok_or_else(|| rule_error("decimal scale overflow"))?;
    Rat::new(d.mantissa, den).map_err(rule_error)
}

fn site_key(site: &str) -> String {
    site.to_ascii_lowercase()
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

fn basis(text: &str) -> Result<(bool, Rat)> {
    if text == "100%" {
        return Ok((false, Rat::from_integer(1)));
    }
    let parsed = text
        .strip_prefix("attributable:")
        .and_then(|s| s.split_once('/'))
        .and_then(|(n, d)| Some((n.parse::<i128>().ok()?, d.parse::<i128>().ok()?)));
    if let Some((n, d)) = parsed
        && n > 0
        && d > 0
        && n <= d
    {
        return Ok((true, Rat::new(n, d).map_err(rule_error)?));
    }
    Err(failure(
        RefusalCode::UndeclaredUnitBasisOrTz107,
        "ownership must state 100% or attributable:n/d",
    ))
}

fn cell_basis(basis: OwnershipBasis) -> Result<(bool, Rat)> {
    match basis {
        OwnershipBasis::Consolidated100 => Ok((false, Rat::from_integer(1))),
        OwnershipBasis::Attributable { num, den, .. } if num > 0 && den > 0 && num <= den => Ok((
            true,
            Rat::new(i128::from(num), i128::from(den)).map_err(rule_error)?,
        )),
        _ => Err(failure(
            RefusalCode::UndeclaredUnitBasisOrTz107,
            "source ownership is unprinted or invalid",
        )),
    }
}

fn metric(quantity: &str) -> Option<(&str, &str)> {
    [
        "production_koz",
        "production_kt",
        "aisc_usd_per_oz",
        "aisc_usd_per_lb",
        "tcc_usd_per_oz",
        "c1_usd_per_lb",
    ]
    .into_iter()
    .find_map(|m| {
        quantity
            .strip_prefix("guidance_")?
            .strip_suffix(&format!("_{m}"))
            .map(|site| (site, m))
    })
}

fn specification(a: &Accepted, name: &str) -> Result<((bool, Rat), String)> {
    let (unit, suffix) = match name {
        "production_koz" => ("koz", ""),
        "production_kt" => ("kt", ""),
        "aisc_usd_per_oz" => ("usd_per_oz", ";aisc=issuer-reported-per-sold-oz"),
        "aisc_usd_per_lb" => ("usd_per_lb", ";aisc=issuer-reported-per-sold-lb"),
        "tcc_usd_per_oz" => ("usd_per_oz", ";tcc=issuer-reported-per-sold-oz"),
        "c1_usd_per_lb" => ("usd_per_lb", ";c1=issuer-reported-per-sold-lb"),
        _ => {
            return Err(failure(
                RefusalCode::NoClaimInDomain201,
                "unsupported guidance metric",
            ));
        }
    };
    if a.unit != unit {
        return Err(failure(
            RefusalCode::BasisMismatch105,
            format!("{} needs unit {unit}", a.locator),
        ));
    }
    let ownership = basis(a.basis.strip_suffix(suffix).ok_or_else(|| {
        failure(
            RefusalCode::UndeclaredUnitBasisOrTz107,
            format!(
                "{} must declare {suffix} to compare the same issuer-reported cost definition",
                a.locator
            ),
        )
    })?)?;
    let line = match name {
        "production_koz" => {
            if ownership.0 {
                "au_prod_attr_koz"
            } else {
                "au_prod_100_koz"
            }
        }
        "production_kt" => {
            if ownership.0 {
                "cu_prod_attr_kt"
            } else {
                "cu_prod_100_kt"
            }
        }
        cost => cost,
    };
    Ok((ownership, format!("summary:{line}")))
}

fn limits(a: &Accepted) -> Result<Guidance> {
    if parse_token(&a.token).map_err(rule_error)? != a.value {
        return Err(failure(
            RefusalCode::SourceCellUnusable208,
            format!("{} token and parsed value differ", a.locator),
        ));
    }
    match a.value {
        Token::Point(d) => Ok(Guidance::Point { value: decimal(d)? }),
        Token::Range(lo, hi) => {
            let (lo, hi) = (decimal(lo)?, decimal(hi)?);
            Ok(if lo == hi {
                Guidance::Point { value: lo }
            } else {
                Guidance::Range { lo, hi }
            })
        }
    }
}

fn affected_status<'a>(
    statuses: &'a [SubjectStatus],
    subject: &str,
    year: u16,
) -> Option<&'a SubjectStatus> {
    statuses.iter().find(|s| {
        s.subject == subject
            && (s.first <= (year, 12)
                // Dated disposal starts in the first full following month. A January status may
                // describe a December sale; the prior annual perimeter then needs evidence.
                || (s.kind == StatusKind::Disposed && s.first == (year.saturating_add(1), 1)))
            && s.last.is_none_or(|last| last >= (year, 1))
    })
}

fn comparative_population(profile: &IssuerProfile, year: u16) -> bool {
    let original = format!("{year}Q4");
    !profile
        .sources
        .iter()
        .any(|s| s.kind == SourceKind::MineStats && s.quarter == original)
        && profile
            .sources
            .iter()
            .any(|s| s.kind == SourceKind::MineStats && s.quarter > original)
}

fn numeric(
    cell: &KeyedCell,
    a: &Accepted,
    own: (bool, Rat),
    production: bool,
) -> Result<PrintedOutcome> {
    if cell_basis(cell.key.ownership_basis)? != own || cell.key.unit != a.unit {
        return Err(failure(
            RefusalCode::BasisMismatch105,
            format!(
                "{} does not match guidance unit and ownership at {}",
                cell.locator, a.locator
            ),
        ));
    }
    if !cell.claim.confidentiality.is_public() {
        return Err(failure(
            RefusalCode::MnpiWall301,
            "non-public outcome in a public report",
        ));
    }
    if cell.claim.stale || cell.claim.calibration_failed {
        return Err(failure(
            RefusalCode::StaleAnchor502,
            format!("{} is stale or has failed calibration", cell.locator),
        ));
    }
    if cell.claim.support != SpatialSupport::PeriodAggregate
        || cell.claim.additivity
            != if production {
                AdditivityClass::ExtensiveAdditive
            } else {
                AdditivityClass::IntensiveWeighted
            }
    {
        return Err(failure(
            RefusalCode::SupportMismatch102,
            format!("{} has incompatible support or additivity", cell.locator),
        ));
    }
    let synthetic_number = cell.key.issuer == fmine_public::synthetic::SYNTHETIC_ISSUER
        && cell.claim.evidence == EvidenceClass::Assumed
        && cell.claim.state == KnowledgeState::Unknown
        && matches!(cell.claim.representation, Representation::Deterministic);
    if (!matches!(
        cell.claim.state,
        KnowledgeState::Known | KnowledgeState::Estimated
    ) && !synthetic_number)
        || !cell.claim.anchors.iter().any(|anchor| {
            Digest32(anchor.artifact_sha256).to_hex() == cell.key.reporting_vintage.file_sha256
        })
    {
        return Err(failure(
            RefusalCode::NotObservable202,
            format!("{} lacks usable source-bound evidence", cell.locator),
        ));
    }
    let PublicValue::Number {
        displayed: Ok(text),
        under_status: None,
        ..
    } = &cell.claim.value
    else {
        return Err(failure(
            RefusalCode::SourceCellUnusable208,
            format!(
                "{} is missing, a placeholder, unreadable or under a subject-status restriction",
                cell.locator
            ),
        ));
    };
    let Token::Point(d) = parse_token(text).map_err(rule_error)? else {
        return Err(failure(
            RefusalCode::SourceCellUnusable208,
            "an actual must print one value",
        ));
    };
    let value = decimal(d)?;
    if production && value.num() < 0 {
        return Err(failure(
            RefusalCode::SourceCellUnusable208,
            "production cannot be negative",
        ));
    }
    let half = decimal(Decimal {
        mantissa: 1,
        scale: d.scale,
    })?
    .checked_div(Rat::from_integer(2))
    .map_err(rule_error)?;
    Ok(PrintedOutcome {
        value,
        interval: Convention::Closed
            .interval(value, half)
            .map_err(|e| rule_error(format!("{e:?}")))?,
    })
}

#[allow(clippy::too_many_arguments)]
fn binding(
    profile: &IssuerProfile,
    cells: &[KeyedCell],
    subject: &str,
    row: &Accepted,
    name: &str,
    year: u16,
    published_at: &str,
) -> Result<(OutcomeBinding, Vec<KeyedCell>, Option<PrintedOutcome>)> {
    let (own, line) = specification(row, name)?;
    if published_at > format!("{year:04}-12-31").as_str() {
        return Ok((
            unresolved(
                RefusalCode::AssayLookahead108,
                "guidance published after the outcome year; cannot establish a forward statement without an independently evidenced outcome-availability date",
            ),
            vec![],
            None,
        ));
    }
    if let Some(status) = affected_status(&profile.subject_statuses, subject, year) {
        let reason = match status.kind {
            StatusKind::Disposed => "divested",
            StatusKind::Withheld => "definition_changed_by_withheld_period",
            StatusKind::CareAndMaintenance => "not_published_during_care_and_maintenance",
        };
        return Ok((
            unresolved(
                RefusalCode::NotObservable202,
                format!(
                    "{reason}: annual support overlaps or adjoins the disposal boundary in {}",
                    status.evidence
                ),
            ),
            vec![],
            None,
        ));
    }
    let original = format!("{year}Q4");
    let has_original = profile
        .sources
        .iter()
        .any(|s| s.kind == SourceKind::MineStats && s.quarter == original);
    let mut candidates: Vec<_> = cells
        .iter()
        .filter(|c| {
            c.key.issuer == profile.issuer
                && c.key.subject == subject
                && c.key.valid_period.year == year
                && c.key.valid_period.kind == PeriodKind::FullYear
                && matches!(c.key.scope, Scope::Site)
                && c.key.line.label == line
                && c.key.reporting_vintage.release >= original
        })
        .collect();
    candidates.sort_by(|a, b| {
        (&a.key.reporting_vintage.release, &a.locator)
            .cmp(&(&b.key.reporting_vintage.release, &b.locator))
    });
    let selected = if has_original {
        Some(original.as_str())
    } else {
        candidates
            .first()
            .map(|c| c.key.reporting_vintage.release.as_str())
    };
    let Some(selected) = selected else {
        return Ok((
            unresolved(
                RefusalCode::NoClaimInDomain201,
                "not_published: no registered annual outcome",
            ),
            vec![],
            None,
        ));
    };
    let first: Vec<_> = candidates
        .iter()
        .copied()
        .filter(|c| c.key.reporting_vintage.release == selected)
        .collect();
    let later = candidates
        .iter()
        .copied()
        .filter(|c| c.key.reporting_vintage.release.as_str() > selected)
        .cloned()
        .collect();
    let cell = match first.as_slice() {
        [one] => *one,
        [] => {
            return Ok((
                unresolved(
                    RefusalCode::NoClaimInDomain201,
                    "no annual summary cell in the original registered release; later comparatives cannot replace it",
                ),
                later,
                None,
            ));
        }
        _ => {
            return Ok((
                unresolved(
                    RefusalCode::NonIdentifiableAttribution104,
                    "multiple annual source cells in the selected release",
                ),
                later,
                None,
            ));
        }
    };
    match numeric(cell, row, own, name.starts_with("production_")) {
        Ok(value) => Ok((
            OutcomeBinding::Resolved {
                cell: Box::new(cell.clone()),
                comparative: !has_original,
            },
            later,
            Some(value),
        )),
        Err(e) => Ok((
            OutcomeBinding::Unresolved {
                code: e.code.into(),
                reason: e.what,
            },
            later,
            None,
        )),
    }
}

fn assess(
    profile: &IssuerProfile,
    transcripts: &TranscriptionReport,
    cells: &[KeyedCell],
    site: Option<&str>,
) -> Result<TrackRecordReport> {
    if !transcripts.agreement.is_consistent() || !transcripts.agreement.single_reader.is_empty() {
        return Err(failure(
            RefusalCode::NotObservable202,
            "complete two-reader agreement before selecting the guidance population",
        ));
    }
    let mut rows = Vec::new();
    for a in transcripts
        .agreement
        .accepted
        .iter()
        .filter(|a| a.quantity.starts_with("guidance_"))
    {
        let (subject_key, name) = metric(&a.quantity).ok_or_else(|| {
            failure(
                RefusalCode::NoClaimInDomain201,
                format!(
                    "unsupported guidance quantity {} at {}",
                    a.quantity, a.locator
                ),
            )
        })?;
        let key = subject_key.replace('_', "-");
        if site.is_some_and(|s| site_key(s) != key) {
            continue;
        }
        let subjects: Vec<_> = profile
            .subjects
            .iter()
            .filter(|s| site_key(s) == key && !profile.regions.contains(s))
            .collect();
        let [subject] = subjects.as_slice() else {
            return Err(failure(
                RefusalCode::NoClaimInDomain201,
                format!("{} does not name one registered site", a.quantity),
            ));
        };
        let period = fmine_public::key::parse_period_code(&a.period)
            .filter(|p| p.kind == PeriodKind::FullYear && p.year > 0)
            .ok_or_else(|| {
                failure(
                    RefusalCode::SupportMismatch102,
                    format!("{} needs a full-year period", a.locator),
                )
            })?;
        let documents: Vec<_> = transcripts
            .documents
            .iter()
            .filter(|d| d.file == a.locator.doc)
            .collect();
        let [doc] = documents.as_slice() else {
            return Err(failure(
                RefusalCode::NotObservable202,
                format!("{} needs one checked document", a.locator),
            ));
        };
        if !is_date(&doc.published_at)
            || doc.sha256 != a.doc_sha256
            || a.readers.iter().collect::<BTreeSet<_>>().len() < 2
        {
            return Err(failure(
                RefusalCode::NotObservable202,
                format!(
                    "{} needs agreement on checked bytes and a registered publication date",
                    a.locator
                ),
            ));
        }
        let limits = limits(a)?;
        let (outcome, later_outcomes, numeric) = binding(
            profile,
            cells,
            subject,
            a,
            name,
            period.year,
            &doc.published_at,
        )?;
        let score = numeric
            .map(|y| scores::score(limits, y))
            .transpose()
            .map_err(rule_error)?;
        let mut enc = Encoder::new_frame(*b"GTRW", 1);
        for field in [
            a.locator.to_string(),
            a.doc_sha256.clone(),
            a.token.clone(),
            a.quantity.clone(),
            a.period.clone(),
            a.unit.clone(),
            a.basis.clone(),
            doc.published_at.clone(),
        ] {
            enc.put_str(&field);
        }
        rows.push(TrackRecordRow {
            id: blake3_hash(&enc.finish()).to_hex(),
            site: (*subject).clone(),
            metric: name.into(),
            year: period.year,
            guidance: a.clone(),
            published_at: doc.published_at.clone(),
            publication_basis: doc.published_at_basis.clone(),
            vintage: 0,
            comparative: comparative_population(profile, period.year),
            outcome,
            later_outcomes,
            score,
        });
    }
    if rows.is_empty() {
        return Err(failure(
            RefusalCode::NoClaimInDomain201,
            "no accepted guidance for the requested sites; no synthetic replacement is used",
        ));
    }
    rows.sort_by(|a, b| {
        (
            &a.site,
            &a.metric,
            a.year,
            &a.published_at,
            &a.guidance.locator,
        )
            .cmp(&(
                &b.site,
                &b.metric,
                b.year,
                &b.published_at,
                &b.guidance.locator,
            ))
    });
    let mut seen = BTreeMap::<(String, String, u16), usize>::new();
    let mut unique = BTreeSet::new();
    for row in &mut rows {
        if !unique.insert((
            row.site.clone(),
            row.metric.clone(),
            row.year,
            row.published_at.clone(),
        )) {
            return Err(failure(
                RefusalCode::NonIdentifiableAttribution104,
                "multiple guidance rows have the same site, metric, year and publication date; the earliest statement is ambiguous",
            ));
        }
        let ordinal = seen
            .entry((row.site.clone(), row.metric.clone(), row.year))
            .or_default();
        row.vintage = *ordinal;
        *ordinal += 1;
    }
    type Group = (String, String, String, String, usize, bool);
    let mut groups = BTreeMap::<Group, Vec<ScoringMember>>::new();
    for row in &rows {
        let numeric = match &row.outcome {
            OutcomeBinding::Resolved { cell, .. } => Some(numeric(
                cell,
                &row.guidance,
                specification(&row.guidance, &row.metric)?.0,
                row.metric.starts_with("production_"),
            )?),
            OutcomeBinding::Unresolved { .. } => None,
        };
        groups
            .entry((
                row.site.clone(),
                row.metric.clone(),
                row.guidance.unit.clone(),
                row.guidance.basis.clone(),
                row.vintage,
                row.comparative,
            ))
            .or_default()
            .push(ScoringMember {
                id: row.id.clone(),
                guidance: limits(&row.guidance)?,
                outcome: numeric,
            });
    }
    let mut summaries = Vec::new();
    for ((site, metric, unit, basis, vintage, comparative), members) in groups {
        summaries.push(TrackRecordSummary {
            site,
            metric,
            unit,
            basis,
            vintage,
            comparative,
            members: members.iter().map(|m| m.id.clone()).collect(),
            scores: scores::summarize(&members).map_err(rule_error)?,
        });
    }
    let mut enc = Encoder::new_frame(*b"GTSR", 1);
    let mut agreement = transcripts.agreement.clone();
    agreement.accepted.sort_by(|a, b| a.locator.cmp(&b.locator));
    enc.put_str(&agreement.digest());
    enc.put_str(&fmine_public::key::claims_digest(cells).to_hex());
    let mut sources: Vec<_> = profile
        .sources
        .iter()
        .map(|s| (&s.file_name, &s.quarter, &s.sha256_hex))
        .collect();
    sources.sort_unstable();
    for (file, quarter, sha) in sources {
        enc.put_str(file);
        enc.put_str(quarter);
        enc.put_str(sha);
    }
    let mut report = TrackRecordReport {
        issuer: profile.issuer.clone(),
        synthetic: profile.issuer == fmine_public::synthetic::SYNTHETIC_ISSUER,
        through: profile.through.clone(),
        digest: Digest32([0; 32]),
        source_digest: blake3_hash(&enc.finish()),
        source_refusals: vec![],
        rows,
        summaries,
    };
    report.digest = blake3_hash(report.body().as_bytes());
    Ok(report)
}

/// Checks registered bytes before binding. The release bound is not a historical information cut.
pub fn run(
    root: &Path,
    issuer: &str,
    site: Option<&str>,
    through: Option<&str>,
) -> Result<TrackRecordReport> {
    let mut profile = load_issuer_profile(root, issuer)?;
    if let Some(bound) = through {
        profile = profile.through(bound)?;
    }
    let claims = fmine_public::key::mine_stats::load_claims(&profile, root)?;
    let transcripts=fmine_public::transcribe::load_transcription_report(&profile,root)?.ok_or_else(||failure(RefusalCode::NoClaimInDomain201,format!("registries/transcriptions/{issuer}.toml is absent; register guidance documents and agreed readings")))?;
    let mut report = assess(&profile, &transcripts, &claims.claims, site)?;
    report.source_refusals = claims.refusals;
    report.digest = blake3_hash(report.body().as_bytes());
    Ok(report)
}

impl TrackRecordReport {
    fn body(&self) -> String {
        let source_refusals = self
            .source_refusals
            .iter()
            .map(|r| {
                format!(
                    "{{\"locator\":{},\"reason\":{}}}",
                    js(&r.locator),
                    js(&r.reason)
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        let prefix = format!("\"source_refusals\":[{source_refusals}],");
        let rows=self.rows.iter().map(|r| {
            let (outcome,inputs)=match &r.outcome {
                OutcomeBinding::Resolved{cell,comparative}=>(format!("{{\"state\":{},\"source\":{}}}",js(if *comparative {"resolved_on_comparative_vintage"} else {"resolved_first_published"}),cell.to_json()),vec![r.guidance.locator.to_string(),cell.locator.clone()]),
                OutcomeBinding::Unresolved{code,reason}=>(format!("{{\"state\":\"unresolved\",\"code\":{},\"reason\":{}}}",js(code),js(reason)),vec![r.guidance.locator.to_string()]),
            };
            format!("{{\"id\":{},\"site\":{},\"metric\":{},\"year\":{},\"guidance_vintage_ordinal\":{},\"published_at\":{},\"published_at_basis\":{},\"guidance\":{{\"at\":{},\"sha256\":{},\"token\":{},\"unit\":{},\"basis\":{},\"period\":{},\"reader_count\":{}}},\"outcome\":{},\"later_outcomes\":[{}],\"score\":{{\"rule\":\"guidance.range_score\",\"rules_version\":\"guidance-track-record/1\",\"in\":[{}],\"value\":{}}}}}",js(&r.id),js(&r.site),js(&r.metric),r.year,r.vintage,js(&r.published_at),js(&r.publication_basis),js(&r.guidance.locator.to_string()),js(&r.guidance.doc_sha256),js(&r.guidance.token),js(&r.guidance.unit),js(&r.guidance.basis),js(&r.guidance.period),r.guidance.readers.len(),outcome,r.later_outcomes.iter().map(KeyedCell::to_json).collect::<Vec<_>>().join(","),inputs.iter().map(|s|js(s)).collect::<Vec<_>>().join(","),r.score.as_ref().map_or_else(||"null".into(),GuidanceScore::to_json))
        }).collect::<Vec<_>>().join(",");
        let summaries=self.summaries.iter().map(|s|format!("{{\"site\":{},\"metric\":{},\"unit\":{},\"basis\":{},\"guidance_vintage_ordinal\":{},\"outcome_selector\":{},\"in\":[{}],\"rule\":\"guidance.descriptive_summary\",\"rules_version\":\"guidance-track-record/1\",\"scores\":{}}}",js(&s.site),js(&s.metric),js(&s.unit),js(&s.basis),s.vintage,js(if s.comparative {"comparative_only"} else {"first_published"}),s.members.iter().map(|id|js(id)).collect::<Vec<_>>().join(","),s.scores.to_json())).collect::<Vec<_>>().join(",");
        prefix
            + &format!(
                "\"schema\":\"fmine.guidance.track-record.v1\",\"issuer\":{},\"lane\":{},\"through\":{},\"source_digest\":{},\"mode\":\"single_site_descriptive_cards\",\"rows\":[{}],\"summaries\":[{}],\"no_affiliation\":{},\"warnings\":[\"Guidance-vintage ordinal is the order of registered documents, not an issuer revision classification.\",\"Workbook release bounds are not historical information-availability cuts. Guidance after year end is left unresolved.\",\"Comparative-only outcomes are scored separately and never treated as original publications.\",\"Rounding enclosures are not sampling confidence intervals. The binomial reference assumes independent identical trials.\",\"Effective sample size, year effects and bias sampling intervals are not estimated; no pooled site score is emitted.\"],\"no_claim\":{}",
                js(&self.issuer),
                js(if self.synthetic {
                    "synthetic"
                } else {
                    "public"
                }),
                self.through.as_deref().map_or_else(|| "null".into(), js),
                js(&self.source_digest.to_hex()),
                rows,
                summaries,
                js(if self.synthetic {
                    "Invented synthetic evidence; no affiliation implied"
                } else {
                    "from public filings; no affiliation implied"
                }),
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
}

fn synthetic_evidence() -> Result<(IssuerProfile, TranscriptionReport, Vec<KeyedCell>)> {
    use fmine_public::key::{LineKey, PublicKey, ShareSource, Vintage};
    let mut profile = fmine_public::synthetic::synthetic_profile();
    profile.subjects = vec!["Example Mine".into()];
    profile.sources.clear();
    profile.subject_statuses.clear();
    let digest =
        sha256(b"invented guidance production 100-120 koz and AISC 1000-1200 USD/oz FY2030");
    let accepted = [
        ("production_koz", "100 - 120", "koz", "attributable:4/5"),
        (
            "aisc_usd_per_oz",
            "1000 - 1200",
            "usd_per_oz",
            "attributable:4/5;aisc=issuer-reported-per-sold-oz",
        ),
    ]
    .into_iter()
    .map(|(metric, token, unit, basis)| {
        Ok(Accepted {
            locator: Locator {
                doc: "synthetic-guidance.txt".into(),
                page: 1,
                table: "Invented annual guidance".into(),
                row: "Example Mine".into(),
                col: metric.into(),
            },
            doc_sha256: digest.to_hex(),
            token: token.into(),
            value: parse_token(token).map_err(rule_error)?,
            quantity: format!("guidance_example_mine_{metric}"),
            unit: unit.into(),
            basis: basis.into(),
            period: "2030FY".into(),
            readers: vec!["synthetic-reader-a".into(), "synthetic-reader-b".into()],
        })
    })
    .collect::<Result<Vec<_>>>()?;
    let transcripts = TranscriptionReport {
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
    let outcome_bytes = b"invented FY2030 production 125 koz attributable 4/5";
    let digest = sha256(outcome_bytes);
    let key = PublicKey {
        issuer: "synthetic".into(),
        subject: "Example Mine".into(),
        valid_period: fmine_public::key::Period {
            year: 2030,
            kind: PeriodKind::FullYear,
        },
        reporting_vintage: Vintage {
            release: "2030Q4".into(),
            file_sha256: digest.to_hex(),
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
    let path = "synthetic/2030Q4/production.txt";
    let claim = Claim::new(
        &key.line.label,
        PublicValue::Number {
            stored: "125".into(),
            displayed: Ok("125".into()),
            under_status: None,
        },
        &key.canonical_tag("125"),
        Representation::Deterministic,
        SpatialSupport::PeriodAggregate,
        AdditivityClass::ExtensiveAdditive,
        EvidenceClass::Assumed,
        EpistemicStatus::KnownUnknown(KuKind::Parametric),
        KnowledgeState::Estimated,
        Confidentiality::public(),
        vec![],
        vec![],
        vec![SourceAnchor::new(path, digest.0, "Annual!C1")],
    );
    let cell = KeyedCell {
        locator: format!("{path}:Annual!C1"),
        printed_basis: key.ownership_basis,
        key,
        claim,
        basis_evidence: None,
        status_evidence: None,
        label_as_printed: None,
        unit_evidence: None,
    };
    // The shared synthetic profile has no on-disk sources; this fixture supplies its own invented bytes.
    profile
        .sources
        .push(fmine_public::sources::RegisteredSource {
            file_name: path.into(),
            kind: SourceKind::MineStats,
            quarter: "2030Q4".into(),
            url: "synthetic:invented-annual-production".into(),
            url_basis: "DESIGN: invented source bytes; no external retrieval".into(),
            bytes: outcome_bytes.len() as u64,
            sha256_hex: digest.to_hex(),
        });
    Ok((profile, transcripts, vec![cell]))
}

/// Invented positive known answer and an unresolved cost outcome through the same binder and scorer.
pub fn synthetic() -> Result<TrackRecordReport> {
    let (profile, transcripts, cells) = synthetic_evidence()?;
    assess(&profile, &transcripts, &cells, None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use fmine_calib::guidance_scores::Side;
    use std::fmt::Write as _;

    fn production(report: &TrackRecordReport) -> &TrackRecordRow {
        report
            .rows
            .iter()
            .find(|r| r.metric == "production_koz" || r.metric == "production_kt")
            .expect("production row")
    }

    fn revised(cell: &KeyedCell, year: u16, release: &str, token: &str) -> KeyedCell {
        let mut cell = cell.clone();
        cell.key.valid_period.year = year;
        cell.key.reporting_vintage.release = release.into();
        let digest = sha256(format!("synthetic {release} FY{year} {token}").as_bytes());
        cell.key.reporting_vintage.file_sha256 = digest.to_hex();
        let path = format!("synthetic/{release}/{year}.txt");
        cell.locator = format!("{path}:Annual!C1");
        cell.claim = Claim::new(
            &cell.key.line.label,
            PublicValue::Number {
                stored: token.into(),
                displayed: Ok(token.into()),
                under_status: None,
            },
            &cell.key.canonical_tag(token),
            Representation::Deterministic,
            SpatialSupport::PeriodAggregate,
            AdditivityClass::ExtensiveAdditive,
            EvidenceClass::Assumed,
            EpistemicStatus::KnownUnknown(KuKind::Parametric),
            KnowledgeState::Estimated,
            Confidentiality::public(),
            vec![],
            vec![],
            vec![SourceAnchor::new(path, digest.0, "Annual!C1")],
        );
        cell
    }

    #[test]
    fn synthetic_track_record_scores_exact_known_answer_and_keeps_missing_cost() {
        let r = synthetic().expect("report");
        assert_eq!(r.rows.len(), 2);
        let p = production(&r);
        assert_eq!(
            p.score.as_ref().expect("score").z,
            Some(Rat::new(3, 2).expect("rational"))
        );
        assert_eq!(p.score.as_ref().expect("score").side, Side::Above);
        assert!(matches!(
            p.outcome,
            OutcomeBinding::Resolved {
                comparative: false,
                ..
            }
        ));
        assert_eq!(
            r.summaries
                .iter()
                .map(|s| s.scores.n_members)
                .sum::<usize>(),
            2
        );
        assert_eq!(
            r.summaries
                .iter()
                .map(|s| s.scores.n_unresolved)
                .sum::<usize>(),
            1
        );
        assert!(r.to_json().contains("\"lane\":\"synthetic\""));
        assert_eq!(r.to_json(), synthetic().expect("rerun").to_json());
    }

    #[test]
    fn first_publication_is_not_replaced_by_later_acceptable_value() {
        let (p, t, c) = synthetic_evidence().expect("fixture");
        let later = revised(&c[0], 2030, "2031Q1", "110");
        let r = assess(&p, &t, &[later.clone(), c[0].clone()], None).expect("report");
        let row = production(&r);
        assert_eq!(row.score.expect("score").side, Side::Above);
        assert_eq!(row.later_outcomes.len(), 1);
        let without = assess(&p, &t, &[later], None).expect("missing original");
        assert!(
            matches!(&production(&without).outcome,OutcomeBinding::Unresolved{reason,..} if reason.contains("original registered release"))
        );
    }

    #[test]
    fn attributable_guidance_never_binds_to_full_or_unprinted_ownership() {
        let (p, t, mut c) = synthetic_evidence().expect("fixture");
        for (basis, code) in [
            (OwnershipBasis::Consolidated100, "FMINE-REF-105"),
            (OwnershipBasis::Unprinted, "FMINE-REF-107"),
        ] {
            c[0].key.ownership_basis = basis;
            let r = assess(&p, &t, &c, None).expect("named unresolved outcome");
            assert!(
                matches!(&production(&r).outcome,OutcomeBinding::Unresolved{code:c,..} if c==code)
            );
            assert!(production(&r).score.is_none());
        }
    }

    #[test]
    fn partial_year_disposal_and_withholding_preserve_all_members() {
        let (mut p, t, c) = synthetic_evidence().expect("fixture");
        for (kind, first, last) in [
            (StatusKind::Disposed, (2030, 12), None),
            (StatusKind::Disposed, (2031, 1), None),
            (StatusKind::Withheld, (2030, 1), Some((2030, 9))),
        ] {
            p.subject_statuses = vec![SubjectStatus {
                subject: "Example Mine".into(),
                kind,
                first,
                last,
                evidence: "synthetic status".into(),
            }];
            let r = assess(&p, &t, &c, None).expect("unresolved annual perimeter");
            assert_eq!(r.rows.len(), 2);
            assert!(r.rows.iter().all(|r| r.score.is_none()));
            assert_eq!(
                r.summaries
                    .iter()
                    .map(|s| s.scores.n_members)
                    .sum::<usize>(),
                2
            );
        }
    }

    #[test]
    fn comparative_members_keep_denominator_when_outcomes_disappear() {
        let (mut p, mut t, c) = synthetic_evidence().expect("fixture");
        t.agreement
            .accepted
            .retain(|a| a.quantity.ends_with("production_koz"));
        let mut doc = t.documents[0].clone();
        doc.file = "synthetic-guidance-2031.txt".into();
        doc.published_at = "2031-01-15".into();
        let mut row = t.agreement.accepted[0].clone();
        row.locator.doc = doc.file.clone();
        row.period = "2031FY".into();
        t.documents.push(doc);
        t.agreement.accepted.push(row);
        p.sources[0].quarter = "2032Q4".into();
        let mut actuals = vec![
            revised(&c[0], 2030, "2032Q4", "110"),
            revised(&c[0], 2031, "2032Q4", "110"),
        ];
        let before = assess(&p, &t, &actuals, None).expect("two comparative outcomes");
        assert_eq!(before.summaries.len(), 1);
        assert!(before.summaries[0].comparative);
        assert_eq!(before.summaries[0].scores.n_members, 2);
        actuals[1].claim.value = PublicValue::Nil {
            token: "—".into(),
            reason: fmine_public::key::NilReason::Unknown,
        };
        let after = assess(&p, &t, &actuals, None).expect("one missing comparative");
        let s = &after.summaries[0];
        assert_eq!(after.summaries.len(), 1);
        assert_eq!(
            (
                s.scores.n_members,
                s.scores.n_resolved,
                s.scores.n_unresolved
            ),
            (2, 1, 1)
        );
        assert_eq!(
            s.scores.whole_cohort_hit_bounds.expect("bound").lo,
            Rat::new(1, 2).expect("half")
        );
        assert_eq!(
            s.scores.whole_cohort_hit_bounds.expect("bound").hi,
            Rat::from_integer(1)
        );
        assert!(after.rows.iter().all(|r| r.comparative));
    }

    #[test]
    fn copper_uses_explicit_kt_and_rejects_unit_relabeling() {
        let (p, mut t, mut c) = synthetic_evidence().expect("fixture");
        t.agreement
            .accepted
            .retain(|a| a.quantity.ends_with("production_koz"));
        t.agreement.accepted[0].quantity = "guidance_example_mine_production_kt".into();
        t.agreement.accepted[0].unit = "kt".into();
        c[0].key.line.label = "summary:cu_prod_attr_kt".into();
        c[0].key.unit = "kt".into();
        let good = assess(&p, &t, &c, None).expect("copper source");
        assert_eq!(
            production(&good).score.expect("score").z,
            Some(Rat::new(3, 2).expect("z"))
        );
        c[0].key.unit = "Mlb".into();
        assert!(
            matches!(&production(&assess(&p,&t,&c,None).expect("unresolved unit")).outcome,OutcomeBinding::Unresolved{code,..} if code=="FMINE-REF-105")
        );
    }

    #[test]
    fn late_guidance_unknown_metrics_and_incomplete_readings_are_not_scored() {
        let (p, mut t, c) = synthetic_evidence().expect("fixture");
        t.documents[0].published_at = "2031-02-01".into();
        let late = assess(&p, &t, &c, None).expect("late rows retained");
        assert!(late.rows.iter().all(
            |r| matches!(&r.outcome,OutcomeBinding::Unresolved{code,..} if code=="FMINE-REF-108")
        ));
        t.documents[0].published_at = "2030-01-15".into();
        t.agreement.single_reader.push((
            t.agreement.accepted[0].locator.clone(),
            "reader-only".into(),
        ));
        assert_eq!(
            assess(&p, &t, &c, None).expect_err("missing reader").code,
            "FMINE-REF-202"
        );
        t.agreement.single_reader.clear();
        t.agreement.accepted[0].quantity = "guidance_example_mine_capex_usd_m".into();
        assert_eq!(
            assess(&p, &t, &c, None)
                .expect_err("unsupported metric")
                .code,
            "FMINE-REF-201"
        );
    }

    #[test]
    fn repeated_guidance_vintages_stay_separate_and_order_is_canonical() {
        let (p, mut t, c) = synthetic_evidence().expect("fixture");
        let mut doc = t.documents[0].clone();
        doc.file = "later-guidance.txt".into();
        doc.published_at = "2030-06-01".into();
        let mut row = t.agreement.accepted[0].clone();
        row.locator.doc = doc.file.clone();
        row.token = "120 - 130".into();
        row.value = parse_token(&row.token).expect("token");
        t.documents.push(doc);
        t.agreement.accepted.push(row);
        let before = assess(&p, &t, &c, None).expect("both vintages");
        let prod: Vec<_> = before
            .rows
            .iter()
            .filter(|r| r.metric == "production_koz")
            .collect();
        assert_eq!((prod[0].vintage, prod[1].vintage), (0, 1));
        assert_eq!(prod[0].score.expect("first").side, Side::Above);
        assert_eq!(prod[1].score.expect("later").side, Side::Inside);
        t.documents.reverse();
        t.agreement.accepted.reverse();
        assert_eq!(
            before.to_json(),
            assess(&p, &t, &c, None).expect("permutation").to_json()
        );
        t.documents[0].published_at = "2030-01-15".into();
        assert_eq!(
            assess(&p, &t, &c, None)
                .expect_err("ambiguous same-date vintages")
                .code,
            "FMINE-REF-104"
        );
    }

    #[test]
    fn point_range_and_rounding_boundary_have_no_invented_standardized_bias() {
        let (p, mut t, c) = synthetic_evidence().expect("fixture");
        let a = t
            .agreement
            .accepted
            .iter_mut()
            .find(|a| a.quantity.ends_with("production_koz"))
            .expect("row");
        a.token = "125 - 125".into();
        a.value = parse_token(&a.token).expect("token");
        let r = assess(&p, &t, &c, None).expect("point guidance");
        let score = production(&r).score.expect("score");
        assert!(score.boundary);
        assert!(score.z.is_none());
        assert_eq!(score.side, Side::Inside);
    }

    fn registered_workspace() -> std::path::PathBuf {
        use fmine_io::xlsx::{MinimalCell, MinimalSheet, build_minimal_xlsx};
        use std::sync::atomic::{AtomicU64, Ordering};
        static SERIAL: AtomicU64 = AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "fmine-track-record-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        for dir in [
            "corpus",
            "docs",
            "registries/sources",
            "registries/transcriptions",
        ] {
            std::fs::create_dir_all(root.join(dir)).expect("fixture dir");
        }
        let t = |at: &str, text: &str| (at.into(), MinimalCell::Text(text.into()));
        let n = |at: &str, value: f64| (at.into(), MinimalCell::Number(value));
        let workbook = build_minimal_xlsx(&[MinimalSheet {
            name: "YTD".into(),
            hidden: false,
            cells: vec![
                t("A1", "For the twelve months ended December 31"),
                n("B1", 2030.0),
                t("B2", "Gold produced (000s oz) 100% basis"),
                t("C2", "Gold produced (000s oz) attributable basis"),
                t("D2", "All-in sustaining costs ($/oz)"),
                t("A3", "Example Mine (80%)"),
                n("B3", 156.25),
                n("C3", 125.0),
                n("D3", 1150.0),
            ],
        }]);
        let pdf = b"%PDF-1.7 invented guidance, never a real issuer filing";
        std::fs::write(root.join("corpus/Annual.xlsx"), &workbook).expect("workbook");
        std::fs::write(root.join("docs/Guidance.pdf"), pdf).expect("document");
        let registry = format!(
            "schema = \"fmine.sources.v1\"\nissuer = \"synthetic\"\ncorpus_dir = \"corpus\"\nretrieved_utc = \"synthetic\"\nurl_prefix = \"https://example.invalid/\"\n\n[[source]]\nfile = \"Annual.xlsx\"\nkind = \"mine_stats\"\nquarter = \"2030Q4\"\npath = \"Annual.xlsx\"\nurl_basis = \"W1 table\"\nbytes = {}\nsha256 = \"{}\"\n\n[[subject]]\nname = \"Example Mine\"\n\n[[document]]\nfile = \"Guidance.pdf\"\ndir = \"docs\"\nkind = \"aif\"\nurl = \"https://example.invalid/Guidance.pdf\"\nurl_basis = \"W1 table\"\npublished_at = \"2030-01-15\"\npublished_at_basis = \"FACT: invented fixture declares 2030-01-15\"\npages = 1\nbytes = {}\nsha256 = \"{}\"\n",
            workbook.len(),
            sha256(&workbook).to_hex(),
            pdf.len(),
            sha256(pdf).to_hex()
        );
        std::fs::write(root.join("registries/sources/synthetic.toml"), registry)
            .expect("source registry");
        let mut transcript =
            "schema = \"fmine.transcriptions.v1\"\nissuer = \"synthetic\"\n".to_string();
        let doc_sha = sha256(pdf).to_hex();
        for (metric, token, unit, basis) in [
            ("production_koz", "100 - 120", "koz", "attributable:4/5"),
            (
                "aisc_usd_per_oz",
                "1000 - 1200",
                "usd_per_oz",
                "attributable:4/5;aisc=issuer-reported-per-sold-oz",
            ),
        ] {
            for reader in ["reader_a", "reader_b"] {
                writeln!(
                    transcript,
                    r#"
[[entry]]
doc = "Guidance.pdf"
doc_sha256 = "{doc_sha}"
page = 1
table = "Annual guidance"
row = "Example Mine"
col = "{metric}"
token = "{token}"
quantity = "guidance_example_mine_{metric}"
unit = "{unit}"
basis = "{basis}"
period = "2030FY"
reader = "{reader}"
"#
                )
                .expect("transcription");
            }
        }
        std::fs::write(
            root.join("registries/transcriptions/synthetic.toml"),
            transcript,
        )
        .expect("transcriptions");
        root
    }

    #[test]
    fn registered_input_path_checks_real_parsing_hashes_and_two_reader_agreement() {
        let root = registered_workspace();
        let r = run(&root, "synthetic", Some("example-mine"), Some("2030Q4"))
            .expect("registered inputs");
        assert_eq!(
            production(&r).score.expect("source score").z,
            Some(Rat::new(3, 2).expect("z"))
        );
        let OutcomeBinding::Resolved { cell, .. } = &production(&r).outcome else {
            panic!("resolved source")
        };
        assert_eq!(cell.locator, "Annual.xlsx:YTD!C3");
        assert_eq!(
            r.to_json(),
            run(&root, "synthetic", None, Some("2030Q4"))
                .expect("rerun")
                .to_json()
        );
        assert_ne!(
            r.digest,
            run(&root, "synthetic", None, None)
                .expect("unbounded")
                .digest
        );
    }

    #[test]
    fn registered_altered_and_missing_documents_refuse_without_fallback() {
        let root = registered_workspace();
        std::fs::write(root.join("docs/Guidance.pdf"), b"altered synthetic bytes")
            .expect("alter own fixture");
        let err = run(&root, "synthetic", None, None).expect_err("altered bytes");
        assert_eq!(err.code, "FMINE-REF-505");
        assert!(err.what.contains("Guidance.pdf"));
        std::fs::rename(
            root.join("docs/Guidance.pdf"),
            root.join("docs/Guidance.saved"),
        )
        .expect("preserve fixture under another name");
        let err = run(&root, "synthetic", None, None).expect_err("missing bytes");
        assert_eq!(err.code, "FMINE-REF-506");
        assert!(err.what.contains("Guidance.pdf"));
    }

    #[test]
    fn synthetic_arithmetic_never_overrides_explicit_negative_states() {
        let (p, t, mut c) = synthetic_evidence().expect("fixture");
        for state in [
            KnowledgeState::Known,
            KnowledgeState::Estimated,
            KnowledgeState::Unknown,
        ] {
            c[0].claim.state = state;
            let r = assess(&p, &t, &c, None).expect("allowed synthetic number");
            assert!(production(&r).score.is_some(), "{state:?}");
        }
        for state in [
            KnowledgeState::Conflicted,
            KnowledgeState::Stale,
            KnowledgeState::NotObservable,
            KnowledgeState::Redacted,
            KnowledgeState::Indeterminate,
            KnowledgeState::NotApplicable,
        ] {
            c[0].claim.state = state;
            let r = assess(&p, &t, &c, None).expect("negative state stays visible");
            assert!(production(&r).score.is_none(), "{state:?}");
            assert!(
                matches!(&production(&r).outcome,
                    OutcomeBinding::Unresolved { code, .. } if code == "FMINE-REF-202"),
                "{state:?}"
            );
        }
    }

    #[test]
    fn registered_aisc_uses_the_declared_sold_ounce_definition() {
        let root = registered_workspace();
        let r = run(&root, "synthetic", None, None).expect("registered AISC input");
        let row = r
            .rows
            .iter()
            .find(|r| r.metric == "aisc_usd_per_oz")
            .expect("AISC row");
        let score = row.score.expect("AISC score");
        assert_eq!(score.side, Side::Inside);
        assert_eq!(score.z, Some(Rat::new(1, 2).expect("half")));
        assert!(!score.boundary);
        let OutcomeBinding::Resolved { cell, comparative } = &row.outcome else {
            panic!("resolved AISC source")
        };
        assert!(!*comparative);
        assert_eq!(cell.locator, "Annual.xlsx:YTD!D3");
        assert_eq!(cell.key.unit, "usd_per_oz");
        assert_eq!(cell.claim.additivity, AdditivityClass::IntensiveWeighted);
    }
}
