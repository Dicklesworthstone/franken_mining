use super::*;
use fmine_io::xlsx::{MinimalCell, MinimalSheet, Workbook, build_minimal_xlsx};
use fmine_public::{LoadedSource, assess, run_synthetic_assurance, synthetic};

fn known_packet(packets: &[ChallengePacket]) -> &ChallengePacket {
    packets
        .iter()
        .find(|p| p.subject == "Site B" && p.family == "tcc_footing")
        .expect("seeded Site B footing packet")
}

#[test]
fn seeded_footing_retains_operands_witness_and_cited_arithmetic() {
    let report = run_synthetic_assurance(&[]).expect("synthetic assurance");
    let root = report.ledger_anchor.state_root;
    let packets = packets_from_report(&report).expect("packets");
    let p = known_packet(&packets);
    assert_eq!(p.kind, PacketKind::FootingDiscrepancy);
    assert_eq!(p.findings.len(), 1);
    assert_eq!(p.source_root, root);
    assert_eq!(
        p.operands
            .values()
            .filter(|o| o.roles.contains("line"))
            .count(),
        3
    );
    for (cell, value) in [("C5", 300), ("C6", -50), ("C7", 0), ("C8", 350)] {
        let at = format!(
            "{}:{}!{cell}",
            synthetic::SYNTHETIC_AISC_FILE,
            synthetic::SYNTHETIC_TABLE_SHEET
        );
        let operand = &p.operands[&at];
        assert_eq!(operand.value, OperandValue::Int(value));
        assert_eq!(operand.sha256.len(), 64);
        assert!(operand.claim.is_some());
    }
    let evidence = &p.findings[0].evidence_json;
    assert!(evidence.contains("\"component_sum\":{\"v\":250,"));
    assert!(evidence.contains("\"residual\":{\"v\":100,"));
    assert!(evidence.contains("\"sign_flip_witness\":\"C6\""));
    assert!(evidence.contains("\"rule\":\"aisc.footing.residual\""));
    p.validate().expect("complete packet");
    assert_eq!(report.ledger_anchor.state_root, root);
}

#[test]
fn packets_cover_visible_exceptions_and_keep_distinct_states() {
    let report = run_synthetic_assurance(&[]).expect("synthetic assurance");
    let packets = packets_from_report(&report).expect("packets");
    for instance in &report.instances {
        let at = format!("{}:{}!{}", instance.file, instance.sheet, instance.cell);
        let findings = packets
            .iter()
            .filter(|p| p.family == instance.family.as_str())
            .flat_map(|p| &p.findings)
            .filter(|f| f.at == at)
            .collect::<Vec<_>>();
        if matches!(instance.state.code(), "ii" | "iii" | "iv" | "v") {
            assert_eq!(findings.len(), 1, "{at} {}", instance.family.as_str());
            assert_eq!(findings[0].state, instance.state.label());
        } else {
            assert!(
                findings.is_empty(),
                "consistent/not-published item was promoted to an exception"
            );
        }
    }
    assert!(
        packets
            .iter()
            .any(|p| p.kind == PacketKind::ExplanationStated)
    );
    assert!(
        packets
            .iter()
            .any(|p| p.kind == PacketKind::ReconstructionUnstated)
    );
    assert!(packets.iter().any(|p| p.kind == PacketKind::Undecidable));
    assert!(packets.iter().any(|p| p.kind == PacketKind::InputRefused));
    for packet in &packets {
        assert!(!packet.external_use_allowed());
        let json = packet.to_json();
        assert!(json.contains("\"external_use_allowed\":false"));
        assert!(json.contains("\"human_check\":null"));
        assert!(json.contains("\"lane\":\"synthetic\""));
        assert!(!json.contains(synthetic::SYNTHETIC_HIDDEN_SHEET));
        let md = packet.to_markdown();
        assert!(md.contains(SYNTHETIC_NO_AFFILIATION));
        assert!(!md.contains("From public filings"));
        assert!(md.contains(REVIEW_BOUNDARY));
    }
}

