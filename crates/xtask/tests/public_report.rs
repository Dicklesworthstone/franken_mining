//! Readable M0 reports over actual synthetic extraction and assurance results.
#![cfg(test)]
#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::path::PathBuf;

use fmine_cli::packet;
use fmine_io::xlsx::{MinimalCell, MinimalSheet, Workbook, build_minimal_xlsx};
use fmine_public::aisc::{IdentityFamily, OperandValue};
use fmine_public::key::PublicValue;
use fmine_public::sources::SourceKind;
use fmine_public::{
    Lane, LoadedSource, PublicAssuranceReport, assess, run_synthetic_assurance, synthetic,
};

// Exercise the private renderer without adding a public product API for tests.
#[path = "../../fmine-cli/src/assure_report.rs"]
mod assure_report;

fn source(file: &str, release: &str, tcc: i64, raw_rate: Option<&str>, url: &str) -> LoadedSource {
    let mut cells = vec![("B2".into(), MinimalCell::Text("Site A".into()))];
    for (row, label, value) in [
        (5, "Cost of sales applicable to gold production", 100),
        (6, "Royalty adjustment", 20),
        (7, "Total cash costs", tcc),
        (8, "Minesite sustaining", 30),
        (9, "All-in sustaining costs", tcc + 30),
        (10, "Ounces sold", 10),
        (11, "Cost of sales per ounce", 10_000),
        (12, "Total cash costs per ounce", tcc * 100),
        (13, "All-in sustaining costs per ounce", (tcc + 30) * 100),
    ] {
        cells.push((format!("A{row}"), MinimalCell::Text(label.into())));
        let value = match (row, raw_rate) {
            (11, Some(raw)) => MinimalCell::Styled {
                raw: raw.into(),
                xf: 0,
            },
            _ => MinimalCell::Number(value as f64),
        };
        cells.push((format!("B{row}"), value));
    }
    let bytes = build_minimal_xlsx(&[MinimalSheet {
        name: "Q1 2030".into(),
        hidden: false,
        cells,
    }]);
    let workbook = Workbook::from_bytes(file, &bytes).expect("invented workbook");
    LoadedSource {
        file_name: file.into(),
        kind: SourceKind::AiscReconciliation,
        quarter: release.into(),
        url: url.into(),
        url_basis: "invented source fixture".into(),
        bytes: bytes.len() as u64,
        census: workbook.cell_census(),
        workbook,
    }
}

fn report(sources: Vec<LoadedSource>) -> PublicAssuranceReport {
    let mut profile = synthetic::synthetic_profile();
    profile.subjects = vec!["Site A".into()];
    profile.reconstructions.clear();
    assess(&profile, Lane::Synthetic, sources)
}

#[test]
fn subtotal_policy_is_cited_and_bound_even_when_no_numeric_state_changes() {
    use fmine_public::sources::SubtotalMemberRow;

    let input = source(
        "synthetic-policy.xlsx",
        "2030Q1",
        120,
        None,
        "synthetic:source",
    );
    let mut profile = synthetic::synthetic_profile();
    profile.subjects = ["Site A", "Site B", "Subtotal", "Other subtotal"]
        .map(str::to_string)
        .to_vec();
    profile.reconstructions.clear();
    let legacy = assess(&profile, Lane::Synthetic, vec![input.clone()]);
    profile.subtotal_members.push(SubtotalMemberRow {
        subtotal: "Subtotal".into(),
        member: "Site A".into(),
        period_from: "2030Q1".into(),
        period_to: None,
        evidence: "Synthetic membership | source note".into(),
    });
    let bound = assess(&profile, Lane::Synthetic, vec![input.clone()]);
    assert_eq!(legacy.mine_ledger.instances, bound.mine_ledger.instances);
    assert_ne!(
        legacy.ledger_anchor.state_root,
        bound.ledger_anchor.state_root
    );
    assert!(
        bound
            .to_json()
            .contains("\"subtotal_members\":[{\"subtotal\":\"Subtotal\"")
    );
    let markdown = assure_report::markdown(&bound).expect("cited report");
    assert!(markdown.contains("Subtotal membership by reported period"));
    assert!(markdown.contains("Synthetic membership \\| source note"));
    for field in 0..5 {
        let mut changed = profile.clone();
        let row = &mut changed.subtotal_members[0];
        match field {
            0 => row.subtotal = "Other subtotal".into(),
            1 => row.member = "Site B".into(),
            2 => row.period_from = "2029Q1".into(),
            3 => row.period_to = Some("2030Q4".into()),
            _ => row.evidence.push_str(" amended"),
        }
        let changed = assess(&changed, Lane::Synthetic, vec![input.clone()]);
        assert_eq!(changed.mine_ledger.instances, bound.mine_ledger.instances);
        assert_ne!(
            changed.ledger_anchor.state_root, bound.ledger_anchor.state_root,
            "field {field}"
        );
    }
    profile.subtotal_members.clear();
    assert_eq!(
        assess(&profile, Lane::Synthetic, vec![input])
            .ledger_anchor
            .state_root,
        legacy.ledger_anchor.state_root
    );
}

