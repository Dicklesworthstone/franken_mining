//! Retrospective source binding for issuer guidance (BD-341).
//! Determinism: Exact (canonical source/score Claims and certified rational reference bounds).
//!
//! DESIGN: retain each agreed guidance document and missing outcome. A later comparative is never
//! relabelled as original publication. This adapter does not backdate registrations in the forecast ledger.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use fmine_calib::guidance_scores::{
    Guidance, GuidanceScore, GuidanceSummary, PrintedOutcome,
    claims::{
        self, GuidanceTarget, GuidanceValue, MemberInput, OutcomeValue, RangeScore, SeriesKey,
    },
};
use fmine_claim::{AdditivityClass, Claim, Confidentiality, Representation, SpatialSupport};
use fmine_core::anchor::SourceAnchor;
use fmine_core::codec::Encoder;
use fmine_core::hash::{Digest32, blake3_hash, sha256};
use fmine_evidence::{
    EpistemicStatus, EvidenceClass, KnowledgeState, KuKind, Refusal, RefusalCode,
};
use fmine_numerics::exact_feas::{Convention, Interval, Rat};
use fmine_public::key::{
    KeyedCell, OwnershipBasis, PeriodKind, PublicValue, Scope, StatusKind, SubjectStatus,
    canonical_value_tag,
};
use fmine_public::sources::{IssuerProfile, RegisteredSource, SourceKind, load_issuer_profile};
use fmine_public::transcribe::{CheckedDocument, TranscriptionReport};
use fmine_registry::sources::is_date;
use fmine_registry::transcriptions::{Accepted, Agreement, Decimal, Locator, Token, parse_token};

fn js(text: &str) -> String {
    format!("\"{}\"", crate::escape_json(text))
}

pub const NO_CLAIM: &str = "Retrospective description of cited issuer guidance and reported outcomes. Any price sensitivity remains paired with its unconditional score. No forecast registration, calibration verdict, ranking, inferred probability or causal attribution.";
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
        /// Selected source evidence remains inspectable even when it cannot supply a scored number.
        sources: Vec<KeyedCell>,
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
    /// Source statement identity includes the registered publication date and the agreed interpretation.
    pub guidance_source: Claim<Accepted>,
    pub member: MemberInput,
    pub score_claim: Option<Claim<RangeScore>>,
    /// Display projection of score_claim; source-bound arithmetic is performed exactly once.
    pub score: Option<GuidanceScore>,
    /// Paired AISC sensitivity or a cited unsplit reason; production has no price-sensitivity model.
    pub price_sensitivity: Option<crate::track_record_price::PriceAssessment>,
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
    pub claim: Claim<claims::GuidanceSummary>,
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

