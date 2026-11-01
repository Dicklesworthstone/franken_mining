//! DESIGN (bd-srqc): two-reader annual inputs and first-registered site-quarter Claims feed two
//! descriptive recovery models. Contextual full-mine ownership never rewrites an unprinted source key.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use fmine_claim::{AdditivityClass, Claim, Confidentiality, Representation, SpatialSupport};
use fmine_core::{Encoder, anchor::SourceAnchor, blake3_hash, hash::Digest32};
use fmine_evidence::{
    EpistemicStatus, EvidenceClass, KnowledgeState, KuKind, Refusal, RefusalCode,
};
use fmine_metrics::recovery::{
    self as kernel, Estimate, Fit, ModelProjection, OperatingRow, Ownership, Period, Projection,
    Quantity, QuantityKind, SquareRoot, TrainingRow,
};
use fmine_numerics::exact_feas::{Convention, Rat};
use fmine_registry::transcriptions::{Accepted, Decimal, Token, parse_token};

use crate::key::{self, KeyedCell, OwnershipBasis, PeriodKind, PublicValue, Scope};
use crate::sources::{IssuerProfile, SourceKind};
use crate::transcribe::{CheckedDocument, TranscriptionReport};
use crate::{js, mine_ledger::rat_text};

pub const RULES_VERSION: &str = "public-recovery/1";
pub const TRAINING_TABLE: &str = "Appendix A: Past Production Records for Lumwana";
pub const OWNERSHIP_QUANTITY: &str = "recovery_lumwana_ownership_pct";
const PREFIX: &str = "recovery_lumwana_";
const SUBJECT: &str = "Lumwana";
type Result<T> = std::result::Result<T, Refusal>;

#[derive(Debug, Clone)]
pub struct QuarterReport {
    pub period: key::Period,
    pub release: String,
    /// Original source Claims, including their unchanged ownership keys and interpretation evidence.
    pub inputs: Vec<KeyedCell>,
    /// The exact interpreted inputs passed to the model, retained even when the kernel refuses them.
    pub operating: Option<OperatingRow>,
    pub outcome: Result<Projection>,
}

#[derive(Debug, Clone)]
pub struct RecoveryReport {
    pub issuer: String,
    pub synthetic: bool,
    pub through: Option<String>,
    pub training: Vec<TrainingRow>,
    pub transcriptions: Vec<Accepted>,
    pub ownership_source: Accepted,
    pub ownership: Claim<Rat>,
    pub fit: Claim<Fit>,
    pub quarters: Vec<QuarterReport>,
    pub documents: Vec<CheckedDocument>,
}

fn refuse(code: RefusalCode, detail: impl Into<String>) -> Refusal {
    Refusal::new(code, format!("recovery gap: {}", detail.into()))
}

fn source_error(detail: impl Into<String>) -> Refusal {
    refuse(RefusalCode::SourceCellUnusable208, detail)
}

fn exact<T>(value: std::result::Result<T, fmine_units::UnitsError>) -> Result<T> {
    value.map_err(|e| refuse(RefusalCode::NumericalCertificateFailed207, e.to_string()))
}

fn sha_bytes(hex: &str) -> Result<[u8; 32]> {
    if hex.len() != 64 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(refuse(
            RefusalCode::DigestMismatch505,
            "invalid source SHA-256",
        ));
    }
    let mut out = [0; 32];
    for (i, byte) in out.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16)
            .map_err(|_| refuse(RefusalCode::DigestMismatch505, "invalid source SHA-256"))?;
    }
    if out == [0; 32] {
        return Err(refuse(
            RefusalCode::DigestMismatch505,
            "empty source digest",
        ));
    }
    Ok(out)
}

fn printed(d: Decimal) -> Result<Estimate> {
    let den = 10_i128
        .checked_pow(d.scale)
        .ok_or_else(|| source_error("printed precision overflows"))?;
    let point = exact(Rat::new(d.mantissa, den))?;
    let half = exact(Rat::new(
        1,
        den.checked_mul(2)
            .ok_or_else(|| source_error("printed precision overflows"))?,
    ))?;
    let interval = Convention::Closed
        .interval(point, half)
        .map_err(|e| source_error(format!("printed rounding interval: {e:?}")))?;
    Ok(Estimate { point, interval })
}

fn point_token(text: &str) -> Result<Decimal> {
    match parse_token(text).map_err(source_error)? {
        Token::Point(d) => Ok(d),
        Token::Range(..) => Err(source_error("one printed point is required, not a range")),
    }
}

fn accepted<'a>(
    profile: &IssuerProfile,
    report: &'a TranscriptionReport,
    quantity: &str,
) -> Result<&'a Accepted> {
    let rows: Vec<_> = report
        .agreement
        .accepted
        .iter()
        .filter(|a| a.quantity == quantity)
        .collect();
    let [a] = rows.as_slice() else {
        return Err(refuse(
            RefusalCode::NotObservable202,
            format!(
                "{quantity} requires one agreed source row; found {}",
                rows.len()
            ),
        ));
    };
    let registered: Vec<_> = profile
        .documents
        .iter()
        .filter(|d| d.file == a.locator.doc)
        .collect();
    let checked: Vec<_> = report
        .documents
        .iter()
        .filter(|d| d.file == a.locator.doc)
        .collect();
    let ([registered], [checked]) = (registered.as_slice(), checked.as_slice()) else {
        return Err(refuse(
            RefusalCode::NoClaimInDomain201,
            format!(
                "{} requires one registered and byte-checked document",
                a.locator
            ),
        ));
    };
    if a.doc_sha256 != registered.sha256
        || a.doc_sha256 != checked.sha256
        || registered.kind != checked.kind
        || registered.published_at != checked.published_at
    {
        return Err(refuse(
            RefusalCode::DigestMismatch505,
            format!("{} differs from its checked document", a.locator),
        ));
    }
    sha_bytes(&a.doc_sha256)?;
    if a.locator.page == 0
        || a.locator.page > registered.pages
        || [&a.locator.table, &a.locator.row, &a.locator.col]
            .iter()
            .any(|s| s.trim().is_empty())
        || a.readers
            .iter()
            .filter(|s| !s.trim().is_empty())
            .collect::<BTreeSet<_>>()
            .len()
            < 2
        || parse_token(&a.token).map_err(source_error)? != a.value
    {
        return Err(refuse(
            RefusalCode::NoClaimInDomain201,
            format!(
                "{} lacks two agreeing, fully cited source readings",
                a.locator
            ),
        ));
    }
    Ok(a)
}

