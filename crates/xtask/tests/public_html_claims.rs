//! Registered HTML source-to-Claim scenarios, using invented filings and independent exact expectations.

#![cfg(test)]
#![allow(clippy::expect_used)]

use std::path::{Path, PathBuf};
use std::process::Command;

fn binary() -> PathBuf {
    if let Some(path) = std::env::var_os("FMINE_TEST_BINARY") {
        return PathBuf::from(path);
    }
    let executable = std::env::current_exe().expect("current T executable");
    let profile = executable
        .ancestors()
        .find(|path| {
            path.file_name()
                .is_some_and(|name| name == "deps" || name == "build")
        })
        .and_then(Path::parent)
        .expect("Cargo profile directory; set FMINE_TEST_BINARY for another layout");
    profile.join(format!("fmine{}", std::env::consts::EXE_SUFFIX))
}

fn oracle() -> Command {
    let mut command = Command::new("python3");
    command
        .arg("-I")
        .arg(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("testdata/html_claims_oracle.py"))
        .arg(binary());
    command
}

#[test]
fn registered_html_claims_pass_independent_exact_source_scenarios() {
    let output = oracle().output().expect("execute HTML source scenarios");
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn html_library_and_binary_keep_identical_canonical_claims_and_markdown() {
    let fixture = oracle()
        .arg("--fixture-only")
        .output()
        .expect("create invented HTML fixture");
    assert!(fixture.status.success());
    let root = PathBuf::from(
        String::from_utf8(fixture.stdout)
            .expect("UTF-8 fixture path")
            .trim(),
    );
    let args = [
        "html",
        "claims",
        "--issuer",
        "html_test",
        "--definitions",
        "html-claims.toml",
    ]
    .map(str::to_owned);
    let (envelope, format) = fmine_cli::invoke(&args, &root);
    assert_eq!(format, fmine_cli::OutputFormat::Json);
    assert_eq!(envelope.exit_code, 0, "{}", envelope.to_json());
    assert_eq!(
        envelope.to_json(),
        fmine_cli::invoke(&args, &root).0.to_json()
    );
    for (format, expected) in [
        ("json", envelope.to_json()),
        (
            "markdown",
            envelope.rendered_markdown.expect("HTML Markdown"),
        ),
    ] {
        let output = Command::new(binary())
            .args(&args)
            .args(["--format", format, "--workspace"])
            .arg(&root)
            .output()
            .expect("execute real-binary HTML rendering");
        assert_eq!(output.status.code(), Some(0));
        assert_eq!(
            String::from_utf8(output.stdout).expect("UTF-8 output"),
            format!("{expected}\n")
        );
    }
}
