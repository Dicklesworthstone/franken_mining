//! FACT: independently hand-calculated synthetic operating alternatives.
//! Determinism: DeterministicPerIsa (conditional physical calculations).

use super::*;
use fmine_plant::{cards, grinding, production};

const MANIFEST: &str = include_str!("../../../../../tests/decisions/production_comparison.toml");
const LOW: &str = include_str!("../../../../../tests/plant/grinding_joint.toml");
const HIGH: &str = include_str!("../../../../../tests/plant/grinding_higher_power.toml");
const RECOVERY: &str = include_str!("../../../../../tests/plant/production_recovery.toml");
const PRODUCTION: &str = include_str!("../../../../../tests/plant/production_joint.toml");
const LOW_TERMS: &str = include_str!("../../../../../tests/decisions/production_valuation.toml");
const HIGH_TERMS: &str =
    include_str!("../../../../../tests/decisions/production_valuation_higher_power.toml");

fn q(n: i128, d: i128) -> Rat {
    Rat::new(n, d).unwrap()
}
fn source(text: &str, path: &str) -> SourceAnchor {
    SourceAnchor::new(
        path,
        sha256(text.as_bytes()).0,
        "complete synthetic worksheet",
    )
}
fn alternative(high: bool, change: impl Fn(&str) -> String) -> ValuationReceipt {
    let text = change(if high { HIGH } else { LOW });
    let grind = grinding::worksheet::evaluate(&text, &source(&text, "grinding.toml")).unwrap();
    let text = change(RECOVERY);
    let recovery = cards::worksheet::evaluate(&text, &source(&text, "recovery.toml")).unwrap();
    let text = change(PRODUCTION);
    let physical = production::worksheet::evaluate(
        &text,
        &source(&text, "production.toml"),
        grind.receipt(),
        recovery.ensemble_receipt().unwrap(),
    )
    .unwrap();
    let text = change(if high { HIGH_TERMS } else { LOW_TERMS });
    crate::valuation::worksheet::evaluate(&text, &source(&text, "terms.toml"), physical.receipt())
        .unwrap()
}
fn manifest() -> worksheet::Manifest {
    worksheet::parse(MANIFEST, &source(MANIFEST, "comparison.toml")).unwrap()
}
fn bind(spec: ComparisonSpec, original: &Claim<ComparisonSpec>) -> Claim<ComparisonSpec> {
    let tag = spec.canonical_tag();
    Claim::new(
        INPUT_METRIC,
        spec,
        &tag,
        original.representation.clone(),
        original.support,
        original.additivity,
        original.evidence,
        original.epistemic,
        original.state,
        original.confidentiality.clone(),
        original.uu_terms.clone(),
        original.parents.clone(),
        original.anchors.clone(),
    )
}

#[test]
fn paired_operating_modes_recompute_common_regret_and_keep_the_risk_tradeoff() {
    let low = alternative(false, str::to_owned);
    let high = alternative(true, str::to_owned);
    let compared = manifest().evaluate(&[low.clone(), high.clone()]).unwrap();
    let s = compared.decision().summary();
    assert_eq!(s.expected_value_winners, ["higher-power"]);
    assert_eq!(s.alternatives.len(), 3);
    let low_stats = s
        .alternatives
        .iter()
        .find(|a| a.action_id == "process")
        .unwrap();
    let high_stats = s
        .alternatives
        .iter()
        .find(|a| a.action_id == "higher-power")
        .unwrap();
    assert_eq!(low_stats.mean.major, q(25_308, 5));
    assert_eq!(high_stats.mean.major, q(45_616, 5));
    assert_eq!(low_stats.cvar95_loss.major, q(-808, 1));
    assert_eq!(high_stats.cvar95_loss.major, q(384, 1));
    // Common-state regret: low misses 9315.2 in hard; high misses 1192 in soft.
    assert_eq!(low_stats.mean_regret.major, q(23_288, 5));
    assert_eq!(high_stats.mean_regret.major, q(596, 1));
    assert_eq!(low_stats.max_regret.major, q(46_576, 5));
    assert_eq!(high_stats.max_regret.major, q(1192, 1));
    assert_eq!(compared.alternatives()[0], high);
    assert_eq!(compared.alternatives()[1], low);
    assert_eq!(compared.decision().claim().evidence, EvidenceClass::Assumed);
    assert_eq!(
        compared.decision().claim().confidentiality,
        Confidentiality::internal()
    );
    assert_eq!(
        *compared.decision(),
        card::evaluate(compared.decision().inputs()).unwrap()
    );
    let json = compared.to_json();
    assert!(json.contains("fmine.production-comparison.receipt.v1"));
    assert!(json.contains("\"submitted_manifest\":\"# SYNTHETIC"));
    let markdown = compared.to_markdown();
    assert!(markdown.contains("minus immediate cash margin"));
    assert!(markdown.contains("higher-power"));
    assert!(!markdown.contains("−NPV"));
}

