#[cfg(test)]
mod tests {
    use super::*;
    use fmine_public::sources::DocumentRow;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT: AtomicU64 = AtomicU64::new(0);

    struct Fixture {
        root: PathBuf,
        profile: IssuerProfile,
        transcription: String,
    }

    impl Fixture {
        fn new() -> Self {
            let root = loop {
                let candidate = std::env::temp_dir().join(format!(
                    "fmine-lint-guidance-{}-{}",
                    std::process::id(),
                    NEXT.fetch_add(1, Ordering::Relaxed)
                ));
                match std::fs::create_dir(&candidate) {
                    Ok(()) => break candidate,
                    Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                    Err(e) => panic!("create isolated synthetic fixture: {e}"),
                }
            };
            std::fs::create_dir_all(root.join("registries/transcriptions")).unwrap();
            std::fs::create_dir_all(root.join("corpus")).unwrap();
            let mut profile = fmine_public::synthetic::synthetic_profile();
            profile.subjects = vec!["Example Mine".into(), "Copper Mine".into()];
            profile.documents.clear();
            Self {
                root,
                profile,
                transcription: "schema = \"fmine.transcriptions.v1\"\nissuer = \"synthetic\"\n"
                    .into(),
            }
        }

        #[allow(clippy::too_many_arguments)]
        fn row(
            &mut self,
            file: &str,
            date: &str,
            subject: &str,
            metric: &str,
            token: &str,
            basis: &str,
            readers: usize,
        ) {
            let bytes = format!(
                "Invented guidance fixture: {file}|{date}|{subject}|{metric}|{token}|{basis}\n"
            );
            let hash = sha256(bytes.as_bytes()).to_hex();
            std::fs::write(self.root.join("corpus").join(file), &bytes).unwrap();
            self.profile.documents.push(DocumentRow {
                file: file.into(),
                dir: "corpus".into(),
                kind: "investor_deck".into(),
                url: format!("https://example.invalid/{file}"),
                url_basis: "synthetic".into(),
                published_at: date.into(),
                published_at_basis: "FACT: invented calendar for regression".into(),
                pages: 1,
                bytes: bytes.len() as u64,
                sha256: hash.clone(),
            });
            let unit = if metric == "production_koz" {
                "koz"
            } else if metric == "production_kt" {
                "kt"
            } else if metric.ends_with("per_lb") {
                "usd_per_lb"
            } else {
                "usd_per_oz"
            };
            for reader in 0..readers {
                let _ = write!(
                    self.transcription,
                    "\n[[entry]]\ndoc = \"{file}\"\ndoc_sha256 = \"{hash}\"\npage = 1\ntable = \"Invented guidance\"\nrow = \"{subject}\"\ncol = \"{metric}\"\ntoken = \"{token}\"\nquantity = \"guidance_{}_{metric}\"\nunit = \"{unit}\"\nbasis = \"{basis}\"\nperiod = \"2030FY\"\nreader = \"reader_{reader}\"\n",
                    crate::track_record_data::site_key(subject).replace('-', "_")
                );
            }
            self.save();
        }

        fn save(&self) {
            std::fs::write(
                self.root.join("registries/transcriptions/synthetic.toml"),
                &self.transcription,
            )
            .unwrap();
        }

        fn assess(&self, text: &str, cut: &str) -> GuidanceReport {
            assess(text, &self.profile, &self.root, cut)
        }

        fn register(&self, workbooks: bool) {
            let path = self.root.join("registries/sources/synthetic.toml");
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            let mut registry = if workbooks {
                let path = fmine_public::synthetic::write_issuer_workspace(&self.root).unwrap();
                std::fs::read_to_string(path).unwrap()
            } else {
                "schema = \"fmine.sources.v1\"\nissuer = \"synthetic\"\ncorpus_dir = \"corpus\"\nretrieved_utc = \"2030-08-01T00:00:00Z\"\nurl_prefix = \"https://example.invalid/\"\n".into()
            };
            for subject in &self.profile.subjects {
                let _ = write!(registry, "\n[[subject]]\nname = \"{subject}\"\n");
            }
            for doc in &self.profile.documents {
                let _ = write!(
                    registry,
                    "\n[[document]]\nfile = \"{}\"\nkind = \"{}\"\nurl = \"https://example.invalid/{}\"\nurl_basis = \"fetched and verified 2030-08-01\"\npublished_at = \"{}\"\npublished_at_basis = \"{}\"\npages = {}\nbytes = {}\nsha256 = \"{}\"\n",
                    doc.file,
                    doc.kind,
                    doc.file,
                    doc.published_at,
                    doc.published_at_basis,
                    doc.pages,
                    doc.bytes,
                    doc.sha256
                );
            }
            std::fs::write(path, registry).unwrap();
        }

