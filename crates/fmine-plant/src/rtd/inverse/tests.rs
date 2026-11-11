//! Known-answer controls use one-/two-tank closed forms, independently of the
//! production Poisson kernel. The source declarations are entirely synthetic.
//! Determinism: DeterministicPerIsa

use super::*;
use fmine_claim::{Confidentiality, Representation, SpatialSupport};
use fmine_core::{anchor::SourceAnchor, hash::sha256};
use fmine_evidence::{EpistemicStatus, EvidenceClass, KnowledgeState};

const SECOND: i64 = 1_000_000_000;
type Inputs = (
    Claim<RtdModel>,
    Claim<RtdOpening>,
    Vec<Claim<InverseInterval>>,
    Claim<InverseErrorModel>,
);

fn q(text: &str) -> ExactQty {
    ExactQty::from_decimal_str(text).unwrap()
}
fn element() -> ElementKey {
    ElementKey {
        element: "Au".into(),
        measurand: "total-Au/all-phases".into(),
    }
}
fn scope(start: i64, end: i64) -> FlowsheetScope {
    FlowsheetScope {
        boundary: "synthetic-inverse-train".into(),
        interval: ValidTime {
            start_ns: start,
            end_ns: end,
        },
    }
}
fn masses(gold: &str) -> MaterialAmounts {
    MaterialAmounts {
        dry_solids_kg: q("100"),
        water_kg: q("100"),
        elements_kg: BTreeMap::from([(element(), q(gold))]),
    }
}
fn source<T>(metric: &str, value: T, tag: String, additivity: AdditivityClass) -> Claim<T> {
    Claim::new(
        metric,
        value,
        &tag,
        Representation::Deterministic,
        SpatialSupport::PeriodAggregate,
        additivity,
        EvidenceClass::Assumed,
        EpistemicStatus::KnownKnown,
        KnowledgeState::Known,
        Confidentiality::public(),
        vec![],
        vec![],
        vec![SourceAnchor::new(
            "crates/fmine-plant/src/rtd/inverse/tests.rs",
            sha256(include_bytes!("tests.rs")).0,
            format!("synthetic independent closed-form input {tag}"),
        )],
    )
}
fn model_claim(value: RtdModel) -> Claim<RtdModel> {
    let tag = value.canonical_tag();
    source(MODEL_METRIC, value, tag, AdditivityClass::NonAdditive)
}
fn opening_claim(value: RtdOpening) -> Claim<RtdOpening> {
    let tag = value.canonical_tag();
    source(
        OPENING_METRIC,
        value,
        tag,
        AdditivityClass::ExtensiveAdditive,
    )
}
fn interval_claim(value: InverseInterval) -> Claim<InverseInterval> {
    let tag = value.canonical_tag();
    source(INPUT_METRIC, value, tag, AdditivityClass::ExtensiveAdditive)
}
fn error_claim(value: InverseErrorModel) -> Claim<InverseErrorModel> {
    let tag = value.canonical_tag();
    source(ERROR_MODEL_METRIC, value, tag, AdditivityClass::NonAdditive)
}
fn setup(feeds: &[Option<&str>], outlets: &[&str]) -> Inputs {
    assert_eq!(feeds.len(), outlets.len());
    let end = SECOND * feeds.len() as i64;
    let model = model_claim(RtdModel {
        scope: scope(0, end),
        elements: BTreeSet::from([element()]),
        tanks: 1,
        volume_m3: CubicMetres::new(1.0),
        bypass_fraction: 0.0,
        dead_fraction: 0.0,
        total_flow_domain_m3_per_s: [
            CubicMetresPerSecond::new(0.0),
            CubicMetresPerSecond::new(2.0),
        ],
        common_conservative_transport: true,
        piecewise_constant_feed_per_volume: true,
        closure_tolerances_kg: [
            MaterialChannel::DrySolids,
            MaterialChannel::Water,
            MaterialChannel::Element(element()),
        ]
        .into_iter()
        .map(|c| (c, Kilograms::new(1e-8)))
        .collect(),
        max_kernel_steps: 128,
    });
    let opening = opening_claim(RtdOpening {
        boundary: model.value.scope.boundary.clone(),
        at_ns: 0,
        tanks: vec![masses("2")],
        dead: MaterialAmounts {
            dry_solids_kg: q("0"),
            water_kg: q("0"),
            elements_kg: BTreeMap::from([(element(), q("0"))]),
        },
    });
    let intervals = feeds
        .iter()
        .zip(outlets)
        .enumerate()
        .map(|(i, (feed, outlet))| {
            interval_claim(InverseInterval {
                scope: scope(i as i64 * SECOND, (i + 1) as i64 * SECOND),
                element: element(),
                slurry_volume_m3: CubicMetres::new(1.0),
                feed_kg: feed.map(q),
                outlet_kg: q(outlet),
            })
        })
        .collect::<Vec<_>>();
    let observations = observation_order(&intervals).unwrap();
    let n = observations.len();
    let errors = error_claim(InverseErrorModel {
        scope: model.value.scope.clone(),
        element: element(),
        model_source: model.claim_id,
        opening_source: opening.claim_id,
        observations,
        covariance: (0..n * n)
            .map(|i| KilogramsSquared::new(if i / n == i % n { 1.0 } else { 0.0 }))
            .collect(),
        error_model_basis: "explicit synthetic Gaussian covariance".into(),
        fixed_transport_and_opening: true,
        unbiased_gaussian_errors: true,
        global_chi_square_limit: 100.0,
        normalized_adjustment_limit: None,
        closure_tolerance_kg: Kilograms::new(1e-8),
    });
    (model, opening, intervals, errors)
}
fn rebind(input: &mut Inputs) {
    let mut error = input.3.value.clone();
    error.scope = input.0.value.scope.clone();
    error.model_source = input.0.claim_id;
    error.opening_source = input.1.claim_id;
    error.observations = observation_order(&input.2).unwrap();
    input.3 = error_claim(error);
}
fn run(input: &Inputs) -> Result<InverseOutcome> {
    deconvolve(&input.0, &input.1, &input.2, &input.3, || false)
}
fn close(actual: f64, expected: f64, tolerance: f64) {
    assert!(
        (actual - expected).abs() <= tolerance,
        "{actual:.17e} != {expected:.17e}"
    );
}

