//! GATE-PLUMB-E3 misspecified synthetic mine verification (`BD-259`, plan §9.2 & §10.7, R7-C120).
//!
//! Verifies plan §10.7 `GATE-PLUMB-E3` across BOTH required misspecified regimes pooled over
//! `R = 200` (`>= 50`) independent seeded replicate worlds each:
//! 1. **Out-of-model fault (`MisspecKind::OutOfModelFault`)** from `fmine-gen::worlds::OpenPitAuConfig`
//!    (sealed via `ScoringHarness`, `-22%` unmodelled grade drop at `t = 60`):
//!    - Layer-5 `DualSurpriseMonitor` detects the fault;
//!    - Layer-3 `LayeredRecalibrator` restores pooled 80% coverage to `[0.75, 0.85]` (`±5 points`)
//!      within 12 post-alarm resolutions (`k = 1..=12`, `k = 4..=12`, and at `k = 12`);
//!    - Single unsegmented `GlobalPosteriorBaseline` (identical conjugate Normal-Inverse-Gamma
//!      Student-t predictor without alarm segmentation) documentably fails (`coverage < 0.65`).
//! 2. **Biased capex process (`MisspecKind::BiasedCapexProcess`, Bertisen & Davis GB-32 style)**
//!    (`T = 120` project milestones; baseline `N(0, 0.10^2)` for `t < 60`, then biased & over-dispersed
//!    overrun regime `+0.18` log-cost drift + `1.4x` core spread + `1/13` blowout tail):
//!    - Layer-5 alarms and Layer-3 restores pooled 80% coverage to `[0.75, 0.85]` within 12 post-alarm
//!      resolutions (`k = 1..=12` and `k = 12`), while the single global posterior documentably fails.
//! 3. **Control-mine non-interference (`WorldCondition::CorrectlySpecified`, `N = 1,000`):**
//!    Running `LayeredRecalibrator` with every layer ON preserves `CP95 <= 0.05` false alarms and
//!    `< 10%` sharpness loss.
//! 4. **Planted negative control:** raw unadapted L1/L2 fails `[0.75, 0.85]` on both misspecified regimes.

#![allow(clippy::expect_used)]

use fmine_calib::{
    DualSurpriseConfig, ForecastOutcome, GlobalPosteriorBaseline, LayeredConfig,
    LayeredRecalibrator, RawForecast,
};
use fmine_core::det::{ln, sqrt};
use fmine_core::rand::{PhiloxStream, RngKey};
use fmine_gen::harness::ScoringHarness;
use fmine_gen::worlds::{BiasVector, MisspecKind, OpenPitAuConfig, WorldCondition};
use fmine_numerics::exact_feas::power::clopper_pearson;

const GATE_PLUMB_E3_SEED_BASE: u64 = 20_260_259;
const REPLICATE_WORLDS: usize = 200; // >= 50 required by GATE-PLUMB-E3 (R7-C120)
const CONTROL_WORLDS: usize = 1_000;
const PERIODS: usize = 120;
const POST_ALARM_WINDOW: usize = 12;

const MEASUREMENT_REL_SIGMA: f64 = 0.05;
const GRADE_LOG_SIGMA: f64 = 0.12;
const BASE_TRUE_GRADE: f64 = 2.50;
const CAPEX_BASE_SIGMA: f64 = 0.10;

fn layered_config() -> LayeredConfig {
    LayeredConfig {
        monitor: DualSurpriseConfig {
            eproc_alpha: 0.04,
            cusum_k: 0.5,
            cusum_h: 8.5,
            alarm_scale: 1.50,
        },
        // Weak conjugate Normal-Inverse-Gamma prior for fast post-alarm adaptation (nu_0 = 2.0).
        prior_kappa: 0.25,
        prior_alpha: 1.0,
        prior_scale_mult: 1.0,
    }
}

#[derive(Debug, Clone)]
struct E3RegimeSummary {
    worlds: usize,
    alarmed_worlds: usize,
    worlds_with_full_window: usize,
    /// Pooled 80% coverage across all 12 post-alarm resolutions (`k = 1..=12`).
    layered_cov_1_12: f64,
    /// Pooled 80% coverage on resolutions `k = 4..=12` (settled post-alarm band).
    layered_cov_4_12: f64,
    /// Pooled 80% coverage specifically at the 12th post-alarm resolution (`k = 12`).
    layered_cov_at_12: f64,
    /// Pooled 80% coverage of the unsegmented single global posterior across `k = 1..=12`.
    global_cov_1_12: f64,
    /// Pooled 80% coverage of the single global posterior at `k = 12`.
    global_cov_at_12: f64,
    /// Pooled 80% coverage of raw unadapted L1/L2 across `k = 1..=12`.
    raw_cov_1_12: f64,
    median_alarm_period: usize,
}