        fn invoke(&self, cut: &str, format: &str) -> crate::Envelope {
            let args = [
                "lint",
                "draft.md",
                "--issuer",
                "synthetic",
                "--as-of",
                cut,
                "--format",
                format,
            ]
            .map(str::to_string);
            crate::invoke(&args, &self.root).0
        }
    }

    const DRAFT: &str = "Example Mine FY2030 100% basis gold production forecast is 125 koz.";

    #[test]
    fn latest_guidance_changes_at_publication_and_future_bytes_do_not_leak() {
        let mut f = Fixture::new();
        f.row(
            "original.txt",
            "2030-01-15",
            "Example Mine",
            "production_koz",
            "100 - 120",
            "100%",
            2,
        );
        f.row(
            "revision.txt",
            "2030-07-15",
            "Example Mine",
            "production_koz",
            "120 - 140",
            "100%",
            2,
        );
        let before = f.assess(DRAFT, "2030-07-14");
        assert_eq!(before.findings[0].comparison, Some("above_guidance"));
        assert_eq!(before.findings[0].evidence.len(), 1);
        let after = f.assess(DRAFT, "2030-07-15");
        assert_eq!(after.findings[0].comparison, Some("within_guidance"));
        assert_eq!(after.findings[0].evidence.len(), 2);
        assert_ne!(before.findings[0].selected, after.findings[0].selected);
        assert!(
            after
                .to_json()
                .contains("revision.txt:p1:Invented guidance")
        );
        assert!(after.to_json().contains("canonical_body_hex"));
        assert!(after.to_markdown().contains("2030-07-15"));
        std::fs::write(f.root.join("corpus/revision.txt"), b"altered later bytes").unwrap();
        assert_eq!(before.to_json(), f.assess(DRAFT, "2030-07-14").to_json());
        let altered = f.assess(DRAFT, "2030-07-15");
        assert_eq!(altered.findings[0].status, "guidance_refused");
        assert_eq!(
            altered.findings[0].refusal.as_ref().unwrap().code,
            "FMINE-REF-505"
        );
    }

    #[test]
    fn missing_or_pending_guidance_never_falls_back_to_an_older_target() {
        let mut f = Fixture::new();
        assert_eq!(
            f.assess(DRAFT, "2030-08-01").findings[0].status,
            "guidance_not_in_registered_record"
        );
        f.row(
            "original.txt",
            "2030-01-15",
            "Example Mine",
            "production_koz",
            "100 - 120",
            "100%",
            2,
        );
        f.row(
            "unfinished.txt",
            "2030-07-15",
            "Example Mine",
            "production_koz",
            "120 - 140",
            "100%",
            1,
        );
        let result = f.assess(DRAFT, "2030-08-01");
        assert_eq!(result.findings[0].status, "guidance_incomplete");
        assert!(result.findings[0].selected.is_none());
        assert!(result.findings[0].pending[0].contains("unfinished.txt"));
        assert_eq!(
            f.assess(DRAFT, "2030-07-14").findings[0].comparison,
            Some("above_guidance")
        );
    }

