//! Source-bound resolution of a previously frozen public forecast target.
//! Determinism: Exact (displayed decimal, source Claim and registered publication date).
//!
//! DESIGN: the policy fixes every semantic selector before an outcome. Original resolution never
//! falls back to a comparative. Explicit later-vintage resolution authenticates the same frozen
//! target, without changing its period or basis. The journal owner checks predecessor and clock
//! policy before admitting a revision; a comparative is not proof of a formal issuer restatement.

use std::io::Read as _;
use std::path::Path;

use fmine_calib::ledger::binding::{
    PublicMetric, PublicOwnership, PublicPeriod, PublicResolutionPolicy,
};
use fmine_calib::ledger::{MissingReason, OutcomeCitation};
#[cfg(test)]
use fmine_claim::{AdditivityClass, Claim, Confidentiality};
use fmine_claim::{Representation, SpatialSupport};
use fmine_core::ObjectId;
use fmine_core::anchor::SourceAnchor;
use fmine_core::hash::{Digest32, sha256};
use fmine_evidence::{EpistemicStatus, EvidenceClass, KnowledgeState, Refusal, RefusalCode};
use fmine_io::xlsx::Workbook;
use fmine_numerics::exact_feas::Rat;
use fmine_public::key::{
    KeyedCell, LineKey, NilReason, OwnershipBasis, PublicKey, PublicValue, Scope, ShareSource,
    StatusKind, SubjectStatus, Vintage, canonical_value_tag, parse_period_code,
    public_claim_anchored,
};
use fmine_public::mine_stats::{View, read_mine_stats};
use fmine_public::sources::{IssuerProfile, SourceKind, is_issuer_name, registry_path};
use fmine_registry::sources::{SourceRow, parse_sources_registry};
use fmine_time::{CivilDate, NS_PER_DAY};

type Result<T> = std::result::Result<T, Refusal>;

/// Immutable admission evidence. A saved journal operation retains these facts rather than claiming
/// that replay has authenticated the current contents of a source file.
#[derive(Debug, Clone)]
pub(super) struct SourceReceipt {
    pub source: KeyedCell,
    pub ownership_source: &'static str,
    pub citation: OutcomeCitation,
    /// Conservative date-only availability: midnight immediately following the publication date.
    pub available_at_ns: i64,
    pub published_at: String,
    pub published_at_basis: String,
}

#[derive(Debug, Clone)]
pub(super) struct ResolvedPublic<T> {
    pub receipt: SourceReceipt,
    pub value: T,
}

impl<T> std::ops::Deref for ResolvedPublic<T> {
    type Target = SourceReceipt;
    fn deref(&self) -> &Self::Target {
        &self.receipt
    }
}

#[derive(Debug, Clone)]
pub(super) struct PublicNumber {
    pub exact: Rat,
    pub stored: String,
    pub displayed: String,
}

/// A missing target retains its original assertion, including a printed partial-period number.
#[derive(Debug, Clone)]
pub(super) struct PublicMissing {
    pub reason: MissingReason,
    pub value_kind: String,
    pub token: String,
    pub displayed: Option<String>,
    pub status: Option<StatusEvidence>,
}

pub(super) type ResolvedPublicOutcome = ResolvedPublic<PublicNumber>;
pub(super) type ResolvedPublicMissing = ResolvedPublic<PublicMissing>;

#[derive(Debug, Clone)]
pub(super) enum ResolvedOriginalOutcome {
    Published(ResolvedPublicOutcome),
    Missing(ResolvedPublicMissing),
}

/// Interpretation data from the exact registry snapshot parsed at admission, not publication-time proof.
#[derive(Debug, Clone)]
pub(super) struct StatusEvidence {
    pub status: SubjectStatus,
    pub registry_sha256: Digest32,
}

