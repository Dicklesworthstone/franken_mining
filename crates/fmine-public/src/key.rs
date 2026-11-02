//! The key of a public-lane figure (bd-a28f.1, BD-332.S1): which subject, period, release, ownership basis,
//! scope, unit and line a printed value belongs to, and the value typed with its raw token.
//!
//! - A first publication and a later comparative of the same figure share a [`SeriesKey`] and differ in their
//!   [`Vintage`], so their claim ids differ ([`PublicKey::canonical_tag`]).
//! - Periods come from sheet names in five forms only; anything else refuses ([`parse_sheet_period`]).
//! - A printed `(x%)` is parsed exactly. Where none is printed the basis is [`OwnershipBasis::Unprinted`], never
//!   100% ([`parse_ownership`]).
//! - A numeric zero where a disposed or withheld status applies is `Nil`, its token kept; a non-zero number there
//!   stays a number, flagged ([`classify`]).

use std::collections::BTreeMap;
use std::fmt;

use fmine_claim::{AdditivityClass, Claim, Confidentiality, Representation, SpatialSupport};
use fmine_core::anchor::SourceAnchor;
use fmine_core::codec::Encoder;
use fmine_core::hash::{Digest32, blake3_hash};
use fmine_evidence::{EpistemicStatus, EvidenceClass, KnowledgeState, KuKind};
use fmine_io::xlsx::{CellValue, ExtractedCell, Workbook, parse_a1_ref};

use crate::mine_stats::{MineStatsValue, View};
use crate::sources::IssuerProfile;

pub mod consistency;
pub mod disclosure;
pub mod inspect;
pub mod mine_stats;

/// What a period's length is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PeriodKind {
    /// Quarter 1 to 4.
    Quarter(u8),
    /// January to June.
    H1,
    /// January to September.
    NineMonths,
    /// The calendar year.
    FullYear,
}

/// A reporting period of a calendar year.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Period {
    pub year: u16,
    pub kind: PeriodKind,
}

impl Period {
    /// `2026Q2`, `2026H1`, `20269M` or `2026FY`, as the Mine Stats reader writes its periods.
    #[must_use]
    pub fn code(&self) -> String {
        match self.kind {
            PeriodKind::Quarter(q) => format!("{}Q{q}", self.year),
            PeriodKind::H1 => format!("{}H1", self.year),
            PeriodKind::NineMonths => format!("{}9M", self.year),
            PeriodKind::FullYear => format!("{}FY", self.year),
        }
    }

    /// The first and last month the period covers, as `(year, month)`.
    #[must_use]
    pub const fn months(&self) -> ((u16, u8), (u16, u8)) {
        let (first, last) = match self.kind {
            PeriodKind::Quarter(q) => (3 * q - 2, 3 * q),
            PeriodKind::H1 => (1, 6),
            PeriodKind::NineMonths => (1, 9),
            PeriodKind::FullYear => (1, 12),
        };
        ((self.year, first), (self.year, last))
    }
}

/// Why a key part could not be read. Each names what it read; none guesses.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyRefusal {
    /// A sheet name outside the five registered forms.
    SheetPeriod { name: String },
    /// A printed share that is not a percentage in (0, 100], or not at the end of the label.
    PrintedShare { label: String },
    /// A data cell holding text or a formula error instead of a number or a registered token.
    CellToken { cell: String },
    /// A Mine Stats period or cell reference that cannot be bound to the source workbook.
    SourceBinding { locator: String, reason: String },
    /// No explicit supported unit in the row, section or summary field.
    Unit { label: String },
}

impl fmt::Display for KeyRefusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SheetPeriod { name } => {
                write!(f, "sheet name {name:?} is not a registered period form")
            }
            Self::PrintedShare { label } => {
                write!(f, "printed share in {label:?} is not a percentage")
            }
            Self::CellToken { cell } => {
                write!(f, "{cell} holds neither a number nor a registered token")
            }
            Self::SourceBinding { locator, reason } => write!(f, "{locator}: {reason}"),
            Self::Unit { label } => write!(f, "no supported explicit unit for {label:?}"),
        }
    }
}

fn year(s: &str) -> Option<u16> {
    (s.len() == 4 && s.bytes().all(|b| b.is_ascii_digit()))
        .then(|| s.parse().ok())
        .flatten()
}