#[test]
fn two_missing_feeds_recover_closed_form_and_full_correlated_covariance() {
    // Independent one-tank equations, x=1 and initial stock 2:
    // O0=2+8/e; O1=10-14/e+8/e² for true feeds [10,4].
    // Literals were evaluated at 70 decimal digits, not through the candidate.
    let mut input = setup(
        &[None, None],
        &[
            "4.943035529371538572764190161",
            "5.932370089492709032814663178",
        ],
    );
    let mut error = input.3.value.clone();
    error.covariance = [0.04, 0.01, 0.01, 0.09].map(KilogramsSquared::new).to_vec();
    input.3 = error_claim(error);
    let result = run(&input).unwrap();
    result.admitted_claim().unwrap();
    let r = result.diagnostics();
    close(r.fitted_feed_kg[0].value(), 10.0, 2e-13);
    close(r.fitted_feed_kg[1].value(), 4.0, 2e-13);
    assert_eq!(r.gross_error, GrossErrorDiagnostic::NoRedundancy);
    assert_eq!(r.feed_coordinate_indices, [2, 3]);
    assert_eq!(r.outlet_coordinate_indices, [0, 1]);
    assert_eq!(
        r.unmeasured_feeds,
        input.2.iter().map(|r| r.claim_id).collect::<Vec<_>>()
    );
    let a = (-1.0_f64).exp();
    let b = (1.0 - a).powi(2);
    for (actual, expected) in r.response_matrix.iter().zip([a, 0.0, b, a]) {
        close(*actual, expected, 1e-15);
    }
    // Cov(F)=H^-1 V H^-T. Include the negative temporal cross covariance.
    let g = [1.0 / a, 0.0, -b / (a * a), 1.0 / a];
    let v = [0.04, 0.01, 0.01, 0.09];
    for i in 0..2 {
        for j in 0..2 {
            let mut expected = 0.0;
            for k in 0..2 {
                for l in 0..2 {
                    expected += g[i * 2 + k] * v[k * 2 + l] * g[j * 2 + l];
                }
            }
            close(
                r.posterior_covariance[(i + 2) * 4 + j + 2].value(),
                expected,
                3e-14,
            );
        }
    }
    assert!(r.posterior_covariance[2 * 4 + 3].value() < 0.0);
    close(
        r.closing_inventory_kg[1].value(),
        16.0 - r.fitted_outlet_kg.iter().map(|q| q.value()).sum::<f64>(),
        3e-14,
    );
    assert!(
        r.transport_residual_kg
            .iter()
            .all(|q| q.value().abs() < 1e-13)
    );
    assert!(
        r.conservation_residual_kg
            .iter()
            .all(|q| q.value().abs() < 1e-13)
    );
    assert_eq!(result, run(&input).unwrap());
}

