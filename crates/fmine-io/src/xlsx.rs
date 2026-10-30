//! Label-anchored `.xlsx` workbook and worksheet extractor (`BD-057`, `BD-271`, `G-BULL-1`, `G-BULL-2`).

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use fmine_core::anchor::SourceAnchor;
use fmine_core::hash::sha256;

use crate::IoError;
use crate::numfmt::{self, DisplayRefusal, Displayed};
use crate::styles::{NumberFormat, Styles, decimal_is_non_integer, parse_styles, styles_part_name};
use crate::zip_inflate::{read_zip_entries, write_stored_zip};

/// Typed cell value preserving the distinction between `0.0`, `Placeholder` (`"-"`/`"N/A"`),
/// and `FormulaError` (`"#REF!"`) (`G-BULL-1`, `G-BULL-2`, `BD-057`).
#[derive(Debug, Clone, PartialEq)]
pub enum CellValue {
    /// Clean numeric scalar.
    Number(f64),
    /// Explicit disclosure placeholder (`"-"`, `"–"`, `"—"`, `"N/A"`, `"n/a"`), with the token exactly as stored,
    /// surrounding whitespace included (`"— "` and `"—"` stay distinct).
    Placeholder(String),
    /// Spreadsheet formula error (`"#REF!"`, `"#DIV/0!"`, `"#VALUE!"`, `"#N/A"`).
    FormulaError(String),
    /// Non-empty text label or header.
    Text(String),
}

impl CellValue {
    /// Returns `Some(f64)` if and only if this cell is `CellValue::Number`.
    #[must_use]
    pub const fn as_number(&self) -> Option<f64> {
        match *self {
            Self::Number(v) => Some(v),
            _ => None,
        }
    }

    /// Returns `Some(&str)` if this cell is `Text`.
    #[must_use]
    pub fn as_text(&self) -> Option<&str> {
        match self {
            Self::Text(s) => Some(s.as_str()),
            _ => None,
        }
    }
}

/// Single extracted worksheet cell with 1-based `(row, col)` and Excel `cell_ref` (`"N14"`).
#[derive(Debug, Clone, PartialEq)]
pub struct ExtractedCell {
    pub cell_ref: String,
    pub row: u32,
    pub col: u32,
    pub value: CellValue,
    pub formula: Option<String>,
    /// For a `CellValue::Number`, the value's text exactly as stored in `<v>` (surrounding whitespace removed),
    /// e.g. `"0"`, `"2026"` or `"1.25"`; `None` for every other kind. Readers that must tell an integer-typed
    /// cell from a float-typed one, or report the printed token, use this instead of re-formatting the `f64`.
    pub number_text: Option<String>,
    /// The cell's `s` attribute (an index into `cellXfs`), if it has one (bd-jg43.1).
    pub style: Option<u32>,
    /// The number format the style resolves to; built-in 0 (General) when the workbook has no styles part.
    pub format: NumberFormat,
}

/// Numeric cells of one number format in `Workbook::format_census`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FormatCount {
    pub numeric: usize,
    /// Numeric cells whose stored decimal text is not an integer (decided exactly, never on the `f64`).
    pub non_integer: usize,
}

impl ExtractedCell {
    /// For a numeric cell, its displayed value under its number format, or why it has none (bd-jg43.2);
    /// `None` for every other kind.
    #[must_use]
    pub fn displayed(&self) -> Option<Result<Displayed, DisplayRefusal>> {
        let CellValue::Number(_) = self.value else {
            return None;
        };
        Some(match &self.number_text {
            Some(raw) => numfmt::display(raw, &self.format),
            None => Err(DisplayRefusal {
                code: numfmt::BAD_STORED_TEXT,
                reason: "numeric cell without stored text".to_string(),
            }),
        })
    }
}

/// Displayed values over the visible sheets (bd-jg43.2): the digest of every numeric cell's displayed text and
/// how the cells divide into shown numbers, zero tokens and refusals.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DisplayedCensus {
    /// SHA-256 over `"{sheet}\t{cell_ref}\t{text}\n"` for every numeric cell, in `CellCensus::numeric_digest`'s
    /// order. `text` is `Displayed::text` or `REFUSED:<code>`.
    pub digest: [u8; 32],
    pub shown: usize,
    pub token: usize,
    pub refused: usize,
    /// Each refusal as `"{sheet}!{cell_ref}: {code}: {reason}"`; the reason names the format code.
    pub refusals: Vec<String>,
}

