//! Workbook relationship resolution and extraction fidelity (BD-271).
use fmine_io::xlsx::Workbook;
use fmine_io::zip_inflate::write_stored_zip;

const WORKBOOK: &[u8] = br#"<workbook><sheets>
<sheet name="Workings" state="hidden" r:id="r1"/>
<sheet name="Production" r:id="r2"/>
</sheets></workbook>"#;
const HIDDEN: &[u8] =
    br#"<worksheet><sheetData><row r="1"><c r="A1"><v>999</v></c></row></sheetData></worksheet>"#;
const VISIBLE: &[u8] =
    br#"<worksheet><sheetData><row r="1"><c r="A1"><v>42</v></c></row></sheetData></worksheet>"#;

#[test]
fn reads_rooted_and_relative_sheet_targets_and_legacy_workbook_order() {
    let rels = br#"<Relationships>
<Relationship Id="r1" Target="/xl/worksheets/private.xml"/>
<Relationship Id="r2" Target="../data/visible.xml"/>
</Relationships>"#;
    for bytes in [
        write_stored_zip(&[
            ("xl/workbook.xml", WORKBOOK),
            ("xl/_rels/workbook.xml.rels", rels),
            ("xl/worksheets/private.xml", HIDDEN),
            ("data/visible.xml", VISIBLE),
        ]),
        write_stored_zip(&[
            ("xl/workbook.xml", WORKBOOK),
            ("xl/worksheets/sheet1.xml", HIDDEN),
            ("xl/worksheets/sheet2.xml", VISIBLE),
        ]),
    ] {
        let book = Workbook::from_bytes("fixture.xlsx", &bytes).unwrap();
        assert_eq!(book.cell_census().numeric_cells, 1);
        let cell = book.sheet("Production").unwrap().cell(1, 1).unwrap();
        assert_eq!(cell.number_text.as_deref(), Some("42"));
        let hidden = book.hidden_sheets[0].cell(1, 1).unwrap();
        assert_eq!(hidden.number_text.as_deref(), Some("999"));
    }
}

#[test]
fn broken_relationships_refuse_the_workbook_even_if_a_positional_sheet_exists() {
    for target in ["", r#"<Relationship Id="r2" Target="/xl/missing.xml"/>"#] {
        let rels = format!(
            r#"<Relationships><Relationship Id="r1" Target="worksheets/sheet1.xml"/>{target}</Relationships>"#
        );
        let bytes = write_stored_zip(&[
            ("xl/workbook.xml", WORKBOOK),
            ("xl/_rels/workbook.xml.rels", rels.as_bytes()),
            ("xl/worksheets/sheet1.xml", HIDDEN),
            ("xl/worksheets/sheet2.xml", VISIBLE),
        ]);
        let err = Workbook::from_bytes("fixture.xlsx", &bytes).unwrap_err();
        assert!(err.to_string().contains("Production"), "{err}");
    }
}
