//! Determinism: BitwisePortable
//! As-of replay harness (`BD-053`, plan §8, §33.4, `GATE-PLUMB-E2`, `INV-003`, `INV-016`).
//!
//! Executes a pure deterministic query function over an [`L0Ledger`] cell snapshot at a historical
//! sequence `as_of_seq`, binds its read closure and output into an `FMT-CERT-1` [`Certificate`],
//! and verifies that replaying the same query after arbitrarily many later commits (supersessions,
//! lease transfers, new keys) produces **bit-identical** output bytes and **bit-identical**
//! certificate digests while preventing any visibility of post-cut records.

use fmine_core::anchor::{FMT_ANCHOR_V1, LedgerAnchor};
use fmine_core::cert::{CertError, Certificate, DeterminismClass, InputDigest};
use fmine_core::hash::{Digest32, sha256};

use crate::l0::{CellSnapshot, L0Error, L0Ledger};

/// Errors raised by the as-of replay harness (`BD-053`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AsOfReplayError {
    L0(L0Error),
    Cert(CertError),
    /// Replay output bytes differed before and after later ledger mutations (`GATE-PLUMB-E2`).
    OutputDiverged {
        before: Digest32,
        after: Digest32,
    },
    /// Replay certificate digest differed before and after later ledger mutations (`PO-BED-9`).
    CertificateDiverged {
        before: Digest32,
        after: Digest32,
    },
}

impl From<L0Error> for AsOfReplayError {
    fn from(value: L0Error) -> Self {
        Self::L0(value)
    }
}

impl From<CertError> for AsOfReplayError {
    fn from(value: CertError) -> Self {
        Self::Cert(value)
    }
}

impl core::fmt::Display for AsOfReplayError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::L0(e) => write!(f, "as-of replay L0 error: {e}"),
            Self::Cert(e) => write!(f, "as-of replay certificate error: {e}"),
            Self::OutputDiverged { before, after } => write!(
                f,
                "as-of replay output diverged: before {}, after {}",
                before.to_hex(),
                after.to_hex()
            ),
            Self::CertificateDiverged { before, after } => write!(
                f,
                "as-of replay certificate diverged: before {}, after {}",
                before.to_hex(),
                after.to_hex()
            ),
        }
    }
}

impl std::error::Error for AsOfReplayError {}

/// Certified output produced by running a deterministic query at `as_of_seq`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplayRun {
    pub cell_id: String,
    pub as_of_seq: u64,
    pub output_bytes: Vec<u8>,
    pub output_digest: Digest32,
    pub certificate: Certificate,
}

/// Run a pure query against `ledger.as_of_seq(cell_id, as_of_seq)` and seal its exact input closure
/// and output digest into an `FMT-CERT-1` [`Certificate`].
pub fn run_as_of<F>(
    ledger: &L0Ledger,
    cell_id: &str,
    as_of_seq: u64,
    subject_kind: &str,
    code_revision: &str,
    seed_root: u64,
    query: F,
) -> Result<ReplayRun, AsOfReplayError>
where
    F: FnOnce(&CellSnapshot) -> Vec<u8>,
{
    let snapshot = ledger.as_of_seq(cell_id, as_of_seq)?;
    let output_bytes = query(&snapshot);
    let output_digest = sha256(&output_bytes);

    let mut inputs = Vec::with_capacity(snapshot.live_keys.len() + 1);
    inputs.push(InputDigest::new(
        "cell_mmr_root",
        format!("{cell_id}@seq:{as_of_seq}"),
        snapshot.mmr_root,
    ));
    for (key, ver) in &snapshot.live_keys {
        inputs.push(InputDigest::new(
            "cell_key",
            format!("{cell_id}/{key}@seq:{}", ver.seq),
            ver.payload_digest,
        ));
    }

    let anchor = LedgerAnchor {
        format_version: FMT_ANCHOR_V1,
        cell_id: cell_id.to_string(),
        ledger_epoch: snapshot
            .live_keys
            .values()
            .map(|v| v.epoch)
            .max()
            .unwrap_or(1),
        commit_sequence: as_of_seq,
        adapter_registry_epoch: 1,
        schema_epoch: 1,
        policy_epoch: 1,
        privacy_epoch: 1,
        residency_epoch: 1,
        state_root: snapshot.mmr_root,
    };

    let certificate = Certificate::build(
        subject_kind,
        code_revision,
        DeterminismClass::BitwisePortable,
        seed_root,
        inputs,
        output_digest,
        Some(anchor),
    )?;
    certificate.verify()?;

    Ok(ReplayRun {
        cell_id: cell_id.to_string(),
        as_of_seq,
        output_bytes,
        output_digest,
        certificate,
    })
}

