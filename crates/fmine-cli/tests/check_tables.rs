#![cfg(test)]

//! BD-340: real-binary source scenarios and typed library/binary rendering parity.
//! The standard-library Python oracle retains the invented PDF fixtures and every negative case.

#![allow(clippy::expect_used)]

#[cfg(test)]
mod tests {
    use fmine_cli::OutputFormat;
    use std::path::PathBuf;
    use std::process::Command;

    fn oracle() -> Command {
        let mut command = Command::new("python3");
        command
            .arg("-I")
            .arg(
                PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../xtask/testdata/table_oracle.py"),
            )
            .arg(env!("CARGO_BIN_EXE_fmine"));
        command
    }

    #[test]
    fn source_scenarios_pass_the_standard_library_python_oracle() {
        let output = oracle().output().expect("execute table source scenarios");
        assert!(
            output.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[test]
    fn exact_library_and_binary_json_markdown_remain_identical() {
        let fixture = oracle()
            .arg("--fixture-only")
            .output()
            .expect("create invented fixture");
        assert!(
            fixture.status.success(),
            "{}",
            String::from_utf8_lossy(&fixture.stderr)
        );
        let root = PathBuf::from(
            String::from_utf8(fixture.stdout)
                .expect("fixture path is UTF-8")
                .trim(),
        );
        let args = [
            "check",
            "tables",
            "--issuer",
            "table_test",
            "--definitions",
            "table-identities.toml",
        ]
        .map(str::to_owned);
        let (expected, format) = fmine_cli::invoke(&args, &root);
        assert_eq!(format, OutputFormat::Json);
        assert_eq!(expected.exit_code, 0, "{}", expected.to_json());
        assert!(expected.refusal.is_none());
        assert_eq!(
            expected.to_json(),
            fmine_cli::invoke(&args, &root).0.to_json()
        );
        for (format, expected_text) in [
            ("json", expected.to_json()),
            (
                "markdown",
                expected.rendered_markdown.clone().expect("table Markdown"),
            ),
        ] {
            let output = Command::new(env!("CARGO_BIN_EXE_fmine"))
                .args(&args)
                .args(["--format", format, "--workspace"])
                .arg(&root)
                .output()
                .expect("execute exact real-binary rendering");
            assert_eq!(output.status.code(), Some(0));
            assert_eq!(
                String::from_utf8(output.stdout).expect("rendering is UTF-8"),
                format!("{expected_text}\n")
            );
        }
    }
}
