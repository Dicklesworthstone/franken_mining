//! Public-API acceptance for saved forecast diagnostics: independent answers, complete populations,
//! immutable selections and durable-format compatibility. All forecast/source fixtures are synthetic.

#![cfg(test)]

use fmine_calib::ledger::diagnostics::{
    AssessmentDiagnostics, BIN_RULE, DiagnosticMember, DiagnosticValue, EmpiricalRankCounts,
    PIT_METHOD, SCOPE, coverage,
};
use fmine_calib::{
    AssessmentPolicy, CohortSpec, ForecastLedger, ForecastPayload, MemberOutcome, MissingReason,
    OutcomeCitation, OutcomeKind, OutcomeSelector,
};
use fmine_core::codec::Encoder;
use fmine_core::hash::{Digest32, blake3_hash};
use fmine_core::id::ObjectId;

fn build_ledger(payloads: Vec<ForecastPayload>) -> (ForecastLedger, Vec<ObjectId>, Digest32) {
    build_ledger_at(payloads, 0.2)
}

fn build_ledger_at(
    payloads: Vec<ForecastPayload>,
    alpha: f64,
) -> (ForecastLedger, Vec<ObjectId>, Digest32) {
    let mut ledger = ForecastLedger::new();
    let ids = payloads
        .into_iter()
        .enumerate()
        .map(|(i, payload)| {
            // Publications at 100 must be strictly after the registered resolution deadline.
            ledger
                .register_payload(format!("synthetic-{i}"), 1, 99, payload)
                .unwrap()
        })
        .collect::<Vec<_>>();
    let cohort = ledger
        .freeze_cohort(CohortSpec {
            name: "diagnostic cohort".into(),
            frozen_at_ns: 2,
            members: ids.clone(),
            resolution_rule: "synthetic fixed population".into(),
            eligible_sources: vec!["synthetic".into()],
            policy: AssessmentPolicy {
                alpha,
                ..AssessmentPolicy::default()
            },
        })
        .unwrap();
    (ledger, ids, cohort)
}

fn citation(vintage: &str) -> OutcomeCitation {
    OutcomeCitation {
        source: "synthetic".into(),
        vintage: vintage.into(),
        locator: "synthetic:whole".into(),
        sha256: fmine_core::hash::sha256(vintage.as_bytes()).0,
    }
}

fn publish(ledger: &mut ForecastLedger, id: ObjectId, value: f64) {
    ledger
        .append_outcome(
            id,
            100,
            100,
            OutcomeKind::Published {
                value,
                citation: citation("original"),
            },
        )
        .unwrap();
}

fn gaussian() -> ForecastPayload {
    ForecastPayload::Gaussian {
        mean: 10.0,
        std_dev: 2.0,
    }
}

fn diagnostics(
    ledger: &mut ForecastLedger,
    cohort: Digest32,
    as_of_ns: i64,
    selector: OutcomeSelector,
) -> AssessmentDiagnostics {
    let parent = ledger.assess(cohort, as_of_ns, selector).unwrap().digest;
    ledger.assessment_diagnostics(parent).unwrap()
}

fn member(report: &AssessmentDiagnostics, id: ObjectId) -> &DiagnosticMember {
    report
        .members
        .iter()
        .find(|item| item.member.forecast_id == id)
        .unwrap()
}

