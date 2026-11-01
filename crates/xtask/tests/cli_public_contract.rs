//! Public CLI contracts exercised across the crate boundary; production internals stay unit tested.

#[cfg(test)]
mod public_adapters {
    use std::path::Path;

    use fmine_cli::packet::{ChallengePacket, packets_from_report};
    use fmine_public::aisc::{self, IdentityState, OperandValue};
    use fmine_public::run_synthetic_assurance;
    use fmine_public::sources::SourceKind;

    #[test]
    fn ordering_does_not_change_packet_ids_or_bytes() {
        let mut report = run_synthetic_assurance(&[]).expect("synthetic assurance");
        let original = packets_from_report(&report).expect("packets");
        report.instances.reverse();
        report.sources.reverse();
        for instance in &mut report.instances {
            instance.operands.reverse();
            if let IdentityState::IssuerDiscrepancy(c) = &mut instance.state {
                c.line_cells.reverse();
            }
        }
        let reordered = packets_from_report(&report).expect("reordered packets");
        assert_eq!(
            original
                .iter()
                .map(ChallengePacket::to_json)
                .collect::<Vec<_>>(),
            reordered
                .iter()
                .map(ChallengePacket::to_json)
                .collect::<Vec<_>>()
        );
        assert_eq!(
            original
                .iter()
                .map(ChallengePacket::to_markdown)
                .collect::<Vec<_>>(),
            reordered
                .iter()
                .map(ChallengePacket::to_markdown)
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn missing_source_and_mismatched_ledger_operand_refuse() {
        let mut report = run_synthetic_assurance(&[]).expect("synthetic assurance");
        report
            .sources
            .retain(|s| s.kind != SourceKind::AiscReconciliation);
        assert_eq!(
            packets_from_report(&report)
                .expect_err("source missing")
                .code,
            "FMINE-REF-506"
        );
        let mut report = run_synthetic_assurance(&[]).expect("synthetic assurance");
        let instance = report
            .instances
            .iter_mut()
            .find(|i| i.subject == "Site B" && i.family == aisc::IdentityFamily::TccFooting)
            .expect("instance");
        instance.operands[0].value = OperandValue::Int(999);
        assert_eq!(
            packets_from_report(&report)
                .expect_err("source differs")
                .code,
            "FMINE-REF-208"
        );
    }

    #[test]
    fn typed_execute_rejects_reserved_synthetic_issuer() {
        use fmine_cli::mirror::{Command as MirrorCommand, execute, parse};
        let args: Vec<String> = [
            "mirror",
            "--asset",
            "example-jv",
            "--period",
            "2025",
            "--as-of",
            "2026-02-01",
            "--left-issuer",
            "alpha",
            "--left-document",
            "Left.pdf",
            "--right-issuer",
            "beta",
            "--right-document",
            "Right.pdf",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect();
        let MirrorCommand::Public(mut selection) = parse(&args).expect("valid explicit selection")
        else {
            panic!("explicit reporters must select the public path");
        };
        selection.reporters[0].issuer = "synthetic".into();
        let report = execute(
            &MirrorCommand::Public(selection),
            Path::new("unused-workspace"),
        );
        assert_eq!(
            report.refusal.as_ref().expect("typed refusal").code,
            "FMINE-REF-107"
        );
        assert_ne!(report.exit_code, 0);
        assert!(!report.payload_json.contains("\"status\":\"consistent\""));
    }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use self::workspace_root as root;
    use fmine_cli::{CliCommand, Envelope, execute_command, invoke, parse_args};
    use fmine_evidence::{Refusal, RefusalCode};
    use fmine_public::run_public_assurance;
    use fmine_public::sources::{load_issuer_profile, registry_path};

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

    /// A synthetic issuer workspace: `assure public --issuer synthetic` runs there on invented numbers.
    fn synthetic_workspace(tag: &str) -> PathBuf {
        let ws = scratch(tag);
        fmine_public::synthetic::write_issuer_workspace(&ws).expect("synthetic workspace");
        ws
    }

    #[test]
    #[ignore = "requires verified Barrick workbooks, PDFs and local two-reader recovery inputs"]
    fn test_real_recovery_is_embedded_in_assurance_with_separate_digest() {
        let root = workspace_root();
        let recovery = fmine_public::recovery::run_public(&root, "barrick", None)
            .expect("registered recovery evidence");
        assert_eq!(recovery.quarters.len(), 14);
        assert!(recovery.quarters.iter().all(|q| q.outcome.is_ok()));
        let (env, _) = invoke(&args(&["assure", "public", "--issuer", "barrick"]), &root);
        assert_eq!((env.status, env.exit_code), ("ok", 0), "{:?}", env.refusal);
        assert!(
            env.payload_json
                .contains(&format!("\"recovery\":{}", recovery.to_json()))
        );
        let roots = fmine_public::sources::load_expected_roots(
            &fmine_public::sources::expected_roots_path(&root, "barrick"),
        )
        .expect("existing identity/waterfall roots");
        let expected = roots.last().expect("current registered root");
        assert_eq!(
            env.certificate.as_deref(),
            Some(expected.state_root.as_str())
        );
        assert_eq!(env.anchor_id.as_deref(), Some(expected.anchor_id.as_str()));
        assert_ne!(recovery.digest().to_hex(), expected.state_root);
        assert!(env.to_json().contains(FLAGS));
    }

    const STATE_I: &str = "\"state\":\"i\"";
    const ISSUER_ASSERTION: &str = "\"evidence\":\"Assertion\",\"source\":\"issuer\"";

    /// The objects of every `"key":[{..},..]` array, with braces balanced outside strings.
    fn json_objects<'a>(json: &'a str, key: &str) -> Vec<&'a str> {
        let mut found = Vec::new();
        for (at, _) in json.match_indices(&format!("\"{key}\":[{{")) {
            let start = at + key.len() + 4;
            let (mut from, mut depth, mut in_str, mut escaped) = (start, 0_u32, false, false);
            for (i, c) in json[start..].char_indices() {
                match c {
                    _ if escaped => escaped = false,
                    '\\' if in_str => escaped = true,
                    '"' => in_str = !in_str,
                    '{' if !in_str => {
                        if depth == 0 {
                            from = start + i;
                        }
                        depth += 1;
                    }
                    '}' if !in_str => {
                        depth -= 1;
                        if depth == 0 {
                            found.push(&json[from..=start + i]);
                        }
                    }
                    ']' if !in_str && depth == 0 => break,
                    _ => {}
                }
            }
        }
        found
    }

    /// bd-306n.3: an issuer's figure is reported as an assertion, whether it reproduced or not. Reproducing shows
    /// its figures agree with each other; it does not show they are true.
    #[test]
    fn test_assure_synthetic_reports_assertion_issuer() {
        let root = workspace_root();
        let (cmd, _, ws) =
            parse_args(&args(&["assure", "public", "--synthetic"]), &root).expect("parse");
        let env = execute_command(&cmd, &ws);
        assert_eq!((env.exit_code, env.status), (0, "ok"));
        let json = env.to_json();
        let instances = json_objects(&json, "instances");
        // The walk must reach the 40 state-(i) instances, or the loop below passes on nothing.
        let reproduced = instances.iter().filter(|i| i.contains(STATE_I)).count();
        assert_eq!(reproduced, 40, "{json}");
        // 2 + 1 + 2 in the visible scope, and the one the hidden sheet `Q4 2029` holds.
        let exceptions = json_objects(&json, "exceptions");
        assert_eq!(exceptions.len(), 6, "{json}");
        for instance in instances.iter().chain(&exceptions) {
            assert!(instance.contains(ISSUER_ASSERTION), "{instance}");
        }
        println!(
            "{{\"test\":\"assure_synthetic_assertion_issuer\",\"instances\":{},\"exceptions\":{},\
             \"non_assertion\":0}}",
            instances.len(),
            exceptions.len()
        );
    }

    /// bd-306n.3 over the real workbooks: one line per workbook, then a summary. Run at the 2026Q2 bound, so a
    /// later quarter's registry row changes neither the file set nor E822's count.
    #[test]
    #[ignore = "needs the registered Barrick workbooks (through 2026Q2) under corpus_of_input_docs/downloaded_docs"]
    fn test_assure_public_reports_every_barrick_instance_as_issuer_assertion() {
        let root = workspace_root();
        let (cmd, _, ws) = parse_args(
            &args(&[
                "assure",
                "public",
                "--issuer",
                "barrick",
                "--through",
                "2026Q2",
            ]),
            &root,
        )
        .expect("parse");
        let env = execute_command(&cmd, &ws);
        assert_eq!(env.exit_code, 0, "{}", env.to_json());
        let json = env.to_json();
        let mut by_file: std::collections::BTreeMap<&str, (u32, u32)> =
            std::collections::BTreeMap::new();
        for instance in json_objects(&json, "instances") {
            let at = instance.strip_prefix("{\"at\":\"").expect("at");
            let file = at.split(':').next().expect("file");
            let row = by_file.entry(file).or_default();
            row.0 += 1;
            row.1 += u32::from(instance.contains(ISSUER_ASSERTION));
        }
        let (mut total, mut other) = (0, 0);
        for (file, (instances, assertion_issuer)) in &by_file {
            let other_class = instances - assertion_issuer;
            println!(
                "{{\"file\":\"{file}\",\"instances\":{instances},\"assertion_issuer\":{assertion_issuer},\
                 \"other_class\":{other_class}}}"
            );
            total += instances;
            other += other_class;
        }
        println!(
            "{{\"files\":{},\"instances\":{total},\"non_assertion\":{other}}}",
            by_file.len()
        );
        // Instances come from the registered AISC workbooks; Mine Stats files carry none.
        let want: std::collections::BTreeSet<String> = load_issuer_profile(&root, "barrick")
            .and_then(|profile| profile.through("2026Q2"))
            .expect("registry")
            .sources
            .into_iter()
            .filter(|s| s.kind == fmine_public::sources::SourceKind::AiscReconciliation)
            .map(|s| s.file_name)
            .collect();
        let got: std::collections::BTreeSet<String> =
            by_file.keys().map(|f| (*f).to_string()).collect();
        assert_eq!(got, want);
        // E822's count at the 2026Q2 bound.
        assert_eq!(total, 2595);
        assert_eq!(other, 0);
    }

    /// A synthetic issuer workspace that `doctor` can also load: every top-level catalogue registry copied in.
    fn doctor_workspace(tag: &str) -> PathBuf {
        let ws = synthetic_workspace(tag);
        let registries = workspace_root().join("registries");
        for entry in std::fs::read_dir(&registries).expect("registries dir") {
            let path = entry.expect("entry").path();
            if path.extension().is_some_and(|e| e == "toml") {
                let name = path.file_name().expect("file name");
                std::fs::copy(&path, ws.join("registries").join(name))
                    .expect("copy catalogue registry");
            }
        }
        ws
    }

    /// BD-339.S2: doctor checks each registered file's size and sha256. Every synthetic file verifies; after one
    /// byte of one file is flipped, that file alone is present with `digest_ok: false`.
    #[test]
    fn test_doctor_reports_digest_per_file() {
        let ws = doctor_workspace("doctor-digest");
        let profile = load_issuer_profile(&ws, "synthetic").expect("synthetic registry");
        let n = profile.sources.len();
        let env = execute_command(&CliCommand::Doctor, &ws);
        let json = env.to_json();
        assert!(
            json.contains(&format!(
                "{{\"issuer\":\"synthetic\",\"registry_ok\":true,\"sources_registered\":{n},\"sources_present\":{n},\"sources_verified\":{n},"
            )),
            "{json}"
        );
        let first = &profile.sources[0];
        let path = ws.join(&profile.corpus_dir).join(&first.file_name);
        let mut bytes = std::fs::read(&path).expect("synthetic file");
        bytes[0] ^= 1;
        std::fs::write(&path, &bytes).expect("flip one byte");
        let json = execute_command(&CliCommand::Doctor, &ws).to_json();
        assert!(
            json.contains(&format!(
                "\"sources_present\":{n},\"sources_verified\":{},",
                n - 1
            )),
            "{json}"
        );
        assert!(
            json.contains(&format!(
                "{{\"file\":\"{}\",\"present\":true,\"bytes_ok\":true,\"digest_ok\":false}}",
                first.file_name
            )),
            "{json}"
        );
        let missing = &profile.sources[1];
        std::fs::rename(
            ws.join(&profile.corpus_dir).join(&missing.file_name),
            ws.join(&profile.corpus_dir).join("moved_aside"),
        )
        .expect("move a scratch file aside");
        let json = execute_command(&CliCommand::Doctor, &ws).to_json();
        assert!(
            json.contains(&format!(
                "{{\"file\":\"{}\",\"present\":false,\"bytes_ok\":false,\"digest_ok\":false}}",
                missing.file_name
            )),
            "{json}"
        );
    }

    /// BD-339.S2: a digest mismatch is a diagnosis, not a refusal.
    #[test]
    fn test_doctor_never_refuses_on_digest_mismatch() {
        let ws = doctor_workspace("doctor-norefuse");
        let profile = load_issuer_profile(&ws, "synthetic").expect("synthetic registry");
        let path = ws
            .join(&profile.corpus_dir)
            .join(&profile.sources[0].file_name);
        std::fs::write(&path, b"not the registered bytes").expect("overwrite scratch file");
        let env = execute_command(&CliCommand::Doctor, &ws);
        assert_eq!((env.exit_code, env.status), (0, "ok"), "{}", env.to_json());
        assert!(
            env.to_json()
                .contains("\"present\":true,\"bytes_ok\":false,\"digest_ok\":false"),
            "{}",
            env.to_json()
        );
    }

    const FLAG_NAMES: [&str; 4] = [
        "reserve_disclosure_claim",
        "execution_authorized",
        "geotechnical_signoff",
        "regulatory_compliance_claim",
    ];

    fn envelope_for(a: &[&str]) -> Envelope {
        invoke(&args(a), &workspace_root()).0
    }

    /// Gate run on the real registered workbooks (public lane; the files are not in git). Run with
    /// `cargo test -p fmine-cli -- --ignored` where `corpus_of_input_docs/downloaded/mine_stats/` is present.
    #[test]
    #[ignore = "needs the registered Barrick workbooks (through 2026Q2) under corpus_of_input_docs/downloaded/mine_stats/"]
    fn test_cli_assure_public_barrick_real_files() {
        let root = workspace_root();
        let (cmd, _, ws) = parse_args(
            &args(&[
                "assure",
                "public",
                "--issuer",
                "barrick",
                "--through",
                "2026Q2",
            ]),
            &root,
        )
        .expect("parse");
        let env = execute_command(&cmd, &ws);
        assert_eq!((env.exit_code, env.status), (0, "ok"), "{}", env.to_json());
        let json = env.to_json();
        assert!(json.contains("\"lane\":\"public\",\"gate_eligible\":true"));
        assert!(json.contains("\"through\":\"2026Q2\""));
        // Regression pin of this implementation's output, not an independent value: the root and anchor of the
        // last row of repro/barrick/expected.toml (BD-339.S1), which records why each root moved.
        let expected = fmine_public::sources::load_expected_roots(
            &fmine_public::sources::expected_roots_path(&root, "barrick"),
        )
        .expect("expected roots");
        let last = expected.last().expect("at least one row");
        assert!(
            json.contains(&format!("\"state_root\":\"{}\"", last.state_root)),
            "root differs from the expected row of {}",
            last.commit
        );
        assert_eq!(
            env.anchor_id.as_deref(),
            Some(last.anchor_id.as_str()),
            "anchor differs from the expected row of {}",
            last.commit
        );
        assert!(json.contains("\"aisc_tables\":34,\"instances\":2595"));
        // BD-331.S2 (census E828): the hidden scope is reported, labelled and never counted. BD-331.S4 (region
        // census, E840): its three regional rates move v → i twice and v → iv once.
        assert!(json.contains(
            "\"hidden_scope\":{\"scope\":\"sheet not displayed to readers\",\"counted\":false,\
             \"aisc_tables\":6,\"instances\":435,\"by_state\":{\"i\":406,\"ii\":0,\"iii\":1,\"iv\":18,\"v\":0,\"np\":10}"
        ));
        assert!(json.contains(
            // BD-331.S3 (census E826): 263 stated-share fits move v → i and 65 misses v → iv. bd-jg43.3 (census
            // E838): E823's 12 Loulo-Gounkoto instances move v → i on displayed values. BD-331.S4 (region census,
            // E840): of the 51 regional rates, 41 move v → i and 10 v → iv.
            "\"totals\":{\"i\":2436,\"ii\":3,\"iii\":4,\"iv\":92,\"v\":0,\"np\":60}"
        ));
        // The public-lane envelope: the root as certificate, the no-claim line, and its two next actions.
        assert_eq!(env.certificate.as_deref(), Some(last.state_root.as_str()));
        assert_eq!(env.no_claim, Some(fmine_public::NO_CLAIM));
        let commands: Vec<&str> = env
            .next_actions
            .iter()
            .map(|a| a.command.as_str())
            .collect();
        assert_eq!(
            commands,
            ["fmine assure public --synthetic", "fmine robot-docs"]
        );
    }

    /// bd-plx1's e2e at the envelope: a synthetic workspace with one registered document and a two-reader
    /// transcription registry runs and prints the `transcriptions` section; one reader's altered token refuses
    /// (exit 8) naming the locator; a removed document refuses (exit 8) naming the file. Rows are invented.
    #[test]
    fn test_assure_public_transcriptions_refuse_by_locator_and_document() {
        let ws = synthetic_workspace("transcribe");
        let doc: &[u8] = b"%PDF-1.7 synthetic document, never parsed";
        std::fs::write(ws.join("corpus").join("Doc.pdf"), doc).expect("document");
        let sha = fmine_core::hash::sha256(doc).to_hex();
        let reg = registry_path(&ws, "synthetic");
        let mut toml = std::fs::read_to_string(&reg).expect("registry");
        toml.push_str(&format!(
            "\n[[document]]\nfile = \"Doc.pdf\"\nkind = \"mda\"\nurl = \"https://example.invalid/Doc.pdf\"\n\
             url_basis = \"W1 table\"\npublished_at = \"2030-02-01\"\npublished_at_basis = \"FACT: synthetic\"\n\
             pages = 3\nbytes = {}\nsha256 = \"{sha}\"\n",
            doc.len()
        ));
        std::fs::write(&reg, toml).expect("registry");
        let rows = |second: &str| {
            let entry = |reader: &str, token: &str| {
                format!(
                    "\n[[entry]]\ndoc = \"Doc.pdf\"\ndoc_sha256 = \"{sha}\"\npage = 2\ntable = \"Table\"\nrow = \"Row\"\n\
                     col = \"Col\"\ntoken = \"{token}\"\nquantity = \"q\"\nunit = \"Moz\"\nbasis = \"attributable\"\n\
                     period = \"2030\"\nreader = \"{reader}\"\n"
                )
            };
            format!(
                "schema = \"fmine.transcriptions.v1\"\nissuer = \"synthetic\"\n{}{}",
                entry("reader_a", "(1.5)"),
                entry("reader_b", second)
            )
        };
        let path = fmine_public::sources::transcriptions_path(&ws, "synthetic");
        std::fs::create_dir_all(path.parent().expect("parent")).expect("dir");
        let cmd = CliCommand::AssurePublic {
            issuer: "synthetic".to_string(),
            through: None,
        };
        std::fs::write(&path, rows("(1.5)")).expect("rows");
        let env = execute_command(&cmd, &ws);
        assert_eq!((env.exit_code, env.status), (0, "ok"), "{}", env.to_json());
        assert!(
            env.to_json()
                .contains("\"entries\":2,\"accepted\":1,\"accepted_digest\":"),
            "{}",
            env.to_json()
        );
        // The readers disagree, so the value is not observable from the evidence (202, exit 3).
        std::fs::write(&path, rows("(1.6)")).expect("rows");
        let env = execute_command(&cmd, &ws);
        assert_eq!((env.exit_code, env.status), (3, "refused"));
        let json = env.to_json();
        assert!(json.contains("\"code\":\"FMINE-REF-202\""), "{json}");
        assert!(
            json.contains("at Doc.pdf:p2:Table:Row/Col (reader_a, reader_b: token)"),
            "{json}"
        );
        assert!(
            !json.contains("1.6") && json.contains("\"payload\":null"),
            "{json}"
        );
        std::fs::write(&path, rows("(1.5)")).expect("rows");
        // A registered document that is altered is a digest mismatch (505, exit 8); one that is gone is
        // foundation I/O (506, exit 5).
        std::fs::write(ws.join("corpus").join("Doc.pdf"), b"%PDF-1.4 altered")
            .expect("alter document");
        let env = execute_command(&cmd, &ws);
        assert_eq!((env.exit_code, env.status), (8, "refused"));
        let json = env.to_json();
        assert!(json.contains("\"code\":\"FMINE-REF-505\""), "{json}");
        std::fs::remove_file(ws.join("corpus").join("Doc.pdf")).expect("remove document");
        let env = execute_command(&cmd, &ws);
        assert_eq!((env.exit_code, env.status), (5, "refused"));
        let json = env.to_json();
        assert!(json.contains("\"code\":\"FMINE-REF-506\""), "{json}");
        assert!(json.contains("missing: Doc.pdf"), "{json}");
        let _ = std::fs::remove_dir_all(&ws);
    }

    /// One envelope for each case of the PO-ADIT-1 golden, named.
    fn every_verb(ok_ws: &Path, no_corpus_ws: &Path) -> Vec<(&'static str, Envelope)> {
        let ok = ok_ws.display().to_string();
        let none = no_corpus_ws.display().to_string();
        vec![
            (
                "assure public ok",
                envelope_for(&[
                    "assure",
                    "public",
                    "--issuer",
                    "synthetic",
                    "--workspace",
                    &ok,
                ]),
            ),
            (
                "assure public refused",
                envelope_for(&[
                    "assure",
                    "public",
                    "--issuer",
                    "barrick",
                    "--workspace",
                    &none,
                ]),
            ),
            (
                "assure public --synthetic",
                envelope_for(&["assure", "public", "--synthetic"]),
            ),
            ("doctor", envelope_for(&["doctor"])),
            ("robot-docs", envelope_for(&["robot-docs"])),
            ("claim show", envelope_for(&["claim", "show", "clm_0001"])),
            ("claim why", envelope_for(&["claim", "why", "clm_0001"])),
            ("claim trace", envelope_for(&["claim", "trace", "clm_0001"])),
            ("usage error (parse_args)", envelope_for(&["frobnicate"])),
            (
                "usage error (execute_command)",
                execute_command(
                    &CliCommand::AssurePublic {
                        issuer: "newmont".to_string(),
                        through: None,
                    },
                    &workspace_root(),
                ),
            ),
        ]
    }

    /// `env` as JSON with the values that move with the ledger masked (an ok payload, the anchor id and the
    /// certificate) and the refusal's `what` text, which is refusal content rather than envelope frame (one names
    /// a scratch path; invalid claim IDs are checked in test_cli_claim_show_refuses_invalid_id). The
    /// refusal code, exit code and fix stay, so BD-328's code migration (bd-h0q1.3) shows up here.
    fn frame(env: &Envelope) -> String {
        let mut e = env.clone();
        if e.status == "ok" {
            e.payload_json = "\"<payload>\"".to_string();
        }
        e.anchor_id = e.anchor_id.map(|_| "<anchor>".to_string());
        e.certificate = e.certificate.map(|_| "<certificate>".to_string());
        if let Some(r) = e.refusal.as_mut() {
            r.what = "<what>".to_string();
        }
        e.to_json()
    }

    /// PO-ADIT-1: every verb's envelope, frame by frame. The expected frames are written by hand from the
    /// BD-329.S3 spec, not captured from this code's output.
    #[test]
    fn test_po_adit_1_golden_every_verb() {
        let ok_ws = synthetic_workspace("golden-ok");
        let none_ws = no_corpus_workspace("golden-none");
        let head = |verb: &str, status: &str, exit: u8, anchor: &str| {
            format!(
                "{{\"schema_version\":\"fmine.env.v1\",\"verb\":\"{verb}\",\"status\":\"{status}\",\
                 \"exit_code\":{exit},\"anchor_id\":{anchor},\"flags\":{FLAGS},"
            )
        };
        let action =
            |command: &str, why: &str| format!("{{\"command\":\"{command}\",\"why\":\"{why}\"}}");
        let synthetic_run = action(
            "fmine assure public --synthetic",
            "known-answer run on synthetic workbooks that exercises every ledger state",
        );
        let robot_docs = action("fmine robot-docs", "verbs, envelope fields and exit codes");
        let doctor = action(
            "fmine doctor",
            "which registered files are present, and whether every registry loads",
        );
        let no_claim = "\"no_claim\":\"Reproduces and checks the issuer's published AISC reconciliation tables \
                        as printed. It makes no reserve, resource, geotechnical, regulatory or investment claim \
                        and implies no issuer affiliation.\",";
        // The issuer name `synthetic` is the synthetic lane, so both ok runs here point to the barrick run;
        // the public-lane actions are checked on the real corpus (test_cli_assure_public_barrick_real_files).
        let barrick_run = action(
            "fmine assure public --issuer barrick",
            "the gate-eligible run over the workbooks in registries/sources/barrick.toml",
        );
        let fix_506 =
            "Put each named file at its registered path, or correct the registry entry named.";
        let fix_201 = "Query a domain that has claims, or produce the claim first (a public figure is emitted \
                       by its report).";
        let expected = [
            format!(
                "{}\"certificate\":\"<certificate>\",\"warnings\":[],\"next_actions\":[{barrick_run},\
                 {robot_docs}],{no_claim}\"payload\":\"<payload>\",\"refusal\":null}}",
                head("assure public", "ok", 0, "\"<anchor>\"")
            ),
            format!(
                "{}\"certificate\":null,\"warnings\":[],\"next_actions\":[{doctor}],\"payload\":null,\
                 \"refusal\":{{\"code\":\"FMINE-REF-506\",\"exit_code\":5,\"what\":\"<what>\",\"fix\":\"{fix_506}\"}}}}",
                head("assure public", "refused", 5, "null")
            ),
            format!(
                "{}\"certificate\":\"<certificate>\",\"warnings\":[],\"next_actions\":[{barrick_run},\
                 {robot_docs}],{no_claim}\"payload\":\"<payload>\",\"refusal\":null}}",
                head("assure public", "ok", 0, "\"<anchor>\"")
            ),
            format!(
                "{}\"certificate\":null,\"warnings\":[],\"next_actions\":[{synthetic_run}],\
                 \"payload\":\"<payload>\",\"refusal\":null}}",
                head("doctor", "ok", 0, "null")
            ),
            format!(
                "{}\"certificate\":null,\"warnings\":[],\"next_actions\":[{doctor}],\
                 \"payload\":\"<payload>\",\"refusal\":null}}",
                head("robot-docs", "ok", 0, "null")
            ),
            format!(
                "{}\"certificate\":null,\"warnings\":[],\"next_actions\":[{doctor}],\"payload\":null,\
                 \"refusal\":{{\"code\":\"FMINE-REF-201\",\"exit_code\":3,\"what\":\"<what>\",\
                 \"fix\":\"{fix_201}\"}}}}",
                head("claim show", "refused", 3, "null")
            ),
            format!(
                "{}\"certificate\":null,\"warnings\":[],\"next_actions\":[{doctor}],\"payload\":null,\
                 \"refusal\":{{\"code\":\"FMINE-REF-201\",\"exit_code\":3,\"what\":\"<what>\",\
                 \"fix\":\"{fix_201}\"}}}}",
                head("claim why", "refused", 3, "null")
            ),
            format!(
                "{}\"certificate\":null,\"warnings\":[],\"next_actions\":[{doctor}],\"payload\":null,\
                 \"refusal\":{{\"code\":\"FMINE-REF-201\",\"exit_code\":3,\"what\":\"<what>\",\
                 \"fix\":\"{fix_201}\"}}}}",
                head("claim trace", "refused", 3, "null")
            ),
            format!(
                "{}\"certificate\":null,\"warnings\":[],\"next_actions\":[{robot_docs}],\
                 \"payload\":{{\"error\":\"unknown verb 'frobnicate'\"}},\"refusal\":null}}",
                head("unknown", "usage_error", 2, "null")
            ),
            format!(
                "{}\"certificate\":null,\"warnings\":[],\"next_actions\":[{robot_docs}],\
                 \"payload\":{{\"error\":\"unknown issuer 'newmont' (registered: barrick)\"}},\"refusal\":null}}",
                head("assure public", "usage_error", 2, "null")
            ),
        ];
        let cases = every_verb(&ok_ws, &none_ws);
        assert_eq!(cases.len(), expected.len());
        for ((case, env), want) in cases.iter().zip(&expected) {
            assert_eq!(&frame(env), want, "{case}");
            // The unmasked envelope: the four flags, all false, exactly once, and no flag named anywhere else.
            let json = env.to_json();
            assert_eq!(json.matches(FLAGS).count(), 1, "{case}: {json}");
            for name in FLAG_NAMES {
                assert_eq!(json.matches(name).count(), 1, "{case}: {name}");
            }
        }
        let _ = std::fs::remove_dir_all(&ok_ws);
        let _ = std::fs::remove_dir_all(&none_ws);
    }

    /// Whether `command` parses as an `fmine` invocation, or names a repository path that exists.
    fn resolves(command: &str, root: &Path) -> bool {
        match command.strip_prefix("fmine ") {
            Some(rest) => {
                let argv: Vec<String> = rest.split_whitespace().map(str::to_string).collect();
                parse_args(&argv, root).is_ok()
            }
            None => root.join(command).exists(),
        }
    }

    #[test]
    fn test_next_actions_resolvable() {
        let root = workspace_root();
        assert!(!resolves("fmine frobnicate", &root));
        assert!(!resolves("fmine doctor --deep", &root));
        assert!(!resolves("fmine assure public --issuer newmont", &root));
        assert!(!resolves("repro/no-such-issuer/fetch.sh", &root));
        let ok_ws = synthetic_workspace("actions-ok");
        let none_ws = no_corpus_workspace("actions-none");
        let mut checked = 0;
        for (case, env) in every_verb(&ok_ws, &none_ws) {
            assert!(!env.next_actions.is_empty(), "{case}");
            for a in &env.next_actions {
                assert!(resolves(&a.command, &root), "{case}: {}", a.command);
                checked += 1;
            }
        }
        assert_eq!(checked, 12);
        let _ = std::fs::remove_dir_all(&ok_ws);
        let _ = std::fs::remove_dir_all(&none_ws);
    }

    #[test]
    fn test_refused_506_points_to_doctor() {
        let ws = no_corpus_workspace("refused-506");
        let env = envelope_for(&[
            "assure",
            "public",
            "--issuer",
            "barrick",
            "--workspace",
            &ws.display().to_string(),
        ]);
        assert_eq!(
            env.refusal.as_ref().map(|r| (r.code, r.exit_code)),
            Some(("FMINE-REF-506", 5))
        );
        let commands: Vec<&str> = env
            .next_actions
            .iter()
            .map(|a| a.command.as_str())
            .collect();
        assert_eq!(commands, ["fmine doctor"]);
        assert_eq!((env.certificate.as_deref(), env.no_claim), (None, None));
        let _ = std::fs::remove_dir_all(&ws);
    }

    #[test]
    fn test_certificate_is_state_root_for_reports() {
        let report = fmine_public::run_synthetic_assurance(&[]).expect("synthetic run");
        let env = envelope_for(&["assure", "public", "--synthetic"]);
        assert_eq!(
            env.certificate,
            Some(report.ledger_anchor.state_root.to_hex())
        );
        assert_eq!(
            env.anchor_id,
            Some(report.ledger_anchor.anchor_id().to_string())
        );
        assert_eq!(env.no_claim, Some(fmine_public::NO_CLAIM));

        let ws = synthetic_workspace("certificate");
        let profile = load_issuer_profile(&ws, "synthetic").expect("generated registry");
        let report = run_public_assurance(&profile, &ws).expect("public-lane run");
        let env = envelope_for(&[
            "assure",
            "public",
            "--issuer",
            "synthetic",
            "--workspace",
            &ws.display().to_string(),
        ]);
        assert_eq!(
            env.certificate,
            Some(report.ledger_anchor.state_root.to_hex())
        );
        assert_ne!(
            env.certificate, env.anchor_id,
            "the certificate is the root, not the anchor id"
        );

        for env in [
            envelope_for(&["doctor"]),
            envelope_for(&["robot-docs"]),
            envelope_for(&["claim", "show", "clm_0001"]),
            envelope_for(&["frobnicate"]),
        ] {
            assert_eq!(
                (env.certificate.as_deref(), env.no_claim),
                (None, None),
                "{}",
                env.verb
            );
            assert!(
                env.to_json().contains(",\"certificate\":null,"),
                "{}",
                env.verb
            );
        }
        let _ = std::fs::remove_dir_all(&ws);
    }

    #[test]
    fn test_assure_payload_has_no_envelope_fields() {
        let ws = synthetic_workspace("payload");
        for env in [
            envelope_for(&["assure", "public", "--synthetic"]),
            envelope_for(&[
                "assure",
                "public",
                "--issuer",
                "synthetic",
                "--workspace",
                &ws.display().to_string(),
            ]),
        ] {
            assert_eq!(env.status, "ok", "{}", env.to_json());
            for field in ["\"flags\"", "\"no_claim\"", "\"next_actions\""] {
                assert!(!env.payload_json.contains(field), "{field} in the payload");
                assert_eq!(
                    env.to_json().matches(field).count(),
                    1,
                    "{field} once in the envelope"
                );
            }
        }
        let _ = std::fs::remove_dir_all(&ws);
    }

    #[test]
    fn selections_are_explicit_and_invalid_combinations_are_usage_errors() {
        let query = args(&[
            "track-record",
            "--issuer",
            "barrick",
            "--site",
            "north-mara",
            "--through",
            "2026Q2",
            "--price-subject",
            "barrick",
        ]);
        let (command, _, _) = parse_args(&query, &root()).expect("public selectors");
        assert_eq!(
            command,
            CliCommand::TrackRecord {
                issuer: Some("barrick".into()),
                site: Some("north-mara".into()),
                through: Some("2026Q2".into()),
                price_subject: Some("barrick".into()),
            }
        );
        for query in [
            vec!["track-record"],
            vec!["track-record", "extra", "--synthetic"],
            vec!["track-record", "--issuer", "unknown-issuer"],
            vec!["track-record", "--issuer", "../barrick"],
            vec!["track-record", "--issuer", "barrick", "--synthetic"],
            vec!["track-record", "--synthetic", "--site", "north-mara"],
            vec!["track-record", "--synthetic", "--through", "2026Q2"],
            vec!["track-record", "--synthetic", "--price-subject", "barrick"],
            vec!["track-record", "--issuer", "barrick", "--price-subject"],
            vec![
                "track-record",
                "--issuer",
                "barrick",
                "--price-subject",
                " ",
            ],
            vec!["track-record", "--issuer", "barrick", "--site", " "],
            vec!["track-record", "--issuer", "barrick", "--through", "2026Q5"],
            vec!["track-record", "--synthetic", "--as-of", "2026-02-30"],
            vec!["track-record", "--synthetic", "--selector", "latest-as-of"],
            vec!["track-record", "--synthetic", "--rank"],
        ] {
            let (envelope, _) = invoke(&args(&query), &root());
            assert_eq!(
                (envelope.verb.as_str(), envelope.status, envelope.exit_code),
                ("track-record", "usage_error", 2),
                "{query:?}"
            );
        }
    }

    #[test]
    fn command_is_registered_and_documented() {
        let registry =
            std::fs::read_to_string(root().join("registries/verbs.toml")).expect("verb registry");
        assert!(registry.contains("path = \"track-record\""));
        let docs = execute_command(&CliCommand::RobotDocs, &root());
        assert!(
            docs.payload_json
                .contains("track-record --issuer <issuer> [--site <subject>] [--through YYYYQn]")
        );
        assert!(docs.payload_json.contains("track-record --synthetic"));
        assert!(docs.payload_json.contains("retrospective_descriptive"));
        assert!(docs.payload_json.contains("[--price-subject <subject>]"));
        assert!(docs.payload_json.contains("current-period compatibility"));
        assert_eq!(
            invoke(&args(&["doctor", "--price-subject", "barrick"]), &root())
                .0
                .exit_code,
            2
        );
    }

    #[test]
    fn explicit_capacity_selection_is_scoped_and_documented() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(std::path::Path::parent)
            .expect("workspace");
        let args = |words: &[&str]| {
            words
                .iter()
                .map(|word| (*word).to_string())
                .collect::<Vec<_>>()
        };
        let base = [
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
        ];
        let mut selected = args(&base);
        selected.extend(args(&["--capacity-document", "Mine-technical-report.pdf"]));
        let (command, _, _) = fmine_cli::parse_args(&selected, root).expect("capacity selection");
        assert!(
            matches!(command, fmine_cli::CliCommand::GuidanceCompletion {
            capacity_document: Some(ref document), ..
        } if document == "Mine-technical-report.pdf")
        );
        let (command, _, _) = fmine_cli::parse_args(&args(&base), root).expect("ordinary guidance");
        assert!(matches!(
            command,
            fmine_cli::CliCommand::GuidanceCompletion {
                capacity_document: None,
                ..
            }
        ));
        for invalid in [
            args(&["doctor", "--capacity-document", "Report.pdf"]),
            args(&[
                "track-record",
                "--synthetic",
                "--capacity-document",
                "Report.pdf",
            ]),
            args(&[
                "guidance",
                "completion",
                "--synthetic",
                "--capacity-document",
                "Report.pdf",
            ]),
            {
                let mut invalid = args(&base);
                invalid.push("--capacity-document".into());
                invalid
            },
            {
                let mut invalid = args(&base);
                invalid.extend(args(&["--capacity-document", " "]));
                invalid
            },
        ] {
            let (envelope, _) = fmine_cli::invoke(&invalid, root);
            assert_eq!(envelope.exit_code, 2, "{invalid:?}");
            assert_eq!(envelope.status, "usage_error");
        }
        let docs = fmine_cli::execute_command(&fmine_cli::CliCommand::RobotDocs, root);
        assert!(
            docs.payload_json
                .contains("--capacity-document <registered file>")
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
            let (env, _) = fmine_cli::invoke(&args(&words), &root());
            assert_eq!((env.verb.as_str(), env.exit_code), ("lint", 2), "{words:?}");
        }
        let (cmd, _, _) = fmine_cli::parse_args(
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
            fmine_cli::CliCommand::Lint {
                issuer: Some(_),
                through: Some(_),
                ..
            }
        ));
    }

    #[test]
    fn test_flags_constant_in_every_constructor() {
        for env in [
            Envelope::ok("doctor", "{}".to_string(), Vec::new()),
            Envelope::report(
                "assure public",
                "anc_0".to_string(),
                "00".to_string(),
                "{}".to_string(),
                Vec::new(),
            ),
            Envelope::usage_error("unknown", "bad"),
            Envelope::refused(
                "claim show",
                Refusal::new(RefusalCode::NoClaimInDomain201, "absent"),
                Vec::new(),
            ),
        ] {
            let json = env.to_json();
            assert_eq!(json.matches(FLAGS).count(), 1, "{json}");
            assert_eq!(json.matches("\"flags\":").count(), 1, "{json}");
        }
    }

    /// Lines of `text` that write one of the four flags true, or name a setter for one.
    fn true_flag_writes(text: &str) -> Vec<String> {
        text.lines()
            .filter(|line| {
                FLAG_NAMES.iter().any(|f| {
                    [
                        format!("{f}\\\":true"),
                        format!("{f}\":true"),
                        format!("{f}\": true"),
                        format!("{f}: true"),
                        format!("{f} = true"),
                        format!("set_{f}"),
                    ]
                    .iter()
                    .any(|p| line.contains(p.as_str()))
                })
            })
            .map(str::to_string)
            .collect()
    }

    #[test]
    fn test_no_source_writes_a_true_flag() {
        // The scan finds planted writes (built at run time, so this file never holds one) before it is
        // trusted with the tree.
        for f in FLAG_NAMES {
            for planted in [
                format!("let s = \"{{\\\"{f}\\\":true}}\";"),
                format!("let j = r#\"{{\"{f}\": true}}\"#;"),
                format!("report.set_{f}(true);"),
                format!("Flags {{ {f}: true }}"),
                format!("self.{f} = true;"),
            ] {
                assert_eq!(true_flag_writes(&planted).len(), 1, "{planted}");
            }
        }
        let mut files = Vec::new();
        let mut dirs = vec![workspace_root().join("crates")];
        while let Some(dir) = dirs.pop() {
            for entry in std::fs::read_dir(&dir).expect("read dir") {
                let path = entry.expect("dir entry").path();
                if path.is_dir() {
                    dirs.push(path);
                } else if path.extension().is_some_and(|e| e == "rs")
                    && path.components().any(|c| c.as_os_str() == "src")
                {
                    files.push(path);
                }
            }
        }
        assert!(files.len() > 16, "scanned only {} files", files.len());
        let hits: Vec<String> = files
            .iter()
            .flat_map(|p| {
                let text = std::fs::read_to_string(p).expect("read source");
                true_flag_writes(&text)
                    .into_iter()
                    .map(move |line| format!("{}: {line}", p.display()))
            })
            .collect();
        assert!(hits.is_empty(), "{hits:#?}");
    }

    mod issued_parser_contracts {
        use fmine_cli::issued::parse;
        use std::path::Path;

        fn args(text: &str) -> Vec<String> {
            text.split_whitespace().map(str::to_string).collect()
        }

        #[test]
        fn issuance_parser_requires_one_lane_authority_and_explicit_bounded_time() {
            let good = format!(
                "assure issue --synthetic --output report --journal journal --key key --expected-head {} --role sponsor --purpose review --now 2026-11-01T00:00:00Z --uncertainty-ns 0",
                "1".repeat(64)
            );
            assert!(parse(&args(&good), Path::new(".")).is_ok());
            for bad in [
                good.replace("--synthetic", "--synthetic --issuer barrick"),
                good.replace("--synthetic", "--synthetic --through 2026Q2"),
                good.replace("--role sponsor", "--role recipient@example.invalid"),
                good.replace("--uncertainty-ns 0", "--uncertainty-ns +1"),
                good.replace("--now 2026-11-01T00:00:00Z", ""),
                format!("{good} --output again"),
                format!("{good} extra"),
            ] {
                assert!(parse(&args(&bad), Path::new(".")).is_err(), "{bad}");
            }
        }

        #[test]
        fn export_has_no_historical_or_untrusted_reuse_mode() {
            let good = format!(
                "reliance export report --output copy --journal journal --head {} --now 2026-11-01T00:00:00Z --uncertainty-ns 0",
                "1".repeat(64)
            );
            assert!(parse(&args(&good), Path::new(".")).is_ok());
            for bad in [
                format!("{good} --as-of 2026-10-31T00:00:00Z"),
                format!("{good} --synthetic"),
                good.replace("--head", "--expected-head"),
            ] {
                assert!(parse(&args(&bad), Path::new(".")).is_err());
            }
        }
    }
}
