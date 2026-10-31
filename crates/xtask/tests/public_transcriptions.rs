//! Cross-crate document verification, two-reader transcription and convention-discovery checks.

#[cfg(test)]
mod tests {
    use std::fmt::Write as _;
    use std::path::Path;

    use fmine_core::hash::sha256;
    use fmine_evidence::Refusal;
    use fmine_numerics::exact_feas::Convention;
    use fmine_numerics::exact_feas::discover::TieFinding;
    use fmine_public::sources::{load_issuer_profile, registry_path, transcriptions_path};
    use fmine_public::transcribe::{
        load_transcription_report, load_transcription_report_as_of, transcription_json,
    };
    use fmine_public::{PublicAssuranceReport, run_public_assurance, synthetic};

    const DECK: &[u8] = b"%PDF-1.7 synthetic deck, never parsed";
    const MDA: &[u8] = b"%PDF-1.7 synthetic MD&A, never parsed";
    const FUTURE: &[u8] = b"%PDF-1.7 synthetic future publication, never parsed";

    /// A synthetic workspace: the synthetic sources registry with two registered documents, their bytes, and a
    /// transcription registry of `rows`.
    fn workspace(tag: &str, rows: &[String]) -> std::path::PathBuf {
        let ws =
            std::env::temp_dir().join(format!("fmine-transcribe-{tag}-{}", std::process::id()));
        let reg = synthetic::write_issuer_workspace(&ws).expect("write workspace");
        let mut toml = std::fs::read_to_string(&reg).expect("registry");
        std::fs::create_dir_all(ws.join("docs")).expect("docs dir");
        for (file, bytes, pages) in [("Deck.pdf", DECK, 15), ("Mda.pdf", MDA, 80)] {
            std::fs::write(ws.join("docs").join(file), bytes).expect("doc bytes");
            let _ = write!(
                toml,
                "\n[[document]]\nfile = \"{file}\"\ndir = \"docs\"\nkind = \"mda\"\n\
             url = \"https://example.invalid/{file}\"\nurl_basis = \"W1 table\"\npublished_at = \"2030-02-01\"\n\
             published_at_basis = \"FACT: synthetic\"\npages = {pages}\nbytes = {}\nsha256 = \"{}\"\n",
                bytes.len(),
                sha256(bytes).to_hex()
            );
        }
        std::fs::write(&reg, toml).expect("registry");
        let path = transcriptions_path(&ws, "synthetic");
        std::fs::create_dir_all(path.parent().expect("parent")).expect("dir");
        std::fs::write(
            &path,
            format!(
                "schema = \"fmine.transcriptions.v1\"\nissuer = \"synthetic\"\n{}",
                rows.concat()
            ),
        )
        .expect("transcriptions");
        ws
    }

    fn entry(reader: &str, doc: &str, page: u64, row: &str, token: &str, quantity: &str) -> String {
        let bytes = match doc {
            "Deck.pdf" => DECK,
            "Future.pdf" => FUTURE,
            _ => MDA,
        };
        let sha = sha256(bytes).to_hex();
        format!(
            "\n[[entry]]\ndoc = \"{doc}\"\ndoc_sha256 = \"{sha}\"\npage = {page}\ntable = \"Reserves roll-forward\"\n\
         row = \"{row}\"\ncol = \"2030\"\ntoken = \"{token}\"\nquantity = \"{quantity}\"\nunit = \"Moz\"\n\
         basis = \"attributable\"\nperiod = \"2030\"\nreader = \"{reader}\"\n"
        )
    }

    /// E698's shape, signed as a roll-forward prints it: the deck and the MD&A print depletion and net change.
    fn rows() -> Vec<String> {
        let mut rows = Vec::new();
        for reader in ["reader_a", "reader_b"] {
            rows.push(entry(
                reader,
                "Deck.pdf",
                14,
                "Depletion",
                "(3.6)",
                "depletion",
            ));
            rows.push(entry(
                reader,
                "Deck.pdf",
                14,
                "Net change",
                "1.7",
                "net_change",
            ));
            rows.push(entry(
                reader,
                "Mda.pdf",
                71,
                "Depletion",
                "(3.7)",
                "depletion",
            ));
            rows.push(entry(
                reader,
                "Mda.pdf",
                71,
                "Net change",
                "1.8",
                "net_change",
            ));
        }
        rows
    }

