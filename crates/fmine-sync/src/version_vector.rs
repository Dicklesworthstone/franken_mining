//! Determinism: BitwisePortable
//!
//! Per-cell version vectors and lag/staleness detection (`BD-102`, plan §26.11 F1, §26.13 GB-36).

use std::collections::{BTreeMap, BTreeSet};

use fmine_core::codec::{Decoder, Encoder};
use fmine_core::hash::Digest32;

use crate::SyncError;

/// Canonical wire frame tag for per-cell version vectors (`FMNE` + `VVER` + CRC32C).
pub const TAG_VERSION_VECTOR: [u8; 4] = *b"VVER";
const FMT_VV_V1: u16 = 1;
const MAX_CELLS: usize = 4_096;

/// Watermark for one authority cell inside a [`VersionVector`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CellWatermark {
    pub cell_id: String,
    pub epoch: u64,
    pub commit_seq: u64,
    pub mmr_root: Digest32,
    pub last_contact_ns: i64,
}

/// Deterministic ordered map of per-cell watermarks (`BD-102`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct VersionVector {
    pub cells: BTreeMap<String, CellWatermark>,
}

impl VersionVector {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn upsert(&mut self, wm: CellWatermark) {
        self.cells.insert(wm.cell_id.clone(), wm);
    }

    /// Encodes this version vector into canonical `FMNE` + `VVER` + CRC32C wire bytes.
    #[must_use]
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut enc = Encoder::new_frame(TAG_VERSION_VECTOR, FMT_VV_V1);
        enc.put_u32(self.cells.len() as u32);
        for wm in self.cells.values() {
            enc.put_str(&wm.cell_id);
            enc.put_u64(wm.epoch);
            enc.put_u64(wm.commit_seq);
            enc.put_fixed(wm.mmr_root.as_bytes());
            enc.put_i64(wm.last_contact_ns);
        }
        enc.finish()
    }

    /// Decodes canonical `FMNE` + `VVER` bytes.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, SyncError> {
        let mut dec = Decoder::open_frame(bytes, TAG_VERSION_VECTOR, FMT_VV_V1, FMT_VV_V1)?;
        let count = dec.get_u32()? as usize;
        if count > MAX_CELLS {
            return Err(SyncError::InvalidField {
                field: "cells",
                detail: format!("count {count} exceeds {MAX_CELLS}"),
            });
        }
        let mut cells = BTreeMap::new();
        for _ in 0..count {
            let cell_id = dec.get_str(256)?.to_string();
            let epoch = dec.get_u64()?;
            let commit_seq = dec.get_u64()?;
            let mmr_root = Digest32(dec.get_fixed::<32>()?);
            let last_contact_ns = dec.get_i64()?;
            cells.insert(
                cell_id.clone(),
                CellWatermark {
                    cell_id,
                    epoch,
                    commit_seq,
                    mmr_root,
                    last_contact_ns,
                },
            );
        }
        dec.finish()?;
        Ok(Self { cells })
    }
}

/// Per-cell lag/staleness classification computed between a local replica and a remote peer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CellLagStatus {
    /// Identical `(epoch, commit_seq, mmr_root)` and within freshness bound.
    InSync,
    /// Local replica is behind remote on the same or newer epoch (`remote_seq > local_seq`).
    Behind {
        local_seq: u64,
        remote_seq: u64,
        missing_batches: u64,
    },
    /// Local replica is ahead of remote on the same epoch.
    Ahead { local_seq: u64, remote_seq: u64 },
    /// Same `(epoch, commit_seq)` with differing `mmr_root` or epoch regression — split-brain (`F6`).
    Diverged {
        local_epoch: u64,
        remote_epoch: u64,
        detail: String,
    },
    /// Partition / satellite link outage exceeded `stale_threshold_ns` (`F1`).
    Stale { age_ns: i64 },
}

/// Complete deterministic lag summary across all cells known to either peer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LagReport {
    pub per_cell: BTreeMap<String, CellLagStatus>,
}