#[test]
fn non_empirical_diagnostics_match_the_prior_v1_wire_layout() {
    const EDGES: [f64; 11] = [0.0, 0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 1.0];
    for (payload, probability) in [
        (gaussian(), None),
        (
            ForecastPayload::Bernoulli { probability: 0.25 },
            Some(0.25_f64),
        ),
    ] {
        let (mut ledger, ids, cohort) = build_ledger(vec![payload]);
        let parent = ledger
            .assess(cohort, 3, OutcomeSelector::FirstPublished)
            .unwrap()
            .digest;
        let report = ledger.assessment_diagnostics(parent).unwrap();
        assert!(report.empirical_rank.is_none());
        // FACT: the pre-extension layout at 24848416. This single unresolved fixture spells out
        // its prior wire values independently, including absent empirical extension markers.
        let mut expected = Encoder::new_frame(*b"DGN1", 1);
        for rule in [SCOPE, BIN_RULE, PIT_METHOD] {
            expected.put_str(rule);
        }
        expected.put_fixed(&parent.0);
        expected.put_fixed(&cohort.0);
        expected.put_i64(3);
        expected.put_u8(0); // first-published
        expected.put_u64(1); // one member
        expected.put_fixed(&ids[0].bytes);
        expected.put_u8(0); // no selected event
        expected.put_u8(1); // unresolved
        expected.put_u8(0); // pending
        if let Some(probability) = probability {
            expected.put_u8(1); // event diagnostic
            expected.put_u64(probability.to_bits());
            expected.put_u8(0); // no Gaussian histogram
            expected.put_u8(1); // reliability diagram
            for count in [1, 0, 1] {
                expected.put_u64(count);
            }
            for (index, edges) in EDGES.windows(2).enumerate() {
                for edge in edges {
                    expected.put_u64(edge.to_bits());
                }
                let populated = index == 2;
                expected.put_u64(u64::from(populated));
                expected.put_u64(0); // resolved
                expected.put_u64(0); // events
                expected.put_u8(u8::from(populated));
                if populated {
                    expected.put_u64(0.25_f64.to_bits());
                }
                expected.put_u8(0); // no resolved mean
                expected.put_u8(0); // no observed frequency
                expected.put_u8(u8::from(populated));
                if populated {
                    expected.put_u64(0.0_f64.to_bits());
                    expected.put_u64(1.0_f64.to_bits());
                }
            }
        } else {
            expected.put_u8(0); // Gaussian diagnostic
            expected.put_u8(0); // no selected PIT
            expected.put_u8(1); // Gaussian histogram
            for count in [1, 0, 1] {
                expected.put_u64(count);
            }
            for edges in EDGES.windows(2) {
                for edge in edges {
                    expected.put_u64(edge.to_bits());
                }
                expected.put_u64(0); // resolved count
                expected.put_u64(0.0_f64.to_bits());
                expected.put_u64(1.0_f64.to_bits());
            }
            expected.put_u8(0); // no reliability diagram
        }
        assert_eq!(report.canonical_bytes(), expected.finish());
        assert_eq!(report.digest, blake3_hash(&report.canonical_bytes()));
    }
}

#[test]
fn empirical_ties_and_missing_members_have_exact_rank_counts_and_completion_bounds() {
    let payload = ForecastPayload::Empirical {
        samples: vec![-2.0, 0.0, 0.0, 4.0],
    };
    let (mut ledger, ids, cohort) = build_ledger(vec![payload; 8]);
    for (id, outcome) in ids.iter().zip([-3.0, -2.0, -1.0, 0.0, 4.0, 5.0]) {
        publish(&mut ledger, *id, outcome);
    }
    ledger
        .append_outcome(
            ids[6],
            100,
            100,
            OutcomeKind::Missing {
                reason: MissingReason::NotPublished,
            },
        )
        .unwrap();
    let parent = ledger
        .assess(cohort, 101, OutcomeSelector::FirstPublished)
        .unwrap()
        .clone();
    let parent_bytes = parent.canonical_bytes();
    let report = ledger.assessment_diagnostics(parent.digest).unwrap();
    let ranks = report.empirical_rank.as_ref().unwrap();
    assert_eq!(
        (ranks.n_members, ranks.n_resolved, ranks.n_unresolved),
        (8, 6, 2)
    );
    assert_eq!(
        ranks.bins.map(|bin| bin.definite_count),
        [1, 0, 1, 0, 0, 0, 0, 0, 0, 1]
    );
    assert_eq!(
        ranks.bins.map(|bin| bin.possible_count),
        [2, 1, 3, 1, 1, 1, 1, 2, 1, 2]
    );
    // The [2/10,3/10) bin has full-population bounds [1/8,5/8], not [1/6,3/6].
    assert_eq!(
        (
            ranks.bins[2].definite_count,
            ranks.bins[2].possible_count + ranks.n_unresolved
        ),
        (1, 5)
    );
    for (index, (below, equal, above)) in [
        (0, 0, 4),
        (0, 1, 3),
        (1, 0, 3),
        (1, 2, 1),
        (3, 1, 0),
        (4, 0, 0),
    ]
    .into_iter()
    .enumerate()
    {
        let item = member(&report, ids[index]);
        assert_eq!(
            item.value,
            DiagnosticValue::EmpiricalRank {
                n_samples: 4,
                counts: Some(EmpiricalRankCounts {
                    below,
                    equal,
                    above
                })
            }
        );
    }
    for (index, selected_event) in [(6, true), (7, false)] {
        let item = member(&report, ids[index]);
        assert_eq!(
            item.value,
            DiagnosticValue::EmpiricalRank {
                n_samples: 4,
                counts: None
            }
        );
        assert_eq!(item.member.selected_event.is_some(), selected_event);
    }
    assert_eq!(
        report
            .members
            .iter()
            .map(|item| &item.member)
            .collect::<Vec<_>>(),
        parent.per_member.iter().collect::<Vec<_>>()
    );
    assert!(report.gaussian_pit.is_none());
    assert!(report.bernoulli_reliability.is_none());
    assert_eq!(&report.canonical_bytes()[8..10], &2_u16.to_le_bytes());
    assert_eq!(report.digest, blake3_hash(&report.canonical_bytes()));
    assert_eq!(
        ledger
            .assess(cohort, 101, OutcomeSelector::FirstPublished)
            .unwrap()
            .canonical_bytes(),
        parent_bytes
    );
}

