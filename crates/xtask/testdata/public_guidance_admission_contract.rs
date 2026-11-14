#[cfg(test)]
mod tests {
    use super::*;

    fn render(t: &TranscriptionReport, cells: &[KeyedCell]) -> Result<GuidanceCompletionReport> {
        assemble(
            "synthetic",
            "example-mine",
            "2030Q2",
            "2030-07-31",
            &crate::sources::PublicationCut {
                available_by: Some("2030-07-31".into()),
                sources: Vec::new(),
            },
            t,
            cells,
            Ok(None),
            true,
        )
    }

    // The public synthetic completion contract is exercised in xtask/tests/public_guidance_history.rs.

    fn reissue(cell: &mut KeyedCell) {
        let c = &cell.claim;
        cell.claim = Claim::new(
            &c.metric_key,
            c.value.clone(),
            &crate::key::canonical_value_tag(&cell.key, &c.value),
            c.representation.clone(),
            c.support,
            c.additivity,
            c.evidence,
            c.epistemic,
            c.state,
            c.confidentiality.clone(),
            c.uu_terms.clone(),
            c.parents.clone(),
            c.anchors.clone(),
        );
    }

    #[test]
    fn edited_source_values_cannot_reuse_frozen_guidance_ancestry() {
        let (t, cells) = synthetic_evidence().unwrap();
        let original = render(&t, &cells).unwrap();
        for index in 0..5 {
            let mut edited = cells.clone();
            let PublicValue::Number { displayed, .. } = &mut edited[index].claim.value else {
                panic!("synthetic input must be numeric");
            };
            *displayed = Ok("900".into());
            assert_eq!(
                render(&t, &edited).map(|_| ()).unwrap_err().code,
                "FMINE-REF-507",
                "selected source {index} must retain its frozen value"
            );
        }
        assert_eq!(render(&t, &cells).unwrap().digest, original.digest);
    }

    #[test]
    fn source_interpretation_and_metadata_must_match_the_frozen_claim() {
        let (_, cells) = synthetic_evidence().unwrap();
        for mutation in 0..8 {
            let mut c = cells[0].clone();
            match mutation {
                0 => {
                    let PublicValue::Number { stored, .. } = &mut c.claim.value else {
                        panic!("numeric fixture");
                    };
                    *stored = "900".into();
                }
                1 => c.key.ownership_basis = OwnershipBasis::Consolidated100,
                2 => c.key.unit = "kt".into(),
                3 => c.claim.epistemic = EpistemicStatus::UnmodelledBudget,
                4 => c.claim.anchors[0].artifact_sha256 = [9; 32],
                5 => c.claim.metric_key = "summary:au_sold_attr_koz".into(),
                6 => c.claim.claim_id = cells[1].claim.claim_id,
                _ => c.key.reporting_vintage.file_sha256 = sha256(b"other source").to_hex(),
            }
            assert_eq!(
                actual_claim(&c, QuantityKind::ProductionKoz)
                    .unwrap_err()
                    .code,
                "FMINE-REF-507",
                "mutation {mutation} cannot acquire new provenance"
            );
        }
        let mut c = cells[0].clone();
        c.key.reporting_vintage.file_sha256 = sha256(b"unanchored vintage").to_hex();
        reissue(&mut c);
        assert_eq!(
            actual_claim(&c, QuantityKind::ProductionKoz)
                .unwrap_err()
                .code,
            "FMINE-REF-505"
        );
    }

    #[test]
    fn guidance_interpretation_cannot_disagree_with_the_reader_agreed_token() {
        let (mut t, cells) = synthetic_evidence().unwrap();
        t.agreement.accepted[0].value = parse_token("900 - 950").unwrap();
        assert_eq!(render(&t, &cells).unwrap_err().code, "FMINE-REF-507");
    }

    #[test]
    fn latest_guidance_obeys_cutoff_and_ambiguity_refuses() {
        let (mut t, cells) = synthetic_evidence().unwrap();
        let original = render(&t, &cells).unwrap().digest;
        let mut later = t.documents[0].clone();
        later.file = "future-guidance.txt".into();
        later.published_at = "2030-08-01".into();
        let mut rows = t.agreement.accepted.clone();
        for row in &mut rows {
            row.locator.doc = later.file.clone();
            row.token = "1 - 2".into();
            row.value = parse_token(&row.token).unwrap();
        }
        t.documents.push(later);
        t.agreement.accepted.extend(rows);
        assert_eq!(render(&t, &cells).unwrap().digest, original);
        t.documents[1].published_at = "2030-01-15".into();
        assert_eq!(render(&t, &cells).unwrap_err().code, "FMINE-REF-104");
        t.documents[1].published_at = "2030-07-01".into();
        assert_ne!(render(&t, &cells).unwrap().digest, original);
    }

