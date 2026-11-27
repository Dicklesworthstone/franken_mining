//! Determinism: BitwisePortable
//! Bedrock formal-model (FM) protocol oracles `PO-BED-1`, `PO-BED-2`, `PO-BED-3`, `PO-BED-4`, and
//! `PO-BED-10` (`BD-101`, plan §26.7 & §39 WP-05).
//!
//! Every oracle is a pure deterministic checker over a recorded protocol event trace, accompanied
//! by both a clean-protocol control and a seeded-bug test proving the oracle fires on violation:
//! - `PO-BED-1` (Single writer): at most one lease holder per `(cell_id, epoch)`; no batch with
//!   `epoch < highest_seen_epoch` may be accepted without quarantine.
//! - `PO-BED-2` (Prefix consistency): every enterprise replica log for a cell must be an exact
//!   hash-matched prefix of the authoritative writer log.
//! - `PO-BED-3` (Root-last CAS): no CAS root may become visible before every one of its referenced
//!   chunks has been written and SHA-256 verified.
//! - `PO-BED-4` (Exactly-once posting): every FA posting slot carries Petri token count `<= 1`
//!   (at most one live original per slot; duplicate commit fires immediately).
//! - `PO-BED-10` (Bridge idempotency): replaying an outbox after a crash under the same idempotency
//!   keys must leave committed state digest unchanged (`already_recorded`, never re-keyed).

use std::collections::{BTreeMap, BTreeSet};

use fmine_core::Digest32;
use fmine_core::hash::blake3_hash;

/// Named Bedrock proof obligation checked by an FM oracle (`BD-101`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ProofObligation {
    PoBed1SingleWriter,
    PoBed2PrefixConsistency,
    PoBed3RootLastCas,
    PoBed4ExactlyOncePosting,
    PoBed10BridgeIdempotency,
}

impl ProofObligation {
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::PoBed1SingleWriter => "PO-BED-1",
            Self::PoBed2PrefixConsistency => "PO-BED-2",
            Self::PoBed3RootLastCas => "PO-BED-3",
            Self::PoBed4ExactlyOncePosting => "PO-BED-4",
            Self::PoBed10BridgeIdempotency => "PO-BED-10",
        }
    }
}

/// Violation emitted when an FM oracle catches a protocol bug (`BD-101`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OracleViolation {
    pub obligation: ProofObligation,
    pub event_index: usize,
    pub detail: String,
}

/// Event observed by the `PO-BED-1` single-writer fencing oracle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LeaseEvent {
    GrantLease {
        cell_id: String,
        epoch: u64,
        holder_id: String,
    },
    ApplyBatch {
        cell_id: String,
        epoch: u64,
        holder_id: String,
        quarantined: bool,
    },
}

