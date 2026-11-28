//! `GATE-TOUCH-02` M1 Kill Gate (`BD-158`), Quarterly 5% Sampler Bias Detection Power (`BD-159`),
//! Hierarchical Bias SBC (`BD-152`), Identifiability & VOI (`BD-153`), and `ReconciliationCertificate`
//! offline `fmine-verify` round-trip (`BD-156`) (plan §20.5–§20.11, `PO-TOUCH-5`, `PO-TOUCH-6`).
//!
//! Verifies all four plan §20.10 `GATE-TOUCH-02` acceptance clauses:
//! 1. Over `N = 400` (`>= 200`) seeded replicates spanning all six mandatory `BD-130` bias families
//!    (`resource_model`, `dilution`, `survey`, `sampler`, `density`, `moisture`) plus a hierarchical
//!    partially-pooled 2-domain group (`BD-152`), empirical 90% credible interval coverage on every
//!    injected bias lies strictly in **`[85%, 95%]`** AND `fmine-gen::sbc` rank histograms pass
//!    (`SbcDiagnosis::Calibrated`).
//! 2. Metal balance closes within stated uncertainty (`PO-TOUCH-1` `< 1e-10`) on 24 monthly periods
//!    from `fmine-gen::mill_balance` (`BD-133`), with reconciled Cu metal error calibrated to its
//!    posterior uncertainty.
//! 3. All four confounded bias pairs declared in `registries/biases.toml` refuse individual
//!    attribution with `FMINE-REF-104` when their separating measurement is absent, while publishing
//!    determined contrasts and Measurement VOI rankings (`BD-153`).
//! 4. Quarterly detection power for a **5% mill head sampler bias** (`BD-159`) is computed
//!    analytically and cross-checked by 2,000-replicate Monte Carlo across realistic assay noise
//!    regimes (`1.5%`, `2.0%`, `3.0%` monthly SD).
//! 5. `ReconciliationCertificate` (`BD-156`) round-trips cleanly through `fmine-verify cert`
//!    (`FMT-CERT-1` + on-disk input closure) with exit code `0`, and exits `8` (`FMINE-REF-505`) on
//!    single-bit tamper of the certificate, input file, or `RCRT` frame.

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use fmine_core::cert::{DeterminismClass, InputDigest};
use fmine_core::codec::Encoder;
use fmine_core::det;
use fmine_core::hash::{blake3_hash, sha256};
use fmine_core::rand::{PhiloxStream, RngKey};
use fmine_evidence::RefusalCode;
use fmine_flow::graph::{FlowGraph, FlowNode, FlowStream, NodeClass};
use fmine_gen::biases::{
    BiasFamily, SeededBiasProfile, TrueOreLot, apply_seeded_biases, parse_bias_registry,
};
use fmine_gen::harness::ScoringHarness;
use fmine_gen::mill_balance::MillBalanceConfig;
use fmine_gen::sbc::{SbcConfig, SbcDiagnosis, SbcReplicate, evaluate_sbc_replicates};
use fmine_recon::bias::{
    BiasObservation, BiasTargetKind, HierarchicalBiasRequest, HierarchicalGroupPrior,
    MiningSourceKind, NodeBiasSpec, PosteriorSampleRequest, StandaloneNodePrior,
    fit_hierarchical_bias_posteriors, sample_hierarchical_posterior,
};
use fmine_recon::bilinear::{
    BilinearFlowRequest, BilinearStreamObservation, GradeValue, ObservedGrade, ObservedScalar,
    reconcile_bilinear_flow,
};
use fmine_recon::certificate::{
    AttestationStatus, BalanceCertificateSummary, CertifiedFactorInterval,
    CertifiedQuantityInterval, IdentifiabilityCertificateSummary, NodeBalanceRecord,
    ParityDifferenceStatus, ParityExplanation, PosteriorCertificateSummary,
    ReconciliationCertificate, ReconciliationCertificateRequest, RegisteredForecastScore,
};
use fmine_recon::gross_error::{
    BiasVectorSpec, CommitDisposition, GrossErrorPolicy, MultiplicityCorrection,
    analyze_bias_power, evaluate_gross_errors,
};
use fmine_recon::identifiability::{
    CandidateMeasurement, CandidateMeasurementKind, IdentifiabilityRequest, MeasurementVoiRequest,
    analyze_identifiability, evaluate_measurement_voi, require_full_identifiability,
};
use fmine_recon::linear::{
    ExtensiveChannel, LinearFlowRequest, MeasuredVariable, VariableKey, reconcile_flow_graph,
};
use fmine_verify::checks::{verify_certificate_bytes, verify_certificate_file};
use fmine_verify::run_cli;

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

struct TempDirGuard(PathBuf);

