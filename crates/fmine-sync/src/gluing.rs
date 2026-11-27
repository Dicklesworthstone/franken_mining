//! Determinism: BitwisePortable
//!
//! Cross-cell anchor gluing gate (`BD-108`, plan §26.3, `INV-012`, `F12`).
//!
//! Before an enterprise anchor publishes a dependent Claim referencing other cells, every
//! `(cell_id, seq, digest)` cross-cell reference must resolve in the held enterprise anchor view.
//! A missing cell yields `KnowledgeState::NotObservable`; a behind/mismatched reference yields
//! `KnowledgeState::Stale`; either blocks publication.

use std::collections::BTreeMap;

use fmine_core::hash::Digest32;
use fmine_evidence::{KnowledgeState, Refusal, RefusalCode};
use fmine_ledger::batch::DeltaBatch;
use fmine_ledger::cell::{GluingIssue, GluingReport, check_gluing};

/// Outcome of the anchor-publish gluing check (`INV-012`, `F12`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GluingGateDecision {
    /// All cross-cell references resolved cleanly; publication may proceed.
    PublishAllowed { checked_refs: usize },
    /// One or more references dangled; publication is blocked (`INV-012`, `F12`).
    PublishBlocked {
        consumer_state: KnowledgeState,
        issues: Vec<GluingIssue>,
        refusal: Refusal,
    },
}

