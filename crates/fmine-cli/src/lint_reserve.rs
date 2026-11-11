//! DESIGN (BD-345.S3/BD-063): bounded company reserve-replacement equalities over authenticated Claims.
//! Named definitions, annual windows, metals and GEO decks select candidates before numeric checking.

use std::fmt::Write as _;
use std::path::Path;

use fmine_core::hash::sha256;
use fmine_evidence::Refusal;
use fmine_io::numtok::{BasisTok, Dec, NumberToken, PeriodTok, Span, Token, UnitTok, tokenize};
use fmine_metrics::reserve_replacement::{Bounds, SeriesAssessment};
use fmine_numerics::exact_feas::Rat;
use fmine_public::sources::IssuerProfile;
use fmine_registry::metrics::ReserveMetal;

use crate::{json_string as js, reserve};

pub(super) const NO_CLAIM: &str = "Checks one explicitly named company reserve-replacement definition, attributable metal basis, acquisitions/divestments exclusion, complete annual window and GEO deck against authenticated registered source Claims. Verified means the exact source rounding enclosure fits the draft's printed precision. It neither establishes reserve truth nor classifies reserves, establishes disclosure permission, authenticates an issuer assertion, or invents a tolerance for approximate prose.";

pub(super) const ROBOT_DOCS_JSON: &str = r#"{"input":"--reserve-bindings selects the existing registered reserve-replacement annual selectors; --issuer and --as-of are mandatory","predicate":"One company attributable gold or GEO reserve-replacement percentage for an explicit YYYY-YYYY window, excluding acquisitions and divestments, under a backtick-quoted definition; GEO also names its backtick-quoted deck definition using","example":"Company attributable gold reserve replacement for 2030-2032 excluding acquisitions and divestments under `replacement.gold@fixture` is 200%.","cited":"Link the percentage to the exact cumulative Claim ID emitted by reserve replacement","discovery":"--discover retains all matching bound series; multiple matches stay ambiguous and source failures never select a survivor","rounding":"The entire exact source rounding enclosure must fit the draft half-step interval; overlap alone is rounding_not_established","limits":"Approximate prose, unnamed definitions, omitted metal or ownership and unsupplied annual windows are unchecked; no reserve classification or disclosure permission"}"#;

struct Request {
    number: NumberToken,
    definition: String,
    geo_definition: Option<String>,
    metal: ReserveMetal,
    years: (u16, u16),
    citation: Option<String>,
}

struct Finding {
    span: Span,
    raw: Option<String>,
    mode: &'static str,
    verdict: &'static str,
    reason: String,
    candidates: Vec<String>,
    claim: Option<String>,
}

pub(super) struct Report {
    as_of: String,
    draft_digest: String,
    source: Result<reserve::Loaded, Refusal>,
    findings: Vec<Finding>,
}

fn erase(text: &mut String, span: Span) {
    text.replace_range(span.0..span.1, &" ".repeat(span.1 - span.0));
}

fn word(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

fn phrase(text: &mut String, wanted: &str) -> usize {
    let spans: Vec<_> = text
        .match_indices(wanted)
        .filter_map(|(start, _)| {
            let end = start + wanted.len();
            (!text[..start].chars().next_back().is_some_and(word)
                && !text[end..].chars().next().is_some_and(word))
            .then_some((start, end))
        })
        .collect();
    for &span in &spans {
        erase(text, span);
    }
    spans.len()
}

/// Quoted definition identifiers are not prose numbers; preserve their case and erase only their bytes.
fn named(text: &mut String, marker: &str) -> Result<Option<String>, &'static str> {
    let prefix = format!("{marker} `");
    let lower = text.to_ascii_lowercase();
    let matches: Vec<_> = lower
        .match_indices(&prefix)
        .filter(|(start, _)| !lower[..*start].chars().next_back().is_some_and(word))
        .map(|(start, _)| start)
        .collect();
    let [start] = matches.as_slice() else {
        return if matches.is_empty() {
            Ok(None)
        } else {
            Err("one_named_definition_or_deck_required")
        };
    };
    let value_start = start + prefix.len();
    let end = text[value_start..]
        .find('`')
        .map(|n| value_start + n)
        .ok_or("unterminated_definition_or_deck")?;
    let id = &text[value_start..end];
    if id.is_empty()
        || !id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._@-".contains(&b))
    {
        return Err("invalid_definition_or_deck_identifier");
    }
    let id = id.to_string();
    erase(text, (*start, end + 1));
    Ok(Some(id))
}

