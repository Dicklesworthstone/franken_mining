//! Registered HTML disclosure checks and canonical-identity regression cases.
#![cfg(test)]
#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::path::{Path, PathBuf};
use std::process::Command;

fn binary() -> PathBuf {
    if let Some(path) = std::env::var_os("FMINE_TEST_BINARY") {
        return path.into();
    }
    let executable = std::env::current_exe().expect("test executable");
    executable
        .ancestors()
        .find(|path| {
            path.file_name()
                .is_some_and(|name| name == "deps" || name == "build")
        })
        .and_then(Path::parent)
        .expect("Cargo profile; set FMINE_TEST_BINARY")
        .join(format!("fmine{}", std::env::consts::EXE_SUFFIX))
}

fn oracle() -> Command {
    let mut command = Command::new("python3");
    command
        .arg("-I")
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("testdata/html_lint_oracle.py"))
        .arg(binary());
    command
}

#[test]
fn html_disclosure_lint_passes_independent_source_and_draft_scenarios() {
    let output = oracle().output().expect("HTML lint oracle");
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn html_metric_recognition_never_bypasses_canonical_claim_integrity() {
    let fixture = oracle()
        .arg("--fixture-only")
        .output()
        .expect("source fixture");
    assert!(
        fixture.status.success(),
        "{}",
        String::from_utf8_lossy(&fixture.stderr)
    );
    let root = PathBuf::from(String::from_utf8(fixture.stdout).unwrap().trim());
    let snapshot = fmine_public::html::run(&root, "html_test", Path::new("html-claims.toml"), None)
        .expect("HTML projection");
    let original = &snapshot
        .claims
        .iter()
        .find(|claim| claim.definition_id == "grade")
        .unwrap()
        .cell;
    let draft = format!(
        "Site A Q2 2025 100% basis processed grade is [2.50 g/t](fmine:{}).",
        original.claim.claim_id
    );
    let positive = fmine_public::lint::lint_cited(&draft, std::slice::from_ref(original));
    assert_eq!(
        positive.findings[0].verdict,
        fmine_public::lint::Verdict::Verified
    );
    let mut changed = original.clone();
    changed.key.unit = "percent".into();
    let refused = fmine_public::lint::lint_cited(&draft, &[changed]);
    assert_eq!(refused.findings[0].reason, "source_claim_identity_mismatch");
    let mut changed = original.clone();
    changed.claim.anchors[0].locator = "Table1!B7".into();
    let refused = fmine_public::lint::lint_cited(&draft, &[changed]);
    assert_eq!(refused.findings[0].reason, "source_claim_identity_mismatch");
}
