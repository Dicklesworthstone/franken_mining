#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_bounds_do_not_become_unlabelled_rounded_decimals() {
        assert_eq!(number(Rat::new(263, 2).unwrap()), "131.5");
        assert_eq!(number(Rat::new(-1, 8).unwrap()), "−0.125");
        assert_eq!(number(Rat::new(1, 3).unwrap()), "1/3 (≈ 0.33)");
    }

    #[test]
    fn source_text_cannot_add_markdown_structure_or_html() {
        let report = fmine_public::guidance::run_synthetic().unwrap();
        let rendered = render(&report, "<script>|[link](bad)\n# forged");
        assert!(!rendered.contains("<script>"));
        assert!(!rendered.contains("\n# forged"));
        assert!(rendered.contains("&lt;script&gt;\\|\\[link\\](bad) # forged"));
        assert!(rendered.contains("Original support"));
        assert!(rendered.contains("Hypothetical replay"));
        assert_eq!(rendered, render(&report, "<script>|[link](bad)\n# forged"));
    }

    #[test]
    fn technical_scenario_retains_its_evidence_and_production_only_boundary() {
        let mut report = fmine_public::guidance::run_synthetic().unwrap();
        let rendered = render(&report, "Synthetic scenario");
        assert!(rendered.contains("**T2 —"));
        assert!(rendered.contains("Optimistic required head grade"));
        assert!(rendered.contains("does not supply a joint production/AISC witness"));
        assert!(rendered.contains("Maximum production under supplied limits"));
        let TechnicalAssessment::Compared(comparison) = &mut report.technical else {
            panic!("the synthetic example must exercise the technical adapter")
        };
        for source in &comparison.sources {
            assert!(rendered.contains(&text(&source.locator.to_string())));
            assert!(rendered.contains(&source.doc_sha256));
        }
        comparison.document.file = "<script>|[source](bad)\n# forged".into();
        let escaped = render(&report, "Synthetic scenario");
        assert!(!escaped.contains("<script>"));
        assert!(!escaped.contains("\n# forged"));
    }

    #[test]
    fn missing_technical_inputs_do_not_hide_the_requirements_or_history() {
        let mut report = fmine_public::guidance::run_synthetic().unwrap();
        report.technical = TechnicalAssessment::Indeterminate {
            reason: fmine_evidence::Refusal::new(
                fmine_evidence::RefusalCode::NoClaimInDomain201,
                "No agreed technical inputs",
            ),
        };
        let rendered = render(&report, "Synthetic scenario");
        assert!(rendered.contains("T3 — technical comparison indeterminate"));
        assert!(rendered.contains("FMINE-REF-201"));
        assert!(rendered.contains("Remaining production"));
        assert!(rendered.contains("Historical joint comparison"));
        assert!(!rendered.contains("Maximum production under supplied limits"));
    }

    #[test]
    fn capacity_section_keeps_the_historical_comparison_and_scopes_its_boundary() {
        let report = fmine_public::guidance::run_synthetic().expect("fixture");
        let ordinary = render(&report, "Synthetic");
        let with_capacity = render_with_capacity(
            &report,
            "Synthetic",
            "## T2 production ceiling\n\nNot excluded.",
        );
        assert!(with_capacity.contains("Historical joint comparison"));
        assert!(with_capacity.contains("T2 production ceiling"));
        assert!(with_capacity.contains("Not excluded."));
        assert!(with_capacity.contains("explicitly selected physical limits"));
        assert!(with_capacity.contains("Automatic technical-report scenario"));
        assert!(with_capacity.contains("Maximum production under supplied limits"));
        assert!(!with_capacity.contains(
            "no probability of meeting guidance or technical-report capacity claim is made"
        ));
        assert!(!ordinary.contains(
            "no probability of meeting guidance or technical-report capacity claim is made"
        ));
        assert!(
            ordinary.contains("Technical comparisons are conditional on their cited scenarios")
        );
    }
}
