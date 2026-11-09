#[cfg(test)]
mod tests {
    use super::*;
    use fmine_claim::{AdditivityClass, Claim, Confidentiality};
    use fmine_core::anchor::SourceAnchor;
    use fmine_evidence::EpistemicStatus;
    use fmine_public::key::{LineKey, OwnershipBasis, PeriodKind, PublicKey, Vintage};
    use fmine_public::sources::SourceKind;

    fn fixture(values: &[&str]) -> (IssuerProfile, Vec<KeyedCell>, Vec<Publication>) {
        let mut profile = fmine_public::synthetic::synthetic_profile();
        profile.issuer = "example".to_string();
        profile.retrieved_utc = "2030-09-01T00:00:00Z".to_string();
        profile.sources.clear();
        let mut cells = Vec::new();
        let mut dates = Vec::new();
        for (i, value) in values.iter().enumerate() {
            let file = format!("Quarter{}.xlsx", i + 1);
            let hash = Digest32([u8::try_from(i + 1).expect("small fixture"); 32]);
            let release = format!("2030Q{}", i + 1);
            profile.sources.push(RegisteredSource {
                published_at: None,
                published_at_basis: None,
                file_name: file.clone(),
                kind: SourceKind::MineStats,
                quarter: release.clone(),
                url: format!("https://example.invalid/{file}"),
                url_basis: "W1 naming rule".to_string(),
                bytes: 1,
                sha256_hex: hash.to_hex(),
            });
            dates.push(Publication {
                file: file.clone(),
                sha256: hash.to_hex(),
                published_at: if i == 0 { "2030-05-01" } else { "2030-08-01" }.to_string(),
                published_at_basis: "FACT: synthetic publication calendar".to_string(),
            });
            let key = PublicKey {
                issuer: profile.issuer.clone(),
                subject: "Site A".to_string(),
                valid_period: Period {
                    year: 2030,
                    kind: PeriodKind::Quarter(1),
                },
                reporting_vintage: Vintage {
                    release,
                    file_sha256: hash.to_hex(),
                },
                ownership_basis: OwnershipBasis::Unprinted,
                scope: Scope::Site,
                unit: "usd_per_oz".to_string(),
                line: LineKey {
                    label: "summary:aisc_usd_per_oz".to_string(),
                    occurrence: 0,
                },
            };
            let value = PublicValue::Number {
                stored: (*value).to_string(),
                displayed: Ok((*value).to_string()),
                under_status: None,
            };
            let tag = canonical_value_tag(&key, &value);
            let claim = Claim::new(
                &key.line.label,
                value,
                &tag,
                Representation::Deterministic,
                SpatialSupport::PeriodAggregate,
                AdditivityClass::IntensiveWeighted,
                EvidenceClass::ISSUER,
                EpistemicStatus::KnownKnown,
                KnowledgeState::Known,
                Confidentiality::public(),
                vec![],
                vec![],
                vec![SourceAnchor::new(&file, hash.0, "QTD!D5")],
            );
            cells.push(KeyedCell {
                locator: format!("{file}:QTD!D5"),
                key,
                claim,
                printed_basis: OwnershipBasis::Unprinted,
                basis_evidence: None,
                status_evidence: None,
                label_as_printed: None,
                unit_evidence: None,
            });
        }
        (profile, cells, dates)
    }

    fn draft(c: &KeyedCell, amount: &str, basis: &str) -> String {
        format!(
            "Site A Q1 2030 {basis} AISC per ounce sold is [{amount}](fmine:{}).",
            c.claim.claim_id
        )
    }

    fn evaluate(
        text: &str,
        cells: &[KeyedCell],
        profile: &IssuerProfile,
        dates: &[Publication],
        cut: &str,
    ) -> PublicnessReport {
        assess(text, &lint_cited(text, cells), cells, profile, dates, cut).expect("assessment")
    }

