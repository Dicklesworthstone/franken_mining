//! DESIGN (BD-341/BD-342): declared source dates and strict retrieval interval upper bounds.
//! Determinism: Exact (calendar arithmetic; no clock, file access or inferred publication date).

use fmine_evidence::{Refusal, RefusalCode};
use fmine_time::CivilDate;

use super::{PUBLISHED_AT_BASES, is_date};

/// The registered workbook vocabulary, shared by schema admission and public projections.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SourceKind {
    MineStats,
    AiscReconciliation,
}

impl SourceKind {
    pub const ALL: [Self; 2] = [Self::MineStats, Self::AiscReconciliation];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::MineStats => "mine_stats",
            Self::AiscReconciliation => "aisc_reconciliation",
        }
    }
}

/// The typed workbook projection of a checked source registry row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisteredSource {
    pub file_name: String,
    pub kind: SourceKind,
    /// Reporting quarter of the release, `YYYYQn`.
    pub quarter: String,
    pub url: String,
    pub url_basis: String,
    pub bytes: u64,
    pub sha256_hex: String,
    pub retrieved_utc: Option<String>,
    pub published_at: Option<String>,
    pub published_at_basis: Option<String>,
}

impl TryFrom<super::SourceRow> for RegisteredSource {
    type Error = String;

    fn try_from(row: super::SourceRow) -> Result<Self, Self::Error> {
        let kind = SourceKind::ALL
            .into_iter()
            .find(|kind| kind.as_str() == row.kind)
            .ok_or_else(|| format!("source kind {:?} has no SourceKind", row.kind))?;
        Ok(Self {
            file_name: row.file,
            kind,
            quarter: row.quarter,
            url: row.url,
            url_basis: row.url_basis,
            bytes: row.bytes,
            sha256_hex: row.sha256,
            retrieved_utc: row.retrieved_utc,
            published_at: row.published_at,
            published_at_basis: row.published_at_basis,
        })
    }
}

impl RegisteredSource {
    /// A later release can carry its own retrieval evidence without changing earlier sources' timing.
    #[must_use]
    pub fn retrieval<'a>(&'a self, batch: &'a str) -> &'a str {
        self.retrieved_utc.as_deref().unwrap_or(batch)
    }
}

fn invalid(why: impl Into<String>) -> Refusal {
    Refusal::new(RefusalCode::UndeclaredUnitBasisOrTz107, why)
}

/// Validate the paired optional source citation without interpreting retrieval as publication.
pub fn publication_date<'a>(
    date: Option<&'a str>,
    basis: Option<&str>,
) -> Result<Option<&'a str>, Refusal> {
    match (date, basis) {
        (None, None) => Ok(None),
        (Some(date), Some(basis)) => {
            if !is_date(date) {
                return Err(invalid(format!("published_at {date:?} is not YYYY-MM-DD")));
            }
            if basis.chars().any(char::is_control)
                || !PUBLISHED_AT_BASES.iter().any(|prefix| {
                    basis
                        .strip_prefix(prefix)
                        .is_some_and(|s| !s.trim().is_empty())
                })
            {
                return Err(invalid(
                    "published_at_basis must start with FACT: or INFERENCE: and cite its source without control characters",
                ));
            }
            Ok(Some(date))
        }
        _ => Err(invalid(
            "published_at and published_at_basis must be supplied together",
        )),
    }
}

fn date_days(text: &str) -> Option<i64> {
    if !is_date(text) {
        return None;
    }
    CivilDate::new(
        text[..4].parse().ok()?,
        text[5..7].parse().ok()?,
        text[8..].parse().ok()?,
    )
    .ok()
    .map(|date| date.to_epoch_days())
}

/// Upper bound for a date-only availability declaration, in UTC seconds since the Unix epoch.
pub fn day_end(text: &str) -> Result<i64, Refusal> {
    date_days(text)
        .and_then(|day| day.checked_mul(86_400)?.checked_add(86_399))
        .ok_or_else(|| invalid(format!("invalid availability date {text:?}")))
}

fn utc(text: &str) -> Option<i64> {
    let b = text.as_bytes();
    if b.len() != 20 || b[10] != b'T' || b[13] != b':' || b[16] != b':' || b[19] != b'Z' {
        return None;
    }
    let digits = |range: std::ops::Range<usize>| {
        b[range].iter().try_fold(0_i64, |n, c| {
            c.is_ascii_digit().then(|| n * 10 + i64::from(c - b'0'))
        })
    };
    let (hour, minute, second) = (digits(11..13)?, digits(14..16)?, digits(17..19)?);
    if hour > 23 || minute > 59 || second > 59 {
        return None;
    }
    Some(date_days(text.get(..10)?)? * 86_400 + hour * 3600 + minute * 60 + second)
}