fn anchor(a: &Accepted) -> Result<SourceAnchor> {
    Ok(SourceAnchor::new(
        &a.locator.doc,
        sha_bytes(&a.doc_sha256)?,
        a.locator
            .to_string()
            .strip_prefix(&format!("{}:", a.locator.doc))
            .unwrap_or_default(),
    ))
}

fn ownership_claim(a: &Accepted, synthetic: bool) -> Result<Claim<Rat>> {
    let token = point_token(&a.token)?;
    let expected = if synthetic {
        (1, TRAINING_TABLE, "Lumwana (100%)", "Interest (%)")
    } else {
        (
            38,
            "Copper Mineral Reserves",
            "Lumwana surface (100%)",
            "Based on attributable tonnes",
        )
    };
    if a.unit != "percent"
        || a.basis != "100%"
        || a.period != "2025FY"
        || token.mantissa != 100
        || token.scale != 0
        || (
            a.locator.page,
            a.locator.table.as_str(),
            a.locator.row.as_str(),
            a.locator.col.as_str(),
        ) != expected
    {
        return Err(refuse(
            RefusalCode::UndeclaredUnitBasisOrTz107,
            "full-site interpretation needs the two-reader printed Lumwana (100%) ownership context",
        ));
    }
    let evidence = if synthetic {
        EvidenceClass::Assumed
    } else {
        EvidenceClass::ISSUER
    };
    Ok(Claim::new(
        OWNERSHIP_QUANTITY,
        Rat::from_integer(100),
        &format!("{RULES_VERSION}|100|{}", a.locator),
        Representation::Deterministic,
        SpatialSupport::PeriodAggregate,
        AdditivityClass::NonAdditive,
        evidence,
        EpistemicStatus::KnownKnown,
        KnowledgeState::Known,
        Confidentiality::public(),
        vec![],
        vec![],
        vec![anchor(a)?],
    ))
}

fn quantity_claim(
    metric: &str,
    value: Quantity,
    ownership: &Claim<Rat>,
    source: Option<&Claim<PublicValue>>,
    mut anchors: Vec<SourceAnchor>,
    synthetic: bool,
) -> Claim<Quantity> {
    anchors.extend(ownership.anchors.iter().cloned());
    anchors.sort();
    anchors.dedup();
    let mut parents = vec![ownership.claim_id];
    let mut evidence = if synthetic {
        EvidenceClass::Assumed
    } else {
        EvidenceClass::ISSUER
    };
    let mut epistemic = if synthetic {
        EpistemicStatus::KnownUnknown(KuKind::Parametric)
    } else {
        EpistemicStatus::KnownKnown
    };
    let mut state = KnowledgeState::Known;
    let mut uu = Vec::new();
    if let Some(c) = source {
        parents.push(c.claim_id);
        evidence = evidence.meet(c.evidence);
        epistemic = epistemic.and(c.epistemic);
        state = c.state;
        uu.extend(c.uu_terms.iter().cloned());
    }
    parents.sort();
    let tag = value.canonical_tag();
    let additivity = value.kind.additivity();
    Claim::new(
        metric,
        value,
        &tag,
        Representation::Deterministic,
        SpatialSupport::PeriodAggregate,
        additivity,
        evidence,
        epistemic,
        state,
        Confidentiality::public(),
        uu,
        parents,
        anchors,
    )
}

fn annual_claim(
    a: &Accepted,
    year: u16,
    kind: QuantityKind,
    ownership: &Claim<Rat>,
    synthetic: bool,
) -> Result<Claim<Quantity>> {
    let (unit, column) = if kind == QuantityKind::HeadGradePercent {
        ("wt_pct", "Head Grade (% Cu)")
    } else {
        ("percent", "Recovery (%)")
    };
    if a.unit != unit
        || a.basis != "100%"
        || a.period != format!("{year}FY")
        || a.locator.table != TRAINING_TABLE
        || a.locator.row != year.to_string()
        || a.locator.col != column
    {
        return Err(refuse(
            RefusalCode::SupportMismatch102,
            format!(
                "{} has inconsistent annual unit, basis, period or table labels",
                a.locator
            ),
        ));
    }
    let e = printed(point_token(&a.token)?)?;
    let q = Quantity {
        subject: SUBJECT.into(),
        period: Period {
            year,
            first_month: 1,
            last_month: 12,
        },
        ownership: Ownership::Full,
        kind,
        point: e.point,
        interval: e.interval,
    };
    Ok(quantity_claim(
        &a.quantity,
        q,
        ownership,
        None,
        vec![anchor(a)?],
        synthetic,
    ))
}

