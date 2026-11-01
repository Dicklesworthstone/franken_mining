//! Source-bound resolution of a previously frozen public forecast target.
//! Determinism: Exact (displayed decimal, source Claim and registered publication date).
//!
//! DESIGN: the policy fixes every semantic selector before an outcome. Resolution reads only the
//! original registered release; a later comparative, different basis or stored unrounded value cannot
//! replace that target. The journal owner supplies and checks its own bounded recording clock.

use std::path::Path;

use fmine_calib::ledger::OutcomeCitation;
use fmine_calib::ledger::binding::{
    PublicMetric, PublicOwnership, PublicPeriod, PublicResolutionPolicy,
};
use fmine_claim::{AdditivityClass, Claim, Confidentiality, Representation, SpatialSupport};
use fmine_core::ObjectId;
use fmine_core::anchor::SourceAnchor;
use fmine_core::hash::Digest32;
use fmine_evidence::{EpistemicStatus, EvidenceClass, KnowledgeState, Refusal, RefusalCode};
use fmine_numerics::exact_feas::Rat;
use fmine_public::key::{
    KeyedCell, LineKey, OwnershipBasis, PublicKey, PublicValue, Scope, ShareSource, Vintage,
    canonical_value_tag, parse_period_code,
};
use fmine_public::sources::{IssuerProfile, SourceKind, is_issuer_name, registry_path};
use fmine_registry::sources::{SourceRow, load_sources_registry};
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