    #[test]
    fn ownership_scaling_never_discards_conflicting_metadata_with_the_same_source_id() {
        let (profile, mut cells, dates) = fixture(&["100"]);
        cells.push(cells[0].clone());
        for (i, c) in cells.iter_mut().enumerate() {
            c.key.unit = "koz".into();
            c.key.line.label = if i == 0 {
                "summary:au_prod_100_koz"
            } else {
                "summary:au_sold_attr_koz"
            }
            .into();
            c.key.ownership_basis = if i == 0 {
                OwnershipBasis::Consolidated100
            } else {
                OwnershipBasis::Attributable {
                    num: 3,
                    den: 5,
                    source: fmine_public::key::ShareSource::Printed,
                }
            };
            c.printed_basis = c.key.ownership_basis;
            c.locator = format!("Quarter1.xlsx:QTD!{}5", if i == 0 { "D" } else { "E" });
            let mut anchor = c.claim.anchors[0].clone();
            anchor.locator = if i == 0 { "QTD!D5" } else { "QTD!E5" }.into();
            c.claim = Claim::new(
                &c.key.line.label,
                c.claim.value.clone(),
                &canonical_value_tag(&c.key, &c.claim.value),
                Representation::Deterministic,
                SpatialSupport::PeriodAggregate,
                AdditivityClass::ExtensiveAdditive,
                c.claim.evidence,
                c.claim.epistemic,
                c.claim.state,
                c.claim.confidentiality.clone(),
                vec![],
                vec![],
                vec![anchor],
            );
        }
        let text = "Site A Q1 2030 attributable gold production is 60 koz.";
        let positive = evaluate(text, &cells, &profile, &dates, "2030-05-01");
        assert_eq!(
            positive.findings[0].ownership.as_ref().unwrap().status,
            "derived"
        );
        let mut conflicting = cells[1].clone();
        conflicting.claim.uu_terms.push(fmine_claim::UuTerm {
            scenario_id: "same-id-distinct-exposure".into(),
            description: "Evidence requiring separate treatment".into(),
            downside_exposure_usd_m: 5.0,
            trigger_indicator: "Ownership disclosure correction".into(),
        });
        conflicting.claim.check_id().unwrap();
        assert_eq!(conflicting.claim.claim_id, cells[1].claim.claim_id);
        cells.push(conflicting);
        let first = evaluate(text, &cells, &profile, &dates, "2030-05-01");
        cells.reverse();
        let reversed = evaluate(text, &cells, &profile, &dates, "2030-05-01");
        for report in [&first, &reversed] {
            assert_eq!(
                report.findings[0].ownership.as_ref().unwrap().status,
                "ambiguous"
            );
            assert_ne!(report.findings[0].status, "derivable");
            assert!(
                report
                    .to_json()
                    .contains("same_source_identity_has_conflicting_claim_metadata")
            );
        }
        assert_eq!(first.to_json(), reversed.to_json());
    }

    #[test]
    fn quarter_sum_unverifiable_competitors_block_unless_positively_after_cut() {
        let (profile, mut cells, mut dates) = fixture(&["100.0", "200.0", "100.0"]);
        for (i, c) in cells.iter_mut().enumerate() {
            c.key.unit = "koz".into();
            c.key.line.label = "summary:au_prod_100_koz".into();
            c.key.valid_period.kind = PeriodKind::Quarter(if i == 1 { 2 } else { 1 });
            c.key.ownership_basis = OwnershipBasis::Consolidated100;
            c.printed_basis = OwnershipBasis::Consolidated100;
            c.claim = Claim::new(
                &c.key.line.label,
                c.claim.value.clone(),
                &canonical_value_tag(&c.key, &c.claim.value),
                Representation::Deterministic,
                SpatialSupport::PeriodAggregate,
                AdditivityClass::ExtensiveAdditive,
                c.claim.evidence,
                c.claim.epistemic,
                c.claim.state,
                c.claim.confidentiality.clone(),
                c.claim.uu_terms.clone(),
                vec![],
                c.claim.anchors.clone(),
            );
        }
        dates[2].published_at = "2030-08-02".into();
        cells[2].claim.metric_key.push_str("-tampered");
        let text = "Site A H1 2030 100% basis gold production is 300 koz.";
        let earlier = evaluate(text, &cells, &profile, &dates, "2030-08-01");
        assert_eq!(
            earlier.findings[0].quarter_sum.as_ref().unwrap().status,
            "derived"
        );
        assert_eq!(earlier.findings[0].status, "derivable");
        cells[2].locator.push_str("-unverifiable");
        let blocked = evaluate(text, &cells, &profile, &dates, "2030-08-01");
        let sum = blocked.findings[0].quarter_sum.as_ref().unwrap();
        assert_eq!(sum.status, "not_available");
        assert_ne!(blocked.findings[0].status, "derivable");
        assert_eq!(sum.evidence.len(), 3);
        assert!(sum.to_json().contains("unusable_quarter_operand"));
    }

