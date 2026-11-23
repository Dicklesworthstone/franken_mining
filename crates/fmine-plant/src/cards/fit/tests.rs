//! FACT: synthetic exact residual oracles, chronology and source-retention controls.
//! Determinism: Exact (nominal application inherits the card's binary64 arithmetic).

use super::*;
use fmine_claim::{Confidentiality, Representation, SpatialSupport, UuTerm};
use fmine_core::{anchor::SourceAnchor, hash::sha256};
use fmine_evidence::{EpistemicStatus, EvidenceClass, KnowledgeState};

fn q(n: i128, d: i128) -> ExactQty {
    ExactQty::new(n, d).unwrap()
}
fn revision(id: &str) -> CardVersion {
    CardVersion {
        id: id.into(),
        revision: "synthetic-v1".into(),
    }
}
fn source<T>(metric: &str, value: T, tag: &str) -> Claim<T> {
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
        vec![SourceAnchor::new(
            "crates/fmine-plant/src/cards/fit/tests.rs",
            sha256(include_bytes!("tests.rs")).0,
            "synthetic aligned recovery pairs and model layers",
        )],
    )
}
fn pair(value: RecoveryPair) -> Claim<RecoveryPair> {
    let tag = value.canonical_tag();
    source(PAIR_METRIC, value, &tag)
}
fn input(value: CorrectionFit) -> Claim<CorrectionFit> {
    let tag = value.canonical_tag();
    source(INPUT_METRIC, value, &tag)
}
fn correction(value: RecoveryCorrection) -> Claim<RecoveryCorrection> {
    let tag = value.canonical_tag();
    source(CORRECTION_METRIC, value, &tag)
}
fn fixture() -> (RecoveryModel, CorrectionFit) {
    let scope = RecoveryScope {
        circuit: "synthetic-circuit".into(),
        material_domain: "synthetic-ore".into(),
        dry_basis: "matched-dry-basis/v1".into(),
        element: "Au".into(),
    };
    let card = TestWorkCard {
        version: revision("test-work"),
        scope: scope.clone(),
        valid_time: ValidTime {
            start_ns: 0,
            end_ns: 1000,
        },
        input_units: BTreeMap::from([(
            "g".into(),
            InputRequirement {
                measurand: "Au/dry".into(),
                unit: InputUnit::GramsPerDryMetricTonne,
                lower: q(0, 1),
                upper: q(10, 1),
                lower_inclusive: true,
                upper_inclusive: true,
            },
        )]),
        formula: "g".into(),
        output_scale: OutputScale::Fraction,
    };
    let tag = card.canonical_tag();
    let card = source(TEST_WORK_METRIC, card, &tag);
    let operations: Vec<_> = [q(1, 2), q(4, 5)]
        .into_iter()
        .enumerate()
        .map(|(i, multiplier)| {
            let factor = OperationalFactor {
                version: revision(&format!("operation-{i}")),
                test_work: card.claim_id,
                multiplier,
            };
            let tag = factor.canonical_tag();
            source(OPERATION_METRIC, factor, &tag)
        })
        .collect();
    let old = correction(RecoveryCorrection {
        version: revision("old-correction"),
        test_work: card.claim_id,
        additive_fraction: q(1, 20),
        upper_cap_fraction: Some(q(9, 10)),
        fit_validity: None,
    });
    let route = RouteAssumption {
        version: revision("route"),
        test_work: card.claim_id,
        route: "synthetic-process".into(),
    };
    let tag = route.canonical_tag();
    let route = source(ROUTE_METRIC, route, &tag);
    let model = RecoveryModel::compile(&card, &operations, &old, &route).unwrap();
    let pairs = [(q(1, 5), q(3, 10)), (q(2, 5), q(3, 5)), (q(3, 5), q(3, 5))]
        .into_iter()
        .enumerate()
        .map(|(i, (baseline_fraction, observed_fraction))| {
            pair(RecoveryPair {
                id: format!("pair-{i}"),
                interval: ValidTime {
                    start_ns: i as i64 * 100,
                    end_ns: (i as i64 + 1) * 100,
                },
                available_ns: (i as i64 + 1) * 100 + 10,
                baseline_fraction,
                observed_fraction,
                uncensored: true,
                baseline_model: None,
            })
        })
        .collect();
    let declaration = CorrectionFit {
        version: revision("fitted-correction"),
        scope,
        test_work: card.claim_id,
        operation_ids: operations
            .iter()
            .map(|operation| operation.claim_id)
            .collect(),
        route: route.claim_id,
        evidence_cut_ns: 400,
        target_interval: ValidTime {
            start_ns: 500,
            end_ns: 700,
        },
        alignment: FitAlignment::SameProcessingSupport,
        upper_cap_fraction: None,
        pairs,
    };
    (model, declaration)
}
fn query(model: &RecoveryModel, start_ns: i64, end_ns: i64) -> Claim<RecoveryInputs> {
    let value = RecoveryInputs {
        scope: model.test_work().value.scope.clone(),
        interval: ValidTime { start_ns, end_ns },
        values: BTreeMap::from([(
            "g".into(),
            InputValue {
                measurand: "Au/dry".into(),
                unit: InputUnit::GramsPerDryMetricTonne,
                value: q(2, 1),
            },
        )]),
    };
    let tag = value.canonical_tag();
    source(super::super::INPUT_METRIC, value, &tag)
}

