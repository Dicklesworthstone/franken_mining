//! DESIGN: runnable nominal recovery cards with four retained source/version layers.
//! FACT: corpus C2 §3.4 (E171) transcribes the three report formulas below and
//! their distinct numeric units/output scales. This example has not re-qualified
//! the original reports or calibrated any plant. All validity boxes, processing
//! intervals, inputs, operational factors, correction, cap and routes are authored.
//! DESIGN: the Cortez demonstration explicitly selects the no-arsenic-impact
//! regime; it neither evaluates arsenic nor establishes that this regime applies.
//! Determinism: DeterministicPerIsa

use fmine_claim::{AdditivityClass, Claim, Confidentiality, Representation, SpatialSupport};
use fmine_core::{anchor::SourceAnchor, hash::sha256};
use fmine_evidence::{EpistemicStatus, EvidenceClass, KnowledgeState};
use fmine_plant::cards::{
    CORRECTION_METRIC, CardVersion, INPUT_METRIC, InputRequirement, InputUnit, InputValue,
    OPERATION_METRIC, OperationalFactor, OutputScale, ROUTE_METRIC, RecoveryCorrection,
    RecoveryInputs, RecoveryModel, RecoveryScope, RouteAssumption, TEST_WORK_METRIC, TestWorkCard,
};
use fmine_time::ValidTime;
use fmine_units::ExactQty;
use std::{collections::BTreeMap, error::Error};

type Result<T> = std::result::Result<T, Box<dyn Error>>;

