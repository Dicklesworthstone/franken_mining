//! GATE-PLUMB-E5 control-mine verification (`BD-051`, plan §10.7 & §33.3, `KILL-PLUMB-6`).
//!
//! Runs `N = 2,000` (>= 1,000) independent seeded control-mine runs from `fmine-gen::worlds`
//! (`WorldCondition::CorrectlySpecified`, `BiasVector::unbiased()`, accessed strictly through
//! `ScoringHarness` sealed-truth boundaries) against `fmine_calib::DualSurpriseMonitor` (combining
//! Waudby-Smith/Ramdas location+dispersion `ScoreBettingEProcess`es and two-sided Page
//! `TwoSidedCusum`), verifying:
//! 1. One-sided 95% Clopper–Pearson upper bound on null false-alarm rate <= alpha (0.05) for the
//!    e-process monitor, the CUSUM monitor, and the combined dual monitor;
//! 2. Sharpness loss < 10% (and CRPS degradation < 10%) with the causal Layer-3 recalibration
//!    stack active across the control mine;
//! 3. High detection power (>= 95%) on both systematic grade-bias (`BiasVector`) and structural
//!    fault (`MisspecKind::OutOfModelFault`) misspecified worlds without retuning;
//! 4. Three planted negative controls that MUST fail the gate (mis-tuned CUSUM threshold, leaked
//!    bias inside the supposed control mine, and unconditional L3 width inflation).

#![allow(clippy::expect_used)]

use fmine_calib::{
    DualSurpriseConfig, DualSurpriseMonitor, StandardizedResidual, gaussian_crps,
    winkler_interval_score,
};
use fmine_core::det::{ln, sqrt};
use fmine_gen::harness::ScoringHarness;
use fmine_gen::worlds::{BiasVector, MisspecKind, OpenPitAuConfig, WorldCondition};
use fmine_numerics::exact_feas::power::clopper_pearson;

// Analytically derived monitor budget for ALPHA_TARGET = 0.05 over PERIODS_PER_RUN = 120:
// - E-process pair (location + dispersion) at EPROC_ALPHA = 0.04 (0.02 each) gives anytime
//   false-alarm probability <= 0.04 by Ville's inequality.
// - Two-sided Page CUSUM at k = 0.5, h = 8.5 has Siegmund ARL_0 ~= exp(8.5 + 1.166) - 10.7 ~= 15,760
//   (satisfying KILL-PLUMB-6's ARL_0 >= 85*M for up to M = 185 streams), giving horizon FAR ~= 0.0076.
// - Union bound: 0.04 + 0.0076 = 0.0476 < ALPHA_TARGET = 0.05.
// Evaluated on fresh pre-registered seed base 20_260_511.
const GATE_PLUMB_E5_SEED_BASE: u64 = 20_260_511;
const CONTROL_RUNS: usize = 2_000;
const SHIFTED_RUNS: usize = 200;
const PERIODS_PER_RUN: usize = 120;
const ALPHA_TARGET: f64 = 0.05;
const EPROC_ALPHA: f64 = 0.04;
const CUSUM_K: f64 = 0.5;
const CUSUM_H: f64 = 8.5;
const L3_ALARM_SCALE: f64 = 1.50;
const Q90_STANDARD_NORMAL: f64 = 1.281_551_565_544_600_4;
const MEASUREMENT_REL_SIGMA: f64 = 0.05;
const GRADE_LOG_SIGMA: f64 = 0.12;
const BASE_TRUE_GRADE: f64 = 2.50;

#[derive(Debug, Clone)]
struct BatchMetrics {
    runs: usize,
    eproc_alarms: usize,
    cusum_alarms: usize,
    combined_alarms: usize,
    eproc_cp_upper_95: f64,
    cusum_cp_upper_95: f64,
    combined_cp_upper_95: f64,
    sharpness_loss_fraction: f64,
    crps_loss_fraction: f64,
    winkler_loss_fraction: f64,
    raw_coverage_80: f64,
    layered_coverage_80: f64,
    median_cusum_alarm: usize,
    median_eproc_alarm: usize,
}

fn control_world_config(condition: WorldCondition, biases: BiasVector) -> OpenPitAuConfig {
    OpenPitAuConfig {
        world_id: "gate-plumb-e5-control-mine".to_string(),
        periods: PERIODS_PER_RUN,
        base_dry_tonnes_per_period: 250_000.0,
        base_true_grade_g_per_t: BASE_TRUE_GRADE,
        measurement_rel_sigma: MEASUREMENT_REL_SIGMA,
        biases,
        condition,
    }
}

