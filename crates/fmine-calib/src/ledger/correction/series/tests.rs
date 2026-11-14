//! Synthetic public-calendar series exercise the public API, with hand-derived correction answers.
//! Determinism: DeterministicPerIsa

use std::collections::BTreeMap;

use crate::ledger::binding::{
    PublicMetric, PublicOwnership, PublicPeriod, PublicResolutionPolicy, PublicVintagePolicy,
};
use crate::ledger::correction::CorrectionMethod;
use crate::ledger::{
    AssessmentPolicy, CohortSpec, ForecastLedger, ForecastMetadata, ForecastPayload,
    OutcomeCitation, OutcomeKind, OutcomeSelector,
};
use fmine_core::{Digest32, ObjectId, sha256};

#[derive(Clone)]
struct Spec {
    period: PublicPeriod,
    public: bool,
    issuer: String,
    ownership: PublicOwnership,
    vintages: PublicVintagePolicy,
    model: String,
    metadata: ForecastMetadata,
    metric: PublicMetric,
    mean: f64,
    std_dev: f64,
    alpha: f64,
    selector: OutcomeSelector,
    outcome: Option<f64>,
    observation: usize,
}

fn specs() -> [Spec; 3] {
    std::array::from_fn(|i| Spec {
        period: PublicPeriod::Quarter {
            year: 2024,
            quarter: i as u8 + 2,
        },
        public: true,
        issuer: "synthetic".into(),
        ownership: PublicOwnership::Full,
        vintages: PublicVintagePolicy::RegisteredComparatives,
        model: "synthetic-quarterly-model".into(),
        metadata: ForecastMetadata {
            family: "quarterly-production".into(),
            author_role: "synthetic-model".into(),
            domain: "same-domain".into(),
            horizon: "one-quarter".into(),
            site: "synthetic-site".into(),
            quantity: "production_koz".into(),
            unit: "koz".into(),
        },
        metric: PublicMetric::ProductionKoz,
        mean: (i + 1) as f64 * 10.0,
        std_dev: (i + 1) as f64,
        alpha: 0.2,
        selector: OutcomeSelector::FirstPublished,
        outcome: Some((i + 1) as f64 * 20.0),
        observation: i,
    })
}

fn freeze(ledger: &mut ForecastLedger, spec: &Spec, id: ObjectId, name: &str, at: i64) -> Digest32 {
    let rule = if spec.public {
        PublicResolutionPolicy {
            issuer: spec.issuer.clone(),
            metric: spec.metric,
            period: spec.period,
            ownership: spec.ownership,
            members: BTreeMap::from([(id, spec.metadata.site.clone())]),
        }
        .to_resolution_rule_with_vintages(spec.vintages)
        .expect("public rule")
    } else {
        "Synthetic observations on the same declared support".into()
    };
    ledger
        .freeze_cohort(CohortSpec {
            name: name.into(),
            frozen_at_ns: at,
            members: vec![id],
            resolution_rule: rule,
            eligible_sources: vec![spec.issuer.clone()],
            policy: AssessmentPolicy {
                alpha: spec.alpha,
                ..AssessmentPolicy::default()
            },
        })
        .expect("cohort")
}

fn citation(spec: &Spec) -> OutcomeCitation {
    OutcomeCitation {
        source: spec.issuer.clone(),
        vintage: if spec.public {
            spec.period.original_release()
        } else {
            "supplied".into()
        },
        locator: format!("invented-{}.xlsx:Minestats1!B4", spec.observation),
        sha256: sha256(format!("synthetic-observation-{}", spec.observation).as_bytes()).0,
    }
}

