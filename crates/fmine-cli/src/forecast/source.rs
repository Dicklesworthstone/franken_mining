//! Source-bound resolution of a previously frozen public forecast target.
//! Determinism: Exact (displayed decimal, source Claim and registered publication date).
//!
//! DESIGN: the policy fixes every semantic selector before an outcome. Original resolution never
//! falls back to a comparative. Explicit later-vintage resolution authenticates the same frozen
//! target, without changing its period or basis. The journal owner checks predecessor and clock
//! policy before admitting a revision; a comparative is not proof of a formal issuer restatement.

use std::io::Read as _;
use std::path::Path;

use fmine_calib::ledger::OutcomeCitation;
use fmine_calib::ledger::binding::{
    PublicMetric, PublicOwnership, PublicPeriod, PublicResolutionPolicy,
};
use fmine_claim::{AdditivityClass, Claim, Confidentiality, Representation, SpatialSupport};
use fmine_core::ObjectId;
use fmine_core::anchor::SourceAnchor;
use fmine_core::hash::{Digest32, sha256};
use fmine_evidence::{EpistemicStatus, EvidenceClass, KnowledgeState, Refusal, RefusalCode};
use fmine_io::xlsx::Workbook;
use fmine_numerics::exact_feas::Rat;
use fmine_public::key::{
    KeyedCell, LineKey, OwnershipBasis, PublicKey, PublicValue, Scope, ShareSource, Vintage,
    canonical_value_tag, parse_period_code,
};
use fmine_public::mine_stats::{View, read_mine_stats};
use fmine_public::sources::{IssuerProfile, SourceKind, is_issuer_name, registry_path};
use fmine_registry::sources::{SourceRow, parse_sources_registry};
use fmine_time::{CivilDate, NS_PER_DAY};

type Result<T> = std::result::Result<T, Refusal>;

/// Immutable admission evidence. A saved journal operation retains these facts rather than claiming
/// that replay has authenticated the current contents of a source file.
#[derive(Debug, Clone)]
pub(super) struct ResolvedPublicOutcome {
    pub source: KeyedCell,
    pub value: Rat,
    pub stored: String,
    pub displayed: String,
    pub ownership_source: &'static str,
    pub citation: OutcomeCitation,
    /// Conservative date-only availability: midnight immediately following the publication date.
    pub available_at_ns: i64,
    pub published_at: String,
    pub published_at_basis: String,
}

fn refusal(code: RefusalCode, reason: impl Into<String>) -> Refusal {
    Refusal::new(
        code,
        format!("registered forecast outcome: {}", reason.into()),
    )
}

fn public_policy(policy: &PublicResolutionPolicy) -> Result<()> {
    policy.validate()?;
    if !is_issuer_name(&policy.issuer) || policy.issuer == fmine_public::synthetic::SYNTHETIC_ISSUER
    {
        return Err(refusal(
            RefusalCode::NoClaimInDomain201,
            "a registered public resolution needs a public issuer registry",
        ));
    }
    Ok(())
}

fn bounded_snapshot(path: &Path, limit: u64) -> Result<Vec<u8>> {
    let failure = |error| {
        refusal(
            RefusalCode::InputMissingOrUnreadable506,
            format!("{}: {error}", path.display()),
        )
    };
    let file = std::fs::File::open(path).map_err(failure)?;
    let metadata = file.metadata().map_err(failure)?;
    if !metadata.is_file() || metadata.len() > limit {
        return Err(refusal(
            RefusalCode::InputMissingOrUnreadable506,
            format!(
                "{} must be a regular file within {limit} bytes",
                path.display()
            ),
        ));
    }
    let mut bytes = Vec::new();
    file.take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(failure)?;
    if bytes.len() as u64 > limit {
        return Err(refusal(
            RefusalCode::InputMissingOrUnreadable506,
            format!("{} grew beyond {limit} bytes", path.display()),
        ));
    }
    Ok(bytes)
}

