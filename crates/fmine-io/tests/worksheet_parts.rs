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
        // A number that is not finite was dropped without a word.
        (r#"<c r="B1"><v>1e999</v></c>"#, "Production!B1"),
        (r#"<c r="B1"><v>NaN</v></c>"#, "Production!B1"),
        (r#"<c r="B1"><v>-inf</v></c>"#, "Production!B1"),
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

// Review of 8c5aa00c, 3cf449a6 and fe638ca7 (2026-11-27): inputs a standards-following XML parser reads
// differently from the scanner, each one a value two readers would disagree on.

#[test]
fn comments_and_processing_instructions_are_not_read_as_cells_strings_or_styles() {
    for cells in [
        r#"<c r="A1"><v>1</v></c><!-- <c r="B1"><v>999</v></c> -->"#,
        r#"<c r="A1"><!--<v>999</v>--><v>1</v></c>"#,
        r#"<!-- old <c> kept for audit --><c r="A1"><?audit note?><v>1</v></c>"#,
    ] {
        let book = Workbook::from_bytes("fixture.xlsx", &production_sheet(cells)).unwrap();
        let sheet = book.sheet("Production").unwrap();
        let a1 = sheet.cell(1, 1).unwrap();
        assert_eq!(a1.number_text.as_deref(), Some("1"), "{cells}");
        assert!(sheet.cell(1, 2).is_none(), "{cells}");
        assert_eq!(book.cell_census().numeric_cells, 1, "{cells}");
    }
    // A commented-out string would shift every index after it; a commented-out cellXfs would set the format.
    let strings: &[u8] = br#"<sst><!--<si><t>Gold produced (koz)</t></si>--><si><t>Copper produced (kt)</t></si></sst>"#;
    let styles: &[u8] =
        br#"<styleSheet><!--<cellXfs count="2"><xf numFmtId="0"/><xf numFmtId="0"/></cellXfs>-->
<cellXfs count="2"><xf numFmtId="0"/><xf numFmtId="10"/></cellXfs></styleSheet>"#;
    let bytes = labeled_workbook(
        "",
        &[("xl/sharedStrings.xml", strings), ("xl/styles.xml", styles)],
    );
    let book = Workbook::from_bytes("fixture.xlsx", &bytes).unwrap();
    let sheet = book.sheet("Production").unwrap();
    assert_eq!(
        sheet.cell(1, 1).unwrap().value,
        CellValue::Text("Copper produced (kt)".into())
    );
    let amount = sheet.cell(1, 2).unwrap().displayed().unwrap().unwrap();
    assert_eq!(amount.text(), "4200.00%");
}

#[test]
fn markup_a_parser_would_expand_or_reject_refuses_the_part() {
    for (cells, what) in [
        (r#"<c r="A1"><v><![CDATA[1]]></v></c>"#, "CDATA"),
        (r#"<c r="A1"><v>1</v></c><!-- a -- b -->"#, "--"),
        (
            r#"<c r="A1" x="<!--"><v>1</v></c><c r="B1"><v>2</v></c>-->"#,
            "attribute value",
        ),
        (r#"<c r="A1"><v>&foo;</v></c>"#, "&foo;"),
        (r#"<c r="A1" t="str"><v>AT&T</v></c>"#, "bare ampersand"),
        (r#"<c r="A1"><v>&#0;</v></c>"#, "&#0;"),
    ] {
        let err = Workbook::from_bytes("fixture.xlsx", &production_sheet(cells)).unwrap_err();
        assert!(err.to_string().contains("sheet2.xml"), "{cells}: {err}");
        assert!(err.to_string().contains(what), "{cells}: {err}");
    }
    let doctype = br#"<!DOCTYPE workbook [<!ENTITY n "Production">]><workbook><sheets>
<sheet name="Workings" state="hidden" r:id="r1"/><sheet name="&n;" r:id="r2"/></sheets></workbook>"#;
    let bytes = write_stored_zip(&[
        ("xl/workbook.xml", doctype),
        ("xl/worksheets/sheet1.xml", HIDDEN),
        ("xl/worksheets/sheet2.xml", VISIBLE),
    ]);
    let err = Workbook::from_bytes("fixture.xlsx", &bytes).unwrap_err();
    assert!(err.to_string().contains("document type"), "{err}");
}

#[test]
fn the_relationship_id_is_read_by_namespace_not_by_the_prefix_r() {
    let rel = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
    let links = br#"<Relationships><Relationship Id="r1" Target="worksheets/sheet1.xml"/>
<Relationship Id="r2" Target="worksheets/sheet2.xml"/></Relationships>"#;
    // `r` is bound to a decoy and marked ignorable; `q` is the relationships namespace, so q:id binds.
    let decoy = format!(
        r#"<workbook xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006" xmlns:r="urn:decoy"
 xmlns:q="{rel}" mc:Ignorable="r"><sheets><sheet name="Production" r:id="r1" q:id="r2"/></sheets></workbook>"#
    );
    let bytes = workbook_with_links(decoy.as_bytes(), links);
    let book = Workbook::from_bytes("fixture.xlsx", &bytes).unwrap();
    let cell = book.sheet("Production").unwrap().cell(1, 1).unwrap();
    assert_eq!(cell.number_text.as_deref(), Some("42"));
    for (workbook, what) in [
        (
            format!(
                r#"<workbook xmlns:r="{rel}" xmlns:q="{rel}"><sheets><sheet name="Production" r:id="r1" q:id="r2"/></sheets></workbook>"#
            ),
            "more than one relationship ID",
        ),
        (
            format!(
                r#"<workbook xmlns:r="{rel}"><sheets><sheet name="Production" xmlns:r="urn:decoy" r:id="r2"/></sheets></workbook>"#
            ),
            "declares a namespace",
        ),
    ] {
        let bytes = workbook_with_links(workbook.as_bytes(), links);
        let err = Workbook::from_bytes("fixture.xlsx", &bytes).unwrap_err();
        assert!(err.to_string().contains(what), "{err}");
    }
}

#[test]
fn character_references_decode_before_any_comparison_type_or_number() {
    // A duplicate spelled with a character reference is still a duplicate.
    let links = br#"<Relationships><Relationship Id="r1" Target="worksheets/sheet1.xml"/>
<Relationship Id="r&#49;" Target="worksheets/sheet2.xml"/><Relationship Id="r2" Target="worksheets/sheet2.xml"/>
</Relationships>"#;
    let bytes = workbook_with_links(WORKBOOK, links);
    let err = Workbook::from_bytes("fixture.xlsx", &bytes).unwrap_err();
    assert!(
        err.to_string()
            .contains("duplicate workbook relationship ID"),
        "{err}"
    );
    let names = br#"<workbook><sheets><sheet name="Pro&#100;uction" r:id="r1"/><sheet name="Production" r:id="r2"/>
</sheets></workbook>"#;
    let plain = br#"<Relationships><Relationship Id="r1" Target="worksheets/sheet1.xml"/>
<Relationship Id="r2" Target="worksheets/sheet2.xml"/></Relationships>"#;
    let err = Workbook::from_bytes("fixture.xlsx", &workbook_with_links(names, plain)).unwrap_err();
    assert!(
        err.to_string().contains("duplicate worksheet name"),
        "{err}"
    );

    // Values, types and states read as a parser delivers them.
    let workbook = br#"<workbook><sheets><sheet name="Workings" state="&#104;idden" r:id="r1"/>
<sheet name="Production" r:id="r2"/></sheets></workbook>"#;
    let sheet = br#"<worksheet><sheetData><row r="1"><c r="A1"><v>&#52;2</v></c>
<c r="B1" t="&#x73;"><v>0</v></c></row></sheetData></worksheet>"#;
    let bytes = write_stored_zip(&[
        ("xl/workbook.xml", workbook),
        ("xl/_rels/workbook.xml.rels", plain),
        ("xl/worksheets/sheet1.xml", HIDDEN),
        ("xl/worksheets/sheet2.xml", sheet),
        ("xl/sharedStrings.xml", STRINGS),
    ]);
    let book = Workbook::from_bytes("fixture.xlsx", &bytes).unwrap();
    assert_eq!(book.hidden_sheets.len(), 1);
    let production = book.sheet("Production").unwrap();
    assert_eq!(
        production.cell(1, 1).unwrap().value,
        CellValue::Number(42.0)
    );
    assert_eq!(
        production.cell(1, 2).unwrap().value,
        CellValue::Text("Copper produced".into())
    );
}

#[test]
fn a_legacy_xl_target_refuses_when_the_standard_resolution_names_another_part() {
    let gold: &[u8] = br#"<sst><si><t>Gold produced (koz)</t></si></sst>"#;
    let link = r#"<Relationship Id="r3" Type="urn:test/sharedStrings" Target="xl/sst.xml"/>"#;
    // Legacy part only: read the legacy way. Standard part only: read as a spreadsheet program reads it.
    for (part, label) in [
        ("xl/sst.xml", "Copper produced"),
        ("xl/xl/sst.xml", "Gold produced (koz)"),
    ] {
        let strings = if part == "xl/sst.xml" { STRINGS } else { gold };
        let book =
            Workbook::from_bytes("fixture.xlsx", &labeled_workbook(link, &[(part, strings)]))
                .unwrap();
        let cell = book.sheet("Production").unwrap().cell(1, 1).unwrap();
        assert_eq!(cell.value, CellValue::Text(label.into()), "{part}");
    }
    let both = labeled_workbook(link, &[("xl/sst.xml", STRINGS), ("xl/xl/sst.xml", gold)]);
    let err = Workbook::from_bytes("fixture.xlsx", &both).unwrap_err();
    assert!(err.to_string().contains("xl/xl/sst.xml"), "{err}");
}

#[test]
fn line_ends_and_attribute_whitespace_read_as_a_parser_reads_them() {
    // In an attribute, a literal tab or line break is a space, so these two names are one name.
    let names = b"<workbook><sheets><sheet name=\"Gold\tproduced\" r:id=\"r1\"/>\
<sheet name=\"Gold produced\" r:id=\"r2\"/></sheets></workbook>";
    let plain = br#"<Relationships><Relationship Id="r1" Target="worksheets/sheet1.xml"/>
<Relationship Id="r2" Target="worksheets/sheet2.xml"/></Relationships>"#;
    let err = Workbook::from_bytes("fixture.xlsx", &workbook_with_links(names, plain)).unwrap_err();
    assert!(
        err.to_string().contains("duplicate worksheet name"),
        "{err}"
    );
    // In text, CR LF is LF; `&#13;` is still a CR.
    let strings: &[u8] = b"<sst><si><t>Copper\r\nproduced&#13;</t></si></sst>";
    let bytes = labeled_workbook("", &[("xl/sharedStrings.xml", strings)]);
    let book = Workbook::from_bytes("fixture.xlsx", &bytes).unwrap();
    assert_eq!(
        book.sheet("Production").unwrap().cell(1, 1).unwrap().value,
        CellValue::Text("Copper\nproduced\r".into())
    );
}

#[test]
fn the_package_root_must_name_xl_workbook_xml_as_the_workbook() {
    for (target, read) in [
        ("xl/workbook.xml", true),
        ("/xl/workbook.xml", true),
        ("xl/real.xml", false),
    ] {
        let root = format!(
            r#"<Relationships><Relationship Id="d" Target="{target}"
 Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument"/></Relationships>"#
        );
        let bytes = write_stored_zip(&[
            ("_rels/.rels", root.as_bytes()),
            ("xl/workbook.xml", WORKBOOK),
            ("xl/real.xml", WORKBOOK),
            ("xl/worksheets/sheet1.xml", HIDDEN),
            ("xl/worksheets/sheet2.xml", VISIBLE),
        ]);
        match Workbook::from_bytes("fixture.xlsx", &bytes) {
            Ok(_) => assert!(read, "{target} was read"),
            Err(err) => {
                assert!(!read, "{target}: {err}");
                assert!(err.to_string().contains("xl/real.xml"), "{err}");
            }
        }
    }
}
