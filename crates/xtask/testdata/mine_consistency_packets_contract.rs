//! Cross-crate Mine Statistics transition packet contracts; all source workbooks are invented.

use super::*;
use fmine_io::xlsx::{MinimalCell, MinimalSheet, Workbook, build_minimal_xlsx};
use fmine_public::{LoadedSource, assess as assure, synthetic};

fn fixture(values: &[(Option<f64>, f64)], duplicate_first: bool) -> PublicAssuranceReport {
    let mut profile = synthetic::synthetic_profile();
    profile.reconstructions.clear();
    let sources = values
        .iter()
        .enumerate()
        .map(|(index, (full, attributable))| {
            let (full_col, attr_col) = if index == 0 { ("B", "C") } else { ("I", "J") };
            let mut cells = vec![
                (
                    "A3".into(),
                    MinimalCell::Text("For the twelve months ended December 31,".into()),
                ),
                (format!("{full_col}3"), MinimalCell::Number(2030.0)),
                (
                    format!("{full_col}4"),
                    MinimalCell::Text("Gold produced (000s oz 100%)".into()),
                ),
                (
                    format!("{attr_col}4"),
                    MinimalCell::Text("Gold produced (000s oz attributable)".into()),
                ),
                ("A5".into(), MinimalCell::Text("Site A (50%)".into())),
                (format!("{attr_col}5"), MinimalCell::Number(*attributable)),
            ];
            if let Some(full) = full {
                cells.push((format!("{full_col}5"), MinimalCell::Number(*full)));
            }
            if index == 0 && duplicate_first {
                cells.extend([
                    ("A6".into(), MinimalCell::Text("Site A (50%)".into())),
                    (format!("{full_col}6"), MinimalCell::Number(100.0)),
                    (format!("{attr_col}6"), MinimalCell::Number(35.0)),
                ]);
            }
            let file_name = format!("mine-transition-{index}.xlsx");
            let bytes = build_minimal_xlsx(&[MinimalSheet {
                name: "YTD".into(),
                hidden: false,
                cells,
            }]);
            let workbook = Workbook::from_bytes(&file_name, &bytes).unwrap();
            LoadedSource {
                file_name,
                kind: SourceKind::MineStats,
                // One original FY block followed by two comparative appearances in the next year.
                quarter: ["2030Q4", "2031Q1", "2031Q2"][index].into(),
                url: "synthetic".into(),
                url_basis: "synthetic".into(),
                bytes: bytes.len() as u64,
                census: workbook.cell_census(),
                workbook,
            }
        })
        .collect();
    assure(&profile, Lane::Synthetic, sources)
}

fn export(report: &PublicAssuranceReport) -> Result<Vec<ChallengePacket>, Refusal> {
    packets_from_report(report).map(|packets| {
        packets
            .into_iter()
            .filter(|p| {
                p.kind == PacketKind::MineStatsIdentityTransition
                    && p.family == "mine_stats_identity:ownership"
            })
            .collect()
    })
}

#[test]
fn corrected_and_broken_mine_identities_keep_both_original_states_and_sources() {
    let report = fixture(
        &[
            (Some(100.0), 40.0),
            (Some(100.0), 50.0),
            (Some(100.0), 60.0),
        ],
        false,
    );
    let before = report.to_json();
    let packets = export(&report).unwrap();
    assert_eq!(packets.len(), 1);
    let packet = &packets[0];
    assert_eq!(packet.family, "mine_stats_identity:ownership");
    assert_eq!(packet.findings.len(), 2);
    assert_eq!(
        packet
            .findings
            .iter()
            .map(|f| f.state.as_str())
            .collect::<Vec<_>>(),
        ["corrected", "broken"]
    );
    for source in &report.sources {
        assert!(
            packet
                .to_json()
                .contains(&Digest32(source.workbook.sha256).to_hex())
        );
    }
    for (file, full, attr) in [(0, "B5", "C5"), (1, "I5", "J5"), (2, "I5", "J5")] {
        for cell in [full, attr] {
            let at = format!("mine-transition-{file}.xlsx:YTD!{cell}");
            assert_eq!(
                packet.operands[&at].claim.as_ref(),
                report.public_claims.cells.get(&at)
            );
        }
        assert!(
            packet
                .operands
                .contains_key(&format!("mine-transition-{file}.xlsx:YTD!A5"))
        );
    }
    assert!(
        packet.findings[0]
            .evidence_json
            .contains("\"state\":\"iii\"")
    );
    assert!(packet.findings[0].evidence_json.contains("\"state\":\"i\""));
    assert!(
        packet.findings[0]
            .evidence_json
            .contains("\"group\":\"prior\"")
    );
    assert!(
        packet
            .to_markdown()
            .contains("## Mine Statistics identity transitions")
    );
    assert!(
        !packet
            .to_markdown()
            .contains("## AISC identity transitions")
    );
    assert!(
        packet
            .to_json()
            .contains("\"disclosure_classification\":\"not_assigned_to_identity_transition\"")
    );
    packet.validate().unwrap();
    assert_eq!(before, report.to_json());
}