#[test]
fn observed_feed_and_outlet_are_jointly_adjusted_against_independent_gls() {
    let mut input = setup(&[Some("12")], &["4"]);
    let mut error = input.3.value.clone();
    error.covariance = [1.0, 0.1, 0.1, 0.25].map(KilogramsSquared::new).to_vec();
    input.3 = error_claim(error);
    let result = run(&input).unwrap();
    let r = &result.admitted_claim().unwrap().value;
    let a = (-1.0_f64).exp();
    let residual = 4.0 - a * 12.0 - (1.0 - a) * 2.0;
    let denominator = 0.25 + a * a - 0.2 * a;
    let w = [0.1 - a, 0.25 - 0.1 * a];
    for (i, observed) in [12.0, 4.0].into_iter().enumerate() {
        close(
            r.estimates_kg[i].value(),
            observed - w[i] * residual / denominator,
            3e-14,
        );
        close(
            r.measured_adjustments_kg[i].value(),
            -w[i] * residual / denominator,
            3e-14,
        );
        for j in 0..2 {
            let original = [1.0, 0.1, 0.1, 0.25][i * 2 + j];
            close(
                r.posterior_covariance[i * 2 + j].value(),
                original - w[i] * w[j] / denominator,
                3e-15,
            );
        }
    }
    let GrossErrorDiagnostic::Available {
        chi_square,
        degrees_of_freedom,
    } = r.gross_error
    else {
        panic!("missing redundant diagnostic");
    };
    assert_eq!(degrees_of_freedom, 1);
    close(chi_square, residual * residual / denominator, 4e-14);
    assert!(r.measured_adjustments_kg.iter().all(|q| q.value() != 0.0));
    assert_eq!(result.intervals(), input.2);
    assert_eq!(
        result.admitted_claim().unwrap().evidence,
        EvidenceClass::Assumed
    );
    assert_eq!(
        result.admitted_claim().unwrap().state,
        KnowledgeState::Estimated
    );
}

#[test]
fn mixed_missing_and_measured_feed_retains_redundancy_and_cross_period_information() {
    let mut input = setup(&[Some("10"), None], &["5", "6.2"]);
    let mut error = input.3.value.clone();
    let v = [1.0, 0.1, 0.2, 0.1, 0.5, 0.15, 0.2, 0.15, 0.8];
    error.covariance = v.map(KilogramsSquared::new).to_vec();
    input.3 = error_claim(error);
    let result = run(&input).unwrap();
    let r = &result.admitted_claim().unwrap().value;
    let a = (-1.0_f64).exp();
    let b = (1.0 - a).powi(2);
    let constraint = [-a, 1.0, 0.0];
    let w: Vec<_> = (0..3)
        .map(|i| (0..3).map(|j| v[i * 3 + j] * constraint[j]).sum::<f64>())
        .collect();
    let denominator: f64 = (0..3).map(|i| constraint[i] * w[i]).sum();
    let residual = 5.0 - 10.0 * a - 2.0 * (1.0 - a);
    let y = [10.0, 5.0, 6.2];
    let measured: Vec<_> = (0..3)
        .map(|i| y[i] - w[i] * residual / denominator)
        .collect();
    let missing = (measured[2] - 2.0 * a * (1.0 - a) - b * measured[0]) / a;
    for (actual, expected) in r
        .estimates_kg
        .iter()
        .zip(measured.iter().copied().chain([missing]))
    {
        close(actual.value(), expected, 8e-14);
    }
    assert_ne!(r.measured_adjustments_kg[2].value(), 0.0);
    let mut jacobian = vec![0.0; 4 * 3];
    for i in 0..3 {
        for j in 0..3 {
            jacobian[i * 3 + j] =
                if i == j { 1.0 } else { 0.0 } - w[i] * constraint[j] / denominator;
        }
    }
    for j in 0..3 {
        jacobian[9 + j] = (jacobian[6 + j] - b * jacobian[j]) / a;
    }
    for i in 0..4 {
        for j in 0..4 {
            let mut expected = 0.0;
            for k in 0..3 {
                for l in 0..3 {
                    expected += jacobian[i * 3 + k] * v[k * 3 + l] * jacobian[j * 3 + l];
                }
            }
            close(r.posterior_covariance[i * 4 + j].value(), expected, 8e-14);
        }
    }
    assert!(matches!(
        r.gross_error,
        GrossErrorDiagnostic::Available {
            degrees_of_freedom: 1,
            ..
        }
    ));
}