/// Item quarantined by `G-BULL-1` (`BD-271`) during workbook ingestion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QuarantineItem {
    /// Hidden or veryHidden worksheet (e.g. external-link scratch tab).
    HiddenSheet { sheet_name: String, state: String },
    /// Broken formula cell (`#REF!`, `#VALUE!`, `#DIV/0!`).
    FormulaErrorCell {
        sheet_name: String,
        cell_ref: String,
        error_token: String,
    },
}

/// Extracted visible worksheet from an `.xlsx` workbook.
#[derive(Debug, Clone, PartialEq)]
pub struct Sheet {
    pub name: String,
    pub cells_by_coord: BTreeMap<(u32, u32), ExtractedCell>,
}

impl Sheet {
    /// Looks up a cell by 1-based `(row, col)`.
    #[must_use]
    pub fn cell(&self, row: u32, col: u32) -> Option<&ExtractedCell> {
        self.cells_by_coord.get(&(row, col))
    }

    /// Returns all cells on 1-based `row` ordered by ascending column.
    #[must_use]
    pub fn row_cells(&self, row: u32) -> Vec<&ExtractedCell> {
        self.cells_by_coord
            .range((row, 0)..=(row, u32::MAX))
            .map(|(_, c)| c)
            .collect()
    }

    /// Finds all 1-based row numbers whose text cells in columns `1..=4` match `needle` (case-insensitive substring).
    #[must_use]
    pub fn find_rows_by_label(&self, needle: &str) -> Vec<u32> {
        let needle_lower = needle.to_ascii_lowercase();
        let mut rows = Vec::new();
        for (&(r, c), cell) in &self.cells_by_coord {
            if c <= 4
                && let CellValue::Text(t) = &cell.value
                && t.to_ascii_lowercase().contains(&needle_lower)
                && !rows.contains(&r)
            {
                rows.push(r);
            }
        }
        rows
    }
}

/// Extracted `.xlsx` workbook with SHA-256 digest and `G-BULL-1` quarantine log (`BD-057`, `BD-271`).
/// Equality compares every field, so an edit to a parsed cell makes two workbooks unequal even when their
/// `sha256` (of the original bytes) agrees.
#[derive(Debug, Clone, PartialEq)]
pub struct Workbook {
    pub artifact_path: String,
    pub sha256: [u8; 32],
    /// Visible worksheets, in workbook order.
    pub sheets: Vec<Sheet>,
    /// Hidden and very hidden worksheets, parsed the same way (BD-331.S2). Never in `cell_census`, the
    /// numeric digest, `sheet()` or the quarantine log's cell items.
    pub hidden_sheets: Vec<Sheet>,
    pub quarantined: Vec<QuarantineItem>,
}

/// Per-workbook cell counts over visible sheets, plus the canonical digest of every numeric cell
/// (`GATE-BULL-01` part 1: extraction fidelity is measured by comparing these with an independent reader).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CellCensus {
    pub visible_sheets: usize,
    pub hidden_sheets: usize,
    pub numeric_cells: usize,
    pub placeholder_cells: usize,
    pub formula_error_cells: usize,
    pub text_cells: usize,
    /// SHA-256 over `"{sheet}\t{cell_ref}\t{value}\n"` for every numeric cell, sheets in workbook order and
    /// cells in `(row, col)` order; `value` is the shortest round-trip decimal in positional notation (Rust
    /// `f64` `Display`). DESIGN: the format is fixed so a reader written independently can reproduce it.
    pub numeric_digest: [u8; 32],
}

impl Workbook {
    /// Reads and parses an `.xlsx` file from disk. A missing or unreadable file is an error naming the
    /// path; there is no fallback content of any kind (`BD-321`).
    pub fn from_file(path: &Path) -> Result<Self, IoError> {
        let bytes = fs::read(path).map_err(|e| IoError::FileRead {
            path: path.display().to_string(),
            detail: e.to_string(),
        })?;
        Self::from_bytes(path.display().to_string(), &bytes)
    }

