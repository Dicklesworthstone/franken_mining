//! Transcription registries, `registries/transcriptions/<issuer>.toml` (schema `fmine.transcriptions.v1`,
//! bd-plx1).
//!
//! The repository has no PDF reader, so a value printed only in a PDF enters as a transcription. Each reader
//! enters it once, at a locator (document, page, and the table, row and column labels as printed), with the
//! token exactly as printed. A value is accepted only when two or more different readers entered the same
//! locator and agree on the token, the quantity key, unit, basis and period. A disagreement is listed for a
//! third re-read and never resolved here. Every entry names the sha256 of the bytes it was read from, which must
//! be the registered document's: a re-posted document is a new vintage, and its rows are read again.
//!
//! Real rows are held back from the repository pending the owner's decision on OPEN-R7-1 (committing
//! transcribed values): the default path is git-ignored, and the tests use synthetic rows.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::path::Path;

use fmine_core::hash::sha256;

use crate::sources::{DocumentRow, Fields, SourcesError, fail, is_lower_hex64};
use crate::toml_subset::parse_subset;

/// Schema id a transcription registry must declare.
pub const TRANSCRIPTIONS_SCHEMA: &str = "fmine.transcriptions.v1";
/// The most decimal places a token may print.
pub const MAX_SCALE: u32 = 18;

/// A printed decimal, `mantissa × 10^−scale`. Its rounding step is `10^−scale`: "6.7" is 67 at scale 1.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Decimal {
    pub mantissa: i128,
    pub scale: u32,
}

/// What a token prints.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Token {
    /// One number.
    Point(Decimal),
    /// A range `lo - hi` (a spaced hyphen or an en dash), with `lo ≤ hi`.
    Range(Decimal, Decimal),
}

fn ten_pow(n: u32) -> Option<i128> {
    10_i128.checked_pow(n)
}

/// `a` against `b`, exactly; `None` on overflow.
fn compare(a: Decimal, b: Decimal) -> Option<std::cmp::Ordering> {
    let scale = a.scale.max(b.scale);
    let x = a.mantissa.checked_mul(ten_pow(scale - a.scale)?)?;
    let y = b.mantissa.checked_mul(ten_pow(scale - b.scale)?)?;
    Some(x.cmp(&y))
}

/// One number: digits with optional thousands commas and decimal point, negative in parentheses or after a
/// minus sign (`-` or U+2212).
fn parse_number(s: &str) -> Result<Decimal, String> {
    let s = s.trim();
    let (negative, body) =
        if let Some(inner) = s.strip_prefix('(').and_then(|r| r.strip_suffix(')')) {
            (true, inner)
        } else if let Some(rest) = s.strip_prefix('-').or_else(|| s.strip_prefix('\u{2212}')) {
            (true, rest)
        } else {
            (false, s)
        };
    let (int, frac) = match body.split_once('.') {
        Some((i, f)) => (i, Some(f)),
        None => (body, None),
    };
    let groups: Vec<&str> = int.split(',').collect();
    let digits = |g: &str| !g.is_empty() && g.bytes().all(|c| c.is_ascii_digit());
    let grouped = match groups.as_slice() {
        [] => false,
        [one] => digits(one),
        [first, rest @ ..] => {
            digits(first) && first.len() <= 3 && rest.iter().all(|g| digits(g) && g.len() == 3)
        }
    };
    if !grouped {
        return Err("not a number (digits, thousands commas, one decimal point, sign)".to_string());
    }
    if int.len() > 1 && int.starts_with('0') {
        return Err("a leading zero".to_string());
    }
    let frac = frac.unwrap_or("");
    if body.contains('.') && !digits(frac) {
        return Err("no digits after the decimal point".to_string());
    }
    let scale = u32::try_from(frac.len()).unwrap_or(u32::MAX);
    if scale > MAX_SCALE {
        return Err(format!("more than {MAX_SCALE} decimal places"));
    }
    let mut mantissa: i128 = 0;
    for c in int.bytes().filter(u8::is_ascii_digit).chain(frac.bytes()) {
        mantissa = mantissa
            .checked_mul(10)
            .and_then(|m| m.checked_add(i128::from(c - b'0')))
            .ok_or_else(|| "too many digits".to_string())?;
    }
    Ok(Decimal {
        mantissa: if negative { -mantissa } else { mantissa },
        scale,
    })
}

