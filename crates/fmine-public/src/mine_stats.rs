//! Label-anchored reader for the issuer's quarterly "Mine Statistics" workbooks (bd-gwf1.1; BD-322, WP-03).
//!
//! Blocks are found from the column-A text "For the (three|six|nine|twelve) months …", never from a row or
//! column number of an earlier vintage. On the `Minestats*` sheets the data columns are the ones whose block
//! header holds a year, and each column's site name comes from the row above, carried left across merged
//! blanks. Metric labels lose a trailing one- or two-digit footnote number, because the issuer renumbers
//! footnotes between files. The `QTD`/`YTD` summary sheets are read the same way, with each column header
//! mapped to a field id. The rules are those of research note W1 §3, §4 and §13.
//!
//! Every data cell becomes a typed value with its locator and its token as stored. The token set is a number,
//! `—` (with or without a trailing space), `n/a` and blank. A cell outside that set is refused by name and not
//! emitted. So is a block whose header names no known period, and a label repeated within a block.
//! A numeric zero stays a number: deciding that a zero in a divested site's column means "no value" belongs to
//! the subject-status registry (BD-332.S3), not to the reader.
//!
//! The reference is W1's openpyxl reader (`scripts/oracle/mine_stats_extract3.py`), written before this code.
//! The two reads are compared line by line: [`MineStatsRead::canonical_lines`] here and
//! `scripts/oracle/mine_stats_lines.py` there produce the same line format. Where the reference reader turns a
//! cell into text (labels, site names), this module follows Python's `str()` and whitespace rules.
//!
//! The issuer registry's `[[mine_stats_label]]` rows are the label vocabulary ([`LabelVocabulary`]). When it is
//! given, a data row whose `(section, label)` is not registered is refused by locator.
//!
//! Which rows are sub-rows of the row above comes from the same vocabulary (a label registered under a
//! section); W1's list (`CHILD_ROWS`) stands in only for a read without one. A summary header must print the
//! unit its field id promises (thousand ounces, kilotonnes, $/oz, $/lb), and a header naming no known field is
//! refused, never dropped.
//!
//! The reader reports what the file prints. W1-DR-01 (Porgera's stale share) is registry data read by the
//! ledger (`mine_ledger`, `[[printed_share_correction]]`), and the ledger asks the region map
//! (`[[region_member]]`) by release; values carry no map version. Not yet here (bd-gwf1.1 stays open for it):
//! the W1-DR-02 correction as data.

use std::collections::BTreeSet;

use fmine_core::hash::{Digest32, sha256};
use fmine_evidence::Refusal;
use fmine_io::xlsx::{CellValue, QuarantineItem, Sheet, Workbook, col_index_to_letters};

/// Which kind of sheet a value was read from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View {
    /// `QTD` and `YTD`: one row per site, columns are fields (cost of sales, cash costs, AISC, ounces).
    Summary,
    /// `Minestats1`–`Minestats4`: one column per site, rows are metrics.
    Minestats,
}

impl View {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Summary => "summary",
            Self::Minestats => "minestats",
        }
    }
}

/// A typed data cell. Placeholders keep their token exactly as stored (`"— "` keeps its trailing space).
#[derive(Debug, Clone, PartialEq)]
pub enum MineStatsCell {
    /// A number, with its text as stored (`"0"` for the numeric-zero placeholders of divested sites).
    Number { raw_text: String, value: f64 },
    /// `—`: no value reported.
    Nil { token: String },
    /// `n/a`: the metric does not apply to this site.
    NotApplicable { token: String },
    /// An empty cell in a row that has data in other columns.
    Missing,
}

impl MineStatsCell {
    /// `number`, `nil`, `na` or `missing`.
    #[must_use]
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::Number { .. } => "number",
            Self::Nil { .. } => "nil",
            Self::NotApplicable { .. } => "na",
            Self::Missing => "missing",
        }
    }

    /// The token in a canonical line: a number as its IEEE-754 bits in 16 hex digits (exact, no decimal
    /// formatting), a placeholder as stored, a missing cell as the empty string.
    fn canonical_token(&self) -> String {
        match self {
            Self::Number { value, .. } => format!("{:016x}", value.to_bits()),
            Self::Nil { token } | Self::NotApplicable { token } => token.clone(),
            Self::Missing => String::new(),
        }
    }
}

/// One data cell read from a Mine Statistics workbook.
#[derive(Debug, Clone, PartialEq)]
pub struct MineStatsValue {
    /// `<quarter>:<sheet>!<cell>`, e.g. `2026Q2:Minestats1!B10`.
    pub locator: String,
    pub sheet: String,
    pub cell: String,
    pub view: View,
    /// Length of the block's period in months: 3, 6, 9 or 12.
    pub months: u8,
    /// `2026Q2`, `2026H1`, `20269M` or `2026FY`: the year from the block header, the quarter from the file.
    pub period: String,
    /// The block header with whitespace collapsed, e.g. `For the three months ended June 30,`.
    pub period_label: String,
    /// The site header as printed (trimmed), e.g. `Carlin (61.5%)` or `Long Canyon (61.5%)1`.
    pub site_raw: String,
    /// The site without footnote number or ownership share, e.g. `Carlin`.
    pub site: String,
    /// The ownership share printed in the site header (`"61.5"`), or `None` where none is printed (100%).
    pub printed_share: Option<String>,
    /// `Minestats*`: the row label without its footnote number. Summary: the field id from the column header
    /// (e.g. `aisc_usd_per_oz`).
    pub label: String,
    /// The label (row label or column header) as printed, units included.
    pub label_as_printed: String,
    /// The footnote number stripped from a `Minestats*` row label.
    pub footnote: Option<u8>,
    /// For a sub-row (`Open Pit`, `Processed`, `Royalties`, …), the label of the row it belongs to.
    pub section: Option<String>,
    pub value: MineStatsCell,
}

impl MineStatsValue {
    /// `sheet \t cell \t view \t period \t site_raw \t site \t label \t section \t kind \t token`, the format of
    /// `scripts/oracle/mine_stats_lines.py` (section `-` when there is none).
    #[must_use]
    pub fn canonical_line(&self) -> String {
        let section = self
            .section
            .as_deref()
            .filter(|s| !s.is_empty())
            .unwrap_or("-");
        format!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
            self.sheet,
            self.cell,
            self.view.as_str(),
            self.period,
            self.site_raw,
            self.site,
            self.label,
            section,
            self.value.kind(),
            self.value.canonical_token()
        )
    }

    /// True for a numeric zero (`Number` whose value is 0).
    #[must_use]
    pub fn is_numeric_zero(&self) -> bool {
        matches!(self.value, MineStatsCell::Number { value, .. } if value == 0.0)
    }
}

/// A cell, or a block header, the reader could not type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CellRefusal {
    /// `<quarter>:<sheet>!<cell>`.
    pub locator: String,
    pub reason: String,
}

/// Everything read from one Mine Statistics workbook.
#[derive(Debug, Clone, PartialEq)]
pub struct MineStatsRead {
    pub quarter: String,
    /// Blocks located (`For the …` headers with a known period), summary and `Minestats*` together.
    pub blocks: usize,
    pub values: Vec<MineStatsValue>,
    pub refusals: Vec<CellRefusal>,
    /// Hidden sheets of the workbook. They are not read; they are named here so they are never skipped silently.
    pub hidden_sheets: Vec<String>,
}

