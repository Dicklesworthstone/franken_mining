//! Invented half-year and nine-month filings exercise frozen cumulative targets through the journal.

use super::*;

const PERIODS: [(&str, &str, &str); 2] = [
    ("2030H1", "six", "June 30"),
    ("20309M", "nine", "September 30"),
];

fn releases(period: &str) -> (&'static str, &'static str) {
    match period {
        "2030H1" => ("2030Q2", "2031Q2"),
        "20309M" => ("2030Q3", "2031Q3"),
        _ => panic!("unknown fixture period"),
    }
}

fn publish(fixture: &Fixture, release: &str, published: &str, sheets: &[MinimalSheet]) {
    register_source_bytes(
        fixture,
        release,
        published,
        &build_minimal_xlsx_styled(
            sheets,
            Some(&MinimalStyles {
                num_fmts: vec![],
                xfs: vec![1],
            }),
        ),
    );
}

fn set(sheet: &mut MinimalSheet, address: &str, value: MinimalCell) {
    sheet
        .cells
        .iter_mut()
        .find(|(at, _)| at == address)
        .unwrap()
        .1 = value;
}

#[test]
fn cumulative_public_targets_resolve_original_ytd_cells_and_preserve_revised_scores() {
    for (period, months, end) in PERIODS {
        let (original_release, later_release) = releases(period);
        for (metric, ownership, cell, header) in [
            (
                "production_koz",
                "100%",
                "B5",
                "All-in sustaining costs ($/oz)",
            ),
            (
                "production_kt",
                "100%",
                "E5",
                "All-in sustaining costs ($/oz)",
            ),
            (
                "aisc_usd_per_oz",
                "attributable:3/5",
                "D5",
                "All-in sustaining costs ($/oz)",
            ),
            (
                "tcc_usd_per_oz",
                "attributable:3/5",
                "D5",
                "Total cash costs ($/oz)",
            ),
            (
                "aisc_usd_per_lb",
                "attributable:3/5",
                "D5",
                "All-in sustaining costs ($/lb)",
            ),
            (
                "c1_usd_per_lb",
                "attributable:3/5",
                "D5",
                "C1 cash costs ($/lb)",
            ),
        ] {
            let mut fixture = Fixture::with_vintages(
                metric,
                ownership,
                period,
                90,
                110,
                Some("registered_comparatives"),
            );
            let mut ytd = sheet("YTD", months, "100.4", header, end);
            set(
                &mut ytd,
                cell,
                MinimalCell::Styled {
                    raw: "100.4".into(),
                    xf: 0,
                },
            );
            // The same release also has a quarter value. It must not substitute for cumulative support.
            let qtd = sheet("QTD", "three", "999.4", header, end);
            publish(
                &fixture,
                original_release,
                "2031-02-15",
                &[ytd.clone(), qtd],
            );
            let locator = format!("{original_release}.xlsx:YTD!{cell}");
            let source = fixture.source_claim(&locator);
            assert_eq!(source.key.valid_period.code(), period);
            let resolved = fixture.ok(fixture.resolve(), ns(AVAILABLE));
            assert!(
                resolved
                    .payload_json
                    .contains(&source.claim.claim_id.to_string())
            );
            assert!(resolved.payload_json.contains(&locator));
            assert!(resolved.payload_json.contains("\"numerator\":\"100\""));

            let first = assess_revision(&mut fixture, CUT, "first-published");
            let first_body = assessment(&first.payload_json);
            // Central 80% [90,110]: a hit scores its width, 20; a miss at 115 adds 10*5.
            assert!(
                first_body.contains("\"interval_score\":{\"n_scored\":1,\"observed_mean\":20,")
            );
            assert!(first_body.contains("\"value\":100,"));
            let first_digest = digest_field(first_body, "digest");
            let saved = full_report(&first.payload_json).to_string();

            set(
                &mut ytd,
                cell,
                MinimalCell::Styled {
                    raw: "115.4".into(),
                    xf: 0,
                },
            );
            publish(&fixture, later_release, "2032-02-15", &[ytd]);
            fixture.ok(restate(&fixture, later_release), ns("2032-02-16T00:00:00Z"));
            let revised = assess_revision(&mut fixture, "2032-02-17T00:00:00Z", "latest-as-of");
            let revised_body = assessment(&revised.payload_json);
            assert!(
                revised_body.contains("\"interval_score\":{\"n_scored\":1,\"observed_mean\":70,")
            );
            assert!(revised_body.contains("\"value\":115,"));
            assert!(revised_body.contains(&format!("\"vintage\":\"{later_release}\"")));
            let still_first =
                assess_revision(&mut fixture, "2032-02-18T00:00:00Z", "first-published");
            assert!(assessment(&still_first.payload_json).contains("\"value\":100,"));

            // Saved cumulative reports are independent of the current source files and registry.
            fs::write(registry_path(&fixture.root, ISSUER), b"offline").unwrap();
            let reopened = reopen_revision(&fixture, first_digest);
            assert_eq!(full_report(&reopened.payload_json), saved);
        }
    }
}

