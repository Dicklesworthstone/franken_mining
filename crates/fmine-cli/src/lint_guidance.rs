//! DESIGN (BD-345.S4): bounded forward-looking sentences against dated registered guidance.
//! `forecast` or `guidance` explicitly marks the sentence. This compares stated bounds with the latest
//! registered issuer target on the stated basis; it neither verifies a forecast nor authorizes disclosure.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::Path;

use fmine_calib::guidance_scores::{self, Guidance, Side};
use fmine_core::hash::{Digest32, blake3_hash, sha256};
use fmine_evidence::{Refusal, RefusalCode};
use fmine_io::numtok::{
    BasisTok, Currency, Dec, NumberToken, PeriodTok, Span, Token, UnitTok, tokenize,
};
use fmine_numerics::exact_feas::{Interval, Rat};
use fmine_public::sources::IssuerProfile;
use fmine_public::transcribe::{TranscriptionReport, load_transcription_quantities_as_of};

use crate::json_string as js;
use crate::track_record_binding::{self, TrackRecordInput, TrackRecordRow};

pub(super) const NO_CLAIM: &str = "Compares explicitly marked forecast/guidance sentences with the latest dated registered issuer guidance on the stated ownership and metric basis. Stated draft bounds are compared exactly; no probability, forecast verification, completeness of the public record or disclosure permission is inferred.";

#[derive(Debug, Clone)]
struct Request {
    span: Span,
    number: NumberToken,
    subject: String,
    metric: &'static str,
    year: u16,
    attributable: bool,
}

#[derive(Debug, Clone)]
struct Finding {
    span: Span,
    raw: Option<String>,
    status: &'static str,
    reason: String,
    comparison: Option<&'static str>,
    selected: Option<String>,
    evidence: Vec<TrackRecordRow>,
    pending: Vec<String>,
    refusal: Option<Refusal>,
}

#[derive(Debug, Clone)]
pub(super) struct GuidanceReport {
    as_of: String,
    draft_digest: Digest32,
    findings: Vec<Finding>,
}

fn erase(text: &mut String, span: Span) {
    text.replace_range(span.0..span.1, &" ".repeat(span.1 - span.0));
}

