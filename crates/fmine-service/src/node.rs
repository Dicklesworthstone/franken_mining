//! Determinism: BitwisePortable
//!
//! Core `fmined` node state machine (`BD-119`, plan §26.6–§26.7, §30.3):
//! compiles the role's `AppSpec` into `CompiledSupervisor`, starts the role-appropriate region
//! tree in topological parent-before-child order, holds single-writer cell leases with monotonic
//! fencing epochs (`PO-BED-1`), optionally persists `FMT-SEG-1` segments and `FMT-CAS-1` objects,
//! and enforces read-only degraded mode (exit `4`) when a ledger writer escalates.
//!
//! All state transitions accept an explicit virtual timestamp `now_ns: u64` and read no ambient
//! wall clock, enabling deterministic virtual-time daemon lab tests.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use fmine_core::anchor::{FMT_ANCHOR_V1, LedgerAnchor};
use fmine_core::hash::{Digest32, sha256};
use fmine_ledger::batch::{DeltaBatch, DeltaKind, DeltaOp, FMT_LEDGER_V2};
use fmine_ledger::cell::{AdmitOutcome, CellLease};
use fmine_ledger::l0::L0Ledger;
use fmine_ledger::segment::SemanticSegmentWriter;
use fmine_object::store::CasStore;
use fmine_runtime::region::{RegionKind, Role};
use fmine_runtime::supervise::{AppSpec, Child, CompiledSupervisor, Outcome};

use crate::config::{NodeConfig, role_name};
use crate::{ServiceError, escape_json};

/// Machine-readable status schema identifier emitted by `fmined`.
pub const STATUS_SCHEMA: &str = "fmine.fmined.status.v1";

/// Operational mode of a running `fmined` node.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NodeMode {
    Running,
    /// Entered permanently for this process when a `LedgerWriter` escalates (exit `4` on writes).
    ReadOnlyDegraded {
        failed_cell: String,
    },
    Stopped,
}

impl NodeMode {
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::ReadOnlyDegraded { .. } => "read_only_degraded",
            Self::Stopped => "stopped",
        }
    }
}

/// Receipt emitted after a durable commit into a leased cell.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitReceipt {
    pub cell_id: String,
    pub epoch: u64,
    pub seq: u64,
    pub batch_digest: Digest32,
    pub mmr_root: Digest32,
}

/// Live `fmined` node state (supervisor + leases + ledger + optional disk persistence).
pub struct FminedNode {
    config: NodeConfig,
    supervisor: CompiledSupervisor,
    mode: NodeMode,
    ledger: L0Ledger,
    known_cells: BTreeSet<String>,
    known_peers: BTreeSet<String>,
    active_leases: BTreeMap<String, CellLease>,
    segment_paths: BTreeMap<String, PathBuf>,
    segment_writers: BTreeMap<String, SemanticSegmentWriter>,
    cas_store: Option<CasStore>,
    restarts_total: u64,
}

