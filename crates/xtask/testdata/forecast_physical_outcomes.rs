//! Invented Minestats workbooks exercise physical targets through the same durable public journal.

use super::*;
use fmine_claim::AdditivityClass;

fn styles() -> MinimalStyles {
    MinimalStyles {
        num_fmts: vec![],
        xfs: vec![1, 2, 9, 10],
    }
}

fn physical_sheet(months: &str, site: &str) -> MinimalSheet {
    let text = |value: &str| MinimalCell::Text(value.into());
    let number = |raw: &str, xf| MinimalCell::Styled {
        raw: raw.into(),
        xf,
    };
    MinimalSheet {
        name: "Minestats1".into(),
        hidden: false,
        cells: vec![
            ("B2".into(), text(site)),
            (
                "A3".into(),
                text(&format!("For the {months} months ended December 31,")),
            ),
            ("B3".into(), MinimalCell::Number(2030.0)),
            ("A4".into(), text("Tonnes processed (thousands)")),
            ("B4".into(), number("1200.4", 0)),
            ("A5".into(), text("Average grade (grams per tonne)")),
            ("A6".into(), text("Processed")),
            ("B6".into(), number("1.2345", 1)),
            ("A7".into(), text("Recovery rate (percent)")),
            ("B7".into(), number("0.914", 2)),
            ("A8".into(), text("Average grade")),
            ("A9".into(), text("Processed")),
            ("B9".into(), number("0.00594", 3)),
            // Other process sections are real competitors only when their complete field matches.
            ("A10".into(), text("Open Pit mined")),
            ("B10".into(), number("0.008", 3)),
        ],
    }
}

fn replace(sheet: &mut MinimalSheet, cell: &str, value: MinimalCell) {
    sheet.cells.iter_mut().find(|(at, _)| at == cell).unwrap().1 = value;
}

fn fixture(metric: &str, period: &str, ownership: &str) -> Fixture {
    let mut fixture = Fixture::with_payload(
        metric,
        ownership,
        period,
        "kind = \"empirical\"\nsamples = \"0,0.5,1,2,1200\"",
        Some("registered_comparatives"),
    );
    fixture.replace_styled_workbook(
        &[physical_sheet(
            if period.ends_with("FY") {
                "twelve"
            } else {
                "three"
            },
            "Site A (60%)",
        )],
        &styles(),
    );
    fixture
}

