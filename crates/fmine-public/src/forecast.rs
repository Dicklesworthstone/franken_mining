//! DESIGN (BD-067/BD-334): an unissued candidate from original, registered quarterly disclosures.
//! The information cut binds exact source bytes and their conservative availability; later comparatives
//! never fill an absent original. Source assertions and model exclusions remain visible beside the draft.
//! Determinism: Exact (frozen rules, source bytes and rational arithmetic).

use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;
use std::path::Path;

use fmine_calib::record::{
    self, Quarter, Rat, RecordRules, SiteStatus, Target, TargetMetric,
    candidate::{Candidate, CandidateSeries, build_candidate},
};
use fmine_core::codec::Encoder;
use fmine_core::hash::{Digest32, blake3_hash, sha256};
use fmine_evidence::{Refusal, RefusalCode};
use fmine_registry::sources::{SourcesRegistry, day_end, parse_sources_registry, retrieval_upper};

use crate::key::{
    self, KeyedCell, OwnershipBasis, Period, PeriodKind, PublicValue, Scope, SeriesOwnershipBasis,
};
use crate::mine_stats::{LabelVocabulary, MineStatsRead, View, read_mine_stats};
use crate::sources::{IssuerProfile, SourceKind, is_issuer_name, registry_path};
use crate::{LoadedSource, load_registered_sources};

type Result<T> = std::result::Result<T, Refusal>;
const MAX_TEXT_BYTES: u64 = 4 * 1024 * 1024;

/// Every selected original workbook, including those with unusable historical observations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CandidateSource {
    pub file: String,
    pub release: String,
    pub url: String,
    pub sha256: String,
    pub bytes: u64,
    /// Registry retrieval evidence, not an independently authenticated publication timestamp.
    pub retrieved_utc: String,
    /// The later of the retrieval upper bound and the end of any declared publication day.
    pub available_by_utc: String,
    pub published_at: Option<String>,
    pub published_at_basis: Option<String>,
}

/// A registered original-release publication boundary. The date is registry evidence, not a
/// timestamp authority or a claim that every public disclosure has been registered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NowcastPublication {
    pub file: String,
    pub sha256: String,
    pub published_at: String,
    pub published_at_basis: String,
    /// The start, not the end, of the stated publication day: same-day proofs cannot precede it.
    pub day_start_utc: i64,
}

/// Retain every registered workbook/HTML release for the scored quarter. Retrieval and inferred
/// publication dates cannot establish that an outcome was still unpublished. An empty result is
/// admissible for an unissued draft only; scoring additionally requires an original Mine Stats release.
pub fn nowcast_publications(
    registry: &SourcesRegistry,
    quarter: Quarter,
) -> Result<Vec<NowcastPublication>> {
    let release = quarter.to_string();
    let mut publications = Vec::new();
    let workbook_rows = registry
        .sources
        .iter()
        .filter(|source| source.quarter == release)
        .map(|source| {
            (
                &source.file,
                &source.sha256,
                source.published_at.as_deref(),
                source.published_at_basis.as_deref(),
            )
        });
    let html_rows = registry
        .html_sources
        .iter()
        .filter(|source| source.release == release)
        .map(|source| {
            (
                &source.file,
                &source.sha256,
                Some(source.published_at.as_str()),
                Some(source.published_at_basis.as_str()),
            )
        });
    for (file, sha256, day, basis) in workbook_rows.chain(html_rows) {
        let bad = || {
            refuse(
                RefusalCode::AssayLookahead108,
                format!("{file}: post-quarter nowcasts require a FACT-cited publication date"),
            )
        };
        let day = day.ok_or_else(bad)?;
        let basis = basis.filter(|basis| basis.starts_with("FACT: ")).ok_or_else(bad)?;
        let day_start_utc = record::parse_date(day)
            .and_then(|day| day.checked_mul(86_400))
            .filter(|day| quarter.end_utc().is_some_and(|end| *day >= end))
            .ok_or_else(bad)?;
        publications.push(NowcastPublication {
            file: file.clone(),
            sha256: sha256.clone(),
            published_at: day.into(),
            published_at_basis: basis.into(),
            day_start_utc,
        });
    }
    publications.sort_by(|a, b| a.file.cmp(&b.file));
    Ok(publications)
}