impl FminedNode {
    /// Boots a `fmined` node for `config.role` at `start_ns`, compiling its `AppSpec`, starting
    /// its role-specific supervision tree, acquiring leases for `config.cells`, and registering
    /// `config.peers` under `SyncRegion`.
    pub fn boot(config: NodeConfig, start_ns: u64) -> Result<Self, ServiceError> {
        config.validate()?;
        let spec = AppSpec::for_role(config.role);
        let mut supervisor = spec.compile()?;

        // Boot core regions in topological parent-before-child order per plan §26.6.
        let base_regions: &[RegionKind] = match config.role {
            Role::Site => &[
                RegionKind::Process,
                RegionKind::Node,
                RegionKind::Authority,
                RegionKind::ObjectStore,
                RegionKind::Ingest,
                RegionKind::Compute,
                RegionKind::Law,
                RegionKind::Projection,
                RegionKind::Sync,
                RegionKind::Effect,
                RegionKind::Operations,
            ],
            Role::Enterprise => &[
                RegionKind::Process,
                RegionKind::Node,
                RegionKind::Authority,
                RegionKind::ObjectStore,
                RegionKind::Compute,
                RegionKind::Law,
                RegionKind::Projection,
                RegionKind::Sync,
                RegionKind::FinanceBridge,
                RegionKind::Effect,
                RegionKind::Operations,
            ],
            Role::Regional | Role::Relay => &[
                RegionKind::Process,
                RegionKind::Node,
                RegionKind::Authority,
                RegionKind::ObjectStore,
                RegionKind::Compute,
                RegionKind::Law,
                RegionKind::Projection,
                RegionKind::Sync,
                RegionKind::Effect,
                RegionKind::Operations,
            ],
        };
        for &region in base_regions {
            supervisor.start(Child::one(region), start_ns)?;
        }

        let cas_store = if config.persist_segments {
            fs::create_dir_all(config.data_dir.join("segments")).map_err(|e| ServiceError::Io {
                detail: format!("failed to create segments dir: {e}"),
            })?;
            Some(CasStore::open(config.data_dir.join("cas"))?)
        } else {
            None
        };

        let mut node = Self {
            config: config.clone(),
            supervisor,
            mode: NodeMode::Running,
            ledger: L0Ledger::new(),
            known_cells: BTreeSet::new(),
            known_peers: BTreeSet::new(),
            active_leases: BTreeMap::new(),
            segment_paths: BTreeMap::new(),
            segment_writers: BTreeMap::new(),
            cas_store,
            restarts_total: 0,
        };

        for cell in &config.cells {
            node.register_cell(cell, start_ns)?;
        }
        for peer in &config.peers {
            node.start_peer_session(peer, start_ns)?;
        }

        Ok(node)
    }

    #[must_use]
    pub fn node_id(&self) -> &str {
        &self.config.node_id
    }

    #[must_use]
    pub fn role(&self) -> Role {
        self.config.role
    }

    #[must_use]
    pub fn config(&self) -> &NodeConfig {
        &self.config
    }

    #[must_use]
    pub fn mode(&self) -> &NodeMode {
        &self.mode
    }

    #[must_use]
    pub fn supervisor(&self) -> &CompiledSupervisor {
        &self.supervisor
    }

    #[must_use]
    pub fn active_leases(&self) -> &BTreeMap<String, CellLease> {
        &self.active_leases
    }

    #[must_use]
    pub fn ledger(&self) -> &L0Ledger {
        &self.ledger
    }

    #[must_use]
    pub fn cas_store(&self) -> Option<&CasStore> {
        self.cas_store.as_ref()
    }

    #[must_use]
    pub fn segment_path(&self, cell_id: &str) -> Option<&Path> {
        self.segment_paths.get(cell_id).map(PathBuf::as_path)
    }

    #[must_use]
    pub fn restarts_total(&self) -> u64 {
        self.restarts_total
    }

    /// Ensures the node is currently permitted to accept authority writes; refuses with exit `4`
    /// (`ServiceError::ReadOnlyDegraded`) after a `LedgerWriter` escalation, or `ServiceError::Stopped`
    /// after shutdown.
    pub fn ensure_writable(&self) -> Result<(), ServiceError> {
        match &self.mode {
            NodeMode::Stopped => return Err(ServiceError::Stopped),
            NodeMode::ReadOnlyDegraded { failed_cell } => {
                return Err(ServiceError::ReadOnlyDegraded {
                    failed_cell: failed_cell.clone(),
                });
            }
            NodeMode::Running => {}
        }
        self.supervisor.writes().map_err(ServiceError::from)
    }