impl Drop for TempDirGuard {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn test_gate_touch_02_bias_recovery_sbc_identifiability_and_5pct_power() {
    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("workspace root")
        .to_path_buf();
    let bias_toml =
        fs::read_to_string(workspace_root.join("registries/biases.toml")).expect("biases.toml");
    let bias_reg = parse_bias_registry(&bias_toml).expect("parse bias registry");
    for fam in BiasFamily::REQUIRED_BD_130 {
        assert!(bias_reg.get(fam).is_some(), "missing BD-130 family {fam:?}");
    }

    // Verify deterministic BD-130 bias injection operator on a physical lot.
    let lot_obs = apply_seeded_biases(
        TrueOreLot {
            volume_m3: 40_000.0,
            true_density_t_m3: 2.65,
            true_moisture_fraction: 0.08,
            true_grade_g_per_t: 2.40,
        },
        SeededBiasProfile {
            resource_model: 1.06,
            dilution: 1.04,
            survey: 0.96,
            sampler: 0.95, // -5% sampler bias
            density: 1.02,
            moisture: 1.08,
            truck_payload: 1.03,
        },
        &bias_reg,
    )
    .expect("apply BD-130 biases");
    assert!((lot_obs.sampler_grade_g_per_t - 2.40 * 0.95).abs() < 1e-12);

    let mut gate_enc = Encoder::new_frame(*b"GT02", 1);

    // =========================================================================
    // Part A: GATE-TOUCH-02 + BD-152 SBC across N = 400 replicates (>= 200)
    // All 6 mandatory BD-130 bias families + hierarchical 2-domain partial pooling.
    // =========================================================================
    const N_REPS: usize = 400;
    const L_DRAWS: usize = 99;
    const HYPER_SD: f64 = 0.030;
    const TAU_SD: f64 = 0.015;
    const STANDALONE_SD: f64 = 0.030;
    const OBS_SD_RATIO: f64 = 0.012;
    const OBS_SD_AUDIT: f64 = 0.015;

    let tracked_params = [
        "site_grade_group",
        "domain_a_rm",
        "domain_b_rm",
        "dilution",
        "survey",
        "sampler",
        "density",
        "moisture",
    ];
    let mut analytical_cov90_hits: BTreeMap<&str, usize> =
        tracked_params.iter().map(|&p| (p, 0usize)).collect();
    let mut sbc_reps: BTreeMap<&str, Vec<SbcReplicate>> = tracked_params
        .iter()
        .map(|&p| (p, Vec::with_capacity(N_REPS)))
        .collect();

    let mut rng_gen = PhiloxStream::new(RngKey {
        seed_root: 202_6158,
        partition: 1,
        domain: 1,
        var_group: 1,
        realization: 0,
        brick: 0,
    });

