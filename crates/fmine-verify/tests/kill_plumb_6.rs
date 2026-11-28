//! `KILL-PLUMB-6` control-mine monitor false-alarm verification (`BD-261`, plan §10.4, §34.5, §39):
//!
//! Plan `KILL-PLUMB-6` threshold:
//! - Monitors must raise **`<= 1` false alarm per site per quarter** on the unbiased control mine.
//! - Tuning each CUSUM stream to `ARL_0 >= 85 * M` (`M = 4` monitored Touchstone reconciliation
//!   streams per site) keeps the expected quarterly false-alarm rate **`<= 0.36`** per site per
//!   quarter and the fraction of site-quarters hitting `> 1` false alarm **`<= 5%`**.
//!
//! Runs:
//! 1. **Control mine (`500` sites × `4` quarters = `2,000` site-quarters, `M = 4` Touchstone
//!    reconciliation residual streams per site)** from `fmine-gen::worlds`
//!    (`WorldCondition::CorrectlySpecified`, `BiasVector::unbiased()`, accessed strictly through
//!    `ScoringHarness` sealed truth) monitored by `fmine-calib::LayeredRecalibrator`
//!    (`DualSurpriseConfig` = `ScoreBettingEProcess` + two-sided Page `TwoSidedCusum`).
//! 2. **Positive control (planted `-5%` mill sampler bias, `100` sites):** confirms the same
//!    untuned monitor stack detects the bias reliably (`>= 90%` detection within 2 quarters).
//! 3. **Negative control (mis-tuned tight CUSUM `h = 1.5`, `100` sites):** confirms the harness
//!    genuinely catches `> 1.0` false alarms per site per quarter and fails closed!

use fmine_calib::{
    DualSurpriseConfig, ForecastOutcome, LayeredConfig, LayeredRecalibrator, RawForecast,
};
use fmine_core::codec::Encoder;
use fmine_core::det::{ln, sqrt};
use fmine_core::hash::blake3_hash;
use fmine_gen::harness::ScoringHarness;
use fmine_gen::sbc::clopper_pearson_interval;
use fmine_gen::worlds::{BiasVector, OpenPitAuConfig, WorldCondition};

const SEED_BASE: u64 = 202_6261;
const CONTROL_SITES: usize = 500;
const QUARTERS_PER_SITE: usize = 4;
const PERIODS_PER_QUARTER: usize = 12; // weekly reconciliation cadence (48 periods/yr)
const TOTAL_PERIODS: usize = QUARTERS_PER_SITE * PERIODS_PER_QUARTER;
const MEAS_REL_SIGMA: f64 = 0.025;
const STREAMS_PER_SITE: usize = 4;

struct MonitorSummary {
    site_quarters: usize,
    total_false_alarms: usize,
    mean_alarms_per_site_quarter: f64,
    quarters_exceeding_one_alarm: usize,
    frac_quarters_exceeding_one: f64,
    sites_alarmed_in_two_quarters: usize,
}