/// Check `PO-BED-1` (Single writer & monotonic fencing epochs).
#[must_use]
pub fn check_po_bed_1_single_writer(events: &[LeaseEvent]) -> Vec<OracleViolation> {
    let mut violations = Vec::new();
    let mut holders: BTreeMap<(String, u64), String> = BTreeMap::new();
    let mut max_epoch: BTreeMap<String, u64> = BTreeMap::new();

    for (idx, ev) in events.iter().enumerate() {
        match ev {
            LeaseEvent::GrantLease {
                cell_id,
                epoch,
                holder_id,
            } => {
                if cell_id.trim().is_empty() || holder_id.trim().is_empty() {
                    violations.push(OracleViolation {
                        obligation: ProofObligation::PoBed1SingleWriter,
                        event_index: idx,
                        detail: format!(
                            "blank cell_id ('{cell_id}') or holder_id ('{holder_id}') in GrantLease"
                        ),
                    });
                    continue;
                }
                if *epoch == 0 {
                    violations.push(OracleViolation {
                        obligation: ProofObligation::PoBed1SingleWriter,
                        event_index: idx,
                        detail: format!("cell '{cell_id}': epoch 0 lease granted to '{holder_id}'"),
                    });
                    continue;
                }
                let cur_max = max_epoch.get(cell_id).copied().unwrap_or(0);
                if *epoch < cur_max {
                    violations.push(OracleViolation {
                        obligation: ProofObligation::PoBed1SingleWriter,
                        event_index: idx,
                        detail: format!(
                            "cell '{cell_id}': regressed lease epoch {epoch} < {cur_max} granted to '{holder_id}'"
                        ),
                    });
                }
                if let Some(prev) = holders.get(&(cell_id.clone(), *epoch)) {
                    if prev != holder_id {
                        violations.push(OracleViolation {
                            obligation: ProofObligation::PoBed1SingleWriter,
                            event_index: idx,
                            detail: format!(
                                "cell '{cell_id}' epoch {epoch}: dual lease holders '{prev}' and '{holder_id}'"
                            ),
                        });
                    }
                } else {
                    holders.insert((cell_id.clone(), *epoch), holder_id.clone());
                }
                if *epoch > cur_max {
                    max_epoch.insert(cell_id.clone(), *epoch);
                }
            }
            LeaseEvent::ApplyBatch {
                cell_id,
                epoch,
                holder_id,
                quarantined,
            } => {
                if cell_id.trim().is_empty() || holder_id.trim().is_empty() {
                    violations.push(OracleViolation {
                        obligation: ProofObligation::PoBed1SingleWriter,
                        event_index: idx,
                        detail: format!(
                            "blank cell_id ('{cell_id}') or holder_id ('{holder_id}') in ApplyBatch"
                        ),
                    });
                    continue;
                }
                let high = max_epoch.get(cell_id).copied().unwrap_or(0);
                if *epoch < high && !quarantined {
                    violations.push(OracleViolation {
                        obligation: ProofObligation::PoBed1SingleWriter,
                        event_index: idx,
                        detail: format!(
                            "cell '{cell_id}': lower-epoch batch ({epoch} < {high}) from '{holder_id}' applied without quarantine"
                        ),
                    });
                }
                if !quarantined {
                    match holders.get(&(cell_id.clone(), *epoch)) {
                        Some(owner) if owner != holder_id => {
                            violations.push(OracleViolation {
                                obligation: ProofObligation::PoBed1SingleWriter,
                                event_index: idx,
                                detail: format!(
                                    "cell '{cell_id}' epoch {epoch}: non-holder '{holder_id}' applied batch (lease held by '{owner}')"
                                ),
                            });
                        }
                        None => {
                            violations.push(OracleViolation {
                                obligation: ProofObligation::PoBed1SingleWriter,
                                event_index: idx,
                                detail: format!(
                                    "cell '{cell_id}' epoch {epoch}: unleased writer '{holder_id}' applied batch without an active lease"
                                ),
                            });
                        }
                        Some(_) => {}
                    }
                }
            }
        }
    }
    violations
}

/// Check `PO-BED-2` (Enterprise replica log must be an exact hash-matched prefix of the writer log).
#[must_use]
pub fn check_po_bed_2_prefix_consistency(
    cell_id: &str,
    writer_hashes: &[Digest32],
    replica_hashes: &[Digest32],
) -> Vec<OracleViolation> {
    let mut violations = Vec::new();
    if replica_hashes.len() > writer_hashes.len() {
        violations.push(OracleViolation {
            obligation: ProofObligation::PoBed2PrefixConsistency,
            event_index: writer_hashes.len(),
            detail: format!(
                "cell '{cell_id}': replica length {} exceeds writer length {}",
                replica_hashes.len(),
                writer_hashes.len()
            ),
        });
    }
    for (idx, (r, w)) in replica_hashes.iter().zip(writer_hashes.iter()).enumerate() {
        if r != w {
            violations.push(OracleViolation {
                obligation: ProofObligation::PoBed2PrefixConsistency,
                event_index: idx,
                detail: format!(
                    "cell '{cell_id}': prefix divergence at seq {idx}: replica {} != writer {}",
                    r.to_hex(),
                    w.to_hex()
                ),
            });
        }
    }
    violations
}

/// Event observed by the `PO-BED-3` root-last CAS visibility oracle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CasTraceEvent {
    WriteAndVerifyChunk {
        chunk_id: String,
    },
    PublishRoot {
        root_id: String,
        required_chunks: Vec<String>,
    },
    DeleteUnreferencedChunk {
        chunk_id: String,
    },
}