fn citation(text: &mut String) -> Result<Option<(String, Span)>, &'static str> {
    if !text.contains("fmine:") {
        return Ok(None);
    }
    let start = text.find('[').ok_or("malformed_reserve_claim_link")?;
    let end_label = text[start + 1..]
        .find("](fmine:")
        .map(|n| start + 1 + n)
        .ok_or("malformed_reserve_claim_link")?;
    let id_start = end_label + "](fmine:".len();
    let end = text[id_start..]
        .find(')')
        .map(|n| id_start + n)
        .ok_or("malformed_reserve_claim_link")?;
    let id = &text[id_start..end];
    if id.len() != 36
        || !id.starts_with("clm_")
        || !id[4..].bytes().all(|b| b.is_ascii_hexdigit())
        || text.matches("fmine:").count() != 1
    {
        return Err("one_canonical_reserve_claim_link_required");
    }
    let id = id.to_string();
    erase(text, (start, start + 1));
    erase(text, (end_label, end + 1));
    Ok(Some((id, (start + 1, end_label))))
}

fn request(line: &str, offset: usize) -> Result<Request, &'static str> {
    let mut rest = line.to_string();
    let definition = named(&mut rest, "under")?.ok_or("explicit_reserve_definition_required")?;
    let geo_definition = named(&mut rest, "using")?;
    let link = citation(&mut rest)?;
    let tokens = tokenize(&rest);
    let (mut numbers, mut periods, mut bases, mut units) =
        (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    rest.make_ascii_lowercase();
    for token in &tokens {
        match token {
            Token::Number(n) => numbers.push(n),
            Token::Period { kind, .. } => periods.push(*kind),
            Token::Basis { kind, .. } => bases.push(*kind),
            Token::Unit { kind, .. } => units.push(*kind),
            Token::UnsupportedNumber { .. } => return Err("unsupported_numeric_syntax"),
        }
        erase(&mut rest, token.span());
    }
    let [number] = numbers.as_slice() else {
        return Err("one_percentage_point_required");
    };
    if !number.percent
        || number.currency.is_some()
        || number.scale_word.is_some()
        || number.hedge.is_some()
        || number.range_with.is_some()
        || number.unit.is_some_and(|u| u != UnitTok::Percent)
        || !units.is_empty()
    {
        return Err("unqualified_percentage_point_required");
    }
    if link
        .as_ref()
        .is_some_and(|(_, label)| *label != number.span)
    {
        return Err("claim_link_must_cover_exactly_the_percentage");
    }
    let [PeriodTok::YearRange { from, to }] = periods.as_slice() else {
        return Err("one_explicit_annual_window_required");
    };
    let geo = bases.iter().filter(|b| **b == BasisTok::Geo).count();
    let other_bases: Vec<_> = bases
        .iter()
        .filter(|b| **b != BasisTok::Geo)
        .copied()
        .collect();
    if other_bases != [BasisTok::Attributable] {
        return Err("explicit_attributable_ownership_required");
    }
    let equivalent = geo + phrase(&mut rest, "gold equivalent");
    let gold = phrase(&mut rest, "gold");
    let metal = match (gold, equivalent) {
        (1, 0) if geo_definition.is_none() => ReserveMetal::Gold,
        (0, 1) if geo_definition.is_some() => ReserveMetal::GoldEquivalent,
        _ => return Err("one_explicit_metal_and_applicable_geo_deck_required"),
    };
    if phrase(&mut rest, "company") != 1
        || phrase(&mut rest, "reserve replacement") != 1
        || phrase(&mut rest, "excluding acquisitions and divestments") != 1
    {
        return Err("company_replacement_and_explicit_acquisition_divestment_exclusion_required");
    }
    let words: Vec<_> = rest
        .split(|c: char| c.is_whitespace() || matches!(c, ',' | ':' | '.'))
        .filter(|w| !w.is_empty())
        .collect();
    if words
        .iter()
        .filter(|w| matches!(**w, "is" | "was" | "equals" | "="))
        .count()
        != 1
        || words.iter().any(|w| {
            !matches!(
                *w,
                "is" | "was"
                    | "equals"
                    | "="
                    | "for"
                    | "in"
                    | "of"
                    | "the"
                    | "on"
                    | "an"
                    | "a"
                    | "basis"
                    | "reported"
            )
        })
    {
        return Err("unsupported_reserve_replacement_prose");
    }
    let mut number = (*number).clone();
    number.span.0 += offset;
    number.span.1 += offset;
    Ok(Request {
        number,
        definition,
        geo_definition,
        metal,
        years: (*from, *to),
        citation: link.map(|(id, _)| id),
    })
}