impl MineStatsRead {
    /// Values of one kind (`number`, `nil`, `na`, `missing`).
    #[must_use]
    pub fn count(&self, kind: &str) -> usize {
        self.values
            .iter()
            .filter(|v| v.value.kind() == kind)
            .count()
    }

    #[must_use]
    pub fn numeric_zeros(&self) -> usize {
        self.values.iter().filter(|v| v.is_numeric_zero()).count()
    }

    /// Every value's canonical line, sorted.
    #[must_use]
    pub fn canonical_lines(&self) -> Vec<String> {
        let mut lines: Vec<String> = self
            .values
            .iter()
            .map(MineStatsValue::canonical_line)
            .collect();
        lines.sort();
        lines
    }

    /// SHA-256 over the sorted canonical lines, each followed by `\n`.
    #[must_use]
    pub fn digest(&self) -> Digest32 {
        let mut text = String::new();
        for line in self.canonical_lines() {
            text.push_str(&line);
            text.push('\n');
        }
        sha256(text.as_bytes())
    }
}

/// Rows that belong to the row above them (W1's list, matched on the lower-cased label without footnote).
const CHILD_ROWS: &[&str] = &[
    "open pit ore",
    "open pit waste",
    "underground",
    "open pit",
    "oxide mill",
    "roasters",
    "autoclave",
    "heap leach",
    "open pit mined",
    "underground mined",
    "processed",
    "royalties",
    "depreciation",
    "total cash costs",
    "all-in sustaining costs",
    "minesite sustaining",
    "projects",
    "c1 cash costs",
];

/// The registered `Minestats*` row labels as `(section, label)` (`[[mine_stats_label]]` in the issuer registry).
pub type LabelVocabulary = BTreeSet<(Option<String>, String)>;

/// Reads every `QTD`, `YTD` and `Minestats*` sheet of `workbook`, registered for `quarter` (`YYYYQn`).
///
/// With a `vocabulary`, a `Minestats*` data row whose `(section, label)` is not registered is refused by
/// locator and not emitted, so a label first seen in a later vintage is named instead of read silently. Without
/// one, every label is read.
///
/// # Errors
/// Refuses (`FMINE-REF-502`) when `quarter` is not `YYYYQn`: the quarter digit is part of every quarterly
/// period code. Cell-level problems never fail the read; they are listed in [`MineStatsRead::refusals`].
pub fn read_mine_stats(
    workbook: &Workbook,
    quarter: &str,
    vocabulary: Option<&LabelVocabulary>,
) -> Result<MineStatsRead, Refusal> {
    let fq = quarter
        .split_once('Q')
        .filter(|(y, q)| {
            y.len() == 4
                && y.bytes().all(|b| b.is_ascii_digit())
                && matches!(*q, "1" | "2" | "3" | "4")
        })
        .and_then(|(_, q)| q.parse::<u8>().ok())
        .ok_or_else(|| {
            Refusal::from_code(
                "FMINE-REF-502",
                format!(
                    "Mine Statistics workbook {} registered for quarter {quarter:?}, not YYYYQn",
                    workbook.artifact_path
                ),
            )
        })?;
    let mut read = MineStatsRead {
        quarter: quarter.to_string(),
        blocks: 0,
        values: Vec::new(),
        refusals: Vec::new(),
        hidden_sheets: workbook
            .quarantined
            .iter()
            .filter_map(|q| match q {
                QuarantineItem::HiddenSheet { sheet_name, .. } => Some(sheet_name.clone()),
                QuarantineItem::FormulaErrorCell { .. } => None,
            })
            .collect(),
    };
    for sheet in &workbook.sheets {
        if sheet.name == "QTD" || sheet.name == "YTD" {
            read_summary(sheet, fq, &mut read);
        } else if sheet.name.starts_with("Minestats") {
            read_minestats(sheet, fq, vocabulary, &mut read);
        }
    }
    Ok(read)
}

fn read_summary(sheet: &Sheet, fq: u8, read: &mut MineStatsRead) {
    let max_row = max_row(sheet);
    let mut r = 1;
    while r <= max_row.min(60) {
        let Some((period_label, months)) = block_header(sheet, r, read) else {
            r += 1;
            continue;
        };
        read.blocks += 1;
        // Field ids from the header row; the current year in columns B–G, the prior year in I–N.
        let mut fields = Vec::new();
        for (first, last, year_col) in [(2, 7, 2), (9, 14, 9)] {
            let mapped: Vec<(u32, String, String)> = (first..=last)
                .filter_map(|c| {
                    let header = py_str(sheet, r + 1, c)?;
                    match summary_field(&header) {
                        Ok(Some(id)) => Some((c, id, header)),
                        // A renamed field must not drop its column silently.
                        Ok(None) => {
                            let reason = format!("summary header {header:?} names no known field");
                            refuse(read, sheet, r + 1, c, reason);
                            None
                        }
                        Err(reason) => {
                            refuse(read, sheet, r + 1, c, reason);
                            None
                        }
                    }
                })
                .collect();
            if mapped.is_empty() {
                continue;
            }
            match integer_text(sheet, r, year_col) {
                Some(year) => fields.extend(
                    mapped
                        .into_iter()
                        .map(|(c, id, h)| (c, id, h, year.clone())),
                ),
                None => refuse(
                    read,
                    sheet,
                    r,
                    year_col,
                    "summary block year header is not an integer".to_string(),
                ),
            }
        }
        let mut rr = r + 2;
        while rr <= max_row {
            let Some(site_cell) = py_str(sheet, rr, 1) else {
                break;
            };
            if footnote_line(&site_cell) || site_cell.starts_with("For the") {
                break;
            }
            for (c, id, header, year) in &fields {
                push_value(
                    read,
                    sheet,
                    (rr, *c),
                    View::Summary,
                    (months, period_code(year, months, fq), &period_label),
                    &site_cell,
                    (id.clone(), header.clone(), None, None),
                );
            }
            rr += 1;
        }
        r = rr;
    }
}