#[test]
fn degenerate_empirical_forecasts_and_gaussian_tail_projections_remain_explicit() {
    for samples in [
        vec![7.0],
        vec![7.0; fmine_calib::scoring::MAX_EMPIRICAL_SAMPLES],
    ] {
        let n_samples = samples.len();
        let (mut ledger, ids, cohort) = build_ledger(vec![ForecastPayload::Empirical { samples }]);
        publish(&mut ledger, ids[0], 7.0);
        let report = diagnostics(&mut ledger, cohort, 101, OutcomeSelector::FirstPublished);
        assert_eq!(
            report.members[0].value,
            DiagnosticValue::EmpiricalRank {
                n_samples,
                counts: Some(EmpiricalRankCounts {
                    below: 0,
                    equal: n_samples,
                    above: 0
                })
            }
        );
        let ranks = report.empirical_rank.unwrap();
        assert!(
            ranks
                .bins
                .iter()
                .all(|bin| bin.definite_count == 0 && bin.possible_count == 1)
        );
    }
    let (mut ledger, ids, cohort) = build_ledger(vec![gaussian(); 2]);
    publish(&mut ledger, ids[0], -90.0);
    publish(&mut ledger, ids[1], 110.0);
    let report = diagnostics(&mut ledger, cohort, 101, OutcomeSelector::FirstPublished);
    let pit = report.gaussian_pit.unwrap();
    assert_eq!(pit.bins[0].count, 1);
    assert_eq!(pit.bins[9].count, 1);
}

#[test]
fn empirical_sample_permutations_preserve_ranks_but_keep_distinct_frozen_identities() {
    let reports = [vec![-2.0, 0.0, 0.0, 4.0], vec![0.0, 4.0, -2.0, 0.0]].map(|samples| {
        let (mut ledger, ids, cohort) = build_ledger(vec![ForecastPayload::Empirical { samples }]);
        publish(&mut ledger, ids[0], 0.0);
        diagnostics(&mut ledger, cohort, 101, OutcomeSelector::FirstPublished)
    });
    assert_eq!(reports[0].empirical_rank, reports[1].empirical_rank);
    assert_eq!(reports[0].members[0].value, reports[1].members[0].value);
    assert_ne!(
        reports[0].members[0].member.forecast_id,
        reports[1].members[0].member.forecast_id
    );
    assert_ne!(reports[0].assessment_digest, reports[1].assessment_digest);
    assert_ne!(reports[0].digest, reports[1].digest);
}

