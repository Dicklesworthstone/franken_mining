#![cfg(test)]

//! BD-504e: invented XLSX bytes through the real Mine Stats reader, identity ledger and vintage projection.
//! These known answers establish synthetic behavior, not a census of the issuer's registered corpus.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use fmine_core::hash::Digest32;
use fmine_io::xlsx::{
    MinimalCell, MinimalSheet, MinimalStyles, Workbook, build_minimal_xlsx_styled,
    col_index_to_letters,
};
use fmine_public::consistency::TransitionKind;
use fmine_public::consistency::mine_stats::{MineStatsConsistency, assess as consistency};
use fmine_public::mine_ledger::{Family, Group};
use fmine_public::sources::{IssuerProfile, RegisteredShare, SourceKind};
use fmine_public::{Lane, LoadedSource, PublicAssuranceReport, assess, synthetic};

fn sheet(
    name: &str,
    full: &str,
    attr: &str,
    share: &str,
    shift: u32,
    hidden: bool,
) -> MinimalSheet {
    let first = col_index_to_letters(2 + shift);
    let second = col_index_to_letters(3 + shift);
    let text = |s: &str| MinimalCell::Text(s.into());
    let data = |s: &str| {
        if s.parse::<f64>().is_ok() {
            MinimalCell::Styled {
                raw: s.into(),
                xf: 0,
            }
        } else {
            text(s)
        }
    };
    MinimalSheet {
        name: name.into(),
        hidden,
        cells: vec![
            ("A1".into(), text("Summary of Operations")),
            (
                "A3".into(),
                text(if name == "YTD" {
                    "For the twelve months ended December 31,"
                } else {
                    "For the three months ended March 31,"
                }),
            ),
            ("B3".into(), MinimalCell::Number(2024.0)),
            (format!("{first}4"), text("Gold produced (000s oz 100%)")),
            (
                format!("{second}4"),
                text("Gold produced (000s oz attributable)"),
            ),
            ("A5".into(), text(&format!("Site A ({share}%)"))),
            (format!("{first}5"), data(full)),
            (format!("{second}5"), data(attr)),
        ],
    }
}

fn source(file: &str, release: &str, sheet: MinimalSheet) -> LoadedSource {
    // Built-in format 1 displays integers, preserving a testable difference between stored and shown values.
    let styles = MinimalStyles {
        num_fmts: vec![],
        xfs: vec![1],
    };
    let bytes = build_minimal_xlsx_styled(&[sheet], Some(&styles));
    let workbook = Workbook::from_bytes(file, &bytes).expect("invented workbook parses");
    LoadedSource {
        file_name: file.into(),
        kind: SourceKind::MineStats,
        quarter: release.into(),
        url: "https://example.invalid/invented.xlsx".into(),
        url_basis: "synthetic".into(),
        bytes: bytes.len() as u64,
        census: workbook.cell_census(),
        workbook,
    }
}

fn run(sources: Vec<LoadedSource>) -> (PublicAssuranceReport, MineStatsConsistency) {
    let mut profile = synthetic::synthetic_profile();
    profile.subjects = vec!["Site A".into()];
    profile.reconstructions.clear();
    profile.shares.clear();
    profile.regions.clear();
    profile.region_members.clear();
    profile.subject_statuses.clear();
    assessed(&profile, sources)
}

fn assessed(
    profile: &IssuerProfile,
    sources: Vec<LoadedSource>,
) -> (PublicAssuranceReport, MineStatsConsistency) {
    let report = assess(profile, Lane::Synthetic, sources);
    for (_, read) in &report.mine_stats {
        assert!(read.refusals.is_empty(), "{:?}", read.refusals);
    }
    assert!(
        report.mine_stats_claims.refusals.is_empty(),
        "{:?}",
        report.mine_stats_claims.refusals
    );
    let projection = consistency(&report.mine_stats_claims, &report.mine_ledger);
    (report, projection)
}

fn pair(first: &str, later: &str) -> (PublicAssuranceReport, MineStatsConsistency) {
    run(vec![
        source(
            "first.xlsx",
            "2024Q1",
            sheet("QTD", "100", first, "50", 0, false),
        ),
        source(
            "later.xlsx",
            "2025Q1",
            sheet("QTD", "100", later, "50", 2, false),
        ),
    ])
}

