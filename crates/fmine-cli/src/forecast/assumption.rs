//! DESIGN (BD-276): prospective, source-bound technical-report assumptions.
//! Published points/ranges retain an unstated probability; no confidence level is invented.

use std::fmt::Write as _;
use std::path::Path;

use fmine_calib::ledger::binding::PublicPeriod;
use fmine_calib::ledger::{ForecastMetadata, ForecastPayload};
use fmine_calib::{OutcomeCitation, OutcomeKind};
use fmine_claim::{AdditivityClass, Claim, Confidentiality, Representation, SpatialSupport};
use fmine_core::anchor::SourceAnchor;
use fmine_core::hash::sha256;
use fmine_evidence::{
    EpistemicStatus, EvidenceClass, KnowledgeState, KuKind, Refusal, RefusalCode,
};
use fmine_public::sources::{load_issuer_profile, registry_path, transcriptions_path};
use fmine_public::transcribe::{check_docs, load_transcription_quantities_as_of};
use fmine_registry::toml_subset::{TomlRow, parse_subset};
use fmine_registry::transcriptions::{Decimal, Token, parse_token};

use super::{Replay, Row, absent, hex, identifier, invalid, js, time};

pub const RULE: &str = "technical-report-assumptions/v1";
pub const NO_CLAIM: &str = "Prospectively recorded technical-report assertions, not calibrated forecasts or measured truth. Points and ranges retain an unstated probability. Source bytes and declared reader agreement are checked on admission; replay checks retained evidence, not current files or reader independence. Resolution uses the exact document, quantity, period, unit and basis frozen at registration, without implicit conversion, interpolation or substitution. A supplied publication date and bounded local host clock are not external timestamp proof. Generic interval scores are descriptive penalties at the cohort's declared alpha, not proper probabilistic scores for these unstated-level ranges. No reserve, operational, regulatory or investment authority is conferred.";

const INPUT_FIELDS: &[&str] = &[
    "id",
    "issuer",
    "document",
    "quantity",
    "subject",
    "period",
    "unit",
    "basis",
    "physical_start",
    "physical_end",
    "outcome_document",
    "outcome_quantity",
    "author_role",
    "lane",
];
const EVIDENCE_FIELDS: &[&str] = &[
    "source_document",
    "source_quantity",
    "source_token",
    "source_sha256",
    "source_locator",
    "source_path",
    "source_published_at",
    "source_published_at_basis",
    "source_readers",
    "sources_sha256",
    "transcriptions_sha256",
    "input_sha256",
];

#[derive(Debug, Clone)]
pub(super) struct Admission {
    row: TomlRow,
    pub start: i64,
    pub end: i64,
    pub issuer: String,
    source: Claim<String>,
}

fn field(row: &mut TomlRow, key: &str, value: &str) -> Result<(), Refusal> {
    // Use the same strict TOML grammar for generated operations and replay.
    let parsed = parse_subset(&format!("[[x]]\n{key} = {}\n", js(value)))
        .map_err(|e| invalid(e.to_string()))?;
    row.fields.extend(parsed.rows[0].fields.iter().cloned());
    Ok(())
}

fn encode(row: &TomlRow) -> Result<String, Refusal> {
    let values = Row(row);
    let mut text = format!("[[{}]]\n", row.table);
    for entry in &row.fields {
        let _ = writeln!(text, "{} = {}", entry.key, js(values.string(&entry.key)?));
    }
    Ok(text)
}

fn digest_file(path: &Path) -> Result<String, Refusal> {
    std::fs::read(path)
        .map(|bytes| sha256(&bytes).to_hex())
        .map_err(|e| {
            Refusal::new(
                RefusalCode::InputMissingOrUnreadable506,
                format!("{}: {e}", path.display()),
            )
        })
}

fn publication_end(date: &str) -> Result<i64, Refusal> {
    if !fmine_registry::sources::is_date(date) {
        return Err(invalid("source publication needs a valid YYYY-MM-DD date"));
    }
    time(&format!("{date}T00:00:00Z"))?
        .checked_add(86_400_000_000_000)
        .ok_or_else(|| invalid("source publication availability overflows"))
}

