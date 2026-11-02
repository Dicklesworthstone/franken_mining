//! DESIGN (BD-345): bounded cited equalities over canonical source Claims. Verification means that the
//! displayed source interval fits the draft's precision and its named semantic keys agree. It establishes
//! neither the truth of an issuer assertion nor the meaning of arbitrary prose. No draft leaves this API.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use fmine_core::hash::{Digest32, blake3_hash, sha256};
use fmine_io::numtok::equality::{self, Context, Metric};
use fmine_io::numtok::{
    BasisTok, Currency, Dec, NumberToken, PeriodTok, Span, Token, UnitTok, tokenize,
};
use fmine_numerics::exact_feas::Rat;

use crate::js;
use crate::key::{
    KeyedCell, OwnershipBasis, Period, PeriodKind, PublicValue, Scope, SeriesKey,
    SeriesOwnershipBasis, canonical_value_tag, claims_digest,
};

pub const RULES_VERSION: &str = "cited-equality/5";
pub const NO_CLAIM: &str = "Checks supported numeric equalities against cited public Claims in the selected source snapshot. It does not establish the truth of issuer assertions, verify other prose, assess publicness or authorize disclosure.";
pub const DISCOVERY_NO_CLAIM: &str = "Matches supported numeric equalities to public Claims in the selected source snapshot only when their stated semantic keys identify exactly one candidate. It does not establish the truth of issuer assertions, verify other prose, assess publicness or authorize disclosure. General-prose discovery precision has not been measured.";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LintMode {
    Cited,
    Discovered,
}