fn fixture(
    spec: [Spec; 3],
    repeated_forecast: bool,
    bind_target: bool,
) -> (ForecastLedger, Vec<Digest32>, ObjectId, Vec<ObjectId>) {
    let mut ledger = ForecastLedger::new();
    let ends = spec
        .iter()
        .map(|s| s.period.end_ns().expect("period end"))
        .collect::<Vec<_>>();
    let early = ends[0].min(ends[1]) - 1_000;
    let mut ids = Vec::new();
    for (i, s) in spec[..2].iter().enumerate() {
        let id = ledger
            .register_payload_with_metadata(
                &s.model,
                early + i as i64,
                ends[i],
                ForecastPayload::Gaussian {
                    mean: s.mean,
                    std_dev: s.std_dev,
                },
                s.metadata.clone(),
            )
            .expect("training forecast");
        ids.push(if i == 1 && repeated_forecast {
            ids[0]
        } else {
            id
        });
    }
    let cohorts = spec[..2]
        .iter()
        .enumerate()
        .map(|(i, s)| {
            freeze(
                &mut ledger,
                s,
                ids[i],
                &format!("training-{i}"),
                early + 10 + i as i64,
            )
        })
        .collect::<Vec<_>>();
    for (i, s) in spec[..2].iter().enumerate() {
        if i == 1 && repeated_forecast {
            continue;
        }
        if let Some(value) = s.outcome {
            ledger
                .append_outcome(
                    ids[i],
                    ends[i] + 1,
                    ends[i] + 2,
                    OutcomeKind::Published {
                        value,
                        citation: citation(s),
                    },
                )
                .expect("published outcome");
        }
    }
    let reports = spec[..2]
        .iter()
        .enumerate()
        .map(|(i, s)| {
            ledger
                .assess(cohorts[i], ends[i] + 10, s.selector)
                .expect("saved assessment")
                .digest
        })
        .collect::<Vec<_>>();
    let target = ledger
        .register_payload_with_metadata(
            &spec[2].model,
            ends[0].max(ends[1]) + 20,
            ends[2],
            ForecastPayload::Gaussian {
                mean: spec[2].mean,
                std_dev: spec[2].std_dev,
            },
            spec[2].metadata.clone(),
        )
        .expect("later raw target");
    if bind_target {
        freeze(
            &mut ledger,
            &spec[2],
            target,
            "target",
            ends[0].max(ends[1]) + 21,
        );
    }
    (ledger, reports, target, ids)
}

fn close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() < 1e-12 * expected.abs().max(1.0),
        "{actual} != {expected}"
    );
}

#[test]
fn two_public_quarters_fit_one_later_target_and_order_is_canonical() {
    // Q2 and Q3 have different durations; both are quarterly support.
    let (ledger, reports, target, ids) = fixture(specs(), false, true);
    let raw = ledger.forecast(target).expect("raw").clone();
    assert!(
        ledger.correction_proposal(reports[0], target).is_err(),
        "one public site supplies only one observation per cohort"
    );
    let result = ledger
        .correction_series_proposal(&reports, target)
        .expect("series");
    assert_eq!(
        result.digest.to_hex(),
        "98dc0a0df5f1a89c6e217b61f94979cc862b8f7aa8608e46b94bf3443aefca4f",
        "FACT: captured from the unchanged log implementation before additive support"
    );
    assert_eq!(result.method, CorrectionMethod::MultiplicativeLog);
    assert_eq!(
        result,
        ledger
            .correction_series_proposal_with_method(
                &reports,
                target,
                CorrectionMethod::MultiplicativeLog,
            )
            .expect("explicit legacy method")
    );
    assert_eq!(
        result,
        ledger
            .correction_series_proposal(&[reports[1], reports[0]], target)
            .expect("permuted series")
    );
    close(result.adjustment, 2.0);
    close(result.mean_error, std::f64::consts::LN_2);
    close(result.sample_error_std_dev, 0.0);
    assert_eq!(result.inflation, 1.0);
    let ForecastPayload::Gaussian { mean, std_dev } = result.corrected else {
        panic!("Gaussian");
    };
    close(mean, 60.0);
    close(std_dev, 6.0);
    assert_eq!(
        (
            result.required_training_hits,
            result.raw_training_hits,
            result.corrected_training_hits
        ),
        (2, 0, 2)
    );
    assert_eq!(result.parents.len(), 2);
    assert_eq!(result.target_cohorts.len(), 1);
    assert_eq!(
        result
            .members
            .iter()
            .map(|m| m.raw.forecast_id)
            .collect::<Vec<_>>(),
        ids
    );
    for (parent, member) in result.parents.iter().zip(&result.members) {
        assert_eq!(parent.cohort.spec.members, [member.raw.forecast_id]);
        assert_eq!(ledger.cohort(parent.cohort.digest), Some(&parent.cohort));
    }
    assert_eq!(ledger.forecast(target), Some(&raw));
    assert!(crate::ledger::correction::SERIES_NO_CLAIM.contains("independent observations"));
}