    #[test]
    fn copper_cost_ranges_use_exact_guidance_and_require_per_sold_pound() {
        let mut f = Fixture::new();
        f.row(
            "copper.txt",
            "2030-01-15",
            "Copper Mine",
            "aisc_usd_per_lb",
            "3.40 - 3.75",
            "100%;aisc=issuer-reported-per-sold-lb",
            2,
        );
        let draft =
            "Copper Mine FY2030 100% basis AISC guidance is $3.40 - 3.75/lb per pound sold.";
        let good = f.assess(draft, "2030-02-01");
        assert_eq!(
            good.findings[0].status,
            "forward_looking",
            "{}",
            good.to_json()
        );
        assert_eq!(good.findings[0].comparison, Some("within_guidance"));
        for (number, expected) in [
            ("3.80 - 4.00", "above_guidance"),
            ("3.20 - 3.30", "below_guidance"),
            ("3.20 - 3.60", "overlaps_guidance_boundary"),
        ] {
            assert_eq!(
                f.assess(&draft.replace("3.40 - 3.75", number), "2030-02-01")
                    .findings[0]
                    .comparison,
                Some(expected)
            );
        }
        let wrong = f.assess(&draft.replace("pound sold", "ounce sold"), "2030-02-01");
        assert_eq!(wrong.findings[0].status, "not_assessed");
        let wrong = f.assess(&draft.replace("/lb", "/oz"), "2030-02-01");
        assert_ne!(wrong.findings[0].status, "forward_looking");
    }

    #[test]
    fn ownership_series_remain_independent_and_amount_never_selects_a_share() {
        let mut f = Fixture::new();
        f.row(
            "full.txt",
            "2030-01-15",
            "Example Mine",
            "production_koz",
            "100 - 120",
            "100%",
            2,
        );
        f.row(
            "share.txt",
            "2030-01-15",
            "Example Mine",
            "production_koz",
            "80 - 96",
            "attributable:4/5",
            2,
        );
        assert_eq!(
            f.assess(DRAFT, "2030-02-01").findings[0].comparison,
            Some("above_guidance")
        );
        let attributable = DRAFT
            .replace("100% basis", "attributable")
            .replace("125", "90");
        assert_eq!(
            f.assess(&attributable, "2030-02-01").findings[0].comparison,
            Some("within_guidance")
        );
        f.row(
            "other-share.txt",
            "2030-02-15",
            "Example Mine",
            "production_koz",
            "70 - 80",
            "attributable:3/4",
            2,
        );
        let ambiguous = f.assess(&attributable, "2030-03-01");
        assert_eq!(ambiguous.findings[0].status, "guidance_ambiguous");
        assert_eq!(ambiguous.findings[0].evidence.len(), 2);
        assert!(ambiguous.findings[0].comparison.is_none());
        assert_eq!(
            f.assess(DRAFT, "2030-03-01").findings[0].comparison,
            Some("above_guidance")
        );
    }

    #[test]
    fn duplicate_latest_dates_and_reader_disagreement_refuse() {
        let mut f = Fixture::new();
        f.row(
            "one.txt",
            "2030-01-15",
            "Example Mine",
            "production_koz",
            "100 - 120",
            "100%",
            2,
        );
        f.row(
            "two.txt",
            "2030-01-15",
            "Example Mine",
            "production_koz",
            "120 - 140",
            "100%",
            2,
        );
        let result = f.assess(DRAFT, "2030-02-01");
        assert_eq!(result.findings[0].status, "guidance_refused");
        assert_eq!(
            result.findings[0].refusal.as_ref().unwrap().code,
            "FMINE-REF-104"
        );
        f.transcription =
            f.transcription
                .replacen("token = \"100 - 120\"", "token = \"101 - 120\"", 1);
        f.save();
        let result = f.assess(DRAFT, "2030-02-01");
        assert_eq!(
            result.findings[0].refusal.as_ref().unwrap().code,
            "FMINE-REF-202"
        );
    }