fn number(decimal: Decimal) -> Result<f64, Refusal> {
    // Decimal source text, not the workbook's binary stored value, defines this projection.
    let value = format!("{}e-{}", decimal.mantissa, decimal.scale)
        .parse::<f64>()
        .map_err(|_| invalid("source decimal cannot enter the floating-point scoring ledger"))?;
    if !value.is_finite() || (value == 0.0 && decimal.mantissa != 0) {
        return Err(invalid(
            "source decimal is not a representable finite forecast value",
        ));
    }
    Ok(value)
}

fn bounds(token: &str) -> Result<(f64, f64), Refusal> {
    match parse_token(token).map_err(invalid)? {
        Token::Point(value) => Ok((number(value)?, number(value)?)),
        Token::Range(lo, hi) => Ok((number(lo)?, number(hi)?)),
        Token::AtMost(_) | Token::AtLeast(_) => Err(invalid(
            "an open-ended report assumption supplies neither a point nor a finite range",
        )),
    }
}

fn check_period(row: &Row<'_>, start: i64, end: i64) -> Result<(), Refusal> {
    let period = row.string("period")?;
    if let Ok(calendar) = PublicPeriod::parse(period) {
        let (year, month) = match calendar {
            PublicPeriod::Quarter { year, quarter } => (year, 1 + (quarter - 1) * 3),
            PublicPeriod::Year(year)
            | PublicPeriod::FirstHalf(year)
            | PublicPeriod::NineMonths(year) => (year, 1),
        };
        let expected_start = time(&format!("{year:04}-{month:02}-01T00:00:00Z"))?;
        let expected_end = calendar
            .end_ns()?
            .checked_add(1)
            .ok_or_else(|| invalid("calendar target end overflows"))?;
        if start == expected_start && end == expected_end {
            return Ok(());
        }
    } else if period
        == format!(
            "UTC [{},{})",
            row.string("physical_start")?,
            row.string("physical_end")?
        )
    {
        return Ok(());
    }
    Err(invalid(
        "source period must match the exact calendar interval or explicit UTC [start,end) target; a historical assumption cannot be redated",
    ))
}

/// Verify one named source; a pending or conflicting competing reading cannot disappear in selection.
fn read_source(
    root: &Path,
    issuer: &str,
    document: &str,
    quantity: &str,
    technical: bool,
) -> Result<TomlRow, Refusal> {
    let profile = load_issuer_profile(root, issuer)?;
    let doc = profile
        .documents
        .iter()
        .find(|doc| doc.file == document)
        .ok_or_else(|| {
            absent(format!(
                "unregistered assumption source document {document}"
            ))
        })?;
    if technical && doc.kind != "technical_report" {
        return Err(invalid(
            "an assumption must cite a registered technical_report document",
        ));
    }
    check_docs(&[doc], root)?;
    let report = load_transcription_quantities_as_of(
        &profile,
        root,
        &doc.published_at,
        &[quantity.to_string()],
        Some(document),
    )?
    .ok_or_else(|| absent("the source needs a two-reader transcription registry"))?;
    if !report.agreement.is_complete_for_prefix("") || report.agreement.accepted.len() != 1 {
        return Err(Refusal::new(
            RefusalCode::NotObservable202,
            "the exact source quantity needs one unambiguous, complete two-reader reading",
        ));
    }
    let accepted = &report.agreement.accepted[0];
    if accepted.quantity != quantity || accepted.locator.doc != document {
        return Err(invalid(
            "the selected transcription differs from the frozen quantity/document",
        ));
    }
    let mut row = TomlRow {
        table: "source".into(),
        fields: vec![],
        line: 0,
    };
    for (key, value) in [
        ("period", accepted.period.as_str()),
        ("unit", &accepted.unit),
        ("basis", &accepted.basis),
        ("source_document", &doc.file),
        ("source_quantity", &accepted.quantity),
        ("source_token", &accepted.token),
        ("source_sha256", &doc.sha256),
        ("source_locator", &accepted.locator.to_string()),
        ("source_path", &format!("{}/{}", doc.dir, doc.file)),
        ("source_published_at", &doc.published_at),
        ("source_published_at_basis", &doc.published_at_basis),
        ("source_readers", &accepted.readers.join(",")),
        (
            "sources_sha256",
            &digest_file(&registry_path(root, issuer))?,
        ),
        (
            "transcriptions_sha256",
            &digest_file(&transcriptions_path(root, issuer))?,
        ),
    ] {
        field(&mut row, key, value)?;
    }
    Ok(row)
}