#[test]
fn comparison_refuses_unmatched_states_weights_and_unrounded_baselines() {
    let low = alternative(false, str::to_owned);
    for high in [
        alternative(true, |s| s.replace("\"soft\"", "\"other-state\"")),
        alternative(true, |s| {
            s.replacen("weight = \"1/2\"", "weight = \"1/4\"", 1)
                .replacen("weight = \"1/2\"", "weight = \"3/4\"", 1)
        }),
        alternative(true, |s| {
            s.replace("do_nothing_usd = \"0\"", "do_nothing_usd = \"0.000001\"")
        }),
        alternative(true, |s| {
            s.replace(
                "with no opening stock",
                "under another commercial assumption with no opening stock",
            )
        }),
        alternative(true, |s| {
            let tail = s.find("[[uu]]").map(|i| &s[i..]).unwrap_or("");
            let body = s.split("[[scenario]]\nid = \"hard\"").next().unwrap();
            format!(
                "{}{tail}",
                body.replace("weight = \"1/2\"", "weight = \"1\"")
            )
        }),
    ] {
        assert_eq!(
            manifest().evaluate(&[low.clone(), high]).unwrap_err().code,
            "FMINE-REF-105"
        );
    }
    let almost_zero = alternative(true, |s| {
        s.replace("do_nothing_usd = \"0\"", "do_nothing_usd = \"0.000001\"")
    });
    assert!(almost_zero.points().iter().all(|p| p.do_nothing.minor == 0));
    assert_eq!(
        manifest().evaluate(&[low]).unwrap_err().code,
        "FMINE-REF-201"
    );
}

#[test]
fn complete_shared_population_changes_the_selected_mode_without_marginal_averaging() {
    let changed = |s: &str| {
        s.replacen("weight = \"1/2\"", "weight = \"19/20\"", 1)
            .replacen("weight = \"1/2\"", "weight = \"1/20\"", 1)
    };
    let receipt = manifest()
        .evaluate(&[alternative(false, changed), alternative(true, changed)])
        .unwrap();
    let s = receipt.decision().summary();
    assert_eq!(s.expected_value_winners, ["process"]);
    let low = s
        .alternatives
        .iter()
        .find(|a| a.action_id == "process")
        .unwrap();
    let high = s
        .alternatives
        .iter()
        .find(|a| a.action_id == "higher-power")
        .unwrap();
    assert_eq!(low.mean.major, q(30_834, 25));
    assert_eq!(high.mean.major, q(14_168, 25));
}

#[test]
fn support_population_and_declared_identity_are_admitted_before_comparing() {
    let input = manifest();
    let receipts = [
        alternative(false, str::to_owned),
        alternative(true, str::to_owned),
    ];
    for field in [
        "boundary",
        "circuit",
        "domain",
        "dry",
        "element",
        "commercial",
        "time",
    ] {
        let mut spec = input.declaration().value.clone();
        match field {
            "boundary" => spec.boundary.push_str("-other"),
            "circuit" => spec.circuit.push_str("-other"),
            "domain" => spec.material_domain.push_str("-other"),
            "dry" => spec.dry_basis.push_str("-other"),
            "element" => spec.element = "Cu".into(),
            "commercial" => spec.commercial_support.push_str("-other"),
            _ => spec.end_ns += 1,
        }
        assert_eq!(
            evaluate(&bind(spec, input.declaration()), &receipts)
                .unwrap_err()
                .code,
            "FMINE-REF-102"
        );
    }
    let mut changed = input.declaration().clone();
    changed.value.shared_scenario_basis.push_str(" edited");
    assert_eq!(
        evaluate(&changed, &receipts).unwrap_err().code,
        "FMINE-REF-507"
    );
    let mut duplicate = input.declaration().value.clone();
    duplicate.action_ids = vec!["process".into(), "process".into()];
    assert_eq!(
        evaluate(&bind(duplicate, input.declaration()), &receipts)
            .unwrap_err()
            .code,
        "FMINE-REF-105"
    );
    assert_eq!(
        input
            .evaluate(&[receipts[0].clone(), receipts[0].clone()])
            .unwrap_err()
            .code,
        "FMINE-REF-105"
    );
}