    #[test]
    fn release_cut_excludes_future_bytes_then_admits_exact_cited_publication() {
        let (profile, cells, dates) = fixture(&["1250"]);
        let text = draft(&cells[0], "$1,250/oz", "ownership basis unprinted");
        let before = evaluate(&text, &cells, &profile, &dates, "2030-04-30");
        assert_eq!(before.findings[0].status, "not_in_public_record");
        assert_eq!(before.findings[0].evidence[0].admission, "after_cut");
        let after = evaluate(&text, &cells, &profile, &dates, "2030-05-01");
        assert_eq!(after.findings[0].status, "disclosed");
        assert!(after.to_json().contains(&dates[0].sha256));
        assert!(
            after
                .to_json()
                .contains("FACT: synthetic publication calendar")
        );
        assert!(after.to_markdown().contains("Quarter1.xlsx:QTD!D5"));
        assert_ne!(before.digest(), after.digest());
    }

    #[test]
    fn later_citation_can_resolve_to_earlier_matching_publication_without_overwriting_vintages() {
        let (profile, cells, dates) = fixture(&["1250", "1250"]);
        let text = draft(&cells[1], "$1,250/oz", "ownership basis unprinted");
        let report = evaluate(&text, &cells, &profile, &dates, "2030-06-01");
        assert_eq!(report.findings[0].status, "disclosed");
        assert_eq!(
            report.findings[0].first_registered_disclosure,
            Some(cells[0].claim.claim_id.to_string())
        );
        assert_eq!(report.findings[0].evidence.len(), 2);
        assert!(
            report.findings[0]
                .evidence
                .iter()
                .any(|e| e.admission == "after_cut")
        );
        let mut reversed = cells.clone();
        reversed.reverse();
        assert_eq!(
            report.to_json(),
            evaluate(&text, &reversed, &profile, &dates, "2030-06-01").to_json()
        );
    }

    #[test]
    fn contradictory_and_other_basis_assertions_never_become_disclosed_equalities() {
        let (profile, cells, dates) = fixture(&["1250"]);
        let text = draft(&cells[0], "$1,500/oz", "ownership basis unprinted");
        let report = evaluate(&text, &cells, &profile, &dates, "2030-06-01");
        assert_eq!(report.findings[0].status, "contradicts_disclosed");
        let other = draft(&cells[0], "$1,250/oz", "attributable");
        let report = evaluate(&other, &cells, &profile, &dates, "2030-06-01");
        assert_eq!(report.findings[0].status, "disclosed_other_basis");
        let unsupported = other.replace("Site A", "Another Mine");
        assert_eq!(
            evaluate(&unsupported, &cells, &profile, &dates, "2030-06-01").findings[0].status,
            "not_assessed"
        );
    }