impl LintMode {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Cited => "cited",
            Self::Discovered => "discovered",
        }
    }

    #[must_use]
    pub const fn no_claim(self) -> &'static str {
        match self {
            Self::Cited => NO_CLAIM,
            Self::Discovered => DISCOVERY_NO_CLAIM,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Verdict {
    Verified,
    Uncited,
    Ambiguous,
    Stale,
    PeriodMismatch,
    BasisMismatch,
    ValueMismatch,
    RoundingBeyondTolerance,
    UncalibratedProbability,
    Unchecked,
}

impl Verdict {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Verified => "verified",
            Self::Uncited => "uncited",
            Self::Ambiguous => "ambiguous",
            Self::Stale => "stale",
            Self::PeriodMismatch => "period_mismatch",
            Self::BasisMismatch => "basis_mismatch",
            Self::ValueMismatch => "value_mismatch",
            Self::RoundingBeyondTolerance => "rounding_beyond_tolerance",
            Self::UncalibratedProbability => "uncalibrated_probability",
            Self::Unchecked => "unchecked",
        }
    }

    const fn fix(self) -> &'static str {
        match self {
            Self::Verified => {
                "Retain the Claim citation and the stated precision and semantic keys."
            }
            Self::Uncited => {
                "Link the printed amount as [amount](fmine:clm_...) to a canonical Claim."
            }
            Self::Ambiguous => {
                "Provide one subject, metric, dated period, ownership basis and unit, and an unambiguous Claim link."
            }
            Self::Stale => {
                "Inspect the later Claims in this snapshot and cite the intended current assertion."
            }
            Self::PeriodMismatch => {
                "Use the cited Claim's valid period or select the intended period's Claim."
            }
            Self::BasisMismatch => {
                "Correct the subject, metric, ownership, denominator or unit; no basis or unit is converted."
            }
            Self::ValueMismatch => {
                "Check the draft amount against the displayed source value and its Claim link."
            }
            Self::RoundingBeyondTolerance => {
                "Do not claim precision narrower than the cited source's displayed interval."
            }
            Self::UncalibratedProbability => {
                "Supply a separately assessed probability and calibration record; source-value Claims do not provide one."
            }
            Self::Unchecked => {
                "Use a supported standalone cited equality or arrange a separate review of this prose."
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct LintFinding {
    pub mode: LintMode,
    pub span: Span,
    pub raw: String,
    pub step: Option<String>,
    pub claim_id: Option<String>,
    pub verdict: Verdict,
    pub also: Vec<Verdict>,
    pub reason: &'static str,
    /// The exact canonical source assertion, not a Claim manufactured from the draft.
    pub source: Option<KeyedCell>,
    pub later: Vec<KeyedCell>,
    /// All semantically matching candidates when discovery cannot choose uniquely. Numeric agreement
    /// never removes a competing candidate, and older registered vintages are not silently discarded.
    pub candidates: Vec<KeyedCell>,
}

impl LintFinding {
    fn to_json(&self) -> String {
        format!(
            "{{\"mode\":{},\"span\":{},\"raw\":{},\"step\":{},\"claim\":{},\"verdict\":{},\"also\":[{}],\"reason\":{},\"fix\":{},\"source_claim\":{},\"later_claims\":[{}],\"candidate_claims\":[{}]}}",
            js(self.mode.as_str()),
            span_json(self.span),
            js(&self.raw),
            self.step.as_deref().map_or_else(|| "null".into(), js),
            self.claim_id.as_deref().map_or_else(|| "null".into(), js),
            js(self.verdict.as_str()),
            self.also
                .iter()
                .map(|v| js(v.as_str()))
                .collect::<Vec<_>>()
                .join(","),
            js(self.reason),
            js(self.verdict.fix()),
            self.source
                .as_ref()
                .map_or_else(|| "null".into(), KeyedCell::to_json),
            self.later
                .iter()
                .map(KeyedCell::to_json)
                .collect::<Vec<_>>()
                .join(","),
            self.candidates
                .iter()
                .map(KeyedCell::to_json)
                .collect::<Vec<_>>()
                .join(","),
        )
    }
}

#[derive(Debug, Clone)]
pub struct LintReport {
    pub mode: LintMode,
    pub draft_digest: Digest32,
    pub claims_digest: Digest32,
    pub findings: Vec<LintFinding>,
    /// Unsupported nonempty sentences remain visible even if they contain no numeric assertion.
    pub unchecked_spans: Vec<Span>,
}

fn span_json((start, end): Span) -> String {
    format!("{{\"start_byte\":\"{start}\",\"end_byte\":\"{end}\"}}")
}

impl LintReport {
    fn body_json(&self) -> String {
        let mut counts = BTreeMap::new();
        for finding in &self.findings {
            *counts.entry(finding.verdict).or_insert(0_usize) += 1;
        }
        format!(
            "\"schema\":\"fmine.disclosure_lint.v1\",\"rules_version\":{},\"mode\":{},\"draft_sha256\":{},\"claims_digest\":{},\"publicness\":\"not_assessed\",\"staleness_rule\":\"later release of the same canonical series in the selected snapshot, or an explicit stale/superseded Claim\",\"interpretation_limit\":{},\"numbers\":\"{}\",\"by_verdict\":{{{}}},\"unchecked_spans\":[{}],\"findings\":[{}]",
            js(RULES_VERSION),
            js(self.mode.as_str()),
            js(&self.draft_digest.to_hex()),
            js(&self.claims_digest.to_hex()),
            js(self.mode.no_claim()),
            self.findings.len(),
            counts
                .iter()
                .map(|(v, n)| format!("{}:\"{n}\"", js(v.as_str())))
                .collect::<Vec<_>>()
                .join(","),
            self.unchecked_spans
                .iter()
                .copied()
                .map(span_json)
                .collect::<Vec<_>>()
                .join(","),
            self.findings
                .iter()
                .map(LintFinding::to_json)
                .collect::<Vec<_>>()
                .join(","),
        )
    }

    #[must_use]
    pub fn digest(&self) -> Digest32 {
        blake3_hash(self.body_json().as_bytes())
    }

    #[must_use]
    pub fn to_json(&self) -> String {
        let body = self.body_json();
        format!(
            "{{\"report_digest\":{},{} }}",
            js(&blake3_hash(body.as_bytes()).to_hex()),
            body
        )
    }

    /// Findings only: no sentence, paragraph or full draft is copied into this view.
    #[must_use]
    pub fn to_markdown(&self) -> String {
        let mut out = format!(
            "# {} disclosure lint\n\n{}\n\nReport: `{}`\n\n| Bytes | Mode | Amount | Verdict | Source |\n| --- | --- | --- | --- | --- |\n",
            match self.mode {
                LintMode::Cited => "Cited",
                LintMode::Discovered => "Discovered",
            },
            self.mode.no_claim(),
            self.digest().to_hex()
        );
        let escape = |s: &str| {
            s.replace('&', "&amp;")
                .replace('<', "&lt;")
                .replace('>', "&gt;")
                .replace('|', "&#124;")
                .replace('`', "&#96;")
                .replace(['\n', '\r'], " ")
        };
        for f in &self.findings {
            let _ = writeln!(
                out,
                "| {}..{} | {} | {} | {} | {} |",
                f.span.0,
                f.span.1,
                f.mode.as_str(),
                escape(&f.raw),
                f.verdict.as_str(),
                f.source.as_ref().map_or_else(
                    || if f.candidates.is_empty() {
                        "unresolved".into()
                    } else {
                        format!("{} candidates; see guidance", f.candidates.len())
                    },
                    |c| format!("{} ({})", escape(&c.locator), c.claim.claim_id)
                )
            );
        }
        out.push_str("\n## Finding guidance\n\n");
        for f in &self.findings {
            let _ = writeln!(
                out,
                "- {}..{} ({}; {}): {}",
                f.span.0,
                f.span.1,
                f.mode.as_str(),
                f.reason,
                f.verdict.fix()
            );
            if !f.candidates.is_empty() {
                out.push('\n');
                for candidate in &f.candidates {
                    let _ = writeln!(
                        out,
                        "  - Candidate `{}`: {}.",
                        candidate.claim.claim_id,
                        escape(&candidate.locator)
                    );
                }
                out.push('\n');
            }
        }
        let _ = writeln!(
            out,
            "\nUnchecked prose spans: {}. Equality checks alone do not establish publication availability or disclosure permission.\n",
            self.unchecked_spans.len()
        );
        out
    }
}

fn metric(c: &KeyedCell) -> Option<Metric> {
    Some(match c.key.line.label.as_str() {
        "summary:aisc_usd_per_oz"
        | "html:aisc_usd_per_oz_sold"
        | "all-in sustaining costs per ounce"
        | "site:costs per ounce:all-in sustaining costs"
        | "site:costs per ounce ($/oz):all-in sustaining costs"
        | "summary:aisc_usd_per_lb"
        | "site:costs per pound:all-in sustaining costs"
        | "site:costs per pound ($/lb):all-in sustaining costs" => Metric::Aisc,
        "summary:tcc_usd_per_oz"
        | "html:tcc_usd_per_oz_sold"
        | "total cash costs per ounce"
        | "total cash costs per ounce (on a co-product basis)"
        | "site:costs per ounce:total cash costs"
        | "site:costs per ounce ($/oz):total cash costs" => Metric::Tcc,
        "summary:c1_usd_per_lb"
        | "site:costs per pound:c1 cash costs"
        | "site:costs per pound ($/lb):c1 cash costs" => Metric::C1,
        "summary:cos_usd_per_oz"
        | "cost of sales per ounce"
        | "site:costs per ounce:cost of sales"
        | "site:costs per ounce ($/oz):cost of sales"
        | "summary:cos_usd_per_lb"
        | "site:costs per pound:cost of sales"
        | "site:costs per pound ($/lb):cost of sales" => Metric::CostOfSales,
        "summary:au_prod_100_koz" | "summary:au_prod_attr_koz" => Metric::Production,
        "summary:cu_prod_100_kt"
        | "summary:cu_prod_attr_kt"
        | "site::production (thousands of tonnes)" => Metric::CopperProduction,
        "summary:au_sold_attr_koz" | "ounces sold" => Metric::Sales,
        "site:average grade (grams per tonne):processed"
        | "html:processed_grade_g_per_t"
        | "html:processed_grade_percent"
        | "site:average grade (percent):processed"
        | "site:average grade:processed" => Metric::ProcessedGrade,
        "site::recovery rate (percent)" | "html:recovery_percent" => Metric::Recovery,
        _ => return None,
    })
}

type AttributionKey = (String, String, Period, Scope, String, Metric, bool);

fn attribution_key(c: &KeyedCell) -> Option<AttributionKey> {
    Some((
        c.key.issuer.clone(),
        c.key.subject.clone(),
        c.key.valid_period,
        c.key.scope.clone(),
        c.key.unit.clone(),
        metric(c)?,
        c.key.line.label == "total cash costs per ounce (on a co-product basis)",
    ))
}

struct Index<'a> {
    ids: BTreeMap<String, Vec<&'a KeyedCell>>,
    latest: BTreeMap<SeriesKey, Vec<&'a KeyedCell>>,
    shares: BTreeMap<AttributionKey, BTreeSet<(i64, i64)>>,
    superseded: BTreeSet<String>,
    subjects: Vec<String>,
    discovery: BTreeMap<(String, Period, Metric), Vec<&'a KeyedCell>>,
}

impl<'a> Index<'a> {
    fn new(claims: &'a [KeyedCell]) -> Self {
        let mut index = Self {
            ids: BTreeMap::new(),
            latest: BTreeMap::new(),
            shares: BTreeMap::new(),
            superseded: BTreeSet::new(),
            subjects: Vec::new(),
            discovery: BTreeMap::new(),
        };
        let mut subjects = BTreeSet::new();
        for c in claims {
            index
                .ids
                .entry(c.claim.claim_id.to_string())
                .or_default()
                .push(c);
            let latest = index.latest.entry(c.key.series_key()).or_default();
            if latest
                .first()
                .is_some_and(|a| a.key.reporting_vintage.release < c.key.reporting_vintage.release)
            {
                latest.clear();
            }
            if latest
                .first()
                .is_none_or(|a| a.key.reporting_vintage.release == c.key.reporting_vintage.release)
            {
                latest.push(c);
            }
            if let Some(id) = c.claim.supersedes {
                index.superseded.insert(id.to_string());
            }
            if let OwnershipBasis::Attributable { num, den, .. } = c.key.ownership_basis
                && let Some(key) = attribution_key(c)
            {
                index.shares.entry(key).or_default().insert((num, den));
            }
            subjects.insert(c.key.subject.to_ascii_lowercase());
            if let Some(m) = metric(c) {
                index
                    .discovery
                    .entry((c.key.subject.to_ascii_lowercase(), c.key.valid_period, m))
                    .or_default()
                    .push(c);
            }
        }
        index.subjects = subjects.into_iter().collect();
        index
            .subjects
            .sort_by(|a, b| b.len().cmp(&a.len()).then(a.cmp(b)));
        for cells in index
            .ids
            .values_mut()
            .chain(index.latest.values_mut())
            .chain(index.discovery.values_mut())
        {
            cells.sort_by_cached_key(|c| c.to_json());
        }
        index
    }
}

#[derive(Debug)]
struct Link {
    span: Span,
    label: Span,
    id: String,
}

fn links(text: &str) -> Vec<Link> {
    let mut result = Vec::new();
    let mut at = 0;
    while let Some(i) = text[at..].find('[') {
        let start = at + i;
        let Some(end_label) = text[start + 1..].find("](").map(|i| start + 1 + i) else {
            break;
        };
        let target_start = end_label + 2;
        let Some(end) = text[target_start..].find(')').map(|i| target_start + i) else {
            break;
        };
        if let Some(id) = text[target_start..end].strip_prefix("fmine:")
            && id.len() == 36
            && id.starts_with("clm_")
            && id[4..].bytes().all(|b| b.is_ascii_hexdigit())
        {
            result.push(Link {
                span: (start, end + 1),
                label: (start + 1, end_label),
                id: id.to_string(),
            });
        }
        at = end + 1;
    }
    result
}

/// Sentences end at a line break or terminal punctuation outside a numeric decimal or Markdown link.
fn sentences(text: &str) -> Vec<Span> {
    let (mut start, mut depth, mut result) = (0, 0_i32, Vec::new());
    for (i, c) in text.char_indices() {
        if matches!(c, '[' | '(') {
            depth += 1;
        }
        if matches!(c, ']' | ')') {
            depth = (depth - 1).max(0);
        }
        let decimal = c == '.'
            && text
                .as_bytes()
                .get(i.wrapping_sub(1))
                .is_some_and(u8::is_ascii_digit)
            && text.as_bytes().get(i + 1).is_some_and(u8::is_ascii_digit);
        if c == '\n' || (depth == 0 && matches!(c, '.' | ';' | '?' | '!') && !decimal) {
            result.push((start, i + c.len_utf8()));
            start = i + c.len_utf8();
        }
    }
    if start < text.len() {
        result.push((start, text.len()));
    }
    result
}

fn context(text: &str, tokens: &[Token], links: &[Link], index: &Index<'_>) -> Context {
    equality::context(
        text,
        tokens,
        &links.iter().map(|l| l.span).collect::<Vec<_>>(),
        &index.subjects,
    )
}

fn period(t: PeriodTok) -> Option<Period> {
    Some(match t {
        PeriodTok::Quarter {
            q,
            year: Some(year),
        } => Period {
            year,
            kind: PeriodKind::Quarter(q),
        },
        PeriodTok::Half {
            h: 1,
            year: Some(year),
        } => Period {
            year,
            kind: PeriodKind::H1,
        },
        PeriodTok::Fy { year } => Period {
            year,
            kind: PeriodKind::FullYear,
        },
        _ => return None,
    })
}

fn unit_matches(unit: UnitTok, key: &str, metric: Metric) -> bool {
    matches!(
        (metric, unit, key),
        (
            Metric::Aisc | Metric::Tcc | Metric::CostOfSales,
            UnitTok::UsdPerOz | UnitTok::DollarPerOz,
            "usd_per_oz"
        ) | (Metric::Production | Metric::Sales, UnitTok::Koz, "koz")
            | (
                Metric::Aisc | Metric::C1 | Metric::CostOfSales,
                UnitTok::UsdPerLb | UnitTok::DollarPerLb,
                "usd_per_lb"
            )
            | (Metric::CopperProduction, UnitTok::Kt, "kt")
            | (Metric::ProcessedGrade, UnitTok::GramsPerTonne, "g_per_t")
            | (
                Metric::ProcessedGrade | Metric::Recovery,
                UnitTok::Percent,
                "percent" | "wt_pct" | "mass_fraction"
            )
    )
}

fn decimal(d: Dec) -> Option<Rat> {
    Rat::new(d.mantissa, 10_i128.checked_pow(d.scale)?).ok()
}

fn number_box(value: Rat, step: Rat) -> Option<(Rat, Rat)> {
    if step.signum() <= 0 {
        return None;
    }
    let half = step.checked_div(Rat::from_integer(2)).ok()?;
    Some((value.checked_sub(half).ok()?, value.checked_add(half).ok()?))
}

fn source_box(c: &KeyedCell) -> Option<(Rat, Rat)> {
    let PublicValue::Number {
        displayed: Ok(text),
        under_status: None,
        ..
    } = &c.claim.value
    else {
        return None;
    };
    let tokens = tokenize(text);
    let [Token::Number(n)] = tokens.as_slice() else {
        return None;
    };
    let physical = matches!(metric(c), Some(Metric::ProcessedGrade | Metric::Recovery));
    // Percent-format workbooks store fractions but print percentage points. Compare that explicit
    // presentation, preserving both stored value and source Claim; an unformatted fraction is unusable.
    let source_unit = match (metric(c), c.key.unit.as_str()) {
        (Some(Metric::ProcessedGrade), "g_per_t") => n.unit.is_none(),
        (Some(Metric::ProcessedGrade | Metric::Recovery), "percent" | "wt_pct") => {
            matches!(n.unit, None | Some(UnitTok::Percent))
        }
        (Some(Metric::ProcessedGrade | Metric::Recovery), "mass_fraction") => n.percent,
        (Some(Metric::ProcessedGrade | Metric::Recovery), _) => false,
        _ => n.unit.is_none(),
    };
    if text.trim() != n.raw.trim()
        || n.range_with.is_some()
        || !source_unit
        || (physical && (n.currency.is_some() || n.scale_word.is_some() || n.hedge.is_some()))
    {
        return None;
    }
    number_box(decimal(n.value)?, decimal(n.step)?)
}

fn interval_verdict(n: &NumberToken, c: &KeyedCell) -> Option<Verdict> {
    let (lo, hi) = number_box(decimal(n.value)?, decimal(n.step)?)?;
    let (source_lo, source_hi) = source_box(c)?;
    if lo.checked_cmp(source_lo).ok()?.is_le() && hi.checked_cmp(source_hi).ok()?.is_ge() {
        Some(Verdict::Verified)
    } else if hi.checked_cmp(source_lo).ok()?.is_lt() || lo.checked_cmp(source_hi).ok()?.is_gt() {
        Some(Verdict::ValueMismatch)
    } else {
        Some(Verdict::RoundingBeyondTolerance)
    }
}

fn missing_basis(c: &Context, metric: Metric) -> Option<&'static str> {
    if c.bases.iter().filter(|b| b.is_ownership()).count() != 1 {
        Some("ownership_required")
    } else if metric.rate() && !c.bases.iter().any(|b| b.is_denominator()) {
        Some("sold_denominator_required")
    } else if metric == Metric::Tcc && !c.bases.iter().any(|b| b.is_credit()) {
        Some("credit_basis_required")
    } else {
        None
    }
}