fn metrics_section(markdown: &str) -> &str {
    markdown
        .split("## Published values and all selected vintages")
        .nth(1)
        .unwrap()
        .split("## Issuer consistency and changes across vintages")
        .next()
        .unwrap()
}

fn table_blocks_have_headers(markdown: &str) {
    let lines: Vec<_> = markdown.lines().collect();
    let mut at = 0;
    while at < lines.len() {
        if !lines[at].starts_with('|') {
            at += 1;
            continue;
        }
        let start = at;
        while at < lines.len() && lines[at].starts_with('|') {
            at += 1;
        }
        assert!(at - start >= 2, "orphan table row: {}", lines[start]);
        assert!(
            lines[start + 1]
                .chars()
                .all(|c| matches!(c, '|' | '-' | ':' | ' ')),
            "table rows lost their heading after a paragraph: {}",
            lines[start]
        );
    }
}

#[test]
fn readable_report_retains_all_states_source_claims_and_exact_packets() {
    let report = run_synthetic_assurance(&[]).expect("synthetic assurance");
    let before = report.to_json();
    let markdown = assure_report::markdown(&report).expect("readable report");
    assert!(markdown.starts_with("# Public assurance — synthetic\n"));
    assert!(markdown.contains("**SYNTHETIC: invented workbooks"));
    assert!(markdown.contains("| Family | i | ii | iii | iv | v | np |"));
    for state in ["i", "ii", "iii", "iv", "v", "np"] {
        assert!(
            report.count_state(state) > 0,
            "fixture must exercise state {state}"
        );
    }
    for cell in report.public_claims.cells.values() {
        assert!(
            metrics_section(&markdown).contains(&cell.claim.claim_id.to_string()),
            "missing {}",
            cell.locator
        );
    }
    for evidence in packet::packets_from_report(&report).unwrap() {
        assert!(
            markdown.contains(&evidence.to_markdown()),
            "packet {} omitted or rewritten",
            evidence.id()
        );
    }
    for label in [
        "Closing evidence requested",
        "Source operands",
        "Arithmetic and retained evidence",
        "Unusable or unclassified source evidence",
        "Mystery Mine",
        "Identity state root:",
    ] {
        assert!(markdown.contains(label), "missing {label}");
    }
    table_blocks_have_headers(&markdown);
    assert!(markdown.contains(&report.ledger_anchor.state_root.to_hex()));
    assert!(markdown.contains(&report.ledger_anchor.anchor_id().to_string()));
    assert_eq!(
        before,
        report.to_json(),
        "rendering must not modify evidence"
    );
    assert_eq!(markdown, assure_report::markdown(&report).unwrap());
}

#[test]
fn original_and_comparative_values_and_corrected_identity_are_both_visible() {
    let report = report(vec![
        source(
            "Early.xlsx",
            "2030Q1",
            121,
            None,
            "https://example.invalid/early.xlsx",
        ),
        source(
            "Later.xlsx",
            "2031Q1",
            120,
            None,
            "https://example.invalid/later.xlsx",
        ),
    ]);
    let original = &report.public_claims.cells["Early.xlsx:Q1 2030!B7"];
    let comparative = &report.public_claims.cells["Later.xlsx:Q1 2030!B7"];
    assert_ne!(original.claim.claim_id, comparative.claim.claim_id);
    assert_eq!(original.key.valid_period, comparative.key.valid_period);
    let markdown = assure_report::markdown(&report).unwrap();
    let metrics = metrics_section(&markdown);
    for value in [
        "2030Q1",
        "2031Q1",
        "121",
        "120",
        "Early.xlsx:Q1 2030!B7",
        "Later.xlsx:Q1 2030!B7",
    ] {
        assert!(
            metrics.contains(value),
            "missing original/comparative evidence {value}"
        );
    }
    assert!(metrics.contains(&original.claim.claim_id.to_string()));
    assert!(metrics.contains(&comparative.claim.claim_id.to_string()));
    assert!(markdown.contains("corrected"));
    assert!(markdown.contains("Against earliest registered"));
    assert!(markdown.contains("Against immediately previous"));
    assert!(markdown.contains(fmine_public::consistency::FIRST_REGISTERED_CAVEAT));
    assert!(markdown.contains("## Cross-vintage identity accounting"));
    assert!(markdown.contains("### aisc cross-vintage ledger"));
    let cross_vintage = &report.aisc_vintages.cross_vintage;
    assert_eq!(
        cross_vintage.entries.len(),
        report.aisc_vintages.totals.comparisons
    );
    assert!(cross_vintage.instances() > 0);
    assert!(cross_vintage.count("ii") > 0);
    assert_eq!(
        report.instances.len(),
        10,
        "cross-vintage comparisons never join the five within-vintage families"
    );
    assert_eq!(report.state_counts().values().sum::<usize>(), 10);
    assert!(
        report
            .to_json()
            .contains("\"cross_vintage_ledger\":{\"aisc\":")
    );
    table_blocks_have_headers(&markdown);
}

