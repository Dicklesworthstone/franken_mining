//! Synthetic known answers for temporal ancestry; no site data or retrospective performance claims.

use super::*;
use fmine_core::id::PrefixKind;

fn id(n: u32) -> ObjectId {
    let mut bytes = [0; 16];
    bytes[12..].copy_from_slice(&n.to_be_bytes());
    ObjectId::from_raw(PrefixKind::Evidence, bytes)
}

fn record(cell: &str, seq: u64, wall: i64) -> RecordTime {
    RecordTime::new(cell, seq, wall, TimeEvidence::HostSynced)
}

fn cut(at: i64) -> ConeCut {
    ConeCut {
        decision_ns: at,
        anchor: Anchor::new([("source", 10), ("acts", 10)]).unwrap(),
    }
}

fn source(n: u32, kind: NodeKind, at: i64) -> TemporalNode {
    TemporalNode {
        id: id(n),
        kind,
        parents: vec![],
        availability: Availability::Source {
            available_at: Some(at),
        },
        recorded_at: Some(record("source", 1, i64::MAX)),
    }
}

fn act(n: u32, kind: NodeKind, parents: &[u32], performed: i64) -> TemporalNode {
    TemporalNode {
        id: id(n),
        kind,
        parents: parents.iter().map(|n| id(*n)).collect(),
        availability: Availability::Derived {
            performed_at: Some(performed),
            declared_available_at: None,
        },
        recorded_at: Some(record("acts", 1, i64::MIN)),
    }
}

fn leak(error: ConeError) -> ConeViolation {
    match error {
        ConeError::Violation(v) => *v,
        other => panic!("expected a temporal frontier, got {other:?}"),
    }
}

#[test]
fn clean_cone_computes_availability_and_retains_exact_cut() {
    let graph = TemporalGraph::new([
        act(0, NodeKind::Forecast, &[1, 2], 25),
        source(1, NodeKind::Observation, 30),
        act(2, NodeKind::FittedParameters, &[3], 20),
        source(3, NodeKind::RegistryRow, 10),
    ])
    .unwrap();
    let requested = cut(30);
    let checked = graph.check(id(0), &requested).unwrap();
    assert_eq!(checked.root(), id(0));
    assert_eq!(checked.cut(), &requested);
    assert_eq!(checked.nodes(), &[id(0), id(1), id(2), id(3)]);
    assert_eq!(checked.available_at(&id(0)), Some(30));
    assert_eq!(checked.available_at(&id(2)), Some(20));
    assert_eq!(checked.available_at(&id(99)), None);
    assert_eq!(
        checked.anchor_marks(),
        &BTreeMap::from([("acts".into(), 10), ("source".into(), 10)])
    );
    assert_eq!(graph.check(id(0), &requested).unwrap(), checked);
}

#[test]
fn known_answer_five_hops_catches_exactly_one_nanosecond_of_lookahead() {
    let nodes = [
        act(0, NodeKind::Forecast, &[1], 80),
        act(1, NodeKind::Claim, &[2], 80),
        act(2, NodeKind::CorrelationCard, &[3], 80),
        act(3, NodeKind::FittedParameters, &[4], 80),
        act(4, NodeKind::AttestedInterpretation, &[5], 80),
        source(5, NodeKind::Observation, 101),
    ];
    let graph = TemporalGraph::new(nodes).unwrap();
    let violation = leak(graph.check(id(0), &cut(100)).unwrap_err());
    assert_eq!(violation.root(), id(0));
    assert_eq!(violation.cut().decision_ns, 100);
    assert_eq!(violation.nodes().len(), 6);
    let [frontier] = violation.frontier() else {
        panic!("one earliest leak")
    };
    assert_eq!(frontier.node, id(5));
    assert_eq!(frontier.kind, NodeKind::Observation);
    assert_eq!(frontier.available_at, 101);
    assert_eq!(frontier.path, (0..=5).map(id).collect::<Vec<_>>());
    assert!(frontier.after_cut && !frontier.after_anchor);
    assert!(graph.check(id(0), &cut(101)).is_ok());
}