#[test]
fn physical_cells_preserve_exact_printed_units_and_saved_diagnostics() {
    for (metric, cell, display, numerator, denominator, unit, additive) in [
        (
            "processed_kt",
            "B4",
            "1200",
            1200,
            1,
            "kt",
            AdditivityClass::ExtensiveAdditive,
        ),
        (
            "processed_grade_g_per_t",
            "B6",
            "1.23",
            123,
            100,
            "g_per_t",
            AdditivityClass::IntensiveWeighted,
        ),
        (
            "processed_grade_fraction",
            "B9",
            "0.59%",
            59,
            10000,
            "mass_fraction",
            AdditivityClass::IntensiveWeighted,
        ),
        (
            "recovery_fraction",
            "B7",
            "91%",
            91,
            100,
            "mass_fraction",
            AdditivityClass::IntensiveWeighted,
        ),
    ] {
        for period in ["2030FY", "2030Q4"] {
            let mut fixture = fixture(metric, period, "attributable:3/5");
            let locator = format!("2030Q4.xlsx:Minestats1!{cell}");
            let source = fixture.source_claim(&locator);
            assert_eq!(source.key.unit, unit);
            assert_eq!(source.claim.additivity, additive);
            let resolved = fixture.ok(fixture.resolve(), ns(AVAILABLE));
            for fragment in [
                format!("\"displayed\":\"{display}\""),
                format!(
                    "\"exact_value\":{{\"numerator\":\"{numerator}\",\"denominator\":\"{denominator}\"}}"
                ),
                source.claim.claim_id.to_string(),
            ] {
                assert!(
                    resolved.payload_json.contains(&fragment),
                    "{metric}: {fragment}: {}",
                    resolved.payload_json
                );
            }
            assert!(
                resolved
                    .rendered_markdown
                    .as_ref()
                    .unwrap()
                    .contains(&format!("{numerator}/{denominator}"))
            );
            let persisted =
                DurableJournal::open_read(&fixture.root.join(JOURNAL), Some(fixture.head)).unwrap();
            let operation = persisted
                .records()
                .iter()
                .find_map(|record| {
                    if let Entry::Operation(text) = &record.entry {
                        parse_subset(text)
                            .unwrap()
                            .rows
                            .into_iter()
                            .find(|row| row.table == "registered_outcome")
                    } else {
                        None
                    }
                })
                .unwrap();
            for (field, expected) in [
                ("sha256", fixture.source_digest.to_hex()),
                ("locator", locator.clone()),
                ("claim_id", source.claim.claim_id.to_string()),
                ("value", display.to_string()),
                ("numerator", numerator.to_string()),
                ("denominator", denominator.to_string()),
            ] {
                assert_eq!(
                    operation.get(field).map(|field| &field.value),
                    Some(&TomlValue::Str(expected)),
                    "{metric}: durable admission {field}"
                );
            }
            drop(persisted);
            let assessed = fixture.ok(
                Command::Assess {
                    journal: JOURNAL.into(),
                    head: fixture.head,
                    cohort: COHORT.into(),
                    as_of: CUT.into(),
                    selector: "first-published".into(),
                    expected_digest: None,
                },
                ns(CUT),
            );
            let expected = assessment(&assessed.payload_json).to_string();
            assert!(expected.contains("\"n_members\":1,\"n_resolved\":1,\"n_unresolved\":0"));
            assert!(expected.contains("\"crps\":{\"n_scored\":1"));
            assert!(expected.contains(&source.claim.claim_id.to_string()));
            assert!(expected.contains(&format!(
                "\"citation\":{{\"source\":\"{ISSUER}\",\"vintage\":\"2030Q4\",\"locator\":\"{locator}\",\"sha256\":\"{}\"}}",
                fixture.source_digest.to_hex()
            )));
            assert!(assessed.payload_json.contains("empirical_rank"));
            let saved = digest_field(&expected, "digest");
            let before = fixture.bytes();
            // Offline receipt replay must retain the same fraction and evidence after source replacement.
            fs::write(
                fixture.root.join("corpus/2030Q4.xlsx"),
                b"not the registered workbook",
            )
            .unwrap();
            let reopened = execute_with_clock(
                &Command::Report {
                    journal: JOURNAL.into(),
                    head: Some(fixture.head),
                    assessment: saved,
                    provenance: None,
                    timestamp: None,
                },
                &fixture.root,
                &NeverClock,
            );
            assert_eq!(reopened.exit_code, 0, "{reopened:?}");
            assert_eq!(assessment(&reopened.payload_json), expected);
            assert_eq!(fixture.bytes(), before);
        }
    }
}

#[test]
fn physical_admission_refuses_unit_scope_basis_calendar_ambiguity_and_altered_bytes() {
    for (case, metric, code) in [
        ("unprinted", "recovery_fraction", "FMINE-REF-105"),
        ("wrong_share", "recovery_fraction", "FMINE-REF-105"),
        ("tonnes_not_grossed_up", "processed_kt", "FMINE-REF-105"),
        ("percent_not_fraction", "recovery_fraction", "FMINE-REF-105"),
        (
            "grade_wrong_section",
            "processed_grade_g_per_t",
            "FMINE-REF-201",
        ),
        ("tonnes_wrong_unit", "processed_kt", "FMINE-REF-201"),
        (
            "circuit_not_site_recovery",
            "recovery_fraction",
            "FMINE-REF-201",
        ),
        ("negative", "processed_kt", "FMINE-REF-208"),
        ("over_one", "recovery_fraction", "FMINE-REF-208"),
        ("duplicate_sheet", "recovery_fraction", "FMINE-REF-104"),
        ("duplicate_label", "recovery_fraction", "FMINE-REF-208"),
        ("calendar", "recovery_fraction", "FMINE-REF-102"),
        ("bytes", "recovery_fraction", "FMINE-REF-505"),
    ] {
        let ownership = if case == "tonnes_not_grossed_up" {
            "100%"
        } else {
            "attributable:3/5"
        };
        let fixture_owner = if case == "wrong_share" {
            "attributable:1/2"
        } else {
            ownership
        };
        let mut fixture = fixture(metric, "2030FY", fixture_owner);
        let mut sheet = physical_sheet("twelve", "Site A (60%)");
        let text = |value: &str| MinimalCell::Text(value.into());
        match case {
            "unprinted" => replace(&mut sheet, "B2", text("Site A")),
            "percent_not_fraction" => replace(
                &mut sheet,
                "B7",
                MinimalCell::Styled {
                    raw: "91".into(),
                    xf: 0,
                },
            ),
            "grade_wrong_section" => replace(&mut sheet, "A5", text("Average grade (percent)")),
            "tonnes_wrong_unit" => replace(&mut sheet, "A4", text("Tonnes processed (tonnes)")),
            "circuit_not_site_recovery" => replace(&mut sheet, "A7", text("Oxide Mill")),
            "negative" => replace(
                &mut sheet,
                "B4",
                MinimalCell::Styled {
                    raw: "-1".into(),
                    xf: 0,
                },
            ),
            "over_one" => replace(
                &mut sheet,
                "B7",
                MinimalCell::Styled {
                    raw: "1.01".into(),
                    xf: 2,
                },
            ),
            "duplicate_label" => sheet.cells.extend([
                ("A11".into(), text("Recovery rate (percent)")),
                (
                    "B11".into(),
                    MinimalCell::Styled {
                        raw: "0.85".into(),
                        xf: 2,
                    },
                ),
            ]),
            "calendar" => replace(
                &mut sheet,
                "A3",
                text("For the twelve months ended June 30,"),
            ),
            _ => {}
        }
        let mut sheets = vec![sheet];
        if case == "duplicate_sheet" {
            let mut duplicate = physical_sheet("twelve", "Site A (150%)");
            duplicate.name = "Minestats2".into();
            sheets.push(duplicate);
        }
        fixture.replace_styled_workbook(&sheets, &styles());
        if case == "bytes" {
            let path = fixture.root.join("corpus/2030Q4.xlsx");
            let mut bytes = fs::read(&path).unwrap();
            bytes.push(0);
            fs::write(path, bytes).unwrap();
        }
        fixture.refuse(&fixture.resolve(), ns(AVAILABLE), code);
    }
}