pub(super) fn prepare(root: &Path, input: &Path) -> Result<String, Refusal> {
    let text = super::live::read_request(&root.join(input))?;
    let doc = parse_subset(&text).map_err(|e| invalid(e.to_string()))?;
    if doc.top.len() != 1
        || !matches!(doc.top_get("schema").map(|f| &f.value),
        Some(fmine_registry::toml_subset::TomlValue::Str(s)) if s == "fmine.report-assumption.v1")
        || doc.rows.len() != 1
        || doc.rows[0].table != "assumption"
    {
        return Err(invalid(
            "expected fmine.report-assumption.v1 and one [[assumption]]",
        ));
    }
    let mut row = doc.rows[0].clone();
    let values = Row(&row);
    values.allowed(INPUT_FIELDS, &[])?;
    let source = read_source(
        root,
        values.string("issuer")?,
        values.string("document")?,
        values.string("quantity")?,
        true,
    )?;
    for key in ["period", "unit", "basis"] {
        if values.string(key)? != Row(&source).string(key)? {
            return Err(invalid(format!(
                "assumption {key} differs from its registered source"
            )));
        }
    }
    row.fields.extend(
        source
            .fields
            .into_iter()
            .filter(|f| !["period", "unit", "basis"].contains(&f.key.as_str())),
    );
    field(&mut row, "input_sha256", &sha256(text.as_bytes()).to_hex())?;
    row.table = "assumption_forecast".into();
    Admission::parse(&row)?;
    encode(&row)
}

impl Admission {
    fn parse(row: &TomlRow) -> Result<Self, Refusal> {
        let values = Row(row);
        values.allowed(INPUT_FIELDS, EVIDENCE_FIELDS)?;
        for key in INPUT_FIELDS.iter().chain(EVIDENCE_FIELDS) {
            values.string(key)?;
        }
        for key in ["id", "issuer", "author_role"] {
            identifier(values.string(key)?)?;
        }
        for key in [
            "source_sha256",
            "sources_sha256",
            "transcriptions_sha256",
            "input_sha256",
        ] {
            hex(values.string(key)?)?;
        }
        if values.string("document")? != values.string("source_document")?
            || values.string("quantity")? != values.string("source_quantity")?
        {
            return Err(invalid(
                "assumption source differs from its frozen document or quantity",
            ));
        }
        let lane = values.string("lane")?;
        if !matches!(lane, "synthetic" | "public") {
            return Err(invalid("assumption lane must be synthetic or public"));
        }
        let readers = values.identifiers("source_readers")?;
        if readers.len() < 2 || readers.windows(2).any(|pair| pair[0] >= pair[1]) {
            return Err(invalid(
                "assumption source requires distinct sorted reader role IDs",
            ));
        }
        let (start, end) = (values.time("physical_start")?, values.time("physical_end")?);
        if start >= end || publication_end(values.string("source_published_at")?)? >= start {
            return Err(invalid(
                "assumption publication must precede its positive physical interval",
            ));
        }
        check_period(&values, start, end)?;
        bounds(values.string("source_token")?)?;
        let body = encode(row)?;
        let source = Claim::new(
            format!(
                "technical-report:{}:{}",
                values.string("issuer")?,
                values.string("quantity")?
            ),
            values.string("source_token")?.to_string(),
            &sha256(body.as_bytes()).to_hex(),
            Representation::Deterministic,
            SpatialSupport::PeriodAggregate,
            AdditivityClass::NonAdditive,
            if lane == "synthetic" {
                EvidenceClass::Assumed
            } else {
                EvidenceClass::ISSUER
            },
            if lane == "synthetic" {
                EpistemicStatus::KnownUnknown(KuKind::Parametric)
            } else {
                EpistemicStatus::KnownKnown
            },
            if lane == "synthetic" {
                KnowledgeState::Unknown
            } else {
                KnowledgeState::Known
            },
            Confidentiality::public(),
            vec![],
            vec![],
            vec![SourceAnchor::new(
                values.string("source_path")?,
                hex(values.string("source_sha256")?)?.0,
                values.string("source_locator")?,
            )],
        );
        Ok(Self {
            row: row.clone(),
            start,
            end,
            issuer: values.string("issuer")?.into(),
            source,
        })
    }

