#[cfg(test)]
mod tests {
    use super::*;

    fn render(t: &TranscriptionReport, cells: &[KeyedCell]) -> Result<GuidanceCompletionReport> {
        assemble(
            "synthetic",
            "example-mine",
            "2030Q2",
            "2030-07-31",
            "2030-07-31",
            t,
            cells,
            Ok(None),
            true,
        )
    }

    // The public synthetic completion contract is exercised in xtask/tests/public_guidance_history.rs.

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
        assert_eq!(render(&t, &changed).unwrap_err().code, "FMINE-REF-107");
        changed = cells.clone();
        changed[4].key.ownership_basis = OwnershipBasis::Consolidated100;
        assert_eq!(render(&t, &changed).unwrap_err().code, "FMINE-REF-105");
        changed = cells.clone();
        if let PublicValue::Number { under_status, .. } = &mut changed[0].claim.value {
            *under_status = Some(crate::key::StatusKind::Disposed);
        }
        assert_eq!(render(&t, &changed).unwrap_err().code, "FMINE-REF-208");
        changed = cells.clone();
        changed.push(cells[0].clone());
        assert_eq!(render(&t, &changed).unwrap_err().code, "FMINE-REF-104");
        changed = cells.clone();
        changed[3].claim.value = changed[4].claim.value.clone();
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
        assert_eq!(
            available_by(&profile, "2030-07-30").unwrap_err().code,
            "FMINE-REF-108"
        );
        assert_eq!(available_by(&profile, "2030-07-31").unwrap(), "2030-07-31");
    }
}
