//! `GATE-TOUCH-06` (`BD-281`, plan §20.10): Public reconciliation known answers across
//! 1. Kibali 2016–2025 MCF series & Table 15-27 verbatim audit (`E206`, `E207`, `E210–E212`, 100% project / Barrick 45%);
//! 2. Pueblo Viejo 2025 stock-and-flow Table 15-10 (`E258`, `E259`, `E666`, 100% project / Barrick 60% Newmont 40%),
//!    inferring ~4,560 kt unreported stockpile additions + 70.4 kt plant-cone swing and filing the p.139 vs p.222 throughput discrepancy;
//! 3. Porgera 2007–2011 DOM vs OR source tables (`C4 §2.4 p.146–147`, `E298`, 100% project / Barrick 95% at 2011 effective date),
//!    recovering opposite-sign Underground (`+36% oz`) vs Open Pit (`-8% oz`) hierarchical bias posteriors.
//!
//! Claim type: **Parity** (reproduces published reconciliation tables & flags every irreproducible cell as a `DiscrepancyRecord`).

use fmine_core::codec::Encoder;
use fmine_core::hash::blake3_hash;
use fmine_evidence::RefusalCode;
use fmine_flow::compensation::{
    AnnualFactorRow, CompensationClass, WithinFactorInput, audit_multi_year_series,
    detect_within_factor_compensation,
};
use fmine_flow::graph::{FlowGraph, FlowNode, FlowStream, NodeClass, StockpileMixingModel};
use fmine_flow::verbatim_table::{
    DerivedGrade, DiscrepancyKind, DiscrepancyRecord, RowRole, VerbatimRatioRowInput,
    VerbatimTableRowInput, audit_verbatim_ratio_row, import_verbatim_table_row,
};
use fmine_recon::bias::{
    BiasObservation, BiasTargetKind, HierarchicalBiasRequest, HierarchicalGroupPrior,
    MiningSourceKind, NodeBiasSpec, fit_hierarchical_bias_posteriors,
};
use fmine_recon::stockpile::{
    GRAMS_PER_TROY_OZ, MetallurgicalStateRequest, StockpileAdditionInferenceRequest,
    SulfurDegradationCard, evaluate_stockpile_metallurgical_state,
    infer_stockpile_additions_and_cone,
};
use fmine_units::ExactQty;

