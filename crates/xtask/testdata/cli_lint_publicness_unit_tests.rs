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
                retrieved_utc: None,
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

    const COST_RATE_DRAFT: &str = "Site A Q1 2030 attributable AISC per ounce sold is $2.0k/oz.";

    fn seal_cost_source(cell: &mut KeyedCell) {
        let old = &cell.claim;
        let mut claim = Claim::new(
            &cell.key.line.label,
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
        claim.supersedes = old.supersedes;
        claim.stale = old.stale;
        claim.calibration_failed = old.calibration_failed;
        cell.claim = claim;
    }

    fn cost_rate_fixture() -> (IssuerProfile, Vec<KeyedCell>, Vec<Publication>) {
        let (mut profile, original, dates) = fixture(&["500"]);
        profile.sources[0].kind = SourceKind::AiscReconciliation;
        let mut cells = vec![original[0].clone(), original[0].clone()];
        for (i, cell) in cells.iter_mut().enumerate() {
            cell.key.line.label = if i == 0 {
                "all-in sustaining costs"
            } else {
                "ounces sold"
            }
            .into();
            cell.key.unit = if i == 0 { "usd_m" } else { "koz" }.into();
            let at = if i == 0 { "QTD!D11" } else { "QTD!D13" };
            cell.claim.anchors[0].locator = at.into();
            cell.locator = format!("Quarter1.xlsx:{at}");
            cell.claim.additivity = AdditivityClass::ExtensiveAdditive;
            let raw = if i == 0 { "500" } else { "250" };
            cell.claim.value = PublicValue::Number {
                stored: raw.into(),
                displayed: Ok(raw.into()),
                under_status: None,
            };
            if i == 1 {
                cell.key.ownership_basis = OwnershipBasis::Attributable {
                    num: 3,
                    den: 5,
                    source: fmine_public::key::ShareSource::Registry,
                };
                cell.basis_evidence = Some(
                    "FACT: registered interpretation of explicitly attributable sold ounces".into(),
                );
            }
            seal_cost_source(cell);
        }
        (profile, cells, dates)
    }

    #[test]
    fn cost_rate_metadata_and_hand_derived_bounds_remain_source_bound_and_order_invariant() {
        let (profile, mut cells, dates) = cost_rate_fixture();
        for (i, cell) in cells.iter_mut().enumerate() {
            cell.claim.uu_terms.push(fmine_claim::UuTerm {
                scenario_id: format!("independent-source-tail-{i}"),
                description: "Unmodeled source revision".into(),
                downside_exposure_usd_m: 5.0 + i as f64,
                trigger_indicator: "A later filed reconciliation".into(),
            });
        }
        let original = evaluate(COST_RATE_DRAFT, &cells, &profile, &dates, "2030-05-01");
        let rate = original.findings[0].cost_rate.as_ref().unwrap();
        assert_eq!(rate.status, "derived");
        let json = rate.to_json();
        // 1000*(499.5)/(250.5) and 1000*(500.5)/(249.5), independently reduced.
        assert!(json.contains("\"lo\":{\"num\":\"333000\",\"den\":\"167\"}"));
        assert!(json.contains("\"hi\":{\"num\":\"1001000\",\"den\":\"499\"}"));
        assert!(json.contains("independent-source-tail-0"));
        assert!(json.contains("independent-source-tail-1"));
        assert!(json.contains("\"canonical_claim\":{\"claim_id\":"));
        for source in &cells {
            assert!(json.contains(&source.claim.claim_id.to_string()));
        }
        let precise = evaluate(
            &COST_RATE_DRAFT.replace("$2.0k", "$2,000"),
            &cells,
            &profile,
            &dates,
            "2030-05-01",
        );
        assert_eq!(
            precise.findings[0].cost_rate.as_ref().unwrap().status,
            "precision_not_established"
        );
        cells.reverse();
        assert_eq!(
            original.to_json(),
            evaluate(COST_RATE_DRAFT, &cells, &profile, &dates, "2030-05-01").to_json()
        );
        let before = cells[0].claim.claim_id;
        cells[0].claim.supersedes = Some(cells[1].claim.claim_id);
        cells[0].claim.check_id().unwrap();
        assert_eq!(before, cells[0].claim.claim_id);
        let revised = evaluate(COST_RATE_DRAFT, &cells, &profile, &dates, "2030-05-01");
        assert_eq!(
            revised.findings[0].cost_rate.as_ref().unwrap().status,
            "derived"
        );
        assert_ne!(
            original.digest(),
            revised.digest(),
            "source metadata outside Claim identity remains bound"
        );
    }

    #[test]
    fn cost_rate_basis_source_and_claim_mutations_never_acquire_a_numeric_derivation() {
        for fault in 0..11 {
            let (profile, mut cells, dates) = cost_rate_fixture();
            match fault {
                0 => cells[0].key.unit = "kt".into(),
                1 => {
                    cells[1].claim.anchors[0].locator = "QTD!E13".into();
                    cells[1].locator = "Quarter1.xlsx:QTD!E13".into();
                }
                2 => {
                    cells[1].key.scope = Scope::Region {
                        map_version: "registered:other-perimeter".into(),
                    }
                }
                3 => cells[1].key.line.occurrence = 1,
                4 => {
                    cells[0].printed_basis = OwnershipBasis::Consolidated100;
                    cells[0].basis_evidence = Some("Conflicting registered correction".into());
                }
                5 => cells[1].claim.additivity = AdditivityClass::IntensiveWeighted,
                6 => cells[0].claim.evidence = EvidenceClass::Assumed,
                7 => cells[1].claim.stale = true,
                8 => cells[1].key.valid_period.kind = PeriodKind::Quarter(2),
                9 => cells[0].claim.metric_key.push_str("-forged"),
                _ => cells[0].printed_basis = OwnershipBasis::Consolidated100,
            }
            if fault != 9 {
                for cell in &mut cells {
                    seal_cost_source(cell);
                }
            }
            let report = evaluate(COST_RATE_DRAFT, &cells, &profile, &dates, "2030-05-01");
            assert_ne!(
                report.findings[0].status,
                "derivable",
                "fault {fault}: {}",
                report.to_json()
            );
            let json = report.findings[0].cost_rate.as_ref().unwrap().to_json();
            assert!(
                json.contains("\"derived_claim\":null"),
                "fault {fault}: {json}"
            );
        }
    }

    #[test]
    fn cost_rate_duplicate_identity_metadata_and_forged_future_operands_block_selection() {
        let (mut profile, mut cells, mut dates) = cost_rate_fixture();
        let mut competing = cells[0].clone();
        competing.claim.supersedes = Some(cells[1].claim.claim_id);
        assert_eq!(competing.claim.claim_id, cells[0].claim.claim_id);
        cells.push(competing);
        let first = evaluate(COST_RATE_DRAFT, &cells, &profile, &dates, "2030-05-01");
        assert_eq!(
            first.findings[0].cost_rate.as_ref().unwrap().status,
            "ambiguous"
        );
        cells.reverse();
        assert_eq!(
            first.to_json(),
            evaluate(COST_RATE_DRAFT, &cells, &profile, &dates, "2030-05-01").to_json()
        );
        let mut future = cells.remove(0);
        future.claim.supersedes = None;
        let mut source = profile.sources[0].clone();
        source.file_name = "Future.xlsx".into();
        source.sha256_hex = Digest32([42; 32]).to_hex();
        future.claim.anchors[0].artifact_path = source.file_name.clone();
        future.claim.anchors[0].artifact_sha256 = [42; 32];
        future.locator = "Future.xlsx:QTD!D11".into();
        future.key.reporting_vintage.file_sha256 = source.sha256_hex.clone();
        let mut publication = dates[0].clone();
        publication.file = source.file_name.clone();
        publication.sha256 = source.sha256_hex.clone();
        publication.published_at = "2030-06-01".into();
        profile.sources.push(source);
        dates.push(publication);
        seal_cost_source(&mut future);
        cells.push(future);
        let later = evaluate(COST_RATE_DRAFT, &cells, &profile, &dates, "2030-05-01");
        assert_eq!(
            later.findings[0].cost_rate.as_ref().unwrap().status,
            "derived"
        );
        cells
            .last_mut()
            .unwrap()
            .claim
            .metric_key
            .push_str("-forged");
        let forged = evaluate(COST_RATE_DRAFT, &cells, &profile, &dates, "2030-05-01");
        assert_eq!(
            forged.findings[0].cost_rate.as_ref().unwrap().status,
            "ambiguous"
        );
        assert_ne!(forged.findings[0].status, "derivable");
    }

    fn copper_rate_fixture() -> (IssuerProfile, Vec<KeyedCell>, Vec<Publication>) {
        let (mut profile, mut cells, dates) = cost_rate_fixture();
        profile.sources[0].kind = SourceKind::MineStats;
        let ownership = OwnershipBasis::Attributable {
            num: 3,
            den: 5,
            source: fmine_public::key::ShareSource::Printed,
        };
        for (i, cell) in cells.iter_mut().enumerate() {
            cell.key.ownership_basis = ownership;
            cell.printed_basis = ownership;
            cell.basis_evidence = None;
            cell.key.line.label = if i == 0 {
                "site:costs ($ millions):c1 cash costs"
            } else {
                "site::sales (millions of pounds)"
            }
            .into();
            cell.key.unit = if i == 0 { "usd_m" } else { "Mlb" }.into();
            cell.claim.value = PublicValue::Number {
                stored: if i == 0 { "200.004" } else { "100.004" }.into(),
                displayed: Ok(if i == 0 { "200.00" } else { "100.00" }.into()),
                under_status: None,
            };
            seal_cost_source(cell);
        }
        (profile, cells, dates)
    }

    #[test]
    fn copper_cost_rate_never_converts_tonnes_or_infers_a_missing_or_conflicting_share() {
        let draft = "Site A Q1 2030 attributable C1 per pound sold is $2.00/lb.";
        let (profile, mut cells, dates) = copper_rate_fixture();
        let original = evaluate(draft, &cells, &profile, &dates, "2030-05-01");
        assert_eq!(original.findings[0].status, "derivable");
        cells.reverse();
        assert_eq!(
            original.to_json(),
            evaluate(draft, &cells, &profile, &dates, "2030-05-01").to_json()
        );
        for fault in 0..8 {
            let (profile, mut cells, dates) = copper_rate_fixture();
            match fault {
                0 => cells[1].key.unit = "kt".into(),
                1 => cells[0].printed_basis = OwnershipBasis::Unprinted,
                2 => cells[1].printed_basis = OwnershipBasis::Unprinted,
                3 => {
                    cells[0].key.ownership_basis = OwnershipBasis::Consolidated100;
                    cells[0].printed_basis = OwnershipBasis::Consolidated100;
                }
                4 => cells[0].printed_basis = OwnershipBasis::Consolidated100,
                5 => cells[0].key.unit = "cad_m".into(),
                6 => {
                    cells[0].key.ownership_basis = OwnershipBasis::Unprinted;
                    cells[0].printed_basis = OwnershipBasis::Attributable {
                        num: 0,
                        den: 1,
                        source: fmine_public::key::ShareSource::Printed,
                    };
                }
                _ => {
                    cells[0].key.scope = Scope::Region {
                        map_version: "unregistered:test".into(),
                    };
                    cells[1].key.scope = cells[0].key.scope.clone();
                }
            }
            for cell in &mut cells {
                seal_cost_source(cell);
            }
            let report = evaluate(draft, &cells, &profile, &dates, "2030-05-01");
            assert_ne!(
                report.findings[0].status,
                "derivable",
                "fault {fault}: {}",
                report.to_json()
            );
            if let Some(rate) = &report.findings[0].cost_rate {
                assert!(
                    rate.to_json().contains("\"derived_claim\":null"),
                    "fault {fault}"
                );
            }
        }
    }

    #[test]
    fn decimal_cost_rate_uses_individual_precision_and_signed_zero_numerator_bounds() {
        let (profile, mut cells, dates) = cost_rate_fixture();
        for (cell, token) in cells.iter_mut().zip(["0.00", "100.0"]) {
            cell.claim.value = PublicValue::Number {
                stored: token.into(),
                displayed: Ok(token.into()),
                under_status: None,
            };
            seal_cost_source(cell);
        }
        let draft = "Site A Q1 2030 attributable AISC per ounce sold is $0/oz.";
        let report = evaluate(draft, &cells, &profile, &dates, "2030-05-01");
        let rate = report.findings[0].cost_rate.as_ref().unwrap();
        assert_eq!(rate.status, "derived");
        // [-0.005, 0.005] US$M / [99.95, 100.05] koz; both extremes use the lower denominator.
        assert!(
            rate.to_json()
                .contains("\"lo\":{\"num\":\"-100\",\"den\":\"1999\"}")
        );
        assert!(
            rate.to_json()
                .contains("\"hi\":{\"num\":\"100\",\"den\":\"1999\"}")
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
