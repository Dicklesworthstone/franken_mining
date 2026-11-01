//! Pure, mediated-read replay for `BD-344` / `PO-PLUMB-8`.
//!
//! The same generator runs against immutable canonical bytes, then against a view that refuses every read
//! whose complete ancestry is inadmissible at the cut. A successful certificate also requires every read to
//! belong to the declared root cone. Swallowing a read error cannot turn a failed replay into a pass.
//!
//! This proves only reads mediated by [`ReplayView`]. The caller must use the same seeds and rebuild the output
//! in both invocations; external captures, I/O and cached results cannot be policed by a Rust closure API.
//! Canonical-key checking authenticates the bytes supplied, not fields omitted by the object's canonical codec
//! (for example, a `Claim` body commits its value tag, not a separate encoding of its generic physical value).

use std::collections::{BTreeMap, BTreeSet};

use fmine_core::codec::Encoder;
use fmine_core::hash::{Digest32, blake3_hash};
use fmine_core::id::ObjectId;
use fmine_evidence::{Refusal, RefusalCode};

use super::{ConeCut, TemporalGraph};

#[derive(Debug, Clone)]
struct StoredObject {
    bytes: Vec<u8>,
    digest: Digest32,
}

/// In-memory immutable object bodies. Existing keys cannot be replaced, including by identical bytes.
#[derive(Debug, Clone, Default)]
pub struct ReplayStore {
    objects: BTreeMap<ObjectId, StoredObject>,
}

impl ReplayStore {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Freeze canonical bytes under their content-derived object id. An arbitrary alias or a digest supplied
    /// separately from the bytes is insufficient (`FMINE-REF-507`). No mutation occurs on refusal.
    pub fn insert(&mut self, id: ObjectId, bytes: Vec<u8>) -> Result<(), Refusal> {
        if self.objects.contains_key(&id) {
            return Err(Refusal::new(
                RefusalCode::FrozenRecordMutation507,
                format!("replay object {id} is already frozen"),
            ));
        }
        let actual = ObjectId::from_canonical_bytes(id.kind, &bytes);
        if actual != id {
            return Err(Refusal::new(
                RefusalCode::FrozenRecordMutation507,
                format!("replay object {id} does not name its supplied canonical bytes ({actual})"),
            ));
        }
        let digest = blake3_hash(&bytes);
        self.objects.insert(id, StoredObject { bytes, digest });
        Ok(())
    }
}

/// One attempted read, in execution order, including repeated reads and refused attempts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadReceipt {
    pub id: ObjectId,
    /// Present only when the body was returned; a refused read never exposes its payload digest to the generator.
    pub content_digest: Option<Digest32>,
}

/// A run's output and reads. A generator error has no output digest; a swallowed read error may have one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunReceipt {
    pub output_digest: Option<Digest32>,
    pub reads: Vec<ReadReceipt>,
}

/// Failure retains any completed output digests and every attempted mediated read for diagnosis.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplayError {
    pub refusal: Refusal,
    pub full: Option<Box<RunReceipt>>,
    pub purged: Option<Box<RunReceipt>>,
}

impl ReplayError {
    #[must_use]
    pub fn refusal(&self) -> Refusal {
        self.refusal.clone()
    }
}

impl std::fmt::Display for ReplayError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.refusal.fmt(f)
    }
}

impl std::error::Error for ReplayError {}

/// A generator's only store access. There is no enumeration, mutable body or unguarded fallback method.
pub struct ReplayView<'a> {
    store: &'a ReplayStore,
    graph: &'a TemporalGraph,
    cut: Option<&'a ConeCut>,
    reads: Vec<ReadReceipt>,
    failure: Option<Refusal>,
}