fn alias_pair(
    label: &str,
    shares: Vec<RegisteredShare>,
) -> (PublicAssuranceReport, MineStatsConsistency) {
    let mut profile = synthetic::synthetic_profile();
    profile.subjects = vec!["Nevada Gold Mines".into()];
    profile.shares = shares;
    profile.reconstructions.clear();
    profile.regions.clear();
    profile.region_members.clear();
    profile.subject_statuses.clear();
    let sources = [("first.xlsx", "2024Q1", 0), ("later.xlsx", "2025Q1", 2)]
        .into_iter()
        .map(|(file, release, shift)| {
            let mut sheet = sheet("QTD", "100", "50", "50", shift, false);
            let (_, cell) = sheet.cells.iter_mut().find(|(at, _)| at == "A5").unwrap();
            *cell = MinimalCell::Text(format!("{label} (50%)"));
            source(file, release, sheet)
        })
        .collect();
    assessed(&profile, sources)
}

#[test]
fn registered_mine_subject_labels_bind_canonical_claims_without_rewriting_instances() {
    // Invented values exercise the real-corpus label mismatch; these are not issuer measurements.
    let registered = RegisteredShare {
        subject: "Nevada Gold Mines".into(),
        label: "Nevada Gold Mines LLC".into(),
        num: 1,
        den: 2,
        evidence: "synthetic registered label".into(),
    };
    let (report, projection) = alias_pair("Nevada Gold Mines LLC", vec![registered.clone()]);
    assert_eq!(
        report
            .mine_stats_claims
            .subject_labels
            .get("Nevada Gold Mines LLC")
            .map(String::as_str),
        Some("Nevada Gold Mines")
    );
    assert!(projection.unkeyed.is_empty());
    assert_eq!(projection.first_registered[&(Family::Ownership, "i")], 1);
    assert_eq!(projection.all_files[&(Family::Ownership, "i")], 2);
    assert_eq!(projection.histories.len(), 1);
    let history = &projection.histories[0];
    assert_eq!(history.series.subject, "Nevada Gold Mines");
    assert_eq!(history.sequence.len(), 2);
    assert_eq!(history.transitions[0].kind, TransitionKind::Persisted);
    assert_eq!(history.transitions[0].operands_changed, Some(false));
    assert_eq!(projection.bindings.len(), 2);
    for binding in &projection.bindings {
        assert_eq!(binding.instance.subject, "Nevada Gold Mines LLC");
        assert_eq!(binding.source.key.subject, "Nevada Gold Mines");
        assert!(report.mine_ledger.instances.contains(&binding.instance));
        assert!(report.mine_stats_claims.claims.contains(&binding.source));
        assert!(binding.source.claim.check_id().is_ok());
    }
    // The digest commits to the exact registered labels even when an added label changes no used Claim.
    let unused = RegisteredShare {
        subject: "Site A".into(),
        label: "Site A LLC".into(),
        num: 1,
        den: 2,
        evidence: "synthetic additional label".into(),
    };
    let (expanded_report, expanded) = alias_pair("Nevada Gold Mines LLC", vec![registered, unused]);
    assert_eq!(
        report.mine_stats_claims.claims,
        expanded_report.mine_stats_claims.claims
    );
    assert_eq!(projection.histories, expanded.histories);
    assert_ne!(projection.digest(), expanded.digest());
}

#[test]
fn unregistered_mine_subject_mismatches_remain_unkeyed() {
    let (raw, _) = alias_pair("Nevada Gold Mines LLC", vec![]);
    let (canonical, _) = alias_pair("Nevada Gold Mines", vec![]);
    assert!(canonical.mine_stats_claims.subject_labels.is_empty());
    assert!(
        canonical
            .mine_stats_claims
            .claims
            .iter()
            .all(|c| c.claim.check_id().is_ok())
    );
    // Both populations came through the actual XLSX/Claim adapter. Their raw anchor, period and field
    // agree, but the caller has supplied no registered authorization to equate the different subjects.
    let projection = consistency(&canonical.mine_stats_claims, &raw.mine_ledger);
    assert_eq!(projection.all_files[&(Family::Ownership, "i")], 2);
    assert_eq!(projection.unkeyed.len(), 2);
    assert!(projection.first_registered.is_empty());
    assert!(projection.histories.is_empty());
    assert!(projection.bindings.is_empty());
    for row in &projection.unkeyed {
        assert_eq!(row.family, Family::Ownership);
        assert!(
            row.reason
                .contains("canonical Claim with the same subject, period and field")
        );
        assert!(row.reason.contains(".xlsx:QTD!"));
    }
}