#[test]
fn content_address_binds_values_digests_and_closing_requests() {
    let report = run_synthetic_assurance(&[]).expect("synthetic assurance");
    let packets = packets_from_report(&report).expect("packets");
    let original = known_packet(&packets);
    let mut value_changed = original.clone();
    value_changed
        .operands
        .values_mut()
        .next()
        .expect("operand")
        .value = OperandValue::Int(991);
    assert_ne!(original.id, value_changed.content_id());
    assert!(value_changed.validate().is_err());
    let mut digest_changed = original.clone();
    digest_changed
        .operands
        .values_mut()
        .next()
        .expect("operand")
        .sha256 = "a".repeat(64);
    assert_ne!(original.id, digest_changed.content_id());
    let mut no_request = original.clone();
    no_request.closing_documents.clear();
    assert!(no_request.validate().is_err());
    let mut no_source = original.clone();
    no_source
        .operands
        .values_mut()
        .next()
        .expect("operand")
        .sha256
        .clear();
    assert!(no_source.validate().is_err());
    let mut bad_word = original.clone();
    bad_word.closing_documents.insert(ClosingDocument {
        document: "requested calculation".into(),
        locator: None,
        would_show: "a likelihood and NPV".into(),
    });
    assert!(bad_word.validate().is_err());
}

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

fn identity_transition_fixture(totals: &[i64]) -> PublicAssuranceReport {
    let mut profile = synthetic::synthetic_profile();
    profile.subjects = vec!["Site A".into()];
    profile.reconstructions.clear();
    let sources = totals
        .iter()
        .enumerate()
        .map(|(index, total)| {
            let column = ["B", "D", "C"][index % 3];
            let mut cells = vec![(format!("{column}2"), MinimalCell::Text("Site A".into()))];
            for (row, label, value) in [
                (5, "Cost of sales applicable to gold production", 100),
                (6, "Royalty adjustment", 20),
                (7, "Total cash costs", *total),
                (8, "Minesite sustaining", 30),
                (9, "All-in sustaining costs", 150),
                (10, "Ounces sold", 10),
                (11, "Cost of sales per ounce", 10_000),
                (12, "Total cash costs per ounce", 12_000),
                (13, "All-in sustaining costs per ounce", 15_000),
            ] {
                cells.push((format!("A{row}"), MinimalCell::Text(label.into())));
                cells.push((format!("{column}{row}"), MinimalCell::Number(value as f64)));
            }
            let mut source = mine_source(
                &format!("identity-{index}.xlsx"),
                &format!("{}Q2", 2030 + index),
                vec![MinimalSheet {
                    name: "Q2 2030".into(),
                    hidden: false,
                    cells,
                }],
            );
            source.kind = SourceKind::AiscReconciliation;
            source
        })
        .collect();
    assess(&profile, Lane::Synthetic, sources)
}