#[test]
fn order_and_ties_are_preserved_but_every_source_and_assumption_binds_identity() {
    let input = manifest();
    let low = alternative(false, str::to_owned);
    let high = alternative(true, str::to_owned);
    let receipt = input.evaluate(&[low.clone(), high.clone()]).unwrap();
    assert_eq!(
        receipt,
        input.evaluate(&[high.clone(), low.clone()]).unwrap()
    );
    let changed_low = alternative(false, |s| {
        format!("# Additional synthetic source note\n{s}")
    });
    let changed = input.evaluate(&[changed_low, high]).unwrap();
    assert_eq!(receipt.decision().summary(), changed.decision().summary());
    assert_ne!(receipt.digest(), changed.digest());
    let mut spec = input.declaration().value.clone();
    spec.shared_scenario_basis
        .push_str("; stronger declared coupling assumption");
    assert_ne!(
        receipt.digest(),
        evaluate(&bind(spec, input.declaration()), receipt.alternatives())
            .unwrap()
            .digest()
    );
    let mut declaration = input.declaration().clone();
    declaration.supersedes = Some(low.decision().claim().claim_id);
    assert_ne!(
        receipt.digest(),
        evaluate(&declaration, receipt.alternatives())
            .unwrap()
            .digest()
    );
    let tied = alternative(false, |s| {
        s.replace("action_id = \"process\"", "action_id = \"higher-power\"")
    });
    let comparison = input.evaluate(&[low.clone(), tied]).unwrap();
    assert_eq!(
        comparison.decision().summary().expected_value_winners,
        ["higher-power", "process"]
    );
    assert!(
        comparison
            .decision()
            .summary()
            .alternatives
            .iter()
            .filter(|a| a.role == ActionRole::Action)
            .all(|a| a.max_regret.major == ZERO)
    );
    let incompatible_uu = alternative(true, |s| {
        s.replace(
            "Invented unmodelled product deductions",
            "Conflicting definition of product deductions",
        )
    });
    assert_eq!(
        input.evaluate(&[low, incompatible_uu]).unwrap_err().code,
        "FMINE-REF-204"
    );
}

#[test]
fn manifest_is_strict_bounded_and_binds_complete_source_bytes() {
    for text in [
        MANIFEST.replace("shared_scenario_basis", "undeclared_scenario_basis"),
        MANIFEST.replace("id = \"higher-power\"", "id = \"process\""),
        MANIFEST.replace("id = \"higher-power\"", "id = \"do-nothing\""),
        MANIFEST.replace(
            "fmine.production-comparison.v1",
            "fmine.production-comparison.v2",
        ),
        MANIFEST.replace("[[alternative]]", "[[other]]"),
        MANIFEST.replace("end_ns = \"3600000000000\"", "end_ns = \"0\""),
        MANIFEST.replace("input =", "unknown_path ="),
        format!("{MANIFEST}\n{}", " ".repeat(worksheet::MAX_BYTES as usize)),
    ] {
        assert!(worksheet::parse(&text, &source(&text, "comparison.toml")).is_err());
    }
    let source = source(MANIFEST, "comparison.toml");
    assert_eq!(
        worksheet::parse(&format!("{MANIFEST}\n"), &source)
            .unwrap_err()
            .code,
        "FMINE-REF-505"
    );
    let parsed = manifest();
    assert_eq!(parsed.sources()[0].id, "higher-power");
    assert_eq!(
        parsed.declaration().anchors[0].artifact_sha256,
        source.artifact_sha256
    );
}