impl<'a> ReplayView<'a> {
    /// Read exact canonical bytes. In the purged run, both the object and all its transitive ancestors must be
    /// admitted by the cut and anchor. Any failed attempt permanently fails this run, even if its error is caught.
    pub fn read(&mut self, id: ObjectId) -> Result<&'a [u8], Refusal> {
        let temporal_error = self.cut.and_then(|cut| {
            self.graph.check(id, cut).err().map(|error| {
                refuse(format!(
                    "purged replay read {id} has inadmissible ancestry: {error}"
                ))
            })
        });
        let object = self.store.objects.get(&id);
        let failure = temporal_error.or_else(|| {
            object
                .is_none()
                .then(|| refuse(format!("replay object {id} has no frozen canonical body")))
        });
        self.reads.push(ReadReceipt {
            id,
            content_digest: if failure.is_none() {
                object.map(|body| body.digest)
            } else {
                None
            },
        });
        if let Some(error) = failure {
            self.failure.get_or_insert_with(|| error.clone());
            return Err(error);
        }
        // The match keeps the successful-read invariant explicit without an unchecked unwrap.
        match object {
            Some(body) => Ok(&body.bytes),
            None => Err(refuse(format!(
                "replay object {id} has no frozen canonical body"
            ))),
        }
    }
}

/// Proof of a completed, matching pair of mediated executions. Private fields prevent constructing a pass from
/// supplied digests. A consumer must also match its exact output bytes, root, cut and current cone digest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplayCertificate {
    root: ObjectId,
    cut: ConeCut,
    cone_digest: Digest32,
    output_digest: Digest32,
    full: RunReceipt,
    purged: RunReceipt,
}

impl ReplayCertificate {
    #[must_use]
    pub const fn root(&self) -> ObjectId {
        self.root
    }

    #[must_use]
    pub const fn cut(&self) -> &ConeCut {
        &self.cut
    }

    #[must_use]
    pub const fn cone_digest(&self) -> Digest32 {
        self.cone_digest
    }

    #[must_use]
    pub const fn output_digest(&self) -> Digest32 {
        self.output_digest
    }

    #[must_use]
    pub fn full_reads(&self) -> &[ReadReceipt] {
        &self.full.reads
    }

    #[must_use]
    pub fn purged_reads(&self) -> &[ReadReceipt] {
        &self.purged.reads
    }

    #[must_use]
    pub const fn full_run(&self) -> &RunReceipt {
        &self.full
    }

    #[must_use]
    pub const fn purged_run(&self) -> &RunReceipt {
        &self.purged
    }

    #[must_use]
    pub fn matches_output(&self, canonical_bytes: &[u8]) -> bool {
        self.output_digest == blake3_hash(canonical_bytes)
    }

    /// Receipt identity binds the checked cone (including its cut), exact generated output and ordered reads.
    #[must_use]
    pub fn digest(&self) -> Digest32 {
        let mut e = Encoder::new_frame(*b"RPLY", 1);
        e.put_str(&self.root.to_string());
        e.put_fixed(self.cone_digest.as_bytes());
        e.put_fixed(self.output_digest.as_bytes());
        e.put_u64(self.full.reads.len() as u64);
        for read in &self.full.reads {
            e.put_str(&read.id.to_string());
            if let Some(digest) = read.content_digest {
                e.put_u8(1);
                e.put_fixed(digest.as_bytes());
            } else {
                e.put_u8(0);
            }
        }
        blake3_hash(&e.finish())
    }
}

fn refuse(detail: impl Into<String>) -> Refusal {
    Refusal::new(RefusalCode::TemporalConeViolation110, detail)
}