fn read_minestats(
    sheet: &Sheet,
    fq: u8,
    vocabulary: Option<&LabelVocabulary>,
    read: &mut MineStatsRead,
) {
    // A sub-row is a label the registry lists under a section; W1's list stands in only when no vocabulary is
    // registered (test_child_rows_equal_the_registered_sections keeps the two equal).
    let children: BTreeSet<String> = match vocabulary {
        Some(v) => v
            .iter()
            .filter(|(section, _)| section.is_some())
            .map(|(_, label)| label.to_lowercase())
            .collect(),
        None => CHILD_ROWS.iter().map(|s| (*s).to_string()).collect(),
    };
    let max_row = max_row(sheet);
    let mut r = 1;
    while r <= max_row {
        let Some((period_label, months)) = block_header(sheet, r, read) else {
            r += 1;
            continue;
        };
        if r == 1 {
            refuse(
                read,
                sheet,
                r,
                1,
                "block header on row 1 has no site row above it".to_string(),
            );
            r += 1;
            continue;
        }
        read.blocks += 1;
        let site_row = r - 1;
        let mut columns = Vec::new();
        for c in 2..=15 {
            let Some(year) = integer_text(sheet, r, c) else {
                continue;
            };
            match site_at_or_left(sheet, site_row, c) {
                Some(site) => columns.push((c, year, site)),
                None => refuse(
                    read,
                    sheet,
                    site_row,
                    c,
                    "no site name at or left of this cell".to_string(),
                ),
            }
        }
        let mut parent: Option<String> = None;
        let mut seen = BTreeSet::new();
        let mut rr = r + 1;
        while rr <= max_row {
            let Some(cell_text) = py_str(sheet, rr, 1) else {
                rr += 1;
                continue;
            };
            let printed = py_strip(&cell_text);
            if footnote_line(printed)
                || printed.starts_with("BARRICK")
                || printed.starts_with("Mine Statistics")
                || printed.starts_with("For the")
            {
                break;
            }
            let (label, footnote) = strip_footnote(printed);
            let section = if parent.is_some() && children.contains(&label.to_lowercase()) {
                parent.clone()
            } else {
                parent = Some(label.clone());
                None
            };
            let has_data = columns.iter().any(|(c, _, _)| sheet.cell(rr, *c).is_some());
            if has_data && !seen.insert((section.clone(), label.clone())) {
                refuse(
                    read,
                    sheet,
                    rr,
                    1,
                    format!("label {label:?} appears twice in this block"),
                );
            } else if has_data
                && vocabulary.is_some_and(|v| !v.contains(&(section.clone(), label.clone())))
            {
                refuse(
                    read,
                    sheet,
                    rr,
                    1,
                    format!(
                        "label {label:?} (section {section:?}) is not in the registered Mine Stats \
                         vocabulary; add a [[mine_stats_label]] row once the label is understood"
                    ),
                );
            } else if has_data {
                for (c, year, site) in &columns {
                    push_value(
                        read,
                        sheet,
                        (rr, *c),
                        View::Minestats,
                        (months, period_code(year, months, fq), &period_label),
                        site,
                        (
                            label.clone(),
                            printed.to_string(),
                            footnote,
                            section.clone(),
                        ),
                    );
                }
            }
            rr += 1;
        }
        r = rr;
    }
}

/// `(label, label as printed, footnote, section)` of a value.
type LabelParts = (String, String, Option<u8>, Option<String>);

fn push_value(
    read: &mut MineStatsRead,
    sheet: &Sheet,
    (row, col): (u32, u32),
    view: View,
    (months, period, period_label): (u8, String, &str),
    site_cell: &str,
    (label, label_as_printed, footnote, section): LabelParts,
) {
    let value = match typed_cell(sheet, row, col) {
        Ok(v) => v,
        Err(reason) => {
            refuse(read, sheet, row, col, reason);
            return;
        }
    };
    let cell = format!("{}{row}", col_index_to_letters(col));
    read.values.push(MineStatsValue {
        locator: format!("{}:{}!{cell}", read.quarter, sheet.name),
        sheet: sheet.name.clone(),
        cell,
        view,
        months,
        period,
        period_label: period_label.to_string(),
        site_raw: py_strip(site_cell).to_string(),
        site: site_name(site_cell),
        printed_share: printed_share(site_cell).map(str::to_string),
        label,
        label_as_printed,
        footnote,
        section,
        value,
    });
}

fn refuse(read: &mut MineStatsRead, sheet: &Sheet, row: u32, col: u32, reason: String) {
    read.refusals.push(CellRefusal {
        locator: format!(
            "{}:{}!{}{row}",
            read.quarter,
            sheet.name,
            col_index_to_letters(col)
        ),
        reason,
    });
}

/// The block header on row `r`, if column A starts with `For the`: the collapsed text and the period length.
/// A header that names no three/six/nine/twelve-month period is refused and is not a block.
fn block_header(sheet: &Sheet, r: u32, read: &mut MineStatsRead) -> Option<(String, u8)> {
    let text = match &sheet.cell(r, 1)?.value {
        CellValue::Text(s) | CellValue::Placeholder(s) | CellValue::FormulaError(s) => s,
        CellValue::Number(_) => return None,
    };
    if !py_strip(text).starts_with("For the") {
        return None;
    }
    let label = collapse(text);
    let lower = label.to_lowercase();
    let months = [("three", 3), ("six", 6), ("nine", 9), ("twelve", 12)]
        .into_iter()
        .find(|(word, _)| lower.contains(word))
        .map(|(_, m)| m);
    if months.is_none() {
        refuse(
            read,
            sheet,
            r,
            1,
            format!("block header {label:?} names no three, six, nine or twelve month period"),
        );
    }
    months.map(|m| (label, m))
}

fn period_code(year: &str, months: u8, fq: u8) -> String {
    match months {
        3 => format!("{year}Q{fq}"),
        6 => format!("{year}H1"),
        9 => format!("{year}9M"),
        _ => format!("{year}FY"),
    }
}

fn typed_cell(sheet: &Sheet, row: u32, col: u32) -> Result<MineStatsCell, String> {
    let Some(cell) = sheet.cell(row, col) else {
        return Ok(MineStatsCell::Missing);
    };
    match &cell.value {
        CellValue::Number(v) => Ok(MineStatsCell::Number {
            raw_text: cell.number_text.clone().unwrap_or_else(|| v.to_string()),
            value: *v,
        }),
        CellValue::Placeholder(t) | CellValue::Text(t) => match py_strip(t) {
            "—" => Ok(MineStatsCell::Nil { token: t.clone() }),
            "n/a" => Ok(MineStatsCell::NotApplicable { token: t.clone() }),
            _ => Err(format!(
                "token {t:?} is outside the set (a number, —, n/a, blank)"
            )),
        },
        CellValue::FormulaError(t) => Err(format!("formula error {t:?} in a data cell")),
    }
}

/// The summary field id of a column header, or `None` for a column that is not read. A header that names a
/// field but no metal is refused (`Err`).
fn summary_field(header: &str) -> Result<Option<String>, String> {
    let raw = collapse(header).to_lowercase();
    let per_100 = raw.contains("100%");
    let n: String = raw.chars().filter(|c| !c.is_ascii_digit()).collect();
    let field = if n.contains("produced") && per_100 {
        "prod_100"
    } else if n.contains("produced") && n.contains("attributable") {
        "prod_attr"
    } else if n.contains("sold") {
        "sold_attr"
    } else if n.contains("cost of sales") {
        "cos"
    } else if n.contains("total cash costs") {
        "tcc"
    } else if n.contains("c cash costs") {
        "c1"
    } else if n.contains("all-in sustaining") {
        "aisc"
    } else {
        return Ok(None);
    };
    let metal = if n.contains("gold") || n.contains("$/oz") {
        "au"
    } else if n.contains("copper") || n.contains("$/lb") {
        "cu"
    } else {
        return Err(format!(
            "summary header {header:?} names field {field} but no metal (gold, copper, $/oz, $/lb)"
        ));
    };
    // The unit the field id promises must be the one printed: ounces in thousands, copper in kilotonnes, costs
    // per ounce or per pound. Any other unit is refused rather than emitted under a wrong unit.
    let quantity = matches!(field, "prod_100" | "prod_attr" | "sold_attr");
    let spellings: &[&str] = match (quantity, metal) {
        (true, "au") => &["(000s oz"],
        (true, _) => &["(kt ", "(thousands of tonnes"],
        (false, "au") => &["($/oz)"],
        (false, _) => &["($/lb)"],
    };
    if !spellings.iter().any(|u| raw.contains(u)) {
        return Err(format!(
            "summary header {header:?} names field {field} but not its unit {:?}",
            spellings[0].trim_matches(|c| c == '(' || c == ')' || c == ' ')
        ));
    }
    Ok(Some(match (field, metal) {
        ("prod_100" | "prod_attr" | "sold_attr", "au") => format!("au_{field}_koz"),
        ("prod_100" | "prod_attr" | "sold_attr", _) => format!("cu_{field}_kt"),
        (_, "au") => format!("{field}_usd_per_oz"),
        _ => format!("{field}_usd_per_lb"),
    }))
}

