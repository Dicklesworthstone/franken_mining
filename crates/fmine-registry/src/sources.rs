//! Issuer source registries, `registries/sources/<issuer>.toml` (schema `fmine.sources.v1`, bd-s9j3.1).
//!
//! The registry is data read at run time: registering a new quarterly vintage is a TOML edit and never a code
//! change. This module parses one file with [`crate::toml_subset`] and checks it (the `registry-check` rules):
//! unique file names, paths and digests; 64-character lowercase hex digests; `YYYYQ[1-4]` quarters; registered
//! kinds, URL bases and identity families; and every reconstruction pointing at registered files, subjects and
//! label-check sets. Row order is kept, because a run's state root binds its sources in registry order.

use std::collections::BTreeSet;
use std::fmt;
use std::path::Path;

use crate::toml_subset::{SubsetError, TomlDoc, TomlField, TomlRow, TomlValue, parse_subset};

/// Schema id a sources registry must declare.
pub const SOURCES_SCHEMA: &str = "fmine.sources.v1";
/// Registered source kinds (`fmine_public::sources::SourceKind::as_str`).
pub const SOURCE_KINDS: [&str; 2] = ["mine_stats", "aisc_reconciliation"];
/// Registered URL bases: `"W1 table"` is FACT (the URL was read from the issuer's page), `"W1 naming rule"` is
/// INFERENCE (the URL follows the issuer's stated naming pattern). A URL fetched and found to serve the
/// registered bytes may instead carry [`URL_BASIS_FETCHED`] followed by the fetch date (`is_url_basis`).
pub const URL_BASES: [&str; 2] = ["W1 table", "W1 naming rule"];
/// Prefix of the dated URL basis, `"fetched and verified YYYY-MM-DD"` (BD-339.S3).
pub const URL_BASIS_FETCHED: &str = "fetched and verified ";
/// Registered identity families (`fmine_public::aisc::IdentityFamily::as_str`).
pub const IDENTITY_FAMILIES: [&str; 5] = [
    "tcc_footing",
    "aisc_footing",
    "cos_per_oz",
    "tcc_per_oz",
    "aisc_per_oz",
];

/// Registered document kinds (bd-plx1, BD-342): PDF filings whose values
/// enter only as transcription rows, since the repository has no PDF reader.
pub const DOCUMENT_KINDS: [&str; 6] = [
    "mda",
    "aif",
    "investor_deck",
    "partner_filing",
    "technical_report",
    "information_circular",
];
/// The prefixes a document's `published_at_basis` starts with: what the date rests on, then the citation.
pub const PUBLISHED_AT_BASES: [&str; 2] = ["FACT: ", "INFERENCE: "];

/// One registered PDF document (bd-plx1). A run that cites a transcribed value checks the document's bytes the
/// way it checks a workbook's; it never reads values from the file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentRow {
    pub file: String,
    /// The directory holding the file, relative to the workspace root: `dir` when given, else `corpus_dir`.
    pub dir: String,
    /// One of [`DOCUMENT_KINDS`].
    pub kind: String,
    pub url: String,
    pub url_basis: String,
    /// `YYYY-MM-DD`: the earliest date its basis shows these bytes, or their text, were public.
    pub published_at: String,
    /// Starts with one of [`PUBLISHED_AT_BASES`].
    pub published_at_basis: String,
    pub pages: u64,
    pub bytes: u64,
    pub sha256: String,
}

/// One registered HTML filing. Its table cells are read directly from hash-verified bytes;
/// it has a reporting release and publication citation, never an invented PDF page count.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HtmlSourceRow {
    pub file: String,
    pub dir: String,
    pub release: String,
    pub url: String,
    pub url_basis: String,
    pub published_at: String,
    pub published_at_basis: String,
    pub bytes: u64,
    pub sha256: String,
}

/// One registered workbook.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceRow {
    pub file: String,
    pub kind: String,
    /// `YYYYQn`.
    pub quarter: String,
    /// `url_prefix` + `path`.
    pub url: String,
    pub url_basis: String,
    pub bytes: u64,
    pub sha256: String,
    /// Optional publication date for these registered bytes or their text, never inferred from retrieval.
    pub published_at: Option<String>,
    /// Required with the date; uses the same FACT/INFERENCE citation convention as documents.
    pub published_at_basis: Option<String>,
}

/// One registered reconstruction (plan §21.6 state (ii)).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReconstructionRow {
    pub evidence_id: String,
    pub table_file: String,
    pub sheet: String,
    pub subject: String,
    pub family: String,
    pub component_file: String,
    pub component_sheet: String,
    pub component_cell: String,
    /// `(cell, prefix)` pairs of the named label-check set, in file order.
    pub label_checks: Vec<(String, String)>,
    pub rule: String,
    pub basis: BasisRow,
}

/// Whether the issuer states a reconstruction's rule or the registry reconstructs it (BD-331.S2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BasisRow {
    /// No issuer text states the rule.
    Reconstructed,
    /// The issuer states it: the run re-reads `doc` at `locator` (`sheet!cell`) and needs text starting
    /// `quote_prefix`.
    Stated {
        doc: String,
        locator: String,
        quote_prefix: String,
    },
}

/// The fact that would decide a (iv) instance of `family` for `subject` (BD-331.S2). `text` is prose with no
/// digits; the source goes in `evidence`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MissingFactRow {
    pub subject: String,
    pub family: String,
    pub text: String,
    pub evidence: String,
    /// A document known to decide the instance, if any.
    pub closing_document: Option<String>,
}

/// The registered subject statuses: a status changes how a subject's printed zeros read (bd-a28f.1).
pub const SUBJECT_STATUSES: [&str; 3] = ["disposed", "withheld", "care_and_maintenance"];

/// One registered subject status (bd-a28f.1). A disposal states its closing date and covers the periods that
/// begin after it; any other status states the first quarter it covers and, when it ended, the last.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubjectStatusRow {
    pub subject: String,
    /// One of [`SUBJECT_STATUSES`].
    pub status: String,
    /// `YYYY-MM-DD`; set exactly when the status is `disposed`.
    pub closed: Option<String>,
    /// `YYYYQn`; set exactly when the status is not `disposed`.
    pub first: Option<String>,
    /// `YYYYQn`, at or after `first`.
    pub last: Option<String>,
    pub evidence: String,
}

/// One registered region member (BD-331.S4, bd-gwf1.1): `member` rolls up into `region` in every file released
/// from `first` through `last` (open when absent). The map is keyed by the file's release quarter, never by a
/// sheet's period, because a release restates its comparatives under its own map (W1 §13 rule 2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegionMemberRow {
    pub region: String,
    pub member: String,
    /// `YYYYQn`.
    pub first: String,
    /// `YYYYQn`, at or after `first`.
    pub last: Option<String>,
}

impl RegionMemberRow {
    /// Whether the row holds for a file released in `release` (`YYYYQn`).
    #[must_use]
    pub fn holds_for(&self, release: &str) -> bool {
        self.first.as_str() <= release && self.last.as_deref().is_none_or(|l| release <= l)
    }

    fn overlaps(&self, other: &Self) -> bool {
        let before = |a: &str, b: Option<&str>| b.is_none_or(|b| a <= b);
        before(&self.first, other.last.as_deref()) && before(&other.first, self.last.as_deref())
    }
}

/// One registered Mine Statistics row label (bd-gwf1.1): the label without its footnote number, and the label of
/// the row it belongs to when it is a sub-row (`Processed` under `Average grade (grams per tonne)`).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct MineStatsLabelRow {
    pub section: Option<String>,
    pub label: String,
}

/// One registered ownership share (BD-331.S3): the issuer's stated share `num/den` in a site, which the run
/// confirms against the same-release Mine Stats label before using it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShareRow {
    pub subject: String,
    pub num: u64,
    pub den: u64,
    /// Where the share comes from (an E id, or a named document).
    pub evidence: String,
    /// The Mine Stats label, when it differs from the subject name (`Nevada Gold Mines LLC`).
    pub label: Option<String>,
}

/// A share a Mine Stats label prints that the issuer's own figures contradict (BD-331.S4, W1-DR-01): where the
/// label of `subject` prints `printed` percent, the run uses `num/den` and keeps the printed share beside it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrintedShareCorrection {
    pub subject: String,
    /// The percentage as printed inside `(…%)`, e.g. `47.5`.
    pub printed: String,
    pub num: u64,
    pub den: u64,
    pub evidence: String,
}

/// A Mine Statistics row label printed with a stale unit (BD-331.S4, W1-DR-02): in files released from `first` to
/// `last`, the row labelled `label` is read as the registered row `reads_as`. Values are never converted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MineStatsLabelAlias {
    pub label: String,
    pub reads_as: String,
    /// `YYYYQn`.
    pub first: String,
    /// `YYYYQn`, at or after `first`.
    pub last: String,
    pub evidence: String,
}

impl MineStatsLabelAlias {
    /// Whether the alias holds for a file released in `release` (`YYYYQn`).
    #[must_use]
    pub fn holds_for(&self, release: &str) -> bool {
        self.first.as_str() <= release && release <= self.last.as_str()
    }
}

/// A registered issuer disclosure (BD-504e.2). Runtime consumers must re-read the exact quoted cell from the
/// registered bytes before the note can explain a change; parsing this row alone does not verify the quote.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RestatementNoteRow {
    pub id: String,
    pub file_name: String,
    /// A single unquoted worksheet name and canonical A1 cell, `sheet!cell`.
    pub locator: String,
    pub quoted_text: String,
    /// Earliest reporting release allowed to use the note, never before its source file's release.
    pub from_quarter: String,
    pub evidence: String,
    /// Derived from the referenced `[[source]]`; never supplied independently in the note row.
    pub sha256: String,
}

/// Exact coverage of one registered note. No wildcard expands a subject or line. A consumer must require the
/// complete valid period (including H1/FY endpoints) inside this inclusive quarter window.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RestatementScopeRow {
    pub note: String,
    pub subject: String,
    pub period_from: String,
    pub period_to: String,
    /// The public key's exact canonical line label, checked against observed keys by the runtime consumer.
    pub line: String,
    /// Zero-based occurrence, as in the canonical public key; defaults to zero when omitted.
    pub occurrence: u32,
}

/// A checked issuer registry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourcesRegistry {
    pub issuer: String,
    pub corpus_dir: String,
    pub retrieved_utc: String,
    pub sources: Vec<SourceRow>,
    pub subjects: Vec<String>,
    /// The subjects registered with `kind = "region"`: segment roll-ups of sites, never one share.
    pub regions: Vec<String>,
    /// Stated shares, one per site at most, in file order.
    pub shares: Vec<ShareRow>,
    pub reconstructions: Vec<ReconstructionRow>,
    /// The Mine Statistics label vocabulary, in file order. Empty when the issuer registers none, in which case
    /// labels are read without a vocabulary check.
    pub mine_stats_labels: Vec<MineStatsLabelRow>,
    /// Missing facts by `(subject, family)`, in file order.
    pub missing_facts: Vec<MissingFactRow>,
    /// Subject statuses, at most one per subject, in file order.
    pub subject_statuses: Vec<SubjectStatusRow>,
    /// Region members, in file order; a site is in at most one region for any release.
    pub region_members: Vec<RegionMemberRow>,
    /// Printed shares the run corrects, at most one per `(subject, printed)`, in file order.
    pub printed_share_corrections: Vec<PrintedShareCorrection>,
    /// Stale Mine Statistics labels read as registered ones, at most one per label for any release, in file order.
    pub mine_stats_label_aliases: Vec<MineStatsLabelAlias>,
    /// Registered PDF documents (bd-plx1), in file order. Their file names and digests are unique across
    /// workbooks and documents together, since a citation names a file.
    pub documents: Vec<DocumentRow>,
    /// Registered HTML filings, with file names and digests unique across all source formats.
    pub html_sources: Vec<HtmlSourceRow>,
    /// Explicit issuer disclosures and their exact scopes, in registry order. Empty for older registries.
    pub restatement_notes: Vec<RestatementNoteRow>,
    pub restatement_scopes: Vec<RestatementScopeRow>,
}

