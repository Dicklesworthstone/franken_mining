use super::*;
use fmine_core::{anchor::SourceAnchor, hash::sha256};
use fmine_evidence::{EvidenceClass, KnowledgeState};

const TEXT: &str = include_str!("../../../../../tests/costs/fitted_driver.toml");
const COSTS: &str = include_str!("../../../../../tests/costs/fitted_costs.toml");
const REGISTRY: &str = include_str!("../../../../../registries/metrics.toml");

fn q(n: i128, d: i128) -> Rat {
    Rat::new(n, d).unwrap()
}
fn anchor(text: &str) -> SourceAnchor {
    SourceAnchor::new(
        "synthetic-costs.toml",
        sha256(text.as_bytes()).0,
        "worksheet",
    )
}
fn run(text: &str) -> DriverReceipt {
    worksheet::evaluate(text, anchor(text)).unwrap()
}
fn retag(c: &mut Claim<DriverInput>) {
    *c = Claim::new(
        INPUT_METRIC,
        c.value.clone(),
        &c.value.canonical_tag(),
        c.representation.clone(),
        c.support,
        c.additivity,
        c.evidence,
        c.epistemic,
        c.state,
        c.confidentiality.clone(),
        c.uu_terms.clone(),
        c.parents.clone(),
        c.anchors.clone(),
    );
}

#[test]
fn exact_fit_retains_signed_residuals_covariance_and_cost_definition_lineage() {
    // Independent centered OLS arithmetic: xbar=250, ybar=600, Sxx=50000,
    // Sxy=90000; residuals=(-30,90,-90,30); SSE=18000; df=2.
    let r = run(TEXT);
    let s = r.summary();
    assert_eq!(s.fixed_usd, q(150, 1));
    assert_eq!(s.usd_per_unit, q(9, 5));
    assert_eq!(s.centered_driver_sum_squares, q(50000, 1));
    assert_eq!(s.residual_sum_squares_usd2, q(18000, 1));
    assert_eq!(s.residual_variance_usd2, q(9000, 1));
    assert_eq!(
        s.coefficient_covariance,
        [q(13500, 1), q(-45, 1), q(-45, 1), q(9, 50)]
    );
    assert_eq!(
        s.rows.iter().map(|r| r.residual_usd).collect::<Vec<_>>(),
        [-30, 90, -90, 30].map(Rat::from_integer)
    );
    assert_eq!(s.mean_usd, q(600, 1));
    assert_eq!(s.mean_variance_usd2, q(2250, 1));
    assert_eq!(s.observation_variance_usd2, q(11250, 1));
    assert_eq!(r.claim().state, KnowledgeState::Estimated);
    assert_eq!(r.claim().evidence, EvidenceClass::Assumed);
    let costs = super::super::worksheet::evaluate_with_fitted_driver(
        COSTS,
        anchor(COSTS),
        REGISTRY,
        anchor(REGISTRY),
        "aisc@wgc-2018",
        &r,
    )
    .unwrap();
    assert_eq!(costs.summary().usd_per_unit, q(6, 1));
    let predicted = r.predicted_line();
    assert!(predicted.parents.contains(&r.claim().claim_id));
    assert!(costs.claim().parents.contains(&predicted.claim_id));
    assert_eq!(costs.claim().state, KnowledgeState::Estimated);
    assert!(r.to_json().contains("synthetic-register-2026"));
    assert!(r.to_markdown().contains("not calibrated intervals"));
    let away = run(&TEXT.replace("quantity = \"250\"", "quantity = \"350\""));
    assert_eq!(away.summary().mean_usd, q(780, 1));
    assert_eq!(away.summary().mean_variance_usd2, q(4050, 1));
    assert_eq!(away.summary().observation_variance_usd2, q(13050, 1));
}

#[test]
fn equal_drivers_refuse_identifiability_and_extrapolation_is_never_silent() {
    let r = run(TEXT);
    let mut rows = r.inputs().to_vec();
    for c in &mut rows {
        match &mut c.value.value {
            DriverValue::Observation { quantity, .. } | DriverValue::Target { quantity } => {
                *quantity = q(100, 1)
            }
            DriverValue::Model {
                quantity_domain, ..
            } => *quantity_domain = [q(100, 1); 2],
        }
        retag(c);
    }
    assert_eq!(fit(&rows).unwrap_err().code, "FMINE-REF-104");
    for text in [
        TEXT.replace("quantity = \"250\"", "quantity = \"401\""),
        TEXT.replace("domain_min = \"100\"", "domain_min = \"0\""),
    ] {
        assert_eq!(
            worksheet::evaluate(&text, anchor(&text)).unwrap_err().code,
            "FMINE-REF-203"
        );
    }
}

#[test]
fn immutable_values_sources_and_unrendered_metadata_bind_the_fit() {
    let original = run(TEXT);
    let mut inputs = original.inputs().to_vec();
    let DriverValue::Observation { quantity, .. } = &mut inputs[1].value.value else {
        panic!("observation");
    };
    *quantity = q(101, 1);
    assert_eq!(fit(&inputs).unwrap_err().code, "FMINE-REF-507");
    retag(&mut inputs[1]);
    assert_eq!(fit(&inputs).unwrap_err().code, "FMINE-REF-203");
    let changed = run(&format!("{TEXT}\n# independent source revision\n"));
    assert_eq!(changed.summary().mean_usd, original.summary().mean_usd);
    assert_ne!(changed.digest(), original.digest());
    let mut inputs = original.inputs().to_vec();
    inputs[1].supersedes = Some(original.claim().claim_id);
    assert_ne!(fit(&inputs).unwrap().digest(), original.digest());
    inputs.reverse();
    assert_eq!(
        fit(&inputs).unwrap().digest(),
        fit(&inputs.iter().cloned().rev().collect::<Vec<_>>())
            .unwrap()
            .digest()
    );
    assert_eq!(
        worksheet::evaluate(&format!("{TEXT} "), anchor(TEXT))
            .unwrap_err()
            .code,
        "FMINE-REF-505"
    );
}

