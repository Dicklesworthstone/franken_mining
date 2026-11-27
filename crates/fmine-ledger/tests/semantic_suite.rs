//! `BD-087`: the shared semantic suite (plan §26.5: "the same semantic test suite runs against every rung").
//!
//! Every case runs on three rungs:
//! - L0, `L0Ledger`, the in-memory oracle, committing under its writer lease;
//! - L1, `SemanticSegmentWriter`, the durable `FMT-SEG-1` writer;
//! - L1 again, closed and reopened from disk after every commit, so its index is rebuilt from the bytes.
//!
//! Each rung must give each case's expected outcomes on its own. A refusal must leave the rung unchanged, and
//! on L1 the file's bytes too. A seeded differential then drives L0 and L1 through random histories and
//! requires the same outcome, digest and state after every batch.
//!
//! The rungs differ in one way by design. L0 commits under a lease that its authority transfers, and L1 learns a
//! new epoch from the first batch it admits. So an epoch step here transfers L0's lease and then commits a
//! batch at the new epoch on every rung.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use std::path::PathBuf;

use fmine_core::anchor::{FMT_ANCHOR_V1, LedgerAnchor};
use fmine_core::hash::Digest32;
use fmine_ledger::batch::{DeltaBatch, DeltaKind, DeltaOp, FMT_LEDGER_V2, Witness};
use fmine_ledger::cell::CellLease;
use fmine_ledger::l0::{L0Error, L0Ledger};
use fmine_ledger::segment::SemanticSegmentWriter;

const CELL: &str = "CELL-EVID(suite)";
const KEYS: [&str; 4] = ["a", "b", "c", "d"];
const OBLIGATIONS: [&str; 3] = ["OB-1", "OB-2", "OB-3"];

type Law = fn(&L0Ledger, &DeltaBatch) -> Result<(), String>;

fn no_law(_: &L0Ledger, _: &DeltaBatch) -> Result<(), String> {
    Ok(())
}

/// A law that reads the ledger before the commit: a key holds at most two versions.
fn at_most_two_versions(state: &L0Ledger, batch: &DeltaBatch) -> Result<(), String> {
    for op in &batch.deltas {
        if matches!(op.kind, DeltaKind::Put | DeltaKind::Supersede) {
            let held = state
                .key_history(&batch.cell_id, &op.key)
                .map_err(|e| e.to_string())?
                .len();
            if held >= 2 {
                return Err(format!("'{}' already holds {held} versions", op.key));
            }
        }
    }
    Ok(())
}

trait Rung {
    fn name(&self) -> &'static str;
    fn commit(&mut self, batch: &DeltaBatch, law: Law) -> Result<Digest32, L0Error>;
    /// L0's authority transfers the lease to the next epoch; L1 learns it from the next batch.
    fn new_epoch(&mut self);
    fn state(&self) -> &L0Ledger;
}

struct L0Rung {
    ledger: L0Ledger,
    lease: CellLease,
}

impl L0Rung {
    fn new() -> Self {
        let mut ledger = L0Ledger::new();
        let lease = ledger.create_cell(CELL, "writer-1").unwrap();
        Self { ledger, lease }
    }
}

impl Rung for L0Rung {
    fn name(&self) -> &'static str {
        "L0"
    }

    fn commit(&mut self, batch: &DeltaBatch, law: Law) -> Result<Digest32, L0Error> {
        self.ledger
            .commit_with_conservation(&self.lease, batch.clone(), law)
    }

    fn new_epoch(&mut self) {
        let holder = format!("writer-{}", self.lease.epoch + 1);
        self.lease = self.ledger.transfer_lease(CELL, holder).unwrap();
    }

    fn state(&self) -> &L0Ledger {
        &self.ledger
    }
}

struct L1Rung {
    path: PathBuf,
    writer: Option<SemanticSegmentWriter>,
    reopen_each_commit: bool,
}