impl SourcesRegistry {
    /// The members of `region` in a file released in `release`, in registry order.
    #[must_use]
    pub fn members_of(&self, region: &str, release: &str) -> Vec<String> {
        self.region_members
            .iter()
            .filter(|m| m.region == region && m.holds_for(release))
            .map(|m| m.member.clone())
            .collect()
    }
}

/// Why a sources registry was refused. `line` is 1-based; 0 means the file as a whole.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourcesError {
    pub line: usize,
    pub reason: String,
}

impl fmt::Display for SourcesError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.line == 0 {
            write!(f, "{}", self.reason)
        } else {
            write!(f, "line {}: {}", self.line, self.reason)
        }
    }
}

impl std::error::Error for SourcesError {}

impl From<SubsetError> for SourcesError {
    fn from(e: SubsetError) -> Self {
        Self {
            line: e.line,
            reason: e.reason,
        }
    }
}

pub(crate) fn fail<T>(line: usize, reason: impl Into<String>) -> Result<T, SourcesError> {
    Err(SourcesError {
        line,
        reason: reason.into(),
    })
}

/// `YYYYQ1` … `YYYYQ4`.
#[must_use]
pub fn is_quarter(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() == 6
        && b[..4].iter().all(u8::is_ascii_digit)
        && b[4] == b'Q'
        && (b'1'..=b'4').contains(&b[5])
}

/// One of [`URL_BASES`], or [`URL_BASIS_FETCHED`] followed by a calendar date `YYYY-MM-DD`.
#[must_use]
pub fn is_url_basis(s: &str) -> bool {
    if URL_BASES.contains(&s) {
        return true;
    }
    s.strip_prefix(URL_BASIS_FETCHED).is_some_and(is_date)
}

/// A calendar date `YYYY-MM-DD`.
#[must_use]
pub fn is_date(date: &str) -> bool {
    let b = date.as_bytes();
    let num = |r: std::ops::Range<usize>| -> Option<u32> {
        b[r].iter().try_fold(0_u32, |n, c| {
            c.is_ascii_digit().then(|| n * 10 + u32::from(c - b'0'))
        })
    };
    if b.len() != 10 || b[4] != b'-' || b[7] != b'-' {
        return false;
    }
    let (Some(y), Some(m), Some(d)) = (num(0..4), num(5..7), num(8..10)) else {
        return false;
    };
    let leap = (y % 4 == 0 && y % 100 != 0) || y % 400 == 0;
    let days = match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap => 29,
        2 => 28,
        _ => return false,
    };
    (1..=days).contains(&d)
}

pub(crate) fn is_lower_hex64(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
}

/// A relative path with no empty, `.` or `..` component and no backslash.
fn is_plain_relative(s: &str) -> bool {
    !s.is_empty()
        && !s.starts_with('/')
        && !s.contains('\\')
        && s.split('/').all(|c| !c.is_empty() && c != "." && c != "..")
}

fn is_single_cell_locator(locator: &str) -> bool {
    let Some((sheet, cell)) = locator.split_once('!') else {
        return false;
    };
    if sheet.is_empty()
        || sheet.trim() != sheet
        || sheet.starts_with('\'')
        || sheet.ends_with('\'')
        || sheet.chars().count() > 31
        || sheet
            .chars()
            .any(|c| c.is_control() || "[]:*?/\\!".contains(c))
    {
        return false;
    }
    let letters = cell.bytes().take_while(u8::is_ascii_uppercase).count();
    if letters == 0 || letters > 3 {
        return false;
    }
    let (column, row) = cell.split_at(letters);
    let col = column
        .bytes()
        .fold(0_u32, |n, c| n * 26 + u32::from(c - b'A' + 1));
    col <= 16_384
        && row.starts_with(|c: char| ('1'..='9').contains(&c))
        && row.bytes().all(|c| c.is_ascii_digit())
        && row.parse::<u32>().is_ok_and(|r| r <= 1_048_576)
}

fn parse_restatements(
    doc: &TomlDoc,
    sources: &[SourceRow],
    subjects: &[String],
) -> Result<(Vec<RestatementNoteRow>, Vec<RestatementScopeRow>), SourcesError> {
    let mut notes: Vec<RestatementNoteRow> = Vec::new();
    for row in doc.rows_of("restatement_note") {
        let f = Fields::of_row(row);
        f.only(&[
            "id",
            "file_name",
            "locator",
            "quoted_text",
            "from_quarter",
            "evidence",
        ])?;
        let (id, id_line) = f.string("id")?;
        if !id
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"_-".contains(&c))
        {
            return fail(
                id_line,
                "restatement note id must use letters, digits, _ or -",
            );
        }
        let (file_name, file_line) = f.string("file_name")?;
        let Some(source) = sources.iter().find(|s| s.file == file_name) else {
            return fail(
                file_line,
                format!("restatement file {file_name:?} is not a registered [[source]]"),
            );
        };
        let (locator, locator_line) = f.string("locator")?;
        if !is_single_cell_locator(&locator) {
            return fail(
                locator_line,
                "restatement locator must be one sheet!A1 cell within worksheet bounds",
            );
        }
        let (quoted_text, quote_line) = f.string("quoted_text")?;
        if quoted_text.trim().is_empty() || quoted_text.chars().any(char::is_control) {
            return fail(
                quote_line,
                "quoted_text must contain nonblank text without control characters",
            );
        }
        let (from_quarter, from_line) = f.string("from_quarter")?;
        if !is_quarter(&from_quarter) || from_quarter < source.quarter {
            return fail(
                from_line,
                "from_quarter must be YYYYQ1..YYYYQ4 at or after the source release",
            );
        }
        if notes.iter().any(|n| n.id == id) {
            return fail(id_line, format!("duplicate restatement note {id:?}"));
        }
        if notes
            .iter()
            .any(|n| n.file_name == file_name && n.locator == locator)
        {
            return fail(locator_line, "duplicate restatement note source cell");
        }
        let (evidence, evidence_line) = f.string("evidence")?;
        if evidence.trim().is_empty() {
            return fail(evidence_line, "restatement evidence must not be blank");
        }
        notes.push(RestatementNoteRow {
            id,
            file_name,
            locator,
            quoted_text,
            from_quarter,
            evidence,
            sha256: source.sha256.clone(),
        });
    }
    let mut scopes: Vec<RestatementScopeRow> = Vec::new();
    for row in doc.rows_of("restatement_scope") {
        let f = Fields::of_row(row);
        f.only(&[
            "note",
            "subject",
            "period_from",
            "period_to",
            "line",
            "occurrence",
        ])?;
        let (note, note_line) = f.string("note")?;
        if !notes.iter().any(|n| n.id == note) {
            return fail(
                note_line,
                format!("scope note {note:?} is not a registered [[restatement_note]]"),
            );
        }
        let (subject, subject_line) = f.string("subject")?;
        if !subjects.contains(&subject) {
            return fail(
                subject_line,
                format!("scope subject {subject:?} is not registered"),
            );
        }
        let (period_from, from_line) = f.string("period_from")?;
        let (period_to, to_line) = f.string("period_to")?;
        if !is_quarter(&period_from) {
            return fail(from_line, "period_from must be YYYYQ1..YYYYQ4");
        }
        if !is_quarter(&period_to) || period_to < period_from {
            return fail(
                to_line,
                "period_to must be YYYYQ1..YYYYQ4 at or after period_from",
            );
        }
        let (line, line_number) = f.string("line")?;
        if line.trim() != line
            || line
                .chars()
                .any(|c| c.is_control() || matches!(c, '*' | '?'))
        {
            return fail(
                line_number,
                "scope line must be an exact nonblank label without wildcards",
            );
        }
        let occurrence = match row.get("occurrence") {
            None => 0,
            Some(TomlField {
                value: TomlValue::Int(n),
                line,
                ..
            }) => u32::try_from(*n).map_err(|_| SourcesError {
                line: *line,
                reason: "occurrence must be a zero-based u32 integer".into(),
            })?,
            Some(f) => return fail(f.line, "occurrence must be a zero-based u32 integer"),
        };
        if scopes.iter().any(|s| {
            s.note == note
                && s.subject == subject
                && s.line == line
                && s.occurrence == occurrence
                && s.period_from <= period_to
                && period_from <= s.period_to
        }) {
            return fail(row.line, "duplicate or overlapping restatement scope");
        }
        scopes.push(RestatementScopeRow {
            note,
            subject,
            period_from,
            period_to,
            line,
            occurrence,
        });
    }
    for (note, row) in notes.iter().zip(doc.rows_of("restatement_note")) {
        if !scopes.iter().any(|s| s.note == note.id) {
            return fail(
                row.line,
                format!("unused restatement note {:?}: no scope", note.id),
            );
        }
    }
    Ok((notes, scopes))
}

/// Reads the fields of one row (or of the top level), refusing missing, mistyped and unknown keys.
pub(crate) struct Fields<'a> {
    pub(crate) fields: &'a [TomlField],
    pub(crate) line: usize,
    pub(crate) what: String,
}

impl<'a> Fields<'a> {
    pub(crate) fn of_row(row: &'a TomlRow) -> Self {
        Self {
            fields: &row.fields,
            line: row.line,
            what: format!("[[{}]] row", row.table),
        }
    }

    pub(crate) fn only(&self, allowed: &[&str]) -> Result<(), SourcesError> {
        for f in self.fields {
            if !allowed.contains(&f.key.as_str()) {
                return fail(
                    f.line,
                    format!(
                        "unknown key {:?} in {} (allowed: {})",
                        f.key,
                        self.what,
                        allowed.join(", ")
                    ),
                );
            }
        }
        Ok(())
    }

    pub(crate) fn string(&self, key: &str) -> Result<(String, usize), SourcesError> {
        match self.fields.iter().find(|f| f.key == key) {
            Some(TomlField {
                value: TomlValue::Str(s),
                line,
                ..
            }) if !s.is_empty() => Ok((s.clone(), *line)),
            Some(f) => fail(f.line, format!("{key} must be a non-empty string")),
            None => fail(self.line, format!("{} has no {key}", self.what)),
        }
    }

    /// Like [`Self::string`], for a key that may be absent.
    fn optional_string(&self, key: &str) -> Result<Option<String>, SourcesError> {
        if self.fields.iter().any(|f| f.key == key) {
            self.string(key).map(|(s, _)| Some(s))
        } else {
            Ok(None)
        }
    }

    pub(crate) fn positive_int(&self, key: &str) -> Result<(u64, usize), SourcesError> {
        match self.fields.iter().find(|f| f.key == key) {
            Some(TomlField {
                value: TomlValue::Int(n),
                line,
                ..
            }) if *n > 0 => Ok((n.unsigned_abs(), *line)),
            Some(f) => fail(f.line, format!("{key} must be a positive integer")),
            None => fail(self.line, format!("{} has no {key}", self.what)),
        }
    }
}