#[test]
fn persisted_and_single_vintage_mine_identities_do_not_create_transition_packets() {
    assert!(
        export(&fixture(&[(Some(100.0), 50.0); 3], false))
            .unwrap()
            .is_empty()
    );
    assert!(
        export(&fixture(&[(Some(100.0), 40.0)], false))
            .unwrap()
            .is_empty()
    );
}

#[test]
fn recorded_missing_inputs_stay_refused_inside_the_history() {
    let report = fixture(&[(Some(100.0), 40.0), (None, 50.0)], false);
    let packets = export(&report).unwrap();
    let packet = &packets[0];
    assert_eq!(packet.findings[0].state, "reclassified");
    assert!(packet.findings[0].evidence_json.contains("\"state\":\"v\""));
    assert!(packet.findings[0].evidence_json.contains("blank_operand"));
    assert_eq!(
        packet.operands["mine-transition-1.xlsx:YTD!I5"].value,
        OperandValue::Blank
    );
}

#[test]
fn forged_transition_population_and_claim_bindings_are_rejected() {
    let report = fixture(&[(Some(100.0), 40.0), (Some(100.0), 50.0)], false);
    for mutation in 0..8 {
        let mut changed = report.clone();
        let projection = &mut changed.mine_stats_consistency;
        match mutation {
            0 => projection.identities.histories[0].transitions[0].kind = TransitionKind::Persisted,
            1 => projection.identities.histories[0].transitions[0].from = usize::MAX,
            2 => projection.identities.histories[0].transitions.clear(),
            3 => projection.identities.first_registered.clear(),
            4 => {
                projection.bindings[0].instance.nil_token =
                    !projection.bindings[0].instance.nil_token
            }
            5 => projection.source_claims.clear(),
            6 => {
                projection
                    .subject_labels
                    .insert("Unused registered label".into(), "Site A".into());
            }
            _ => {
                changed
                    .mine_stats_claims
                    .subject_labels
                    .insert("Unused registered label".into(), "Site A".into());
            }
        }
        assert!(export(&changed).is_err(), "mutation {mutation}");
    }
}

#[test]
fn packets_authenticate_source_values_and_are_order_independent() {
    let report = fixture(
        &[
            (Some(100.0), 40.0),
            (Some(100.0), 50.0),
            (Some(100.0), 60.0),
        ],
        false,
    );
    let expected = export(&report)
        .unwrap()
        .iter()
        .map(ChallengePacket::to_json)
        .collect::<Vec<_>>();
    let mut reordered = report.clone();
    reordered.sources.reverse();
    reordered.mine_stats_claims.claims.reverse();
    reordered
        .mine_stats_consistency
        .identities
        .histories
        .reverse();
    reordered.mine_stats_consistency.bindings.reverse();
    reordered.mine_stats_consistency.source_claims.reverse();
    for history in &mut reordered.mine_stats_consistency.identities.histories {
        history.transitions.reverse();
    }
    assert_eq!(
        expected,
        export(&reordered)
            .unwrap()
            .iter()
            .map(ChallengePacket::to_json)
            .collect::<Vec<_>>()
    );
    let mut forged = report.clone();
    forged
        .public_claims
        .cells
        .get_mut("mine-transition-1.xlsx:YTD!J5")
        .unwrap()
        .claim
        .value = PublicValue::Number {
        stored: "999".into(),
        displayed: Ok("999".into()),
        under_status: None,
    };
    assert!(export(&forged).is_err());
    let mut absent = report;
    absent.sources.remove(1);
    assert!(export(&absent).is_err());
}

#[test]
fn ambiguous_earliest_source_rows_are_all_retained_without_invented_identity_states() {
    let report = fixture(&[(Some(100.0), 40.0), (Some(100.0), 50.0)], true);
    let packets = export(&report).unwrap();
    let packet = &packets[0];
    let finding = &packet.findings[0].evidence_json;
    assert!(finding.contains("\"first_registered\":null"));
    for cell in ["C5", "C6"] {
        assert!(
            packet
                .operands
                .contains_key(&format!("mine-transition-0.xlsx:YTD!{cell}"))
        );
    }
    assert!(finding.contains("\"identity\":null"));
    assert!(finding.contains("no separate original identity in the ledger"));
    assert_eq!(packet.findings[0].state, "reclassified");
}