struct Illustration {
    id: &'static str,
    circuit: &'static str,
    domain: &'static str,
    formula: &'static str,
    unit: InputUnit,
    unit_label: &'static str,
    scale: OutputScale,
    grades: [&'static str; 3],
    source_page: &'static str,
    route: &'static str,
}

fn q(value: &str) -> Result<ExactQty> {
    Ok(ExactQty::from_decimal_str(value)?)
}
fn version(id: &str, layer: &str) -> CardVersion {
    CardVersion {
        id: format!("{id}/{layer}"),
        revision: "synthetic-example-v1".into(),
    }
}
fn source<T>(metric: &str, value: T, tag: &str, formula_page: Option<&str>) -> Claim<T> {
    let mut anchors = vec![SourceAnchor::new(
        "crates/fmine-plant/examples/process_recovery_cards.rs",
        sha256(include_bytes!("process_recovery_cards.rs")).0,
        "authored validity boxes, intervals, inputs, operational factors, correction, cap and route",
    )];
    if let Some(page) = formula_page {
        anchors.push(SourceAnchor::new(
            "planning/corpus/C2_nevada_technical_reports.md",
            sha256(include_bytes!(
                "../../../planning/corpus/C2_nevada_technical_reports.md"
            ))
            .0,
            page,
        ));
    }
    Claim::new(
        metric,
        value,
        tag,
        Representation::Deterministic,
        SpatialSupport::PeriodAggregate,
        AdditivityClass::NonAdditive,
        EvidenceClass::Assumed,
        EpistemicStatus::KnownKnown,
        KnowledgeState::Known,
        Confidentiality::public(),
        vec![],
        vec![],
        anchors,
    )
}

fn model(example: &Illustration) -> Result<RecoveryModel> {
    let card = TestWorkCard {
        version: version(example.id, "test-work-formula"),
        scope: RecoveryScope {
            circuit: example.circuit.into(),
            material_domain: example.domain.into(),
            dry_basis: "synthetic-matched-total-dry-matter/v1".into(),
            element: "Au".into(),
        },
        valid_time: ValidTime::new(0, 100)?,
        input_units: BTreeMap::from([(
            "Au".into(),
            InputRequirement {
                measurand: "total-Au/dry".into(),
                unit: example.unit,
                lower: q(example.grades[0])?,
                upper: q(example.grades[2])?,
                lower_inclusive: true,
                upper_inclusive: true,
            },
        )]),
        formula: example.formula.into(),
        output_scale: example.scale,
    };
    let tag = card.canonical_tag();
    let card = source(TEST_WORK_METRIC, card, &tag, Some(example.source_page));
    let factor = OperationalFactor {
        version: version(example.id, "authored-operational-factor"),
        test_work: card.claim_id,
        multiplier: q("0.97")?,
    };
    let tag = factor.canonical_tag();
    let factor = source(OPERATION_METRIC, factor, &tag, None);
    let correction = RecoveryCorrection {
        version: version(example.id, "authored-correction-and-cap"),
        test_work: card.claim_id,
        additive_fraction: q("0.02")?,
        upper_cap_fraction: Some(q("0.90")?),
    };
    let tag = correction.canonical_tag();
    let correction = source(CORRECTION_METRIC, correction, &tag, None);
    let route = RouteAssumption {
        version: version(example.id, "authored-route"),
        test_work: card.claim_id,
        route: example.route.into(),
    };
    let tag = route.canonical_tag();
    let route = source(ROUTE_METRIC, route, &tag, None);
    Ok(RecoveryModel::compile(
        &card,
        &[factor],
        &correction,
        &route,
    )?)
}

fn run() -> Result<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let wrong_unit = match args.as_slice() {
        [] => false,
        [arg] if arg == "--wrong-unit" => true,
        _ => return Err("usage: process_recovery_cards [--wrong-unit]".into()),
    };
    let cases = [
        Illustration {
            id: "carlin",
            circuit: "Goldstrike Roaster / Carlin formula illustration",
            domain: "synthetic-box / reported All Others and West Leeville formula",
            formula: "92.03 - 37.36 * exp(-12.94 * (Au * 0.029167))",
            unit: InputUnit::GramsPerDryMetricTonne,
            unit_label: "g/t dry metric",
            scale: OutputScale::Percent,
            grades: ["1", "5", "10"],
            source_page: "C2 §3.4: Carlin 2025 report PDF p.144; printed percent formula and internal 0.029167 conversion",
            route: "synthetic roast-and-CIL route assumption",
        },
        Illustration {
            id: "cortez",
            circuit: "Cortez ore at Goldstrike Roaster / formula illustration",
            domain: "synthetic-box / explicitly selected no-arsenic-impact regime",
            formula: "(92.027536 - 37.35906 * exp(-12.94386 * Au)) / 100.0",
            unit: InputUnit::TroyOuncesPerDryShortTon,
            unit_label: "troy oz/dry short ton",
            scale: OutputScale::Fraction,
            grades: ["0.1", "0.2", "0.3"],
            source_page: "C2 §3.4: Cortez 2022 report PDF p.173; oz/st input and fractional output; arsenic term explicitly excluded by the example regime",
            route: "synthetic Goldstrike Roaster route / no-arsenic-impact regime selected",
        },
        Illustration {
            id: "juniper-mega",
            circuit: "Juniper oxide mill / Mega formula illustration",
            domain: "synthetic-positive-grade-box / Mega formula",
            formula: "0.0754 * ln(Au) + 0.72708",
            unit: InputUnit::GramsPerDryMetricTonne,
            unit_label: "g/t dry metric",
            scale: OutputScale::Fraction,
            grades: ["1", "5", "10"],
            source_page: "C2 §3.4: Turquoise Ridge 2024 report PDF p.145; g/t input and fractional formula despite the source percent heading",
            route: "synthetic Juniper oxide-mill route assumption",
        },
    ];
    // DESIGN: compile each card once and evaluate all nine inputs before stdout.
    let mut completed = Vec::new();
    for case in cases {
        let model = model(&case)?;
        let mut evaluations = Vec::new();
        for grade in case.grades {
            let inputs = RecoveryInputs {
                scope: model.test_work().value.scope.clone(),
                interval: ValidTime::new(10, 20)?,
                values: BTreeMap::from([(
                    "Au".into(),
                    InputValue {
                        measurand: "total-Au/dry".into(),
                        unit: if wrong_unit && case.id == "cortez" {
                            InputUnit::GramsPerDryMetricTonne
                        } else {
                            case.unit
                        },
                        value: q(grade)?,
                    },
                )]),
            };
            let tag = inputs.canonical_tag();
            let receipt = model.evaluate(&source(INPUT_METRIC, inputs, &tag, None))?;
            evaluations.push((grade, receipt));
        }
        completed.push((case, model, evaluations));
    }
    println!("DESIGN: synthetic nominal recovery-card demonstration, not plant calibration.");
    println!(
        "FACT: formula/unit transcriptions are in planning/corpus/C2_nevada_technical_reports.md §3.4."
    );
    println!(
        "DESIGN: all input ranges, intervals and routes are authored; Cortez assumes no arsenic impact."
    );
    println!(
        "DESIGN: each card has multiplier 0.97, correction +0.02 fraction and explicit upper cap 0.90."
    );
    for (case, model, evaluations) in completed {
        println!("\n{}", case.circuit);
        println!(
            "  formula: {} ({:?} source output)",
            case.formula, case.scale
        );
        println!(
            "  authored Au range: [{}, {}] {}",
            case.grades[0], case.grades[2], case.unit_label
        );
        let version = &model.test_work().value.version;
        println!("  retained layer: {} @ {}", version.id, version.revision);
        for operation in model.operations() {
            println!(
                "  retained layer: {} @ {}",
                operation.value.version.id, operation.value.version.revision
            );
        }
        for version in [
            &model.correction().value.version,
            &model.route().value.version,
        ] {
            println!("  retained layer: {} @ {}", version.id, version.revision);
        }
        for (grade, receipt) in evaluations {
            let value = &receipt.claim().value;
            println!(
                "  Au={grade} {}: test-work fraction={:.12}",
                case.unit_label,
                value.test_work_fraction.value()
            );
            for intermediate in &value.operational_fractions {
                println!("    after operational layer={:.12}", intermediate.value());
            }
            println!(
                "    corrected fraction={:.12}; final fraction={:.12}; cap_applied={}",
                value.corrected_before_cap.value(),
                value.recovery_fraction.value(),
                value.cap_applied
            );
            println!(
                "    evidence={:?}; knowledge={:?}; receipt={}",
                receipt.claim().evidence,
                receipt.claim().state,
                receipt.digest()
            );
        }
    }
    Ok(())
}
fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
