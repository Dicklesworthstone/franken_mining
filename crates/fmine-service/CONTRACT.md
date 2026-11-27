# CONTRACT.md — `fmine-service`

## 1. Purpose
Node composition crate producing the `fmined` daemon binary (`BD-119`, plan §26.6–§26.7, §30.3). One `fmined` runs per node in one of four explicit roles (`site`, `regional`, `enterprise`, `relay`), compiling its role `AppSpec` into `fmine_runtime::supervise::CompiledSupervisor`, holding single-writer cell leases with monotonic fencing epochs (`fmine-ledger`, `PO-BED-1`), optionally persisting `FMT-SEG-1` segments and `FMT-CAS-1` objects, enforcing read-only degraded mode (exit `4`) when a ledger writer escalates, isolating stale peers without halting local authority, and serving bounded local NDJSON IPC over `<node>/fmined.sock`.

- **Crate:** `fmine-service`
- **Layer:** V
- **Subsystem:** Adit
- **LOC budget:** 15,000 planned / 17,000 ceiling
- **Allowed Module Families:** `["config", "node", "socket"]`

## 2. Inputs
- CLI arguments / `config::NodeConfig`: `node_id`, `role` (`site | regional | enterprise | relay`), `data_dir`, optional `socket_path` (default `<data_dir>/fmined.sock`), initial local `cells`, initial `peers`, and `persist_segments` toggle.
- Explicit monotonic virtual timestamps (`now_ns: u64`) on `node::FminedNode` state transitions (so lab tests drive restart windows and escalations deterministically without ambient wall clocks).
- Bounded line-delimited JSON requests over `<node>/fmined.sock` (`ping`, `status`, `leases`, `commit`, `shutdown`; max 64 KiB per line).
- Dependencies: `fmine-core`, `fmine-evidence`, `fmine-ledger`, `fmine-object`, `fmine-runtime` (strictly downward `V -> S1 / S0`; zero external crates). Dev-dependency: `fmine-lab` for `PO-BED-1` oracle verification in lab tests.

## 3. Outputs
- Binary `fmined` serving `<node>/fmined.sock` or emitting deterministic `--once-status` JSON (`fmine.fmined.status.v1`).
- `node::FminedNode` state transitions (`CellLease`, `CommitReceipt`, `Outcome`, deterministic `status_json`), on-disk `FMT-SEG-1` segments under `<data_dir>/segments/` when persistence is enabled, and structured `ServiceError` mapped to registered `FMINE-REF-*` refusal codes and §30.1 exit codes (including exit `4` for read-only degraded write refusals).
- Every status envelope sets all four `INV-002` authority flags (`reserve_disclosure_claim`, `execution_authorized`, `geotechnical_signoff`, `regulatory_compliance_claim`) to `false`.

## 4. Invariants
- **Role isolation (`BD-079`, §26.6):** `Ingest` runs only on `Role::Site`; `FinanceBridge` runs only on `Role::Enterprise`; `Role::Regional` and `Role::Relay` start neither (`daemon_lab::test_all_four_roles_compile_appspec_isolate_regions_and_enforce_inv_002`).
- **INV-002 (no authority claim):** all four authority flags are unconditionally `false` in every status payload.
- **PO-BED-1 (single-writer leases and fencing):** each local cell has at most one active writer lease per epoch; lease transfer monotonically increments the epoch; stale-epoch commits are refused (`FMINE-REF-502`) and lower-epoch replica batches are quarantined (`F6`) without mutating cell state (`daemon_lab::test_leases_fencing_epochs_and_po_bed_1_oracle`).
- **Read-only degraded mode (`BD-079`, §26.7):** after `RESTARTS` (5) restarts within `WINDOW_NS` (60 s), the next `LedgerWriter` failure escalates to `Outcome::ReadOnly`, stops all ledger writers, revokes all active write leases, closes segment writer handles, and refuses all subsequent writes and writer restarts with exit `4` until a new process starts after `fmine doctor` verifies the hash chain (`daemon_lab::test_supervisor_restart_intensity_escalates_writer_to_read_only_exit_4_and_isolates_stale_peer`).
- **Peer-stale isolation:** `PeerSession` escalation marks only that peer stale while local authority writes and other peers remain healthy.
- **Socket exclusivity & cleanup:** `socket::FminedServer` refuses to overwrite a live listening socket (`SocketInUse`) and removes its socket path cleanly on drop (`daemon_lab::test_unix_socket_ipc_persists_verified_fmt_seg_1_and_enforces_degraded_exit_4`).