#[test]
fn first_registered_mine_counts_merge_original_and_shifted_comparative_columns() {
    let (report, projection) = pair("49", "50");
    assert_eq!(report.mine_ledger.instances.len(), 2);
    assert!(projection.unkeyed.is_empty());
    assert_eq!(projection.histories.len(), 1);
    assert_eq!(projection.first_registered[&(Family::Ownership, "iii")], 1);
    assert!(
        !projection
            .first_registered
            .contains_key(&(Family::Ownership, "i"))
    );
    assert_eq!(projection.all_files[&(Family::Ownership, "iii")], 1);
    assert_eq!(projection.all_files[&(Family::Ownership, "i")], 1);
    let history = &projection.histories[0];
    assert_eq!(history.series.valid_period.code(), "2024Q1");
    assert_eq!(
        history
            .sequence
            .iter()
            .map(|a| (a.release.as_str(), a.state))
            .collect::<Vec<_>>(),
        [("2024Q1", "iii"), ("2025Q1", "i")]
    );
    assert_eq!(history.sequence[0].at, "first.xlsx:QTD!C5");
    assert_eq!(history.sequence[1].at, "later.xlsx:QTD!E5");
    assert_eq!(history.transitions[0].kind, TransitionKind::Corrected);
    assert_eq!(history.transitions[0].operands_changed, Some(true));
    assert!(
        projection
            .bindings
            .iter()
            .any(|b| b.instance.group == Group::Current)
    );
    assert!(
        projection
            .bindings
            .iter()
            .any(|b| b.instance.group == Group::Prior)
    );
    let json = projection.to_json();
    for claim in &report.mine_stats_claims.claims {
        assert!(
            json.contains(&claim.claim.claim_id.to_string()),
            "missing cited input {}",
            claim.locator
        );
    }
    for source in &report.sources {
        assert!(json.contains(&Digest32(source.workbook.sha256).to_hex()));
    }
}

#[test]
fn mine_state_transition_matrix_uses_operand_changes_and_retains_first_vintage() {
    for (first, later, kind) in [
        ("49", "49", TransitionKind::Persisted),
        ("49", "50", TransitionKind::Corrected),
        ("50", "49", TransitionKind::Broken),
        ("49", "48", TransitionKind::Reclassified),
    ] {
        let (_, projection) = pair(first, later);
        let history = &projection.histories[0];
        assert_eq!(history.transitions[0].kind, kind, "{first} -> {later}");
        assert_eq!(
            history.transitions[0].operands_changed,
            Some(first != later)
        );
        assert_eq!(history.sequence[0].at, "first.xlsx:QTD!C5");
    }
    let (_, unchanged_display) = run(vec![
        source(
            "first.xlsx",
            "2024Q1",
            sheet("QTD", "100.001", "50", "50", 0, false),
        ),
        source(
            "later.xlsx",
            "2025Q1",
            sheet("QTD", "100.002", "50", "50", 2, false),
        ),
    ]);
    assert_eq!(
        unchanged_display.histories[0].transitions[0].kind,
        TransitionKind::Persisted
    );
    assert_eq!(
        unchanged_display.histories[0].transitions[0].operands_changed,
        Some(false)
    );
}

#[test]
fn adjacent_mine_transitions_and_projection_digest_are_order_invariant() {
    let sources = vec![
        source(
            "first.xlsx",
            "2024Q4",
            sheet("YTD", "100", "49", "50", 0, false),
        ),
        source(
            "middle.xlsx",
            "2025Q1",
            sheet("YTD", "100", "50", "50", 2, false),
        ),
        source(
            "last.xlsx",
            "2025Q2",
            sheet("YTD", "100", "48", "50", 4, false),
        ),
    ];
    let (_, expected) = run(sources.clone());
    let history = expected
        .histories
        .iter()
        .find(|h| h.family == Family::Ownership)
        .unwrap();
    assert_eq!(
        history.sequence.iter().map(|a| a.state).collect::<Vec<_>>(),
        ["iii", "i", "iii"]
    );
    assert_eq!(
        history
            .transitions
            .iter()
            .map(|t| t.kind)
            .collect::<Vec<_>>(),
        [TransitionKind::Corrected, TransitionKind::Broken]
    );
    assert_eq!(
        (history.transitions[0].from, history.transitions[0].to),
        (0, 1)
    );
    assert_eq!(
        (history.transitions[1].from, history.transitions[1].to),
        (1, 2)
    );
    for order in [[2, 0, 1], [1, 2, 0], [2, 1, 0]] {
        let (mut report, actual) = run(order.into_iter().map(|i| sources[i].clone()).collect());
        assert_eq!(actual.to_json(), expected.to_json());
        report.mine_ledger.instances.reverse();
        report.mine_stats_claims.claims.reverse();
        assert_eq!(
            consistency(&report.mine_stats_claims, &report.mine_ledger).to_json(),
            expected.to_json()
        );
    }
}