#[test]
fn empirical_restatements_preserve_saved_counts_sources_and_parent_bytes() {
    let payload = ForecastPayload::Empirical {
        samples: vec![-2.0, 0.0, 0.0, 4.0],
    };
    let (mut ledger, ids, cohort) = build_ledger(vec![payload; 2]);
    publish(&mut ledger, ids[0], 0.0);
    let old = ledger
        .assess(cohort, 101, OutcomeSelector::LatestAsOf)
        .unwrap()
        .clone();
    let old_bytes = old.canonical_bytes();
    let saved = ledger.assessment_diagnostics(old.digest).unwrap();
    ledger
        .append_outcome(
            ids[0],
            102,
            102,
            OutcomeKind::Restated {
                value: 4.0,
                citation: citation("revision"),
                supersedes: 1,
            },
        )
        .unwrap();
    ledger
        .append_outcome(
            ids[1],
            102,
            102,
            OutcomeKind::Published {
                value: 1.0,
                citation: citation("original"),
            },
        )
        .unwrap();
    let changed = diagnostics(&mut ledger, cohort, 103, OutcomeSelector::LatestAsOf);
    assert_ne!(changed.digest, saved.digest);
    assert_eq!(changed.empirical_rank.as_ref().unwrap().n_unresolved, 0);
    let original = member(&saved, ids[0]);
    let revised = member(&changed, ids[0]);
    assert!(
        matches!(&original.member.outcome, MemberOutcome::Resolved { citation, .. } if citation.vintage == "original")
    );
    assert!(
        matches!(&revised.member.outcome, MemberOutcome::Resolved { citation, .. } if citation.vintage == "revision")
    );
    assert_eq!(ledger.assessment_diagnostics(old.digest).unwrap(), saved);
    assert_eq!(
        ledger
            .assess(cohort, 101, OutcomeSelector::LatestAsOf)
            .unwrap()
            .canonical_bytes(),
        old_bytes
    );
    let first = diagnostics(&mut ledger, cohort, 103, OutcomeSelector::FirstPublished);
    let item = member(&first, ids[0]);
    assert_eq!(item.value, original.value);
    assert_eq!(item.member.selected_event, original.member.selected_event);
}

#[test]
fn gaussian_pit_matches_known_normal_values_and_retains_missing_members() {
    let (mut ledger, ids, cohort) = build_ledger(vec![gaussian(); 4]);
    for (id, value) in ids.iter().zip([8.0, 10.0, 12.0]) {
        publish(&mut ledger, *id, value);
    }
    ledger
        .append_outcome(
            ids[3],
            100,
            100,
            OutcomeKind::Missing {
                reason: MissingReason::NotPublished,
            },
        )
        .unwrap();
    let parent = ledger
        .assess(cohort, 101, OutcomeSelector::FirstPublished)
        .unwrap()
        .clone();
    let bytes = parent.canonical_bytes();
    let report = ledger.assessment_diagnostics(parent.digest).unwrap();
    let pit = report.gaussian_pit.as_ref().unwrap();
    assert_eq!((pit.n_members, pit.n_resolved, pit.n_unresolved), (4, 3, 1));
    assert_eq!(
        pit.bins.map(|bin| bin.count),
        [0, 1, 0, 0, 0, 1, 0, 0, 1, 0]
    );
    assert_eq!(pit.bins[5].whole_population_fraction, (0.25, 0.5));
    assert_eq!(pit.bins[0].whole_population_fraction, (0.0, 0.25));
    for (id, expected) in ids
        .iter()
        .zip([0.158_655_253_931_457_07, 0.5, 0.841_344_746_068_542_9])
    {
        let item = member(&report, *id);
        let DiagnosticValue::GaussianPit(Some(actual)) = item.value else {
            panic!("resolved Gaussian");
        };
        assert!((actual - expected).abs() < 7.5e-8);
    }
    let pending = member(&report, ids[3]);
    assert_eq!(pending.value, DiagnosticValue::GaussianPit(None));
    assert!(pending.member.selected_event.is_some());
    assert!(report.bernoulli_reliability.is_none());
    assert_eq!(report.digest, blake3_hash(&report.canonical_bytes()));
    assert_eq!(
        ledger
            .assess(cohort, 101, OutcomeSelector::FirstPublished)
            .unwrap()
            .canonical_bytes(),
        bytes
    );
}