fn word(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

fn phrase(text: &mut String, wanted: &str) -> usize {
    let mut n = 0;
    loop {
        let span = text.match_indices(wanted).find_map(|(start, _)| {
            let end = start + wanted.len();
            (text[..start].chars().next_back().is_none_or(|c| !word(c))
                && text[end..].chars().next().is_none_or(|c| !word(c)))
            .then_some((start, end))
        });
        let Some(span) = span else { return n };
        erase(text, span);
        n += 1;
    }
}

// One line is the declared predicate boundary. Arbitrary prose, conjunctions and several numeric
// assertions are left unchecked, so removing a marker cannot silently change a statement's meaning.
fn request(line: &str, offset: usize, profile: &IssuerProfile) -> Result<Request, &'static str> {
    let tokens = tokenize(line);
    let mut rest = line.to_ascii_lowercase();
    let (mut numbers, mut periods, mut bases, mut units) =
        (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    for token in &tokens {
        match token {
            Token::Number(n) => {
                numbers.push(n);
                units.extend(n.unit);
            }
            Token::Period { kind, .. } => periods.push(*kind),
            Token::Basis { kind, .. } => bases.push(*kind),
            Token::Unit { kind, .. } => units.push(*kind),
            Token::UnsupportedNumber { .. } => return Err("unsupported_numeric_syntax"),
        }
        erase(&mut rest, token.span());
    }
    let [number] = numbers.as_slice() else {
        return Err("one_numeric_point_or_range_required");
    };
    let [PeriodTok::Fy { year }] = periods.as_slice() else {
        return Err("one_explicit_fiscal_year_required");
    };
    let ownership: Vec<_> = bases.iter().filter(|b| b.is_ownership()).copied().collect();
    let attributable = match ownership.as_slice() {
        [BasisTok::Hundred] => false,
        [BasisTok::Attributable] => true,
        _ => return Err("one_explicit_ownership_basis_required"),
    };
    let mut subjects = Vec::new();
    let mut names = profile.subjects.clone();
    names.sort_by_key(|s| std::cmp::Reverse(s.len()));
    for name in names {
        for _ in 0..phrase(&mut rest, &name.to_ascii_lowercase()) {
            subjects.push(name.clone());
        }
    }
    let [subject] = subjects.as_slice() else {
        return Err("one_registered_subject_required");
    };
    if profile.regions.contains(subject) {
        return Err("site_guidance_required");
    }
    let mut metrics = Vec::new();
    for (label, metric) in [
        ("all-in sustaining costs", "aisc"),
        ("all in sustaining costs", "aisc"),
        ("total cash costs", "tcc"),
        ("copper c1 cash costs", "c1"),
        ("c1 cash costs", "c1"),
        ("copper production", "production_kt"),
        ("gold production", "production_koz"),
        ("aisc", "aisc"),
        ("tcc", "tcc"),
        ("c1", "c1"),
    ] {
        for _ in 0..phrase(&mut rest, label) {
            metrics.push(metric);
        }
    }
    let [metric] = metrics.as_slice() else {
        return Err("one_supported_metric_required");
    };
    let unit = match units.as_slice() {
        [unit] => *unit,
        _ => return Err("one_explicit_unit_required"),
    };
    let metric = match (*metric, unit) {
        ("production_koz", UnitTok::Koz) => "production_koz",
        ("production_kt", UnitTok::Kt) => "production_kt",
        ("aisc", UnitTok::UsdPerOz | UnitTok::DollarPerOz) => "aisc_usd_per_oz",
        ("aisc", UnitTok::UsdPerLb | UnitTok::DollarPerLb) => "aisc_usd_per_lb",
        ("tcc", UnitTok::UsdPerOz | UnitTok::DollarPerOz) => "tcc_usd_per_oz",
        ("c1", UnitTok::UsdPerLb | UnitTok::DollarPerLb) => "c1_usd_per_lb",
        _ => return Err("metric_unit_mismatch_no_conversion"),
    };
    let cost = !metric.starts_with("production_");
    if number
        .currency
        .is_some_and(|c| !cost || !matches!(c, Currency::Usd | Currency::Dollar))
    {
        return Err("currency_mismatch_no_conversion");
    }
    let denominator = if metric.ends_with("_per_lb") {
        BasisTok::PerLbSold
    } else {
        BasisTok::PerOzSold
    };
    if cost
        && bases
            .iter()
            .filter(|b| b.is_denominator())
            .copied()
            .collect::<Vec<_>>()
            != [denominator]
    {
        return Err("explicit_sold_denominator_required");
    }
    if bases.iter().any(|b| match b {
        BasisTok::Hundred | BasisTok::Attributable => false,
        BasisTok::PerOzSold | BasisTok::PerLbSold => !cost || *b != denominator,
        BasisTok::NonGaap => !cost,
        BasisTok::GoldOunces => metric != "production_koz" && !metric.ends_with("_per_oz"),
        _ => true,
    }) {
        return Err("basis_not_established_by_registered_guidance");
    }
    if number.hedge.is_some() {
        return Err("qualified_forecast_not_in_bounded_grammar");
    }
    let markers = phrase(&mut rest, "forecast") + phrase(&mut rest, "guidance");
    let words: Vec<_> = rest
        .split(|c: char| c.is_whitespace() || matches!(c, ',' | ':' | '.'))
        .filter(|w| !w.is_empty())
        .collect();
    if markers != 1
        || words
            .iter()
            .filter(|w| matches!(**w, "is" | "equals" | "="))
            .count()
            != 1
        || words.iter().any(|w| {
            !matches!(
                *w,
                "is" | "equals"
                    | "="
                    | "for"
                    | "in"
                    | "of"
                    | "the"
                    | "on"
                    | "a"
                    | "basis"
                    | "reported"
            )
        })
    {
        return Err("unsupported_forward_looking_prose");
    }
    let mut number = (*number).clone();
    number.span.0 += offset;
    number.span.1 += offset;
    Ok(Request {
        span: number.span,
        number,
        subject: subject.clone(),
        metric,
        year: *year,
        attributable,
    })
}

fn refusal(code: RefusalCode, reason: impl Into<String>) -> Refusal {
    Refusal::new(code, format!("forward guidance: {}", reason.into()))
}

fn decimal(d: Dec) -> Result<Rat, Refusal> {
    let den = 10_i128.checked_pow(d.scale).ok_or_else(|| {
        refusal(
            RefusalCode::NumericalCertificateFailed207,
            "draft decimal scale overflow",
        )
    })?;
    Rat::new(d.mantissa, den)
        .map_err(|e| refusal(RefusalCode::NumericalCertificateFailed207, e.to_string()))
}

fn comparison(request: &Request, row: &TrackRecordRow) -> Result<&'static str, Refusal> {
    let lo = decimal(request.number.value)?;
    let hi = decimal(request.number.range_with.unwrap_or(request.number.value))?;
    if lo.num() < 0
        || hi.num() < 0
        || lo
            .checked_cmp(hi)
            .map_err(|e| refusal(RefusalCode::NumericalCertificateFailed207, e.to_string()))?
            .is_gt()
    {
        return Err(refusal(
            RefusalCode::SourceCellUnusable208,
            "draft target is negative or reversed",
        ));
    }
    let guidance: Guidance = row.guidance.value.target.into();
    let side = |value| {
        guidance_scores::score(
            guidance,
            guidance_scores::PrintedOutcome {
                value,
                interval: Interval {
                    lo: value,
                    hi: value,
                    lo_strict: false,
                    hi_strict: false,
                },
            },
        )
        .map(|s| s.side)
        .map_err(|e| refusal(RefusalCode::NumericalCertificateFailed207, e.to_string()))
    };
    let (low, high) = (side(lo)?, side(hi)?);
    Ok(match (low, high) {
        (Side::Above, _) => "above_guidance",
        (_, Side::Below) => "below_guidance",
        (Side::Inside, Side::Inside) => "within_guidance",
        _ => "overlaps_guidance_boundary",
    })
}