#[test]
fn physical_endpoints_missing_values_and_formatted_zero_keep_distinct_meanings() {
    for (raw, expected) in [("0", "0"), ("1", "1")] {
        let mut fixture = fixture("recovery_fraction", "2030FY", "100%");
        let mut sheet = physical_sheet("twelve", "Site A (100%)");
        replace(
            &mut sheet,
            "B7",
            MinimalCell::Styled {
                raw: raw.into(),
                xf: 2,
            },
        );
        fixture.replace_styled_workbook(&[sheet], &styles());
        let outcome = fixture.ok(fixture.resolve(), ns(AVAILABLE));
        assert!(outcome.payload_json.contains(&format!(
            "\"numerator\":\"{expected}\",\"denominator\":\"1\""
        )));
    }
    let mut missing = fixture("processed_kt", "2030FY", "attributable:3/5");
    let mut sheet = physical_sheet("twelve", "Site A (60%)");
    replace(&mut sheet, "B4", MinimalCell::Text("—".into()));
    missing.replace_styled_workbook(&[sheet], &styles());
    let outcome = missing.ok(missing.resolve(), ns(AVAILABLE));
    assert!(
        outcome
            .payload_json
            .contains("\"source_value_kind\":\"nil_unknown\"")
    );
    assert!(
        outcome
            .payload_json
            .contains("\"missing_reason\":\"not_published\"")
    );

    let mut fixture = fixture("recovery_fraction", "2030FY", "attributable:3/5");
    let mut sheet = physical_sheet("twelve", "Site A (60%)");
    // A styled percent dash preserves its explicit stored-unit interpretation.
    replace(
        &mut sheet,
        "B7",
        MinimalCell::Styled {
            raw: "0".into(),
            xf: 4,
        },
    );
    let mut styled = styles();
    styled.num_fmts.push((164, "0%;-0%;\"—\"".into()));
    styled.xfs.push(164);
    fixture.replace_styled_workbook(&[sheet], &styled);
    // A formatted numeric zero is still a numeric zero, never a fabricated unpublished outcome.
    fixture.refuse(&fixture.resolve(), ns(AVAILABLE), "FMINE-REF-208");
}