fn basis_findings(c: &Context, claim: &KeyedCell, metric: Metric) -> Vec<(Verdict, &'static str)> {
    use BasisTok as B;
    let mut failures = Vec::new();
    if let Some(reason) = missing_basis(c, metric) {
        failures.push((Verdict::Ambiguous, reason));
    }
    let ownership: Vec<_> = c.bases.iter().filter(|b| b.is_ownership()).collect();
    if ownership.len() == 1
        && !matches!(
            (ownership[0], claim.key.ownership_basis),
            (B::Attributable, OwnershipBasis::Attributable { .. })
                | (B::Hundred, OwnershipBasis::Consolidated100)
                | (B::Unprinted, OwnershipBasis::Unprinted)
        )
    {
        failures.push((Verdict::BasisMismatch, "ownership"));
    }
    let sold = if claim.key.unit == "usd_per_lb" {
        B::PerLbSold
    } else {
        B::PerOzSold
    };
    if c.bases
        .iter()
        .any(|b| b.is_denominator() && (!metric.rate() || *b != sold))
    {
        failures.push((Verdict::BasisMismatch, "sold_produced_denominator"));
    }
    let co_product = claim.key.line.label == "total cash costs per ounce (on a co-product basis)";
    // HTML labels establish the printed rate, not the issuer's by-/co-product definition.
    let unstated_credit = matches!(
        claim.key.line.label.as_str(),
        "html:aisc_usd_per_oz_sold" | "html:tcc_usd_per_oz_sold"
    );
    if unstated_credit && c.bases.iter().any(|basis| basis.is_credit()) {
        failures.push((Verdict::Unchecked, "source_credit_basis_unstated"));
    }
    if c.bases.iter().any(|b| match b {
        B::CoProduct => !unstated_credit && !co_product,
        B::ByProduct => {
            !unstated_credit && (co_product || !matches!(metric, Metric::Aisc | Metric::Tcc))
        }
        B::Geo | B::Scorecard => true,
        B::NonGaap => !matches!(metric, Metric::Aisc | Metric::Tcc | Metric::C1),
        B::GoldOunces => claim.key.unit == "usd_per_lb" || metric == Metric::CopperProduction,
        _ => false,
    }) {
        failures.push((Verdict::BasisMismatch, "metric_definition"));
    }
    if claim.basis_evidence.is_some()
        && claim.printed_basis != OwnershipBasis::Unprinted
        && SeriesOwnershipBasis::from(claim.printed_basis)
            != SeriesOwnershipBasis::from(claim.key.ownership_basis)
    {
        failures.push((Verdict::Ambiguous, "registered_printed_basis_conflict"));
    }
    failures
}