#[derive(Debug, Clone)]
struct Loaded {
    rows: Vec<TrackRecordRow>,
    pending: Vec<String>,
}

fn load(
    profile: &IssuerProfile,
    root: &Path,
    as_of: &str,
    quantity: &str,
) -> Result<Loaded, Refusal> {
    let Some(transcriptions) =
        load_transcription_quantities_as_of(profile, root, as_of, &[quantity.to_string()], None)?
    else {
        return Ok(Loaded {
            rows: Vec::new(),
            pending: Vec::new(),
        });
    };
    bind(profile, as_of, transcriptions)
}

fn bind(
    profile: &IssuerProfile,
    as_of: &str,
    transcriptions: TranscriptionReport,
) -> Result<Loaded, Refusal> {
    let mut pending: Vec<_> = transcriptions
        .agreement
        .single_reader
        .iter()
        .map(|(at, reader)| format!("{at} ({reader})"))
        .collect();
    pending.sort();
    // An entered but unfinished revision cannot disappear so that an older accepted target wins.
    if !pending.is_empty() || transcriptions.agreement.accepted.is_empty() {
        return Ok(Loaded {
            rows: Vec::new(),
            pending,
        });
    }
    let available_by = transcriptions
        .documents
        .iter()
        .map(|d| d.published_at.as_str())
        .max()
        .unwrap_or(as_of);
    let report = track_record_binding::assess(TrackRecordInput {
        issuer: &profile.issuer,
        site: None,
        through: None,
        as_of,
        available_by,
        transcriptions: &transcriptions,
        claims: &[],
        statuses: &[],
        synthetic: profile.issuer == fmine_public::synthetic::SYNTHETIC_ISSUER,
    })?;
    Ok(Loaded {
        rows: report.rows,
        pending,
    })
}

