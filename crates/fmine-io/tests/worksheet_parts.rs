//! Workbook relationship resolution and extraction fidelity (BD-271).
use fmine_io::xlsx::{CellValue, Workbook};
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

const LABELED: &[u8] = br#"<worksheet><sheetData><row r="1">
<c r="A1" t="s"><v>0</v></c><c r="B1" s="1"><v>42</v></c>
</row></sheetData></worksheet>"#;
const STRINGS: &[u8] = br#"<sst><si><t>Copper produced</t></si></sst>"#;
const STYLES: &[u8] =
    br#"<styleSheet><cellXfs><xf numFmtId="0"/><xf numFmtId="2"/></cellXfs></styleSheet>"#;

fn labeled_workbook(metadata_links: &str, metadata: &[(&str, &[u8])]) -> Vec<u8> {
    let rels = format!(
        r#"<Relationships>
<Relationship Id="r1" Target="worksheets/sheet1.xml"/>
<Relationship Id="r2" Target="worksheets/sheet2.xml"/>
{metadata_links}
</Relationships>"#,
    );
    let mut entries = vec![
        ("xl/workbook.xml", WORKBOOK),
        ("xl/_rels/workbook.xml.rels", rels.as_bytes()),
        ("xl/worksheets/sheet1.xml", HIDDEN),
        ("xl/worksheets/sheet2.xml", LABELED),
    ];
    entries.extend_from_slice(metadata);
    write_stored_zip(&entries)
}

#[test]
fn reads_declared_shared_strings_and_styles_at_nonstandard_paths() {
    let bytes = labeled_workbook(
        r#"<Relationship Id="r3" Type="urn:test/sharedStrings" Target="../data/shared&amp;strings.xml"/>
<Relationship Id="r4" Type="urn:test/styles" Target="/metadata/styles.xml"/>"#,
        &[
            ("data/shared&strings.xml", STRINGS),
            ("metadata/styles.xml", STYLES),
        ],
    );
    let book = Workbook::from_bytes("fixture.xlsx", &bytes).unwrap();
    let sheet = book.sheet("Production").unwrap();
    assert_eq!(
        sheet.cell(1, 1).unwrap().value,
        CellValue::Text("Copper produced".into()),
    );
    let amount = sheet.cell(1, 2).unwrap();
    assert_eq!(amount.number_text.as_deref(), Some("42"));
    assert_eq!(amount.displayed().unwrap().unwrap().text(), "42.00");
    assert_eq!(book.cell_census().numeric_cells, 1);
}

#[test]
fn refuses_a_missing_declared_metadata_part_even_if_the_standard_path_exists() {
    for kind in ["sharedStrings", "styles"] {
        let rels = format!(
            r#"<Relationship Id="r3" Type="urn:test/{kind}" Target="../data/missing.xml"/>"#
        );
        let bytes = labeled_workbook(
            &rels,
            &[("xl/sharedStrings.xml", STRINGS), ("xl/styles.xml", STYLES)],
        );
        let err = Workbook::from_bytes("fixture.xlsx", &bytes).unwrap_err();
        assert!(err.to_string().contains("data/missing.xml"), "{err}");
    }
}

#[test]
fn refuses_an_unresolved_shared_string_with_its_cell_locator() {
    let bytes = labeled_workbook("", &[("xl/sharedStrings.xml", b"<sst/>")]);
    let err = Workbook::from_bytes("fixture.xlsx", &bytes).unwrap_err();
    assert!(err.to_string().contains("Production!A1"), "{err}");
}