#[test]
fn series_reuses_the_same_inverse_empirical_inflation_known_answer() {
    let mut spec = specs();
    spec[0].outcome = Some(5.0);
    spec[1].outcome = Some(40.0);
    let (ledger, reports, target, _) = fixture(spec, false, true);
    let result = ledger
        .correction_series_proposal(&reports, target)
        .expect("series");
    // Ratios 1/2 and 2: mu=0, sample log SD=sqrt(2)*ln(2), max residual/(raw sigma)=10.
    close(result.adjustment, 1.0);
    close(
        result.sample_error_std_dev,
        2.0_f64.sqrt() * std::f64::consts::LN_2,
    );
    close(result.inflation, 10.0 / 1.281_551_565_544_600_4);
    close(result.corrected_interval.0, 0.0);
    close(result.corrected_interval.1, 60.0);
}

#[test]
fn saved_series_does_not_reselect_a_later_restatement_or_count_it_twice() {
    let (mut ledger, reports, target, ids) = fixture(specs(), false, true);
    let before = ledger
        .correction_series_proposal(&reports, target)
        .expect("initial series");
    let registered = before.raw.registered_ns;
    let predecessor = ledger.outcomes(ids[0])[0].seq;
    let mut revised_source = citation(&specs()[0]);
    revised_source.vintage = "2024Q3".into();
    ledger
        .append_outcome(
            ids[0],
            registered + 30,
            registered + 31,
            OutcomeKind::Restated {
                value: 200.0,
                citation: revised_source,
                supersedes: predecessor,
            },
        )
        .expect("later revision");
    assert_eq!(
        before,
        ledger
            .correction_series_proposal(&reports, target)
            .expect("same saved series")
    );
    let revised = ledger
        .assess(
            before.parents[0].cohort.digest,
            registered + 40,
            OutcomeSelector::LatestAsOf,
        )
        .expect("revised report")
        .digest;
    assert!(
        ledger
            .correction_series_proposal(&[reports[0], revised], target)
            .is_err(),
        "repeated cohort cannot inflate training"
    );
    assert!(
        ledger
            .correction_series_proposal(&[reports[1], revised], target)
            .is_err(),
        "later cut and source cannot train an earlier target"
    );
}

#[test]
fn duplicate_reports_forecasts_and_cited_observations_never_expand_the_population() {
    let (ledger, reports, target, _) = fixture(specs(), false, true);
    for method in [
        CorrectionMethod::MultiplicativeLog,
        CorrectionMethod::AdditiveResidual,
    ] {
        for digests in [
            vec![reports[0], reports[0]],
            vec![reports[0]],
            vec![reports[0], Digest32([7; 32])],
        ] {
            assert!(
                ledger
                    .correction_series_proposal_with_method(&digests, target, method)
                    .is_err()
            );
        }
    }
    let mut supplied = specs();
    supplied.iter_mut().for_each(|s| s.public = false);
    let (ledger, reports, target, _) = fixture(supplied.clone(), true, true);
    for method in [
        CorrectionMethod::MultiplicativeLog,
        CorrectionMethod::AdditiveResidual,
    ] {
        let error = ledger
            .correction_series_proposal_with_method(&reports, target, method)
            .expect_err("same forecast in two cohorts");
        assert!(error.what.contains("repeats a training forecast"));
    }
    supplied[1].observation = supplied[0].observation;
    let (ledger, reports, target, _) = fixture(supplied, false, true);
    for method in [
        CorrectionMethod::MultiplicativeLog,
        CorrectionMethod::AdditiveResidual,
    ] {
        let error = ledger
            .correction_series_proposal_with_method(&reports, target, method)
            .expect_err("same cited observation");
        assert!(error.what.contains("same cited observation"));
    }
}