    /// Counts cells by kind over the visible sheets and computes the canonical numeric-cell digest.
    #[must_use]
    pub fn cell_census(&self) -> CellCensus {
        let mut census = CellCensus {
            visible_sheets: self.sheets.len(),
            hidden_sheets: self
                .quarantined
                .iter()
                .filter(|q| matches!(q, QuarantineItem::HiddenSheet { .. }))
                .count(),
            numeric_cells: 0,
            placeholder_cells: 0,
            formula_error_cells: 0,
            text_cells: 0,
            numeric_digest: [0; 32],
        };
        let mut listing = String::new();
        for sheet in &self.sheets {
            for cell in sheet.cells_by_coord.values() {
                match &cell.value {
                    CellValue::Number(v) => {
                        census.numeric_cells += 1;
                        listing.push_str(&format!("{}\t{}\t{v}\n", sheet.name, cell.cell_ref));
                    }
                    CellValue::Placeholder(_) => census.placeholder_cells += 1,
                    CellValue::FormulaError(_) => census.formula_error_cells += 1,
                    CellValue::Text(_) => census.text_cells += 1,
                }
            }
        }
        census.numeric_digest = sha256(listing.as_bytes()).0;
        census
    }

    /// Numeric cells of the visible sheets by number-format code (`NumberFormat::code`), each with how many
    /// store a non-integer decimal (bd-jg43.1). `cell_census` is unaffected.
    #[must_use]
    pub fn format_census(&self) -> BTreeMap<String, FormatCount> {
        let mut out: BTreeMap<String, FormatCount> = BTreeMap::new();
        for cell in self.sheets.iter().flat_map(|s| s.cells_by_coord.values()) {
            if let CellValue::Number(_) = cell.value {
                let count = out.entry(cell.format.code()).or_default();
                count.numeric += 1;
                if cell
                    .number_text
                    .as_deref()
                    .and_then(decimal_is_non_integer)
                    .unwrap_or(false)
                {
                    count.non_integer += 1;
                }
            }
        }
        out
    }

    /// The displayed value of every numeric cell of the visible sheets (bd-jg43.2). `cell_census` is unaffected.
    #[must_use]
    pub fn displayed_census(&self) -> DisplayedCensus {
        let mut census = DisplayedCensus {
            digest: [0; 32],
            shown: 0,
            token: 0,
            refused: 0,
            refusals: Vec::new(),
        };
        let mut listing = String::new();
        for sheet in &self.sheets {
            for cell in sheet.cells_by_coord.values() {
                let Some(displayed) = cell.displayed() else {
                    continue;
                };
                let text = match displayed {
                    Ok(d) => {
                        if d.token.is_some() {
                            census.token += 1;
                        } else {
                            census.shown += 1;
                        }
                        d.text()
                    }
                    Err(r) => {
                        census.refused += 1;
                        census.refusals.push(format!(
                            "{}!{}: {}: {}",
                            sheet.name, cell.cell_ref, r.code, r.reason
                        ));
                        format!("REFUSED:{}", r.code)
                    }
                };
                listing.push_str(&format!("{}\t{}\t{text}\n", sheet.name, cell.cell_ref));
            }
        }
        census.digest = sha256(listing.as_bytes()).0;
        census
    }