/// Check `PO-BED-3` (No CAS root visible unless all its referenced chunks are already verified).
#[must_use]
pub fn check_po_bed_3_root_last_cas(events: &[CasTraceEvent]) -> Vec<OracleViolation> {
    let mut violations = Vec::new();
    let mut verified_chunks: BTreeSet<String> = BTreeSet::new();
    let mut live_pinned_chunks: BTreeSet<String> = BTreeSet::new();
    let mut published_roots: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();

    for (idx, ev) in events.iter().enumerate() {
        match ev {
            CasTraceEvent::WriteAndVerifyChunk { chunk_id } => {
                verified_chunks.insert(chunk_id.clone());
            }
            CasTraceEvent::PublishRoot {
                root_id,
                required_chunks,
            } => {
                let missing: Vec<&str> = required_chunks
                    .iter()
                    .filter(|c| !verified_chunks.contains(c.as_str()))
                    .map(String::as_str)
                    .collect();
                if !missing.is_empty() {
                    violations.push(OracleViolation {
                        obligation: ProofObligation::PoBed3RootLastCas,
                        event_index: idx,
                        detail: format!(
                            "root '{root_id}' published before chunks verified: {}",
                            missing.join(", ")
                        ),
                    });
                } else {
                    let req_set: BTreeSet<String> = required_chunks.iter().cloned().collect();
                    if let Some(existing) = published_roots.get(root_id)
                        && existing != &req_set
                    {
                        violations.push(OracleViolation {
                            obligation: ProofObligation::PoBed3RootLastCas,
                            event_index: idx,
                            detail: format!(
                                "root '{root_id}' re-published with conflicting chunk set"
                            ),
                        });
                    } else {
                        published_roots.insert(root_id.clone(), req_set);
                    }
                    for c in required_chunks {
                        live_pinned_chunks.insert(c.clone());
                    }
                }
            }
            CasTraceEvent::DeleteUnreferencedChunk { chunk_id } => {
                if live_pinned_chunks.contains(chunk_id) {
                    violations.push(OracleViolation {
                        obligation: ProofObligation::PoBed3RootLastCas,
                        event_index: idx,
                        detail: format!(
                            "chunk '{chunk_id}' deleted while still referenced by a published root"
                        ),
                    });
                }
                verified_chunks.remove(chunk_id);
            }
        }
    }
    violations
}

/// Event observed by the `PO-BED-4` exactly-once posting Petri-net oracle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PostingEvent {
    CommitOriginal {
        slot_id: String,
        journal_entry_id: String,
    },
    SupersedeWithReversal {
        slot_id: String,
        reversal_entry_id: String,
    },
}

/// Check `PO-BED-4` (At most 1 live original per FA posting slot; Petri token count `<= 1`).
#[must_use]
pub fn check_po_bed_4_exactly_once_posting(events: &[PostingEvent]) -> Vec<OracleViolation> {
    let mut violations = Vec::new();
    let mut live_tokens: BTreeMap<String, String> = BTreeMap::new();
    let mut seen_entry_ids: BTreeSet<String> = BTreeSet::new();

    for (idx, ev) in events.iter().enumerate() {
        match ev {
            PostingEvent::CommitOriginal {
                slot_id,
                journal_entry_id,
            } => {
                if slot_id.trim().is_empty() || journal_entry_id.trim().is_empty() {
                    violations.push(OracleViolation {
                        obligation: ProofObligation::PoBed4ExactlyOncePosting,
                        event_index: idx,
                        detail: format!(
                            "blank slot_id ('{slot_id}') or journal_entry_id ('{journal_entry_id}')"
                        ),
                    });
                    continue;
                }
                if !seen_entry_ids.insert(journal_entry_id.clone()) {
                    violations.push(OracleViolation {
                        obligation: ProofObligation::PoBed4ExactlyOncePosting,
                        event_index: idx,
                        detail: format!(
                            "slot '{slot_id}': reused journal_entry_id '{journal_entry_id}'"
                        ),
                    });
                }
                if let Some(existing) = live_tokens.get(slot_id) {
                    violations.push(OracleViolation {
                        obligation: ProofObligation::PoBed4ExactlyOncePosting,
                        event_index: idx,
                        detail: format!(
                            "slot '{slot_id}': duplicate live original '{journal_entry_id}' while '{existing}' is still live (Petri tokens = 2)"
                        ),
                    });
                } else {
                    live_tokens.insert(slot_id.clone(), journal_entry_id.clone());
                }
            }
            PostingEvent::SupersedeWithReversal {
                slot_id,
                reversal_entry_id,
            } => {
                if slot_id.trim().is_empty() || reversal_entry_id.trim().is_empty() {
                    violations.push(OracleViolation {
                        obligation: ProofObligation::PoBed4ExactlyOncePosting,
                        event_index: idx,
                        detail: format!(
                            "blank slot_id ('{slot_id}') or reversal_entry_id ('{reversal_entry_id}')"
                        ),
                    });
                    continue;
                }
                if !seen_entry_ids.insert(reversal_entry_id.clone()) {
                    violations.push(OracleViolation {
                        obligation: ProofObligation::PoBed4ExactlyOncePosting,
                        event_index: idx,
                        detail: format!(
                            "slot '{slot_id}': reused reversal_entry_id '{reversal_entry_id}'"
                        ),
                    });
                }
                if live_tokens.remove(slot_id).is_none() {
                    violations.push(OracleViolation {
                        obligation: ProofObligation::PoBed4ExactlyOncePosting,
                        event_index: idx,
                        detail: format!(
                            "slot '{slot_id}': reversal '{reversal_entry_id}' fired with 0 live originals"
                        ),
                    });
                }
            }
        }
    }
    violations
}

