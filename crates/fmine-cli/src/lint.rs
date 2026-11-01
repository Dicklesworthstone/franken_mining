//! Local draft adapter for the cited and discovery equality checkers (BD-345). A draft never leaves the process, and only
//! number spans and their verdicts reach stdout. The source snapshot is verified once for the entire draft.

use std::io::Read;
use std::path::Path;

use crate::lint_publicness::{self, Publication};
use fmine_evidence::{Refusal, RefusalCode};
use fmine_public::sources::{IssuerProfile, is_issuer_name, registry_path};

use crate::{Envelope, NextAction, escape_json};

/// Bound the untrusted local draft before allocating or parsing it. This is a text checker, not a document
/// conversion engine; a larger document can be checked in explicit sections.
const MAX_DRAFT_BYTES: u64 = 4 * 1024 * 1024;

fn read_draft(path: &Path) -> Result<String, Refusal> {
    let refuse = |why: String| {
        Refusal::new(
            RefusalCode::InputMissingOrUnreadable506,
            format!("cannot read draft {}: {why}", path.display()),
        )
    };
    let file = std::fs::File::open(path).map_err(|e| refuse(e.to_string()))?;
    let mut draft = String::new();
    file.take(MAX_DRAFT_BYTES + 1)
        .read_to_string(&mut draft)
        .map_err(|e| refuse(e.to_string()))?;
    if draft.len() as u64 > MAX_DRAFT_BYTES {
        return Err(refuse("draft exceeds the four MiB text limit".to_string()));
    }
    Ok(draft)
}

fn load_profile(
    root: &Path,
    issuer: &str,
    through: Option<&str>,
) -> Result<(IssuerProfile, Vec<Publication>), Refusal> {
    let path = registry_path(root, issuer);
    let bad = |why: String| {
        Refusal::new(
            RefusalCode::InputMissingOrUnreadable506,
            format!("source registry {}: {why}", path.display()),
        )
    };
    // Parse once: the publication dates and numerical projection must refer to the same registry.
    let registry =
        fmine_registry::sources::load_sources_registry(&path).map_err(|e| bad(e.to_string()))?;
    let publications = registry
        .sources
        .iter()
        .filter_map(|source| {
            let (Some(date), Some(basis)) = (&source.published_at, &source.published_at_basis)
            else {
                return None;
            };
            Some(Publication {
                file: source.file.clone(),
                sha256: source.sha256.clone(),
                published_at: date.clone(),
                published_at_basis: basis.clone(),
            })
        })
        .collect();
    let profile = IssuerProfile::from_registry(registry).map_err(bad)?;
    let profile = match through {
        Some(cut) => profile.through(cut)?,
        None => profile,
    };
    Ok((profile, publications))
}