#[test]
fn exact_residual_oracle_keeps_sources_and_uses_n_minus_one_variance() {
    let (model, declaration) = fixture();
    let declaration = input(declaration);
    let receipt = fit_correction(&model, &declaration).unwrap();
    let summary = &receipt.summary().value;
    assert_eq!(summary.pair_count, 3);
    // Residuals [1/10, 1/5, 0] have mean 1/10 and sample variance 1/100.
    assert_eq!(summary.mean_residual_fraction, q(1, 10));
    assert_eq!(summary.sample_residual_variance, q(1, 100));
    assert_eq!(
        summary
            .residuals
            .iter()
            .map(|r| r.residual_fraction)
            .collect::<Vec<_>>(),
        [q(1, 10), q(1, 5), q(0, 1)]
    );
    assert_eq!(receipt.input(), &declaration);
    assert_eq!(receipt.model(), &model);
    assert_eq!(receipt.correction().value.additive_fraction, q(1, 10));
    assert_eq!(model.correction().value.additive_fraction, q(1, 20));
    let validity = receipt.correction().value.fit_validity.as_ref().unwrap();
    assert_eq!(validity.fit_digest, receipt.digest());
    assert_eq!(validity.operation_ids, declaration.value.operation_ids);
    assert_eq!(validity.route, model.route().claim_id);
    for parent in [
        model.test_work().claim_id,
        model.correction().claim_id,
        model.route().claim_id,
    ] {
        assert!(receipt.summary().parents.contains(&parent));
        assert!(receipt.correction().parents.contains(&parent));
    }
    for pair in &declaration.value.pairs {
        assert!(receipt.summary().parents.contains(&pair.claim_id));
        assert!(receipt.correction().parents.contains(&pair.claim_id));
    }
    assert_eq!(receipt.summary().evidence, EvidenceClass::Assumed);
    assert_eq!(receipt.summary().state, KnowledgeState::Estimated);
    assert_eq!(receipt, fit_correction(&model, &declaration).unwrap());
    let mut reordered = declaration.value.clone();
    reordered.pairs.reverse();
    let reordered = fit_correction(&model, &input(reordered)).unwrap();
    assert_eq!(receipt.digest(), reordered.digest());
    assert_eq!(receipt.summary(), reordered.summary());
    assert_eq!(receipt.correction(), reordered.correction());
}

#[test]
fn future_application_replaces_old_correction_and_checks_interval_in_every_model() {
    let (model, declaration) = fixture();
    let receipt = fit_correction(&model, &input(declaration.clone())).unwrap();
    let applied = model.with_correction(receipt.correction()).unwrap();
    // Operational baseline is 2 * 1/2 * 4/5 = 4/5. The old 1/20 is not added.
    let value = applied.evaluate(&query(&applied, 500, 700)).unwrap();
    assert!((value.claim().value.recovery_fraction.value() - 0.9).abs() < 1e-15);
    assert_eq!(value.model().correction(), receipt.correction());
    let recompiled = RecoveryModel::compile(
        model.test_work(),
        model.operations(),
        receipt.correction(),
        model.route(),
    )
    .unwrap();
    for candidate in [&applied, &recompiled] {
        candidate.evaluate(&query(candidate, 600, 650)).unwrap();
        for (start, end) in [(100, 200), (400, 500), (499, 600), (600, 701)] {
            assert_eq!(
                candidate
                    .evaluate(&query(candidate, start, end))
                    .unwrap_err()
                    .code,
                "FMINE-REF-203"
            );
        }
    }
    let mut capped = declaration;
    capped.upper_cap_fraction = Some(q(22, 25));
    let capped = fit_correction(&model, &input(capped)).unwrap();
    let model = model.with_correction(capped.correction()).unwrap();
    let value = model.evaluate(&query(&model, 500, 700)).unwrap();
    assert!(value.claim().value.cap_applied);
    assert!((value.claim().value.recovery_fraction.value() - 0.88).abs() < 1e-15);
}