#[test]
fn an_unbound_earlier_ambiguous_release_does_not_promote_a_later_appearance() {
    let mut report = fixture(
        &[
            (Some(100.0), 40.0),
            (Some(100.0), 50.0),
            (Some(100.0), 60.0),
        ],
        false,
    );
    // The original quarter alias now names two authentic source files. Its ledger identity cannot
    // bind uniquely, while the complete source population retains both first-release assertions.
    let mut copy = report.sources[0].clone();
    copy.file_name = "mine-transition-original-copy.xlsx".into();
    copy.workbook.artifact_path = copy.file_name.clone();
    report
        .mine_stats
        .push((copy.file_name.clone(), report.mine_stats[0].1.clone()));
    report.sources.push(copy);
    report.mine_stats_claims = fmine_public::key::mine_stats::build_claims(
        &synthetic::synthetic_profile(),
        &report.sources,
        &report.mine_stats,
    );
    for source in &report.mine_stats_claims.claims {
        report
            .public_claims
            .cells
            .insert(source.locator.clone(), source.clone());
    }
    report.mine_stats_consistency = assess(&report.mine_stats_claims, &report.mine_ledger);
    let history = report
        .mine_stats_consistency
        .histories
        .iter()
        .find(|h| h.family == mine_ledger::Family::Ownership)
        .unwrap();
    assert_eq!(history.sequence[0].release, "2031Q1");
    assert!(history.ambiguous_vintages.contains(&"2030Q4".into()));
    // Exercise this export boundary directly: the separate raw-alias discrepancy path correctly
    // refuses a duplicate quarter alias rather than choosing one of these files.
    let mut groups = BTreeMap::new();
    add_packets(&mut groups, &report).unwrap();
    let packet = groups
        .values()
        .find(|p| p.family == "mine_stats_identity:ownership")
        .unwrap();
    let finding = &packet.findings[0].evidence_json;
    assert!(finding.contains("\"first_registered\":null"));
    assert!(finding.contains("\"earliest_registered_release\":\"2030Q4\""));
    assert!(finding.contains("\"earliest_registered_assertions\":[]"));
    for file in [
        "mine-transition-0.xlsx",
        "mine-transition-original-copy.xlsx",
    ] {
        assert!(packet.operands.contains_key(&format!("{file}:YTD!C5")));
    }
}

#[test]
fn intervening_ambiguous_sources_prevent_correction_and_remain_cited() {
    let mut report = fixture(
        &[
            (Some(100.0), 40.0),
            (Some(100.0), 45.0),
            (Some(100.0), 50.0),
        ],
        false,
    );
    // Two authentic middle-release files prevent binding that identity. Comparing only the
    // remaining endpoints would wrongly describe the discrepancy as corrected.
    let mut copy = report.sources[1].clone();
    copy.file_name = "mine-transition-middle-copy.xlsx".into();
    copy.workbook.artifact_path = copy.file_name.clone();
    report
        .mine_stats
        .push((copy.file_name.clone(), report.mine_stats[1].1.clone()));
    report.sources.push(copy);
    report.mine_stats_claims = fmine_public::key::mine_stats::build_claims(
        &synthetic::synthetic_profile(),
        &report.sources,
        &report.mine_stats,
    );
    for source in &report.mine_stats_claims.claims {
        report
            .public_claims
            .cells
            .insert(source.locator.clone(), source.clone());
    }
    report.mine_stats_consistency = assess(&report.mine_stats_claims, &report.mine_ledger);
    let history = report
        .mine_stats_consistency
        .histories
        .iter()
        .find(|h| h.family == mine_ledger::Family::Ownership)
        .unwrap();
    assert_eq!(
        history
            .sequence
            .iter()
            .map(|appearance| appearance.release.as_str())
            .collect::<Vec<_>>(),
        ["2030Q4", "2031Q2"]
    );
    assert_eq!(history.transitions[0].kind, TransitionKind::Reclassified);
    // The generic raw-alias discrepancy export independently refuses duplicate release aliases.
    let mut groups = BTreeMap::new();
    add_packets(&mut groups, &report).unwrap();
    let packet = groups
        .values()
        .find(|p| p.family == "mine_stats_identity:ownership")
        .unwrap();
    let finding = &packet.findings[0].evidence_json;
    assert!(finding.contains("\"intervening_ambiguous_releases\":[{\"release\":\"2031Q1\""));
    assert!(finding.contains("\"previous_bound_identity\":{\"release\":\"2030Q4\""));
    assert!(finding.contains("\"identity\":null"));
    assert!(!packet.to_json().contains("immediately_previous"));
    for file in ["mine-transition-1.xlsx", "mine-transition-middle-copy.xlsx"] {
        let operand = &packet.operands[&format!("{file}:YTD!J5")];
        assert!(
            operand
                .roles
                .contains("intervening_ambiguous_release:coexisting_assertion")
        );
        assert!(operand.claim.is_some());
    }
    assert!(packet.closing_documents.iter().any(|document| {
        document
            .would_show
            .contains("resolve the coexisting assertions")
    }));
    report.sources.pop();
    assert!(add_packets(&mut BTreeMap::new(), &report).is_err());
}
