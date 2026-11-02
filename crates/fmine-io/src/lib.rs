//! Label-anchored `.xlsx` (`xlsx`) and byte-bound HTML (`HtmlDocument`) filing table extractors
//! with SHA-256 provenance and `G-BULL-1` / `G-BULL-2` quarantine (`WP-03`, `BD-056`, `BD-057`, `BD-271`).

pub mod forecast;
pub mod numfmt;
pub mod numtok;
pub mod reliance;
pub mod styles;
pub mod xlsx;
pub mod zip_inflate;

use std::fmt;

use xlsx::CellValue;

/// Errors raised during `.xlsx` or HTML filing ingestion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IoError {
    FileRead { path: String, detail: String },
    Zip(String),
    Xml(String),
    MissingAnchorLabel { artifact: String, label: String },
}

impl fmt::Display for IoError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::FileRead { path, detail } => write!(f, "failed to read {path}: {detail}"),
            Self::Zip(msg) => write!(f, "ZIP error: {msg}"),
            Self::Xml(msg) => write!(f, "XML structure error: {msg}"),
            Self::MissingAnchorLabel { artifact, label } => {
                write!(f, "missing required anchor label '{label}' in {artifact}")
            }
        }
    }
}

impl std::error::Error for IoError {}

/// Legacy table projection. Covered merged-cell slots are empty, never copied observations.
#[derive(Debug, Clone, PartialEq)]
pub struct HtmlTable {
    pub table_index: usize,
    pub rows: Vec<Vec<CellValue>>,
}

/// Resource limits cover the entire document, including empty expanded-grid slots.
pub const HTML_MAX_BYTES: usize = 8 * 1024 * 1024;
pub const HTML_MAX_CELLS: usize = 100_000;
pub const HTML_MAX_SLOTS: usize = 1_000_000;
const HTML_MAX_TABLES: usize = 1024;
const HTML_MAX_ROWS: usize = 16_384;
const HTML_MAX_COLUMNS: usize = 4096;
const HTML_MAX_CELL_BYTES: usize = 65_536;

type HtmlResult<T> = Result<T, fmine_evidence::Refusal>;

fn html_refusal(reason: impl Into<String>) -> fmine_evidence::Refusal {
    fmine_evidence::Refusal::new(
        fmine_evidence::RefusalCode::SourceCellUnusable208,
        format!("HTML table ingestion: {}", reason.into()),
    )
}

/// One source cell, not one expanded slot. Coordinates and spans are one-based and positive.
/// Raw HTML and its byte range preserve the original spelling. Text decodes entities once and
/// separates distinct text runs; annotations and split runs never become numeric assertions.
#[derive(Debug, Clone, PartialEq)]
pub struct HtmlSourceCell {
    row: usize,
    column: usize,
    row_span: usize,
    column_span: usize,
    byte_range: (usize, usize),
    raw_html: String,
    text: String,
    value: CellValue,
    number: Option<numtok::NumberToken>,
}

impl HtmlSourceCell {
    pub const fn row(&self) -> usize {
        self.row
    }
    pub const fn column(&self) -> usize {
        self.column
    }
    pub const fn row_span(&self) -> usize {
        self.row_span
    }
    pub const fn column_span(&self) -> usize {
        self.column_span
    }
    pub const fn byte_range(&self) -> (usize, usize) {
        self.byte_range
    }
    pub fn raw_html(&self) -> &str {
        &self.raw_html
    }
    pub fn text(&self) -> &str {
        &self.text
    }
    pub fn value(&self) -> &CellValue {
        &self.value
    }
    /// Exact decimal and printed precision; never reconstructed from the floating projection.
    pub fn number(&self) -> Option<&numtok::NumberToken> {
        self.number.as_ref()
    }
}

/// A rectangular slot grid whose occupied slots reference immutable source-cell origins.
#[derive(Debug, Clone, PartialEq)]
pub struct HtmlSourceTable {
    table_index: usize,
    cells: Vec<HtmlSourceCell>,
    slots: Vec<Vec<Option<usize>>>,
}

impl HtmlSourceTable {
    pub const fn table_index(&self) -> usize {
        self.table_index
    }
    pub fn rows(&self) -> usize {
        self.slots.len()
    }
    pub fn columns(&self) -> usize {
        self.slots.first().map_or(0, Vec::len)
    }
    pub fn cells(&self) -> &[HtmlSourceCell] {
        &self.cells
    }
    /// A covered slot returns its origin cell, preserving one observation and one anchor.
    pub fn cell(&self, row: usize, column: usize) -> Option<&HtmlSourceCell> {
        let index = self
            .slots
            .get(row.checked_sub(1)?)?
            .get(column.checked_sub(1)?)?;
        self.cells.get((*index)?)
    }
}

/// Structurally decoded, byte-bound HTML tables. This bounded reader requires explicit table,
/// row, cell and row-group closing tags. It does not render CSS, execute scripts, interpret
/// inline-XBRL scales or claim that a numeric token is a valid financial measurement.
#[derive(Debug, Clone, PartialEq)]
pub struct HtmlDocument {
    artifact_path: String,
    sha256: [u8; 32],
    bytes: Vec<u8>,
    tables: Vec<HtmlSourceTable>,
}

impl HtmlDocument {
    pub fn from_bytes(
        path: impl Into<String>,
        bytes: &[u8],
    ) -> Result<Self, fmine_evidence::Refusal> {
        if bytes.len() > HTML_MAX_BYTES {
            return Err(html_refusal("document exceeds the byte limit"));
        }
        let artifact_path = path.into();
        if artifact_path.is_empty()
            || artifact_path.len() > 4096
            || artifact_path.chars().any(char::is_control)
        {
            return Err(html_refusal("artifact path must be nonempty and bounded"));
        }
        let text = std::str::from_utf8(bytes)
            .map_err(|_| html_refusal("document must be valid UTF-8"))?;
        Ok(Self {
            artifact_path,
            sha256: fmine_core::hash::sha256(bytes).0,
            bytes: bytes.to_vec(),
            tables: html_tables(text)?,
        })
    }

    pub fn artifact_path(&self) -> &str {
        &self.artifact_path
    }
    pub const fn sha256(&self) -> [u8; 32] {
        self.sha256
    }
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
    pub fn tables(&self) -> &[HtmlSourceTable] {
        &self.tables
    }