fn run<'a, F>(
    store: &'a ReplayStore,
    graph: &'a TemporalGraph,
    cut: Option<&'a ConeCut>,
    generate: &F,
) -> (Result<Vec<u8>, Refusal>, RunReceipt)
where
    F: Fn(&mut ReplayView<'a>) -> Result<Vec<u8>, Refusal>,
{
    let mut view = ReplayView {
        store,
        graph,
        cut,
        reads: Vec::new(),
        failure: None,
    };
    let output = generate(&mut view);
    let receipt = RunReceipt {
        output_digest: output.as_ref().ok().map(|bytes| blake3_hash(bytes)),
        reads: view.reads,
    };
    (view.failure.map_or(output, Err), receipt)
}

/// Check the declared cone, execute the same generator over full and purged views, and compare exact output
/// bytes and ordered read receipts. Undeclared pre-cut reads refuse too: equal digests cannot excuse missing
/// provenance. The full and purged views share immutable inputs and supply no derived-output cache.
pub fn verify_replay<'a, F>(
    store: &'a ReplayStore,
    graph: &'a TemporalGraph,
    root: ObjectId,
    cut: &'a ConeCut,
    generate: F,
) -> Result<ReplayCertificate, ReplayError>
where
    F: Fn(&mut ReplayView<'a>) -> Result<Vec<u8>, Refusal>,
{
    let cone = graph.check(root, cut).map_err(|error| ReplayError {
        refusal: refuse(format!(
            "replay root {root} has inadmissible ancestry: {error}"
        )),
        full: None,
        purged: None,
    })?;
    let (output, full) = run(store, graph, None, &generate);
    let full_output = output.map_err(|refusal| ReplayError {
        refusal,
        full: Some(Box::new(full.clone())),
        purged: None,
    })?;
    let (output, purged) = run(store, graph, Some(cut), &generate);
    let failure = |refusal| ReplayError {
        refusal,
        full: Some(Box::new(full.clone())),
        purged: Some(Box::new(purged.clone())),
    };
    let purged_output = output.map_err(failure)?;
    let allowed: BTreeSet<_> = cone.nodes().iter().copied().collect();
    for read in full.reads.iter().chain(&purged.reads) {
        if !allowed.contains(&read.id) {
            return Err(failure(refuse(format!(
                "replay read {} is not in the declared ancestry of {root}",
                read.id
            ))));
        }
    }
    if full_output != purged_output {
        return Err(failure(refuse(
            "full and purged replay output bytes differ",
        )));
    }
    if full.reads != purged.reads {
        return Err(failure(refuse(
            "full and purged replay read receipts differ",
        )));
    }
    Ok(ReplayCertificate {
        root,
        cut: cut.clone(),
        cone_digest: cone.digest(),
        output_digest: blake3_hash(&full_output),
        full,
        purged,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    use fmine_core::id::PrefixKind;
    use fmine_time::{Anchor, RecordTime, TimeEvidence};

    use crate::dag::{Availability, NodeKind, TemporalNode};

    fn id(bytes: &[u8]) -> ObjectId {
        ObjectId::from_canonical_bytes(PrefixKind::Claim, bytes)
    }

    fn source(bytes: &[u8], available_at: i64, seq: u64) -> TemporalNode {
        TemporalNode {
            id: id(bytes),
            kind: NodeKind::Observation,
            parents: vec![],
            availability: Availability::Source {
                available_at: Some(available_at),
            },
            recorded_at: Some(RecordTime::new(
                "site",
                seq,
                available_at,
                TimeEvidence::HostSynced,
            )),
        }
    }

    fn cut() -> ConeCut {
        ConeCut {
            decision_ns: 10,
            anchor: Anchor::new([("site", 10)]).expect("unique authority cell"),
        }
    }

    struct Fixture {
        store: ReplayStore,
        nodes: Vec<TemporalNode>,
        a: ObjectId,
        b: ObjectId,
        root: ObjectId,
    }

    impl Fixture {
        fn new() -> Self {
            let (a, b, root) = (id(b"2"), id(b"3"), id(b"sum of two inputs"));
            let mut store = ReplayStore::new();
            for bytes in [b"2".as_slice(), b"3", b"sum of two inputs"] {
                store.insert(id(bytes), bytes.to_vec()).expect("canonical");
            }
            let mut root_node = source(b"sum of two inputs", 3, 3);
            root_node.kind = NodeKind::Claim;
            root_node.parents = vec![a, b];
            root_node.availability = Availability::Derived {
                performed_at: Some(3),
                declared_available_at: Some(3),
            };
            Self {
                store,
                nodes: vec![source(b"2", 1, 1), source(b"3", 2, 2), root_node],
                a,
                b,
                root,
            }
        }

        fn add(&mut self, bytes: &[u8], available_at: i64, seq: u64) -> ObjectId {
            let node = source(bytes, available_at, seq);
            let id = node.id;
            self.store.insert(id, bytes.to_vec()).expect("new object");
            self.nodes.push(node);
            id
        }
    }

    fn sum(view: &mut ReplayView<'_>, a: ObjectId, b: ObjectId) -> Result<Vec<u8>, Refusal> {
        let a = view.read(a)?[0] - b'0';
        let b = view.read(b)?[0] - b'0';
        Ok(vec![b'0' + a + b])
    }

    #[test]
    fn clean_replay_rebuilds_exact_output_and_binds_actual_payload_reads() {
        let fixture = Fixture::new();
        let graph = TemporalGraph::new(fixture.nodes).expect("graph");
        let cut = cut();
        let calls = Cell::new(0);
        let certificate = verify_replay(&fixture.store, &graph, fixture.root, &cut, |view| {
            calls.set(calls.get() + 1);
            sum(view, fixture.a, fixture.b)
        })
        .expect("two admissible runs");
        assert_eq!(calls.get(), 2);
        assert_eq!(certificate.root(), fixture.root);
        assert_eq!(certificate.cut(), &cut);
        assert_eq!(certificate.output_digest(), blake3_hash(b"5"));
        assert!(certificate.matches_output(b"5"));
        assert!(!certificate.matches_output(b"6"));
        assert_eq!(certificate.full_run(), certificate.purged_run());
        assert_eq!(
            certificate.full_reads(),
            &[
                ReadReceipt {
                    id: fixture.a,
                    content_digest: Some(blake3_hash(b"2"))
                },
                ReadReceipt {
                    id: fixture.b,
                    content_digest: Some(blake3_hash(b"3"))
                },
            ]
        );
        let again = verify_replay(&fixture.store, &graph, fixture.root, &cut, |view| {
            sum(view, fixture.a, fixture.b)
        })
        .expect("deterministic replay");
        assert_eq!(certificate, again);
        assert_eq!(certificate.digest(), again.digest());
    }

    #[test]
    fn store_rejects_wrong_body_and_both_identical_and_changed_replacements() {
        let mut store = ReplayStore::new();
        let key = id(b"original");
        assert_eq!(
            store.insert(key, b"different".to_vec()).unwrap_err().code,
            "FMINE-REF-507"
        );
        // The failed insertion neither reserves the key nor writes the wrong bytes.
        store
            .insert(key, b"original".to_vec())
            .expect("first valid write");
        for bytes in [b"original".as_slice(), b"different"] {
            assert_eq!(
                store.insert(key, bytes.to_vec()).unwrap_err().code,
                "FMINE-REF-507"
            );
        }
        assert_eq!(store.objects[&key].bytes, b"original");
        assert_eq!(store.objects[&key].digest, blake3_hash(b"original"));
    }

    #[test]
    fn unrecorded_future_dependency_fails_even_when_graph_and_output_look_clean() {
        let mut fixture = Fixture::new();
        let future = fixture.add(b"future fitted parameter", 11, 4);
        let graph = TemporalGraph::new(fixture.nodes).expect("graph");
        let cut = cut();
        assert!(graph.check(fixture.root, &cut).is_ok());
        let error = verify_replay(&fixture.store, &graph, fixture.root, &cut, |view| {
            // Deliberately swallow the failed purged read and return the same output as the full run.
            let _ = view.read(future);
            sum(view, fixture.a, fixture.b)
        })
        .expect_err("a caught post-cut read must still fail");
        assert_eq!(error.refusal.code, "FMINE-REF-110");
        assert!(error.refusal.what.contains(&future.to_string()));
        let full = error.full.expect("completed full run");
        let purged = error.purged.expect("attempted purged run");
        assert_eq!(full.output_digest, Some(blake3_hash(b"5")));
        assert_eq!(full.output_digest, purged.output_digest);
        assert_eq!(
            full.reads[0].content_digest,
            Some(blake3_hash(b"future fitted parameter"))
        );
        assert_eq!(
            purged.reads[0],
            ReadReceipt {
                id: future,
                content_digest: None
            }
        );
    }

    #[test]
    fn unrecorded_pre_cut_dependency_is_not_excused_by_matching_replay() {
        let mut fixture = Fixture::new();
        let hidden = fixture.add(b"available but undeclared prior", 4, 4);
        let graph = TemporalGraph::new(fixture.nodes).expect("graph");
        let error = verify_replay(&fixture.store, &graph, fixture.root, &cut(), |view| {
            let _bytes = view.read(hidden)?;
            sum(view, fixture.a, fixture.b)
        })
        .expect_err("all reads need declared provenance");
        assert!(error.refusal.what.contains("not in the declared ancestry"));
        assert!(error.refusal.what.contains(&hidden.to_string()));
        assert_eq!(error.full, error.purged);
    }

    #[test]
    fn guarded_reads_check_anchor_sequence_and_missing_ancestor_metadata() {
        for mode in [
            "post_anchor",
            "missing_cell",
            "missing_time",
            "missing_parent",
            "cycle",
        ] {
            let mut fixture = Fixture::new();
            let hidden = fixture.add(mode.as_bytes(), 4, 4);
            let node = fixture.nodes.last_mut().expect("hidden node");
            match mode {
                "post_anchor" => {
                    node.recorded_at =
                        Some(RecordTime::new("site", 11, 4, TimeEvidence::HostSynced))
                }
                "missing_cell" => {
                    node.recorded_at = Some(RecordTime::new(
                        "unsynchronized",
                        1,
                        4,
                        TimeEvidence::HostSynced,
                    ))
                }
                "missing_time" => node.recorded_at = None,
                "missing_parent" => node.parents.push(id(b"unregistered ancestor")),
                "cycle" => node.parents.push(hidden),
                _ => unreachable!(),
            }
            let graph =
                TemporalGraph::new(fixture.nodes).expect("unrelated incomplete node is retained");
            let cut = cut();
            assert!(graph.check(fixture.root, &cut).is_ok(), "{mode}");
            let error = verify_replay(&fixture.store, &graph, fixture.root, &cut, |view| {
                Ok(view.read(hidden)?.to_vec())
            })
            .expect_err("the read must validate its own complete temporal cone");
            assert_eq!(error.refusal.code, "FMINE-REF-110", "{mode}");
            assert!(
                error.full.expect("full run").output_digest.is_some(),
                "{mode}"
            );
            assert!(
                error
                    .purged
                    .expect("guarded attempt")
                    .output_digest
                    .is_none(),
                "{mode}"
            );
        }
    }

    #[test]
    fn missing_body_and_bad_root_fail_before_a_certificate_can_exist() {
        let fixture = Fixture::new();
        let graph = TemporalGraph::new(fixture.nodes).expect("graph");
        let mut incomplete_store = ReplayStore::new();
        incomplete_store
            .insert(fixture.a, b"2".to_vec())
            .expect("one body");
        let error = verify_replay(&incomplete_store, &graph, fixture.root, &cut(), |view| {
            let _ = view.read(fixture.b);
            Ok(b"cached answer".to_vec())
        })
        .expect_err("catching missing-body errors cannot pass");
        assert!(error.full.expect("full attempt").output_digest.is_some());
        assert!(error.purged.is_none());
        let called = Cell::new(false);
        let error = verify_replay(&fixture.store, &graph, id(b"absent root"), &cut(), |_| {
            called.set(true);
            Ok(Vec::new())
        })
        .expect_err("invalid declared cone");
        assert!(!called.get());
        assert!(error.full.is_none());
        assert!(error.purged.is_none());
    }

    #[test]
    fn unequal_outputs_and_unequal_read_traces_both_refuse() {
        let fixture = Fixture::new();
        let graph = TemporalGraph::new(fixture.nodes).expect("graph");
        for change_output in [true, false] {
            let calls = Cell::new(0);
            let error = verify_replay(&fixture.store, &graph, fixture.root, &cut(), |view| {
                calls.set(calls.get() + 1);
                let mut result = sum(view, fixture.a, fixture.b)?;
                if calls.get() == 2 {
                    if change_output {
                        result.push(b'!');
                    } else {
                        let _repeated = view.read(fixture.a)?;
                    }
                }
                Ok(result)
            })
            .expect_err("the two runs must match outputs and ordered reads");
            assert_eq!(calls.get(), 2);
            let expected = if change_output {
                "output bytes differ"
            } else {
                "read receipts differ"
            };
            assert!(error.refusal.what.contains(expected));
        }
    }

    #[test]
    fn unrelated_future_objects_do_not_change_successful_replay() {
        let mut fixture = Fixture::new();
        let cut = cut();
        let clean = TemporalGraph::new(fixture.nodes.clone()).expect("clean graph");
        let before = verify_replay(&fixture.store, &clean, fixture.root, &cut, |view| {
            sum(view, fixture.a, fixture.b)
        })
        .expect("clean replay");
        fixture.add(b"later observation never read", 100, 100);
        let augmented = TemporalGraph::new(fixture.nodes).expect("graph with unrelated future");
        let after = verify_replay(&fixture.store, &augmented, fixture.root, &cut, |view| {
            sum(view, fixture.a, fixture.b)
        })
        .expect("unrelated future is irrelevant");
        assert_eq!(before, after);
        assert_eq!(before.digest(), after.digest());
    }
}