#[test]
fn identity_transitions_keep_corrected_broken_states_and_source_evidence() {
    let report = identity_transition_fixture(&[119, 120, 121]);
    let original = report.to_json();
    let packets = packets_from_report(&report).expect("source-verified transitions");
    let packet = packets
        .iter()
        .find(|p| p.family == "aisc_identity:tcc_footing")
        .expect("footing history");
    assert_eq!(packet.kind, PacketKind::IdentityTransition);
    assert_eq!(packet.findings.len(), 2);
    assert_eq!(
        packet
            .findings
            .iter()
            .map(|f| f.state.as_str())
            .collect::<BTreeSet<_>>(),
        BTreeSet::from(["corrected", "broken"])
    );
    for finding in &packet.findings {
        assert!(finding.evidence_json.contains("\"state\":\"iii\""));
        assert!(finding.evidence_json.contains("\"state\":\"i\""));
        assert!(finding.evidence_json.contains("\"operands_changed\":true"));
        assert!(
            finding
                .evidence_json
                .contains("\"component_sum\":{\"v\":120,")
        );
    }
    for (file, cell, value) in [
        ("identity-0.xlsx", "B7", 119),
        ("identity-1.xlsx", "D7", 120),
        ("identity-2.xlsx", "C7", 121),
    ] {
        let at = format!("{file}:Q2 2030!{cell}");
        let operand = &packet.operands[&at];
        assert_eq!(operand.value, OperandValue::Int(value));
        assert_eq!(operand.claim.as_ref(), report.public_claims.cells.get(&at));
    }
    for pair in [
        "identity-0.xlsx:Q2 2030!B7 -> identity-1.xlsx:Q2 2030!D7",
        "identity-1.xlsx:Q2 2030!D7 -> identity-2.xlsx:Q2 2030!C7",
    ] {
        assert!(
            packet
                .closing_documents
                .iter()
                .any(|r| r.locator.as_deref() == Some(pair))
        );
    }
    assert!(
        packet
            .to_json()
            .contains("\"disclosure_classification\":\"not_assigned_to_identity_transition\"")
    );
    assert!(packet.to_markdown().contains("Recorded transition"));
    assert!(packet.to_markdown().contains("not an issuer admission"));
    assert!(!packet.external_use_allowed());
    assert_eq!(
        report.to_json(),
        original,
        "original ledger and source root remain immutable"
    );
    for totals in [vec![120], vec![120, 120]] {
        assert!(
            packets_from_report(&identity_transition_fixture(&totals))
                .expect("ordinary histories")
                .iter()
                .all(|p| p.kind != PacketKind::IdentityTransition)
        );
    }
}

#[test]
fn identity_transitions_export_reclassification_without_printed_value_changes() {
    let mut profile = synthetic::synthetic_profile();
    // Without a registered map a region has a release-specific, non-comparable scope. Keep the
    // invented region's membership fixed so only the reconstruction evidence changes here.
    profile.region_members = vec![fmine_public::sources::RegionMemberRow {
        region: "Region North".into(),
        member: "Site G".into(),
        first: "2030Q1".into(),
        last: None,
    }];
    let mut sources = Vec::new();
    for (file, kind, release, bytes) in synthetic::synthetic_workbooks(&[]) {
        let identities = if kind == SourceKind::AiscReconciliation {
            vec![("earlier-aisc.xlsx", "2030Q1"), (file, "2031Q1")]
        } else {
            vec![(file, release)]
        };
        for (file, release) in identities {
            let workbook = Workbook::from_bytes(file, &bytes).expect("invented source bytes");
            sources.push(LoadedSource {
                file_name: file.into(),
                kind,
                quarter: release.into(),
                url: "synthetic".into(),
                url_basis: "synthetic".into(),
                bytes: bytes.len() as u64,
                census: workbook.cell_census(),
                workbook,
            });
        }
    }
    // The registered reconstruction applies to the later file only; both AISC workbooks contain
    // identical printed values. Its cited 13-unit component is in the retained Mine Stats workbook.
    let report = assess(&profile, Lane::Synthetic, sources);
    let earlier = &report.public_claims.cells
        [&format!("earlier-aisc.xlsx:{}!H16", synthetic::SYNTHETIC_TABLE_SHEET)];
    let current = &report.public_claims.cells[&format!(
        "{}:{}!H16",
        synthetic::SYNTHETIC_AISC_FILE,
        synthetic::SYNTHETIC_TABLE_SHEET
    )];
    assert_eq!(earlier.key.series_key(), current.key.series_key());
    assert_eq!(earlier.claim.value, current.claim.value);
    let family = aisc::IdentityFamily::AiscPerOunce;
    let history = report
        .issuer_consistency
        .histories
        .iter()
        .find(|h| h.series.subject == "Region North" && h.family == family)
        .expect("region history");
    assert_eq!(
        history.sequence.iter().map(|a| a.state).collect::<Vec<_>>(),
        vec!["iv", "ii"]
    );
    assert_eq!(history.transitions[0].kind, TransitionKind::Reclassified);
    assert!(
        report
            .aisc_vintages
            .items
            .iter()
            .all(|i| i.current.iter().all(|c| c.key.subject != "Region North")),
        "the source values themselves did not change"
    );
    let packets = packets_from_report(&report).expect("reclassification packet");
    let packet = packets
        .iter()
        .find(|p| {
            p.kind == PacketKind::IdentityTransition
                && p.subject == "Region North"
                && p.family == format!("aisc_identity:{}", family.as_str())
        })
        .expect("state-only transition is retained even without a value-change packet");
    assert_eq!(packet.findings.len(), 1);
    assert_eq!(packet.findings[0].state, "reclassified");
    assert!(
        packet.findings[0]
            .evidence_json
            .contains("\"state\":\"iv\"")
    );
    assert!(
        packet.findings[0]
            .evidence_json
            .contains("\"state\":\"ii\"")
    );
    assert!(
        packet
            .operands
            .keys()
            .any(|at| at == &format!("{}:Minestats3!B32", synthetic::SYNTHETIC_STATS_FILE))
    );
    assert!(!packet.external_use_allowed());
}