#[test]
fn future_variogram_exclusion_and_price_deck_are_distinct_frontier_causes() {
    for kind in [
        NodeKind::FittedParameters,
        NodeKind::ExclusionAct,
        NodeKind::PriceDeck,
    ] {
        let late = if kind == NodeKind::PriceDeck {
            source(2, kind, 101)
        } else {
            act(2, kind, &[3], 101)
        };
        let graph = TemporalGraph::new([
            act(0, NodeKind::Forecast, &[1], 90),
            act(1, NodeKind::Claim, &[2], 90),
            late,
            source(3, NodeKind::Observation, 80),
        ])
        .unwrap();
        let error = graph.check(id(0), &cut(100)).unwrap_err();
        assert_eq!(error.refusal().code, "FMINE-REF-110");
        let violation = leak(error);
        let [v] = violation.frontier() else {
            panic!("one causal act or source")
        };
        assert_eq!((v.node, v.kind), (id(2), kind));
        assert_eq!(v.path, vec![id(0), id(1), id(2)]);
    }
}

#[test]
fn diamond_frontier_uses_shortest_path_with_id_tie_break_and_transitive_edges() {
    let mut nodes = vec![
        act(0, NodeKind::Forecast, &[2, 1], 50),
        act(1, NodeKind::Claim, &[3], 50),
        act(2, NodeKind::Claim, &[3], 50),
        act(3, NodeKind::CorrelationCard, &[4], 200),
        source(4, NodeKind::Observation, 101),
    ];
    let v = leak(
        TemporalGraph::new(nodes.clone())
            .unwrap()
            .check(id(0), &cut(100))
            .unwrap_err(),
    );
    assert_eq!(
        v.frontier().len(),
        1,
        "a late descendant is not an earliest leak"
    );
    assert_eq!(v.frontier()[0].path, vec![id(0), id(1), id(3), id(4)]);
    nodes[0].parents.push(id(4));
    let v = leak(
        TemporalGraph::new(nodes)
            .unwrap()
            .check(id(0), &cut(100))
            .unwrap_err(),
    );
    assert_eq!(v.frontier()[0].path, vec![id(0), id(4)]);
}

#[test]
fn record_visibility_uses_per_cell_sequence_even_when_clocks_run_backwards() {
    let mut node = source(0, NodeKind::Observation, 10);
    node.recorded_at = Some(record("pit", 5, i64::MAX));
    let requested = ConeCut {
        decision_ns: 10,
        anchor: Anchor::new([("pit", 5), ("mill", 9)]).unwrap(),
    };
    assert!(
        TemporalGraph::new([node.clone()])
            .unwrap()
            .check(id(0), &requested)
            .is_ok()
    );
    node.recorded_at = Some(record("mill", 10, i64::MIN));
    let v = leak(
        TemporalGraph::new([node.clone()])
            .unwrap()
            .check(id(0), &requested)
            .unwrap_err(),
    );
    assert!(!v.frontier()[0].after_cut && v.frontier()[0].after_anchor);
    node.recorded_at = Some(record("unsynchronized", 0, 0));
    assert!(
        matches!(TemporalGraph::new([node]).unwrap().check(id(0), &requested), Err(ConeError::MissingAnchorCell { cell, .. }) if cell == "unsynchronized")
    );
}

#[test]
fn missing_times_and_backdated_acts_fail_closed_without_substituted_instants() {
    let base = source(0, NodeKind::Observation, 10);
    for (availability, recorded_at, field) in [
        (
            Availability::Source { available_at: None },
            base.recorded_at.clone(),
            TimeField::AvailableAt,
        ),
        (
            Availability::Derived {
                performed_at: None,
                declared_available_at: Some(10),
            },
            base.recorded_at.clone(),
            TimeField::PerformedAt,
        ),
        (base.availability, None, TimeField::RecordedAt),
    ] {
        let mut node = base.clone();
        node.availability = availability;
        node.recorded_at = recorded_at;
        let error = TemporalGraph::new([node])
            .unwrap()
            .check(id(0), &cut(100))
            .unwrap_err();
        assert!(
            matches!(error, ConeError::MissingTime { field: got, path, .. } if got == field && path == [id(0)])
        );
    }
    let mut fit = act(0, NodeKind::FittedParameters, &[1], 20);
    fit.availability = Availability::Derived {
        performed_at: Some(20),
        declared_available_at: Some(29),
    };
    let observed = source(1, NodeKind::Observation, 30);
    assert!(matches!(
        TemporalGraph::new([fit.clone(), observed.clone()])
            .unwrap()
            .check(id(0), &cut(100)),
        Err(ConeError::BackdatedAvailability {
            declared: 29,
            computed: 30,
            ..
        })
    ));
    fit.availability = Availability::Derived {
        performed_at: Some(20),
        declared_available_at: Some(40),
    };
    let checked = TemporalGraph::new([fit, observed])
        .unwrap()
        .check(id(0), &cut(100))
        .unwrap();
    assert_eq!(
        checked.available_at(&id(0)),
        Some(40),
        "an embargo delays availability"
    );
}

