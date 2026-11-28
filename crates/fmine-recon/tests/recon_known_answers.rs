//! Known-answer, oracle, and statistical verification tests for `fmine-recon` (`BD-149`,
//! `BD-150`, `BD-151`; plan §20.2–§20.6, §20.11–§20.12 `GB-24`, `GB-25`, `PO-TOUCH-1`,
//! `PO-TOUCH-2`, `G-TOUCH-1`, `G-TOUCH-2`, `G-TOUCH-3`).

use std::collections::BTreeMap;

use fmine_claim::refclass::RefClassId;
use fmine_core::cert::{DeterminismClass, InputDigest};
use fmine_core::det;
use fmine_core::hash::sha256;
use fmine_core::rand::{PhiloxStream, RngKey};
use fmine_evidence::RefusalCode;
use fmine_flow::graph::{FlowGraph, FlowNode, FlowStream, NodeClass, StockpileMixingModel};
use fmine_recon::bias::{
    BiasObservation, BiasTargetKind, FactorMultiplierAttempt, HierarchicalBiasRequest,
    HierarchicalGroupPrior, MiningSourceKind, NodeBiasSpec, PosteriorSampleRequest,
    StandaloneNodePrior, fit_hierarchical_bias_posteriors, refuse_factor_multiplier_application,
    sample_hierarchical_posterior,
};
use fmine_recon::bilinear::{
    BilinearFlowRequest, BilinearObsCorrelation, BilinearObsRef, BilinearStreamObservation,
    GradeValue, ObservedGrade, ObservedScalar, reconcile_bilinear_flow,
};
use fmine_recon::certificate::{
    AttestationStatus, BalanceCertificateSummary, CertifiedFactorInterval,
    CertifiedQuantityInterval, IdentifiabilityCertificateSummary, NodeBalanceRecord,
    ParityDifferenceStatus, ParityExplanation, PosteriorCertificateSummary,
    ReconciliationCertificate, ReconciliationCertificateRequest, RefusedPairVoiRecord,
    RegisteredForecastScore,
};
use fmine_recon::gross_error::{
    BiasVectorSpec, CommitDisposition, GrossErrorPolicy, MultiplicityCorrection,
    analyze_bias_power, enforce_commit_gate, evaluate_gross_errors,
};
use fmine_recon::identifiability::{
    CandidateMeasurement, CandidateMeasurementKind, IdentifiabilityRequest, MeasurementVoiRequest,
    analyze_hierarchical_identifiability, analyze_identifiability, evaluate_measurement_voi,
    require_full_identifiability, require_parameter_identifiable,
};
use fmine_recon::linear::{
    BiasIdentifiabilityRequest, BiasLinearObservation, BiasPriorSpec, CovarianceOffDiagonal,
    DirectGradeReconciliationAttempt, ExtensiveChannel, LinearFlowRequest, MeasuredVariable,
    NamedLinearConstraint, NamedLinearRequest, PartialFlowRequest, VariableKey,
    analyze_bias_identifiability, reconcile_flow_graph, reconcile_named_system,
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

#[test]
fn test_bd152_hierarchical_bias_pooling_porgera_sources_and_safeguards() {
    // 1. Exact analytical partial-pooling oracle:
    //    mu_g ~ N(0, sigma_mu^2), b_1, b_2 | mu_g ~ N(mu_g, tau^2)
    //    Observe y_1 = b_1 + eps_1 = -0.08, eps_1 ~ N(0, sigma_y^2), NO observation on b_2.
    //    Exact marginal posterior of b_2 given y_1:
    //      Cov(b_2, y_1) = sigma_mu^2, Var(y_1) = sigma_mu^2 + tau^2 + sigma_y^2
    //      E[b_2 | y_1] = (sigma_mu^2 / Var(y_1)) * y_1
    //      Var(b_2 | y_1) = (sigma_mu^2 + tau^2) - sigma_mu^4 / Var(y_1).
    let sigma_mu = 0.05_f64;
    let tau = 0.03_f64;
    let sigma_y = 0.01_f64;
    let y1 = -0.08_f64;
    let rc_id = RefClassId::parse("REFCLASS-AU-OP-RECON").expect("valid refclass id");

    let req = HierarchicalBiasRequest {
        evidence_cut_id: "cut-2026Q1-001".to_string(),
        surprise_alarm_active: false,
        confidence_level: 0.90,
        groups: vec![HierarchicalGroupPrior {
            group_id: "op_domains".to_string(),
            source: MiningSourceKind::OpenPit,
            target: BiasTargetKind::InSituMetal,
            hyper_mean: 0.0,
            hyper_sd: sigma_mu,
            between_node_sd: tau,
            refclass_id: Some(rc_id.clone()),
        }],
        nodes: vec![
            NodeBiasSpec {
                node_id: "domain_12".to_string(),
                source: MiningSourceKind::OpenPit,
                target: BiasTargetKind::InSituMetal,
                group_id: Some("op_domains".to_string()),
                standalone_prior: None,
                refclass_id: None,
            },
            NodeBiasSpec {
                node_id: "domain_13".to_string(),
                source: MiningSourceKind::OpenPit,
                target: BiasTargetKind::InSituMetal,
                group_id: Some("op_domains".to_string()),
                standalone_prior: None,
                refclass_id: None,
            },
        ],
        observations: vec![BiasObservation {
            label: "domain_12_q1".to_string(),
            period_index: 0,
            terms: vec![("domain_12".to_string(), 1.0)],
            observed: y1,
            obs_sd: sigma_y,
        }],
    };
    let res = fit_hierarchical_bias_posteriors(&req).expect("hierarchical fit");
    let var_y1 = sigma_mu * sigma_mu + tau * tau + sigma_y * sigma_y;
    let expected_b2_mean = (sigma_mu * sigma_mu / var_y1) * y1;
    let expected_b2_var = (sigma_mu * sigma_mu + tau * tau) - (sigma_mu.powi(4) / var_y1);
    let expected_b1_mean = ((sigma_mu * sigma_mu + tau * tau) / var_y1) * y1;

    let d12 = res.node_posterior("domain_12").expect("domain_12");
    let d13 = res.node_posterior("domain_13").expect("domain_13");
    assert!((d12.posterior_mean - expected_b1_mean).abs() < 1e-12);
    assert!((d13.posterior_mean - expected_b2_mean).abs() < 1e-12);
    assert!((d13.posterior_sd - det::sqrt(expected_b2_var)).abs() < 1e-12);
    assert_eq!(d12.direct_observation_count, 1);
    assert_eq!(d13.direct_observation_count, 0);
    assert!(!res.update_event.requires_investigation_case);
    assert_eq!(res.update_event.refclass_ids, vec![rc_id]);

    // 2. Opposite-sign mining sources (Porgera E298: UG +36%, OP -8%) separated by source group,
    //    plus surprise-monitor safeguard routing through InvestigationCase (§20.5).
    let porgera = fit_hierarchical_bias_posteriors(&HierarchicalBiasRequest {
        evidence_cut_id: "cut-porgera-2011".to_string(),
        surprise_alarm_active: true,
        confidence_level: 0.95,
        groups: vec![
            HierarchicalGroupPrior {
                group_id: "open_pit".to_string(),
                source: MiningSourceKind::OpenPit,
                target: BiasTargetKind::InSituMetal,
                hyper_mean: 0.0,
                hyper_sd: 0.25,
                between_node_sd: 0.05,
                refclass_id: None,
            },
            HierarchicalGroupPrior {
                group_id: "underground".to_string(),
                source: MiningSourceKind::Underground,
                target: BiasTargetKind::InSituMetal,
                hyper_mean: 0.0,
                hyper_sd: 0.25,
                between_node_sd: 0.05,
                refclass_id: None,
            },
        ],
        nodes: vec![
            NodeBiasSpec {
                node_id: "op_main".to_string(),
                source: MiningSourceKind::OpenPit,
                target: BiasTargetKind::InSituMetal,
                group_id: Some("open_pit".to_string()),
                standalone_prior: None,
                refclass_id: None,
            },
            NodeBiasSpec {
                node_id: "ug_zone7".to_string(),
                source: MiningSourceKind::Underground,
                target: BiasTargetKind::InSituMetal,
                group_id: Some("underground".to_string()),
                standalone_prior: None,
                refclass_id: None,
            },
        ],
        observations: vec![
            BiasObservation {
                label: "op_2007_2011".to_string(),
                period_index: 0,
                terms: vec![("op_main".to_string(), 1.0)],
                observed: -0.08,
                obs_sd: 0.015,
            },
            BiasObservation {
                label: "ug_2007_2011".to_string(),
                period_index: 0,
                terms: vec![("ug_zone7".to_string(), 1.0)],
                observed: 0.36,
                obs_sd: 0.025,
            },
        ],
    })
    .expect("porgera fit");
    assert!(
        porgera
            .node_posterior("op_main")
            .expect("op")
            .posterior_mean
            < -0.05
    );
    assert!(
        porgera
            .node_posterior("ug_zone7")
            .expect("ug")
            .posterior_mean
            > 0.30
    );
    assert!(porgera.update_event.requires_investigation_case);

    // 3. Guard G-TOUCH-5 refuses factor multiplier application.
    let mult_err = refuse_factor_multiplier_application(&FactorMultiplierAttempt {
        factor_name: "F2".to_string(),
        multiplier: 0.93,
        target_context: "next_month_gc_forecast".to_string(),
    })
    .expect_err("G-TOUCH-5 must refuse multiplier");
    assert_eq!(
        mult_err.to_refusal().code,
        RefusalCode::OutsidePhysicalDomain109.as_str()
    );

    // 4. Posterior sampler recovers exact joint mean & covariance over 4,000 draws.
    let draws = sample_hierarchical_posterior(
        &res,
        &PosteriorSampleRequest {
            seed: RngKey {
                seed_root: 152_2026,
                partition: 1,
                domain: 1,
                var_group: 1,
                realization: 0,
                brick: 0,
            },
            draw_count: 4_000,
        },
    )
    .expect("sample draws");
    let d13_draws = draws.draws_for("domain_13").expect("domain_13 draws");
    let mean_emp = d13_draws.iter().sum::<f64>() / 4_000.0;
    let var_emp = d13_draws
        .iter()
        .map(|&v| (v - mean_emp) * (v - mean_emp))
        .sum::<f64>()
        / 3_999.0;
    assert!((mean_emp - d13.posterior_mean).abs() < 3.5 * d13.posterior_sd / det::sqrt(4_000.0));
    assert!((det::sqrt(var_emp) - d13.posterior_sd).abs() / d13.posterior_sd < 0.06);
}

#[test]
fn test_bd153_observation_rank_nullspace_and_measurement_voi() {
    // 1. GB-25 confounded system: H = [[1, -1]], rank 1 < 2, nullspace dim 1.
    let rep = analyze_identifiability(&IdentifiabilityRequest {
        parameter_names: vec!["b_gc".to_string(), "b_mill".to_string()],
        prior_means: vec![0.0, 0.0],
        prior_covariance: vec![0.0025, 0.0, 0.0, 0.0025], // sd = 0.05 each
        observation_design: vec![1.0, -1.0],
        observed: vec![-0.096],
        obs_sds: vec![0.015],
        confidence_level: 0.95,
    })
    .expect("GB-25 identifiability");
    assert_eq!(rep.observation_rank, 1);
    assert_eq!(rep.nullspace_dimension, 1);
    assert!(!rep.is_fully_identifiable);
    assert_eq!(rep.confounded_pairs.len(), 1);
    assert!(rep.confounded_pairs[0].rank_deficient_split);
    assert!((rep.confounded_pairs[0].correlation - 0.917_431).abs() < 0.005);
    assert_eq!(rep.identifiable_contrasts.len(), 1);
    assert_eq!(rep.nullspace_directions.len(), 1);
    // Contrast c = [1/sqrt(2), -1/sqrt(2)] -> c^T b = (b_gc - b_mill) / sqrt(2).
    let inv_s2 = 1.0 / det::sqrt(2.0);
    assert!((rep.identifiable_contrasts[0].weights[0].abs() - inv_s2).abs() < 1e-12);
    assert!((rep.identifiable_contrasts[0].weights[1].abs() - inv_s2).abs() < 1e-12);
    assert!(
        (rep.identifiable_contrasts[0].weights[0] * rep.identifiable_contrasts[0].weights[1]) < 0.0
    );
    // Nullspace direction v = [1/sqrt(2), 1/sqrt(2)] satisfies H v = 0.
    let h_dot_v = rep.nullspace_directions[0][0] - rep.nullspace_directions[0][1];
    assert!(h_dot_v.abs() < 1e-12);

    let err = require_full_identifiability(&rep).expect_err("must refuse FMINE-REF-104");
    assert_eq!(
        err.to_refusal().code,
        RefusalCode::NonIdentifiableAttribution104.as_str()
    );

    // 2. Measurement VOI (§20.6): stopped-belt sampler audit resolves identifiability with positive
    //    net VOI and outranks a redundant repeat F2 ratio.
    let voi = evaluate_measurement_voi(&MeasurementVoiRequest {
        baseline: &rep,
        target_indices: vec![0, 1],
        unit_error_value_usd: 50_000_000.0, // $50M/yr revenue exposure
        candidates: vec![
            CandidateMeasurement {
                id: "repeat_f2_ratio".to_string(),
                kind: CandidateMeasurementKind::CheckAssayRoundRobin,
                description: "Another F2 difference measurement along [1, -1]".to_string(),
                design_row: vec![1.0, -1.0],
                obs_sd: 0.015,
                cost_usd: 10_000.0,
            },
            CandidateMeasurement {
                id: "mill_stopped_belt_audit".to_string(),
                kind: CandidateMeasurementKind::SamplerAudit,
                description: "Gy stopped-belt bias test on mill head sampler".to_string(),
                design_row: vec![0.0, 1.0],
                obs_sd: 0.02,
                cost_usd: 35_000.0,
            },
        ],
    })
    .expect("VOI evaluation");
    assert_eq!(voi[0].id, "mill_stopped_belt_audit");
    assert!(voi[0].resolves_identifiability);
    assert_eq!(voi[0].post_rank, 2);
    assert!(voi[0].max_abs_corr_after <= 0.80);
    assert!(voi[0].net_voi_usd > 500_000.0);
    assert!(!voi[1].resolves_identifiability);
    assert_eq!(voi[1].post_rank, 1);

    // 3. Partial identifiability + BD-152 -> BD-153 bridge:
    //    (b0, b1) confounded via b0 - b1, while standalone b2 is directly observed.
    let hier = fit_hierarchical_bias_posteriors(&HierarchicalBiasRequest {
        evidence_cut_id: "cut-partial-id".to_string(),
        surprise_alarm_active: false,
        confidence_level: 0.90,
        groups: Vec::new(),
        nodes: vec![
            NodeBiasSpec {
                node_id: "b0".to_string(),
                source: MiningSourceKind::OpenPit,
                target: BiasTargetKind::InSituMetal,
                group_id: None,
                standalone_prior: Some(StandaloneNodePrior {
                    mean: 0.0,
                    std_dev: 0.05,
                }),
                refclass_id: None,
            },
            NodeBiasSpec {
                node_id: "b1".to_string(),
                source: MiningSourceKind::Plant,
                target: BiasTargetKind::MillSampler,
                group_id: None,
                standalone_prior: Some(StandaloneNodePrior {
                    mean: 0.0,
                    std_dev: 0.05,
                }),
                refclass_id: None,
            },
            NodeBiasSpec {
                node_id: "b2".to_string(),
                source: MiningSourceKind::Plant,
                target: BiasTargetKind::Moisture,
                group_id: None,
                standalone_prior: Some(StandaloneNodePrior {
                    mean: 0.0,
                    std_dev: 0.05,
                }),
                refclass_id: None,
            },
        ],
        observations: vec![
            BiasObservation {
                label: "f2_diff".to_string(),
                period_index: 0,
                terms: vec![("b0".to_string(), 1.0), ("b1".to_string(), -1.0)],
                observed: -0.06,
                obs_sd: 0.01,
            },
            BiasObservation {
                label: "moisture_audit".to_string(),
                period_index: 0,
                terms: vec![("b2".to_string(), 1.0)],
                observed: 0.03,
                obs_sd: 0.008,
            },
        ],
    })
    .expect("hierarchical partial id fit");
    let h_id = analyze_hierarchical_identifiability(&hier).expect("hier identifiability");
    assert_eq!(h_id.observation_rank, 2);
    assert_eq!(h_id.nullspace_dimension, 1);
    assert!(!h_id.is_fully_identifiable);
    assert!(h_id.is_parameter_identifiable("b2"));
    assert!(!h_id.is_parameter_identifiable("b0"));
    assert!(!h_id.is_parameter_identifiable("b1"));
    assert!(require_parameter_identifiable(&h_id, "b2").is_ok());
    assert_eq!(
        require_parameter_identifiable(&h_id, "b0")
            .expect_err("b0 confounded")
            .to_refusal()
            .code,
        RefusalCode::NonIdentifiableAttribution104.as_str()
    );
}

#[test]
fn test_bd156_reconciliation_certificate_roundtrip_and_guards() {
    let input_bytes = b"period=2026Q1,feed=1000000,mill=960000,sp=25000\n";
    let base_req = ReconciliationCertificateRequest {
        period_id: "2026Q1".to_string(),
        site_id: "synthetic_cu_au_01".to_string(),
        evidence_cut_id: "cut-2026Q1-final".to_string(),
        error_model_version: "err-model-v1.2".to_string(),
        code_revision: "7ccdef62".to_string(),
        determinism_class: DeterminismClass::DeterministicPerIsa,
        seed_root: 202_6156,
        ledger_anchor: None,
        inputs: vec![InputDigest::new(
            "measurements",
            "period_2026Q1.csv",
            sha256(input_bytes),
        )],
        parity_notes: vec![ParityExplanation {
            metric_id: "F2_Au".to_string(),
            site_value: 0.904,
            recomputed_value: 0.9042,
            status: ParityDifferenceStatus::Explained,
            rationale: "0.0002 rounding to 3 decimals in site monthly pack".to_string(),
        }],
        balance: BalanceCertificateSummary {
            node_residuals: vec![NodeBalanceRecord {
                node_label: "node:rom:dry_mass".to_string(),
                raw_residual: 15_000.0,
                reconciled_residual: 0.0,
            }],
            chi_square: 0.1849,
            degrees_of_freedom: 1,
            global_p_value: 0.6672,
            max_relative_closure_error: 1.2e-16,
            disposition: CommitDisposition::Admit,
            flagged_suspects: Vec::new(),
        },
        posteriors: PosteriorCertificateSummary {
            node_quantities: vec![CertifiedQuantityInterval {
                id: "stream:mill_feed:dry_mass".to_string(),
                mean: 961_135.76,
                std_dev: 9_192.0,
                ci_lower: 946_016.0,
                ci_upper: 976_255.0,
            }],
            f_factors: vec![CertifiedFactorInterval {
                registry_key: "site:demo:F2_au_metal".to_string(),
                definition_version: "v2.0".to_string(),
                mean: 0.9042,
                ci_lower: 0.879,
                ci_upper: 0.929,
            }],
            bias_terms: vec![CertifiedQuantityInterval {
                id: "bias:mill_sampler".to_string(),
                mean: 0.018,
                std_dev: 0.018,
                ci_lower: -0.012,
                ci_upper: 0.048,
            }],
            latent_inventories: vec![CertifiedQuantityInterval {
                id: "inv:rom_pad:dry_mass".to_string(),
                mean: 27_772.85,
                std_dev: 13_416.0,
                ci_lower: 5_705.0,
                ci_upper: 49_840.0,
            }],
        },
        identifiability: IdentifiabilityCertificateSummary {
            observation_rank: 1,
            parameter_count: 2,
            refused_pairs: vec![RefusedPairVoiRecord {
                first: "b_gc".to_string(),
                second: "b_mill".to_string(),
                correlation: 0.917,
                determined_contrast: "+0.7071*b_gc -0.7071*b_mill".to_string(),
                separating_measurement: "mill_stopped_belt_audit".to_string(),
                net_voi_usd: 745_000.0,
            }],
        },
        forecast_scores: vec![RegisteredForecastScore {
            forecast_id: "fcst-2026Q1-au".to_string(),
            forecast_digest_hex: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                .to_string(),
            realized_mean: 33_991.0,
            interval_covered: true,
            crps: 412.5,
        }],
        attestation: AttestationStatus::ReviewedAgainst {
            reviewer_role: "Site Metallurgical Accountant".to_string(),
            against_digest_hex: "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
                .to_string(),
        },
        require_admitted_balance: true,
    };

    let cert_a = ReconciliationCertificate::build(base_req.clone()).expect("build cert A");
    let cert_b = ReconciliationCertificate::build(base_req.clone()).expect("build cert B");
    assert_eq!(cert_a.replay_digest_blake3, cert_b.replay_digest_blake3);
    assert_eq!(cert_a.payload_sha256, cert_b.payload_sha256);
    assert!(!cert_a.reserve_disclosure_claim());
    assert!(!cert_a.execution_authorized());
    assert!(!cert_a.geotechnical_signoff());
    assert!(!cert_a.regulatory_compliance_claim());

    let bytes = cert_a.to_bytes();
    let decoded = ReconciliationCertificate::from_bytes(&bytes).expect("round-trip decode");
    assert_eq!(decoded, cert_a);

    // FMT-CERT-1 conversion round-trip.
    let fmt_cert = cert_a.to_fmt_cert_1().expect("to FMT-CERT-1");
    assert_eq!(fmt_cert.output_digest, cert_a.payload_sha256);
    let fmt_bytes = fmt_cert.to_bytes().expect("FMT-CERT-1 encode");
    let fmt_decoded =
        fmine_core::cert::Certificate::from_bytes(&fmt_bytes).expect("FMT-CERT-1 decode");
    assert_eq!(fmt_decoded, fmt_cert);

    // Single-bit tamper refuses with FMINE-REF-505.
    let mut bad_bytes = bytes;
    let mid = bad_bytes.len() / 2;
    bad_bytes[mid] ^= 0x01;
    let tamper_err =
        ReconciliationCertificate::from_bytes(&bad_bytes).expect_err("tamper must refuse");
    assert_eq!(
        tamper_err.to_refusal().code,
        RefusalCode::DigestMismatch505.as_str()
    );

    // Guard G-TOUCH-4 refuses empty F-factor definition version (FMINE-REF-107).
    let mut bad_ff = base_req.clone();
    bad_ff.posteriors.f_factors[0].definition_version = String::new();
    let g4 = ReconciliationCertificate::build(bad_ff).expect_err("G-TOUCH-4");
    assert_eq!(
        g4.to_refusal().code,
        RefusalCode::UndeclaredUnitBasisOrTz107.as_str()
    );

    // Guard INV-005 refuses sealing a committed certificate when balance disposition is refused.
    let mut bad_disp = base_req;
    bad_disp.balance.disposition = CommitDisposition::RefuseForInvestigation;
    let g_inv5 = ReconciliationCertificate::build(bad_disp).expect_err("INV-005");
    assert_eq!(
        g_inv5.to_refusal().code,
        RefusalCode::Conservation101.as_str()
    );
}

#[test]
fn test_fresh_eyes_hardening_guards_and_large_lambda_power() {
    // 1. G-TOUCH-3 must refuse missing InventoryDelta on ANY holds_inventory == true interior node,
    //    even if its class is PlantUnit (e.g. CIL leach tank) rather than StockpileOrPad!
    let cil_graph = FlowGraph::build(
        vec![
            FlowNode::new("feed_in", "Feed", NodeClass::Transport, false, None).expect("n"),
            FlowNode::new(
                "cil_tank",
                "CIL Circuit",
                NodeClass::PlantUnit,
                true, // holds_inventory = true!
                Some(StockpileMixingModel::WellMixed),
            )
            .expect("n"),
            FlowNode::new("dore_out", "Dore", NodeClass::ProductMarket, false, None).expect("n"),
        ],
        vec![
            FlowStream::new("in_s", "feed_in", "cil_tank", false, ["Au"]).expect("s"),
            FlowStream::new("out_s", "cil_tank", "dore_out", false, ["Au"]).expect("s"),
        ],
    )
    .expect("cil graph");
    let ch = ExtensiveChannel::DryMass;
    let err_lin = reconcile_flow_graph(&LinearFlowRequest {
        graph: &cil_graph,
        channel: ch.clone(),
        measurements: vec![
            MeasuredVariable {
                key: VariableKey::Stream {
                    stream_id: "in_s".to_string(),
                    channel: ch.clone(),
                },
                observed: 100.0,
                std_dev: 1.0,
            },
            MeasuredVariable {
                key: VariableKey::Stream {
                    stream_id: "out_s".to_string(),
                    channel: ch.clone(),
                },
                observed: 98.0,
                std_dev: 1.0,
            },
        ],
        off_diagonal_covariances: Vec::new(),
        enforce_non_negative_streams: true,
    })
    .expect_err("holds_inventory PlantUnit without InventoryDelta must refuse");
    assert_eq!(
        err_lin.to_refusal().code,
        RefusalCode::SupportMismatch102.as_str()
    );

    // 2. Canceling terms [(x, +1), (x, -1)] in reconcile_named_system must refuse as all-zero row.
    let k_x = VariableKey::Named {
        name: "x".to_string(),
        channel: ch.clone(),
    };
    let cancel_err = reconcile_named_system(&NamedLinearRequest {
        measurements: vec![MeasuredVariable {
            key: k_x.clone(),
            observed: 100.0,
            std_dev: 1.0,
        }],
        off_diagonal_covariances: Vec::new(),
        constraints: vec![NamedLinearConstraint {
            label: "canceling".to_string(),
            terms: vec![(k_x.clone(), 1.0), (k_x, -1.0)],
            target: 0.0,
        }],
    })
    .expect_err("canceling terms must refuse");
    assert_eq!(
        cancel_err.to_refusal().code,
        RefusalCode::OutsidePhysicalDomain109.as_str()
    );

    // 3. GrossErrorPolicy refuses target_power <= alpha.
    let bad_policy = GrossErrorPolicy {
        global_alpha: 0.05,
        measurement_alpha: 0.10,
        target_power: 0.08,
        ..GrossErrorPolicy::default()
    };
    assert_eq!(
        bad_policy
            .validate()
            .expect_err("target_power <= measurement_alpha")
            .to_refusal()
            .code,
        RefusalCode::OutsidePhysicalDomain109.as_str()
    );

    // 4. Huge bias (10,000 sigma -> lambda ~ 1e8) in analyze_bias_power must not truncate or
    //    overflow; Sankaran 3-moment branch returns exact 1.0.
    let g = build_concentrator_graph().expect("concentrator");
    let feed_k = VariableKey::Stream {
        stream_id: "feed".to_string(),
        channel: ch.clone(),
    };
    let rec = reconcile_flow_graph(&LinearFlowRequest {
        graph: &g,
        channel: ch.clone(),
        measurements: vec![
            MeasuredVariable {
                key: feed_k.clone(),
                observed: 1000.0,
                std_dev: 10.0,
            },
            MeasuredVariable {
                key: VariableKey::Stream {
                    stream_id: "conc".to_string(),
                    channel: ch.clone(),
                },
                observed: 100.0,
                std_dev: 1.0,
            },
            MeasuredVariable {
                key: VariableKey::Stream {
                    stream_id: "tails".to_string(),
                    channel: ch,
                },
                observed: 900.0,
                std_dev: 10.0,
            },
        ],
        off_diagonal_covariances: Vec::new(),
        enforce_non_negative_streams: true,
    })
    .expect("conc rec");
    let huge = analyze_bias_power(
        &rec,
        &GrossErrorPolicy::default(),
        &BiasVectorSpec {
            biases: vec![(feed_k, 1.0e6)], // ~70,000 sigma -> lambda ~ 5e9!
            periods: 3,
        },
    )
    .expect("large lambda power");
    assert!(huge.global_non_centrality > 1.0e8);
    assert!(
        (huge.global_power - 1.0).abs() < 1e-12,
        "large lambda global power must be 1.0, got {}",
        huge.global_power
    );
}

#[test]
fn test_fresh_eyes_pass2_bias_identifiability_and_certificate_hardening() {
    // 1. Huge obs_sd = 1e200 must refuse precision underflow rather than silently dropping the obs.
    let huge_sd_err = fit_hierarchical_bias_posteriors(&HierarchicalBiasRequest {
        evidence_cut_id: "cut-huge-sd".to_string(),
        confidence_level: 0.90,
        surprise_alarm_active: false,
        groups: Vec::new(),
        nodes: vec![NodeBiasSpec {
            node_id: "n0".to_string(),
            source: MiningSourceKind::Plant,
            target: BiasTargetKind::MillSampler,
            group_id: None,
            standalone_prior: Some(StandaloneNodePrior {
                mean: 0.0,
                std_dev: 0.05,
            }),
            refclass_id: None,
        }],
        observations: vec![BiasObservation {
            label: "underflow_obs".to_string(),
            period_index: 0,
            terms: vec![("n0".to_string(), 1.0)],
            observed: 0.02,
            obs_sd: 1.0e200,
        }],
    })
    .expect_err("obs_sd = 1e200 must refuse rather than silently zeroing precision");
    assert_eq!(
        huge_sd_err.to_refusal().code,
        RefusalCode::NumericalCertificateFailed207.as_str()
    );

    // 2. Pooling a Moisture node into an InSituMetal group must refuse (target mismatch).
    let mismatch_err = fit_hierarchical_bias_posteriors(&HierarchicalBiasRequest {
        evidence_cut_id: "cut-mismatch".to_string(),
        confidence_level: 0.90,
        surprise_alarm_active: false,
        groups: vec![HierarchicalGroupPrior {
            group_id: "pit_metal".to_string(),
            source: MiningSourceKind::OpenPit,
            target: BiasTargetKind::InSituMetal,
            hyper_mean: 0.0,
            hyper_sd: 0.05,
            between_node_sd: 0.02,
            refclass_id: None,
        }],
        nodes: vec![NodeBiasSpec {
            node_id: "bad_moisture".to_string(),
            source: MiningSourceKind::OpenPit,
            target: BiasTargetKind::Moisture,
            group_id: Some("pit_metal".to_string()),
            standalone_prior: None,
            refclass_id: None,
        }],
        observations: Vec::new(),
    })
    .expect_err("mismatched target in pooling group must refuse");
    assert_eq!(
        mismatch_err.to_refusal().code,
        RefusalCode::OutsidePhysicalDomain109.as_str()
    );

    // 3. Asymmetric prior_covariance in analyze_identifiability must refuse.
    let asym_err = analyze_identifiability(&IdentifiabilityRequest {
        parameter_names: vec!["a".to_string(), "b".to_string()],
        prior_means: vec![0.0, 0.0],
        prior_covariance: vec![0.04, 0.01, -0.01, 0.04],
        observation_design: vec![1.0, -1.0],
        observed: vec![0.02],
        obs_sds: vec![0.01],
        confidence_level: 0.90,
    })
    .expect_err("asymmetric prior_covariance must refuse");
    assert_eq!(
        asym_err.to_refusal().code,
        RefusalCode::OutsidePhysicalDomain109.as_str()
    );

    // 4. Duplicate target_indices in evaluate_measurement_voi must refuse (prevents VOI double-counting).
    let base_id = analyze_identifiability(&IdentifiabilityRequest {
        parameter_names: vec!["a".to_string(), "b".to_string()],
        prior_means: vec![0.0, 0.0],
        prior_covariance: vec![0.04, 0.0, 0.0, 0.04],
        observation_design: vec![1.0, -1.0],
        observed: vec![0.02],
        obs_sds: vec![0.01],
        confidence_level: 0.90,
    })
    .expect("baseline identifiability");
    let dup_voi_err = evaluate_measurement_voi(&MeasurementVoiRequest {
        baseline: &base_id,
        unit_error_value_usd: 1_000_000.0,
        target_indices: vec![0, 0],
        candidates: vec![CandidateMeasurement {
            id: "audit_a".to_string(),
            kind: CandidateMeasurementKind::SamplerAudit,
            description: "direct audit of a".to_string(),
            design_row: vec![1.0, 0.0],
            obs_sd: 0.01,
            cost_usd: 10_000.0,
        }],
    })
    .expect_err("duplicate target_indices must refuse");
    assert_eq!(
        dup_voi_err.to_refusal().code,
        RefusalCode::OutsidePhysicalDomain109.as_str()
    );

    // 5. ReconciliationCertificate refuses invalid identifiability (rank > parameter_count),
    //    NaN parity values, and crafted valid-checksum frames that violate domain invariants on decode.
    let valid_req = ReconciliationCertificateRequest {
        period_id: "2026Q2".to_string(),
        site_id: "site_x".to_string(),
        evidence_cut_id: "cut-2".to_string(),
        error_model_version: "v1".to_string(),
        code_revision: "72b15a48".to_string(),
        determinism_class: DeterminismClass::DeterministicPerIsa,
        seed_root: 1,
        ledger_anchor: None,
        inputs: vec![InputDigest::new("raw", "a.csv", sha256(b"x"))],
        parity_notes: vec![ParityExplanation {
            metric_id: "F2".to_string(),
            site_value: 0.95,
            recomputed_value: 0.95,
            status: ParityDifferenceStatus::Explained,
            rationale: "matched".to_string(),
        }],
        balance: BalanceCertificateSummary {
            node_residuals: vec![NodeBalanceRecord {
                node_label: "rom".to_string(),
                raw_residual: 1.0,
                reconciled_residual: 0.0,
            }],
            chi_square: 0.5,
            degrees_of_freedom: 1,
            global_p_value: 0.48,
            max_relative_closure_error: 1e-15,
            disposition: CommitDisposition::Admit,
            flagged_suspects: Vec::new(),
        },
        posteriors: PosteriorCertificateSummary {
            node_quantities: Vec::new(),
            f_factors: Vec::new(),
            bias_terms: Vec::new(),
            latent_inventories: Vec::new(),
        },
        identifiability: IdentifiabilityCertificateSummary {
            observation_rank: 3,
            parameter_count: 2,
            refused_pairs: Vec::new(),
        },
        forecast_scores: Vec::new(),
        attestation: AttestationStatus::DraftUnreviewed,
        require_admitted_balance: true,
    };
    assert_eq!(
        ReconciliationCertificate::build(valid_req.clone())
            .expect_err("rank > parameter_count must refuse")
            .to_refusal()
            .code,
        RefusalCode::OutsidePhysicalDomain109.as_str()
    );
    let mut bad_parity = valid_req;
    bad_parity.identifiability.observation_rank = 2;
    bad_parity.parity_notes[0].site_value = f64::NAN;
    assert_eq!(
        ReconciliationCertificate::build(bad_parity)
            .expect_err("NaN parity note must refuse")
            .to_refusal()
            .code,
        RefusalCode::OutsidePhysicalDomain109.as_str()
    );
}