#[test]
fn identity_transitions_refuse_forged_history_and_keep_order_deterministic() {
    let report = identity_transition_fixture(&[119, 120, 121]);
    let original = packets_from_report(&report).expect("original packets");
    for mutation in ["classification", "predecessor", "omission", "state"] {
        let mut forged = report.clone();
        let history = forged
            .issuer_consistency
            .histories
            .iter_mut()
            .find(|h| h.family == aisc::IdentityFamily::TccFooting)
            .expect("footing");
        match mutation {
            "classification" => history.transitions[0].kind = TransitionKind::Persisted,
            "predecessor" => history.transitions[0].to = usize::MAX,
            "omission" => history.transitions.clear(),
            "state" => history.sequence[0].state = "i",
            _ => unreachable!(),
        }
        assert_eq!(
            packets_from_report(&forged).expect_err(mutation).code,
            "FMINE-REF-208"
        );
    }
    let mut reordered = report;
    reordered.sources.reverse();
    reordered.instances.reverse();
    reordered.issuer_consistency.histories.reverse();
    for instance in &mut reordered.instances {
        instance.operands.reverse();
    }
    for history in &mut reordered.issuer_consistency.histories {
        history.transitions.reverse();
    }
    let after = packets_from_report(&reordered).expect("same evidence in different order");
    assert_eq!(
        original
            .iter()
            .map(ChallengePacket::to_json)
            .collect::<Vec<_>>(),
        after
            .iter()
            .map(ChallengePacket::to_json)
            .collect::<Vec<_>>()
    );
    assert_eq!(
        original
            .iter()
            .map(ChallengePacket::to_markdown)
            .collect::<Vec<_>>(),
        after
            .iter()
            .map(ChallengePacket::to_markdown)
            .collect::<Vec<_>>()
    );
}

#[test]
fn identity_transitions_keep_every_ambiguous_earliest_assertion() {
    let mut sources = identity_transition_fixture(&[119, 120, 121]).sources;
    sources
        .iter_mut()
        .find(|s| s.file_name == "identity-1.xlsx")
        .expect("second assertion")
        .quarter = "2030Q2".into();
    let mut profile = synthetic::synthetic_profile();
    profile.subjects = vec!["Site A".into()];
    profile.reconstructions.clear();
    let report = assess(&profile, Lane::Synthetic, sources);
    let packets = packets_from_report(&report).expect("ambiguous identity history");
    let packet = packets
        .iter()
        .find(|p| p.family == "aisc_identity:tcc_footing")
        .expect("footing history");
    assert_eq!(packet.findings.len(), 2);
    for finding in &packet.findings {
        assert!(finding.evidence_json.contains("\"first_registered\":null"));
        assert!(
            finding
                .evidence_json
                .contains("\"earliest_registered_assertions\":[")
        );
        assert!(finding.evidence_json.contains("identity-0.xlsx:Q2 2030!B7"));
        assert!(finding.evidence_json.contains("identity-1.xlsx:Q2 2030!D7"));
    }
    assert!(packet.operands.contains_key("identity-0.xlsx:Q2 2030!B7"));
    assert!(packet.operands.contains_key("identity-1.xlsx:Q2 2030!D7"));
    assert!(
        packet
            .closing_documents
            .iter()
            .any(|r| r.would_show.contains("without choosing one arbitrarily"))
    );
}

