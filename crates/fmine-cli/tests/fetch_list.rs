//! BD-339.S2: `repro/barrick/fetch.sh` reads the registry with its own awk parser, so that a reader can fetch
//! without building. Its listing must equal the Rust loader's rows (file, url, bytes, sha256) in order: the
//! registry is the one copy, and the two readers of it cannot drift apart unnoticed.
//!
//! This test starts a process, so it has its own test binary. In the lib test binary, its spawn ran beside the
//! reliance tests, which take the journal's file lock: 9 of 60 runs failed one of them with "reliance journal
//! is locked", and none of 40 runs without this test did (2026-11-01).
#![cfg(test)]

use std::path::PathBuf;
use std::process::Command;

use fmine_public::sources::load_issuer_profile;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("workspace root")
        .to_path_buf()
}

#[test]
fn test_fetch_list_equals_registry() {
    let root = workspace_root();
    let out = Command::new("bash")
        .arg(root.join("repro").join("barrick").join("fetch.sh"))
        .arg("--list")
        .env("FMINE_WORKSPACE", &root)
        .output()
        .expect("run fetch.sh --list");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let want: String = load_issuer_profile(&root, "barrick")
        .expect("registry")
        .sources
        .iter()
        .map(|s| {
            format!(
                "{}\t{}\t{}\t{}\n",
                s.file_name, s.url, s.bytes, s.sha256_hex
            )
        })
        .collect();
    assert_eq!(String::from_utf8_lossy(&out.stdout), want);
}