fn decimal(d: Dec) -> Result<Rat, &'static str> {
    Rat::new(
        d.mantissa,
        10_i128
            .checked_pow(d.scale)
            .ok_or("draft_decimal_overflow")?,
    )
    .map_err(|_| "draft_decimal_overflow")
}

fn compare(request: &Request, bounds: Bounds) -> Result<&'static str, &'static str> {
    let value = decimal(request.number.value)?;
    let half = decimal(request.number.step)?
        .checked_div(Rat::from_integer(2))
        .map_err(|_| "draft_rounding_overflow")?;
    let lo = value
        .checked_sub(half)
        .map_err(|_| "draft_rounding_overflow")?;
    let hi = value
        .checked_add(half)
        .map_err(|_| "draft_rounding_overflow")?;
    let cmp = |a: Rat, b: Rat| a.checked_cmp(b).map_err(|_| "rounding_comparison_overflow");
    Ok(
        if cmp(lo, bounds.lo)?.is_le() && cmp(hi, bounds.hi)?.is_ge() {
            "verified"
        } else if cmp(hi, bounds.lo)?.is_lt() || cmp(lo, bounds.hi)?.is_gt() {
            "value_mismatch"
        } else {
            "rounding_not_established"
        },
    )
}

fn check(finding: &mut Finding, request: &Request, source: &reserve::Loaded, discover: bool) {
    let selections: Vec<_> = source
        .report
        .series
        .iter()
        .filter(|s| {
            let selection = match s {
                SeriesAssessment::Computed(s) => &s.selection,
                SeriesAssessment::Refused(s) => &s.selection,
            };
            if let Some(id) = &request.citation {
                source.claims.iter().any(|c| {
                    c.derived().claim_id.to_string() == *id
                        && c.derived().value.series_id == selection.id
                })
            } else {
                selection.definition == request.definition
                    && (selection.start_year, selection.end_year) == request.years
            }
        })
        .collect();
    finding.candidates = selections
        .iter()
        .map(|s| match s {
            SeriesAssessment::Computed(s) => s.selection.id.clone(),
            SeriesAssessment::Refused(s) => s.selection.id.clone(),
        })
        .collect();
    if selections.len() > 1 {
        finding.verdict = "ambiguous";
        finding.reason = "competing_bound_series_are_not_selected_by_value".into();
        return;
    }
    let [selected] = selections.as_slice() else {
        finding.verdict = "uncited";
        finding.reason =
            "no_bound_series_or_explicit_claim_matches_the_named_definition_and_window".into();
        return;
    };
    let SeriesAssessment::Computed(result) = selected else {
        finding.verdict = "refused";
        if let SeriesAssessment::Refused(r) = selected {
            finding.reason = r.reason.clone();
        }
        return;
    };
    if result.selection.definition != request.definition
        || result.rule.metal != request.metal
        || result.rule.geo_definition != request.geo_definition
        || !result.rule.excludes_acquisitions_divestments
    {
        finding.verdict = "basis_mismatch";
        finding.reason = "metal_definition_exclusions_or_geo_deck_differ".into();
        return;
    }
    if (result.selection.start_year, result.selection.end_year) != request.years {
        finding.verdict = "period_mismatch";
        finding.reason = "explicit_annual_window_differs_from_the_cited_claim".into();
        return;
    }
    let claims: Vec<_> = source
        .claims
        .iter()
        .filter(|c| c.derived().value.series_id == result.selection.id)
        .collect();
    let [claims] = claims.as_slice() else {
        finding.verdict = "refused";
        finding.reason = "canonical_source_claim_not_unique".into();
        return;
    };
    finding.claim = Some(claims.derived().claim_id.to_string());
    if request.citation.is_none() && !discover {
        finding.verdict = "uncited";
        finding.reason =
            "link_the_percentage_to_its_canonical_reserve_claim_or_select_discovery".into();
        return;
    }
    match compare(request, result.joint.percent) {
        Ok(verdict) => {
            finding.verdict = verdict;
            finding.reason = match verdict {
                "verified" => "exact_source_rounding_enclosure_fits_the_draft_precision",
                "value_mismatch" => "exact_source_rounding_enclosure_is_disjoint_from_the_draft",
                _ => "source_rounding_uncertainty_exceeds_the_draft_precision",
            }
            .into();
        }
        Err(reason) => {
            finding.verdict = "refused";
            finding.reason = reason.into();
        }
    }
}