/// Parsed equality semantics only; the prototype supplies no evidence for the target period or value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EqualityTarget {
    pub period: Period,
    pub value: Dec,
    pub step: Dec,
}

/// Parses one bounded equality at the exact amount span, retaining the existing metric/basis grammar.
/// The caller must independently admit source evidence and resolve competing ownership shares.
#[must_use]
pub fn equality_target(draft: &str, span: Span, prototype: &KeyedCell) -> Option<EqualityTarget> {
    draft.get(span.0..span.1)?;
    let sentence = sentences(draft)
        .into_iter()
        .find(|s| s.0 <= span.0 && span.1 <= s.1)?;
    let text = draft.get(sentence.0..sentence.1)?;
    let tokens = tokenize(text);
    let amount = tokens.iter().find_map(|token| match token {
        Token::Number(n) if (sentence.0 + n.span.0, sentence.0 + n.span.1) == span => Some(n),
        _ => None,
    })?;
    let links = links(text);
    if amount.range_with.is_some()
        || links.len() > 1
        || (text.contains("fmine:") && links.len() != 1)
        || links.iter().any(|link| {
            link.label != amount.span
                || text[link.label.0..link.label.1].trim() != amount.raw.trim()
        })
    {
        return None;
    }
    let index = Index::new(std::slice::from_ref(prototype));
    let ctx = context(text, &tokens, &links, &index);
    let metric = metric(prototype)?;
    if !ctx.supported
        || ctx.numbers != 1
        || ctx.subjects.as_slice() != [prototype.key.subject.to_ascii_lowercase()]
        || ctx.metrics.as_slice() != [metric]
        || ctx.periods.len() != 1
        || ctx.units.is_empty()
        || !ctx
            .units
            .iter()
            .all(|u| unit_matches(*u, &prototype.key.unit, metric))
        || amount
            .currency
            .is_some_and(|c| !matches!(c, Currency::Usd | Currency::Dollar) || !metric.rate())
        || !basis_findings(&ctx, prototype, metric).is_empty()
    {
        return None;
    }
    Some(EqualityTarget {
        period: period(ctx.periods[0])?,
        value: amount.value,
        step: amount.step,
    })
}

