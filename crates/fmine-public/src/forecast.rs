//! DESIGN (BD-067/BD-334): an unissued candidate from original, registered quarterly disclosures.
//! The information cut binds exact source bytes and their conservative availability; later comparatives
//! never fill an absent original. Source assertions and model exclusions remain visible beside the draft.
//! Determinism: Exact (frozen rules, source bytes and rational arithmetic).

use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;
use std::path::Path;

use fmine_calib::record::{
    self, BaselineKind, Outcome, Quarter, Rat, RecordRules, SiteStatus, Target,
    candidate::{Candidate, CandidateSeries, build_candidate},
};
use fmine_core::codec::Encoder;
use fmine_core::hash::{Digest32, blake3_hash, sha256};
use fmine_evidence::{Refusal, RefusalCode};
use fmine_registry::sources::{day_end, parse_sources_registry, retrieval_upper};

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
    let next = q.next();
    record::parse_utc(&format!(
        "{:04}-{:02}-01T00:00:00Z",
        next.year,
        3 * (next.q - 1) + 1
    ))
    .ok_or_else(|| invalid("source quarter is outside the supported UTC calendar"))
}

fn expected_unit(target: &Target) -> Result<&'static str> {
    match (target.field.as_str(), target.outcome) {
        ("au_prod_100_koz", Outcome::FyMinus9m) => Ok("koz"),
        ("aisc_usd_per_oz", Outcome::Q4Qtd) => Ok("usd_per_oz"),
        _ => Err(invalid(format!(
            "unsupported field/outcome combination for {}",
            target.id
        ))),
    }
}

fn pool(rules: &RecordRules, target: &str) -> Result<(Quarter, Quarter, Quarter)> {
    let matches: Vec<_> = rules
        .baselines
        .iter()
        .filter(|b| b.target == target)
        .filter_map(|b| match b.kind {
            BaselineKind::Persistence {
                pool_from,
                pool_through,
                point,
            } => Some((pool_from, pool_through, point)),
            BaselineKind::IssuerGuidance => None,
        })
        .collect();
    let [value] = matches.as_slice() else {
        return Err(invalid(format!(
            "{target} needs exactly one declared persistence pool"
        )));
    };
    Ok(*value)
}

struct Prepared {
    rules: RecordRules,
    profile: IssuerProfile,
    sources: Vec<CandidateSource>,
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
    let registry =
        parse_sources_registry(utf8(registry_bytes)?).map_err(|e| invalid(e.to_string()))?;
    if registry.issuer != rules.issuer {
        return Err(invalid("source registry issuer differs from the rules"));
    }
    let mut wanted = BTreeSet::new();
    let mut points = BTreeSet::new();
    for target in &rules.targets {
        expected_unit(target)?;
        let (first, last, point) = pool(&rules, &target.id)?;
        if first >= last || last >= point || point.q != 3 {
            return Err(invalid(
                "Q4 candidates require an earlier pool of consecutive quarters and a Q3 point",
            ));
        }
        points.insert(point);
        wanted.extend(first.through(last));
        wanted.insert(point);
    }
    if points.len() != 1 {
        return Err(invalid("all targets must name the same Q3 point year"));
    }
    let mut sources = Vec::new();
    for q in &wanted {
        let matches: Vec<_> = registry
            .sources
            .iter()
            .filter(|s| s.kind == "mine_stats" && s.quarter == q.to_string())
            .collect();
        let [source] = matches.as_slice() else {
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
        as_of: as_of.into(),
        synthetic,
        rules_digest: record::rules_digest(rules_bytes),
        rules_file_sha256: sha256(rules_bytes),
        registry_sha256: sha256(registry_bytes),
    })
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
        || (target.outcome == Outcome::FyMinus9m
            && claim.key.ownership_basis != OwnershipBasis::Consolidated100)
    {
        return Err(refuse(
            RefusalCode::BasisMismatch105,
            format!(
                "{} has incompatible target, scope, unit or ownership",
                claim.locator
            ),
        ));
    }
    let displayed = source
        .workbook
        .sheet(&raw.sheet)
        .and_then(|s| s.cells_by_coord.values().find(|c| c.cell_ref == raw.cell))
        .and_then(|c| c.displayed());
    let (value, reason) = match displayed {
        Some(Ok(display)) => match Rat::from_decimal_str(&display.text()) {
            Ok(value) => (
                Some(value),
                (value.signum() <= 0)
                    .then(|| "non_positive_printed_number_excluded_by_model".to_string()),
            ),
            Err(_) if matches!(display.text().trim(), "" | "-" | "–" | "—" | "n/a" | "N/A") => {
                (None, Some("printed_placeholder".into()))
            }
            Err(_) => {
                return Err(source_error(format!(
                    "{} display is not an exact number",
                    claim.locator
                )));
            }
        },
        Some(Err(e)) => {
            return Err(source_error(format!(
                "{} display refused: {}",
                claim.locator, e.code
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
        for status in profile
            .subject_statuses
            .iter()
            .filter(|s| s.subject == site)
        {
            // from_date records the first FULL month after a disposal. Include the closing month
            // when checking quarterly support, so an unflagged partial-quarter stub is not forecast.
            let start = if status.kind == key::StatusKind::Disposed {
                let (year, month) = status.first;
                if month == 1 {
                    (
                        year.checked_sub(1)
                            .ok_or_else(|| invalid("invalid disposal year"))?,
                        12,
                    )
                } else {
                    (
                        year,
                        month
                            .checked_sub(1)
                            .ok_or_else(|| invalid("invalid disposal month"))?,
                    )
                }
            } else {
                status.first
            };
            overlaps_status |= start <= last && status.last.is_none_or(|end| end >= first);
        }
        if value.signum() != 0 && overlaps_status {
            return Err(refuse(
                RefusalCode::BasisMismatch105,
                format!(
                    "{} numeric observation overlaps a registered site status",
                    claim.locator
                ),
            ));
        }
        if matches!(
            claim.claim.value,
            PublicValue::Number {
                under_status: Some(_),
                ..
            }
        ) {
            return Err(refuse(
                RefusalCode::BasisMismatch105,
                format!("{} has a flagged site status", claim.locator),
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
            if target.outcome == Outcome::Q4Qtd {
                let mut bases = BTreeSet::new();
                for row in &observations[start..] {
                    if row.value.is_some_and(|x| x.signum() > 0)
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