    for rep in 0..N_REPS {
        // Draw exact generative prior truth theta* ~ p(theta).
        let mu_site = HYPER_SD * next_std_normal(&mut rng_gen);
        let b_dom_a = mu_site + TAU_SD * next_std_normal(&mut rng_gen);
        let b_dom_b = mu_site + TAU_SD * next_std_normal(&mut rng_gen);
        let b_dil = STANDALONE_SD * next_std_normal(&mut rng_gen);
        let b_sur = STANDALONE_SD * next_std_normal(&mut rng_gen);
        let b_sam = STANDALONE_SD * next_std_normal(&mut rng_gen);
        let b_den = STANDALONE_SD * next_std_normal(&mut rng_gen);
        let b_moi = STANDALONE_SD * next_std_normal(&mut rng_gen);

        let truth_map = [
            ("site_grade_group", mu_site),
            ("domain_a_rm", b_dom_a),
            ("domain_b_rm", b_dom_b),
            ("dilution", b_dil),
            ("survey", b_sur),
            ("sampler", b_sam),
            ("density", b_den),
            ("moisture", b_moi),
        ];

        // Simulate 3 monthly periods of coupled cross-stage ratios + separating audits so all
        // 6 BD-130 families and both hierarchical domains are jointly observed.
        let mut observations = Vec::with_capacity(21);
        for period in 0..3u32 {
            // F1 grade-control vs resource + dilution across domains A & B.
            observations.push(BiasObservation {
                label: format!("f1_dom_a_p{period}"),
                period_index: period,
                terms: vec![
                    ("domain_a_rm".to_string(), 1.0),
                    ("dilution".to_string(), 1.0),
                ],
                observed: b_dom_a + b_dil + OBS_SD_RATIO * next_std_normal(&mut rng_gen),
                obs_sd: OBS_SD_RATIO,
            });
            observations.push(BiasObservation {
                label: format!("f1_dom_b_p{period}"),
                period_index: period,
                terms: vec![
                    ("domain_b_rm".to_string(), 1.0),
                    ("dilution".to_string(), 1.0),
                ],
                observed: b_dom_b + b_dil + OBS_SD_RATIO * next_std_normal(&mut rng_gen),
                obs_sd: OBS_SD_RATIO,
            });
            // F2 resource vs mill head sampler.
            observations.push(BiasObservation {
                label: format!("f2_sampler_p{period}"),
                period_index: period,
                terms: vec![
                    ("domain_a_rm".to_string(), 1.0),
                    ("sampler".to_string(), -1.0),
                ],
                observed: b_dom_a - b_sam + OBS_SD_RATIO * next_std_normal(&mut rng_gen),
                obs_sd: OBS_SD_RATIO,
            });
            // Stockpile dry tonnage balance: survey + density.
            observations.push(BiasObservation {
                label: format!("stockpile_tonnes_p{period}"),
                period_index: period,
                terms: vec![("survey".to_string(), 1.0), ("density".to_string(), 1.0)],
                observed: b_sur + b_den + OBS_SD_RATIO * next_std_normal(&mut rng_gen),
                obs_sd: OBS_SD_RATIO,
            });
            // Oven-dry moisture audit.
            observations.push(BiasObservation {
                label: format!("moisture_audit_p{period}"),
                period_index: period,
                terms: vec![("moisture".to_string(), 1.0)],
                observed: b_moi + OBS_SD_AUDIT * next_std_normal(&mut rng_gen),
                obs_sd: OBS_SD_AUDIT,
            });
        }
        // Separating quarterly audits (blast-movement pickup, stopped-belt Gy test, core SG wax).
        observations.push(BiasObservation {
            label: "bmm_pickup_audit".to_string(),
            period_index: 2,
            terms: vec![("dilution".to_string(), 1.0)],
            observed: b_dil + OBS_SD_AUDIT * next_std_normal(&mut rng_gen),
            obs_sd: OBS_SD_AUDIT,
        });
        observations.push(BiasObservation {
            label: "gy_stopped_belt_audit".to_string(),
            period_index: 2,
            terms: vec![("sampler".to_string(), 1.0)],
            observed: b_sam + OBS_SD_AUDIT * next_std_normal(&mut rng_gen),
            obs_sd: OBS_SD_AUDIT,
        });
        observations.push(BiasObservation {
            label: "core_sg_wax_audit".to_string(),
            period_index: 2,
            terms: vec![("density".to_string(), 1.0)],
            observed: b_den + OBS_SD_AUDIT * next_std_normal(&mut rng_gen),
            obs_sd: OBS_SD_AUDIT,
        });

        let fit = fit_hierarchical_bias_posteriors(&HierarchicalBiasRequest {
            evidence_cut_id: format!("gate-touch-02-rep-{rep}"),
            surprise_alarm_active: false,
            confidence_level: 0.90,
            groups: vec![HierarchicalGroupPrior {
                group_id: "site_grade_group".to_string(),
                source: MiningSourceKind::OpenPit,
                target: BiasTargetKind::InSituMetal,
                hyper_mean: 0.0,
                hyper_sd: HYPER_SD,
                between_node_sd: TAU_SD,
                refclass_id: None,
            }],
            nodes: vec![
                NodeBiasSpec {
                    node_id: "domain_a_rm".to_string(),
                    source: MiningSourceKind::OpenPit,
                    target: BiasTargetKind::InSituMetal,
                    group_id: Some("site_grade_group".to_string()),
                    standalone_prior: None,
                    refclass_id: None,
                },
                NodeBiasSpec {
                    node_id: "domain_b_rm".to_string(),
                    source: MiningSourceKind::OpenPit,
                    target: BiasTargetKind::InSituMetal,
                    group_id: Some("site_grade_group".to_string()),
                    standalone_prior: None,
                    refclass_id: None,
                },
                NodeBiasSpec {
                    node_id: "dilution".to_string(),
                    source: MiningSourceKind::OpenPit,
                    target: BiasTargetKind::Dilution,
                    group_id: None,
                    standalone_prior: Some(StandaloneNodePrior {
                        mean: 0.0,
                        std_dev: STANDALONE_SD,
                    }),
                    refclass_id: None,
                },
                NodeBiasSpec {
                    node_id: "survey".to_string(),
                    source: MiningSourceKind::Stockpile,
                    target: BiasTargetKind::StockpileSurvey,
                    group_id: None,
                    standalone_prior: Some(StandaloneNodePrior {
                        mean: 0.0,
                        std_dev: STANDALONE_SD,
                    }),
                    refclass_id: None,
                },
                NodeBiasSpec {
                    node_id: "sampler".to_string(),
                    source: MiningSourceKind::Plant,
                    target: BiasTargetKind::MillSampler,
                    group_id: None,
                    standalone_prior: Some(StandaloneNodePrior {
                        mean: 0.0,
                        std_dev: STANDALONE_SD,
                    }),
                    refclass_id: None,
                },
                NodeBiasSpec {
                    node_id: "density".to_string(),
                    source: MiningSourceKind::Stockpile,
                    target: BiasTargetKind::Density,
                    group_id: None,
                    standalone_prior: Some(StandaloneNodePrior {
                        mean: 0.0,
                        std_dev: STANDALONE_SD,
                    }),
                    refclass_id: None,
                },
                NodeBiasSpec {
                    node_id: "moisture".to_string(),
                    source: MiningSourceKind::Plant,
                    target: BiasTargetKind::Moisture,
                    group_id: None,
                    standalone_prior: Some(StandaloneNodePrior {
                        mean: 0.0,
                        std_dev: STANDALONE_SD,
                    }),
                    refclass_id: None,
                },
            ],
            observations,
        })
        .expect("hierarchical bias fit");

        if rep == 0 {
            gate_enc.put_str(&fit.update_event.digest_hex);
        }

        let draws = sample_hierarchical_posterior(
            &fit,
            &PosteriorSampleRequest {
                seed: RngKey {
                    seed_root: 202_6158,
                    partition: 2,
                    domain: 1,
                    var_group: 1,
                    realization: rep as u32,
                    brick: 0,
                },
                draw_count: L_DRAWS,
            },
        )
        .expect("sample posterior draws");

        for &(name, truth_val) in &truth_map {
            let summary = fit
                .group_posterior(name)
                .or_else(|| fit.node_posterior(name))
                .expect("tracked parameter");
            if truth_val >= summary.ci_lower && truth_val <= summary.ci_upper {
                *analytical_cov90_hits.get_mut(name).expect("hit slot") += 1;
            }
            sbc_reps
                .get_mut(name)
                .expect("sbc slot")
                .push(SbcReplicate {
                    prior_draw: truth_val,
                    posterior_draws: draws.draws_for(name).expect("param draws").to_vec(),
                });
        }
    }