#[test]
fn reliability_keeps_resolved_and_full_populations_distinct_and_empty_bins_absent() {
    let (mut ledger, ids, cohort) = build_ledger(
        [0.1, 0.15, 0.19, 0.0, 1.0]
            .map(|probability| ForecastPayload::Bernoulli { probability })
            .to_vec(),
    );
    for (index, value) in [(0, 1.0), (1, 0.0), (3, 0.0), (4, 1.0)] {
        publish(&mut ledger, ids[index], value);
    }
    let report = diagnostics(&mut ledger, cohort, 101, OutcomeSelector::FirstPublished);
    let diagram = report.bernoulli_reliability.unwrap();
    assert_eq!(
        (diagram.n_members, diagram.n_resolved, diagram.n_unresolved),
        (5, 4, 1)
    );
    let bin = diagram.bins[1];
    assert_eq!((bin.n_members, bin.n_resolved, bin.n_events), (3, 2, 1));
    assert!((bin.mean_probability.unwrap() - 0.44 / 3.0).abs() < 1e-15);
    assert_eq!(bin.mean_resolved_probability, Some(0.125));
    assert_eq!(bin.observed_frequency, Some(0.5));
    assert_eq!(bin.whole_bin_frequency, Some((1.0 / 3.0, 2.0 / 3.0)));
    assert_eq!(diagram.bins[0].whole_bin_frequency, Some((0.0, 0.0)));
    assert_eq!(diagram.bins[9].whole_bin_frequency, Some((1.0, 1.0)));
    let empty = diagram.bins[2];
    assert_eq!(
        (
            empty.mean_probability,
            empty.mean_resolved_probability,
            empty.observed_frequency,
            empty.whole_bin_frequency
        ),
        (None, None, None, None)
    );
}

#[test]
fn newer_outcomes_and_restatements_cannot_rewrite_saved_diagnostics() {
    let (mut ledger, ids, cohort) = build_ledger(vec![gaussian(); 2]);
    publish(&mut ledger, ids[0], 10.0);
    let original = diagnostics(&mut ledger, cohort, 101, OutcomeSelector::LatestAsOf);
    let old = original.assessment_digest;
    ledger
        .append_outcome(
            ids[0],
            102,
            102,
            OutcomeKind::Restated {
                value: 8.0,
                citation: citation("revision"),
                supersedes: 1,
            },
        )
        .unwrap();
    ledger
        .append_outcome(
            ids[1],
            102,
            102,
            OutcomeKind::Published {
                value: 12.0,
                citation: citation("original"),
            },
        )
        .unwrap();
    let latest = diagnostics(&mut ledger, cohort, 103, OutcomeSelector::LatestAsOf);
    assert_ne!(latest.digest, original.digest);
    assert_eq!(ledger.assessment_diagnostics(old).unwrap(), original);
    assert_eq!(latest.gaussian_pit.unwrap().n_unresolved, 0);
    assert!(ledger.assessment_diagnostics(Digest32::default()).is_err());
    // Projection is read-only and cannot close a later authority cut.
    ledger
        .register_payload("later", 104, 200, gaussian())
        .unwrap();
}

