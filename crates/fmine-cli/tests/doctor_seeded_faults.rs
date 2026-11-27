#![allow(clippy::expect_used, clippy::unwrap_used)]
//! End-to-end `BD-117` seeded-fault tests for `fmine doctor`:
//! proves `fmine doctor` detects seeded faults across ALL six planned dimensions:
//! 1. hash chains (`--segment` torn tail & `--journal` corrupt CRC);
//! 2. projection high-water marks (`--hwm` lagging, ahead of authority, and unknown cell);
//! 3. foundation pins (`franken-stack.lock` missing or missing `asupersync` pin);
//! 4. LOC budgets (`registries/loc_budget.toml` crate exceeding ceiling);
//! 5. durable obligations (`--obligations` open `OB-*` leak `PO-BED-5` and corrupt segment);
//! 6. Little's-law ingest queue audit (`--little-queue` `ρ > 0.7`, `ρ >= 1.0`, stalled backlog `PO-BED-7`).

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use fmine_core::anchor::{FMT_ANCHOR_V1, LedgerAnchor};
use fmine_core::hash::sha256;
use fmine_ledger::batch::{DeltaBatch, DeltaKind, DeltaOp, FMT_LEDGER_V2, GENESIS_PREV_HASH};
use fmine_ledger::segment::SemanticSegmentWriter;
use fmine_runtime::obligations::{DurableObligationRegistry, ObligationKind};
use fmine_runtime::region::RegionKind;

fn real_workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("workspace root")
        .to_path_buf()
}

struct TempDirGuard(PathBuf);

