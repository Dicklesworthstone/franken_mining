//! Independent nominal unit-conversion answers and source-admission regressions.
//! Determinism: Exact

use super::*;
use fmine_claim::{LabelRegistry, UuTerm};
use fmine_core::{anchor::SourceAnchor, hash::sha256};
use fmine_evidence::{AssertionSource, KuKind};

fn q(value: &str) -> ExactQty {
    ExactQty::from_decimal_str(value).unwrap()
}

fn element(symbol: &str) -> ElementKey {
    ElementKey {
        element: symbol.into(),
        measurand: format!("total-{symbol}/all-phases"),
    }
}

fn source(value: MeasurementValue) -> Claim<MaterialMeasurement> {
    let value = MaterialMeasurement {
        scope: MeasurementScope {
            boundary: "synthetic-boundary/v1".into(),
            material: "feed/period-1".into(),
            physical_time: ValidTime::new(100, 200).unwrap(),
        },
        value,
    };
    let tag = value.canonical_tag();
    let additivity = value.value.additivity();
    Claim::new(
        INPUT_METRIC,
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
            "crates/fmine-plant/src/measurements/tests.rs",
            sha256(include_bytes!("tests.rs")).0,
            format!("constructed source measurement {tag}"),
        )],
    )
}

fn rebind(input: &mut Claim<MaterialMeasurement>) {
    *input = Claim::new(
        input.metric_key.clone(),
        input.value.clone(),
        &input.value.canonical_tag(),
        input.representation.clone(),
        input.support,
        input.additivity,
        input.evidence,
        input.epistemic,
        input.state,
        input.confidentiality.clone(),
        input.uu_terms.clone(),
        input.parents.clone(),
        input.anchors.clone(),
    );
}

fn dry_inputs() -> Vec<Claim<MaterialMeasurement>> {
    vec![
        source(MeasurementValue::DryMass {
            value: q("2.5"),
            unit: MassUnit::Tonnes,
        }),
        source(MeasurementValue::WaterMass {
            value: q("125"),
            unit: MassUnit::Kilograms,
        }),
        source(MeasurementValue::DryAssay {
            element: element("Au"),
            value: q("4"),
            unit: AssayUnit::GramsPerTonne,
        }),
        source(MeasurementValue::DryAssay {
            element: element("Cu"),
            value: q("2"),
            unit: AssayUnit::MassPercent,
        }),
    ]
}

fn wet_inputs() -> Vec<Claim<MaterialMeasurement>> {
    vec![
        source(MeasurementValue::WetMass {
            value: q("1000"),
            unit: MassUnit::Tonnes,
        }),
        source(MeasurementValue::Moisture {
            water_over_wet_mass: q("0.08"),
        }),
        source(MeasurementValue::DryAssay {
            element: element("Cu"),
            value: q("0.006"),
            unit: AssayUnit::MassFraction,
        }),
    ]
}

fn refusal(inputs: &[Claim<MaterialMeasurement>], code: RefusalCode) {
    assert_eq!(project_material(inputs).unwrap_err().code, code.as_str());
}

#[test]
fn dry_tonnes_and_distinct_assay_units_project_exact_elemental_kilograms() {
    // Independent arithmetic: 2.5 t * 4 g/t = 10 g Au; 2% of 2500 kg = 50 kg Cu.
    let inputs = dry_inputs();
    let receipt = project_material(&inputs).unwrap();
    let result = &receipt.summary().amounts;
    assert_eq!(result.dry_solids_kg, q("2500"));
    assert_eq!(result.water_kg, q("125"));
    assert_eq!(result.elements_kg[&element("Au")], q("0.01"));
    assert_eq!(result.elements_kg[&element("Cu")], q("50"));
    assert_eq!(receipt.inputs(), inputs);
    assert_eq!(receipt.claim().parents.len(), inputs.len());
    assert_eq!(receipt.claim().anchors.len(), inputs.len());
    assert_eq!(receipt.claim().evidence, EvidenceClass::Assumed);
    assert_eq!(receipt.claim().metric_key, OUTPUT_METRIC);
    assert_eq!(receipt.claim().value_tag(), receipt.digest());
    assert_eq!(receipt.summary().scope, inputs[0].value.scope);
    receipt.claim().check_id().unwrap();
}