/// One selected field. A missing row keeps its file and digest; placeholders keep their canonical Claim.
#[derive(Debug, Clone)]
pub struct CandidateObservation {
    pub site: String,
    pub target: String,
    pub quarter: Quarter,
    pub role: &'static str,
    pub source: String,
    pub sha256: String,
    /// The exact printed number, including excluded zero/negative observations. Never stored Excel digits.
    pub value: Option<Rat>,
    pub reason: Option<String>,
    pub claim: Option<KeyedCell>,
}

/// A draft candidate and the evidence needed to review it. It is neither frozen nor externally time-proved.
#[derive(Debug, Clone)]
pub struct PublicCandidate {
    pub rules: RecordRules,
    pub candidate: Candidate,
    pub as_of: String,
    pub synthetic: bool,
    pub rules_digest: Digest32,
    pub rules_file_sha256: Digest32,
    pub registry_sha256: Digest32,
    pub sources: Vec<CandidateSource>,
    /// Empty on ordinary forecasts; registered release boundaries remain separate from model inputs.
    pub nowcast_publications: Vec<NowcastPublication>,
    pub observations: Vec<CandidateObservation>,
    pub digest: Digest32,
}

fn refuse(code: RefusalCode, detail: impl Into<String>) -> Refusal {
    Refusal::new(code, format!("forecast candidate: {}", detail.into()))
}

fn invalid(detail: impl Into<String>) -> Refusal {
    refuse(RefusalCode::UndeclaredUnitBasisOrTz107, detail)
}

fn source_error(detail: impl Into<String>) -> Refusal {
    refuse(RefusalCode::SourceCellUnusable208, detail)
}

fn utf8(bytes: &[u8]) -> Result<&str> {
    if bytes.len() as u64 > MAX_TEXT_BYTES {
        return Err(invalid("rules and registry text must fit the byte limit"));
    }
    std::str::from_utf8(bytes).map_err(|_| invalid("rules and registry text must be UTF-8"))
}

fn read_registry(path: &Path) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .and_then(|f| f.take(MAX_TEXT_BYTES + 1).read_to_end(&mut bytes))
        .map_err(|e| {
            refuse(
                RefusalCode::InputMissingOrUnreadable506,
                format!("{}: {e}", path.display()),
            )
        })?;
    utf8(&bytes)?;
    Ok(bytes)
}

fn after_quarter(q: Quarter) -> Result<i64> {
    q.end_utc()
        .ok_or_else(|| invalid("source quarter is outside the supported UTC calendar"))
}

fn expected_unit(target: &Target) -> Result<&'static str> {
    TargetMetric::from_target(target)
        .map(TargetMetric::unit)
        .ok_or_else(|| invalid(format!("unsupported field/outcome for {}", target.id)))
}

fn pool(rules: &RecordRules, target: &str) -> Result<(Quarter, Quarter, Quarter)> {
    rules
        .persistence_pool(target)
        .map_err(|e| invalid(e.to_string()))
}

struct Prepared {
    rules: RecordRules,
    profile: IssuerProfile,
    sources: Vec<CandidateSource>,
    nowcast_publications: Vec<NowcastPublication>,
    as_of: String,
    synthetic: bool,
    rules_digest: Digest32,
    rules_file_sha256: Digest32,
    registry_sha256: Digest32,
}

