use std::path::PathBuf;
use std::process::Command;

/// BD-142 [WP-07]: executes `tests/e2e/self_test.sh` against the compiled real `fmine` binary,
/// verifying NDJSON step logging, anchor chaining, deterministic rerun digests, Markdown summary
/// rendering from the log, self-contained CrashPack capture + replay after source workspace removal,
/// and CrashPack SHA-256 tamper rejection.
#[test]
fn test_bd142_e2e_ndjson_harness_and_crashpack_self_test() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("workspace root")
        .to_path_buf();
    let fmine_bin = PathBuf::from(env!("CARGO_BIN_EXE_fmine"));
    assert!(
        fmine_bin.is_file(),
        "compiled fmine binary missing at {}",
        fmine_bin.display()
    );

    let output = Command::new("bash")
        .arg(root.join("tests/e2e/self_test.sh"))
        .current_dir(&root)
        .env("FMINE_BIN", &fmine_bin)
        .output()
        .expect("execute tests/e2e/self_test.sh");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "BD-142 self_test.sh failed (exit {:?}):\n--- stdout ---\n{stdout}\n--- stderr ---\n{stderr}",
        output.status.code()
    );
    assert!(
        stdout.contains(
            "\"event\":\"bd142_self_test\",\"ok\":true,\"pass_steps\":7,\"crashpack_reproduced\":true,\"tamper_checks\":3"
        ),
        "unexpected BD-142 self_test.sh output:\n{stdout}"
    );
}
