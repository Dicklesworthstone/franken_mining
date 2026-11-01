//! Public CLI admission and synthetic assurance contracts exercised through the T layer.

#![cfg(test)]
#![allow(clippy::expect_used, clippy::unwrap_used)]

mod tests {
    use fmine_cli::{CliCommand, OutputFormat, execute_command, invoke, parse_args, run};
    use fmine_public::sources::registry_path;
    use std::path::{Path, PathBuf};

    fn workspace_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|p| p.parent())
            .expect("workspace root")
            .to_path_buf()
    }

    fn args(a: &[&str]) -> Vec<String> {
        a.iter().map(|s| (*s).to_string()).collect()
    }

    #[test]
    fn reliance_router_preserves_required_values_before_global_options() {
        let mut issue = args(&[
            "assure",
            "issue",
            "--synthetic",
            "--output",
            "report.json",
            "--journal",
            "journal",
            "--expected-head",
            &"a".repeat(64),
            "--key",
            "owner.key",
            "--role",
            "sponsor",
            "--purpose",
            "review",
            "--now",
            "2026-11-01T00:00:00Z",
            "--uncertainty-ns",
            "0",
        ]);
        for (flag, inserted) in [
            ("--purpose", vec!["--json"]),
            ("--purpose", vec!["--format", "json"]),
            ("--key", vec!["--workspace", "/other"]),
        ] {
            let mut malformed = issue.clone();
            let value = malformed.iter().position(|arg| arg == flag).unwrap() + 1;
            malformed.splice(value..value, inserted.into_iter().map(str::to_string));
            let error = parse_args(&malformed, Path::new(".")).unwrap_err();
            assert!(
                error.contains(&format!("missing value for {flag}")),
                "{error}"
            );
        }
        issue.splice(0..0, args(&["--workspace", "/selected"]));
        issue.extend(args(&["--format", "markdown"]));
        let (command, format, workspace) = parse_args(&issue, Path::new(".")).unwrap();
        assert!(matches!(command, CliCommand::Issued(_)));
        assert_eq!(format, OutputFormat::Markdown);
        assert_eq!(workspace, Path::new("/selected"));
        for value in ["", "--json"] {
            assert!(
                parse_args(
                    &args(&["--workspace", value, "reliance", "check"]),
                    Path::new(".")
                )
                .is_err()
            );
        }
    }

    #[test]
    fn reliance_router_preserves_published_grammar_without_parser_fallback() {
        let digest = "a".repeat(64);
        let explicit = args(&[
            "--json",
            "reliance",
            "check",
            &digest,
            "--journal",
            "journal",
        ]);
        let host = args(&[
            "--json",
            "reliance",
            "check",
            "--digest",
            &digest,
            "--journal",
            "journal",
        ]);
        assert!(matches!(
            parse_args(&explicit, Path::new(".")).unwrap().0,
            CliCommand::RelianceExplicit(_)
        ));
        assert!(matches!(
            parse_args(&host, Path::new(".")).unwrap().0,
            CliCommand::Reliance(_)
        ));
        let mut required = host.clone();
        required.insert(3, "--require-current".into());
        assert!(matches!(
            parse_args(&required, Path::new(".")).unwrap().0,
            CliCommand::Reliance(_)
        ));
        let mut mixed = host;
        mixed.extend(args(&[
            "--now",
            "2026-11-01T00:00:00Z",
            "--uncertainty-ns",
            "0",
        ]));
        assert!(parse_args(&mixed, Path::new(".")).is_err());
    }

    fn scratch(tag: &str) -> PathBuf {
        std::env::temp_dir().join(format!("fmine-cli-{tag}-{}", std::process::id()))
    }

    /// A workspace holding a copy of the barrick registry and no corpus.
    fn no_corpus_workspace(tag: &str) -> PathBuf {
        let ws = scratch(tag);
        let reg = registry_path(&ws, "barrick");
        std::fs::create_dir_all(reg.parent().expect("dir")).expect("registry dir");
        std::fs::copy(registry_path(&workspace_root(), "barrick"), &reg).expect("copy registry");
        ws
    }

    #[test]
    fn test_guidance_command_is_exact_cited_and_explicitly_conditional() {
        let query = args(&["guidance", "completion", "--synthetic"]);
        let (env, _) = invoke(&query, &workspace_root());
        assert_eq!((env.status, env.exit_code), ("ok", 0), "{}", env.to_json());
        assert_eq!(env.verb, "guidance completion");
        assert!(env.payload_json.contains("\"lane\":\"synthetic\""));
        assert!(env.payload_json.contains("\"envelope_tier\":\"T1\""));
        assert!(
            env.payload_json
                .contains("within_supplied_history_under_sales_scenario")
        );
        assert!(env.payload_json.contains("\"independently_checked\":true"));
        // Invented target 200..230 minus actual 68 rounded to the nearest koz gives [131.5,162.5].
        assert!(
            env.payload_json
                .contains("\"lo\":\"263/2\",\"hi\":\"325/2\"")
        );
        assert!(
            env.payload_json
                .contains("ASSUMPTION: repeat preceding-year")
        );
        assert!(env.payload_json.contains("\"source_claims\":[{"));
        assert!(
            env.anchor_id.is_none(),
            "a computation is not a registered forecast"
        );
        assert!(
            env.certificate.is_none(),
            "a content digest is not a ledger root"
        );
        let digest = fmine_public::guidance::run_synthetic()
            .expect("synthetic guidance")
            .digest
            .to_hex();
        assert!(
            env.payload_json
                .contains(&format!("\"digest\":\"{digest}\""))
        );
        assert_eq!(env.no_claim, Some(fmine_public::guidance::NO_CLAIM));
        assert_eq!(invoke(&query, &workspace_root()).0, env);
        let (markdown, exit) = run(
            &args(&[
                "guidance",
                "completion",
                "--synthetic",
                "--format",
                "markdown",
            ]),
            &workspace_root(),
        );
        assert_eq!(exit, 0);
        assert!(markdown.starts_with("# Guidance completion:"));
        assert!(markdown.contains("Invented synthetic evidence"));
        assert!(markdown.contains("131.5 to 162.5 koz"));
        assert!(markdown.contains("Checked witness"));
        assert!(markdown.contains("## Source citations"));
        assert!(markdown.contains("conditional on that sales interval"));
        assert!(markdown.contains("not a mine-capacity conclusion or a forecast"));
        assert!(!markdown.starts_with("```json"));
    }

    #[test]
    fn test_track_record_requires_an_explicit_evidence_cut_and_registered_inputs() {
        let (command, _, _) = parse_args(
            &args(&[
                "track-record",
                "--issuer",
                "barrick",
                "--site",
                "north-mara",
                "--as-of",
                "2026-11-01",
            ]),
            &workspace_root(),
        )
        .expect("explicit evidence cut");
        assert_eq!(
            command,
            CliCommand::TrackRecordAsOf {
                issuer: "barrick".into(),
                site: Some("north-mara".into()),
                as_of: "2026-11-01".into(),
                price_subject: None,
            }
        );
        let (priced, _, _) = parse_args(
            &args(&[
                "--price-subject",
                "Barrick",
                "track-record",
                "--issuer",
                "barrick",
                "--as-of",
                "2026-11-01",
            ]),
            &workspace_root(),
        )
        .expect("explicit price proxy at an evidence cut");
        assert_eq!(
            priced,
            CliCommand::TrackRecordAsOf {
                issuer: "barrick".into(),
                site: None,
                as_of: "2026-11-01".into(),
                price_subject: Some("Barrick".into()),
            }
        );
        for bad in [
            vec!["track-record"],
            vec!["track-record", "--as-of", "2026-11-01"],
            vec![
                "track-record",
                "--issuer",
                "unknown",
                "--as-of",
                "2026-11-01",
            ],
            vec![
                "track-record",
                "--issuer",
                "barrick",
                "--as-of",
                "2026-02-30",
            ],
            vec![
                "track-record",
                "--issuer",
                "barrick",
                "--as-of",
                "2026-11-01T00:00:00Z",
            ],
            vec![
                "track-record",
                "--issuer",
                "barrick",
                "--as-of",
                "2026-11-01",
                "--site",
                "",
            ],
            vec![
                "track-record",
                "--issuer",
                "barrick",
                "--through",
                "2025Q4",
                "--as-of",
                "2026-11-01",
            ],
            vec!["track-record", "--synthetic", "--issuer", "barrick"],
            vec!["track-record", "--synthetic", "--as-of", "2026-11-01"],
            vec!["track-record", "--synthetic", "--price-subject", "Barrick"],
            vec!["track-record", "--synthetic", "--site", "north-mara"],
            vec!["track-record", "--synthetic", "extra"],
        ] {
            let (env, _) = invoke(&args(&bad), &workspace_root());
            assert_eq!(env.exit_code, 2, "{bad:?}: {}", env.to_json());
            assert!(env.anchor_id.is_none());
            assert!(env.certificate.is_none());
        }
        let ws = no_corpus_workspace("track-record-no-inputs");
        let (env, _) = invoke(
            &args(&[
                "track-record",
                "--issuer",
                "barrick",
                "--as-of",
                "2026-11-01",
            ]),
            &ws,
        );
        assert_eq!(env.status, "refused");
        let refusal = env.refusal.expect("missing registered workbooks");
        assert_eq!(refusal.code, "FMINE-REF-506");
        assert!(refusal.what.contains("Barrick_Q1_2024_Mine_Stats.xlsx"));
        assert!(refusal.what.contains("Barrick_Q2_2026_Mine_Stats.xlsx"));
        assert_eq!(env.payload_json, "null");
        assert!(env.anchor_id.is_none());
        assert!(env.certificate.is_none());
        let (docs, _) = invoke(&args(&["robot-docs"]), &workspace_root());
        assert!(docs.payload_json.contains("track-record --issuer"));
        assert!(docs.payload_json.contains("track-record --synthetic"));
        assert!(docs.payload_json.contains("[--price-subject <subject>]"));
        assert!(
            docs.payload_json
                .contains("Separate paired price sensitivities for first-published and latest registered outcomes; prices and completed quarterly history are bounded by the evidence cut")
        );
    }

    #[test]
    fn test_cli_assure_synthetic_end_to_end() {
        let root = workspace_root();
        let (cmd, fmt, ws) =
            parse_args(&args(&["assure", "public", "--synthetic"]), &root).expect("parse");
        assert_eq!(
            (cmd.clone(), fmt),
            (CliCommand::AssureSynthetic, OutputFormat::Json)
        );
        let env = execute_command(&cmd, &ws);
        assert_eq!((env.exit_code, env.status), (0, "ok"));
        assert!(env.anchor_id.as_deref().unwrap_or("").starts_with("anc_"));
        let json = env.to_json();
        assert!(json.contains(
            "\"payload\":{\"issuer\":\"synthetic\",\"lane\":\"synthetic\",\"gate_eligible\":false"
        ));
        // bd-jg43.3: column G scored on displayed values, column N "Site G" refused by format.
        assert!(
            json.contains("\"totals\":{\"i\":40,\"ii\":2,\"iii\":1,\"iv\":2,\"v\":5,\"np\":5}")
        );
        // The next actions sit in the envelope, not the payload.
        assert_eq!(
            env.next_actions.first().map(|a| a.command.as_str()),
            Some("fmine assure public --issuer barrick")
        );
        assert!(
            json.contains(
                "\"next_actions\":[{\"command\":\"fmine assure public --issuer barrick\""
            )
        );
        assert!(!env.payload_json.contains("next_actions"));
    }
}