fn field(cell: &KeyedCell) -> Option<QuantityKind> {
    let (section, label) = cell.key.line.label.strip_prefix("site:")?.split_once(':')?;
    match (section, label) {
        ("average grade" | "average grade (percent)", "processed") => {
            Some(QuantityKind::HeadGradePercent)
        }
        ("", "recovery rate (percent)") => Some(QuantityKind::RecoveryPercent),
        ("", "tonnes processed (thousands)") => Some(QuantityKind::ProcessedKt),
        ("", "production (thousands of tonnes)") => Some(QuantityKind::CopperProducedKt),
        _ => None,
    }
}

fn operating_claim(
    profile: &IssuerProfile,
    c: &KeyedCell,
    kind: QuantityKind,
    ownership: &Claim<Rat>,
) -> Result<Claim<Quantity>> {
    let (file, at) = c
        .locator
        .split_once(':')
        .ok_or_else(|| source_error("invalid workbook locator"))?;
    let v = &c.key.reporting_vintage;
    let sources: Vec<_> = profile
        .sources
        .iter()
        .filter(|s| s.file_name == file)
        .collect();
    let [registered] = sources.as_slice() else {
        return Err(refuse(
            RefusalCode::NoClaimInDomain201,
            format!("{} has no unique registered source", c.locator),
        ));
    };
    if registered.kind != SourceKind::MineStats
        || registered.quarter != v.release
        || registered.sha256_hex != v.file_sha256
        || c.claim.anchors.len() != 1
        || !c.claim.anchors.iter().all(|a| {
            a.artifact_path.rsplit('/').next() == Some(file)
                && a.locator == at
                && Digest32(a.artifact_sha256).to_hex() == registered.sha256_hex
        })
    {
        return Err(refuse(
            RefusalCode::DigestMismatch505,
            format!("{} source identity differs", c.locator),
        ));
    }
    if !c.claim.confidentiality.is_public()
        || c.claim.support != SpatialSupport::PeriodAggregate
        || c.claim.stale
        || c.claim.calibration_failed
    {
        return Err(source_error(format!(
            "{} is not a usable public period Claim",
            c.locator
        )));
    }
    if matches!(c.key.ownership_basis, OwnershipBasis::Attributable { num, den, .. } if num != den || num <= 0)
        || matches!(c.printed_basis, OwnershipBasis::Attributable { num, den, .. } if num != den || num <= 0)
    {
        return Err(refuse(
            RefusalCode::BasisMismatch105,
            format!("{} is not full-site basis", c.locator),
        ));
    }
    let PublicValue::Number {
        displayed: Ok(text),
        under_status: None,
        ..
    } = &c.claim.value
    else {
        return Err(source_error(format!(
            "{} is missing, nonnumeric, status-qualified or undisplayable",
            c.locator
        )));
    };
    let percent = matches!(
        kind,
        QuantityKind::HeadGradePercent | QuantityKind::RecoveryPercent
    );
    let raw = if percent {
        match c.key.unit.as_str() {
            "mass_fraction" => text.strip_suffix('%').ok_or_else(|| {
                source_error(format!("{} needs explicit percent display", c.locator))
            })?,
            "percent" | "wt_pct" => text.strip_suffix('%').unwrap_or(text),
            _ => {
                return Err(refuse(
                    RefusalCode::BasisMismatch105,
                    format!("{} is not in percent units", c.locator),
                ));
            }
        }
    } else if c.key.unit == "kt" {
        text.as_str()
    } else {
        return Err(refuse(
            RefusalCode::BasisMismatch105,
            format!("{} is not in thousand tonnes", c.locator),
        ));
    };
    let e = printed(point_token(raw)?)?;
    let PeriodKind::Quarter(q) = c.key.valid_period.kind else {
        return Err(refuse(
            RefusalCode::SupportMismatch102,
            "quarterly physical support required",
        ));
    };
    if !(1..=4).contains(&q) {
        return Err(refuse(
            RefusalCode::SupportMismatch102,
            "invalid calendar quarter",
        ));
    }
    let value = Quantity {
        subject: SUBJECT.into(),
        period: Period {
            year: c.key.valid_period.year,
            first_month: q * 3 - 2,
            last_month: q * 3,
        },
        ownership: Ownership::Full,
        kind,
        point: e.point,
        interval: e.interval,
    };
    Ok(quantity_claim(
        &format!("recovery.{}", c.key.line.label),
        value,
        ownership,
        Some(&c.claim),
        c.claim.anchors.clone(),
        profile.issuer == crate::synthetic::SYNTHETIC_ISSUER,
    ))
}

fn quarter(
    profile: &IssuerProfile,
    fit: &Claim<Fit>,
    ownership: &Claim<Rat>,
    period: key::Period,
    candidates: &[&KeyedCell],
) -> QuarterReport {
    let release = candidates
        .iter()
        .map(|c| c.key.reporting_vintage.release.as_str())
        .min()
        .unwrap_or_default()
        .to_string();
    let selected: Vec<_> = candidates
        .iter()
        .filter(|c| c.key.reporting_vintage.release == release)
        .copied()
        .collect();
    let mut inputs: Vec<_> = selected
        .iter()
        .filter(|c| field(c).is_some())
        .map(|c| (*c).clone())
        .collect();
    inputs.sort_by(|a, b| (&a.key, &a.locator).cmp(&(&b.key, &b.locator)));
    let mut operating = None;
    let outcome = (|| {
        if release < period.code() {
            return Err(refuse(
                RefusalCode::SupportMismatch102,
                "release predates the observed quarter",
            ));
        }
        let files: BTreeSet<_> = selected
            .iter()
            .map(|c| c.locator.split_once(':').map(|(f, _)| f))
            .collect();
        if files.len() != 1 || files.contains(&None) {
            return Err(refuse(
                RefusalCode::NotObservable202,
                "ambiguous first-registered workbook",
            ));
        }
        let read = |kind| {
            let found: Vec<_> = inputs.iter().filter(|c| field(c) == Some(kind)).collect();
            let [c] = found.as_slice() else {
                return Err(refuse(
                    RefusalCode::NotObservable202,
                    format!(
                        "{kind:?} needs one first-registered physical cell; found {}",
                        found.len()
                    ),
                ));
            };
            operating_claim(profile, c, kind, ownership)
        };
        let row = OperatingRow {
            head: read(QuantityKind::HeadGradePercent)?,
            recovery: read(QuantityKind::RecoveryPercent)?,
            processed: read(QuantityKind::ProcessedKt)?,
            produced: read(QuantityKind::CopperProducedKt)?,
        };
        let result = kernel::project(fit, &row);
        operating = Some(row);
        result
    })();
    QuarterReport {
        period,
        release,
        inputs,
        operating,
        outcome,
    }
}