    /// Parses an `.xlsx` workbook from in-memory bytes and computes its SHA-256 digest.
    pub fn from_bytes(artifact_path: impl Into<String>, bytes: &[u8]) -> Result<Self, IoError> {
        let artifact_path = artifact_path.into();
        let digest = sha256(bytes).0;
        let entries = read_zip_entries(bytes).map_err(|e| IoError::Zip(e.to_string()))?;

        let shared_strings = if let Some(ss_bytes) = entries.get("xl/sharedStrings.xml") {
            let ss_xml = String::from_utf8_lossy(ss_bytes);
            parse_shared_strings(&ss_xml)
        } else {
            Vec::new()
        };

        let wb_xml = entries
            .get("xl/workbook.xml")
            .ok_or_else(|| IoError::Xml("missing xl/workbook.xml".to_string()))?;
        let wb_text = String::from_utf8_lossy(wb_xml);

        let rels_text = entries
            .get("xl/_rels/workbook.xml.rels")
            .map(|b| String::from_utf8_lossy(b).into_owned())
            .unwrap_or_default();
        let rel_targets = parse_workbook_rels(&rels_text);
        // The styles part (bd-jg43.1): the one the relationships name, else xl/styles.xml; with neither, every
        // cell is General. A styles part that does not parse refuses the workbook.
        let styles_key = styles_part_name(&rels_text)
            .filter(|k| entries.contains_key(k))
            .or_else(|| {
                entries
                    .contains_key("xl/styles.xml")
                    .then(|| "xl/styles.xml".to_string())
            });
        let styles = styles_key
            .and_then(|k| entries.get(&k))
            .map(|b| parse_styles(&String::from_utf8_lossy(b)))
            .transpose()?;

        let mut sheets = Vec::new();
        let mut hidden_sheets = Vec::new();
        let mut quarantined = Vec::new();
        let zip_key_of = |rel_id: &str, fallback: usize| {
            let target = rel_targets
                .get(rel_id)
                .cloned()
                .unwrap_or_else(|| format!("worksheets/sheet{fallback}.xml"));
            if target.starts_with("xl/") {
                target
            } else {
                format!("xl/{}", target.trim_start_matches('/'))
            }
        };

        for (position, (sheet_name, rel_id, state)) in
            parse_workbook_sheets(&wb_text).into_iter().enumerate()
        {
            if state == "hidden" || state == "veryHidden" {
                // Parsed for the hidden scope (BD-331.S2) but kept out of `sheets`, the census and the
                // quarantine log: a reader never sees these cells.
                if let Some(sheet_bytes) = entries.get(&zip_key_of(&rel_id, position + 1)) {
                    let sheet_xml = String::from_utf8_lossy(sheet_bytes);
                    let mut ignored = Vec::new();
                    let cells = parse_worksheet_cells(
                        &sheet_name,
                        &sheet_xml,
                        &shared_strings,
                        styles.as_ref(),
                        &mut ignored,
                    )?;
                    hidden_sheets.push(Sheet {
                        name: sheet_name.clone(),
                        cells_by_coord: cells,
                    });
                }
                quarantined.push(QuarantineItem::HiddenSheet { sheet_name, state });
                continue;
            }
            if let Some(sheet_bytes) = entries.get(&zip_key_of(&rel_id, sheets.len() + 1)) {
                let sheet_xml = String::from_utf8_lossy(sheet_bytes);
                let cells = parse_worksheet_cells(
                    &sheet_name,
                    &sheet_xml,
                    &shared_strings,
                    styles.as_ref(),
                    &mut quarantined,
                )?;
                sheets.push(Sheet {
                    name: sheet_name,
                    cells_by_coord: cells,
                });
            }
        }

        Ok(Self {
            artifact_path,
            sha256: digest,
            sheets,
            hidden_sheets,
            quarantined,
        })
    }

    /// Finds a visible worksheet by exact or case-insensitive name.
    #[must_use]
    pub fn sheet(&self, name: &str) -> Option<&Sheet> {
        self.sheets
            .iter()
            .find(|s| s.name.eq_ignore_ascii_case(name))
    }

    /// Creates a `SourceAnchor` for `<sheet_name>!<cell_ref>` in this workbook.
    #[must_use]
    pub fn make_anchor(&self, sheet_name: &str, cell_ref: &str) -> SourceAnchor {
        SourceAnchor::new(
            self.artifact_path.clone(),
            self.sha256,
            format!("{sheet_name}!{cell_ref}"),
        )
    }

    /// Total number of extracted cells across all visible sheets.
    #[must_use]
    pub fn total_cells(&self) -> usize {
        self.sheets.iter().map(|s| s.cells_by_coord.len()).sum()
    }
}

fn parse_workbook_rels(xml: &str) -> BTreeMap<String, String> {
    let mut map = BTreeMap::new();
    for tag in iter_xml_start_tags(xml, "Relationship") {
        if let (Some(id), Some(target)) = (extract_attr(tag, "Id"), extract_attr(tag, "Target")) {
            map.insert(id, target);
        }
    }
    map
}