/// The period an AISC reconciliation sheet name states. The forms are exactly `Qn YYYY` (n in 1..=4),
/// `YTD Q2 YYYY` (H1), `YTD Q3 YYYY` and `YTD Sept 30 YYYY` (nine months), and `YTD Dec 31 YYYY` and `YYYY`
/// (full year). Anything else refuses.
pub fn parse_sheet_period(name: &str) -> Result<Period, KeyRefusal> {
    let refuse = || KeyRefusal::SheetPeriod {
        name: name.to_string(),
    };
    let words: Vec<&str> = name.split(' ').collect();
    let (kind, y) = match words.as_slice() {
        [y] => (PeriodKind::FullYear, *y),
        [q, y] => {
            let n = q.strip_prefix('Q').and_then(|n| n.parse::<u8>().ok());
            match n {
                Some(n @ 1..=4) if q.len() == 2 => (PeriodKind::Quarter(n), *y),
                _ => return Err(refuse()),
            }
        }
        ["YTD", "Q2", y] => (PeriodKind::H1, *y),
        ["YTD", "Q3", y] | ["YTD", "Sept", "30", y] => (PeriodKind::NineMonths, *y),
        ["YTD", "Dec", "31", y] => (PeriodKind::FullYear, *y),
        _ => return Err(refuse()),
    };
    let year = year(y).ok_or_else(refuse)?;
    Ok(Period { year, kind })
}

/// Where a share came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ShareSource {
    /// Printed in the label as `(x%)`.
    Printed,
    /// A registry row with an evidence id.
    Registry,
}

/// The ownership basis of a figure. A share is an exact fraction in lowest terms.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum OwnershipBasis {
    /// The label prints `100%`.
    Consolidated100,
    Attributable {
        num: i64,
        den: i64,
        source: ShareSource,
    },
    /// No share is printed. Never read as 100%.
    Unprinted,
}

/// The effective basis used to compare figures across releases, independently of share provenance.
/// The exact share and an unprinted basis remain distinct; [`PublicKey`] retains the source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SeriesOwnershipBasis {
    Consolidated100,
    Attributable { num: i64, den: i64 },
    Unprinted,
}

impl From<OwnershipBasis> for SeriesOwnershipBasis {
    fn from(basis: OwnershipBasis) -> Self {
        match basis {
            OwnershipBasis::Consolidated100 => Self::Consolidated100,
            OwnershipBasis::Attributable { num, den, .. } => Self::Attributable { num, den },
            OwnershipBasis::Unprinted => Self::Unprinted,
        }
    }
}

const fn gcd(mut a: i64, mut b: i64) -> i64 {
    while b != 0 {
        let t = a % b;
        a = b;
        b = t;
    }
    a
}

/// The ownership basis printed in a header label such as `Porgera (24.5%)1`.
///
/// `100%` anywhere in the label is [`OwnershipBasis::Consolidated100`], checked before anything is stripped.
/// Otherwise a trailing footnote run of one or two digits is stripped, and a final `(x%)` with x in (0, 100) is
/// the printed share. A label with no `%` is [`OwnershipBasis::Unprinted`]; a `%` that does not read that way
/// refuses.
pub fn parse_ownership(label: &str) -> Result<OwnershipBasis, KeyRefusal> {
    if label.contains("100%") {
        return Ok(OwnershipBasis::Consolidated100);
    }
    if !label.contains('%') {
        return Ok(OwnershipBasis::Unprinted);
    }
    let refuse = || KeyRefusal::PrintedShare {
        label: label.to_string(),
    };
    let trimmed = label.trim_end();
    let digits = trimmed.len() - trimmed.trim_end_matches(|c: char| c.is_ascii_digit()).len();
    let body = if (1..=2).contains(&digits) {
        trimmed[..trimmed.len() - digits].trim_end()
    } else {
        trimmed
    };
    let pct = body
        .strip_suffix("%)")
        .and_then(|b| b.rsplit_once('('))
        .map(|(_, p)| p)
        .ok_or_else(refuse)?;
    let (int, frac) = pct.split_once('.').unwrap_or((pct, ""));
    let all_digits = |s: &str| s.bytes().all(|b| b.is_ascii_digit());
    if int.is_empty() || int.len() > 2 || frac.len() > 3 || !all_digits(int) || !all_digits(frac) {
        return Err(refuse());
    }
    // x% = digits / (100 · 10^places).
    let scale = 10_i64.pow(frac.len() as u32);
    let num: i64 = format!("{int}{frac}").parse().map_err(|_| refuse())?;
    let den = 100 * scale;
    if num <= 0 || num >= den {
        return Err(refuse());
    }
    let g = gcd(num, den);
    Ok(OwnershipBasis::Attributable {
        num: num / g,
        den: den / g,
        source: ShareSource::Printed,
    })
}

/// A registered status of a subject that changes how its zeros read.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum StatusKind {
    Disposed,
    Withheld,
    CareAndMaintenance,
}

/// One subject-status row: the status applies to a period that lies entirely within months `first` to `last`
/// (open-ended when `last` is `None`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubjectStatus {
    pub subject: String,
    pub kind: StatusKind,
    pub first: (u16, u8),
    pub last: Option<(u16, u8)>,
    pub evidence: String,
}