    /// Registers a new local authority cell, starts its supervised `LedgerWriter{cell}` child,
    /// acquires the initial epoch-1 [`CellLease`], and optionally initializes its on-disk `FMT-SEG-1`
    /// segment.
    pub fn register_cell(&mut self, cell_id: &str, now_ns: u64) -> Result<CellLease, ServiceError> {
        self.ensure_writable()?;
        if cell_id.trim().is_empty() || cell_id.trim() != cell_id {
            return Err(ServiceError::InvalidConfig {
                detail: format!("invalid cell_id '{cell_id}'"),
            });
        }
        if self.known_cells.contains(cell_id) {
            return Err(ServiceError::Ledger(
                fmine_ledger::l0::L0Error::DuplicateCell {
                    cell_id: cell_id.to_string(),
                },
            ));
        }
        // Prepare segment writer on disk BEFORE mutating supervisor/ledger state so an I/O failure
        // leaves no half-registered cell behind.
        let prepared_segment = if self.config.persist_segments {
            let safe_name = sanitize_cell_filename(cell_id);
            let seg_path = self
                .config
                .data_dir
                .join("segments")
                .join(format!("{safe_name}.fmseg"));
            let writer = SemanticSegmentWriter::create(&seg_path, cell_id).map_err(|e| {
                ServiceError::Segment {
                    cell_id: cell_id.to_string(),
                    detail: e.to_string(),
                }
            })?;
            Some((seg_path, writer))
        } else {
            None
        };
        let child = Child::keyed(RegionKind::LedgerWriter, cell_id);
        self.supervisor.start(child, now_ns)?;
        let lease = self.ledger.create_cell(cell_id, &self.config.node_id)?;
        if let Some((seg_path, writer)) = prepared_segment {
            self.segment_paths.insert(cell_id.to_string(), seg_path);
            self.segment_writers.insert(cell_id.to_string(), writer);
        }
        self.known_cells.insert(cell_id.to_string());
        self.active_leases
            .insert(cell_id.to_string(), lease.clone());
        Ok(lease)
    }

    /// Starts a supervised `PeerSession{peer}` child under `SyncRegion`.
    pub fn start_peer_session(&mut self, peer: &str, now_ns: u64) -> Result<(), ServiceError> {
        if self.mode == NodeMode::Stopped {
            return Err(ServiceError::Stopped);
        }
        if peer.trim().is_empty() || peer.trim() != peer {
            return Err(ServiceError::InvalidConfig {
                detail: format!("invalid peer_id '{peer}'"),
            });
        }
        self.supervisor
            .start(Child::keyed(RegionKind::PeerSession, peer), now_ns)?;
        self.known_peers.insert(peer.to_string());
        Ok(())
    }

    /// Starts a supervised `Transfer{root}` child under `PeerSession{peer}`.
    pub fn start_transfer(
        &mut self,
        root_id: &str,
        peer: &str,
        now_ns: u64,
    ) -> Result<(), ServiceError> {
        if self.mode == NodeMode::Stopped {
            return Err(ServiceError::Stopped);
        }
        self.supervisor
            .start(Child::transfer(root_id, peer), now_ns)?;
        Ok(())
    }

    /// Transfers a cell's single-writer lease to `new_holder`, monotonically advancing its fencing
    /// epoch (`PO-BED-1`). Refuses with exit `4` when the node is in read-only degraded mode.
    pub fn transfer_lease(
        &mut self,
        cell_id: &str,
        new_holder: &str,
        now_ns: u64,
    ) -> Result<CellLease, ServiceError> {
        let _ = now_ns;
        self.ensure_writable()?;
        if new_holder.trim().is_empty() || new_holder.trim() != new_holder {
            return Err(ServiceError::InvalidConfig {
                detail: format!("invalid lease holder '{new_holder}'"),
            });
        }
        let next_lease = self.ledger.transfer_lease(cell_id, new_holder)?;
        self.active_leases
            .insert(cell_id.to_string(), next_lease.clone());
        Ok(next_lease)
    }

    /// Commits a pre-constructed [`DeltaBatch`] under an explicit caller-supplied [`CellLease`].
    /// Enforces read-only degraded mode (exit `4`), `PO-BED-1` single-writer epoch fencing, L0
    /// append-only semantics, and optional L1 `FMT-SEG-1` segment persistence.
    pub fn commit_batch(
        &mut self,
        lease: &CellLease,
        batch: DeltaBatch,
        now_ns: u64,
    ) -> Result<CommitReceipt, ServiceError> {
        let _ = now_ns;
        self.ensure_writable()?;
        let cell_id = batch.cell_id.clone();
        let seq = batch.seq;
        let epoch = batch.epoch;
        // Pre-verify active lease fencing BEFORE touching disk so stale leases never write to
        // on-disk segments, and write on-disk segment BEFORE mutating L0 so I/O failures leave L0
        // and disk consistent.
        if let Some(active) = self.active_leases.get(&cell_id)
            && (active.epoch != lease.epoch
                || active.holder_id != lease.holder_id
                || active.cell_id != lease.cell_id)
        {
            return Err(ServiceError::Ledger(
                fmine_ledger::batch::LedgerError::LeaseViolation {
                    detail: format!(
                        "stale or mismatched lease for '{cell_id}': provided (epoch={}, holder='{}') != active (epoch={}, holder='{}')",
                        lease.epoch, lease.holder_id, active.epoch, active.holder_id
                    ),
                }
                .into(),
            ));
        }
        if let Some(writer) = self.segment_writers.get_mut(&cell_id) {
            let _seg_digest = writer.append(&batch).map_err(|e| ServiceError::Segment {
                cell_id: cell_id.clone(),
                detail: e.to_string(),
            })?;
        }
        let digest = self.ledger.commit(lease, batch)?;
        let mmr_root = self.ledger.mmr_root(&cell_id)?;
        Ok(CommitReceipt {
            cell_id,
            epoch,
            seq,
            batch_digest: digest,
            mmr_root,
        })
    }