fn parse_workbook_sheets(xml: &str) -> Vec<(String, String, String)> {
    let mut out = Vec::new();
    for tag in iter_xml_start_tags(xml, "sheet") {
        let Some(name) = extract_attr(tag, "name") else {
            continue;
        };
        let rid = extract_attr(tag, "r:id")
            .or_else(|| extract_attr(tag, "id"))
            .unwrap_or_default();
        let state = extract_attr(tag, "state").unwrap_or_else(|| "visible".to_string());
        out.push((decode_xml_entities(&name), rid, state));
    }
    out
}

fn parse_shared_strings(xml: &str) -> Vec<String> {
    let mut strings = Vec::new();
    let mut pos = 0;
    while let Some(si_start) = xml[pos..].find("<si") {
        let abs_start = pos + si_start;
        let Some(tag_end) = xml[abs_start..].find('>') else {
            break;
        };
        if xml[abs_start..abs_start + tag_end].ends_with('/') {
            strings.push(String::new());
            pos = abs_start + tag_end + 1;
            continue;
        }
        let body_start = abs_start + tag_end + 1;
        let Some(si_close) = xml[body_start..].find("</si>") else {
            break;
        };
        let si_body = &xml[body_start..body_start + si_close];
        let mut combined = String::new();
        let mut t_pos = 0;
        while let Some(t_open) = si_body[t_pos..].find("<t") {
            let abs_t = t_pos + t_open;
            let after_t = si_body.as_bytes().get(abs_t + 2).copied().unwrap_or(b' ');
            if after_t != b'>' && after_t != b' ' && after_t != b'/' {
                t_pos = abs_t + 2;
                continue;
            }
            let Some(gt) = si_body[abs_t..].find('>') else {
                break;
            };
            if si_body[abs_t..abs_t + gt].ends_with('/') {
                t_pos = abs_t + gt + 1;
                continue;
            }
            let text_start = abs_t + gt + 1;
            let Some(t_close) = si_body[text_start..].find("</t>") else {
                break;
            };
            combined.push_str(&decode_xml_entities(
                &si_body[text_start..text_start + t_close],
            ));
            t_pos = text_start + t_close + 4;
        }
        strings.push(combined);
        pos = body_start + si_close + 5;
    }
    strings
}