impl StatusEvidence {
    pub(super) fn reason(
        &self,
        policy: &PublicResolutionPolicy,
        forecast_id: ObjectId,
    ) -> Result<Option<MissingReason>> {
        let status = &self.status;
        if policy.members.get(&forecast_id) != Some(&status.subject)
            || self.registry_sha256 == Digest32::default()
            || status.evidence.trim().is_empty()
            || status.last.is_some_and(|last| last < status.first)
            || std::iter::once(status.first)
                .chain(status.last)
                .any(|(year, month)| {
                    year == 0 || CivilDate::new(i32::from(year), month, 1).is_err()
                })
        {
            return Err(refusal(
                RefusalCode::SourceCellUnusable208,
                "invalid retained subject status",
            ));
        }
        let period = parse_period_code(&policy.period.label())
            .ok_or_else(|| refusal(RefusalCode::SupportMismatch102, "invalid target period"))?;
        let (start, end) = period.months();
        // A disposal's first full month follows its closing month. A target overlapping that
        // closing month is a stub even though the raw source Claim correctly keeps its number.
        let first = match (status.kind, status.first) {
            (StatusKind::Disposed, (year, 1)) => (year - 1, 12),
            (StatusKind::Disposed, (year, month)) => (year, month - 1),
            _ => status.first,
        };
        if status.kind == StatusKind::CareAndMaintenance
            || end < first
            || status.last.is_some_and(|last| start > last)
        {
            return Ok(None);
        }
        Ok(Some(if !status.applies(&period) {
            MissingReason::DefinitionChanged
        } else if status.kind == StatusKind::Disposed {
            MissingReason::Divested
        } else {
            MissingReason::NotPublished
        }))
    }
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
) -> Result<ResolvedOriginalOutcome> {
    resolve_selected(root, policy, forecast_id, &policy.period.original_release())
}

/// Authenticate exactly one requested later release while keeping every target selector frozen.
/// Comparative placeholders never replace an original outcome or missing-status event.
pub(super) fn resolve_release(
    root: &Path,
    policy: &PublicResolutionPolicy,
    forecast_id: ObjectId,
    release: &str,
) -> Result<ResolvedPublicOutcome> {
    match resolve_selected(root, policy, forecast_id, release)? {
        ResolvedOriginalOutcome::Published(outcome) => Ok(outcome),
        ResolvedOriginalOutcome::Missing(_) => Err(refusal(
            RefusalCode::NotObservable202,
            "selected comparative does not publish a numeric value for the frozen target",
        )),
    }
}