/// Parses a token as printed: "(708)" is −708, "6.7" is 67 at scale 1 (step 0.1), "200 - 230" is the range
/// [200, 230]. The reason names no value, so it can be logged.
pub fn parse_token(token: &str) -> Result<Token, String> {
    let range = token
        .split_once(" - ")
        .or_else(|| token.split_once('\u{2013}'));
    match range {
        Some((lo, hi)) => {
            let (lo, hi) = (parse_number(lo)?, parse_number(hi)?);
            match compare(lo, hi) {
                Some(std::cmp::Ordering::Greater) => {
                    Err("a range whose low end exceeds its high end".to_string())
                }
                Some(_) => Ok(Token::Range(lo, hi)),
                None => Err("too many digits".to_string()),
            }
        }
        None => parse_number(token).map(Token::Point),
    }
}

/// Where a value is printed. Cited as `<doc>:p<page>:<table>:<row>/<col>`: bd-plx1's `<doc>:p<page>:<row>/<col>`
/// with the table label added, since one page can print two tables with the same row and column labels.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Locator {
    pub doc: String,
    pub page: u64,
    pub table: String,
    pub row: String,
    pub col: String,
}

impl fmt::Display for Locator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}:p{}:{}:{}/{}",
            self.doc, self.page, self.table, self.row, self.col
        )
    }
}

/// One reader's entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntryRow {
    pub locator: Locator,
    pub doc_sha256: String,
    /// Exactly as printed.
    pub token: String,
    pub value: Token,
    /// The key of what the value measures (lowercase letters, digits and `_`). Two documents that print the
    /// same quantity share the key; that is what a tie discovery reads.
    pub quantity: String,
    pub unit: String,
    pub basis: String,
    pub period: String,
    /// A role id, never a person's name.
    pub reader: String,
    pub line: usize,
}

/// A checked transcription registry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TranscriptionRegistry {
    pub issuer: String,
    /// In file order.
    pub entries: Vec<EntryRow>,
}

fn is_key(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 64
        && s.bytes()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'_' || c == b'-')
}

/// Parses and checks a transcription registry for `issuer` against its registered documents.
pub fn parse_transcriptions(
    text: &str,
    issuer: &str,
    documents: &[DocumentRow],
) -> Result<TranscriptionRegistry, SourcesError> {
    let doc = parse_subset(text)?;
    let top = Fields {
        fields: &doc.top,
        line: 0,
        what: "the top level".to_string(),
    };
    top.only(&["schema", "issuer"])?;
    let (schema, schema_line) = top.string("schema")?;
    if schema != TRANSCRIPTIONS_SCHEMA {
        return fail(
            schema_line,
            format!("schema {schema:?} is not {TRANSCRIPTIONS_SCHEMA:?}"),
        );
    }
    let (named, issuer_line) = top.string("issuer")?;
    if named != issuer {
        return fail(
            issuer_line,
            format!("issuer {named:?} is not the sources registry's {issuer:?}"),
        );
    }
    let mut entries: Vec<EntryRow> = Vec::new();
    for row in &doc.rows {
        if row.table != "entry" {
            return fail(row.line, format!("unknown table [[{}]]", row.table));
        }
        let f = Fields::of_row(row);
        f.only(&[
            "doc",
            "doc_sha256",
            "page",
            "table",
            "row",
            "col",
            "token",
            "quantity",
            "unit",
            "basis",
            "period",
            "reader",
        ])?;
        let (doc_name, doc_line) = f.string("doc")?;
        let (doc_sha256, sha_line) = f.string("doc_sha256")?;
        let (page, page_line) = f.positive_int("page")?;
        let (token, token_line) = f.string("token")?;
        let (quantity, quantity_line) = f.string("quantity")?;
        let (reader, reader_line) = f.string("reader")?;
        let Some(registered) = documents.iter().find(|d| d.file == doc_name) else {
            return fail(
                doc_line,
                format!("doc {doc_name:?} is not a registered [[document]]"),
            );
        };
        if !is_lower_hex64(&doc_sha256) {
            return fail(sha_line, "doc_sha256 must be 64 lowercase hex characters");
        }
        if doc_sha256 != registered.sha256 {
            return fail(
                sha_line,
                format!(
                    "doc_sha256 is not the registered digest of {doc_name:?}: the entry was read from other bytes"
                ),
            );
        }
        if page > registered.pages {
            return fail(
                page_line,
                format!(
                    "page {page} is past {doc_name:?}'s {} pages",
                    registered.pages
                ),
            );
        }
        let value = parse_token(&token).map_err(|why| SourcesError {
            line: token_line,
            reason: format!("token: {why}"),
        })?;
        if !is_key(&quantity) || quantity.contains('-') {
            return fail(
                quantity_line,
                format!("quantity {quantity:?} must be lowercase letters, digits or _"),
            );
        }
        if !is_key(&reader) {
            return fail(
                reader_line,
                format!("reader {reader:?} must be a role id (lowercase letters, digits, _ or -)"),
            );
        }
        let locator = Locator {
            doc: doc_name,
            page,
            table: f.string("table")?.0,
            row: f.string("row")?.0,
            col: f.string("col")?.0,
        };
        if entries
            .iter()
            .any(|e| e.reader == reader && e.locator == locator)
        {
            return fail(row.line, format!("{reader} enters {locator} twice"));
        }
        if entries
            .iter()
            .any(|e| e.reader == reader && e.locator.doc == locator.doc && e.quantity == quantity)
        {
            return fail(
                quantity_line,
                format!(
                    "{reader} enters quantity {quantity:?} twice in {:?}",
                    locator.doc
                ),
            );
        }
        entries.push(EntryRow {
            locator,
            doc_sha256,
            token,
            value,
            quantity,
            unit: f.string("unit")?.0,
            basis: f.string("basis")?.0,
            period: f.string("period")?.0,
            reader,
            line: row.line,
        });
    }
    Ok(TranscriptionRegistry {
        issuer: issuer.to_string(),
        entries,
    })
}