fn disclosed_vintage_fixture(first_uses_old_section: bool) -> PublicAssuranceReport {
    use fmine_public::sources::{RegisteredSource, RestatementNoteRow, RestatementScopeRow};
    let mut profile = synthetic::synthetic_profile();
    profile.reconstructions.clear();
    profile.mine_stats_labels = ["Costs per ounce ($/oz)", "Costs per ounce"]
        .into_iter()
        .map(|section| {
            (
                Some(section.to_string()),
                "Cost of sales ($/oz)".to_string(),
            )
        })
        .collect();
    let mut sources = Vec::new();
    for (i, release) in ["2030Q2", "2031Q2", "2032Q2"].into_iter().enumerate() {
        let section = if i == 0 && first_uses_old_section {
            "Costs per ounce ($/oz)"
        } else {
            "Costs per ounce"
        };
        let sheet = MinimalSheet {
            name: "Minestats1".into(),
            hidden: false,
            cells: vec![
                ("B3".into(), MinimalCell::Text("Site A (50%)".into())),
                (
                    "A4".into(),
                    MinimalCell::Text("For the three months ended June 30,".into()),
                ),
                ("B4".into(), MinimalCell::Number(2030.0)),
                ("A5".into(), MinimalCell::Text(section.into())),
                (
                    "A6".into(),
                    MinimalCell::Text("Cost of sales ($/oz)".into()),
                ),
                ("B6".into(), MinimalCell::Number([100.0, 200.0, 300.0][i])),
                (
                    "A8".into(),
                    MinimalCell::Text(
                        "Comparative figures\n  reflect the stated calculation.".into(),
                    ),
                ),
            ],
        };
        sources.push(mine_source(
            &format!("disclosure-{i}.xlsx"),
            release,
            vec![sheet],
        ));
    }
    profile.sources = sources
        .iter()
        .map(|s| RegisteredSource {
            file_name: s.file_name.clone(),
            kind: s.kind,
            quarter: s.quarter.clone(),
            url: s.url.clone(),
            url_basis: s.url_basis.clone(),
            bytes: s.bytes,
            sha256_hex: Digest32(s.workbook.sha256).to_hex(),
        })
        .collect();
    profile.restatement_notes.push(RestatementNoteRow {
        id: "SYNTHETIC-NOTE".into(),
        file_name: "disclosure-2.xlsx".into(),
        locator: "Minestats1!A8".into(),
        quoted_text: "Comparative figures reflect the stated calculation.".into(),
        from_quarter: "2032Q2".into(),
        evidence: "SYNTHETIC".into(),
        sha256: Digest32(sources[2].workbook.sha256).to_hex(),
    });
    profile.restatement_scopes.push(RestatementScopeRow {
        note: "SYNTHETIC-NOTE".into(),
        subject: "Site A".into(),
        period_from: "2030Q2".into(),
        period_to: "2030Q2".into(),
        line: "site:costs per ounce:cost of sales ($/oz)".into(),
        occurrence: 0,
    });
    let report = assess(&profile, Lane::Synthetic, sources);
    assert!(
        report.disclosures.is_complete(),
        "{}",
        report.disclosures.to_json()
    );
    report
}