fn evaluate_regime<F>(worlds: usize, mut generate_world: F) -> E3RegimeSummary
where
    F: FnMut(usize) -> (Vec<f64>, f64),
{
    let cfg = layered_config();
    let mut alarmed_worlds = 0usize;
    let mut worlds_with_full_window = 0usize;
    let mut alarm_periods = Vec::with_capacity(worlds);

    let mut layered_hits_1_12 = 0usize;
    let mut layered_hits_4_12 = 0usize;
    let mut layered_hits_12 = 0usize;
    let mut global_hits_1_12 = 0usize;
    let mut global_hits_12 = 0usize;
    let mut raw_hits_1_12 = 0usize;

    let mut total_1_12 = 0usize;
    let mut total_4_12 = 0usize;
    let mut total_12 = 0usize;

    for w in 0..worlds {
        let (ys, raw_sigma) = generate_world(w);
        let raw = RawForecast {
            mean: 0.0,
            std_dev: raw_sigma,
        };
        let mut stack = LayeredRecalibrator::new(cfg).expect("layered stack");
        let mut global = GlobalPosteriorBaseline::new(cfg).expect("global posterior");

        let mut alarm_step: Option<usize> = None;
        let mut post_count = 0usize;

        for (t, &y) in ys.iter().enumerate() {
            let pred = stack.predict(raw).expect("stack predict");
            let glob_iv = global.predict(raw).expect("global predict");

            if alarm_step.is_some() && post_count < POST_ALARM_WINDOW {
                post_count += 1;
                let y_obs = ForecastOutcome(y);
                let l_hit = usize::from(pred.published_interval_80.contains(y_obs));
                let g_hit = usize::from(glob_iv.contains(y_obs));
                let r_hit = usize::from(pred.raw_interval_80.contains(y_obs));

                layered_hits_1_12 += l_hit;
                global_hits_1_12 += g_hit;
                raw_hits_1_12 += r_hit;
                total_1_12 += 1;

                if post_count >= 4 {
                    layered_hits_4_12 += l_hit;
                    total_4_12 += 1;
                }
                if post_count == POST_ALARM_WINDOW {
                    layered_hits_12 += l_hit;
                    global_hits_12 += g_hit;
                    total_12 += 1;
                }
            }

            let snap = stack
                .observe_resolution(raw, ForecastOutcome(y))
                .expect("stack observe");
            global
                .observe_resolution(raw, ForecastOutcome(y))
                .expect("global observe");

            if alarm_step.is_none() && snap.any_alarm {
                alarm_step = Some(t + 1);
            }
        }

        if let Some(step) = alarm_step {
            alarmed_worlds += 1;
            alarm_periods.push(step);
        } else {
            alarm_periods.push(ys.len() + 1);
        }
        if post_count == POST_ALARM_WINDOW {
            worlds_with_full_window += 1;
        }
    }

    alarm_periods.sort_unstable();
    let median_alarm_period = alarm_periods[alarm_periods.len() / 2];

    E3RegimeSummary {
        worlds,
        alarmed_worlds,
        worlds_with_full_window,
        layered_cov_1_12: (layered_hits_1_12 as f64) / (total_1_12.max(1) as f64),
        layered_cov_4_12: (layered_hits_4_12 as f64) / (total_4_12.max(1) as f64),
        layered_cov_at_12: (layered_hits_12 as f64) / (total_12.max(1) as f64),
        global_cov_1_12: (global_hits_1_12 as f64) / (total_1_12.max(1) as f64),
        global_cov_at_12: (global_hits_12 as f64) / (total_12.max(1) as f64),
        raw_cov_1_12: (raw_hits_1_12 as f64) / (total_1_12.max(1) as f64),
        median_alarm_period,
    }
}