/// Exact timestamps, date-only evidence and monotone intervals, including same-day short endpoints.
pub fn retrieval_upper(text: &str) -> Result<i64, Refusal> {
    let mut previous = None;
    let mut day = None;
    for part in text.split('/') {
        let full = if part.len() == 9 && part.ends_with('Z') {
            format!(
                "{}T{part}",
                day.as_deref()
                    .ok_or_else(|| invalid("retrieval interval lacks its date"))?
            )
        } else {
            part.to_string()
        };
        let time = match utc(&full) {
            Some(time) => time,
            None => day_end(&full)?,
        };
        if previous.is_some_and(|p| time < p) {
            return Err(invalid("retrieval interval is reversed"));
        }
        day = full.get(..10).map(str::to_string);
        previous = Some(time);
    }
    previous.ok_or_else(|| invalid("the source registry lacks retrieval evidence"))
}

/// Date-level public-record availability: a cited publication date, or retrieval as a fallback bound.
/// This does not prove the historical existence of a registry or the possession of bytes at that date.
pub fn available_date(
    date: Option<&str>,
    basis: Option<&str>,
    retrieved: &str,
) -> Result<String, Refusal> {
    if let Some(date) = publication_date(date, basis)? {
        return Ok(date.to_string());
    }
    let date = CivilDate::from_epoch_days(retrieval_upper(retrieved)?.div_euclid(86_400));
    Ok(format!(
        "{:04}-{:02}-{:02}",
        date.year, date.month, date.day
    ))
}

/// The actual declaration used to admit or exclude one workbook at a date-level cut.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceAvailability {
    pub file: String,
    pub available_by: String,
    pub basis_kind: &'static str,
    pub basis: String,
    pub eligible: bool,
}

/// A declared public-record cut, not proof that this registry existed at that time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublicationCut {
    pub available_by: Option<String>,
    pub sources: Vec<SourceAvailability>,
}

impl PublicationCut {
    #[must_use]
    pub fn includes(&self, file: &str) -> bool {
        self.sources.iter().any(|s| s.file == file && s.eligible)
    }
}

/// Selects registered source declarations before their bytes are opened; undated rows use retrieval.
pub fn publication_cut<'a>(
    rows: impl IntoIterator<Item = (&'a str, Option<&'a str>, Option<&'a str>)>,
    retrieved: &str,
    as_of: &str,
) -> Result<PublicationCut, Refusal> {
    publication_cut_rows(
        rows.into_iter()
            .map(|(file, date, basis)| (file, date, basis, retrieved)),
        as_of,
    )
}

/// The same public-record selection with each workbook's own retrieval fallback.
pub fn source_publication_cut<'a>(
    sources: impl IntoIterator<Item = &'a RegisteredSource>,
    batch: &'a str,
    as_of: &str,
) -> Result<PublicationCut, Refusal> {
    publication_cut_rows(
        sources.into_iter().map(|s| {
            (
                s.file_name.as_str(),
                s.published_at.as_deref(),
                s.published_at_basis.as_deref(),
                s.retrieval(batch),
            )
        }),
        as_of,
    )
}

fn publication_cut_rows<'a>(
    rows: impl IntoIterator<Item = (&'a str, Option<&'a str>, Option<&'a str>, &'a str)>,
    as_of: &str,
) -> Result<PublicationCut, Refusal> {
    if !is_date(as_of) {
        return Err(Refusal::new(
            RefusalCode::AssayLookahead108,
            "as-of must be YYYY-MM-DD",
        ));
    }
    let mut sources = Vec::new();
    let mut available_by = None;
    let mut files = std::collections::BTreeSet::new();
    for (file, date, basis, retrieved) in rows {
        if !files.insert(file) {
            return Err(invalid(format!("duplicate availability source {file:?}")));
        }
        let date_bound = available_date(date, basis, retrieved)?;
        let eligible = date_bound.as_str() <= as_of;
        if eligible && available_by.as_ref().is_none_or(|old| old < &date_bound) {
            available_by = Some(date_bound.clone());
        }
        sources.push(SourceAvailability {
            file: file.into(),
            available_by: date_bound,
            basis_kind: if date.is_some() {
                "registered_publication"
            } else {
                "verified_retrieval_upper_bound"
            },
            basis: basis.unwrap_or(retrieved).into(),
            eligible,
        });
    }
    sources.sort_by(|a, b| a.file.cmp(&b.file));
    Ok(PublicationCut {
        available_by,
        sources,
    })
}