#[test]
fn missing_parents_duplicates_cycles_and_source_parents_refuse() {
    let error = TemporalGraph::new([act(0, NodeKind::Claim, &[1], 0)])
        .unwrap()
        .check(id(0), &cut(0))
        .unwrap_err();
    assert!(
        matches!(error, ConeError::MissingNode { node, path, .. } if node == id(1) && path == [id(0), id(1)])
    );
    let duplicate = source(0, NodeKind::Observation, 0);
    assert!(
        matches!(TemporalGraph::new([duplicate.clone(), duplicate]), Err(ConeError::DuplicateNode { node }) if node == id(0))
    );
    let graph = TemporalGraph::new([
        act(0, NodeKind::Claim, &[1], 0),
        act(1, NodeKind::Claim, &[0], 0),
    ])
    .unwrap();
    assert!(
        matches!(graph.check(id(0), &cut(0)), Err(ConeError::Cycle { cycle, .. }) if cycle == [id(0), id(1), id(0)])
    );
    let mut wrong = source(0, NodeKind::Observation, 0);
    wrong.parents.push(id(1));
    assert!(matches!(
        TemporalGraph::new([wrong, source(1, NodeKind::Observation, 0)])
            .unwrap()
            .check(id(0), &cut(0)),
        Err(ConeError::SourceHasParents { .. })
    ));
}

#[test]
fn input_order_unrelated_bad_nodes_and_unrelated_anchor_marks_do_not_change_a_cone() {
    let mut nodes = vec![
        act(0, NodeKind::Claim, &[2, 1], 3),
        source(1, NodeKind::Observation, 1),
        source(2, NodeKind::PriceDeck, 2),
    ];
    let expected = TemporalGraph::new(nodes.clone())
        .unwrap()
        .check(id(0), &cut(10))
        .unwrap();
    nodes.reverse();
    for node in &mut nodes {
        node.parents.reverse();
    }
    nodes.extend([
        source(30, NodeKind::PriceDeck, i64::MAX),
        act(31, NodeKind::Claim, &[32], 0),
        act(32, NodeKind::Claim, &[31], 0),
        act(33, NodeKind::Claim, &[99], 0),
    ]);
    let graph = TemporalGraph::new(nodes).unwrap();
    assert_eq!(graph.check(id(0), &cut(10)).unwrap(), expected);
    let extra = ConeCut {
        decision_ns: 10,
        anchor: Anchor::new([("source", 10), ("acts", 10), ("unrelated", u64::MAX)]).unwrap(),
    };
    assert_eq!(
        graph.check(id(0), &extra).unwrap().digest(),
        expected.digest()
    );
    assert_ne!(
        graph.check(id(0), &cut(11)).unwrap().digest(),
        expected.digest()
    );
}

#[test]
fn iterative_walk_handles_a_deep_chain_without_call_stack_growth() {
    let depth = 20_000;
    let mut nodes: Vec<_> = (0..depth)
        .map(|n| act(n, NodeKind::Claim, &[n + 1], 0))
        .collect();
    nodes.push(source(depth, NodeKind::Observation, 1));
    let graph = TemporalGraph::new(nodes).unwrap();
    assert_eq!(
        graph.check(id(0), &cut(1)).unwrap().nodes().len(),
        depth as usize + 1
    );
    let v = leak(graph.check(id(0), &cut(0)).unwrap_err());
    assert_eq!(v.frontier()[0].node, id(depth));
    assert_eq!(v.frontier()[0].path.len(), depth as usize + 1);
}

