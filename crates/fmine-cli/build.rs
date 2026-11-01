//! DESIGN (BD-347): capture source revision when the executable is built, never from its runtime cwd.

use std::path::Path;
use std::process::Command;

fn git(root: &Path, args: &[&str]) -> Option<String> {
    let result = Command::new("git")
        .current_dir(root)
        .args(args)
        .output()
        .ok()?;
    if !result.status.success() {
        return None;
    }
    String::from_utf8(result.stdout)
        .ok()
        .map(|s| s.trim().to_string())
}

fn versioned_workspace(root: &Path) -> bool {
    let Ok(workspace) = root.canonicalize() else {
        return false;
    };
    let Some(top_level) = git(root, &["rev-parse", "--show-toplevel"])
        .and_then(|path| Path::new(&path).canonicalize().ok())
    else {
        return false;
    };
    // DESIGN: an archive nested inside another checkout must not inherit that checkout's revision.
    workspace == top_level
        && git(
            root,
            &[
                "ls-files",
                "--error-unmatch",
                "--",
                "Cargo.toml",
                "crates/fmine-cli/src/lib.rs",
            ],
        )
        .is_some()
}

fn main() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    // DESIGN: the executable covers the workspace closure, including changes in sibling crates.
    println!("cargo:rerun-if-changed=../../crates");
    println!("cargo:rerun-if-changed=../../Cargo.toml");
    println!("cargo:rerun-if-changed=../../Cargo.lock");
    for name in ["HEAD", "index", "refs/heads/main", "packed-refs"] {
        if let Some(path) = git(&root, &["rev-parse", "--git-path", name]) {
            println!("cargo:rerun-if-changed={}", root.join(path).display());
        }
    }
    let revision = if versioned_workspace(&root) {
        match (
            git(&root, &["rev-parse", "--verify", "HEAD"]),
            git(
                &root,
                &["status", "--porcelain", "--untracked-files=normal"],
            ),
        ) {
            (Some(sha), Some(status))
                if sha.len() == 40 && sha.bytes().all(|b| b.is_ascii_hexdigit()) =>
            {
                if status.is_empty() {
                    sha
                } else {
                    format!("{sha}+dirty")
                }
            }
            _ => "unknown".to_string(),
        }
    } else {
        "unknown".to_string()
    };
    println!("cargo:rustc-env=FMINE_BUILD_REVISION={revision}");
}