    /// Table index is zero-based; row and column are one-based, like XLSX coordinates.
    /// Every slot covered by a merged cell produces the same origin locator.
    pub fn make_anchor(
        &self,
        table_index: usize,
        row: usize,
        column: usize,
    ) -> Option<fmine_core::anchor::SourceAnchor> {
        let cell = self.tables.get(table_index)?.cell(row, column)?;
        let column = xlsx::col_index_to_letters(u32::try_from(cell.column).ok()?);
        Some(fmine_core::anchor::SourceAnchor::new(
            self.artifact_path.clone(),
            self.sha256,
            format!("Table{}!{column}{}", table_index + 1, cell.row),
        ))
    }

    fn legacy_tables(&self) -> Vec<HtmlTable> {
        self.tables
            .iter()
            .map(|table| HtmlTable {
                table_index: table.table_index,
                rows: table
                    .slots
                    .iter()
                    .enumerate()
                    .map(|(r, row)| {
                        row.iter()
                            .enumerate()
                            .map(|(c, origin)| {
                                origin
                                    .and_then(|i| table.cells.get(i))
                                    .filter(|cell| cell.row == r + 1 && cell.column == c + 1)
                                    .map_or_else(
                                        || CellValue::Text(String::new()),
                                        |cell| cell.value.clone(),
                                    )
                            })
                            .collect()
                    })
                    .collect(),
            })
            .collect()
    }
}

/// Compatibility projection through the byte-bound parser. Malformed or unsupported documents
/// produce no partial numeric output. Use HtmlDocument::from_bytes to retain typed refusals,
/// exact tokens, spans and source anchors.
#[must_use]
pub fn extract_html_tables(html: &str) -> Vec<HtmlTable> {
    HtmlDocument::from_bytes("inline.html", html.as_bytes())
        .map_or_else(|_| Vec::new(), |document| document.legacy_tables())
}

#[derive(Default)]
struct HtmlBudget {
    cells: usize,
    slots: usize,
}

impl HtmlBudget {
    fn add_slots(&mut self, count: usize) -> HtmlResult<()> {
        let next = self
            .slots
            .checked_add(count)
            .filter(|n| *n <= HTML_MAX_SLOTS)
            .ok_or_else(|| html_refusal("expanded document grid exceeds the slot limit"))?;
        self.slots = next;
        Ok(())
    }
}

struct HtmlOpenCell {
    tag: String,
    row_span: usize,
    column_span: usize,
    start: usize,
    text: String,
    runs: usize,
    plain: bool,
}

impl HtmlOpenCell {
    fn text(&mut self, raw: &str) -> HtmlResult<()> {
        let text = html_entities(raw)?;
        let separator = usize::from(self.runs > 0 && !text.trim().is_empty());
        if self.text.len() + text.len() + separator > HTML_MAX_CELL_BYTES {
            return Err(html_refusal("decoded cell exceeds the byte limit"));
        }
        if !text.trim().is_empty() {
            if separator != 0 {
                self.text.push(' ');
            }
            self.runs += 1;
        }
        self.text.push_str(&text);
        Ok(())
    }
}

struct HtmlBuilder {
    table: HtmlSourceTable,
    declared_rows: usize,
    row: Option<usize>,
    cursor: usize,
    group: Option<String>,
    cell: Option<HtmlOpenCell>,
}

impl HtmlBuilder {
    fn new(table_index: usize) -> Self {
        Self {
            table: HtmlSourceTable {
                table_index,
                cells: Vec::new(),
                slots: Vec::new(),
            },
            declared_rows: 0,
            row: None,
            cursor: 0,
            group: None,
            cell: None,
        }
    }

    fn boundary(&self) -> HtmlResult<()> {
        if self.row.is_some()
            || self.cell.is_some()
            || self.table.slots.len() > self.declared_rows
        {
            Err(html_refusal(
                "unclosed row/cell or rowspan crossing a row-group boundary",
            ))
        } else {
            Ok(())
        }
    }

    fn close_cell(&mut self, end: usize, html: &str, budget: &mut HtmlBudget) -> HtmlResult<()> {
        let cell = self
            .cell
            .take()
            .ok_or_else(|| html_refusal("cell closing tag has no opener"))?;
        let row = self.row.ok_or_else(|| html_refusal("cell has no row"))?;
        if end - cell.start > HTML_MAX_CELL_BYTES || budget.cells >= HTML_MAX_CELLS {
            return Err(html_refusal("source cell count or byte limit exceeded"));
        }
        let mut column = self.cursor;
        while self.table.slots[row]
            .get(column)
            .is_some_and(Option::is_some)
        {
            column += 1;
        }
        let bottom = row
            .checked_add(cell.row_span)
            .filter(|n| *n <= HTML_MAX_ROWS)
            .ok_or_else(|| html_refusal("rowspan exceeds the row limit"))?;
        let right = column
            .checked_add(cell.column_span)
            .filter(|n| *n <= HTML_MAX_COLUMNS)
            .ok_or_else(|| html_refusal("colspan exceeds the column limit"))?;
        // Skip occupied slots only to find the start; an occupied slot inside the span is an error.
        for target in self.table.slots.iter().skip(row).take(cell.row_span) {
            if target
                .iter()
                .skip(column)
                .take(cell.column_span)
                .any(Option::is_some)
            {
                return Err(html_refusal("overlapping table spans"));
            }
        }
        let additional = (row..bottom)
            .map(|r| right.saturating_sub(self.table.slots.get(r).map_or(0, Vec::len)))
            .sum();
        budget.add_slots(additional)?;
        self.table
            .slots
            .resize_with(bottom.max(self.table.slots.len()), Vec::new);
        let index = self.table.cells.len();
        for target in &mut self.table.slots[row..bottom] {
            if right > target.len() {
                target.resize(right, None);
            }
            target[column..right].fill(Some(index));
        }
        let text = cell.text.trim().to_string();
        let (value, number) = html_cell_value(&text, cell.plain && cell.runs <= 1);
        self.table.cells.push(HtmlSourceCell {
            row: row + 1,
            column: column + 1,
            row_span: cell.row_span,
            column_span: cell.column_span,
            byte_range: (cell.start, end),
            raw_html: html[cell.start..end].to_string(),
            text,
            value,
            number,
        });
        self.cursor = right;
        budget.cells += 1;
        Ok(())
    }

    fn finish(mut self, budget: &mut HtmlBudget) -> HtmlResult<HtmlSourceTable> {
        self.boundary()?;
        if self.group.is_some() {
            return Err(html_refusal("row group has no closing tag"));
        }
        let width = self.table.slots.iter().map(Vec::len).max().unwrap_or(0);
        budget.add_slots(self.table.slots.iter().map(|row| width - row.len()).sum())?;
        for row in &mut self.table.slots {
            row.resize(width, None);
        }
        Ok(self.table)
    }
}

