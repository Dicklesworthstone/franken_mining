//! Escalation lab tests on virtual time: the restart strategies, the intensity window, each escalation, the spec
//! checks, and seeded schedules checked against an independent model of the failure history.

use std::collections::BTreeMap;

use super::*;
use RegionKind as K;

const S: u64 = 1_000_000_000;

/// A full site tree in start order. Indices: 0 process, 1 node, 2 authority, 3-4 writers c1 and c2, 5 object store,
/// 6 ingest, 7-8 telemetry s1 and s2, 9 quarantine csv, 10 compute, 11 law, 12 run r1, 13 projection, 14 sync,
/// 15 session p1, 16 transfer t1 over p1, 17 session p2, 18 transfer t2 over p2, 19 effect, 20 operations.
fn site_tree() -> Vec<Child> {
    vec![
        Child::one(K::Process),
        Child::one(K::Node),
        Child::one(K::Authority),
        Child::keyed(K::LedgerWriter, "c1"),
        Child::keyed(K::LedgerWriter, "c2"),
        Child::one(K::ObjectStore),
        Child::one(K::Ingest),
        Child::keyed(K::Telemetry, "s1"),
        Child::keyed(K::Telemetry, "s2"),
        Child::keyed(K::ImportQuarantine, "csv"),
        Child::one(K::Compute),
        Child::one(K::Law),
        Child::keyed(K::EstimationRun, "r1"),
        Child::one(K::Projection),
        Child::one(K::Sync),
        Child::keyed(K::PeerSession, "p1"),
        Child::transfer("t1", "p1"),
        Child::keyed(K::PeerSession, "p2"),
        Child::transfer("t2", "p2"),
        Child::one(K::Effect),
        Child::one(K::Operations),
    ]
}

fn pick(indices: impl IntoIterator<Item = usize>) -> Vec<Child> {
    let tree = site_tree();
    indices.into_iter().map(|i| tree[i].clone()).collect()
}

fn at(i: usize) -> Child {
    site_tree()[i].clone()
}

fn site() -> CompiledSupervisor {
    let mut sup = AppSpec::for_role(Role::Site).compile().unwrap();
    for child in site_tree() {
        sup.start(child, 0).unwrap();
    }
    sup
}

fn restart(failed: usize, restarted: impl IntoIterator<Item = usize>) -> Outcome {
    Outcome::Restart {
        failed: at(failed),
        restarted: pick(restarted),
    }
}

fn plan() -> String {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../COMPREHENSIVE_PLAN_FOR_THE_DESIGN_OF_FRANKENMINING.md"
    );
    std::fs::read_to_string(path).unwrap()
}

#[test]
fn every_role_compiles_and_a_broken_spec_is_refused() {
    for role in Role::ALL {
        let spec = AppSpec::for_role(role);
        assert_eq!(spec.regions, profile(role));
        let sup = spec.compile().unwrap();
        assert_eq!(sup.role(), role);
        assert!(sup.running().is_empty());
    }
    let site = AppSpec::for_role(Role::Site);
    let broken = |edit: &dyn Fn(&mut Vec<RegionSpec>)| {
        let mut spec = site.clone();
        edit(&mut spec.regions);
        spec.compile().unwrap_err()
    };
    let find = |regions: &[RegionSpec], kind| regions.iter().position(|r| r.kind == kind).unwrap();
    assert_eq!(broken(&|r| r.clear()), SpecError::Empty);
    assert_eq!(
        broken(&|r| {
            r.remove(0);
        }),
        SpecError::RootHasParent(K::Node)
    );
    assert_eq!(
        broken(&|r| r[1].parent = None),
        SpecError::SecondRoot(K::Node)
    );
    assert_eq!(
        broken(&|r| {
            let writer = r.remove(find(r, K::LedgerWriter));
            r.insert(find(r, K::Authority), writer);
        }),
        SpecError::MissingParent(K::LedgerWriter)
    );
    assert_eq!(
        broken(&|r| r.push(r[find(r, K::Law)])),
        SpecError::Duplicate(K::Law)
    );
    assert_eq!(
        broken(&|r| {
            let i = find(r, K::Authority);
            r[i].instances = Instances::PerKey("x");
        }),
        SpecError::KeyedParent(K::LedgerWriter)
    );
    assert_eq!(
        broken(&|r| {
            let i = find(r, K::Sync);
            r[i].supervisor = None;
        }),
        SpecError::NoStrategy(K::Sync)
    );
    let mut wrong = site.clone();
    wrong.role = Role::Enterprise;
    assert_eq!(wrong.compile(), Err(SpecError::WrongRole(K::Ingest)));
}

