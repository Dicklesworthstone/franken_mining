#![allow(clippy::expect_used, clippy::unwrap_used)]
//! End-to-end `BD-147` (`PO-BED-6`, `INV-005`) tests wiring `fmine-flow`'s exact extensive
//! conservation law directly into `fmine-ledger` on BOTH L0 (`L0Ledger::commit_with_conservation`)
//! and L1 (`SemanticSegmentWriter::append_with_law` and `commit_prepared_with_law`).

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use fmine_core::anchor::{FMT_ANCHOR_V1, LedgerAnchor};
use fmine_core::hash::{Digest32, sha256};
use fmine_flow::balance::{
    BalanceTolerance, ExtensiveQuantities, InventoryDelta, PeriodBalanceRecord, StreamFlow,
    conservation_commit_law, conservation_witness, verify_period_balance,
};
use fmine_flow::graph::{FlowGraph, FlowNode, FlowStream, NodeClass, StockpileMixingModel};
use fmine_ledger::batch::{DeltaBatch, DeltaKind, DeltaOp, FMT_LEDGER_V2, Witness, WitnessKind};
use fmine_ledger::l0::{L0Error, L0Ledger};
use fmine_ledger::segment::SemanticSegmentWriter;
use fmine_units::ExactQty;

const CELL_FLOW: &str = "CELL-FLOW(lumwana)";

fn qty(dry: i128, water: i128, au: i128) -> ExtensiveQuantities {
    let mut elems = BTreeMap::new();
    elems.insert("Au".to_string(), ExactQty::from_integer(au));
    ExtensiveQuantities::new(
        ExactQty::from_integer(dry),
        ExactQty::from_integer(water),
        elems,
    )
    .expect("extensive qty")
}

fn site_graph() -> FlowGraph {
    FlowGraph::build(
        vec![
            FlowNode::new("pit", "Pit", NodeClass::BlastedAndDug, false, None).expect("pit"),
            FlowNode::new(
                "rom",
                "ROM Pad",
                NodeClass::StockpileOrPad,
                true,
                Some(StockpileMixingModel::WellMixed),
            )
            .expect("rom"),
            FlowNode::new("mill", "Mill", NodeClass::PlantUnit, true, None).expect("mill"),
            FlowNode::new("dore", "Dore", NodeClass::ProductMarket, false, None).expect("dore"),
            FlowNode::new("tsf", "TSF", NodeClass::WasteAndTailings, false, None).expect("tsf"),
        ],
        vec![
            FlowStream::new("s1", "pit", "rom", true, ["Au"]).expect("s1"),
            FlowStream::new("s2", "rom", "mill", true, ["Au"]).expect("s2"),
            FlowStream::new("s3", "mill", "dore", false, ["Au"]).expect("s3"),
            FlowStream::new("s4", "mill", "tsf", true, ["Au"]).expect("s4"),
        ],
    )
    .expect("graph")
}

