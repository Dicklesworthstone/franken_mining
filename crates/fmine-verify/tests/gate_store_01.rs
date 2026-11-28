//! GATE-STORE-01 (`BD-098`, plan §26.12): ledger → fsqlite projection rebuild of 10⁵ holes / 10⁷ intervals in
//! ≤ 15 min; as-known-at and bbox answers from one anchor.
//!
//! The rebuild is [`drill_rebuild`]'s, shared with GATE-CORE-01: the generator's campaign (one interval, half-core
//! sample and gold result per metre; collar, gyro station, dispatch and certificate per hole), bridged to ledger
//! records and written to sealed `FMT-SEG-1` segments (not timed), then rebuilt from them into a sharded fsqlite
//! drill projection with deferred indexes, each hole desurveyed as its batch is applied. When the projection stands
//! at the last campaign batch, before a re-assay batch, the R-tree is loaded on every shard and one
//! `Projection::answer` reads, stamped with that anchor:
//! - the checked holes' records (and the composites made from them), against the generator before the re-assay;
//! - box queries: a column around each checked collar returns exactly that hole's intervals, and a point where the
//!   generator places an interval's midpoint returns exactly that interval.
//!
//! The re-assay batch is then applied with the indexes kept current. The rebuild is timed from opening the first
//! segment to the head with its R-tree current. After it the head is checked: records with the re-assay applied,
//! version counts, the same box queries, and a box around the whole campaign that returns every interval once.
//!
//! The scale run is ignored by default; run it in release:
//!
//! ```text
//! cargo test --release -p fmine-verify --test gate_store_01 -- --ignored --nocapture
//! ```
//!
//! `FMINE_GATE_STORE_HOLES` sets the hole count (default 100 000), `FMINE_GATE_STORE_INTERVALS` the intervals per
//! hole (default 100), `FMINE_GATE_STORE_SHARDS` the shard stores (default 8), `FMINE_GATE_STORE_DIR` the
//! directory for the segments and the store (default the temp dir) and `FMINE_GATE_STORE_OUT` a file for the
//! measured numbers as JSON lines.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]

mod drill_rebuild;

use drill_rebuild::{Gate, scale_run, small_run};

/// The pipeline at a small size (`drill_rebuild::small_run`): the R-tree is loaded before the anchor's answer,
/// whose box queries would refuse on a stale index, and no hole is composited in bulk.
#[test]
fn test_a_small_campaign_answers_records_and_boxes_from_the_anchor() {
    let lines = small_run(Gate::Store);
    let at = |phase: &str| {
        lines
            .iter()
            .position(|l| l.contains(&format!("\"phase\":\"{phase}\"")))
            .unwrap_or_else(|| panic!("{phase}: {lines:?}"))
    };
    let order = ["indexes", "feed", "rebuild", "head"].map(at);
    assert!(order.is_sorted(), "{lines:?}");
    assert!(
        !lines.iter().any(|l| l.contains("\"phase\":\"composite\"")),
        "{lines:?}"
    );
}

#[test]
#[ignore = "scale run: 10^7 intervals in release; see the module documentation"]
fn test_gate_store_01_full_rebuild() {
    scale_run(Gate::Store, 100);
}