#[test]
fn test_gate_touch_06_kibali_pueblo_viejo_and_porgera_public_known_answers() {
    let mut all_discrepancies: Vec<DiscrepancyRecord> = Vec::new();
    let mut enc = Encoder::new_frame(*b"GT06", 1);

    // =========================================================================
    // 1. KIBALI 2016-2025 MCF SERIES & TABLE 15-27 (E206, E207, E210-E212)
    //    Ownership basis: 100% project; Barrick 45% attributable.
    // =========================================================================
    let kibali_rows = vec![
        AnnualFactorRow::from_variance_pct(2016, -11, 9, -4).expect("2016"),
        AnnualFactorRow::from_variance_pct(2017, -5, 2, -3).expect("2017"),
        AnnualFactorRow::from_variance_pct(2018, 8, -9, -2).expect("2018"),
        AnnualFactorRow::from_variance_pct(2019, 4, -6, -3).expect("2019"),
        AnnualFactorRow::from_variance_pct(2020, 0, -8, -8).expect("2020"),
        AnnualFactorRow::from_variance_pct(2021, -6, -7, -13).expect("2021"),
        AnnualFactorRow::from_variance_pct(2022, 3, -16, -13).expect("2022"),
        AnnualFactorRow::from_variance_pct(2023, 18, -21, -7).expect("2023"),
        AnnualFactorRow::from_variance_pct(2024, 31, -27, -4).expect("2024"),
        AnnualFactorRow::from_variance_pct(2025, 27, -27, -8).expect("2025"),
    ];
    let kibali_audit = audit_multi_year_series(
        "barrick_kibali_op",
        "GC_call_vs_GC_model.barrick_kibali@v1",
        "100% project; Barrick 45% attributable",
        &kibali_rows,
        500,
        &["E210".to_string(), "E211".to_string(), "E212".to_string()],
    )
    .expect("Kibali 2016-2025 series audit");
    assert_eq!(kibali_audit.bias_prior.n_periods, 10);
    assert!(kibali_audit.bias_prior.all_same_sign);
    assert_eq!(kibali_audit.bias_prior.persistent_sign, -1);
    assert_eq!(
        kibali_audit.bias_prior.mean_metal_bias,
        ExactQty::new(-13, 200).expect("exact -6.5%")
    );
    assert_eq!(kibali_audit.masked_years, vec![2016, 2018, 2024]);
    assert_eq!(kibali_audit.partially_masked_years, vec![2023, 2025]);

    // Kibali 2024 Table 15-27 irreproducible printed MCF ratios (E206: printed 100/96/95 vs 107.07/94.56/101.16%).
    let kibali_2024_mcf = audit_verbatim_ratio_row(&VerbatimRatioRowInput {
        table_id: "Kibali_Table_15_27".to_string(),
        row_label: "2024_MCF".to_string(),
        source_locator: "Kibali_TR_2025:p222".to_string(),
        source_e_id: "E206".to_string(),
        denominator_tonnes: ExactQty::from_integer(8_541_084),
        denominator_grade_g_per_t: ExactQty::new(294, 100).expect("2.94"),
        denominator_metal_oz: ExactQty::from_integer(808_423),
        numerator_tonnes: ExactQty::from_integer(9_145_188),
        numerator_grade_g_per_t: ExactQty::new(278, 100).expect("2.78"),
        numerator_metal_oz: ExactQty::from_integer(817_769),
        printed_tonnes_ratio: ExactQty::new(100, 100).expect("100%"),
        printed_grade_ratio: ExactQty::new(96, 100).expect("96%"),
        printed_metal_ratio: ExactQty::new(95, 100).expect("95%"),
        ratio_tol_bps: 100,
    })
    .expect("Kibali 2024 MCF ratio audit");
    assert_eq!(kibali_2024_mcf.discrepancies.len(), 3);
    all_discrepancies.extend(kibali_2024_mcf.discrepancies);

    // Kibali 2024 GC Adjustment residual row printed -31.15 g/t (E207, G-TOUCH-6).
    let kibali_adj = import_verbatim_table_row(&VerbatimTableRowInput {
        table_id: "Kibali_Table_15_27".to_string(),
        row_label: "GC Adjustment".to_string(),
        source_locator: "Kibali_TR_2025:p222".to_string(),
        source_e_id: "E207".to_string(),
        role: RowRole::ResidualDifference,
        printed_tonnes: ExactQty::from_integer(-10_466),
        printed_grade_g_per_t: Some(ExactQty::new(-3_115, 100).expect("-31.15")),
        printed_metal_oz: ExactQty::from_integer(10_482),
        metal_identity_tol_bps: 100,
    })
    .expect("Kibali GC Adjustment residual row");
    assert!(matches!(
        kibali_adj.grade_g_per_t,
        DerivedGrade::NotDefined { .. }
    ));
    assert_eq!(kibali_adj.discrepancies.len(), 1);
    all_discrepancies.extend(kibali_adj.discrepancies);

    // =========================================================================
    // 2. PUEBLO VIEJO 2025 STOCK-AND-FLOW TABLE 15-10 (E258, E259, E666)
    //    Ownership basis: 100% project (Barrick 60% / Newmont 40% via PVD).
    // =========================================================================
    let pv_rows: [(&str, i128, i128, i128, i128, i128); 7] = [
        // (label, tonnes_t, grade_num, grade_den, oz_num, oz_den)
        ("Opening stocks", 95_811_000, 207, 100, 6_382_000, 1),
        ("Stocks to crusher", 8_030_500, 216, 100, 557_100, 1),
        ("Closing stocks", 92_340_000, 203, 100, 6_035_000, 1),
        ("Crusher feed", 11_377_300, 234, 100, 857_700, 1),
        ("GC mine call", 11_221_900, 236, 100, 849_800, 1),
        ("Mill check-in", 10_715_300, 242, 100, 835_100, 1),
        ("Mill check-out", 10_715_300, 244, 100, 839_600, 1),
    ];
    for (label, t, g_n, g_d, oz_n, oz_d) in pv_rows {
        let imported = import_verbatim_table_row(&VerbatimTableRowInput {
            table_id: "PV_Table_15_10_2025".to_string(),
            row_label: label.to_string(),
            source_locator: "PV_TR_2026:p222".to_string(),
            source_e_id: "E258".to_string(),
            role: RowRole::PhysicalStream,
            printed_tonnes: ExactQty::from_integer(t),
            printed_grade_g_per_t: Some(ExactQty::new(g_n, g_d).expect("grade")),
            printed_metal_oz: ExactQty::new(oz_n, oz_d).expect("oz"),
            metal_identity_tol_bps: 100, // 1% rounding bound for 2dp grades
        })
        .expect("PV row import");
        assert!(
            imported.discrepancies.is_empty(),
            "PV row '{label}' should close within 1% rounding bound, got {:?}",
            imported.discrepancies
        );
    }

    // Verify PV 2025 MCF ratios (E259: Mill check-out / GC mine call = 95% t / 103% g / 99% oz).
    let pv_mcf = audit_verbatim_ratio_row(&VerbatimRatioRowInput {
        table_id: "PV_Table_15_10_2025".to_string(),
        row_label: "2025_MCF".to_string(),
        source_locator: "PV_TR_2026:p222".to_string(),
        source_e_id: "E259".to_string(),
        denominator_tonnes: ExactQty::from_integer(11_221_900),
        denominator_grade_g_per_t: ExactQty::new(236, 100).expect("2.36"),
        denominator_metal_oz: ExactQty::from_integer(849_800),
        numerator_tonnes: ExactQty::from_integer(10_715_300),
        numerator_grade_g_per_t: ExactQty::new(244, 100).expect("2.44"),
        numerator_metal_oz: ExactQty::from_integer(839_600),
        printed_tonnes_ratio: ExactQty::new(95, 100).expect("95%"),
        printed_grade_ratio: ExactQty::new(103, 100).expect("103%"),
        printed_metal_ratio: ExactQty::new(99, 100).expect("99%"),
        ratio_tol_bps: 100,
    })
    .expect("PV 2025 MCF ratio audit");
    assert!(pv_mcf.discrepancies.is_empty());

    // Infer unreported PV 2025 stockpile additions (~4,560 kt @ ~1.43 g/t, ~210 koz, 70.6% reclaim share)
    // and plant cone swing (281.7 kt -> 352.1 kt = +70.4 kt; E258, E666).
    let pv_add = infer_stockpile_additions_and_cone(&StockpileAdditionInferenceRequest {
        site_id: "pueblo_viejo".to_string(),
        period_id: "2025".to_string(),
        grade_to_metal_divisor: GRAMS_PER_TROY_OZ,
        opening_tonnes: 95_811.0,
        opening_tonnes_sd: 958.0,
        opening_metal: 6_382.0,
        opening_metal_sd: 95.0,
        closing_tonnes: 92_340.0,
        closing_tonnes_sd: 923.0,
        closing_metal: 6_035.0,
        closing_metal_sd: 90.0,
        reclaim_to_crusher_tonnes: 8_030.5,
        reclaim_to_crusher_tonnes_sd: 160.0,
        reclaim_to_crusher_metal: 557.1,
        reclaim_to_crusher_metal_sd: 16.0,
        pit_to_crusher_tonnes: 3_346.8,
        cone_opening_tonnes: 281.7,
        cone_opening_grade: 2.41,
        cone_closing_tonnes: 352.1,
        cone_closing_grade: 2.44,
        confidence_level: 0.90,
    })
    .expect("PV 2025 addition & cone inference");
    assert!((pv_add.inferred_addition_tonnes - 4_559.5).abs() < 1.0);
    assert!((pv_add.inferred_addition_grade - 1.433).abs() < 0.01);
    assert!((pv_add.reclaim_share_of_crusher_feed_pct - 70.58).abs() < 0.05);
    assert!((pv_add.cone_delta_tonnes - 70.4).abs() < 0.1);

    // G-TOUCH-3: verify naive pit -> mill ratio across PV's stockpile refuses with FMINE-REF-102.
    let pv_graph = FlowGraph::build(
        vec![
            FlowNode::new("pit", "Pits", NodeClass::BlastedAndDug, false, None).expect("pit"),
            FlowNode::new(
                "stockpile",
                "PV Stockpiles",
                NodeClass::StockpileOrPad,
                true,
                Some(StockpileMixingModel::Layered),
            )
            .expect("sp"),
            FlowNode::new("crusher", "Crusher", NodeClass::PlantUnit, false, None).expect("cr"),
        ],
        vec![
            FlowStream::new("pit_to_sp", "pit", "stockpile", false, ["Au"]).expect("s1"),
            FlowStream::new("sp_to_cr", "stockpile", "crusher", false, ["Au"]).expect("s2"),
        ],
    )
    .expect("PV graph");
    let g3_err = pv_graph
        .check_direct_ratio_admissible("pit", "crusher")
        .expect_err("G-TOUCH-3 must refuse direct ratio across PV stockpile");
    assert_eq!(
        g3_err.to_refusal().code,
        RefusalCode::SupportMismatch102.as_str()
    );

    // File documented PV 2025 cross-section throughput discrepancy (C4 §2.5 #3: 10,868 kt p.139 vs 10,715.3 kt p.222).
    let pv_p139_vs_p222 = audit_verbatim_ratio_row(&VerbatimRatioRowInput {
        table_id: "PV_2025_CrossSection_Throughput".to_string(),
        row_label: "p139_throughput_vs_p222_mill_check_in".to_string(),
        source_locator: "PV_TR_2026:p139,p222".to_string(),
        source_e_id: "E257".to_string(),
        denominator_tonnes: ExactQty::from_integer(10_715_300),
        denominator_grade_g_per_t: ExactQty::new(242, 100).expect("2.42"),
        denominator_metal_oz: ExactQty::from_integer(835_100),
        numerator_tonnes: ExactQty::from_integer(10_868_000),
        numerator_grade_g_per_t: ExactQty::new(238, 100).expect("2.38"),
        numerator_metal_oz: ExactQty::from_integer(831_500),
        printed_tonnes_ratio: ExactQty::new(100, 100).expect("100% expected parity"),
        printed_grade_ratio: ExactQty::new(100, 100).expect("100%"),
        printed_metal_ratio: ExactQty::new(100, 100).expect("100%"),
        ratio_tol_bps: 100, // 1.425% tonnage gap exceeds 1% tolerance -> filed as DiscrepancyRecord!
    })
    .expect("PV p139 vs p222 discrepancy audit");
    assert!(
        pv_p139_vs_p222
            .discrepancies
            .iter()
            .any(|d| d.kind == DiscrepancyKind::RatioIdentityMismatch),
        "PV p139 vs p222 throughput conflict must file DiscrepancyRecord"
    );
    all_discrepancies.extend(pv_p139_vs_p222.discrepancies);

    // PV E255 sulphur degradation check (> 8.5% Stot BSD -> 74% recovery).
    let pv_met = evaluate_stockpile_metallurgical_state(&MetallurgicalStateRequest {
        stockpile_id: "pv_stockpile_l3".to_string(),
        stot_pct_mean: 9.04,
        stot_pct_sd: 0.25,
        sulfide_s_pct_mean: 8.3,
        corg_pct_mean: 0.7,
        exposure_days: 365.0,
        oxidation_rate_per_day: 0.0005,
        card: SulfurDegradationCard::pueblo_viejo_e255_bsd(),
    })
    .expect("PV metallurgical state");
    assert!((pv_met.expected_recovery - 0.742).abs() < 0.01);

    // =========================================================================
    // 3. PORGERA 2007-2011 DOM vs OR SOURCE TABLES (C4 §2.4 p.146-147, E298)
    //    Ownership basis: 100% project (Barrick 95% at 31 Dec 2011 effective date).
    // =========================================================================
    // Underground: DOM 3,141 kt / 850 koz vs OR 2,123 kt / 626 koz (+35.78% ≈ +36% oz)
    // Open pit:    DOM 15,563 kt / 1,465 koz vs OR 15,657 kt / 1,586 koz (-7.63% ≈ -8% oz)
    let porgera_ug = detect_within_factor_compensation(&WithinFactorInput {
        site_id: "porgera_ug".to_string(),
        period: "2007-2011".to_string(),
        factor_key: "DOM_over_OR.porgera_ug@v1".to_string(),
        ownership_basis: "100% project; Barrick 95% at 2011-12-31".to_string(),
        tonnes_ratio: ExactQty::new(3_141, 2_123).expect("UG t ratio"),
        grade_ratio: ExactQty::new(850 * 2_123, 626 * 3_141).expect("UG g ratio"),
        metal_ratio: ExactQty::new(850, 626).expect("UG oz ratio"),
        tol_bps: 500,
        source_e_ids: vec!["E298".to_string()],
    })
    .expect("Porgera UG reconciliation");
    assert_eq!(porgera_ug.class, CompensationClass::PartiallyMaskedOpposing);
    let ug_oz_var = (850.0_f64 / 626.0) - 1.0;
    assert!((ug_oz_var - 0.3578).abs() < 0.001);

    let porgera_op = detect_within_factor_compensation(&WithinFactorInput {
        site_id: "porgera_op".to_string(),
        period: "2007-2011".to_string(),
        factor_key: "DOM_over_OR.porgera_op@v1".to_string(),
        ownership_basis: "100% project; Barrick 95% at 2011-12-31".to_string(),
        tonnes_ratio: ExactQty::new(15_563, 15_657).expect("OP t ratio"),
        grade_ratio: ExactQty::new(1_465 * 15_657, 1_586 * 15_563).expect("OP g ratio"),
        metal_ratio: ExactQty::new(1_465, 1_586).expect("OP oz ratio"),
        tol_bps: 500,
        source_e_ids: vec!["E298".to_string()],
    })
    .expect("Porgera OP reconciliation");
    assert_eq!(porgera_op.class, CompensationClass::None);
    let op_oz_var = (1_465.0_f64 / 1_586.0) - 1.0;
    assert!((op_oz_var - (-0.0763)).abs() < 0.001);

    // Porgera source-keyed hierarchical bias posteriors preserve opposite signs (UG +36%, OP -8%).
    let porgera_bias = fit_hierarchical_bias_posteriors(&HierarchicalBiasRequest {
        evidence_cut_id: "porgera-2007-2011-tr".to_string(),
        confidence_level: 0.90,
        surprise_alarm_active: false,
        groups: vec![
            HierarchicalGroupPrior {
                group_id: "ug_group".to_string(),
                source: MiningSourceKind::Underground,
                target: BiasTargetKind::InSituMetal,
                hyper_mean: 0.0,
                hyper_sd: 0.25,
                between_node_sd: 0.05,
                refclass_id: None,
            },
            HierarchicalGroupPrior {
                group_id: "op_group".to_string(),
                source: MiningSourceKind::OpenPit,
                target: BiasTargetKind::InSituMetal,
                hyper_mean: 0.0,
                hyper_sd: 0.25,
                between_node_sd: 0.05,
                refclass_id: None,
            },
        ],
        nodes: vec![
            NodeBiasSpec {
                node_id: "porgera_ug_dom".to_string(),
                source: MiningSourceKind::Underground,
                target: BiasTargetKind::InSituMetal,
                group_id: Some("ug_group".to_string()),
                standalone_prior: None,
                refclass_id: None,
            },
            NodeBiasSpec {
                node_id: "porgera_op_dom".to_string(),
                source: MiningSourceKind::OpenPit,
                target: BiasTargetKind::InSituMetal,
                group_id: Some("op_group".to_string()),
                standalone_prior: None,
                refclass_id: None,
            },
        ],
        observations: vec![
            BiasObservation {
                label: "porgera_ug_5yr".to_string(),
                period_index: 0,
                terms: vec![("porgera_ug_dom".to_string(), 1.0)],
                observed: ug_oz_var,
                obs_sd: 0.03,
            },
            BiasObservation {
                label: "porgera_op_5yr".to_string(),
                period_index: 0,
                terms: vec![("porgera_op_dom".to_string(), 1.0)],
                observed: op_oz_var,
                obs_sd: 0.02,
            },
        ],
    })
    .expect("Porgera opposite-sign hierarchical bias fit");
    let ug_post = porgera_bias
        .node_posterior("porgera_ug_dom")
        .expect("UG posterior");
    let op_post = porgera_bias
        .node_posterior("porgera_op_dom")
        .expect("OP posterior");
    assert!(ug_post.posterior_mean > 0.30 && ug_post.excludes_zero);
    assert!(op_post.posterior_mean < -0.06 && op_post.excludes_zero);

    // Porgera 2011 reserve rows verbatim metal-mass identity check (C4 §2.4 p.138).
    for (label, t, g_n, g_d, oz) in [
        ("Open pit Proven", 13_944_000_i128, 300, 100, 1_343_000_i128),
        ("Open pit Probable", 31_105_000, 211, 100, 2_115_000),
        ("Underground Proven", 3_500_000, 717, 100, 807_000),
        ("Underground Probable", 3_624_000, 756, 100, 880_000),
        ("Stockpiles Probable", 19_802_000, 229, 100, 1_455_000),
    ] {
        let row = import_verbatim_table_row(&VerbatimTableRowInput {
            table_id: "Porgera_2011_Reserves".to_string(),
            row_label: label.to_string(),
            source_locator: "Porgera_TR_2012:p138".to_string(),
            source_e_id: "E298".to_string(),
            role: RowRole::PhysicalStream,
            printed_tonnes: ExactQty::from_integer(t),
            printed_grade_g_per_t: Some(ExactQty::new(g_n, g_d).expect("grade")),
            printed_metal_oz: ExactQty::from_integer(oz),
            metal_identity_tol_bps: 100,
        })
        .expect("Porgera reserve row");
        assert!(row.discrepancies.is_empty(), "{label} failed closure");
    }

    // Encode deterministic gate digest over all findings & filed DiscrepancyRecords.
    enc.put_u32(all_discrepancies.len() as u32);
    for d in &all_discrepancies {
        enc.put_str(&d.table_id);
        enc.put_str(&d.row_label);
        enc.put_str(&d.source_e_id);
        enc.put_str(&d.printed_value);
        enc.put_str(&d.recomputed_value);
    }
    enc.put_u64(pv_add.inferred_addition_tonnes.to_bits());
    enc.put_u64(pv_add.cone_delta_tonnes.to_bits());
    enc.put_str(&porgera_bias.update_event.digest_hex);
    let gate_digest = blake3_hash(&enc.finish()).to_hex();

    assert!(
        all_discrepancies.len() >= 5,
        "expected >= 5 filed DiscrepancyRecords across Kibali E206/E207 and PV p139/p222, got {}",
        all_discrepancies.len()
    );
    println!(
        "GATE-TOUCH-06 PASS | kibali_10y_bias=-6.50% | pv_inferred_add_kt={:.1} | pv_add_grade={:.3} | pv_reclaim_share={:.2}% | pv_cone_delta_kt={:.1} | porgera_ug_post={:+.4} | porgera_op_post={:+.4} | discrepancies_filed={} | digest={gate_digest}",
        pv_add.inferred_addition_tonnes,
        pv_add.inferred_addition_grade,
        pv_add.reclaim_share_of_crusher_feed_pct,
        pv_add.cone_delta_tonnes,
        ug_post.posterior_mean,
        op_post.posterior_mean,
        all_discrepancies.len(),
    );
}