impl L1Rung {
    fn new(tag: &str, reopen_each_commit: bool) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "fmine-ledger-suite-{tag}-{}-{reopen_each_commit}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("seg-000001.fmseg");
        let writer = SemanticSegmentWriter::create(&path, CELL).unwrap();
        Self {
            path,
            writer: Some(writer),
            reopen_each_commit,
        }
    }

    fn reopen(&mut self) {
        drop(self.writer.take());
        self.writer = Some(SemanticSegmentWriter::open_append(&self.path).unwrap());
    }
}

impl Rung for L1Rung {
    fn name(&self) -> &'static str {
        if self.reopen_each_commit {
            "L1 reopened"
        } else {
            "L1"
        }
    }

    fn commit(&mut self, batch: &DeltaBatch, law: Law) -> Result<Digest32, L0Error> {
        let bytes = std::fs::read(&self.path).unwrap();
        let outcome = self.writer.as_mut().unwrap().append_with_law(batch, law);
        if outcome.is_err() {
            assert_eq!(
                std::fs::read(&self.path).unwrap(),
                bytes,
                "a refusal changed the segment"
            );
        }
        if self.reopen_each_commit {
            self.reopen();
        }
        outcome
    }

    fn new_epoch(&mut self) {}

    fn state(&self) -> &L0Ledger {
        self.writer.as_ref().unwrap().ledger()
    }
}

fn rungs(tag: &str) -> Vec<Box<dyn Rung>> {
    vec![
        Box::new(L0Rung::new()),
        Box::new(L1Rung::new(tag, false)),
        Box::new(L1Rung::new(tag, true)),
    ]
}

fn batch_at(
    seq: u64,
    prev_hash: Digest32,
    epoch: u64,
    mut deltas: Vec<DeltaOp>,
    mut witnesses: Vec<Witness>,
) -> DeltaBatch {
    deltas.sort();
    deltas.dedup();
    witnesses.sort();
    witnesses.dedup();
    DeltaBatch {
        format_version: FMT_LEDGER_V2,
        cell_id: CELL.to_string(),
        epoch,
        seq,
        prev_hash,
        anchor: LedgerAnchor {
            format_version: FMT_ANCHOR_V1,
            cell_id: CELL.to_string(),
            ledger_epoch: epoch,
            commit_sequence: seq,
            adapter_registry_epoch: 1,
            schema_epoch: 1,
            policy_epoch: 1,
            privacy_epoch: 1,
            residency_epoch: 1,
            state_root: prev_hash,
        },
        deltas,
        witnesses,
        cross_cell_refs: Vec::new(),
    }
}

/// A batch at the rung's head.
fn next(rung: &dyn Rung, epoch: u64, deltas: Vec<DeltaOp>, witnesses: Vec<Witness>) -> DeltaBatch {
    let state = rung.state();
    batch_at(
        state.committed_count(CELL).unwrap(),
        state.head_hash(CELL).unwrap(),
        epoch,
        deltas,
        witnesses,
    )
}

fn op(key: &str, kind: DeltaKind, payload: &[u8]) -> DeltaOp {
    DeltaOp::new(key, kind, payload.to_vec())
}

fn witness(rung: &dyn Rung, key: &str) -> Witness {
    rung.state().key_witness(CELL, key).unwrap()
}

/// Everything a reader can ask a rung, at every committed count.
#[derive(Debug, PartialEq, Eq)]
struct Observed {
    committed: u64,
    head: Digest32,
    snapshots: Vec<fmine_ledger::l0::CellSnapshot>,
    histories: Vec<Vec<fmine_ledger::l0::KeyVersion>>,
    witnesses: Vec<Witness>,
    open: Vec<String>,
}

