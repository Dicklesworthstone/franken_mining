//! Cross-crate command regressions for source-backed disclosure lint and local draft privacy.

#[cfg(test)]
mod tests {
    use fmine_public::sources::load_issuer_profile;
    use std::sync::atomic::{AtomicU64, Ordering};

    static WORKSPACE: AtomicU64 = AtomicU64::new(0);
    const FLAGS_JSON: &str = "{\"reserve_disclosure_claim\":false,\"execution_authorized\":false,\
                              \"geotechnical_signoff\":false,\"regulatory_compliance_claim\":false}";

    fn args(words: &[&str]) -> Vec<String> {
        words.iter().map(|s| (*s).to_string()).collect()
    }

    fn temporary_workspace(label: &str) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!(
            "fmine-lint-{label}-{}-{}",
            std::process::id(),
            WORKSPACE.fetch_add(1, Ordering::Relaxed),
        ));
        std::fs::create_dir_all(&path).expect("temporary workspace");
        path
    }

    #[test]
    fn discovery_is_explicit_and_keeps_source_verification_and_local_draft_privacy() {
        let workspace = temporary_workspace("discovery");
        fmine_public::synthetic::write_issuer_workspace(&workspace).expect("synthetic registry");
        let draft = "Site A Q1 2030 ownership basis unprinted AISC per ounce sold is $1,250/oz.\n\
                     PRIVATE_DISCOVERY_TEXT_MUST_NOT_BE_ECHOED.\n";
        let path = workspace.join("draft.md");
        std::fs::write(&path, draft).expect("local draft");
        let mut arguments = args(&["lint", "draft.md", "--issuer", "synthetic"]);
        let cited = fmine_cli::invoke(&arguments, &workspace).0;
        assert_eq!(cited.exit_code, 0);
        assert!(cited.payload_json.contains("\"verdict\":\"uncited\""));

        arguments.push("--discover".to_string());
        let envelope = fmine_cli::invoke(&arguments, &workspace).0;
        let json = envelope.to_json();
        assert_eq!(envelope.exit_code, 0, "{json}");
        assert!(json.contains("\"verdict\":\"verified\""), "{json}");
        assert!(json.contains("Synthetic_AISC.xlsx:Q1 2030!B16"), "{json}");
        assert!(json.contains(FLAGS_JSON));
        assert!(!json.contains("PRIVATE_DISCOVERY_TEXT"));
        assert_eq!(fmine_cli::invoke(&arguments, &workspace).0.to_json(), json);
        assert_eq!(
            std::fs::read_to_string(&path).expect("draft retained"),
            draft
        );

        let mut markdown = arguments.clone();
        markdown.extend(args(&["--format", "markdown"]));
        let (text, code) = fmine_cli::run(&markdown, &workspace);
        assert_eq!(code, 0);
        assert!(text.contains("verified") && text.contains("Synthetic_AISC.xlsx"));
        assert!(!text.contains("PRIVATE_DISCOVERY_TEXT"));

        let profile = load_issuer_profile(&workspace, "synthetic").expect("registry");
        let source = workspace
            .join(&profile.corpus_dir)
            .join(&profile.sources[0].file_name);
        let mut bytes = std::fs::read(&source).expect("source fixture");
        bytes.push(0);
        std::fs::write(&source, bytes).expect("alter source fixture");
        assert_eq!(
            fmine_cli::invoke(&arguments, &workspace)
                .0
                .refusal
                .expect("source refusal")
                .code,
            "FMINE-REF-505"
        );

        for words in [
            vec!["doctor", "--discover"],
            vec!["assure", "public", "--synthetic", "--discover"],
            vec!["forecast", "assess", "--discover"],
        ] {
            assert_eq!(fmine_cli::invoke(&args(&words), &workspace).0.exit_code, 2);
        }
    }

    #[test]
    fn physical_discovery_uses_registered_source_display_precision() {
        use fmine_io::xlsx::{MinimalCell, MinimalSheet, MinimalStyles, build_minimal_xlsx_styled};
        use std::fmt::Write as _;

        let workspace = temporary_workspace("physical-discovery");
        let registry = fmine_public::synthetic::write_issuer_workspace(&workspace)
            .expect("synthetic registry");
        let text = |s: &str| MinimalCell::Text(s.into());
        let bytes = build_minimal_xlsx_styled(
            &[MinimalSheet {
                name: "Minestats1".into(),
                hidden: false,
                cells: vec![
                    ("B2".into(), text("Site A")),
                    ("A3".into(), text("For the three months ended March 31,")),
                    ("B3".into(), MinimalCell::Number(2030.0)),
                    ("A4".into(), text("Average grade (grams per tonne)")),
                    ("A5".into(), text("Processed")),
                    (
                        "B5".into(),
                        MinimalCell::Styled {
                            raw: "2.50".into(),
                            xf: 1,
                        },
                    ),
                    ("A7".into(), text("Recovery rate (percent)")),
                    (
                        "B7".into(),
                        MinimalCell::Styled {
                            raw: "0.915".into(),
                            xf: 2,
                        },
                    ),
                ],
            }],
            Some(&MinimalStyles {
                num_fmts: vec![(164, "0.00".into()), (165, "0.0%".into())],
                xfs: vec![0, 164, 165],
            }),
        );
        std::fs::write(workspace.join("corpus/Physical.xlsx"), &bytes)
            .expect("invented physical source");
        let mut registered = std::fs::read_to_string(&registry).expect("registry");
        writeln!(
            registered,
            "\n[[source]]\nfile = \"Physical.xlsx\"\nkind = \"mine_stats\"\n\
             quarter = \"2030Q1\"\npath = \"Physical.xlsx\"\n\
             url_basis = \"W1 table\"\n\
             bytes = {}\nsha256 = \"{}\"",
            bytes.len(),
            fmine_core::hash::sha256(&bytes).to_hex(),
        )
        .expect("source registration");
        std::fs::write(registry, registered).expect("registered source bytes");
        let draft = concat!(
            "Site A Q1 2030 ownership basis unprinted processed grade is 2.50 g/t.\n",
            "Site A Q1 2030 ownership basis unprinted recovery is 91.5%.\n",
            "Site A Q1 2030 ownership basis unprinted head grade is 2.500 g/t.\n",
            "Site A Q1 2030 ownership basis unprinted recovery rate is 0.915%.\n",
        );
        std::fs::write(workspace.join("physical.md"), draft).expect("draft");
        let arguments = args(&["lint", "physical.md", "--issuer", "synthetic", "--discover"]);
        let envelope = fmine_cli::invoke(&arguments, &workspace).0;
        let json = envelope.to_json();
        assert_eq!(envelope.exit_code, 0, "{json}");
        for (verdict, count) in [
            ("verified", 2),
            ("rounding_beyond_tolerance", 1),
            ("value_mismatch", 1),
        ] {
            assert_eq!(
                json.matches(&format!("\"verdict\":\"{verdict}\"")).count(),
                count,
                "{json}",
            );
        }
        assert!(json.contains("Physical.xlsx:Minestats1!B5"));
        assert!(json.contains("Physical.xlsx:Minestats1!B7"));
        assert!(json.contains("0.915") && json.contains("91.5%"));
        assert!(json.contains(FLAGS_JSON));
        assert_eq!(fmine_cli::invoke(&arguments, &workspace).0.to_json(), json);
        let mut markdown = arguments;
        markdown.extend(args(&["--format", "markdown"]));
        let (rendered, exit) = fmine_cli::run(&markdown, &workspace);
        assert_eq!(exit, 0);
        assert!(rendered.contains("rounding_beyond_tolerance"));
        assert!(rendered.contains("Physical.xlsx:Minestats1!B7"));
        assert_eq!(
            std::fs::read_to_string(workspace.join("physical.md")).expect("unchanged draft"),
            draft,
        );
    }

    #[test]
    fn cited_draft_runs_end_to_end_and_does_not_modify_the_workspace() {
        let workspace = temporary_workspace("read-only");
        fmine_public::synthetic::write_issuer_workspace(&workspace).expect("synthetic registry");
        let profile = load_issuer_profile(&workspace, "synthetic").expect("registry");
        let source = fmine_public::run_public_assurance(&profile, &workspace).expect("snapshot");
        let cell = &source.public_claims.cells["Synthetic_AISC.xlsx:Q1 2030!B16"];
        let id = cell.claim.claim_id;
        // The fixture prints 1250, independently of this checker. Every other line changes one predicate.
        let draft = format!(
            "Site A Q1 2030 ownership basis unprinted AISC per ounce sold is [$1,250/oz](fmine:{id}).\n\
             Site A Q1 2030 ownership basis unprinted AISC per ounce sold is [$1,300/oz](fmine:{id}).\n\
             Site A Q2 2030 ownership basis unprinted AISC per ounce sold is [$1,250/oz](fmine:{id}).\n\
             Site A Q1 2030 100% basis AISC per ounce sold is [$1,250/oz](fmine:{id}).\n\
             Site A Q1 2030 ownership basis unprinted AISC per ounce sold is not [$1,250/oz](fmine:{id}).\n\
             Site A Q1 2030 ownership basis unprinted AISC per ounce sold is $1,250/oz.\n\
             PRIVATE_UNRELATED_SENTENCE_MUST_NOT_BE_ECHOED.\n"
        );
        let path = workspace.join("draft.md");
        std::fs::write(&path, &draft).expect("local draft");
        let entries_before = std::fs::read_dir(&workspace).expect("directory").count();
        let arguments = args(&["lint", "draft.md", "--issuer", "synthetic"]);
        let (envelope, _) = fmine_cli::invoke(&arguments, &workspace);
        assert_eq!((envelope.status, envelope.exit_code), ("ok", 0));
        let json = envelope.to_json();
        assert!(json.contains("\"lane\":\"synthetic\""));
        for verdict in [
            "verified",
            "value_mismatch",
            "period_mismatch",
            "basis_mismatch",
            "unchecked",
            "uncited",
        ] {
            assert_eq!(
                json.matches(&format!("\"verdict\":\"{verdict}\"")).count(),
                1,
                "{verdict}: {json}"
            );
        }
        assert!(!json.contains("PRIVATE_UNRELATED_SENTENCE"));
        assert!(json.contains(&format!(
            "\"draft_sha256\":\"{}\"",
            fmine_core::hash::sha256(draft.as_bytes()).to_hex()
        )));
        assert_eq!(
            envelope.certificate,
            Some(source.ledger_anchor.state_root.to_hex())
        );
        assert_eq!(envelope.no_claim, Some(fmine_public::lint::NO_CLAIM));
        assert!(json.contains(FLAGS_JSON));
        assert_eq!(
            fmine_cli::invoke(&arguments, &workspace).0.to_json(),
            json,
            "byte-identical rerun"
        );
        let mut markdown = arguments.clone();
        markdown.extend(args(&["--format", "markdown"]));
        let (rendered, exit) = fmine_cli::run(&markdown, &workspace);
        assert_eq!(exit, 0);
        assert!(rendered.contains("verified") && rendered.contains("value_mismatch"));
        assert!(!rendered.contains("PRIVATE_UNRELATED_SENTENCE"));
        assert_eq!(
            std::fs::read_to_string(&path).expect("unchanged draft"),
            draft
        );
        assert_eq!(
            std::fs::read_dir(&workspace).expect("directory").count(),
            entries_before
        );

        // A previously usable snapshot cannot be silently cached after its registered bytes change.
        let input = workspace
            .join(&profile.corpus_dir)
            .join(&profile.sources[0].file_name);
        let mut altered = std::fs::read(&input).expect("fixture source");
        altered.push(0);
        std::fs::write(&input, altered).expect("alter synthetic fixture");
        let (refused, _) = fmine_cli::invoke(&arguments, &workspace);
        assert_eq!(
            refused.refusal.expect("altered source").code,
            "FMINE-REF-505"
        );
    }
}
