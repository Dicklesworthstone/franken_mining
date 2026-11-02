// Workbook-to-packet integration contracts, included in the CLI test scope to retain its private fixture seam.
    fn summary(name: &str, months: &str, rows: &[(&str, [&str; 4])]) -> MinimalSheet {
        let mut cells = vec![
            (
                "A3".into(),
                MinimalCell::Text(format!("For the {months} months ended June 30,")),
            ),
            ("B3".into(), MinimalCell::Number(2030.0)),
        ];
        for (col, label) in [
            ("B", "Gold produced (000s oz 100%)"),
            ("C", "Gold produced (000s oz attributable)"),
            ("D", "Gold sold (000s oz attributable)"),
            ("E", "Cost of sales ($/oz)"),
        ] {
            cells.push((format!("{col}4"), MinimalCell::Text(label.to_string())));
        }
        for (index, (site, values)) in rows.iter().enumerate() {
            let row = index + 5;
            cells.push((format!("A{row}"), MinimalCell::Text(site.to_string())));
            for (col, value) in ["B", "C", "D", "E"].into_iter().zip(values) {
                if !value.is_empty() {
                    cells.push((
                        format!("{col}{row}"),
                        MinimalCell::Number(value.parse().expect("numeric fixture")),
                    ));
                }
            }
        }
        MinimalSheet {
            name: name.to_string(),
            hidden: false,
            cells,
        }
    }

    fn mine_source(file: &str, quarter: &str, sheets: Vec<MinimalSheet>) -> LoadedSource {
        let bytes = build_minimal_xlsx(&sheets);
        let workbook = Workbook::from_bytes(file, &bytes).expect("synthetic workbook");
        LoadedSource {
            file_name: file.to_string(),
            kind: SourceKind::MineStats,
            quarter: quarter.to_string(),
            url: "synthetic".into(),
            url_basis: "synthetic".into(),
            bytes: bytes.len() as u64,
            census: workbook.cell_census(),
            workbook,
        }
    }

    fn vintage_fixture(values: &[Option<MinimalCell>], with_ytd: bool) -> PublicAssuranceReport {
        let mut profile = synthetic::synthetic_profile();
        profile.reconstructions.clear();
        let releases = ["2030Q2", "2031Q2", "2032Q2", "2033Q2"];
        let sources = values
            .iter()
            .enumerate()
            .map(|(i, value)| {
                let mut sheets = vec![summary(
                    "QTD",
                    "three",
                    &[("Site A (50%)", ["100", "50", "50", "100"])],
                )];
                if with_ytd {
                    sheets.push(summary(
                        "YTD",
                        "six",
                        &[("Site A (50%)", ["200", "100", "100", "100"])],
                    ));
                }
                for sheet in &mut sheets {
                    sheet.cells.retain(|(at, _)| at != "E5");
                    if let Some(value) = value {
                        sheet.cells.push(("E5".into(), value.clone()));
                    }
                }
                mine_source(&format!("vintage-{i}.xlsx"), releases[i], sheets)
            })
            .collect();
        assess(&profile, Lane::Synthetic, sources)
    }

    fn vintage_packets(report: &PublicAssuranceReport) -> Vec<ChallengePacket> {
        packets_from_report(report)
            .expect("cited packets")
            .into_iter()
            .filter(|p| {
                matches!(
                    p.kind,
                    PacketKind::VintageChange | PacketKind::VintageConflict
                )
            })
            .collect()
    }

    #[test]
    fn vintage_changes_keep_first_previous_current_without_duplicate_findings() {
        let mut report = vintage_fixture(
            &[
                Some(MinimalCell::Number(100.0)),
                Some(MinimalCell::Number(200.0)),
                Some(MinimalCell::Number(100.0)),
            ],
            true,
        );
        let packets = vintage_packets(&report);
        assert_eq!(packets.len(), 2, "Q2 and H1 changes remain separate");
        let periods: BTreeSet<_> = packets
            .iter()
            .flat_map(|p| &p.findings)
            .map(|f| f.period.as_str())
            .collect();
        assert_eq!(periods, BTreeSet::from(["2030Q2", "2030H1"]));
        for packet in &packets {
            assert_eq!(
                packet.finding_count(),
                2,
                "each later appearance occurs once"
            );
            assert_eq!(packet.operands.len(), 3);
            let second = packet
                .findings
                .iter()
                .find(|f| f.at.starts_with("vintage-2"))
                .expect("reversion remains a finding against the prior release");
            assert!(
                second
                    .evidence_json
                    .contains("\"against_first\":{\"state\":\"unchanged\"")
            );
            assert!(
                second
                    .evidence_json
                    .contains("\"against_previous\":{\"state\":\"restated_silent\"")
            );
            assert!(second.evidence_json.contains("\"rounding_only\":null"));
            for operand in packet.operands.values() {
                let claim = operand.claim.as_ref().expect("canonical Claim");
                assert!(
                    second
                        .evidence_json
                        .contains(&claim.claim.claim_id.to_string())
                );
                assert!(second.evidence_json.contains(&operand.sha256));
            }
            assert!(!packet.external_use_allowed());
            assert!(
                packet
                    .to_markdown()
                    .contains("absence of disclosure elsewhere")
            );
            packet.validate().expect("valid vintage packet");
        }
        report.sources.reverse();
        report.mine_stats_vintages.items.reverse();
        for item in &mut report.mine_stats_vintages.items {
            item.first.reverse();
            item.previous.reverse();
            item.current.reverse();
        }
        assert_eq!(
            packets
                .iter()
                .map(ChallengePacket::to_json)
                .collect::<Vec<_>>(),
            vintage_packets(&report)
                .iter()
                .map(ChallengePacket::to_json)
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn vintage_packets_preserve_number_nil_not_applicable_and_missing() {
        let report = vintage_fixture(
            &[
                Some(MinimalCell::Number(100.0)),
                Some(MinimalCell::Text("—".into())),
                Some(MinimalCell::Text("n/a".into())),
                None,
            ],
            false,
        );
        let packets = vintage_packets(&report);
        assert_eq!(packets.len(), 1);
        let packet = &packets[0];
        assert_eq!(packet.finding_count(), 3);
        assert_eq!(packet.operands.len(), 4);
        let values: Vec<_> = packet
            .operands
            .values()
            .map(|o| &o.claim.as_ref().expect("Claim").claim.value)
            .collect();
        assert!(
            values
                .iter()
                .any(|v| matches!(v, PublicValue::Number { .. }))
        );
        assert!(
            values
                .iter()
                .any(|v| matches!(v, PublicValue::Nil { token, .. } if token == "—"))
        );
        assert!(
            values
                .iter()
                .any(|v| matches!(v, PublicValue::NotApplicable { token } if token == "n/a"))
        );
        assert!(values.iter().any(|v| matches!(v, PublicValue::Missing)));
    }

    #[test]
    fn vintage_packets_reject_missing_forged_and_mutated_source_assertions() {
        let mut report = vintage_fixture(
            &[
                Some(MinimalCell::Number(100.0)),
                Some(MinimalCell::Number(200.0)),
            ],
            false,
        );
        let item = report
            .mine_stats_vintages
            .items
            .iter_mut()
            .find(|i| i.against_first.state == VintageState::RestatedSilent)
            .expect("change");
        let original = item.current[0].clone();
        item.current[0].claim.value = PublicValue::Number {
            stored: "999".into(),
            displayed: Ok("999".into()),
            under_status: None,
        };
        assert_eq!(
            packets_from_report(&report).expect_err("forged Claim").code,
            "FMINE-REF-208"
        );
        let forged = report
            .mine_stats_vintages
            .items
            .iter()
            .find(|i| i.current[0].locator == original.locator)
            .expect("change")
            .current[0]
            .clone();
        report
            .public_claims
            .cells
            .insert(original.locator.clone(), forged);
        assert_eq!(
            packets_from_report(&report)
                .expect_err("source value differs despite matching projection")
                .code,
            "FMINE-REF-208"
        );
        report
            .public_claims
            .cells
            .insert(original.locator.clone(), original.clone());
        report
            .mine_stats_vintages
            .items
            .iter_mut()
            .find(|i| i.current[0].locator == original.locator)
            .expect("change")
            .current[0] = original.clone();
        report.sources.retain(|s| s.file_name != "vintage-0.xlsx");
        assert_eq!(
            packets_from_report(&report)
                .expect_err("missing compared source")
                .code,
            "FMINE-REF-506"
        );
    }

    #[test]
    fn vintage_packets_refuse_forged_verdicts_and_predecessor_chronology() {
        let mut unchanged = vintage_fixture(
            &[
                Some(MinimalCell::Number(100.0)),
                Some(MinimalCell::Number(100.0)),
            ],
            false,
        );
        assert!(vintage_packets(&unchanged).is_empty());
        let first = unchanged.public_claims.cells["vintage-0.xlsx:QTD!E5"].clone();
        let current = unchanged.public_claims.cells["vintage-1.xlsx:QTD!E5"].clone();
        let forged = Comparison {
            state: VintageState::RestatedSilent,
            reason: Some("value"),
            token_glyph_changed: false,
            disclosures: Vec::new(),
        };
        unchanged.mine_stats_vintages.items.push(VintageItem {
            first: vec![first.clone()],
            previous: vec![first],
            current: vec![current],
            against_first: forged.clone(),
            against_previous: Some(forged),
        });
        assert_eq!(
            packets_from_report(&unchanged)
                .expect_err("unchanged source pair cannot be called silent")
                .code,
            "FMINE-REF-208"
        );
        let mut report = vintage_fixture(
            &[
                Some(MinimalCell::Number(100.0)),
                Some(MinimalCell::Number(200.0)),
                Some(MinimalCell::Number(300.0)),
            ],
            false,
        );
        let third = report
            .mine_stats_vintages
            .items
            .iter_mut()
            .find(|i| i.current[0].locator.starts_with("vintage-2"))
            .expect("third release");
        third.previous = third.first.clone();
        assert_eq!(
            packets_from_report(&report)
                .expect_err("wrong predecessor")
                .code,
            "FMINE-REF-208"
        );
    }

    #[test]
    fn vintage_ownership_conflicts_keep_printed_and_registered_basis() {
        let mut profile = synthetic::synthetic_profile();
        profile.reconstructions.clear();
        profile
            .printed_share_corrections
            .push(fmine_public::sources::PrintedShareCorrection {
                subject: "Site A".into(),
                printed: "50".into(),
                num: 1,
                den: 4,
                evidence: "SYNTHETIC-BASIS".into(),
            });
        let report = assess(
            &profile,
            Lane::Synthetic,
            vec![mine_source(
                "basis.xlsx",
                "2030Q2",
                vec![summary(
                    "QTD",
                    "three",
                    &[("Site A (50%)", ["100", "50", "50", "100"])],
                )],
            )],
        );
        let packets = vintage_packets(&report);
        assert!(!packets.is_empty());
        for packet in packets {
            assert_eq!(packet.kind, PacketKind::VintageConflict);
            assert_eq!(packet.finding_count(), 1);
            let json = packet.to_json();
            assert!(json.contains("registered_basis_correction"));
            assert!(json.contains("SYNTHETIC-BASIS"));
            assert!(json.contains("\"printed_basis\":"));
            assert!(!packet.external_use_allowed());
        }
    }

    #[test]
    fn aisc_vintage_change_uses_the_same_packet_path_and_canonical_claims() {
        let mut profile = synthetic::synthetic_profile();
        profile.reconstructions.clear();
        let sources = [
            ("first-aisc.xlsx", "2030Q1", 300.0),
            ("later-aisc.xlsx", "2030Q2", 301.0),
        ]
        .into_iter()
        .map(|(file, release, value)| {
            let mut cells: Vec<_> = synthetic::synthetic_aisc_cells()
                .into_iter()
                .filter(|(at, _)| at.starts_with('A') || at.starts_with('B'))
                .collect();
            cells.retain(|(at, _)| at != "B5");
            cells.push(("B5".into(), MinimalCell::Number(value)));
            let mut source = mine_source(
                file,
                release,
                vec![MinimalSheet {
                    name: synthetic::SYNTHETIC_TABLE_SHEET.into(),
                    hidden: false,
                    cells,
                }],
            );
            source.kind = SourceKind::AiscReconciliation;
            source
        })
        .collect();
        let report = assess(&profile, Lane::Synthetic, sources);
        let packets = vintage_packets(&report);
        assert_eq!(packets.len(), 1);
        let packet = &packets[0];
        assert!(packet.family.starts_with("aisc:"));
        assert_eq!(packet.kind, PacketKind::VintageChange);
        assert_eq!(packet.finding_count(), 1);
        assert_eq!(packet.findings[0].state, "restated_silent");
        for (at, value) in [
            ("first-aisc.xlsx:Q1 2030!B5", 300),
            ("later-aisc.xlsx:Q1 2030!B5", 301),
        ] {
            assert_eq!(packet.operands[at].value, OperandValue::Int(value));
            assert_eq!(
                packet.operands[at].claim.as_ref(),
                report.public_claims.cells.get(at)
            );
        }
    }