fn parse_worksheet_cells(
    sheet_name: &str,
    xml: &str,
    shared_strings: &[String],
    styles: Option<&Styles>,
    quarantined: &mut Vec<QuarantineItem>,
) -> Result<BTreeMap<(u32, u32), ExtractedCell>, IoError> {
    let mut cells = BTreeMap::new();
    let mut pos = 0;
    while let Some(c_idx) = xml[pos..].find("<c ") {
        let abs_c = pos + c_idx;
        let Some(gt) = xml[abs_c..].find('>') else {
            break;
        };
        let open_tag = &xml[abs_c..abs_c + gt + 1];
        if open_tag.ends_with("/>") {
            pos = abs_c + gt + 1;
            continue;
        }
        let body_start = abs_c + gt + 1;
        let Some(c_close) = xml[body_start..].find("</c>") else {
            break;
        };
        let c_body = &xml[body_start..body_start + c_close];
        pos = body_start + c_close + 4;

        let Some(cell_ref) = extract_attr(open_tag, "r") else {
            continue;
        };
        let Some((row, col)) = parse_a1_ref(&cell_ref) else {
            continue;
        };
        let cell_type = extract_attr(open_tag, "t").unwrap_or_default();
        let style = extract_attr_exact(open_tag, "s")
            .map(|s| {
                s.parse::<u32>().map_err(|_| {
                    IoError::Xml(format!(
                        "{sheet_name}!{cell_ref}: style index {s:?} is not an unsigned integer"
                    ))
                })
            })
            .transpose()?;
        let format = styles.map_or(NumberFormat::Builtin(0), |st| st.format_of(style));
        let formula = extract_element_text(c_body, "f").map(|s| decode_xml_entities(&s));
        let raw_v = extract_element_text(c_body, "v")
            .or_else(|| extract_element_text(c_body, "t"))
            .unwrap_or_default();

        let mut number_text = None;
        let value = match cell_type.as_str() {
            "s" => {
                let idx = raw_v.trim().parse::<usize>().ok();
                let s = idx
                    .and_then(|i| shared_strings.get(i))
                    .cloned()
                    .unwrap_or_default();
                classify_string_cell(sheet_name, &cell_ref, s, quarantined)
            }
            "str" | "inlineStr" => {
                let s = decode_xml_entities(&raw_v);
                classify_string_cell(sheet_name, &cell_ref, s, quarantined)
            }
            "e" => {
                let tok = decode_xml_entities(&raw_v);
                quarantined.push(QuarantineItem::FormulaErrorCell {
                    sheet_name: sheet_name.to_string(),
                    cell_ref: cell_ref.clone(),
                    error_token: tok.clone(),
                });
                CellValue::FormulaError(tok)
            }
            _ => {
                let trimmed = raw_v.trim();
                if trimmed.is_empty() {
                    continue;
                }
                if let Ok(num) = trimmed.parse::<f64>() {
                    if num.is_finite() {
                        number_text = Some(trimmed.to_string());
                        CellValue::Number(num)
                    } else {
                        continue;
                    }
                } else {
                    classify_string_cell(
                        sheet_name,
                        &cell_ref,
                        decode_xml_entities(trimmed),
                        quarantined,
                    )
                }
            }
        };

        cells.insert(
            (row, col),
            ExtractedCell {
                cell_ref,
                row,
                col,
                value,
                formula,
                number_text,
                style,
                format,
            },
        );
    }
    Ok(cells)
}

fn classify_string_cell(
    sheet_name: &str,
    cell_ref: &str,
    s: String,
    quarantined: &mut Vec<QuarantineItem>,
) -> CellValue {
    let t = s.trim();
    if matches!(t, "-" | "–" | "—" | "N/A" | "n/a" | "NA" | "na") {
        // Keep the token as stored: the issuer's `'— '` (trailing space) and `'—'` are counted separately
        // in the W1 census, so trimming here would make that census unreproducible (bd-gwf1.1).
        CellValue::Placeholder(s)
    } else if matches!(
        t,
        "#REF!" | "#DIV/0!" | "#VALUE!" | "#NAME?" | "#N/A" | "#NUM!"
    ) {
        quarantined.push(QuarantineItem::FormulaErrorCell {
            sheet_name: sheet_name.to_string(),
            cell_ref: cell_ref.to_string(),
            error_token: t.to_string(),
        });
        CellValue::FormulaError(t.to_string())
    } else {
        CellValue::Text(s)
    }
}

/// Converts a 1-based column index into Excel letters (`1 -> "A"`, `27 -> "AA"`); `0` gives `""`.
#[must_use]
pub fn col_index_to_letters(mut col: u32) -> String {
    let mut out = Vec::new();
    while col > 0 {
        let rem = (col - 1) % 26;
        out.push(char::from(b'A' + rem as u8));
        col = (col - 1) / 26;
    }
    out.iter().rev().collect()
}

/// One cell for [`build_minimal_xlsx`].
#[derive(Debug, Clone, PartialEq)]
pub enum MinimalCell {
    Number(f64),
    Text(String),
    /// A formula-error token such as `#REF!`, written as an error-typed cell.
    Error(String),
    /// A number stored as this decimal text, with cell style `xf` (an index into [`MinimalStyles::xfs`]).
    Styled {
        raw: String,
        xf: u32,
    },
}

/// The styles part for [`build_minimal_xlsx_styled`] (bd-jg43.3): custom number formats by id, and the cell
/// styles (`cellXfs`), each naming its number format id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MinimalStyles {
    pub num_fmts: Vec<(u32, String)>,
    pub xfs: Vec<u32>,
}

/// One worksheet for [`build_minimal_xlsx`]: cells keyed by A1 reference.
#[derive(Debug, Clone, PartialEq)]
pub struct MinimalSheet {
    pub name: String,
    pub hidden: bool,
    pub cells: Vec<(String, MinimalCell)>,
}