#[test]
fn physical_negative_and_gross_error_fits_keep_diagnostics_without_claims() {
    let negative = setup(&[None], &["0"]);
    let result = run(&negative).unwrap();
    assert_eq!(result.refusal().unwrap().code, "FMINE-REF-109");
    assert!(result.admitted_claim().is_err());
    assert!(result.diagnostics().fitted_feed_kg[0].value() < 0.0);
    assert_eq!(result.intervals()[0].value.outlet_kg, q("0"));

    let mut gross = setup(&[Some("10")], &["20"]);
    let mut error = gross.3.value.clone();
    error.global_chi_square_limit = 1.0;
    gross.3 = error_claim(error);
    let result = run(&gross).unwrap();
    assert_eq!(result.refusal().unwrap().code, "FMINE-REF-101");
    assert!(result.admitted_claim().is_err());
    assert_eq!(result.intervals(), gross.2);
    assert!(
        matches!(result.diagnostics().gross_error, GrossErrorDiagnostic::Available { chi_square, .. } if chi_square > 1.0)
    );
    let mut error = gross.3.value.clone();
    error.global_chi_square_limit = 10000.0;
    error.normalized_adjustment_limit = Some(0.1);
    gross.3 = error_claim(error);
    assert!(
        run(&gross)
            .unwrap()
            .refusal()
            .unwrap()
            .what
            .contains("simultaneous")
    );
}

#[test]
fn two_tank_independent_solution_and_throughput_clock_equivalence() {
    // Two tanks, x=1, opening [1,3], constant feed 2: O=2+e^-1.
    let mut input = setup(&[None], &["2.367879441171442321595523770"]);
    let mut model = input.0.value.clone();
    model.tanks = 2;
    model.volume_m3 = CubicMetres::new(2.0);
    input.0 = model_claim(model);
    let mut opening = input.1.value.clone();
    opening.tanks = vec![masses("1"), masses("3")];
    input.1 = opening_claim(opening);
    rebind(&mut input);
    let original = run(&input).unwrap();
    close(
        original.admitted_claim().unwrap().value.fitted_feed_kg[0].value(),
        2.0,
        2e-13,
    );
    close(
        original.diagnostics().response_matrix[0],
        3.0 * (-1.0_f64).exp() - 1.0,
        6e-16,
    );
    let mut model = input.0.value.clone();
    model.scope.interval.end_ns = 100 * SECOND;
    input.0 = model_claim(model);
    let mut row = input.2[0].value.clone();
    row.scope.interval.end_ns = 100 * SECOND;
    input.2[0] = interval_claim(row);
    rebind(&mut input);
    let slowed = run(&input).unwrap();
    assert_eq!(
        original.diagnostics().fitted_feed_kg,
        slowed.diagnostics().fitted_feed_kg
    );
    assert_eq!(
        original.diagnostics().response_matrix,
        slowed.diagnostics().response_matrix
    );
    assert_ne!(original.digest(), slowed.digest());
}

#[test]
fn complete_bypass_dead_stock_and_shutdown_are_explicit() {
    let mut input = setup(&[None], &["7"]);
    let mut model = input.0.value.clone();
    model.bypass_fraction = 1.0;
    model.dead_fraction = 0.25;
    input.0 = model_claim(model);
    let mut opening = input.1.value.clone();
    opening.dead = masses("3");
    input.1 = opening_claim(opening);
    rebind(&mut input);
    let result = run(&input).unwrap();
    let r = &result.admitted_claim().unwrap().value;
    assert_eq!(r.fitted_feed_kg[0].value(), 7.0);
    assert_eq!(r.predicted_outlet_kg[0].value(), 7.0);
    assert_eq!(r.closing_inventory_kg[0].value(), 5.0);
    assert_eq!(r.response_matrix, [1.0]);
    assert_eq!(r.kernel_steps, 0);

    let mut row = input.2[0].value.clone();
    row.slurry_volume_m3 = CubicMetres::new(0.0);
    row.feed_kg = Some(q("0"));
    row.outlet_kg = q("0");
    input.2[0] = interval_claim(row);
    let mut error = input.3.value.clone();
    error.covariance = [1.0, 0.0, 0.0, 1.0].map(KilogramsSquared::new).to_vec();
    input.3 = error_claim(error);
    rebind(&mut input);
    let r = run(&input).unwrap();
    assert_eq!(
        r.admitted_claim().unwrap().value.closing_inventory_kg[0].value(),
        5.0
    );
    assert_eq!(r.diagnostics().predicted_outlet_kg[0].value(), 0.0);
    let mut row = input.2[0].value.clone();
    row.feed_kg = None;
    input.2[0] = interval_claim(row);
    let mut error = input.3.value.clone();
    error.covariance = vec![KilogramsSquared::new(1.0)];
    input.3 = error_claim(error);
    rebind(&mut input);
    assert_eq!(run(&input).unwrap_err().code, "FMINE-REF-104");
}

