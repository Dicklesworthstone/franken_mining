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

fn workbook_with_links(workbook: &[u8], links: &[u8]) -> Vec<u8> {
    write_stored_zip(&[
        ("xl/workbook.xml", workbook),
        ("xl/_rels/workbook.xml.rels", links),
        ("xl/worksheets/sheet1.xml", HIDDEN),
        ("xl/worksheets/sheet2.xml", VISIBLE),
    ])
}

#[test]
fn duplicate_relationship_id_refuses_instead_of_selecting_the_last_target() {
    let links = br#"<Relationships>
<Relationship Id="r1" Target="worksheets/sheet1.xml"/>
<Relationship Id="r2" Target="worksheets/sheet2.xml"/>
<Relationship Id="r2" Target="worksheets/sheet1.xml"/>
</Relationships>"#;
    let bytes = workbook_with_links(WORKBOOK, links);
    let err = Workbook::from_bytes("fixture.xlsx", &bytes).unwrap_err();
    assert!(
        err.to_string().contains("duplicate workbook relationship"),
        "{err}"
    );
    assert!(err.to_string().contains("r2"), "{err}");
}

#[test]
fn an_external_sheet_relationship_refuses_even_when_its_target_exists_in_the_zip() {
    let links = br#"<Relationships>
<Relationship Id="r1" Target="worksheets/sheet1.xml"/>
<Relationship Id="r2" TargetMode="External" Target="worksheets/sheet2.xml"/>
</Relationships>"#;
    let bytes = workbook_with_links(WORKBOOK, links);
    let err = Workbook::from_bytes("fixture.xlsx", &bytes).unwrap_err();
    assert!(err.to_string().contains("external"), "{err}");
    assert!(err.to_string().contains("Production"), "{err}");
}

#[test]
fn duplicate_binding_attributes_refuse_instead_of_choosing_one_value() {
    let valid_links = br#"<Relationships>
<Relationship Id="r1" Target="worksheets/sheet1.xml"/>
<Relationship Id="r2" Target="worksheets/sheet2.xml"/>
</Relationships>"#;
    let duplicate_target = br#"<Relationships>
<Relationship Id="r1" Target="worksheets/sheet1.xml"/>
<Relationship Id="r2" Target="worksheets/sheet2.xml" Target="worksheets/sheet1.xml"/>
</Relationships>"#;
    let duplicate_sheet_binding = br#"<workbook><sheets>
<sheet name="Workings" state="hidden" r:id="r1"/>
<sheet name="Production" r:id="r2" r:id="r1"/>
</sheets></workbook>"#;
    for (workbook, links, attribute) in [
        (WORKBOOK, duplicate_target.as_slice(), "Target"),
        (
            duplicate_sheet_binding.as_slice(),
            valid_links.as_slice(),
            "r:id",
        ),
    ] {
        let bytes = workbook_with_links(workbook, links);
        let err = Workbook::from_bytes("fixture.xlsx", &bytes).unwrap_err();
        assert!(err.to_string().contains("duplicate attribute"), "{err}");
        assert!(err.to_string().contains(attribute), "{err}");
    }
}

#[test]
fn visible_and_hidden_sheets_cannot_share_a_name_or_part() {
    let distinct_parts = br#"<Relationships>
<Relationship Id="r1" Target="worksheets/sheet1.xml"/>
<Relationship Id="r2" Target="worksheets/sheet2.xml"/>
</Relationships>"#;
    let duplicate_name = br#"<workbook><sheets>
<sheet name="PRODUCTION" state="hidden" r:id="r1"/>
<sheet name="Production" r:id="r2"/>
</sheets></workbook>"#;
    let same_part = br#"<Relationships>
<Relationship Id="r1" Target="worksheets/sheet1.xml"/>
<Relationship Id="r2" Target="/xl/worksheets/nested/../sheet1.xml"/>
</Relationships>"#;
    for (workbook, links, diagnostic) in [
        (
            duplicate_name.as_slice(),
            distinct_parts.as_slice(),
            "duplicate worksheet name",
        ),
        (WORKBOOK, same_part.as_slice(), "shared worksheet part"),
    ] {
        let bytes = workbook_with_links(workbook, links);
        let err = Workbook::from_bytes("fixture.xlsx", &bytes).unwrap_err();
        assert!(err.to_string().contains(diagnostic), "{err}");
    }
}