#[test]
fn the_intensity_is_the_plans_and_the_sixth_failure_in_60_s_escalates() {
    let text = format!(
        "Restart intensity is {RESTARTS} restarts per {} s per child",
        WINDOW_NS / S
    );
    assert!(plan().contains(&text), "{text}");

    let r1 = at(12);
    let mut sup = site();
    for t in 0..5 {
        assert_eq!(sup.fail(&r1, t * S), Ok(restart(12, [12])));
    }
    // The run's sixth failure within 60 s fails the compute region, and the node restarts it and everything after.
    assert_eq!(sup.fail(&r1, 59 * S), Ok(restart(10, 10..21)));
    // The restart cleared the run's history.
    assert_eq!(sup.fail(&r1, 59 * S + 1), Ok(restart(12, [12])));

    let mut edge = site();
    for t in 0..5 {
        edge.fail(&r1, t * S).unwrap();
    }
    // At 60 s the failure at 0 s has left the window.
    assert_eq!(edge.fail(&r1, 60 * S), Ok(restart(12, [12])));
    assert_eq!(edge.fail(&r1, 60 * S), Ok(restart(10, 10..21)));
}

#[test]
fn strategies_restart_the_failed_child_its_peer_group_or_the_rest() {
    let mut sup = site();
    // Ingest and compute are one-for-one.
    assert_eq!(sup.fail(&at(7), S), Ok(restart(7, [7])));
    assert_eq!(sup.fail(&at(11), S), Ok(restart(11, [11])));
    // Authority is rest-for-one, in start order.
    assert_eq!(sup.fail(&at(3), S), Ok(restart(3, 3..6)));
    assert_eq!(sup.fail(&at(4), S), Ok(restart(4, 4..6)));
    assert_eq!(sup.fail(&at(5), S), Ok(restart(5, [5])));
    // Sync is one-for-all per peer: a session or a transfer restarts its peer's group only.
    assert_eq!(sup.fail(&at(15), S), Ok(restart(15, [15, 16])));
    assert_eq!(sup.fail(&at(18), S), Ok(restart(18, [17, 18])));
    // The node is rest-for-one, and an interior child restarts with every descendant.
    assert_eq!(sup.fail(&at(2), S), Ok(restart(2, 2..21)));
    assert_eq!(sup.fail(&at(20), S), Ok(restart(20, [20])));
    assert_eq!(sup.running(), site_tree());
}

#[test]
fn an_escalated_peer_goes_stale_and_local_authority_keeps_running() {
    let (p1, t1) = (at(15), at(16));
    let mut sup = site();
    sup.fail(&at(17), 0).unwrap();
    for t in 0..5 {
        assert_eq!(sup.fail(&p1, t * S), Ok(restart(15, [15, 16])));
    }
    assert_eq!(
        sup.fail(&p1, 5 * S),
        Ok(Outcome::PeerStale {
            peer: "p1".into(),
            stopped: vec![p1.clone(), t1.clone()],
        })
    );
    assert!(sup.is_stale("p1") && !sup.is_stale("p2"));
    let mut rest = site_tree();
    rest.retain(|c| *c != p1 && *c != t1);
    assert_eq!(sup.running(), rest);
    assert_eq!(sup.writes(), Ok(()));
    assert_eq!(sup.fail(&at(3), 6 * S), Ok(restart(3, 3..6)));
    assert_eq!(sup.fail(&p1, 6 * S), Err(Refused::NotRunning(p1.clone())));
    assert_eq!(
        sup.start(Child::transfer("t3", "p1"), 6 * S),
        Err(Refused::ParentNotRunning(p1.clone()))
    );
    // The other peer keeps its own history and group: its second failure restarts it with its transfer.
    assert_eq!(sup.fail(&at(17), 6 * S), Ok(restart(17, [17, 18])));
    // A new session clears the stale mark and starts with no failures.
    sup.start(p1.clone(), 7 * S).unwrap();
    assert!(!sup.is_stale("p1"));
    assert_eq!(sup.fail(&p1, 7 * S), Ok(restart(15, [15])));
}