#[test]
fn wet_basis_moisture_removes_water_before_applying_the_dry_assay() {
    // 1000 wet t contains 80 t H2O and 920 dry t; 0.6% gives 5.52 t Cu.
    let receipt = project_material(&wet_inputs()).unwrap();
    let result = &receipt.summary().amounts;
    assert_eq!(result.dry_solids_kg, q("920000"));
    assert_eq!(result.water_kg, q("80000"));
    assert_eq!(result.elements_kg[&element("Cu")], q("5520"));
    assert_eq!(
        result.dry_solids_kg.checked_add(result.water_kg).unwrap(),
        q("1000000")
    );
    assert_ne!(result.elements_kg[&element("Cu")], q("6000"));
}

#[test]
fn declared_equivalent_units_and_mass_routes_agree_without_merging_source_identities() {
    let baseline = project_material(&dry_inputs()).unwrap();
    let mut inputs = dry_inputs();
    inputs[0] = source(MeasurementValue::DryMass {
        value: q("2500"),
        unit: MassUnit::Kilograms,
    });
    inputs[1] = source(MeasurementValue::WaterMass {
        value: q("0.125"),
        unit: MassUnit::Tonnes,
    });
    inputs[2] = source(MeasurementValue::DryAssay {
        element: element("Au"),
        value: q("0.000004"),
        unit: AssayUnit::MassFraction,
    });
    inputs[3] = source(MeasurementValue::DryAssay {
        element: element("Cu"),
        value: q("20000"),
        unit: AssayUnit::GramsPerTonne,
    });
    let equivalent = project_material(&inputs).unwrap();
    assert_eq!(baseline.summary(), equivalent.summary());
    assert_ne!(baseline.digest(), equivalent.digest());

    inputs[0] = source(MeasurementValue::WetMass {
        value: q("2625"),
        unit: MassUnit::Kilograms,
    });
    inputs[1] = source(MeasurementValue::Moisture {
        water_over_wet_mass: ExactQty::new(1, 21).unwrap(),
    });
    let wet = project_material(&inputs).unwrap();
    assert_eq!(baseline.summary(), wet.summary());
    assert_ne!(baseline.digest(), wet.digest());
}

#[test]
fn bead_monthly_copper_example_keeps_the_unbalanced_original_metal_amounts() {
    // BD-336's constructed example: 12000 t in, 11000 + 975 t out. The 25 t
    // discrepancy is retained; projection is not a hidden balance adjustment.
    let project = |tonnes: &str, grade_percent: &str| {
        let inputs = [
            source(MeasurementValue::DryMass {
                value: q(tonnes),
                unit: MassUnit::Tonnes,
            }),
            source(MeasurementValue::WaterMass {
                value: ZERO,
                unit: MassUnit::Kilograms,
            }),
            source(MeasurementValue::DryAssay {
                element: element("Cu"),
                value: q(grade_percent),
                unit: AssayUnit::MassPercent,
            }),
        ];
        project_material(&inputs)
            .unwrap()
            .summary()
            .amounts
            .elements_kg[&element("Cu")]
    };
    let feed = project("2000000", "0.60");
    let concentrate = project("50000", "22");
    let tails = project("1950000", "0.05");
    assert_eq!(feed, q("12000000"));
    assert_eq!(concentrate, q("11000000"));
    assert_eq!(tails, q("975000"));
    assert_eq!(
        feed.checked_sub(concentrate)
            .unwrap()
            .checked_sub(tails)
            .unwrap(),
        q("25000")
    );
}

#[test]
fn all_measurements_need_one_material_time_and_supported_spatial_support() {
    for field in 0..4 {
        let mut inputs = dry_inputs();
        let scope = &mut inputs[2].value.scope;
        match field {
            0 => scope.boundary = "synthetic-boundary/v2".into(),
            1 => scope.material = "other-lot".into(),
            2 => scope.physical_time.start_ns += 1,
            _ => scope.physical_time.end_ns += 1,
        }
        rebind(&mut inputs[2]);
        refusal(&inputs, RefusalCode::SupportMismatch102);
    }
    for support in [SpatialSupport::PointCore, SpatialSupport::PlantLot] {
        let mut inputs = dry_inputs();
        inputs[2].support = support;
        refusal(&inputs, RefusalCode::SupportMismatch102);
    }
    for slot in 0..4 {
        let mut inputs = dry_inputs();
        inputs[slot].additivity = if slot < 2 {
            AdditivityClass::IntensiveWeighted
        } else {
            AdditivityClass::ExtensiveAdditive
        };
        refusal(&inputs, RefusalCode::NonAdditiveAverage103);
    }
    let mut wet = wet_inputs();
    wet[1].additivity = AdditivityClass::ExtensiveAdditive;
    refusal(&wet, RefusalCode::NonAdditiveAverage103);
}

