#![allow(clippy::expect_used, clippy::unwrap_used)]
//! Daemon lab tests for `fmined` (`BD-119`, plan §26.6–§26.7, §30.3):
//! 1. All four node roles (`site`, `regional`, `enterprise`, `relay`) compile their `AppSpec`,
//!    isolate role-specific regions (`Ingest` site-only, `FinanceBridge` enterprise-only), and
//!    enforce all four `INV-002` authority flags = `false`.
//! 2. Virtual-time supervisor restart-intensity tests prove:
//!    - 5 restarts in 60 s stay running; failures spaced > 60 s never falsely escalate;
//!    - 6th `LedgerWriter` failure escalates to `Outcome::ReadOnly`, stops all writers, revokes
//!      leases, and refuses writes/restarts with exit code `4`;
//!    - 6th `PeerSession` failure escalates to `Outcome::PeerStale`, isolating ONLY that peer
//!      while other peers and local authority writes keep running.
//! 3. Single-writer leases and monotonic fencing epochs (`PO-BED-1`), stale-epoch refusal,
//!    split-brain replica quarantine, and `fmine-lab` `check_po_bed_1_single_writer` oracle.
//! 4. Live Unix domain socket `<node>/fmined.sock` IPC persisting verified `FMT-SEG-1` segments,
//!    refusing duplicate bind on live socket, returning `exit_code: 4` in degraded mode, and
//!    cleaning up the socket on shutdown.
//! 5. Real compiled `fmined` binary execution (`--once-status` and invalid-role exit `2`).

use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::thread;

use fmine_core::anchor::{FMT_ANCHOR_V1, LedgerAnchor};
use fmine_core::hash::sha256;
use fmine_lab::oracles::{LeaseEvent, check_po_bed_1_single_writer};
use fmine_ledger::batch::{DeltaBatch, DeltaKind, DeltaOp, FMT_LEDGER_V2};
use fmine_ledger::cell::AdmitOutcome;
use fmine_ledger::verify::verify_segment;
use fmine_runtime::region::{RegionKind, Role};
use fmine_runtime::supervise::{Child, Outcome, RESTARTS, WINDOW_NS};
use fmine_service::ServiceError;
use fmine_service::config::NodeConfig;
use fmine_service::node::{FminedNode, NodeMode};

struct TempDirGuard(PathBuf);