fn observe(rung: &dyn Rung) -> Observed {
    let state = rung.state();
    let committed = state.committed_count(CELL).unwrap();
    Observed {
        committed,
        head: state.head_hash(CELL).unwrap(),
        snapshots: (0..=committed)
            .map(|n| state.as_of_seq(CELL, n).unwrap())
            .collect(),
        histories: KEYS
            .iter()
            .map(|key| state.key_history(CELL, key).unwrap().to_vec())
            .collect(),
        witnesses: KEYS
            .iter()
            .map(|key| state.key_witness(CELL, key).unwrap())
            .collect(),
        open: state
            .open_obligations(CELL)
            .unwrap()
            .iter()
            .cloned()
            .collect(),
    }
}

fn commits(rung: &mut dyn Rung, batch: &DeltaBatch, law: Law) -> Digest32 {
    let name = rung.name();
    rung.commit(batch, law)
        .unwrap_or_else(|e| panic!("{name}: expected a commit, got {e}"))
}

fn refuses(rung: &mut dyn Rung, batch: &DeltaBatch, law: Law, code: &str) {
    let before = observe(rung);
    let name = rung.name();
    match rung.commit(batch, law) {
        Ok(_) => panic!("{name}: expected {code}, the batch committed"),
        Err(e) => assert_eq!(e.to_refusal().code, code, "{name}: {e}"),
    }
    assert_eq!(observe(rung), before, "{name}: a refusal changed the state");
}

type Case = fn(&mut dyn Rung);

fn history_is_append_only(r: &mut dyn Rung) {
    let first = next(r, 1, vec![op("a", DeltaKind::Put, b"1")], Vec::new());
    commits(r, &first, no_law);
    let second = next(r, 1, vec![op("a", DeltaKind::Supersede, b"2")], Vec::new());
    commits(r, &second, no_law);
    // Superseding with bytes already held is a new version.
    let third = next(r, 1, vec![op("a", DeltaKind::Supersede, b"2")], Vec::new());
    commits(r, &third, no_law);
    // A key put and superseded in one batch holds both versions at that seq.
    let both = next(
        r,
        1,
        vec![
            op("b", DeltaKind::Put, b"x"),
            op("b", DeltaKind::Supersede, b"y"),
        ],
        Vec::new(),
    );
    commits(r, &both, no_law);
    let state = r.state();
    let a: Vec<&[u8]> = state
        .key_history(CELL, "a")
        .unwrap()
        .iter()
        .map(|v| v.payload.as_slice())
        .collect();
    assert_eq!(a, [b"1".as_slice(), b"2", b"2"], "{}", r.name());
    let b = state.key_history(CELL, "b").unwrap();
    assert_eq!((b.len(), b[0].seq, b[1].seq), (2, 3, 3), "{}", r.name());
    assert_eq!(
        state.as_of_seq(CELL, 1).unwrap().live_keys["a"].payload,
        b"1",
        "{}",
        r.name()
    );
    assert!(
        !state
            .as_of_seq(CELL, 3)
            .unwrap()
            .live_keys
            .contains_key("b")
    );
    assert_eq!(
        state.as_of_seq(CELL, 4).unwrap().live_keys["b"].payload,
        b"y"
    );
}

fn a_held_key_cannot_be_put_again(r: &mut dyn Rung) {
    let first = next(r, 1, vec![op("a", DeltaKind::Put, b"1")], Vec::new());
    commits(r, &first, no_law);
    let again = next(r, 1, vec![op("a", DeltaKind::Put, b"2")], Vec::new());
    refuses(r, &again, no_law, "FMINE-REF-502");
    let twice = next(
        r,
        1,
        vec![op("b", DeltaKind::Put, b"1"), op("b", DeltaKind::Put, b"2")],
        Vec::new(),
    );
    refuses(r, &twice, no_law, "FMINE-REF-502");
}

fn an_absent_key_cannot_be_superseded(r: &mut dyn Rung) {
    let absent = next(r, 1, vec![op("a", DeltaKind::Supersede, b"1")], Vec::new());
    refuses(r, &absent, no_law, "FMINE-REF-201");
    // Nothing from a refused batch is applied, including its valid ops.
    let mixed = next(
        r,
        1,
        vec![
            op("a", DeltaKind::Put, b"1"),
            op("b", DeltaKind::Supersede, b"2"),
        ],
        Vec::new(),
    );
    refuses(r, &mixed, no_law, "FMINE-REF-201");
}