fn prepare(rules_bytes: &[u8], registry_bytes: &[u8], as_of: &str) -> Result<Prepared> {
    let rules_text = utf8(rules_bytes)?;
    if rules_text.lines().any(record::has_open_marker) {
        return Err(invalid("candidate rules contain an unresolved OPEN marker"));
    }
    let rules = record::parse_rules(rules_text).map_err(|e| invalid(e.to_string()))?;
    let cut = record::parse_utc(as_of)
        .ok_or_else(|| invalid("information cut requires YYYY-MM-DDTHH:MM:SSZ"))?;
    let outcome = rules
        .outcome_quarter()
        .map_err(|e| invalid(e.to_string()))?;
    if rules.is_post_quarter_nowcast() {
        if cut < after_quarter(outcome)? || cut >= rules.dates.proof_by {
            return Err(refuse(
                RefusalCode::LateRegistration206,
                "post-quarter nowcast cuts must follow quarter end and precede the frozen proof deadline",
            ));
        }
    } else if rules.targets.iter().any(|t| t.outcome.is_quarterly())
        && cut >= after_quarter(outcome)?
    {
        return Err(refuse(
            RefusalCode::LateRegistration206,
            "quarterly candidates require an information cut before quarter end; declare quarter_qtd_nowcast in new rules for a post-quarter record",
        ));
    }
    let registry =
        parse_sources_registry(utf8(registry_bytes)?).map_err(|e| invalid(e.to_string()))?;
    if registry.issuer != rules.issuer {
        return Err(invalid("source registry issuer differs from the rules"));
    }
    let nowcast_publications = if rules.is_post_quarter_nowcast() {
        let publications = nowcast_publications(&registry, outcome)?;
        if publications.iter().any(|source| cut >= source.day_start_utc) {
            return Err(refuse(
                RefusalCode::AssayLookahead108,
                "post-quarter nowcast information cut reaches a registered original publication day",
            ));
        }
        publications
    } else {
        Vec::new()
    };
    let mut wanted = BTreeSet::new();
    for target in &rules.targets {
        expected_unit(target)?;
        let (first, last, point) = pool(&rules, &target.id)?;
        wanted.extend(first.through(last));
        wanted.insert(point);
    }
    let mut sources = Vec::new();
    for q in &wanted {
        let mut matches = registry
            .sources
            .iter()
            .filter(|s| s.kind == "mine_stats" && s.quarter == q.to_string());
        let (Some(source), None) = (matches.next(), matches.next()) else {
            return Err(refuse(
                RefusalCode::NoClaimInDomain201,
                format!("{q} needs exactly one registered original Mine Stats release"),
            ));
        };
        let complete = after_quarter(*q)?;
        let retrieved_utc = source
            .retrieved_utc
            .as_deref()
            .unwrap_or(&registry.retrieved_utc);
        let retrieved = retrieval_upper(retrieved_utc)?;
        let publication = source.published_at.as_deref().map(day_end).transpose()?;
        if retrieved < complete || publication.is_some_and(|t| t < complete) {
            return Err(refuse(
                RefusalCode::AssayLookahead108,
                format!(
                    "{} claims completed-quarter data before {q} ended",
                    source.file
                ),
            ));
        }
        let available = publication.map_or(retrieved, |p| p.max(retrieved));
        if available > cut {
            return Err(refuse(
                RefusalCode::AssayLookahead108,
                format!(
                    "{} was not available by the explicit information cut",
                    source.file
                ),
            ));
        }
        sources.push(CandidateSource {
            file: source.file.clone(),
            release: source.quarter.clone(),
            url: source.url.clone(),
            sha256: source.sha256.clone(),
            bytes: source.bytes,
            retrieved_utc: retrieved_utc.to_string(),
            available_by_utc: record::format_utc(available),
            published_at: source.published_at.clone(),
            published_at_basis: source.published_at_basis.clone(),
        });
    }
    let synthetic = rules.issuer == crate::synthetic::SYNTHETIC_ISSUER;
    let mut profile = IssuerProfile::from_registry(registry).map_err(invalid)?;
    profile.sources.retain(|s| {
        s.kind == SourceKind::MineStats
            && Quarter::parse(&s.quarter).is_some_and(|q| wanted.contains(&q))
    });
    profile.sources.sort_by(|a, b| a.quarter.cmp(&b.quarter));
    Ok(Prepared {
        rules,
        profile,
        sources,
        nowcast_publications,
        as_of: as_of.into(),
        synthetic,
        rules_digest: record::rules_digest(rules_bytes),
        rules_file_sha256: sha256(rules_bytes),
        registry_sha256: sha256(registry_bytes),
    })
}