/// Only an original or later calendar-quarter release can report the complete frozen period.
/// This does not assert that the release contains the target, or that its figure was formally restated.
pub(super) fn release_period(
    policy: &PublicResolutionPolicy,
    release: &str,
) -> Result<PublicPeriod> {
    let period = PublicPeriod::parse(release)?;
    if !matches!(period, PublicPeriod::Quarter { .. })
        || period.end_ns()? < policy.period.end_ns()?
    {
        return Err(refusal(
            RefusalCode::SupportMismatch102,
            "outcome release must be the original calendar quarter or a later quarter",
        ));
    }
    Ok(period)
}

/// Authenticate only the original workbook. A later registered release is never a fallback.
pub(super) fn resolve(
    root: &Path,
    policy: &PublicResolutionPolicy,
    forecast_id: ObjectId,
) -> Result<ResolvedPublicOutcome> {
    resolve_release(root, policy, forecast_id, &policy.period.original_release())
}

/// Authenticate exactly one requested release while keeping every target selector frozen.
/// The live journal must additionally require an admitted predecessor for a later release.
pub(super) fn resolve_release(
    root: &Path,
    policy: &PublicResolutionPolicy,
    forecast_id: ObjectId,
    release: &str,
) -> Result<ResolvedPublicOutcome> {
    public_policy(policy)?;
    release_period(policy, release)?;
    if !policy.members.contains_key(&forecast_id) {
        return Err(refusal(
            RefusalCode::NoClaimInDomain201,
            "forecast is not in the frozen public target mapping",
        ));
    }
    let path = registry_path(root, &policy.issuer);
    let registry_bytes = bounded_snapshot(&path, 4 * 1024 * 1024)?;
    let registry_text = std::str::from_utf8(&registry_bytes).map_err(|error| {
        refusal(
            RefusalCode::InputMissingOrUnreadable506,
            format!("{}: {error}", path.display()),
        )
    })?;
    let registry = parse_sources_registry(registry_text).map_err(|error| {
        refusal(
            RefusalCode::InputMissingOrUnreadable506,
            format!("{}: {error}", path.display()),
        )
    })?;
    if registry.issuer != policy.issuer {
        return Err(refusal(
            RefusalCode::BasisMismatch105,
            "source registry issuer differs from the frozen policy",
        ));
    }
    let mut candidates = registry
        .sources
        .iter()
        .filter(|row| row.kind == "mine_stats" && row.quarter == release);
    let source = candidates.next().ok_or_else(|| {
        refusal(
            RefusalCode::NoClaimInDomain201,
            format!("selected {release} Mine Stats workbook is not registered"),
        )
    })?;
    if candidates.next().is_some() {
        return Err(refusal(
            RefusalCode::NonIdentifiableAttribution104,
            "more than one workbook claims the selected outcome release",
        ));
    }
    let source = source.clone();
    publication(&source, policy)?;
    let mut profile = IssuerProfile::from_registry(registry)
        .map_err(|error| refusal(RefusalCode::InputMissingOrUnreadable506, error))?;
    // Do not require unrelated documents, and never let one supply a fallback.
    profile.sources.retain(|row| {
        row.kind == SourceKind::MineStats && row.quarter == release && row.file_name == source.file
    });
    let artifact = format!("{}/{}", profile.corpus_dir, source.file);
    let bytes = bounded_snapshot(&root.join(&artifact), 64 * 1024 * 1024)?;
    if bytes.len() as u64 != source.bytes || sha256(&bytes).to_hex() != source.sha256 {
        return Err(refusal(
            RefusalCode::DigestMismatch505,
            format!(
                "{} differs from its registered byte count or SHA-256",
                source.file
            ),
        ));
    }
    let workbook = Workbook::from_bytes(artifact, &bytes).map_err(|error| {
        refusal(
            RefusalCode::InputMissingOrUnreadable506,
            format!("{}: {error}", source.file),
        )
    })?;
    let vocabulary = profile.mine_stats_labels.iter().cloned().collect();
    let read = read_mine_stats(
        &workbook,
        release,
        (!profile.mine_stats_labels.is_empty()).then_some(&vocabulary),
    )?;
    // Summary reader errors may hide a competing row before it reaches raw candidate selection.
    let summary_prefixes = [format!("{release}:QTD!"), format!("{release}:YTD!")];
    if let Some(error) = read.refusals.iter().find(|error| {
        summary_prefixes
            .iter()
            .any(|prefix| error.locator.starts_with(prefix.as_str()))
    }) {
        return Err(refusal(
            RefusalCode::SourceCellUnusable208,
            format!("summary reader refused {}: {}", error.locator, error.reason),
        ));
    }
    let period = policy.period.label();
    let field = policy.metric.field(policy.ownership);
    let subject = &policy.members[&forecast_id];
    // Count raw competitors before Claim conversion: a malformed share or unit must not hide a
    // second assertion and let the remaining cell become an apparently unambiguous outcome.
    let mut rows = read.values.iter().filter(|row| {
        let named = profile
            .shares
            .iter()
            .find(|share| share.label == row.site)
            .map_or(row.site.as_str(), |share| share.subject.as_str());
        let label = profile
            .mine_stats_label_aliases
            .iter()
            .find(|alias| alias.label == row.label && alias.holds_for(release))
            .map_or(row.label.as_str(), |alias| alias.reads_as.as_str());
        row.view == View::Summary
            && row.period == period
            && named == subject
            && field
                == format!(
                    "summary:{}",
                    label
                        .split_whitespace()
                        .collect::<Vec<_>>()
                        .join(" ")
                        .to_lowercase()
                )
    });
    let row = rows.next().ok_or_else(|| {
        refusal(
            RefusalCode::NoClaimInDomain201,
            format!(
                "the selected release has no assertion for frozen target {subject} {period} {field}"
            ),
        )
    })?;
    if rows.next().is_some() {
        return Err(refusal(
            RefusalCode::NonIdentifiableAttribution104,
            format!(
                "multiple selected source assertions match frozen target {subject} {period} {field}"
            ),
        ));
    }
    let cell = fmine_public::key::from_mine_stats(&profile, &workbook, release, row)
        .map_err(|error| refusal(RefusalCode::SourceCellUnusable208, error.to_string()))?;
    let result = if release == policy.period.original_release() {
        bind(&source, policy, forecast_id, &[cell])?
    } else {
        bind_release(&source, policy, forecast_id, release, &[cell])?
    };
    calendar_period(row, policy.period)?;
    Ok(result)
}