fn run_control_mine_monitors(
    seed_offset: u64,
    sites: usize,
    biases: BiasVector,
    eproc_alpha: f64,
    cusum_k: f64,
    cusum_h: f64,
) -> Result<MonitorSummary, String> {
    let cfg = OpenPitAuConfig {
        world_id: "kill-plumb-6-control-mine".to_string(),
        periods: TOTAL_PERIODS,
        base_dry_tonnes_per_period: 250_000.0,
        base_true_grade_g_per_t: 2.50,
        measurement_rel_sigma: MEAS_REL_SIGMA,
        biases,
        condition: WorldCondition::CorrectlySpecified,
    };
    let harness = ScoringHarness::new(SEED_BASE + seed_offset, "KILL-PLUMB-6");

    // Exact analytical innovation SDs for the 4 Touchstone reconciliation log-ratio streams
    // directly from `fmine-gen::worlds` equations:
    //   0: ln(truck_tonnes / rm_tonnes)                   -> sqrt(2) * sigma
    //   1: ln(rm_grade / plant_sampler_grade)             -> sqrt(2) * sigma
    //   2: ln(truck * sampler * R_nom / refinery_au)      -> sqrt(2.0625) * sigma
    //   3: ln(rm_t * rm_g * R_nom / refinery_au)          -> sqrt(2.0625) * sigma
    let sigmas = [
        sqrt(2.0) * MEAS_REL_SIGMA,
        sqrt(2.0) * MEAS_REL_SIGMA,
        sqrt(2.0625) * MEAS_REL_SIGMA,
        sqrt(2.0625) * MEAS_REL_SIGMA,
    ];

    let mut total_false_alarms = 0usize;
    let mut quarters_exceeding_one_alarm = 0usize;
    let mut sites_alarmed_in_two_quarters = 0usize;

    for s in 0..sites {
        let (obs, _seal) = harness
            .generate_open_pit_au(SEED_BASE + seed_offset + (s as u64), &cfg)
            .map_err(|e| format!("generate world: {e}"))?;

        let mut site_alarmed_2q = false;
        // Evaluate each quarter independently (fresh quarterly monitor window per site-quarter
        // matching the "alarms per site per quarter" accounting basis of KILL-PLUMB-6).
        for q in 0..QUARTERS_PER_SITE {
            let mut stacks: Vec<LayeredRecalibrator> = (0..STREAMS_PER_SITE)
                .map(|_| {
                    LayeredRecalibrator::new(LayeredConfig {
                        monitor: DualSurpriseConfig {
                            eproc_alpha,
                            cusum_k,
                            cusum_h,
                            alarm_scale: 1.50,
                        },
                        prior_kappa: 0.25,
                        prior_alpha: 1.0,
                        prior_scale_mult: 1.50,
                    })
                    .map_err(|e| format!("recalibrator init: {e}"))
                })
                .collect::<Result<Vec<_>, _>>()?;

            let mut stream_triggered = [false; STREAMS_PER_SITE];
            let start = q * PERIODS_PER_QUARTER;
            let end = start + PERIODS_PER_QUARTER;

            for p in &obs.periods[start..end] {
                let y = [
                    ln(p.truck_scale_dry_tonnes / p.resource_model_tonnes),
                    ln(p.resource_model_grade_g_per_t / p.plant_sampler_grade_g_per_t),
                    ln((p.truck_scale_dry_tonnes
                        * p.plant_sampler_grade_g_per_t
                        * obs.nominal_recovery)
                        / p.refinery_recovered_au_grams),
                    ln((p.resource_model_tonnes
                        * p.resource_model_grade_g_per_t
                        * obs.nominal_recovery)
                        / p.refinery_recovered_au_grams),
                ];

                for k in 0..STREAMS_PER_SITE {
                    let raw = RawForecast {
                        mean: 0.0,
                        std_dev: sigmas[k],
                    };
                    let snap = stacks[k]
                        .observe_resolution(raw, ForecastOutcome(y[k]))
                        .map_err(|e| format!("observe stream {k}: {e}"))?;
                    if snap.any_alarm {
                        stream_triggered[k] = true;
                    }
                }
            }

            let q_alarms: usize = stream_triggered.iter().filter(|&&b| b).count();
            total_false_alarms += q_alarms;
            if q_alarms > 1 {
                quarters_exceeding_one_alarm += 1;
            }
            if q < 2 && q_alarms > 0 {
                site_alarmed_2q = true;
            }
        }
        if site_alarmed_2q {
            sites_alarmed_in_two_quarters += 1;
        }
    }

    let site_quarters = sites * QUARTERS_PER_SITE;
    Ok(MonitorSummary {
        site_quarters,
        total_false_alarms,
        mean_alarms_per_site_quarter: (total_false_alarms as f64) / (site_quarters as f64),
        quarters_exceeding_one_alarm,
        frac_quarters_exceeding_one: (quarters_exceeding_one_alarm as f64) / (site_quarters as f64),
        sites_alarmed_in_two_quarters,
    })
}

