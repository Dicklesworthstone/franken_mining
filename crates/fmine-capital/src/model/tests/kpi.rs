//! Strict source-model integration of finite KPI threshold comparisons.
//! Determinism: Exact

use super::*;

const KPI: &str = include_str!("../../../../../tests/capital/kpi_constraints.toml");

#[test]
fn source_model_dispatches_to_kpi_selection_and_retains_component_anchors() {
    let evaluated = evaluate(KPI, source(KPI)).unwrap();
    assert_eq!(evaluated.mode(), "declared_kpi_capital_comparison");
    assert_eq!(evaluated.no_claim(), crate::robustness::kpi::NO_CLAIM);
    let EvaluatedModel::Kpi(report) = evaluated else { panic!("KPI declarations must be evaluated") };
    assert_eq!(report.summary().constrained_winners, ["bundle_01"]);
    assert_eq!(report.summary().opportunity_cost, Some(Rat::from_integer(20)));
    for suffix in ["KPI constraint gold_aisc_cap", "KPI baseline gold_aisc_cap/base", "KPI increment gold_aisc_cap/efficient/base", "KPI increment gold_aisc_cap/growth/base", "model_key"] {
        assert!(report.design().anchors.iter().any(|a| a.locator.ends_with(suffix)), "{suffix}");
    }
    assert_eq!(report.design().evidence, EvidenceClass::Assumed);
    assert_eq!(report.design().confidentiality, Confidentiality::internal());
    assert!(report.to_json().contains("\"schema\":\"fmine.capital-kpi.v1\""));
    assert!(report.to_markdown().starts_with("# KPI-constrained capital comparison"));
    assert_eq!(evaluate(KPI, source(KPI)).unwrap().to_json(), report.to_json());
}

#[test]
fn static_only_parser_refuses_every_kpi_table_instead_of_ignoring_constraints() {
    for table in ["kpi_constraint", "kpi_baseline", "kpi_increment"] {
        let text = format!("{STATIC}\n{}", first_table_block(KPI, table));
        let refusal = parse(&text, source(&text)).unwrap_err();
        assert_eq!(refusal.code, "FMINE-REF-506");
        assert!(refusal.what.contains("unknown table"));
    }
}

#[test]
fn original_source_bytes_bind_the_kpi_result_even_when_economics_do_not_change() {
    let annotated = format!("{KPI}\n# same assumptions, new original source bytes\n");
    assert_eq!(evaluate(&annotated, source(KPI)).unwrap_err().code, "FMINE-REF-505");
    let EvaluatedModel::Kpi(before) = evaluate(KPI, source(KPI)).unwrap() else { panic!() };
    let EvaluatedModel::Kpi(after) = evaluate(&annotated, source(&annotated)).unwrap() else { panic!() };
    assert_eq!(before.summary(), after.summary());
    assert_ne!(before.digest(), after.digest());
    assert_ne!(before.design().claim_id, after.design().claim_id);
}

#[test]
fn malformed_incomplete_or_duplicate_kpi_rows_refuse_without_partial_selection() {
    for text in [
        KPI.replace("metric = \"gold_aisc\"", "metric = \"safety\""),
        KPI.replace("aggregation = \"ratio_of_expected_totals\"", "aggregation = \"mean_of_ratios\""),
        KPI.replace("direction = \"upper\"", "direction = \"less_than\""),
        KPI.replace("limit = \"3/2\"", "limit = \"1.5\""),
        format!("{KPI}\n{}", first_table_block(KPI, "kpi_baseline")),
        format!("{KPI}\n{}", first_table_block(KPI, "kpi_increment")),
    ] {
        assert_eq!(evaluate(&text, source(&text)).unwrap_err().code, "FMINE-REF-506");
    }
    let missing = KPI.replacen(&first_table_block(KPI, "kpi_increment"), "", 1);
    assert_eq!(evaluate(&missing, source(&missing)).unwrap_err().code, "FMINE-REF-201");
    let zero_denominator = KPI.replacen("denominator = \"100/1\"", "denominator = \"0/1\"", 1);
    assert_eq!(evaluate(&zero_denominator, source(&zero_denominator)).unwrap_err().code, "FMINE-REF-107");
}

#[test]
fn incompatible_recourse_and_stress_modes_refuse_before_a_kpi_selection() {
    let start = KPI.find("[[kpi_constraint]]").unwrap();
    let end = start + KPI[start..].find("[[uu]]").unwrap();
    let staged = format!("{STAGED}\n{}", &KPI[start..end]);
    assert_eq!(evaluate(&staged, source(&staged)).unwrap_err().code, "FMINE-REF-506");
    let stressed = format!("{KPI}\n[[robustness]]\nmethod = \"wasserstein_1\"\nradius = \"0/1\"\nmetric_basis = \"Explicit test geometry\"\nradius_basis = \"No stress\"\n[[transport_distance]]\nfrom = \"base\"\nto = \"base\"\nvalue = \"0/1\"\n");
    assert_eq!(evaluate(&stressed, source(&stressed)).unwrap_err().code, "FMINE-REF-506");
    let sampled = format!("{KPI}\n[[selection_stability]]\nmethod = \"weighted_multinomial\"\nseed = \"42\"\nreplicates = \"200\"\ndraws_per_replicate = \"1\"\nsampling_basis = \"Declared test bootstrap\"\n");
    assert_eq!(evaluate(&sampled, source(&sampled)).unwrap_err().code, "FMINE-REF-506");
}
