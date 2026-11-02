// Cross-crate XLSX, extractor, identity-ledger and live-dependency contracts. Included by the private
// mutation adapter so that no production testing API or duplicate workbook mutation implementation is needed.

use crate::{Lane, synthetic};

// Public live-dependency integration checks also live in xtask/tests/public_report_contract.rs.
// S2: leaves. Every expected value below is read from the seeded answers in `synthetic.rs`, never from a run.
fn synthetic_sources() -> Vec<LoadedSource> {
    crate::load_synthetic(synthetic::synthetic_workbooks(&[]))
        .expect("synthetic workbooks parse")
}

fn assess_synthetic(sources: Vec<LoadedSource>) -> PublicAssuranceReport {
    crate::assess(&synthetic::synthetic_profile(), Lane::Synthetic, sources)
}

/// The synthetic run after setting one AISC-sheet cell.
fn edited(cell: &str, value: OperandValue) -> PublicAssuranceReport {
    let mut sources = synthetic_sources();
    let o = Operand {
        role: OperandRole::Line,
        file: synthetic::SYNTHETIC_AISC_FILE.to_string(),
        sheet: synthetic::SYNTHETIC_TABLE_SHEET.to_string(),
        cell: cell.to_string(),
        value: OperandValue::Blank,
    };
    assert!(set_leaf(&mut sources, &o, &value));
    assess_synthetic(sources)
}

fn state_at<'a>(r: &'a PublicAssuranceReport, cell: &str) -> &'a IdentityState {
    match r.instances.iter().find(|i| i.cell == cell) {
        Some(i) => &i.state,
        None => panic!("no instance at {cell}"),
    }
}

/// B8 prints 400 against 500 − 100 + 0; 401 leaves a residual of 1.
#[test]
fn test_flip_margin_footing_plus_one_gives_residual_one() {
    let r = edited("B8", OperandValue::Int(401));
    match state_at(&r, "B8") {
        IdentityState::IssuerDiscrepancy(c) => assert_eq!((c.printed, c.residual), (401, 1)),
        other => panic!("B8: {other:?}"),
    }
}

/// E7 −160 → 0: E14 is checked plainly, 1000·[999.5, 1000.5]/[399.5, 400.5] spans [2495.63, 2504.38] and holds
/// 2500; E8's lines now sum to 800 against 640 printed.
#[test]
fn test_flip_margin_nci_to_zero_evaluates_cos_rate() {
    let base = assess_synthetic(synthetic_sources());
    assert_eq!(state_at(&base, "E14").code(), "v");
    let r = edited("E7", OperandValue::Int(0));
    assert_eq!(state_at(&r, "E14").code(), "i");
    match state_at(&r, "E8") {
        IdentityState::IssuerDiscrepancy(c) => {
            assert_eq!((c.printed, c.component_sum), (640, 800))
        }
        other => panic!("E8: {other:?}"),
    }
}

/// F5 0 → 1: no instance of column F is an all-zero column any more.
#[test]
fn test_flip_margin_all_zero_column_leaves_np() {
    let all_zero = IdentityState::NotPublished {
        reason: NotPublishedReason::AllZeroColumn,
    };
    let base = assess_synthetic(synthetic_sources());
    let r = edited("F5", OperandValue::Int(1));
    for cell in ["F8", "F11", "F14", "F15", "F16"] {
        assert_eq!(state_at(&base, cell), &all_zero);
        assert_ne!(state_at(&r, cell), &all_zero, "{cell}");
    }
}

/// Region North's reconstruction checks `Minestats3!B2` starts "Site G"; broken, H16 is (iv) and names the cell.
#[test]
fn test_flip_margin_label_check_breaks_reconstruction() {
    let mut sources = synthetic_sources();
    let o = Operand {
        role: OperandRole::LabelCheck,
        file: synthetic::SYNTHETIC_STATS_FILE.to_string(),
        sheet: "Minestats3".to_string(),
        cell: "B2".to_string(),
        value: OperandValue::Blank,
    };
    assert!(set_leaf(
        &mut sources,
        &o,
        &OperandValue::Token("x".to_string())
    ));
    let r = assess_synthetic(sources);
    let s = state_at(&r, "H16");
    assert_eq!(s.code(), "iv");
    assert!(s.detail().contains("Minestats3!B2 reads"), "{}", s.detail());
}