fn limited_manifest(floor: &str, maximum: &str) -> worksheet::Manifest {
    let text = format!(
        "{MANIFEST}\n[[downside_limit]]\nfloor_usd = \"{floor}\"\nmax_breach_probability = \"{maximum}\"\n"
    );
    worksheet::parse(&text, &source(&text, "comparison.toml")).unwrap()
}

#[test]
fn declared_downside_screen_selects_the_eligible_mode_without_hiding_the_nominal_winner() {
    let alternatives = [
        alternative(false, str::to_owned),
        alternative(true, str::to_owned),
    ];
    let plain = manifest().evaluate(&alternatives).unwrap();
    let input = limited_manifest("0", "1/4");
    let receipt = input.evaluate(&alternatives).unwrap();
    let summary = receipt.decision().summary();
    assert_eq!(
        summary.alternatives,
        plain.decision().summary().alternatives
    );
    assert_eq!(summary.expected_value_winners, ["higher-power"]);
    let screen = summary.downside.as_ref().unwrap();
    assert_eq!(screen.choices.len(), 3);
    assert_eq!(screen.nominal.winners[0].id, "process");
    assert_eq!(screen.nominal.best_expected_value, Some(q(25_308, 5)));
    assert_eq!(
        screen.nominal.expected_value_cost_of_limit,
        Some(q(20_308, 5))
    );
    for choice in &screen.choices {
        let risky = choice.choice.id == "higher-power";
        assert_eq!(
            choice.nominal_breach_probability,
            if risky { q(1, 2) } else { ZERO }
        );
        assert_eq!(choice.nominal_eligible, !risky);
    }
    let claim = receipt.decision().inputs().downside_limit.as_ref().unwrap();
    assert_eq!(claim.evidence, EvidenceClass::Assumed);
    assert_eq!(claim.confidentiality, Confidentiality::internal());
    assert!(claim.parents.contains(&input.declaration().claim_id));
    for key in [
        "downside_limit.floor_usd",
        "downside_limit.max_breach_probability",
    ] {
        assert!(claim.anchors.iter().any(|anchor| {
            anchor.artifact_sha256 == input.declaration().anchors[0].artifact_sha256
                && anchor.locator.ends_with(key)
        }));
    }
    assert_eq!(
        *receipt.decision(),
        card::evaluate(receipt.decision().inputs()).unwrap()
    );
    assert!(
        receipt
            .to_json()
            .contains("\"max_breach_probability\":\"1/4\"")
    );
    let markdown = receipt.to_markdown();
    assert!(markdown.contains("**Downside-eligible expected-value co-winners:** process"));
    assert!(markdown.contains("| higher-power | 1/2"));
    assert!(markdown.contains("Expected-value co-winners:** higher-power"));
}

#[test]
fn downside_floor_and_probability_equalities_are_inclusive_and_all_failed_has_no_fallback() {
    let alternatives = [
        alternative(false, str::to_owned),
        alternative(true, str::to_owned),
    ];
    for (floor, maximum, winners) in [
        ("808", "0", vec!["process"]),
        ("0", "1/2", vec!["higher-power"]),
        ("-384", "0", vec!["higher-power"]),
        ("20000", "1", vec!["higher-power"]),
        ("20000", "0", vec![]),
    ] {
        let receipt = limited_manifest(floor, maximum)
            .evaluate(&alternatives)
            .unwrap();
        let summary = receipt.decision().summary();
        assert_eq!(summary.expected_value_winners, ["higher-power"]);
        let screen = summary.downside.as_ref().unwrap();
        assert_eq!(
            screen
                .nominal
                .winners
                .iter()
                .map(|c| c.id.as_str())
                .collect::<Vec<_>>(),
            winners,
        );
        if winners.is_empty() {
            assert_eq!(
                screen.nominal.status,
                card::downside::DownsideStatus::NoEligibleChoice
            );
            assert!(screen.nominal.eligible_order.is_empty());
            assert_eq!(screen.nominal.best_expected_value, None);
            assert_eq!(screen.nominal.expected_value_cost_of_limit, None);
            assert!(receipt.to_markdown().contains("no fallback is selected"));
        }
    }
    let tied = alternative(false, |s| {
        s.replace("action_id = \"process\"", "action_id = \"higher-power\"")
    });
    let receipt = limited_manifest("808", "0")
        .evaluate(&[alternatives[0].clone(), tied])
        .unwrap();
    assert_eq!(
        receipt
            .decision()
            .summary()
            .downside
            .as_ref()
            .unwrap()
            .nominal
            .winners
            .iter()
            .map(|c| c.id.as_str())
            .collect::<Vec<_>>(),
        ["higher-power", "process"],
    );
}