impl Drop for TempDirGuard {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn temp_node_dir(tag: &str) -> TempDirGuard {
    let dir = std::env::temp_dir().join(format!("fmine-service-{tag}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("mkdir");
    TempDirGuard(dir)
}

#[test]
fn test_all_four_roles_compile_appspec_isolate_regions_and_enforce_inv_002() {
    let guard = temp_node_dir("roles");

    for role in Role::ALL {
        let cfg = NodeConfig::new(
            format!("node-{role:?}").to_lowercase(),
            role,
            guard.0.join(format!("{role:?}")),
        )
        .expect("config");
        let node = FminedNode::boot(cfg, 1_000).expect("boot role");

        let running = node.supervisor().running();
        let has_ingest = running.iter().any(|c| c.region == RegionKind::Ingest);
        let has_finance = running
            .iter()
            .any(|c| c.region == RegionKind::FinanceBridge);
        assert_eq!(has_ingest, role == Role::Site, "Ingest must be Site-only");
        assert_eq!(
            has_finance,
            role == Role::Enterprise,
            "FinanceBridge must be Enterprise-only"
        );

        let status = node.status_json();
        assert!(status.contains("\"mode\":\"running\""), "{status}");
        assert!(
            status.contains("\"reserve_disclosure_claim\":false"),
            "{status}"
        );
        assert!(
            status.contains("\"execution_authorized\":false"),
            "{status}"
        );
        assert!(
            status.contains("\"geotechnical_signoff\":false"),
            "{status}"
        );
        assert!(
            status.contains("\"regulatory_compliance_claim\":false"),
            "{status}"
        );
    }
}

#[test]
fn test_supervisor_restart_intensity_escalates_writer_to_read_only_exit_4_and_isolates_stale_peer()
{
    let guard = temp_node_dir("supervise");

    // 1. Writer failures spaced > WINDOW_NS apart stay running; 6 within WINDOW_NS -> ReadOnly (exit 4).
    let mut cfg = NodeConfig::new("site-lumwana", Role::Site, guard.0.join("site")).expect("cfg");
    cfg.cells = vec![
        "CELL-SITE(lumwana)".to_string(),
        "CELL-FLOW(lumwana)".to_string(),
    ];
    cfg.peers = vec!["peer-ent".to_string(), "peer-reg".to_string()];
    let mut node = FminedNode::boot(cfg, 1_000).expect("boot");

    let writer_lum = Child::keyed(RegionKind::LedgerWriter, "CELL-SITE(lumwana)");
    let mut now = 10_000_000_000u64;

    // 3 failures spaced > 60s apart do NOT accumulate toward escalation.
    for _ in 0..3 {
        let out = node
            .report_failure(&writer_lum, now)
            .expect("spaced failure");
        assert!(matches!(out, Outcome::Restart { .. }));
        now += WINDOW_NS + 1_000_000;
    }
    assert_eq!(node.mode(), &NodeMode::Running);
    node.commit_put("CELL-SITE(lumwana)", "k/ok", b"v0".to_vec(), now)
        .expect("still writable after spaced restarts");

    // Now burst RESTARTS (5) failures within 60s -> all restart.
    for i in 0..RESTARTS {
        let out = node
            .report_failure(&writer_lum, now + (i as u64 + 1) * 1_000_000)
            .expect("burst restart");
        assert!(matches!(out, Outcome::Restart { .. }));
    }
    assert_eq!(node.mode(), &NodeMode::Running);

    // 6th failure within 60s -> escalates to ReadOnly, stops ALL ledger writers, revokes leases!
    let escalate_ns = now + (RESTARTS as u64 + 1) * 1_000_000;
    let escalated = node
        .report_failure(&writer_lum, escalate_ns)
        .expect("escalate");
    assert!(
        matches!(
            &escalated,
            Outcome::ReadOnly { cell, stopped }
                if cell == "CELL-SITE(lumwana)" && stopped.len() == 2
        ),
        "expected ReadOnly stopping both writers, got {escalated:?}"
    );
    assert_eq!(
        node.mode(),
        &NodeMode::ReadOnlyDegraded {
            failed_cell: "CELL-SITE(lumwana)".to_string()
        }
    );
    assert!(node.active_leases().is_empty());

    // Every write / lease transfer / new cell registration refuses with exit code 4!
    let err_write = node
        .commit_put(
            "CELL-FLOW(lumwana)",
            "k/blocked",
            b"no".to_vec(),
            escalate_ns + 1,
        )
        .expect_err("write must refuse in degraded mode");
    assert_eq!(err_write.exit_code(), 4);
    assert!(matches!(err_write, ServiceError::ReadOnlyDegraded { .. }));

    let err_reg = node
        .register_cell("CELL-SITE(new)", escalate_ns + 2)
        .expect_err("new writer must refuse in degraded mode");
    assert_eq!(err_reg.exit_code(), 4);

    let err_xfer = node
        .transfer_lease("CELL-SITE(lumwana)", "node-b", escalate_ns + 3)
        .expect_err("lease transfer must refuse in degraded mode");
    assert_eq!(err_xfer.exit_code(), 4);

    // Status remains readable and reports read_only_degraded + failed cell.
    let status = node.status_json();
    assert!(
        status.contains("\"mode\":\"read_only_degraded\""),
        "{status}"
    );
    assert!(
        status.contains("\"read_only_cell\":\"CELL-SITE(lumwana)\""),
        "{status}"
    );

    // 2. PeerSession escalation marks ONLY that peer stale; other peers & local authority run on!
    let mut cfg2 = NodeConfig::new("site-kibali", Role::Site, guard.0.join("kibali")).expect("cfg");
    cfg2.cells = vec!["CELL-SITE(kibali)".to_string()];
    cfg2.peers = vec!["peer-bad".to_string(), "peer-good".to_string()];
    let mut node2 = FminedNode::boot(cfg2, 1_000).expect("boot2");
    node2
        .start_transfer("root-abc", "peer-bad", 2_000)
        .expect("start transfer");

    let bad_peer = Child::keyed(RegionKind::PeerSession, "peer-bad");
    for i in 0..RESTARTS {
        let out = node2
            .report_failure(&bad_peer, 10_000 + (i as u64) * 1_000)
            .expect("peer restart");
        assert!(matches!(out, Outcome::Restart { .. }));
    }
    let stale_out = node2
        .report_failure(&bad_peer, 20_000)
        .expect("peer stale escalation");
    assert!(matches!(
        &stale_out,
        Outcome::PeerStale { peer, stopped } if peer == "peer-bad" && stopped.len() == 2
    ));
    assert_eq!(node2.mode(), &NodeMode::Running);
    assert!(node2.supervisor().is_stale("peer-bad"));
    assert!(!node2.supervisor().is_stale("peer-good"));
    // Local cell writes still succeed cleanly while peer-bad is stale!
    let receipt = node2
        .commit_put("CELL-SITE(kibali)", "prod/shift1", b"ok".to_vec(), 25_000)
        .expect("local authority must remain writable when peer is stale");
    assert_eq!(receipt.seq, 0);
    assert!(
        node2
            .status_json()
            .contains("\"stale_peers\":[\"peer-bad\"]")
    );
}

#[test]
fn test_leases_fencing_epochs_and_po_bed_1_oracle() {
    let guard = temp_node_dir("leases");
    let cell = "CELL-SITE(carlin)";
    let mut cfg = NodeConfig::new("node-primary", Role::Site, guard.0.join("carlin")).expect("cfg");
    cfg.cells = vec![cell.to_string()];
    let mut node = FminedNode::boot(cfg, 1_000).expect("boot");

    let lease1 = node.active_leases()[cell].clone();
    assert_eq!(lease1.epoch, 1);

    let mut events = vec![LeaseEvent::GrantLease {
        cell_id: cell.to_string(),
        epoch: lease1.epoch,
        holder_id: lease1.holder_id.clone(),
    }];

    // Epoch 1 commit succeeds.
    let r0 = node
        .commit_put(cell, "assay/001", b"au=3.2".to_vec(), 2_000)
        .expect("epoch 1 commit");
    assert_eq!((r0.epoch, r0.seq), (1, 0));
    events.push(LeaseEvent::ApplyBatch {
        cell_id: cell.to_string(),
        epoch: lease1.epoch,
        holder_id: lease1.holder_id.clone(),
        quarantined: false,
    });

    // Transfer lease -> epoch monotonically advances to 2.
    let lease2 = node
        .transfer_lease(cell, "node-standby", 3_000)
        .expect("transfer lease");
    assert_eq!(lease2.epoch, 2);
    events.push(LeaseEvent::GrantLease {
        cell_id: cell.to_string(),
        epoch: lease2.epoch,
        holder_id: lease2.holder_id.clone(),
    });

    // Stale epoch-1 commit attempt with old lease1 is refused (FMINE-REF-502)!
    let prev = node.ledger().head_hash(cell).expect("head");
    let stale_batch = DeltaBatch {
        format_version: FMT_LEDGER_V2,
        cell_id: cell.to_string(),
        epoch: lease1.epoch,
        seq: 1,
        prev_hash: prev,
        anchor: LedgerAnchor {
            format_version: FMT_ANCHOR_V1,
            cell_id: cell.to_string(),
            ledger_epoch: lease1.epoch,
            commit_sequence: 1,
            adapter_registry_epoch: 1,
            schema_epoch: 1,
            policy_epoch: 1,
            privacy_epoch: 1,
            residency_epoch: 1,
            state_root: sha256(b"stale"),
        },
        deltas: vec![DeltaOp::new("assay/stale", DeltaKind::Put, b"bad".to_vec())],
        witnesses: Vec::new(),
        cross_cell_refs: Vec::new(),
    };
    let fenced_err = node
        .commit_batch(&lease1, stale_batch.clone(), 4_000)
        .expect_err("stale lease commit must be fenced");
    assert_eq!(fenced_err.to_refusal().code, "FMINE-REF-502");
    assert_eq!(node.ledger().committed_count(cell).expect("cnt"), 1);

    // Split-brain replica arrival from epoch 1 is quarantined without mutating head (F6).
    let admit = node
        .admit_replica(stale_batch, 5_000)
        .expect("admit replica");
    assert!(matches!(admit, AdmitOutcome::Quarantined { .. }));
    assert_eq!(node.ledger().committed_count(cell).expect("cnt"), 1);
    events.push(LeaseEvent::ApplyBatch {
        cell_id: cell.to_string(),
        epoch: lease1.epoch,
        holder_id: lease1.holder_id.clone(),
        quarantined: true,
    });

    // Epoch 2 commit succeeds.
    let r1 = node
        .commit_put(cell, "assay/002", b"au=4.1".to_vec(), 6_000)
        .expect("epoch 2 commit");
    assert_eq!((r1.epoch, r1.seq), (2, 1));
    events.push(LeaseEvent::ApplyBatch {
        cell_id: cell.to_string(),
        epoch: lease2.epoch,
        holder_id: lease2.holder_id.clone(),
        quarantined: false,
    });

    // PO-BED-1 oracle verifies zero fencing violations across the full lease lifecycle!
    assert!(check_po_bed_1_single_writer(&events).is_empty());
}

#[cfg(unix)]
#[test]
fn test_unix_socket_ipc_persists_verified_fmt_seg_1_and_enforces_degraded_exit_4() {
    use fmine_service::socket::{FminedClient, FminedServer};

    let guard = temp_node_dir("ipc");
    let cell = "CELL-SITE(pueblo_viejo)";
    let mut cfg = NodeConfig::new("site-pv", Role::Site, &guard.0).expect("cfg");
    cfg.cells = vec![cell.to_string()];
    cfg.persist_segments = true;

    let node = FminedNode::boot(cfg, 1_000).expect("boot");
    let seg_path = node.segment_path(cell).expect("segment path").to_path_buf();
    let sock_path = node.config().socket_path.clone();

    let shared = Arc::new(Mutex::new(node));
    let server = FminedServer::bind(Arc::clone(&shared)).expect("bind socket");

    // A second daemon trying to bind the same live socket MUST be refused with SocketInUse!
    let dup_err = FminedServer::bind(Arc::clone(&shared))
        .err()
        .expect("duplicate bind on live socket must refuse");
    assert!(matches!(dup_err, ServiceError::SocketInUse { .. }));

    let handle = thread::spawn(move || server.serve_until_shutdown());

    // 1. Ping & status over Unix socket.
    let ping = FminedClient::send_request(&sock_path, "{\"op\":\"ping\"}").expect("ping");
    assert!(ping.contains("\"ok\":true"), "{ping}");
    assert!(ping.contains("\"node_id\":\"site-pv\""), "{ping}");

    let status = FminedClient::send_request(&sock_path, "{\"op\":\"status\"}").expect("status");
    assert!(
        status.contains("\"schema\":\"fmine.fmined.status.v1\""),
        "{status}"
    );
    assert!(
        status.contains("\"reserve_disclosure_claim\":false"),
        "{status}"
    );

    // 2. Commit two batches over socket; while fmined is actively writing, its SemanticSegmentWriter
    //    holds an exclusive OS lock on the FMT-SEG-1 file!
    let c0 = FminedClient::send_request(
        &sock_path,
        "{\"op\":\"commit\",\"cell_id\":\"CELL-SITE(pueblo_viejo)\",\"key\":\"lot/1\",\"value\":\"dry_t=1250\"}",
    )
    .expect("commit 0");
    assert!(c0.contains("\"ok\":true"), "{c0}");
    assert!(c0.contains("\"seq\":0"), "{c0}");

    let c1 = FminedClient::send_request(
        &sock_path,
        "{\"op\":\"commit\",\"cell_id\":\"CELL-SITE(pueblo_viejo)\",\"key\":\"lot/2\",\"value\":\"dry_t=1310\"}",
    )
    .expect("commit 1");
    assert!(c1.contains("\"ok\":true"), "{c1}");
    assert!(c1.contains("\"seq\":1"), "{c1}");

    assert!(
        verify_segment(&seg_path).is_err(),
        "live SemanticSegmentWriter must hold exclusive lock while node is writable"
    );

    let leases = FminedClient::send_request(&sock_path, "{\"op\":\"leases\"}").expect("leases");
    assert!(leases.contains("\"committed_count\":2"), "{leases}");

    // 3. Trigger writer escalation -> fmined drops segment writers (releasing the lock so
    //    `fmine doctor` / `verify_segment` can inspect the hash chain per §26.7!), refuses socket
    //    writes with exit_code: 4, and keeps serving read-only status!
    {
        let mut locked = shared.lock().expect("lock");
        let writer = Child::keyed(RegionKind::LedgerWriter, cell);
        for i in 0..=RESTARTS {
            let _ = locked
                .report_failure(&writer, 50_000 + (i as u64) * 1_000)
                .expect("fail");
        }
    }
    let verified = verify_segment(&seg_path)
        .expect("segment lock must be released on ReadOnly so doctor can verify hash chain");
    assert_eq!(verified.cell_id, cell);
    assert_eq!(verified.record_count, 2);
    assert_eq!(verified.torn_bytes, 0);

    let blocked = FminedClient::send_request(
        &sock_path,
        "{\"op\":\"commit\",\"cell_id\":\"CELL-SITE(pueblo_viejo)\",\"key\":\"lot/3\",\"value\":\"blocked\"}",
    )
    .expect("blocked commit response");
    assert!(blocked.contains("\"ok\":false"), "{blocked}");
    assert!(blocked.contains("\"exit_code\":4"), "{blocked}");

    let degraded_status =
        FminedClient::send_request(&sock_path, "{\"op\":\"status\"}").expect("degraded status");
    assert!(
        degraded_status.contains("\"mode\":\"read_only_degraded\""),
        "{degraded_status}"
    );

    // 4. Malformed request refuses cleanly without crashing server.
    let bad = FminedClient::send_request(&sock_path, "not-json").expect("bad response");
    assert!(bad.contains("\"ok\":false"), "{bad}");
    assert!(bad.contains("\"code\":\"FMINE-REF-208\""), "{bad}");

    // 5. Shutdown stops server cleanly, refuses further writes, and removes socket file on Drop!
    let bye = FminedClient::send_request(&sock_path, "{\"op\":\"shutdown\"}").expect("shutdown");
    assert!(bye.contains("\"shutting_down\""), "{bye}");
    handle
        .join()
        .expect("server thread join")
        .expect("server exit ok");
    assert!(
        !sock_path.exists(),
        "fmined.sock must be removed cleanly on shutdown"
    );
    {
        let mut locked = shared.lock().expect("lock after shutdown");
        assert!(matches!(
            locked.register_cell("CELL-SITE(after_stop)", 99_000),
            Err(ServiceError::Stopped)
        ));
    }

    // 6. Server refuses to overwrite a regular non-socket file at socket_path.
    let regular_file = guard.0.join("do_not_delete.txt");
    fs::write(&regular_file, b"keep-me").expect("write regular file");
    let mut cfg_reg = NodeConfig::new("site-guard", Role::Site, guard.0.join("g")).expect("cfg");
    cfg_reg.socket_path = regular_file.clone();
    let guard_node = Arc::new(Mutex::new(FminedNode::boot(cfg_reg, 1_000).expect("boot")));
    assert!(FminedServer::bind(guard_node).is_err());
    assert_eq!(fs::read(&regular_file).expect("still intact"), b"keep-me");
}

#[test]
fn test_fmined_binary_once_status_across_roles() {
    let guard = temp_node_dir("bin");
    let bin = env!("CARGO_BIN_EXE_fmined");

    let out_site = Command::new(bin)
        .args([
            "--role",
            "site",
            "--node-id",
            "site-reko-diq",
            "--data-dir",
            &guard.0.join("site").to_string_lossy(),
            "--cell",
            "CELL-SITE(reko_diq)",
            "--peer",
            "peer-enterprise",
            "--once-status",
        ])
        .output()
        .expect("run fmined site");
    assert!(
        out_site.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out_site.stderr)
    );
    let stdout = String::from_utf8(out_site.stdout).expect("utf8");
    assert!(stdout.contains("\"role\":\"site\""), "{stdout}");
    assert!(stdout.contains("CELL-SITE(reko_diq)"), "{stdout}");
    assert!(stdout.contains("\"Ingest\""), "{stdout}");
    assert!(!stdout.contains("\"FinanceBridge\""), "{stdout}");

    let out_ent = Command::new(bin)
        .args([
            "--role",
            "enterprise",
            "--node-id",
            "ent-toronto",
            "--data-dir",
            &guard.0.join("ent").to_string_lossy(),
            "--once-status",
        ])
        .output()
        .expect("run fmined ent");
    assert!(out_ent.status.success());
    let stdout_ent = String::from_utf8(out_ent.stdout).expect("utf8");
    assert!(
        stdout_ent.contains("\"role\":\"enterprise\""),
        "{stdout_ent}"
    );
    assert!(stdout_ent.contains("\"FinanceBridge\""), "{stdout_ent}");
    assert!(!stdout_ent.contains("\"Ingest\""), "{stdout_ent}");

    // Unknown role exits 2 (usage error).
    let out_bad = Command::new(bin)
        .args([
            "--role",
            "invalid-role",
            "--node-id",
            "x",
            "--data-dir",
            &guard.0.to_string_lossy(),
            "--once-status",
        ])
        .output()
        .expect("run fmined invalid");
    assert_eq!(out_bad.status.code(), Some(2));
}