fn finding(
    n: &NumberToken,
    offset: usize,
    text: &str,
    tokens: &[Token],
    link_set: &[Link],
    index: &Index<'_>,
) -> LintFinding {
    let mut f = LintFinding {
        mode: LintMode::Cited,
        span: (offset + n.span.0, offset + n.span.1),
        raw: n.raw.clone(),
        step: decimal(n.step).map(|q| {
            if q.den() == 1 {
                q.num().to_string()
            } else {
                format!("{}/{}", q.num(), q.den())
            }
        }),
        claim_id: None,
        verdict: Verdict::Unchecked,
        also: Vec::new(),
        reason: "unsupported_predicate",
        source: None,
        later: Vec::new(),
        candidates: Vec::new(),
    };
    let links: Vec<_> = link_set
        .iter()
        .filter(|l| l.label.0 <= n.span.0 && n.span.1 <= l.label.1)
        .collect();
    if links.len() != 1 {
        f.verdict = Verdict::Uncited;
        f.reason = "claim_link_required";
        return f;
    }
    let link = links[0];
    f.claim_id = Some(link.id.clone());
    let Some(candidates) = index.ids.get(&link.id).filter(|c| c.len() == 1) else {
        f.verdict = Verdict::Ambiguous;
        f.reason = "claim_not_unique_in_snapshot";
        return f;
    };
    let claim = candidates[0];
    f.source = Some(claim.clone());
    if claim.claim.check_id().is_err()
        || claim.claim.metric_key != claim.key.line.label
        || claim.claim.value_tag() != canonical_value_tag(&claim.key, &claim.claim.value)
    {
        f.reason = "source_claim_identity_mismatch";
        return f;
    }
    if link_set.len() != 1
        || text[link.label.0..link.label.1].trim() != n.raw.trim()
        || n.range_with.is_some()
    {
        f.reason = "unsupported_link_label_or_range";
        return f;
    }
    let ctx = context(text, tokens, link_set, index);
    if !ctx.supported || ctx.numbers != 1 {
        let lower = text.to_ascii_lowercase();
        if ["probability", "chance", "confidence", "likelihood"]
            .iter()
            .any(|w| lower.split(|c: char| !c.is_alphabetic()).any(|s| s == *w))
        {
            f.verdict = Verdict::UncalibratedProbability;
            f.reason = "probability_not_assessed_by_source_claim";
            f.also.push(Verdict::Unchecked);
        }
        return f;
    }
    let Some(m) = metric(claim) else {
        f.reason = "unsupported_canonical_metric";
        return f;
    };
    let mut failures = Vec::new();
    if ctx.subjects.len() != 1 || ctx.metrics.len() != 1 || ctx.periods.len() != 1 {
        failures.push((Verdict::Ambiguous, "complete_equality_context_required"));
    } else {
        if ctx.subjects[0] != claim.key.subject.to_ascii_lowercase() || ctx.metrics[0] != m {
            failures.push((Verdict::BasisMismatch, "subject_or_metric"));
        }
        match period(ctx.periods[0]) {
            Some(p) if p != claim.key.valid_period => {
                failures.push((Verdict::PeriodMismatch, "valid_period"))
            }
            None => failures.push((Verdict::Ambiguous, "dated_supported_period_required")),
            _ => {}
        }
    }
    failures.extend(basis_findings(&ctx, claim, m));
    if matches!(
        claim.key.ownership_basis,
        OwnershipBasis::Attributable { .. }
    ) && let Some(key) = attribution_key(claim)
        && index
            .shares
            .get(&key)
            .is_some_and(|shares| shares.len() > 1)
    {
        failures.push((Verdict::Ambiguous, "attributable_share_unspecified"));
    }
    if ctx.units.is_empty() {
        failures.push((Verdict::Ambiguous, "explicit_unit_required"));
    } else if !ctx
        .units
        .iter()
        .all(|u| unit_matches(*u, &claim.key.unit, m))
        || n.currency
            .is_some_and(|c| !matches!(c, Currency::Usd | Currency::Dollar) || !m.rate())
    {
        failures.push((Verdict::BasisMismatch, "unit_or_currency"));
    }
    if let Some(latest) = index.latest.get(&claim.key.series_key()) {
        f.later = latest
            .iter()
            .filter(|c| c.key.reporting_vintage.release > claim.key.reporting_vintage.release)
            .map(|c| (*c).clone())
            .collect();
        if latest.len() > 1 {
            failures.push((Verdict::Ambiguous, "concurrent_series_assertions"));
        }
    }
    if claim.claim.stale || index.superseded.contains(&link.id) || !f.later.is_empty() {
        failures.push((Verdict::Stale, "superseded_in_selected_snapshot"));
    }
    match interval_verdict(n, claim) {
        Some(Verdict::Verified) => {}
        Some(v) => failures.push((v, "displayed_interval")),
        None => failures.push((Verdict::Unchecked, "source_value_or_precision_unavailable")),
    }
    failures.sort_by_key(|(v, _)| *v);
    failures.dedup_by_key(|(v, _)| *v);
    if let Some(&(v, reason)) = failures.first() {
        f.verdict = v;
        f.reason = reason;
        f.also = failures.iter().skip(1).map(|(v, _)| *v).collect();
    } else {
        f.verdict = Verdict::Verified;
        f.reason = "displayed_interval_and_keys_match";
    }
    f
}