    fn run(ws: &Path) -> Result<PublicAssuranceReport, Refusal> {
        run_public_assurance(&load_issuer_profile(ws, "synthetic").expect("registry"), ws)
    }

    #[test]
    fn test_transcribed_tie_discovers_the_deck_convention() {
        let ws = workspace("tie", &rows());
        let r = run(&ws).expect("run");
        let t = r.transcriptions.as_ref().expect("registry present");
        assert_eq!((t.entries, t.agreement.accepted.len()), (8, 4));
        assert_eq!(
            t.documents
                .iter()
                .map(|d| d.file.as_str())
                .collect::<Vec<_>>(),
            ["Deck.pdf", "Mda.pdf"]
        );
        let Some(Ok(outcome)) = &t.tie else {
            panic!("tie: {:?}", t.tie)
        };
        assert!(outcome.conflicts.is_empty());
        let d = outcome.discovery.as_ref().expect("discovery");
        assert_eq!(
            d.findings,
            [
                (
                    "Deck.pdf".to_string(),
                    TieFinding::Identified(Convention::Truncate)
                ),
                (
                    "Mda.pdf".to_string(),
                    TieFinding::Ambiguous(vec![
                        Convention::HalfUp,
                        Convention::HalfDown,
                        Convention::HalfEven
                    ])
                ),
            ]
        );
        // The section names conventions, counts and digests, never a transcribed value.
        let json = transcription_json(Some(t));
        assert!(
        json.contains(
            "\"tie\":{\"conflicts\":[],\"discovery\":{\"documents\":[\"Deck.pdf\",\"Mda.pdf\"],\
             \"quantities\":[\"depletion\",\"net_change\"],\"assignments\":16,\"fits\":{\"proper\":3,\
             \"points\":0,\"empty\":13},\"closed_fit\":\"points\",\"findings\":[{\"document\":\"Deck.pdf\",\
             \"finding\":\"identified\",\"conventions\":[\"truncate\"],\"reason_code\":null},\
             {\"document\":\"Mda.pdf\",\"finding\":\"ambiguous\",\"conventions\":[\"half_up\",\"half_down\",\
             \"half_even\"],\"reason_code\":\"FMINE-REF-104\"}]}}"
        ),
        "{json}"
    );
        for value in ["3.6", "3.7", "1.7", "1.8"] {
            assert!(!json.contains(value), "{value} in {json}");
        }
        assert!(
            r.to_json().contains(&json),
            "the payload carries the section"
        );
        // The section is outside the state root: the same run without the registry has the same root.
        std::fs::remove_file(transcriptions_path(&ws, "synthetic")).expect("remove registry");
        let bare = run(&ws).expect("run");
        assert!(bare.transcriptions.is_none());
        assert!(bare.to_json().contains("\"transcriptions\":null,"));
        assert_eq!(bare.ledger_anchor.state_root, r.ledger_anchor.state_root);
        let _ = std::fs::remove_dir_all(&ws);
    }

    #[test]
    fn test_irreconcilable_quantity_is_a_conflict_and_left_out() {
        // E692's shape: the later document prints the prior year's net change as 79, the first as 6.7.
        let mut rows = rows();
        for reader in ["reader_a", "reader_b"] {
            rows.push(entry(
                reader,
                "Deck.pdf",
                13,
                "Prior net change",
                "6.7",
                "net_change_prior",
            ));
            rows.push(entry(
                reader,
                "Mda.pdf",
                70,
                "Prior net change",
                "79",
                "net_change_prior",
            ));
        }
        let ws = workspace("conflict", &rows);
        let r = run(&ws).expect("run");
        let t = r.transcriptions.as_ref().expect("registry present");
        let Some(Ok(outcome)) = &t.tie else {
            panic!("tie: {:?}", t.tie)
        };
        let at: Vec<String> = outcome.conflicts[0]
            .1
            .iter()
            .map(ToString::to_string)
            .collect();
        assert_eq!(outcome.conflicts[0].0, "net_change_prior");
        assert_eq!(
            at,
            [
                "Deck.pdf:p13:Reserves roll-forward:Prior net change/2030",
                "Mda.pdf:p70:Reserves roll-forward:Prior net change/2030"
            ]
        );
        let d = outcome
            .discovery
            .as_ref()
            .expect("re-run without the conflict");
        assert_eq!(
            d.findings[0].1,
            TieFinding::Identified(Convention::Truncate)
        );
        let _ = std::fs::remove_dir_all(&ws);
    }