#[test]
fn source_precision_above_binary_float_integer_range_is_not_reformatted() {
    let original = "9007199254740993.25";
    let report = report(vec![source(
        "Exact.xlsx",
        "2030Q1",
        120,
        Some(original),
        "synthetic",
    )]);
    let cell = &report.public_claims.cells["Exact.xlsx:Q1 2030!B11"];
    assert!(matches!(&cell.claim.value, PublicValue::Number { stored, .. } if stored == original));
    let markdown = assure_report::markdown(&report).unwrap();
    assert!(metrics_section(&markdown).contains(original));
    assert!(metrics_section(&markdown).contains(&cell.claim.claim_id.to_string()));
}

#[test]
fn source_metadata_cannot_create_markdown_structure_or_html() {
    let hostile =
        "https://example.invalid/<script>oops</script>|[fake](javascript:alert(1))\n# New heading";
    let report = report(vec![source("Escaped.xlsx", "2030Q1", 120, None, hostile)]);
    let markdown = assure_report::markdown(&report).unwrap();
    let inventory = markdown.split("## Identity accounting").next().unwrap();
    assert!(inventory.contains("&lt;script&gt;"));
    assert!(inventory.contains("\\|\\[fake\\]\\("));
    assert!(!inventory.contains("<script>"));
    assert!(!inventory.contains("\n# New heading"));
    assert!(!inventory.contains("[fake](javascript:"));
    table_blocks_have_headers(&markdown);
}

#[test]
fn hidden_sheet_results_stay_outside_visible_metrics_and_counts() {
    let report = run_synthetic_assurance(&[]).unwrap();
    assert!(!report.hidden_scope.instances.is_empty());
    let markdown = assure_report::markdown(&report).unwrap();
    assert!(!metrics_section(&markdown).contains(synthetic::SYNTHETIC_HIDDEN_SHEET));
    let hidden = markdown.split("## Hidden-sheet appendix").nth(1).unwrap();
    assert!(hidden.contains(synthetic::SYNTHETIC_HIDDEN_SHEET));
    assert!(hidden.contains("never merged into the visible population"));
    assert!(markdown.contains("Additional reports outside the identity certificate"));
    assert!(markdown.contains("PDF transcription assessment: not present"));
    assert!(markdown.contains("Recovery comparison: not present"));
}

#[test]
fn renderer_propagates_existing_source_packet_integrity_refusals() {
    let mut report = run_synthetic_assurance(&[]).unwrap();
    let instance = report
        .instances
        .iter_mut()
        .find(|i| i.subject == "Site B" && i.family == IdentityFamily::TccFooting)
        .unwrap();
    instance.operands[0].value = OperandValue::Int(999);
    assert_eq!(
        assure_report::markdown(&report).unwrap_err().code,
        "FMINE-REF-208"
    );
}

#[test]
fn public_cli_markdown_uses_the_report_without_changing_the_json_receipt() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let args = |format: &str| {
        ["assure", "public", "--synthetic", "--format", format]
            .into_iter()
            .map(str::to_string)
            .collect::<Vec<_>>()
    };
    let (json, _) = fmine_cli::invoke(&args("json"), &root);
    let (markdown, format) = fmine_cli::invoke(&args("markdown"), &root);
    assert_eq!(json.exit_code, 0);
    let source_report = run_synthetic_assurance(&[]).expect("same synthetic source report");
    let comparisons = assure_report::consolidation_json(&source_report);
    assert!(
        json.to_json()
            .contains(&format!("\"aisc_component_comparisons\":{comparisons}")),
        "the CLI envelope must retain the exact source-bound comparison projection"
    );
    assert_eq!(format, fmine_cli::OutputFormat::Markdown);
    assert_eq!(json.to_json(), markdown.to_json());
    let (rendered, exit) = fmine_cli::run(&args("markdown"), &root);
    assert_eq!(exit, 0);
    assert!(rendered.starts_with("# Public assurance — synthetic\n"));
    assert!(rendered.contains("## AISC component comparisons"));
    assert!(rendered.contains("## Cross-vintage identity accounting"));
    assert_eq!(rendered, markdown.rendered_markdown.unwrap());
    assert!(!rendered.starts_with("# fmine"));
    let table_args = ["check", "tables", "--synthetic"].map(str::to_string);
    let (tables, _) = fmine_cli::invoke(&table_args, &root);
    assert_eq!(tables.exit_code, 0);
    assert!(
        tables
            .to_json()
            .contains("\"cross_vintage_ledger\":{\"aisc\":")
    );
    assert!(
        tables
            .rendered_markdown
            .unwrap()
            .contains("## Cross-vintage identity accounting")
    );
}