fn discover(
    n: &NumberToken,
    offset: usize,
    text: &str,
    tokens: &[Token],
    index: &Index<'_>,
) -> LintFinding {
    let mut f = finding(n, offset, text, tokens, &[], index);
    f.mode = LintMode::Discovered;
    f.verdict = Verdict::Unchecked;
    f.reason = "unsupported_discovery_predicate";
    let ctx = context(text, tokens, &[], index);
    if !ctx.supported
        || n.range_with.is_some()
        || !matches!(
            ctx.metrics.as_slice(),
            [Metric::Aisc
                | Metric::Tcc
                | Metric::C1
                | Metric::Production
                | Metric::CopperProduction
                | Metric::ProcessedGrade
                | Metric::Recovery]
        )
        || ctx.subjects.is_empty()
        || ctx.numbers != 1
    {
        return f;
    }
    if ctx.subjects.len() != 1 || ctx.periods.len() != 1 || ctx.units.is_empty() {
        f.verdict = Verdict::Ambiguous;
        f.reason = "complete_equality_context_required";
        return f;
    }
    let Some(period) = period(ctx.periods[0]) else {
        f.verdict = Verdict::Ambiguous;
        f.reason = "dated_supported_period_required";
        return f;
    };
    let m = ctx.metrics[0];
    if missing_basis(&ctx, m).is_some() {
        f.verdict = Verdict::Ambiguous;
        f.reason = "explicit_discovery_basis_required";
        return f;
    }
    // DESIGN (BD-345.S3): candidate selection uses semantic keys alone. Even a stale or nonnumeric
    // candidate remains visible, so its absence cannot turn another source into a unique match.
    let candidates: Vec<_> = index
        .discovery
        .get(&(ctx.subjects[0].clone(), period, m))
        .into_iter()
        .flatten()
        .copied()
        .filter(|c| {
            ctx.units.iter().all(|u| unit_matches(*u, &c.key.unit, m))
                && !n.currency.is_some_and(|currency| {
                    !matches!(currency, Currency::Usd | Currency::Dollar) || !m.rate()
                })
                && basis_findings(&ctx, c, m).iter().all(|(_, reason)| {
                    matches!(
                        *reason,
                        "registered_printed_basis_conflict" | "source_credit_basis_unstated"
                    )
                })
        })
        .collect();
    match candidates.as_slice() {
        [] => {
            f.verdict = Verdict::Uncited;
            f.reason = "no_claim_matching_stated_keys";
        }
        [claim] => {
            // Reuse every cited-mode check without changing the user's draft or manufacturing a Claim.
            let link = Link {
                span: n.span,
                label: n.span,
                id: claim.claim.claim_id.to_string(),
            };
            f = finding(n, offset, text, tokens, &[link], index);
            f.mode = LintMode::Discovered;
        }
        _ => {
            f.verdict = Verdict::Ambiguous;
            f.reason = "multiple_claims_matching_stated_keys";
            f.candidates = candidates.into_iter().cloned().collect();
        }
    }
    f
}