impl SubjectStatus {
    /// Applies from the first month entirely after `(year, month, day)`: a disposal on 1 December applies from
    /// January, so the December quarter and the full year keep their numbers.
    #[must_use]
    pub fn from_date(
        subject: &str,
        kind: StatusKind,
        (year, month, _day): (u16, u8, u8),
        evidence: &str,
    ) -> Self {
        let first = if month == 12 {
            (year + 1, 1)
        } else {
            (year, month + 1)
        };
        Self {
            subject: subject.to_string(),
            kind,
            first,
            last: None,
            evidence: evidence.to_string(),
        }
    }

    /// Whether the status applies to all of `period`.
    #[must_use]
    pub fn applies(&self, period: &Period) -> bool {
        let (start, end) = period.months();
        start >= self.first && self.last.is_none_or(|l| end <= l)
    }
}

/// The status that applies to `subject` in `period`, if any.
#[must_use]
pub fn status_for(
    statuses: &[SubjectStatus],
    subject: &str,
    period: &Period,
) -> Option<StatusKind> {
    statuses
        .iter()
        .find(|s| s.subject == subject && s.applies(period))
        .map(|s| s.kind)
}

/// Why a value is `Nil`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum NilReason {
    /// A dash token: the issuer prints nothing and says nothing why.
    Unknown,
    Withheld,
    Disposed,
}

/// A printed value, typed, with its token as stored.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PublicValue {
    /// A number with its stored text, and its text as displayed under the cell's number format
    /// ([`fmine_io::numfmt::Displayed::text`]), or the code of the display refusal when the format has no exact
    /// rendering. `under_status` flags a non-zero number printed where a disposed or withheld status applies; it
    /// stays visible.
    Number {
        stored: String,
        displayed: Result<String, &'static str>,
        under_status: Option<StatusKind>,
    },
    Nil {
        token: String,
        reason: NilReason,
    },
    NotApplicable {
        token: String,
    },
    Missing,
}

/// The exact source token, including whitespace; a missing cell has no token.
#[must_use]
pub fn raw_token(value: &PublicValue) -> &str {
    match value {
        PublicValue::Number { stored, .. } => stored,
        PublicValue::Nil { token, .. } | PublicValue::NotApplicable { token } => token,
        PublicValue::Missing => "",
    }
}

/// A public figure with both its semantic key and its source-anchored Claim (BD-332).
#[derive(Debug, Clone, PartialEq)]
pub struct KeyedCell {
    /// Full file name, sheet and cell, not the release shorthand used by the raw Mine Stats reader.
    pub locator: String,
    pub key: PublicKey,
    pub claim: Claim<PublicValue>,
    /// Kept beside the effective basis when an evidenced registry row corrects a printed share.
    pub printed_basis: OwnershipBasis,
    pub basis_evidence: Option<String>,
    /// Registry evidence for an applicable disposed, withheld or care-and-maintenance status.
    pub status_evidence: Option<String>,
    /// Printed label and the evidence for any registered unit correction; no quantity is rescaled.
    pub label_as_printed: Option<String>,
    pub unit_evidence: Option<String>,
}

fn encode_value(enc: &mut Encoder, value: &PublicValue) {
    match value {
        PublicValue::Number {
            stored,
            displayed,
            under_status,
        } => {
            enc.put_u8(0);
            enc.put_str(stored);
            match displayed {
                Ok(text) => {
                    enc.put_u8(0);
                    enc.put_str(text);
                }
                Err(code) => {
                    enc.put_u8(1);
                    enc.put_str(code);
                }
            }
            enc.put_u8(match under_status {
                None => 0,
                Some(StatusKind::Disposed) => 1,
                Some(StatusKind::Withheld) => 2,
                Some(StatusKind::CareAndMaintenance) => 3,
            });
        }
        PublicValue::Nil { token, reason } => {
            enc.put_u8(1);
            enc.put_str(token);
            enc.put_u8(match reason {
                NilReason::Unknown => 0,
                NilReason::Withheld => 1,
                NilReason::Disposed => 2,
            });
        }
        PublicValue::NotApplicable { token } => {
            enc.put_u8(2);
            enc.put_str(token);
        }
        PublicValue::Missing => enc.put_u8(3),
    }
}

/// Builds an issuer assertion or an explicitly unknown placeholder. DESIGN: the identity commits to the
/// typed interpretation as well as the raw token, so changing a status rule cannot reuse a numeric Claim id.
#[must_use]
pub fn public_claim(
    workbook: &Workbook,
    sheet: &str,
    cell_ref: &str,
    key: PublicKey,
    value: PublicValue,
) -> KeyedCell {
    public_claim_anchored(workbook.make_anchor(sheet, cell_ref), vec![], key, value)
}