fn median_time(times: &[Option<u64>], censor: usize) -> usize {
    let mut vals: Vec<usize> = times
        .iter()
        .map(|t| t.map_or(censor + 1, |v| v as usize))
        .collect();
    vals.sort_unstable();
    vals[vals.len() / 2]
}

/// Evaluate `DualSurpriseMonitor` + causal L3 interval recalibration on `runs` sealed-truth
/// `OpenPitAuConfig` worlds.
///
/// - `channel == 0`: Grade forecast innovation `y_t = ln(plant_sampler_grade / base_grade)` with
///   exact control law `N(0, GRADE_LOG_SIGMA^2 + MEASUREMENT_REL_SIGMA^2)`.
/// - `channel == 1`: Resource-vs-sampler reconciliation `y_t = ln(rm_grade / plant_sampler_grade)`
///   with exact control law `N(0, 2 * MEASUREMENT_REL_SIGMA^2)`.
#[allow(clippy::too_many_arguments)]
fn run_world_batch(
    seed_offset: u64,
    runs: usize,
    condition: WorldCondition,
    biases: BiasVector,
    channel: u8,
    eproc_alpha: f64,
    cusum_k: f64,
    cusum_h: f64,
    alarm_scale: f64,
    unconditional_l3_scale: f64,
) -> BatchMetrics {
    let cfg = control_world_config(condition, biases);
    let sigma = if channel == 0 {
        sqrt(GRADE_LOG_SIGMA * GRADE_LOG_SIGMA + MEASUREMENT_REL_SIGMA * MEASUREMENT_REL_SIGMA)
    } else {
        sqrt(2.0) * MEASUREMENT_REL_SIGMA
    };

    let mut eproc_alarms = 0usize;
    let mut cusum_alarms = 0usize;
    let mut combined_alarms = 0usize;
    let mut eproc_times = Vec::with_capacity(runs);
    let mut cusum_times = Vec::with_capacity(runs);

    let mut total_raw_width = 0.0_f64;
    let mut total_layered_width = 0.0_f64;
    let mut total_raw_crps = 0.0_f64;
    let mut total_layered_crps = 0.0_f64;
    let mut total_raw_winkler = 0.0_f64;
    let mut total_layered_winkler = 0.0_f64;
    let mut raw_covered = 0usize;
    let mut layered_covered = 0usize;
    let mut total_obs = 0usize;

    let harness = ScoringHarness::new(GATE_PLUMB_E5_SEED_BASE + seed_offset, "gate-plumb-e5");
    for r in 0..runs {
        // Strictly sealed-truth session: only OpenPitAuObservations are ever read by the monitor.
        let (obs, _sealed_truth) = harness
            .generate_open_pit_au(GATE_PLUMB_E5_SEED_BASE + seed_offset + (r as u64), &cfg)
            .expect("generate world");

        let mut monitor = DualSurpriseMonitor::new(DualSurpriseConfig {
            eproc_alpha,
            cusum_k,
            cusum_h,
            alarm_scale,
        })
        .expect("monitor");

        for p in &obs.periods {
            // Strictly causal L3 scale from prior periods only (INV-008: zero lookahead).
            let raw_sd = sigma;
            let layered_sd = sigma * (monitor.l3_scale() * unconditional_l3_scale);

            let y = if channel == 0 {
                ln(p.plant_sampler_grade_g_per_t / BASE_TRUE_GRADE)
            } else {
                ln(p.resource_model_grade_g_per_t / p.plant_sampler_grade_g_per_t)
            };

            let raw_half = Q90_STANDARD_NORMAL * raw_sd;
            let lay_half = Q90_STANDARD_NORMAL * layered_sd;
            total_raw_width += 2.0 * raw_half;
            total_layered_width += 2.0 * lay_half;
            if y >= -raw_half && y <= raw_half {
                raw_covered += 1;
            }
            if y >= -lay_half && y <= lay_half {
                layered_covered += 1;
            }
            total_raw_crps += gaussian_crps(0.0, raw_sd, y).expect("raw crps");
            total_layered_crps += gaussian_crps(0.0, layered_sd, y).expect("layered crps");
            total_raw_winkler +=
                winkler_interval_score(-raw_half, raw_half, 0.2, y).expect("raw winkler");
            total_layered_winkler +=
                winkler_interval_score(-lay_half, lay_half, 0.2, y).expect("layered winkler");
            total_obs += 1;

            let z = y / sigma;
            monitor.observe(StandardizedResidual(z)).expect("finite z");
        }

        let snap = monitor.snapshot();
        if snap.eproc_alarm {
            eproc_alarms += 1;
        }
        if snap.cusum_alarm {
            cusum_alarms += 1;
        }
        if snap.any_alarm {
            combined_alarms += 1;
        }
        eproc_times.push(snap.first_eproc_alarm);
        cusum_times.push(snap.first_cusum_alarm);
    }

    assert!(total_raw_winkler.is_finite() && total_layered_winkler.is_finite());

    let cp_upper = |k: usize| {
        clopper_pearson(k as u64, runs as u64, 0.10)
            .expect("clopper_pearson")
            .1
    };

    BatchMetrics {
        runs,
        eproc_alarms,
        cusum_alarms,
        combined_alarms,
        eproc_cp_upper_95: cp_upper(eproc_alarms),
        cusum_cp_upper_95: cp_upper(cusum_alarms),
        combined_cp_upper_95: cp_upper(combined_alarms),
        sharpness_loss_fraction: (total_layered_width - total_raw_width) / total_raw_width,
        crps_loss_fraction: (total_layered_crps - total_raw_crps) / total_raw_crps,
        winkler_loss_fraction: (total_layered_winkler - total_raw_winkler) / total_raw_winkler,
        raw_coverage_80: (raw_covered as f64) / (total_obs as f64),
        layered_coverage_80: (layered_covered as f64) / (total_obs as f64),
        median_cusum_alarm: median_time(&cusum_times, PERIODS_PER_RUN),
        median_eproc_alarm: median_time(&eproc_times, PERIODS_PER_RUN),
    }
}

