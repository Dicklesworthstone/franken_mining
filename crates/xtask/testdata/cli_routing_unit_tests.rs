#[cfg(test)]
mod tests {
    use super::*;

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
    fn test_track_record_as_of_envelope_preserves_original_outcomes_and_missing_members() {
        let report = fmine_public::guidance::track_record::synthetic;
        let env = track_record_envelope(report());
        assert_eq!((env.status, env.exit_code), ("ok", 0), "{}", env.to_json());
        assert_eq!(env.verb, "track-record");
        for required in [
            "\"schema\":\"fmine.guidance.track-record.as-of.v1\"",
            "\"lane\":\"synthetic\"",
            "\"gate_eligible\":false",
            "\"n_members\":2,\"n_scored\":1,\"n_unresolved\":1,\"n_inside\":0",
            "\"n_members\":2,\"n_scored\":1,\"n_unresolved\":1,\"n_inside\":1",
            "\"z\":\"3/2\"",
            "\"z\":\"1/2\"",
            "\"cp90_iid\":{",
            "\"whole_cohort_bounds\":{\"lo\":\"0\",\"hi\":\"1/2\"",
            "Synthetic_2030Q4.xlsx:YTD!C4",
            "Synthetic_2031Q4.xlsx:YTD!C4",
            "historical completeness unassessed",
            "Cost scores are unconditional",
        ] {
            assert!(
                env.payload_json.contains(required),
                "missing {required}: {}",
                env.payload_json
            );
        }
        assert!(env.anchor_id.is_none());
        assert!(env.certificate.is_none());
        assert_eq!(
            env.no_claim,
            Some(fmine_public::guidance::track_record::NO_CLAIM)
        );
        assert!(env.to_json().contains(FLAGS));
        assert_eq!(track_record_envelope(report()), env);
        let markdown = env
            .rendered_markdown
            .as_deref()
            .expect("readable as-of report");
        assert!(markdown.starts_with("# Guidance track record"));
        assert!(markdown.contains("125; above; z 3/2"));
        assert!(markdown.contains("115; inside; z 1/2"));
        assert!(markdown.contains("1 unresolved of 2 members"));
        assert!(markdown.contains("## Sources"));
        assert!(markdown.contains("Synthetic invented evidence"));
    }

    /// BD-274 over the real workbooks: the gate-eligible run's payload ends with the seven published cut-offs.
    #[test]
    #[ignore = "needs the registered Barrick workbooks (through 2026Q2) under corpus_of_input_docs/downloaded_docs"]
    fn test_assure_public_issuer_run_carries_the_published_cutoffs() {
        let env = envelope_for(&[
            "assure",
            "public",
            "--issuer",
            "barrick",
            "--through",
            "2026Q2",
        ]);
        assert_eq!(env.exit_code, 0, "{}", env.to_json());
        let section = published_cutoffs(&workspace_root(), "barrick")
            .expect("reads")
            .expect("barrick rows");
        let head = &section[..section.find(",\"rows\"").expect("rows")];
        println!("{head}}}");
        assert!(
            env.payload_json
                .ends_with(&format!(",\"published_cutoffs\":{section}}}")),
            "{head}"
        );
        assert_eq!(section.matches("{\"id\":").count(), 7);
    }

    /// BD-274: an issuer run carries the workspace's published cut-offs for the registry's own issuer only. No
    /// registry is no section; a registry that does not read refuses (506) rather than dropping rows.
    #[test]
    fn test_published_cutoffs_for_the_registry_issuer_only() {
        let root = workspace_root();
        let section = published_cutoffs(&root, "barrick")
            .expect("reads")
            .expect("barrick rows");
        assert!(
            section.contains(
                "\"cases\":7,\"reproduced\":6,\"within_input_rounding\":1,\"conflict\":0,"
            ),
            "{section}"
        );
        assert_eq!(published_cutoffs(&root, "newmont").expect("reads"), None);

        let ws = std::env::temp_dir().join(format!("fmine-cli-cutoffs-{}", std::process::id()));
        let reg = ws.join(cutoff::CUTOFFS_REGISTRY);
        std::fs::create_dir_all(reg.parent().expect("dir")).expect("registry dir");
        assert_eq!(
            published_cutoffs(&ws, "barrick").expect("no registry"),
            None
        );
        let text = std::fs::read_to_string(root.join(cutoff::CUTOFFS_REGISTRY)).expect("registry");
        std::fs::write(
            &reg,
            text.replacen("published = \"5.66\"", "published = 5.66", 1),
        )
        .expect("write");
        let refusal = published_cutoffs(&ws, "barrick").expect_err("malformed row");
        assert_eq!(refusal.code, "FMINE-REF-506");
        assert!(
            refusal.what.contains("published_cutoffs.toml"),
            "{}",
            refusal.what
        );
        let _ = std::fs::remove_dir_all(&ws);
    }

    /// The section is the payload's last field and leaves the anchor and certificate alone: it is not in the root.
    /// The synthetic lane never carries it.
    #[test]
    fn test_published_cutoffs_join_the_payload_outside_the_state_root() {
        let report = run_synthetic_assurance(&[]).expect("synthetic");
        let section = published_cutoffs(&workspace_root(), "barrick")
            .expect("reads")
            .expect("barrick rows");
        let (bare, with) = (
            report_envelope(&report, None),
            report_envelope(&report, Some(&section)),
        );
        assert_eq!(
            (&bare.anchor_id, &bare.certificate),
            (&with.anchor_id, &with.certificate)
        );
        let body = bare.payload_json.strip_suffix('}').expect("object");
        assert_eq!(
            with.payload_json,
            format!("{body},\"published_cutoffs\":{section}}}")
        );
        let synthetic = envelope_for(&["assure", "public", "--synthetic"]);
        assert_eq!(synthetic.exit_code, 0);
        assert!(!synthetic.payload_json.contains("published_cutoffs"));
    }

