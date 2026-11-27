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
        fixed_transport: true,
        opening_treatment: OpeningTreatment::Fixed,
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
    error.observations =
        observation_order_with_opening(&input.1, &input.2, error.opening_treatment).unwrap();
    input.3 = error_claim(error);
}
fn measured_opening(input: &mut Inputs, covariance: &[f64]) {
    let mut error = input.3.value.clone();
    error.opening_treatment = OpeningTreatment::Measured;
    error.covariance = covariance
        .iter()
        .copied()
        .map(KilogramsSquared::new)
        .collect();
    input.3 = error_claim(error);
    rebind(input);
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
fn measured_opening_joint_gls_matches_independent_one_tank_answer() {
    // Independent x=1 CSTR equation: O=(1-a)S+aF, a=exp(-1).
    // Observations [S,F,O] share a full covariance, including both stock/flow pairs.
    let mut input = setup(&[Some("12")], &["4"]);
    let v = [0.25, 0.05, -0.02, 0.05, 1.0, 0.1, -0.02, 0.1, 0.16];
    measured_opening(&mut input, &v);
    let result = run(&input).unwrap();
    let r = &result.admitted_claim().unwrap().value;
    assert_eq!(r.opening_coordinate_indices, [0]);
    assert_eq!(r.feed_coordinate_indices, [1]);
    assert_eq!(r.outlet_coordinate_indices, [2]);
    assert_eq!(
        r.observations[0],
        InverseObservation {
            source: input.1.claim_id,
            role: ObservationRole::OpeningTank(0),
        }
    );
    let a = (-1.0_f64).exp();
    let c = [a - 1.0, -a, 1.0];
    let y = [2.0, 12.0, 4.0];
    let w: Vec<_> = (0..3)
        .map(|i| (0..3).map(|j| v[i * 3 + j] * c[j]).sum::<f64>())
        .collect();
    let residual: f64 = c.iter().zip(y).map(|(c, y)| c * y).sum();
    let variance: f64 = c.iter().zip(&w).map(|(c, w)| c * w).sum();
    // Closed-form GLS: x=y-Vc(c'Vc)^-1 c'y; Cov(x)=V-Vc(c'Vc)^-1 c'V.
    for i in 0..3 {
        close(
            r.estimates_kg[i].value(),
            y[i] - w[i] * residual / variance,
            4e-14,
        );
        close(
            r.measured_adjustments_kg[i].value(),
            -w[i] * residual / variance,
            4e-14,
        );
        for j in 0..3 {
            let adjustment_variance = w[i] * w[j] / variance;
            close(
                r.posterior_covariance[i * 3 + j].value(),
                v[i * 3 + j] - adjustment_variance,
                4e-15,
            );
            close(
                r.measured_adjustment_covariance[i * 3 + j].value(),
                adjustment_variance,
                4e-15,
            );
        }
    }
    // Independently evaluated with 70-digit decimal exponentiation.
    close(r.fitted_opening_kg[0].value(), 1.1092323769993064, 4e-14);
    close(r.fitted_feed_kg[0].value(), 10.641_857_371_547_56, 4e-14);
    close(r.fitted_outlet_kg[0].value(), 4.616089132890642, 4e-14);
    close(r.predicted_outlet_kg[0].value(), 4.616089132890642, 4e-14);
    close(r.closing_inventory_kg[0].value(), 7.135000615656226, 4e-14);
    close(
        r.opening_outlet_kg[0].value(),
        (1.0 - a) * r.fitted_opening_kg[0].value(),
        4e-15,
    );
    let GrossErrorDiagnostic::Available {
        chi_square,
        degrees_of_freedom,
    } = r.gross_error
    else {
        panic!("measured feed supplies one redundant equation");
    };
    assert_eq!(degrees_of_freedom, 1);
    close(chi_square, 7.61319826872064, 6e-14);
    assert_eq!(result.opening().specified(), Some(&input.1));
    assert_eq!(result.error_model(), &input.3);
    assert_eq!(result, run(&input).unwrap());
}

#[test]
fn measured_opening_and_missing_feeds_retain_full_joint_covariance() {
    let mut input = setup(
        &[None, None],
        &[
            "4.943035529371538572764190161",
            "5.932370089492709032814663178",
        ],
    );
    let v = [0.25, 0.02, -0.03, 0.02, 0.04, 0.01, -0.03, 0.01, 0.09];
    measured_opening(&mut input, &v);
    let result = run(&input).unwrap();
    let r = &result.admitted_claim().unwrap().value;
    assert_eq!(r.opening_coordinate_indices, [0]);
    assert_eq!(r.outlet_coordinate_indices, [1, 2]);
    assert_eq!(r.feed_coordinate_indices, [3, 4]);
    assert_eq!(r.gross_error, GrossErrorDiagnostic::NoRedundancy);
    assert_eq!(
        r.measured_adjustments_kg
            .iter()
            .map(|q| q.value())
            .collect::<Vec<_>>(),
        [0.0; 3]
    );
    close(r.fitted_opening_kg[0].value(), 2.0, 2e-14);
    close(r.fitted_feed_kg[0].value(), 10.0, 2e-13);
    close(r.fitted_feed_kg[1].value(), 4.0, 2e-13);
    let a = (-1.0_f64).exp();
    let g = 1.0 - a;
    // F=H^-1(O-GS), H=[[a,0],[g²,a]], G=[g,ag]. With no redundant
    // observations, T maps [S,O0,O1] directly to [S,O0,O1,F0,F1].
    let t = [
        [1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [0.0, 0.0, 1.0],
        [-g / a, 1.0 / a, 0.0],
        [-g + g.powi(3) / a.powi(2), -g.powi(2) / a.powi(2), 1.0 / a],
    ];
    for i in 0..5 {
        for j in 0..5 {
            let mut expected = 0.0;
            for k in 0..3 {
                for l in 0..3 {
                    expected += t[i][k] * v[k * 3 + l] * t[j][l];
                }
            }
            close(r.posterior_covariance[i * 5 + j].value(), expected, 8e-14);
        }
    }
    close(
        r.posterior_covariance[3 * 5 + 3].value(),
        0.8468543836415017,
        8e-14,
    );
    close(
        r.posterior_covariance[3 * 5 + 4].value(),
        -0.4686319625863019,
        8e-14,
    );
    close(
        r.posterior_covariance[4 * 5 + 4].value(),
        0.8869525049521716,
        8e-14,
    );
    for (actual, expected) in r.opening_response_matrix.iter().zip([g, a * g]) {
        close(*actual, expected, 1e-15);
    }
    close(r.closing_inventory_kg[1].value(), 5.124594381135752, 8e-14);
    assert!(
        r.conservation_residual_kg
            .iter()
            .all(|q| q.value().abs() < 1e-13)
    );
    assert!(
        r.transport_residual_kg
            .iter()
            .all(|q| q.value().abs() < 1e-13)
    );
    let mut reversed = input.clone();
    reversed.2.reverse();
    assert_eq!(result, run(&reversed).unwrap());
}

#[test]
fn measured_two_tank_openings_retain_separate_closed_form_responses() {
    // For x=1 per interval, initial stocks advance by E=a[[1,0],[1,1]].
    // Their interval discharge rows are [1-2a,1-a] and [2a-3a²,a-a²].
    let mut input = setup(
        &[None, None],
        &[
            "2.367879441171442321595523770",
            "2.110067772330436991765617841",
        ],
    );
    let mut model = input.0.value.clone();
    model.tanks = 2;
    model.volume_m3 = CubicMetres::new(2.0);
    input.0 = model_claim(model);
    let mut opening = input.1.value.clone();
    opening.tanks = vec![masses("1"), masses("3")];
    input.1 = opening_claim(opening);
    measured_opening(
        &mut input,
        &[
            1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
        ],
    );
    let result = run(&input).unwrap();
    let r = &result.admitted_claim().unwrap().value;
    assert_eq!(r.opening_coordinate_indices, [0, 1]);
    assert_eq!(r.outlet_coordinate_indices, [2, 3]);
    assert_eq!(r.feed_coordinate_indices, [4, 5]);
    assert_eq!(r.observations[0].role, ObservationRole::OpeningTank(0));
    assert_eq!(r.observations[1].role, ObservationRole::OpeningTank(1));
    assert_eq!(r.observations[0].source, input.1.claim_id);
    assert_eq!(r.observations[1].source, input.1.claim_id);
    let a = (-1.0_f64).exp();
    for (actual, expected) in r.opening_response_matrix.iter().zip([
        1.0 - 2.0 * a,
        1.0 - a,
        2.0 * a - 3.0 * a * a,
        a - a * a,
    ]) {
        close(*actual, expected, 1e-15);
    }
    for (actual, expected) in r.response_matrix.iter().zip([
        3.0 * a - 1.0,
        0.0,
        2.0 * (1.0 - a) * (1.0 - 2.0 * a),
        3.0 * a - 1.0,
    ]) {
        close(*actual, expected, 1e-15);
    }
    close(r.fitted_opening_kg[0].value(), 1.0, 1e-14);
    close(r.fitted_opening_kg[1].value(), 3.0, 1e-14);
    close(r.fitted_feed_kg[0].value(), 2.0, 2e-13);
    close(r.fitted_feed_kg[1].value(), 4.0, 2e-13);
    assert_eq!(r.gross_error, GrossErrorDiagnostic::NoRedundancy);
}

#[test]
fn measured_opening_source_order_and_covariance_are_bound() {
    let mut input = setup(&[Some("10")], &["5"]);
    measured_opening(&mut input, &[1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0]);
    let original = run(&input).unwrap();
    original.admitted_claim().unwrap();
    let mut changed = input.clone();
    changed.1.value.tanks[0] = masses("2.1");
    assert_eq!(run(&changed).unwrap_err().code, "FMINE-REF-507");
    changed.1 = opening_claim(changed.1.value.clone());
    assert_eq!(run(&changed).unwrap_err().code, "FMINE-REF-102");
    rebind(&mut changed);
    assert_ne!(original.digest(), run(&changed).unwrap().digest());
    for mutation in 0..4 {
        let mut changed = input.clone();
        let mut error = changed.3.value.clone();
        match mutation {
            0 => error.observations[0].source = input.2[0].claim_id,
            1 => {
                error.observations.remove(0);
            }
            2 => error.observations.swap(0, 1),
            _ => error.observations[0].role = ObservationRole::OpeningTank(1),
        }
        changed.3 = error_claim(error);
        let expected = if mutation == 1 {
            "FMINE-REF-105"
        } else {
            "FMINE-REF-102"
        };
        assert_eq!(run(&changed).unwrap_err().code, expected);
    }
    let mut changed = input.clone();
    let mut error = changed.3.value.clone();
    error.covariance = vec![KilogramsSquared::new(1.0); 4];
    changed.3 = error_claim(error);
    assert!(run(&changed).is_err());
    let mut error = input.3.value.clone();
    error.covariance[1] = KilogramsSquared::new(0.1);
    changed.3 = error_claim(error);
    assert!(run(&changed).is_err());
    let mut error = input.3.value.clone();
    error.covariance[0] = KilogramsSquared::new(0.0);
    changed.3 = error_claim(error);
    assert!(run(&changed).is_err());
}

#[test]
fn negative_fitted_opening_keeps_diagnostics_without_admitted_claim() {
    let mut input = setup(&[Some("10")], &["0"]);
    let mut opening = input.1.value.clone();
    opening.tanks[0] = masses("0");
    input.1 = opening_claim(opening);
    measured_opening(&mut input, &[1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0]);
    let result = run(&input).unwrap();
    assert_eq!(result.refusal().unwrap().code, "FMINE-REF-109");
    assert!(result.admitted_claim().is_err());
    let r = result.diagnostics();
    let a = (-1.0_f64).exp();
    let g = 1.0 - a;
    close(
        r.fitted_opening_kg[0].value(),
        -10.0 * a * g / (g * g + a * a + 1.0),
        2e-14,
    );
    assert!(r.fitted_opening_kg[0].value() < 0.0);
    assert!(r.fitted_feed_kg[0].value() > 0.0);
    assert!(r.fitted_outlet_kg[0].value() > 0.0);
    assert!(r.closing_inventory_kg[0].value() > 0.0);
    assert_eq!(r.posterior_covariance.len(), 9);
    assert_eq!(result.opening().specified(), Some(&input.1));
    assert_eq!(
        result.opening().specified().unwrap().value.tanks[0].elements_kg[&element()],
        q("0")
    );
    assert_eq!(result, run(&input).unwrap());
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
    let mut jacobian = [0.0; 4 * 3];
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

fn unmeasured_opening(input: &Inputs) -> (Claim<UnmeasuredOpening>, Claim<InverseErrorModel>) {
    let value = UnmeasuredOpening {
        boundary: input.0.value.scope.boundary.clone(),
        at_ns: input.0.value.scope.interval.start_ns,
        element: element(),
        tanks: input.0.value.tanks,
        dead: input.1.value.dead.clone(),
    };
    let tag = value.canonical_tag();
    let opening = source(
        UNMEASURED_OPENING_METRIC,
        value,
        tag,
        AdditivityClass::NonAdditive,
    );
    let mut errors = input.3.value.clone();
    errors.opening_treatment = OpeningTreatment::Unmeasured;
    errors.opening_source = opening.claim_id;
    errors.model_source = input.0.claim_id;
    errors.observations = observation_order(&input.2).unwrap();
    (opening, error_claim(errors))
}

#[test]
fn unmeasured_opening_and_missing_feed_have_the_independent_joint_covariance() {
    let mut input = setup(
        &[Some("10"), None],
        &[
            "4.943035529371538572764190161",
            "5.932370089492709032814663178",
        ],
    );
    let v = [
        [0.04, 0.01, -0.005],
        [0.01, 0.09, 0.02],
        [-0.005, 0.02, 0.16],
    ];
    let mut errors = input.3.value.clone();
    errors.covariance = v.into_iter().flatten().map(KilogramsSquared::new).collect();
    input.3 = error_claim(errors);
    let (opening, errors) = unmeasured_opening(&input);
    let result =
        deconvolve_unmeasured_opening(&input.0, &opening, &input.2, &errors, || false).unwrap();
    let r = &result.admitted_claim().unwrap().value;
    let a = (-1.0_f64).exp();
    let g = 1.0 - a;
    // Analytic inverse from measured [F0,O0,O1] to [F0,O0,O1,F1,S].
    let t = [
        [1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [0.0, 0.0, 1.0],
        [2.0 - 1.0 / a, -1.0, 1.0 / a],
        [-a / g, 1.0 / g, 0.0],
    ];
    for (i, row) in t.iter().enumerate() {
        for (j, column) in t.iter().enumerate() {
            let expected: f64 = (0..3)
                .map(|k| (0..3).map(|l| row[k] * v[k][l] * column[l]).sum::<f64>())
                .sum();
            close(r.posterior_covariance[i * 5 + j].value(), expected, 1e-13);
        }
    }
    close(r.fitted_opening_kg[0].value(), 2.0, 2e-13);
    close(r.fitted_feed_kg[1].value(), 4.0, 2e-13);
    assert_eq!(r.observations.len(), 3);
    assert_eq!(r.opening_coordinate_indices, [4]);
    assert_eq!(r.feed_coordinate_indices, [0, 3]);
    assert_eq!(r.gross_error, GrossErrorDiagnostic::NoRedundancy);
    assert_eq!(result.opening().unmeasured(), Some(&opening));
    assert!(result.opening().specified().is_none());
    assert!(
        r.observations
            .iter()
            .all(|o| !matches!(o.role, ObservationRole::OpeningTank(_)))
    );
    assert!(
        result
            .admitted_claim()
            .unwrap()
            .parents
            .contains(&opening.claim_id)
    );
    close(r.whole_window_residual_kg.value(), 0.0, 2e-14);
    input.2.reverse();
    assert_eq!(
        result,
        deconvolve_unmeasured_opening(&input.0, &opening, &input.2, &errors, || false).unwrap()
    );
}

#[test]
fn redundant_observations_estimate_opening_and_adjust_flows_without_an_initial_prior() {
    let mut input = setup(
        &[Some("10"), Some("4")],
        &[
            "5.043035529371538572764190161",
            "5.732370089492709032814663178",
        ],
    );
    let variance = [0.04, 0.09, 0.16, 0.25];
    let mut errors = input.3.value.clone();
    errors.covariance = (0..16)
        .map(|i| KilogramsSquared::new(if i / 4 == i % 4 { variance[i / 4] } else { 0.0 }))
        .collect();
    input.3 = error_claim(errors);
    let (opening, errors) = unmeasured_opening(&input);
    let result =
        deconvolve_unmeasured_opening(&input.0, &opening, &input.2, &errors, || false).unwrap();
    let r = &result.admitted_claim().unwrap().value;
    let a = (-1.0_f64).exp();
    // Eliminating S leaves B*y=0 for y=[F0,O0,F1,O1]. This is a one-row GLS
    // oracle independent of the candidate's two-row measured/latent solve.
    let b = [2.0 * a - 1.0, -a, -a, 1.0];
    let residual: f64 = b
        .iter()
        .zip(&r.observed_kg)
        .map(|(&b, y)| b * y.value())
        .sum();
    let balance_variance: f64 = b.iter().zip(variance).map(|(b, v)| b * b * v).sum();
    for i in 0..4 {
        let adjustment = -variance[i] * b[i] * residual / balance_variance;
        close(r.measured_adjustments_kg[i].value(), adjustment, 2e-14);
        close(
            r.estimates_kg[i].value(),
            r.observed_kg[i].value() + adjustment,
            2e-14,
        );
    }
    let s = [-a / (1.0 - a), 1.0 / (1.0 - a), 0.0, 0.0];
    let mut expected_variance = 0.0;
    for i in 0..4 {
        for j in 0..4 {
            let posterior = if i == j { variance[i] } else { 0.0 }
                - variance[i] * b[i] * variance[j] * b[j] / balance_variance;
            close(r.posterior_covariance[i * 5 + j].value(), posterior, 2e-14);
            expected_variance += s[i] * posterior * s[j];
        }
    }
    close(r.posterior_covariance[24].value(), expected_variance, 3e-14);
    close(
        r.fitted_opening_kg[0].value(),
        s.iter()
            .zip(&r.estimates_kg)
            .map(|(&s, y)| s * y.value())
            .sum(),
        2e-14,
    );
    let GrossErrorDiagnostic::Available {
        chi_square,
        degrees_of_freedom,
    } = r.gross_error
    else {
        panic!("one redundant balance");
    };
    assert_eq!(degrees_of_freedom, 1);
    close(chi_square, residual * residual / balance_variance, 3e-14);
    let mut strict = errors.value.clone();
    strict.global_chi_square_limit = chi_square / 2.0;
    let rejected =
        deconvolve_unmeasured_opening(&input.0, &opening, &input.2, &error_claim(strict), || false)
            .unwrap();
    assert_eq!(rejected.refusal().unwrap().code, "FMINE-REF-101");
    assert!(rejected.admitted_claim().is_err());
    assert_eq!(rejected.diagnostics().estimates_kg, r.estimates_kg);
}

#[test]
fn two_tank_unmeasured_initial_stocks_are_separately_observable() {
    // Erlang survival gives G=[[1-2a,1-a],[2a-3a²,a-a²]], a=exp(-1).
    // Decimal literals are independent 60-digit evaluations for S=[2,3], F=[0,0].
    let mut input = setup(
        &[Some("0"), Some("0")],
        &[
            "2.424843911799903748831333609",
            "1.357138539070582024122670936",
        ],
    );
    let mut model = input.0.value.clone();
    model.tanks = 2;
    model.volume_m3 = CubicMetres::new(2.0);
    input.0 = model_claim(model);
    let (opening, errors) = unmeasured_opening(&input);
    let result =
        deconvolve_unmeasured_opening(&input.0, &opening, &input.2, &errors, || false).unwrap();
    let r = &result.admitted_claim().unwrap().value;
    close(r.fitted_opening_kg[0].value(), 2.0, 2e-13);
    close(r.fitted_opening_kg[1].value(), 3.0, 2e-13);
    let a = (-1.0_f64).exp();
    for (actual, expected) in r.opening_response_matrix.iter().zip([
        1.0 - 2.0 * a,
        1.0 - a,
        2.0 * a - 3.0 * a * a,
        a - a * a,
    ]) {
        close(*actual, expected, 2e-15);
    }
    assert_eq!(r.opening_coordinate_indices, [4, 5]);
    assert_eq!(r.gross_error, GrossErrorDiagnostic::NoRedundancy);
    close(r.whole_window_residual_kg.value(), 0.0, 2e-14);
}

#[test]
fn unmeasured_opening_refuses_unobservable_or_negative_initial_inventory() {
    for bypass in [false, true] {
        let mut input = if bypass {
            setup(&[Some("10")], &["10"])
        } else {
            setup(&[None], &["5"])
        };
        if bypass {
            let mut model = input.0.value.clone();
            model.bypass_fraction = 1.0;
            input.0 = model_claim(model);
        }
        let (opening, errors) = unmeasured_opening(&input);
        assert_eq!(
            deconvolve_unmeasured_opening(&input.0, &opening, &input.2, &errors, || false)
                .unwrap_err()
                .code,
            "FMINE-REF-104"
        );
    }
    let input = setup(&[Some("10")], &["0"]);
    let (opening, errors) = unmeasured_opening(&input);
    let result =
        deconvolve_unmeasured_opening(&input.0, &opening, &input.2, &errors, || false).unwrap();
    assert_eq!(result.refusal().unwrap().code, "FMINE-REF-109");
    assert!(result.admitted_claim().is_err());
    let a = (-1.0_f64).exp();
    close(
        result.diagnostics().fitted_opening_kg[0].value(),
        -10.0 * a / (1.0 - a),
        2e-14,
    );
    assert_eq!(result.opening().unmeasured(), Some(&opening));
}

#[test]
fn unmeasured_opening_source_policy_taint_and_cancellation_are_binding() {
    let input = setup(&[Some("10")], &["5"]);
    let (mut opening, errors) = unmeasured_opening(&input);
    opening.value.tanks += 1;
    assert_eq!(
        deconvolve_unmeasured_opening(&input.0, &opening, &input.2, &errors, || false)
            .unwrap_err()
            .code,
        "FMINE-REF-507"
    );
    let (opening, errors) = unmeasured_opening(&input);
    let opening = Claim::new(
        &opening.metric_key,
        opening.value.clone(),
        opening.value_tag(),
        opening.representation.clone(),
        opening.support,
        opening.additivity,
        opening.evidence,
        opening.epistemic,
        opening.state,
        Confidentiality::internal(),
        opening.uu_terms.clone(),
        opening.parents.clone(),
        opening.anchors.clone(),
    );
    let mut bound = errors.value.clone();
    bound.opening_source = opening.claim_id;
    let errors = error_claim(bound);
    let result =
        deconvolve_unmeasured_opening(&input.0, &opening, &input.2, &errors, || false).unwrap();
    assert_eq!(
        *result.admitted_claim().unwrap().confidentiality(),
        Confidentiality::internal()
    );
    let mut wrong = errors.value.clone();
    wrong.opening_treatment = OpeningTreatment::Measured;
    assert_eq!(
        deconvolve_unmeasured_opening(&input.0, &opening, &input.2, &error_claim(wrong), || false)
            .unwrap_err()
            .code,
        "FMINE-REF-105"
    );
    assert!(
        deconvolve_unmeasured_opening(&input.0, &opening, &input.2, &errors, || true)
            .unwrap_err()
            .what
            .contains("cancelled")
    );
}

fn mixed_opening_claim(value: MixedOpening) -> Claim<MixedOpening> {
    let tag = value.canonical_tag();
    source(
        MIXED_OPENING_METRIC,
        value,
        tag,
        AdditivityClass::NonAdditive,
    )
}

fn mixed_input(
    feeds: &[Option<&str>],
    outlets: &[&str],
    covariance: &[f64],
) -> (Inputs, Claim<MixedOpening>, Claim<InverseErrorModel>) {
    let mut input = setup(feeds, outlets);
    let mut model = input.0.value.clone();
    model.tanks = 2;
    model.volume_m3 = CubicMetres::new(2.0);
    input.0 = model_claim(model);
    let opening = mixed_opening_claim(MixedOpening {
        boundary: input.0.value.scope.boundary.clone(),
        at_ns: 0,
        element: element(),
        tanks: vec![None, Some(masses("3"))],
        dead: input.1.value.dead.clone(),
    });
    let mut errors = input.3.value.clone();
    errors.opening_treatment = OpeningTreatment::Mixed;
    errors.opening_source = opening.claim_id;
    errors.model_source = input.0.claim_id;
    errors.observations = observation_order_with_mixed_opening(&opening, &input.2).unwrap();
    errors.covariance = covariance
        .iter()
        .copied()
        .map(KilogramsSquared::new)
        .collect();
    (input, opening, error_claim(errors))
}

#[test]
fn mixed_opening_and_missing_feed_preserve_the_full_analytic_joint_covariance() {
    // Independent two-tank closed forms, a=exp(-1):
    // O0=(1-2a)S0+(1-a)S1+(3a-1)F0;
    // O1=(2a-3a²)S0+(a-a²)S1+(2-6a+4a²)F0+(3a-1)F1.
    // Only S1 is measured. Exact outlet decimals are evaluated from S=[2,3], F=[10,4].
    let v = [
        [0.25, 0.01, -0.02, 0.005],
        [0.01, 0.04, 0.006, -0.003],
        [-0.02, 0.006, 0.09, 0.012],
        [0.005, -0.003, 0.012, 0.16],
    ];
    let (mut input, opening, errors) = mixed_input(
        &[Some("10"), None],
        &[
            "3.461227146943173396697046714",
            "5.112336692305858263297509768",
        ],
        &v.into_iter().flatten().collect::<Vec<_>>(),
    );
    let result = deconvolve_mixed_opening(&input.0, &opening, &input.2, &errors, || false).unwrap();
    let r = &result.admitted_claim().unwrap().value;
    assert_eq!(r.observations.len(), 4);
    assert_eq!(r.opening_coordinate_indices, [5, 0]);
    assert_eq!(r.feed_coordinate_indices, [1, 4]);
    assert_eq!(r.outlet_coordinate_indices, [2, 3]);
    assert_eq!(r.observations[0].role, ObservationRole::OpeningTank(1));
    assert!(
        !r.observations
            .iter()
            .any(|o| o.role == ObservationRole::OpeningTank(0))
    );
    assert_eq!(r.observations[0].source, opening.claim_id);
    close(r.fitted_opening_kg[0].value(), 2.0, 3e-13);
    close(r.fitted_opening_kg[1].value(), 3.0, 3e-13);
    close(r.fitted_feed_kg[1].value(), 4.0, 3e-13);
    assert_eq!(r.gross_error, GrossErrorDiagnostic::NoRedundancy);
    let a = (-1.0_f64).exp();
    let g0 = 1.0 - 2.0 * a;
    let g1 = 1.0 - a;
    let h = 3.0 * a - 1.0;
    let next0 = 2.0 * a - 3.0 * a * a;
    let next1 = a - a * a;
    let lag = 2.0 - 6.0 * a + 4.0 * a * a;
    let s = [-g1 / g0, -h / g0, 1.0 / g0, 0.0];
    let f = [
        (-next0 * s[0] - next1) / h,
        (-next0 * s[1] - lag) / h,
        -next0 * s[2] / h,
        1.0 / h,
    ];
    let t = [
        [1.0, 0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [0.0, 0.0, 0.0, 1.0],
        f,
        s,
    ];
    for (i, row) in t.iter().enumerate() {
        for (j, column) in t.iter().enumerate() {
            let expected: f64 = (0..4)
                .map(|k| (0..4).map(|l| row[k] * v[k][l] * column[l]).sum::<f64>())
                .sum();
            close(r.posterior_covariance[i * 6 + j].value(), expected, 2e-12);
        }
    }
    close(r.whole_window_residual_kg.value(), 0.0, 3e-14);
    assert_eq!(result.opening().mixed(), Some(&opening));
    assert!(
        result
            .admitted_claim()
            .unwrap()
            .parents
            .contains(&opening.claim_id)
    );
    input.2.reverse();
    assert_eq!(
        result,
        deconvolve_mixed_opening(&input.0, &opening, &input.2, &errors, || false).unwrap()
    );
}

#[test]
fn mixed_opening_redundant_fit_matches_an_independently_eliminated_constraint() {
    let diagonal = [0.25, 0.04, 0.09, 0.16, 0.25];
    let u = [0.1, 0.05, -0.08, 0.03, 0.09];
    let v: Vec<_> = (0..25)
        .map(|i| u[i / 5] * u[i % 5] + if i / 5 == i % 5 { diagonal[i / 5] } else { 0.0 })
        .collect();
    let (input, opening, errors) = mixed_input(
        &[Some("10"), Some("4")],
        &[
            "3.561227146943173396697046714",
            "5.042336692305858263297509768",
        ],
        &v,
    );
    let result = deconvolve_mixed_opening(&input.0, &opening, &input.2, &errors, || false).unwrap();
    let r = &result.admitted_claim().unwrap().value;
    let a = (-1.0_f64).exp();
    let g0 = 1.0 - 2.0 * a;
    let g1 = 1.0 - a;
    let h = 3.0 * a - 1.0;
    let ratio = (2.0 * a - 3.0 * a * a) / g0;
    // Eliminate the unknown first tank from the two balance equations. One
    // measured-only GLS constraint remains on [S1,F0,O0,F1,O1].
    let b = [
        ratio * g1 - (a - a * a),
        ratio * h - (2.0 - 6.0 * a + 4.0 * a * a),
        -ratio,
        -h,
        1.0,
    ];
    let w: Vec<_> = (0..5)
        .map(|i| (0..5).map(|j| v[i * 5 + j] * b[j]).sum::<f64>())
        .collect();
    let variance: f64 = b.iter().zip(&w).map(|(b, w)| b * w).sum();
    let residual: f64 = b
        .iter()
        .zip(&r.observed_kg)
        .map(|(b, y)| b * y.value())
        .sum();
    let mut posterior = [0.0; 25];
    for i in 0..5 {
        close(
            r.measured_adjustments_kg[i].value(),
            -w[i] * residual / variance,
            3e-14,
        );
        close(
            r.estimates_kg[i].value(),
            r.observed_kg[i].value() - w[i] * residual / variance,
            3e-14,
        );
        for j in 0..5 {
            posterior[i * 5 + j] = v[i * 5 + j] - w[i] * w[j] / variance;
            close(
                r.posterior_covariance[i * 6 + j].value(),
                posterior[i * 5 + j],
                3e-14,
            );
            close(
                r.measured_adjustment_covariance[i * 5 + j].value(),
                w[i] * w[j] / variance,
                3e-14,
            );
        }
    }
    let s = [-g1 / g0, -h / g0, 1.0 / g0, 0.0, 0.0];
    let stock_variance: f64 = (0..5)
        .map(|i| {
            (0..5)
                .map(|j| s[i] * posterior[i * 5 + j] * s[j])
                .sum::<f64>()
        })
        .sum();
    close(r.posterior_covariance[35].value(), stock_variance, 1e-13);
    for j in 0..5 {
        let cross: f64 = (0..5).map(|i| s[i] * posterior[i * 5 + j]).sum();
        close(r.posterior_covariance[30 + j].value(), cross, 5e-14);
        close(r.posterior_covariance[j * 6 + 5].value(), cross, 5e-14);
    }
    let GrossErrorDiagnostic::Available {
        chi_square,
        degrees_of_freedom,
    } = r.gross_error
    else {
        panic!("one redundant constraint");
    };
    assert_eq!(degrees_of_freedom, 1);
    close(chi_square, residual * residual / variance, 3e-14);
    let mut strict = errors.value.clone();
    strict.global_chi_square_limit = chi_square / 2.0;
    let rejected =
        deconvolve_mixed_opening(&input.0, &opening, &input.2, &error_claim(strict), || false)
            .unwrap();
    assert_eq!(rejected.refusal().unwrap().code, "FMINE-REF-101");
    assert!(rejected.admitted_claim().is_err());
    assert_eq!(rejected.diagnostics().estimates_kg, r.estimates_kg);
}

#[test]
fn mixed_opening_refuses_unobservable_stocks_and_retains_negative_fit_diagnostics() {
    let covariance = [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0];
    let (mut input, opening, errors) = mixed_input(&[Some("10")], &["0"], &covariance);
    let negative =
        deconvolve_mixed_opening(&input.0, &opening, &input.2, &errors, || false).unwrap();
    assert_eq!(negative.refusal().unwrap().code, "FMINE-REF-109");
    assert!(negative.diagnostics().fitted_opening_kg[0].value() < 0.0);
    assert!(negative.admitted_claim().is_err());
    let mut model = input.0.value.clone();
    model.bypass_fraction = 1.0;
    input.0 = model_claim(model);
    let mut bypass_errors = errors.value.clone();
    bypass_errors.model_source = input.0.claim_id;
    assert_eq!(
        deconvolve_mixed_opening(
            &input.0,
            &opening,
            &input.2,
            &error_claim(bypass_errors),
            || false
        )
        .unwrap_err()
        .code,
        "FMINE-REF-104"
    );
    let (input, opening, errors) = mixed_input(&[None], &["5"], &[1.0, 0.0, 0.0, 1.0]);
    assert_eq!(
        deconvolve_mixed_opening(&input.0, &opening, &input.2, &errors, || false)
            .unwrap_err()
            .code,
        "FMINE-REF-104"
    );
}

#[test]
fn mixed_opening_population_source_and_treatment_are_bound_without_filling_missing_stocks() {
    let (input, opening, errors) = mixed_input(
        &[Some("10")],
        &["4"],
        &[1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0],
    );
    let mut changed = opening.clone();
    changed.value.tanks[0] = Some(masses("0"));
    assert_eq!(
        deconvolve_mixed_opening(&input.0, &changed, &input.2, &errors, || false)
            .unwrap_err()
            .code,
        "FMINE-REF-507"
    );
    for tanks in [vec![None, None], vec![Some(masses("0")), Some(masses("3"))]] {
        let mut value = opening.value.clone();
        value.tanks = tanks;
        assert_eq!(
            observation_order_with_mixed_opening(&mixed_opening_claim(value), &input.2)
                .unwrap_err()
                .code,
            "FMINE-REF-105"
        );
    }
    let mut wrong = errors.value.clone();
    wrong.opening_treatment = OpeningTreatment::Measured;
    assert_eq!(
        deconvolve_mixed_opening(&input.0, &opening, &input.2, &error_claim(wrong), || false)
            .unwrap_err()
            .code,
        "FMINE-REF-105"
    );
    assert!(
        deconvolve_mixed_opening(&input.0, &opening, &input.2, &errors, || true)
            .unwrap_err()
            .what
            .contains("cancelled")
    );
}
