//! Label-anchored `.xlsx` (`xlsx`) and HTML (`extract_html_tables`) filing table extractors
//! with SHA-256 provenance and `G-BULL-1` / `G-BULL-2` quarantine (`WP-03`, `BD-056`, `BD-057`, `BD-271`).

pub mod numfmt;
pub mod styles;
pub mod xlsx;
pub mod zip_inflate;

use std::fmt;

use xlsx::{CellValue, decode_xml_entities};

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

/// Extracted HTML `<table>` from SEC/SEDAR+ filing disclosures (`BD-056`).
#[derive(Debug, Clone, PartialEq)]
pub struct HtmlTable {
    pub table_index: usize,
    pub rows: Vec<Vec<CellValue>>,
}

/// Extracts all `<table>...</table>` grids from an HTML filing document, typing numeric cells
/// and explicit placeholders (`"-"`, `"N/A"`) (`BD-056`, `G-BULL-2`).
#[must_use]
pub fn extract_html_tables(html: &str) -> Vec<HtmlTable> {
    let mut tables = Vec::new();
    let lower = html.to_ascii_lowercase();
    let mut pos = 0;
    let mut table_index = 0;

    while let Some(t_open) = lower[pos..].find("<table") {
        let abs_open = pos + t_open;
        let Some(gt) = lower[abs_open..].find('>') else {
            break;
        };
        let body_start = abs_open + gt + 1;
        let Some(t_close) = lower[body_start..].find("</table>") else {
            break;
        };
        let table_slice = &html[body_start..body_start + t_close];
        let table_lower = &lower[body_start..body_start + t_close];
        pos = body_start + t_close + 8;

        let mut rows = Vec::new();
        let mut r_pos = 0;
        while let Some(tr_open) = table_lower[r_pos..].find("<tr") {
            let abs_tr = r_pos + tr_open;
            let Some(tr_gt) = table_lower[abs_tr..].find('>') else {
                break;
            };
            let tr_start = abs_tr + tr_gt + 1;
            let Some(tr_close) = table_lower[tr_start..].find("</tr>") else {
                break;
            };
            let row_slice = &table_slice[tr_start..tr_start + tr_close];
            let row_lower = &table_lower[tr_start..tr_start + tr_close];
            r_pos = tr_start + tr_close + 5;

            let mut cells = Vec::new();
            let mut c_pos = 0;
            while c_pos < row_lower.len() {
                let next_td = row_lower[c_pos..].find("<td");
                let next_th = row_lower[c_pos..].find("<th");
                let (cell_off, close_tag) = match (next_td, next_th) {
                    (Some(a), Some(b)) => {
                        if a <= b {
                            (a, "</td>")
                        } else {
                            (b, "</th>")
                        }
                    }
                    (Some(a), None) => (a, "</td>"),
                    (None, Some(b)) => (b, "</th>"),
                    (None, None) => break,
                };
                let abs_c = c_pos + cell_off;
                let Some(c_gt) = row_lower[abs_c..].find('>') else {
                    break;
                };
                let c_start = abs_c + c_gt + 1;
                let Some(c_close) = row_lower[c_start..].find(close_tag) else {
                    break;
                };
                let raw_cell = strip_html_tags(&row_slice[c_start..c_start + c_close]);
                cells.push(parse_html_cell_value(&raw_cell));
                c_pos = c_start + c_close + close_tag.len();
            }
            if !cells.is_empty() {
                rows.push(cells);
            }
        }
        tables.push(HtmlTable { table_index, rows });
        table_index += 1;
    }
    tables
}

fn strip_html_tags(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_tag = false;
    for ch in s.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(ch),
            _ => {}
        }
    }
    decode_xml_entities(&out).trim().to_string()
}

fn parse_html_cell_value(text: &str) -> CellValue {
    let t = text.trim();
    if matches!(t, "-" | "–" | "—" | "N/A" | "n/a" | "NA") {
        return CellValue::Placeholder(t.to_string());
    }
    let clean = t.replace([',', '$', ' '], "");
    let (neg, digits) =
        if let Some(inner) = clean.strip_prefix('(').and_then(|s| s.strip_suffix(')')) {
            (true, inner)
        } else {
            (false, clean.as_str())
        };
    if !digits.is_empty()
        && let Ok(v) = digits.parse::<f64>()
        && v.is_finite()
    {
        return CellValue::Number(if neg { -v } else { v });
    }
    CellValue::Text(t.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::xlsx::{MinimalCell, MinimalSheet, QuarantineItem, Workbook, build_minimal_xlsx};
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
}