fn an_obligation_opens_once_and_resolves_once(r: &mut dyn Rung) {
    let reserve = next(
        r,
        1,
        vec![op("OB-1", DeltaKind::ReserveObligation, b"")],
        Vec::new(),
    );
    commits(r, &reserve, no_law);
    assert!(r.state().open_obligations(CELL).unwrap().contains("OB-1"));
    let again = next(
        r,
        1,
        vec![op("OB-1", DeltaKind::ReserveObligation, b"")],
        Vec::new(),
    );
    refuses(r, &again, no_law, "FMINE-REF-502");
    let resolve = next(
        r,
        1,
        vec![op("OB-1", DeltaKind::ResolveObligation, b"")],
        Vec::new(),
    );
    commits(r, &resolve, no_law);
    let resolve_again = next(
        r,
        1,
        vec![op("OB-1", DeltaKind::ResolveObligation, b"")],
        Vec::new(),
    );
    refuses(r, &resolve_again, no_law, "FMINE-REF-502");
    let never_reserved = next(
        r,
        1,
        vec![op("OB-2", DeltaKind::ResolveObligation, b"")],
        Vec::new(),
    );
    refuses(r, &never_reserved, no_law, "FMINE-REF-502");
    // Reserved and resolved in one batch: committed, not open.
    let both = next(
        r,
        1,
        vec![
            op("OB-3", DeltaKind::ReserveObligation, b""),
            op("OB-3", DeltaKind::ResolveObligation, b""),
        ],
        Vec::new(),
    );
    commits(r, &both, no_law);
    assert!(
        r.state().open_obligations(CELL).unwrap().is_empty(),
        "{}",
        r.name()
    );
    assert!(
        r.state()
            .as_of_seq(CELL, 1)
            .unwrap()
            .open_obligations
            .contains("OB-1")
    );
}

fn a_read_holds_only_while_its_key_is_unchanged(r: &mut dyn Rung) {
    let absent = witness(r, "a");
    let first = next(
        r,
        1,
        vec![op("a", DeltaKind::Put, b"1")],
        vec![absent.clone()],
    );
    commits(r, &first, no_law);
    // The absence it read is gone.
    let stale = next(r, 1, vec![op("c", DeltaKind::Put, b"1")], vec![absent]);
    refuses(r, &stale, no_law, "FMINE-REF-502");
    let read = witness(r, "a");
    // An unrelated write keeps the read valid.
    let unrelated = next(r, 1, vec![op("b", DeltaKind::Put, b"1")], Vec::new());
    commits(r, &unrelated, no_law);
    let update = next(
        r,
        1,
        vec![op("a", DeltaKind::Supersede, b"1")],
        vec![read.clone()],
    );
    commits(r, &update, no_law);
    // A supersede with the same bytes still invalidates the read.
    let after = next(r, 1, vec![op("a", DeltaKind::Supersede, b"2")], vec![read]);
    refuses(r, &after, no_law, "FMINE-REF-502");
}

fn the_chain_admits_only_the_next_batch(r: &mut dyn Rung) {
    let first = next(r, 1, vec![op("a", DeltaKind::Put, b"1")], Vec::new());
    let head = commits(r, &first, no_law);
    let skip = batch_at(2, head, 1, vec![op("b", DeltaKind::Put, b"1")], Vec::new());
    refuses(r, &skip, no_law, "FMINE-REF-505");
    let wrong_prev = batch_at(
        1,
        Digest32([7; 32]),
        1,
        vec![op("b", DeltaKind::Put, b"1")],
        Vec::new(),
    );
    refuses(r, &wrong_prev, no_law, "FMINE-REF-505");
    refuses(r, &first, no_law, "FMINE-REF-505");
    let second = next(r, 1, vec![op("b", DeltaKind::Put, b"1")], Vec::new());
    commits(r, &second, no_law);
}