#[test]
fn physical_fraction_restatements_keep_original_scores_and_saved_publication_evidence() {
    let mut fixture = fixture("recovery_fraction", "2030FY", "attributable:3/5");
    fixture.ok(fixture.resolve(), ns(AVAILABLE));
    let first = fixture.ok(
        Command::Assess {
            journal: JOURNAL.into(),
            head: fixture.head,
            cohort: COHORT.into(),
            as_of: CUT.into(),
            selector: "first-published".into(),
            expected_digest: None,
        },
        ns(CUT),
    );
    let original = assessment(&first.payload_json).to_string();
    let saved = digest_field(&original, "digest");
    let mut later = physical_sheet("twelve", "Site A (60%)");
    replace(
        &mut later,
        "B7",
        MinimalCell::Styled {
            raw: "0.854".into(),
            xf: 2,
        },
    );
    let bytes = build_minimal_xlsx_styled(&[later], Some(&styles()));
    let digest = sha256(&bytes);
    fs::write(fixture.root.join("corpus/2031Q4.xlsx"), &bytes).unwrap();
    let registry = registry_path(&fixture.root, ISSUER);
    let mut text = fs::read_to_string(&registry).unwrap();
    text.push_str(&format!(
        "\n[[source]]\nfile = \"2031Q4.xlsx\"\nkind = \"mine_stats\"\nquarter = \"2031Q4\"\npath = \"2031Q4.xlsx\"\nurl_basis = \"W1 table\"\nbytes = {}\nsha256 = \"{digest}\"\npublished_at = \"2032-02-15\"\npublished_at_basis = \"FACT: invented physical comparative\"\n",
        bytes.len(),
    ));
    fs::write(registry, text).unwrap();
    let revision = fixture.ok(restate(&fixture, "2031Q4"), ns("2032-02-16T00:00:00Z"));
    assert!(
        revision
            .payload_json
            .contains("\"numerator\":\"17\",\"denominator\":\"20\"")
    );
    for (selector, value, publication) in [
        ("first-published", "0.91", "2030Q4.xlsx:Minestats1!B7"),
        ("latest-as-of", "0.85", "2031Q4.xlsx:Minestats1!B7"),
    ] {
        let result = fixture.ok(
            Command::Assess {
                journal: JOURNAL.into(),
                head: fixture.head,
                cohort: COHORT.into(),
                as_of: "2032-02-17T00:00:00Z".into(),
                selector: selector.into(),
                expected_digest: None,
            },
            ns("2032-02-17T00:00:00Z"),
        );
        let body = assessment(&result.payload_json);
        assert!(
            body.contains(&format!("\"status\":\"resolved\",\"value\":{value},")),
            "{body}"
        );
        assert!(body.contains(publication));
    }
    let reopened = execute_with_clock(
        &Command::Report {
            journal: JOURNAL.into(),
            head: Some(fixture.head),
            assessment: saved,
            provenance: None,
            timestamp: None,
        },
        &fixture.root,
        &NeverClock,
    );
    assert_eq!(reopened.exit_code, 0, "{reopened:?}");
    assert_eq!(assessment(&reopened.payload_json), original);
}

#[test]
fn physical_percent_receipts_reject_rescaling_and_changed_source_numbers() {
    let mut original = fixture("recovery_fraction", "2030FY", "attributable:3/5");
    original.ok(original.resolve(), ns(AVAILABLE));
    let journal =
        DurableJournal::open_read(&original.root.join(JOURNAL), Some(original.head)).unwrap();
    let receipt = journal
        .records()
        .iter()
        .find_map(|record| match &record.entry {
            Entry::Operation(text) if text.contains("[[registered_outcome]]") => Some(text.clone()),
            _ => None,
        })
        .unwrap();
    drop(journal);
    for changed in [
        receipt.replace("denominator = \"100\"", "denominator = \"1\""),
        receipt.replace("value = \"91%\"", "value = \"91\""),
        receipt
            .replace("value = \"91%\"", "value = \"92%\"")
            .replace("numerator = \"91\"", "numerator = \"92\""),
    ] {
        assert_ne!(changed, receipt);
        let recipient = fixture("recovery_fraction", "2030FY", "attributable:3/5");
        let mut writer =
            DurableJournal::open_write(&recipient.root.join(JOURNAL), recipient.head).unwrap();
        writer
            .append(&changed, &FixedClock(ns(AVAILABLE)), |_, _| Ok(()))
            .unwrap();
        let head = writer.head();
        drop(writer);
        let before = recipient.bytes();
        let replay = execute_with_clock(
            &Command::Inspect {
                journal: JOURNAL.into(),
                head: Some(head),
            },
            &recipient.root,
            &NeverClock,
        );
        assert_ne!(
            replay.exit_code, 0,
            "an intact frame cannot change the admitted fraction: {replay:?}"
        );
        assert_eq!(recipient.bytes(), before);
    }
}