fn finding(request: &Request, loaded: &Loaded) -> Result<Finding, Refusal> {
    let mut result = Finding {
        span: request.span,
        raw: Some(request.number.raw.clone()),
        status: "forward_looking",
        reason: "latest_registered_guidance_on_stated_basis".into(),
        comparison: None,
        selected: None,
        evidence: Vec::new(),
        pending: loaded.pending.clone(),
        refusal: None,
    };
    if !loaded.pending.is_empty() {
        result.status = "guidance_incomplete";
        result.reason = "pending_readers_prevent_latest_vintage_selection".into();
        return Ok(result);
    }
    let site = crate::track_record_data::site_key(&request.subject);
    let mut shares = BTreeSet::new();
    for row in &loaded.rows {
        if crate::track_record_data::site_key(&row.site) != site
            || row.year != request.year
            || row.metric != request.metric
        {
            continue;
        }
        let (basis, _) =
            crate::track_record_data::specification(&row.guidance_source, request.metric)?;
        if basis.0 != request.attributable {
            continue;
        }
        shares.insert((basis.1.num(), basis.1.den()));
        result.evidence.push(row.clone());
    }
    result.evidence.sort_by(|a, b| {
        (&a.published_at, &a.guidance_source.locator)
            .cmp(&(&b.published_at, &b.guidance_source.locator))
    });
    if result.evidence.is_empty() {
        result.status = "guidance_not_in_registered_record";
        result.reason = "no_agreed_guidance_on_the_requested_year_metric_and_basis_by_cut".into();
    } else if shares.len() > 1 {
        result.status = "guidance_ambiguous";
        result.reason = "attributable_share_unspecified".into();
    } else if let Some(latest) = result.evidence.last() {
        let latest_date = &latest.published_at;
        if result
            .evidence
            .iter()
            .filter(|r| &r.published_at == latest_date)
            .count()
            != 1
        {
            result.status = "guidance_ambiguous";
            result.reason = "several_guidance_statements_share_latest_publication_date".into();
        } else {
            result.comparison = Some(comparison(request, latest)?);
            result.selected = Some(latest.guidance.claim_id.to_string());
        }
    }
    Ok(result)
}

/// Optional guidance failures stay in their own findings; they never discard a historical lint report.
pub(super) fn assess(
    draft: &str,
    profile: &IssuerProfile,
    root: &Path,
    as_of: &str,
) -> GuidanceReport {
    let mut findings = Vec::new();
    let mut cache = BTreeMap::new();
    let mut offset = 0;
    for line in draft.split_inclusive('\n') {
        let mut probe = line.to_ascii_lowercase();
        if phrase(&mut probe, "forecast") + phrase(&mut probe, "guidance") == 0 {
            offset += line.len();
            continue;
        }
        let parsed = request(line, offset, profile);
        let (span, raw, result) = match parsed {
            Err(reason) => {
                findings.push(Finding {
                    span: (offset, offset + line.len()),
                    raw: None,
                    status: "not_assessed",
                    reason: reason.into(),
                    comparison: None,
                    selected: None,
                    evidence: Vec::new(),
                    pending: Vec::new(),
                    refusal: None,
                });
                offset += line.len();
                continue;
            }
            Ok(request) => {
                let quantity = format!(
                    "guidance_{}_{}",
                    crate::track_record_data::site_key(&request.subject).replace('-', "_"),
                    request.metric
                );
                let loaded = cache
                    .entry(quantity.clone())
                    .or_insert_with(|| load(profile, root, as_of, &quantity));
                (
                    request.span,
                    request.number.raw.clone(),
                    loaded
                        .as_ref()
                        .map_err(Clone::clone)
                        .and_then(|loaded| finding(&request, loaded)),
                )
            }
        };
        findings.push(result.unwrap_or_else(|refusal| Finding {
            span,
            raw: Some(raw),
            status: "guidance_refused",
            reason: "registered_guidance_could_not_be_authenticated_or_compared".into(),
            comparison: None,
            selected: None,
            evidence: Vec::new(),
            pending: Vec::new(),
            refusal: Some(refusal),
        }));
        offset += line.len();
    }
    GuidanceReport {
        as_of: as_of.into(),
        draft_digest: sha256(draft.as_bytes()),
        findings,
    }
}