#[test]
fn vintage_packets_keep_scoped_disclosures_without_promoting_explained_changes() {
    let mut report = disclosed_vintage_fixture(true);
    let packets = vintage_packets(&report);
    assert_eq!(packets.len(), 1);
    assert_eq!(packets[0].finding_count(), 2);
    let third = packets[0]
        .findings
        .iter()
        .find(|f| f.at.starts_with("disclosure-2"))
        .expect("first comparison stays unexplained across section alias");
    assert!(
        third
            .evidence_json
            .contains("\"against_first\":{\"state\":\"restated_silent\"")
    );
    assert!(
        third
            .evidence_json
            .contains("\"against_previous\":{\"state\":\"restated_disclosed\"")
    );
    assert!(third.evidence_json.contains("SYNTHETIC-NOTE"));
    let note = &packets[0].operands["disclosure-2.xlsx:Minestats1!A8"];
    assert!(note.roles.contains("registered_disclosure"));
    assert_eq!(
        note.value,
        OperandValue::Token("Comparative figures\n  reflect the stated calculation.".into())
    );
    assert!(
        note.claim.is_none(),
        "source note is not a manufactured numerical Claim"
    );
    let disclosed_only = disclosed_vintage_fixture(false);
    let retained = vintage_packets(&disclosed_only);
    assert_eq!(retained.len(), 1);
    assert_eq!(
        retained[0].finding_count(),
        1,
        "only the earlier, uncovered change remains"
    );
    assert!(retained[0].findings[0].at.starts_with("disclosure-1"));
    let third = report
        .mine_stats_vintages
        .items
        .iter_mut()
        .find(|i| i.current[0].locator.starts_with("disclosure-2"))
        .expect("third");
    third.against_previous.as_mut().expect("prior").disclosures[0].quoted_text =
        "invented note".into();
    assert_eq!(
        packets_from_report(&report)
            .expect_err("forged disclosure")
            .code,
        "FMINE-REF-208"
    );
}

#[test]
fn mine_stats_discrepancies_unknown_costs_and_blanks_remain_distinct() {
    let mut profile = synthetic::synthetic_profile();
    profile.reconstructions.clear();
    profile.region_members = vec![fmine_public::sources::RegionMemberRow {
        region: "Region North".into(),
        member: "Site A".into(),
        first: "2030Q1".into(),
        last: None,
    }];
    let report = assess(
        &profile,
        Lane::Synthetic,
        vec![mine_source(
            "mine.xlsx",
            "2030Q1",
            vec![summary(
                "QTD",
                "three",
                &[
                    ("Site A (50%)", ["100", "50", "50", "100"]),
                    ("Site B (50%)", ["100", "80", "80", "100"]),
                    ("Site C (50%)", ["", "30", "30", "100"]),
                    ("Region North", ["100", "50", "50", "500"]),
                ],
            )],
        )],
    );
    assert!(
        report
            .mine_ledger
            .instances
            .iter()
            .any(|i| i.state == State::Discrepancy)
    );
    assert!(
        report
            .mine_ledger
            .instances
            .iter()
            .any(|i| matches!(i.state, State::Unresolved(_)))
    );
    let packets = packets_from_report(&report).expect("mine packets");
    for instance in report.mine_ledger.instances.iter().filter(|i| {
        matches!(
            i.state,
            State::Discrepancy | State::Unresolved(_) | State::Refused { .. }
        )
    }) {
        let family = format!("{}:{}", instance.family.as_str(), instance.field);
        let packet = packets
            .iter()
            .find(|p| p.subject == instance.subject && p.family == family)
            .expect("every exceptional identity has a packet");
        assert!(
            packet
                .findings
                .iter()
                .any(|f| f.state == instance.state.code())
        );
        assert!(
            packet
                .operands
                .keys()
                .all(|at| at.starts_with("mine.xlsx:"))
        );
    }
    let refused = packets
        .iter()
        .find(|p| p.subject == "Site C" && p.kind == PacketKind::InputRefused)
        .expect("blank stays refused");
    assert!(refused.to_json().contains("blank_operand"));
    assert!(
        refused
            .operands
            .values()
            .any(|o| o.value == OperandValue::Blank)
    );
    let rate = packets
        .iter()
        .find(|p| p.kind == PacketKind::Undecidable)
        .expect("missing costs");
    assert!(rate.to_json().contains("cost_totals_unpublished"));
    assert!(rate.to_markdown().contains("costs in dollars"));
}