    #[test]
    fn missing_or_unagreed_guidance_never_falls_back_to_earlier_values() {
        let (mut t, cells) = synthetic_evidence().unwrap();
        let original = render(&t, &cells).unwrap().digest;
        let mut pending_doc = t.documents[0].clone();
        pending_doc.file = "pending-guidance.txt".into();
        pending_doc.published_at = "2030-08-01".into();
        let mut pending = t.agreement.accepted[0].locator.clone();
        pending.doc = pending_doc.file.clone();
        t.documents.push(pending_doc);
        t.agreement
            .single_reader
            .push((pending, "first-reader".into()));
        assert_eq!(render(&t, &cells).unwrap().digest, original);
        t.documents[1].published_at = "2030-07-01".into();
        assert_eq!(render(&t, &cells).unwrap_err().code, "FMINE-REF-202");
        t.agreement.single_reader.clear();
        t.agreement.accepted[0].readers = vec!["only-one".into()];
        assert_eq!(render(&t, &cells).unwrap_err().code, "FMINE-REF-202");
        t.agreement.accepted.clear();
        assert_eq!(render(&t, &cells).unwrap_err().code, "FMINE-REF-201");
    }

    #[test]
    fn refuses_basis_status_ambiguity_and_zero_sales_instead_of_guessing() {
        let (t, cells) = synthetic_evidence().unwrap();
        let mut changed = cells.clone();
        changed[1].key.ownership_basis = OwnershipBasis::Unprinted;
        reissue(&mut changed[1]);
        assert_eq!(render(&t, &changed).unwrap_err().code, "FMINE-REF-107");
        changed = cells.clone();
        changed[4].key.ownership_basis = OwnershipBasis::Consolidated100;
        reissue(&mut changed[4]);
        assert_eq!(render(&t, &changed).unwrap_err().code, "FMINE-REF-105");
        changed = cells.clone();
        if let PublicValue::Number { under_status, .. } = &mut changed[0].claim.value {
            *under_status = Some(crate::key::StatusKind::Disposed);
        }
        reissue(&mut changed[0]);
        assert_eq!(render(&t, &changed).unwrap_err().code, "FMINE-REF-208");
        changed = cells.clone();
        changed.push(cells[0].clone());
        assert_eq!(render(&t, &changed).unwrap_err().code, "FMINE-REF-104");
        changed = cells.clone();
        changed[3].claim.value = changed[4].claim.value.clone();
        reissue(&mut changed[3]);
        assert!(render(&t, &changed).is_err());
    }

    #[test]
    fn source_mutation_moves_requirements_and_future_vintages_do_not() {
        let (t, mut cells) = synthetic_evidence().unwrap();
        let before = render(&t, &cells).unwrap();
        let mut future = cells[0].clone();
        future.key.reporting_vintage.release = "2031Q2".into();
        cells.push(future);
        assert_eq!(before.digest, render(&t, &cells).unwrap().digest);
        cells[3].claim.value = PublicValue::Number {
            stored: "220".into(),
            displayed: Ok("220".into()),
            under_status: None,
        };
        let digest = sha256(b"synthetic revised FY sales: 220");
        cells[3].key.reporting_vintage.file_sha256 = digest.to_hex();
        cells[3].claim.anchors[0].artifact_sha256 = digest.0;
        reissue(&mut cells[3]);
        let after = render(&t, &cells).unwrap();
        assert_ne!(before.digest, after.digest);
        assert_ne!(
            before.completion.remaining_aisc_rate.value.interval,
            after.completion.remaining_aisc_rate.value.interval
        );
    }

    #[test]
    fn converted_claims_keep_uncertainty_and_refuse_changed_support() {
        let (t, mut cells) = synthetic_evidence().unwrap();
        cells[3].claim.epistemic = EpistemicStatus::UnmodelledBudget;
        cells[3].claim.uu_terms.push(UuTerm {
            scenario_id: "synthetic-sales-interruption".into(),
            description: "Invented historical accounting uncertainty".into(),
            downside_exposure_usd_m: 1.0,
            trigger_indicator: "source revision".into(),
        });
        reissue(&mut cells[3]);
        let report = render(&t, &cells).unwrap();
        assert_eq!(
            report.inputs.remaining_sold.epistemic,
            EpistemicStatus::UnmodelledBudget
        );
        assert_eq!(
            report.completion.remaining_aisc_rate.uu_terms,
            cells[3].claim.uu_terms
        );
        assert!(report.to_json().contains("synthetic-sales-interruption"));
        cells[0].claim.support = SpatialSupport::PointCore;
        assert_eq!(render(&t, &cells).unwrap_err().code, "FMINE-REF-102");
        cells[0].claim.support = SpatialSupport::PeriodAggregate;
        cells[0].claim.additivity = AdditivityClass::IntensiveWeighted;
        assert_eq!(render(&t, &cells).unwrap_err().code, "FMINE-REF-103");
        cells[0].claim.stale = true;
        assert_eq!(render(&t, &cells).unwrap_err().code, "FMINE-REF-502");
        cells[0].claim.stale = false;
        cells[0].claim.calibration_failed = true;
        assert_eq!(render(&t, &cells).unwrap_err().code, "FMINE-REF-205");
    }

    #[test]
    fn invalid_dates_and_cuts_before_verified_availability_refuse() {
        assert!(request("2030Q2", "2030-02-30").is_err());
        assert_eq!(
            request("2030Q2", "2030-06-29").unwrap_err().code,
            "FMINE-REF-108"
        );
        assert!(request("2030Q4", "2031-01-31").is_err());
        let mut profile = crate::synthetic::synthetic_profile();
        profile.retrieved_utc = "2030-07-31T00:00:00Z".into();
        assert!(
            profile
                .mine_stats_as_of("2030-07-30")
                .unwrap()
                .available_by
                .is_none()
        );
        assert!(profile.mine_stats_as_of("2030-02-30").is_err());
    }
}
