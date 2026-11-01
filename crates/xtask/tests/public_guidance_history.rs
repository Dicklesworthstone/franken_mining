//! Cross-crate original-vintage selection and exact quarterly guidance completion (BD-342).

#[cfg(test)]
mod tests {
    use fmine_claim::{Claim, Confidentiality, Representation, SpatialSupport};
    use fmine_core::anchor::SourceAnchor;
    use fmine_core::hash::sha256;
    use fmine_evidence::{EpistemicStatus, EvidenceClass, KnowledgeState, KuKind};
    use fmine_metrics::guidance::{EnvelopeTier, HistoryVerdict, quarters};
    use fmine_numerics::exact_feas::Rat;
    use fmine_public::guidance::{self, history};
    use fmine_public::key::{
        KeyedCell, OwnershipBasis, PublicValue, ShareSource, parse_period_code,
    };

    #[test]
    fn synthetic_completion_uses_sold_not_produced_and_keeps_every_source() {
        use fmine_metrics::guidance as kernel;
        use fmine_numerics::exact_feas::Interval;
        use fmine_public::guidance::run_synthetic;

        fn bounds(lo: Rat, hi: Rat) -> Interval {
            Interval {
                lo,
                hi,
                lo_strict: false,
                hi_strict: false,
            }
        }

        let report = run_synthetic().unwrap();
        assert_eq!(
            report.completion.remaining_production.value.interval,
            bounds(Rat::new(263, 2).unwrap(), Rat::new(325, 2).unwrap())
        );
        assert_eq!(
            report.inputs.remaining_sold.value.interval,
            bounds(Rat::from_integer(139), Rat::from_integer(141))
        );
        assert_eq!(
            report.inputs.ytd_sold.value.interval.lo,
            Rat::new(119, 2).unwrap()
        );
        assert_eq!(
            report.completion.ytd_aisc_spend.value.interval.lo,
            Rat::new(108647, 800).unwrap()
        );
        assert_eq!(report.to_json(), run_synthetic().unwrap().to_json());
        assert!(report.to_json().contains("\"envelope_tier\":\"T1\""));
        assert_eq!(
            report.history.verdict,
            kernel::HistoryVerdict::WithinEnvelope
        );
        assert!(report.to_json().contains("\"lane\":\"synthetic\""));
        assert_eq!(report.inputs.remaining_sold.parents.len(), 2);
        assert_eq!(report.inputs.remaining_sold.anchors.len(), 2);
    }

    fn fixture() -> (guidance::GuidanceCompletionReport, Vec<KeyedCell>) {
        let report =
            guidance::run_synthetic().expect("explicitly invented public guidance fixture");
        let cells = report.history_sources[0].source_claims.clone();
        (report, cells)
    }

    /// Makes another explicitly invented source quarter, with a new source binding and Claim identity.
    fn cell(template: &KeyedCell, period: &str, release: &str, token: &str) -> KeyedCell {
        let mut c = template.clone();
        let bytes = format!(
            "invented quarterly source|{period}|{release}|{}|{token}",
            c.key.line.label
        );
        let digest = sha256(bytes.as_bytes());
        c.key.valid_period = parse_period_code(period).unwrap();
        c.key.reporting_vintage.release = release.into();
        c.key.reporting_vintage.file_sha256 = digest.to_hex();
        c.locator = format!("synthetic/{release}/{period}/{}.txt", c.key.line.label);
        c.claim = Claim::new(
            &c.key.line.label,
            PublicValue::Number {
                stored: token.into(),
                displayed: Ok(token.into()),
                under_status: None,
            },
            &c.key.canonical_tag(token),
            Representation::Deterministic,
            SpatialSupport::PeriodAggregate,
            template.claim.additivity,
            EvidenceClass::Assumed,
            EpistemicStatus::KnownUnknown(KuKind::Parametric),
            KnowledgeState::Estimated,
            Confidentiality::public(),
            vec![],
            vec![],
            vec![SourceAnchor::new(&c.locator, digest.0, "invented value")],
        );
        c
    }

