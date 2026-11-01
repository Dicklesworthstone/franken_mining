#![cfg(test)]

//! BD-340: actual file and CLI checks on invented PDF bytes and synthetic reader records.
//! These stubs establish loader behavior, never a real filing or independent human review.

#![allow(clippy::expect_used, clippy::unwrap_used)]

#[cfg(test)]
mod tests {
    use std::fmt::Write as _;
    use std::path::{Path, PathBuf};
    use std::process::Command;
    use std::sync::atomic::{AtomicU64, Ordering};

    use fmine_cli::{Envelope, OutputFormat};
    use fmine_core::hash::sha256;
    use fmine_public::sources::{registry_path, transcriptions_path};

    const DOCUMENT: &[u8] = b"%PDF-1.7 synthetic reserve table, not a real PDF or parsed source";
    const FILE: &str = "Invented.pdf";
    const DEFINITIONS: &str = "table-identities.toml";
    static NEXT: AtomicU64 = AtomicU64::new(0);

    #[derive(Clone, Copy)]
    enum Readings {
        Agreed,
        MissingOpening,
        PendingOpening,
        ConflictingOpening,
        Closing75,
    }

    fn write_definitions(root: &Path, net_change_sign: i8) {
        let mut definitions =
            String::from("schema = \"fmine.table-identities.v1\"\nissuer = \"table_test\"\n");
        for (id, family, kind, total, period) in [
            (
                "reserve_rollforward",
                "reserve_balance",
                "roll_forward",
                "closing",
                "2025FY",
            ),
            (
                "multi_year",
                "reserve_additions",
                "sum",
                "total",
                "2023-2025",
            ),
        ] {
            write!(
                definitions,
                "\n[[identity]]\nid = \"{id}\"\ndoc = \"{FILE}\"\nfamily = \"{family}\"\n\
                 kind = \"{kind}\"\ntotal = \"{total}\"\ntotal_period = \"{period}\"\n\
                 unit = \"koz\"\nbasis = \"100%\"\nconstruction = \"rounded_once\"\n\
                 convention = \"closed\"\n",
            )
            .unwrap();
        }
        for (identity, quantity, period, sign, role) in [
            ("reserve_rollforward", "opening", "2024FY", 1, "opening"),
            ("reserve_rollforward", "depletion", "2025FY", 1, "move"),
            (
                "reserve_rollforward",
                "net_change",
                "2025FY",
                net_change_sign,
                "move",
            ),
            ("multi_year", "year_2023", "2023FY", 1, "member"),
            ("multi_year", "year_2024", "2024FY", 1, "member"),
            ("multi_year", "year_2025", "2025FY", 1, "member"),
        ] {
            write!(
                definitions,
                "\n[[term]]\nidentity = \"{identity}\"\nquantity = \"{quantity}\"\n\
                 period = \"{period}\"\nsign = {sign}\nrole = \"{role}\"\n",
            )
            .unwrap();
        }
        std::fs::write(root.join(DEFINITIONS), definitions).unwrap();
    }

    fn write_readings(root: &Path, mode: Readings) {
        let mut readings =
            String::from("schema = \"fmine.transcriptions.v1\"\nissuer = \"table_test\"\n");
        for reader in ["reader_a", "reader_b"] {
            for (quantity, token, period) in [
                ("opening", "100", "2024FY"),
                ("depletion", "(20)", "2025FY"),
                ("net_change", "(5)", "2025FY"),
                ("closing", "85", "2025FY"),
                ("year_2023", "5", "2023FY"),
                ("year_2024", "7", "2024FY"),
                ("year_2025", "8", "2025FY"),
                ("total", "20", "2023-2025"),
            ] {
                if quantity == "opening"
                    && (matches!(mode, Readings::MissingOpening)
                        || matches!(mode, Readings::PendingOpening) && reader == "reader_b")
                {
                    continue;
                }
                let token = if quantity == "opening"
                    && matches!(mode, Readings::ConflictingOpening)
                    && reader == "reader_b"
                {
                    "101"
                } else if quantity == "closing" && matches!(mode, Readings::Closing75) {
                    "75"
                } else {
                    token
                };
                write!(
                    readings,
                    "\n[[entry]]\ndoc = \"{FILE}\"\ndoc_sha256 = \"{}\"\npage = 1\n\
                     table = \"Invented reserves\"\nrow = \"{quantity}\"\ncol = \"{period}\"\n\
                     token = \"{token}\"\nquantity = \"{quantity}\"\nunit = \"koz\"\n\
                     basis = \"100%\"\nperiod = \"{period}\"\nreader = \"{reader}\"\n",
                    sha256(DOCUMENT).to_hex(),
                )
                .unwrap();
            }
        }
        std::fs::write(transcriptions_path(root, "table_test"), readings).unwrap();
    }