#[test]
fn downside_limits_bind_source_identity_and_admit_the_same_model_currency_and_basis() {
    let alternatives = [
        alternative(false, str::to_owned),
        alternative(true, str::to_owned),
    ];
    let original = manifest();
    let limited = limited_manifest("0", "0.25");
    let mut without_limit = limited.declaration().value.clone();
    without_limit.downside_limit = None;
    assert_eq!(
        without_limit.canonical_tag(),
        original.declaration().value.canonical_tag()
    );
    assert_ne!(
        limited.declaration().claim_id,
        original.declaration().claim_id
    );
    let first = limited.evaluate(&alternatives).unwrap();
    let changed = limited_manifest("0/1", "1/4")
        .evaluate(&alternatives)
        .unwrap();
    assert_eq!(first.decision().summary(), changed.decision().summary());
    assert_ne!(first.digest(), changed.digest());
    let relaxed = limited_manifest("0", "1/2")
        .evaluate(&alternatives)
        .unwrap();
    assert_ne!(first.digest(), relaxed.digest());
    let mut tampered = limited.declaration().clone();
    tampered
        .value
        .downside_limit
        .as_mut()
        .unwrap()
        .max_breach_probability = q(1, 2);
    assert_eq!(
        evaluate(&tampered, &alternatives).unwrap_err().code,
        "FMINE-REF-507"
    );
    for field in ["model", "basis", "currency", "probability"] {
        let mut spec = limited.declaration().value.clone();
        let limit = spec.downside_limit.as_mut().unwrap();
        match field {
            "model" => limit.model_key.push_str("-foreign"),
            "basis" => limit.valuation_basis = "project_npv".into(),
            "currency" => limit.floor.ccy = Currency::CAD,
            _ => limit.max_breach_probability = q(3, 2),
        }
        assert_eq!(
            evaluate(&bind(spec, limited.declaration()), &alternatives)
                .unwrap_err()
                .code,
            if field == "probability" {
                "FMINE-REF-109"
            } else {
                "FMINE-REF-105"
            },
        );
    }
}

#[test]
fn manifest_limits_refuse_lossy_money_invalid_probabilities_duplicates_and_unknown_fields() {
    let valid = "\n[[downside_limit]]\nfloor_usd = \"-1/8\"\nmax_breach_probability = \"1/3\"\n";
    let text = format!("{MANIFEST}{valid}");
    let parsed = worksheet::parse(&text, &source(&text, "comparison.toml")).unwrap();
    let limit = parsed.declaration().value.downside_limit.as_ref().unwrap();
    assert_eq!(limit.floor, Money::from_minor(Currency::USD, -1250));
    assert_eq!(limit.max_breach_probability, q(1, 3));
    for invalid_limit in [
        valid.replace("-1/8", "0.00001"),
        valid.replace("-1/8", "1/3"),
        valid.replace("-1/8", "170141183460469231731687303715884105727"),
        valid.replace("1/3", "-1/3"),
        valid.replace("1/3", "4/3"),
        valid.replace("1/3", "NaN"),
        valid.replace("1/3", "1/0"),
        valid.replace("max_breach_probability", "unknown_probability"),
        format!("{valid}\nmax_breach_probability = \"1/2\""),
        format!("{valid}{valid}"),
    ] {
        let text = format!("{MANIFEST}{invalid_limit}");
        assert!(
            worksheet::parse(&text, &source(&text, "comparison.toml")).is_err(),
            "{invalid_limit}"
        );
    }
}