/// Builds the same canonical public Claim from a primary value anchor and explicit interpretation
/// evidence. The workbook wrapper passes no additional anchors, preserving its existing IDs exactly.
#[must_use]
pub fn public_claim_anchored(
    anchor: SourceAnchor,
    interpretation_anchors: Vec<SourceAnchor>,
    key: PublicKey,
    value: PublicValue,
) -> KeyedCell {
    let tag = canonical_value_tag(&key, &value);
    let numeric = matches!(value, PublicValue::Number { .. });
    let representation = if numeric {
        Representation::Deterministic
    } else {
        Representation::Placeholder {
            token: raw_token(&value).to_string(),
        }
    };
    let (evidence, epistemic, state) =
        if numeric && key.issuer != crate::synthetic::SYNTHETIC_ISSUER {
            (
                EvidenceClass::ISSUER,
                EpistemicStatus::KnownKnown,
                KnowledgeState::Known,
            )
        } else {
            (
                EvidenceClass::Assumed,
                EpistemicStatus::KnownUnknown(KuKind::Parametric),
                KnowledgeState::Unknown,
            )
        };
    let additivity = match key.unit.as_str() {
        "usd_m" | "koz" | "kt" | "Mlb" => AdditivityClass::ExtensiveAdditive,
        "g_per_t" | "wt_pct" | "percent" | "mass_fraction" | "usd_per_oz" | "usd_per_lb"
        | "usd_per_t" => AdditivityClass::IntensiveWeighted,
        _ => AdditivityClass::NonAdditive,
    };
    let file = std::path::Path::new(&anchor.artifact_path)
        .file_name()
        .and_then(|p| p.to_str())
        .unwrap_or(&anchor.artifact_path);
    let locator = format!("{file}:{}", anchor.locator);
    let mut anchors = vec![anchor];
    for evidence in interpretation_anchors {
        if !anchors.contains(&evidence) {
            anchors.push(evidence);
        }
    }
    let claim = Claim::new(
        &key.line.label,
        value,
        &tag,
        representation,
        SpatialSupport::PeriodAggregate,
        additivity,
        evidence,
        epistemic,
        state,
        Confidentiality::public(),
        vec![],
        vec![],
        anchors,
    );
    KeyedCell {
        locator,
        printed_basis: key.ownership_basis,
        key,
        claim,
        basis_evidence: None,
        status_evidence: None,
        label_as_printed: None,
        unit_evidence: None,
    }
}

/// The existing public Claim value tag, shared with consumers that recheck a supplied keyed value.
/// It binds the semantic key, stored token, displayed value and subject-status interpretation.
#[must_use]
pub fn canonical_value_tag(key: &PublicKey, value: &PublicValue) -> String {
    let mut enc = Encoder::new_frame(*b"PVAL", 1);
    encode_value(&mut enc, value);
    format!(
        "{}|{}",
        key.canonical_tag(raw_token(value)),
        blake3_hash(&enc.finish()).to_hex()
    )
}

/// Groups figures without losing earlier vintages or colliding cells. Order is independent of input order.
#[must_use]
pub fn series(cells: &[KeyedCell]) -> BTreeMap<SeriesKey, Vec<&KeyedCell>> {
    let mut grouped: BTreeMap<SeriesKey, Vec<&KeyedCell>> = BTreeMap::new();
    for cell in cells {
        grouped.entry(cell.key.series_key()).or_default().push(cell);
    }
    for values in grouped.values_mut() {
        values.sort_by(|a, b| {
            (&a.key.reporting_vintage, &a.locator).cmp(&(&b.key.reporting_vintage, &b.locator))
        });
    }
    grouped
}

/// DESIGN: a region's comparability follows its registered membership at the release, including when a
/// later release restates a comparative. A missing map stays release-specific, never implicitly comparable.
#[must_use]
pub fn scope_for(profile: &IssuerProfile, subject: &str, release: &str) -> Scope {
    if profile.regions.iter().any(|s| s == subject) {
        let mut members: Vec<&str> = profile
            .region_members
            .iter()
            .filter(|r| r.region == subject && r.holds_for(release))
            .map(|r| r.member.as_str())
            .collect();
        members.sort_unstable();
        members.dedup();
        let map_version = if members.is_empty() {
            format!("unregistered:{release}")
        } else {
            let mut enc = Encoder::new_frame(*b"RMAP", 1);
            for member in members {
                enc.put_str(member);
            }
            blake3_hash(&enc.finish()).to_hex()
        };
        Scope::Region { map_version }
    } else if matches!(subject, "Total Gold" | "Total Copper") {
        Scope::Company
    } else {
        Scope::Site
    }
}

/// A Mine Stats period already read from the block header. No calendar is inferred from its position.
pub fn parse_period_code(code: &str) -> Option<Period> {
    if !code.is_ascii() || code.len() < 6 {
        return None;
    }
    let y = year(&code[..4])?;
    let kind = match &code[4..] {
        "Q1" => PeriodKind::Quarter(1),
        "Q2" => PeriodKind::Quarter(2),
        "Q3" => PeriodKind::Quarter(3),
        "Q4" => PeriodKind::Quarter(4),
        "H1" => PeriodKind::H1,
        "9M" => PeriodKind::NineMonths,
        "FY" => PeriodKind::FullYear,
        _ => return None,
    };
    Some(Period { year: y, kind })
}

