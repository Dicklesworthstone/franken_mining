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

use std::fmt;

use fmine_io::xlsx::{CellValue, ExtractedCell};

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
    /// A number with its stored text. `under_status` flags a non-zero number printed where a disposed or withheld
    /// status applies; it stays visible.
    Number {
        stored: String,
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

/// Types one data cell. `status` is the status that applies to the cell's subject and period
/// ([`status_for`]).
///
/// - No cell: `Missing`.
/// - A dash (`-`, `–`, `—`): `Nil { Unknown }`; `n/a` or `N/A`: `NotApplicable`. Both keep the token as stored,
///   whitespace included.
/// - A number: its stored text. Zero where a disposed or withheld status applies is `Nil` with token `"0"` (as
///   stored); any other number there is flagged.
/// - Text or a formula error refuses.
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
            let stored = cell.number_text.clone().unwrap_or_else(|| v.to_string());
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
                    under_status: status,
                },
                None => PublicValue::Number {
                    stored,
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

/// The same figure across releases: a [`PublicKey`] without its vintage.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SeriesKey {
    pub issuer: String,
    pub subject: String,
    pub valid_period: Period,
    pub ownership_basis: OwnershipBasis,
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
            ownership_basis: self.ownership_basis,
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

#[cfg(test)]
mod tests {
    use super::*;
    use fmine_claim::{
        AdditivityClass, Claim, ConfidentialityTier, Representation, SpatialSupport,
    };
    use fmine_core::rand::{PhiloxStream, RngKey};
    use fmine_evidence::{EvidenceClass, KnowledgeState};
    use fmine_io::styles::NumberFormat;
    use std::collections::BTreeMap;

    fn p(year: u16, kind: PeriodKind) -> Period {
        Period { year, kind }
    }

    /// The 23 distinct sheet names of the eight registered AISC workbooks (40 sheets), hand-listed from their
    /// workbook parts; the expected period of each is read from its name.
    #[test]
    fn test_parse_every_aisc_sheet_name() {
        use PeriodKind::{FullYear, H1, NineMonths, Quarter};
        let want: [(&str, Period); 23] = [
            ("2024", p(2024, FullYear)),
            ("2025", p(2025, FullYear)),
            ("Q1 2024", p(2024, Quarter(1))),
            ("Q1 2025", p(2025, Quarter(1))),
            ("Q1 2026", p(2026, Quarter(1))),
            ("Q2 2024", p(2024, Quarter(2))),
            ("Q2 2025", p(2025, Quarter(2))),
            ("Q2 2026", p(2026, Quarter(2))),
            ("Q3 2023", p(2023, Quarter(3))),
            ("Q3 2024", p(2024, Quarter(3))),
            ("Q3 2025", p(2025, Quarter(3))),
            ("Q4 2023", p(2023, Quarter(4))),
            ("Q4 2024", p(2024, Quarter(4))),
            ("Q4 2025", p(2025, Quarter(4))),
            ("YTD Dec 31 2023", p(2023, FullYear)),
            ("YTD Dec 31 2024", p(2024, FullYear)),
            ("YTD Q2 2024", p(2024, H1)),
            ("YTD Q2 2025", p(2025, H1)),
            ("YTD Q2 2026", p(2026, H1)),
            ("YTD Q3 2024", p(2024, NineMonths)),
            ("YTD Q3 2025", p(2025, NineMonths)),
            ("YTD Sept 30 2023", p(2023, NineMonths)),
            ("YTD Sept 30 2024", p(2024, NineMonths)),
        ];
        for (name, period) in want {
            assert_eq!(parse_sheet_period(name), Ok(period), "{name}");
        }
        assert_eq!(p(2026, Quarter(2)).code(), "2026Q2");
        assert_eq!(p(2026, H1).code(), "2026H1");
        assert_eq!(p(2024, NineMonths).code(), "20249M");
        assert_eq!(p(2025, FullYear).code(), "2025FY");
    }

    #[test]
    fn test_unknown_sheet_name_refused() {
        for name in [
            "",
            "Q5 2024",
            "Q0 2024",
            "Q01 2024",
            "Q1 24",
            "Q1  2024",
            " Q1 2024",
            "Q1 2024 ",
            "YTD Q1 2024",
            "YTD Q4 2024",
            "YTD June 30 2024",
            "YTD Sept 30 202",
            "FY2024",
            "Sheet1",
            "Scratch",
        ] {
            assert_eq!(
                parse_sheet_period(name),
                Err(KeyRefusal::SheetPeriod {
                    name: name.to_string()
                }),
                "{name:?}"
            );
        }
    }

    #[test]
    fn test_ownership_porgera_site_g_hemlo() {
        let printed = |num, den| OwnershipBasis::Attributable {
            num,
            den,
            source: ShareSource::Printed,
        };
        // 24.5% = 245/1000 = 49/200; 60% = 3/5; 61.5% = 123/200.
        assert_eq!(parse_ownership("Porgera (24.5%)1"), Ok(printed(49, 200)));
        assert_eq!(parse_ownership("Site G (60%)1"), Ok(printed(3, 5)));
        assert_eq!(
            parse_ownership("Long Canyon (61.5%)1"),
            Ok(printed(123, 200))
        );
        assert_eq!(parse_ownership("Carlin (61.5%)"), Ok(printed(123, 200)));
        // No share printed: unprinted, never 100%.
        for label in ["Hemlo2", "Hemlo", "Kibali", "Region North12"] {
            assert_eq!(
                parse_ownership(label),
                Ok(OwnershipBasis::Unprinted),
                "{label}"
            );
        }
        // A % that does not read as a share in (0, 100) refuses.
        for label in [
            "Site (abc%)",
            "Site (0%)",
            "Site (150%)",
            "Site (1.2.3%)",
            "Site (60%)123",
            "Site (60%) of the JV",
            "Site 60%",
        ] {
            assert!(
                matches!(parse_ownership(label), Err(KeyRefusal::PrintedShare { .. })),
                "{label}"
            );
        }
    }

    /// `100%` is read before the footnote run is stripped: stripping first would read `(100%)1` as the
    /// attributable share 1/1.
    #[test]
    fn test_ownership_100_detected_before_footnote_strip() {
        for label in [
            "Nevada Gold Mines (100%)1",
            "Turquoise Ridge (100%)",
            "Site 100%",
        ] {
            assert_eq!(
                parse_ownership(label),
                Ok(OwnershipBasis::Consolidated100),
                "{label}"
            );
        }
    }

    fn number(text: &str, v: f64) -> ExtractedCell {
        ExtractedCell {
            cell_ref: "B10".to_string(),
            row: 10,
            col: 2,
            value: CellValue::Number(v),
            formula: None,
            number_text: Some(text.to_string()),
            style: None,
            format: NumberFormat::Builtin(0),
        }
    }

    fn token(t: &str) -> ExtractedCell {
        ExtractedCell {
            value: CellValue::Placeholder(t.to_string()),
            number_text: None,
            ..number("0", 0.0)
        }
    }

    #[test]
    fn test_token_mapping_four_types() {
        assert_eq!(
            classify(Some(&number("1691", 1691.0)), None),
            Ok(PublicValue::Number {
                stored: "1691".to_string(),
                under_status: None
            })
        );
        for dash in ["—", "-", "–", "— "] {
            assert_eq!(
                classify(Some(&token(dash)), None),
                Ok(PublicValue::Nil {
                    token: dash.to_string(),
                    reason: NilReason::Unknown
                })
            );
        }
        for na in ["n/a", "N/A"] {
            assert_eq!(
                classify(Some(&token(na)), None),
                Ok(PublicValue::NotApplicable {
                    token: na.to_string()
                })
            );
        }
        assert_eq!(classify(None, None), Ok(PublicValue::Missing));
        let text = ExtractedCell {
            value: CellValue::Text("see note".to_string()),
            ..number("0", 0.0)
        };
        let error = ExtractedCell {
            value: CellValue::FormulaError("#REF!".to_string()),
            ..number("0", 0.0)
        };
        for c in [text, error] {
            assert_eq!(
                classify(Some(&c), None),
                Err(KeyRefusal::CellToken {
                    cell: "B10".to_string()
                })
            );
        }
    }

    fn statuses() -> Vec<SubjectStatus> {
        vec![
            SubjectStatus::from_date("Tongon", StatusKind::Disposed, (2025, 12, 1), "E485"),
            SubjectStatus::from_date("Hemlo", StatusKind::Disposed, (2025, 11, 26), "E485"),
            SubjectStatus {
                subject: "Loulo-Gounkoto".to_string(),
                kind: StatusKind::Withheld,
                first: (2025, 1),
                last: Some((2025, 9)),
                evidence: "E818".to_string(),
            },
        ]
    }

    #[test]
    fn test_disposed_zero_is_nil_nonzero_kept_flagged() {
        let s = statuses();
        let q1_2026 = p(2026, PeriodKind::Quarter(1));
        let disposed = status_for(&s, "Tongon", &q1_2026);
        assert_eq!(disposed, Some(StatusKind::Disposed));
        assert_eq!(
            classify(Some(&number("0", 0.0)), disposed),
            Ok(PublicValue::Nil {
                token: "0".to_string(),
                reason: NilReason::Disposed
            })
        );
        // E116's 1691 stays visible, flagged.
        assert_eq!(
            classify(Some(&number("1691", 1691.0)), disposed),
            Ok(PublicValue::Number {
                stored: "1691".to_string(),
                under_status: Some(StatusKind::Disposed)
            })
        );
        // Withheld 2025Q1–Q3: H1 and nine months lie inside; the full year does not.
        for (period, applies) in [
            (p(2025, PeriodKind::Quarter(2)), true),
            (p(2025, PeriodKind::H1), true),
            (p(2025, PeriodKind::NineMonths), true),
            (p(2025, PeriodKind::Quarter(4)), false),
            (p(2025, PeriodKind::FullYear), false),
        ] {
            let st = status_for(&s, "Loulo-Gounkoto", &period);
            assert_eq!(st.is_some(), applies, "{period:?}");
            if applies {
                assert_eq!(
                    classify(Some(&number("0", 0.0)), st),
                    Ok(PublicValue::Nil {
                        token: "0".to_string(),
                        reason: NilReason::Withheld
                    })
                );
            }
        }
        // Care and maintenance does not make a zero Nil.
        let cm = [SubjectStatus {
            subject: "Long Canyon".to_string(),
            kind: StatusKind::CareAndMaintenance,
            first: (2024, 1),
            last: None,
            evidence: "W1".to_string(),
        }];
        let st = status_for(&cm, "Long Canyon", &p(2024, PeriodKind::Quarter(1)));
        assert_eq!(
            classify(Some(&number("0", 0.0)), st),
            Ok(PublicValue::Number {
                stored: "0".to_string(),
                under_status: None
            })
        );
    }

    /// A disposal on 1 December (Tongon) or 26 November (Hemlo) leaves the December quarter and the full year
    /// with their numbers; the status applies from the first quarter after.
    #[test]
    fn test_stub_quarter_keeps_numbers() {
        let s = statuses();
        for subject in ["Tongon", "Hemlo"] {
            for period in [
                p(2025, PeriodKind::Quarter(4)),
                p(2025, PeriodKind::FullYear),
            ] {
                let st = status_for(&s, subject, &period);
                assert_eq!(st, None, "{subject} {period:?}");
                assert_eq!(
                    classify(Some(&number("0", 0.0)), st),
                    Ok(PublicValue::Number {
                        stored: "0".to_string(),
                        under_status: None
                    })
                );
            }
            assert_eq!(
                status_for(&s, subject, &p(2026, PeriodKind::Quarter(1))),
                Some(StatusKind::Disposed)
            );
        }
    }

    /// Planted negative: without its status row, a synthetic disposed site's zero reads as a number.
    #[test]
    fn test_remove_tongon_row_breaks_synthetic_zero_test() {
        let mut s = vec![SubjectStatus::from_date(
            "Site Z",
            StatusKind::Disposed,
            (2029, 6, 30),
            "synthetic",
        )];
        let q = p(2030, PeriodKind::Quarter(1));
        let zero = number("0", 0.0);
        assert!(matches!(
            classify(Some(&zero), status_for(&s, "Site Z", &q)),
            Ok(PublicValue::Nil { .. })
        ));
        s.clear();
        assert!(matches!(
            classify(Some(&zero), status_for(&s, "Site Z", &q)),
            Ok(PublicValue::Number { .. })
        ));
    }

    /// The claim id differs whenever any key field or the raw token differs: 10⁴ (key, token) pairs drawn from
    /// small alphabets that include the tag's own separators, so near-collisions are common.
    #[test]
    fn test_property_claim_id_injective_on_key_and_token() {
        let mut rng = PhiloxStream::new(RngKey {
            seed_root: 332,
            partition: 1,
            domain: 0,
            var_group: 0,
            realization: 0,
            brick: 0,
        });
        let alphabet = ["", "a", "b", "|", ":", "1:", "a|b", "|1:a"];
        let mut pick = |n: usize| (rng.next_u64() % n as u64) as usize;
        let mut seen: BTreeMap<String, (PublicKey, String)> = BTreeMap::new();
        let mut ids = std::collections::BTreeSet::new();
        for _ in 0..10_000 {
            let s = |i: usize| alphabet[i].to_string();
            let kinds = [
                PeriodKind::Quarter(1),
                PeriodKind::Quarter(2),
                PeriodKind::H1,
                PeriodKind::FullYear,
            ];
            let bases = [
                OwnershipBasis::Consolidated100,
                OwnershipBasis::Unprinted,
                OwnershipBasis::Attributable {
                    num: 1,
                    den: 2,
                    source: ShareSource::Printed,
                },
                OwnershipBasis::Attributable {
                    num: 1,
                    den: 2,
                    source: ShareSource::Registry,
                },
            ];
            let key = PublicKey {
                issuer: s(pick(3)),
                subject: s(pick(8)),
                valid_period: p(2024 + pick(2) as u16, kinds[pick(4)]),
                reporting_vintage: Vintage {
                    release: s(pick(4)),
                    file_sha256: s(pick(3)),
                },
                ownership_basis: bases[pick(4)],
                scope: match pick(3) {
                    0 => Scope::Site,
                    1 => Scope::Company,
                    _ => Scope::Region {
                        map_version: s(pick(8)),
                    },
                },
                unit: s(pick(8)),
                line: LineKey {
                    label: s(pick(8)),
                    occurrence: pick(2) as u32,
                },
            };
            let raw = s(pick(8));
            let tag = key.canonical_tag(&raw);
            if let Some(prev) = seen.get(&tag) {
                assert_eq!(prev, &(key.clone(), raw.clone()), "tag collision: {tag}");
                continue;
            }
            let claim = Claim::new(
                "public.aisc",
                0_i64,
                &tag,
                Representation::Deterministic,
                SpatialSupport::PeriodAggregate,
                AdditivityClass::IntensiveWeighted,
                EvidenceClass::Exact,
                KnowledgeState::KnownKnown,
                ConfidentialityTier::Public,
                Vec::new(),
                Vec::new(),
                Vec::new(),
            );
            assert!(ids.insert(claim.claim_id), "claim id collision: {tag}");
            seen.insert(tag, (key, raw));
        }
        // The draws revisit pairs, and every distinct pair got its own tag and id.
        assert!(seen.len() > 5_000, "{}", seen.len());
        assert_eq!(ids.len(), seen.len());
    }

    fn workspace_root() -> std::path::PathBuf {
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
    }

    /// The registered statuses, hand-written from E486–E489 and E818, and what each covers.
    #[test]
    fn test_subject_status_registry_golden() {
        let profile = crate::sources::load_issuer_profile(&workspace_root(), "barrick")
            .expect("barrick registry loads");
        let row = |subject: &str, kind, first, last, evidence: &str| SubjectStatus {
            subject: subject.to_string(),
            kind,
            first,
            last,
            evidence: evidence.to_string(),
        };
        assert_eq!(
            profile.subject_statuses,
            vec![
                row(
                    "Hemlo",
                    StatusKind::Disposed,
                    (2025, 12),
                    None,
                    "E486, E488"
                ),
                row(
                    "Tongon",
                    StatusKind::Disposed,
                    (2026, 1),
                    None,
                    "E487, E488"
                ),
                row(
                    "Loulo-Gounkoto",
                    StatusKind::Withheld,
                    (2025, 1),
                    Some((2025, 9)),
                    "E818"
                ),
                row(
                    "Long Canyon",
                    StatusKind::CareAndMaintenance,
                    (2024, 1),
                    None,
                    "E489"
                ),
            ]
        );
        let s = &profile.subject_statuses;
        let q = |y, n| p(y, PeriodKind::Quarter(n));
        for (subject, period, want) in [
            ("Hemlo", q(2025, 4), None),
            ("Hemlo", p(2025, PeriodKind::FullYear), None),
            ("Hemlo", q(2026, 1), Some(StatusKind::Disposed)),
            ("Hemlo", p(2026, PeriodKind::H1), Some(StatusKind::Disposed)),
            ("Tongon", q(2025, 4), None),
            ("Tongon", q(2026, 2), Some(StatusKind::Disposed)),
            ("Loulo-Gounkoto", q(2024, 4), None),
            ("Loulo-Gounkoto", q(2025, 1), Some(StatusKind::Withheld)),
            (
                "Loulo-Gounkoto",
                p(2025, PeriodKind::NineMonths),
                Some(StatusKind::Withheld),
            ),
            ("Loulo-Gounkoto", q(2025, 4), None),
            ("Long Canyon", q(2023, 4), None),
            (
                "Long Canyon",
                q(2026, 2),
                Some(StatusKind::CareAndMaintenance),
            ),
            ("Kibali", q(2026, 2), None),
        ] {
            assert_eq!(
                status_for(s, subject, &period),
                want,
                "{subject} {period:?}"
            );
        }
    }

    #[test]
    fn test_units_registry_has_usd_m_usd_per_oz() {
        let catalog = fmine_registry::RegistryCatalog::load_from_root(&workspace_root())
            .expect("registries load");
        for code in ["usd_m", "usd_per_oz"] {
            assert!(
                catalog.unit_symbols.iter().any(|u| u == code),
                "{code} not registered"
            );
        }
    }

    /// First publication and comparative: same series, different vintage, different tag.
    #[test]
    fn test_series_key_drops_only_the_vintage() {
        let first = PublicKey {
            issuer: "barrick".to_string(),
            subject: "Total Gold".to_string(),
            valid_period: p(2024, PeriodKind::Quarter(1)),
            reporting_vintage: Vintage {
                release: "2024Q1".to_string(),
                file_sha256: "a".repeat(64),
            },
            ownership_basis: OwnershipBasis::Unprinted,
            scope: Scope::Company,
            unit: "usd_per_oz".to_string(),
            line: LineKey {
                label: "all-in sustaining costs".to_string(),
                occurrence: 0,
            },
        };
        let comparative = PublicKey {
            reporting_vintage: Vintage {
                release: "2025Q1".to_string(),
                file_sha256: "b".repeat(64),
            },
            ..first.clone()
        };
        assert_eq!(first.series_key(), comparative.series_key());
        assert_ne!(
            first.canonical_tag("1479"),
            comparative.canonical_tag("1479")
        );
        assert_ne!(first.canonical_tag("1479"), first.canonical_tag("1474"));
    }
}