#[test]
fn an_escalated_ledger_writer_turns_the_node_read_only_and_is_never_restarted() {
    let plan = plan();
    let exits = &plan[plan.find("**Exit codes**").unwrap()..];
    let row = exits
        .lines()
        .find(|l| l.starts_with('|') && l.contains("writer escalation"))
        .unwrap();
    let code: u8 = row.split('|').nth(1).unwrap().trim().parse().unwrap();

    let c2 = at(4);
    let mut sup = site();
    for t in 0..5 {
        assert_eq!(sup.fail(&c2, t * S), Ok(restart(4, 4..6)));
    }
    assert_eq!(
        sup.fail(&c2, 5 * S),
        Ok(Outcome::ReadOnly {
            cell: "c2".into(),
            stopped: pick(3..5),
        })
    );
    let refused = Refused::ReadOnly { cell: "c2".into() };
    assert_eq!(sup.writes(), Err(refused.clone()));
    assert_eq!(refused.exit_code(), Some(code));
    assert_eq!(code, 4);
    for writer in [c2, at(3), Child::keyed(K::LedgerWriter, "c3")] {
        assert_eq!(sup.start(writer, 6 * S), Err(refused.clone()));
    }
    // Reads and the rest of the tree run on, and a later authority restart brings back no writer.
    assert_eq!(sup.fail(&at(5), 6 * S), Ok(restart(5, [5])));
    let Ok(Outcome::Restart { restarted, .. }) = sup.fail(&at(2), 7 * S) else {
        panic!("authority restarts")
    };
    assert_eq!(restarted, pick([2, 5].into_iter().chain(6..21)));
    assert!(sup.running().iter().all(|c| c.region != K::LedgerWriter));
    assert_eq!(sup.writes(), Err(refused));
}

#[test]
fn escalation_climbs_to_the_root_and_shuts_the_node_down() {
    let mut sup = site();
    for t in 0..5 {
        sup.fail(&at(7), t * S).unwrap();
    }
    // Telemetry's sixth failure fails ingest, and the node restarts ingest and everything after it.
    assert_eq!(sup.fail(&at(7), 5 * S), Ok(restart(6, 6..21)));
    // Ingest has one failure in the window, so four more restart it and the fifth fails the node.
    for t in 6..10 {
        assert_eq!(sup.fail(&at(6), t * S), Ok(restart(6, 6..21)));
    }
    assert_eq!(sup.fail(&at(6), 10 * S), Ok(Outcome::Shutdown));
    assert!(sup.running().is_empty());
    assert_eq!(sup.writes(), Err(Refused::Stopped));
    assert_eq!(sup.fail(&at(0), 11 * S), Err(Refused::Stopped));
    assert_eq!(sup.start(at(0), 11 * S), Err(Refused::Stopped));

    // The process root has no strategy, so a node failure or a root failure stops at once.
    for i in [0, 1] {
        assert_eq!(site().fail(&at(i), 0), Ok(Outcome::Shutdown));
    }
}

#[test]
fn starts_are_checked_against_the_spec_and_time_never_goes_back() {
    let mut enterprise = AppSpec::for_role(Role::Enterprise).compile().unwrap();
    enterprise.start(at(0), 0).unwrap();
    enterprise.start(at(1), 0).unwrap();
    assert_eq!(
        enterprise.start(at(6), 0),
        Err(Refused::NotInSpec(K::Ingest))
    );
    enterprise.start(Child::one(K::FinanceBridge), 0).unwrap();
    let mut sup = site();
    assert_eq!(
        sup.start(Child::one(K::FinanceBridge), 0),
        Err(Refused::NotInSpec(K::FinanceBridge))
    );
    for bad in [
        Child::one(K::LedgerWriter),
        Child::keyed(K::Authority, "x"),
        Child::one(K::EstimationRun),
        Child {
            region: K::PeerSession,
            key: Some("p".into()),
            peer: Some("q".into()),
        },
        Child {
            region: K::Transfer,
            key: Some("r".into()),
            peer: None,
        },
        Child {
            region: K::Telemetry,
            key: Some("s".into()),
            peer: Some("p1".into()),
        },
    ] {
        assert_eq!(sup.start(bad.clone(), 0), Err(Refused::Shape(bad)));
    }
    assert_eq!(sup.start(at(7), 0), Err(Refused::AlreadyRunning(at(7))));
    sup.start(Child::keyed(K::EstimationRun, "r2"), 10).unwrap();
    assert_eq!(sup.fail(&at(12), 9), Err(Refused::TimeWentBack));
    assert_eq!(sup.start(at(12), 9), Err(Refused::TimeWentBack));
    let ghost = Child::keyed(K::Telemetry, "s9");
    assert_eq!(sup.fail(&ghost, 10), Err(Refused::NotRunning(ghost)));

    let mut fresh = AppSpec::for_role(Role::Site).compile().unwrap();
    assert_eq!(fresh.start(at(1), 0), Err(Refused::ParentNotRunning(at(0))));
    fresh.start(at(0), 0).unwrap();
    fresh.start(at(1), 0).unwrap();
    fresh.start(at(14), 0).unwrap();
    assert_eq!(
        fresh.start(at(16), 0),
        Err(Refused::ParentNotRunning(at(15)))
    );
}