#[test]
fn cumulative_site_physical_and_capital_targets_keep_exact_printed_scope() {
    for (period, months, end) in PERIODS {
        for (metric, label, value) in [
            ("processed_kt", "Tonnes processed (thousands)", "1200.4"),
            (
                "capex_usd_m",
                "Total capital expenditures ($ millions)",
                "-12.4",
            ),
        ] {
            let mut fixture = Fixture::new(metric, "attributable:3/5", period, -20, 1300);
            let text = |value: &str| MinimalCell::Text(value.into());
            let row = MinimalSheet {
                name: "Minestats1".into(),
                hidden: false,
                cells: vec![
                    ("B2".into(), text("Site A (60%)")),
                    (
                        "A3".into(),
                        text(&format!("For the {months} months ended {end},")),
                    ),
                    ("B3".into(), MinimalCell::Number(2030.0)),
                    ("A4".into(), text(label)),
                    (
                        "B4".into(),
                        MinimalCell::Styled {
                            raw: value.into(),
                            xf: 0,
                        },
                    ),
                ],
            };
            let release = releases(period).0;
            publish(&fixture, release, "2031-02-15", &[row]);
            let source = fixture.source_claim(&format!("{release}.xlsx:Minestats1!B4"));
            assert_eq!(source.key.valid_period.code(), period);
            let resolved = fixture.ok(fixture.resolve(), ns(AVAILABLE));
            assert!(
                resolved
                    .payload_json
                    .contains(&source.claim.claim_id.to_string())
            );
            let expected = if metric == "processed_kt" {
                "1200"
            } else {
                "-12"
            };
            assert!(
                resolved
                    .payload_json
                    .contains(&format!("\"numerator\":\"{expected}\""))
            );
        }
    }
}

#[test]
fn cumulative_resolution_never_substitutes_quarters_or_infers_calendar_headers() {
    for (period, months, end) in PERIODS {
        let fixture = Fixture::new("production_koz", "100%", period, 90, 110);
        let (release, _) = releases(period);
        // The default fixture contains only Q4. No later release may stand in for the original.
        fixture.refuse(&fixture.resolve(), ns(AVAILABLE), "FMINE-REF-201");
        publish(
            &fixture,
            release,
            "2031-02-15",
            &[sheet(
                "QTD",
                "three",
                "100.4",
                "All-in sustaining costs ($/oz)",
                end,
            )],
        );
        fixture.refuse(&fixture.resolve(), ns(AVAILABLE), "FMINE-REF-201");
        // Matching duration alone cannot turn a noncalendar six/nine-month window into this target.
        publish(
            &fixture,
            release,
            "2031-02-15",
            &[sheet(
                "YTD",
                months,
                "100.4",
                "All-in sustaining costs ($/oz)",
                "December 31",
            )],
        );
        fixture.refuse(&fixture.resolve(), ns(AVAILABLE), "FMINE-REF-102");
        publish(
            &fixture,
            release,
            "2030-06-30",
            &[sheet(
                "YTD",
                months,
                "100.4",
                "All-in sustaining costs ($/oz)",
                end,
            )],
        );
        fixture.refuse(&fixture.resolve(), ns(AVAILABLE), "FMINE-REF-108");
    }
}

#[test]
fn a_status_inside_cumulative_support_retains_the_member_as_unresolved() {
    for (period, months, end) in PERIODS {
        let mut fixture = Fixture::new("production_koz", "100%", period, 90, 110);
        publish(
            &fixture,
            releases(period).0,
            "2031-02-15",
            &[sheet(
                "YTD",
                months,
                "100.4",
                "All-in sustaining costs ($/oz)",
                end,
            )],
        );
        register_subject_status(
            &fixture,
            "withheld",
            "first = \"2030Q2\"\nlast = \"2030Q2\"",
        );
        fixture.ok(fixture.resolve(), ns(AVAILABLE));
        let scored = assess_revision(&mut fixture, CUT, "first-published");
        let body = assessment(&scored.payload_json);
        assert!(body.contains("\"n_members\":1,\"n_resolved\":0,\"n_unresolved\":1"));
        assert!(body.contains("\"reason\":\"definition_changed\""));
    }
}