pub(super) fn assess(
    draft: &str,
    profile: &IssuerProfile,
    bindings: &Path,
    as_of: &str,
    discover: bool,
    root: &Path,
) -> Report {
    let source = reserve::load(profile, &profile.issuer, bindings, Some(as_of), root);
    let mut findings = Vec::new();
    let mut offset = 0;
    for line in draft.split_inclusive('\n') {
        if line.to_ascii_lowercase().contains("reserve replacement") {
            let mut finding = Finding {
                span: (offset, offset + line.len()),
                raw: None,
                mode: if discover { "discovered" } else { "cited" },
                verdict: "unchecked",
                reason: String::new(),
                candidates: Vec::new(),
                claim: None,
            };
            match request(line, offset) {
                Err(reason) => finding.reason = reason.into(),
                Ok(request) => {
                    finding.span = request.number.span;
                    finding.raw = Some(request.number.raw.clone());
                    if request.citation.is_some() {
                        finding.mode = "cited";
                    }
                    match &source {
                        Ok(source) => check(&mut finding, &request, source, discover),
                        Err(refusal) => {
                            finding.verdict = "refused";
                            finding.reason = refusal.what.clone();
                        }
                    }
                }
            }
            findings.push(finding);
        }
        offset += line.len();
    }
    Report {
        as_of: as_of.into(),
        draft_digest: sha256(draft.as_bytes()).to_hex(),
        source,
        findings,
    }
}

impl Report {
    fn body(&self) -> String {
        let (source, refusal) = match &self.source {
            Ok(source) => (source.to_json(), "null".into()),
            Err(r) => (
                "null".into(),
                format!(
                    "{{\"code\":{},\"what\":{},\"fix\":{}}}",
                    js(r.code),
                    js(&r.what),
                    js(r.fix)
                ),
            ),
        };
        let findings = self.findings.iter().map(|f| format!(
            "{{\"span\":{{\"start_byte\":{},\"end_byte\":{}}},\"raw\":{},\"mode\":{},\"verdict\":{},\"reason\":{},\"candidate_series\":[{}],\"claim\":{}}}",
            f.span.0, f.span.1,
            f.raw.as_deref().map_or_else(|| "null".into(), js),
            js(f.mode), js(f.verdict), js(&f.reason),
            f.candidates.iter().map(|s| js(s)).collect::<Vec<_>>().join(","),
            f.claim.as_deref().map_or_else(|| "null".into(), js)
        )).collect::<Vec<_>>().join(",");
        format!(
            "\"schema\":\"fmine.reserve-replacement.lint.v1\",\"as_of\":{},\"draft_sha256\":{},\"no_claim\":{},\"findings\":[{findings}],\"source_report\":{source},\"source_refusal\":{refusal}",
            js(&self.as_of),
            js(&self.draft_digest),
            js(NO_CLAIM)
        )
    }

    pub(super) fn to_json(&self) -> String {
        let body = self.body();
        format!(
            "{{\"digest\":{},{body}}}",
            js(&sha256(body.as_bytes()).to_hex())
        )
    }

    pub(super) fn to_markdown(&self) -> String {
        let mut out = format!(
            "# Reserve-replacement disclosure lint\n\n{NO_CLAIM}\n\nEvidence cut: {}. Report digest: `{}`.\n\n",
            self.as_of,
            sha256(self.body().as_bytes()).to_hex()
        );
        for f in &self.findings {
            let _ = writeln!(
                out,
                "- Bytes {}–{}, {}: **{}**; {}. Claim: `{}`.",
                f.span.0,
                f.span.1,
                f.mode,
                f.verdict,
                crate::guidance::text(&f.reason),
                f.claim.as_deref().unwrap_or("none")
            );
        }
        match &self.source {
            Ok(source) => out.push_str(&source.to_markdown(Some(&self.as_of))),
            Err(r) => {
                let _ = writeln!(
                    out,
                    "\nSource refusal {}: {}.\n",
                    r.code,
                    crate::guidance::text(&r.what)
                );
            }
        }
        out
    }
}