/// Absence of this model's source rows leaves ordinary assurance usable. Partial or conflicting
/// registration refuses explicitly; quarter-level unusable inputs remain in the returned population.
pub fn assess(
    profile: &IssuerProfile,
    transcription: Option<&TranscriptionReport>,
    cells: &[KeyedCell],
) -> Result<Option<RecoveryReport>> {
    let Some(report) = transcription else {
        return Ok(None);
    };
    let active = report
        .agreement
        .accepted
        .iter()
        .any(|a| a.quantity.starts_with(PREFIX))
        || report
            .agreement
            .single_reader
            .iter()
            .any(|(l, _)| l.table == TRAINING_TABLE)
        || report
            .agreement
            .disagreements
            .iter()
            .any(|d| d.locator.table == TRAINING_TABLE);
    if !active {
        return Ok(None);
    }
    validate_through(profile.through.as_deref())?;
    if !report.agreement.is_consistent() {
        return Err(refuse(
            RefusalCode::NotObservable202,
            "transcription readers disagree",
        ));
    }
    let synthetic = profile.issuer == crate::synthetic::SYNTHETIC_ISSUER;
    let ownership_source = accepted(profile, report, OWNERSHIP_QUANTITY)?.clone();
    if !synthetic
        && !profile
            .documents
            .iter()
            .any(|d| d.file == ownership_source.locator.doc && d.kind == "aif")
    {
        return Err(source_error(
            "ownership context must cite the checked annual information form",
        ));
    }
    let ownership = ownership_claim(&ownership_source, synthetic)?;
    let mut training = Vec::new();
    let mut transcriptions = Vec::new();
    for year in 2013..=2022 {
        let head = accepted(profile, report, &format!("{PREFIX}{year}_head_grade_pct"))?;
        let recovery = accepted(profile, report, &format!("{PREFIX}{year}_recovery_pct"))?;
        training.push(TrainingRow {
            head: annual_claim(
                head,
                year,
                QuantityKind::HeadGradePercent,
                &ownership,
                synthetic,
            )?,
            recovery: annual_claim(
                recovery,
                year,
                QuantityKind::RecoveryPercent,
                &ownership,
                synthetic,
            )?,
        });
        transcriptions.extend([head.clone(), recovery.clone()]);
    }
    let allowed: BTreeSet<_> = transcriptions
        .iter()
        .chain([&ownership_source])
        .map(|a| a.quantity.as_str())
        .collect();
    if report
        .agreement
        .accepted
        .iter()
        .any(|a| a.quantity.starts_with(PREFIX) && !allowed.contains(a.quantity.as_str()))
    {
        return Err(source_error(
            "unrecognised historical recovery quantity; only the named 2013–2022 decade is supported",
        ));
    }
    let fit = kernel::fit(&training)?;
    let mut periods: BTreeMap<key::Period, Vec<&KeyedCell>> = BTreeMap::new();
    for c in cells.iter().filter(|c| {
        c.key.issuer == profile.issuer
            && c.key.subject == SUBJECT
            && c.key.scope == Scope::Site
            && c.key.line.label.starts_with("site:")
            && c.key.valid_period.year >= 2023
            && matches!(c.key.valid_period.kind, PeriodKind::Quarter(_))
    }) {
        periods.entry(c.key.valid_period).or_default().push(c);
    }
    if periods.is_empty() {
        return Err(refuse(
            RefusalCode::NoClaimInDomain201,
            "no visible Lumwana quarter Claims were supplied",
        ));
    }
    let quarters = periods
        .into_iter()
        .map(|(p, c)| quarter(profile, &fit, &ownership, p, &c))
        .collect();
    let docs: BTreeSet<_> = transcriptions
        .iter()
        .chain([&ownership_source])
        .map(|a| a.locator.doc.as_str())
        .collect();
    let mut documents: Vec<_> = report
        .documents
        .iter()
        .filter(|d| docs.contains(d.file.as_str()))
        .cloned()
        .collect();
    documents.sort_by(|a, b| a.file.cmp(&b.file));
    Ok(Some(RecoveryReport {
        issuer: profile.issuer.clone(),
        synthetic,
        through: profile.through.clone(),
        training,
        transcriptions,
        ownership_source,
        ownership,
        fit,
        quarters,
        documents,
    }))
}