    /// Builds and commits a single `DeltaKind::Put` batch into `cell_id` using this node's active
    /// [`CellLease`]. Refuses with exit `4` in read-only degraded mode.
    pub fn commit_put(
        &mut self,
        cell_id: &str,
        key: &str,
        payload: Vec<u8>,
        now_ns: u64,
    ) -> Result<CommitReceipt, ServiceError> {
        self.ensure_writable()?;
        let lease = self.active_leases.get(cell_id).cloned().ok_or_else(|| {
            ServiceError::Ledger(fmine_ledger::l0::L0Error::UnknownCell {
                cell_id: cell_id.to_string(),
            })
        })?;
        let seq = self.ledger.committed_count(cell_id)?;
        let prev_hash = self.ledger.head_hash(cell_id)?;
        let batch = DeltaBatch {
            format_version: FMT_LEDGER_V2,
            cell_id: cell_id.to_string(),
            epoch: lease.epoch,
            seq,
            prev_hash,
            anchor: LedgerAnchor {
                format_version: FMT_ANCHOR_V1,
                cell_id: cell_id.to_string(),
                ledger_epoch: lease.epoch,
                commit_sequence: seq,
                adapter_registry_epoch: 1,
                schema_epoch: 1,
                policy_epoch: 1,
                privacy_epoch: 1,
                residency_epoch: 1,
                state_root: sha256(&payload),
            },
            deltas: vec![DeltaOp::new(key, DeltaKind::Put, payload)],
            witnesses: Vec::new(),
            cross_cell_refs: Vec::new(),
        };
        self.commit_batch(&lease, batch, now_ns)
    }

    /// Admits a replicated batch; lower-epoch split-brain batches are quarantined (`F6`) without
    /// mutating the authoritative head (`PO-BED-1`).
    pub fn admit_replica(
        &mut self,
        batch: DeltaBatch,
        now_ns: u64,
    ) -> Result<AdmitOutcome, ServiceError> {
        let _ = now_ns;
        self.ensure_writable()?;
        Ok(self.ledger.admit_replica(batch)?)
    }

    /// Reports a child region failure at virtual timestamp `now_ns` and applies the compiled
    /// supervisor's restart or escalation decision (`BD-079`, plan §26.7).
    pub fn report_failure(&mut self, child: &Child, now_ns: u64) -> Result<Outcome, ServiceError> {
        let outcome = self.supervisor.fail(child, now_ns)?;
        match &outcome {
            Outcome::Restart { .. } => {
                self.restarts_total = self.restarts_total.saturating_add(1);
            }
            Outcome::PeerStale { .. } => {
                // Local authority writers and other peers continue untouched.
            }
            Outcome::ReadOnly { cell, .. } => {
                // Revoke all write leases and drop segment writers immediately so no further bytes
                // can be appended to a potentially torn segment until `fmine doctor` checks chains.
                self.active_leases.clear();
                self.segment_writers.clear();
                self.mode = NodeMode::ReadOnlyDegraded {
                    failed_cell: cell.clone(),
                };
            }
            Outcome::Shutdown => {
                self.active_leases.clear();
                self.segment_writers.clear();
                self.mode = NodeMode::Stopped;
            }
        }
        Ok(outcome)
    }