#[test]
fn test_gate_plumb_e5_control_mine_false_alarm_sharpness_and_power() {
    // 1. Control mine — Channel 0 (grade forecast innovations), N = 2,000 (>= 1,000) null runs.
    let ctrl_forecast = run_world_batch(
        0,
        CONTROL_RUNS,
        WorldCondition::CorrectlySpecified,
        BiasVector::unbiased(),
        0,
        EPROC_ALPHA,
        CUSUM_K,
        CUSUM_H,
        L3_ALARM_SCALE,
        1.0,
    );

    // 2. Control mine — Channel 1 (resource-vs-sampler reconciliation), N = 2,000 null runs.
    let ctrl_recon = run_world_batch(
        10_000,
        CONTROL_RUNS,
        WorldCondition::CorrectlySpecified,
        BiasVector::unbiased(),
        1,
        EPROC_ALPHA,
        CUSUM_K,
        CUSUM_H,
        L3_ALARM_SCALE,
        1.0,
    );

    for (label, m) in [
        ("control_forecast_channel", &ctrl_forecast),
        ("control_reconciliation_channel", &ctrl_recon),
    ] {
        assert!(
            m.eproc_cp_upper_95 <= ALPHA_TARGET,
            "{label}: e-process 95% CP upper bound {:.4} > {ALPHA_TARGET} (alarms {}/{})",
            m.eproc_cp_upper_95,
            m.eproc_alarms,
            m.runs
        );
        assert!(
            m.cusum_cp_upper_95 <= ALPHA_TARGET,
            "{label}: CUSUM 95% CP upper bound {:.4} > {ALPHA_TARGET} (alarms {}/{})",
            m.cusum_cp_upper_95,
            m.cusum_alarms,
            m.runs
        );
        assert!(
            m.combined_cp_upper_95 <= ALPHA_TARGET,
            "{label}: combined 95% CP upper bound {:.4} > {ALPHA_TARGET} (alarms {}/{})",
            m.combined_cp_upper_95,
            m.combined_alarms,
            m.runs
        );
        assert!(
            m.sharpness_loss_fraction < 0.10,
            "{label}: sharpness loss {:.4} >= 10%",
            m.sharpness_loss_fraction
        );
        assert!(
            m.crps_loss_fraction < 0.10,
            "{label}: CRPS degradation {:.4} >= 10%",
            m.crps_loss_fraction
        );
        assert!(
            m.winkler_loss_fraction < 0.10,
            "{label}: Winkler interval-score degradation {:.4} >= 10%",
            m.winkler_loss_fraction
        );
        assert!(
            (0.79..=0.81).contains(&m.raw_coverage_80),
            "{label}: control mine raw 80% coverage {:.4} outside [0.79, 0.81]",
            m.raw_coverage_80
        );
        assert!(
            (0.79..=0.82).contains(&m.layered_coverage_80),
            "{label}: control mine layered 80% coverage {:.4} outside [0.79, 0.82]",
            m.layered_coverage_80
        );
    }

    // 3. Positive power on misspecified worlds (exact same parameters, zero retuning).
    let fault_shift = run_world_batch(
        20_000,
        SHIFTED_RUNS,
        WorldCondition::Misspecified(MisspecKind::OutOfModelFault),
        BiasVector::unbiased(),
        0,
        EPROC_ALPHA,
        CUSUM_K,
        CUSUM_H,
        L3_ALARM_SCALE,
        1.0,
    );
    let bias_shift = run_world_batch(
        30_000,
        SHIFTED_RUNS,
        WorldCondition::CorrectlySpecified,
        BiasVector {
            resource_grade_factor: 1.10,
            ..BiasVector::unbiased()
        },
        1,
        EPROC_ALPHA,
        CUSUM_K,
        CUSUM_H,
        L3_ALARM_SCALE,
        1.0,
    );

    assert!(
        (fault_shift.eproc_alarms as f64) / (fault_shift.runs as f64) >= 0.95,
        "OutOfModelFault e-process power too low: {}/{}",
        fault_shift.eproc_alarms,
        fault_shift.runs
    );
    assert!(
        (fault_shift.cusum_alarms as f64) / (fault_shift.runs as f64) >= 0.95,
        "OutOfModelFault CUSUM power too low: {}/{}",
        fault_shift.cusum_alarms,
        fault_shift.runs
    );
    assert!(
        fault_shift.layered_coverage_80 > fault_shift.raw_coverage_80,
        "L3 recalibration must improve coverage after OutOfModelFault: layered {:.4} vs raw {:.4}",
        fault_shift.layered_coverage_80,
        fault_shift.raw_coverage_80
    );
    assert!(
        (bias_shift.eproc_alarms as f64) / (bias_shift.runs as f64) >= 0.98
            && (bias_shift.cusum_alarms as f64) / (bias_shift.runs as f64) >= 0.98,
        "BiasVector (+10% resource grade) detection power too low: eproc={}/{}, cusum={}/{}",
        bias_shift.eproc_alarms,
        bias_shift.runs,
        bias_shift.cusum_alarms,
        bias_shift.runs
    );
    assert!(
        bias_shift.median_cusum_alarm <= 15,
        "CUSUM median alarm on +10% grade bias too slow: {}",
        bias_shift.median_cusum_alarm
    );

    // 4. Planted negative controls (each MUST violate GATE-PLUMB-E5!).
    let neg_mistuned_cusum = run_world_batch(
        40_000,
        1_000,
        WorldCondition::CorrectlySpecified,
        BiasVector::unbiased(),
        0,
        EPROC_ALPHA,
        CUSUM_K,
        2.5, // far too small -> high false alarms
        L3_ALARM_SCALE,
        1.0,
    );
    assert!(
        neg_mistuned_cusum.cusum_cp_upper_95 > ALPHA_TARGET,
        "planted negative (h=2.5) must fail CP upper bound: {:.4}",
        neg_mistuned_cusum.cusum_cp_upper_95
    );

    let neg_leaked_bias = run_world_batch(
        50_000,
        1_000,
        WorldCondition::CorrectlySpecified,
        BiasVector {
            resource_grade_factor: 1.05, // +5% unmodelled bias leaked into control
            ..BiasVector::unbiased()
        },
        1,
        EPROC_ALPHA,
        CUSUM_K,
        CUSUM_H,
        L3_ALARM_SCALE,
        1.0,
    );
    assert!(
        neg_leaked_bias.combined_cp_upper_95 > ALPHA_TARGET,
        "planted negative (+5% leaked bias) must fail CP upper bound: {:.4}",
        neg_leaked_bias.combined_cp_upper_95
    );

    let neg_unconditional_inflation = run_world_batch(
        60_000,
        200,
        WorldCondition::CorrectlySpecified,
        BiasVector::unbiased(),
        0,
        EPROC_ALPHA,
        CUSUM_K,
        CUSUM_H,
        L3_ALARM_SCALE,
        1.25, // always inflates by +25% -> fails < 10% sharpness loss
    );
    assert!(
        neg_unconditional_inflation.sharpness_loss_fraction >= 0.10,
        "planted negative (unconditional 1.25x inflation) must fail sharpness < 10%: {:.4}",
        neg_unconditional_inflation.sharpness_loss_fraction
    );

    println!(
        "{{\"gate\":\"GATE-PLUMB-E5\",\"seed_base\":{GATE_PLUMB_E5_SEED_BASE},\"n_control\":{CONTROL_RUNS},\"periods\":{PERIODS_PER_RUN},\
          \"alpha\":{ALPHA_TARGET},\"cusum_k\":{CUSUM_K},\"cusum_h\":{CUSUM_H},\"l3_scale\":{L3_ALARM_SCALE},\
          \"ctrl_forecast\":{{\"eproc_alarms\":{},\"eproc_cp95\":{:.5},\"cusum_alarms\":{},\"cusum_cp95\":{:.5},\"combined_alarms\":{},\"combined_cp95\":{:.5},\"sharpness_loss\":{:.5},\"crps_loss\":{:.5},\"winkler_loss\":{:.5},\"raw_cov80\":{:.5},\"layered_cov80\":{:.5}}},\
          \"ctrl_recon\":{{\"eproc_alarms\":{},\"eproc_cp95\":{:.5},\"cusum_alarms\":{},\"cusum_cp95\":{:.5},\"combined_alarms\":{},\"combined_cp95\":{:.5},\"sharpness_loss\":{:.5},\"crps_loss\":{:.5},\"winkler_loss\":{:.5},\"raw_cov80\":{:.5},\"layered_cov80\":{:.5}}},\
          \"fault_power\":{{\"n\":{SHIFTED_RUNS},\"eproc\":{},\"cusum\":{},\"median_eproc\":{},\"median_cusum\":{},\"raw_cov80\":{:.5},\"layered_cov80\":{:.5}}},\
          \"bias_power\":{{\"n\":{SHIFTED_RUNS},\"eproc\":{},\"cusum\":{},\"median_eproc\":{},\"median_cusum\":{}}},\
          \"neg_controls\":{{\"mistuned_cusum_cp95\":{:.5},\"leaked_bias_cp95\":{:.5},\"unconditional_sharpness_loss\":{:.5}}}}}",
        ctrl_forecast.eproc_alarms,
        ctrl_forecast.eproc_cp_upper_95,
        ctrl_forecast.cusum_alarms,
        ctrl_forecast.cusum_cp_upper_95,
        ctrl_forecast.combined_alarms,
        ctrl_forecast.combined_cp_upper_95,
        ctrl_forecast.sharpness_loss_fraction,
        ctrl_forecast.crps_loss_fraction,
        ctrl_forecast.winkler_loss_fraction,
        ctrl_forecast.raw_coverage_80,
        ctrl_forecast.layered_coverage_80,
        ctrl_recon.eproc_alarms,
        ctrl_recon.eproc_cp_upper_95,
        ctrl_recon.cusum_alarms,
        ctrl_recon.cusum_cp_upper_95,
        ctrl_recon.combined_alarms,
        ctrl_recon.combined_cp_upper_95,
        ctrl_recon.sharpness_loss_fraction,
        ctrl_recon.crps_loss_fraction,
        ctrl_recon.winkler_loss_fraction,
        ctrl_recon.raw_coverage_80,
        ctrl_recon.layered_coverage_80,
        fault_shift.eproc_alarms,
        fault_shift.cusum_alarms,
        fault_shift.median_eproc_alarm,
        fault_shift.median_cusum_alarm,
        fault_shift.raw_coverage_80,
        fault_shift.layered_coverage_80,
        bias_shift.eproc_alarms,
        bias_shift.cusum_alarms,
        bias_shift.median_eproc_alarm,
        bias_shift.median_cusum_alarm,
        neg_mistuned_cusum.cusum_cp_upper_95,
        neg_leaked_bias.combined_cp_upper_95,
        neg_unconditional_inflation.sharpness_loss_fraction,
    );
}
