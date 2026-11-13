    use super::*;

    fn doc() -> DocumentRow {
        DocumentRow {
            file: "Report.pdf".into(),
            dir: "docs".into(),
            kind: "annual_report".into(),
            url: "https://example.test/report.pdf".into(),
            url_basis: "synthetic".into(),
            published_at: "2031-02-01".into(),
            published_at_basis: "synthetic".into(),
            pages: 2,
            bytes: 10,
            sha256: "a".repeat(64),
        }
    }

    fn text() -> String {
        "schema = \"fmine.table-identities.v1\"\nissuer = \"synthetic\"\n\
        [[identity]]\nid = \"reserves\"\ndoc = \"Report.pdf\"\nfamily = \"reserves\"\n\
        kind = \"roll_forward\"\ntotal = \"closing\"\ntotal_period = \"2030FY\"\n\
        unit = \"moz\"\nbasis = \"attributable;contained\"\nconstruction = \"rounded_once\"\nconvention = \"closed\"\n\
        [[term]]\nidentity = \"reserves\"\nquantity = \"opening\"\nperiod = \"2029FY\"\nsign = 1\nrole = \"opening\"\n\
        [[term]]\nidentity = \"reserves\"\nquantity = \"depletion\"\nperiod = \"2030FY\"\nsign = 1\nrole = \"move\"\n".into()
    }

    #[test]
    fn strict_declaration_has_explicit_signed_membership_and_canonical_order() {
        let registry = parse_table_identities(&text(), "synthetic", &[doc()]).unwrap();
        let identity = &registry.identities[0];
        assert_eq!(identity.terms[0].quantity, "depletion");
        assert_eq!(identity.terms[0].sign, 1);
        assert_eq!(identity.terms[1].period, "2029FY");
        for malformed in [
            text().replace("sign = 1", "sign = 0"),
            text().replace("role = \"opening\"", "role = \"move\""),
            text().replace("quantity = \"depletion\"", "quantity = \"closing\""),
            text().replace("Report.pdf", "Unknown.pdf"),
            text().replace("construction =", "unknown ="),
            text().replace("issuer = \"synthetic\"", "issuer = \"other\""),
        ] {
            assert!(parse_table_identities(&malformed, "synthetic", &[doc()]).is_err());
        }
    }

    #[test]
    fn families_and_documents_cannot_pick_rules_per_identity() {
        let mut registry = parse_table_identities(&text(), "synthetic", &[doc()]).unwrap();
        let mut second = registry.identities[0].clone();
        second.id = "other".into();
        second.construction = "component_sum".into();
        registry.identities.push(second);
        assert!(
            registry
                .validate(&[doc()])
                .unwrap_err()
                .reason
                .contains("construction")
        );
        registry.identities[1].construction = "rounded_once".into();
        registry.identities[1].convention = "half_up".into();
        assert!(
            registry
                .validate(&[doc()])
                .unwrap_err()
                .reason
                .contains("conventions")
        );
    }

    fn reserve_text() -> &'static str {
        "schema = \"fmine.table-identities.v1\"\nissuer = \"synthetic\"\n\
        [[reserve_row]]\nid = \"reserve_change\"\ndoc = \"Report.pdf\"\nquantity = \"reserve_change\"\n\
        subject = \"invented_issuer\"\nitem = \"net_change\"\nreserve_year = 2030\nperiod = \"2030FY\"\n\
        metal = \"gold\"\nunit = \"moz\"\nownership = \"full\"\nsupport = \"contained\"\nconvention = \"closed\"\n\
        deck = \"quoted\"\ndeck_period = \"2030FY\"\ndeck_basis = \"reserve_prices\"\ngold_price_quantity = \"gold_price\"\n"
    }

    #[test]
    fn reserve_only_definition_selects_every_value_and_price_without_duplicates() {
        let registry = parse_table_identities(reserve_text(), "synthetic", &[doc()]).unwrap();
        assert!(registry.identities.is_empty());
        assert_eq!(registry.reserve_rows[0].basis(), "gold;full;contained");
        assert_eq!(
            registry.quantities_by_document()["Report.pdf"],
            ["gold_price", "reserve_change"]
        );
        let mut merged = registry.clone();
        merged.identities = parse_table_identities(&text(), "synthetic", &[doc()])
            .unwrap()
            .identities;
        assert_eq!(
            merged.quantities_by_document()["Report.pdf"],
            [
                "closing",
                "depletion",
                "gold_price",
                "opening",
                "reserve_change"
            ]
        );
        merged.reserve_rows[0].convention = "half_up".into();
        assert!(
            merged
                .validate(&[doc()])
                .unwrap_err()
                .reason
                .contains("conventions")
        );
    }

    #[test]
    fn reserve_declarations_refuse_orphan_prices_duplicate_rows_and_incompatible_semantics() {
        for text in [
            reserve_text().replace("deck = \"quoted\"", "deck = \"unstated\""),
            reserve_text().replace("gold_price_quantity", "unregistered_price_quantity"),
            reserve_text().replace(
                "gold_price_quantity = \"gold_price\"",
                "gold_price_quantity = \"reserve_change\"",
            ),
            reserve_text().replace("metal = \"gold\"", "metal = \"geo\""),
            reserve_text().replace("period = \"2030FY\"", "period = \"2029FY\""),
            reserve_text().replace("ownership = \"full\"", "ownership = \"attributable:4/3\""),
            reserve_text().replace("support = \"contained\"", "support = \"unspecified\""),
            reserve_text().replace("unit = \"moz\"", "unit = \"usd_per_oz\""),
        ] {
            assert!(
                parse_table_identities(&text, "synthetic", &[doc()]).is_err(),
                "{text}"
            );
        }
        let mut registry = parse_table_identities(reserve_text(), "synthetic", &[doc()]).unwrap();
        let mut duplicate = registry.reserve_rows[0].clone();
        duplicate.id = "alias".into();
        registry.reserve_rows.push(duplicate);
        assert!(
            registry
                .validate(&[doc()])
                .unwrap_err()
                .reason
                .contains("document/quantity")
        );
    }

    fn geo_text() -> String {
        let mut text =
            "schema = \"fmine.table-identities.v1\"\nissuer = \"synthetic\"\n".to_string();
        for (id, metal, unit) in [
            ("gold", "gold", "moz"),
            ("copper", "copper", "mlb"),
            ("geo", "geo", "moz"),
            ("alternative", "geo", "moz"),
        ] {
            text.push_str(&format!(
                "[[reserve_row]]\nid = \"{id}\"\ndoc = \"Report.pdf\"\nquantity = \"{id}\"\n\
                subject = \"invented_issuer\"\nitem = \"closing\"\nreserve_year = 2030\nperiod = \"2030FY\"\n\
                metal = \"{metal}\"\nunit = \"{unit}\"\nownership = \"full\"\nsupport = \"contained\"\n\
                convention = \"closed\"\ndeck = \"quoted\"\ndeck_period = \"2030FY\"\ndeck_basis = \"reserve_prices\"\n\
                gold_price_quantity = \"gold_price\"\ncopper_price_quantity = \"copper_price\"\n"
            ));
        }
        text.push_str("[[geo_check]]\nid = \"conversion\"\ngold_row = \"gold\"\ncopper_row = \"copper\"\ngeo_row = \"geo\"\n[[geo_candidate]]\ncheck = \"conversion\"\nrow = \"alternative\"\n");
        text
    }

    #[test]
    fn geo_checks_bind_explicit_membership_and_existing_price_sources() {
        let mut registry = parse_table_identities(&geo_text(), "synthetic", &[doc()]).unwrap();
        assert_eq!(registry.geo_checks[0].candidate_deck_rows, ["alternative"]);
        assert_eq!(
            registry.quantities_by_document()["Report.pdf"],
            [
                "alternative",
                "copper",
                "copper_price",
                "geo",
                "gold",
                "gold_price"
            ]
        );
        for row in &mut registry.reserve_rows[..3] {
            row.ownership = "attributable:1/2".into();
        }
        registry.reserve_rows[0].ownership = "attributable:2/4".into();
        assert!(registry.validate(&[doc()]).is_ok());
        for text in [
            geo_text().replace("gold_row = \"gold\"", "gold_row = \"unknown\""),
            geo_text().replace("check = \"conversion\"", "check = \"orphan\""),
            geo_text().replace("row = \"alternative\"", "row = \"geo\""),
            geo_text().replace("unit = \"mlb\"", "unit = \"kt\""),
            format!(
                "{}[[geo_candidate]]\ncheck = \"conversion\"\nrow = \"alternative\"\n",
                geo_text()
            ),
        ] {
            assert!(parse_table_identities(&text, "synthetic", &[doc()]).is_err());
        }
    }

    #[test]
    fn geo_members_cannot_mix_source_vintages_or_economic_interpretations() {
        let registry = parse_table_identities(&geo_text(), "synthetic", &[doc()]).unwrap();
        let mut other_doc = doc();
        other_doc.file = "Other.pdf".into();
        other_doc.sha256 = "b".repeat(64);
        let docs = [doc(), other_doc];
        let changes: [fn(&mut ReserveDeclaration); 8] = [
            |row| row.doc = "Other.pdf".into(),
            |row| row.subject = "other_subject".into(),
            |row| row.item = "net_change".into(),
            |row| {
                row.reserve_year = 2029;
                row.period = "2029FY".into();
            },
            |row| row.ownership = "attributable:1/2".into(),
            |row| row.support = "recoverable".into(),
            |row| row.unit = "koz".into(),
            |row| row.convention = "truncate".into(),
        ];
        for change in changes {
            let mut invalid = registry.clone();
            change(&mut invalid.reserve_rows[0]);
            assert!(invalid.validate(&docs).is_err());
        }
        let mut invalid = registry;
        invalid.reserve_rows[3].copper_price_quantity = None;
        assert!(invalid.validate(&docs).is_err());
    }

    #[test]
    fn constructed_replacements_cannot_bypass_basis_window_or_role_validation() {
        let mut registry = parse_table_identities(&text(), "synthetic", &[doc()]).unwrap();
        let identity = &mut registry.identities[0];
        identity.kind = IdentityKind::ReserveReplacement;
        identity.total_period = "2028-2030".into();
        identity.unit = "percent".into();
        identity.basis = "attributable;contained;metal=gold;ex_ad;deck=none".into();
        identity.replacement = Some(Replacement {
            definition: "reserve_replacement.gold_oz@synthetic".into(),
            numerator_unit: "moz".into(),
            denominator_unit: "moz".into(),
            factor: 100,
            price_deck: "none".into(),
        });
        identity.terms = vec![
            Term {
                quantity: "net_change".into(),
                period: "2028-2030".into(),
                sign: 1,
                role: TermRole::Numerator,
            },
            Term {
                quantity: "depletion".into(),
                period: "2028-2030".into(),
                sign: -1,
                role: TermRole::Denominator,
            },
        ];
        assert!(registry.validate(&[doc()]).is_ok());
        let mutations: [fn(&mut TableIdentity); 6] = [
            |identity| identity.kind = IdentityKind::Sum,
            |identity| identity.replacement = None,
            |identity| identity.total_period = "2028-2031".into(),
            |identity| identity.basis = "attributable;contained".into(),
            |identity| identity.terms[1].sign = 0,
            |identity| identity.terms[1].role = TermRole::Member,
        ];
        for mutation in mutations {
            let mut invalid = registry.clone();
            mutation(&mut invalid.identities[0]);
            assert!(invalid.validate(&[doc()]).is_err());
        }
    }