/// One finance-bridge outbox intent with stable idempotency key (`PO-BED-10`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BridgeIntent {
    pub idempotency_key: String,
    pub account_code: String,
    pub amount_cents: i64,
}

/// Result of replaying a batch of intents against a simulated FA ledger state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BridgeReplayState {
    pub recorded_keys: BTreeSet<String>,
    pub ledger_rows: Vec<(String, String, i64)>,
}

impl BridgeReplayState {
    #[must_use]
    pub fn state_digest(&self) -> Digest32 {
        let mut buf = Vec::new();
        buf.extend_from_slice(b"fmine:po-bed-10:fa-state:v1\0");
        buf.extend_from_slice(&(self.ledger_rows.len() as u64).to_le_bytes());
        for (k, acct, amt) in &self.ledger_rows {
            buf.extend_from_slice(&(k.len() as u32).to_le_bytes());
            buf.extend_from_slice(k.as_bytes());
            buf.extend_from_slice(&(acct.len() as u32).to_le_bytes());
            buf.extend_from_slice(acct.as_bytes());
            buf.extend_from_slice(&amt.to_le_bytes());
        }
        blake3_hash(&buf)
    }

    /// Apply one intent; when `enforce_idempotency` is `false`, models a seeded re-key/duplicate bug.
    pub fn apply(&mut self, intent: &BridgeIntent, enforce_idempotency: bool) {
        if enforce_idempotency && self.recorded_keys.contains(&intent.idempotency_key) {
            return;
        }
        self.recorded_keys.insert(intent.idempotency_key.clone());
        self.ledger_rows.push((
            intent.idempotency_key.clone(),
            intent.account_code.clone(),
            intent.amount_cents,
        ));
    }
}

