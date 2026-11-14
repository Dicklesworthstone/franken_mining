// Cargo supplies the executable path for the fmine-cli integration target in every profile.
include!("peer_comparison_fixture.rs");

#[test]
fn copper_peer_binary_matches_library_json_markdown_and_refusals() {
    let binary = env!("CARGO_BIN_EXE_fmine");
    for metric in [
        Metric::CopperProduction,
        Metric::CopperC1,
        Metric::CopperAisc,
    ] {
        let fixture = Fixture::new_metric(metric);
        let result = fixture.ok();
        for (format, expected) in [
            ("json", result.to_json()),
            ("markdown", result.rendered_markdown.unwrap()),
        ] {
            let output = std::process::Command::new(binary)
                .args(fixture.args())
                .args(["--format", format, "--workspace"])
                .arg(&fixture.root)
                .output()
                .unwrap();
            assert_eq!(
                output.status.code(),
                Some(0),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            assert_eq!(
                String::from_utf8(output.stdout).unwrap(),
                format!("{expected}\n")
            );
        }
        fixture.change_rows("peer_d", "metal=copper", "metal=gold");
        let refused = fixture.invoke();
        let output = std::process::Command::new(binary)
            .args(fixture.args())
            .arg("--workspace")
            .arg(&fixture.root)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(i32::from(refused.exit_code)));
        assert_eq!(
            String::from_utf8(output.stdout).unwrap(),
            format!("{}\n", refused.to_json())
        );
    }
}