fn normalized(text: &str) -> String {
    text.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

fn mine_stats_unit(
    value: &MineStatsValue,
    label: &str,
    cell: Option<&ExtractedCell>,
) -> Result<&'static str, KeyRefusal> {
    let unit = if value.view == View::Summary {
        if label.ends_with("_koz") {
            Some("koz")
        } else if label.ends_with("_kt") {
            Some("kt")
        } else if label.ends_with("_usd_per_oz") {
            Some("usd_per_oz")
        } else if label.ends_with("_usd_per_lb") {
            Some("usd_per_lb")
        } else {
            None
        }
    } else {
        let context = normalized(value.section.as_deref().unwrap_or(label));
        let percent_format = cell
            .and_then(|c| c.displayed())
            .and_then(Result::ok)
            .filter(|d| d.token.is_none())
            .map(|d| d.percent)
            .unwrap_or_else(|| {
                cell.and_then(|c| fmine_io::numfmt::parse_format(&c.format.code()).ok())
                    .is_some_and(|f| {
                        let numeric: Vec<_> = f
                            .sections
                            .iter()
                            .filter(|s| s.zero_token.is_none())
                            .collect();
                        !numeric.is_empty() && numeric.iter().all(|s| s.percent)
                    })
            });
        if context.contains("($/tonne)") {
            Some("usd_per_t")
        } else if context.contains("($/oz)") || context == "costs per ounce" {
            Some("usd_per_oz")
        } else if context.contains("($/lb)") || context == "costs per pound" {
            Some("usd_per_lb")
        } else if context.contains("($ millions)") {
            Some("usd_m")
        } else if context.contains("(grams per tonne)") {
            Some("g_per_t")
        } else if context.contains("(percent)") || context == "average grade" {
            // Excel's percent format scales only the presentation. The stored number's unit stays a
            // fraction; `0.91` and `91%` are both preserved, never silently relabelled as 0.91 percent.
            if percent_format {
                Some("mass_fraction")
            } else if context.contains("(percent)") {
                Some("percent")
            } else {
                None
            }
        } else if context.contains("(thousands of ounces)") {
            Some("koz")
        } else if context.contains("(millions of pounds)") {
            Some("Mlb")
        } else if context.contains("(thousands of tonnes)")
            || (context.starts_with("tonnes ") && context.contains("(thousands)"))
        {
            Some("kt")
        } else if context == "strip ratio" {
            Some("ratio")
        } else {
            None
        }
    };
    unit.ok_or_else(|| KeyRefusal::Unit {
        label: format!("{} / {label}", value.section.as_deref().unwrap_or("")),
    })
}