    #[test]
    fn retrieval_dates_and_invented_precision_are_not_publication_evidence() {
        let (profile, cells, dates) = fixture(&["1250"]);
        let text = draft(&cells[0], "$1,250/oz", "ownership basis unprinted");
        let report = evaluate(&text, &cells, &profile, &[], "2030-10-01");
        assert_eq!(report.findings[0].status, "publication_date_unknown");
        assert_eq!(
            report.findings[0].evidence[0].known_available_by.as_deref(),
            Some("2030-09-01")
        );
        assert!(report.findings[0].first_registered_disclosure.is_none());
        let precise = draft(&cells[0], "$1,250.00/oz", "ownership basis unprinted");
        assert_eq!(
            evaluate(&precise, &cells, &profile, &dates, "2030-06-01").findings[0].status,
            "precision_not_established"
        );
    }

    #[test]
    fn discovery_preserves_ambiguous_equalities_but_identifies_dated_public_appearances() {
        let (profile, cells, dates) = fixture(&["1250", "1250"]);
        let text = "Site A Q1 2030 ownership basis unprinted AISC per ounce sold is $1,250/oz.";
        let lint = fmine_public::lint::lint_discover(text, &cells);
        assert_eq!(lint.findings[0].verdict, Verdict::Ambiguous);
        let report =
            assess(text, &lint, &cells, &profile, &dates, "2030-06-01").expect("dated appearances");
        assert_eq!(report.findings[0].status, "disclosed");
        assert_eq!(report.findings[0].evidence.len(), 2);
        assert_eq!(
            report.findings[0].first_registered_disclosure,
            Some(cells[0].claim.claim_id.to_string())
        );
        assert_eq!(lint.findings[0].verdict, Verdict::Ambiguous);
    }

    fn with_share(cell: &mut KeyedCell, num: i64, den: i64) {
        cell.key.ownership_basis = OwnershipBasis::Attributable {
            num,
            den,
            source: fmine_public::key::ShareSource::Printed,
        };
        cell.printed_basis = cell.key.ownership_basis;
        let old = &cell.claim;
        cell.claim = Claim::new(
            &old.metric_key,
            old.value.clone(),
            &canonical_value_tag(&cell.key, &old.value),
            old.representation.clone(),
            old.support,
            old.additivity,
            old.evidence,
            old.epistemic,
            old.state,
            old.confidentiality.clone(),
            old.uu_terms.clone(),
            old.parents.clone(),
            old.anchors.clone(),
        );
    }

    #[test]
    fn undisclosed_share_ambiguity_survives_singleton_value_probes() {
        for other_amount in ["1250", "1500"] {
            let (profile, mut cells, dates) = fixture(&["1250", other_amount]);
            with_share(&mut cells[0], 1, 2);
            with_share(&mut cells[1], 4, 5);
            let text = "Site A Q1 2030 attributable AISC per ounce sold is $1,250/oz.";
            let lint = fmine_public::lint::lint_discover(text, &cells);
            assert_eq!(lint.findings[0].verdict, Verdict::Ambiguous);
            assert_eq!(
                lint.findings[0].reason,
                "multiple_claims_matching_stated_keys"
            );
            let report = assess(text, &lint, &cells, &profile, &dates, "2030-09-01")
                .expect("ambiguous shares");
            assert_eq!(report.findings[0].status, "not_assessed");
            assert_eq!(report.findings[0].reason, "attributable_share_unspecified");
            assert_eq!(report.findings[0].evidence.len(), 2);
            assert!(report.findings[0].first_registered_disclosure.is_none());
            let cited = draft(&cells[0], "$1,250/oz", "attributable");
            assert_eq!(
                evaluate(&cited, &cells, &profile, &dates, "2030-09-01").findings[0].reason,
                "attributable_share_unspecified"
            );
        }
    }