#[test]
fn mixed_continuous_payloads_keep_empirical_ranks_separate_and_sparse_quantiles_unsupported() {
    let (mut ledger, ids, cohort) = build_ledger(vec![
        gaussian(),
        ForecastPayload::Empirical {
            samples: vec![8.0, 10.0, 12.0],
        },
    ]);
    publish(&mut ledger, ids[0], 10.0);
    publish(&mut ledger, ids[1], 10.0);
    let report = diagnostics(&mut ledger, cohort, 101, OutcomeSelector::FirstPublished);
    assert_eq!(report.members.len(), 2);
    assert_eq!(report.gaussian_pit.as_ref().unwrap().n_members, 1);
    assert_eq!(
        member(&report, ids[1]).value,
        DiagnosticValue::EmpiricalRank {
            n_samples: 3,
            counts: Some(EmpiricalRankCounts {
                below: 1,
                equal: 1,
                above: 1
            })
        }
    );
    assert_eq!(report.empirical_rank.as_ref().unwrap().n_members, 1);
    assert!(report.bernoulli_reliability.is_none());
    let (mut ledger, _, cohort) = build_ledger(vec![ForecastPayload::Quantiles {
        levels: vec![0.1, 0.9],
        values: vec![0.0, 1.0],
    }]);
    let report = diagnostics(&mut ledger, cohort, 3, OutcomeSelector::FirstPublished);
    assert!(report.gaussian_pit.is_none());
    assert!(report.bernoulli_reliability.is_none());
    assert_eq!(
        report.members[0].value,
        DiagnosticValue::Unsupported("quantiles")
    );
}

#[test]
fn multilevel_coverage_uses_known_gaussian_quantiles_and_keeps_the_frozen_denominator() {
    let (mut ledger, ids, cohort) = build_ledger(vec![gaussian(); 5]);
    for (id, y) in ids.iter().zip([10.0, 11.5, 13.0, 6.0]) {
        publish(&mut ledger, *id, y);
    }
    let parent = ledger
        .assess(cohort, 101, OutcomeSelector::LatestAsOf)
        .unwrap()
        .clone();
    let prior = ledger.assessment_diagnostics(parent.digest).unwrap();
    let report = ledger.coverage_diagnostics(parent.digest).unwrap();
    // Standard normal inverse probabilities .75, .90 and .975; these expected values are
    // independent constants, not generated by the candidate quantile implementation.
    for (i, z) in [
        0.674_489_750_196_081_7,
        1.281_551_565_544_600_4,
        1.959_963_984_540_054,
    ]
    .into_iter()
    .enumerate()
    {
        let (lo, hi) = report.members[0].intervals[i].bounds.unwrap();
        assert!((lo - (10.0 - 2.0 * z)).abs() < 1e-7);
        assert!((hi - (10.0 + 2.0 * z)).abs() < 1e-7);
        let s = report.levels[i];
        assert_eq!(
            (
                s.n_supported,
                s.n_unsupported,
                s.n_resolved(),
                s.n_unresolved
            ),
            (5, 0, 4, 1)
        );
        assert_eq!((s.inside, s.below, s.above), (i + 1, 1, 2 - i));
    }
    assert_eq!(
        report.members.iter().map(|m| &m.member).collect::<Vec<_>>(),
        parent.per_member.iter().collect::<Vec<_>>()
    );
    assert!(report.to_json().contains("\"whole_supported_fraction\":{\"lower\":{\"num\":1,\"den\":5},\"upper\":{\"num\":2,\"den\":5}}"));
    assert_eq!(ledger.assessment_diagnostics(parent.digest).unwrap(), prior);
    assert_eq!(
        ledger
            .assess(cohort, 101, OutcomeSelector::LatestAsOf)
            .unwrap()
            .canonical_bytes(),
        parent.canonical_bytes()
    );
    ledger
        .append_outcome(
            ids[0],
            102,
            102,
            OutcomeKind::Restated {
                value: 20.0,
                citation: citation("restated"),
                supersedes: 1,
            },
        )
        .unwrap();
    let latest = ledger
        .assess(cohort, 103, OutcomeSelector::LatestAsOf)
        .unwrap()
        .digest;
    assert_ne!(
        ledger.coverage_diagnostics(latest).unwrap().digest,
        report.digest
    );
    assert_eq!(ledger.coverage_diagnostics(parent.digest).unwrap(), report);
    assert_eq!(&report.canonical_bytes()[8..10], &1_u16.to_le_bytes());
    assert_eq!(report.digest, blake3_hash(&report.canonical_bytes()));
    assert!(ledger.coverage_diagnostics(Digest32::default()).is_err());
}