/// Parses and checks one sources registry.
pub fn parse_sources_registry(text: &str) -> Result<SourcesRegistry, SourcesError> {
    let doc = parse_subset(text)?;
    let top = Fields {
        fields: &doc.top,
        line: 0,
        what: "the top level".to_string(),
    };
    top.only(&[
        "schema",
        "issuer",
        "corpus_dir",
        "retrieved_utc",
        "url_prefix",
    ])?;
    let (schema, schema_line) = top.string("schema")?;
    if schema != SOURCES_SCHEMA {
        return fail(
            schema_line,
            format!("schema {schema:?} is not {SOURCES_SCHEMA:?}"),
        );
    }
    let (issuer, issuer_line) = top.string("issuer")?;
    if !issuer
        .bytes()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'_')
    {
        return fail(
            issuer_line,
            format!("issuer {issuer:?} must be lowercase letters, digits or _"),
        );
    }
    let (corpus_dir, corpus_line) = top.string("corpus_dir")?;
    if !is_plain_relative(&corpus_dir) {
        return fail(
            corpus_line,
            format!("corpus_dir {corpus_dir:?} must be a plain relative path"),
        );
    }
    let (retrieved_utc, _) = top.string("retrieved_utc")?;
    let (url_prefix, prefix_line) = top.string("url_prefix")?;
    if !url_prefix.starts_with("https://") || !url_prefix.ends_with('/') {
        return fail(
            prefix_line,
            "url_prefix must start with https:// and end with /",
        );
    }

    for row in &doc.rows {
        if ![
            "source",
            "subject",
            "reconstruction",
            "label_check",
            "mine_stats_label",
            "share",
            "missing_fact",
            "subject_status",
            "region_member",
            "printed_share_correction",
            "mine_stats_label_alias",
            "document",
            "html",
            "restatement_note",
            "restatement_scope",
        ]
        .contains(&row.table.as_str())
        {
            return fail(row.line, format!("unknown table [[{}]]", row.table));
        }
    }

    let mut sources = Vec::new();
    let (mut files, mut paths, mut digests) = (BTreeSet::new(), BTreeSet::new(), BTreeSet::new());
    for row in doc.rows_of("source") {
        let f = Fields::of_row(row);
        f.only(&[
            "file",
            "kind",
            "quarter",
            "path",
            "url_basis",
            "bytes",
            "sha256",
            "published_at",
            "published_at_basis",
        ])?;
        let (file, file_line) = f.string("file")?;
        let (kind, kind_line) = f.string("kind")?;
        let (quarter, quarter_line) = f.string("quarter")?;
        let (path, path_line) = f.string("path")?;
        let (url_basis, basis_line) = f.string("url_basis")?;
        let (bytes, _) = f.positive_int("bytes")?;
        let (sha256, sha_line) = f.string("sha256")?;
        if file.contains('/') || file.contains('\\') || file.starts_with('.') {
            return fail(file_line, format!("file {file:?} must be a bare file name"));
        }
        if !files.insert(file.clone()) {
            return fail(file_line, format!("duplicate file {file:?}"));
        }
        if !SOURCE_KINDS.contains(&kind.as_str()) {
            return fail(
                kind_line,
                format!(
                    "kind {kind:?} is not registered ({})",
                    SOURCE_KINDS.join(", ")
                ),
            );
        }
        if !is_quarter(&quarter) {
            return fail(
                quarter_line,
                format!("quarter {quarter:?} is not YYYYQ1..YYYYQ4"),
            );
        }
        if !is_plain_relative(&path) {
            return fail(
                path_line,
                format!("path {path:?} must be a plain relative path"),
            );
        }
        if !paths.insert(path.clone()) {
            return fail(path_line, format!("duplicate path {path:?}"));
        }
        if !is_url_basis(&url_basis) {
            return fail(
                basis_line,
                format!(
                    "url_basis {url_basis:?} is not registered ({}, or \"{URL_BASIS_FETCHED}YYYY-MM-DD\")",
                    URL_BASES.join(", ")
                ),
            );
        }
        if !is_lower_hex64(&sha256) {
            return fail(sha_line, "sha256 must be 64 lowercase hex characters");
        }
        if !digests.insert(sha256.clone()) {
            return fail(sha_line, format!("duplicate sha256 {sha256}"));
        }
        let published_at = f.optional_string("published_at")?;
        let published_at_basis = f.optional_string("published_at_basis")?;
        match (&published_at, &published_at_basis) {
            (None, None) => {}
            (Some(date), Some(basis)) => {
                if !is_date(date) {
                    return fail(row.line, format!("published_at {date:?} is not YYYY-MM-DD"));
                }
                if !PUBLISHED_AT_BASES.iter().any(|prefix| {
                    basis
                        .strip_prefix(prefix)
                        .is_some_and(|citation| !citation.trim().is_empty())
                }) || basis.chars().any(char::is_control)
                {
                    return fail(
                        row.line,
                        "published_at_basis must start with FACT: or INFERENCE: and cite its source without control characters",
                    );
                }
            }
            _ => {
                return fail(
                    row.line,
                    "published_at and published_at_basis must be supplied together",
                );
            }
        }
        sources.push(SourceRow {
            file,
            kind,
            quarter,
            url: format!("{url_prefix}{path}"),
            url_basis,
            bytes,
            sha256,
            published_at,
            published_at_basis,
        });
    }
    let mut documents = Vec::new();
    for row in doc.rows_of("document") {
        let f = Fields::of_row(row);
        f.only(&[
            "file",
            "dir",
            "kind",
            "url",
            "url_basis",
            "published_at",
            "published_at_basis",
            "pages",
            "bytes",
            "sha256",
        ])?;
        let (file, file_line) = f.string("file")?;
        let (kind, kind_line) = f.string("kind")?;
        let (url, url_line) = f.string("url")?;
        let (url_basis, basis_line) = f.string("url_basis")?;
        let (published_at, date_line) = f.string("published_at")?;
        let (published_at_basis, pub_basis_line) = f.string("published_at_basis")?;
        let (pages, _) = f.positive_int("pages")?;
        let (bytes, _) = f.positive_int("bytes")?;
        let (sha256, sha_line) = f.string("sha256")?;
        if file.contains('/') || file.contains('\\') || file.starts_with('.') {
            return fail(file_line, format!("file {file:?} must be a bare file name"));
        }
        if !files.insert(file.clone()) {
            return fail(file_line, format!("duplicate file {file:?}"));
        }
        let dir = match f.optional_string("dir")? {
            Some(d) if !is_plain_relative(&d) => {
                return fail(row.line, format!("dir {d:?} must be a plain relative path"));
            }
            Some(d) => d,
            None => corpus_dir.clone(),
        };
        if !DOCUMENT_KINDS.contains(&kind.as_str()) {
            return fail(
                kind_line,
                format!(
                    "document kind {kind:?} is not registered ({})",
                    DOCUMENT_KINDS.join(", ")
                ),
            );
        }
        if !url.starts_with("https://") || url.ends_with('/') {
            return fail(url_line, "url must start with https:// and name a file");
        }
        if !is_url_basis(&url_basis) {
            return fail(
                basis_line,
                format!(
                    "url_basis {url_basis:?} is not registered ({}, or \"{URL_BASIS_FETCHED}YYYY-MM-DD\")",
                    URL_BASES.join(", ")
                ),
            );
        }
        if !is_date(&published_at) {
            return fail(
                date_line,
                format!("published_at {published_at:?} is not YYYY-MM-DD"),
            );
        }
        if !PUBLISHED_AT_BASES.iter().any(|p| {
            published_at_basis
                .strip_prefix(p)
                .is_some_and(|r| !r.trim().is_empty())
        }) {
            return fail(
                pub_basis_line,
                format!(
                    "published_at_basis must start with {} and cite its source",
                    PUBLISHED_AT_BASES.map(|p| format!("{p:?}")).join(" or ")
                ),
            );
        }
        if !is_lower_hex64(&sha256) {
            return fail(sha_line, "sha256 must be 64 lowercase hex characters");
        }
        if !digests.insert(sha256.clone()) {
            return fail(sha_line, format!("duplicate sha256 {sha256}"));
        }
        documents.push(DocumentRow {
            file,
            dir,
            kind,
            url,
            url_basis,
            published_at,
            published_at_basis,
            pages,
            bytes,
            sha256,
        });
    }
    let mut html_sources = Vec::new();
    for row in doc.rows_of("html") {
        let f = Fields::of_row(row);
        f.only(&[
            "file",
            "dir",
            "release",
            "url",
            "url_basis",
            "published_at",
            "published_at_basis",
            "bytes",
            "sha256",
        ])?;
        let (file, file_line) = f.string("file")?;
        if file.trim() != file
            || file.contains('/')
            || file.contains('\\')
            || file.starts_with('.')
            || file.chars().any(char::is_control)
            || !(file.ends_with(".html") || file.ends_with(".htm"))
        {
            return fail(
                file_line,
                "HTML file must be a bare .html or .htm file name",
            );
        }
        if !files.insert(file.clone()) {
            return fail(file_line, format!("duplicate file {file:?}"));
        }
        let dir = f
            .optional_string("dir")?
            .unwrap_or_else(|| corpus_dir.clone());
        if !is_plain_relative(&dir) || dir.chars().any(char::is_control) {
            return fail(row.line, "HTML dir must be a plain relative path");
        }
        let (release, release_line) = f.string("release")?;
        if !is_quarter(&release) {
            return fail(release_line, "HTML release must be YYYYQ1..YYYYQ4");
        }
        let (url, url_line) = f.string("url")?;
        if !url.starts_with("https://")
            || url.ends_with('/')
            || url.chars().any(char::is_whitespace)
        {
            return fail(
                url_line,
                "HTML url must be an https:// file URL without whitespace",
            );
        }
        let (url_basis, basis_line) = f.string("url_basis")?;
        if !is_url_basis(&url_basis) {
            return fail(basis_line, "HTML url_basis is not registered");
        }
        let (published_at, date_line) = f.string("published_at")?;
        if !is_date(&published_at) {
            return fail(date_line, "HTML published_at must be YYYY-MM-DD");
        }
        let (published_at_basis, pub_line) = f.string("published_at_basis")?;
        if published_at_basis.chars().any(char::is_control)
            || !PUBLISHED_AT_BASES.iter().any(|prefix| {
                published_at_basis
                    .strip_prefix(prefix)
                    .is_some_and(|citation| !citation.trim().is_empty())
            })
        {
            return fail(
                pub_line,
                "HTML published_at_basis must cite a source after FACT: or INFERENCE:",
            );
        }
        let (bytes, _) = f.positive_int("bytes")?;
        let (sha256, sha_line) = f.string("sha256")?;
        if !is_lower_hex64(&sha256) {
            return fail(sha_line, "sha256 must be 64 lowercase hex characters");
        }
        if !digests.insert(sha256.clone()) {
            return fail(sha_line, format!("duplicate sha256 {sha256}"));
        }
        html_sources.push(HtmlSourceRow {
            file,
            dir,
            release,
            url,
            url_basis,
            published_at,
            published_at_basis,
            bytes,
            sha256,
        });
    }
    if sources.is_empty() && documents.is_empty() && html_sources.is_empty() {
        return fail(0, "no [[source]], [[document]] or [[html]] rows");
    }

    let mut subjects = Vec::new();
    let mut regions = Vec::new();
    for row in doc.rows_of("subject") {
        let f = Fields::of_row(row);
        f.only(&["name", "kind"])?;
        let (name, line) = f.string("name")?;
        if subjects.contains(&name) {
            return fail(line, format!("duplicate subject {name:?}"));
        }
        match f.optional_string("kind")?.as_deref() {
            None | Some("site") => {}
            Some("region") => regions.push(name.clone()),
            Some(other) => {
                return fail(
                    row.line,
                    format!("subject kind {other:?} is not \"site\" or \"region\""),
                );
            }
        }
        subjects.push(name);
    }
    if subjects.is_empty() {
        return fail(0, "no [[subject]] rows");
    }

    let mut label_sets: Vec<(String, Vec<(String, String)>)> = Vec::new();
    for row in doc.rows_of("label_check") {
        let f = Fields::of_row(row);
        f.only(&["set", "cell", "prefix"])?;
        let (set, _) = f.string("set")?;
        let (cell, _) = f.string("cell")?;
        let (prefix, _) = f.string("prefix")?;
        match label_sets.iter_mut().find(|(s, _)| *s == set) {
            Some((_, checks)) => checks.push((cell, prefix)),
            None => label_sets.push((set, vec![(cell, prefix)])),
        }
    }

    let mut reconstructions = Vec::new();
    let mut used_sets = BTreeSet::new();
    for row in doc.rows_of("reconstruction") {
        let f = Fields::of_row(row);
        f.only(&[
            "evidence_id",
            "table_file",
            "sheet",
            "subject",
            "family",
            "component_file",
            "component_sheet",
            "component_cell",
            "label_checks",
            "rule",
            "basis",
            "quote_doc",
            "quote_locator",
            "quote_prefix",
        ])?;
        let quote = [
            f.optional_string("quote_doc")?,
            f.optional_string("quote_locator")?,
            f.optional_string("quote_prefix")?,
        ];
        let (basis_text, basis_line) = f.string("basis")?;
        let basis = match (basis_text.as_str(), quote) {
            ("reconstructed", [None, None, None]) => BasisRow::Reconstructed,
            ("stated", [Some(doc), Some(locator), Some(quote_prefix)]) => {
                if !files.contains(&doc) {
                    return fail(
                        basis_line,
                        format!("quote_doc {doc:?} is not a registered [[source]] file"),
                    );
                }
                if !locator.contains('!') || quote_prefix.is_empty() {
                    return fail(
                        basis_line,
                        "quote_locator must read sheet!cell and quote_prefix must not be empty",
                    );
                }
                BasisRow::Stated {
                    doc,
                    locator,
                    quote_prefix,
                }
            }
            ("reconstructed", _) => {
                return fail(
                    basis_line,
                    "a reconstructed basis takes no quote_doc, quote_locator or quote_prefix",
                );
            }
            ("stated", _) => {
                return fail(
                    basis_line,
                    "a stated basis needs quote_doc, quote_locator and quote_prefix",
                );
            }
            (other, _) => {
                return fail(
                    basis_line,
                    format!("basis {other:?} is not \"stated\" or \"reconstructed\""),
                );
            }
        };
        let (table_file, table_line) = f.string("table_file")?;
        let (component_file, component_line) = f.string("component_file")?;
        let (subject, subject_line) = f.string("subject")?;
        let (family, family_line) = f.string("family")?;
        let (set, set_line) = f.string("label_checks")?;
        for (file, line) in [(&table_file, table_line), (&component_file, component_line)] {
            if !files.contains(file) {
                return fail(
                    line,
                    format!("{file:?} is not a registered [[source]] file"),
                );
            }
        }
        if !subjects.contains(&subject) {
            return fail(
                subject_line,
                format!("subject {subject:?} is not a registered [[subject]]"),
            );
        }
        if !IDENTITY_FAMILIES.contains(&family.as_str()) {
            return fail(
                family_line,
                format!(
                    "family {family:?} is not registered ({})",
                    IDENTITY_FAMILIES.join(", ")
                ),
            );
        }
        let Some((_, checks)) = label_sets.iter().find(|(s, _)| *s == set) else {
            return fail(
                set_line,
                format!("label-check set {set:?} has no [[label_check]] rows"),
            );
        };
        used_sets.insert(set.clone());
        reconstructions.push(ReconstructionRow {
            evidence_id: f.string("evidence_id")?.0,
            table_file,
            sheet: f.string("sheet")?.0,
            subject,
            family,
            component_file,
            component_sheet: f.string("component_sheet")?.0,
            component_cell: f.string("component_cell")?.0,
            label_checks: checks.clone(),
            rule: f.string("rule")?.0,
            basis,
        });
    }
    if let Some((set, _)) = label_sets.iter().find(|(s, _)| !used_sets.contains(s)) {
        return fail(
            0,
            format!("label-check set {set:?} is used by no [[reconstruction]]"),
        );
    }

    let mut mine_stats_labels: Vec<MineStatsLabelRow> = Vec::new();
    for row in doc.rows_of("mine_stats_label") {
        let f = Fields::of_row(row);
        f.only(&["section", "label"])?;
        let entry = MineStatsLabelRow {
            section: f.optional_string("section")?,
            label: f.string("label")?.0,
        };
        if mine_stats_labels.contains(&entry) {
            return fail(
                row.line,
                format!("duplicate [[mine_stats_label]] {entry:?}"),
            );
        }
        mine_stats_labels.push(entry);
    }

    let mut shares: Vec<ShareRow> = Vec::new();
    for row in doc.rows_of("share") {
        let f = Fields::of_row(row);
        f.only(&["subject", "num", "den", "evidence", "label"])?;
        let (subject, subject_line) = f.string("subject")?;
        if !subjects.contains(&subject) {
            return fail(
                subject_line,
                format!("subject {subject:?} is not a registered [[subject]]"),
            );
        }
        if regions.contains(&subject) {
            return fail(
                subject_line,
                format!("{subject:?} is a region: a share is registered for a site"),
            );
        }
        if shares.iter().any(|s| s.subject == subject) {
            return fail(subject_line, format!("duplicate [[share]] for {subject:?}"));
        }
        let (num, _) = f.positive_int("num")?;
        let (den, den_line) = f.positive_int("den")?;
        if num > den {
            return fail(den_line, format!("share {num}/{den} exceeds 1"));
        }
        shares.push(ShareRow {
            subject,
            num,
            den,
            evidence: f.string("evidence")?.0,
            label: f.optional_string("label")?,
        });
    }

    let mut missing_facts: Vec<MissingFactRow> = Vec::new();
    for row in doc.rows_of("missing_fact") {
        let f = Fields::of_row(row);
        f.only(&["subject", "family", "text", "evidence", "closing_document"])?;
        let (subject, subject_line) = f.string("subject")?;
        let (family, family_line) = f.string("family")?;
        let (text, text_line) = f.string("text")?;
        if !subjects.contains(&subject) {
            return fail(
                subject_line,
                format!("subject {subject:?} is not a registered [[subject]]"),
            );
        }
        if !IDENTITY_FAMILIES.contains(&family.as_str()) {
            return fail(
                family_line,
                format!(
                    "family {family:?} is not registered ({})",
                    IDENTITY_FAMILIES.join(", ")
                ),
            );
        }
        if missing_facts
            .iter()
            .any(|m| m.subject == subject && m.family == family)
        {
            return fail(
                subject_line,
                format!("duplicate [[missing_fact]] for {subject:?} {family}"),
            );
        }
        if text.is_empty() || text.bytes().any(|b| b.is_ascii_digit()) {
            return fail(
                text_line,
                "missing-fact text must be prose with no digits; put the source in evidence",
            );
        }
        missing_facts.push(MissingFactRow {
            subject,
            family,
            text,
            evidence: f.string("evidence")?.0,
            closing_document: f.optional_string("closing_document")?,
        });
    }

    let mut subject_statuses: Vec<SubjectStatusRow> = Vec::new();
    for row in doc.rows_of("subject_status") {
        let f = Fields::of_row(row);
        f.only(&["subject", "status", "closed", "first", "last", "evidence"])?;
        let (subject, subject_line) = f.string("subject")?;
        let (status, status_line) = f.string("status")?;
        if !subjects.contains(&subject) {
            return fail(
                subject_line,
                format!("subject {subject:?} is not a registered [[subject]]"),
            );
        }
        if regions.contains(&subject) {
            return fail(
                subject_line,
                format!("{subject:?} is a region: a status is registered for a site"),
            );
        }
        if subject_statuses.iter().any(|s| s.subject == subject) {
            return fail(
                subject_line,
                format!("duplicate [[subject_status]] for {subject:?}"),
            );
        }
        if !SUBJECT_STATUSES.contains(&status.as_str()) {
            return fail(
                status_line,
                format!(
                    "status {status:?} is not registered ({})",
                    SUBJECT_STATUSES.join(", ")
                ),
            );
        }
        let (closed, first, last) = (
            f.optional_string("closed")?,
            f.optional_string("first")?,
            f.optional_string("last")?,
        );
        let disposed = status == "disposed";
        if disposed != closed.is_some()
            || disposed == first.is_some()
            || (disposed && last.is_some())
        {
            return fail(
                status_line,
                "a disposal states closed only; any other status states first and, optionally, last",
            );
        }
        if closed.as_deref().is_some_and(|d| !is_date(d)) {
            return fail(row.line, "closed must be a date YYYY-MM-DD");
        }
        if [&first, &last]
            .into_iter()
            .flatten()
            .any(|q| !is_quarter(q))
        {
            return fail(row.line, "first and last must be quarters YYYYQ1..YYYYQ4");
        }
        if let (Some(a), Some(b)) = (&first, &last)
            && b < a
        {
            return fail(row.line, format!("last {b} is before first {a}"));
        }
        subject_statuses.push(SubjectStatusRow {
            subject,
            status,
            closed,
            first,
            last,
            evidence: f.string("evidence")?.0,
        });
    }

    let mut region_members: Vec<RegionMemberRow> = Vec::new();
    for row in doc.rows_of("region_member") {
        let f = Fields::of_row(row);
        f.only(&["region", "member", "first", "last"])?;
        let (region, region_line) = f.string("region")?;
        let (member, member_line) = f.string("member")?;
        if !regions.contains(&region) {
            return fail(
                region_line,
                format!("{region:?} is not a registered [[subject]] with kind = \"region\""),
            );
        }
        if !subjects.contains(&member) || regions.contains(&member) {
            return fail(
                member_line,
                format!("member {member:?} is not a registered site"),
            );
        }
        let row_member = RegionMemberRow {
            region,
            member,
            first: f.string("first")?.0,
            last: f.optional_string("last")?,
        };
        if [Some(&row_member.first), row_member.last.as_ref()]
            .into_iter()
            .flatten()
            .any(|q| !is_quarter(q))
        {
            return fail(row.line, "first and last must be quarters YYYYQ1..YYYYQ4");
        }
        if let Some(last) = &row_member.last
            && *last < row_member.first
        {
            return fail(
                row.line,
                format!("last {last} is before first {}", row_member.first),
            );
        }
        if let Some(other) = region_members
            .iter()
            .find(|m| m.member == row_member.member && m.overlaps(&row_member))
        {
            return fail(
                member_line,
                format!(
                    "{:?} is in {:?} and {:?} for overlapping releases",
                    row_member.member, other.region, row_member.region
                ),
            );
        }
        region_members.push(row_member);
    }

    let mut printed_share_corrections: Vec<PrintedShareCorrection> = Vec::new();
    for row in doc.rows_of("printed_share_correction") {
        let f = Fields::of_row(row);
        f.only(&["subject", "printed", "num", "den", "evidence"])?;
        let (subject, subject_line) = f.string("subject")?;
        let (printed, printed_line) = f.string("printed")?;
        if !subjects.contains(&subject) || regions.contains(&subject) {
            return fail(
                subject_line,
                format!("{subject:?} is not a registered site"),
            );
        }
        let decimal = |s: &str| {
            let (int, frac) = s.split_once('.').unwrap_or((s, "0"));
            !int.is_empty()
                && !frac.is_empty()
                && int.bytes().chain(frac.bytes()).all(|b| b.is_ascii_digit())
        };
        if !decimal(&printed) {
            return fail(
                printed_line,
                format!("printed {printed:?} must be a percentage as printed, e.g. \"47.5\""),
            );
        }
        if printed_share_corrections
            .iter()
            .any(|c| c.subject == subject && c.printed == printed)
        {
            return fail(
                subject_line,
                format!("duplicate [[printed_share_correction]] for {subject:?} at {printed}%"),
            );
        }
        let (num, _) = f.positive_int("num")?;
        let (den, den_line) = f.positive_int("den")?;
        if num > den {
            return fail(den_line, format!("share {num}/{den} exceeds 1"));
        }
        printed_share_corrections.push(PrintedShareCorrection {
            subject,
            printed,
            num,
            den,
            evidence: f.string("evidence")?.0,
        });
    }

    let mut mine_stats_label_aliases: Vec<MineStatsLabelAlias> = Vec::new();
    for row in doc.rows_of("mine_stats_label_alias") {
        let f = Fields::of_row(row);
        f.only(&["label", "reads_as", "first", "last", "evidence"])?;
        let (label, label_line) = f.string("label")?;
        let (reads_as, reads_line) = f.string("reads_as")?;
        for (l, line) in [(&label, label_line), (&reads_as, reads_line)] {
            if !mine_stats_labels.iter().any(|m| m.label == *l) {
                return fail(
                    line,
                    format!("{l:?} is not a registered [[mine_stats_label]]"),
                );
            }
        }
        if label == reads_as {
            return fail(reads_line, format!("{label:?} is read as itself"));
        }
        let (first, last) = (f.string("first")?.0, f.string("last")?.0);
        if !is_quarter(&first) || !is_quarter(&last) {
            return fail(row.line, "first and last must be quarters YYYYQ1..YYYYQ4");
        }
        if last < first {
            return fail(row.line, format!("last {last} is before first {first}"));
        }
        if mine_stats_label_aliases
            .iter()
            .any(|a| a.label == label && a.first <= last && first <= a.last)
        {
            return fail(
                label_line,
                format!(
                    "{label:?} has two [[mine_stats_label_alias]] rows for overlapping releases"
                ),
            );
        }
        mine_stats_label_aliases.push(MineStatsLabelAlias {
            label,
            reads_as,
            first,
            last,
            evidence: f.string("evidence")?.0,
        });
    }

    let (restatement_notes, restatement_scopes) = parse_restatements(&doc, &sources, &subjects)?;
    Ok(SourcesRegistry {
        issuer,
        corpus_dir,
        retrieved_utc,
        sources,
        subjects,
        regions,
        shares,
        reconstructions,
        mine_stats_labels,
        missing_facts,
        subject_statuses,
        region_members,
        printed_share_corrections,
        mine_stats_label_aliases,
        documents,
        html_sources,
        restatement_notes,
        restatement_scopes,
    })
}