#[test]
fn incomplete_incompatible_and_overlapping_training_series_refuse() {
    for case in 0..11 {
        let mut spec = specs();
        match case {
            0 => spec[1].outcome = None,
            1 => spec[1].issuer = "another-issuer".into(),
            2 => spec[1].ownership = PublicOwnership::Attributable { num: 3, den: 5 },
            3 => spec[1].vintages = PublicVintagePolicy::OriginalOnly,
            4 => spec[1].period = PublicPeriod::FirstHalf(2024),
            5 => spec[1].period = spec[0].period,
            6 => spec[1].public = false,
            7 => spec[1].alpha = 0.1,
            8 => spec[1].selector = OutcomeSelector::LatestAsOf,
            9 => spec[1].model = "different-model".into(),
            _ => {
                spec[1].metric = PublicMetric::TccUsdPerOz;
                spec[1].metadata.quantity = "tcc_usd_per_oz".into();
                spec[1].metadata.unit = "usd_per_oz".into();
            }
        }
        let (ledger, reports, target, _) = fixture(spec, false, true);
        for method in [
            CorrectionMethod::MultiplicativeLog,
            CorrectionMethod::AdditiveResidual,
        ] {
            assert!(
                ledger
                    .correction_series_proposal_with_method(&reports, target, method)
                    .is_err(),
                "case {case}"
            );
        }
    }
    for field in 0..7 {
        let mut spec = specs();
        if field >= 5 {
            spec.iter_mut().for_each(|s| s.public = false);
        }
        match field {
            0 => spec[1].metadata.family.push_str("-changed"),
            1 => spec[1].metadata.author_role.push_str("-changed"),
            2 => spec[1].metadata.domain.push_str("-changed"),
            3 => spec[1].metadata.horizon.push_str("-changed"),
            4 => spec[1].metadata.site.push_str("-changed"),
            5 => spec[1].metadata.quantity.push_str("-changed"),
            _ => spec[1].metadata.unit.push_str("-changed"),
        }
        let (ledger, reports, target, _) = fixture(spec, false, true);
        for method in [
            CorrectionMethod::MultiplicativeLog,
            CorrectionMethod::AdditiveResidual,
        ] {
            assert!(
                ledger
                    .correction_series_proposal_with_method(&reports, target, method)
                    .is_err(),
                "metadata field {field}"
            );
        }
    }
}

#[test]
fn public_target_needs_a_compatible_frozen_definition_and_no_outcome() {
    let (ledger, reports, target, _) = fixture(specs(), false, false);
    for method in [
        CorrectionMethod::MultiplicativeLog,
        CorrectionMethod::AdditiveResidual,
    ] {
        assert!(
            ledger
                .correction_series_proposal_with_method(&reports, target, method)
                .is_err()
        );
    }
    for case in 0..5 {
        let mut spec = specs();
        match case {
            0 => spec[2].ownership = PublicOwnership::Attributable { num: 3, den: 5 },
            1 => spec[2].vintages = PublicVintagePolicy::OriginalOnly,
            2 => spec[2].period = PublicPeriod::FirstHalf(2025),
            3 => spec[2].public = false,
            _ => spec[2].alpha = 0.1,
        }
        let (ledger, reports, target, _) = fixture(spec, false, true);
        for method in [
            CorrectionMethod::MultiplicativeLog,
            CorrectionMethod::AdditiveResidual,
        ] {
            assert!(
                ledger
                    .correction_series_proposal_with_method(&reports, target, method)
                    .is_err(),
                "target case {case}"
            );
        }
    }
    let (mut ledger, reports, target, _) = fixture(specs(), false, true);
    let due = ledger.forecast(target).expect("target").freeze_time_ns;
    ledger
        .append_outcome(
            target,
            due + 1,
            due + 2,
            OutcomeKind::Published {
                value: 60.0,
                citation: citation(&specs()[2]),
            },
        )
        .expect("target outcome");
    for method in [
        CorrectionMethod::MultiplicativeLog,
        CorrectionMethod::AdditiveResidual,
    ] {
        assert!(
            ledger
                .correction_series_proposal_with_method(&reports, target, method)
                .is_err()
        );
    }
}

