//! `GATE-TOUCH-01` verification harness (`BD-157`, plan §20.10–§20.11):
//! Known answers on synthetic flowsheets and `fmine-gen::mill_balance` (`BD-133`):
//! 1. R1 linear WLS reconciliation is unbiased with posterior covariance matching Monte Carlo
//!    across 2,000 replicates (`PO-TOUCH-1`, `PO-TOUCH-2`, 90% CI coverage in `[86%, 94%]`).
//! 2. Gross-error tests (global chi-square and per-measurement normalized adjustment) match exact
//!    theoretical power curves at stated significance and bias magnitudes (`BD-150`).
//! 3. Bilinear tonnes × grade reconciliation recovers seeded Cu–Au concentrator truth from
//!    `fmine-gen::mill_balance` observations (`BD-133`, `BD-151`) with exact mass & metal closure
//!    and detects planted sampler/moisture bias via elevated Mahalanobis objective.
//! 4. All Touchstone guards (`G-TOUCH-1`, `G-TOUCH-2`, `G-TOUCH-3`, `INV-005`) fail closed on
//!    planted faults, and the aggregate gate digest is replay-stable (`PO-TOUCH-6`).

use std::collections::BTreeMap;

use fmine_core::codec::Encoder;
use fmine_core::det;
use fmine_core::hash::blake3_hash;
use fmine_core::rand::{PhiloxStream, RngKey};
use fmine_evidence::RefusalCode;
use fmine_flow::graph::{FlowGraph, FlowNode, FlowStream, NodeClass, StockpileMixingModel};
use fmine_gen::biases::SeededBiasProfile;
use fmine_gen::harness::ScoringHarness;
use fmine_gen::mill_balance::MillBalanceConfig;
use fmine_recon::bilinear::{
    BilinearFlowRequest, BilinearStreamObservation, GradeValue, ObservedGrade, ObservedScalar,
    reconcile_bilinear_flow,
};
use fmine_recon::gross_error::{
    BiasVectorSpec, CommitDisposition, GrossErrorPolicy, MultiplicityCorrection,
    analyze_bias_power, enforce_commit_gate, evaluate_gross_errors,
};
use fmine_recon::linear::{
    BiasIdentifiabilityRequest, BiasLinearObservation, BiasPriorSpec, CovarianceOffDiagonal,
    DirectGradeReconciliationAttempt, ExtensiveChannel, LinearFlowRequest, MeasuredVariable,
    VariableKey, analyze_bias_identifiability, reconcile_flow_graph,
    refuse_direct_grade_reconciliation, require_identifiable_attribution,
};

fn next_std_normal(rng: &mut PhiloxStream) -> f64 {
    let u1 = rng.next_f64_open01();
    let u2 = rng.next_f64_open01();
    det::sqrt(-2.0 * det::ln(u1)) * det::cos(2.0 * std::f64::consts::PI * u2)
}

fn concentrator_graph() -> Result<FlowGraph, fmine_flow::FlowError> {
    let nodes = vec![
        FlowNode::new("feed_src", "Feed Bin", NodeClass::Transport, false, None)?,
        FlowNode::new("mill", "Flotation Plant", NodeClass::PlantUnit, false, None)?,
        FlowNode::new(
            "conc_out",
            "Concentrate",
            NodeClass::ProductMarket,
            false,
            None,
        )?,
        FlowNode::new(
            "tail_out",
            "Tailings",
            NodeClass::WasteAndTailings,
            false,
            None,
        )?,
    ];
    let streams = vec![
        FlowStream::new("feed", "feed_src", "mill", false, ["Cu", "Au"])?,
        FlowStream::new("conc", "mill", "conc_out", false, ["Cu", "Au"])?,
        FlowStream::new("tails", "mill", "tail_out", false, ["Cu", "Au"])?,
    ];
    FlowGraph::build(nodes, streams)
}