/// Reads and checks `<registries_dir>/sources/<issuer>.toml`; the file's `issuer` must match its name.
pub fn load_sources_registry(path: &Path) -> Result<SourcesRegistry, SourcesError> {
    let text = std::fs::read_to_string(path).map_err(|e| SourcesError {
        line: 0,
        reason: format!("cannot read {}: {e}", path.display()),
    })?;
    let reg = parse_sources_registry(&text)?;
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or_default();
    if reg.issuer != stem {
        return fail(
            0,
            format!(
                "issuer {:?} does not match the file name {stem:?}",
                reg.issuer
            ),
        );
    }
    Ok(reg)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SHA_A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const SHA_B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

    fn minimal(source_b: &str, extra: &str) -> String {
        format!(
            "schema = \"fmine.sources.v1\"\nissuer = \"demo\"\ncorpus_dir = \"corpus/demo\"\n\
             retrieved_utc = \"2030-01-01T00:00:00Z\"\nurl_prefix = \"https://example.org/files/\"\n\n\
             [[source]]\nfile = \"A.xlsx\"\nkind = \"mine_stats\"\nquarter = \"2030Q1\"\npath = \"q1/A.xlsx\"\n\
             url_basis = \"W1 table\"\nbytes = 10\nsha256 = \"{SHA_A}\"\n\n{source_b}\n\
             [[subject]]\nname = \"Site A\"\n{extra}"
        )
    }

    fn source_b(kind: &str, sha: &str) -> String {
        format!(
            "[[source]]\nfile = \"B.xlsx\"\nkind = \"{kind}\"\nquarter = \"2030Q2\"\npath = \"q2/B.xlsx\"\n\
             url_basis = \"W1 naming rule\"\nbytes = 20\nsha256 = \"{sha}\"\n"
        )
    }

    fn refusal(text: &str) -> SourcesError {
        match parse_sources_registry(text) {
            Ok(r) => panic!("accepted: {r:?}"),
            Err(e) => e,
        }
    }

    const RESTATEMENT_NOTE: &str = "\n[[restatement_note]]\nid = \"N1\"\nfile_name = \"B.xlsx\"\n\
        locator = \"Q2 2030!A40\"\nquoted_text = \"Comparative information has been restated.\"\n\
        from_quarter = \"2030Q2\"\nevidence = \"synthetic note\"\n";
    const RESTATEMENT_SCOPE: &str = "\n[[restatement_scope]]\nnote = \"N1\"\nsubject = \"Site A\"\n\
        period_from = \"2029Q1\"\nperiod_to = \"2030Q2\"\nline = \"summary:aisc_usd_per_oz\"\n";

    fn restatement_registry(extra: &str) -> String {
        minimal(&source_b("aisc_reconciliation", SHA_B), extra)
    }

    #[test]
    fn test_restatement_registry_keeps_exact_scope_and_derives_source_digest() {
        let old = parse_sources_registry(&restatement_registry("")).expect("old registry");
        assert!(old.restatement_notes.is_empty() && old.restatement_scopes.is_empty());
        let text = format!("{RESTATEMENT_NOTE}{RESTATEMENT_SCOPE}");
        let reg = parse_sources_registry(&restatement_registry(&text)).expect("disclosure");
        assert_eq!(
            reg.restatement_notes,
            [RestatementNoteRow {
                id: "N1".into(),
                file_name: "B.xlsx".into(),
                locator: "Q2 2030!A40".into(),
                quoted_text: "Comparative information has been restated.".into(),
                from_quarter: "2030Q2".into(),
                evidence: "synthetic note".into(),
                sha256: SHA_B.into(),
            }]
        );
        assert_eq!(
            reg.restatement_scopes,
            [RestatementScopeRow {
                note: "N1".into(),
                subject: "Site A".into(),
                period_from: "2029Q1".into(),
                period_to: "2030Q2".into(),
                line: "summary:aisc_usd_per_oz".into(),
                occurrence: 0,
            }]
        );
        // An explicit occurrence selects another canonical row; it is never treated as a wildcard.
        let text = format!("{text}{RESTATEMENT_SCOPE}occurrence = 1\n");
        let reg = parse_sources_registry(&restatement_registry(&text)).expect("second occurrence");
        assert_eq!(
            reg.restatement_scopes
                .iter()
                .map(|s| s.occurrence)
                .collect::<Vec<_>>(),
            [0, 1]
        );
    }

    #[test]
    fn test_restatement_registry_rejects_malformed_and_unbound_rows() {
        let valid = format!("{RESTATEMENT_NOTE}{RESTATEMENT_SCOPE}");
        for (from, to, reason) in [
            ("id = \"N1\"", "id = \"*\"", "note id"),
            (
                "file_name = \"B.xlsx\"",
                "file_name = \"unknown.xlsx\"",
                "registered [[source]]",
            ),
            ("locator = \"Q2 2030!A40\"", "locator = \"A40\"", "sheet!A1"),
            (
                "quoted_text = \"Comparative information has been restated.\"",
                "quoted_text = \"   \"",
                "quoted_text",
            ),
            (
                "from_quarter = \"2030Q2\"",
                "from_quarter = \"2030Q1\"",
                "source release",
            ),
            (
                "from_quarter = \"2030Q2\"",
                "from_quarter = \"2030Q5\"",
                "from_quarter",
            ),
            (
                "evidence = \"synthetic note\"",
                "evidence = \" \"",
                "evidence",
            ),
            ("note = \"N1\"", "note = \"unknown\"", "scope note"),
            ("subject = \"Site A\"", "subject = \"*\"", "subject"),
            (
                "period_from = \"2029Q1\"",
                "period_from = \"2029H1\"",
                "period_from",
            ),
            (
                "period_to = \"2030Q2\"",
                "period_to = \"2028Q4\"",
                "period_to",
            ),
            (
                "period_to = \"2030Q2\"",
                "period_to = \"2030Q0\"",
                "period_to",
            ),
            (
                "line = \"summary:aisc_usd_per_oz\"",
                "line = \"summary:*\"",
                "exact",
            ),
            (
                "line = \"summary:aisc_usd_per_oz\"",
                "line = \" \"",
                "exact",
            ),
        ] {
            let e = refusal(&restatement_registry(&valid.replacen(from, to, 1)));
            assert!(e.line > 0 && e.reason.contains(reason), "{from}: {e}");
        }
        for occurrence in ["-1", "4294967296", "\"0\""] {
            let e = refusal(&restatement_registry(&format!(
                "{valid}occurrence = {occurrence}\n"
            )));
            assert!(e.reason.contains("occurrence"), "{e}");
        }
        for locator in [
            "!A1",
            "QTD!",
            "QTD!A0",
            "QTD!A01",
            "QTD!a1",
            "QTD!A1:B2",
            "QTD!XFE1",
            "QTD!A1048577",
            "QTD!A1!B2",
            "QTD!$A$1",
            "QTD\t!A1",
        ] {
            assert!(!is_single_cell_locator(locator), "{locator}");
        }
        assert!(is_single_cell_locator("QTD!XFD1048576"));
        assert!(
            refusal(&restatement_registry(&format!("{valid}unknown = 1\n")))
                .reason
                .contains("unknown key")
        );
        let extra_digest =
            RESTATEMENT_NOTE.to_string() + "sha256 = \"unbound\"\n" + RESTATEMENT_SCOPE;
        assert!(
            refusal(&restatement_registry(&extra_digest))
                .reason
                .contains("unknown key")
        );
    }

    #[test]
    fn test_restatement_registry_rejects_missing_duplicate_and_unused_rows() {
        for (row, other, keys) in [
            (
                RESTATEMENT_NOTE,
                RESTATEMENT_SCOPE,
                &[
                    "id",
                    "file_name",
                    "locator",
                    "quoted_text",
                    "from_quarter",
                    "evidence",
                ][..],
            ),
            (
                RESTATEMENT_SCOPE,
                RESTATEMENT_NOTE,
                &["note", "subject", "period_from", "period_to", "line"][..],
            ),
        ] {
            for key in keys {
                let without: String = row
                    .lines()
                    .filter(|l| !l.starts_with(&format!("{key} = ")))
                    .map(|l| format!("{l}\n"))
                    .collect();
                let e = refusal(&restatement_registry(&format!("{other}{without}")));
                assert!(e.reason.contains(&format!("no {key}")), "{key}: {e}");
            }
        }
        let no_note = refusal(&restatement_registry(RESTATEMENT_SCOPE));
        assert!(no_note.reason.contains("scope note"), "{no_note}");
        let unused = refusal(&restatement_registry(RESTATEMENT_NOTE));
        assert!(
            unused.reason.contains("unused restatement note"),
            "{unused}"
        );
        let valid = format!("{RESTATEMENT_NOTE}{RESTATEMENT_SCOPE}");
        for (extra, reason) in [
            (RESTATEMENT_NOTE.to_string(), "duplicate restatement note"),
            (
                RESTATEMENT_NOTE.replace("N1", "N2"),
                "duplicate restatement note source cell",
            ),
            (RESTATEMENT_SCOPE.to_string(), "duplicate or overlapping"),
            (
                RESTATEMENT_SCOPE
                    .replace("2029Q1", "2030Q1")
                    .replace("2030Q2", "2030Q3"),
                "overlapping",
            ),
        ] {
            let e = refusal(&restatement_registry(&format!("{valid}{extra}")));
            assert!(e.reason.contains(reason), "{e}");
        }
    }

    /// FACT E461: pinned from the independently read, SHA-256-checked Q4 2025 workbook, QTD!A33.
    #[test]
    fn test_barrick_e461_restatement_registry_golden() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let reg =
            load_sources_registry(&root.join("registries/sources/barrick.toml")).expect("registry");
        assert_eq!(reg.restatement_notes.len(), 1);
        let note = &reg.restatement_notes[0];
        assert_eq!(
            (
                &*note.id,
                &*note.file_name,
                &*note.locator,
                &*note.from_quarter,
                &*note.evidence
            ),
            (
                "E461",
                "Barrick_Q4_2025_Mine_Stats.xlsx",
                "QTD!A33",
                "2025Q4",
                "E461"
            )
        );
        assert_eq!(
            note.sha256,
            "94f6692acfa5f133b97ae028f172cc9c5e7527945788da7bd9a06ffd11c7eec4"
        );
        assert_eq!(
            note.quoted_text,
            "4 Starting Q4 2025, we have presented Pueblo Viejo as part of North America instead of South America & Asia Pacific. Comparative information has been restated."
        );
        let lines = [
            "summary:au_prod_100_koz",
            "summary:au_prod_attr_koz",
            "summary:au_sold_attr_koz",
            "summary:cos_usd_per_oz",
            "summary:tcc_usd_per_oz",
            "summary:aisc_usd_per_oz",
        ];
        let mut want = Vec::new();
        for subject in [
            "North America",
            "Latin America & Asia Pacific",
            "South America & Asia Pacific",
        ] {
            for line in lines {
                want.push(RestatementScopeRow {
                    note: "E461".into(),
                    subject: subject.into(),
                    period_from: "2024Q1".into(),
                    period_to: "2025Q4".into(),
                    line: line.into(),
                    occurrence: 0,
                });
            }
        }
        assert_eq!(reg.restatement_scopes, want);
    }

    #[test]
    fn test_registry_row_order_preserved() {
        let reg = parse_sources_registry(&minimal(&source_b("aisc_reconciliation", SHA_B), ""))
            .expect("valid");
        let files: Vec<&str> = reg.sources.iter().map(|s| s.file.as_str()).collect();
        assert_eq!(files, ["A.xlsx", "B.xlsx"]);
        assert_eq!(reg.sources[1].url, "https://example.org/files/q2/B.xlsx");
        assert_eq!(reg.sources[1].bytes, 20);
    }

    #[test]
    fn test_registry_check_rejects_duplicate_sha256() {
        let e = refusal(&minimal(&source_b("aisc_reconciliation", SHA_A), ""));
        assert!(e.reason.contains("duplicate sha256"), "{e}");
        assert_eq!(e.line, 23);
    }

    #[test]
    fn test_registry_check_rejects_bad_hex() {
        let upper = SHA_B.to_uppercase();
        assert!(
            refusal(&minimal(&source_b("aisc_reconciliation", &upper), ""))
                .reason
                .contains("lowercase hex")
        );
        assert!(
            refusal(&minimal(&source_b("aisc_reconciliation", "abc"), ""))
                .reason
                .contains("lowercase hex")
        );
    }

    #[test]
    fn test_registry_check_rejects_unknown_kind() {
        let e = refusal(&minimal(&source_b("annual_report", SHA_B), ""));
        assert!(
            e.reason
                .contains("kind \"annual_report\" is not registered"),
            "{e}"
        );
    }

    #[test]
    fn test_registry_check_rejects_unregistered_reconstruction_file() {
        let recon = "\n[[reconstruction]]\nevidence_id = \"E1\"\ntable_file = \"C.xlsx\"\nsheet = \"Q1\"\n\
                     subject = \"Site A\"\nfamily = \"aisc_per_oz\"\ncomponent_file = \"A.xlsx\"\n\
                     component_sheet = \"S\"\ncomponent_cell = \"B2\"\nlabel_checks = \"set1\"\nrule = \"r\"\n\
                     basis = \"reconstructed\"\n\
                     [[label_check]]\nset = \"set1\"\ncell = \"A1\"\nprefix = \"x\"\n";
        let e = refusal(&minimal(&source_b("aisc_reconciliation", SHA_B), recon));
        assert!(e.reason.contains("\"C.xlsx\" is not a registered"), "{e}");
        // The same row against a registered file is accepted and resolves its label checks.
        let ok = recon.replace("C.xlsx", "B.xlsx");
        let reg = parse_sources_registry(&minimal(&source_b("aisc_reconciliation", SHA_B), &ok))
            .expect("valid");
        assert_eq!(
            reg.reconstructions[0].label_checks,
            vec![("A1".to_string(), "x".to_string())]
        );
        // An unknown family, subject or label set is refused too.
        for (from, to, needle) in [
            ("aisc_per_oz", "aisc_per_tonne", "family"),
            ("subject = \"Site A\"", "subject = \"Site Z\"", "subject"),
            (
                "label_checks = \"set1\"",
                "label_checks = \"set2\"",
                "label-check set",
            ),
        ] {
            let bad = ok.replacen(from, to, 1);
            let e = refusal(&minimal(&source_b("aisc_reconciliation", SHA_B), &bad));
            assert!(e.reason.contains(needle), "{needle}: {e}");
        }
    }

    /// BD-331.S2: every reconstruction declares its basis; a stated one names the quote it re-reads.
    #[test]
    fn test_reconstruction_basis_parse_and_refuse() {
        let recon = "\n[[reconstruction]]\nevidence_id = \"E1\"\ntable_file = \"B.xlsx\"\nsheet = \"Q1\"\n\
                     subject = \"Site A\"\nfamily = \"aisc_per_oz\"\ncomponent_file = \"A.xlsx\"\n\
                     component_sheet = \"S\"\ncomponent_cell = \"B2\"\nlabel_checks = \"set1\"\nrule = \"r\"\n\
                     basis = \"reconstructed\"\n\
                     [[label_check]]\nset = \"set1\"\ncell = \"A1\"\nprefix = \"x\"\n";
        let b = source_b("aisc_reconciliation", SHA_B);
        let parse = |text: &str| parse_sources_registry(&minimal(&b, text));
        let reg = parse(recon).expect("valid");
        assert_eq!(reg.reconstructions[0].basis, BasisRow::Reconstructed);
        let quote =
            "quote_doc = \"B.xlsx\"\nquote_locator = \"Q1!A40\"\nquote_prefix = \"Includes\"\n";
        let stated = recon.replace(
            "basis = \"reconstructed\"\n",
            &format!("basis = \"stated\"\n{quote}"),
        );
        assert_eq!(
            parse(&stated).expect("valid").reconstructions[0].basis,
            BasisRow::Stated {
                doc: "B.xlsx".into(),
                locator: "Q1!A40".into(),
                quote_prefix: "Includes".into()
            }
        );
        for (bad, why) in [
            (
                recon.replace("basis = \"reconstructed\"\n", ""),
                "has no basis",
            ),
            (
                recon.replace("\"reconstructed\"", "\"guessed\""),
                "is not \"stated\" or \"reconstructed\"",
            ),
            (
                recon.replace("basis = \"reconstructed\"\n", "basis = \"stated\"\n"),
                "needs quote_doc",
            ),
            (
                recon.replace(
                    "basis = \"reconstructed\"\n",
                    &format!("basis = \"reconstructed\"\n{quote}"),
                ),
                "takes no quote_doc",
            ),
            (
                stated.replace("quote_doc = \"B.xlsx\"", "quote_doc = \"C.xlsx\""),
                "quote_doc",
            ),
            (stated.replace("Q1!A40", "A40"), "sheet!cell"),
        ] {
            let e = refusal(&minimal(&b, &bad));
            assert!(e.reason.contains(why) && e.line > 0, "{why}: {e}");
        }
    }

    /// BD-331.S2: `[[missing_fact]]` rows keyed by (subject, family); the text is prose with no digits.
    #[test]
    fn test_missing_fact_has_no_digits() {
        let b = source_b("aisc_reconciliation", SHA_B);
        let row = "[[missing_fact]]\nsubject = \"Site A\"\nfamily = \"cos_per_oz\"\n\
                   text = \"the ounce series behind the per-ounce rows\"\nevidence = \"E691\"\n";
        let reg = parse_sources_registry(&minimal(&b, row)).expect("accepted");
        assert_eq!(
            reg.missing_facts,
            vec![MissingFactRow {
                subject: "Site A".into(),
                family: "cos_per_oz".into(),
                text: "the ounce series behind the per-ounce rows".into(),
                evidence: "E691".into(),
                closing_document: None,
            }]
        );
        for (bad, why) in [
            (
                row.replace("per-ounce rows", "per-ounce rows of Q4 2025"),
                "no digits",
            ),
            (
                row.replace("Site A", "Site Z"),
                "not a registered [[subject]]",
            ),
            (row.replace("cos_per_oz", "cos_per_t"), "family"),
            (format!("{row}\n{row}"), "duplicate [[missing_fact]]"),
            (row.replace("evidence = \"E691\"\n", ""), "has no evidence"),
        ] {
            let e = refusal(&minimal(&b, &bad));
            assert!(e.reason.contains(why) && e.line > 0, "{why}: {e}");
        }
    }

    #[test]
    fn workbook_publication_requires_a_cited_date_pair_without_retrieval_inference() {
        let old = parse_sources_registry(&minimal("", "")).expect("undated registry");
        assert_eq!(old.sources[0].published_at, None);
        assert_eq!(old.sources[0].published_at_basis, None);
        let base = source_b("mine_stats", SHA_B);
        let dated = format!(
            "{base}published_at = \"2030-08-10\"\npublished_at_basis = \"INFERENCE: synthetic release page\"\n"
        );
        let parsed = parse_sources_registry(&minimal(&dated, "")).expect("cited date");
        assert_eq!(
            parsed.sources[1].published_at.as_deref(),
            Some("2030-08-10")
        );
        assert_eq!(
            parsed.sources[1].published_at_basis.as_deref(),
            Some("INFERENCE: synthetic release page")
        );
        assert_eq!(parsed.sources[0].published_at, None);
        for (fields, reason) in [
            ("published_at = \"2030-08-10\"\n", "supplied together"),
            (
                "published_at_basis = \"FACT: synthetic index\"\n",
                "supplied together",
            ),
            (
                "published_at = \"2030-02-29\"\npublished_at_basis = \"FACT: index\"\n",
                "YYYY-MM-DD",
            ),
            (
                "published_at = \"2030-08-10\"\npublished_at_basis = \"retrieved\"\n",
                "cite its source",
            ),
            (
                "published_at = \"2030-08-10\"\npublished_at_basis = \"FACT: \"\n",
                "cite its source",
            ),
            (
                "published_at = \"2030-08-10\"\npublished_at_basis = 1\n",
                "string",
            ),
        ] {
            let error = refusal(&minimal(&format!("{base}{fields}"), ""));
            assert!(error.reason.contains(reason), "{fields}: {error}");
        }
    }

    /// BD-339.S3: a URL basis is one of the two W1 bases or a dated fetch; anything else refuses.
    #[test]
    fn test_url_basis_values_are_registered() {
        for ok in [
            "W1 table",
            "W1 naming rule",
            "fetched and verified 2026-10-30",
            "fetched and verified 2028-02-29",
        ] {
            assert!(is_url_basis(ok), "{ok}");
        }
        for bad in [
            "fetched and verified",
            "fetched and verified 2026-13-01",
            "fetched and verified 2026-02-29",
            "fetched and verified 2026-04-31",
            "fetched and verified 2026-10-30 by hand",
            "fetched and verified 26-10-30",
            "verified 2026-10-30",
            "W1 Table",
        ] {
            assert!(!is_url_basis(bad), "{bad}");
        }
        let b = source_b("aisc_reconciliation", SHA_B);
        let fetched = minimal(
            &b.replacen("W1 naming rule", "fetched and verified 2026-10-30", 1),
            "",
        );
        let reg = parse_sources_registry(&fetched).expect("dated basis accepted");
        assert!(
            reg.sources
                .iter()
                .any(|s| s.url_basis == "fetched and verified 2026-10-30")
        );
        let e = refusal(&minimal(
            &b.replacen("W1 naming rule", "fetched and verified 2026-02-30", 1),
            "",
        ));
        assert!(e.reason.contains("url_basis"), "{e}");
    }

    /// The real registry: every row's basis is registered (default CI, no corpus).
    #[test]
    fn test_barrick_url_bases_are_registered() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let text = std::fs::read_to_string(root.join("registries/sources/barrick.toml"))
            .expect("barrick registry");
        let reg = parse_sources_registry(&text).expect("barrick registry parses");
        assert!(reg.sources.iter().all(|s| is_url_basis(&s.url_basis)));
    }

    #[test]
    fn test_registry_check_rejects_bad_quarter_path_basis_and_unknown_keys() {
        let b = source_b("aisc_reconciliation", SHA_B);
        for (from, to, needle) in [
            ("2030Q2", "2030Q5", "quarter"),
            ("q2/B.xlsx", "../B.xlsx", "path"),
            ("W1 naming rule", "guess", "url_basis"),
            ("bytes = 20", "bytes = 0", "positive integer"),
            ("bytes = 20", "bytes = 20\nbites = 3", "unknown key"),
        ] {
            let e = refusal(&minimal(&b.replacen(from, to, 1), ""));
            assert!(e.reason.contains(needle), "{needle}: {e}");
        }
        assert!(
            refusal(&minimal(&b, "").replace("fmine.sources.v1", "v0"))
                .reason
                .contains("schema")
        );
        assert!(
            refusal(&minimal(&b, "[[extra]]\nx = 1\n"))
                .reason
                .contains("unknown table")
        );
    }

    #[test]
    fn test_mine_stats_labels_parse_in_order_and_refuse_duplicates() {
        let b = source_b("aisc_reconciliation", SHA_B);
        let rows = "[[mine_stats_label]]\nlabel = \"Tonnes processed (thousands)\"\n\n\
                    [[mine_stats_label]]\nsection = \"Average grade (grams per tonne)\"\nlabel = \"Processed\"\n";
        let reg = parse_sources_registry(&minimal(&b, rows)).expect("accepted");
        assert_eq!(
            reg.mine_stats_labels,
            vec![
                MineStatsLabelRow {
                    section: None,
                    label: "Tonnes processed (thousands)".into()
                },
                MineStatsLabelRow {
                    section: Some("Average grade (grams per tonne)".into()),
                    label: "Processed".into()
                },
            ]
        );
        assert!(
            parse_sources_registry(&minimal(&b, ""))
                .expect("accepted")
                .mine_stats_labels
                .is_empty()
        );
        let dup =
            format!("{rows}\n[[mine_stats_label]]\nlabel = \"Tonnes processed (thousands)\"\n");
        let e = refusal(&minimal(&b, &dup));
        assert!(
            e.reason.contains("duplicate [[mine_stats_label]]") && e.line > 0,
            "{e}"
        );
        let e = refusal(&minimal(
            &b,
            "[[mine_stats_label]]\nlabel = \"X\"\nunit = \"kt\"\n",
        ));
        assert!(e.reason.contains("unknown key"), "{e}");
        let e = refusal(&minimal(
            &b,
            "[[mine_stats_label]]\nsection = \"\"\nlabel = \"X\"\n",
        ));
        assert!(e.reason.contains("non-empty"), "{e}");
    }

    /// BD-331.S3: `[[share]]` rows for sites and `kind = "region"` subjects.
    #[test]
    fn test_shares_and_regions_parse_and_refuse() {
        let b = source_b("aisc_reconciliation", SHA_B);
        let rows = "[[subject]]\nname = \"Region North\"\nkind = \"region\"\n\n\
                    [[share]]\nsubject = \"Site A\"\nnum = 123\nden = 200\nevidence = \"E457\"\n\
                    label = \"Site A LLC\"\n";
        let reg = parse_sources_registry(&minimal(&b, rows)).expect("accepted");
        assert_eq!(reg.regions, ["Region North"]);
        assert_eq!(
            reg.shares,
            vec![ShareRow {
                subject: "Site A".into(),
                num: 123,
                den: 200,
                evidence: "E457".into(),
                label: Some("Site A LLC".into()),
            }]
        );
        let plain = parse_sources_registry(&minimal(&b, "")).expect("accepted");
        assert!(plain.regions.is_empty() && plain.shares.is_empty());

        for (extra, why) in [
            (
                "[[share]]\nsubject = \"Site Z\"\nnum = 1\nden = 2\nevidence = \"E1\"\n",
                "not a registered [[subject]]",
            ),
            (
                "[[subject]]\nname = \"R\"\nkind = \"region\"\n\n\
                 [[share]]\nsubject = \"R\"\nnum = 1\nden = 2\nevidence = \"E1\"\n",
                "is a region",
            ),
            (
                "[[share]]\nsubject = \"Site A\"\nnum = 3\nden = 2\nevidence = \"E1\"\n",
                "exceeds 1",
            ),
            (
                "[[share]]\nsubject = \"Site A\"\nnum = 0\nden = 2\nevidence = \"E1\"\n",
                "positive integer",
            ),
            (
                "[[share]]\nsubject = \"Site A\"\nnum = 1\nden = 2\nevidence = \"E1\"\n\n\
                 [[share]]\nsubject = \"Site A\"\nnum = 1\nden = 2\nevidence = \"E2\"\n",
                "duplicate [[share]]",
            ),
            (
                "[[share]]\nsubject = \"Site A\"\nnum = 1\nden = 2\n",
                "has no evidence",
            ),
            (
                "[[subject]]\nname = \"R\"\nkind = \"segment\"\n",
                "is not \"site\" or \"region\"",
            ),
        ] {
            let e = refusal(&minimal(&b, extra));
            assert!(e.reason.contains(why) && e.line > 0, "{why}: {e}");
        }
    }

    /// BD-331.S4: `[[region_member]]` rows, keyed by release. A site moves between regions only across releases;
    /// every malformed or overlapping row refuses at its line.
    #[test]
    fn test_region_members_parse_and_refuse() {
        let b = source_b("aisc_reconciliation", SHA_B);
        let head = "[[subject]]\nname = \"Site B\"\n\n\
                    [[subject]]\nname = \"North\"\nkind = \"region\"\n\n\
                    [[subject]]\nname = \"South\"\nkind = \"region\"\n\n";
        let member = |region: &str, site: &str, first: &str, last: Option<&str>| {
            let last = last
                .map(|l| format!("last = \"{l}\"\n"))
                .unwrap_or_default();
            format!(
                "[[region_member]]\nregion = \"{region}\"\nmember = \"{site}\"\nfirst = \"{first}\"\n{last}\n"
            )
        };
        let rows = format!(
            "{head}{}{}{}",
            member("North", "Site A", "2030Q1", None),
            member("North", "Site B", "2030Q1", Some("2030Q2")),
            member("South", "Site B", "2030Q3", None),
        );
        let reg = parse_sources_registry(&minimal(&b, &rows)).expect("accepted");
        assert_eq!(reg.region_members.len(), 3);
        assert_eq!(reg.members_of("North", "2030Q2"), ["Site A", "Site B"]);
        assert_eq!(reg.members_of("North", "2030Q3"), ["Site A"]);
        assert_eq!(reg.members_of("South", "2030Q3"), ["Site B"]);
        assert!(reg.members_of("South", "2030Q2").is_empty());
        assert!(reg.members_of("North", "2029Q4").is_empty());

        for (extra, why) in [
            (member("Site A", "Site B", "2030Q1", None), "kind = \"region\""),
            (member("North", "South", "2030Q1", None), "not a registered site"),
            (member("North", "Site Z", "2030Q1", None), "not a registered site"),
            (member("North", "Site A", "2030Q5", None), "quarters YYYYQ1..YYYYQ4"),
            (
                member("North", "Site A", "2030Q2", Some("2030Q1")),
                "is before first",
            ),
            (
                format!(
                    "{}{}",
                    member("North", "Site A", "2030Q1", Some("2030Q3")),
                    member("South", "Site A", "2030Q3", None)
                ),
                "for overlapping releases",
            ),
            (
                format!(
                    "{}{}",
                    member("North", "Site A", "2030Q1", None),
                    member("North", "Site A", "2031Q1", None)
                ),
                "for overlapping releases",
            ),
            (
                "[[region_member]]\nregion = \"North\"\nmember = \"Site A\"\nfirst = \"2030Q1\"\nshare = 1\n"
                    .to_string(),
                "share",
            ),
        ] {
            let e = refusal(&minimal(&b, &format!("{head}{extra}")));
            assert!(e.reason.contains(why) && e.line > 0, "{why}: {e}");
        }
    }

    /// BD-331.S4 (W1-DR-01): `[[printed_share_correction]]` rows. A site's printed share maps to a registered one;
    /// every malformed row refuses at its line.
    #[test]
    fn test_printed_share_corrections_parse_and_refuse() {
        let b = source_b("aisc_reconciliation", SHA_B);
        let head = "[[subject]]\nname = \"North\"\nkind = \"region\"\n\n";
        let row = |subject: &str, printed: &str, num: u64, den: u64| {
            format!(
                "[[printed_share_correction]]\nsubject = \"{subject}\"\nprinted = \"{printed}\"\nnum = {num}\n\
                 den = {den}\nevidence = \"W1-DR-01\"\n\n"
            )
        };
        let reg = parse_sources_registry(&minimal(
            &b,
            &format!("{head}{}", row("Site A", "47.5", 49, 200)),
        ))
        .expect("accepted");
        assert_eq!(
            reg.printed_share_corrections,
            vec![PrintedShareCorrection {
                subject: "Site A".to_string(),
                printed: "47.5".to_string(),
                num: 49,
                den: 200,
                evidence: "W1-DR-01".to_string(),
            }]
        );
        let plain = parse_sources_registry(&minimal(&b, head)).expect("accepted");
        assert!(plain.printed_share_corrections.is_empty());
        for (extra, why) in [
            (row("North", "47.5", 49, 200), "not a registered site"),
            (row("Site Z", "47.5", 49, 200), "not a registered site"),
            (row("Site A", "47.5%", 49, 200), "percentage as printed"),
            (row("Site A", ".5", 49, 200), "percentage as printed"),
            (row("Site A", "47.", 49, 200), "percentage as printed"),
            (row("Site A", "47.5", 201, 200), "exceeds 1"),
            (row("Site A", "47.5", 0, 200), "num"),
            (
                format!("{}{}", row("Site A", "47.5", 49, 200), row("Site A", "47.5", 1, 2)),
                "duplicate [[printed_share_correction]]",
            ),
            (
                "[[printed_share_correction]]\nsubject = \"Site A\"\nprinted = \"47.5\"\nnum = 1\nden = 2\n"
                    .to_string(),
                "evidence",
            ),
        ] {
            let e = refusal(&minimal(&b, &format!("{head}{extra}")));
            assert!(e.reason.contains(why) && e.line > 0, "{why}: {e}");
        }
    }

    /// BD-331.S4 (W1-DR-02): `[[mine_stats_label_alias]]` rows. Both labels must be registered; every malformed
    /// row refuses at its line.
    #[test]
    fn test_mine_stats_label_aliases_parse_and_refuse() {
        let b = source_b("aisc_reconciliation", SHA_B);
        let head = "[[mine_stats_label]]\nlabel = \"Production (millions of pounds)\"\n\n\
                    [[mine_stats_label]]\nlabel = \"Production (Thousands of tonnes)\"\n\n";
        let row = |label: &str, reads_as: &str, first: &str, last: &str| {
            format!(
                "[[mine_stats_label_alias]]\nlabel = \"{label}\"\nreads_as = \"{reads_as}\"\nfirst = \"{first}\"\n\
                 last = \"{last}\"\nevidence = \"W1-DR-02\"\n\n"
            )
        };
        let (mlb, kt) = (
            "Production (millions of pounds)",
            "Production (Thousands of tonnes)",
        );
        let reg = parse_sources_registry(&minimal(
            &b,
            &format!("{head}{}", row(mlb, kt, "2024Q1", "2024Q4")),
        ))
        .expect("accepted");
        let alias = MineStatsLabelAlias {
            label: mlb.to_string(),
            reads_as: kt.to_string(),
            first: "2024Q1".to_string(),
            last: "2024Q4".to_string(),
            evidence: "W1-DR-02".to_string(),
        };
        assert_eq!(reg.mine_stats_label_aliases, vec![alias.clone()]);
        assert!(alias.holds_for("2024Q1") && alias.holds_for("2024Q4"));
        assert!(!alias.holds_for("2023Q4") && !alias.holds_for("2025Q1"));
        for (extra, why) in [
            (
                row("Sales (pounds)", kt, "2024Q1", "2024Q4"),
                "not a registered",
            ),
            (
                row(mlb, "Sales (tonnes)", "2024Q1", "2024Q4"),
                "not a registered",
            ),
            (row(mlb, mlb, "2024Q1", "2024Q4"), "read as itself"),
            (row(mlb, kt, "2024Q5", "2024Q4"), "quarters"),
            (row(mlb, kt, "2024Q3", "2024Q2"), "before first"),
            (
                format!(
                    "{}{}",
                    row(mlb, kt, "2024Q1", "2024Q4"),
                    row(mlb, kt, "2024Q4", "2025Q1")
                ),
                "overlapping releases",
            ),
            (
                format!(
                    "[[mine_stats_label_alias]]\nlabel = \"{mlb}\"\nreads_as = \"{kt}\"\nfirst = \"2024Q1\"\n\
                     last = \"2024Q4\"\n"
                ),
                "evidence",
            ),
        ] {
            let e = refusal(&minimal(&b, &format!("{head}{extra}")));
            assert!(e.reason.contains(why) && e.line > 0, "{why}: {e}");
        }
    }

    /// bd-a28f.1: `[[subject_status]]` rows. A disposal states its closing date; any other status states its
    /// quarters. Every malformed row refuses at its line.
    #[test]
    fn test_subject_statuses_parse_and_refuse() {
        let b = source_b("aisc_reconciliation", SHA_B);
        let rows = "[[subject]]\nname = \"Site B\"\n\n\
                    [[subject_status]]\nsubject = \"Site A\"\nstatus = \"disposed\"\nclosed = \"2029-12-01\"\n\
                    evidence = \"E1\"\n\n\
                    [[subject_status]]\nsubject = \"Site B\"\nstatus = \"withheld\"\nfirst = \"2030Q1\"\n\
                    last = \"2030Q3\"\nevidence = \"E2\"\n";
        let reg = parse_sources_registry(&minimal(&b, rows)).expect("accepted");
        assert_eq!(
            reg.subject_statuses,
            vec![
                SubjectStatusRow {
                    subject: "Site A".into(),
                    status: "disposed".into(),
                    closed: Some("2029-12-01".into()),
                    first: None,
                    last: None,
                    evidence: "E1".into(),
                },
                SubjectStatusRow {
                    subject: "Site B".into(),
                    status: "withheld".into(),
                    closed: None,
                    first: Some("2030Q1".into()),
                    last: Some("2030Q3".into()),
                    evidence: "E2".into(),
                },
            ]
        );
        assert!(
            parse_sources_registry(&minimal(&b, ""))
                .expect("accepted")
                .subject_statuses
                .is_empty()
        );

        let row = |body: &str| {
            format!("[[subject_status]]\nsubject = \"Site A\"\n{body}evidence = \"E1\"\n")
        };
        for (extra, why) in [
            (
                "[[subject_status]]\nsubject = \"Site Z\"\nstatus = \"disposed\"\nclosed = \"2029-12-01\"\n\
                 evidence = \"E1\"\n"
                    .to_string(),
                "not a registered [[subject]]",
            ),
            (
                "[[subject]]\nname = \"R\"\nkind = \"region\"\n\n[[subject_status]]\nsubject = \"R\"\n\
                 status = \"withheld\"\nfirst = \"2030Q1\"\nevidence = \"E1\"\n"
                    .to_string(),
                "is a region",
            ),
            (row("status = \"sold\"\nclosed = \"2029-12-01\"\n"), "is not registered"),
            (row("status = \"disposed\"\nfirst = \"2030Q1\"\n"), "a disposal states closed only"),
            (
                row("status = \"disposed\"\nclosed = \"2029-12-01\"\nlast = \"2030Q1\"\n"),
                "a disposal states closed only",
            ),
            (row("status = \"withheld\"\nclosed = \"2029-12-01\"\n"), "a disposal states closed only"),
            (row("status = \"withheld\"\n"), "a disposal states closed only"),
            (row("status = \"disposed\"\nclosed = \"2029-02-30\"\n"), "date YYYY-MM-DD"),
            (row("status = \"withheld\"\nfirst = \"2030Q5\"\n"), "quarters YYYYQ1..YYYYQ4"),
            (
                row("status = \"withheld\"\nfirst = \"2030Q3\"\nlast = \"2030Q1\"\n"),
                "is before first",
            ),
            (
                format!(
                    "{}\n{}",
                    row("status = \"withheld\"\nfirst = \"2030Q1\"\n"),
                    row("status = \"withheld\"\nfirst = \"2030Q2\"\n")
                ),
                "duplicate [[subject_status]]",
            ),
            (
                "[[subject_status]]\nsubject = \"Site A\"\nstatus = \"withheld\"\nfirst = \"2030Q1\"\n".to_string(),
                "has no evidence",
            ),
        ] {
            let e = refusal(&minimal(&b, &extra));
            assert!(e.reason.contains(why) && e.line > 0, "{why}: {e}");
        }
    }

    #[test]
    fn test_quarter_format() {
        assert!(is_quarter("2026Q2") && is_quarter("2026Q4"));
        assert!(
            !is_quarter("2026Q0")
                && !is_quarter("2026q2")
                && !is_quarter("26Q2")
                && !is_quarter("2026Q22")
        );
    }

    /// A `[[document]]` row with one field replaced (`key = value` lines, `None` drops the key).
    fn document(replace: &[(&str, Option<&str>)]) -> String {
        let fields = [
            ("file", "\"D.pdf\""),
            ("dir", "\"corpus/docs\""),
            ("kind", "\"mda\""),
            ("url", "\"https://example.org/files/D.pdf\""),
            ("url_basis", "\"fetched and verified 2030-02-01\""),
            ("published_at", "\"2030-01-15\""),
            (
                "published_at_basis",
                "\"FACT: filing index accepted 2030-01-15\"",
            ),
            ("pages", "12"),
            ("bytes", "30"),
            (
                "sha256",
                "\"cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc\"",
            ),
        ];
        let mut out = String::from("\n[[document]]\n");
        for (key, value) in fields {
            match replace.iter().find(|(k, _)| *k == key) {
                Some((_, None)) => {}
                Some((_, Some(v))) => out.push_str(&format!("{key} = {v}\n")),
                None => out.push_str(&format!("{key} = {value}\n")),
            }
        }
        for (key, value) in replace {
            if let (false, Some(v)) = (fields.iter().any(|(k, _)| k == key), value) {
                out.push_str(&format!("{key} = {v}\n"));
            }
        }
        out
    }

    const DOCUMENT_ONLY_HEAD: &str = "schema = \"fmine.sources.v1\"\nissuer = \"demo\"\n\
        corpus_dir = \"corpus/demo\"\nretrieved_utc = \"2030-02-01T00:00:00Z\"\n\
        url_prefix = \"https://example.org/files/\"\n[[subject]]\nname = \"Site A\"\n";

    #[test]
    fn test_document_only_profile_keeps_document_validation() {
        let doc = document(&[("kind", Some("\"partner_filing\""))]);
        let reg = parse_sources_registry(&format!("{DOCUMENT_ONLY_HEAD}{doc}"))
            .expect("document-only partner profile");
        let mixed = parse_sources_registry(&minimal("", &doc)).expect("workbook and document");
        assert!(reg.sources.is_empty());
        assert_eq!(reg.documents, mixed.documents);
        assert_eq!(reg.documents[0].kind, "partner_filing");
        assert_eq!(reg.subjects, ["Site A"]);
        for (replace, why) in [
            (vec![("sha256", Some("\"short\""))], "64 lowercase hex"),
            (vec![("pages", Some("0"))], "positive integer"),
            (vec![("published_at", None)], "has no published_at"),
        ] {
            let e = refusal(&format!("{DOCUMENT_ONLY_HEAD}{}", document(&replace)));
            assert!(e.reason.contains(why) && e.line > 0, "{why}: {e}");
        }
        let e = refusal(&format!("{DOCUMENT_ONLY_HEAD}{doc}{doc}"));
        assert!(e.reason.contains("duplicate file"), "{e}");
    }

    #[test]
    fn test_profile_requires_a_workbook_document_or_html() {
        let e = refusal(DOCUMENT_ONLY_HEAD);
        assert_eq!(e.reason, "no [[source]], [[document]] or [[html]] rows");
        let workbook = parse_sources_registry(&minimal("", "")).expect("workbook-only profile");
        assert_eq!(workbook.sources.len(), 1);
        assert_eq!(workbook.sources[0].file, "A.xlsx");
        assert_eq!(workbook.sources[0].sha256, SHA_A);
        assert!(workbook.documents.is_empty());
        let mixed = parse_sources_registry(&minimal("", &document(&[]))).expect("mixed profile");
        assert_eq!(workbook.sources, mixed.sources);
    }

    #[test]
    fn information_circular_keeps_its_document_kind_and_publication_requirements() {
        let row = document(&[("kind", Some("\"information_circular\""))]);
        let parsed = parse_sources_registry(&format!("{DOCUMENT_ONLY_HEAD}{row}"))
            .expect("a circular is an explicitly registered PDF source");
        assert_eq!(parsed.documents[0].kind, "information_circular");
        assert_eq!(parsed.documents[0].published_at, "2030-01-15");
        let missing_date_basis = document(&[
            ("kind", Some("\"information_circular\"")),
            ("published_at_basis", None),
        ]);
        assert!(
            parse_sources_registry(&format!("{DOCUMENT_ONLY_HEAD}{missing_date_basis}")).is_err()
        );
    }

    #[test]
    fn test_documents_parse_and_refuse() {
        let reg = parse_sources_registry(&minimal("", &document(&[]))).expect("valid");
        assert_eq!(
            reg.documents,
            [DocumentRow {
                file: "D.pdf".to_string(),
                dir: "corpus/docs".to_string(),
                kind: "mda".to_string(),
                url: "https://example.org/files/D.pdf".to_string(),
                url_basis: "fetched and verified 2030-02-01".to_string(),
                published_at: "2030-01-15".to_string(),
                published_at_basis: "FACT: filing index accepted 2030-01-15".to_string(),
                pages: 12,
                bytes: 30,
                sha256: "c".repeat(64),
            }]
        );
        // No dir: the registry's corpus_dir. No documents at all is a valid registry.
        let reg = parse_sources_registry(&minimal("", &document(&[("dir", None)]))).expect("valid");
        assert_eq!(reg.documents[0].dir, "corpus/demo");
        let technical = parse_sources_registry(&minimal(
            "",
            &document(&[("kind", Some("\"technical_report\""))]),
        ))
        .expect("registered technical-report kind");
        assert_eq!(technical.documents[0].kind, "technical_report");
        assert!(
            parse_sources_registry(&minimal("", ""))
                .expect("valid")
                .documents
                .is_empty()
        );
        let sha_a = format!("\"{SHA_A}\"");
        for (replace, why) in [
            (
                vec![("kind", Some("\"annual_report\""))],
                "document kind \"annual_report\" is not registered",
            ),
            (
                vec![("file", Some("\"A.xlsx\""))],
                "duplicate file \"A.xlsx\"",
            ),
            (vec![("file", Some("\"docs/D.pdf\""))], "bare file name"),
            (vec![("sha256", Some(sha_a.as_str()))], "duplicate sha256"),
            (vec![("dir", Some("\"../corpus\""))], "plain relative path"),
            (
                vec![("url", Some("\"http://example.org/D.pdf\""))],
                "https://",
            ),
            (
                vec![("url_basis", Some("\"guessed\""))],
                "url_basis \"guessed\"",
            ),
            (
                vec![("published_at", Some("\"2030-02-30\""))],
                "is not YYYY-MM-DD",
            ),
            (
                vec![("published_at_basis", Some("\"filing index\""))],
                "must start with",
            ),
            (
                vec![("published_at_basis", Some("\"FACT: \""))],
                "cite its source",
            ),
            (
                vec![("pages", Some("0"))],
                "pages must be a positive integer",
            ),
            (vec![("published_at", None)], "has no published_at"),
            (
                vec![("quarter", Some("\"2030Q1\""))],
                "unknown key \"quarter\"",
            ),
        ] {
            let e = refusal(&minimal("", &document(&replace)));
            assert!(e.reason.contains(why) && e.line > 0, "{why}: {e}");
        }
    }
}