    #[test]
    fn public_entry_point_retains_checked_history_sources_and_conditional_scope() {
        let (report, cells) = fixture();
        assert_eq!(
            (report.history.tier, report.history.verdict),
            (EnvelopeTier::T1, HistoryVerdict::WithinEnvelope)
        );
        assert_eq!(report.history_sources.len(), 1);
        assert_eq!(cells.len(), 6);
        assert!(report.history_exclusions.is_empty());
        let check = &report.history.checks[0];
        assert_eq!(check.id, "2029Q3..2029Q4");
        assert!(check.same_season);
        assert_eq!(check.totals.value.production_koz.lo, Rat::from_integer(139));
        assert_eq!(check.totals.value.production_koz.hi, Rat::from_integer(141));
        let w = check.witness.as_ref().unwrap();
        assert_eq!(w.evidence, EvidenceClass::Assumed);
        let original = quarters::QuarterlyHistory {
            id: report.history_sources[0].id.clone(),
            quarters: report.history_sources[0].quarters.clone(),
        };
        assert!(quarters::check_quarter_witness(&report.inputs, &original, &w.value).unwrap());
        for (source, converted) in cells.iter().zip(
            original
                .quarters
                .iter()
                .flat_map(|q| [&q.production, &q.sold, &q.aisc]),
        ) {
            assert_eq!(converted.parents, [source.claim.claim_id]);
            assert_eq!(converted.anchors, source.claim.anchors);
            assert_eq!(converted.evidence, EvidenceClass::Assumed);
            assert!(w.parents.contains(&converted.claim_id));
        }
        let json = report.to_json();
        assert!(json.contains("\"envelope_tier\":\"T1\""));
        assert!(json.contains("within_supplied_history_under_sales_scenario"));
        assert!(json.contains("\"marginal_projections_only\":true"));
        assert!(json.contains("\"independently_checked\":true"));
        assert!(json.contains("never to mine capacity"));
        assert!(json.contains("no forecast, probability of meeting guidance"));
        assert_eq!(json, guidance::run_synthetic().unwrap().to_json());
    }

    #[test]
    fn rolling_and_cross_year_windows_are_compared_without_rewriting_source_periods() {
        let (report, mut cells) = fixture();
        for (template, token) in cells[..3].to_vec().iter().zip(["70", "70", "1300"]) {
            cells.push(cell(template, "2029Q2", "2029Q2", token));
        }
        let r = history::compare(&report.inputs, &cells).unwrap();
        assert_eq!(
            r.comparison
                .checks
                .iter()
                .map(|c| c.id.as_str())
                .collect::<Vec<_>>(),
            ["2029Q2..2029Q3", "2029Q3..2029Q4"]
        );
        assert!(!r.comparison.checks[0].same_season);
        assert!(r.comparison.checks[1].same_season);
        assert!(r.comparison.checks.iter().all(|c| c.witness.is_some()));
        cells.reverse();
        assert_eq!(
            r.comparison,
            history::compare(&report.inputs, &cells).unwrap().comparison
        );

        let (_, original) = fixture();
        let cross: Vec<_> = original
            .iter()
            .zip(["2028Q4", "2028Q4", "2028Q4", "2029Q1", "2029Q1", "2029Q1"])
            .zip(["70", "70", "1300", "70", "70", "1350"])
            .map(|((source, period), token)| cell(source, period, period, token))
            .collect();
        let r = history::compare(&report.inputs, &cross).unwrap();
        assert_eq!(r.comparison.verdict, HistoryVerdict::WithinEnvelope);
        assert_eq!(r.comparison.checks[0].id, "2028Q4..2029Q1");
        assert!(!r.comparison.checks[0].same_season);
        let q = &r.comparison.checks[0]
            .witness
            .as_ref()
            .unwrap()
            .value
            .quarters;
        assert_eq!(
            (q[0].source_period.year, q[0].source_period.first_month),
            (2028, 10)
        );
        assert_eq!(
            (q[1].source_period.year, q[1].source_period.first_month),
            (2029, 1)
        );
        assert_eq!(
            (q[0].replay_period.year, q[0].replay_period.first_month),
            (2030, 7)
        );
        assert_eq!(
            (q[1].replay_period.year, q[1].replay_period.first_month),
            (2030, 10)
        );
    }