    let sbc_cfg = SbcConfig {
        replicates: N_REPS,
        posterior_draws: L_DRAWS,
        bins: 10,
        chi2_alpha: 0.01,
        coverage_levels: vec![0.50, 0.80, 0.90, 0.95],
        coverage_ci_alpha: 0.01,
    };
    let mut min_cov90 = 1.0_f64;
    let mut max_cov90 = 0.0_f64;
    let mut min_sbc_p = 1.0_f64;
    for &name in &tracked_params {
        let cov90 = (analytical_cov90_hits[name] as f64) / (N_REPS as f64);
        min_cov90 = min_cov90.min(cov90);
        max_cov90 = max_cov90.max(cov90);
        assert!(
            (0.85..=0.95).contains(&cov90),
            "GATE-TOUCH-02 KILL THRESHOLD VIOLATED on '{name}': 90% coverage = {cov90:.4} outside [0.85, 0.95]"
        );

        let sbc_rep =
            evaluate_sbc_replicates(&sbc_cfg, &sbc_reps[name], 202_6158).expect("SBC report");
        min_sbc_p = min_sbc_p.min(sbc_rep.chi2_p_value);
        assert!(
            sbc_rep.passed && sbc_rep.diagnosis == SbcDiagnosis::Calibrated,
            "BD-152 SBC failed on '{name}': p={:.4}, diag={:?}, hist={:?}",
            sbc_rep.chi2_p_value,
            sbc_rep.diagnosis,
            sbc_rep.rank_histogram
        );
        gate_enc.put_str(&sbc_rep.canonical_digest().to_hex());
    }

    // =========================================================================
    // Part B: Metal balance closure within stated uncertainty (mill_balance)
    // =========================================================================
    let cgraph = concentrator_graph().expect("concentrator graph");
    let harness = ScoringHarness::new(202_6158, "GATE-TOUCH-02");
    let mut mb_cfg = MillBalanceConfig::standard();
    mb_cfg.biases = SeededBiasProfile::unbiased();
    let (mb_obs, mb_seal) = harness
        .generate_mill_balance(202_6158, &mb_cfg, None)
        .expect("mill balance");
    let mb_truth = harness
        .open_mill_balance_truth(&mb_seal)
        .expect("open truth");