fn resolve_selected(
    root: &Path,
    policy: &PublicResolutionPolicy,
    forecast_id: ObjectId,
    release: &str,
) -> Result<ResolvedOriginalOutcome> {
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
    // Reader errors may hide a competing row before raw selection. Site targets require clean Minestats
    // blocks; repeated-label refusals cannot leave a seemingly unique first row.
    let summary_prefixes = [format!("{release}:QTD!"), format!("{release}:YTD!")];
    if let Some(error) = read.refusals.iter().find(|error| {
        if policy.metric.is_site_row() {
            error.locator.starts_with(&format!("{release}:Minestats"))
        } else {
            summary_prefixes
                .iter()
                .any(|prefix| error.locator.starts_with(prefix.as_str()))
        }
    }) {
        return Err(refusal(
            RefusalCode::SourceCellUnusable208,
            format!("source reader refused {}: {}", error.locator, error.reason),
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
        let normalized = |value: &str| {
            value
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
                .to_lowercase()
        };
        let line = if row.view == View::Summary {
            format!("summary:{}", normalized(label))
        } else {
            format!(
                "site:{}:{}",
                normalized(row.section.as_deref().unwrap_or("")),
                normalized(label)
            )
        };
        let wanted_view = if policy.metric.is_site_row() {
            View::Minestats
        } else {
            View::Summary
        };
        row.view == wanted_view && row.period == period && named == subject && field == line
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
    let mut statuses = profile
        .subject_statuses
        .iter()
        .filter(|s| s.subject == *subject);
    let status = statuses.next().cloned().map(|status| StatusEvidence {
        status,
        registry_sha256: sha256(&registry_bytes),
    });
    if statuses.next().is_some() {
        return Err(refusal(
            RefusalCode::NonIdentifiableAttribution104,
            "multiple subject-status interpretations",
        ));
    }
    let status = match status {
        Some(status) if status.reason(policy, forecast_id)?.is_some() => Some(status),
        _ => None,
    };
    let result = if status.is_none() && release == policy.period.original_release() {
        bind(&source, policy, forecast_id, &[cell])?
    } else {
        bind_value(&source, policy, forecast_id, release, &[cell], status)?
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
        PublicPeriod::FirstHalf(_) => (6, "six", "june 30"),
        PublicPeriod::NineMonths(_) => (9, "nine", "september 30"),
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

/// Costs require their exact sold-unit definition; capex requires its printed reported-line scope.
/// Numeric footnotes are allowed. A registered label alias cannot redefine the frozen monetary target.
fn target_header(metric: PublicMetric, header: Option<&str>) -> bool {
    let name = match metric {
        PublicMetric::AiscUsdPerOz | PublicMetric::AiscUsdPerLb => "all-in sustaining costs",
        PublicMetric::TccUsdPerOz => "total cash costs",
        PublicMetric::C1UsdPerLb => "c1 cash costs",
        _ => {
            return !metric.is_capex()
                || header.is_some_and(|printed| {
                    let (label, _) = fmine_public::mine_stats::strip_footnote(printed);
                    let label = label.split_whitespace().collect::<Vec<_>>().join(" ");
                    metric
                        .field(PublicOwnership::Full)
                        .rsplit_once(':')
                        .is_some_and(|(_, expected)| label.eq_ignore_ascii_case(expected))
                });
        }
    };
    let (unit, sold) = if metric.unit() == "usd_per_oz" {
        ("($/oz)", ["per sold ounce", "per ounce sold"])
    } else {
        ("($/lb)", ["per sold pound", "per pound sold"])
    };
    let Some(header) = header else {
        return false;
    };
    let header = header
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase();
    let Some(rest) = header.strip_prefix(name) else {
        return false;
    };
    let Some(rest) = numeric_footnote(rest).strip_prefix(unit) else {
        return false;
    };
    let rest = numeric_footnote(rest);
    ["", sold[0], sold[1]].iter().any(|suffix| {
        rest.strip_prefix(suffix)
            .is_some_and(|tail| numeric_footnote(tail).is_empty())
    })
}

/// The same publication metadata validation is used at admission and offline receipt replay.
pub(super) fn publication_start(
    date: &str,
    basis: &str,
) -> std::result::Result<i64, (RefusalCode, &'static str)> {
    if !fmine_registry::sources::is_date(date)
        || !fmine_registry::sources::PUBLISHED_AT_BASES
            .iter()
            .any(|prefix| {
                basis
                    .strip_prefix(prefix)
                    .is_some_and(|s| !s.trim().is_empty())
            })
    {
        return Err((
            RefusalCode::NotObservable202,
            "invalid publication-date evidence",
        ));
    }
    fmine_calib::record::parse_utc(&format!("{date}T00:00:00Z"))
        .and_then(|seconds| seconds.checked_mul(1_000_000_000))
        .ok_or((
            RefusalCode::AssayLookahead108,
            "publication time exceeds the supported calendar",
        ))
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
    let start = publication_start(date, basis).map_err(|(code, reason)| refusal(code, reason))?;
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

/// Scoring uses the printed value in the frozen unit. A mass-fraction source has Excel's percent
/// presentation, so `91%` is exactly 91/100, not 91 or the more precise unprinted stored value.
/// Both source admission and offline journal replay use this rule and retain the original display token.
pub(super) fn displayed_value(metric: PublicMetric, displayed: &str) -> Result<Rat> {
    let invalid = |reason: String| refusal(RefusalCode::SourceCellUnusable208, reason);
    let number = if metric.is_fraction() {
        displayed.strip_suffix('%').ok_or_else(|| {
            invalid("a frozen mass-fraction target requires an explicit percent display".into())
        })?
    } else {
        displayed
    };
    let value = Rat::from_decimal_str(number).map_err(|error| {
        invalid(format!(
            "displayed outcome is not an exact decimal: {error}"
        ))
    })?;
    let value = if metric.is_fraction() {
        value
            .checked_div(Rat::from_integer(100))
            .map_err(|error| invalid(format!("percent display conversion failed: {error}")))?
    } else {
        value
    };
    physical_domain(metric, value)?;
    Ok(value)
}

fn physical_domain(metric: PublicMetric, value: Rat) -> Result<()> {
    if (metric.is_production() || metric.is_site_physical()) && value.num() < 0 {
        return Err(refusal(
            RefusalCode::SourceCellUnusable208,
            "physical outcomes cannot be negative",
        ));
    }
    if metric.is_fraction() && value.num() > value.den() {
        return Err(refusal(
            RefusalCode::SourceCellUnusable208,
            "grade and recovery fractions cannot exceed one",
        ));
    }
    Ok(())
}

fn receipt_ownership(ownership: PublicOwnership, source: &str) -> Result<OwnershipBasis> {
    Ok(match (ownership, source) {
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
                source: if source == "printed" {
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
    })
}

fn receipt_locator<'a>(citation: &'a OutcomeCitation, anchor_path: &str) -> Result<&'a str> {
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
    Ok(locator)
}

fn receipt_number(
    policy: &PublicResolutionPolicy,
    stored: &str,
    displayed: &str,
) -> Result<PublicValue> {
    if policy.metric.is_site_physical() {
        let stored = Rat::from_decimal_str(stored).map_err(|error| {
            refusal(
                RefusalCode::SourceCellUnusable208,
                format!("invalid stored physical value: {error}"),
            )
        })?;
        physical_domain(policy.metric, stored)?;
        displayed_value(policy.metric, displayed)?;
    }
    Ok(PublicValue::Number {
        stored: stored.into(),
        displayed: Ok(displayed.into()),
        under_status: None,
    })
}

/// Pure reconstruction of one frozen source Claim; no current-source or comparative authority is implied.
pub(super) struct ReceiptClaim<'a> {
    pub policy: &'a PublicResolutionPolicy,
    pub forecast_id: ObjectId,
    pub citation: &'a OutcomeCitation,
    pub ownership_source: &'a str,
    pub anchor_path: &'a str,
}

impl ReceiptClaim<'_> {
    fn original_release(&self) -> Result<()> {
        if self.citation.vintage != self.policy.period.original_release() {
            return Err(refusal(
                RefusalCode::SupportMismatch102,
                "receipt is not from the frozen original release",
            ));
        }
        Ok(())
    }

    fn key(&self) -> Result<PublicKey> {
        let policy = self.policy;
        let citation = self.citation;
        public_policy(policy)?;
        release_period(policy, &citation.vintage)?;
        let subject = policy.members.get(&self.forecast_id).ok_or_else(|| {
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
        Ok(PublicKey {
            issuer: policy.issuer.clone(),
            subject: subject.clone(),
            valid_period: parse_period_code(&policy.period.label()).ok_or_else(|| {
                refusal(RefusalCode::SupportMismatch102, "invalid receipt period")
            })?,
            reporting_vintage: Vintage {
                release: citation.vintage.clone(),
                file_sha256: Digest32(citation.sha256).to_hex(),
            },
            ownership_basis: receipt_ownership(policy.ownership, self.ownership_source)?,
            scope: Scope::Site,
            unit: policy.metric.unit().into(),
            line: LineKey {
                label: policy.metric.field(policy.ownership).into(),
                occurrence: 0,
            },
        })
    }

    fn body(&self, value: PublicValue) -> Result<Vec<u8>> {
        let key = self.key()?;
        let locator = receipt_locator(self.citation, self.anchor_path)?;
        Ok(public_claim_anchored(
            SourceAnchor::new(self.anchor_path, self.citation.sha256, locator),
            vec![],
            key,
            value,
        )
        .claim
        .canonical_body())
    }

    /// The journal separately checks a permitted comparative's numeric predecessor before admission.
    pub(super) fn number(
        &self,
        stored: &str,
        displayed: &str,
        allow_comparative: bool,
    ) -> Result<Vec<u8>> {
        if !allow_comparative {
            self.original_release()?;
        }
        self.body(receipt_number(self.policy, stored, displayed)?)
    }

    /// Keep the raw assertion unchanged and independently recheck its retained status interpretation.
    pub(super) fn missing(&self, missing: &PublicMissing) -> Result<Vec<u8>> {
        self.original_release()?;
        let invalid = || {
            refusal(
                RefusalCode::SourceCellUnusable208,
                "source value or reason contradicts its retained status",
            )
        };
        let reason = match &missing.status {
            Some(status) => status
                .reason(self.policy, self.forecast_id)?
                .ok_or_else(invalid)?,
            None => MissingReason::NotPublished,
        };
        if missing.reason != reason {
            return Err(invalid());
        }
        let full_status = missing
            .status
            .as_ref()
            .and_then(|s| (reason != MissingReason::DefinitionChanged).then_some(s.status.kind));
        let token = missing.token.clone();
        let zero = token.trim().parse::<f64>() == Ok(0.0);
        let value = match missing.value_kind.as_str() {
            "nil_unknown"
                if missing.displayed.is_none()
                    && (missing.status.is_none() || matches!(token.trim(), "-" | "–" | "—")) =>
            {
                PublicValue::Nil {
                    token,
                    reason: NilReason::Unknown,
                }
            }
            "nil_withheld" | "nil_disposed" if zero && missing.displayed.is_none() => {
                let nil = match (missing.value_kind.as_str(), full_status) {
                    ("nil_withheld", Some(StatusKind::Withheld)) => NilReason::Withheld,
                    ("nil_disposed", Some(StatusKind::Disposed)) => NilReason::Disposed,
                    _ => return Err(invalid()),
                };
                PublicValue::Nil { token, reason: nil }
            }
            "number" if missing.status.is_some() && !(zero && full_status.is_some()) => {
                let mut value = receipt_number(
                    self.policy,
                    &token,
                    missing.displayed.as_deref().ok_or_else(invalid)?,
                )?;
                if let PublicValue::Number { under_status, .. } = &mut value {
                    *under_status = full_status;
                }
                value
            }
            _ => return Err(invalid()),
        };
        self.body(value)
    }
}

fn bind(
    registered: &SourceRow,
    policy: &PublicResolutionPolicy,
    forecast_id: ObjectId,
    cells: &[KeyedCell],
) -> Result<ResolvedOriginalOutcome> {
    bind_value(
        registered,
        policy,
        forecast_id,
        &policy.period.original_release(),
        cells,
        None,
    )
}

fn bind_value(
    registered: &SourceRow,
    policy: &PublicResolutionPolicy,
    forecast_id: ObjectId,
    release: &str,
    cells: &[KeyedCell],
    status: Option<StatusEvidence>,
) -> Result<ResolvedOriginalOutcome> {
    public_policy(policy)?;
    release_period(policy, release)?;
    let allow_missing = release == policy.period.original_release();
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
    if !target_header(policy.metric, cell.label_as_printed.as_deref()) {
        return Err(refusal(
            RefusalCode::BasisMismatch105,
            "source header does not state the frozen issuer monetary definition and denominator",
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
    let ownership_source = match cell.key.ownership_basis {
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
    };
    let citation = OutcomeCitation {
        source: policy.issuer.clone(),
        vintage: registered.quarter.clone(),
        locator: cell.locator.clone(),
        sha256: super::hex(&registered.sha256)?.0,
    };
    let (published_at, published_at_basis, available_at_ns) = publication(registered, policy)?;
    let [anchor] = cell.claim.anchors.as_slice() else {
        return Err(refusal(
            RefusalCode::NotObservable202,
            "outcome must be one standalone source assertion",
        ));
    };
    let receipt = SourceReceipt {
        source: cell.clone(),
        ownership_source,
        citation,
        available_at_ns,
        published_at,
        published_at_basis,
    };
    let reconstruction = ReceiptClaim {
        policy,
        forecast_id,
        citation: &receipt.citation,
        ownership_source,
        anchor_path: &anchor.artifact_path,
    };
    if status.is_some() && (!allow_missing || cell.claim.calibration_failed) {
        return Err(refusal(
            RefusalCode::NotObservable202,
            "status-dependent target is not an admissible comparative outcome",
        ));
    }
    if allow_missing
        && (status.is_some()
            || matches!(
                cell.claim.value,
                PublicValue::Nil {
                    reason: NilReason::Unknown,
                    ..
                }
            ))
    {
        let (value_kind, token, displayed) = match &cell.claim.value {
            PublicValue::Nil { token, reason } => (
                match reason {
                    NilReason::Unknown => "nil_unknown",
                    NilReason::Withheld => "nil_withheld",
                    NilReason::Disposed => "nil_disposed",
                },
                token.clone(),
                None,
            ),
            PublicValue::Number {
                stored,
                displayed: Ok(displayed),
                ..
            } => ("number", stored.clone(), Some(displayed.clone())),
            _ => {
                return Err(refusal(
                    RefusalCode::SourceCellUnusable208,
                    "status cannot supply an absent or unusable source assertion",
                ));
            }
        };
        let reason = match &status {
            Some(s) => s.reason(policy, forecast_id)?.ok_or_else(|| {
                refusal(
                    RefusalCode::SourceCellUnusable208,
                    "status does not affect the target",
                )
            })?,
            None => MissingReason::NotPublished,
        };
        let missing = PublicMissing {
            reason,
            value_kind: value_kind.into(),
            token,
            displayed,
            status,
        };
        let expected = reconstruction.missing(&missing)?;
        if expected != cell.claim.canonical_body() {
            return Err(refusal(
                RefusalCode::NotObservable202,
                "missing-outcome Claim differs from the admitted public source shape",
            ));
        }
        return Ok(ResolvedOriginalOutcome::Missing(ResolvedPublicMissing {
            receipt,
            value: missing,
        }));
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
            "outcome source is not an admissible numeric issuer assertion",
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
    let value = displayed_value(policy.metric, displayed)?;
    if reconstruction.number(stored, displayed, true)? != cell.claim.canonical_body() {
        return Err(refusal(
            RefusalCode::NotObservable202,
            "outcome Claim differs from the admitted public issuer source shape",
        ));
    }
    Ok(ResolvedOriginalOutcome::Published(ResolvedPublicOutcome {
        receipt,
        value: PublicNumber {
            exact: value,
            stored: stored.clone(),
            displayed: displayed.clone(),
        },
    }))
}

#[cfg(test)]
mod tests;
