//! Compositing inputs from a hole's records (`BD-173`): `sample_inputs` turns logged intervals, samples and their
//! results into the inputs `composite` takes. Known answers use binary fractions, so they compare exactly; each
//! refusal is checked for its kind and the record it names.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use fmine_claim::{AdditivityClass, SpatialSupport};
use fmine_drill::composite::{
    Bounds, Domain, Input, Residual, Rule, Weighting, composite, sample_inputs,
};
use fmine_drill::join::Span;
use fmine_drill::records::{self, AssayResult, AssayValue, DrillError, Interval, Sample};

#[allow(dead_code)]
#[path = "common/fixtures.rs"]
mod fixtures;

const GRADE: f64 = 1.0 / 524_288.0; // 2^-19
const LIMIT: f64 = 1.0 / 134_217_728.0; // 2^-27

fn span(from_mm: u64, to_mm: u64) -> Span {
    Span { from_mm, to_mm }
}

fn intervals() -> Vec<Interval> {
    vec![
        fixtures::interval("I1", "H1", 0, 1_000),
        fixtures::interval("I2", "H1", 1_000, 2_500),
        fixtures::interval("I3", "H1", 2_500, 4_000),
        fixtures::interval("I4", "H1", 4_000, 5_000),
        fixtures::interval("I5", "H2", 0, 1_000),
    ]
}

fn sample(id: &str, interval_ids: &[&str], mass_g: f64) -> Sample {
    Sample {
        mass_g: Some(mass_g),
        ..fixtures::sample(id, interval_ids)
    }
}

fn result(id: &str, sample_id: &str, analyte: &str, value: AssayValue) -> AssayResult {
    AssayResult {
        analyte: analyte.to_string(),
        value,
        ..fixtures::result(id, sample_id, "D1", "LAB-A", Some("C1"))
    }
}

fn measured(mass_fraction: f64) -> AssayValue {
    AssayValue::Measured { mass_fraction }
}

/// P2 spans two intervals, P3 has no gold result, Q1 is QC material and R3 is copper: two inputs, in depth order.
fn hole() -> (Vec<Sample>, Vec<AssayResult>) {
    let samples = vec![
        sample("P3", &["I4"], 3_250.0),
        sample("P2", &["I2", "I3"], 3_000.0),
        fixtures::qc_material("Q1"),
        sample("P1", &["I1"], 2_000.0),
    ];
    let results = vec![
        result("R1", "P1", "Au", measured(GRADE)),
        result(
            "R2",
            "P2",
            "Au",
            AssayValue::BelowDetection { limit: LIMIT },
        ),
        result("R3", "P1", "Cu", measured(0.25)),
        result("RQ", "Q1", "Au", measured(GRADE)),
    ];
    (samples, results)
}

#[test]
fn test_a_holes_samples_become_inputs_in_depth_order() {
    let (samples, results) = hole();
    let inputs = sample_inputs("H1", &intervals(), &samples, &results, "Au").unwrap();
    assert_eq!(
        inputs,
        vec![
            Input {
                span: span(0, 1_000),
                value: Bounds {
                    lo: GRADE,
                    hi: GRADE
                },
                mass_kg: Some(2.0),
            },
            Input {
                span: span(1_000, 4_000),
                value: Bounds { lo: 0.0, hi: LIMIT },
                mass_kg: Some(3.0),
            },
        ]
    );
    // Copper has one input: P1's.
    let copper = sample_inputs("H1", &intervals(), &samples, &results, "Cu").unwrap();
    assert_eq!(copper.len(), 1);
    assert_eq!(copper[0].value, Bounds { lo: 0.25, hi: 0.25 });
}

/// The inputs composite to known values: 2 m composites over one 5 m domain, length weighted. The first takes P1's
/// metre and P2's first metre, the second the rest of P2, and the last metre has no input, so no value.
#[test]
fn test_the_inputs_composite_to_known_values() {
    let (samples, results) = hole();
    let inputs = sample_inputs("H1", &intervals(), &samples, &results, "Au").unwrap();
    let set = composite(
        &[Domain {
            code: "SCHIST".to_string(),
            span: span(0, 5_000),
        }],
        &inputs,
        &Rule {
            length_mm: 2_000,
            weighting: Weighting::Length,
            residual: Residual::Keep,
            additivity: AdditivityClass::IntensiveWeighted,
            input_support: SpatialSupport::PointCore,
        },
    )
    .unwrap();
    let got: Vec<(Span, u64, Option<Bounds>)> = set
        .composites
        .iter()
        .map(|c| (c.span, c.sampled_mm, c.value))
        .collect();
    assert_eq!(
        got,
        vec![
            (
                span(0, 2_000),
                2_000,
                Some(Bounds {
                    lo: GRADE / 2.0,
                    hi: (GRADE + LIMIT) / 2.0
                })
            ),
            (
                span(2_000, 4_000),
                2_000,
                Some(Bounds { lo: 0.0, hi: LIMIT })
            ),
            (span(4_000, 5_000), 0, None),
        ]
    );
    assert!(set.conservation.relative_error <= fmine_drill::composite::CONSERVATION_TOLERANCE);
}

#[test]
fn test_inputs_that_would_misstate_the_core_are_refused() {
    let (samples, results) = hole();
    let with = |extra_samples: Vec<Sample>, extra_results: Vec<AssayResult>| {
        let mut s = samples.clone();
        s.extend(extra_samples);
        let mut r = results.clone();
        r.extend(extra_results);
        (s, r)
    };
    let cases = [
        (
            "a gap",
            with(vec![sample("P9", &["I1", "I3"], 1.0)], vec![]),
            "leaves 1000 to 2500 mm",
        ),
        (
            "another hole's interval",
            with(vec![sample("P9", &["I5"], 1.0)], vec![]),
            "on hole 'H2'",
        ),
        (
            "an interval not logged",
            with(vec![sample("P9", &["I9"], 1.0)], vec![]),
            "interval 'I9'",
        ),
        (
            "a result for no sample of the hole",
            with(vec![], vec![result("R9", "P9", "Au", measured(GRADE))]),
            "sample 'P9'",
        ),
        (
            "two results for one sample",
            with(vec![], vec![result("R9", "P1", "Au", measured(GRADE))]),
            "two Au results",
        ),
        (
            "two samples of one core",
            with(
                vec![sample("P1b", &["I1"], 1.0)],
                vec![result("R9", "P1b", "Au", measured(GRADE))],
            ),
            "samples 'P1' and 'P1b' both cover 0 to 1000 mm",
        ),
    ];
    for (what, (s, r), names) in cases {
        let e = sample_inputs("H1", &intervals(), &s, &r, "Au").unwrap_err();
        assert!(matches!(e, DrillError::Invalid { .. }), "{what}: {e:?}");
        assert!(e.to_string().contains(names), "{what}: {e}");
    }
}
