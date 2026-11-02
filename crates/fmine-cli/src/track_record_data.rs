//! Retrospective source binding for issuer guidance (BD-341).
//! Determinism: Exact (canonical source/score Claims and certified rational reference bounds).
//!
//! DESIGN: retain each agreed guidance document and missing outcome. A later comparative is never
//! relabelled as original publication. This adapter does not backdate registrations in the forecast ledger.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use fmine_calib::guidance_scores::{
    Guidance, GuidanceScore, GuidanceSummary,
    claims::{
        self, GuidanceTarget, GuidanceValue, MemberInput, OutcomeValue, RangeScore, SeriesKey,
    },
};
use fmine_claim::Claim;
use fmine_core::codec::Encoder;
use fmine_core::hash::{Digest32, blake3_hash, sha256};
use fmine_evidence::{Refusal, RefusalCode};
use fmine_numerics::exact_feas::{Interval, Rat};
use fmine_public::key::{
    KeyedCell, OwnershipBasis, PeriodKind, PublicValue, Scope, canonical_value_tag,
};
use fmine_public::sources::{IssuerProfile, SourceKind, load_issuer_profile};
use fmine_public::transcribe::{CheckedDocument, TranscriptionReport};
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
        /// All selected source candidates remain inspectable when scoring is refused.
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
    pub source_binding: crate::track_record_binding::TrackRecordRow,
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
    pub pending_transcriptions: Vec<String>,
    pub unsupported_guidance: Vec<Accepted>,
}