fn calendar_period(
    row: &fmine_public::mine_stats::MineStatsValue,
    period: PublicPeriod,
) -> Result<()> {
    let (months, duration, end) = match period {
        PublicPeriod::Year(_) => (12, "twelve", "december 31"),
        PublicPeriod::Quarter { quarter: 1, .. } => (3, "three", "march 31"),
        PublicPeriod::Quarter { quarter: 2, .. } => (3, "three", "june 30"),
        PublicPeriod::Quarter { quarter: 3, .. } => (3, "three", "september 30"),
        PublicPeriod::Quarter { quarter: 4, .. } => (3, "three", "december 31"),
        _ => {
            return Err(refusal(
                RefusalCode::SupportMismatch102,
                "invalid frozen calendar period",
            ));
        }
    };
    let label = row
        .period_label
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase();
    if row.months != months
        || row.period != period.label()
        || label.trim_end_matches(',') != format!("for the {duration} months ended {end}")
    {
        return Err(refusal(
            RefusalCode::SupportMismatch102,
            "outcome header does not state the frozen calendar duration and end date",
        ));
    }
    Ok(())
}

fn numeric_footnote(text: &str) -> &str {
    let text = text.trim_start();
    if let Some(note) = text.strip_prefix('(')
        && let Some((digits, rest)) = note.split_once(')')
        && !digits.is_empty()
        && digits.bytes().all(|b| b.is_ascii_digit())
    {
        return rest.trim_start();
    }
    text.trim_start_matches(|c: char| c.is_ascii_digit())
        .trim_start()
}