    fn workspace(tag: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "fmine-check-tables-{tag}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed),
        ));
        for dir in [
            "documents",
            "registries/sources",
            "registries/transcriptions",
        ] {
            std::fs::create_dir_all(root.join(dir)).unwrap();
        }
        std::fs::write(root.join("documents").join(FILE), DOCUMENT).unwrap();
        // The source schema requires a workbook entry. Its deliberately absent bytes cannot affect
        // this PDF-only request; the normal workbook path must still refuse them.
        let source = format!(
            "schema = \"fmine.sources.v1\"\nissuer = \"table_test\"\ncorpus_dir = \"corpus\"\n\
             retrieved_utc = \"synthetic fixture\"\nurl_prefix = \"https://example.invalid/\"\n\
             \n[[source]]\nfile = \"Unused.xlsx\"\nkind = \"mine_stats\"\nquarter = \"2025Q4\"\n\
             path = \"Unused.xlsx\"\nurl_basis = \"W1 table\"\nbytes = 1\nsha256 = \"{}\"\n\
             \n[[subject]]\nname = \"Invented Mine\"\n\
             \n[[document]]\nfile = \"{FILE}\"\ndir = \"documents\"\nkind = \"aif\"\n\
             url = \"https://example.invalid/{FILE}\"\nurl_basis = \"W1 table\"\n\
             published_at = \"2026-02-01\"\npublished_at_basis = \"FACT: synthetic fixture date\"\n\
             pages = 1\nbytes = {}\nsha256 = \"{}\"\n",
            sha256(b"x").to_hex(),
            DOCUMENT.len(),
            sha256(DOCUMENT).to_hex(),
        );
        std::fs::write(registry_path(&root, "table_test"), source).unwrap();
        write_readings(&root, Readings::Agreed);
        write_definitions(&root, -1);
        root
    }

    fn invoke(root: &Path, extra: &[&str]) -> Envelope {
        let args: Vec<String> = [
            "check",
            "tables",
            "--issuer",
            "table_test",
            "--definitions",
            DEFINITIONS,
        ]
        .into_iter()
        .chain(extra.iter().copied())
        .map(str::to_owned)
        .collect();
        let (envelope, format) = fmine_cli::invoke(&args, root);
        assert_eq!(format, OutputFormat::Json);
        envelope
    }

    fn successful(root: &Path) -> Envelope {
        let result = invoke(root, &[]);
        assert_eq!(result.exit_code, 0, "{}", result.to_json());
        assert!(result.refusal.is_none(), "{}", result.to_json());
        result
    }

    fn report_digest(payload: &str) -> &str {
        let digest = payload
            .rsplit_once("\"digest\":\"")
            .expect("report has a content digest")
            .1
            .split('"')
            .next()
            .unwrap();
        assert_eq!(digest.len(), 64);
        assert!(digest.bytes().all(|b| b.is_ascii_hexdigit()));
        digest
    }

    fn assert_refusal(root: &Path, code: &str, exit: u8) {
        let result = invoke(root, &[]);
        assert_eq!(result.exit_code, exit, "{}", result.to_json());
        assert_eq!(result.refusal.as_ref().unwrap().code, code);
        assert!(result.to_json().contains(FILE));
        assert!(result.to_json().contains("\"payload\":null"));
    }

    #[test]
    fn registered_pdf_rollforward_and_explicit_multiyear_membership_are_solved() {
        let root = workspace("positive");
        let result = successful(&root);
        let payload = &result.payload_json;
        assert!(payload.contains("\"schema\":\"fmine.pdf-table-check.v1\""));
        assert_eq!(
            payload.matches("\"verdict\":\"feasible\"").count(),
            2,
            "{payload}"
        );
        assert_eq!(
            payload
                .matches("\"verdict\":\"feasible\",\"checked\":true")
                .count(),
            2
        );
        assert!(payload.contains("\"refused\":[]"));
        for expected in [
            "reserve_rollforward",
            "multi_year",
            "roll_forward",
            "2023-2025",
            "2023FY",
            "2024FY",
            "2025FY",
            "reader_a",
            "reader_b",
            "Invented.pdf:p1:Invented reserves:closing/2025FY",
            "(20)",
            "(5)",
        ] {
            assert!(payload.contains(expected), "missing {expected}: {payload}");
        }
        assert!(payload.contains(&sha256(DOCUMENT).to_hex()));
        assert!(payload.contains(&format!(
            "\"definitions_sha256\":\"{}\"",
            sha256(&std::fs::read(root.join(DEFINITIONS)).unwrap()).to_hex()
        )));
        assert!(payload.contains("\"gate_eligible\":false"));
        let args = ["check", "tables", "--issuer", "table_test"].map(str::to_owned);
        let (workbooks, _) = fmine_cli::invoke(&args, &root);
        assert_eq!(workbooks.exit_code, 5, "{}", workbooks.to_json());
        assert!(workbooks.to_json().contains("Unused.xlsx"));
    }

    #[test]
    fn declared_sign_change_changes_the_finding_and_digest_without_changing_source_bytes() {
        let root = workspace("signed");
        let positive = successful(&root);
        // Raw net_change is (5). Its declared negative sign makes +5 in the original identity;
        // changing the declaration gives 100 - 20 - 5 = 75, which cannot round to the printed 85.
        write_definitions(&root, 1);
        let conflict = successful(&root);
        assert_eq!(
            conflict
                .payload_json
                .matches("\"verdict\":\"infeasible\"")
                .count(),
            1,
            "{}",
            conflict.payload_json
        );
        assert_eq!(
            conflict
                .payload_json
                .matches("\"verdict\":\"feasible\"")
                .count(),
            1
        );
        assert!(conflict.payload_json.contains("certificate_digest"));
        assert!(
            conflict
                .payload_json
                .contains("\"verdict\":\"infeasible\",\"checked\":true")
        );
        assert!(conflict.payload_json.contains("\"certificate_digest\":\""));
        assert!(
            conflict
                .payload_json
                .contains("\"blame\":{\"status\":\"blamed\"")
        );
        assert_ne!(positive.payload_json, conflict.payload_json);
        assert_ne!(
            report_digest(&positive.payload_json),
            report_digest(&conflict.payload_json)
        );
        assert_eq!(
            std::fs::read(root.join("documents").join(FILE)).unwrap(),
            DOCUMENT
        );
        for field in ["\"corrected\":", "\"best\":", "\"fix\":"] {
            assert!(!conflict.payload_json.contains(field));
        }
        write_readings(&root, Readings::Closing75);
        let changed_readings = successful(&root);
        assert_eq!(
            changed_readings
                .payload_json
                .matches("\"verdict\":\"feasible\"")
                .count(),
            2,
            "{}",
            changed_readings.payload_json
        );
        assert_ne!(conflict.payload_json, changed_readings.payload_json);
        assert_ne!(
            report_digest(&conflict.payload_json),
            report_digest(&changed_readings.payload_json)
        );
    }

    #[test]
    fn missing_and_single_reader_operands_remain_refused_identity_members() {
        let root = workspace("missing");
        for mode in [Readings::MissingOpening, Readings::PendingOpening] {
            write_readings(&root, mode);
            let result = successful(&root);
            let payload = &result.payload_json;
            assert!(
                payload.contains("\"identity\":\"reserve_rollforward\""),
                "{payload}"
            );
            assert!(payload.contains("opening"));
            assert!(!payload.contains("\"refused\":[]"));
            assert_eq!(
                payload.matches("\"verdict\":\"feasible\"").count(),
                1,
                "{payload}"
            );
            assert!(payload.contains("multi_year"));
            let markdown = result.rendered_markdown.as_ref().unwrap();
            assert!(markdown.contains("Evidence retained for unresolved identities"));
            assert!(markdown.contains("Invented.pdf:p1:Invented reserves:closing/2025FY"));
            assert!(markdown.contains(&sha256(DOCUMENT).to_hex()));
        }
    }

    #[test]
    fn altered_and_missing_pdf_bytes_refuse_before_any_certified_result() {
        let root = workspace("bytes");
        let path = root.join("documents").join(FILE);
        let mut altered = DOCUMENT.to_vec();
        altered[0] ^= 1;
        std::fs::write(&path, altered).unwrap();
        assert_refusal(&root, "FMINE-REF-505", 8);
        std::fs::write(&path, DOCUMENT).unwrap();
        let held = root.join("documents/Invented.held");
        std::fs::rename(&path, &held).unwrap();
        assert_refusal(&root, "FMINE-REF-506", 5);
        std::fs::rename(&held, &path).unwrap();
        successful(&root);
    }

    #[test]
    fn declared_pdf_bytes_are_required_even_before_any_readings_exist() {
        let root = workspace("unread-document");
        std::fs::write(
            transcriptions_path(&root, "table_test"),
            "schema = \"fmine.transcriptions.v1\"\nissuer = \"table_test\"\n",
        )
        .unwrap();
        let path = root.join("documents").join(FILE);
        let held = root.join("documents/Unread.held");
        std::fs::rename(&path, &held).unwrap();
        assert_refusal(&root, "FMINE-REF-506", 5);
        std::fs::rename(&held, &path).unwrap();
        let result = successful(&root);
        assert!(result.payload_json.contains("\"components\":[]"));
        for id in ["reserve_rollforward", "multi_year"] {
            assert!(
                result
                    .payload_json
                    .contains(&format!("\"identity\":\"{id}\"")),
                "{}",
                result.payload_json
            );
        }
        assert!(!result.payload_json.contains("\"checked\":true"));
    }

    #[test]
    fn conflicting_reader_never_becomes_a_solver_operand() {
        let root = workspace("readers");
        write_readings(&root, Readings::ConflictingOpening);
        assert_refusal(&root, "FMINE-REF-202", 3);
        write_readings(&root, Readings::Agreed);
        successful(&root);
    }

    #[test]
    fn exact_binary_json_markdown_and_reruns_keep_citations_and_authority_boundaries() {
        let root = workspace("binary");
        let expected = successful(&root);
        assert_eq!(expected.to_json(), successful(&root).to_json());
        for name in [
            "reserve_disclosure_claim",
            "execution_authorized",
            "geotechnical_signoff",
            "regulatory_compliance_claim",
        ] {
            assert!(expected.to_json().contains(&format!("\"{name}\":false")));
        }
        for (format, expected_text) in [
            ("json", expected.to_json()),
            ("markdown", expected.rendered_markdown.clone().unwrap()),
        ] {
            let output = Command::new(env!("CARGO_BIN_EXE_fmine"))
                .args([
                    "check",
                    "tables",
                    "--issuer",
                    "table_test",
                    "--definitions",
                    DEFINITIONS,
                    "--format",
                    format,
                    "--workspace",
                ])
                .arg(&root)
                .output()
                .unwrap();
            assert_eq!(output.status.code(), Some(0));
            assert_eq!(
                String::from_utf8(output.stdout).unwrap(),
                format!("{expected_text}\n")
            );
        }
        let markdown = expected.rendered_markdown.unwrap();
        assert!(markdown.contains(FILE));
        assert!(markdown.contains(&sha256(DOCUMENT).to_hex()));
        assert!(markdown.contains("2023-2025"));
        assert!(markdown.contains("Signed members (period, role)"));
        assert!(markdown.contains("-1 × net\\_change (2025FY, move)"));
        assert!(markdown.contains("rounded\\_once / closed"));
    }

    #[test]
    fn definitions_selectors_and_unknown_flags_are_usage_errors() {
        let root = workspace("grammar");
        for extra in [
            vec!["--through", "2025Q4"],
            vec!["--synthetic"],
            vec!["--fix"],
            vec!["--as-of", "2026-02-01"],
            vec!["--definitions", DEFINITIONS],
        ] {
            let result = invoke(&root, &extra);
            assert_eq!(result.exit_code, 2, "{extra:?}: {}", result.to_json());
            assert!(result.refusal.is_none());
        }
        for args in [
            vec!["check", "tables", "--definitions", DEFINITIONS],
            vec!["check", "tables", "--issuer", "not_registered"],
        ] {
            let args: Vec<String> = args.into_iter().map(str::to_owned).collect();
            let (result, _) = fmine_cli::invoke(&args, &root);
            assert_eq!(result.exit_code, 2, "{}", result.to_json());
        }
    }
}