    let mut max_metal_closure = 0.0_f64;
    let mut metal_z_covered_95 = 0usize;
    for (p_obs, p_true) in mb_obs.periods.iter().zip(&mb_truth.periods) {
        let mut fg = BTreeMap::new();
        fg.insert(
            "Cu".to_string(),
            ObservedGrade {
                grade: GradeValue::Percent(p_obs.obs_feed_cu_pct),
                std_dev: GradeValue::Percent(p_obs.obs_feed_cu_pct * mb_cfg.assay_rel_sigma),
            },
        );
        let mut cg = BTreeMap::new();
        cg.insert(
            "Cu".to_string(),
            ObservedGrade {
                grade: GradeValue::Percent(p_obs.obs_conc_cu_pct),
                std_dev: GradeValue::Percent(p_obs.obs_conc_cu_pct * 0.25 * mb_cfg.assay_rel_sigma),
            },
        );
        let mut tg = BTreeMap::new();
        tg.insert(
            "Cu".to_string(),
            ObservedGrade {
                grade: GradeValue::Percent(p_obs.obs_tail_cu_pct),
                std_dev: GradeValue::Percent(p_obs.obs_tail_cu_pct * mb_cfg.assay_rel_sigma),
            },
        );
        let bil = reconcile_bilinear_flow(&BilinearFlowRequest {
            graph: &cgraph,
            elements: vec!["Cu".to_string()],
            stream_observations: vec![
                BilinearStreamObservation {
                    stream_id: "feed".to_string(),
                    dry_mass: Some(ObservedScalar {
                        observed: p_obs.obs_feed_dry_tonnes,
                        std_dev: p_obs.obs_feed_dry_tonnes * mb_cfg.feed_weightometer_sigma,
                    }),
                    element_grades: fg,
                },
                BilinearStreamObservation {
                    stream_id: "conc".to_string(),
                    dry_mass: Some(ObservedScalar {
                        observed: p_obs.obs_conc_dry_tonnes,
                        std_dev: p_obs.obs_conc_dry_tonnes * 0.25 * mb_cfg.feed_weightometer_sigma,
                    }),
                    element_grades: cg,
                },
                BilinearStreamObservation {
                    stream_id: "tails".to_string(),
                    dry_mass: None,
                    element_grades: tg,
                },
            ],
            observation_correlations: Vec::new(),
            max_iterations: 64,
            relative_closure_tolerance: 1e-10,
        })
        .expect("bilinear balance");
        max_metal_closure = max_metal_closure.max(bil.max_relative_closure_error);
        let feed_cu = bil.streams["feed"].elements["Cu"];
        let true_feed_cu_t = p_true.true_feed_dry_tonnes * (p_true.true_feed_cu_pct * 0.01);
        let z = (feed_cu.metal_mass - true_feed_cu_t).abs() / feed_cu.metal_mass_sd;
        if z <= 2.2 {
            metal_z_covered_95 += 1;
        }
    }
    assert!(max_metal_closure < 1e-10);
    assert!(
        metal_z_covered_95 >= 20,
        "reconciled metal must cover truth within uncertainty ({metal_z_covered_95}/24)"
    );

    // =========================================================================
    // Part C: All 4 confounded pairs in biases.toml refuse with FMINE-REF-104
    // =========================================================================
    let confounded_cases = [
        (
            "resource_model",
            "dilution",
            [1.0, 1.0],
            [0.0, 1.0],
            CandidateMeasurementKind::GradeControlInfill,
        ),
        (
            "survey",
            "density",
            [1.0, 1.0],
            [0.0, 1.0],
            CandidateMeasurementKind::DensityProgramme,
        ),
        (
            "moisture",
            "truck_payload",
            [-1.0, 1.0],
            [1.0, 0.0],
            CandidateMeasurementKind::CheckAssayRoundRobin,
        ),
        (
            "resource_model",
            "sampler",
            [1.0, -1.0],
            [0.0, 1.0],
            CandidateMeasurementKind::SamplerAudit,
        ),
    ];
    for (p1, p2, obs_row, audit_row, kind) in confounded_cases {
        let id_rep = analyze_identifiability(&IdentifiabilityRequest {
            parameter_names: vec![p1.to_string(), p2.to_string()],
            prior_means: vec![0.0, 0.0],
            prior_covariance: vec![0.0025, 0.0, 0.0, 0.0025],
            observation_design: obs_row.to_vec(),
            observed: vec![-0.07],
            obs_sds: vec![0.012],
            confidence_level: 0.90,
        })
        .expect("confounded analysis");
        assert_eq!(id_rep.observation_rank, 1);
        let ref_err = require_full_identifiability(&id_rep).expect_err("must refuse FMINE-REF-104");
        assert_eq!(
            ref_err.to_refusal().code,
            RefusalCode::NonIdentifiableAttribution104.as_str()
        );

        let voi = evaluate_measurement_voi(&MeasurementVoiRequest {
            baseline: &id_rep,
            target_indices: vec![0, 1],
            unit_error_value_usd: 40_000_000.0,
            candidates: vec![CandidateMeasurement {
                id: format!("separate_{p1}_{p2}"),
                kind,
                description: format!("Separating audit for ({p1}, {p2})"),
                design_row: audit_row.to_vec(),
                obs_sd: 0.015,
                cost_usd: 25_000.0,
            }],
        })
        .expect("VOI");
        assert!(
            voi[0].resolves_identifiability,
            "separating audit for ({p1}, {p2}) must resolve FMINE-REF-104"
        );
        assert!(voi[0].net_voi_usd > 0.0);
    }