fn sold_ounce_aisc(header: Option<&str>) -> bool {
    let Some(header) = header else {
        return false;
    };
    let header = header
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase();
    let Some(rest) = header.strip_prefix("all-in sustaining costs") else {
        return false;
    };
    let Some(rest) = numeric_footnote(rest).strip_prefix("($/oz)") else {
        return false;
    };
    let rest = numeric_footnote(rest);
    ["", "per sold ounce", "per ounce sold"]
        .iter()
        .any(|suffix| {
            rest.strip_prefix(suffix)
                .is_some_and(|tail| numeric_footnote(tail).is_empty())
        })
}

fn publication(
    source: &SourceRow,
    policy: &PublicResolutionPolicy,
) -> Result<(String, String, i64)> {
    let date = source.published_at.as_ref().ok_or_else(|| {
        refusal(
            RefusalCode::NotObservable202,
            "workbook has no registered publication date; retrieval is not publication",
        )
    })?;
    let basis = source.published_at_basis.as_ref().ok_or_else(|| {
        refusal(
            RefusalCode::NotObservable202,
            "publication date has no stated evidence basis",
        )
    })?;
    if !fmine_registry::sources::is_date(date)
        || !fmine_registry::sources::PUBLISHED_AT_BASES
            .iter()
            .any(|prefix| {
                basis
                    .strip_prefix(prefix)
                    .is_some_and(|s| !s.trim().is_empty())
            })
    {
        return Err(refusal(
            RefusalCode::NotObservable202,
            "invalid publication-date evidence",
        ));
    }
    let year = date[..4]
        .parse::<i32>()
        .map_err(|_| refusal(RefusalCode::AssayLookahead108, "invalid publication year"))?;
    let month = date[5..7]
        .parse::<u8>()
        .map_err(|_| refusal(RefusalCode::AssayLookahead108, "invalid publication month"))?;
    let day = date[8..]
        .parse::<u8>()
        .map_err(|_| refusal(RefusalCode::AssayLookahead108, "invalid publication day"))?;
    let days = CivilDate::new(year, month, day)
        .map_err(|error| refusal(RefusalCode::AssayLookahead108, error.to_string()))?
        .to_epoch_days();
    let start = days.checked_mul(NS_PER_DAY).ok_or_else(|| {
        refusal(
            RefusalCode::AssayLookahead108,
            "publication time exceeds the supported calendar",
        )
    })?;
    let available = start.checked_add(NS_PER_DAY).ok_or_else(|| {
        refusal(
            RefusalCode::AssayLookahead108,
            "publication availability exceeds the supported calendar",
        )
    })?;
    if start < release_period(policy, &source.quarter)?.end_ns()? {
        return Err(refusal(
            RefusalCode::AssayLookahead108,
            "a complete-period workbook cannot be published before its release period ends",
        ));
    }
    Ok((date.clone(), basis.clone(), available))
}

fn same_ownership(actual: OwnershipBasis, expected: PublicOwnership) -> bool {
    match (actual, expected) {
        (OwnershipBasis::Consolidated100, PublicOwnership::Full)
        | (OwnershipBasis::Consolidated100, PublicOwnership::Attributable { num: 1, den: 1 }) => {
            true
        }
        (
            OwnershipBasis::Attributable { num, den, .. },
            PublicOwnership::Attributable { num: n, den: d },
        ) if num > 0 && den > 0 => {
            i128::from(num) * i128::from(d) == i128::from(den) * i128::from(n)
        }
        _ => false,
    }
}

/// Reconstruct an original-release receipt, without accepting a later comparative in its place.
pub(super) fn receipt_value_tag(
    policy: &PublicResolutionPolicy,
    forecast_id: ObjectId,
    citation: &OutcomeCitation,
    stored: &str,
    displayed: &str,
    ownership_source: &str,
) -> Result<String> {
    if citation.vintage != policy.period.original_release() {
        return Err(refusal(
            RefusalCode::SupportMismatch102,
            "receipt is not from the frozen original release",
        ));
    }
    receipt_value_tag_at_vintage(
        policy,
        forecast_id,
        citation,
        stored,
        displayed,
        ownership_source,
    )
}