/// Writes a minimal valid `.xlsx` (stored ZIP, inline strings, no styles) holding the given sheets.
/// It exists so tests and the synthetic lane can build workbooks in a real filing's layout without shipping
/// any issuer's bytes. Cells whose reference does not parse are skipped.
#[must_use]
pub fn build_minimal_xlsx(sheets: &[MinimalSheet]) -> Vec<u8> {
    build_minimal_xlsx_styled(sheets, None)
}

/// [`build_minimal_xlsx`] with a styles part (`xl/styles.xml` and its workbook relationship) when `styles` is
/// given; with `None` the bytes are exactly [`build_minimal_xlsx`]'s.
#[must_use]
pub fn build_minimal_xlsx_styled(
    sheets: &[MinimalSheet],
    styles: Option<&MinimalStyles>,
) -> Vec<u8> {
    let mut wb_xml = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?><workbook xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\" xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\"><sheets>",
    );
    let mut rels_xml = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?><Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">",
    );
    let mut sheet_xmls = Vec::with_capacity(sheets.len());
    for (i, sheet) in sheets.iter().enumerate() {
        let n = i + 1;
        let state = if sheet.hidden {
            " state=\"hidden\""
        } else {
            ""
        };
        wb_xml.push_str(&format!(
            "<sheet name=\"{}\" sheetId=\"{n}\"{state} r:id=\"rId{n}\"/>",
            escape_xml(&sheet.name)
        ));
        rels_xml.push_str(&format!(
            "<Relationship Id=\"rId{n}\" Target=\"worksheets/sheet{n}.xml\"/>"
        ));
        let mut rows: BTreeMap<u32, BTreeMap<u32, String>> = BTreeMap::new();
        for (cell_ref, cell) in &sheet.cells {
            let Some((row, col)) = parse_a1_ref(cell_ref) else {
                continue;
            };
            let xml = match cell {
                MinimalCell::Number(v) => format!("<c r=\"{cell_ref}\"><v>{v}</v></c>"),
                MinimalCell::Text(t) => format!(
                    "<c r=\"{cell_ref}\" t=\"inlineStr\"><is><t>{}</t></is></c>",
                    escape_xml(t)
                ),
                MinimalCell::Error(t) => {
                    format!("<c r=\"{cell_ref}\" t=\"e\"><v>{}</v></c>", escape_xml(t))
                }
                MinimalCell::Styled { raw, xf } => {
                    format!(
                        "<c r=\"{cell_ref}\" s=\"{xf}\"><v>{}</v></c>",
                        escape_xml(raw)
                    )
                }
            };
            rows.entry(row).or_default().insert(col, xml);
        }
        let mut xml = String::from(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?><worksheet xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\"><sheetData>",
        );
        for (row, cells) in rows {
            xml.push_str(&format!("<row r=\"{row}\">"));
            for c in cells.values() {
                xml.push_str(c);
            }
            xml.push_str("</row>");
        }
        xml.push_str("</sheetData></worksheet>");
        sheet_xmls.push((format!("xl/worksheets/sheet{n}.xml"), xml));
    }
    wb_xml.push_str("</sheets></workbook>");
    let styles_xml = styles.map(|s| {
        rels_xml.push_str(&format!(
            "<Relationship Id=\"rId{}\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles\" Target=\"styles.xml\"/>",
            sheets.len() + 1
        ));
        let mut xml = String::from(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?><styleSheet xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\">",
        );
        if !s.num_fmts.is_empty() {
            xml.push_str(&format!("<numFmts count=\"{}\">", s.num_fmts.len()));
            for (id, code) in &s.num_fmts {
                xml.push_str(&format!(
                    "<numFmt numFmtId=\"{id}\" formatCode=\"{}\"/>",
                    escape_xml(code)
                ));
            }
            xml.push_str("</numFmts>");
        }
        xml.push_str(&format!("<cellXfs count=\"{}\">", s.xfs.len()));
        for id in &s.xfs {
            xml.push_str(&format!("<xf numFmtId=\"{id}\"/>"));
        }
        xml.push_str("</cellXfs></styleSheet>");
        xml
    });
    rels_xml.push_str("</Relationships>");
    let mut entries: Vec<(&str, &[u8])> = vec![
        ("xl/workbook.xml", wb_xml.as_bytes()),
        ("xl/_rels/workbook.xml.rels", rels_xml.as_bytes()),
    ];
    if let Some(xml) = &styles_xml {
        entries.push(("xl/styles.xml", xml.as_bytes()));
    }
    for (name, xml) in &sheet_xmls {
        entries.push((name.as_str(), xml.as_bytes()));
    }
    write_stored_zip(&entries)
}

fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Parses an Excel cell reference such as `"N14"` or `"AA102"` into 1-based `(row, col)`.
#[must_use]
pub fn parse_a1_ref(cell_ref: &str) -> Option<(u32, u32)> {
    let mut col: u32 = 0;
    let mut row: u32 = 0;
    let mut in_digits = false;
    for b in cell_ref.bytes() {
        if b == b'$' {
            continue;
        }
        if b.is_ascii_uppercase() && !in_digits {
            col = col.checked_mul(26)?.checked_add(u32::from(b - b'A' + 1))?;
        } else if b.is_ascii_digit() {
            in_digits = true;
            row = row.checked_mul(10)?.checked_add(u32::from(b - b'0'))?;
        } else {
            return None;
        }
    }
    if col == 0 || row == 0 {
        None
    } else {
        Some((row, col))
    }
}

pub(crate) fn iter_xml_start_tags<'a>(xml: &'a str, tag_name: &'a str) -> Vec<&'a str> {
    let mut out = Vec::new();
    let pat = format!("<{tag_name}");
    let mut pos = 0;
    while let Some(idx) = xml[pos..].find(&pat) {
        let abs = pos + idx;
        let next_ch = xml.as_bytes().get(abs + pat.len()).copied().unwrap_or(b' ');
        if next_ch != b' ' && next_ch != b'>' && next_ch != b'/' {
            pos = abs + pat.len();
            continue;
        }
        let Some(gt) = xml[abs..].find('>') else {
            break;
        };
        out.push(&xml[abs..abs + gt + 1]);
        pos = abs + gt + 1;
    }
    out
}

fn extract_attr(tag: &str, attr_name: &str) -> Option<String> {
    let pat = format!("{attr_name}=\"");
    let start = tag.find(&pat)? + pat.len();
    let end = tag[start..].find('"')? + start;
    Some(tag[start..end].to_string())
}

/// The value of attribute `name` in a start tag, matched as a whole name: preceded by whitespace and followed
/// by `=` (whitespace allowed around it), with either quote style. `cs="9"` does not match `s`. The value is
/// returned undecoded.
pub(crate) fn extract_attr_exact(tag: &str, name: &str) -> Option<String> {
    let bytes = tag.as_bytes();
    let mut from = 0;
    while let Some(i) = tag[from..].find(name) {
        let at = from + i;
        from = at + name.len();
        if at == 0 || !bytes[at - 1].is_ascii_whitespace() {
            continue;
        }
        let Some(rest) = tag[from..].trim_start().strip_prefix('=') else {
            continue;
        };
        let rest = rest.trim_start();
        let quote = rest.chars().next()?;
        if quote != '"' && quote != '\'' {
            return None;
        }
        let body = &rest[1..];
        let end = body.find(quote)?;
        return Some(body[..end].to_string());
    }
    None
}

fn extract_element_text(xml: &str, tag_name: &str) -> Option<String> {
    let open_pat = format!("<{tag_name}");
    let idx = xml.find(&open_pat)?;
    let gt = xml[idx..].find('>')? + idx;
    if xml[idx..=gt].ends_with("/>") {
        return None;
    }
    let close_pat = format!("</{tag_name}>");
    let end = xml[gt + 1..].find(&close_pat)? + gt + 1;
    Some(xml[gt + 1..end].to_string())
}

pub(crate) fn decode_xml_entities(s: &str) -> String {
    if !s.contains('&') {
        return s.to_string();
    }
    s.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&#10;", "\n")
        .replace("&#13;", "\r")
        .replace("&#160;", " ")
}