## 5. Refusals
- `ServiceError::ReadOnlyDegraded` -> exit `4` (`FMINE-REF-502`): write, cell registration, or lease transfer attempted after ledger writer escalation.
- `ServiceError::Ledger` / `ServiceError::L0` -> preserves underlying `fmine-ledger` refusal codes (`FMINE-REF-502` on stale lease/epoch, `FMINE-REF-101` on conservation, `FMINE-REF-505` on chain/digest failure).
- `ServiceError::Supervisor` -> exit `4` (`FMINE-REF-502`) for `ReadOnly`, `FMINE-REF-502` for stopped/lifecycle conflicts, `FMINE-REF-107` for invalid role/region shape.
- `ServiceError::SocketInUse` / `ServiceError::Io` / `ServiceError::Protocol` -> `FMINE-REF-506` (foundation I/O) or `FMINE-REF-208` (malformed IPC request).
- CLI usage errors in `fmined` -> exit `2`.

## 6. Determinism class
`config` and `node` (`FminedNode` state machine) are `BitwisePortable` given identical inputs and explicit `now_ns` timestamps (ordered `BTreeMap`/`BTreeSet` state, zero ambient clocks or RNG). `socket` and `main` perform local OS socket/filesystem I/O and wall-clock stamping for live IPC requests.

## 7. Evidence class of outputs
Operational node status and ledger commit receipts; emits no domain `Claim`s and never elevates evidence classes (`INV-006`).

## 8. Validity domain
- Single-node local daemon serving `<node>/fmined.sock` over Unix domain sockets (`#[cfg(unix)]`).
- Does not implement multi-host QUIC/ATP wire transport (`fmine-sync`), HTTP/MCP serving (`fmine-mcp`), or TUI rendering (`fmine-tui`).

## 9. Oracles
- `fmine_lab::oracles::check_po_bed_1_single_writer` verifying zero fencing violations over multi-transfer lease traces.
- `fmine_ledger::verify::verify_segment` verifying on-disk `FMT-SEG-1` segments written via live socket commits.
- Virtual-time supervisor lab suite and real `fmined` binary execution tests in `tests/daemon_lab.rs`.

## 10. Known gaps
- Live peer QUIC/ATP sync sessions and remote replication scheduling await `INT-ATP-001` (`BD-110`).
- Live `fmine doctor --socket` / `fmine-tui` streaming feed integration will attach to `fmined.sock` in downstream WP-12 / WP-17 beads (`BD-202`, `BD-245`).

## 11. Public API
- `ServiceError`
- `config::{NodeConfig, CliArgs, parse_role, role_name, parse_cli_args}`
- `node::{STATUS_SCHEMA, NodeMode, CommitReceipt, FminedNode}`
- `socket::{MAX_IPC_LINE_BYTES, FminedServer, FminedClient}` (`#[cfg(unix)]`)
- Binary `fmined` (`--role`, `--node-id`, `--data-dir`, `--socket`, `--cell`, `--peer`, `--once-status`).

## 12. Proof obligations
- `BD-119` (`Daemon lab tests`, `tests/daemon_lab.rs`):
  - `daemon_lab::test_all_four_roles_compile_appspec_isolate_regions_and_enforce_inv_002`
  - `daemon_lab::test_supervisor_restart_intensity_escalates_writer_to_read_only_exit_4_and_isolates_stale_peer`
  - `daemon_lab::test_leases_fencing_epochs_and_po_bed_1_oracle`
  - `daemon_lab::test_unix_socket_ipc_persists_verified_fmt_seg_1_and_enforces_degraded_exit_4`
  - `daemon_lab::test_fmined_binary_once_status_across_roles`