    /// Gracefully stops the node, releasing leases and closing open segment writers.
    pub fn shutdown(&mut self) {
        self.active_leases.clear();
        self.segment_writers.clear();
        self.mode = NodeMode::Stopped;
    }

    /// Renders a deterministic status JSON snapshot carrying role, mode, running regions, active
    /// leases, stale peers, open obligation count, and mandatory `INV-002` all-false authority flags.
    #[must_use]
    pub fn status_json(&self) -> String {
        let read_only_cell = match &self.mode {
            NodeMode::ReadOnlyDegraded { failed_cell } => {
                format!("\"{}\"", escape_json(failed_cell))
            }
            _ => "null".to_string(),
        };
        let running_rows: Vec<String> = self
            .supervisor
            .running()
            .iter()
            .map(|c| {
                let label = match (&c.key, &c.peer) {
                    (Some(k), Some(p)) if c.region == RegionKind::Transfer => {
                        format!("{:?}({k}@{p})", c.region)
                    }
                    (Some(k), _) => format!("{:?}({k})", c.region),
                    (None, _) => format!("{:?}", c.region),
                };
                format!("\"{}\"", escape_json(&label))
            })
            .collect();
        let leases_json = self.leases_json();
        let mut stale_peers: BTreeSet<&str> = BTreeSet::new();
        for peer in &self.known_peers {
            if self.supervisor.is_stale(peer) {
                stale_peers.insert(peer.as_str());
            }
        }
        let stale_rows: Vec<String> = stale_peers
            .iter()
            .map(|p| format!("\"{}\"", escape_json(p)))
            .collect();
        let mut open_obligations_count = 0usize;
        for cell_id in &self.known_cells {
            if let Ok(obs) = self.ledger.open_obligations(cell_id) {
                open_obligations_count = open_obligations_count.saturating_add(obs.len());
            }
        }
        format!(
            "{{\"schema\":\"{STATUS_SCHEMA}\",\"node_id\":\"{}\",\"role\":\"{}\",\"mode\":\"{}\",\"read_only_cell\":{read_only_cell},\"restarts_total\":{},\"running_regions_count\":{},\"running_regions\":[{}],\"stale_peers\":[{}],\"leases\":{leases_json},\"open_obligations_count\":{open_obligations_count},\"flags\":{{\"reserve_disclosure_claim\":false,\"execution_authorized\":false,\"geotechnical_signoff\":false,\"regulatory_compliance_claim\":false}}}}",
            escape_json(&self.config.node_id),
            role_name(self.config.role),
            self.mode.as_str(),
            self.restarts_total,
            running_rows.len(),
            running_rows.join(","),
            stale_rows.join(","),
        )
    }

    /// Renders deterministic JSON array of active cell leases and their ledger heads.
    #[must_use]
    pub fn leases_json(&self) -> String {
        let rows: Vec<String> = self
            .active_leases
            .values()
            .map(|lease| {
                let committed = self
                    .ledger
                    .committed_count(&lease.cell_id)
                    .unwrap_or_default();
                let head = self
                    .ledger
                    .head_hash(&lease.cell_id)
                    .map(|h| h.to_hex())
                    .unwrap_or_default();
                let mmr = self
                    .ledger
                    .mmr_root(&lease.cell_id)
                    .map(|h| h.to_hex())
                    .unwrap_or_default();
                format!(
                    "{{\"cell_id\":\"{}\",\"epoch\":{},\"holder_id\":\"{}\",\"committed_count\":{committed},\"head_hash\":\"{head}\",\"mmr_root\":\"{mmr}\"}}",
                    escape_json(&lease.cell_id),
                    lease.epoch,
                    escape_json(&lease.holder_id),
                )
            })
            .collect();
        format!("[{}]", rows.join(","))
    }
}

fn sanitize_cell_filename(cell_id: &str) -> String {
    let mut out: String = cell_id
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '-' | '_') {
                c
            } else {
                '_'
            }
        })
        .collect();
    // Append short hash suffix so distinct cell IDs that collide under punctuation sanitization
    // (e.g. `CELL-A/B` vs `CELL-A_B`) always map to distinct segment filenames.
    let tag = &sha256(cell_id.as_bytes()).to_hex()[..8];
    out.push('_');
    out.push_str(tag);
    out
}
