//! `GATE-CORE-02` verification harness (`BD-169`, `BD-283`, plan §12.4, §12.6):
//! - Anytime-valid e-process drift detection: detects 1 SD drift within documented delay with
//!   null false alarms <= alpha across seeded synthetic streams.
//! - Dispersion-collapse tampering monitor: alarms on injected variance collapse (running SD at
//!   0.3 of certified) with `AlarmScope::Integrity` (never recalibration), plus Leaver et al.
//!   pattern rules (2 consecutive > ±2 SD, 4 consecutive > ±1 SD same side, sign imbalance).

use fmine_core::det;
use fmine_core::rand::{PhiloxStream, RngKey};
use fmine_drill::records::AssayMethod;
use fmine_qaqc::crm::SdBasis;
use fmine_qaqc::monitors::{
    AlarmScope, CrmMonitorConfig, CrmReferenceParams, CrmSequentialMonitor, CrmStreamKey,
    CrmStreamValue, MonitorKind,
};

fn next_std_normal(rng: &mut PhiloxStream) -> f64 {
    let u1 = rng.next_f64_open01();
    let u2 = rng.next_f64_open01();
    det::sqrt(-2.0 * det::ln(u1)) * det::cos(2.0 * std::f64::consts::PI * u2)
}

fn stream_config() -> CrmMonitorConfig {
    CrmMonitorConfig::with_reference(
        CrmStreamKey {
            crm_id: "OREAS-264".to_string(),
            lab: "ALS".to_string(),
            method: AssayMethod::FireAssayAas,
            sd_basis: SdBasis::CertifiedWithinLabSigmaC,
        },
        CrmReferenceParams {
            certified_mean: 2.0e-6,
            reference_sd: 0.1e-6,
        },
    )
}

#[test]
fn test_gate_core_02_eprocess_null_fpr_and_1sd_drift_detection_delay() {
    let cfg = stream_config();
    let alpha = cfg.eproc_alpha;
    let mu = cfg.certified_mean;
    let sigma = cfg.reference_sd;

    // 1. Null streams: 400 independent streams x 100 steps under H_0: X_t ~ N(mu, sigma^2).
    // Ville's inequality bounds anytime false-alarm rate <= alpha (0.05).
    let n_null = 400usize;
    let horizon = 100usize;
    let mut null_alarms = 0usize;

    for trial in 0..n_null {
        let mut rng = PhiloxStream::new(RngKey {
            seed_root: 202_602,
            partition: 1,
            domain: 1,
            var_group: 1,
            realization: trial as u32,
            brick: 0,
        });
        let mut mon = CrmSequentialMonitor::new(cfg.clone()).expect("monitor init");
        for _ in 0..horizon {
            let x = mu + sigma * next_std_normal(&mut rng);
            mon.observe(CrmStreamValue { measured: x })
                .expect("observe");
        }
        if mon.snapshot().eproc_alarm_step.is_some() {
            null_alarms += 1;
        }
    }
    let empirical_fpr = (null_alarms as f64) / (n_null as f64);
    assert!(
        empirical_fpr <= alpha,
        "GATE-CORE-02 null FPR {empirical_fpr:.4} ({null_alarms}/{n_null}) exceeded alpha={alpha}"
    );

    // 2. 1.0 SD drift alternative: 200 independent streams x 100 steps with X_t ~ N(mu + 1.0*sigma, sigma^2).
    let n_alt = 200usize;
    let mut delays = Vec::with_capacity(n_alt);
    for trial in 0..n_alt {
        let mut rng = PhiloxStream::new(RngKey {
            seed_root: 202_602,
            partition: 2,
            domain: 1,
            var_group: 1,
            realization: trial as u32,
            brick: 0,
        });
        let mut mon = CrmSequentialMonitor::new(cfg.clone()).expect("monitor init");
        for _ in 0..horizon {
            let x = mu + 1.0 * sigma + sigma * next_std_normal(&mut rng);
            mon.observe(CrmStreamValue { measured: x })
                .expect("observe");
        }
        let snap = mon.snapshot();
        let step = snap
            .eproc_alarm_step
            .expect("1 SD drift must trigger e-process within 100 steps");
        delays.push(step);
        let eproc_alarm = snap
            .alarms
            .iter()
            .find(|a| a.monitor == MonitorKind::EProcessDrift)
            .expect("eproc alarm recorded");
        assert_eq!(eproc_alarm.scope, AlarmScope::CalibrationOrBias);
    }

    delays.sort_unstable();
    let median_delay = delays[n_alt / 2];
    let p95_delay = delays[(n_alt * 95) / 100];
    assert!(
        median_delay <= 35,
        "GATE-CORE-02 1 SD drift median delay {median_delay} exceeded documented bound 35 (p95={p95_delay})"
    );
}