/// Checks one supported equality per sentence/line. The caller supplies a canonical, source-verified
/// snapshot; no publication timestamp, ownership share, metric definition or calibration is invented.
#[must_use]
pub fn lint_cited(draft: &str, claims: &[KeyedCell]) -> LintReport {
    lint(draft, claims, LintMode::Cited)
}

/// Discovers AISC, TCC, copper C1, gold/copper production, grade and recovery using the explicit
/// equality grammar of [`lint_cited`]. Other discovery is unchecked. All matching assertions remain
/// candidates, including repeated vintages; numeric agreement never breaks a tie. Explicit citations
/// retain cited-mode checks and are never silently repaired.
#[must_use]
pub fn lint_discover(draft: &str, claims: &[KeyedCell]) -> LintReport {
    lint(draft, claims, LintMode::Discovered)
}

fn lint(draft: &str, claims: &[KeyedCell], mode: LintMode) -> LintReport {
    let index = Index::new(claims);
    let mut sorted: Vec<_> = claims.iter().collect();
    sorted.sort_by_cached_key(|c| c.to_json());
    let mut report = LintReport {
        mode,
        draft_digest: sha256(draft.as_bytes()),
        claims_digest: claims_digest(sorted),
        findings: Vec::new(),
        unchecked_spans: Vec::new(),
    };
    for span in sentences(draft) {
        let text = &draft[span.0..span.1];
        if text.trim().is_empty() {
            continue;
        }
        let tokens = tokenize(text);
        let links = links(text);
        let before = report.findings.len();
        for token in &tokens {
            match token {
                Token::Number(n) => report.findings.push(
                    if mode == LintMode::Discovered && links.is_empty() && !text.contains("fmine:")
                    {
                        discover(n, span.0, text, &tokens, &index)
                    } else {
                        finding(n, span.0, text, &tokens, &links, &index)
                    },
                ),
                Token::UnsupportedNumber {
                    span: s,
                    raw,
                    reason,
                } => report.findings.push(LintFinding {
                    mode,
                    span: (span.0 + s.0, span.0 + s.1),
                    raw: raw.clone(),
                    step: None,
                    claim_id: None,
                    verdict: Verdict::Unchecked,
                    also: Vec::new(),
                    reason,
                    source: None,
                    later: Vec::new(),
                    candidates: Vec::new(),
                }),
                _ => {}
            }
        }
        if before == report.findings.len()
            || report.findings[before..]
                .iter()
                .any(|f| f.verdict == Verdict::Unchecked || f.also.contains(&Verdict::Unchecked))
        {
            report.unchecked_spans.push(span);
        }
    }
    report
}