/// The site name for column `col`: the site row's text at `col`, or the nearest non-empty cell to its left
/// (a merged header is stored in its first cell only), trimmed.
fn site_at_or_left(sheet: &Sheet, site_row: u32, col: u32) -> Option<String> {
    let mut c = col;
    while c >= 2 && sheet.cell(site_row, c).is_none() {
        c -= 1;
    }
    py_str(sheet, site_row, c).map(|s| py_strip(&s).to_string())
}

/// The text of an integer-typed number (stored without `.` or an exponent), as Python's `int` prints it.
fn integer_text(sheet: &Sheet, row: u32, col: u32) -> Option<String> {
    let text = sheet.cell(row, col)?.number_text.as_deref()?;
    if text.contains(['.', 'e', 'E']) {
        return None;
    }
    text.parse::<i128>().ok().map(|i| i.to_string())
}

/// `str(value)` of a cell as the reference reader sees it: text as stored; an integer-typed number as its
/// integer; a float-typed number in Rust's `{:?}` form, which equals Python's `repr` outside exponent notation.
fn py_str(sheet: &Sheet, row: u32, col: u32) -> Option<String> {
    let cell = sheet.cell(row, col)?;
    Some(match &cell.value {
        CellValue::Text(s) | CellValue::Placeholder(s) | CellValue::FormulaError(s) => s.clone(),
        CellValue::Number(v) => integer_text(sheet, row, col).unwrap_or_else(|| format!("{v:?}")),
    })
}

/// A footnote line under a table (`"1 Barrick's share …"`): leading digits followed by whitespace.
fn footnote_line(s: &str) -> bool {
    let t = s.trim_start_matches(py_space);
    let digits = t.bytes().take_while(u8::is_ascii_digit).count();
    digits > 0 && t[digits..].chars().next().is_some_and(py_space)
}

/// Python's `str.isspace`, which `\s` and `strip()` use in the reference reader: Unicode `White_Space` plus the
/// ASCII information separators U+001C–U+001F.
fn py_space(c: char) -> bool {
    c.is_whitespace() || ('\u{1c}'..='\u{1f}').contains(&c)
}

fn py_strip(s: &str) -> &str {
    s.trim_matches(py_space)
}

/// Whitespace runs collapsed to one space, ends trimmed (`re.sub(r'\s+', ' ', s).strip()`).
fn collapse(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for part in s.split(py_space).filter(|p| !p.is_empty()) {
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(part);
    }
    out
}

/// A label without its trailing footnote number: `"Total cash costs3"` gives `("Total cash costs", Some(3))`.
/// Only a run of one or two digits directly after a character that is neither a digit nor a space counts
/// (`^(.*?[^\d\s])(\d{1,2})$`), so `"Q2 2026"` and `"Tonnes (000s)"` are unchanged. Whitespace is collapsed.
#[must_use]
pub fn strip_footnote(label: &str) -> (String, Option<u8>) {
    let s = collapse(label);
    let digits = s.bytes().rev().take_while(u8::is_ascii_digit).count();
    let cut = s.len() - digits;
    let before = s[..cut].chars().next_back();
    if (1..=2).contains(&digits)
        && before.is_some_and(|c| !py_space(c))
        && !s.ends_with("%)")
        && let Ok(n) = s[cut..].parse::<u8>()
    {
        return (py_strip(&s[..cut]).to_string(), Some(n));
    }
    (s, None)
}

/// Byte range of the first `(<digits and dots>%)` group in `s`.
fn share_span(s: &str) -> Option<(usize, usize)> {
    let b = s.as_bytes();
    (0..b.len()).find_map(|i| {
        if b[i] != b'(' {
            return None;
        }
        let digits = b[i + 1..]
            .iter()
            .take_while(|c| c.is_ascii_digit() || **c == b'.')
            .count();
        let end = i + 1 + digits;
        (digits > 0 && b.get(end) == Some(&b'%') && b.get(end + 1) == Some(&b')'))
            .then_some((i, end + 2))
    })
}

/// The ownership share printed in a site header (`"Carlin (61.5%)"` gives `"61.5"`), or `None`.
#[must_use]
pub fn printed_share(site: &str) -> Option<&str> {
    share_span(site).map(|(start, end)| &site[start + 1..end - 2])
}

/// A site header without footnote number or ownership share; `Copper - Total` reads as `Total Copper`.
#[must_use]
pub fn site_name(raw: &str) -> String {
    let (s, _) = strip_footnote(raw);
    let mut out = String::with_capacity(s.len());
    let mut rest = s.as_str();
    while let Some((start, end)) = share_span(rest) {
        out.push_str(rest[..start].trim_end_matches(py_space));
        rest = &rest[end..];
    }
    out.push_str(rest);
    match py_strip(&out) {
        "Copper - Total" => "Total Copper".to_string(),
        other => other.to_string(),
    }
}

fn max_row(sheet: &Sheet) -> u32 {
    sheet
        .cells_by_coord
        .keys()
        .map(|(r, _)| *r)
        .max()
        .unwrap_or(0)
}

/// Sites the M0 target-cell census excludes: region and metal totals, and the Nevada Gold Mines aggregate.
const AGGREGATES: &[&str] = &[
    "North America",
    "Latin America & Asia Pacific",
    "South America & Asia Pacific",
    "Africa & Middle East",
    "Total Gold",
    "Total Copper",
    "Nevada Gold Mines LLC",
];