    #[test]
    fn test_missing_or_altered_document_and_disagreement_refuse() {
        let ws = workspace("refuse", &rows());
        // A pending locator (one reader) is reported, not refused.
        let mut pending = rows();
        pending.push(entry(
            "reader_a",
            "Mda.pdf",
            72,
            "Acquisitions",
            "2.2",
            "acquisitions",
        ));
        let path = transcriptions_path(&ws, "synthetic");
        let header = "schema = \"fmine.transcriptions.v1\"\nissuer = \"synthetic\"\n";
        std::fs::write(&path, format!("{header}{}", pending.concat())).expect("write");
        let t = run(&ws).expect("run").transcriptions.expect("present");
        assert_eq!(t.agreement.single_reader.len(), 1);
        assert!(transcription_json(Some(&t)).contains(
            "\"single_reader\":[\"Mda.pdf:p72:Reserves roll-forward:Acquisitions/2030\"]"
        ));
        // Planted negative: reader_b's depletion in the deck differs. The run refuses, naming the locator and
        // the field, never the values.
        let mut conflict = rows();
        conflict[4] = entry(
            "reader_b",
            "Deck.pdf",
            14,
            "Depletion",
            "(3.5)",
            "depletion",
        );
        std::fs::write(&path, format!("{header}{}", conflict.concat())).expect("write");
        let e = run(&ws).expect_err("disagreement refuses");
        assert_eq!((e.code, e.exit_code), ("FMINE-REF-202", 3));
        assert!(
            e.what.contains(
                "at Deck.pdf:p14:Reserves roll-forward:Depletion/2030 (reader_a, reader_b: token)"
            ),
            "{}",
            e.what
        );
        assert!(
            !e.what.contains("3.5") && !e.what.contains("3.6"),
            "{}",
            e.what
        );
        std::fs::write(&path, format!("{header}{}", rows().concat())).expect("write");
        // One altered byte in a cited document.
        let mut altered = DECK.to_vec();
        altered[0] ^= 1;
        std::fs::write(ws.join("docs").join("Deck.pdf"), &altered).expect("alter");
        let e = run(&ws).expect_err("altered refuses");
        assert_eq!((e.code, e.exit_code), ("FMINE-REF-505", 8));
        assert!(e.what.contains("digest mismatch: Deck.pdf"), "{}", e.what);
        // A missing cited document.
        std::fs::remove_file(ws.join("docs").join("Deck.pdf")).expect("remove");
        let e = run(&ws).expect_err("missing refuses");
        assert_eq!((e.code, e.exit_code), ("FMINE-REF-506", 5));
        assert!(e.what.contains("missing: Deck.pdf"), "{}", e.what);
        // A `--through` run reads no transcriptions, so the missing document does not refuse it.
        let profile = load_issuer_profile(&ws, "synthetic")
            .expect("registry")
            .through("2030Q2")
            .expect("bound");
        assert!(
            run_public_assurance(&profile, &ws)
                .expect("run")
                .transcriptions
                .is_none()
        );
        let _ = std::fs::remove_dir_all(&ws);
    }