/// Reads and checks a transcription registry file.
pub fn load_transcriptions(
    path: &Path,
    issuer: &str,
    documents: &[DocumentRow],
) -> Result<TranscriptionRegistry, SourcesError> {
    let text = std::fs::read_to_string(path).map_err(|e| SourcesError {
        line: 0,
        reason: format!("cannot read {}: {e}", path.display()),
    })?;
    parse_transcriptions(&text, issuer, documents)
}

/// A value two or more readers agree on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Accepted {
    pub locator: Locator,
    pub doc_sha256: String,
    pub token: String,
    pub value: Token,
    pub quantity: String,
    pub unit: String,
    pub basis: String,
    pub period: String,
    /// Sorted.
    pub readers: Vec<String>,
}

/// Readers who entered one locator differently: the fields that differ, by name (never their values).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Disagreement {
    pub locator: Locator,
    pub readers: Vec<String>,
    pub fields: Vec<&'static str>,
}

/// The two-reader check over a registry.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Agreement {
    /// In locator order.
    pub accepted: Vec<Accepted>,
    /// Locators only one reader has entered so far, with that reader.
    pub single_reader: Vec<(Locator, String)>,
    pub disagreements: Vec<Disagreement>,
    /// One quantity agreed at two locators of one document: `(doc, quantity, locators)`. Neither is accepted.
    pub quantity_clashes: Vec<(String, String, Vec<Locator>)>,
    /// Every quantity named by any reader at a rejected locator. No raw tokens are retained here.
    /// Missing metadata remains unknown, including for manually constructed legacy agreements.
    pub rejected_quantities: BTreeMap<Locator, BTreeSet<String>>,
}

impl Agreement {
    /// sha256 over `"{locator}\t{doc_sha256}\t{token}\t{quantity}\t{unit}\t{basis}\t{period}\n"` for every
    /// accepted value in locator order. Readers are left out, so a third reader who agrees reproduces it.
    #[must_use]
    pub fn digest(&self) -> String {
        let lines: String = self
            .accepted
            .iter()
            .map(|a| {
                format!(
                    "{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
                    a.locator, a.doc_sha256, a.token, a.quantity, a.unit, a.basis, a.period
                )
            })
            .collect();
        sha256(lines.as_bytes()).to_hex()
    }

    /// Whether every entered locator is accepted or still waits for its second reader.
    #[must_use]
    pub fn is_consistent(&self) -> bool {
        self.disagreements.is_empty() && self.quantity_clashes.is_empty()
    }

    /// Accepted quantities with competing pending locations in the same document, in stable order.
    /// Missing rejection metadata leaves the pending identity unknown and conservatively competing.
    /// An unrelated pending quantity does not invalidate an otherwise agreed value.
    #[must_use]
    pub fn accepted_pending_clashes(&self) -> Vec<String> {
        let mut clashes = BTreeSet::new();
        for (at, reader) in &self.single_reader {
            let quantities = self.rejected_quantities.get(at);
            let unknown = quantities.is_none_or(BTreeSet::is_empty);
            for accepted in &self.accepted {
                if accepted.locator.doc == at.doc
                    && (unknown || quantities.is_some_and(|q| q.contains(&accepted.quantity)))
                {
                    clashes.insert(format!(
                        "quantity {}: accepted at {}; pending at {at} by {reader}{}",
                        accepted.quantity,
                        accepted.locator,
                        if unknown {
                            " (quantity identity unknown)"
                        } else {
                            ""
                        },
                    ));
                }
            }
        }
        clashes.into_iter().collect()
    }