    #[test]
    fn unsupported_prose_stays_unchecked_and_unrelated_optional_documents_are_not_read() {
        let mut f = Fixture::new();
        f.row(
            "gold.txt",
            "2030-01-15",
            "Example Mine",
            "production_koz",
            "100 - 120",
            "100%",
            2,
        );
        f.row(
            "copper.txt",
            "2030-01-15",
            "Copper Mine",
            "production_kt",
            "30 - 40",
            "100%",
            2,
        );
        std::fs::write(f.root.join("corpus/copper.txt"), b"unrelated altered bytes").unwrap();
        assert_eq!(
            f.assess(DRAFT, "2030-02-01").findings[0].comparison,
            Some("above_guidance")
        );
        assert!(
            f.assess("Ordinary historical AISC is $100/oz.", "2030-02-01")
                .findings
                .is_empty()
        );
        for draft in [
            DRAFT.replace(" is ", " might be "),
            DRAFT.replace("125 koz", "125 koz and 200 koz"),
            DRAFT.replace("125 koz", "125 kt"),
            DRAFT.replace("Example Mine", "Unknown Mine"),
            DRAFT.replace("100% basis", ""),
            DRAFT.replace("FY2030", "Q1 2030"),
            DRAFT.replace("forecast", "forecast probability"),
        ] {
            let report = f.assess(&draft, "2030-02-01");
            assert_eq!(report.findings[0].status, "not_assessed", "{draft}");
            assert!(report.findings[0].comparison.is_none());
        }
    }

    #[test]
    fn cli_loads_document_only_guidance_and_keeps_optional_failures_out_of_numeric_lint() {
        for workbooks in [false, true] {
            let mut f = Fixture::new();
            f.row(
                "original.txt",
                "2030-01-15",
                "Example Mine",
                "production_koz",
                "100 - 120",
                "100%",
                2,
            );
            f.row(
                "revision.txt",
                "2030-07-15",
                "Example Mine",
                "production_koz",
                "120 - 140",
                "100%",
                2,
            );
            f.row(
                "optional-copper.txt",
                "2030-01-15",
                "Copper Mine",
                "production_kt",
                "30 - 40",
                "100%",
                2,
            );
            f.register(workbooks);
            std::fs::write(f.root.join("draft.md"), DRAFT).unwrap();
            let expected_root = workbooks.then(|| {
                let profile =
                    fmine_public::sources::load_issuer_profile(&f.root, "synthetic").unwrap();
                fmine_public::run_public_assurance(&profile, &f.root)
                    .unwrap()
                    .ledger_anchor
                    .state_root
                    .to_hex()
            });
            std::fs::write(
                f.root.join("corpus/optional-copper.txt"),
                b"irrelevant altered source",
            )
            .unwrap();
            let before = f.invoke("2030-07-14", "json");
            assert_eq!(before.exit_code, 0, "{}", before.to_json());
            assert!(
                before
                    .to_json()
                    .contains("\"forward_guidance\":{\"report_digest\"")
            );
            assert!(
                before
                    .to_json()
                    .contains("\"comparison\":\"above_guidance\"")
            );
            assert!(
                before
                    .to_json()
                    .contains("\"regulatory_compliance_claim\":false")
            );
            let state = expected_root
                .as_ref()
                .map_or_else(|| "null".to_string(), |root| js(root));
            assert!(
                before
                    .to_json()
                    .contains(&format!("\"source_state_root\":{state}"))
            );
            assert_eq!(before.to_json(), f.invoke("2030-07-14", "json").to_json());
            let after = f.invoke("2030-07-15", "markdown");
            assert_eq!(after.exit_code, 0, "{}", after.to_json());
            let md = after.rendered_markdown.as_ref().unwrap();
            assert!(md.contains("within_guidance"));
            assert!(md.contains("revision.txt:p1:Invented guidance"));
            std::fs::write(
                f.root.join("corpus/revision.txt"),
                b"altered selected revision",
            )
            .unwrap();
            assert_eq!(before.to_json(), f.invoke("2030-07-14", "json").to_json());
            let failure = f.invoke("2030-07-15", "json");
            assert_eq!(failure.exit_code, 0, "{}", failure.to_json());
            assert!(failure.to_json().contains("guidance_refused"));
            assert!(failure.to_json().contains("FMINE-REF-505"));
            assert!(failure.to_json().contains("\"lint\":{\"report_digest\""));
            assert!(
                failure
                    .to_json()
                    .contains(&format!("\"source_state_root\":{state}"))
            );
        }
    }
}
