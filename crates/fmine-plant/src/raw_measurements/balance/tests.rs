use super::*;

const FIXTURE: &str = include_str!("../../../../../tests/plant/raw_balance.toml");

fn source(text: &str) -> SourceAnchor {
    SourceAnchor::new("synthetic/raw_balance.toml", sha256(text.as_bytes()).0, "complete worksheet")
}

fn run(text: &str) -> WorksheetResult {
    evaluate(text, &source(text)).unwrap()
}

fn admitted(result: &WorksheetResult) -> &RawBalanceSummary {
    assert!(result.refusal().is_none(), "{:?}", result.refusal());
    &result.admitted_claim().unwrap().value
}

fn near(actual: f64, expected: f64, tolerance: f64) {
    assert!(actual.is_finite() && (actual - expected).abs() <= tolerance,
        "{actual} != {expected}, tolerance {tolerance}");
}

#[test]
fn nonlinear_raw_likelihood_recovers_independent_extensive_known_answer() {
    // Constructed independently from physical KKT equations at D/W/E =
    // (80,20,8), (40,10,2), with inventory growth (40,10,6).
    // For half the squared objective, g=(0,0,-1,0,0,1) and lambda=(0,0,1).
    let result = run(FIXTURE);
    let summary = admitted(&result);
    for (actual, expected) in summary.streams.iter().zip([[80.0, 20.0, 8.0], [40.0, 10.0, 2.0]]) {
        near(actual.dry_kg.value(), expected[0], 1e-7);
        near(actual.water_kg.value(), expected[1], 1e-7);
        near(actual.elemental_kg.value(), expected[2], 1e-7);
        near(actual.derived_dry_element_fraction,
            actual.elemental_kg.value() / actual.dry_kg.value(), 1e-15);
    }
    near(summary.standardized_misfit, 0.818625, 1e-9);
    for (actual, expected) in summary.standardized_measurement_residuals.iter()
        .zip([-0.08, 0.1, -0.8, 0.04, -0.025, 0.4])
    {
        near(*actual, expected, 1e-7);
    }
    assert!(summary.residuals_kg.iter().all(|v| v.value().abs() <= 1e-8));
    assert!(summary.raw_residuals_exact_kg.iter().any(|v| *v != ZERO));
    assert!(summary.iterations > 1);
    assert!(summary.relative_stationarity <= 1e-9 && summary.standardized_step <= 1e-9);
    // Along conservation, both streams have the same D/W/E perturbation.
    // The independent information matrix is
    // [357/160, 6/5, -15/32; 6/5, 26/5, 0; -15/32, 0, 125/16].
    // Inverting that rational matrix gives the common 3x3 block below.
    let block = [
        [4160.0 / 8013.0, -960.0 / 8013.0, 416.0 / 13355.0],
        [-960.0 / 8013.0, 15275.0 / 69446.0, -96.0 / 13355.0],
        [416.0 / 13355.0, -96.0 / 13355.0, 8672.0 / 66775.0],
    ];
    assert_eq!(summary.local_extensive_covariance.len(), 36);
    for i in 0..6 {
        for j in 0..6 {
            near(summary.local_extensive_covariance[6 * i + j].value(), block[i % 3][j % 3], 1e-8);
        }
    }
    assert_eq!(result.source_claim().value, FIXTURE);
    assert_eq!(result.admitted_claim().unwrap().parents, vec![result.source_claim().claim_id]);
    assert!(result.to_json().contains("\"state\":\"Estimated\""));
    assert!(result.to_markdown().contains("Derived dry fraction"));
}

#[test]
fn known_solution_has_positive_curvature_along_independent_feasible_perturbations() {
    // This raw likelihood is evaluated directly in physical kg, independently
    // of the adapter's coordinates, Jacobian and optimization implementation.
    let objective = |x: &[f64; 6]| {
        let observed = [[100.08, 0.199, 0.108], [49.96, 0.20025, 0.046]];
        (0..2).map(|i| {
            let at = 3 * i;
            let total = x[at] + x[at + 1];
            let wet = total - observed[i][0];
            let water = (x[at + 1] / total - observed[i][1]) / 0.01;
            let assay = (x[at + 2] / x[at] - observed[i][2]) / 0.01;
            wet * wet + water * water + assay * assay
        }).sum::<f64>()
    };
    let exact = [80.0, 20.0, 8.0, 40.0, 10.0, 2.0];
    near(objective(&exact), 0.818625, 1e-12);
    for axis in 0..3 {
        for delta in [-0.001, 0.001] {
            let mut perturbed = exact;
            perturbed[axis] += delta;
            perturbed[axis + 3] += delta;
            assert!(objective(&perturbed) > objective(&exact));
        }
    }
}