#[test]
fn test_gate_core_02_dispersion_collapse_integrity_alarm_and_pattern_monitors() {
    let cfg = stream_config();
    let mu = cfg.certified_mean;
    let sigma = cfg.reference_sd;

    // 1. Injected variance collapse (GATE-CORE-02: running SD at 0.3 of certified after 20 clean steps).
    let n_trials = 200usize;
    let warmup = 20usize;
    let post_steps = 30usize;
    let mut detected = 0usize;
    let mut delays = Vec::new();

    for trial in 0..n_trials {
        let mut rng = PhiloxStream::new(RngKey {
            seed_root: 202_603,
            partition: 1,
            domain: 1,
            var_group: 1,
            realization: trial as u32,
            brick: 0,
        });
        let mut mon = CrmSequentialMonitor::new(cfg.clone()).expect("monitor init");
        for _ in 0..warmup {
            let x = mu + sigma * next_std_normal(&mut rng);
            mon.observe(CrmStreamValue { measured: x })
                .expect("observe");
        }
        for _ in 0..post_steps {
            // Tampering: values tightly clustered around certified mean (0.3 * sigma).
            let x = mu + 0.3 * sigma * next_std_normal(&mut rng);
            mon.observe(CrmStreamValue { measured: x })
                .expect("observe");
        }
        let snap = mon.snapshot();
        if let Some(step) = snap.dispersion_collapse_step {
            detected += 1;
            if step > warmup {
                delays.push(step - warmup);
            }
            let alarm = snap
                .alarms
                .iter()
                .find(|a| a.monitor == MonitorKind::DispersionCollapse)
                .expect("dispersion collapse alarm");
            // Plan §12.4 (E368): dispersion collapse MUST open an Integrity case, NEVER recalibration.
            assert_eq!(alarm.scope, AlarmScope::Integrity);
        }
    }
    let detection_rate = (detected as f64) / (n_trials as f64);
    assert!(
        detection_rate >= 0.98,
        "GATE-CORE-02 variance-collapse detection rate {detection_rate:.3} ({detected}/{n_trials}) < 0.98"
    );
    delays.sort_unstable();
    let median_delay = delays[delays.len() / 2];
    assert!(
        median_delay <= 20,
        "GATE-CORE-02 variance-collapse median delay {median_delay} exceeded 20 steps"
    );

    // 2. Clean control streams verify low false-alarm rate on dispersion collapse.
    let mut clean_collapse_fp = 0usize;
    for trial in 0..200usize {
        let mut rng = PhiloxStream::new(RngKey {
            seed_root: 202_603,
            partition: 2,
            domain: 1,
            var_group: 1,
            realization: trial as u32,
            brick: 0,
        });
        let mut mon = CrmSequentialMonitor::new(cfg.clone()).expect("monitor init");
        for _ in 0..60 {
            let x = mu + sigma * next_std_normal(&mut rng);
            mon.observe(CrmStreamValue { measured: x })
                .expect("observe");
        }
        if mon.snapshot().dispersion_collapse_step.is_some() {
            clean_collapse_fp += 1;
        }
    }
    assert!(
        clean_collapse_fp <= 6,
        "clean stream dispersion-collapse false positives too high: {clean_collapse_fp}/200"
    );

    // 3. Pattern monitors (E367): 2 consecutive > 2 SD, 4 consecutive > 1 SD same side, sign imbalance.
    let mut pat = CrmSequentialMonitor::new(cfg).expect("pattern monitor");
    // 12 positive deviations, including 4 consecutive > +1 SD and 2 consecutive > +2 SD.
    let zs = [
        0.5, 1.2, 1.3, 1.4, 1.5, // step 5 triggers 4 consec > +1 SD
        0.3, 2.2, 2.4, // step 8 triggers 2 consec > +2 SD
        0.4, 0.6, 0.2,
        0.8, // step 12 triggers SignImbalance (12+/0-, p = 2 * 2^-12 = 0.000488 <= 0.01)
    ];
    for &z in &zs {
        pat.observe(CrmStreamValue {
            measured: mu + z * sigma,
        })
        .expect("observe");
    }
    let snap = pat.snapshot();
    assert!(snap.alarms.iter().any(|a| {
        a.monitor == MonitorKind::FourConsecutiveBeyond1Sd
            && a.step == 5
            && a.scope == AlarmScope::CalibrationOrBias
    }));
    assert!(snap.alarms.iter().any(|a| {
        a.monitor == MonitorKind::TwoConsecutiveBeyond2Sd
            && a.step == 8
            && a.scope == AlarmScope::CalibrationOrBias
    }));
    assert!(snap.alarms.iter().any(|a| {
        a.monitor == MonitorKind::SignImbalance
            && a.step == 12
            && a.scope == AlarmScope::CalibrationOrBias
    }));

    // 4. Copy-paste constant tampering (ratio_sd == 0.0) triggers DispersionCollapse with Integrity scope immediately on window completion.
    let mut copied = CrmSequentialMonitor::new(stream_config()).expect("copied monitor");
    for _ in 0..15 {
        copied
            .observe(CrmStreamValue { measured: mu })
            .expect("observe");
    }
    let copied_snap = copied.snapshot();
    assert_eq!(copied_snap.dispersion_collapse_step, Some(15));
    assert!(copied_snap.alarms.iter().any(|a| {
        a.monitor == MonitorKind::DispersionCollapse
            && a.scope == AlarmScope::Integrity
            && a.statistic == 0.0
    }));

    // 5. Long-horizon (2,000-step) strongly drifted stream keeps log-wealth & display wealth
    //    strictly finite (no f64 infinity overflow or zero underflow over multi-year streams).
    let mut long_mon = CrmSequentialMonitor::new(stream_config()).expect("long monitor");
    for _ in 0..2_000 {
        long_mon
            .observe(CrmStreamValue {
                measured: mu + 2.5 * sigma,
            })
            .expect("observe");
    }
    let long_snap = long_mon.snapshot();
    assert!(long_snap.eproc_log_wealth.is_finite() && long_snap.eproc_log_wealth > 500.0);
    assert!(long_snap.eproc_max_log_wealth.is_finite());
    assert!(long_snap.eproc_wealth.is_finite());
    assert!(long_snap.eproc_alarm_step.is_some());
}