#[test]
fn empirical_coverage_retains_ties_exact_boundaries_and_unresolved_members() {
    let (mut ledger, ids, cohort) = build_ledger(vec![
        ForecastPayload::Empirical {
            samples: vec![-2.0, 0.0, 0.0, 4.0]
        };
        4
    ]);
    for (id, y) in ids.iter().zip([-2.0, 0.0, 4.0]) {
        publish(&mut ledger, *id, y);
    }
    let parent = ledger
        .assess(cohort, 101, OutcomeSelector::FirstPublished)
        .unwrap()
        .digest;
    let report = ledger.coverage_diagnostics(parent).unwrap();
    for member in &report.members {
        assert_eq!(
            member.intervals.map(|v| v.bounds),
            [Some((-2.0, 0.0)), Some((-2.0, 4.0)), Some((-2.0, 4.0))]
        );
    }
    assert_eq!(
        report
            .levels
            .map(|s| (s.inside, s.below, s.above, s.n_unresolved)),
        [(2, 0, 1, 1), (3, 0, 0, 1), (3, 0, 0, 1)]
    );
    assert!(
        report
            .to_markdown()
            .contains("| 50% | 4 / 4 | 3 | 2 | 0 | 1 | 1 | [2/4, 3/4] |")
    );
}

#[test]
fn coverage_levels_require_exact_declarations_and_do_not_invent_sparse_quantiles() {
    use coverage::CoverageStatus::*;
    let cases = [
        (
            ForecastPayload::Interval {
                lo: -1.0,
                hi: 1.0,
                nominal: Some(0.95),
            },
            0.05,
            [MissingLevel, MissingLevel, Inside],
        ),
        (
            ForecastPayload::Interval {
                lo: -1.0,
                hi: 1.0,
                nominal: None,
            },
            0.2,
            [UnstatedLevel; 3],
        ),
        (
            ForecastPayload::Interval {
                lo: -1.0,
                hi: 1.0,
                nominal: Some(f64::from_bits(0.8_f64.to_bits() + 1)),
            },
            0.2,
            [MissingLevel; 3],
        ),
        (
            ForecastPayload::Quantiles {
                levels: vec![0.1, 0.9],
                values: vec![-1.0, 1.0],
            },
            0.2,
            [MissingLevel, Inside, MissingLevel],
        ),
        (
            ForecastPayload::Quantiles {
                levels: vec![f64::from_bits(0.1_f64.to_bits() + 1), 0.9],
                values: vec![-1.0, 1.0],
            },
            0.2,
            [MissingLevel; 3],
        ),
        (
            ForecastPayload::Quantiles {
                levels: vec![0.025, 0.1, 0.25, 0.75, 0.9, 0.975],
                values: vec![-3.0, -2.0, -1.0, 1.0, 2.0, 3.0],
            },
            0.2,
            [Inside; 3],
        ),
        (
            ForecastPayload::Bernoulli { probability: 0.5 },
            0.2,
            [EventDistribution; 3],
        ),
        (
            ForecastPayload::Gaussian {
                mean: 0.0,
                std_dev: 6e307,
            },
            0.2,
            [Inside, Inside, UnrepresentableInterval],
        ),
    ];
    for (payload, alpha, expected) in cases {
        let (mut ledger, ids, cohort) = build_ledger_at(vec![payload], alpha);
        publish(&mut ledger, ids[0], 0.0);
        let parent = ledger
            .assess(cohort, 101, OutcomeSelector::FirstPublished)
            .unwrap()
            .digest;
        let report = ledger.coverage_diagnostics(parent).unwrap();
        assert_eq!(report.members[0].intervals.map(|v| v.status), expected);
        for (s, status) in report.levels.iter().zip(expected) {
            assert_eq!(s.n_supported, usize::from(status == Inside));
            assert_eq!(s.n_supported + s.n_unsupported, 1);
        }
        if report.levels.iter().all(|s| s.n_supported == 0) {
            assert!(report.to_json().contains("\"observed_fraction\":null,\"whole_supported_fraction\":{\"lower\":null,\"upper\":null}"));
        }
    }
}