#[test]
fn scope_time_support_and_duplicate_cost_items_fail_closed() {
    let original = run(TEXT);
    for change in 0..6 {
        let mut inputs = original.inputs().to_vec();
        match change {
            0 => inputs[1].value.scope.subject = "Other site".into(),
            1 => inputs[1].value.scope.ownership = Ownership::Attributable { share: q(1, 2) },
            2 => inputs[1].value.scope.period.last_month = 3,
            3 => inputs[1].value.scope.period = inputs[2].value.scope.period,
            4 => {
                if let DriverValue::Observation { available, .. } = &mut inputs[1].value.value {
                    available.year = 2030;
                }
            }
            _ => {
                if let DriverValue::Model { unit, .. } = &mut inputs[0].value.value {
                    *unit = "kg".into();
                }
            }
        }
        for input in &mut inputs {
            retag(input);
        }
        assert!(fit(&inputs).is_err(), "change {change}");
    }
    let repeated = format!("{COSTS}\n[[cost]]\nitem = \"on_site_mining_costs\"\nusd = \"600\"\n");
    assert_eq!(
        super::super::worksheet::evaluate_with_fitted_driver(
            &repeated,
            anchor(&repeated),
            REGISTRY,
            anchor(REGISTRY),
            "aisc@wgc-2018",
            &original
        )
        .unwrap_err()
        .code,
        "FMINE-REF-105"
    );
}

#[test]
fn coefficients_remain_signed_and_nearest_even_rounding_is_explicit() {
    let r = run(TEXT);
    let mut inputs = r.inputs().to_vec();
    for c in &mut inputs {
        if let DriverValue::Observation { usd, .. } = &mut c.value.value {
            usd.minor = -usd.minor;
        }
        retag(c);
    }
    let s = fit(&inputs).unwrap();
    assert_eq!(s.summary().fixed_usd, q(-150, 1));
    assert_eq!(s.summary().usd_per_unit, q(-9, 5));
    assert_eq!(s.summary().mean_usd, q(-600, 1));
    for (n, want) in [(1, 0), (3, 2), (5, 2), (-1, 0), (-3, -2), (-5, -2)] {
        assert_eq!(round_money(q(n, 20000)).unwrap().minor, want);
    }
    let rounded = run(&TEXT.replace("quantity = \"250\"", "quantity = \"250.00003\""));
    assert_eq!(rounded.summary().mean_usd, q(300000027, 500000));
    assert_eq!(rounded.summary().rounded_usd.minor, 6_000_001);
    assert_eq!(rounded.summary().rounding_delta_usd, q(23, 500000));
    let target = TEXT
        .replace("quantity = \"250\"", "quantity = \"1/3\"")
        .replace("domain_min = \"100\"", "domain_min = \"0\"");
    assert!(worksheet::evaluate(&target, anchor(&target)).is_err());
    let undeclared = TEXT.replace(ROUNDING, "implicit");
    assert_eq!(
        worksheet::evaluate(&undeclared, anchor(&undeclared))
            .unwrap_err()
            .code,
        "FMINE-REF-107"
    );
}

#[test]
fn rescaling_the_declared_driver_preserves_costs_and_transforms_covariance() {
    let original = run(TEXT);
    let mut inputs = original.inputs().to_vec();
    for c in &mut inputs {
        match &mut c.value.value {
            DriverValue::Observation { quantity, .. } | DriverValue::Target { quantity } => {
                *quantity = quantity.checked_mul(q(2, 1)).unwrap();
            }
            DriverValue::Model {
                quantity_domain, ..
            } => {
                for x in quantity_domain {
                    *x = x.checked_mul(q(2, 1)).unwrap();
                }
            }
        }
        retag(c);
    }
    let scaled = fit(&inputs).unwrap();
    assert_eq!(scaled.summary().fixed_usd, original.summary().fixed_usd);
    assert_eq!(scaled.summary().usd_per_unit, q(9, 10));
    assert_eq!(
        scaled.summary().coefficient_covariance,
        [q(13500, 1), q(-45, 2), q(-45, 2), q(9, 200)]
    );
    assert_eq!(scaled.summary().mean_usd, original.summary().mean_usd);
    assert_eq!(
        scaled.summary().mean_variance_usd2,
        original.summary().mean_variance_usd2
    );
}

#[test]
fn strict_schema_and_evidence_cap_do_not_upgrade_fitted_inputs() {
    let r = run(TEXT);
    let mut inputs = r.inputs().to_vec();
    for c in &mut inputs {
        c.evidence = EvidenceClass::Measured;
        retag(c);
    }
    let fitted = fit(&inputs).unwrap();
    assert_eq!(fitted.claim().evidence, EvidenceClass::ModelledUncalibrated);
    assert_eq!(
        fitted.predicted_line().evidence,
        EvidenceClass::ModelledUncalibrated
    );
    for text in [
        TEXT.replace(
            "unit = \"dry_t\"",
            "unit = \"dry_t\"\nimplicit_conversion = \"true\"",
        ),
        TEXT.replace("currency = \"USD\"", "currency = \"CAD\""),
        TEXT.replace("[[observation]]", "[[unknown]]"),
    ] {
        assert_eq!(
            worksheet::evaluate(&text, anchor(&text)).unwrap_err().code,
            "FMINE-REF-506"
        );
    }
}