fn build_batch(
    cell_id: &str,
    epoch: u64,
    seq: u64,
    prev_hash: Digest32,
    mut deltas: Vec<DeltaOp>,
    mut witnesses: Vec<Witness>,
) -> DeltaBatch {
    deltas.sort();
    witnesses.sort();
    DeltaBatch {
        format_version: FMT_LEDGER_V2,
        cell_id: cell_id.to_string(),
        epoch,
        seq,
        prev_hash,
        anchor: LedgerAnchor {
            format_version: FMT_ANCHOR_V1,
            cell_id: cell_id.to_string(),
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

fn scratch_seg(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("fmine-flow-bd147-{tag}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("mkdir");
    dir.join("cell_flow.fmseg")
}

fn balanced_period_1() -> PeriodBalanceRecord {
    // ROM: in 1000/50/2000, out 800/40/1600, open 500/25/1000 -> close 700/35/1400 (Δ=+200/+10/+400)
    // Mill: in 800/40/1600, out dore 0/0/1440 + tsf 800/40/160, Δ=0
    PeriodBalanceRecord {
        period_id: "2026-01".to_string(),
        previous_period_id: None,
        flows: vec![
            StreamFlow {
                stream_id: "s1".into(),
                quantities: qty(1000, 50, 2000),
            },
            StreamFlow {
                stream_id: "s2".into(),
                quantities: qty(800, 40, 1600),
            },
            StreamFlow {
                stream_id: "s3".into(),
                quantities: qty(0, 0, 1440),
            },
            StreamFlow {
                stream_id: "s4".into(),
                quantities: qty(800, 40, 160),
            },
        ],
        inventories: vec![InventoryDelta {
            node_id: "rom".into(),
            opening: qty(500, 25, 1000),
            closing: qty(700, 35, 1400),
        }],
    }
}

fn balanced_period_2() -> PeriodBalanceRecord {
    // Continues exactly from period 1 closing (700/35/1400):
    // ROM: in 900/45/1800, out 1000/50/2000, open 700/35/1400 -> close 600/30/1200 (Δ=-100/-5/-200)
    // Mill: in 1000/50/2000, out dore 0/0/1800 + tsf 1000/50/200, Δ=0
    PeriodBalanceRecord {
        period_id: "2026-02".to_string(),
        previous_period_id: Some("2026-01".to_string()),
        flows: vec![
            StreamFlow {
                stream_id: "s1".into(),
                quantities: qty(900, 45, 1800),
            },
            StreamFlow {
                stream_id: "s2".into(),
                quantities: qty(1000, 50, 2000),
            },
            StreamFlow {
                stream_id: "s3".into(),
                quantities: qty(0, 0, 1800),
            },
            StreamFlow {
                stream_id: "s4".into(),
                quantities: qty(1000, 50, 200),
            },
        ],
        inventories: vec![InventoryDelta {
            node_id: "rom".into(),
            opening: qty(700, 35, 1400),
            closing: qty(600, 30, 1200),
        }],
    }
}

#[test]
fn test_po_bed_6_conservation_at_commit_on_l0_and_l1_refuses_plug_and_preserves_state() {
    let graph = site_graph();
    let tol = BalanceTolerance::exact_zero();
    let law = conservation_commit_law(&graph, &tol);

    // Prepare L0 in-memory ledger and L1 durable segment writer in lockstep.
    let mut l0 = L0Ledger::new();
    let lease = l0
        .create_cell(CELL_FLOW, "writer-lumwana")
        .expect("create L0 cell");
    let seg_path = scratch_seg("l0_l1");
    let mut l1 = SemanticSegmentWriter::create(&seg_path, CELL_FLOW).expect("create L1 segment");

    // 1. Commit exact closed Period 1 with cryptographic conservation witness on BOTH L0 and L1.
    let p1 = balanced_period_1();
    let p1_rep = verify_period_balance(&graph, &p1.flows, &p1.inventories, &tol).expect("p1 ok");
    let p1_wit = conservation_witness(&p1.period_id, &p1_rep).expect("p1 witness");
    let p1_batch = build_batch(
        CELL_FLOW,
        lease.epoch,
        0,
        l0.head_hash(CELL_FLOW).expect("head 0"),
        vec![p1.into_delta_op(DeltaKind::Put).expect("p1 op")],
        vec![p1_wit],
    );
    let d1_l0 = l0
        .commit_with_conservation(&lease, p1_batch.clone(), &law)
        .expect("L0 commit p1");
    let d1_l1 = l1
        .append_with_law(&p1_batch, &law)
        .expect("L1 durable commit p1");
    assert_eq!(d1_l0, d1_l1, "L0 and L1 must produce bit-identical digests");
    let seg_len_after_p1 = fs::metadata(&seg_path).expect("meta").len();

    // 2. Period 2 with +1 g Au conservation plug violation refuses with FMINE-REF-101 (exit 3)
    //    on BOTH L0 and L1, leaving zero state mutated and zero bytes appended on disk.
    let mut unplugged_p2 = balanced_period_2();
    unplugged_p2.flows[2].quantities = qty(0, 0, 1801);
    let bad_p2_batch = build_batch(
        CELL_FLOW,
        lease.epoch,
        1,
        d1_l0,
        vec![unplugged_p2.into_delta_op(DeltaKind::Put).expect("bad op")],
        Vec::new(),
    );
    let err_l0 = l0
        .commit_with_conservation(&lease, bad_p2_batch.clone(), &law)
        .expect_err("L0 must refuse conservation violation");
    assert!(matches!(err_l0, L0Error::ConservationViolation { .. }));
    assert_eq!(err_l0.to_refusal().code, "FMINE-REF-101");
    assert_eq!(err_l0.to_refusal().exit_code, 3);

    let err_l1 = l1
        .append_with_law(&bad_p2_batch, &law)
        .expect_err("L1 must refuse conservation violation");
    assert!(matches!(err_l1, L0Error::ConservationViolation { .. }));
    assert_eq!(err_l1.to_refusal().code, "FMINE-REF-101");
    assert_eq!(err_l1.to_refusal().exit_code, 3);
    assert_eq!(l0.committed_count(CELL_FLOW).expect("count"), 1);
    assert_eq!(l1.ledger().committed_count(CELL_FLOW).expect("l1 count"), 1);
    assert_eq!(
        fs::metadata(&seg_path).expect("meta").len(),
        seg_len_after_p1,
        "refused conservation batch must append zero bytes to L1 segment"
    );

    // 3. Internally balanced Period 2 that breaks cross-period stockpile continuity (opening 705
    //    instead of Period 1 closing 700, hidden +5 t plug) refuses FMINE-REF-101 on BOTH rungs!
    let mut broken_continuity = balanced_period_2();
    broken_continuity.inventories[0].opening = qty(705, 35, 1400);
    broken_continuity.inventories[0].closing = qty(605, 30, 1200);
    let cont_batch = build_batch(
        CELL_FLOW,
        lease.epoch,
        1,
        d1_l0,
        vec![
            broken_continuity
                .into_delta_op(DeltaKind::Put)
                .expect("cont op"),
        ],
        Vec::new(),
    );
    assert_eq!(
        l0.commit_with_conservation(&lease, cont_batch.clone(), &law)
            .expect_err("L0 continuity break must refuse")
            .to_refusal()
            .code,
        "FMINE-REF-101"
    );
    assert_eq!(
        l1.append_with_law(&cont_batch, &law)
            .expect_err("L1 continuity break must refuse")
            .to_refusal()
            .code,
        "FMINE-REF-101"
    );

    // 4. Forged WitnessKind::Conservation digest refuses FMINE-REF-101 on BOTH rungs.
    let p2 = balanced_period_2();
    let p2_rep = verify_period_balance(&graph, &p2.flows, &p2.inventories, &tol).expect("p2 ok");
    let p2_wit = conservation_witness(&p2.period_id, &p2_rep).expect("p2 witness");
    let forged_wit = Witness::new(
        WitnessKind::Conservation,
        p2.delta_key(),
        sha256(b"forged-witness"),
    );
    let forged_batch = build_batch(
        CELL_FLOW,
        lease.epoch,
        1,
        d1_l0,
        vec![p2.into_delta_op(DeltaKind::Put).expect("p2 op")],
        vec![forged_wit],
    );
    assert_eq!(
        l0.commit_with_conservation(&lease, forged_batch.clone(), &law)
            .expect_err("forged witness L0")
            .to_refusal()
            .code,
        "FMINE-REF-101"
    );
    assert_eq!(
        l1.append_with_law(&forged_batch, &law)
            .expect_err("forged witness L1")
            .to_refusal()
            .code,
        "FMINE-REF-101"
    );

    // 5. Corrupt/non-FLWB payload under flow/period/ cannot silently bypass conservation.
    let garbage_batch = build_batch(
        CELL_FLOW,
        lease.epoch,
        1,
        d1_l0,
        vec![DeltaOp::new(
            "flow/period/2026-02",
            DeltaKind::Put,
            b"not-a-valid-flwb-frame".to_vec(),
        )],
        Vec::new(),
    );
    assert_eq!(
        l0.commit_with_conservation(&lease, garbage_batch.clone(), &law)
            .expect_err("garbage payload L0")
            .to_refusal()
            .code,
        "FMINE-REF-101"
    );
    assert_eq!(
        l1.append_with_law(&garbage_batch, &law)
            .expect_err("garbage payload L1")
            .to_refusal()
            .code,
        "FMINE-REF-101"
    );

    // 6. Valid Period 2 commits identically on L0 and L1 and replays cleanly from disk!
    let p2_valid = build_batch(
        CELL_FLOW,
        lease.epoch,
        1,
        d1_l0,
        vec![p2.into_delta_op(DeltaKind::Put).expect("p2 op")],
        vec![p2_wit],
    );
    let d2_l0 = l0
        .commit_with_conservation(&lease, p2_valid.clone(), &law)
        .expect("L0 commit p2");
    let d2_l1 = l1.append_with_law(&p2_valid, &law).expect("L1 commit p2");
    assert_eq!(d2_l0, d2_l1);
    drop(l1);

    let reopened = SemanticSegmentWriter::open_append(&seg_path).expect("reopen L1");
    assert_eq!(
        reopened.ledger().committed_count(CELL_FLOW).expect("cnt"),
        2
    );
    assert_eq!(reopened.ledger().head_hash(CELL_FLOW).expect("head"), d2_l0);
    let stored = reopened
        .ledger()
        .key_history(CELL_FLOW, "flow/period/2026-02")
        .expect("p2 history");
    assert_eq!(
        PeriodBalanceRecord::from_bytes(&stored[0].payload).expect("decode p2"),
        p2
    );

    let _ = fs::remove_dir_all(seg_path.parent().expect("parent"));
}

#[test]
fn test_po_bed_6_conservation_enforced_under_l1_narrow_combiner_rebase() {
    let graph = site_graph();
    let tol = BalanceTolerance::exact_zero();
    let law = conservation_commit_law(&graph, &tol);

    let seg_path = scratch_seg("rebase");
    let mut l1 = SemanticSegmentWriter::create(&seg_path, CELL_FLOW).expect("create L1");

    // Seq 0: commit Period 1.
    let p1 = balanced_period_1();
    let b0 = build_batch(
        CELL_FLOW,
        1,
        0,
        l1.ledger().head_hash(CELL_FLOW).expect("genesis"),
        vec![p1.into_delta_op(DeltaKind::Put).expect("p1 op")],
        Vec::new(),
    );
    let d0 = l1.append_with_law(&b0, &law).expect("commit seq 0");
    let p1_witness = l1
        .ledger()
        .key_witness(CELL_FLOW, "flow/period/2026-01")
        .expect("p1 witness");

    // Seq 1: concurrent unrelated metadata commit advances head from seq 0 -> seq 1.
    let b1 = build_batch(
        CELL_FLOW,
        1,
        1,
        d0,
        vec![DeltaOp::new(
            "meta/site_note",
            DeltaKind::Put,
            b"shift-handover".to_vec(),
        )],
        Vec::new(),
    );
    l1.append_with_law(&b1, &law).expect("commit meta seq 1");

    // An unbalanced Period 2 prepared against seq 0 MUST refuse FMINE-REF-101 during rebase!
    let mut bad_p2 = balanced_period_2();
    bad_p2.flows[0].quantities = qty(901, 45, 1800);
    let prepared_bad = build_batch(
        CELL_FLOW,
        1,
        1,
        d0,
        vec![bad_p2.into_delta_op(DeltaKind::Put).expect("bad p2")],
        vec![p1_witness.clone()],
    );
    let err = l1
        .commit_prepared_with_law(&prepared_bad, &law)
        .expect_err("unbalanced rebase must refuse");
    assert_eq!(err.to_refusal().code, "FMINE-REF-101");
    assert_eq!(l1.ledger().committed_count(CELL_FLOW).expect("cnt"), 2);

    // Valid balanced Period 2 prepared against seq 0 rebases cleanly onto seq 2!
    let p2 = balanced_period_2();
    let prepared_ok = build_batch(
        CELL_FLOW,
        1,
        1,
        d0,
        vec![p2.into_delta_op(DeltaKind::Put).expect("ok p2")],
        vec![p1_witness],
    );
    let (_d2, committed) = l1
        .commit_prepared_with_law(&prepared_ok, &law)
        .expect("balanced rebase commits");
    assert_eq!(committed.seq, 2);
    assert_eq!(l1.ledger().committed_count(CELL_FLOW).expect("cnt"), 3);

    let _ = fs::remove_dir_all(seg_path.parent().expect("parent"));
}