    pub(super) fn to_json(&self) -> String {
        let values = Row(&self.row);
        let fields = self
            .row
            .fields
            .iter()
            .filter_map(|field| {
                values
                    .string(&field.key)
                    .ok()
                    .map(|value| format!("{}:{}", js(&field.key), js(value)))
            })
            .collect::<Vec<_>>()
            .join(",");
        format!(
            "{{\"schema\":\"fmine.report-assumption-admission.v1\",\"source_claim_id\":{},\"probability_level\":null,\"resolution_rule\":{},\"declarations\":{{{fields}}},\"no_claim\":{}}}",
            js(&self.source.claim_id.to_string()),
            js(RULE),
            js(NO_CLAIM)
        )
    }
}

pub(super) fn apply_forecast(
    state: &mut Replay,
    row: Row<'_>,
    lower: i64,
    upper: i64,
) -> Result<(), Refusal> {
    let admission = Admission::parse(row.0)?;
    if upper >= admission.start || lower < publication_end(row.string("source_published_at")?)? {
        return Err(Refusal::new(
            RefusalCode::LateRegistration206,
            "registration must certainly follow source publication and precede the physical interval",
        ));
    }
    let alias = row.string("id")?;
    if state.forecasts.contains_key(alias) {
        return Err(invalid("duplicate assumption forecast alias"));
    }
    let (lo, hi) = bounds(row.string("source_token")?)?;
    let metadata = ForecastMetadata {
        family: RULE.into(),
        author_role: row.string("author_role")?.into(),
        domain: row.string("basis")?.into(),
        horizon: row.string("period")?.into(),
        site: row.string("subject")?.into(),
        quantity: row.string("outcome_quantity")?.into(),
        unit: row.string("unit")?.into(),
    };
    let id = state.ledger.register_payload_with_metadata(
        format!("technical-report:{}", admission.source.claim_id),
        upper,
        admission.end,
        ForecastPayload::Interval {
            lo,
            hi,
            nominal: None,
        },
        metadata,
    )?;
    state.forecasts.insert(alias.into(), id);
    state.aliases.insert(id, alias.into());
    state.assumption_admissions.insert(id, admission);
    Ok(())
}

pub(super) fn validate_cohort(state: &Replay, name: &str, upper: i64) -> Result<(), Refusal> {
    let cohort = state
        .cohorts
        .get(name)
        .and_then(|id| state.ledger.cohort(*id))
        .ok_or_else(|| absent("frozen assumption cohort missing"))?;
    for id in &cohort.spec.members {
        if let Some(admission) = state.assumption_admissions.get(id)
            && (upper >= admission.start
                || cohort.spec.resolution_rule != RULE
                || cohort.spec.eligible_sources != vec![admission.issuer.clone()]
                || cohort
                    .spec
                    .members
                    .iter()
                    .any(|id| !state.assumption_admissions.contains_key(id)))
        {
            return Err(invalid(
                "assumption cohorts must freeze before physical start, use their exact resolution rule and sole issuer, and contain only report assumptions",
            ));
        }
    }
    Ok(())
}