/// A retrospective public computation: `through` filters workbook releases only. Current verified
/// historical documents supply the model; this command is not an as-of forecast or hindcast.
pub fn run_public(root: &Path, issuer: &str, through: Option<&str>) -> Result<RecoveryReport> {
    validate_through(through)?;
    let profile = crate::sources::load_issuer_profile(root, issuer)?;
    let profile = match through {
        Some(bound) => profile.through(bound)?,
        None => profile,
    };
    if profile.issuer == crate::synthetic::SYNTHETIC_ISSUER {
        return Err(refuse(
            RefusalCode::NoClaimInDomain201,
            "use the explicit synthetic recovery command for invented inputs",
        ));
    }
    let transcription = crate::transcribe::load_transcription_report(&profile, root)?;
    let cells = key::mine_stats::load_claims(&profile, root)?;
    assess(&profile, transcription.as_ref(), &cells.claims)?.ok_or_else(|| {
        refuse(
            RefusalCode::NoClaimInDomain201,
            "no registered two-reader recovery historical inputs",
        )
    })
}

fn validate_through(through: Option<&str>) -> Result<()> {
    if through.is_some_and(|bound| !crate::sources::is_through_bound(bound)) {
        return Err(refuse(
            RefusalCode::UndeclaredUnitBasisOrTz107,
            "workbook through must be YYYYQ1 through YYYYQ4",
        ));
    }
    Ok(())
}

