//! GATE-CORE-01 (`BD-173`, plan §12): 10⁵ holes / 2×10⁷ intervals ingested, desurveyed and composited; full
//! rebuild ≤ 15 min; as-known-at query from one anchor.
//!
//! The rebuild is [`drill_rebuild`]'s, shared with GATE-STORE-01: the generator's campaign, bridged to ledger
//! records and written to sealed `FMT-SEG-1` segments (not timed), then rebuilt from them into a sharded drill
//! projection with deferred indexes. When the projection stands at the last campaign batch, before a re-assay
//! batch, the checked holes' records and composites are read there, stamped with that anchor, and checked against
//! the generator. Then the re-assay, the R-tree load, and a pass on several threads that reads every hole's records
//! from the store and composites them to 2 m within lithology runs (length-weighted; `composite` refuses a hole
//! whose composites do not conserve its inputs). The rebuild is timed from opening the first segment to the last
//! composite. After it the head is checked: records and composites with the re-assay applied, version counts, and
//! desurvey through column, midpoint and whole-campaign box queries.
//!
//! The scale run is ignored by default; run it in release:
//!
//! ```text
//! cargo test --release -p fmine-verify --test gate_core_01 -- --ignored --nocapture
//! ```
//!
//! `FMINE_GATE_CORE_HOLES` sets the hole count (default 100 000), `FMINE_GATE_CORE_INTERVALS` the intervals per
//! hole (default 200), `FMINE_GATE_CORE_SHARDS` the shard stores (default 8), `FMINE_GATE_CORE_THREADS` the
//! compositing threads (default 16), `FMINE_GATE_CORE_DIR` the directory for the segments and the store (default
//! the temp dir) and `FMINE_GATE_CORE_OUT` a file for the measured numbers as JSON lines.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]

mod drill_rebuild;

use drill_rebuild::{Gate, campaign, cell, generated, hole_changes, reassay, scale_run, small_run};
use fmine_drill::records::{AssayResult, AssayValue, DrillRecord};
use fmine_drill::store::Change;
use fmine_gen::drillholes::DrillholeCampaignConfig;

/// The bridged records are ones a ledger takes: three holes of ten metres, and the re-assay of one of them, commit
/// through `drill_law`, which checks every record and that each one's references exist.
#[test]
fn test_the_bridged_campaign_commits_through_drill_law() {
    let cfg = campaign(Gate::Core, 3, 10);
    let changes = hole_changes(&cfg, 0, 3);
    assert_eq!(changes.len(), 3 * (4 + 3 * 10));
    let mut ledger = fmine_ledger::l0::L0Ledger::new();
    let lease = ledger.create_cell(cell(&cfg), "node-1").unwrap();
    fmine_drill::store::commit(&mut ledger, &lease, &changes).unwrap();
    let below = changes
        .iter()
        .filter(|c| {
            matches!(
                c,
                Change::Put(DrillRecord::AssayResult(AssayResult {
                    value: AssayValue::BelowDetection { .. },
                    ..
                }))
            )
        })
        .count();
    assert!(below < 30, "{below} of 30 results below detection");
    let again = reassay(&cfg, 1);
    assert_eq!(again.len(), 10 - changes_below(&cfg, 1));
    fmine_drill::store::commit(&mut ledger, &lease, &again).unwrap();
}

/// Results of hole `h` below detection.
fn changes_below(cfg: &DrillholeCampaignConfig, h: u64) -> usize {
    generated(cfg, h, false)
        .results
        .iter()
        .filter(|r| matches!(r.value, AssayValue::BelowDetection { .. }))
        .count()
}

/// One hole generated alone is the same as within its batch, so a hole's records can be regenerated to check it.
#[test]
fn test_a_hole_generated_alone_is_the_hole_of_its_batch() {
    let cfg = campaign(Gate::Core, 30, 12);
    let batch = hole_changes(&cfg, 10, 10);
    let alone: Vec<Change> = (10..20).flat_map(|h| hole_changes(&cfg, h, 1)).collect();
    assert_eq!(batch, alone);
}

/// The whole pipeline at a small size (`drill_rebuild::small_run`): every hole is checked as known at the anchor
/// and at the head, desurvey included, and every hole is composited.
#[test]
fn test_a_small_campaign_rebuilds_from_segments_and_answers_as_known_at() {
    let lines = small_run(Gate::Core);
    let at = |phase: &str| {
        lines
            .iter()
            .position(|l| l.contains(&format!("\"phase\":\"{phase}\"")))
            .unwrap_or_else(|| panic!("{phase}: {lines:?}"))
    };
    let order = ["feed", "indexes", "composite", "rebuild", "head"].map(at);
    assert!(order.is_sorted(), "{lines:?}");
}

#[test]
#[ignore = "scale run: 2x10^7 intervals in release; see the module documentation"]
fn test_gate_core_01_full_rebuild() {
    scale_run(Gate::Core, 200);
}