#[test]
fn fitted_claim_cannot_be_transferred_to_different_operations_or_route() {
    let (model, declaration) = fixture();
    let receipt = fit_correction(&model, &input(declaration)).unwrap();
    let mut operations = model.operations().to_vec();
    operations.reverse();
    assert_eq!(
        RecoveryModel::compile(
            model.test_work(),
            &operations,
            receipt.correction(),
            model.route()
        )
        .unwrap_err()
        .code,
        "FMINE-REF-507"
    );
    let changed = RecoveryModel::compile(
        model.test_work(),
        &operations,
        model.correction(),
        model.route(),
    )
    .unwrap();
    assert_eq!(
        changed
            .with_correction(receipt.correction())
            .unwrap_err()
            .code,
        "FMINE-REF-507"
    );
    let mut route = model.route().value.clone();
    route.version.revision = "different-route-version".into();
    let tag = route.canonical_tag();
    let route = source(ROUTE_METRIC, route, &tag);
    assert_eq!(
        RecoveryModel::compile(
            model.test_work(),
            model.operations(),
            receipt.correction(),
            &route
        )
        .unwrap_err()
        .code,
        "FMINE-REF-507"
    );
}

#[test]
fn historical_pairs_require_completed_available_nondoubled_support() {
    let (model, declaration) = fixture();
    for (case, expected) in [
        (0, "FMINE-REF-102"),
        (1, "FMINE-REF-203"),
        (2, "FMINE-REF-203"),
        (3, "FMINE-REF-203"),
        (4, "FMINE-REF-109"),
        (5, "FMINE-REF-201"),
    ] {
        let mut changed = declaration.clone();
        let mut value = changed.pairs[1].value.clone();
        match case {
            0 => value.interval.start_ns = 99,
            1 => value.available_ns = 199,
            2 => value.available_ns = 401,
            3 => value.interval.start_ns = -1,
            4 => value.interval.end_ns = value.interval.start_ns,
            5 => value.id = changed.pairs[0].value.id.clone(),
            _ => unreachable!(),
        }
        changed.pairs[1] = pair(value);
        assert_eq!(
            fit_correction(&model, &input(changed)).unwrap_err().code,
            expected
        );
    }
    for count in [1, MAX_PAIRS + 1] {
        let mut changed = declaration.clone();
        changed.pairs = vec![changed.pairs[0].clone(); count];
        assert_eq!(
            fit_correction(&model, &input(changed)).unwrap_err().code,
            "FMINE-REF-201"
        );
    }
}

#[test]
fn fit_requires_future_target_and_exact_complete_baseline_identity() {
    let (model, declaration) = fixture();
    for case in 0..8 {
        let mut changed = declaration.clone();
        let expected = match case {
            0 => {
                changed.evidence_cut_ns = 500;
                "FMINE-REF-203"
            }
            1 => {
                changed.target_interval.end_ns = 1001;
                "FMINE-REF-203"
            }
            2 => {
                changed.scope.circuit = "another-circuit".into();
                "FMINE-REF-102"
            }
            3 => {
                changed.test_work = model.route().claim_id;
                "FMINE-REF-102"
            }
            4 => {
                changed.operation_ids.reverse();
                "FMINE-REF-102"
            }
            5 => {
                let _ = changed.operation_ids.pop();
                "FMINE-REF-102"
            }
            6 => {
                changed.route = model.test_work().claim_id;
                "FMINE-REF-102"
            }
            7 => {
                changed.version = model.correction().value.version.clone();
                "FMINE-REF-507"
            }
            _ => unreachable!(),
        };
        assert_eq!(
            fit_correction(&model, &input(changed)).unwrap_err().code,
            expected
        );
    }
}

#[test]
fn capped_pairs_and_nonphysical_observations_refuse_but_uncapped_baselines_may_exceed_one() {
    let (model, declaration) = fixture();
    for case in 0..4 {
        let mut changed = declaration.clone();
        let mut value = changed.pairs[0].value.clone();
        match case {
            0 => value.uncensored = false,
            1 => value.observed_fraction = q(1001, 1000),
            2 => value.observed_fraction = q(-1, 1000),
            3 => value.baseline_fraction = q(-1, 1000),
            _ => unreachable!(),
        }
        changed.pairs[0] = pair(value);
        let expected = if case == 0 {
            "FMINE-REF-202"
        } else {
            "FMINE-REF-109"
        };
        assert_eq!(
            fit_correction(&model, &input(changed)).unwrap_err().code,
            expected
        );
    }
    let mut negative = declaration;
    for pair_claim in &mut negative.pairs {
        let mut value = pair_claim.value.clone();
        value.baseline_fraction = q(6, 5);
        value.observed_fraction = q(9, 10);
        *pair_claim = pair(value);
    }
    let receipt = fit_correction(&model, &input(negative)).unwrap();
    assert_eq!(receipt.summary().value.mean_residual_fraction, q(-3, 10));
    assert_eq!(receipt.summary().value.sample_residual_variance, q(0, 1));
}