pub(super) fn execute(
    path: &Path,
    issuer: Option<&str>,
    through: Option<&str>,
    discover: bool,
    as_of: Option<&str>,
    root: &Path,
) -> Envelope {
    let actions = || vec![NextAction::new("fmine robot-docs", crate::ROBOT_DOCS_WHY)];
    if issuer.is_none() && (through.is_some() || as_of.is_some()) {
        return Envelope::usage_error("lint", "--through and --as-of require --issuer");
    }
    if as_of.is_some_and(|cut| !fmine_registry::sources::is_date(cut)) {
        return Envelope::usage_error("lint", "lint --as-of requires YYYY-MM-DD");
    }
    if let Some(name) = issuer
        && (!is_issuer_name(name) || !registry_path(root, name).is_file())
    {
        return Envelope::usage_error("lint", &format!("unknown issuer '{name}'"));
    }
    let draft = match read_draft(&root.join(path)) {
        Ok(draft) => draft,
        Err(refusal) => return Envelope::refused("lint", refusal, actions()),
    };
    let registered = match issuer {
        Some(name) => match load_profile(root, name, through) {
            Ok(profile) => Some(profile),
            Err(refusal) => return Envelope::refused("lint", refusal, actions()),
        },
        None => None,
    };
    let snapshot = match &registered {
        Some((profile, _)) => fmine_public::run_public_assurance(profile, root),
        None => fmine_public::run_synthetic_assurance(&[]),
    };
    let snapshot = match snapshot {
        Ok(snapshot) => snapshot,
        Err(refusal) => {
            return Envelope::refused(
                "lint",
                refusal,
                vec![NextAction::new("fmine doctor", crate::DOCTOR_WHY)],
            );
        }
    };
    let claims: Vec<_> = snapshot.public_claims.cells.values().cloned().collect();
    let lint = if discover {
        fmine_public::lint::lint_discover(&draft, &claims)
    } else {
        fmine_public::lint::lint_cited(&draft, &claims)
    };
    let public_record = match (as_of, registered.as_ref()) {
        (Some(cut), Some((profile, publications))) => {
            match lint_publicness::assess(&draft, &lint, &claims, profile, publications, cut) {
                Ok(report) => Some(report),
                Err(refusal) => return Envelope::refused("lint", refusal, actions()),
            }
        }
        _ => None,
    };
    let cut = through.map_or_else(|| "null".to_string(), |s| format!("\"{}\"", escape_json(s)));
    let as_of = as_of.map_or_else(|| "null".to_string(), |s| format!("\"{}\"", escape_json(s)));
    let public_json = public_record
        .as_ref()
        .map_or_else(|| "null".into(), |r| r.to_json());
    let payload = format!(
        "{{\"schema\":\"fmine.lint_snapshot.v1\",\"issuer\":\"{}\",\"lane\":\"{}\",\
         \"through\":{cut},\"as_of\":{as_of},\"read_mode\":\"verified_source_projection\",\
         \"source_state_root\":\"{}\",\"source_mapping_refusals\":{{\"count\":{}}},\"lint\":{},\
         \"public_record\":{public_json}}}",
        escape_json(&snapshot.issuer),
        snapshot.lane.as_str(),
        snapshot.ledger_anchor.state_root.to_hex(),
        snapshot.public_claims.refusals.len(),
        lint.to_json(),
    );
    let mut envelope = Envelope::report(
        "lint",
        snapshot.ledger_anchor.anchor_id().to_string(),
        snapshot.ledger_anchor.state_root.to_hex(),
        payload,
        actions(),
    );
    let mut markdown = lint.to_markdown();
    if let Some(report) = public_record {
        markdown.push('\n');
        markdown.push_str(&report.to_markdown());
        envelope.no_claim = Some(lint_publicness::NO_CLAIM);
    } else {
        envelope.no_claim = Some(lint.mode.no_claim());
    }
    envelope.rendered_markdown = Some(markdown);
    envelope
}

#[cfg(test)]
mod tests {
    use super::*;
    use fmine_public::sources::load_issuer_profile;
    use std::sync::atomic::{AtomicU64, Ordering};

    static WORKSPACE: AtomicU64 = AtomicU64::new(0);

