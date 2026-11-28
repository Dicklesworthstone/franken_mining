//! Known-answer and proof-obligation tests for `BD-154` (plan §20.8, `PO-TOUCH-3`):
//! - `PO-TOUCH-3`: R2 Kalman/RTS smoother equals exact joint batch Gaussian posterior to `< 1e-9`;
//! - Volumetric surveys reset tonnage uncertainty while preserving conditional grade uncertainty;
//! - Predictive comparison across `WellMixed`, `Fifo`, and `Layered` mixing models;
//! - Pueblo Viejo `E255` sulphur-class degradation card and `E258`/`E666` unreported additions + cone swing.

use fmine_flow::graph::StockpileMixingModel;
use fmine_recon::stockpile::{
    GRAMS_PER_TROY_OZ, MetallurgicalStateRequest, MixingComparisonRequest, ReclaimObservation,
    StockpileAdditionInferenceRequest, StockpileFlowStep, StockpileObservation, StockpileParcel,
    StockpileStatePrior, StockpileStateSpaceRequest, SulfurDegradationCard, compare_mixing_models,
    evaluate_stockpile_metallurgical_state, infer_stockpile_additions_and_cone,
    run_stockpile_kalman_smoother, solve_batch_gaussian_stockpile,
};

#[test]
fn test_po_touch_3_kalman_rts_smoother_equals_batch_gaussian_posterior() {
    let req = StockpileStateSpaceRequest {
        stockpile_id: "rom_pad_north".to_string(),
        grade_to_metal_divisor: GRAMS_PER_TROY_OZ,
        prior: StockpileStatePrior {
            opening_tonnes: 1_000.0, // kt
            opening_metal: 64.3,     // koz (~2.00 g/t)
            tonnes_sd: 60.0,
            metal_sd: 5.5,
            tonnes_metal_corr: 0.72,
        },
        steps: vec![
            StockpileFlowStep {
                period_label: "M1".to_string(),
                inflow_tonnes: 220.0,
                inflow_tonnes_sd: 9.0,
                inflow_grade: 2.30,
                inflow_grade_sd: 0.14,
                reclaim_tonnes: 180.0,
                reclaim_tonnes_sd: 4.5,
                reclaim_grade: 2.05,
                reclaim_grade_sd: 0.10,
                process_tonnes_sd: 3.0,
                process_metal_sd: 0.25,
            },
            StockpileFlowStep {
                period_label: "M2".to_string(),
                inflow_tonnes: 190.0,
                inflow_tonnes_sd: 8.5,
                inflow_grade: 1.85,
                inflow_grade_sd: 0.12,
                reclaim_tonnes: 240.0,
                reclaim_tonnes_sd: 5.0,
                reclaim_grade: 2.10,
                reclaim_grade_sd: 0.11,
                process_tonnes_sd: 3.0,
                process_metal_sd: 0.25,
            },
            StockpileFlowStep {
                period_label: "M3".to_string(),
                inflow_tonnes: 250.0,
                inflow_tonnes_sd: 10.0,
                inflow_grade: 2.40,
                inflow_grade_sd: 0.15,
                reclaim_tonnes: 200.0,
                reclaim_tonnes_sd: 4.0,
                reclaim_grade: 2.08,
                reclaim_grade_sd: 0.09,
                process_tonnes_sd: 3.0,
                process_metal_sd: 0.25,
            },
            StockpileFlowStep {
                period_label: "M4".to_string(),
                inflow_tonnes: 170.0,
                inflow_tonnes_sd: 7.0,
                inflow_grade: 1.95,
                inflow_grade_sd: 0.13,
                reclaim_tonnes: 210.0,
                reclaim_tonnes_sd: 4.5,
                reclaim_grade: 2.02,
                reclaim_grade_sd: 0.10,
                process_tonnes_sd: 3.0,
                process_metal_sd: 0.25,
            },
            StockpileFlowStep {
                period_label: "M5".to_string(),
                inflow_tonnes: 230.0,
                inflow_tonnes_sd: 9.0,
                inflow_grade: 2.20,
                inflow_grade_sd: 0.12,
                reclaim_tonnes: 195.0,
                reclaim_tonnes_sd: 4.0,
                reclaim_grade: 2.12,
                reclaim_grade_sd: 0.08,
                process_tonnes_sd: 3.0,
                process_metal_sd: 0.25,
            },
        ],
        observations_by_time: vec![
            // t=0: opening volumetric survey
            vec![StockpileObservation::VolumetricSurvey {
                observed_tonnes: 1_015.0,
                survey_sd: 18.0,
            }],
            // t=1: unobserved intermediate period
            vec![],
            // t=2: volumetric survey + metal stocktake
            vec![
                StockpileObservation::VolumetricSurvey {
                    observed_tonnes: 982.0,
                    survey_sd: 15.0,
                },
                StockpileObservation::MetalStocktake {
                    observed_metal: 65.1,
                    metal_sd: 2.2,
                },
            ],
            // t=3: unobserved
            vec![],
            // t=4: volumetric survey only
            vec![StockpileObservation::VolumetricSurvey {
                observed_tonnes: 1_010.0,
                survey_sd: 16.0,
            }],
            // t=5: closing joint survey + sonic-drill stocktake (Cortez style E194)
            vec![StockpileObservation::JointStocktake {
                observed_tonnes: 1_038.0,
                observed_metal: 70.4,
                tonnes_sd: 14.0,
                metal_sd: 1.9,
                corr: 0.55,
            }],
        ],
        confidence_level: 0.90,
    };

    let rts = run_stockpile_kalman_smoother(&req).expect("RTS smoother");
    let rts_repeat = run_stockpile_kalman_smoother(&req).expect("RTS replay");
    assert_eq!(rts.digest_hex, rts_repeat.digest_hex);

    let batch = solve_batch_gaussian_stockpile(&req).expect("batch Gaussian oracle");
    let dim = 2 * (req.steps.len() + 1);

    // PO-TOUCH-3: exact agreement of smoothed means and 2x2 covariances with joint batch Gaussian.
    for (t, est) in rts.periods.iter().enumerate() {
        let b_m = batch.joint_mean[2 * t];
        let b_met = batch.joint_mean[2 * t + 1];
        assert!(
            (est.tonnes_mean - b_m).abs() < 1.0e-9,
            "t={t} tonnes mean mismatch: rts={} batch={b_m}",
            est.tonnes_mean
        );
        assert!(
            (est.metal_mean - b_met).abs() < 1.0e-9,
            "t={t} metal mean mismatch: rts={} batch={b_met}",
            est.metal_mean
        );
        let b_p00 = batch.joint_covariance[(2 * t) * dim + (2 * t)];
        let b_p01 = batch.joint_covariance[(2 * t) * dim + (2 * t + 1)];
        let b_p11 = batch.joint_covariance[(2 * t + 1) * dim + (2 * t + 1)];
        assert!(
            (est.covariance_2x2[0] - b_p00).abs() < 1.0e-9,
            "t={t} P00 mismatch: rts={} batch={b_p00}",
            est.covariance_2x2[0]
        );
        assert!(
            (est.covariance_2x2[1] - b_p01).abs() < 1.0e-9,
            "t={t} P01 mismatch: rts={} batch={b_p01}",
            est.covariance_2x2[1]
        );
        assert!(
            (est.covariance_2x2[3] - b_p11).abs() < 1.0e-9,
            "t={t} P11 mismatch: rts={} batch={b_p11}",
            est.covariance_2x2[3]
        );
        // Smoothing cannot increase uncertainty relative to forward filtering.
        assert!(est.tonnes_sd <= est.filtered_tonnes_sd + 1.0e-12);
    }

    // Verify exact lag-1 delta SDs against full joint batch covariance Var(s_{t+1} - s_t).
    for (t, d) in rts.deltas.iter().enumerate() {
        let i0 = 2 * t;
        let i1 = 2 * (t + 1);
        let var_dt = batch.joint_covariance[i1 * dim + i1] + batch.joint_covariance[i0 * dim + i0]
            - 2.0 * batch.joint_covariance[i1 * dim + i0];
        let var_dm = batch.joint_covariance[(i1 + 1) * dim + (i1 + 1)]
            + batch.joint_covariance[(i0 + 1) * dim + (i0 + 1)]
            - 2.0 * batch.joint_covariance[(i1 + 1) * dim + (i0 + 1)];
        assert!(
            (d.delta_tonnes_sd - var_dt.sqrt()).abs() < 1.0e-9,
            "t={t} delta_tonnes_sd mismatch"
        );
        assert!(
            (d.delta_metal_sd - var_dm.sqrt()).abs() < 1.0e-9,
            "t={t} delta_metal_sd mismatch"
        );
    }
}

