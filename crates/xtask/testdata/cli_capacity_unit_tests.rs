#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use fmine_core::sha256;
    use fmine_registry::transcriptions::{Locator, parse_token};

    const BYTES: &[u8] = b"SYNTHETIC technical-report upper bounds; not issuer data.";

    fn document() -> DocumentRow {
        DocumentRow {
            file: "Bounds.pdf".into(),
            dir: "corpus".into(),
            kind: "technical_report".into(),
            url: "https://example.invalid/Bounds.pdf".into(),
            url_basis: "W1 table".into(),
            published_at: "2030-07-01".into(),
            published_at_basis: "FACT: synthetic publication date".into(),
            pages: 1,
            bytes: BYTES.len() as u64,
            sha256: sha256(BYTES).to_hex(),
        }
    }

    fn rows() -> Vec<Accepted> {
        [
            ("remaining_dry_feed_mt", "10", "dry_mt"),
            ("head_grade_g_per_t", "2", "g_per_dry_t"),
            ("recovery_fraction", "0.90", "fraction"),
        ]
        .into_iter()
        .map(|(suffix, token, unit)| Accepted {
            locator: Locator {
                doc: "Bounds.pdf".into(),
                page: 1,
                table: "Explicit maximum limits".into(),
                row: suffix.into(),
                col: "Remaining months".into(),
            },
            doc_sha256: sha256(BYTES).to_hex(),
            token: token.into(),
            value: parse_token(token).unwrap(),
            quantity: format!("capacity_example_mine_{suffix}"),
            unit: unit.into(),
            basis: BASIS.into(),
            period: "2030-07/2030-12".into(),
            readers: vec!["synthetic-a".into(), "synthetic-b".into()],
        })
        .collect()
    }

    fn readings(rows: &[Accepted]) -> String {
        let mut transcript =
            "schema = \"fmine.transcriptions.v1\"\nissuer = \"synthetic\"\n".to_string();
        for row in rows {
            for reader in &row.readers {
                let _ = write!(
                    transcript,
                    "\n[[entry]]\ndoc = {}\ndoc_sha256 = {}\npage = 1\ntable = {}\nrow = {}\ncol = {}\n\
                     token = {}\nquantity = {}\nunit = {}\nbasis = {}\nperiod = {}\nreader = {}\n",
                    j(&row.locator.doc),
                    j(&row.doc_sha256),
                    j(&row.locator.table),
                    j(&row.locator.row),
                    j(&row.locator.col),
                    j(&row.token),
                    j(&row.quantity),
                    j(&row.unit),
                    j(&row.basis),
                    j(&row.period),
                    j(reader),
                );
            }
        }
        transcript
    }

    fn required() -> Claim<Quantity> {
        let base = fmine_public::guidance::run_synthetic().unwrap();
        let mut value = base.completion.remaining_production.value.clone();
        value.ownership = Ownership::Full;
        value.interval = Interval {
            lo: Rat::from_integer(600),
            hi: Rat::from_integer(600),
            lo_strict: false,
            hi_strict: false,
        };
        let tag = value.canonical_tag();
        Claim::new(
            "synthetic.remaining-production",
            value,
            &tag,
            Representation::Deterministic,
            SpatialSupport::PeriodAggregate,
            AdditivityClass::ExtensiveAdditive,
            EvidenceClass::Assumed,
            EpistemicStatus::KnownKnown,
            KnowledgeState::Known,
            Confidentiality::public(),
            vec![],
            vec![],
            base.completion.remaining_production.anchors.clone(),
        )
    }

    #[test]
    fn explicit_caps_reproduce_cod01_exactly() {
        let required = required();
        let (limits, _) =
            limits_from_rows(&rows(), &document(), &required, EvidenceClass::Assumed).unwrap();
        let result = compare_production(&required, &limits).unwrap();
        let grams = Rat::new(311_034_768, 10_000_000).unwrap();
        let expected = Rat::from_integer(18_000).checked_div(grams).unwrap();
        assert_eq!(
            result.verdict,
            CapacityVerdict::ExceedsStatedProductionCeiling
        );
        assert_eq!(result.maximum_production.value.interval.lo, expected);
        assert_eq!(result.maximum_production.value.interval.hi, expected);
        let needed = Rat::from_integer(600)
            .checked_mul(grams)
            .unwrap()
            .checked_div(Rat::from_integer(9_000))
            .unwrap();
        assert_eq!(result.optimistic_required_grade.value.interval.lo, needed);
        assert_eq!(result.maximum_production.evidence, EvidenceClass::Assumed);
        assert!(
            result
                .maximum_production
                .anchors
                .iter()
                .any(|a| a.locator.contains("Bounds.pdf"))
        );
    }

    #[test]
    fn limits_refuse_estimates_units_support_ranges_and_missing_agreement() {
        let required = required();
        for case in 0..7 {
            let mut inputs = rows();
            let code = match case {
                0 => {
                    inputs[0].unit = "wet_mt".into();
                    RefusalCode::BasisMismatch105
                }
                1 => {
                    inputs[0].basis = "100%".into();
                    RefusalCode::UndeclaredUnitBasisOrTz107
                }
                2 => {
                    inputs[0].period = "2030FY".into();
                    RefusalCode::SupportMismatch102
                }
                3 => {
                    inputs[0].value = parse_token("8 - 10").unwrap();
                    RefusalCode::UndeclaredUnitBasisOrTz107
                }
                4 => {
                    inputs[0].readers.pop();
                    RefusalCode::NotObservable202
                }
                5 => {
                    inputs[0].locator.table = "Ordinary annual estimates".into();
                    RefusalCode::UndeclaredUnitBasisOrTz107
                }
                _ => {
                    inputs.pop();
                    RefusalCode::NotObservable202
                }
            };
            assert_eq!(
                limits_from_rows(&inputs, &document(), &required, EvidenceClass::Assumed)
                    .unwrap_err()
                    .code,
                code.spec().code
            );
        }
        let mut future = document();
        future.published_at = "2030-08-01".into();
        assert_eq!(
            document_admitted(&future, "2030-07-31").unwrap_err().code,
            RefusalCode::AssayLookahead108.spec().code
        );
        future = document();
        future.kind = "investor_deck".into();
        assert_eq!(
            document_admitted(&future, "2030-07-31").unwrap_err().code,
            RefusalCode::UndeclaredUnitBasisOrTz107.spec().code
        );
    }

    #[test]
    fn registered_document_binding_and_byte_tampering() {
        let root = std::env::temp_dir().join(format!("fmine-capacity-e2e-{}", std::process::id()));
        fmine_public::synthetic::write_issuer_workspace(&root).unwrap();
        std::fs::write(root.join("corpus/Bounds.pdf"), BYTES).unwrap();
        let registry = root.join("registries/sources/synthetic.toml");
        let mut text = std::fs::read_to_string(&registry).unwrap();
        let doc = document();
        text.push_str(&format!(
            "\n[[document]]\nfile = \"Bounds.pdf\"\nkind = \"technical_report\"\n\
             url = \"https://example.invalid/Bounds.pdf\"\nurl_basis = \"W1 table\"\n\
             published_at = \"2030-07-01\"\npublished_at_basis = \"FACT: synthetic date\"\n\
             pages = 1\nbytes = {}\nsha256 = \"{}\"\n",
            BYTES.len(),
            doc.sha256,
        ));
        std::fs::write(&registry, text).unwrap();
        std::fs::create_dir_all(root.join("registries/transcriptions")).unwrap();
        let transcription_path = root.join("registries/transcriptions/synthetic.toml");
        std::fs::write(&transcription_path, readings(&rows())).unwrap();
        let base = fmine_public::guidance::run_synthetic().unwrap();
        let report = assess(
            &root,
            "synthetic",
            "example-mine",
            "2030Q2",
            "2030-07-31",
            "Bounds.pdf",
            &base,
        )
        .unwrap();
        assert_eq!(report.sources.len(), 3);
        for claim in [
            &report.limits.feed,
            &report.limits.grade,
            &report.limits.recovery,
        ] {
            assert_eq!(claim.evidence, EvidenceClass::Assumed);
            claim.check_id().unwrap();
        }
        assert_eq!(report.comparison.verdict, CapacityVerdict::NotExcluded);
        assert_eq!(report.guidance_digest, base.digest);
        assert!(
            report
                .to_json()
                .contains("\"scope\":\"gold_production_ceiling_only\"")
        );
        assert!(
            report
                .to_markdown()
                .contains("T1 historical joint production-cost comparison remains separate")
        );
        assert_eq!(
            report.digest,
            assess(
                &root,
                "synthetic",
                "example-mine",
                "2030Q2",
                "2030-07-31",
                "Bounds.pdf",
                &base
            )
            .unwrap()
            .digest
        );
        let envelope =
            crate::guidance_capacity_envelope(&base, &report, "Invented synthetic evidence.");
        assert_eq!(envelope.status, "ok");
        assert!(envelope.anchor_id.is_none() && envelope.certificate.is_none());
        assert!(
            envelope
                .payload_json
                .contains("\"schema\":\"fmine.guidance.capacity-completion.v1\"")
        );
        assert!(envelope.payload_json.contains(&base.digest.to_hex()));
        assert!(envelope.payload_json.contains(&report.digest.to_hex()));
        let markdown = envelope.rendered_markdown.as_deref().unwrap();
        assert!(markdown.contains("T2: explicit technical-report production limits"));
        assert!(markdown.contains("Historical joint comparison"));
        assert!(markdown.contains("Composite report BLAKE3"));
        assert!(!markdown.contains(
            "no probability of meeting guidance or technical-report capacity claim is made"
        ));
        assert_eq!(
            envelope.to_json(),
            crate::guidance_capacity_envelope(&base, &report, "Invented synthetic evidence.")
                .to_json()
        );
        // The explicit document and role selection precedes optional byte/agreement checks.
        const OTHER: &[u8] = b"SYNTHETIC different technical-report bytes; not issuer data.";
        let mut sources = std::fs::read_to_string(&registry).unwrap();
        write!(
            sources,
            "\n[[document]]\nfile = \"OtherBounds.pdf\"\nkind = \"technical_report\"\n\
             url = \"https://example.invalid/OtherBounds.pdf\"\nurl_basis = \"W1 table\"\n\
             published_at = \"2030-07-01\"\npublished_at_basis = \"FACT: synthetic date\"\n\
             pages = 1\nbytes = {}\nsha256 = \"{}\"\n",
            OTHER.len(),
            sha256(OTHER).to_hex()
        )
        .unwrap();
        std::fs::write(&registry, sources).unwrap();
        let mut other = rows()[0].clone();
        other.locator.doc = "OtherBounds.pdf".into();
        other.doc_sha256 = sha256(OTHER).to_hex();
        let mut optional = rows()[0].clone();
        optional.locator.row = "Optional annual plan".into();
        optional.quantity = "technical_example_mine_remaining_dry_feed_mt".into();
        optional.readers = vec!["optional-a".into()];
        let selected_rows = rows();
        let mut extra_rows = selected_rows.clone();
        extra_rows.extend([other.clone(), optional.clone()]);
        std::fs::write(&transcription_path, readings(&extra_rows)).unwrap();
        let selected = || {
            assess(
                &root,
                "synthetic",
                "example-mine",
                "2030Q2",
                "2030-07-31",
                "Bounds.pdf",
                &base,
            )
        };
        assert_eq!(selected().unwrap().to_json(), report.to_json()); // Missing other document.
        std::fs::write(
            root.join("corpus/OtherBounds.pdf"),
            b"altered optional bytes",
        )
        .unwrap();
        assert_eq!(selected().unwrap().to_json(), report.to_json());
        std::fs::write(root.join("corpus/OtherBounds.pdf"), OTHER).unwrap();
        other.readers = vec!["other-c".into()];
        other.token = "11".into();
        optional.readers = vec!["optional-b".into()];
        optional.token = "11".into();
        extra_rows.extend([other, optional]);
        std::fs::write(&transcription_path, readings(&extra_rows)).unwrap();
        assert_eq!(selected().unwrap().to_json(), report.to_json()); // Both optional locators disagree.
        // A different key at a selected locator must still participate in its reader check.
        let mut third = selected_rows[0].clone();
        third.readers = vec!["selected-c".into()];
        third.quantity = "technical_example_mine_another_interpretation".into();
        extra_rows.push(third);
        std::fs::write(&transcription_path, readings(&extra_rows)).unwrap();
        assert_eq!(
            selected().unwrap_err().code,
            RefusalCode::NotObservable202.spec().code
        );
        // Optional source identity metadata remains validated globally, even when bytes are unselected.
        let valid = readings(&extra_rows[..extra_rows.len() - 1]);
        std::fs::write(
            &transcription_path,
            valid.replace(&sha256(OTHER).to_hex(), &"0".repeat(64)),
        )
        .unwrap();
        assert_eq!(
            selected().unwrap_err().code,
            RefusalCode::InputMissingOrUnreadable506.spec().code
        );
        std::fs::write(&transcription_path, readings(&selected_rows)).unwrap();
        std::fs::write(root.join("corpus/Bounds.pdf"), b"SYNTHETIC changed bytes").unwrap();
        assert_eq!(
            assess(
                &root,
                "synthetic",
                "example-mine",
                "2030Q2",
                "2030-07-31",
                "Bounds.pdf",
                &base
            )
            .unwrap_err()
            .code,
            RefusalCode::DigestMismatch505.spec().code
        );
    }
}