fn failure(code: RefusalCode, what: impl Into<String>) -> Refusal {
    Refusal::new(code, format!("guidance track record: {}", what.into()))
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
    let bound = crate::track_record_binding::assess_snapshot(profile, transcripts, cells, site)?;
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
            || cell.claim.value_tag() != canonical_value_tag(&cell.key, &cell.claim.value)
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
    for source_binding in bound.rows {
        let subject = &source_binding.site;
        if !registered_sites.contains(subject.as_str()) || profile.regions.contains(subject) {
            return Err(failure(
                RefusalCode::NoClaimInDomain201,
                format!("{subject} is not one registered site"),
            ));
        }
        let a = &source_binding.guidance_source;
        let comparative = comparative_population(profile, source_binding.year);
        let selected = if comparative {
            source_binding
                .later
                .first()
                .unwrap_or(&source_binding.original)
        } else {
            &source_binding.original
        };
        let member = MemberInput {
            guidance: source_binding.guidance.clone(),
            outcome: selected.outcome.clone(),
        };
        let score_claim = selected.score.clone();
        let score = score_claim.as_ref().map(|c| GuidanceScore::from(&c.value));
        let outcome = if member.outcome.is_some() {
            OutcomeBinding::Resolved {
                cell: Box::new(selected.source_claims[0].clone()),
                comparative,
            }
        } else {
            let mut reason = selected
                .reason
                .clone()
                .unwrap_or_else(|| "no admissible annual outcome".into());
            if !comparative
                && selected.source_claims.is_empty()
                && reason.starts_with("FMINE-REF-201")
            {
                reason = "FMINE-REF-201: no annual summary cell in the original registered release; original annual outcome is absent; later comparatives cannot replace it".into();
            }
            let code = reason
                .split(':')
                .next()
                .filter(|c| c.starts_with("FMINE-REF-"))
                .map(String::from)
                .unwrap_or_else(|| {
                    if reason.contains("no prospective score") {
                        "FMINE-REF-108"
                    } else {
                        "FMINE-REF-202"
                    }
                    .into()
                });
            OutcomeBinding::Unresolved {
                code,
                reason,
                sources: selected.source_claims.clone(),
            }
        };
        let later_outcomes = source_binding
            .later
            .iter()
            .filter(|later| later.release > selected.release)
            .flat_map(|later| later.source_claims.iter().cloned())
            .collect();
        let guidance_source = source_binding.guidance_source_claim.clone();
        rows.push(TrackRecordRow {
            id: guidance_source.value_tag().to_string(),
            site: source_binding.site.clone(),
            metric: source_binding.metric.clone(),
            year: source_binding.year,
            guidance: a.clone(),
            published_at: source_binding.published_at.clone(),
            publication_basis: source_binding.published_at_basis.clone(),
            vintage: 0,
            comparative,
            outcome,
            later_outcomes,
            guidance_source,
            member,
            score_claim,
            score,
            source_binding,
            price_sensitivity: None,
        });
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
        pending_transcriptions: bound.pending_transcriptions,
        unsupported_guidance: bound.unsupported_guidance,
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
    if !transcripts.agreement.is_complete_for_prefix("guidance_") {
        return Err(failure(
            RefusalCode::NotObservable202,
            "complete two-reader agreement before selecting the guidance population",
        ));
    }
    let mut report =
        assess_with_price_subject(&profile, &transcripts, &claims.claims, site, price_subject)?;
    if let Some(unsupported) = report.unsupported_guidance.first() {
        return Err(failure(
            RefusalCode::NoClaimInDomain201,
            format!(
                "{}: unsupported guidance metric {}",
                unsupported.locator, unsupported.quantity
            ),
        ));
    }
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
        let pending = self
            .pending_transcriptions
            .iter()
            .map(|s| js(s))
            .collect::<Vec<_>>()
            .join(",");
        let unsupported = self.unsupported_guidance.iter().map(|a|format!("{{\"at\":{},\"quantity\":{},\"token\":{},\"unit\":{},\"basis\":{},\"sha256\":{}}}",js(&a.locator.to_string()),js(&a.quantity),js(&a.token),js(&a.unit),js(&a.basis),js(&a.doc_sha256))).collect::<Vec<_>>().join(",");
        let prefix = format!(
            "\"source_refusals\":[{source_refusals}],\"pending_transcriptions\":[{pending}],\"unsupported_guidance\":[{unsupported}],"
        );
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
            format!("{{\"id\":{},\"site\":{},\"metric\":{},\"year\":{},\"guidance_vintage_ordinal\":{},\"published_at\":{},\"published_at_basis\":{},\"guidance\":{{\"at\":{},\"sha256\":{},\"token\":{},\"unit\":{},\"basis\":{},\"period\":{},\"reader_count\":{},\"source_claim\":{},\"claim\":{}}},\"outcome\":{outcome},\"later_outcomes\":[{}],\"interpreted_outcome_claim\":{},\"score\":{score},\"price_sensitivity\":{price},\"claims\":{},\"source_binding\":{}}}",
                js(&r.id), js(&r.site), js(&r.metric), r.year, r.vintage, js(&r.published_at), js(&r.publication_basis),
                js(&r.guidance.locator.to_string()), js(&r.guidance.doc_sha256), js(&r.guidance.token), js(&r.guidance.unit),
                js(&r.guidance.basis), js(&r.guidance.period), r.guidance.readers.len(),
                claim_json(&r.guidance_source, &source_value), guidance_claim_json(&r.member.guidance),
                r.later_outcomes.iter().map(KeyedCell::to_json).collect::<Vec<_>>().join(","),
                r.member.outcome.as_ref().map_or_else(|| "null".into(), outcome_claim_json), r.claims_json(), r.source_binding.to_json())
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
    let workbook = fmine_io::xlsx::Workbook {
        artifact_path: path.into(),
        sha256: digest.0,
        sheets: vec![],
        hidden_sheets: vec![],
        quarantined: vec![],
    };
    let cell = fmine_public::key::public_claim(
        &workbook,
        "Annual",
        "C1",
        key,
        PublicValue::Number {
            stored: "125".into(),
            displayed: Ok("125".into()),
            under_status: None,
        },
    );
    // The shared synthetic profile has no on-disk sources; this fixture supplies its own invented bytes.
    profile.corpus_dir = "synthetic/2030Q4".into();
    profile
        .sources
        .push(fmine_public::sources::RegisteredSource {
            file_name: "production.txt".into(),
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

include!("../../xtask/testdata/cli_track_record_data_tests.rs");
