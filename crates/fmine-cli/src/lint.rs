//! Local draft adapter for the cited and discovery equality checkers (BD-345). A draft never leaves the process, and only
//! number spans and their verdicts reach stdout. The source snapshot is verified once for the entire draft.

use std::io::Read;
use std::path::Path;

use fmine_evidence::{Refusal, RefusalCode};
use fmine_public::sources::{is_issuer_name, load_issuer_profile, registry_path};

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

pub(super) fn execute(
    path: &Path,
    issuer: Option<&str>,
    through: Option<&str>,
    discover: bool,
    root: &Path,
) -> Envelope {
    let actions = || vec![NextAction::new("fmine robot-docs", crate::ROBOT_DOCS_WHY)];
    if issuer.is_none() && through.is_some() {
        return Envelope::usage_error("lint", "--through requires --issuer");
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
    let snapshot = match issuer {
        Some(name) => load_issuer_profile(root, name)
            .and_then(|profile| match through {
                Some(cut) => profile.through(cut),
                None => Ok(profile),
            })
            .and_then(|profile| fmine_public::run_public_assurance(&profile, root)),
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
    let cut = through.map_or_else(|| "null".to_string(), |s| format!("\"{}\"", escape_json(s)));
    let payload = format!(
        "{{\"schema\":\"fmine.lint_snapshot.v1\",\"issuer\":\"{}\",\"lane\":\"{}\",\
         \"through\":{cut},\"read_mode\":\"verified_source_projection\",\
         \"source_state_root\":\"{}\",\"source_mapping_refusals\":{{\"count\":{}}},\"lint\":{}}}",
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
    envelope.no_claim = Some(lint.mode.no_claim());
    envelope.rendered_markdown = Some(lint.to_markdown());
    envelope
}

#[cfg(test)]
mod tests {
    use super::*;
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
        let envelope = execute(&path, None, None, false, &root());
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
        let envelope = execute(&draft, Some("barrick"), Some("2026Q2"), false, &root());
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
        let historical = execute(&draft, Some("barrick"), Some("2024Q1"), false, &root());
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