/// Binds one raw Mine Stats value to its workbook and registry (BD-332.S3). The raw reader remains the sole
/// extractor; only this adapter applies disposal/withholding and the evidenced printed-share/unit corrections.
pub fn from_mine_stats(
    profile: &IssuerProfile,
    workbook: &Workbook,
    release: &str,
    value: &MineStatsValue,
) -> Result<KeyedCell, KeyRefusal> {
    let binding = |reason: &str| KeyRefusal::SourceBinding {
        locator: value.locator.clone(),
        reason: reason.to_string(),
    };
    let period = parse_period_code(&value.period).ok_or_else(|| binding("unparsed period"))?;
    if !fmine_registry::sources::is_quarter(release) {
        return Err(binding("unparsed reporting release"));
    }
    if value.locator != format!("{release}:{}!{}", value.sheet, value.cell) {
        return Err(binding(
            "read locator differs from the requested release or cell",
        ));
    }
    if !profile.sources.is_empty() {
        let anchor = workbook.make_anchor(&value.sheet, &value.cell);
        let file = std::path::Path::new(&anchor.artifact_path)
            .file_name()
            .and_then(|p| p.to_str());
        if !profile.sources.iter().any(|s| {
            Some(s.file_name.as_str()) == file
                && s.quarter == release
                && s.sha256_hex == Digest32(workbook.sha256).to_hex()
        }) {
            return Err(binding(
                "source file, digest and release do not match the registry",
            ));
        }
    }
    let sheet = workbook
        .sheet(&value.sheet)
        .ok_or_else(|| binding("source sheet absent"))?;
    let (row, col) = parse_a1_ref(&value.cell).ok_or_else(|| binding("invalid cell reference"))?;
    let cell = sheet.cell(row, col);
    // The extracted value and source cell must still agree; callers cannot pair a read with another workbook.
    let matches = match (&value.value, cell.map(|c| &c.value)) {
        (
            crate::mine_stats::MineStatsCell::Number { raw_text, value: x },
            Some(CellValue::Number(y)),
        ) => {
            x.to_bits() == y.to_bits()
                && cell.and_then(|c| c.number_text.as_ref()) == Some(raw_text)
        }
        (
            crate::mine_stats::MineStatsCell::Nil { token }
            | crate::mine_stats::MineStatsCell::NotApplicable { token },
            Some(CellValue::Placeholder(t)),
        ) => token == t,
        (crate::mine_stats::MineStatsCell::Missing, None) => true,
        _ => false,
    };
    if !matches {
        return Err(binding("read does not match the source cell"));
    }
    let alias = profile
        .mine_stats_label_aliases
        .iter()
        .find(|a| a.label == value.label && a.holds_for(release));
    let label = alias.map_or(value.label.as_str(), |a| a.reads_as.as_str());
    let unit = mine_stats_unit(value, label, cell)?;
    let subject = profile
        .shares
        .iter()
        .find(|s| s.label == value.site)
        .map_or(value.site.as_str(), |s| s.subject.as_str());
    let printed = parse_ownership(&value.site_raw)?;
    let correction = profile.printed_share_corrections.iter().find(|c| {
        c.subject == subject && value.printed_share.as_deref() == Some(c.printed.as_str())
    });
    let basis = if value.view == View::Summary && value.label.contains("_prod_100_") {
        OwnershipBasis::Consolidated100
    } else if let Some(c) = correction {
        let num = i64::try_from(c.num).map_err(|_| binding("registered share out of range"))?;
        let den = i64::try_from(c.den).map_err(|_| binding("registered share out of range"))?;
        if num <= 0 || den <= 0 || num > den {
            return Err(binding("invalid registered share"));
        }
        let g = gcd(num, den);
        OwnershipBasis::Attributable {
            num: num / g,
            den: den / g,
            source: ShareSource::Registry,
        }
    } else {
        printed
    };
    let line = if value.view == View::Summary {
        format!("summary:{}", normalized(label))
    } else {
        format!(
            "site:{}:{}",
            normalized(value.section.as_deref().unwrap_or("")),
            normalized(label)
        )
    };
    let key = PublicKey {
        issuer: profile.issuer.clone(),
        subject: subject.to_string(),
        valid_period: period,
        reporting_vintage: Vintage {
            release: release.to_string(),
            file_sha256: Digest32(workbook.sha256).to_hex(),
        },
        ownership_basis: basis,
        scope: scope_for(profile, subject, release),
        unit: unit.to_string(),
        line: LineKey {
            label: line,
            occurrence: 0,
        },
    };
    let typed = classify(
        cell,
        status_for(&profile.subject_statuses, subject, &period),
    )?;
    let mut keyed = public_claim(workbook, &value.sheet, &value.cell, key, typed);
    keyed.printed_basis = printed;
    keyed.basis_evidence = correction
        .filter(|_| !value.label.contains("_prod_100_"))
        .map(|c| c.evidence.clone());
    keyed.label_as_printed = Some(value.label_as_printed.clone());
    keyed.unit_evidence = alias.map(|a| a.evidence.clone());
    keyed.cite_status(profile);
    Ok(keyed)
}

/// A separate digest for the keyed projection. It preserves the existing identity-ledger root and binds
/// status/basis interpretations, source anchors and all typed values for consumers of this projection.
#[must_use]
pub fn claims_digest<'a>(cells: impl IntoIterator<Item = &'a KeyedCell>) -> Digest32 {
    let mut ordered: Vec<&KeyedCell> = cells.into_iter().collect();
    ordered.sort_by(|a, b| (&a.key, &a.locator).cmp(&(&b.key, &b.locator)));
    let mut enc = Encoder::new_frame(*b"PCLM", 1);
    enc.put_u64(ordered.len() as u64);
    for cell in ordered {
        enc.put_str(&cell.locator);
        enc.put_str(&cell.claim.claim_id.to_string());
        enc.put_str(&cell.key.canonical_tag(raw_token(&cell.claim.value)));
        encode_value(&mut enc, &cell.claim.value);
        let mut printed = cell.key.clone();
        printed.ownership_basis = cell.printed_basis;
        enc.put_str(&printed.canonical_tag(""));
        enc.put_str(cell.basis_evidence.as_deref().unwrap_or(""));
        enc.put_str(cell.status_evidence.as_deref().unwrap_or(""));
        enc.put_str(cell.label_as_printed.as_deref().unwrap_or(""));
        enc.put_str(cell.unit_evidence.as_deref().unwrap_or(""));
    }
    blake3_hash(&enc.finish())
}