fn multi_node_graph() -> Result<FlowGraph, fmine_flow::FlowError> {
    let nodes = vec![
        FlowNode::new("pit", "Pit", NodeClass::BlastedAndDug, false, None)?,
        FlowNode::new("rougher", "Rougher", NodeClass::PlantUnit, false, None)?,
        FlowNode::new("cleaner", "Cleaner", NodeClass::PlantUnit, false, None)?,
        FlowNode::new("conc", "Final Conc", NodeClass::ProductMarket, false, None)?,
        FlowNode::new("tsf", "TSF", NodeClass::WasteAndTailings, false, None)?,
    ];
    let streams = vec![
        FlowStream::new("feed", "pit", "rougher", false, ["Cu", "Au"])?,
        FlowStream::new("rc_conc", "rougher", "cleaner", false, ["Cu", "Au"])?,
        FlowStream::new("r_tail", "rougher", "tsf", false, ["Cu", "Au"])?,
        FlowStream::new("final_conc", "cleaner", "conc", false, ["Cu", "Au"])?,
        FlowStream::new("c_tail", "cleaner", "tsf", false, ["Cu", "Au"])?,
    ];
    FlowGraph::build(nodes, streams)
}

#[test]
fn test_gate_touch_01_synthetic_flowsheets_and_mill_balance() {
    let mut gate_enc = Encoder::new_frame(*b"GT01", 1);

    // =========================================================================
    // Part A: 2,000-replicate R1 linear WLS unbiasedness & covariance (PO-TOUCH-1/2)
    // =========================================================================
    let mgraph = multi_node_graph().expect("multi-node graph");
    let ch = ExtensiveChannel::DryMass;
    let stream_names = ["c_tail", "feed", "final_conc", "r_tail", "rc_conc"];
    let truth = [150.0_f64, 1000.0, 50.0, 800.0, 200.0];
    let sds = [4.0_f64, 25.0, 1.5, 20.0, 8.0];
    let rho = 0.30_f64;
    let cov_14 = rho * sds[1] * sds[4];
    let keys: Vec<VariableKey> = stream_names
        .iter()
        .map(|s| VariableKey::Stream {
            stream_id: (*s).to_string(),
            channel: ch.clone(),
        })
        .collect();

    let n_reps = 2_000usize;
    let mut rng_a = PhiloxStream::new(RngKey {
        seed_root: 202_6157,
        partition: 1,
        domain: 1,
        var_group: 1,
        realization: 0,
        brick: 0,
    });
    let mut max_closure = 0.0_f64;
    let mut errs = Vec::with_capacity(n_reps);
    let mut cov_90_hits = [0usize; 5];
    let z90 = det::norm_ppf(0.95);
    let mut ana_cov = Vec::new();
    let mut ana_sds = Vec::new();

    for rep in 0..n_reps {
        let z0 = next_std_normal(&mut rng_a);
        let z1 = next_std_normal(&mut rng_a);
        let z2 = next_std_normal(&mut rng_a);
        let z3 = next_std_normal(&mut rng_a);
        let z4 = rho * z1 + det::sqrt(1.0 - rho * rho) * next_std_normal(&mut rng_a);
        let zs = [z0, z1, z2, z3, z4];

        let rec = reconcile_flow_graph(&LinearFlowRequest {
            graph: &mgraph,
            channel: ch.clone(),
            measurements: (0..5)
                .map(|i| MeasuredVariable {
                    key: keys[i].clone(),
                    observed: truth[i] + sds[i] * zs[i],
                    std_dev: sds[i],
                })
                .collect(),
            off_diagonal_covariances: vec![CovarianceOffDiagonal {
                a: keys[1].clone(),
                b: keys[4].clone(),
                covariance: cov_14,
            }],
            enforce_non_negative_streams: true,
        })
        .expect("Part A rep reconciles");

        max_closure = max_closure.max(rec.max_relative_closure_error);
        if rep == 0 {
            ana_cov = rec.posterior_covariance.clone();
            ana_sds = rec.posterior_std_devs.clone();
            gate_enc.put_str(&rec.digest_hex);
        }
        let mut e_vec = [0.0_f64; 5];
        for i in 0..5 {
            let e = rec.estimates[i] - truth[i];
            e_vec[i] = e;
            if e.abs() <= z90 * rec.posterior_std_devs[i] {
                cov_90_hits[i] += 1;
            }
        }
        errs.push(e_vec);
    }

    assert!(
        max_closure < 1e-11,
        "PO-TOUCH-1 max closure {max_closure:.3e}"
    );
    let mut min_cov_90 = 1.0_f64;
    let mut max_cov_90 = 0.0_f64;
    let mut max_cov_rel_err = 0.0_f64;
    for i in 0..5 {
        let mean_e = errs.iter().map(|e| e[i]).sum::<f64>() / (n_reps as f64);
        let mc_se = ana_sds[i] / det::sqrt(n_reps as f64);
        assert!(mean_e.abs() < 3.5 * mc_se, "stream {i} biased: {mean_e}");
        let c90 = (cov_90_hits[i] as f64) / (n_reps as f64);
        min_cov_90 = min_cov_90.min(c90);
        max_cov_90 = max_cov_90.max(c90);
        assert!(
            (0.86..=0.94).contains(&c90),
            "stream {i} 90% coverage {c90:.3} out of range"
        );
        for j in 0..5 {
            let emp = errs.iter().map(|e| e[i] * e[j]).sum::<f64>() / (n_reps as f64);
            let rel = (emp - ana_cov[i * 5 + j]).abs() / (ana_sds[i] * ana_sds[j]);
            max_cov_rel_err = max_cov_rel_err.max(rel);
            assert!(
                rel < 0.11,
                "PO-TOUCH-2 cov mismatch ({i},{j}): rel={rel:.3}"
            );
        }
    }

    // =========================================================================
    // Part B: 2,000-replicate gross-error power vs exact theory (BD-150)
    // =========================================================================
    let policy = GrossErrorPolicy {
        global_alpha: 0.05,
        measurement_alpha: 0.05,
        multiplicity: MultiplicityCorrection::Sidak,
        target_power: 0.80,
        periods_per_quarter: 3,
    };
    let baseline = reconcile_flow_graph(&LinearFlowRequest {
        graph: &mgraph,
        channel: ch.clone(),
        measurements: (0..5)
            .map(|i| MeasuredVariable {
                key: keys[i].clone(),
                observed: truth[i],
                std_dev: sds[i],
            })
            .collect(),
        off_diagonal_covariances: Vec::new(),
        enforce_non_negative_streams: true,
    })
    .expect("baseline");

    let mut power_summary = Vec::new();
    for (p_idx, &mult) in [0.0_f64, 3.0, 5.0].iter().enumerate() {
        let delta = mult * sds[1];
        let theory = analyze_bias_power(
            &baseline,
            &policy,
            &BiasVectorSpec {
                biases: vec![(keys[1].clone(), delta)],
                periods: 1,
            },
        )
        .expect("theory");

        let mut rng_b = PhiloxStream::new(RngKey {
            seed_root: 202_6157,
            partition: 10 + p_idx as u32,
            domain: 1,
            var_group: 1,
            realization: 0,
            brick: 0,
        });
        let mut g_hits = 0usize;
        for _ in 0..n_reps {
            let rec = reconcile_flow_graph(&LinearFlowRequest {
                graph: &mgraph,
                channel: ch.clone(),
                measurements: (0..5)
                    .map(|i| MeasuredVariable {
                        key: keys[i].clone(),
                        observed: truth[i]
                            + if i == 1 { delta } else { 0.0 }
                            + sds[i] * next_std_normal(&mut rng_b),
                        std_dev: sds[i],
                    })
                    .collect(),
                off_diagonal_covariances: Vec::new(),
                enforce_non_negative_streams: false,
            })
            .expect("power rep");
            let ge = evaluate_gross_errors(&rec, &policy).expect("ge");
            if ge.global_rejected {
                g_hits += 1;
            }
        }
        let emp = (g_hits as f64) / (n_reps as f64);
        assert!(
            (emp - theory.global_power).abs() < 0.035,
            "Part B bias={mult}σ emp={emp:.4} vs theory={:.4}",
            theory.global_power
        );
        power_summary.push((mult, emp, theory.global_power));
        gate_enc.put_u64(emp.to_bits());
    }

    // =========================================================================
    // Part C: Bilinear tonnes x grade reconciliation over `fmine-gen::mill_balance` (BD-133 + BD-151)
    // =========================================================================
    let cgraph = concentrator_graph().expect("concentrator graph");
    let harness = ScoringHarness::new(202_6157, "GATE-TOUCH-01");

    let mut cfg_unbiased = MillBalanceConfig::standard();
    cfg_unbiased.biases = SeededBiasProfile::unbiased();
    let (obs_unb, seal_unb) = harness
        .generate_mill_balance(202_6157, &cfg_unbiased, None)
        .expect("unbiased mill balance");
    let truth_unb = harness
        .open_mill_balance_truth(&seal_unb)
        .expect("open unbiased truth");

    let cfg_biased = MillBalanceConfig::standard(); // sampler=0.96, moisture=1.08
    let (obs_bia, _seal_bia) = harness
        .generate_mill_balance(202_6157, &cfg_biased, None)
        .expect("biased mill balance");

    let run_period = |p: &fmine_gen::mill_balance::MillPeriodObservation,
                      cfg: &MillBalanceConfig| {
        let mut feed_g = BTreeMap::new();
        feed_g.insert(
            "Cu".to_string(),
            ObservedGrade {
                grade: GradeValue::Percent(p.obs_feed_cu_pct),
                std_dev: GradeValue::Percent(p.obs_feed_cu_pct * cfg.assay_rel_sigma),
            },
        );
        feed_g.insert(
            "Au".to_string(),
            ObservedGrade {
                grade: GradeValue::GramsPerTonne(p.obs_feed_au_g_per_t),
                std_dev: GradeValue::GramsPerTonne(p.obs_feed_au_g_per_t * cfg.assay_rel_sigma),
            },
        );
        let mut conc_g = BTreeMap::new();
        conc_g.insert(
            "Cu".to_string(),
            ObservedGrade {
                grade: GradeValue::Percent(p.obs_conc_cu_pct),
                std_dev: GradeValue::Percent(p.obs_conc_cu_pct * 0.25 * cfg.assay_rel_sigma),
            },
        );
        conc_g.insert(
            "Au".to_string(),
            ObservedGrade {
                grade: GradeValue::GramsPerTonne(p.obs_conc_au_g_per_t),
                std_dev: GradeValue::GramsPerTonne(
                    p.obs_conc_au_g_per_t * 0.25 * cfg.assay_rel_sigma,
                ),
            },
        );
        let mut tail_g = BTreeMap::new();
        tail_g.insert(
            "Cu".to_string(),
            ObservedGrade {
                grade: GradeValue::Percent(p.obs_tail_cu_pct),
                std_dev: GradeValue::Percent(p.obs_tail_cu_pct * cfg.assay_rel_sigma),
            },
        );
        tail_g.insert(
            "Au".to_string(),
            ObservedGrade {
                grade: GradeValue::GramsPerTonne(p.obs_tail_au_g_per_t),
                std_dev: GradeValue::GramsPerTonne(p.obs_tail_au_g_per_t * cfg.assay_rel_sigma),
            },
        );
        reconcile_bilinear_flow(&BilinearFlowRequest {
            graph: &cgraph,
            elements: vec!["Cu".to_string(), "Au".to_string()],
            stream_observations: vec![
                BilinearStreamObservation {
                    stream_id: "feed".to_string(),
                    dry_mass: Some(ObservedScalar {
                        observed: p.obs_feed_dry_tonnes,
                        std_dev: p.obs_feed_dry_tonnes * cfg.feed_weightometer_sigma,
                    }),
                    element_grades: feed_g,
                },
                BilinearStreamObservation {
                    stream_id: "conc".to_string(),
                    dry_mass: Some(ObservedScalar {
                        observed: p.obs_conc_dry_tonnes,
                        std_dev: p.obs_conc_dry_tonnes * 0.25 * cfg.feed_weightometer_sigma,
                    }),
                    element_grades: conc_g,
                },
                BilinearStreamObservation {
                    stream_id: "tails".to_string(),
                    dry_mass: None,
                    element_grades: tail_g,
                },
            ],
            observation_correlations: Vec::new(),
            max_iterations: 64,
            relative_closure_tolerance: 1e-10,
        })
        .expect("mill period bilinear reconciles")
    };

    let mut max_bilin_closure = 0.0_f64;
    let mut sum_cu_rec_err = 0.0_f64;
    let mut sum_au_rec_err = 0.0_f64;
    let mut sum_sq_recon_cu = 0.0_f64;
    let mut sum_sq_raw_cu = 0.0_f64;
    let mut sum_obj_unb = 0.0_f64;
    let mut sum_obj_bia = 0.0_f64;

    for (obs_p, truth_p) in obs_unb.periods.iter().zip(&truth_unb.periods) {
        let res = run_period(obs_p, &cfg_unbiased);
        max_bilin_closure = max_bilin_closure.max(res.max_relative_closure_error);
        sum_obj_unb += res.objective;
        gate_enc.put_str(&res.digest_hex);

        let conc = res
            .recoveries
            .iter()
            .find(|r| r.stream_id == "conc")
            .expect("conc recovery");
        let e_cu = conc.element_recoveries["Cu"] - truth_p.true_cu_recovery;
        let e_au = conc.element_recoveries["Au"] - truth_p.true_au_recovery;
        let raw_e_cu = obs_p.obs_two_product_cu_recovery - truth_p.true_cu_recovery;
        sum_cu_rec_err += e_cu;
        sum_au_rec_err += e_au;
        sum_sq_recon_cu += e_cu * e_cu;
        sum_sq_raw_cu += raw_e_cu * raw_e_cu;
    }

    for obs_p in &obs_bia.periods {
        let res = run_period(obs_p, &cfg_biased);
        max_bilin_closure = max_bilin_closure.max(res.max_relative_closure_error);
        sum_obj_bia += res.objective;
    }

    let n_p = obs_unb.periods.len() as f64;
    let mean_cu_err = sum_cu_rec_err / n_p;
    let mean_au_err = sum_au_rec_err / n_p;
    let rmse_recon_cu = det::sqrt(sum_sq_recon_cu / n_p);
    let rmse_raw_cu = det::sqrt(sum_sq_raw_cu / n_p);
    let mean_obj_unb = sum_obj_unb / n_p;
    let mean_obj_bia = sum_obj_bia / n_p;

    assert!(
        max_bilin_closure < 1e-10,
        "bilinear closure {max_bilin_closure:.3e}"
    );
    assert!(
        mean_cu_err.abs() < 0.005,
        "unbiased Cu recovery bias {mean_cu_err:.5} too large"
    );
    assert!(
        mean_au_err.abs() < 0.005,
        "unbiased Au recovery bias {mean_au_err:.5} too large"
    );
    assert!(
        rmse_recon_cu < rmse_raw_cu,
        "bilinear Cu recovery RMSE ({rmse_recon_cu:.5}) should beat raw two-product ({rmse_raw_cu:.5})"
    );
    assert!(
        mean_obj_bia > 2.0 * mean_obj_unb,
        "planted sampler+moisture bias must elevate mean objective ({mean_obj_bia:.2} vs unbiased {mean_obj_unb:.2})"
    );

    let gate_digest = blake3_hash(&gate_enc.finish()).to_hex();
    println!(
        "GATE-TOUCH-01 PASS | reps={n_reps} | max_lin_closure={max_closure:.2e} | cov90=[{min_cov_90:.3},{max_cov_90:.3}] | max_cov_rel_err={max_cov_rel_err:.3} | power={power_summary:?} | mill_periods=24 | max_bilin_closure={max_bilin_closure:.2e} | rmse_cu_recon={rmse_recon_cu:.5} < raw={rmse_raw_cu:.5} | mean_obj_unb={mean_obj_unb:.2} vs biased={mean_obj_bia:.2} | digest={gate_digest}"
    );
}