#[test]
fn material_stocktake_instants_are_allowed_but_reversed_or_unnamed_supports_refuse() {
    let mut inputs = dry_inputs();
    for input in &mut inputs {
        input.value.scope.physical_time = ValidTime::instant(100);
        rebind(input);
    }
    assert!(project_material(&inputs).is_ok());
    inputs[0].value.scope.physical_time.end_ns = 99;
    rebind(&mut inputs[0]);
    refusal(&inputs, RefusalCode::OutsidePhysicalDomain109);
    for boundary in [true, false] {
        let mut inputs = dry_inputs();
        if boundary {
            inputs[0].value.scope.boundary.clear();
        } else {
            inputs[0].value.scope.material = " name-with-space ".into();
        }
        rebind(&mut inputs[0]);
        refusal(&inputs, RefusalCode::UndeclaredUnitBasisOrTz107);
    }
}

#[test]
fn missing_conflicting_and_duplicated_mass_routes_or_assays_refuse() {
    let original = dry_inputs();
    for slot in [0, 1] {
        let mut inputs = original.clone();
        inputs.remove(slot);
        refusal(&inputs, RefusalCode::BasisMismatch105);
    }
    let mut inputs = original.clone();
    inputs.push(original[0].clone());
    refusal(&inputs, RefusalCode::BasisMismatch105);
    let mut inputs = original.clone();
    inputs.push(wet_inputs()[0].clone());
    refusal(&inputs, RefusalCode::BasisMismatch105);
    let mut inputs = wet_inputs();
    inputs.push(original[1].clone());
    refusal(&inputs, RefusalCode::BasisMismatch105);
    let mut inputs = original.clone();
    inputs.push(original[2].clone());
    refusal(&inputs, RefusalCode::BasisMismatch105);
    let mut alternative = element("Cu");
    alternative.measurand = "different-total-Cu-method".into();
    let mut inputs = original;
    inputs.push(source(MeasurementValue::DryAssay {
        element: alternative,
        value: q("2"),
        unit: AssayUnit::MassPercent,
    }));
    refusal(&inputs, RefusalCode::BasisMismatch105);
    refusal(&[], RefusalCode::NoClaimInDomain201);
    refusal(&dry_inputs()[..2], RefusalCode::NoClaimInDomain201);
}

#[test]
fn physical_mass_moisture_and_assay_ranges_are_exact_and_never_clamped() {
    for slot in [0, 1] {
        let mut inputs = dry_inputs();
        match &mut inputs[slot].value.value {
            MeasurementValue::DryMass { value, .. } | MeasurementValue::WaterMass { value, .. } => {
                *value = q("-0.0001")
            }
            _ => unreachable!(),
        }
        rebind(&mut inputs[slot]);
        refusal(&inputs, RefusalCode::OutsidePhysicalDomain109);
    }
    for ratio in ["-0.01", "1", "8"] {
        let mut inputs = wet_inputs();
        inputs[1] = source(MeasurementValue::Moisture {
            water_over_wet_mass: q(ratio),
        });
        refusal(&inputs, RefusalCode::OutsidePhysicalDomain109);
    }
    for (unit, value) in [
        (AssayUnit::MassFraction, "-0.01"),
        (AssayUnit::MassFraction, "1.000000001"),
        (AssayUnit::MassPercent, "100.0001"),
        (AssayUnit::GramsPerTonne, "1000000.0001"),
    ] {
        let mut inputs = wet_inputs();
        inputs[2] = source(MeasurementValue::DryAssay {
            element: element("Cu"),
            value: q(value),
            unit,
        });
        refusal(&inputs, RefusalCode::OutsidePhysicalDomain109);
    }
    let mut inputs = wet_inputs();
    inputs.push(source(MeasurementValue::DryAssay {
        element: element("Fe"),
        value: q("0.995"),
        unit: AssayUnit::MassFraction,
    }));
    refusal(&inputs, RefusalCode::OutsidePhysicalDomain109);
}

