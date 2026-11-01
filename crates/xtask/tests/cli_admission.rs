//! Public CLI admission and synthetic assurance contracts exercised through the T layer.

#![cfg(test)]
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

mod tests {
    use fmine_cli::{CliCommand, OutputFormat, execute_command, invoke, parse_args, run};
    use fmine_evidence::CATALOG;
    use fmine_public::sources::{load_issuer_profile, registry_path};
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

    /// The four INV-002 flags, all false, written by hand from BD-329.S3 (not taken from `FLAGS_JSON`).
    const FLAGS: &str = "{\"reserve_disclosure_claim\":false,\"execution_authorized\":false,\
                         \"geotechnical_signoff\":false,\"regulatory_compliance_claim\":false}";

    #[test]
    fn test_challenge_command_renders_cited_json_and_readable_markdown() {
        let query = args(&["assure", "challenges", "--synthetic"]);
        let (env, _) = invoke(&query, &workspace_root());
        assert_eq!((env.status, env.exit_code), ("ok", 0), "{}", env.to_json());
        assert_eq!(env.verb, "assure challenges");
        assert!(
            env.payload_json
                .contains("\"schema\":\"fmine.challenge_packets.v1\"")
        );
        assert!(env.payload_json.contains("\"subject\":\"Site B\""));
        assert!(env.payload_json.contains("\"component_sum\":{\"v\":250,"));
        assert!(env.payload_json.contains("\"sign_flip_witness\":\"C6\""));
        assert!(env.payload_json.contains("\"external_use_allowed\":false"));
        assert!(env.to_json().contains(FLAGS));
        assert_eq!(invoke(&query, &workspace_root()).0.to_json(), env.to_json());
        let (markdown, exit) = run(
            &args(&[
                "assure",
                "challenges",
                "--synthetic",
                "--format",
                "markdown",
            ]),
            &workspace_root(),
        );
        assert_eq!(exit, 0);
        assert!(markdown.starts_with("# Evidence challenge packets\n"));
        assert!(markdown.contains("## Closing evidence requested"));
        assert!(markdown.contains("## Source operands"));
        assert!(markdown.contains("invented synthetic workbooks"));
        assert!(!markdown.contains("external_use_allowed\":true"));
    }

    #[test]
    fn test_new_commands_refuse_missing_evidence_and_reject_mixed_selectors() {
        let ws = no_corpus_workspace("new-commands-no-corpus");
        let (env, _) = invoke(&args(&["assure", "challenges", "--issuer", "barrick"]), &ws);
        assert_eq!(env.status, "refused");
        assert_eq!(env.refusal.expect("missing source").code, "FMINE-REF-506");
        assert_eq!(env.payload_json, "null");
        let (env, _) = invoke(
            &args(&[
                "guidance",
                "completion",
                "--issuer",
                "barrick",
                "--site",
                "north-mara",
                "--through",
                "2026Q2",
                "--as-of",
                "2026-10-31",
            ]),
            &ws,
        );
        assert_eq!(env.status, "refused");
        assert_eq!(env.refusal.expect("missing guidance").code, "FMINE-REF-201");
        assert_eq!(env.payload_json, "null");
        for bad in [
            vec!["assure", "challenges"],
            vec!["assure", "challenges", "--synthetic", "--issuer", "barrick"],
            vec![
                "guidance",
                "completion",
                "--synthetic",
                "--through",
                "2026Q2",
            ],
            vec![
                "guidance",
                "completion",
                "--issuer",
                "barrick",
                "--site",
                "north-mara",
            ],
            vec![
                "guidance",
                "completion",
                "--synthetic",
                "--as-of",
                "2026-02-30",
            ],
            vec!["doctor", "--site", "north-mara"],
        ] {
            assert!(
                parse_args(&args(&bad), &workspace_root()).is_err(),
                "{bad:?}"
            );
        }
    }