fn per_ounce_withholding_note(source: &LoadedSource, site: &str, year: u16) -> Option<String> {
    const MONTHS: &str =
        "january february march april may june july august september october november december";
    let prefix = format!("as a result of temporary suspension of operations at {site} starting ")
        .to_ascii_lowercase();
    let suffix = format!(", {year}, no per ounce data is provided");
    let mut note = None;
    for sheet in &source.workbook.sheets {
        for (&(row, col), cell) in &sheet.cells_by_coord {
            let text = crate::aisc::cell_text(sheet, row, col)?.to_ascii_lowercase();
            let text = text.trim_start_matches(|c: char| c.is_ascii_digit() || c.is_whitespace());
            if let Some(date) = text.strip_prefix(&prefix) {
                if note.is_some() || !matches!(cell.value, fmine_io::xlsx::CellValue::Text(_)) {
                    return None;
                }
                let (month, day) = date.strip_suffix(&suffix)?.split_once(' ')?;
                let month = MONTHS.split_whitespace().position(|m| m == month)? + 1;
                let day = day.parse::<u8>().ok().filter(|_| !day.starts_with('+'))?;
                record::parse_utc(&format!("{year:04}-{month:02}-{day:02}T00:00:00Z"))?;
                note = Some(format!(
                    "{}:{}!{}",
                    source.file_name, sheet.name, cell.cell_ref
                ));
            }
        }
    }
    note
}

fn observation(
    profile: &IssuerProfile,
    source: &LoadedSource,
    read: &MineStatsRead,
    site: &str,
    target: &Target,
    quarter: Quarter,
    role: &'static str,
) -> Result<CandidateObservation> {
    let period = Period {
        year: u16::try_from(quarter.year)
            .map_err(|_| invalid("source year is outside the supported calendar"))?,
        kind: PeriodKind::Quarter(quarter.q),
    };
    let mut hits = read.values.iter().filter(|v| {
        let subject = profile
            .shares
            .iter()
            .find(|s| s.label == v.site)
            .map_or(v.site.as_str(), |s| s.subject.as_str());
        v.view == View::Summary
            && v.sheet == "QTD"
            && v.months == 3
            && v.period == period.code()
            && subject == site
            && v.label == target.field
    });
    let first = hits.next();
    if hits.next().is_some() {
        return Err(source_error(format!(
            "{} {site} {} has ambiguous original-quarter cells",
            source.file_name, target.id
        )));
    }
    let mut row = CandidateObservation {
        site: site.into(),
        target: target.id.clone(),
        quarter,
        role,
        source: source.file_name.clone(),
        sha256: Digest32(source.workbook.sha256).to_hex(),
        value: None,
        reason: Some("original_release_has_no_matching_row".into()),
        claim: None,
    };
    let Some(raw) = first else {
        return Ok(row);
    };
    let claim = key::from_mine_stats(profile, &source.workbook, &source.quarter, raw)
        .map_err(|e| source_error(e.to_string()))?;
    let locator = &claim.locator;
    if claim.key.issuer != profile.issuer
        || claim.key.subject != site
        || claim.key.valid_period != period
        || claim.key.reporting_vintage.release != quarter.to_string()
        || claim.key.reporting_vintage.file_sha256 != row.sha256
        || claim.key.scope != Scope::Site
        || claim.key.unit != expected_unit(target)?
        || claim.key.line.label != format!("summary:{}", target.field)
        || claim.key.line.occurrence != 0
        || claim.key.ownership_basis == OwnershipBasis::Unprinted
        || (TargetMetric::from_target(target).is_some_and(TargetMetric::is_production)
            && claim.key.ownership_basis != OwnershipBasis::Consolidated100)
    {
        return Err(refuse(
            RefusalCode::BasisMismatch105,
            format!("{locator} has incompatible target, scope, unit or ownership"),
        ));
    }
    let displayed = source
        .workbook
        .sheet(&raw.sheet)
        .and_then(|s| s.cells_by_coord.values().find(|c| c.cell_ref == raw.cell))
        .and_then(|c| c.displayed());
    let signed_point = role == "point"
        && matches!(
            TargetMetric::from_target(target),
            Some(
                TargetMetric::GoldAisc
                    | TargetMetric::GoldTcc
                    | TargetMetric::CopperC1
                    | TargetMetric::CopperAisc
            )
        );
    let (value, mut reason) = match displayed {
        Some(Ok(display)) => match Rat::from_decimal_str(&display.text()) {
            Ok(value) => (
                Some(value),
                (value.signum() <= 0 && !signed_point)
                    .then(|| "non_positive_printed_number_excluded_by_model".to_string()),
            ),
            Err(_) if matches!(display.text().trim(), "" | "-" | "–" | "—" | "n/a" | "N/A") => {
                (None, Some("printed_placeholder".into()))
            }
            Err(_) => {
                return Err(source_error(format!(
                    "{locator} display is not an exact number"
                )));
            }
        },
        Some(Err(e)) => {
            return Err(source_error(format!(
                "{locator} display refused: {}",
                e.code
            )));
        }
        None => (
            None,
            Some(format!(
                "source_{}",
                key::mine_stats::value_kind(&claim.claim.value)
            )),
        ),
    };
    if let Some(value) = value {
        let (first, last) = period.months();
        let mut overlaps_status = false;
        let mut only_withheld = true;
        for status in profile
            .subject_statuses
            .iter()
            .filter(|s| s.subject == site)
        {
            // from_date records the first FULL month after a disposal. Include the closing month
            // when checking quarterly support, so an unflagged partial-quarter stub is not forecast.
            let start = match (status.kind, status.first) {
                (key::StatusKind::Disposed, (year, 1)) => (
                    year.checked_sub(1)
                        .ok_or_else(|| invalid("invalid disposal year"))?,
                    12,
                ),
                (key::StatusKind::Disposed, (year, month)) => (
                    year,
                    month
                        .checked_sub(1)
                        .ok_or_else(|| invalid("invalid disposal month"))?,
                ),
                (_, start) => start,
            };
            let overlaps = start <= last && status.last.is_none_or(|end| end >= first);
            overlaps_status |= overlaps;
            only_withheld &= !overlaps || status.kind == key::StatusKind::Withheld;
        }
        let flagged = match &claim.claim.value {
            PublicValue::Number { under_status, .. } => *under_status,
            _ => None,
        };
        // The original historical production Claim keeps its flag and the source's per-ounce limitation.
        let note = (role == "history"
            && TargetMetric::from_target(target) == Some(TargetMetric::GoldProduction)
            && value.signum() > 0
            && only_withheld
            && flagged == Some(key::StatusKind::Withheld))
        .then(|| per_ounce_withholding_note(source, site, period.year))
        .flatten();
        // A zero cost point is usable only as an unflagged source number. In particular, a status-derived
        // Nil must not become a zero forecast just because its stored cell happens to contain zero.
        if (((value.signum() != 0 || signed_point) && overlaps_status)
            || flagged.is_some()
            || (signed_point && !matches!(&claim.claim.value, PublicValue::Number { .. })))
            && note.is_none()
        {
            return Err(refuse(
                RefusalCode::BasisMismatch105,
                format!("{locator} numeric observation overlaps a registered site status"),
            ));
        }
        if let Some(note) = note {
            reason = Some(format!(
                "included_production_with_per_ounce_only_note:{note}"
            ));
        }
    }
    row.value = value;
    row.reason = reason;
    row.claim = Some(claim);
    Ok(row)
}