#[test]
fn test_bd154_volumetric_survey_resets_tonnage_sd_without_collapsing_grade_sd() {
    // §20.8 physical law: when opening tonnage M and grade g are independent (so Cov(M, m) = g * Var(M) / div),
    // a volumetric survey observing M collapses Var(M) while leaving conditional grade variance Var(g | M) untouched!
    let m0 = 1_000.0_f64; // kt
    let g0 = 2.0_f64; // g/t
    let sd_m = 50.0_f64; // 5% tonnage uncertainty
    let sd_g = 0.25_f64; // 12.5% grade uncertainty
    let metal0 = m0 * g0 / GRAMS_PER_TROY_OZ;
    let var_m = sd_m * sd_m;
    let var_metal =
        (g0 * g0 * var_m + m0 * m0 * sd_g * sd_g) / (GRAMS_PER_TROY_OZ * GRAMS_PER_TROY_OZ);
    let cov_mm = g0 * var_m / GRAMS_PER_TROY_OZ;
    let corr = cov_mm / (sd_m * var_metal.sqrt());

    let base_step = StockpileFlowStep {
        period_label: "P1".to_string(),
        inflow_tonnes: 0.0,
        inflow_tonnes_sd: 0.0,
        inflow_grade: 0.0,
        inflow_grade_sd: 0.0,
        reclaim_tonnes: 0.0,
        reclaim_tonnes_sd: 0.0,
        reclaim_grade: 0.0,
        reclaim_grade_sd: 0.0,
        process_tonnes_sd: 1.0e-3,
        process_metal_sd: 1.0e-5,
    };

    // Case A: sharp volumetric survey at t=0 (survey_sd = 2 kt vs prior 50 kt).
    let survey_only = run_stockpile_kalman_smoother(&StockpileStateSpaceRequest {
        stockpile_id: "sp_survey_test".to_string(),
        grade_to_metal_divisor: GRAMS_PER_TROY_OZ,
        prior: StockpileStatePrior {
            opening_tonnes: m0,
            opening_metal: metal0,
            tonnes_sd: sd_m,
            metal_sd: var_metal.sqrt(),
            tonnes_metal_corr: corr,
        },
        steps: vec![base_step.clone()],
        observations_by_time: vec![
            vec![StockpileObservation::VolumetricSurvey {
                observed_tonnes: m0,
                survey_sd: 2.0,
            }],
            vec![],
        ],
        confidence_level: 0.90,
    })
    .expect("survey-only smoother");

    let t0 = &survey_only.periods[0];
    // Tonnage SD collapsed > 95% (50 kt -> ~2.0 kt).
    assert!(
        t0.tonnes_sd < 2.1,
        "survey must reset tonnage SD, got {}",
        t0.tonnes_sd
    );
    // Grade SD remains at prior 0.25 g/t (volumetric survey carries ZERO grade information!).
    assert!(
        (t0.derived_grade_sd - sd_g).abs() < 1.0e-4,
        "volumetric survey must preserve grade SD ({sd_g}), got {}",
        t0.derived_grade_sd
    );

    // Case B: add a sonic-drill metal stocktake -> now grade SD drops sharply!
    let with_sonic = run_stockpile_kalman_smoother(&StockpileStateSpaceRequest {
        stockpile_id: "sp_sonic_test".to_string(),
        grade_to_metal_divisor: GRAMS_PER_TROY_OZ,
        prior: StockpileStatePrior {
            opening_tonnes: m0,
            opening_metal: metal0,
            tonnes_sd: sd_m,
            metal_sd: var_metal.sqrt(),
            tonnes_metal_corr: corr,
        },
        steps: vec![base_step],
        observations_by_time: vec![
            vec![
                StockpileObservation::VolumetricSurvey {
                    observed_tonnes: m0,
                    survey_sd: 2.0,
                },
                StockpileObservation::MetalStocktake {
                    observed_metal: metal0,
                    metal_sd: 1.5, // ~0.047 g/t equivalent
                },
            ],
            vec![],
        ],
        confidence_level: 0.90,
    })
    .expect("survey + sonic smoother");
    assert!(
        with_sonic.periods[0].derived_grade_sd < 0.06,
        "sonic drill stocktake must reduce grade SD, got {}",
        with_sonic.periods[0].derived_grade_sd
    );
}