/// Evaluates `INV-012` gluing on `batch` against the held enterprise per-cell digest history.
#[must_use]
pub fn check_anchor_publish_gluing(
    batch: &DeltaBatch,
    held_cells: &BTreeMap<String, Vec<Digest32>>,
) -> GluingGateDecision {
    match check_gluing(batch, held_cells) {
        GluingReport::Coherent { checked_refs } => {
            GluingGateDecision::PublishAllowed { checked_refs }
        }
        GluingReport::Dangling(issues) => {
            let has_missing_cell = issues
                .iter()
                .any(|i| matches!(i, GluingIssue::MissingCell { .. }));
            let consumer_state = if has_missing_cell {
                KnowledgeState::NotObservable
            } else {
                KnowledgeState::Stale
            };
            let refusal = Refusal::new(
                RefusalCode::StaleAnchor502,
                format!(
                    "INV-012 gluing blocked anchor publish for cell '{}' seq {}: {} dangling cross-cell reference(s)",
                    batch.cell_id,
                    batch.seq,
                    issues.len()
                ),
            );
            GluingGateDecision::PublishBlocked {
                consumer_state,
                issues,
                refusal,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fmine_core::anchor::{FMT_ANCHOR_V1, LedgerAnchor};
    use fmine_core::hash::sha256;
    use fmine_ledger::batch::{CrossCellRef, DeltaKind, DeltaOp, FMT_LEDGER_V1, GENESIS_PREV_HASH};

    fn batch_with_refs(refs: Vec<CrossCellRef>) -> DeltaBatch {
        let anchor = LedgerAnchor {
            format_version: FMT_ANCHOR_V1,
            cell_id: "CELL-DEC".into(),
            ledger_epoch: 1,
            commit_sequence: 0,
            adapter_registry_epoch: 1,
            schema_epoch: 1,
            policy_epoch: 1,
            privacy_epoch: 1,
            residency_epoch: 1,
            state_root: sha256(b"dec-state-root"),
        };
        DeltaBatch {
            format_version: FMT_LEDGER_V1,
            cell_id: "CELL-DEC".into(),
            epoch: 1,
            seq: 0,
            prev_hash: GENESIS_PREV_HASH,
            anchor,
            deltas: vec![DeltaOp::new(
                "decision_card/dec_0001",
                DeltaKind::Put,
                b"card".to_vec(),
            )],
            witnesses: Vec::new(),
            cross_cell_refs: refs,
        }
    }

    /// BD-108 done-when (`Dangling reference blocks publication`): verifies INV-012 gluing blocks
    /// publication with `NotObservable` for missing cell, `Stale` for behind/mismatched digest,
    /// and allows publication once all referenced cells catch up with matching digests.
    #[test]
    fn test_gluing_gate_blocks_anchor_publish_on_dangling_cross_cell_ref() {
        let d0 = sha256(b"lumwana-seq-0");
        let d1 = sha256(b"lumwana-seq-1");
        let batch = batch_with_refs(vec![CrossCellRef::new("CELL-SITE(lumwana)", 1, d1)]);

        // 1. Missing cell -> PublishBlocked + KnowledgeState::NotObservable.
        let empty = BTreeMap::new();
        let blocked_missing = check_anchor_publish_gluing(&batch, &empty);
        assert!(matches!(
            blocked_missing,
            GluingGateDecision::PublishBlocked {
                consumer_state: KnowledgeState::NotObservable,
                ..
            }
        ));

        // 2. Behind cell (only seq 0 held) -> PublishBlocked + KnowledgeState::Stale.
        let mut behind = BTreeMap::new();
        behind.insert("CELL-SITE(lumwana)".to_string(), vec![d0]);
        let blocked_stale = check_anchor_publish_gluing(&batch, &behind);
        assert!(matches!(
            blocked_stale,
            GluingGateDecision::PublishBlocked {
                consumer_state: KnowledgeState::Stale,
                ..
            }
        ));

        // 3. Digest mismatch at seq 1 -> PublishBlocked + KnowledgeState::Stale.
        let mut mismatched = BTreeMap::new();
        mismatched.insert(
            "CELL-SITE(lumwana)".to_string(),
            vec![d0, sha256(b"wrong-digest")],
        );
        assert!(matches!(
            check_anchor_publish_gluing(&batch, &mismatched),
            GluingGateDecision::PublishBlocked {
                consumer_state: KnowledgeState::Stale,
                ..
            }
        ));

        // 4. Coherent enterprise anchor view -> PublishAllowed.
        let mut coherent = BTreeMap::new();
        coherent.insert("CELL-SITE(lumwana)".to_string(), vec![d0, d1]);
        assert_eq!(
            check_anchor_publish_gluing(&batch, &coherent),
            GluingGateDecision::PublishAllowed { checked_refs: 1 }
        );
    }

    #[test]
    fn test_gluing_gate_multi_cell_precedence_and_zero_refs() {
        let d0 = sha256(b"lumwana-seq-0");
        let d1 = sha256(b"lumwana-seq-1");
        let k0 = sha256(b"kibali-seq-0");

        // Zero cross-cell references publish cleanly.
        let zero_refs = batch_with_refs(Vec::new());
        assert_eq!(
            check_anchor_publish_gluing(&zero_refs, &BTreeMap::new()),
            GluingGateDecision::PublishAllowed { checked_refs: 0 }
        );

        // Multi-cell: missing cell takes NotObservable precedence over Stale behind-cell issue.
        let multi = batch_with_refs(vec![
            CrossCellRef::new("CELL-SITE(lumwana)", 1, d1),
            CrossCellRef::new("CELL-SITE(kibali)", 0, k0),
        ]);
        let mut partial = BTreeMap::new();
        partial.insert("CELL-SITE(lumwana)".to_string(), vec![d0]);
        match check_anchor_publish_gluing(&multi, &partial) {
            GluingGateDecision::PublishBlocked {
                consumer_state,
                issues,
                refusal,
            } => {
                assert_eq!(consumer_state, KnowledgeState::NotObservable);
                assert_eq!(issues.len(), 2);
                assert_eq!(refusal.code, "FMINE-REF-502");
            }
            other => panic!("expected PublishBlocked, got {other:?}"),
        }

        // Once both cells hold matching digests, multi-cell anchor publishes.
        partial.insert("CELL-SITE(lumwana)".to_string(), vec![d0, d1]);
        partial.insert("CELL-SITE(kibali)".to_string(), vec![k0]);
        assert_eq!(
            check_anchor_publish_gluing(&multi, &partial),
            GluingGateDecision::PublishAllowed { checked_refs: 2 }
        );
    }
}