#[test]
fn a_sheet_without_a_name_refuses_instead_of_shrinking_the_census() {
    let workbook = br#"<workbook><sheets>
<sheet name="Workings" state="hidden" r:id="r1"/>
<sheet r:id="r2"/>
</sheets></workbook>"#;
    let links = br#"<Relationships>
<Relationship Id="r1" Target="worksheets/sheet1.xml"/>
<Relationship Id="r2" Target="worksheets/sheet2.xml"/>
</Relationships>"#;
    let bytes = workbook_with_links(workbook, links);
    let err = Workbook::from_bytes("fixture.xlsx", &bytes).unwrap_err();
    assert!(err.to_string().contains("sheet declaration 2"), "{err}");
}

#[test]
fn quoted_and_line_wrapped_bindings_keep_the_exact_visible_cell_anchor() {
    let workbook = br#"<workbook><sheets>
<!-- <sheet name="Workings" r:id="r1"/> -->
<sheet name='Workings' state='hidden' r:id='r1'/>
<sheet
 name = 'Production' r:id = 'r2'/>
</sheets></workbook>"#;
    let links = br#"<Relationships>
<!-- <Relationship Id="r2" Target="worksheets/sheet1.xml"/> -->
<Relationship Id='r1' Target='worksheets/sheet1.xml'/>
<Relationship
 Id = 'r2' TargetMode = 'Internal' Target = 'worksheets/sheet2.xml'/>
</Relationships>"#;
    let bytes = workbook_with_links(workbook, links);
    let book = Workbook::from_bytes("fixture.xlsx", &bytes).unwrap();
    assert_eq!(book.hidden_sheets.len(), 1);
    assert_eq!(book.cell_census().numeric_cells, 1);
    let cell = book.sheet("Production").unwrap().cell(1, 1).unwrap();
    assert_eq!(cell.number_text.as_deref(), Some("42"));
    assert_eq!(
        book.make_anchor("Production", &cell.cell_ref).locator,
        "Production!A1"
    );
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

fn production_sheet(cells: &str) -> Vec<u8> {
    let sheet =
        format!(r#"<worksheet><sheetData><row r="1">{cells}</row></sheetData></worksheet>"#);
    write_stored_zip(&[
        ("xl/workbook.xml", WORKBOOK),
        ("xl/worksheets/sheet1.xml", HIDDEN),
        ("xl/worksheets/sheet2.xml", sheet.as_bytes()),
    ])
}

#[test]
fn a_cell_the_scanner_cannot_place_or_type_refuses_with_its_locator() {
    for (cells, locator) in [
        // Two values for one cell: keeping either one is a guess.
        (
            r#"<c r="B1"><v>1</v></c><c r="B1"><v>2</v></c>"#,
            "Production!B1",
        ),
        (
            r#"<c r="B1" s="0"/><c r="B1"><v>2</v></c>"#,
            "Production!B1",
        ),
        // A boolean's stored 1 is not the number 1.
        (r#"<c r="B1" t="b"><v>1</v></c>"#, "Production!B1"),
        (r#"<c r="B1" t='b'><v>1</v></c>"#, "Production!B1"),
        // A cell with no usable reference cannot be anchored.
        (r#"<c><v>5</v></c>"#, "Production cell 1"),
        (r#"<c r="1B"><v>5</v></c>"#, "Production cell 1"),
    ] {
        let err = Workbook::from_bytes("fixture.xlsx", &production_sheet(cells)).unwrap_err();
        assert!(err.to_string().contains(locator), "{cells}: {err}");
    }
}

#[test]
fn a_wrapped_cell_tag_with_single_quoted_reference_is_read() {
    let book = Workbook::from_bytes(
        "fixture.xlsx",
        &production_sheet("<c\n r='B1'\t><v>42</v></c>"),
    )
    .unwrap();
    let cell = book.sheet("Production").unwrap().cell(1, 2).unwrap();
    assert_eq!(cell.number_text.as_deref(), Some("42"));
    assert_eq!(book.cell_census().numeric_cells, 1);
}