fn a_law_reads_the_ledger_before_the_commit(r: &mut dyn Rung) {
    for (kind, payload) in [(DeltaKind::Put, b"1"), (DeltaKind::Supersede, b"2")] {
        let write = next(r, 1, vec![op("a", kind, payload)], Vec::new());
        commits(r, &write, at_most_two_versions);
    }
    let third = next(r, 1, vec![op("a", DeltaKind::Supersede, b"3")], Vec::new());
    refuses(r, &third, at_most_two_versions, "FMINE-REF-101");
    // The same batch is admitted without the law.
    commits(r, &third, no_law);
    let other = next(r, 1, vec![op("b", DeltaKind::Put, b"1")], Vec::new());
    commits(r, &other, at_most_two_versions);
}

fn an_older_epoch_is_fenced(r: &mut dyn Rung) {
    let first = next(r, 1, vec![op("a", DeltaKind::Put, b"1")], Vec::new());
    commits(r, &first, no_law);
    r.new_epoch();
    let moved = next(r, 2, vec![op("b", DeltaKind::Put, b"1")], Vec::new());
    commits(r, &moved, no_law);
    let old = next(r, 1, vec![op("c", DeltaKind::Put, b"1")], Vec::new());
    refuses(r, &old, no_law, "FMINE-REF-502");
    let state = r.state();
    assert_eq!(state.key_history(CELL, "a").unwrap()[0].epoch, 1);
    assert_eq!(state.key_history(CELL, "b").unwrap()[0].epoch, 2);
    assert_eq!(
        state.as_of_seq(CELL, 2).unwrap().head_epoch,
        2,
        "{}",
        r.name()
    );
}

#[test]
fn every_rung_passes_the_shared_semantic_suite() {
    let cases: [(&str, Case); 8] = [
        ("history_is_append_only", history_is_append_only),
        (
            "a_held_key_cannot_be_put_again",
            a_held_key_cannot_be_put_again,
        ),
        (
            "an_absent_key_cannot_be_superseded",
            an_absent_key_cannot_be_superseded,
        ),
        (
            "an_obligation_opens_once_and_resolves_once",
            an_obligation_opens_once_and_resolves_once,
        ),
        (
            "a_read_holds_only_while_its_key_is_unchanged",
            a_read_holds_only_while_its_key_is_unchanged,
        ),
        (
            "the_chain_admits_only_the_next_batch",
            the_chain_admits_only_the_next_batch,
        ),
        (
            "a_law_reads_the_ledger_before_the_commit",
            a_law_reads_the_ledger_before_the_commit,
        ),
        ("an_older_epoch_is_fenced", an_older_epoch_is_fenced),
    ];
    for (name, case) in cases {
        let mut ran = rungs(name);
        for rung in &mut ran {
            case(rung.as_mut());
        }
        // The rungs end in one state, which the reopened file reproduces.
        let oracle = observe(ran[0].as_ref());
        for rung in &ran[1..] {
            assert_eq!(
                observe(rung.as_ref()),
                oracle,
                "{name}: {} differs from L0",
                rung.name()
            );
        }
    }
}

/// xorshift64: a fixed seed gives the same histories on every platform.
struct Rng(u64);

impl Rng {
    fn below(&mut self, n: usize) -> usize {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 % n as u64) as usize
    }
}