    /// Whether this quantity namespace has no pending or conflicting readings. Every reader's identity
    /// participates: disagreement about whether a quantity is guidance cannot move it outside this gate.
    /// Unknown or incomplete rejection metadata fails closed. An empty prefix covers every quantity.
    #[must_use]
    pub fn is_complete_for_prefix(&self, prefix: &str) -> bool {
        let rejected: BTreeSet<&Locator> = self
            .single_reader
            .iter()
            .map(|(at, _)| at)
            .chain(self.disagreements.iter().map(|d| &d.locator))
            .chain(self.quantity_clashes.iter().flat_map(|(_, _, at)| at))
            .collect();
        if rejected.len() != self.rejected_quantities.len() {
            return false;
        }
        for (doc, quantity, locators) in &self.quantity_clashes {
            if locators.is_empty()
                || locators.iter().any(|at| {
                    at.doc != *doc
                        || self
                            .rejected_quantities
                            .get(at)
                            .is_none_or(|quantities| !quantities.contains(quantity))
                })
            {
                return false;
            }
        }
        rejected.into_iter().all(|at| {
            self.rejected_quantities.get(at).is_some_and(|quantities| {
                !quantities.is_empty()
                    && quantities.iter().all(|quantity| {
                        is_key(quantity) && !quantity.contains('-') && !quantity.starts_with(prefix)
                    })
            })
        })
    }
}