    #[test]
    fn equivalent_shares_keep_the_earliest_matching_publication() {
        let (profile, mut cells, dates) = fixture(&["1250", "1250"]);
        with_share(&mut cells[0], 1, 2);
        with_share(&mut cells[1], 2, 4);
        let text = "Site A Q1 2030 attributable AISC per ounce sold is $1,250/oz.";
        let lint = fmine_public::lint::lint_discover(text, &cells);
        assert_eq!(lint.findings[0].verdict, Verdict::Ambiguous);
        let report =
            assess(text, &lint, &cells, &profile, &dates, "2030-09-01").expect("equivalent shares");
        assert_eq!(report.findings[0].status, "disclosed");
        assert_eq!(report.findings[0].evidence.len(), 2);
        assert_eq!(
            report.findings[0].first_registered_disclosure,
            Some(cells[0].claim.claim_id.to_string())
        );
        let cited = draft(&cells[1], "$1,250/oz", "attributable");
        let report = evaluate(&cited, &cells, &profile, &dates, "2030-09-01");
        assert_eq!(report.findings[0].status, "disclosed");
        assert_eq!(
            report.findings[0].first_registered_disclosure,
            Some(cells[0].claim.claim_id.to_string())
        );
    }

    #[test]
    fn a_different_claim_snapshot_cannot_reuse_an_equality_report() {
        let (profile, cells, dates) = fixture(&["1250"]);
        let text = draft(&cells[0], "$1,250/oz", "ownership basis unprinted");
        let lint = lint_cited(&text, &cells);
        assert_eq!(
            assess(&text, &lint, &[], &profile, &dates, "2030-06-01")
                .expect_err("snapshot mismatch")
                .code,
            "FMINE-REF-507"
        );
    }

    #[test]
    fn mismatched_publication_digest_and_changed_draft_refuse() {
        let (profile, cells, mut dates) = fixture(&["1250"]);
        let text = draft(&cells[0], "$1,250/oz", "ownership basis unprinted");
        dates[0].sha256 = Digest32([99; 32]).to_hex();
        assert!(
            assess(
                &text,
                &lint_cited(&text, &cells),
                &cells,
                &profile,
                &dates,
                "2030-06-01"
            )
            .is_err()
        );
        assert!(
            assess(
                "changed",
                &lint_cited(&text, &cells),
                &cells,
                &profile,
                &[],
                "2030-06-01"
            )
            .is_err()
        );
        dates[0].sha256 = profile.sources[0].sha256_hex.clone();
        let original = evaluate(&text, &cells, &profile, &dates, "2030-06-01");
        dates[0].published_at_basis = "INFERENCE: explicitly corrected calendar basis".to_string();
        assert_ne!(
            original.digest(),
            evaluate(&text, &cells, &profile, &dates, "2030-06-01").digest()
        );
    }

    #[test]
    fn altered_or_unknown_source_claim_cannot_gain_public_evidence_status() {
        let (profile, mut cells, dates) = fixture(&["1250"]);
        cells[0].claim.state = KnowledgeState::Unknown;
        // Preserve a valid Unknown Claim identity, rather than relying only on a stale digest to reject it.
        let old = &cells[0].claim;
        cells[0].claim = Claim::new(
            &old.metric_key,
            old.value.clone(),
            old.value_tag(),
            old.representation.clone(),
            old.support,
            old.additivity,
            EvidenceClass::Assumed,
            old.epistemic,
            old.state,
            old.confidentiality.clone(),
            old.uu_terms.clone(),
            old.parents.clone(),
            old.anchors.clone(),
        );
        let text = draft(&cells[0], "$1,250/oz", "ownership basis unprinted");
        assert_eq!(
            evaluate(&text, &cells, &profile, &dates, "2030-06-01").findings[0].status,
            "evidence_unusable"
        );
        let (profile, mut cells, dates) = fixture(&["1250"]);
        cells[0].locator = "Different.xlsx:QTD!D5".to_string();
        let text = draft(&cells[0], "$1,250/oz", "ownership basis unprinted");
        assert_eq!(
            evaluate(&text, &cells, &profile, &dates, "2030-06-01").findings[0].status,
            "evidence_unusable"
        );
    }
}