#[test]
fn test_bd154_mixing_model_predictive_comparison_and_sulfide_degradation() {
    // Opening parcel: 100 kt @ 1.20 g/t; dumped in P1: 100 kt @ 3.60 g/t.
    // Reclaiming 100 kt in P1:
    // - Fifo predicts 1.20 g/t (oldest first)
    // - WellMixed predicts 2.40 g/t
    // - Layered (top-down LIFO) predicts 3.60 g/t
    let opening = vec![StockpileParcel {
        parcel_id: "base_lift".to_string(),
        dump_period: 0,
        dry_tonnes: 100.0,
        grade: 1.20,
        stot_pct: 6.2,
        sulfide_s_pct: 5.5,
        corg_pct: 0.4,
    }];
    let dumped = vec![StockpileParcel {
        parcel_id: "top_lift".to_string(),
        dump_period: 1,
        dry_tonnes: 100.0,
        grade: 3.60,
        stot_pct: 9.1,
        sulfide_s_pct: 8.2,
        corg_pct: 0.8,
    }];

    // Data generated by FIFO (1.22 g/t then 3.58 g/t).
    let fifo_cmp = compare_mixing_models(&MixingComparisonRequest {
        opening_parcels: opening.clone(),
        dumped_parcels: dumped.clone(),
        reclaims: vec![
            ReclaimObservation {
                period: 1,
                reclaim_tonnes: 100.0,
                observed_grade: 1.22,
                obs_grade_sd: 0.15,
            },
            ReclaimObservation {
                period: 2,
                reclaim_tonnes: 100.0,
                observed_grade: 3.58,
                obs_grade_sd: 0.15,
            },
        ],
    })
    .expect("FIFO comparison");
    assert_eq!(fifo_cmp.ranked_models[0].model, StockpileMixingModel::Fifo);
    assert!(fifo_cmp.top_log_likelihood_margin > 20.0);

    // Data generated by Layered/LIFO (3.55 g/t then 1.24 g/t).
    let lifo_cmp = compare_mixing_models(&MixingComparisonRequest {
        opening_parcels: opening,
        dumped_parcels: dumped,
        reclaims: vec![
            ReclaimObservation {
                period: 1,
                reclaim_tonnes: 100.0,
                observed_grade: 3.55,
                obs_grade_sd: 0.15,
            },
            ReclaimObservation {
                period: 2,
                reclaim_tonnes: 100.0,
                observed_grade: 1.24,
                obs_grade_sd: 0.15,
            },
        ],
    })
    .expect("Layered comparison");
    assert_eq!(
        lifo_cmp.ranked_models[0].model,
        StockpileMixingModel::Layered
    );
    assert!(lifo_cmp.top_log_likelihood_margin > 20.0);

    // Pueblo Viejo E255 BSD sulphur degradation: Stot > 8.5% drops recovery from 87% to 74%.
    let low_s = evaluate_stockpile_metallurgical_state(&MetallurgicalStateRequest {
        stockpile_id: "pv_sp_low_s".to_string(),
        stot_pct_mean: 6.0,
        stot_pct_sd: 0.3,
        sulfide_s_pct_mean: 5.4,
        corg_pct_mean: 0.5,
        exposure_days: 90.0,
        oxidation_rate_per_day: 0.001,
        card: SulfurDegradationCard::pueblo_viejo_e255_bsd(),
    })
    .expect("low S");
    assert!((low_s.expected_recovery - 0.87).abs() < 1.0e-3);
    assert!(!low_s.reserve_disclosure_claim());

    let high_s = evaluate_stockpile_metallurgical_state(&MetallurgicalStateRequest {
        stockpile_id: "pv_sp_high_s".to_string(),
        stot_pct_mean: 9.5,
        stot_pct_sd: 0.3,
        sulfide_s_pct_mean: 8.8,
        corg_pct_mean: 0.9,
        exposure_days: 365.0,
        oxidation_rate_per_day: 0.001,
        card: SulfurDegradationCard::pueblo_viejo_e255_bsd(),
    })
    .expect("high S");
    assert!((high_s.expected_recovery - 0.74).abs() < 1.0e-3);
    assert!(high_s.oxidation_exposure_theta > low_s.oxidation_exposure_theta);

    // Boundary Stot = 8.5% +/- 0.5% induces substantial recovery uncertainty (~6.5 percentage points SD).
    let boundary_s = evaluate_stockpile_metallurgical_state(&MetallurgicalStateRequest {
        stockpile_id: "pv_sp_boundary".to_string(),
        stot_pct_mean: 8.5,
        stot_pct_sd: 0.5,
        sulfide_s_pct_mean: 7.8,
        corg_pct_mean: 0.7,
        exposure_days: 180.0,
        oxidation_rate_per_day: 0.001,
        card: SulfurDegradationCard::pueblo_viejo_e255_bsd(),
    })
    .expect("boundary S");
    assert!((boundary_s.expected_recovery - 0.805).abs() < 0.01);
    assert!(boundary_s.recovery_sd > 0.06);
}