    #[test]
    fn future_document_failures_do_not_change_an_earlier_guidance_cut() {
        let ws = workspace("guidance-as-of", &rows());
        // Exercise the public entry point with explicitly invented bytes under a test registry name.
        // No guidance rows exist: the later source-selection refusal must remain exactly the same.
        let issuer = "guidance_cut_test";
        let source_path = registry_path(&ws, issuer);
        let mut source = std::fs::read_to_string(registry_path(&ws, "synthetic"))
            .unwrap()
            .replace("issuer = \"synthetic\"", &format!("issuer = \"{issuer}\""))
            .replace(
                "retrieved_utc = \"synthetic\"",
                "retrieved_utc = \"2030-07-31T00:00:00Z\"",
            );
        std::fs::write(&source_path, &source).unwrap();
        let transcript_path = transcriptions_path(&ws, issuer);
        let baseline_rows = std::fs::read_to_string(transcriptions_path(&ws, "synthetic"))
            .unwrap()
            .replace("issuer = \"synthetic\"", &format!("issuer = \"{issuer}\""));
        std::fs::write(&transcript_path, &baseline_rows).unwrap();
        let profile = load_issuer_profile(&ws, issuer).unwrap();
        let baseline = load_transcription_report_as_of(&profile, &ws, "2030-07-31")
            .unwrap()
            .unwrap();
        let baseline_json = transcription_json(Some(&baseline));
        let baseline_refusal =
            fmine_public::guidance::run_public(&ws, issuer, "site-a", "2030Q2", "2030-07-31")
                .unwrap_err();
        assert_eq!(baseline_refusal.code, "FMINE-REF-201");

        write!(
            source,
            "\n[[document]]\nfile = \"Future.pdf\"\ndir = \"docs\"\nkind = \"mda\"\n\
             url = \"https://example.invalid/Future.pdf\"\nurl_basis = \"W1 table\"\n\
             published_at = \"2030-08-01\"\npublished_at_basis = \"FACT: synthetic\"\n\
             pages = 80\nbytes = {}\nsha256 = \"{}\"\n",
            FUTURE.len(),
            sha256(FUTURE).to_hex(),
        )
        .unwrap();
        std::fs::write(&source_path, &source).unwrap();
        let future_rows = format!(
            "{baseline_rows}{}{}",
            entry(
                "reader_a",
                "Future.pdf",
                72,
                "Additions",
                "1.2",
                "additions"
            ),
            entry(
                "reader_b",
                "Future.pdf",
                72,
                "Additions",
                "1.3",
                "additions"
            ),
        );
        std::fs::write(&transcript_path, future_rows).unwrap();
        let profile = load_issuer_profile(&ws, issuer).unwrap();
        let assert_earlier_unchanged = || {
            let earlier = load_transcription_report_as_of(&profile, &ws, "2030-07-31")
                .unwrap()
                .unwrap();
            assert_eq!(transcription_json(Some(&earlier)), baseline_json);
            let refusal =
                fmine_public::guidance::run_public(&ws, issuer, "site-a", "2030Q2", "2030-07-31")
                    .unwrap_err();
            assert_eq!(refusal.code, baseline_refusal.code);
            assert_eq!(refusal.what, baseline_refusal.what);
        };
        let assert_included_refuses = |expected| {
            assert_eq!(
                load_transcription_report_as_of(&profile, &ws, "2030-08-01")
                    .unwrap_err()
                    .code,
                expected,
            );
            assert_eq!(
                fmine_public::guidance::run_public(&ws, issuer, "site-a", "2030Q2", "2030-08-01",)
                    .unwrap_err()
                    .code,
                expected,
            );
            assert_eq!(
                load_transcription_report(&profile, &ws).unwrap_err().code,
                expected,
                "ordinary assurance still checks all registered transcriptions",
            );
        };

        assert_earlier_unchanged();
        assert_included_refuses("FMINE-REF-506");
        let mut altered = FUTURE.to_vec();
        altered[0] ^= 1;
        std::fs::write(ws.join("docs/Future.pdf"), altered).unwrap();
        assert_earlier_unchanged();
        assert_included_refuses("FMINE-REF-505");
        std::fs::write(ws.join("docs/Future.pdf"), FUTURE).unwrap();
        assert_earlier_unchanged();
        assert_included_refuses("FMINE-REF-202");

        let agreed = format!(
            "{baseline_rows}{}{}",
            entry(
                "reader_a",
                "Future.pdf",
                72,
                "Additions",
                "1.2",
                "additions"
            ),
            entry(
                "reader_b",
                "Future.pdf",
                72,
                "Additions",
                "1.2",
                "additions"
            ),
        );
        std::fs::write(&transcript_path, agreed).unwrap();
        assert_earlier_unchanged();
        let included = load_transcription_report_as_of(&profile, &ws, "2030-08-01")
            .unwrap()
            .unwrap();
        assert_eq!(included.entries, baseline.entries + 2);
        assert_eq!(
            included.agreement.accepted.len(),
            baseline.agreement.accepted.len() + 1,
        );
        assert!(included.documents.iter().any(|d| d.file == "Future.pdf"));
    }
}