fn basis_json(basis: OwnershipBasis) -> String {
    match basis {
        OwnershipBasis::Consolidated100 => "{\"kind\":\"consolidated100\"}".to_string(),
        OwnershipBasis::Unprinted => "{\"kind\":\"unprinted\"}".to_string(),
        OwnershipBasis::Attributable { num, den, source } => format!(
            "{{\"kind\":\"attributable\",\"share\":\"{num}/{den}\",\"source\":\"{}\"}}",
            if source == ShareSource::Printed {
                "printed"
            } else {
                "registry"
            }
        ),
    }
}

impl KeyedCell {
    pub(crate) fn cite_status(&mut self, profile: &IssuerProfile) {
        self.status_evidence = profile
            .subject_statuses
            .iter()
            .find(|s| s.subject == self.key.subject && s.applies(&self.key.valid_period))
            .map(|s| s.evidence.clone());
    }

    /// Stable robot output. `unit` describes the stored number; `displayed` preserves formatting such as `%`.
    #[must_use]
    pub fn to_json(&self) -> String {
        use crate::js;
        let scope = match &self.key.scope {
            Scope::Site => "{\"kind\":\"site\"}".to_string(),
            Scope::JointVenture => "{\"kind\":\"joint_venture\"}".to_string(),
            Scope::Company => "{\"kind\":\"company\"}".to_string(),
            Scope::Region { map_version } => format!(
                "{{\"kind\":\"region\",\"map_version\":{}}}",
                js(map_version)
            ),
        };
        let value = match &self.claim.value {
            PublicValue::Number {
                stored,
                displayed,
                under_status,
            } => format!(
                "{{\"kind\":\"number\",\"stored\":{},\"displayed\":{},\"display_refusal\":{},\"under_status\":{}}}",
                js(stored),
                displayed
                    .as_ref()
                    .ok()
                    .map_or_else(|| "null".to_string(), |s| js(s)),
                displayed
                    .as_ref()
                    .err()
                    .map_or_else(|| "null".to_string(), |s| js(s)),
                under_status.map_or("null", |s| match s {
                    StatusKind::Disposed => "\"after_disposal\"",
                    StatusKind::Withheld => "\"withheld\"",
                    StatusKind::CareAndMaintenance => "\"care_and_maintenance\"",
                })
            ),
            PublicValue::Nil { token, reason } => format!(
                "{{\"kind\":\"nil\",\"token\":{},\"reason\":\"{}\"}}",
                js(token),
                match reason {
                    NilReason::Unknown => "unknown",
                    NilReason::Withheld => "withheld",
                    NilReason::Disposed => "disposed",
                }
            ),
            PublicValue::NotApplicable { token } => {
                format!("{{\"kind\":\"not_applicable\",\"token\":{}}}", js(token))
            }
            PublicValue::Missing => "{\"kind\":\"missing\"}".to_string(),
        };
        let optional = |s: &Option<String>| s.as_deref().map_or_else(|| "null".to_string(), js);
        format!(
            "{{\"claim_id\":{},\"at\":{},\"sha256\":{},\"issuer\":{},\"subject\":{},\"period\":{},\"vintage\":{},\
             \"ownership_basis\":{},\"printed_basis\":{},\"basis_evidence\":{},\"status_evidence\":{},\"scope\":{},\"unit\":{},\
             \"line\":{},\"occurrence\":{},\"label_as_printed\":{},\"unit_evidence\":{},\"value\":{},\
             \"evidence\":\"{}\",\"knowledge\":\"{}\"}}",
            js(&self.claim.claim_id.to_string()),
            js(&self.locator),
            js(&self.key.reporting_vintage.file_sha256),
            js(&self.key.issuer),
            js(&self.key.subject),
            js(&self.key.valid_period.code()),
            js(&self.key.reporting_vintage.release),
            basis_json(self.key.ownership_basis),
            basis_json(self.printed_basis),
            optional(&self.basis_evidence),
            optional(&self.status_evidence),
            scope,
            js(&self.key.unit),
            js(&self.key.line.label),
            self.key.line.occurrence,
            optional(&self.label_as_printed),
            optional(&self.unit_evidence),
            value,
            if self.key.issuer == crate::synthetic::SYNTHETIC_ISSUER {
                "synthetic"
            } else if self.claim.evidence == EvidenceClass::ISSUER {
                "issuer_assertion"
            } else {
                "assumed"
            },
            if self.claim.state == KnowledgeState::Known {
                "known"
            } else {
                "unknown"
            },
        )
    }
}