#[test]
fn weak_transfer_reports_large_uncertainty_instead_of_regularizing() {
    let mut input = setup(&[None], &["0.0000000005"]);
    let mut opening = input.1.value.clone();
    opening.tanks = vec![masses("0")];
    input.1 = opening_claim(opening);
    let mut row = input.2[0].value.clone();
    row.slurry_volume_m3 = CubicMetres::new(1e-10);
    input.2[0] = interval_claim(row);
    rebind(&mut input);
    let result = run(&input).unwrap();
    let r = &result.admitted_claim().unwrap().value;
    // Single tank feed exit = x/2 - x²/6 + x³/24 ... . No cancellation-prone
    // subtraction of 1-exp(-x) is used in this independent small-x control.
    let x: f64 = 1e-10;
    let h = x / 2.0 - x.powi(2) / 6.0 + x.powi(3) / 24.0;
    close(r.response_matrix[0], h, 2e-25);
    close(r.fitted_feed_kg[0].value(), 0.0000000005 / h, 5e-14);
    let expected = 1.0 / (h * h);
    close(r.posterior_covariance[3].value() / expected, 1.0, 3e-15);
    assert!(r.posterior_covariance[3].value() > 1e20);
    assert_eq!(r.gross_error, GrossErrorDiagnostic::NoRedundancy);
}

#[test]
fn source_identity_covariance_domain_work_and_cancellation_fail_closed() {
    let input = setup(&[Some("10"), None], &["5", "6"]);
    let original = run(&input).unwrap();
    let mut reversed = input.clone();
    reversed.2.reverse();
    assert_eq!(original, run(&reversed).unwrap());
    let mut changed = input.clone();
    changed.2[0].value.outlet_kg = q("5.1");
    assert_eq!(run(&changed).unwrap_err().code, "FMINE-REF-507");
    changed.2[0] = interval_claim(changed.2[0].value.clone());
    assert_eq!(run(&changed).unwrap_err().code, "FMINE-REF-102");
    rebind(&mut changed);
    assert_ne!(original.digest(), run(&changed).unwrap().digest());
    let mut error = input.3.value.clone();
    error.covariance[1] = KilogramsSquared::new(0.5);
    changed = input.clone();
    changed.3 = error_claim(error);
    assert!(run(&changed).is_err());
    let mut model = input.0.value.clone();
    model.max_kernel_steps = 1;
    changed = input.clone();
    changed.0 = model_claim(model);
    rebind(&mut changed);
    assert!(run(&changed).unwrap_err().what.contains("budget"));
    let mut model = input.0.value.clone();
    model.total_flow_domain_m3_per_s[1] = CubicMetresPerSecond::new(0.5);
    changed = input.clone();
    changed.0 = model_claim(model);
    rebind(&mut changed);
    assert_eq!(run(&changed).unwrap_err().code, "FMINE-REF-203");
    let mut calls = 0;
    let failure = deconvolve(&input.0, &input.1, &input.2, &input.3, || {
        calls += 1;
        calls >= 5
    })
    .unwrap_err();
    assert!(failure.what.contains("cancelled"));
    assert_eq!(original, run(&input).unwrap());
}

#[test]
fn inverse_worksheet_rejects_bytes_not_bound_to_its_source() {
    let anchor = SourceAnchor::new("synthetic/inverse.toml", [0; 32], "worksheet");
    let error = worksheet::evaluate("schema = \"synthetic\"\n", &anchor).unwrap_err();
    assert_eq!(error.code, "FMINE-REF-505");
}