    // =========================================================================
    // Part D: BD-159 Quarterly detection power for a 5% mill head sampler bias
    // =========================================================================
    let ch_cu = ExtensiveChannel::Element("Cu".to_string());
    let feed_key = VariableKey::Stream {
        stream_id: "feed".to_string(),
        channel: ch_cu.clone(),
    };
    let conc_key = VariableKey::Stream {
        stream_id: "conc".to_string(),
        channel: ch_cu.clone(),
    };
    let tail_key = VariableKey::Stream {
        stream_id: "tails".to_string(),
        channel: ch_cu.clone(),
    };

    // Baseline monthly Cu metal balance (100,000 t @ 1.0% = 1,000 t Cu feed; 880 t conc; 120 t tails).
    // A 5% head sampler bias shifts observed feed Cu metal by delta = 50.0 t/month.
    let policy_05 = GrossErrorPolicy {
        global_alpha: 0.05,
        measurement_alpha: 0.05,
        multiplicity: MultiplicityCorrection::Sidak,
        target_power: 0.80,
        periods_per_quarter: 3,
    };
    let mut power_5pct_results = Vec::new();
    for &rel_sigma in &[0.015_f64, 0.020, 0.030] {
        let sd_feed = 1_000.0 * rel_sigma;
        let sd_conc = 880.0 * (0.25 * rel_sigma); // settlement concentrate weigh + assay
        let sd_tail = 120.0 * rel_sigma;
        let base_rec = reconcile_flow_graph(&LinearFlowRequest {
            graph: &cgraph,
            channel: ch_cu.clone(),
            measurements: vec![
                MeasuredVariable {
                    key: feed_key.clone(),
                    observed: 1_000.0,
                    std_dev: sd_feed,
                },
                MeasuredVariable {
                    key: conc_key.clone(),
                    observed: 880.0,
                    std_dev: sd_conc,
                },
                MeasuredVariable {
                    key: tail_key.clone(),
                    observed: 120.0,
                    std_dev: sd_tail,
                },
            ],
            off_diagonal_covariances: Vec::new(),
            enforce_non_negative_streams: true,
        })
        .expect("baseline Cu balance");

        let ge = evaluate_gross_errors(&base_rec, &policy_05).expect("gross error MDB");
        let feed_mdb = ge
            .detectable_biases
            .iter()
            .find(|b| b.variable == feed_key)
            .expect("feed MDB");
        let pwr = analyze_bias_power(
            &base_rec,
            &policy_05,
            &BiasVectorSpec {
                biases: vec![(feed_key.clone(), -50.0)], // -5% sampler bias!
                periods: 3,                              // 1 quarter = 3 monthly periods
            },
        )
        .expect("5% quarterly sampler power");

        // Cross-check analytical quarterly power against 2,000-replicate Monte Carlo (quarterly mean).
        let mut rng_pwr = PhiloxStream::new(RngKey {
            seed_root: 202_6159,
            partition: (rel_sigma * 1000.0) as u32,
            domain: 1,
            var_group: 1,
            realization: 0,
            brick: 0,
        });
        let q_sd_feed = sd_feed / det::sqrt(3.0);
        let q_sd_conc = sd_conc / det::sqrt(3.0);
        let q_sd_tail = sd_tail / det::sqrt(3.0);
        let mut mc_hits = 0usize;
        let mc_reps = 2_000usize;
        for _ in 0..mc_reps {
            let rec_q = reconcile_flow_graph(&LinearFlowRequest {
                graph: &cgraph,
                channel: ch_cu.clone(),
                measurements: vec![
                    MeasuredVariable {
                        key: feed_key.clone(),
                        observed: 1_000.0 - 50.0 + q_sd_feed * next_std_normal(&mut rng_pwr),
                        std_dev: q_sd_feed,
                    },
                    MeasuredVariable {
                        key: conc_key.clone(),
                        observed: 880.0 + q_sd_conc * next_std_normal(&mut rng_pwr),
                        std_dev: q_sd_conc,
                    },
                    MeasuredVariable {
                        key: tail_key.clone(),
                        observed: 120.0 + q_sd_tail * next_std_normal(&mut rng_pwr),
                        std_dev: q_sd_tail,
                    },
                ],
                off_diagonal_covariances: Vec::new(),
                enforce_non_negative_streams: true,
            })
            .expect("mc quarterly rec");
            let ge_q = evaluate_gross_errors(&rec_q, &policy_05).expect("mc ge");
            if ge_q.global_rejected {
                mc_hits += 1;
            }
        }
        let emp_power = (mc_hits as f64) / (mc_reps as f64);
        assert!(
            (emp_power - pwr.global_power).abs() < 0.035,
            "BD-159 5% sampler bias at rel_sigma={rel_sigma}: emp={emp_power:.4} vs theory={:.4}",
            pwr.global_power
        );
        let mdb_q_pct = feed_mdb.measurement_mdb_quarter_pct.expect("mdb pct");
        power_5pct_results.push((rel_sigma, pwr.global_power, emp_power, mdb_q_pct));
        gate_enc.put_u64(pwr.global_power.to_bits());
    }
    // At nominal 1.5% and 2.0% monthly feed assay SD, a 5% sampler bias is detected at > 95% power/qtr.
    assert!(power_5pct_results[0].1 > 0.99);
    assert!(power_5pct_results[1].1 > 0.95);
    assert!(power_5pct_results[2].1 > 0.75);