/// Groups entries by locator and accepts those that two or more readers entered alike.
#[must_use]
pub fn agree(registry: &TranscriptionRegistry) -> Agreement {
    let mut by_locator: BTreeMap<&Locator, Vec<&EntryRow>> = BTreeMap::new();
    for e in &registry.entries {
        by_locator.entry(&e.locator).or_default().push(e);
    }
    let mut out = Agreement::default();
    for (locator, entries) in by_locator {
        let mut readers: Vec<String> = entries.iter().map(|e| e.reader.clone()).collect();
        readers.sort();
        let [first, rest @ ..] = entries.as_slice() else {
            continue;
        };
        if rest.is_empty() {
            out.single_reader
                .push((locator.clone(), first.reader.clone()));
            continue;
        }
        let mut fields = Vec::new();
        for (name, get) in [
            (
                "token",
                (|e: &EntryRow| e.token.clone()) as fn(&EntryRow) -> String,
            ),
            ("quantity", |e| e.quantity.clone()),
            ("unit", |e| e.unit.clone()),
            ("basis", |e| e.basis.clone()),
            ("period", |e| e.period.clone()),
        ] {
            if rest.iter().any(|e| get(e) != get(first)) {
                fields.push(name);
            }
        }
        if fields.is_empty() {
            out.accepted.push(Accepted {
                locator: locator.clone(),
                doc_sha256: first.doc_sha256.clone(),
                token: first.token.clone(),
                value: first.value,
                quantity: first.quantity.clone(),
                unit: first.unit.clone(),
                basis: first.basis.clone(),
                period: first.period.clone(),
                readers,
            });
        } else {
            out.disagreements.push(Disagreement {
                locator: locator.clone(),
                readers,
                fields,
            });
        }
    }
    let mut seen: BTreeMap<(String, String), Vec<Locator>> = BTreeMap::new();
    for a in &out.accepted {
        seen.entry((a.locator.doc.clone(), a.quantity.clone()))
            .or_default()
            .push(a.locator.clone());
    }
    for ((doc, quantity), locators) in seen {
        if locators.len() > 1 {
            out.accepted
                .retain(|a| !(a.locator.doc == doc && a.quantity == quantity));
            out.quantity_clashes.push((doc, quantity, locators));
        }
    }
    let rejected: BTreeSet<Locator> = out
        .single_reader
        .iter()
        .map(|(at, _)| at.clone())
        .chain(out.disagreements.iter().map(|d| d.locator.clone()))
        .chain(
            out.quantity_clashes
                .iter()
                .flat_map(|(_, _, at)| at.iter().cloned()),
        )
        .collect();
    for entry in &registry.entries {
        if rejected.contains(&entry.locator) {
            out.rejected_quantities
                .entry(entry.locator.clone())
                .or_default()
                .insert(entry.quantity.clone());
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const SHA_D: &str = "dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd";
    const SHA_E: &str = "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee";

    fn docs() -> Vec<DocumentRow> {
        let doc = |file: &str, sha: &str, pages| DocumentRow {
            file: file.to_string(),
            dir: "corpus".to_string(),
            kind: "mda".to_string(),
            url: format!("https://example.invalid/{file}"),
            url_basis: "W1 table".to_string(),
            published_at: "2030-02-01".to_string(),
            published_at_basis: "FACT: synthetic".to_string(),
            pages,
            bytes: 10,
            sha256: sha.to_string(),
        };
        vec![doc("Deck.pdf", SHA_D, 15), doc("Mda.pdf", SHA_E, 80)]
    }

    fn d(mantissa: i128, scale: u32) -> Decimal {
        Decimal { mantissa, scale }
    }

    /// One `[[entry]]` row; `token` is printed as given.
    fn entry(reader: &str, doc: &str, page: u64, row: &str, token: &str, quantity: &str) -> String {
        let sha = if doc == "Deck.pdf" { SHA_D } else { SHA_E };
        format!(
            "\n[[entry]]\ndoc = \"{doc}\"\ndoc_sha256 = \"{sha}\"\npage = {page}\ntable = \"Reserves roll-forward\"\n\
             row = \"{row}\"\ncol = \"2030\"\ntoken = \"{token}\"\nquantity = \"{quantity}\"\nunit = \"Moz\"\n\
             basis = \"attributable\"\nperiod = \"2030\"\nreader = \"{reader}\"\n"
        )
    }

    fn registry(rows: &[String]) -> String {
        format!(
            "schema = \"fmine.transcriptions.v1\"\nissuer = \"demo\"\n{}",
            rows.concat()
        )
    }

    /// The synthetic golden list: two documents, two quantities each, both read by two readers.
    fn golden() -> Vec<String> {
        let mut rows = Vec::new();
        for reader in ["reader_a", "reader_b"] {
            rows.push(entry(
                reader,
                "Deck.pdf",
                14,
                "Depletion",
                "(3.6)",
                "depletion",
            ));
            rows.push(entry(
                reader,
                "Deck.pdf",
                14,
                "Net change",
                "1.7",
                "net_change",
            ));
            rows.push(entry(
                reader,
                "Mda.pdf",
                71,
                "Depletion",
                "(3.7)",
                "depletion",
            ));
            rows.push(entry(
                reader,
                "Mda.pdf",
                71,
                "Net change",
                "1.8",
                "net_change",
            ));
        }
        rows
    }

    fn refusal(text: &str) -> SourcesError {
        match parse_transcriptions(text, "demo", &docs()) {
            Ok(r) => panic!("accepted: {r:?}"),
            Err(e) => e,
        }
    }

    #[test]
    fn test_parse_token_cases() {
        for (token, want) in [
            ("(708)", Token::Point(d(-708, 0))),
            ("6.7", Token::Point(d(67, 1))),
            ("200 - 230", Token::Range(d(200, 0), d(230, 0))),
            ("200\u{2013}230", Token::Range(d(200, 0), d(230, 0))),
            ("1.5 \u{2013} 2", Token::Range(d(15, 1), d(2, 0))),
            ("1,423", Token::Point(d(1423, 0))),
            ("12,345,678.90", Token::Point(d(1_234_567_890, 2))),
            ("-3.6", Token::Point(d(-36, 1))),
            ("\u{2212}3.6", Token::Point(d(-36, 1))),
            ("(0.05)", Token::Point(d(-5, 2))),
            ("0", Token::Point(d(0, 0))),
            ("85", Token::Point(d(85, 0))),
        ] {
            assert_eq!(parse_token(token), Ok(want), "{token}");
        }
        for (token, why) in [
            ("", "not a number"),
            ("n/a", "not a number"),
            ("\u{2014}", "not a number"),
            ("1,23", "not a number"),
            ("1234,567", "not a number"),
            ("1 234", "not a number"),
            ("012", "leading zero"),
            ("1.", "no digits after"),
            (".5", "not a number"),
            ("1.2.3", "no digits after"),
            ("((1))", "not a number"),
            ("-(1)", "not a number"),
            ("--1", "not a number"),
            ("230 - 200", "low end exceeds"),
            ("0.1234567890123456789", "decimal places"),
            ("170141183460469231731687303715884105728", "too many digits"),
        ] {
            match parse_token(token) {
                Ok(t) => panic!("{token:?} parsed as {t:?}"),
                Err(e) => assert!(e.contains(why), "{token:?}: {e}"),
            }
        }
    }

    #[test]
    fn test_two_reader_agreement_and_golden_digest() {
        let reg = parse_transcriptions(&registry(&golden()), "demo", &docs()).expect("valid");
        assert_eq!(reg.entries.len(), 8);
        let a = agree(&reg);
        assert!(a.is_consistent() && a.single_reader.is_empty());
        let cited: Vec<String> = a.accepted.iter().map(|v| v.locator.to_string()).collect();
        assert_eq!(
            cited,
            [
                "Deck.pdf:p14:Reserves roll-forward:Depletion/2030",
                "Deck.pdf:p14:Reserves roll-forward:Net change/2030",
                "Mda.pdf:p71:Reserves roll-forward:Depletion/2030",
                "Mda.pdf:p71:Reserves roll-forward:Net change/2030",
            ]
        );
        assert_eq!(a.accepted[0].value, Token::Point(d(-36, 1)));
        assert_eq!(a.accepted[0].readers, ["reader_a", "reader_b"]);
        // Golden: the digest of the synthetic list above, recomputed outside Rust from the same four lines.
        assert_eq!(
            a.digest(),
            "64cb8b46f1188b6dbdfee984255b2170406e1168acd2dd232542961ad470cada"
        );
        // Entry order does not move the digest; readers do not enter it.
        let mut shuffled = golden();
        shuffled.reverse();
        let b = agree(&parse_transcriptions(&registry(&shuffled), "demo", &docs()).expect("valid"));
        assert_eq!(b.digest(), a.digest());
    }

    #[test]
    fn pending_quantity_competitors_retain_both_locations_even_for_equal_values() {
        let mut rows = golden();
        rows.push(entry(
            "reader_c",
            "Deck.pdf",
            13,
            "Depletion",
            "(3.6)",
            "depletion",
        ));
        let agreement = agree(&parse_transcriptions(&registry(&rows), "demo", &docs()).unwrap());
        let clashes = agreement.accepted_pending_clashes();
        assert_eq!(clashes.len(), 1);
        assert!(clashes[0].contains("quantity depletion"));
        assert!(clashes[0].contains("accepted at Deck.pdf:p14:"));
        assert!(clashes[0].contains("pending at Deck.pdf:p13:"));
        assert!(clashes[0].contains("reader_c"));
        // Ordinary consistency permits pending reads; consumers requiring a unique quantity opt in.
        assert!(agreement.is_consistent());
        rows.reverse();
        let reordered = agree(&parse_transcriptions(&registry(&rows), "demo", &docs()).unwrap());
        assert_eq!(clashes, reordered.accepted_pending_clashes());

        let mut unknown = agreement.clone();
        unknown.rejected_quantities.clear();
        assert_eq!(unknown.accepted_pending_clashes().len(), 2);
        assert!(
            unknown
                .accepted_pending_clashes()
                .iter()
                .all(|s| s.contains("identity unknown"))
        );
        let at = unknown.single_reader[0].0.clone();
        unknown.rejected_quantities.insert(at, BTreeSet::new());
        assert_eq!(unknown.accepted_pending_clashes().len(), 2);
    }

    #[test]
    fn unrelated_pending_quantities_and_other_documents_do_not_compete() {
        let mut rows = golden();
        rows.push(entry(
            "reader_c", "Deck.pdf", 13, "Opening", "10.0", "opening",
        ));
        let mut agreement =
            agree(&parse_transcriptions(&registry(&rows), "demo", &docs()).unwrap());
        assert!(agreement.accepted_pending_clashes().is_empty());
        let pending = agreement.single_reader[0].0.clone();
        agreement
            .rejected_quantities
            .insert(pending, ["depletion".into()].into());
        agreement.accepted.retain(|a| a.locator.doc == "Mda.pdf");
        assert!(agreement.accepted_pending_clashes().is_empty());
    }

    #[test]
    fn test_disagreement_is_listed_never_resolved() {
        // Planted negative: reader_b reads the deck's depletion as (3.5).
        let mut rows = golden();
        rows[4] = entry(
            "reader_b",
            "Deck.pdf",
            14,
            "Depletion",
            "(3.5)",
            "depletion",
        );
        let a = agree(&parse_transcriptions(&registry(&rows), "demo", &docs()).expect("valid"));
        assert!(!a.is_consistent());
        assert_eq!(a.accepted.len(), 3);
        assert_eq!(
            a.disagreements,
            [Disagreement {
                locator: Locator {
                    doc: "Deck.pdf".to_string(),
                    page: 14,
                    table: "Reserves roll-forward".to_string(),
                    row: "Depletion".to_string(),
                    col: "2030".to_string(),
                },
                readers: vec!["reader_a".to_string(), "reader_b".to_string()],
                fields: vec!["token"],
            }]
        );
        // A third reader who agrees with reader_a does not outvote reader_b.
        rows.push(entry(
            "reader_c",
            "Deck.pdf",
            14,
            "Depletion",
            "(3.6)",
            "depletion",
        ));
        let a = agree(&parse_transcriptions(&registry(&rows), "demo", &docs()).expect("valid"));
        assert_eq!(a.disagreements.len(), 1);
        assert_eq!(a.disagreements[0].readers.len(), 3);
        // A misread row label is a different locator: two single-reader entries, nothing accepted there.
        let mut rows = golden();
        rows[5] = entry(
            "reader_b",
            "Deck.pdf",
            14,
            "Net changes",
            "1.7",
            "net_change",
        );
        let a = agree(&parse_transcriptions(&registry(&rows), "demo", &docs()).expect("valid"));
        assert!(a.is_consistent());
        assert_eq!(a.accepted.len(), 3);
        assert_eq!(a.single_reader.len(), 2);
        // One quantity agreed at two locators of one document: neither is accepted.
        let mut rows = golden();
        for reader in ["reader_c", "reader_d"] {
            rows.push(entry(
                reader,
                "Deck.pdf",
                13,
                "Depletion",
                "(3.6)",
                "depletion",
            ));
        }
        let a = agree(&parse_transcriptions(&registry(&rows), "demo", &docs()).expect("valid"));
        assert_eq!(a.quantity_clashes.len(), 1);
        assert_eq!(a.quantity_clashes[0].2.len(), 2);
        assert_eq!(a.accepted.len(), 3);
        assert!(!a.is_consistent());
    }

    #[test]
    fn test_transcription_registry_refusals() {
        let one = |row: String| registry(&[row]);
        let base = entry(
            "reader_a",
            "Deck.pdf",
            14,
            "Depletion",
            "(3.6)",
            "depletion",
        );
        for (text, why) in [
            (
                one(base.replace("Deck.pdf", "Other.pdf")),
                "is not a registered [[document]]",
            ),
            (one(base.replace(SHA_D, SHA_E)), "read from other bytes"),
            (
                one(base.replace("page = 14", "page = 16")),
                "past \"Deck.pdf\"'s 15 pages",
            ),
            (one(base.replace("(3.6)", "n/a")), "token: not a number"),
            (
                one(base.replace("reader_a", "Jane Doe")),
                "must be a role id",
            ),
            (
                one(base.replace("\"depletion\"", "\"Depletion\"")),
                "quantity \"Depletion\"",
            ),
            (
                one(base.replace("period = ", "vintage = ")),
                "unknown key \"vintage\"",
            ),
            (
                registry(&[base.clone(), base.clone()]),
                "enters Deck.pdf:p14",
            ),
            (
                registry(&[
                    base.clone(),
                    entry(
                        "reader_a",
                        "Deck.pdf",
                        14,
                        "Depletions",
                        "(3.6)",
                        "depletion",
                    ),
                ]),
                "enters quantity \"depletion\" twice",
            ),
            (
                one(base.replace("[[entry]]", "[[row]]")),
                "unknown table [[row]]",
            ),
        ] {
            let e = refusal(&text);
            assert!(e.reason.contains(why) && e.line > 0, "{why}: {e}");
        }
        let e = refusal(&registry(&[]).replace("\"demo\"", "\"other\""));
        assert!(e.reason.contains("not the sources registry's"), "{e}");
        let e = refusal(&registry(&[]).replace("v1", "v0"));
        assert!(
            e.reason.contains("is not \"fmine.transcriptions.v1\""),
            "{e}"
        );
    }

    fn scoped(rows: &[String]) -> Agreement {
        agree(&parse_transcriptions(&registry(rows), "demo", &docs()).expect("valid readings"))
    }

    #[test]
    fn test_scoped_agreement_keeps_optional_pending_rows_outside_guidance() {
        let mut rows = vec![
            entry(
                "reader_a",
                "Deck.pdf",
                1,
                "Guidance",
                "100",
                "guidance_site_a_production",
            ),
            entry(
                "reader_b",
                "Deck.pdf",
                1,
                "Guidance",
                "100",
                "guidance_site_a_production",
            ),
        ];
        let original = scoped(&rows);
        rows.push(entry(
            "reader_a",
            "Deck.pdf",
            2,
            "Price",
            "2000",
            "price_site_a_gold",
        ));
        let result = scoped(&rows);
        assert!(result.is_complete_for_prefix("guidance_"));
        assert!(!result.is_complete_for_prefix("price_"));
        assert!(!result.is_complete_for_prefix(""));
        assert_eq!(result.single_reader.len(), 1);
        assert_eq!(result.accepted, original.accepted);
        assert_eq!(result.digest(), original.digest());
        assert_eq!(
            result.rejected_quantities[&result.single_reader[0].0],
            BTreeSet::from(["price_site_a_gold".to_string()])
        );
    }

    #[test]
    fn test_scoped_agreement_keeps_all_disagreeing_quantity_identities() {
        let rows = vec![
            entry(
                "reader_a",
                "Deck.pdf",
                1,
                "Same locator",
                "100",
                "guidance_site_a_production",
            ),
            entry(
                "reader_b",
                "Deck.pdf",
                1,
                "Same locator",
                "100",
                "price_site_a_gold",
            ),
            entry(
                "reader_c",
                "Deck.pdf",
                1,
                "Same locator",
                "100",
                "price_site_a_gold",
            ),
        ];
        let result = scoped(&rows);
        assert!(!result.is_consistent());
        assert!(!result.is_complete_for_prefix("guidance_"));
        assert!(!result.is_complete_for_prefix("price_"));
        assert!(result.accepted.is_empty());
        assert_eq!(
            result.rejected_quantities[&result.disagreements[0].locator],
            BTreeSet::from([
                "guidance_site_a_production".to_string(),
                "price_site_a_gold".to_string()
            ])
        );
        let mut reversed = rows;
        reversed.reverse();
        assert_eq!(
            scoped(&reversed).rejected_quantities,
            result.rejected_quantities
        );
    }

    #[test]
    fn test_scoped_agreement_allows_optional_conflicts_without_resolving_them() {
        let result = scoped(&[
            entry(
                "reader_a",
                "Deck.pdf",
                1,
                "Price",
                "2000",
                "price_site_a_gold",
            ),
            entry(
                "reader_b",
                "Deck.pdf",
                1,
                "Price",
                "2100",
                "price_site_a_gold",
            ),
        ]);
        assert!(!result.is_consistent());
        assert!(result.is_complete_for_prefix("guidance_"));
        assert!(!result.is_complete_for_prefix("price_"));
        assert_eq!(result.disagreements.len(), 1);
        assert!(result.accepted.is_empty());
    }

    #[test]
    fn test_scoped_agreement_retains_every_quantity_clash_locator() {
        let mut result = scoped(&[
            entry(
                "reader_a",
                "Deck.pdf",
                1,
                "Price A",
                "2000",
                "price_site_a_gold",
            ),
            entry(
                "reader_b",
                "Deck.pdf",
                1,
                "Price A",
                "2000",
                "price_site_a_gold",
            ),
            entry(
                "reader_c",
                "Deck.pdf",
                2,
                "Price B",
                "2100",
                "price_site_a_gold",
            ),
            entry(
                "reader_d",
                "Deck.pdf",
                2,
                "Price B",
                "2100",
                "price_site_a_gold",
            ),
        ]);
        assert_eq!(result.quantity_clashes.len(), 1);
        assert_eq!(result.rejected_quantities.len(), 2);
        assert!(result.accepted.is_empty());
        assert!(result.is_complete_for_prefix("guidance_"));
        assert!(!result.is_complete_for_prefix("price_"));
        let missing = result.quantity_clashes[0].2[0].clone();
        result.rejected_quantities.remove(&missing);
        assert!(!result.is_complete_for_prefix("guidance_"));
        result
            .rejected_quantities
            .insert(missing, BTreeSet::from(["other_quantity".into()]));
        assert!(!result.is_complete_for_prefix("guidance_"));
    }

    #[test]
    fn test_scoped_agreement_unknown_empty_and_orphan_metadata_fail_closed() {
        let result = scoped(&[entry(
            "reader_a",
            "Deck.pdf",
            1,
            "Price",
            "2000",
            "price_site_a_gold",
        )]);
        let at = result.single_reader[0].0.clone();
        assert!(result.is_complete_for_prefix("guidance_"));
        let mut unknown = result.clone();
        unknown.rejected_quantities.clear();
        assert!(!unknown.is_complete_for_prefix("guidance_"));
        let mut empty = result.clone();
        empty
            .rejected_quantities
            .insert(at.clone(), BTreeSet::new());
        assert!(!empty.is_complete_for_prefix("guidance_"));
        let mut invalid = result.clone();
        invalid
            .rejected_quantities
            .insert(at.clone(), BTreeSet::from([String::new()]));
        assert!(!invalid.is_complete_for_prefix("guidance_"));
        let mut orphan = Agreement::default();
        orphan
            .rejected_quantities
            .insert(at, BTreeSet::from(["price_site_a_gold".into()]));
        assert!(!orphan.is_complete_for_prefix("guidance_"));
        let mut empty_clash = Agreement::default();
        empty_clash
            .quantity_clashes
            .push(("Deck.pdf".into(), "price_site_a_gold".into(), vec![]));
        assert!(!empty_clash.is_complete_for_prefix("guidance_"));
    }
}
