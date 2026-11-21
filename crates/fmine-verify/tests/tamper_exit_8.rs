//! Integration tamper tests executing the real `fmine-verify` binary (`BD-140`, `GOAL-024`).
//!
//! Proves exit code `0` on valid `FMT-CERT-1`, `FMT-CAS-1`, `SyncCert`, and `FMT-SEG-1` artifacts,
//! exit code **`8`** (`FMINE-REF-505`) on single-bit flips / missing chunks / torn tails, and exit
//! code `5` (`FMINE-REF-506`) on missing top-level files.

#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use fmine_core::anchor::{FMT_ANCHOR_V1, LedgerAnchor};
use fmine_core::cert::{Certificate, DeterminismClass, InputDigest};
use fmine_core::hash::{blake3_hash, sha256};
use fmine_ledger::batch::{DeltaBatch, DeltaKind, DeltaOp, FMT_LEDGER_V1, GENESIS_PREV_HASH};
use fmine_ledger::mmr::Mmr;
use fmine_ledger::segment::SegmentWriter;
use fmine_object::store::CasStore;
use fmine_sync::sync_cert::SyncCert;

struct TempDir(PathBuf);

impl TempDir {
    fn new(tag: &str) -> Self {
        let dir =
            std::env::temp_dir().join(format!("fmine-verify-test-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("create temp dir");
        Self(dir)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn run_bin(args: &[&str]) -> (i32, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_fmine-verify"))
        .args(args)
        .output()
        .expect("spawn fmine-verify");
    let code = out.status.code().unwrap_or(-1);
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    (code, stdout)
}

#[test]
fn test_fmine_verify_cert_roundtrip_and_tamper_exits_8() {
    let tmp = TempDir::new("cert");
    let inputs_dir = tmp.path().join("inputs");
    fs::create_dir_all(&inputs_dir).expect("mkdir inputs");
    let wb_bytes = b"synthetic-workbook-bytes-v1";
    fs::write(inputs_dir.join("wb.xlsx"), wb_bytes).expect("write input");

    let anchor = LedgerAnchor {
        format_version: FMT_ANCHOR_V1,
        cell_id: "CELL-PUBLIC(barrick)".to_string(),
        ledger_epoch: 1,
        commit_sequence: 3,
        adapter_registry_epoch: 1,
        schema_epoch: 1,
        policy_epoch: 1,
        privacy_epoch: 1,
        residency_epoch: 1,
        state_root: blake3_hash(b"root-3"),
    };
    let cert = Certificate::build(
        "public_assurance",
        "3274dc2",
        DeterminismClass::BitwisePortable,
        7,
        vec![InputDigest::new("workbook", "wb.xlsx", sha256(wb_bytes))],
        sha256(b"report-out"),
        Some(anchor),
    )
    .expect("build cert");
    let cert_bytes = cert.to_bytes().expect("encode cert");
    let cert_path = tmp.path().join("run.fcert");
    fs::write(&cert_path, &cert_bytes).expect("write cert");

    // 1. Clean cert + matching inputs directory exits 0.
    let (code, stdout) = run_bin(&[
        "cert",
        cert_path.to_str().unwrap(),
        "--inputs-dir",
        inputs_dir.to_str().unwrap(),
    ]);
    assert_eq!(code, 0, "stdout={stdout}");
    assert!(stdout.contains("\"status\":\"verified\""), "{stdout}");
    assert!(
        stdout.contains(&cert.certificate_digest.to_hex()),
        "{stdout}"
    );

    // 2. Flip 1 bit in the certificate bytes -> exit 8 (FMINE-REF-505).
    let mut bad_cert = cert_bytes.clone();
    bad_cert[20] ^= 0x01;
    let bad_cert_path = tmp.path().join("tampered.fcert");
    fs::write(&bad_cert_path, &bad_cert).expect("write bad cert");
    let (code, stdout) = run_bin(&["cert", bad_cert_path.to_str().unwrap()]);
    assert_eq!(code, 8, "tampered cert must exit 8: {stdout}");
    assert!(stdout.contains("FMINE-REF-505"), "{stdout}");

    // 3. Flip 1 bit in a declared input file -> exit 8 (FMINE-REF-505).
    fs::write(inputs_dir.join("wb.xlsx"), b"synthetic-workbook-bytes-v2").expect("tamper input");
    let (code, stdout) = run_bin(&[
        "cert",
        cert_path.to_str().unwrap(),
        "--inputs-dir",
        inputs_dir.to_str().unwrap(),
    ]);
    assert_eq!(code, 8, "tampered input file must exit 8: {stdout}");
    assert!(stdout.contains("FMINE-REF-505"), "{stdout}");

    // 4. Missing top-level file -> exit 5 (FMINE-REF-506).
    let missing = tmp.path().join("does-not-exist.fcert");
    let (code, stdout) = run_bin(&["cert", missing.to_str().unwrap()]);
    assert_eq!(code, 5, "missing file must exit 5: {stdout}");
    assert!(stdout.contains("FMINE-REF-506"), "{stdout}");
}

#[test]
fn test_fmine_verify_cas_root_and_chunk_tamper_exits_8() {
    let tmp = TempDir::new("cas");
    let cas_dir = tmp.path().join("cas");
    let store = CasStore::open(&cas_dir).expect("open cas");
    let payload: Vec<u8> = (0u32..50_000).map(|i| (i % 251) as u8).collect();
    let manifest = store
        .put_bytes("application/x-fmine-test", &payload, 8192)
        .expect("put cas object");
    let root_hex = manifest.root_digest.to_hex();

    // 1. Clean CAS root exits 0.
    let (code, stdout) = run_bin(&["cas", cas_dir.to_str().unwrap(), &root_hex]);
    assert_eq!(code, 0, "clean cas must exit 0: {stdout}");
    assert!(stdout.contains("\"status\":\"verified\""), "{stdout}");

    // 2. Flip 1 bit in one chunk file -> exit 8.
    let chunk_path = store.chunk_path(&manifest.chunks[2].sha256);
    let mut chunk_data = fs::read(&chunk_path).expect("read chunk");
    chunk_data[0] ^= 0x80;
    fs::write(&chunk_path, &chunk_data).expect("write tampered chunk");
    let (code, stdout) = run_bin(&["cas", cas_dir.to_str().unwrap(), &root_hex]);
    assert_eq!(code, 8, "tampered CAS chunk must exit 8: {stdout}");
    assert!(stdout.contains("FMINE-REF-505"), "{stdout}");

    // 3. Flip 1 bit in the manifest itself -> exit 8.
    let manifest_path = store.manifest_path(&manifest.root_digest);
    let mut mbytes = fs::read(&manifest_path).expect("read manifest");
    mbytes[12] ^= 0x01;
    fs::write(&manifest_path, &mbytes).expect("write tampered manifest");
    let (code, stdout) = run_bin(&["cas", cas_dir.to_str().unwrap(), &root_hex]);
    assert_eq!(code, 8, "tampered CAS manifest must exit 8: {stdout}");
    assert!(stdout.contains("FMINE-REF-505"), "{stdout}");
}

#[test]
fn test_fmine_verify_synccert_mmr_proof_and_tamper_exits_8() {
    let tmp = TempDir::new("synccert");
    let mut mmr = Mmr::new();
    for i in 0u64..9 {
        mmr.push(format!("batch-{i}").as_bytes()).expect("push mmr");
    }
    let cert = SyncCert::build_from_mmr("CELL-EVID(site-a)", 2, &mmr, 4, 9).expect("build cert");
    let bytes = cert.to_bytes();
    let sc_path = tmp.path().join("sync.scert");
    fs::write(&sc_path, &bytes).expect("write synccert");

    // 1. Clean SyncCert exits 0.
    let (code, stdout) = run_bin(&["synccert", sc_path.to_str().unwrap()]);
    assert_eq!(code, 0, "clean synccert must exit 0: {stdout}");
    assert!(stdout.contains("\"status\":\"verified\""), "{stdout}");

    // 2. Tampered proof hash inside valid CRC frame -> exit 8.
    let mut bad = cert.clone();
    bad.consistency_proof[0].0[0] ^= 0x01;
    let bad_path = tmp.path().join("bad-proof.scert");
    fs::write(&bad_path, bad.to_bytes()).expect("write bad synccert");
    let (code, stdout) = run_bin(&["synccert", bad_path.to_str().unwrap()]);
    assert_eq!(code, 8, "tampered MMR proof must exit 8: {stdout}");
    assert!(stdout.contains("FMINE-REF-505"), "{stdout}");

    // 3. Raw 1-bit flip breaking CRC32C -> exit 8.
    let mut bad_raw = bytes.clone();
    bad_raw[15] ^= 0x04;
    let bad_raw_path = tmp.path().join("bad-crc.scert");
    fs::write(&bad_raw_path, &bad_raw).expect("write bad crc");
    let (code, stdout) = run_bin(&["synccert", bad_raw_path.to_str().unwrap()]);
    assert_eq!(code, 8, "corrupted synccert frame must exit 8: {stdout}");
    assert!(stdout.contains("FMINE-REF-505"), "{stdout}");
}

#[test]
fn test_fmine_verify_segment_and_tamper_exits_8() {
    let tmp = TempDir::new("segment");
    let seg_path = tmp.path().join("cell.fmseg");
    let mut writer = SegmentWriter::create(
        &seg_path,
        "CELL-FLOW(site-a)",
        0,
        GENESIS_PREV_HASH,
        Vec::new(),
    )
    .expect("create segment");

    let mut prev = GENESIS_PREV_HASH;
    for seq in 0u64..3 {
        let anchor = LedgerAnchor {
            format_version: FMT_ANCHOR_V1,
            cell_id: "CELL-FLOW(site-a)".to_string(),
            ledger_epoch: 1,
            commit_sequence: seq,
            adapter_registry_epoch: 1,
            schema_epoch: 1,
            policy_epoch: 1,
            privacy_epoch: 1,
            residency_epoch: 1,
            state_root: prev,
        };
        let batch = DeltaBatch {
            format_version: FMT_LEDGER_V1,
            cell_id: "CELL-FLOW(site-a)".to_string(),
            epoch: 1,
            seq,
            prev_hash: prev,
            anchor,
            deltas: vec![DeltaOp::new(
                format!("k/{seq}"),
                DeltaKind::Put,
                format!("payload-{seq}").into_bytes(),
            )],
            witnesses: Vec::new(),
            cross_cell_refs: Vec::new(),
        };
        prev = writer.append(&batch).expect("append batch");
    }
    let _footer = writer.seal().expect("seal segment");

    // 1. Clean sealed segment exits 0.
    let (code, stdout) = run_bin(&["segment", seg_path.to_str().unwrap()]);
    assert_eq!(code, 0, "clean segment must exit 0: {stdout}");
    assert!(stdout.contains("\"record_count\":3"), "{stdout}");

    // 2. Flip 1 bit in interior of segment -> exit 8.
    let orig = fs::read(&seg_path).expect("read seg");
    let mut flipped = orig.clone();
    let mid = flipped.len() / 2;
    flipped[mid] ^= 0x01;
    let bad_seg = tmp.path().join("flipped.fmseg");
    fs::write(&bad_seg, &flipped).expect("write flipped seg");
    let (code, stdout) = run_bin(&["segment", bad_seg.to_str().unwrap()]);
    assert_eq!(code, 8, "flipped segment must exit 8: {stdout}");
    assert!(stdout.contains("FMINE-REF-505"), "{stdout}");

    // 3. Append torn tail bytes -> exit 8.
    let mut torn = orig;
    torn.extend_from_slice(&[0xDE, 0xAD, 0xBE, 0xEF, 0x01]);
    let torn_seg = tmp.path().join("torn.fmseg");
    fs::write(&torn_seg, &torn).expect("write torn seg");
    let (code, stdout) = run_bin(&["segment", torn_seg.to_str().unwrap()]);
    assert_eq!(code, 8, "torn segment must exit 8: {stdout}");
    assert!(stdout.contains("FMINE-REF-505"), "{stdout}");
}