#[test]
fn hidden_mine_sheets_are_named_but_excluded_from_both_denominators() {
    let (report, projection) = run(vec![
        source(
            "hidden.xlsx",
            "2024Q1",
            sheet("QTD", "100", "49", "50", 0, true),
        ),
        source(
            "visible.xlsx",
            "2025Q1",
            sheet("QTD", "100", "50", "50", 2, false),
        ),
    ]);
    assert!(
        report
            .mine_stats
            .iter()
            .any(|(file, read)| file == "hidden.xlsx" && read.hidden_sheets == ["QTD"])
    );
    assert_eq!(projection.first_registered.values().sum::<usize>(), 1);
    assert_eq!(projection.all_files.values().sum::<usize>(), 1);
    assert_eq!(projection.histories[0].sequence[0].release, "2025Q1");
    assert_eq!(
        projection.histories[0].transitions[0].kind,
        TransitionKind::SingleVintage
    );
}

#[test]
fn mine_not_published_rows_stay_in_family_denominators() {
    let (_, projection) = run(vec![
        source(
            "nil-first.xlsx",
            "2024Q1",
            sheet("QTD", "—", "—", "50", 0, false),
        ),
        source(
            "nil-later.xlsx",
            "2025Q1",
            sheet("QTD", "—", "—", "50", 2, false),
        ),
    ]);
    assert_eq!(projection.first_registered[&(Family::Ownership, "np")], 1);
    assert_eq!(projection.all_files[&(Family::Ownership, "np")], 2);
    assert_eq!(
        projection.histories[0].transitions[0].kind,
        TransitionKind::Persisted
    );
    assert!(
        projection
            .to_json()
            .contains("including not-published (np)")
    );
}

#[test]
fn missing_mine_claims_stay_named_and_cannot_prove_a_correction() {
    let (mut report, _) = pair("49", "50");
    report
        .mine_stats_claims
        .claims
        .retain(|c| c.locator != "first.xlsx:QTD!B5");
    let missing_operand = consistency(&report.mine_stats_claims, &report.mine_ledger);
    assert_eq!(missing_operand.all_files.values().sum::<usize>(), 2);
    assert_eq!(
        missing_operand.histories[0].transitions[0].kind,
        TransitionKind::Reclassified
    );
    assert_eq!(
        missing_operand.histories[0].transitions[0].operands_changed,
        None
    );
    report
        .mine_stats_claims
        .claims
        .retain(|c| c.locator != "first.xlsx:QTD!C5");
    let missing_anchor = consistency(&report.mine_stats_claims, &report.mine_ledger);
    assert_eq!(missing_anchor.all_files.values().sum::<usize>(), 2);
    assert_eq!(missing_anchor.unkeyed.len(), 1);
    assert!(missing_anchor.to_json().contains("2024Q1:QTD!C5"));
}

#[test]
fn ambiguous_same_release_mine_sources_never_choose_a_filename() {
    let (_, projection) = run(vec![
        source(
            "one.xlsx",
            "2024Q1",
            sheet("QTD", "100", "49", "50", 0, false),
        ),
        source(
            "two.xlsx",
            "2024Q1",
            sheet("QTD", "100", "50", "50", 0, false),
        ),
        source(
            "later.xlsx",
            "2025Q1",
            sheet("QTD", "100", "50", "50", 2, false),
        ),
    ]);
    assert!(!projection.unkeyed.is_empty());
    assert!(projection.first_registered.is_empty());
    let json = projection.to_json();
    assert!(json.contains("\"first_registered\":null"));
    assert!(json.contains("one.xlsx:QTD!C5"));
    assert!(json.contains("two.xlsx:QTD!C5"));
    assert!(
        projection
            .histories
            .iter()
            .flat_map(|h| &h.sequence)
            .all(|a| a.release != "2024Q1")
    );
}