fn locations<T>(claim: &Claim<T>) -> Vec<String> {
    claim
        .anchors
        .iter()
        .map(|a| format!("{}:{}", a.artifact_path, a.locator))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn strings(values: &[String]) -> String {
    values.iter().map(|s| js(s)).collect::<Vec<_>>().join(",")
}

fn derived(value: &str, rule: &str, inputs: &[String]) -> String {
    format!(
        "{{\"v\":{value},\"rule\":{},\"rules_version\":{},\"in\":[{}]}}",
        js(rule),
        js(kernel::RULES_VERSION),
        strings(inputs)
    )
}

fn rational(value: Rat, rule: &str, inputs: &[String]) -> String {
    derived(&js(&rat_text(value)), rule, inputs)
}

fn estimate_json(e: Estimate, rule: &str, inputs: &[String]) -> String {
    format!(
        "{{\"point\":{},\"lo\":{},\"hi\":{},\"closed\":true}}",
        rational(e.point, rule, inputs),
        rational(e.interval.lo, rule, inputs),
        rational(e.interval.hi, rule, inputs)
    )
}

fn sqrt_json(s: SquareRoot, rule: &str, inputs: &[String]) -> String {
    format!(
        "{{\"approx\":{},\"lo\":{},\"hi\":{},\"interpretation\":\"numerical square-root enclosure only\"}}",
        derived(&s.approx.to_string(), rule, inputs),
        derived(&s.enclosure[0].to_string(), rule, inputs),
        derived(&s.enclosure[1].to_string(), rule, inputs)
    )
}

fn claim_fields<T>(c: &Claim<T>, calibration: &str) -> String {
    let confidentiality = if c.confidentiality.is_public() {
        "public".into()
    } else {
        c.confidentiality.to_string()
    };
    let anchors = c
        .anchors
        .iter()
        .map(|a| {
            format!(
                "{{\"file\":{},\"sha256\":{},\"locator\":{}}}",
                js(&a.artifact_path),
                js(&Digest32(a.artifact_sha256).to_hex()),
                js(&a.locator)
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let parents = c
        .parents
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    let inputs = locations(c);
    let uu = c.uu_terms.iter().map(|u| {
        let downside = if u.downside_exposure_usd_m.is_finite() && u.downside_exposure_usd_m != 0.0 {
            format!("\"monetary_downside\":\"source_estimate\",\"downside_exposure_usd_m\":{{\"v\":{},\"rule\":\"recovery.inherited_uu\",\"rules_version\":{},\"in\":[{}]}}", u.downside_exposure_usd_m, js(RULES_VERSION), strings(&inputs))
        } else {
            "\"monetary_downside\":\"unquantified\"".into()
        };
        format!("{{\"scenario_id\":{},\"description\":{},\"trigger_indicator\":{},{downside}}}", js(&u.scenario_id), js(&u.description), js(&u.trigger_indicator))
    }).collect::<Vec<_>>().join(",");
    format!(
        "\"claim_id\":{},\"metric\":{},\"evidence\":{},\"epistemic\":{},\"knowledge\":{},\"support\":\"period_aggregate\",\
        \"confidentiality\":{},\"calibration\":{},\"parents\":[{}],\"anchors\":[{anchors}],\"uu_terms\":[{uu}]",
        js(&c.claim_id.to_string()),
        js(&c.metric_key),
        js(&format!("{:?}", c.evidence)),
        js(&format!("{:?}", c.epistemic)),
        js(&format!("{:?}", c.state)),
        js(&confidentiality),
        js(calibration),
        strings(&parents)
    )
}

fn ownership_json(ownership: Ownership, inputs: &[String]) -> String {
    match ownership {
        Ownership::Full => "{\"kind\":\"full\"}".into(),
        Ownership::Attributable { share } => format!(
            "{{\"kind\":\"attributable\",\"share\":{}}}",
            rational(share, "recovery.ownership_interpretation", inputs)
        ),
    }
}

fn quantity_json(c: &Claim<Quantity>) -> String {
    let q = &c.value;
    let (kind, unit) = match q.kind {
        QuantityKind::HeadGradePercent => ("head_grade_percent", "wt_pct"),
        QuantityKind::RecoveryPercent => ("recovery_percent", "percent"),
        QuantityKind::ProcessedKt => ("processed_kt", "kt"),
        QuantityKind::CopperProducedKt => ("copper_produced_kt", "kt"),
    };
    let inputs = locations(c);
    let ownership = ownership_json(q.ownership, &inputs);
    format!(
        "{{{},\"subject\":{},\"period\":{{\"year\":{},\"first_month\":{},\"last_month\":{}}},\"quantity_kind\":{},\"unit\":{},\"ownership_basis\":{ownership},\"value\":{}}}",
        claim_fields(c, "source_interpretation"),
        js(&q.subject),
        js(&q.period.year.to_string()),
        js(&q.period.first_month.to_string()),
        js(&q.period.last_month.to_string()),
        js(kind),
        js(unit),
        estimate_json(
            Estimate {
                point: q.point,
                interval: q.interval
            },
            "recovery.printed_quantity",
            &inputs
        )
    )
}

fn transcription_json(a: &Accepted) -> String {
    format!(
        "{{\"quantity\":{},\"token\":{},\"unit\":{},\"basis\":{},\"period\":{},\"at\":{},\"sha256\":{},\"readers\":[{}]}}",
        js(&a.quantity),
        js(&a.token),
        js(&a.unit),
        js(&a.basis),
        js(&a.period),
        js(&a.locator.to_string()),
        js(&a.doc_sha256),
        strings(&a.readers)
    )
}

fn projection_json(c: &Claim<ModelProjection>) -> String {
    let p = &c.value;
    let inputs = locations(c);
    format!(
        "{{{},\"ownership_basis\":{},\"expected_recovery_percent\":{},\"gap_pp\":{},\"copper_difference_kt\":{},\"fraction_of_reported_output\":{}}}",
        claim_fields(c, "uncalibrated_descriptive_model"),
        ownership_json(p.ownership, &inputs),
        estimate_json(
            p.expected_recovery_percent,
            "recovery.expected_recovery",
            &inputs
        ),
        estimate_json(p.gap_pp, "recovery.gap_pp", &inputs),
        estimate_json(
            p.copper_difference_kt,
            "recovery.copper_difference",
            &inputs
        ),
        estimate_json(
            p.fraction_of_reported_output,
            "recovery.fraction_of_reported_output",
            &inputs
        )
    )
}

impl RecoveryReport {
    /// A separate canonical digest; the public identity/waterfall authority root remains unchanged.
    #[must_use]
    pub fn digest(&self) -> Digest32 {
        let mut enc = Encoder::new_frame(*b"RCGP", 1);
        enc.put_str(RULES_VERSION);
        enc.put_str(kernel::RULES_VERSION);
        enc.put_str(&self.issuer);
        enc.put_u8(u8::from(self.synthetic));
        enc.put_str(self.through.as_deref().unwrap_or(""));
        let mut rows: Vec<_> = self
            .transcriptions
            .iter()
            .chain([&self.ownership_source])
            .collect();
        rows.sort_by(|a, b| (&a.quantity, &a.locator).cmp(&(&b.quantity, &b.locator)));
        enc.put_u64(rows.len() as u64);
        for a in rows {
            for text in [
                &a.quantity,
                &a.locator.to_string(),
                &a.doc_sha256,
                &a.token,
                &a.unit,
                &a.basis,
                &a.period,
            ] {
                enc.put_str(text);
            }
        }
        enc.put_str(&self.fit.claim_id.to_string());
        enc.put_str(&self.fit.value.canonical_tag());
        enc.put_u64(self.quarters.len() as u64);
        for q in &self.quarters {
            enc.put_str(&q.period.code());
            enc.put_str(&q.release);
            enc.put_fixed(&key::claims_digest(&q.inputs).0);
            match &q.outcome {
                Ok(p) => {
                    enc.put_u8(0);
                    enc.put_str(&p.constant_tails.claim_id.to_string());
                    enc.put_str(&p.constant_tails.value.canonical_tag());
                    enc.put_str(&p.grade_dependent_tails.claim_id.to_string());
                    enc.put_str(&p.grade_dependent_tails.value.canonical_tag());
                }
                Err(r) => {
                    enc.put_u8(1);
                    enc.put_str(r.code);
                }
            }
        }
        blake3_hash(&enc.finish())
    }

    #[must_use]
    pub fn to_json(&self) -> String {
        let f = &self.fit.value;
        let inputs = locations(&self.fit);
        let fit = format!(
            "{{{},\"ownership_basis\":{},\"training_years\":[{}],\"mean_head_percent\":{},\"constant_tails_percent\":{},\"intercept_percent\":{},\"slope\":{},\
            \"residual_sample_variance_at_printed\":{},\"residual_sample_sd_at_printed\":{},\"residual_sample_divisor\":{},\
            \"regression_error_variance_at_printed\":{},\"regression_error_sd_at_printed\":{},\"regression_error_divisor\":{}}}",
            claim_fields(&self.fit, "uncalibrated_descriptive_model"),
            ownership_json(f.ownership, &inputs),
            strings(
                &f.training_years
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
            ),
            estimate_json(f.mean_head, "recovery.mean_head", &inputs),
            estimate_json(f.constant_tails, "recovery.constant_tails", &inputs),
            estimate_json(f.intercept, "recovery.ols_intercept", &inputs),
            estimate_json(f.slope, "recovery.ols_slope", &inputs),
            rational(
                f.residual_sample_variance_at_printed,
                "recovery.residual_sample_variance",
                &inputs
            ),
            sqrt_json(
                f.residual_sample_sd_at_printed,
                "recovery.residual_sample_sd",
                &inputs
            ),
            derived(
                &f.training_years.len().saturating_sub(1).to_string(),
                "recovery.sample_divisor",
                &inputs
            ),
            rational(
                f.regression_error_variance_at_printed,
                "recovery.regression_error_variance",
                &inputs
            ),
            sqrt_json(
                f.regression_error_sd_at_printed,
                "recovery.regression_error_sd",
                &inputs
            ),
            derived(
                &f.training_years.len().saturating_sub(2).to_string(),
                "recovery.regression_divisor",
                &inputs
            )
        );
        let quarters = self.quarters.iter().map(|q| {
            let source_claims = q.inputs.iter().map(KeyedCell::to_json).collect::<Vec<_>>().join(",");
            let operating_claims = q.operating.as_ref().map(|row| [&row.head, &row.recovery, &row.processed, &row.produced].into_iter().map(quantity_json).collect::<Vec<_>>().join(",")).unwrap_or_default();
            let outcome = match &q.outcome {
                Ok(p) => format!("\"status\":\"ok\",\"constant_tails\":{},\"grade_dependent_tails\":{}",
                    projection_json(&p.constant_tails), projection_json(&p.grade_dependent_tails)),
                Err(r) => format!("\"status\":\"refused\",\"refusal\":{{\"code\":{},\"what\":{},\"fix\":{}}}", js(r.code), js(&r.what), js(r.fix)),
            };
            format!("{{\"quarter\":{},\"first_registered_release\":{},\"comparative_at_first_registration\":{},\"ownership_interpretation\":\"100% full site, independently cited; source keys preserved\",\"source_claims\":[{source_claims}],\"operating_claims\":[{operating_claims}],{outcome}}}",
                js(&q.period.code()), js(&q.release), q.release != q.period.code())
        }).collect::<Vec<_>>().join(",");
        let rows = self
            .transcriptions
            .iter()
            .map(transcription_json)
            .collect::<Vec<_>>()
            .join(",");
        let training_claims = self
            .training
            .iter()
            .flat_map(|row| [&row.head, &row.recovery])
            .map(quantity_json)
            .collect::<Vec<_>>()
            .join(",");
        let ownership_claim = format!(
            "{{{},\"unit\":\"percent\",\"interpretation\":\"exact full-site ownership context, not a rounded operating measurement\",\"value\":{}}}",
            claim_fields(&self.ownership, "source_assertion"),
            rational(
                self.ownership.value,
                "recovery.ownership_interpretation",
                &locations(&self.ownership)
            )
        );
        let documents = self
            .documents
            .iter()
            .map(|d| {
                format!(
                    "{{\"file\":{},\"sha256\":{},\"published_at\":{},\"published_at_basis\":{}}}",
                    js(&d.file),
                    js(&d.sha256),
                    js(&d.published_at),
                    js(&d.published_at_basis)
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        let count_inputs = self
            .quarters
            .iter()
            .flat_map(|q| q.inputs.iter().map(|c| c.locator.clone()))
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let count = |n: usize| derived(&n.to_string(), "recovery.count", &count_inputs);
        let computed = self.quarters.iter().filter(|q| q.outcome.is_ok()).count();
        format!(
            "{{\"schema\":\"fmine.recovery.gap.v1\",\"rules_version\":{},\"digest\":{},\"issuer\":{},\"lane\":{},\"subject\":\"Lumwana\",\
            \"descriptive_only\":true,\"identifying\":false,\"interpretation_note\":{},\"model_caveat\":{},\
            \"workbook_through\":{},\"temporal_interpretation\":\"retrospective comparison using current verified model documents; workbook through is not an as-of cut or hindcast\",\
            \"vintage_policy\":\"earliest registered visible site-quarter release; later comparative values never repair an unusable first appearance\",\
            \"intervals\":\"conservative input-rounding enclosures, not confidence or prediction intervals\",\
            \"counts\":{{\"quarters\":{},\"computed\":{},\"refused\":{}}},\"ownership_source\":{},\"ownership_claim_id\":{},\
            \"ownership_claim\":{ownership_claim},\"documents\":[{documents}],\"training_inputs\":[{rows}],\"training_claims\":[{training_claims}],\"fit\":{fit},\"quarters\":[{quarters}]}}",
            js(RULES_VERSION),
            js(&self.digest().to_hex()),
            js(&self.issuer),
            js(if self.synthetic {
                "synthetic"
            } else {
                "public"
            }),
            js(kernel::NO_CLAIM),
            js(kernel::MODEL_CAVEAT),
            self.through.as_deref().map_or_else(|| "null".into(), js),
            count(self.quarters.len()),
            count(computed),
            count(self.quarters.len() - computed),
            transcription_json(&self.ownership_source),
            js(&self.ownership.claim_id.to_string())
        )
    }
}

/// Invented inputs use the same source adapter and model kernel, never the real historical source rows.
pub fn run_synthetic() -> Result<RecoveryReport> {
    let (profile, report, cells) = synthetic_inputs()?;
    assess(&profile, Some(&report), &cells)?
        .ok_or_else(|| source_error("synthetic recovery inputs are absent"))
}

fn synthetic_inputs() -> Result<(IssuerProfile, TranscriptionReport, Vec<KeyedCell>)> {
    use crate::sources::{DocumentRow, RegisteredSource};
    use fmine_core::hash::sha256;
    use fmine_io::xlsx::{
        MinimalCell, MinimalSheet, MinimalStyles, Workbook, build_minimal_xlsx_styled,
    };
    use fmine_registry::transcriptions::{agree, parse_transcriptions};

    let mut profile = crate::synthetic::synthetic_profile();
    let doc_file = "Synthetic_Recovery.pdf";
    let doc_bytes =
        b"Invented annual recovery history and full-site ownership context, not a public document.";
    let digest = sha256(doc_bytes).to_hex();
    let doc = DocumentRow {
        file: doc_file.into(),
        dir: "invented".into(),
        kind: "investor_deck".into(),
        url: "https://example.invalid/Synthetic_Recovery.pdf".into(),
        url_basis: "synthetic fixture".into(),
        published_at: "2025-01-01".into(),
        published_at_basis: "FACT: invented fixture date".into(),
        pages: 1,
        bytes: doc_bytes.len() as u64,
        sha256: digest.clone(),
    };
    profile.documents = vec![doc];
    let mut text = "schema = \"fmine.transcriptions.v1\"\nissuer = \"synthetic\"\n".to_string();
    let mut add = |quantity: String, token: String, year: u16, column: &str, unit: &str| {
        let row = if quantity == OWNERSHIP_QUANTITY {
            "Lumwana (100%)".into()
        } else {
            year.to_string()
        };
        for reader in ["synthetic-reader-a", "synthetic-reader-b"] {
            text.push_str(&format!("\n[[entry]]\ndoc = \"{doc_file}\"\ndoc_sha256 = \"{digest}\"\npage = 1\ntable = \"{TRAINING_TABLE}\"\nrow = \"{row}\"\ncol = \"{column}\"\ntoken = \"{token}\"\nquantity = \"{quantity}\"\nunit = \"{unit}\"\nbasis = \"100%\"\nperiod = \"{year}FY\"\nreader = \"{reader}\"\n"));
        }
    };
    for year in 2013..=2022 {
        let i = year - 2013;
        add(
            format!("{PREFIX}{year}_head_grade_pct"),
            format!("0.{:02}", 40 + i * 4),
            year,
            "Head Grade (% Cu)",
            "wt_pct",
        );
        add(
            format!("{PREFIX}{year}_recovery_pct"),
            format!("{}.{:01}", 91 + i * 4 / 10, i * 4 % 10),
            year,
            "Recovery (%)",
            "percent",
        );
    }
    add(
        OWNERSHIP_QUANTITY.into(),
        "100".into(),
        2025,
        "Interest (%)",
        "percent",
    );
    let registry = parse_transcriptions(&text, &profile.issuer, &profile.documents)
        .map_err(|e| source_error(e.to_string()))?;
    let report = TranscriptionReport {
        registry: "invented recovery rows".into(),
        documents: vec![CheckedDocument {
            file: doc_file.into(),
            kind: "investor_deck".into(),
            published_at: "2025-01-01".into(),
            published_at_basis: "FACT: invented fixture date".into(),
            sha256: digest,
        }],
        entries: registry.entries.len(),
        agreement: agree(&registry),
        tie: None,
    };
    let file = "Synthetic_Recovery_Stats.xlsx";
    let values = [
        ("B2", "0.62", "site:average grade:processed", "wt_pct"),
        ("B3", "90", "site::recovery rate (percent)", "percent"),
        ("B4", "6000", "site::tonnes processed (thousands)", "kt"),
        ("B5", "33", "site::production (thousands of tonnes)", "kt"),
    ];
    let bytes = build_minimal_xlsx_styled(
        &[MinimalSheet {
            name: "Minestats4".into(),
            hidden: false,
            cells: values
                .iter()
                .map(|(at, text, _, _)| {
                    (
                        at.to_string(),
                        MinimalCell::Styled {
                            raw: text.to_string(),
                            xf: u32::from(*at != "B2"),
                        },
                    )
                })
                .collect(),
        }],
        Some(&MinimalStyles {
            num_fmts: vec![],
            xfs: vec![2, 1],
        }),
    );
    let workbook = Workbook::from_bytes(file, &bytes)
        .map_err(|e| source_error(format!("synthetic workbook: {e:?}")))?;
    let sha = Digest32(workbook.sha256).to_hex();
    profile.sources = vec![RegisteredSource {
        file_name: file.into(),
        kind: SourceKind::MineStats,
        quarter: "2025Q1".into(),
        url: "https://example.invalid/Synthetic_Recovery_Stats.xlsx".into(),
        url_basis: "synthetic fixture".into(),
        bytes: bytes.len() as u64,
        sha256_hex: sha.clone(),
    }];
    let mut cells = Vec::new();
    for (at, text, line, unit) in values {
        let key = key::PublicKey {
            issuer: profile.issuer.clone(),
            subject: SUBJECT.into(),
            valid_period: key::Period {
                year: 2025,
                kind: PeriodKind::Quarter(1),
            },
            reporting_vintage: key::Vintage {
                release: "2025Q1".into(),
                file_sha256: sha.clone(),
            },
            ownership_basis: OwnershipBasis::Unprinted,
            scope: Scope::Site,
            unit: unit.into(),
            line: key::LineKey {
                label: line.into(),
                occurrence: 0,
            },
        };
        let value = PublicValue::Number {
            stored: text.into(),
            displayed: Ok(text.into()),
            under_status: None,
        };
        let mut cell = key::public_claim(&workbook, "Minestats4", at, key, value);
        // The invented fixture fixes its inputs; it does not represent measured knowledge of an issuer.
        cell.claim = Claim::new(
            line,
            cell.claim.value.clone(),
            &format!("invented-recovery|{at}|{text}"),
            Representation::Deterministic,
            SpatialSupport::PeriodAggregate,
            cell.claim.additivity,
            EvidenceClass::Assumed,
            EpistemicStatus::KnownUnknown(KuKind::Parametric),
            KnowledgeState::Known,
            Confidentiality::public(),
            vec![],
            vec![],
            cell.claim.anchors.clone(),
        );
        cells.push(cell);
    }
    Ok((profile, report, cells))
}