impl GuidanceReport {
    fn body(&self) -> String {
        let findings = self.findings.iter().map(|f| {
            let refusal = f.refusal.as_ref().map_or_else(|| "null".into(), |r| format!("{{\"code\":{},\"what\":{},\"fix\":{}}}", js(r.code), js(&r.what), js(r.fix)));
            format!("{{\"span\":{{\"start_byte\":{},\"end_byte\":{}}},\"raw\":{},\"status\":{},\"reason\":{},\"comparison\":{},\"selected_guidance_claim\":{},\"guidance_vintages\":[{}],\"pending\":[{}],\"refusal\":{}}}", f.span.0, f.span.1, f.raw.as_deref().map_or_else(|| "null".into(), js), js(f.status), js(&f.reason), f.comparison.map_or_else(|| "null".into(), js), f.selected.as_deref().map_or_else(|| "null".into(), js), f.evidence.iter().map(evidence_json).collect::<Vec<_>>().join(","), f.pending.iter().map(|p| js(p)).collect::<Vec<_>>().join(","), refusal)
        }).collect::<Vec<_>>().join(",");
        format!(
            "\"schema\":\"fmine.lint.forward-guidance.v1\",\"as_of\":{},\"draft_sha256\":{},\"no_claim\":{},\"comparison_semantics\":\"exact stated point or range bounds; no assumed coverage or rounding tolerance\",\"supported_grammar\":\"one line, one forecast or guidance marker, one equality, one registered site, FYyear, ownership basis, metric and explicit unit; costs require a sold denominator\",\"findings\":[{}]",
            js(&self.as_of),
            js(&self.draft_digest.to_hex()),
            js(NO_CLAIM),
            findings
        )
    }

    pub(super) fn to_json(&self) -> String {
        let body = self.body();
        format!(
            "{{\"report_digest\":{},{} }}",
            js(&blake3_hash(body.as_bytes()).to_hex()),
            body
        )
    }

    pub(super) fn to_markdown(&self) -> String {
        let mut out = format!(
            "\n## Forward-looking guidance at {}\n\n{}\n\n",
            self.as_of, NO_CLAIM
        );
        for f in &self.findings {
            let _ = writeln!(
                out,
                "- Bytes {}..{}: `{}`; **{}**; {}. {}",
                f.span.0,
                f.span.1,
                f.raw.as_deref().unwrap_or("unsupported predicate"),
                f.status,
                f.comparison.unwrap_or("comparison unavailable"),
                f.reason
            );
            if let Some(id) = &f.selected {
                let _ = writeln!(out, "  Selected guidance Claim: `{id}`.");
            }
            for row in &f.evidence {
                let at = row
                    .guidance_source
                    .locator
                    .to_string()
                    .replace(['\n', '\r', '`'], " ");
                let basis = row.basis.replace(['\n', '\r', '`'], " ");
                let _ = writeln!(
                    out,
                    "  - {}: `{at}`, sha256 `{}`, basis `{basis}`; Claim `{}`.",
                    row.published_at, row.guidance_source.doc_sha256, row.guidance.claim_id
                );
            }
            if let Some(r) = &f.refusal {
                let _ = writeln!(out, "  {}: {}", r.code, r.what.replace(['\n', '\r'], " "));
            }
        }
        out
    }
}

fn evidence_json(row: &TrackRecordRow) -> String {
    let hex = |bytes: Vec<u8>| {
        let mut out = String::with_capacity(bytes.len() * 2);
        for byte in bytes {
            let _ = write!(out, "{byte:02x}");
        }
        out
    };
    format!(
        "{{\"statement\":{},\"guidance_canonical_body_hex\":{},\"source_canonical_body_hex\":{}}}",
        row.to_json(),
        js(&hex(row.guidance.canonical_body())),
        js(&hex(row.guidance_source_claim.canonical_body()))
    )
}

include!("../../xtask/testdata/cli_lint_guidance_unit_tests.rs");