#[test]
fn unbound_ambiguous_release_between_bound_vintages_prevents_correction_claim() {
    let fy = |file, release, attr| {
        let mut sheet = sheet("YTD", "100", attr, "50", 0, false);
        let (_, year) = sheet.cells.iter_mut().find(|(at, _)| at == "B3").unwrap();
        *year = MinimalCell::Number(2030.0);
        source(file, release, sheet)
    };
    let (report, projection) = run(vec![
        fy("original.xlsx", "2030Q4", "49"),
        fy("ambiguous-one.xlsx", "2031Q1", "50"),
        fy("ambiguous-two.xlsx", "2031Q1", "48"),
        fy("later.xlsx", "2031Q2", "50"),
    ]);
    let history = projection
        .histories
        .iter()
        .find(|h| h.family == Family::Ownership && h.series.valid_period.code() == "2030FY")
        .expect("FY2030 ownership history");
    assert_eq!(
        history
            .sequence
            .iter()
            .map(|a| (a.release.as_str(), a.state))
            .collect::<Vec<_>>(),
        [("2030Q4", "iii"), ("2031Q2", "i")]
    );
    assert_eq!(history.ambiguous_vintages, ["2031Q1"]);
    assert_eq!(history.transitions.len(), 1);
    assert_eq!(history.transitions[0].kind, TransitionKind::Reclassified);
    assert_eq!(history.transitions[0].operands_changed, Some(true));
    assert!(
        projection
            .unkeyed
            .iter()
            .any(|u| { u.family == Family::Ownership && u.at == "2031Q1:YTD!C5" })
    );
    let intermediate = report
        .mine_stats_claims
        .claims
        .iter()
        .filter(|c| c.key.reporting_vintage.release == "2031Q1")
        .collect::<Vec<_>>();
    assert_eq!(intermediate.len(), 4);
    for claim in intermediate {
        assert!(projection.source_claims.contains(claim));
    }
    let json = projection.to_json();
    assert!(json.contains("ambiguous-one.xlsx:YTD!C5"));
    assert!(json.contains("ambiguous-two.xlsx:YTD!C5"));
}

#[test]
fn shifted_same_release_assertions_block_first_registered_selection() {
    let (_, projection) = run(vec![
        source(
            "one.xlsx",
            "2024Q1",
            sheet("QTD", "100", "49", "50", 0, false),
        ),
        source(
            "two.xlsx",
            "2024Q1",
            sheet("QTD", "100", "50", "50", 2, false),
        ),
        source(
            "later.xlsx",
            "2025Q1",
            sheet("QTD", "100", "50", "50", 4, false),
        ),
    ]);
    assert!(projection.first_registered.is_empty());
    assert_eq!(projection.histories.len(), 1);
    assert_eq!(projection.histories[0].ambiguous_vintages, ["2024Q1"]);
    assert!(
        projection.histories[0]
            .transitions
            .iter()
            .all(|t| t.kind == TransitionKind::Reclassified)
    );
    let json = projection.to_json();
    assert!(json.contains("one.xlsx:QTD!C5"));
    assert!(json.contains("two.xlsx:QTD!E5"));
}

#[test]
fn different_ownership_bases_do_not_form_a_mine_identity_transition() {
    let (_, projection) = run(vec![
        source(
            "first.xlsx",
            "2024Q1",
            sheet("QTD", "100", "50", "50", 0, false),
        ),
        source(
            "later.xlsx",
            "2025Q1",
            sheet("QTD", "100", "60", "60", 2, false),
        ),
    ]);
    assert_eq!(projection.histories.len(), 2);
    assert_eq!(projection.first_registered[&(Family::Ownership, "i")], 2);
    assert!(
        projection.histories.iter().all(
            |h| h.sequence.len() == 1 && h.transitions[0].kind == TransitionKind::SingleVintage
        )
    );
}

#[test]
fn public_report_exposes_mine_findings_without_merging_aisc_denominators() {
    use std::io::Write as _;
    use std::process::{Command, Stdio};

    let (report, projection) = pair("49", "50");
    assert_eq!(
        report.mine_stats_consistency.to_json(),
        projection.to_json()
    );
    let script = r#"
import json, sys
report = json.load(sys.stdin)
issuer = report['issuer_consistency']
mine = issuer['mine_stats_identities']
assert mine['schema'] == 'fmine.issuer_consistency.mine-stats.v1'
assert 'first publication' in mine['caveat'] and 'Hidden sheets are excluded' in mine['caveat']
assert len(mine['digest']) == len(mine['input_claims_digest']) == 64
within = mine['within_vintage']
first = within['first_registered']['ownership']
all_files = within['all_files']['ownership']
assert first['n'] == 1 and first['states']['iii']['n'] == 1
assert all_files['n'] == 2 and all_files['states']['i']['n'] == all_files['states']['iii']['n'] == 1
for family in issuer['within_vintage']['all_files'].values():
    assert family['n'] == 0
assert mine['transitions'][0]['steps'] == [{'from': 0, 'to': 1, 'kind': 'corrected', 'operands_changed': True}]
assert len(mine['bindings']) == 2 and len(mine['source_claims']) == 4
assert {b['group'] for b in mine['bindings']} == {'current', 'prior'}
assert all(b['source']['claim_id'].startswith('clm_') for b in mine['bindings'])
"#;
    let mut child = Command::new("python3")
        .args(["-I", "-c", script])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(report.to_json().as_bytes())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