#[test]
fn joint_conflict_is_exported_even_when_every_identity_holds_alone() {
    let mut profile = synthetic::synthetic_profile();
    profile.reconstructions.clear();
    let report = assess(
        &profile,
        Lane::Synthetic,
        vec![
            mine_source(
                "first.xlsx",
                "2030Q1",
                vec![summary(
                    "QTD",
                    "three",
                    &[("Site A (50%)", ["101", "51", "40", "100"])],
                )],
            ),
            mine_source(
                "second.xlsx",
                "2030Q2",
                vec![
                    summary(
                        "QTD",
                        "three",
                        &[("Site A (50%)", ["101", "51", "40", "100"])],
                    ),
                    summary(
                        "YTD",
                        "six",
                        &[("Site A (50%)", ["201", "101", "80", "100"])],
                    ),
                ],
            ),
        ],
    );
    assert!(
        report
            .mine_ledger
            .instances
            .iter()
            .all(|i| i.state == State::Consistent)
    );
    assert_eq!(report.mine_ledger.infeasible.len(), 1);
    assert!(report.mine_ledger.infeasible[0].joint_only);
    let packets = packets_from_report(&report).expect("joint packet");
    assert_eq!(packets.len(), 1);
    let p = &packets[0];
    assert_eq!(p.kind, PacketKind::JointDiscrepancy);
    assert!(p.to_json().contains("\"joint_only\":true"));
    assert!(p.to_json().contains("\"minimum\":true"));
    assert!(p.to_json().contains("exact_component_projection"));
    assert_eq!(p.findings.iter().filter(|f| f.state == "i").count(), 5);
    assert!(p.operands.keys().any(|at| at.starts_with("first.xlsx:")));
    assert!(p.operands.keys().any(|at| at.starts_with("second.xlsx:")));
    assert!(!p.to_json().contains("\"in\":[\"2030Q"));
}

#[test]
fn markdown_escapes_source_markup_and_cannot_close_a_json_fence() {
    let report = run_synthetic_assurance(&[]).expect("synthetic assurance");
    let mut packet = known_packet(&packets_from_report(&report).expect("packets")).clone();
    packet.subject = "<script>|[claim](elsewhere)".into();
    packet.findings[0].evidence_json = "{\"token\":\"```\\nunsafe markup\"}".into();
    let md = packet.to_markdown();
    assert!(!md.contains("<script>"));
    assert!(md.contains("&lt;script&gt;\\|\\[claim\\]"));
    assert!(md.contains("\\u0060\\u0060\\u0060"));
}