#[test]
fn undeclared_elemental_basis_and_implicit_water_stoichiometry_refuse() {
    for symbol in ["", "cu", "CU", "CuO", "Cu "] {
        let mut inputs = wet_inputs();
        inputs[2] = source(MeasurementValue::DryAssay {
            element: element(symbol),
            value: q("0.006"),
            unit: AssayUnit::MassFraction,
        });
        refusal(&inputs, RefusalCode::UndeclaredUnitBasisOrTz107);
    }
    for symbol in ["H", "O"] {
        let mut inputs = wet_inputs();
        inputs[2] = source(MeasurementValue::DryAssay {
            element: element(symbol),
            value: q("0.006"),
            unit: AssayUnit::MassFraction,
        });
        refusal(&inputs, RefusalCode::BasisMismatch105);
    }
}

#[test]
fn explicit_zero_assays_survive_and_zero_dry_mass_cannot_carry_nonzero_grade() {
    let mut inputs = wet_inputs();
    inputs[2] = source(MeasurementValue::DryAssay {
        element: element("Cu"),
        value: ZERO,
        unit: AssayUnit::MassFraction,
    });
    let result = project_material(&inputs).unwrap();
    assert_eq!(result.summary().amounts.elements_kg[&element("Cu")], ZERO);
    inputs[0] = source(MeasurementValue::WetMass {
        value: ZERO,
        unit: MassUnit::Kilograms,
    });
    assert!(project_material(&inputs).is_ok());
    inputs[2] = wet_inputs()[2].clone();
    refusal(&inputs, RefusalCode::NotObservable202);
}

#[test]
fn exact_projection_overflow_refuses_without_a_rounded_number() {
    let mut inputs = wet_inputs();
    inputs[0] = source(MeasurementValue::WetMass {
        value: ExactQty::from_integer(i128::MAX),
        unit: MassUnit::Tonnes,
    });
    refusal(&inputs, RefusalCode::NumericalCertificateFailed207);
}

#[test]
fn typed_values_units_measurands_and_scope_are_bound_independently_of_claim_ids() {
    for field in 0..6 {
        let mut inputs = wet_inputs();
        match field {
            0 => {
                inputs[0].value.value = MeasurementValue::WetMass {
                    value: q("1001"),
                    unit: MassUnit::Tonnes,
                };
            }
            1 => {
                inputs[0].value.value = MeasurementValue::WetMass {
                    value: q("1000"),
                    unit: MassUnit::Kilograms,
                };
            }
            2 => inputs[0].value.scope.material.push_str("-changed"),
            3 => inputs[0].value.scope.physical_time.start_ns += 1,
            4 => inputs[0].value.scope.boundary.push_str("-changed"),
            _ => {
                inputs[0].value.value = MeasurementValue::DryMass {
                    value: q("1000"),
                    unit: MassUnit::Tonnes,
                };
            }
        }
        inputs[0].check_id().unwrap();
        refusal(&inputs, RefusalCode::FrozenRecordMutation507);
    }
    for field in 0..4 {
        let mut inputs = wet_inputs();
        if let MeasurementValue::DryAssay {
            element,
            value,
            unit,
        } = &mut inputs[2].value.value
        {
            match field {
                0 => element.element = "Au".into(),
                1 => element.measurand.push_str("-different-method"),
                2 => *value = q("0.007"),
                _ => *unit = AssayUnit::MassPercent,
            }
        }
        inputs[2].check_id().unwrap();
        refusal(&inputs, RefusalCode::FrozenRecordMutation507);
    }
    let mut inputs = wet_inputs();
    inputs[1].value.value = MeasurementValue::Moisture {
        water_over_wet_mass: q("0.09"),
    };
    inputs[1].check_id().unwrap();
    refusal(&inputs, RefusalCode::FrozenRecordMutation507);
    let mut inputs = wet_inputs();
    inputs[2].metric_key = "wrong.metric".into();
    rebind(&mut inputs[2]);
    refusal(&inputs, RefusalCode::FrozenRecordMutation507);
    let mut inputs = wet_inputs();
    inputs[2].anchors[0].locator.push_str("-changed");
    refusal(&inputs, RefusalCode::FrozenRecordMutation507);
}