fn receipt_value_tag_at_vintage(
    policy: &PublicResolutionPolicy,
    forecast_id: ObjectId,
    citation: &OutcomeCitation,
    stored: &str,
    displayed: &str,
    ownership_source: &str,
) -> Result<String> {
    public_policy(policy)?;
    release_period(policy, &citation.vintage)?;
    let subject = policy.members.get(&forecast_id).ok_or_else(|| {
        refusal(
            RefusalCode::NoClaimInDomain201,
            "receipt forecast is outside the frozen member mapping",
        )
    })?;
    if citation.source != policy.issuer {
        return Err(refusal(
            RefusalCode::SupportMismatch102,
            "receipt issuer differs from the frozen policy",
        ));
    }
    let ownership_basis = match (policy.ownership, ownership_source) {
        (PublicOwnership::Full, "full")
        | (PublicOwnership::Attributable { num: 1, den: 1 }, "full") => {
            OwnershipBasis::Consolidated100
        }
        (PublicOwnership::Attributable { num, den }, "printed" | "registry") => {
            let share = Rat::new(i128::from(num), i128::from(den))
                .map_err(|error| refusal(RefusalCode::BasisMismatch105, error.to_string()))?;
            OwnershipBasis::Attributable {
                num: share.num() as i64,
                den: share.den() as i64,
                source: if ownership_source == "printed" {
                    ShareSource::Printed
                } else {
                    ShareSource::Registry
                },
            }
        }
        _ => {
            return Err(refusal(
                RefusalCode::BasisMismatch105,
                "receipt ownership provenance differs from its frozen basis",
            ));
        }
    };
    let key = PublicKey {
        issuer: policy.issuer.clone(),
        subject: subject.clone(),
        valid_period: parse_period_code(&policy.period.label())
            .ok_or_else(|| refusal(RefusalCode::SupportMismatch102, "invalid receipt period"))?,
        reporting_vintage: Vintage {
            release: citation.vintage.clone(),
            file_sha256: Digest32(citation.sha256).to_hex(),
        },
        ownership_basis,
        scope: Scope::Site,
        unit: policy.metric.unit().into(),
        line: LineKey {
            label: policy.metric.field(policy.ownership).into(),
            occurrence: 0,
        },
    };
    Ok(canonical_value_tag(
        &key,
        &PublicValue::Number {
            stored: stored.into(),
            displayed: Ok(displayed.into()),
            under_status: None,
        },
    ))
}

/// Rebuild only the original admitted public assertion, without I/O or comparative fallback.
pub(super) fn receipt_claim_body(
    policy: &PublicResolutionPolicy,
    forecast_id: ObjectId,
    citation: &OutcomeCitation,
    stored: &str,
    displayed: &str,
    ownership_source: &str,
    anchor_path: &str,
) -> Result<Vec<u8>> {
    receipt_value_tag(
        policy,
        forecast_id,
        citation,
        stored,
        displayed,
        ownership_source,
    )?;
    receipt_claim_body_at_vintage(
        policy,
        forecast_id,
        citation,
        stored,
        displayed,
        ownership_source,
        anchor_path,
    )
}