/// True for a cell of the M0 target-cell census (W1 §4): the five per-site fields of the current quarter.
/// Cost of sales, total cash costs or C1, and AISC come from the `QTD` sheet; processed grade and site recovery
/// from the three-month `Minestats*` blocks. Aggregates are excluded and `current_year` is the file's year.
#[must_use]
pub fn is_m0_target(v: &MineStatsValue, current_year: &str) -> bool {
    if !v.period.starts_with(current_year) || v.months != 3 || AGGREGATES.contains(&v.site.as_str())
    {
        return false;
    }
    match v.view {
        View::Summary => {
            v.sheet == "QTD"
                && matches!(
                    v.label.as_str(),
                    "cos_usd_per_oz"
                        | "tcc_usd_per_oz"
                        | "aisc_usd_per_oz"
                        | "cos_usd_per_lb"
                        | "c1_usd_per_lb"
                        | "aisc_usd_per_lb"
                )
        }
        View::Minestats => {
            (v.label == "Processed"
                && v.section
                    .as_deref()
                    .is_some_and(|s| s.starts_with("Average grade")))
                || (v.label == "Recovery rate (percent)" && v.section.is_none())
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    use fmine_io::xlsx::{MinimalCell, MinimalSheet, build_minimal_xlsx};

    use super::*;
    use crate::sources::SourceKind;
    use crate::{load_issuer_profile, load_registered_sources};

    type Cells = Vec<(String, MinimalCell)>;

    fn t(s: &str) -> MinimalCell {
        MinimalCell::Text(s.to_string())
    }

    fn put(cells: &mut Cells, col: u32, row: u32, v: MinimalCell) {
        let at = format!("{}{row}", col_index_to_letters(col));
        cells.retain(|(r, _)| *r != at);
        cells.push((at, v));
    }

    /// A Minestats sheet and a QTD sheet in the issuer's layout. With `drift`, the same figures move the way
    /// the issuer's files drift between vintages (W1 §3): a site inserted in front (columns move right by two),
    /// a Revenue row inserted (rows move down by one), every footnote renumbered, a misspelled month and a
    /// doubled space in the block headers, and a 100%-basis column inserted in the summary.
    fn fixture(drift: bool) -> (Cells, Cells) {
        let (sh, rs) = if drift { (2, 1) } else { (0, 0) };
        let fnum = |base: u8| if drift { base + 1 } else { base };
        let mut ms = Cells::new();
        put(&mut ms, 1, 1, t("Mine Statistics (Gold)"));
        let mut sites = vec![
            (2 + sh, "Carlin (61.5%)".to_string()),
            (4 + sh, format!("Hemlo{}", fnum(3))),
        ];
        if drift {
            sites.push((2, "Pueblo Viejo (60%)".to_string()));
        }
        for header_row in [3, 12 + rs] {
            for (c, name) in &sites {
                put(&mut ms, *c, header_row, t(name));
            }
        }
        let three = if drift {
            "For the  three months ended June 30,"
        } else {
            "For the three months ended June 30,"
        };
        let six = if drift {
            "For the six months ended Juen 30,"
        } else {
            "For the six months ended June 30,"
        };
        for (row, label) in [(4, three), (13 + rs, six)] {
            put(&mut ms, 1, row, t(label));
            for (c, _) in &sites {
                put(&mut ms, *c, row, MinimalCell::Number(2030.0));
                put(&mut ms, c + 1, row, MinimalCell::Number(2029.0));
            }
        }
        if drift {
            put(&mut ms, 1, 5, t("Revenue ($ millions)"));
            for c in 2..=7 {
                put(&mut ms, c, 5, MinimalCell::Number(100.0));
            }
        }
        let n = MinimalCell::Number;
        let (carlin, hemlo) = (2 + sh, 4 + sh);
        let rows: Vec<(u32, String, [MinimalCell; 4], [MinimalCell; 2])> = vec![
            (
                5,
                "Tonnes processed (thousands)".into(),
                [n(1250.0), n(1190.0), t("— "), t("—")],
                [n(900.0), n(880.0)],
            ),
            (
                6,
                format!("Recovery rate (percent){}", fnum(2)),
                [n(0.91), n(0.9), n(0.0), t("n/a")],
                [n(0.85), n(0.84)],
            ),
            (
                8,
                "Processed".into(),
                [n(3.12), n(2.98), t(""), t("n/a")],
                [n(2.5), n(2.4)],
            ),
            (
                9,
                format!("Total cash costs ($/oz){}", fnum(4)),
                [n(1100.0), n(1050.0), t("n/a"), n(1400.0)],
                [n(950.0), n(900.0)],
            ),
            (
                14,
                "Tonnes processed (thousands)".into(),
                [n(2500.0), n(2380.0), n(300.0), n(310.0)],
                [n(1800.0), n(1700.0)],
            ),
        ];
        put(&mut ms, 1, 7 + rs, t("Average grade (grams per tonne)"));
        put(
            &mut ms,
            1,
            10 + rs,
            t("1 Barrick's share of the site's figures."),
        );
        for (row, label, [a, b, c, d], [pa, pb]) in rows {
            let row = row + rs;
            put(&mut ms, 1, row, t(&label));
            for (col, v) in [(carlin, a), (carlin + 1, b), (hemlo, c), (hemlo + 1, d)] {
                if v != t("") {
                    put(&mut ms, col, row, v);
                }
            }
            if drift {
                put(&mut ms, 2, row, pa);
                put(&mut ms, 3, row, pb);
            }
        }

        let mut qtd = Cells::new();
        put(&mut qtd, 1, 3, t("For the three months ended June 30,"));
        put(&mut qtd, 2, 3, n(2030.0));
        put(&mut qtd, 9, 3, n(2029.0));
        let qs = u32::from(drift);
        let headers = [
            "Gold produced (000s oz attributable)1".to_string(),
            "Cost of sales ($/oz)".to_string(),
            format!("All-in sustaining costs ($/oz){}", fnum(2)),
        ];
        for base in [2, 9] {
            if drift {
                put(&mut qtd, base, 4, t("Gold produced (000s oz 100%)"));
                put(&mut qtd, base, 5, n(667.0));
            }
            for (k, h) in headers.iter().enumerate() {
                put(&mut qtd, base + qs + k as u32, 4, t(h));
            }
        }
        let carlin_q = [
            n(410.0),
            n(1300.0),
            n(1650.0),
            n(400.0),
            n(1280.0),
            n(1600.0),
        ];
        let hemlo_q = [t("— "), t("— "), t("—"), n(50.0), n(1500.0), n(1900.0)];
        for (row, site, vals) in [
            (5, "Carlin (61.5%)".to_string(), carlin_q),
            (6, format!("Hemlo{}", fnum(2)), hemlo_q),
        ] {
            put(&mut qtd, 1, row, t(&site));
            for (k, v) in vals.into_iter().enumerate() {
                let k = k as u32;
                let col = if k < 3 { 2 + qs + k } else { 9 + qs + (k - 3) };
                put(&mut qtd, col, row, v);
            }
        }
        if drift {
            put(&mut qtd, 1, 7, t("Pueblo Viejo (60%)"));
            put(&mut qtd, 3, 7, n(120.0));
        }
        (ms, qtd)
    }

    fn workbook((ms, qtd): (Cells, Cells)) -> Workbook {
        let bytes = build_minimal_xlsx(&[
            MinimalSheet {
                name: "QTD".to_string(),
                hidden: false,
                cells: qtd,
            },
            MinimalSheet {
                name: "Minestats1".to_string(),
                hidden: false,
                cells: ms,
            },
        ]);
        Workbook::from_bytes("synthetic_mine_stats.xlsx", &bytes)
            .expect("synthetic workbook parses")
    }

    fn value<'a>(read: &'a MineStatsRead, locator: &str) -> &'a MineStatsValue {
        read.values
            .iter()
            .find(|v| v.locator == locator)
            .unwrap_or_else(|| panic!("no value at {locator}"))
    }

    #[test]
    fn test_label_rules() {
        assert_eq!(
            strip_footnote("Total cash costs3"),
            ("Total cash costs".to_string(), Some(3))
        );
        assert_eq!(
            strip_footnote("  Gold  produced (000s oz)12 "),
            ("Gold produced (000s oz)".to_string(), Some(12))
        );
        assert_eq!(
            strip_footnote("Q2 2026"),
            ("Q2 2026".to_string(), None),
            "digits after a space stay"
        );
        assert_eq!(
            strip_footnote("Tonnes (000s)"),
            ("Tonnes (000s)".to_string(), None)
        );
        assert_eq!(
            strip_footnote("Grade123"),
            ("Grade123".to_string(), None),
            "three digits are not a footnote"
        );
        assert_eq!(
            strip_footnote("Total\u{a0}cash costs 4"),
            ("Total cash costs 4".to_string(), None)
        );
        assert_eq!(site_name("Long Canyon (61.5%)1"), "Long Canyon");
        assert_eq!(site_name("Hemlo3"), "Hemlo");
        assert_eq!(site_name("Copper - Total"), "Total Copper");
        assert_eq!(site_name("Zaldívar (50%)"), "Zaldívar");
        assert_eq!(
            printed_share("Nevada Gold Mines LLC (61.5%)2"),
            Some("61.5")
        );
        assert_eq!(
            printed_share("Lumwana"),
            None,
            "no share printed means 100%, recorded as unprinted"
        );
        assert_eq!(printed_share("Site (n/a%)"), None);
        assert!(footnote_line("1 Barrick's share") && footnote_line("  12\tNote"));
        assert!(!footnote_line("2026") && !footnote_line("Tonnes"));
        let field = |h: &str| summary_field(h).expect("maps");
        assert_eq!(
            field("Gold produced (000s oz 100%)").as_deref(),
            Some("au_prod_100_koz")
        );
        assert_eq!(
            field("Gold produced (000s oz attributable)3").as_deref(),
            Some("au_prod_attr_koz")
        );
        assert_eq!(
            field("Copper sold (thousands of tonnes attributable)").as_deref(),
            Some("cu_sold_attr_kt")
        );
        assert_eq!(
            field("Total Cost of sales ($/oz)").as_deref(),
            Some("cos_usd_per_oz")
        );
        assert_eq!(
            field("C1 Cash Costs ($/lb)4").as_deref(),
            Some("c1_usd_per_lb")
        );
        assert_eq!(
            field("All-in sustaining costs ($/lb)").as_deref(),
            Some("aisc_usd_per_lb")
        );
        assert_eq!(field("Site"), None);
        assert!(
            summary_field("Total cash costs").is_err(),
            "a field with no metal is refused"
        );
    }

    /// Known answers on the synthetic layout: locators, periods, sites, sections, footnotes, raw tokens,
    /// numeric zero kept as a number, a blank in a row with data as Missing, and the summary field ids.
    #[test]
    fn test_synthetic_known_answers() {
        let read = read_mine_stats(&workbook(fixture(false)), "2030Q2", None).expect("read");
        assert!(read.refusals.is_empty(), "{:?}", read.refusals);
        assert_eq!(read.blocks, 3);
        assert_eq!(read.values.len(), 32);
        let kinds = ["number", "nil", "na", "missing"].map(|k| read.count(k));
        assert_eq!(kinds, [23, 5, 3, 1]);
        assert_eq!(read.numeric_zeros(), 1);

        let rec = value(&read, "2030Q2:Minestats1!B6");
        assert_eq!(
            (rec.label.as_str(), rec.footnote, rec.section.as_deref()),
            ("Recovery rate (percent)", Some(2), None)
        );
        assert_eq!(
            (
                rec.site.as_str(),
                rec.printed_share.as_deref(),
                rec.period.as_str()
            ),
            ("Carlin", Some("61.5"), "2030Q2")
        );
        assert_eq!(
            rec.value,
            MineStatsCell::Number {
                raw_text: "0.91".into(),
                value: 0.91
            }
        );
        let zero = value(&read, "2030Q2:Minestats1!D6");
        assert_eq!(
            (zero.site.as_str(), zero.printed_share.as_deref()),
            ("Hemlo", None)
        );
        assert_eq!(
            zero.value,
            MineStatsCell::Number {
                raw_text: "0".into(),
                value: 0.0
            }
        );
        assert_eq!(
            value(&read, "2030Q2:Minestats1!D5").value,
            MineStatsCell::Nil {
                token: "— ".into()
            }
        );
        assert_eq!(
            value(&read, "2030Q2:Minestats1!E5").value,
            MineStatsCell::Nil {
                token: "—".into()
            }
        );
        assert_eq!(
            value(&read, "2030Q2:Minestats1!E6").value,
            MineStatsCell::NotApplicable {
                token: "n/a".into()
            }
        );
        let processed = value(&read, "2030Q2:Minestats1!D8");
        assert_eq!(processed.value, MineStatsCell::Missing);
        assert_eq!(
            processed.section.as_deref(),
            Some("Average grade (grams per tonne)")
        );
        assert_eq!(value(&read, "2030Q2:Minestats1!C8").period, "2029Q2");
        assert!(
            read.values.iter().all(|v| v.cell != "B7"),
            "a label row with no data is not emitted"
        );
        let cum = value(&read, "2030Q2:Minestats1!C14");
        assert_eq!((cum.months, cum.period.as_str()), (6, "2029H1"));

        let cos = value(&read, "2030Q2:QTD!C5");
        assert_eq!(
            (cos.view, cos.label.as_str(), cos.period.as_str()),
            (View::Summary, "cos_usd_per_oz", "2030Q2")
        );
        assert_eq!(cos.label_as_printed, "Cost of sales ($/oz)");
        assert_eq!(value(&read, "2030Q2:QTD!K6").label, "aisc_usd_per_oz");
        assert_eq!(value(&read, "2030Q2:QTD!K6").period, "2029Q2");
        assert_eq!(
            value(&read, "2030Q2:QTD!B6").value,
            MineStatsCell::Nil {
                token: "— ".into()
            }
        );
        let m0 = read
            .values
            .iter()
            .filter(|v| is_m0_target(v, "2030"))
            .count();
        assert_eq!(
            m0,
            2 + 2 + 2 * 2,
            "QTD cost of sales and AISC for two sites, grade and recovery for two sites"
        );
        assert_eq!(
            read.digest(),
            read.clone().digest(),
            "the digest depends only on the values"
        );
    }

    /// The drifted layout reads to the same values: no coordinate from one vintage is used on another.
    #[test]
    fn test_drifted_layout_reads_identically() {
        let key = |v: &MineStatsValue| {
            (
                v.view.as_str(),
                v.period.clone(),
                v.site.clone(),
                v.label.clone(),
                v.section.clone(),
            )
        };
        let base = read_mine_stats(&workbook(fixture(false)), "2030Q2", None).expect("read");
        let drift = read_mine_stats(&workbook(fixture(true)), "2030Q2", None).expect("read");
        assert!(drift.refusals.is_empty(), "{:?}", drift.refusals);
        assert_eq!(drift.blocks, 3);
        let base_map: BTreeMap<_, _> = base
            .values
            .iter()
            .map(|v| (key(v), v.value.clone()))
            .collect();
        let drift_map: BTreeMap<_, _> = drift
            .values
            .iter()
            .map(|v| (key(v), v.value.clone()))
            .collect();
        assert_eq!(base_map.len(), base.values.len(), "keys are unique");
        for (k, v) in &base_map {
            assert_eq!(drift_map.get(k), Some(v), "{k:?}");
        }
        let extra: BTreeSet<_> = drift_map
            .keys()
            .filter(|k| !base_map.contains_key(*k))
            .map(|k| (k.2.clone(), k.3.clone()))
            .collect();
        let extra_sites_or_labels: BTreeSet<_> = extra
            .iter()
            .map(|(s, l)| {
                if s == "Pueblo Viejo" {
                    s.as_str()
                } else {
                    l.as_str()
                }
            })
            .collect();
        assert_eq!(
            extra_sites_or_labels,
            BTreeSet::from(["Pueblo Viejo", "Revenue ($ millions)", "au_prod_100_koz"]),
            "only the inserted site, row and column are new"
        );
        assert_ne!(
            base.digest(),
            drift.digest(),
            "the locators moved, so the canonical lines differ"
        );
    }

    /// Negative controls: each refusal names its cell, and nothing is emitted for it.
    #[test]
    fn test_refusals_name_the_cell() {
        let (mut ms, qtd) = fixture(false);
        put(&mut ms, 4, 5, t("x"));
        let read = read_mine_stats(&workbook((ms, qtd)), "2030Q2", None).expect("read");
        assert_eq!(read.refusals.len(), 1);
        assert_eq!(read.refusals[0].locator, "2030Q2:Minestats1!D5");
        assert!(
            read.refusals[0].reason.contains("outside the set"),
            "{}",
            read.refusals[0].reason
        );
        assert_eq!(read.values.len(), 31);

        let (mut ms, qtd) = fixture(false);
        put(&mut ms, 1, 9, t("Tonnes processed (thousands)2"));
        let read = read_mine_stats(&workbook((ms, qtd)), "2030Q2", None).expect("read");
        assert_eq!(read.refusals.len(), 1);
        assert_eq!(read.refusals[0].locator, "2030Q2:Minestats1!A9");
        assert!(read.refusals[0].reason.contains("appears twice"));
        assert_eq!(read.values.len(), 28, "the repeated row is not emitted");

        let (mut ms, qtd) = fixture(false);
        put(&mut ms, 1, 13, t("For the fortnight ended June 30,"));
        let read = read_mine_stats(&workbook((ms, qtd)), "2030Q2", None).expect("read");
        assert_eq!(read.refusals.len(), 1);
        assert_eq!(read.refusals[0].locator, "2030Q2:Minestats1!A13");
        assert_eq!(
            (read.blocks, read.values.len()),
            (2, 28),
            "the unknown block is not read"
        );

        let (ms, mut qtd) = fixture(false);
        put(&mut qtd, 3, 5, MinimalCell::Error("#REF!".into()));
        let read = read_mine_stats(&workbook((ms, qtd)), "2030Q2", None).expect("read");
        assert_eq!(read.refusals.len(), 1);
        assert_eq!(read.refusals[0].locator, "2030Q2:QTD!C5");

        let refused =
            read_mine_stats(&workbook(fixture(false)), "2030-Q2", None).expect_err("bad quarter");
        assert!(refused.to_string().contains("not YYYYQn"), "{refused}");
    }

    /// Under the vocabulary of the base layout, the drifted layout reads the same except for the one row whose
    /// label is new (Revenue), which is refused by locator, in both blocks it would appear in.
    #[test]
    fn test_unregistered_label_is_refused_by_locator() {
        let base = read_mine_stats(&workbook(fixture(false)), "2030Q2", None).expect("read");
        let vocabulary: LabelVocabulary = base
            .values
            .iter()
            .filter(|v| v.view == View::Minestats)
            .map(|v| (v.section.clone(), v.label.clone()))
            .collect();
        let again =
            read_mine_stats(&workbook(fixture(false)), "2030Q2", Some(&vocabulary)).expect("read");
        assert!(again.refusals.is_empty() && again.values == base.values);

        let drift =
            read_mine_stats(&workbook(fixture(true)), "2030Q2", Some(&vocabulary)).expect("read");
        assert_eq!(drift.refusals.len(), 1, "{:?}", drift.refusals);
        assert_eq!(drift.refusals[0].locator, "2030Q2:Minestats1!A5");
        assert!(
            drift.refusals[0]
                .reason
                .contains("\"Revenue ($ millions)\"")
        );
        assert!(
            drift
                .values
                .iter()
                .all(|v| v.label != "Revenue ($ millions)")
        );

        let mut narrow = vocabulary.clone();
        narrow.remove(&(
            Some("Average grade (grams per tonne)".to_string()),
            "Processed".to_string(),
        ));
        let read =
            read_mine_stats(&workbook(fixture(false)), "2030Q2", Some(&narrow)).expect("read");
        assert_eq!(read.refusals.len(), 1);
        assert_eq!(
            read.refusals[0].locator, "2030Q2:Minestats1!A8",
            "a sub-row is keyed by its section too"
        );
    }

    /// The registry decides which rows are sub-rows; W1's list, used without a vocabulary, must say the same.
    #[test]
    fn test_child_rows_equal_the_registered_sections() {
        let profile = load_issuer_profile(&workspace_root(), "barrick").expect("registry");
        let registered: BTreeSet<String> = profile
            .mine_stats_labels
            .iter()
            .filter(|(section, _)| section.is_some())
            .map(|(_, label)| label.to_lowercase())
            .collect();
        let code: BTreeSet<String> = CHILD_ROWS.iter().map(|s| (*s).to_string()).collect();
        assert_eq!(registered, code);
    }

    /// A unit the reader does not know refuses by locator: a summary header in ounces rather than thousands,
    /// a summary header naming no field, and (under the vocabulary) a `Minestats*` label in millions.
    #[test]
    fn test_unknown_unit_is_refused_by_locator() {
        let (ms, mut qtd) = fixture(false);
        put(&mut qtd, 2, 4, t("Gold produced (oz attributable)1"));
        let read = read_mine_stats(&workbook((ms, qtd)), "2030Q2", None).expect("read");
        assert_eq!(read.refusals.len(), 1, "{:?}", read.refusals);
        assert_eq!(read.refusals[0].locator, "2030Q2:QTD!B4");
        assert!(
            read.refusals[0].reason.contains("not its unit \"000s oz\""),
            "{}",
            read.refusals[0].reason
        );
        assert!(
            read.values
                .iter()
                .all(|v| !(v.view == View::Summary && v.locator.ends_with("!B5"))),
            "the column is not emitted under a wrong unit"
        );

        let (ms, mut qtd) = fixture(false);
        put(&mut qtd, 9, 4, t("Gold output (000s oz attributable)"));
        let read = read_mine_stats(&workbook((ms, qtd)), "2030Q2", None).expect("read");
        assert_eq!(read.refusals.len(), 1, "{:?}", read.refusals);
        assert_eq!(read.refusals[0].locator, "2030Q2:QTD!I4");
        assert!(read.refusals[0].reason.contains("names no known field"));

        let base = read_mine_stats(&workbook(fixture(false)), "2030Q2", None).expect("read");
        let vocabulary: LabelVocabulary = base
            .values
            .iter()
            .filter(|v| v.view == View::Minestats)
            .map(|v| (v.section.clone(), v.label.clone()))
            .collect();
        let (mut ms, qtd) = fixture(false);
        put(&mut ms, 1, 5, t("Tonnes processed (millions)"));
        let read =
            read_mine_stats(&workbook((ms, qtd)), "2030Q2", Some(&vocabulary)).expect("read");
        assert_eq!(read.refusals.len(), 1, "{:?}", read.refusals);
        assert_eq!(read.refusals[0].locator, "2030Q2:Minestats1!A5");
    }

    fn workspace_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
    }

    /// GATE-BULL-01 part 1 on the ten registered Mine Stats workbooks. Every expected count and digest comes
    /// from W1's openpyxl reader (`scripts/oracle/mine_stats_extract3.py`, run 2026-10-29 before this code; its
    /// output `long_all3.json` sha256 8992ae50…81dd), turned into canonical lines by
    /// `scripts/oracle/mine_stats_lines.py`. None of it comes from this implementation.
    ///
    /// `[cells, number, numeric zero, nil, na, missing]`; "number" includes the zeros. Set
    /// `FMINE_MINE_STATS_DUMP=<dir>` to write this reader's lines per file for a diff against the oracle's.
    #[test]
    #[ignore = "needs the registered Barrick Mine Stats workbooks under corpus_of_input_docs/downloaded/mine_stats/"]
    fn barrick_mine_stats_match_the_w1_reader() {
        let oracle: &[(&str, [usize; 6], &str)] = &[
            (
                "Barrick_Q1_2024_Mine_Stats.xlsx",
                [1380, 1082, 0, 108, 190, 0],
                "4e513f4c0c7eca8bd2a4a43a3c66356a4d8b6f873b0d558e54d6c1ac767411ec",
            ),
            (
                "Barrick_Q2_2024_Mine_Stats.xlsx",
                [2760, 2204, 0, 183, 373, 0],
                "c5241d4c2cd55bc1ea982c98ca54e2fadd135e1437f3e3babaa1bbe3b33747f9",
            ),
            (
                "Barrick_Q3_2024_Mine_Stats.xlsx",
                [2760, 2210, 0, 177, 373, 0],
                "d23779d499353369690f833f38cb1d33f0f9a780325973db0ee1c517b012b90b",
            ),
            (
                "Barrick_Q4_2024_Mine_Stats.xlsx",
                [2760, 2213, 0, 173, 372, 2],
                "bd7b23ee73b2f2782cc20fb3462cf9407ed83f9d01aa748d4bab15e76d8cf31f",
            ),
            (
                "Barrick_Q1_2025_Mine_Stats.xlsx",
                [1292, 1095, 0, 52, 145, 0],
                "f41e13223eb52d3be07fc4181735156df4b9385467e9722dd4446095b57c0abc",
            ),
            (
                "Barrick_Q2_2025_Mine_Stats.xlsx",
                [2584, 2213, 0, 82, 288, 1],
                "3915d7b17e746664b0a0c10e92c6d3b0dc975395974f564dba40b74b596dda9b",
            ),
            (
                "Barrick_Q3_2025_Mine_Stats.xlsx",
                [2584, 2220, 0, 82, 281, 1],
                "c8a90061ceca3e84bb4dca7828bb0660bcc46faf4eb7ef2b53b7fa39b1a75720",
            ),
            (
                "Barrick_Q4_2025_Mine_Stats.xlsx",
                [2616, 2266, 0, 44, 306, 0],
                "4705cfb746cc307058c17b1751fa9b3226758d75dc853d53e8fd3036b4a2e6ce",
            ),
            (
                "Barrick_Q1_2026_Mine_Stats.xlsx",
                [1308, 1066, 3, 90, 152, 0],
                "2728f03979a853be77bc08c6796041496fef98af8d89077a8ca401fac0fe42f3",
            ),
            (
                "Barrick_Q2_2026_Mine_Stats.xlsx",
                [2688, 2206, 4, 182, 298, 2],
                "92f18041a04855e5df35951a3511f1ae2db19509edb6cfdd763b4268b3210788",
            ),
        ];
        let root = workspace_root();
        let profile = load_issuer_profile(&root, "barrick")
            .and_then(|p| p.through("2026Q2"))
            .expect("registry");
        let sources = load_registered_sources(&profile, &root)
            .expect("registered files present and unaltered");
        let dump = std::env::var_os("FMINE_MINE_STATS_DUMP").map(PathBuf::from);
        // The registered vocabulary is in force: every label of the ten files must be registered.
        let vocabulary: LabelVocabulary = profile.mine_stats_labels.iter().cloned().collect();
        assert!(
            !vocabulary.is_empty(),
            "barrick registers a Mine Stats vocabulary"
        );
        let mut reads = BTreeMap::new();
        for src in sources.iter().filter(|s| s.kind == SourceKind::MineStats) {
            let read = read_mine_stats(&src.workbook, &src.quarter, Some(&vocabulary))
                .expect("registered quarter");
            let got = [
                read.values.len(),
                read.count("number"),
                read.numeric_zeros(),
                read.count("nil"),
                read.count("na"),
                read.count("missing"),
            ];
            let digest = read.digest().to_hex();
            eprintln!(
                "{{\"file\":\"{}\",\"sha256\":\"{}\",\"blocks\":{},\"cells\":{},\"cells_numeric\":{},\"numeric_zero\":{},\
                 \"cells_nil\":{},\"cells_na\":{},\"cells_missing\":{},\"refusals\":{},\"digest\":\"{digest}\"}}",
                src.file_name,
                Digest32(src.workbook.sha256).to_hex(),
                read.blocks,
                got[0],
                got[1],
                got[2],
                got[3],
                got[4],
                got[5],
                read.refusals.len()
            );
            if let Some(dir) = &dump {
                let text: String = read
                    .canonical_lines()
                    .iter()
                    .map(|l| format!("{l}\n"))
                    .collect();
                std::fs::write(dir.join(format!("{}.rust.tsv", src.file_name)), text)
                    .expect("write dump");
            }
            reads.insert(src.file_name.clone(), (read, got, digest));
        }
        assert_eq!(
            reads.len(),
            oracle.len(),
            "every registered Mine Stats file through 2026Q2 is read"
        );
        for (file, counts, digest) in oracle {
            let (read, got, d) = reads
                .get(*file)
                .unwrap_or_else(|| panic!("{file} not loaded"));
            assert!(read.refusals.is_empty(), "{file}: {:?}", read.refusals);
            assert!(
                read.hidden_sheets.is_empty(),
                "{file}: {:?}",
                read.hidden_sheets
            );
            assert_eq!(got, counts, "{file} counts");
            assert_eq!(d, digest, "{file} canonical-line digest");
        }

        // Plan §21.6 / C1: the Q1 and Q2 2026 files hold 3,996 parsed cells, and the seven numeric-zero
        // placeholders (recovery of divested Hemlo and Tongon, C1 DR-07) are numbers with stored text "0".
        let in_2026: usize = [
            "Barrick_Q1_2026_Mine_Stats.xlsx",
            "Barrick_Q2_2026_Mine_Stats.xlsx",
        ]
        .iter()
        .map(|f| reads[*f].0.values.len())
        .sum();
        assert_eq!(in_2026, 3_996);
        let mut zeros: Vec<&str> = reads
            .values()
            .flat_map(|(r, ..)| r.values.iter().filter(|v| v.is_numeric_zero()))
            .map(|v| {
                assert_eq!(
                    v.value,
                    MineStatsCell::Number {
                        raw_text: "0".into(),
                        value: 0.0
                    },
                    "{}",
                    v.locator
                );
                v.locator.as_str()
            })
            .collect();
        zeros.sort_unstable();
        assert_eq!(
            zeros,
            [
                "2026Q1:Minestats1!N27",
                "2026Q1:Minestats1!N28",
                "2026Q1:Minestats3!F19",
                "2026Q2:Minestats1!N27",
                "2026Q2:Minestats1!N28",
                "2026Q2:Minestats3!F19",
                "2026Q2:Minestats3!F64",
            ]
        );

        // W1 §4, the M0 target-cell census: 820 = 744 numeric, 33 '— ', 11 '—', 28 n/a, 4 numeric zero.
        // W1 states the counts, not the selection; the selection in `is_m0_target` is the one that reproduces
        // them from W1's own read (checked in Python on long_all3.json before this assertion was written).
        let mut census = BTreeMap::new();
        for (r, ..) in reads.values() {
            for v in r.values.iter().filter(|v| is_m0_target(v, &r.quarter[..4])) {
                let k = match &v.value {
                    MineStatsCell::Number { .. } if v.is_numeric_zero() => "zero",
                    MineStatsCell::Number { .. } => "numeric",
                    MineStatsCell::Nil { token } if token == "— " => "dash_space",
                    MineStatsCell::Nil { .. } => "dash",
                    MineStatsCell::NotApplicable { .. } => "na",
                    MineStatsCell::Missing => "missing",
                };
                *census.entry(k).or_insert(0usize) += 1;
            }
        }
        assert_eq!(
            census,
            BTreeMap::from([
                ("dash", 11),
                ("dash_space", 33),
                ("na", 28),
                ("numeric", 744),
                ("zero", 4)
            ])
        );
    }
}