pub(super) fn prepare_outcome(root: &Path, state: &Replay, alias: &str) -> Result<String, Refusal> {
    let id = state.forecast_id(alias)?;
    let admission = state
        .assumption_admissions
        .get(&id)
        .ok_or_else(|| absent("forecast is not a report assumption"))?;
    let original = Row(&admission.row);
    let mut source = read_source(
        root,
        &admission.issuer,
        original.string("outcome_document")?,
        original.string("outcome_quantity")?,
        false,
    )?;
    for key in ["period", "unit", "basis"] {
        if Row(&source).string(key)? != original.string(key)? {
            return Err(invalid(format!(
                "reported outcome {key} differs from the frozen assumption"
            )));
        }
    }
    source.table = "assumption_outcome".into();
    field(&mut source, "forecast", alias)?;
    field(
        &mut source,
        "assumption_claim",
        &admission.source.claim_id.to_string(),
    )?;
    encode(&source)
}

pub(super) fn apply_outcome(
    state: &mut Replay,
    row: Row<'_>,
    lower: i64,
    upper: i64,
) -> Result<(), Refusal> {
    row.allowed(
        &["forecast", "assumption_claim", "period", "unit", "basis"],
        &EVIDENCE_FIELDS[..EVIDENCE_FIELDS.len() - 1],
    )?;
    for key in &EVIDENCE_FIELDS[..EVIDENCE_FIELDS.len() - 1] {
        row.string(key)?;
    }
    let id = state.forecast_id(row.string("forecast")?)?;
    let admission = state
        .assumption_admissions
        .get(&id)
        .ok_or_else(|| absent("forecast is not a report assumption"))?;
    let original = Row(&admission.row);
    if row.string("assumption_claim")? != admission.source.claim_id.to_string() {
        return Err(invalid("outcome binds a different assumption"));
    }
    if row.string("source_document")? != original.string("outcome_document")?
        || row.string("source_quantity")? != original.string("outcome_quantity")?
    {
        return Err(invalid("outcome changes the frozen document or quantity"));
    }
    if !state.ledger.outcomes(id).is_empty() {
        return Err(Refusal::new(
            RefusalCode::FrozenRecordMutation507,
            "the frozen report-assumption outcome was already recorded",
        ));
    }
    for key in ["period", "unit", "basis"] {
        if row.string(key)? != original.string(key)? {
            return Err(invalid(format!("outcome changes the frozen {key}")));
        }
    }
    let readers = row.identifiers("source_readers")?;
    if readers.len() < 2 || readers.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(invalid("outcome needs distinct sorted reader roles"));
    }
    for key in ["sources_sha256", "transcriptions_sha256"] {
        hex(row.string(key)?)?;
    }
    let (lo, hi) = bounds(row.string("source_token")?)?;
    if !matches!(
        parse_token(row.string("source_token")?).map_err(invalid)?,
        Token::Point(_)
    ) || lo != hi
    {
        return Err(invalid(
            "a scored reported outcome must be a printed point, not a range or bound",
        ));
    }
    let available = publication_end(row.string("source_published_at")?)?;
    if available > lower || available - 86_400_000_000_000 < admission.end {
        return Err(Refusal::new(
            RefusalCode::LateRegistration206,
            "outcome publication must follow the entire target interval and precede the writer's earliest clock",
        ));
    }
    let source_json = format!(
        "{{\"schema\":\"fmine.report-assumption-outcome.v1\",\"forecast_id\":{},\"assumption_claim_id\":{},\"source_locator\":{},\"source_sha256\":{},\"printed_token\":{},\"published_at\":{},\"no_claim\":{}}}",
        js(&id.to_string()),
        js(row.string("assumption_claim")?),
        js(row.string("source_locator")?),
        js(row.string("source_sha256")?),
        js(row.string("source_token")?),
        js(row.string("source_published_at")?),
        js(NO_CLAIM)
    );
    let citation = OutcomeCitation {
        source: admission.issuer.clone(),
        vintage: original.string("outcome_document")?.into(),
        locator: row.string("source_locator")?.into(),
        sha256: hex(row.string("source_sha256")?)?.0,
    };
    state.ledger.append_outcome(
        id,
        available,
        upper,
        OutcomeKind::Published {
            value: lo,
            citation,
        },
    )?;
    state.assumption_outcomes.insert(id, source_json);
    Ok(())
}