#[test]
fn test_gate_plumb_e3_layered_stack_restores_coverage_while_global_posterior_fails() {
    // 1. Regime A: Out-of-model fault from fmine-gen::worlds::OpenPitAuConfig (sealed truth).
    let fault_cfg = OpenPitAuConfig {
        world_id: "gate-plumb-e3-out-of-model-fault".to_string(),
        periods: PERIODS,
        base_dry_tonnes_per_period: 250_000.0,
        base_true_grade_g_per_t: BASE_TRUE_GRADE,
        measurement_rel_sigma: MEASUREMENT_REL_SIGMA,
        biases: BiasVector::unbiased(),
        condition: WorldCondition::Misspecified(MisspecKind::OutOfModelFault),
    };
    let fault_sigma =
        sqrt(GRADE_LOG_SIGMA * GRADE_LOG_SIGMA + MEASUREMENT_REL_SIGMA * MEASUREMENT_REL_SIGMA);
    let harness = ScoringHarness::new(GATE_PLUMB_E3_SEED_BASE, "gate-plumb-e3-fault");

    let fault = evaluate_regime(REPLICATE_WORLDS, |w| {
        let (obs, _sealed) = harness
            .generate_open_pit_au(GATE_PLUMB_E3_SEED_BASE + (w as u64), &fault_cfg)
            .expect("generate fault world");
        let ys: Vec<f64> = obs
            .periods
            .iter()
            .map(|p| ln(p.plant_sampler_grade_g_per_t / BASE_TRUE_GRADE))
            .collect();
        (ys, fault_sigma)
    });

    // 2. Regime B: Biased capex overrun process (plan §9.3 GB-32 / §10.7 GATE-PLUMB-E3).
    // First 60 periods match baseline N(0, 0.10^2); from t = 60 onward, an unmodelled biased
    // contractor/escalation regime introduces +0.18 mean log-cost overrun, 1.4x core dispersion,
    // and a 1/13 Bertisen & Davis heavy overrun blowout mixture component (+0.55 +/- 0.25).
    let capex = evaluate_regime(REPLICATE_WORLDS, |w| {
        let mut rng = PhiloxStream::new(RngKey {
            seed_root: GATE_PLUMB_E3_SEED_BASE + 10_000,
            partition: 259,
            domain: 2,
            var_group: 1,
            realization: w as u32,
            brick: 0,
        });
        let mut ys = Vec::with_capacity(PERIODS);
        for t in 0..PERIODS {
            let y = if t < PERIODS / 2 {
                CAPEX_BASE_SIGMA * rng.next_normal()
            } else if rng.next_f64_open01() < (1.0 / 13.0) {
                0.55 + 0.25 * rng.next_normal()
            } else {
                0.18 + (1.4 * CAPEX_BASE_SIGMA) * rng.next_normal()
            };
            ys.push(y);
        }
        (ys, CAPEX_BASE_SIGMA)
    });

    for (name, res) in [
        ("out_of_model_fault", &fault),
        ("biased_capex_process", &capex),
    ] {
        assert!(
            res.worlds_with_full_window >= 190,
            "{name}: expected >= 190/{} worlds with full 12-resolution post-alarm window, got {}",
            res.worlds,
            res.worlds_with_full_window
        );
        // GATE-PLUMB-E3 core bar: layered stack restores 80% coverage to +/-5 points [0.75, 0.85]
        // within 12 resolutions after an alarm, pooled over R >= 50 replicate worlds.
        assert!(
            (0.75..=0.85).contains(&res.layered_cov_1_12),
            "{name}: layered pooled 1..=12 coverage {:.4} outside [0.75, 0.85]",
            res.layered_cov_1_12
        );
        assert!(
            (0.75..=0.85).contains(&res.layered_cov_4_12),
            "{name}: layered pooled 4..=12 coverage {:.4} outside [0.75, 0.85]",
            res.layered_cov_4_12
        );
        assert!(
            (0.75..=0.85).contains(&res.layered_cov_at_12),
            "{name}: layered resolution-12 coverage {:.4} outside [0.75, 0.85]",
            res.layered_cov_at_12
        );
        // Single unsegmented global posterior MUST documentably fail (dragged by pre-fault history).
        assert!(
            res.global_cov_1_12 < 0.65 && res.global_cov_at_12 < 0.70,
            "{name}: expected single global posterior to documentably fail, got cov_1_12={:.4}, cov_at_12={:.4}",
            res.global_cov_1_12,
            res.global_cov_at_12
        );
        // Raw unadapted L1/L2 baseline also fails badly.
        assert!(
            res.raw_cov_1_12 < 0.55,
            "{name}: expected raw L1/L2 baseline to fail, got {:.4}",
            res.raw_cov_1_12
        );
    }

    // 3. Control-mine non-interference check: LayeredRecalibrator with every layer ON must still
    // keep false alarms <= 0.05 (95% CP upper bound) and sharpness loss < 10% on N = 1,000 nulls.
    let ctrl_cfg = OpenPitAuConfig {
        world_id: "gate-plumb-e3-control-check".to_string(),
        periods: PERIODS,
        base_dry_tonnes_per_period: 250_000.0,
        base_true_grade_g_per_t: BASE_TRUE_GRADE,
        measurement_rel_sigma: MEASUREMENT_REL_SIGMA,
        biases: BiasVector::unbiased(),
        condition: WorldCondition::CorrectlySpecified,
    };
    let ctrl_harness = ScoringHarness::new(GATE_PLUMB_E3_SEED_BASE + 20_000, "gate-plumb-e3-ctrl");
    let cfg = layered_config();
    let raw = RawForecast {
        mean: 0.0,
        std_dev: fault_sigma,
    };
    let mut ctrl_alarms = 0usize;
    let mut ctrl_raw_width = 0.0_f64;
    let mut ctrl_pub_width = 0.0_f64;

    for w in 0..CONTROL_WORLDS {
        let (obs, _sealed) = ctrl_harness
            .generate_open_pit_au(GATE_PLUMB_E3_SEED_BASE + 20_000 + (w as u64), &ctrl_cfg)
            .expect("generate control world");
        let mut stack = LayeredRecalibrator::new(cfg).expect("layered stack");
        for p in &obs.periods {
            let pred = stack.predict(raw).expect("predict");
            ctrl_raw_width += 2.0 * pred.raw_interval_80.half_width;
            ctrl_pub_width += 2.0 * pred.published_interval_80.half_width;
            let y = ln(p.plant_sampler_grade_g_per_t / BASE_TRUE_GRADE);
            stack
                .observe_resolution(raw, ForecastOutcome(y))
                .expect("observe");
        }
        if stack.monitor_snapshot().any_alarm {
            ctrl_alarms += 1;
        }
    }
    let ctrl_cp95 = clopper_pearson(ctrl_alarms as u64, CONTROL_WORLDS as u64, 0.10)
        .expect("cp")
        .1;
    let ctrl_sharpness_loss = (ctrl_pub_width - ctrl_raw_width) / ctrl_raw_width;
    assert!(
        ctrl_cp95 <= 0.05,
        "control interference check: CP95 {ctrl_cp95:.4} > 0.05 ({ctrl_alarms}/{CONTROL_WORLDS})"
    );
    assert!(
        ctrl_sharpness_loss < 0.10,
        "control interference check: sharpness loss {ctrl_sharpness_loss:.4} >= 10%"
    );

    println!(
        "{{\"gate\":\"GATE-PLUMB-E3\",\"seed_base\":{GATE_PLUMB_E3_SEED_BASE},\"replicates\":{REPLICATE_WORLDS},\"window\":{POST_ALARM_WINDOW},\
          \"fault\":{{\"alarmed\":{},\"full_window\":{},\"median_alarm\":{},\"layered_cov_1_12\":{:.5},\"layered_cov_4_12\":{:.5},\"layered_cov_at_12\":{:.5},\"global_cov_1_12\":{:.5},\"global_cov_at_12\":{:.5},\"raw_cov_1_12\":{:.5}}},\
          \"capex\":{{\"alarmed\":{},\"full_window\":{},\"median_alarm\":{},\"layered_cov_1_12\":{:.5},\"layered_cov_4_12\":{:.5},\"layered_cov_at_12\":{:.5},\"global_cov_1_12\":{:.5},\"global_cov_at_12\":{:.5},\"raw_cov_1_12\":{:.5}}},\
          \"control\":{{\"worlds\":{CONTROL_WORLDS},\"alarms\":{ctrl_alarms},\"cp95\":{ctrl_cp95:.5},\"sharpness_loss\":{ctrl_sharpness_loss:.5}}}}}",
        fault.alarmed_worlds,
        fault.worlds_with_full_window,
        fault.median_alarm_period,
        fault.layered_cov_1_12,
        fault.layered_cov_4_12,
        fault.layered_cov_at_12,
        fault.global_cov_1_12,
        fault.global_cov_at_12,
        fault.raw_cov_1_12,
        capex.alarmed_worlds,
        capex.worlds_with_full_window,
        capex.median_alarm_period,
        capex.layered_cov_1_12,
        capex.layered_cov_4_12,
        capex.layered_cov_at_12,
        capex.global_cov_1_12,
        capex.global_cov_at_12,
        capex.raw_cov_1_12,
    );
}