fn finish(
    prepared: Prepared,
    sources: Vec<LoadedSource>,
    decimal_places: u32,
) -> Result<PublicCandidate> {
    let mut reads = BTreeMap::new();
    let vocabulary: LabelVocabulary = prepared.profile.mine_stats_labels.iter().cloned().collect();
    for source in &sources {
        let read = read_mine_stats(
            &source.workbook,
            &source.quarter,
            (!vocabulary.is_empty()).then_some(&vocabulary),
        )?;
        if !read.refusals.is_empty() {
            return Err(source_error(format!(
                "{} has refused source cells: {}",
                source.file_name,
                read.refusals
                    .iter()
                    .map(|r| format!("{}: {}", r.locator, r.reason))
                    .collect::<Vec<_>>()
                    .join("; ")
            )));
        }
        if reads
            .insert(source.quarter.clone(), (source, read))
            .is_some()
        {
            return Err(source_error("duplicate loaded original release"));
        }
    }
    let mut observations = Vec::new();
    let mut series = Vec::new();
    for site in prepared
        .rules
        .sites
        .iter()
        .filter(|s| s.status == SiteStatus::Live)
    {
        for target in &prepared.rules.targets {
            let (first, last, point) = pool(&prepared.rules, &target.id)?;
            let start = observations.len();
            let mut history = Vec::new();
            for (quarter, role) in first
                .through(last)
                .into_iter()
                .map(|q| (q, "history"))
                .chain([(point, "point")])
            {
                let (source, read) = reads.get(&quarter.to_string()).ok_or_else(|| {
                    source_error(format!("{quarter} original release was not loaded"))
                })?;
                let row = observation(
                    &prepared.profile,
                    source,
                    read,
                    &site.name,
                    target,
                    quarter,
                    role,
                )?;
                if role == "history" {
                    history.push(row.value);
                }
                observations.push(row);
            }
            let point_row = observations
                .last()
                .ok_or_else(|| source_error("missing point observation"))?;
            if TargetMetric::from_target(target).is_some_and(|m| !m.is_production()) {
                let mut bases = BTreeSet::new();
                for row in &observations[start..] {
                    if row
                        .value
                        .is_some_and(|x| x.signum() > 0 || row.role == "point")
                        && let Some(claim) = &row.claim
                    {
                        bases.insert(SeriesOwnershipBasis::from(claim.key.ownership_basis));
                    }
                }
                if bases.len() > 1 {
                    return Err(refuse(
                        RefusalCode::BasisMismatch105,
                        format!(
                            "{} {} has changing ownership across the historical and point values",
                            site.name, target.id
                        ),
                    ));
                }
            }
            series.push(CandidateSeries {
                site: site.name.clone(),
                target: target.id.clone(),
                history,
                point: point_row.value,
            });
        }
    }
    let candidate = build_candidate(&prepared.rules, &series, decimal_places)
        .map_err(|e| source_error(e.to_string()))?;
    let mut enc = Encoder::new_frame(*b"FCAN", 1);
    enc.put_fixed(&prepared.rules_digest.0);
    enc.put_fixed(&prepared.rules_file_sha256.0);
    enc.put_fixed(&prepared.registry_sha256.0);
    enc.put_str(&prepared.as_of);
    enc.put_u8(u8::from(prepared.synthetic));
    enc.put_str(candidate.model_id);
    enc.put_u64(u64::from(decimal_places));
    enc.put_str(&candidate.csv);
    enc.put_fixed(&key::claims_digest(observations.iter().filter_map(|o| o.claim.as_ref())).0);
    enc.put_u64(observations.len() as u64);
    for row in &observations {
        for value in [
            &row.site,
            &row.target,
            &row.quarter.to_string(),
            row.role,
            &row.source,
            &row.sha256,
        ] {
            enc.put_str(value);
        }
        enc.put_str(row.reason.as_deref().unwrap_or(""));
        enc.put_u8(u8::from(row.value.is_some()));
        if let Some(value) = row.value {
            enc.put_str(&format!("{}/{}", value.num(), value.den()));
        }
    }
    let digest = blake3_hash(&enc.finish());
    Ok(PublicCandidate {
        rules: prepared.rules,
        candidate,
        as_of: prepared.as_of,
        synthetic: prepared.synthetic,
        rules_digest: prepared.rules_digest,
        rules_file_sha256: prepared.rules_file_sha256,
        registry_sha256: prepared.registry_sha256,
        sources: prepared.sources,
        nowcast_publications: prepared.nowcast_publications,
        observations,
        digest,
    })
}

/// Build a complete unissued candidate at an explicit information cut. Only the original Mine Stats
/// releases required by the declared pools and Q3 point are opened. Missing or ambiguous source releases,
/// unreadable or altered bytes, look-ahead, unprinted/changing cost bases and unsafe statuses refuse.
/// Historical missing/nonpositive values remain in the audit; every live point must be usable.
pub fn build(
    root: &Path,
    rules_bytes: &[u8],
    as_of: &str,
    decimal_places: u32,
) -> Result<PublicCandidate> {
    let rules = record::parse_rules(utf8(rules_bytes)?).map_err(|e| invalid(e.to_string()))?;
    if !is_issuer_name(&rules.issuer) {
        return Err(invalid("rules issuer is not a safe registered issuer name"));
    }
    let registry = read_registry(&registry_path(root, &rules.issuer))?;
    let prepared = prepare(rules_bytes, &registry, as_of)?;
    let sources = load_registered_sources(&prepared.profile, root)?;
    finish(prepared, sources, decimal_places)
}

#[cfg(test)]
mod tests;
