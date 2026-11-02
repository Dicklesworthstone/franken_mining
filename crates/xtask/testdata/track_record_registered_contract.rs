// Registered workbook and document integration contracts; included in their original CLI test scope.
    fn registered_workspace() -> std::path::PathBuf {
        use fmine_io::xlsx::{MinimalCell, MinimalSheet, build_minimal_xlsx};
        use std::sync::atomic::{AtomicU64, Ordering};
        static SERIAL: AtomicU64 = AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "fmine-track-record-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        for dir in [
            "corpus",
            "docs",
            "registries/sources",
            "registries/transcriptions",
        ] {
            std::fs::create_dir_all(root.join(dir)).expect("fixture dir");
        }
        let t = |at: &str, text: &str| (at.into(), MinimalCell::Text(text.into()));
        let n = |at: &str, value: f64| (at.into(), MinimalCell::Number(value));
        let workbook = build_minimal_xlsx(&[MinimalSheet {
            name: "YTD".into(),
            hidden: false,
            cells: vec![
                t("A1", "For the twelve months ended December 31"),
                n("B1", 2030.0),
                t("B2", "Gold produced (000s oz) 100% basis"),
                t("C2", "Gold produced (000s oz) attributable basis"),
                t("D2", "All-in sustaining costs ($/oz)"),
                t("A3", "Example Mine (80%)"),
                n("B3", 156.25),
                n("C3", 125.0),
                n("D3", 1150.0),
            ],
        }]);
        let pdf = b"%PDF-1.7 invented guidance, never a real issuer filing";
        std::fs::write(root.join("corpus/Annual.xlsx"), &workbook).expect("workbook");
        std::fs::write(root.join("docs/Guidance.pdf"), pdf).expect("document");
        let registry = format!(
            "schema = \"fmine.sources.v1\"\nissuer = \"synthetic\"\ncorpus_dir = \"corpus\"\nretrieved_utc = \"synthetic\"\nurl_prefix = \"https://example.invalid/\"\n\n[[source]]\nfile = \"Annual.xlsx\"\nkind = \"mine_stats\"\nquarter = \"2030Q4\"\npath = \"Annual.xlsx\"\nurl_basis = \"W1 table\"\nbytes = {}\nsha256 = \"{}\"\n\n[[subject]]\nname = \"Example Mine\"\n\n[[document]]\nfile = \"Guidance.pdf\"\ndir = \"docs\"\nkind = \"aif\"\nurl = \"https://example.invalid/Guidance.pdf\"\nurl_basis = \"W1 table\"\npublished_at = \"2030-01-15\"\npublished_at_basis = \"FACT: invented fixture declares 2030-01-15\"\npages = 1\nbytes = {}\nsha256 = \"{}\"\n",
            workbook.len(),
            sha256(&workbook).to_hex(),
            pdf.len(),
            sha256(pdf).to_hex()
        );
        std::fs::write(root.join("registries/sources/synthetic.toml"), registry)
            .expect("source registry");
        let mut transcript =
            "schema = \"fmine.transcriptions.v1\"\nissuer = \"synthetic\"\n".to_string();
        let doc_sha = sha256(pdf).to_hex();
        for (metric, token, unit, basis) in [
            ("production_koz", "100 - 120", "koz", "attributable:4/5"),
            (
                "aisc_usd_per_oz",
                "1000 - 1200",
                "usd_per_oz",
                "attributable:4/5;aisc=issuer-reported-per-sold-oz",
            ),
        ] {
            for reader in ["reader_a", "reader_b"] {
                writeln!(
                    transcript,
                    r#"
[[entry]]
doc = "Guidance.pdf"
doc_sha256 = "{doc_sha}"
page = 1
table = "Annual guidance"
row = "Example Mine"
col = "{metric}"
token = "{token}"
quantity = "guidance_example_mine_{metric}"
unit = "{unit}"
basis = "{basis}"
period = "2030FY"
reader = "{reader}"
"#
                )
                .expect("transcription");
            }
        }
        std::fs::write(
            root.join("registries/transcriptions/synthetic.toml"),
            transcript,
        )
        .expect("transcriptions");
        root
    }

    #[test]
    fn registered_input_path_checks_real_parsing_hashes_and_two_reader_agreement() {
        let root = registered_workspace();
        let r = run(&root, "synthetic", Some("example-mine"), Some("2030Q4"))
            .expect("registered inputs");
        assert_eq!(
            production(&r).score.expect("source score").z,
            Some(Rat::new(3, 2).expect("z"))
        );
        let OutcomeBinding::Resolved { cell, .. } = &production(&r).outcome else {
            panic!("resolved source")
        };
        assert_eq!(cell.locator, "Annual.xlsx:YTD!C3");
        assert_eq!(
            r.to_json(),
            run(&root, "synthetic", None, Some("2030Q4"))
                .expect("rerun")
                .to_json()
        );
        assert_ne!(
            r.digest,
            run(&root, "synthetic", None, None)
                .expect("unbounded")
                .digest
        );
    }

    #[test]
    fn registered_cell_mutations_cannot_reuse_its_canonical_identity() {
        let root = registered_workspace();
        let p = load_issuer_profile(&root, "synthetic").expect("profile");
        let t = fmine_public::transcribe::load_transcription_report(&p, &root)
            .expect("checked transcripts")
            .expect("agreement");
        let source = fmine_public::key::mine_stats::load_claims(&p, &root)
            .expect("checked workbook")
            .claims;
        let i = source
            .iter()
            .position(|c| c.locator == "Annual.xlsx:YTD!C3")
            .expect("attributable production");
        let before = assess(&p, &t, &source, None).expect("canonical source");
        assert_eq!(
            production(&before).score.expect("score").z,
            Some(Rat::new(3, 2).unwrap())
        );
        source[i].claim.check_id().expect("source identity");
        assert_eq!(
            source[i].claim.value_tag(),
            canonical_value_tag(&source[i].key, &source[i].claim.value)
        );

        for mutation in 0..5 {
            let mut changed = source.clone();
            let cell = &mut changed[i];
            match mutation {
                0 => {
                    let PublicValue::Number { displayed, .. } = &mut cell.claim.value else {
                        panic!("numeric fixture")
                    };
                    *displayed = Ok("105".into());
                    // Claim::check_id alone cannot detect a displayed-value mutation.
                    cell.claim.check_id().expect("stored body unchanged");
                }
                1 => cell.key.line.occurrence += 1,
                2 => cell.claim.state = KnowledgeState::Known,
                3 => cell.claim.anchors[0].locator = "YTD!B3".into(),
                _ => cell.claim.metric_key = "another metric".into(),
            }
            let after = assess(&p, &t, &changed, None).expect("named unresolved member");
            assert!(production(&after).score.is_none(), "mutation {mutation}");
            assert!(matches!(&production(&after).outcome,
                OutcomeBinding::Unresolved { code, .. } if code == "FMINE-REF-507"));
            let summary = after
                .summaries
                .iter()
                .find(|s| s.metric == "production_koz")
                .unwrap();
            assert_eq!(
                (
                    summary.scores.n_members,
                    summary.scores.n_resolved,
                    summary.scores.n_unresolved
                ),
                (1, 0, 1)
            );
        }
    }

    #[test]
    fn numeric_placeholder_remains_unresolved_even_with_usable_knowledge_state() {
        let (p, t, mut c) = synthetic_evidence().expect("fixture");
        c[0].claim.representation = Representation::Placeholder {
            token: "N/A".into(),
        };
        c[0].claim
            .check_id()
            .expect("representation is outside current Claim body");
        let report = assess(&p, &t, &c, None).expect("retained member");
        assert!(production(&report).score.is_none());
        assert!(matches!(&production(&report).outcome,
            OutcomeBinding::Unresolved { code, .. } if code == "FMINE-REF-208"));
    }

    #[test]
    fn registered_altered_and_missing_documents_refuse_without_fallback() {
        let root = registered_workspace();
        std::fs::write(root.join("docs/Guidance.pdf"), b"altered synthetic bytes")
            .expect("alter own fixture");
        let err = run(&root, "synthetic", None, None).expect_err("altered bytes");
        assert_eq!(err.code, "FMINE-REF-505");
        assert!(err.what.contains("Guidance.pdf"));
        std::fs::rename(
            root.join("docs/Guidance.pdf"),
            root.join("docs/Guidance.saved"),
        )
        .expect("preserve fixture under another name");
        let err = run(&root, "synthetic", None, None).expect_err("missing bytes");
        assert_eq!(err.code, "FMINE-REF-506");
        assert!(err.what.contains("Guidance.pdf"));
    }

    #[test]
    fn synthetic_arithmetic_never_overrides_explicit_negative_states() {
        let (p, t, mut c) = synthetic_evidence().expect("fixture");
        for state in [
            KnowledgeState::Known,
            KnowledgeState::Estimated,
            KnowledgeState::Unknown,
        ] {
            c[0].claim.state = state;
            reseal(&mut c[0]);
            let r = assess(&p, &t, &c, None).expect("allowed synthetic number");
            assert!(production(&r).score.is_some(), "{state:?}");
        }
        for state in [
            KnowledgeState::Conflicted,
            KnowledgeState::Stale,
            KnowledgeState::NotObservable,
            KnowledgeState::Redacted,
            KnowledgeState::Indeterminate,
            KnowledgeState::NotApplicable,
        ] {
            c[0].claim.state = state;
            reseal(&mut c[0]);
            let r = assess(&p, &t, &c, None).expect("negative state stays visible");
            assert!(production(&r).score.is_none(), "{state:?}");
            assert!(production(&r).score_claim.is_none(), "{state:?}");
            assert!(
                matches!(&production(&r).outcome,
                    OutcomeBinding::Unresolved { code, .. } if code == "FMINE-REF-202"),
                "{state:?}"
            );
        }
    }

    #[test]
    fn registered_aisc_uses_the_declared_sold_ounce_definition() {
        let root = registered_workspace();
        let r = run(&root, "synthetic", None, None).expect("registered AISC input");
        let row = r
            .rows
            .iter()
            .find(|r| r.metric == "aisc_usd_per_oz")
            .expect("AISC row");
        let score = row.score.expect("AISC score");
        assert_eq!(score.side, Side::Inside);
        assert_eq!(score.z, Some(Rat::new(1, 2).expect("half")));
        assert!(!score.boundary);
        let OutcomeBinding::Resolved { cell, comparative } = &row.outcome else {
            panic!("resolved AISC source")
        };
        assert!(!*comparative);
        assert_eq!(cell.locator, "Annual.xlsx:YTD!D3");
        assert_eq!(cell.key.unit, "usd_per_oz");
        assert_eq!(cell.claim.additivity, AdditivityClass::IntensiveWeighted);
    }

    #[test]
    fn canonical_claims_keep_source_parents_and_missing_guidance() {
        let (p, t, c) = synthetic_evidence().expect("fixture");
        let r = assess(&p, &t, &c, None).expect("canonical rows");
        let row = production(&r);
        let g = &row.member.guidance;
        let y = row.member.outcome.as_ref().expect("interpreted outcome");
        let score = row.score_claim.as_ref().expect("canonical score");
        g.check_id().expect("guidance id");
        y.check_id().expect("outcome id");
        score.check_id().expect("score id");
        assert_eq!(g.value_tag(), g.value.canonical_tag());
        assert_eq!(y.value_tag(), y.value.canonical_tag());
        assert_eq!(score.value_tag(), score.value.canonical_tag());
        assert_eq!(score.value.z, row.score.expect("numeric score").z);
        assert_eq!(y.parents, vec![c[0].claim.claim_id]);
        assert!(score.parents.contains(&g.claim_id));
        assert!(score.parents.contains(&y.claim_id));
        assert_eq!(score.evidence, EvidenceClass::Assumed);
        assert!(
            score
                .anchors
                .iter()
                .any(|a| a.artifact_path == "synthetic-guidance.txt")
        );
        assert!(
            score
                .anchors
                .iter()
                .any(|a| a.artifact_sha256 == c[0].claim.anchors[0].artifact_sha256)
        );
        let missing = r
            .rows
            .iter()
            .find(|r| r.metric == "aisc_usd_per_oz")
            .expect("missing member");
        missing
            .member
            .guidance
            .check_id()
            .expect("missing member guidance");
        assert!(missing.member.outcome.is_none());
        assert!(missing.score_claim.is_none());
        assert!(r.to_json().contains("\"canonical_body_hex\""));
        let legacy = row.claims_json();
        assert!(legacy.contains("\"metric_key\":\"guidance.interpreted_target\""));
        assert!(legacy.contains("\"score_refusal\":null"));
        assert!(legacy.contains("\"hit\":\"certain_miss\""));
        assert!(legacy.contains("\"z_bounds\":"));
        assert!(legacy.contains("\"path\":\"synthetic-guidance.txt\""));
    }

    #[test]
    fn registered_synthetic_unknown_remains_unknown_in_its_score_claim() {
        let root = registered_workspace();
        let r = run(&root, "synthetic", None, None).expect("synthetic arithmetic");
        for row in &r.rows {
            assert!(row.score.is_some());
            assert_eq!(
                row.member
                    .outcome
                    .as_ref()
                    .expect("source interpretation")
                    .state,
                KnowledgeState::Unknown
            );
            let score = row.score_claim.as_ref().expect("hypothetical score");
            assert_eq!(score.state, KnowledgeState::Unknown);
            assert_eq!(score.evidence, EvidenceClass::Assumed);
            assert!(row.claims_json().contains("\"score_refusal\":null"));
        }
    }

    #[test]
    fn registered_public_interpretations_use_canonical_derived_claims() {
        let root = registered_workspace();
        // The invented test issuer exercises the public path without introducing real filings.
        for kind in ["sources", "transcriptions"] {
            let path = root.join(format!("registries/{kind}"));
            let text =
                std::fs::read_to_string(path.join("synthetic.toml")).expect("fixture registry");
            std::fs::write(
                path.join("example.toml"),
                text.replace("issuer = \"synthetic\"", "issuer = \"example\""),
            )
            .expect("invented public-path registry");
        }
        let r = run(&root, "example", None, None).expect("public-path Claims");
        assert!(!r.synthetic);
        for row in &r.rows {
            let score = row.score_claim.as_ref().expect("canonical score");
            score.check_id().expect("canonical id");
            assert_eq!(
                score.evidence,
                EvidenceClass::Derived.meet(EvidenceClass::ISSUER)
            );
            assert_eq!(score.state, KnowledgeState::Known);
            assert_eq!(score.value.z, row.score.expect("numeric score").z);
            assert!(row.claims_json().contains("\"score_refusal\":null"));
        }
    }