/// Check `PO-BED-10` (Bridge outbox replay after crash leaves FA state digest completely unchanged,
/// and no idempotency key may be bound to conflicting payloads).
#[must_use]
pub fn check_po_bed_10_bridge_idempotency(
    intents: &[BridgeIntent],
    enforce_idempotency: bool,
) -> Vec<OracleViolation> {
    let mut violations = Vec::new();
    let mut expected_payloads: BTreeMap<&str, (&str, i64)> = BTreeMap::new();
    for (idx, intent) in intents.iter().enumerate() {
        if intent.idempotency_key.trim().is_empty() {
            violations.push(OracleViolation {
                obligation: ProofObligation::PoBed10BridgeIdempotency,
                event_index: idx,
                detail: "blank idempotency_key in BridgeIntent".to_string(),
            });
            continue;
        }
        let payload = (intent.account_code.as_str(), intent.amount_cents);
        if let Some(&prev) = expected_payloads.get(intent.idempotency_key.as_str()) {
            if prev != payload {
                violations.push(OracleViolation {
                    obligation: ProofObligation::PoBed10BridgeIdempotency,
                    event_index: idx,
                    detail: format!(
                        "idempotency_key '{}' reused with conflicting payload ({}, {}) vs ({}, {})",
                        intent.idempotency_key, prev.0, prev.1, payload.0, payload.1
                    ),
                });
            }
        } else {
            expected_payloads.insert(intent.idempotency_key.as_str(), payload);
        }
    }

    let mut state = BridgeReplayState {
        recorded_keys: BTreeSet::new(),
        ledger_rows: Vec::new(),
    };
    for intent in intents {
        state.apply(intent, enforce_idempotency);
    }
    let after_first_pass = state.state_digest();
    let first_rows = state.ledger_rows.len();

    // Simulate crash + full outbox replay under the exact same idempotency keys.
    for (idx, intent) in intents.iter().enumerate() {
        state.apply(intent, enforce_idempotency);
        if state.state_digest() != after_first_pass || state.ledger_rows.len() != first_rows {
            violations.push(OracleViolation {
                obligation: ProofObligation::PoBed10BridgeIdempotency,
                event_index: idx,
                detail: format!(
                    "replaying key '{}' mutated FA state (rows {} -> {})",
                    intent.idempotency_key,
                    first_rows,
                    state.ledger_rows.len()
                ),
            });
            break;
        }
    }
    violations
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_five_fm_oracles_pass_clean_and_fire_on_seeded_bugs() {
        // 1. PO-BED-1 Single writer: clean lease transfer + quarantined stale batch passes;
        //    unquarantined lower-epoch write AND dual lease holders both fire!
        let clean_bed1 = vec![
            LeaseEvent::GrantLease {
                cell_id: "CELL-PIT".into(),
                epoch: 1,
                holder_id: "node-a".into(),
            },
            LeaseEvent::GrantLease {
                cell_id: "CELL-PIT".into(),
                epoch: 2,
                holder_id: "node-b".into(),
            },
            LeaseEvent::ApplyBatch {
                cell_id: "CELL-PIT".into(),
                epoch: 1,
                holder_id: "node-a".into(),
                quarantined: true,
            },
        ];
        assert!(check_po_bed_1_single_writer(&clean_bed1).is_empty());

        let mut buggy_bed1 = clean_bed1;
        buggy_bed1[2] = LeaseEvent::ApplyBatch {
            cell_id: "CELL-PIT".into(),
            epoch: 1,
            holder_id: "node-a".into(),
            quarantined: false,
        };
        let v1 = check_po_bed_1_single_writer(&buggy_bed1);
        assert_eq!(v1.len(), 1);
        assert_eq!(v1[0].obligation, ProofObligation::PoBed1SingleWriter);

        // 2. PO-BED-2 Prefix consistency: valid prefix passes; forked digest fires!
        let d0 = blake3_hash(b"b0");
        let d1 = blake3_hash(b"b1");
        let d2 = blake3_hash(b"b2");
        let d_fork = blake3_hash(b"b1-fork");
        assert!(check_po_bed_2_prefix_consistency("CELL-PIT", &[d0, d1, d2], &[d0, d1]).is_empty());
        let v2 = check_po_bed_2_prefix_consistency("CELL-PIT", &[d0, d1, d2], &[d0, d_fork]);
        assert_eq!(v2.len(), 1);
        assert_eq!(v2[0].obligation, ProofObligation::PoBed2PrefixConsistency);

        // 3. PO-BED-3 Root-last CAS: write-all-then-publish passes; premature publish fires!
        let clean_cas = vec![
            CasTraceEvent::WriteAndVerifyChunk {
                chunk_id: "c0".into(),
            },
            CasTraceEvent::WriteAndVerifyChunk {
                chunk_id: "c1".into(),
            },
            CasTraceEvent::PublishRoot {
                root_id: "r0".into(),
                required_chunks: vec!["c0".into(), "c1".into()],
            },
        ];
        assert!(check_po_bed_3_root_last_cas(&clean_cas).is_empty());
        let buggy_cas = vec![
            CasTraceEvent::WriteAndVerifyChunk {
                chunk_id: "c0".into(),
            },
            CasTraceEvent::PublishRoot {
                root_id: "r0".into(),
                required_chunks: vec!["c0".into(), "c1".into()],
            },
        ];
        let v3 = check_po_bed_3_root_last_cas(&buggy_cas);
        assert_eq!(v3.len(), 1);
        assert_eq!(v3[0].obligation, ProofObligation::PoBed3RootLastCas);

        // 4. PO-BED-4 Exactly-once posting: commit -> supersede -> re-commit passes; double commit fires!
        let clean_post = vec![
            PostingEvent::CommitOriginal {
                slot_id: "2026Q2-ore-haul".into(),
                journal_entry_id: "JE-1".into(),
            },
            PostingEvent::SupersedeWithReversal {
                slot_id: "2026Q2-ore-haul".into(),
                reversal_entry_id: "REV-1".into(),
            },
            PostingEvent::CommitOriginal {
                slot_id: "2026Q2-ore-haul".into(),
                journal_entry_id: "JE-2".into(),
            },
        ];
        assert!(check_po_bed_4_exactly_once_posting(&clean_post).is_empty());
        let buggy_post = vec![
            PostingEvent::CommitOriginal {
                slot_id: "2026Q2-ore-haul".into(),
                journal_entry_id: "JE-1".into(),
            },
            PostingEvent::CommitOriginal {
                slot_id: "2026Q2-ore-haul".into(),
                journal_entry_id: "JE-DUP".into(),
            },
        ];
        let v4 = check_po_bed_4_exactly_once_posting(&buggy_post);
        assert_eq!(v4.len(), 1);
        assert_eq!(v4[0].obligation, ProofObligation::PoBed4ExactlyOncePosting);

        // 5. PO-BED-10 Bridge idempotency: idempotent replay passes; unprotected replay fires!
        let intents = vec![
            BridgeIntent {
                idempotency_key: "key-001".into(),
                account_code: "5100-MINING".into(),
                amount_cents: 45_000_000,
            },
            BridgeIntent {
                idempotency_key: "key-002".into(),
                account_code: "5200-MILLING".into(),
                amount_cents: 32_000_000,
            },
        ];
        assert!(check_po_bed_10_bridge_idempotency(&intents, true).is_empty());
        let v10 = check_po_bed_10_bridge_idempotency(&intents, false);
        assert_eq!(v10.len(), 1);
        assert_eq!(v10[0].obligation, ProofObligation::PoBed10BridgeIdempotency);
    }

    #[test]
    fn test_all_five_fm_oracles_catch_every_subtle_violation_class() {
        // PO-BED-1: active holder write passes; epoch-0 grant, regressed epoch grant, dual holder,
        // unleased writer, and non-holder write all fire.
        let clean_write = vec![
            LeaseEvent::GrantLease {
                cell_id: "CELL-PIT".into(),
                epoch: 1,
                holder_id: "node-a".into(),
            },
            LeaseEvent::ApplyBatch {
                cell_id: "CELL-PIT".into(),
                epoch: 1,
                holder_id: "node-a".into(),
                quarantined: false,
            },
        ];
        assert!(check_po_bed_1_single_writer(&clean_write).is_empty());

        let unleased = vec![LeaseEvent::ApplyBatch {
            cell_id: "CELL-PIT".into(),
            epoch: 1,
            holder_id: "rogue".into(),
            quarantined: false,
        }];
        let v_unleased = check_po_bed_1_single_writer(&unleased);
        assert_eq!(v_unleased.len(), 1);
        assert!(v_unleased[0].detail.contains("unleased writer"));

        let regressed_and_dual = vec![
            LeaseEvent::GrantLease {
                cell_id: "CELL-PIT".into(),
                epoch: 0,
                holder_id: "node-z".into(),
            },
            LeaseEvent::GrantLease {
                cell_id: "CELL-PIT".into(),
                epoch: 2,
                holder_id: "node-b".into(),
            },
            LeaseEvent::GrantLease {
                cell_id: "CELL-PIT".into(),
                epoch: 1,
                holder_id: "node-a".into(),
            },
            LeaseEvent::GrantLease {
                cell_id: "CELL-PIT".into(),
                epoch: 2,
                holder_id: "node-c".into(),
            },
            LeaseEvent::ApplyBatch {
                cell_id: "CELL-PIT".into(),
                epoch: 2,
                holder_id: "node-c".into(),
                quarantined: false,
            },
        ];
        let v_bed1 = check_po_bed_1_single_writer(&regressed_and_dual);
        assert_eq!(v_bed1.len(), 4);

        // PO-BED-2: empty replica and exact match pass; overlong replica fires.
        let d0 = blake3_hash(b"b0");
        let d1 = blake3_hash(b"b1");
        assert!(check_po_bed_2_prefix_consistency("CELL-PIT", &[d0], &[]).is_empty());
        assert!(check_po_bed_2_prefix_consistency("CELL-PIT", &[d0], &[d0]).is_empty());
        let v_overlong = check_po_bed_2_prefix_consistency("CELL-PIT", &[d0], &[d0, d1]);
        assert_eq!(v_overlong.len(), 1);
        assert!(v_overlong[0].detail.contains("exceeds writer length"));

        // PO-BED-3: deleting unreferenced chunk passes; deleting pinned chunk and conflicting root
        // re-publication both fire.
        let cas_events = vec![
            CasTraceEvent::WriteAndVerifyChunk {
                chunk_id: "c0".into(),
            },
            CasTraceEvent::WriteAndVerifyChunk {
                chunk_id: "c1".into(),
            },
            CasTraceEvent::WriteAndVerifyChunk {
                chunk_id: "c_unused".into(),
            },
            CasTraceEvent::DeleteUnreferencedChunk {
                chunk_id: "c_unused".into(),
            },
            CasTraceEvent::PublishRoot {
                root_id: "r0".into(),
                required_chunks: vec!["c0".into()],
            },
            CasTraceEvent::PublishRoot {
                root_id: "r0".into(),
                required_chunks: vec!["c0".into(), "c1".into()],
            },
            CasTraceEvent::DeleteUnreferencedChunk {
                chunk_id: "c0".into(),
            },
        ];
        let v_cas = check_po_bed_3_root_last_cas(&cas_events);
        assert_eq!(v_cas.len(), 2);
        assert!(v_cas[0].detail.contains("conflicting chunk set"));
        assert!(v_cas[1].detail.contains("still referenced"));

        // PO-BED-4: unmatched reversal and reused journal entry ID both fire.
        let post_bugs = vec![
            PostingEvent::SupersedeWithReversal {
                slot_id: "slot-empty".into(),
                reversal_entry_id: "REV-ORPHAN".into(),
            },
            PostingEvent::CommitOriginal {
                slot_id: "slot-a".into(),
                journal_entry_id: "JE-1".into(),
            },
            PostingEvent::SupersedeWithReversal {
                slot_id: "slot-a".into(),
                reversal_entry_id: "REV-1".into(),
            },
            PostingEvent::CommitOriginal {
                slot_id: "slot-a".into(),
                journal_entry_id: "JE-1".into(),
            },
        ];
        let v_post = check_po_bed_4_exactly_once_posting(&post_bugs);
        assert_eq!(v_post.len(), 2);
        assert!(v_post[0].detail.contains("0 live originals"));
        assert!(v_post[1].detail.contains("reused journal_entry_id"));

        // PO-BED-10: conflicting payload under same idempotency key and blank key both fire even
        // when enforce_idempotency is true.
        let drift_intents = vec![
            BridgeIntent {
                idempotency_key: "key-001".into(),
                account_code: "5100-MINING".into(),
                amount_cents: 100,
            },
            BridgeIntent {
                idempotency_key: "key-001".into(),
                account_code: "5100-MINING".into(),
                amount_cents: 200,
            },
            BridgeIntent {
                idempotency_key: "  ".into(),
                account_code: "5200-MILLING".into(),
                amount_cents: 50,
            },
        ];
        let v_drift = check_po_bed_10_bridge_idempotency(&drift_intents, true);
        assert_eq!(v_drift.len(), 2);
        assert!(v_drift[0].detail.contains("conflicting payload"));
        assert!(v_drift[1].detail.contains("blank idempotency_key"));
    }
}