#[test]
fn test_bd154_pueblo_viejo_2025_additions_and_cone_inference() {
    // FACT E258, E666 (Pueblo Viejo 2026 NI 43-101 Table 15-10, PDF p.222; 100% project basis):
    let inf = infer_stockpile_additions_and_cone(&StockpileAdditionInferenceRequest {
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
    .expect("PV 2025 addition inference");

    // 92,340 - 95,811 + 8,030.5 = 4,559.5 kt (~4,560 kt; E258).
    assert!((inf.inferred_addition_tonnes - 4_559.5).abs() < 1.0e-6);
    // 6,035 - 6,382 + 557.1 = 210.1 koz (~210 koz; E258).
    assert!((inf.inferred_addition_metal - 210.1).abs() < 1.0e-6);
    // 210.1 * 31.1034768 / 4,559.5 = 1.4332 g/t (~1.43 g/t; E258).
    assert!((inf.inferred_addition_grade - 1.4332).abs() < 0.005);
    // Reclaim share: 8,030.5 / (8,030.5 + 3,346.8) = 70.5835% (~70.6%; E258).
    assert!((inf.reclaim_share_of_crusher_feed_pct - 70.5835).abs() < 0.01);
    // Cone swing: 352.1 - 281.7 = +70.4 kt (E666).
    assert!((inf.cone_delta_tonnes - 70.4).abs() < 1.0e-6);
}