    /// Without the registered workbooks the gate-eligible run refuses with 506 (foundation I/O, exit 5) and
    /// names the files; it never substitutes stand-in content (BD-321).
    ///
    /// The scratch workspace holds a copy of the registry and no corpus.
    #[test]
    fn test_cli_assure_public_refuses_without_registered_files() {
        let ws = std::env::temp_dir().join(format!("fmine-cli-no-corpus-{}", std::process::id()));
        let reg = registry_path(&ws, "barrick");
        std::fs::create_dir_all(reg.parent().expect("dir")).expect("registry dir");
        std::fs::copy(registry_path(&workspace_root(), "barrick"), &reg).expect("copy registry");
        let ws_arg = ws.display().to_string();
        let (cmd, _, ws_parsed) = parse_args(
            &args(&[
                "assure",
                "public",
                "--issuer",
                "barrick",
                "--workspace",
                &ws_arg,
            ]),
            &workspace_root(),
        )
        .expect("parse");
        let env = execute_command(&cmd, &ws_parsed);
        assert_eq!((env.exit_code, env.status), (5, "refused"));
        let json = env.to_json();
        assert!(json.contains("\"code\":\"FMINE-REF-506\""));
        assert!(json.contains("Barrick_Q2_2026_AISC.xlsx"));
        assert!(json.contains("\"payload\":null"));

        // --through narrows the refusal to the rows at or before the bound.
        let (cmd, _, _) = parse_args(
            &args(&[
                "assure",
                "public",
                "--issuer",
                "barrick",
                "--through",
                "2024Q4",
                "--workspace",
                &ws_arg,
            ]),
            &workspace_root(),
        )
        .expect("parse");
        assert_eq!(
            cmd,
            CliCommand::AssurePublic {
                issuer: "barrick".to_string(),
                through: Some("2024Q4".to_string())
            }
        );
        let json = execute_command(&cmd, &ws_parsed).to_json();
        assert!(json.contains("Barrick_Q4_2024_AISC.xlsx"), "{json}");
        assert!(!json.contains("Barrick_Q1_2025"), "{json}");
        let _ = std::fs::remove_dir_all(&ws);
    }

    /// A workspace without the issuer's registry file makes the issuer unknown: a usage error, exit 2.
    #[test]
    fn test_cli_unknown_issuer_without_registry_is_usage_error() {
        let empty = workspace_root()
            .join("crates")
            .join("fmine-cli")
            .join("no-such-workspace");
        let err = parse_args(
            &args(&[
                "assure",
                "public",
                "--issuer",
                "barrick",
                "--workspace",
                &empty.display().to_string(),
            ]),
            &workspace_root(),
        )
        .expect_err("no registry there");
        assert!(err.contains("no registries/sources/barrick.toml"), "{err}");
    }

    #[test]
    fn test_cli_usage_errors() {
        let root = workspace_root();
        for bad in [
            &["assure", "public", "--issuer", "newmont"][..],
            &["assure", "public"][..],
            &["assure", "public", "--issuer", "barrick", "--synthetic"][..],
            &["assure", "private"][..],
            &[
                "assure",
                "public",
                "--issuer",
                "barrick",
                "--through",
                "2026Q5",
            ][..],
            &[
                "assure",
                "public",
                "--issuer",
                "barrick",
                "--through",
                "2026-Q2",
            ][..],
            &["assure", "public", "--issuer", "barrick", "--through"][..],
            &["assure", "public", "--synthetic", "--through", "2026Q2"][..],
            &[
                "assure",
                "public",
                "--issuer",
                "../registries/sources/barrick",
            ][..],
            &[][..],
            // Unknown flags, surplus words and flags a verb does not take (none is ignored).
            &["doctor", "--deep"][..],
            &["robot-docs", "extra"][..],
            &["assure", "public", "issuer=barrick"][..],
            &["assure", "public", "--synthetic", "now"][..],
            &["claim", "show", "clm_0001", "clm_0002"][..],
            &["claim", "why", "clm_0001", "extra"][..],
            &["claim", "trace"][..],
            &["claim", "invent", "clm_0001"][..],
            &[
                "claim",
                "why",
                "clm_0001",
                "--synthetic",
                "--issuer",
                "barrick",
            ][..],
            &["doctor", "--issuer", "barrick"][..],
            &["robot-docs", "--synthetic"][..],
        ] {
            assert!(
                parse_args(&args(bad), &root).is_err(),
                "{bad:?} must be a usage error"
            );
            // What `fmine` prints: one envelope line on stdout, exit 2.
            let (text, exit) = run(&args(bad), &root);
            assert_eq!(exit, 2, "{bad:?}");
            assert!(!text.contains('\n'), "{bad:?}: {text}");
            assert!(
                text.starts_with("{\"schema_version\":\"fmine.env.v1\",\"verb\":\""),
                "{bad:?}: {text}"
            );
            assert!(
                text.contains("\"status\":\"usage_error\",\"exit_code\":2,"),
                "{bad:?}: {text}"
            );
        }
        let env = execute_command(
            &CliCommand::AssurePublic {
                issuer: "newmont".to_string(),
                through: None,
            },
            &root,
        );
        assert_eq!(
            (env.exit_code, env.status, env.refusal.is_none()),
            (2, "usage_error", true)
        );
    }