#[test]
fn additive_cost_series_preserves_signed_sources_public_definitions_and_canonical_order() {
    for metric in [
        PublicMetric::AiscUsdPerOz,
        PublicMetric::C1UsdPerLb,
        PublicMetric::AiscUsdPerLb,
    ] {
        let mut spec = specs();
        for (i, row) in spec.iter_mut().enumerate() {
            row.metric = metric;
            row.metadata.quantity = metric.as_str().into();
            row.metadata.unit = metric.unit().into();
            row.metadata.family = "quarterly-signed-cost".into();
            row.mean = [-2.0, 0.0, -1.0][i];
            row.std_dev = 1.0;
            row.outcome = Some([0.0, 2.0, 1.0][i]);
        }
        let (mut ledger, reports, target, ids) = fixture(spec.clone(), false, true);
        assert!(ledger.correction_series_proposal(&reports, target).is_err());
        let raw = ledger.forecast(target).expect("raw").clone();
        let result = ledger
            .correction_series_proposal_with_method(
                &reports,
                target,
                CorrectionMethod::AdditiveResidual,
            )
            .expect("signed public cost series");
        assert_eq!(result.method, CorrectionMethod::AdditiveResidual);
        close(result.mean_error, 2.0);
        close(result.adjustment, 2.0);
        close(result.sample_error_std_dev, 0.0);
        assert_eq!(result.inflation, 1.0);
        assert_eq!(
            result.corrected,
            ForecastPayload::Gaussian {
                mean: 1.0,
                std_dev: 1.0,
            }
        );
        assert_eq!(result.metadata.unit, metric.unit());
        assert_eq!(
            (
                result.required_training_hits,
                result.raw_training_hits,
                result.corrected_training_hits,
            ),
            (2, 0, 2)
        );
        assert_eq!(
            result,
            ledger
                .correction_series_proposal_with_method(
                    &[reports[1], reports[0]],
                    target,
                    CorrectionMethod::AdditiveResidual,
                )
                .expect("canonical additive series")
        );
        assert_eq!(result.parents.len(), 2);
        assert_eq!(result.members.len(), 2);
        assert_eq!(result.target_cohorts.len(), 1);
        for ((parent, member), id) in result.parents.iter().zip(&result.members).zip(&ids) {
            assert_eq!(parent.cohort.spec.members, [*id]);
            assert_eq!(member.raw.forecast_id, *id);
            close(member.error, 2.0);
            let (policy, _) = PublicResolutionPolicy::parse_resolution_rule_with_vintages(
                &parent.cohort.spec.resolution_rule,
            )
            .expect("parse retained definition")
            .expect("public definition");
            assert_eq!(policy.metric, metric);
            assert_eq!(policy.ownership, PublicOwnership::Full);
        }
        assert_eq!(ledger.forecast(target), Some(&raw));
        let registered = result.raw.registered_ns;
        let predecessor = ledger.outcomes(ids[0])[0].seq;
        let mut revised_source = citation(&spec[0]);
        revised_source.vintage = spec[1].period.original_release();
        ledger
            .append_outcome(
                ids[0],
                registered + 1,
                registered + 2,
                OutcomeKind::Restated {
                    value: -20.0,
                    citation: revised_source,
                    supersedes: predecessor,
                },
            )
            .expect("later signed restatement");
        assert_eq!(
            result,
            ledger
                .correction_series_proposal_with_method(
                    &reports,
                    target,
                    CorrectionMethod::AdditiveResidual,
                )
                .expect("immutable additive source selection")
        );
        assert!(
            result
                .method
                .series_no_claim()
                .contains("independent observations")
        );
    }
}