/// Rebuild a source assertion at its retained reporting vintage. Admission of a revision additionally
/// needs the journal's checked predecessor; this pure reconstruction grants no current-source trust.
pub(super) fn receipt_claim_body_at_vintage(
    policy: &PublicResolutionPolicy,
    forecast_id: ObjectId,
    citation: &OutcomeCitation,
    stored: &str,
    displayed: &str,
    ownership_source: &str,
    anchor_path: &str,
) -> Result<Vec<u8>> {
    let (file, locator) = citation.locator.split_once(':').ok_or_else(|| {
        refusal(
            RefusalCode::SourceCellUnusable208,
            "receipt has no workbook cell locator",
        )
    })?;
    if file.is_empty()
        || locator.is_empty()
        || Path::new(anchor_path).file_name().and_then(|s| s.to_str()) != Some(file)
        || citation.sha256 == [0; 32]
    {
        return Err(refusal(
            RefusalCode::DigestMismatch505,
            "receipt anchor differs from its source citation",
        ));
    }
    let tag = receipt_value_tag_at_vintage(
        policy,
        forecast_id,
        citation,
        stored,
        displayed,
        ownership_source,
    )?;
    let value = PublicValue::Number {
        stored: stored.into(),
        displayed: Ok(displayed.into()),
        under_status: None,
    };
    Ok(Claim::new(
        policy.metric.field(policy.ownership),
        value,
        &tag,
        Representation::Deterministic,
        SpatialSupport::PeriodAggregate,
        if policy.metric.is_production() {
            AdditivityClass::ExtensiveAdditive
        } else {
            AdditivityClass::IntensiveWeighted
        },
        EvidenceClass::ISSUER,
        EpistemicStatus::KnownKnown,
        KnowledgeState::Known,
        Confidentiality::public(),
        vec![],
        vec![],
        vec![SourceAnchor::new(anchor_path, citation.sha256, locator)],
    )
    .canonical_body())
}

fn bind(
    registered: &SourceRow,
    policy: &PublicResolutionPolicy,
    forecast_id: ObjectId,
    cells: &[KeyedCell],
) -> Result<ResolvedPublicOutcome> {
    bind_release(
        registered,
        policy,
        forecast_id,
        &policy.period.original_release(),
        cells,
    )
}