/// Authenticate the original workbook and resolve only the target bound into `policy` for this id.
/// No caller-supplied value, locator, subject, basis, source date or clock participates in selection.
pub(super) fn resolve(
    root: &Path,
    policy: &PublicResolutionPolicy,
    forecast_id: ObjectId,
) -> Result<ResolvedPublicOutcome> {
    public_policy(policy)?;
    if !policy.members.contains_key(&forecast_id) {
        return Err(refusal(
            RefusalCode::NoClaimInDomain201,
            "forecast is not in the frozen public target mapping",
        ));
    }
    let path = registry_path(root, &policy.issuer);
    let registry = load_sources_registry(&path).map_err(|error| {
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
    let release = policy.period.original_release();
    let mut candidates = registry
        .sources
        .iter()
        .filter(|row| row.kind == "mine_stats" && row.quarter == release);
    let source = candidates.next().ok_or_else(|| {
        refusal(
            RefusalCode::NoClaimInDomain201,
            format!("original {release} Mine Stats workbook is not registered"),
        )
    })?;
    if candidates.next().is_some() {
        return Err(refusal(
            RefusalCode::NonIdentifiableAttribution104,
            "more than one workbook claims the original outcome release",
        ));
    }
    let source = source.clone();
    publication(&source, policy)?;
    let mut profile = IssuerProfile::from_registry(registry)
        .map_err(|error| refusal(RefusalCode::InputMissingOrUnreadable506, error))?;
    // Do not require unrelated historical or later documents, and never let one supply a fallback.
    profile.sources.retain(|row| {
        row.kind == SourceKind::MineStats && row.quarter == release && row.file_name == source.file
    });
    let sources = fmine_public::load_registered_sources(&profile, root)?;
    let vocabulary = profile.mine_stats_labels.iter().cloned().collect();
    let reads = sources
        .iter()
        .map(|verified| {
            Ok((
                verified.file_name.clone(),
                fmine_public::mine_stats::read_mine_stats(
                    &verified.workbook,
                    &verified.quarter,
                    (!profile.mine_stats_labels.is_empty()).then_some(&vocabulary),
                )?,
            ))
        })
        .collect::<Result<Vec<_>>>()?;
    let loaded = fmine_public::key::mine_stats::build_claims(&profile, &sources, &reads);
    let result = bind(&source, policy, forecast_id, &loaded.claims)?;
    let mut rows = reads
        .iter()
        .flat_map(|(file, read)| read.values.iter().map(move |row| (file, row)))
        .filter(|(file, row)| {
            format!("{file}:{}!{}", row.sheet, row.cell) == result.source.locator
        });
    let (_, row) = rows.next().ok_or_else(|| {
        refusal(
            RefusalCode::SupportMismatch102,
            "outcome has no original period-header evidence",
        )
    })?;
    if rows.next().is_some() {
        return Err(refusal(
            RefusalCode::SupportMismatch102,
            "outcome has ambiguous period-header evidence",
        ));
    }
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
            "original outcome header does not state the frozen calendar duration and end date",
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
            "original workbook has no registered publication date; retrieval is not publication",
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
    if start < policy.period.end_ns()? {
        return Err(refusal(
            RefusalCode::AssayLookahead108,
            "a complete-period outcome cannot be published before its period ends",
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

/// Reconstruct the existing public value tag from the frozen target and retained numeric evidence.
/// Replay compares this with the tag in the source's canonical Claim body; a different displayed
/// amount or target cannot reuse an admitted source Claim id.
pub(super) fn receipt_value_tag(
    policy: &PublicResolutionPolicy,
    forecast_id: ObjectId,
    citation: &OutcomeCitation,
    stored: &str,
    displayed: &str,
    ownership_source: &str,
) -> Result<String> {
    public_policy(policy)?;
    let subject = policy.members.get(&forecast_id).ok_or_else(|| {
        refusal(
            RefusalCode::NoClaimInDomain201,
            "receipt forecast is outside the frozen member mapping",
        )
    })?;
    if citation.source != policy.issuer || citation.vintage != policy.period.original_release() {
        return Err(refusal(
            RefusalCode::SupportMismatch102,
            "receipt is not from the frozen original release",
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

/// Rebuild the canonical body of the admitted standalone public issuer assertion, without I/O.
/// Only the original one-anchor source shape is supported; all target semantics come from `policy`.
pub(super) fn receipt_claim_body(
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
    let tag = receipt_value_tag(
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
        if policy.metric == PublicMetric::ProductionKoz {
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
    public_policy(policy)?;
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
            && cell.key.reporting_vintage.release == policy.period.original_release()
    });
    let cell = candidates.next().ok_or_else(|| {
        refusal(
            RefusalCode::NoClaimInDomain201,
            "the original release has no assertion for the exact frozen target",
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
        // A 100%-production column declares its own basis; the site-row share applies to the
        // separate attributable and cost columns, not to that explicitly full-production value.
        || (!(policy.metric == PublicMetric::ProductionKoz && policy.ownership == PublicOwnership::Full)
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
    if policy.metric == PublicMetric::ProductionKoz && value.num() < 0 {
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
            "outcome must be one standalone original source assertion",
        ));
    };
    if receipt_claim_body(
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
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    use fmine_calib::ledger::binding::PublicPeriod;
    use fmine_core::id::PrefixKind;
    use fmine_io::xlsx::{MinimalCell, MinimalSheet, Workbook, build_minimal_xlsx};
    use fmine_public::key::public_claim;

    /// Invented standalone source Claims; file-loading and durable admission are covered by the
    /// registered-workbook integration test, which independently exercises the actual parser.
    fn example(metric: PublicMetric) -> (PublicResolutionPolicy, ObjectId, SourceRow, KeyedCell) {
        let id = ObjectId::from_canonical_bytes(PrefixKind::Forecast, b"invented public target");
        let policy = PublicResolutionPolicy {
            issuer: "example".into(),
            metric,
            period: PublicPeriod::Year(2030),
            ownership: PublicOwnership::Full,
            members: BTreeMap::from([(id, "Example Mine".into())]),
        };
        let bytes = build_minimal_xlsx(&[MinimalSheet {
            name: "YTD".into(),
            hidden: false,
            cells: vec![("B5".into(), MinimalCell::Number(400.4))],
        }]);
        let workbook =
            Workbook::from_bytes("corpus/Original.xlsx", &bytes).expect("invented workbook");
        let registered = SourceRow {
            file: "Original.xlsx".into(),
            kind: "mine_stats".into(),
            quarter: "2030Q4".into(),
            url: "https://example.invalid/Original.xlsx".into(),
            url_basis: "W1 table".into(),
            bytes: bytes.len() as u64,
            sha256: Digest32(workbook.sha256).to_hex(),
            published_at: Some("2031-02-15".into()),
            published_at_basis: Some("FACT: invented test release date".into()),
        };
        let key = PublicKey {
            issuer: policy.issuer.clone(),
            subject: policy.members[&id].clone(),
            valid_period: parse_period_code("2030FY").expect("period"),
            reporting_vintage: Vintage {
                release: "2030Q4".into(),
                file_sha256: registered.sha256.clone(),
            },
            ownership_basis: OwnershipBasis::Consolidated100,
            scope: Scope::Site,
            unit: metric.unit().into(),
            line: LineKey {
                label: metric.field(policy.ownership).into(),
                occurrence: 0,
            },
        };
        let mut cell = public_claim(
            &workbook,
            "YTD",
            "B5",
            key,
            PublicValue::Number {
                stored: "400.4".into(),
                displayed: Ok("400".into()),
                under_status: None,
            },
        );
        if metric == PublicMetric::AiscUsdPerOz {
            cell.label_as_printed = Some("All-in sustaining costs ($/oz)".into());
        }
        (policy, id, registered, cell)
    }

    fn reseal(cell: &mut KeyedCell) {
        let c = &cell.claim;
        cell.claim = Claim::new(
            &cell.key.line.label,
            c.value.clone(),
            &canonical_value_tag(&cell.key, &c.value),
            c.representation.clone(),
            c.support,
            c.additivity,
            c.evidence,
            c.epistemic,
            c.state,
            c.confidentiality.clone(),
            c.uu_terms.clone(),
            c.parents.clone(),
            c.anchors.clone(),
        );
    }

    #[test]
    fn original_displayed_number_and_source_identity_survive_the_receipt() {
        for metric in [PublicMetric::ProductionKoz, PublicMetric::AiscUsdPerOz] {
            let (policy, id, source, cell) = example(metric);
            let result = bind(&source, &policy, id, std::slice::from_ref(&cell)).expect("binding");
            assert_eq!(result.value, Rat::from_integer(400));
            assert_eq!(result.stored, "400.4");
            assert_eq!(result.source.claim.claim_id, cell.claim.claim_id);
            assert_eq!(
                result.available_at_ns,
                CivilDate::new(2031, 2, 16).expect("date").to_epoch_days() * NS_PER_DAY
            );
            assert_eq!(result.published_at, "2031-02-15");
            assert_eq!(
                result.published_at_basis,
                "FACT: invented test release date"
            );
            let body = receipt_claim_body(
                &policy,
                id,
                &result.citation,
                &result.stored,
                &result.displayed,
                result.ownership_source,
                &cell.claim.anchors[0].artifact_path,
            )
            .expect("reconstructed original source");
            assert_eq!(body, cell.claim.canonical_body());
            assert_ne!(
                receipt_claim_body(
                    &policy,
                    id,
                    &result.citation,
                    &result.stored,
                    "401",
                    result.ownership_source,
                    &cell.claim.anchors[0].artifact_path
                )
                .expect("different assertion"),
                body
            );
            assert_ne!(
                receipt_claim_body(
                    &policy,
                    id,
                    &result.citation,
                    "400.5",
                    &result.displayed,
                    result.ownership_source,
                    &cell.claim.anchors[0].artifact_path
                )
                .expect("different stored token"),
                body
            );
        }
    }

    #[test]
    fn frozen_target_fields_cannot_be_chosen_at_resolution() {
        let (policy, id, source, cell) = example(PublicMetric::AiscUsdPerOz);
        let mut variants = vec![policy.clone(); 5];
        variants[0].issuer = "different".into();
        variants[1].members.insert(id, "Other Mine".into());
        variants[2].period = PublicPeriod::Quarter {
            year: 2030,
            quarter: 4,
        };
        variants[3].metric = PublicMetric::ProductionKoz;
        variants[4].ownership = PublicOwnership::Attributable { num: 4, den: 5 };
        for changed in variants {
            assert!(bind(&source, &changed, id, std::slice::from_ref(&cell)).is_err());
        }
        let other = ObjectId::from_canonical_bytes(PrefixKind::Forecast, b"different forecast");
        assert!(bind(&source, &policy, other, std::slice::from_ref(&cell)).is_err());
        let mut later = cell.clone();
        later.key.reporting_vintage.release = "2031Q1".into();
        reseal(&mut later);
        assert_eq!(
            bind(&source, &policy, id, &[later])
                .expect_err("no comparative fallback")
                .code,
            "FMINE-REF-201"
        );
        assert_eq!(
            bind(&source, &policy, id, &[cell.clone(), cell])
                .expect_err("ambiguous sources")
                .code,
            "FMINE-REF-104"
        );
    }

    #[test]
    fn mutated_source_or_wrong_scope_never_becomes_an_outcome() {
        let (policy, id, source, cell) = example(PublicMetric::ProductionKoz);
        let mut changed = cell.clone();
        if let PublicValue::Number { displayed, .. } = &mut changed.claim.value {
            *displayed = Ok("999".into());
        }
        assert_eq!(
            bind(&source, &policy, id, &[changed])
                .expect_err("mutated value")
                .code,
            "FMINE-REF-507"
        );
        for modify in [
            (|c: &mut KeyedCell| c.key.unit = "usd_per_oz".into()) as fn(&mut KeyedCell),
            |c| c.key.scope = Scope::Company,
            |c| c.key.line.occurrence = 1,
        ] {
            let mut changed = cell.clone();
            modify(&mut changed);
            reseal(&mut changed);
            assert_eq!(
                bind(&source, &policy, id, &[changed])
                    .expect_err("wrong support")
                    .code,
                "FMINE-REF-105"
            );
        }
        let mut changed = cell.clone();
        changed.claim.confidentiality = Confidentiality::internal();
        reseal(&mut changed);
        assert_eq!(
            bind(&source, &policy, id, &[changed])
                .expect_err("nonpublic")
                .code,
            "FMINE-REF-301"
        );
        let mut changed = cell;
        changed.claim.stale = true;
        assert_eq!(
            bind(&source, &policy, id, &[changed])
                .expect_err("stale")
                .code,
            "FMINE-REF-502"
        );
        let (policy, id, source, mut cost) = example(PublicMetric::AiscUsdPerOz);
        cost.printed_basis = OwnershipBasis::Unprinted;
        assert_eq!(
            bind(&source, &policy, id, &[cost])
                .expect_err("unprinted cost basis")
                .code,
            "FMINE-REF-105"
        );
    }

    #[test]
    fn full_production_column_does_not_adopt_the_site_rows_attributable_share() {
        let (policy, id, source, mut cell) = example(PublicMetric::ProductionKoz);
        for printed in [
            OwnershipBasis::Unprinted,
            OwnershipBasis::Attributable {
                num: 3,
                den: 5,
                source: ShareSource::Printed,
            },
        ] {
            cell.printed_basis = printed;
            assert_eq!(
                bind(&source, &policy, id, std::slice::from_ref(&cell))
                    .expect("explicit 100% production column")
                    .value,
                Rat::from_integer(400)
            );
        }
    }

    #[test]
    fn wholly_owned_attributable_production_still_binds_the_attributable_column() {
        let (mut policy, id, source, mut cell) = example(PublicMetric::ProductionKoz);
        policy.ownership = PublicOwnership::Attributable { num: 1, den: 1 };
        assert!(bind(&source, &policy, id, std::slice::from_ref(&cell)).is_err());
        cell.key.line.label = policy.metric.field(policy.ownership).into();
        reseal(&mut cell);
        let out = bind(&source, &policy, id, std::slice::from_ref(&cell))
            .expect("explicit attributable column at a wholly owned site");
        assert_eq!(out.ownership_source, "full");
        assert_eq!(out.source.claim.claim_id, cell.claim.claim_id);
        assert_eq!(out.value, Rat::from_integer(400));
    }

    #[test]
    fn numeric_zero_is_preserved_and_unknown_or_status_values_refuse() {
        let (policy, id, source, cell) = example(PublicMetric::ProductionKoz);
        let mut zero = cell.clone();
        zero.claim.value = PublicValue::Number {
            stored: "0".into(),
            displayed: Ok("0".into()),
            under_status: None,
        };
        reseal(&mut zero);
        assert_eq!(
            bind(&source, &policy, id, &[zero])
                .expect("printed zero")
                .value,
            Rat::from_integer(0)
        );
        for value in [
            PublicValue::Number {
                stored: "0".into(),
                displayed: Ok("—".into()),
                under_status: None,
            },
            PublicValue::Number {
                stored: "2".into(),
                displayed: Ok("2".into()),
                under_status: Some(fmine_public::key::StatusKind::Withheld),
            },
            PublicValue::Missing,
        ] {
            let mut changed = cell.clone();
            changed.claim.value = value;
            reseal(&mut changed);
            assert!(bind(&source, &policy, id, &[changed]).is_err());
        }
        let mut changed = cell;
        changed.claim.evidence = EvidenceClass::Assumed;
        changed.claim.state = KnowledgeState::Unknown;
        reseal(&mut changed);
        assert!(bind(&source, &policy, id, &[changed]).is_err());
    }

    #[test]
    fn availability_requires_registered_publication_after_period_end() {
        let (policy, id, source, cell) = example(PublicMetric::ProductionKoz);
        for date in [None, Some("2030-12-31"), Some("2031-02-30")] {
            let mut changed = source.clone();
            changed.published_at = date.map(str::to_string);
            assert!(bind(&changed, &policy, id, std::slice::from_ref(&cell)).is_err());
        }
        let result = bind(&source, &policy, id, &[cell]).expect("valid publication");
        let mut citation = result.citation.clone();
        citation.locator = "Different.xlsx:YTD!B5".into();
        assert!(
            receipt_claim_body(
                &policy,
                id,
                &citation,
                &result.stored,
                &result.displayed,
                result.ownership_source,
                "corpus/Original.xlsx"
            )
            .is_err()
        );
        citation = result.citation;
        citation.vintage = "2031Q1".into();
        assert!(
            receipt_value_tag(
                &policy,
                id,
                &citation,
                &result.stored,
                &result.displayed,
                result.ownership_source
            )
            .is_err()
        );
        let mut synthetic = policy;
        synthetic.issuer = fmine_public::synthetic::SYNTHETIC_ISSUER.into();
        citation.source = synthetic.issuer.clone();
        citation.vintage = synthetic.period.original_release();
        assert!(
            receipt_claim_body(
                &synthetic,
                id,
                &citation,
                &result.stored,
                &result.displayed,
                result.ownership_source,
                "corpus/Original.xlsx",
            )
            .is_err()
        );
    }
}