    /// BD-357.S2: `fmine assure public` refuses a workspace carrying a salted review-packet manifest
    /// before loading any source, emitting no report, anchor or certificate.
    #[test]
    fn test_assure_public_refuses_salted_packet_workspace() {
        let ws =
            std::env::temp_dir().join(format!("fmine-cli-salted-refusal-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&ws);
        std::fs::create_dir_all(&ws).expect("temp dir");
        std::fs::write(
            ws.join(fmine_public::REVIEW_PACKET_MANIFEST),
            "packet = \"salted\"\nround_id = \"round-1\"\nitem_ids = [\"item-0001\"]\n",
        )
        .expect("write salted manifest");

        let ws_str = ws.to_string_lossy().into_owned();
        let (env, _) = invoke(
            &args(&[
                "assure",
                "public",
                "--issuer",
                "barrick",
                "--workspace",
                &ws_str,
            ]),
            &workspace_root(),
        );
        let expected_spec = RefusalCode::MnpiWall301.spec();
        assert_eq!(env.status, "refused", "{}", env.to_json());
        assert_eq!(env.exit_code, expected_spec.exit_code);
        assert_eq!(
            env.refusal.as_ref().expect("refusal").code,
            expected_spec.code
        );
        assert_eq!(env.payload_json, "null");
        assert!(env.anchor_id.is_none());
        assert!(env.certificate.is_none());

        // Malformed manifest also fails closed with FMINE-REF-301.
        std::fs::write(
            ws.join(fmine_public::REVIEW_PACKET_MANIFEST),
            "packet = \"bogus\"\n",
        )
        .expect("write malformed manifest");
        let (bad_env, _) = invoke(
            &args(&[
                "assure",
                "public",
                "--issuer",
                "barrick",
                "--workspace",
                &ws_str,
            ]),
            &workspace_root(),
        );
        assert_eq!(bad_env.status, "refused");
        assert_eq!(
            bad_env.refusal.as_ref().expect("refusal").code,
            expected_spec.code
        );
        assert_eq!(bad_env.payload_json, "null");

        let _ = std::fs::remove_dir_all(&ws);
    }

    /// BD-357.S2 positive control: an unsalted manifest leaves `fmine assure public` behaviour
    /// identical to a workspace with no `review-packet.toml`.
    #[test]
    fn test_assure_public_accepts_unsalted_manifest_path_unchanged() {
        let pid = std::process::id();
        let ws_bare = std::env::temp_dir().join(format!("fmine-cli-unsalted-bare-{pid}"));
        let ws_unsalted = std::env::temp_dir().join(format!("fmine-cli-unsalted-manifest-{pid}"));
        for dir in [&ws_bare, &ws_unsalted] {
            let _ = std::fs::remove_dir_all(dir);
            let reg_dir = dir.join("registries").join("sources");
            std::fs::create_dir_all(&reg_dir).expect("reg dir");
            std::fs::copy(
                workspace_root().join("registries/sources/barrick.toml"),
                reg_dir.join("barrick.toml"),
            )
            .expect("copy barrick registry");
        }
        std::fs::write(
            ws_unsalted.join(fmine_public::REVIEW_PACKET_MANIFEST),
            "packet = \"unsalted\"\nround_id = \"round-1\"\nitem_ids = [\"item-0001\"]\n",
        )
        .expect("write unsalted manifest");

        let bare_str = ws_bare.to_string_lossy().into_owned();
        let unsalted_str = ws_unsalted.to_string_lossy().into_owned();
        let (env_bare, _) = invoke(
            &args(&[
                "assure",
                "public",
                "--issuer",
                "barrick",
                "--workspace",
                &bare_str,
            ]),
            &workspace_root(),
        );
        let (env_unsalted, _) = invoke(
            &args(&[
                "assure",
                "public",
                "--issuer",
                "barrick",
                "--workspace",
                &unsalted_str,
            ]),
            &workspace_root(),
        );
        assert_eq!(env_bare.status, "refused");
        assert_eq!(env_unsalted.status, env_bare.status);
        assert_eq!(env_unsalted.exit_code, env_bare.exit_code);
        assert_eq!(
            env_unsalted.refusal.as_ref().expect("refusal").code,
            RefusalCode::InputMissingOrUnreadable506.spec().code
        );
        assert_eq!(
            env_unsalted.refusal.as_ref().expect("refusal").code,
            env_bare.refusal.as_ref().expect("refusal").code
        );

        let _ = std::fs::remove_dir_all(&ws_bare);
        let _ = std::fs::remove_dir_all(&ws_unsalted);
    }

    /// The four INV-002 flags, all false, written by hand from BD-329.S3 (not taken from `FLAGS_JSON`).
    const FLAGS: &str = "{\"reserve_disclosure_claim\":false,\"execution_authorized\":false,\
                         \"geotechnical_signoff\":false,\"regulatory_compliance_claim\":false}";

    fn envelope_for(a: &[&str]) -> Envelope {
        invoke(&args(a), &workspace_root()).0
    }
}