    let gate_digest = blake3_hash(&gate_enc.finish()).to_hex();
    println!(
        "GATE-TOUCH-02 PASS | reps={N_REPS} | params={} | cov90=[{min_cov90:.4},{max_cov90:.4}] | min_sbc_p={min_sbc_p:.4} | max_metal_closure={max_metal_closure:.2e} | metal_cov={metal_z_covered_95}/24 | power_5pct_qtr={power_5pct_results:?} | digest={gate_digest}",
        tracked_params.len()
    );
}

#[test]
fn test_bd156_reconciliation_certificate_fmine_verify_roundtrip_and_tamper() {
    let dir = std::env::temp_dir().join(format!(
        "fmine_bd156_verify_{}_{}",
        std::process::id(),
        202_6156
    ));
    let _guard = TempDirGuard(dir.clone());
    let inputs_dir = dir.join("inputs");
    fs::create_dir_all(&inputs_dir).expect("create inputs dir");

    let meas_bytes =
        b"stream,dry_tonnes,sd\nfeed,1000000,30000\nconc,960000,9600\nsp_delta,25000,15000\n";
    let meas_path = inputs_dir.join("period_2026Q1_measurements.csv");
    fs::write(&meas_path, meas_bytes).expect("write input");

    let cert = ReconciliationCertificate::build(ReconciliationCertificateRequest {
        period_id: "2026Q1".to_string(),
        site_id: "synthetic_cu_au_01".to_string(),
        evidence_cut_id: "cut-2026Q1-close".to_string(),
        error_model_version: "err-model-v1".to_string(),
        code_revision: "7ccdef62".to_string(),
        determinism_class: DeterminismClass::DeterministicPerIsa,
        seed_root: 202_6156,
        ledger_anchor: None,
        inputs: vec![InputDigest::new(
            "measurements",
            "period_2026Q1_measurements.csv",
            sha256(meas_bytes),
        )],
        parity_notes: vec![ParityExplanation {
            metric_id: "F2_Au".to_string(),
            site_value: 0.904,
            recomputed_value: 0.904,
            status: ParityDifferenceStatus::Explained,
            rationale: "Exact match to site reconciliation pack".to_string(),
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
            max_relative_closure_error: 1.0e-16,
            disposition: CommitDisposition::Admit,
            flagged_suspects: Vec::new(),
        },
        posteriors: PosteriorCertificateSummary {
            node_quantities: vec![CertifiedQuantityInterval {
                id: "stream:mill:dry_mass".to_string(),
                mean: 961_135.76,
                std_dev: 9_192.0,
                ci_lower: 946_016.0,
                ci_upper: 976_255.0,
            }],
            f_factors: vec![CertifiedFactorInterval {
                registry_key: "site:demo:F2_au".to_string(),
                definition_version: "v1.0".to_string(),
                mean: 0.904,
                ci_lower: 0.880,
                ci_upper: 0.928,
            }],
            bias_terms: vec![CertifiedQuantityInterval {
                id: "bias:mill_sampler".to_string(),
                mean: 0.018,
                std_dev: 0.018,
                ci_lower: -0.012,
                ci_upper: 0.048,
            }],
            latent_inventories: vec![CertifiedQuantityInterval {
                id: "inv:rom:dry_mass".to_string(),
                mean: 27_772.85,
                std_dev: 13_416.0,
                ci_lower: 5_705.0,
                ci_upper: 49_840.0,
            }],
        },
        identifiability: IdentifiabilityCertificateSummary {
            observation_rank: 2,
            parameter_count: 2,
            refused_pairs: Vec::new(),
        },
        forecast_scores: vec![RegisteredForecastScore {
            forecast_id: "fcst-2026Q1".to_string(),
            forecast_digest_hex: "1111111111111111111111111111111111111111111111111111111111111111"
                .to_string(),
            realized_mean: 33_991.0,
            interval_covered: true,
            crps: 390.0,
        }],
        attestation: AttestationStatus::DraftUnreviewed,
        require_admitted_balance: true,
    })
    .expect("build ReconciliationCertificate");

    // Persist both RCRT payload and outer FMT-CERT-1 envelope.
    let rcrt_bytes = cert.to_bytes();
    let fmt_cert = cert.to_fmt_cert_1().expect("build FMT-CERT-1");
    let fmt_bytes = fmt_cert.to_bytes().expect("encode FMT-CERT-1");
    let cert_path = dir.join("recon_2026Q1.cert");
    fs::write(&cert_path, &fmt_bytes).expect("write cert file");

    // 1. Direct API round-trip via fmine-verify::checks.
    let v_mem = verify_certificate_bytes(&fmt_bytes).expect("verify bytes");
    assert_eq!(v_mem.output_digest, cert.payload_sha256);
    let v_disk =
        verify_certificate_file(&cert_path, Some(&inputs_dir)).expect("verify file + inputs");
    assert_eq!(v_disk.certificate_digest, fmt_cert.certificate_digest);

    // 2. CLI verifier round-trip (`fmine-verify cert <file> --inputs-dir <dir>`).
    let (json_ok, code_ok) = run_cli(&[
        "cert".to_string(),
        cert_path.to_string_lossy().to_string(),
        "--inputs-dir".to_string(),
        inputs_dir.to_string_lossy().to_string(),
    ]);
    assert_eq!(code_ok, 0, "valid cert must exit 0, got {json_ok}");
    assert!(json_ok.contains("\"status\":\"verified\""));
    assert!(json_ok.contains("\"subject_kind\":\"touchstone_reconciliation\""));
    assert!(json_ok.contains(&cert.payload_sha256.to_hex()));

    // 3. Single-bit tamper of FMT-CERT-1 file exits 8 (FMINE-REF-505).
    let tampered_cert_path = dir.join("recon_tampered.cert");
    let mut bad_fmt = fmt_bytes;
    let idx = bad_fmt.len() / 2;
    bad_fmt[idx] ^= 0x01;
    fs::write(&tampered_cert_path, &bad_fmt).expect("write tampered cert");
    let (json_bad, code_bad) = run_cli(&[
        "cert".to_string(),
        tampered_cert_path.to_string_lossy().to_string(),
    ]);
    assert_eq!(code_bad, 8, "tampered cert must exit 8: {json_bad}");
    assert!(json_bad.contains("FMINE-REF-505"));

    // 4. Single-bit tamper of referenced input file exits 8 (FMINE-REF-505).
    let mut bad_input = meas_bytes.to_vec();
    bad_input[0] ^= 0x01;
    fs::write(&meas_path, &bad_input).expect("tamper input");
    let (json_inp, code_inp) = run_cli(&[
        "cert".to_string(),
        cert_path.to_string_lossy().to_string(),
        "--inputs-dir".to_string(),
        inputs_dir.to_string_lossy().to_string(),
    ]);
    assert_eq!(code_inp, 8, "tampered input must exit 8: {json_inp}");
    assert!(json_inp.contains("FMINE-REF-505"));

    // 5. Decode round-trip of RCRT frame + tamper check.
    assert_eq!(
        ReconciliationCertificate::from_bytes(&rcrt_bytes).expect("decode RCRT"),
        cert
    );
    let mut bad_rcrt = rcrt_bytes;
    let r_mid = bad_rcrt.len() / 2;
    bad_rcrt[r_mid] ^= 0x01;
    assert_eq!(
        ReconciliationCertificate::from_bytes(&bad_rcrt)
            .expect_err("tampered RCRT must refuse")
            .to_refusal()
            .code,
        RefusalCode::DigestMismatch505.as_str()
    );

    println!(
        "BD-156 VERIFIER ROUND-TRIP PASS | subject={} | payload_sha256={} | replay_blake3={} | fmt_cert_digest={}",
        v_disk.subject_kind,
        cert.payload_sha256.to_hex(),
        cert.replay_digest_blake3.to_hex(),
        fmt_cert.certificate_digest.to_hex()
    );
}