#[test]
#[ignore = "requires the registered public workbook corpus"]
fn barrick_vintage_packets_match_nine_silent_changes_and_e474() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let report = fmine_public::run_barrick_m0_assurance(&root).expect("registered assurance");
    let packets = vintage_packets(&report);
    let mine_packets: Vec<_> = packets
        .iter()
        .filter(|p| p.family.starts_with("mine_stats:"))
        .collect();
    let silent: Vec<_> = mine_packets
        .iter()
        .flat_map(|p| &p.findings)
        .filter(|f| f.state == "restated_silent")
        .collect();
    assert_eq!(
        silent.len(),
        9,
        "E474 plus W1-DR-09's eight changes, each retained once"
    );
    for original in [
        "Barrick_Q1_2024_Mine_Stats.xlsx:QTD!G23",
        "Barrick_Q3_2024_Mine_Stats.xlsx:Minestats3!F57",
        "Barrick_Q2_2025_Mine_Stats.xlsx:Minestats1!F107",
        "Barrick_Q2_2025_Mine_Stats.xlsx:Minestats1!F108",
        "Barrick_Q2_2024_Mine_Stats.xlsx:Minestats2!F14",
        "Barrick_Q2_2024_Mine_Stats.xlsx:Minestats2!F27",
        "Barrick_Q2_2024_Mine_Stats.xlsx:Minestats2!F63",
        "Barrick_Q2_2024_Mine_Stats.xlsx:Minestats2!F76",
        "Barrick_Q2_2025_Mine_Stats.xlsx:Minestats3!B8",
    ] {
        assert_eq!(
            silent
                .iter()
                .filter(|f| f.evidence_json.contains(original))
                .count(),
            1,
            "one packet finding for independently recorded source {original}"
        );
    }
    assert_eq!(
        mine_packets
            .iter()
            .filter(|p| p.kind == PacketKind::VintageConflict)
            .map(|p| p.finding_count())
            .sum::<usize>(),
        60
    );
    let e474 = mine_packets
        .iter()
        .find(|p| {
            p.findings
                .iter()
                .any(|f| f.at == "Barrick_Q1_2025_Mine_Stats.xlsx:QTD!N22")
        })
        .expect("E474 packet");
    for (at, value) in [
        ("Barrick_Q1_2024_Mine_Stats.xlsx:QTD!G23", 1479),
        ("Barrick_Q1_2025_Mine_Stats.xlsx:QTD!N22", 1474),
    ] {
        let operand = &e474.operands[at];
        assert_eq!(operand.value, OperandValue::Int(value));
        let claim = operand.claim.as_ref().expect("source Claim");
        assert_eq!(claim, &report.public_claims.cells[at]);
        assert_eq!(operand.sha256, claim.key.reporting_vintage.file_sha256);
    }
    for packet in &packets {
        packet.validate().expect("complete source-cited packet");
        assert!(!packet.external_use_allowed());
        assert!(packet.findings.iter().all(|f| f.state != "not_comparable"));
    }
    let envelope = crate::challenge_envelope(&report).expect("end-to-end challenge envelope");
    let json = envelope.to_json();
    assert!(json.contains("\"schema\":\"fmine.challenge_packets.v1\""));
    assert!(json.contains(&e474.id().to_string()));
    assert!(json.contains("\"human_check\":null,\"external_use_allowed\":false"));
    assert!(!json.contains("\"external_use_allowed\":true"));
    assert!(
        envelope
            .rendered_markdown
            .as_ref()
            .expect("readable packets")
            .contains("Vintage comparisons and registered disclosures")
    );
    println!(
        "E474/W1: 9 silent changes and 60 ownership-conflict findings retain canonical source Claims; JSON/Markdown packet envelope checked"
    );
}

#[test]
#[ignore = "requires the registered public workbook corpus"]
fn barrick_footing_packets_keep_both_vintages_and_known_values() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let report = fmine_public::run_barrick_m0_assurance(&root).expect("registered assurance");
    let packets = packets_from_report(&report).expect("public packets");
    let expected_transitions = report
        .issuer_consistency
        .histories
        .iter()
        .flat_map(|h| &h.transitions)
        .filter(|t| {
            !matches!(
                t.kind,
                TransitionKind::Persisted | TransitionKind::SingleVintage
            )
        })
        .count();
    assert_eq!(
        packets
            .iter()
            .filter(|p| p.kind == PacketKind::IdentityTransition)
            .map(ChallengePacket::finding_count)
            .sum::<usize>(),
        expected_transitions,
        "every exceptional AISC transition is retained exactly once"
    );
    println!("AISC identity transition packet coverage: {expected_transitions}");
    for (subject, family, total, printed, residual) in [
        ("Kibali", "tcc_footing", "O11", 70, -2),
        ("Porgera", "aisc_footing", "L18", 13, 1),
    ] {
        let p = packets
            .iter()
            .find(|p| {
                p.subject == subject
                    && p.family == family
                    && p.kind == PacketKind::FootingDiscrepancy
            })
            .expect("known footing");
        assert_eq!(p.findings.len(), 2);
        for file in ["Barrick_Q3_2024_AISC.xlsx", "Barrick_Q2_2025_AISC.xlsx"] {
            let at = format!("{file}:Q2 2024!{total}");
            assert_eq!(p.operands[&at].value, OperandValue::Int(printed));
            let source = report
                .sources
                .iter()
                .find(|s| s.file_name == file)
                .expect("registered source");
            assert_eq!(
                p.operands[&at].sha256,
                Digest32(source.workbook.sha256).to_hex()
            );
        }
        assert!(
            p.to_json()
                .contains(&format!("\"residual\":{{\"v\":{residual},"))
        );
        assert!(!p.external_use_allowed());
    }
}