#[test]
fn test_kill_plumb_6_control_mine_quarterly_false_alarm_rate_and_shift_power() {
    // 1. Control mine: M = 4 streams, eproc_alpha = 0.01 per stream, CUSUM k = 0.5, h = 8.5
    //    (ARL_0 ~= 15,760 >> 85 * 4 = 340 per KILL-PLUMB-6).
    let ctrl = run_control_mine_monitors(0, CONTROL_SITES, BiasVector::unbiased(), 0.01, 0.5, 8.5)
        .expect("control run");

    assert!(
        ctrl.mean_alarms_per_site_quarter <= 0.36,
        "KILL-PLUMB-6 control expected rate {:.4} exceeds 0.36 target (and 1.0 kill gate)",
        ctrl.mean_alarms_per_site_quarter
    );
    assert!(
        ctrl.frac_quarters_exceeding_one <= 0.05,
        "KILL-PLUMB-6 >1 alarm/quarter fraction {:.4} exceeds 5%",
        ctrl.frac_quarters_exceeding_one
    );
    let (_lo, cp_hi) =
        clopper_pearson_interval(ctrl.quarters_exceeding_one_alarm, ctrl.site_quarters, 0.05)
            .expect("clopper-pearson");
    assert!(
        cp_hi <= 0.05,
        "95% CP upper bound on P(alarms > 1 / site-quarter) = {cp_hi:.4} > 0.05"
    );

    // 2. Positive control: planted -5% sampler bias detected without retuning (100 sites).
    let mut biased = BiasVector::unbiased();
    biased.sampler_grade_factor = 0.95; // -5% sampler bias
    let pos =
        run_control_mine_monitors(10_000, 100, biased, 0.01, 0.5, 8.5).expect("positive control");
    let detection_rate_2q = (pos.sites_alarmed_in_two_quarters as f64) / 100.0;
    assert!(
        detection_rate_2q >= 0.90,
        "untuned monitor must detect -5% sampler bias within 2 quarters: got {detection_rate_2q:.3}"
    );

    // 3. Negative control: mis-tuned tight CUSUM h = 1.5 MUST breach KILL-PLUMB-6 (> 1.0/site-qtr).
    let neg = run_control_mine_monitors(20_000, 100, BiasVector::unbiased(), 0.01, 0.5, 1.5)
        .expect("negative control");
    assert!(
        neg.mean_alarms_per_site_quarter > 1.0,
        "mis-tuned CUSUM h=1.5 should breach KILL-PLUMB-6 >1.0/qtr, got {:.3}",
        neg.mean_alarms_per_site_quarter
    );

    let mut enc = Encoder::new_frame(*b"KP06", 1);
    enc.put_u64(ctrl.site_quarters as u64);
    enc.put_u64(ctrl.total_false_alarms as u64);
    enc.put_u64(ctrl.quarters_exceeding_one_alarm as u64);
    enc.put_u64(pos.sites_alarmed_in_two_quarters as u64);
    enc.put_u64(neg.total_false_alarms as u64);
    let digest = blake3_hash(&enc.finish()).to_hex();

    println!(
        "KILL-PLUMB-6 PASS | site_quarters={} | ctrl_alarms={} | rate_per_site_qtr={:.4} (<=0.36, kill<=1.0) | gt1_qtrs={}/{} ({:.4}, cp95_hi={:.4}) | pos_5pct_det_2q={:.2} | neg_mistuned_rate={:.3} | digest={digest}",
        ctrl.site_quarters,
        ctrl.total_false_alarms,
        ctrl.mean_alarms_per_site_quarter,
        ctrl.quarters_exceeding_one_alarm,
        ctrl.site_quarters,
        ctrl.frac_quarters_exceeding_one,
        cp_hi,
        detection_rate_2q,
        neg.mean_alarms_per_site_quarter
    );
}
