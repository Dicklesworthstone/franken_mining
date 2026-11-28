//! Known-answer, oracle, and statistical verification tests for `fmine-recon` (`BD-149`,
//! `BD-150`, `BD-151`; plan §20.2–§20.6, §20.11–§20.12 `GB-24`, `GB-25`, `PO-TOUCH-1`,
//! `PO-TOUCH-2`, `G-TOUCH-1`, `G-TOUCH-2`, `G-TOUCH-3`).

use std::collections::BTreeMap;

use fmine_core::det;
use fmine_core::rand::{PhiloxStream, RngKey};
use fmine_evidence::RefusalCode;
use fmine_flow::graph::{FlowGraph, FlowNode, FlowStream, NodeClass, StockpileMixingModel};
use fmine_recon::bilinear::{
    BilinearFlowRequest, BilinearObsCorrelation, BilinearObsRef, BilinearStreamObservation,
    GradeValue, ObservedGrade, ObservedScalar, reconcile_bilinear_flow,
};
use fmine_recon::gross_error::{
    BiasVectorSpec, CommitDisposition, GrossErrorPolicy, MultiplicityCorrection,
    analyze_bias_power, enforce_commit_gate, evaluate_gross_errors,
};
use fmine_recon::linear::{
    BiasIdentifiabilityRequest, BiasLinearObservation, BiasPriorSpec, CovarianceOffDiagonal,
    DirectGradeReconciliationAttempt, ExtensiveChannel, LinearFlowRequest, MeasuredVariable,
    PartialFlowRequest, VariableKey, analyze_bias_identifiability, reconcile_flow_graph,
    reconcile_partial_flow_graph, refuse_direct_grade_reconciliation,
    require_identifiable_attribution,
};

fn next_std_normal(rng: &mut PhiloxStream) -> f64 {
    let u1 = rng.next_f64_open01();
    let u2 = rng.next_f64_open01();
    det::sqrt(-2.0 * det::ln(u1)) * det::cos(2.0 * std::f64::consts::PI * u2)
}

fn build_rougher_cleaner_graph() -> Result<FlowGraph, fmine_flow::FlowError> {
    let nodes = vec![
        FlowNode::new("feed_src", "Mill Feed", NodeClass::Transport, false, None)?,
        FlowNode::new("rougher", "Rougher Bank", NodeClass::PlantUnit, false, None)?,
        FlowNode::new("cleaner", "Cleaner Bank", NodeClass::PlantUnit, false, None)?,
        FlowNode::new(
            "conc_out",
            "Final Concentrate",
            NodeClass::ProductMarket,
            false,
            None,
        )?,
        FlowNode::new(
            "tail_out",
            "Combined Tails",
            NodeClass::WasteAndTailings,
            false,
            None,
        )?,
    ];
    let streams = vec![
        FlowStream::new("feed", "feed_src", "rougher", false, ["Cu", "Au"])?,
        FlowStream::new("rc_conc", "rougher", "cleaner", false, ["Cu", "Au"])?,
        FlowStream::new("r_tail", "rougher", "tail_out", false, ["Cu", "Au"])?,
        FlowStream::new("final_conc", "cleaner", "conc_out", false, ["Cu", "Au"])?,
        FlowStream::new("c_tail", "cleaner", "tail_out", false, ["Cu", "Au"])?,
    ];
    FlowGraph::build(nodes, streams)
}