struct HtmlTag {
    name: String,
    closing: bool,
    self_closing: bool,
    end: usize,
    attrs: Vec<(String, Option<String>)>,
}

fn html_space(c: char) -> bool {
    matches!(c, '\t' | '\n' | '\u{000c}' | '\r' | ' ')
}

fn html_tag_end(html: &str, start: usize) -> HtmlResult<usize> {
    let bytes = html.as_bytes();
    let mut at = start + 1;
    let mut quote = None;
    while at < bytes.len() {
        let b = bytes[at];
        if at - start > HTML_MAX_CELL_BYTES {
            return Err(html_refusal("tag exceeds the byte limit"));
        }
        if let Some(q) = quote {
            if b == q {
                quote = None;
            }
        } else if b == b'\'' || b == b'"' {
            quote = Some(b);
        } else if b == b'>' {
            break;
        }
        at += 1;
    }
    if at == bytes.len() {
        return Err(html_refusal("unterminated tag or quoted attribute"));
    }
    Ok(at)
}

fn html_tag(html: &str, start: usize) -> HtmlResult<HtmlTag> {
    let at = html_tag_end(html, start)?;
    let body = html[start + 1..at].trim_end_matches(html_space);
    let closing = body.starts_with('/');
    let body = if closing { &body[1..] } else { body };
    let self_closing = body.ends_with('/');
    let body = if self_closing {
        &body[..body.len() - 1]
    } else {
        body
    };
    let length = body
        .bytes()
        .take_while(|b| b.is_ascii_alphanumeric() || b"-_:".contains(b))
        .count();
    if length == 0 || !body.as_bytes()[0].is_ascii_alphabetic() {
        return Err(html_refusal("unsupported or malformed tag name"));
    }
    let name = body[..length].to_ascii_lowercase();
    let rest = &body[length..];
    if !rest.is_empty() && !rest.starts_with(html_space) {
        return Err(html_refusal("tag attributes require HTML whitespace"));
    }
    let mut tail = rest.trim_start_matches(html_space);
    if closing && (!tail.is_empty() || self_closing) {
        return Err(html_refusal("closing tags cannot have attributes"));
    }
    let mut attrs: Vec<(String, Option<String>)> = Vec::new();
    while !tail.is_empty() {
        if attrs.len() >= 128 {
            return Err(html_refusal("too many tag attributes"));
        }
        let n = tail
            .bytes()
            .take_while(|b| b.is_ascii_alphanumeric() || b"-_:".contains(b))
            .count();
        if n == 0 {
            return Err(html_refusal("malformed attribute name"));
        }
        let key = tail[..n].to_ascii_lowercase();
        if attrs.iter().any(|(prior, _)| prior == &key) {
            return Err(html_refusal("duplicate attribute"));
        }
        tail = tail[n..].trim_start_matches(html_space);
        let value = if let Some(rest) = tail.strip_prefix('=') {
            tail = rest.trim_start_matches(html_space);
            let first = tail
                .as_bytes()
                .first()
                .copied()
                .ok_or_else(|| html_refusal("attribute has no value"))?;
            let raw;
            if first == b'"' || first == b'\'' {
                let end = tail[1..]
                    .find(char::from(first))
                    .ok_or_else(|| html_refusal("unterminated attribute"))? + 1;
                raw = &tail[1..end];
                tail = &tail[end + 1..];
                if !tail.is_empty() && !tail.starts_with(html_space) {
                    return Err(html_refusal("attributes require HTML whitespace"));
                }
            } else {
                let end = tail.find(html_space).unwrap_or(tail.len());
                raw = &tail[..end];
                if raw.contains(['"', '\'', '<', '>', '=']) {
                    return Err(html_refusal("malformed unquoted attribute"));
                }
                tail = &tail[end..];
            }
            Some(html_entities(raw)?)
        } else {
            None
        };
        attrs.push((key, value));
        tail = tail.trim_start_matches(html_space);
    }
    Ok(HtmlTag {
        name,
        closing,
        self_closing,
        end: at + 1,
        attrs,
    })
}

fn html_span(tag: &HtmlTag, key: &str, limit: usize) -> HtmlResult<usize> {
    let Some((_, value)) = tag.attrs.iter().find(|(name, _)| name == key) else {
        return Ok(1);
    };
    let value = value
        .as_deref()
        .ok_or_else(|| html_refusal("span has no value"))?;
    if value.is_empty() || !value.bytes().all(|b| b.is_ascii_digit()) {
        return Err(html_refusal("span must be a positive decimal integer"));
    }
    value
        .parse::<usize>()
        .ok()
        .filter(|n| *n > 0 && *n <= limit)
        .ok_or_else(|| html_refusal("zero, overflowing or excessive span"))
}

fn html_hidden(tag: &HtmlTag) -> bool {
    tag.attrs.iter().any(|(key, value)| {
        key == "hidden"
            || (key == "aria-hidden"
                && value
                    .as_deref()
                    .is_some_and(|v| v.trim().eq_ignore_ascii_case("true")))
            || (key == "style"
                && value.as_deref().is_some_and(|v| {
                    let normalized: String = v
                        .chars()
                        .filter(|c| !c.is_ascii_whitespace())
                        .flat_map(char::to_lowercase)
                        .collect();
                    normalized.split(';').any(|rule| {
                        matches!(
                            rule,
                            "display:none"
                                | "display:none!important"
                                | "visibility:hidden"
                                | "visibility:hidden!important"
                        )
                    })
                }))
    })
}

fn html_entities(text: &str) -> HtmlResult<String> {
    let mut result = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find('&') {
        result.push_str(&rest[..at]);
        rest = &rest[at..];
        let Some(end) = rest.as_bytes().iter().take(33).position(|b| *b == b';') else {
            result.push('&');
            rest = &rest[1..];
            continue;
        };
        let entity = &rest[1..end];
        let decoded = match entity {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            "nbsp" => Some('\u{00a0}'),
            "ndash" => Some('–'),
            "mdash" => Some('—'),
            "minus" => Some('−'),
            _ if entity.starts_with('#') => {
                let (digits, radix) = entity[1..]
                    .strip_prefix('x')
                    .or_else(|| entity[1..].strip_prefix('X'))
                    .map_or((&entity[1..], 10), |s| (s, 16));
                if digits.is_empty()
                    || !digits.bytes().all(|b| {
                        if radix == 16 {
                            b.is_ascii_hexdigit()
                        } else {
                            b.is_ascii_digit()
                        }
                    })
                {
                    return Err(html_refusal("invalid numeric character reference"));
                }
                let code = u32::from_str_radix(digits, radix)
                    .ok()
                    .and_then(char::from_u32)
                    .filter(|c| *c != '\0' && (!c.is_control() || matches!(*c, '\t' | '\n' | '\r')))
                    .ok_or_else(|| html_refusal("invalid numeric character reference"))?;
                Some(code)
            }
            _ => None,
        };
        if let Some(c) = decoded {
            result.push(c);
        } else {
            result.push_str(&rest[..=end]);
        }
        rest = &rest[end + 1..];
    }
    result.push_str(rest);
    Ok(result)
}