/// Compares `local` against `remote` at explicit wall time `now_ns` with staleness bound `stale_threshold_ns`.
#[must_use]
pub fn detect_lag(
    local: &VersionVector,
    remote: &VersionVector,
    now_ns: i64,
    stale_threshold_ns: i64,
) -> LagReport {
    let all_ids: BTreeSet<&str> = local
        .cells
        .keys()
        .chain(remote.cells.keys())
        .map(String::as_str)
        .collect();
    let mut per_cell = BTreeMap::new();

    for cell_id in all_ids {
        let status = match (local.cells.get(cell_id), remote.cells.get(cell_id)) {
            (None, Some(r)) => CellLagStatus::Behind {
                local_seq: 0,
                remote_seq: r.commit_seq,
                missing_batches: r.commit_seq,
            },
            (Some(l), None) => {
                let age = now_ns.saturating_sub(l.last_contact_ns);
                if age > stale_threshold_ns {
                    CellLagStatus::Stale { age_ns: age }
                } else {
                    CellLagStatus::Ahead {
                        local_seq: l.commit_seq,
                        remote_seq: 0,
                    }
                }
            }
            (Some(l), Some(r)) => {
                let age = now_ns.saturating_sub(l.last_contact_ns.max(r.last_contact_ns));
                if r.epoch < l.epoch {
                    CellLagStatus::Diverged {
                        local_epoch: l.epoch,
                        remote_epoch: r.epoch,
                        detail: "remote epoch regressed below local highest seen epoch (F6)"
                            .to_string(),
                    }
                } else if r.commit_seq == l.commit_seq && r.mmr_root != l.mmr_root {
                    CellLagStatus::Diverged {
                        local_epoch: l.epoch,
                        remote_epoch: r.epoch,
                        detail: "equal commit_seq with differing MMR root".to_string(),
                    }
                } else if r.commit_seq > l.commit_seq {
                    CellLagStatus::Behind {
                        local_seq: l.commit_seq,
                        remote_seq: r.commit_seq,
                        missing_batches: r.commit_seq - l.commit_seq,
                    }
                } else if l.commit_seq > r.commit_seq {
                    CellLagStatus::Ahead {
                        local_seq: l.commit_seq,
                        remote_seq: r.commit_seq,
                    }
                } else if age > stale_threshold_ns {
                    CellLagStatus::Stale { age_ns: age }
                } else {
                    CellLagStatus::InSync
                }
            }
            (None, None) => CellLagStatus::InSync,
        };
        per_cell.insert(cell_id.to_string(), status);
    }

    LagReport { per_cell }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fmine_core::hash::sha256;

    #[test]
    fn test_version_vector_lag_detection_and_stale_partition_f1() {
        let r_a = sha256(b"root-a");
        let r_b1 = sha256(b"root-b1");
        let r_b2 = sha256(b"root-b2");
        let r_c1 = sha256(b"root-c1");
        let r_c2 = sha256(b"root-c2");

        let mut local = VersionVector::new();
        local.upsert(CellWatermark {
            cell_id: "CELL-SITE(lumwana)".into(),
            epoch: 2,
            commit_seq: 10,
            mmr_root: r_a,
            last_contact_ns: 1_000,
        });
        local.upsert(CellWatermark {
            cell_id: "CELL-SITE(kibali)".into(),
            epoch: 1,
            commit_seq: 4,
            mmr_root: r_b1,
            last_contact_ns: 1_000,
        });
        local.upsert(CellWatermark {
            cell_id: "CELL-SITE(porgera)".into(),
            epoch: 3,
            commit_seq: 7,
            mmr_root: r_c1,
            last_contact_ns: 1_000,
        });
        local.upsert(CellWatermark {
            cell_id: "CELL-SITE(veladero)".into(),
            epoch: 1,
            commit_seq: 5,
            mmr_root: r_a,
            last_contact_ns: 100, // 7-day satellite outage
        });

        let mut remote = VersionVector::new();
        remote.upsert(CellWatermark {
            cell_id: "CELL-SITE(lumwana)".into(),
            epoch: 2,
            commit_seq: 10,
            mmr_root: r_a,
            last_contact_ns: 1_000,
        });
        remote.upsert(CellWatermark {
            cell_id: "CELL-SITE(kibali)".into(),
            epoch: 1,
            commit_seq: 12,
            mmr_root: r_b2,
            last_contact_ns: 1_050,
        });
        remote.upsert(CellWatermark {
            cell_id: "CELL-SITE(porgera)".into(),
            epoch: 2, // epoch regression!
            commit_seq: 7,
            mmr_root: r_c2,
            last_contact_ns: 1_050,
        });

        let wire = local.to_bytes();
        assert_eq!(VersionVector::from_bytes(&wire).expect("decode vv"), local);

        let report = detect_lag(&local, &remote, 1_100, 500);
        assert_eq!(report.per_cell["CELL-SITE(lumwana)"], CellLagStatus::InSync);
        assert_eq!(
            report.per_cell["CELL-SITE(kibali)"],
            CellLagStatus::Behind {
                local_seq: 4,
                remote_seq: 12,
                missing_batches: 8,
            }
        );
        assert!(matches!(
            report.per_cell["CELL-SITE(porgera)"],
            CellLagStatus::Diverged {
                local_epoch: 3,
                remote_epoch: 2,
                ..
            }
        ));
        assert_eq!(
            report.per_cell["CELL-SITE(veladero)"],
            CellLagStatus::Stale { age_ns: 1_000 }
        );
    }
}