/// A parsed-level edit recomputes the file's census: Z99 = 1 changes the numeric digest.
#[test]
fn test_live_leaves_census_recomputed() {
    let base = synthetic_sources();
    let mut sources = base.clone();
    let o = Operand {
        role: OperandRole::Line,
        file: synthetic::SYNTHETIC_AISC_FILE.to_string(),
        sheet: synthetic::SYNTHETIC_TABLE_SHEET.to_string(),
        cell: "Z99".to_string(),
        value: OperandValue::Blank,
    };
    assert!(set_leaf(&mut sources, &o, &OperandValue::Int(1)));
    let file = |s: &[LoadedSource]| {
        s.iter()
            .find(|x| x.file_name == synthetic::SYNTHETIC_AISC_FILE)
            .map(|x| x.census)
            .expect("aisc")
    };
    assert_ne!(file(&base).numeric_digest, file(&sources).numeric_digest);
    assert_eq!(file(&sources).numeric_cells, file(&base).numeric_cells + 1);
}

/// The same edit made in the xlsx bytes and in the parsed cells gives the same instances and root: C8 = 351 in
/// the AISC workbook, and Site H's `QTD` label broken in the Mine Stats workbook.
#[test]
fn test_live_leaves_byte_level_matches_parsed_level() {
    // The root also binds each file's sha256, which only the byte-level edit changes, so it is not compared.
    let compare = |bytes: Vec<LoadedSource>, parsed: Vec<LoadedSource>| {
        let censuses = |s: &[LoadedSource]| s.iter().map(|x| x.census).collect::<Vec<_>>();
        assert_eq!(censuses(&bytes), censuses(&parsed));
        let (a, b) = (assess_synthetic(bytes), assess_synthetic(parsed));
        assert_eq!(a.instances, b.instances);
    };
    let at = |file: &str, sheet: &str, cell: &str| Operand {
        role: OperandRole::Line,
        file: file.to_string(),
        sheet: sheet.to_string(),
        cell: cell.to_string(),
        value: OperandValue::Blank,
    };
    let bytes = crate::load_synthetic(synthetic::synthetic_workbooks(&[(
        "C8".to_string(),
        fmine_io::xlsx::MinimalCell::Number(351.0),
    )]))
    .expect("parse");
    let mut parsed = synthetic_sources();
    let c8 = at(
        synthetic::SYNTHETIC_AISC_FILE,
        synthetic::SYNTHETIC_TABLE_SHEET,
        "C8",
    );
    assert!(set_leaf(&mut parsed, &c8, &OperandValue::Int(351)));
    compare(bytes, parsed);

    let mut files = synthetic::synthetic_workbooks(&[]);
    for f in &mut files {
        if f.0 == synthetic::SYNTHETIC_STATS_FILE {
            f.3 = synthetic::synthetic_stats_workbook("x");
        }
    }
    let bytes = crate::load_synthetic(files).expect("parse");
    let mut parsed = synthetic_sources();
    let a5 = at(synthetic::SYNTHETIC_STATS_FILE, "QTD", "A5");
    assert!(set_leaf(
        &mut parsed,
        &a5,
        &OperandValue::Token("x".to_string())
    ));
    compare(bytes, parsed);
}

/// The ounces anchor is a prefix: appending "x" to synthetic A13 changes nothing, and prepending "x" refuses
/// the table with "no 'Ounces sold' row".
#[test]
fn test_label_perturbation_prefix_label_is_prepended() {
    let base = assess_synthetic(synthetic_sources());
    let label = "Ounces sold - attributable basis (000s ounces)";
    let appended = edited("A13", OperandValue::Token(format!("{label}x")));
    assert_eq!(appended.instances, base.instances);
    assert!(appended.table_refusals.is_empty());
    let prepended = edited("A13", OperandValue::Token(format!("x{label}")));
    let refusals: Vec<(&str, &str)> = prepended
        .table_refusals
        .iter()
        .map(|t| (t.sheet.as_str(), t.reason.as_str()))
        .collect();
    assert_eq!(
        refusals,
        [(synthetic::SYNTHETIC_TABLE_SHEET, "no 'Ounces sold' row")]
    );
    // All seven anchors of the synthetic table, each mutated alone, have an effect.
    let tables = [(
        synthetic::SYNTHETIC_AISC_FILE.to_string(),
        synthetic::SYNTHETIC_TABLE_SHEET.to_string(),
    )];
    let checks = prove_labels(
        &synthetic_sources(),
        &assess_synthetic,
        &synthetic::synthetic_profile().subjects,
        &tables,
    );
    assert_eq!(checks.len(), 7);
    assert!(
        checks.iter().all(|c| c.effect == "table_refusal"),
        "{checks:#?}"
    );
    assert_eq!(checks[3].mutation, "prepend_x");
}