/// Assert bit-identical output bytes and `FMT-CERT-1` certificate digest between an earlier run
/// and a replay run executed after subsequent ledger commits (`GATE-PLUMB-E2`).
pub fn assert_bit_identical_replay(
    earlier: &ReplayRun,
    replayed: &ReplayRun,
) -> Result<(), AsOfReplayError> {
    if earlier.output_bytes != replayed.output_bytes
        || earlier.output_digest != replayed.output_digest
    {
        return Err(AsOfReplayError::OutputDiverged {
            before: earlier.output_digest,
            after: replayed.output_digest,
        });
    }
    if earlier.certificate.certificate_digest != replayed.certificate.certificate_digest {
        return Err(AsOfReplayError::CertificateDiverged {
            before: earlier.certificate.certificate_digest,
            after: replayed.certificate.certificate_digest,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use fmine_core::codec::Encoder;

    use crate::batch::{DeltaBatch, DeltaKind, DeltaOp, FMT_LEDGER_V1, GENESIS_PREV_HASH};

    fn mk_batch(
        cell_id: &str,
        epoch: u64,
        seq: u64,
        prev_hash: Digest32,
        deltas: Vec<DeltaOp>,
    ) -> DeltaBatch {
        let anchor = LedgerAnchor {
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
        };
        DeltaBatch {
            format_version: FMT_LEDGER_V1,
            cell_id: cell_id.to_string(),
            epoch,
            seq,
            prev_hash,
            anchor,
            deltas,
            witnesses: Vec::new(),
            cross_cell_refs: Vec::new(),
        }
    }

    fn canonical_snapshot_report(snap: &CellSnapshot) -> Vec<u8> {
        let mut enc = Encoder::new_frame(*b"ASOF", 1);
        enc.put_str(&snap.cell_id);
        enc.put_u64(snap.committed_count);
        enc.put_fixed(snap.mmr_root.as_bytes());
        enc.put_u32(snap.live_keys.len() as u32);
        for (k, v) in &snap.live_keys {
            enc.put_str(k);
            enc.put_u64(v.seq);
            enc.put_bytes(&v.payload);
        }
        enc.finish()
    }

    #[test]
    fn test_as_of_replay_is_bit_identical_before_and_after_later_commits() {
        let cell = "CELL-EVID(barrick)";
        let mut l0 = L0Ledger::new();
        let lease1 = l0.create_cell(cell, "writer-1").expect("create cell");

        // Commit seq 0 and seq 1.
        let d0 = l0
            .commit(
                &lease1,
                mk_batch(
                    cell,
                    1,
                    0,
                    GENESIS_PREV_HASH,
                    vec![DeltaOp::new(
                        "assay/hole-1",
                        DeltaKind::Put,
                        b"au=2.10".to_vec(),
                    )],
                ),
            )
            .expect("commit 0");
        let d1 = l0
            .commit(
                &lease1,
                mk_batch(
                    cell,
                    1,
                    1,
                    d0,
                    vec![DeltaOp::new(
                        "assay/hole-2",
                        DeltaKind::Put,
                        b"au=1.75".to_vec(),
                    )],
                ),
            )
            .expect("commit 1");

        // Capture historical run at committed_count = 2 BEFORE future data arrives.
        let run_before = run_as_of(
            &l0,
            cell,
            2,
            "as_of_assay_report",
            "31cd53e",
            101,
            canonical_snapshot_report,
        )
        .expect("run before");

        // Now transfer lease to epoch 2, supersede hole-1, and add hole-3 (future data!).
        let lease2 = l0.transfer_lease(cell, "writer-2").expect("transfer");
        let d2 = l0
            .commit(
                &lease2,
                mk_batch(
                    cell,
                    2,
                    2,
                    d1,
                    vec![
                        DeltaOp::new("assay/hole-1", DeltaKind::Supersede, b"au=9.99".to_vec()),
                        DeltaOp::new("assay/hole-3", DeltaKind::Put, b"au=4.44".to_vec()),
                    ],
                ),
            )
            .expect("commit 2");
        let _d3 = l0
            .commit(
                &lease2,
                mk_batch(
                    cell,
                    2,
                    3,
                    d2,
                    vec![DeltaOp::new(
                        "assay/hole-4",
                        DeltaKind::Put,
                        b"au=0.50".to_vec(),
                    )],
                ),
            )
            .expect("commit 3");

        // Replay at committed_count = 2 AFTER future data arrived: must be bit-identical!
        let run_after = run_as_of(
            &l0,
            cell,
            2,
            "as_of_assay_report",
            "31cd53e",
            101,
            canonical_snapshot_report,
        )
        .expect("run after");
        assert_bit_identical_replay(&run_before, &run_after).expect("bit-identical replay");
        assert_eq!(
            run_before.certificate.to_bytes().expect("enc before"),
            run_after.certificate.to_bytes().expect("enc after")
        );

        // And a run at committed_count = 4 sees the new data and produces a strictly distinct digest.
        let run_latest = run_as_of(
            &l0,
            cell,
            4,
            "as_of_assay_report",
            "31cd53e",
            101,
            canonical_snapshot_report,
        )
        .expect("run latest");
        assert!(matches!(
            assert_bit_identical_replay(&run_before, &run_latest),
            Err(AsOfReplayError::OutputDiverged { .. })
        ));

        // Requesting beyond committed count refuses cleanly.
        assert!(matches!(
            run_as_of(
                &l0,
                cell,
                5,
                "as_of_assay_report",
                "31cd53e",
                101,
                canonical_snapshot_report,
            ),
            Err(AsOfReplayError::L0(L0Error::SequenceOutOfRange { .. }))
        ));
    }
}