/// Types one data cell. `status` is the status that applies to the cell's subject and period
/// ([`status_for`]).
///
/// - No cell: `Missing`.
/// - A dash (`-`, `–`, `—`): `Nil { Unknown }`; `n/a` or `N/A`: `NotApplicable`. Both keep the token as stored,
///   whitespace included.
/// - A number: its stored and displayed text. Zero where a disposed or withheld status applies is `Nil` with
///   token `"0"` (as stored); any other number there is flagged.
/// - Text, a formula error, or a number without its stored text refuses.
pub fn classify(
    cell: Option<&ExtractedCell>,
    status: Option<StatusKind>,
) -> Result<PublicValue, KeyRefusal> {
    let Some(cell) = cell else {
        return Ok(PublicValue::Missing);
    };
    let refuse = || KeyRefusal::CellToken {
        cell: cell.cell_ref.clone(),
    };
    match &cell.value {
        CellValue::Number(v) => {
            let Some(stored) = cell.number_text.clone() else {
                return Err(refuse());
            };
            let displayed = match cell.displayed() {
                Some(Ok(d)) => Ok(d.text()),
                Some(Err(r)) => Err(r.code),
                None => return Err(refuse()),
            };
            let nil_reason = match status {
                Some(StatusKind::Disposed) => Some(NilReason::Disposed),
                Some(StatusKind::Withheld) => Some(NilReason::Withheld),
                _ => None,
            };
            Ok(match nil_reason {
                Some(reason) if *v == 0.0 => PublicValue::Nil {
                    token: stored,
                    reason,
                },
                Some(_) => PublicValue::Number {
                    stored,
                    displayed,
                    under_status: status,
                },
                None => PublicValue::Number {
                    stored,
                    displayed,
                    under_status: None,
                },
            })
        }
        CellValue::Placeholder(t) => match t.trim() {
            "-" | "–" | "—" => Ok(PublicValue::Nil {
                token: t.clone(),
                reason: NilReason::Unknown,
            }),
            "n/a" | "N/A" => Ok(PublicValue::NotApplicable { token: t.clone() }),
            _ => Err(refuse()),
        },
        CellValue::Text(_) | CellValue::FormulaError(_) => Err(refuse()),
    }
}

/// The release a figure was read from.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Vintage {
    /// `2025Q2`.
    pub release: String,
    pub file_sha256: String,
}

/// What a figure covers.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Scope {
    Site,
    JointVenture,
    Region { map_version: String },
    Company,
}

/// A row: its normalised label and which occurrence of that label (two NCI rows can exist).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LineKey {
    pub label: String,
    pub occurrence: u32,
}

/// The same figure across releases: a [`PublicKey`] without its vintage or share provenance.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SeriesKey {
    pub issuer: String,
    pub subject: String,
    pub valid_period: Period,
    pub ownership_basis: SeriesOwnershipBasis,
    pub scope: Scope,
    pub unit: String,
    pub line: LineKey,
}

/// The key of one printed public figure.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PublicKey {
    pub issuer: String,
    pub subject: String,
    pub valid_period: Period,
    pub reporting_vintage: Vintage,
    pub ownership_basis: OwnershipBasis,
    pub scope: Scope,
    /// A unit id registered in `registries/units.toml`, e.g. `usd_m`.
    pub unit: String,
    pub line: LineKey,
}

impl PublicKey {
    #[must_use]
    pub fn series_key(&self) -> SeriesKey {
        SeriesKey {
            issuer: self.issuer.clone(),
            subject: self.subject.clone(),
            valid_period: self.valid_period,
            ownership_basis: self.ownership_basis.into(),
            scope: self.scope.clone(),
            unit: self.unit.clone(),
            line: self.line.clone(),
        }
    }

    /// `fmk1` followed by every key field and the raw token, each as `|<length>:<text>`, so no two different
    /// (key, token) pairs share a tag. It is the claim's canonical value tag: the claim id hashes it.
    #[must_use]
    pub fn canonical_tag(&self, raw_token: &str) -> String {
        let basis = match self.ownership_basis {
            OwnershipBasis::Consolidated100 => "consolidated100".to_string(),
            OwnershipBasis::Attributable { num, den, source } => {
                let source = match source {
                    ShareSource::Printed => "printed",
                    ShareSource::Registry => "registry",
                };
                format!("attributable:{num}/{den}:{source}")
            }
            OwnershipBasis::Unprinted => "unprinted".to_string(),
        };
        let scope = match &self.scope {
            Scope::Site => "site".to_string(),
            Scope::JointVenture => "joint_venture".to_string(),
            Scope::Region { map_version } => format!("region:{map_version}"),
            Scope::Company => "company".to_string(),
        };
        let fields = [
            self.issuer.as_str(),
            &self.subject,
            &self.valid_period.code(),
            &self.reporting_vintage.release,
            &self.reporting_vintage.file_sha256,
            &basis,
            &scope,
            &self.unit,
            &self.line.label,
            &self.line.occurrence.to_string(),
            raw_token,
        ];
        let mut tag = String::from("fmk1");
        for f in fields {
            tag.push_str(&format!("|{}:{f}", f.len()));
        }
        tag
    }
}

// Public key contract checks live in xtask/tests/public_key_contract.rs.