impl Drop for TempDirGuard {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn build_clean_temp_workspace(tag: &str) -> TempDirGuard {
    let real_root = real_workspace_root();
    let dir = std::env::temp_dir().join(format!("fmine-cli-doctor-{tag}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(dir.join("registries")).expect("mkdir registries");

    // Copy required core registries so RegistryCatalog::load_from_root succeeds cleanly.
    for file in [
        "units.toml",
        "crs.toml",
        "refusals.toml",
        "verbs.toml",
        "calendars.toml",
        "workspace_topology.toml",
    ] {
        let src = real_root.join("registries").join(file);
        if src.is_file() {
            fs::copy(&src, dir.join("registries").join(file)).expect("copy registry");
        }
    }

    // Minimal valid pins, budgets, and lifts for controlled fault injection.
    fs::write(
        dir.join("franken-stack.lock"),
        "[[foundation]]\nname = \"asupersync\"\npin = \"=0.5.0\"\n",
    )
    .expect("write lock");

    fs::create_dir_all(dir.join("crates/demo-crate/src")).expect("mkdir demo crate");
    fs::write(
        dir.join("crates/demo-crate/src/lib.rs"),
        "pub fn ok() -> u64 { 1 }\n",
    )
    .expect("write demo crate");
    fs::write(
        dir.join("registries/loc_budget.toml"),
        "[[crate]]\nname = \"demo-crate\"\nplanned_loc = 10\nceiling_loc = 15\n",
    )
    .expect("write loc_budget");

    fs::write(
        dir.join("registries/lifts.toml"),
        "[[lift]]\ntarget = \"crates/demo-crate/src/lib.rs\"\n",
    )
    .expect("write lifts");

    TempDirGuard(dir)
}

fn write_valid_segment(path: &Path, cell_id: &str, count: u64) {
    let mut writer = SemanticSegmentWriter::create(path, cell_id).expect("create segment");
    let mut prev_hash = GENESIS_PREV_HASH;
    for seq in 0..count {
        let batch = DeltaBatch {
            format_version: FMT_LEDGER_V2,
            cell_id: cell_id.to_string(),
            epoch: 1,
            seq,
            prev_hash,
            anchor: LedgerAnchor {
                format_version: FMT_ANCHOR_V1,
                cell_id: cell_id.to_string(),
                ledger_epoch: 1,
                commit_sequence: seq,
                adapter_registry_epoch: 1,
                schema_epoch: 1,
                policy_epoch: 1,
                privacy_epoch: 1,
                residency_epoch: 1,
                state_root: sha256(format!("{cell_id}:{seq}").as_bytes()),
            },
            deltas: vec![DeltaOp::new(
                format!("k/{seq:04}"),
                DeltaKind::Put,
                vec![seq as u8],
            )],
            witnesses: Vec::new(),
            cross_cell_refs: Vec::new(),
        };
        prev_hash = writer.append(&batch).expect("append batch");
    }
}

fn run_doctor(root: &Path, extra_args: &[String]) -> (String, u8) {
    let mut args = vec!["--json".to_string(), "doctor".to_string()];
    args.extend_from_slice(extra_args);
    fmine_cli::run(&args, root)
}

#[test]
fn test_doctor_finds_seeded_faults() {
    let guard = build_clean_temp_workspace("seeded");
    let ws = &guard.0;
    let cell_id = "CELL-SITE(lumwana)";

    // Create a healthy 3-record FMT-SEG-1 segment and a quiescent durable OB-* obligation segment.
    let seg_path = ws.join("lumwana.fmseg");
    write_valid_segment(&seg_path, cell_id, 3);

    let ob_path = ws.join("obligations.fmseg");
    {
        let mut reg = DurableObligationRegistry::create(&ob_path, "CELL-OB(site)").expect("ob");
        reg.open_obligation(
            "ob-sync-1",
            ObligationKind::ObSyncAck,
            RegionKind::Sync,
            1_000,
        )
        .expect("open ob");
        reg.resolve("ob-sync-1", 2_000, "acked by enterprise")
            .expect("resolve ob");
    }

    // 0. Clean baseline across ALL six dimensions (including exact boundary rho == 0.70)
    //    reports healthy:true and findings_count:0.
    let (clean_out, clean_code) = run_doctor(
        ws,
        &[
            "--segment".into(),
            seg_path.to_string_lossy().into_owned(),
            "--obligations".into(),
            ob_path.to_string_lossy().into_owned(),
            "--hwm".into(),
            format!("{cell_id}=3"),
            "--little-queue".into(),
            "ingest_telemetry:70:100:14".into(),
        ],
    );
    assert_eq!(clean_code, 0, "clean doctor failed: {clean_out}");
    assert!(clean_out.contains("\"healthy\":true"), "{clean_out}");
    assert!(clean_out.contains("\"findings_count\":0"), "{clean_out}");
    assert!(
        clean_out.contains("\"status\":\"caught_up\""),
        "{clean_out}"
    );
    assert!(
        clean_out.contains("\"status\":\"quiescent\""),
        "{clean_out}"
    );
    assert!(clean_out.contains("\"status\":\"healthy\""), "{clean_out}");

    // 1. Seeded CHAIN faults: torn-tail segment + corrupt journal.
    let torn_seg = ws.join("torn.fmseg");
    fs::copy(&seg_path, &torn_seg).expect("copy seg");
    {
        let mut f = fs::OpenOptions::new()
            .append(true)
            .open(&torn_seg)
            .expect("open torn");
        f.write_all(&[0xDE, 0xAD, 0xBE, 0xEF, 0x01]).expect("torn");
    }
    let bad_journal = ws.join("corrupt.fjr");
    fs::write(&bad_journal, b"not-a-valid-journal-line\n").expect("write bad journal");
    let (chain_out, chain_code) = run_doctor(
        ws,
        &[
            "--segment".into(),
            torn_seg.to_string_lossy().into_owned(),
            "--journal".into(),
            bad_journal.to_string_lossy().into_owned(),
        ],
    );
    assert_eq!(chain_code, 0);
    assert!(chain_out.contains("\"healthy\":false"), "{chain_out}");
    assert!(
        chain_out.contains("\"status\":\"torn_tail\""),
        "{chain_out}"
    );
    assert!(chain_out.contains("chain:segment:"), "{chain_out}");
    assert!(chain_out.contains("chain:journal:"), "{chain_out}");

    // 2. Seeded HWM faults: lagging, ahead_of_authority, and unknown_authority.
    let (hwm_out, _) = run_doctor(
        ws,
        &[
            "--segment".into(),
            seg_path.to_string_lossy().into_owned(),
            "--hwm".into(),
            format!("{cell_id}=1"),
            "--hwm".into(),
            format!("{cell_id}=9"),
            "--hwm".into(),
            "CELL-SITE(missing)=2".into(),
        ],
    );
    assert!(hwm_out.contains("\"healthy\":false"), "{hwm_out}");
    assert!(hwm_out.contains("\"status\":\"lagging\""), "{hwm_out}");
    assert!(hwm_out.contains("\"lag\":2"), "{hwm_out}");
    assert!(
        hwm_out.contains("\"status\":\"ahead_of_authority\""),
        "{hwm_out}"
    );
    assert!(
        hwm_out.contains("\"status\":\"unknown_authority\""),
        "{hwm_out}"
    );

    // 2b. Duplicate --segment for the same cell_id must NOT let a healthy copy mask a torn one
    //     or silently overwrite a conflicting record count.
    let seg_longer = ws.join("lumwana_5.fmseg");
    write_valid_segment(&seg_longer, cell_id, 5);
    let (masked_out, _) = run_doctor(
        ws,
        &[
            "--segment".into(),
            torn_seg.to_string_lossy().into_owned(),
            "--segment".into(),
            seg_path.to_string_lossy().into_owned(),
            "--hwm".into(),
            format!("{cell_id}=3"),
        ],
    );
    assert!(masked_out.contains("\"healthy\":false"), "{masked_out}");
    assert!(
        masked_out.contains("\"status\":\"authority_unhealthy\""),
        "{masked_out}"
    );
    let (conflict_out, _) = run_doctor(
        ws,
        &[
            "--segment".into(),
            seg_path.to_string_lossy().into_owned(),
            "--segment".into(),
            seg_longer.to_string_lossy().into_owned(),
            "--hwm".into(),
            format!("{cell_id}=5"),
        ],
    );
    assert!(conflict_out.contains("\"healthy\":false"), "{conflict_out}");
    assert!(
        conflict_out.contains("conflicting segment record counts"),
        "{conflict_out}"
    );
    assert!(
        conflict_out.contains("\"status\":\"authority_unhealthy\""),
        "{conflict_out}"
    );

    // 3. Seeded OBLIGATION faults (PO-BED-5): open obligation leak + corrupt obligation segment.
    let leaky_ob = ws.join("leaky_ob.fmseg");
    {
        let mut reg =
            DurableObligationRegistry::create(&leaky_ob, "CELL-OB(leak)").expect("leaky ob");
        reg.open_obligation(
            "ob-tarp-99",
            ObligationKind::ObTarp,
            RegionKind::Operations,
            5_000,
        )
        .expect("open tarp");
    }
    let corrupt_ob = ws.join("corrupt_ob.fmseg");
    fs::write(&corrupt_ob, b"corrupt-obligation-segment").expect("write corrupt ob");
    let (ob_out, _) = run_doctor(
        ws,
        &[
            "--obligations".into(),
            leaky_ob.to_string_lossy().into_owned(),
            "--obligations".into(),
            corrupt_ob.to_string_lossy().into_owned(),
        ],
    );
    assert!(ob_out.contains("\"healthy\":false"), "{ob_out}");
    assert!(
        ob_out.contains("\"status\":\"open_obligations\""),
        "{ob_out}"
    );
    assert!(ob_out.contains("\"quiescent\":false"), "{ob_out}");
    assert!(ob_out.contains("ob-tarp-99"), "{ob_out}");
    assert!(ob_out.contains("OB-TARP"), "{ob_out}");
    assert!(ob_out.contains("\"status\":\"corrupt\""), "{ob_out}");

    // 4. Seeded LITTLE'S-LAW queue faults (PO-BED-7): ρ = 0.7001 > 0.7, ρ = 1.10 >= 1.0, stalled backlog.
    let (little_out, _) = run_doctor(
        ws,
        &[
            "--little-queue".into(),
            "haul_bus:70.01:100:40".into(),
            "--little-queue".into(),
            "mill_plc:110:100:250".into(),
            "--little-queue".into(),
            "stalled_q:0:100:12".into(),
        ],
    );
    assert!(little_out.contains("\"healthy\":false"), "{little_out}");
    assert!(
        little_out.contains("\"status\":\"over_target\""),
        "{little_out}"
    );
    assert!(
        little_out.contains("\"status\":\"unstable\""),
        "{little_out}"
    );
    assert!(
        little_out.contains("\"status\":\"stalled_backlog\""),
        "{little_out}"
    );
    assert!(
        little_out.contains("\"implied_wait_s\":null"),
        "stalled backlog must report null implied_wait_s, not 0.0: {little_out}"
    );

    // 5. Seeded BUDGET fault: demo-crate grows past ceiling_loc = 15.
    let bloated: String = (0..25)
        .map(|i| format!("pub const C{i}: u64 = {i};\n"))
        .collect();
    fs::write(ws.join("crates/demo-crate/src/lib.rs"), bloated).expect("bloat crate");
    let (budget_out, _) = run_doctor(ws, &[]);
    assert!(budget_out.contains("\"healthy\":false"), "{budget_out}");
    assert!(
        budget_out.contains("\"over_ceiling\":[\"demo-crate\"]"),
        "{budget_out}"
    );
    assert!(budget_out.contains("budget:demo-crate:"), "{budget_out}");

    // 6. Seeded PINS fault: remove asupersync pin from franken-stack.lock.
    fs::write(
        ws.join("franken-stack.lock"),
        "[[foundation]]\nname = \"other\"\npin = \"0.1.0\"\n",
    )
    .expect("overwrite lock");
    let (pin_out, _) = run_doctor(ws, &[]);
    assert!(pin_out.contains("\"healthy\":false"), "{pin_out}");
    assert!(pin_out.contains("\"asupersync_pin\":null"), "{pin_out}");
    assert!(pin_out.contains("missing asupersync pin"), "{pin_out}");
}
