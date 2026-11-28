//! Regression controls for finite, atomic sequential monitor updates.
use fmine_drill::records::AssayMethod;
use fmine_qaqc::crm::SdBasis;
use fmine_qaqc::monitors::{
    CrmMonitorConfig, CrmReferenceParams, CrmSequentialMonitor, CrmStreamKey, CrmStreamValue,
    MonitorKind,
};

fn config(mean: f64, sd: f64) -> CrmMonitorConfig {
    let mut c = CrmMonitorConfig::with_reference(
        CrmStreamKey {
            crm_id: "synthetic".into(),
            lab: "synthetic".into(),
            method: AssayMethod::FireAssayAas,
            sd_basis: SdBasis::CertifiedWithinLabSigmaC,
        },
        CrmReferenceParams {
            certified_mean: mean,
            reference_sd: sd,
        },
    );
    c.dispersion_window = 6;
    c
}

#[test]
fn refused_standardization_preserves_every_monitor_state() {
    let mut monitor = CrmSequentialMonitor::new(config(0.0, f64::MIN_POSITIVE)).expect("config");
    monitor
        .observe(CrmStreamValue {
            measured: f64::MIN_POSITIVE,
        })
        .expect("valid prefix");
    let before = monitor.snapshot();
    for value in [f64::INFINITY, f64::NAN, f64::MAX] {
        assert!(monitor.observe(CrmStreamValue { measured: value }).is_err());
        assert_eq!(monitor.snapshot(), before);
    }
    let mut monitor = CrmSequentialMonitor::new(config(0.0, f64::MAX)).expect("config");
    let before = monitor.snapshot();
    assert!(
        monitor
            .observe(CrmStreamValue {
                measured: f64::MIN_POSITIVE
            })
            .is_err()
    );
    assert_eq!(monitor.snapshot(), before);
}

#[test]
fn enormous_finite_dispersion_is_not_silently_reported_as_collapse() {
    let mut monitor = CrmSequentialMonitor::new(config(0.0, 1.0)).expect("config");
    for value in [1e200, -1e200, 1e200, -1e200, 1e200, -1e200] {
        monitor
            .observe(CrmStreamValue { measured: value })
            .expect("finite normalized dispersion");
    }
    let snapshot = monitor.snapshot();
    let ratio = snapshot.latest_dispersion_ratio.expect("full window");
    assert!(ratio.is_finite() && (ratio / 1e200 - (6.0_f64 / 5.0).sqrt()).abs() < 1e-14);
    assert_eq!(snapshot.dispersion_collapse_step, None);
    assert!(
        !snapshot
            .alarms
            .iter()
            .any(|a| a.monitor == MonitorKind::DispersionCollapse)
    );
}

#[test]
fn overflowing_dispersion_is_refused_before_advancing_the_prefix() {
    let mut monitor = CrmSequentialMonitor::new(config(0.0, 1.0)).expect("config");
    for value in [f64::MAX, -f64::MAX, f64::MAX, -f64::MAX, f64::MAX] {
        monitor
            .observe(CrmStreamValue { measured: value })
            .expect("prefix");
    }
    let before = monitor.snapshot();
    assert!(
        monitor
            .observe(CrmStreamValue {
                measured: -f64::MAX
            })
            .is_err()
    );
    assert_eq!(monitor.snapshot(), before);
    monitor
        .observe(CrmStreamValue { measured: 0.0 })
        .expect("usable continuation");
    assert_eq!(monitor.snapshot().n_steps, 6);
}