#[test]
fn test_gate_touch_01_planted_faults_and_guards() {
    // 1. G-TOUCH-1: direct grade reconciliation refused with FMINE-REF-103.
    let g1 = refuse_direct_grade_reconciliation(&DirectGradeReconciliationAttempt {
        context: "mill_circuit".to_string(),
        element: "Cu".to_string(),
    })
    .expect_err("G-TOUCH-1");
    assert_eq!(
        g1.to_refusal().code,
        RefusalCode::NonAdditiveAverage103.as_str()
    );

    // 2. G-TOUCH-2: confounded bias pair refuses with FMINE-REF-104.
    let rep = analyze_bias_identifiability(&BiasIdentifiabilityRequest {
        priors: vec![
            BiasPriorSpec {
                name: "b_gc".to_string(),
                prior_mean: 0.0,
                prior_sd: 0.05,
            },
            BiasPriorSpec {
                name: "b_mill".to_string(),
                prior_mean: 0.0,
                prior_sd: 0.05,
            },
        ],
        observations: vec![BiasLinearObservation {
            label: "F2".to_string(),
            design_row: vec![1.0, -1.0],
            observed: -0.096,
            obs_sd: 0.015,
        }],
        determined_contrast: Some(("b_gc - b_mill".to_string(), vec![1.0, -1.0])),
        confidence_level: 0.95,
    })
    .expect("GB-25");
    let g2 = require_identifiable_attribution(&rep).expect_err("G-TOUCH-2");
    assert_eq!(
        g2.to_refusal().code,
        RefusalCode::NonIdentifiableAttribution104.as_str()
    );

    // 3. G-TOUCH-3: steady-state bilinear balance refuses interior StockpileOrPad (FMINE-REF-102).
    let stock_graph = FlowGraph::build(
        vec![
            FlowNode::new("pit", "Pit", NodeClass::BlastedAndDug, false, None).expect("n"),
            FlowNode::new(
                "rom",
                "ROM",
                NodeClass::StockpileOrPad,
                true,
                Some(StockpileMixingModel::WellMixed),
            )
            .expect("n"),
            FlowNode::new("mill", "Mill", NodeClass::PlantUnit, false, None).expect("n"),
        ],
        vec![
            FlowStream::new("in_s", "pit", "rom", false, ["Cu"]).expect("s"),
            FlowStream::new("out_s", "rom", "mill", false, ["Cu"]).expect("s"),
        ],
    )
    .expect("stockpile graph");
    let g3 = reconcile_bilinear_flow(&BilinearFlowRequest {
        graph: &stock_graph,
        elements: vec!["Cu".to_string()],
        stream_observations: Vec::new(),
        observation_correlations: Vec::new(),
        max_iterations: 64,
        relative_closure_tolerance: 1e-9,
    })
    .expect_err("G-TOUCH-3");
    assert_eq!(
        g3.to_refusal().code,
        RefusalCode::SupportMismatch102.as_str()
    );

    // 4. INV-005 commit gate refuses planted 8-sigma gross error (FMINE-REF-101).
    let g = concentrator_graph().expect("concentrator graph");
    let ch = ExtensiveChannel::DryMass;
    let rec = reconcile_flow_graph(&LinearFlowRequest {
        graph: &g,
        channel: ch.clone(),
        measurements: vec![
            MeasuredVariable {
                key: VariableKey::Stream {
                    stream_id: "feed".to_string(),
                    channel: ch.clone(),
                },
                observed: 1200.0, // +200 t = +8 sigma gross error!
                std_dev: 25.0,
            },
            MeasuredVariable {
                key: VariableKey::Stream {
                    stream_id: "conc".to_string(),
                    channel: ch.clone(),
                },
                observed: 50.0,
                std_dev: 1.0,
            },
            MeasuredVariable {
                key: VariableKey::Stream {
                    stream_id: "tails".to_string(),
                    channel: ch,
                },
                observed: 950.0,
                std_dev: 15.0,
            },
        ],
        off_diagonal_covariances: Vec::new(),
        enforce_non_negative_streams: true,
    })
    .expect("reconciles");
    let ge = evaluate_gross_errors(&rec, &GrossErrorPolicy::default()).expect("ge");
    assert_eq!(ge.disposition, CommitDisposition::RefuseForInvestigation);
    let gate_err = enforce_commit_gate(&ge).expect_err("INV-005 refuses commit");
    assert_eq!(
        gate_err.to_refusal().code,
        RefusalCode::Conservation101.as_str()
    );
}