fn unresolved(refusal: Refusal, sources: Vec<KeyedCell>) -> OutcomeBinding {
    OutcomeBinding::Unresolved {
        code: refusal.code.into(),
        reason: refusal.what,
        sources,
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

pub(super) fn site_key(site: &str) -> String {
    site
        // The registered Mine Stats header prints Zaldívar; its transcription key is ASCII "zaldivar".
        .replace(['í', 'Í'], "i")
        .to_ascii_lowercase()
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

pub(super) fn cell_basis(basis: OwnershipBasis) -> Result<(bool, Rat)> {
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

pub(super) fn specification(a: &Accepted, name: &str) -> Result<((bool, Rat), String)> {
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

// Raw transcription bases stay on the source record; interpreted series use exact normalized shares.
fn series_basis(a: &Accepted, metric: &str) -> Result<String> {
    let ((attributable, share), _) = specification(a, metric)?;
    let mut basis = if attributable {
        format!("attributable:{}/{}", share.num(), share.den())
    } else {
        "100%".into()
    };
    if let Some((_, definition)) = a.basis.split_once(';') {
        basis.push(';');
        basis.push_str(definition);
    }
    Ok(basis)
}

pub(super) fn limits(a: &Accepted) -> Result<Guidance> {
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

fn source_digest(text: &str) -> Result<Digest32> {
    if text.len() != 64 || !text.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(failure(
            RefusalCode::DigestMismatch505,
            "invalid guidance source digest",
        ));
    }
    let mut bytes = [0; 32];
    for (i, byte) in bytes.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&text[2 * i..2 * i + 2], 16).map_err(|_| {
            failure(
                RefusalCode::DigestMismatch505,
                "invalid guidance source digest",
            )
        })?;
    }
    if bytes == [0; 32] {
        return Err(failure(
            RefusalCode::DigestMismatch505,
            "empty guidance source digest",
        ));
    }
    Ok(Digest32(bytes))
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

fn guidance_source(
    a: &Accepted,
    tag: &str,
    production: bool,
    synthetic: bool,
) -> Result<Claim<Accepted>> {
    Ok(Claim::new(
        "guidance.published_statement",
        a.clone(),
        tag,
        Representation::Deterministic,
        SpatialSupport::PeriodAggregate,
        if production {
            AdditivityClass::ExtensiveAdditive
        } else {
            AdditivityClass::IntensiveWeighted
        },
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
            source_digest(&a.doc_sha256)?.0,
            a.locator.to_string(),
        )],
    ))
}

fn scoring_member(
    source: &Claim<Accepted>,
    key: SeriesKey,
    guidance: Guidance,
    outcome: &OutcomeBinding,
    numeric: Option<PrintedOutcome>,
) -> Result<MemberInput> {
    let value = GuidanceValue {
        key: key.clone(),
        target: match guidance {
            Guidance::Range { lo, hi } => GuidanceTarget::Range { lo, hi },
            Guidance::Point { value } => GuidanceTarget::Point(value),
            Guidance::AtMost { upper } => GuidanceTarget::AtMost(upper),
            Guidance::AtLeast { lower } => GuidanceTarget::AtLeast(lower),
        },
    };
    let tag = value.canonical_tag();
    let guidance = interpreted(value, &tag, "guidance.interpreted_target", source);
    let outcome = match (outcome, numeric) {
        (OutcomeBinding::Resolved { cell, .. }, Some(numeric)) => {
            cell.claim.check_id()?;
            let value = OutcomeValue {
                key,
                point: numeric.value,
                rounding: numeric.interval,
            };
            let tag = value.canonical_tag();
            Some(interpreted(
                value,
                &tag,
                "guidance.interpreted_outcome",
                &cell.claim,
            ))
        }
        (OutcomeBinding::Unresolved { .. }, None) => None,
        _ => {
            return Err(rule_error(
                "outcome selection and numeric interpretation differ",
            ));
        }
    };
    Ok(MemberInput { guidance, outcome })
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
    cell.claim.check_id()?;
    if cell.claim.metric_key != cell.key.line.label
        || cell.claim.value_tag() != canonical_value_tag(&cell.key, &cell.claim.value)
    {
        return Err(failure(
            RefusalCode::FrozenRecordMutation507,
            format!(
                "{} key or interpreted value differs from its canonical Claim",
                cell.locator
            ),
        ));
    }
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
    if matches!(
        cell.claim.representation,
        Representation::Placeholder { .. }
    ) {
        return Err(failure(
            RefusalCode::SourceCellUnusable208,
            format!("{} is an unresolved placeholder", cell.locator),
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
    let blocked = if published_at > format!("{year:04}-12-31").as_str() {
        Some(failure(
            RefusalCode::AssayLookahead108,
            "guidance published after the outcome year; cannot establish a forward statement without an independently evidenced outcome-availability date",
        ))
    } else if let Some(status) = affected_status(&profile.subject_statuses, subject, year) {
        let reason = match status.kind {
            StatusKind::Disposed => "divested",
            StatusKind::Withheld => "definition_changed_by_withheld_period",
            StatusKind::CareAndMaintenance => "not_published_during_care_and_maintenance",
        };
        Some(failure(
            RefusalCode::NotObservable202,
            format!(
                "{reason}: annual support overlaps or adjoins the disposal boundary in {}",
                status.evidence
            ),
        ))
    } else {
        None
    };
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
        (
            &a.key.reporting_vintage.release,
            &a.locator,
            a.claim.claim_id,
        )
            .cmp(&(
                &b.key.reporting_vintage.release,
                &b.locator,
                b.claim.claim_id,
            ))
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
                blocked.unwrap_or_else(|| {
                    failure(
                        RefusalCode::NoClaimInDomain201,
                        "not_published: no registered annual outcome",
                    )
                }),
                vec![],
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
    // A timing or perimeter restriction blocks scoring, not visibility of the selected source evidence.
    if let Some(refusal) = blocked {
        return Ok((
            unresolved(refusal, first.into_iter().cloned().collect()),
            later,
            None,
        ));
    }
    let cell = match first.as_slice() {
        [one] => *one,
        [] => {
            return Ok((
                unresolved(
                    failure(
                        RefusalCode::NoClaimInDomain201,
                        "no annual summary cell in the original registered release; later comparatives cannot replace it",
                    ),
                    vec![],
                ),
                later,
                None,
            ));
        }
        _ => {
            return Ok((
                unresolved(
                    failure(
                        RefusalCode::NonIdentifiableAttribution104,
                        "multiple annual source cells in the selected release",
                    ),
                    first.into_iter().cloned().collect(),
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
        Err(e) => Ok((unresolved(e, vec![cell.clone()]), later, None)),
    }
}

fn assess(
    profile: &IssuerProfile,
    transcripts: &TranscriptionReport,
    cells: &[KeyedCell],
    site: Option<&str>,
) -> Result<TrackRecordReport> {
    assess_with_price_subject(profile, transcripts, cells, site, None)
}

fn assess_with_price_subject(
    profile: &IssuerProfile,
    transcripts: &TranscriptionReport,
    cells: &[KeyedCell],
    site: Option<&str>,
    price_subject: Option<&str>,
) -> Result<TrackRecordReport> {
    if !transcripts.agreement.is_complete_for_prefix("guidance_") {
        return Err(failure(
            RefusalCode::NotObservable202,
            "complete two-reader agreement before selecting the guidance population",
        ));
    }
    // The subject registry lists AISC table headers, which need not include copper sites. A canonical
    // site Claim from a registered Mine Stats workbook also establishes a subject; a transcription alone
    // cannot introduce one. Keep regions and company totals out of this site population.
    let mut registered_sites: BTreeSet<&str> =
        profile.subjects.iter().map(String::as_str).collect();
    for cell in cells {
        if registered_sites.contains(cell.key.subject.as_str())
            || cell.key.issuer != profile.issuer
            || !matches!(cell.key.scope, Scope::Site)
            || cell.claim.check_id().is_err()
            || cell.claim.value_tag()
                != fmine_public::key::canonical_value_tag(&cell.key, &cell.claim.value)
        {
            continue;
        }
        if profile.sources.iter().any(|source| {
            source.kind == SourceKind::MineStats
                && source.quarter == cell.key.reporting_vintage.release
                && source.sha256_hex == cell.key.reporting_vintage.file_sha256
                && cell.locator.split_once(':').is_some_and(|(file, _)| {
                    file == source.file_name
                        || Path::new(file) == Path::new(&profile.corpus_dir).join(&source.file_name)
                })
                && cell
                    .claim
                    .anchors
                    .iter()
                    .any(|anchor| Digest32(anchor.artifact_sha256).to_hex() == source.sha256_hex)
        }) {
            registered_sites.insert(&cell.key.subject);
        }
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
        let subjects: Vec<_> = registered_sites
            .iter()
            .copied()
            .filter(|s| site_key(s) == key && !profile.regions.iter().any(|region| region == s))
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
        let id = blake3_hash(&enc.finish()).to_hex();
        let guidance_source = guidance_source(
            a,
            &id,
            name.starts_with("production_"),
            profile.issuer == fmine_public::synthetic::SYNTHETIC_ISSUER,
        )?;
        let member = scoring_member(
            &guidance_source,
            SeriesKey {
                subject: (*subject).to_string(),
                metric: name.into(),
                unit: a.unit.clone(),
                basis: series_basis(a, name)?,
                year: period.year,
            },
            limits,
            &outcome,
            numeric,
        )?;
        let score_claim = member
            .outcome
            .as_ref()
            .map(|outcome| claims::score(&member.guidance, outcome))
            .transpose()?;
        let score = score_claim
            .as_ref()
            .map(|claim| GuidanceScore::from(&claim.value));
        rows.push(TrackRecordRow {
            id,
            site: (*subject).to_string(),
            metric: name.into(),
            year: period.year,
            guidance: a.clone(),
            published_at: doc.published_at.clone(),
            publication_basis: doc.published_at_basis.clone(),
            vintage: 0,
            comparative: comparative_population(profile, period.year),
            outcome,
            later_outcomes,
            guidance_source,
            member,
            score_claim,
            score,
            price_sensitivity: None,
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
    type Family = (String, String, String, String, u16);
    let mut seen = BTreeMap::<Family, usize>::new();
    let mut unique = BTreeSet::new();
    for row in &mut rows {
        let key = &row.member.guidance.value.key;
        let family = (
            key.subject.clone(),
            key.metric.clone(),
            key.unit.clone(),
            key.basis.clone(),
            key.year,
        );
        if !unique.insert((family.clone(), row.published_at.clone())) {
            return Err(failure(
                RefusalCode::NonIdentifiableAttribution104,
                "multiple guidance rows have the same site, metric, unit, ownership/definition basis, year and publication date; the earliest statement is ambiguous",
            ));
        }
        let ordinal = seen.entry(family).or_default();
        row.vintage = *ordinal;
        *ordinal += 1;
        if !row.metric.starts_with("production_") {
            row.price_sensitivity = Some(crate::track_record_price::assess(
                profile,
                transcripts,
                cells,
                row,
                profile.through.as_deref(),
                price_subject,
            ));
        }
    }
    type Group = (String, String, String, String, usize, bool);
    let mut groups = BTreeMap::<Group, Vec<&TrackRecordRow>>::new();
    for row in &rows {
        groups
            .entry((
                row.site.clone(),
                row.metric.clone(),
                row.member.guidance.value.key.unit.clone(),
                row.member.guidance.value.key.basis.clone(),
                row.vintage,
                row.comparative,
            ))
            .or_default()
            .push(row);
    }
    let mut summaries = Vec::new();
    for ((site, metric, unit, basis, vintage, comparative), rows) in groups {
        let members: Vec<_> = rows.iter().map(|row| row.member.clone()).collect();
        let claim = claims::summarize_history(&members)?;
        let scores = GuidanceSummary::from(&claim.value);
        summaries.push(TrackRecordSummary {
            site,
            metric,
            unit,
            basis,
            vintage,
            comparative,
            members: rows.iter().map(|row| row.id.clone()).collect(),
            claim,
            scores,
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
    run_with_price_subject(root, issuer, site, through, None)
}

pub fn run_with_price_subject(
    root: &Path,
    issuer: &str,
    site: Option<&str>,
    through: Option<&str>,
    price_subject: Option<&str>,
) -> Result<TrackRecordReport> {
    let mut profile = load_issuer_profile(root, issuer)?;
    if let Some(bound) = through {
        profile = profile.through(bound)?;
    }
    let claims = fmine_public::key::mine_stats::load_claims(&profile, root)?;
    let transcripts=fmine_public::transcribe::load_guidance_transcription_report(&profile,root,None)?.ok_or_else(||failure(RefusalCode::NoClaimInDomain201,format!("registries/transcriptions/{issuer}.toml is absent; register guidance documents and agreed readings")))?;
    let mut report =
        assess_with_price_subject(&profile, &transcripts, &claims.claims, site, price_subject)?;
    report.source_refusals = claims.refusals;
    report.digest = blake3_hash(report.body().as_bytes());
    Ok(report)
}

fn claim_json<T>(claim: &Claim<T>, value: &str) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut body = String::new();
    for byte in claim.canonical_body() {
        body.push(char::from(HEX[usize::from(byte >> 4)]));
        body.push(char::from(HEX[usize::from(byte & 15)]));
    }
    let parents = claim
        .parents
        .iter()
        .map(|id| js(&id.to_string()))
        .collect::<Vec<_>>()
        .join(",");
    let anchors = claim
        .anchors
        .iter()
        .map(|anchor| {
            format!(
                "{{\"file\":{},\"path\":{},\"sha256\":{},\"locator\":{}}}",
                js(&anchor.artifact_path),
                js(&anchor.artifact_path),
                js(&Digest32(anchor.artifact_sha256).to_hex()),
                js(&anchor.locator)
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let uu_terms = claim.uu_terms.iter().map(|term| format!(
        "{{\"scenario_id\":{},\"description\":{},\"downside_exposure_usd_m\":{},\"trigger_indicator\":{}}}",
        js(&term.scenario_id), js(&term.description),
        if term.downside_exposure_usd_m.is_finite() { term.downside_exposure_usd_m.to_string() } else { "null".into() },
        js(&term.trigger_indicator)
    )).collect::<Vec<_>>().join(",");
    format!(
        "{{\"claim_id\":{},\"metric\":{},\"metric_key\":{},\"value_tag\":{},\"canonical_body_hex\":{},\"value\":{value},\"representation\":{},\"support\":{},\"additivity\":{},\"evidence\":{},\"epistemic\":{},\"knowledge\":{},\"confidentiality\":{},\"public\":{},\"stale\":{},\"calibration_failed\":{},\"uu_terms\":[{uu_terms}],\"parents\":[{parents}],\"anchors\":[{anchors}]}}",
        js(&claim.claim_id.to_string()),
        js(&claim.metric_key),
        js(&claim.metric_key),
        js(claim.value_tag()),
        js(&body),
        js(&format!("{:?}", claim.representation)),
        js(&format!("{:?}", claim.support)),
        js(&format!("{:?}", claim.additivity)),
        js(&format!("{:?}", claim.evidence)),
        js(&format!("{:?}", claim.epistemic)),
        js(&format!("{:?}", claim.state)),
        js(&claim.confidentiality.to_string()),
        claim.confidentiality.is_public(),
        claim.stale,
        claim.calibration_failed,
    )
}

fn rational_json(value: Rat) -> String {
    format!(
        "{{\"num\":\"{}\",\"den\":\"{}\"}}",
        value.num(),
        value.den()
    )
}

fn interval_json(value: Interval) -> String {
    format!(
        "{{\"lo\":{},\"hi\":{},\"lo_strict\":{},\"hi_strict\":{}}}",
        rational_json(value.lo),
        rational_json(value.hi),
        value.lo_strict,
        value.hi_strict
    )
}

fn key_json(key: &SeriesKey) -> String {
    format!(
        "{{\"subject\":{},\"metric\":{},\"unit\":{},\"basis\":{},\"year\":{}}}",
        js(&key.subject),
        js(&key.metric),
        js(&key.unit),
        js(&key.basis),
        key.year
    )
}

fn guidance_claim_json(claim: &Claim<GuidanceValue>) -> String {
    let target = match claim.value.target {
        GuidanceTarget::Range { lo, hi } => format!(
            "{{\"kind\":\"range\",\"lo\":{},\"hi\":{}}}",
            rational_json(lo),
            rational_json(hi)
        ),
        GuidanceTarget::Point(value) => {
            format!("{{\"kind\":\"point\",\"value\":{}}}", rational_json(value))
        }
        GuidanceTarget::AtMost(upper) => {
            format!(
                "{{\"kind\":\"at_most\",\"upper\":{}}}",
                rational_json(upper)
            )
        }
        GuidanceTarget::AtLeast(lower) => {
            format!(
                "{{\"kind\":\"at_least\",\"lower\":{}}}",
                rational_json(lower)
            )
        }
    };
    claim_json(
        claim,
        &format!(
            "{{\"key\":{},\"target\":{target}}}",
            key_json(&claim.value.key)
        ),
    )
}

fn outcome_claim_json(claim: &Claim<OutcomeValue>) -> String {
    let value = &claim.value;
    claim_json(
        claim,
        &format!(
            "{{\"key\":{},\"point\":{},\"rounding\":{{\"lo\":{},\"hi\":{},\"lo_strict\":{},\"hi_strict\":{}}}}}",
            key_json(&value.key),
            rational_json(value.point),
            rational_json(value.rounding.lo),
            rational_json(value.rounding.hi),
            value.rounding.lo_strict,
            value.rounding.hi_strict,
        ),
    )
}

fn score_claim_json(claim: &Claim<RangeScore>) -> String {
    let r = |value: Option<Rat>| value.map_or_else(|| "null".into(), rational_json);
    let hit = match claim.value.hit {
        claims::Hit::CertainHit => "certain_hit",
        claims::Hit::CertainMiss => "certain_miss",
        claims::Hit::Boundary => "boundary",
    };
    claim_json(
        claim,
        &format!(
            "{{\"midpoint\":{},\"half_width\":{},\"z\":{},\"z_bounds\":{},\"side\":{},\"boundary\":{},\"hit\":{},\"above_midpoint\":{}}}",
            r(claim.value.midpoint),
            r(claim.value.half_width),
            r(claim.value.z),
            claim
                .value
                .z_bounds
                .map_or_else(|| "null".into(), interval_json),
            js(claim.value.side.as_str()),
            claim.value.boundary,
            js(hit),
            claim
                .value
                .above_midpoint
                .map_or_else(|| "null".into(), |b| b.to_string())
        ),
    )
}

impl TrackRecordRow {
    /// Preserve the published row-level Claim view using the same canonical inputs and score.
    fn claims_json(&self) -> String {
        let guidance = guidance_claim_json(&self.member.guidance);
        let outcome = self
            .member
            .outcome
            .as_ref()
            .map_or_else(|| "null".into(), outcome_claim_json);
        let score = self
            .score_claim
            .as_ref()
            .map_or_else(|| "null".into(), score_claim_json);
        format!(
            "{{\"guidance\":{guidance},\"outcome\":{outcome},\"score\":{score},\"score_refusal\":null}}"
        )
    }
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
        let rows = self.rows.iter().map(|r| {
            let mut inputs = vec![r.guidance.locator.to_string()];
            let outcome = match &r.outcome {
                OutcomeBinding::Resolved { cell, comparative } => {
                    inputs.push(cell.locator.clone());
                    format!("{{\"state\":{},\"source\":{}}}", js(if *comparative {
                        "resolved_on_comparative_vintage"
                    } else { "resolved_first_published" }), cell.to_json())
                }
                OutcomeBinding::Unresolved { code, reason, sources } => {
                    inputs.extend(sources.iter().map(|c| c.locator.clone()));
                    format!("{{\"state\":\"unresolved\",\"code\":{},\"reason\":{},\"sources\":[{}]}}",
                        js(code), js(reason), sources.iter().map(KeyedCell::to_json).collect::<Vec<_>>().join(","))
                }
            };
            let score = format!("{{\"rule\":\"guidance.range_score\",\"rules_version\":\"guidance-track-record/2\",\"in\":[{}],\"value\":{},\"claim\":{}}}",
                inputs.iter().map(|s| js(s)).collect::<Vec<_>>().join(","),
                r.score.as_ref().map_or_else(|| "null".into(), GuidanceScore::to_json),
                r.score_claim.as_ref().map_or_else(|| "null".into(), |c| claim_json(c, &GuidanceScore::from(&c.value).to_json())));
            let source_value = format!("{{\"statement_id\":{},\"locator\":{},\"sha256\":{},\"token\":{},\"quantity\":{},\"unit\":{},\"basis\":{},\"period\":{},\"published_at\":{}}}",
                js(&r.id), js(&r.guidance.locator.to_string()), js(&r.guidance.doc_sha256), js(&r.guidance.token),
                js(&r.guidance.quantity), js(&r.guidance.unit), js(&r.guidance.basis), js(&r.guidance.period), js(&r.published_at));
            let price = r.price_sensitivity.as_ref().map_or_else(|| "null".into(), crate::track_record_price::PriceAssessment::to_json);
            format!("{{\"id\":{},\"site\":{},\"metric\":{},\"year\":{},\"guidance_vintage_ordinal\":{},\"published_at\":{},\"published_at_basis\":{},\"guidance\":{{\"at\":{},\"sha256\":{},\"token\":{},\"unit\":{},\"basis\":{},\"period\":{},\"reader_count\":{},\"source_claim\":{},\"claim\":{}}},\"outcome\":{outcome},\"later_outcomes\":[{}],\"interpreted_outcome_claim\":{},\"score\":{score},\"price_sensitivity\":{price},\"claims\":{}}}",
                js(&r.id), js(&r.site), js(&r.metric), r.year, r.vintage, js(&r.published_at), js(&r.publication_basis),
                js(&r.guidance.locator.to_string()), js(&r.guidance.doc_sha256), js(&r.guidance.token), js(&r.guidance.unit),
                js(&r.guidance.basis), js(&r.guidance.period), r.guidance.readers.len(),
                claim_json(&r.guidance_source, &source_value), guidance_claim_json(&r.member.guidance),
                r.later_outcomes.iter().map(KeyedCell::to_json).collect::<Vec<_>>().join(","),
                r.member.outcome.as_ref().map_or_else(|| "null".into(), outcome_claim_json), r.claims_json())
        }).collect::<Vec<_>>().join(",");
        let summaries = self.summaries.iter().map(|s| {
            let scope = match &s.claim.value.scope {
                claims::SummaryScope::Annual { year } => format!("{{\"kind\":\"annual\",\"year\":{year}}}"),
                claims::SummaryScope::SiteHistory { subject, basis, years } => format!(
                    "{{\"kind\":\"site_history\",\"subject\":{},\"basis\":{},\"years\":[{}]}}",
                    js(subject), js(basis), years.iter().map(u16::to_string).collect::<Vec<_>>().join(",")),
            };
            let claim_value = format!("{{\"metric\":{},\"unit\":{},\"scope\":{scope},\"scores\":{}}}", js(&s.metric), js(&s.unit), s.scores.to_json());
            format!("{{\"site\":{},\"metric\":{},\"unit\":{},\"basis\":{},\"guidance_vintage_ordinal\":{},\"outcome_selector\":{},\"in\":[{}],\"rule\":\"guidance.descriptive_summary\",\"rules_version\":\"guidance-track-record/2\",\"scores\":{},\"claim\":{}}}",js(&s.site),js(&s.metric),js(&s.unit),js(&s.basis),s.vintage,js(if s.comparative {"comparative_only"} else {"first_published"}),s.members.iter().map(|id|js(id)).collect::<Vec<_>>().join(","),s.scores.to_json(),claim_json(&s.claim, &claim_value))
        }).collect::<Vec<_>>().join(",");
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
    let source_bytes = b"invented FY2030 production 125 koz attributable 4/5";
    let digest = sha256(source_bytes);
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
    let value = PublicValue::Number {
        stored: "125".into(),
        displayed: Ok("125".into()),
        under_status: None,
    };
    let tag = canonical_value_tag(&key, &value);
    let claim = Claim::new(
        &key.line.label,
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
    // The shared in-memory synthetic profile intentionally has no registered sources. This fixture
    // declares its own original release so withholding that outcome cannot select a later comparative.
    profile.corpus_dir = "synthetic/2030Q4".into();
    profile.sources.push(RegisteredSource {
        file_name: "production.txt".into(),
        kind: SourceKind::MineStats,
        quarter: "2030Q4".into(),
        url: "https://example.invalid/2030Q4/production.txt".into(),
        url_basis: "DESIGN: invented in-memory source".into(),
        bytes: source_bytes.len() as u64,
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
        let value = PublicValue::Number {
            stored: token.into(),
            displayed: Ok(token.into()),
            under_status: None,
        };
        let tag = canonical_value_tag(&cell.key, &value);
        cell.claim = Claim::new(
            &cell.key.line.label,
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
            vec![SourceAnchor::new(path, digest.0, "Annual!C1")],
        );
        cell
    }

    // Intentional source changes construct a new Claim; mutation tests deliberately skip this.
    fn reconstruct(cell: &mut KeyedCell) {
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

    fn refresh_source_claim(cell: &mut KeyedCell) {
        let old = &cell.claim;
        cell.claim = Claim::new(
            &cell.key.line.label,
            old.value.clone(),
            &fmine_public::key::canonical_value_tag(&cell.key, &old.value),
            old.representation.clone(),
            old.support,
            old.additivity,
            old.evidence,
            old.epistemic,
            old.state,
            old.confidentiality.clone(),
            old.uu_terms.clone(),
            old.parents.clone(),
            old.anchors.clone(),
        );
    }

    #[test]
    fn accented_registered_site_binds_ascii_guidance_and_site_selections() {
        let (mut profile, mut transcripts, mut cells) = synthetic_evidence().expect("fixture");
        profile.subjects[0] = "Zaldívar".into();
        for guidance in &mut transcripts.agreement.accepted {
            guidance.quantity = guidance.quantity.replace("example_mine", "zaldivar");
            guidance.locator.row = "Zaldívar".into();
        }
        cells[0].key.subject = "Zaldívar".into();
        refresh_source_claim(&mut cells[0]);
        let report =
            assess(&profile, &transcripts, &cells, None).expect("registered accented site");
        assert_eq!(report.rows.len(), 2);
        assert_eq!(production(&report).site, "Zaldívar");
        assert_eq!(
            production(&report).score.expect("source score").z,
            Some(Rat::new(3, 2).expect("z"))
        );
        for site in ["zaldivar", "Zaldívar", "ZALDÍVAR"] {
            assert_eq!(
                report.to_json(),
                assess(&profile, &transcripts, &cells, Some(site))
                    .expect("equivalent site selection")
                    .to_json()
            );
        }
    }

    #[test]
    fn registered_copper_claim_declares_site_outside_aisc_headers() {
        let (mut profile, mut transcripts, mut cells) = synthetic_evidence().expect("fixture");
        profile.subjects.clear();
        transcripts
            .agreement
            .accepted
            .retain(|a| a.quantity.ends_with("production_koz"));
        let guidance = &mut transcripts.agreement.accepted[0];
        guidance.quantity = "guidance_example_copper_production_kt".into();
        guidance.unit = "kt".into();
        guidance.locator.row = "Example Copper".into();
        cells[0].key.subject = "Example Copper".into();
        cells[0].key.line.label = "summary:cu_prod_attr_kt".into();
        cells[0].key.unit = "kt".into();
        refresh_source_claim(&mut cells[0]);
        let report = assess(&profile, &transcripts, &cells, Some("example-copper"))
            .expect("site declared by a registered canonical copper source");
        assert_eq!(report.rows.len(), 1);
        assert_eq!(production(&report).site, "Example Copper");
        assert_eq!(
            production(&report).score.expect("copper score").z,
            Some(Rat::new(3, 2).expect("z"))
        );

        let mut unregistered = profile.clone();
        unregistered.sources.clear();
        assert_eq!(
            assess(&unregistered, &transcripts, &cells, None)
                .expect_err("an unregistered source cannot introduce a site")
                .code,
            RefusalCode::NoClaimInDomain201.as_str()
        );
        let mut altered = cells.clone();
        altered[0].claim.value = PublicValue::Missing;
        assert_eq!(
            assess(&profile, &transcripts, &altered, None)
                .expect_err("a changed canonical source cannot introduce a site")
                .code,
            RefusalCode::NoClaimInDomain201.as_str()
        );
        cells[0].key.scope = Scope::Company;
        refresh_source_claim(&mut cells[0]);
        assert_eq!(
            assess(&profile, &transcripts, &cells, None)
                .expect_err("a company total cannot introduce a site")
                .code,
            RefusalCode::NoClaimInDomain201.as_str()
        );
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
    fn canonical_score_and_summary_preserve_source_ancestry_and_uu_terms() {
        let (profile, transcripts, mut cells) = synthetic_evidence().expect("fixture");
        let term = fmine_claim::UuTerm {
            scenario_id: "synthetic-reporting-change".into(),
            description: "Invented structural reporting uncertainty".into(),
            downside_exposure_usd_m: 0.0,
            trigger_indicator: "synthetic restatement".into(),
        };
        cells[0].claim.uu_terms.push(term.clone());
        let report = assess(&profile, &transcripts, &cells, None).expect("canonical report");
        let row = production(&report);
        let score = row.score_claim.as_ref().expect("canonical score");
        let outcome = row.member.outcome.as_ref().expect("canonical outcome");
        row.member.guidance.check_id().expect("input identity");
        assert_eq!(
            row.member.guidance.value_tag(),
            row.member.guidance.value.canonical_tag()
        );
        outcome.check_id().expect("outcome identity");
        score.check_id().expect("score identity");
        assert_eq!(score.value_tag(), score.value.canonical_tag());
        assert_eq!(outcome.parents, vec![cells[0].claim.claim_id]);
        assert_eq!(
            row.member.guidance.parents,
            vec![row.guidance_source.claim_id]
        );
        assert!(score.parents.contains(&row.member.guidance.claim_id));
        assert!(score.parents.contains(&outcome.claim_id));
        assert_eq!(score.uu_terms, vec![term.clone()]);
        assert_eq!(score.evidence, EvidenceClass::Assumed);
        assert_eq!(row.score, Some(GuidanceScore::from(&score.value)));
        let summary = report
            .summaries
            .iter()
            .find(|s| s.metric == "production_koz")
            .expect("summary");
        summary.claim.check_id().expect("summary identity");
        assert_eq!(
            summary.claim.value_tag(),
            summary.claim.value.canonical_tag()
        );
        assert_eq!(summary.claim.uu_terms, vec![term]);
        assert_eq!(
            summary.scores.iid_reference_interval_90_exact,
            summary.claim.value.binomial_90_exact
        );
        let missing = report
            .rows
            .iter()
            .find(|r| r.metric == "aisc_usd_per_oz")
            .expect("unresolved cost");
        let missing_summary = report
            .summaries
            .iter()
            .find(|s| s.metric == "aisc_usd_per_oz")
            .expect("unresolved summary");
        assert!(
            missing_summary
                .claim
                .parents
                .contains(&missing.member.guidance.claim_id)
        );
        assert_eq!(
            (
                missing_summary.claim.value.members,
                missing_summary.claim.value.unresolved
            ),
            (1, 1)
        );
        let json = report.to_json();
        for id in [
            row.guidance_source.claim_id,
            row.member.guidance.claim_id,
            outcome.claim_id,
            score.claim_id,
            summary.claim.claim_id,
        ] {
            assert!(json.contains(&format!("\"claim_id\":\"{id}\"")));
        }
        assert!(json.contains("synthetic-reporting-change"));
        let markdown = crate::track_record::render(&report);
        assert!(markdown.contains(&score.claim_id.to_string()));
        assert!(markdown.contains(&summary.claim.claim_id.to_string()));
        assert!(markdown.contains("Certified outward rational enclosure"));
    }

    #[test]
    fn publication_date_correction_changes_claim_ancestry_without_changing_arithmetic() {
        let (profile, mut transcripts, cells) = synthetic_evidence().expect("fixture");
        let before = assess(&profile, &transcripts, &cells, None).expect("first interpretation");
        transcripts.documents[0].published_at = "2030-01-16".into();
        let after =
            assess(&profile, &transcripts, &cells, None).expect("corrected publication date");
        let (first, next) = (production(&before), production(&after));
        assert_eq!(first.score, next.score);
        assert_ne!(
            first.guidance_source.claim_id,
            next.guidance_source.claim_id
        );
        assert_ne!(
            first.member.guidance.claim_id,
            next.member.guidance.claim_id
        );
        assert_ne!(
            first.score_claim.as_ref().unwrap().claim_id,
            next.score_claim.as_ref().unwrap().claim_id
        );
        assert_ne!(before.digest, after.digest);
    }

    #[test]
    fn rejected_original_retains_source_bytes_identity_and_later_appearance() {
        let (p, t, mut cells) = synthetic_evidence().expect("fixture");
        let later = revised(&cells[0], 2030, "2031Q1", "110");
        cells[0].claim.stale = true;
        let original = cells[0].clone();
        cells.push(later.clone());
        let report = assess(&p, &t, &cells, None).expect("unscored original");
        let row = production(&report);
        let OutcomeBinding::Unresolved { code, sources, .. } = &row.outcome else {
            panic!("original must stay unresolved");
        };
        assert_eq!(code, "FMINE-REF-502");
        assert_eq!(sources.len(), 1);
        assert_eq!(sources[0].to_json(), original.to_json());
        assert_eq!(row.later_outcomes[0].to_json(), later.to_json());
        assert!(row.score.is_none());
        let summary = report
            .summaries
            .iter()
            .find(|s| s.metric == "production_koz")
            .expect("summary");
        assert_eq!(
            (
                summary.scores.n_members,
                summary.scores.n_resolved,
                summary.scores.n_unresolved
            ),
            (1, 0, 1)
        );
        let json = report.to_json();
        assert!(json.contains(&format!("\"sources\":[{}]", original.to_json())));
        let markdown = crate::track_record::render(&report);
        assert!(markdown.contains("Rejected outcome evidence"));
        assert!(markdown.contains(&original.claim.claim_id.to_string()));
        assert!(markdown.contains(&original.key.reporting_vintage.file_sha256));
    }

    #[test]
    fn ambiguous_selected_sources_are_all_cited_in_canonical_order() {
        let (p, t, mut cells) = synthetic_evidence().expect("fixture");
        let conflicting = revised(&cells[0], 2030, "2030Q4", "110");
        cells.push(conflicting);
        let report = assess(&p, &t, &cells, None).expect("ambiguous originals");
        let row = production(&report);
        let OutcomeBinding::Unresolved { code, sources, .. } = &row.outcome else {
            panic!("simultaneous assertions must remain unresolved");
        };
        assert_eq!(code, "FMINE-REF-104");
        assert_eq!(sources.len(), 2);
        assert!(row.score.is_none());
        let json = report.to_json();
        let markdown = crate::track_record::render(&report);
        for source in &cells {
            assert!(sources.iter().any(|s| s.to_json() == source.to_json()));
            assert!(json.contains(&source.to_json()));
            assert!(markdown.contains(&source.claim.claim_id.to_string()));
            assert!(markdown.contains(&source.key.reporting_vintage.file_sha256));
        }
        cells.reverse();
        assert_eq!(
            json,
            assess(&p, &t, &cells, None).expect("reordered").to_json()
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
            reconstruct(&mut c[0]);
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
            let OutcomeBinding::Unresolved { sources, .. } = &production(&r).outcome else {
                panic!("restricted outcome");
            };
            assert_eq!(sources.len(), 1);
            assert_eq!(sources[0].to_json(), c[0].to_json());
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
        reconstruct(&mut actuals[1]);
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
        reconstruct(&mut c[0]);
        let good = assess(&p, &t, &c, None).expect("copper source");
        assert_eq!(
            production(&good).score.expect("score").z,
            Some(Rat::new(3, 2).expect("z"))
        );
        c[0].key.unit = "Mlb".into();
        reconstruct(&mut c[0]);
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

    fn multi_basis_evidence() -> (IssuerProfile, TranscriptionReport, Vec<KeyedCell>) {
        let (p, mut t, mut cells) = synthetic_evidence().expect("fixture");
        t.agreement
            .accepted
            .retain(|a| a.quantity.ends_with("production_koz"));
        let mut full = t.agreement.accepted[0].clone();
        full.locator.col = "100% production".into();
        full.basis = "100%".into();
        full.token = "140 - 160".into();
        full.value = parse_token(&full.token).expect("full-mine guidance");
        t.agreement.accepted.push(full);
        let mut actual = cells[0].clone();
        actual.key.ownership_basis = OwnershipBasis::Consolidated100;
        actual.printed_basis = OwnershipBasis::Consolidated100;
        actual.key.line.label = "summary:au_prod_100_koz".into();
        actual.locator = "synthetic/2030Q4/production.txt:Annual!B1".into();
        actual.claim.anchors[0].locator = "Annual!B1".into();
        actual.claim.value = PublicValue::Number {
            stored: "150".into(),
            displayed: Ok("150".into()),
            under_status: None,
        };
        reconstruct(&mut actual);
        cells.push(actual);
        (p, t, cells)
    }

    #[test]
    fn same_date_full_and_attributable_guidance_keep_independent_scores() {
        let (p, t, cells) = multi_basis_evidence();
        let report = assess(&p, &t, &cells, None).expect("independent ownership series");
        assert_eq!(report.rows.len(), 2);
        assert_eq!(report.summaries.len(), 2);
        for row in &report.rows {
            assert_eq!(row.vintage, 0);
            let (source, side) = if row.member.guidance.value.key.basis == "100%" {
                (&cells[1], Side::Inside)
            } else {
                assert_eq!(row.member.guidance.value.key.basis, "attributable:4/5");
                (&cells[0], Side::Above)
            };
            assert_eq!(row.score.expect("separate score").side, side);
            let outcome = row.member.outcome.as_ref().expect("interpreted outcome");
            assert_eq!(outcome.parents, vec![source.claim.claim_id]);
            row.score_claim
                .as_ref()
                .expect("score Claim")
                .check_id()
                .expect("canonical");
        }
        assert!(
            report
                .summaries
                .iter()
                .all(|s| s.vintage == 0 && s.scores.n_members == 1 && s.scores.n_resolved == 1)
        );
        let mut attributable_all = t.agreement.accepted[0].clone();
        attributable_all.basis = "attributable:1/1".into();
        assert_ne!(
            series_basis(&attributable_all, "production_koz").expect("attributable"),
            series_basis(&t.agreement.accepted[1], "production_koz").expect("full")
        );
    }

    #[test]
    fn ownership_series_revision_ordinals_do_not_interleave() {
        let (p, mut t, mut cells) = multi_basis_evidence();
        // The full-mine series starts later, but its first statement is still vintage zero.
        for (index, file, date) in [
            (1, "full-initial.txt", "2030-03-01"),
            (0, "attributable-revision.txt", "2030-06-01"),
            (1, "full-revision.txt", "2030-09-01"),
        ] {
            let mut doc = t.documents[0].clone();
            doc.file = file.into();
            doc.published_at = date.into();
            let mut row = t.agreement.accepted[index].clone();
            row.locator.doc = file.into();
            t.documents.push(doc);
            if file == "full-initial.txt" {
                t.agreement.accepted[index] = row;
            } else {
                t.agreement.accepted.push(row);
            }
        }
        let report = assess(&p, &t, &cells, None).expect("both series with revisions");
        assert_eq!(report.rows.len(), 4);
        assert_eq!(report.summaries.len(), 4);
        for basis in ["100%", "attributable:4/5"] {
            let rows: Vec<_> = report
                .rows
                .iter()
                .filter(|r| r.member.guidance.value.key.basis == basis)
                .collect();
            assert_eq!(
                rows.iter().map(|r| r.vintage).collect::<Vec<_>>(),
                vec![0, 1]
            );
            for summary in report.summaries.iter().filter(|s| s.basis == basis) {
                assert_eq!(summary.scores.n_members, 1);
                assert_eq!(summary.members, vec![rows[summary.vintage].id.clone()]);
            }
        }
        t.agreement.accepted.reverse();
        t.documents.reverse();
        cells.reverse();
        assert_eq!(
            report.to_json(),
            assess(&p, &t, &cells, None).expect("permutation").to_json()
        );
    }

    #[test]
    fn equivalent_fraction_spellings_share_one_history_without_losing_source_basis() {
        let (mut p, mut t, cells) = synthetic_evidence().expect("fixture");
        t.agreement
            .accepted
            .retain(|a| a.quantity.ends_with("production_koz"));
        let mut doc = t.documents[0].clone();
        doc.file = "guidance-2031.txt".into();
        doc.published_at = "2031-01-15".into();
        let mut row = t.agreement.accepted[0].clone();
        row.locator.doc = doc.file.clone();
        row.period = "2031FY".into();
        row.basis = "attributable:8/10".into();
        t.documents.push(doc);
        t.agreement.accepted.push(row);
        let next = revised(&cells[0], 2031, "2031Q4", "115");
        let mut source = p.sources[0].clone();
        source.file_name = "2031.txt".into();
        source.quarter = "2031Q4".into();
        source.sha256_hex = next.key.reporting_vintage.file_sha256.clone();
        p.sources.push(source);
        let report = assess(&p, &t, &[cells[0].clone(), next], None).expect("canonical shares");
        assert_eq!(report.rows.len(), 2);
        assert_eq!(report.rows[1].guidance.basis, "attributable:8/10");
        assert!(
            report
                .rows
                .iter()
                .all(|r| r.member.guidance.value.key.basis == "attributable:4/5" && r.vintage == 0)
        );
        assert_eq!(report.summaries.len(), 1);
        let summary = &report.summaries[0];
        assert_eq!(summary.basis, "attributable:4/5");
        assert_eq!(
            (
                summary.scores.n_members,
                summary.scores.n_resolved,
                summary.scores.inside
            ),
            (2, 2, 1)
        );
        summary
            .claim
            .check_id()
            .expect("canonical combined history");
        let mut cost = t.agreement.accepted[0].clone();
        cost.unit = "usd_per_oz".into();
        cost.basis = "attributable:8/10;aisc=issuer-reported-per-sold-oz".into();
        assert_eq!(
            series_basis(&cost, "aisc_usd_per_oz").expect("cost definition"),
            "attributable:4/5;aisc=issuer-reported-per-sold-oz"
        );
    }

    #[test]
    fn same_date_equivalent_basis_duplicates_still_refuse() {
        let (p, t, cells) = synthetic_evidence().expect("fixture");
        for basis in ["attributable:4/5", "attributable:8/10"] {
            let mut transcripts = t.clone();
            let mut duplicate = transcripts.agreement.accepted[0].clone();
            duplicate.locator.col = "second attributable statement".into();
            duplicate.basis = basis.into();
            duplicate.token = "120 - 130".into();
            duplicate.value = parse_token(&duplicate.token).expect("conflicting statement");
            transcripts.agreement.accepted.push(duplicate);
            assert_eq!(
                assess(&p, &transcripts, &cells, None)
                    .expect_err("same-series ambiguity")
                    .code,
                "FMINE-REF-104"
            );
        }
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
    fn registered_cell_mutations_cannot_reuse_its_canonical_identity() {
        let root = registered_workspace();
        let p = load_issuer_profile(&root, "synthetic").expect("profile");
        let t = fmine_public::transcribe::load_transcription_report(&p, &root)
            .expect("checked transcripts")
            .expect("agreement");
        let source = fmine_public::key::mine_stats::load_claims(&p, &root)
            .expect("checked workbook")
            .claims;
        let i = source
            .iter()
            .position(|c| c.locator == "Annual.xlsx:YTD!C3")
            .expect("attributable production");
        let before = assess(&p, &t, &source, None).expect("canonical source");
        assert_eq!(
            production(&before).score.expect("score").z,
            Some(Rat::new(3, 2).unwrap())
        );
        source[i].claim.check_id().expect("source identity");
        assert_eq!(
            source[i].claim.value_tag(),
            canonical_value_tag(&source[i].key, &source[i].claim.value)
        );

        for mutation in 0..5 {
            let mut changed = source.clone();
            let cell = &mut changed[i];
            match mutation {
                0 => {
                    let PublicValue::Number { displayed, .. } = &mut cell.claim.value else {
                        panic!("numeric fixture")
                    };
                    *displayed = Ok("105".into());
                    // Claim::check_id alone cannot detect a displayed-value mutation.
                    cell.claim.check_id().expect("stored body unchanged");
                }
                1 => cell.key.line.occurrence += 1,
                2 => cell.claim.state = KnowledgeState::Known,
                3 => cell.claim.anchors[0].locator = "YTD!B3".into(),
                _ => cell.claim.metric_key = "another metric".into(),
            }
            let after = assess(&p, &t, &changed, None).expect("named unresolved member");
            assert!(production(&after).score.is_none(), "mutation {mutation}");
            assert!(matches!(&production(&after).outcome,
                OutcomeBinding::Unresolved { code, .. } if code == "FMINE-REF-507"));
            let summary = after
                .summaries
                .iter()
                .find(|s| s.metric == "production_koz")
                .unwrap();
            assert_eq!(
                (
                    summary.scores.n_members,
                    summary.scores.n_resolved,
                    summary.scores.n_unresolved
                ),
                (1, 0, 1)
            );
        }
    }

    #[test]
    fn numeric_placeholder_remains_unresolved_even_with_usable_knowledge_state() {
        let (p, t, mut c) = synthetic_evidence().expect("fixture");
        c[0].claim.representation = Representation::Placeholder {
            token: "N/A".into(),
        };
        c[0].claim
            .check_id()
            .expect("representation is outside current Claim body");
        let report = assess(&p, &t, &c, None).expect("retained member");
        assert!(production(&report).score.is_none());
        assert!(matches!(&production(&report).outcome,
            OutcomeBinding::Unresolved { code, .. } if code == "FMINE-REF-208"));
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
            reconstruct(&mut c[0]);
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
            reconstruct(&mut c[0]);
            let r = assess(&p, &t, &c, None).expect("negative state stays visible");
            assert!(production(&r).score.is_none(), "{state:?}");
            assert!(production(&r).score_claim.is_none(), "{state:?}");
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

    #[test]
    fn canonical_claims_keep_source_parents_and_missing_guidance() {
        let (p, t, c) = synthetic_evidence().expect("fixture");
        let r = assess(&p, &t, &c, None).expect("canonical rows");
        let row = production(&r);
        let g = &row.member.guidance;
        let y = row.member.outcome.as_ref().expect("interpreted outcome");
        let score = row.score_claim.as_ref().expect("canonical score");
        g.check_id().expect("guidance id");
        y.check_id().expect("outcome id");
        score.check_id().expect("score id");
        assert_eq!(g.value_tag(), g.value.canonical_tag());
        assert_eq!(y.value_tag(), y.value.canonical_tag());
        assert_eq!(score.value_tag(), score.value.canonical_tag());
        assert_eq!(score.value.z, row.score.expect("numeric score").z);
        assert_eq!(y.parents, vec![c[0].claim.claim_id]);
        assert!(score.parents.contains(&g.claim_id));
        assert!(score.parents.contains(&y.claim_id));
        assert_eq!(score.evidence, EvidenceClass::Assumed);
        assert!(
            score
                .anchors
                .iter()
                .any(|a| a.artifact_path == "synthetic-guidance.txt")
        );
        assert!(
            score
                .anchors
                .iter()
                .any(|a| a.artifact_sha256 == c[0].claim.anchors[0].artifact_sha256)
        );
        let missing = r
            .rows
            .iter()
            .find(|r| r.metric == "aisc_usd_per_oz")
            .expect("missing member");
        missing
            .member
            .guidance
            .check_id()
            .expect("missing member guidance");
        assert!(missing.member.outcome.is_none());
        assert!(missing.score_claim.is_none());
        assert!(r.to_json().contains("\"canonical_body_hex\""));
        let legacy = row.claims_json();
        assert!(legacy.contains("\"metric_key\":\"guidance.interpreted_target\""));
        assert!(legacy.contains("\"score_refusal\":null"));
        assert!(legacy.contains("\"hit\":\"certain_miss\""));
        assert!(legacy.contains("\"z_bounds\":"));
        assert!(legacy.contains("\"path\":\"synthetic-guidance.txt\""));
    }

    #[test]
    fn registered_synthetic_unknown_remains_unknown_in_its_score_claim() {
        let root = registered_workspace();
        let r = run(&root, "synthetic", None, None).expect("synthetic arithmetic");
        for row in &r.rows {
            assert!(row.score.is_some());
            assert_eq!(
                row.member
                    .outcome
                    .as_ref()
                    .expect("source interpretation")
                    .state,
                KnowledgeState::Unknown
            );
            let score = row.score_claim.as_ref().expect("hypothetical score");
            assert_eq!(score.state, KnowledgeState::Unknown);
            assert_eq!(score.evidence, EvidenceClass::Assumed);
            assert!(row.claims_json().contains("\"score_refusal\":null"));
        }
    }

    #[test]
    fn registered_public_interpretations_use_canonical_derived_claims() {
        let root = registered_workspace();
        // The invented test issuer exercises the public path without introducing real filings.
        for kind in ["sources", "transcriptions"] {
            let path = root.join(format!("registries/{kind}"));
            let text =
                std::fs::read_to_string(path.join("synthetic.toml")).expect("fixture registry");
            std::fs::write(
                path.join("example.toml"),
                text.replace("issuer = \"synthetic\"", "issuer = \"example\""),
            )
            .expect("invented public-path registry");
        }
        let r = run(&root, "example", None, None).expect("public-path Claims");
        assert!(!r.synthetic);
        for row in &r.rows {
            let score = row.score_claim.as_ref().expect("canonical score");
            score.check_id().expect("canonical id");
            assert_eq!(
                score.evidence,
                EvidenceClass::Derived.meet(EvidenceClass::ISSUER)
            );
            assert_eq!(score.state, KnowledgeState::Known);
            assert_eq!(score.value.z, row.score.expect("numeric score").z);
            assert!(row.claims_json().contains("\"score_refusal\":null"));
        }
    }
}