#[test]
fn l0_and_l1_agree_on_random_histories() {
    const HISTORIES: usize = 150;
    const STEPS: usize = 16;
    let mut rng = Rng(0x0bd0_8700_5eed_1e55);
    let mut tally = std::collections::BTreeMap::<String, usize>::new();
    for history in 0..HISTORIES {
        let mut l0 = L0Rung::new();
        let mut l1 = L1Rung::new(&format!("random-{history}"), false);
        let mut epoch = 1;
        let mut reads: Vec<Witness> = Vec::new();
        for step in 0..STEPS {
            let at = format!("history {history}, step {step}");
            let (batch, law): (DeltaBatch, Law) = if rng.below(16) == 0 {
                l0.new_epoch();
                l1.new_epoch();
                epoch += 1;
                let key = format!("epoch-{epoch}");
                (
                    next(&l0, epoch, vec![op(&key, DeltaKind::Put, b"")], Vec::new()),
                    no_law,
                )
            } else {
                let mut deltas = Vec::new();
                for _ in 0..=rng.below(3) {
                    if rng.below(3) == 0 {
                        let key = OBLIGATIONS[rng.below(OBLIGATIONS.len())];
                        let kind = if rng.below(2) == 0 {
                            DeltaKind::ReserveObligation
                        } else {
                            DeltaKind::ResolveObligation
                        };
                        deltas.push(op(key, kind, b""));
                    } else {
                        let key = KEYS[rng.below(KEYS.len())];
                        let kind = if rng.below(2) == 0 {
                            DeltaKind::Put
                        } else {
                            DeltaKind::Supersede
                        };
                        deltas.push(op(key, kind, &[b'0' + rng.below(3) as u8]));
                    }
                }
                let mut witnesses = Vec::new();
                for _ in 0..rng.below(3) {
                    if !reads.is_empty() && rng.below(4) == 0 {
                        witnesses.push(reads[rng.below(reads.len())].clone());
                    } else {
                        let read = witness(&l0, KEYS[rng.below(KEYS.len())]);
                        reads.push(read.clone());
                        witnesses.push(read);
                    }
                }
                let mut batch = next(&l0, epoch, deltas, witnesses);
                match rng.below(30) {
                    0 => {
                        batch = batch_at(
                            batch.seq + 1,
                            batch.prev_hash,
                            epoch,
                            batch.deltas,
                            batch.witnesses,
                        )
                    }
                    1 => {
                        batch = batch_at(
                            batch.seq,
                            Digest32([9; 32]),
                            epoch,
                            batch.deltas,
                            batch.witnesses,
                        )
                    }
                    2 if epoch > 1 => {
                        batch = batch_at(
                            batch.seq,
                            batch.prev_hash,
                            epoch - 1,
                            batch.deltas,
                            batch.witnesses,
                        );
                    }
                    _ => {}
                }
                let law = if rng.below(2) == 0 {
                    at_most_two_versions
                } else {
                    no_law
                };
                (batch, law)
            };
            let oracle = l0.commit(&batch, law);
            let durable = l1.commit(&batch, law);
            let outcome = match (&oracle, &durable) {
                (Ok(x), Ok(y)) => {
                    assert_eq!(x, y, "{at}: digests differ");
                    "committed".to_string()
                }
                (Err(x), Err(y)) => {
                    assert_eq!(
                        (x.to_refusal().code, x.to_string()),
                        (y.to_refusal().code, y.to_string()),
                        "{at}"
                    );
                    x.to_refusal().code.to_string()
                }
                _ => panic!("{at}: L0 {oracle:?}, L1 {durable:?} for {batch:?}"),
            };
            *tally.entry(outcome).or_default() += 1;
            assert_eq!(observe(&l1), observe(&l0), "{at}: state differs");
            if rng.below(5) == 0 {
                l1.reopen();
                assert_eq!(
                    observe(&l1),
                    observe(&l0),
                    "{at}: state differs after reopening L1"
                );
            }
        }
    }
    println!("{tally:?}");
    // The histories reach every outcome the suite names, so agreement is not vacuous.
    for outcome in [
        "committed",
        "FMINE-REF-101",
        "FMINE-REF-201",
        "FMINE-REF-502",
        "FMINE-REF-505",
    ] {
        assert!(
            tally.get(outcome).copied().unwrap_or(0) >= 20,
            "{outcome}: {tally:?}"
        );
    }
}