#[test]
fn source_mutation_staleness_and_unrepresentable_variance_fail_closed() {
    let (model, declaration) = fixture();
    let mut changed = input(declaration.clone());
    changed.value.pairs[0].value.observed_fraction = q(2, 5);
    assert_eq!(
        fit_correction(&model, &changed).unwrap_err().code,
        "FMINE-REF-507"
    );
    let mut changed = declaration.clone();
    changed.pairs[0].stale = true;
    assert_eq!(
        fit_correction(&model, &input(changed)).unwrap_err().code,
        "FMINE-REF-502"
    );
    let mut overflow = declaration;
    let mut value = overflow.pairs[0].value.clone();
    value.baseline_fraction = q(10_000_000_000_000_000_000, 1);
    value.observed_fraction = q(0, 1);
    overflow.pairs[0] = pair(value);
    assert_eq!(
        fit_correction(&model, &input(overflow)).unwrap_err().code,
        "FMINE-REF-207"
    );
}

#[test]
fn complete_nominal_and_pair_metadata_taint_the_fit_and_fitted_correction() {
    let (model, mut declaration) = fixture();
    let original = fit_correction(&model, &input(declaration.clone())).unwrap();
    let old = model.correction();
    let mut old = Claim::new(
        &old.metric_key,
        old.value.clone(),
        old.value_tag(),
        old.representation.clone(),
        old.support,
        old.additivity,
        old.evidence,
        old.epistemic,
        old.state,
        Confidentiality::internal(),
        vec![UuTerm {
            scenario_id: "old-model-gap".into(),
            description: "Retained uncertainty in the original correction".into(),
            downside_exposure_usd_m: 1.0,
            trigger_indicator: "independent recovery reconciliation".into(),
        }],
        old.parents.clone(),
        old.anchors.clone(),
    );
    old.supersedes = Some(model.route().claim_id);
    let tainted = model.with_correction(&old).unwrap();
    declaration.pairs[0].uu_terms.push(UuTerm {
        scenario_id: "pair-alignment-gap".into(),
        description: "Retained unrepresented residence-time spread".into(),
        downside_exposure_usd_m: 2.0,
        trigger_indicator: "new tracer survey".into(),
    });
    let receipt = fit_correction(&tainted, &input(declaration)).unwrap();
    assert_ne!(receipt.digest(), original.digest());
    assert_eq!(receipt.model().correction(), &old);
    assert_eq!(
        receipt.summary().confidentiality,
        Confidentiality::internal()
    );
    assert_eq!(
        receipt.correction().confidentiality,
        Confidentiality::internal()
    );
    assert_eq!(receipt.summary().uu_terms.len(), 2);
    assert_eq!(receipt.correction().uu_terms, receipt.summary().uu_terms);
    assert!(receipt.correction().parents.contains(&old.claim_id));
    let mut no_supersession = old.clone();
    no_supersession.supersedes = None;
    assert_eq!(old.claim_id, no_supersession.claim_id);
    let alternate = tainted.with_correction(&no_supersession).unwrap();
    let alternate = fit_correction(&alternate, receipt.input()).unwrap();
    assert_ne!(receipt.digest(), alternate.digest());
    assert_ne!(
        receipt.correction().claim_id,
        alternate.correction().claim_id
    );
}

#[test]
fn old_correction_tags_stay_identical_and_fitted_chronology_is_bound() {
    let (model, declaration) = fixture();
    let old = &model.correction().value;
    let mut enc = layer_encoder(*b"RCCO", &old.version, old.test_work);
    put_exact(&mut enc, old.additive_fraction);
    enc.put_u8(u8::from(old.upper_cap_fraction.is_some()));
    if let Some(cap) = old.upper_cap_fraction {
        put_exact(&mut enc, cap);
    }
    assert_eq!(old.canonical_tag(), blake3_hash(&enc.finish()).to_hex());
    let receipt = fit_correction(&model, &input(declaration)).unwrap();
    let mut changed = receipt.correction().value.clone();
    changed.fit_validity.as_mut().unwrap().evidence_cut_ns = 500;
    assert_ne!(
        changed.canonical_tag(),
        receipt.correction().value.canonical_tag()
    );
    assert_eq!(
        model
            .with_correction(&correction(changed))
            .unwrap_err()
            .code,
        "FMINE-REF-203"
    );
    let mut changed = receipt.correction().value.clone();
    changed.fit_validity.as_mut().unwrap().fit_digest = "invalid".into();
    assert_eq!(
        model
            .with_correction(&correction(changed))
            .unwrap_err()
            .code,
        "FMINE-REF-507"
    );
}