/// Enumerate actual paths on a bounded DAG, independently of the kernel's predecessor/indegree tables.
fn shortest_path(nodes: &[TemporalNode], root: ObjectId, target: ObjectId) -> Vec<ObjectId> {
    let mut paths = VecDeque::from([vec![root]]);
    while let Some(path) = paths.pop_front() {
        let end = *path.last().unwrap();
        if end == target {
            return path;
        }
        let mut parents = nodes.iter().find(|n| n.id == end).unwrap().parents.clone();
        parents.sort();
        for parent in parents {
            let mut extended = path.clone();
            extended.push(parent);
            paths.push_back(extended);
        }
    }
    panic!("target belongs to the checked cone")
}

#[test]
fn bounded_dag_property_each_ancestor_leak_has_the_minimal_order_invariant_path() {
    for mask in 0..256_u32 {
        let nodes: Vec<_> = (0..7_u32)
            .map(|n| {
                let parents: Vec<_> = (0..n)
                    .filter(|p| mask & (1 << ((n * (n - 1) / 2 + p) % 8)) != 0)
                    .collect();
                if parents.is_empty() {
                    source(n, NodeKind::Observation, 0)
                } else {
                    act(n, NodeKind::Claim, &parents, 0)
                }
            })
            .collect();
        let baseline = TemporalGraph::new(nodes.clone())
            .unwrap()
            .check(id(6), &cut(0))
            .unwrap();
        for ancestor in baseline.nodes() {
            let mut changed = nodes.clone();
            let node = changed.iter_mut().find(|n| n.id == *ancestor).unwrap();
            node.availability = match node.availability {
                Availability::Source { .. } => Availability::Source {
                    available_at: Some(1),
                },
                Availability::Derived { .. } => Availability::Derived {
                    performed_at: Some(1),
                    declared_available_at: None,
                },
            };
            let expected_path = shortest_path(&changed, id(6), *ancestor);
            let violation = leak(
                TemporalGraph::new(changed.clone())
                    .unwrap()
                    .check(id(6), &cut(0))
                    .unwrap_err(),
            );
            let [frontier] = violation.frontier() else {
                panic!("only the moved ancestor can originate a violation")
            };
            assert_eq!(frontier.node, *ancestor);
            assert_eq!(frontier.path, expected_path);
            changed.reverse();
            for node in &mut changed {
                node.parents.reverse();
            }
            assert_eq!(
                leak(
                    TemporalGraph::new(changed)
                        .unwrap()
                        .check(id(6), &cut(0))
                        .unwrap_err()
                ),
                violation
            );
        }
    }
}

#[test]
fn claim_binding_refuses_a_mutated_body_or_omitted_canonical_parent() {
    use crate::{AdditivityClass, Confidentiality, Representation, SpatialSupport};
    use fmine_evidence::{EpistemicStatus, EvidenceClass, KnowledgeState};
    let mut claim = Claim::new(
        "synthetic.temporal.claim",
        1_u32,
        "1",
        Representation::Deterministic,
        SpatialSupport::PeriodAggregate,
        AdditivityClass::NonAdditive,
        EvidenceClass::Measured,
        EpistemicStatus::KnownKnown,
        KnowledgeState::Known,
        Confidentiality::public(),
        vec![],
        vec![id(1)],
        vec![],
    );
    let mut node = act(0, NodeKind::Claim, &[1], 0);
    node.id = claim.claim_id;
    let leaf = source(1, NodeKind::Observation, 0);
    let valid = TemporalGraph::new([node.clone(), leaf.clone()]).unwrap();
    assert_eq!(valid.verify_claim(&claim), Ok(()));
    assert!(valid.check(claim.claim_id, &cut(0)).is_ok());
    node.parents.clear();
    assert!(matches!(
        TemporalGraph::new([node.clone(), leaf.clone()])
            .unwrap()
            .verify_claim(&claim),
        Err(ConeError::ClaimBinding { .. })
    ));
    node.parents.push(id(1));
    node.kind = NodeKind::Forecast;
    assert!(matches!(
        TemporalGraph::new([node, leaf])
            .unwrap()
            .verify_claim(&claim),
        Err(ConeError::ClaimBinding { .. })
    ));
    claim.state = KnowledgeState::Estimated;
    assert!(matches!(
        valid.verify_claim(&claim),
        Err(ConeError::ClaimBinding { .. })
    ));
}