    #[test]
    fn a_later_comparative_cannot_replace_the_missing_original_release() {
        let (report, mut cells) = fixture();
        let comparative = cell(&cells[0], "2029Q3", "2030Q3", "70000");
        cells.push(comparative.clone());
        assert_eq!(
            report.history,
            history::compare(&report.inputs, &cells).unwrap().comparison
        );
        cells.remove(0);
        let r = history::compare(&report.inputs, &cells).unwrap();
        assert_eq!(
            (r.comparison.tier, r.comparison.verdict),
            (EnvelopeTier::T3, HistoryVerdict::Indeterminate)
        );
        assert!(r.sources.is_empty());
        assert_eq!(r.exclusions.len(), 1);
        assert_eq!(r.exclusions[0].id, "2029Q3..2029Q4");
        assert_eq!(r.exclusions[0].code, "FMINE-REF-201");
        assert!(r.exclusions[0].reason.contains("2029Q3 in release 2029Q3"));
    }

    #[test]
    fn missing_history_names_its_gap_and_current_or_future_quarters_do_not_leak_in() {
        let (report, cells) = fixture();
        let empty = history::compare(&report.inputs, &[]).unwrap();
        assert_eq!(empty.comparison.tier, EnvelopeTier::T3);
        assert_eq!(empty.exclusions[0].code, "FMINE-REF-201");
        let short = history::compare(&report.inputs, &cells[..3]).unwrap();
        assert_eq!(short.comparison.tier, EnvelopeTier::T3);
        assert!(
            short.exclusions[0]
                .reason
                .contains("required remaining duration")
        );
        let mut with_future = cells.clone();
        with_future.extend([
            cell(&cells[0], "2030Q1", "2030Q1", "9999"),
            cell(&cells[0], "2031Q4", "2031Q4", "9999"),
        ]);
        let filtered = history::compare(&report.inputs, &with_future).unwrap();
        assert_eq!(filtered.comparison, report.history);
        assert!(filtered.exclusions.is_empty());
    }

    #[test]
    fn incompatible_basis_and_unusable_source_claims_are_explicit_exclusions() {
        let (report, cells) = fixture();
        let mut wrong_basis = cells.clone();
        wrong_basis[0].key.ownership_basis = OwnershipBasis::Attributable {
            num: 3,
            den: 4,
            source: ShareSource::Printed,
        };
        let mut placeholder = cells.clone();
        placeholder[0].claim.representation = Representation::Placeholder {
            token: "not supplied".into(),
        };
        let mut stale = cells.clone();
        stale[0].claim.stale = true;
        let mut unknown = cells;
        unknown[0].claim.state = KnowledgeState::Unknown;
        for (input, code) in [
            (wrong_basis, "FMINE-REF-105"),
            (placeholder, "FMINE-REF-208"),
            (stale, "FMINE-REF-502"),
            (unknown, "FMINE-REF-202"),
        ] {
            let r = history::compare(&report.inputs, &input).unwrap();
            assert_eq!(r.comparison.tier, EnvelopeTier::T3);
            assert!(r.sources.is_empty());
            assert_eq!(r.exclusions.len(), 1);
            assert_eq!(r.exclusions[0].code, code);
        }
    }

    #[test]
    fn changing_the_explicit_sales_scenario_can_exclude_an_otherwise_feasible_window() {
        let (report, cells) = fixture();
        let mut inputs = report.inputs;
        inputs.remaining_sold.value.interval.lo = Rat::from_integer(150);
        inputs.remaining_sold.value.interval.hi = Rat::from_integer(150);
        let r = history::compare(&inputs, &cells).unwrap();
        assert_eq!(
            (r.comparison.tier, r.comparison.verdict),
            (EnvelopeTier::T1, HistoryVerdict::ExceedsEnvelope)
        );
        let c = &r.comparison.checks[0];
        assert_eq!(
            (c.production_possible, c.cost_possible),
            (Some(true), Some(false))
        );
        assert!(c.witness.is_none());
        assert_eq!(c.certificates[0].component, "cost_and_sales");
        assert!(r.exclusions.is_empty());
    }
}