fn build_concentrator_graph() -> Result<FlowGraph, fmine_flow::FlowError> {
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

#[test]
fn test_gb24_linear_wls_reconciliation_and_guards() {
    // Plan §20.12 GB-24:
    //   Truck tonnes (pit -> rom):    1,000,000 t, sigma = 3% = 30,000 t (var = 9.0e8)
    //   Mill weightometer (rom -> mill): 960,000 t, sigma = 1% = 9,600 t (var = 9.216e7)
    //   Stockpile delta (rom):          +25,000 t, sigma = 15,000 t      (var = 2.25e8)
    // Balance: trucks - mill - delta_rom = 0 -> raw residual r = 15,000 t, A V A^T = 1.21716e9.
    let nodes = vec![
        FlowNode::new("pit", "Open Pit", NodeClass::BlastedAndDug, false, None).expect("pit"),
        FlowNode::new(
            "rom",
            "ROM Stockpile",
            NodeClass::StockpileOrPad,
            true,
            Some(StockpileMixingModel::Fifo),
        )
        .expect("rom"),
        FlowNode::new("mill", "Mill Feed", NodeClass::PlantUnit, false, None).expect("mill"),
    ];
    let streams = vec![
        FlowStream::new("trucks", "pit", "rom", false, ["Au"]).expect("trucks"),
        FlowStream::new("mill_feed", "rom", "mill", false, ["Au"]).expect("mill_feed"),
    ];
    let graph = FlowGraph::build(nodes, streams).expect("GB-24 graph");

    let k_trucks = VariableKey::Stream {
        stream_id: "trucks".to_string(),
        channel: ExtensiveChannel::DryMass,
    };
    let k_mill = VariableKey::Stream {
        stream_id: "mill_feed".to_string(),
        channel: ExtensiveChannel::DryMass,
    };
    let k_rom = VariableKey::InventoryDelta {
        node_id: "rom".to_string(),
        channel: ExtensiveChannel::DryMass,
    };

    let req = LinearFlowRequest {
        graph: &graph,
        channel: ExtensiveChannel::DryMass,
        measurements: vec![
            MeasuredVariable {
                key: k_trucks.clone(),
                observed: 1_000_000.0,
                std_dev: 30_000.0,
            },
            MeasuredVariable {
                key: k_mill.clone(),
                observed: 960_000.0,
                std_dev: 9_600.0,
            },
            MeasuredVariable {
                key: k_rom.clone(),
                observed: 25_000.0,
                std_dev: 15_000.0,
            },
        ],
        off_diagonal_covariances: Vec::new(),
        enforce_non_negative_streams: true,
    };

    let res = reconcile_flow_graph(&req).expect("GB-24 reconciles");
    let res2 = reconcile_flow_graph(&req).expect("GB-24 replay");
    assert_eq!(
        res.digest_hex, res2.digest_hex,
        "PO-TOUCH-6 replay stability"
    );

    // Exact closed-form WLS:
    //   A V A^T = 9e8 + 9.216e7 + 2.25e8 = 1.21716e9
    //   trucks = 1_000_000 - 15_000 * (9e8 / 1.21716e9) = 988_908.6069209964
    //   mill   =   960_000 + 15_000 * (9.216e7 / 1.21716e9) = 961_135.7586512867
    //   rom    =    25_000 + 15_000 * (2.25e8 / 1.21716e9)  = 27_772.8482697427
    //   chi_sq = 15_000^2 / 1.21716e9 = 0.18485655131617865
    let trucks_hat = res.estimate_for(&k_trucks).expect("trucks");
    let mill_hat = res.estimate_for(&k_mill).expect("mill");
    let rom_hat = res.estimate_for(&k_rom).expect("rom");

    assert!((trucks_hat - 988_908.606_920_996).abs() < 1e-5);
    assert!((mill_hat - 961_135.758_651_287).abs() < 1e-5);
    assert!((rom_hat - 27_772.848_269_743).abs() < 1e-5);
    assert!(res.max_relative_closure_error < 1e-12, "PO-TOUCH-1");
    assert!((res.chi_square - 0.184_856_551_316).abs() < 1e-9);
    assert_eq!(res.degrees_of_freedom, 1);

    let ge = evaluate_gross_errors(&res, &GrossErrorPolicy::default()).expect("gross error");
    assert_eq!(ge.disposition, CommitDisposition::Admit);
    assert!((ge.global_p_value - 0.667_232).abs() < 1e-3);
    enforce_commit_gate(&ge).expect("GB-24 admits commit");

    // Guard G-TOUCH-1: direct grade reconciliation unconditionally refuses (FMINE-REF-103).
    let g1 = refuse_direct_grade_reconciliation(&DirectGradeReconciliationAttempt {
        context: "rom_to_mill".to_string(),
        element: "Au".to_string(),
    })
    .expect_err("G-TOUCH-1 refuses direct grade recon");
    assert_eq!(
        g1.to_refusal().code,
        RefusalCode::NonAdditiveAverage103.as_str()
    );

    // Guard G-TOUCH-3: omitting stockpile InventoryDelta refuses (FMINE-REF-102).
    let missing_stockpile = LinearFlowRequest {
        graph: &graph,
        channel: ExtensiveChannel::DryMass,
        measurements: vec![req.measurements[0].clone(), req.measurements[1].clone()],
        off_diagonal_covariances: Vec::new(),
        enforce_non_negative_streams: true,
    };
    let g3 = reconcile_flow_graph(&missing_stockpile)
        .expect_err("G-TOUCH-3 refuses silent zero stockpile delta");
    assert_eq!(
        g3.to_refusal().code,
        RefusalCode::SupportMismatch102.as_str()
    );
}

#[test]
fn test_amira_p754_multi_node_wls_covariance_and_partial_observability() {
    let graph = build_rougher_cleaner_graph().expect("rougher-cleaner graph");
    let ch = ExtensiveChannel::Element("Cu".to_string());
    let key = |s: &str| VariableKey::Stream {
        stream_id: s.to_string(),
        channel: ch.clone(),
    };

    // True Cu metal (t): feed = 100.0, rc_conc = 92.0, r_tail = 8.0, final_conc = 85.0, c_tail = 7.0.
    // Noisy measurements with correlated feed/rc_conc sampler error (cov = +0.40).
    let req = LinearFlowRequest {
        graph: &graph,
        channel: ch.clone(),
        measurements: vec![
            MeasuredVariable {
                key: key("feed"),
                observed: 102.4,
                std_dev: 2.0,
            },
            MeasuredVariable {
                key: key("rc_conc"),
                observed: 90.8,
                std_dev: 1.5,
            },
            MeasuredVariable {
                key: key("r_tail"),
                observed: 8.3,
                std_dev: 0.4,
            },
            MeasuredVariable {
                key: key("final_conc"),
                observed: 85.2,
                std_dev: 0.5,
            },
            MeasuredVariable {
                key: key("c_tail"),
                observed: 6.8,
                std_dev: 0.35,
            },
        ],
        off_diagonal_covariances: vec![CovarianceOffDiagonal {
            a: key("feed"),
            b: key("rc_conc"),
            covariance: 0.40,
        }],
        enforce_non_negative_streams: true,
    };

    let res = reconcile_flow_graph(&req).expect("AMIRA P754 2-node Cu balance");
    assert_eq!(res.degrees_of_freedom, 2);
    assert!(res.max_relative_closure_error < 1e-12);

    // Verify exact conservation at both nodes:
    let feed = res.estimate_for(&key("feed")).expect("feed");
    let rc_conc = res.estimate_for(&key("rc_conc")).expect("rc_conc");
    let r_tail = res.estimate_for(&key("r_tail")).expect("r_tail");
    let final_conc = res.estimate_for(&key("final_conc")).expect("final_conc");
    let c_tail = res.estimate_for(&key("c_tail")).expect("c_tail");
    assert!((feed - (rc_conc + r_tail)).abs() < 1e-11);
    assert!((rc_conc - (final_conc + c_tail)).abs() < 1e-11);

    // Every posterior variance is strictly smaller than its prior variance (variance reduction).
    for (i, k) in res.variables.iter().enumerate() {
        let prior_v = res.prior_covariance[i * res.variables.len() + i];
        let post_v = res.posterior_covariance[i * res.variables.len() + i];
        assert!(
            post_v < prior_v,
            "variable '{}' did not reduce variance ({post_v} >= {prior_v})",
            k.canonical_id()
        );
    }

    // Partial observability: r_tail unmeasured -> observable via rougher balance, leaving df = 1.
    let partial_ok = reconcile_partial_flow_graph(&PartialFlowRequest {
        graph: &graph,
        channel: ch.clone(),
        measured: vec![
            MeasuredVariable {
                key: key("feed"),
                observed: 100.0,
                std_dev: 2.0,
            },
            MeasuredVariable {
                key: key("rc_conc"),
                observed: 92.0,
                std_dev: 1.5,
            },
            MeasuredVariable {
                key: key("final_conc"),
                observed: 85.0,
                std_dev: 0.5,
            },
            MeasuredVariable {
                key: key("c_tail"),
                observed: 7.0,
                std_dev: 0.35,
            },
        ],
        off_diagonal_covariances: Vec::new(),
        unmeasured: vec![key("r_tail")],
        enforce_non_negative_streams: true,
    })
    .expect("single unmeasured r_tail is observable");
    assert_eq!(partial_ok.degrees_of_freedom, 1);
    assert!((partial_ok.latent_estimates[0] - 8.0).abs() < 1e-11);
    assert!(partial_ok.max_relative_closure_error < 1e-12);

    // Unobservable pair: BOTH r_tail and c_tail unmeasured across two nodes -> Wait, r_tail is on
    // rougher and c_tail is on cleaner, so both ARE observable (df = 0)!
    // Instead, BOTH r_tail AND rc_conc unmeasured? At rougher both appear, at cleaner rc_conc appears,
    // so rc_conc = final_conc + c_tail and r_tail = feed - rc_conc -> still observable!
    // Unobservable case: BOTH final_conc AND c_tail unmeasured at cleaner -> 2 unknowns in 1 balance!
    let unobs_err = reconcile_partial_flow_graph(&PartialFlowRequest {
        graph: &graph,
        channel: ch.clone(),
        measured: vec![
            MeasuredVariable {
                key: key("feed"),
                observed: 100.0,
                std_dev: 2.0,
            },
            MeasuredVariable {
                key: key("rc_conc"),
                observed: 92.0,
                std_dev: 1.5,
            },
            MeasuredVariable {
                key: key("r_tail"),
                observed: 8.0,
                std_dev: 0.4,
            },
        ],
        off_diagonal_covariances: Vec::new(),
        unmeasured: vec![key("final_conc"), key("c_tail")],
        enforce_non_negative_streams: true,
    })
    .expect_err("two unmeasured cleaner outlets cannot be identified");
    assert_eq!(
        unobs_err.to_refusal().code,
        RefusalCode::NonIdentifiableAttribution104.as_str()
    );
}

#[test]
fn test_gb25_bias_identifiability_refusal_and_audit_resolution() {
    // Plan §20.12 GB-25:
    //   Priors: b_gc, b_mill ~ N(0, 0.05^2)
    //   Obs 1 (F2 log-ratio): b_gc - b_mill = -0.096 +/- 0.015
    let priors = vec![
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
    ];
    let obs_f2 = BiasLinearObservation {
        label: "F2_log_ratio".to_string(),
        design_row: vec![1.0, -1.0],
        observed: -0.096,
        obs_sd: 0.015,
    };

    let rep_f2 = analyze_bias_identifiability(&BiasIdentifiabilityRequest {
        priors: priors.clone(),
        observations: vec![obs_f2.clone()],
        determined_contrast: Some(("b_gc - b_mill".to_string(), vec![1.0, -1.0])),
        confidence_level: 0.95,
    })
    .expect("GB-25 F2-only posterior");

    // Exact posterior: b_gc = -0.04593 +/- 0.03612, b_mill = +0.04593 +/- 0.03612, corr = +0.9173 > 0.80.
    assert!((rep_f2.estimates[0].posterior_mean - (-0.045_933)).abs() < 1e-4);
    assert!((rep_f2.estimates[0].posterior_sd - 0.036_118).abs() < 1e-4);
    assert!((rep_f2.estimates[1].posterior_mean - 0.045_933).abs() < 1e-4);
    assert!(!rep_f2.attribution_admissible);
    assert_eq!(rep_f2.confounded_pairs.len(), 1);
    assert!((rep_f2.confounded_pairs[0].2 - 0.9173).abs() < 1e-3);

    // Determined contrast b_gc - b_mill is tight (~ -0.092 +/- 0.015).
    let contrast = rep_f2.determined_contrast.as_ref().expect("contrast");
    assert!((contrast.mean - (-0.091_866)).abs() < 1e-4);
    assert!((contrast.sd - 0.014_674).abs() < 1e-4);

    // Guard G-TOUCH-2 refuses individual attribution (FMINE-REF-104).
    let err = require_identifiable_attribution(&rep_f2).expect_err("G-TOUCH-2 refuses attribution");
    assert_eq!(
        err.to_refusal().code,
        RefusalCode::NonIdentifiableAttribution104.as_str()
    );

    // Add mill-sampler audit: b_mill = +0.010 +/- 0.020 -> correlation drops below 0.80,
    // b_gc = -0.0713 +/- 0.0224 (95% CI excludes 0!), b_mill = +0.0182 +/- 0.0178.
    let obs_audit = BiasLinearObservation {
        label: "mill_sampler_audit".to_string(),
        design_row: vec![0.0, 1.0],
        observed: 0.010,
        obs_sd: 0.020,
    };
    let rep_audited = analyze_bias_identifiability(&BiasIdentifiabilityRequest {
        priors,
        observations: vec![obs_f2, obs_audit],
        determined_contrast: Some(("b_gc - b_mill".to_string(), vec![1.0, -1.0])),
        confidence_level: 0.95,
    })
    .expect("GB-25 audited posterior");

    assert!(rep_audited.attribution_admissible);
    require_identifiable_attribution(&rep_audited).expect("audit resolves identifiability");
    assert!((rep_audited.estimates[0].posterior_mean - (-0.0713)).abs() < 1e-3);
    assert!((rep_audited.estimates[0].posterior_sd - 0.0224).abs() < 1e-3);
    assert!(rep_audited.estimates[0].excludes_zero);
    assert!((rep_audited.estimates[1].posterior_mean - 0.0182).abs() < 1e-3);
    assert!((rep_audited.estimates[1].posterior_sd - 0.0178).abs() < 1e-3);
}

#[test]
fn test_po_touch_2_monte_carlo_posterior_covariance_match() {
    let graph = build_rougher_cleaner_graph().expect("rougher-cleaner graph");
    let ch = ExtensiveChannel::DryMass;
    let stream_names = ["c_tail", "feed", "final_conc", "r_tail", "rc_conc"]; // canonical BTree order
    // Exact true dry tonnes satisfying both balances:
    //   feed = 1000 = rc_conc (200) + r_tail (800)
    //   rc_conc = 200 = final_conc (50) + c_tail (150)
    let truth = [150.0_f64, 1000.0, 50.0, 800.0, 200.0];
    let sds = [4.0_f64, 25.0, 1.5, 20.0, 8.0];
    // Correlated error between feed (idx 1) and rc_conc (idx 4) with rho = 0.35.
    let rho = 0.35_f64;
    let cov_14 = rho * sds[1] * sds[4];

    let keys: Vec<VariableKey> = stream_names
        .iter()
        .map(|s| VariableKey::Stream {
            stream_id: (*s).to_string(),
            channel: ch.clone(),
        })
        .collect();

    let n_reps = 4_000usize;
    let n = 5usize;
    let mut sum_err = vec![0.0_f64; n];
    let mut all_errs: Vec<Vec<f64>> = Vec::with_capacity(n_reps);
    let mut covered_90 = vec![0usize; n];
    let z90 = det::norm_ppf(0.95);
    let mut analytical_cov = Vec::new();
    let mut analytical_sds = Vec::new();

    let mut rng = PhiloxStream::new(RngKey {
        seed_root: 149_2026,
        partition: 1,
        domain: 1,
        var_group: 1,
        realization: 0,
        brick: 0,
    });

    for rep in 0..n_reps {
        let z0 = next_std_normal(&mut rng);
        let z1 = next_std_normal(&mut rng);
        let z2 = next_std_normal(&mut rng);
        let z3 = next_std_normal(&mut rng);
        let z4_ind = next_std_normal(&mut rng);
        let z4 = rho * z1 + det::sqrt(1.0 - rho * rho) * z4_ind;
        let zs = [z0, z1, z2, z3, z4];

        let measurements: Vec<MeasuredVariable> = (0..n)
            .map(|i| MeasuredVariable {
                key: keys[i].clone(),
                observed: truth[i] + sds[i] * zs[i],
                std_dev: sds[i],
            })
            .collect();

        let res = reconcile_flow_graph(&LinearFlowRequest {
            graph: &graph,
            channel: ch.clone(),
            measurements,
            off_diagonal_covariances: vec![CovarianceOffDiagonal {
                a: keys[1].clone(),
                b: keys[4].clone(),
                covariance: cov_14,
            }],
            enforce_non_negative_streams: true,
        })
        .expect("MC rep reconciles");

        if rep == 0 {
            analytical_cov = res.posterior_covariance.clone();
            analytical_sds = res.posterior_std_devs.clone();
        }
        let mut errs = vec![0.0_f64; n];
        for i in 0..n {
            let e = res.estimates[i] - truth[i];
            errs[i] = e;
            sum_err[i] += e;
            if e.abs() <= z90 * res.posterior_std_devs[i] {
                covered_90[i] += 1;
            }
        }
        all_errs.push(errs);
    }

    // 1. Unbiasedness across all 5 streams.
    for i in 0..n {
        let mean_e = sum_err[i] / (n_reps as f64);
        let mc_se = analytical_sds[i] / det::sqrt(n_reps as f64);
        assert!(
            mean_e.abs() < 3.5 * mc_se,
            "stream '{}' biased: mean_err={mean_e} vs 3.5*SE={}",
            stream_names[i],
            3.5 * mc_se
        );
    }

    // 2. Empirical covariance matrix matches analytical Joseph-form posterior covariance (PO-TOUCH-2).
    for i in 0..n {
        for j in 0..n {
            let emp_cov: f64 = all_errs.iter().map(|e| e[i] * e[j]).sum::<f64>() / (n_reps as f64);
            let ana_cov = analytical_cov[i * n + j];
            let scale = analytical_sds[i] * analytical_sds[j];
            // Simultaneous 4.2-SE band across 15 distinct symmetric entries at N=4,000 (SE = sqrt(2/4000) = 2.24%).
            let tol = 0.095 * scale;
            assert!(
                (emp_cov - ana_cov).abs() < tol,
                "PO-TOUCH-2 covariance mismatch at ({i}, {j}): emp={emp_cov:.3}, ana={ana_cov:.3}, tol={tol:.3}"
            );
        }
    }

    // 3. 90% CI coverage within [87%, 93%] for every stream.
    for i in 0..n {
        let cov_pct = (covered_90[i] as f64) / (n_reps as f64);
        assert!(
            (0.87..=0.93).contains(&cov_pct),
            "stream '{}' 90% CI coverage {cov_pct:.3} outside [0.87, 0.93]",
            stream_names[i]
        );
    }
}

#[test]
fn test_gross_error_power_curves_match_exact_theory_and_quarterly_mdb() {
    let graph = build_rougher_cleaner_graph().expect("rougher-cleaner graph");
    let ch = ExtensiveChannel::DryMass;
    let stream_names = ["c_tail", "feed", "final_conc", "r_tail", "rc_conc"];
    let truth = [150.0_f64, 1000.0, 50.0, 800.0, 200.0];
    let sds = [4.0_f64, 25.0, 1.5, 20.0, 8.0];
    let keys: Vec<VariableKey> = stream_names
        .iter()
        .map(|s| VariableKey::Stream {
            stream_id: (*s).to_string(),
            channel: ch.clone(),
        })
        .collect();

    let policy = GrossErrorPolicy {
        global_alpha: 0.05,
        measurement_alpha: 0.05,
        multiplicity: MultiplicityCorrection::Sidak,
        target_power: 0.80,
        periods_per_quarter: 3,
    };

    // Baseline reconciliation at truth to inspect analytical power curves & MDB.
    let baseline = reconcile_flow_graph(&LinearFlowRequest {
        graph: &graph,
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

    let base_ge = evaluate_gross_errors(&baseline, &policy).expect("baseline gross error");
    assert_eq!(base_ge.disposition, CommitDisposition::Admit);

    // Check quarterly MDB scaling and exact inversion at target_power = 0.80 for every redundant variable!
    for mdb in &base_ge.detectable_biases {
        assert!(
            (mdb.measurement_mdb_quarter - mdb.measurement_mdb_single_period / det::sqrt(3.0))
                .abs()
                < 1e-10
        );
        assert!(
            (mdb.global_mdb_quarter - mdb.global_mdb_single_period / det::sqrt(3.0)).abs() < 1e-10
        );
        // Plugging single-period measurement MDB into analyze_bias_power must recover exact 0.80 power!
        let p_meas = analyze_bias_power(
            &baseline,
            &policy,
            &BiasVectorSpec {
                biases: vec![(mdb.variable.clone(), mdb.measurement_mdb_single_period)],
                periods: 1,
            },
        )
        .expect("power at meas MDB");
        let vp = p_meas
            .per_variable_power
            .iter()
            .find(|v| v.variable == mdb.variable)
            .expect("variable power");
        assert!(
            (vp.detection_power - policy.target_power).abs() < 1e-6,
            "meas MDB inversion error: got {} vs {}",
            vp.detection_power,
            policy.target_power
        );

        let p_glob = analyze_bias_power(
            &baseline,
            &policy,
            &BiasVectorSpec {
                biases: vec![(mdb.variable.clone(), mdb.global_mdb_single_period)],
                periods: 1,
            },
        )
        .expect("power at global MDB");
        assert!(
            (p_glob.global_power - policy.target_power).abs() < 1e-6,
            "global MDB inversion error: got {} vs {}",
            p_glob.global_power,
            policy.target_power
        );
    }

    // Monte Carlo empirical power curve sweep across bias_mult in {0, 1, 2, 3, 4, 5} sigma on `feed` (idx 1).
    let n_reps = 1_500usize;
    let feed_key = keys[1].clone();
    for (level_idx, &bias_mult) in [0.0_f64, 1.0, 2.0, 3.0, 4.0, 5.0].iter().enumerate() {
        let delta_feed = bias_mult * sds[1];
        let theory = analyze_bias_power(
            &baseline,
            &policy,
            &BiasVectorSpec {
                biases: vec![(feed_key.clone(), delta_feed)],
                periods: 1,
            },
        )
        .expect("theory power");
        let theory_feed_power = theory
            .per_variable_power
            .iter()
            .find(|v| v.variable == feed_key)
            .expect("feed power")
            .detection_power;

        let mut rng = PhiloxStream::new(RngKey {
            seed_root: 150_2026,
            partition: (level_idx as u32) + 1,
            domain: 1,
            var_group: 1,
            realization: 0,
            brick: 0,
        });
        let mut global_rej = 0usize;
        let mut feed_rej = 0usize;
        for _ in 0..n_reps {
            let measurements: Vec<MeasuredVariable> = (0..5)
                .map(|i| {
                    let bias = if i == 1 { delta_feed } else { 0.0 };
                    MeasuredVariable {
                        key: keys[i].clone(),
                        observed: truth[i] + bias + sds[i] * next_std_normal(&mut rng),
                        std_dev: sds[i],
                    }
                })
                .collect();
            let rec = reconcile_flow_graph(&LinearFlowRequest {
                graph: &graph,
                channel: ch.clone(),
                measurements,
                off_diagonal_covariances: Vec::new(),
                enforce_non_negative_streams: false,
            })
            .expect("rep reconciles");
            let ge = evaluate_gross_errors(&rec, &policy).expect("rep gross error");
            if ge.global_rejected {
                global_rej += 1;
            }
            if ge
                .suspect_ranking
                .iter()
                .any(|s| s.variable == feed_key && s.flagged)
            {
                feed_rej += 1;
            }
        }
        let emp_global = (global_rej as f64) / (n_reps as f64);
        let emp_feed = (feed_rej as f64) / (n_reps as f64);

        assert!(
            (emp_global - theory.global_power).abs() < 0.035,
            "bias={bias_mult}σ global power mismatch: emp={emp_global:.4} vs theory={:.4}",
            theory.global_power
        );
        assert!(
            (emp_feed - theory_feed_power).abs() < 0.035,
            "bias={bias_mult}σ feed measurement power mismatch: emp={emp_feed:.4} vs theory={theory_feed_power:.4}"
        );
    }

    // Large planted gross error (+150 t = +6 sigma on feed) triggers FMINE-REF-101 commit refusal
    // and ranks `feed` among top suspects!
    let mut gross_meas: Vec<MeasuredVariable> = (0..5)
        .map(|i| MeasuredVariable {
            key: keys[i].clone(),
            observed: truth[i],
            std_dev: sds[i],
        })
        .collect();
    gross_meas[1].observed += 150.0;
    let gross_rec = reconcile_flow_graph(&LinearFlowRequest {
        graph: &graph,
        channel: ch,
        measurements: gross_meas,
        off_diagonal_covariances: Vec::new(),
        enforce_non_negative_streams: true,
    })
    .expect("gross rec");
    let gross_ge = evaluate_gross_errors(&gross_rec, &policy).expect("gross eval");
    assert_eq!(
        gross_ge.disposition,
        CommitDisposition::RefuseForInvestigation
    );
    assert!(gross_ge.global_rejected);
    let gate_err = enforce_commit_gate(&gross_ge).expect_err("commit gate refuses gross error");
    assert_eq!(
        gate_err.to_refusal().code,
        RefusalCode::Conservation101.as_str()
    );
}

#[test]
fn test_bilinear_two_product_exact_and_noisy_cu_au_concentrator() {
    let graph = build_concentrator_graph().expect("concentrator graph");

    // True Cu-Au concentrator period:
    //   Feed = 100,000 t @ 0.85% Cu (850 t Cu), 1.20 g/t Au (0.12 t Au)
    //   Conc =   3,000 t @ 25.5% Cu (765 t Cu, R_Cu = 90.0%), 32.0 g/t Au (0.096 t Au, R_Au = 80.0%)
    //   Tails = 97,000 t @ 85/97000 = 0.087628866% Cu, 0.024/97000 = 0.24742268 g/t Au
    // Note: Tails dry mass is UNMEASURED (standard plant practice!).
    let f_m = 100_000.0;
    let c_m = 3_000.0;
    let t_m = 97_000.0;
    let f_cu = 0.85;
    let c_cu = 25.5;
    let t_cu = 85.0 / 97_000.0 * 100.0;
    let f_au = 1.20;
    let c_au = 32.0;
    let t_au = 0.024 / 97_000.0 * 1.0e6;

    let make_obs = |dm: Option<(f64, f64)>, cu: (f64, f64), au: (f64, f64), id: &str| {
        let mut grades = BTreeMap::new();
        grades.insert(
            "Cu".to_string(),
            ObservedGrade {
                grade: GradeValue::Percent(cu.0),
                std_dev: GradeValue::Percent(cu.1),
            },
        );
        grades.insert(
            "Au".to_string(),
            ObservedGrade {
                grade: GradeValue::GramsPerTonne(au.0),
                std_dev: GradeValue::GramsPerTonne(au.1),
            },
        );
        BilinearStreamObservation {
            stream_id: id.to_string(),
            dry_mass: dm.map(|(observed, std_dev)| ObservedScalar { observed, std_dev }),
            element_grades: grades,
        }
    };

    // 1. Noise-free exact known answer recovers exact truth & exact two-product formula.
    let exact_res = reconcile_bilinear_flow(&BilinearFlowRequest {
        graph: &graph,
        elements: vec!["Cu".to_string(), "Au".to_string()],
        stream_observations: vec![
            make_obs(Some((f_m, 1_500.0)), (f_cu, 0.017), (f_au, 0.024), "feed"),
            make_obs(Some((c_m, 15.0)), (c_cu, 0.13), (c_au, 0.16), "conc"),
            make_obs(None, (t_cu, 0.002), (t_au, 0.005), "tails"),
        ],
        observation_correlations: Vec::new(),
        max_iterations: 64,
        relative_closure_tolerance: 1e-10,
    })
    .expect("exact bilinear reconciles");

    assert_eq!(exact_res.degrees_of_freedom, 2);
    assert!(exact_res.objective < 1e-16);
    assert!(exact_res.max_relative_closure_error < 1e-12);
    assert!(!exact_res.streams["tails"].dry_mass_was_measured);
    assert!((exact_res.streams["tails"].dry_mass - t_m).abs() < 1e-6);
    assert!((exact_res.streams["feed"].elements["Cu"].grade_pct - f_cu).abs() < 1e-10);
    assert!((exact_res.streams["conc"].elements["Au"].grade_g_per_t - c_au).abs() < 1e-9);

    let conc_rec = exact_res
        .recoveries
        .iter()
        .find(|r| r.stream_id == "conc")
        .expect("conc recovery");
    assert!((conc_rec.mass_pull - 0.03).abs() < 1e-10);
    assert!((conc_rec.element_recoveries["Cu"] - 0.90).abs() < 1e-10);
    assert!((conc_rec.element_recoveries["Au"] - 0.80).abs() < 1e-10);

    // 2. Perturbed noisy measurements with correlated Cu/Au feed-sampler error (rho = 0.4) close
    //    exactly and produce replay-stable BLAKE3 digest.
    let noisy_req = BilinearFlowRequest {
        graph: &graph,
        elements: vec!["Cu".to_string(), "Au".to_string()],
        stream_observations: vec![
            make_obs(
                Some((f_m * 1.02, 1_500.0)),
                (f_cu * 0.97, 0.017),
                (f_au * 0.98, 0.024),
                "feed",
            ),
            make_obs(
                Some((c_m * 0.998, 15.0)),
                (c_cu * 1.003, 0.13),
                (c_au * 0.996, 0.16),
                "conc",
            ),
            make_obs(None, (t_cu * 1.03, 0.002), (t_au * 0.96, 0.005), "tails"),
        ],
        observation_correlations: vec![BilinearObsCorrelation {
            a: BilinearObsRef::Grade {
                stream_id: "feed".to_string(),
                element: "Cu".to_string(),
            },
            b: BilinearObsRef::Grade {
                stream_id: "feed".to_string(),
                element: "Au".to_string(),
            },
            correlation: 0.4,
        }],
        max_iterations: 64,
        relative_closure_tolerance: 1e-10,
    };
    let noisy_1 = reconcile_bilinear_flow(&noisy_req).expect("noisy bilinear");
    let noisy_2 = reconcile_bilinear_flow(&noisy_req).expect("noisy bilinear replay");
    assert_eq!(noisy_1.digest_hex, noisy_2.digest_hex);
    assert!(noisy_1.max_relative_closure_error < 1e-10);
    assert!(noisy_1.streams["feed"].elements["Cu"].grade_mass_fraction_sd > 0.0);

    // 3. Unobservable dry masses (both conc and tails unmeasured at a single node) refuse FMINE-REF-104.
    let unobs = reconcile_bilinear_flow(&BilinearFlowRequest {
        graph: &graph,
        elements: vec!["Cu".to_string()],
        stream_observations: vec![
            make_obs(Some((f_m, 1_500.0)), (f_cu, 0.017), (f_au, 0.024), "feed"),
            make_obs(None, (c_cu, 0.13), (c_au, 0.16), "conc"),
            make_obs(None, (t_cu, 0.002), (t_au, 0.005), "tails"),
        ],
        observation_correlations: Vec::new(),
        max_iterations: 64,
        relative_closure_tolerance: 1e-9,
    })
    .expect_err("unobservable masses must refuse");
    assert_eq!(
        unobs.to_refusal().code,
        RefusalCode::NonIdentifiableAttribution104.as_str()
    );
}

#[test]
fn test_bilinear_two_node_rougher_cleaner_circuit() {
    let graph = build_rougher_cleaner_graph().expect("rougher-cleaner graph");
    // Exact 2-node truth (Cu + Au):
    //   feed:       1000 t @ 1.00% Cu (10.0 t), 2.00 g/t Au (2000 g = 0.002 t)
    //   rc_conc:     200 t @ 4.60% Cu (9.20 t), 8.50 g/t Au (1700 g = 0.0017 t)   [UNMEASURED MASS]
    //   r_tail:      800 t @ 0.10% Cu (0.80 t), 0.375 g/t Au (300 g = 0.0003 t)   [UNMEASURED MASS]
    //   final_conc:   40 t @ 21.0% Cu (8.40 t), 37.5 g/t Au (1500 g = 0.0015 t)   [MEASURED MASS]
    //   c_tail:      160 t @ 0.50% Cu (0.80 t), 1.25 g/t Au (200 g = 0.0002 t)    [MEASURED MASS]
    // Two-pass mass elimination: cleaner resolves rc_conc = final_conc + c_tail,
    // then rougher resolves r_tail = feed - rc_conc!
    let make_obs = |dm: Option<(f64, f64)>, cu: (f64, f64), au: (f64, f64), id: &str| {
        let mut grades = BTreeMap::new();
        grades.insert(
            "Cu".to_string(),
            ObservedGrade {
                grade: GradeValue::Percent(cu.0),
                std_dev: GradeValue::Percent(cu.1),
            },
        );
        grades.insert(
            "Au".to_string(),
            ObservedGrade {
                grade: GradeValue::GramsPerTonne(au.0),
                std_dev: GradeValue::GramsPerTonne(au.1),
            },
        );
        BilinearStreamObservation {
            stream_id: id.to_string(),
            dry_mass: dm.map(|(observed, std_dev)| ObservedScalar { observed, std_dev }),
            element_grades: grades,
        }
    };

    let mut rng = PhiloxStream::new(RngKey {
        seed_root: 151_2026,
        partition: 1,
        domain: 1,
        var_group: 1,
        realization: 0,
        brick: 0,
    });

    let mut sum_cu_rec = 0.0;
    let mut sum_au_rec = 0.0;
    let n_trials = 50usize;
    for _ in 0..n_trials {
        let pert = |v: f64, rel_sd: f64, r: &mut PhiloxStream| {
            (v * (1.0 + rel_sd * next_std_normal(r))).max(1e-6)
        };
        let obs = vec![
            make_obs(
                Some((pert(1000.0, 0.015, &mut rng), 15.0)),
                (pert(1.00, 0.02, &mut rng), 0.02),
                (pert(2.00, 0.025, &mut rng), 0.05),
                "feed",
            ),
            make_obs(
                None,
                (pert(4.60, 0.02, &mut rng), 0.092),
                (pert(8.50, 0.025, &mut rng), 0.2125),
                "rc_conc",
            ),
            make_obs(
                None,
                (pert(0.10, 0.03, &mut rng), 0.003),
                (pert(0.375, 0.03, &mut rng), 0.01125),
                "r_tail",
            ),
            make_obs(
                Some((pert(40.0, 0.005, &mut rng), 0.20)),
                (pert(21.0, 0.005, &mut rng), 0.105),
                (pert(37.5, 0.005, &mut rng), 0.1875),
                "final_conc",
            ),
            make_obs(
                Some((pert(160.0, 0.02, &mut rng), 3.2)),
                (pert(0.50, 0.025, &mut rng), 0.0125),
                (pert(1.25, 0.025, &mut rng), 0.03125),
                "c_tail",
            ),
        ];

        let res = reconcile_bilinear_flow(&BilinearFlowRequest {
            graph: &graph,
            elements: vec!["Cu".to_string(), "Au".to_string()],
            stream_observations: obs,
            observation_correlations: Vec::new(),
            max_iterations: 64,
            relative_closure_tolerance: 1e-10,
        })
        .expect("2-node bilinear reconciles");

        assert!(res.max_relative_closure_error < 1e-10);
        assert!(!res.streams["rc_conc"].dry_mass_was_measured);
        assert!(!res.streams["r_tail"].dry_mass_was_measured);
        assert_eq!(res.degrees_of_freedom, 4); // 13 obs + 6 balances - 15 states = 4

        let cu_rec = res.streams["final_conc"].elements["Cu"].metal_mass
            / res.streams["feed"].elements["Cu"].metal_mass;
        let au_rec = res.streams["final_conc"].elements["Au"].metal_mass
            / res.streams["feed"].elements["Au"].metal_mass;
        sum_cu_rec += cu_rec;
        sum_au_rec += au_rec;
    }

    let mean_cu_rec = sum_cu_rec / (n_trials as f64);
    let mean_au_rec = sum_au_rec / (n_trials as f64);
    assert!(
        (mean_cu_rec - 0.84).abs() < 0.008,
        "overall Cu recovery mean={mean_cu_rec:.4} vs true 0.84"
    );
    assert!(
        (mean_au_rec - 0.75).abs() < 0.008,
        "overall Au recovery mean={mean_au_rec:.4} vs true 0.75"
    );
}

#[test]
fn test_recon_fresh_eyes_hardening_trace_scale_and_physicality() {
    // 1. Trace Au metal balance at 1e-6 tonnes (variance ~1e-16) where unscaled cholesky_solve
    //    rejects pivot <= 1e-14: equilibrated SPD solve keeps gross-error & power tests exact!
    let graph = build_concentrator_graph().expect("concentrator graph");
    let ch = ExtensiveChannel::Element("Au".to_string());
    let key = |s: &str| VariableKey::Stream {
        stream_id: s.to_string(),
        channel: ch.clone(),
    };
    let trace_rec = reconcile_flow_graph(&LinearFlowRequest {
        graph: &graph,
        channel: ch.clone(),
        measurements: vec![
            MeasuredVariable {
                key: key("feed"),
                observed: 1.02e-6,
                std_dev: 2.0e-8, // variance = 4e-16!
            },
            MeasuredVariable {
                key: key("conc"),
                observed: 0.80e-6,
                std_dev: 0.5e-8,
            },
            MeasuredVariable {
                key: key("tails"),
                observed: 0.20e-6,
                std_dev: 1.0e-8,
            },
        ],
        off_diagonal_covariances: Vec::new(),
        enforce_non_negative_streams: true,
    })
    .expect("trace Au reconciles");
    assert!(trace_rec.max_relative_closure_error < 1e-12);
    let trace_ge = evaluate_gross_errors(&trace_rec, &GrossErrorPolicy::default())
        .expect("trace Au gross-error evaluation succeeds despite variance 4e-16");
    assert_eq!(trace_ge.disposition, CommitDisposition::Admit);
    assert_eq!(trace_ge.detectable_biases.len(), 3);

    // 2. Bilinear refuses unused requested element ("Ag" not on any stream) and refuses sum(grades) >= 1.
    let mut g_ok = BTreeMap::new();
    g_ok.insert(
        "Cu".to_string(),
        ObservedGrade {
            grade: GradeValue::Percent(1.0),
            std_dev: GradeValue::Percent(0.02),
        },
    );
    g_ok.insert(
        "Au".to_string(),
        ObservedGrade {
            grade: GradeValue::GramsPerTonne(1.0),
            std_dev: GradeValue::GramsPerTonne(0.02),
        },
    );
    let obs_base = vec![
        BilinearStreamObservation {
            stream_id: "feed".to_string(),
            dry_mass: Some(ObservedScalar {
                observed: 100.0,
                std_dev: 1.0,
            }),
            element_grades: g_ok.clone(),
        },
        BilinearStreamObservation {
            stream_id: "conc".to_string(),
            dry_mass: Some(ObservedScalar {
                observed: 10.0,
                std_dev: 0.1,
            }),
            element_grades: g_ok.clone(),
        },
        BilinearStreamObservation {
            stream_id: "tails".to_string(),
            dry_mass: None,
            element_grades: g_ok,
        },
    ];
    assert!(
        reconcile_bilinear_flow(&BilinearFlowRequest {
            graph: &graph,
            elements: vec!["Cu".to_string(), "Ag".to_string()],
            stream_observations: obs_base.clone(),
            observation_correlations: Vec::new(),
            max_iterations: 64,
            relative_closure_tolerance: 1e-9,
        })
        .is_err()
    );

    // Unphysical sum of grades >= 100% (60% Cu + 50% Au = 110%).
    let mut g_bad = BTreeMap::new();
    g_bad.insert(
        "Cu".to_string(),
        ObservedGrade {
            grade: GradeValue::Percent(60.0),
            std_dev: GradeValue::Percent(1.0),
        },
    );
    g_bad.insert(
        "Au".to_string(),
        ObservedGrade {
            grade: GradeValue::Percent(50.0),
            std_dev: GradeValue::Percent(1.0),
        },
    );
    let mut obs_bad = obs_base;
    obs_bad[0].element_grades = g_bad;
    assert!(
        reconcile_bilinear_flow(&BilinearFlowRequest {
            graph: &graph,
            elements: vec!["Cu".to_string(), "Au".to_string()],
            stream_observations: obs_bad,
            observation_correlations: Vec::new(),
            max_iterations: 64,
            relative_closure_tolerance: 1e-9,
        })
        .is_err()
    );
}