fn bind_release(
    registered: &SourceRow,
    policy: &PublicResolutionPolicy,
    forecast_id: ObjectId,
    release: &str,
    cells: &[KeyedCell],
) -> Result<ResolvedPublicOutcome> {
    public_policy(policy)?;
    release_period(policy, release)?;
    if registered.quarter != release || registered.kind != "mine_stats" {
        return Err(refusal(
            RefusalCode::SupportMismatch102,
            "registered workbook does not match the selected outcome release",
        ));
    }
    let subject = policy.members.get(&forecast_id).ok_or_else(|| {
        refusal(
            RefusalCode::NoClaimInDomain201,
            "forecast is outside the frozen member mapping",
        )
    })?;
    let period = parse_period_code(&policy.period.label()).ok_or_else(|| {
        refusal(
            RefusalCode::SupportMismatch102,
            "unsupported frozen reporting period",
        )
    })?;
    let label = policy.metric.field(policy.ownership);
    let mut candidates = cells.iter().filter(|cell| {
        cell.key.issuer == policy.issuer
            && cell.key.subject == *subject
            && cell.key.valid_period == period
            && cell.key.line.label == label
            && cell.key.reporting_vintage.release == release
    });
    let cell = candidates.next().ok_or_else(|| {
        refusal(
            RefusalCode::NoClaimInDomain201,
            "the selected release has no assertion for the exact frozen target",
        )
    })?;
    if candidates.next().is_some() {
        return Err(refusal(
            RefusalCode::NonIdentifiableAttribution104,
            "multiple source assertions match the frozen target",
        ));
    }
    cell.claim.check_id()?;
    if cell.claim.metric_key != label
        || cell.claim.value_tag() != canonical_value_tag(&cell.key, &cell.claim.value)
    {
        return Err(refusal(
            RefusalCode::FrozenRecordMutation507,
            "source Claim key or value was mutated",
        ));
    }
    if !cell.claim.confidentiality.is_public() {
        return Err(refusal(
            RefusalCode::MnpiWall301,
            "outcome source is not public",
        ));
    }
    if policy.metric == PublicMetric::AiscUsdPerOz
        && !sold_ounce_aisc(cell.label_as_printed.as_deref())
    {
        return Err(refusal(
            RefusalCode::BasisMismatch105,
            "source header does not state the supported issuer AISC definition per sold gold ounce",
        ));
    }
    if cell.key.scope != Scope::Site
        || cell.key.line.occurrence != 0
        || cell.key.unit != policy.metric.unit()
        || !same_ownership(cell.key.ownership_basis, policy.ownership)
        || (!(policy.metric.is_production() && policy.ownership == PublicOwnership::Full)
            && !same_ownership(cell.printed_basis, policy.ownership))
        || cell.claim.support != SpatialSupport::PeriodAggregate
    {
        return Err(refusal(
            RefusalCode::BasisMismatch105,
            "outcome support, unit or printed ownership differs from the frozen target",
        ));
    }
    if cell.key.reporting_vintage.file_sha256 != registered.sha256
        || !cell.claim.anchors.iter().any(|anchor| {
            Digest32(anchor.artifact_sha256).to_hex() == registered.sha256
                && Path::new(&anchor.artifact_path)
                    .file_name()
                    .and_then(|s| s.to_str())
                    == Some(registered.file.as_str())
                && cell.locator == format!("{}:{}", registered.file, anchor.locator)
        })
    {
        return Err(refusal(
            RefusalCode::DigestMismatch505,
            "source citation does not bind the registered workbook bytes",
        ));
    }
    if cell.claim.stale {
        return Err(refusal(
            RefusalCode::StaleAnchor502,
            "outcome source is stale",
        ));
    }
    if cell.claim.calibration_failed
        || cell.claim.evidence != EvidenceClass::ISSUER
        || cell.claim.state != KnowledgeState::Known
        || cell.claim.epistemic != EpistemicStatus::KnownKnown
        || !cell.claim.uu_terms.is_empty()
        || !matches!(cell.claim.representation, Representation::Deterministic)
    {
        return Err(refusal(
            RefusalCode::NotObservable202,
            "outcome source is not an admissible issuer assertion",
        ));
    }
    let PublicValue::Number {
        stored,
        displayed: Ok(displayed),
        under_status: None,
    } = &cell.claim.value
    else {
        return Err(refusal(
            RefusalCode::SourceCellUnusable208,
            "outcome is missing, a placeholder, status-bearing or not exactly displayable",
        ));
    };
    let value = Rat::from_decimal_str(displayed).map_err(|error| {
        refusal(
            RefusalCode::SourceCellUnusable208,
            format!("displayed outcome is not an exact decimal: {error}"),
        )
    })?;
    if policy.metric.is_production() && value.num() < 0 {
        return Err(refusal(
            RefusalCode::SourceCellUnusable208,
            "production cannot be negative",
        ));
    }
    let (published_at, published_at_basis, available_at_ns) = publication(registered, policy)?;
    let out = ResolvedPublicOutcome {
        source: cell.clone(),
        value,
        stored: stored.clone(),
        displayed: displayed.clone(),
        ownership_source: match cell.key.ownership_basis {
            OwnershipBasis::Consolidated100 => "full",
            OwnershipBasis::Attributable {
                source: ShareSource::Printed,
                ..
            } => "printed",
            OwnershipBasis::Attributable {
                source: ShareSource::Registry,
                ..
            } => "registry",
            OwnershipBasis::Unprinted => {
                return Err(refusal(
                    RefusalCode::BasisMismatch105,
                    "unprinted source ownership",
                ));
            }
        },
        citation: OutcomeCitation {
            source: policy.issuer.clone(),
            vintage: registered.quarter.clone(),
            locator: cell.locator.clone(),
            sha256: super::hex(&registered.sha256)?.0,
        },
        available_at_ns,
        published_at,
        published_at_basis,
    };
    let [anchor] = out.source.claim.anchors.as_slice() else {
        return Err(refusal(
            RefusalCode::NotObservable202,
            "outcome must be one standalone source assertion",
        ));
    };
    if receipt_claim_body_at_vintage(
        policy,
        forecast_id,
        &out.citation,
        &out.stored,
        &out.displayed,
        out.ownership_source,
        &anchor.artifact_path,
    )? != out.source.claim.canonical_body()
    {
        return Err(refusal(
            RefusalCode::NotObservable202,
            "outcome Claim differs from the admitted public issuer source shape",
        ));
    }
    Ok(out)
}

#[cfg(test)]
mod tests;
