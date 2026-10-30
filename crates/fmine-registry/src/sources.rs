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

use crate::toml_subset::{SubsetError, TomlField, TomlRow, TomlValue, parse_subset};

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

fn fail<T>(line: usize, reason: impl Into<String>) -> Result<T, SourcesError> {
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

fn is_lower_hex64(s: &str) -> bool {
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

/// Reads the fields of one row (or of the top level), refusing missing, mistyped and unknown keys.
struct Fields<'a> {
    fields: &'a [TomlField],
    line: usize,
    what: String,
}

impl<'a> Fields<'a> {
    fn of_row(row: &'a TomlRow) -> Self {
        Self {
            fields: &row.fields,
            line: row.line,
            what: format!("[[{}]] row", row.table),
        }
    }

    fn only(&self, allowed: &[&str]) -> Result<(), SourcesError> {
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

    fn string(&self, key: &str) -> Result<(String, usize), SourcesError> {
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

    fn positive_int(&self, key: &str) -> Result<(u64, usize), SourcesError> {
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
        sources.push(SourceRow {
            file,
            kind,
            quarter,
            url: format!("{url_prefix}{path}"),
            url_basis,
            bytes,
            sha256,
        });
    }
    if sources.is_empty() {
        return fail(0, "no [[source]] rows");
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
}