#[test]
fn trace_element_rescaling_does_not_hide_unresolved_metal_steps() {
    let text = FIXTURE
        .replace("dry_element_fraction = \"0.108\"", "dry_element_fraction = \"0.000000108\"")
        .replace("dry_element_fraction = \"0.046\"", "dry_element_fraction = \"0.000000046\"")
        .replace("sigma_dry_element_fraction = \"0.01\"", "sigma_dry_element_fraction = \"0.00000001\"")
        .replace("closing_element_kg = \"6\"", "closing_element_kg = \"0.000006\"");
    let result = run(&text);
    let summary = admitted(&result);
    near(summary.streams[0].dry_kg.value(), 80.0, 1e-7);
    near(summary.streams[0].elemental_kg.value(), 8e-6, 1e-13);
    near(summary.streams[1].elemental_kg.value(), 2e-6, 1e-13);
    near(summary.standardized_misfit, 0.818625, 1e-9);
}

#[test]
fn full_raw_correlation_changes_fit_and_is_never_replaced_by_independence() {
    let text = FIXTURE.replace(
        "left = \"feed/wet_mass\"\nright = \"product/wet_mass\"\nvalue = \"0\"",
        "left = \"feed/wet_mass\"\nright = \"product/wet_mass\"\nvalue = \"0.5\"",
    );
    let result = run(&text);
    let summary = admitted(&result);
    assert!((summary.standardized_misfit - 0.818625).abs() > 1e-5);
    assert!(summary.residuals_kg.iter().all(|v| v.value().abs() <= 1e-8));
    near(result.parsed.correlation[3], 0.5, 0.0);
}

#[test]
fn rejection_retains_originals_and_diagnostics_without_an_admitted_claim() {
    let text = FIXTURE.replace("max_standardized_misfit = \"10\"", "max_standardized_misfit = \"0.5\"");
    let result = run(&text);
    assert_eq!(result.refusal().unwrap().code, "FMINE-REF-101");
    near(result.diagnostics().unwrap().standardized_misfit, 0.818625, 1e-9);
    assert!(result.admitted_claim().is_none());
    assert_eq!(result.source_claim().value, text);

    let text = FIXTURE.replace("max_iterations = \"128\"", "max_iterations = \"1\"");
    let result = run(&text);
    assert!(result.refusal().is_some());
    assert!(result.admitted_claim().is_none());
    assert_eq!(result.source_claim().value, text);
}

#[test]
fn unsupported_bases_incomplete_covariance_and_source_tampering_refuse() {
    for text in [
        FIXTURE.replace("mass_unit = \"kg\"", "mass_unit = \"tonnes\""),
        FIXTURE.replace("moisture_is_h2o = \"true\"", "moisture_is_h2o = \"false\""),
        FIXTURE.replace("element = \"Cu\"", "element = \"H\""),
        FIXTURE.replace("water_fraction = \"0.199\"", "water_fraction = \"1\""),
        FIXTURE.replace(
            "[[correlation]]\nleft = \"product/assay\"\nright = \"product/assay\"\nvalue = \"1\"\n",
            "",
        ),
    ] {
        assert!(evaluate(&text, &source(&text)).is_err());
    }
    let altered = FIXTURE.replace("100.08", "100.09");
    assert!(evaluate(&altered, &source(FIXTURE)).is_err());

    let changed_bytes = format!("# distinct source bytes\n{FIXTURE}");
    let original = run(FIXTURE);
    let changed = run(&changed_bytes);
    assert_eq!(admitted(&original), admitted(&changed));
    assert_ne!(original.digest(), changed.digest());
}