/// splitmix64, for the lab schedules.
struct Rng(u64);

impl Rng {
    fn below(&mut self, n: u64) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        (z ^ (z >> 31)) % n
    }
}

#[derive(Debug, PartialEq)]
enum Step {
    Start(Child, Result<(), Refused>),
    Fail(Child, Outcome),
}

/// How often each kind of outcome came up, over every seed.
#[derive(Debug, Default)]
struct Seen {
    restarts: BTreeMap<&'static str, usize>,
    escalated_restarts: usize,
    stale: usize,
    read_only: usize,
    shutdowns: usize,
    writer_refused: usize,
}

fn ancestors(region: RegionKind) -> Vec<RegionKind> {
    std::iter::successors(region.spec().parent, |p| p.spec().parent).collect()
}

fn within(window: &mut Vec<u64>, now: u64) -> usize {
    window.retain(|&t| now - t < WINDOW_NS);
    window.len()
}

/// One seeded schedule on a site node: failures, two in three of the last child to fail, and starts of anything,
/// with time advancing 0 to 1.5 s most steps and up to 89 s otherwise. Every outcome is checked against `own`,
/// a model of each child's restarts kept from the outcomes alone.
fn lab(seed: u64, seen: &mut Seen) -> Vec<Step> {
    let tree = site_tree();
    let mut sup = site();
    let mut rng = Rng(seed);
    let mut now = 0;
    let mut own: BTreeMap<Child, Vec<u64>> = BTreeMap::new();
    let mut read_only: Option<String> = None;
    let mut hot: Option<Child> = None;
    let mut trace = Vec::new();
    for _ in 0..200 {
        now += if rng.below(5) == 0 {
            rng.below(90) * S
        } else {
            rng.below(4) * S / 2
        };
        if rng.below(8) == 0 {
            let child = tree[rng.below(tree.len() as u64) as usize].clone();
            let was_running = sup.running().contains(&child);
            let result = sup.start(child.clone(), now);
            match &result {
                Ok(()) => {
                    assert!(!was_running);
                    assert!(child.region != K::LedgerWriter || read_only.is_none());
                    own.remove(&child);
                }
                Err(Refused::AlreadyRunning(_)) => assert!(was_running),
                Err(Refused::ReadOnly { cell }) => {
                    assert_eq!(child.region, K::LedgerWriter);
                    assert_eq!(Some(cell), read_only.as_ref());
                    seen.writer_refused += 1;
                }
                Err(Refused::ParentNotRunning(p)) => assert!(!sup.running().contains(p)),
                Err(other) => panic!("seed {seed}: unexpected refusal {other:?}"),
            }
            trace.push(Step::Start(child, result));
        } else {
            // The process and node fail only by escalation here; a direct failure is the scripted test's.
            let below: Vec<&Child> = sup
                .running()
                .iter()
                .filter(|c| !matches!(c.region, K::Process | K::Node))
                .collect();
            let child = match &hot {
                Some(h) if below.contains(&h) && rng.below(3) != 0 => h.clone(),
                _ => below[rng.below(below.len() as u64) as usize].clone(),
            };
            hot = Some(child.clone());
            let outcome = sup.fail(&child, now).unwrap();
            // Walk up from the failed child: every child the failure passes has a full window.
            let mut x = child.clone();
            let top = loop {
                let full = within(own.entry(x.clone()).or_default(), now) == RESTARTS;
                let stops_here = match &outcome {
                    Outcome::Restart { failed, .. } => *failed == x,
                    Outcome::PeerStale { .. } | Outcome::ReadOnly { .. } => true,
                    Outcome::Shutdown => matches!(x.region, K::Node | K::Process),
                };
                if stops_here {
                    break (x, full);
                }
                assert!(full, "seed {seed}: {x:?} escalated with room left");
                x = Child::one(x.region.spec().parent.unwrap());
            };
            match &outcome {
                Outcome::Restart { failed, restarted } => {
                    assert!(!top.1, "seed {seed}: {failed:?} restarted a sixth time");
                    own.entry(failed.clone()).or_default().push(now);
                    assert!(restarted.contains(failed));
                    assert!(restarted.iter().all(|c| sup.running().contains(c)));
                    let parent = failed.region.spec().parent.unwrap();
                    let strategy = parent.spec().supervisor.unwrap();
                    if parent == K::Sync {
                        assert!(restarted.iter().all(|c| c.peer == failed.peer));
                    }
                    let name = match strategy {
                        Supervisor::OneForOne => "OneForOne",
                        Supervisor::RestForOne => "RestForOne",
                        Supervisor::OneForAll => "OneForAll",
                    };
                    *seen.restarts.entry(name).or_default() += 1;
                    seen.escalated_restarts += usize::from(*failed != child);
                    own.retain(|k, _| {
                        !restarted
                            .iter()
                            .any(|g| ancestors(k.region).contains(&g.region))
                    });
                }
                Outcome::PeerStale { peer, stopped } => {
                    assert!(top.1 && child.region == K::PeerSession);
                    assert!(sup.is_stale(peer));
                    assert!(stopped.iter().all(|c| c.peer.as_ref() == Some(peer)));
                    for c in stopped {
                        own.remove(c);
                    }
                    seen.stale += 1;
                }
                Outcome::ReadOnly { cell, stopped } => {
                    assert!(top.1 && child.region == K::LedgerWriter);
                    assert!(stopped.iter().all(|c| c.region == K::LedgerWriter));
                    for c in stopped {
                        own.remove(c);
                    }
                    read_only = Some(cell.clone());
                    seen.read_only += 1;
                }
                Outcome::Shutdown => {
                    assert!(sup.running().is_empty());
                    assert_eq!(sup.writes(), Err(Refused::Stopped));
                    assert_eq!(sup.fail(&child, now), Err(Refused::Stopped));
                    seen.shutdowns += 1;
                    trace.push(Step::Fail(child, outcome));
                    return trace;
                }
            }
            trace.push(Step::Fail(child, outcome));
        }
        // The running set is a closed tree, a read-only node runs no writer, and a stale peer runs nothing.
        let running = sup.running();
        for c in running {
            if let Some(parent) = c.region.spec().parent {
                assert!(
                    running.contains(&Child::one(parent)),
                    "seed {seed}: orphan {c:?}"
                );
            }
            if let (K::Transfer, Some(peer)) = (c.region, &c.peer) {
                assert!(running.contains(&Child::keyed(K::PeerSession, peer)));
            }
            if let Some(peer) = &c.peer {
                assert!(!sup.is_stale(peer));
            }
        }
        match &read_only {
            Some(cell) => {
                assert_eq!(sup.writes(), Err(Refused::ReadOnly { cell: cell.clone() }));
                assert!(running.iter().all(|c| c.region != K::LedgerWriter));
            }
            None => assert_eq!(sup.writes(), Ok(())),
        }
    }
    trace
}

#[test]
fn seeded_schedules_match_the_model_and_replay_the_same() {
    let mut seen = Seen::default();
    for seed in 0..1000 {
        let trace = lab(seed, &mut seen);
        assert_eq!(lab(seed, &mut Seen::default()), trace, "seed {seed}");
    }
    eprintln!("{seen:?}");
    // Every path came up, so no check above passed for want of a case.
    for strategy in ["OneForOne", "RestForOne", "OneForAll"] {
        assert!(
            seen.restarts.get(strategy).is_some_and(|&n| n > 0),
            "{seen:?}"
        );
    }
    assert!(seen.escalated_restarts > 0, "{seen:?}");
    assert!(
        seen.stale > 0 && seen.read_only > 0 && seen.shutdowns > 0,
        "{seen:?}"
    );
    assert!(seen.writer_refused > 0, "{seen:?}");
}
