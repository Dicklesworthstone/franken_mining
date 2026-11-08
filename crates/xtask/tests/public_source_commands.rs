//! Cross-crate source-registry parity and actual-binary mirror acceptance checks.
//! Separate process tests avoid inheriting journal locks from reliance unit tests.
#![cfg(test)]

use fmine_public::sources::load_issuer_profile;
use std::path::PathBuf;
use std::process::Command;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("workspace root")
        .to_path_buf()
}

fn executable() -> PathBuf {
    std::env::var_os("FMINE_TEST_BINARY")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            let executable = std::env::current_exe().expect("test executable");
            executable
                .ancestors()
                .find(|p| p.file_name().is_some_and(|n| n == "deps" || n == "build"))
                .and_then(std::path::Path::parent)
                .expect("build directory")
                .join(format!("fmine{}", std::env::consts::EXE_SUFFIX))
        })
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

#[test]
fn source_bound_matrix_passes_independent_python_oracle() {
    let output = Command::new("python3")
        .arg("-I")
        .arg(workspace_root().join("crates/xtask/testdata/mirror_oracle.py"))
        .arg("--binary")
        .arg(executable())
        .output()
        .expect("run T-layer mirror source matrix against the real executable");
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn synthetic_binary_emits_readable_markdown() {
    let output = Command::new(executable())
        .args(["mirror", "--synthetic", "--format", "markdown"])
        .output()
        .expect("run mirror executable");
    assert!(output.status.success(), "{:?}", output);
    let markdown = String::from_utf8(output.stdout).expect("mirror emits UTF-8");
    // The source matrix checks synthetic JSON and every authority/evidence flag independently.
    for expected in ["Alpha.pdf", "Beta.pdf", "615", "385", "invented-mine"] {
        assert!(
            markdown.contains(expected),
            "missing {expected}: {markdown}"
        );
    }
}