fn html_cell_value(text: &str, plain: bool) -> (CellValue, Option<numtok::NumberToken>) {
    if matches!(text, "-" | "–" | "—" | "N/A" | "n/a" | "NA") {
        return (CellValue::Placeholder(text.into()), None);
    }
    if plain {
        let tokens = numtok::tokenize(text);
        if let [numtok::Token::Number(number)] = tokens.as_slice()
            && number.span == (0, text.len())
            && number.currency.is_none()
            && number.unit.is_none()
            && !number.percent
            && number.scale_word.is_none()
            && number.hedge.is_none()
            && number.range_with.is_none()
            && let Ok(value) =
                format!("{}e-{}", number.value.mantissa, number.value.scale).parse::<f64>()
            && value.is_finite()
        {
            return (CellValue::Number(value), Some(number.clone()));
        }
    }
    (CellValue::Text(text.into()), None)
}

fn html_tables(html: &str) -> HtmlResult<Vec<HtmlSourceTable>> {
    let lower = html.to_ascii_lowercase();
    let mut tables = Vec::new();
    let mut active: Option<HtmlBuilder> = None;
    let mut budget = HtmlBudget::default();
    let mut at = 0;
    while at < html.len() {
        let start = html[at..].find('<').map_or(html.len(), |n| at + n);
        if let Some(cell) = active.as_mut().and_then(|table| table.cell.as_mut()) {
            cell.text(&html[at..start])?;
        }
        if start == html.len() {
            break;
        }
        if html[start..].starts_with("<!--") {
            let end = html[start + 4..]
                .find("-->")
                .ok_or_else(|| html_refusal("unterminated comment"))?;
            if let Some(cell) = active.as_mut().and_then(|table| table.cell.as_mut()) {
                cell.plain = false;
            }
            at = start + 4 + end + 3;
            continue;
        }
        if lower[start..].starts_with("<!doctype")
            && html
                .as_bytes()
                .get(start + 9)
                .is_some_and(|b| b.is_ascii_whitespace() || *b == b'>')
        {
            if active.is_some() {
                return Err(html_refusal("doctype inside a table"));
            }
            let end = html_tag_end(html, start)?;
            if html[start..end].contains('[') {
                return Err(html_refusal("doctype internal subsets are unsupported"));
            }
            at = end + 1;
            continue;
        }
        let tag = html_tag(html, start)?;
        at = tag.end;
        if matches!(
            tag.name.as_str(),
            "script" | "style" | "textarea" | "title" | "xmp" | "iframe"
        ) && !tag.closing
        {
            let needle = format!("</{}", tag.name);
            let mut cursor = at;
            loop {
                let found = lower[cursor..]
                    .find(&needle)
                    .map(|n| cursor + n)
                    .ok_or_else(|| html_refusal("unterminated raw-text element"))?;
                let closing = html_tag(html, found)?;
                if closing.closing && closing.name == tag.name {
                    at = closing.end;
                    break;
                }
                cursor = closing.end;
            }
            if let Some(cell) = active.as_mut().and_then(|table| table.cell.as_mut()) {
                cell.plain = false;
            }
            continue;
        }
        if html_hidden(&tag)
            || matches!(
                tag.name.as_str(),
                "template" | "noscript" | "plaintext" | "noembed" | "noframes" | "ix:hidden"
            )
        {
            return Err(html_refusal(
                "hidden or conditional content requires explicit review",
            ));
        }
        if tag.name == "table" {
            if tag.self_closing {
                return Err(html_refusal("table cannot be self-closing"));
            }
            if tag.closing {
                let table = active
                    .take()
                    .ok_or_else(|| html_refusal("table has no opener"))?;
                tables.push(table.finish(&mut budget)?);
            } else {
                if active.is_some() || tables.len() >= HTML_MAX_TABLES {
                    return Err(html_refusal("nested table or table-count limit"));
                }
                active = Some(HtmlBuilder::new(tables.len()));
            }
            continue;
        }
        let Some(table) = active.as_mut() else {
            continue;
        };
        match tag.name.as_str() {
            "thead" | "tbody" | "tfoot" => {
                table.boundary()?;
                if tag.self_closing {
                    return Err(html_refusal("row group cannot be self-closing"));
                }
                if tag.closing {
                    if table.group.as_deref() != Some(tag.name.as_str()) {
                        return Err(html_refusal("mismatched row-group closing tag"));
                    }
                    table.group = None;
                } else {
                    if table.group.is_some() {
                        return Err(html_refusal("nested row groups"));
                    }
                    table.group = Some(tag.name);
                }
            }
            "tr" => {
                if tag.self_closing {
                    return Err(html_refusal("row cannot be self-closing"));
                }
                if tag.closing {
                    if table.cell.is_some() || table.row.take().is_none() {
                        return Err(html_refusal("unclosed cell or row without opener"));
                    }
                } else {
                    if table.row.is_some() || table.declared_rows >= HTML_MAX_ROWS {
                        return Err(html_refusal("nested row or row-count limit"));
                    }
                    let row = table.declared_rows;
                    table.declared_rows += 1;
                    if table.table.slots.len() <= row {
                        table.table.slots.push(Vec::new());
                    }
                    table.row = Some(row);
                    table.cursor = 0;
                }
            }
            "td" | "th" => {
                if tag.self_closing {
                    return Err(html_refusal("cell cannot be self-closing"));
                }
                if tag.closing {
                    if table.cell.as_ref().is_none_or(|cell| cell.tag != tag.name) {
                        return Err(html_refusal("mismatched cell closing tag"));
                    }
                    table.close_cell(start, html, &mut budget)?;
                } else {
                    if table.row.is_none() || table.cell.is_some() {
                        return Err(html_refusal("nested cell or cell outside a row"));
                    }
                    table.cell = Some(HtmlOpenCell {
                        row_span: html_span(&tag, "rowspan", HTML_MAX_ROWS)?,
                        column_span: html_span(&tag, "colspan", HTML_MAX_COLUMNS)?,
                        tag: tag.name,
                        start: tag.end,
                        text: String::new(),
                        runs: 0,
                        plain: true,
                    });
                }
            }
            _ => {
                if let Some(cell) = table.cell.as_mut()
                    && !matches!(
                        tag.name.as_str(),
                        "span" | "b" | "strong" | "i" | "em" | "u" | "font" | "p" | "div"
                    )
                {
                    cell.plain = false;
                }
            }
        }
    }
    if active.is_some() {
        return Err(html_refusal("table has no closing tag"));
    }
    Ok(tables)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::styles::NumberFormat;
    use crate::xlsx::{
        MinimalCell, MinimalSheet, MinimalStyles, QuarantineItem, Workbook, build_minimal_xlsx,
        build_minimal_xlsx_styled,
    };
    use fmine_core::hash::sha256;
    use std::path::PathBuf;

    fn cell(r: &str, c: MinimalCell) -> (String, MinimalCell) {
        (r.to_string(), c)
    }

    /// A one-sheet workbook with the given styles part (`None`: no styles part) and sheet XML rows.
    fn styled_workbook(styles: Option<&str>, rows: &str) -> Vec<u8> {
        let wb = r#"<workbook xmlns:r="r"><sheets><sheet name="S" sheetId="1" r:id="rId1"/></sheets></workbook>"#;
        let mut rels = String::from(
            r#"<Relationships><Relationship Id="rId1" Type="http://x/worksheet" Target="worksheets/sheet1.xml"/>"#,
        );
        if styles.is_some() {
            rels.push_str(
                r#"<Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/>"#,
            );
        }
        rels.push_str("</Relationships>");
        let sheet = format!("<worksheet><sheetData>{rows}</sheetData></worksheet>");
        let mut entries: Vec<(&str, &[u8])> = vec![
            ("xl/workbook.xml", wb.as_bytes()),
            ("xl/_rels/workbook.xml.rels", rels.as_bytes()),
            ("xl/worksheets/sheet1.xml", sheet.as_bytes()),
        ];
        if let Some(s) = styles {
            entries.push(("xl/styles.xml", s.as_bytes()));
        }
        zip_inflate::write_stored_zip(&entries)
    }

    const STYLES: &str = r##"<styleSheet><numFmts count="1"><numFmt numFmtId="164" formatCode="#,##0.0&quot;x&quot;"/></numFmts><cellXfs count="3"><xf numFmtId="0"/><xf numFmtId="164"/><xf numFmtId="4"/></cellXfs></styleSheet>"##;

    /// bd-jg43.1 end to end: each cell's `s` resolves through cellXfs; a cell without `s` uses xf 0; an index
    /// past cellXfs stays a bad index; the census counts non-integers on the stored text.
    #[test]
    fn test_cells_carry_style_and_format() {
        let rows = r#"<row r="1"><c r="A1" s="1"><v>-37.899642999999998</v></c><c r="B1"><v>12</v></c><c r="C1" s="2"><v>1.5E+1</v></c><c r="D1" s="7"><v>3</v></c><c r="E1" s="1"><v>2.5</v></c></row>"#;
        let wb = Workbook::from_bytes("styled.xlsx", &styled_workbook(Some(STYLES), rows))
            .expect("styled workbook");
        let s = &wb.sheets[0];
        let a1 = s.cell(1, 1).expect("A1");
        assert_eq!(a1.style, Some(1));
        assert_eq!(
            a1.format,
            styles::NumberFormat::Custom {
                id: 164,
                code: "#,##0.0\"x\"".to_string()
            }
        );
        assert_eq!(
            s.cell(1, 2).map(|c| (c.style, c.format.clone())),
            Some((None, styles::NumberFormat::Builtin(0)))
        );
        assert_eq!(
            s.cell(1, 3).map(|c| c.format.code()),
            Some("#,##0.00".to_string())
        );
        assert_eq!(
            s.cell(1, 4).map(|c| c.format.clone()),
            Some(styles::NumberFormat::BadStyleIndex(7))
        );
        let census = wb.format_census();
        let count = |code: &str| census.get(code).map(|c| (c.numeric, c.non_integer));
        assert_eq!(count("#,##0.0\"x\""), Some((2, 2)));
        assert_eq!(count("General"), Some((1, 0)));
        assert_eq!(count("#,##0.00"), Some((1, 0)), "1.5E+1 is the integer 15");
        assert_eq!(count("bad style index 7"), Some((1, 0)));
    }

    /// bd-jg43.1: the stored text survives verbatim (E823 K8).
    #[test]
    fn test_raw_text_preserved_exactly() {
        let rows = r#"<row r="1"><c r="A1"><v>-37.899642999999998</v></c></row>"#;
        let wb = Workbook::from_bytes("raw.xlsx", &styled_workbook(None, rows)).expect("workbook");
        assert_eq!(
            wb.sheets[0]
                .cell(1, 1)
                .and_then(|c| c.number_text.as_deref()),
            Some("-37.899642999999998")
        );
    }

    /// bd-jg43.1: with no styles part every cell is built-in 0, whatever its `s`.
    #[test]
    fn test_workbook_without_styles_is_general() {
        let rows = r#"<row r="1"><c r="A1" s="5"><v>1</v></c><c r="B1"><v>2</v></c></row>"#;
        let wb =
            Workbook::from_bytes("plain.xlsx", &styled_workbook(None, rows)).expect("workbook");
        for col in [1, 2] {
            assert_eq!(
                wb.sheets[0].cell(1, col).map(|c| c.format.clone()),
                Some(styles::NumberFormat::Builtin(0))
            );
        }
        assert_eq!(wb.sheets[0].cell(1, 1).and_then(|c| c.style), Some(5));
    }

    /// bd-jg43.1 negative control: `cs="9"` must not be read as `s`; a non-integer `s` refuses the workbook.
    /// Each entity is decoded once: `&amp;lt;` is the text `&lt;`, not `<`.
    #[test]
    fn test_xml_entities_decode_once() {
        use crate::xlsx::decode_xml_entities;
        assert_eq!(decode_xml_entities("&amp;lt;"), "&lt;");
        assert_eq!(decode_xml_entities("&amp;amp;"), "&amp;");
        assert_eq!(
            decode_xml_entities("Cost &amp; Sales &lt;1&gt;"),
            "Cost & Sales <1>"
        );
    }

    #[test]
    fn test_extract_attr_exact_ignores_suffix_match() {
        assert_eq!(xlsx::extract_attr_exact(r#"<c r="A1" cs="9">"#, "s"), None);
        assert_eq!(
            xlsx::extract_attr_exact(r#"<c r="A1" cs="9" s='2'>"#, "s").as_deref(),
            Some("2")
        );
        assert_eq!(
            xlsx::extract_attr_exact("<c r=\"A1\"\n\ts = \"3\">", "s").as_deref(),
            Some("3")
        );
        let rows = r#"<row r="1"><c r="A1" cs="9"><v>1</v></c></row>"#;
        let wb = Workbook::from_bytes("cs.xlsx", &styled_workbook(Some(STYLES), rows))
            .expect("workbook");
        assert_eq!(wb.sheets[0].cell(1, 1).and_then(|c| c.style), None);
        let bad = r#"<row r="1"><c r="A1" s="x"><v>1</v></c></row>"#;
        assert!(Workbook::from_bytes("bad.xlsx", &styled_workbook(Some(STYLES), bad)).is_err());
    }

    /// BD-321: a missing workbook is an error that names the path, even under `corpus_of_input_docs/`
    /// (where the removed fallback used to substitute an in-code stand-in).
    #[test]
    fn test_missing_file_is_an_error_never_a_stand_in() {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("corpus_of_input_docs")
            .join("no-such-workbook.xlsx");
        match Workbook::from_file(&path) {
            Err(IoError::FileRead { path: p, .. }) => assert!(p.ends_with("no-such-workbook.xlsx")),
            other => panic!("expected FileRead, got {other:?}"),
        }
    }

    /// The styled writer adds nothing when given no styles (bd-jg43.3): the digest is the one HEAD's writer
    /// (dfc23c5, before `build_minimal_xlsx_styled` existed) gives for this input, computed on that tree.
    #[test]
    fn test_minimal_xlsx_without_styles_byte_identical() {
        let sheets = [
            MinimalSheet {
                name: "Q1 2026".to_string(),
                hidden: false,
                cells: vec![
                    cell(
                        "A1",
                        MinimalCell::Text("Cost of sales & <other>".to_string()),
                    ),
                    cell("B2", MinimalCell::Number(-37.899_643)),
                    cell("C3", MinimalCell::Error("#REF!".to_string())),
                    cell("AA10", MinimalCell::Number(2008.0)),
                ],
            },
            MinimalSheet {
                name: "Hidden".to_string(),
                hidden: true,
                cells: vec![],
            },
        ];
        let bytes = build_minimal_xlsx(&sheets);
        assert_eq!(bytes.len(), 1618);
        assert_eq!(
            sha256(&bytes).to_hex(),
            "88255e9f00ef93004c46bc33063ad34db3b0d0d0a486c67cc53f81fa4c96ff36"
        );
        assert_eq!(build_minimal_xlsx_styled(&sheets, None), bytes);
    }

    /// The styles part the writer adds is read back: custom and built-in formats by cell style, the stored text
    /// kept, and the displayed values derived from it; an unstyled cell stays General.
    #[test]
    fn test_minimal_xlsx_styled_round_trip_formats() {
        let styles = MinimalStyles {
            num_fmts: vec![(164, "#,##0;(#,##0);\"-\"".to_string())],
            xfs: vec![0, 3, 164, 14, 4],
        };
        let styled = |raw: &str, xf| MinimalCell::Styled {
            raw: raw.to_string(),
            xf,
        };
        let bytes = build_minimal_xlsx_styled(
            &[MinimalSheet {
                name: "S".to_string(),
                hidden: false,
                cells: vec![
                    cell("A1", styled("-37.899642999999998", 1)),
                    cell("A2", styled("-20", 2)),
                    cell("A3", styled("0", 2)),
                    cell("A4", styled("45000", 3)),
                    cell("A5", styled("2.5", 4)),
                    cell("A6", MinimalCell::Number(1.5)),
                ],
            }],
            Some(&styles),
        );
        let wb = Workbook::from_bytes("styled.xlsx", &bytes).expect("parse styled workbook");
        let s = wb.sheet("S").expect("sheet");
        let c = |row| s.cell(row, 1).expect("cell");
        assert_eq!(c(1).format, NumberFormat::Builtin(3));
        assert_eq!(c(1).number_text.as_deref(), Some("-37.899642999999998"));
        assert_eq!(
            c(2).format,
            NumberFormat::Custom {
                id: 164,
                code: "#,##0;(#,##0);\"-\"".to_string()
            }
        );
        assert_eq!(c(4).format, NumberFormat::Builtin(14));
        assert_eq!(c(6).format, NumberFormat::Builtin(0));
        let shown = |row| match c(row).displayed() {
            Some(Ok(d)) => d.text(),
            other => format!("{other:?}"),
        };
        assert_eq!(shown(1), "-38");
        assert_eq!(shown(2), "-20");
        assert_eq!(shown(3), "-");
        assert_eq!(shown(5), "2.50");
        let code = |row| c(row).displayed().and_then(Result::err).map(|r| r.code);
        assert_eq!(code(4), Some(numfmt::UNSUPPORTED_FORMAT));
        assert_eq!(code(6), Some(numfmt::GENERAL_NON_INTEGER));
    }

    /// Writer → parser round trip on a synthetic workbook: typed cells, G-BULL-1/2 quarantine, the cell census
    /// and the canonical numeric digest (known answer computed from the fixed listing format).
    #[test]
    fn test_minimal_xlsx_round_trip_census_and_digest() {
        let bytes = build_minimal_xlsx(&[
            MinimalSheet {
                name: "Synthetic A&B".to_string(),
                hidden: false,
                cells: vec![
                    cell("A1", MinimalCell::Text("Label <one>".to_string())),
                    cell("B1", MinimalCell::Number(42.0)),
                    cell("C1", MinimalCell::Number(-0.125)),
                    cell("B2", MinimalCell::Text("—".to_string())),
                    cell("B3", MinimalCell::Text("— ".to_string())),
                    cell("C2", MinimalCell::Error("#REF!".to_string())),
                    cell("AA10", MinimalCell::Number(0.00001)),
                ],
            },
            MinimalSheet {
                name: "Hidden scratch".to_string(),
                hidden: true,
                cells: vec![cell("A1", MinimalCell::Number(7.0))],
            },
        ]);
        let wb = Workbook::from_bytes("synthetic.xlsx", &bytes).expect("parse synthetic workbook");
        let sheet = wb.sheet("Synthetic A&B").expect("visible sheet");
        assert_eq!(
            sheet.cell(1, 1).map(|c| c.value.clone()),
            Some(CellValue::Text("Label <one>".into()))
        );
        assert_eq!(
            sheet.cell(1, 2).map(|c| c.value.clone()),
            Some(CellValue::Number(42.0))
        );
        assert_eq!(
            sheet.cell(2, 2).map(|c| c.value.clone()),
            Some(CellValue::Placeholder("—".into()))
        );
        // The stored text of a number is kept beside the value; other kinds carry none.
        assert_eq!(
            sheet.cell(1, 3).and_then(|c| c.number_text.as_deref()),
            Some("-0.125")
        );
        assert_eq!(
            sheet.cell(2, 2).map(|c| c.number_text.is_none()),
            Some(true)
        );
        // Regression (bd-gwf1.1): the trailing space of the issuer's `'— '` token is kept, not trimmed.
        assert_eq!(
            sheet.cell(3, 2).map(|c| c.value.clone()),
            Some(CellValue::Placeholder("— ".into()))
        );
        assert_eq!(
            sheet.cell(10, 27).map(|c| c.cell_ref.as_str()),
            Some("AA10")
        );
        assert!(wb.sheet("Hidden scratch").is_none());
        assert!(wb.quarantined.contains(&QuarantineItem::HiddenSheet {
            sheet_name: "Hidden scratch".into(),
            state: "hidden".into()
        }));
        assert!(wb.quarantined.contains(&QuarantineItem::FormulaErrorCell {
            sheet_name: "Synthetic A&B".into(),
            cell_ref: "C2".into(),
            error_token: "#REF!".into()
        }));

        let census = wb.cell_census();
        assert_eq!(
            (
                census.visible_sheets,
                census.hidden_sheets,
                census.numeric_cells
            ),
            (1, 1, 3)
        );
        assert_eq!(
            (
                census.placeholder_cells,
                census.formula_error_cells,
                census.text_cells
            ),
            (2, 1, 1)
        );
        let listing =
            "Synthetic A&B\tB1\t42\nSynthetic A&B\tC1\t-0.125\nSynthetic A&B\tAA10\t0.00001\n";
        assert_eq!(census.numeric_digest, sha256(listing.as_bytes()).0);
        assert_eq!(xlsx::col_index_to_letters(27), "AA");
        assert_eq!(xlsx::col_index_to_letters(703), "AAA");
    }

    #[test]
    fn test_html_table_extractor_and_placeholder_typing() {
        let html = r#"
        <table>
          <tr><th>Site</th><th>Tonnes</th><th>Grade</th></tr>
          <tr><td>Carlin</td><td>1,250.5</td><td>-</td></tr>
          <tr><td>Cortez</td><td>(45.0)</td><td>N/A</td></tr>
        </table>
        "#;
        let tables = extract_html_tables(html);
        assert_eq!(tables.len(), 1);
        assert_eq!(tables[0].rows.len(), 3);
        assert_eq!(tables[0].rows[1][1], CellValue::Number(1250.5));
        assert_eq!(
            tables[0].rows[1][2],
            CellValue::Placeholder("-".to_string())
        );
        assert_eq!(tables[0].rows[2][1], CellValue::Number(-45.0));
        assert_eq!(
            tables[0].rows[2][2],
            CellValue::Placeholder("N/A".to_string())
        );
    }
    #[test]
    fn html_merged_slots_keep_one_origin_and_one_byte_bound_anchor() {
        let html = "<p>Éléonore</p><table><tr><th rowspan=\"2\">Site</th>\
            <th colspan=\"2\">Gold</th></tr><tr><th>Q1</th><th>Q2</th></tr>\
            <tr><td>A</td><td colspan=\"2\">12</td></tr></table>";
        let document = HtmlDocument::from_bytes("filing.html", html.as_bytes()).expect("table");
        assert_eq!(document.bytes(), html.as_bytes());
        let table = &document.tables()[0];
        assert_eq!((table.rows(), table.columns(), table.cells().len()), (3, 3, 6));
        assert_eq!(
            table.cells().iter().filter(|c| c.number().is_some()).count(),
            1
        );
        assert!(std::ptr::eq(
            table.cell(3, 2).expect("origin"),
            table.cell(3, 3).expect("covered")
        ));
        let anchor = document.make_anchor(0, 3, 2).expect("anchor");
        assert_eq!(anchor.artifact_path, "filing.html");
        assert_eq!(anchor.artifact_sha256, sha256(html.as_bytes()).0);
        assert_eq!(anchor.locator, "Table1!B3");
        assert_eq!(document.make_anchor(0, 3, 3), Some(anchor));
        assert!(table.cell(0, 1).is_none());
        assert!(document.make_anchor(0, 4, 1).is_none());
        let legacy = extract_html_tables(html);
        assert_eq!(legacy[0].rows[2][1], CellValue::Number(12.0));
        assert_eq!(legacy[0].rows[2][2], CellValue::Text(String::new()));
        assert_eq!(
            legacy[0]
                .rows
                .iter()
                .flatten()
                .filter(|v| matches!(v, CellValue::Number(_)))
                .count(),
            1
        );
    }

    #[test]
    fn html_exact_lexemes_entities_and_utf8_ranges_survive_ingestion() {
        let html = "<p>Éléonore</p><table title=\"x </table> > y\"><tr>\
            <td>1&#44;250.50</td><td>&amp;#49;</td><td>&lt;table&gt;</td>\
            <td></td><td>—</td></tr></table>";
        let document = HtmlDocument::from_bytes("filing.html", html.as_bytes()).expect("table");
        let cell = document.tables()[0].cell(1, 1).expect("numeric cell");
        let (start, end) = cell.byte_range();
        assert_eq!(&html[start..end], "1&#44;250.50");
        assert_eq!(cell.raw_html(), "1&#44;250.50");
        assert_eq!(cell.text(), "1,250.50");
        let number = cell.number().expect("exact token");
        assert_eq!(number.raw, "1,250.50");
        assert_eq!(
            number.value,
            numtok::Dec {
                mantissa: 125050,
                scale: 2,
            }
        );
        assert_eq!(
            number.step,
            numtok::Dec {
                mantissa: 1,
                scale: 2,
            }
        );
        assert_eq!(cell.value(), &CellValue::Number(1250.5));
        let table = &document.tables()[0];
        assert_eq!(table.cell(1, 2).expect("one pass").text(), "&#49;");
        assert!(table.cell(1, 2).expect("one pass").number().is_none());
        assert_eq!(table.cell(1, 3).expect("literal markup").text(), "<table>");
        assert_eq!(
            table.cell(1, 4).expect("empty").value(),
            &CellValue::Text(String::new())
        );
        assert_eq!(
            table.cell(1, 5).expect("dash").value(),
            &CellValue::Placeholder("—".into())
        );
        let changed = html.replace("title=\"x", "title=\"z");
        let changed =
            HtmlDocument::from_bytes("filing.html", changed.as_bytes()).expect("changed title");
        assert_ne!(document.sha256(), changed.sha256());
        assert_eq!(
            cell.number(),
            changed.tables()[0].cell(1, 1).expect("same number").number()
        );
    }

    #[test]
    fn html_comments_raw_text_and_exact_tag_names_cannot_invent_tables() {
        let html = "<!-- <table><tr><td>999</td></tr></table> -->\
            <script>const x = '<table><tr><td>888</td></tr></table>';</script>\
            <style>p:after { content: '<table>777</table>'; }</style>\
            <textarea><table><tr><td>666</td></tr></table></textarea>\
            <tableau><trailer><tdx>555</tdx></trailer></tableau>\
            <table><tbody><tr><td><b>12.00</b></td></tr></tbody></table>";
        let document = HtmlDocument::from_bytes("filing.html", html.as_bytes()).expect("one table");
        assert_eq!(document.tables().len(), 1);
        assert_eq!(document.tables()[0].cells().len(), 1);
        assert_eq!(
            document.tables()[0].cell(1, 1).expect("cell").value(),
            &CellValue::Number(12.0)
        );
    }

    #[test]
    fn html_annotations_split_runs_and_malformed_numbers_never_promote() {
        for raw in [
            "12<sup>3</sup>",
            "1<br>2",
            "<span>100</span><span>1</span>",
            "1,2",
            "1 2",
            "$4",
            "$$4",
            "(-12)",
            "1,234,56",
            "4%",
            "2 million",
            "US$4",
            "12<ix:nonfraction scale=\"3\">3</ix:nonfraction>",
        ] {
            let html = format!("<table><tr><td>{raw}</td></tr></table>");
            let document =
                HtmlDocument::from_bytes("filing.html", html.as_bytes()).expect("text cell");
            let cell = document.tables()[0].cell(1, 1).expect("cell");
            assert_eq!(cell.raw_html(), raw);
            assert!(cell.number().is_none(), "{raw}");
            assert!(matches!(cell.value(), CellValue::Text(_)), "{raw}");
        }
    }

    #[test]
    fn html_malformed_spans_groups_and_structure_refuse_without_partial_tables() {
        let valid = "<table><tr><td>1</td></tr></table>";
        for bad in [
            "<table><tr><td>A</td><td rowspan=\"2\">B</td></tr>\
             <tr><td colspan=\"2\">C</td></tr></table>",
            "<table><thead><tr><th rowspan=\"2\">A</th></tr></thead>\
             <tbody><tr><td>1</td></tr></tbody></table>",
            "<table><tr><td rowspan=\"2\">1</td></tr></table>",
            "<table><tr><td rowspan=\"0\">1</td></tr></table>",
            "<table><tr><td colspan=\"-1\">1</td></tr></table>",
            "<table><tr><td colspan=\"1.5\">1</td></tr></table>",
            "<table><tr><td colspan=\"2\" COLSPAN=\"3\">1</td></tr></table>",
            "<table><tr><td colspan=\"999999999999999999999999\">1</td></tr></table>",
            "<table><tr><td>1</th></tr></table>",
            "<table><tr><td>1</td></table>",
            "<table><tr><td><table></table></td></tr></table>",
            "< table><tr><td>1</td></tr></table>",
            "<table\u{00a0}id=\"x\"><tr><td>1</td></tr></table>",
            "<table><tr><td colspan=\"1\"rowspan=\"1\">1</td></tr></table>",
        ] {
            let html = format!("{valid}{bad}");
            let refusal = HtmlDocument::from_bytes("filing.html", html.as_bytes()).expect_err(bad);
            assert_eq!(refusal.code, "FMINE-REF-208");
            assert!(extract_html_tables(&html).is_empty(), "{bad}");
        }
    }

    #[test]
    fn html_hidden_content_and_invalid_encodings_require_review() {
        for html in [
            "<plaintext><table><tr><td>999</td></tr></table>",
            "<noembed><table><tr><td>999</td></tr></table></noembed>",
            "<noframes><table><tr><td>999</td></tr></table></noframes>",
            "<ix:hidden><table><tr><td>999</td></tr></table></ix:hidden>",
            "<div hidden><table><tr><td>1</td></tr></table></div>",
            "<table><tr><td aria-hidden=\"true\">1</td></tr></table>",
            "<table><tr><td style=\"display: none !important\">1</td></tr></table>",
            "<table><tr><td>&#xD800;</td></tr></table>",
            "<table><tr><td>&#x110000;</td></tr></table>",
            "<table><tr><td>&#0;</td></tr></table>",
            "<table><tr><td>&#+49;</td></tr></table>",
            "<table><tr><td>&#x+31;</td></tr></table>",
        ] {
            assert!(
                HtmlDocument::from_bytes("filing.html", html.as_bytes()).is_err(),
                "{html}"
            );
        }
        assert!(HtmlDocument::from_bytes("filing.html", &[0xff]).is_err());
        assert!(
            HtmlDocument::from_bytes("filing.html", &vec![b'x'; HTML_MAX_BYTES + 1]).is_err()
        );
    }

    #[test]
    fn html_expansion_budget_counts_padding_globally() {
        let mut table =
            String::from("<table><tr><td rowspan=\"200\" colspan=\"3000\">1</td></tr>");
        for _ in 1..200 {
            table.push_str("<tr></tr>");
        }
        table.push_str("</table>");
        let first =
            HtmlDocument::from_bytes("filing.html", table.as_bytes()).expect("600000 slots");
        assert_eq!(
            (first.tables()[0].rows(), first.tables()[0].columns()),
            (200, 3000)
        );
        let twice = format!("{table}{table}");
        assert!(HtmlDocument::from_bytes("filing.html", twice.as_bytes()).is_err());
        // A late wide row allocates padding in every earlier row, not only its own occupied slots.
        let mut sparse = String::from("<table>");
        for _ in 0..250 {
            sparse.push_str("<tr><td>1</td></tr>");
        }
        sparse.push_str("<tr><td colspan=\"4096\">2</td></tr></table>");
        assert!(HtmlDocument::from_bytes("filing.html", sparse.as_bytes()).is_err());
    }

}