    /// BD-339.S2: `repro/barrick/fetch.sh` reads the registry with its own awk parser, so that a reader can
    /// fetch without building. Its listing must equal the Rust loader's rows (file, url, bytes, sha256) in
    /// order: the registry is the one copy, and the two readers of it cannot drift apart unnoticed.
    #[test]
    fn test_fetch_list_equals_registry() {
        let root = workspace_root();
        let out = std::process::Command::new("bash")
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
    fn test_cli_doctor_and_robot_docs() {
        let root = workspace_root();
        let env_doc = execute_command(&CliCommand::Doctor, &root);
        assert_eq!(env_doc.exit_code, 0);
        assert!(env_doc.to_json().contains("\"registries_ok\":true"));
        let n = load_issuer_profile(&root, "barrick")
            .expect("registry")
            .sources
            .len();
        assert!(
            env_doc.to_json().contains(&format!(
                "{{\"issuer\":\"barrick\",\"registry_ok\":true,\"sources_registered\":{n},"
            )),
            "{}",
            env_doc.to_json()
        );

        let env_rob = execute_command(&CliCommand::RobotDocs, &root);
        assert_eq!(env_rob.exit_code, 0);
        let rob = env_rob.to_json();
        assert!(rob.contains("\"FMT-ENV-1\""));
        assert!(rob.contains("\"issuers\":[\"barrick\"]"), "{rob}");
        assert!(rob.contains("{\"code\":\"FMINE-REF-502\",\"exit_code\":6,\"status\":\"active\","));
        assert!(
            rob.contains("{\"code\":\"FMINE-REF-506\",\"exit_code\":5,\"status\":\"proposed\",")
        );
        assert_eq!(
            rob.matches("{\"code\":\"FMINE-REF-").count(),
            CATALOG.len(),
            "{rob}"
        );
        // robot-docs documents every envelope field, in print order.
        let doc = &env_rob.payload_json[env_rob
            .payload_json
            .find("\"envelope_fields\":")
            .expect("envelope fields documented")..];
        let mut at = 0;
        for field in [
            "schema_version",
            "verb",
            "status",
            "exit_code",
            "anchor_id",
            "flags",
            "certificate",
            "warnings",
            "next_actions",
            "no_claim",
            "payload",
            "refusal",
        ] {
            at += doc[at..]
                .find(&format!("\"{field}\":"))
                .unwrap_or_else(|| panic!("{field} documented after the previous field: {doc}"));
        }
    }

    #[test]
    fn test_usage_error_from_parse_args_is_envelope() {
        let root = workspace_root();
        for (bad, verb) in [
            (&["frobnicate"][..], "unknown"),
            (&[][..], "unknown"),
            (
                &["--format", "md", "assure", "private"][..],
                "assure public",
            ),
            (&["--format", "xml", "doctor"][..], "doctor"),
            (&["claim", "show"][..], "claim show"),
            (&["claim", "why"][..], "claim why"),
            (&["claim", "trace"][..], "claim trace"),
            (&["claim", "--format", "json", "trace"][..], "claim trace"),
        ] {
            let (text, exit) = run(&args(bad), &root);
            assert_eq!(exit, 2, "{bad:?}");
            assert_eq!(text.lines().count(), 1, "{bad:?}: {text}");
            assert!(
                text.starts_with(&format!(
                    "{{\"schema_version\":\"fmine.env.v1\",\"verb\":\"{verb}\",\"status\":\"usage_error\",\
                     \"exit_code\":2,\"anchor_id\":null,\"flags\":{FLAGS},"
                )),
                "{bad:?}: {text}"
            );
            assert!(text.ends_with(",\"refusal\":null}"), "{bad:?}: {text}");
        }
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