#[test]
fn unavailable_uncertain_and_unanchored_inputs_do_not_become_material_amounts() {
    for state in [
        KnowledgeState::Unknown,
        KnowledgeState::Conflicted,
        KnowledgeState::NotObservable,
    ] {
        let mut inputs = wet_inputs();
        inputs[2].state = state;
        rebind(&mut inputs[2]);
        refusal(&inputs, RefusalCode::NotObservable202);
    }
    let mut inputs = wet_inputs();
    inputs[2].representation = Representation::Interval { lo: 0.0, hi: 1.0 };
    refusal(&inputs, RefusalCode::NotObservable202);
    let mut inputs = wet_inputs();
    inputs[2].stale = true;
    refusal(&inputs, RefusalCode::StaleAnchor502);
    let mut inputs = wet_inputs();
    inputs[2].calibration_failed = true;
    refusal(&inputs, RefusalCode::CalibrationRequired205);
    let mut inputs = wet_inputs();
    inputs[2].anchors.clear();
    rebind(&mut inputs[2]);
    refusal(&inputs, RefusalCode::NoClaimInDomain201);
}

#[test]
fn every_source_retains_its_evidence_and_confidentiality() {
    let labels = LabelRegistry::new(["JV-A"], ["W-A"]).unwrap();
    let venture = Confidentiality::venture(labels.venture("JV-A").unwrap());
    let wall = Confidentiality::mnpi(labels.wall("W-A").unwrap());
    for slot in 0..3 {
        let mut inputs = wet_inputs();
        for input in &mut inputs {
            input.evidence = EvidenceClass::Measured;
            rebind(input);
        }
        inputs[slot].evidence = EvidenceClass::Assertion {
            source: AssertionSource::Extraction,
        };
        inputs[slot].epistemic = EpistemicStatus::KnownUnknown(KuKind::Parametric);
        inputs[slot].state = KnowledgeState::Estimated;
        inputs[slot].confidentiality = venture.clone();
        rebind(&mut inputs[slot]);
        let other = (slot + 1) % inputs.len();
        inputs[other].confidentiality = wall.clone();
        rebind(&mut inputs[other]);
        let result = project_material(&inputs).unwrap();
        assert_eq!(result.claim().evidence, inputs[slot].evidence);
        assert!(result.claim().color().is_err());
        assert_eq!(result.claim().epistemic, inputs[slot].epistemic);
        assert_eq!(result.claim().state, KnowledgeState::Estimated);
        assert_eq!(result.claim().confidentiality, venture.join(&wall));
        assert!(
            result
                .claim()
                .verify_export_clearance(&Confidentiality::public())
                .is_err()
        );
    }
}

fn uu() -> UuTerm {
    UuTerm {
        scenario_id: "unrepresentative-assay".into(),
        description: "Constructed assay sample misses segregated material".into(),
        downside_exposure_usd_m: 1.0,
        trigger_indicator: "Independent full-material resampling differs".into(),
    }
}

#[test]
fn metadata_changes_and_input_order_have_their_required_receipt_identity_effects() {
    let mut inputs = dry_inputs();
    let baseline = project_material(&inputs).unwrap();
    inputs.reverse();
    assert_eq!(project_material(&inputs).unwrap(), baseline);
    inputs[0].uu_terms.push(uu());
    inputs[1].uu_terms.push(uu());
    let result = project_material(&inputs).unwrap();
    assert_eq!(result.claim().uu_terms, vec![uu()]);
    assert_ne!(result.digest(), baseline.digest());
    inputs[1].uu_terms[0].description.push_str(" changed");
    refusal(&inputs, RefusalCode::UnbudgetedUnknowns204);
    inputs[1].uu_terms[0] = uu();
    inputs[1].supersedes = Some(baseline.claim().claim_id);
    assert_ne!(project_material(&inputs).unwrap().digest(), result.digest());
    let mut cloned_output = result.claim().clone();
    cloned_output.value.amounts.dry_solids_kg = ZERO;
    assert_eq!(result.summary().amounts.dry_solids_kg, q("2500"));
}