    fn root() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .expect("workspace root")
            .to_path_buf()
    }

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

    const PUBLIC_RECORD_ISSUER: &str = "publicationtest";

    fn public_record_workspace(
        label: &str,
        published_at: Option<&str>,
    ) -> (std::path::PathBuf, std::path::PathBuf) {
        let workspace = temporary_workspace(label);
        let synthetic_registry = fmine_public::synthetic::write_issuer_workspace(&workspace)
            .expect("invented workbooks");
        let mut registry = std::fs::read_to_string(synthetic_registry)
            .expect("synthetic registry")
            .replace(
                "issuer = \"synthetic\"",
                &format!("issuer = \"{PUBLIC_RECORD_ISSUER}\""),
            )
            .replace(
                "retrieved_utc = \"synthetic\"",
                "retrieved_utc = \"2030-06-01T00:00:00Z\"",
            );
        if let Some(date) = published_at {
            let file = "file = \"Synthetic_AISC.xlsx\"\n";
            assert_eq!(registry.matches(file).count(), 1);
            registry = registry.replace(
                file,
                &format!(
                    "{file}published_at = \"{date}\"\n\
                     published_at_basis = \"FACT: invented release calendar for CLI regression\"\n"
                ),
            );
        }
        let path = registry_path(&workspace, PUBLIC_RECORD_ISSUER);
        std::fs::write(&path, registry).expect("registered imaginary issuer");
        (workspace, path)
    }

    fn public_record_component_digest<'a>(payload: &'a str, key: &str) -> &'a str {
        let key = format!("\"{key}\":");
        payload
            .split_once(key.as_str())
            .expect("report component")
            .1
            .split_once("\"report_digest\":\"")
            .expect("component digest")
            .1
            .split('"')
            .next()
            .expect("digest value")
    }

    #[test]
    fn public_record_cut_checks_registered_publication_without_changing_cited_equality() {
        let (workspace, registry) =
            public_record_workspace("public-record-cited", Some("2030-05-01"));
        let profile = load_issuer_profile(&workspace, PUBLIC_RECORD_ISSUER)
            .expect("registered imaginary issuer")
            .through("2030Q1")
            .expect("selected release");
        let snapshot = fmine_public::run_public_assurance(&profile, &workspace).expect("snapshot");
        let cell = &snapshot.public_claims.cells["Synthetic_AISC.xlsx:Q1 2030!B16"];
        assert_eq!(cell.claim.evidence, fmine_evidence::EvidenceClass::ISSUER);
        let id = cell.claim.claim_id.to_string();
        // Independently fixed fixture value; the report must preserve this exact source Claim.
        let draft = format!(
            "Site A Q1 2030 ownership basis unprinted AISC per ounce sold is [$1,250/oz](fmine:{id}).\n\
             PRIVATE_PUBLICATION_DRAFT_PROSE_MUST_NOT_BE_ECHOED.\n"
        );
        let draft_path = workspace.join("publication.md");
        std::fs::write(&draft_path, &draft).expect("draft");
        let arguments = |cut| {
            args(&[
                "lint",
                "publication.md",
                "--issuer",
                PUBLIC_RECORD_ISSUER,
                "--through",
                "2030Q1",
                "--as-of",
                cut,
            ])
        };
        let before = crate::invoke(&arguments("2030-04-30"), &workspace).0;
        let after_args = arguments("2030-05-03");
        let after = crate::invoke(&after_args, &workspace).0;
        for report in [&before, &after] {
            assert_eq!(report.exit_code, 0, "{}", report.to_json());
            assert!(report.payload_json.contains("\"verdict\":\"verified\""));
            assert!(report.payload_json.contains(&id));
            assert!(report.to_json().contains(crate::FLAGS_JSON));
            assert!(!report.to_json().contains("PRIVATE_PUBLICATION_DRAFT_PROSE"));
        }
        assert!(
            before
                .payload_json
                .contains("\"status\":\"not_in_public_record\"")
        );
        assert!(before.payload_json.contains("\"admission\":\"after_cut\""));
        assert!(after.payload_json.contains("\"status\":\"disclosed\""));
        assert!(
            after
                .payload_json
                .contains(&format!("\"first_registered_disclosure\":\"{id}\""))
        );
        assert_eq!(before.certificate, after.certificate);
        assert_eq!(
            public_record_component_digest(&before.payload_json, "lint"),
            public_record_component_digest(&after.payload_json, "lint"),
        );
        assert_ne!(
            public_record_component_digest(&before.payload_json, "public_record"),
            public_record_component_digest(&after.payload_json, "public_record"),
        );

        // Correcting metadata changes the public-record result's identity, never the workbook Claim.
        let original_registry = std::fs::read_to_string(&registry).expect("registry");
        assert_eq!(
            original_registry
                .matches("published_at = \"2030-05-01\"")
                .count(),
            1
        );
        std::fs::write(
            &registry,
            original_registry.replace(
                "published_at = \"2030-05-01\"",
                "published_at = \"2030-05-02\"",
            ),
        )
        .expect("correct registered date");
        let revised = crate::invoke(&after_args, &workspace).0;
        assert_eq!(revised.exit_code, 0, "{}", revised.to_json());
        assert!(revised.payload_json.contains("\"status\":\"disclosed\""));
        assert!(revised.payload_json.contains(&id));
        assert_eq!(after.certificate, revised.certificate);
        assert_eq!(
            public_record_component_digest(&after.payload_json, "lint"),
            public_record_component_digest(&revised.payload_json, "lint"),
        );
        assert_ne!(
            public_record_component_digest(&after.payload_json, "public_record"),
            public_record_component_digest(&revised.payload_json, "public_record"),
        );
        assert_eq!(revised, crate::invoke(&after_args, &workspace).0);

        let mut markdown = after_args;
        markdown.extend(args(&["--format", "markdown"]));
        let (text, exit) = crate::run(&markdown, &workspace);
        assert_eq!(exit, 0);
        assert!(text.contains("Public record at 2030-05-03"));
        assert!(text.contains("Synthetic_AISC.xlsx:Q1 2030!B16"));
        assert!(text.contains("2030-05-02"));
        assert!(!text.contains("PRIVATE_PUBLICATION_DRAFT_PROSE"));
        assert_eq!(
            std::fs::read_to_string(draft_path).expect("draft retained"),
            draft
        );
    }

    #[test]
    fn public_record_discovery_never_uses_retrieval_as_a_publication_date() {
        let (workspace, _) = public_record_workspace("public-record-undated", None);
        let draft = "Site A Q1 2030 ownership basis unprinted AISC per ounce sold is $1,250/oz.\n\
                     PRIVATE_UNDATED_DRAFT_PROSE_MUST_NOT_BE_ECHOED.\n";
        let path = workspace.join("undated.md");
        std::fs::write(&path, draft).expect("uncited draft");
        let arguments = args(&[
            "lint",
            "undated.md",
            "--issuer",
            PUBLIC_RECORD_ISSUER,
            "--through",
            "2030Q1",
            "--as-of",
            "2030-06-02",
            "--discover",
        ]);
        let result = crate::invoke(&arguments, &workspace).0;
        assert_eq!(result.exit_code, 0, "{}", result.to_json());
        assert!(result.payload_json.contains("\"verdict\":\"verified\""));
        assert!(
            result
                .payload_json
                .contains("\"status\":\"publication_date_unknown\"")
        );
        assert!(
            result
                .payload_json
                .contains("\"known_available_by_retrieval\":\"2030-06-01\"")
        );
        assert!(result.payload_json.contains("\"published_at\":null"));
        assert!(
            result
                .payload_json
                .contains("\"first_registered_disclosure\":null")
        );
        assert!(!result.payload_json.contains("\"status\":\"disclosed\""));
        assert!(!result.to_json().contains("PRIVATE_UNDATED_DRAFT_PROSE"));
        assert_eq!(result, crate::invoke(&arguments, &workspace).0);
        assert_eq!(
            std::fs::read_to_string(path).expect("unchanged draft"),
            draft
        );
    }

    #[test]
    fn public_record_discovery_rechecks_source_bytes_before_reusing_a_disclosure_result() {
        let (workspace, _) = public_record_workspace("public-record-tamper", Some("2030-05-01"));
        let draft = "Site A Q1 2030 ownership basis unprinted AISC per ounce sold is $1,250/oz.\n";
        let draft_path = workspace.join("discovery.md");
        std::fs::write(&draft_path, draft).expect("draft");
        let arguments = args(&[
            "lint",
            "discovery.md",
            "--issuer",
            PUBLIC_RECORD_ISSUER,
            "--as-of",
            "2030-06-02",
            "--discover",
        ]);
        let admitted = crate::invoke(&arguments, &workspace).0;
        assert_eq!(admitted.exit_code, 0, "{}", admitted.to_json());
        assert!(admitted.payload_json.contains("\"status\":\"disclosed\""));
        assert!(
            admitted
                .payload_json
                .contains("Synthetic_AISC.xlsx:Q1 2030!B16")
        );
        let source = workspace.join("corpus/Synthetic_AISC.xlsx");
        let mut bytes = std::fs::read(&source).expect("registered workbook");
        bytes.push(0);
        std::fs::write(&source, bytes).expect("alter fixture bytes");
        let refused = crate::invoke(&arguments, &workspace).0;
        assert_eq!(refused.status, "refused");
        assert_eq!(
            refused.refusal.as_ref().expect("integrity refusal").code,
            "FMINE-REF-505"
        );
        assert!(!refused.to_json().contains("\"status\":\"disclosed\""));
        assert_eq!(
            std::fs::read_to_string(draft_path).expect("unchanged draft"),
            draft
        );
    }

    #[test]
    fn lint_requires_an_explicit_snapshot_and_rejects_unimplemented_options() {
        for words in [
            vec!["lint"],
            vec!["lint", "draft.md"],
            vec!["lint", "draft.md", "--issuer", "barrick", "--synthetic"],
            vec!["lint", "draft.md", "--synthetic", "--through", "2026Q2"],
            vec!["lint", "draft.md", "--synthetic", "--as-of", "2026-10-31"],
            vec!["lint", "draft.md", "--synthetic", "--site", "north-mara"],
            vec!["lint", "draft.md", "--synthetic", "extra"],
        ] {
            let (env, _) = crate::invoke(&args(&words), &root());
            assert_eq!((env.verb.as_str(), env.exit_code), ("lint", 2), "{words:?}");
        }
        let (cmd, _, _) = crate::parse_args(
            &args(&[
                "lint",
                "draft.md",
                "--issuer",
                "barrick",
                "--through",
                "2026Q2",
            ]),
            &root(),
        )
        .expect("explicit snapshot");
        assert!(matches!(
            cmd,
            crate::CliCommand::Lint {
                issuer: Some(_),
                through: Some(_),
                ..
            }
        ));
    }

    #[test]
    fn missing_draft_is_a_named_refusal_and_is_never_created() {
        let path = root().join("corpus_of_input_docs/lint-does-not-exist.md");
        assert!(!path.exists());
        let envelope = execute(&path, None, None, false, None, &root());
        assert_eq!(envelope.status, "refused");
        assert!(
            envelope
                .refusal
                .expect("missing draft")
                .what
                .contains("lint-does-not-exist.md")
        );
        assert!(!path.exists());
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
        let cited = crate::invoke(&arguments, &workspace).0;
        assert_eq!(cited.exit_code, 0);
        assert!(cited.payload_json.contains("\"verdict\":\"uncited\""));

        arguments.push("--discover".to_string());
        let envelope = crate::invoke(&arguments, &workspace).0;
        let json = envelope.to_json();
        assert_eq!(envelope.exit_code, 0, "{json}");
        assert!(json.contains("\"verdict\":\"verified\""), "{json}");
        assert!(json.contains("Synthetic_AISC.xlsx:Q1 2030!B16"), "{json}");
        assert!(json.contains(crate::FLAGS_JSON));
        assert!(!json.contains("PRIVATE_DISCOVERY_TEXT"));
        assert_eq!(crate::invoke(&arguments, &workspace).0.to_json(), json);
        assert_eq!(
            std::fs::read_to_string(&path).expect("draft retained"),
            draft
        );

        let mut markdown = arguments.clone();
        markdown.extend(args(&["--format", "markdown"]));
        let (text, code) = crate::run(&markdown, &workspace);
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
            crate::invoke(&arguments, &workspace)
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
            assert_eq!(crate::invoke(&args(&words), &workspace).0.exit_code, 2);
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
        let envelope = crate::invoke(&arguments, &workspace).0;
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
        assert!(json.contains(crate::FLAGS_JSON));
        assert_eq!(crate::invoke(&arguments, &workspace).0.to_json(), json);
        let mut markdown = arguments;
        markdown.extend(args(&["--format", "markdown"]));
        let (rendered, exit) = crate::run(&markdown, &workspace);
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
        let (envelope, _) = crate::invoke(&arguments, &workspace);
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
        assert!(json.contains(crate::FLAGS_JSON));
        assert_eq!(
            crate::invoke(&arguments, &workspace).0.to_json(),
            json,
            "byte-identical rerun"
        );
        let mut markdown = arguments.clone();
        markdown.extend(args(&["--format", "markdown"]));
        let (rendered, exit) = crate::run(&markdown, &workspace);
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
        let (refused, _) = crate::invoke(&arguments, &workspace);
        assert_eq!(
            refused.refusal.expect("altered source").code,
            "FMINE-REF-505"
        );
    }

    #[test]
    fn lint_names_missing_registered_inputs_and_rejects_invalid_draft_bytes() {
        let workspace = temporary_workspace("missing");
        let registry = workspace.join("registries/sources");
        std::fs::create_dir_all(&registry).expect("registry directory");
        std::fs::copy(
            root().join("registries/sources/barrick.toml"),
            registry.join("barrick.toml"),
        )
        .expect("public registry");
        std::fs::write(workspace.join("draft.md"), "A local draft with no numbers.")
            .expect("draft");
        let envelope = execute(
            Path::new("draft.md"),
            Some("barrick"),
            Some("2026Q2"),
            false,
            None,
            &workspace,
        );
        let refusal = envelope.refusal.expect("missing corpus");
        assert_eq!(refusal.code, "FMINE-REF-506");
        assert!(refusal.what.contains("Barrick_Q2_2026_Mine_Stats.xlsx"));
        let invalid = workspace.join("invalid.md");
        std::fs::write(&invalid, [0xff, 0xfe]).expect("invalid text fixture");
        assert!(read_draft(&invalid).is_err());
    }

    #[test]
    #[ignore = "requires the registered Barrick workbooks through 2026Q2"]
    fn real_draft_verifies_north_mara_and_flags_e474_as_stale() {
        let workspace = temporary_workspace("real-draft");
        let draft = workspace.join("draft.md");
        std::fs::write(&draft, concat!(
            "North Mara Q2 2026 attributable AISC per ounce sold is [$2,577/oz](fmine:clm_f6e6c9b2ea703557f2a3b2f49b22b1aa).\n",
            "Total Gold Q1 2024 ownership basis unprinted AISC per ounce sold is [$1,479/oz](fmine:clm_4750828f283992dd0a3fbf0db5febf70).\n",
        )).expect("local real-reference draft");
        let envelope = execute(
            &draft,
            Some("barrick"),
            Some("2026Q2"),
            false,
            None,
            &root(),
        );
        assert_eq!(envelope.exit_code, 0, "{}", envelope.to_json());
        assert_eq!(
            envelope
                .payload_json
                .matches("\"verdict\":\"verified\"")
                .count(),
            1
        );
        assert_eq!(
            envelope
                .payload_json
                .matches("\"verdict\":\"stale\"")
                .count(),
            1
        );
        assert!(
            envelope
                .payload_json
                .contains("Barrick_Q2_2026_Mine_Stats.xlsx:QTD!G19")
        );
        let historical = execute(
            &draft,
            Some("barrick"),
            Some("2024Q1"),
            false,
            None,
            &root(),
        );
        assert_eq!(historical.exit_code, 0);
        assert_eq!(
            historical
                .payload_json
                .matches("\"verdict\":\"verified\"")
                .count(),
            1
        );
        assert!(!historical.payload_json.contains("\"verdict\":\"stale\""));
    }
}
