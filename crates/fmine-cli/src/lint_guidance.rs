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

fn js(value: &str) -> String {
    format!("\"{}\"", crate::escape_json(value))
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

#[cfg(test)]
mod tests {
    use super::*;
    use fmine_public::sources::DocumentRow;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT: AtomicU64 = AtomicU64::new(0);

    struct Fixture {
        root: PathBuf,
        profile: IssuerProfile,
        transcription: String,
    }

    impl Fixture {
        fn new() -> Self {
            let root = loop {
                let candidate = std::env::temp_dir().join(format!(
                    "fmine-lint-guidance-{}-{}",
                    std::process::id(),
                    NEXT.fetch_add(1, Ordering::Relaxed)
                ));
                match std::fs::create_dir(&candidate) {
                    Ok(()) => break candidate,
                    Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                    Err(e) => panic!("create isolated synthetic fixture: {e}"),
                }
            };
            std::fs::create_dir_all(root.join("registries/transcriptions")).unwrap();
            std::fs::create_dir_all(root.join("corpus")).unwrap();
            let mut profile = fmine_public::synthetic::synthetic_profile();
            profile.subjects = vec!["Example Mine".into(), "Copper Mine".into()];
            profile.documents.clear();
            Self {
                root,
                profile,
                transcription: "schema = \"fmine.transcriptions.v1\"\nissuer = \"synthetic\"\n"
                    .into(),
            }
        }

        #[allow(clippy::too_many_arguments)]
        fn row(
            &mut self,
            file: &str,
            date: &str,
            subject: &str,
            metric: &str,
            token: &str,
            basis: &str,
            readers: usize,
        ) {
            let bytes = format!(
                "Invented guidance fixture: {file}|{date}|{subject}|{metric}|{token}|{basis}\n"
            );
            let hash = sha256(bytes.as_bytes()).to_hex();
            std::fs::write(self.root.join("corpus").join(file), &bytes).unwrap();
            self.profile.documents.push(DocumentRow {
                file: file.into(),
                dir: "corpus".into(),
                kind: "investor_deck".into(),
                url: format!("https://example.invalid/{file}"),
                url_basis: "synthetic".into(),
                published_at: date.into(),
                published_at_basis: "FACT: invented calendar for regression".into(),
                pages: 1,
                bytes: bytes.len() as u64,
                sha256: hash.clone(),
            });
            let unit = if metric == "production_koz" {
                "koz"
            } else if metric == "production_kt" {
                "kt"
            } else if metric.ends_with("per_lb") {
                "usd_per_lb"
            } else {
                "usd_per_oz"
            };
            for reader in 0..readers {
                let _ = write!(
                    self.transcription,
                    "\n[[entry]]\ndoc = \"{file}\"\ndoc_sha256 = \"{hash}\"\npage = 1\ntable = \"Invented guidance\"\nrow = \"{subject}\"\ncol = \"{metric}\"\ntoken = \"{token}\"\nquantity = \"guidance_{}_{metric}\"\nunit = \"{unit}\"\nbasis = \"{basis}\"\nperiod = \"2030FY\"\nreader = \"reader_{reader}\"\n",
                    crate::track_record_data::site_key(subject).replace('-', "_")
                );
            }
            self.save();
        }

        fn save(&self) {
            std::fs::write(
                self.root.join("registries/transcriptions/synthetic.toml"),
                &self.transcription,
            )
            .unwrap();
        }

        fn assess(&self, text: &str, cut: &str) -> GuidanceReport {
            assess(text, &self.profile, &self.root, cut)
        }

        fn register(&self, workbooks: bool) {
            let path = self.root.join("registries/sources/synthetic.toml");
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            let mut registry = if workbooks {
                let path = fmine_public::synthetic::write_issuer_workspace(&self.root).unwrap();
                std::fs::read_to_string(path).unwrap()
            } else {
                "schema = \"fmine.sources.v1\"\nissuer = \"synthetic\"\ncorpus_dir = \"corpus\"\nretrieved_utc = \"2030-08-01T00:00:00Z\"\nurl_prefix = \"https://example.invalid/\"\n".into()
            };
            for subject in &self.profile.subjects {
                let _ = write!(registry, "\n[[subject]]\nname = \"{subject}\"\n");
            }
            for doc in &self.profile.documents {
                let _ = write!(
                    registry,
                    "\n[[document]]\nfile = \"{}\"\nkind = \"{}\"\nurl = \"https://example.invalid/{}\"\nurl_basis = \"fetched and verified 2030-08-01\"\npublished_at = \"{}\"\npublished_at_basis = \"{}\"\npages = {}\nbytes = {}\nsha256 = \"{}\"\n",
                    doc.file,
                    doc.kind,
                    doc.file,
                    doc.published_at,
                    doc.published_at_basis,
                    doc.pages,
                    doc.bytes,
                    doc.sha256
                );
            }
            std::fs::write(path, registry).unwrap();
        }

        fn invoke(&self, cut: &str, format: &str) -> crate::Envelope {
            let args = [
                "lint",
                "draft.md",
                "--issuer",
                "synthetic",
                "--as-of",
                cut,
                "--format",
                format,
            ]
            .map(str::to_string);
            crate::invoke(&args, &self.root).0
        }
    }

    const DRAFT: &str = "Example Mine FY2030 100% basis gold production forecast is 125 koz.";

    #[test]
    fn latest_guidance_changes_at_publication_and_future_bytes_do_not_leak() {
        let mut f = Fixture::new();
        f.row(
            "original.txt",
            "2030-01-15",
            "Example Mine",
            "production_koz",
            "100 - 120",
            "100%",
            2,
        );
        f.row(
            "revision.txt",
            "2030-07-15",
            "Example Mine",
            "production_koz",
            "120 - 140",
            "100%",
            2,
        );
        let before = f.assess(DRAFT, "2030-07-14");
        assert_eq!(before.findings[0].comparison, Some("above_guidance"));
        assert_eq!(before.findings[0].evidence.len(), 1);
        let after = f.assess(DRAFT, "2030-07-15");
        assert_eq!(after.findings[0].comparison, Some("within_guidance"));
        assert_eq!(after.findings[0].evidence.len(), 2);
        assert_ne!(before.findings[0].selected, after.findings[0].selected);
        assert!(
            after
                .to_json()
                .contains("revision.txt:p1:Invented guidance")
        );
        assert!(after.to_json().contains("canonical_body_hex"));
        assert!(after.to_markdown().contains("2030-07-15"));
        std::fs::write(f.root.join("corpus/revision.txt"), b"altered later bytes").unwrap();
        assert_eq!(before.to_json(), f.assess(DRAFT, "2030-07-14").to_json());
        let altered = f.assess(DRAFT, "2030-07-15");
        assert_eq!(altered.findings[0].status, "guidance_refused");
        assert_eq!(
            altered.findings[0].refusal.as_ref().unwrap().code,
            "FMINE-REF-505"
        );
    }

    #[test]
    fn missing_or_pending_guidance_never_falls_back_to_an_older_target() {
        let mut f = Fixture::new();
        assert_eq!(
            f.assess(DRAFT, "2030-08-01").findings[0].status,
            "guidance_not_in_registered_record"
        );
        f.row(
            "original.txt",
            "2030-01-15",
            "Example Mine",
            "production_koz",
            "100 - 120",
            "100%",
            2,
        );
        f.row(
            "unfinished.txt",
            "2030-07-15",
            "Example Mine",
            "production_koz",
            "120 - 140",
            "100%",
            1,
        );
        let result = f.assess(DRAFT, "2030-08-01");
        assert_eq!(result.findings[0].status, "guidance_incomplete");
        assert!(result.findings[0].selected.is_none());
        assert!(result.findings[0].pending[0].contains("unfinished.txt"));
        assert_eq!(
            f.assess(DRAFT, "2030-07-14").findings[0].comparison,
            Some("above_guidance")
        );
    }

    #[test]
    fn copper_cost_ranges_use_exact_guidance_and_require_per_sold_pound() {
        let mut f = Fixture::new();
        f.row(
            "copper.txt",
            "2030-01-15",
            "Copper Mine",
            "aisc_usd_per_lb",
            "3.40 - 3.75",
            "100%;aisc=issuer-reported-per-sold-lb",
            2,
        );
        let draft =
            "Copper Mine FY2030 100% basis AISC guidance is $3.40 - 3.75/lb per pound sold.";
        let good = f.assess(draft, "2030-02-01");
        assert_eq!(
            good.findings[0].status,
            "forward_looking",
            "{}",
            good.to_json()
        );
        assert_eq!(good.findings[0].comparison, Some("within_guidance"));
        for (number, expected) in [
            ("3.80 - 4.00", "above_guidance"),
            ("3.20 - 3.30", "below_guidance"),
            ("3.20 - 3.60", "overlaps_guidance_boundary"),
        ] {
            assert_eq!(
                f.assess(&draft.replace("3.40 - 3.75", number), "2030-02-01")
                    .findings[0]
                    .comparison,
                Some(expected)
            );
        }
        let wrong = f.assess(&draft.replace("pound sold", "ounce sold"), "2030-02-01");
        assert_eq!(wrong.findings[0].status, "not_assessed");
        let wrong = f.assess(&draft.replace("/lb", "/oz"), "2030-02-01");
        assert_ne!(wrong.findings[0].status, "forward_looking");
    }

    #[test]
    fn ownership_series_remain_independent_and_amount_never_selects_a_share() {
        let mut f = Fixture::new();
        f.row(
            "full.txt",
            "2030-01-15",
            "Example Mine",
            "production_koz",
            "100 - 120",
            "100%",
            2,
        );
        f.row(
            "share.txt",
            "2030-01-15",
            "Example Mine",
            "production_koz",
            "80 - 96",
            "attributable:4/5",
            2,
        );
        assert_eq!(
            f.assess(DRAFT, "2030-02-01").findings[0].comparison,
            Some("above_guidance")
        );
        let attributable = DRAFT
            .replace("100% basis", "attributable")
            .replace("125", "90");
        assert_eq!(
            f.assess(&attributable, "2030-02-01").findings[0].comparison,
            Some("within_guidance")
        );
        f.row(
            "other-share.txt",
            "2030-02-15",
            "Example Mine",
            "production_koz",
            "70 - 80",
            "attributable:3/4",
            2,
        );
        let ambiguous = f.assess(&attributable, "2030-03-01");
        assert_eq!(ambiguous.findings[0].status, "guidance_ambiguous");
        assert_eq!(ambiguous.findings[0].evidence.len(), 2);
        assert!(ambiguous.findings[0].comparison.is_none());
        assert_eq!(
            f.assess(DRAFT, "2030-03-01").findings[0].comparison,
            Some("above_guidance")
        );
    }

    #[test]
    fn duplicate_latest_dates_and_reader_disagreement_refuse() {
        let mut f = Fixture::new();
        f.row(
            "one.txt",
            "2030-01-15",
            "Example Mine",
            "production_koz",
            "100 - 120",
            "100%",
            2,
        );
        f.row(
            "two.txt",
            "2030-01-15",
            "Example Mine",
            "production_koz",
            "120 - 140",
            "100%",
            2,
        );
        let result = f.assess(DRAFT, "2030-02-01");
        assert_eq!(result.findings[0].status, "guidance_refused");
        assert_eq!(
            result.findings[0].refusal.as_ref().unwrap().code,
            "FMINE-REF-104"
        );
        f.transcription =
            f.transcription
                .replacen("token = \"100 - 120\"", "token = \"101 - 120\"", 1);
        f.save();
        let result = f.assess(DRAFT, "2030-02-01");
        assert_eq!(
            result.findings[0].refusal.as_ref().unwrap().code,
            "FMINE-REF-202"
        );
    }

    #[test]
    fn unsupported_prose_stays_unchecked_and_unrelated_optional_documents_are_not_read() {
        let mut f = Fixture::new();
        f.row(
            "gold.txt",
            "2030-01-15",
            "Example Mine",
            "production_koz",
            "100 - 120",
            "100%",
            2,
        );
        f.row(
            "copper.txt",
            "2030-01-15",
            "Copper Mine",
            "production_kt",
            "30 - 40",
            "100%",
            2,
        );
        std::fs::write(f.root.join("corpus/copper.txt"), b"unrelated altered bytes").unwrap();
        assert_eq!(
            f.assess(DRAFT, "2030-02-01").findings[0].comparison,
            Some("above_guidance")
        );
        assert!(
            f.assess("Ordinary historical AISC is $100/oz.", "2030-02-01")
                .findings
                .is_empty()
        );
        for draft in [
            DRAFT.replace(" is ", " might be "),
            DRAFT.replace("125 koz", "125 koz and 200 koz"),
            DRAFT.replace("125 koz", "125 kt"),
            DRAFT.replace("Example Mine", "Unknown Mine"),
            DRAFT.replace("100% basis", ""),
            DRAFT.replace("FY2030", "Q1 2030"),
            DRAFT.replace("forecast", "forecast probability"),
        ] {
            let report = f.assess(&draft, "2030-02-01");
            assert_eq!(report.findings[0].status, "not_assessed", "{draft}");
            assert!(report.findings[0].comparison.is_none());
        }
    }

    #[test]
    fn cli_loads_document_only_guidance_and_keeps_optional_failures_out_of_numeric_lint() {
        for workbooks in [false, true] {
            let mut f = Fixture::new();
            f.row(
                "original.txt",
                "2030-01-15",
                "Example Mine",
                "production_koz",
                "100 - 120",
                "100%",
                2,
            );
            f.row(
                "revision.txt",
                "2030-07-15",
                "Example Mine",
                "production_koz",
                "120 - 140",
                "100%",
                2,
            );
            f.row(
                "optional-copper.txt",
                "2030-01-15",
                "Copper Mine",
                "production_kt",
                "30 - 40",
                "100%",
                2,
            );
            f.register(workbooks);
            std::fs::write(f.root.join("draft.md"), DRAFT).unwrap();
            let expected_root = workbooks.then(|| {
                let profile =
                    fmine_public::sources::load_issuer_profile(&f.root, "synthetic").unwrap();
                fmine_public::run_public_assurance(&profile, &f.root)
                    .unwrap()
                    .ledger_anchor
                    .state_root
                    .to_hex()
            });
            std::fs::write(
                f.root.join("corpus/optional-copper.txt"),
                b"irrelevant altered source",
            )
            .unwrap();
            let before = f.invoke("2030-07-14", "json");
            assert_eq!(before.exit_code, 0, "{}", before.to_json());
            assert!(
                before
                    .to_json()
                    .contains("\"forward_guidance\":{\"report_digest\"")
            );
            assert!(
                before
                    .to_json()
                    .contains("\"comparison\":\"above_guidance\"")
            );
            assert!(
                before
                    .to_json()
                    .contains("\"regulatory_compliance_claim\":false")
            );
            let state = expected_root
                .as_ref()
                .map_or_else(|| "null".to_string(), |root| js(root));
            assert!(
                before
                    .to_json()
                    .contains(&format!("\"source_state_root\":{state}"))
            );
            assert_eq!(before.to_json(), f.invoke("2030-07-14", "json").to_json());
            let after = f.invoke("2030-07-15", "markdown");
            assert_eq!(after.exit_code, 0, "{}", after.to_json());
            let md = after.rendered_markdown.as_ref().unwrap();
            assert!(md.contains("within_guidance"));
            assert!(md.contains("revision.txt:p1:Invented guidance"));
            std::fs::write(
                f.root.join("corpus/revision.txt"),
                b"altered selected revision",
            )
            .unwrap();
            assert_eq!(before.to_json(), f.invoke("2030-07-14", "json").to_json());
            let failure = f.invoke("2030-07-15", "json");
            assert_eq!(failure.exit_code, 0, "{}", failure.to_json());
            assert!(failure.to_json().contains("guidance_refused"));
            assert!(failure.to_json().contains("FMINE-REF-505"));
            assert!(failure.to_json().contains("\"lint\":{\"report_digest\""));
            assert!(
                failure
                    .to_json()
                    .contains(&format!("\"source_state_root\":{state}"))
            );
        }
    }
}
