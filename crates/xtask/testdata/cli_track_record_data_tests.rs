#[cfg(test)]
mod tests {
    use super::*;
    use fmine_calib::guidance_scores::Side;
    use fmine_claim::{AdditivityClass, Claim, Representation};
    use fmine_evidence::{EvidenceClass, KnowledgeState};
    use fmine_public::key::{StatusKind, SubjectStatus};
    use std::fmt::Write as _;

    fn production(report: &TrackRecordReport) -> &TrackRecordRow {
        report
            .rows
            .iter()
            .find(|r| r.metric == "production_koz" || r.metric == "production_kt")
            .expect("production row")
    }

    fn reseal(cell: &mut KeyedCell) {
        let workbook = fmine_io::xlsx::Workbook {
            artifact_path: cell.claim.anchors[0].artifact_path.clone(),
            sha256: cell.claim.anchors[0].artifact_sha256,
            sheets: vec![],
            hidden_sheets: vec![],
            quarantined: vec![],
        };
        let canonical = fmine_public::key::public_claim(
            &workbook,
            "Annual",
            "C1",
            cell.key.clone(),
            cell.claim.value.clone(),
        );
        let c = &cell.claim;
        cell.claim = Claim::new(
            &cell.key.line.label,
            c.value.clone(),
            canonical.claim.value_tag(),
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

    fn revised(cell: &KeyedCell, year: u16, release: &str, token: &str) -> KeyedCell {
        let mut key = cell.key.clone();
        key.valid_period.year = year;
        key.reporting_vintage.release = release.into();
        let digest = sha256(format!("synthetic {release} FY{year} {token}").as_bytes());
        key.reporting_vintage.file_sha256 = digest.to_hex();
        let workbook = fmine_io::xlsx::Workbook {
            artifact_path: format!("synthetic/{release}/{year}.txt"),
            sha256: digest.0,
            sheets: vec![],
            hidden_sheets: vec![],
            quarantined: vec![],
        };
        fmine_public::key::public_claim(
            &workbook,
            "Annual",
            "C1",
            key,
            PublicValue::Number {
                stored: token.into(),
                displayed: Ok(token.into()),
                under_status: None,
            },
        )
    }

    #[test]
    fn synthetic_track_record_scores_exact_known_answer_and_keeps_missing_cost() {
        let r = synthetic().expect("report");
        assert_eq!(r.rows.len(), 2);
        let p = production(&r);
        assert_eq!(
            p.score.as_ref().expect("score").z,
            Some(Rat::new(3, 2).expect("rational"))
        );
        assert_eq!(p.score.as_ref().expect("score").side, Side::Above);
        assert!(matches!(
            p.outcome,
            OutcomeBinding::Resolved {
                comparative: false,
                ..
            }
        ));
        assert_eq!(
            r.summaries
                .iter()
                .map(|s| s.scores.n_members)
                .sum::<usize>(),
            2
        );
        assert_eq!(
            r.summaries
                .iter()
                .map(|s| s.scores.n_unresolved)
                .sum::<usize>(),
            1
        );
        assert!(r.to_json().contains("\"lane\":\"synthetic\""));
        assert!(!r.to_json().contains("9999-12-31"));
        assert!(
            r.to_json()
                .contains("\"iid_reference_interval_90_exact\":{\"lo\":")
        );
        let source = &p.source_binding.original.source_claims[0].claim;
        let calculation = p
            .source_binding
            .original
            .outcome
            .as_ref()
            .expect("synthetic interpretation");
        assert_eq!(source.state, KnowledgeState::Unknown);
        assert_eq!(calculation.state, KnowledgeState::Unknown);
        assert_eq!(calculation.evidence, EvidenceClass::Assumed);
        assert_eq!(calculation.metric_key, "guidance.interpreted_outcome");
        assert_eq!(calculation.parents, vec![source.claim_id]);
        assert_eq!(r.to_json(), synthetic().expect("rerun").to_json());
    }

    #[test]
    fn accented_registered_site_binds_ascii_guidance_and_site_selections() {
        let (mut profile, mut transcripts, mut cells) = synthetic_evidence().expect("fixture");
        profile.subjects[0] = "Zaldívar".into();
        for guidance in &mut transcripts.agreement.accepted {
            guidance.quantity = guidance.quantity.replace("example_mine", "zaldivar");
            guidance.locator.row = "Zaldívar".into();
        }
        cells[0].key.subject = "Zaldívar".into();
        reseal(&mut cells[0]);
        let report =
            assess(&profile, &transcripts, &cells, None).expect("registered accented site");
        assert_eq!(report.rows.len(), 2);
        assert_eq!(production(&report).site, "Zaldívar");
        assert_eq!(
            production(&report).score.expect("source score").z,
            Some(Rat::new(3, 2).expect("z"))
        );
        for site in ["zaldivar", "Zaldívar", "ZALDÍVAR"] {
            assert_eq!(
                report.to_json(),
                assess(&profile, &transcripts, &cells, Some(site))
                    .expect("equivalent site selection")
                    .to_json()
            );
        }
    }

    #[test]
    fn registered_copper_claim_declares_site_outside_aisc_headers() {
        let (mut profile, mut transcripts, mut cells) = synthetic_evidence().expect("fixture");
        profile.subjects.clear();
        transcripts
            .agreement
            .accepted
            .retain(|a| a.quantity.ends_with("production_koz"));
        let guidance = &mut transcripts.agreement.accepted[0];
        guidance.quantity = "guidance_example_copper_production_kt".into();
        guidance.unit = "kt".into();
        guidance.locator.row = "Example Copper".into();
        cells[0].key.subject = "Example Copper".into();
        cells[0].key.line.label = "summary:cu_prod_attr_kt".into();
        cells[0].key.unit = "kt".into();
        reseal(&mut cells[0]);
        let report = assess(&profile, &transcripts, &cells, Some("example-copper"))
            .expect("site declared by a registered canonical copper source");
        assert_eq!(report.rows.len(), 1);
        assert_eq!(production(&report).site, "Example Copper");
        assert_eq!(
            production(&report).score.expect("copper score").z,
            Some(Rat::new(3, 2).expect("z"))
        );

        let mut unregistered = profile.clone();
        unregistered.sources.clear();
        assert_eq!(
            assess(&unregistered, &transcripts, &cells, None)
                .expect_err("an unregistered source cannot introduce a site")
                .code,
            RefusalCode::NoClaimInDomain201.as_str()
        );
        let mut altered = cells.clone();
        altered[0].claim.value = PublicValue::Missing;
        assert_eq!(
            assess(&profile, &transcripts, &altered, None)
                .expect_err("a changed canonical source cannot introduce a site")
                .code,
            RefusalCode::NoClaimInDomain201.as_str()
        );
        cells[0].key.scope = Scope::Company;
        reseal(&mut cells[0]);
        assert_eq!(
            assess(&profile, &transcripts, &cells, None)
                .expect_err("a company total cannot introduce a site")
                .code,
            RefusalCode::NoClaimInDomain201.as_str()
        );
    }

    #[test]
    fn first_publication_is_not_replaced_by_later_acceptable_value() {
        let (p, t, c) = synthetic_evidence().expect("fixture");
        let later = revised(&c[0], 2030, "2031Q1", "110");
        let r = assess(&p, &t, &[later.clone(), c[0].clone()], None).expect("report");
        let row = production(&r);
        assert_eq!(row.score.expect("score").side, Side::Above);
        assert_eq!(row.later_outcomes.len(), 1);
        let without = assess(&p, &t, &[later], None).expect("missing original");
        assert!(
            matches!(&production(&without).outcome,OutcomeBinding::Unresolved{reason,..} if reason.contains("original annual outcome"))
        );
    }

    #[test]
    fn canonical_score_and_summary_preserve_source_ancestry_and_uu_terms() {
        let (profile, transcripts, mut cells) = synthetic_evidence().expect("fixture");
        let term = fmine_claim::UuTerm {
            scenario_id: "synthetic-reporting-change".into(),
            description: "Invented structural reporting uncertainty".into(),
            downside_exposure_usd_m: 0.0,
            trigger_indicator: "synthetic restatement".into(),
        };
        cells[0].claim.uu_terms.push(term.clone());
        let report = assess(&profile, &transcripts, &cells, None).expect("canonical report");
        let row = production(&report);
        let score = row.score_claim.as_ref().expect("canonical score");
        let outcome = row.member.outcome.as_ref().expect("canonical outcome");
        row.member.guidance.check_id().expect("input identity");
        assert_eq!(
            row.member.guidance.value_tag(),
            row.member.guidance.value.canonical_tag()
        );
        outcome.check_id().expect("outcome identity");
        score.check_id().expect("score identity");
        assert_eq!(score.value_tag(), score.value.canonical_tag());
        assert_eq!(outcome.parents, vec![cells[0].claim.claim_id]);
        assert_eq!(
            row.member.guidance.parents,
            vec![row.guidance_source.claim_id]
        );
        assert!(score.parents.contains(&row.member.guidance.claim_id));
        assert!(score.parents.contains(&outcome.claim_id));
        assert_eq!(score.uu_terms, vec![term.clone()]);
        assert_eq!(score.evidence, EvidenceClass::Assumed);
        assert_eq!(row.score, Some(GuidanceScore::from(&score.value)));
        let summary = report
            .summaries
            .iter()
            .find(|s| s.metric == "production_koz")
            .expect("summary");
        summary.claim.check_id().expect("summary identity");
        assert_eq!(
            summary.claim.value_tag(),
            summary.claim.value.canonical_tag()
        );
        assert_eq!(summary.claim.uu_terms, vec![term]);
        assert_eq!(
            summary.scores.iid_reference_interval_90_exact,
            summary.claim.value.binomial_90_exact
        );
        let missing = report
            .rows
            .iter()
            .find(|r| r.metric == "aisc_usd_per_oz")
            .expect("unresolved cost");
        let missing_summary = report
            .summaries
            .iter()
            .find(|s| s.metric == "aisc_usd_per_oz")
            .expect("unresolved summary");
        assert!(
            missing_summary
                .claim
                .parents
                .contains(&missing.member.guidance.claim_id)
        );
        assert_eq!(
            (
                missing_summary.claim.value.members,
                missing_summary.claim.value.unresolved
            ),
            (1, 1)
        );
        let json = report.to_json();
        for id in [
            row.guidance_source.claim_id,
            row.member.guidance.claim_id,
            outcome.claim_id,
            score.claim_id,
            summary.claim.claim_id,
        ] {
            assert!(json.contains(&format!("\"claim_id\":\"{id}\"")));
        }
        assert!(json.contains("synthetic-reporting-change"));
        let markdown = crate::track_record::render(&report);
        assert!(markdown.contains(&score.claim_id.to_string()));
        assert!(markdown.contains(&summary.claim.claim_id.to_string()));
        assert!(markdown.contains("Certified outward rational enclosure"));
    }

    #[test]
    fn publication_date_correction_changes_claim_ancestry_without_changing_arithmetic() {
        let (profile, mut transcripts, cells) = synthetic_evidence().expect("fixture");
        let before = assess(&profile, &transcripts, &cells, None).expect("first interpretation");
        transcripts.documents[0].published_at = "2030-01-16".into();
        let after =
            assess(&profile, &transcripts, &cells, None).expect("corrected publication date");
        let (first, next) = (production(&before), production(&after));
        assert_eq!(first.score, next.score);
        assert_ne!(
            first.guidance_source.claim_id,
            next.guidance_source.claim_id
        );
        assert_ne!(
            first.member.guidance.claim_id,
            next.member.guidance.claim_id
        );
        assert_ne!(
            first.score_claim.as_ref().unwrap().claim_id,
            next.score_claim.as_ref().unwrap().claim_id
        );
        assert_ne!(before.digest, after.digest);
    }

    #[test]
    fn rejected_original_retains_source_bytes_identity_and_later_appearance() {
        let (p, t, mut cells) = synthetic_evidence().expect("fixture");
        let later = revised(&cells[0], 2030, "2031Q1", "110");
        cells[0].claim.stale = true;
        let original = cells[0].clone();
        cells.push(later.clone());
        let report = assess(&p, &t, &cells, None).expect("unscored original");
        let row = production(&report);
        let OutcomeBinding::Unresolved { code, sources, .. } = &row.outcome else {
            panic!("original must stay unresolved");
        };
        assert_eq!(code, "FMINE-REF-502");
        assert_eq!(sources.len(), 1);
        assert_eq!(sources[0].to_json(), original.to_json());
        assert_eq!(row.later_outcomes[0].to_json(), later.to_json());
        assert!(row.score.is_none());
        let summary = report
            .summaries
            .iter()
            .find(|s| s.metric == "production_koz")
            .expect("summary");
        assert_eq!(
            (
                summary.scores.n_members,
                summary.scores.n_resolved,
                summary.scores.n_unresolved
            ),
            (1, 0, 1)
        );
        let json = report.to_json();
        assert!(json.contains(&format!("\"sources\":[{}]", original.to_json())));
        let markdown = crate::track_record::render(&report);
        assert!(markdown.contains("Rejected outcome evidence"));
        assert!(markdown.contains(&original.claim.claim_id.to_string()));
        assert!(markdown.contains(&original.key.reporting_vintage.file_sha256));
    }

    #[test]
    fn ambiguous_selected_sources_are_all_cited_in_canonical_order() {
        let (p, t, mut cells) = synthetic_evidence().expect("fixture");
        let conflicting = revised(&cells[0], 2030, "2030Q4", "110");
        cells.push(conflicting);
        let report = assess(&p, &t, &cells, None).expect("ambiguous originals");
        let row = production(&report);
        let OutcomeBinding::Unresolved { code, sources, .. } = &row.outcome else {
            panic!("simultaneous assertions must remain unresolved");
        };
        assert_eq!(code, "FMINE-REF-104");
        assert_eq!(sources.len(), 2);
        assert!(row.score.is_none());
        let json = report.to_json();
        let markdown = crate::track_record::render(&report);
        for source in &cells {
            assert!(sources.iter().any(|s| s.to_json() == source.to_json()));
            assert!(json.contains(&source.to_json()));
            assert!(markdown.contains(&source.claim.claim_id.to_string()));
            assert!(markdown.contains(&source.key.reporting_vintage.file_sha256));
        }
        cells.reverse();
        assert_eq!(
            json,
            assess(&p, &t, &cells, None).expect("reordered").to_json()
        );
    }

    #[test]
    fn attributable_guidance_never_binds_to_full_or_unprinted_ownership() {
        let (p, t, mut c) = synthetic_evidence().expect("fixture");
        for (basis, code) in [
            (OwnershipBasis::Consolidated100, "FMINE-REF-105"),
            (OwnershipBasis::Unprinted, "FMINE-REF-105"),
        ] {
            c[0].key.ownership_basis = basis;
            reseal(&mut c[0]);
            let r = assess(&p, &t, &c, None).expect("named unresolved outcome");
            assert!(
                matches!(&production(&r).outcome,OutcomeBinding::Unresolved{code:c,..} if c==code)
            );
            assert!(production(&r).score.is_none());
        }
    }

    #[test]
    fn partial_year_disposal_and_withholding_preserve_all_members() {
        let (mut p, t, c) = synthetic_evidence().expect("fixture");
        for (kind, first, last) in [
            (StatusKind::Disposed, (2030, 12), None),
            (StatusKind::Disposed, (2031, 1), None),
            (StatusKind::Withheld, (2030, 1), Some((2030, 9))),
        ] {
            p.subject_statuses = vec![SubjectStatus {
                subject: "Example Mine".into(),
                kind,
                first,
                last,
                evidence: "synthetic status".into(),
            }];
            let r = assess(&p, &t, &c, None).expect("unresolved annual perimeter");
            assert_eq!(r.rows.len(), 2);
            assert!(r.rows.iter().all(|r| r.score.is_none()));
            let OutcomeBinding::Unresolved { sources, .. } = &production(&r).outcome else {
                panic!("restricted outcome");
            };
            assert_eq!(sources.len(), 1);
            assert_eq!(sources[0].to_json(), c[0].to_json());
            assert_eq!(
                r.summaries
                    .iter()
                    .map(|s| s.scores.n_members)
                    .sum::<usize>(),
                2
            );
        }
    }

    #[test]
    fn comparative_members_keep_denominator_when_outcomes_disappear() {
        let (mut p, mut t, c) = synthetic_evidence().expect("fixture");
        t.agreement
            .accepted
            .retain(|a| a.quantity.ends_with("production_koz"));
        let mut doc = t.documents[0].clone();
        doc.file = "synthetic-guidance-2031.txt".into();
        doc.published_at = "2031-01-15".into();
        let mut row = t.agreement.accepted[0].clone();
        row.locator.doc = doc.file.clone();
        row.period = "2031FY".into();
        t.documents.push(doc);
        t.agreement.accepted.push(row);
        p.sources[0].quarter = "2032Q4".into();
        let mut actuals = vec![
            revised(&c[0], 2030, "2032Q4", "110"),
            revised(&c[0], 2031, "2032Q4", "110"),
        ];
        let before = assess(&p, &t, &actuals, None).expect("two comparative outcomes");
        assert_eq!(before.summaries.len(), 1);
        assert!(before.summaries[0].comparative);
        assert_eq!(before.summaries[0].scores.n_members, 2);
        actuals[1].claim.value = PublicValue::Nil {
            token: "—".into(),
            reason: fmine_public::key::NilReason::Unknown,
        };
        reseal(&mut actuals[1]);
        let after = assess(&p, &t, &actuals, None).expect("one missing comparative");
        let s = &after.summaries[0];
        assert_eq!(after.summaries.len(), 1);
        assert_eq!(
            (
                s.scores.n_members,
                s.scores.n_resolved,
                s.scores.n_unresolved
            ),
            (2, 1, 1)
        );
        assert_eq!(
            s.scores.whole_cohort_hit_bounds.expect("bound").lo,
            Rat::new(1, 2).expect("half")
        );
        assert_eq!(
            s.scores.whole_cohort_hit_bounds.expect("bound").hi,
            Rat::from_integer(1)
        );
        assert!(after.rows.iter().all(|r| r.comparative));
    }

    #[test]
    fn copper_uses_explicit_kt_and_rejects_unit_relabeling() {
        let (p, mut t, mut c) = synthetic_evidence().expect("fixture");
        t.agreement
            .accepted
            .retain(|a| a.quantity.ends_with("production_koz"));
        t.agreement.accepted[0].quantity = "guidance_example_mine_production_kt".into();
        t.agreement.accepted[0].unit = "kt".into();
        c[0].key.line.label = "summary:cu_prod_attr_kt".into();
        c[0].key.unit = "kt".into();
        reseal(&mut c[0]);
        let good = assess(&p, &t, &c, None).expect("copper source");
        assert_eq!(
            production(&good).score.expect("score").z,
            Some(Rat::new(3, 2).expect("z"))
        );
        c[0].key.unit = "Mlb".into();
        reseal(&mut c[0]);
        assert!(
            matches!(&production(&assess(&p,&t,&c,None).expect("unresolved unit")).outcome,OutcomeBinding::Unresolved{code,..} if code=="FMINE-REF-105")
        );
    }

    #[test]
    fn late_guidance_unknown_metrics_and_incomplete_readings_are_not_scored() {
        let (p, mut t, c) = synthetic_evidence().expect("fixture");
        t.documents[0].published_at = "2031-02-01".into();
        let late = assess(&p, &t, &c, None).expect("late rows retained");
        assert!(late.rows.iter().all(
            |r| matches!(&r.outcome,OutcomeBinding::Unresolved{code,..} if code=="FMINE-REF-108")
        ));
        t.documents[0].published_at = "2030-01-15".into();
        t.agreement.single_reader.push((
            t.agreement.accepted[0].locator.clone(),
            "reader-only".into(),
        ));
        let pending =
            assess(&p, &t, &c, None).expect("agreed rows survive unrelated pending readings");
        assert_eq!(pending.pending_transcriptions.len(), 1);
        assert!(production(&pending).score.is_some());
        t.agreement.single_reader.clear();
        t.agreement.accepted[0].quantity = "guidance_example_mine_capex_usd_m".into();
        let unsupported = assess(&p, &t, &c, None).expect("supported cost remains visible");
        assert_eq!(unsupported.unsupported_guidance.len(), 1);
        assert!(unsupported.rows.iter().all(|r| r.metric != "capex_usd_m"));
    }

    #[test]
    fn repeated_guidance_vintages_stay_separate_and_order_is_canonical() {
        let (p, mut t, c) = synthetic_evidence().expect("fixture");
        let mut doc = t.documents[0].clone();
        doc.file = "later-guidance.txt".into();
        doc.published_at = "2030-06-01".into();
        let mut row = t.agreement.accepted[0].clone();
        row.locator.doc = doc.file.clone();
        row.token = "120 - 130".into();
        row.value = parse_token(&row.token).expect("token");
        t.documents.push(doc);
        t.agreement.accepted.push(row);
        let before = assess(&p, &t, &c, None).expect("both vintages");
        let prod: Vec<_> = before
            .rows
            .iter()
            .filter(|r| r.metric == "production_koz")
            .collect();
        assert_eq!((prod[0].vintage, prod[1].vintage), (0, 1));
        assert_eq!(prod[0].score.expect("first").side, Side::Above);
        assert_eq!(prod[1].score.expect("later").side, Side::Inside);
        t.documents.reverse();
        t.agreement.accepted.reverse();
        assert_eq!(
            before.to_json(),
            assess(&p, &t, &c, None).expect("permutation").to_json()
        );
        t.documents[0].published_at = "2030-01-15".into();
        assert_eq!(
            assess(&p, &t, &c, None)
                .expect_err("ambiguous same-date vintages")
                .code,
            "FMINE-REF-104"
        );
    }

    fn series_basis(a: &Accepted, metric: &str) -> Result<String> {
        let ((attributable, share), _) = specification(a, metric)?;
        let mut basis = if attributable {
            format!("attributable:{}/{}", share.num(), share.den())
        } else {
            "100%".into()
        };
        if let Some((_, definition)) = a.basis.split_once(';') {
            basis.push(';');
            basis.push_str(definition);
        }
        Ok(basis)
    }

    fn multi_basis_evidence() -> (IssuerProfile, TranscriptionReport, Vec<KeyedCell>) {
        let (p, mut t, mut cells) = synthetic_evidence().expect("fixture");
        t.agreement
            .accepted
            .retain(|a| a.quantity.ends_with("production_koz"));
        let mut full = t.agreement.accepted[0].clone();
        full.locator.col = "100% production".into();
        full.basis = "100%".into();
        full.token = "140 - 160".into();
        full.value = parse_token(&full.token).expect("full-mine guidance");
        t.agreement.accepted.push(full);
        let mut actual = cells[0].clone();
        actual.key.ownership_basis = OwnershipBasis::Consolidated100;
        actual.printed_basis = OwnershipBasis::Consolidated100;
        actual.key.line.label = "summary:au_prod_100_koz".into();
        actual.locator = "synthetic/2030Q4/production.txt:Annual!B1".into();
        actual.claim.anchors[0].locator = "Annual!B1".into();
        actual.claim.value = PublicValue::Number {
            stored: "150".into(),
            displayed: Ok("150".into()),
            under_status: None,
        };
        reseal(&mut actual);
        cells.push(actual);
        (p, t, cells)
    }

    #[test]
    fn same_date_full_and_attributable_guidance_keep_independent_scores() {
        let (p, t, cells) = multi_basis_evidence();
        let report = assess(&p, &t, &cells, None).expect("independent ownership series");
        assert_eq!(report.rows.len(), 2);
        assert_eq!(report.summaries.len(), 2);
        for row in &report.rows {
            assert_eq!(row.vintage, 0);
            let (source, side) = if row.member.guidance.value.key.basis == "100%" {
                (&cells[1], Side::Inside)
            } else {
                assert_eq!(row.member.guidance.value.key.basis, "attributable:4/5");
                (&cells[0], Side::Above)
            };
            assert_eq!(row.score.expect("separate score").side, side);
            let outcome = row.member.outcome.as_ref().expect("interpreted outcome");
            assert_eq!(outcome.parents, vec![source.claim.claim_id]);
            row.score_claim
                .as_ref()
                .expect("score Claim")
                .check_id()
                .expect("canonical");
        }
        assert!(
            report
                .summaries
                .iter()
                .all(|s| s.vintage == 0 && s.scores.n_members == 1 && s.scores.n_resolved == 1)
        );
        let mut attributable_all = t.agreement.accepted[0].clone();
        attributable_all.basis = "attributable:1/1".into();
        assert_ne!(
            series_basis(&attributable_all, "production_koz").expect("attributable"),
            series_basis(&t.agreement.accepted[1], "production_koz").expect("full")
        );
    }

    #[test]
    fn ownership_series_revision_ordinals_do_not_interleave() {
        let (p, mut t, mut cells) = multi_basis_evidence();
        // The full-mine series starts later, but its first statement is still vintage zero.
        for (index, file, date) in [
            (1, "full-initial.txt", "2030-03-01"),
            (0, "attributable-revision.txt", "2030-06-01"),
            (1, "full-revision.txt", "2030-09-01"),
        ] {
            let mut doc = t.documents[0].clone();
            doc.file = file.into();
            doc.published_at = date.into();
            let mut row = t.agreement.accepted[index].clone();
            row.locator.doc = file.into();
            t.documents.push(doc);
            if file == "full-initial.txt" {
                t.agreement.accepted[index] = row;
            } else {
                t.agreement.accepted.push(row);
            }
        }
        let report = assess(&p, &t, &cells, None).expect("both series with revisions");
        assert_eq!(report.rows.len(), 4);
        assert_eq!(report.summaries.len(), 4);
        for basis in ["100%", "attributable:4/5"] {
            let rows: Vec<_> = report
                .rows
                .iter()
                .filter(|r| r.member.guidance.value.key.basis == basis)
                .collect();
            assert_eq!(
                rows.iter().map(|r| r.vintage).collect::<Vec<_>>(),
                vec![0, 1]
            );
            for summary in report.summaries.iter().filter(|s| s.basis == basis) {
                assert_eq!(summary.scores.n_members, 1);
                assert_eq!(summary.members, vec![rows[summary.vintage].id.clone()]);
            }
        }
        t.agreement.accepted.reverse();
        t.documents.reverse();
        cells.reverse();
        assert_eq!(
            report.to_json(),
            assess(&p, &t, &cells, None).expect("permutation").to_json()
        );
    }

    #[test]
    fn equivalent_fraction_spellings_share_one_history_without_losing_source_basis() {
        let (mut p, mut t, cells) = synthetic_evidence().expect("fixture");
        t.agreement
            .accepted
            .retain(|a| a.quantity.ends_with("production_koz"));
        let mut doc = t.documents[0].clone();
        doc.file = "guidance-2031.txt".into();
        doc.published_at = "2031-01-15".into();
        let mut row = t.agreement.accepted[0].clone();
        row.locator.doc = doc.file.clone();
        row.period = "2031FY".into();
        row.basis = "attributable:8/10".into();
        t.documents.push(doc);
        t.agreement.accepted.push(row);
        let next = revised(&cells[0], 2031, "2031Q4", "115");
        let mut source = p.sources[0].clone();
        source.file_name = "2031.txt".into();
        source.quarter = "2031Q4".into();
        source.sha256_hex = next.key.reporting_vintage.file_sha256.clone();
        p.sources.push(source);
        let mut outcomes = vec![cells[0].clone(), next];
        let report = assess(&p, &t, &outcomes, None).expect("canonical shares");
        assert_eq!(report.rows.len(), 2);
        assert_eq!(report.rows[1].guidance.basis, "attributable:8/10");
        assert!(
            report
                .rows
                .iter()
                .all(|r| r.member.guidance.value.key.basis == "attributable:4/5" && r.vintage == 0)
        );
        assert_eq!(report.summaries.len(), 1);
        let summary = &report.summaries[0];
        assert_eq!(summary.basis, "attributable:4/5");
        assert_eq!(
            (
                summary.scores.n_members,
                summary.scores.n_resolved,
                summary.scores.inside
            ),
            (2, 2, 1)
        );
        summary
            .claim
            .check_id()
            .expect("canonical combined history");
        let mut cost = t.agreement.accepted[0].clone();
        cost.unit = "usd_per_oz".into();
        cost.basis = "attributable:8/10;aisc=issuer-reported-per-sold-oz".into();
        assert_eq!(
            series_basis(&cost, "aisc_usd_per_oz").expect("cost definition"),
            "attributable:4/5;aisc=issuer-reported-per-sold-oz"
        );
        if let OwnershipBasis::Attributable { num, den, .. } = &mut outcomes[1].key.ownership_basis
        {
            *num = 8;
            *den = 10;
        }
        outcomes[1].printed_basis = outcomes[1].key.ownership_basis;
        reseal(&mut outcomes[1]);
        let equivalent = assess(&p, &t, &outcomes, None).expect("equivalent source share");
        assert_eq!(equivalent.summaries[0].scores, summary.scores);
        assert_eq!(
            equivalent.rows[1]
                .member
                .outcome
                .as_ref()
                .unwrap()
                .value
                .key
                .basis,
            "attributable:4/5"
        );
    }

    #[test]
    fn same_date_equivalent_basis_duplicates_still_refuse() {
        let (p, t, cells) = synthetic_evidence().expect("fixture");
        for basis in ["attributable:4/5", "attributable:8/10"] {
            let mut transcripts = t.clone();
            let mut duplicate = transcripts.agreement.accepted[0].clone();
            duplicate.locator.col = "second attributable statement".into();
            duplicate.basis = basis.into();
            duplicate.token = "120 - 130".into();
            duplicate.value = parse_token(&duplicate.token).expect("conflicting statement");
            transcripts.agreement.accepted.push(duplicate);
            assert_eq!(
                assess(&p, &transcripts, &cells, None)
                    .expect_err("same-series ambiguity")
                    .code,
                "FMINE-REF-104"
            );
        }
    }

    #[test]
    fn point_range_and_rounding_boundary_have_no_invented_standardized_bias() {
        let (p, mut t, c) = synthetic_evidence().expect("fixture");
        let a = t
            .agreement
            .accepted
            .iter_mut()
            .find(|a| a.quantity.ends_with("production_koz"))
            .expect("row");
        a.token = "125 - 125".into();
        a.value = parse_token(&a.token).expect("token");
        let r = assess(&p, &t, &c, None).expect("point guidance");
        let score = production(&r).score.expect("score");
        assert!(score.boundary);
        assert!(score.z.is_none());
        assert_eq!(score.side, Side::Inside);
    }

    include!("track_record_registered_contract.rs");
}
